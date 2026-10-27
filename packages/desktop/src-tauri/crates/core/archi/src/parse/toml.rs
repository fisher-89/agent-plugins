//! TOML 实现：`toml` + span 反序列化（字段面严格）+ 封闭枚举值域 + parser 错误
//! → 结构化诊断（原文文案与行列零加工）。
//!
//! 行 / 列换算只做字节偏移的后处理（[`line_of`] / [`column_of`]），MUST NOT 二次
//! 扫文本反推行列——span 由反序列化同时产出。

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::dto::{DiagnosticKind, ElementKind, GateTier};
use crate::model::{Diagnostic, Element, Model, ParseOutcome, Relation, SourceSpan};

use super::SourceParser;

/// 顶层文档形态：`element` 为「一元素一表头、键为带引号的完整 FQN 单键」的
/// 扁平表；`relation` 为条目数组。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDocument {
    #[serde(default)]
    element: BTreeMap<::toml::Spanned<String>, RawElement>,
    #[serde(default)]
    relation: Vec<RawRelation>,
}

/// 元素表字段面（未知字段拒绝；`metadata` 内未知键由 map 语义接受）。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawElement {
    kind: ElementKind,
    #[serde(default)]
    gate: GateTier,
    #[serde(default)]
    path: Vec<::toml::Spanned<String>>,
    #[serde(default)]
    docs: Vec<::toml::Spanned<String>>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
}

/// 关系条目字段面（`from` / `to` 必填，`desc` 可选）。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRelation {
    from: ::toml::Spanned<String>,
    to: ::toml::Spanned<String>,
    #[serde(default)]
    desc: Option<String>,
}

/// TOML 源解析器（[`SourceParser`] 的唯一实现）。
pub struct TomlParser;

impl SourceParser for TomlParser {
    fn parse(source: &str) -> ParseOutcome {
        match ::toml::from_str::<RawDocument>(source) {
            Ok(raw) => ParseOutcome {
                model: to_model(raw, source),
                diagnostics: Vec::new(),
            },
            Err(error) => {
                let span = error.span();
                ParseOutcome {
                    model: Model::empty(),
                    diagnostics: vec![Diagnostic {
                        kind: classify(error.message()),
                        // 文案原样：parser 自带行 / 列、源码片段与 caret 指示
                        message: error.to_string(),
                        file: String::new(),
                        line: span.as_ref().map_or(0, |span| line_of(source, span.start)),
                        column: span.map(|span| column_of(source, span.start)),
                    }],
                }
            }
        }
    }
}

/// 反序列化产物 → 格式无关 AST（`file` 留空由聚合层回填；行号取自 span）。
fn to_model(raw: RawDocument, source: &str) -> Model {
    let mut elements: Vec<Element> = Vec::with_capacity(raw.element.len());
    for (key, raw_element) in raw.element {
        let span = key.span();
        elements.push(Element {
            fqn: key.into_inner(),
            kind: raw_element.kind,
            gate: raw_element.gate,
            path: raw_element
                .path
                .into_iter()
                .map(::toml::Spanned::into_inner)
                .collect(),
            docs: raw_element
                .docs
                .into_iter()
                .map(::toml::Spanned::into_inner)
                .collect(),
            summary: raw_element.summary,
            metadata: raw_element.metadata,
            file: String::new(),
            line: line_of(source, span.start),
        });
    }
    let mut relations: Vec<Relation> = Vec::with_capacity(raw.relation.len());
    for raw_relation in raw.relation {
        let span = raw_relation.from.span();
        relations.push(Relation {
            from: raw_relation.from.into_inner(),
            to: raw_relation.to.into_inner(),
            description: raw_relation.desc,
            file: String::new(),
            line: line_of(source, span.start),
            span: Some(SourceSpan::from(span)),
        });
    }
    Model {
        elements,
        relations,
    }
}

/// parser / serde 错误 → 诊断种类（判定只看文案前缀；分类学需要机器可辨，文案
/// 本身零加工）。
///
/// `invalid type` 归 `invalidValue`（值域违例）：`kind` / `gate` 非法取值与
/// `metadata` 非字符串值同属「值不合法」，不是语法面失败（spec 明写非字符串
/// metadata 值即 `invalidValue`）。
fn classify(message: &str) -> DiagnosticKind {
    if message.contains("unknown variant") || message.contains("invalid type") {
        DiagnosticKind::InvalidValue
    } else if message.contains("unknown field") {
        DiagnosticKind::UnknownField
    } else if message.contains("missing field") {
        DiagnosticKind::MissingField
    } else if message.contains("duplicate key") {
        DiagnosticKind::DuplicateElement
    } else {
        DiagnosticKind::SyntaxError
    }
}

/// 字节偏移 → 行号（1 起始；char 边界安全回退）。
pub(crate) fn line_of(source: &str, byte: usize) -> u32 {
    let end = boundary_before(source, byte);
    1 + source[..end].matches('\n').count() as u32
}

/// 字节偏移 → 列（1 起始，按 **char** 计——与 parser 的 caret 同单位）。
pub(crate) fn column_of(source: &str, byte: usize) -> u32 {
    let end = boundary_before(source, byte);
    let line_start = source[..end].rfind('\n').map_or(0, |index| index + 1);
    1 + source[line_start..end].chars().count() as u32
}

/// 偏移裁到 ≤ 源长的最近 char 边界（越界 / 落在多字节中间均回退）。
fn boundary_before(source: &str, byte: usize) -> usize {
    let mut end = byte.min(source.len());
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    end
}
