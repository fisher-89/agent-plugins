//! `write::phase_next` 的单元测试（test-design「phase_next.rs ->
//! phase_next_test.rs」节）：只读路由状态机的初始 / 推进 / fail 重试 / 重试
//! 上限 / 终态 / 白名单下发 / last_result 快照 / 会话锚点（基线登记、run_id
//! 隔离）/ mid-phase interruption 承接 / 只读性 / 三类显式 Err。
//!
//! Mock策略：无进程边界 mock（fs 真实组合）——tempdir 真实 change fixture
//! （workflow.json 以插件直跑形态真盘落盘，沿 core/workflow detail_test 装置
//! 先例）；SessionAnchors 以真实实例参与（进程内值对象不 mock）。「零写入」
//! 以调用前后字节比对断言。
//!
//! 窗口语义注记：锚点在 `(change, run_id)` 首见时登记 eval 条目数基线，
//! fail 窗口只数基线之后的条目——「重试上限」必须在 run 期间逐条追加 fail
//! 记录驱动（与真实 run 的 phase-log 节奏同构），预置全量历史的首见调用不
//! 触发上限（窗口外历史不重复记账）。

use std::fs;
use std::path::PathBuf;

use foundation::layout::resolve;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::phase_next::{phase_next, PhaseNextError, SessionAnchors};

/// 临时 workspace 根 RAII（沿 detail_test 装置先例）：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-phase-next-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// 预置 change 的 workflow.json（真盘落盘）。
    fn change(&self, name: &str, workflow_json: &str) {
        let dir = self.0.join("openspec/changes").join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        fs::write(dir.join("workflow.json"), workflow_json).expect("写 workflow.json 失败");
    }

    /// 整体改写 workflow.json（模拟 run 期间 phase-log 落账驱动窗口演进）。
    fn rewrite_workflow(&self, name: &str, workflow_json: &str) {
        fs::write(
            self.0
                .join("openspec/changes")
                .join(name)
                .join("workflow.json"),
            workflow_json,
        )
        .expect("改写 workflow.json 失败");
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
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// pass 条目行（ISO 时间戳递进控制「最新条目」定位）。
fn pass_entry(phase: &str, attempt: u32, ts: &str) -> String {
    format!(
        r#"{{ "phase": "{phase}", "attempt": {attempt}, "verdict": "pass", "report": "{phase} 通过", "checklist": [], "timestamp": "{ts}" }}"#
    )
}

/// fail 条目行。
fn fail_entry(phase: &str, attempt: u32, ts: &str, report: &str) -> String {
    format!(
        r#"{{ "phase": "{phase}", "attempt": {attempt}, "verdict": "fail", "report": "{report}", "checklist": [{{ "item": "检查项", "pass": false, "evidence": "未达成" }}], "timestamp": "{ts}" }}"#
    )
}

/// 组装 workflow.json 文本（requirement + eval 条目行 + 可选 active_phase）。
fn workflow_json(entries: &[String], active_phase: Option<&str>) -> String {
    let eval = entries.join(",\n    ");
    let active = active_phase.unwrap_or("null");
    format!(
        r#"{{
  "workflow_type": "requirement",
  "eval": [
    {eval}
  ],
  "active_phase": {active}
}}"#
    )
}

const CHANGE: &str = "demo-change";
const RUN: &str = "run-1";

/// 一次路由调用（tempdir 根 + 独立锚点实例）。
fn route(
    ws: &TempWs,
    run_id: &str,
    anchors: &SessionAnchors,
) -> super::phase_next::PhaseNextOutcome {
    let layout = resolve(&ws.0);
    phase_next(&layout, CHANGE, run_id, anchors).expect("phase_next 应成功")
}

// ---------------------------------------------------------------------------
// 正向：初始路由 / pass 推进 / fail 预算内重试 / 全 pass 终态 / 白名单 / last_result
// ---------------------------------------------------------------------------

