use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use foundation::layout::ReportSlot;
use orchestration::port::{StaticCheckOutcome, StaticCheckRunner, ToolStepOutput};

use crate::static_check::ProcessStaticCheck;

/// 进程环境锁窗口（RAII）
struct EnvWindow {
    _guard: std::sync::MutexGuard<'static, ()>,
}

fn env_window() -> EnvWindow {
    EnvWindow {
        _guard: crate::lock_test_env(),
    }
}

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

    fn path(&self) -> &Path {
        &self.0
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

    /// 输出记录目录（`<report_dir>/exec-outputs`——落盘与回查同锚推导）。
    fn exec_dir(&self, change: &str) -> PathBuf {
        foundation::layout::change_test_reports(self.path(), change).join("exec-outputs")
    }

    /// 槽位版输出记录目录（`<report_dir(slot)>/exec-outputs`——baseline 树锚定
    /// 断言面，check-before-flow-start）。
    fn exec_dir_slot(&self, change: &str, slot: ReportSlot) -> PathBuf {
        foundation::layout::change_reports(self.path(), change, slot).join("exec-outputs")
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// PATH 前置 workspace 根窗口（shim 裸名经 PATH 解析——不依赖 cmd 的 cwd
/// 搜索，NoDefaultCurrentDirectoryInExePath 环境免疫；RAII 恢复原 PATH）。
struct PathPrefix {
    original: Option<OsString>,
}

impl PathPrefix {
    fn enter(root: &Path) -> Self {
        let original = std::env::var_os("PATH");
        let base = original.clone().unwrap_or_default();
        let dirs = std::env::split_paths(&base);
        let joined = std::env::join_paths(std::iter::once(root.to_path_buf()).chain(dirs))
            .expect("join PATH 失败");
        std::env::set_var("PATH", joined);
        Self { original }
    }
}

impl Drop for PathPrefix {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }
}

/// shim 输出行（文本 + 是否走 stderr）。
type ShimLine<'a> = (&'a str, bool);

