//! stdout JSONL 逐行归一化：单行 → [`AgentEventKind`]（不盖 seq/时间戳，
//! 由泵任务经 `AgentEvent::stamp` 统一补）。
//!
//! 归一化口径（能力 spec `specs/desktop-agent-execution/spec.md`，路径相对域根）：
//! - 空白行：跳过（返回 `None`，不占 seq）
//! - `type=system` + `subtype=init` → `RunStarted`（缺失字段置 null / 空表）
//! - `type=system` 其余 subtype → `SystemNotice{subtype, payload=原 JSON}`
//!   （subtype 不做封闭枚举，开放演进不炸解析）
//! - `type=assistant | user` → `Message{role=type, blocks, parent_tool_use_id}`
//! - `type=result` → `RunResult`（`total_cost_usd → cost_usd` 等字段对齐，缺失置 null）
//! - 其余合法 JSON / 未知 `type` → `Raw{eventType=type, rawJson=原文}`
//! - 非 JSON 行 → `Raw{eventType="unparsable", rawJson=原文}`——永不丢事件、
//!   永不炸解析

use agent::{AgentBlock, AgentEventKind};
use serde_json::Value;

/// 单行归一化；空白行（纯空白）返回 `None`。
pub fn normalize_line(line: &str) -> Option<AgentEventKind> {
    if line.trim().is_empty() {
        return None;
    }
    let parsed: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => return Some(unparsable(line)),
    };
    let event_type = parsed.get("type").and_then(Value::as_str);
    match event_type {
        Some("system") => Some(system_event(&parsed)),
        Some(role @ ("assistant" | "user")) => Some(message_event(role, &parsed)),
        Some("result") => Some(result_event(&parsed)),
        Some(event_type) => Some(raw(event_type, line)),
        None => Some(raw("unknown", line)),
    }
}

/// `Raw` 兜底：未知 `type` / 非 JSON 行原文透传。
fn raw(event_type: &str, line: &str) -> AgentEventKind {
    AgentEventKind::Raw {
        event_type: event_type.to_owned(),
        raw_json: line.to_owned(),
    }
}

/// 非 JSON 行兜底变体。
fn unparsable(line: &str) -> AgentEventKind {
    AgentEventKind::Raw {
        event_type: "unparsable".to_owned(),
        raw_json: line.to_owned(),
    }
}

/// `type=system`：`subtype=init` → RunStarted，其余 → SystemNotice（payload
/// 存原 JSON，不封闭枚举）。
fn system_event(parsed: &Value) -> AgentEventKind {
    let subtype = parsed
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if subtype == "init" {
        let tools = string_list(parsed.get("tools"));
        let mcp_servers = string_list(parsed.get("mcp_servers"));
        return AgentEventKind::RunStarted {
            model: optional_string(parsed.get("model")),
            session_id: optional_string(parsed.get("session_id")),
            tools,
            mcp_servers,
        };
    }
    AgentEventKind::SystemNotice {
        subtype: subtype.to_owned(),
        payload: parsed.clone(),
    }
}

/// `type=assistant | user` → Message：blocks 由 `message.content` 数组映射；
/// content 为字符串时降级为单个 Text 块（不丢正文）。
fn message_event(role: &str, parsed: &Value) -> AgentEventKind {
    let content = parsed.get("message").and_then(|m| m.get("content"));
    let blocks = match content {
        Some(Value::Array(items)) => items.iter().filter_map(block).collect(),
        Some(Value::String(text)) => vec![AgentBlock::Text { text: text.clone() }],
        _ => Vec::new(),
    };
    AgentEventKind::Message {
        role: role.to_owned(),
        blocks,
        parent_tool_use_id: optional_string(parsed.get("parent_tool_use_id")),
    }
}

/// 块映射：text→Text、thinking→Thinking、tool_use→ToolUse、tool_result→
/// ToolResult（content 扁平化为字符串、is_error 缺失 false）；其余未知块
/// 类型跳过（块级未知不堵信封——事件级由 Raw 兜底）。
fn block(value: &Value) -> Option<AgentBlock> {
    match value.get("type").and_then(Value::as_str) {
        Some("text") => Some(AgentBlock::Text {
            text: value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        }),
        Some("thinking") => Some(AgentBlock::Thinking {
            thinking: value
                .get("thinking")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        }),
        Some("tool_use") => Some(AgentBlock::ToolUse {
            id: value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            input: value.get("input").cloned().unwrap_or(Value::Null),
        }),
        Some("tool_result") => Some(AgentBlock::ToolResult {
            id: value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            content: flatten_content(value.get("content")),
            is_error: value
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }),
        _ => None,
    }
}

/// ToolResult.content 扁平化：字符串原样；数组逐元素取文本后按行拼接
/// （元素为对象时取其 `text` 字段，否则 JSON 序列化兜底）；其余形态 JSON
/// 序列化；缺失空串。
fn flatten_content(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::String(text) => text.clone(),
                other => other
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| other.to_string()),
            })
            .collect::<Vec<String>>()
            .join("\n"),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// `type=result` → RunResult：`total_cost_usd` → `cost_usd` 等字段对齐，
/// 缺失置 null / false。
fn result_event(parsed: &Value) -> AgentEventKind {
    AgentEventKind::RunResult {
        subtype: parsed
            .get("subtype")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        is_error: parsed
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        num_turns: parsed.get("num_turns").and_then(Value::as_u64),
        duration_ms: parsed.get("duration_ms").and_then(Value::as_u64),
        cost_usd: parsed.get("total_cost_usd").and_then(Value::as_f64),
        usage: parsed.get("usage").cloned().unwrap_or(Value::Null),
        session_id: optional_string(parsed.get("session_id")),
    }
}

/// 可空字符串字段提取：缺失或非字符串 → None。
fn optional_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

/// 字符串数组字段提取：缺失或形态不符 → 空表（保留可归一化的元素）。
fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}
