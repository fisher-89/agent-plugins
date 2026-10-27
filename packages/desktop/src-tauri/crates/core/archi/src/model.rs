//! 格式无关 AST：模型 / 元素 / 关系 / 源区间 / 诊断 / 解析产物。
//!
//! 词汇（`ElementKind` / `GateTier` / `DiagnosticKind`）单点在 [`crate::dto`]，
//! 本模块反向引用——不存在第二份枚举定义。`derive PartialEq`（UI diff 预留）；
//! 关系级源区间为可选占位（当前只消费行号）。

use std::collections::BTreeMap;

use crate::dto::{DiagnosticKind, ElementKind, GateTier};

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
    /// 覆盖路径（原文，不归一）
    pub path: Vec<String>,
    /// 显式文档路径（原文，不归一）
    pub docs: Vec<String>,
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
