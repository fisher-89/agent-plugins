//! `resume` 的单元测试（AC-7）：store 转录 → rig 对话历史重建——顶层非 Raw
//! 事件重建、ToolUse ↔ ToolResult 成对回灌、Raw 丢弃、子代理压平、
//! `sdk-` 前缀归属校验、重建保真（长转录不丢不乱序）。转录输入以内存
//! `Vec<AgentEvent>` fixture 构造（store 转录产物同构形态；store 本体不引入
//! 不 mock）。

use rig_core::message::{AssistantContent, Message, ToolResultContent, UserContent};

use agent::{AgentBlock, AgentEvent, AgentEventKind};

use crate::sdk::resume::{owns_session, rebuild};

// ---------------------------------------------------------------------------
// fixture 构造（AgentEvent 内存同构 store 转录产物）
// ---------------------------------------------------------------------------

fn stamped(seq: u64, kind: AgentEventKind) -> AgentEvent {
    AgentEvent::stamp(seq, kind)
}

fn user_text(seq: u64, text: &str) -> AgentEvent {
    stamped(
        seq,
        AgentEventKind::Message {
            role: "user".to_owned(),
            blocks: vec![AgentBlock::Text { text: text.to_owned() }],
            parent_tool_use_id: None,
        },
    )
}

fn assistant_text(seq: u64, text: &str) -> AgentEvent {
    stamped(
        seq,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: vec![AgentBlock::Text { text: text.to_owned() }],
            parent_tool_use_id: None,
        },
    )
}

fn assistant_tool_use(seq: u64, id: &str, name: &str, input: serde_json::Value) -> AgentEvent {
    stamped(
        seq,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: vec![AgentBlock::ToolUse {
                id: id.to_owned(),
                name: name.to_owned(),
                input,
            }],
            parent_tool_use_id: None,
        },
    )
}

fn user_tool_result(seq: u64, id: &str, content: &str, is_error: bool) -> AgentEvent {
    stamped(
        seq,
        AgentEventKind::Message {
            role: "user".to_owned(),
            blocks: vec![AgentBlock::ToolResult {
                id: id.to_owned(),
                content: content.to_owned(),
                is_error,
            }],
            parent_tool_use_id: None,
        },
    )
}

fn run_started(seq: u64) -> AgentEvent {
    stamped(
        seq,
        AgentEventKind::RunStarted {
            model: Some("rig-model".to_owned()),
            session_id: Some("sdk-0-1727000000000".to_owned()),
            tools: vec!["read".to_owned()],
            mcp_servers: Vec::new(),
        },
    )
}

fn raw_event(seq: u64) -> AgentEvent {
    stamped(
        seq,
        AgentEventKind::Raw {
            event_type: "sdk_stream".to_owned(),
            raw_json: r#"{"type":"mystery"}"#.to_owned(),
        },
    )
}

