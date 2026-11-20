//! TOML 实现：`toml` + span 反序列化（字段面严格）+ 封闭枚举值域 + parser 错误
//! → 结构化诊断（原文文案与行列零加工）。
//!
//! 行 / 列换算只做字节偏移的后处理（[`line_of`] / [`column_of`]），MUST NOT 二次
//! 扫文本反推行列——span 由反序列化同时产出。

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::dto::{ApiStatus, DiagnosticKind, ElementKind, GateTier};
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
    /// 契约状态指针（缺省 `declared`；拼错的值由既有 `classify` 归 `invalidValue`）
    #[serde(default)]
    api: ApiStatus,
    /// 实现入口指针（缺省 `None`）：数组 / 内联表形态由 serde `invalid type` 归
    /// `invalidValue`（`entry` 是单值），拼错的键仍由 `deny_unknown_fields` 归
    /// `unknownField`
    #[serde(default)]
    entry: Option<String>,
    #[serde(default)]
    path: Vec<::toml::Spanned<String>>,
    /// 入口文档指针（缺省 `None`）：`doc` 是单值，数组形态由 serde `invalid type` 归
    /// `invalidValue`（MUST NOT 静默取首条）；旧的多值文档字段已出白名单，写了即
    /// `unknownField`
    #[serde(default)]
    doc: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
}

/// 关系条目字段面（`from` / `to` 必填，`desc` / `apis` 可选）。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRelation {
    from: ::toml::Spanned<String>,
    to: ::toml::Spanned<String>,
    #[serde(default)]
    desc: Option<String>,
    /// 依赖 API 名清单（选填；值面五拒在 [`to_model`] 校验并出 `invalidValue`——
    /// 只有在落模型期才拿得到逐项的 `span`）
    #[serde(default)]
    apis: Option<Vec<::toml::Spanned<String>>>,
}

/// TOML 源解析器（[`SourceParser`] 的唯一实现）。
pub struct TomlParser;

impl SourceParser for TomlParser {
    fn parse(source: &str) -> ParseOutcome {
        match ::toml::from_str::<RawDocument>(source) {
            Ok(raw) => {
                let (model, diagnostics) = to_model(raw, source);
                ParseOutcome { model, diagnostics }
            }
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
///
/// 关系 `apis` 的值面校验（[`validate_apis`]）在此产出带位置的 `invalidValue`——
/// serde 层只能给类型错的位置，拿不到逐项 `span`。
fn to_model(raw: RawDocument, source: &str) -> (Model, Vec<Diagnostic>) {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut elements: Vec<Element> = Vec::with_capacity(raw.element.len());
    for (key, raw_element) in raw.element {
        let span = key.span();
        elements.push(Element {
            fqn: key.into_inner(),
            kind: raw_element.kind,
            gate: raw_element.gate,
            api: raw_element.api,
            entry: raw_element.entry,
            path: raw_element
                .path
                .into_iter()
                .map(::toml::Spanned::into_inner)
                .collect(),
            doc: raw_element.doc,
            summary: raw_element.summary,
            metadata: raw_element.metadata,
            file: String::new(),
            line: line_of(source, span.start),
        });
    }
    let mut relations: Vec<Relation> = Vec::with_capacity(raw.relation.len());
    for raw_relation in raw.relation {
        let span = raw_relation.from.span();
        let from = raw_relation.from.into_inner();
        let to = raw_relation.to.into_inner();
        let apis = raw_relation.apis;
        // 缺省（未写该键）与显式空数组必须可辨：前者是「不声明 API 粒度」，后者是
        // 值面违例（`invalidValue`）
        if let Some(items) = &apis {
            diagnostics.extend(validate_apis(source, &from, &to, span.clone(), items));
        }
        relations.push(Relation {
            from,
            to,
            description: raw_relation.desc,
            apis: apis
                .unwrap_or_default()
                .into_iter()
                .map(::toml::Spanned::into_inner)
                .collect(),
            file: String::new(),
            line: line_of(source, span.start),
            span: Some(SourceSpan::from(span)),
        });
    }
    (
        Model {
            elements,
            relations,
        },
        diagnostics,
    )
}

/// `apis` 值面五拒（空串 / 含空白 / 含 `::` / 同名重复 / 显式空数组），逐条出带位置
/// 的 `invalidValue`。
///
/// 纪律：MUST NOT 静默去重、MUST NOT 静默截断、MUST NOT 把空数组当省略——「写重了」
/// 与「写对了」、「声明了零项」与「没声明」必须可辨。
fn validate_apis(
    source: &str,
    from: &str,
    to: &str,
    relation_span: std::ops::Range<usize>,
    apis: &[::toml::Spanned<String>],
) -> Vec<Diagnostic> {
    let diagnostic = |message: String, span: Option<std::ops::Range<usize>>| Diagnostic {
        kind: DiagnosticKind::InvalidValue,
        message,
        file: String::new(),
        line: span.as_ref().map_or(0, |span| line_of(source, span.start)),
        column: span.map(|span| column_of(source, span.start)),
    };
    if apis.is_empty() {
        return vec![diagnostic(
            format!(
                "关系 {from} → {to} 的 apis 是空数组：空数组与省略同义（不声明 API 粒度）——\
                 删掉该键，或写入至少一个名字"
            ),
            Some(relation_span),
        )];
    }
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for item in apis {
        let span = item.span();
        let name = item.get_ref();
        if name.is_empty() {
            diagnostics.push(diagnostic(
                format!("关系 {from} → {to} 的 apis 含空串：只写名字（不得为空）"),
                Some(span),
            ));
            continue;
        }
        if name.chars().any(char::is_whitespace) {
            diagnostics.push(diagnostic(
                format!(
                    "关系 {from} → {to} 的 apis 项 \"{name}\" 含空白：只写名字——不写签名 / \
                     参数 / 类型形状"
                ),
                Some(span),
            ));
            continue;
        }
        if name.contains("::") {
            diagnostics.push(diagnostic(
                format!("关系 {from} → {to} 的 apis 项 \"{name}\" 含 `::`：只写名字，不写限定路径"),
                Some(span),
            ));
            continue;
        }
        if !seen.insert(name) {
            diagnostics.push(diagnostic(
                format!("关系 {from} → {to} 的 apis 项 \"{name}\" 重复：不重复（不做静默去重）"),
                Some(span),
            ));
            continue;
        }
    }
    diagnostics
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
