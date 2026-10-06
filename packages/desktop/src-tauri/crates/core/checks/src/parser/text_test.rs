use crate::model::TestCaseStatus;
use crate::parser::text::{parse_cargo_test, parse_spec_report};

/// 真实 node-test spec 绿跑样本（语料文件，见 tests/fixtures/README.md 清单表）。
const NODE_SPEC_GREEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/node-spec/output.txt"
));

/// 真实 node-test spec 红跑样本（含失败用例与错误块）。
const NODE_SPEC_RED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/node-spec/output-fail.txt"
));

/// 真实 cargo test 文本样本（ok / FAILED / ignored + 失败详情块）。
const CARGO_SAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/cargo/output.txt"
));

/// 真实 cargo test 编译失败红态样本（零结果行）。
const CARGO_COMPILE_ERROR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/cargo-compile-error/output.txt"
));

// ---------------------------------------------------------------------------
// parse_spec_report
// ---------------------------------------------------------------------------

/// 正向：真实 node-test spec 输出样本（pass / fail / skip 结果行 + 子测试
/// 缩进层级）→ TestCaseResult 列表 status 三值正确。
#[test]
fn 真实spec样本status三值正确() {
    let cases = parse_spec_report(NODE_SPEC_GREEN).expect("绿跑样本应解析成功");

    assert_eq!(cases.len(), 5, "五条用例行（套件收口行与汇总行不产用例）");

    let mut by_name: std::collections::BTreeMap<&str, TestCaseStatus> = cases
        .iter()
        .map(|case| (case.name.as_str(), case.status))
        .collect();
    assert_eq!(by_name.remove("adds numbers"), Some(TestCaseStatus::Passed));
    assert_eq!(
        by_name.remove("handles unicode 名称 ✓"),
        Some(TestCaseStatus::Passed),
        "unicode 名称保真"
    );
    assert_eq!(
        by_name.remove("skips todo case"),
        Some(TestCaseStatus::Skipped)
    );
    assert_eq!(
        by_name.remove("works nested"),
        Some(TestCaseStatus::Passed),
        "子测试缩进层级提取"
    );
    assert_eq!(
        by_name.remove("top level case"),
        Some(TestCaseStatus::Passed)
    );
    assert!(by_name.is_empty(), "无多余用例（无漏无增）");

    // 耗时注记剥离（` (1.2ms)` → duration_ms 1.2）
    let adds = cases
        .iter()
        .find(|case| case.name == "adds numbers")
        .expect("用例在场");
    assert_eq!(
        adds.duration_ms,
        Some(1.2),
        "行尾耗时注记剥离为 duration_ms"
    );
}

/// 正向：失败用例行 → errorType / errorMessage / stackTrace 仅失败态填充且
/// 内容捕获不丢（错误块首行拆 `错误类型: 消息`、全块入堆栈）。
#[test]
fn 失败用例错误块捕获不丢() {
    let cases = parse_spec_report(NODE_SPEC_RED).expect("红跑样本应解析成功");
    assert_eq!(cases.len(), 2);

    let ok = &cases[0];
    assert_eq!(ok.status, TestCaseStatus::Passed);
    assert!(
        ok.error_type.is_none() && ok.error_message.is_none() && ok.stack_trace.is_none(),
        "通过态三错误字段不填充"
    );

    let failed = &cases[1];
    assert_eq!(failed.status, TestCaseStatus::Failed);
    assert_eq!(
        failed.error_type.as_deref(),
        Some("AssertionError [ERR_ASSERTION]"),
        "错误块首行拆类型"
    );
    assert_eq!(
        failed.error_message.as_deref(),
        Some("expected 1 to equal 2"),
        "错误块首行拆消息"
    );
    let stack = failed.stack_trace.as_deref().expect("堆栈在");
    assert!(
        stack.contains("AssertionError [ERR_ASSERTION]")
            && stack.contains("at Test.<anonymous> (test/app/math.test.js:3:5)")
            && stack.contains("at process.processTicksAndRejections"),
        "错误块全行入堆栈捕获不丢，实际: {stack}"
    );
}

