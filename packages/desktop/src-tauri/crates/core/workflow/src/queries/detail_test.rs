//! `queries::change_detail` 的单元测试：9 站流水线聚合 + 运行状态 + 产物清单（AC-6）。

use std::fs;
use std::path::{Path, PathBuf};

use super::change_detail;
use super::detail::PIPELINE_PHASES;
use crate::model::{FileLogOp, Inventory};
use foundation::layout::resolve;

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-detail-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn change(&self, rel_dir: &str, files: &[(&str, &str)]) {
        let dir = self.0.join(rel_dir);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        for (name, content) in files {
            fs::write(dir.join(name), content).expect("写文件失败");
        }
    }

    fn detail(&self, name: &str) -> super::ChangeDetail {
        change_detail(&resolve(&self.0), name).unwrap_or_else(|| panic!("应能定位 change {name}"))
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// 把 fixture change 目录拷入临时 workspace 的 archive 树（保留目录名）。
fn install_fixture(ws: &TempWs, fixture: &str, archived_name: &str) {
    let source = fixtures_dir().join(fixture);
    let target = ws.0.join("openspec/changes/archive").join(archived_name);
    copy_dir(&source, &target);
}

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

const MULTI_ATTEMPT_WORKFLOW: &str = r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "dev-design", "attempt": 2, "verdict": "pass", "report": "第二次通过", "checklist": [
      { "item": "组件表完整", "pass": true, "evidence": "五组件齐全" }
    ], "backtrack_to": "dev-design", "backtrack_reason": "首轮缺产物区组件" },
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [] },
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [
      { "item": "组件表完整", "pass": false, "evidence": "缺 renderers 职责" }
    ] }
  ]
}"#;

#[test]
fn 同phase多attempt折叠为单站且attempts升序() {
    let ws = TempWs::new("multi-attempt");
    ws.change(
        "openspec/changes/multi",
        &[("workflow.json", MULTI_ATTEMPT_WORKFLOW)],
    );

    let detail = ws.detail("multi");

    // 9 站全量输出，顺序固定
    let phases: Vec<&str> = detail.pipeline.iter().map(|s| s.phase.as_str()).collect();
    assert_eq!(phases, PIPELINE_PHASES.to_vec());

    // dev-design 单站折叠两条 attempt，按 attempt 升序
    let dev_design = &detail.pipeline[1];
    assert_eq!(dev_design.phase, "dev-design");
    let attempts: Vec<Option<u32>> = dev_design.attempts.iter().map(|r| r.attempt).collect();
    assert_eq!(attempts, vec![Some(1), Some(2)]);
    assert_eq!(dev_design.attempts[0].report, "首轮未过");
    assert_eq!(dev_design.attempts[1].report, "第二次通过");
}

#[test]
fn attempt记录携带verdict_report_checklist全量字段() {
    let ws = TempWs::new("record-fields");
    ws.change(
        "openspec/changes/full-record",
        &[("workflow.json", MULTI_ATTEMPT_WORKFLOW)],
    );

    let detail = ws.detail("full-record");
    let pass_record = &detail.pipeline[0].attempts[0]; // proposal
    assert_eq!(pass_record.verdict, crate::model::Verdict::Pass);
    assert_eq!(pass_record.report, "提案通过");
    assert!(pass_record.checklist.is_empty());
    assert!(!pass_record.skipped);
    assert!(!pass_record.stale);
    assert_eq!(pass_record.timestamp, None);
    assert_eq!(pass_record.backtrack_to, None);

    let fail_record = &detail.pipeline[1].attempts[0]; // dev-design attempt 1
    assert_eq!(fail_record.verdict, crate::model::Verdict::Fail);
    assert_eq!(fail_record.checklist.len(), 1);
    assert_eq!(fail_record.checklist[0].item, "组件表完整");
    assert!(!fail_record.checklist[0].pass);
    assert_eq!(fail_record.checklist[0].evidence, "缺 renderers 职责");
}

#[test]
fn v1_b_fixture的backtrack字段随条目暴露() {
    let ws = TempWs::new("v1b-backtrack");
    install_fixture(&ws, "v1-b", "2026-09-17-workflow-file-inventory");

    let detail = ws.detail("2026-09-17-workflow-file-inventory");
    assert_eq!(detail.inventory, Inventory::V1);
    assert_eq!(detail.source, crate::queries::ChangeSource::Archive);
    // v1-b：12 条 eval、1 条含 backtrack_to / backtrack_reason
    let with_backtrack: Vec<_> = detail
        .pipeline
        .iter()
        .flat_map(|station| station.attempts.iter())
        .filter(|record| record.backtrack_to.is_some())
        .collect();
    assert_eq!(with_backtrack.len(), 1);
    assert!(!with_backtrack[0]
        .backtrack_reason
        .as_deref()
        .unwrap_or_default()
        .is_empty());
    // v1 无 active_phase 与 file_log
    assert!(detail.active_phase.is_none());
    assert!(detail.file_log.is_none());
}

#[test]
fn v2_b_fixture暴露active_phase与file_log条目() {
    let ws = TempWs::new("v2b-blocks");
    install_fixture(&ws, "v2-b", "v2-b");

    let detail = ws.detail("v2-b");
    assert_eq!(detail.inventory, Inventory::V2);

    let active = detail.active_phase.expect("v2-b 应有 active_phase");
    assert_eq!(active.phase, "implement");
    assert_eq!(active.attempt, 2);
    assert!(active.start_at.is_some());

    assert_eq!(detail.interrupted.len(), 1);
    assert_eq!(detail.interrupted[0].phase, "test-gen");

    // file_log：三种 op 全量透出
    let file_log = detail.file_log.as_ref().expect("v2 应有 file_log");
    assert_eq!(file_log.len(), 3);
    assert_eq!(file_log[0].op, FileLogOp::Write);
    assert_eq!(file_log[1].op, FileLogOp::Delete);
    assert_eq!(file_log[2].op, FileLogOp::Revert);
    assert_eq!(file_log[0].scope, "workflow");
    assert!(file_log[0].at.is_some());
}

