//! 模块 API 契约面：手写 `<FQN>.md`（唯一真相）→ 四类生成物 → 十条判据的
//! 判定与装配层。
//!
//! **纯读盘、零写盘、零 spawn**（沿本 crate 既有纪律）：契约解析、`$defs` 规则、
//! schema / 索引装配、声明抽取、判据求值与渲染全在这里；唯一写盘面是
//! `crates/tools/archi-api-gen`（CLI 工具，「生成物要写盘」与「工具需要 CLI 入口」
//! 是该边界的正当属性，MUST NOT 反向稀释本 crate 的零写盘纪律）。
//!
//! **契约面启用条件（冷启动纪律）**：契约目录缺席或其中零份 `*.md` ⇒ 未启用，判据
//! 返回空集且既有漂移语料逐字不变；目录一旦存在即**全部 `path` 非空元素**纳入判据，
//! 无白名单、无开关、无部分豁免。
//!
//! 边界：零 UI 框架 / 零库句柄 / 零子进程 / 零工作流与编排 crate 依赖 / 零工作区
//! 配置读取。

pub mod check;
pub mod contract;
pub mod emit;
pub mod extract;
pub mod paths;
pub mod schema;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::dto::{ApiStatus, ArchiApiRef, ArchiElement};

// 出口面只留 `lib.rs` 的逐名 `pub use`（D60）：本文件只 `use` 自己消费的名字。
use contract::Contract;
use emit::{EmitTarget, ModuleRef};
use extract::{ApiSymbol, ApiSymbolKind, ApiType};
use schema::{build_index, ApiFormStatus, IndexEntry};

/// 契约面错误面（封闭三值）：契约是判定的输入资产，资产坏了应「检查不执行、不产出
/// 半真报告」（[`crate::drift::check`] 把它映射进既有 `CheckError::Config`，不新增
/// 第四变体）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// 读盘失败（契约文件存在但不可读 / 被目录占位）
    Io(String),
    /// 契约资产非法（结构 / 双向 gate / `$defs` 规则 / 签名形态）
    Contract(String),
    /// 元素入参非法（FQN 不存在 / 无实现入口 / 语言不可判）或元素表不可得
    /// （模型结构性错，见 [`elements_of`]——不新增第四变体，D20 的封闭三值不动）
    Element(String),
}

impl ApiError {
    /// 错误面人读文案（透传内层原文；零包装、零补默认值）。
    pub fn message(&self) -> String {
        match self {
            ApiError::Io(message) | ApiError::Contract(message) | ApiError::Element(message) => {
                message.clone()
            }
        }
    }
}

/// 跨语言映射规则的一条（`common.json` 解析后的内存形态）。
///
/// 资产键 = **协议名字**（首期唯一键 `Outcome`，与 `ApiType::Outcome` 变体名、
/// `#/$defs/Outcome` 同形）；本类型的语言槽位是「协议在目标语言里就地展开成
/// 什么文本形」的唯一承载（D54 ① / D57 ①）。`degraded` / `note` 只作留痕，
/// 不驱动渲染。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonRule {
    /// Rust 侧文本形（含恰 ≥1 个 `{}` 内容位占位符）
    pub rust: String,
    /// TypeScript 侧文本形（含恰 ≥1 个 `{}` 占位符）
    pub typescript: String,
    /// 降级语言（该语言的形态不在类型系统内时记名）
    pub degraded: Option<String>,
    /// 自由注记（不进生成物、不参与判据）
    pub note: Option<String>,
}

/// 生成载元（`schema` / `emit` / 索引三处**共用**的一份；MUST NOT 另立第二份）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiMeta {
    /// 生成器版本（头注释与索引说明段的工具标识）
    pub tool_version: &'static str,
    /// 映射规则资产摘要（`sha256:<hex>`；渲染侧截前 12 位）
    pub common_digest: String,
    /// schema 方言常量（draft 2020-12）
    pub schema_uri: &'static str,
    /// 生成物警示文案（schema 的 `$comment` 与 `.i.*` 头注释共用）
    pub generated_notice: &'static str,
    /// 跨语言映射规则表（键 = 协议名字，值 = 各语言单槽位；D54 ②）
    pub common: BTreeMap<String, CommonRule>,
}

/// 应生成物（内存载体）：写盘归工具 crate，本类型只承载「应生成什么」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    /// 目标路径（调用方以 root 锚定）
    pub path: PathBuf,
    /// 全文内容（含末尾换行）
    pub content: String,
}

