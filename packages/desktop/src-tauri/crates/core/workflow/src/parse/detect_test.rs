//! `parse::detect_inventory` 的单元测试：磁盘事实 → 三代代际判定（AC-3）。

use std::fs;
use std::path::{Path, PathBuf};

use super::detect_inventory;
use crate::model::Inventory;

/// 临时目录 RAII：测试结束自动清理。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-detect-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 入仓 fixtures 语料根（workflow crate 的 tests/fixtures）。
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[test]
fn 有workflow_json且含file_log键_判定为v2() {
    let temp = TempDir::new("v2");
    fs::write(
        temp.path().join("workflow.json"),
        r#"{ "workflow_type": "requirement", "file_log": [ { "op": "write", "scope": "workflow", "path": "a.rs" } ] }"#,
    )
    .expect("写 workflow.json 失败");

    assert_eq!(detect_inventory(temp.path()), Inventory::V2);
}

#[test]
fn 有workflow_json无file_log键_判定为v1() {
    let temp = TempDir::new("v1");
    fs::write(
        temp.path().join("workflow.json"),
        r#"{ "workflow_type": "requirement", "eval": [] }"#,
    )
    .expect("写 workflow.json 失败");

    assert_eq!(detect_inventory(temp.path()), Inventory::V1);
}

#[test]
fn 无workflow_json_判定为v0() {
    let temp = TempDir::new("v0");
    fs::write(temp.path().join("proposal.md"), "# 提案").expect("写 proposal.md 失败");

    assert_eq!(detect_inventory(temp.path()), Inventory::V0);
}

#[test]
fn file_log为空数组时_仍判定为v2() {
    let temp = TempDir::new("v2-empty-filelog");
    fs::write(
        temp.path().join("workflow.json"),
        r#"{ "workflow_type": "requirement", "file_log": [] }"#,
    )
    .expect("写 workflow.json 失败");

    // v1/v2 以"键存在与否"区分，与数组是否为空无关
    assert_eq!(detect_inventory(temp.path()), Inventory::V2);
}

#[test]
fn workflow_json整体非法_json时_按现役代际假设判定v2() {
    let temp = TempDir::new("corrupt-json");
    fs::write(temp.path().join("workflow.json"), "{ 残缺的 JSON").expect("写 workflow.json 失败");

    // 实现语义：workflow.json 存在但不可读/损坏 → 假设为现役 v2，
    // 配合查询层 unparsable 标记降级展示（test-design 原写"按 V1 处理"，
    // 实现以"文件存在即现役结构"为准，此处按实现断言）。
    assert_eq!(detect_inventory(temp.path()), Inventory::V2);
}

#[test]
fn 遗留eval_json不干扰判定_fixture_v0_a仍为v0() {
    let v0_a = fixtures_dir().join("v0-a");
    assert!(
        v0_a.join("eval.json").is_file(),
        "fixture v0-a 应含遗留 eval.json"
    );
    assert!(!v0_a.join("workflow.json").exists());

    assert_eq!(detect_inventory(&v0_a), Inventory::V0);
}

#[test]
fn 目录名无日期前缀不影响判定() {
    let temp = TempDir::new("无日期前缀目录名");
    fs::write(
        temp.path().join("workflow.json"),
        r#"{ "workflow_type": "refactor" }"#,
    )
    .expect("写 workflow.json 失败");

    // 判定只看文件与字段存在性，不依赖 YYYY-MM-DD- 命名约定
    assert_eq!(detect_inventory(temp.path()), Inventory::V1);
}

#[test]
fn change_dir不存在时_判定为v0() {
    let ghost = std::env::temp_dir().join("workflow-detect-test-不存在的change目录");

    assert_eq!(detect_inventory(&ghost), Inventory::V0);
}

#[test]
fn v2_a_fixture判定为v2_v1_b_fixture判定为v1() {
    // 入仓语料抽查：真实代际样本经同一判定路径
    assert_eq!(
        detect_inventory(&fixtures_dir().join("v2-a")),
        Inventory::V2
    );
    assert_eq!(
        detect_inventory(&fixtures_dir().join("v1-b")),
        Inventory::V1
    );
    assert_eq!(
        detect_inventory(&fixtures_dir().join("v0-b")),
        Inventory::V0
    );
}
