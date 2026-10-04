//! `write::phase_log` 的单元测试（test-design「phase_log.rs ->
//! phase_log_test.rs」节）：verdict 推导（checklist 全 pass）、skipped 约束、
//! report 长度门、纯追加落账（W9 缺陷修复回归——backtrack 回跳后同相位重评
//! 逐条 append）、start_at 自 `active_phase` 继承、落账后清 `active_phase`；
//! 会话槽位显式在位落账（AC-4）——仅 `Some` 槽位以 raw snake_case 键 insert
//!（`executor_session_id` / `evaluator_session_id` / `decision_session_id`），
//! 缺省槽位不产生键（条目形状与既有形态一致），落账产物经宽松解析面读回
//! 三槽位值逐字还原。
//!
//! Mock策略：无进程边界 mock（fs 真实组合）——tempdir 真实 change fixture
//! 真盘（含 backtrack 后形态 fixture 供缺陷回归行驱动）；「零写入」以调用
//! 前后字节比对断言。
//!
//! 分层注记（写面严格语义，对照插件「非匹配 active_phase 照落不拒」的宽容
//! 形态收紧）：phase_log 门控 workflow_type、表位（相位不在 requirement 表 →
//! Err）、verdict-skipped 约束与 report 长度；开相前置——`active_phase`
//! 缺失或停留他相均显式 `Err`（开相位才可落账，杜绝无主落账与 start_at
//! 无源），匹配时 `start_at` 无条件自 `active_phase` 继承。

use std::fs;
use std::path::PathBuf;

use foundation::layout::resolve;

use super::phase_log::{phase_log, PhaseLogInput};
use crate::model::ChecklistItem;
use crate::parse::{parse_workflow_file, WorkflowFileParse};

/// 临时 workspace 根 RAII（沿 detail_test 装置先例）。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-phase-log-test-{}-{}",
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

    fn log(
        &self,
        name: &str,
        input: &PhaseLogInput,
    ) -> Result<super::phase_log::PhaseLogOutcome, String> {
        let layout = resolve(&self.0);
        phase_log(&layout, name, input)
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

/// pass checklist 行构造。
fn pass_items(n: usize) -> Vec<ChecklistItem> {
    (0..n)
        .map(|idx| ChecklistItem {
            item: format!("检查项{idx}"),
            pass: true,
            evidence: "事实依据".to_owned(),
        })
        .collect()
}

/// 带 fail 项的 checklist。
fn with_fail_item(mut items: Vec<ChecklistItem>) -> Vec<ChecklistItem> {
    items.push(ChecklistItem {
        item: "组件表完整".to_owned(),
        pass: false,
        evidence: "缺 renderers 职责".to_owned(),
    });
    items
}

/// 带 active_phase 的 workflow.json fixture。
fn fixture_with_active_phase(eval: &str, active_phase: Option<&str>) -> String {
    let active = active_phase.unwrap_or("null");
    format!(
        r#"{{
  "workflow_type": "requirement",
  "eval": [{eval}
  ],
  "active_phase": {active}
}}"#
    )
}

/// dev-design 的 active_phase fixture（start_at 在场供继承断言）。
const ACTIVE_DEV_DESIGN: &str =
    r#"{ "phase": "dev-design", "attempt": 2, "start_at": "2026-10-01T08:15:00Z" }"#;

/// test-gen 的 active_phase fixture（skipped 形态用例的开相前置）。
const ACTIVE_TEST_GEN: &str =
    r#"{ "phase": "test-gen", "attempt": 1, "start_at": "2026-10-01T09:00:00Z" }"#;

/// proposal 的 active_phase fixture（保形用例的开相前置）。
const ACTIVE_PROPOSAL: &str =
    r#"{ "phase": "proposal", "attempt": 1, "start_at": "2026-10-01T07:30:00Z" }"#;

const CHANGE: &str = "demo-change";

// ---------------------------------------------------------------------------
// 正向：pass / fail 落账、W9 纯追加回归
// ---------------------------------------------------------------------------

