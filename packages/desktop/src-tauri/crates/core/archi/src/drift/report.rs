//! 架构漂移报告结构（四组封闭词汇 + 全部线面 wire 类型）。
//!
//! 纯数据（零 IO、零判定）：词汇与字段面在这里单点定义，判定与装配在
//! [`super::check`]，报告落盘在 `crates/infra/archi`。
//!
//! **词汇封闭且单点**：`status`（五值）、漂移条目 `kind`（四值）、观察项 `kind`
//! （两值）、**契约判据 `kind`（十值）** 各持一份 `ALL` 常量——新增变体必须同补
//! `ALL`（长度即红灯），语料对拍覆盖全部取值。四组词汇 MUST NOT 复用既有 DTO 词汇
//! （`DiagnosticKind` 结构诊断九类 / `EdgeMark` / `NodeMark` 画布标记 /
//! `WarningKind` 文档警告）——混用会让「结构错」「文档警告」「漂移违例」三类在类型
//! 上不可辨。
//!
//! **线面名带 `Archi` 前缀**：specta 的类型注册表按**裸类型名**唯一（`dto.rs` 的
//! `ArchiDiagnosticKind` 先例同约束），故报告类型一律前缀化，短名 `Report` 经
//! `pub use` 别名出线（消费侧仍按 spec Module Contract 的短名引用）。

use serde::{Deserialize, Serialize};

use crate::dto::{ArchiDiagnostic, GateTier};

/// 检查结论状态（封闭五值；线格式 `lowercase` 其四 + `noChanges` 驼峰）。
///
/// 五值语义：`clean`（有扫描、零 errors 零 warnings）/ `errors`（`errors` 非空或
/// 模型结构错）/ `warnings`（零 errors、`warnings` 非空）/ `noChanges`（无文件
/// 可判）/ `skipped`（无模型）。`skipped` 与 `noChanges` MUST NOT 被呈现或消费为
/// 「通过」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ArchiCheckStatus {
    /// 零违例零提示
    Clean,
    /// 存在违例（或模型结构错）
    Errors,
    /// 零违例、存在提示
    Warnings,
    /// 无文件可判（增量变更集内无源文件）
    NoChanges,
    /// 无模型（不检查）
    Skipped,
}

impl ArchiCheckStatus {
    /// 线格式词。
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiCheckStatus::Clean => "clean",
            ArchiCheckStatus::Errors => "errors",
            ArchiCheckStatus::Warnings => "warnings",
            ArchiCheckStatus::NoChanges => "noChanges",
            ArchiCheckStatus::Skipped => "skipped",
        }
    }

    /// 全部合法值（清单单点：新增变体不同步即语料红）。
    pub const ALL: [ArchiCheckStatus; 5] = [
        ArchiCheckStatus::Clean,
        ArchiCheckStatus::Errors,
        ArchiCheckStatus::Warnings,
        ArchiCheckStatus::NoChanges,
        ArchiCheckStatus::Skipped,
    ];
}

/// 检查模式（封闭二值）：`full` = workspace 级全量（不落盘）、`incremental` =
/// change 级增量（只扫变更集，落 `reports/archi`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ArchiDriftMode {
    /// 全量模式
    Full,
    /// 增量模式
    Incremental,
}

impl ArchiDriftMode {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiDriftMode::Full => "full",
            ArchiDriftMode::Incremental => "incremental",
        }
    }

    /// 全部合法值。
    pub const ALL: [ArchiDriftMode; 2] = [ArchiDriftMode::Full, ArchiDriftMode::Incremental];
}

/// 漂移条目种类（封闭四值；线格式 camelCase）：`undeclaredDependency`（未声明
/// 依赖，进 errors / warnings）、`pathNotFound`（元素 path 失效）、`ambiguousOwner`
/// （归属不确定）、`noPath`（元素未声明 path）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ArchiDriftKind {
    /// 代码存在跨元素 import 而模型未声明该关系
    UndeclaredDependency,
    /// 元素 `path` 指向不存在的目录 / 文件
    PathNotFound,
    /// 同一最长前缀命中多个元素（归属不确定）
    AmbiguousOwner,
    /// 元素未声明 `path`（不参与归属）
    NoPath,
}

impl ArchiDriftKind {
    /// 线格式 camelCase 词。
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiDriftKind::UndeclaredDependency => "undeclaredDependency",
            ArchiDriftKind::PathNotFound => "pathNotFound",
            ArchiDriftKind::AmbiguousOwner => "ambiguousOwner",
            ArchiDriftKind::NoPath => "noPath",
        }
    }

    /// 全部合法值。
    pub const ALL: [ArchiDriftKind; 4] = [
        ArchiDriftKind::UndeclaredDependency,
        ArchiDriftKind::PathNotFound,
        ArchiDriftKind::AmbiguousOwner,
        ArchiDriftKind::NoPath,
    ];
}

