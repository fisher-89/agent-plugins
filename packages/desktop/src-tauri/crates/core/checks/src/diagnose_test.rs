use time::OffsetDateTime;

use crate::aggregate::build_summary_report;
use crate::diagnose::{check_integrity, diagnose_findings};
use crate::model::{
    CaseSummary, Conclusion, CoverageBlock, CoverageMeasured, CoverageThresholds, PlanIndexEntry,
    Problem, ProblemType, SourceFileEntry, SubReport, SummaryReport,
};

// ---------------------------------------------------------------------------
// 装置：一致 summary 组装（真实聚合产出 → 完整性基准）
// ---------------------------------------------------------------------------

fn file_entry(file: &str, total: u64, covered: u64) -> SourceFileEntry {
    SourceFileEntry {
        file: file.to_owned(),
        coverage: Some(crate::model::FileCoverageEntry {
            lines: covered as f64 / total as f64 * 100.0,
            branches: None,
            functions: None,
            total_lines: Some(total),
            covered_lines: Some(covered),
            total_branches: None,
            covered_branches: None,
            total_functions: None,
            covered_functions: None,
        }),
    }
}

/// 一致子报告（覆盖达标、计数绿态）。
fn green_sub_report(root: &str, framework: &str, counts: CaseSummary) -> SubReport {
    SubReport {
        framework: framework.to_owned(),
        root: root.to_owned(),
        timestamp: "2026-10-06T08:00:00.000Z".to_owned(),
        exit_code: 0,
        duration_ms: 100.0,
        summary: counts,
        error_cases: Vec::new(),
        test_files: Vec::new(),
        source_files: vec![file_entry("src/app/main.rs", 20, 18)],
        coverage: Some(CoverageBlock {
            pass: true,
            measured: CoverageMeasured {
                lines: Some(90.0),
                branches: Some(80.0),
                functions: Some(85.0),
            },
            thresholds: CoverageThresholds {
                lines: 80.0,
                branches: 70.0,
                functions: 75.0,
            },
            overrides: None,
        }),
        mutation: None,
        findings: None,
    }
}

/// 一致 summary：由 build_summary_report 真实组装（聚合 → 完整性自洽基准）。
fn consistent_summary(sub_reports: &[SubReport]) -> SummaryReport {
    build_summary_report(sub_reports, "cmd", OffsetDateTime::now_utc())
}

// ---------------------------------------------------------------------------
// check_integrity（完整性 checklist）
// ---------------------------------------------------------------------------

#[test]
fn 一致summary与子报告全集_三查全过空findings() {
    let sub = green_sub_report(
        "src/app",
        "vite-plus",
        CaseSummary {
            total: 2,
            passed: 2,
            failed: 0,
            skipped: 0,
        },
    );
    let summary = consistent_summary(std::slice::from_ref(&sub));

    let findings = check_integrity(&summary, &[sub]);
    assert!(
        findings.is_empty(),
        "计数对账 / 报告齐全 / conclusion 合法三查全过，实际: {findings:?}"
    );
}

#[test]
fn 聚合计数对账失败记因() {
    let sub = green_sub_report(
        "src/app",
        "vite-plus",
        CaseSummary {
            total: 2,
            passed: 2,
            failed: 0,
            skipped: 0,
        },
    );
    let mut summary = consistent_summary(std::slice::from_ref(&sub));
    // 篡改聚合计数（漂移形态：summary 记 3，子报告之和 2）
    summary.total = 3;

    let findings = check_integrity(&summary, &[sub]);
    assert_eq!(findings.len(), 1, "仅计数对账违例");
    assert!(
        findings[0].contains("聚合计数对账失败")
            && findings[0].contains("3")
            && findings[0].contains("2"),
        "对账违例面可读（两侧数值俱陈），实际: {findings:?}"
    );
}