/// 在 workspace 根写入平台 shim 脚本（逐行 echo；stdout / stderr 交错面可编，
/// 可选非零退出码）。返回 config 应登记的裸名（Windows 带 `.cmd` 后缀）。
fn write_shim(ws: &TempWs, name: &str, lines: &[ShimLine<'_>], exit: Option<i32>) -> String {
    #[cfg(windows)]
    {
        let mut body = String::from("@echo off\r\n");
        for (text, to_stderr) in lines {
            // 重定向须置于命令前：`echo {text} 1>&2` 会把分隔空格计入 echo 参数
            //（cmd 语义），前置形态输出与登记文本逐字一致（stderr 行无尾随空格）
            if *to_stderr {
                body.push_str(&format!("1>&2 echo {text}\r\n"));
            } else {
                body.push_str(&format!("echo {text}\r\n"));
            }
        }
        if let Some(code) = exit {
            body.push_str(&format!("exit /b {code}\r\n"));
        }
        fs::write(ws.path().join(format!("{name}.cmd")), body).expect("写 shim 失败");
        format!("{name}.cmd")
    }
    #[cfg(not(windows))]
    {
        let mut body = String::from("#!/bin/sh\n");
        for (text, to_stderr) in lines {
            let suffix = if *to_stderr { " >&2" } else { "" };
            body.push_str(&format!("echo {text}{suffix}\n"));
        }
        if let Some(code) = exit {
            body.push_str(&format!("exit {code}\n"));
        }
        let script = ws.path().join(name);
        fs::write(&script, body).expect("写 shim 失败");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        name.to_owned()
    }
}

/// 一次 static-check 执行（收口产出；`name` 为输出记录目录派生键；`slot`
/// 报告槽位——报告锚点 / exec-outputs 落点按槽位派生，check-before-flow-start）。
async fn check(root: &str, name: &str, slot: ReportSlot) -> Result<StaticCheckOutcome, String> {
    let output = ProcessStaticCheck::new()
        .run(
            root,
            name,
            slot,
            foundation::cancel::CancelFlag::never(),
            None,
        )
        .await?;
    match output {
        ToolStepOutput::StaticCheck(outcome) => Ok(outcome),
        other => panic!("产出应为 StaticCheck 变体，实际: {other:?}"),
    }
}

/// 缺省槽位（`ReportSlot::Test`）包装——既有用例零改动。
async fn check_test(root: &str, name: &str) -> Result<StaticCheckOutcome, String> {
    check(root, name, ReportSlot::Test).await
}

/// 输出记录回查（bytes 面）。
fn read_output(ws: &TempWs, change: &str, exec_id: &str) -> Vec<u8> {
    exec::read_exec_output(ws.path(), change, exec_id).expect("回查应命中")
}

#[tokio::test]
async fn 通过形态_passed真_诊断空_cwd为root() {
    let _env = env_window();
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

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("通过形态应 Ok");

    assert!(outcome.passed, "退出码 0 → passed");
    assert!(
        outcome.diagnostics.is_empty(),
        "诊断空（无输出命令），实际: {}",
        outcome.diagnostics
    );
}

#[tokio::test]
async fn 失败形态_passed假_诊断捕获不丢() {
    let _env = env_window();
    let ws = TempWs::new("fail");
    // 平台 shim（stderr 行 + stdout 行 + 非零退出）：2>&1 后两行同流捕获，
    // 序即真实写出序（不静默丢任一面）
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(
        &ws,
        "fail-shim",
        &[
            ("lint-failed-to-stderr", true),
            ("lint-failed-to-stdout", false),
        ],
        Some(3),
    );
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("失败形态仍 Ok 携产出");

    assert!(!outcome.passed, "非零退出码 → failed（反馈边原料）");
    assert!(
        outcome.diagnostics.contains("lint-failed-to-stderr")
            && outcome.diagnostics.contains("lint-failed-to-stdout"),
        "stderr / stdout 同流捕获不丢（反馈边注入原料——AC-4），实际: {}",
        outcome.diagnostics
    );
    assert_eq!(
        outcome.diagnostics.lines().collect::<Vec<_>>(),
        ["lint-failed-to-stderr", "lint-failed-to-stdout"],
        "非零退出形态诊断序同真实写出序"
    );
}

#[tokio::test]
async fn 无配置或空命令直接过_零spawn且零输出引用() {
    // config 无 static_analysis 键（无配置 = passed 直过，不拉子进程）
    let ws = TempWs::new("no-config");
    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("无配置应直过");
    assert!(outcome.passed && outcome.diagnostics.is_empty());
    assert_eq!(
        outcome.output_ref, None,
        "零 spawn 即无输出记录（不落空 .log）"
    );

    // 显式空白命令（trim 后空 = 直过语义）
    let ws_blank = TempWs::new("blank-command");
    ws_blank.write_config(Some("   "));
    let outcome = check_test(&ws_blank.root_str(), "demo-change")
        .await
        .expect("空命令应直过");
    assert!(outcome.passed, "空串命令直过语义");
    assert_eq!(outcome.output_ref, None);
}

#[tokio::test]
async fn 非零退出码判定_诊断空亦不误判通过() {
    let _env = env_window();
    let ws = TempWs::new("nonzero-silent");
    // 首 token 为真实可达程序（shell 内建字不构成可达程序——程序解析前置会先拒）
    #[cfg(windows)]
    let command = r#"cmd /C exit /b 7"#;
    #[cfg(not(windows))]
    let command = "false";
    ws.write_config(Some(command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("Ok 携产出");

    assert!(!outcome.passed, "退出码即判据——诊断空不误判通过");
}

/// 命令不可达 → 显式 Err（spawn 前程序解析前置）：裸名不在 PATH（Windows 额外
/// 查 cwd 亦未命中）→ Err（shell 会把命令缺失吞成非零退出码——passed=false
/// 反馈边空转烧预算，配置错误停给用户，不静默直过也不落产出）。
#[tokio::test]
async fn 命令不存在err显式不静默() {
    let _env = env_window();
    let ws = TempWs::new("missing-command");
    ws.write_config(Some("no-such-static-check-command-xyz"));

    let err = check_test(&ws.root_str(), "demo-change")
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
    let _env = env_window();
    let ws = TempWs::new("relative-missing");
    #[cfg(windows)]
    let command = "tools/absent-check.cmd";
    #[cfg(not(windows))]
    let command = "tools/absent-check";
    ws.write_config(Some(command));

    let err = check_test(&ws.root_str(), "demo-change")
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
    let _env = env_window();
    let ws = TempWs::new("launch-fail");
    ws.write_config(Some("false"));

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let result = check_test(&ws.root_str(), "demo-change").await;
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let err = result.expect_err("shell 不可达应 Err");
    assert!(
        err.contains("拉起失败"),
        "Err 记因（AC-4 记因面），实际: {err}"
    );
}

// ---------------------------------------------------------------------------
// 输出记录（change-flow-show-step-only，AC-6 / D4）：`ProcessStaticCheck::new()`
// 零字段单位语义 + `run(root, name)` 默认即 recorded——`{command} 2>&1` 单一
// 交错流落单一 `.log` 并恒回填 `output_ref`；`.err` 分文件退役。
// ---------------------------------------------------------------------------

/// 正向：`new()` 零字段单位语义 + `run(root, name)` 默认 recorded → `output_ref`
/// 恒回填 `Some(exec_id)`，`read_exec_output` 读回内容 = 交错 stdout（与
/// `diagnostics` 同流）；落盘仅 `<exec_id>.log`（零 `.err`）。
#[tokio::test]
async fn 默认recorded_引用恒回填且交错流落单一log() {
    let _env = env_window();
    let ws = TempWs::new("recorded-default");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(
        &ws,
        "interleave-shim",
        &[
            ("out-1", false),
            ("err-1", true),
            ("out-2", false),
            ("err-2", true),
        ],
        None,
    );
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("recorded 形态应 Ok");

    assert!(outcome.passed, "四行输出命令退出 0 → passed");
    // 交错序逐行成立：stderr 未沉底（非「stdout 全量 + stderr 全量」拼接形态）
    let lines: Vec<&str> = outcome.diagnostics.lines().collect();
    assert_eq!(
        lines,
        ["out-1", "err-1", "out-2", "err-2"],
        "诊断 = 真实交错序（2>&1 同流），实际: {:?}",
        outcome.diagnostics
    );

    let exec_id = outcome
        .output_ref
        .clone()
        .expect("默认 recorded → output_ref 恒回填 Some(exec_id)");
    assert!(exec_id.starts_with("exec-"), "exec_id 定式: {exec_id}");
    let read_back = read_output(&ws, "demo-change", &exec_id);
    let recorded_text = String::from_utf8_lossy(&read_back).into_owned();
    let recorded: Vec<&str> = recorded_text.lines().collect();
    assert_eq!(
        recorded,
        ["out-1", "err-1", "out-2", "err-2"],
        "record 内容 = 交错 stdout（与 diagnostics 同流同序）"
    );

    let dir = ws.exec_dir("demo-change");
    assert!(
        dir.join(format!("{exec_id}.log")).is_file(),
        "落盘 <report_dir>/exec-outputs/<exec_id>.log"
    );
    assert!(
        !dir.join(format!("{exec_id}.err")).exists(),
        ".err 分文件退役（单一 .log 承载全量）"
    );
    // 形态适配：记录面新增命令 sidecar `<exec_id>.cmd`（同目录同 id 前缀），故只
    // 裁 `.log` 集合判定「单次执行恰一个输出记录文件」
    let names: Vec<String> = fs::read_dir(&dir)
        .expect("读输出记录目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".log"))
        .collect();
    assert_eq!(names, [format!("{exec_id}.log")], "单次执行恰一个记录文件");
}

/// 正向：`run(root, name)` 收 `name` 作为记录目录派生键——落盘位于该 name 的
/// change 报告目录树，与 id 形态无关联（名称化磁盘面）。
#[tokio::test]
async fn 落盘目录按name派生_与id形态无关() {
    let _env = env_window();
    let ws = TempWs::new("name-derived");
    #[cfg(windows)]
    let command = r#"cmd /C echo name-derived-line"#;
    #[cfg(not(windows))]
    let command = "echo name-derived-line";
    ws.write_config(Some(command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("执行应 Ok");
    let exec_id = outcome.output_ref.clone().expect("回填引用");

    let expected_dir =
        foundation::layout::change_test_reports(ws.path(), "demo-change").join("exec-outputs");
    assert!(
        expected_dir.join(format!("{exec_id}.log")).is_file(),
        "记录目录经 change name 派生: {}",
        expected_dir.display()
    );
    assert!(
        String::from_utf8_lossy(&read_output(&ws, "demo-change", &exec_id))
            .contains("name-derived-line")
    );
    // 以另一 change 名回查 → miss（目录按 name 隔离）
    assert!(
        exec::read_exec_output(ws.path(), "other-change", &exec_id).is_none(),
        "跨 name 回查 None（磁盘面按 name 定位）"
    );
}

/// 边界：命令仅向 stderr 写 → `2>&1` 合并后诊断含该文本（stdout 管道捕获面成立）；
/// 空输出命令 → 落空 `.log` 可回查（`Some(vec![])`）。
#[tokio::test]
async fn 仅stderr输出与空输出_单流捕获与空log可回查() {
    let _env = env_window();
    let ws = TempWs::new("stderr-only");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(
        &ws,
        "stderr-only-shim",
        &[("only-stderr-marker", true)],
        None,
    );
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("执行应 Ok");
    assert!(
        outcome.diagnostics.contains("only-stderr-marker"),
        "仅 stderr 写的文本经 2>&1 并入诊断，实际: {:?}",
        outcome.diagnostics
    );
    let exec_id = outcome.output_ref.clone().expect("回填引用");
    assert_eq!(
        String::from_utf8_lossy(&read_output(&ws, "demo-change", &exec_id)).trim_end(),
        "only-stderr-marker",
        "记录与诊断同流（stderr 文本在单一 .log 内）"
    );

    // 空输出命令：落空 .log 仍可回查（引用式不虚构内容）
    let ws_empty = TempWs::new("empty-output");
    #[cfg(windows)]
    let quiet = r#"cmd /C exit /b 0"#;
    #[cfg(not(windows))]
    let quiet = "true";
    ws_empty.write_config(Some(quiet));
    let outcome = check_test(&ws_empty.root_str(), "demo-change")
        .await
        .expect("静默命令应 Ok");
    let exec_id = outcome.output_ref.clone().expect("空输出仍回填引用");
    assert_eq!(
        read_output(&ws_empty, "demo-change", &exec_id),
        Vec::<u8>::new(),
        "空输出 → 空 .log 可回查（Some(vec![])）"
    );
}

/// 异常：落盘失败（报告树路径被文件占位）→ `output_ref` 降级 None，检查主流程
/// 照常返回判定（不因记录失败阻断 static-check）。
#[tokio::test]
async fn 落盘失败降级none不阻断主流程() {
    let _env = env_window();
    let ws = TempWs::new("record-degrade");
    #[cfg(windows)]
    let command = r#"cmd /C echo degrade-line"#;
    #[cfg(not(windows))]
    let command = "echo degrade-line";
    ws.write_config(Some(command));
    // 报告树路径被文件占位 → exec-outputs 目录不可建
    let reports = foundation::layout::change_test_reports(ws.path(), "demo-change");
    fs::create_dir_all(reports.parent().expect("父目录")).expect("创建父目录失败");
    fs::write(&reports, "占位文件").expect("占位写失败");

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("记录失败不阻断检查主流程");

    assert_eq!(
        outcome.output_ref, None,
        "落盘失败降级 None（不阻断、不虚构引用）"
    );
    assert!(outcome.passed, "判定照常返回");
    assert!(
        outcome.diagnostics.contains("degrade-line"),
        "诊断面不受记录失败影响: {:?}",
        outcome.diagnostics
    );
}

/// 幂等：重复 `run(root, name)` → 每次新铸互异 `exec_id`、仅 `<exec_id>.log`
/// 落盘（零 `.err` 残留），前次 `.log` 不被覆写；重复回读各自 `exec_id` 内容
/// 一致（记录面无副作用）。
#[tokio::test]
async fn 重复run_各自exec_id落独立log且前次不被覆写() {
    let _env = env_window();
    let ws = TempWs::new("idempotent");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(
        &ws,
        "idempotent-shim",
        &[("first-line", false), ("second-line", true)],
        None,
    );
    ws.write_config(Some(&command));

    let first = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("首跑应 Ok");
    let second = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("重跑应 Ok");

    let first_id = first.output_ref.clone().expect("首跑回填引用");
    let second_id = second.output_ref.clone().expect("重跑回填引用");
    assert_ne!(first_id, second_id, "每次执行铸造新 exec_id（引用互异）");

    let first_bytes = read_output(&ws, "demo-change", &first_id);
    let second_bytes = read_output(&ws, "demo-change", &second_id);
    assert_eq!(first_bytes, second_bytes, "两次执行内容各自保真");
    assert_eq!(
        String::from_utf8_lossy(&first_bytes)
            .lines()
            .collect::<Vec<_>>(),
        ["first-line", "second-line"]
    );
    // 前次记录未被覆写（重复回读恒同字节）
    assert_eq!(
        read_output(&ws, "demo-change", &first_id),
        first_bytes,
        "前次 .log 未被覆写"
    );

    // 形态适配：记录面新增命令 sidecar `<exec_id>.cmd`（同目录同 id 前缀），故只
    // 裁 `.log` 集合判定「各自独立 .log、零 .err 残留」
    let mut names: Vec<String> = fs::read_dir(ws.exec_dir("demo-change"))
        .expect("读输出记录目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".log"))
        .collect();
    names.sort();
    let mut expected = vec![format!("{first_id}.log"), format!("{second_id}.log")];
    expected.sort();
    assert_eq!(names, expected, "仅 .log 文件（零 .err 残留、零清理）");
}

// ---------------------------------------------------------------------------
// slot 槽位（check-before-flow-start / AC-1 / AC-2）：报告锚点 / exec-outputs
// 按槽位派生、`StaticCheckOutcome.report_dir` 回填（pass / fail / 无配置三分支
// 均携）；执行语义零分叉（Mock策略：真件 shim + PATH 窗口沿既有装置——spawn
// 为被测本体不 mock）
// ---------------------------------------------------------------------------

/// slot=Baseline pass 场景（正向）：`report_dir` = baseline 报告树锚点、
/// `output_ref` 落盘；`.log` 输出记录落 `<change>/reports/baseline/exec-outputs/`
///（经 `record_exec_output` slot 透传——AC-1 报告分离半边；与 `reports/test`
/// 树零混居）。
#[tokio::test]
async fn run_slot_baseline_pass_报告与exec_outputs落baseline树() {
    let _env = env_window();
    let ws = TempWs::new("baseline-pass");
    // shim 裸名经 PATH 解析（与其余 shim 用例同装置）：不依赖 cmd 的 cwd 搜索，
    // NoDefaultCurrentDirectoryInExePath 环境下同样命中
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "baseline-lint", &[("ok", false)], None);
    ws.write_config(Some(&command));

    let outcome = check(&ws.root_str(), "demo-change", ReportSlot::Baseline)
        .await
        .expect("baseline pass 应 Ok");

    assert!(outcome.passed, "pass 场景 passed=true");
    assert!(
        outcome
            .report_dir
            .ends_with(&format!("demo-change{SEP}reports{SEP}baseline")),
        "report_dir = baseline 报告树锚点: {}",
        outcome.report_dir
    );
    let exec_id = outcome.output_ref.as_deref().expect("recorded 引用应回填");
    // .log 落 baseline 树（零混居）
    let baseline_log = ws
        .exec_dir_slot("demo-change", ReportSlot::Baseline)
        .join(format!("{exec_id}.log"));
    assert!(
        baseline_log.is_file(),
        "输出记录落 baseline exec-outputs: {baseline_log:?}"
    );
    // test 树零残留（分树隔离）
    assert!(
        !ws.exec_dir("demo-change").exists(),
        "test 树零混居（exec-outputs 不落 reports/test）"
    );
}

const SEP: &str = std::path::MAIN_SEPARATOR_STR;

/// slot=Baseline fail 场景（异常）：shim 非零退出 → passed=false + diagnostics；
/// report_dir 仍回填 baseline 锚（fail 分支亦携——walker static 红文案供给面）。
#[tokio::test]
async fn run_slot_baseline_fail_report_dir仍回填baseline锚() {
    let _env = env_window();
    let ws = TempWs::new("baseline-fail");
    // 同上：shim 裸名靠 PATH 前置可达（cwd 搜索在 NoDefaultCurrentDirectoryInExePath
    // 环境被禁用，不能作为可达性依赖）
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "baseline-lint", &[("E0308 类型错误", true)], Some(2));
    ws.write_config(Some(&command));

    let outcome = check(&ws.root_str(), "demo-change", ReportSlot::Baseline)
        .await
        .expect("fail 场景仍 Ok（passed=false 语义）");

    assert!(!outcome.passed, "fail 场景 passed=false");
    assert!(
        outcome.diagnostics.contains("E0308"),
        "诊断捕获不丢: {}",
        outcome.diagnostics
    );
    assert!(
        outcome
            .report_dir
            .ends_with(&format!("demo-change{SEP}reports{SEP}baseline")),
        "fail 分支 report_dir 仍回填 baseline 锚: {}",
        outcome.report_dir
    );
}

/// run 无配置直过（边界）：`openspec/config.json` 无 static_analysis 配置 →
/// 直过 passed、零 spawn；report_dir 亦回填本槽位锚点（语义 = 本槽位报告树
/// 锚点——「无配置分支 report_dir 亦回填」锚）。
#[tokio::test]
async fn run无配置直过_report_dir亦回填本槽位锚点() {
    let _env = env_window();
    let ws = TempWs::new("no-config-slot");
    ws.write_config(None);

    for slot in [ReportSlot::Test, ReportSlot::Baseline] {
        let outcome = check(&ws.root_str(), "demo-change", slot)
            .await
            .expect("无配置直过应 Ok");
        assert!(outcome.passed, "slot={slot:?} 直过 passed");
        assert_eq!(outcome.output_ref, None, "slot={slot:?} 零 spawn 零引用");
        let expect_suffix = match slot {
            ReportSlot::Test => format!("demo-change{SEP}reports{SEP}test"),
            ReportSlot::Baseline => format!("demo-change{SEP}reports{SEP}baseline"),
            // archi 槽位归架构门禁 runner（本域不接受——编译级补位）
            ReportSlot::Archi => format!("demo-change{SEP}reports{SEP}archi"),
        };
        assert!(
            outcome.report_dir.ends_with(&expect_suffix),
            "slot={slot:?} report_dir 回填本槽位锚点: {}",
            outcome.report_dir
        );
    }
}

/// run 槽位等价性（边界，枚举遍历）：slot=Test / Baseline 两值行为等价——同
/// shim 同退出码下 passed / diagnostics / output_ref 语义仅锚点不同（执行语义
/// 零分叉锚）；程序不可达显式 Err 持衡（slot 参数化不改变 Err 面）。
#[tokio::test]
async fn run槽位等价性_语义同锚点异且err面持衡() {
    let _env = env_window();
    // 等价性：同 shim 同退出码两槽位行为等值
    let ws = TempWs::new("slot-equivalence");
    // shim 裸名经 PATH 解析（否则两槽位都「命令不可达」→ passed=false 且诊断相同，
    // 等价性断言在双失败上空转——必须真执行才成立）
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(
        &ws,
        "lint",
        &[("warning: unused", true), ("done", false)],
        Some(1),
    );
    ws.write_config(Some(&command));

    let test_outcome = check(&ws.root_str(), "demo-change", ReportSlot::Test)
        .await
        .expect("test 槽位应 Ok");
    let baseline_outcome = check(&ws.root_str(), "demo-change", ReportSlot::Baseline)
        .await
        .expect("baseline 槽位应 Ok");

    assert_eq!(
        test_outcome.passed, baseline_outcome.passed,
        "passed 语义等值"
    );
    assert_eq!(
        test_outcome.diagnostics, baseline_outcome.diagnostics,
        "diagnostics 语义等值（执行零分叉）"
    );
    assert!(
        test_outcome.output_ref.is_some() && baseline_outcome.output_ref.is_some(),
        "recorded 引用两槽位均回填"
    );
    // 非空转锚：两槽位都真跑到 shim（否则「双不可达」下 passed / 诊断也等值）
    assert!(
        test_outcome.diagnostics.contains("warning: unused")
            && baseline_outcome.diagnostics.contains("warning: unused"),
        "两槽位均真执行 shim，实际: {:?} / {:?}",
        test_outcome.diagnostics,
        baseline_outcome.diagnostics
    );
    assert_ne!(
        test_outcome.report_dir, baseline_outcome.report_dir,
        "锚点不同"
    );
    assert!(
        test_outcome.report_dir.ends_with("reports/test")
            || test_outcome
                .report_dir
                .ends_with(&format!("reports{SEP}test")),
        "test 槽位锚 reports/test: {}",
        test_outcome.report_dir
    );
    assert!(
        baseline_outcome
            .report_dir
            .ends_with(&format!("reports{SEP}baseline")),
        "baseline 槽位锚 reports/baseline: {}",
        baseline_outcome.report_dir
    );

    // Err 面：裸名不在 PATH → 显式 Err（slot 参数化不改变 Err 面——回归锚）
    let ws_missing = TempWs::new("slot-err");
    ws_missing.write_config(Some("definitely-not-on-path-shim"));
    for slot in [ReportSlot::Test, ReportSlot::Baseline] {
        let err = check(&ws_missing.root_str(), "demo-change", slot)
            .await
            .expect_err("程序不可达应 Err");
        assert!(!err.is_empty(), "slot={slot:?} Err 串语义持衡");
    }
}

// ---------------------------------------------------------------------------
// 超时 / 取消（test-design「static_check.rs -> static_check_test.rs」节，
// AC-2 / AC-3 / AC-7）：static-check 首获超时预算（此前零超时 → cargo 卡文件
// 锁时永久悬挂）；取消经 `run_shell` 入口短路与轮询两径收敛为
// `fault = Some(RunFault::Stopped)` 的正常产出（**不 Err**——旧
// 「`Err("执行已取消")`」过渡口径已被本变更取代）。
// ---------------------------------------------------------------------------

use std::time::{Duration, Instant};

use foundation::cancel::CancelFlag;

use crate::static_check::STATIC_CHECK_TIMEOUT;

/// 在 workspace 根写入自旋 shim（先落自身 pid 再挂起——超时 / 取消臂的「进程
/// 恒活」形态与「被杀后 pid 回收」断言面）。`2>&1` 由生产侧命令串追加，shim
/// 自身不必带。
///
/// Windows 臂的直接子进程是 `cmd.exe`（无内建 pid 变量），故 `.cmd` 委派 msys
/// bash 自旋脚本记录 winpid——`taskkill /T /F` 整树击杀后该 pid 必消失（整树
/// 语义的可稳定观测面）；unix 臂直接以 `sh` 进程 pid 为直接子进程面。
fn write_spin_shim(ws: &TempWs, name: &str, pid_file: &Path) -> String {
    #[cfg(windows)]
    {
        let script = ws.path().join(format!("{name}.sh"));
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nwin=$(cat /proc/$$/winpid 2>/dev/null || printf '%s' \"$$\")\nprintf '%s' \"$win\" > \"{}\"\nwhile :; do sleep 0.1; done\n",
                pid_file.display()
            ),
        )
        .expect("写自旋 bash 脚本失败");
        let bash = match exec::resolve_shell() {
            exec::ShellBase::GitBash(path) => path,
            _ => panic!("自旋 shim 需要 Git for Windows bash（resolve_shell 未探测到 GitBash）"),
        };
        let body = format!(
            "@echo off\r\n\"{}\" \"{}\"\r\n",
            bash.display(),
            script.display()
        );
        fs::write(ws.path().join(format!("{name}.cmd")), body).expect("写自旋 shim 失败");
        format!("{name}.cmd")
    }
    #[cfg(not(windows))]
    {
        let body = format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > \"{}\"\nwhile :; do sleep 0.1; done\n",
            pid_file.display()
        );
        let script = ws.path().join(name);
        fs::write(&script, body).expect("写自旋 shim 失败");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        name.to_owned()
    }
}

/// 自旋 shim 落盘的 pid（winpid 口径——`tasklist` / `kill -0` 可探测）。
fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse().ok())
}

/// 等自旋 shim 落 pid（真实时钟握手：虚拟时钟下进程启动不受影响，等待以真实
/// 睡眠 + 让出表达）。
async fn wait_pid(path: &Path, budget: Duration) -> u32 {
    let deadline = Instant::now() + budget;
    loop {
        if let Some(pid) = read_pid(path) {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "等待自旋 shim 落 pid 超时（{budget:?}）"
        );
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// pid 存活探测（Windows `tasklist`、unix `kill -0`）。
#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output();
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// 等待 pid 消失（杀树是尽力语义 + 平台回收时延，轮询到上界即判定仍存活）。
fn wait_pid_gone(pid: u32, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !pid_alive(pid)
}

/// 边界（AC-2）：`STATIC_CHECK_TIMEOUT` 常量锚定 1200 s——预算单点消费，取值
/// 改动即被击杀。
#[test]
fn static_check_timeout常量锚定() {
    assert_eq!(
        STATIC_CHECK_TIMEOUT,
        Duration::from_secs(1200),
        "static-check 超时预算 1200 秒（冷构建约 90 s 的约 3 倍余量）"
    );
    assert!(
        STATIC_CHECK_TIMEOUT > Duration::from_secs(600),
        "预算须显著高于既有 suite 命令口径（原零超时缺陷不得复发）"
    );
}

/// 正向（AC-7 / checks-domain）：超时臂 —— 自旋 shim + 虚拟时钟推进越过
/// `STATIC_CHECK_TIMEOUT` → **`Ok`（不再 `Err`）**、`passed == false`、
/// `fault == Some(Environment("static-check 命令超时（1200 秒）强制终止"))`
///（文案逐字锚定 1200 秒）+ 墙钟上界（不悬挂）+ 目标 pid 消失。
#[tokio::test(start_paused = true)]
async fn 超时臂_fault为环境错误且pid消失不悬挂() {
    let _env = env_window();
    let ws = TempWs::new("timeout");
    let _path = PathPrefix::enter(ws.path());
    let pid_file = ws.path().join("static-pid.txt");
    let command = write_spin_shim(&ws, "spin-timeout-shim", &pid_file);
    ws.write_config(Some(&command));

    // 运行体在任务内（主任务负责「等 pid 落盘 → 推进虚拟时钟」的握手）
    let root_str = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessStaticCheck::new()
            .run(
                &root_str,
                "demo-change",
                ReportSlot::Test,
                CancelFlag::never(),
                None,
            )
            .await
    });
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;

    let started = Instant::now();
    tokio::time::advance(STATIC_CHECK_TIMEOUT + Duration::from_secs(60)).await;
    let output = runner
        .await
        .expect("static-check 任务应正常结束")
        .expect("超时不 Err（执行面归因走 fault）");
    let ToolStepOutput::StaticCheck(outcome) = output else {
        panic!("产出应为 StaticCheck 变体");
    };
    let elapsed = started.elapsed();

    assert!(!outcome.passed, "超时臂 passed 恒 false（不吞成绿）");
    assert_eq!(
        outcome.fault,
        Some(orchestration::port::RunFault::Environment(
            "static-check 命令超时（1200 秒）强制终止".to_owned()
        )),
        "超时文案逐字（预算秒数由常量拼装）且由 fault 承载"
    );
    assert!(
        elapsed < Duration::from_secs(120),
        "墙钟上界（不悬挂，实际 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "超时强杀后目标 pid {pid} 应消失（kill_tree 整树回收）"
    );
}

/// 正向（AC-7）：已置位 cancel —— 入口短路不 spawn，产出以
/// `fault = Some(Stopped)` 正常返回（**MUST NOT 返回 `Err`**）、`passed == false`；
/// 记录面为「单次尝试原样部分字节」（`attempts == 1` → 零分隔头，取消面同样回填
/// `output_ref`，`exec-outputs` 落空 `.log`）。
#[tokio::test]
async fn 已置位cancel_fault为stopped且零spawn() {
    let _env = env_window();
    let ws = TempWs::new("cancel-pre");
    let _path = PathPrefix::enter(ws.path());
    let marker = ws.path().join("shim-ran.txt");
    #[cfg(windows)]
    let command = {
        fs::write(
            ws.path().join("marker-shim.cmd"),
            format!("@echo off\r\necho ran > \"{}\"\r\n", marker.display()),
        )
        .expect("写标记 shim");
        "marker-shim.cmd".to_owned()
    };
    #[cfg(not(windows))]
    let command = {
        let script = ws.path().join("marker-shim");
        fs::write(
            &script,
            format!("#!/bin/sh\nprintf ran > \"{}\"\n", marker.display()),
        )
        .expect("写标记 shim");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        "marker-shim".to_owned()
    };
    ws.write_config(Some(&command));

    let cancel = CancelFlag::never();
    cancel.handle().set();
    let started = Instant::now();
    let output = ProcessStaticCheck::new()
        .run(
            &ws.root_str(),
            "demo-change",
            ReportSlot::Test,
            cancel,
            None,
        )
        .await
        .expect("取消不 Err（产出携 fault）");
    let ToolStepOutput::StaticCheck(outcome) = output else {
        panic!("产出应为 StaticCheck 变体");
    };
    assert_eq!(
        outcome.fault,
        Some(orchestration::port::RunFault::Stopped),
        "取消 → fault = Stopped（run 收敛 stopped，非 failed）"
    );
    assert!(!outcome.passed, "取消臂 passed 恒 false");
    assert!(
        outcome.diagnostics.is_empty(),
        "取消臂零诊断（未读任何输出）"
    );
    assert!(!marker.exists(), "入口短路不 spawn（shim 零执行）");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "短路墙钟近零（不依赖轮询时序）"
    );
    let exec_id = outcome
        .output_ref
        .expect("记录面单点回填引用（落盘不阻断）");
    assert!(
        read_output(&ws, "demo-change", &exec_id).is_empty(),
        "取消短路零尝试字节 → `.log` 空"
    );
}

