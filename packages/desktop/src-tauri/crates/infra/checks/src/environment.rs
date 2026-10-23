//! 检查域环境归因装配单点（static-check 与 test-execution 共用；MUST NOT 两写）。
//!
//! 两个职责：
//! - [`attempts_log`]：输出记录内容形态——单次尝试**逐字节同现状**（`stdout ++
//!   stderr`，零分隔头）；发生过重试则各次尝试按序拼接、每段前加分隔头（审计不丢
//!   上下文）；
//! - [`fault_text`]：`RunFault::Environment` 的人读文本——占用者点名 / 被锁文件
//!   （原料来自 [`exec::attribution_text`]）+ 「已自动重试 N 次」+ 末次诊断摘要 +
//!   环境侧处理指引。
//!
//! exec 层只供事实原料与尝试面字节（MUST NOT 自带业务文案：尝试分隔头 / 归因文案
//! 归调用方），故重试次数与处置指引的措辞单点驻本模块；本模块为**纯函数**（零 io
//! / 零 async）。

use exec::{ProcessOutput, RetryPolicy, RunReport};

/// 环境侧处理指引（用户 / IT 侧动作）：排除 **workspace 父目录**（逐 worktree
/// 排除无效——worktree 动态新建），并建议一并排除包管理器缓存。与基线红文案
/// （「请在基线上修复后重新发起」）零共用句，故两态可辨。
const ENV_GUIDANCE: &str = "环境侧处理：请 IT 在【火绒企业版（服务端策略）】与【云枢】中排除 \
     workspace 父目录（逐 worktree 排除无效——worktree 动态新建），建议一并排除 `.cargo` 与 \
     pnpm 缓存目录后重新发起。";

/// 末次诊断摘要的字符上限（诊断全量已在输出记录 / 报告目录，本处只需可读主因）。
const DIAGNOSIS_EXCERPT_LIMIT: usize = 2000;

/// 输出记录内容装配（`exec::ExecOutput::Recorded` 的单一 `.log` 正文）。
///
/// - `attempts == 1 && retried.is_empty()`（未重试）→ **原样** `stdout ++ stderr`
///   ——逐字节同单次执行（既有 record 断言不被破）；
/// - 发生过重试 → `retried` 按序在前、末次在后，每段一段分隔头
///   `===== attempt {n}/{max} · exit={exit_code} · transient={命中签名或 -} =====`
///   + 该次的 `stdout ++ stderr`。
pub(crate) fn attempts_log(report: &RunReport) -> Vec<u8> {
    if report.attempts <= 1 && report.retried.is_empty() {
        let mut plain = report.stdout.clone();
        plain.extend_from_slice(&report.stderr);
        return plain;
    }
    let max = RetryPolicy::transient().max_attempts;
    let mut log: Vec<u8> = Vec::new();
    for (index, trace) in report.retried.iter().enumerate() {
        push_attempt(
            &mut log,
            index as u32 + 1,
            max,
            trace.exit_code,
            trace.transient.as_deref(),
            &trace.stdout,
            &trace.stderr,
        );
    }
    push_attempt(
        &mut log,
        report.attempts,
        max,
        report.exit_code,
        exec::is_transient(report),
        &report.stdout,
        &report.stderr,
    );
    log
}

/// 单段（分隔头 + 该次合并流）追加：段内顺序为 stdout 前缀 + stderr 后缀（不
/// `2>&1`、不交错重排）。
fn push_attempt(
    log: &mut Vec<u8>,
    attempt: u32,
    max: u32,
    exit_code: i32,
    transient: Option<&str>,
    stdout: &[u8],
    stderr: &[u8],
) {
    let signature = transient.unwrap_or("-");
    log.extend_from_slice(
        format!("===== attempt {attempt}/{max} · exit={exit_code} · transient={signature} =====\n")
            .as_bytes(),
    );
    log.extend_from_slice(stdout);
    log.extend_from_slice(stderr);
}

/// 环境错误人读文本（`RunFault::Environment` 的唯一装配点）：归因原料（占用者 /
/// 被锁文件）+ 重试事实 + 末次诊断摘要 + 处置指引。
///
/// `attempts` / `exit_code` / `output` 取**末次**尝试（`static-check` 传
/// `&run.stdout`——命令串带 `2>&1`，`stderr` 恒空；test-execution 传合并流）。
/// 诊断摘要复用 [`ProcessOutput`] 的既有截断口径（不自造「取末尾 N 字符」）。
pub(crate) fn fault_text(attempts: u32, exit_code: i32, output: &[u8]) -> String {
    let diagnostics = String::from_utf8_lossy(output);
    let attribution = exec::attribution_text(&diagnostics).unwrap_or_else(|| {
        // 解析不出路径（非文件锁形态的瞬态签名，如链接器 / Node 错误）→ 只给降级
        // 说明，不拼空段
        "被锁文件：未能从诊断文本中解析出路径（可能非文件锁形态）".to_owned()
    });
    let output_read = ProcessOutput::new(exit_code, output.to_vec());
    let mut excerpt = output_read.error_excerpt();
    if excerpt.is_empty() {
        excerpt = output_read.tail();
    }
    let excerpt = clip_chars(&excerpt, DIAGNOSIS_EXCERPT_LIMIT);
    let mut text = attribution;
    text.push('\n');
    text.push_str(&format!(
        "已自动重试 {} 次（每次重试会跳过已完成部分）",
        attempts.saturating_sub(1)
    ));
    if !excerpt.is_empty() {
        text.push_str("\n最后一次诊断：\n");
        text.push_str(&excerpt);
    }
    text.push('\n');
    text.push_str(ENV_GUIDANCE);
    text
}

/// chars 口径有界截断（≤ limit 原样；超限取前 limit 字符 + 省略号）。
fn clip_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(limit).collect::<String>())
    }
}

#[cfg(test)]
#[path = "environment_test.rs"]
mod environment_test;
