//! `queries::change_detail` 的单元测试：9 站流水线聚合 + 运行状态 + 产物清单（AC-6）；
//! AttemptRecord 三会话槽位透出（desktop-change-session-visibility）——
//! `From<&PhaseLog>` 直读不派生不回填，旧条目三值均 null 不报错且 wire 三键
//! 恒在场（纯 derive 零字段属性口径不变）。

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

#[test]
fn 线面契约缺省为null且时间戳为iso串() {
    let ws = TempWs::new("wire-shape");
    ws.change(
        "openspec/changes/wire",
        &[(
            "workflow.json",
            r#"{
              "workflow_type": "requirement",
              "created": "2026-09-18",
              "eval": [
                { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "start_at": "2026-09-18T08:00:00Z", "timestamp": "2026-09-18T08:30:00Z" },
                { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [], "backtrack_to": "proposal", "backtrack_reason": "缺组件" }
              ],
              "active_phase": { "phase": "dev-design", "attempt": 2, "start_at": "2026-09-18T09:00:00Z" },
              "interrupted": [ { "phase": "test-gen", "attempt": 1, "start_at": "2026-09-18T07:00:00Z", "end_at": "2026-09-18T07:30:00Z" } ],
              "file_log": [
                { "op": "write", "scope": "workflow", "attempt": 2, "path": "a.md", "at": "2026-09-18T07:00:00Z" },
                { "op": "delete", "scope": "workflow", "path": "b.md" }
              ]
            }"#,
        )],
    );

    let detail = ws.detail("wire");
    let value = serde_json::to_value(&detail).expect("线面序列化应成功");

    // 缺省 → null（golden 契约：线面与数据源形态一致，不省键）
    assert_eq!(
        value["pipeline"][1]["attempts"][0]["startAt"],
        serde_json::Value::Null
    );
    assert_eq!(
        value["pipeline"][1]["attempts"][0]["timestamp"],
        serde_json::Value::Null
    );
    assert_eq!(value["fileLog"][1]["attempt"], serde_json::Value::Null);
    assert_eq!(value["fileLog"][1]["at"], serde_json::Value::Null);

    // 有值时间戳 → ISO 串原样往返（含 AttemptRecord，修正前的组件数组伪影不再出现）
    assert_eq!(value["activePhase"]["startAt"], "2026-09-18T09:00:00Z");
    assert_eq!(value["fileLog"][0]["at"], "2026-09-18T07:00:00Z");
    assert_eq!(
        value["pipeline"][0]["attempts"][0]["timestamp"], "2026-09-18T08:30:00Z",
        "AttemptRecord 时间戳应为 ISO 串（非裸 OffsetDateTime 组件数组）"
    );

    // v1 形态：无 active_phase / file_log 的代际，缺省为 null
    let ws_v1 = TempWs::new("wire-v1");
    ws_v1.change(
        "openspec/changes/wire-v1",
        &[(
            "workflow.json",
            r#"{ "workflow_type": "requirement", "eval": [] }"#,
        )],
    );
    let v1 = ws_v1.detail("wire-v1");
    let v1_value = serde_json::to_value(&v1).expect("线面序列化应成功");
    assert_eq!(v1_value["activePhase"], serde_json::Value::Null);
    assert_eq!(v1_value["fileLog"], serde_json::Value::Null);
}

// ---------------------------------------------------------------------------
// AttemptRecord 三会话槽位透出（desktop-change-session-visibility，AC-6）：
// `From<&PhaseLog>` 直读透出，无槽位三值 null 不报错，wire 三键恒在场
// ---------------------------------------------------------------------------

