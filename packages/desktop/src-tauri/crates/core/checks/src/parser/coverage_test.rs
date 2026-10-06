use crate::parser::coverage::{parse_coverage, parse_coverage_measured, CoverageFormat};

use crate::parser::js::parse_istanbul_summary;
use crate::parser::rust::parse_llvm_cov;

const ISTANBUL_SAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/istanbul/coverage-summary.json"
));

const LLVM_COV_SAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/llvm-cov/coverage.json"
));

/// 正向：CoverageFormat istanbul 值路由至 istanbul 解析（与子解析器同构样本
/// 同结果——路由层零语义偏移）。
#[test]
fn istanbul路由与子解析器同构同结果() {
    let routed =
        parse_coverage(CoverageFormat::Istanbul, ISTANBUL_SAMPLE).expect("istanbul 路由应解析成功");
    let direct = parse_istanbul_summary(ISTANBUL_SAMPLE).expect("直接解析应成功");
    assert_eq!(routed, direct, "路由结果与子解析器逐条目相等");
    assert_eq!(routed.len(), 2, "语料样本条目数持衡");
}

/// 正向：CoverageFormat llvm-cov 值路由至 llvm-cov 解析（与子解析器同构样本
/// 同结果）。
#[test]
fn llvm_cov路由与子解析器同构同结果() {
    let routed =
        parse_coverage(CoverageFormat::LlvmCov, LLVM_COV_SAMPLE).expect("llvm-cov 路由应解析成功");
    let direct = parse_llvm_cov(LLVM_COV_SAMPLE).expect("直接解析应成功");
    assert_eq!(routed, direct, "路由结果与子解析器逐条目相等");
    assert_eq!(routed.len(), 3, "语料样本条目数持衡");
}

/// 正向：CoverageFormat node-test 值在 V1 支持面路由可达（三值封闭集齐备）：
/// 逐文件计数恒空集（文本表格无逐文件原始计数），实测块自 "All files" 汇总行
/// 提取（`-` 列为该维度不支持 → None）。
#[test]
fn node_test路由可达_逐文件空集与实测块提取() {
    // 逐文件面：恒空集（Ok 空 Vec，不 Err）
    let entries = parse_coverage(
        CoverageFormat::NodeTest,
        "file            | line % | branch % | funcs %\nAll files       |  82.5  |   74.1   |  90.3  |",
    )
    .expect("node-test 路由应可达");
    assert!(entries.is_empty(), "node-test 无逐文件原始计数（V1 形态）");

    // 实测块面："All files" 汇总行 → 三维百分比（`-` 列 → None）
    let table = "| file | line % | branch % | funcs % | unlocs |\n\
                 | All files | 82.5 | 74.1 | 90.3 | 12 |";
    let measured =
        parse_coverage_measured(CoverageFormat::NodeTest, table).expect("All files 汇总行应可提取");
    assert_eq!(measured.lines, Some(82.5));
    assert_eq!(measured.branches, Some(74.1));
    assert_eq!(measured.functions, Some(90.3));

    // `-` 列 = 该维度不支持 → None（null 感知判定面）
    let dashes = "| All files | 82.5 |    -    |     -     |";
    let measured = parse_coverage_measured(CoverageFormat::NodeTest, dashes).expect("应可提取");
    assert_eq!(measured.lines, Some(82.5));
    assert_eq!(measured.branches, None, "`-` 列 → None");
    assert_eq!(measured.functions, None, "`-` 列 → None");

    // 无 "All files" 汇总行 → None（覆盖采集失败语义，判定按 null 感知跳过）
    assert!(parse_coverage_measured(CoverageFormat::NodeTest, "no summary line").is_none());
}

/// 异常：结构残缺样本经各路由 Err 上抛且记因（路由层不吞错不换语义）。
#[test]
fn 结构残缺样本经各路由err上抛且记因() {
    // 非法 JSON
    let err = parse_coverage(CoverageFormat::Istanbul, "{ broken").expect_err("应 Err");
    assert!(
        err.contains("istanbul"),
        "istanbul 路由记因保持，实际: {err}"
    );
    let err = parse_coverage(CoverageFormat::LlvmCov, "{ broken").expect_err("应 Err");
    assert!(
        err.contains("llvm-cov"),
        "llvm-cov 路由记因保持，实际: {err}"
    );

    // 结构残缺（各路由各自的完整性前提）
    let err = parse_coverage(
        CoverageFormat::Istanbul,
        r#"{ "src/a.ts": { "lines": { "pct": 50 } } }"#,
    )
    .expect_err("缺 total 键应 Err");
    assert!(
        err.contains("total"),
        "istanbul 完整性前提记因，实际: {err}"
    );
    let err = parse_coverage(CoverageFormat::LlvmCov, r#"{ "not_data": [] }"#)
        .expect_err("缺 data 键应 Err");
    assert!(err.contains("data"), "llvm-cov 完整性前提记因，实际: {err}");
}

/// 边界：CoverageFormat 三值互异（istanbul / llvm-cov / node-test 构造可辨，
/// 线格式词与 CLI coverage_format 字段同串）。
#[test]
fn 覆盖格式三值互异且线格式词同串() {
    let formats = [
        CoverageFormat::Istanbul,
        CoverageFormat::LlvmCov,
        CoverageFormat::NodeTest,
    ];
    let words: Vec<&str> = formats.iter().map(|format| format.as_str()).collect();
    assert_eq!(
        words,
        vec!["istanbul", "llvm-cov", "node-test"],
        "线格式词逐字"
    );
    let distinct: std::collections::BTreeSet<&str> = words.iter().copied().collect();
    assert_eq!(distinct.len(), 3, "三值互异（封闭集可辨）");

    // 三值路由互异：同输入不同格式 → 不同产物面（istanbul 解析成功、llvm-cov
    // 对同输入 Err、node-test 恒空集）
    let sample = r#"{ "total": { "lines": { "pct": 80 }, "branches": { "pct": 70 }, "functions": { "pct": 75 } } }"#;
    assert!(parse_coverage(CoverageFormat::Istanbul, sample).is_ok());
    assert!(parse_coverage(CoverageFormat::LlvmCov, sample).is_err());
    assert_eq!(
        parse_coverage(CoverageFormat::NodeTest, sample),
        Ok(Vec::new())
    );
}
