//! 架构工具执行体：五只读 + 两写。
//!
//! 只读面：`archi_module`（定位：两形态——指定元素 / 省略元素薄概览）、`archi_search`（发现：
//! 三 scope）、`archi_api`（接口面三动作：`check` 两轴 / `bootstrap` 双草稿 / `link` 算
//! `$ref`）、`archi_drift`（漂移判据读数）与 `archi_scaffold`（字段词典草稿）。
//!
//! 写面：`archi_element`（元素 upsert / delete，可内联出边）与 `archi_relation`（单条关系
//! upsert / delete）。落盘链固定为「E1 入参解析与硬闸 → E2 结构诊断前置 → `plan_*`
//! （零写盘）→ 目标文件过 `ProtectRules` → `write_atomic` 落盘 → 三通道响应装配」，
//! 任一前置失败即零落盘。写面 MUST NOT 自拼 TOML、MUST NOT 重实现任一判据（`path` 形态 /
//! `kind × path` 基数 / `apis` 值面全归 parser 与 `archi::edit` 的自证链）。
//!
//! 七支共用同一 per-turn 上下文（组合根铸造，轮 root = 会话 cwd；change 会话即 worktree），
//! 经 `LoopTurn` 携带。响应经 `archi::dto` / `archi::edit` 单点投影（MUST NOT 在此另拼一份
//! JSON）；坏模型 / 参数缺失一律 `Err`（loop 侧 is_error 回灌，不 panic），命中既有 L1
//! 单结果上限收口（禁第二份上限实现）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use archi::dto::{ApiStatus, ElementKind, GateTier, SearchScope};
use archi::edit::{ElementSpec, RelationSpec, WriteAction, WriteOutcome};
use serde_json::Value;

use crate::sdk::archi_drift_view::{drift_entries, drift_files, drift_summary, DRIFT_SCOPES};
use crate::sdk::mount::WRITE_LIMITS;
use crate::sdk::protect::ProtectRules;
use crate::sdk::{atomic, tools};

/// 架构工具执行上下文（per-turn 铸造：轮 root + 写入保护规则）。
pub(crate) struct ArchiContext {
    root: PathBuf,
    /// 写入保护规则（写臂在落点解析后判定；读臂零消费）
    protect: Option<ProtectRules>,
}

