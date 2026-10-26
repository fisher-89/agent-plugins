use rig::message::{AssistantContent, Message, ToolResultContent, UserContent};

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
                citations: None,
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
                citations: None,
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
                    citations: None,
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
    // tool_use 无结果回灌 → 重建侧补一条中断占位结果闭合配对
    assert_eq!(history.len(), 2, "assistant 消息 + 中断占位结果");
    let Message::Assistant { content, .. } = &history[0] else {
        panic!("应为 assistant 消息");
    };
    assert_eq!(content.len(), 3, "三块全部收进（Text+Thinking+ToolUse）");
    assert!(matches!(
        &content[0],
        AssistantContent::Text(text) if text.text == "结论先行"
    ));
    assert!(
        matches!(&content[1], AssistantContent::Reasoning(reasoning)
            if reasoning.open(reasoning.issuer()).is_some_and(|value| value.display_text() == "推理过程")),
        "Thinking → reasoning 块保真（本地发行方密封可开）"
    );
    assert!(matches!(
        &content[2],
        AssistantContent::ToolCall(tool_call)
            if tool_call.id.wire().as_ref() == "tu_1"
                && tool_call.function.name.as_str() == "read"
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
                if result.call.wire().as_ref() == "tu_7"
                    && result.name.as_str() == "grep"
                    && matches!(&result.content[0], ToolResultContent::Text(text) if text.text == "命中一行")
        ),
        "工具名回溯自先行 ToolUse（id → name 映射），实际: {:?}",
        content[0]
    );
}

#[test]
fn is_error结果以工具执行失败前缀回灌且孤儿结果剔除() {
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

    // 先行 ToolUse 缺席（孤儿结果）：0.43 起 ToolName 不可空且 provider 侧
    // 要求配对——孤儿结果自重建史剔除（与请求史防线 sweep_orphans 同口径）
    let orphan = vec![
        user_text(0, "任务书"),
        tool_result_event(1, "tu_missing", "孤儿结果", false),
    ];
    let history = rebuild(&orphan).expect("重建应成功");
    assert_eq!(
        history.len(),
        1,
        "孤儿 ToolResult 剔除（仅任务书 user 消息存续），实际: {history:?}"
    );
    assert!(
        !matches!(&history[0], Message::User { content }
            if content.iter().any(|item| matches!(item, UserContent::ToolResult(_)))),
        "孤儿结果不进重建史（无空名回灌形态）"
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
                    citations: None,
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
                if result.call.wire().as_ref() == "tu_9"
                    && result.name.as_str() == "read"
                    && matches!(&result.content[0], ToolResultContent::Text(text) if text.text == "文件内容")
        ),
        "tool role 归并回 user 位且工具名回溯保真，实际: {:?}",
        content[0]
    );
}

#[test]
fn 同一assistant的多个工具结果重建为紧随其后的一条user消息() {
    // 生产转录形态：单条 assistant 密封含两个 tool_use，结果逐条 tool 事件落库
    let transcript = vec![
        user_text(0, "任务书"),
        event(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![
                    tool_use_block("tu_a", "grep"),
                    tool_use_block("tu_b", "read"),
                ],
                parent_tool_use_id: None,
            },
        ),
        event(
            2,
            AgentEventKind::Message {
                role: AgentMessageRole::Tool,
                blocks: vec![AgentBlock::ToolResult {
                    id: "tu_a".to_owned(),
                    content: "命中一行".to_owned(),
                    is_error: false,
                }],
                parent_tool_use_id: None,
            },
        ),
        event(
            3,
            AgentEventKind::Message {
                role: AgentMessageRole::Tool,
                blocks: vec![AgentBlock::ToolResult {
                    id: "tu_b".to_owned(),
                    content: "文件内容".to_owned(),
                    is_error: false,
                }],
                parent_tool_use_id: None,
            },
        ),
    ];

    let history = rebuild(&transcript).expect("重建应成功");
    assert_eq!(
        history.len(),
        3,
        "user 提示 + assistant + 恰一条结果 user 消息（结果不拆条）"
    );
    let Message::User { content } = &history[2] else {
        panic!("工具结果应重建为紧随 assistant 的 user 消息");
    };
    assert_eq!(content.len(), 2, "两条结果并入同一条 user 消息");
    assert!(
        matches!(&content[0], UserContent::ToolResult(result)
            if result.call.wire().as_ref() == "tu_a" && result.name.as_str() == "grep"),
        "结果保序：tu_a 在前，实际: {:?}",
        content[0]
    );
    assert!(
        matches!(&content[1], UserContent::ToolResult(result)
            if result.call.wire().as_ref() == "tu_b" && result.name.as_str() == "read"),
        "结果保序：tu_b 在后，实际: {:?}",
        content[1]
    );
}

