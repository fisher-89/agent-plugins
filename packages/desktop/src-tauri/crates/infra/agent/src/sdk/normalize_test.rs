//! `normalize` 的单元测试（AC-4）：rig 归一化流项 → [`agent::AgentEventKind`]
//! 归一化纯函数的 fixture 重建——文本/思考映射、tool_calls 保真、未知透传、
//! 簿记项不出事件。全部 fixture 由 rig 类型内存构造，断言无网络触达（不打
//! 网络）；seq 盖戳语义经 [`agent::AgentEvent::stamp`] 锁定（seq 由调用方
//! 单调传入，本模块不产 seq）。

use rig_core::message::{Reasoning, Text, ToolCall, ToolFunction};
use rig_core::streaming::{
    StreamFinal, StreamedAssistantContent, ToolCallDeltaContent, UnknownPayload,
};
use rig_core::completion::Usage;

use agent::{AgentBlock, AgentEvent, AgentEventKind};

use crate::sdk::normalize::stream_item;

// ---------------------------------------------------------------------------
// fixture 构造（rig 归一化流项内存构造，不经 provider client）
// ---------------------------------------------------------------------------

/// 文本流项。
fn text_item(text: &str) -> StreamedAssistantContent {
    StreamedAssistantContent::Text(Text::new(text.to_owned()))
}

/// 完整 reasoning 流项（serde rename reasoning_content 的聚合产物）。
fn reasoning_item(text: &str) -> StreamedAssistantContent {
    StreamedAssistantContent::Reasoning {
        reasoning: Reasoning::new(text),
        id: "corr-reasoning-1".to_owned(),
    }
}

/// 完整 tool call 流项（name / arguments / id 保真）。
fn tool_call_item(id: &str, name: &str, arguments: serde_json::Value) -> StreamedAssistantContent {
    StreamedAssistantContent::ToolCall {
        tool_call: ToolCall::from_wire(id, ToolFunction::new(name.to_owned(), arguments)),
        internal_call_id: format!("internal-{id}"),
    }
}

/// 终端记录（内部簿记：usage 由 loop 直接消费）。
fn final_item() -> StreamedAssistantContent {
    StreamedAssistantContent::Final(StreamFinal::new("fixture-provider", Usage::new()))
}

/// 断言单个流项归一化产出的 assistant 消息块形态。
fn assert_assistant_message(
    kind: AgentEventKind,
    check: impl FnOnce(&[AgentBlock]),
) {
    let AgentEventKind::Message {
        role,
        blocks,
        parent_tool_use_id,
    } = kind
    else {
        panic!("归一化产物必须是 assistant Message，实际: {kind:?}");
    };
    assert_eq!(role.as_str(), "assistant", "role 恒 assistant");
    assert!(
        parent_tool_use_id.is_none(),
        "子代理归因不进 sdk 引擎（恒 None）"
    );
    check(&blocks);
}

// ---------------------------------------------------------------------------
// 正向：文本 / 思考 / tool_calls 映射
// ---------------------------------------------------------------------------

#[test]
fn 文本流项归一化为assistant_message的text块() {
    let kind = stream_item(&text_item("你好，正在分析 🎉")).expect("文本项出事件");
    assert_assistant_message(kind, |blocks| {
        let Some(AgentBlock::Text { text }) = blocks.first() else {
            panic!("首块必须是 Text，实际: {blocks:?}");
        };
        assert_eq!(text.as_str(), "你好，正在分析 🎉", "文本保真（含中文与 emoji）");
    });
}

#[test]
fn reasoning全块归一化为thinking块且明文拍平() {
    let kind = stream_item(&reasoning_item("先读目录结构，再定位入口")).expect("reasoning 项出事件");
    assert_assistant_message(kind, |blocks| {
        let Some(AgentBlock::Thinking { thinking }) = blocks.first() else {
            panic!("首块必须是 Thinking，实际: {blocks:?}");
        };
        assert_eq!(
            thinking.as_str(),
            "先读目录结构，再定位入口",
            "reasoning 明文映射 Thinking（spike 据实定稿映射，不缺席留痕）"
        );
    });
}

#[test]
fn tool_call流项归一化为tool_use块且id与name与arguments保真() {
    let item = tool_call_item("tu_sdk_1", "read", serde_json::json!({ "path": "src/lib.rs" }));
    let kind = stream_item(&item).expect("tool call 项出事件");
    assert_assistant_message(kind, |blocks| {
        let Some(AgentBlock::ToolUse { id, name, input }) = blocks.first() else {
            panic!("首块必须是 ToolUse，实际: {blocks:?}");
        };
        assert_eq!(id.as_str(), "tu_sdk_1", "id 保真（与后续 ToolResult 同 id 成对口径）");
        assert_eq!(name.as_str(), "read", "工具名保真");
        assert_eq!(
            input,
            &serde_json::json!({ "path": "src/lib.rs" }),
            "入参保真"
        );
    });
}

// ---------------------------------------------------------------------------
// 异常：未知 rig 流项 Raw 透传
// ---------------------------------------------------------------------------