/// 一次装载的只读快照（契约 / 内存 schema / 代码符号集）。
///
/// **无「缺契约清单」字段**：`api = declared` 而契约文件缺失是判据 ② 的求值结果
/// （[`check::check`] 逐元素比对 `contracts` 与模型意图即得），装载期再存一份
/// 同名清单就是同一事实的第二个来源——两份会漂移，且判据从不消费它。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiSnapshot {
    /// 已解析并通过双向 gate 的契约（FQN 字典序）
    pub contracts: Vec<Contract>,
    /// FQN → 装配好的 schema（`$defs` 判据的唯一读取面）
    pub schemas: BTreeMap<String, Value>,
    /// FQN → 代码符号集（取数器产出，判据的「代码导出集」一侧）
    pub symbols: BTreeMap<String, Vec<ApiSymbol>>,
}

/// 判据求值入参（全部借用；零 clone 大结构）。
pub struct ApiCheckInput<'a> {
    /// 观察根
    pub root: &'a Path,
    /// 模型元素表（`path` 非空者参与判据）
    pub elements: &'a [ArchiElement],
    /// 已装载契约
    pub contracts: &'a [Contract],
    /// FQN → schema（`$defs` 只读这里，MUST NOT 并读 markdown）
    pub schemas: &'a BTreeMap<String, Value>,
    /// FQN → 代码符号集
    pub symbols: &'a BTreeMap<String, Vec<ApiSymbol>>,
    /// 契约面是否启用（`false` ⇒ 判据空集）
    pub enabled: bool,
}

/// 生成载元单点（`CARGO_PKG_VERSION` + `common.json` 的 `sha256` 摘要 + 规则表）。
///
/// **零失败**（签名无 `Result`）：资产经 `include_str!` 恒在场，本函数只做装载与
/// 摘要，不承担槽位合法性判定——合法性由 [`load_contracts`] 以同一
/// [`parse_common`] 复核并映射为 `ApiError::Contract`（D54 ②；静默跳过不允许）。
pub fn meta() -> ApiMeta {
    let mut hasher = Sha256::new();
    hasher.update(COMMON_JSON.as_bytes());
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    ApiMeta {
        tool_version: env!("CARGO_PKG_VERSION"),
        common_digest: format!("sha256:{digest}"),
        schema_uri: SCHEMA_URI,
        generated_notice: GENERATED_NOTICE,
        // 资产恒在场且形态由 `load_contracts` 把关；此处不 panic（见上注）。
        common: parse_common(COMMON_JSON).unwrap_or_default(),
    }
}

/// 映射规则资产的**唯一解析点**（D54 ②）：`common.json` 文本 → 协议名 → 语言槽位。
///
/// 形态判据（任一不满足即 `ApiError::Contract`，文案点名键与语言）：
/// ① 顶层必须是对象；② 每条必须是对象且**恰含** `rust` / `typescript` 两个字符串
/// 槽位；③ 每个槽位内 MUST 恰含 ≥1 个 `{}` 占位符；④ `degraded` / `note` 可选，
/// 在时为字符串。
pub(crate) fn parse_common(source: &str) -> Result<BTreeMap<String, CommonRule>, ApiError> {
    let root: Value = serde_json::from_str(source)
        .map_err(|error| ApiError::Contract(format!("common.json 不是合法 JSON：{error}")))?;
    let object = root
        .as_object()
        .ok_or_else(|| ApiError::Contract("common.json 顶层必须是对象".to_owned()))?;
    let mut rules = BTreeMap::new();
    for (name, entry) in object {
        let fields = entry
            .as_object()
            .ok_or_else(|| ApiError::Contract(format!("common.json 的 `{name}` 必须是对象")))?;
        let slot = |language: &str| -> Result<String, ApiError> {
            let text = fields
                .get(language)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ApiError::Contract(format!(
                        "common.json 的 `{name}` 缺 `{language}` 语言槽位（或非字符串）"
                    ))
                })?;
            if !text.contains("{}") {
                return Err(ApiError::Contract(format!(
                    "common.json 的 `{name}.{language}` 槽位 MUST 含 ≥1 个 `{{}}` 占位符"
                )));
            }
            Ok(text.to_owned())
        };
        let rust = slot("rust")?;
        let typescript = slot("typescript")?;
        let optional = |key: &str| -> Result<Option<String>, ApiError> {
            match fields.get(key) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(text)) => Ok(Some(text.clone())),
                Some(_) => Err(ApiError::Contract(format!(
                    "common.json 的 `{name}.{key}` 必须是字符串"
                ))),
            }
        };
        let degraded = optional("degraded")?;
        let note = optional("note")?;
        rules.insert(
            name.clone(),
            CommonRule {
                rust,
                typescript,
                degraded,
                note,
            },
        );
    }
    Ok(rules)
}