impl ArchiContext {
    /// 上下文构造（泵起播按名字清单含 archi 工具名铸造；`root` = 会话 cwd）。
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            protect: None,
        }
    }

    /// 接线写入保护规则（装配侧单点：`session_pump` 在 `ProtectRules` 装配之后调用）。
    pub(crate) fn with_protect(self, rules: ProtectRules) -> Self {
        Self {
            root: self.root,
            protect: Some(rules),
        }
    }

    /// 执行体：`archi_module`（有 / 无 `element` 两形态）、`archi_search`（`query` +
    /// `scope`）、`archi_api`（`action` + `element` / `from` / `to` / `name`）、`archi_drift`、
    /// `archi_scaffold`、
    /// `archi_element`、`archi_relation`；未知名 `Err`。输出紧凑 JSON 后过 L1 收口。
    pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        let payload = match name {
            tools::ARCHI_MODULE_TOOL => self.module_payload(input)?,
            tools::ARCHI_SEARCH_TOOL => self.search_payload(input)?,
            tools::ARCHI_API_TOOL => self.api_payload(input)?,
            tools::ARCHI_DRIFT_TOOL => self.drift_payload(input)?,
            tools::ARCHI_SCAFFOLD_TOOL => self.scaffold_payload(input)?,
            tools::ARCHI_ELEMENT_TOOL => self.element_payload(input)?,
            tools::ARCHI_RELATION_TOOL => self.relation_payload(input)?,
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
    /// TOML 草稿。落盘走 `archi_element` / `archi_relation`（工具表达不了的才手改
    /// `write` / `edit`），本工具只出模板、不触盘。
    fn scaffold_payload(&self, input: &Value) -> Result<Value, String> {
        let kind = match input.get("kind").and_then(Value::as_str).map(str::trim) {
            Some(raw) if !raw.is_empty() => {
                domain_value(raw, "kind", &ElementKind::ALL, ElementKind::as_str)?
            }
            _ => ElementKind::Domain,
        };
        Ok(serde_json::json!({ "draft": scaffold_draft(kind) }))
    }

    /// `archi_api`：`action` 封闭三值（未知值 `Err` 并点名三个合法值）。
    ///
    /// - `check`：元素表（`graph_view`，结构诊断 → `is_error`）→ 三类差异
    ///   （`api::artifact_check`）→ 接口判据（`api::findings`，与漂移判定同源）；
    ///   响应 = `{ artifacts, findings: { total, byCategory, items } }`（摘要优先，
    ///   `cap_result` 截断时计数仍在）。
    /// - `bootstrap`：`element` 必填，出**双草稿** `{ schema, doc }`（schema 骨架可直接
    ///   落盘并过写闸；doc 只有 H1 与一句指引），零写盘。
    /// - `link`：算跨元素 `$ref` 串（`from` + `to` + `name`）或本文件 `$ref` 串
    ///   （`element` + `name`），返回 `{ ref }`；**纯计算零读盘零写盘**——手拼 `../` 层数
    ///   少一层会绕过 `apiRefMissing`、到编译期才炸。
    fn api_payload(&self, input: &Value) -> Result<Value, String> {
        let action = input
            .get("action")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|action| !action.is_empty())
            .ok_or_else(|| {
                "入参缺失或非字符串字段: action（合法值：check / bootstrap / link）".to_owned()
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
                let drafts =
                    archi::api::bootstrap(&self.root, element).map_err(|error| error.message())?;
                Ok(serde_json::json!({
                    "schema": drafts.schema,
                    "doc": drafts.doc,
                }))
            }
            "link" => self.link_payload(input),
            other => Err(format!(
                "未知 action: {other}（合法值：check / bootstrap / link）"
            )),
        }
    }

    /// `link` 动作：算一条 `$ref` 串（两形态互斥，纯计算零 IO）。
    ///
    /// - 跨元素：`{ from, to, name }` → `<ref_relative(from, to)><to 叶段>.schema.json#/$defs/<name>`
    /// - 本文件：`{ element, name }` → `#/$defs/<name>`
    ///
    /// 前缀取 [`archi::api::paths::ref_relative`]（MUST NOT 手算层数），文件名取
    /// [`archi::api::paths::schema_path`] 的末段（MUST NOT 行内拼 `.schema.json`），
    /// `from` / `to` / `element` 过 FQN 形态闸、`name` 过标识符闸——任一处不合法即 `Err`
    /// 并点名。
    fn link_payload(&self, input: &Value) -> Result<Value, String> {
        let name = input
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                "入参缺失或非字符串字段: name（link 必填：导出名 / 类型名）".to_owned()
            })?;
        if !archi::is_identifier(name) {
            return Err(format!(
                "name `{name}` 不是 ASCII 标识符（`[A-Za-z_][A-Za-z0-9_]*`）——`$ref` 的末段是类型名，MUST NOT 含空格 / 连字符 / 点"
            ));
        }
        let (from, to, element) = (
            field(input, "from"),
            field(input, "to"),
            field(input, "element"),
        );
        if from.is_some() || to.is_some() {
            let (Some(from), Some(to)) = (from, to) else {
                return Err(
                    "from / to 必须同时给出（跨元素形态）——只给一个无法定位目标文件".to_owned(),
                );
            };
            if element.is_some() {
                return Err(
                    "`from` / `to` 与 `element` 是互斥两形态：跨元素用 from + to，本文件用 element"
                        .to_owned(),
                );
            }
            let (from, to) = (checked_fqn(&from, "from")?, checked_fqn(&to, "to")?);
            let target = archi::api::paths::schema_path(&self.root, &to)
                .and_then(|path| {
                    path.file_name()
                        .and_then(|file| file.to_str())
                        .map(str::to_owned)
                })
                .ok_or_else(|| format!("目标元素 FQN `{to}` 形态非法，派生不出接口文件名"))?;
            return Ok(serde_json::json!({
                "ref": format!(
                    "{}{target}#/$defs/{name}",
                    archi::api::paths::ref_relative(&from, &to)
                ),
            }));
        }
        let element = element.ok_or_else(|| {
            "入参缺失: 需要 from + to（跨元素）或 element（本文件）其一".to_owned()
        })?;
        checked_fqn(&element, "element")?;
        Ok(serde_json::json!({ "ref": format!("#/$defs/{name}") }))
    }
    /// `archi_element`：E1 入参解析与硬闸 → E2 诊断前置 → `plan_*`（零写盘）→ 保护闸 →
    /// 原子落盘 → 三通道响应装配。
    fn element_payload(&self, input: &Value) -> Result<Value, String> {
        let parsed = ElementWriteInput::parse(input)?;
        self.ensure_model_clean()?;
        let outcome = match parsed.action {
            WriteAction::Delete => archi::edit::plan_element_delete(
                &self.root,
                &parsed.element,
                parsed.cascade,
                parsed.expect.as_deref(),
            ),
            WriteAction::Upsert => archi::edit::plan_element_upsert(&self.root, &parsed.spec()),
        }
        .map_err(|error| error.message())?;
        self.commit(tools::ARCHI_ELEMENT_TOOL, &outcome)?;
        Ok(self.element_response(&parsed, &outcome))
    }

    /// `archi_relation`：同一落盘链；响应是薄形（一次写只动一条边，不付全量重解析）。
    fn relation_payload(&self, input: &Value) -> Result<Value, String> {
        let parsed = RelationWriteInput::parse(input)?;
        self.ensure_model_clean()?;
        let outcome = match parsed.action {
            WriteAction::Delete => archi::edit::plan_relation_delete(
                &self.root,
                &parsed.from,
                &parsed.to,
                parsed.expect.as_deref(),
            ),
            WriteAction::Upsert => archi::edit::plan_relation_upsert(
                &self.root,
                &parsed.from,
                &parsed.to,
                parsed.desc.as_ref().map(|change| change.as_deref()),
                parsed.apis.as_deref(),
                parsed.expect.as_deref(),
            ),
        }
        .map_err(|error| error.message())?;
        self.commit(tools::ARCHI_RELATION_TOOL, &outcome)?;
        Ok(serde_json::json!({
            "warnings": &outcome.warnings,
            "action": parsed.action.as_str(),
            "from": &parsed.from,
            "to": &parsed.to,
            "file": archi::display(&self.root, &outcome.write.file),
            "hash": &outcome.write.hash,
            "created": outcome.created,
            "changed": &outcome.changed,
            "reordered": &outcome.reordered,
        }))
    }

    /// E2 前置读数：模型已有结构诊断即 `Err`（`dto::graph_diagnostics_error` 的全量形
    /// ——诊断渲染是四个消费面共用的单点，MUST NOT 为写工具另写第二份）。
    fn ensure_model_clean(&self) -> Result<(), String> {
        let graph =
            archi::dto::graph_view(&self.root).map_err(|error| format!("模型读取失败：{error}"))?;
        if graph.diagnostics.is_empty() {
            return Ok(());
        }
        Err(archi::dto::graph_diagnostics_error(&graph.diagnostics))
    }

    /// 保护闸 + 落盘：目标文件只有 `plan_*` 之后才知道，故在执行体内判定（同一
    /// `ProtectRules`、同一拒串族）；`protect` 缺席即 `Err`（生产不可达——装配侧恒接线）。
    fn commit(&self, tool: &str, outcome: &WriteOutcome) -> Result<(), String> {
        let rules = self.protect.as_ref().ok_or_else(|| {
            format!("写入保护规则未装配（{tool} 仅在装配侧接线后可用；生产不可达）")
        })?;
        if let Some(denial) = rules.evaluate_target(tool, &outcome.write.file) {
            return Err(denial);
        }
        atomic::write_atomic(&outcome.write.file, &outcome.write.content)
    }

    /// 元素写面的响应（三通道：`warnings` / `changed` / 计数；`file` 为 repo 相对 POSIX）。
    ///
    /// `reordered` 照带（规范化已在 `apply_*` 内完成，零额外解析、零计数重算）。
    fn element_response(&self, parsed: &ElementWriteInput, outcome: &WriteOutcome) -> Value {
        serde_json::json!({
            "warnings": &outcome.warnings,
            "action": parsed.action.as_str(),
            "element": &parsed.element,
            "file": archi::display(&self.root, &outcome.write.file),
            "hash": &outcome.write.hash,
            "created": outcome.created,
            "changed": &outcome.changed,
            "relations": outcome.relations,
            "counts": outcome.counts,
            "model": outcome.model,
            "reordered": &outcome.reordered,
        })
    }
}

