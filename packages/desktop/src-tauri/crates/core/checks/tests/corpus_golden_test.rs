//! 集成测试：语料 fixtures → 解析 → 聚合 → 诊断全链 → golden 快照（AC-8 全链
//! 黄金对拍；AC-1 core/checks crate 注册编译证据）。
//!
//! 防线机制：对每个 corpus fixture 跑「宽容解析 → 完整性 checklist → 诊断树 →
//! 覆盖聚合」全链，投影为 JSON 与入仓 golden 全量对比；损坏样本的投影记录
//! unparsable 标记（宽容解析不悄悄吞数据在 golden 上可见——workflow
//! corpus_golden_test 先例同款）。istanbul / llvm-cov / node-test / 红跑 /
//! mutation 块 / 未知字段 / 损坏族八套语料齐备（清单见 fixtures/README.md）。
//!
//! ## golden 重写开关
//!
//! 环境变量 `DESKTOP_GOLDEN_REWRITE=1` 时，投影不再对比而是覆写入仓 golden。
//! 预期工作流（DESKTOP_GOLDEN_REWRITE 先例同款，env 开关 + 复核两步工作流）：
//!
//! ```text
//! DESKTOP_GOLDEN_REWRITE=1 cargo test -p checks --test corpus_golden_test   # 重写
//! cargo test -p checks --test corpus_golden_test                            # 复核：与现 golden 等价
//! ```
//!
//! golden 期望值基准：CLI zod 权威（plugins/dev-team/bin/src/schemas/
//! test-execution-output.schema.ts）产出形态录制；语料样本采集自真实工作区
//! 产出形态（vite-plus / rust / node-test 三档绿跑 + 红跑 + 特殊样本族）。

use std::fs;
use std::path::PathBuf;

use checks::aggregate::aggregate_coverage;
use checks::model::{parse_sub_report, parse_summary_report};
use checks::parser::parse_spec_report;
use checks::{check_integrity, diagnose_findings};
use config::{MutationConfig, TestFramework};

/// golden 重写开关环境变量（workflow corpus_golden_test 先例同款定名）。
const REWRITE_ENV: &str = "DESKTOP_GOLDEN_REWRITE";