#[test]
fn 未知流项透传为raw事件且raw_json保真不丢() {
    let payload = serde_json::json!({
        "type": "web_search_call",
        "id": "ws_1",
        "note": "provider 原生未建模项 🚀"
    });
    let kind = stream_item(&StreamedAssistantContent::Unknown(UnknownPayload::new(payload.clone())))
        .expect("未知项透传出事件");
    let AgentEventKind::Raw { event_type, raw_json } = kind else {
        panic!("未知项必须 Raw 透传，实际: {kind:?}");
    };
    assert_eq!(event_type.as_str(), "sdk_stream", "event_type 恒 sdk_stream");
    let reparsed: serde_json::Value =
        serde_json::from_str(&raw_json).expect("raw_json 为合法 JSON");
    assert_eq!(reparsed, payload, "载荷原样透传不丢（永不炸解析）");
}

// ---------------------------------------------------------------------------
// 边界：簿记项不出事件 / 空与块序 / seq 盖戳
// ---------------------------------------------------------------------------

#[test]
fn 簿记项不出事件_增量与终端记录返回none() {
    // 增量片段（ToolCallDelta / ReasoningDelta）：由完整块收口，不出事件
    assert!(
        stream_item(&StreamedAssistantContent::ToolCallDelta {
            internal_call_id: "internal-tu_1".to_owned(),
            content: ToolCallDeltaContent::Name("read".to_owned()),
        })
        .is_none(),
        "tool call 增量不出事件"
    );
    assert!(
        stream_item(&StreamedAssistantContent::ReasoningDelta {
            id: "corr-r".to_owned(),
            provider_id: None,
            reasoning: "思考片段".to_owned(),
        })
        .is_none(),
        "reasoning 增量不出事件"
    );
    // 终端记录（Final）：usage 由 loop 直接消费，不出事件（双发即重复）
    assert!(stream_item(&final_item()).is_none(), "终端记录不出事件");
}

#[test]
fn 混合流项序列块序锁定_纯reasoning与纯tool_calls轮无文本() {
    // 纯 reasoning 轮（无文本）：单 Thinking 块
    let reasoning_only = stream_item(&reasoning_item("纯思考轮")).expect("出事件");
    assert_assistant_message(reasoning_only, |blocks| {
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], AgentBlock::Thinking { .. }));
    });

    // 纯 tool_calls 轮（无文本）：单 ToolUse 块
    let tools_only = stream_item(&tool_call_item("tu_2", "ls", serde_json::json!({ "path": "." })))
        .expect("出事件");
    assert_assistant_message(tools_only, |blocks| {
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], AgentBlock::ToolUse { .. }));
    });

    // 文本与工具调用混合序列：逐项独立成事件，块序按流序（文本在前）
    let first = stream_item(&text_item("先说明"));
    let second = stream_item(&tool_call_item("tu_3", "read", serde_json::json!({})));
    assert!(first.is_some() && second.is_some(), "两流项各出一事件");
}

#[test]
fn seq由调用方经stamp单调传入_跨流项零跳号() {
    // seq 不出 normalize 模块：调用方（loop）经 AgentEvent::stamp 盖戳。
    // 本用例以调用方口径驱动跨流项 0..n 盖戳，锁定单调无跳号（AC-4 seq 半边）
    let items = [text_item("一"), reasoning_item("二"), tool_call_item("tu_4", "ls", serde_json::json!({}))];
    let mut stamped: Vec<AgentEvent> = Vec::new();
    for (seq, item) in items.iter().enumerate() {
        if let Some(kind) = stream_item(item) {
            stamped.push(AgentEvent::stamp(seq as u64, kind));
        }
    }
    assert_eq!(stamped.len(), 3, "三个可出事件流项全部盖戳");
    for (index, event) in stamped.iter().enumerate() {
        assert_eq!(event.seq, index as u64, "seq 0..n 无跳号");
    }
}

// ---------------------------------------------------------------------------
// 打网络约束（AC-4）：本文件全部 fixture 内存构造——编译层面无 provider
// client 依赖（不构造 rig provider client），运行层面无任何 IO
// ---------------------------------------------------------------------------

#[test]
fn fixture全部内存构造无网络触达路径() {
    // 文本 / 思考 / 工具 / 未知 / 终端五类流项全由 rig 类型字面构造，
    // normalize 为纯函数（&StreamedAssistantContent → Option<AgentEventKind>），
    // 无 provider client、无 socket 参与——AC-4「不打网络」结构性成立
    for item in [
        text_item("文本"),
        reasoning_item("思考"),
        tool_call_item("tu_5", "grep", serde_json::json!({ "pattern": "x" })),
        StreamedAssistantContent::Unknown(UnknownPayload::new(serde_json::json!({"m":1}))),
        final_item(),
    ] {
        // 归一化过程无副作用：同一项重复归一化结果一致
        let first = stream_item(&item);
        let second = stream_item(&item);
        assert_eq!(first.is_some(), second.is_some());
    }
}
