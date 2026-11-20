use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::drift::report::ArchiDriftOccurrence;
use crate::graph::ModelGraph;
use crate::model::{Diagnostic, Element, Relation};
use crate::parse;
use crate::{diff, search};

// ---------------------------------------------------------------------------
// 封闭词汇（九枚举，词汇单点）
// ---------------------------------------------------------------------------

/// 元素种类（封闭四值；线格式小写词）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ElementKind {
    /// 包级（顶层分组）
    Package,
    /// 域级（业务域）
    Domain,
    /// 模块级
    Module,
    /// 组件级
    Component,
}

impl ElementKind {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            ElementKind::Package => "package",
            ElementKind::Domain => "domain",
            ElementKind::Module => "module",
            ElementKind::Component => "component",
        }
    }

    /// 全部合法值（清单单点：工具描述从同一份词表派生，不另抄一份字面量）。
    pub const ALL: [ElementKind; 4] = [
        ElementKind::Package,
        ElementKind::Domain,
        ElementKind::Module,
        ElementKind::Component,
    ];
}

/// gate 档位（封闭三值；缺省 `Off`——新元素缺省不收紧的 ratchet 起点）。
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "lowercase")]
pub enum GateTier {
    /// 不参与门禁（缺省）
    #[default]
    Off,
    /// 违例仅告警
    Warn,
    /// 违例即门禁失败
    Strict,
}

impl GateTier {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            GateTier::Off => "off",
            GateTier::Warn => "warn",
            GateTier::Strict => "strict",
        }
    }

    /// 全部合法值（清单单点：工具描述从同一份词表派生，不另抄一份字面量）。
    pub const ALL: [GateTier; 3] = [GateTier::Off, GateTier::Warn, GateTier::Strict];
}

/// 契约状态指针（封闭二值；缺省 `Declared`）。
///
/// 语义：`declared` = 本模块承诺有公共 API 契约（缺省）、`none` = 显式声明本模块
/// 无 API。它是元素的**属性**而非协议正文——签名 / 符号 / 接口定义 MUST NOT 出现在
/// 模型里（契约权威在架构域 `models/<分层目录…>/` 的契约资产，本枚举只是状态指针）。
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "lowercase")]
pub enum ApiStatus {
    /// 承诺有契约（缺省）
    #[default]
    Declared,
    /// 显式声明本模块无 API
    None,
}

impl ApiStatus {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            ApiStatus::Declared => "declared",
            ApiStatus::None => "none",
        }
    }

    /// 全部合法值（清单单点：全量常量 = 知识文本与四面词汇对拍的唯一派生源）。新增
    /// 变体必须同补本常量（长度即红灯），MUST NOT 在消费侧手抄一份词表字面量。
    pub const ALL: [ApiStatus; 2] = [ApiStatus::Declared, ApiStatus::None];
}

/// 结构诊断种类（封闭九值：parser 免费五 + 自实现四；线格式 camelCase）。
///
/// 类型名带 `Archi` 前缀是**线面约束**：specta 的类型注册表按裸类型名唯一，工作区
/// 配置侧已有同名的 `DiagnosticKind`（两侧同名为注册冲突），故本枚举的导出名带
/// 前缀；进程内仍以 [`DiagnosticKind`] 别名引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ArchiDiagnosticKind {
    /// 非法 TOML（漏引号 / 漏闭合）
    SyntaxError,
    /// 缺必填字段
    MissingField,
    /// 值不在封闭值域（错误里列出全部合法值）
    InvalidValue,
    /// 未知字段（拼错的键）
    UnknownField,
    /// 重复声明（同文件 + 跨文件）
    DuplicateElement,
    /// 成环
    Cycle,
    /// 依赖自己的祖先
    DependsOnAncestor,
    /// 关系引用不存在的元素
    UnknownReference,
    /// 子元素已声明而祖先未声明
    OrphanHierarchy,
}

/// 进程内词汇名（线面导出名为 `ArchiDiagnosticKind`，见其 doc）。
pub use ArchiDiagnosticKind as DiagnosticKind;

