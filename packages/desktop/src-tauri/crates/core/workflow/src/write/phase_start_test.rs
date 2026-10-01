//! `write::phase_start` 的单元测试（test-design「phase_start.rs ->
//! phase_start_test.rs」节）：`active_phase` 定点写入、attempt 自既有 eval
//! 条目数推导（重入即新一轮计时）、非法相位 / 非 requirement 拒绝零写入、
//! 未知字段保形（W2）、file_log 零触碰（AC-3 写面半边）、pretty 写回可再读
//!（W3 zod 兼容 fixture 对照口径）。
//!
//! Mock策略：无进程边界 mock（fs 真实组合）——tempdir 真实 change fixture
//! 真盘（合法 / 带 legacy 字段 / 损坏三族）；「零写入」以调用前后字节比对
//! 断言。

use std::fs;
use std::path::PathBuf;

use foundation::layout::resolve;

use super::phase_start::{phase_start, PhaseStartOutcome};

/// 临时 workspace 根 RAII（沿 detail_test 装置先例）。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-phase-start-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn change(&self, name: &str, workflow_json: &str) {
        let dir = self.0.join("openspec/changes").join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        fs::write(dir.join("workflow.json"), workflow_json).expect("写 workflow.json 失败");
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

    fn start(&self, name: &str, phase: &str) -> Result<PhaseStartOutcome, String> {
        let layout = resolve(&self.0);
        phase_start(&layout, name, phase)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 解析当前 workflow.json（写回面断言用）。
fn parse_of(ws: &TempWs, name: &str) -> serde_json::Value {
    let text = String::from_utf8(ws.workflow_json_bytes(name)).expect("workflow.json 应为 UTF-8");
    serde_json::from_str(&text).expect("写回文件应可再解析")
}

const CHANGE: &str = "demo-change";

// ---------------------------------------------------------------------------
// 正向：开相位落盘 / attempt 递增
// ---------------------------------------------------------------------------

/// 开相位落盘：workflow.json active_phase 定点写入 {phase, attempt, start_at}，
/// outcome 三字段与落盘一致（AC-9 attempt 计时对照面）。
#[test]
fn 开相位落盘active_phase三字段与outcome一致() {
    let ws = TempWs::new("start-basic");
    ws.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

    let outcome = ws.start(CHANGE, "proposal").expect("开相位应成功");

    assert_eq!(outcome.phase, "proposal");
    assert_eq!(outcome.attempt, 1, "无既有条目 → attempt 1");

    let doc = parse_of(&ws, CHANGE);
    let active = &doc["active_phase"];
    assert_eq!(active["phase"], serde_json::json!("proposal"));
    assert_eq!(active["attempt"], serde_json::json!(1));
    let start_at = active["start_at"].as_str().expect("start_at 应为 ISO 串");
    assert!(
        start_at.starts_with("20"),
        "start_at 应为 RFC3339 形态，实际: {start_at}"
    );

    // outcome.start_at 与落盘串可互证（RFC3339 再解析等值）
    let parsed =
        time::OffsetDateTime::parse(start_at, &time::format_description::well_known::Rfc3339)
            .expect("落盘 start_at 应可 RFC3339 解析");
    assert_eq!(parsed, outcome.start_at, "outcome 三字段与落盘一致");
}

/// attempt 自既有值递增：既有 eval 条目 2 条（active_phase.attempt=2 残留）→
/// start 后 attempt=3（推导 = 该相位既有条目数 + 1，与插件 phase-start 语义
/// 一致——重试自然递增）。
#[test]
fn attempt自既有eval条目数递增() {
    let ws = TempWs::new("attempt-increment");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" },
    { "phase": "dev-design", "attempt": 2, "verdict": "fail", "report": "次轮未过", "checklist": [], "timestamp": "2026-10-01T08:20:00Z" }
  ],
  "active_phase": { "phase": "dev-design", "attempt": 2, "start_at": "2026-10-01T08:15:00Z" }
}"#,
    );

    let outcome = ws.start(CHANGE, "dev-design").expect("开相位应成功");

    assert_eq!(outcome.attempt, 3, "既有 2 条 + 1（重试自然递增）");
    let doc = parse_of(&ws, CHANGE);
    assert_eq!(doc["active_phase"]["attempt"], serde_json::json!(3));
    assert_eq!(
        doc["active_phase"]["phase"],
        serde_json::json!("dev-design")
    );
}

