//! `parse::parse_workflow_file` 的单元测试：两段式宽松解析与单条降级（AC-4）。

use std::fs;
use std::path::PathBuf;

use super::{parse_workflow_file, WorkflowFileParse};
use crate::model::{FileLogOp, Verdict};

/// 临时目录 RAII：测试结束自动清理。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-parse-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn write_workflow(&self, text: &str) -> PathBuf {
        let path = self.0.join("workflow.json");
        fs::write(&path, text).expect("写 workflow.json 失败");
        path
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

const V2_FULL: &str = r#"{
  "workflow_type": "requirement",
  "created": "2026-08-01",
  "file_log": [
    { "op": "write", "scope": "workflow", "attempt": 1, "path": "src/a.rs", "at": "2026-08-01T08:00:00Z" },
    { "op": "revert", "scope": "workflow", "path": "src/b.rs" }
  ],
  "active_phase": { "phase": "implement", "attempt": 2, "start_at": "2026-08-03T10:00:00Z" },
  "interrupted": [
    { "phase": "test-gen", "attempt": 1, "start_at": "2026-08-02T11:00:00Z", "end_at": "2026-08-02T12:00:00Z" }
  ],
  "eval": [
    {
      "phase": "proposal",
      "attempt": 1,
      "verdict": "pass",
      "report": "提案评估通过。",
      "checklist": [
        { "item": "问题清晰", "pass": true, "evidence": "L1-10" }
      ],
      "timestamp": "2026-08-01T07:00:00Z"
    },
    {
      "phase": "dev-design",
      "verdict": "fail",
      "report": "设计首轮未过。",
      "checklist": [],
      "backtrack_to": "proposal",
      "backtrack_reason": "范围未定"
    }
  ]
}"#;

#[test]
fn 合法v2全字段解析为强类型() {
    let temp = TempDir::new("v2-full");
    let path = temp.write_workflow(V2_FULL);

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("合法 v2 应解析为 Parsed");
    };

    assert_eq!(workflow.workflow_type, "requirement");
    assert_eq!(workflow.created.as_deref(), Some("2026-08-01"));

    // eval：两条，verdict / checklist / backtrack 强类型还原
    assert_eq!(workflow.eval.len(), 2);
    let first = &workflow.eval[0];
    assert_eq!(first.phase, "proposal");
    assert_eq!(first.attempt, Some(1));
    assert_eq!(first.verdict, Verdict::Pass);
    assert_eq!(first.report, "提案评估通过。");
    assert_eq!(first.checklist.len(), 1);
    assert_eq!(first.checklist[0].item, "问题清晰");
    assert!(first.checklist[0].pass);
    assert_eq!(first.checklist[0].evidence, "L1-10");
    assert!(first.timestamp.is_some());

    let second = &workflow.eval[1];
    assert_eq!(second.verdict, Verdict::Fail);
    assert_eq!(second.attempt, None);
    assert_eq!(second.backtrack_to.as_deref(), Some("proposal"));
    assert_eq!(second.backtrack_reason.as_deref(), Some("范围未定"));

    // file_log：两条，op 枚举与缺省 attempt / at 宽松还原
    let file_log = workflow.file_log.expect("v2 应有 file_log");
    assert_eq!(file_log.len(), 2);
    assert_eq!(file_log[0].op, FileLogOp::Write);
    assert_eq!(file_log[0].attempt, Some(1));
    assert_eq!(file_log[0].path, "src/a.rs");
    assert!(file_log[0].at.is_some());
    assert_eq!(file_log[1].op, FileLogOp::Revert);
    assert_eq!(file_log[1].attempt, None);
    assert_eq!(file_log[1].at, None);

    // active_phase / interrupted
    let active = workflow.active_phase.expect("应有 active_phase");
    assert_eq!(active.phase, "implement");
    assert_eq!(active.attempt, 2);
    assert!(active.start_at.is_some());
    assert_eq!(workflow.interrupted.len(), 1);
    assert_eq!(workflow.interrupted[0].phase, "test-gen");
    assert!(workflow.interrupted[0].end_at.is_some());
}

#[test]
fn 未知顶层键被忽略_fixture_v1_c正常解析() {
    let path = fixtures_dir().join("v1-c").join("workflow.json");
    let text = fs::read_to_string(&path).expect("fixture v1-c workflow.json 应存在");
    // 前置：fixture 确实含 legacy 键
    assert!(text.contains("\"files\"") || text.contains("\"source\""), "v1-c 应含 legacy 未知键");

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("含未知键的 v1 应正常解析（未知字段忽略）");
    };
    assert_eq!(workflow.workflow_type, "requirement");
    assert!(workflow.file_log.is_none(), "v1 无 file_log");
    assert!(!workflow.eval.is_empty(), "v1-c 的 eval 应还原");
}