#[test]
fn 结果回灌前停止的尾部调用以合成中断结果闭合配对() {
    let transcript = vec![
        user_text(0, "任务书"),
        event(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![tool_use_block("tu_stop", "bash")],
                parent_tool_use_id: None,
            },
        ),
    ];

    let history = rebuild(&transcript).expect("重建应成功");
    assert_eq!(history.len(), 3, "尾部补恰一条中断结果消息");
    let Message::User { content } = &history[2] else {
        panic!("中断占位结果应为 user 消息");
    };
    assert!(
        matches!(&content[0], UserContent::ToolResult(result)
            if result.call.wire().as_ref() == "tu_stop"
                && result.name.as_str() == "bash"
                && matches!(&result.content[0], ToolResultContent::Text(text) if text.text.contains("中断"))),
        "合成结果与 tool_use 同源配对并标注中断，实际: {:?}",
        content[0]
    );
}

#[test]
fn 中断后新轮提问前先补齐未应答调用() {
    // 停止后用户续问：新 user 文本之前必须已有结果消息，否则 provider 判
    // tool_use 的下一条消息缺 tool_result
    let transcript = vec![
        user_text(0, "任务书"),
        event(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![tool_use_block("tu_stop", "bash")],
                parent_tool_use_id: None,
            },
        ),
        user_text(2, "继续"),
    ];

    let history = rebuild(&transcript).expect("重建应成功");
    assert_eq!(history.len(), 4, "assistant + 中断结果 + 新轮提问");
    assert!(matches!(&history[2], Message::User { content }
        if matches!(&content[0], UserContent::ToolResult(result) if result.call.wire().as_ref() == "tu_stop")));
    assert!(matches!(&history[3], Message::User { content }
        if matches!(&content[0], UserContent::Text(text) if text.text == "继续")));
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

// ---------------------------------------------------------------------------
// 回放展开：WebSearch 块重建两条 AssistantContent + Text.citations 回填
// （AC-5；展示字段不参与重建）
// ---------------------------------------------------------------------------

/// 携 raw 的 use wire（`server_tool_use`）。
fn web_search_use_raw(use_id: &str, query: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "server_tool_use",
        "id": use_id,
        "name": "web_search",
        "input": { "query": query },
        "encrypted_content": "use 加密索引 🎉",
    })
}

/// 携 raw 的 result wire（`web_search_tool_result`）。
fn web_search_result_raw(use_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "web_search_tool_result",
        "tool_use_id": use_id,
        "content": [{ "type": "web_search_result", "url": "https://a/1" }],
        "encrypted_content": "result 加密索引",
    })
}

/// WebSearch 块 fixture（两侧 raw 可分别缺省）。
fn web_search_block(
    use_id: &str,
    raw_use: Option<serde_json::Value>,
    raw_result: Option<serde_json::Value>,
) -> AgentBlock {
    AgentBlock::WebSearch {
        id: use_id.to_owned(),
        query: "今日汇率".to_owned(),
        results: Vec::new(),
        error: None,
        raw_use,
        raw_result,
    }
}

/// 携块清单的 assistant 事件。
fn assistant_blocks(seq: u64, blocks: Vec<AgentBlock>) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks,
            parent_tool_use_id: None,
        },
    )
}

/// 摘第 `index` 条 assistant 消息的 `content`。
fn assistant_content_of(history: &[Message], index: usize) -> &Vec<AssistantContent> {
    let assistants: Vec<&Vec<AssistantContent>> = history
        .iter()
        .filter_map(|message| match message {
            Message::Assistant { content, .. } => Some(content),
            _ => None,
        })
        .collect();
    assistants
        .get(index)
        .unwrap_or_else(|| panic!("第 {index} 条 assistant 消息应在场"))
}

/// 摘 `AssistantContent::Text` 的 `additional_params` 键值。
fn param_of(content: &AssistantContent, key: &str) -> serde_json::Value {
    let AssistantContent::Text(text) = content else {
        panic!("应为 Text 变体，实际: {content:?}");
    };
    text.additional_params
        .as_ref()
        .unwrap_or_else(|| panic!("additional_params 应在场"))
        .get(key)
        .cloned()
        .unwrap_or_else(|| panic!("{key} 键应在场"))
}

