use time::OffsetDateTime;

use config::TestSuite;

use crate::model::{
    CaseSummary, Conclusion, CoverageBlock, CoverageMeasured, CoverageOverride, CoverageThresholds,
    MutationBlock, MutationMeasured, PlanIndexEntry, Problem, ProblemType, SourceFileEntry,
    SubReport, SummaryReport,
};

/// 汇总报告 phase 字面量（CLI 固定串同源）。
const PHASE_TEST_EXECUTION: &str = "test-execution";

/// plans[] 路径索引的 change 相对前缀两级（与 foundation layout 报告目录
/// 常量组同名的 change 相对形式——plans[].path 为 change 目录相对索引串，
/// 磁盘写盘路径仍唯一经 `foundation::layout::change_test_reports` 推导）。
const RELATIVE_REPORTS_PREFIX: &str = "reports/test";

/// schema 缺省覆盖阈值（CLI `defaults.ts` 同值：80 / 70 / 75；仅当无任何
/// suite 配置时作为全局阈值兜底）。
const DEFAULT_THRESHOLDS: CoverageThresholds = CoverageThresholds {
    lines: 80.0,
    branches: 70.0,
    functions: 75.0,
};

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

/// 聚合结论判定（CLI `determineConclusion` 同语义）：error 优先 → failed>0 /
/// 覆盖未达 / 突变未达 → fail → pass。
pub fn determine_conclusion(
    failed: u64,
    coverage: Option<&CoverageBlock>,
    mutation: Option<&MutationBlock>,
    has_execution_error: bool,
) -> Conclusion {
    if has_execution_error {
        return Conclusion::Error;
    }
    if failed > 0
        || coverage.is_some_and(|block| !block.pass)
        || mutation.is_some_and(|block| !block.pass)
    {
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
// 覆盖聚合
// ---------------------------------------------------------------------------

/// 全局覆盖聚合：全局阈值（首个 suite 的阈值，缺省 80 / 70 / 75）+ 实测块
/// + per-suite override 组（CLI `computeCoverageResult` 同语义）。
///
/// 无实测数据 → `None`（覆盖采集失败语义，判定按 null 感知跳过）。
pub fn aggregate_coverage(
    sub_reports: &[SubReport],
    suites: &[TestSuite],
) -> Option<CoverageBlock> {
    let thresholds = suites
        .first()
        .map(|suite| to_thresholds(&suite.coverage))
        .unwrap_or(DEFAULT_THRESHOLDS);
    let measured = weighted_measured(sub_reports)?;
    let overrides = compute_overrides(sub_reports, suites);
    let mut pass = coverage_meets_thresholds(&measured, &thresholds);
    if !overrides.is_empty() {
        pass = pass && overrides.iter().all(|override_entry| override_entry.pass);
    }
    Some(CoverageBlock {
        pass,
        measured,
        thresholds,
        overrides: if overrides.is_empty() {
            None
        } else {
            Some(overrides)
        },
    })
}

/// per-suite override 组（CLI `computeOverrides` 同语义）：逐 suite 圈定范围
/// 内的原始计数条目，按该 suite 阈值判定；范围零命中 → 该组不产条目。
fn compute_overrides(sub_reports: &[SubReport], suites: &[TestSuite]) -> Vec<CoverageOverride> {
    if suites.is_empty() {
        return Vec::new();
    }
    let all_entries: Vec<&SourceFileEntry> = sub_reports
        .iter()
        .flat_map(|report| report.source_files.iter())
        .collect();
    let mut results = Vec::new();
    for suite in suites {
        let matched: Vec<&SourceFileEntry> = all_entries
            .iter()
            .copied()
            .filter(|entry| in_suite_scope(&entry.file, suite, suites))
            .collect();
        if matched.is_empty() {
            continue;
        }
        let thresholds = to_thresholds(&suite.coverage);
        let measured = aggregate_measured(matched.iter().copied());
        let file_count = matched.len() as u64;
        let passed_count = matched
            .iter()
            .filter(|entry| file_meets_thresholds(entry, &thresholds))
            .count() as u64;
        results.push(CoverageOverride {
            glob: suite.root.clone(),
            thresholds,
            measured,
            pass: coverage_meets_thresholds(&measured, &thresholds),
            file_count,
            passed_count,
        });
    }
    results
}

/// 逐文件判定：单文件覆盖计数换算百分点后过阈值（无计数条目不计通过）。
fn file_meets_thresholds(entry: &SourceFileEntry, thresholds: &CoverageThresholds) -> bool {
    let Some(coverage) = entry.coverage else {
        return false;
    };
    let pct = |covered: Option<u64>, total: Option<u64>| -> Option<f64> {
        match (total, covered) {
            (Some(total), _) if total > 0 => Some(crate::parser::rust::round_two(
                covered.unwrap_or(0) as f64 / total as f64 * 100.0,
            )),
            _ => None,
        }
    };
    coverage_meets_thresholds(
        &CoverageMeasured {
            lines: pct(coverage.covered_lines, coverage.total_lines),
            branches: pct(coverage.covered_branches, coverage.total_branches),
            functions: pct(coverage.covered_functions, coverage.total_functions),
        },
        thresholds,
    )
}

/// 原始计数加权聚合（CLI `computeRawOverrideCoverage` 同语义）：逐维度累计
/// 非空正总数与覆盖数，真百分比两位舍入；零有效数据维度为 `None`。
pub fn aggregate_measured<'a>(
    entries: impl Iterator<Item = &'a SourceFileEntry>,
) -> CoverageMeasured {
    let mut totals = [0u64; 3];
    let mut covered = [0u64; 3];
    for entry in entries {
        let Some(coverage) = entry.coverage else {
            continue;
        };
        let dimensions = [
            (coverage.total_lines, coverage.covered_lines),
            (coverage.total_branches, coverage.covered_branches),
            (coverage.total_functions, coverage.covered_functions),
        ];
        for (index, (total, hit)) in dimensions.iter().enumerate() {
            if let Some(total) = total.filter(|total| *total > 0) {
                totals[index] += total;
                covered[index] += hit.unwrap_or(0);
            }
        }
    }
    let pct = |index: usize| -> Option<f64> {
        if totals[index] > 0 {
            Some(crate::parser::rust::round_two(
                covered[index] as f64 / totals[index] as f64 * 100.0,
            ))
        } else {
            None
        }
    };
    CoverageMeasured {
        lines: pct(0),
        branches: pct(1),
        functions: pct(2),
    }
}

/// 汇总侧加权实测（CLI `computeRawWeightedCoverage` 同语义）：单子报告直接
/// 取其覆盖块实测；多子报告自全部逐文件条目加权聚合；全部无数据 → `None`。
fn weighted_measured(sub_reports: &[SubReport]) -> Option<CoverageMeasured> {
    if sub_reports.len() == 1 {
        return sub_reports[0].coverage.as_ref().map(|block| block.measured);
    }
    let has_any = sub_reports.iter().any(|report| {
        report
            .source_files
            .iter()
            .any(|entry| entry.coverage.is_some())
    });
    if !has_any {
        return None;
    }
    Some(aggregate_measured(
        sub_reports
            .iter()
            .flat_map(|report| report.source_files.iter()),
    ))
}

fn to_thresholds(coverage: &config::CoverageThresholds) -> CoverageThresholds {
    CoverageThresholds {
        lines: coverage.lines,
        branches: coverage.branches,
        functions: coverage.functions,
    }
}

// ---------------------------------------------------------------------------
// suite 范围匹配（CLI isInSuiteScope / isExcludedBySuite 同语义纯移植）
// ---------------------------------------------------------------------------

/// glob 匹配选项（picomatch `{ dot: true }` 对齐：`*` 不跨路径分隔符、
/// dot 文件可匹配）。
fn match_options() -> glob::MatchOptions {
    glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    }
}