/// corpus 语料清单（对应 fixtures/README.md 清单表；八套 ≥ 规模下限）。
const CORPUS_FIXTURES: &[&str] = &[
    "green-istanbul",
    "green-llvm-cov",
    "green-node-test",
    "red-run",
    "mutation-block",
    "unknown-fields",
    "corrupt-invalid-json",
    "corrupt-missing-required",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn corpus_root() -> PathBuf {
    manifest_dir().join("tests").join("fixtures").join("corpus")
}

fn golden_dir() -> PathBuf {
    manifest_dir().join("tests").join("golden")
}

fn rewrite_mode() -> bool {
    std::env::var(REWRITE_ENV).is_ok_and(|value| value != "0" && !value.is_empty())
}

/// 语料对应的 config::TestSuite 集（suite 阈值随语料档位；config::TestSuite
/// 为 Serialize-only 类型，Rust 侧显式构造——清单同步维护于 fixtures/README.md）。
fn suites_for(fixture: &str) -> Vec<config::TestSuite> {
    let thresholds = |lines: f64, branches: f64, functions: f64| config::CoverageThresholds {
        lines,
        branches,
        functions,
    };
    let suite = |root: &str, framework: TestFramework, coverage| config::TestSuite {
        root: root.to_owned(),
        framework,
        cwd: ".".to_owned(),
        config: None,
        includes: None,
        excludes: None,
        coverage,
        mutation: MutationConfig {
            cwd: None,
            score: 70.0,
        },
    };
    match fixture {
        "green-istanbul" | "red-run" => vec![suite(
            ".",
            TestFramework::VitePlus,
            thresholds(80.0, 70.0, 75.0),
        )],
        "green-llvm-cov" => vec![suite(
            "crates/core",
            TestFramework::Rust,
            thresholds(70.0, 65.0, 60.0),
        )],
        "green-node-test" => vec![suite(
            ".",
            TestFramework::NodeTest,
            thresholds(60.0, 70.0, 75.0),
        )],
        // 无 summary 面（无聚合消费）的解析保真样本：suites 不参与投影
        _ => Vec::new(),
    }
}

/// 单个 corpus fixture 的全链投影：宽容解析 → 完整性 checklist → 诊断树 →
/// 覆盖聚合（确定性字段全量——时间戳等墙钟字段不入投影）。
fn project_corpus_fixture(fixture: &str) -> serde_json::Value {
    let dir = corpus_root().join(fixture);
    let sub_text = fs::read_to_string(dir.join("sub-report.json"))
        .expect("语料 fixture sub-report.json 应存在");

    let mut projection = serde_json::json!({ "fixture": fixture });

    // ① 子报告宽容解析（unparsable 标记可见——不悄悄吞数据）；损坏语料无
    // summary 面（完整性 checklist 以子报告全集为输入，解析失败即全链降级）
    let sub = match parse_sub_report(&sub_text) {
        Err(_) => {
            projection["subReport"] = serde_json::json!({ "outcome": "unparsable" });
            return projection;
        }
        Ok(sub) => sub,
    };

    let mut sub_projection = serde_json::json!({
        "outcome": "parsed",
        "framework": sub.framework,
        "root": sub.root,
        "exitCode": sub.exit_code,
        "summaryCounts": [sub.summary.total, sub.summary.passed, sub.summary.failed, sub.summary.skipped],
        "errorCases": sub.error_cases.len(),
        "testFiles": sub.test_files,
        "sourceFiles": sub.source_files.iter().map(|entry| entry.file.clone()).collect::<Vec<_>>(),
        "coveragePresent": sub.coverage.is_some(),
    });
    // mutation 块保真面（生产聚合恒 null 与解析保真两半分离）
    match &sub.mutation {
        None => sub_projection["mutation"] = serde_json::json!(null),
        Some(mutation) => {
            sub_projection["mutation"] = serde_json::json!({
                "pass": mutation.pass,
                "score": mutation.score,
                "threshold": mutation.threshold,
                "measured": [mutation.measured.killed, mutation.measured.survived,
                             mutation.measured.timeout, mutation.measured.no_coverage,
                             mutation.measured.total, mutation.measured.detected,
                             mutation.measured.undetected],
                "overrides": mutation.overrides.as_ref().map(|entries| entries.len()),
            });
        }
    }
    projection["subReport"] = sub_projection;

    // ② summary 面：完整性 checklist + 诊断树 + 计算侧覆盖聚合（绿跑三档与
    // 红跑语料；本语料族均为单子报告形态）
    if let Ok(summary_text) = fs::read_to_string(dir.join("summary.json")) {
        match parse_summary_report(&summary_text) {
            Err(_) => {
                projection["summary"] = serde_json::json!({ "outcome": "unparsable" });
            }
            Ok(summary) => {
                let own = vec![sub];
                projection["summary"] = serde_json::json!({
                    "outcome": "parsed",
                    "conclusion": summary.conclusion.as_str(),
                    "counts": [summary.total, summary.passed, summary.failed, summary.skipped],
                    "problemTypes": summary.problems.iter().map(|problem| match problem.problem_type {
                        checks::model::ProblemType::TestFailure => "test_failure",
                        checks::model::ProblemType::CoverageFailure => "coverage_failure",
                        checks::model::ProblemType::ExecutionError => "execution_error",
                    }).collect::<Vec<_>>(),
                    "integrity": check_integrity(&summary, &own),
                    "diagnose": diagnose_findings(&summary),
                });

                // 计算侧聚合对拍：解析语料重算覆盖块与 summary 记载一致
                let computed = aggregate_coverage(&own, &suites_for(fixture));
                projection["computedCoverage"] = computed
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .expect("CoverageBlock 序列化应成功")
                    .unwrap_or(serde_json::Value::Null);
            }
        }
    }

    projection
}

/// 对比或覆写单份 golden（workflow corpus_golden_test 先例同款）。
fn check_or_rewrite(golden_name: &str, projection: &serde_json::Value) {
    let normalized = format!(
        "{}\n",
        serde_json::to_string_pretty(projection).expect("投影序列化失败")
    );
    let golden_path = golden_dir().join(format!("{golden_name}.json"));
    if rewrite_mode() {
        fs::create_dir_all(golden_dir()).expect("创建 golden 目录失败");
        fs::write(&golden_path, &normalized).expect("写 golden 失败");
        return;
    }
    let expected = fs::read_to_string(&golden_path).unwrap_or_else(|err| {
        panic!("golden {golden_name}.json 缺失（先以 {REWRITE_ENV}=1 生成）: {err}")
    });
    assert_eq!(
        normalized, expected,
        "fixture {golden_name} 的全链投影与 golden 漂移；若为有意的 schema 演进，以 {REWRITE_ENV}=1 重写并 review diff"
    );
}

macro_rules! corpus_golden_test {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            check_or_rewrite($fixture, &project_corpus_fixture($fixture));
        }
    };
}

