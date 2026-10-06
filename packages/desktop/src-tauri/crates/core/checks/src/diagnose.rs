use crate::aggregate::{derive_plan_id, determine_conclusion};
use crate::model::{Conclusion, CoverageMeasured, ProblemType, SubReport, SummaryReport};

/// 失败问题归并的单框架上限（措辞面收敛，明细在子报告 error_cases）。
const CLUSTER_FRAMEWORK_LIMIT: usize = 10;

/// 完整性 checklist：三项全过 → 空集（无违例）。
///
/// ① 聚合计数对账——summary 四计数与子报告计数之和解恒等；
/// ② plan 报告齐全性——`plans[]` 条目与子报告按 planId 双向一一对应；
/// ③ conclusion 一致性——聚合结论与计数 / 覆盖 / 突变 / execution_error
///    问题面重算结果恒等（"全绿计数却 fail" 等漂移即违例）。
pub fn check_integrity(summary: &SummaryReport, sub_reports: &[SubReport]) -> Vec<String> {
    let mut findings = Vec::new();

    // ① 聚合计数对账
    let mut totals = (0u64, 0u64, 0u64, 0u64);
    for report in sub_reports {
        totals.0 += report.summary.total;
        totals.1 += report.summary.passed;
        totals.2 += report.summary.failed;
        totals.3 += report.summary.skipped;
    }
    if (
        summary.total,
        summary.passed,
        summary.failed,
        summary.skipped,
    ) != totals
    {
        findings.push(format!(
            "聚合计数对账失败: summary total/passed/failed/skipped = {}/{}/{}/{}, 子报告之和 = {}/{}/{}/{}",
            summary.total, summary.passed, summary.failed, summary.skipped,
            totals.0, totals.1, totals.2, totals.3
        ));
    }

    // ② plan 报告齐全性（双向：plans 条目有子报告、子报告入索引）
    let recorded: Vec<&str> = summary.plans.iter().map(|plan| plan.id.as_str()).collect();
    let expected: Vec<String> = sub_reports
        .iter()
        .map(|report| derive_plan_id(&report.root, &report.framework))
        .collect();
    for plan in &summary.plans {
        if !expected.contains(&plan.id) {
            findings.push(format!(
                "plan 报告缺失: plans[] 条目 \"{}\"（{}）无对应子报告",
                plan.id, plan.framework
            ));
        }
    }
    for report in sub_reports {
        let id = derive_plan_id(&report.root, &report.framework);
        if !recorded.contains(&id.as_str()) {
            findings.push(format!(
                "plan 索引缺失: 子报告 \"{}\"（{}）未入 plans[] 路径索引",
                id, report.framework
            ));
        }
    }

    // ③ conclusion 一致性（重算比对）
    let has_execution_error = summary
        .problems
        .iter()
        .any(|problem| problem.problem_type == ProblemType::ExecutionError);
    let expected_conclusion = determine_conclusion(
        summary.failed,
        summary.coverage.as_ref(),
        summary.mutation.as_ref(),
        has_execution_error,
    );
    if expected_conclusion != summary.conclusion {
        findings.push(format!(
            "conclusion 一致性失败: 计数 / 覆盖 / 突变面重算结论为 \"{}\", 报告记载 \"{}\"",
            expected_conclusion.as_str(),
            summary.conclusion.as_str()
        ));
    }
    findings
}

