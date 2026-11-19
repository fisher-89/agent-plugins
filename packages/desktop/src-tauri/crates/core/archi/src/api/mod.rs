//! 模块 API 契约面：手写 `<FQN>.md`（唯一真相）→ 四类生成物 → 十三条判据的
//! 判定与装配层。
//!
//! **纯读盘、零写盘、零 spawn**（沿本 crate 既有纪律）：契约解析、`$defs` 规则、
//! schema / 索引装配、声明抽取、判据求值、渲染与比对全在这里。生成物写盘单点在
//! `crates/infra/archi::api_artifacts`（与报告落盘并列的第二处 dumb IO 面），CLI
//! （`archi-api-gen`）与两条 IPC（检查 / 生成）两个驱动面同源；本 crate 零写盘
//! 纪律 MUST NOT 反向稀释。
//!
//! **契约面启用条件（冷启动纪律）**：`path` 非空元素的入口文档载体全都不在盘、或
//! 在盘者正文皆无 api fence 语言标 ⇒ 未启用，判据返回空集且既有漂移语料逐字不变；
//! 一级 `NN-<域>.toml`（模型声明）与 `models/` 里的杂项 md **不是**启用证据。载体
//! 派生单点在 [`paths::carrier_path`]（`declared` → 派生契约 md / `none` → `doc`
//! 声明）。一旦启用即**全部 `path` 非空元素**纳入判据，无白名单、无开关、无部分豁免。
//!
//! 边界：零 UI 框架 / 零库句柄 / 零子进程 / 零工作流与编排 crate 依赖 / 零工作区
//! 配置读取。

pub mod check;
pub mod contract;
pub mod diff;
pub mod emit;
pub mod extract;
pub mod paths;
pub mod render;
pub mod schema;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::drift::report::ArchiApiFinding;
use crate::dto::{ApiStatus, ArchiApiCheck, ArchiApiMeta, ArchiApiRef, ArchiElement};

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
    /// 元素入参非法（FQN 不存在 / 无可用入口 / 语言不可判）或元素表不可得
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

/// 载体事实（进程内三态，不进 IPC）：`path` 非空元素的入口文档载体的盘上事实。
///
/// 三态互斥且穷尽：
/// - [`DocFact::Absent`]：`api = none` 且未声明 `doc`（没有载体，合法但产出 `docMissing`）；
/// - [`DocFact::Missing`]：载体已派生（`declared`）或已声明（`none`）但指不到盘——
///   不在盘，或声明值形态越根（绝对路径 / 盘符 / `~` / `..` / 空段）而派生不出路径；
///   两种成因同属「声明在案」，故同报 `docNotFound`，MUST NOT 混进 `Absent`；
/// - [`DocFact::Present`]：载体在盘 + 正文是否含 api fence 语言标（只判有无，不判
///   fence 合法性——那是 `contract::parse` 的 `Err` 面）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocFact {
    /// 无载体（`api = none` 且未声明 `doc`）
    Absent,
    /// 载体声明在案但指不到盘（不在盘 / 声明值形态越根）
    Missing,
    /// 载体在盘；`has_api_fence` = 正文含 `api-op` / `api-types` 语言标
    Present {
        /// 正文是否含 api fence 语言标
        has_api_fence: bool,
    },
}

/// 一次装载的只读快照（契约 / 内存 schema / 代码符号集 / 载体事实）。
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
    /// FQN → 载体事实（与契约装载同批产出；启用判据与 `apiDocMissing` 的唯一来源）
    pub doc_facts: BTreeMap<String, DocFact>,
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
    /// FQN → 载体事实（check 侧唯一读取面：零读盘）
    pub doc_facts: &'a BTreeMap<String, DocFact>,
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

/// 十三条契约判据的**对外唯一入口**：`enabled == false` → 空 `Vec`（契约面未启用）；
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

/// 检查即生成预览（DTO 投影单点）：三类差异 + `enabled` + `meta()` 三面对照。
///
/// 命令层零投影——`archi_api_check` 只是本函数 + `check_root` + 错误映射；
/// `enabled = false` 仍 `Ok`（应生成集只含 `INDEX.toml`，`missing` 照实报，面板据此
/// 呈现冷启动态而非空面板）。
pub fn artifact_check(root: &Path) -> Result<ArchiApiCheck, ApiError> {
    let diff = diff::compare(root)?;
    let meta = meta();
    // 启用判据需元素表（经 `elements_of` 单点取数；模型结构诊断 ⇒ `ApiError::Element`）
    let elements = elements_of(root)?;
    Ok(ArchiApiCheck {
        enabled: enabled(root, &elements),
        meta: ArchiApiMeta {
            tool_version: meta.tool_version.to_owned(),
            common_digest: meta.common_digest,
        },
        mismatched: diff.mismatched,
        orphaned: diff.orphaned,
        missing: diff.missing,
    })
}