/// `archi_element` 的 E1 解析产物（私有）：动作 + 字段面三态 + 内联出边 + 落点 + 乐观锁。
struct ElementWriteInput {
    action: WriteAction,
    element: String,
    kind: Option<ElementKind>,
    gate: Option<GateTier>,
    api: Option<ApiStatus>,
    entry: Option<Option<String>>,
    path: Option<Vec<String>>,
    doc: Option<Option<String>>,
    summary: Option<Option<String>>,
    metadata: Option<BTreeMap<String, String>>,
    relations: Vec<RelationSpec>,
    expect: Option<String>,
    file: Option<String>,
    create: bool,
    cascade: bool,
}

impl ElementWriteInput {
    /// E1：值域 / 上限 / 必填逐项检查，**一次报全**（`；` 分隔；值域违例点名合法值、上限
    /// 违例点名字段与上限）。不做语义判断——批量四条、祖先、端点、落点全在 `plan_*`。
    fn parse(input: &Value) -> Result<Self, String> {
        let Some(object) = input.as_object() else {
            return Err("入参必须是 JSON 对象（字段面见本工具 parameters）".to_owned());
        };
        let mut problems: Vec<String> = Vec::new();
        let action = action_of(object.get("action"), "action", &mut problems);
        let element = required_text(object.get("element"), "element", &mut problems)
            .map(|text| bounded("element", text, WRITE_LIMITS.fqn, &mut problems));
        let kind = optional_domain(
            object.get("kind"),
            "kind",
            &ElementKind::ALL,
            ElementKind::as_str,
            &mut problems,
        );
        let gate = optional_domain(
            object.get("gate"),
            "gate",
            &GateTier::ALL,
            GateTier::as_str,
            &mut problems,
        );
        let api = optional_domain(
            object.get("api"),
            "api",
            &ApiStatus::ALL,
            ApiStatus::as_str,
            &mut problems,
        );
        let entry = optional_nullable_text(
            object.get("entry"),
            "entry",
            WRITE_LIMITS.pointer,
            &mut problems,
        );
        let path = optional_text_list(
            object.get("path"),
            "path",
            WRITE_LIMITS.path_item,
            WRITE_LIMITS.path_items,
            &mut problems,
        );
        let doc = optional_nullable_text(
            object.get("doc"),
            "doc",
            WRITE_LIMITS.pointer,
            &mut problems,
        );
        let summary = optional_nullable_text(
            object.get("summary"),
            "summary",
            WRITE_LIMITS.text,
            &mut problems,
        );
        let metadata = optional_metadata(object.get("metadata"), &mut problems);
        let relations = parse_relations(object.get("relations"), &mut problems);
        let expect = expect_of(object.get("expect"), &mut problems);
        let file = optional_text(object.get("file"), "file", WRITE_LIMITS.file, &mut problems);
        let create = flag(object.get("create"), "create", &mut problems);
        let cascade = flag(object.get("cascade"), "cascade", &mut problems);
        if cascade && action != Some(WriteAction::Delete) {
            problems.push(
                "cascade 仅 action = delete（action = upsert 时不得给 cascade = true）".to_owned(),
            );
        }
        if !problems.is_empty() {
            return Err(problems.join("；"));
        }
        Ok(ElementWriteInput {
            action: action.unwrap_or_default(),
            element: element.unwrap_or_default(),
            kind,
            gate,
            api,
            entry,
            path,
            doc,
            summary,
            metadata,
            relations,
            expect,
            file,
            create,
            cascade,
        })
    }

