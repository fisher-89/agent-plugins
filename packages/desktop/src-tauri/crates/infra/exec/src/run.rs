use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use foundation::cancel::CancelFlag;
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio::time::{sleep, timeout, Instant};

use crate::transient::{
    compiling_count, is_progress_stalled, transient_signature, AttemptTrace, RetryPolicy,
};
use crate::{kill_tree, JobTree, LiveSink, SystemCommand};

/// 取消轮询间隔：执行体观测取消信号的最小粒度（`foundation::cancel` 零 tokio，
/// 无常量可订阅的通知面；200 ms 在「停止秒级收敛」验收面下足够，且不构成空转）。
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// 排水 chunk 尺寸（单次 `read` 通常是「一批就绪字节」——与管道环缓冲同量级；
/// 4 KiB 页对齐之上取 8 KiB，减少调用次数而不放大单次拷贝）。
const DRAIN_CHUNK_BYTES: usize = 8192;

/// 收尾上界（收尸与排水各自有界）：杀树命令本身是尽力语义（`kill -9` 不杀
/// rustc 类子孙），若被杀的壳进程留下持有管道写端的孙进程，无界等待会让
/// 「超时悬挂」换形为「杀后悬挂」。同一上界也用于**自然退出臂收尸后的排水**：
/// 子进程已退出、孙进程仍持写端时（Windows 上 cargo-mutants 超时只杀
/// `cargo test`、遗留测试可执行文件即典型），无界等 EOF 会把「已结束」换形为
/// 「悬挂」。2 s 覆盖正常收尸与排空时延，异常路径下截断即可（输出不完整面在
/// TimedOut / Cancelled 两臂无消费者；Completed 臂仅在孤儿持写端的异常面下
/// 可能截断，见 [`RunReport`] doc）。
const FINISH_GRACE: Duration = Duration::from_secs(2);

/// 执行入参：工作目录 / 环境变量 / **必填**超时 / 取消信号 / 重试策略 / 实时观察者。
///
/// 刻意不实现 `Default`——`timeout` 无缺省语义（超时必填是本执行体的守门约束）。
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// 子进程工作目录（`None` = 继承当前进程 cwd）
    pub cwd: Option<PathBuf>,
    /// 追加 / 覆盖的环境变量（`(key, value)`；空 vec = 全继承）
    pub env: Vec<(OsString, OsString)>,
    /// 墙钟预算（必填；到点 → [`RunOutcome::TimedOut`] 并杀整树）
    pub timeout: Duration,
    /// 取消信号读端（[`CancelFlag::never`] = 永不取消）
    pub cancel: CancelFlag,
    /// 重试策略（缺省语义 = 单次执行 [`RetryPolicy::none`]；代码常量，MUST NOT 进
    /// config schema）
    pub retry: RetryPolicy,
    /// 实时输出观察者（`None` = 纯内存，既有语义逐字节零变）。同一实例交给每次
    /// 尝试（多尝试面下 sink 收到全部尝试字节按序；权威内容仍由调用方收口时一次性
    /// 写入）。
    pub live: Option<LiveSink>,
}

/// 执行结局封闭三值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// 进程自然退出（退出码可读或不可读，见 [`RunReport::exit_code`]）
    Completed,
    /// 预算到期强杀
    TimedOut,
    /// 取消置位强杀（含入口即已置位的零 spawn 短路）
    Cancelled,
}

/// 单次执行的内存产出（不持久化；输出零改写零截断、不预合流）。
///
/// `TimedOut` / `Cancelled` 两臂的输出**不保证完整**（被杀进程可能仍有未排出的
/// 字节；排水以 [`FINISH_GRACE`] 有界截断），四消费点在该两臂均不消费输出
///（static-check / testexec 以归因面收口，两 mutation 段走固定文案）。
/// `Completed` 臂正常树输出完整；仅当子进程已退出、孙进程仍持有继承的管道写端
///（孤儿）时，收尸后的排水同样以 [`FINISH_GRACE`] 截断——该异常面下输出可能
/// 少掉尾字节，换取执行体不悬挂。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// 结局
    pub outcome: RunOutcome,
    /// 退出码（`TimedOut` / `Cancelled` 与「无可读退出码」同记 `-1` 哨兵——
    /// MUST NOT 回落 `0` 吞成假绿）
    pub exit_code: i32,
    /// **末次**尝试的 stdout 原始字节（重试场景下此前尝试的字节只在 [`Self::retried`]
    /// 中可读——既有消费点的读法零变）
    pub stdout: Vec<u8>,
    /// **末次**尝试的 stderr 原始字节
    pub stderr: Vec<u8>,
    /// 本命令实际尝试次数（≥ 1）
    pub attempts: u32,
    /// 此前各次尝试的留痕（**末次不入**；未命中瞬态签名 / 单次执行时恒空）
    pub retried: Vec<AttemptTrace>,
}

