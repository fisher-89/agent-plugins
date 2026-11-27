//! 模块 API 接口面：手写 `<叶段>.schema.json`（唯一凭据）+ 纯散文 `<叶段>.md` →
//! 三类生成物 → 十条判据的判定与装配层。
//!
//! **三资产分工**：拓扑在 `graph/model/*.toml`（谁在谁的里面、谁依赖谁）、接口在同目录
//! `<叶段>.schema.json`（跨过这条边时接触面长什么样，**唯一凭据**）、理由在同目录
//! `<叶段>.md`（纯散文、无结构、读面零解析）。
//!
//! **纯读盘、零写盘、零 spawn**（沿本 crate 既有纪律）：接口校验、`$defs` 规则、索引
//! 装配、声明抽取、判据求值、渲染与比对全在这里。生成物写盘单点在
//! `crates/infra/archi::api_artifacts`（与报告落盘并列的第二处 dumb IO 面），
//! 两条 IPC（检查 / 生成）与 agent 工具同源；本 crate 零写盘
//! 纪律 MUST NOT 反向稀释。写生成物的 agent 工具经 [`ArtifactWriter`] 端口委托
//! 同一写盘单点——**端口是类型，不是实现**，不破「本 crate 只读盘」。
//!
//! **零 md 回落**：读面只从 `.schema.json` 取接口事实，MUST NOT 解析文档正文（历史
//! `api-*` fence 视为普通内容）。接口文件不在盘 ⇒ 该元素「接口未写」（零判据）；在盘但
//! 校验失败 ⇒ [`ApiError`] **停摆**（不产半真报告）。
//!
//! **接口面启用条件（冷启动纪律）**：任一 `path` 非空 ∧ `api = declared` 元素的
//! `<叶段>.schema.json` 在盘 ⇒ 启用；否则判据返回空集且既有漂移语料逐字不变。散落的
//! 与任何元素无关的 `*.md` / `*.schema.json`、以及 `api = none` 元素在盘的 schema 都**不是**
//! 启用证据。
//! 一旦启用即**全部 `path` 非空元素**纳入判据，无白名单、无开关、无部分豁免。
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
use crate::dto::{
    ApiStatus, ArchiApiCheck, ArchiApiMeta, ArchiApiRef, ArchiApiWriteOutcome, ArchiElement,
};

// 出口面只留 `lib.rs` 的逐名 `pub use`（D60）：本文件只 `use` 自己消费的名字。
use contract::Contract;
use emit::{EmitTarget, ModuleRef};
use extract::{ApiSymbol, ApiSymbolKind, ApiType};
use schema::{build_index, ApiFormStatus, IndexEntry};

/// 接口面错误面（封闭三值）：接口定义是判定的输入资产，资产坏了应「检查不执行、不产出
/// 半真报告」（[`crate::drift::check`] 把它映射进既有 `CheckError::Config`，不新增
/// 第四变体）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// 读盘失败（接口定义文件存在但不可读 / 被目录占位）
    Io(String),
    /// 接口定义非法（身份 / 键空间 / 形态 / 镜像 / `$ref` 前缀 / 顶层键 / 名字 / `$defs` 规则）
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

/// 生成物写盘端口（**纯类型**：本 crate 仍零写盘、零 spawn、零 IO，实现由壳层注入）。
///
/// 它只声明「把一批应生成物写到某个 root」这一动作，使 `agent-runtime` 面无需依赖
/// `infra/archi`——写盘单点仍是 `crates/infra/archi::api_artifacts::write_artifacts`，
/// 唯一实现（`src-tauri` 的薄委托）由组合根注入到 agent 工具执行体。
pub trait ArtifactWriter: Send + Sync {
    /// 写一批应生成物（字节相同者计入 `unchanged` 且不重写；只增只改、从不删）。
    fn write(&self, root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>;
}

/// 起草的两段文本（**唯一返回形**，CLI 与 agent 工具两个出口共用同一份内容）。
///
/// 两段都不含段模板：`schema` 是可直接过写闸的接口骨架，`doc` 只有 H1 与一句指引
/// （文档无结构，给段名等于把一条已废除的约定重新推荐）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapDrafts {
    /// 接口骨架（`<叶段>.schema.json` 的初稿；可直接落盘并过写闸）
    pub schema: String,
    /// 文档骨架（`# <FQN>` + 一句指引；零 `##` 段名、零 fence）
    pub doc: String,
}

