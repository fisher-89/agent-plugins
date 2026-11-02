use crate::model::TestCaseStatus;
use crate::parser::junit::parse_junit;

/// 真实采集样本（`cargo llvm-cov nextest --no-report -P <profile>
/// --tool-config-file "…"`，nextest 0.9.150 / cargo-llvm-cov 0.9.1 / rustc 1.95；
/// `report-skipped = "ignored"` 使 `#[ignore]` 用例以 `<skipped>` 出线）。
const GREEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/junit/green.xml"
));
/// 红跑样本：一条 `assertion` 失败（`<failure>` 带正文与 message/type 属性）。
const RED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/junit/red.xml"
));
/// 退出码失败样本：`std::process::exit(3)` → `<failure type="test failure with
/// exit code 3"/>`（自闭合、无正文、无 message 属性）。
const EXIT_CODE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/junit/exit-code.xml"
));

/// 正向：真实绿跑样本 → 三态计数正确，skipped 不虚构耗时、通过用例耗时按秒换算。
#[test]
fn 真实绿跑样本三态计数与耗时换算() {
    let cases = parse_junit(GREEN).expect("真实样本应解析成功");
    let count = |status: TestCaseStatus| cases.iter().filter(|case| case.status == status).count();
    assert_eq!(
        (
            count(TestCaseStatus::Passed),
            count(TestCaseStatus::Failed),
            count(TestCaseStatus::Skipped)
        ),
        (2, 0, 1),
        "两通过 / 一忽略"
    );

    let passed = cases
        .iter()
        .find(|case| case.name.ends_with("tests::it_works"))
        .expect("通过用例在场");
    assert_eq!(
        passed.name, "nxprobe$tests::it_works",
        "名称按 `classname$name` 合成（沿用 libtest 的 $ 分隔风格）"
    );
    assert_eq!(
        passed
            .duration_ms
            .map(|value| (value * 1000.0).round() as i64),
        Some(646_000),
        "time 为秒，换算毫秒"
    );

    let skipped = cases
        .iter()
        .find(|case| case.status == TestCaseStatus::Skipped)
        .expect("忽略用例在场");
    assert_eq!(skipped.name, "nxprobe$tests::it_ignored");
    assert!(
        skipped.duration_ms.is_none(),
        "skipped 的 time=\"0.000\" 是占位而非实测 → None（不虚构耗时）"
    );
    assert!(skipped.error_type.is_none() && skipped.stack_trace.is_none());
}

/// 正向：红跑样本 → 失败用例的错误面走 panic 定位行的下一非空行拆分，正文全块
/// 入堆栈；`system-out` / `system-err` 兄弟元素不干扰结论。
#[test]
fn 红跑样本失败面拆分_跳过panic定位行() {
    let cases = parse_junit(RED).expect("真实样本应解析成功");
    let failed = cases
        .iter()
        .find(|case| case.status == TestCaseStatus::Failed)
        .expect("失败用例在场");

    assert_eq!(failed.name, "nxprobe$tests::it_fails");
    assert_eq!(
        failed.error_type.as_deref(),
        Some("Error"),
        "信号行无冒号 → 类型取共享拆分口径的兜底词"
    );
    assert_eq!(
        failed.error_message.as_deref(),
        Some("assertion `left == right` failed"),
        "信号行（panic 定位行的下一非空行）即错误消息"
    );
    let stack = failed.stack_trace.as_deref().expect("stackTrace 在");
    assert!(
        stack.contains("panicked at src\\lib.rs")
            && stack.contains("left: 3")
            && stack.contains("RUST_BACKTRACE"),
        "<failure> 正文全块入 stackTrace，实际: {stack}"
    );
    assert!(
        !stack.contains("captured stdout line"),
        "用例自身 println 输出在 system-out（非 failure 正文），不入堆栈：{stack}"
    );

    for case in cases
        .iter()
        .filter(|case| case.status != TestCaseStatus::Failed)
    {
        assert!(
            case.error_type.is_none() && case.error_message.is_none() && case.stack_trace.is_none(),
            "非失败用例零错误面，实际: {:?}",
            case.name
        );
    }
}

/// 边界：退出码失败（`<failure/>` 自闭合、无正文）→ 状态 Failed、错误面退化为
/// `type` 属性，且不入堆栈、不 panic。
#[test]
fn 退出码失败_无正文退化属性面() {
    let cases = parse_junit(EXIT_CODE).expect("退出码样本应解析成功");
    let failed = cases
        .iter()
        .find(|case| case.name.ends_with("tests::it_exits"))
        .expect("退出码失败用例在场");
    assert_eq!(failed.status, TestCaseStatus::Failed);
    assert_eq!(
        failed.error_type.as_deref(),
        Some("test failure with exit code 3"),
        "无正文形态用 `type` 属性承载错误类型"
    );
    assert_eq!(failed.error_message, None, "无 message 属性 → 消息留空");
    assert_eq!(failed.stack_trace, None, "无正文 → 堆栈留空（不虚构）");
}

/// 正向：`<error>` 元素同样判失败（nextest 的 `ExecFail` / 泄漏态走 `<error>`，
/// 漏判会把执行失败说成通过）。
#[test]
fn error元素判为失败() {
    let text = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites tests="1" skipped="0" failures="0" errors="1">
    <testsuite name="pkg" tests="1" skipped="0" errors="1" failures="0">
        <testcase name="b$case" classname="pkg" time="0.010">
            <error message="execution failure" type="exec failure"/>
        </testcase>
    </testsuite>
</testsuites>"#;
    let cases = parse_junit(text).expect("error 元素样本应解析成功");
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].status, TestCaseStatus::Failed);
    assert_eq!(cases[0].error_type.as_deref(), Some("exec failure"));
    assert_eq!(cases[0].error_message.as_deref(), Some("execution failure"));
}

/// 正向：零 classname（合法落单形态）→ 名称不拼 `$` 前缀；无 time 属性 → 耗时为
/// None。
#[test]
fn 缺classname与time_不虚构() {
    let text = r#"<testsuites tests="1"><testsuite name="pkg"><testcase name="bare"/></testsuite></testsuites>"#;
    let cases = parse_junit(text).expect("极简形态应解析成功");
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "bare", "缺 classname 不拼前缀");
    assert_eq!(cases[0].status, TestCaseStatus::Passed);
    assert_eq!(cases[0].duration_ms, None, "缺 time 属性 → None");
}

/// 边界：零用例（`tests="0"`）→ `Ok(空集)`（零用例守卫由 runner 侧定型，本层
/// 不重复判定）。
#[test]
fn 零用例返回空集() {
    let text = r#"<testsuites name="nextest-run" tests="0" skipped="0" failures="0" errors="0"/>"#;
    let cases = parse_junit(text).expect("零用例是合法工件面");
    assert!(cases.is_empty());
}

/// 异常：非 XML / 非 JUnit 根 → `Err`（响亮失败，MUST NOT 降级成零用例）；
/// 根汇总计数与实际用例数不符 → `Err`（工件截断的强信号）。
#[test]
fn 非junit工件与计数不符_报错() {
    let err = parse_junit("this is not xml at all\n").expect_err("非 XML 必须报错");
    assert!(!err.is_empty(), "错误面非空: {err}");

    let mismatch = r#"<testsuites tests="5"><testsuite name="pkg"><testcase name="a" classname="p"/></testsuite></testsuites>"#;
    let err = parse_junit(mismatch).expect_err("计数不符必须报错");
    assert!(
        err.contains("计数不符") && err.contains('5') && err.contains('1'),
        "错误面点名声明值与实际值，实际: {err}"
    );
}
