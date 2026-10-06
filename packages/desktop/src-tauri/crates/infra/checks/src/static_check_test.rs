use std::fs;
use std::path::PathBuf;

use orchestration::port::{StaticCheckRunner, ToolStepOutput};

use crate::static_check::ProcessStaticCheck;

/// PATH 环境变量修改串行化（进程全局变量边界；与 worker_test 的 CLI shim /
/// PATH 隔离窗口共用 crate 级锁——拉起本体失败用例仅在 Unix 触达 PATH 隔离臂）。
#[cfg(not(windows))]
use crate::TEST_PATH_LOCK as PATH_LOCK;

/// 临时 workspace 根 RAII。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "agent-runtime-static-check-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建 workspace 失败");
        Self(dir)
    }

    fn root_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    /// 预置 config（`<root>/openspec/config.json`，layout config_path 单点）。
    fn write_config(&self, static_analysis: Option<&str>) {
        let config_dir = self.0.join("openspec");
        fs::create_dir_all(&config_dir).expect("创建 openspec 目录失败");
        let body = match static_analysis {
            Some(command) => format!("{{ \"static_analysis\": \"{command}\" }}"),
            None => "{}".to_owned(),
        };
        fs::write(config_dir.join("config.json"), body).expect("写 config 失败");
    }

    /// 在 root 下落一个文件（cwd 断言面：命令以相对路径读到它）。
    fn write_root_file(&self, name: &str, content: &str) {
        fs::write(self.0.join(name), content).expect("写 root 文件失败");
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 一次 static-check 执行（收口产出）。
async fn check(root: &str) -> Result<orchestration::port::StaticCheckOutcome, String> {
    let output = ProcessStaticCheck::new().run(root).await?;
    match output {
        ToolStepOutput::StaticCheck(outcome) => Ok(outcome),
        other => panic!("产出应为 StaticCheck 变体，实际: {other:?}"),
    }
}

#[tokio::test]
async fn 通过形态_passed真_诊断空_cwd为root() {
    let ws = TempWs::new("pass");
    // 命令以相对路径读取 root 下文件：仅在 cwd = root 时成功（W4 spawn 语义）；
    // 首 token 为真实可达程序（cmd 在系统目录、test 在 PATH），且不经 shell
    // 元字符形态（括号组经双层 cmd 解析会破碎——`if not exist … exit /b 1`
    // 单分支形态保双层解析稳定）
    ws.write_root_file("pass-marker.txt", "ok");
    #[cfg(windows)]
    let command = r#"cmd /C if not exist pass-marker.txt exit /b 1"#;
    #[cfg(not(windows))]
    let command = "test -f pass-marker.txt";
    ws.write_config(Some(command));

    let outcome = check(&ws.root_str()).await.expect("通过形态应 Ok");

    assert!(outcome.passed, "退出码 0 → passed");
    assert!(
        outcome.diagnostics.is_empty(),
        "诊断空（无输出命令），实际: {}",
        outcome.diagnostics
    );
}

#[tokio::test]
async fn 失败形态_passed假_诊断捕获不丢() {
    let ws = TempWs::new("fail");
    #[cfg(windows)]
    let command =
        r#"cmd /C echo lint-failed-to-stderr 1>&2 & echo lint-failed-to-stdout & exit /b 3"#;
    #[cfg(not(windows))]
    let command = "echo lint-failed-to-stderr >&2; echo lint-failed-to-stdout; exit 3";
    ws.write_config(Some(command));

    let outcome = check(&ws.root_str()).await.expect("失败形态仍 Ok 携产出");

    assert!(!outcome.passed, "非零退出码 → failed（反馈边原料）");
    assert!(
        outcome.diagnostics.contains("lint-failed-to-stderr")
            && outcome.diagnostics.contains("lint-failed-to-stdout"),
        "stderr / stdout 捕获拼接不丢（反馈边注入原料——AC-4），实际: {}",
        outcome.diagnostics
    );
}

#[tokio::test]
async fn 无配置或空命令直接过() {
    // config 无 static_analysis 键（无配置 = passed 直过，不拉子进程）
    let ws = TempWs::new("no-config");
    let outcome = check(&ws.root_str()).await.expect("无配置应直过");
    assert!(outcome.passed && outcome.diagnostics.is_empty());

    // 显式空白命令（trim 后空 = 直过语义）
    let ws_blank = TempWs::new("blank-command");
    ws_blank.write_config(Some("   "));
    let outcome = check(&ws_blank.root_str()).await.expect("空命令应直过");
    assert!(outcome.passed, "空串命令直过语义");
}

#[tokio::test]
async fn 非零退出码判定_诊断空亦不误判通过() {
    let ws = TempWs::new("nonzero-silent");
    // 首 token 为真实可达程序（shell 内建字不构成可达程序——程序解析前置会先拒）
    #[cfg(windows)]
    let command = r#"cmd /C exit /b 7"#;
    #[cfg(not(windows))]
    let command = "false";
    ws.write_config(Some(command));

    let outcome = check(&ws.root_str()).await.expect("Ok 携产出");

    assert!(!outcome.passed, "退出码即判据——诊断空不误判通过");
}

/// 命令不可达 → 显式 Err（spawn 前程序解析前置）：裸名不在 PATH（Windows 额外
/// 查 cwd 亦未命中）→ Err（shell 会把命令缺失吞成非零退出码——passed=false
/// 反馈边空转烧预算，配置错误停给用户，不静默直过也不落产出）。
#[tokio::test]
async fn 命令不存在err显式不静默() {
    let ws = TempWs::new("missing-command");
    ws.write_config(Some("no-such-static-check-command-xyz"));

    let err = check(&ws.root_str())
        .await
        .expect_err("不可达命令应 Err（配置错误显式停给用户）");

    assert!(
        err.contains("no-such-static-check-command-xyz") && err.contains("未找到"),
        "Err 记因裸名与解析失败面，实际: {err}"
    );
}

/// 显式相对路径不可达 → Err：带路径分隔符的命令查相对 root（spawn cwd = root
/// 的可达性口径），root 下无对应可执行 → Err 记因「相对 root 不存在」（路径
/// 以正斜杠书写——config.json 内反斜杠转义非法，Path 分量解析跨平台一致）。
#[tokio::test]
async fn 相对路径不可达err显式() {
    let ws = TempWs::new("relative-missing");
    #[cfg(windows)]
    let command = "tools/absent-check.cmd";
    #[cfg(not(windows))]
    let command = "tools/absent-check";
    ws.write_config(Some(command));

    let err = check(&ws.root_str())
        .await
        .expect_err("相对 root 不可达应 Err");

    assert!(
        err.contains("不可达") && err.contains("相对 root"),
        "Err 记因相对路径解析面，实际: {err}"
    );
}

/// 拉起本体失败 → Err(String)：shell 二进制不可达（PATH 隔离）时
/// `output()` 层 io 失败 → Err（显式失败面；Windows 下 cmd 由系统目录兜底
/// 解析恒可达，故 Err 臂仅在 Unix 可达——两平台各自可达的失败形态均不静默
/// passed）。PATH 隔离窗口经共享互斥锁串行化、测毕恢复。
#[cfg(not(windows))]
#[tokio::test]
async fn 拉起本体失败err显式() {
    let ws = TempWs::new("launch-fail");
    ws.write_config(Some("false"));

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let result = check(&ws.root_str()).await;
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(_guard);

    let err = result.expect_err("shell 不可达应 Err");
    assert!(
        err.contains("拉起失败"),
        "Err 记因（AC-4 记因面），实际: {err}"
    );
}