/// 三形态条目 fixture：三槽位齐全 pass 条目 / 仅 executor 槽位 fail 升格形态 /
/// 无槽位键旧形态对照。
const SLOTS_WORKFLOW: &str = r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "implement", "attempt": 2, "verdict": "pass", "report": "三槽位齐全", "checklist": [], "executor_session_id": "ses-1-1727740000001", "evaluator_session_id": "ses-2-1727740000002", "decision_session_id": "ses-3-1727740000003" },
    { "phase": "implement", "attempt": 1, "verdict": "fail", "report": "仅 executor 槽位（升格形态）", "checklist": [], "executor_session_id": "ses-4-1727740000004" },
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "无槽位键旧形态", "checklist": [] }
  ]
}"#;

#[test]
fn 三槽位键在场时attempt记录逐字直读透出() {
    let ws = TempWs::new("slots-full");
    ws.change(
        "openspec/changes/slots",
        &[("workflow.json", SLOTS_WORKFLOW)],
    );

    let detail = ws.detail("slots");
    let implement = &detail.pipeline[3]; // "implement"
    assert_eq!(implement.attempts.len(), 2);

    // attempt 2（升序后末位）：三字段逐字直读（From 直读不派生不回填）
    let full = &implement.attempts[1];
    assert_eq!(full.attempt, Some(2));
    assert_eq!(
        full.executor_session_id.as_deref(),
        Some("ses-1-1727740000001"),
        "值与条目记录一致"
    );
    assert_eq!(
        full.evaluator_session_id.as_deref(),
        Some("ses-2-1727740000002")
    );
    assert_eq!(
        full.decision_session_id.as_deref(),
        Some("ses-3-1727740000003")
    );

    // attempt 1（升格形态）：仅 executor 承接，evaluator / decision null（缺字段
    // 降级 null 不报错）
    let partial = &implement.attempts[0];
    assert_eq!(
        partial.executor_session_id.as_deref(),
        Some("ses-4-1727740000004")
    );
    assert_eq!(partial.evaluator_session_id, None);
    assert_eq!(partial.decision_session_id, None);
}

#[test]
fn 旧条目无槽位键投影三值均null且wire三键恒在场() {
    let ws = TempWs::new("slots-legacy");
    ws.change(
        "openspec/changes/legacy",
        &[("workflow.json", SLOTS_WORKFLOW)],
    );

    let detail = ws.detail("legacy");
    let value = serde_json::to_value(&detail).expect("线面序列化应成功");

    // 旧形态条目（proposal）：三字段均 None 不报错
    let proposal = &detail.pipeline[0].attempts[0];
    assert_eq!(proposal.executor_session_id, None);
    assert_eq!(proposal.evaluator_session_id, None);
    assert_eq!(proposal.decision_session_id, None);

    // wire 三键恒在场、值 null 不省略（golden 线面「缺省字段 null」契约；纯
    // derive 零字段属性口径不破）
    let record = &value["pipeline"][0]["attempts"][0];
    for key in [
        "executorSessionId",
        "evaluatorSessionId",
        "decisionSessionId",
    ] {
        assert_eq!(
            record.get(key),
            Some(&serde_json::Value::Null),
            "wire 键 {key} 恒在场（null 不省略）"
        );
    }

    // 有值条目 wire 键逐字透出（camelCase 线面）
    let full = &value["pipeline"][3]["attempts"][1];
    assert_eq!(
        full["executorSessionId"],
        serde_json::json!("ses-1-1727740000001")
    );
    assert_eq!(
        full["evaluatorSessionId"],
        serde_json::json!("ses-2-1727740000002")
    );
    assert_eq!(
        full["decisionSessionId"],
        serde_json::json!("ses-3-1727740000003")
    );
}

// ---------------------------------------------------------------------------
// 线面 interrupted 键删除（desktop-drawer-session-column，AC-6）：磁盘模型
// 停提取 `interrupted` 后 detail 聚合投影不再携带该键（AC-5「存量留档不出图」
// 的上游保证——detail 不再携带则转换层无从收集）；存量文件解析照常
// ---------------------------------------------------------------------------