/// 观察项种类（封闭二值；线格式 camelCase）：`unfulfilledDeclaration`（声明未
/// 兑现）、`unusedDeclaration`（长期未使用——本期不产出，词汇位由 spec 明列）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ArchiObservationKind {
    /// 模型声明 `A -> B` 而代码无该 import
    UnfulfilledDeclaration,
    /// 声明长期未被使用（本期不产出）
    UnusedDeclaration,
}

impl ArchiObservationKind {
    /// 线格式 camelCase 词。
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiObservationKind::UnfulfilledDeclaration => "unfulfilledDeclaration",
            ArchiObservationKind::UnusedDeclaration => "unusedDeclaration",
        }
    }

    /// 全部合法值。
    pub const ALL: [ArchiObservationKind; 2] = [
        ArchiObservationKind::UnfulfilledDeclaration,
        ArchiObservationKind::UnusedDeclaration,
    ];
}

/// 判据级档位（封闭二值，纯进程内枚举——零 serde / specta，不进 IPC）：`off` 不产出
/// 条目、`warn` 产出提示。
///
/// **与元素 `gate` 类型分立**：`strict` 在判据面上不可表达，复用 `GateTier` 会让
/// 「契约判据档位」与「元素 gate」在类型上可互换——正是本变更要解耦的那件事。唯一
/// 来源是 [`ArchiApiFindingKind::severity`]；无配置键、无环境变量、无命令行 flag
/// 可改档位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiFindingSeverity {
    /// 不产出条目（完备性判据的档位）
    Off,
    /// 产出提示（一致性判据的档位）
    Warn,
}

impl ApiFindingSeverity {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            ApiFindingSeverity::Off => "off",
            ApiFindingSeverity::Warn => "warn",
        }
    }

    /// 全部合法值（清单单点）。
    pub const ALL: [ApiFindingSeverity; 2] = [ApiFindingSeverity::Off, ApiFindingSeverity::Warn];
}

/// 契约判据种类（封闭十值；线格式 camelCase）。
///
/// **与 [`ArchiDriftKind`] 分立**（不并入的四值枚举）：① 事实源不同——`ArchiDriftKind`
/// 是「模型 relation 图 vs 代码」，本枚举是「契约文件 vs 代码」；② 定级语义不同——
/// 前者看引入方元素 `gate`，后者自带 [`ArchiApiFindingKind::severity`]；③ 扩展节奏
/// 不同——契约判据按迁移进度逐个打开，混装要动既有语料金样与全部 `match` 臂。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ArchiApiFindingKind {
    /// 元素根路径下既无生成物也无实现入口（结构前提失效）
    ApiEntryMissing,
    /// 契约 → 代码缺失（`declared` 而契约文件缺失，或声明的 op / 类型在代码里没有）
    ApiMissing,
    /// 代码有 `external` 面但契约未声明（**唯一完备性判据**，档位 `off`）
    ApiUndeclared,
    /// 契约签名与代码签名（归一后）不一致
    ApiSignatureMismatch,
    /// 留白与其反面矛盾（`api = none` 而契约文件存在）
    ApiUnexpected,
    /// 父 `$defs` 与子元素 `$defs` 类型名交集非空
    ApiShadowing,
    /// 跨模块 `$ref` 目标缺失 / 未 declared / 非直接子元素
    ApiRefMissing,
    /// 嵌套元素父子分属不同 crate
    ApiNestedCrossCrate,
    /// markdown 散文列表与 `api-reexport` fence 键集不等
    ApiReexportDrift,
    /// 契约 `### 操作` 声明了构造器（应由生成器产出）
    ApiConstructorDeclared,
}