/// 契约判据装配单点：一次装载 → 由快照的载体事实派生启用 → 十三判据（稳定排序）。
///
/// [`crate::drift::check`] 与 agent 工具 `archi_api` 共用本函数，MUST NOT 出现第二份
/// 「契约输入装配」（两份必然在契约资产非法时分叉）。`enabled` 不再前置独立判定——
/// 事实装配只有 [`load_contracts`] 一处，`api::check` 的对外入口仍保留
/// `enabled == false → 空集` 短路。
pub fn findings(root: &Path, elements: &[ArchiElement]) -> Result<Vec<ArchiApiFinding>, ApiError> {
    let snapshot = load_contracts(root, elements)?;
    let enabled = facts_enabled(&snapshot.doc_facts);
    Ok(check(ApiCheckInput {
        root,
        elements,
        contracts: &snapshot.contracts,
        schemas: &snapshot.schemas,
        symbols: &snapshot.symbols,
        doc_facts: &snapshot.doc_facts,
        enabled,
    }))
}

/// `status` 纳管子集筛选单点：口径 = **入口文档承诺已成立**。
///
/// 入选 = `element` 非空 ∧（`api = declared`：派生的契约 md 在盘 / `api = none`：
/// `doc` 字段已声明——声明本身即承诺，`docNotFound` 判的正是它）。`docMissing` 两侧
/// 都不成立 ⇒ 永不纳管（否则存量入口文档债会把状态词常规化为永久 `warnings`）。
///
/// 「载体在盘」的判定只在本函数一次性发生——载体经 [`paths::carrier_path`] 单点派生，
/// MUST NOT 在此另写 `declared` / `none` 的旁路派生；`drift` 侧 MUST NOT 自行判
/// `is_file`，前端 / 命令层 MUST NOT 二次筛选。`element = None` 的条目落选（无归属即
/// 无承诺可对）。
pub fn gating_findings(
    root: &Path,
    elements: &[ArchiElement],
    findings: &[ArchiApiFinding],
) -> Vec<ArchiApiFinding> {
    let gating: BTreeSet<&str> = elements
        .iter()
        .filter(|element| match element.api {
            ApiStatus::Declared => {
                paths::carrier_path(root, element).is_some_and(|path| path.is_file())
            }
            ApiStatus::None => element.doc.is_some(),
        })
        .map(|element| element.fqn.as_str())
        .collect();
    findings
        .iter()
        .filter(|finding| {
            finding
                .element
                .as_deref()
                .is_some_and(|element| gating.contains(element))
        })
        .cloned()
        .collect()
}

/// 契约面启用的唯一判据：任一 `path` 非空元素的入口文档载体在盘且正文含 api fence
/// 语言标（= [`doc_facts`] 中至少一个 `Present { has_api_fence: true }`）。
///
/// MUST NOT 退化为「`<archi_models_dir>` 递归任意 `*.md`」——`models/` 里放一份笔记
/// （非任何元素的载体）或一份无 fence 的 `<分层目录>/notes.md` MUST NOT 误判为启用。
/// 判据与判据面同源（同一 `doc_facts`、同一 fence 语言标常量），命令层 MUST NOT 另
/// 判一套。
pub fn enabled(root: &Path, elements: &[ArchiElement]) -> bool {
    facts_enabled(&doc_facts(root, elements))
}

/// 启用判据本体（[`enabled`] 与 [`findings`] 共用的唯一判定）：任一载体在盘且含 api
/// fence 语言标。
fn facts_enabled(facts: &BTreeMap<String, DocFact>) -> bool {
    facts.values().any(|fact| {
        matches!(
            fact,
            DocFact::Present {
                has_api_fence: true
            }
        )
    })
}