#[test]
fn 文件整体非法json时返回_unparsable() {
    let temp = TempDir::new("invalid-json");
    let path = temp.write_workflow("{ eval: [残缺");

    let WorkflowFileParse::Unparsable { reason } = parse_workflow_file(&path) else {
        panic!("非法 JSON 应返回 Unparsable");
    };
    assert!(!reason.is_empty(), "应携带原因");
}

#[test]
fn workflow_type缺失时整体_unparsable_不返回半份数据() {
    let temp = TempDir::new("missing-type");
    let path = temp.write_workflow(r#"{ "eval": [], "file_log": [] }"#);

    let WorkflowFileParse::Unparsable { .. } = parse_workflow_file(&path) else {
        panic!("缺 workflow_type 应整体 Unparsable");
    };

    // 类型非法同样整体降级
    let temp2 = TempDir::new("bad-type");
    let path2 = temp2.write_workflow(r#"{ "workflow_type": 42 }"#);
    let WorkflowFileParse::Unparsable { .. } = parse_workflow_file(&path2) else {
        panic!("workflow_type 非字符串应整体 Unparsable");
    };
}

#[test]
fn 单条eval条目checklist损坏时跳过该条其余保留() {
    let temp = TempDir::new("bad-eval-entry");
    let path = temp.write_workflow(
        r#"{
          "workflow_type": "requirement",
          "eval": [
            { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "完好", "checklist": [] },
            { "phase": "dev-design", "attempt": 1, "verdict": "pass", "report": "损坏", "checklist": "不是数组" },
            { "phase": "test-design", "attempt": 1, "verdict": "pass", "report": "也完好", "checklist": [] }
          ]
        }"#,
    );

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("单条损坏不应整体降级");
    };
    let phases: Vec<&str> = workflow.eval.iter().map(|entry| entry.phase.as_str()).collect();
    assert_eq!(phases, vec!["proposal", "test-design"], "损坏条目被跳过，其余完整保留");
}

#[test]
fn 单条eval的verdict为非法枚举时该条降级跳过() {
    let temp = TempDir::new("bad-verdict");
    let path = temp.write_workflow(
        r#"{
          "workflow_type": "requirement",
          "eval": [
            { "phase": "proposal", "attempt": 1, "verdict": "not-a-verdict", "report": "非法枚举", "checklist": [] },
            { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "合法 fail", "checklist": [] }
          ]
        }"#,
    );

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("单条 verdict 损坏不应整体降级");
    };
    assert_eq!(workflow.eval.len(), 1);
    assert_eq!(workflow.eval[0].verdict, Verdict::Fail);
}

#[test]
fn 单条file_log条目op非法时该条跳过其余保留() {
    let temp = TempDir::new("bad-filelog");
    let path = temp.write_workflow(
        r#"{
          "workflow_type": "requirement",
          "file_log": [
            { "op": "write", "scope": "workflow", "path": "src/keep.rs" },
            { "op": "explode", "scope": "workflow", "path": "src/skip.rs" },
            { "op": "delete", "scope": "workflow", "path": "src/also-keep.rs" }
          ]
        }"#,
    );

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("单条 file_log 损坏不应整体降级");
    };
    let file_log = workflow.file_log.expect("file_log 键存在");
    let paths: Vec<&str> = file_log.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, vec!["src/keep.rs", "src/also-keep.rs"]);
}

#[test]
fn eval为空数组时解析为空vec() {
    let temp = TempDir::new("empty-eval");
    let path = temp.write_workflow(r#"{ "workflow_type": "requirement", "eval": [] }"#);

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("空 eval 应正常解析");
    };
    assert!(workflow.eval.is_empty());
}