impl ArchiApiFindingKind {
    /// 线格式 camelCase 词。
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiApiFindingKind::ApiEntryMissing => "apiEntryMissing",
            ArchiApiFindingKind::ApiMissing => "apiMissing",
            ArchiApiFindingKind::ApiUndeclared => "apiUndeclared",
            ArchiApiFindingKind::ApiSignatureMismatch => "apiSignatureMismatch",
            ArchiApiFindingKind::ApiUnexpected => "apiUnexpected",
            ArchiApiFindingKind::ApiShadowing => "apiShadowing",
            ArchiApiFindingKind::ApiRefMissing => "apiRefMissing",
            ArchiApiFindingKind::ApiNestedCrossCrate => "apiNestedCrossCrate",
            ArchiApiFindingKind::ApiReexportDrift => "apiReexportDrift",
            ArchiApiFindingKind::ApiConstructorDeclared => "apiConstructorDeclared",
        }
    }

    /// 判据级档位（**唯一来源**）：九条一致性判据 `warn`、唯一完备性判据
    /// `apiUndeclared` 为 `off`（迁移期全量打开等于让门禁永久红）。
    pub fn severity(self) -> ApiFindingSeverity {
        match self {
            ArchiApiFindingKind::ApiUndeclared => ApiFindingSeverity::Off,
            _ => ApiFindingSeverity::Warn,
        }
    }

    /// 全部合法值（清单单点：新增变体不同步即语料红）。
    pub const ALL: [ArchiApiFindingKind; 10] = [
        ArchiApiFindingKind::ApiEntryMissing,
        ArchiApiFindingKind::ApiMissing,
        ArchiApiFindingKind::ApiUndeclared,
        ArchiApiFindingKind::ApiSignatureMismatch,
        ArchiApiFindingKind::ApiUnexpected,
        ArchiApiFindingKind::ApiShadowing,
        ArchiApiFindingKind::ApiRefMissing,
        ArchiApiFindingKind::ApiNestedCrossCrate,
        ArchiApiFindingKind::ApiReexportDrift,
        ArchiApiFindingKind::ApiConstructorDeclared,
    ];
}

/// 豁免来源（封闭二值；线格式小写词）：`config` = 架构域配置元素对、`inline` =
/// 内联注释 `// archi-ignore-next-line: <reason>`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ArchiSuppressVia {
    /// 配置元素对豁免
    Config,
    /// 内联注释豁免
    Inline,
}

impl ArchiSuppressVia {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiSuppressVia::Config => "config",
            ArchiSuppressVia::Inline => "inline",
        }
    }
}

/// 违例命中点（同元素对的全部命中点聚合在一条条目内，不按行铺开）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDriftOccurrence {
    /// 命中文件（repo 相对 POSIX）
    pub file: String,
    /// 命中行（1 起始；`0` = 位置未知——`path` 类条目恒 0）
    pub line: u32,
    /// import 声明原文 specifier
    pub specifier: Option<String>,
    /// 是否 type-only 导入（只标注，MUST NOT 因它豁免）
    pub type_only: bool,
}

/// 漂移条目（`errors[]` / `warnings[]` / `model.warnings[]` 共用形状）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDriftEntry {
    /// 条目种类
    pub kind: ArchiDriftKind,
    /// 引入方元素 FQN（定级依据；path 类条目为该 path 所属元素）
    pub from: Option<String>,
    /// 被引入元素 FQN（path 类条目为 `null`）
    pub to: Option<String>,
    /// 定级依据的 gate 档（`null` = 无元素可归因）
    pub gate: Option<GateTier>,
    /// 人读文案
    pub message: String,
    /// 命中点（path 类条目恒一条、`line = 0`）
    pub occurrences: Vec<ArchiDriftOccurrence>,
}

/// 被抑制的违例（豁免不等于不存在——保留条目 + 理由 + 来源，豁免了什么才可
/// 审计）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDriftSuppressedEntry {
    /// 引入方元素 FQN
    pub from: String,
    /// 被引入元素 FQN
    pub to: String,
    /// 豁免来源
    pub via: ArchiSuppressVia,
    /// 豁免理由（必填非空白）
    pub reason: String,
    /// 内联豁免所在文件（`via = config` 时 `null`）
    pub file: Option<String>,
    /// 内联豁免所在行（`via = config` 时 `null`）
    pub line: Option<u32>,
}

/// 观察项（设计态陈述；永不影响 `status` / `passed`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDriftObservation {
    /// 观察项种类
    pub kind: ArchiObservationKind,
    /// 声明方元素 FQN
    pub from: String,
    /// 被声明依赖的元素 FQN
    pub to: String,
    /// 人读文案（增量模式显式声明扫描面，避免被读成全仓结论）
    pub message: String,
}

/// 契约判据条目（`apiFindings[]` 的条目形状）。
///
/// `element` 对应既有条目的「引入方」语义、`target` 对应「被引入方」语义；
/// `occurrences` 复用 [`ArchiDriftOccurrence`]（同元素对的多处命中聚在一条内，不按行
/// 铺开）。`category` 是**开放值集字符串**（无 `ALL`，渲染层据此分组）：新增分类只改
/// 打标处，不动枚举、不动 `ALL`、不动金样词汇对拍。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiApiFinding {
    /// 判据种类
    pub kind: ArchiApiFindingKind,
    /// 报告分组（开放值集：`entry` / `contract` / `signature` / `reexport` / `nesting` /
    /// `constructor`）
    pub category: String,
    /// 归属元素 FQN
    pub element: Option<String>,
    /// 涉及对象（缺定义的类型名 / 缺失的 op / 重复类型名等）
    pub target: Option<String>,
    /// 人读文案
    pub message: String,
    /// 命中点（同元素对聚合）
    pub occurrences: Vec<ArchiDriftOccurrence>,
}

