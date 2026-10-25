//! 瞬态签名与有限重试策略（执行体诊断面的单点实现）。
//!
//! 纪律：
//! - **字典扩展只改本表**（[`TRANSIENT_SIGNATURES`]）——调用点 MUST NOT 各写一份
//!   特征串（否则信号面漂移、假阴性无从审计）；
//! - **刻意不收裸 `failed to write`**：该措辞过泛（真实写盘缺陷会被吞成「环境
//!   错误」而重试 5 次）；rustc 的 `.rmeta` 写失败已由同一行内的 `os error 32` /
//!   `另一个程序正在使用此文件` 覆盖；
//! - `TimedOut` / `Cancelled` **永不命中**：超时重试会把 1200 s 变数小时，取消
//!   必须立即生效；
//! - 判定为**逐行包含**（[`str::lines`] 后按表内签名逐行做包含匹配；表内签名
//!   均不含换行），且 rustc / clippy 诊断的**源码摘录行先被剔除**
//!   （[`is_source_excerpt_line`]：`287 | b"… os error 32"` 这类被引用的源码
//!   文本不是执行环境的证据）；
//! - **增量推进判据单点驻本模块**（[`compiling_count`] / [`progress_series`] /
//!   [`is_progress_stalled`]）：执行体与检查域各只调用，MUST NOT 重算计数口径
//!   （否则「为何早停」与「早停的自证材料」会对不上）；该判据只用于**提前放弃**，
//!   MUST NOT 用于延长预算。

use std::time::Duration;

use crate::output::is_source_excerpt_line;
use crate::run::{RunOutcome, RunReport};

/// 瞬态签名封闭表（三族按注释分组；不建族枚举——无消费者读族名）。
///
/// 现场来源（企业级安全软件的 minifilter 驱动持有 `target/` 下构建产物）：
/// `rename` / `write` / `remove` 三动词失败，报错形态落三族之一。
///
/// `remove` 形态取 `failed to remove`（**不含** `file`）：生产文本为
/// `failed to remove <path>`，带 `file` 的旧条目永不命中（功能上被同行
/// `os error 32` 兜住，但会误导维护者以为 remove 形态有独立覆盖）；收窄风险低
/// ——该短语只出现在删除失败语境。
const TRANSIENT_SIGNATURES: [&str; 13] = [
    // Windows 文件锁族（DELETE 共享语义冲突）
    "os error 32",
    "os error 5",
    "另一个程序正在使用此文件",
    "being used by another process",
    "failed to rename archive file",
    "failed to remove",
    "Access is denied",
    // MSVC 链接器族（.pdb / .lib 被占用或未关闭）
    "LNK1104",
    // POSIX / Node 族（文件句柄 / 可执行文件被占用）
    "EBUSY",
    "ETXTBSY",
    "Text file busy",
    "EPERM: operation not permitted",
    "resource busy or locked",
];

/// 重试上限（首跑 + 4 次重试）：5 次足以跨越安全软件的扫描窗口（实测碰撞多在
/// 前 3 次内自愈），又不把单步墙钟放大到不可接受的量级。
const RETRY_MAX_ATTEMPTS: u32 = 5;

/// 重试退避四段（第 k 次重试前等待 `RETRY_BACKOFF[k - 1]`）：5 / 10 / 20 / 40 s
/// 慢档——实测安全软件的静默窗口可达分钟级，秒级退避在「只剩 rename 收尾」
/// （待编译计数已归零、只剩最后一两个产物）的尝试上大概率撞同一个扫描窗口，而那
/// 正是最需要耐心的一档。成功路径零影响（未命中签名不重试），只有真需要重试时才
/// 多等：最坏 5 次共 75 s，仍远低于 [`RETRY_TOTAL_BUDGET`] 硬顶。
const RETRY_BACKOFF: [Duration; 4] = [
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(20),
    Duration::from_secs(40),
];

/// 重试面**总预算**（自首跑开始时刻起算，非「每次尝试一份」）：1500 s 只够一次
/// 慢挂（`static-check` / suite 的单次预算 1200 s）+ 少量余量；超预算即不再发起
/// 新尝试，把末次结果原样返回——墙钟上界 = 总预算 + 单次尝试预算（有界）。
const RETRY_TOTAL_BUDGET: Duration = Duration::from_secs(1500);

/// 重试策略（构造面经 [`RetryPolicy::none`] / [`RetryPolicy::transient`] 收口）。
///
/// 三参数为**代码常量**（与既有超时预算同性质），MUST NOT 进 config schema——
/// 观察期后再议用户可配。字段 `pub` 供执行体兄弟模块读取与测试断言。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// 尝试次数上限（≥ 1；`1` = 单次执行）
    pub max_attempts: u32,
    /// 逐次退避（索引 `attempts - 1` 取档，越界退化到末档）
    pub backoff: [Duration; 4],
    /// 总预算（自首跑起算；每次重试尝试开始前判定）
    pub total_budget: Duration,
}

impl RetryPolicy {
    /// 单次执行（缺省语义）：`max_attempts = 1`，退避与预算不消费。取值即本变更
    /// 前的行为（`attempts == 1 && retried` 空）。
    pub fn none() -> RetryPolicy {
        RetryPolicy {
            max_attempts: 1,
            backoff: RETRY_BACKOFF,
            total_budget: RETRY_TOTAL_BUDGET,
        }
    }

