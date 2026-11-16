//! `web_search` 能力模块单测：常量面 / provider tool / raw 原语 / 回放构造。
//!
//! 投影与回放的**分派**住 `blocks`（其单测见 `blocks_test`）。
//!
//! 装置约定（见 test-design `Mock策略`）：**无 Mock**——全部为纯函数 + 值对象，
//! `rig::message` 与 `serde_json::Value` 仅作构造载荷（不 mock rig），零 I/O /
//! 零 async / 零网络 / 零全局态。

use agent::AgentCitation;
use rig::completion::{CompletionRequest, ProviderToolDefinition};
use rig::message::{AdditionalParams, AssistantContent, Text};
use serde_json::{json, Value};

use crate::sdk::web_search::{
    citations_of, provider_tool, raw_content, replay_raw, replay_text, ANTHROPIC_RAW_CONTENT_KEY,
    CITATIONS_KEY, WEB_SEARCH_MAX_USES, WEB_SEARCH_TOOL_KIND, WEB_SEARCH_TOOL_NAME,
};

// ---------------------------------------------------------------------------
// 装置：raw wire 载荷构造与 Text 块构造（rig 类型真实构造，不 mock）
// ---------------------------------------------------------------------------

/// 携原始 wire 的文本块（raw server 块在 rig 面的承载形态：**空文本 + 原始值**）。
fn raw_block(raw: Value) -> AssistantContent {
    AssistantContent::Text(Text {
        text: String::new(),
        additional_params: AdditionalParams::from_entries([(ANTHROPIC_RAW_CONTENT_KEY, raw)]),
    })
}

/// 携 `additional_params` 的文本块（键值逐条给出，用于 raw_content /
/// citations_of 的多键与类型异常形态）。
fn text_with_entries(text: &str, entries: Vec<(&str, Value)>) -> Text {
    Text {
        text: text.to_owned(),
        additional_params: AdditionalParams::from_entries(entries),
    }
}

/// 摘 `AssistantContent::Text`（其余变体 panic）。
fn as_text(content: &AssistantContent) -> &Text {
    match content {
        AssistantContent::Text(text) => text,
        other => panic!("应为 Text 变体，实际: {other:?}"),
    }
}

/// 摘 `additional_params` 的键值（缺席 panic）。
fn param_of<'a>(text: &'a Text, key: &str) -> &'a Value {
    text.additional_params
        .as_ref()
        .unwrap_or_else(|| panic!("additional_params 应在场"))
        .get(key)
        .unwrap_or_else(|| panic!("{key} 键应在场"))
}

// ---------------------------------------------------------------------------
// provider_tool：请求侧形状与常量面
// ---------------------------------------------------------------------------

/// 空底座请求（字段面与 loop 侧构造同口径：全部 None）。
fn empty_request() -> CompletionRequest {
    CompletionRequest {
        model: None,
        chat_history: Vec::new(),
        documents: Vec::new(),
        tools: Vec::new(),
        temperature: None,
        max_tokens: None,
        tool_choice: None,
        additional_params: None,
        output_schema: None,
        record_telemetry_content: false,
    }
}

#[test]
fn provider_tool经provider_tool落additional_params后形状恰一项三键() {
    let request = empty_request().provider_tool(provider_tool());
    let params = request
        .additional_params
        .expect("provider_tool 必须落 additional_params");

    // 深等于：恰一项、恰三键（无 cache_control / allowed_domains 等附加键）
    assert_eq!(
        params["tools"],
        json!([{ "type": "web_search_20250305", "name": "web_search", "max_uses": 5 }]),
        "additional_params.tools 形状逐字为 [{{type,name,max_uses}}]"
    );

    let tools = params["tools"].as_array().expect("tools 为数组");
    assert_eq!(tools.len(), 1, "恰好一项服务端工具");
    let entry = tools[0].as_object().expect("工具项为对象");
    let mut keys: Vec<&str> = entry.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["max_uses", "name", "type"], "恰三键，无附加键");

    // 类型面：type / name 为字符串、max_uses 为 JSON 数字 5（非 "5"）
    assert_eq!(entry["type"], json!(WEB_SEARCH_TOOL_KIND));
    assert_eq!(entry["name"], json!(WEB_SEARCH_TOOL_NAME));
    assert!(
        entry["type"].is_string() && entry["name"].is_string(),
        "两键为字符串"
    );
    assert!(
        entry["max_uses"].is_number(),
        "max_uses 为 JSON 数字（非字符串）"
    );
    assert_eq!(entry["max_uses"], json!(5));
}

