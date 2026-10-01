//! `transcript`（转录提取器）的单元测试：final_assistant_text 末条
//! assistant Message 多文本块按序拼接（verdict / 决策 JSON 载体保真）、
//! 工具块不参与拼接、无 assistant Message / 空转录显式缺席 None。无进程
//! 边界依赖：密封 AgentEvent 转录以内存 Vec fixture 构造。

use agent::{AgentBlock, AgentEvent, AgentEventKind, AgentMessageRole};

use crate::transcript::final_assistant_text;

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
