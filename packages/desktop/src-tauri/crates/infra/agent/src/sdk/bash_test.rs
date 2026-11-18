// 整测串行锁跨 await 持有为刻意语义（PATH 全局替换窗口须整测互斥；tokio
// 默认单线程 runtime 无跨线程死锁面），clippy 按多线程假设误报。
#![allow(clippy::await_holding_lock)]

use std::time::Instant;

use serde_json::json;

use crate::sdk::bash::execute;

/// PATH 环境变量修改串行化（crate 级锁：PATH 替换窗口期间其它 spawn 缝等窗口关闭）。
use crate::TEST_PATH_LOCK as PATH_LOCK;

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-bash-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

// ---------------------------------------------------------------------------
// execute：命令执行输出合并回灌（正向）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 命令执行stdout回灌ok且输出段在前() {
    let dir = tempdir("stdout-ok");

    let output = execute(
        dir.path(),
        &json!({ "command": "echo sdk-bash-ok", "intent": "校验输出回灌" }),
    )
    .await
    .expect("echo 短命令应成功");

    assert!(output.contains("sdk-bash-ok"), "stdout 段回灌: {output}");
}

#[tokio::test]
async fn 同命令追加stderr输出时stderr段带标记在stdout段之后拼接() {
    let dir = tempdir("stderr-merge");
    // 双底座兼容语法：`1>&2` 在 git-bash（POSIX）与 cmd 均成立
    let command = "echo out-marker && echo err-marker 1>&2";

    let output = execute(
        dir.path(),
        &json!({ "command": command, "intent": "校验双流合并口径" }),
    )
    .await
    .expect("双流命令应成功");

    let stdout_at = output
        .find("out-marker")
        .unwrap_or_else(|| panic!("stdout 段在场: {output}"));
    let marker_at = output
        .find("[stderr]")
        .unwrap_or_else(|| panic!("stderr 段带 [stderr] 标记: {output}"));
    let stderr_at = output
        .find("err-marker")
        .unwrap_or_else(|| panic!("stderr 内容在场: {output}"));
    assert!(
        stdout_at < marker_at && marker_at < stderr_at,
        "stdout 段在前、[stderr] 标记居中、stderr 段在后: {output}"
    );
}

// ---------------------------------------------------------------------------
// execute：cwd 承接 workspace root（正向）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 进程cwd承接workspace_root_相对路径写文件落在root下() {
    let dir = tempdir("cwd");
    let command = "echo cwd-marker > rel-from-bash.txt";

    execute(
        dir.path(),
        &json!({ "command": command, "intent": "校验 cwd 承接" }),
    )
    .await
    .expect("写文件命令应成功");

    let written = dir.path().join("rel-from-bash.txt");
    assert!(written.exists(), "相对路径产物落在 root 下（cwd 语义锁定）");
    let content = std::fs::read_to_string(&written).expect("读回产物");
    assert!(content.contains("cwd-marker"), "内容出自命令: {content}");
}

// ---------------------------------------------------------------------------
// execute：非零退出码 / 未知命令（异常）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 非零退出码err携退出码与已产出输出() {
    let dir = tempdir("exit-code");
    // 双底座兼容：`exit 3` 在 bash 直接退出、在 cmd 以 errorlevel 3 终结
    let command = "echo failed-output && exit 3";

    let error = execute(
        dir.path(),
        &json!({ "command": command, "intent": "校验非零退出码收口" }),
    )
    .await
    .expect_err("非零退出码必须 Err");

    assert!(error.contains("3"), "Err 携退出码: {error}");
    assert!(
        error.contains("failed-output"),
        "Err 携已产出输出（is_error ToolResult 内容来源）: {error}"
    );
}