#[test]
fn plan报告缺失与索引缺失双向记因() {
    let sub = green_sub_report(
        "src/app",
        "vite-plus",
        CaseSummary {
            total: 1,
            passed: 1,
            failed: 0,
            skipped: 0,
        },
    );
    let mut summary = consistent_summary(std::slice::from_ref(&sub));

    // summary.plans 引用缺失子报告（ghost 条目无对应 report.json）
    summary.plans = vec![PlanIndexEntry {
        id: "ghost_plan".to_owned(),
        framework: "vite-plus".to_owned(),
        root: "ghost".to_owned(),
        path: "reports/test/ghost_plan".to_owned(),
    }];
    let findings = check_integrity(&summary, std::slice::from_ref(&sub));
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("plan 报告缺失") && finding.contains("ghost_plan")),
        "plans 引用缺失子报告记因，实际: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("plan 索引缺失")),
        "子报告未入索引记因（双向齐全性），实际: {findings:?}"
    );
}

#[test]
fn conclusion与计数不一致记因() {
    let sub = green_sub_report(
        "src/app",
        "vite-plus",
        CaseSummary {
            total: 2,
            passed: 2,
            failed: 0,
            skipped: 0,
        },
    );
    let mut summary = consistent_summary(std::slice::from_ref(&sub));
    // 全绿计数却 fail → conclusion 一致性违例
    summary.conclusion = Conclusion::Fail;

    let findings = check_integrity(&summary, &[sub]);
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("conclusion 一致性失败")
                && finding.contains("\"fail\"")
                && finding.contains("\"pass\"")),
        "重算结论与记载结论俱陈，实际: {findings:?}"
    );
}

#[test]
fn 零子报告零plans空summary不panic且结论明确() {
    // 一致空 summary（绿态空集）：三查全过零 findings
    let summary = consistent_summary(&[]);
    assert_eq!(summary.conclusion, Conclusion::Pass);
    let findings = check_integrity(&summary, &[]);
    assert!(
        findings.is_empty(),
        "空集自洽 → 空 findings，实际: {findings:?}"
    );

    // 空集 + 漂移 conclusion → 记因不 panic
    let mut drifted = summary;
    drifted.conclusion = Conclusion::Fail;
    let findings = check_integrity(&drifted, &[]);
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("conclusion 一致性失败")),
        "空集结论违例仍被 checklist 捕获，实际: {findings:?}"
    );
}

// ---------------------------------------------------------------------------
// diagnose_findings（诊断树确定性分支）
// ---------------------------------------------------------------------------

/// 汇总内存构造（跳过聚合组装——诊断树分支的定向输入面）。
fn hand_summary(conclusion: Conclusion, failed: u64, problems: Vec<Problem>) -> SummaryReport {
    SummaryReport {
        phase: "test-execution".to_owned(),
        command: "cmd".to_owned(),
        timestamp: "2026-10-06T08:00:00.000Z".to_owned(),
        duration_seconds: 1.0,
        total: failed + 2,
        passed: 2,
        failed,
        skipped: 0,
        conclusion,
        problems,
        plans: Vec::new(),
        findings: None,
    }
}

#[test]
fn execution_error问题在场_归因findings() {
    let summary = hand_summary(
        Conclusion::Error,
        0,
        vec![Problem {
            framework: "vite-plus".to_owned(),
            problem_type: ProblemType::ExecutionError,
            message: "Exit code 1".to_owned(),
        }],
    );

    let findings = diagnose_findings(&summary);
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("执行错误归因 [vite-plus]: Exit code 1")),
        "error 结论归因 problems 中的 execution_error 条目（CLI 定义 4a），实际: {findings:?}"
    );
}

#[test]
fn execution_error结论但problems未列_漂移记因() {
    // 结论 error 但 problems 无 execution_error 条目 → 漂移显式记因
    let summary = hand_summary(Conclusion::Error, 0, Vec::new());
    let findings = diagnose_findings(&summary);
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("结论为 error 但 problems 未列 execution_error 条目")),
        "漂移面可读，实际: {findings:?}"
    );
}

#[test]
fn 多文件失败用例_失败聚类措辞() {
    let failure = |name: &str| Problem {
        framework: "vitest".to_owned(),
        problem_type: ProblemType::TestFailure,
        message: format!("Test \"{name}\" failed: boom"),
    };
    // 多点失败（同框架 3 项）→ 聚类措辞
    let multi = hand_summary(
        Conclusion::Fail,
        3,
        vec![failure("a"), failure("b"), failure("c")],
    );
    let findings = diagnose_findings(&multi);
    assert!(
        findings.iter().any(|finding| finding
            .contains("「vitest」3 项测试失败——聚类失败（疑似同模块回归或共同假设变化）")),
        "多点失败聚类措辞（CLI 定义 4b 同型），实际: {findings:?}"
    );

    // 单点失败 → flaky 疑似措辞
    let single = hand_summary(Conclusion::Fail, 1, vec![failure("a")]);
    let findings = diagnose_findings(&single);
    assert!(
        findings.iter().any(|finding| finding
            .contains("「vitest」1 项测试失败——单点失败（疑似 flaky 用例或孤立回归）")),
        "单点失败措辞，实际: {findings:?}"
    );
}