#[test]
fn 常量面上限为5且连续两次调用产出深相等值() {
    assert_eq!(
        WEB_SEARCH_MAX_USES, 5,
        "单请求搜索次数上限常量 = 5（成本护栏）"
    );
    assert_eq!(
        WEB_SEARCH_TOOL_KIND, "web_search_20250305",
        "kind 字面量单点"
    );
    assert_eq!(WEB_SEARCH_TOOL_NAME, "web_search", "name 字面量单点");
    assert_eq!(
        ANTHROPIC_RAW_CONTENT_KEY, "anthropic_content",
        "raw 承载键单点"
    );
    assert_eq!(CITATIONS_KEY, "citations", "citations 承载键单点");

    // 纯函数性：连续两次调用无内部状态差异（无静态可变全局）
    let first: ProviderToolDefinition = provider_tool();
    let second = provider_tool();
    assert_eq!(first.kind, second.kind, "kind 两次调用一致");
    assert_eq!(first.config, second.config, "config 两次调用深相等");

    // 与 `loop::PAUSE_TURN_MAX_CONTINUATIONS`（单轮续跑上限）为两个概念：
    // 常量名不同源，此处只锚定本常量自身类型与取值，MUST NOT 与续跑上限混用
    let max_uses: u32 = WEB_SEARCH_MAX_USES;
    assert_eq!(max_uses, 5);
}

// ---------------------------------------------------------------------------
// raw_content：命中 / 缺席 / 非对象值
// ---------------------------------------------------------------------------

#[test]
fn raw_content命中时逐字节返回原始wire值() {
    let raw = json!({
        "type": "server_tool_use",
        "id": "srvtoolu_1",
        "name": "web_search",
        "input": { "query": "今日汇率" },
        "encrypted_content": "不可推导的加密索引 🎉\n",
        "nested": [1, 2, { "deep": [true, null] }],
    });
    let text = text_with_entries("", vec![(ANTHROPIC_RAW_CONTENT_KEY, raw.clone())]);

    let hit = raw_content(&text).expect("键在场必须命中");
    assert_eq!(hit, &raw, "与入参深相等");
    assert_eq!(
        serde_json::to_string(hit).expect("序列化成功"),
        serde_json::to_string(&raw).expect("序列化成功"),
        "逐字节相等（含 encrypted_content 等不确定字段）"
    );
    assert_eq!(
        as_text(&raw_block(raw.clone())).text,
        "",
        "raw 块承载形态为**空文本**（rig 硬校验）"
    );
}

#[test]
fn raw_content缺席三形态一律none() {
    // additional_params == None
    let bare = Text::new("普通正文");
    assert!(raw_content(&bare).is_none(), "无 additional_params → None");

    // Some({})：空对象经 AdditionalParams 规范化为 None（等价缺席）
    let empty_map = text_with_entries("正文", Vec::new());
    assert!(
        empty_map.additional_params.is_none(),
        "空键集规范化为 None（rig 语义）"
    );
    assert!(raw_content(&empty_map).is_none(), "空 params → None");

    // 其他键在场但无 raw 承载键
    let other_key = text_with_entries("正文", vec![(CITATIONS_KEY, json!([]))]);
    assert!(raw_content(&other_key).is_none(), "无 raw 键 → None");

    // 键名大小写不符（键名大小写敏感）
    let wrong_case = text_with_entries("正文", vec![("ANTHROPIC_CONTENT", json!({ "a": 1 }))]);
    assert!(
        raw_content(&wrong_case).is_none(),
        "键名大小写敏感：ANTHROPIC_CONTENT 不命中"
    );
}

#[test]
fn raw_content对非对象值原样返回some且多键共存只取raw键() {
    // 值域四形态：null / 字符串 / 数字 / 数组 → 均原样 Some（不做类型校验）
    for value in [
        Value::Null,
        json!("字符串载荷"),
        json!(42),
        json!([1, 2, 3]),
    ] {
        let text = text_with_entries("", vec![(ANTHROPIC_RAW_CONTENT_KEY, value.clone())]);
        let hit = raw_content(&text).expect("键在场即 Some（不校验值类型）");
        assert_eq!(hit, &value, "值 {value} 原样返回，不 panic");
    }

    // 多键共存：只取 raw 键，不串到 citations
    let text = text_with_entries(
        "",
        vec![
            (
                ANTHROPIC_RAW_CONTENT_KEY,
                json!({ "type": "server_tool_use" }),
            ),
            (CITATIONS_KEY, json!([{ "url": "https://a" }])),
        ],
    );
    assert_eq!(
        raw_content(&text).expect("raw 键在场"),
        &json!({ "type": "server_tool_use" }),
        "只取 anthropic_content，不串 citations 键"
    );
}

