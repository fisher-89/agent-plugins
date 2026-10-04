//! `write::decision_log` 的单元测试（test-design「decision_log.rs ->
//! decision_log_test.rs」节）：决策会话槽位挂账——定位该相位最新 eval 条目
//!（与 backtrack 同锚定：`latest_entry_index` + `entry_phase`，timestamp 降序
//! 取首、stale 不出局），raw 定点改写 `decision_session_id` 键；幂等覆写
//!（同值二连写 workflow.json 字节逐位不变——ask 续轮同会话同值重挂的写面
//! 底座）；无条目显式 `Err` 且零写入（不新增条目、不做表位校验——D6 纪律）。
//!
//! Mock策略：无进程边界 mock（fs 真实组合）——TempWs tempdir 真盘 fixture
//! 真实读写（`foundation::layout::resolve` 真实组合）；「零写入」以调用前后
//! 字节比对断言（phase_log_test 装置先例）。

use std::fs;
use std::path::PathBuf;

use foundation::layout::resolve;

use super::decision_log::{decision_log, DecisionLogOutcome};

/// 临时 workspace 根 RAII（沿 backtrack_test 装置先例）。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-decision-log-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn change(&self, name: &str, workflow_json: impl AsRef<str>) {
        let dir = self.0.join("openspec/changes").join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        fs::write(dir.join("workflow.json"), workflow_json.as_ref())
            .expect("写 workflow.json 失败");
    }

    fn workflow_json_bytes(&self, name: &str) -> Vec<u8> {
        fs::read(
            self.0
                .join("openspec/changes")
                .join(name)
                .join("workflow.json"),
        )
        .expect("读 workflow.json 失败")
    }

    fn log(&self, name: &str, phase: &str, session_id: &str) -> Result<DecisionLogOutcome, String> {
        let layout = resolve(&self.0);
        decision_log(&layout, name, phase, session_id)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn parse_of(ws: &TempWs, name: &str) -> serde_json::Value {
    let text = String::from_utf8(ws.workflow_json_bytes(name)).expect("workflow.json 应为 UTF-8");
    serde_json::from_str(&text).expect("写回应可再解析")
}

/// 条目的规范化字节（键序规范化后的逐字节比对面——更早条目「逐字节不动」
/// 以值域全等承载，写面 pretty 重排不改变条目内容）。
fn canonical(entry: &serde_json::Value) -> String {
    serde_json::to_string(entry).expect("条目序列化应成功")
}

const CHANGE: &str = "demo-change";

// ---------------------------------------------------------------------------
// 正向：最新条目定点挂账 / outcome 形状 / 幂等覆写
// ---------------------------------------------------------------------------

/// 同相位多条目挂账：`latest_entry_index` 锚定该相位 timestamp 最新条目（跨
/// 相位不串锚——全局最新为 proposal 条目也不影响），更早条目逐字节不动。
#[test]
fn 同相位多条目挂账锚定最新条目且更早条目逐字节不动() {
    let ws = TempWs::new("latest-anchor");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:20:00Z" },
    { "phase": "dev-design", "attempt": 2, "verdict": "fail", "report": "次轮未过", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" }
  ]
}"#,
    );
    let before = parse_of(&ws, CHANGE);
    let before_entries: Vec<String> = before["eval"]
        .as_array()
        .expect("eval 数组")
        .iter()
        .map(canonical)
        .collect();

    let outcome = ws
        .log(CHANGE, "dev-design", "ses-decision-1")
        .expect("挂账应成功");

    assert_eq!(
        outcome,
        DecisionLogOutcome {
            phase: "dev-design".to_owned(),
        },
        "outcome {{ phase }} 逐字段（沿 PhaseLogOutcome / BacktrackOutcome 惯例）"
    );

    let doc = parse_of(&ws, CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(
        eval.len(),
        3,
        "挂账不新增条目（amend 语义——decision 会话产生于 fail 条目落账之后）"
    );
    // 锚定该相位 timestamp 最新条目（idx 2，08:10；idx 1 的 proposal 全局更新不串锚）
    assert_eq!(
        eval[2]["decision_session_id"],
        serde_json::json!("ses-decision-1"),
        "最新条目定点改写"
    );
    assert!(
        eval[0].get("decision_session_id").is_none(),
        "更早条目不携带挂账键（逐字节不动）"
    );
    assert!(
        eval[1].get("decision_session_id").is_none(),
        "他相位条目零触碰（跨相位不串锚）"
    );
    let after_entries: Vec<String> = eval.iter().take(2).map(canonical).collect();
    assert_eq!(
        before_entries[0], after_entries[0],
        "更早 dev-design 条目逐字节不动"
    );
    assert_eq!(
        before_entries[1], after_entries[1],
        "proposal 条目逐字节不动"
    );
}