/// 初始路由：初始 fixture（无 active_phase、eval 空）→ next_phase=首相位、
/// done=false、executor/evaluator prompt 已插值 `<change>`、白名单为空。
#[test]
fn 初始路由落首相位且prompt已插值白名单为空() {
    let ws = TempWs::new("initial");
    ws.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

    let outcome = route(&ws, RUN, &SessionAnchors::new());

    assert!(!outcome.done);
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("proposal"),
        "首相位路由"
    );
    assert_eq!(outcome.round, 1, "空 eval 窗口首轮");
    assert!(outcome.error.is_none());

    // executor / evaluator prompt 已插值 `<change>`（写面单源插值随行下发）
    let executor = outcome.executor.expect("proposal 应有 executor");
    let evaluator = outcome.evaluator.expect("proposal 应有 evaluator");
    assert!(
        executor.prompt.contains(CHANGE) && !executor.prompt.contains("<change>"),
        "executor prompt 已完成 <change> 插值: {}",
        executor.prompt
    );
    assert!(
        evaluator.prompt.contains(CHANGE) && !evaluator.prompt.contains("<change>"),
        "evaluator prompt 已完成 <change> 插值"
    );
    assert!(
        executor.prompt.contains("proposal"),
        "executor prompt 已完成 <phase> 插值"
    );

    // 白名单为空（首相位无前置可回溯）
    assert!(outcome.allowed_backtrack_phases.is_empty());
    // 无 eval 史 → last_result None
    assert!(outcome.last_result.is_none());
}

/// pass 推进：当前相位最新条目 pass → next_phase=下一相位、round 归位、
/// 白名单=新相位的表序前置集（AC-1 推进面）。
#[test]
fn pass推进到下一相位且round归位白名单随行() {
    let ws = TempWs::new("advance");
    ws.change(
        CHANGE,
        &workflow_json(&[pass_entry("proposal", 1, "2026-10-01T08:00:00Z")], None),
    );

    let outcome = route(&ws, RUN, &SessionAnchors::new());

    assert_eq!(outcome.next_phase.as_deref(), Some("dev-design"));
    assert_eq!(outcome.round, 1, "round 归位（锚点取当前条目数基线）");
    assert_eq!(
        outcome.allowed_backtrack_phases,
        vec!["proposal".to_owned(), "dev-design".to_owned()],
        "白名单 = 新相位的表序前置集"
    );
    // 上一相位的 pass 记录成为 last_result（决策输入面）
    let last = outcome.last_result.expect("有 eval 史应携 last_result");
    assert_eq!(last.phase, "proposal");
    assert_eq!(last.report, "proposal 通过");
}

/// fail 预算内重试：最新条目 fail 且窗口 fail 数 < MAX_RETRY_TIMES →
/// next_phase=同相位、round 递增、executor/evaluator prompt 同相位插值
///（AC-1 重试面；上限判定不自建——写面 phase-next 权威返回）。
#[test]
fn fail预算内重试同相位且round递增() {
    let ws = TempWs::new("retry");
    ws.change(
        CHANGE,
        &workflow_json(&[pass_entry("proposal", 1, "2026-10-01T08:00:00Z")], None),
    );
    let anchors = SessionAnchors::new();

    let first = route(&ws, RUN, &anchors);
    assert_eq!(first.next_phase.as_deref(), Some("dev-design"));
    assert_eq!(first.round, 1);

    // 模拟 run 期间 phase-log 落账两条 fail（窗口内 fail 数 2 < 5）
    ws.rewrite_workflow(
        CHANGE,
        &workflow_json(
            &[
                pass_entry("proposal", 1, "2026-10-01T08:00:00Z"),
                fail_entry("dev-design", 1, "2026-10-01T08:10:00Z", "首轮未过"),
                fail_entry("dev-design", 2, "2026-10-01T08:20:00Z", "次轮未过"),
            ],
            None,
        ),
    );

    let second = route(&ws, RUN, &anchors);
    assert_eq!(
        second.next_phase.as_deref(),
        Some("dev-design"),
        "fail 同相位重试"
    );
    assert_eq!(second.round, 3, "窗口条目数 + 1（1 基线 + 2 fail）");
    assert!(second.error.is_none(), "预算内不触发上限");
    assert!(
        second
            .executor
            .as_ref()
            .expect("executor 在场")
            .prompt
            .contains(CHANGE),
        "executor prompt 同相位插值（<change> 已替换）"
    );
    assert!(
        second
            .evaluator
            .as_ref()
            .expect("evaluator 在场")
            .prompt
            .contains("dev-design"),
        "evaluator prompt 同相位插值（<phase> → dev-design）"
    );
    // last_result 携最新 fail 条目（重试输入面）
    let last = second.last_result.expect("last_result 在场");
    assert_eq!(last.phase, "dev-design");
    assert_eq!(last.report, "次轮未过");
}

