use time::OffsetDateTime;

use config::{MutationConfig, TestFramework, TestSuite};

use crate::aggregate::{
    aggregate_coverage, build_summary_report, coverage_meets_thresholds, derive_plan_id,
    determine_conclusion, in_suite_scope,
};
use crate::model::{
    CaseSummary, Conclusion, CoverageBlock, CoverageMeasured, CoverageThresholds, MutationBlock,
    MutationMeasured, SourceFileEntry, SubReport, TestCaseResult, TestCaseStatus,
};

// ---------------------------------------------------------------------------
// 装置：模型 fixture 构造器
// ---------------------------------------------------------------------------

fn thresholds(lines: f64, branches: f64, functions: f64) -> CoverageThresholds {
    CoverageThresholds {
        lines,
        branches,
        functions,
    }
}

fn measured(lines: Option<f64>, branches: Option<f64>, functions: Option<f64>) -> CoverageMeasured {
    CoverageMeasured {
        lines,
        branches,
        functions,
    }
}

fn block(pass: bool, measured: CoverageMeasured, thresholds: CoverageThresholds) -> CoverageBlock {
    CoverageBlock {
        pass,
        measured,
        thresholds,
        overrides: None,
    }
}

fn config_thresholds(lines: f64, branches: f64, functions: f64) -> config::CoverageThresholds {
    config::CoverageThresholds {
        lines,
        branches,
        functions,
    }
}

/// config::TestSuite 内存构造（framework 无 Copy，逐调用点显式构造枚举值）。
#[allow(clippy::too_many_arguments)]
fn suite(
    root: &str,
    framework: TestFramework,
    coverage: config::CoverageThresholds,
    includes: Option<Vec<String>>,
    excludes: Option<Vec<String>>,
) -> TestSuite {
    TestSuite {
        root: root.to_owned(),
        framework,
        cwd: ".".to_owned(),
        config: None,
        includes,
        excludes,
        coverage,
        mutation: MutationConfig {
            cwd: None,
            score: 70.0,
        },
    }
}

