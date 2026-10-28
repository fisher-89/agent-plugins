//! 门禁执行体：[`ArchiCheckRunner`] 的 `archi-runtime` 实现——调判定 → 落盘 →
//! 派生 outcome。
//!
//! `passed` / 摘要 / 违例明细 / 固定处置提示**全部在本 runner 侧装配**（walker 只
//! 认 `passed` / `diagnostics` / `fault`，不引入任何 `archi` 词汇）；文本上限复用
//! `orchestration::verdict::MAX_REPORT_CHARS`（零新常量）。
//!
//! **零进程** ⇒ 生产路径 `fault` 恒 `None`（字段保留以与既有三 outcome 同形）；
//! 报告写盘失败 MUST NOT 返回 `Err`（降级 `report_ref = None` 并保留结论）。

use std::path::Path;

use archi::drift::{check, ArchiCheckStatus, ArchiDriftMode, ArchiDriftReport, CheckError};
use foundation::cancel::CancelFlag;
use foundation::layout::{change_reports, ReportSlot};
use orchestration::port::{
    ArchiCheckOutcome, ArchiCheckRunner, BoxToolFuture, RunFault, ToolStepOutput,
};
use orchestration::verdict::MAX_REPORT_CHARS;

use crate::report::write_report;

/// 失败态固定处置提示（提示来自本常量，MUST NOT 改动 `prompts/` 静态角色 md）。
const FIX_HINT: &str = "先按模型改代码；若真实设计如此，应回到 dev-design 补模型声明，\
                        MUST NOT 在此处硬改代码消违例";

/// 截断留痕尾注。
const TRUNCATED: &str = "…（已截断，全量见报告文件）";

/// 架构漂移门禁 runner（零参构造——判定全在 `archi`，本层无可配项）。
pub struct LocalArchiCheck;

impl LocalArchiCheck {
    /// 组合根装配（零参）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalArchiCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiCheckRunner for LocalArchiCheck {
    fn run(
        &self,
        root: &str,
        name: &str,
        changed: Vec<String>,
        cancel: CancelFlag,
    ) -> BoxToolFuture {
        let root = root.to_owned();
        let name = name.to_owned();
        Box::pin(async move {
            let root_path = Path::new(&root);
            let report_dir = change_reports(root_path, &name, ReportSlot::Archi)
                .to_string_lossy()
                .into_owned();
            let outcome = check(archi::drift::CheckInput {
                root: root_path,
                mode: ArchiDriftMode::Incremental,
                changed: &changed,
                change: Some(&name),
                cancel,
            });
            let report = match outcome {
                Ok(report) => report,
                // 取消 MUST NOT 以 `Err` 表达（检查本身毫秒级，取消只在扫描长尾
                // 命中）——正常返回并落 `fault`
                Err(CheckError::Stopped) => {
                    return Ok(ToolStepOutput::ArchiCheck(ArchiCheckOutcome {
                        passed: false,
                        diagnostics: CheckError::Stopped.message(),
                        fault: Some(RunFault::Stopped),
                        report_dir,
                        report_ref: None,
                    }));
                }
                // 配置非法 / 扫描硬失败：停给用户（不产出部分结论）
                Err(error) => return Err(error.message()),
            };
            // 报告落盘失败降级 `None`（审计面失败不阻断结论，主流程不中断）
            let report_ref = write_report(root_path, &name, &report).ok();
            let passed = report.errors.is_empty() && report.model.structural_errors.is_empty();
            let diagnostics = compose_text(&report, report_ref.as_deref());
            Ok(ToolStepOutput::ArchiCheck(ArchiCheckOutcome {
                passed,
                diagnostics,
                fault: None,
                report_dir,
                report_ref,
            }))
        })
    }
}

