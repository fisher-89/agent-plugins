//! `commands::changes` 的单元测试（test-design「commands/changes/mod.rs ->
//! mod_test.rs」节）：本组四命令——三读（`list_changes` / `get_change_detail` /
//! `read_artifact`）为无状态薄包装（参数 → resolve → core → DTO）；
//! `create_change` 为三件事薄包装（参数转换 → 写面 create → 错误映射），
//! 返回 DTO 仅 `name` 与 `created` 两字段；跨模块组合用例（链路最上层调用
//! 方 = 命令层）：命令 → `layout::resolve` → core / 写面 → 磁盘产物。
//!
//! blank root 双口径并存不互换（AC-3）：读命令空/空白 root 早退空结果语义
//! （空列表 / `None`）；`create_change` 显式 `Err`（不进入写面链路）。
//!
//! `#[tauri::command]` 保留原函数可直调；全组 sync 纯函数命令（无 State /
//! AppHandle / Channel），不启动 Tauri runtime，直调即测——可脱离 State
//! 直接调用本身即"无状态薄包装"的结构性证明。
//!
//! Mock策略：无进程边界 mock（fs 真实组合）——tempdir workspace root 真盘，
//! 薄包装等值以同根直调 core / 写面比对。

use std::fs;
use std::path::{Path, PathBuf};

use foundation::layout::resolve;
use serde_json::json;
use workflow::write;

use super::{create_change, get_change_detail, list_changes, read_artifact};

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "desktop-changes-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn root_string(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    fn layout(&self) -> foundation::layout::Layout {
        resolve(&self.0)
    }

    fn change_dir(&self, name: &str) -> PathBuf {
        self.layout().changes_root.join(name)
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
        fs::write(
            v2_dir.join("proposal.md"),
            "# 提案\n\n- [x] 完成\n- [ ] 待办\n",
        )
        .expect("写 proposal 失败");

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

    let detail = get_change_detail(root, "sample-v2".to_string()).expect("已知 change 应返回 Some");
    let value = serde_json::to_value(&detail).expect("详情序列化失败");
    assert_eq!(value["name"], json!("sample-v2"));
    assert_eq!(value["inventory"], json!("v2"));
    assert_eq!(value["pipeline"].as_array().map(Vec::len), Some(9));
    assert!(value["fileLog"].is_array(), "v2 的 fileLog 区块为数组");
    assert!(
        value["artifacts"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
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
        read_artifact(
            root.clone(),
            "sample-v2".into(),
            "file-log".into(),
            "proposal.md".into()
        )
        .is_none(),
        "未注册 kind → None"
    );
    // 不存在 source
    assert!(
        read_artifact(
            root,
            "sample-v2".into(),
            "markdown-doc".into(),
            "没有这个.md".into()
        )
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
            read_artifact(
                root.clone(),
                "sample-v2".into(),
                "markdown-doc".into(),
                hostile.to_string()
            )
            .is_none(),
            "命令边界必须拒绝敌意 source {hostile:?}"
        );
    }

    // 正向对照：合法相对路径正常回放信封
    assert!(
        read_artifact(
            root,
            "sample-v2".into(),
            "markdown-doc".into(),
            "proposal.md".into()
        )
        .is_some(),
        "合法 source 不应被误伤"
    );
}

// ---------------------------------------------------------------------------
// 正向：命令 → resolve → 写面 → 磁盘产物（薄包装不加工）
// ---------------------------------------------------------------------------

/// `create_change 命令 → layout::resolve → write::create → 磁盘产物`：Ok
/// 与直调写面 serde 等值（薄包装不加工）；workflow.json + explore.md 真盘
/// 落位（AC-4 薄包装半边）。
#[test]
fn create_change命令到磁盘产物_与直调写面serde等值() {
    let via_command = TempWs::new("cmd");
    let via_core = TempWs::new("core");

    let command_outcome = create_change(
        via_command.root_string(),
        "fix-bug".to_owned(),
        "修复登录重试的竞态问题".to_owned(),
    )
    .expect("合法输入经命令层应 Ok");
    let core_outcome = write::create(&via_core.layout(), "fix-bug", "修复登录重试的竞态问题")
        .expect("直调写面应 Ok");

    assert_eq!(
        serde_json::to_value(&command_outcome).expect("命令结果序列化失败"),
        serde_json::to_value(&core_outcome).expect("写面结果序列化失败"),
        "薄包装不加工"
    );

    // 真盘落位
    assert!(
        via_command
            .change_dir("fix-bug")
            .join("workflow.json")
            .is_file(),
        "workflow.json 真盘落位"
    );
    assert!(
        via_command
            .change_dir("fix-bug")
            .join("explore.md")
            .is_file(),
        "explore.md 真盘落位"
    );
}

// ---------------------------------------------------------------------------
// 边界：返回 DTO 仅两字段
// ---------------------------------------------------------------------------

/// 返回 DTO 仅两字段：serde_json 序列化恰 `{"name":…,"created":…}` 两键、
/// 零磁盘路径字段（AC-4 DTO 纪律）。
#[test]
fn 返回dto恰name_created两键零磁盘路径字段() {
    let ws = TempWs::new("dto-shape");

    let outcome = create_change(ws.root_string(), "fix-bug".to_owned(), "goal".to_owned())
        .expect("合法输入应 Ok");

    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("序列化为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["created", "name"], "恰两键且零磁盘路径字段");
}

// ---------------------------------------------------------------------------
// 异常：blank root 命令层拦截 / 写面拒绝透传
// ---------------------------------------------------------------------------

/// blank root 显式 Err 不进写面：root "" / "   " 各 Err（命令层拦截文案，
/// 写面错误面无此分支）且 changes_root 下零新目录（AC-4；沿 explores 组写
/// 命令 blank root 纪律口径）。
#[test]
fn blank_root显式err不进写面链路() {
    let ws = TempWs::new("blank-root");
    for root in ["", "   "] {
        let error = create_change(root.to_owned(), "fix-bug".to_owned(), "goal".to_owned())
            .expect_err("blank root 应 Err");
        assert_eq!(
            error, "非法 root: 不得为空白",
            "命令层拦截（不进入写面链路），实际: {error}"
        );
    }

    assert!(
        !ws.layout().changes_root.exists(),
        "changes_root 下零新目录"
    );
}

/// 写面拒绝透传：非法名 / 空白 goal 经命令层 Err（错误映射不吞不加工，写面
/// 原文透传）且零产生（AC-3 命令入口同口径）。
#[test]
fn 写面拒绝透传_非法名与空白goal经命令层err且零产生() {
    let ws = TempWs::new("passthrough-err");

    let name_error = create_change(ws.root_string(), "Fix_Bug".to_owned(), "goal".to_owned())
        .expect_err("非法名应 Err");
    assert!(
        name_error.contains("kebab-case"),
        "错误映射不吞不加工（写面原文透传），实际: {name_error}"
    );

    let goal_error = create_change(ws.root_string(), "fix-bug".to_owned(), "   ".to_owned())
        .expect_err("空白 goal 应 Err");
    assert!(
        goal_error.contains("goal"),
        "空白 goal 错误透传，实际: {goal_error}"
    );

    assert!(
        !ws.layout().changes_root.exists(),
        "零产生（拒绝面无目录无文件）"
    );
}
