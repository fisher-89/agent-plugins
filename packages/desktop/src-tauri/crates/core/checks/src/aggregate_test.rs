use time::OffsetDateTime;

use config::{MutationConfig, TestFramework, TestSuite};

use crate::aggregate::{
    build_summary_report, coverage_meets_thresholds, derive_plan_id, determine_conclusion,
    is_file_excluded,
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

/// config::TestSuite 内存构造（framework 无 Copy，逐调用点显式构造枚举值）。
fn suite(root: &str, framework: TestFramework, excludes: Option<Vec<String>>) -> TestSuite {
    TestSuite {
        root: root.to_owned(),
        framework,
        cwd: ".".to_owned(),
        config: None,
        includes: None,
        excludes,
        coverage: config::CoverageThresholds {
            lines: 80.0,
            branches: 70.0,
            functions: 75.0,
        },
        mutation: MutationConfig {
            cwd: None,
            score: 70.0,
        },
    }
}

// ---------------------------------------------------------------------------
// build_summary_report（汇总组装）
// ---------------------------------------------------------------------------

/// 边界：零子报告 → 空汇总（四计数 0、problems 空不 panic）。
#[test]
fn 零子报告空汇总不panic() {
    let summary = build_summary_report(&[], "cmd", OffsetDateTime::now_utc());

    assert_eq!(summary.total, 0);
    assert_eq!(summary.passed, 0);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.skipped, 0);
    assert!(summary.problems.is_empty(), "零子报告零 problems");
    assert!(summary.plans.is_empty());
    assert_eq!(summary.conclusion, Conclusion::Pass, "空集绿态判定");

    // 汇总自洽：完整 SummaryReport 形态可序列化（写盘面消费形态）
    let text = serde_json::to_string(&summary).expect("空汇总应可序列化");
    assert!(text.contains("\"conclusion\":\"pass\""));
    let value: serde_json::Value = serde_json::from_str(&text).expect("线格式可回读");
    assert!(
        !value
            .as_object()
            .expect("顶层对象")
            .contains_key("coverage"),
        "空汇总零 coverage 键"
    );
    assert!(
        !value
            .as_object()
            .expect("顶层对象")
            .contains_key("mutation"),
        "空汇总零 mutation 键"
    );
}

/// 边界：command 字段断言仅存在性不锚措辞（桌面固定串实现期定稿——design
/// 待决遗留：非空即持恒，措辞漂移不破坏聚合契约）。
#[test]
fn command字段仅存在性断言不锚措辞() {
    let summary = build_summary_report(&[], "任意命令描述措辞", OffsetDateTime::now_utc());
    assert!(
        !summary.command.is_empty(),
        "command 在场即可（透传入参，措辞不进契约）"
    );
}

// ---------------------------------------------------------------------------
// mutation 聚合单点（desktop-fix-mutation-test / AC-4 / AC-5）：
// build_mutation_block（score 公式单点 `(killed+timeout)/(total-ignored-
// compileError-runtimeError)`，分母 0 → 100.0，两位舍入，pass = score >=
// threshold）+ merge_mutation_blocks（段计数求和归并，阈值取首块；空 → None）
// ---------------------------------------------------------------------------

use crate::aggregate::{build_mutation_block, merge_mutation_blocks};

/// mutation 计数 fixture（全字段显式）。
#[allow(clippy::too_many_arguments)]
fn mutation_counts(
    killed: u64,
    survived: u64,
    timeout: u64,
    no_coverage: u64,
    compile_error: u64,
    runtime_error: u64,
    ignored: u64,
    total: u64,
) -> MutationMeasured {
    MutationMeasured {
        killed,
        survived,
        timeout,
        no_coverage,
        compile_error,
        runtime_error,
        ignored,
        total,
        detected: killed + timeout,
        undetected: survived + no_coverage,
    }
}

/// 正向：典型计数 → score 逐字等于公式值（两位舍入，1/3 → 33.33）；
/// `pass = score >= threshold`。
#[test]
fn build_mutation_block典型计数score逐字与阈值判定() {
    // killed=1, timeout=0, valid = total(3) - 0 - 0 - 0 = 3 → 1/3 → 33.33
    let block = build_mutation_block(mutation_counts(1, 1, 0, 0, 0, 0, 0, 3), 80.0);
    assert_eq!(block.score, 33.33, "两位舍入（1/3 → 33.33）");
    assert!(!block.pass, "33.33 < 80 → fail");
    assert_eq!(block.threshold, 80.0);
    assert_eq!(block.overrides, None, "override 组不产（V1 边界）");

    // killed=7, timeout=1 → 8/10-2(ignored)=8/8 → 100.0
    let block = build_mutation_block(mutation_counts(7, 1, 1, 0, 0, 0, 2, 10), 80.0);
    assert_eq!(block.score, 100.0);
    assert!(block.pass, "100.0 >= 80 → pass");

    // 典型 55 分样本（corpus mutation-block fixture 同型：11 killed + 2 timeout
    // / 27 total - 3 noCoverage 计入分母吗——noCoverage 不入分母排除面）
    // valid = 27 - 0 - 0 - 0 = 27；(11+2)/27 = 48.15
    let block = build_mutation_block(mutation_counts(11, 7, 2, 3, 0, 0, 0, 27), 70.0);
    assert_eq!(block.score, 48.15, "(11+2)/27 = 48.148… → 48.15");
    assert!(!block.pass);
}

/// 正向：`killed + timeout` 混合计入分子、`no_coverage` 不入分母排除面（分母
/// 仅排 `ignored / compileError / runtimeError`）。
#[test]
fn build_mutation_block分子分母口径_no_coverage入分母不入排除面() {
    // noCoverage 计入 total（分母）但不在排除面：
    // killed=2, timeout=1, noCoverage=1, total=10 → valid = 10-0-0-0 = 10 → 3/10
    let block = build_mutation_block(mutation_counts(2, 0, 1, 1, 0, 0, 0, 10), 30.0);
    assert_eq!(block.score, 30.0, "noCoverage 计入分母（不入排除面）");
    assert!(block.pass, "30.0 >= 30 → 恰等 pass");

    // 排除面三键各扣：total=10, ignored=2, compileError=1, runtimeError=1 →
    // valid = 6；killed=4, timeout=2 → 6/6 = 100.0
    let block = build_mutation_block(mutation_counts(4, 0, 2, 0, 1, 1, 2, 10), 95.0);
    assert_eq!(block.score, 100.0, "排除面三键各扣（valid = 6）");
    assert!(block.pass);
}

