use serde_json::{json, Value};

use crate::event::{
    AgentBlock, AgentCitation, AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole,
    WebSearchSource,
};

/// 测试侧当前时钟毫秒（镜像 event.rs 的取值口径，用于 stamp 边界断言）。
fn millis_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 引用 fixture（raw 含不确定字段，锚定「原值不规范化」）。
fn citation_fixture() -> AgentCitation {
    AgentCitation {
        url: Some("https://cite.example.com/1".to_owned()),
        title: Some("引用标题".to_owned()),
        cited_text: Some("被引用的原文片段 🎉".to_owned()),
        raw: json!({
            "url": "https://cite.example.com/1",
            "title": "引用标题",
            "cited_text": "被引用的原文片段 🎉",
            "encrypted_index": "不确定字段",
        }),
    }
}

/// 搜索来源 fixture（`title` / `page_age` 可空）。
fn source_fixture(url: &str, title: Option<&str>, page_age: Option<&str>) -> WebSearchSource {
    WebSearchSource {
        url: url.to_owned(),
        title: title.map(str::to_owned),
        page_age: page_age.map(str::to_owned),
    }
}

/// `WebSearch` 全字段在场 fixture（raw 两侧含 `encrypted_content` 与嵌套数组）。
fn web_search_block() -> AgentBlock {
    AgentBlock::WebSearch {
        id: "srvtoolu_1".to_owned(),
        query: "今日汇率".to_owned(),
        results: vec![source_fixture(
            "https://a.example.com/1",
            Some("甲来源"),
            Some("2025-10-01"),
        )],
        error: None,
        raw_use: Some(json!({
            "type": "server_tool_use",
            "id": "srvtoolu_1",
            "name": "web_search",
            "input": { "query": "今日汇率" },
            "encrypted_content": "加密索引 🎉",
            "nested": [1, [2, 3]],
        })),
        raw_result: Some(json!({
            "type": "web_search_tool_result",
            "tool_use_id": "srvtoolu_1",
            "content": [{ "type": "web_search_result", "url": "https://a.example.com/1" }],
            "encrypted_content": "结果加密",
        })),
    }
}