impl ArchiDiagnosticKind {
    /// 线格式 camelCase 词。
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticKind::SyntaxError => "syntaxError",
            DiagnosticKind::MissingField => "missingField",
            DiagnosticKind::InvalidValue => "invalidValue",
            DiagnosticKind::UnknownField => "unknownField",
            DiagnosticKind::DuplicateElement => "duplicateElement",
            DiagnosticKind::Cycle => "cycle",
            DiagnosticKind::DependsOnAncestor => "dependsOnAncestor",
            DiagnosticKind::UnknownReference => "unknownReference",
            DiagnosticKind::OrphanHierarchy => "orphanHierarchy",
        }
    }

    /// 全部合法值（清单单点：全量常量 = 知识文本与语料对拍的唯一派生源）。新增
    /// 变体必须同补本常量（长度即红灯），MUST NOT 在消费侧手抄一份词表字面量。
    pub const ALL: [DiagnosticKind; 9] = [
        DiagnosticKind::SyntaxError,
        DiagnosticKind::MissingField,
        DiagnosticKind::InvalidValue,
        DiagnosticKind::UnknownField,
        DiagnosticKind::DuplicateElement,
        DiagnosticKind::Cycle,
        DiagnosticKind::DependsOnAncestor,
        DiagnosticKind::UnknownReference,
        DiagnosticKind::OrphanHierarchy,
    ];
}

/// 边标记（封闭四值；本期生产者恒 `None`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EdgeMark {
    /// 无标记（实线）
    None,
    /// 声明未兑现（灰虚线）
    Unfulfilled,
    /// 跨种类向上（橙虚线）
    CrossKind,
    /// 豁免（灰实线）
    Exempt,
}

/// 节点标记（封闭两值；结构错不入标记——结构错一律停摆）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NodeMark {
    /// 正常（⚪）
    None,
    /// 存在未兑现声明（🟡）
    Unfulfilled,
}

/// 文件树条目的形态（封闭二值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum FileEntryKind {
    /// 目录（枚举文件的祖先目录 / 模块根目录 / 缺失的声明根）
    Dir,
    /// 文件（枚举出的源文件 / 单文件形模块根）
    File,
}

impl FileEntryKind {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            FileEntryKind::Dir => "dir",
            FileEntryKind::File => "file",
        }
    }

    /// 全部合法值（清单单点：新增变体必须同补本常量，长度即红灯）。
    pub const ALL: [FileEntryKind; 2] = [FileEntryKind::Dir, FileEntryKind::File];
}

/// 文件树节点的归属标记（封闭五值；优先级单值
/// `elementRoot > ambiguous > member > containsElements > none`）。
///
/// 与画布节点标记 [`NodeMark`] **同名不同义、MUST NOT 复用**：后者归画布节点、生命
/// 周期在漂移报告；本词表只表达归属，MUST NOT 承载漂移违例 / 门禁 / 契约状态，且
/// MUST NOT 进「四面词汇对拍」（那四面锚定模型 / 报告词汇，另一条生命周期）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FileTreeMark {
    /// 本节点恰是一个元素的 `path` 根（`owners` 恰 1）
    ElementRoot,
    /// 同长多命中 ≥ 2 个元素（含同一归一路径被多元素声明、以及同长不同前缀两种来源）
    Ambiguous,
    /// 位于某元素 `path` 的严格内部
    Member,
    /// 自身无归属，但后代含元素根
    ContainsElements,
    /// 以上都不成立
    None,
}

impl FileTreeMark {
    /// 线格式 camelCase 词。
    pub fn as_str(self) -> &'static str {
        match self {
            FileTreeMark::ElementRoot => "elementRoot",
            FileTreeMark::Ambiguous => "ambiguous",
            FileTreeMark::Member => "member",
            FileTreeMark::ContainsElements => "containsElements",
            FileTreeMark::None => "none",
        }
    }

    /// 全部合法值（清单单点：新增变体必须同补本常量，长度即红灯）。
    pub const ALL: [FileTreeMark; 5] = [
        FileTreeMark::ElementRoot,
        FileTreeMark::Ambiguous,
        FileTreeMark::Member,
        FileTreeMark::ContainsElements,
        FileTreeMark::None,
    ];
}