/// 异常：分母 0（全 ignored / compileError / runtimeError / 全 0）→ score =
/// 100.0（既有语义）；`threshold` = `NaN` → `pass = false`（`>=` 恒假）；
/// `threshold` = `Inf` → fail。
#[test]
#[allow(non_snake_case)]
fn build_mutation_block分母零与NaN与Inf阈值() {
    // 全 0（零段块边界）→ 100.0
    let block = build_mutation_block(mutation_counts(0, 0, 0, 0, 0, 0, 0, 0), 80.0);
    assert_eq!(block.score, 100.0, "分母 0 → 100.0（既有语义）");
    assert!(block.pass, "100.0 >= 80");

    // 全 ignored（valid = 3-3 = 0）→ 100.0
    let block = build_mutation_block(mutation_counts(0, 0, 0, 0, 0, 0, 3, 3), 80.0);
    assert_eq!(block.score, 100.0, "全 ignored → 分母 0 → 100.0");

    // 全 compileError + runtimeError → 100.0
    let block = build_mutation_block(mutation_counts(0, 0, 0, 0, 2, 1, 0, 3), 80.0);
    assert_eq!(block.score, 100.0);

    // threshold NaN → pass = false（>= 恒假——f64 NaN 比较面）
    let block = build_mutation_block(mutation_counts(10, 0, 0, 0, 0, 0, 0, 10), f64::NAN);
    assert_eq!(block.score, 100.0);
    assert!(!block.pass, "threshold = NaN → pass 恒假");

    // threshold Inf → fail（100.0 < Inf）
    let block = build_mutation_block(mutation_counts(10, 0, 0, 0, 0, 0, 0, 10), f64::INFINITY);
    assert!(!block.pass, "threshold = Inf → fail");
}

/// 边界：`score == threshold` 等值 → `pass = true`（`>=` 边界）；`threshold`
/// = `0.0` / `-0.0` 恒 pass；`threshold` > 100 → 必 fail；计数极大（f64 转换
/// 精度稳定不 panic）。
#[test]
fn build_mutation_block边界阈值与极大计数() {
    // 恰等边界（killed=1 + timeout=1 / total=4 → 50.0 >= 50.0 → pass）
    let block = build_mutation_block(mutation_counts(1, 0, 1, 0, 0, 0, 0, 4), 50.0);
    assert_eq!(block.score, 50.0);
    assert!(block.pass, "score == threshold → pass（>= 边界）");

    // threshold 0.0 / -0.0 恒 pass（score ≥ 0 恒真——全 survived 面 0 分也 pass）
    for threshold in [0.0, -0.0] {
        let block = build_mutation_block(mutation_counts(0, 5, 0, 0, 0, 0, 0, 5), threshold);
        assert_eq!(block.score, 0.0, "全 survived → 0.0 分");
        assert!(block.pass, "threshold = {threshold} → 恒 pass");
    }

    // threshold > 100 → 必 fail
    let block = build_mutation_block(mutation_counts(10, 0, 0, 0, 0, 0, 0, 10), 100.001);
    assert!(!block.pass, "threshold > 100 → 必 fail");

    // 极大计数（f64 转换精度稳定）：u64::MAX/4 量级不 panic、求和不溢出
    let big = u64::MAX / 4;
    let block = build_mutation_block(mutation_counts(big, 0, big, 0, 0, 0, 0, big * 2), 100.0);
    assert_eq!(block.score, 100.0, "2big/2big = 100.0（极大计数精度稳定）");
    assert!(block.pass, "恰等阈值 pass");

    // 分子分母俱大但不等：killed=big, total=4big → big/4big = 25.0
    let block = build_mutation_block(mutation_counts(big, 0, 0, 0, 0, 0, 0, big * 4), 20.0);
    assert_eq!(block.score, 25.0, "big/4big = 25.0");
    assert!(block.pass, "25.0 >= 20");
}

/// mutation 块 fixture（pass / score / threshold 显式——merge 用例构造面）。
fn mutation_block(score: f64, threshold: f64, counts: MutationMeasured) -> MutationBlock {
    MutationBlock {
        pass: score >= threshold,
        score,
        threshold,
        measured: counts,
        overrides: None,
    }
}

/// 正向：多段计数求和归并（各字段和 = 段和）、阈值取首块（载荷展示归并口径
/// ——MUST NOT 新增归并公式）。
#[test]
fn merge_mutation_blocks多段求和与阈值取首块() {
    let seg0 = mutation_block(100.0, 80.0, mutation_counts(4, 0, 1, 0, 0, 0, 1, 6));
    let seg1 = mutation_block(50.0, 70.0, mutation_counts(1, 2, 0, 0, 0, 0, 0, 4));
    let merged = merge_mutation_blocks(&[seg0.clone(), seg1.clone()]).expect("归并块应在场");

    // 各字段和 = 段和
    assert_eq!(merged.measured.killed, 5, "killed 求和");
    assert_eq!(merged.measured.survived, 2);
    assert_eq!(merged.measured.timeout, 1);
    assert_eq!(merged.measured.ignored, 1);
    assert_eq!(merged.measured.total, 10, "total 求和");
    // 归并块重算 score：(5+1)/(10-1) = 6/9 = 66.67
    assert_eq!(
        merged.score, 66.67,
        "归并块重算 score（计数求和后交 build 单点）"
    );
    assert_eq!(merged.threshold, 80.0, "阈值取首块（既有语义）");
    assert!(!merged.pass, "66.67 < 80（首块阈值判定）");
    assert_eq!(merged.overrides, None);
}