/// 诊断树确定性分支：按 CLI executor 定义 4a → 4b → 4c → 4d/4e → 4f 顺序
/// 产出 findings（全绿零误报——4e 产单条正向 finding）。
pub fn diagnose_findings(summary: &SummaryReport) -> Vec<String> {
    let mut findings = Vec::new();

    // 4a: conclusion=error → execution_error 归因
    if summary.conclusion == Conclusion::Error {
        let exec_errors: Vec<_> = summary
            .problems
            .iter()
            .filter(|problem| problem.problem_type == ProblemType::ExecutionError)
            .collect();
        if exec_errors.is_empty() {
            findings.push("结论为 error 但 problems 未列 execution_error 条目".to_owned());
        } else {
            for problem in exec_errors.iter().take(CLUSTER_FRAMEWORK_LIMIT) {
                findings.push(format!(
                    "执行错误归因 [{}]: {}",
                    problem.framework, problem.message
                ));
            }
        }
        let failures = summary
            .problems
            .iter()
            .filter(|problem| problem.problem_type != ProblemType::ExecutionError)
            .count();
        if failures > 0 {
            findings.push(format!(
                "error 结论优先: 另有 {failures} 项测试 / 覆盖问题随报告留档"
            ));
        }
    }

    // 4b: 失败聚类措辞（按框架归并计数；单点疑似 flaky、多点疑似同模块回归）
    if summary.failed > 0 {
        let mut by_framework: Vec<(String, usize)> = Vec::new();
        for problem in summary
            .problems
            .iter()
            .filter(|problem| problem.problem_type == ProblemType::TestFailure)
        {
            match by_framework
                .iter_mut()
                .find(|(fw, _)| fw == &problem.framework)
            {
                Some((_, count)) => *count += 1,
                None => by_framework.push((problem.framework.clone(), 1)),
            }
        }
        for (framework, count) in by_framework.iter().take(CLUSTER_FRAMEWORK_LIMIT) {
            let pattern = if *count == 1 {
                "单点失败（疑似 flaky 用例或孤立回归）"
            } else {
                "聚类失败（疑似同模块回归或共同假设变化）"
            };
            findings.push(format!(
                "「{framework}」{count} 项测试失败——{pattern}; 明细见子报告 error_cases"
            ));
        }
    }

    // 4c: 覆盖阈值比对（null 维度感知跳过 + override 组缺口）
    if let Some(coverage) = &summary.coverage {
        if !coverage.pass {
            findings.extend(threshold_findings(&coverage.measured, &coverage.thresholds));
            for override_entry in coverage.overrides.iter().flatten() {
                if !override_entry.pass {
                    findings.push(format!(
                        "覆盖 override \"{}\" 未达阈: {}（命中 {} 文件, 逐文件达阈 {}）",
                        override_entry.glob,
                        threshold_brief(&override_entry.measured, &override_entry.thresholds),
                        override_entry.file_count,
                        override_entry.passed_count
                    ));
                }
            }
        }
    }

    // 4d / 4e: pass 态覆盖面
    if summary.conclusion == Conclusion::Pass {
        match &summary.coverage {
            None => findings
                .push("全部框架未产出覆盖数据（未配置采集或命令未及覆盖阶段即结束）".to_owned()),
            Some(coverage) if coverage.pass => {
                findings.push("全部测试通过且覆盖率达阈值".to_owned());
            }
            _ => {}
        }
    }

    // 4f: 突变缺口（V1 生产恒 None，corpus 含块样本触达）
    if let Some(mutation) = &summary.mutation {
        if !mutation.pass {
            findings.push(format!(
                "突变得分 {:.1}% < 阈值 {}%（survived {} / noCoverage {}）",
                mutation.score,
                mutation.threshold,
                mutation.measured.survived,
                mutation.measured.no_coverage
            ));
        }
    }
    findings
}

/// 阈值比对 findings（逐维度，null 维度跳过）。
fn threshold_findings(
    measured: &CoverageMeasured,
    thresholds: &crate::model::CoverageThresholds,
) -> Vec<String> {
    let mut findings = Vec::new();
    for (name, value, threshold) in [
        ("行覆盖", measured.lines, thresholds.lines),
        ("分支覆盖", measured.branches, thresholds.branches),
        ("函数覆盖", measured.functions, thresholds.functions),
    ] {
        if let Some(value) = value.filter(|value| *value < threshold) {
            findings.push(format!("{name} {value:.1}% < 阈值 {threshold}%"));
        }
    }
    findings
}

/// 阈值缺口人读摘要（override 行内嵌）。
fn threshold_brief(
    measured: &CoverageMeasured,
    thresholds: &crate::model::CoverageThresholds,
) -> String {
    let parts = threshold_findings(measured, thresholds);
    if parts.is_empty() {
        "逐文件达阈数不足".to_owned()
    } else {
        parts.join(", ")
    }
}