/// 挂账幂等：同值二连写后 workflow.json 字节逐位不变（ask 续轮同会话同值
/// 重挂的写面底座——D6）。
#[test]
fn 挂账幂等同值二连写字节逐位不变() {
    let ws = TempWs::new("idempotent");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "implement", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" }
  ]
}"#,
    );

    ws.log(CHANGE, "implement", "ses-decision-9")
        .expect("首次挂账应成功");
    let after_first = ws.workflow_json_bytes(CHANGE);

    ws.log(CHANGE, "implement", "ses-decision-9")
        .expect("同值重挂应成功");
    let after_second = ws.workflow_json_bytes(CHANGE);

    assert_eq!(
        after_first, after_second,
        "同值二连写字节逐位不变（幂等覆写）"
    );
    assert_eq!(
        parse_of(&ws, CHANGE)["eval"][0]["decision_session_id"],
        serde_json::json!("ses-decision-9")
    );
}

// ---------------------------------------------------------------------------
// 边界：stale 条目仍被锚定 / 覆写旧值 / 其余内容保形
// ---------------------------------------------------------------------------

/// 最新条目为 stale 条目仍被锚定改写（同 backtrack 锚定——stale 不出局）；
/// 条目已携占位 `decision_session_id` 时覆写旧值以新值生效。
#[test]
fn stale条目仍被锚定改写且覆写旧值以新值生效() {
    let ws = TempWs::new("stale-anchor");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "dev-design", "attempt": 2, "verdict": "fail", "report": "次轮未过", "checklist": [], "stale": true, "decision_session_id": "ses-placeholder", "timestamp": "2026-10-01T08:10:00Z" }
  ]
}"#,
    );

    ws.log(CHANGE, "dev-design", "ses-decision-new")
        .expect("stale 条目锚定挂账应成功");

    let doc = parse_of(&ws, CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(
        eval[1]["decision_session_id"],
        serde_json::json!("ses-decision-new"),
        "最新（stale）条目被锚定覆写，占位值让位新值"
    );
    assert_eq!(
        eval[1]["stale"],
        serde_json::json!(true),
        "stale 标记原样保留（只改槽位键）"
    );
    assert!(
        eval[0].get("decision_session_id").is_none(),
        "更早条目零触碰"
    );
}

