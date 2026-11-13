use time::OffsetDateTime;

use config::TestSuite;

use crate::globmatch::match_glob;

use crate::model::{
    CaseSummary, Conclusion, CoverageMeasured, CoverageThresholds, MutationBlock, MutationMeasured,
    PlanIndexEntry, Problem, ProblemType, SubReport, SummaryReport,
};

/// 汇总报告 phase 字面量（CLI 固定串同源）。
const PHASE_TEST_EXECUTION: &str = "test-execution";

/// plans[] 路径索引的 change 相对前缀两级（与 foundation layout 报告目录
/// 常量组同名的 change 相对形式——plans[].path 为 change 目录相对索引串，
/// 磁盘写盘路径仍唯一经 `foundation::layout::change_test_reports` 推导）。
const RELATIVE_REPORTS_PREFIX: &str = "reports/test";

// ---------------------------------------------------------------------------
// plan id（CLI derivePlanId 同语义；diagnose / detect 共用的唯一实现）
// ---------------------------------------------------------------------------

/// 由 suite 锚点与框架推导 plan 目录 id：`root` 的路径分隔符替换为 `_`（`.`
/// 归一为空），再接 `_` + 框架名（CLI `derivePlanId` 逐字对齐）。
pub fn derive_plan_id(root: &str, framework: &str) -> String {
    let sanitized = if root == "." {
        String::new()
    } else {
        root.replace(['\\', '/'], "_")
            .trim_end_matches('/')
            .to_owned()
    };
    if sanitized.is_empty() {
        framework.to_owned()
    } else {
        format!("{sanitized}_{framework}")
    }
}

// ---------------------------------------------------------------------------
// conclusion 判定
// ---------------------------------------------------------------------------

/// 聚合结论判定（CLI `determineConclusion` 同语义的布尔面收窄）：error 优先 →
/// failed>0 / 任一 suite 覆盖判定非通过 → fail → pass。
///
/// MUST NOT 接受或读取 `CoverageBlock` / `MutationBlock`（或任何覆盖率数值）——
/// 逐 suite 覆盖判定是唯一判定点，汇总层只做布尔归并；突变结论唯一载体是
/// MutationTest 步载荷，不参与汇总结论。
pub fn determine_conclusion(
    failed: u64,
    sub_reports: &[SubReport],
    has_execution_error: bool,
) -> Conclusion {
    if has_execution_error {
        return Conclusion::Error;
    }
    if failed > 0 || sub_reports.iter().any(coverage_non_pass) {
        return Conclusion::Fail;
    }
    Conclusion::Pass
}