// ---------------------------------------------------------------------------
// citations_of：正常提取 / 空折叠 / 类型异常
// ---------------------------------------------------------------------------

#[test]
fn citations_of正常提取两条其中一条缺title字段逐项映射且raw逐字节相等() {
    let c1 = json!({
        "type": "web_search_result_location",
        "url": "https://a.example.com/1",
        "title": "甲标题",
        "cited_text": "被引用的原文片段 🎉",
    });
    // 缺 title 的一条（展示字段缺席 → None，不报错、不丢元素）
    let c2 = json!({
        "type": "web_search_result_location",
        "url": "https://b.example.com/2",
        "cited_text": "另一段原文\n含换行",
    });
    let text = text_with_entries("正文", vec![(CITATIONS_KEY, json!([c1, c2]))]);

    let citations = citations_of(&text).expect("键在场且非空 → Some");

    assert_eq!(citations.len(), 2, "两条引用逐一保留");
    assert_eq!(citations[0].url.as_deref(), Some("https://a.example.com/1"));
    assert_eq!(citations[0].title.as_deref(), Some("甲标题"));
    assert_eq!(
        citations[0].cited_text.as_deref(),
        Some("被引用的原文片段 🎉")
    );
    assert_eq!(citations[0].raw, c1, "raw 与数组元素逐字节相等");
    assert_eq!(citations[1].url.as_deref(), Some("https://b.example.com/2"));
    assert_eq!(
        citations[1].title, None,
        "缺键项对应字段 None（不规范化成字符串 null）"
    );
    assert_eq!(citations[1].raw, c2);
}

#[test]
fn citations_of键缺席与none_params与空数组一律none() {
    // 键缺席
    let absent = text_with_entries("正文", vec![(ANTHROPIC_RAW_CONTENT_KEY, json!({}))]);
    assert!(citations_of(&absent).is_none(), "citations 键缺席 → None");

    // additional_params == None
    let bare = Text::new("正文");
    assert!(citations_of(&bare).is_none(), "无 additional_params → None");

    // 空数组：空 citations 在 Rust 侧即折叠为 None（前端无需第二判定）
    let empty = text_with_entries("正文", vec![(CITATIONS_KEY, json!([]))]);
    assert!(
        citations_of(&empty).is_none(),
        "空数组 → None（不挂 metadata）"
    );
}

#[test]
fn citations_of类型异常非数组一律none且元素非对象仍逐条产出() {
    // 键值非数组：对象 / 字符串 / null → None
    for value in [
        json!({ "url": "https://a" }),
        json!("不是数组"),
        Value::Null,
    ] {
        let text = text_with_entries("正文", vec![(CITATIONS_KEY, value.clone())]);
        assert!(
            citations_of(&text).is_none(),
            "键值 {value} 非数组 → None（不 panic）"
        );
    }

    // 数组元素为非对象：该元素仍产 AgentCitation（raw 为原值），元素数不丢
    let text = text_with_entries(
        "正文",
        vec![(CITATIONS_KEY, json!(["字符串元素", 7, null]))],
    );
    let citations = citations_of(&text).expect("数组非空即 Some");
    assert_eq!(citations.len(), 3, "元素数不丢");
    for (index, citation) in citations.iter().enumerate() {
        assert!(
            citation.url.is_none() && citation.title.is_none() && citation.cited_text.is_none()
        );
        assert_eq!(
            citation.raw,
            json!(["字符串元素", 7, null])[index],
            "raw 原值逐条保留"
        );
    }
}

// ---------------------------------------------------------------------------
// replay_raw / replay_text：回放侧形状与保真
// ---------------------------------------------------------------------------

