//! 集成测试：foundation::layout::resolve → workflow::queries 全链路（AC-2 / AC-5 / AC-6）。
//!
//! 验证 resolve 推导的目录名与磁盘真实布局一致：在真实形状的 workspace 目录树上
//! 跑通 resolve → list_changes / change_detail，并覆盖路径输入异常的组合行为。

use std::fs;
use std::path::PathBuf;

use foundation::layout::resolve;
use workflow::model::Inventory;
use workflow::queries::{change_detail, list_changes};

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-it-layout-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        Self(dir)
    }

    fn change(&self, rel_dir: &str, files: &[(&str, &str)]) {
        let dir = self.0.join(rel_dir);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        for (name, content) in files {
            fs::write(dir.join(name), content).expect("写文件失败");
        }
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const V1_WORKFLOW: &str =
    r#"{ "workflow_type": "requirement", "created": "2026-04-04", "eval": [
         { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "OK", "checklist": [] } ] }"#;

/// 无 created 字段的 v1 形状（验证 archive 目录名日期回退）。
const V1_WORKFLOW_NO_CREATED: &str =
    r#"{ "workflow_type": "requirement", "eval": [
         { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "OK", "checklist": [] } ] }"#;

#[test]
fn resolve到list_changes全链路_归属与月分组和磁盘一致() {
    let ws = TempWs::new("full-chain");
    // 按"用户眼中"的真实 workspace 形状搭目录（不再手工构造 Layout）
    ws.change(
        "openspec/changes/active-one",
        &[("workflow.json", V1_WORKFLOW), ("proposal.md", "# 提案")],
    );
    ws.change("openspec/changes/archive/2026-03-03-archived-v0", &[("proposal.md", "# v0")]);
    ws.change("openspec/changes/archive/2026-08-08-archived-v1", &[("workflow.json", V1_WORKFLOW)]);
    ws.change("openspec/explores", &[("note.md", "# 探索")]);

    // 契约缝合点：resolve 推导的目录名必须与上面搭出的真实布局一致，
    // 否则查询会静默返回空
    let layout = resolve(&ws.0);
    assert!(layout.changes_root.is_dir(), "changes_root 必须指向真实目录");
    assert!(layout.archive_root.is_dir(), "archive_root 必须指向真实目录");
    assert!(layout.explores_root.is_dir(), "explores_root 必须指向真实目录");

    let list = list_changes(&layout);
    assert_eq!(list.active.len(), 1);
    assert_eq!(list.active[0].name, "active-one");
    assert_eq!(list.active[0].inventory, Inventory::V1);

    let months: Vec<Option<&str>> = list
        .archive_groups
        .iter()
        .map(|group| group.month.as_deref())
        .collect();
    assert_eq!(months, vec![Some("2026-08"), Some("2026-03")], "新月份在前");
    assert_eq!(list.archive_groups.len(), 2);
}

#[test]
fn change_detail命中active与archive各一_archive经archive_root定位() {
    let ws = TempWs::new("detail-both");
    ws.change(
        "openspec/changes/live-change",
        &[("workflow.json", V1_WORKFLOW), ("proposal.md", "# 进行中")],
    );
    ws.change(
        "openspec/changes/archive/2026-06-06-gone-change",
        &[("workflow.json", V1_WORKFLOW_NO_CREATED), ("proposal.md", "# 已归档")],
    );

    let layout = resolve(&ws.0);

    let live = change_detail(&layout, "live-change").expect("active change 应可定位");
    assert_eq!(live.source, workflow::queries::ChangeSource::Active);
    assert_eq!(live.inventory, Inventory::V1);
    assert_eq!(live.created.as_deref(), Some("2026-04-04"), "active 取 workflow.json created");

    let archived = change_detail(&layout, "2026-06-06-gone-change").expect("archive change 应可定位");
    assert_eq!(archived.source, workflow::queries::ChangeSource::Archive);
    assert_eq!(
        archived.created.as_deref(),
        Some("2026-06-06"),
        "workflow.json 无 created → 回退目录名日期前缀"
    );
    assert_eq!(archived.pipeline.len(), 9, "v1 详情为 9 站流水线");
}

#[test]
fn workspace缺explores子目录时resolve正常查询不受影响() {
    let ws = TempWs::new("no-explores");
    ws.change("openspec/changes/only-change", &[("proposal.md", "# 提案")]);
    // 不创建 openspec/explores

    let layout = resolve(&ws.0);
    assert!(!layout.explores_root.exists(), "前置：explores 子目录确实缺失");
    assert!(layout.changes_root.is_dir());

    let list = list_changes(&layout);
    assert_eq!(list.active.len(), 1);
    assert!(change_detail(&layout, "only-change").is_some());
}

#[test]
fn root指向文件时list_changes返回空不panic() {
    let temp = TempWs::new("root-is-file");
    let file_root = temp.0.join("普通文件.txt");
    fs::write(&file_root, "内容").expect("写文件失败");

    let layout = resolve(&file_root);
    let list = list_changes(&layout);
    assert!(list.active.is_empty());
    assert!(list.archive_groups.is_empty());
}

#[test]
fn change名含路径分隔符或点点时返回none不逃逸() {
    let ws = TempWs::new("escape");
    ws.change(
        "openspec/changes/inside",
        &[("workflow.json", V1_WORKFLOW), ("secret.md", "# 不应被越权读取")],
    );
    // 越权目标：changes 树外的目录树
    ws.change("outside/secret-change", &[("workflow.json", V1_WORKFLOW)]);

    let layout = resolve(&ws.0);

    for hostile in ["../outside/secret-change", "..", "inside/secret.md", "a/b", "C:\\evil"] {
        assert!(
            change_detail(&layout, hostile).is_none(),
            "敌意 change 名 {hostile:?} 必须被拒绝"
        );
    }
    // 正名不受影响
    assert!(change_detail(&layout, "inside").is_some());
}

#[test]
fn changes子树整体缺失时list_changes空结果() {
    let ws = TempWs::new("no-changes");
    // 只建 openspec/，不建 openspec/changes/
    fs::create_dir_all(ws.0.join("openspec/explores")).expect("创建 openspec 失败");

    let list = list_changes(&resolve(&ws.0));
    assert!(list.active.is_empty());
    assert!(list.archive_groups.is_empty());
}
