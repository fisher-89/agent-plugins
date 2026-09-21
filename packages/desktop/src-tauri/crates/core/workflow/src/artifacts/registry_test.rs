//! `artifacts::registry` 的单元测试：候选枚举、静态注册表分发与信封读取（AC-7 / AC-8）。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::registry::{discover_artifacts, read_artifact};
use crate::model::{Inventory, Workflow};

/// 临时 change 目录 RAII：测试结束自动清理。
struct TempChange(PathBuf);

impl TempChange {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-registry-test-{}-{}",
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

const EVAL_WITH_CHECKLIST: &str = r#"{
  "workflow_type": "requirement",
  "file_log": [],
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "有清单", "checklist": [
      { "item": "问题清晰", "pass": true, "evidence": "L1-10" }
    ] },
    { "phase": "dev-design", "attempt": 1, "verdict": "pass", "report": "无清单", "checklist": [] }
  ]
}"#;

fn parse_workflow(text: &str) -> Workflow {
    match crate::parse::parse_workflow_file(&write_temp_json(text)) {
        crate::parse::WorkflowFileParse::Parsed(workflow) => workflow,
        crate::parse::WorkflowFileParse::Unparsable { reason } => panic!("workflow 应可解析: {reason}"),
    }
}

fn write_temp_json(text: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "workflow-registry-wf-{}-{}.json",
        std::process::id(),
        seq
    ));
    fs::write(&path, text).expect("写临时 workflow.json 失败");
    path
}

#[test]
fn 文件树与eval候选全部命中为descriptor清单() {
    let change = TempChange::new("discover-all");
    change.write("proposal.md", "# 提案");
    change.write("tasks.md", "- [x] 一\n- [ ] 二\n");
    change.write("workflow.json", EVAL_WITH_CHECKLIST);
    let workflow = parse_workflow(EVAL_WITH_CHECKLIST);

    let descriptors = discover_artifacts(&change.0, Inventory::V2, Some(&workflow));

    // 期望命中：proposal.md（markdown-doc）、tasks.md（markdown-doc + tasks-progress）、
    // eval[0]（eval-checklist，eval[1] 空 checklist 不命中）
    let summary: Vec<(String, String)> = descriptors
        .iter()
        .map(|d| (d.kind.clone(), d.source.clone()))
        .collect();
    assert!(summary.contains(&("markdown-doc".into(), "proposal.md".into())));
    assert!(summary.contains(&("tasks-progress".into(), "tasks.md".into())));
    assert!(summary.contains(&("eval-checklist".into(), "0".into())));
    // 三个字段齐全（title 非空）
    assert!(descriptors.iter().all(|d| !d.title.is_empty()));
    // 插件顺序：特化 kind 先于 markdown-doc
    let kinds: Vec<&str> = descriptors.iter().map(|d| d.kind.as_str()).collect();
    let tasks_pos = kinds.iter().position(|k| *k == "tasks-progress").unwrap();
    let doc_pos = kinds.iter().position(|k| *k == "markdown-doc").unwrap();
    assert!(tasks_pos < doc_pos, "tasks-progress 应排在 markdown-doc 之前");
}

#[test]
fn 同一候选多kind命中并存_无排他() {
    let change = TempChange::new("multi-kind");
    change.write("tasks.md", "- [x] 完成\n- [ ] 待办\n");

    let descriptors = discover_artifacts(&change.0, Inventory::V0, None);

    let hits: Vec<&str> = descriptors.iter().map(|d| d.kind.as_str()).collect();
    assert!(hits.contains(&"tasks-progress"), "tasks-progress 命中");
    assert!(hits.contains(&"markdown-doc"), "同一 tasks.md 仍以 markdown-doc 并存命中");
    assert_eq!(
        descriptors.iter().filter(|d| d.kind == "tasks-progress").count(),
        1
    );
}

#[test]
fn 无插件命中时返回空清单() {
    let change = TempChange::new("no-hit");
    change.write("data.json", "{ \"not\": \"markdown\" }");
    change.write("code.ts", "const x = 1;");

    let descriptors = discover_artifacts(&change.0, Inventory::V0, None);
    assert!(descriptors.is_empty(), "json/ts 不被任何插件命中");
}

#[test]
fn change目录为空或不存在时返回空清单不报错() {
    let empty = TempChange::new("empty-dir");
    assert!(discover_artifacts(&empty.0, Inventory::V0, None).is_empty());

    let ghost = std::env::temp_dir().join("workflow-registry-test-不存在的change目录");
    assert!(discover_artifacts(&ghost, Inventory::V2, None).is_empty());
}

