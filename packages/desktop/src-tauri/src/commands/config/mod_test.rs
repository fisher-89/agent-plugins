//! `commands::config` 的单元测试（AC-3 / AC-7）：`workspace_config` 无状态
//! 薄包装 + `workspace_config_inner` 领域组装纯函数。
//!
//! `#[tauri::command]` 保留原函数可直调，测试不启动 Tauri runtime（stats
//! mod_test 先例）。文件系统不 mock：真实 tempdir fixture（RAII 清理）承载
//! 有效 / 缺失 / 坏 JSON / 非目录 / 不可读（悬空 junction，cfg(windows)）root
//! 与配置文件改写序列；断言只收敛自研组装层（Err 通道分层、信封平移、无缓存
//! 行为化核对），不逐项断言 fs / serde 库自身语义。

use std::fs;
use std::path::{Path, PathBuf};

use super::{workspace_config, workspace_config_inner};

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "desktop-app-config-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn ensure_dir(&self) {
        fs::create_dir_all(&self.0).expect("创建临时根目录失败");
    }

    /// 相对 root 落一份配置文件（自动建父目录域目录）。
    fn write_config(&self, content: &str) {
        let path = self.0.join("openspec").join("config.json");
        fs::create_dir_all(path.parent().expect("配置路径应有父目录")).expect("建域目录失败");
        fs::write(path, content).expect("写配置文件失败");
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// workspace_config：薄包装结构证明（不启动 runtime 的可直调性）
// ---------------------------------------------------------------------------

#[test]
fn workspace_config命令与inner纯函数结果逐字段一致_命令层零加工() {
    let ws = TempWs::new("wrapper-parity");
    ws.ensure_dir();
    ws.write_config(r#"{"context": "桌面端插件仓库", "tests": []}"#);
    let root = ws.root().to_string_lossy().into_owned();

    let via_command = workspace_config(root.clone()).expect("有效 root 应 Ok");
    let via_inner = workspace_config_inner(Path::new(&root)).expect("有效 root 应 Ok");

    // 经 serde 序列化对比：命令层除参数转换（String → &Path）与 Err 透传外零加工
    let a = serde_json::to_value(&via_command).expect("命令结果序列化失败");
    let b = serde_json::to_value(&via_inner).expect("inner 结果序列化失败");
    assert_eq!(a, b, "命令层与纯函数对同一 tempdir 根结果逐字段一致");
}

#[test]
fn workspace_config以string原签名直调_无state薄包装结构证明() {
    let ws = TempWs::new("wrapper-direct");
    ws.ensure_dir();
    ws.write_config(r#"{"schema": "spec-driven", "tests": []}"#);
    let root = ws.root().to_string_lossy().into_owned();

    // `#[tauri::command]` 保留 (String,) 原函数签名可直调、无 State 入参
    let report = workspace_config(root).expect("有效 root 应返回 Ok");
    assert_eq!(report.config.schema, "spec-driven");
    assert!(report.diagnostics.is_empty());

    // 无效 root 的 Err 经命令层透传且错误串与 inner 一致（错误映射之末环）
    let ghost = "肯定不存在的路径-直调".to_string();
    let via_command = workspace_config(ghost.clone());
    let via_inner = workspace_config_inner(Path::new(&ghost));
    assert_eq!(
        via_command.unwrap_err(),
        via_inner.unwrap_err(),
        "无效 root 的错误串应与 inner 一致（命令层透传）"
    );
}

// ---------------------------------------------------------------------------
// workspace_config_inner：root 有效性检查（Err 通道唯一来源）
// ---------------------------------------------------------------------------

#[test]
fn 不存在root路径返回err不panic() {
    let missing = std::env::temp_dir().join(format!(
        "desktop-app-config-test-{}-不存在",
        std::process::id()
    ));

    let err = workspace_config_inner(&missing).expect_err("缺失 root 应返回 Err");
    assert!(
        err.contains("root 无效"),
        "错误串应出自前置 metadata 检查，实际 {err:?}"
    );
}

#[test]
fn root指向普通文件返回err() {
    let ws = TempWs::new("file-root");
    ws.ensure_dir();
    fs::write(ws.0.join("plain.txt"), "普通文本文件\n").expect("写 fixture 文件失败");

    let err = workspace_config_inner(&ws.0.join("plain.txt")).expect_err("非目录 root 应返回 Err");
    assert!(err.contains("不是目录"), "错误串应指明非目录，实际 {err:?}");
}

#[test]
fn 空白root返回err() {
    // blank root 与无效 root 同走 Err（与 code_stats 裁定同式：blank 只能来自
    // 调用 bug，不静默空报告）
    assert!(
        workspace_config_inner(Path::new("")).is_err(),
        "空串 root 应返回 Err 而非空报告"
    );
}

#[test]
fn 超长不存在路径root返回err不panic() {
    let long = format!(
        "{}\\{}",
        std::env::temp_dir().display(),
        "very-long-segment-".repeat(80)
    );
    assert!(long.chars().count() > 1000, "前置：路径超长");
    assert!(!Path::new(&long).exists(), "前置：路径不存在");

    assert!(
        workspace_config_inner(Path::new(&long)).is_err(),
        "超长不存在路径应返回 Err 而非 panic"
    );
}

#[test]
#[cfg(windows)]
fn 不可读root返回err_windows下以悬空junction使metadata失败() {
    // stats mod_test 先例 fixture：提权环境下目录 ACL 收紧拦不住 fs::metadata，
    // 改以悬空 junction（指向不存在目标）使 metadata 必然失败——被测属性不变：
    // metadata 失败 → Err 通道，不 panic、不静默空报告。
    let ws = TempWs::new("unreadable-root");
    ws.ensure_dir();
    let link = ws.0.join("dangling");
    let mklink = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(ws.0.join("no-such-target"))
        .output()
        .expect("mklink 应可执行（cmd 内置）");
    assert!(mklink.status.success(), "创建悬空 junction 应成功");
    assert!(
        std::fs::metadata(&link).is_err(),
        "前置：悬空 junction 的 metadata 应失败"
    );

    let err = workspace_config_inner(&link).expect_err("不可达 root 应返回 Err");
    assert!(
        err.contains("root 无效"),
        "错误串应出自前置 metadata 检查，实际 {err:?}"
    );
}

// ---------------------------------------------------------------------------
// workspace_config_inner：文件态分层（Err 通道 vs 报告内标记）
// ---------------------------------------------------------------------------

#[test]
fn 有效root无配置文件返回ok加file_missing诊断与全默认config() {
    let ws = TempWs::new("missing-file");
    ws.ensure_dir();

    // 配置文件缺失不是 Err（多数 workspace 常态）：空态标记在报告内
    let report = workspace_config_inner(ws.root()).expect("配置文件缺失应返回 Ok");
    assert_eq!(report.diagnostics.len(), 1);
    assert!(
        matches!(
            report.diagnostics[0].kind,
            config::DiagnosticKind::FileMissing
        ),
        "缺失应以 fileMissing 诊断在案，实际 {:?}",
        report.diagnostics[0].kind
    );
    assert_eq!(report.diagnostics[0].path, "$");
    // 全默认 config（空态数据面）
    assert_eq!(report.config.schema, "spec-driven");
    assert!(report.config.tests.is_empty());
}

#[test]
fn 坏json返回ok加json_invalid诊断与默认config_两通道分层() {
    let ws = TempWs::new("bad-json");
    ws.ensure_dir();
    ws.write_config("{ 这不是合法 JSON ]");

    // 报告内 fatal（JsonInvalid）走 Ok：仅无效 root 走 Err——两通道分层裁定
    let report = workspace_config_inner(ws.root()).expect("报告内 fatal 应返回 Ok");
    assert!(matches!(
        report.diagnostics[0].kind,
        config::DiagnosticKind::JsonInvalid
    ));
    assert_eq!(
        report.config.schema, "spec-driven",
        "fatal 同样落默认值报告"
    );
}

#[test]
fn 合法配置文件返回ok_diagnostics为空且信封字段平移一致() {
    let ws = TempWs::new("legal-config");
    ws.ensure_dir();
    ws.write_config(r#"{"schema": "spec-driven", "tests": [], "context": "合法上下文", "rules": {"tasks": ["规则一"]}}"#);

    let report = workspace_config_inner(ws.root()).expect("合法配置应返回 Ok");
    assert!(
        report.diagnostics.is_empty(),
        "实际诊断 {:?}",
        report.diagnostics
    );
    assert_eq!(report.config.context.as_deref(), Some("合法上下文"));

    // ConfigReport → WorkspaceConfigReport 字段平移一致：与 config crate
    // load 的领域报告逐字段对照（命令层零加工）
    let loaded = config::load(ws.root());
    let inner_value = serde_json::to_value(&report.config).expect("命令线面 config 序列化失败");
    let load_value = serde_json::to_value(&loaded.config).expect("领域 config 序列化失败");
    assert_eq!(inner_value, load_value, "信封 config 平移一致");
}

// ---------------------------------------------------------------------------
// workspace_config_inner：不落库无缓存（行为化核对）
// ---------------------------------------------------------------------------

#[test]
fn 连续两次调用各自完整读取_改写文件后结果随之变化() {
    let ws = TempWs::new("no-cache");
    ws.ensure_dir();
    ws.write_config(r#"{"context": "第一版"}"#);

    let first = workspace_config_inner(ws.root()).expect("首次读取应 Ok");
    assert_eq!(first.config.context.as_deref(), Some("第一版"));

    // 同一进程内改写文件内容后再次调用：结果随文件变化（无缓存行为化核对）
    ws.write_config(r#"{"context": "第二版"}"#);
    let second = workspace_config_inner(ws.root()).expect("二次读取应 Ok");
    assert_eq!(
        second.config.context.as_deref(),
        Some("第二版"),
        "每次调用完整重读重校验（无缓存）"
    );
}