/// phase_log pass 落账：checklist 全 pass → eval 追加条目 verdict=pass、
/// phase/attempt/checklist/skipped/start_at 齐全（start_at 自 active_phase
/// 继承）、落账后 active_phase 清除。
#[test]
fn pass落账条目齐全且active_phase清除() {
    let ws = TempWs::new("pass-log");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );

    let outcome = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "设计与任务拆解齐备".to_owned(),
                checklist: pass_items(2),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect("落账应成功");

    assert_eq!(outcome.phase, "dev-design");
    assert_eq!(outcome.attempt, 1, "无既有条目 → attempt 1");

    let doc = parse_of(&ws, CHANGE);
    let entry = &doc["eval"][0];
    assert_eq!(entry["phase"], serde_json::json!("dev-design"));
    assert_eq!(entry["attempt"], serde_json::json!(1));
    assert_eq!(
        entry["verdict"],
        serde_json::json!("pass"),
        "checklist 全 pass → pass"
    );
    assert_eq!(entry["report"], serde_json::json!("设计与任务拆解齐备"));
    assert_eq!(entry["checklist"].as_array().map(Vec::len), Some(2));
    assert!(
        entry.get("skipped").is_none(),
        "skipped=false 不落扩展字段（与插件 buildEntry 同形态——仅显式在位时写入）"
    );
    assert_eq!(
        entry["start_at"],
        serde_json::json!("2026-10-01T08:15:00Z"),
        "start_at 自匹配的 active_phase 继承"
    );
    assert!(
        entry["timestamp"]
            .as_str()
            .expect("timestamp 在场")
            .starts_with("20"),
        "timestamp 以 ISO 串落盘"
    );
    // 落账后 active_phase 清除（匹配才清）
    assert!(doc["active_phase"].is_null(), "落账后 active_phase 清除");
}

/// phase_log fail 落账：checklist 含 fail 项 → verdict=fail 条目追加（进重试
/// / 决策分叉的输入面——AC-4 升格代写形态同通道）。
#[test]
fn fail落账verdict推导为fail() {
    let ws = TempWs::new("fail-log");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );

    let outcome = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "首轮未过".to_owned(),
                checklist: with_fail_item(pass_items(1)),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect("落账应成功");

    assert_eq!(outcome.attempt, 1);
    let entry = &parse_of(&ws, CHANGE)["eval"][0];
    assert_eq!(entry["verdict"], serde_json::json!("fail"));
    assert_eq!(entry["checklist"][1]["pass"], serde_json::json!(false));
    assert_eq!(
        entry["checklist"][1]["evidence"],
        serde_json::json!("缺 renderers 职责")
    );
}

/// 纯追加缺陷回归（W9）：backtrack 回跳后同相位重评——既有 pass 条目在场仍
/// 追加新条目、attempt=该相位既有条目数+1，不因「相位已存在 pass 条目」短路
/// 跳过（AC-9 缺陷不复发的主回归行）。
#[test]
fn 纯追加回归_既有pass条目在场仍追加新条目() {
    // backtrack 后形态 fixture：proposal pass 在场 + dev-design 曾 pass 但被
    // backtrack 标 stale（回跳重评场景）
    let ws = TempWs::new("append-only");
    ws.change(
        CHANGE,
        fixture_with_active_phase(
            r#"
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "dev-design", "attempt": 1, "verdict": "pass", "report": "首轮已过", "checklist": [], "stale": true, "backtrack_to": "dev-design", "backtrack_reason": "缺产物区组件", "timestamp": "2026-10-01T08:10:00Z" },
    { "phase": "dev-design", "attempt": 2, "verdict": "fail", "report": "重评未过", "checklist": [], "timestamp": "2026-10-01T08:20:00Z" }"#,
            Some(ACTIVE_DEV_DESIGN),
        ),
    );

    let outcome = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "回跳后重评通过".to_owned(),
                checklist: pass_items(2),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect("回跳后重评应照常落账");

    assert_eq!(outcome.attempt, 3, "attempt = 该相位既有条目数 2 + 1");

    let doc = parse_of(&ws, CHANGE);
    assert_eq!(
        doc["eval"].as_array().map(Vec::len),
        Some(4),
        "纯追加：既有 3 条 + 新 1 条（不短路跳过、不覆盖历史）"
    );
    let fresh = &doc["eval"][3];
    assert_eq!(fresh["report"], serde_json::json!("回跳后重评通过"));
    assert_eq!(fresh["verdict"], serde_json::json!("pass"));
    assert_eq!(fresh["attempt"], serde_json::json!(3));
    // 历史条目原样
    assert_eq!(doc["eval"][1]["report"], serde_json::json!("首轮已过"));
    assert_eq!(doc["eval"][1]["stale"], serde_json::json!(true));
}