corpus_golden_test!(golden_green_istanbul, "green-istanbul");
corpus_golden_test!(golden_green_llvm_cov, "green-llvm-cov");
corpus_golden_test!(golden_green_node_test, "green-node-test");
corpus_golden_test!(golden_red_run, "red-run");
corpus_golden_test!(golden_mutation_block, "mutation-block");
corpus_golden_test!(golden_unknown_fields, "unknown-fields");
corpus_golden_test!(golden_corrupt_invalid_json, "corrupt-invalid-json");
corpus_golden_test!(golden_corrupt_missing_required, "corrupt-missing-required");

// ---------------------------------------------------------------------------
// 结论级断言（golden 之上的人读锚——AC-8 全链 conclusion 面）
// ---------------------------------------------------------------------------

/// istanbul 族绿跑真实 summary + report fixtures → 解析 → 聚合 → 诊断全链
/// conclusion=pass 且与 golden 期望逐字段一致（golden 行 1 的结论级锚）。
#[test]
fn 全链绿跑istanbul_conclusion_pass且golden一致() {
    let projection = project_corpus_fixture("green-istanbul");
    assert_eq!(projection["subReport"]["outcome"], "parsed");
    assert_eq!(projection["summary"]["conclusion"], "pass");
    assert_eq!(projection["summary"]["integrity"], serde_json::json!([]));
    assert_eq!(
        projection["summary"]["diagnose"],
        serde_json::json!(["全部测试通过且覆盖率达阈值"]),
        "全绿零误报：仅 4e 正向单条"
    );
    check_or_rewrite("green-istanbul", &projection);
}

/// llvm-cov 绿跑 fixtures → 同链 conclusion 与聚合面一致（src-tauri rust 档）。
#[test]
fn 全链绿跑llvm_cov_conclusion_pass且聚合面一致() {
    let projection = project_corpus_fixture("green-llvm-cov");
    assert_eq!(projection["summary"]["conclusion"], "pass");
    assert_eq!(projection["summary"]["integrity"], serde_json::json!([]));
    // 计算侧聚合与 summary 记载一致：lines 71.43 ≥ 70（branches / functions
    // null 维度感知跳过）
    assert_eq!(projection["computedCoverage"]["pass"], true);
    assert_eq!(projection["computedCoverage"]["measured"]["lines"], 71.43);
    assert_eq!(
        projection["computedCoverage"]["measured"]["branches"],
        serde_json::json!(null)
    );
    check_or_rewrite("green-llvm-cov", &projection);
}

/// node-test 文本 fixtures → text 解析计数聚合与 golden 一致（spec 文本五用例
/// 4 pass 1 skipped）。
#[test]
fn 全链node_test文本解析计数与golden一致() {
    let text = include_str!("fixtures/node-spec/output.txt");
    let cases = parse_spec_report(text).expect("绿跑 spec 文本应解析成功");
    let count = |status: checks::model::TestCaseStatus| {
        cases.iter().filter(|case| case.status == status).count()
    };
    assert_eq!(cases.len(), 5);
    assert_eq!(count(checks::model::TestCaseStatus::Passed), 4);
    assert_eq!(count(checks::model::TestCaseStatus::Failed), 0);
    assert_eq!(count(checks::model::TestCaseStatus::Skipped), 1);

    let projection = project_corpus_fixture("green-node-test");
    assert_eq!(projection["summary"]["conclusion"], "pass");
    assert_eq!(
        projection["subReport"]["summaryCounts"],
        serde_json::json!([5, 4, 0, 1])
    );
    check_or_rewrite("green-node-test", &projection);
}

/// 红跑 fixtures（含 test_failure problems）→ conclusion=fail 且 findings 与
/// CLI 权威一致（聚类措辞、计数对账零违例）。
#[test]
fn 全链红跑_conclusion_fail且findings一致() {
    let projection = project_corpus_fixture("red-run");
    assert_eq!(projection["summary"]["conclusion"], "fail");
    assert_eq!(
        projection["summary"]["problemTypes"],
        serde_json::json!(["test_failure", "test_failure"]),
        "失败用例逐条 test_failure 归并"
    );
    assert_eq!(
        projection["summary"]["integrity"],
        serde_json::json!([]),
        "红跑语料自洽（零完整性违例）"
    );
    assert_eq!(
        projection["summary"]["diagnose"],
        serde_json::json!([
            "「vite-plus」2 项测试失败——聚类失败（疑似同模块回归或共同假设变化）; 明细见子报告 error_cases"
        ]),
        "诊断树 4b 聚类措辞与 CLI 权威一致"
    );
    check_or_rewrite("red-run", &projection);
}