/// 检索范围（封闭三值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SearchScope {
    /// 只搜元素
    Elements,
    /// 只搜入口文档路径
    Doc,
    /// 两者皆搜
    All,
}

impl SearchScope {
    /// 线格式词 → 枚举（其余值 `Err` 并列出合法值）。
    pub fn parse(raw: &str) -> Result<SearchScope, String> {
        match raw {
            "elements" => Ok(SearchScope::Elements),
            "doc" => Ok(SearchScope::Doc),
            "all" => Ok(SearchScope::All),
            other => Err(format!(
                "非法 scope: {other}（合法值：elements / doc / all）"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// 线面 DTO（specta::Type → bindings.ts）
// ---------------------------------------------------------------------------

/// 契约状态的线面投影（并列字段，**投影只报文件在案事实**）：
///
/// - `status` = 契约文件是否在案（`declared` / `none`）；
/// - `none` 时三个路径字段为 `null`；在案时三者为 repo 相对 POSIX 路径（`schema`
///   与生成物即使尚未生成也给路径串）。
///
/// 该字段 MUST NOT 回写模型字段（模型存的是意图，投影报的是事实——「`declared`
/// 而文件缺失」是迁移期主形态，不得被投影抹平），MUST NOT 承载协议正文。
/// `declared` 元素的 `contract_path` 同时是它的**入口文档**路径（模型 `doc` 字段对它
/// 不可写）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiApiRef {
    /// 契约状态（文件在案事实）
    pub status: ApiStatus,
    /// 契约文件（`<archi_models_dir>/<分层目录…>/<FQN>.md`）的 repo 相对路径
    pub contract_path: Option<String>,
    /// schema 生成物（同目录 `<FQN>.schema.json`）的 repo 相对路径
    pub schema_path: Option<String>,
    /// 元素生成物（实现入口同级的 `<入口主干>.i.rs` / `<入口主干>.i.ts`）的 repo 相对
    /// 路径；无可用入口（`Absent` / `Invalid`）⇒ `null`
    pub generated_path: Option<String>,
}

/// 画布数据（停摆判据 = `diagnostics` 非空；停摆态 `elements` / `relations` 恒空）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiGraph {
    /// 元素（FQN 字典序；停摆态为空数组）
    pub elements: Vec<ArchiElement>,
    /// 关系（`(from, to)` 字典序；停摆态为空数组）
    pub relations: Vec<ArchiRelation>,
    /// 结构诊断（非空即停摆）
    pub diagnostics: Vec<ArchiDiagnostic>,
}

/// 文件树条目（七字段按声明序出线；扁平、无嵌套）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiFileEntry {
    /// repo 相对 POSIX 路径（无前导 `./`、无尾 `/`；顶层条目即其单段名）
    pub path: String,
    /// 末段名（无 `/` 时等于 `path`）
    pub name: String,
    /// 形态（目录 / 文件）
    pub kind: FileEntryKind,
    /// 归属标记（五值单值）
    pub mark: FileTreeMark,
    /// 归属 FQN（字典序去重）：`elementRoot` 恰 1 / `ambiguous` ≥ 2 / 其余空数组
    pub owners: Vec<String>,
    /// 后代模块数（严格前缀、按 FQN 去重、不含自身）；`kind = File` 恒 0
    pub contains: u32,
    /// `Dir` 且为模块根时该目录是否在盘（其余恒 `true`）
    pub exists: bool,
}

/// 文件树数据（扁平条目数组、`path` 字节序、确定性；不产 `""` 根条目）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiFileTree {
    /// 条目（扁平：嵌套关系由前端按 `path` 前缀派生）
    pub entries: Vec<ArchiFileEntry>,
}

/// 生成载元的出线形态（`ArchiApiCheck.meta`）。
///
/// 只两项：`ApiMeta` 的 `schemaUri` / `generatedNotice` / `common` 规则表 MUST NOT
/// 出线（前端不消费，出线等于把内部载元变成协议面）。`commonDigest` 出**全量**
/// `sha256:<hex>`——截断只发生在生成物文本（`api::short_digest`），面板自行截断显示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiApiMeta {
    /// 生成器 crate 版本（`CARGO_PKG_VERSION`）
    pub tool_version: String,
    /// 映射规则资产摘要（全量 `sha256:<hex>`）
    pub common_digest: String,
}