/// 载体事实装配（**唯一装配点**）：每份载体至多读一次即弃（不缓存全文）。
///
/// **「派生不出路径」不直接等于「无载体」**：`doc` 已声明而形态越根 ⇒ 声明在案、指不到
/// 盘（`Missing`），`doc` 未声明 ⇒ 真无载体（`Absent`）——判据面对两者分档、分纳管结论。
///
/// 读失败按 `Present { has_api_fence: false }` 处置——启用判定是探针，不得因一个坏
/// 文件把 `archi_api_check` 整条打成 `Err`；契约解析的读失败仍是既有 `ApiError::Io`。
fn doc_facts(root: &Path, elements: &[ArchiElement]) -> BTreeMap<String, DocFact> {
    let mut facts: BTreeMap<String, DocFact> = BTreeMap::new();
    for element in elements.iter().filter(|element| !element.path.is_empty()) {
        // 派生不出载体有两种成因，分流依据是「`doc` 声明在不在」而不是「路径派生成没成」：
        // 未声明 ⇒ 真无载体（`Absent` ⇒ `docMissing`，warn 不纳管）；已声明而形态越根
        // ⇒ 声明在案、指不到盘（`Missing` ⇒ `docNotFound`，error 且纳管）。归并成
        // `Absent` 会让「把 `doc` 指向不存在文件」降级为「还没写」，`docNotFound` 的
        // error 档与纳管口径同时失效——那正是本档要堵的消红后门。
        let Some(carrier) = paths::carrier_path(root, element) else {
            let fact = if element.doc.is_some() {
                DocFact::Missing
            } else {
                DocFact::Absent
            };
            facts.insert(element.fqn.clone(), fact);
            continue;
        };
        if !carrier.is_file() {
            facts.insert(element.fqn.clone(), DocFact::Missing);
            continue;
        }
        let has_api_fence = match fs::read_to_string(&carrier) {
            Ok(source) => has_api_fence(&source),
            Err(_) => false,
        };
        facts.insert(element.fqn.clone(), DocFact::Present { has_api_fence });
    }
    facts
}

/// 正文是否含 api fence（只看语言标有无，不判 fence 合法性——那是 `contract::parse`
/// 的 `Err` 面）。
///
/// 语言标与切分单点复用 [`contract::FENCE_OP`] / [`contract::FENCE_TYPES`] /
/// [`contract::info_tokens`]：MUST NOT 抄第二份字面量、MUST NOT 补写半个 markdown
/// 解析器。
fn has_api_fence(source: &str) -> bool {
    source.lines().any(|raw| {
        let trimmed_start = raw.trim_end().trim_start();
        if !trimmed_start.starts_with("```") {
            return false;
        }
        let info = trimmed_start.trim_start_matches("```").trim();
        let tokens = contract::info_tokens(info);
        tokens.contains(&contract::FENCE_OP) || tokens.contains(&contract::FENCE_TYPES)
    })
}

/// 逐元素装载契约：`api = none` 跳过（留 `apiUnexpected` 自判）；契约文件存在 →
/// 解析 → 装配 schema → 双向 gate → `$defs` 规则；不存在 → 跳过（缺口由 `docMissing`
/// 判据从载体事实直接得出，不在此另记一份清单）。
///
/// 任一契约非法即 `Err`（MUST NOT 静默跳过坏契约、MUST NOT 返回部分快照）。
pub fn load_contracts(root: &Path, elements: &[ArchiElement]) -> Result<ApiSnapshot, ApiError> {
    // 映射规则资产形态复核（D54 ②）：与 `meta()` 共用同一解析点；失败即
    // `ApiError::Contract`（静默跳过不允许——形态坏的资产会让渲染器落回硬编码）。
    parse_common(COMMON_JSON)?;
    let meta = meta();
    // 载体事实与契约装载同批产出（`facts` 与函数同名会遮蔽，故先命名清楚）
    let facts = doc_facts(root, elements);
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
        symbols.insert(
            element.fqn.clone(),
            element_symbols(root, elements, element),
        );
    }
    contracts.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(ApiSnapshot {
        contracts,
        schemas,
        symbols,
        doc_facts: facts,
    })
}

/// 元素的代码符号集（无可用入口 → 空集，缺口由 `apiEntryMissing` 报）。
fn element_symbols(
    root: &Path,
    elements: &[ArchiElement],
    element: &ArchiElement,
) -> Vec<ApiSymbol> {
    let paths::EntryState::Resolved { entry, .. } = paths::entry_state(root, elements, element)
    else {
        return Vec::new();
    };
    extract::extract_element(root, elements, element, &entry)
}

