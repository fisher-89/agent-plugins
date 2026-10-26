//! Anthropic 云端 `web_search` 服务端工具：常量单点 + 六个纯函数
//! （`provider_tool` / `raw_content` / `citations_of` / `project_choice` /
//! `replay_raw` / `replay_text`）。
//!
//! 两条不变量（实现期 MUST NOT 破）：
//!
//! 1. **raw 块必须配空文本**：rig 的 anthropic 回放侧校验「raw 与文本不可
//!    共存」（非空文本 + `anthropic_content` → `ConversionError`），故
//!    [`replay_raw`] 恒产 `Text { text: "" }`；正文与引用走 [`replay_text`]。
//! 2. **展示字段不参与回放**：`query` / `results` / `error` 只供时间线呈现，
//!    重建只吃 `raw_use` / `raw_result` 的原始 wire 值——`encrypted_content` 等
//!    字段不可推导，任何经类型往返（如 rig `Citation` 的 `title: Option` →
//!    `null`）都会失真。
//!
//! 常量单点：`kind` / `name` / `max_uses` / raw 承载键 / citations 承载键全部
//! 住本模块，`loop` / `resume` / `context` 零字面量重复。
//!
//! 两轴上界分名：本模块的 [`WEB_SEARCH_MAX_USES`] 是**单请求**搜索次数上限
//! （成本护栏，不可配置——配置化需 store v5 迁移）；`loop` 的
//! `PAUSE_TURN_MAX_CONTINUATIONS` 是**单轮**续跑上限，两者概念不同，常量名 /
//! 注释 / 日志字段 MUST NOT 混用。

use std::collections::{HashMap, HashSet};

use agent::{AgentBlock, AgentCitation, WebSearchSource};
use rig::completion::ProviderToolDefinition;
use rig::message::{AdditionalParams, AssistantContent, Text, ToolCall};
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

