use rig_core::message::{Reasoning, Text};
use rig_core::streaming::{
    StreamFinal, StreamedAssistantContent, ToolCallDeltaContent, UnknownPayload,
};
use rig_core::completion::Usage;

use agent::{AgentDelta, AgentEventKind};

use crate::sdk::normalize::stream_item;

/// 文本流项（rig `Text` 即增量变体）。
fn text_item(text: &str) -> StreamedAssistantContent {
    StreamedAssistantContent::Text(Text::new(text.to_owned()))
}

/// 思考增量流项。
fn reasoning_delta_item(reasoning: &str) -> StreamedAssistantContent {
    StreamedAssistantContent::ReasoningDelta {
        id: "rc-1".to_owned(),
        provider_id: None,
        reasoning: reasoning.to_owned(),
    }
}

/// 完整思考块流项（supersedes 同 correlator 的增量）。
fn reasoning_item(reasoning: &str) -> StreamedAssistantContent {
    StreamedAssistantContent::Reasoning {
        reasoning: Reasoning::new(reasoning),
        id: "rc-1".to_owned(),
    }
}

/// 完整工具调用流项（复用 AssistantContent 构造器取 rig ToolCall）。
fn tool_call_item() -> StreamedAssistantContent {
    let rig_core::message::AssistantContent::ToolCall(tool_call) =
        rig_core::message::AssistantContent::tool_call(
            "tu_1",
            "read",
            serde_json::json!({ "path": "README.md" }),
        )
    else {
        panic!("tool_call 构造器恒产出 ToolCall 变体");
    };
    StreamedAssistantContent::ToolCall {
        tool_call,
        internal_call_id: "ic-1".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// 双层化：增量产出（AC-1 传输半边核心）
// ---------------------------------------------------------------------------

#[test]
fn text流项归一化为message_delta文本增量且逐字保真() {
    let kind = stream_item(&text_item("正文结论 🎉 中文")).expect("Text 流项必出事件");

    let AgentEventKind::MessageDelta {
        parent_tool_use_id,
        delta,
    } = &kind
    else {
        panic!("Text 流项应归一化为 MessageDelta，实际: {kind:?}");
    };
    assert_eq!(*parent_tool_use_id, None, "sdk 引擎单 provisional 通道");
    assert_eq!(
        delta,
        &AgentDelta::Text {
            text: "正文结论 🎉 中文".to_owned()
        },
        "文本逐字保真（含中文/emoji）"
    );
    // 与密封 Message 变体判别分明（碎事件根因修复的传输半边）
    assert!(kind.is_delta());
    assert!(!kind.is_sealed());
}

#[test]
fn reasoning_delta流项归一化为thinking增量逐片段出delta() {
    let kind = stream_item(&reasoning_delta_item("先想一步")).expect("ReasoningDelta 必出事件");

    let AgentEventKind::MessageDelta { delta, .. } = &kind else {
        panic!("ReasoningDelta 应归一化为 MessageDelta，实际: {kind:?}");
    };
    assert_eq!(
        delta,
        &AgentDelta::Thinking {
            thinking: "先想一步".to_owned()
        },
        "思考增量可辨（原「等完整块」语义改为逐片段出 delta）"
    );
    assert!(kind.is_delta());
}

#[test]
fn 完整reasoning块同样归一化为thinking_delta无第二密封通道() {
    let kind = stream_item(&reasoning_item("整块思考")).expect("完整 Reasoning 必出事件");

    let AgentEventKind::MessageDelta { delta, .. } = &kind else {
        panic!("完整 Reasoning 应归一化为 MessageDelta（Thinking），实际: {kind:?}");
    };
    assert_eq!(
        delta,
        &AgentDelta::Thinking {
            thinking: "整块思考".to_owned()
        },
        "双层词汇无第二密封通道（完整块也走 delta 面）"
    );
    assert!(kind.is_delta());
}

// ---------------------------------------------------------------------------
// 轮末收口与簿记项：不出事件
// ---------------------------------------------------------------------------

#[test]
fn 完整tool_call流项不出事件_轮末choice统一收进密封() {
    assert!(
        stream_item(&tool_call_item()).is_none(),
        "完整 ToolCall 不再立即出密封 Message 事件（轮末收口聚合）"
    );
}

#[test]
fn 簿记项final与tool_call_delta返回none不出事件() {
    let final_item = StreamedAssistantContent::Final(StreamFinal::new("fake-provider", Usage::new()));
    let delta_item = StreamedAssistantContent::ToolCallDelta {
        internal_call_id: "ic-1".to_owned(),
        content: ToolCallDeltaContent::Delta("{\"pa".to_owned()),
    };

    assert!(
        stream_item(&final_item).is_none(),
        "Final 为内部簿记（终端 usage 由 loop 直接消费）"
    );
    assert!(
        stream_item(&delta_item).is_none(),
        "ToolCallDelta 为内部簿记（增量片段不出事件）"
    );
}

// ---------------------------------------------------------------------------
// Unknown 透传（Raw 逃生舱）
// ---------------------------------------------------------------------------

#[test]
fn unknown流项透传为raw且event_type恒sdk_stream载荷原样() {
    let payload = UnknownPayload::new(serde_json::json!({
        "type": "web_search_call",
        "id": "ws_1",
        "nested": { "中文": "🎉" }
    }));
    let item = StreamedAssistantContent::Unknown(payload);

    let kind = stream_item(&item).expect("Unknown 必出事件（永不丢）");
    let AgentEventKind::Raw { event_type, raw_json } = &kind else {
        panic!("Unknown 应归一化为 Raw，实际: {kind:?}");
    };
    assert_eq!(event_type, "sdk_stream", "event_type 恒 sdk_stream");
    assert!(
        raw_json.contains("web_search_call") && raw_json.contains("ws_1"),
        "raw_json 载荷原样透传不丢，实际: {raw_json}"
    );
    assert!(kind.is_sealed(), "Raw 属密封层（透传也是 durable 词汇）");
}

// ---------------------------------------------------------------------------
// 纯函数无副作用
// ---------------------------------------------------------------------------

#[test]
fn 同一流项重复归一化结果一致且空文本delta形态合法() {
    // 幂等：同一流项重复归一化结果一致（无内部状态）
    let item = text_item("重复归一化");
    let first = stream_item(&item).expect("第一次归一化出事件");
    let second = stream_item(&item).expect("第二次归一化出事件");
    assert_eq!(first, second, "纯函数无副作用");

    let reasoning = reasoning_delta_item("重复思考");
    assert_eq!(
        stream_item(&reasoning).expect("重复出事件"),
        stream_item(&reasoning).expect("重复出事件")
    );

    // 空文本 delta 形态合法（增量词法允许空串占位）
    let kind = stream_item(&text_item("")).expect("空文本 delta 也出事件");
    assert_eq!(
        kind,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: None,
            delta: AgentDelta::Text {
                text: String::new()
            }
        },
        "空文本 delta 形态合法"
    );
}