// ---------------------------------------------------------------------------
// 边界：重入新一轮计时 / 未知字段保形 / file_log 零触碰 / pretty 写回可再读
// ---------------------------------------------------------------------------

/// phase_start 重入新一轮计时：同相位重复 start（重试节奏：start → 落账 →
/// 再 start）attempt 再递增、start_at 刷新（重入即新一轮计时）。
#[test]
fn 重入开相位attempt再递增且start_at刷新() {
    let ws = TempWs::new("restart");
    ws.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

    let first = ws.start(CHANGE, "implement").expect("首次 start 应成功");
    assert_eq!(first.attempt, 1);

    // 模拟 run 期间 phase-log 落账一条 fail（重试节奏中的落账步）
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "implement", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" }
  ]
}"#,
    );
    std::thread::sleep(std::time::Duration::from_millis(5));

    let second = ws.start(CHANGE, "implement").expect("重入 start 应成功");
    assert_eq!(second.attempt, 2, "重入 attempt 再递增");
    assert!(
        second.start_at >= first.start_at,
        "start_at 刷新（重入即新一轮计时）"
    );

    let doc = parse_of(&ws, CHANGE);
    assert_eq!(doc["active_phase"]["attempt"], serde_json::json!(2));
}

/// 未知字段保形：预置未知 / legacy 字段的 fixture 写后原样保留（W2 raw Value
/// 定点改写——serde↔zod 兼容最强形态，AC-9）。
#[test]
fn 未知字段与legacy字段写后原样保留() {
    let ws = TempWs::new("preserve-fields");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "created": "2026-03-03",
  "custom_note": "保留我",
  "legacy_files_bucket": { "files": ["a.rs"], "count": 2 },
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "OK", "checklist": [], "legacy_extra": "条目级保留" }
  ]
}"#,
    );

    ws.start(CHANGE, "dev-design").expect("开相位应成功");

    let doc = parse_of(&ws, CHANGE);
    assert_eq!(doc["custom_note"], serde_json::json!("保留我"));
    assert_eq!(
        doc["legacy_files_bucket"],
        serde_json::json!({ "files": ["a.rs"], "count": 2 }),
        "legacy 字段原样保留"
    );
    assert_eq!(doc["created"], serde_json::json!("2026-03-03"));
    assert_eq!(
        doc["eval"][0]["legacy_extra"],
        serde_json::json!("条目级保留"),
        "既有 eval 条目零触碰（条目级未知字段保留）"
    );
}

/// file_log 零触碰：既有 file_log 条目写前后逐字节不变（AC-3 写面半边——
/// desktop run 零 file_log 新增）。
#[test]
fn file_log既有条目写前后零触碰() {
    let ws = TempWs::new("file-log-untouched");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [],
  "file_log": [
    { "op": "write", "scope": "workflow", "attempt": 2, "path": "a.md", "at": "2026-10-01T07:00:00Z" },
    { "op": "delete", "scope": "workflow", "path": "b.md" }
  ]
}"#,
    );

    let before = parse_of(&ws, CHANGE)["file_log"].clone();

    ws.start(CHANGE, "proposal").expect("开相位应成功");

    let after = parse_of(&ws, CHANGE);
    assert_eq!(
        after["file_log"], before,
        "file_log 逐字节不变（写面零触点）"
    );
    assert_eq!(
        after["eval"].as_array().map(Vec::len),
        Some(0),
        "eval 亦零新增（phase_start 不落账）"
    );
}