/// 损坏样本（非法 JSON / 必填缺失）→ 宽容解析不悄悄吞数据：unparsable 标记与
/// 跳过统计在 golden 上可见（workflow corpus_golden_test 先例同款）。
#[test]
fn 损坏样本unparsable标记在golden可见() {
    for fixture in ["corrupt-invalid-json", "corrupt-missing-required"] {
        let projection = project_corpus_fixture(fixture);
        assert_eq!(
            projection["subReport"]["outcome"], "unparsable",
            "{fixture} 宽容解析不产半截子报告（unparsable 标记可见）"
        );
        assert!(
            projection.get("summary").is_none(),
            "{fixture} 无 summary 面（解析降级统计只走子报告面）"
        );
        check_or_rewrite(fixture, &projection);
    }

    // 跳过统计：八套语料中恰两套 unparsable，其余六套 parsed
    let parsed = CORPUS_FIXTURES
        .iter()
        .filter(|fixture| project_corpus_fixture(fixture)["subReport"]["outcome"] == "parsed")
        .count();
    assert_eq!(
        parsed, 6,
        "六套 parsed + 两套 unparsable = 八套全量（零静默丢弃）"
    );
}

/// 含真实 mutation 块样本 → mutation 位保真解析不丢块（解析保真与生产聚合恒
/// null 两半分离）。
#[test]
fn mutation块语料保真解析不丢块() {
    let projection = project_corpus_fixture("mutation-block");
    assert_eq!(projection["subReport"]["outcome"], "parsed");
    assert_eq!(projection["subReport"]["coveragePresent"], true);
    let mutation = &projection["subReport"]["mutation"];
    assert_eq!(mutation["pass"], false);
    assert_eq!(mutation["score"], 55.0);
    assert_eq!(mutation["threshold"], 70.0);
    assert_eq!(
        mutation["measured"],
        serde_json::json!([11, 7, 2, 3, 27, 13, 10]),
        "实测计数逐字段保真（killed / survived / timeout / noCoverage / total / detected / undetected）"
    );
    assert_eq!(
        mutation["overrides"],
        serde_json::json!(1),
        "override 组不丢块"
    );
    check_or_rewrite("mutation-block", &projection);
}

/// 未知字段样本 → 宽容忽略对拍一致（已知字段零丢失）。
#[test]
fn 未知字段语料宽容忽略对拍一致() {
    let projection = project_corpus_fixture("unknown-fields");
    assert_eq!(
        projection["subReport"]["outcome"], "parsed",
        "未知字段不拒识"
    );
    assert_eq!(
        projection["subReport"]["framework"], "node-test",
        "已知字段不丢"
    );
    assert_eq!(
        projection["subReport"]["summaryCounts"],
        serde_json::json!([3, 3, 0, 0])
    );
    check_or_rewrite("unknown-fields", &projection);
}

// ---------------------------------------------------------------------------
// 语料完整性（fixtures / golden 目录与清单一致——workflow 先例同款）
// ---------------------------------------------------------------------------

#[test]
fn 语料完整性_corpus目录与清单表逐项对应() {
    let mut actual: Vec<String> = fs::read_dir(corpus_root())
        .expect("corpus 目录应存在")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    actual.sort();
    let mut expected: Vec<String> = CORPUS_FIXTURES.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(
        actual, expected,
        "corpus 目录与清单表不一致（防样本被静默删减）；fixtures/README.md 清单需同步维护"
    );
    // 语料说明文档在位
    assert!(
        manifest_dir()
            .join("tests")
            .join("fixtures")
            .join("README.md")
            .is_file(),
        "fixtures/README.md 应存在"
    );
}

#[test]
fn 语料完整性_golden目录与语料集合一致() {
    if rewrite_mode() {
        // 重写模式下 golden 目录正在被覆写，跳过该一致性断言（复核轮再验）
        return;
    }
    let mut expected: Vec<String> = CORPUS_FIXTURES
        .iter()
        .map(|fixture| format!("{fixture}.json"))
        .collect();
    expected.push("README.md".to_string());
    expected.sort();

    let mut actual: Vec<String> = fs::read_dir(golden_dir())
        .expect("golden 目录应存在（先以 DESKTOP_GOLDEN_REWRITE=1 生成）")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    actual.sort();
    assert_eq!(actual, expected, "golden 文件集应与语料集合一致");
}