/// attempt 推导含历史条目：同相位既有条目含 stale / pass / fail 混合历史 →
/// attempt 计数含全部历史条目 +1（不挑食）。
#[test]
fn attempt推导含stale_pass_fail混合历史() {
    let ws = TempWs::new("attempt-history");
    ws.change(
        CHANGE,
        fixture_with_active_phase(
            r#"
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "一", "checklist": [] },
    { "phase": "dev-design", "attempt": 2, "verdict": "pass", "report": "二", "checklist": [], "stale": true },
    { "phase": "dev-design", "attempt": 3, "verdict": "fail", "report": "三", "checklist": [] }"#,
            Some(ACTIVE_DEV_DESIGN),
        ),
    );

    let outcome = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "第四轮".to_owned(),
                checklist: pass_items(1),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect("落账应成功");

    assert_eq!(
        outcome.attempt, 4,
        "stale/pass/fail 混合历史全数 +1（不挑食）"
    );
}

// ---------------------------------------------------------------------------
// 边界：report 2000 端点 / skipped 形态 / 保形与 file_log 零触碰
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 异常：开相前置（无 active_phase / 停留他相）/ 表外相位
// ---------------------------------------------------------------------------

/// report 恰 2000 字符 → 落账成功（≤2000 边界含端点）。
#[test]
fn report恰2000字符落账成功() {
    let ws = TempWs::new("report-2000");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );

    let report = "评".repeat(2000);
    let outcome = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "dev-design".to_owned(),
                report,
                checklist: pass_items(1),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect("恰 2000 字符应落账成功（边界含端点）");
    assert_eq!(outcome.attempt, 1);
    assert_eq!(
        parse_of(&ws, CHANGE)["eval"][0]["report"]
            .as_str()
            .expect("report 在场")
            .chars()
            .count(),
        2000
    );
}

/// report 超长拒绝：2001 字符 → Err 且 eval 零新增。
#[test]
fn report超长2001拒绝且eval零新增() {
    let ws = TempWs::new("report-2001");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "评".repeat(2001),
                checklist: pass_items(1),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect_err("2001 字符应 Err");

    assert!(
        err.contains("2000") && err.contains("2001"),
        "错误携带上限与实际值，实际: {err}"
    );
    assert_eq!(
        ws.workflow_json_bytes(CHANGE),
        before,
        "拒绝零写入（eval 零新增）"
    );
}

/// skipped 形态：skipped=true（约束：verdict 必须 pass）落账形态——all-pass
/// 清单约束校验不误拒、`skipped: true` 字段落盘；fail 清单配 skipped=true 仍
/// 被约束显式拒绝（约束的另一面）。
#[test]
fn skipped形态落盘且约束两面各就位() {
    // 合法形态：skipped=true + 全 pass 清单
    let ws = TempWs::new("skipped-ok");
    ws.change(CHANGE, fixture_with_active_phase("", Some(ACTIVE_TEST_GEN)));
    let outcome = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "test-gen".to_owned(),
                report: "相位跳过".to_owned(),
                checklist: pass_items(1),
                skipped: true,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect("skipped=true 配 pass 清单不误拒");
    let entry = &parse_of(&ws, CHANGE)["eval"][0];
    assert_eq!(entry["verdict"], serde_json::json!("pass"));
    assert_eq!(
        entry["skipped"],
        serde_json::json!(true),
        "skipped 字段落盘"
    );
    let _ = outcome;

    // 约束另一面：skipped=true + 含 fail 项 → Err 零写入
    let ws_fail = TempWs::new("skipped-fail");
    ws_fail.change(CHANGE, fixture_with_active_phase("", Some(ACTIVE_TEST_GEN)));
    let before = ws_fail.workflow_json_bytes(CHANGE);
    let err = ws_fail
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "test-gen".to_owned(),
                report: "跳过但清单未过".to_owned(),
                checklist: with_fail_item(Vec::new()),
                skipped: true,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect_err("skipped=true 配 fail 清单应 Err");
    assert!(
        err.contains("pass") && err.contains("fail"),
        "约束记因，实际: {err}"
    );
    assert_eq!(
        ws_fail.workflow_json_bytes(CHANGE),
        before,
        "约束拒绝零写入"
    );
}