#[test]
fn 全绿summary零误报_findings仅单条正向() {
    // 全绿 + 覆盖达标：CLI 定义 4e 产单条正向 finding——零失败 / 阈值 / 归因
    // 类误报（负向 findings 为空集）
    let summary = hand_summary(Conclusion::Pass, 0, Vec::new());

    let findings = diagnose_findings(&summary);
    assert_eq!(
        findings,
        vec!["全部测试通过且覆盖率达阈值".to_owned()],
        "全绿零误报：仅 CLI 定义 4e 的单条正向 finding"
    );
    assert!(
        !findings.iter().any(|finding| finding.contains("失败")
            || finding.contains("错误")
            || finding.contains("未达")),
        "负向 finding 零误报"
    );
}

// ---------------------------------------------------------------------------
// check_integrity ③（以子报告布尔 + problems 的 execution_error 重算，复用
// `determine_conclusion`；覆盖判定单一真值源 = 子报告 `coverage.pass`）
// ---------------------------------------------------------------------------

/// 正向：缺采集 fail 报告自洽（AC-5 / AC-8）——绿跑子报告 `coverage = None`
/// → summary `conclusion = fail` + 一条 `coverage_failure` → ③ 重算同为 fail、
/// 零违例（新判据不放宽既有结论）。
#[test]
fn 缺采集fail报告自洽零违例() {
    let mut sub = green_sub_report(
        "crates",
        "rust",
        CaseSummary {
            total: 3,
            passed: 3,
            failed: 0,
            skipped: 0,
        },
    );
    sub.coverage = None;
    let summary = consistent_summary(std::slice::from_ref(&sub));

    assert_eq!(summary.conclusion, Conclusion::Fail);
    assert_eq!(summary.problems.len(), 1, "恰一条覆盖归因");
    let findings = check_integrity(&summary, &[sub]);
    assert!(
        findings.is_empty(),
        "重算与记载一致 → 零违例，实际: {findings:?}"
    );
}

/// 正向（AC-11）：突变块不入重算——子报告 `mutation = Some(pass: false)` +
/// 覆盖全 pass + `failed = 0` + summary `conclusion = pass` → 零违例。
#[test]
fn 突变块不入重算() {
    let mut sub = green_sub_report(
        "crates",
        "rust",
        CaseSummary {
            total: 2,
            passed: 2,
            failed: 0,
            skipped: 0,
        },
    );
    sub.mutation = Some(crate::model::MutationBlock {
        pass: false,
        score: 55.0,
        threshold: 70.0,
        measured: crate::model::MutationMeasured {
            killed: 11,
            survived: 7,
            timeout: 2,
            no_coverage: 3,
            compile_error: 1,
            runtime_error: 1,
            ignored: 2,
            total: 27,
            detected: 13,
            undetected: 10,
        },
        overrides: None,
    });
    let summary = consistent_summary(std::slice::from_ref(&sub));
    assert_eq!(summary.conclusion, Conclusion::Pass, "突变不参与汇总结论");

    let findings = check_integrity(&summary, &[sub]);
    assert!(
        findings.is_empty(),
        "突变块零贡献于重算，实际: {findings:?}"
    );
}

