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
    // poison 容错：单例环境锁被某用例 panic 污染后仍可取用（避免一例失败级联
    // 拖垮同文件其余用例）
    EnvWindow {
        _guard: crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
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
        .run(root, name, slot, foundation::cancel::CancelFlag::never())
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
    let names: Vec<String> = fs::read_dir(&dir)
        .expect("读输出记录目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
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

    let mut names: Vec<String> = fs::read_dir(ws.exec_dir("demo-change"))
        .expect("读输出记录目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
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
    let command = write_shim(&ws, "baseline-lint", &[("ok", false)], None);
    ws.write_config(Some(&command));

    let outcome = check(&ws.root_str(), "demo-change", ReportSlot::Baseline)
        .await
        .expect("baseline pass 应 Ok");

    assert!(outcome.passed, "pass 场景 passed=true");
    assert!(
        outcome.report_dir.ends_with(&format!("demo-change{SEP}reports{SEP}baseline")),
        "report_dir = baseline 报告树锚点: {}",
        outcome.report_dir
    );
    let exec_id = outcome.output_ref.as_deref().expect("recorded 引用应回填");
    // .log 落 baseline 树（零混居）
    let baseline_log = ws
        .exec_dir_slot("demo-change", ReportSlot::Baseline)
        .join(format!("{exec_id}.log"));
    assert!(baseline_log.is_file(), "输出记录落 baseline exec-outputs: {baseline_log:?}");
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
        outcome.report_dir.ends_with(&format!("demo-change{SEP}reports{SEP}baseline")),
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

    assert_eq!(test_outcome.passed, baseline_outcome.passed, "passed 语义等值");
    assert_eq!(
        test_outcome.diagnostics, baseline_outcome.diagnostics,
        "diagnostics 语义等值（执行零分叉）"
    );
    assert!(
        test_outcome.output_ref.is_some() && baseline_outcome.output_ref.is_some(),
        "recorded 引用两槽位均回填"
    );
    assert_ne!(test_outcome.report_dir, baseline_outcome.report_dir, "锚点不同");
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
// AC-2 / AC-3 / AC-4）：static-check 首获超时预算（此前零超时 → cargo 卡文件
// 锁时永久悬挂）；取消经 `run_shell` 入口短路与轮询两径收敛为
// `Err("执行已取消")`。
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
        let body = format!("@echo off\r\n\"{}\" \"{}\"\r\n", bash.display(), script.display());
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

/// 正向（AC-3）：超时臂——自旋 shim + 虚拟时钟推进越过 `STATIC_CHECK_TIMEOUT`
/// → `Err` 文案恰为「static-check 命令超时（1200 秒）强制终止」+ 墙钟上界
///（不悬挂——本变更修复的现状缺陷）+ 目标 pid 消失（`kill_tree` 整树回收锚）。
#[tokio::test(start_paused = true)]
async fn 超时臂_到点err且pid消失不悬挂() {
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
            .run(&root_str, "demo-change", ReportSlot::Test, CancelFlag::never())
            .await
    });
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;

    let started = Instant::now();
    tokio::time::advance(STATIC_CHECK_TIMEOUT + Duration::from_secs(60)).await;
    let err = runner
        .await
        .expect("static-check 任务应正常结束")
        .expect_err("超时应 Err（不悬挂）");
    let elapsed = started.elapsed();

    assert_eq!(
        err, "static-check 命令超时（1200 秒）强制终止",
        "超时文案逐字（预算秒数由常量拼装）"
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

/// 正向（AC-4 / D4）：已置位 cancel —— 入口短路不 spawn，直接 `Err("执行已取消")`。
#[tokio::test]
async fn 已置位cancel_入口短路零spawn() {
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
        .run(&ws.root_str(), "demo-change", ReportSlot::Test, cancel)
        .await;
    let err = output.expect_err("取消短路应 Err");
    assert_eq!(err, "执行已取消", "取消文案（过渡语义）");
    assert!(!marker.exists(), "入口短路不 spawn（shim 零执行）");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "短路墙钟近零（不依赖轮询时序）"
    );
    let exec_dir = ws.exec_dir("demo-change");
    assert!(
        !exec_dir.exists() || fs::read_dir(&exec_dir).map(|mut d| d.next().is_none()).unwrap_or(true),
        "取消臂不落输出记录引用"
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
        .run(&ws.root_str(), "demo-change", ReportSlot::Test, cancel)
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
    assert_eq!(outcome.output_ref, None, "零 spawn 即无输出记录（不落空 .log）");
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

/// 正向（AC-4）：运行中 cancel——自旋 shim 落 pid 后挂起，运行中置位 →
/// `Err("执行已取消")` + 墙钟上界（秒级收敛，不等 `STATIC_CHECK_TIMEOUT`）+
/// 目标 pid 消失；取消臂不落 diagnostics / 输出记录引用。
#[tokio::test]
async fn 运行中cancel_秒级收敛且pid消失不落输出引用() {
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
            .run(&root_str, "demo-change", ReportSlot::Test, cancel)
            .await
    });

    // 等 shim 落 pid（进程已建立）后置位取消
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;
    let started = Instant::now();
    handle.set();

    let output = runner.await.expect("run 任务应正常结束");
    let err = output.expect_err("取消应 Err");
    assert_eq!(err, "执行已取消", "取消文案（过渡语义）");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "秒级收敛（不等 1200 s 预算，实际 {:?}）",
        started.elapsed()
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "取消强杀后目标 pid {pid} 应消失（kill_tree 整树回收）"
    );
    let exec_dir = ws.exec_dir("demo-change");
    assert!(
        !exec_dir.exists()
            || fs::read_dir(&exec_dir)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(true),
        "取消臂不落输出记录引用"
    );
}
