use rig::streaming::{Item, StreamEvent, Transcript};

use agent::{AgentDelta, AgentEventKind};

use crate::sdk::normalize::stream_item;

// ---------------------------------------------------------------------------
// 装置：流项 fixture 经 rig `Transcript::parse_prefix` 公开读回缝构造（0.43
// 起 Part 由 rig 内部簿记，外部以合法流序 JSON 铸出等价 `Item<StreamEvent>`）
// ---------------------------------------------------------------------------

/// 合法流序 JSON → 流项序列。
fn transcript_items(json: serde_json::Value) -> Vec<Item<StreamEvent>> {
    Transcript::parse_prefix(json)
        .expect("fixture 为合法流序")
        .into()
}

/// 文本片段流项（start + text 片段，取片段）。
fn text_item(text: &str) -> Item<StreamEvent> {
    transcript_items(serde_json::json!([
        { "item": "event", "value": { "event": "start", "part": 0, "kind": "text" } },
        { "item": "event", "value": { "event": "text", "part": 0, "text": text } },
    ]))
    .pop()
    .expect("text 片段在场")
}

/// 思考片段流项（start + reasoning 片段，取片段）。
fn reasoning_item(text: &str) -> Item<StreamEvent> {
    transcript_items(serde_json::json!([
        { "item": "event", "value": { "event": "start", "part": 0, "kind": "reasoning" } },
        { "item": "event", "value": { "event": "reasoning", "part": 0, "text": text } },
    ]))
    .pop()
    .expect("reasoning 片段在场")
}

/// 部分生命周期簿记流项：Start（开部） / Arguments（工具参数片段） / End
/// （部分收口，content 为终结内容）各一枚。
fn bookkeeping_items() -> Vec<Item<StreamEvent>> {
    let mut items = transcript_items(serde_json::json!([
        { "item": "event", "value": { "event": "start", "part": 0, "kind": "text" } },
        { "item": "event", "value": { "event": "start", "part": 1, "kind": "tool_call" } },
        { "item": "event", "value": { "event": "arguments", "part": 1, "json": "{\"pa" } },
        { "item": "event", "value": { "event": "end", "part": 0, "content": { "type": "text", "text": "完整正文" } } },
    ]));
    assert_eq!(items.len(), 4, "四枚簿记 fixture 在场");
    items.remove(0); // 弃首个 start（start 形态由第二 start 代表）
    items
}

// ---------------------------------------------------------------------------
// 双层化：增量产出（AC-1 传输半边核心）
// ---------------------------------------------------------------------------

#[test]
fn text流项归一化为message_delta文本增量且逐字保真() {
    let kind = stream_item(&text_item("正文结论 🎉 中文")).expect("Text 片段必出事件");

    let AgentEventKind::MessageDelta {
        parent_tool_use_id,
        delta,
    } = &kind
    else {
        panic!("Text 片段应归一化为 MessageDelta，实际: {kind:?}");
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
fn reasoning片段归一化为thinking增量逐片段出delta() {
    let kind = stream_item(&reasoning_item("先想一步")).expect("Reasoning 片段必出事件");

    let AgentEventKind::MessageDelta { delta, .. } = &kind else {
        panic!("Reasoning 片段应归一化为 MessageDelta，实际: {kind:?}");
    };
    assert_eq!(
        delta,
        &AgentDelta::Thinking {
            thinking: "先想一步".to_owned()
        },
        "思考增量可辨（0.43 单一片段形态，逐片段出 delta）"
    );
    assert!(kind.is_delta());
}

// ---------------------------------------------------------------------------
// 部分生命周期簿记项：不出事件（完整内容由轮末 choice 密封收口）
// ---------------------------------------------------------------------------

#[test]
fn 部分生命周期簿记项不出事件_start_arguments与end均none() {
    for item in bookkeeping_items() {
        let label = match &item {
            Item::Event(StreamEvent::Start { .. }) => "Start",
            Item::Event(StreamEvent::Arguments { .. }) => "Arguments",
            Item::Event(StreamEvent::End { .. }) => "End",
            other => panic!("fixture 应为簿记形态，实际: {other:?}"),
        };
        assert!(
            stream_item(&item).is_none(),
            "{label} 为内部簿记不出事件（完整内容轮末 finish 折叠收口，片段重复透出会双计）"
        );
    }
}

// ---------------------------------------------------------------------------
// Unknown 透传（Raw 逃生舱）
// ---------------------------------------------------------------------------

#[test]
fn unknown流项透传为raw且event_type恒sdk_stream载荷原样() {
    let item = transcript_items(serde_json::json!([
        { "item": "unknown", "value": {
            "type": "web_search_call",
            "id": "ws_1",
            "nested": { "中文": "🎉" }
        } },
    ]))
    .pop()
    .expect("Unknown fixture 在场");

    let kind = stream_item(&item).expect("Unknown 必出事件（永不丢）");
    let AgentEventKind::Raw {
        event_type,
        raw_json,
    } = &kind
    else {
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

    let reasoning = reasoning_item("重复思考");
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
