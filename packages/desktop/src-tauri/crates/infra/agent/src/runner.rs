//! `ClaudeCliRunner`：CLI 租户实现——CLI 发现 → spawn → stdout 逐行泵 →
//! 逐行归一化发入有界 mpsc → EOF 无 result 补发合成收敛事件；停止信号到达
//! 即进程树击杀并中止行读取（停止路径不合成 result）。
//!
//! 进程细节（`cmd /C` shim 包装、stderr 排水、stdin 关闭、树杀）全部封在
//! 本模块，trait 面上只暴露逻辑事件流与句柄。

use std::path::Path;

use agent::{
    AgentEvent, AgentEventKind, AgentRun, AgentRunParams, AgentRunner, AgentStartError, RunHandle,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStdout, Command};
use tokio::sync::mpsc;

use crate::{discover, flags, jsonl};

/// 事件通道有界容量（背压策略：泵任务阻塞在 `send`，事件不丢、内存有界）。
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// EOF 无 result 事件时合成收敛事件的 subtype（design D6：收敛恒由
/// RunResult 驱动，状态机不需要 EOF 特判）。
const PROCESS_EXIT_SUBTYPE: &str = "error_process_exit";

/// 无状态 runner 构造。
pub struct ClaudeCliRunner;

impl ClaudeCliRunner {
    /// 无状态 runner：全部运行态在 `start` 产出的 [`AgentRun`] 内。
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeCliRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRunner for ClaudeCliRunner {
    fn start(&self, params: AgentRunParams) -> Result<AgentRun, AgentStartError> {
        let program = discover::discover()?;
        let args = flags::build_args(&params);
        let mut command = build_command(&program, &args, &params.cwd);
        let mut child = command.spawn().map_err(|e| {
            AgentStartError::SpawnFailed(format!("{} 启动失败: {e}", program.display()))
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentStartError::SpawnFailed("stdout 未按管道打开".to_owned()))?;
        let stderr = child.stderr.take();
        let (sender, receiver) = mpsc::channel::<AgentEvent>(EVENT_CHANNEL_CAPACITY);
        // stderr 排水任务：不读会撑满管道缓冲导致子进程写阻塞（内容本期不消费）
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(_)) = lines.next_line().await {}
            });
        }
        // 退出码任务：独占 child.wait()（EOF 后泵消费退出码）；停止路径经
        // 击杀缝终止进程后 wait 随之返回，oneshot 发送端静默失败（接收端已关）
        let pid = child.id();
        let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = exit_tx.send(child.wait().await.ok().and_then(|status| status.code()));
        });
        let handle = RunHandle::default();
        tokio::spawn(pump_with_kill(stdout, exit_rx, sender, handle.clone(), pid));
        Ok(AgentRun {
            events: receiver,
            handle,
        })
    }
}

/// 组装 spawn 命令：Windows `.cmd` / `.bat` shim 不可直接 spawn，经
/// `cmd /C` 包装（args 逐参传递）；cwd 经 `current_dir` 传递（非 flag）；
/// stdout 按管道打开，stderr 按管道打开供排水，stdin 关闭（无头无输入）。
fn build_command(program: &Path, args: &[String], cwd: &Path) -> Command {
    let needs_shim = cfg!(windows)
        && matches!(
            program.extension().and_then(std::ffi::OsStr::to_str),
            Some("cmd") | Some("bat")
        );
    let mut command = if needs_shim {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(program);
        command
    } else {
        Command::new(program)
    };
    command
        .args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    command
}

/// 真实进程泵：行流 + 退出码 oneshot + 击杀缝（捕获 spawn 时的 child pid）。
/// 退出码经任务转交以化解「wait 独占 child」与「击杀需 pid」的借用冲突。
async fn pump_with_kill(
    stdout: ChildStdout,
    exit_rx: tokio::sync::oneshot::Receiver<Option<i32>>,
    sender: mpsc::Sender<AgentEvent>,
    handle: RunHandle,
    pid: Option<u32>,
) {
    pump_lines(
        BufReader::new(stdout),
        async { exit_rx.await.unwrap_or(None) },
        sender,
        &handle,
        move || kill_process_tree(pid),
    )
    .await;
}

/// 进程树击杀（尽力语义，静默不重试不阻塞收敛）：Windows 下 `.cmd` shim 经
/// `cmd /C` 包装 spawn，捕获的 pid 即 cmd 进程，`taskkill /PID <pid> /T /F`
/// 连带 claude 孙进程整树终止；其余平台无包装、pid 即 claude 本进程，pid 级
/// 强杀与 `child.start_kill()`（SIGKILL）等价直达。失败不报错（能力 spec
/// `specs/desktop-agent-execution/spec.md`「进程树击杀失败」已知限制留痕，
/// 路径相对域根）；经 [`pump_lines`] 停止路径的击杀缝触发。
fn kill_process_tree(pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    #[cfg(windows)]
    {
        // GUI 进程 spawn 控制台命令须抑制窗口闪烁（CREATE_NO_WINDOW = 0x08000000）
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
}

/// 逐行泵核心（可注入缝，决策 D6/D8）：任意 AsyncRead 行流 + EOF 后退出码
/// future + 停止信号句柄 + 击杀缝 → select 行流 vs 停止信号：信号到达即中止
/// 行读取（已读事件保留）、调用击杀缝并直接返回——停止路径不取退出码、不补
/// 发合成 [`AgentEventKind::RunResult`]（给编排侧 stopped 显式收敛让路）。
/// 正常 EOF 后取退出码：未见 result 事件则补发合成 RunResult（subtype
/// `error_process_exit`、is_error=true、退出码记入 usage），正常 result 后
/// 退出不补发（design D6）。
/// 真实进程经 [`ClaudeCliRunner::start`] 装配的击杀缝驱动；测试以内存行流 +
/// 即成 future + 记录型击杀缝驱动，不 spawn 进程。
pub(crate) async fn pump_lines<R, F, K>(
    reader: BufReader<R>,
    exit_code: F,
    sender: mpsc::Sender<AgentEvent>,
    handle: &RunHandle,
    kill: K,
) where
    R: tokio::io::AsyncRead + Unpin,
    F: std::future::Future<Output = Option<i32>>,
    K: FnOnce(),
{
    let mut lines = reader.lines();
    let mut seq: u64 = 0;
    let mut saw_result = false;
    let stopped = loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let Some(kind) = jsonl::normalize_line(&line) else {
                        continue;
                    };
                    if matches!(kind, AgentEventKind::RunResult { .. }) {
                        saw_result = true;
                    }
                    if sender.send(AgentEvent::stamp(seq, kind)).await.is_err() {
                        return;
                    }
                    seq += 1;
                }
                _ => break false,
            },
            _ = handle.wait_requested() => break true,
        }
    };
    if stopped {
        // 尽力语义：击杀失败静默不重试不阻塞（能力 spec
        // `specs/desktop-agent-execution/spec.md` 已留痕已知限制，路径相对域根）
        kill();
        return;
    }
    let usage = exit_code
        .await
        .map(|code| serde_json::json!({ "exitCode": code }))
        .unwrap_or(serde_json::Value::Null);
    if !saw_result {
        let kind = AgentEventKind::RunResult {
            subtype: PROCESS_EXIT_SUBTYPE.to_owned(),
            is_error: true,
            num_turns: None,
            duration_ms: None,
            cost_usd: None,
            usage,
            session_id: None,
        };
        let _ = sender.send(AgentEvent::stamp(seq, kind)).await;
    }
}
