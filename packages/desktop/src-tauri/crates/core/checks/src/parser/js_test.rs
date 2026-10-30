use crate::model::TestCaseStatus;
use crate::parser::js::{parse_istanbul_results, parse_istanbul_summary};

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

// ---------------------------------------------------------------------------
// parse_istanbul_results（jest / vitest / vite-plus 的 --reporter=json 结果面）
// ---------------------------------------------------------------------------

/// 真实采集的 vite-plus `--reporter=json` 结果样本（1 通过 + 1 失败 + 1 跳过）。
const ISTANBUL_RESULTS_RED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/istanbul/results.json"
));

/// 真实采集的文件级失败样本：一个「用例内抛错」（有断言行）+ 一个「转译失败」
/// （零断言行、`message` 带 Transform 错误）。
const ISTANBUL_RESULTS_FILE_FAILURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/istanbul/results-file-failure.json"
));

/// 正向：三态映射与字段面（`fullName` 保层级、`duration` 已是毫秒、
/// `failureMessages` 首行拆类型 / 消息且全量入堆栈）。
#[test]
fn istanbul结果三态与失败归因逐字对齐() {
    let parsed = parse_istanbul_results(ISTANBUL_RESULTS_RED).expect("真实样本应解析成功");

    let counts = |status: TestCaseStatus| {
        parsed
            .cases
            .iter()
            .filter(|case| case.status == status)
            .count()
    };
    assert_eq!(
        (
            counts(TestCaseStatus::Passed),
            counts(TestCaseStatus::Failed),
            counts(TestCaseStatus::Skipped)
        ),
        (1, 1, 1),
        "passed / failed / skipped 三态各一"
    );
    assert_eq!(parsed.cases.len(), 3, "全量用例行");
    assert!(parsed.file_failures.is_empty(), "无文件级失败");
    assert!(parsed.mismatch.is_none(), "numTotalTests = 3 与用例数一致");

    let failed = parsed
        .cases
        .iter()
        .find(|case| case.status == TestCaseStatus::Failed)
        .expect("失败用例在场");
    assert_eq!(
        failed.name, "probe suite 套件 probe failing case 失败用例",
        "fullName 保层级（ancestorTitles + title）"
    );
    assert_eq!(
        failed.file.as_deref(),
        Some("D:/ws/src/failing.test.ts"),
        "文件路径取 testResults[].name"
    );
    assert_eq!(
        failed.error_type.as_deref(),
        Some("AssertionError"),
        "failureMessages 首行按首个冒号拆类型"
    );
    assert_eq!(
        failed.error_message.as_deref(),
        Some("expected 1 to be 2 // Object.is equality"),
        "冒号后为消息段"
    );
    assert!(
        failed
            .stack_trace
            .as_deref()
            .is_some_and(|stack| stack.contains("failing.test.ts:9")),
        "failureMessages 全量入堆栈，实际: {:?}",
        failed.stack_trace
    );
    // duration 是毫秒，不乘 1000（与 libtest-json 的秒口径不同）
    assert_eq!(failed.duration_ms, Some(3.0880999999999403));

    let skipped = parsed
        .cases
        .iter()
        .find(|case| case.status == TestCaseStatus::Skipped)
        .expect("跳过用例在场");
    assert!(
        skipped.duration_ms.is_none(),
        "skipped 断言的 duration 为 null → None"
    );
    assert!(skipped.error_type.is_none(), "非失败用例零错误面");
}