/// 协议名字 → 契约 / schema 承载键的**换算单点**（D57 ②）。
///
/// 资产键是协议名字（`Outcome`，与 `ApiType::Outcome` / `#/$defs/Outcome` 同形），
/// 而契约与 `schema.json` 里的承载键是小写（`"outcome"`）。两侧渲染器 MUST 经本
/// 函数求承载键，MUST NOT 各写一份键变换；**资产键 MUST NOT 改成小写**。
pub(crate) fn protocol_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// 十条契约判据的**对外唯一入口**：`enabled == false` → 空 `Vec`（契约面未启用）；
/// 否则委托 [`check::check`] 单点求值，再按 `(kind, element, target)` 稳定排序（可逐字
/// diff 的报告）。
///
/// 模块名与函数名分属不同命名空间（`api::check` 是模块、`api::check::check` 是函数），
/// 对外只暴露本函数；调用方一律走 `api::check(input)`。
pub fn check(input: ApiCheckInput<'_>) -> Vec<crate::drift::report::ArchiApiFinding> {
    if !input.enabled {
        return Vec::new();
    }
    let mut findings = check::check(input);
    findings.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then_with(|| left.element.cmp(&right.element))
            .then_with(|| left.target.cmp(&right.target))
    });
    findings
}

/// 契约面启用判定：契约目录存在且**递归**含 ≥1 份 `*.md`（手写 walk，不跟随符号
/// 链接——零 `ignore` crate 依赖）。
pub fn enabled(root: &Path) -> bool {
    has_markdown(&paths::directory(root), 0)
}

/// 递归找 `*.md`（深度上限防环；符号链接不跟随）。
fn has_markdown(directory: &Path, depth: usize) -> bool {
    if depth > MAX_API_WALK_DEPTH {
        return false;
    }
    let Ok(reader) = fs::read_dir(directory) else {
        return false;
    };
    for entry in reader.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if has_markdown(&path, depth + 1) {
                return true;
            }
            continue;
        }
        if metadata.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(MARKDOWN_SUFFIX))
        {
            return true;
        }
    }
    false
}

/// 逐元素装载契约：`api = none` 跳过（留 `apiUnexpected` 自判）；契约文件存在 →
/// 解析 → 装配 schema → 双向 gate → `$defs` 规则；不存在 → 跳过（缺口由判据 ②
/// 从模型意图与 `contracts` 的差集直接得出，不在此另记一份清单）。
///
/// 任一契约非法即 `Err`（MUST NOT 静默跳过坏契约、MUST NOT 返回部分快照）。
pub fn load_contracts(root: &Path, elements: &[ArchiElement]) -> Result<ApiSnapshot, ApiError> {
    // 映射规则资产形态复核（D54 ②）：与 `meta()` 共用同一解析点；失败即
    // `ApiError::Contract`（静默跳过不允许——形态坏的资产会让渲染器落回硬编码）。
    parse_common(COMMON_JSON)?;
    let meta = meta();
    let mut contracts: Vec<Contract> = Vec::new();
    let mut schemas: BTreeMap<String, Value> = BTreeMap::new();
    let mut symbols: BTreeMap<String, Vec<ApiSymbol>> = BTreeMap::new();

    for element in elements {
        if element.path.is_empty() || element.api == ApiStatus::None {
            continue;
        }
        let file = paths::contract_path(root, &element.fqn);
        let Some(file) = file.filter(|path| path.is_file()) else {
            continue;
        };
        let source = fs::read_to_string(&file).map_err(|error| {
            ApiError::Io(format!(
                "契约文件读取失败（{}）：{error}",
                display(root, &file)
            ))
        })?;
        let relative = display(root, &file);
        let parsed = contract::parse(&source, &element.fqn, &relative)?;
        let built = schema::build_schema(&parsed, &meta)?;
        contract::verify(&parsed, &built)?;
        schema::verify_defs(&built)?;
        schemas.insert(element.fqn.clone(), built);
        contracts.push(parsed);
        symbols.insert(element.fqn.clone(), element_symbols(root, element));
    }
    contracts.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(ApiSnapshot {
        contracts,
        schemas,
        symbols,
    })
}

/// 元素的代码符号集（无实现入口 → 空集，缺口由 `apiEntryMissing` 报）。
fn element_symbols(root: &Path, element: &ArchiElement) -> Vec<ApiSymbol> {
    let Some((_, entry)) = paths::entry_form(root, element) else {
        return Vec::new();
    };
    extract::extract_element(root, &entry)
}