// ---------------------------------------------------------------------------
// 段级 mutation 语料对拍（desktop-fix-mutation-test / AC-9）：fixtures/
// mutation-seg/{cargo,stryker} → parse_*_seg → merge_mutation_blocks →
// build_mutation_block → 归并块结论（score / threshold / pass）投影与 golden
// 等值；映射表样本与未知 outcome（runtimeError 留痕）在案
// ---------------------------------------------------------------------------

/// 段级语料清单（独立子树——不入 corpus 八套清单，README 独立维护）。
const MUTATION_SEG_FIXTURES: &[&str] = &["mutation-seg-cargo", "mutation-seg-stryker"];

fn mutation_seg_root() -> PathBuf {
    manifest_dir()
        .join("tests")
        .join("fixtures")
        .join("mutation-seg")
}

fn mutation_seg_golden_dir() -> PathBuf {
    manifest_dir().join("tests").join("golden-mutation-seg")
}

/// 段级语料全链投影：宽容解析（映射表归位）→ 计数归并（单段）→ 归并块结论
/// 投影（确定性字段——duration_ms 取段耗时、计数七键 + score / threshold /
/// pass）。
fn project_seg_fixture(fixture: &str) -> serde_json::Value {
    let raw = fs::read_to_string(mutation_seg_root().join(fixture).join("seg-report.json"))
        .expect("段级 fixture seg-report.json 应存在");
    let parsed = match fixture {
        "mutation-seg-cargo" => checks::model::parse_cargo_mutants_seg(&raw),
        "mutation-seg-stryker" => checks::model::parse_stryker_seg(&raw),
        other => panic!("段级 fixture {other} 未路由解析器"),
    };
    let report = parsed.unwrap_or_else(|error| panic!("{fixture} 解析失败: {error}"));
    let threshold = 70.0;
    let block = checks::aggregate::build_mutation_block(report.measured, threshold);
    let merged =
        checks::aggregate::merge_mutation_blocks(std::slice::from_ref(&block)).expect("单段归并非空");
    serde_json::json!({
        "fixture": fixture,
        "tool": report.tool.as_str(),
        "durationMs": report.duration_ms,
        "counts": [
            merged.measured.killed,
            merged.measured.survived,
            merged.measured.timeout,
            merged.measured.no_coverage,
            merged.measured.compile_error,
            merged.measured.runtime_error,
            merged.measured.ignored,
            merged.measured.total,
        ],
        "score": merged.score,
        "threshold": merged.threshold,
        "pass": merged.pass,
    })
}

/// 对比或覆写单份段级 golden（corpus 先例同款——独立目录独立开关）。
fn check_or_rewrite_seg(golden_name: &str, projection: &serde_json::Value) {
    let normalized = format!(
        "{}\n",
        serde_json::to_string_pretty(projection).expect("投影序列化失败")
    );
    let golden_path = mutation_seg_golden_dir().join(format!("{golden_name}.json"));
    if rewrite_mode() {
        fs::create_dir_all(mutation_seg_golden_dir()).expect("创建段级 golden 目录失败");
        fs::write(&golden_path, &normalized).expect("写段级 golden 失败");
        return;
    }
    let expected = fs::read_to_string(&golden_path).unwrap_or_else(|err| {
        panic!(
            "段级 golden {golden_name}.json 缺失（先以 {REWRITE_ENV}=1 生成）: {err}"
        )
    });
    assert_eq!(
        normalized, expected,
        "fixture {golden_name} 的段级投影与 golden 漂移；若为有意演进，以 {REWRITE_ENV}=1 重写并 review diff"
    );
}

/// 正向：cargo-mutants 段级 fixture → `parse_cargo_mutants_seg` → 计数归并 →
/// 归并块结论投影与 golden 等值（AC-9 映射表样本 + 段级报告 fixtures）。
#[test]
fn golden_mutation_seg_cargo_映射表样本全链投影() {
    let projection = project_seg_fixture("mutation-seg-cargo");
    assert_eq!(projection["tool"], "cargo_mutants");
    // 映射表样本：4 Caught + 1 TIMEOUT → killed=4, timeout=1；missed=2 → survived
    // unviable=1 → compileError；not-mutated×2 → ignored=2；未知 outcome + 缺
    // outcome 字段 → runtimeError=2
    assert_eq!(
        projection["counts"],
        serde_json::json!([4, 2, 1, 0, 1, 2, 2, 12]),
        "映射表逐位归位（killed / survived / timeout / noCoverage / compileError / runtimeError / ignored / total）"
    );
    // score = (4+1)/(12-1-2-2) = 5/7 = 71.43 → pass (>= 70)
    assert_eq!(projection["score"], 71.43, "score 公式单点（两位舍入）");
    assert_eq!(projection["threshold"], 70.0);
    assert_eq!(projection["pass"], true);
    check_or_rewrite_seg("mutation-seg-cargo", &projection);
}

