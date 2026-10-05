use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::json;

#[cfg(windows)]
use crate::sdk::bash::probe_windows_shell;
use crate::sdk::bash::{execute, ShellBase};

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
// probe_windows_shell：fabricated PATH 纯函数直测（无进程参与）
// ---------------------------------------------------------------------------

/// 在 base 下落一个真实占位文件（探测仅 `is_file` 判定，零内容即可）。
fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("占位路径应有父目录")).expect("创建父目录失败");
    std::fs::write(path, b"").expect("落占位文件失败");
}

#[cfg(windows)]
#[test]
fn probe命中git_bin形态条目返回gitbash全路径() {
    let dir = tempdir("probe-git-bin");
    let bash = dir.path().join("Git\\bin\\bash.exe");
    touch(&bash);

    let base = probe_windows_shell(OsStr::new(dir.path().as_os_str()));

    match base {
        ShellBase::GitBash(found) => assert_eq!(
            found, bash,
            "命中 <条目>\\Git\\bin\\bash.exe 形态，返回全路径"
        ),
        _ => panic!("Git\\bin 形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }
}

#[cfg(windows)]
#[test]
fn probe命中git_usr_bin形态条目且条目本身为git段时直接补bash_exe() {
    // 形态一：<条目>\Git\usr\bin\bash.exe
    let dir = tempdir("probe-git-usr-bin");
    let bash = dir.path().join("Git\\usr\\bin\\bash.exe");
    touch(&bash);
    match probe_windows_shell(OsStr::new(dir.path().as_os_str())) {
        ShellBase::GitBash(found) => assert_eq!(found, bash, "Git\\usr\\bin 形态同判"),
        _ => panic!("Git\\usr\\bin 形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }

    // 形态二：PATH 条目本身已是 …\Git\bin 段 → 直接补 <条目>\bash.exe
    let segment_root = tempdir("probe-segment");
    let segment_bash = segment_root.path().join("Git\\bin\\bash.exe");
    touch(&segment_bash);
    let segment = segment_root.path().join("Git\\bin");
    match probe_windows_shell(segment.as_os_str()) {
        ShellBase::GitBash(found) => assert_eq!(
            found, segment_bash,
            "条目本身为 Git\\bin 段时补 <条目>\\bash.exe 命中"
        ),
        _ => panic!("段形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }
}

#[cfg(windows)]
#[test]
fn probe命中git_cmd标准安装布局返回兄弟bin的bash全路径() {
    // 安装器标准布局：PATH 只写 `…\Git\cmd`，bash 落在兄弟 `bin\bash.exe`
    let dir = tempdir("probe-git-cmd");
    let bash = dir.path().join("Git\\bin\\bash.exe");
    touch(&bash);
    let entry = dir.path().join("Git\\cmd");

    match probe_windows_shell(entry.as_os_str()) {
        ShellBase::GitBash(found) => assert_eq!(
            found, bash,
            "Git\\cmd 条目命中兄弟 bin\\bash.exe（标准安装布局不退 Cmd）"
        ),
        _ => panic!("Git\\cmd 形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }

    // usr\bin 兄弟布局同判
    let dir2 = tempdir("probe-git-cmd-usr");
    let bash2 = dir2.path().join("Git\\usr\\bin\\bash.exe");
    touch(&bash2);
    let entry2 = dir2.path().join("Git\\cmd");
    match probe_windows_shell(entry2.as_os_str()) {
        ShellBase::GitBash(found) => {
            assert_eq!(found, bash2, "Git\\cmd 条目命中兄弟 usr\\bin\\bash.exe")
        }
        _ => panic!("Git\\cmd + usr\\bin 形态必须命中 GitBash"),
    }
}

#[cfg(windows)]
#[test]
fn probe命中git根条目返回子bin的bash全路径() {
    // PATH 直接写 Git 根：bash 落在子 `bin\bash.exe`
    let dir = tempdir("probe-git-root");
    let bash = dir.path().join("Git\\bin\\bash.exe");
    touch(&bash);
    let entry = dir.path().join("Git");

    match probe_windows_shell(entry.as_os_str()) {
        ShellBase::GitBash(found) => assert_eq!(found, bash, "Git 根条目命中子 bin\\bash.exe"),
        _ => panic!("Git 根形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }
}

#[cfg(windows)]
#[test]
fn probe排除system32同名误中_仅wsl形态条目时退cmd() {
    // 同名误中形态：条目顶层放 bash.exe（System32\bash.exe 的 WSL 同名语义，
    // 不在 Git 形态段内）→ 不误中
    let dir = tempdir("probe-system32");
    touch(&dir.path().join("bash.exe"));

    let base = probe_windows_shell(OsStr::new(dir.path().as_os_str()));

    assert!(
        matches!(base, ShellBase::Cmd),
        "System32 WSL bash 同名不命中，退 Cmd 兜底"
    );
}

#[cfg(windows)]
#[test]
fn probe无任何bash条目退cmd() {
    let dir = tempdir("probe-empty");

    let base = probe_windows_shell(OsStr::new(dir.path().as_os_str()));

    assert!(matches!(base, ShellBase::Cmd), "无 bash 条目退 Cmd");
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

// ---------------------------------------------------------------------------
// ShellBase 形状锚定（编译期 + 调试形态）
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[test]
fn shell_base枚举三变体调试形态可辨() {
    // GitBash / Cmd（Sh 为 unix 臂变体）形状可辨，探测产物按底座组装命令
    let cmd_base: ShellBase = ShellBase::Cmd;
    assert!(matches!(cmd_base, ShellBase::Cmd));
    let git_base: ShellBase = ShellBase::GitBash(PathBuf::from("C:\\Git\\bin\\bash.exe"));
    assert!(matches!(git_base, ShellBase::GitBash(_)));
}
