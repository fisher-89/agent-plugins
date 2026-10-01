//! `event` 的单元测试（AC-1 / AC-5）：双层词汇（增量 `MessageDelta` / 密封
//! 五变体）判别与 serde camelCase 线格式、`TurnDone` 更名线值回归（runResult
//! 退役不别名）、块模型与 `AgentDelta` 内部 tag、盖戳原语零回归。纯内存构造 +
//! serde_json，无 Mock。

use serde_json::{json, Value};

use crate::event::{AgentBlock, AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole};

/// 测试侧当前时钟毫秒（镜像 event.rs 的取值口径，用于 stamp 边界断言）。
fn millis_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 六变体各一枚代表事件（stamp 递增 seq）。
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
            AgentEventKind::MessageDelta {
                parent_tool_use_id: None,
                delta: AgentDelta::Text {
                    text: "增量".to_owned(),
                },
            },
        ),
        AgentEvent::stamp(
            2,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![AgentBlock::Text {
                    text: "正文".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            3,
            AgentEventKind::SystemNotice {
                subtype: "permission_denial".to_owned(),
                payload: json!({ "tool": "Bash" }),
            },
        ),
        AgentEvent::stamp(
            4,
            AgentEventKind::TurnDone {
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
            5,
            AgentEventKind::Raw {
                event_type: "mystery".to_owned(),
                raw_json: "{\"type\":\"mystery\"}".to_owned(),
            },
        ),
    ]
}

// ---------------------------------------------------------------------------
// 双层判别：is_delta / is_sealed（内核泵分类与 store sink 防御共用）
// ---------------------------------------------------------------------------

#[test]
fn message_delta变体is_delta为真且sealed判别为假() {
    let event = AgentEventKind::MessageDelta {
        parent_tool_use_id: None,
        delta: AgentDelta::Text {
            text: "增量".to_owned(),
        },
    };

    assert!(event.is_delta(), "增量变体 is_delta 为 true");
    assert!(!event.is_sealed(), "增量变体不属密封层（互补不重叠）");
}

#[test]
fn 增量判别对parent归因与thinking两形态同真() {
    // 子代理归因词汇同源（parent_tool_use_id 有值）与思考增量两形态：
    // 判别只看变体不看载荷形态
    let attributed = AgentEventKind::MessageDelta {
        parent_tool_use_id: Some("tu_1".to_owned()),
        delta: AgentDelta::Text {
            text: "子代理增量".to_owned(),
        },
    };
    let thinking = AgentEventKind::MessageDelta {
        parent_tool_use_id: None,
        delta: AgentDelta::Thinking {
            thinking: "先想一下".to_owned(),
        },
    };

    assert!(attributed.is_delta());
    assert!(thinking.is_delta());
    assert!(!attributed.is_sealed() && !thinking.is_sealed());
}

#[test]
fn 六变体判别矩阵_仅message_delta为增量_其余五变体为密封() {
    let kinds = variant_events()
        .into_iter()
        .map(|event| event.kind)
        .collect::<Vec<_>>();
    assert_eq!(kinds.len(), 6, "六变体逐一断言");

    for kind in &kinds {
        let is_delta = kind.is_delta();
        let is_sealed = kind.is_sealed();
        assert_eq!(
            is_delta, !is_sealed,
            "判别互补不重叠：{kind:?} is_delta={is_delta} is_sealed={is_sealed}"
        );
        assert_eq!(
            is_delta,
            matches!(kind, AgentEventKind::MessageDelta { .. }),
            "仅 MessageDelta 为增量，实际: {kind:?}"
        );
    }
    // 密封五变体逐一显式核对（durable 词汇面）
    assert!(kinds[0].is_sealed(), "RunStarted 密封");
    assert!(kinds[2].is_sealed(), "Message 密封");
    assert!(kinds[3].is_sealed(), "SystemNotice 密封");
    assert!(kinds[4].is_sealed(), "TurnDone 密封");
    assert!(kinds[5].is_sealed(), "Raw 密封（透传也是 durable 词汇）");
}

// ---------------------------------------------------------------------------
// MessageDelta serde 线格式：camelCase 判别值 + 驼峰键 + 往返
// ---------------------------------------------------------------------------

#[test]
fn message_delta序列化判别值逐字为messageDelta且驼峰键在场() {
    let event = AgentEvent::stamp(
        7,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: None,
            delta: AgentDelta::Text {
                text: "token".to_owned(),
            },
        },
    );

    let value = serde_json::to_value(&event).expect("序列化成功");
    assert_eq!(
        value["kind"],
        json!("messageDelta"),
        "增量判别值逐字为 messageDelta（碎事件根因修复的传输词汇）"
    );
    assert!(
        value.get("parentToolUseId").is_some(),
        "parentToolUseId 驼峰键在场"
    );
    assert!(value.get("delta").is_some(), "delta 载荷键在场");
    assert_eq!(value["delta"]["kind"], json!("text"), "delta 内部 tag 驼峰");

    let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, event, "增量事件 serde 往返逐字段相等");
}

#[test]
fn thinking_delta往返保真且与text增量可辨() {
    let thinking = AgentEvent::stamp(
        8,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: Some("tu_1".to_owned()),
            delta: AgentDelta::Thinking {
                thinking: "思考增量 🎉".to_owned(),
            },
        },
    );

    let value = serde_json::to_value(&thinking).expect("序列化成功");
    assert_eq!(value["delta"]["kind"], json!("thinking"));
    assert_eq!(value["parentToolUseId"], json!("tu_1"));
    let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, thinking);

    // 思考/回复可辨：两形态互不相等
    assert_ne!(
        thinking.kind,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: Some("tu_1".to_owned()),
            delta: AgentDelta::Text {
                text: "思考增量 🎉".to_owned()
            }
        },
        "Thinking 与 Text 同文不同类，判别分明"
    );
}

