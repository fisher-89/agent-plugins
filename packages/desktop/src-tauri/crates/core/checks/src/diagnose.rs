use crate::aggregate::{derive_plan_id, determine_conclusion};
use crate::model::{Conclusion, ProblemType, SubReport, SummaryReport};

/// 失败问题归并的单框架上限（措辞面收敛，明细在子报告 error_cases）。
const CLUSTER_FRAMEWORK_LIMIT: usize = 10;

/// 完整性 checklist：三项全过 → 空集（无违例）。
///
/// ① 聚合计数对账——summary 四计数与子报告计数之和解恒等；
/// ② plan 报告齐全性——`plans[]` 条目与子报告按 planId 双向一一对应；
/// ③ conclusion 一致性——聚合结论与「计数 + 逐 suite 覆盖判定布尔 +
///    execution_error 问题面」重算结果恒等（判据与门禁同一个
///    [`determine_conclusion`]，MUST NOT 就地另写一套）。
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

    // ③ conclusion 一致性（以子报告布尔重算比对）
    let has_execution_error = summary
        .problems
        .iter()
        .any(|problem| problem.problem_type == ProblemType::ExecutionError);
    let expected_conclusion =
        determine_conclusion(summary.failed, sub_reports, has_execution_error);
    if expected_conclusion != summary.conclusion {
        findings.push(format!(
            "conclusion 一致性失败: 计数 / 逐 suite 覆盖判定面重算结论为 \"{}\", 报告记载 \"{}\"",
            expected_conclusion.as_str(),
            summary.conclusion.as_str()
        ));
    }
    findings
}

/// 诊断树确定性分支：按 CLI executor 定义 4a → 4b → 4c → 4e 顺序产出 findings
/// （全绿零误报——4e 产单条正向 finding）。
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

    // 4c: 覆盖判定未通过（逐 suite 归因条目已在 problems 内）
    for problem in summary
        .problems
        .iter()
        .filter(|problem| problem.problem_type == ProblemType::CoverageFailure)
        .take(CLUSTER_FRAMEWORK_LIMIT)
    {
        findings.push(format!(
            "覆盖判定未通过 [{}]: {}",
            problem.framework, problem.message
        ));
    }

    // 4e: pass 态正向锚（缺席即 fail ⇒ pass 必为覆盖全达标）
    if summary.conclusion == Conclusion::Pass {
        findings.push("全部测试通过且覆盖率达阈值".to_owned());
    }
    findings
}
