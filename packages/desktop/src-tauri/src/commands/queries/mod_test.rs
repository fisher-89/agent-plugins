//! `commands::queries` 的单元测试：三命令为无状态薄包装（参数 → resolve → core → DTO）（AC-11）。
//!
//! `#[tauri::command]` 保留原函数可直调，测试不启动 Tauri runtime——
//! 可脱离 State 直接调用本身即"无状态薄包装"的结构性证明。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{get_change_detail, list_changes, read_artifact};

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "desktop-app-queries-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// 搭一个最小 workspace：一个 active v2 change + 一个 archive v0 change。
    fn with_sample_tree(&self) {
        let v2_dir = self.0.join("openspec/changes/sample-v2");
        fs::create_dir_all(&v2_dir).expect("创建 change 目录失败");
        fs::write(
            v2_dir.join("workflow.json"),
            r#"{ "workflow_type": "requirement", "file_log": [], "eval": [
                     { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "OK",
                       "checklist": [ { "item": "问题清晰", "pass": true, "evidence": "L1-10" } ] } ] }"#,
        )
        .expect("写 workflow.json 失败");
        fs::write(v2_dir.join("proposal.md"), "# 提案\n\n- [x] 完成\n- [ ] 待办\n").expect("写 proposal 失败");

        let v0_dir = self.0.join("openspec/changes/archive/2026-02-02-old-docs");
        fs::create_dir_all(&v0_dir).expect("创建 archive 目录失败");
        fs::write(v0_dir.join("proposal.md"), "# 旧提案").expect("写 proposal 失败");
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn list_changes命令与core查询结果一致_纯透传无加工() {
    use foundation::layout::resolve;
    use workflow::queries as core_queries;

    let ws = TempWs::new("passthrough");
    ws.with_sample_tree();
    let root = ws.0.to_string_lossy().into_owned();

    let via_command = list_changes(root.clone());
    let via_core = core_queries::list_changes(&resolve(Path::new(&root)));

    // 经 serde 序列化对比：命令层不加工 DTO
    let a = serde_json::to_value(&via_command).expect("命令结果序列化失败");
    let b = serde_json::to_value(&via_core).expect("core 结果序列化失败");
    assert_eq!(a, b);

    // 内容抽查：全量（active + archive 月分组）
    assert_eq!(a["active"].as_array().map(Vec::len), Some(1));
    assert_eq!(a["archiveGroups"].as_array().map(Vec::len), Some(1));
    assert_eq!(a["archiveGroups"][0]["month"], json!("2026-02"));
}

#[test]
fn get_change_detail已知change返回完整详情dto() {
    let ws = TempWs::new("detail-known");
    ws.with_sample_tree();
    let root = ws.0.to_string_lossy().into_owned();

    let detail = get_change_detail(root, "sample-v2".to_string())
        .expect("已知 change 应返回 Some");
    let value = serde_json::to_value(&detail).expect("详情序列化失败");
    assert_eq!(value["name"], json!("sample-v2"));
    assert_eq!(value["inventory"], json!("v2"));
    assert_eq!(value["pipeline"].as_array().map(Vec::len), Some(9));
    assert!(value["fileLog"].is_array(), "v2 的 fileLog 区块为数组");
    assert!(
        value["artifacts"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "产物清单非空"
    );
}

#[test]
fn get_change_detail未知change返回none而非错误() {
    let ws = TempWs::new("detail-unknown");
    ws.with_sample_tree();

    assert!(
        get_change_detail(ws.0.to_string_lossy().into_owned(), "不存在".to_string()).is_none(),
        "未知 change 名返回 None（非错误）"
    );
}

#[test]
fn read_artifact未注册kind或不存在source返回none() {
    let ws = TempWs::new("read-artifact");
    ws.with_sample_tree();
    let root = ws.0.to_string_lossy().into_owned();

    // 未注册 kind
    assert!(
        read_artifact(root.clone(), "sample-v2".into(), "file-log".into(), "proposal.md".into())
            .is_none(),
        "未注册 kind → None"
    );
    // 不存在 source
    assert!(
        read_artifact(root, "sample-v2".into(), "markdown-doc".into(), "没有这个.md".into())
            .is_none(),
        "不存在 source → None"
    );
}

#[test]
fn read_artifact正向路径返回五字段信封() {
    let ws = TempWs::new("read-artifact-ok");
    let change_dir = ws.0.join("openspec/changes/with-tasks");
    fs::create_dir_all(&change_dir).expect("创建 change 失败");
    fs::write(change_dir.join("tasks.md"), "- [x] 已完成\n- [ ] 待办\n").expect("写 tasks.md 失败");

    let envelope = read_artifact(
        ws.0.to_string_lossy().into_owned(),
        "with-tasks".into(),
        "tasks-progress".into(),
        "tasks.md".into(),
    )
    .expect("有效读取应返回 Some");

    assert_eq!(envelope.kind, "tasks-progress");
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.payload["total"], 2);
    assert_eq!(envelope.payload["done"], 1);
    assert!(envelope.fallback_text.is_some());
}

#[test]
fn root为空字符串时经resolve空路径得空列表不panic() {
    let list = list_changes(String::new());
    // 空白 root 走显式格式检查：直接空结果语义（不报错、不 panic）
    assert!(list.active.is_empty());
    assert!(list.archive_groups.is_empty());

    assert!(get_change_detail(String::new(), "任意".into()).is_none());
}

#[test]
fn read_artifact敌意source在命令边界被拒绝不逃逸change目录() {
    let ws = TempWs::new("hostile-source");
    ws.with_sample_tree();
    // 越权目标：change 目录树外的秘密文件（workspace 根）
    fs::write(ws.0.join("secret.md"), "# 不应被越权读取").expect("写根 secret 失败");
    let root = ws.0.to_string_lossy().into_owned();

    for hostile in [
        "../secret.md",
        "sample-v2/../../secret.md",
        "..\\..\\secret.md",
        "C:\\evil\\secret.md",
        "/secret.md",
        "..",
        "",
    ] {
        assert!(
            read_artifact(root.clone(), "sample-v2".into(), "markdown-doc".into(), hostile.to_string())
                .is_none(),
            "命令边界必须拒绝敌意 source {hostile:?}"
        );
    }

    // 正向对照：合法相对路径正常回放信封
    assert!(
        read_artifact(root, "sample-v2".into(), "markdown-doc".into(), "proposal.md".into()).is_some(),
        "合法 source 不应被误伤"
    );
}