/// 装配四类生成物为内存清单（**零写盘**）：`<FQN>.schema.json` / `INDEX.toml` /
/// `<入口主干>.i.rs` / `<入口主干>.i.ts`（后两者落**实现入口同级**）。
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
        // 入口形态与落点均取 `entry_state` 的 `Resolved`（D41 / D6）：无可用入口 ⇒ 该契约
        // 不产 `.i.*`（与「零内容不生成」同口径）——形态 MUST NOT 由渲染器二次推导
        let paths::EntryState::Resolved { form, entry } =
            paths::entry_state(root, &elements, element)
        else {
            continue;
        };
        // 语言 → 生成物种类由 `paths::generated_kind` 单点分派（输入是入口扩展名）
        let kind = paths::generated_kind(paths::language_of(&entry));
        let Some(path) = paths::generated_path(root, &entry, kind) else {
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
///
/// `generated` 取**入口同级**的生成物路径；无可用入口（`Absent` / `Invalid`）留空——
/// 生成物路径是入口的函数，无入口即无路径可写。
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
        // 降级计数与取值校验同源（`schema::degraded_marks` 单点全深度扫描）：口径是
        // **标记出现次数**，不是 `$defs` 条目数——真实降级落点在属性位。
        let degraded = schema_value
            .map(|value| schema::degraded_marks(value).len())
            .unwrap_or(0);
        let generated_rel = generated_path_of(root, elements, element)
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
/// 每个符号附 `visibility` 与**定义文件与行**的注释（`ApiSymbol.file` / `line`，不是入口
/// 串——闭包一跳后行号属于另一个文件）；构造器条目不进草稿（构造器由生成器产出）。
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
    let state = paths::entry_state(root, &elements, found);
    let paths::EntryState::Resolved { entry, .. } = state else {
        let candidates = paths::entry_candidates(found);
        // 理由与 `check::entry_missing_message` 同一套判据（同一 `EntryProblem`）：判据面出
        // 完整 finding 文案，本处只补「为什么没有入口」那一句
        let reason = match &state {
            paths::EntryState::Invalid(problem) => {
                format!("声明的 `entry` {}", entry_problem_reason(problem))
            }
            _ => "未声明 `entry`".to_owned(),
        };
        let expected = if candidates.is_empty() {
            "（`path` 为空或形态非法，无候选可派生）".to_owned()
        } else {
            candidates
                .iter()
                .map(|candidate| format!("`{candidate}`"))
                .collect::<Vec<String>>()
                .join(" / ")
        };
        return Err(ApiError::Element(format!(
            "元素 {element} 无可用入口（{reason}；约定的候选 {expected} 皆不在盘）——\
             无法起草契约"
        )));
    };
    // 取数面与判据的「代码导出集」同源（同一元素的公开面散在入口与它引用的各文件里）；
    // 只读入口会把真正的操作漏掉，草稿随即失真
    let symbols = extract::extract_element(root, &elements, found, &entry);
    Ok(bootstrap_text(element, &symbols))
}

/// `EntryProblem` 的一句话理由（「为什么这个入口不可用」）——[`bootstrap`] 失败文案的
/// 素材，与 `check::entry_missing_message` 同源（同一 `EntryProblem` 四值，零第二份判据）。
fn entry_problem_reason(problem: &paths::EntryProblem) -> String {
    match problem {
        paths::EntryProblem::Malformed { entry } => {
            format!("`{entry}` 形态非法（MUST 是 repo 相对 POSIX 路径）")
        }
        paths::EntryProblem::OutsidePath { entry } => {
            format!("`{entry}` 落在该元素的 `path` 之外")
        }
        paths::EntryProblem::NotOnDisk { entry } => format!("`{entry}` 不在盘上"),
        paths::EntryProblem::Duplicated { entry, other_fqn } => {
            format!("`{entry}` 与元素 {other_fqn} 是同一个文件")
        }
    }
}