#[tokio::test]
async fn 未知命令名经shell非零退出err_不panic不悬挂() {
    let dir = tempdir("unknown-cmd");

    let result = execute(
        dir.path(),
        &json!({
            "command": "definitely-not-a-command-xyz-12345",
            "intent": "校验未知命令不悬挂"
        }),
    )
    .await;

    assert!(result.is_err(), "shell 报错退出必须 Err: {result:?}");
}

#[tokio::test]
async fn command缺失或空串显式err() {
    let dir = tempdir("missing-command");

    let missing = execute(dir.path(), &json!({}))
        .await
        .expect_err("缺 command 必须 Err");
    assert!(
        missing.contains("command"),
        "记因指明 command 必填: {missing}"
    );
    assert!(
        !missing.contains("intent"),
        "校验序：`command` 先报，文案不得飘到 intent: {missing}"
    );

    let blank = execute(dir.path(), &json!({ "command": "   " }))
        .await
        .expect_err("空串 command 必须 Err");
    assert!(blank.contains("command"), "空串同口径显式 Err: {blank}");
    assert!(
        !blank.contains("intent"),
        "校验序：空白 command 先报（intent 缺席也不改结论）: {blank}"
    );
}

// ---------------------------------------------------------------------------
// execute：意图闸（缺失 / 非字符串 / trim 后空一律硬拒；长度只软约束）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 合规意图下命令正常执行且输出口径不变() {
    let dir = tempdir("intent-ok");

    let output = execute(
        dir.path(),
        &json!({ "command": "echo sdk-intent-ok", "intent": "跑一条回声命令" }),
    )
    .await
    .expect("合规意图不应拦截执行");

    assert!(output.contains("sdk-intent-ok"), "执行照常: {output}");
}

#[tokio::test]
async fn 长度软约束_两百字意图零err且照常执行() {
    let dir = tempdir("intent-200");
    let long = "复核构建产物与依赖解析链的完整性，确认打包前后模块入口一致且无多余副作用隐患再继续推进下一步验证工作".repeat(4);
    assert!(long.chars().count() >= 200, "语料长度需覆盖两百字档");

    let output = execute(
        dir.path(),
        &json!({ "command": "echo sdk-intent-200", "intent": long }),
    )
    .await
    .expect("长度只软提示：两百字意图零 Err");

    assert!(output.contains("sdk-intent-200"), "执行照常: {output}");
}

#[tokio::test]
async fn 超长意图千二字符无长度分支仍ok() {
    let dir = tempdir("intent-1200");
    let huge = "长".repeat(1_200);

    let output = execute(
        dir.path(),
        &json!({ "command": "echo sdk-intent-1200", "intent": huge }),
    )
    .await
    .expect("超过一千字符也无长度分支（长度不可升为约束）");

    assert!(output.contains("sdk-intent-1200"), "执行照常: {output}");
}

#[tokio::test]
async fn 意图首尾空白trim后非空即放行() {
    let dir = tempdir("intent-trim");

    let output = execute(
        dir.path(),
        &json!({ "command": "echo sdk-intent-trim", "intent": "  跑单测  " }),
    )
    .await
    .expect("只判 trim 后是否为空，首尾空白不影响放行");

    assert!(output.contains("sdk-intent-trim"), "执行照常: {output}");
}

#[tokio::test]
async fn 意图含换行制表emoji与井号时内容不参与判定() {
    let dir = tempdir("intent-special");

    let output = execute(
        dir.path(),
        &json!({ "command": "echo sdk-intent-special", "intent": "跑\n单测 🚀\t#1" }),
    )
    .await
    .expect("特殊字符只作内容，不参与判定");

    assert!(output.contains("sdk-intent-special"), "执行照常: {output}");
}

#[tokio::test]
async fn 单字符意图即可通过闸() {
    let dir = tempdir("intent-one");

    let output = execute(
        dir.path(),
        &json!({ "command": "echo sdk-intent-one", "intent": "读" }),
    )
    .await
    .expect("非空白即通过（不设最小长度）");

    assert!(output.contains("sdk-intent-one"), "执行照常: {output}");
}