/// phase_log 保形与 file_log 零触碰：未知字段保形 + file_log 逐字节不变 +
/// pretty 写回可再读（W2/W3/AC-3 口径）。
#[test]
fn 落账保形_file_log零触碰_pretty可再读() {
    let ws = TempWs::new("preserve-and-filelog");
    ws.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "created": "2026-03-03",
  "custom_note": "保留我",
  "file_log": [
    { "op": "write", "scope": "workflow", "attempt": 1, "path": "a.md", "at": "2026-10-01T07:00:00Z" }
  ],
  "eval": [],
  "active_phase": { "phase": "proposal", "attempt": 1, "start_at": "2026-10-01T07:30:00Z" }
}"#,
    );
    let before_file_log = parse_of(&ws, CHANGE)["file_log"].clone();

    ws.log(
        CHANGE,
        &PhaseLogInput {
            phase: "proposal".to_owned(),
            report: "通过".to_owned(),
            checklist: pass_items(1),
            skipped: false,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
        },
    )
    .expect("落账应成功");

    let doc = parse_of(&ws, CHANGE);
    assert_eq!(
        doc["custom_note"],
        serde_json::json!("保留我"),
        "未知字段保形"
    );
    assert_eq!(doc["created"], serde_json::json!("2026-03-03"));
    assert_eq!(
        doc["file_log"], before_file_log,
        "file_log 逐字节不变（desktop run 零 file_log 新增——AC-3）"
    );

    // pretty 写回可再读：2 空格缩进 + 尾换行
    let text = String::from_utf8(ws.workflow_json_bytes(CHANGE)).expect("UTF-8");
    assert!(
        text.contains("\n  \"") && text.ends_with('\n'),
        "pretty 形态"
    );
    serde_json::from_str::<serde_json::Value>(&text).expect("写回应可再解析");
}

/// 无 active_phase 的落账拒绝：未开相位直接 phase_log → Err（start_at 无
/// 继承源，写面不为无主落账兜底）且 workflow.json 零写入。
#[test]
fn 无active_phase落账显式拒绝零写入() {
    let ws = TempWs::new("no-active");
    ws.change(CHANGE, fixture_with_active_phase("", None));
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "无运行态落账".to_owned(),
                checklist: pass_items(1),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect_err("无 active_phase 应 Err（开相前置）");

    assert!(
        err.contains("dev-design") && err.contains("active_phase"),
        "错误记因相位与开相前置，实际: {err}"
    );
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "拒绝零写入");
}

/// active_phase 停留他相的落账拒绝：跨相落账显式 Err（开相前置的另一面——
/// 只认匹配相位的开态，不静默按非匹配 active_phase 落账）。
#[test]
fn active_phase停留他相落账拒绝零写入() {
    let ws = TempWs::new("stale-active");
    ws.change(CHANGE, fixture_with_active_phase("", Some(ACTIVE_PROPOSAL)));
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "跨相落账".to_owned(),
                checklist: pass_items(1),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect_err("active_phase 停留 proposal 时落 dev-design 应 Err");

    assert!(
        err.contains("proposal") && err.contains("dev-design"),
        "错误记因停留相位与目标相位，实际: {err}"
    );
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "拒绝零写入");
}

/// 非法相位拒绝：相位不在 requirement 表 → Err 且零写入（表位前置在开相
/// 前置之前——表外相位不进开相比对）。
#[test]
fn 非法相位落账拒绝零写入() {
    let ws = TempWs::new("off-table-phase");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "不存在的相位".to_owned(),
                report: "表外相位".to_owned(),
                checklist: pass_items(1),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect_err("表外相位应 Err");

    assert!(
        err.contains("不存在的相位") && err.contains("不包含"),
        "错误记因表位缺失，实际: {err}"
    );
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "拒绝零写入");
}

// ---------------------------------------------------------------------------
// 异常：workflow_type 非 requirement / change 不存在
// ---------------------------------------------------------------------------

