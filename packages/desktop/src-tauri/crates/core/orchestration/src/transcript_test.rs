//! `transcript`（转录提取器）的单元测试：final_assistant_text 末条
//! assistant Message 多文本块按序拼接（verdict / 决策 JSON 载体保真）、
//! 工具块不参与拼接、无 assistant Message / 空转录显式缺席 None。无进程
//! 边界依赖：密封 AgentEvent 转录以内存 Vec fixture 构造。

use agent::{AgentBlock, AgentEvent, AgentEventKind, AgentMessageRole};

use crate::finish::{FinishPayload, TOOL_NAME};
use crate::transcript::{failure_reason, final_assistant_text, finish_payload};

// ---------------------------------------------------------------------------
// 装置：密封事件 fixture（Message 事件 + ToolUse / ToolResult / Text 块）
// ---------------------------------------------------------------------------

fn message(seq: u64, role: AgentMessageRole, blocks: Vec<AgentBlock>) -> AgentEvent {
    AgentEvent::stamp(
        None,
        seq,
        AgentEventKind::Message {
            role,
            blocks,
            parent_tool_use_id: None,
        },
    )
}

fn notice(seq: u64, subtype: &str, payload: serde_json::Value) -> AgentEvent {
    AgentEvent::stamp(
        None,
        seq,
        AgentEventKind::SystemNotice {
            subtype: subtype.to_owned(),
            payload,
        },
    )
}

fn turn_done(seq: u64, subtype: &str, is_error: bool) -> AgentEvent {
    AgentEvent::stamp(
        None,
        seq,
        AgentEventKind::TurnDone {
            subtype: subtype.to_owned(),
            is_error,
            num_turns: None,
            duration_ms: None,
            cost_usd: None,
            usage: serde_json::Value::Null,
            session_id: None,
        },
    )
}

fn tool_use(id: &str, name: &str, input: serde_json::Value) -> AgentBlock {
    AgentBlock::ToolUse {
        id: id.to_owned(),
        name: name.to_owned(),
        input,
    }
}

fn tool_result(id: &str, content: &str) -> AgentBlock {
    AgentBlock::ToolResult {
        id: id.to_owned(),
        content: content.to_owned(),
        is_error: false,
    }
}

fn text_block(text: &str) -> AgentBlock {
    AgentBlock::Text {
        text: text.to_owned(),
        citations: None,
    }
}

/// 字符串对 → JSON 对象载荷（键动态，避开 json! 字面键）。
fn tool_input(pairs: &[(&str, &str)]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in pairs {
        map.insert(
            (*key).to_owned(),
            serde_json::Value::String((*value).to_owned()),
        );
    }
    serde_json::Value::Object(map)
}

// ---------------------------------------------------------------------------
// final_assistant_text：末条 assistant 文本载体
// ---------------------------------------------------------------------------

#[test]
fn final_assistant_text_joins_text_blocks_in_order() {
    let transcript = vec![
        message(
            0,
            AgentMessageRole::User,
            vec![text_block("给出评估结论。")],
        ),
        // 中间轮：文本块 + 工具块 + 文本块混排（工具块不参与拼接）
        message(
            1,
            AgentMessageRole::Assistant,
            vec![
                text_block("前导说明"),
                tool_use("tu-1", "Read", tool_input(&[("file_path", "src/a.rs")])),
                text_block("{\"verdict\": \"pass\"}"),
            ],
        ),
        message(2, AgentMessageRole::Tool, vec![tool_result("tu-1", "内容")]),
        // 末条 assistant Message：多文本块按序 \n 拼接
        message(
            3,
            AgentMessageRole::Assistant,
            vec![
                text_block("最终结论："),
                text_block("{\"verdict\": \"fail\", \"checklist\": []}"),
            ],
        ),
        // 末尾 user 言语不改变「末条 assistant」定位
        message(4, AgentMessageRole::User, vec![text_block("收到。")]),
    ];
    assert_eq!(
        final_assistant_text(&transcript).as_deref(),
        Some("最终结论：\n{\"verdict\": \"fail\", \"checklist\": []}"),
        "末条 assistant Message 多文本块按序拼接（verdict / 决策 JSON 载体保真）"
    );
}