/// 正向：单段（`[单元素]`）→ 归并块计数 / 阈值与原段等值。
#[test]
fn merge_mutation_blocks单段等值透传() {
    let seg = mutation_block(95.5, 80.0, mutation_counts(9, 0, 1, 0, 0, 0, 0, 10));
    let merged = merge_mutation_blocks(std::slice::from_ref(&seg)).expect("单段归并不为空");
    assert_eq!(merged.measured, seg.measured, "单段计数等值");
    assert_eq!(merged.threshold, seg.threshold, "阈值等值");
    assert_eq!(merged.score, 100.0, "(9+1)/10 = 100.0");
    assert!(merged.pass);
}

/// 异常：空 slice `&[]` → `None`（不产块——段清单非空前置的调用方语义）。
#[test]
fn merge_mutation_blocks空slice返回none() {
    assert_eq!(
        merge_mutation_blocks(&[]),
        None,
        "空 slice 不产块（不产半截归并）"
    );
}

/// 边界：超大列表（1000 段）归并不炸、顺序不影响计数和；段计数含 0 / `u64`
/// 大值混合。
#[test]
fn merge_mutation_blocks超大列表与顺序无关与大值混合() {
    // 1000 段等值计数：每段 killed=1, total=2 → 归并 killed=1000, total=2000
    let segs: Vec<MutationBlock> = (0..1000)
        .map(|index| {
            mutation_block(
                if index % 2 == 0 { 100.0 } else { 0.0 },
                50.0,
                mutation_counts(1, 1, 0, 0, 0, 0, 0, 2),
            )
        })
        .collect();
    let merged = merge_mutation_blocks(&segs).expect("超大列表归并不炸");
    assert_eq!(merged.measured.killed, 1000);
    assert_eq!(merged.measured.total, 2000);
    // (1000+0)/2000 = 50.0
    assert_eq!(merged.score, 50.0);
    assert!(merged.pass, "50.0 >= 50.0（首块阈值恰等）");

    // 顺序无关：逆序归并计数和一致
    let reversed: Vec<MutationBlock> = segs.iter().rev().cloned().collect();
    let merged_rev = merge_mutation_blocks(&reversed).expect("逆序归并不炸");
    assert_eq!(
        merged_rev.measured, merged.measured,
        "顺序不影响计数和（求和交换律）"
    );

    // 0 / 大值混合段
    let mixed = [
        mutation_block(0.0, 10.0, mutation_counts(0, 0, 0, 0, 0, 0, 0, 0)),
        mutation_block(
            100.0,
            10.0,
            mutation_counts(u64::MAX / 8, 0, 0, 0, 0, 0, 0, u64::MAX / 8),
        ),
    ];
    let merged = merge_mutation_blocks(&mixed).expect("混合段归并不炸");
    assert_eq!(merged.measured.total, u64::MAX / 8, "0 段不扰求和");
    assert_eq!(merged.score, 100.0);
}

// ---------------------------------------------------------------------------
// determine_conclusion（布尔归并唯一判定点：error → failed>0 / 逐 suite 覆盖
// 判定非通过 → fail → pass；签名内无 CoverageBlock / MutationBlock）
// ---------------------------------------------------------------------------

/// 全绿布尔面子报告（覆盖判定 pass、计数绿态）。
fn passing_report(root: &str, framework: &str) -> SubReport {
    sub_report(
        root,
        framework,
        counts(2, 2, 0, 0),
        Vec::new(),
        Some(block(
            true,
            measured(Some(95.0), Some(80.0), Some(85.0)),
            thresholds(80.0, 70.0, 75.0),
        )),
    )
}

/// 正向：`failed = 0` + 两子报告覆盖判定 pass + 无 execution_error → pass。
#[test]
fn 结论判定_全绿布尔面pass() {
    let conclusion = determine_conclusion(
        0,
        &[
            passing_report("src/app", "vite-plus"),
            passing_report("crates", "rust"),
        ],
        false,
    );
    assert_eq!(conclusion, Conclusion::Pass);
}

/// 正向（AC-4 核心回归锚）：掩盖不可达——vite-plus 三维全达标 + rust
/// `functions 87.60 < 90` → fail（一个 suite 的达标值 MUST NOT 稀释另一个
/// suite 的缺口）。
#[test]
fn 结论判定_掩盖不可达() {
    let vite = sub_report(
        "src/app",
        "vite-plus",
        counts(3, 3, 0, 0),
        Vec::new(),
        Some(block(
            true,
            measured(Some(95.40), Some(94.06), Some(91.60)),
            thresholds(80.0, 70.0, 75.0),
        )),
    );
    let rust = sub_report(
        "crates",
        "rust",
        counts(2, 2, 0, 0),
        Vec::new(),
        Some(block(
            false,
            measured(Some(95.94), None, Some(87.60)),
            thresholds(80.0, 70.0, 90.0),
        )),
    );
    assert_eq!(
        determine_conclusion(0, &[vite, rust], false),
        Conclusion::Fail
    );
}

/// 正向：`failed > 0` → fail（覆盖全 pass 不豁免）。
#[test]
fn 结论判定_failed大于零fail() {
    assert_eq!(
        determine_conclusion(1, &[passing_report("src/app", "vite-plus")], false),
        Conclusion::Fail
    );
}

/// 正向（2 形态）：`has_execution_error` 优先级最高——`failed = 5` + 覆盖未达
/// + error → error；`failed = 0` + 覆盖全 pass + error → error。
#[test]
fn 结论判定_execution_error优先级最高() {
    let failing = sub_report(
        "crates",
        "rust",
        counts(5, 0, 5, 0),
        Vec::new(),
        Some(block(
            false,
            measured(Some(10.0), Some(10.0), Some(10.0)),
            thresholds(80.0, 70.0, 75.0),
        )),
    );
    assert_eq!(determine_conclusion(5, &[failing], true), Conclusion::Error);

    assert_eq!(
        determine_conclusion(0, &[passing_report("crates", "rust")], true),
        Conclusion::Error
    );
}

/// 正向（AC-11）：突变块退出判定——覆盖全 pass + 无失败 + 子报告
/// `mutation = Some(pass: false)` → pass（突变只走 MutationTest 载荷）。
#[test]
fn 结论判定_突变不参与汇总() {
    let mut report = passing_report("crates", "rust");
    report.mutation = Some(mutation_block(
        55.0,
        70.0,
        mutation_counts(11, 7, 2, 3, 0, 0, 0, 27),
    ));
    assert_eq!(determine_conclusion(0, &[report], false), Conclusion::Pass);
}