/// 六变体各一枚代表事件（stamp 递增 seq）。
///
/// Message 变体携带**五变体块面**（Text 携 citations + WebSearch），使信封整体
/// 往返用例覆盖新块与新增字段（迭代性质 = 修改；断言语义不变）。
fn variant_events() -> Vec<AgentEvent> {
    vec![
        AgentEvent::stamp(
            None,
            0,
            AgentEventKind::RunStarted {
                model: Some("claude-opus".to_owned()),
                session_id: Some("s-1".to_owned()),
                tools: vec!["Bash".to_owned()],
                mcp_servers: vec!["mcp-a".to_owned()],
            },
        ),
        AgentEvent::stamp(
            None,
            1,
            AgentEventKind::MessageDelta {
                parent_tool_use_id: None,
                delta: AgentDelta::Text {
                    text: "增量".to_owned(),
                },
            },
        ),
        AgentEvent::stamp(
            None,
            2,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![
                    AgentBlock::Text {
                        text: "正文".to_owned(),
                        citations: Some(vec![citation_fixture()]),
                    },
                    web_search_block(),
                ],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            None,
            3,
            AgentEventKind::SystemNotice {
                subtype: "permission_denial".to_owned(),
                payload: json!({ "tool": "Bash" }),
            },
        ),
        AgentEvent::stamp(
            None,
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
            None,
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
fn message_delta序列化判别值为驼峰() {
    let event = AgentEvent::stamp(
        None,
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
        None,
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
fn turn_done序列化判别值为驼峰() {
    let event = AgentEvent::stamp(
        None,
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
        None,
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
        None,
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
        None,
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
        None,
        11,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: vec![
                AgentBlock::Text {
                    text: "文本".to_owned(),
                    citations: None,
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
        None,
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

// ---------------------------------------------------------------------------
// 五变体块面：WebSearch 变体与 Text.citations 字段（AC-1 / AC-2 事件面）
// ---------------------------------------------------------------------------

#[test]
fn web_search变体序列化逐键核对为驼峰线格式且results保序() {
    let block = AgentBlock::WebSearch {
        id: "srvtoolu_1".to_owned(),
        query: "今日汇率".to_owned(),
        results: vec![
            source_fixture("https://a.example.com/1", Some("甲"), Some("2025-10-01")),
            source_fixture("https://b.example.com/2", None, None),
        ],
        error: Some("max_uses_exceeded".to_owned()),
        raw_use: Some(json!({ "type": "server_tool_use", "id": "srvtoolu_1" })),
        raw_result: Some(json!({ "type": "web_search_tool_result" })),
    };

    let value = serde_json::to_value(&block).expect("序列化成功");

    assert_eq!(value["kind"], json!("webSearch"), "判别值驼峰 webSearch");
    assert_eq!(value["id"], json!("srvtoolu_1"));
    assert_eq!(value["query"], json!("今日汇率"));
    let results = value["results"].as_array().expect("results 为数组");
    assert_eq!(results.len(), 2, "results 非空保序");
    assert_eq!(
        results[0],
        json!({ "url": "https://a.example.com/1", "title": "甲", "pageAge": "2025-10-01" }),
        "来源元素键为 url / title / pageAge"
    );
    assert_eq!(
        results[1]["title"],
        Value::Null,
        "title 缺席呈 null（不省略）"
    );
    assert_eq!(results[1]["pageAge"], Value::Null);
    assert_eq!(value["error"], json!("max_uses_exceeded"), "error 为字符串");
    assert_eq!(
        value["rawUse"]["type"],
        json!("server_tool_use"),
        "rawUse 为对象"
    );
    assert_eq!(value["rawResult"]["type"], json!("web_search_tool_result"));

    let roundtrip: AgentBlock = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, block);
}

#[test]
fn web_search变体缺省键恒在且往返等价() {
    let bare = AgentBlock::WebSearch {
        id: "srvtoolu_bare".to_owned(),
        query: String::new(),
        results: Vec::new(),
        error: None,
        raw_use: None,
        raw_result: None,
    };

    let value = serde_json::to_value(&bare).expect("序列化成功");
    // 键恒在（只标 serde(default)，无 skip_serializing_if）：空 results 落 []，
    // 三个 Option 缺省落 null。反向判据：任一键缺席即说明 skip_serializing_if 回流。
    assert_eq!(
        value.get("results"),
        Some(&json!([])),
        "缺省 results 键恒在且落空数组，实际: {value}"
    );
    for key in ["error", "rawUse", "rawResult"] {
        assert_eq!(
            value.get(key),
            Some(&Value::Null),
            "缺省键 {key} 键恒在且落 null，实际: {value}"
        );
    }
    assert_eq!(value["kind"], json!("webSearch"));

    let roundtrip: AgentBlock = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, bare, "缺省形态往返等价");
}

#[test]
fn web_search块raw值往返逐字节保真含加密字段与嵌套数组() {
    let block = web_search_block();
    let AgentBlock::WebSearch {
        raw_use,
        raw_result,
        ..
    } = &block
    else {
        panic!("fixture 应为 WebSearch 块");
    };
    let serialized = serde_json::to_string(&block).expect("序列化成功");
    let roundtrip: AgentBlock = serde_json::from_str(&serialized).expect("反序列化成功");
    let AgentBlock::WebSearch {
        raw_use: back_use,
        raw_result: back_result,
        ..
    } = &roundtrip
    else {
        panic!("往返后仍为 WebSearch 块");
    };

    assert_eq!(
        serde_json::to_string(back_use.as_ref().expect("rawUse 在场")).expect("序列化成功"),
        serde_json::to_string(raw_use.as_ref().expect("rawUse 在场")).expect("序列化成功"),
        "rawUse 往返逐字节相等（encrypted_content 原样保全）"
    );
    assert_eq!(
        serde_json::to_string(back_result.as_ref().expect("rawResult 在场")).expect("序列化成功"),
        serde_json::to_string(raw_result.as_ref().expect("rawResult 在场")).expect("序列化成功"),
        "rawResult 往返逐字节相等"
    );
    assert_eq!(roundtrip, block, "整块往返逐字段相等");
}

#[test]
fn text块旧json无citations键读出none且再序列化落null键() {
    let legacy = json!({ "kind": "text", "text": "正文" });

    let block: AgentBlock = serde_json::from_value(legacy).expect("旧 JSON 必须可读（不报错）");
    match &block {
        AgentBlock::Text { text, citations } => {
            assert_eq!(text, "正文");
            assert_eq!(citations, &None, "旧 JSON 读出 citations == None");
        }
        other => panic!("应为 Text 块，实际: {other:?}"),
    }

    let value = serde_json::to_value(&block).expect("序列化成功");
    // 键恒在（D1 第四稿）：缺省再序列化落 citations: null。TS 面仍出可选位，
    // 故前端既有 fixture 零改动；反向判据：键缺席即说明 skip_serializing_if 回流。
    assert_eq!(
        value.get("citations"),
        Some(&Value::Null),
        "citations 键恒在且落 null，实际: {value}"
    );

    // 显式 citations: null 同样读出 None 且往返等价
    let explicit: AgentBlock =
        serde_json::from_value(json!({ "kind": "text", "text": "正文", "citations": null }))
            .expect("显式 null 可读");
    assert_eq!(explicit, block, "显式 null 与缺键解出同一形态");
    let explicit_value = serde_json::to_value(&explicit).expect("序列化成功");
    assert_eq!(explicit_value.get("citations"), Some(&Value::Null));
}

#[test]
fn text块citations在场形态为数组且空数组不折叠() {
    let block = AgentBlock::Text {
        text: "带引用正文".to_owned(),
        citations: Some(vec![citation_fixture(), citation_fixture()]),
    };

    let value = serde_json::to_value(&block).expect("序列化成功");
    let citations = value["citations"].as_array().expect("citations 为数组");
    assert_eq!(citations.len(), 2);
    assert_eq!(citations[0]["url"], json!("https://cite.example.com/1"));
    assert_eq!(citations[0]["title"], json!("引用标题"));
    assert_eq!(
        citations[0]["citedText"],
        json!("被引用的原文片段 🎉"),
        "cited_text 线键为 citedText"
    );
    assert_eq!(
        citations[0]["raw"]["encrypted_index"],
        json!("不确定字段"),
        "raw 原值落线格式"
    );

    // Some(vec![]) → 落 "citations": []（折叠归 citations_of，序列化层不折叠）
    let empty = AgentBlock::Text {
        text: "空引用".to_owned(),
        citations: Some(Vec::new()),
    };
    let empty_value = serde_json::to_value(&empty).expect("序列化成功");
    assert_eq!(empty_value["citations"], json!([]), "空数组形态落线格式");

    let roundtrip: AgentBlock = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, block);
}

#[test]
fn agent_citation三展示字段为空时呈null不省略且往返相等() {
    let citation = AgentCitation {
        url: None,
        title: None,
        cited_text: None,
        raw: Value::Null,
    };

    let value = serde_json::to_value(&citation).expect("序列化成功");
    for key in ["url", "title", "citedText"] {
        assert!(
            value.get(key).is_some(),
            "Option 字段不加 skip：{key} 键恒在场"
        );
        assert_eq!(value[key], Value::Null, "{key} 呈 null（固定形态）");
    }
    assert_eq!(value["raw"], Value::Null);

    let roundtrip: AgentCitation = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, citation, "全空形态往返相等");
    assert_eq!(citation.clone(), citation, "Clone / PartialEq 派生可用");
}

#[test]
fn web_search_source形态_url必填且title与page_age缺席呈null() {
    let full = source_fixture("https://a.example.com/1", Some("甲"), Some("2025-10-01"));
    let value = serde_json::to_value(&full).expect("序列化成功");
    assert_eq!(
        value,
        json!({ "url": "https://a.example.com/1", "title": "甲", "pageAge": "2025-10-01" })
    );

    let absent = source_fixture("https://b.example.com/2", None, None);
    let absent_value = serde_json::to_value(&absent).expect("序列化成功");
    assert_eq!(
        absent_value["title"],
        Value::Null,
        "title 缺席呈 null（不省略）"
    );
    assert_eq!(absent_value["pageAge"], Value::Null);

    // url 空串合法（类型非 Option，构造不校验）
    let empty_url = source_fixture("", None, None);
    let empty_value = serde_json::to_value(&empty_url).expect("序列化成功");
    assert_eq!(empty_value["url"], json!(""));
    let roundtrip: WebSearchSource = serde_json::from_value(empty_value).expect("反序列化成功");
    assert_eq!(roundtrip, empty_url);
}

#[test]
fn 非法载荷拒收_缺id与未知kind与类型不符均err() {
    // kind 缺 id
    assert!(
        serde_json::from_value::<AgentBlock>(json!({ "kind": "webSearch", "query": "q" })).is_err(),
        "缺 id 必须 Err"
    );
    // 缺 query
    assert!(
        serde_json::from_value::<AgentBlock>(json!({ "kind": "webSearch", "id": "a" })).is_err(),
        "缺 query 必须 Err"
    );
    // kind 未知值
    for kind in ["webSearchX", "", "WebSearch"] {
        assert!(
            serde_json::from_value::<AgentBlock>(json!({ "kind": kind, "id": "a", "query": "q" }))
                .is_err(),
            "未知 kind {kind:?} 必须 Err"
        );
    }
    // citations 为对象 / 字符串 → Err；null 则解为 None
    for citations in [json!({ "url": "https://a" }), json!("不是数组")] {
        assert!(
            serde_json::from_value::<AgentBlock>(
                json!({ "kind": "text", "text": "t", "citations": citations })
            )
            .is_err(),
            "citations 形态非法必须 Err"
        );
    }
    let null_citations: AgentBlock =
        serde_json::from_value(json!({ "kind": "text", "text": "t", "citations": null }))
            .expect("citations: null 解为 None（Option 语义不报错）");
    assert!(matches!(&null_citations, AgentBlock::Text { citations, .. } if citations.is_none()));

    // results 为 null / 对象 → Err（非 Option 列表）
    for results in [Value::Null, json!({ "url": "https://a" })] {
        assert!(
            serde_json::from_value::<AgentBlock>(
                json!({ "kind": "webSearch", "id": "a", "query": "q", "results": results })
            )
            .is_err(),
            "results 形态非法必须 Err"
        );
    }
    // raw 键缺席 → Err（Value 无默认值，值域受控、不静默兜底）
    assert!(
        serde_json::from_value::<AgentCitation>(json!({ "url": "https://a" })).is_err(),
        "AgentCitation 缺 raw 必须 Err"
    );
    // WebSearchSource 缺 url → Err
    assert!(
        serde_json::from_value::<WebSearchSource>(json!({ "title": "甲" })).is_err(),
        "WebSearchSource 缺 url 必须 Err"
    );
}

#[test]
fn 字符串与列表边界_超长特殊字符与百条来源往返逐字节() {
    let long_query = format!("{}\n含换行\0与 emoji 🎉", "查".repeat(1200));
    let sources: Vec<WebSearchSource> = (0..100)
        .map(|index| source_fixture(&format!("https://s.example.com/{index}"), None, None))
        .collect();
    let block = AgentBlock::WebSearch {
        id: String::new(),
        query: long_query.clone(),
        results: sources,
        error: None,
        raw_use: Some(json!({})),
        raw_result: Some(json!({ "deep": [[[1]]] })),
    };

    let value = serde_json::to_value(&block).expect("序列化成功");
    let roundtrip: AgentBlock = serde_json::from_value(value).expect("反序列化成功");

    match &roundtrip {
        AgentBlock::WebSearch {
            id,
            query,
            results,
            raw_use,
            raw_result,
            ..
        } => {
            assert_eq!(id, "", "id 空串合法");
            assert_eq!(query, &long_query, "超长 / 特殊字符逐字节往返");
            assert!(query.chars().count() > 1000, "fixture 超 1000 字符");
            assert_eq!(results.len(), 100, "百条来源保序无损");
            assert_eq!(results[99].url, "https://s.example.com/99");
            assert_eq!(raw_use, &Some(json!({})), "空对象 raw 往返相等");
            assert_eq!(
                raw_result,
                &Some(json!({ "deep": [[[1]]] })),
                "深嵌套往返相等"
            );
        }
        other => panic!("应为 WebSearch 块，实际: {other:?}"),
    }

    // 单元素列表（边界下界）
    let single = AgentBlock::WebSearch {
        id: "srvtoolu_one".to_owned(),
        query: "单条".to_owned(),
        results: vec![source_fixture("https://only/1", Some("唯一"), None)],
        error: None,
        raw_use: None,
        raw_result: None,
    };
    let single_value = serde_json::to_value(&single).expect("序列化成功");
    assert_eq!(single_value["results"].as_array().expect("数组").len(), 1);
}

#[test]
fn 五变体信封整体往返逐字段相等且agentdelta词汇仍为两变体() {
    // 六事件信封整体往返：Message 变体携 Text{citations} + WebSearch 两块
    for event in variant_events() {
        let value = serde_json::to_value(&event).expect("序列化成功");
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event, "seq={} 整体往返逐字段相等", event.seq);
    }

    let sealed = variant_events()
        .into_iter()
        .find(|event| matches!(event.kind, AgentEventKind::Message { .. }))
        .expect("Message 代表事件在场");
    let AgentEventKind::Message { blocks, .. } = &sealed.kind else {
        panic!("应为 Message 事件");
    };
    let value = serde_json::to_value(&sealed.kind).expect("序列化成功");
    assert_eq!(
        value["blocks"][0]["citations"]
            .as_array()
            .expect("数组")
            .len(),
        1,
        "citations 随信封往返在场"
    );
    assert_eq!(value["blocks"][1]["kind"], json!("webSearch"));
    assert!(matches!(&blocks[1], AgentBlock::WebSearch { .. }));

    // 决策 ⑦ 机械判据：AgentDelta 恰两变体（无 Search 变体）——穷举 match 不写
    // `_` 分支，新增第三变体即编译失败
    for delta in [
        AgentDelta::Text {
            text: String::new(),
        },
        AgentDelta::Thinking {
            thinking: String::new(),
        },
    ] {
        match delta {
            AgentDelta::Text { .. } | AgentDelta::Thinking { .. } => {}
        }
    }
}

// ---------------------------------------------------------------------------
// 信封轮归属 turn_id（agent-event-turn-id）
//
// 契约面断言：轮归属为 `Option<i64>`、线格式键 `turnId` 恒在（`None` → null）、
// 缺键旧载荷仍可读（AC-5）、`Some` 往返保真（AC-7 / AC-1）。不新增第二条生产
// 入口；本组全部调用以三参形态 `stamp(turn_id, seq, kind)` 书写。
// ---------------------------------------------------------------------------

/// 六变体线格式判别值（与既有六变体用例同序）。
const TURN_VARIANT_KINDS: [&str; 6] = [
    "runStarted",
    "messageDelta",
    "message",
    "systemNotice",
    "turnDone",
    "raw",
];

/// 六变体各一枚以给定轮归属盖戳（复用既有 `variant_events()` 的变体载荷，
/// 轮归属就地指定——不修改既有夹具语义）。
fn turn_scoped_events(turn_id: Option<i64>) -> Vec<AgentEvent> {
    variant_events()
        .into_iter()
        .enumerate()
        .map(|(index, event)| AgentEvent::stamp(turn_id, index as u64, event.kind))
        .collect()
}

/// 无轮归属的出行形态（时钟不可注入，故用 Raw 变体承载）。
fn raw_kind() -> AgentEventKind {
    AgentEventKind::Raw {
        event_type: "x".to_owned(),
        raw_json: "{}".to_owned(),
    }
}

/// 存量转录载荷形态：当前线格式去掉 `turnId` 键（旧行原文面）。
fn legacy_value_without_turn_id(event: &AgentEvent) -> Value {
    let mut value = serde_json::to_value(event).expect("序列化成功");
    let object = value.as_object_mut().expect("顶层为对象");
    assert!(
        object.remove("turnId").is_some(),
        "前置：当前线格式含 turnId 键"
    );
    value
}

#[test]
fn stamp对some轮归属逐字保留且时间戳落在调用区间() {
    let expected_kind = raw_kind();
    let before = millis_now();
    let event = AgentEvent::stamp(Some(7), 3, expected_kind.clone());
    let after = millis_now();

    assert_eq!(event.turn_id, Some(7), "轮归属逐字保留（首参）");
    assert_eq!(event.seq, 3, "seq 逐字保留");
    assert_eq!(event.kind, expected_kind, "kind 逐字保留");
    assert!(
        event.timestamp_ms >= before && event.timestamp_ms <= after,
        "时间戳落在调用时刻区间内，实际: {}（区间 [{before}, {after}]）",
        event.timestamp_ms
    );
}

#[test]
fn stamp首参none不改盖戳口径且其余字段与扩参前等价() {
    let before = millis_now();
    let event = AgentEvent::stamp(
        None,
        0,
        AgentEventKind::TurnDone {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(1),
            duration_ms: Some(10),
            cost_usd: None,
            usage: json!({ "input_tokens": 1 }),
            session_id: None,
        },
    );
    let after = millis_now();

    assert_eq!(event.turn_id, None, "无轮归属构造面读出 None");
    assert_eq!(event.seq, 0);
    assert!(
        event.timestamp_ms >= before && event.timestamp_ms <= after,
        "时间戳仍取 now_millis()（None 不改盖戳口径）"
    );
    assert!(
        matches!(&event.kind, AgentEventKind::TurnDone { subtype, .. } if subtype == "success"),
        "kind 逐字保留，实际: {:?}",
        event.kind
    );
}

#[test]
fn turn_id五形态经盖戳与往返逐字相等不夹取不改符号() {
    for turn_id in [None, Some(0), Some(-1), Some(i64::MIN), Some(i64::MAX)] {
        let event = AgentEvent::stamp(turn_id, 3, raw_kind());
        assert_eq!(event.turn_id, turn_id, "轮归属逐字相等: {turn_id:?}");

        let value = serde_json::to_value(&event).expect("序列化成功");
        assert_eq!(
            value["turnId"],
            serde_json::to_value(turn_id).expect("Option<i64> 可 JSON 化"),
            "线格式 turnId 逐字等于入参: {turn_id:?}"
        );
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event, "往返逐字段相等: {turn_id:?}");
    }
}

#[test]
fn seq三形态与轮归属极值正交且无截断() {
    for seq in [0_u64, 1, u64::MAX] {
        let event = AgentEvent::stamp(Some(i64::MAX), seq, raw_kind());

        assert_eq!(event.seq, seq, "seq 原样保留");
        assert_eq!(event.turn_id, Some(i64::MAX), "轮归属不被 seq 覆盖");
        let value = serde_json::to_value(&event).expect("序列化成功");
        assert_eq!(value["seq"], json!(seq), "seq 线格式不截断");
        assert_eq!(value["turnId"], json!(i64::MAX), "轮归属线格式不截断");
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event, "两字段互不干扰地往返");
    }
}

#[test]
fn 六变体线格式顶层turn_id为驼峰键且kind仍扁平() {
    for (event, expected_kind) in turn_scoped_events(Some(7))
        .into_iter()
        .zip(TURN_VARIANT_KINDS)
    {
        let value = serde_json::to_value(&event).expect("序列化成功");
        let object = value.as_object().expect("顶层为对象");

        assert_eq!(value["turnId"], json!(7), "顶层 turnId 为数字");
        assert_eq!(value["kind"], json!(expected_kind), "判别值为驼峰字符串");
        assert!(
            object.contains_key("turnId")
                && object.contains_key("seq")
                && object.contains_key("timestampMs")
                && object.contains_key("kind"),
            "信封四键与变体字段同层（turnId 不是变体内键），实际键: {object:?}"
        );
        // 轮归属与 TurnDone.session_id（引擎侧标识）同层并存、互不影响
        if expected_kind == "turnDone" {
            assert_eq!(value["sessionId"], json!("s-1"));
            assert_ne!(
                value["turnId"], value["sessionId"],
                "轮归属（数字）与引擎侧会话标识（字符串）语义不可混用"
            );
        }
    }
}

#[test]
fn turn_id为none时线格式键恒在且值为null() {
    let event = AgentEvent::stamp(None, 0, raw_kind());
    let value = serde_json::to_value(&event).expect("序列化成功");
    let object = value.as_object().expect("顶层为对象");

    assert!(
        object.contains_key("turnId"),
        "None 形态键恒在（反向判据：键缺席即说明 skip_serializing_if 被加上），实际: {object:?}"
    );
    assert_eq!(value["turnId"], Value::Null, "None 落 null");
}

#[test]
fn 六变体顶层键集合只增turn_id一个信封键() {
    let expected_own_keys: [&[&str]; 6] = [
        &["model", "sessionId", "tools", "mcpServers"],
        &["parentToolUseId", "delta"],
        &["role", "blocks", "parentToolUseId"],
        &["subtype", "payload"],
        &[
            "subtype",
            "isError",
            "numTurns",
            "durationMs",
            "costUsd",
            "usage",
            "sessionId",
        ],
        &["eventType", "rawJson"],
    ];

    for (event, own_keys) in turn_scoped_events(Some(7))
        .into_iter()
        .zip(expected_own_keys)
    {
        let value = serde_json::to_value(&event).expect("序列化成功");
        let object = value.as_object().expect("顶层为对象");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();

        let mut expected: Vec<&str> = vec!["turnId", "seq", "timestampMs", "kind"];
        expected.extend_from_slice(own_keys);
        expected.sort_unstable();

        assert_eq!(
            keys, expected,
            "顶层键集合 = 信封四键 ∪ 变体自有键（信封字段面只增 turnId、无相位伴生键），实际: {object:?}"
        );
    }
}

#[test]
fn 六变体序列化文本中turn_id片段恰出现一次() {
    for event in turn_scoped_events(Some(7)) {
        let text = serde_json::to_string(&event).expect("序列化成功");
        assert_eq!(
            text.matches("\"turnId\"").count(),
            1,
            "kind 扁平且信封字段不重复落键（出现 2 次即说明被误置于变体内层），实际: {text}"
        );
    }
}

#[test]
fn 六变体旧json无turn_id键仍可读且读出none其余字段不变() {
    for (event, expected_kind) in turn_scoped_events(Some(7))
        .into_iter()
        .zip(TURN_VARIANT_KINDS)
    {
        let legacy = legacy_value_without_turn_id(&event);
        let decoded: AgentEvent =
            serde_json::from_value(legacy).expect("旧 JSON 必须可读（缺键不使整条事件解码失败）");

        assert_eq!(decoded.turn_id, None, "缺键读出 None");
        assert_eq!(decoded.seq, event.seq, "seq 逐字段不变");
        assert_eq!(decoded.timestamp_ms, event.timestamp_ms, "时间戳逐字段不变");
        assert_eq!(decoded.kind, event.kind, "kind 与变体字段逐字段不变");
        let value = serde_json::to_value(&decoded).expect("序列化成功");
        assert_eq!(value["kind"], json!(expected_kind));
    }
}

#[test]
fn 显式null与缺键解出同一形态且再序列化均键恒在() {
    let baseline = AgentEvent::stamp(None, 7, raw_kind());
    let missing = legacy_value_without_turn_id(&baseline);
    let mut explicit = missing.clone();
    explicit["turnId"] = Value::Null;

    let from_missing: AgentEvent = serde_json::from_value(missing).expect("缺键可读");
    let from_explicit: AgentEvent = serde_json::from_value(explicit).expect("显式 null 可读");

    assert_eq!(
        from_missing, from_explicit,
        "显式 null 与缺键解出同一形态（不可分辨地归一到 None）"
    );
    assert_eq!(from_missing, baseline);
    for event in [&from_missing, &from_explicit] {
        let value = serde_json::to_value(event).expect("序列化成功");
        assert_eq!(
            value.get("turnId"),
            Some(&Value::Null),
            "再序列化均落 turnId:null（键恒在）"
        );
    }
}

#[test]
fn 非法turn_id形态四类均err不产半成品事件() {
    let base = legacy_value_without_turn_id(&AgentEvent::stamp(None, 0, raw_kind()));

    for invalid in [
        json!("7"),
        json!(7.5),
        json!(9223372036854775808_u64),
        json!([7]),
    ] {
        let mut value = base.clone();
        value["turnId"] = invalid.clone();
        assert!(
            serde_json::from_value::<AgentEvent>(value).is_err(),
            "非法 turnId 形态 {invalid} 必须 Err（不产半成品事件）"
        );
    }
    // 对照组：合法数字可读
    let mut valid = base;
    valid["turnId"] = json!(7);
    assert!(serde_json::from_value::<AgentEvent>(valid).is_ok());
}

#[test]
fn snake_case键输入不产出轮归属_线键只认camel_case() {
    let mut value = legacy_value_without_turn_id(&AgentEvent::stamp(None, 0, raw_kind()));
    value["turn_id"] = json!(7);

    // 析取形态：未知键被忽略则读回 None，被拒绝则 Err——两种策略都不认 snake_case
    if let Ok(event) = serde_json::from_value::<AgentEvent>(value) {
        assert_ne!(
            event.turn_id,
            Some(7),
            "snake_case 键不得被识别为轮归属（线键只认 camelCase turnId）"
        );
    }
}

#[test]
fn some轮归属六变体往返逐字段相等() {
    for event in turn_scoped_events(Some(7)) {
        let value = serde_json::to_value(&event).expect("序列化成功");
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event, "seq={} 轮归属参与往返", event.seq);
        assert_eq!(roundtrip.turn_id, Some(7));
    }
}

#[test]
fn none轮归属六变体往返闭环() {
    for event in turn_scoped_events(None) {
        let value = serde_json::to_value(&event).expect("序列化成功");
        assert_eq!(value["turnId"], Value::Null, "None 落 null");
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, event, "seq={} null 与 None 闭环", event.seq);
    }
}

#[test]
fn 轮归属极值经json文本往返不失符号不截断() {
    for turn_id in [Some(i64::MIN), Some(i64::MAX)] {
        let event = AgentEvent::stamp(turn_id, 2, raw_kind());
        let text = serde_json::to_string(&event).expect("序列化成功");
        let roundtrip: AgentEvent = serde_json::from_str(&text).expect("JSON 文本往返成功");
        assert_eq!(roundtrip, event, "文本面不发生 f64 化截断: {turn_id:?}");
        assert_eq!(roundtrip.turn_id, turn_id, "符号与量值逐字保留");
    }
}

#[test]
fn turn_id参与相等性比较不被skip() {
    let event = AgentEvent::stamp(Some(7), 3, raw_kind());
    let mut other = event.clone();
    other.turn_id = Some(8);

    assert_ne!(event, other, "turn_id 不同即事件不等（derive 面敏感）");
    assert_eq!(event, event.clone(), "同 turn_id 同形态相等");
    let mut same = other.clone();
    same.turn_id = Some(7);
    assert_eq!(
        event, same,
        "turn_id 相同且其余全同时相等（该字段被 PartialEq 纳入比较，未被 skip）"
    );
}
