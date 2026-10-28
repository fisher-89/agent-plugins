use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::docs;
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

/// 文档警告种类（单值；与诊断分立使「文档警告不停摆」在类型上成立）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum WarningKind {
    /// `docs` 指向的文件不存在（只报不拦）
    DocNotFound,
}

impl WarningKind {
    /// 线格式 camelCase 词。
    pub fn as_str(self) -> &'static str {
        match self {
            WarningKind::DocNotFound => "docNotFound",
        }
    }
}

/// 文档形态（由落地路径决定：决策记录目录内恒 `Adr`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    /// 普通 markdown 文档
    Md,
    /// 架构决策记录
    Adr,
}

/// 文档关联来源（强度序即声明序：`Explicit` > `Adjacent` > `Text`）。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "lowercase")]
pub enum DocSource {
    /// 元素 `docs` 显式指定（最强）
    Explicit,
    /// 元素 `path` 覆盖目录内扫描命中
    Adjacent,
    /// ADR 正文提及元素 FQN（最弱）
    Text,
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

/// 检索范围（封闭三值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SearchScope {
    /// 只搜元素
    Elements,
    /// 只搜文档关联条目
    Docs,
    /// 两者皆搜
    All,
}

impl SearchScope {
    /// 线格式词 → 枚举（其余值 `Err` 并列出合法值）。
    pub fn parse(raw: &str) -> Result<SearchScope, String> {
        match raw {
            "elements" => Ok(SearchScope::Elements),
            "docs" => Ok(SearchScope::Docs),
            "all" => Ok(SearchScope::All),
            other => Err(format!(
                "非法 scope: {other}（合法值：elements / docs / all）"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// 线面 DTO（specta::Type → bindings.ts）
// ---------------------------------------------------------------------------

/// 文档关联条目（恒四字段：无摘要、无正文、无行区间）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocRef {
    /// repo 相对路径（`/` 分隔）
    pub path: String,
    /// 首行 ATX 标题（无标题则文件名去扩展名）
    pub title: String,
    /// 形态标记
    pub kind: DocKind,
    /// 来源强度词汇
    pub source: DocSource,
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

/// 文档警告投影（只报不拦）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiWarning {
    /// 警告种类
    pub kind: WarningKind,
    /// 文案
    pub message: String,
    /// 归属元素 FQN
    pub element: Option<String>,
    /// 涉及的路径
    pub path: Option<String>,
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
    /// 一行摘要
    pub summary: Option<String>,
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
    /// 文档关联条目（强度序）
    pub docs: Vec<DocRef>,
    /// 文档警告（只报不拦）
    pub warnings: Vec<ArchiWarning>,
}

/// 检索结果（薄条目 + 四字段文档条目）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiSearchResult {
    /// 命中元素（FQN 字典序）
    pub elements: Vec<ArchiElementBrief>,
    /// 命中文档（来源强度 → 路径字典序）
    pub docs: Vec<DocRef>,
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
struct Loaded {
    graph: ModelGraph,
    diagnostics: Vec<Diagnostic>,
}

/// 读盘 → 建图 → 诊断合并（本 crate 内 `ModelGraph::build` 的唯一调用点）。
fn load(root: &Path) -> Loaded {
    let outcome = parse::parse_dir(root);
    let parse_diagnostics = outcome.diagnostics;
    let graph = ModelGraph::build(outcome.model);
    let mut diagnostics = parse_diagnostics;
    diagnostics.extend(graph.diagnostics());
    Loaded { graph, diagnostics }
}

/// 元素投影（只裁剪）。
fn to_element(element: &Element) -> ArchiElement {
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
        summary: element.summary.clone(),
    }
}

/// 诊断清单 → 单条错误文案（`kind @ file:line` 逐行；使调用方一眼看出「模型
/// 本身坏了」而不是「没有这个模块」）。
fn diagnostics_error(diagnostics: &[Diagnostic]) -> String {
    let mut lines: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| {
            let kind = diagnostic.kind.as_str();
            if diagnostic.line == 0 {
                format!("{kind} @ {}", diagnostic.file)
            } else {
                format!("{kind} @ {}:{}", diagnostic.file, diagnostic.line)
            }
        })
        .collect();
    lines.insert(0, format!("模型结构错（共 {} 条）：", diagnostics.len()));
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

/// 单元素投影：元素 + 邻域（出边 / 入边 / 直接子元素）+ 文档关联 + 警告。
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
    let docs_outcome = docs::element_docs(&loaded.graph, root, fqn);
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
        docs: docs_outcome.docs,
        warnings: docs_outcome
            .warnings
            .iter()
            .map(|warning| ArchiWarning {
                kind: WarningKind::DocNotFound,
                message: format!("{} 指向的文档不存在", warning.path),
                element: Some(warning.element.clone()),
                path: Some(warning.path.clone()),
            })
            .collect(),
    })
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

/// 检索投影（元素薄条目 + 四字段文档条目）。
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
        docs: outcome.docs,
    })
}

/// 模型结构 diff 摘要：两侧模型目录俱缺席 → `None`；两侧等价（`is_empty` 且
/// 改动侧零诊断）→ `None`（零占位）；否则摘要 + 改动侧诊断随行。
pub fn change_summary(
    main_root: &Path,
    next_root: &Path,
) -> Result<Option<ArchiChangeSummary>, String> {
    let main_models = foundation::layout::archi_models_dir(main_root);
    let next_models = foundation::layout::archi_models_dir(next_root);
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

/// 关联集路径集（文档正文取数的越界校验唯一判据）。
pub fn linked_doc_paths(root: &Path) -> Result<BTreeSet<String>, String> {
    let loaded = load(root);
    if !loaded.diagnostics.is_empty() {
        return Err(diagnostics_error(&loaded.diagnostics));
    }
    let outcome = docs::all_docs(&loaded.graph, root);
    Ok(outcome.docs.iter().map(|doc| doc.path.clone()).collect())
}