/// 异常（AC-3 / AC-5）：缺采集判 fail 不判 error——绿跑子报告（退出码 0 /
/// 零失败）`coverage = None` + 另一 suite 达标 → fail。
#[test]
fn 结论判定_缺采集判fail不判error() {
    let missing = sub_report("crates", "rust", counts(3, 3, 0, 0), Vec::new(), None);
    let conclusion =
        determine_conclusion(0, &[missing, passing_report("src/app", "vite-plus")], false);
    assert_eq!(conclusion, Conclusion::Fail, "缺采集非 pass 亦非 error");
}

/// 异常：两缺口并存（一块自判未达 + 一块缺席）→ 单一 fail（零重复判据）。
#[test]
fn 结论判定_两类缺口并存单一fail() {
    let below = sub_report(
        "crates",
        "rust",
        counts(1, 1, 0, 0),
        Vec::new(),
        Some(block(
            false,
            measured(Some(50.0), None, None),
            thresholds(80.0, 70.0, 75.0),
        )),
    );
    let missing = sub_report("src/app", "vite-plus", counts(1, 1, 0, 0), Vec::new(), None);
    assert_eq!(
        determine_conclusion(0, &[below, missing], false),
        Conclusion::Fail
    );
}

/// 边界：空集两极——`failed = 0` + 无 error → pass；同空集 + error → error。
#[test]
fn 结论判定_空集两极() {
    assert_eq!(determine_conclusion(0, &[], false), Conclusion::Pass);
    assert_eq!(determine_conclusion(0, &[], true), Conclusion::Error);
}

/// 边界（2 形态）：计数两极——`failed = 0` 不触发 fail 臂；`u64::MAX` → fail
/// （零溢出、零饱和）。
#[test]
fn 结论判定_u64计数两极() {
    let reports = [passing_report("crates", "rust")];
    assert_eq!(determine_conclusion(0, &reports, false), Conclusion::Pass);
    assert_eq!(
        determine_conclusion(u64::MAX, &reports, false),
        Conclusion::Fail
    );
}

/// 边界（2 组合）：`block.pass` 是单一真值源——块自判 pass 而维度值低于阈值
/// （手搓不自洽块）→ pass；块自判未达而三维全 null → fail（判据不重算维度）。
#[test]
fn 结论判定_只读块布尔不重算维度() {
    let self_contradicting = sub_report(
        "crates",
        "rust",
        counts(1, 1, 0, 0),
        Vec::new(),
        Some(block(
            true,
            measured(Some(50.0), None, None),
            thresholds(80.0, 70.0, 75.0),
        )),
    );
    assert_eq!(
        determine_conclusion(0, &[self_contradicting], false),
        Conclusion::Pass
    );

    let no_dimension = sub_report(
        "crates",
        "rust",
        counts(1, 1, 0, 0),
        Vec::new(),
        Some(block(
            false,
            measured(None, None, None),
            thresholds(80.0, 70.0, 75.0),
        )),
    );
    assert_eq!(
        determine_conclusion(0, &[no_dimension], false),
        Conclusion::Fail
    );
}

/// 边界（AC-6，2 组合）：rust 形态 `branches = None` + 有值维度达阈——块自判
/// pass → pass；同形块自判未达 → fail（null 维度不进判定）。
#[test]
fn 结论判定_维度null跳过两形态() {
    let rust_measured = || measured(Some(95.94), None, Some(95.0));
    let pass = sub_report(
        "crates",
        "rust",
        counts(1, 1, 0, 0),
        Vec::new(),
        Some(block(true, rust_measured(), thresholds(80.0, 70.0, 90.0))),
    );
    assert_eq!(determine_conclusion(0, &[pass], false), Conclusion::Pass);

    let fail = sub_report(
        "crates",
        "rust",
        counts(1, 1, 0, 0),
        Vec::new(),
        Some(block(false, rust_measured(), thresholds(80.0, 70.0, 90.0))),
    );
    assert_eq!(determine_conclusion(0, &[fail], false), Conclusion::Fail);
}

// ---------------------------------------------------------------------------
// coverage_meets_thresholds（逐 suite 判定单点：null 维度感知跳过）
// ---------------------------------------------------------------------------

/// 正向：三维有值且 ≥ 阈（含恰等于阈值）→ true；任一维低于阈值 → false。
#[test]
fn 逐维达标与缺口判定() {
    let limits = thresholds(80.0, 70.0, 75.0);
    assert!(
        coverage_meets_thresholds(&measured(Some(80.0), Some(70.0), Some(75.0)), &limits),
        "恰等于阈值即达标（≥ 边界）"
    );
    assert!(
        coverage_meets_thresholds(&measured(Some(99.9), Some(99.9), Some(99.9)), &limits),
        "全维超出阈值达标"
    );
    assert!(
        !coverage_meets_thresholds(&measured(Some(79.999), Some(70.0), Some(75.0)), &limits),
        "差值 0.001 即未达（非四舍五入口径）"
    );
}

/// 异常（3 形态）：病态数值不误判通过——`NaN` 维度沿既有比较语义视为达标；
/// 负覆盖 → false；阈值 0.0 + 实测 0.0 → true（零阈不误 fail）。
#[test]
fn 病态数值不误判通过() {
    let limits = thresholds(80.0, 70.0, 75.0);
    assert!(
        coverage_meets_thresholds(&measured(Some(f64::NAN), None, None), &limits),
        "NaN < threshold 为假 ⇒ 沿既有比较语义不判未达"
    );
    assert!(
        !coverage_meets_thresholds(&measured(Some(-1.0), None, None), &limits),
        "负覆盖必未达"
    );
    assert!(
        coverage_meets_thresholds(
            &measured(Some(0.0), Some(0.0), Some(0.0)),
            &thresholds(0.0, 0.0, 0.0)
        ),
        "零阈 + 零实测达标"
    );
}

