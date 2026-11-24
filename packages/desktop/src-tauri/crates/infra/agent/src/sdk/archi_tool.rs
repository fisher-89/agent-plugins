//! 架构只读工具执行体：`archi_module`（定位：两形态——指定元素 / 省略元素薄概览）、
//! `archi_search`（发现：三 scope）、`archi_api`（契约面自检：`check` 两轴 /
//! `bootstrap` 草稿）与 `archi_drift`（漂移判据读数：恒全量、零跨调用缓存）。
//! 无路径参数，执行上下文由组合根 per-turn 铸造（轮 root = 会话 cwd；change 会话即
//! worktree），经 `LoopTurn` 携带。
//!
//! 响应经 `archi::dto` / `archi::api` 单点投影（三方一致性的机械前提——MUST NOT 在此
//! 另拼一份 JSON）；坏模型 / 参数缺失一律 `Err`（loop 侧 is_error 回灌，不 panic），
//! 命中既有 L1 单结果上限收口（禁第二份上限实现）。
//!
//! 四支（含 `archi_scaffold`）同样**零写盘**，`cap_result` 与另三支共用同一上限单点。

use std::collections::BTreeMap;
use std::path::PathBuf;

use archi::dto::SearchScope;
use serde_json::Value;

use crate::sdk::archi_drift_view::{drift_entries, drift_files, drift_summary, DRIFT_SCOPES};
use crate::sdk::tools;

/// 架构工具执行上下文（per-turn 铸造：轮 root）。
pub(crate) struct ArchiContext {
    root: PathBuf,
}

impl ArchiContext {
    /// 上下文构造（泵起播按名字清单含 archi 工具名铸造；`root` = 会话 cwd）。
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// 执行体：`archi_module`（有 / 无 `element` 两形态）、`archi_search`（`query` +
    /// `scope`）、`archi_api`（`action` + 可选 `element`）；未知名 `Err`。输出紧凑 JSON
    /// 后过 L1 收口。
    pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        let payload = match name {
            tools::ARCHI_MODULE_TOOL => self.module_payload(input)?,
            tools::ARCHI_SEARCH_TOOL => self.search_payload(input)?,
            tools::ARCHI_API_TOOL => self.api_payload(input)?,
            tools::ARCHI_DRIFT_TOOL => self.drift_payload(input)?,
            tools::ARCHI_SCAFFOLD_TOOL => self.scaffold_payload(input)?,
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

    /// `archi_drift`：恒全量判定（`mode = Full` + 空变更集 + 无 change 上下文 +
    /// `CancelFlag::never()`），`scope` 三值逐级追加；不命中即 `Err` 点名合法值
    /// （MUST NOT 静默退化成缺省）。零跨调用缓存：每次调用都重新判定并各自计时。
    fn drift_payload(&self, input: &Value) -> Result<Value, String> {
        let scope = match input.get("scope").and_then(Value::as_str) {
            Some("summary") | None => "summary",
            Some("entries") => "entries",
            Some("files") => "files",
            Some(other) => {
                return Err(format!(
                    "非法 scope: {other}（合法值：{}）",
                    DRIFT_SCOPES.join(" / ")
                ))
            }
        };
        let started = std::time::Instant::now();
        let report = archi::drift::check(archi::drift::CheckInput {
            root: &self.root,
            mode: archi::drift::ArchiDriftMode::Full,
            changed: &[],
            change: None,
            cancel: foundation::cancel::CancelFlag::never(),
        })
        .map_err(|error| error.message())?;
        let duration_ms = started.elapsed().as_millis() as u64;
        Ok(match scope {
            "entries" => drift_entries(&report, duration_ms),
            "files" => drift_files(&report, duration_ms),
            _ => drift_summary(&report, duration_ms),
        })
    }

    /// `archi_scaffold`：`kind` 可选（缺省 `domain`；值集外 `Err` 点名四值）→ 新元素
    /// TOML 草稿。只读零写盘——写盘仍走既有 `write` / `edit`。
    fn scaffold_payload(&self, input: &Value) -> Result<Value, String> {
        let kind = match input.get("kind").and_then(Value::as_str).map(str::trim) {
            Some(raw) if !raw.is_empty() => archi::dto::ElementKind::ALL
                .iter()
                .copied()
                .find(|kind| kind.as_str() == raw)
                .ok_or_else(|| {
                    format!(
                        "非法 kind: {raw}（合法值：{}）",
                        archi::dto::ElementKind::ALL
                            .map(archi::dto::ElementKind::as_str)
                            .join(" / ")
                    )
                })?,
            _ => archi::dto::ElementKind::Domain,
        };
        Ok(serde_json::json!({ "draft": scaffold_draft(kind) }))
    }

    /// `archi_api`：`action` 封闭二值（未知值 `Err` 并点名合法值）。
    ///
    /// - `check`：元素表（`graph_view`，结构诊断 → `is_error`）→ 三类差异
    ///   （`api::artifact_check`）→ 契约判据（`api::findings`，与漂移判定同源）；
    ///   响应 = `{ artifacts, findings: { total, byCategory, items } }`（摘要优先，
    ///   `cap_result` 截断时计数仍在）。
    /// - `bootstrap`：`element` 必填，出契约 md 草稿（零写盘）。
    fn api_payload(&self, input: &Value) -> Result<Value, String> {
        let action = input
            .get("action")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|action| !action.is_empty())
            .ok_or_else(|| {
                "入参缺失或非字符串字段: action（合法值：check / bootstrap）".to_owned()
            })?;
        match action {
            "check" => {
                let graph = archi::dto::graph_view(&self.root)
                    .map_err(|error| format!("模型读取失败：{error}"))?;
                if !graph.diagnostics.is_empty() {
                    return Err(archi::dto::graph_diagnostics_error(&graph.diagnostics));
                }
                let artifacts =
                    archi::api::artifact_check(&self.root).map_err(|error| error.message())?;
                let items = archi::api::findings(&self.root, &graph.elements)
                    .map_err(|error| error.message())?;
                let mut by_category: BTreeMap<String, usize> = BTreeMap::new();
                for finding in &items {
                    *by_category.entry(finding.category.clone()).or_insert(0) += 1;
                }
                Ok(serde_json::json!({
                    "artifacts": artifacts,
                    "findings": {
                        "total": items.len(),
                        "byCategory": by_category,
                        "items": items,
                    },
                }))
            }
            "bootstrap" => {
                let element = input
                    .get("element")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|element| !element.is_empty())
                    .ok_or_else(|| {
                        "入参缺失或非字符串字段: element（bootstrap 必填）".to_owned()
                    })?;
                let draft =
                    archi::api::bootstrap(&self.root, element).map_err(|error| error.message())?;
                Ok(serde_json::json!({ "draft": draft }))
            }
            other => Err(format!("未知 action: {other}（合法值：check / bootstrap）")),
        }
    }
}