/// 执行失败面（进程没跑起来 / 收尸失败——与三值结局正交：三值表达「跑起来之后
/// 怎么结束」，本类型表达「根本没建立或无法收敛」）。
///
/// 分类只为让调用点各自拼回**逐字同现状**的失败文案：本类型只提供类别词（「拉起
/// 失败」/「等待失败」）+ `io::Error` 原文，调用点的前缀 / 指引括号由调用点保留。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// 进程未能建立（`spawn` 失败：程序不可达 / 权限等）
    Spawn(String),
    /// 收尸失败（`child.wait()` io 错误）
    Wait(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => write!(formatter, "拉起失败: {error}"),
            Self::Wait(error) => write!(formatter, "等待失败: {error}"),
        }
    }
}

/// 执行体：入口短路 → 重试循环（每次尝试 = [`attempt_once`]）→ 尝试面装配。
///
/// 循环判定序（每次尝试之后）：①非 `Completed`（`TimedOut` / `Cancelled`）即返
/// （零重试）→ ②未命中瞬态签名即返（真红 / 绿）→ ③卡死判否（[`is_progress_stalled`]）
/// 即返末次 → ④达尝试上限即返末次 → ⑤总预算耗尽即返末次 → ⑥退避（同样观测
/// `cancel`）后进入下一轮。末次尝试的字节留在 [`RunReport::stdout`] /
/// [`RunReport::stderr`]，此前各次进 [`RunReport::retried`]——「转入 `retried`」落在
/// 退避成功之后，故 `retried` 恒等于「末次之前」的尝试集。
///
/// 卡死只**提前放弃**，MUST NOT 延长预算（`total_budget` 仍是硬顶）；`attempts == 1`
/// 无前值不触发；末次计数 `0`（只剩收尾）不触发。
pub async fn run_shell(command: SystemCommand, options: RunOptions) -> Result<RunReport, RunError> {
    let RunOptions {
        cwd,
        env,
        timeout: budget,
        cancel,
        retry,
        live,
    } = options;
    // 总预算自首跑开始起算（非每次尝试一份）
    let started = Instant::now();
    let mut attempts: u32 = 0;
    let mut retried: Vec<AttemptTrace> = Vec::new();
    // 按尝试序的待编译计数序列：只用于卡死早停判定，**不进** `RunReport`（读侧
    // 由 `transient::progress_series` 反推，两处共用 `compiling_count` 故值必然一致）
    let mut counts: Vec<u32> = Vec::new();
    loop {
        attempts += 1;
        let attempt = attempt_once(
            &command,
            cwd.as_deref(),
            &env,
            budget,
            &cancel,
            live.as_ref(),
        )
        .await?;
        // 每次尝试立即记账（先于三臂判定：被杀两臂的输出不消费计数，但先记账使
        // 「序列长度 == 尝试次数」在正常路径上恒成立）
        counts.push(compiling_count(&attempt.stdout, &attempt.stderr));
        // ① 非自然退出（超时 / 取消）零重试：超时重试会把预算放大数倍、取消必须
        // 立即生效
        if attempt.outcome != RunOutcome::Completed {
            return Ok(attempt.into_report(attempts, retried));
        }
        // ② 未命中签名（真红 / 绿）零重试——重试策略在位但不触发
        let Some(signature) = transient_signature(
            attempt.outcome,
            attempt.exit_code,
            &attempt.stdout,
            &attempt.stderr,
        ) else {
            return Ok(attempt.into_report(attempts, retried));
        };
        // ③ 卡死早停：末两次计数非降且末值 > 0 = 同一批写不进去 → 提前放弃，不烧满
        // `max_attempts`（末次结果原样返回，末次不入 `retried`）
        if is_progress_stalled(&counts) {
            return Ok(attempt.into_report(attempts, retried));
        }
        // ④ 上限 / ⑤ 预算早停：末次结果原样返回（`Completed` + 非零，由调用方按
        // 瞬态命中断言 fault），末次不入 `retried`
        if attempts >= retry.max_attempts || started.elapsed() >= retry.total_budget {
            return Ok(attempt.into_report(attempts, retried));
        }
        // ⑥ 退避等待同样观测 cancel（复用既有轮询单点，不新增计时原语）
        let backoff = retry.backoff[(attempts as usize - 1).min(retry.backoff.len() - 1)];
        let cancelled = tokio::select! {
            biased;
            _ = wait_cancel(&cancel) => true,
            _ = sleep(backoff) => false,
        };
        if cancelled {
            // 退避期取消：本次尝试已完成（按「末次」字节返回），`attempts` 记已
            // 完成次数，该次不进 `retried`
            return Ok(RunReport {
                outcome: RunOutcome::Cancelled,
                exit_code: -1,
                stdout: attempt.stdout,
                stderr: attempt.stderr,
                attempts,
                retried,
            });
        }
        retried.push(AttemptTrace {
            exit_code: attempt.exit_code,
            transient: Some(signature.to_owned()),
            stdout: attempt.stdout,
            stderr: attempt.stderr,
        });
    }
}