#[test]
fn 未知change名返回none() {
    let ws = TempWs::new("unknown");
    ws.change("openspec/changes/real", &[("proposal.md", "# 提案")]);

    let layout = resolve(&ws.0);
    assert!(change_detail(&layout, "不存在的change").is_none());
    assert!(change_detail(&layout, "").is_none());
}

#[test]
fn v0_change为空流水线加纯文档形态不报错() {
    // 实现语义：v0（无 workflow.json）→ pipeline 为空序列（纯文档形态），
    // 与 v1/v2 的 9 站全量输出相区分
    let ws = TempWs::new("v0-detail");
    ws.change(
        "openspec/changes/old-docs",
        &[("proposal.md", "# v0 提案"), ("design.md", "# v0 设计")],
    );

    let detail = ws.detail("old-docs");
    assert_eq!(detail.inventory, Inventory::V0);
    assert!(detail.pipeline.is_empty(), "v0 无流水线区块");
    assert!(detail.file_log.is_none());
    assert!(detail.active_phase.is_none());
    // 文档产物照常进入产物清单
    assert!(
        detail.artifacts.iter().any(|a| a.kind == "markdown-doc"),
        "v0 仍产出 markdown-doc 产物清单"
    );
}

#[test]
fn v1_change缺file_log时区块留空其余正常() {
    let ws = TempWs::new("v1-detail");
    ws.change(
        "openspec/changes/v1-change",
        &[(
            "workflow.json",
            r#"{ "workflow_type": "requirement", "created": "2026-03-03",
                 "eval": [ { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "OK", "checklist": [] } ] }"#,
        )],
    );

    let detail = ws.detail("v1-change");
    assert_eq!(detail.inventory, Inventory::V1);
    assert!(
        detail.file_log.is_none(),
        "v1 的 file_log 区块为 None（留空降级）"
    );
    assert_eq!(
        detail.pipeline.len(),
        PIPELINE_PHASES.len(),
        "其余区块正常（9 站全量）"
    );
    assert_eq!(detail.pipeline[0].attempts.len(), 1);
    assert_eq!(detail.created.as_deref(), Some("2026-03-03"));
}

#[test]
fn eval未覆盖的phase站点仍在且attempts为空() {
    let ws = TempWs::new("uncovered-phases");
    ws.change(
        "openspec/changes/partial",
        &[
            ("workflow.json", MULTI_ATTEMPT_WORKFLOW),
            ("tasks.md", "- [x] 完成"),
        ],
    );

    let detail = ws.detail("partial");
    let phases: Vec<&str> = detail.pipeline.iter().map(|s| s.phase.as_str()).collect();
    assert_eq!(
        phases,
        PIPELINE_PHASES.to_vec(),
        "顺序固定，不依赖 eval 排列"
    );

    let test_gen = &detail.pipeline[4]; // "test-gen"
    assert_eq!(test_gen.phase, "test-gen");
    assert!(test_gen.attempts.is_empty(), "未覆盖站 attempts 为空序列");

    // 有记录的站不受空站影响
    assert_eq!(detail.pipeline[0].attempts.len(), 1);
    assert_eq!(detail.pipeline[1].attempts.len(), 2);
}

#[test]
fn 无attempt字段的条目归入对应站且不影响其他条目排序() {
    let ws = TempWs::new("no-attempt");
    ws.change(
        "openspec/changes/no-attempt",
        &[(
            "workflow.json",
            r#"{
              "workflow_type": "requirement",
              "eval": [
                { "phase": "implement", "attempt": 5, "verdict": "pass", "report": "高序号", "checklist": [] },
                { "phase": "implement", "verdict": "pass", "report": "无序号（视为 0）", "checklist": [] }
              ]
            }"#,
        )],
    );

    let detail = ws.detail("no-attempt");
    let implement = &detail.pipeline[3]; // "implement"
    let attempts: Vec<Option<u32>> = implement.attempts.iter().map(|r| r.attempt).collect();
    assert_eq!(
        attempts,
        vec![None, Some(5)],
        "缺省 attempt 视为 0 稳定排序"
    );
}

#[test]
fn skipped与stale标记随条目透出() {
    let ws = TempWs::new("flags");
    ws.change(
        "openspec/changes/flags",
        &[(
            "workflow.json",
            r#"{
              "workflow_type": "requirement",
              "eval": [
                { "phase": "test-gen", "attempt": 1, "verdict": "pass", "report": "跳过项", "checklist": [], "skipped": true },
                { "phase": "test-execution", "attempt": 1, "verdict": "pass", "report": "过期项", "checklist": [], "stale": true }
              ]
            }"#,
        )],
    );

    let detail = ws.detail("flags");
    // test-gen = 第 5 站（下标 4），test-execution = 第 6 站（下标 5）
    assert!(detail.pipeline[4].attempts[0].skipped);
    assert!(!detail.pipeline[4].attempts[0].stale);
    assert!(detail.pipeline[5].attempts[0].stale);
    assert!(!detail.pipeline[5].attempts[0].skipped);
}