/// 一次装载的只读快照（接口投影 / 接口原文 / 代码符号集 / 文档存在性）。
///
/// **无「缺接口清单」字段**：`api = declared` 而接口文件缺失在判据面上**零对象**
/// （装载期即跳过该元素，MUST NOT 以别名预铺），故不需要、也 MUST NOT 存一份清单
/// ——同一事实的第二个来源必然漂移。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiSnapshot {
    /// 已通过校验的接口投影（FQN 字典序）
    pub contracts: Vec<Contract>,
    /// FQN → 接口定义原文（`$defs` / 判据 / 渲染的唯一读取面）
    pub schemas: BTreeMap<String, Value>,
    /// FQN → 代码符号集（取数器产出，判据的「代码导出集」一侧）
    pub symbols: BTreeMap<String, Vec<ApiSymbol>>,
    /// FQN → 派生 `<叶段>.md` 是否在盘（与接口装载同批产出；`docMissing` 的唯一来源）
    pub doc_present: BTreeMap<String, bool>,
}

/// 判据求值入参（全部借用；零 clone 大结构）。
pub struct ApiCheckInput<'a> {
    /// 观察根
    pub root: &'a Path,
    /// 模型元素表（`path` 非空者参与判据）
    pub elements: &'a [ArchiElement],
    /// 已装载的接口投影
    pub contracts: &'a [Contract],
    /// FQN → 接口定义原文（`$defs` 只读这里，MUST NOT 并读文档正文）
    pub schemas: &'a BTreeMap<String, Value>,
    /// FQN → 代码符号集
    pub symbols: &'a BTreeMap<String, Vec<ApiSymbol>>,
    /// FQN → 派生 `<叶段>.md` 是否在盘（check 侧唯一读取面：零读盘）
    pub doc_present: &'a BTreeMap<String, bool>,
    /// 接口面是否启用（`false` ⇒ 判据空集）
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

/// 协议名字 → `schema.json` 承载键的**换算单点**（D57 ②）。
///
/// 资产键是协议名字（`Outcome`，与 `ApiType::Outcome` / `#/$defs/Outcome` 同形），
/// 而接口定义里的承载键是小写（`"outcome"`）。两侧渲染器 MUST 经本
/// 函数求承载键，MUST NOT 各写一份键变换；**资产键 MUST NOT 改成小写**。
pub(crate) fn protocol_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// 十条接口判据的**对外唯一入口**：`enabled == false` → 空 `Vec`（接口面未启用）；
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

/// 接口判据装配单点：一次装载 → 由元素表派生启用 → 十条判据（稳定排序）。
///
/// [`crate::drift::check`] 与 agent 工具 `archi_api` 共用本函数，MUST NOT 出现第二份
/// 「接口输入装配」（两份必然在接口文件非法时分叉）。`enabled` 由元素表直判（
/// [`enabled`] 单点），`api::check` 的对外入口仍保留 `enabled == false → 空集` 短路。
pub fn findings(root: &Path, elements: &[ArchiElement]) -> Result<Vec<ArchiApiFinding>, ApiError> {
    let snapshot = load_contracts(root, elements)?;
    Ok(check(ApiCheckInput {
        root,
        elements,
        contracts: &snapshot.contracts,
        schemas: &snapshot.schemas,
        symbols: &snapshot.symbols,
        doc_present: &snapshot.doc_present,
        enabled: enabled(root, elements),
    }))
}