/// 契约生成物新鲜度的出线事实（`archi_api_check` 出参）。
///
/// 三类差异合起来恰是「点生成会写什么」的完整清单；三列元素为 repo 相对 POSIX
/// 串（经 `api::display` 单点产出，MUST NOT 在命令层或前端另归一）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiApiCheck {
    /// 契约面是否启用（`api::enabled` 单点判定）
    pub enabled: bool,
    /// 生成载元（`api::meta()` 投影）
    pub meta: ArchiApiMeta,
    /// 内容不符（文件在盘但字节与渲染结果不同）
    pub mismatched: Vec<String>,
    /// 应删未删（盘上存在、命名符合生成物判据、不在应生成集）
    pub orphaned: Vec<String>,
    /// 应生成未生成（`enabled = false` 时通常只 `INDEX.toml` 一条）
    pub missing: Vec<String>,
}

/// 生成物写盘结果（`archi_api_generate` 出参）。
///
/// `written` = 本次实际写入（新创建或内容变化）；`unchanged` = 应生成且盘上已一致
/// （本次零写入、mtime 不动）。两者不交叠；生成从不删文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiApiWriteOutcome {
    /// 本次实际写入的生成物（repo 相对 POSIX 串）
    pub written: Vec<String>,
    /// 应生成且盘上已一致的生成物（repo 相对 POSIX 串）
    pub unchanged: Vec<String>,
}

/// 元素投影（`modelFile` / `modelLine` 指向声明表头，可直接 `edit`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiElement {
    /// 完整限定名
    pub fqn: String,
    /// 元素种类
    pub kind: ElementKind,
    /// gate 档位
    pub gate: GateTier,
    /// 覆盖路径（原文，不归一）
    pub path: Vec<String>,
    /// 一行摘要
    pub summary: Option<String>,
    /// 自由元数据表
    pub metadata: BTreeMap<String, String>,
    /// 声明所在文件（repo 相对）
    pub model_file: String,
    /// 声明所在行（1 起始）
    pub model_line: u32,
    /// 节点标记（本期恒 `none`）
    pub mark: NodeMark,
    /// 契约状态指针（模型字段的拷贝；协议正文不进模型）
    pub api: ApiStatus,
    /// 实现入口指针（模型字段的拷贝；线格式 `entry`，缺省 `null`，**显式字段不省略**
    /// ——与「投影不补默认值」同款）
    pub entry: Option<String>,
    /// 入口文档指针（模型字段的拷贝；单值，仅 `api = none` 元素有值；`declared` 的
    /// 入口文档由派生 `contract_path` 给出）
    pub doc: Option<String>,
}

/// 关系投影（`reason` 本期恒 `null`，`exempt` 场景的呈现载体）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiRelation {
    /// 起点元素 FQN
    pub from: String,
    /// 终点元素 FQN
    pub to: String,
    /// 关系说明
    pub description: Option<String>,
    /// 边标记（本期恒 `none`）
    pub mark: EdgeMark,
    /// 豁免理由（本期恒 `null`）
    pub reason: Option<String>,
    /// 依赖 API 名清单（模型字段的投影拷贝、**字段不省略**、顺序按模型原文；空数组
    /// = 未声明 API 粒度）。画布据此在点开前呈现声明清单，零额外取数
    pub apis: Vec<String>,
}

