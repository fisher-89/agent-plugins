//! store 转录 → rig 对话历史重建纯函数。
//!
//! 重建口径：仅取顶层（`parent_tool_use_id = None`）非 Raw 事件——Raw /
//! RunStarted / SystemNotice 不进对话史，子代理事件（parent 归因非空）压
//! 平丢弃（保真度缺口已在能力 spec 留痕）。块映射：Text → 文本、Thinking →
//! reasoning、ToolUse → tool call、ToolResult → tool result（`sdk-` 前缀
//! 会话的归属校验由 [`owns_session`] 承载，前缀即引擎归属标记）。
//!
//! 重建空历史视同会话缺失（调用方以启动失败显式拒绝，不空转）。

use std::collections::HashMap;

use agent::{AgentBlock, AgentEvent, AgentEventKind};
use rig_core::message::{AssistantContent, Message, Reasoning, ToolResultContent, UserContent};

/// sdk 会话 id 前缀（引擎归属标记）：`sdk-<进程内计数>-<毫秒时戳>`。
pub const SESSION_PREFIX: &str = "sdk-";

/// 会话 id 是否属 sdk 引擎产出（resume 归属校验；非前缀显式拒绝）。
pub fn owns_session(session_id: &str) -> bool {
    session_id.starts_with(SESSION_PREFIX)
}

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
            continue; // RunStarted / SystemNotice / Raw 不进对话史
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
