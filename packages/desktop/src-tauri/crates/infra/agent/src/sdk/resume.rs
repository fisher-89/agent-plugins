use std::collections::HashMap;

use agent::{AgentBlock, AgentEvent, AgentEventKind};
use rig_core::message::{AssistantContent, Message, Reasoning, ToolResultContent, UserContent};

/// sdk 引擎侧会话 id 铸造前缀：`sdk-<进程内计数>-<毫秒时戳>`（每轮铸造，
/// 经 RunStarted / TurnDone 上报由内核记双 id 映射；不再承担归属校验）。
pub const SESSION_PREFIX: &str = "sdk-";

/// 重建 rig 对话历史：转录事件序列 → user / assistant 消息序列（ToolUse ↔
/// ToolResult 成对回灌）。空历史返回 `Err`（视同会话缺失）。
pub fn rebuild(events: &[AgentEvent]) -> Result<Vec<Message>, String> {
    let mut history: Vec<Message> = Vec::new();
    // tool_use id → 工具名（rig 的 tool result 必带执行工具名；转录的
    // ToolResult 块只有 id，名字自同会话先行的 ToolUse 块回溯）
    let mut tool_names: HashMap<String, String> = HashMap::new();
    for event in events {
        let AgentEventKind::Message {
            role,
            blocks,
            parent_tool_use_id,
        } = &event.kind
        else {
            continue; // RunStarted / SystemNotice / Raw / delta 不进对话史
        };
        if parent_tool_use_id.is_some() {
            continue; // 子代理事件压平丢弃
        }
        match role.as_str() {
            "user" => {
                let content = user_content(blocks, &mut tool_names);
                if !content.is_empty() {
                    history.push(Message::User { content });
                }
            }
            "assistant" => {
                let content = assistant_content(blocks, &mut tool_names);
                if !content.is_empty() {
                    history.push(Message::Assistant { id: None, content });
                }
            }
            _ => {} // 未知 role 跳过（不炸重建）
        }
    }
    if history.is_empty() {
        Err("重建历史为空（无有效顶层转录）".to_owned())
    } else {
        Ok(history)
    }
}

/// user 消息块映射：Text → 文本、ToolResult → tool result（名字回溯自
/// 先行 ToolUse，miss 落空串——openai chat completions 的 result 面不消费
/// 名字，仅部分方言作键）。
fn user_content(
    blocks: &[AgentBlock],
    tool_names: &mut HashMap<String, String>,
) -> Vec<UserContent> {
    blocks
        .iter()
        .filter_map(|block| match block {
            AgentBlock::Text { text } => Some(UserContent::text(text.clone())),
            AgentBlock::ToolResult {
                id,
                content,
                is_error,
            } => {
                let name = tool_names.get(id).cloned().unwrap_or_default();
                let body = if *is_error {
                    format!("工具执行失败: {content}")
                } else {
                    content.clone()
                };
                Some(UserContent::tool_result(
                    id.clone(),
                    name,
                    vec![ToolResultContent::text(body)],
                ))
            }
            _ => None, // user 消息内的 Thinking / ToolUse 非法形态跳过
        })
        .collect()
}

/// assistant 消息块映射：Text → 文本、Thinking → reasoning、ToolUse →
/// tool call（登记 id → 名字供后续 ToolResult 回溯）。
fn assistant_content(
    blocks: &[AgentBlock],
    tool_names: &mut HashMap<String, String>,
) -> Vec<AssistantContent> {
    blocks
        .iter()
        .filter_map(|block| match block {
            AgentBlock::Text { text } => Some(AssistantContent::text(text.clone())),
            AgentBlock::Thinking { thinking } => {
                Some(AssistantContent::Reasoning(Reasoning::new(thinking)))
            }
            AgentBlock::ToolUse { id, name, input } => {
                tool_names.insert(id.clone(), name.clone());
                Some(AssistantContent::tool_call(
                    id.clone(),
                    name.clone(),
                    input.clone(),
                ))
            }
            _ => None, // assistant 消息内的 ToolResult 非法形态跳过
        })
        .collect()
}