/// glob 匹配（CLI `matchGlob` 同语义）：无通配符模式按目录前缀匹配，含
/// 通配符模式经 `glob::Pattern` 全匹配。
fn match_glob(path: &str, pattern: &str) -> bool {
    let pattern = to_forward_slash(pattern);
    if !pattern.contains(['*', '?', '{', '[']) {
        let path = to_forward_slash(path);
        return path == pattern || path.starts_with(&format!("{pattern}/"));
    }
    glob::Pattern::new(&pattern)
        .map(|compiled| compiled.matches_with(&to_forward_slash(path), match_options()))
        .unwrap_or(false)
}

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

/// 项目相对路径是否位于 suite root 树下（含等于）：前缀段匹配；绝对路径按
/// CLI `relativeToRoot` 同口径以 `/root/` 段标记搜索容忍项目前缀。
fn under_root(path: &str, root: &str) -> bool {
    if path == root {
        return true;
    }
    if path.starts_with(&format!("{root}/")) {
        return true;
    }
    if is_absolute_posix(path) {
        let marker = format!("/{root}/");
        if path.contains(&marker) || path.ends_with(&format!("/{root}")) {
            return true;
        }
    }
    false
}

/// 文件是否被任一 suite 的 excludes 剔除（CLI `isFileExcluded` 同语义：
/// 排除 glob 相对 suite root 解释，命中该 suite 树下文件即剔除）。
fn is_file_excluded(path: &str, suites: &[TestSuite]) -> bool {
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

/// 文件是否落在 suite 范围内（CLI `isInSuiteScope` 同语义）：root 树下 ∧
/// 未被任一 suite excludes 剔除 ∧ 命中 includes（未设吃框架 default_glob，
/// 模式挂 root 归一为项目相对）。
pub fn in_suite_scope(path: &str, suite: &TestSuite, suites: &[TestSuite]) -> bool {
    let posix = to_forward_slash(path);
    let root = normalize_root(&suite.root);
    if !under_root(&posix, &root) {
        return false;
    }
    if is_file_excluded(&posix, suites) {
        return false;
    }
    let include_patterns: Vec<String> = match suite.includes.as_ref() {
        Some(list) if !list.is_empty() => list.clone(),
        _ => vec![default_glob(&suite.framework).to_owned()],
    };
    include_patterns
        .iter()
        .any(|pattern| match_glob(&posix, &posix_join(&root, pattern)))
}

/// 框架缺省 glob（CLI `test-framework.ts` 注册表 default_glob 同值镜像）：
/// 纯层不可依赖 infra 注册表，此处仅镜像五支持框架的范围词汇供 suite 未设
/// includes 时圈定（权威在 infra `detect.rs` 注册表，两侧由测试对齐锚定）。
fn default_glob(framework: &config::TestFramework) -> &'static str {
    match framework {
        config::TestFramework::Jest
        | config::TestFramework::Vitest
        | config::TestFramework::VitePlus => "**/*.{test,spec}.{js,ts,jsx,tsx}",
        config::TestFramework::Rust => "**/tests/**/*.rs",
        config::TestFramework::NodeTest => "**/*.test.{mjs,js,cjs}",
        // 不支持框架不出现在生产链（detect 显式 Err），范围退化为仅 root 圈定
        config::TestFramework::Bun | config::TestFramework::Go | config::TestFramework::Pytest => {
            "**"
        }
    }
}