/// 边界（AC-4 / D4 分支优先级）：**无配置 + 已置位 cancel**——取消短路发生在
/// `run_shell` 内，故「无配置 / 空命令直过」分支先于 spawn 返回：取消已置位仍
/// `Ok(passed = true)`（不 `Err`）、零 spawn（无 `.log` 输出记录）、墙钟近零、
/// report_dir 回填本槽位锚点。
#[tokio::test]
async fn 无配置加已置位cancel_仍直过零spawn不err() {
    let _env = env_window();
    let ws = TempWs::new("no-config-cancel-pre");
    ws.write_config(None);

    let cancel = CancelFlag::never();
    cancel.handle().set();
    let started = Instant::now();
    let output = ProcessStaticCheck::new()
        .run(
            &ws.root_str(),
            "demo-change",
            ReportSlot::Test,
            cancel,
            None,
        )
        .await
        .expect("无配置直过先于取消短路（取消已置位亦不 Err）");
    let elapsed = started.elapsed();
    let outcome = match output {
        ToolStepOutput::StaticCheck(outcome) => outcome,
        other => panic!("产出应为 StaticCheck 变体，实际: {other:?}"),
    };

    assert!(outcome.passed, "无配置 → passed 直过（非取消 Err）");
    assert!(
        outcome.diagnostics.is_empty(),
        "诊断空（零 spawn 即零输出），实际: {}",
        outcome.diagnostics
    );
    assert_eq!(
        outcome.output_ref, None,
        "零 spawn 即无输出记录（不落空 .log）"
    );
    let expect_suffix = format!("demo-change{SEP}reports{SEP}test");
    assert!(
        outcome.report_dir.ends_with(&expect_suffix),
        "report_dir 仍回填本槽位锚点: {}",
        outcome.report_dir
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "直过墙钟近零（不依赖取消轮询时序，实际 {elapsed:?}）"
    );
    let exec_dir = ws.exec_dir("demo-change");
    assert!(
        !exec_dir.exists()
            || fs::read_dir(&exec_dir)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(true),
        "零 spawn 零输出记录（exec-outputs 无新文件）"
    );
}

/// 异常（既有前置）：程序不可达——裸名不在 PATH → `ensure_program_resolvable`
/// 先 Err 记因（`run_shell` 的 `Spawn` 臂因此不可达）。
#[tokio::test]
async fn 程序不可达_解析前置err且不spawn() {
    let _env = env_window();
    let ws = TempWs::new("unreachable");
    ws.write_config(Some("definitely-not-on-path-shim"));

    let err = check_test(&ws.root_str(), "demo-change")
        .await
        .expect_err("不可达程序应 Err");
    assert!(
        err.contains("未找到") && err.contains("PATH 上无对应可执行"),
        "解析前置记因（spawn 前先解析，Spawn 臂因此不可达）: {err}"
    );
}

