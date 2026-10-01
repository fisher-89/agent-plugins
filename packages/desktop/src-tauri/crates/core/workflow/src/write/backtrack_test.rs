//! `write::backtrack` 的单元测试（test-design「backtrack.rs ->
//! backtrack_test.rs」节）：白名单二次校验（越权 Err 不写——坏决议损坏不了
//! 状态）、reason 长度门、最新条目标记 backtrack_to / backtrack_reason、stale
//! 标记与相位表依赖向后传播。
//!
//! Mock策略：无进程边界 mock（fs 真实组合）——tempdir 真实 change fixture
//! 真盘；「越权 / 超长零写入」以调用前后字节比对断言（沿原工具侧字节比对
//! 装置口径迁入写面）。

use std::fs;
use std::path::PathBuf;

use foundation::layout::resolve;

use super::backtrack::{backtrack, BacktrackInput};

/// 临时 workspace 根 RAII（沿 detail_test 装置先例）。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-backtrack-test-{}-{}",
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

    fn run(
        &self,
        name: &str,
        input: &BacktrackInput,
    ) -> Result<super::backtrack::BacktrackOutcome, String> {
        let layout = resolve(&self.0);
        backtrack(&layout, name, input)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn parse_of(ws: &TempWs, name: &str) -> serde_json::Value {
    let text = String::from_utf8(ws.workflow_json_bytes(name)).expect("workflow.json 应为 UTF-8");
    serde_json::from_str(&text).expect("应可再解析")
}

/// 全相位 pass 历史 fixture（stale 传播断言的底座；条目按表序）。
fn full_pass_history() -> String {
    let phases = [
        ("proposal", "08:00:00Z"),
        ("dev-design", "08:10:00Z"),
        ("test-design", "08:20:00Z"),
        ("implement", "08:30:00Z"),
        ("test-gen", "08:40:00Z"),
        ("test-execution", "08:50:00Z"),
        ("code-review", "09:00:00Z"),
        ("acceptance", "09:10:00Z"),
    ];
    let entries = phases
        .iter()
        .enumerate()
        .map(|(idx, (phase, ts))| {
            format!(
                r#"
    {{ "phase": "{phase}", "attempt": {}, "verdict": "pass", "report": "{phase} 通过", "checklist": [], "timestamp": "2026-10-01T{ts}" }}"#,
                idx + 1
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{
  "workflow_type": "requirement",
  "eval": [{entries}
  ]
}}"#
    )
}

const CHANGE: &str = "demo-change";

// ---------------------------------------------------------------------------
// 正向：白名单内落盘 / stale 传播 / 表首回跳
// ---------------------------------------------------------------------------

/// backtrack 白名单内落盘：to ∈ allowed → 最新条目标记 backtrack_to/reason、
/// outcome{phase, target}（AC-2 写面执行半边）。
#[test]
fn 白名单内回溯落盘且最新条目带标记() {
    let ws = TempWs::new("in-whitelist");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" },
    { "phase": "dev-design", "attempt": 2, "verdict": "fail", "report": "次轮未过", "checklist": [], "timestamp": "2026-10-01T08:20:00Z" }
  ]
}"#,
    );

    let outcome = ws
        .run(
            CHANGE,
            &BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "proposal".to_owned(),
                reason: "提案缺验收标准".to_owned(),
                allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
            },
        )
        .expect("白名单内回溯应成功");

    assert_eq!(outcome.phase, "dev-design", "outcome 承载 phase 与 target");
    assert_eq!(outcome.target, "proposal");

    let doc = parse_of(&ws, CHANGE);
    // 失败相位的「最新」条目（timestamp 降序）标记 backtrack_to / backtrack_reason
    let latest = &doc["eval"][2];
    assert_eq!(latest["backtrack_to"], serde_json::json!("proposal"));
    assert_eq!(
        latest["backtrack_reason"],
        serde_json::json!("提案缺验收标准")
    );
    // 更早条目不带标记
    assert!(doc["eval"][0].get("backtrack_to").is_none());
    assert!(doc["eval"][1].get("backtrack_to").is_none());
}