/// 单次尝试产出（重试循环的内部载体；`into_report` 把末次尝试与既有尝试集装配
/// 成 [`RunReport`]）。
struct Attempt {
    outcome: RunOutcome,
    exit_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Attempt {
    /// 末次尝试 + 此前各次尝试 → 对外的 [`RunReport`]。
    fn into_report(self, attempts: u32, retried: Vec<AttemptTrace>) -> RunReport {
        RunReport {
            outcome: self.outcome,
            exit_code: self.exit_code,
            stdout: self.stdout,
            stderr: self.stderr,
            attempts,
            retried,
        }
    }
}

/// 单次尝试：spawn（`kill_on_drop`）→ 双管道并发排水（每块 tee 给 `live`）→ 三臂
/// 竞速（取消轮询 / 超时 / 「排水 + 收尸」）→ 被杀两臂杀整树 + 有界收尾。
///
/// 三臂 `biased` 序 cancel → timeout → 退出：取消优先于超时，两者都优先于自然
/// 退出（同一轮 poll 同时就绪时按此序收敛，不出现「已到期却记 Completed」）。
/// 重试循环复用本函数（同一骨架，不另写第二条 spawn 路径）。
async fn attempt_once(
    command: &SystemCommand,
    cwd: Option<&Path>,
    env: &[(OsString, OsString)],
    budget: Duration,
    cancel: &CancelFlag,
    live: Option<&LiveSink>,
) -> Result<Attempt, RunError> {
    // 入口短路：已置位即不 spawn（取消用例不必依赖轮询时序；也避免为一次必然被
    // 杀的调用付出进程建立成本）
    if cancel.is_set() {
        return Ok(Attempt {
            outcome: RunOutcome::Cancelled,
            exit_code: -1,
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }

    let mut shell = command.clone().into_tokio();
    shell
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // future 被 drop（调用方取消等待 / panic）时兜底终止进程
        .kill_on_drop(true);
    if let Some(dir) = cwd {
        shell.current_dir(dir);
    }
    for (key, value) in env {
        shell.env(key, value);
    }
    let mut child = shell
        .spawn()
        .map_err(|error| RunError::Spawn(error.to_string()))?;
    // 作业成员资格必须紧贴 spawn：assign 之前创建的后代不进作业（见 [`crate::job`]）
    let mut job = JobTree::attach(&child);

    // 两缓冲区 / 两条管道 / child 均落本函数局部作用域：竞速 future 只持可变借用
    //（不 move、不 tokio::spawn）——被杀两臂仍可继续收尸与排水，且本 crate 无需
    // tokio 的 `rt` 特性
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let mut stdout_bytes: Vec<u8> = Vec::new();
    let mut stderr_bytes: Vec<u8> = Vec::new();

    let raced = tokio::select! {
        biased;
        _ = wait_cancel(cancel) => Race::Cancelled,
        _ = sleep(budget) => Race::TimedOut,
        status = await_exit(
            &mut child,
            &mut stdout_pipe,
            &mut stderr_pipe,
            &mut stdout_bytes,
            &mut stderr_bytes,
            live,
        ) => Race::Exited(status),
    };

    let outcome = match raced {
        Race::Exited(status) => {
            let status = status.map_err(|error| RunError::Wait(error.to_string()))?;
            // 自然退出臂解除武装：孤儿子孙按既有语义存续（排水已在 await_exit 内
            // 以 FINISH_GRACE 有界收口），Drop 不再整组终止
            if let Some(job) = job.as_mut() {
                job.disarm();
            }
            return Ok(Attempt {
                outcome: RunOutcome::Completed,
                // 无可读退出码（unix 信号终止）同记 -1 哨兵，不回落 0
                exit_code: status.code().unwrap_or(-1),
                stdout: stdout_bytes,
                stderr: stderr_bytes,
            });
        }
        Race::Cancelled => RunOutcome::Cancelled,
        Race::TimedOut => RunOutcome::TimedOut,
    };

    // 被杀两臂：整树击杀 → 有界收尸 → 有界排水（顺序不可倒：先收尸关管道，排水
    // 才以 EOF 收口）。排水仍以 FINISH_GRACE 截断——**tee 不引入新的无界等待**：
    // sink 为同步写文件（微秒级），且整段排水仍在外层 `timeout(FINISH_GRACE, ..)`
    // 之内。
    //
    // 作业对象在位时先整组终止：`taskkill /T` 按父子快照遍历，msys fork 副本可能
    // 换过父 pid 而被漏杀（漏杀者持管道写端 ⇒ 排水永不 EOF ⇒ tokio 阻塞线程池被
    // 钉死到该进程退出）。两条击杀都保留——作业挂载失败（系统调用失败）时回落既
    // 有语义，成功时 `taskkill` 只是对已死管线的幂等补刀。
    //
    // 走到这里之外还有第三条收口路径：本 future 被 drop（调用方 abort / panic
    // 展开）时上述代码一行都执行不到，故 `JobTree::Drop` 在未解除武装时整组终止
    // ——`kill_on_drop` 只杀直接子进程，孙进程持管道写端会让 runtime 析构无界等待。
    if let Some(job) = job.as_mut() {
        job.terminate();
    }
    kill_tree(child.id());
    let _ = timeout(FINISH_GRACE, child.wait()).await;
    let _ = timeout(FINISH_GRACE, async {
        let _ = tokio::join!(
            drain(&mut stdout_pipe, &mut stdout_bytes, live),
            drain(&mut stderr_pipe, &mut stderr_bytes, live),
        );
    })
    .await;
    Ok(Attempt {
        outcome,
        exit_code: -1,
        stdout: stdout_bytes,
        stderr: stderr_bytes,
    })
}

/// 竞速三结果（`select!` 臂产出内部载体）。
enum Race {
    Exited(std::io::Result<std::process::ExitStatus>),
    TimedOut,
    Cancelled,
}

/// 自然退出臂：**双管道并发排水与收尸并行**——排水必须先于收尸起步（否则子进程
/// 写满管道会阻塞在 `write` 上永不退出），但排水本身不能被无限拖住：子进程退出
/// 不等于管道 EOF，孙进程可能继承写端（Windows 上 cargo-mutants 超时只杀
/// `cargo test`、遗留测试可执行文件即典型）。故收尸完成后排水最多再等
/// [`FINISH_GRACE`]，超时以已读字节收口，不悬挂。
///
/// 两路管道先 EOF（子进程已关闭写端）时继续等收尸；外层 `select!` 的超时 /
/// 取消两臂仍守护这一等待。
async fn await_exit(
    child: &mut Child,
    stdout_pipe: &mut Option<ChildStdout>,
    stderr_pipe: &mut Option<ChildStderr>,
    stdout_bytes: &mut Vec<u8>,
    stderr_bytes: &mut Vec<u8>,
    live: Option<&LiveSink>,
) -> std::io::Result<std::process::ExitStatus> {
    let drain_both = async {
        let _ = tokio::join!(
            drain(stdout_pipe, stdout_bytes, live),
            drain(stderr_pipe, stderr_bytes, live),
        );
    };
    tokio::pin!(drain_both);
    let mut drained = false;
    let status = tokio::select! {
        status = child.wait() => status?,
        () = &mut drain_both => {
            drained = true;
            child.wait().await?
        }
    };
    // 收尸已完成：孤儿孙进程可能仍持写端，有界收口不悬挂（排水已完成则无需再等）
    if !drained {
        let _ = timeout(FINISH_GRACE, &mut drain_both).await;
    }
    Ok(status)
}

/// 管道 chunk 循环读尽至 EOF（错误忽略——沿退役的私有 `read_all` 语义：读失败不
/// 改变「已读到的字节即输出」的结论，如实收口而非上抛）。
///
/// 每块先追加进内存缓冲（既有消费面），再在 `live` 在位时 tee 给实时观察者（后
/// 者只写文件，不影响本函数的控制流）。
async fn drain<R: tokio::io::AsyncRead + Unpin>(
    pipe: &mut Option<R>,
    buffer: &mut Vec<u8>,
    live: Option<&LiveSink>,
) {
    use tokio::io::AsyncReadExt as _;
    let Some(pipe) = pipe.as_mut() else {
        return;
    };
    let mut chunk = [0u8; DRAIN_CHUNK_BYTES];
    loop {
        match pipe.read(&mut chunk).await {
            Ok(0) => break,
            Ok(read) => {
                let bytes = &chunk[..read];
                buffer.extend_from_slice(bytes);
                if let Some(live) = live {
                    live.absorb(bytes);
                }
            }
            Err(_) => break,
        }
    }
}

/// 取消信号等待半边：定间隔轮询（零 tokio 通知面的唯一形态）。
async fn wait_cancel(cancel: &CancelFlag) {
    while !cancel.is_set() {
        sleep(CANCEL_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