/// 装配四类生成物为内存清单（**零写盘**）：`<FQN>.schema.json` / `INDEX.toml` /
/// `<root>.i.rs` / `<root>.i.ts`。
///
/// 元素表经 [`elements_of`]（`dto::graph_view` 单点取数）——模型结构性坏 ⇒ `Err`
/// （fail-closed：坏模型不得被固化成入库生成物）。
///
/// 契约无类型无 op 时**跳过** `.i.*`（零 pub 项的生成物会让入口的
/// `pub use api_gen::*;` 触发 `unused_imports`，`clippy -D warnings` 即红）。
pub fn render_all(root: &Path, snapshot: &ApiSnapshot) -> Result<Vec<GeneratedFile>, ApiError> {
    let meta = meta();
    let mut files: Vec<GeneratedFile> = Vec::new();
    for contract in &snapshot.contracts {
        let Some(schema_value) = snapshot.schemas.get(&contract.fqn) else {
            continue;
        };
        let mut text = serde_json::to_string_pretty(schema_value)
            .map_err(|error| ApiError::Contract(format!("schema 序列化失败：{error}")))?;
        text.push('\n');
        let Some(path) = paths::schema_path(root, &contract.fqn) else {
            return Err(ApiError::Contract(format!(
                "契约 FQN `{}` 无法派生出 schema 生成物路径（FQN 形态非法）",
                contract.fqn
            )));
        };
        files.push(GeneratedFile {
            path,
            content: text,
        });
    }
    let elements = elements_of(root)?;
    for element in &elements {
        if element.path.is_empty() {
            continue;
        }
        let Some(contract) = snapshot
            .contracts
            .iter()
            .find(|contract| contract.fqn == element.fqn)
        else {
            continue;
        };
        let Some(schema_value) = snapshot.schemas.get(&contract.fqn) else {
            continue;
        };
        if contract.ops.is_empty() && contract.types.is_empty() && contract.reexports.is_empty() {
            continue;
        }
        // 语言 → 生成物种类由 `paths::generated_kind` 单点分派（D42：路径只收 `<root>`）
        let scope = element.path.first().map(String::as_str).unwrap_or("");
        let kind = paths::generated_kind(paths::language_of(scope));
        let Some(path) = paths::generated_path(root, scope, kind) else {
            continue;
        };
        // 入口形态由调用方给（D41）：拿不到入口 ⇒ 该契约不产 `.i.*`（与「零内容不生成」
        // 同口径）——形态 MUST NOT 由渲染器二次推导
        let Some((form, _)) = paths::entry_form(root, element) else {
            continue;
        };
        let target = EmitTarget {
            kind,
            form,
            modules: module_refs(contract, &elements, kind),
        };
        let content = emit::render_target(&target, contract, schema_value, &meta)?;
        files.push(GeneratedFile { path, content });
    }
    files.push(GeneratedFile {
        path: paths::index_path(root),
        content: build_index(&index_entries(root, snapshot, &elements), &meta),
    });
    Ok(files)
}

/// `INDEX.toml` 条目（全部 `path` 非空元素各一条；`status` 按契约文件是否在案）。
fn index_entries(
    root: &Path,
    snapshot: &ApiSnapshot,
    elements: &[ArchiElement],
) -> Vec<IndexEntry> {
    let mut entries: Vec<IndexEntry> = Vec::new();
    for element in elements {
        if element.path.is_empty() {
            continue;
        }
        let contract = snapshot
            .contracts
            .iter()
            .find(|contract| contract.fqn == element.fqn);
        let present = contract.is_some()
            && paths::contract_path(root, &element.fqn).is_some_and(|path| path.is_file());
        let schema_value = snapshot.schemas.get(&element.fqn);
        let types = schema_value
            .and_then(|value| value.get("$defs"))
            .and_then(Value::as_object)
            .map(|defs| defs.len())
            .unwrap_or(0);
        let ops = contract.map(|contract| contract.ops.len()).unwrap_or(0);
        let degraded = schema_value
            .and_then(|value| value.get("$defs"))
            .and_then(Value::as_object)
            .map(|defs| {
                defs.values()
                    .filter(|definition| {
                        definition
                            .get("x-common")
                            .and_then(|common| common.get("degraded"))
                            .is_some()
                    })
                    .count()
            })
            .unwrap_or(0);
        let generated_rel = generated_path_of(root, &element.path)
            .filter(|_| present)
            .map(|path| display(root, &path));
        entries.push(IndexEntry {
            fqn: element.fqn.clone(),
            status: if present {
                ApiFormStatus::Declared
            } else {
                ApiFormStatus::Absent
            },
            types,
            ops,
            degraded,
            categories: categories_of(schema_value),
            schema_rel: match paths::schema_path(root, &element.fqn) {
                Some(path) if present => display(root, &path),
                _ => String::new(),
            },
            generated_rel,
        });
    }
    entries
}

