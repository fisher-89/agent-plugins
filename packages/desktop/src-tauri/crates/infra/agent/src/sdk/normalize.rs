use agent::{AgentDelta, AgentEventKind};
use rig::streaming::{Item, StreamEvent};

/// 增量事件构造（子代理归因不进 sdk 引擎——parent_tool_use_id 恒 None，
/// 即单 provisional 通道）。
fn message_delta(delta: AgentDelta) -> AgentEventKind {
    AgentEventKind::MessageDelta {
        parent_tool_use_id: None,
        delta,
    }
}

/// 单个流项归一化：可出事件返回 [`AgentEventKind`]（增量层），
pub fn stream_item(item: &Item<StreamEvent>) -> Option<AgentEventKind> {
    match item {
        Item::Event(StreamEvent::Text { text, .. }) => {
            Some(message_delta(AgentDelta::Text { text: text.clone() }))
        }
        Item::Event(StreamEvent::Reasoning { text, .. }) => {
            Some(message_delta(AgentDelta::Thinking {
                thinking: text.clone(),
            }))
        }
        Item::Event(
            StreamEvent::Start { .. } | StreamEvent::Arguments { .. } | StreamEvent::End { .. },
        ) => None,
        Item::Unknown(payload) => Some(AgentEventKind::Raw {
            event_type: "sdk_stream".to_owned(),
            // UnknownPayload 为 serde 透明载体（内嵌 Value），序列化实际不可失败；
            // 兜底占位保守留痕（永不丢事件）
            raw_json: serde_json::to_string(payload)
                .unwrap_or_else(|_| "{\"unserializable\":true}".to_owned()),
        }),
    }
}