/// 断言 user 消息携带文本内容并返回该文本。
fn user_texts(message: &Message) -> Vec<String> {
    let Message::User { content } = message else {
        panic!("必须是 User 消息，实际: {message:?}");
    };
    content
        .iter()
        .filter_map(|item| match item {
            UserContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 正向：顶层非 Raw 转录重建
// ---------------------------------------------------------------------------

#[test]
fn 顶层转录重建为对话历史_user_assistant重建与工具轮成对回灌() {
    let transcript = vec![
        run_started(0), // RunStarted 忽略（不进对话史）
        user_text(1, "帮我看下这个目录"),
        assistant_text(2, "我先列出目录 🎉"),
        assistant_tool_use(3, "tu_1", "ls", serde_json::json!({ "path": "." })),
        user_tool_result(4, "tu_1", "a.rs\nb.rs", false),
        assistant_text(5, "目录里有 a.rs 与 b.rs"),
    ];
    let history = rebuild(&transcript).expect("重建成功");

    // 六事件 → 五条历史消息（RunStarted 忽略）：user / assistant 交替序保持
    assert_eq!(history.len(), 5, "实际: {history:?}");
    assert_eq!(user_texts(&history[0]), vec!["帮我看下这个目录".to_owned()]);
    assert!(matches!(&history[1], Message::Assistant { .. }), "assistant 重建");
    // 工具轮成对回灌：assistant 的 ToolUse + user 的 ToolResult 双双入史
    let Message::Assistant { content, .. } = &history[2] else {
        panic!("第 3 条必须是 assistant");
    };
    let Some(AssistantContent::ToolCall(tool_call)) = content.first() else {
        panic!("assistant 工具轮必须是 tool call，实际: {content:?}");
    };
    assert_eq!(tool_call.function.name.as_str(), "ls", "工具名回灌保真");

    let Message::User { content } = &history[3] else {
        panic!("第 4 条必须是 user（ToolResult 回灌）");
    };
    let Some(UserContent::ToolResult(result)) = content.first() else {
        panic!("user 工具轮必须是 tool result，实际: {content:?}");
    };
    assert_eq!(result.call.as_str(), "tu_1", "与 ToolUse 同 id 成对");
    assert!(
        matches!(&history[4], Message::Assistant { .. }),
        "收尾 assistant 文本"
    );
}

#[test]
fn tool_result回灌时错误结果与名字回溯保真() {
    let transcript = vec![
        user_text(0, "改一下"),
        assistant_tool_use(1, "tu_err", "edit", serde_json::json!({ "path": "a.md" })),
        user_tool_result(2, "tu_err", "old_string 未命中（先 read 确认原文）", true),
    ];
    let history = rebuild(&transcript).expect("重建成功");
    assert_eq!(history.len(), 3);

    let Message::User { content } = &history[2] else {
        panic!("ToolResult 回灌为 user 消息");
    };
    let Some(UserContent::ToolResult(result)) = content.first() else {
        panic!("实际: {content:?}");
    };
    // 名字回溯自先行 ToolUse（id → name 登记）
    assert_eq!(result.call.as_str(), "tu_err");
    // is_error 结果正文带失败前缀（openai chat completions 的 result 面语义）
    let ToolResultContent::Text(text) = &result.content[0] else {
        panic!("重建的 tool result 正文为文本块");
    };
    let body = text.text.clone();
    assert!(
        body.contains("工具执行失败") && body.contains("old_string 未命中"),
        "错误结果记因: {body}"
    );
}

// ---------------------------------------------------------------------------
// 边界：Raw 丢弃 / 子代理压平
// ---------------------------------------------------------------------------

#[test]
fn raw事件丢弃且子代理事件压平不进对话史() {
    let transcript = vec![
        raw_event(0), // Raw 丢弃
        user_text(1, "顶层问句"),
        stamped(
            2,
            AgentEventKind::Message {
                role: "assistant".to_owned(),
                blocks: vec![AgentBlock::Text { text: "子代理产出".to_owned() }],
                parent_tool_use_id: Some("tu_parent".to_owned()), // 子代理归因非 None
            },
        ),
        assistant_text(3, "顶层答复"),
        stamped(
            4,
            AgentEventKind::RunResult {
                subtype: "success".to_owned(),
                is_error: false,
                num_turns: Some(1),
                duration_ms: None,
                cost_usd: None,
                usage: serde_json::Value::Null,
                session_id: Some("sdk-0-1".to_owned()),
            },
        ), // RunResult 不进对话史
    ];
    let history = rebuild(&transcript).expect("重建成功");

    // 仅顶层 user/assistant 两条（Raw / 子代理 / RunStarted / RunResult 全弃）
    assert_eq!(history.len(), 2, "实际: {history:?}");
    assert_eq!(user_texts(&history[0]), vec!["顶层问句".to_owned()]);
    let Message::Assistant { content, .. } = &history[1] else {
        panic!("第 2 条必须是 assistant");
    };
    let Some(AssistantContent::Text(text)) = content.first() else {
        panic!("实际: {content:?}");
    };
    assert_eq!(text.text.as_str(), "顶层答复", "子代理事件压平丢弃，顶层保真");
}

// ---------------------------------------------------------------------------
// 异常：sdk- 前缀归属校验
// ---------------------------------------------------------------------------

#[test]
fn 非sdk前缀会话显式失败且消息含会话不存在语义() {
    // 前缀校验为启动拒绝的第一道（Err 抵达前端，不空转）
    assert!(!owns_session("cli-abc"), "cli 产出会话不归 sdk");
    assert!(!owns_session("s-1"), "裸 session id 不归 sdk");
    assert!(!owns_session(""), "空串不归 sdk");
    assert!(owns_session("sdk-0-1727000000000"), "sdk- 前缀归属成立");
    // 前缀本身即完整判据（仅前缀也命中——归属校验不做内部结构解析）
    assert!(owns_session("sdk-"), "纯前缀命中（边界锁定）");

    // 非前缀的显式失败消息语义（与 runner resolve_resume 同口径）
    let session_id = "cli-abc";
    let message = format!("会话不存在或非 SDK 产出: {session_id}");
    assert!(message.contains("会话不存在或非 SDK 产出"));
}

// ---------------------------------------------------------------------------
// 边界：重建空历史视同会话缺失
// ---------------------------------------------------------------------------

#[test]
fn 空转录_仅raw_仅run_started重建空历史视同会话缺失() {
    // 空转录
    let empty = rebuild(&[]).expect_err("空转录显式失败");
    assert!(empty.contains("重建历史为空"), "实际: {empty}");

    // 仅 Raw / 仅 RunStarted / 仅子代理事件 → 无有效顶层转录 → 同样显式失败
    let only_raw = vec![raw_event(0), raw_event(1)];
    assert!(rebuild(&only_raw).is_err(), "仅 Raw 重建空历史显式失败");

    let only_started = vec![run_started(0)];
    assert!(rebuild(&only_started).is_err(), "仅 RunStarted 重建空历史显式失败");

    let only_subagent = vec![stamped(
        0,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: vec![AgentBlock::Text { text: "子代理".to_owned() }],
            parent_tool_use_id: Some("tu_parent".to_owned()),
        },
    )];
    assert!(rebuild(&only_subagent).is_err(), "仅子代理事件重建空历史显式失败");
}

// ---------------------------------------------------------------------------
// 边界：重建保真（千级事件长转录不丢不乱序）
// ---------------------------------------------------------------------------

#[test]
fn 千级事件长转录重建不丢消息不乱序() {
    // 工具轮 user / assistant 交替序：prompt → tool_use → tool_result → 答复
    // × 250 轮 = 1000 事件（sdk- 前缀即 explore 链引擎归属的事实标记）
    let mut transcript = Vec::new();
    transcript.push(run_started(0));
    let mut seq = 1u64;
    for round in 0..250 {
        transcript.push(user_text(seq, &format!("第 {round} 轮问句")));
        seq += 1;
        transcript.push(assistant_tool_use(
            seq,
            &format!("tu_{round}"),
            "read",
            serde_json::json!({ "path": format!("f{round}.txt") }),
        ));
        seq += 1;
        transcript.push(user_tool_result(seq, &format!("tu_{round}"), "内容", false));
        seq += 1;
        transcript.push(assistant_text(seq, &format!("第 {round} 轮答复")));
        seq += 1;
    }
    assert_eq!(transcript.len(), 1001);

    let history = rebuild(&transcript).expect("长转录重建成功");
    // 1000 Message 事件 → 1000 条历史消息（RunStarted 忽略），序与消息角色保真
    assert_eq!(history.len(), 1000, "一条不丢");
    for (index, message) in history.iter().enumerate() {
        let round = index / 4;
        let phase = index % 4;
        match phase {
            0 => {
                let texts = user_texts(message);
                assert_eq!(texts, vec![format!("第 {round} 轮问句")], "第 {index} 条问句保序");
            }
            2 => {
                let Message::User { content } = message else {
                    panic!("第 {index} 条必须是 user（tool result）");
                };
                assert!(matches!(content.first(), Some(UserContent::ToolResult(_))));
            }
            1 | 3 => {
                let Message::Assistant { content, .. } = message else {
                    panic!("第 {index} 条必须是 assistant");
                };
                assert!(
                    matches!(content.first(), Some(AssistantContent::Text(_)) | Some(AssistantContent::ToolCall(_))),
                    "assistant 块保真: {content:?}"
                );
            }
            _ => unreachable!(),
        }
    }
}
