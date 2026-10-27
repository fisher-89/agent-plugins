//! 轻量检索：ASCII 大小写不敏感子串 + 驼峰分词，匹配对象恰为元素 FQN / `summary`
//! / `metadata` 值 / 文档 `path` 与文件名（去扩展名）。
//!
//! 刻意不做全文搜索：不读文档正文、不碰规约语料、无相关度打分、无倒排索引与
//! 跨调用缓存（每次现算——第二事实源无从产生）。文档条目的 `title` 取自正文首行，
//! 属正文射程，故 MUST NOT 参与匹配。

use crate::docs;
use crate::dto::{DocRef, SearchScope};
use crate::graph::ModelGraph;
use crate::model::Element;

/// 检索结果（元素 FQN 清单 + 文档条目）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOutcome {
    /// 命中元素 FQN（字典序）
    pub element_fqns: Vec<String>,
    /// 命中文档（来源强度序 → 路径字典序）
    pub docs: Vec<DocRef>,
}

/// 检索入口：空白 query → `Err`（MUST NOT 返回全量）；`Elements` 时 `docs` 恒空、
/// `Docs` 时 `element_fqns` 恒空。
pub fn search(
    graph: &ModelGraph,
    root: &std::path::Path,
    query: &str,
    scope: SearchScope,
) -> Result<SearchOutcome, String> {
    let tokens = tokenize(query);
    if tokens.is_empty() {
        return Err("query 不能为空（需至少一个字母或数字）".to_owned());
    }
    let mut outcome = SearchOutcome {
        element_fqns: Vec::new(),
        docs: Vec::new(),
    };
    if scope != SearchScope::Docs {
        outcome.element_fqns = graph
            .model()
            .elements
            .iter()
            .filter(|element| matches_element(element, &tokens))
            .map(|element| element.fqn.clone())
            .collect();
        outcome.element_fqns.sort_unstable();
        outcome.element_fqns.dedup();
    }
    if scope != SearchScope::Elements {
        outcome.docs = docs::all_docs(graph, root)
            .docs
            .into_iter()
            .filter(|doc| matches_doc(doc, &tokens))
            .collect();
    }
    Ok(outcome)
}

/// 元素命中判据：FQN / `summary` / `metadata` 各值拼合后包含**全部**分词段。
fn matches_element(element: &Element, tokens: &[String]) -> bool {
    let mut haystack = String::with_capacity(element.fqn.len() + 32);
    haystack.push_str(&element.fqn);
    if let Some(summary) = &element.summary {
        haystack.push('\n');
        haystack.push_str(summary);
    }
    for value in element.metadata.values() {
        haystack.push('\n');
        haystack.push_str(value);
    }
    contains_all(&haystack, tokens)
}

/// 文档命中判据：路径与文件名去扩展名（**不读正文**，故 `title` 不参与——标题取自
/// 正文首行，属正文射程）。
fn matches_doc(doc: &DocRef, tokens: &[String]) -> bool {
    let mut haystack = String::with_capacity(doc.path.len() + 32);
    haystack.push_str(&doc.path);
    haystack.push('\n');
    haystack.push_str(&docs::file_stem(&doc.path));
    contains_all(&haystack, tokens)
}

/// 目标文本（小写后）是否包含全部分词段（AND 语义）。
fn contains_all(haystack: &str, tokens: &[String]) -> bool {
    let lowered = haystack.to_lowercase();
    tokens.iter().all(|token| lowered.contains(token))
}

/// 分词：按非字母数字切，再按驼峰 / 数字边界切，小写后去重保序
/// （`AuthToken` → `auth` / `token`）。
fn tokenize(raw: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    for chunk in raw.split(|c: char| !c.is_alphanumeric()) {
        for part in split_camel(chunk) {
            let lowered = part.to_lowercase();
            if !lowered.is_empty() && !tokens.contains(&lowered) {
                tokens.push(lowered);
            }
        }
    }
    tokens
}

/// 驼峰与数字边界切分（`AuthToken` → `Auth` / `Token`）。
fn split_camel(chunk: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for character in chunk.chars() {
        if let Some(previous) = previous {
            let boundary = (character.is_ascii_uppercase()
                && (previous.is_ascii_lowercase() || previous.is_ascii_digit()))
                || (character.is_ascii_digit() && previous.is_ascii_alphabetic())
                || (character.is_ascii_alphabetic() && previous.is_ascii_digit());
            if boundary {
                parts.push(std::mem::take(&mut current));
            }
        }
        current.push(character);
        previous = Some(character);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}