    /// 解析产物 → 引擎规格（零改写：字段面逐位搬运）。
    fn spec(&self) -> ElementSpec {
        ElementSpec {
            fqn: self.element.clone(),
            kind: self.kind,
            gate: self.gate,
            api: self.api,
            entry: self.entry.clone(),
            path: self.path.clone(),
            doc: self.doc.clone(),
            summary: self.summary.clone(),
            metadata: self.metadata.clone(),
            relations: self.relations.clone(),
            file: self.file.clone(),
            create: self.create,
            expect: self.expect.clone(),
        }
    }
}

/// `archi_relation` 的 E1 解析产物（私有）：动作 + 两端点 + 三态字段 + 乐观锁。
struct RelationWriteInput {
    action: WriteAction,
    from: String,
    to: String,
    desc: Option<Option<String>>,
    apis: Option<Vec<String>>,
    expect: Option<String>,
}

impl RelationWriteInput {
    /// E1：同一「一次报全」纪律（见 [`ElementWriteInput::parse`]）。
    fn parse(input: &Value) -> Result<Self, String> {
        let Some(object) = input.as_object() else {
            return Err("入参必须是 JSON 对象（字段面见本工具 parameters）".to_owned());
        };
        let mut problems: Vec<String> = Vec::new();
        let action = action_of(object.get("action"), "action", &mut problems);
        let from = required_text(object.get("from"), "from", &mut problems)
            .map(|text| bounded("from", text, WRITE_LIMITS.fqn, &mut problems));
        let to = required_text(object.get("to"), "to", &mut problems)
            .map(|text| bounded("to", text, WRITE_LIMITS.fqn, &mut problems));
        let desc =
            optional_nullable_text(object.get("desc"), "desc", WRITE_LIMITS.text, &mut problems);
        let apis = optional_text_list(
            object.get("apis"),
            "apis",
            WRITE_LIMITS.relation_api,
            WRITE_LIMITS.relation_apis,
            &mut problems,
        );
        let expect = expect_of(object.get("expect"), &mut problems);
        if !problems.is_empty() {
            return Err(problems.join("；"));
        }
        Ok(RelationWriteInput {
            action: action.unwrap_or_default(),
            from: from.unwrap_or_default(),
            to: to.unwrap_or_default(),
            desc,
            apis,
            expect,
        })
    }
}

