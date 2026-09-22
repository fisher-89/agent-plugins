//! `queries::list_changes` 的单元测试：全量扫描 + 代际标注 + archive 月分组（AC-5）。

use std::fs;
use std::path::{Path, PathBuf};

use super::list_changes;
use crate::model::Inventory;
use foundation::layout::resolve;

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("workflow-list-test-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// 在 workspace 内创建一个 change 目录并写入文件（相对 root 的目录串 + 文件列表）。
    fn change(&self, rel_dir: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = self.0.join(rel_dir);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        for (name, content) in files {
            fs::write(dir.join(name), content).expect("写文件失败");
        }
        dir
    }

    fn layout(&self) -> foundation::layout::Layout {
        resolve(&self.0)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const V1_WORKFLOW: &str =
    r#"{ "workflow_type": "requirement", "created": "2026-01-01", "eval": [] }"#;
const V2_WORKFLOW: &str = r#"{ "workflow_type": "requirement", "file_log": [] }"#;

#[test]
fn active与archive全量返回_代际标注与磁盘事实一致() {
    let ws = TempWs::new("full-scan");
    ws.change(
        "openspec/changes/active-v2",
        &[("workflow.json", V2_WORKFLOW), ("proposal.md", "# 提案")],
    );
    ws.change("openspec/changes/plain-active", &[("proposal.md", "# v0")]);
    ws.change(
        "openspec/changes/archive/2026-05-01-archived-v1",
        &[("workflow.json", V1_WORKFLOW)],
    );
    ws.change(
        "openspec/changes/archive/2026-09-15-archived-v0",
        &[("proposal.md", "# v0 归档")],
    );

    let list = list_changes(&ws.layout());

    let active_names: Vec<&str> = list.active.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(active_names, vec!["active-v2", "plain-active"]);

    let by_name = |name: &str| {
        list.active
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("active 中应含 {name}"))
    };
    assert_eq!(by_name("active-v2").inventory, Inventory::V2);
    assert_eq!(
        by_name("active-v2").source,
        crate::queries::ChangeSource::Active
    );
    assert_eq!(by_name("plain-active").inventory, Inventory::V0);

    // archive 折叠进月组
    let archived_names: Vec<&str> = list
        .archive_groups
        .iter()
        .flat_map(|group| group.changes.iter().map(|c| c.name.as_str()))
        .collect();
    assert_eq!(
        archived_names,
        vec!["2026-09-15-archived-v0", "2026-05-01-archived-v1"],
        "月组新月份在前"
    );
    assert_eq!(list.archive_groups[0].changes[0].inventory, Inventory::V0);
    assert_eq!(list.archive_groups[1].changes[0].inventory, Inventory::V1);

    // active 扫描不得把 archive 目录本身误当作名为 archive 的 active change
    assert!(!active_names.contains(&"archive"));
}

#[test]
fn archive按月前缀折叠_组间新月份在前_组内新名在前() {
    let ws = TempWs::new("month-groups");
    ws.change("openspec/changes/archive/2026-01-02-a", &[]);
    ws.change("openspec/changes/archive/2026-01-15-b", &[]);
    ws.change("openspec/changes/archive/2026-03-05-c", &[]);
    ws.change("openspec/changes/archive/2025-12-31-d", &[]);

    let list = list_changes(&ws.layout());

    let months: Vec<Option<&str>> = list
        .archive_groups
        .iter()
        .map(|group| group.month.as_deref())
        .collect();
    assert_eq!(
        months,
        vec![Some("2026-03"), Some("2026-01"), Some("2025-12")]
    );

    let jan: Vec<&str> = list.archive_groups[1]
        .changes
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(
        jan,
        vec!["2026-01-15-b", "2026-01-02-a"],
        "组内按目录名倒序（新名在前）"
    );
}

#[test]
fn created优先取workflow_json_archive无created时回退目录名前缀() {
    // fixture v1-a：workflow.json 仅 workflow_type（无 created），目录名 2026-07-06- 前缀
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("v1-a");
    let ws = TempWs::new("created-fallback");
    let archived =
        ws.0.join("openspec/changes/archive/2026-07-06-backtrack-reason-propagation");
    fs::create_dir_all(&archived).expect("创建 archive 目录失败");
    for entry in fs::read_dir(&fixture).expect("读取 fixture 失败").flatten() {
        let target = archived.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("拷贝 fixture 失败");
        }
    }

    let list = list_changes(&ws.layout());
    assert_eq!(list.archive_groups.len(), 1);
    assert_eq!(list.archive_groups[0].month.as_deref(), Some("2026-07"));
    let summary = &list.archive_groups[0].changes[0];
    assert_eq!(
        summary.created.as_deref(),
        Some("2026-07-06"),
        "回退目录名日期前缀"
    );

    // 对照：workflow.json 带 created 时优先于目录名
    let ws2 = TempWs::new("created-priority");
    ws2.change(
        "openspec/changes/archive/2026-02-03-with-created",
        &[("workflow.json", V1_WORKFLOW)],
    );
    let list2 = list_changes(&ws2.layout());
    assert_eq!(
        list2.archive_groups[0].changes[0].created.as_deref(),
        Some("2026-01-01")
    );
}

