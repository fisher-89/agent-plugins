use agent::{AgentDelta, AgentEventKind};
use rig_core::message::{Reasoning, Text};
use rig_core::streaming::StreamedAssistantContent;

/// Thinking 块提取：reasoning 全块按可显示文本（text / summary 类块按行
/// 拼接）拍平；加密 / 涂改块无明文，落空串占位（块不丢、正文不可还原）。
fn thinking_text(reasoning: &Reasoning) -> String {
    reasoning.display_text()
}

/// 增量事件构造（子代理归因不进 sdk 引擎——parent_tool_use_id 恒 None，
/// 即单 provisional 通道）。
fn message_delta(delta: AgentDelta) -> AgentEventKind {
    AgentEventKind::MessageDelta {
        parent_tool_use_id: None,
        delta,
    }
}

/// 单个流项归一化：可出事件返回 [`AgentEventKind`]（增量层），内部簿记项
/// 与轮末收口件返回 `None`（调用方跳过、不占 seq）。
pub fn stream_item(item: &StreamedAssistantContent) -> Option<AgentEventKind> {
    match item {
        StreamedAssistantContent::Text(text) => {
            let Text { text, .. } = text;
            Some(message_delta(AgentDelta::Text { text: text.clone() }))
        }
        StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
            Some(message_delta(AgentDelta::Thinking {
                thinking: reasoning.clone(),
            }))
        }
        StreamedAssistantContent::Reasoning { reasoning, .. } => {
            Some(message_delta(AgentDelta::Thinking {
                thinking: thinking_text(reasoning),
            }))
        }
        StreamedAssistantContent::Unknown(payload) => Some(AgentEventKind::Raw {
            event_type: "sdk_stream".to_owned(),
            // UnknownPayload 为 serde 透明载体（内嵌 Value），序列化实际不可失败；
            // 兜底占位保守留痕（永不丢事件）
            raw_json: serde_json::to_string(payload)
                .unwrap_or_else(|_| "{\"unserializable\":true}".to_owned()),
        }),
        // 完整 tool call 由轮末 choice 密封收口；增量片段（tool call delta）
        // 与终端记录（usage 由 loop 消费）为内部簿记，均不出事件
        StreamedAssistantContent::ToolCall { .. }
        | StreamedAssistantContent::ToolCallDelta { .. }
        | StreamedAssistantContent::Final(_) => None,
    }
}