/// 边界（AC-6 / D4）：维度 `None` 跳过——`branches = None` + lines / functions
/// 达标 → true；三维全 `None` → true（零维度面不误判）。
#[test]
fn 维度null跳过不误判() {
    assert!(
        coverage_meets_thresholds(
            &measured(Some(95.94), None, Some(95.0)),
            &thresholds(80.0, 70.0, 90.0)
        ),
        "rust 形态 branches 不支持 → 该维跳过"
    );
    assert!(
        coverage_meets_thresholds(&measured(None, None, None), &thresholds(80.0, 70.0, 75.0)),
        "零维度面不判未达"
    );
}

// ---------------------------------------------------------------------------
// is_file_excluded（突变归桶过滤唯一入口；excludes 全桶消费口径）
// ---------------------------------------------------------------------------

/// 正向（2 形态）：排除 glob 相对 suite root 解释——树下命中排除模式的路径
/// 剔除，同树未命中路径保留。
#[test]
fn 排除面_树下命中与未命中() {
    let suites = vec![suite(
        "src/app",
        TestFramework::VitePlus,
        Some(vec!["**/ui/**".to_owned()]),
    )];
    assert!(
        is_file_excluded("src/app/ui/button.ts", &suites),
        "树下命中排除模式 → 剔除"
    );
    assert!(
        !is_file_excluded("src/app/main.ts", &suites),
        "同树未命中 → 保留"
    );
}

/// 异常（4 形态）：非排除臂皆不剔除且零 panic——`excludes = None` /
/// `excludes = Some(vec![])` / 路径不在任何 suite root 下 / `suites = []`。
#[test]
fn 非排除臂不剔除() {
    let none_excludes = vec![suite("src/app", TestFramework::VitePlus, None)];
    assert!(!is_file_excluded("src/app/ui/button.ts", &none_excludes));

    let empty_excludes = vec![suite("src/app", TestFramework::VitePlus, Some(Vec::new()))];
    assert!(!is_file_excluded("src/app/ui/button.ts", &empty_excludes));

    let excludes = Some(vec!["**/ui/**".to_owned()]);
    assert!(!is_file_excluded(
        "other/x.ts",
        &[suite("src/app", TestFramework::VitePlus, excludes.clone())]
    ));
    assert!(!is_file_excluded("src/app/ui/button.ts", &[]));
}

/// 边界（5 形态）：路径形态边界——反斜杠归一命中；尾随 `/` 仍命中
/// `ui/**`；绝对 POSIX 路径以 `/<root>/` 段标记容忍项目前缀；`root = "."`
/// 的树根自身不剔除；花括号交替方言命中 `.ts`。
#[test]
fn 路径形态边界() {
    let excludes = Some(vec!["**/ui/**".to_owned()]);
    assert!(
        is_file_excluded(
            "src\\app\\ui\\x.ts",
            &[suite("src/app", TestFramework::VitePlus, excludes.clone())]
        ),
        "反斜杠路径归一后命中"
    );

    let dir_excludes = Some(vec!["ui/**".to_owned()]);
    assert!(
        is_file_excluded(
            "src/app/ui/",
            &[suite("src/app", TestFramework::VitePlus, dir_excludes)]
        ),
        "尾随分隔符路径仍命中目录排除面"
    );

    assert!(
        is_file_excluded(
            "/repo/src/app/ui/x.ts",
            &[suite("src/app", TestFramework::VitePlus, excludes.clone())]
        ),
        "绝对 POSIX 路径按 /<root>/ 段标记定位树内相对位置"
    );

    assert!(
        !is_file_excluded(
            ".",
            &[suite(".", TestFramework::VitePlus, excludes.clone())]
        ),
        "树根自身（rel_to_root == \".\"）不进剔除面"
    );

    assert!(
        is_file_excluded(
            "src/app/ui/x.ts",
            &[suite(
                "src/app",
                TestFramework::VitePlus,
                Some(vec!["**/*.{ts,tsx}".to_owned()])
            )]
        ),
        "花括号交替方言逐变体命中"
    );
}

// ---------------------------------------------------------------------------
// build_summary_report 的覆盖归因面（逐 suite `CoverageFailure`；缺采集判
// fail 不判 error）与 summary 瘦身面
// ---------------------------------------------------------------------------

/// 失败用例 fixture（`TestFailure` 归因原料）。
fn failed_case(name: &str, message: &str) -> TestCaseResult {
    TestCaseResult {
        name: name.to_owned(),
        file: None,
        duration_ms: None,
        status: TestCaseStatus::Failed,
        line: None,
        error_type: Some("AssertionError".to_owned()),
        error_message: Some(message.to_owned()),
        stack_trace: None,
    }
}

/// 通例子报告构造（退出码 / 计数 / 覆盖块逐面显式）。
#[allow(clippy::too_many_arguments)]
fn report_with(
    root: &str,
    framework: &str,
    exit_code: i32,
    summary: CaseSummary,
    source_files: Vec<SourceFileEntry>,
    coverage: Option<CoverageBlock>,
    error_cases: Vec<TestCaseResult>,
    findings: Option<Vec<String>>,
) -> SubReport {
    let mut report = sub_report(root, framework, summary, source_files, coverage);
    report.exit_code = exit_code;
    report.error_cases = error_cases;
    report.findings = findings;
    report
}

/// 有覆盖率且达阈的绿色子报告（vite 96.21%）。
fn covered_report(root: &str, framework: &str) -> SubReport {
    report_with(
        root,
        framework,
        0,
        counts(1, 1, 0, 0),
        vec![entry("src/lib.rs", 1000, 962, None, None, None, None)],
        Some(block(
            true,
            measured(Some(96.21), None, None),
            thresholds(80.0, 70.0, 75.0),
        )),
        Vec::new(),
        None,
    )
}

/// 覆盖归因条目（`problems` 中该类问题的框架 / 消息对）。
fn coverage_problems(summary: &crate::model::SummaryReport) -> Vec<(&str, &str)> {
    summary
        .problems
        .iter()
        .filter(|problem| problem.problem_type == crate::model::ProblemType::CoverageFailure)
        .map(|problem| (problem.framework.as_str(), problem.message.as_str()))
        .collect()
}

fn count_of(summary: &crate::model::SummaryReport, kind: crate::model::ProblemType) -> usize {
    summary
        .problems
        .iter()
        .filter(|problem| problem.problem_type == kind)
        .count()
}

