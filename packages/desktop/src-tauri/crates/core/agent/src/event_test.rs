//! `event` 的单元测试（AC-1）：信封盖戳、五变体 serde camelCase 线格式、
//! 块模型、Raw 透传保真与必填键校验。纯内存构造 + serde_json，无 Mock。

use serde_json::{json, Value};

use crate::event::{AgentBlock, AgentEvent, AgentEventKind};

/// 测试侧当前时钟毫秒（镜像 event.rs 的取值口径，用于 stamp 边界断言）。
fn millis_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 五变体各一枚代表事件（stamp(seq=0)）。
fn variant_events() -> Vec<AgentEvent> {
    vec![
        AgentEvent::stamp(
            0,
            AgentEventKind::RunStarted {
                model: Some("claude-opus".to_owned()),
                session_id: Some("s-1".to_owned()),
                tools: vec!["Bash".to_owned()],
                mcp_servers: vec!["mcp-a".to_owned()],
            },
        ),
        AgentEvent::stamp(
            1,
            AgentEventKind::Message {
                role: "assistant".to_owned(),
                blocks: vec![AgentBlock::Text {
                    text: "正文".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            2,
            AgentEventKind::SystemNotice {
                subtype: "permission_denial".to_owned(),
                payload: json!({ "tool": "Bash" }),
            },
        ),
        AgentEvent::stamp(
            3,
            AgentEventKind::RunResult {
                subtype: "success".to_owned(),
                is_error: false,
                num_turns: Some(3),
                duration_ms: Some(1234),
                cost_usd: Some(0.42),
                usage: json!({ "input_tokens": 10 }),
                session_id: Some("s-1".to_owned()),
            },
        ),
        AgentEvent::stamp(
            4,
            AgentEventKind::Raw {
                event_type: "mystery".to_owned(),
                raw_json: "{\"type\":\"mystery\"}".to_owned(),
            },
        ),
    ]
}

// ---------------------------------------------------------------------------
// AgentEvent::stamp
// ---------------------------------------------------------------------------

#[test]
fn stamp以0盖戳产出seq为0且时间戳为当前时钟毫秒() {
    let before = millis_now();
    let event = AgentEvent::stamp(
        0,
        AgentEventKind::Raw {
            event_type: "x".to_owned(),
            raw_json: "{}".to_owned(),
        },
    );
    let after = millis_now();

    assert_eq!(event.seq, 0);
    assert!(event.timestamp_ms > 0, "时间戳为当前时钟毫秒（大于 0）");
    assert!(
        event.timestamp_ms >= before && event.timestamp_ms <= after,
        "时间戳落在调用时刻区间内，实际: {}（区间 [{before}, {after}]）",
        event.timestamp_ms
    );
}

#[test]
fn stamp对u64最大seq原样保留不截断() {
    let event = AgentEvent::stamp(
        u64::MAX,
        AgentEventKind::Raw {
            event_type: "x".to_owned(),
            raw_json: "{}".to_owned(),
        },
    );

    assert_eq!(event.seq, u64::MAX, "seq 原样保留");
    // 线格式同样不截断（u64 全值域可表达）
    let value = serde_json::to_value(&event).expect("序列化成功");
    assert_eq!(value["seq"], json!(u64::MAX));
    let roundtrip: AgentEvent = serde_json::from_value(value).expect("u64::MAX 反序列化不溢出");
    assert_eq!(roundtrip, event);
}

// ---------------------------------------------------------------------------
// AgentEventKind serde：五变体线格式与往返
// ---------------------------------------------------------------------------

#[test]
fn 五变体序列化含seq与时间戳顶层键且判别值为驼峰() {
    let expected_kinds = ["runStarted", "message", "systemNotice", "runResult", "raw"];
    for (event, expected_kind) in variant_events().into_iter().zip(expected_kinds) {
        let value = serde_json::to_value(&event).expect("序列化成功");

        let object = value.as_object().expect("顶层为对象");
        assert!(
            object.contains_key("seq") && object.contains_key("timestampMs"),
            "顶层含 seq/timestampMs，实际键: {object:?}"
        );
        // kind 为内部 tag：值扁平进顶层
        assert_eq!(value["kind"], json!(expected_kind), "判别值为驼峰字符串");
    }
}

#[test]
fn 各变体驼峰字段键落在线格式顶层() {
    let events = variant_events();
    let values: Vec<Value> = events
        .iter()
        .map(|event| serde_json::to_value(event).expect("序列化成功"))
        .collect();

    // runStarted：sessionId / mcpServers 驼峰
    assert!(values[0].get("sessionId").is_some(), "runStarted.sessionId");
    assert!(
        values[0].get("mcpServers").is_some(),
        "runStarted.mcpServers"
    );
    // message：parentToolUseId 驼峰
    assert!(
        values[1].get("parentToolUseId").is_some(),
        "message.parentToolUseId"
    );
    // runResult：isError / numTurns / durationMs / costUsd / sessionId 驼峰
    for key in ["isError", "numTurns", "durationMs", "costUsd", "sessionId"] {
        assert!(values[3].get(key).is_some(), "runResult.{key}");
    }
    // raw：eventType / rawJson 驼峰
    assert!(values[4].get("eventType").is_some(), "raw.eventType");
    assert!(values[4].get("rawJson").is_some(), "raw.rawJson");
}

#[test]
fn 五变体各自反序列化往返逐字段相等() {
    for event in variant_events() {
        let value = serde_json::to_value(&event).expect("序列化成功");
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event, "seq={}", event.seq);
    }
}

#[test]
fn message携带空blocks与四类块混合数组均往返无损() {
    let mixed = AgentEvent::stamp(
        7,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: vec![
                AgentBlock::Text {
                    text: "文本".to_owned(),
                },
                AgentBlock::Thinking {
                    thinking: "思考".to_owned(),
                },
                AgentBlock::ToolUse {
                    id: "tu_1".to_owned(),
                    name: "Bash".to_owned(),
                    input: json!({ "command": "ls" }),
                },
                AgentBlock::ToolResult {
                    id: "tu_1".to_owned(),
                    content: "结果".to_owned(),
                    is_error: false,
                },
            ],
            parent_tool_use_id: None,
        },
    );
    let empty = AgentEvent::stamp(
        8,
        AgentEventKind::Message {
            role: "user".to_owned(),
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    );

    for event in [mixed, empty] {
        let value = serde_json::to_value(&event).expect("序列化成功");
        // 块模型 tag 值亦为驼峰（kind 判别）
        if let AgentEventKind::Message { blocks, .. } = &event.kind {
            if !blocks.is_empty() {
                assert_eq!(value["blocks"][0]["kind"], json!("text"), "块判别值为驼峰");
            }
        }
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event);
    }
}

#[test]
fn 子代理归因字段为null与有值两形态均保真() {
    let with_parent = AgentEvent::stamp(
        9,
        AgentEventKind::Message {
            role: "user".to_owned(),
            blocks: Vec::new(),
            parent_tool_use_id: Some("tu_1".to_owned()),
        },
    );
    let without_parent = AgentEvent::stamp(
        10,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    );

    for event in [with_parent, without_parent] {
        let value = serde_json::to_value(&event).expect("序列化成功");
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event);
    }
    // null 与有值在线格式上可区分
    let null_value = serde_json::to_value(&AgentEventKind::Message {
        role: "user".to_owned(),
        blocks: Vec::new(),
        parent_tool_use_id: None,
    })
    .expect("序列化成功");
    assert_eq!(null_value["parentToolUseId"], Value::Null);
    let some_value = serde_json::to_value(&AgentEventKind::Message {
        role: "user".to_owned(),
        blocks: Vec::new(),
        parent_tool_use_id: Some("tu_1".to_owned()),
    })
    .expect("序列化成功");
    assert_eq!(some_value["parentToolUseId"], json!("tu_1"));
}