/// 诊断文本边界用例装置：落盘 shim（给定行）→ 驱动一次 static-check。
async fn outcome_with_lines(tag: &str, lines: &[ShimLine<'_>]) -> StaticCheckOutcome {
    let ws = TempWs::new(tag);
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "diag-shim", lines, None);
    ws.write_config(Some(&command));
    check_test(&ws.root_str(), "demo-change")
        .await
        .expect("退出 0 形态应 Ok")
}

/// 边界（表驱动 3 形态）：诊断文本边界——输出为空 / 超长（>1000 字符）/ 含
/// 特殊字符（引号、emoji）→ `diagnostics` 逐字（不截断不转义）、`passed` 判定
/// 不受影响。
#[tokio::test]
async fn 诊断文本边界_空超长与特殊字符逐字() {
    let _env = env_window();

    // 形态一：空输出
    let empty = outcome_with_lines("diagnostics-empty", &[]).await;
    assert!(empty.passed, "空输出: 退出 0 → passed");
    assert!(empty.diagnostics.is_empty(), "空输出: 诊断空");

    // 形态二：超长（>1000 字符）不截断
    let long = "L".repeat(1200);
    let oversized = outcome_with_lines("diagnostics-long", &[(long.as_str(), false)]).await;
    assert!(oversized.passed, "超长输出: 退出 0 → passed");
    assert_eq!(
        oversized.diagnostics.lines().count(),
        1,
        "超长输出: 单行（未换行拆散）"
    );
    assert!(
        oversized.diagnostics.len() >= 1200,
        "超长输出: 诊断不截断（实际长度 {}）",
        oversized.diagnostics.len()
    );

    // 形态三：特殊字符（引号 / emoji）不转义
    let special = outcome_with_lines(
        "diagnostics-special",
        &[("quote-double-\"-and-emoji-🎉", false)],
    )
    .await;
    assert!(special.passed, "特殊字符: 退出 0 → passed");
    assert!(
        special.diagnostics.contains('"') && special.diagnostics.contains("🎉"),
        "特殊字符: 诊断逐字（不转义），实际: {:?}",
        special.diagnostics
    );
}

/// 正向（AC-7）：运行中 cancel —— 自旋 shim 落 pid 后挂起，运行中置位 →
/// `Ok` + `fault == Some(Stopped)` + 墙钟上界（秒级收敛，不等
/// `STATIC_CHECK_TIMEOUT`）+ 目标 pid 消失；记录面为「单次尝试原样部分字节」
///（`attempts == 1` → 零分隔头，`output_ref` 回填且 `.log` == 已排水部分字节）。
#[tokio::test]
async fn 运行中cancel_秒级收敛且pid消失() {
    let _env = env_window();
    let ws = TempWs::new("cancel-running");
    let _path = PathPrefix::enter(ws.path());
    let pid_file = ws.path().join("static-pid-running.txt");
    let command = write_spin_shim(&ws, "spin-cancel-shim", &pid_file);
    ws.write_config(Some(&command));

    let cancel = CancelFlag::never();
    let handle = cancel.handle();
    let root_str = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessStaticCheck::new()
            .run(&root_str, "demo-change", ReportSlot::Test, cancel, None)
            .await
    });

    // 等 shim 落 pid（进程已建立）后置位取消
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;
    let started = Instant::now();
    handle.set();

    let output = runner.await.expect("run 任务应正常结束");
    let ToolStepOutput::StaticCheck(outcome) = output.expect("取消不 Err（产出携 fault）")
    else {
        panic!("产出应为 StaticCheck 变体");
    };
    assert_eq!(
        outcome.fault,
        Some(orchestration::port::RunFault::Stopped),
        "运行中取消 → fault = Stopped"
    );
    assert!(!outcome.passed, "取消臂 passed 恒 false");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "秒级收敛（不等 1200 s 预算，实际 {:?}）",
        started.elapsed()
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "取消强杀后目标 pid {pid} 应消失（kill_tree 整树回收）"
    );
    // 记录面：`attempts == 1` → 零分隔头，`.log` 即已排水的部分字节（完整性不
    // 断言——被杀臂排水有界截断）
    let exec_id = outcome.output_ref.expect("取消面同样回填 output_ref");
    let logged = read_output(&ws, "demo-change", &exec_id);
    assert!(
        !String::from_utf8_lossy(&logged).contains("===== attempt"),
        "单次尝试零分隔头（取消面不加审计头）"
    );
}

// ---------------------------------------------------------------------------
// 组合锚：瞬态重试 → 归因 → fault → 同一 `.log`（AC-1 / AC-3）
// ---------------------------------------------------------------------------

/// 自愈 shim：首次拉起落标记并输出锁文本 + 非零，之后输成功行 + 零退出
///（平台双形态；尝试次数由标记文件自身表达，无需计数装置）。
fn write_selfheal_shim(ws: &TempWs, name: &str, marker: &Path, lock_text: &str) -> String {
    #[cfg(windows)]
    {
        let body = format!(
            "@echo off\r\nif exist \"{marker}\" (\r\n  exit /b 0\r\n)\r\necho. > \"{marker}\"\r\necho {lock_text}\r\nexit /b 1\r\n",
            marker = marker.display(),
            lock_text = lock_text,
        );
        fs::write(ws.path().join(format!("{name}.cmd")), body).expect("写自愈 shim");
        format!("{name}.cmd")
    }
    #[cfg(not(windows))]
    {
        let body = format!(
            "#!/bin/sh\nif [ -f \"{marker}\" ]; then\n  exit 0\nfi\n: > \"{marker}\"\nprintf '%s\n' '{lock_text}'\nexit 1\n",
            marker = marker.display(),
            lock_text = lock_text,
        );
        let script = ws.path().join(name);
        fs::write(&script, body).expect("写自愈 shim");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        name.to_owned()
    }
}

/// 分隔头计数（输出记录回查面：按 `===== attempt ` 子串计数）。
fn attempt_headers(ws: &TempWs, change: &str, exec_id: &str) -> usize {
    String::from_utf8_lossy(&read_output(ws, change, exec_id))
        .matches("===== attempt ")
        .count()
}

/// 虚拟时钟闸门：以常驻 `spawn_blocking` 任务抑制 tokio 的「空闲即自动推进」
/// （沿 `exec::run_test::ClockGate` 同型装置）。暂停时钟下等待真实子进程会让虚拟
/// 时间成百倍空转，使「退避 1/2/4/8 s 恰不真等」「1500 s 预算不提前耗尽」
/// 「1200 s 超时臂不被误触」失去确定性；闸门在位时虚拟时间**只**随
/// [`advance_until`] 前进。
struct ClockGate {
    release: Option<std::sync::mpsc::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl ClockGate {
    fn new() -> Self {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let task = tokio::task::spawn_blocking(move || {
            let _ = wait.recv();
        });
        Self {
            release: Some(release),
            task: Some(task),
        }
    }
}

impl Drop for ClockGate {
    fn drop(&mut self) {
        self.release.take();
        self.task.take();
    }
}

/// 重试退避不真等：按 `step` 粒度推进虚拟时钟直到 `until` 为真，真实时间上界
/// `real_budget`（虚拟推进 ≈ 10 × 真实耗时，故 60 s 真实上界对应 ≤ 600 s 虚拟，
/// 既不触 1200 s 超时臂也不触 1500 s 总预算）。
async fn advance_until<F: Fn() -> bool>(until: F, step: Duration, real_budget: Duration) {
    let deadline = Instant::now() + real_budget;
    while !until() && Instant::now() < deadline {
        tokio::time::advance(step).await;
        // 真实让出：子进程排水 / 收尸在真实时间上进行（虚拟时钟不参与）
        let _ = tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(10))).await;
    }
    assert!(
        until(),
        "推进真实预算 {real_budget:?} 后仍未收敛（暂停时钟下虚拟时间不会自行前进）"
    );
}

/// 正向（AC-3 组合锚）：瞬态耗尽 → `fault = Environment`——shim 每次都输出锁文本
/// 并 `exit 1`（`transient()` 5 次耗尽）→ `Ok`、`passed == false`、
/// `fault == Some(Environment(text))`；`text` 含「已自动重试 4 次」、被锁文件路径、
/// 占用者点名或「未定位占用者」降级说明、环境侧指引；`diagnostics` 为末次原文；
/// `.log` 恰 5 段。退避（1+2+4+8 s）随虚拟时钟推进，不真等。
#[tokio::test(start_paused = true)]
async fn 瞬态耗尽_fault为环境错误且log五段() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempWs::new("fault-exhaust");
    let _path = PathPrefix::enter(ws.path());
    let locked = ws.path().join("liba.rlib");
    fs::write(&locked, b"body").expect("落盘被锁文件");
    let lock_text = format!(
        "error: failed to rename archive file: {}: os error 32",
        locked.display()
    );
    let command = write_shim(
        &ws,
        "lock-always-shim",
        &[(lock_text.as_str(), false)],
        Some(1),
    );
    ws.write_config(Some(&command));

    // 运行体在任务内（主任务负责推进虚拟时钟：4 段退避 15 s 不真等）
    let root = ws.root_str();
    let runner = tokio::spawn(async move { check_test(&root, "demo-change").await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(60),
    )
    .await;
    let outcome = runner
        .await
        .expect("static-check 任务应正常结束")
        .expect("耗尽面仍 Ok（执行面归因走 fault）");

    assert!(!outcome.passed, "非零退出 → passed = false");
    let Some(orchestration::port::RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!("瞬态耗尽应产 Environment fault，实际: {:?}", outcome.fault);
    };
    assert!(
        text.contains("已自动重试 4 次"),
        "重试次数按 attempts - 1 计: {text}"
    );
    assert!(
        text.contains(&locked.to_string_lossy().into_owned()),
        "被锁文件路径在案: {text}"
    );
    assert!(
        text.contains("未定位占用者") || text.contains("占用进程："),
        "占用者点名或降级说明在案: {text}"
    );
    assert!(
        text.contains("workspace 父目录") && text.contains("逐 worktree"),
        "环境侧处置指引在案: {text}"
    );
    assert!(
        !text.contains("请在基线上修复"),
        "环境错误文案与基线红文案零共用句: {text}"
    );
    assert_eq!(
        outcome.diagnostics, lock_text,
        "diagnostics 为末次尝试原文（trim_end 后）"
    );

    let exec_id = outcome.output_ref.expect("output_ref 恒回填");
    assert_eq!(
        attempt_headers(&ws, "demo-change", &exec_id),
        5,
        "耗尽面记录 5 段 attempt 分隔头"
    );
    let log = String::from_utf8_lossy(&read_output(&ws, "demo-change", &exec_id)).into_owned();
    for index in 1..=5 {
        assert!(
            log.contains(&format!("===== attempt {index}/5")),
            "序号 {index}/5 在案: {log}"
        );
    }
    assert!(
        log.contains("transient=os error 32"),
        "此前尝试段头携命中签名: {log}"
    );
    assert!(
        !text.contains("重试进展"),
        "非 cargo 形态（全 0 计数）不产推进序列行: {text}"
    );
}