/// 正向：全测齐全零变——两 suite 达标 + 全绿 → 四计数求和、`problems` 空、
/// `conclusion = pass`、`plans` 两条（`id == derive_plan_id(root, framework)`、
/// `path == "reports/test/<id>"`）、`phase` / `command` 透传、`duration_seconds`
/// 整秒。
#[test]
fn 全测齐全汇总逐字段() {
    let started = OffsetDateTime::now_utc() - time::Duration::seconds(3);
    let summary = build_summary_report(
        &[
            covered_report("src/app", "vite-plus"),
            covered_report("crates", "rust"),
        ],
        "desktop change run: test-execution gate",
        started,
    );

    assert_eq!(
        (
            summary.total,
            summary.passed,
            summary.failed,
            summary.skipped
        ),
        (2, 2, 0, 0),
        "四计数求和"
    );
    assert!(summary.problems.is_empty(), "全绿零归因");
    assert_eq!(summary.conclusion, Conclusion::Pass);
    assert_eq!(summary.phase, "test-execution");
    assert_eq!(summary.command, "desktop change run: test-execution gate");
    assert_eq!(summary.duration_seconds, 3.0, "墙钟差整秒");

    assert_eq!(summary.plans.len(), 2);
    for (plan, (root, framework)) in summary
        .plans
        .iter()
        .zip([("src/app", "vite-plus"), ("crates", "rust")])
    {
        assert_eq!(plan.id, derive_plan_id(root, framework), "id 沿单点派生");
        assert_eq!(
            plan.path,
            format!("reports/test/{}", plan.id),
            "path 前缀恒定"
        );
        assert_eq!(plan.framework, framework);
        assert_eq!(plan.root, root);
    }
}

/// 正向（AC-2）：summary 瘦身面——序列化后顶层键集恰为 11 个必在键（`findings`
/// 为 `None` 时整键不出现），`coverage` / `mutation` 两键缺席。
#[test]
fn 汇总序列化面无覆盖与突变块() {
    let summary = build_summary_report(
        &[covered_report("src/app", "vite-plus")],
        "cmd",
        OffsetDateTime::now_utc(),
    );
    let value = serde_json::to_value(&summary).expect("汇总应可序列化");
    let object = value.as_object().expect("顶层为对象");

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "command",
            "conclusion",
            "duration_seconds",
            "failed",
            "passed",
            "phase",
            "plans",
            "problems",
            "skipped",
            "timestamp",
            "total",
        ],
        "summary 面 11 键（findings 为 None 时整键不出现）"
    );
    assert!(!object.contains_key("coverage"), "MUST NOT 产 coverage 块");
    assert!(!object.contains_key("mutation"), "MUST NOT 产 mutation 块");
}

/// 正向（AC-4）：逐 suite 覆盖归因——vite 达标 + rust `functions 87.60` 未达
/// 阈 90 → `problems` 恰一条 `coverage_failure`、`framework == "rust"`、消息为
/// `format_coverage_failure` 原文、`conclusion = fail`。
#[test]
fn 逐suite覆盖缺口归因() {
    let vite = sub_report(
        "src/app",
        "vite-plus",
        counts(3, 3, 0, 0),
        Vec::new(),
        Some(block(
            true,
            measured(Some(95.40), Some(94.06), Some(91.60)),
            thresholds(80.0, 70.0, 75.0),
        )),
    );
    let rust = sub_report(
        "crates",
        "rust",
        counts(2, 2, 0, 0),
        Vec::new(),
        Some(block(
            false,
            measured(Some(95.94), None, Some(87.60)),
            thresholds(80.0, 70.0, 90.0),
        )),
    );
    let summary = build_summary_report(&[vite, rust], "cmd", OffsetDateTime::now_utc());

    assert_eq!(
        coverage_problems(&summary),
        vec![("rust", "Coverage below threshold: functions 87.6% < 90%")],
        "达标 suite 零归因，缺口 suite 恰一条（消息原文零改写）"
    );
    assert_eq!(summary.conclusion, Conclusion::Fail);
}

/// 异常（AC-5）：缺采集归因——单 suite 绿跑（退出码 0 / 零失败）
/// `coverage = None` → `problems` 恰一条 `coverage_failure`（框架为该 suite、
/// 消息携「覆盖率采集缺失」「退出码」与动作指引）、零 `execution_error`、
/// `conclusion = fail`。
#[test]
fn 缺采集归因判fail不判error() {
    let missing = report_with(
        "crates",
        "rust",
        0,
        counts(3, 3, 0, 0),
        Vec::new(),
        None,
        Vec::new(),
        None,
    );
    let summary = build_summary_report(&[missing], "cmd", OffsetDateTime::now_utc());

    let coverage = coverage_problems(&summary);
    assert_eq!(coverage.len(), 1, "恰一条覆盖归因: {coverage:?}");
    assert_eq!(coverage[0].0, "rust", "框架为该 suite");
    assert!(
        coverage[0].1.contains("覆盖率采集缺失"),
        "消息点名采集缺失: {}",
        coverage[0].1
    );
    assert!(
        coverage[0].1.contains("退出码 0"),
        "消息携退出码: {}",
        coverage[0].1
    );
    assert!(
        coverage[0].1.contains("采集旗标与覆盖率工件路径"),
        "消息携动作指引: {}",
        coverage[0].1
    );
    assert_eq!(
        count_of(&summary, crate::model::ProblemType::ExecutionError),
        0,
        "缺采集不产 execution_error"
    );
    assert_eq!(summary.conclusion, Conclusion::Fail, "缺采集判 fail");

    // 哨兵退出码（-1）+ 失败用例：覆盖归因逐值携该退出码，且不追加
    // execution_error（失败臂先占，两臂互不改写）
    let sentinel = report_with(
        "crates",
        "rust",
        -1,
        counts(1, 0, 1, 0),
        Vec::new(),
        None,
        vec![failed_case("a", "b")],
        None,
    );
    let summary = build_summary_report(&[sentinel], "cmd", OffsetDateTime::now_utc());
    let coverage = coverage_problems(&summary);
    assert_eq!(coverage.len(), 1, "哨兵面仍恰一条覆盖归因");
    assert!(
        coverage[0].1.contains("退出码 -1"),
        "哨兵退出码逐值可读: {}",
        coverage[0].1
    );
    assert_eq!(
        count_of(&summary, crate::model::ProblemType::ExecutionError),
        0,
        "失败用例面不产 execution_error"
    );
}