/// 重试上限恰达：窗口 fail 数 == MAX_RETRY_TIMES → error=MaxRetriesExceeded
///{phase, round}，next_phase=None、done=false（决策分叉触发——AC-2 唤起触发面）。
#[test]
fn 重试上限恰达返回max_retries_exceeded() {
    let ws = TempWs::new("limit-hit");
    ws.change(
        CHANGE,
        &workflow_json(&[pass_entry("proposal", 1, "2026-10-01T08:00:00Z")], None),
    );
    let anchors = SessionAnchors::new();
    let _ = route(&ws, RUN, &anchors); // 首见登记基线 1

    // run 期间逐条落账 5 条 fail（恰达 MAX_RETRY_TIMES）
    let mut entries = vec![pass_entry("proposal", 1, "2026-10-01T08:00:00Z")];
    for attempt in 1..=5u32 {
        entries.push(fail_entry(
            "dev-design",
            attempt,
            &format!("2026-10-01T08:{:02}:00Z", 10 + attempt),
            "未过",
        ));
        ws.rewrite_workflow(CHANGE, &workflow_json(&entries, None));
        let outcome = route(&ws, RUN, &anchors);
        if attempt < 5 {
            assert!(outcome.error.is_none(), "第 {attempt} 条 fail 未达上限");
        } else {
            assert_eq!(
                outcome.error,
                Some(PhaseNextError::MaxRetriesExceeded {
                    phase: "dev-design".to_owned(),
                    round: 6,
                }),
                "第 5 条 fail 恰达上限（5 条窗口 fail）"
            );
            assert_eq!(outcome.next_phase, None, "上限响应不下发相位");
            assert!(!outcome.done, "上限非终态（决策分叉待定）");
            assert!(outcome.executor.is_none() && outcome.evaluator.is_none());
        }
    }
}

/// 上限前一手：窗口 fail 数 == MAX_RETRY_TIMES - 1 → 仍同相位重试、
/// error=None（上限边界不提前触发）。
#[test]
fn 上限前一手仍重试不提前触发() {
    let ws = TempWs::new("limit-edge");
    ws.change(
        CHANGE,
        &workflow_json(&[pass_entry("proposal", 1, "2026-10-01T08:00:00Z")], None),
    );
    let anchors = SessionAnchors::new();
    let _ = route(&ws, RUN, &anchors);

    // 落账 4 条 fail（MAX_RETRY_TIMES - 1）
    let entries: Vec<String> = (1..=4)
        .map(|attempt| fail_entry("dev-design", attempt, "2026-10-01T08:10:00Z", "未过"))
        .collect();
    let mut all = vec![pass_entry("proposal", 1, "2026-10-01T08:00:00Z")];
    all.extend(entries);
    ws.rewrite_workflow(CHANGE, &workflow_json(&all, None));

    let outcome = route(&ws, RUN, &anchors);
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("dev-design"),
        "4 条窗口 fail 仍同相位重试"
    );
    assert!(outcome.error.is_none(), "上限前一手不提前触发");
    assert_eq!(outcome.round, 5);
}

