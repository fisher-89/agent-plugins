use std::path::{Path, PathBuf};

use agent::{
    AgentEventKind, AgentRunner, AgentSession, AgentStartError, RunHandle, SessionOpen,
    TurnQuestion,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStdout, Command};
use tokio::sync::mpsc;

use crate::{discover, flags, jsonl};

/// 事件通道有界容量（背压策略：泵任务阻塞在 `send`，事件不丢、内存有界）。
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// 提问通道有界容量（单问送达；单会话单活动轮，容量 4 为余量）。
const QUESTION_CHANNEL_CAPACITY: usize = 4;

/// EOF 无 TurnDone 事件时合成收敛事件的 subtype（design D6：收敛恒由
/// TurnDone 驱动，状态机不需要 EOF 特判）。
const PROCESS_EXIT_SUBTYPE: &str = "error_process_exit";

/// 无状态 runner 构造。
pub struct ClaudeCliRunner;

impl ClaudeCliRunner {
    /// 无状态 runner：全部运行态在 `open_session` 产出的会话泵任务内。
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
    /// open 段无 IO（CLI 发现延后至 ask——发现是文件系统扫描；injections 为
    /// 会话级注入，CLI 租户 MVP 不消费，整合方式引擎自选：忽略）。
    fn open_session(&self, open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        let (question_tx, question_rx) = mpsc::channel::<TurnQuestion>(QUESTION_CHANNEL_CAPACITY);
        let (observation_tx, observation_rx) =
            mpsc::channel::<agent::AgentEventKind>(EVENT_CHANNEL_CAPACITY);
        let handle = RunHandle::default();
        tokio::spawn(session_pump(
            open,
            question_rx,
            observation_tx,
            handle.clone(),
        ));
        Ok(AgentSession {
            observations: observation_rx,
            questions: question_tx,
            handle,
        })
    }
}

/// 会话泵任务（ask 段）
async fn session_pump(
    open: SessionOpen,
    mut questions: mpsc::Receiver<TurnQuestion>,
    observations: mpsc::Sender<agent::AgentEventKind>,
    handle: RunHandle,
) {
    let Some(question) = questions.recv().await else {
        return; // 组合根半边先关：无问即无泵生命周期
    };
    match spawn_turn(&open, &question, &handle) {
        Ok(process) => {
            // 一轮一命：run 返回即泵生命周期终点（EOF 收敛与停止路径在
            // `pump_lines` 内均已单轮终止），stopped 返回值不再消费
            process.run(observations.clone()).await;
        }
        Err(error) => {
            let _ = observations
                .send(AgentEventKind::TurnDone {
                    subtype: spawn_failure_subtype(&error),
                    is_error: true,
                    num_turns: None,
                    duration_ms: None,
                    cost_usd: None,
                    usage: serde_json::Value::Null,
                    session_id: None,
                })
                .await;
        }
    }
}

/// spawn 失败合成收敛事件的 subtype（CLI 合成收敛事件的命名口径）。
fn spawn_failure_subtype(error: &AgentStartError) -> String {
    match error {
        AgentStartError::CliMissing(_) => "error_cli_missing".to_owned(),
        _ => "error_spawn_failed".to_owned(),
    }
}

/// 单轮进程租户：spawn 产物（stdout 行流 + 退出码 oneshot + 击杀缝 + 停止
/// 信号句柄）。每轮进程即逻辑 actor 的一次 ask 兑现。
struct TurnProcess {
    stdout: ChildStdout,
    exit: tokio::sync::oneshot::Receiver<Option<i32>>,
    kill: Box<dyn FnOnce() + Send>,
    handle: RunHandle,
}

impl TurnProcess {
    /// 逐行泵至轮终态：EOF（正常/异常）或停止路径。返回是否停止路径终止。
    async fn run(self, observations: mpsc::Sender<agent::AgentEventKind>) -> bool {
        let Self {
            stdout,
            exit,
            kill,
            handle,
        } = self;
        pump_lines(
            BufReader::new(stdout),
            async { exit.await.unwrap_or(None) },
            observations,
            &handle,
            kill,
        )
        .await
    }
}

/// spawn 单轮进程：CLI 发现 → flag 组装（`--resume` 走先行句柄）→ 命令组装
/// → spawn。启动失败 `Err`（CliMissing / SpawnFailed）。
fn spawn_turn(
    open: &SessionOpen,
    question: &TurnQuestion,
    handle: &RunHandle,
) -> Result<TurnProcess, AgentStartError> {
    let program = discover::discover()?;
    let args = flags::build_args(&flags::TurnParams {
        prompt: question.prompt.clone(),
        permission_mode: open.ctx.permission_mode,
        resume_handle: open.prior_handle.clone(),
    });
    let mut command = build_command(&program, &args, &open.ctx.workspace_root);
    let mut child = command.spawn().map_err(|e| {
        AgentStartError::SpawnFailed(format!("{} 启动失败: {e}", program.display()))
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AgentStartError::SpawnFailed("stdout 未按管道打开".to_owned()))?;
    let stderr = child.stderr.take();
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
    Ok(TurnProcess {
        stdout,
        exit: exit_rx,
        kill: Box::new(move || kill_process_tree(pid)),
        handle: handle.clone(),
    })
}

/// 组装 spawn 命令：Windows `.cmd` / `.bat` shim 不可直接 spawn，经
/// `cmd /C` 包装（args 逐参传递）；cwd 经 `current_dir` 传递（非 flag）；
/// stdout 按管道打开，stderr 按管道打开供排水，stdin 关闭（无头无输入）。
fn build_command(program: &Path, args: &[String], cwd: &PathBuf) -> Command {
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

/// 进程树击杀（尽力语义，静默不重试不阻塞收敛）
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

/// 逐行泵核心
pub(crate) async fn pump_lines<R, F, K>(
    reader: BufReader<R>,
    exit_code: F,
    sender: mpsc::Sender<agent::AgentEventKind>,
    handle: &RunHandle,
    kill: K,
) -> bool
where
    R: tokio::io::AsyncRead + Unpin,
    F: std::future::Future<Output = Option<i32>>,
    K: FnOnce(),
{
    let mut lines = reader.lines();
    let mut saw_result = false;
    let stopped = loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let Some(kind) = jsonl::normalize_line(&line) else {
                        continue;
                    };
                    if matches!(kind, AgentEventKind::TurnDone { .. }) {
                        saw_result = true;
                    }
                    if sender.send(kind).await.is_err() {
                        return false;
                    }
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
        return true;
    }
    let usage = exit_code
        .await
        .map(|code| serde_json::json!({ "exitCode": code }))
        .unwrap_or(serde_json::Value::Null);
    if !saw_result {
        let kind = AgentEventKind::TurnDone {
            subtype: PROCESS_EXIT_SUBTYPE.to_owned(),
            is_error: true,
            num_turns: None,
            duration_ms: None,
            cost_usd: None,
            usage,
            session_id: None,
        };
        let _ = sender.send(kind).await;
    }
    false
}
