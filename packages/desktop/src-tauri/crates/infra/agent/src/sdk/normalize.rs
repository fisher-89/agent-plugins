//! rig 归一化流项 → [`agent::AgentEventKind`] 归一化纯函数。
//!
//! 输入取 rig 归一化流（上游即 openai chat completions wire：tool_calls
//! 聚合与 reasoning_content 解析复用库语义，不重造线格式累积器）：
//! - 文本 delta → `Message{role=assistant, blocks=[Text]}`
//! - 完整 reasoning 块（supersedes 同 correlator 的增量）→ `Thinking` 块
//! - 完整 tool call → `ToolUse` 块（与 ToolResult 同 id 成对由 loop 兑现）
//! - rig 未建模的 provider 原生项 → `Raw{eventType="sdk_stream"}` 透传
//!   （永不丢事件、永不炸解析，与 CLI jsonl 的 Raw 兜底同哲学）
//! - 内部簿记项（增量片段 / 终端记录）不出事件：终端 usage 由 loop 直接
//!   消费，增量由完整块收口，双发即重复
//!
//! seq / 时间戳不出本模块：由调用方（loop）经 [`agent::AgentEvent::stamp`]
//! 单调盖戳。

use agent::{AgentBlock, AgentEventKind};
use rig_core::message::{Reasoning, Text};
use rig_core::streaming::StreamedAssistantContent;

/// Thinking 块提取：reasoning 全块按可显示文本（text / summary 类块按行
/// 拼接）拍平；加密 / 涂改块无明文，落空串占位（块不丢、正文不可还原）。
fn thinking_block(reasoning: &Reasoning) -> AgentBlock {
    AgentBlock::Thinking {
        thinking: reasoning.display_text(),
    }
}

/// 单条 assistant 消息事件（role 恒 assistant；子代理归因不进 sdk 引擎——
/// parent_tool_use_id 恒 None）。
fn assistant_message(blocks: Vec<AgentBlock>) -> AgentEventKind {
    AgentEventKind::Message {
        role: "assistant".to_owned(),
        blocks,
        parent_tool_use_id: None,
    }
}

/// 单个流项归一化：可出事件返回 [`AgentEventKind`]，内部簿记项返回 `None`
/// （调用方跳过、不占 seq）。
pub fn stream_item(item: &StreamedAssistantContent) -> Option<AgentEventKind> {
    match item {
        StreamedAssistantContent::Text(text) => {
            let Text { text, .. } = text;
            Some(assistant_message(vec![AgentBlock::Text {
                text: text.clone(),
            }]))
        }
        StreamedAssistantContent::Reasoning { reasoning, .. } => {
            Some(assistant_message(vec![thinking_block(reasoning)]))
        }
        StreamedAssistantContent::ToolCall { tool_call, .. } => {
            Some(assistant_message(vec![AgentBlock::ToolUse {
                id: tool_call.id.as_str().to_owned(),
                name: tool_call.function.name.clone(),
                input: tool_call.function.arguments.clone(),
            }]))
        }
        StreamedAssistantContent::Unknown(payload) => Some(AgentEventKind::Raw {
            event_type: "sdk_stream".to_owned(),
            // UnknownPayload 为 serde 透明载体（内嵌 Value），序列化实际不可失败；
            // 兜底占位保守留痕（永不丢事件）
            raw_json: serde_json::to_string(payload)
                .unwrap_or_else(|_| "{\"unserializable\":true}".to_owned()),
        }),
        // 增量片段（由完整块收口）与终端记录（usage 由 loop 消费）不出事件
        StreamedAssistantContent::ToolCallDelta { .. }
        | StreamedAssistantContent::ReasoningDelta { .. }
        | StreamedAssistantContent::Final(_) => None,
    }
}