/// 词表拼接（`/` 分隔；值域文案与 schema `enum` 同源，零手抄字面量）。
fn vocabulary(words: impl IntoIterator<Item = &'static str>) -> String {
    words.into_iter().collect::<Vec<&str>>().join(" / ")
}

/// 长度上限检查（超限记一条违例，返回值原样供后续装配）。
fn bounded(name: &str, text: String, limit: usize, problems: &mut Vec<String>) -> String {
    let length = text.chars().count();
    if length > limit {
        problems.push(format!("{name} 超上限（{length} > {limit} 字符）"));
    }
    text
}

/// 必填字符串（缺失 / 空白 / 非字符串各一条违例）。
fn required_text(value: Option<&Value>, name: &str, problems: &mut Vec<String>) -> Option<String> {
    match value {
        Some(Value::String(text)) if !text.trim().is_empty() => Some(text.clone()),
        Some(Value::String(_)) => {
            problems.push(format!("{name} 不得为空白"));
            None
        }
        Some(_) => {
            problems.push(format!("{name} 必须是字符串"));
            None
        }
        None => {
            problems.push(format!("{name} 必填"));
            None
        }
    }
}

/// 可选字符串（缺失 / `null` ⇒ `None`）。
fn optional_text(
    value: Option<&Value>,
    name: &str,
    limit: usize,
    problems: &mut Vec<String>,
) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(bounded(name, text.clone(), limit, problems)),
        Some(_) => {
            problems.push(format!("{name} 必须是字符串"));
            None
        }
    }
}

/// 可清空字符串的三态（缺失 = 不动、`null` = 清键、字符串 = 设值）。
fn optional_nullable_text(
    value: Option<&Value>,
    name: &str,
    limit: usize,
    problems: &mut Vec<String>,
) -> Option<Option<String>> {
    match value {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(text)) => Some(Some(bounded(name, text.clone(), limit, problems))),
        Some(_) => {
            problems.push(format!("{name} 必须是字符串或 null"));
            None
        }
    }
}