// ---------------------------------------------------------------------------
// problems 归并与 summary 组装
// ---------------------------------------------------------------------------

/// 问题归并（CLI `collectProblems` 同语义）：失败用例逐条（至多 10 条）
/// test_failure；非零退出且零失败、或零失败携 findings → execution_error。
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

/// 突变聚合（CLI `computeMutationResult` 主干同语义）：无任何子报告携突变
/// 块 → `None`（生产聚合恒 `None`——V1 不移植 mutation；corpus 含块样本时
/// 按计数加权归并）。override 组不产（V1 边界）。
fn aggregate_mutation(sub_reports: &[SubReport]) -> Option<MutationBlock> {
    let blocks: Vec<&MutationBlock> = sub_reports
        .iter()
        .filter_map(|report| report.mutation.as_ref())
        .collect();
    let first = blocks.first()?;
    let mut measured = MutationMeasured::default();
    for block in &blocks {
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
    let threshold = first.threshold;
    Some(MutationBlock {
        pass: score >= threshold,
        score,
        threshold,
        measured,
        overrides: None,
    })
}

/// 汇总报告组装（CLI `generateSummaryReport` 同语义）：计数合并、problems
/// 归并、coverage / mutation 块、plans 路径索引、conclusion 判定。`command`
/// 为命令描述固定串、`started_at` 为链路起点（耗时取墙钟差整秒）。
pub fn build_summary_report(
    sub_reports: &[SubReport],
    suites: &[TestSuite],
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
    let mut problems = collect_problems(sub_reports);
    let coverage = aggregate_coverage(sub_reports, suites);
    if coverage.as_ref().is_some_and(|block| !block.pass) {
        let block = coverage.as_ref().expect("coverage 已判定在位");
        problems.push(Problem {
            framework: "all".to_owned(),
            problem_type: ProblemType::CoverageFailure,
            message: format_coverage_failure(&block.measured, &block.thresholds),
        });
    }
    let mutation = aggregate_mutation(sub_reports);
    if mutation.as_ref().is_some_and(|block| !block.pass) {
        let block = mutation.as_ref().expect("mutation 已判定在位");
        problems.push(Problem {
            framework: "all".to_owned(),
            problem_type: ProblemType::CoverageFailure,
            message: format!(
                "Mutation score {:.1}% < threshold {}%",
                block.score, block.threshold
            ),
        });
    }
    let has_execution_error = problems
        .iter()
        .any(|problem| problem.problem_type == ProblemType::ExecutionError);
    let conclusion = determine_conclusion(
        totals.failed,
        coverage.as_ref(),
        mutation.as_ref(),
        has_execution_error,
    );
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
        coverage,
        mutation,
        plans,
        findings: None,
    }
}