/// 异常（R-10）：两类归因并存互不顶替——同 suite `exit_code = 1` + 零失败 +
/// `coverage = None` → 恰两条（`execution_error` 文案 `Exit code 1` 零改写 +
/// `coverage_failure`）、`conclusion = error`（error 优先定型）。
#[test]
fn 两类归因并存互不顶替() {
    let report = report_with(
        "crates",
        "rust",
        1,
        counts(2, 2, 0, 0),
        Vec::new(),
        None,
        Vec::new(),
        None,
    );
    let summary = build_summary_report(&[report], "cmd", OffsetDateTime::now_utc());

    assert_eq!(
        summary.problems.len(),
        2,
        "两类各一条: {:?}",
        summary.problems
    );
    assert_eq!(
        count_of(&summary, crate::model::ProblemType::ExecutionError),
        1
    );
    let execution = summary
        .problems
        .iter()
        .find(|problem| problem.problem_type == crate::model::ProblemType::ExecutionError)
        .expect("execution_error 在场");
    assert_eq!(execution.message, "Exit code 1", "既有文案零改写");
    assert_eq!(coverage_problems(&summary).len(), 1);
    assert_eq!(summary.conclusion, Conclusion::Error, "error 优先定型");
}

/// 异常：失败用例 + 缺采集——`exit_code = 0` + `failed = 2` + `coverage = None`
/// → 2 条 `test_failure` + 1 条 `coverage_failure`、`conclusion = fail`（MUST NOT
/// 升格 error）、`problems[0]` 仍为 `test_failure`（同 suite 内条目序不变）。
#[test]
fn 失败用例与缺采集合并归因() {
    let report = report_with(
        "src/app",
        "vite-plus",
        0,
        counts(2, 0, 2, 0),
        Vec::new(),
        None,
        vec![
            failed_case("math > a", "expected 1 to equal 2"),
            failed_case("math > b", "expected 3 to equal 4"),
        ],
        None,
    );
    let summary = build_summary_report(&[report], "cmd", OffsetDateTime::now_utc());

    assert_eq!(
        count_of(&summary, crate::model::ProblemType::TestFailure),
        2
    );
    assert_eq!(coverage_problems(&summary).len(), 1);
    assert_eq!(
        count_of(&summary, crate::model::ProblemType::ExecutionError),
        0
    );
    assert_eq!(
        summary.conclusion,
        Conclusion::Fail,
        "失败 + 缺采集合判 fail"
    );
    assert_eq!(
        summary.problems[0].problem_type,
        crate::model::ProblemType::TestFailure,
        "同 suite 内条目序不变"
    );
}

/// 异常：全体缺采集——全绿但两 suite 皆 `coverage = None` → 逐 suite 各一条
/// `coverage_failure`、`conclusion = fail`（绿计数不掩盖采集缺口）。
#[test]
fn 全体缺采集不因全绿而通过() {
    let rust = report_with(
        "crates",
        "rust",
        0,
        counts(3, 3, 0, 0),
        Vec::new(),
        None,
        Vec::new(),
        None,
    );
    let vite = report_with(
        "src/app",
        "vite-plus",
        0,
        counts(2, 2, 0, 0),
        Vec::new(),
        None,
        Vec::new(),
        None,
    );
    let summary = build_summary_report(&[rust, vite], "cmd", OffsetDateTime::now_utc());

    assert_eq!(coverage_problems(&summary).len(), 2, "逐 suite 各一条");
    assert_eq!(summary.conclusion, Conclusion::Fail);
}

/// 边界（2 形态）：每 suite 至多一条覆盖归因——块自判未达且三维全缺口 →
/// 恰一条（多维缺口合成一条 message）；块自判未达且三维全 null → 仍一条
/// （消息仅前缀，零维度片段——判据只读 `block.pass`）。
#[test]
fn 每suite至多一条覆盖归因() {
    let all_dimensions = sub_report(
        "crates",
        "rust",
        counts(1, 1, 0, 0),
        Vec::new(),
        Some(block(
            false,
            measured(Some(10.0), Some(20.0), Some(30.0)),
            thresholds(80.0, 70.0, 75.0),
        )),
    );
    let summary = build_summary_report(&[all_dimensions], "cmd", OffsetDateTime::now_utc());
    assert_eq!(
        count_of(&summary, crate::model::ProblemType::ExecutionError),
        0,
        "有覆盖率块即不判缺测（不追加 execution_error）"
    );
    let coverage = coverage_problems(&summary);
    assert_eq!(coverage.len(), 1, "多维缺口合成一条");
    assert_eq!(
        coverage[0].1,
        "Coverage below threshold: lines 10.0% < 80%, branches 20.0% < 70%, functions 30.0% < 75%"
    );

    let no_dimension = sub_report(
        "crates",
        "rust",
        counts(1, 1, 0, 0),
        Vec::new(),
        Some(block(
            false,
            measured(None, None, None),
            thresholds(80.0, 70.0, 75.0),
        )),
    );
    let summary = build_summary_report(&[no_dimension], "cmd", OffsetDateTime::now_utc());
    let coverage = coverage_problems(&summary);
    assert_eq!(coverage.len(), 1, "零维度面仍一条");
    assert!(
        coverage[0].1.starts_with("Coverage below threshold:"),
        "前缀固定: {}",
        coverage[0].1
    );
    assert!(
        !coverage[0].1.contains("lines") && !coverage[0].1.contains("branches"),
        "零维度片段: {}",
        coverage[0].1
    );
}