/// 正向（AC-1）：瞬态自愈转绿——shim 首次输出锁文本 `exit 1`、第 2 次 `exit 0`
/// → `passed == true`、`fault == None`、诊断为空；record `.log` 含 2 段（首段
/// `transient=os error 32`、末段 `transient=-`）。退避（1 s）随虚拟时钟推进，不真等。
#[tokio::test(start_paused = true)]
async fn 瞬态自愈转绿_fault为空且log两段() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempWs::new("fault-selfheal");
    let _path = PathPrefix::enter(ws.path());
    let marker = ws.path().join("ran-once.txt");
    let locked = ws.path().join("liba.rlib");
    fs::write(&locked, b"body").expect("落盘被锁文件");
    let lock_text = format!(
        "error: failed to rename archive file: {}: os error 32",
        locked.display()
    );
    let command = write_selfheal_shim(&ws, "selfheal-shim", &marker, &lock_text);
    ws.write_config(Some(&command));

    // 运行体在任务内（主任务推进虚拟时钟：1 s 退避不真等）
    let root = ws.root_str();
    let runner = tokio::spawn(async move { check_test(&root, "demo-change").await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(60),
    )
    .await;
    let outcome = runner
        .await
        .expect("static-check 任务应正常结束")
        .expect("自愈面应 Ok");

    assert!(outcome.passed, "第 2 次尝试转绿 → passed = true");
    assert_eq!(outcome.fault, None, "自愈后不产 fault（绿不误判环境错误）");
    assert!(
        outcome.diagnostics.is_empty(),
        "末次无输出 → 诊断为空白名单: {:?}",
        outcome.diagnostics
    );

    let exec_id = outcome.output_ref.expect("output_ref 恒回填");
    assert_eq!(
        attempt_headers(&ws, "demo-change", &exec_id),
        2,
        "自愈面记录 2 段"
    );
    let log = String::from_utf8_lossy(&read_output(&ws, "demo-change", &exec_id)).into_owned();
    assert!(
        log.contains("===== attempt 1/5 · exit=1 · transient=os error 32 ====="),
        "首段携命中签名: {log}"
    );
    assert!(
        log.contains("===== attempt 2/5 · exit=0 · transient=- ====="),
        "末段无签名（`transient=-`）: {log}"
    );
}

/// 正向（AC-8 / R-1）：真红不产 fault——普通编译错误 + `exit 1` → `passed ==
/// false`、`fault == None`、单次尝试（`.log` 零分隔头）、`diagnostics` 逐字为该
/// 输出（`trim_end` 后）。
#[tokio::test]
async fn 真红不产fault且单次尝试() {
    let _env = env_window();
    let ws = TempWs::new("real-red");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(
        &ws,
        "real-red-shim",
        &[
            ("error[E0308]: mismatched types", true),
            ("  --> src/lib.rs:3:9", false),
        ],
        Some(1),
    );
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("真红仍 Ok 携产出");

    assert!(!outcome.passed, "真红 → passed = false");
    assert_eq!(outcome.fault, None, "真红不产 fault（不吞成环境错误）");
    assert!(
        outcome
            .diagnostics
            .contains("error[E0308]: mismatched types"),
        "诊断为该输出原文: {:?}",
        outcome.diagnostics
    );

    let exec_id = outcome.output_ref.expect("output_ref 恒回填");
    assert_eq!(
        attempt_headers(&ws, "demo-change", &exec_id),
        0,
        "单次尝试零分隔头（既有 record 断言不被破）"
    );
}

/// 边界（R-1）：裸 `failed to write <path>` 不产 fault——`fault == None`、单次
/// 尝试（签名表刻意不收该泛措辞，不当环境错误重试）。
#[tokio::test]
async fn 裸failed_to_write不产fault() {
    let _env = env_window();
    let ws = TempWs::new("bare-write");
    let _path = PathPrefix::enter(ws.path());
    let target = ws.path().join("libb.rmeta");
    let command = write_shim(
        &ws,
        "bare-write-shim",
        &[(
            format!("error: failed to write {}", target.display()).as_str(),
            false,
        )],
        Some(1),
    );
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("非零退出仍 Ok");

    assert!(!outcome.passed, "非零退出 → passed = false");
    assert_eq!(
        outcome.fault, None,
        "裸 `failed to write` 不命中签名 → 不产 fault"
    );

    let exec_id = outcome.output_ref.expect("output_ref 恒回填");
    assert_eq!(
        attempt_headers(&ws, "demo-change", &exec_id),
        0,
        "零重试（单次尝试无分隔头）"
    );
}

/// 边界（D12）：`fault` 与 `passed` 正交——shim 在 `exit 0` 的同时输出签名文本
/// → `passed == true`、`fault == None`（绿不产 fault）；`passed` 恒
/// `exit_code == 0` 口径未被 fault 面改动。
#[tokio::test]
async fn fault与passed正交_绿不产fault() {
    let _env = env_window();
    let ws = TempWs::new("orthogonal");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(
        &ws,
        "green-signature-shim",
        &[("test 打印了 os error 32 这句话", false)],
        Some(0),
    );
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("绿跑应 Ok");

    assert!(outcome.passed, "退出 0 → passed = true");
    assert_eq!(
        outcome.fault, None,
        "绿不产 fault（fault 面未改 passed 口径）"
    );
    assert!(
        outcome.diagnostics.contains("os error 32"),
        "诊断照常捕获该文本: {:?}",
        outcome.diagnostics
    );
}

// ---------------------------------------------------------------------------
// 正向 / 异常（AC-5 组合锚）：推进判据 → fault 文本全链
// （`static_check::execute` → `exec::run_shell`（重试 + 卡死早停）→
// `environment::fault_text` → `StaticCheckOutcome.fault`）
// ---------------------------------------------------------------------------

/// 按尝试序输出 `Compiling` 行数的 shim（平台双形态：Windows `.cmd` +
/// PowerShell、POSIX sh）：第 n 次尝试输出 `counts[n - 1]` 行
/// `   Compiling crate v0.1.0` + `lock_text` 并 `exit 1`；超出清单的尝试输出 0 行。
/// 返回 config 应登记的裸名（Windows 带 `.cmd` 后缀）。
fn write_counting_shim(
    ws: &TempWs,
    name: &str,
    counter: &Path,
    counts: &[u32],
    lock_text: &str,
) -> String {
    #[cfg(windows)]
    {
        let counts_array = counts
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(
            "$cf = '{cf}'\n$n = 0\nif (Test-Path $cf) {{ $n = [int](Get-Content $cf -Raw) }}\n$n = $n + 1\nSet-Content -Path $cf -Value $n -NoNewline -Encoding ascii\n$counts = @({counts})\n$count = 0\nif ($n -le $counts.Count) {{ $count = $counts[$n - 1] }}\nfor ($i = 0; $i -lt $count; $i++) {{ Write-Output '   Compiling crate v0.1.0' }}\nWrite-Output '{lock}'\nexit 1\n",
            cf = counter.display(),
            counts = counts_array,
            lock = lock_text,
        );
        fs::write(ws.path().join(format!("{name}-shim.ps1")), body).expect("写计数 shim 脚本失败");
        let wrapper = format!(
            "@echo off\r\npowershell -NoProfile -ExecutionPolicy Bypass -File \"%~dp0{name}-shim.ps1\" %*\r\nexit /b %errorlevel%\r\n"
        );
        fs::write(ws.path().join(format!("{name}.cmd")), wrapper).expect("写计数 shim 失败");
        format!("{name}.cmd")
    }
    #[cfg(not(windows))]
    {
        let mut arms = String::new();
        for (index, count) in counts.iter().enumerate() {
            arms.push_str(&format!("  {}) count={count} ;;\n", index + 1));
        }
        let body = format!(
            "#!/bin/sh\ncf=\"{cf}\"\nn=$(cat \"$cf\" 2>/dev/null || printf '0')\nn=$((n+1))\nprintf '%s' \"$n\" > \"$cf\"\ncount=0\ncase \"$n\" in\n{arms}esac\ni=0\nwhile [ \"$i\" -lt \"$count\" ]; do\n  printf '   Compiling crate v0.1.0\\n'\n  i=$((i+1))\ndone\nprintf '%s\\n' '{lock}'\nexit 1\n",
            cf = counter.display(),
            arms = arms,
            lock = lock_text,
        );
        let script = ws.path().join(name);
        fs::write(&script, body).expect("写计数 shim 失败");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        name.to_owned()
    }
}

/// 正向（AC-5 组合锚）：卡死推进序列到 fault——shim 第 1 次 264 行、第 2 / 3
/// 次 5 行 `Compiling` + 锁文本 + `exit 1` → 卡死早停（`attempts == 3 < 5`）→
/// `passed == false`、`fault == Some(Environment(text))`，`text` 含
/// `重试进展：待编译 264 → 5 → 5（末两次无进展，判定为卡死）`、「已自动重试 2
/// 次」、被锁文件路径与指引；输出记录恰 3 段 attempt 分隔头；`diagnostics` 为
/// 末次原文（`trim_end`）。
#[tokio::test(start_paused = true)]
async fn 卡死推进序列到fault文本() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempWs::new("stall-sequence");
    let _path = PathPrefix::enter(ws.path());
    let locked = ws.path().join("liba.rlib");
    fs::write(&locked, b"body").expect("落盘被锁文件");
    let lock_text = format!(
        "error: failed to rename archive file: {}: os error 32",
        locked.display()
    );
    let counter = ws.path().join("attempt-count.txt");
    let command = write_counting_shim(&ws, "stall-shim", &counter, &[264, 5, 5], &lock_text);
    ws.write_config(Some(&command));

    let root = ws.root_str();
    let runner = tokio::spawn(async move { check_test(&root, "demo-change").await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(120),
    )
    .await;
    let outcome = runner
        .await
        .expect("static-check 任务应正常结束")
        .expect("卡死面仍 Ok（执行面归因走 fault）");

    assert!(!outcome.passed, "非零退出 → passed = false");
    let Some(orchestration::port::RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!("卡死形态应产 Environment fault，实际: {:?}", outcome.fault);
    };
    assert!(
        text.contains("重试进展：待编译 264 → 5 → 5（末两次无进展，判定为卡死）"),
        "推进序列行与卡死判定语在案: {text}"
    );
    assert!(
        text.contains("已自动重试 2 次"),
        "attempts == 3 → 重试 2 次: {text}"
    );
    assert!(
        text.contains(&locked.to_string_lossy().into_owned()),
        "被锁文件路径在案: {text}"
    );
    assert!(
        text.contains("workspace 父目录") && text.contains("逐 worktree"),
        "环境侧指引在案: {text}"
    );

    let exec_id = outcome.output_ref.expect("output_ref 恒回填");
    assert_eq!(
        attempt_headers(&ws, "demo-change", &exec_id),
        3,
        "输出记录恰 3 段 attempt 分隔头（早停面不再烧满 5 次）"
    );
    let lines = outcome.diagnostics.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 6, "末次原文 5 行 Compiling + 1 行锁文本");
    assert!(
        lines[..5]
            .iter()
            .all(|line| line.trim_start().starts_with("Compiling ")),
        "诊断为末次尝试的原文（Compiling 行）: {:?}",
        outcome.diagnostics
    );
    assert_eq!(
        lines.last().copied(),
        Some(lock_text.as_str()),
        "诊断末行为锁文本（trim_end 后）"
    );
}

/// 正向（AC-2 组合锚）：递减推进无判定语——shim 第 1 / 2 / 3 次分别 264 / 5 / 0
/// 行 `Compiling` + 锁文本、此后恒 0 行 → 递减序列不早停、跑满上限：`text` 含
/// 五个数字（264 → 5 → 0 → 0 → 0）、无卡死判定语；`attempts` 由 5 段分隔头表达。
#[tokio::test(start_paused = true)]
async fn 递减推进无卡死判定语() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempWs::new("stall-decreasing");
    let _path = PathPrefix::enter(ws.path());
    let locked = ws.path().join("libb.rlib");
    fs::write(&locked, b"body").expect("落盘被锁文件");
    let lock_text = format!(
        "error: failed to rename archive file: {}: os error 32",
        locked.display()
    );
    let counter = ws.path().join("attempt-count.txt");
    let command = write_counting_shim(&ws, "decreasing-shim", &counter, &[264, 5, 0], &lock_text);
    ws.write_config(Some(&command));

    let root = ws.root_str();
    let runner = tokio::spawn(async move { check_test(&root, "demo-change").await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(120),
    )
    .await;
    let outcome = runner
        .await
        .expect("static-check 任务应正常结束")
        .expect("耗尽面仍 Ok");

    let Some(orchestration::port::RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!("瞬态耗尽应产 Environment fault，实际: {:?}", outcome.fault);
    };
    assert!(
        text.contains("重试进展：待编译 264 → 5 → 0 → 0 → 0"),
        "递减序列五个数字在案: {text}"
    );
    assert!(!text.contains("判定为卡死"), "末值 0 不产判定语: {text}");
    assert!(
        text.contains("已自动重试 4 次"),
        "跑满上限（attempts == 5）: {text}"
    );
    let exec_id = outcome.output_ref.expect("output_ref 恒回填");
    assert_eq!(
        attempt_headers(&ws, "demo-change", &exec_id),
        5,
        "跑满上限 → 5 段分隔头"
    );
}