/// stale 按依赖向后传播：目标相位最新 pass 条目标 stale、依赖下游相位全部
/// 条目标 stale、更早相位条目不动（AC-9 传播面；白名单=表序前置与传播方向
/// 自洽——proposal 为 dev-design 的前置，绝不被回跳标 stale）。
#[test]
fn stale按依赖向后传播且更早相位不动() {
    let ws = TempWs::new("stale-propagation");
    ws.change(CHANGE, &full_pass_history());

    ws.run(
        CHANGE,
        &BacktrackInput {
            phase: "test-gen".to_owned(),
            to: "dev-design".to_owned(),
            reason: "设计返工".to_owned(),
            allowed: vec![
                "proposal".to_owned(),
                "dev-design".to_owned(),
                "test-design".to_owned(),
                "implement".to_owned(),
                "test-gen".to_owned(),
            ],
        },
    )
    .expect("回溯应成功");

    let doc = parse_of(&ws, CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    let verdict_of = |idx: usize| eval[idx]["stale"].as_bool().unwrap_or(false);

    assert!(verdict_of(1), "目标相位 dev-design 最新 pass 条目标 stale");
    // 依赖下游（test-design → implement → test-gen → test-execution →
    // code-review → acceptance）全部条目标 stale
    for idx in 2..8 {
        assert!(verdict_of(idx), "下游相位条目 {idx} 应标 stale");
    }
    assert!(!verdict_of(0), "更早相位 proposal 条目不动（传播方向自洽）");
}

/// backtrack 目标为表首：全部既有条目按表序标 stale、首相位自身承接重跑
///（proposal 既有 pass 条目亦被标 stale）。
#[test]
fn 目标为表首时全部既有条目标stale() {
    let ws = TempWs::new("target-first");
    ws.change(CHANGE, &full_pass_history());

    ws.run(
        CHANGE,
        &BacktrackInput {
            phase: "implement".to_owned(),
            to: "proposal".to_owned(),
            reason: "需求基线返工".to_owned(),
            allowed: vec![
                "proposal".to_owned(),
                "dev-design".to_owned(),
                "test-design".to_owned(),
                "implement".to_owned(),
            ],
        },
    )
    .expect("回溯应成功");

    let doc = parse_of(&ws, CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(eval.len(), 8);
    for entry in eval {
        assert_eq!(
            entry["stale"],
            serde_json::json!(true),
            "首相位回跳 → 全部既有条目标 stale（含 proposal 自身承接重跑）"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：空白名单 / reason 端点 / 保形与 file_log 零触碰
// ---------------------------------------------------------------------------

/// backtrack 空白名单拒绝：allowed 空数组 + 任何 backtrack → Err（无跳转出口）。
#[test]
fn 空白名单拒绝零写入() {
    let ws = TempWs::new("empty-whitelist");
    ws.change(CHANGE, &full_pass_history());
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .run(
            CHANGE,
            &BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "proposal".to_owned(),
                reason: "任意".to_owned(),
                allowed: Vec::new(),
            },
        )
        .expect_err("空白名单应 Err");

    assert!(
        err.contains("越权") || err.contains("白名单"),
        "记因: {err}"
    );
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "零写入");
}

/// backtrack reason 恰 500 → 成功（≤500 边界含端点）。
#[test]
fn reason恰500字符成功() {
    let ws = TempWs::new("reason-500");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "未过", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" }
  ]
}"#,
    );

    let outcome = ws
        .run(
            CHANGE,
            &BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "proposal".to_owned(),
                reason: "因".repeat(500),
                allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
            },
        )
        .expect("恰 500 字符应成功（边界含端点）");
    assert_eq!(outcome.target, "proposal");
    assert_eq!(
        parse_of(&ws, CHANGE)["eval"][1]["backtrack_reason"]
            .as_str()
            .expect("reason 在场")
            .chars()
            .count(),
        500
    );
}

/// backtrack 保形与 file_log 零触碰：未知字段保形 + file_log 逐字节不变 +
/// pretty 写回可再读（W2/W3/AC-3 口径）。
#[test]
fn 回溯保形_file_log零触碰_pretty可再读() {
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
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "legacy_extra": "条目级保留", "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "未过", "checklist": [] }
  ]
}"#,
    );
    let before_file_log = parse_of(&ws, CHANGE)["file_log"].clone();

    ws.run(
        CHANGE,
        &BacktrackInput {
            phase: "dev-design".to_owned(),
            to: "proposal".to_owned(),
            reason: "设计返工".to_owned(),
            allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
        },
    )
    .expect("回溯应成功");

    let doc = parse_of(&ws, CHANGE);
    assert_eq!(
        doc["custom_note"],
        serde_json::json!("保留我"),
        "未知字段保形"
    );
    assert_eq!(
        doc["eval"][0]["legacy_extra"],
        serde_json::json!("条目级保留"),
        "未被触碰条目的条目级未知字段保留"
    );
    assert_eq!(
        doc["file_log"], before_file_log,
        "file_log 逐字节不变（写面零触点——AC-3）"
    );

    let text = String::from_utf8(ws.workflow_json_bytes(CHANGE)).expect("UTF-8");
    assert!(
        text.contains("\n  \"") && text.ends_with('\n'),
        "pretty 形态"
    );
    serde_json::from_str::<serde_json::Value>(&text).expect("写回应可再解析");
}

