use rig_core::message::{AssistantContent, Message, ToolResultContent, UserContent};

use agent::{AgentBlock, AgentEvent, AgentEventKind, AgentMessageRole};

use crate::sdk::resume::rebuild;

// ---------------------------------------------------------------------------
// fixture 构造
// ---------------------------------------------------------------------------

fn event(seq: u64, kind: AgentEventKind) -> AgentEvent {
    AgentEvent::stamp(seq, kind)
}

fn user_text(seq: u64, text: &str) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::User,
            blocks: vec![AgentBlock::Text {
                text: text.to_owned(),
            }],
            parent_tool_use_id: None,
        },
    )
}

fn assistant_text(seq: u64, text: &str) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: vec![AgentBlock::Text {
                text: text.to_owned(),
            }],
            parent_tool_use_id: None,
        },
    )
}

fn tool_use_block(id: &str, name: &str) -> AgentBlock {
    AgentBlock::ToolUse {
        id: id.to_owned(),
        name: name.to_owned(),
        input: serde_json::json!({ "path": "README.md" }),
    }
}

fn tool_result_event(seq: u64, id: &str, content: &str, is_error: bool) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::User,
            blocks: vec![AgentBlock::ToolResult {
                id: id.to_owned(),
                content: content.to_owned(),
                is_error,
            }],
            parent_tool_use_id: None,
        },
    )
}

/// 三轮全史转录（user / assistant 密封往返 x3）。
fn three_rounds() -> Vec<AgentEvent> {
    vec![
        user_text(0, "第一问"),
        assistant_text(1, "第一答"),
        user_text(2, "第二问"),
        assistant_text(3, "第二答"),
        user_text(4, "第三问"),
        assistant_text(5, "第三答"),
    ]
}

// ---------------------------------------------------------------------------
// 全史重建：多轮往返（第 N 轮重建史含全部前 N-1 轮——AC-3 正解断言）
// ---------------------------------------------------------------------------

#[test]
fn 三轮全史转录重建含全部往返且逐条保序() {
    let history = rebuild(&three_rounds()).expect("全史重建应成功");

    assert_eq!(
        history.len(),
        6,
        "三轮往返全量还原（user/assistant 各三则）——第一轮不再丢失"
    );
    for (index, message) in history.iter().enumerate() {
        let expected_role = if index % 2 == 0 { "user" } else { "assistant" };
        let expected_text = format!(
            "第{}{}",
            ["一", "二", "三"][index / 2],
            if expected_role == "user" {
                "问"
            } else {
                "答"
            }
        );
        match message {
            Message::User { content } => {
                assert_eq!(expected_role, "user", "保序错位: {index}");
                assert!(
                    matches!(&content[0], UserContent::Text(text) if text.text == expected_text),
                    "第 {index} 条文本保真，实际: {:?}",
                    content[0]
                );
            }
            Message::Assistant { content, id } => {
                assert_eq!(expected_role, "assistant", "保序错位: {index}");
                assert!(id.is_none(), "重建史不携带 provider 消息 id");
                assert!(
                    matches!(&content[0], AssistantContent::Text(text) if text.text == expected_text),
                    "第 {index} 条文本保真，实际: {:?}",
                    content[0]
                );
            }
            other => panic!("重建史仅含 user/assistant 消息，实际: {other:?}"),
        }
    }
}

#[test]
fn 链式续会话的第三轮重建史含全部前两轮往返() {
    // AC-3 场景化断言：第 N 轮（N=3）续会话时重建史含前 N-1 轮全部往返
    let history = rebuild(&three_rounds()).expect("重建应成功");
    let texts: Vec<String> = history
        .iter()
        .filter_map(|message| match message {
            Message::User { content } => content.first().and_then(|item| match item {
                UserContent::Text(text) => Some(text.text.clone()),
                _ => None,
            }),
            Message::Assistant { content, .. } => content.first().and_then(|item| match item {
                AssistantContent::Text(text) => Some(text.text.clone()),
                _ => None,
            }),
            _ => None,
        })
        .collect();

    assert_eq!(
        texts,
        vec!["第一问", "第一答", "第二问", "第二答", "第三问", "第三答"],
        "首轮问答在重建史中在场（链式丢上下文根因修复）"
    );
}

// ---------------------------------------------------------------------------
// 密封 Message 多块重建（块保真不拆分）
// ---------------------------------------------------------------------------

