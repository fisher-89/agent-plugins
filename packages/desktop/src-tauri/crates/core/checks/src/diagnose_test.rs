use time::OffsetDateTime;

use config::{MutationConfig, TestFramework};

use crate::aggregate::build_summary_report;
use crate::diagnose::{check_integrity, diagnose_findings};
use crate::model::{
    CaseSummary, Conclusion, CoverageBlock, CoverageMeasured, CoverageThresholds, PlanIndexEntry,
    Problem, ProblemType, SourceFileEntry, SubReport, SummaryReport,
};

// ---------------------------------------------------------------------------
// 装置：一致 summary 组装（真实聚合产出 → 完整性基准）
// ---------------------------------------------------------------------------

fn config_thresholds(lines: f64, branches: f64, functions: f64) -> config::CoverageThresholds {
    config::CoverageThresholds {
        lines,
        branches,
        functions,
    }
}

fn suite(root: &str) -> config::TestSuite {
    config::TestSuite {
        root: root.to_owned(),
        framework: TestFramework::VitePlus,
        cwd: ".".to_owned(),
        config: None,
        includes: None,
        excludes: None,
        coverage: config_thresholds(80.0, 70.0, 75.0),
        mutation: MutationConfig {
            cwd: None,
            score: 70.0,
        },
    }
}

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
    let suites = vec![suite("src/app")];
    build_summary_report(sub_reports, &suites, "cmd", OffsetDateTime::now_utc())
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
    let summary = consistent_summary(&[sub.clone()]);

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
    let findings = check_integrity(&summary, &[sub.clone()]);
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
fn hand_summary(
    conclusion: Conclusion,
    failed: u64,
    problems: Vec<Problem>,
    coverage: Option<CoverageBlock>,
) -> SummaryReport {
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
        coverage,
        mutation: None,
        plans: Vec::new(),
        findings: None,
    }
}

#[test]
fn 覆盖未达阈_阈值比对findings携维度与数值() {
    let coverage = CoverageBlock {
        pass: false,
        measured: CoverageMeasured {
            lines: Some(62.4),
            branches: Some(50.0),
            functions: None,
        },
        thresholds: CoverageThresholds {
            lines: 80.0,
            branches: 70.0,
            functions: 75.0,
        },
        overrides: Some(vec![crate::model::CoverageOverride {
            glob: "src/app".to_owned(),
            thresholds: CoverageThresholds {
                lines: 85.0,
                branches: 75.0,
                functions: 90.0,
            },
            measured: CoverageMeasured {
                lines: Some(62.4),
                branches: Some(50.0),
                functions: None,
            },
            pass: false,
            file_count: 3,
            passed_count: 0,
        }]),
    };
    let summary = hand_summary(Conclusion::Fail, 0, Vec::new(), Some(coverage));

    let findings = diagnose_findings(&summary);
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("行覆盖 62.4% < 阈值 80%")),
        "阈值比对措辞携维度与数值（CLI 定义同型），实际: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("分支覆盖 50.0% < 阈值 70%")),
        "分支维度同型比对，实际: {findings:?}"
    );
    assert!(
        !findings.iter().any(|finding| finding.contains("函数覆盖")),
        "null 维度感知跳过（functions None 不产比对行）"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("覆盖 override") && finding.contains("src/app")),
        "override 组缺口随行（CLI 定义 4c 同型），实际: {findings:?}"
    );
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
        None,
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
    let summary = hand_summary(Conclusion::Error, 0, Vec::new(), None);
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
        None,
    );
    let findings = diagnose_findings(&multi);
    assert!(
        findings.iter().any(|finding| finding
            .contains("「vitest」3 项测试失败——聚类失败（疑似同模块回归或共同假设变化）")),
        "多点失败聚类措辞（CLI 定义 4b 同型），实际: {findings:?}"
    );

    // 单点失败 → flaky 疑似措辞
    let single = hand_summary(Conclusion::Fail, 1, vec![failure("a")], None);
    let findings = diagnose_findings(&single);
    assert!(
        findings.iter().any(|finding| finding
            .contains("「vitest」1 项测试失败——单点失败（疑似 flaky 用例或孤立回归）")),
        "单点失败措辞，实际: {findings:?}"
    );
}

#[test]
fn coverage维度null_null感知跳过不产覆盖findings() {
    // coverage 块 pass=false 但实测全 None（采集失败形态）→ 无阈值比对行
    let coverage = CoverageBlock {
        pass: false,
        measured: CoverageMeasured {
            lines: None,
            branches: None,
            functions: None,
        },
        thresholds: CoverageThresholds {
            lines: 80.0,
            branches: 70.0,
            functions: 75.0,
        },
        overrides: None,
    };
    let summary = hand_summary(Conclusion::Fail, 0, Vec::new(), Some(coverage));

    let findings = diagnose_findings(&summary);
    assert!(
        findings.is_empty(),
        "null 维度全跳过且无失败无 error → 零覆盖 findings（不产假缺口），实际: {findings:?}"
    );
}

#[test]
fn 全绿summary零误报_findings仅单条正向() {
    // 全绿 + 覆盖达标：CLI 定义 4e 产单条正向 finding——零失败 / 阈值 / 归因
    // 类误报（负向 findings 为空集）
    let coverage = CoverageBlock {
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
    };
    let summary = hand_summary(Conclusion::Pass, 0, Vec::new(), Some(coverage));

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
