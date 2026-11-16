//! Anthropic 云端 `web_search` 服务端工具：常量单点 + wire 原语 + 回放构造。

use agent::{AgentBlock, AgentCitation, WebSearchSource};
use rig::completion::ProviderToolDefinition;
use rig::message::{AdditionalParams, AssistantContent, Text};
use serde_json::{json, Value};

/// 请求侧 provider tool 的 `type`（anthropic 服务端工具版本化标识）。
pub(crate) const WEB_SEARCH_TOOL_KIND: &str = "web_search_20250305";

/// 请求侧 provider tool 的 `name`（同时是 `server_tool_use.name` 的配对判别值）。
pub(crate) const WEB_SEARCH_TOOL_NAME: &str = "web_search";

/// **单请求**搜索次数上限（成本护栏；不可配置——配置化需 store v5 迁移）。
/// 与 `loop::PAUSE_TURN_MAX_CONTINUATIONS`（单轮续跑上限）是两个概念。
pub(crate) const WEB_SEARCH_MAX_USES: u32 = 5;

/// rig 的 raw server 块承载键（与 rig 内部
/// `providers::anthropic::completion::ANTHROPIC_RAW_CONTENT_KEY` 同字面量；
/// rig 侧常量为 `pub(crate)`，故此处按 wire 事实单点重述）。
pub(crate) const ANTHROPIC_RAW_CONTENT_KEY: &str = "anthropic_content";

/// rig 的 citations 承载键（`Text.additional_params["citations"]`）。
pub(crate) const CITATIONS_KEY: &str = "citations";

/// raw 块的 wire `type` 判别值：服务端工具调用。
const SERVER_TOOL_USE_TYPE: &str = "server_tool_use";
/// raw 块的 wire `type` 判别值：网页搜索结果。
const WEB_SEARCH_RESULT_TYPE: &str = "web_search_tool_result";
/// result `content` 数组内单条来源的 wire `type`。
const WEB_SEARCH_SOURCE_TYPE: &str = "web_search_result";

/// 请求侧 provider tool 定义：`{type, name, max_uses}`（经
/// `CompletionRequest::provider_tool(..)` 落 `additional_params.tools`）。
pub(crate) fn provider_tool() -> ProviderToolDefinition {
    ProviderToolDefinition::new(WEB_SEARCH_TOOL_KIND)
        .with_config("name", json!(WEB_SEARCH_TOOL_NAME))
        .with_config("max_uses", json!(WEB_SEARCH_MAX_USES))
}

/// 文本块携的 raw server 块原文（`None` = 普通文本块）。
pub(crate) fn raw_content(text: &Text) -> Option<&Value> {
    text.additional_params
        .as_ref()?
        .get(ANTHROPIC_RAW_CONTENT_KEY)
}

/// 文本块携的引用列表：缺席 / 非数组 / **空数组** → `None`；`raw` 取数组元素
/// 原值（不经 rig `Citation` 反序列化再序列化，避免规范化失真）。展示字段
/// 缺失 / 类型不符 → `None`，不报错。
pub(crate) fn citations_of(text: &Text) -> Option<Vec<AgentCitation>> {
    let items = text
        .additional_params
        .as_ref()?
        .get(CITATIONS_KEY)?
        .as_array()?;
    if items.is_empty() {
        return None;
    }
    Some(
        items
            .iter()
            .map(|item| AgentCitation {
                url: text_field(item, "url"),
                title: text_field(item, "title"),
                cited_text: text_field(item, "cited_text"),
                raw: item.clone(),
            })
            .collect(),
    )
}

/// 预索引用：raw 为 `web_search` 的 `server_tool_use` 时取其 `id`
/// （`None` = 非本域 server use——其他 server 工具、结果块、缺 `id`）。
pub(crate) fn use_id(raw: &Value) -> Option<&str> {
    if raw_type(raw) != Some(SERVER_TOOL_USE_TYPE) {
        return None;
    }
    if raw.get("name").and_then(Value::as_str) != Some(WEB_SEARCH_TOOL_NAME) {
        return None;
    }
    raw.get("id").and_then(Value::as_str)
}