#[test]
fn final_assistant_text_absence_shapes_return_none() {
    // 无 assistant Message（仅 user / 工具事件）→ None
    let no_assistant = vec![
        message(0, AgentMessageRole::User, vec![text_block("问题")]),
        message(1, AgentMessageRole::Tool, vec![tool_result("tu-1", "结果")]),
    ];
    assert_eq!(
        final_assistant_text(&no_assistant),
        None,
        "无 assistant Message → None（解析层显式缺席，verdict 解析 Err 的前置）"
    );

    // assistant 文本块全空白 → None（空白载体不充当 JSON 载体）
    let blank_only = vec![message(
        0,
        AgentMessageRole::Assistant,
        vec![text_block("   \n\t")],
    )];
    assert_eq!(final_assistant_text(&blank_only), None);
}

#[test]
fn empty_transcript_yields_none_text() {
    assert_eq!(
        final_assistant_text(&[]),
        None,
        "空事件数组 → None（文本提取器合法空态）"
    );
}

// ---------------------------------------------------------------------------
// failure_reason：失败成因提取（api_error 记因优先，失败 TurnDone subtype 兜底）
// ---------------------------------------------------------------------------

#[test]
fn failure_reason_prefers_api_error_cause() {
    // 会话中途恢复过的 api_retry 不充当成因（已恢复通知非失败记因）；
    // 末条 api_error 的 error 载荷才是失败记因
    let transcript = vec![
        message(0, AgentMessageRole::Assistant, vec![text_block("干活")]),
        notice(
            1,
            "api_retry",
            serde_json::json!({ "attempt": 1, "error": "已恢复的抖动" }),
        ),
        notice(
            2,
            "api_error",
            serde_json::json!({ "error": "API 流失败: HttpError: Invalid content type was returned: \"text/html\"" }),
        ),
        turn_done(3, "api_error", true),
    ];
    assert_eq!(
        failure_reason(&transcript).as_deref(),
        Some("API 流失败: HttpError: Invalid content type was returned: \"text/html\""),
        "api_error 记因载荷为失败成因唯一口径"
    );
}

#[test]
fn failure_reason_falls_back_to_failed_turn_done_subtype() {
    // 记因通知缺席（CLI 引擎无 api_error 留痕）：失败 TurnDone.subtype 兜底
    let transcript = vec![
        message(0, AgentMessageRole::Assistant, vec![text_block("干活")]),
        turn_done(1, "error_during_execution", true),
    ];
    assert_eq!(
        failure_reason(&transcript).as_deref(),
        Some("error_during_execution"),
        "TurnDone 失败 subtype 兜底成因"
    );
}

#[test]
fn failure_reason_carries_model_refusal_to_step_detail() {
    // 拒答终局：sdk 引擎发 api_error 记因 + TurnDone{model_refused, is_error}。
    // step detail 与 run 终态同文取记因载荷（walker 的 failure_reason 调用点），
    // 故断言记因串即固定「session 横幅与 step 都显示」的闭环
    let transcript = vec![
        message(
            0,
            AgentMessageRole::Assistant,
            vec![text_block("我不能协助")],
        ),
        notice(
            1,
            "api_error",
            serde_json::json!({
                "error": "模型拒答：provider 安全分类器拦截了本次请求（不可重试；请调整输入或更换模型）"
            }),
        ),
        turn_done(2, "model_refused", true),
    ];
    let reason = failure_reason(&transcript).expect("拒答必有失败成因");
    assert!(
        reason.contains("拒答") && reason.contains("安全分类器"),
        "记因串到达编排侧（步 detail 取同一串），实际: {reason}"
    );
}

#[test]
fn failure_reason_carries_unknown_finish_with_provider_value() {
    // 未知终局（fail-closed）：成因必须携 provider 原值，便于排障归因网关
    let transcript = vec![
        notice(
            0,
            "api_error",
            serde_json::json!({
                "error": "模型返回未识别的终局原因: some_gateway_reason（本地无对应处置，按失败收敛）"
            }),
        ),
        turn_done(1, "unexpected_finish", true),
    ];
    let reason = failure_reason(&transcript).expect("未知终局必有失败成因");
    assert!(
        reason.contains("some_gateway_reason"),
        "成因携 provider 原值，实际: {reason}"
    );
}