/// 可清空字符串清单（缺失 = 不动、`null` / `[]` = 清键）。
fn optional_text_list(
    value: Option<&Value>,
    name: &str,
    item_limit: usize,
    items_limit: usize,
    problems: &mut Vec<String>,
) -> Option<Vec<String>> {
    match value {
        None => None,
        Some(Value::Null) => Some(Vec::new()),
        Some(Value::Array(items)) => {
            if items.len() > items_limit {
                problems.push(format!(
                    "{name} 超上限（{} 项 > {items_limit}）",
                    items.len()
                ));
            }
            let mut values: Vec<String> = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                match item.as_str() {
                    Some(text) => values.push(bounded(
                        &format!("{name}[{index}]"),
                        text.to_owned(),
                        item_limit,
                        problems,
                    )),
                    None => problems.push(format!("{name}[{index}] 必须是字符串")),
                }
            }
            Some(values)
        }
        Some(_) => {
            problems.push(format!("{name} 必须是字符串数组或 null"));
            None
        }
    }
}

/// 可清空元数据表（缺失 = 不动、`null` / `{}` = 清键；值 MUST 是字符串）。
fn optional_metadata(
    value: Option<&Value>,
    problems: &mut Vec<String>,
) -> Option<BTreeMap<String, String>> {
    match value {
        None => None,
        Some(Value::Null) => Some(BTreeMap::new()),
        Some(Value::Object(entries)) => {
            if entries.len() > WRITE_LIMITS.metadata_items {
                problems.push(format!(
                    "metadata 超上限（{} 项 > {}）",
                    entries.len(),
                    WRITE_LIMITS.metadata_items
                ));
            }
            let mut values: BTreeMap<String, String> = BTreeMap::new();
            for (key, entry) in entries.iter() {
                if key.chars().count() > WRITE_LIMITS.metadata_key {
                    problems.push(format!(
                        "metadata 键 {key:?} 超上限（{} > {} 字符）",
                        key.chars().count(),
                        WRITE_LIMITS.metadata_key
                    ));
                }
                match entry.as_str() {
                    Some(text) => {
                        let length = text.chars().count();
                        if length > WRITE_LIMITS.metadata_value {
                            problems.push(format!(
                                "metadata[{key:?}] 超上限（{length} > {} 字符）",
                                WRITE_LIMITS.metadata_value
                            ));
                        }
                        values.insert(key.clone(), text.to_owned());
                    }
                    None => problems.push(format!("metadata[{key:?}] 的值必须是字符串")),
                }
            }
            Some(values)
        }
        Some(_) => {
            problems.push("metadata 必须是对象或 null".to_owned());
            None
        }
    }
}

/// 内联出边清单（缺失 / `null` = 出边不动；同批重复 `to` 与 delete 带字段的违例在
/// 引擎侧 `plan_*` 判——此处只管类型与上限）。
fn parse_relations(value: Option<&Value>, problems: &mut Vec<String>) -> Vec<RelationSpec> {
    let items = match value {
        None | Some(Value::Null) => return Vec::new(),
        Some(Value::Array(items)) => items,
        Some(_) => {
            problems.push("relations 必须是数组".to_owned());
            return Vec::new();
        }
    };
    if items.len() > WRITE_LIMITS.relations {
        problems.push(format!(
            "relations 超上限（{} 项 > {}）",
            items.len(),
            WRITE_LIMITS.relations
        ));
    }
    let mut relations: Vec<RelationSpec> = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let name = format!("relations[{}]", index + 1);
        let Some(entry) = item.as_object() else {
            problems.push(format!("{name} 必须是对象"));
            continue;
        };
        let Some(to) = required_text(entry.get("to"), &format!("{name}.to"), problems) else {
            continue;
        };
        let to = bounded(&format!("{name}.to"), to, WRITE_LIMITS.fqn, problems);
        let desc = optional_nullable_text(
            entry.get("desc"),
            &format!("{name}.desc"),
            WRITE_LIMITS.text,
            problems,
        );
        let apis = optional_text_list(
            entry.get("apis"),
            &format!("{name}.apis"),
            WRITE_LIMITS.relation_api,
            WRITE_LIMITS.relation_apis,
            problems,
        );
        let action = match entry.get("action") {
            None | Some(Value::Null) => WriteAction::Upsert,
            Some(Value::String(raw)) => WriteAction::parse(raw).unwrap_or_else(|| {
                problems.push(format!(
                    "{name}.action 非法值 {raw:?}（合法值：{}）",
                    vocabulary(WriteAction::ALL.map(WriteAction::as_str))
                ));
                WriteAction::Upsert
            }),
            Some(_) => {
                problems.push(format!(
                    "{name}.action 必须是字符串（合法值：{}）",
                    vocabulary(WriteAction::ALL.map(WriteAction::as_str))
                ));
                WriteAction::Upsert
            }
        };
        relations.push(RelationSpec {
            to,
            desc,
            apis,
            action,
        });
    }
    relations
}