/// `status` 纳管子集筛选单点：口径 = **接口承诺已成立**（`api = declared` ∧ schema 在盘）。
///
/// 入选 = `element` 非空 ∧ `api = declared` ∧ `<叶段>.schema.json` 在盘。**文档在盘与否
/// 退出纳管口径**——`docMissing` 恒 `warn` 且两侧都不成立 ⇒ 永不纳管（否则存量文档债会
/// 把状态词常规化为永久 `warnings`）；`api = none` 元素因其 schema 在盘即违约
/// （`apiUnexpected`、`warn`）⇒ 也不纳管：留白违约是判据面的债，不是「承诺未兑现」。
///
/// 「接口在盘」的判定只在本函数一次性发生（Q9 的锚点），`drift` 侧 MUST NOT 自行判
/// `is_file`，前端 / 命令层 MUST NOT 二次筛选。`element = None` 的条目落选（无归属即
/// 无承诺可对）。
pub fn gating_findings(
    root: &Path,
    elements: &[ArchiElement],
    findings: &[ArchiApiFinding],
) -> Vec<ArchiApiFinding> {
    let gating: BTreeSet<&str> = elements
        .iter()
        .filter(|element| element.api == ApiStatus::Declared)
        .filter(|element| schema_on_disk(root, &element.fqn))
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

/// 接口面启用的唯一判据：任一 `path` 非空 ∧ `api = declared` 元素的
/// `<叶段>.schema.json` 在盘。
///
/// **单点纯函数**：[`artifact_check`]（生成预览）与 [`findings`]（判据面）同调本函数，
/// MUST NOT 从快照反推、MUST NOT 在命令层另判一套。MUST NOT 退化为「`<archi_models_dir>`
/// 递归任意 `*.schema.json`」——散落的无关文件 MUST NOT 构成启用证据；`api = none`
/// 元素在盘的 schema 是违约事实（`apiUnexpected`），同样 MUST NOT 构成启用证据。
pub fn enabled(root: &Path, elements: &[ArchiElement]) -> bool {
    elements
        .iter()
        .filter(|element| !element.path.is_empty())
        .filter(|element| element.api == ApiStatus::Declared)
        .any(|element| schema_on_disk(root, &element.fqn))
}

/// 接口文件是否在盘（形态非法的 FQN 一律按「不在盘」处理——它派生不出路径）。
fn schema_on_disk(root: &Path, fqn: &str) -> bool {
    paths::schema_path(root, fqn).is_some_and(|path| path.is_file())
}

/// 文档事实装配（**唯一装配点**）：派生 `<叶段>.md` 是否在盘，逐元素一次。
///
/// 只回答「在不在」——文档无结构、零解析、零正文读取；`api` 状态与模型 `doc` 字段都
/// 不参与（`docMissing` 单成因）。
fn doc_presence(root: &Path, elements: &[ArchiElement]) -> BTreeMap<String, bool> {
    let mut present: BTreeMap<String, bool> = BTreeMap::new();
    for element in elements.iter().filter(|element| !element.path.is_empty()) {
        let exists = paths::contract_path(root, &element.fqn).is_some_and(|path| path.is_file());
        present.insert(element.fqn.clone(), exists);
    }
    present
}

/// 逐元素装载接口定义：`api = none` / `path` 空跳过（留 `apiUnexpected` 自判）；
/// `<叶段>.schema.json` 不在盘跳过（「接口未写」零判据）；在盘即经
/// [`schema::validate_schema`] 校验并装配 `Contract`。
///
/// **零 md 回落**：文档正文不被读取（历史 `api-*` fence 视为普通内容）。
/// **任一份失败即 `Err`**（MUST NOT 静默跳过坏接口、MUST NOT 返回部分快照）：接口是判定
/// 的输入资产，资产坏了应「检查不执行、不产出半真报告」。
pub fn load_contracts(root: &Path, elements: &[ArchiElement]) -> Result<ApiSnapshot, ApiError> {
    // 映射规则资产形态复核（D54 ②）：与 `meta()` 共用同一解析点；失败即
    // `ApiError::Contract`（静默跳过不允许——形态坏的资产会让渲染器落回硬编码）。
    parse_common(COMMON_JSON)?;
    // 文档事实与接口装载同批产出（`facts` 与函数同名会遮蔽，故先命名清楚）
    let facts = doc_presence(root, elements);
    let mut contracts: Vec<Contract> = Vec::new();
    let mut schemas: BTreeMap<String, Value> = BTreeMap::new();
    let mut symbols: BTreeMap<String, Vec<ApiSymbol>> = BTreeMap::new();

    for element in elements {
        if element.path.is_empty() || element.api == ApiStatus::None {
            continue;
        }
        let file = paths::schema_path(root, &element.fqn);
        let Some(file) = file.filter(|path| path.is_file()) else {
            continue;
        };
        let relative = display(root, &file);
        let source = fs::read_to_string(&file).map_err(|error| {
            ApiError::Io(format!("接口定义文件读取失败（{relative}）：{error}"))
        })?;
        let mut parsed = schema::validate_schema(&element.fqn, &source)?;
        let value: Value = serde_json::from_str(&source)
            .map_err(|error| ApiError::Contract(format!("{relative}: 不是合法 JSON：{error}")))?;
        // `validate_schema` 无路径入参 ⇒ 报告的 `occurrences.file` 须在此覆写为
        // repo 相对路径（口径与其他 occurrence 一致）
        parsed.file = relative;
        schemas.insert(element.fqn.clone(), value);
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
        doc_present: facts,
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

/// 装配三类生成物为内存清单（**零写盘**）：`INDEX.toml` / `<入口主干>.i.rs` /
/// `<入口主干>.i.ts`（后两者落**实现入口同级**）。
///
/// `<叶段>.schema.json` **不在应生成集**：它是手写真相（第二份资产），进应生成集就会让
/// `generate` 覆写手写内容、让生成物检查一改即报 `mismatched`。
///
/// 元素表经 [`elements_of`]（`dto::graph_view` 单点取数）——模型结构性坏 ⇒ `Err`
/// （fail-closed：坏模型不得被固化成入库生成物）。
///
/// 接口无类型无 op 时**跳过** `.i.*`（零 pub 项的生成物会让入口的
/// `pub use api_gen::*;` 触发 `unused_imports`，`clippy -D warnings` 即红）。
pub fn render_all(root: &Path, snapshot: &ApiSnapshot) -> Result<Vec<GeneratedFile>, ApiError> {
    let meta = meta();
    let mut files: Vec<GeneratedFile> = Vec::new();
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
        // 入口形态与落点均取 `entry_state` 的 `Resolved`（D41 / D6）：无可用入口 ⇒ 该接口
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
        let content =
            emit::render_target(&target, contract, schema_value, &meta, &snapshot.schemas)?;
        files.push(GeneratedFile { path, content });
    }
    files.push(GeneratedFile {
        path: paths::index_path(root),
        content: build_index(&index_entries(root, snapshot, &elements), &meta),
    });
    Ok(files)
}

/// `INDEX.toml` 条目（全部 `path` 非空元素各一条；`status` 按**接口文件是否在案**）。
///
/// `present` 直判 `paths::schema_path(...).is_file()`（D-13：不依赖 `contract.is_some()`，
/// 因为 `api = none` 的越界 schema 同样在案）；统计字段取 `snapshot`（不另读盘）。
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
        let present = paths::schema_path(root, &element.fqn).is_some_and(|path| path.is_file());
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
/// 无接口的 `absent` 元素返回空表（[`build_index`] 会整段省略，MUST NOT 输出空表）。
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
/// `enum`（非空且**全 `const` 分支**，即封闭枚举）或 `variant`（其余 union：带负载分支
/// 或空 union——两者都是「线形态未定义」的 union）；标量（`string` / `integer` /
/// `number` / `boolean` 且无 `enum`）→ `scalar`；其余（含 `object`）→ `object`。
///
/// `enum` 类目**仍保留名字、只改触发点**：`enum` 数组已在装载期被
/// [`schema::verify_defs`] 拒收，故旧路径自然归零，`categories` 的键集合不变
/// （`INDEX.toml` 与面板零改动）。
///
/// **MUST NOT 复用判据分类**（`check` 模块里按 kind 打标那个函数）：那是判据结果的
/// 分类（`entry` / `contract` / …），随债数漂移；本函数是接口形状的静态分布，只随
/// 接口改动变化。
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
    if let Some(choices) = union_choices(definition) {
        return if is_closed_enum(choices) {
            "enum"
        } else {
            "variant"
        };
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

/// union 分支集（`oneOf` 优先、`anyOf` 次之；两者皆缺 / 非数组 ⇒ `None`）——union 的
/// **唯一识别点**（`def_shape` 的类目分派与 [`emit::rust`] 的形态判定共读它）。
pub(crate) fn union_choices(definition: &Value) -> Option<&Vec<Value>> {
    definition
        .get("oneOf")
        .or_else(|| definition.get("anyOf"))
        .and_then(Value::as_array)
}

/// 封闭枚举判据（`enum` 类目与 Rust 封闭枚举形态的**唯一触发点**）：union **非空**且
/// **每个分支都是标量 `const`**。
///
/// 空 union 与任一分支带负载都不是封闭词表——前者没有词，后者线形态未定义
/// （`oneOf` 不定义判别位），故一律不是。
pub(crate) fn is_closed_enum(choices: &[Value]) -> bool {
    !choices.is_empty()
        && choices.iter().all(|choice| {
            choice.get("const").is_some_and(|constant| {
                constant.is_string() || constant.is_number() || constant.is_boolean()
            })
        })
}

/// 取数器草稿（**只出字符串、零写盘**）：定位入口 → 读源 → 抽取 → **双草稿**
/// （[`BootstrapDrafts`]：schema 骨架 + doc 骨架）。
///
/// 两段都是「可直接落盘」的骨架，且**都不含任何段模板**——复盘 #1 的成因正是「模型把
/// 六段草稿当规格写」；文档无结构后，草稿再给段名等于把一条已废除的约定重新推荐。
///
/// 失败面（元素不存在 / 无可用入口 / 符号抽取失败）逐字不变。
pub fn bootstrap(root: &Path, element: &str) -> Result<BootstrapDrafts, ApiError> {
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
             无法起草接口定义"
        )));
    };
    // 取数面与判据的「代码导出集」同源（同一元素的公开面散在入口与它引用的各文件里）；
    // 只读入口会把真正的操作漏掉，草稿随即失真
    let symbols = extract::extract_element(root, &elements, found, &entry);
    Ok(BootstrapDrafts {
        schema: schema_draft(element, &symbols),
        doc: doc_draft(element),
    })
}