/// 预索引用：raw 为 `web_search_tool_result` 时取其 `tool_use_id`
/// （`None` = 非结果块或缺配对键的 wild 块）。
pub(crate) fn result_use_id(raw: &Value) -> Option<&str> {
    if raw_type(raw) != Some(WEB_SEARCH_RESULT_TYPE) {
        return None;
    }
    raw.get("tool_use_id").and_then(Value::as_str)
}

/// use 侧成块：`server_tool_use` 原文 + 可选配对 result 原文（载荷并入本块，
/// result 侧不再产块）。
pub(crate) fn server_block(raw_use: &Value, raw_result: Option<&Value>) -> AgentBlock {
    let id = text_field(raw_use, "id").unwrap_or_default();
    let (results, error) = match raw_result {
        Some(result) => result_payload(result),
        None => (Vec::new(), None),
    };
    AgentBlock::WebSearch {
        id,
        query: query_of(raw_use),
        results,
        error,
        raw_use: Some(raw_use.clone()),
        raw_result: raw_result.cloned(),
    }
}

/// 孤儿 result 成块（无配对 use）：载荷自 result 侧解，`query` 空串。
/// 返回 `None` 即无 `tool_use_id` 可承载（wild 块，调用方跳过）。
pub(crate) fn orphan_block(raw_result: &Value) -> Option<AgentBlock> {
    let id = text_field(raw_result, "tool_use_id")?;
    let (results, error) = result_payload(raw_result);
    Some(AgentBlock::WebSearch {
        id,
        query: String::new(),
        results,
        error,
        raw_use: None,
        raw_result: Some(raw_result.clone()),
    })
}

/// 回放侧 raw 构造：`Text { text: "", additional_params: { anthropic_content:
/// raw } }`——**必须空文本**（rig 侧带文本即 `ConversionError`）。
pub(crate) fn replay_raw(raw: &Value) -> AssistantContent {
    AssistantContent::Text(Text {
        text: String::new(),
        additional_params: AdditionalParams::from_entries([(
            ANTHROPIC_RAW_CONTENT_KEY,
            raw.clone(),
        )]),
    })
}

/// 回放侧正文构造：非空 citations 才挂 `additional_params.citations =
/// [raw, …]`（元素原值回填，逐字节保真）；否则纯 `Text`。
pub(crate) fn replay_text(text: &str, citations: Option<&[AgentCitation]>) -> AssistantContent {
    let entries = citations.filter(|items| !items.is_empty()).map(|items| {
        (
            CITATIONS_KEY,
            Value::Array(items.iter().map(|item| item.raw.clone()).collect()),
        )
    });
    AssistantContent::Text(Text {
        text: text.to_owned(),
        additional_params: AdditionalParams::from_entries(entries),
    })
}

/// raw 块的 wire `type`。
fn raw_type(raw: &Value) -> Option<&str> {
    raw.get("type").and_then(Value::as_str)
}

/// 宽松取值：键缺席 / 非字符串 → `None`（不报错）。
fn text_field(raw: &Value, key: &str) -> Option<String> {
    raw.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// 查询词：`server_tool_use.input.query`（形态不符 → 空串）。
fn query_of(raw: &Value) -> String {
    raw.get("input")
        .and_then(|input| input.get("query"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// result 载荷解析（纯展示）：`content` 为来源数组 → 逐条 `url` / `title` /
/// `page_age`；为错误对象 → `error_code`。类型不符 / 缺席一律空载荷（不报错、
/// 不丢块）。
fn result_payload(raw: &Value) -> (Vec<WebSearchSource>, Option<String>) {
    let Some(content) = raw.get("content") else {
        return (Vec::new(), None);
    };
    let Some(items) = content.as_array() else {
        return (Vec::new(), text_field(content, "error_code"));
    };
    let results = items
        .iter()
        .filter(|item| match raw_type(item) {
            Some(kind) => kind == WEB_SEARCH_SOURCE_TYPE,
            None => false,
        })
        .filter_map(|item| {
            Some(WebSearchSource {
                url: text_field(item, "url")?,
                title: text_field(item, "title"),
                page_age: text_field(item, "page_age"),
            })
        })
        .collect();
    (results, None)
}