#[test]
fn workflow_type非requirement拒绝零写入() {
    let ws = TempWs::new("bad-type");
    ws.change(CHANGE, r#"{ "workflow_type": "refactor", "eval": [] }"#);
    let before = ws.workflow_json_bytes(CHANGE);

    let err = ws
        .log(
            CHANGE,
            &PhaseLogInput {
                phase: "proposal".to_owned(),
                report: "任意".to_owned(),
                checklist: pass_items(1),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect_err("非 requirement 应 Err");
    assert!(err.contains("refactor"), "W8 分层出口记因，实际: {err}");
    assert_eq!(ws.workflow_json_bytes(CHANGE), before, "拒绝零写入");
}

#[test]
fn change不存在显式err() {
    let ws = TempWs::new("missing-change");

    let err = ws
        .log(
            "不存在的-change",
            &PhaseLogInput {
                phase: "proposal".to_owned(),
                report: "任意".to_owned(),
                checklist: pass_items(1),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        )
        .expect_err("未知 change 应 Err");
    assert!(!err.is_empty(), "Err 显式，实际: {err}");
}

// ---------------------------------------------------------------------------
// 会话槽位显式在位落账（AC-4）：仅 Some 槽位以 raw snake_case 键 insert，
// 缺省槽位不产生键；宽松解析面读回逐字还原
// ---------------------------------------------------------------------------

/// 双槽位（executor + evaluator，decision None）落账：两 raw 键在场、值逐字
/// 透传（写面不解释不改写），`decision_session_id` 键不在场。
#[test]
fn 双槽位显式在位落账_raw键逐字透传且decision键不在场() {
    let ws = TempWs::new("slots-dual");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );

    ws.log(
        CHANGE,
        &PhaseLogInput {
            phase: "dev-design".to_owned(),
            report: "双槽位落账".to_owned(),
            checklist: pass_items(1),
            skipped: false,
            executor_session_id: Some("ses-exec-1".to_owned()),
            evaluator_session_id: Some("ses-eval-1".to_owned()),
            decision_session_id: None,
        },
    )
    .expect("落账应成功");

    let entry = &parse_of(&ws, CHANGE)["eval"][0];
    assert_eq!(
        entry["executor_session_id"],
        serde_json::json!("ses-exec-1"),
        "raw snake_case 键、值逐字透传"
    );
    assert_eq!(
        entry["evaluator_session_id"],
        serde_json::json!("ses-eval-1")
    );
    assert!(
        entry.get("decision_session_id").is_none(),
        "decision 槽位不走本输入（归 decision_log 单点挂账），缺省不产生键"
    );
}

/// 仅 executor 槽位（static-check 升格 fail 形态）：仅 `executor_session_id`
/// 单键在场，其余两键不产生。
#[test]
fn 仅executor槽位落账单键在场() {
    let ws = TempWs::new("slot-executor-only");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );

    ws.log(
        CHANGE,
        &PhaseLogInput {
            phase: "dev-design".to_owned(),
            report: "升格 fail".to_owned(),
            checklist: with_fail_item(Vec::new()),
            skipped: false,
            executor_session_id: Some("ses-exec-2".to_owned()),
            evaluator_session_id: None,
            decision_session_id: None,
        },
    )
    .expect("落账应成功");

    let entry = &parse_of(&ws, CHANGE)["eval"][0];
    assert_eq!(entry["verdict"], serde_json::json!("fail"));
    assert_eq!(
        entry["executor_session_id"],
        serde_json::json!("ses-exec-2"),
        "仅 executor 单键在场"
    );
    assert!(entry.get("evaluator_session_id").is_none());
    assert!(entry.get("decision_session_id").is_none());
}

/// 三槽位全 None：条目形状与既有形态逐字段一致（零新键——显式在位纪律使
/// 「缺省槽位条目与既有形态一致」静态保证）。
#[test]
fn 三槽位全none落账零新键_条目形状与既有形态一致() {
    let ws = TempWs::new("slots-none");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );

    ws.log(
        CHANGE,
        &PhaseLogInput {
            phase: "dev-design".to_owned(),
            report: "无槽位形态".to_owned(),
            checklist: pass_items(1),
            skipped: false,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
        },
    )
    .expect("落账应成功");

    let entry = &parse_of(&ws, CHANGE)["eval"][0];
    let keys: Vec<&str> = entry
        .as_object()
        .expect("条目对象")
        .keys()
        .map(String::as_str)
        .collect();
    for key in &keys {
        assert!(
            !key.ends_with("_session_id"),
            "零会话槽位新键，实际含: {key}"
        );
    }
    // 条目形状与既有形态一致（phase / attempt / verdict / report / checklist /
    // timestamp / start_at 七键）
    assert_eq!(keys.len(), 7, "条目键集与既有形态一致: {keys:?}");
}

/// 旧 workflow.json（eval 条目无槽位键）在场时落新条目：全文件经宽松解析不
/// 报错（`PhaseLog` `#[serde(default)]` 读兼容——旧条目槽位字段 None）；旧
/// 条目原样、未知字段保形、file_log 零触碰。
#[test]
fn 旧workflow无槽位键条目在场落新条目宽松解析不报错() {
    let ws = TempWs::new("legacy-compat");
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
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "旧形态条目", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" }
  ],
  "active_phase": { "phase": "dev-design", "attempt": 1, "start_at": "2026-10-01T08:15:00Z" }
}"#,
    );

    ws.log(
        CHANGE,
        &PhaseLogInput {
            phase: "dev-design".to_owned(),
            report: "新条目携槽位".to_owned(),
            checklist: pass_items(1),
            skipped: false,
            executor_session_id: Some("ses-exec-3".to_owned()),
            evaluator_session_id: Some("ses-eval-3".to_owned()),
            decision_session_id: None,
        },
    )
    .expect("旧文件在场落新条目应成功");

    let doc = parse_of(&ws, CHANGE);
    assert_eq!(doc["eval"].as_array().map(Vec::len), Some(2), "纯追加");
    // 旧条目原样（无槽位键、未知字段保留）
    assert_eq!(doc["eval"][0]["report"], serde_json::json!("旧形态条目"));
    assert!(doc["eval"][0].get("executor_session_id").is_none());
    assert_eq!(doc["custom_note"], serde_json::json!("保留我"));
    assert_eq!(doc["file_log"].as_array().map(Vec::len), Some(1));

    // 全文件经宽松解析面读回不报错：旧条目槽位字段 None
    let path =
        ws.0.join("openspec/changes")
            .join(CHANGE)
            .join("workflow.json");
    let typed = match parse_workflow_file(&path) {
        WorkflowFileParse::Parsed(workflow) => workflow,
        WorkflowFileParse::Unparsable { reason } => {
            panic!("旧文件落新条目后应可宽松解析: {reason}")
        }
    };
    assert_eq!(typed.eval.len(), 2);
    assert_eq!(
        typed.eval[0].executor_session_id, None,
        "旧条目槽位字段 None（#[serde(default)] 读兼容）"
    );
    assert_eq!(typed.eval[0].evaluator_session_id, None);
    assert_eq!(typed.eval[0].decision_session_id, None);
    assert_eq!(
        typed.eval[1].executor_session_id.as_deref(),
        Some("ses-exec-3"),
        "新条目槽位承接"
    );
}