/// 逐文件覆盖原始计数条目（lines / branches / functions 三维 total+covered）。
#[allow(clippy::too_many_arguments)]
fn entry(
    file: &str,
    total_lines: u64,
    covered_lines: u64,
    total_branches: Option<u64>,
    covered_branches: Option<u64>,
    total_functions: Option<u64>,
    covered_functions: Option<u64>,
) -> SourceFileEntry {
    SourceFileEntry {
        file: file.to_owned(),
        coverage: Some(crate::model::FileCoverageEntry {
            lines: if total_lines > 0 {
                round_two(covered_lines as f64 / total_lines as f64 * 100.0)
            } else {
                0.0
            },
            branches: None,
            functions: None,
            total_lines: Some(total_lines),
            covered_lines: Some(covered_lines),
            total_branches,
            covered_branches,
            total_functions,
            covered_functions,
        }),
    }
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// 子报告内存构造（默认绿态：退出码 0、计数一致、无 error_cases）。
fn sub_report(
    root: &str,
    framework: &str,
    summary: CaseSummary,
    source_files: Vec<SourceFileEntry>,
    coverage: Option<CoverageBlock>,
) -> SubReport {
    SubReport {
        framework: framework.to_owned(),
        root: root.to_owned(),
        timestamp: "2026-10-06T08:00:00.000Z".to_owned(),
        exit_code: 0,
        duration_ms: 100.0,
        summary,
        error_cases: Vec::new(),
        test_files: Vec::new(),
        source_files,
        coverage,
        mutation: None,
        findings: None,
    }
}

fn counts(total: u64, passed: u64, failed: u64, skipped: u64) -> CaseSummary {
    CaseSummary {
        total,
        passed,
        failed,
        skipped,
    }
}

// ---------------------------------------------------------------------------
// determine_conclusion（CLI determineConclusion 同语义）
// ---------------------------------------------------------------------------

#[test]
fn 结论判定_全绿pass() {
    let coverage = block(
        true,
        measured(Some(90.0), Some(80.0), Some(85.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    let conclusion = determine_conclusion(0, Some(&coverage), None, false);
    assert_eq!(conclusion, Conclusion::Pass);
}

#[test]
fn 结论判定_failed大于零fail() {
    // error 不在场时 fail 优先于 pass
    let coverage = block(
        true,
        measured(Some(90.0), Some(80.0), Some(85.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    let conclusion = determine_conclusion(1, Some(&coverage), None, false);
    assert_eq!(conclusion, Conclusion::Fail);

    // failed>0 且覆盖也未达 → 仍 fail（同档不升级）
    let failing_coverage = block(
        false,
        measured(Some(50.0), Some(40.0), Some(30.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    let conclusion = determine_conclusion(3, Some(&failing_coverage), None, false);
    assert_eq!(conclusion, Conclusion::Fail);
}

#[test]
fn 结论判定_覆盖未达阈fail() {
    // 任一维度低于阈值 → fail（failed=0 不豁免覆盖缺口）
    let coverage = block(
        false,
        measured(Some(79.9), Some(80.0), Some(85.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    let conclusion = determine_conclusion(0, Some(&coverage), None, false);
    assert_eq!(conclusion, Conclusion::Fail);
}

#[test]
fn 结论判定_execution_error优先级最高() {
    // error 优先级最高——即使 failed>0 且覆盖未达也判 error（CLI determineConclusion 同序）
    let failing_coverage = block(
        false,
        measured(Some(10.0), Some(10.0), Some(10.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    let conclusion = determine_conclusion(5, Some(&failing_coverage), None, true);
    assert_eq!(conclusion, Conclusion::Error);

    // 全绿面 + execution_error 仍 error
    let passing_coverage = block(
        true,
        measured(Some(99.0), Some(99.0), Some(99.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    assert_eq!(
        determine_conclusion(0, Some(&passing_coverage), None, true),
        Conclusion::Error
    );
}

#[test]
fn 结论判定_覆盖恰等于阈值达标() {
    // ≥ 边界不误判未达：三维恰等于阈值 → 达标 → pass
    let coverage = block(
        true,
        measured(Some(80.0), Some(70.0), Some(75.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    assert_eq!(
        determine_conclusion(0, Some(&coverage), None, false),
        Conclusion::Pass
    );
    // 判定函数直接对齐（coverage_meets_thresholds 同边界）
    assert!(coverage_meets_thresholds(
        &measured(Some(80.0), Some(70.0), Some(75.0)),
        &thresholds(80.0, 70.0, 75.0)
    ));
    // 略低一分即未达
    assert!(!coverage_meets_thresholds(
        &measured(Some(79.999), Some(70.0), Some(75.0)),
        &thresholds(80.0, 70.0, 75.0)
    ));
}

#[test]
fn 结论判定_null维度感知跳过不误fail() {
    // coverage=None 且 mutation=None（null 维度感知跳过）→ 不因缺块误 fail
    assert_eq!(determine_conclusion(0, None, None, false), Conclusion::Pass);

    // 部分维度 None：None 维度不参与判定（只有有值维度比对）
    let partial = block(
        true,
        measured(Some(90.0), None, None),
        thresholds(80.0, 70.0, 75.0),
    );
    assert_eq!(
        determine_conclusion(0, Some(&partial), None, false),
        Conclusion::Pass
    );
}

#[test]
fn 结论判定_u64计数两极一致() {
    let coverage = block(
        true,
        measured(Some(90.0), Some(80.0), Some(85.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    // failed=0 与全失败大计数两极判定一致
    assert_eq!(
        determine_conclusion(0, Some(&coverage), None, false),
        Conclusion::Pass
    );
    assert_eq!(
        determine_conclusion(u64::MAX, Some(&coverage), None, false),
        Conclusion::Fail
    );
    assert_eq!(
        determine_conclusion(1, Some(&coverage), None, false),
        Conclusion::Fail
    );
}

#[test]
fn 结论判定_mutation未达阈fail() {
    // 函数面完整语义（生产聚合恒 None 不触达——corpus 含块样本与该面对齐）
    let passing_coverage = block(
        true,
        measured(Some(90.0), Some(80.0), Some(85.0)),
        thresholds(80.0, 70.0, 75.0),
    );
    let failing_mutation = MutationBlock {
        pass: false,
        score: 55.0,
        threshold: 70.0,
        measured: MutationMeasured {
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
    };
    assert_eq!(
        determine_conclusion(0, Some(&passing_coverage), Some(&failing_mutation), false),
        Conclusion::Fail
    );
    // 突变达标 → pass
    let passing_mutation = MutationBlock {
        pass: true,
        ..failing_mutation
    };
    assert_eq!(
        determine_conclusion(0, Some(&passing_coverage), Some(&passing_mutation), false),
        Conclusion::Pass
    );
}

// ---------------------------------------------------------------------------
// aggregate_coverage（全局聚合 + per-suite override 组）
// ---------------------------------------------------------------------------

/// 正向：多子报告全局聚合 + per-suite override 组（suite glob 纯匹配命中
/// source_files 取该 suite 阈值；范围圈定 / 加权实测 / 达阈判定逐字段相等）。
#[test]
fn 多子报告全局聚合与per_suite_override组() {
    let suites = vec![
        suite(
            "src/app",
            TestFramework::VitePlus,
            config_thresholds(60.0, 50.0, 40.0),
            Some(vec!["**/*.rs".to_owned()]),
            None,
        ),
        suite(
            "crates",
            TestFramework::Rust,
            config_thresholds(80.0, 70.0, 75.0),
            Some(vec!["**/*.rs".to_owned()]),
            None,
        ),
        suite(
            "absent",
            TestFramework::Vitest,
            config_thresholds(80.0, 70.0, 75.0),
            Some(vec!["**/*.rs".to_owned()]),
            None,
        ),
    ];

    let vite_report = sub_report(
        "src/app",
        "vite-plus",
        counts(2, 2, 0, 0),
        vec![
            entry(
                "src/app/main.rs",
                10,
                9,
                Some(10),
                Some(5),
                Some(4),
                Some(2),
            ),
            entry("src/lib/extra.rs", 10, 5, None, None, None, None),
        ],
        None,
    );
    let rust_report = sub_report(
        "crates",
        "rust",
        counts(1, 1, 0, 0),
        vec![entry(
            "crates/foo/src/lib.rs",
            10,
            8,
            None,
            None,
            None,
            None,
        )],
        None,
    );
    let sub_reports = vec![vite_report, rust_report];

    let result = aggregate_coverage(&sub_reports, &suites).expect("有覆盖数据应产块");

    // 全局阈值 = 首 suite 阈值（60/50/40）
    assert_eq!(result.thresholds, thresholds(60.0, 50.0, 40.0));

    // 全局加权实测：lines 22/30=73.33、branches 5/10=50、functions 2/4=50
    assert_eq!(
        result.measured,
        measured(Some(73.33), Some(50.0), Some(50.0))
    );

    // 全局达阈：73.33≥60 ∧ 50≥50 ∧ 50≥40 ✓
    assert!(result.pass);

    // override 组：命中圈定范围的两个 suite 各产一条，glob 不命中的 absent 组不产
    let overrides = result.overrides.as_ref().expect("override 组在场");
    assert_eq!(overrides.len(), 2, "absent 圈定零命中 → 该组不产条目");
    assert_eq!(overrides[0].glob, "src/app");
    assert_eq!(
        overrides[0].measured,
        measured(Some(90.0), Some(50.0), Some(50.0)),
        "src/app 圈定仅命中 main.rs（加权 9/10、5/10、2/4）"
    );
    assert_eq!(overrides[0].thresholds, thresholds(60.0, 50.0, 40.0));
    assert!(overrides[0].pass);
    assert_eq!(overrides[0].file_count, 1);
    assert_eq!(overrides[0].passed_count, 1, "逐文件达阈计数");
    assert_eq!(overrides[1].glob, "crates");
    assert_eq!(overrides[1].measured, measured(Some(80.0), None, None));
    assert!(overrides[1].pass, "None 维度感知跳过，lines 80≥80 达阈");
    assert_eq!(overrides[1].file_count, 1);
}

/// 异常：suite 阈值缺失（suites 空集——CLI config coverage null 的 Rust 形态）
/// → 默认档语义不 panic（80/70/75 兜底，CLI 同语义）。
#[test]
fn suite阈值缺失走默认档不panic() {
    let report = sub_report(
        ".",
        "vite-plus",
        counts(1, 1, 0, 0),
        vec![entry("src/a.ts", 10, 9, None, None, None, None)],
        Some(block(
            true,
            measured(Some(90.0), None, None),
            thresholds(80.0, 70.0, 75.0),
        )),
    );

    let result = aggregate_coverage(&[report], &[]).expect("空 suites 仍产块");
    assert_eq!(
        result.thresholds,
        thresholds(80.0, 70.0, 75.0),
        "无 suite 配置 → CLI defaults 同值兜底档"
    );
    assert!(result.pass, "90% ≥ 80% 默认档达阈");
    assert!(result.overrides.is_none(), "空 suites 无 override 组");
}

/// 边界：子报告 source_files 全空（无任何覆盖数据）→ 聚合产出 None 零维面
///（不产假计数；判定按 null 感知跳过）。
#[test]
fn 子报告source_files全空_产出null零维面() {
    // 单子报告无覆盖块 → None
    let bare = sub_report(".", "vite-plus", counts(1, 1, 0, 0), Vec::new(), None);
    let suites = vec![suite(
        ".",
        TestFramework::VitePlus,
        config_thresholds(80.0, 70.0, 75.0),
        None,
        None,
    )];
    assert!(
        aggregate_coverage(&[bare.clone()], &suites).is_none(),
        "无数据产 None 不产假块"
    );

    // 多子报告全空 → None
    let another = sub_report("app", "node-test", counts(2, 2, 0, 0), Vec::new(), None);
    assert!(aggregate_coverage(&[bare, another], &suites).is_none());

    // 逐文件条目在场但 coverage 计数缺失（entry.coverage = None）同 None
    let no_counts = SourceFileEntry {
        file: "src/x.ts".to_owned(),
        coverage: None,
    };
    let with_missing = sub_report(".", "vite-plus", counts(1, 1, 0, 0), vec![no_counts], None);
    assert!(
        aggregate_coverage(&[with_missing], &suites).is_none(),
        "计数缺失条目不产假计数"
    );
}

/// 边界：suite glob 不命中任何文件 → 该 suite override 不产条目（全局块
/// overrides 缺省 None）。
#[test]
fn suite_glob不命中_override组不产条目() {
    let suites = vec![suite(
        "no/such/dir",
        TestFramework::Rust,
        config_thresholds(80.0, 70.0, 75.0),
        Some(vec!["**/*.rs".to_owned()]),
        None,
    )];
    let report = sub_report(
        ".",
        "rust",
        counts(1, 1, 0, 0),
        vec![entry("src/elsewhere.rs", 10, 5, None, None, None, None)],
        Some(block(
            false,
            measured(Some(50.0), None, None),
            thresholds(80.0, 70.0, 75.0),
        )),
    );

    let result = aggregate_coverage(&[report], &suites).expect("覆盖块在场产块");
    assert!(result.overrides.is_none(), "圈定零命中 → 无 override 组");
    assert!(
        !result.pass,
        "全局实测 lines 50% 低于 80% 阈值（无 override 组时的纯全局判定）"
    );
}

/// 花括号方言（CLI picomatch `{a,b}` 交替）：includes / excludes 花括号形态
/// 经共享 `globmatch` 展开——override 圈定面与本仓库 config 同形模式对齐。
#[test]
fn suite范围_花括号includes与excludes() {
    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        config_thresholds(80.0, 70.0, 75.0),
        Some(vec!["src/**/*.{ts,tsx}".to_owned()]),
        Some(vec!["src/{legacy,old}/**".to_owned()]),
    )];
    let scoped = |path: &str| in_suite_scope(path, &suites[0], &suites);

    assert!(scoped("app/src/a.ts"), "includes 花括号 .ts 选枝命中");
    assert!(
        scoped("app/src/nested/b.tsx"),
        "includes 花括号 .tsx 选枝命中"
    );
    assert!(!scoped("app/src/c.md"), "非选枝扩展不命中");
    assert!(!scoped("app/src/legacy/x.ts"), "excludes 花括号剪除");
    assert!(!scoped("app/src/old/y.tsx"), "excludes 花括号剪除");
    assert!(!scoped("other/src/a.ts"), "root 树外不命中");
}

/// 缺省 glob 花括号形态命中（jest / vitest / vite-plus 档镜像
/// `**/*.{test,spec}.{js,ts,jsx,tsx}`——修复前引擎无交替方言恒零命中，
/// 逐文件 override 圈定静默失效）。
#[test]
fn suite范围_缺省glob花括号命中() {
    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        config_thresholds(80.0, 70.0, 75.0),
        None,
        None,
    )];

    assert!(
        in_suite_scope("app/src/a.test.ts", &suites[0], &suites),
        "缺省 glob .test.ts 命中"
    );
    assert!(
        in_suite_scope("app/src/b.spec.tsx", &suites[0], &suites),
        "缺省 glob .spec.tsx 命中"
    );
    assert!(
        !in_suite_scope("app/src/c.ts", &suites[0], &suites),
        "非测试命名不命中"
    );
}

// ---------------------------------------------------------------------------
// build_summary_report（汇总组装）
// ---------------------------------------------------------------------------

/// 正向：计数合并 / problems 归并 / plans 路径索引 / coverage 块 / conclusion
/// 判定组装逐字段相等。
#[test]
fn 汇总组装逐字段相等() {
    let suites = vec![suite(
        "src/app",
        TestFramework::VitePlus,
        config_thresholds(80.0, 70.0, 75.0),
        Some(vec!["**/*.rs".to_owned()]),
        None,
    )];

    let failing_case = TestCaseResult {
        name: "math > rejects NaN".to_owned(),
        file: None,
        duration_ms: None,
        status: TestCaseStatus::Failed,
        line: None,
        error_type: Some("AssertionError".to_owned()),
        error_message: Some("expected 1 to equal 2".to_owned()),
        stack_trace: None,
    };
    let passing_case = TestCaseResult {
        name: "math > adds".to_owned(),
        file: None,
        duration_ms: None,
        status: TestCaseStatus::Passed,
        line: None,
        error_type: None,
        error_message: None,
        stack_trace: None,
    };

    let mut vite = sub_report(
        "src/app",
        "vite-plus",
        counts(2, 1, 1, 0),
        vec![entry("src/app/main.rs", 10, 9, None, None, None, None)],
        None,
    );
    vite.error_cases = vec![failing_case.clone(), passing_case];
    let node = sub_report(".", "node-test", counts(3, 3, 0, 0), Vec::new(), None);

    let summary = build_summary_report(
        &[vite, node],
        &suites,
        "desktop change run: test-execution gate",
        OffsetDateTime::now_utc(),
    );

    // 计数合并
    assert_eq!(summary.total, 5);
    assert_eq!(summary.passed, 4);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.skipped, 0);

    // 固定串与 command 透传
    assert_eq!(summary.phase, "test-execution");
    assert_eq!(
        summary.command, "desktop change run: test-execution gate",
        "command 透传入参（措辞不锚定桌面固定串，仅存在性/透传面）"
    );

    // problems 归并：失败用例逐条 test_failure（passed 态同列 error_cases 不产）
    assert_eq!(summary.problems.len(), 1, "仅失败用例产 problem");
    assert_eq!(summary.problems[0].framework, "vite-plus");
    assert_eq!(
        summary.problems[0].problem_type,
        crate::model::ProblemType::TestFailure
    );
    assert_eq!(
        summary.problems[0].message,
        "Test \"math > rejects NaN\" failed: expected 1 to equal 2"
    );

    // coverage 块：多子报告自全部逐文件条目加权聚合（vite 条目 9/10 → 90%；
    // branches / functions 零有效数据 → None 零维面）
    let coverage = summary.coverage.as_ref().expect("有覆盖数据应产块");
    assert!(coverage.pass, "lines 90% ≥ 80% 且 None 维度跳过");
    assert_eq!(coverage.measured, measured(Some(90.0), None, None));
    assert_eq!(coverage.thresholds, thresholds(80.0, 70.0, 75.0));

    // mutation 恒 None（V1 不移植）
    assert!(summary.mutation.is_none());

    // plans 路径索引（planId 推导 + reports/test 相对前缀）
    assert_eq!(summary.plans.len(), 2);
    assert_eq!(summary.plans[0].id, derive_plan_id("src/app", "vite-plus"));
    assert_eq!(summary.plans[0].framework, "vite-plus");
    assert_eq!(
        summary.plans[0].path,
        format!("reports/test/{}", summary.plans[0].id)
    );
    assert_eq!(summary.plans[1].id, derive_plan_id(".", "node-test"));

    // conclusion：failed=1 且无 execution_error、无覆盖块 → fail
    assert_eq!(summary.conclusion, Conclusion::Fail);
}

/// 边界：零子报告 → 空汇总（四计数 0、problems 空不 panic）。
#[test]
fn 零子报告空汇总不panic() {
    let summary = build_summary_report(&[], &[], "cmd", OffsetDateTime::now_utc());

    assert_eq!(summary.total, 0);
    assert_eq!(summary.passed, 0);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.skipped, 0);
    assert!(summary.problems.is_empty(), "零子报告零 problems");
    assert!(summary.coverage.is_none() && summary.mutation.is_none());
    assert!(summary.plans.is_empty());
    assert_eq!(summary.conclusion, Conclusion::Pass, "空集绿态判定");

    // 汇总自洽：完整 SummaryReport 形态可序列化（写盘面消费形态）
    let text = serde_json::to_string(&summary).expect("空汇总应可序列化");
    assert!(text.contains("\"conclusion\":\"pass\""));
}

/// 边界：command 字段断言仅存在性不锚措辞（桌面固定串实现期定稿——design
/// 待决遗留：非空即持恒，措辞漂移不破坏聚合契约）。
#[test]
fn command字段仅存在性断言不锚措辞() {
    let summary = build_summary_report(&[], &[], "任意命令描述措辞", OffsetDateTime::now_utc());
    assert!(
        !summary.command.is_empty(),
        "command 在场即可（透传入参，措辞不进契约）"
    );
}