#[tokio::test]
async fn 意图缺失显式err且记因指明intent() {
    let dir = tempdir("intent-missing");

    let error = execute(dir.path(), &json!({ "command": "echo should-not-run" }))
        .await
        .expect_err("缺 intent 必须 Err");

    assert!(error.contains("intent"), "记因指明 intent 字段: {error}");
    assert!(
        error.contains("入参缺失或非字符串字段"),
        "缺失口径与 command 闸逐字同构: {error}"
    );
}

#[tokio::test]
async fn 意图非字符串六形态一致err且记因指明intent() {
    let dir = tempdir("intent-non-string");

    for bad in [
        json!(1),
        json!(1.5),
        json!({}),
        json!([]),
        json!(null),
        json!(true),
    ] {
        let error = execute(
            dir.path(),
            &json!({ "command": "echo should-not-run", "intent": bad }),
        )
        .await
        .expect_err("intent 非字符串必须 Err");
        assert!(
            error.contains("intent"),
            "记因指明 intent 字段: {bad} -> {error}"
        );
        assert!(
            error.contains("入参缺失或非字符串字段"),
            "非字符串与缺失共用一条文案: {bad} -> {error}"
        );
    }
}

#[tokio::test]
async fn 意图为空白串时err携不得为空() {
    let dir = tempdir("intent-blank");

    for blank in [json!(""), json!("   "), json!("\t\n")] {
        let error = execute(
            dir.path(),
            &json!({ "command": "echo should-not-run", "intent": blank }),
        )
        .await
        .expect_err("trim 后为空必须 Err");
        assert!(
            error.contains("intent"),
            "记因指明 intent 字段: {blank} -> {error}"
        );
        assert!(
            error.contains("不得为空"),
            "空值口径与 command 闸逐字同构: {blank} -> {error}"
        );
    }
}

#[tokio::test]
async fn 意图闸先于超时闸_缺意图先报intent越界时后报timeout_ms() {
    let dir = tempdir("intent-before-timeout");

    let missing_intent = execute(dir.path(), &json!({ "command": "echo x", "timeout_ms": 0 }))
        .await
        .expect_err("缺 intent 与越界超时并存时必须先报 intent");
    assert!(
        missing_intent.contains("intent"),
        "位点在 resolve_timeout_ms 之前: {missing_intent}"
    );
    assert!(
        !missing_intent.contains("timeout_ms"),
        "缺意图时不落超时记因（既有超时文案零漂移）: {missing_intent}"
    );

    let bad_timeout = execute(
        dir.path(),
        &json!({ "command": "echo x", "intent": "说个理由", "timeout_ms": 0 }),
    )
    .await
    .expect_err("意图合规后越界超时仍必须 Err");
    assert!(
        bad_timeout.contains("timeout_ms"),
        "意图闸放行后交给超时闸: {bad_timeout}"
    );
}

#[tokio::test]
async fn 意图闸先于spawn_缺意图不留产物副作用() {
    let dir = tempdir("intent-before-spawn");
    // 命令留产物——若被执行则文件在场；意图闸必须先于 spawn（纯前置校验）。
    let command = "echo x > leak-from-intent.txt";

    let error = execute(dir.path(), &json!({ "command": command }))
        .await
        .expect_err("缺 intent 必须 Err");

    assert!(error.contains("intent"), "记因指明 intent: {error}");
    assert!(
        !dir.path().join("leak-from-intent.txt").exists(),
        "意图闸先于 spawn：缺 intent 的调用不得执行命令"
    );
}