/// 诊断文本装配（有界：整体不超过 [`MAX_REPORT_CHARS`] 字符）：
///
/// - `skipped` → `未检查（无模型）`（逐字，**不附路径**——「没检查」MUST NOT 被读成
///   「检查过了没问题」）；
/// - `noChanges` → `未检查（无可判文件）`（逐字，不附路径）；
/// - `clean` → 通过摘要（模式 / 状态 / 扫描文件数）+ 报告路径；
/// - `warnings` → 通过摘要 + 逐条提示 + 报告路径；
/// - `errors` → 逐条违例（`from → to`、档位、出现次数、至多两处 `file:line`）+
///   单列结构错 + 末行固定处置提示。
fn compose_text(report: &ArchiDriftReport, report_ref: Option<&str>) -> String {
    match report.status {
        ArchiCheckStatus::Skipped => "未检查（无模型）".to_owned(),
        ArchiCheckStatus::NoChanges => "未检查（无可判文件）".to_owned(),
        ArchiCheckStatus::Clean => {
            let mut text = format!(
                "架构检查通过（mode={} status=clean，零违例，扫描 {} 文件）",
                report.mode.as_str(),
                report.scan.scanned_files
            );
            text.push_str(&report_suffix(report_ref));
            clip(text)
        }
        ArchiCheckStatus::Warnings => {
            let mut text = format!(
                "架构检查通过（mode={} status=warnings，{} 条提示，扫描 {} 文件）",
                report.mode.as_str(),
                report.warnings.len(),
                report.scan.scanned_files
            );
            for warning in &report.warnings {
                text.push('\n');
                text.push_str(&entry_line(warning));
            }
            text.push_str(&report_suffix(report_ref));
            clip(text)
        }
        ArchiCheckStatus::Errors => {
            let mut text = format!(
                "架构漂移门禁未通过（mode={} status=errors，{} 条违例，扫描 {} 文件）",
                report.mode.as_str(),
                report.errors.len(),
                report.scan.scanned_files
            );
            for error in &report.errors {
                text.push('\n');
                text.push_str(&entry_line(error));
            }
            if !report.model.structural_errors.is_empty() {
                text.push_str(&format!(
                    "\n模型结构错（共 {} 条，按模型修好模型文件本身）：",
                    report.model.structural_errors.len()
                ));
                for diagnostic in &report.model.structural_errors {
                    text.push_str(&format!(
                        "\n- {} @ {}",
                        diagnostic.kind.as_str(),
                        location(&diagnostic.file, diagnostic.line)
                    ));
                }
            }
            text.push_str(&report_suffix(report_ref));
            text.push('\n');
            text.push_str(FIX_HINT);
            clip(text)
        }
    }
}

/// 单条目摘要行（元素对 + 档位 + 出现次数 + 至多两处 `file:line`）。
fn entry_line(entry: &archi::drift::ArchiDriftEntry) -> String {
    let pair = match (&entry.from, &entry.to) {
        (Some(from), Some(to)) => format!("{from} → {to}"),
        (Some(from), None) => from.clone(),
        _ => entry.kind.as_str().to_owned(),
    };
    let gate = entry
        .gate
        .map(|gate| format!(" gate={}", gate.as_str()))
        .unwrap_or_default();
    let count = entry.occurrences.len();
    let mut line = format!(
        "- {} [{}]{} 命中 {} 处：{}",
        pair,
        entry.kind.as_str(),
        gate,
        count,
        entry.message
    );
    for occurrence in entry.occurrences.iter().take(2) {
        line.push_str(&format!(
            " @ {}",
            location(&occurrence.file, occurrence.line)
        ));
    }
    if count > 2 {
        line.push_str(&format!(" … 另 {} 处", count - 2));
    }
    line
}

/// `file` / `file:line`（`line == 0` 表示位置未知）。
fn location(file: &str, line: u32) -> String {
    if line == 0 {
        file.to_owned()
    } else {
        format!("{file}:{line}")
    }
}

/// 报告路径尾注（`report_ref` 在位时追加；`skipped` / `noChanges` 不调用）。
fn report_suffix(report_ref: Option<&str>) -> String {
    match report_ref {
        Some(reference) => format!("；报告 {reference}"),
        None => String::new(),
    }
}

/// 文本截断至 [`MAX_REPORT_CHARS`] 字符（尾部留截断痕迹）。
fn clip(text: String) -> String {
    if text.chars().count() <= MAX_REPORT_CHARS {
        return text;
    }
    let keep = MAX_REPORT_CHARS.saturating_sub(TRUNCATED.chars().count());
    format!(
        "{}{}",
        text.chars().take(keep).collect::<String>(),
        TRUNCATED
    )
}