/// `EntryProblem` 的一句话理由（「为什么这个入口不可用」）——[`bootstrap`] 失败文案的
/// 素材，与 `check::entry_missing_message` 同源（同一 `EntryProblem` 四值，零第二份判据）。
///
/// `dto` 的 `resolvedEntry.problem` 复用本函数，MUST NOT 另抄四值措辞。
pub(crate) fn entry_problem_reason(problem: &paths::EntryProblem) -> String {
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

/// 接口骨架的写者纪律注（`$comment`；人员可读、零判据读者）。
///
/// **`GENERATED_NOTICE` 与写者纪律分家**：前者经 `emit::notice_lines` 进 `.i.rs` / `.i.ts`
/// 头注释，改它即改生成物字节（须与在案生成物同批重生成）。故写者纪律另立一条常量，
/// 只出现在新骨架的 `$comment` 里。
const SCHEMA_WRITER_NOTICE: &str = "本文件是手写接口真相（不是生成物）：导出名 / 签名 / 类型的唯一持有者；`x-function.description` 只写调用方要知道的语义、MUST NOT 复述签名；改完用 `archi_api` 的 `check` 自查";

/// 接口骨架文本（六顶层键、每 op 一项 `x-function`、每类型一项 `$defs` + `properties`
/// 镜像；空符号集 ⇒ 两空对象）。
///
/// **骨架可直接过写闸**（D-14）：identity / 键空间 / 形态白名单 / 镜像 / `$ref` 前缀 /
/// 顶层键 / 复用三闸全满足，故「取草稿 → 原样 `write` 落盘」是一条通路。`description`
/// 落空串（与缺省语义等价）而 MUST NOT 写「（待补）」一类提示语——它会被读者当正文渲染。
/// 透传（`api-reexport` 形态）**不产占位**：透传是设计判断，空条目冒充已声明。
fn schema_draft(fqn: &str, symbols: &[ApiSymbol]) -> String {
    let mut properties: Vec<String> = Vec::new();
    let mut defs: Vec<String> = Vec::new();
    for symbol in symbols
        .iter()
        .filter(|symbol| symbol.kind == ApiSymbolKind::Function)
        .filter(|symbol| !contract::is_constructor(&symbol.name))
    {
        properties.push(format!(
            "    \"{}\": {{\n      \"x-function\": {}\n    }}",
            symbol.name,
            draft_function(symbol)
        ));
    }
    for symbol in symbols
        .iter()
        .filter(|symbol| symbol.kind != ApiSymbolKind::Function)
        .filter(|symbol| !contract::is_constructor(&symbol.name))
    {
        defs.push(format!(
            "    \"{}\": {{\n      \"type\": \"object\",\n      \"description\": \"\"\n    }}",
            symbol.name
        ));
        properties.push(format!(
            "    \"{}\": {{\n      \"$ref\": \"#/$defs/{}\"\n    }}",
            symbol.name, symbol.name
        ));
    }
    format!(
        "{{\n  \"$schema\": \"{schema_uri}\",\n  \"$id\": \"{fqn}{suffix}\",\n  \
         \"title\": \"{fqn}\",\n  \"$comment\": \"{notice}\",\n  \"$defs\": {{{defs}}},\n  \
         \"properties\": {{{properties}}}\n}}\n",
        schema_uri = SCHEMA_URI,
        suffix = SCHEMA_FILE_SUFFIX,
        notice = SCHEMA_WRITER_NOTICE,
        defs = join_objects(&defs),
        properties = join_objects(&properties),
    )
}

/// `properties` / `$defs` 的条目拼装（空集 ⇒ 空对象体内的空串）。
fn join_objects(entries: &[String]) -> String {
    if entries.is_empty() {
        String::new()
    } else {
        format!("\n{}\n  ", entries.join(",\n"))
    }
}

/// 单个操作的 `x-function` 内容（参数 / 返回类型按逻辑类型投影，`description` 落空串）。
fn draft_function(symbol: &ApiSymbol) -> String {
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
        "\"description\": \"\",\n        \"params\": [{}],\n        \"returns\": {}",
        params.join(", "),
        json_type(&symbol.returns)
    );
    if symbol.is_async {
        body.push_str(",\n        \"async\": true");
    }
    format!("{{\n        {body}\n      }}")
}

