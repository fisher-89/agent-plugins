//! 格式无关 AST：模型 / 元素 / 关系 / 源区间 / 诊断 / 解析产物。
//!
//! 词汇（`ElementKind` / `GateTier` / `DiagnosticKind`）单点在 [`crate::dto`]，
//! 本模块反向引用——不存在第二份枚举定义。`derive PartialEq`（UI diff 预留）；
//! 关系级源区间为可选占位（当前只消费行号）。

use std::collections::BTreeMap;

use crate::dto::{ApiStatus, DiagnosticKind, ElementKind, GateTier};

/// 聚合后的完整模型（多文件合并，声明序保留）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// 元素（文件序 + 声明序）
    pub elements: Vec<Element>,
    /// 关系（文件序 + 声明序）
    pub relations: Vec<Relation>,
}

impl Model {
    /// 空模型（模型目录缺席 / 空目录的合法起点）。
    pub fn empty() -> Self {
        Self {
            elements: Vec::new(),
            relations: Vec::new(),
        }
    }

    /// 按 FQN 取元素引用（线性扫描——模型规模按元素数百计；投影、`gate` 写回
    /// 定位与检索薄条目共用，禁第二份查找实现）。
    pub fn element(&self, fqn: &str) -> Option<&Element> {
        self.elements.iter().find(|element| element.fqn == fqn)
    }
}

/// 单个架构元素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// 完整限定名（层级由 FQN 最长前缀推导）
    pub fqn: String,
    /// 元素种类
    pub kind: ElementKind,
    /// gate 档位（缺省 `off`）
    pub gate: GateTier,
    /// 接口承诺开关（缺省 `declared`）：`declared` = 断言本模块应有一份
    /// `<models>/<分层目录…>/<叶段>.schema.json`、`none` = 显式无接口面（schema 在盘即违约）。
    ///
    /// **它不再兼任入口文档载体的开关**：文档（资产①）的落点恒为派生
    /// `<models>/<分层目录…>/<叶段>.md`，与 `api` 无关。协议正文 MUST NOT 进模型——签名 /
    /// 导出符号 / 接口定义写在架构域 `models/` 目录的接口定义文件里（与文档同目录）。
    pub api: ApiStatus,
    /// 实现入口指针（缺省 `None`）：单值、repo 相对 POSIX，与 `path` 同口径
    /// （`./` 前缀由归一剥除、不参与身份）。四条判据（基准 / 落位 / 存在 /
    /// 跨元素唯一）与三态解析链归 [`crate::api::paths`]——本字段只承载「入口在
    /// 哪一扇门」，MUST NOT 承载协议正文。
    pub entry: Option<String>,
    /// 覆盖路径（原文，不归一）
    pub path: Vec<String>,
    /// 文档指针（**约定，非机械判据**）：单值、repo 相对 POSIX；约定只有 `api = none`
    /// 的元素会写（`declared` 元素的文档在派生落点）。
    ///
    /// **零判据读者**：判据面 / 纳管面 / 路径派生面 / 检索语料面都不读它——写了不报、
    /// 指不到盘也不报（`apiUnexpected` 的形态② 已随字段惰性化删除）。保留它是为了外部
    /// 工作区写了 `doc` 不突然变成 `unknownField` 结构错；其值仅供 `archi_module` 投影与
    /// 画布「入口文档」段呈现。
    pub doc: Option<String>,
    /// 一行摘要
    pub summary: Option<String>,
    /// 自由元数据表（字符串 → 字符串，未知键忽略）
    pub metadata: BTreeMap<String, String>,
    /// 声明所在文件（repo 相对、`/` 分隔；由聚合层回填）
    pub file: String,
    /// 声明所在行（1 起始；0 = 位置未知）
    pub line: u32,
}

/// 单条关系（`from -> to`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    /// 起点元素 FQN
    pub from: String,
    /// 终点元素 FQN
    pub to: String,
    /// 关系说明
    pub description: Option<String>,
    /// 依赖 API 名清单（模型原文序；缺省空 = 不声明 API 粒度；去重 / 排序只在
    /// 判定侧的 `drift::api_buckets` 内做）
    pub apis: Vec<String>,
    /// 声明所在文件（repo 相对、`/` 分隔）
    pub file: String,
    /// 声明所在行（`from` 键行，1 起始；0 = 位置未知）
    pub line: u32,
    /// 声明源区间（占位：当前只消费行号，不消费列区间）
    pub span: Option<SourceSpan>,
}

/// 源文本字节区间（进程内，不进线面、无 serde）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    /// 起始字节偏移
    pub start: usize,
    /// 结束字节偏移
    pub end: usize,
}

impl From<std::ops::Range<usize>> for SourceSpan {
    fn from(range: std::ops::Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

/// 结构化诊断（`line == 0` 表示位置未知——serde 自定义错误无 span 时的表达）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    /// 诊断种类（封闭九值）
    pub kind: DiagnosticKind,
    /// 原文文案（parser 面零加工，含行 / 列 / 片段 / caret）
    pub message: String,
    /// 所在文件（repo 相对、`/` 分隔）
    pub file: String,
    /// 所在行（1 起始；0 = 位置未知）
    pub line: u32,
    /// 所在列（1 起始，按 char 计；未知即 `None`）
    pub column: Option<u32>,
}

/// 解析产物：模型 + 诊断（单次加载单次诊断，不做首个错误短路）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutcome {
    /// 聚合模型（有诊断时可能只含可解析部分）
    pub model: Model,
    /// 诊断清单（空即健康）
    pub diagnostics: Vec<Diagnostic>,
}