/// 异常（AC-8 核心行）：篡改子报告 `coverage.pass`——子报告块自判未达而
/// summary 记 pass → 恰一条违例，文案含「conclusion 一致性失败」且两侧
/// `"fail"` / `"pass"` 俱陈。
#[test]
fn 篡改子报告覆盖布尔记因() {
    let sub = green_sub_report(
        "src/app",
        "vite-plus",
        CaseSummary {
            total: 2,
            passed: 2,
            failed: 0,
            skipped: 0,
        },
    );
    let summary = consistent_summary(std::slice::from_ref(&sub));
    assert_eq!(summary.conclusion, Conclusion::Pass);

    let mut tampered = sub;
    let block = tampered.coverage.as_mut().expect("覆盖块在场");
    block.pass = false;

    let findings = check_integrity(&summary, &[tampered]);
    let violations: Vec<&String> = findings
        .iter()
        .filter(|finding| finding.contains("conclusion 一致性失败"))
        .collect();
    assert_eq!(violations.len(), 1, "恰一条结论违例，实际: {findings:?}");
    assert!(
        violations[0].contains("\"fail\"") && violations[0].contains("\"pass\""),
        "重算结论与记载结论俱陈: {}",
        violations[0]
    );
}

/// 异常：缺采集子报告对记载 pass——子报告 `coverage = None`（绿跑）而 summary
/// 记 `conclusion = pass` → 违例（重算 fail）。
#[test]
fn 缺采集子报告对记载pass记因() {
    let mut sub = green_sub_report(
        "crates",
        "rust",
        CaseSummary {
            total: 1,
            passed: 1,
            failed: 0,
            skipped: 0,
        },
    );
    sub.coverage = None;
    let mut summary = consistent_summary(std::slice::from_ref(&sub));
    assert_eq!(summary.conclusion, Conclusion::Fail);
    summary.conclusion = Conclusion::Pass;

    let findings = check_integrity(&summary, &[sub]);
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("conclusion 一致性失败")),
        "缺采集面重算 fail 与记载 pass 冲突，实际: {findings:?}"
    );
}

/// 异常（2 组合）：`execution_error` 优先级在重算面成立——`problems` 含
/// `execution_error` + 覆盖全 pass + `failed = 0` → 重算 error；summary 记
/// error → 零违例；记 fail → 违例。
#[test]
fn execution_error优先级在重算面成立() {
    let mut sub = green_sub_report(
        "src/app",
        "vite-plus",
        CaseSummary {
            total: 1,
            passed: 1,
            failed: 0,
            skipped: 0,
        },
    );
    sub.findings = Some(vec!["工件不可解析降级留痕".to_owned()]);
    let summary = consistent_summary(std::slice::from_ref(&sub));
    assert_eq!(summary.conclusion, Conclusion::Error, "error 重算定型");
    assert!(
        summary
            .problems
            .iter()
            .any(|problem| problem.problem_type == ProblemType::ExecutionError),
        "problems 携 execution_error: {:?}",
        summary.problems
    );
    let findings = check_integrity(&summary, &[sub.clone()]);
    assert!(findings.is_empty(), "记载一致 → 零违例: {findings:?}");

    let mut drifted = summary;
    drifted.conclusion = Conclusion::Fail;
    let findings = check_integrity(&drifted, &[sub]);
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("conclusion 一致性失败")),
        "优先级降格即违例，实际: {findings:?}"
    );
}

/// 边界：超大列表——50 条子报告 + 1 条 `coverage = None` + summary
/// `conclusion = fail` → 零违例（重算对 50 条遍历不误报）。
#[test]
fn 超大列表一条缺采集仍自洽() {
    let mut sub_reports: Vec<SubReport> = (0..49)
        .map(|index| {
            green_sub_report(
                &format!("suite{index}"),
                "rust",
                CaseSummary {
                    total: 1,
                    passed: 1,
                    failed: 0,
                    skipped: 0,
                },
            )
        })
        .collect();
    let mut missing = green_sub_report(
        "missing",
        "vite-plus",
        CaseSummary {
            total: 1,
            passed: 1,
            failed: 0,
            skipped: 0,
        },
    );
    missing.coverage = None;
    sub_reports.push(missing);

    let summary = consistent_summary(&sub_reports);
    assert_eq!(summary.conclusion, Conclusion::Fail);
    let findings = check_integrity(&summary, &sub_reports);
    assert!(findings.is_empty(), "50 条遍历零违例: {findings:?}");
}

// ---------------------------------------------------------------------------
// diagnose_findings 4c（逐 suite 覆盖归因条目展开）与并存面
// ---------------------------------------------------------------------------