// ---------------------------------------------------------------------------
// archi 槽位守卫（D19 / AC-14）：checks 域 MUST NOT 接受 `archi`
// ---------------------------------------------------------------------------

/// 异常（D19）：`ReportSlot::Archi` → `Err`，文案指明槽位归属（不静默执行）。
#[tokio::test]
#[allow(non_snake_case)]
async fn archi槽位显式Err且文案归因() {
    let ws = TempWs::new("archi-slot");
    ws.write_config(None);
    let error = check(&ws.root_str(), "demo-change", ReportSlot::Archi)
        .await
        .expect_err("checks 域 MUST NOT 接受 archi 槽位");
    assert!(
        error.contains("archi") && error.contains("架构门禁"),
        "文案含槽位词与归属方：{error}"
    );
}

/// 边界：守卫先于一切 IO——空目录（无 `openspec/config.json`、无命令配置）仍返回
/// 槽位 `Err`（不是「缺配置」类错误），且零写盘（`reports/archi` 零创建）。
#[tokio::test]
#[allow(non_snake_case)]
async fn archi槽位守卫先于IO且零写盘() {
    let ws = TempWs::new("archi-slot-empty");
    let before = snapshot_root(ws.path());
    let first = check(&ws.root_str(), "demo-change", ReportSlot::Archi)
        .await
        .expect_err("空目录同样拒收");
    let second = check(&ws.root_str(), "demo-change", ReportSlot::Archi)
        .await
        .expect_err("二次调用同样拒收");
    assert_eq!(first, second, "幂等：两次文案逐字相等");
    assert_eq!(
        before,
        snapshot_root(ws.path()),
        "拒收路径零写盘（字节快照相等）"
    );
    assert!(
        !foundation::layout::change_reports(ws.path(), "demo-change", ReportSlot::Archi).exists(),
        "reports/archi 零创建"
    );
}

/// 正向：非 `Archi` 槽位放行——`ReportSlot::Baseline` 于无配置 tempdir 走既有
/// 「无配置直过」链（不是槽位 `Err`），证守卫只收窄第三值。
#[tokio::test]
#[allow(non_snake_case)]
async fn 非archi槽位放行() {
    let ws = TempWs::new("archi-slot-passthrough");
    ws.write_config(None);
    let outcome = check(&ws.root_str(), "demo-change", ReportSlot::Baseline)
        .await
        .expect("baseline 槽位应放行");
    assert!(outcome.passed, "既有直过语义零回归");
}

/// 目录全量字节快照（「零写盘」的行为化断言）。
fn snapshot_root(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn collect(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let Ok(reader) = fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<PathBuf> = reader.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect(&path, out);
            } else if let Ok(bytes) = fs::read(&path) {
                out.push((path, bytes));
            }
        }
    }
    let mut out: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    collect(root, &mut out);
    out
}

/// 边界：零 spawn——`PATH` 隔离窗口（清空，任何外部程序不可达）下 `Archi` 槽位
/// 仍返回槽位 `Err`：守卫先于 spawn，MUST NOT 因环境错短路成「拉起失败」/
///「未找到」类错误（配置错与环境错两类可辨）。
#[tokio::test]
#[allow(non_snake_case)]
async fn archi槽位零spawn_PATH隔离仍返槽位err() {
    let _env = env_window();
    let ws = TempWs::new("archi-slot-no-spawn");
    ws.write_config(Some("no-such-static-check-command-xyz"));

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let result = check(&ws.root_str(), "demo-change", ReportSlot::Archi).await;
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let error = result.expect_err("PATH 隔离下仍应返回槽位 Err");
    assert!(
        error.contains("archi") && error.contains("架构门禁"),
        "文案仍为槽位归属（守卫先于 spawn）：{error}"
    );
    assert!(
        !error.contains("拉起失败") && !error.contains("未找到") && !error.contains("不可达"),
        "不因 PATH 隔离短路成 spawn / 解析类错误：{error}"
    );
    assert!(
        !ws.exec_dir_slot("demo-change", ReportSlot::Archi).exists(),
        "零 spawn ⇒ 零执行记录目录创建"
    );
}

// ---------------------------------------------------------------------------
// 运行中记录链（change-detail-live-stdout，AC-1 / AC-2 / AC-3）
//
// 被测面：`ProcessStaticCheck` 的 `begin → write_command → run_shell(live) →
// commit` 记录链——进程未结束时 `.log` 已存在且内容单调增长；收口以**同一
// id** 覆盖为权威内容；`.cmd` 逐字等于 `{static_analysis} 2>&1`；无配置 / 空
// 命令分支零 `begin`。
//
// Mock 策略（见 test-design 本节 Mock策略表）：真实 `TempWs` tempdir + 真实
// `openspec/config.json` + 真实平台 shim（分阶段输出用 gate 文件时序）；
// `PathPrefix` / `EnvWindow` 为既有 RAII 装置（非模块替身）。
//
// 上抛面（`OutputRefSink`）的构造面为 `pub`（design D16，本轮回溯补正）：checks
// crate 可以直构 `OutputRefSink::new(..)`，故「上抛恰 1 次」「上抛时刻 `.log` 与
// 父目录已在」（AC-11）与「命令不可达 / archi 槽位拒收 ⇒ 回调零调用」两负面臂均在
// **本节**可写，MUST NOT 再以 `None` 兜底。真实双侧（walker × 真实
// `ProcessStaticCheck`）的端到端组合行另落壳层
// `src-tauri/src/commands/change_flow/mod_test.rs`（design D17：`orchestration`
// 加 `checks-runtime` dev-dep 即 core → infra 反向边，不可取）。
// ---------------------------------------------------------------------------

/// change 目录（`exec_output_log_path_in` / `read_exec_command_in` /
/// `read_exec_output_delta_in` 的入参锚）。
fn change_dir(w: &TempWs, change: &str) -> PathBuf {
    foundation::layout::resolve(w.path())
        .changes_root
        .join(change)
}

/// 输出记录目录内当前在场的 `.log` 快照：`(exec_id, 内容字节)`（运行中探测面
/// ——不依赖上抛回调即可观察「进程未结束时文件已存在」）。
fn live_logs(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(reader) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut logs: Vec<(String, Vec<u8>)> = reader
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let exec_id = name.strip_suffix(".log")?.to_owned();
            let bytes = fs::read(entry.path()).ok()?;
            Some((exec_id, bytes))
        })
        .collect();
    logs.sort();
    logs
}

/// 分阶段输出 shim：先写 `first` 行与 marker，随后轮询等待 `gate` 文件出现，
/// 再写 `second` 行（运行中读取的时序原料）。
fn write_staged_shim(ws: &TempWs, name: &str, first: &str, second: &str, gate: &Path) -> String {
    let marker = staged_marker(ws, name);
    #[cfg(windows)]
    {
        let body = format!(
            "@echo off\r\necho {first}\r\necho. > \"{marker}\"\r\n:wait\r\nping -n 1 -w 50 127.0.0.1 > nul\r\nif not exist \"{gate}\" goto wait\r\necho {second}\r\n",
            marker = marker.display(),
            gate = gate.display()
        );
        fs::write(ws.path().join(format!("{name}.cmd")), body).expect("写分阶段 shim 失败");
        format!("{name}.cmd")
    }
    #[cfg(not(windows))]
    {
        let body = format!(
            "#!/bin/sh\necho {first}\n: > \"{marker}\"\nwhile [ ! -f \"{gate}\" ]; do sleep 0.05; done\necho {second}\n",
            marker = marker.display(),
            gate = gate.display()
        );
        let script = ws.path().join(name);
        fs::write(&script, body).expect("写分阶段 shim 失败");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        name.to_owned()
    }
}

/// 分阶段 shim 的 marker 文件路径（第一行已写的观测面）。
fn staged_marker(ws: &TempWs, name: &str) -> PathBuf {
    ws.path().join(format!("{name}-stage-marker.txt"))
}

/// 落原始字节（含非 UTF-8）的 shim：`printf` 逐字节输出（`.log` 字节保真面）。
fn write_raw_shim(ws: &TempWs, name: &str) -> String {
    let body = "#!/bin/sh\nprintf 'raw\\377\\376tail\\n'\n";
    #[cfg(windows)]
    {
        let bash = match exec::resolve_shell() {
            exec::ShellBase::GitBash(path) => path,
            _ => panic!("原始字节 shim 需要 Git for Windows bash（resolve_shell 未探测到）"),
        };
        let script = ws.path().join(format!("{name}.sh"));
        fs::write(&script, body).expect("写原始字节脚本失败");
        let wrapper = format!(
            "@echo off\r\n\"{}\" \"{}\"\r\n",
            bash.display(),
            script.display()
        );
        fs::write(ws.path().join(format!("{name}.cmd")), wrapper).expect("写原始字节 shim 失败");
        format!("{name}.cmd")
    }
    #[cfg(not(windows))]
    {
        let script = ws.path().join(name);
        fs::write(&script, body).expect("写原始字节脚本失败");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        name.to_owned()
    }
}

/// 轮询等待文件出现（真实时间上界 30 s 防悬挂）。
///
/// 以 `spawn_blocking` 的短睡让位：`#[tokio::test]` 默认单线程 runtime，同步
/// `std::thread::sleep` 会把被 spawn 的执行任务一并钉住（shim 的 stdout 无法
/// 排水）。
async fn wait_for_file(path: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        let _ = tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(20))).await;
    }
    path.exists()
}

/// 轮询等待输出记录目录内出现内容非空的 `.log`（运行中读取的取样面；让位
/// 口径同 [`wait_for_file`]）。
async fn wait_for_non_empty_log(dir: &Path) -> Option<(String, Vec<u8>)> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Some(hit) = live_logs(dir)
            .into_iter()
            .find(|(_, bytes)| !bytes.is_empty())
        {
            return Some(hit);
        }
        let _ = tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(20))).await;
    }
    None
}

/// 正向：分阶段写输出的 shim → 进程未结束时 `.log` 已存在且内容已含第一行；
/// 随后再读后读 ⊇ 前读（单调增长，AC-1 / R-11 用包含关系不断言精确长度）。
#[tokio::test]
async fn 运行中读取_进程未结束时log已可读且单调增长() {
    let _env = env_window();
    let ws = TempWs::new("live-progress");
    let _path = PathPrefix::enter(ws.path());
    let gate = ws.path().join("release-gate.txt");
    let command = write_staged_shim(&ws, "staged-shim", "STAGE-ONE", "STAGE-TWO", &gate);
    ws.write_config(Some(&command));
    let marker = staged_marker(&ws, "staged-shim");
    let dir = ws.exec_dir("demo-change");

    let root = ws.root_str();
    let runner = tokio::spawn(async move { check_test(&root, "demo-change").await });

    assert!(wait_for_file(&marker).await, "shim 应写出第一行");
    // 进程仍在跑（gate 未放行）：输出记录目录已建、`.log` 已存在可读
    let live = wait_for_non_empty_log(&dir)
        .await
        .expect("进程未结束时 `.log` 已可读");
    let (live_id, first_read) = live;
    let first_text = String::from_utf8_lossy(&first_read).into_owned();
    assert!(
        first_text.contains("STAGE-ONE"),
        "运行中已读到第一行: {first_text:?}"
    );
    assert!(
        !first_text.contains("STAGE-TWO"),
        "读取时第二行尚未产出: {first_text:?}"
    );

    fs::write(&gate, b"go").expect("放行 shim 失败");
    let outcome = runner
        .await
        .expect("任务应结束")
        .expect("static-check 应 Ok");

    let exec_id = outcome.output_ref.clone().expect("终态回填引用");
    assert_eq!(
        exec_id, live_id,
        "运行中观察到的 id 与终态引用同值（同 id 收口）"
    );
    let final_text =
        String::from_utf8_lossy(&read_output(&ws, "demo-change", &exec_id)).into_owned();
    assert!(
        final_text.contains(first_text.trim_end()),
        "后读 ⊇ 前读（前读 {first_text:?} / 后读 {final_text:?}）"
    );
    assert!(final_text.contains("STAGE-TWO"), "终态含第二行");
}