/// 边界：噪声行（诊断 / 汇总 / 空行 / 套件行）不误提取为用例（提取器只认
/// 结果行锚模式）。
#[test]
fn 噪声行不误提取为用例() {
    let cases = parse_spec_report(NODE_SPEC_GREEN).expect("解析应成功");
    let names: Vec<&str> = cases.iter().map(|case| case.name.as_str()).collect();
    // 汇总关键词行（ℹ tests 5 / ℹ pass 4 / ℹ skipped 1 …）与空行不产用例
    assert!(
        names.iter().all(|name| {
            ![
                "tests",
                "suites",
                "pass",
                "fail",
                "cancelled",
                "skipped",
                "todo",
            ]
            .iter()
            .any(|keyword| name.starts_with(keyword))
        }),
        "汇总行关键词不误提取，实际: {names:?}"
    );

    // 纯噪声文本（诊断输出、横幅、空行）→ 零用例
    let noisy = "\n\nthis is a plain diagnostic line\n\nanother plain line\n";
    let cases = parse_spec_report(noisy).expect("纯噪声应解析成功");
    assert!(cases.is_empty(), "无结果行锚模式 → 零用例");

    // 套件起始与收口行不产用例（绿样本里 `▶ math 套件` / `✔ math 套件` 均不在
    // 用例集）
    assert!(!names.contains(&"math 套件"), "套件行不产用例");
    assert!(!names.contains(&"nested"), "套件收口行不产用例");
}

/// 边界：测试名含特殊字符（空格 / unicode / 中文）保真到用例行。
#[test]
fn 测试名特殊字符保真到用例行() {
    let sample =
        "✔ 用例 名称 with spaces ∑ (3ms)\n✔ 中文描述 保持 (0.4ms)\n✖ 带引号 \"quoted\" (1ms)\n";
    let cases = parse_spec_report(sample).expect("解析应成功");

    assert_eq!(cases.len(), 3);
    assert_eq!(
        cases[0].name, "用例 名称 with spaces ∑",
        "空格 / unicode 保真"
    );
    assert_eq!(cases[0].duration_ms, Some(3.0));
    assert_eq!(cases[1].name, "中文描述 保持");
    assert_eq!(cases[2].name, "带引号 \"quoted\"");
    assert_eq!(cases[2].status, TestCaseStatus::Failed);
}

/// 边界：空文本输入返回空用例集不 panic（零用例行计数面交完整性门禁对账）。
#[test]
fn 空文本输入返回空用例集不panic() {
    let cases = parse_spec_report("").expect("空文本应 Ok");
    assert!(cases.is_empty());

    let cases = parse_spec_report("\n\n   \n").expect("纯空白应 Ok");
    assert!(cases.is_empty());

    // ANSI 着色输入归一（ESC 序列剥离后仍可识别结果行）
    let colored = "\u{1b}[32m✔ colored pass (1ms)\u{1b}[0m\n";
    let cases = parse_spec_report(colored).expect("着色样本应解析成功");
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "colored pass");
    assert_eq!(cases[0].status, TestCaseStatus::Passed);
}

// ---------------------------------------------------------------------------
// parse_cargo_test
// ---------------------------------------------------------------------------

/// 正向：真实 cargo test 文本输出样本（`test xxx ... ok / FAILED / ignored`）
/// → 用例行 status 三值正确。
#[test]
fn 真实cargo样本status三值正确() {
    let cases = parse_cargo_test(CARGO_SAMPLE).expect("cargo 样本应解析成功");

    assert_eq!(
        cases.len(),
        3,
        "三条结果行（running / test result 汇总不产用例）"
    );
    assert_eq!(
        cases[0].name,
        "checks::aggregate::determine_conclusion_passes"
    );
    assert_eq!(cases[0].status, TestCaseStatus::Passed);
    assert_eq!(
        cases[1].name,
        "checks::aggregate::determine_conclusion_fails"
    );
    assert_eq!(cases[1].status, TestCaseStatus::Failed);
    assert_eq!(cases[2].name, "checks::model::parses_minimal");
    assert_eq!(
        cases[2].status,
        TestCaseStatus::Skipped,
        "ignored → skipped 三值映射"
    );
}