/// 正向：Stryker 段级 fixture → `parse_stryker_seg`（metrics 面优先）→ 归并
/// 结论投影与 golden 等值；未知 status 留痕 runtimeError（宽容不丢数据）。
#[test]
fn golden_mutation_seg_stryker_metrics面全链投影() {
    let projection = project_seg_fixture("mutation-seg-stryker");
    assert_eq!(projection["tool"], "stryker");
    // metrics 面显式在案（files 面不参与计数——metrics 优先口径）
    assert_eq!(
        projection["counts"],
        serde_json::json!([18, 4, 2, 1, 0, 1, 3, 29]),
        "metrics 面逐位在案"
    );
    // score = (18+2)/(29-3-0-1) = 20/25 = 80.0 → pass
    assert_eq!(projection["score"], 80.0);
    assert_eq!(projection["pass"], true);
    check_or_rewrite_seg("mutation-seg-stryker", &projection);
}

/// 边界：未知 outcome 样本投影含 runtimeError 计数（宽容不丢数据留痕——cargo
/// fixture 的 surprising-outcome + 缺 outcome 条目恰 2 计入 runtimeError）。
#[test]
fn 段级未知outcome样本runtimeError留痕() {
    let cargo = project_seg_fixture("mutation-seg-cargo");
    assert_eq!(
        cargo["counts"][5], 2,
        "未知 outcome + 缺 outcome 字段条目 → runtimeError=2（留痕不丢）"
    );
    let stryker = project_seg_fixture("mutation-seg-stryker");
    // stryker metrics 面 runtimeErrors=1 显式在案（files 面的 surprised-status
    // 不参与 metrics 优先计数）
    assert_eq!(stryker["counts"][5], 1, "metrics 面 runtimeErrors 显式在案");
}

/// golden 重写纪律（异常）：对拍差异 → 测试失败并提示；仅
/// `DESKTOP_GOLDEN_REWRITE=1` 显式重写（禁止静默重写，AC-9）——以重写开关
/// 缺省下 golden 文件在位断言承载。
#[test]
fn 段级golden重写纪律_开关缺省下文件在位() {
    if rewrite_mode() {
        // 重写模式下目录正在被覆写，跳过在位断言（复核轮再验）
        return;
    }
    for fixture in MUTATION_SEG_FIXTURES {
        let path = mutation_seg_golden_dir().join(format!("{fixture}.json"));
        assert!(
            path.is_file(),
            "段级 golden {fixture}.json 应在位（先以 {REWRITE_ENV}=1 生成）"
        );
    }
}

/// 语料清单（边界）：段级语料 ≥ 规模下限（两套：cargo 映射表 + stryker
/// metrics 面）；fixture 目录与清单一致（防样本被静默删减）；损坏面口径——
/// 解析失败显式 panic 不静默（结构面严格）。
#[test]
fn 段级语料清单_目录与清单一致且规模下限() {
    let mut actual: Vec<String> = fs::read_dir(mutation_seg_root())
        .expect("mutation-seg 目录应存在")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    actual.sort();
    let mut expected: Vec<String> = MUTATION_SEG_FIXTURES.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(actual, expected, "mutation-seg 目录与清单表不一致");
    assert!(
        MUTATION_SEG_FIXTURES.len() >= 2,
        "段级语料规模下限（≥2 套：cargo 映射表 + stryker metrics）"
    );

    // 损坏面口径：非 JSON seg-report → 解析显式 Err（结构面严格——corpus
    // 损坏族「解析不 panic、显式降级」同口径的严格半边）
    let err = checks::model::parse_cargo_mutants_seg("{ truncated")
        .expect_err("损坏 seg-report 应 Err");
    assert!(!err.is_empty(), "Err 记因非空（不 panic 不静默）");
    let err = checks::model::parse_stryker_seg("")
        .expect_err("空 seg-report 应 Err");
    assert!(!err.is_empty());
}