/// 单条关系的详情投影（`archi_relation` 的出参）。
///
/// 模型面（`from` / `to` / `description` / `mark` / `reason` / `modelFile` /
/// `modelLine` / `declared`）与事实面（`used`）+ 两桶（`usedNotDeclared` /
/// `declaredNotUsed`）+ 命中点（`occurrences`，复用 [`ArchiDriftOccurrence`]）。
/// 两桶由 `drift::api_buckets` 求差（**同一纯函数**，命令层零领域规则）；
/// **不是一次检查**：不出 `status` / `passed`、不组装报告、零缓存、零写盘。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiRelationDetail {
    /// 起点元素 FQN
    pub from: String,
    /// 终点元素 FQN
    pub to: String,
    /// 关系说明
    pub description: Option<String>,
    /// 边标记（与 [`ArchiRelation::mark`] 同源）
    pub mark: EdgeMark,
    /// 豁免理由（与 [`ArchiRelation::reason`] 同源）
    pub reason: Option<String>,
    /// 该 `[[relation]]` 声明所在文件（repo 相对）
    pub model_file: String,
    /// 声明所在行（`from` 键行，1 起始）
    pub model_line: u32,
    /// 模型 `apis`（空 = 未声明 API 粒度）
    pub declared: Vec<String>,
    /// 扫描事实名（该元素对命中边的名字并集，去重 + 字典序）
    pub used: Vec<String>,
    /// 用了没声明（`used ∖ declared`）
    pub used_not_declared: Vec<String>,
    /// 声明没用（`declared ∖ used`；两种成因不可机械分辨，呈现时须并列）
    pub declared_not_used: Vec<String>,
    /// 命中点（同一批边，按文件 / 行排序）
    pub occurrences: Vec<ArchiDriftOccurrence>,
}

/// 结构诊断投影（`line == 0` 表示位置未知）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDiagnostic {
    /// 诊断种类
    pub kind: DiagnosticKind,
    /// 原文文案（parser 面原样含行 / 列 / 片段 / caret）
    pub message: String,
    /// 所在文件（repo 相对）
    pub file: String,
    /// 所在行（1 起始；0 = 位置未知）
    pub line: u32,
    /// 所在列（1 起始；未知即 `null`）
    pub column: Option<u32>,
}

/// 元素薄概览（定位工具省略元素时的全量形态 / 检索条目）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiElementBrief {
    /// 完整限定名
    pub fqn: String,
    /// 元素种类
    pub kind: ElementKind,
    /// gate 档位
    pub gate: GateTier,
    /// 契约状态指针（模型字段的拷贝）
    pub api: ApiStatus,
    /// 一行摘要
    pub summary: Option<String>,
    /// `path` 面是否有值（`path` 数组非空；纯投影——不补默认值、不替模型兜底）。
    /// 字段不省略：一次无参调用即可判出「谁缺 `path`」。
    pub has_path: bool,
}

/// 入口解析的来源（封闭二值）。
///
/// 约定默认与显式命中之外无第三来源，MUST NOT 增设第三值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EntryVia {
    /// 模型显式声明的 `entry`
    Explicit,
    /// 约定默认候选命中
    Convention,
}

/// 入口解析结果（`resolvedEntry`；三字段皆不省略）。
///
/// 判据单点在 `api::paths::entry_state`；`EntryForm`（crate / subdir / single-file）
/// 不出线——本类型只回答「入口在哪、凭什么判定」，形态位无消费方。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiResolvedEntry {
    /// repo 相对 POSIX 入口路径（不可用 / 不存在时 `null`）
    pub path: Option<String>,
    /// 解析来源（`explicit` / `convention`；不可用 / 不存在时 `null`）
    pub via: Option<EntryVia>,
    /// 不可用或缺席的人读成因（可用时 `null`）
    pub problem: Option<String>,
}

/// 出边对端（`dependsOn` 条目）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDependsOn {
    /// 被依赖元素 FQN
    pub target: String,
    /// 关系说明
    pub description: Option<String>,
}

/// 入边对端（`dependedBy` 条目）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDependedBy {
    /// 依赖方元素 FQN
    pub source: String,
    /// 关系说明
    pub description: Option<String>,
}

/// 单元素详情（定位工具的响应面；与 IPC `archi_module` 同一投影）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiModuleDetail {
    /// 元素本体
    pub element: ArchiElement,
    /// 出边
    pub depends_on: Vec<ArchiDependsOn>,
    /// 入边
    pub depended_by: Vec<ArchiDependedBy>,
    /// 直接子元素 FQN（字典序）
    pub children: Vec<String>,
    /// 入口文档指针：单值、仅 `api = none` 元素有值；`declared` 恒 `null`，其入口文档
    /// 走 `api.contractPath`
    pub doc: Option<String>,
    /// 契约状态与三路径的并列字段
    pub api: ArchiApiRef,
    /// 入口解析结果（显式命中 / 约定默认命中 / 不可用成因）
    pub resolved_entry: ArchiResolvedEntry,
}

