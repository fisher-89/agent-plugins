//! 架构只读工具执行体：`archi_module`（定位：两形态——指定元素 / 省略元素薄概览）
//! 与 `archi_search`（发现：三 scope）。无路径参数，执行上下文由组合根 per-turn
//! 铸造（轮 root = 会话 cwd；change 会话即 worktree），经 `LoopTurn` 携带。
//!
//! 响应经 `archi::dto` 单点投影（三方一致性的机械前提——MUST NOT 在此另拼一份
//! JSON）；坏模型 / 参数缺失一律 `Err`（loop 侧 is_error 回灌，不 panic），命中
//! 既有 L1 单结果上限收口（禁第二份上限实现）。

use std::path::PathBuf;

use archi::dto::SearchScope;
use serde_json::Value;

use crate::sdk::tools;

/// 架构工具执行上下文（per-turn 铸造：轮 root）。
pub(crate) struct ArchiContext {
    root: PathBuf,
}

impl ArchiContext {
    /// 上下文构造（泵起播按 `toolset == Archi { .. }` 铸造；`root` = 会话 cwd）。
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// 执行体：`archi_module`（有 / 无 `element` 两形态）、`archi_search`（`query`
    /// + `scope`）；未知名 `Err`。输出紧凑 JSON 后过 L1 收口。
    pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        let payload = match name {
            tools::ARCHI_MODULE_TOOL => self.module_payload(input)?,
            tools::ARCHI_SEARCH_TOOL => self.search_payload(input)?,
            other => return Err(format!("未知架构工具: {other}")),
        };
        let text = serde_json::to_string(&payload)
            .map_err(|error| format!("架构工具结果序列化失败: {error}"))?;
        Ok(tools::cap_result(text))
    }

    /// `archi_module`：指定元素 → 单元素详情；省略 / 空白 → 全元素薄概览。
    fn module_payload(&self, input: &Value) -> Result<Value, String> {
        let element = input
            .get("element")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|element| !element.is_empty());
        match element {
            Some(element) => archi::dto::module_detail(&self.root, element).and_then(to_value),
            None => archi::dto::module_overview(&self.root).and_then(to_value),
        }
    }

    /// `archi_search`：`query` 必填（空白即 `Err`）、`scope` 缺省 `all`。
    fn search_payload(&self, input: &Value) -> Result<Value, String> {
        let query = input
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| "入参缺失或非字符串字段: query".to_owned())?;
        let scope = match input.get("scope").and_then(Value::as_str) {
            Some(raw) => SearchScope::parse(raw)?,
            None => SearchScope::All,
        };
        archi::dto::search(&self.root, query, scope).and_then(to_value)
    }
}

/// 投影结果 → JSON 值（序列化失败即 `Err`）。
fn to_value<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("架构工具结果序列化失败: {error}"))
}
