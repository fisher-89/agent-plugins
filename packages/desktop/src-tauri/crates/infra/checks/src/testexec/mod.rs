mod detect;
mod report;
mod runner;

#[cfg(test)]
mod mod_test;
#[cfg(test)]
mod report_test;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use checks::aggregate::{build_summary_report, coverage_meets_thresholds};
use checks::model::{
    CaseSummary, Conclusion, CoverageBlock, CoverageThresholds, SubReport, SummaryReport,
    TestCaseResult, TestCaseStatus,
};
use checks::{check_integrity, diagnose_findings, is_reusable};
use config::TestSuite;
use orchestration::port::{
    BoxToolFuture, TestExecutionConclusion, TestExecutionOutcome, TestExecutionRunner,
    ToolStepOutput,
};
use time::OffsetDateTime;

use self::detect::TestPlan;

/// summary `command` 字段的桌面固定串（design 待决定稿——桌面 run 内无 CLI
/// 子命令面）。
const DESKTOP_COMMAND_LABEL: &str = "desktop change run: test-execution gate";

/// 诊断摘要单条截断上限（walker `diagnose_brief` 同口径）。
const FINDING_BRIEF_LIMIT: usize = 200;

/// 诊断摘要条数上限。
const FINDING_BRIEF_MAX_ITEMS: usize = 10;

/// test-execution 进程执行器（无状态，组合根按需构造）。
pub struct ProcessTestExecution;

impl ProcessTestExecution {
    /// 构造（组合根装配）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessTestExecution {
    fn default() -> Self {
        Self::new()
    }
}

impl TestExecutionRunner for ProcessTestExecution {
    fn run(&self, root: &str, change: &str) -> BoxToolFuture {
        let root = root.to_owned();
        let change = change.to_owned();
        Box::pin(async move { execute(&root, &change).await })
    }
}

/// 一次 test-execution 执行全链。
async fn execute(root: &str, change: &str) -> Result<ToolStepOutput, String> {
    let started_at = OffsetDateTime::now_utc();
    let root_path = Path::new(root);
    let report = config::load(root_path);
    let suites = &report.config.tests;

    // ① detect（不支持框架显式 Err——不静默跳过不产部分结论）
    let plans = detect::detect_plans(root_path, suites)?;
    let reports_dir = foundation::layout::change_test_reports(root_path, change);

    // ② 完整性校验 + 复用门（既有 summary 在场且齐整且输入未变 → 复用零
    // spawn；反馈边修复重入后 mtime 推进自动失效重跑）
    if let Some(existing) = read_existing_summary(&reports_dir) {
        let disk_reports = load_disk_sub_reports(&reports_dir, &existing);
        let plan_ids_match = plan_ids_match(&plans, &existing);
        let newest = newest_input_mtime_ms(
            &scan_roots(root_path, &plans),
            &prune_roots(root_path, &reports_dir),
        );
        if plan_ids_match
            && check_integrity(&existing, &disk_reports).is_empty()
            && is_reusable(&existing, newest)
        {
            return Ok(outcome(&existing, &reports_dir));
        }
    }

    // ③ 逐 suite 执行（模板展开 + spawn + 超时 + 工件解析；子报告随 suite
    // 阈值组装）
    let mut sub_reports = Vec::new();
    for plan in &plans {
        let execution = runner::execute_plan(plan, &reports_dir.join(&plan.id)).await?;
        sub_reports.push(build_sub_report(plan, execution, suites));
    }

    // ④ 聚合 → 诊断 → 报告写盘（子报告批量落盘后 summary 写一次；findings
    // 随 summary 持久化，修复会话经 report_dir 自读全量）
    let mut summary = build_summary_report(&sub_reports, suites, DESKTOP_COMMAND_LABEL, started_at);
    let findings = diagnose_findings(&summary);
    if !findings.is_empty() {
        summary.findings = Some(findings);
    }
    report::write_reports(&reports_dir, &sub_reports, &summary)?;

    Ok(outcome(&summary, &reports_dir))
}