// ---------------------------------------------------------------------------
// 六变体线格式回归（TurnDone 更名后的全变体面）
// ---------------------------------------------------------------------------

#[test]
fn 六变体序列化含seq与时间戳顶层键且判别值为驼峰() {
    let expected_kinds = [
        "runStarted",
        "messageDelta",
        "message",
        "systemNotice",
        "turnDone",
        "raw",
    ];
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
fn 六变体各自反序列化往返逐字段相等() {
    for event in variant_events() {
        let value = serde_json::to_value(&event).expect("序列化成功");
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event, "seq={}", event.seq);
    }
}

// ---------------------------------------------------------------------------
// TurnDone 更名线格式（runResult 线值退役）
// ---------------------------------------------------------------------------

#[test]
fn turn_done序列化判别值逐字为turnDone且驼峰统计键回归() {
    let event = AgentEvent::stamp(
        9,
        AgentEventKind::TurnDone {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(3),
            duration_ms: Some(1234),
            cost_usd: Some(0.42),
            usage: json!({ "input_tokens": 10 }),
            session_id: Some("s-1".to_owned()),
        },
    );

    let value = serde_json::to_value(&event).expect("序列化成功");
    assert_eq!(
        value["kind"],
        json!("turnDone"),
        "收敛判别值逐字为 turnDone（runResult 线值退役）"
    );
    // is_error/numTurns/durationMs/costUsd/sessionId 驼峰键回归
    for key in ["isError", "numTurns", "durationMs", "costUsd", "sessionId"] {
        assert!(value.get(key).is_some(), "turnDone.{key} 驼峰键在场");
    }
    let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, event);
}

#[test]
fn 旧线值run_result反序列化返回err更名不别名() {
    let legacy = json!({
        "seq": 0,
        "timestampMs": 1,
        "kind": "runResult",
        "subtype": "success",
        "isError": false,
        "numTurns": 1,
        "durationMs": 10,
        "costUsd": null,
        "usage": null,
        "sessionId": null
    });

    let result = serde_json::from_value::<AgentEvent>(legacy.clone());
    assert!(
        result.is_err(),
        "旧线值 runResult 不再可读（更名不别名，无兼容读负担），实际: {result:?}"
    );
    // 新线值同载荷可读：仅判别值不同
    let mut current = legacy;
    current["kind"] = json!("turnDone");
    assert!(
        serde_json::from_value::<AgentEvent>(current).is_ok(),
        "同载荷换 turnDone 判别值即可读"
    );
}

#[test]
fn turn_done全字段缺席形态serde往返无损() {
    let bare = AgentEvent::stamp(
        10,
        AgentEventKind::TurnDone {
            subtype: String::new(),
            is_error: false,
            num_turns: None,
            duration_ms: None,
            cost_usd: None,
            usage: Value::Null,
            session_id: None,
        },
    );

    let value = serde_json::to_value(&bare).expect("序列化成功");
    assert_eq!(value["usage"], Value::Null, "usage null 形态");
    assert_eq!(value["numTurns"], Value::Null);
    assert_eq!(value["costUsd"], Value::Null);
    assert_eq!(value["durationMs"], Value::Null);
    assert_eq!(value["sessionId"], Value::Null);
    let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, bare, "全字段缺席形态往返无损");
}

// ---------------------------------------------------------------------------
// AgentDelta 内部 tag 枚举（text/thinking 线值）
// ---------------------------------------------------------------------------

#[test]
fn agent_delta两变体序列化判别值逐字为text与thinking且往返一致() {
    for (delta, expected) in [
        (
            AgentDelta::Text {
                text: "增量文本".to_owned(),
            },
            "text",
        ),
        (
            AgentDelta::Thinking {
                thinking: "增量思考".to_owned(),
            },
            "thinking",
        ),
    ] {
        let value = serde_json::to_value(&delta).expect("序列化成功");
        assert_eq!(value["kind"], json!(expected), "判别值逐字 {expected}");
        let roundtrip: AgentDelta = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, delta);
    }
}

// ---------------------------------------------------------------------------
// 盖戳原语 stamp 回归
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
    assert!(
        event.timestamp_ms >= before && event.timestamp_ms <= after,
        "时间戳落在调用时刻区间内，实际: {}（区间 [{before}, {after}]）",
        event.timestamp_ms
    );
}

#[test]
fn stamp对u64最大seq原样保留不截断且往返无损() {
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
// 密封层回归：Message 块模型 / Raw 透传 / 必填键
// ---------------------------------------------------------------------------

#[test]
fn message四类块混合数组往返无损且块判别值为驼峰() {
    let mixed = AgentEvent::stamp(
        11,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
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

    let value = serde_json::to_value(&mixed).expect("序列化成功");
    assert_eq!(value["blocks"][0]["kind"], json!("text"), "块判别值为驼峰");
    let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, mixed);
}

#[test]
fn raw变体原文json逐字节保留含中文emoji换行引号() {
    let original = "第一行 🎉\n\"quoted\" {\"inner\":true}\t制表";
    let event = AgentEvent::stamp(
        12,
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
    // 对照组：两键齐备即可反序列化
    let complete = json!({
        "seq": 0,
        "timestampMs": 1,
        "kind": "raw",
        "eventType": "x",
        "rawJson": "{}"
    });
    assert!(serde_json::from_value::<AgentEvent>(complete).is_ok());
}