/// `$defs` 形状分布（`category` → 条数；`BTreeMap` 保字典序 ⇒ 幂等 diff）。
///
/// 无契约 / `absent` 元素返回空表（[`build_index`] 会整段省略，MUST NOT 输出空表）。
fn categories_of(schema: Option<&Value>) -> BTreeMap<String, usize> {
    let mut categories: BTreeMap<String, usize> = BTreeMap::new();
    let Some(defs) = schema
        .and_then(|value| value.get("$defs"))
        .and_then(Value::as_object)
    else {
        return categories;
    };
    for definition in defs.values() {
        *categories
            .entry(def_shape(definition).to_owned())
            .or_insert(0) += 1;
    }
    categories
}

/// 单个 `$defs` 条目的形状名（开放字符串集；新增形状只改本函数）。
///
/// 判序固定：纯 `$ref` → `reexport`；`x-class` → `class`；`oneOf` / `anyOf` →
/// `variant`；`enum` → `enum`；标量（`string` / `integer` / `number` / `boolean` 且
/// 无 `enum`）→ `scalar`；其余（含 `object`）→ `object`。
///
/// **MUST NOT 复用判据分类**（`check` 模块里按 kind 打标那个函数）：那是判据结果的
/// 分类（`entry` / `contract` / …），随债数漂移；本函数是契约形状的静态分布，只随
/// 契约改动变化。
fn def_shape(definition: &Value) -> &'static str {
    let Some(object) = definition.as_object() else {
        return "object";
    };
    if object.contains_key("$ref") {
        return "reexport";
    }
    if object.get("x-class").and_then(Value::as_bool) == Some(true) {
        return "class";
    }
    if object.contains_key("oneOf") || object.contains_key("anyOf") {
        return "variant";
    }
    if object.contains_key("enum") {
        return "enum";
    }
    if object
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "string" | "integer" | "number" | "boolean"))
    {
        return "scalar";
    }
    "object"
}

/// 取数器草稿（**只出字符串、零写盘**）：定位入口 → 读源 → 抽取 → md 骨架。
///
/// 每个符号附 `visibility` 与定义位置注释；构造器条目不进草稿（构造器由生成器产出）。
pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError> {
    if element.trim().is_empty() {
        return Err(ApiError::Element("元素 FQN 为空".to_owned()));
    }
    let elements = elements_of(root)?;
    let found = elements
        .iter()
        .find(|candidate| candidate.fqn == element)
        .ok_or_else(|| {
            ApiError::Element(format!(
                "元素不存在：{element}（模型共 {} 个元素）",
                elements.len()
            ))
        })?;
    let Some((_, entry)) = paths::entry_form(root, found) else {
        return Err(ApiError::Element(format!(
            "元素 {element} 无实现入口（`path` 与三条 / 四条入口候选均未命中）——\
             无法起草契约"
        )));
    };
    // 取数面与判据的「代码导出集」同源（同一元素的公开面散在入口与它声明的模块文件里）；
    // 只读入口会把真正的操作漏掉，草稿随即失真
    let symbols = extract::extract_element(root, &entry);
    Ok(bootstrap_text(element, &entry, &symbols))
}

/// md 骨架文本（H1 + 空散文段 + 每操作一个 `### <op-id>` + `api-types` fence）。
fn bootstrap_text(fqn: &str, entry: &str, symbols: &[ApiSymbol]) -> String {
    let mut text =
        format!("# {fqn}\n\n## 职能\n\n（待补：一句职能）\n\n## 不变量\n\n（待补）\n\n## 操作\n\n");
    for symbol in symbols
        .iter()
        .filter(|symbol| symbol.kind == ApiSymbolKind::Function)
        .filter(|symbol| !contract::is_constructor(&symbol.name))
    {
        text.push_str(&format!(
            "### {}\n\n<!-- {} @ {entry}:{}{} -->\n\n",
            symbol.name,
            symbol.visibility.as_str(),
            symbol.line,
            if symbol.is_async { " async" } else { "" }
        ));
        text.push_str("```json api-op\n");
        text.push_str(&bootstrap_op(symbol));
        text.push_str("\n```\n\n");
    }
    text.push_str("## 类型\n\n```json api-types\n");
    text.push_str(&bootstrap_types(symbols));
    text.push_str("\n```\n");
    text
}

/// 单个操作的 `api-op` fence 内容（参数 / 返回类型按逻辑类型投影）。
fn bootstrap_op(symbol: &ApiSymbol) -> String {
    let params: Vec<String> = symbol
        .params
        .iter()
        .map(|param| {
            format!(
                "{{\"name\": \"{}\", \"schema\": {}}}",
                param.name,
                json_type(&param.ty)
            )
        })
        .collect();
    let mut body = format!(
        "  \"params\": [{}],\n  \"returns\": {}",
        params.join(", "),
        json_type(&symbol.returns)
    );
    if symbol.is_async {
        body.push_str(",\n  \"async\": true");
    }
    format!("{{\n{body}\n}}")
}