#[test]
fn failure_reason_absence_shapes_return_none() {
    // 成功 TurnDone 不充当成因（非失败语境）
    let ok = vec![turn_done(0, "success", false)];
    assert_eq!(failure_reason(&ok), None, "成功收口无失败成因");

    // api_error 记因载荷缺 error 字段 / 空白 → 不记因
    let blank_cause = vec![
        notice(0, "api_error", serde_json::json!({ "error": "  " })),
        turn_done(1, "api_error", true),
    ];
    assert_eq!(
        failure_reason(&blank_cause).as_deref(),
        Some("api_error"),
        "空白记因载荷回落 TurnDone subtype"
    );

    // 空转录 → None（调用点回落通用文案）
    assert_eq!(failure_reason(&[]), None);
}

// ---------------------------------------------------------------------------
// finish_payload：收口载荷提取单点（末条 finish_step notice）
// ---------------------------------------------------------------------------

/// evaluator 形态收口 notice（`{ "evaluator": { verdict, report, checklist } }`）。
fn evaluator_notice_payload(verdict: &str, report: &str, item: &str) -> serde_json::Value {
    serde_json::json!({
        "evaluator": {
            "verdict": verdict,
            "report": report,
            "checklist": [{ "item": item, "pass": verdict == "pass", "evidence": "据" }],
        }
    })
}

/// decision 形态收口 notice（`{ "decision": { action, ... } }`）。
fn decision_notice_payload(action: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "decision": action })
}

#[test]
fn finish_payload末条notice命中且逐字段相等() {
    let transcript = vec![
        message(0, AgentMessageRole::User, vec![text_block("评估")]),
        message(1, AgentMessageRole::Assistant, vec![text_block("结论")]),
        notice(
            2,
            TOOL_NAME,
            evaluator_notice_payload("fail", "评估报告正文", "覆盖缺口"),
        ),
    ];
    let payload = finish_payload(&transcript).expect("末条 notice 应命中");
    let FinishPayload::Evaluator(checklist) = payload else {
        panic!("evaluator 形态");
    };
    assert_eq!(
        checklist.verdict,
        workflow::model::Verdict::Fail,
        "verdict 逐字段相等"
    );
    assert_eq!(checklist.report, "评估报告正文");
    assert_eq!(checklist.checklist[0].item, "覆盖缺口");
}

#[test]
fn finish_payload的decision形态命中() {
    let transcript = vec![notice(
        0,
        TOOL_NAME,
        decision_notice_payload(serde_json::json!({
            "action": "ask",
            "question": "选哪个？",
            "options": ["甲"],
        })),
    )];
    let payload = finish_payload(&transcript).expect("decision notice 应命中");
    let FinishPayload::Decision(action) = payload else {
        panic!("decision 形态");
    };
    assert_eq!(
        action,
        crate::decision::DecisionAction::Ask {
            question: "选哪个？".to_owned(),
            options: vec!["甲".to_owned()],
        },
        "两臂皆可辨"
    );
}

#[test]
fn finish_payload缺notice三形态返回none() {
    assert_eq!(finish_payload(&[]), None, "空转录 → None");

    let only_messages = vec![
        message(0, AgentMessageRole::User, vec![text_block("问")]),
        message(1, AgentMessageRole::Assistant, vec![text_block("答")]),
        message(
            2,
            AgentMessageRole::Assistant,
            vec![tool_use("tu-1", "read", tool_input(&[("path", "a.rs")]))],
        ),
    ];
    assert_eq!(
        finish_payload(&only_messages),
        None,
        "只有 Message / ToolUse 的转录 → None"
    );

    let other_notices = vec![
        notice(0, "api_retry", serde_json::json!({ "attempt": 1 })),
        notice(1, "context_pruned", serde_json::json!({ "pruned": 2 })),
        notice(
            2,
            "output_truncated",
            serde_json::json!({ "reason": "max_tokens" }),
        ),
    ];
    assert_eq!(
        finish_payload(&other_notices),
        None,
        "其他 subtype（含 output_truncated）→ None（截断判据在 walker）"
    );
}