/// 覆盖实测块是否达阈（null 维度感知跳过——`None` 维度不参与判定）。
pub fn coverage_meets_thresholds(
    measured: &CoverageMeasured,
    thresholds: &CoverageThresholds,
) -> bool {
    if measured.lines.is_some_and(|value| value < thresholds.lines) {
        return false;
    }
    if measured
        .branches
        .is_some_and(|value| value < thresholds.branches)
    {
        return false;
    }
    if measured
        .functions
        .is_some_and(|value| value < thresholds.functions)
    {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// suite 范围匹配（CLI isExcludedBySuite 同语义纯移植；glob 匹配经
// crate::globmatch——CLI `matchGlob` 同语义 + 花括号交替方言，detect 与本模块
// 共用实现。消费面 = checks-runtime 突变归桶过滤）
// ---------------------------------------------------------------------------

/// POSIX 正斜杠归一。
fn to_forward_slash(path: &str) -> String {
    path.replace('\\', "/")
}

/// suite root 归一：POSIX 化、`./` 收敛、尾分隔符剥离。
fn normalize_root(root: &str) -> String {
    let normalized = posix_normalize(&to_forward_slash(root));
    normalized.trim_end_matches('/').to_owned()
}

/// POSIX 路径归一（`.` 段消解、`..` 回退；CLI `path.posix.normalize` 同语义
/// 的最小实现）。
fn posix_normalize(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    let joined = segments.join("/");
    if path.starts_with('/') {
        format!("/{joined}")
    } else {
        joined
    }
}

/// POSIX 拼接归一（CLI `path.posix.join` 同语义的最小实现）。
fn posix_join(base: &str, tail: &str) -> String {
    let joined = if base == "." || base.is_empty() {
        tail.to_owned()
    } else {
        format!("{base}/{tail}")
    };
    posix_normalize(&joined)
}

/// 是否 POSIX 绝对路径（`/` 或盘符前缀起手）。
fn is_absolute_posix(path: &str) -> bool {
    path.starts_with('/')
        || path.as_bytes().get(1) == Some(&b':') && path.as_bytes().get(2) == Some(&b'/')
}

/// 文件是否被任一 suite 的 excludes 剔除（CLI `isFileExcluded` 同语义：
/// 排除 glob 相对 suite root 解释，命中该 suite 树下文件即剔除）。消费面：
/// checks-runtime mutation 归桶过滤（`suite.excludes` 全桶消费）。
pub fn is_file_excluded(path: &str, suites: &[TestSuite]) -> bool {
    suites.iter().any(|suite| is_excluded_by_suite(path, suite))
}

/// 单 suite excludes 判定（CLI `isExcludedBySuite` 同语义）：文件须先落在
/// 该 suite root 树下；排除模式既匹配项目相对路径也匹配 root 相对路径。
fn is_excluded_by_suite(path: &str, suite: &TestSuite) -> bool {
    let Some(excludes) = suite.excludes.as_ref().filter(|list| !list.is_empty()) else {
        return false;
    };
    let root = normalize_root(&suite.root);
    let rel_to_root = relative_to_root(path, &root);
    let Some(rel_to_root) = rel_to_root else {
        return false;
    };
    excludes.iter().any(|exclude| {
        let scoped = posix_join(&root, exclude);
        let project_rel = if rel_to_root == "." {
            root.clone()
        } else {
            posix_join(&root, &rel_to_root)
        };
        match_glob(&project_rel, &scoped) || match_glob(&rel_to_root, exclude)
    })
}

/// 路径相对 root 的相对形式（CLI `relativeToRoot` 同语义）：树下返回相对
/// 串（等于 root 返回 "."），树下之外返回 `None`。
fn relative_to_root(path: &str, root: &str) -> Option<String> {
    let path = to_forward_slash(path);
    if path == root {
        return Some(".".to_owned());
    }
    if let Some(rest) = path.strip_prefix(&format!("{root}/")) {
        return Some(rest.to_owned());
    }
    if is_absolute_posix(&path) {
        let marker = format!("/{root}/");
        if let Some(index) = path.find(&marker) {
            return Some(path[index + marker.len()..].to_owned());
        }
        if path.ends_with(&format!("/{root}")) {
            return Some(".".to_owned());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// problems 归并与 summary 组装
// ---------------------------------------------------------------------------

/// 块级覆盖判据之一：该 suite 无覆盖率块（工件缺失 / 解析失败）。判据**无条件**
/// ——采集旗标在注册表模板里是常量，`coverage == None` 只可能是采集失败，不可测
/// ≠ 达标。
fn misses_coverage(report: &SubReport) -> bool {
    report.coverage.is_none()
}

/// 该 suite 覆盖判定是否非通过：块缺席（[`misses_coverage`]）或块自判未达阈。
///
/// 覆盖判定的四形态封闭集在本层收敛：维度级 `None` 由 [`coverage_meets_thresholds`]
/// 在子报告层消化成 `block.pass`，块级缺席由 [`misses_coverage`] 消化。MUST NOT
/// 在别处再写 `coverage.is_none()` / `!block.pass` 的等价判断。
fn coverage_non_pass(report: &SubReport) -> bool {
    misses_coverage(report) || report.coverage.as_ref().is_some_and(|block| !block.pass)
}

/// 问题归并（CLI `collectProblems` 同语义）：失败用例逐条（至多 10 条）
/// test_failure；非零退出且零失败、或零失败携 findings → execution_error。
///
/// 另含**逐 suite 覆盖归因**（[`coverage_problem`]）：覆盖缺失不再与该 suite 的
/// `execution_error` 去重——覆盖面无数据 / 未达阈与执行面失败是两件事，两类条目
/// 并存且互不改写。
fn collect_problems(sub_reports: &[SubReport]) -> Vec<Problem> {
    let mut problems = Vec::new();
    for report in sub_reports {
        if report.summary.failed > 0 {
            let failures = report
                .error_cases
                .iter()
                .filter(|case| case.status == crate::model::TestCaseStatus::Failed);
            for case in failures.take(10) {
                problems.push(Problem {
                    framework: report.framework.clone(),
                    problem_type: ProblemType::TestFailure,
                    message: format!(
                        "Test \"{}\" failed: {}",
                        case.name,
                        case.error_message.as_deref().unwrap_or("Unknown error")
                    ),
                });
            }
        }
        if report.exit_code != 0 && report.summary.failed == 0 {
            let detail = report
                .findings
                .as_ref()
                .and_then(|findings| findings.first().cloned())
                .unwrap_or_else(|| format!("Exit code {}", report.exit_code));
            problems.push(Problem {
                framework: report.framework.clone(),
                problem_type: ProblemType::ExecutionError,
                message: detail,
            });
        } else if report.summary.failed == 0
            && report
                .findings
                .as_ref()
                .is_some_and(|findings| !findings.is_empty())
        {
            problems.push(Problem {
                framework: report.framework.clone(),
                problem_type: ProblemType::ExecutionError,
                message: report
                    .findings
                    .as_ref()
                    .and_then(|findings| findings.first().cloned())
                    .unwrap_or_default(),
            });
        }
        if let Some(problem) = coverage_problem(report) {
            problems.push(problem);
        }
    }
    problems
}

/// 覆盖未达阈人读措辞（CLI `formatCoverageFailure` 同语义：null 维度跳过）。
fn format_coverage_failure(measured: &CoverageMeasured, thresholds: &CoverageThresholds) -> String {
    let mut parts: Vec<String> = Vec::new();
    if measured.lines.is_some_and(|value| value < thresholds.lines) {
        parts.push(format!(
            "lines {:.1}% < {}%",
            measured.lines.unwrap_or_default(),
            thresholds.lines
        ));
    }
    if measured
        .branches
        .is_some_and(|value| value < thresholds.branches)
    {
        parts.push(format!(
            "branches {:.1}% < {}%",
            measured.branches.unwrap_or_default(),
            thresholds.branches
        ));
    }
    if measured
        .functions
        .is_some_and(|value| value < thresholds.functions)
    {
        parts.push(format!(
            "functions {:.1}% < {}%",
            measured.functions.unwrap_or_default(),
            thresholds.functions
        ));
    }
    format!("Coverage below threshold: {}", parts.join(", "))
}

/// 逐 suite 覆盖归因（`framework` = 该 suite 框架，每 suite 至多一条
/// `CoverageFailure`）：块缺席 → 采集缺失措辞（含退出码与动作指引）；块在场但
/// 未达阈 → [`format_coverage_failure`] 原文；通过与块缺席之外的一切形态 → `None`。
fn coverage_problem(report: &SubReport) -> Option<Problem> {
    if misses_coverage(report) {
        return Some(Problem {
            framework: report.framework.clone(),
            problem_type: ProblemType::CoverageFailure,
            message: format!(
                "覆盖率采集缺失（退出码 {}）：工件缺失 / 解析失败，该 suite 覆盖率不可测——请检查该 suite 的采集旗标与覆盖率工件路径",
                report.exit_code
            ),
        });
    }
    let block = report.coverage.as_ref()?;
    if block.pass {
        return None;
    }
    Some(Problem {
        framework: report.framework.clone(),
        problem_type: ProblemType::CoverageFailure,
        message: format_coverage_failure(&block.measured, &block.thresholds),
    })
}

/// score 公式单点：`score = (killed + timeout) / (total - ignored - compileError -
/// runtimeError)`（分母排除 ignored + compileError + runtimeError；分母为 0 →
/// 100.0；两位舍入），阈值判定 `pass = score >= threshold`。MUST NOT 引入百分比
/// 平均等新口径。
pub fn build_mutation_block(measured: MutationMeasured, threshold: f64) -> MutationBlock {
    let valid = measured
        .total
        .saturating_sub(measured.ignored)
        .saturating_sub(measured.compile_error)
        .saturating_sub(measured.runtime_error);
    let score = if valid == 0 {
        100.0
    } else {
        crate::parser::rust::round_two(
            (measured.killed + measured.timeout) as f64 / valid as f64 * 100.0,
        )
    };
    MutationBlock {
        pass: score >= threshold,
        score,
        threshold,
        measured,
        overrides: None,
    }
}

/// 段计数求和归并（段当子报告——计数求和后交 [`build_mutation_block`] 重算
/// score；阈值取首块）。空 slice → `None`（不产块）。MUST NOT 新增归并公式。
pub fn merge_mutation_blocks(blocks: &[MutationBlock]) -> Option<MutationBlock> {
    let first = blocks.first()?;
    let mut measured = MutationMeasured::default();
    for block in blocks {
        measured.killed += block.measured.killed;
        measured.survived += block.measured.survived;
        measured.timeout += block.measured.timeout;
        measured.no_coverage += block.measured.no_coverage;
        measured.compile_error += block.measured.compile_error;
        measured.runtime_error += block.measured.runtime_error;
        measured.ignored += block.measured.ignored;
        measured.total += block.measured.total;
        measured.detected += block.measured.detected;
        measured.undetected += block.measured.undetected;
    }
    Some(build_mutation_block(measured, first.threshold))
}

/// 汇总报告组装（CLI `generateSummaryReport` 同语义的布尔收敛面）：计数求和、
/// problems 归并（含逐 suite 覆盖归因）、plans 路径索引、conclusion 布尔归并。
/// `command` 为命令描述固定串、`started_at` 为链路起点（耗时取墙钟差整秒）。
///
/// MUST NOT 产 `coverage` / `mutation` 块——覆盖率数值的唯一权威面是逐 suite 子报告。
pub fn build_summary_report(
    sub_reports: &[SubReport],
    command: &str,
    started_at: OffsetDateTime,
) -> SummaryReport {
    let mut totals = CaseSummary::default();
    for report in sub_reports {
        totals.total += report.summary.total;
        totals.passed += report.summary.passed;
        totals.failed += report.summary.failed;
        totals.skipped += report.summary.skipped;
    }
    let problems = collect_problems(sub_reports);
    let has_execution_error = problems
        .iter()
        .any(|problem| problem.problem_type == ProblemType::ExecutionError);
    let conclusion = determine_conclusion(totals.failed, sub_reports, has_execution_error);
    let plans = sub_reports
        .iter()
        .map(|report| {
            let id = derive_plan_id(&report.root, &report.framework);
            PlanIndexEntry {
                path: format!("{RELATIVE_REPORTS_PREFIX}/{id}"),
                id,
                framework: report.framework.clone(),
                root: report.root.clone(),
            }
        })
        .collect();
    let now = OffsetDateTime::now_utc();
    let duration_seconds = (now - started_at).as_seconds_f64().max(0.0).round();
    SummaryReport {
        phase: PHASE_TEST_EXECUTION.to_owned(),
        command: command.to_owned(),
        timestamp: now
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        duration_seconds,
        total: totals.total,
        passed: totals.passed,
        failed: totals.failed,
        skipped: totals.skipped,
        conclusion,
        problems,
        plans,
        findings: None,
    }
}