/// 文档骨架文本：`# <FQN>` + 一句指引（**零段名、零 fence、零作者注**）。
fn doc_draft(fqn: &str) -> String {
    format!("# {fqn}\n\n段落按内容自定，先例见同目录同类元素。\n")
}

/// 逻辑类型 → JSON Schema 片段（粗投影：bootstrap 是草稿，形状由人剪枝）。
///
/// 只产**本文件内**的 `$ref`（`#/$defs/<名>`）：跨元素引用是设计判断，草稿不猜
/// （手拼 `../` 层数少一层会绕过 `apiRefMissing`，故用 `archi_api` 的 `link` 算）。
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

/// 接口状态投影：`status` = **接口文件在案**（`declared` ⟺ `<叶段>.schema.json` 在盘），
/// 三条路径**各自按自身在盘事实出线**。
///
/// **投影只报文件事实，MUST NOT 回写模型字段**——模型存的是意图（`declared` 而文件
/// 缺失是迁移期主形态），抹平会让消费方以为「有接口可读」。
///
/// 旧的全或无投影会把三份资产的存在事实压成一个开关（缺一份则三项皆空）；现在
/// `contractPath`（文档）/ `schemaPath`（接口）/ `generatedPath`（生成物，有可用入口即给
/// 路径串、即便尚未生成）各报各的。
///
/// 模型结构坏（[`elements_of`] 的停摆态）⇒ 取不到元素表，按「元素缺席」投影：文档 /
/// 接口路径仍报文件事实，生成物路径退 `None`（本入口无错误面）。
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
/// （MUST NOT 为凑参数构造占位元素）；三路径各自独立，`status` 只看接口文件。
fn ref_projection(root: &Path, fqn: &str, entry: Option<&str>) -> ArchiApiRef {
    let doc = paths::contract_path(root, fqn).filter(|path| path.is_file());
    let schema = paths::schema_path(root, fqn).filter(|path| path.is_file());
    let generated = entry.and_then(|entry| {
        paths::generated_path(
            root,
            entry,
            paths::generated_kind(paths::language_of(entry)),
        )
    });
    ArchiApiRef {
        status: match schema {
            Some(_) => ApiStatus::Declared,
            None => ApiStatus::None,
        },
        contract_path: doc.map(|path| display(root, &path)),
        schema_path: schema.map(|path| display(root, &path)),
        generated_path: generated.map(|path| display(root, &path)),
    }
}

