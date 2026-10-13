//! `transcript`（转录提取器）的单元测试：final_assistant_text 末条
//! assistant Message 多文本块按序拼接（verdict / 决策 JSON 载体保真）、
//! 工具块不参与拼接、无 assistant Message / 空转录显式缺席 None。无进程
//! 边界依赖：密封 AgentEvent 转录以内存 Vec fixture 构造。

use agent::{AgentBlock, AgentEvent, AgentEventKind, AgentMessageRole};

use crate::transcript::{failure_reason, final_assistant_text};

// ---------------------------------------------------------------------------
// 装置：密封事件 fixture（Message 事件 + ToolUse / ToolResult / Text 块）
// ---------------------------------------------------------------------------

fn message(seq: u64, role: AgentMessageRole, blocks: Vec<AgentBlock>) -> AgentEvent {
    AgentEvent::stamp(
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
        seq,
        AgentEventKind::SystemNotice {
            subtype: subtype.to_owned(),
            payload,
        },
    )
}

fn turn_done(seq: u64, subtype: &str, is_error: bool) -> AgentEvent {
    AgentEvent::stamp(
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