#[test]
fn 单条密封message收三块时重建为单条assistant消息不拆分() {
    let transcript = vec![event(
        0,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: vec![
                AgentBlock::Text {
                    text: "结论先行".to_owned(),
                },
                AgentBlock::Thinking {
                    thinking: "推理过程".to_owned(),
                },
                tool_use_block("tu_1", "read"),
            ],
            parent_tool_use_id: None,
        },
    )];

    let history = rebuild(&transcript).expect("重建应成功");
    assert_eq!(history.len(), 1, "恰一条 rig assistant 消息（不拆分）");
    let Message::Assistant { content, .. } = &history[0] else {
        panic!("应为 assistant 消息");
    };
    assert_eq!(content.len(), 3, "三块全部收进（Text+Thinking+ToolUse）");
    assert!(matches!(
        &content[0],
        AssistantContent::Text(text) if text.text == "结论先行"
    ));
    assert!(
        matches!(&content[1], AssistantContent::Reasoning(reasoning) if reasoning.display_text() == "推理过程"),
        "Thinking → reasoning 块保真"
    );
    assert!(matches!(
        &content[2],
        AssistantContent::ToolCall(tool_call)
            if tool_call.id.as_str() == "tu_1"
                && tool_call.function.name == "read"
                && tool_call.function.arguments == serde_json::json!({ "path": "README.md" })
    ));
}

// ---------------------------------------------------------------------------
// ToolResult 成对回灌（is_error 结果与工具名回溯保真）
// ---------------------------------------------------------------------------

#[test]
fn tool_result成对回灌且工具名自先行tool_use回溯() {
    let transcript = vec![
        event(
            0,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![tool_use_block("tu_7", "grep")],
                parent_tool_use_id: None,
            },
        ),
        tool_result_event(1, "tu_7", "命中一行", false),
    ];

    let history = rebuild(&transcript).expect("重建应成功");
    assert_eq!(history.len(), 2, "assistant + tool result 成对回灌");
    let Message::User { content } = &history[1] else {
        panic!("ToolResult 应重建为 user 消息");
    };
    assert!(
        matches!(
            &content[0],
            UserContent::ToolResult(result)
                if result.call.as_str() == "tu_7"
                    && result.name == "grep"
                    && matches!(&result.content[0], ToolResultContent::Text(text) if text.text == "命中一行")
        ),
        "工具名回溯自先行 ToolUse（id → name 映射），实际: {:?}",
        content[0]
    );
}

#[test]
fn is_error结果以工具执行失败前缀回灌且miss名字落空串() {
    // is_error 结果
    let errored = vec![
        event(
            0,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![tool_use_block("tu_9", "write")],
                parent_tool_use_id: None,
            },
        ),
        tool_result_event(1, "tu_9", "磁盘已满", true),
    ];
    let history = rebuild(&errored).expect("重建应成功");
    let Message::User { content } = &history[1] else {
        panic!("ToolResult 应重建为 user 消息");
    };
    assert!(
        matches!(
            &content[0],
            UserContent::ToolResult(result)
                if matches!(&result.content[0], ToolResultContent::Text(text) if text.text == "工具执行失败: 磁盘已满")
        ),
        "is_error 结果回灌「工具执行失败: 」前缀保真"
    );

    // 先行 ToolUse 缺席：名字回溯 miss 落空串（openai result 面不消费名字）
    let orphan = vec![tool_result_event(0, "tu_missing", "孤儿结果", false)];
    let history = rebuild(&orphan).expect("重建应成功");
    let Message::User { content } = &history[0] else {
        panic!("孤儿 ToolResult 应重建为 user 消息");
    };
    assert!(
        matches!(
            &content[0],
            UserContent::ToolResult(result) if result.name.is_empty()
        ),
        "名字 miss 落空串不炸重建"
    );
}

// ---------------------------------------------------------------------------
// 非对话事件丢弃
// ---------------------------------------------------------------------------

#[test]
fn 非对话密封事件与子代理归因事件不进对话史() {
    let transcript = vec![
        event(
            0,
            AgentEventKind::RunStarted {
                model: Some("gpt-x".to_owned()),
                session_id: Some("sdk-1".to_owned()),
                tools: Vec::new(),
                mcp_servers: Vec::new(),
            },
        ),
        user_text(1, "提问"),
        assistant_text(2, "回应"),
        event(
            3,
            AgentEventKind::TurnDone {
                subtype: "success".to_owned(),
                is_error: false,
                num_turns: Some(1),
                duration_ms: Some(10),
                cost_usd: None,
                usage: serde_json::Value::Null,
                session_id: Some("sdk-1".to_owned()),
            },
        ),
        event(
            4,
            AgentEventKind::SystemNotice {
                subtype: "api_retry".to_owned(),
                payload: serde_json::json!({ "attempt": 2 }),
            },
        ),
        event(
            5,
            AgentEventKind::Raw {
                event_type: "sdk_stream".to_owned(),
                raw_json: "{\"raw\":true}".to_owned(),
            },
        ),
        // 子代理归因密封事件（parent_tool_use_id 非空）压平丢弃
        event(
            6,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![AgentBlock::Text {
                    text: "子代理输出".to_owned(),
                }],
                parent_tool_use_id: Some("tu_1".to_owned()),
            },
        ),
        // 增量词汇同理不进对话史（重建源是密封转录）
        event(
            7,
            AgentEventKind::MessageDelta {
                parent_tool_use_id: None,
                delta: agent::AgentDelta::Text {
                    text: "增量".to_owned(),
                },
            },
        ),
    ];

    let history = rebuild(&transcript).expect("重建应成功");
    assert_eq!(history.len(), 2, "仅顶层非 Raw 密封对话事件进史");
    assert!(matches!(&history[0], Message::User { .. }));
    assert!(matches!(&history[1], Message::Assistant { .. }));
}

