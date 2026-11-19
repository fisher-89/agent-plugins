use crate::api;
use crate::dto::SearchScope;
use crate::graph::ModelGraph;
use crate::model::Element;

/// 检索结果（元素 FQN 清单 + 入口文档路径串）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOutcome {
    /// 命中元素 FQN（字典序）
    pub element_fqns: Vec<String>,
    /// 命中的入口文档路径（`path` 非空元素的在盘载体，路径字典序）
    pub doc: Vec<String>,
}

/// 检索入口：空白 query → `Err`（MUST NOT 返回全量）；`Elements` 时 `doc` 恒空、
/// `Doc` 时 `element_fqns` 恒空。
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
        doc: Vec::new(),
    };
    if scope != SearchScope::Doc {
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
        // 语料 = `path` 非空元素的在盘入口文档载体（与判据面同一集合、同一载体派生）
        outcome.doc = graph
            .model()
            .elements
            .iter()
            .filter(|element| !element.path.is_empty())
            .filter_map(|element| {
                let carrier = api::paths::carrier_path(root, &crate::dto::to_element(element))?;
                carrier.is_file().then(|| api::display(root, &carrier))
            })
            .filter(|path| matches_doc(path, &tokens))
            .collect();
        outcome.doc.sort();
        outcome.doc.dedup();
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

/// 入口文档命中判据：路径与文件名去扩展名（**不读正文**）。
fn matches_doc(doc: &str, tokens: &[String]) -> bool {
    let mut haystack = String::with_capacity(doc.len() + 32);
    haystack.push_str(doc);
    haystack.push('\n');
    haystack.push_str(&file_stem(doc));
    contains_all(&haystack, tokens)
}

/// 文件名去扩展名（末段）：`guides/auth-token.md` → `auth-token`（检索文件名匹配用）。
fn file_stem(relative: &str) -> String {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    match name.rsplit_once('.') {
        Some((stem, _)) => stem.to_owned(),
        None => name.to_owned(),
    }
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