/// 块投影唯一实现（`loop` 只调用，不再各自解 raw）：先**完整预索引**（收集
/// 全部 `web_search` 的 `server_tool_use` id + 按 `tool_use_id` 索引全部
/// `web_search_tool_result`，并在预索引期即预置「已消费」下标），再顺序遍历
/// choice——故 result 与 use 的**顺序无关**（`pause_turn` 拆分后 result 可能
/// 先到）：
///
/// - `server_tool_use`（`name == "web_search"`）→ 一块
///   [`AgentBlock::WebSearch`]，其配对 result 的载荷并入该块（result 侧不重复
///   产出）；
/// - 孤儿 `web_search_tool_result`（无配对 use）→ 单独成块
///   （`raw_use: None`、`id` 取 `tool_use_id`、`query` 空串）；
/// - 其他 raw server 块（非 `web_search` 的 `server_tool_use`、
///   `code_execution_tool_result` 等）→ 跳过（不产块、不报错）；
/// - `Text` 无 raw → [`AgentBlock::Text`]（citations 自
///   `additional_params["citations"]` 提取）；
/// - `Reasoning` / `ToolCall` / `Image` → 现状不变。
pub(crate) fn project_choice(choice: &[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>) {
    // 预索引源 1：`web_search` 的 `server_tool_use` id 集合（配对判别值）
    let mut use_ids: HashSet<&str> = HashSet::new();
    // 预索引源 2：tool_use_id → （choice 下标, result 原文）；同 id 取首个
    let mut result_by_use: HashMap<&str, (usize, &Value)> = HashMap::new();
    for (index, content) in choice.iter().enumerate() {
        let AssistantContent::Text(text) = content else {
            continue;
        };
        let Some(raw) = raw_content(text) else {
            continue;
        };
        if let Some(use_id) = web_search_use_id(raw) {
            use_ids.insert(use_id);
            continue;
        }
        if raw_type(raw) != Some(WEB_SEARCH_RESULT_TYPE) {
            continue;
        }
        let Some(use_id) = raw.get("tool_use_id").and_then(Value::as_str) else {
            continue;
        };
        result_by_use.entry(use_id).or_insert((index, raw));
    }

    let mut consumed: Vec<bool> = vec![false; choice.len()];
    // 「已消费」在预索引期落定（而非遍历到 use 时才标）——否则 choice 倒序
    // （result 先于 use）时 result 会先以孤儿成块、随后 use 再配出第二块。
    // 只预置**有配对 use** 的首个 result 下标：无 use 的 result 不预置，保留
    // 孤儿规则；同 id 的第二条 result 亦不预置，照常成块。
    for (use_id, (index, _)) in &result_by_use {
        if use_ids.contains(use_id) {
            consumed[*index] = true;
        }
    }

    let mut blocks: Vec<AgentBlock> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    for (index, content) in choice.iter().enumerate() {
        match content {
            AssistantContent::Text(text) => {
                let Some(raw) = raw_content(text) else {
                    // 普通文本：正文 + 引用原值
                    blocks.push(AgentBlock::Text {
                        text: text.text.clone(),
                        citations: citations_of(text),
                    });
                    continue;
                };
                match raw_type(raw) {
                    Some(SERVER_TOOL_USE_TYPE) => {
                        // 非 web_search 的服务端工具（未来块类型）跳过
                        if text_field(raw, "name").as_deref() != Some(WEB_SEARCH_TOOL_NAME) {
                            continue;
                        }
                        let Some(use_id) = text_field(raw, "id") else {
                            continue;
                        };
                        // 配对已由预索引落定（含 result 先于 use 的倒序形态），
                        // 此处只取值、不再改 `consumed`
                        let paired = result_by_use.get(use_id.as_str());
                        let raw_result = paired.map(|(_, result)| (*result).clone());
                        let (results, error) = match &raw_result {
                            Some(result) => result_payload(result),
                            None => (Vec::new(), None),
                        };
                        blocks.push(AgentBlock::WebSearch {
                            id: use_id,
                            query: query_of(raw),
                            results,
                            error,
                            raw_use: Some(raw.clone()),
                            raw_result,
                        });
                    }
                    Some(WEB_SEARCH_RESULT_TYPE) => {
                        if consumed[index] {
                            continue; // 已被 use 侧消费：载荷并入、不重复产块
                        }
                        let Some(use_id) = text_field(raw, "tool_use_id") else {
                            continue; // 无配对键的 wild 块：无 id 可承载，跳过
                        };
                        let (results, error) = result_payload(raw);
                        blocks.push(AgentBlock::WebSearch {
                            id: use_id,
                            query: String::new(),
                            results,
                            error,
                            raw_use: None,
                            raw_result: Some(raw.clone()),
                        });
                    }
                    // 其他 raw server 块（code_execution_tool_result 等）跳过
                    _ => {}
                }
            }
            // 0.43 起 reasoning 以 Sealed 承载（发行方才可读）：以自身发行方
            // 打开取可显示文本（text / summary 块按行拼接）
            AssistantContent::Reasoning(reasoning) => blocks.push(AgentBlock::Thinking {
                thinking: reasoning
                    .open(reasoning.issuer())
                    .map(|value| value.display_text())
                    .unwrap_or_default(),
            }),
            AssistantContent::ToolCall(tool_call) => {
                blocks.push(AgentBlock::ToolUse {
                    id: tool_call.id.to_string(),
                    name: tool_call.function.name.as_str().to_owned(),
                    input: tool_call.function.arguments.clone(),
                });
                tool_calls.push(tool_call.clone());
            }
            AssistantContent::Image(_) => {} // sdk 引擎不产图像内容（工具面无边）
        }
    }
    (blocks, tool_calls)
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

/// 预索引用：raw 为 `web_search` 的 `server_tool_use` 时取其 `id`
/// （`None` = 不进配对索引集合——非 `server_tool_use`、非 `web_search` 或缺
/// `id`）。
fn web_search_use_id(raw: &Value) -> Option<&str> {
    if raw_type(raw) != Some(SERVER_TOOL_USE_TYPE) {
        return None;
    }
    if raw.get("name").and_then(Value::as_str) != Some(WEB_SEARCH_TOOL_NAME) {
        return None;
    }
    raw.get("id").and_then(Value::as_str)
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