/// 检索结果（薄条目 + 入口文档路径串）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiSearchResult {
    /// 命中元素（FQN 字典序）
    pub elements: Vec<ArchiElementBrief>,
    /// 命中的入口文档路径（repo 相对 POSIX，路径字典序）
    pub doc: Vec<String>,
}

/// gate 变更条目（`from` → `to`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiGateChange {
    /// 元素 FQN
    pub fqn: String,
    /// 旧档位
    pub from: GateTier,
    /// 新档位
    pub to: GateTier,
}

/// 模型结构 diff 摘要（归档确认面「架构变更」区块的数据源）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiChangeSummary {
    /// 新增元素 FQN（字典序）
    pub elements_added: Vec<String>,
    /// 删除元素 FQN（字典序）
    pub elements_removed: Vec<String>,
    /// 新增关系数
    pub relations_added: u32,
    /// 删除关系数
    pub relations_removed: u32,
    /// gate 变更清单
    pub gate_changes: Vec<ArchiGateChange>,
    /// 改动侧结构诊断（红字面；不阻断确认）
    pub diagnostics: Vec<ArchiDiagnostic>,
}

// ---------------------------------------------------------------------------
// 投影管线（单点构建 + 零改写映射）
// ---------------------------------------------------------------------------

/// 已加载模型：图（唯一构建点产物）+ 合并后的诊断（解析免费五类 ++ 图自实现四类）。
///
/// `pub(crate)`：`relation` 的按需取数复用同一份装载（MUST NOT 第二份 parse → build）。
pub(crate) struct Loaded {
    pub(crate) graph: ModelGraph,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

/// 读盘 → 建图 → 诊断合并（本 crate 内 `ModelGraph::build` 的唯一调用点）。
pub(crate) fn load(root: &Path) -> Loaded {
    let outcome = parse::parse_dir(root);
    let parse_diagnostics = outcome.diagnostics;
    let graph = ModelGraph::build(outcome.model);
    let mut diagnostics = parse_diagnostics;
    diagnostics.extend(graph.diagnostics());
    Loaded { graph, diagnostics }
}

/// 元素投影（只裁剪）。`pub(crate)`：检索面的载体派生复用同一份投影（MUST NOT 另拼
/// 第二份 `ArchiElement`）。
pub(crate) fn to_element(element: &Element) -> ArchiElement {
    ArchiElement {
        fqn: element.fqn.clone(),
        kind: element.kind,
        gate: element.gate,
        path: element.path.clone(),
        summary: element.summary.clone(),
        metadata: element.metadata.clone(),
        model_file: element.file.clone(),
        model_line: element.line,
        mark: NodeMark::None,
        api: element.api,
        entry: element.entry.clone(),
        doc: element.doc.clone(),
    }
}

/// 关系投影（只裁剪；标记与豁免理由本期恒空）。
fn to_relation(relation: &Relation) -> ArchiRelation {
    ArchiRelation {
        from: relation.from.clone(),
        to: relation.to.clone(),
        description: relation.description.clone(),
        mark: EdgeMark::None,
        reason: None,
        apis: relation.apis.clone(),
    }
}

/// 诊断投影（只裁剪）。
fn to_diagnostic(diagnostic: &Diagnostic) -> ArchiDiagnostic {
    ArchiDiagnostic {
        kind: diagnostic.kind,
        message: diagnostic.message.clone(),
        file: diagnostic.file.clone(),
        line: diagnostic.line,
        column: diagnostic.column,
    }
}

/// 薄概览投影（只裁剪）。
fn to_brief(element: &Element) -> ArchiElementBrief {
    ArchiElementBrief {
        fqn: element.fqn.clone(),
        kind: element.kind,
        gate: element.gate,
        api: element.api,
        summary: element.summary.clone(),
        has_path: !element.path.is_empty(),
    }
}

/// 诊断清单 → 单条错误文案（`kind @ file:line` 逐行；使调用方一眼看出「模型
/// 本身坏了」而不是「没有这个模块」）。
///
/// `pub(crate)`：`relation` 的 `Err` 复用同一文案单点。
pub(crate) fn diagnostics_error(diagnostics: &[Diagnostic]) -> String {
    diagnostics_lines(
        diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.kind, diagnostic.file.as_str(), diagnostic.line)),
    )
}

