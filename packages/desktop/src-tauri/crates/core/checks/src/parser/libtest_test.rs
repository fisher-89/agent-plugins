use crate::model::TestCaseStatus;
use crate::parser::libtest::parse_libtest_json;

/// 真实采集样本（`cargo llvm-cov nextest --message-format libtest-json
/// --message-format-version 0.1 --no-fail-fast`，nextest 0.9.150 / cargo-llvm-cov
/// 0.9.1 / rustc 1.95，红跑形态）：三个 binary 块（integration 红 + 单测绿 +
/// integration 部分过滤），覆盖 ok / failed / ignored 三态与 `exec_time` 字段。
const LIBTEST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/libtest-json/output.txt"
));

/// 正向：真实事件流 → 三态计数与耗时换算正确，`suite` / `started` 不产用例。
#[test]
fn 真实libtest样本三态计数正确() {
    let cases = parse_libtest_json(LIBTEST_JSON).expect("真实样本应解析成功");

    let count = |status: TestCaseStatus| cases.iter().filter(|case| case.status == status).count();
    assert_eq!(
        (
            count(TestCaseStatus::Passed),
            count(TestCaseStatus::Failed),
            count(TestCaseStatus::Skipped)
        ),
        (3, 1, 1),
        "ok 三条 / failed 一条 / ignored 一条（started 与 suite 行不产用例）"
    );
    assert_eq!(cases.len(), 5, "用例集无重复（同运行内同名终态只留一条）");

    let passed = cases
        .iter()
        .find(|case| case.name == "nxprobe::it$it_works")
        .expect("integration 通过用例在场");
    assert_eq!(passed.status, TestCaseStatus::Passed);
    assert_eq!(
        passed
            .duration_ms
            .map(|value| (value * 1000.0).round() as i64),
        Some(1_406_316),
        "exec_time 为秒，换算毫秒（精度到微秒）"
    );

    let ignored = cases
        .iter()
        .find(|case| case.status == TestCaseStatus::Skipped)
        .expect("ignored 用例在场");
    assert_eq!(ignored.name, "nxprobe::it$it_ignored");
    assert!(
        ignored.duration_ms.is_none(),
        "ignored 事件无 exec_time → duration None（不虚构耗时）"
    );
    assert!(ignored.error_type.is_none() && ignored.stack_trace.is_none());
}

/// 正向：failed 事件的 `stdout`（混装 stdout + stderr）→ panic 定位行被跳过、
/// 信号行拆 `错误类型: 消息`、全块入 stackTrace，且非失败用例不回填错误面。
#[test]
fn 失败事件错误面捕获_跳过panic定位行() {
    let cases = parse_libtest_json(LIBTEST_JSON).expect("真实样本应解析成功");
    let failed = cases
        .iter()
        .find(|case| case.status == TestCaseStatus::Failed)
        .expect("失败用例在场");

    assert_eq!(failed.name, "nxprobe::it$it_fails");
    assert_eq!(
        failed.error_type.as_deref(),
        Some("assertion `left == right` failed"),
        "信号行（panic 定位行的下一非空行）按首个冒号拆分"
    );
    assert_eq!(
        failed.error_message.as_deref(),
        Some("left == right"),
        "冒号后为消息段"
    );
    let stack = failed.stack_trace.as_deref().expect("stackTrace 在");
    assert!(
        stack.contains("panicked at tests\\it.rs")
            && stack.contains("left: 1")
            && stack.contains("RUST_BACKTRACE"),
        "stdout 全块入 stackTrace（含 panic 定位行与捕获的 println），实际: {stack}"
    );
    assert!(
        stack.contains("captured stdout line"),
        "用例自身 println 输出同在块内（libtest 语义）"
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

/// 边界：`--ensure-time` 的 `time limit exceeded` 形态（failed 但无 `stdout`
/// 字段）→ 状态 Failed、错误面全 None、不 panic。
#[test]
fn 超时失败无输出字段_错误面留空() {
    let text = concat!(
        r#"{"type":"test","event":"started","name":"pkg::b$slow"}"#,
        "\n",
        r#"{"type":"test","event":"failed","name":"pkg::b$slow","exec_time":600.0,"reason":"time limit exceeded"}"#,
        "\n",
    );
    let cases = parse_libtest_json(text).expect("无 stdout 的 failed 事件应解析成功");
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].status, TestCaseStatus::Failed);
    assert_eq!(cases[0].duration_ms, Some(600_000.0));
    assert!(
        cases[0].error_type.is_none() && cases[0].stack_trace.is_none(),
        "无输出形态不虚构错误面"
    );
}

/// 边界：坏行（壳噪声 / 格式漂移）跳过后仍解析出用例；**零事件行**才 Err
/// （工件非空却读不出任何事件 = 模板或报告器参数失配的强信号）。
#[test]
fn 坏行容忍_零事件行才报错() {
    let mixed = concat!(
        "Compiling dev-team v0.1.0\n",
        r#"{"type":"test","event":"ok","name":"pkg::b$case","exec_time":0.5}"#,
        "\n",
        "some trailing noise\n",
    );
    let cases = parse_libtest_json(mixed).expect("夹杂噪声应解析成功");
    assert_eq!(cases.len(), 1, "坏行跳过、事件行照常产出");

    // 纯噪声（非空工件但零事件）→ Err（MUST NOT 降级成零用例）
    let err =
        parse_libtest_json("this is not json\nneither is this\n").expect_err("零事件行必须报错");
    assert!(
        err.contains("不含任何事件行") && err.contains("2 行不可解析"),
        "错误面点名零事件与坏行计数，实际: {err}"
    );

    // 只有 suite / started 行（报告器只打了元数据）同样是零用例面 → Ok 空集，
    // 由 runner 的「零用例」守卫定型（本层不重复该判定）
    let metadata_only = concat!(
        r#"{"type":"suite","event":"started","test_count":1}"#,
        "\n",
        r#"{"type":"test","event":"started","name":"pkg::b$case"}"#,
        "\n",
        r#"{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.1}"#,
        "\n",
    );
    let cases = parse_libtest_json(metadata_only).expect("仅元数据行应 Ok（空集）");
    assert!(cases.is_empty(), "started / suite 事件不产用例");
}

/// 边界：同名终态重复（防御同运行内重复行）→ 末次覆盖，不产重名用例。
#[test]
fn 同名终态重复_末次覆盖不产重名() {
    let text = concat!(
        r#"{"type":"test","event":"failed","name":"pkg::b$flaky","exec_time":1.0,"stdout":"first\n"}"#,
        "\n",
        r#"{"type":"test","event":"ok","name":"pkg::b$flaky","exec_time":2.0}"#,
        "\n",
    );
    let cases = parse_libtest_json(text).expect("重复形态应解析成功");
    assert_eq!(cases.len(), 1, "同名只留一条");
    assert_eq!(cases[0].status, TestCaseStatus::Passed, "末次终态为准");
    assert_eq!(cases[0].duration_ms, Some(2000.0));
    assert!(cases[0].error_type.is_none(), "末态为 ok 时不留旧错误面");
}