/// 投影结果 → JSON 值（序列化失败即 `Err`）。
fn to_value<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("架构工具结果序列化失败: {error}"))
}

/// 新元素 TOML 草稿（`__KIND__` 由调用方替换为实际 kind）。
///
/// 形状受两条硬约束：元素部分只留一行实值（`kind`），其余字段**全部注释行**；关系
/// 部分同样全注释——空的 `[[relation]]` 表会被解析成缺必填字段的关系，与「草稿过
/// `TomlParser` 零诊断」冲突。FQN 用**单段**占位：多段 FQN 而祖先未声明会触发
/// `orphanHierarchy`。
const SCAFFOLD_TEMPLATE: &str = r#"# 新元素模板（只读草稿：写盘仍用既有 write / edit）
# 单段占位 FQN：改成你的完整 FQN；多段 FQN（如 App.Auth）必须先声明祖先元素
[element."NewElement"]

kind = "__KIND__"
# kind：package = 包（至多一条包根 path，纯容器可留空）/ domain = 领域容器（path 选填）/ module = crate、目录型模块、视图或单文件组件（path 必填、可多条）/ composite = 纯分组（无层级，可嵌套任意类型之上；纯分层容器是标准形态）

# gate：缺省 off（ratchet 逐块收紧），值 off / warn / strict
# gate = "off"

# api：缺省 declared（承诺有契约）；显式无 API 写 none
# api = "none"

# entry：只在约定默认指不到时写（repo 相对 POSIX；单文件根 = 根文件自身 / 目录型根 = index.ts(x) / crate = src/lib.rs）
# entry = "./src-tauri/crates/core/example/src/lib.rs"

# path：module 必填（可多条 = 若干根的清单）；package 至多一条包根（纯容器可留空；≥2 条即模型停摆）；domain / composite 选填；`./` 前缀、repo 相对；目录以 `/` 结尾、文件精确到扩展名
# path = ["./src-tauri/crates/core/example/src/"]
# path = ["./src-tauri/crates/core/example/src/lib.rs"]

# doc：单值指针，只有 api = none 的元素可写（declared 写了即 apiUnexpected）
# doc = "./openspec/specs/example/spec.md"

# summary：一行摘要
# summary = "一句话说明本元素负责什么"

# metadata：子表与内联表等价（[element."X".metadata] 写在已声明元素之后零诊断；成为唯一声明才缺 kind）
# metadata = { owner = "team-x", layer = "core" }

# 关系模板
# [[relation]]
# from = "App.NewElement"
# to = "App.Dependency"
# desc = "为什么依赖"
# apis = ["ExportedName"]
# apis：选填名字清单，只写名字、可留空、生成物面留空；差额看 archi_drift
"#;

/// 按 kind 出草稿（占位替换单点；草稿含注释行与一行实值 `kind`）。
fn scaffold_draft(kind: archi::dto::ElementKind) -> String {
    SCAFFOLD_TEMPLATE.replace("__KIND__", kind.as_str())
}