#[test]
fn raw变体原文json逐字节保留含中文emoji换行引号() {
    let original = "第一行 🎉\n\"quoted\" {\"inner\":true}\t制表";
    let event = AgentEvent::stamp(
        11,
        AgentEventKind::Raw {
            event_type: "unparsable".to_owned(),
            raw_json: original.to_owned(),
        },
    );

    let value = serde_json::to_value(&event).expect("序列化成功");
    let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");

    match roundtrip.kind {
        AgentEventKind::Raw { raw_json, .. } => {
            assert_eq!(
                raw_json, original,
                "原文逐字节保留（含中文/emoji/换行/引号）"
            );
        }
        other => panic!("变体应为 Raw，实际: {other:?}"),
    }
}

#[test]
fn 缺seq或时间戳必填键的json反序列化返回err不产半成品事件() {
    // 缺 seq
    let missing_seq = json!({
        "timestampMs": 1,
        "kind": "raw",
        "eventType": "x",
        "rawJson": "{}"
    });
    assert!(
        serde_json::from_value::<AgentEvent>(missing_seq).is_err(),
        "缺 seq 必须 Err"
    );
    // 缺 timestampMs
    let missing_timestamp = json!({
        "seq": 0,
        "kind": "raw",
        "eventType": "x",
        "rawJson": "{}"
    });
    assert!(
        serde_json::from_value::<AgentEvent>(missing_timestamp).is_err(),
        "缺 timestampMs 必须 Err"
    );
    // 对照组：两键齐备即可反序列化（不产生半成品的前提是键全时正常通过）
    let complete = json!({
        "seq": 0,
        "timestampMs": 1,
        "kind": "raw",
        "eventType": "x",
        "rawJson": "{}"
    });
    assert!(serde_json::from_value::<AgentEvent>(complete).is_ok());
}