/// gate 档位计数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiGateCount {
    /// strict 档元素数
    pub strict: u32,
    /// warn 档元素数
    pub warn: u32,
    /// off 档元素数
    pub off: u32,
}

/// 模型面事实（元素 / 关系计数 + 结构错 + 模型警告）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDriftModel {
    /// 元素总数
    pub elements: u32,
    /// 关系总数
    pub relations: u32,
    /// gate 档位分布
    pub by_gate: ArchiGateCount,
    /// 结构诊断（读存量模型遇错也照录；进 `status = errors` 但不进 `errors[]`）
    pub structural_errors: Vec<ArchiDiagnostic>,
    /// 模型警告（`pathNotFound` / `noPath`——判定域自己的模型警告）
    pub warnings: Vec<ArchiDriftEntry>,
}

/// 被排除的文件（逐条留痕，MUST NOT 静默丢弃）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiExcludedFile {
    /// 被排除文件（repo 相对 POSIX）
    pub file: String,
    /// 命中的具体 glob 规则原文
    pub rule: String,
}

/// 按元素聚合的归属结果（元素 FQN 字典序、`files` 路径字典序）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiMatchedElement {
    /// 元素 FQN
    pub element: String,
    /// 归属到该元素的文件（repo 相对 POSIX）
    pub files: Vec<String>,
}

/// 不可判定的文件 / 导入（留痕，MUST NOT 冒充违例）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiUnresolvedFile {
    /// 文件（repo 相对 POSIX）
    pub file: String,
    /// 涉及的 specifier（无可归因时为 `null`）
    pub specifier: Option<String>,
    /// 不可判定原因
    pub message: String,
}

/// 扫描面事实（枚举 / 排除 / 归属 / 留痕）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiScanReport {
    /// 扫描模式
    pub mode: ArchiDriftMode,
    /// 变更集内源文件数（全量模式恒 0）
    pub changed_files: u32,
    /// 实际可判源文件数（候选源文件 − 被排除数）
    pub scanned_files: u32,
    /// 被排除文件（逐条留痕）
    pub excluded: Vec<ArchiExcludedFile>,
    /// 无元素认领的文件（含 `off` 元素覆盖与归属不确定）
    pub unmatched_files: Vec<String>,
    /// 按元素聚合的归属结果
    pub matched: Vec<ArchiMatchedElement>,
    /// 不可判定留痕
    pub unresolved: Vec<ArchiUnresolvedFile>,
}

/// 架构漂移检查报告（两模式共用；命令面出线与门禁面 `passed` 派生同源）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiDriftReport {
    /// 报告结构版本号（字段演进须递增并走显式兼容说明）
    pub version: u32,
    /// RFC3339 UTC 时间戳
    pub timestamp: String,
    /// 检查模式
    pub mode: ArchiDriftMode,
    /// 观察根（绝对路径）
    pub root: String,
    /// change 磁盘名（全量模式 / 命令面无 change 上下文时 `null`）
    pub change: Option<String>,
    /// 增量基准（增量恒 `HEAD`；全量模式 `null`）
    pub base: Option<String>,
    /// `models/*.toml` 聚合哈希（`sha256:<hex>`）
    pub model_hash: String,
    /// 模型面事实
    pub model: ArchiDriftModel,
    /// 扫描面事实
    pub scan: ArchiScanReport,
    /// 违例（按引入方 `gate` 档收口）
    pub errors: Vec<ArchiDriftEntry>,
    /// 提示（`warn` 档违例 / 目标未建模 / 归属不确定 / path 失效等）
    pub warnings: Vec<ArchiDriftEntry>,
    /// 被抑制的违例（豁免留痕）
    pub suppressed: Vec<ArchiDriftSuppressedEntry>,
    /// 观察项（永不影响 `status` / `passed`）
    pub observations: Vec<ArchiDriftObservation>,
    /// 契约判据（并列槽位；空数组 = 无契约问题）。**不进 `passed`**；对 `status`
    /// 的影响限于**契约在案元素**的判据（纳管口径归 `desktop-archi-api-check`，筛选
    /// 单点在 `api::gating_findings`）——严重度由判据级
    /// [`ArchiApiFindingKind::severity`] 决定
    pub api_findings: Vec<ArchiApiFinding>,
    /// 检查结论
    pub status: ArchiCheckStatus,
}

/// 报告结构短名（spec Module Contract 的 `Report`；线面名带 `Archi` 前缀是
/// specta 注册表按裸名唯一的约束，见模块 doc）。
pub use ArchiDriftReport as Report;