#[test]
fn replay_raw形状为空文本加原始wire且连续两次调用相等() {
    let raw = json!({
        "type": "web_search_tool_result",
        "tool_use_id": "srvtoolu_1",
        "content": [{ "type": "web_search_result", "url": "https://a/1" }],
        "encrypted_content": "加密索引 🎉",
        "cache_control": { "type": "ephemeral" },
        "nested": [[1, 2], [3]],
    });

    let content = replay_raw(&raw);
    let text = as_text(&content);
    assert_eq!(
        text.text, "",
        "text 必为空串（rig 侧带文本即 ConversionError）"
    );
    assert_eq!(
        param_of(text, ANTHROPIC_RAW_CONTENT_KEY),
        &raw,
        "raw 与入参逐字节相等"
    );
    let map = text
        .additional_params
        .as_ref()
        .expect("additional_params 在场")
        .as_map();
    assert_eq!(map.len(), 1, "additional_params 恰一个承载键");

    // 连续两次调用产出相等值（纯函数）
    assert_eq!(
        serde_json::to_string(&replay_raw(&raw)).expect("序列化成功"),
        serde_json::to_string(&content).expect("序列化成功"),
        "两次调用产物逐字节相等"
    );
}

#[test]
fn replay_raw对非对象载荷原样包裹不panic不校验type() {
    for raw in [
        Value::Null,
        json!([1, 2, 3]),
        json!({}),
        json!({ "deep": { "a": { "b": { "c": [1, 2, { "d": null }] } } } }),
        json!("字符串载荷"),
    ] {
        let content = replay_raw(&raw);
        let text = as_text(&content);
        assert_eq!(text.text, "", "恒空文本（载荷 {raw}）");
        assert_eq!(param_of(text, ANTHROPIC_RAW_CONTENT_KEY), &raw, "原样包裹");
    }
}

#[test]
fn replay_text无引用时不落任何承载键() {
    let content = replay_text("正文 🎉", None);

    let text = as_text(&content);
    assert_eq!(text.text, "正文 🎉", "原文保真");
    assert!(
        text.additional_params.is_none(),
        "无引用不落 citations / anthropic_content 键"
    );
}

#[test]
fn replay_text非空引用回填raw数组且display字段不参与重建() {
    // 构造 url 与 raw.url 不一致的 citation：产物 MUST 只等于 raw
    let citations = vec![
        AgentCitation {
            url: Some("https://display.example.com/1".to_owned()),
            title: Some("展示标题".to_owned()),
            cited_text: Some("展示片段".to_owned()),
            raw: json!({ "url": "https://raw.example.com/1", "cited_text": "原始片段", "extra": 1 }),
        },
        AgentCitation {
            url: Some("https://display.example.com/2".to_owned()),
            title: None,
            cited_text: None,
            raw: json!({ "url": "https://raw.example.com/2" }),
        },
    ];

    let content = replay_text("正文", Some(&citations));

    let text = as_text(&content);
    assert_eq!(text.text, "正文");
    assert_eq!(
        param_of(text, CITATIONS_KEY),
        &json!([
            { "url": "https://raw.example.com/1", "cited_text": "原始片段", "extra": 1 },
            { "url": "https://raw.example.com/2" },
        ]),
        "citations 深等于 [c.raw, …]（不以 url / title / cited_text 重组）"
    );
    // 不落 raw 承载键（正文走 citations，不是 raw server 块）
    assert!(
        text.additional_params
            .as_ref()
            .expect("params 在场")
            .get(ANTHROPIC_RAW_CONTENT_KEY)
            .is_none(),
        "正文块不落 anthropic_content 键"
    );
}

#[test]
fn replay_text空引用视同无引用且文本边界逐字节保真() {
    // 空切片
    let empty = replay_text("正文", Some(&[]));
    assert!(
        as_text(&empty).additional_params.is_none(),
        "空切片视同无引用（不落 citations 键）"
    );

    // 空文本 + 非空 citations
    let citations = vec![AgentCitation {
        url: Some("https://a/1".to_owned()),
        title: None,
        cited_text: None,
        raw: json!({ "url": "https://a/1" }),
    }];
    let empty_text = replay_text("", Some(&citations));
    let text = as_text(&empty_text);
    assert_eq!(text.text, "");
    assert_eq!(
        param_of(text, CITATIONS_KEY),
        &json!([{ "url": "https://a/1" }]),
        "空文本仍携 citations"
    );

    // 超长 / 含换行 / NUL / emoji 的正文逐字节保真
    let long = format!("{}\n行尾\0emoji 🎉", "长".repeat(1000));
    let roundtrip = replay_text(&long, None);
    assert_eq!(as_text(&roundtrip).text, long, "超长与特殊字符逐字节保真");
    assert!(long.chars().count() > 1000, "fixture 超过 1000 字符");
}