// ---------------------------------------------------------------------------
// execute：超时收口触达进程树清理 + timeout_ms 边界校验（边界）
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[tokio::test]
async fn 超时收口返回err携超时记因且进程树清理路径被触达() {
    // PATH 锁：本用例的 ping 须由 PATH 解析，同二进制内 PATH 全局替换窗口
    // （空 PATH 注入）期间 spawn 会得到「命令不可解析 → 秒级非零退出」而非
    // 超时收口——按 crate 互斥纪律与替换窗口串行。
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let dir = tempdir("timeout");
    // 跨 shell 可用的长驻命令：Windows ping（git-bash 与 cmd 同解析到系统 ping）
    let command = "ping -n 30 127.0.0.1";
    let started = Instant::now();

    let error = execute(
        dir.path(),
        &json!({ "command": command, "intent": "校验超时护栏", "timeout_ms": 1_000 }),
    )
    .await
    .expect_err("长驻命令必须在超时护栏处收口");
    let elapsed = started.elapsed();

    assert!(
        elapsed.as_millis() >= 900 && elapsed.as_millis() < 10_000,
        "1 秒级返回（活性护栏收口，不等到 30s）: {elapsed:?}"
    );
    assert!(
        error.contains("超时") && error.contains("进程树已清理"),
        "Err 携超时记因（kill 路径被触达）: {error}"
    );
}

// timeout_ms 入参闸的判决面（缺省 / 放行 / 越界）全部钉在 `resolve_timeout_ms` 上——
// 它是 `execute` 里唯一决定超时的判据（`execute` 只把它 `Duration::from_millis` 一次），
// 因此「放行」= 不落越界拒绝分支，「缺省」= 常量底座。命令面的耗时属**活性面**，只在
// `超时收口…`（>=900ms 收口）与「短命令成功」两用例里承重：门禁以
// `cargo llvm-cov nextest --workspace --no-fail-fast` 全量并行跑，覆盖率插桩下
// spawn 壳 + 进程启动实测可越过千毫秒（exec-1794026202127-0 / exec-1794038704172-1
// 两跑同为 1.88s / 2.02s），把下界 1s 当「1s 内必跑完」是对护栏语义的误读。
// 直测私有 `resolve_timeout_ms`（`pub(crate)`、同文件内联 `#[test]`，零 test-only export）。

/// 入参闸上界与下界的**独立真值**（不从 `bash.rs` 的常量导入：导入即与被测方同源，
/// 常量搬错时两侧一起错，断言不再有判别力）。
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 1_800_000;

/// 入参闸的判决调用单点（用例只造入参、不重复解析口径）。
fn timeout_ms_of(input: serde_json::Value) -> Result<u64, String> {
    crate::sdk::bash::resolve_timeout_ms(&input)
}

#[test]
fn timeout_ms缺省不落入参闸_取120s底座() {
    // 缺省形态：无 timeout_ms 键 / 整对象为空，均不落校验分支而回常量底座。
    for input in [json!({}), json!({ "command": "echo no-timeout-field" })] {
        let resolved = timeout_ms_of(input.clone()).expect("缺省超时不应报错");
        assert_eq!(resolved, 120_000, "缺省即 120s 底座: {input}");
    }
}

#[test]
fn timeout_ms上下界原样放行_不钳位不落越界拒绝() {
    for ok in [MIN_TIMEOUT_MS, MAX_TIMEOUT_MS] {
        let resolved = timeout_ms_of(json!({ "timeout_ms": ok }))
            .unwrap_or_else(|error| panic!("边界值 {ok} 应放行，实际拒绝: {error}"));
        assert_eq!(resolved, ok, "放行即原样承接入参（不钳位）: {ok}");
    }
    // 界内非端点值同口径（防「只对两个端点特判」的实现蒙混）。
    let inside = (MIN_TIMEOUT_MS + MAX_TIMEOUT_MS) / 2;
    assert_eq!(
        timeout_ms_of(json!({ "timeout_ms": inside })).expect("界内值应放行"),
        inside,
        "界内任意值原样透传"
    );
}

