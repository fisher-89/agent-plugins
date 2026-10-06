use crate::parser::js::parse_istanbul_summary;

/// 真实 istanbul coverage-summary.json 样本（语料文件，见
/// tests/fixtures/README.md 清单表）。
const ISTANBUL_SAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/istanbul/coverage-summary.json"
));

/// 正向：真实 istanbul coverage-summary 样本（total 键 + 多文件键）→
/// SourceFileEntry 逐文件四维计数逐字对齐。
#[test]
fn 真实istanbul样本逐文件四维计数逐字对齐() {
    let entries = parse_istanbul_summary(ISTANBUL_SAMPLE).expect("真实样本应解析成功");

    assert_eq!(entries.len(), 2, "total 键不产条目，仅两个文件键");

    let main = entries
        .iter()
        .find(|entry| entry.file == "src/app/main.ts")
        .expect("main.ts 条目在场");
    let coverage = main.coverage.as_ref().expect("coverage 在场");
    assert_eq!(coverage.lines, 90.0);
    assert_eq!(coverage.total_lines, Some(20));
    assert_eq!(coverage.covered_lines, Some(18));
    assert_eq!(coverage.branches, Some(75.0));
    assert_eq!(coverage.total_branches, Some(8));
    assert_eq!(coverage.covered_branches, Some(6));
    assert_eq!(coverage.functions, Some(80.0));
    assert_eq!(coverage.total_functions, Some(10));
    assert_eq!(coverage.covered_functions, Some(8));

    let util = entries
        .iter()
        .find(|entry| entry.file == "src/lib/util.ts")
        .expect("util.ts 条目在场");
    let coverage = util.coverage.as_ref().expect("coverage 在场");
    assert_eq!(coverage.lines, 75.0);
    assert_eq!(coverage.total_lines, Some(12));
    assert_eq!(coverage.covered_lines, Some(9));
    assert_eq!(coverage.branches, Some(66.67), "小数 pct 保真");
    assert_eq!(coverage.total_branches, Some(9));
    assert_eq!(coverage.covered_branches, Some(6));
    assert_eq!(coverage.functions, Some(100.0));
    assert_eq!(coverage.total_functions, Some(4));
    assert_eq!(coverage.covered_functions, Some(4));
}

/// 正向：多文件样本全量提取——文件集合与计数逐条对应（顺序无关断言：
/// JSON 键序不构成契约）。
#[test]
fn 多文件样本全量提取_顺序无关() {
    let entries = parse_istanbul_summary(ISTANBUL_SAMPLE).expect("解析应成功");

    let mut files: Vec<&str> = entries.iter().map(|entry| entry.file.as_str()).collect();
    files.sort_unstable();
    assert_eq!(
        files,
        vec!["src/app/main.ts", "src/lib/util.ts"],
        "文件集合全量（无增无减）"
    );

    // 逐条对应：每条目 coverage 在场且 lines 与该文件键的 pct 一致
    for entry in &entries {
        let coverage = entry.coverage.as_ref().expect("逐条 coverage 在场");
        let expected = match entry.file.as_str() {
            "src/app/main.ts" => 90.0,
            "src/lib/util.ts" => 75.0,
            other => panic!("未知条目混入: {other}"),
        };
        assert_eq!(coverage.lines, expected, "文件 {} 计数逐条对应", entry.file);
    }
}

/// 异常：非法 JSON → Err 记因。
#[test]
fn 非法json_err记因() {
    for broken in ["{", "not json", "", "42"] {
        let err = parse_istanbul_summary(broken).expect_err("非法 JSON 应 Err");
        assert!(
            err.starts_with("istanbul 覆盖解析失败:"),
            "Err 记因面可读，实际: {err}"
        );
    }
}

/// 异常：文件键结构残缺（缺 lines 计数字段）→ Err 记因，不产半截条目。
#[test]
fn 文件键结构残缺err不产半截条目() {
    // 文件键缺 lines 块
    let missing_lines_block = r#"{
      "total": { "lines": { "pct": 80 }, "branches": { "pct": 70 }, "functions": { "pct": 75 } },
      "src/a.ts": { "branches": { "pct": 50, "total": 2, "covered": 1 } }
    }"#;
    let err = parse_istanbul_summary(missing_lines_block).expect_err("缺 lines 块应 Err");
    assert!(
        err.contains("src/a.ts") && err.contains("结构残缺"),
        "记因指认残缺文件键，实际: {err}"
    );

    // lines 块缺 pct 计数字段
    let missing_pct = r#"{
      "total": { "lines": { "pct": 80 }, "branches": { "pct": 70 }, "functions": { "pct": 75 } },
      "src/b.ts": { "lines": { "total": 10, "covered": 5 } }
    }"#;
    let err = parse_istanbul_summary(missing_pct).expect_err("缺 pct 应 Err");
    assert!(err.contains("src/b.ts"), "记因指认残缺文件键，实际: {err}");

    // total 键缺三维度 pct（全局实测块的解析前提）同 Err
    let total_missing_pct = r#"{
      "total": { "lines": { "covered": 1, "total": 2 } },
      "src/c.ts": { "lines": { "pct": 50 } }
    }"#;
    let err = parse_istanbul_summary(total_missing_pct).expect_err("total 缺 pct 应 Err");
    assert!(
        err.contains("total.lines"),
        "记因指向 total 维度，实际: {err}"
    );
}

/// 边界：仅 total 键无文件键 → 空条目集（total 键不产条目）且不 panic。
#[test]
fn 仅total键无文件键_空条目集不panic() {
    let total_only = r#"{
      "total": { "lines": { "pct": 83.01 }, "branches": { "pct": 74.95 }, "functions": { "pct": 85.43 } }
    }"#;
    let entries = parse_istanbul_summary(total_only).expect("仅 total 键应解析成功");
    assert!(
        entries.is_empty(),
        "total 键不产条目（全局实测块另走 measured 面）"
    );
}

/// 边界：全零覆盖文件（四维 covered=0）→ 计数 0 保真（不丢不抹平）。
#[test]
fn 全零覆盖文件计数0保真() {
    let zero = r#"{
      "total": { "lines": { "pct": 0 }, "branches": { "pct": 0 }, "functions": { "pct": 0 } },
      "src/never_run.ts": {
        "lines": { "pct": 0, "total": 30, "covered": 0, "skipped": 0 },
        "branches": { "pct": 0, "total": 12, "covered": 0, "skipped": 0 },
        "functions": { "pct": 0, "total": 5, "covered": 0, "skipped": 0 }
      }
    }"#;
    let entries = parse_istanbul_summary(zero).expect("全零覆盖应解析成功");
    assert_eq!(entries.len(), 1);
    let coverage = entries[0].coverage.as_ref().expect("coverage 在场");
    assert_eq!(entries[0].file, "src/never_run.ts");
    assert_eq!(coverage.lines, 0.0, "lines 0 保真");
    assert_eq!(coverage.total_lines, Some(30));
    assert_eq!(coverage.covered_lines, Some(0), "covered 0 不抹平为缺失");
    assert_eq!(coverage.branches, Some(0.0));
    assert_eq!(coverage.total_branches, Some(12));
    assert_eq!(coverage.covered_branches, Some(0));
    assert_eq!(coverage.functions, Some(0.0));
    assert_eq!(coverage.total_functions, Some(5));
    assert_eq!(coverage.covered_functions, Some(0));
}