/// 落账产物经宽松解析面（`workflow::parse::parse_workflow_file`）读回：三槽
/// 位值逐字还原（alias 承接 snake_case 磁盘键——「serde 写出仍可被插件解析
/// 面读取」的 desktop 侧机械证明）。
#[test]
fn 落账产物经宽松解析面读回三槽位值逐字还原() {
    let ws = TempWs::new("slots-roundtrip");
    ws.change(
        CHANGE,
        fixture_with_active_phase("", Some(ACTIVE_DEV_DESIGN)),
    );

    ws.log(
        CHANGE,
        &PhaseLogInput {
            phase: "dev-design".to_owned(),
            report: "三槽位往返".to_owned(),
            checklist: pass_items(1),
            skipped: false,
            executor_session_id: Some("ses-1-1727000000001".to_owned()),
            evaluator_session_id: Some("ses-2-1727000000002".to_owned()),
            decision_session_id: Some("ses-3-1727000000003".to_owned()),
        },
    )
    .expect("落账应成功");

    let path =
        ws.0.join("openspec/changes")
            .join(CHANGE)
            .join("workflow.json");
    let typed = match parse_workflow_file(&path) {
        WorkflowFileParse::Parsed(workflow) => workflow,
        WorkflowFileParse::Unparsable { reason } => {
            panic!("落账产物应可经宽松解析面读回: {reason}")
        }
    };
    let entry = typed.eval.last().expect("新条目在场");
    assert_eq!(
        entry.executor_session_id.as_deref(),
        Some("ses-1-1727000000001"),
        "alias 承接 snake_case 磁盘键、值逐字还原"
    );
    assert_eq!(
        entry.evaluator_session_id.as_deref(),
        Some("ses-2-1727000000002")
    );
    assert_eq!(
        entry.decision_session_id.as_deref(),
        Some("ses-3-1727000000003")
    );
}