/// pretty 写回可再读：写回文件 serde 再解析成功、键集与字段形状不变（W3
/// zod 兼容 fixture 对照口径：插件直跑形态 fixture 为基准样本）。
#[test]
fn pretty写回可再读且键集形状不变() {
    let ws = TempWs::new("pretty-roundtrip");
    ws.change(
        CHANGE,
        r#"{ "workflow_type": "requirement", "created": "2026-03-03", "eval": [] }"#,
    );
    let before: serde_json::Value =
        serde_json::from_str(&String::from_utf8(ws.workflow_json_bytes(CHANGE)).expect("UTF-8"))
            .expect("fixture 应合法");

    ws.start(CHANGE, "test-gen").expect("开相位应成功");

    let text = String::from_utf8(ws.workflow_json_bytes(CHANGE)).expect("workflow.json 应为 UTF-8");
    // pretty 形态：2 空格缩进 + 尾换行（与插件 writeEvalJson 输出形态一致）
    assert!(text.contains("\n  \""), "应为 2 空格缩进 pretty 形态");
    assert!(text.ends_with('\n'), "应以尾换行收口");

    let after: serde_json::Value = serde_json::from_str(&text).expect("写回应可再解析");
    // 键集不变（active_phase 新增 / 覆写除外）
    let mut before_keys: Vec<&str> = before
        .as_object()
        .expect("顶层对象")
        .keys()
        .map(String::as_str)
        .collect();
    before_keys.push("active_phase");
    before_keys.sort_unstable();
    let mut after_keys: Vec<&str> = after
        .as_object()
        .expect("顶层对象")
        .keys()
        .map(String::as_str)
        .collect();
    after_keys.sort_unstable();
    assert_eq!(after_keys, before_keys, "键集与字段形状不变");

    // 既有字段值不变
    assert_eq!(after["workflow_type"], before["workflow_type"]);
    assert_eq!(after["created"], before["created"]);
    assert_eq!(after["eval"], before["eval"]);
}

// ---------------------------------------------------------------------------
// 异常：非法相位 / 非 requirement / change 不存在（全部零写入）
// ---------------------------------------------------------------------------

/// phase_start 非法相位拒绝：相位不在 requirement 表 → Err 且文件零变更。
#[test]
fn 非法相位拒绝且文件零变更() {
    let ws = TempWs::new("bad-phase");
    ws.change(
        CHANGE,
        r#"{ "workflow_type": "requirement", "eval": [], "custom_note": "在场" }"#,
    );
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .start(CHANGE, "不存在的相位")
        .expect_err("非法相位应 Err");
    assert!(
        err.contains("不存在的相位") && err.contains("requirement"),
        "错误显式携带相位与 workflow_type 语境，实际: {err}"
    );
    assert_eq!(
        ws.workflow_json_bytes(CHANGE),
        before,
        "拒绝零写入（workflow.json 原文不动）"
    );
}

/// phase_start workflow_type 非 requirement：Err（W8 分层出口）。
#[test]
fn workflow_type非requirement拒绝() {
    let ws = TempWs::new("bad-type");
    ws.change(CHANGE, r#"{ "workflow_type": "test-only", "eval": [] }"#);
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .start(CHANGE, "proposal")
        .expect_err("非 requirement 应 Err");
    assert!(err.contains("test-only"), "W8 分层出口记因，实际: {err}");
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "拒绝零写入");
}

/// phase_start change 不存在：Err 显式。
#[test]
fn change不存在显式err() {
    let ws = TempWs::new("missing-change");

    let err = ws
        .start("不存在的-change", "proposal")
        .expect_err("未知 change 应 Err");
    assert!(
        !err.is_empty(),
        "Err 显式（文件缺失 / 无法解析人读文案），实际: {err}"
    );
}