/// 顶层动作（`upsert` / `delete`，值集由 [`WriteAction::ALL`] 派生）。
fn action_of(value: Option<&Value>, name: &str, problems: &mut Vec<String>) -> Option<WriteAction> {
    let legal = vocabulary(WriteAction::ALL.map(WriteAction::as_str));
    let raw = match value {
        None | Some(Value::Null) => {
            problems.push(format!("{name} 必填（合法值：{legal}）"));
            return None;
        }
        Some(Value::String(raw)) => raw.as_str(),
        Some(_) => {
            problems.push(format!("{name} 必须是字符串（合法值：{legal}）"));
            return None;
        }
    };
    match WriteAction::parse(raw) {
        Some(action) => Some(action),
        None => {
            problems.push(format!(
                "{name} 非法值 {raw:?}（合法值：{legal}；重试：改用合法值）"
            ));
            None
        }
    }
}

/// 值域查表唯一核（E1 的三处封闭字段与 `archi_scaffold` 的 `kind` 同调）：命中即值，
/// 词表外即 `Err` 一条判据句 `{name} 非法值 "{raw}"（合法值：…）`（合法值由 `all` 现算）。
fn domain_value<T: Copy>(
    raw: &str,
    name: &str,
    all: &[T],
    as_str: impl Fn(T) -> &'static str,
) -> Result<T, String> {
    all.iter()
        .copied()
        .find(|candidate| as_str(*candidate) == raw)
        .ok_or_else(|| {
            format!(
                "{name} 非法值 {raw:?}（合法值：{}）",
                vocabulary(all.iter().copied().map(&as_str))
            )
        })
}

/// 封闭值域字段（缺失 / `null` = 不动；非法值经 [`domain_value`] 收进一次报全清单）。
fn optional_domain<T: Copy>(
    value: Option<&Value>,
    name: &str,
    all: &[T],
    as_str: impl Fn(T) -> &'static str,
    problems: &mut Vec<String>,
) -> Option<T> {
    let raw = match value {
        None | Some(Value::Null) => return None,
        Some(Value::String(raw)) => raw.as_str(),
        Some(_) => {
            problems.push(format!(
                "{name} 必须是字符串（合法值：{}）",
                vocabulary(all.iter().copied().map(&as_str))
            ));
            return None;
        }
    };
    match domain_value(raw, name, all, as_str) {
        Ok(candidate) => Some(candidate),
        Err(message) => {
            problems.push(message);
            None
        }
    }
}

/// 布尔开关（缺失 / `null` = `false`）。
fn flag(value: Option<&Value>, name: &str, problems: &mut Vec<String>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            problems.push(format!("{name} 必须是布尔值"));
            false
        }
    }
}

/// 乐观锁基线（`sha256:<hex>`；缺失 / `null` = 不校验）。
fn expect_of(value: Option<&Value>, problems: &mut Vec<String>) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if !text.trim().is_empty() => Some(text.clone()),
        Some(Value::String(_)) => {
            problems.push("expect 不得为空白".to_owned());
            None
        }
        Some(_) => {
            problems.push("expect 必须是字符串（sha256:<hex>）".to_owned());
            None
        }
    }
}