/// 存量含 `interrupted[]` 的 v2 形态（磁盘 `interrupted[]` 键保留在场——
/// fixture 形状实证，proposal「不要修改」；键由 serde 未知字段忽略承接）。
const LEGACY_INTERRUPTED_WORKFLOW: &str = r#"{
  "workflow_type": "requirement",
  "created": "2026-09-01",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [] }
  ],
  "active_phase": { "phase": "implement", "attempt": 1, "start_at": "2026-09-01T09:00:00Z" },
  "interrupted": [
    { "phase": "test-gen", "attempt": 1, "start_at": "2026-09-01T07:00:00Z", "end_at": "2026-09-01T07:30:00Z" }
  ],
  "file_log": [
    { "op": "write", "scope": "workflow", "attempt": 1, "path": "a.md", "at": "2026-09-01T08:00:00Z" }
  ]
}"#;

#[test]
fn 含interrupted的存量文件detail照常聚合且wire无interrupted键() {
    let ws = TempWs::new("detail-legacy-interrupted");
    ws.change(
        "openspec/changes/legacy-interrupted",
        &[
            ("workflow.json", LEGACY_INTERRUPTED_WORKFLOW),
            ("tasks.md", "- [x] 任务"),
        ],
    );

    // change_detail 返回 Some，聚合投影逐字段照常
    let detail = ws.detail("legacy-interrupted");
    assert_eq!(detail.name, "legacy-interrupted");
    assert!(!detail.unparsable);
    assert_eq!(detail.created.as_deref(), Some("2026-09-01"));

    // pipeline 9 站聚合照常（proposal 站一条 pass 记录）
    let phases: Vec<&str> = detail.pipeline.iter().map(|s| s.phase.as_str()).collect();
    assert_eq!(phases, PIPELINE_PHASES.to_vec());
    assert_eq!(detail.pipeline[0].attempts.len(), 1);
    assert_eq!(detail.pipeline[0].attempts[0].report, "提案通过");

    // active_phase / fileLog 投影照常
    let active = detail.active_phase.as_ref().expect("应有 active_phase");
    assert_eq!(active.phase, "implement");
    assert_eq!(active.attempt, 1);
    assert_eq!(active.start_at.as_deref(), Some("2026-09-01T09:00:00Z"));
    let file_log = detail.file_log.as_ref().expect("v2 应有 file_log");
    assert_eq!(file_log.len(), 1);
    assert_eq!(file_log[0].path, "a.md");

    // 序列化 wire 无 interrupted 键（线面删除的直接证据）
    let value = serde_json::to_value(&detail).expect("线面序列化应成功");
    assert!(
        value.get("interrupted").is_none(),
        "detail 线面不再携带 interrupted 键: {value}"
    );
}

#[test]
fn 含interrupted且eval条目损坏的存量文件宽松降级后照常聚合() {
    // 一条 eval 条目损坏（checklist 非数组）：宽松解析跳过该条，其余照常聚合
    // ——停提取 interrupted 不改变容错语义
    let ws = TempWs::new("detail-legacy-corrupted");
    ws.change(
        "openspec/changes/legacy-corrupted",
        &[(
            "workflow.json",
            r#"{
              "workflow_type": "requirement",
              "eval": [
                { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "完好", "checklist": [] },
                { "phase": "dev-design", "attempt": 1, "verdict": "pass", "report": "损坏", "checklist": "不是数组" }
              ],
              "interrupted": [ { "phase": "test-gen", "attempt": 1, "start_at": null, "end_at": null } ]
            }"#,
            )],
    );

    let detail = ws.detail("legacy-corrupted");
    assert!(!detail.unparsable, "单条损坏不整体降级");
    // proposal 站保留完好条目，dev-design 站损坏条目被跳过
    assert_eq!(detail.pipeline[0].attempts.len(), 1);
    assert!(
        detail.pipeline[1].attempts.is_empty(),
        "损坏条目跳过后站内无记录"
    );

    let value = serde_json::to_value(&detail).expect("线面序列化应成功");
    assert!(
        value.get("interrupted").is_none(),
        "容错路径同样不携带 interrupted 键"
    );
}