/// checks 结论 → port 最小载荷映射（checks-runtime 装配点唯一映射面）。
fn outcome(summary: &SummaryReport, reports_dir: &Path) -> ToolStepOutput {
    let conclusion = match summary.conclusion {
        Conclusion::Pass => TestExecutionConclusion::Pass,
        Conclusion::Fail => TestExecutionConclusion::Fail,
        Conclusion::Error => TestExecutionConclusion::Error,
    };
    ToolStepOutput::TestExecution(TestExecutionOutcome {
        conclusion,
        total: summary.total,
        passed: summary.passed,
        failed: summary.failed,
        skipped: summary.skipped,
        findings_brief: findings_brief(summary),
        report_dir: detect::to_posix(reports_dir),
    })
}

/// 诊断摘要装配：单条 200 字符截断、至多 10 条（walker `diagnose_brief`
/// 口径对齐）；findings 缺省回落 problems 消息面（CLI 产出的复用 summary
/// 无 findings 时诊断面不空转）。
fn findings_brief(summary: &SummaryReport) -> String {
    let source: Vec<String> = summary
        .findings
        .clone()
        .unwrap_or_else(|| summary.problems.iter().map(|p| p.message.clone()).collect());
    source
        .iter()
        .take(FINDING_BRIEF_MAX_ITEMS)
        .map(|finding| {
            if finding.chars().count() <= FINDING_BRIEF_LIMIT {
                finding.clone()
            } else {
                format!(
                    "{}…",
                    finding
                        .chars()
                        .take(FINDING_BRIEF_LIMIT)
                        .collect::<String>()
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 子报告组装：suite 阈值（findSuite 语义——framework + root 精确命中 →
/// 框架命中 → 首个 suite → 缺省档）与覆盖块、用例计数、失败用例集、错误
/// findings 随行。
fn build_sub_report(
    plan: &TestPlan,
    execution: runner::PlanExecution,
    suites: &[TestSuite],
) -> SubReport {
    let thresholds = suite_thresholds(plan, suites);
    let coverage = execution.measured.map(|measured| CoverageBlock {
        pass: coverage_meets_thresholds(&measured, &thresholds),
        measured,
        thresholds,
        overrides: None,
    });
    let summary = CaseSummary {
        total: execution.cases.len() as u64,
        passed: count_status(&execution.cases, TestCaseStatus::Passed),
        failed: count_status(&execution.cases, TestCaseStatus::Failed),
        skipped: count_status(&execution.cases, TestCaseStatus::Skipped),
    };
    let error_cases = execution
        .cases
        .iter()
        .filter(|case| case.status != TestCaseStatus::Passed)
        .cloned()
        .collect();
    SubReport {
        framework: plan.framework.clone(),
        root: plan.root.clone(),
        timestamp: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        exit_code: execution.exit_code,
        duration_ms: execution.duration_ms,
        summary,
        error_cases,
        // V1 支持面解析器不产测试文件清单（istanbul 族 JSON 结果不解析用例行
        // 面、文本引擎无文件名输出面——CLI text-parser 同形恒空）
        test_files: Vec::new(),
        source_files: execution.source_files,
        coverage,
        mutation: None,
        findings: execution.error.map(|error| vec![error]),
    }
}

/// suite 覆盖阈值解析（CLI `readCoverageThresholds` 同语义）：framework +
/// root 精确 → 框架 → 首个 suite → 缺省档。
fn suite_thresholds(plan: &TestPlan, suites: &[TestSuite]) -> CoverageThresholds {
    let plan_root = plan
        .root
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_owned();
    let matches_plan = |suite: &TestSuite| {
        detect::framework_name(&suite.framework) == Some(plan.framework.as_str())
            && suite.root.replace('\\', "/").trim_end_matches('/') == plan_root
    };
    let suite = suites
        .iter()
        .find(|suite| matches_plan(suite))
        .or_else(|| {
            suites.iter().find(|suite| {
                detect::framework_name(&suite.framework) == Some(plan.framework.as_str())
            })
        })
        .or_else(|| suites.first());
    let (lines, branches, functions) = suite
        .map(|suite| {
            (
                suite.coverage.lines,
                suite.coverage.branches,
                suite.coverage.functions,
            )
        })
        .unwrap_or((80.0, 70.0, 75.0));
    CoverageThresholds {
        lines,
        branches,
        functions,
    }
}

/// 用例状态计数。
fn count_status(cases: &[TestCaseResult], status: TestCaseStatus) -> u64 {
    cases.iter().filter(|case| case.status == status).count() as u64
}

/// 既有 summary 读取（缺失 / 不可解析 → `None`，交全量跑）。
fn read_existing_summary(reports_dir: &Path) -> Option<SummaryReport> {
    let text = std::fs::read_to_string(reports_dir.join("summary.json")).ok()?;
    checks::parse_summary_report(&text).ok()
}

/// 既有子报告装载（逐 plan 读 `<planId>/report.json`；缺失 / 不可解析跳过
/// ——完整性 checklist 对缺失条目记因）。
fn load_disk_sub_reports(reports_dir: &Path, summary: &SummaryReport) -> Vec<SubReport> {
    summary
        .plans
        .iter()
        .filter_map(|plan| {
            let text =
                std::fs::read_to_string(reports_dir.join(&plan.id).join("report.json")).ok()?;
            checks::parse_sub_report(&text).ok()
        })
        .collect()
}

/// plan 集一致判定（CLI `summaryMatchesScope` 同语义）：detected plan id 集
/// 与既有 summary.plans id 集相等（scope 变了 → 概不复用）。
fn plan_ids_match(plans: &[TestPlan], summary: &SummaryReport) -> bool {
    let expected: BTreeSet<&str> = plans.iter().map(|plan| plan.id.as_str()).collect();
    let recorded: BTreeSet<&str> = summary.plans.iter().map(|plan| plan.id.as_str()).collect();
    expected == recorded
}

/// 复用门扫描根：各 plan 绝对 root（去重）+ 配置文件（阈值 / suite 映射
/// 变更即输入变化）。
fn scan_roots(root_path: &Path, plans: &[TestPlan]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = plans
        .iter()
        .map(|plan| root_path.join(&plan.root))
        .collect();
    roots.push(foundation::layout::config_path(root_path));
    roots
}

/// 复用门剪枝目录：change 目录树（workflow 状态与报告同树）与报告目录
/// ——两者都不是执行输入。
fn prune_roots(root_path: &Path, reports_dir: &Path) -> Vec<PathBuf> {
    let layout = foundation::layout::resolve(root_path);
    vec![
        layout.changes_root,
        layout.explores_root,
        reports_dir.to_path_buf(),
    ]
}

/// 输入最新 mtime 递归扫描（CLI `computeNewestInputMtime` 同口径）：扫描根
/// 为文件时直接计、为目录时递归遍历——dot 目录、依赖 / 工具产物目录名与
/// 剪枝目录树内文件不计；不可读文件忽略。空输入集 → `None`（视为新鲜）。
pub(crate) fn newest_input_mtime_ms(scan_roots: &[PathBuf], prune: &[PathBuf]) -> Option<u64> {
    let mut newest: Option<u64> = None;
    for scan_root in scan_roots {
        if scan_root.is_file() {
            newest = consider(scan_root, newest);
        } else if scan_root.is_dir() {
            newest = scan_dir(scan_root, prune, newest);
        }
    }
    newest
}

/// 依赖与工具产物目录名（CLI `REUSE_SCAN_EXCLUDED_DIRS` 同集——永不属于
/// 执行输入）。
const EXCLUDED_DIR_NAMES: [&str; 6] = [
    "node_modules",
    "dist",
    "build",
    "target",
    "coverage",
    "_stryker-tmp",
];

/// 目录递归扫描（迭代栈；dot 目录 / 排除目录名 / 剪枝目录树内路径剪枝）。
fn scan_dir(dir: &Path, prune: &[PathBuf], mut newest: Option<u64>) -> Option<u64> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || EXCLUDED_DIR_NAMES.contains(&name.as_str()) {
                    continue;
                }
                if prune.iter().any(|prune_dir| path.starts_with(prune_dir)) {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() {
                newest = consider(&path, newest);
            }
        }
    }
    newest
}

/// 单文件 mtime 折入最新值追踪（不可读忽略）。
fn consider(path: &Path, newest: Option<u64>) -> Option<u64> {
    let mtime = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    Some(newest.map_or(mtime, |current| current.max(mtime)))
}