/// md 骨架文本（H1 + 空散文段 + 每操作一个 `### <op-id>` + `api-types` fence）。
///
/// 注记行 `<!-- <visibility> @ <file>:<line> -->` 取每个符号的**定义文件与行**
/// （`ApiSymbol.file` / `line`），入口退出注记面——闭包一跳之后行号属于另一个文件，
/// 用入口串拼它就等于注解说谎。
fn bootstrap_text(fqn: &str, symbols: &[ApiSymbol]) -> String {
    let mut text =
        format!("# {fqn}\n\n## 职能\n\n（待补：一句职能）\n\n## 不变量\n\n（待补）\n\n## 操作\n\n");
    for symbol in symbols
        .iter()
        .filter(|symbol| symbol.kind == ApiSymbolKind::Function)
        .filter(|symbol| !contract::is_constructor(&symbol.name))
    {
        text.push_str(&format!(
            "### {}\n\n<!-- {} @ {}:{}{} -->\n\n",
            symbol.name,
            symbol.visibility.as_str(),
            symbol.file,
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
///
/// `待剪枝` 注记同样取符号的**定义文件与行**（`ApiSymbol.file`）。
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
                symbol.file,
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
/// 模型结构坏（[`elements_of`] 的停摆态）⇒ 取不到元素表，按「元素缺席」投影：契约 /
/// schema 路径仍报文件事实，生成物路径退 `None`（本入口无错误面）。
pub fn api_ref(root: &Path, fqn: &str) -> ArchiApiRef {
    let elements = elements_of(root).unwrap_or_default();
    match elements.iter().find(|candidate| candidate.fqn == fqn) {
        Some(element) => api_ref_of(root, &elements, element),
        None => ref_projection(root, fqn, None),
    }
}

/// 同一投影的元素形态入口（消费方已有元素表时用——入口解析需**全表**判跨元素唯一，
/// 投影层不必为拿一条路径而二次读模型）。
pub fn api_ref_of(root: &Path, elements: &[ArchiElement], element: &ArchiElement) -> ArchiApiRef {
    let entry = match paths::entry_state(root, elements, element) {
        paths::EntryState::Resolved { entry, .. } => Some(entry),
        _ => None,
    };
    ref_projection(root, &element.fqn, entry.as_deref())
}

/// 投影面唯一装配体：`entry = None` 同时表达「元素缺席」与「该元素无可用入口」两态
/// （MUST NOT 为凑参数构造占位元素），契约 / schema 路径仍报文件事实。
fn ref_projection(root: &Path, fqn: &str, entry: Option<&str>) -> ArchiApiRef {
    let contract = paths::contract_path(root, fqn).filter(|path| path.is_file());
    let schema = paths::schema_path(root, fqn);
    let generated = entry.and_then(|entry| {
        paths::generated_path(
            root,
            entry,
            paths::generated_kind(paths::language_of(entry)),
        )
    });
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

/// 元素生成物路径（入口解析 → 语言分派 → 落点派生；无可用入口 → `None`）。
///
/// 语言 → 生成物种类经 [`paths::generated_kind`] 单点分派、**输入改为入口**，本函数零
/// 第二份 match；`paths::generated_path` 只收入口串——「容器元素天然无生成物」由
/// [`paths::entry_state`] 的 `Absent` 表达，MUST NOT 为凑参数构造占位 `ArchiElement`。
fn generated_path_of(
    root: &Path,
    elements: &[ArchiElement],
    element: &ArchiElement,
) -> Option<PathBuf> {
    let paths::EntryState::Resolved { entry, .. } = paths::entry_state(root, elements, element)
    else {
        return None;
    };
    let kind = paths::generated_kind(paths::language_of(&entry));
    paths::generated_path(root, &entry, kind)
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

/// 契约资产与契约索引两处有界枚举共用的深度上限（分层目录深度随 FQN 深度增长，8 层
/// 对「`Frontend/Components/Agent/` 级」留足余量；本值只防环，不是布局判据）。
const MAX_API_WALK_DEPTH: usize = 8;

/// `$ref` 的 `$defs` 片段（[`ref_name`] 的切分点，schema 装配与两渲染器同源）。
pub(crate) const DEFS_FRAGMENT: &str = "#/$defs/";

/// schema 生成物后缀（[`reexport_target`] 的切分点，与 [`paths::schema_path`] 同源）。
pub(crate) const SCHEMA_FILE_SUFFIX: &str = ".schema.json";

#[cfg(test)]
mod check_test;
#[cfg(test)]
mod contract_test;
#[cfg(test)]
mod diff_test;
#[cfg(test)]
mod mod_test;
#[cfg(test)]
mod paths_test;
#[cfg(test)]
mod render_test;
#[cfg(test)]
mod schema_test;