/// 同一文案的投影侧入口（`api` 面经 [`graph_view`] 取元素时的错误面、agent 工具
/// `archi_api` 的结构错回灌）——文案单点在 [`diagnostics_lines`]，MUST NOT 在调用方
/// 另拼一份。
pub fn graph_diagnostics_error(diagnostics: &[ArchiDiagnostic]) -> String {
    diagnostics_lines(
        diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.kind, diagnostic.file.as_str(), diagnostic.line)),
    )
}

/// 两族诊断类型（进程内 / 线面投影）共用的文案形。
fn diagnostics_lines<'a>(items: impl Iterator<Item = (DiagnosticKind, &'a str, u32)>) -> String {
    let mut lines: Vec<String> = items
        .map(|(kind, file, line)| {
            let kind = kind.as_str();
            if line == 0 {
                format!("{kind} @ {file}")
            } else {
                format!("{kind} @ {file}:{line}")
            }
        })
        .collect();
    lines.insert(0, format!("模型结构错（共 {} 条）：", lines.len()));
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// 六个投影入口（三消费面同源）
// ---------------------------------------------------------------------------

/// 画布数据单点：无结构诊断 → 完整图（元素 FQN 序、关系 `(from, to)` 序、标记
/// 恒 `none`）；有任一结构诊断 → 停摆态（`elements` / `relations` 空 + 诊断全量）。
pub fn graph_view(root: &Path) -> Result<ArchiGraph, String> {
    let loaded = load(root);
    if !loaded.diagnostics.is_empty() {
        return Ok(ArchiGraph {
            elements: Vec::new(),
            relations: Vec::new(),
            diagnostics: loaded.diagnostics.iter().map(to_diagnostic).collect(),
        });
    }
    let model = loaded.graph.model();
    let mut elements: Vec<ArchiElement> = model.elements.iter().map(to_element).collect();
    elements.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    let mut relations: Vec<ArchiRelation> = model.relations.iter().map(to_relation).collect();
    relations.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then_with(|| left.to.cmp(&right.to))
    });
    Ok(ArchiGraph {
        elements,
        relations,
        diagnostics: Vec::new(),
    })
}

/// 单元素投影：元素 + 邻域（出边 / 入边 / 直接子元素）+ 入口文档单值 + 契约面。
/// 结构错 → `Err`（附诊断清单）；元素缺席 → `Err`（附元素总数提示）。
pub fn module_detail(root: &Path, fqn: &str) -> Result<ArchiModuleDetail, String> {
    let loaded = load(root);
    if !loaded.diagnostics.is_empty() {
        return Err(diagnostics_error(&loaded.diagnostics));
    }
    let element = loaded.graph.model().element(fqn).ok_or_else(|| {
        format!(
            "元素不存在：{fqn}（模型共 {} 个元素；省略 element 参数可看全部元素薄概览）",
            loaded.graph.model().elements.len()
        )
    })?;
    let neighborhood = loaded.graph.neighborhood(fqn);
    // 入口解析需全表（跨元素唯一判据），故投影一次元素表；投影只裁剪、零重算
    let projected: Vec<ArchiElement> = loaded
        .graph
        .model()
        .elements
        .iter()
        .map(to_element)
        .collect();
    let current = projected
        .iter()
        .find(|candidate| candidate.fqn == fqn)
        .expect("元素已在上面查得");
    Ok(ArchiModuleDetail {
        element: to_element(element),
        depends_on: neighborhood
            .depends_on
            .iter()
            .map(|relation| ArchiDependsOn {
                target: relation.to.clone(),
                description: relation.description.clone(),
            })
            .collect(),
        depended_by: neighborhood
            .depended_by
            .iter()
            .map(|relation| ArchiDependedBy {
                source: relation.from.clone(),
                description: relation.description.clone(),
            })
            .collect(),
        children: neighborhood
            .children
            .iter()
            .map(|child| (*child).to_owned())
            .collect(),
        // 入口文档只出线一处：`api = none` 出模型原文，`declared` 恒 `null`（其入口文档
        // 走 `api.contractPath`——两处同时给出会让前端渲两条）
        doc: (element.api == ApiStatus::None)
            .then(|| element.doc.clone())
            .flatten(),
        // 契约状态的定位与投影单点在 `crate::api`（本层零路径拼接、零二次读模型）
        api: crate::api::api_ref_of(root, &projected, current),
        resolved_entry: resolve_entry(root, &projected, current),
    })
}

