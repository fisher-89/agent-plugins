//! 检查域环境归因装配单点（static-check 与 test-execution 共用；MUST NOT 两写）。
//!
//! 四个职责：
//! - [`attempts_log`]：输出记录内容形态——单次尝试**逐字节同现状**（`stdout ++
//!   stderr`，零分隔头）；发生过重试则各次尝试按序拼接、每段前加分隔头（审计不丢
//!   上下文）；
//! - [`fault_text`]：`RunFault::Environment` 的人读文本——占用者点名 / 被锁文件
//!   （原料来自 [`exec::attribution_text`]）+ 「已自动重试 N 次」+ 可选「重试进展」
//!   行 + 末次诊断摘要 + 环境侧处理指引；
//! - [`progress_line`]：重试进展行（各次尝试的待编译计数序列 + 卡死判定附语）——
//!   计数口径单点驻 `exec::transient`，本模块**不重算**（只经
//!   [`exec::progress_series`] / [`exec::is_progress_stalled`] 取数）；
//! - [`argv_budget_text`] / [`argv_degrade_note`]：**命令行超平台长度上限**成因两臂
//!   （拒绝面摘要 / 降级面记录注）——与文件锁 / 超时 / 瞬态耗尽成因不同，MUST NOT
//!   互相套用文案（误引会让用户去改安全软件配置）。
//!
//! exec 层只供事实原料与尝试面字节（MUST NOT 自带业务文案：尝试分隔头 / 归因文案
//! 归调用方），故重试次数与处置指引的措辞单点驻本模块；本模块为**纯函数**（零 io
//! / 零 async）。

use exec::{AttemptTrace, ProcessOutput, RetryPolicy, RunOutcome, RunReport};

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
/// 被锁文件）+ 重试事实 + 可选「重试进展」行 + 末次诊断摘要 + 处置指引。
///
/// `attempts` / `exit_code` / `output` 取**末次**尝试（`static-check` 传
/// `&run.stdout`——命令串带 `2>&1`，`stderr` 恒空；test-execution 传合并流），
/// `retried` 为此前各次尝试的留痕（末次在 `output` 里）。诊断摘要复用
/// [`ProcessOutput`] 的既有截断口径（不自造「取末尾 N 字符」）；推进序列行长度为
/// `O(attempts)`（至多 5 个数字）。
pub(crate) fn fault_text(
    attempts: u32,
    exit_code: i32,
    output: &[u8],
    retried: &[AttemptTrace],
) -> String {
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
    // 计数口径单点驻 `exec::transient`：本模块只构造一次性「视图报告」取序列，零
    // 计数逻辑。test-execution 侧传入的已是 `stdout ++ stderr` 合并流，故末次只填
    // `stdout`、`stderr` 留空（否则同一段字节被计两次）；`outcome` 只为构造视图，
    // 计数不看它。
    let series = exec::progress_series(&RunReport {
        outcome: RunOutcome::Completed,
        exit_code,
        stdout: output.to_vec(),
        stderr: Vec::new(),
        attempts,
        retried: retried.to_vec(),
    });
    let mut text = attribution;
    text.push('\n');
    text.push_str(&format!(
        "已自动重试 {} 次（每次重试会跳过已完成部分）",
        attempts.saturating_sub(1)
    ));
    if let Some(line) = progress_line(&series) {
        text.push('\n');
        text.push_str(&line);
    }
    if !excerpt.is_empty() {
        text.push_str("\n最后一次诊断：\n");
        text.push_str(&excerpt);
    }
    text.push('\n');
    text.push_str(ENV_GUIDANCE);
    text
}

/// 重试进展行（`None` = 不输出该行）：
/// - `attempts == 1`（`series.len() < 2`，未重试）——无进展可言；
/// - 序列各次计数全为 `0`（非 cargo 构建形态，如 Node / 链接器报错）——退化语义下
///   计数无可读信息。
///
/// 其余一律输出（`[264, 0]` 也输出——确有推进可读）；卡死判定经
/// [`exec::is_progress_stalled`] 取（本模块不重算判据）。
fn progress_line(series: &[u32]) -> Option<String> {
    if series.len() < 2 || series.iter().all(|count| *count == 0) {
        return None;
    }
    let counts = series
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" → ");
    let mut line = format!("重试进展：待编译 {counts}");
    if exec::is_progress_stalled(series) {
        line.push_str("（末两次无进展，判定为卡死）");
    }
    Some(line)
}

/// 命令行超平台上限的**拒绝面**摘要（`RunFault::Environment(String)` 的唯一装配点
/// ——test-execution 展开长度超预算且不可安全降级 / 降级后仍超预算）。
///
/// 三项依序：① 定性（harness 限制、**非被测代码问题**——避免把修复引向被测仓库）；
/// ② 数字（展开长度 / 平台预算 / 测试文件数；`omitted_len` 为 `Some` 时追加「降级
/// 为框架自发现后仍为 N 字符」，使「降级无效」也能回查两次长度）；③ 动作（收窄
/// suite `includes` 或拆分 suite）。
///
/// 消费面：随 `RunFault::Environment` 交 walker 终局文案的 `{fault}` 段；MUST NOT
/// 进 IPC / `RunUpdate` / `RunNodeRecord` / `bindings.ts`。本函数 MUST NOT 复用
/// 安全软件排除指引（成因是命令长度而非文件锁，误引会让用户去改安全软件）。
pub(crate) fn argv_budget_text(
    budget: usize,
    expanded_len: usize,
    omitted_len: Option<usize>,
    files: usize,
) -> String {
    let mut text = format!(
        "命令行长度超平台上限（{budget} 字符）：属 harness 限制，非被测代码问题\n\
         展开 {expanded_len} 字符 / 平台预算 {budget} 字符 / {files} 个测试文件"
    );
    if let Some(omitted_len) = omitted_len {
        text.push_str(&format!("\n降级为框架自发现后仍为 {omitted_len} 字符"));
    }
    text.push_str("\n请收窄该 suite 的 `includes` 或拆分 suite");
    text
}

/// 命令行超平台上限的**降级面**记录注（安全降级时 `PlanExecution::argv_note` 的
/// 唯一装配点，只进输出记录的 plan 头 `[argv]` 行）。
///
/// 陈述三数字 + 「已降级为框架自发现（K 个测试文件）」，**零动作建议**（降级已发生，
/// 此事只需可查）。MUST NOT 进 `SubReport.findings` / `summary.findings`（`findings`
/// 非空 + `failed == 0` 会被 `collect_problems` 判成 `execution_error`，绿跑翻红）；
/// MUST NOT 含安全软件排除指引 / 占用者 / 被锁文件 / 重试次数措辞。
pub(crate) fn argv_degrade_note(budget: usize, expanded_len: usize, files: usize) -> String {
    format!(
        "展开 {expanded_len} 字符超平台预算 {budget} 字符，已降级为框架自发现（{files} 个测试文件）"
    )
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