/// 异常（关键面）：文件级失败必须与「零用例」区分开——转译失败的文件
/// `assertionResults` 为空且带 `message`，若并入用例行会被零用例守卫误判成
/// 「报告器写空」。
#[test]
fn istanbul文件级失败与零用例可区分() {
    let parsed =
        parse_istanbul_results(ISTANBUL_RESULTS_FILE_FAILURE).expect("文件级失败样本应解析成功");

    assert_eq!(parsed.cases.len(), 1, "仅 broken.test.ts 的一条断言行");
    assert_eq!(
        parsed.cases[0].status,
        TestCaseStatus::Failed,
        "用例内抛错归 failed"
    );
    assert_eq!(
        parsed.file_failures.len(),
        1,
        "转译失败文件单列（零断言 + message；broken.test.ts 的断言已进用例行）"
    );
    let (file, head) = &parsed.file_failures[0];
    assert_eq!(file, "D:/ws/src/loadfail.test.ts", "文件级失败点名来源文件");
    assert!(
        head.starts_with("Transform failed"),
        "取 message 首行作为归因，实际: {head}"
    );
    assert!(
        !head.contains('\u{1b}'),
        "首行不含 ANSI 转义（行内着色不进 finding）"
    );
}

/// 边界：计数漂移（`numTotalTests` 与用例行不一致）→ `mismatch` 记因，
/// 用例行照常返回（报告仍可用，不升格成不可测）。
#[test]
fn istanbul计数漂移记为mismatch不丢用例() {
    let drifted = r#"{
      "numTotalTests": 7,
      "testResults": [
        { "name": "D:/ws/src/a.test.ts", "status": "passed",
          "assertionResults": [
            { "fullName": "a one", "status": "passed", "duration": 1.0, "failureMessages": [] }
          ] }
      ]
    }"#;
    let parsed = parse_istanbul_results(drifted).expect("漂移样本应解析成功");
    assert_eq!(parsed.cases.len(), 1, "用例行不丢");
    let mismatch = parsed.mismatch.expect("漂移须记因");
    assert!(
        mismatch.contains('7') && mismatch.contains('1'),
        "说明载两侧计数，实际: {mismatch}"
    );
}

/// 边界：状态四值闭合——`pending` / `todo` / `disabled` 与未知值一律 Skipped
/// （跳过不参与 failed，也不计入 passed）；缺 `status` 的断言不 panic。
#[test]
fn istanbul状态四值闭合与未知值按跳过() {
    let sample = r#"{
      "numTotalTests": 5,
      "testResults": [
        { "name": "D:/ws/src/b.test.ts", "status": "passed",
          "assertionResults": [
            { "fullName": "p", "status": "pending", "failureMessages": [] },
            { "fullName": "t", "status": "todo", "failureMessages": [] },
            { "fullName": "d", "status": "disabled", "failureMessages": [] },
            { "fullName": "u", "status": "unknown-status", "failureMessages": [] },
            { "fullName": "n", "failureMessages": [] }
          ] }
      ]
    }"#;
    let parsed = parse_istanbul_results(sample).expect("四值样本应解析成功");
    assert_eq!(parsed.cases.len(), 5);
    assert!(
        parsed
            .cases
            .iter()
            .all(|case| case.status == TestCaseStatus::Skipped),
        "四值全落 Skipped，实际: {:?}",
        parsed
            .cases
            .iter()
            .map(|case| case.status)
            .collect::<Vec<_>>()
    );
    assert!(parsed.mismatch.is_none(), "numTotalTests 5 = 用例 5");
}

/// 异常：缺 `testResults` / 非数组 → `Err` 记因（语法面错误不得降级成零用例）。
#[test]
fn istanbul结果缺键或类型错显式报错() {
    let missing =
        parse_istanbul_results(r#"{ "numTotalTests": 0 }"#).expect_err("缺 testResults 应 Err");
    assert!(missing.contains("缺 testResults 键"), "实际: {missing}");

    let wrong = parse_istanbul_results(r#"{ "testResults": 3 }"#).expect_err("非数组应 Err");
    assert!(wrong.contains("必须为数组"), "实际: {wrong}");

    let broken = parse_istanbul_results("{ not json").expect_err("语法错应 Err");
    assert!(broken.contains("istanbul 结果解析失败"), "实际: {broken}");
}