#[test]
fn 已注册kind与有效source返回五字段齐全的信封() {
    let change = TempChange::new("read-ok");
    change.write("tasks.md", "- [x] 完成\n");
    change.write("proposal.md", "# 提案正文");

    let envelope =
        read_artifact(&change.0, Inventory::V0, None, "tasks-progress", "tasks.md")
            .expect("tasks-progress 信封应可读取");
    assert_eq!(envelope.kind, "tasks-progress");
    assert_eq!(envelope.version, 1, "version 从 1 起");
    assert_eq!(envelope.title, "任务进度");
    assert!(envelope.payload.is_object());
    assert!(envelope.fallback_text.is_some(), "五字段之 fallback_text 齐全");

    let envelope = read_artifact(&change.0, Inventory::V0, None, "markdown-doc", "proposal.md")
        .expect("markdown-doc 信封应可读取");
    assert_eq!(envelope.kind, "markdown-doc");
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.payload["markdown"], "# 提案正文");
}

#[test]
fn 未注册的kind返回none() {
    let change = TempChange::new("unknown-kind");
    change.write("proposal.md", "# 提案");

    // file-log 是第二刀规划的 kind，当前未注册
    assert!(read_artifact(&change.0, Inventory::V2, None, "file-log", "proposal.md").is_none());
    assert!(read_artifact(&change.0, Inventory::V2, None, "", "proposal.md").is_none());
}

#[test]
fn source指向不存在文件或越界eval序号时返回none() {
    let change = TempChange::new("bad-source");
    change.write("workflow.json", EVAL_WITH_CHECKLIST);
    let workflow = parse_workflow(EVAL_WITH_CHECKLIST);

    // 不存在的文件 source
    assert!(
        read_artifact(&change.0, Inventory::V2, Some(&workflow), "markdown-doc", "不存在的.md")
            .is_none()
    );
    // 越界 eval 序号（eval 长度为 2）
    assert!(
        read_artifact(&change.0, Inventory::V2, Some(&workflow), "eval-checklist", "99").is_none()
    );
    // 非数字 source 被 decode 为文件路径，指向不存在文件 → None
    assert!(
        read_artifact(&change.0, Inventory::V2, Some(&workflow), "eval-checklist", "not-a-number")
            .is_none()
    );
}

#[test]
fn 敌意source在注册表层被拒绝_不逃逸change目录() {
    let change = TempChange::new("hostile-source");
    change.write("proposal.md", "# 提案正文");

    // 越权目标：change 目录树外的秘密文件（workspace 侧真实存在，
    // 排除"恰好读不到"的假阳性）
    let outside = change.0.parent().unwrap_or_else(|| std::path::Path::new("/"));
    let secret = outside.join("registry-test-secret.md");
    fs::write(&secret, "# 不应被越权读取").expect("写外部 secret 失败");

    // 形态层拒绝：.. 分量 / 绝对路径 / 反斜杠 / 盘符 / 空串 / 空分量 / 点分量
    for hostile in [
        "../registry-test-secret.md",
        "proposal.md/../registry-test-secret.md",
        "..\\..\\registry-test-secret.md",
        "..",
        ".",
        "proposal.md/../",
        "",
        "/registry-test-secret.md",
        "C:\\evil\\secret.md",
        "C:/evil/secret.md",
    ] {
        assert!(
            read_artifact(&change.0, Inventory::V0, None, "markdown-doc", hostile).is_none(),
            "敌意 source {hostile:?} 必须在 read_artifact 入口被拒绝"
        );
    }

    // 越权目标确实存在（排除环境假阳性），但无法经 read_artifact 触达
    assert!(fs::read_to_string(&secret).is_ok());
    let _ = fs::remove_file(&secret);

    // 正向对照：合法相对路径不受校验误伤
    assert!(
        read_artifact(&change.0, Inventory::V0, None, "markdown-doc", "proposal.md").is_some(),
        "合法 source 不应被包含性校验误伤"
    );
}

#[test]
fn eval候选的source序号串与discover寻址一致可回放() {
    let change = TempChange::new("eval-roundtrip");
    change.write("workflow.json", EVAL_WITH_CHECKLIST);
    let workflow = parse_workflow(EVAL_WITH_CHECKLIST);

    let descriptors = discover_artifacts(&change.0, Inventory::V2, Some(&workflow));
    let eval_descriptor = descriptors
        .iter()
        .find(|d| d.kind == "eval-checklist")
        .expect("应产出 eval-checklist descriptor");

    // discover 给出的 source 可被 read_artifact 原样回放
    let envelope = read_artifact(
        &change.0,
        Inventory::V2,
        Some(&workflow),
        &eval_descriptor.kind,
        &eval_descriptor.source,
    )
    .expect("descriptor 的 (kind, source) 应可回放成信封");
    assert_eq!(envelope.payload["phase"], "proposal");
    assert_eq!(envelope.payload["attempt"], 1);
    assert_eq!(envelope.payload["verdict"], "pass");
    assert_eq!(envelope.payload["items"].as_array().map(Vec::len), Some(1));
}
