//! `ClaudeCliRunner`：CLI 租户实现——CLI 发现 → spawn → stdout 逐行泵 →
//! 逐行归一化发入有界 mpsc → EOF 无 result 补发合成收敛事件。
//!
//! 进程细节（`cmd /C` shim 包装、stderr 排水、stdin 关闭）全部封在本模块，
//! trait 面上只暴露逻辑事件流与句柄。

use std::path::Path;

use agent::{
    AgentEvent, AgentEventKind, AgentRun, AgentRunParams, AgentRunner, AgentStartError, RunHandle,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
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
        tokio::spawn(pump(stdout, child, sender));
        Ok(AgentRun {
            events: receiver,
            handle: RunHandle,
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

/// stdout 逐行泵：委托 [`pump_lines`]——行流为子进程 stdout，EOF 后退出码
/// 取自 `child.wait()`。
async fn pump(stdout: ChildStdout, mut child: Child, sender: mpsc::Sender<AgentEvent>) {
    let exit_code = async move { child.wait().await.ok().and_then(|status| status.code()) };
    pump_lines(BufReader::new(stdout), exit_code, sender).await;
}

/// 逐行泵核心（可注入缝，决策 D6/D8）：任意 AsyncRead 行流 + EOF 后退出码
/// future → 逐行归一化 → `AgentEvent::stamp` 盖 seq/时间戳 → 发入有界通道
/// （seq 每 run 从 0 单调递增；空白行跳过不占 seq）。消费端关闭时泵随之
/// 停止。EOF 后取退出码：未见 result 事件则补发合成
/// [`AgentEventKind::RunResult`]（subtype `error_process_exit`、is_error=true、
/// 退出码记入 usage），正常 result 后退出不补发（design D6）。
/// 真实进程经 [`pump`] 驱动；测试以内存行流 + 即成 future 驱动，不 spawn 进程。
pub(crate) async fn pump_lines<R, F>(
    reader: BufReader<R>,
    exit_code: F,
    sender: mpsc::Sender<AgentEvent>,
) where
    R: tokio::io::AsyncRead + Unpin,
    F: std::future::Future<Output = Option<i32>>,
{
    let mut lines = reader.lines();
    let mut seq: u64 = 0;
    let mut saw_result = false;
    while let Ok(Some(line)) = lines.next_line().await {
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