/// 投影结果 → JSON 值（序列化失败即 `Err`）。
fn to_value<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("架构工具结果序列化失败: {error}"))
}

/// 入参的可选非空字符串字段（`link` 的 `from` / `to` / `element` 三入口）。
fn field(input: &Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// FQN 形态闸（`link` 的两侧入参）：形态非法即 `Err` 并点名字段。
fn checked_fqn(raw: &str, field: &str) -> Result<String, String> {
    if archi::is_fqn(raw) {
        return Ok(raw.to_owned());
    }
    Err(format!(
        "{field} `{raw}` 不是合法 FQN（点分段标识符，如 App.Auth）——`$ref` 的文件主干由目标 FQN 派生"
    ))
}
/// 新元素 TOML 草稿（`__KIND__` 由调用方替换为实际 kind）。
///
/// 形状受两条硬约束：元素部分只留一行实值（`kind`），其余字段**全部注释行**；关系
/// 部分同样全注释——空的 `[[relation]]` 表会被解析成缺必填字段的关系，与「草稿过
/// `TomlParser` 零诊断」冲突。FQN 用**单段**占位：多段 FQN 而祖先未声明会触发
/// `orphanHierarchy`。
const SCAFFOLD_TEMPLATE: &str = r#"# 新元素模板（只读草稿：写盘用 archi_element / archi_relation；接口草稿用 archi_api 的 bootstrap；工具表达不了的才手改 TOML）
# 单段占位 FQN：改成你的完整 FQN；多段 FQN（如 App.Auth）必须先声明祖先元素
[element."NewElement"]

kind = "__KIND__"
# kind：package = 包（至多一条包根 path，纯容器可留空）/ domain = 领域容器（path 选填）/ module = crate、目录型模块、视图或单文件组件（path 必填、可多条）/ composite = 纯分组（无层级，可嵌套任意类型之上；纯分层容器是标准形态）

# gate：缺省 off（ratchet 逐块收紧），值 off / warn / strict
# gate = "off"

# api：缺省 declared（承诺有接口定义文件）；显式无接口面写 none（schema 在盘即违约）
# api = "none"

# entry：单值，只在约定默认指不到时写（repo 相对 POSIX；单文件根 = 根文件自身 / 目录型根 = index.ts(x) / crate = src/lib.rs）
# entry = "./src-tauri/crates/core/example/src/lib.rs"

# path：module 必填（可多条 = 若干根的清单）；package 至多一条包根（纯容器可留空；≥2 条即模型停摆）；domain / composite 选填；`./` 前缀、repo 相对；目录以 `/` 结尾、文件精确到扩展名
# path = ["./src-tauri/crates/core/example/src/"]
# path = ["./src-tauri/crates/core/example/src/lib.rs"]

# doc：约定字段（零判据读者）：常规只有 api = none 的元素会写，写了不报、指不到不报
# doc = "./openspec/specs/example/spec.md"

# summary：一行摘要
# summary = "一句话说明本元素负责什么"

# metadata：选填自由表（字符串 → 字符串）——值 MUST 是字符串（数字 / 布尔等非字符串值被拒）；
#           子表与内联表等价（[element."X".metadata] 写在已声明元素之后零诊断；成为唯一声明才缺 kind）
# metadata = { owner = "team-x", layer = "core" }

# 关系模板
# [[relation]]
# from = "App.NewElement"
# to = "App.Dependency"
# desc = "为什么依赖"
# apis = ["ExportedName"]
# apis：选填名字清单（可留空 = 不声明 API 粒度）——只写名字（禁签名 / 类型形状 / `::` 限定路径 / 空白）；
#       逐项非空、同名不重复；`apis = []` 与省略同义（显式空数组被拒）；生成物面留空，差额看 archi_drift
"#;

/// 按 kind 出草稿（占位替换单点；草稿含注释行与一行实值 `kind`）。
fn scaffold_draft(kind: archi::dto::ElementKind) -> String {
    SCAFFOLD_TEMPLATE.replace("__KIND__", kind.as_str())
}
