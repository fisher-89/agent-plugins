use crate::parser::rust::parse_llvm_cov;

/// 真实 llvm-cov JSON 样本（语料文件，见 tests/fixtures/README.md 清单表）：
/// 双 data 窗口，窗口 1 含多 segment 文件与空 segments 文件 + totals 块。
const LLVM_COV_SAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/llvm-cov/coverage.json"
));

/// 正向：真实 llvm-cov JSON 样本 → 逐文件覆盖计数（行覆盖由 segments 推导：
/// hasCount 段所在行计总数，count > 0 者计覆盖；分支 / 函数维度恒 None）。
#[test]
fn 真实llvm_cov样本segments推导行覆盖() {
    let entries = parse_llvm_cov(LLVM_COV_SAMPLE).expect("真实样本应解析成功");

    let lib = entries
        .iter()
        .find(|entry| entry.file == "crates/core/checks/src/lib.rs")
        .expect("lib.rs 条目在场");
    let coverage = lib.coverage.as_ref().expect("coverage 在场");
    // segments: 行 1（count 4）、行 2（count 4）、行 3（count 0）、行 4（count 0）
    // 均带 hasCount → 总数 4；行 1 的 [1,20,0,false] 段无 hasCount 不计；
    // 覆盖行 1、2 → 2/4 = 50%
    assert_eq!(coverage.lines, 50.0, "行覆盖由 segments 推导（2/4）");
    assert_eq!(coverage.total_lines, Some(4));
    assert_eq!(coverage.covered_lines, Some(2));
    assert_eq!(coverage.branches, None, "llvm-cov 不产逐文件分支计数");
    assert_eq!(coverage.functions, None, "llvm-cov 不产逐文件函数计数");
    assert_eq!(coverage.total_branches, None);
    assert_eq!(coverage.covered_branches, None);
    assert_eq!(coverage.total_functions, None);
    assert_eq!(coverage.covered_functions, None);

    // 空 segments 文件：计数 0 边界保真（不 panic 不丢条目）
    let model = entries
        .iter()
        .find(|entry| entry.file == "crates/core/checks/src/model.rs")
        .expect("model.rs 条目在场（空 segments 不丢条目）");
    let coverage = model.coverage.as_ref().expect("coverage 在场");
    assert_eq!(coverage.lines, 0.0);
    assert_eq!(coverage.total_lines, Some(0));
    assert_eq!(coverage.covered_lines, Some(0));
}

/// 正向：多文件多 data 窗口样本全量提取（窗口顺序拼接、totals 块不产条目）。
#[test]
fn 多文件多data窗口全量提取() {
    let entries = parse_llvm_cov(LLVM_COV_SAMPLE).expect("解析应成功");

    // 两个 data 窗口的文件按窗口序拼接，共三文件条目
    let files: Vec<&str> = entries.iter().map(|entry| entry.file.as_str()).collect();
    assert_eq!(
        files,
        vec![
            "crates/core/checks/src/lib.rs",
            "crates/core/checks/src/model.rs",
            "crates/core/checks/src/aggregate.rs",
        ],
        "多窗口全量提取且保序"
    );

    // 第二窗口单 segment（count 1）→ 单行全覆盖 100%
    let aggregate = &entries[2];
    let coverage = aggregate.coverage.as_ref().expect("coverage 在场");
    assert_eq!(coverage.lines, 100.0);
    assert_eq!(coverage.total_lines, Some(1));
    assert_eq!(coverage.covered_lines, Some(1));
}

/// 异常：非法 JSON → Err 记因。
#[test]
fn 非法json_err记因() {
    for broken in ["{", "not json", "", "[]"] {
        let err = parse_llvm_cov(broken).expect_err("非法 JSON 应 Err");
        assert!(
            err.starts_with("llvm-cov 覆盖解析失败:"),
            "Err 记因面可读，实际: {err}"
        );
    }
}

/// 异常：结构残缺（缺 data / files 键）→ Err 记因。
#[test]
fn 结构残缺缺data或files键err记因() {
    // 顶层缺 data 键
    let no_data = r#"{ "totals": { "lines": { "percent": 75.0 } } }"#;
    let err = parse_llvm_cov(no_data).expect_err("缺 data 键应 Err");
    assert!(err.contains("data"), "记因指向缺失键，实际: {err}");

    // data 窗口缺 files 键
    let no_files = r#"{ "data": [ { "totals": {} } ] }"#;
    let err = parse_llvm_cov(no_files).expect_err("缺 files 键应 Err");
    assert!(err.contains("files"), "记因指向缺失键，实际: {err}");

    // 文件条目缺 filename
    let no_filename = r#"{ "data": [ { "files": [ { "segments": [] } ] } ] }"#;
    let err = parse_llvm_cov(no_filename).expect_err("缺 filename 应 Err");
    assert!(err.contains("filename"), "记因指向缺失键，实际: {err}");

    // 文件条目缺 segments 键
    let no_segments = r#"{ "data": [ { "files": [ { "filename": "a.rs" } ] } ] }"#;
    let err = parse_llvm_cov(no_segments).expect_err("缺 segments 键应 Err");
    assert!(err.contains("segments"), "记因指向缺失键，实际: {err}");
}

/// 边界：空 files 数组 → 空条目集不 panic。
#[test]
fn 空files数组_空条目集不panic() {
    let empty = r#"{ "data": [ { "files": [] } ] }"#;
    let entries = parse_llvm_cov(empty).expect("空 files 应解析成功");
    assert!(entries.is_empty());

    let multiple_empty = r#"{ "data": [ { "files": [] }, { "files": [] } ] }"#;
    let entries = parse_llvm_cov(multiple_empty).expect("多空窗口应解析成功");
    assert!(entries.is_empty());
}

/// 边界：无 segments / 单 segment 文件 → 计数 0 边界保真。
#[test]
fn 无segments或单segment计数0边界保真() {
    // 单 segment 但 count = 0（未覆盖单行）
    let uncovered_single = r#"{ "data": [ { "files": [
      { "filename": "src/a.rs", "segments": [[1, 1, 0, true]] }
    ] } ] }"#;
    let entries = parse_llvm_cov(uncovered_single).expect("解析应成功");
    let coverage = entries[0].coverage.as_ref().expect("coverage 在场");
    assert_eq!(
        coverage.lines, 0.0,
        "单行未覆盖 → 0%（total 1 / covered 0）"
    );
    assert_eq!(coverage.total_lines, Some(1));
    assert_eq!(coverage.covered_lines, Some(0));

    // hasCount=false 的段不计数（region 边界段形态）→ 计数 0
    let no_count_segments = r#"{ "data": [ { "files": [
      { "filename": "src/b.rs", "segments": [[1, 1, 0, false], [2, 1, 0, false]] }
    ] } ] }"#;
    let entries = parse_llvm_cov(no_count_segments).expect("解析应成功");
    let coverage = entries[0].coverage.as_ref().expect("coverage 在场");
    assert_eq!(coverage.lines, 0.0, "无 hasCount 段 → 计数 0 保真不 panic");
    assert_eq!(coverage.total_lines, Some(0));
    assert_eq!(coverage.covered_lines, Some(0));
}