#[test]
fn timeout_ms越界或非整数显式err且记因指明字段() {
    // 越界上侧（上界 +1、u64::MAX）与下侧（0、下界 -1）+ 非整数形态
    // （float / 字符串 / 负数 / null / 数组，as_u64 均不可解析）。
    let rejected = [
        json!(0),
        json!(MIN_TIMEOUT_MS - 1),
        json!(MAX_TIMEOUT_MS + 1),
        json!(u64::MAX),
        json!(1.5),
        json!("1000"),
        json!(-1),
        json!(null),
        json!([1_000]),
    ];

    for bad in rejected {
        let error = timeout_ms_of(json!({ "timeout_ms": bad }))
            .expect_err("timeout_ms 非法必须 Err（不静默钳位）");
        assert!(
            error.contains("timeout_ms"),
            "记因指明 timeout_ms 字段: {bad} -> {error}"
        );
    }
}

/// 边界（**AC-16**）：可见性提升是实现体零改动的纯签名一提——`bash.rs` 内
/// `resolve_timeout_ms` 恰 1 处定义、`pub(crate)` 而非 `pub`（不扩大公共面）；入参闸语义
/// 由上面三条用例逐面覆盖（缺省 / 上下界原样放行 / 越界与非整数显式 `Err`），本节只
/// 固化「可见性与实现体」这一面。
#[test]
fn resolve_timeout_ms可见性提升且实现体零改动() {
    let source = include_str!("bash.rs");
    assert_eq!(
        source.matches("fn resolve_timeout_ms(").count(),
        1,
        "`resolve_timeout_ms` 恰 1 处定义"
    );
    assert_eq!(
        source.matches("pub(crate) fn resolve_timeout_ms(").count(),
        1,
        "可见性为 `pub(crate)`（兄弟测试模块直测所需）"
    );
    assert_eq!(
        source.matches("pub fn resolve_timeout_ms(").count(),
        0,
        "MUST NOT 升为 `pub`（不涉公共面）"
    );

    // 行为面：缺省底座原样（实现体逐字不动 ⇒ 语义零变化）
    assert_eq!(
        timeout_ms_of(json!({})).expect("缺省不报错"),
        120_000,
        "缺省即 120s 底座（入参闸语义零变化）"
    );
}

#[tokio::test]
async fn timeout_ms越界时命令不执行() {
    let dir = tempdir("timeout-reject");
    // 命令留产物——若被执行则文件在场；越界拒绝必须先于 spawn（入参校验面）。
    let command = "echo should-not-run > leak-from-bash.txt";

    for bad in [json!(0), json!(MAX_TIMEOUT_MS + 1), json!("1000")] {
        let error = execute(
            dir.path(),
            &json!({
                "command": command,
                "intent": "校验超时入参越界拒绝",
                "timeout_ms": bad
            }),
        )
        .await
        .expect_err("timeout_ms 非法必须 Err");
        assert!(
            error.contains("timeout_ms"),
            "记因指明 timeout_ms 字段: {error}"
        );
    }
    assert!(
        !dir.path().join("leak-from-bash.txt").exists(),
        "入参校验先于 spawn：非法 timeout_ms 调用不得执行命令"
    );
}

// ---------------------------------------------------------------------------
// execute：无 git-bash 环境走 cmd /C 兜底（边界；PATH 全局替换窗口）
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[tokio::test]
async fn path剔除git_bash条目后经cmd底座兜底成功() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let dir = tempdir("cmd-fallback");
    // 空 PATH：探测零命中（退 Cmd），cmd.exe 由系统目录解析（不依赖 PATH）、
    // echo 为 cmd 内建（无外部查询）
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let result = execute(
        dir.path(),
        &json!({ "command": "echo cmd-fallback-ok", "intent": "校验 cmd 底座兜底" }),
    )
    .await;

    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let output = result.expect("无 git-bash 环境必须经 cmd /C 兜底成功");
    assert!(
        output.contains("cmd-fallback-ok"),
        "兜底执行输出回灌: {output}"
    );
}