/// 正向（AC-4 / D5）：4c 逐 suite 展开——`problems` 的 `coverage_failure` 条目
/// 逐字展开为 `覆盖判定未通过 [framework]: message`。
#[test]
fn 覆盖归因逐suite展开() {
    let summary = hand_summary(
        Conclusion::Fail,
        0,
        vec![Problem {
            framework: "rust".to_owned(),
            problem_type: ProblemType::CoverageFailure,
            message: "Coverage below threshold: functions 87.6% < 90%".to_owned(),
        }],
    );
    let findings = diagnose_findings(&summary);
    assert!(
        findings.contains(
            &"覆盖判定未通过 [rust]: Coverage below threshold: functions 87.6% < 90%".to_owned()
        ),
        "逐 suite 覆盖归因展开，实际: {findings:?}"
    );
}

/// 异常：4c 零假缺口——`conclusion = fail` + `problems` 无 `coverage_failure`
/// 条目 → 零 4c 行（不再由 summary 覆盖块取值重算数值）。
#[test]
fn 覆盖面无假缺口() {
    let summary = hand_summary(
        Conclusion::Fail,
        1,
        vec![Problem {
            framework: "vitest".to_owned(),
            problem_type: ProblemType::TestFailure,
            message: "Test \"a\" failed: boom".to_owned(),
        }],
    );
    let findings = diagnose_findings(&summary);
    assert!(
        !findings
            .iter()
            .any(|finding| finding.starts_with("覆盖判定未通过")),
        "无覆盖归因条目即零 4c 行，实际: {findings:?}"
    );
}

/// 异常：4c 条数上限——12 条 `coverage_failure`（跨多 framework）→ 4c 行恰
/// 10 条（`CLUSTER_FRAMEWORK_LIMIT`）。
#[test]
fn 覆盖归因条数上限() {
    let problems: Vec<Problem> = (0..12)
        .map(|index| Problem {
            framework: format!("fw{index}"),
            problem_type: ProblemType::CoverageFailure,
            message: format!("Coverage below threshold: lines {index}.0% < 80%"),
        })
        .collect();
    let summary = hand_summary(Conclusion::Fail, 0, problems);

    let findings = diagnose_findings(&summary);
    let coverage_lines = findings
        .iter()
        .filter(|finding| finding.starts_with("覆盖判定未通过"))
        .count();
    assert_eq!(coverage_lines, 10, "4c 条数上限 10，实际: {findings:?}");
}

/// 边界（R-10）：并存互不顶替——`problems = [execution_error,
/// coverage_failure]` + `conclusion = error` → 4a 归因行与 4c 行同现，且 4a
/// 的「另有 N 项测试 / 覆盖问题随报告留档」计数含该覆盖条目。
#[test]
fn 执行错误与覆盖归因并存() {
    let summary = hand_summary(
        Conclusion::Error,
        0,
        vec![
            Problem {
                framework: "vite-plus".to_owned(),
                problem_type: ProblemType::ExecutionError,
                message: "Exit code 1".to_owned(),
            },
            Problem {
                framework: "rust".to_owned(),
                problem_type: ProblemType::CoverageFailure,
                message: "覆盖率采集缺失（退出码 0）：工件缺失 / 解析失败".to_owned(),
            },
        ],
    );
    let findings = diagnose_findings(&summary);

    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("执行错误归因 [vite-plus]: Exit code 1")),
        "4a 归因行在场，实际: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding == "error 结论优先: 另有 1 项测试 / 覆盖问题随报告留档"),
        "并存计数含覆盖条目，实际: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.starts_with("覆盖判定未通过 [rust]:")),
        "4c 行与 4a 同现，实际: {findings:?}"
    );
}

/// 边界（2 组合）：空输入——`problems = []` / `failed = 0` / `conclusion =
/// pass` → 仅 4e 单条；同空输入 `conclusion = fail` → 零 findings（无块可读时
/// 的既有空集语义）。
#[test]
fn 空输入两形态() {
    let pass = hand_summary(Conclusion::Pass, 0, Vec::new());
    assert_eq!(
        diagnose_findings(&pass),
        vec!["全部测试通过且覆盖率达阈值".to_owned()]
    );

    let fail = hand_summary(Conclusion::Fail, 0, Vec::new());
    assert!(diagnose_findings(&fail).is_empty(), "无归因条目可读 → 空集");
}