#[test]
fn file_log空数组为some空vec_与缺失的none严格区分() {
    let temp = TempDir::new("empty-filelog");
    let path = temp.write_workflow(r#"{ "workflow_type": "requirement", "file_log": [] }"#);

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("空 file_log 应正常解析");
    };
    let file_log = workflow.file_log.expect("键存在 → Some");
    assert!(file_log.is_empty(), "空数组 → Some(空 Vec)");

    // 对照组：键缺失 → None（v1/v2 判定的数据基础）
    let temp2 = TempDir::new("no-filelog-key");
    let path2 = temp2.write_workflow(r#"{ "workflow_type": "requirement" }"#);
    let WorkflowFileParse::Parsed(workflow2) = parse_workflow_file(&path2) else {
        panic!("无 file_log 应正常解析");
    };
    assert!(workflow2.file_log.is_none(), "键缺失 → None");
}

#[test]
fn 非法时间戳降级为none_条目本身保留() {
    let temp = TempDir::new("bad-timestamp");
    let path = temp.write_workflow(
        r#"{
          "workflow_type": "requirement",
          "eval": [
            { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "时间戳非法", "checklist": [], "timestamp": "2026年8月12日" }
          ]
        }"#,
    );

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("时间戳非法不应炸条目");
    };
    assert_eq!(workflow.eval.len(), 1);
    assert_eq!(workflow.eval[0].timestamp, None, "时间戳降级为 None");
}

#[test]
fn attempt缺失为none_负数条目整体跳过() {
    let temp = TempDir::new("attempt-cases");
    let path = temp.write_workflow(
        r#"{
          "workflow_type": "requirement",
          "eval": [
            { "phase": "proposal", "verdict": "pass", "report": "无 attempt 字段", "checklist": [] },
            { "phase": "dev-design", "attempt": -1, "verdict": "pass", "report": "负数 attempt", "checklist": [] },
            { "phase": "test-design", "attempt": 3, "verdict": "pass", "report": "正常 attempt", "checklist": [] }
          ]
        }"#,
    );

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("解析不应失败");
    };
    let phases: Vec<&str> = workflow.eval.iter().map(|entry| entry.phase.as_str()).collect();
    assert_eq!(phases, vec!["proposal", "test-design"]);
    assert_eq!(workflow.eval[0].attempt, None, "attempt 缺失 → None");
    assert_eq!(workflow.eval[1].attempt, Some(3));
}

#[test]
fn 超长与含特殊字符的report原样保留() {
    let mut report = String::from("开头🎯含emoji与换行\n");
    for i in 0..100 {
        report.push_str(&format!("第 {i} 行——超长内容不含换行与转义，长度远超一千字符上限验证。🚀✅\n"));
    }
    assert!(report.chars().count() > 1000);

    let temp = TempDir::new("long-report");
    let json = format!(
        r#"{{ "workflow_type": "requirement", "eval": [ {{ "phase": "proposal", "attempt": 1, "verdict": "pass", "report": {}, "checklist": [] }} ] }}"#,
        serde_json::to_string(&report).expect("报告序列化失败")
    );
    let path = temp.write_workflow(&json);

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("超长 report 应正常解析");
    };
    assert_eq!(workflow.eval[0].report, report, "report 应逐字符原样保留");
}

#[test]
fn checklist条目缺失evidence时按损坏条目降级跳过() {
    // ChecklistItem.evidence 为必填 String（无 serde default）：
    // 缺 evidence 的 checklist 项使所在 eval 条目解析失败，按"单条损坏跳过"处理
    let temp = TempDir::new("missing-evidence");
    let path = temp.write_workflow(
        r#"{
          "workflow_type": "requirement",
          "eval": [
            { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "checklist 项缺 evidence",
              "checklist": [ { "item": "检查项", "pass": true } ] },
            { "phase": "dev-design", "attempt": 1, "verdict": "pass", "report": "完好条目", "checklist": [] }
          ]
        }"#,
    );

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&path) else {
        panic!("单条损坏不应整体降级");
    };
    let phases: Vec<&str> = workflow.eval.iter().map(|entry| entry.phase.as_str()).collect();
    assert_eq!(phases, vec!["dev-design"], "缺 evidence 的条目按损坏跳过");
}

#[test]
fn load_workflow_对缺失与损坏文件的_none语义() {
    use super::load_workflow;

    // 文件不存在 → None
    let temp = TempDir::new("load-missing");
    assert!(load_workflow(temp.path()).is_none());

    // 文件损坏 → None（Unparsable 被收敛）
    fs::write(temp.path().join("workflow.json"), "残缺").expect("写文件失败");
    assert!(load_workflow(temp.path()).is_none());

    // 文件合法 → Some
    fs::write(
        temp.path().join("workflow.json"),
        r#"{ "workflow_type": "requirement" }"#,
    )
    .expect("写文件失败");
    assert!(load_workflow(temp.path()).is_some());
}