#[test]
fn tool_role结果消息归并回user位重建() {
    // 引擎归一后的 tool role（工具结果管道）重建时归并回 rig user 位——
    // rig 方言要求 result 挂 user 消息，此即协议 → rig 的反向归一
    let transcript = vec![
        event(
            0,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![tool_use_block("tu_9", "read")],
                parent_tool_use_id: None,
            },
        ),
        event(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Tool,
                blocks: vec![AgentBlock::ToolResult {
                    id: "tu_9".to_owned(),
                    content: "文件内容".to_owned(),
                    is_error: false,
                }],
                parent_tool_use_id: None,
            },
        ),
    ];
    let history = rebuild(&transcript).expect("重建应成功");
    assert_eq!(history.len(), 2, "assistant + tool 结果成对回灌");
    let Message::User { content } = &history[1] else {
        panic!("tool role 结果应重建为 rig user 消息");
    };
    assert!(
        matches!(
            &content[0],
            UserContent::ToolResult(result)
                if result.call.as_str() == "tu_9"
                    && result.name == "read"
                    && matches!(&result.content[0], ToolResultContent::Text(text) if text.text == "文件内容")
        ),
        "tool role 归并回 user 位且工具名回溯保真，实际: {:?}",
        content[0]
    );
}

// ---------------------------------------------------------------------------
// 空史显式失败
// ---------------------------------------------------------------------------

#[test]
fn 空史四形态重建返回err视同会话缺失() {
    // 空转录
    let empty: Vec<AgentEvent> = Vec::new();
    let error = rebuild(&empty).expect_err("空转录必须 Err");
    assert!(
        error.contains("重建历史为空"),
        "空史 Err 语义保留，实际: {error}"
    );

    // 仅 Raw
    let only_raw = vec![event(
        0,
        AgentEventKind::Raw {
            event_type: "x".to_owned(),
            raw_json: "{}".to_owned(),
        },
    )];
    assert!(rebuild(&only_raw).is_err(), "仅 Raw 必须 Err");

    // 仅 RunStarted
    let only_started = vec![event(
        0,
        AgentEventKind::RunStarted {
            model: None,
            session_id: None,
            tools: Vec::new(),
            mcp_servers: Vec::new(),
        },
    )];
    assert!(rebuild(&only_started).is_err(), "仅 RunStarted 必须 Err");

    // 仅子代理归因密封事件
    let only_subagent = vec![event(
        0,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: Vec::new(),
            parent_tool_use_id: Some("tu_1".to_owned()),
        },
    )];
    assert!(rebuild(&only_subagent).is_err(), "仅子代理必须 Err");
}

// ---------------------------------------------------------------------------
// 千级长转录保真（会话全史规模护栏）
// ---------------------------------------------------------------------------

#[test]
fn 千级长转录重建不丢消息不乱序() {
    // 1000 枚密封事件（500 轮往返）
    let mut transcript = Vec::with_capacity(1000);
    for round in 0..500 {
        transcript.push(user_text(round * 2, &format!("问{round}")));
        transcript.push(assistant_text(round * 2 + 1, &format!("答{round}")));
    }
    assert_eq!(transcript.len(), 1000);

    let history = rebuild(&transcript).expect("千级重建应成功");
    assert_eq!(history.len(), 1000, "不丢消息");

    // 保序抽查：首、中、尾三处
    for index in [0usize, 499, 998] {
        let round = index / 2;
        let expected = format!("{}{round}", if index % 2 == 0 { "问" } else { "答" });
        let text = match &history[index] {
            Message::User { content } => match &content[0] {
                UserContent::Text(text) => text.text.clone(),
                other => panic!("应为文本，实际: {other:?}"),
            },
            Message::Assistant { content, .. } => match &content[0] {
                AssistantContent::Text(text) => text.text.clone(),
                other => panic!("应为文本，实际: {other:?}"),
            },
            other => panic!("应为对话消息，实际: {other:?}"),
        };
        assert_eq!(text, expected, "index={index} 保序");
    }
}