/// `api-types` fence 内容（本次抽到的类型类符号；形状留给人工剪枝）。
fn bootstrap_types(symbols: &[ApiSymbol]) -> String {
    let named: Vec<&ApiSymbol> = symbols
        .iter()
        .filter(|symbol| symbol.kind != ApiSymbolKind::Function)
        .collect();
    if named.is_empty() {
        return "{}".to_owned();
    }
    let entries: Vec<String> = named
        .iter()
        .map(|symbol| {
            format!(
                "  \"{}\": {{\"type\": \"object\", \"x-common\": {{\"note\": \"待剪枝：{} @ {}:{} {}\"}}}}",
                symbol.name,
                match symbol.kind {
                    ApiSymbolKind::Type => "类型",
                    ApiSymbolKind::Trait => "trait",
                    ApiSymbolKind::Enum => "枚举",
                    ApiSymbolKind::Function => "函数",
                },
                "入口",
                symbol.line,
                symbol.visibility.as_str()
            )
        })
        .collect();
    format!("{{\n{}\n}}", entries.join(",\n"))
}

/// 逻辑类型 → JSON Schema 片段（粗投影：bootstrap 是草稿，形状由人剪枝）。
fn json_type(ty: &ApiType) -> String {
    match ty {
        ApiType::Str | ApiType::OwnedString => "{\"type\": \"string\"}".to_owned(),
        ApiType::Int => "{\"type\": \"integer\"}".to_owned(),
        ApiType::Float => "{\"type\": \"number\"}".to_owned(),
        ApiType::Bool => "{\"type\": \"boolean\"}".to_owned(),
        ApiType::List(inner) => format!("{{\"type\": \"array\", \"items\": {}}}", json_type(inner)),
        ApiType::Map(inner) => format!(
            "{{\"type\": \"object\", \"additionalProperties\": {}}}",
            json_type(inner)
        ),
        ApiType::Optional(inner) => json_type(inner),
        ApiType::Outcome(inner) => format!(
            "{{\"x-common\": {{\"note\": \"错误面\"}}, \"outcome\": {}}}",
            json_type(inner)
        ),
        ApiType::Named(name) => format!("{{\"$ref\": \"#/$defs/{name}\"}}"),
        ApiType::Unknown(raw) if raw.is_empty() => "{\"type\": \"object\"}".to_owned(),
        ApiType::Unknown(raw) => format!("{{\"x-common\": {{\"note\": \"待剪枝：{raw}\"}}}}"),
    }
}

/// 契约状态投影（D22 双层口径）：契约文件在案 → `declared` 且三路径给出（`schema`
/// 与生成物即使尚未生成也给路径串）；否则 `none` 且三路径 `None`。
///
/// **投影只报文件事实，MUST NOT 回写模型字段**——模型存的是意图（`declared` 而文件
/// 缺失是迁移期主形态），抹平会让消费方以为「有契约可读」。
///
/// 模型结构坏（[`elements_of`] 的停摆态）⇒ 拿不到元素 `path`，按空 `path` 投影：
/// 契约 / schema 路径仍报文件事实，生成物路径退 `None`（本入口无错误面）。
pub fn api_ref(root: &Path, fqn: &str) -> ArchiApiRef {
    let path = elements_of(root)
        .ok()
        .and_then(|elements| {
            elements
                .into_iter()
                .find(|candidate| candidate.fqn == fqn)
                .map(|element| element.path)
        })
        .unwrap_or_default();
    api_ref_of(root, fqn, &path)
}

/// 同一投影的元素形态入口（消费方手上已有元素 `path` 时用——投影层不必为拿一条
/// 路径而二次读模型）。
pub fn api_ref_of(root: &Path, fqn: &str, element_path: &[String]) -> ArchiApiRef {
    let contract = paths::contract_path(root, fqn).filter(|path| path.is_file());
    let schema = paths::schema_path(root, fqn);
    let generated = generated_path_of(root, element_path);
    match (contract, schema, generated) {
        (Some(contract), Some(schema), generated) => ArchiApiRef {
            status: ApiStatus::Declared,
            contract_path: Some(display(root, &contract)),
            schema_path: Some(display(root, &schema)),
            generated_path: generated.map(|path| display(root, &path)),
        },
        _ => ArchiApiRef {
            status: ApiStatus::None,
            contract_path: None,
            schema_path: None,
            generated_path: None,
        },
    }
}