/// 接口的透传目标查表（键 = 目标元素 FQN，值 = 该元素根的 [`ModuleRef`]，D40）。
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
        .map_err(|_| ApiError::Element("模型读取失败（接口面的元素表来源）".to_owned()))?;
    if !graph.diagnostics.is_empty() {
        return Err(ApiError::Element(crate::dto::graph_diagnostics_error(
            &graph.diagnostics,
        )));
    }
    Ok(graph.elements)
}

/// `$ref` → 类型名（`#/$defs/<名>` 与外部文件形态 `…/<叶段>.schema.json#/$defs/<名>`
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

/// 外部接口定义文件引用 → 目标元素 FQN（`<相对前缀><目标叶段>.schema.json#/$defs/<名>`）；
/// 同文件引用（`#/$defs/<名>`）与无 `.schema.json` 后缀者 → `None`。
///
/// 目标靠**本元素 FQN + 相对前缀**解出（文件名只剩叶段，单看文件名已无从定位祖先链）；
/// 解析单点在 [`paths::ref_target`]。
///
/// **单点**：`check::ref_missing`（跨模块引用检查）与 `emit::{rust, ts}`（透传渲染）
/// 共用本函数。
pub(crate) fn reexport_target(from_fqn: &str, reference: &str) -> Option<String> {
    let file = reference.split_once('#').map(|(file, _)| file)?;
    paths::ref_target(from_fqn, file)
}