/// 全部 pass 终态：末相位 pass → done=true、next_phase=None、双 prompt 缺席
///（walker 收敛 completed 的判定输入）。
#[test]
fn 全部pass终态done收敛() {
    let entries: Vec<String> = [
        ("proposal", "2026-10-01T08:00:00Z"),
        ("dev-design", "2026-10-01T08:10:00Z"),
        ("test-design", "2026-10-01T08:20:00Z"),
        ("implement", "2026-10-01T08:30:00Z"),
        ("test-gen", "2026-10-01T08:40:00Z"),
        ("test-execution", "2026-10-01T08:50:00Z"),
        ("code-review", "2026-10-01T09:00:00Z"),
        ("acceptance", "2026-10-01T09:10:00Z"),
    ]
    .iter()
    .enumerate()
    .map(|(idx, (phase, ts))| pass_entry(phase, (idx + 1) as u32, ts))
    .collect();
    let ws = TempWs::new("all-pass");
    ws.change(CHANGE, &workflow_json(&entries, None));

    let outcome = route(&ws, RUN, &SessionAnchors::new());

    assert!(outcome.done, "全相位 pass → done");
    assert_eq!(outcome.next_phase, None);
    assert!(outcome.executor.is_none() && outcome.evaluator.is_none());
    assert!(outcome.error.is_none());
    assert!(outcome.allowed_backtrack_phases.is_empty());
}

/// 白名单下发：中段相位的 outcome.allowed_backtrack_phases 与白名单计算一致
///（表序前置，AC-2 下发的内容面）。
#[test]
fn 白名单随行下发与表序前置集一致() {
    let entries = vec![pass_entry("proposal", 1, "2026-10-01T08:00:00Z")];
    let ws = TempWs::new("whitelist");
    ws.change(CHANGE, &workflow_json(&entries, None));

    let outcome = route(&ws, RUN, &SessionAnchors::new());
    assert_eq!(outcome.next_phase.as_deref(), Some("dev-design"));
    assert_eq!(
        outcome.allowed_backtrack_phases,
        vec!["proposal".to_owned(), "dev-design".to_owned()],
        "白名单随 phase-next 响应缓存下发"
    );

    // 深段相位（implement）白名单扩展至表序前置全量
    let mut entries = vec![
        pass_entry("proposal", 1, "2026-10-01T08:00:00Z"),
        pass_entry("dev-design", 1, "2026-10-01T08:10:00Z"),
        pass_entry("test-design", 1, "2026-10-01T08:20:00Z"),
    ];
    // implement fail 一条 → 路由到 implement 重试
    entries.push(fail_entry("implement", 1, "2026-10-01T08:30:00Z", "未过"));
    ws.rewrite_workflow(CHANGE, &workflow_json(&entries, None));
    let anchors = SessionAnchors::new();
    let outcome = route(&ws, "run-deep", &anchors);
    assert_eq!(outcome.next_phase.as_deref(), Some("implement"));
    assert_eq!(
        outcome.allowed_backtrack_phases,
        vec![
            "proposal".to_owned(),
            "dev-design".to_owned(),
            "test-design".to_owned(),
            "implement".to_owned(),
        ],
    );
}

/// last_result 快照：有 eval 史 → 携最近条目 phase/verdict/report/timestamp
///（timestamp 降序定位，决策输入面）。
#[test]
fn last_result快照携最近条目全字段() {
    //乱序落盘 + 时间戳定序：时间戳最晚者胜出
    let entries = vec![
        fail_entry("dev-design", 2, "2026-10-01T09:00:00Z", "时间戳最晚"),
        pass_entry("proposal", 1, "2026-10-01T08:00:00Z"),
        fail_entry("dev-design", 1, "2026-10-01T08:30:00Z", "较早失败"),
    ];
    let ws = TempWs::new("last-result");
    ws.change(CHANGE, &workflow_json(&entries, None));

    let outcome = route(&ws, RUN, &SessionAnchors::new());
    let last = outcome.last_result.expect("有 eval 史应携 last_result");
    assert_eq!(last.phase, "dev-design");
    assert_eq!(last.report, "时间戳最晚");
    let expected =
        OffsetDateTime::parse("2026-10-01T09:00:00Z", &Rfc3339).expect("基准时间戳应可解析");
    assert_eq!(last.timestamp, Some(expected), "timestamp 宽松解析随行");

    // 无 eval 史 → None（初始路由用例已断言，此处以空 eval fixture 复核）
    let ws_empty = TempWs::new("last-result-empty");
    ws_empty.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);
    assert!(route(&ws_empty, RUN, &SessionAnchors::new())
        .last_result
        .is_none());
}