#[test]
fn web_search块重建为两条空文本assistant_content且顺序为use先result后() {
    let raw_use = web_search_use_raw("srvtoolu_1", "今日汇率");
    let raw_result = web_search_result_raw("srvtoolu_1");
    let events = vec![
        user_text(0, "帮我查汇率"),
        assistant_blocks(
            1,
            vec![web_search_block(
                "srvtoolu_1",
                Some(raw_use.clone()),
                Some(raw_result.clone()),
            )],
        ),
    ];

    let history = rebuild(&events).expect("重建成功");
    let content = assistant_content_of(&history, 0);

    assert_eq!(content.len(), 2, "一条块最多产两条 AssistantContent");
    for (index, expected) in [(0, &raw_use), (1, &raw_result)] {
        let AssistantContent::Text(text) = &content[index] else {
            panic!("第 {index} 条应为 Text 变体");
        };
        assert_eq!(
            text.text, "",
            "raw 块必须空文本（rig 硬校验「raw 与文本不可共存」）"
        );
        assert_eq!(
            param_of(&content[index], "anthropic_content"),
            *expected,
            "第 {index} 条 additional_params 与原 wire 逐字节相等"
        );
    }
    assert_eq!(
        serde_json::to_string(&param_of(&content[0], "anthropic_content")).expect("序列化成功"),
        serde_json::to_string(&raw_use).expect("序列化成功"),
        "use 侧含 encrypted_content 逐字节保真"
    );
}

#[test]
fn web_search块单侧与双侧缺席时按在场侧产出() {
    let raw_use = web_search_use_raw("srvtoolu_side", "单侧查询");
    let raw_result = web_search_result_raw("srvtoolu_side");

    let use_only = rebuild(&[
        user_text(0, "问"),
        assistant_blocks(1, vec![web_search_block("s1", Some(raw_use), None)]),
    ])
    .expect("重建成功");
    assert_eq!(
        assistant_content_of(&use_only, 0).len(),
        1,
        "仅 use 侧产一条"
    );

    let result_only = rebuild(&[
        user_text(0, "问"),
        assistant_blocks(1, vec![web_search_block("s2", None, Some(raw_result))]),
    ])
    .expect("重建成功");
    assert_eq!(
        assistant_content_of(&result_only, 0).len(),
        1,
        "仅 result 侧产一条"
    );

    // 双 None：0 条 → 该 assistant 消息因 content 空而不入史（既有语义）
    let both_none = rebuild(&[
        user_text(0, "问"),
        assistant_blocks(1, vec![web_search_block("s3", None, None)]),
        assistant_text(2, "答"),
    ])
    .expect("重建成功");
    let assistants = both_none
        .iter()
        .filter(|message| matches!(message, Message::Assistant { .. }))
        .count();
    assert_eq!(assistants, 1, "双 None 的 assistant 消息不入史");
    assert_eq!(
        assistant_content_of(&both_none, 0).len(),
        1,
        "仅保留后一条正文 assistant"
    );
}

#[test]
fn text块citations回填raw数组且展示字段不参与重建() {
    let citation_raw = serde_json::json!({ "url": "https://raw/1", "cited_text": "原始片段" });
    let blocks = vec![AgentBlock::Text {
        text: "正文".to_owned(),
        citations: Some(vec![agent::AgentCitation {
            url: Some("https://display/1".to_owned()),
            title: Some("展示标题".to_owned()),
            cited_text: Some("展示片段".to_owned()),
            raw: citation_raw.clone(),
        }]),
    }];
    let history = rebuild(&[user_text(0, "问"), assistant_blocks(1, blocks)]).expect("重建成功");

    let content = assistant_content_of(&history, 0);
    assert_eq!(content.len(), 1);
    let AssistantContent::Text(text) = &content[0] else {
        panic!("应为 Text 变体");
    };
    assert_eq!(text.text, "正文", "正文原文保真");
    assert_eq!(
        param_of(&content[0], "citations"),
        serde_json::json!([citation_raw]),
        "citations 深等于 [c.raw]（不以 url / title / cited_text 重组）"
    );

    // citations: None → 不落键
    let plain = rebuild(&[user_text(0, "问"), assistant_text(1, "无引用正文")]).expect("重建成功");
    let content = assistant_content_of(&plain, 0);
    let AssistantContent::Text(text) = &content[0] else {
        panic!("应为 Text 变体");
    };
    assert!(
        text.additional_params.is_none(),
        "无引用不落 additional_params（不出现空壳）"
    );
}

#[test]
fn 混合块序展开保原序_text_use_result_text() {
    let raw_use = web_search_use_raw("srvtoolu_mix", "混合查询");
    let raw_result = web_search_result_raw("srvtoolu_mix");
    let events = vec![
        user_text(0, "问"),
        assistant_blocks(
            1,
            vec![
                AgentBlock::Text {
                    text: "前置".to_owned(),
                    citations: None,
                },
                web_search_block("srvtoolu_mix", Some(raw_use), Some(raw_result)),
                AgentBlock::Text {
                    text: "后置".to_owned(),
                    citations: None,
                },
            ],
        ),
    ];

    let history = rebuild(&events).expect("重建成功");
    let content = assistant_content_of(&history, 0);

    assert_eq!(content.len(), 4, "text → use → result → text 共四条");
    let texts: Vec<String> = content
        .iter()
        .map(|item| match item {
            AssistantContent::Text(text) => text.text.clone(),
            other => panic!("应为 Text 变体，实际: {other:?}"),
        })
        .collect();
    assert_eq!(
        texts,
        vec!["前置", "", "", "后置"],
        "顺序保原序（无块重排）"
    );
}