/// 正向：失败行携断言输出 → errorMessage / stackTrace 捕获（`---- <name>
/// stdout ----` 块按用例名对位回填）。
#[test]
fn cargo失败行断言输出捕获() {
    let cases = parse_cargo_sample().0;
    let failed = cases
        .iter()
        .find(|case| case.status == TestCaseStatus::Failed)
        .expect("失败用例在场");

    assert_eq!(
        failed.error_type.as_deref(),
        Some("assertion `left == right` failed"),
        "详情块首行按 `错误类型: 消息` 拆分（首个冒号前为类型段）"
    );
    let message = failed.error_message.as_deref().expect("errorMessage 在");
    assert_eq!(message, "conclusion 漂移", "首个冒号后为消息段: {message}");
    let stack = failed.stack_trace.as_deref().expect("stackTrace 在");
    assert!(
        stack.contains("assertion `left == right` failed")
            && stack.contains("left: Fail")
            && stack.contains("right: Pass")
            && stack.contains("RUST_BACKTRACE"),
        "详情块全行入 stackTrace 捕获不丢，实际: {stack}"
    );

    // 通过 / skipped 用例不回填错误面
    let ok = cases
        .iter()
        .find(|case| case.status == TestCaseStatus::Passed)
        .expect("通过用例在场");
    assert!(ok.error_type.is_none() && ok.stack_trace.is_none());
}

/// cargo 样本双形态解析（原文与 trim 行尾等价——解析对行尾空白鲁棒）。
fn parse_cargo_sample() -> (
    Vec<crate::model::TestCaseResult>,
    Vec<crate::model::TestCaseResult>,
) {
    let cases = parse_cargo_test(CARGO_SAMPLE).expect("解析应成功");
    (cases.clone(), cases)
}

/// 异常：编译失败输出（零结果行）→ 与 CLI parseTextOutput rust 分支同语义
///（corpus 红态样本对拍锚定），不误产用例行。
#[test]
fn cargo编译失败零结果行返回空集() {
    let cases = parse_cargo_test(CARGO_COMPILE_ERROR).expect("编译失败样本应 Ok（空集语义）");
    assert!(
        cases.is_empty(),
        "零结果行 → 空用例集（不产假用例，计数面交完整性门禁对账），实际: {cases:?}"
    );

    // 失败详情块形态与用例名对位：块名无对应失败用例 → 不误增不误配
    let orphan_block = "---- checks::ghost::orphan stdout ----\npanicked at orphan\n";
    let cases = parse_cargo_test(orphan_block).expect("孤儿块样本应 Ok");
    assert!(cases.is_empty(), "孤儿块不产用例");
}

/// 边界：超长输出（>1000 行噪声夹杂少量结果行）提取不丢不误增。
#[test]
fn cargo超长输出提取不丢不误增() {
    let mut text = String::new();
    for index in 0..1000 {
        text.push_str(&format!("warning: unused variable `x_{index}`\n"));
        if index % 200 == 0 {
            text.push_str(&format!("test noise::case_{index} ... ok\n"));
        }
    }
    text.push_str("test tail::last ... FAILED\n");
    text.push_str("\n---- tail::last stdout ----\nassertion failed\n");

    let cases = parse_cargo_test(&text).expect("超长输出应解析成功");
    assert_eq!(cases.len(), 6, "五条散布 ok + 末尾 FAILED，噪声零误提取");
    let passed: Vec<&str> = cases
        .iter()
        .filter(|case| case.status == TestCaseStatus::Passed)
        .map(|case| case.name.as_str())
        .collect();
    assert_eq!(passed.len(), 5, "散布结果行全数保留（不丢）");
    assert_eq!(cases[5].name, "tail::last");
    assert_eq!(cases[5].status, TestCaseStatus::Failed);
    assert!(cases[5]
        .stack_trace
        .as_deref()
        .is_some_and(|stack| stack.contains("assertion failed")));
}