/// 契约的透传目标查表（键 = 目标元素 FQN，值 = 该元素根的 [`ModuleRef`]，D40）。
///
/// 查表属调用方：`render_all` 手上已有元素表，逐条把 `reexports` 的目标 FQN 经
/// [`paths::module_name`] 与 [`paths::rust_module_path`] 各算一次；模型里没有的目标不入
/// 表——渲染器据此只出 `// 透传自 <FQN>` 注释（MUST NOT 退化到 FQN 派生）。
///
/// **形态筛选按语言分列（「路径值进目标语言源码」的安全边界，本函数是唯一落点）**：
/// `ModuleRef` 的两个字段都会落进目标语言源码，而两语言的约束不同——
/// - **Rust**：`pub use <模块路径>::<名>` 的模块段是**语法位置**，只接受标识符。末段非法即
///   产出 `pub use crate::welcome-view.tsx::X;` 一类**不可编译文本** ⇒ 非
///   [`paths::is_module_segment`] 的目标**不入表**（降级为注释，与「模型里没有该元素」同
///   一条通路）。Rust 无「转义成标识符」这回事，故只有两态。该判据是**合取二项**——形态
///   `[A-Za-z_][A-Za-z0-9_]*` **且非 Rust 关键字**（`crate` / `self` / `super` / `loop` 一类
///   进语法位置同样不可编译，而产出 `r#loop` 是第二形态、MUST NOT，D50）。
/// - **TS**：`from '<specifier>'` 落在**字符串字面量**里，任意文本都有安全形态（渲染器经
///   `emit::ts::ts_string_literal` 转义），且带扩展名的末段（`views/architecture.tsx` →
///   `./architecture.tsx.i`）本就是**合法且必需**的 TS 引用形式 ⇒ **零筛选**。拿 Rust 的
///   标识符判据去筛 TS 会误杀前端 barrel 对单文件组件元素的合法透传。
///
/// **MUST NOT 在此改名 / 清洗末段**：那是第二套命名（D40 的射程），也是「静默错配」的来源。
/// **MUST NOT 在渲染期重复判定形态**：本函数是唯一形态闸（在盘实测 `render_all` 是它唯一
/// 调用点），落点面因此零二次校验（D40 / D41 / D45 同款纪律）。
fn module_refs(
    contract: &Contract,
    elements: &[ArchiElement],
    kind: paths::GeneratedKind,
) -> BTreeMap<String, ModuleRef> {
    let mut modules: BTreeMap<String, ModuleRef> = BTreeMap::new();
    for target in contract.reexports.values() {
        let Some(fqn) = target.as_str() else {
            continue;
        };
        if modules.contains_key(fqn) {
            continue;
        }
        let Some(element) = elements.iter().find(|candidate| candidate.fqn == fqn) else {
            continue;
        };
        let Some(element_root) = element.path.first() else {
            continue;
        };
        let name = paths::module_name(element_root);
        // 空末段（`"./"` 归一后）在两语言下都「没有模块名可派生」——语言无关的退化，与
        // 「模型里没有该元素」同一条降级通路（不是字符集判据）。
        if name.is_empty() {
            continue;
        }
        if kind == paths::GeneratedKind::Rust && !paths::is_module_segment(&name) {
            continue;
        }
        modules.insert(
            fqn.to_owned(),
            ModuleRef {
                name,
                module_path: paths::rust_module_path(element_root),
            },
        );
    }
    modules
}

/// 元素生成物路径（按 `path[0]` 的扩展名判语言；`path` 空或语言不可判 → `None`）。
///
/// 语言 → 生成物种类经 [`paths::generated_kind`] 单点分派，本函数零第二份 match；
/// `paths::generated_path` 只收 `<root>` 字符串（D42）——「容器元素天然无生成物」由
/// 本函数的 `path.first()?` 表达，MUST NOT 为凑参数构造占位 `ArchiElement`。
fn generated_path_of(root: &Path, element_path: &[String]) -> Option<PathBuf> {
    let element_root = element_path.first()?;
    let kind = paths::generated_kind(paths::language_of(element_root));
    paths::generated_path(root, element_root, kind)
}