// ---------------------------------------------------------------------------
// 边界：会话锚点（基线登记 / run_id 隔离）、mid-phase interruption 承接、只读性
// ---------------------------------------------------------------------------

/// 锚点首见记基线：新 (change, run_id) 首次 phase_next 记录当时 eval 条目数
/// 基线——窗口外历史不重复记账（历史 fail 不计入重试窗口，round 自 1 起）。
#[test]
fn 锚点首见登记条目数基线且窗口外历史不重复记账() {
    let entries = vec![
        pass_entry("proposal", 1, "2026-10-01T08:00:00Z"),
        fail_entry("dev-design", 1, "2026-10-01T08:10:00Z", "历史失败一"),
        fail_entry("dev-design", 2, "2026-10-01T08:20:00Z", "历史失败二"),
    ];
    let ws = TempWs::new("anchor-baseline");
    ws.change(CHANGE, &workflow_json(&entries, None));

    let outcome = route(&ws, RUN, &SessionAnchors::new());

    assert_eq!(outcome.round, 1, "基线 = 首见时条目数 3，round 自 1 起");
    assert_eq!(outcome.next_phase.as_deref(), Some("dev-design"));
    assert!(
        outcome.error.is_none(),
        "窗口外 2 条历史 fail 不计入重试窗口（不虚触上限）"
    );

    // 锚点复用：基线后新落 1 条 → round 2（非 4）
    let mut entries = entries;
    entries.push(fail_entry(
        "dev-design",
        3,
        "2026-10-01T08:30:00Z",
        "run 内新增",
    ));
    ws.rewrite_workflow(CHANGE, &workflow_json(&entries, None));
    let anchors = SessionAnchors::new();
    let _ = route(&ws, "fresh-run", &anchors); // 新 run 重新锚定（基线 4）
    assert_eq!(route(&ws, "fresh-run", &anchors).round, 1);
}

/// 锚点 run_id 隔离：同 change 不同 run_id 基线互不影响（per-run 实例、跨 run
/// 不共享）；同 (change, run_id) 复用同一基线。
#[test]
fn 锚点按run_id隔离且同run复用基线() {
    let ws = TempWs::new("anchor-isolation");
    ws.change(
        CHANGE,
        &workflow_json(&[pass_entry("proposal", 1, "2026-10-01T08:00:00Z")], None),
    );
    let anchors = SessionAnchors::new();

    // run-a 首见：基线 1
    assert_eq!(route(&ws, "run-a", &anchors).round, 1);

    // run 期间落账 2 条 → run-a 复用基线 1：round = 3
    let entries = vec![
        pass_entry("proposal", 1, "2026-10-01T08:00:00Z"),
        fail_entry("dev-design", 1, "2026-10-01T08:10:00Z", "未过"),
        fail_entry("dev-design", 2, "2026-10-01T08:20:00Z", "未过"),
    ];
    ws.rewrite_workflow(CHANGE, &workflow_json(&entries, None));
    assert_eq!(route(&ws, "run-a", &anchors).round, 3, "同 run 复用基线");

    // run-b（新窗口）：基线取当前条目数 3 → round 归位 1（跨 run 不共享）
    assert_eq!(
        route(&ws, "run-b", &anchors).round,
        1,
        "不同 run_id 基线互不影响"
    );
}

