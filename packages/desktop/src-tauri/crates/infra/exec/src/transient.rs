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
//! - 判定为「整串包含」——表内签名均不含换行，故与「逐行包含」等价。

use std::time::Duration;

use crate::run::{RunOutcome, RunReport};

/// 瞬态签名封闭表（三族按注释分组；不建族枚举——无消费者读族名）。
///
/// 现场来源（企业级安全软件的 minifilter 驱动持有 `target/` 下构建产物）：
/// `rename` / `write` / `remove` 三动词失败，报错形态落三族之一。
const TRANSIENT_SIGNATURES: [&str; 13] = [
    // Windows 文件锁族（DELETE 共享语义冲突）
    "os error 32",
    "os error 5",
    "另一个程序正在使用此文件",
    "being used by another process",
    "failed to rename archive file",
    "failed to remove file",
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

/// 重试退避四段（第 k 次重试前等待 `RETRY_BACKOFF[k - 1]`）：1 / 2 / 4 / 8 s
/// 递增覆盖「杀软扫描完一个文件」的典型窗口，同时避免空转重试。
const RETRY_BACKOFF: [Duration; 4] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
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

    /// 瞬态重试：`max_attempts = 5`（首跑 + 4 次重试）、退避 `[1, 2, 4, 8] s`、
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

/// 瞬态判定核心（三合一判据 + 封闭表匹配）：执行体在 [`RunReport`] **成型之前**
/// 就要判定（重试循环里 report 尚不存在），故核心落在字节面、公开面
/// [`is_transient`] 只做委托。
///
/// 命中返回**表中原文**（`LNK1104` / `EBUSY` 保持大写形态）——尝试留痕与记录分隔
/// 头因此稳定可断言。
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
    TRANSIENT_SIGNATURES
        .iter()
        .copied()
        .find(|signature| text.contains(&signature.to_lowercase()))
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

#[cfg(test)]
#[path = "transient_test.rs"]
mod transient_test;