/// 边界：超大列表——50 条子报告中恰 1 条 `coverage = None` → 恰 1 条
/// `coverage_failure`、其余零归因、`conclusion = fail`。
#[test]
fn 超大列表恰一条缺测() {
    let mut reports: Vec<SubReport> = (0..49)
        .map(|index| covered_report(&format!("suite{index}"), "rust"))
        .collect();
    reports.push(report_with(
        "missing",
        "vite-plus",
        0,
        counts(1, 1, 0, 0),
        Vec::new(),
        None,
        Vec::new(),
        None,
    ));

    let summary = build_summary_report(&reports, "cmd", OffsetDateTime::now_utc());

    let coverage = coverage_problems(&summary);
    assert_eq!(coverage.len(), 1, "50 条中恰 1 条覆盖归因");
    assert_eq!(coverage[0].0, "vite-plus", "归因挂缺采集 suite");
    assert_eq!(
        summary
            .problems
            .iter()
            .filter(|problem| problem.problem_type != crate::model::ProblemType::CoverageFailure)
            .count(),
        0,
        "其余 49 条零归因"
    );
    assert_eq!(summary.total, 50, "计数合并不因覆盖缺口变化");
    assert_eq!(summary.conclusion, Conclusion::Fail);
}

/// 边界（3 形态）：`plans[]` 派生边界——`root = "."` → id 退化为框架名；
/// 同 root 不同 framework → 两不同 id；`root` 含 `\` / `/` → 归一为 `_`
/// （两种分隔符同 id）。
#[test]
fn plans索引派生边界() {
    let rootless = build_summary_report(
        &[covered_report(".", "rust")],
        "cmd",
        OffsetDateTime::now_utc(),
    );
    assert_eq!(rootless.plans[0].id, "rust", "root 为 . 时 id == framework");
    assert_eq!(rootless.plans[0].path, "reports/test/rust");

    let two_frameworks = build_summary_report(
        &[
            covered_report("crates", "rust"),
            covered_report("crates", "go"),
        ],
        "cmd",
        OffsetDateTime::now_utc(),
    );
    assert_ne!(
        two_frameworks.plans[0].id, two_frameworks.plans[1].id,
        "同 root 不同框架 id 不撞"
    );

    let backslash = build_summary_report(
        &[covered_report("src\\app", "rust")],
        "cmd",
        OffsetDateTime::now_utc(),
    );
    let slash = build_summary_report(
        &[covered_report("src/app", "rust")],
        "cmd",
        OffsetDateTime::now_utc(),
    );
    assert_eq!(
        backslash.plans[0].id, slash.plans[0].id,
        "反斜杠与斜杠归一同形"
    );
    assert!(
        !backslash.plans[0].id.contains(['\\', '/']),
        "id 不含路径分隔符: {}",
        backslash.plans[0].id
    );
}

/// 边界（2 形态）：`duration_seconds` 不产负值——`started_at` 为未来时刻 →
/// `0.0`；`started_at` 恰为现在 → `0.0`。
#[test]
fn duration秒差边界() {
    let future = OffsetDateTime::now_utc() + time::Duration::seconds(5);
    let summary = build_summary_report(&[], "cmd", future);
    assert_eq!(summary.duration_seconds, 0.0, "未来时刻截 0（max(0.0)）");

    let summary = build_summary_report(&[], "cmd", OffsetDateTime::now_utc());
    assert_eq!(summary.duration_seconds, 0.0, "零差值整秒 0");
}

/// 边界（3 形态）：`framework` 串边界——空串 / 超长（1200 字符）/ 含换行 ·
/// NUL · emoji → 覆盖归因的 `framework` 逐字透传（零 trim、零截断）。
#[test]
fn framework串边界逐字透传() {
    let long = "f".repeat(1200);
    let special = "框架\n\0🚀";
    for framework in ["", long.as_str(), special] {
        let report = report_with(
            "crates",
            framework,
            0,
            counts(1, 1, 0, 0),
            Vec::new(),
            None,
            Vec::new(),
            None,
        );
        let summary = build_summary_report(&[report], "cmd", OffsetDateTime::now_utc());
        let coverage = coverage_problems(&summary);
        assert_eq!(coverage.len(), 1);
        assert_eq!(coverage[0].0, framework, "framework 逐字透传（零 trim）");
    }
}

/// 边界：缺测面汇总可序列化——`coverage = None` 的归因面 summary 序列化后
/// `conclusion: fail`、`type: coverage_failure`，且 `coverage` 键整键缺席
/// （落盘面消费形态）。
#[test]
fn 缺测面汇总可序列化() {
    let report = report_with(
        "crates",
        "rust",
        0,
        counts(1, 1, 0, 0),
        Vec::new(),
        None,
        Vec::new(),
        None,
    );
    let summary = build_summary_report(&[report], "cmd", OffsetDateTime::now_utc());
    let json = serde_json::to_string(&summary).expect("缺测面汇总应可序列化");

    assert!(
        json.contains("\"conclusion\":\"fail\""),
        "结论线格式: {json}"
    );
    assert!(
        json.contains("\"type\":\"coverage_failure\""),
        "覆盖归因线格式: {json}"
    );
    assert!(
        !json.contains("\"coverage\""),
        "summary 面零 coverage 键: {json}"
    );
    assert!(
        !json.contains("\"mutation\""),
        "summary 面零 mutation 键: {json}"
    );
}

/// 边界：组合链路 aggregate → reuse（入口 = 本模块）——缺采集 fail 汇总按既有
/// fail 同待遇经 `checks::is_reusable` → `true`（`is_reusable` 只拒 error）；
/// error 结论仍 → `false`。
#[test]
fn 缺采集fail汇总仍可复用() {
    let missing = report_with(
        "crates",
        "rust",
        0,
        counts(1, 1, 0, 0),
        Vec::new(),
        None,
        Vec::new(),
        None,
    );
    let summary = build_summary_report(&[missing], "cmd", OffsetDateTime::now_utc());
    assert_eq!(summary.conclusion, Conclusion::Fail);
    assert!(
        crate::is_reusable(&summary, None),
        "覆盖缺失的 fail 报告按既有 fail 同待遇（复用门三条件守恒）"
    );

    let error_report = report_with(
        "crates",
        "rust",
        1,
        counts(1, 1, 0, 0),
        Vec::new(),
        None,
        Vec::new(),
        None,
    );
    let summary = build_summary_report(&[error_report], "cmd", OffsetDateTime::now_utc());
    assert_eq!(summary.conclusion, Conclusion::Error);
    assert!(
        !crate::is_reusable(&summary, None),
        "error 结论不可复用（既有判定零改动）"
    );
}