    /// 瞬态重试：`max_attempts = 5`（首跑 + 4 次重试）、退避 `[5, 10, 20, 40] s`、
    /// 总预算 1500 s。
    pub fn transient() -> RetryPolicy {
        RetryPolicy {
            max_attempts: RETRY_MAX_ATTEMPTS,
            backoff: RETRY_BACKOFF,
            total_budget: RETRY_TOTAL_BUDGET,
        }
    }
}

/// 此前各次尝试的留痕（**末次不入**——末次结果仍在 [`RunReport`] 四个既有字段
/// 上；本类型只承载「重试过什么」的审计面）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptTrace {
    /// 该次尝试的退出码（`TimedOut` / `Cancelled` 不入 `retried`，故不为 -1 哨兵）
    pub exit_code: i32,
    /// 命中的签名原文（未命中 `None`——单独留痕便于审计「为何重试」）
    pub transient: Option<String>,
    /// 该次尝试的 stdout 原始字节
    pub stdout: Vec<u8>,
    /// 该次尝试的 stderr 原始字节
    pub stderr: Vec<u8>,
}

/// 瞬态判定核心（三合一判据 + 封闭表匹配）
pub(crate) fn transient_signature(
    outcome: RunOutcome,
    exit_code: i32,
    stdout: &[u8],
    stderr: &[u8],
) -> Option<&'static str> {
    if outcome != RunOutcome::Completed || exit_code == 0 {
        return None;
    }
    let mut merged = stdout.to_vec();
    merged.extend_from_slice(stderr);
    let text = String::from_utf8_lossy(&merged).to_lowercase();
    TRANSIENT_SIGNATURES.iter().copied().find(|signature| {
        let needle = signature.to_lowercase();
        text.lines()
            .filter(|line| !is_source_excerpt_line(line))
            .any(|line| line.contains(&needle))
    })
}

/// 瞬态判定（对外单一签名）：`Completed` + 非零退出 + 合并输出（stdout ++ stderr）
/// 命中签名表任一原文，三者同时满足才命中；`TimedOut` / `Cancelled` 恒 `None`。
pub fn is_transient(report: &RunReport) -> Option<&'static str> {
    transient_signature(
        report.outcome,
        report.exit_code,
        &report.stdout,
        &report.stderr,
    )
}

/// 待编译计数（「什么是一个待编译单位」这条易漂移口径的**单点**）：合并输出
/// （`stdout ++ stderr`，UTF-8 lossy）按 [`str::lines`] 逐行，去前导空白后以
/// `Compiling ` 为前缀的行数（rustc / cargo 原文形态，**大小写敏感**、含尾随
/// 空格）；无该字样 → `0`。
///
/// `lines()` 天然吃掉 CRLF 的 `\r`（与既有判定口径一致）。无该字样的输出
/// （Node / 链接器 / 真红）退化为固定次数语义：每次尝试计数恒 `0`，卡死判据因此
/// 恒判否，重试照旧走满上限 / 预算（既有行为零变）。
///
/// 零 IO / 零 spawn / 零 panic。检查域 MUST NOT 重算本口径——装配文案只经
/// [`progress_series`] 取数。
pub fn compiling_count(stdout: &[u8], stderr: &[u8]) -> u32 {
    let mut merged = stdout.to_vec();
    merged.extend_from_slice(stderr);
    String::from_utf8_lossy(&merged)
        .lines()
        .filter(|line| line.trim_start().starts_with("Compiling "))
        .count() as u32
}

/// 按尝试序的待编译计数序列：`retried` 逐条在前、末次尝试在后。
///
/// 长度 = `retried.len() + 1`；生产不变量（`run.rs` 的入栈动作在退避成功之后）
/// 下 `retried.len() + 1 == attempts`，故长度等于 `attempts`；未重试即长度 1。
/// 与执行体重试循环内的计数序列**同源同值**（两处共用 [`compiling_count`]，字节
/// 零改写零截断是等值的前提）——故 [`RunReport`] 无需新增字段。
pub fn progress_series(report: &RunReport) -> Vec<u32> {
    let mut series: Vec<u32> = report
        .retried
        .iter()
        .map(|trace| compiling_count(&trace.stdout, &trace.stderr))
        .collect();
    series.push(compiling_count(&report.stdout, &report.stderr));
    series
}

/// 卡死判据：末两次计数**非降**且末值 `> 0`（`series.len() < 2` 恒 `false`）。
///
/// 「非降」而非「相等」——「新增 crate 数抵消已完成 crate 数」的上升形态同属写不
/// 进去；末值 `0`（只剩 rename / 清理收尾）恒判否，那恰恰是最该再试的一档。
///
/// 判据只用于**提前放弃**，MUST NOT 用于延长预算：`attempts == 1` 无前值不触发，
/// `TimedOut` / `Cancelled` / 真红（无签名）三条既有零重试面与判据无关。
pub fn is_progress_stalled(series: &[u32]) -> bool {
    let Some((last, previous)) = series.split_last() else {
        return false;
    };
    let Some(previous) = previous.last() else {
        return false;
    };
    *last > 0 && *last >= *previous
}

#[cfg(test)]
#[path = "transient_test.rs"]
mod transient_test;