/// JSON 值的类型名（错误文案用）。接口校验与索引装配共用同一份词表。
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
/// **跨 crate 唯一单点**（D55）：`archi-runtime` 经 `archi::display` 生成 repo
/// 相对串；其它 crate MUST NOT 再持一份逐字实现。
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
const GENERATED_NOTICE: &str = "本文件由元素的契约 markdown 生成，勿手改：手改将在下次生成时被覆盖，且与契约的差异会被生成物检查判出";

/// 索引侧有界枚举（`diff::api_dir_artifacts`）的深度上限（分层目录深度随 FQN 深度增长，8 层
/// 对「`Frontend/Components/Agent/` 级」留足余量；本值只防环，不是布局判据）。
const MAX_API_WALK_DEPTH: usize = 8;

/// `$ref` 的 `$defs` 片段（[`ref_name`] 的切分点，schema 装配与两渲染器同源）。
pub(crate) const DEFS_FRAGMENT: &str = "#/$defs/";

/// 接口文件后缀（[`reexport_target`] 的切分点，与 [`paths::schema_path`] 同源）。
pub(crate) const SCHEMA_FILE_SUFFIX: &str = ".schema.json";

#[cfg(test)]
mod check_test;
#[cfg(test)]
mod contract_test;
#[cfg(test)]
mod diff_test;
#[cfg(test)]
pub(crate) mod mod_test;
#[cfg(test)]
mod paths_test;
#[cfg(test)]
mod render_test;
#[cfg(test)]
mod schema_test;