#[test]
fn finish_payload载荷损坏四形态返回none() {
    for (label, payload) in [
        ("null", serde_json::Value::Null),
        ("空对象", serde_json::json!({})),
        ("未知键", serde_json::json!({ "unknown": 1 })),
        ("内层形状错", serde_json::json!({ "evaluator": "str" })),
    ] {
        let transcript = vec![notice(0, TOOL_NAME, payload)];
        assert_eq!(
            finish_payload(&transcript),
            None,
            "损坏载荷形态 {label} → None（不 panic、不 unwrap 抛错）"
        );
    }
}

#[test]
fn finish_payload多条取末条且末条损坏不回退() {
    // 两条 notice：先 decision Stop、后 evaluator → 取末条
    let transcript = vec![
        notice(
            0,
            TOOL_NAME,
            decision_notice_payload(serde_json::json!({ "action": "stop", "reason": "停" })),
        ),
        notice(
            1,
            TOOL_NAME,
            evaluator_notice_payload("pass", "后条报告", "后项"),
        ),
    ];
    let payload = finish_payload(&transcript).expect("末条命中");
    assert!(
        matches!(payload, FinishPayload::Evaluator(_)),
        "取末条（evaluator 臂），实际: {payload:?}"
    );
}

/// 「末条」是语义而非「最后一条可解析的」：末条损坏时 MUST NOT 回退到前条
///（design D10 的显式判据）。本用例是该判据的守卫面——若实现改为「取最后一条
/// 可解析载荷」语义，本断言即红。
#[test]
fn finish_payload末条损坏不回退到前条() {
    let transcript = vec![
        notice(
            0,
            TOOL_NAME,
            evaluator_notice_payload("pass", "前条报告", "前项"),
        ),
        notice(1, TOOL_NAME, serde_json::json!({})),
    ];
    assert_eq!(
        finish_payload(&transcript),
        None,
        "末条损坏 → None（MUST NOT 回退到前条可解析载荷）"
    );
}

#[test]
fn finish_payload规模与位置边界三形态() {
    // 1000 条事件、finish_step 在第 1 条（最前）→ 仍命中（倒序扫描覆盖全长）
    let mut transcript = vec![notice(
        0,
        TOOL_NAME,
        evaluator_notice_payload("pass", "报告", "项"),
    )];
    for seq in 1..1000 {
        transcript.push(message(
            seq,
            AgentMessageRole::Assistant,
            vec![text_block("填充")],
        ));
    }
    assert!(
        finish_payload(&transcript).is_some(),
        "倒序扫描覆盖全长（首条命中）"
    );

    // 1000 条事件中零 notice → None 且不超时
    let transcript: Vec<AgentEvent> = (0..1000)
        .map(|seq| message(seq, AgentMessageRole::Assistant, vec![text_block("填充")]))
        .collect();
    assert_eq!(finish_payload(&transcript), None, "千条零 notice → None");

    // subtype 大小写漂移 → None（字面量精确匹配）
    let transcript = vec![notice(
        0,
        "Finish_Step",
        evaluator_notice_payload("pass", "报告", "项"),
    )];
    assert_eq!(
        finish_payload(&transcript),
        None,
        "subtype 大小写漂移不命中（精确匹配）"
    );
}

#[test]
fn finish_payload提取实现单点() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/transcript.rs"),
    )
    .expect("读取 transcript.rs");
    assert_eq!(
        own.matches("fn finish_payload").count(),
        1,
        "transcript.rs 内提取实现恰一处"
    );
    assert!(
        own.contains("from_value(payload.clone())"),
        "提取经 serde_json::from_value 并容忍失败（.ok()）"
    );

    // worker.rs 只调不另写：零第二份 `from_value::<FinishPayload>` 提取
    let worker = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../infra/agent/src/worker.rs"),
    )
    .expect("读取 worker.rs");
    assert_eq!(
        worker.matches("finish_payload").count(),
        1,
        "worker.rs 调用恰一处"
    );
    assert!(
        !worker.contains("from_value::<FinishPayload>"),
        "worker.rs 内零第二份提取实现"
    );
}