/// 挂账保形：既有 `executor_session_id` / `evaluator_session_id` 槽位、
/// `backtrack_to` / `backtrack_reason`、checklist、未知键逐字节不动；pretty
/// 写回可再读（2 空格缩进 + 尾换行）；file_log 零触碰。
#[test]
fn 挂账保形_既有槽位与回溯键与未知键零触碰_file_log零触碰_pretty可再读() {
    let ws = TempWs::new("preserve");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "created": "2026-03-03",
  "custom_note": "保留我",
  "file_log": [
    { "op": "write", "scope": "workflow", "path": "a.md", "at": "2026-10-01T07:00:00Z" }
  ],
  "eval": [
    { "phase": "implement", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [
      { "item": "写面保形", "pass": false, "evidence": "custom_note 丢失" }
    ], "executor_session_id": "ses-exec-1", "evaluator_session_id": "ses-eval-1", "backtrack_to": "dev-design", "backtrack_reason": "设计返工", "legacy_extra": "条目级保留", "timestamp": "2026-10-01T08:00:00Z" }
  ]
}"#,
    );
    let before = parse_of(&ws, CHANGE);
    let before_file_log = before["file_log"].clone();
    let before_others: Vec<String> = before["eval"].as_array().expect("eval 数组")[0]
        .as_object()
        .expect("条目对象")
        .iter()
        .filter(|(key, _)| key.as_str() != "decision_session_id")
        .map(|(key, value)| format!("{key}={}", canonical(value)))
        .collect();

    ws.log(CHANGE, "implement", "ses-decision-1")
        .expect("挂账应成功");

    let doc = parse_of(&ws, CHANGE);
    assert_eq!(
        doc["custom_note"],
        serde_json::json!("保留我"),
        "顶层未知字段保形"
    );
    assert_eq!(doc["created"], serde_json::json!("2026-03-03"));
    assert_eq!(
        doc["file_log"], before_file_log,
        "file_log 逐字节不变（写面零触点——AC-3）"
    );

    let entry = &doc["eval"][0];
    let after_others: Vec<String> = entry
        .as_object()
        .expect("条目对象")
        .iter()
        .filter(|(key, _)| key.as_str() != "decision_session_id")
        .map(|(key, value)| format!("{key}={}", canonical(value)))
        .collect();
    assert_eq!(
        before_others, after_others,
        "除 decision_session_id 外条目键逐字节不动（既有槽位 / 回溯键 / checklist / 未知键）"
    );
    assert_eq!(
        entry["executor_session_id"],
        serde_json::json!("ses-exec-1")
    );
    assert_eq!(
        entry["evaluator_session_id"],
        serde_json::json!("ses-eval-1")
    );
    assert_eq!(entry["backtrack_to"], serde_json::json!("dev-design"));
    assert_eq!(entry["backtrack_reason"], serde_json::json!("设计返工"));

    // pretty 写回可再读：2 空格缩进 + 尾换行
    let text = String::from_utf8(ws.workflow_json_bytes(CHANGE)).expect("UTF-8");
    assert!(
        text.contains("\n  \"") && text.ends_with('\n'),
        "pretty 形态"
    );
    serde_json::from_str::<serde_json::Value>(&text).expect("写回应可再解析");
}

// ---------------------------------------------------------------------------
// 异常：该相位无 eval 条目 / change 不存在（全部零写入）
// ---------------------------------------------------------------------------

/// 该相位无 eval 条目：显式 `Err` 且 workflow.json 字节零变化（不新增条目、
/// 不做表位校验——D6 纪律；表外相位名同样按「无条目」拒绝，不做表位白名单）。
#[test]
fn 该相位无eval条目拒绝且字节零变化() {
    let ws = TempWs::new("no-entry");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" }
  ]
}"#,
    );
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .log(CHANGE, "dev-design", "ses-decision-1")
        .expect_err("该相位无评估条目必须 Err");
    assert!(
        err.contains("dev-design") && err.contains("评估条目"),
        "错误显式携带相位与无条目记因，实际: {err}"
    );
    assert_eq!(
        ws.workflow_json_bytes(CHANGE),
        before,
        "拒绝零写入（不新增条目）"
    );

    // 空表 eval 数组在场的全新 change 同样拒绝
    let empty = TempWs::new("no-entry-empty-eval");
    empty.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);
    let before = empty.workflow_json_bytes(CHANGE);
    let err = empty
        .log(CHANGE, "proposal", "ses-decision-1")
        .expect_err("空 eval 数组必须 Err");
    assert!(
        err.contains("proposal") && err.contains("评估条目"),
        "记因: {err}"
    );
    assert_eq!(empty.workflow_json_bytes(CHANGE), before, "零写入");
}

/// change 不存在（无 workflow.json）：显式 `Err` 零写入（无目录可写）。
#[test]
fn change不存在显式err零写入() {
    let ws = TempWs::new("missing-change");

    let err = ws
        .log("不存在的-change", "dev-design", "ses-decision-1")
        .expect_err("未知 change 必须 Err");
    assert!(!err.is_empty(), "Err 显式，实际: {err}");
    assert!(
        !ws.0.join("openspec/changes/不存在的-change").exists(),
        "零写入（不无中生有 change 目录）"
    );
}