#[test]
fn changes或archive目录缺失时返回空结果而非报错() {
    let ws = TempWs::new("missing-roots");
    // 整个 openspec 子树都不存在
    let list = list_changes(&ws.layout());
    assert!(list.active.is_empty());
    assert!(list.archive_groups.is_empty());

    // 仅缺 archive 子目录
    ws.change("openspec/changes/only-active", &[("proposal.md", "# 提案")]);
    let list2 = list_changes(&ws.layout());
    assert_eq!(list2.active.len(), 1);
    assert!(list2.archive_groups.is_empty());
}

#[test]
fn workflow_json不可解析的change标注unparsable且仍入列() {
    let ws = TempWs::new("unparsable");
    ws.change(
        "openspec/changes/broken-change",
        &[("workflow.json", "{ 残缺"), ("proposal.md", "# 提案")],
    );

    let list = list_changes(&ws.layout());
    assert_eq!(list.active.len(), 1, "unparsable 仍入列");
    assert!(list.active[0].unparsable);
    assert_eq!(
        list.active[0].inventory,
        Inventory::V2,
        "文件存在按现役代际假设"
    );
}

#[test]
fn 无日期前缀的archive目录归入未知时间组置尾() {
    let ws = TempWs::new("unknown-month");
    ws.change("openspec/changes/archive/2026-04-01-dated", &[]);
    ws.change("openspec/changes/archive/no-date-prefix", &[]);
    ws.change("openspec/changes/archive/not-a-date-2026", &[]);

    let list = list_changes(&ws.layout());
    assert_eq!(list.archive_groups.len(), 2);
    assert_eq!(list.archive_groups[0].month.as_deref(), Some("2026-04"));
    let unknown = &list.archive_groups[1];
    assert_eq!(unknown.month, None, "无前缀 → 未知时间组");
    let mut names: Vec<&str> = unknown.changes.iter().map(|c| c.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["no-date-prefix", "not-a-date-2026"]);
}

#[test]
fn changes_root存在但为空时全空() {
    let ws = TempWs::new("empty-tree");
    fs::create_dir_all(ws.0.join("openspec/changes")).expect("创建空 changes 失败");

    let list = list_changes(&ws.layout());
    assert!(list.active.is_empty());
    assert!(list.archive_groups.is_empty());
}

#[test]
fn 目录树混入普通文件时被忽略() {
    let ws = TempWs::new("mixed-files");
    ws.change("openspec/changes/real-change", &[("proposal.md", "# 提案")]);
    // 混入的普通文件（非目录）
    fs::write(ws.0.join("openspec/changes/stray.md"), "散落文件").expect("写散落文件失败");
    fs::create_dir_all(ws.0.join("openspec/changes/archive")).expect("创建 archive 目录失败");
    fs::write(ws.0.join("openspec/changes/archive/loose.txt"), "散落文件").expect("写散落文件失败");

    let list = list_changes(&ws.layout());
    assert_eq!(list.active.len(), 1);
    assert_eq!(list.active[0].name, "real-change");
    // archive 内只有散落文件、无 change 目录 → 空分组（未知时间组不凭空产生）
    assert_eq!(list.archive_groups.len(), 0);
}

#[test]
fn 单月单元素分组正确成组() {
    let ws = TempWs::new("single-month");
    ws.change(
        "openspec/changes/archive/2025-11-09-only-one",
        &[("proposal.md", "# v0")],
    );

    let list = list_changes(&ws.layout());
    assert_eq!(list.archive_groups.len(), 1);
    assert_eq!(list.archive_groups[0].month.as_deref(), Some("2025-11"));
    assert_eq!(list.archive_groups[0].changes.len(), 1);
    assert_eq!(list.archive_groups[0].changes[0].inventory, Inventory::V0);
}

/// 递归拷贝目录（仅测试辅助使用）。
fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("创建目标目录失败");
    for entry in fs::read_dir(src).expect("读取源目录失败").flatten() {
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("拷贝文件失败");
        }
    }
}