/// mid-phase interruption 承接：active_phase 残留 + 该相位已有 eval 条目 →
/// phase_next 解析到 active_phase 相位承接（不跳相位、不重置已落账条目，
/// 差异表 #9 进程内复活——AC-7 续走伴生面）。
#[test]
fn 中断重入承接active_phase不跳相位不重置已落账() {
    let entries = vec![
        pass_entry("proposal", 1, "2026-10-01T08:00:00Z"),
        fail_entry("dev-design", 1, "2026-10-01T08:10:00Z", "中断前未过"),
    ];
    let active = r#"{ "phase": "dev-design", "attempt": 2, "start_at": "2026-10-01T08:15:00Z" }"#;
    let ws = TempWs::new("interruption");
    ws.change(CHANGE, &workflow_json(&entries, Some(active)));

    // 桌面重启后重新发起 run：全新锚点实例（新 run_id）
    let outcome = route(&ws, "run-after-restart", &SessionAnchors::new());

    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("dev-design"),
        "承接 active_phase 残留相位（不跳相位）"
    );
    let last = outcome.last_result.expect("已落账条目不被重置");
    assert_eq!(last.phase, "dev-design");
    assert_eq!(last.report, "中断前未过", "既有 eval 条目随行承接");
    assert_eq!(outcome.round, 1, "新 run 窗口自 1 起");
    assert!(
        outcome.error.is_none(),
        "历史 fail 不入新窗口（不虚触上限）"
    );
    assert!(
        outcome
            .evaluator
            .as_ref()
            .expect("evaluator 在场")
            .prompt
            .contains("dev-design"),
        "承接相位 prompt 同相位插值（<phase> → dev-design）"
    );
}

/// phase_next 只读性：调用前后 workflow.json 字节不变（只读路由不改 eval
/// store——写面红线锚）。
#[test]
fn 只读路由不改workflow_json字节() {
    let entries = vec![pass_entry("proposal", 1, "2026-10-01T08:00:00Z")];
    let ws = TempWs::new("readonly");
    ws.change(CHANGE, &workflow_json(&entries, None));

    let before = ws.workflow_json_bytes(CHANGE);
    let anchors = SessionAnchors::new();
    for _ in 0..3 {
        let _ = route(&ws, RUN, &anchors);
    }
    assert_eq!(
        ws.workflow_json_bytes(CHANGE),
        before,
        "只读路由：调用前后字节不变"
    );
}

// ---------------------------------------------------------------------------
// 异常：change 不存在 / workflow_type 非 requirement / workflow.json 不可解析
// ---------------------------------------------------------------------------

#[test]
fn change不存在时显式err不静默空产出() {
    let ws = TempWs::new("missing-change");
    ws.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

    let layout = resolve(&ws.0);
    let err = phase_next(&layout, "不存在的-change", RUN, &SessionAnchors::new())
        .expect_err("未知 change 应 Err");
    assert!(
        err.contains("不存在") || err.contains("读取") || err.contains("无法解析"),
        "Err 显式（不静默空产出），实际: {err}"
    );
}

#[test]
fn workflow_type非requirement时err显式分层出口() {
    let ws = TempWs::new("bad-type");
    ws.change(CHANGE, r#"{ "workflow_type": "bug-fix", "eval": [] }"#);

    let layout = resolve(&ws.0);
    let err = phase_next(&layout, CHANGE, RUN, &SessionAnchors::new())
        .expect_err("非 requirement 应 Err");
    assert!(
        err.contains("bug-fix") && err.contains("requirement"),
        "W8 写面侧出口（与命令层前置校验分层），实际: {err}"
    );
}

#[test]
fn workflow_json不可解析时err显式不臆测路由() {
    let ws = TempWs::new("corrupt");
    ws.change(CHANGE, "{ not valid json !!!");

    let layout = resolve(&ws.0);
    let err =
        phase_next(&layout, CHANGE, RUN, &SessionAnchors::new()).expect_err("损坏 fixture 应 Err");
    assert!(
        err.contains("无法解析") || err.contains("解析"),
        "Err 显式（不臆测路由），实际: {err}"
    );
}