// ---------------------------------------------------------------------------
// 异常：越权 / 超长 / 非法目标 / 无 eval 条目（全部零写入）
// ---------------------------------------------------------------------------

/// backtrack 越权拒绝零写入：to ∉ allowed → Err 且 workflow.json 字节零变更
///（写面二次校验兜底——坏决议损坏不了状态，AC-2/AC-9）。
#[test]
fn 越权回溯拒绝且字节零变更() {
    let ws = TempWs::new("overreach");
    ws.change(CHANGE, &full_pass_history());
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .run(
            CHANGE,
            &BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "proposal".to_owned(),
                reason: "任意".to_owned(),
                allowed: vec!["dev-design".to_owned(), "test-design".to_owned()],
            },
        )
        .expect_err("越权目标应 Err");

    assert!(
        err.contains("proposal") && err.contains("越权"),
        "错误显式携带目标与越权记因，实际: {err}"
    );
    assert_eq!(
        ws.workflow_json_bytes(CHANGE),
        before,
        "字节零变更（二次校验兜底）"
    );
}

/// backtrack reason 超长拒绝：501 字符 → Err 且零写入（≤500 约定的写面承载
/// ——decision 解析层对端）。
#[test]
fn reason超长501拒绝零写入() {
    let ws = TempWs::new("reason-501");
    ws.change(CHANGE, &full_pass_history());
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .run(
            CHANGE,
            &BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "proposal".to_owned(),
                reason: "因".repeat(501),
                allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
            },
        )
        .expect_err("501 字符应 Err");

    assert!(
        err.contains("500") && err.contains("501"),
        "错误携带上限与实际值，实际: {err}"
    );
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "零写入");
}

/// backtrack 非法目标相位：to 不在表 → Err 零写入；回溯到未来相位同样 Err
///（双端表位校验——与插件 validatePhaseTarget 同语义）。
#[test]
fn 非法目标相位与未来目标拒绝零写入() {
    let ws = TempWs::new("bad-target");
    ws.change(CHANGE, &full_pass_history());
    let before = ws.workflow_json_bytes(CHANGE);

    // to 不在表（allowed 随行含该目标——walker 预校验 normally 拦下，写面
    // 二次校验兜底直达表位门）
    let err = ws
        .run(
            CHANGE,
            &BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "幽灵相位".to_owned(),
                reason: "任意".to_owned(),
                allowed: vec!["幽灵相位".to_owned(), "dev-design".to_owned()],
            },
        )
        .expect_err("表外目标应 Err");
    assert!(err.contains("幽灵相位"), "记因: {err}");
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "零写入");

    // 回溯到未来相位（target 在 phase 之后）→ Err
    let err = ws
        .run(
            CHANGE,
            &BacktrackInput {
                phase: "proposal".to_owned(),
                to: "implement".to_owned(),
                reason: "任意".to_owned(),
                allowed: vec!["implement".to_owned(), "proposal".to_owned()],
            },
        )
        .expect_err("未来相位应 Err");
    assert!(
        err.contains("未来") || err.contains("不支持"),
        "记因: {err}"
    );
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "零写入");
}

/// backtrack 无 eval 条目拒绝：全新 change 直接 backtrack → Err（无可标记
/// 条目）。
#[test]
fn 无eval条目拒绝() {
    let ws = TempWs::new("no-entries");
    ws.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .run(
            CHANGE,
            &BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "proposal".to_owned(),
                reason: "任意".to_owned(),
                allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
            },
        )
        .expect_err("无评估条目应 Err");

    assert!(
        err.contains("dev-design") && (err.contains("评估") || err.contains("条目")),
        "记因: {err}"
    );
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "零写入");
}