/// 入口解析结果的投影（判据单点在 `api::paths::entry_state`；本函数只做三态映射）。
fn resolve_entry(
    root: &Path,
    elements: &[ArchiElement],
    element: &ArchiElement,
) -> ArchiResolvedEntry {
    match crate::api::paths::entry_state(root, elements, element) {
        crate::api::paths::EntryState::Resolved { entry, .. } => ArchiResolvedEntry {
            path: Some(entry),
            via: Some(if element.entry.is_some() {
                EntryVia::Explicit
            } else {
                EntryVia::Convention
            }),
            problem: None,
        },
        crate::api::paths::EntryState::Invalid(problem) => ArchiResolvedEntry {
            path: None,
            via: None,
            problem: Some(crate::api::entry_problem_reason(&problem)),
        },
        crate::api::paths::EntryState::Absent => ArchiResolvedEntry {
            path: None,
            via: None,
            problem: Some("未声明 entry，且约定默认候选皆不在盘".to_owned()),
        },
    }
}

/// 全元素薄概览（FQN 字典序；结构错 → `Err`）。
pub fn module_overview(root: &Path) -> Result<Vec<ArchiElementBrief>, String> {
    let loaded = load(root);
    if !loaded.diagnostics.is_empty() {
        return Err(diagnostics_error(&loaded.diagnostics));
    }
    let mut briefs: Vec<ArchiElementBrief> =
        loaded.graph.model().elements.iter().map(to_brief).collect();
    briefs.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(briefs)
}

/// 检索投影（元素薄条目 + 入口文档路径串）。
pub fn search(root: &Path, query: &str, scope: SearchScope) -> Result<ArchiSearchResult, String> {
    let loaded = load(root);
    if !loaded.diagnostics.is_empty() {
        return Err(diagnostics_error(&loaded.diagnostics));
    }
    let outcome = search::search(&loaded.graph, root, query, scope)?;
    let mut elements: Vec<ArchiElementBrief> = outcome
        .element_fqns
        .iter()
        .filter_map(|fqn| loaded.graph.model().element(fqn))
        .map(to_brief)
        .collect();
    elements.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(ArchiSearchResult {
        elements,
        doc: outcome.doc,
    })
}

/// 模型结构 diff 摘要：两侧模型声明目录俱缺席 → `None`；两侧等价（`is_empty` 且
/// 改动侧零诊断）→ `None`（零占位）；否则摘要 + 改动侧诊断随行。
pub fn change_summary(
    main_root: &Path,
    next_root: &Path,
) -> Result<Option<ArchiChangeSummary>, String> {
    let main_models = foundation::layout::archi_graph_model_dir(main_root);
    let next_models = foundation::layout::archi_graph_model_dir(next_root);
    if !main_models.exists() && !next_models.exists() {
        return Ok(None);
    }
    let base = load(main_root);
    let next = load(next_root);
    let model_diff = diff::model_diff(base.graph.model(), next.graph.model());
    if model_diff.is_empty() && next.diagnostics.is_empty() {
        return Ok(None);
    }
    Ok(Some(ArchiChangeSummary {
        elements_added: model_diff.elements_added,
        elements_removed: model_diff.elements_removed,
        relations_added: model_diff.relations_added,
        relations_removed: model_diff.relations_removed,
        gate_changes: model_diff
            .gate_changes
            .iter()
            .map(|change| ArchiGateChange {
                fqn: change.fqn.clone(),
                from: change.from,
                to: change.to,
            })
            .collect(),
        diagnostics: next.diagnostics.iter().map(to_diagnostic).collect(),
    }))
}