/// 正向：终态 `.log` 为权威内容——重试面下逐段含 attempt 分隔头（live 交错流被
/// 整体覆盖，AC-2），且 `output_ref` 指向该 `.log`（同 id）。
#[tokio::test(start_paused = true)]
async fn 终态收口_同id覆盖为权威内容() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempWs::new("commit-same-id");
    let _path = PathPrefix::enter(ws.path());
    let marker = ws.path().join("commit-selfheal-once.txt");
    let command = write_selfheal_shim(
        &ws,
        "commit-selfheal",
        &marker,
        "error: failed to rename archive file: /t/liba.rlib: os error 32",
    );
    ws.write_config(Some(&command));

    let root = ws.root_str();
    let runner = tokio::spawn(async move { check_test(&root, "demo-change").await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(60),
    )
    .await;
    let outcome = runner.await.expect("任务应结束").expect("自愈面应 Ok");

    assert!(outcome.passed, "第 2 次尝试转绿");
    let exec_id = outcome.output_ref.clone().expect("终态引用");
    let text = String::from_utf8_lossy(&read_output(&ws, "demo-change", &exec_id)).into_owned();
    assert_eq!(
        text.matches("===== attempt ").count(),
        2,
        "权威内容为两次尝试的拼接（live 交错流被整体覆盖）: {text:?}"
    );
    assert!(
        text.contains("===== attempt 1/5 · exit=1 · transient=os error 32 ====="),
        "首段携命中签名: {text:?}"
    );
    assert!(
        text.contains("===== attempt 2/5 · exit=0 · transient=- ====="),
        "末段为收口语义: {text:?}"
    );
    // 落点与引用同名同目录（同 id 收口的物理载体）
    assert_eq!(
        exec::exec_output_log_path_in(&change_dir(&ws, "demo-change"), &exec_id)
            .expect("派生路径应命中"),
        ws.exec_dir("demo-change").join(format!("{exec_id}.log")),
        "引用指向的 .log 与布局派生同值"
    );
}

/// 正向：`.cmd` 逐字 == `"{static_analysis} 2>&1"`（无尾随换行）、与 `.log`
/// 同目录同 id 前缀（AC-3）。
#[tokio::test]
async fn 命令sidecar_逐字命令串且同目录同id前缀() {
    let _env = env_window();
    let ws = TempWs::new("cmd-sidecar");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "cmd-shim", &[("cmd-line", false)], None);
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("应 Ok");
    let exec_id = outcome.output_ref.clone().expect("回填引用");

    let recorded = exec::read_exec_command_in(&change_dir(&ws, "demo-change"), &exec_id)
        .expect("sidecar 命中");
    assert_eq!(
        recorded,
        format!("{command} 2>&1"),
        "逐字 == {{static_analysis}} 2>&1（无尾随换行）"
    );

    let dir = ws.exec_dir("demo-change");
    let cmd_path = dir.join(format!("{exec_id}.cmd"));
    assert!(cmd_path.is_file(), "sidecar 与 .log 同目录");
    assert_eq!(
        cmd_path.parent(),
        dir.join(format!("{exec_id}.log")).parent(),
        "同目录同 id 前缀"
    );
}

/// 异常（参数化 3 形态）：无配置键 / 空命令 / 全空白命令 → `passed == true`、
/// `output_ref == None`、`reports/<slot>/exec-outputs/` 零新增文件（零 `begin`，
/// AC-1 的负面判据）。
#[tokio::test]
async fn 无配置与空命令三形态零begin零新文件() {
    let _env = env_window();
    for (index, (label, config)) in [
        ("无配置键", None),
        ("空命令", Some("")),
        ("全空白命令", Some("   ")),
    ]
    .into_iter()
    .enumerate()
    {
        let ws = TempWs::new(&format!("no-spawn-{index}"));
        ws.write_config(config);
        let outcome = check_test(&ws.root_str(), "demo-change")
            .await
            .unwrap_or_else(|error| panic!("{label}: 应直过: {error}"));

        assert!(outcome.passed, "{label}: 直过");
        assert!(outcome.output_ref.is_none(), "{label}: 零 begin ⇒ 无引用");
        assert!(
            !ws.exec_dir("demo-change").exists(),
            "{label}: exec-outputs 零新增"
        );
    }
}

/// 异常：取消臂仍 `commit` 已收字节——`output_ref` 在位、`.log` 可读、`fault`
/// 文案与 `passed` 口径零变。
#[tokio::test]
async fn 取消臂_仍commit且引用在位() {
    let _env = env_window();
    let ws = TempWs::new("cancel-commit");
    let _path = PathPrefix::enter(ws.path());
    let pid_file = ws.path().join("spin-pid-commit.txt");
    let command = write_spin_shim(&ws, "spin-cancel-commit", &pid_file);
    ws.write_config(Some(&command));

    let cancel = CancelFlag::never();
    let handle = cancel.handle();
    let root = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessStaticCheck::new()
            .run(&root, "demo-change", ReportSlot::Test, cancel, None)
            .await
    });
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;
    handle.set();
    let output = runner
        .await
        .expect("任务应结束")
        .expect("取消以 fault 收口（不 Err）");
    let ToolStepOutput::StaticCheck(outcome) = output else {
        panic!("产出应为 StaticCheck 变体");
    };

    assert!(
        matches!(outcome.fault, Some(orchestration::port::RunFault::Stopped)),
        "取消 → fault = Stopped，实际: {:?}",
        outcome.fault
    );
    assert!(!outcome.passed, "取消臂 passed 恒 false");
    let exec_id = outcome
        .output_ref
        .clone()
        .expect("取消臂仍 commit（引用在位）");
    let bytes = read_output(&ws, "demo-change", &exec_id);
    // 被杀前可能零输出：只锁「可读且不 panic」
    let _ = String::from_utf8_lossy(&bytes);
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "取消强杀后目标 pid 应消失"
    );
}

/// 异常：超时臂同样 `commit` 已收字节——虚拟时钟推进触发，`fault` 为环境错误、
/// `.log` 可读（可为 0 字节）。
#[tokio::test(start_paused = true)]
async fn 超时臂_仍commit且fault为环境错误() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempWs::new("timeout-commit");
    let _path = PathPrefix::enter(ws.path());
    let pid_file = ws.path().join("spin-pid-timeout.txt");
    let command = write_spin_shim(&ws, "spin-timeout-commit", &pid_file);
    ws.write_config(Some(&command));

    let root = ws.root_str();
    let runner = tokio::spawn(async move { check_test(&root, "demo-change").await });
    // 等 pid 落盘（真实时间让位），再推进虚拟时钟越过预算
    let deadline = Instant::now() + Duration::from_secs(60);
    while !pid_file.exists() && Instant::now() < deadline {
        let _ = tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(10))).await;
    }
    assert!(pid_file.exists(), "自旋 shim 应落 pid");
    let pid = read_pid(&pid_file).expect("pid 可读");
    advance_until(
        || runner.is_finished(),
        Duration::from_secs(60),
        Duration::from_secs(180),
    )
    .await;
    let outcome = runner.await.expect("任务应结束");

    let outcome = outcome.expect("超时以 fault 收口（不 Err）");
    assert!(
        matches!(
            outcome.fault,
            Some(orchestration::port::RunFault::Environment(_))
        ),
        "超时 → fault = Environment，实际: {:?}",
        outcome.fault
    );
    let exec_id = outcome.output_ref.clone().expect("超时臂仍 commit");
    let bytes = read_output(&ws, "demo-change", &exec_id);
    let _ = String::from_utf8_lossy(&bytes);
    let _ = wait_pid_gone(pid, Duration::from_secs(15));
}

/// 异常：`on_output_ref: None` → 零行为差异：`output_ref` 仍按 `commit` 回填、
/// `.log` / `.cmd` 形态在位（与上抛臂同构，见 walker 侧组合行）。
#[tokio::test]
async fn 无回调臂_记录形态仍完整() {
    let _env = env_window();
    let ws = TempWs::new("no-callback");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "noref-shim", &[("noref-line", false)], None);
    ws.write_config(Some(&command));

    let outcome = check(&ws.root_str(), "demo-change", ReportSlot::Test)
        .await
        .expect("应 Ok");
    let exec_id = outcome.output_ref.clone().expect("仍回填引用");

    let dir = ws.exec_dir("demo-change");
    assert!(
        dir.join(format!("{exec_id}.log")).is_file(),
        ".log 形态一致"
    );
    assert!(
        dir.join(format!("{exec_id}.cmd")).is_file(),
        ".cmd 形态一致（回调缺席零影响）"
    );
    assert!(
        String::from_utf8_lossy(&read_output(&ws, "demo-change", &exec_id)).contains("noref-line"),
        "内容口径一致"
    );
}

/// 异常（两零次臂）：命令不可达 `Err` 臂与 `archi` 槽位拒收臂 → 零 spawn ⇒
/// 零记录（不含孤儿空记录 / 空目录）。
#[tokio::test]
async fn 零spawn两臂_零记录目录() {
    let _env = env_window();
    // 零 spawn 臂一：命令不可达（程序解析前置检查失败）
    let missing_ws = TempWs::new("no-record-missing");
    missing_ws.write_config(Some("no-such-static-check-cmd-for-ref-xyz"));
    assert!(
        check_test(&missing_ws.root_str(), "demo-change")
            .await
            .is_err(),
        "不可达命令应 Err"
    );
    assert!(
        !missing_ws.exec_dir("demo-change").exists(),
        "零 spawn ⇒ 零记录（不含孤儿空记录）"
    );

    // 零 spawn 臂二：archi 槽位拒收（守卫先于 begin）
    let archi_ws = TempWs::new("no-record-archi");
    let ok_command = write_shim(&archi_ws, "archi-shim", &[("x", false)], None);
    archi_ws.write_config(Some(&ok_command));
    assert!(
        check(&archi_ws.root_str(), "demo-change", ReportSlot::Archi)
            .await
            .is_err(),
        "archi 槽位应 Err"
    );
    assert!(
        !archi_ws
            .exec_dir_slot("demo-change", ReportSlot::Archi)
            .exists(),
        "拒收臂零记录目录"
    );
}

/// 边界：输出为空（零 echo 命令）→ `.log` 0 字节且 `output_ref` 在位；
/// `read_exec_output_delta_in(dir, id, 0)` → `Some { text: "", next_offset: 0 }`
///（空文件 ≠ miss）。
#[tokio::test]
async fn 空输出_零字节log与增量读可辨() {
    let _env = env_window();
    let ws = TempWs::new("empty-output");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "silent-shim", &[], None);
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("应 Ok");
    let exec_id = outcome.output_ref.clone().expect("引用在位");
    assert_eq!(
        read_output(&ws, "demo-change", &exec_id).len(),
        0,
        ".log 0 字节"
    );
    assert!(outcome.passed, "零输出零退出仍 passed");

    let dir = change_dir(&ws, "demo-change");
    let delta = exec::read_exec_output_delta_in(&dir, &exec_id, 0).expect("空文件 ≠ miss");
    assert_eq!(delta.text, "");
    assert_eq!(delta.next_offset, 0);
    assert!(
        exec::read_exec_output_delta_in(&dir, "exec-404-0", 0).is_none(),
        "miss 与空文件可分辨"
    );
}

/// 边界（参数化 超长命令 / 非 UTF-8 输出）：超长命令串的 `.cmd` 原样不截断；
/// 非 UTF-8 输出的 `.log` 逐字节保真，`diagnostics` 口径零变（lossy 串）。
#[tokio::test]
async fn 超长命令与非utf8输出逐字节保真() {
    let _env = env_window();
    let ws = TempWs::new("long-cmd");
    let _path = PathPrefix::enter(ws.path());
    let shim = write_shim(&ws, "raw-shim", &[("raw-line", false)], None);
    // 命令串在可达程序之后追加超长无害参数（echo 忽略多余参数）
    let padding = "p".repeat(1100);
    let command = format!("{shim} {padding}");
    ws.write_config(Some(&command));

    let outcome = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("应 Ok");
    let exec_id = outcome.output_ref.clone().expect("引用在位");
    let recorded = exec::read_exec_command_in(&change_dir(&ws, "demo-change"), &exec_id)
        .expect("sidecar 命中");
    assert_eq!(recorded, format!("{command} 2>&1"), "超长命令原样不截断");
    assert!(recorded.chars().count() > 1000, "确实超过 1000 字符");

    // 非 UTF-8 输出字节（shim 以 printf 形态落原始字节）
    let raw_ws = TempWs::new("raw-bytes");
    let _raw_path = PathPrefix::enter(raw_ws.path());
    let raw_command = write_raw_shim(&raw_ws, "raw-bytes-shim");
    raw_ws.write_config(Some(&raw_command));
    let raw_outcome = check_test(&raw_ws.root_str(), "demo-change")
        .await
        .expect("应 Ok");
    let raw_id = raw_outcome.output_ref.clone().expect("引用在位");
    let raw_bytes = read_output(&raw_ws, "demo-change", &raw_id);
    assert!(
        raw_bytes.windows(2).any(|window| window == b"\xff\xfe"),
        "非 UTF-8 字节逐字节保真: {raw_bytes:?}"
    );
    assert!(
        raw_outcome.diagnostics.contains('\u{FFFD}') || raw_outcome.diagnostics.contains("raw"),
        "诊断面为 lossy 串（口径零变）: {:?}",
        raw_outcome.diagnostics
    );
}

