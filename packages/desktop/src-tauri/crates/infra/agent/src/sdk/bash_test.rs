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

    let output = execute(dir.path(), &json!({ "command": "echo sdk-bash-ok" }))
        .await
        .expect("echo 短命令应成功");

    assert!(output.contains("sdk-bash-ok"), "stdout 段回灌: {output}");
}

#[tokio::test]
async fn 同命令追加stderr输出时stderr段带标记在stdout段之后拼接() {
    let dir = tempdir("stderr-merge");
    // 双底座兼容语法：`1>&2` 在 git-bash（POSIX）与 cmd 均成立
    let command = "echo out-marker && echo err-marker 1>&2";

    let output = execute(dir.path(), &json!({ "command": command }))
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

    execute(dir.path(), &json!({ "command": command }))
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

    let error = execute(dir.path(), &json!({ "command": command }))
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
        &json!({ "command": "definitely-not-a-command-xyz-12345" }),
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

    let blank = execute(dir.path(), &json!({ "command": "   " }))
        .await
        .expect_err("空串 command 必须 Err");
    assert!(blank.contains("command"), "空串同口径显式 Err: {blank}");
}

// ---------------------------------------------------------------------------
// execute：超时收口触达进程树清理 + timeout_ms 钳位（边界）
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[tokio::test]
async fn 超时收口返回err携超时记因且进程树清理路径被触达() {
    let dir = tempdir("timeout");
    // 跨 shell 可用的长驻命令：Windows ping（git-bash 与 cmd 同解析到系统 ping）
    let command = "ping -n 30 127.0.0.1";
    let started = Instant::now();

    let error = execute(
        dir.path(),
        &json!({ "command": command, "timeout_ms": 1_000 }),
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

#[tokio::test]
async fn timeout_ms缺省走120s底座且短命令成功() {
    let dir = tempdir("timeout-default");

    let output = execute(dir.path(), &json!({ "command": "echo no-timeout-field" }))
        .await
        .expect("缺省超时下短命令应成功");
    assert!(output.contains("no-timeout-field"), "{output}");
}

#[tokio::test]
async fn timeout_ms零值被钳入下限1秒_短命令仍成功() {
    let dir = tempdir("timeout-clamp");
    // 若按 0 生效（timeout(0) 立即收口）短命令不可能成功——Ok 即证钳位
    let output = execute(
        dir.path(),
        &json!({ "command": "echo clamped-to-floor", "timeout_ms": 0 }),
    )
    .await
    .expect("timeout_ms=0 钳入下限 1000ms 后短命令应成功");
    assert!(output.contains("clamped-to-floor"), "{output}");
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

    let result = execute(dir.path(), &json!({ "command": "echo cmd-fallback-ok" })).await;

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