/// 模型元素表（`path` 非空判据的输入；模型目录缺席 → 空表）。
///
/// **取材单点**：经 [`crate::dto::graph_view`]（画布 / 漂移门禁 / 工具侧同一入口），
/// MUST NOT 自行 `parse_dir` 再抄一份元素投影——同一事实两个来源必然在停摆态分叉
/// （`graph_view` 给空表，自抄投影照出元素，`INDEX.toml` 随即被改写成全体 `absent`）。
///
/// 结构诊断 ⇒ `Err(ApiError::Element)`（fail-closed）：模型本身坏了时写生成物会把
/// 坏事实固化进入库资产，本面与既有 `dto` 各投影的「结构错 ⇒ `Err`」同口径。
fn elements_of(root: &Path) -> Result<Vec<ArchiElement>, ApiError> {
    let graph = crate::dto::graph_view(root)
        .map_err(|_| ApiError::Element("模型读取失败（契约面的元素表来源）".to_owned()))?;
    if !graph.diagnostics.is_empty() {
        return Err(ApiError::Element(crate::dto::graph_diagnostics_error(
            &graph.diagnostics,
        )));
    }
    Ok(graph.elements)
}

/// `$ref` → 类型名（`#/$defs/<名>` 与外部文件形态 `…/<FQN>.schema.json#/$defs/<名>`
/// 都取末段；无 `$defs` 段时退化为路径末段）。
///
/// **单点**：`check`（签名归一）与 `emit::{rust, ts}`（类型映射）共用本函数，
/// MUST NOT 各写一份 `rsplit_once`。
pub(crate) fn ref_name(reference: &str) -> String {
    reference
        .rsplit_once(DEFS_FRAGMENT)
        .map(|(_, name)| name.to_owned())
        .unwrap_or_else(|| reference.rsplit('/').next().unwrap_or(reference).to_owned())
}

/// 外部契约文件引用 → 目标元素 FQN（`<相对前缀><FQN>.schema.json#/$defs/<名>` 的
/// 中间段）；同文件引用（`#/$defs/<名>`）与无 `.schema.json` 后缀者 → `None`。
///
/// **单点**：`check::ref_missing`（跨模块引用检查）与 `emit::{rust, ts}`（透传渲染）
/// 共用本函数。
pub(crate) fn reexport_target(reference: &str) -> Option<String> {
    let file = reference.split_once('#').map(|(file, _)| file)?;
    file.rsplit('/')
        .next()?
        .strip_suffix(SCHEMA_FILE_SUFFIX)
        .map(str::to_owned)
}

/// JSON 值的类型名（错误文案用）。契约解析与 schema 装配共用同一份词表。
pub(crate) fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "布尔",
        Value::Number(_) => "数字",
        Value::String(_) => "字符串",
        Value::Array(_) => "数组",
        Value::Object(_) => "对象",
    }
}

/// `sha256:<hex>` → `sha256:<前 12 位>`（头注释与索引说明段共用的短形单点）。
pub(crate) fn short_digest(digest: &str) -> String {
    match digest.split_once(':') {
        Some((algorithm, hex)) => {
            format!("{algorithm}:{}", hex.chars().take(12).collect::<String>())
        }
        None => digest.chars().take(12).collect(),
    }
}

/// 绝对路径 → repo 相对 POSIX 串（剥 root 前缀、`\` → `/`）。
///
/// **跨 crate 唯一单点**（D55）：`crates/tools/archi-api-gen` 经 `archi::display`
/// 复用本函数；工具侧 MUST NOT 再持一份逐字实现。
pub fn display(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_owned()
}

/// 映射规则资产（`Outcome` 信封的跨语言展开规则；`include_str!` 编译期内嵌）。
pub(crate) const COMMON_JSON: &str = include_str!("common.json");

/// schema 方言常量（draft 2020-12；消费方 MUST 按此方言 + `strict: "log"` 校验，
/// `x-*` 是未知关键字）。
const SCHEMA_URI: &str = "https://json-schema.org/draft/2020-12/schema";

/// 生成物警示文案（`$comment` 与 `.i.*` 头注释共用同一句）。
const GENERATED_NOTICE: &str = "本文件由元素的契约 markdown 生成，勿手改：手改将在下次 generate 时被覆盖，且 api:check 零 diff 门会失败";

/// 契约目录递归扫描的深度上限（`<Domain>/<Sub>/` 级足够；防环）。
const MAX_API_WALK_DEPTH: usize = 4;

/// 契约文件扩展名（启用判定的唯一收面判据）。
const MARKDOWN_SUFFIX: &str = ".md";

/// `$ref` 的 `$defs` 片段（[`ref_name`] 的切分点，schema 装配与两渲染器同源）。
pub(crate) const DEFS_FRAGMENT: &str = "#/$defs/";

/// schema 生成物后缀（[`reexport_target`] 的切分点，与 [`paths::schema_path`] 同源）。
pub(crate) const SCHEMA_FILE_SUFFIX: &str = ".schema.json";

#[cfg(test)]
mod check_test;
#[cfg(test)]
mod contract_test;
#[cfg(test)]
mod mod_test;
#[cfg(test)]
mod paths_test;
#[cfg(test)]
mod schema_test;