#[test]
fn assistant位tool_result块仍跳过且不误当result侧重建() {
    let events = vec![
        user_text(0, "问"),
        assistant_blocks(
            1,
            vec![
                AgentBlock::ToolResult {
                    id: "tu_1".to_owned(),
                    content: "非法形态".to_owned(),
                    is_error: false,
                },
                AgentBlock::Text {
                    text: "正文".to_owned(),
                    citations: None,
                },
            ],
        ),
    ];

    let history = rebuild(&events).expect("重建成功");
    let content = assistant_content_of(&history, 0);

    assert_eq!(
        content.len(),
        1,
        "assistant 位 ToolResult 不产 AssistantContent（不 panic）"
    );
    assert!(
        matches!(&content[0], AssistantContent::Text(text) if text.text == "正文"),
        "仅正文块入重建"
    );
}

#[test]
fn user位citations不参与重建且user位web_search块跳过不panic() {
    let citation_raw = serde_json::json!({ "url": "https://raw/1" });
    let events = vec![
        event(
            0,
            AgentEventKind::Message {
                role: AgentMessageRole::User,
                blocks: vec![AgentBlock::Text {
                    text: "补充".to_owned(),
                    citations: Some(vec![agent::AgentCitation {
                        url: Some("https://display/1".to_owned()),
                        title: None,
                        cited_text: None,
                        raw: citation_raw,
                    }]),
                }],
                parent_tool_use_id: None,
            },
        ),
        // user 位 WebSearch 块（非法形态）→ 跳过不 panic
        event(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::User,
                blocks: vec![web_search_block(
                    "srvtoolu_user",
                    Some(web_search_use_raw("srvtoolu_user", "q")),
                    None,
                )],
                parent_tool_use_id: None,
            },
        ),
        assistant_text(2, "答"),
    ];

    let history = rebuild(&events).expect("重建成功");

    let Message::User { content } = &history[0] else {
        panic!("首条应为 user 消息");
    };
    assert_eq!(content.len(), 1, "恰一条 UserContent 文本");
    let UserContent::Text(text) = &content[0] else {
        panic!("应为文本项");
    };
    assert_eq!(text.text, "补充", "user 位 citations 丢弃（不参与重建）");
    assert_eq!(history.len(), 2, "user 位 WebSearch 块零产出（消息不新增）");
}

#[test]
fn project与resume往返_重建后additional_params与原始wire逐字节等价() {
    use crate::sdk::web_search::{project_choice, ANTHROPIC_RAW_CONTENT_KEY};
    use rig::message::{AdditionalParams, Text};

    let raw_use = web_search_use_raw("srvtoolu_rt", "往返查询");
    let raw_result = web_search_result_raw("srvtoolu_rt");
    let citation = serde_json::json!({ "url": "https://cite/1", "cited_text": "片段" });

    // 原始 wire（loop 侧 choice 形态）→ project_choice → 密封块序列
    let raw_item = |raw: serde_json::Value| {
        AssistantContent::Text(Text {
            text: String::new(),
            additional_params: AdditionalParams::from_entries([(ANTHROPIC_RAW_CONTENT_KEY, raw)]),
        })
    };
    let choice = vec![
        raw_item(raw_use.clone()),
        raw_item(raw_result.clone()),
        AssistantContent::Text(Text {
            text: "带引用正文".to_owned(),
            additional_params: AdditionalParams::from_entries([(
                "citations",
                serde_json::json!([citation.clone()]),
            )]),
        }),
    ];
    let (blocks, _tool_calls) = project_choice(&choice);
    assert_eq!(blocks.len(), 2, "use + result 收敛一块，正文一块");

    let history = rebuild(&[user_text(0, "问"), assistant_blocks(1, blocks)]).expect("重建成功");
    let content = assistant_content_of(&history, 0);

    assert_eq!(content.len(), 3, "use → result → text 三条");
    assert_eq!(
        param_of(&content[0], "anthropic_content"),
        raw_use,
        "use 侧 raw 逐字节等价"
    );
    assert_eq!(
        param_of(&content[1], "anthropic_content"),
        raw_result,
        "result 侧 raw 逐字节等价"
    );
    assert_eq!(
        param_of(&content[2], "citations"),
        serde_json::json!([citation]),
        "citations 原值回填"
    );
}
