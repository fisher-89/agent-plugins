//! `artifacts::tasks_progress` 的单元测试：tasks.md 勾选计数（AC-9）。

use std::fs;
use std::path::{Path, PathBuf};

use super::registry::{ArtifactInput, ArtifactPlugin};
use super::tasks_progress::{TasksProgressPlugin, KIND};
use super::ArtifactCandidate;
use crate::model::Inventory;

/// 临时 change 目录 RAII：测试结束自动清理。
struct TempChange(PathBuf);

impl TempChange {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-taskprog-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn write(&self, rel_path: &str, content: &str) {
        let path = self.0.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建子目录失败");
        }
        fs::write(path, content).expect("写文件失败");
    }
}

impl Drop for TempChange {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// tasks.md 文件候选（相对路径恒为 tasks.md）。
fn tasks_candidate() -> ArtifactCandidate {
    ArtifactCandidate::File {
        relative_path: PathBuf::from("tasks.md"),
    }
}

fn input_for<'a>(change_dir: &'a Path, candidate: &'a ArtifactCandidate) -> ArtifactInput<'a> {
    ArtifactInput {
        change_dir,
        inventory: Inventory::V0,
        workflow: None,
        candidate,
    }
}

#[test]
fn 混合勾选计数正确且三者自洽() {
    let change = TempChange::new("mixed");
    change.write(
        "tasks.md",
        "- [ ] 待办一\n- [x] 已办一\n- [ ] 待办二\n- [x] 已办二\n- [x] 已办三\n",
    );
    let candidate = tasks_candidate();
    let input = input_for(&change.0, &candidate);

    assert!(TasksProgressPlugin.matches(&input));
    let envelope = TasksProgressPlugin.parse(&input).expect("应产出信封");

    assert_eq!(envelope.kind, KIND);
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.title, "任务进度");
    assert_eq!(envelope.payload["total"], 5);
    assert_eq!(envelope.payload["done"], 3);
    assert_eq!(envelope.payload["pending"], 2);
    let total = envelope.payload["total"].as_u64().unwrap();
    let done = envelope.payload["done"].as_u64().unwrap();
    let pending = envelope.payload["pending"].as_u64().unwrap();
    assert_eq!(done + pending, total, "done + pending = total 自洽");
}

#[test]
fn fallback_text为统计结果文本() {
    let change = TempChange::new("fallback");
    change.write(
        "tasks.md",
        "- [x] 甲\n- [x] 乙\n- [ ] 丙\n- [ ] 丁\n- [ ] 戊\n",
    );
    let candidate = tasks_candidate();

    let envelope = TasksProgressPlugin
        .parse(&input_for(&change.0, &candidate))
        .expect("应产出信封");
    let fallback = envelope.fallback_text.expect("fallback 非空");
    assert!(fallback.contains("2 / 5"), "实际: {fallback}");
    assert!(fallback.contains("待办 3"), "实际: {fallback}");
}

#[test]
fn tasks_md不存在时matcher按名命中但解析产出none() {
    // matcher 以候选文件名判定（不触磁盘）；文件缺失时 parse 返回 None，
    // discover 层因此不产出 descriptor——两条路径分别断言
    let change = TempChange::new("missing");
    change.write("proposal.md", "# 提案");
    let candidate = tasks_candidate();
    let input = input_for(&change.0, &candidate);

    assert!(
        TasksProgressPlugin.matches(&input),
        "matcher 仅按文件名判定"
    );
    assert!(
        TasksProgressPlugin.parse(&input).is_none(),
        "文件缺失 → 解析 None"
    );
    assert!(
        !super::discover_artifacts(&change.0, Inventory::V0, None)
            .iter()
            .any(|d| d.kind == KIND),
        "discover 不产出 tasks-progress descriptor"
    );
}

#[test]
fn 空tasks_md产出零计数信封() {
    let change = TempChange::new("empty");
    change.write("tasks.md", "");
    let candidate = tasks_candidate();
    let input = input_for(&change.0, &candidate);

    assert!(
        TasksProgressPlugin.matches(&input),
        "文件存在即命中，与内容无关"
    );
    let envelope = TasksProgressPlugin.parse(&input).expect("空文件仍产出信封");
    assert_eq!(envelope.payload["total"], 0);
    assert_eq!(envelope.payload["done"], 0);
    assert_eq!(envelope.payload["pending"], 0);
}

#[test]
fn 全部勾选与全部未勾两个极端() {
    let all_done = TempChange::new("all-done");
    all_done.write("tasks.md", "- [x] 一\n- [X] 二\n");
    let done_candidate = tasks_candidate();
    let envelope = TasksProgressPlugin
        .parse(&input_for(&all_done.0, &done_candidate))
        .expect("信封");
    assert_eq!(envelope.payload["done"], 2);
    assert_eq!(envelope.payload["pending"], 0);
    assert_eq!(envelope.payload["total"], 2);

    let all_pending = TempChange::new("all-pending");
    all_pending.write("tasks.md", "- [ ] 一\n- [ ] 二\n- [ ] 三\n");
    let pending_candidate = tasks_candidate();
    let envelope = TasksProgressPlugin
        .parse(&input_for(&all_pending.0, &pending_candidate))
        .expect("信封");
    assert_eq!(envelope.payload["done"], 0);
    assert_eq!(envelope.payload["pending"], 3);
    assert_eq!(envelope.payload["total"], 3);
}

#[test]
fn 缩进嵌套复选框计数_普通行与行内字样不计入() {
    let change = TempChange::new("nested");
    change.write(
        "tasks.md",
        "- [x] 顶层\n  - [ ] 嵌套待办\n\t- [x] 制表符缩进已完成\n- 普通列表行\n正文里出现 [x] 与 - [ ] 字样的段落不算\n> 引用块行首不是列表标记，不计\n",
    );
    let candidate = tasks_candidate();

    let envelope = TasksProgressPlugin
        .parse(&input_for(&change.0, &candidate))
        .expect("信封");
    // 计入：顶层 [x]、空格缩进 [ ]、制表符缩进 [x] → done=2 pending=1
    assert_eq!(envelope.payload["done"], 2);
    assert_eq!(envelope.payload["pending"], 1);
    assert_eq!(envelope.payload["total"], 3);
}