/// 边界：两次 `run` → 两 `exec_id` 互异、各自 `.log` 与 `.cmd` 独立互不覆写。
#[tokio::test]
async fn 两次run_各自log与cmd独立互不覆写() {
    let _env = env_window();
    let ws = TempWs::new("repeat-two");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "repeat-shim", &[("repeat-line", false)], None);
    ws.write_config(Some(&command));

    let first = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("首次");
    let second = check_test(&ws.root_str(), "demo-change")
        .await
        .expect("二次");
    let first_id = first.output_ref.clone().expect("首次引用");
    let second_id = second.output_ref.clone().expect("二次引用");
    assert_ne!(first_id, second_id, "两次铸造互异");

    let dir = ws.exec_dir("demo-change");
    for exec_id in [&first_id, &second_id] {
        assert!(dir.join(format!("{exec_id}.log")).is_file(), "各自 .log");
        assert!(dir.join(format!("{exec_id}.cmd")).is_file(), "各自 .cmd");
    }
    assert_eq!(
        exec::read_exec_command_in(&change_dir(&ws, "demo-change"), &first_id),
        Some(format!("{command} 2>&1")),
        "首次 .cmd 未被覆写"
    );
}

/// 边界：`slot = Baseline` → `.log` / `.cmd` 均落 `reports/baseline/exec-outputs/`，
/// `reports/test/` 零混居。
#[tokio::test]
async fn baseline槽位_log与cmd均落baseline树() {
    let _env = env_window();
    let ws = TempWs::new("baseline-sidecar");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "baseline-shim", &[("baseline-line", false)], None);
    ws.write_config(Some(&command));

    let outcome = check(&ws.root_str(), "demo-change", ReportSlot::Baseline)
        .await
        .expect("baseline 应 Ok");
    let exec_id = outcome.output_ref.clone().expect("引用在位");

    let baseline_dir = ws.exec_dir_slot("demo-change", ReportSlot::Baseline);
    assert!(
        baseline_dir.join(format!("{exec_id}.log")).is_file(),
        "baseline .log"
    );
    assert!(
        baseline_dir.join(format!("{exec_id}.cmd")).is_file(),
        "baseline .cmd"
    );
    assert!(!ws.exec_dir("demo-change").exists(), "test 树零混居");
}

// ---------------------------------------------------------------------------
// 上抛面（`OutputRefSink`，change-detail-live-stdout，AC-11 / AC-2）
//
// 被测面：`ProcessStaticCheck::run(.., Some(on_output_ref))` 的上抛时机与次数——
// 回调触发时刻 live `.log` 与父目录**已在**、命令 sidecar 已写、spawn **未**发生；
// 上抛恰 1 次且终态 `output_ref` 与之同值；两零 spawn 臂（命令不可达 / archi 槽位
// 拒收）与记录面降级臂回调**零**调用。
//
// Mock 策略（见 test-design 本节 Mock策略表）：`OutputRefSink` 用真实
// `OutputRefSink::new(..)`（design D16 的 `pub` 构造面——checks crate 跨 crate
// 可见），回调体做三件事：计数/记录 `exec_id`、回调内断言 `.log` 与父目录已在、
// 记录回调时刻的 `.log` 长度与 `.cmd` 在场性（顺序不变量）。零替身：真实 tempdir +
// 真实 config + 真实平台 shim。
// ---------------------------------------------------------------------------

use std::sync::{Arc, Mutex};

use orchestration::port::OutputRefSink;

/// 上抛回调内的单次探测快照（顺序不变量的观测面）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct RefProbe {
    /// 回调收到的 `exec_id`（原样）
    exec_id: String,
    /// 回调时刻 `exec_output_log_path_in(..)` 命中且 `is_file()`
    log_present: bool,
    /// 回调时刻 `.log` 的父目录已存在（watch 订阅可生效的前提）
    parent_present: bool,
    /// 回调时刻 `.log` 字节数（`0` ⇒ spawn 尚未发生）
    log_len: u64,
    /// 回调时刻命令 sidecar 已在（`begin → write_command → 上抛` 的顺序判据）
    cmd_present: bool,
}

/// 上抛录制器（计数 + 逐次探测快照）。
#[derive(Clone, Default)]
struct RefTrace {
    probes: Arc<Mutex<Vec<RefProbe>>>,
}

impl RefTrace {
    fn probes(&self) -> Vec<RefProbe> {
        self.probes.lock().expect("上抛记录锁不可中毒").clone()
    }
}

/// 真实 `OutputRefSink::new(..)`（checks 侧唯一构造面，design D16）：回调体在
/// **回调内部**完成全部探测（不是事后探测——顺序不变量的判据必须同刻取得）。
fn ref_sink(change_dir: PathBuf) -> (OutputRefSink, RefTrace) {
    let trace = RefTrace::default();
    let probes = Arc::clone(&trace.probes);
    let sink = OutputRefSink::new(move |exec_id: &str| {
        let log_path = exec::exec_output_log_path_in(&change_dir, exec_id);
        let log_present = log_path.as_deref().is_some_and(Path::is_file);
        let parent_present = log_path
            .as_deref()
            .and_then(Path::parent)
            .is_some_and(Path::is_dir);
        let log_len = log_path
            .as_deref()
            .and_then(|path| fs::metadata(path).ok())
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        // `.cmd` 与 `.log` 同目录同 id 前缀（派生口径见 exec 的 sidecar 单点）
        let cmd_present = log_path
            .as_deref()
            .and_then(Path::parent)
            .map(|dir| dir.join(format!("{exec_id}.cmd")).is_file())
            .unwrap_or(false);
        probes.lock().expect("上抛记录锁不可中毒").push(RefProbe {
            exec_id: exec_id.to_owned(),
            log_present,
            parent_present,
            log_len,
            cmd_present,
        });
    });
    (sink, trace)
}

/// 带上抛回调的执行（`Some(sink)` 交给 `ProcessStaticCheck::run` 的末位形参）。
async fn check_with_ref(
    root: &str,
    name: &str,
    slot: ReportSlot,
    sink: OutputRefSink,
) -> Result<StaticCheckOutcome, String> {
    let output = ProcessStaticCheck::new()
        .run(root, name, slot, CancelFlag::never(), Some(sink))
        .await?;
    match output {
        ToolStepOutput::StaticCheck(outcome) => Ok(outcome),
        other => panic!("产出应为 StaticCheck 变体，实际: {other:?}"),
    }
}

/// 正向：上抛**恰 1 次**，且回调内部同刻可证「`.log` 与父目录已在、命令 sidecar
/// 已写、`.log` 仍为 0 字节（spawn 尚未发生）」（AC-11 顺序不变量）；终态
/// `output_ref` 与回调收到值同值（AC-2 同 id 收口）。
#[tokio::test]
async fn 上抛回调_恰一次且回调时刻log与父目录已在_spawn未发生() {
    let _env = env_window();
    let ws = TempWs::new("ref-order");
    let _path = PathPrefix::enter(ws.path());
    let command = write_shim(&ws, "ref-order-shim", &[("ref-line", false)], None);
    ws.write_config(Some(&command));
    let (sink, trace) = ref_sink(change_dir(&ws, "demo-change"));

    let outcome = check_with_ref(&ws.root_str(), "demo-change", ReportSlot::Test, sink)
        .await
        .expect("应 Ok");

    let probes = trace.probes();
    assert_eq!(probes.len(), 1, "上抛恰 1 次（非 0 非多次）: {probes:?}");
    let probe = &probes[0];
    assert!(probe.log_present, "回调时刻 `.log` 已在（AC-11）");
    assert!(probe.parent_present, "回调时刻父目录已在（watch 订阅前提）");
    assert!(
        probe.cmd_present,
        "回调时刻命令 sidecar 已在（begin → 写命令 → 上抛）"
    );
    assert_eq!(
        probe.log_len, 0,
        "回调时刻 .log 为 0 字节（spawn 尚未发生）"
    );

    let exec_id = outcome.output_ref.clone().expect("终态回填引用");
    assert_eq!(
        probe.exec_id, exec_id,
        "回调收到值与终态 output_ref 同 id（AC-2）"
    );
    assert!(outcome.passed, "判定口径零变");
    assert!(
        String::from_utf8_lossy(&read_output(&ws, "demo-change", &exec_id)).contains("ref-line"),
        "收口后内容为权威输出"
    );
}

/// 异常（组合：1 次 × 两零次臂）：命令不可达 `Err` 臂与 `archi` 槽位拒收臂
/// → **零** spawn ⇒ 回调**零**调用（无 `exec_id` 可上抛，不得虚构一拍）。
#[tokio::test]
async fn 零spawn两臂_上抛回调零调用() {
    let _env = env_window();

    // 零 spawn 臂一：命令不可达（程序解析前置检查失败，其位点在 begin 之前）
    let missing_ws = TempWs::new("ref-no-spawn-missing");
    missing_ws.write_config(Some("no-such-static-check-cmd-for-ref-xyz"));
    let (sink, trace) = ref_sink(change_dir(&missing_ws, "demo-change"));
    assert!(
        check_with_ref(
            &missing_ws.root_str(),
            "demo-change",
            ReportSlot::Test,
            sink
        )
        .await
        .is_err(),
        "不可达命令应 Err"
    );
    assert!(
        trace.probes().is_empty(),
        "命令不可达臂：回调零调用: {:?}",
        trace.probes()
    );

    // 零 spawn 臂二：archi 槽位拒收（守卫先于 begin）
    let archi_ws = TempWs::new("ref-no-spawn-archi");
    let ok_command = write_shim(&archi_ws, "ref-archi-shim", &[("x", false)], None);
    archi_ws.write_config(Some(&ok_command));
    let (sink, trace) = ref_sink(change_dir(&archi_ws, "demo-change"));
    assert!(
        check_with_ref(&archi_ws.root_str(), "demo-change", ReportSlot::Archi, sink)
            .await
            .is_err(),
        "archi 槽位应 Err"
    );
    assert!(
        trace.probes().is_empty(),
        "槽位拒收臂：回调零调用: {:?}",
        trace.probes()
    );
}

/// 异常：记录面降级（`LiveExec::begin` 失败——报告树路径被文件占位）→
/// `output_ref == None` 且回调**零**调用（无 id 可上抛），检查主流程照常。
#[tokio::test]
async fn 记录面降级_上抛回调零调用且主流程照常() {
    let _env = env_window();
    let ws = TempWs::new("ref-degrade");
    #[cfg(windows)]
    let command = r#"cmd /C echo degrade-ref-line"#;
    #[cfg(not(windows))]
    let command = "echo degrade-ref-line";
    ws.write_config(Some(command));
    // 报告树路径被文件占位 → exec-outputs 目录不可建
    let reports = foundation::layout::change_test_reports(ws.path(), "demo-change");
    fs::create_dir_all(reports.parent().expect("父目录")).expect("创建父目录失败");
    fs::write(&reports, "占位文件").expect("占位写失败");
    let (sink, trace) = ref_sink(change_dir(&ws, "demo-change"));

    let outcome = check_with_ref(&ws.root_str(), "demo-change", ReportSlot::Test, sink)
        .await
        .expect("记录失败不阻断检查主流程");

    assert_eq!(outcome.output_ref, None, "降级 None（不虚构引用）");
    assert!(outcome.passed, "判定照常返回");
    assert!(
        outcome.diagnostics.contains("degrade-ref-line"),
        "诊断面不受记录失败影响: {:?}",
        outcome.diagnostics
    );
    assert!(
        trace.probes().is_empty(),
        "无 id 可上抛 ⇒ 回调零调用: {:?}",
        trace.probes()
    );
}
