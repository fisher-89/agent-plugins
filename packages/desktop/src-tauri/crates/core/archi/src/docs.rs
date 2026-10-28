use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use foundation::layout::{archi_decisions_dir, is_archi_rel_path};
use ignore::WalkBuilder;

use crate::dto::{DocKind, DocRef, DocSource};
use crate::graph::ModelGraph;
use crate::model::Element;
use crate::parse::relative_path;

/// 邻近扫描内置排除目录名（测试 / 构建 / VCS；用户可配键集不在本期）。
const EXCLUDED_DIR_NAMES: [&str; 10] = [
    "__tests__",
    "tests",
    "test",
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    ".svn",
    ".hg",
];

/// markdown 扩展名（文档面唯一收面判据）。
const MARKDOWN_SUFFIX: &str = ".md";

/// 关联结果（条目 + 警告）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocsOutcome {
    /// 关联条目（强度序 → 路径字典序）
    pub docs: Vec<DocRef>,
    /// 文档缺失警告（只报不拦）
    pub warnings: Vec<DocWarning>,
}

/// 文档缺失警告（进程内形态；DTO 侧映射为线面警告）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocWarning {
    /// 归属元素 FQN
    pub element: String,
    /// 缺失的路径（repo 相对）
    pub path: String,
}

/// 单元素三源关联（`explicit` > `adjacent` > `text`；同文件合并取最强；强度序 →
/// 路径字典序稳定序）。
pub fn element_docs(graph: &ModelGraph, root: &Path, fqn: &str) -> DocsOutcome {
    let decisions = read_decisions(root);
    match graph.model().element(fqn) {
        Some(element) => collect_element(element, root, &decisions),
        None => DocsOutcome {
            docs: Vec::new(),
            warnings: Vec::new(),
        },
    }
}

/// 全元素并集（决策记录正文只读一次并按 FQN 分发；去重后按路径字典序）。
pub fn all_docs(graph: &ModelGraph, root: &Path) -> DocsOutcome {
    let decisions = read_decisions(root);
    let mut merged: BTreeMap<String, DocRef> = BTreeMap::new();
    let mut warnings: Vec<DocWarning> = Vec::new();
    for element in &graph.model().elements {
        let outcome = collect_element(element, root, &decisions);
        for doc in outcome.docs {
            merge_doc(&mut merged, doc);
        }
        warnings.extend(outcome.warnings);
    }
    let mut docs: Vec<DocRef> = merged.into_values().collect();
    docs.sort_by(|left, right| left.path.cmp(&right.path));
    warnings.sort_by(|left, right| {
        left.element
            .cmp(&right.element)
            .then_with(|| left.path.cmp(&right.path))
    });
    DocsOutcome { docs, warnings }
}

/// 单元素三源收集（决策记录清单由调用方一次读盘传入）。
fn collect_element(element: &Element, root: &Path, decisions: &[(String, String)]) -> DocsOutcome {
    let mut merged: BTreeMap<String, DocRef> = BTreeMap::new();
    let mut warnings: Vec<DocWarning> = Vec::new();
    for raw in &element.docs {
        let Some(relative) = normalize_rel(raw) else {
            continue;
        };
        // 收集期二次拦截：归一通过仍需复核拼接结果落在 `root` 之内（`join` 遇
        // 绝对路径 / 盘符会丢弃 base，关联集一旦收进 root 外路径即成任意读取面）
        let Some(full) = resolved_within(root, &relative) else {
            continue;
        };
        if full.is_file() {
            merge_doc(&mut merged, make_doc(root, &relative, DocSource::Explicit));
        } else {
            warnings.push(DocWarning {
                element: element.fqn.clone(),
                path: relative,
            });
        }
    }
    for raw in &element.path {
        let Some(relative) = normalize_rel(raw) else {
            continue;
        };
        for path in scan_adjacent(root, &relative) {
            merge_doc(&mut merged, make_doc(root, &path, DocSource::Adjacent));
        }
    }
    for (path, content) in decisions {
        if content.contains(&element.fqn) {
            merge_doc(&mut merged, make_doc(root, path, DocSource::Text));
        }
    }
    let mut docs: Vec<DocRef> = merged.into_values().collect();
    docs.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.path.cmp(&right.path))
    });
    DocsOutcome { docs, warnings }
}

/// 同路径合并：来源取最强（`Explicit` > `Adjacent` > `Text`）。
fn merge_doc(merged: &mut BTreeMap<String, DocRef>, doc: DocRef) {
    match merged.get(&doc.path) {
        Some(existing) if existing.source <= doc.source => {}
        _ => {
            merged.insert(doc.path.clone(), doc);
        }
    }
}

/// 条目组装：`kind` 由落地路径决定（决策记录目录内恒 `Adr`），`title` 取首个
/// 非空行若为 ATX 标题，否则取文件名去扩展名。
fn make_doc(root: &Path, relative: &str, source: DocSource) -> DocRef {
    let kind = if is_decision_path(root, relative) {
        DocKind::Adr
    } else {
        DocKind::Md
    };
    DocRef {
        path: relative.to_owned(),
        title: title_of(root, relative),
        kind,
        source,
    }
}

/// 路径是否落在决策记录目录内（目录前缀由 `foundation` 派生单点）。
fn is_decision_path(root: &Path, relative: &str) -> bool {
    let Some(prefix) = relative_path(root, &archi_decisions_dir(root)) else {
        return false;
    };
    relative == prefix || relative.starts_with(&format!("{prefix}/"))
}

/// 标题取法：首个非空行若为 ATX 标题取标题文本，否则文件名去扩展名。
fn title_of(root: &Path, relative: &str) -> String {
    if let Some(content) =
        resolved_within(root, relative).and_then(|full| fs::read_to_string(full).ok())
    {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(heading) = trimmed.strip_prefix('#') {
                let heading = heading.trim_start_matches('#').trim();
                if !heading.is_empty() {
                    return heading.to_owned();
                }
            }
            break;
        }
    }
    file_stem(relative)
}

/// 文件名去扩展名（末段）：`docs/auth-token.md` → `auth-token`（标题回退与检索
/// 文件名匹配共用同一取法）。
pub(crate) fn file_stem(relative: &str) -> String {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    match name.rsplit_once('.') {
        Some((stem, _)) => stem.to_owned(),
        None => name.to_owned(),
    }
}

/// 邻近扫描（严格限于元素 `path` 覆盖范围）：目录递归 / 单文件精确匹配；
/// 排除内置谓词命中的目录与架构域目录。
fn scan_adjacent(root: &Path, relative: &str) -> Vec<String> {
    let Some(target) = resolved_within(root, relative) else {
        return Vec::new();
    };
    let Ok(metadata) = fs::metadata(&target) else {
        return Vec::new();
    };
    if metadata.is_file() {
        return if relative.ends_with(MARKDOWN_SUFFIX) && !is_excluded(relative) {
            vec![relative.to_owned()]
        } else {
            Vec::new()
        };
    }
    if !metadata.is_dir() {
        return Vec::new();
    }
    let mut docs: Vec<String> = Vec::new();
    for entry in WalkBuilder::new(&target)
        .filter_entry(keep_entry)
        .build()
        .flatten()
    {
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Some(path) = relative_path(root, entry.path()) else {
            continue;
        };
        if !path.ends_with(MARKDOWN_SUFFIX) || is_excluded(&path) || is_archi_rel_path(&path) {
            continue;
        }
        docs.push(path);
    }
    docs.sort();
    docs
}

/// 遍历剪枝谓词：根条目放行，内置排除目录不进入。
fn keep_entry(entry: &ignore::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    match entry.file_name().to_str() {
        Some(name) => !EXCLUDED_DIR_NAMES.contains(&name),
        None => true,
    }
}

/// 路径是否命中内置排除谓词（任一段命中即排除）。
fn is_excluded(relative: &str) -> bool {
    relative
        .split('/')
        .any(|segment| EXCLUDED_DIR_NAMES.contains(&segment))
}

/// 决策记录目录内全部 markdown 的 `(repo 相对路径, 正文)`（按文件名字典序，
/// 只读一次）。
fn read_decisions(root: &Path) -> Vec<(String, String)> {
    let dir = archi_decisions_dir(root);
    let Ok(reader) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<std::path::PathBuf> = reader
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|path| {
            if !path.to_string_lossy().ends_with(MARKDOWN_SUFFIX) {
                return None;
            }
            let relative = relative_path(root, &path)?;
            let content = fs::read_to_string(&path).ok()?;
            Some((relative, content))
        })
        .collect()
}

/// 归一：`\` → `/`、剥前导 `./` 与尾分隔符；空串即跳过。
///
/// **repo 相对路径判据（第一层拦截）**：绝对路径（含 UNC）/ 盘符前缀 / `~` 起始 /
/// 含 `.`、`..`、空段的形态一律拒收——`root.join(绝对路径)` 会替换 base，放行即
/// 成任意文件读取通道（关联集成员校验随之失真）。
fn normalize_rel(raw: &str) -> Option<String> {
    let normalized = raw.replace('\\', "/");
    let mut trimmed: &str = &normalized;
    while let Some(rest) = trimmed.strip_prefix("./") {
        trimmed = rest;
    }
    let trimmed = trimmed.trim_end_matches('/');
    if !is_repo_relative(trimmed) {
        return None;
    }
    Some(trimmed.to_owned())
}

/// repo 相对路径判据（归一与收集两处共用）：`/` 分隔、无绝对前缀、无盘符、无
/// `.` / `..` / 空段。
fn is_repo_relative(relative: &str) -> bool {
    if relative.is_empty() || relative.starts_with('/') || relative.starts_with('~') {
        return false;
    }
    let bytes = relative.as_bytes();
    // 盘符前缀（`C:` / `c:`）—— Windows 绝对路径在 `join` 时同样替换 base
    if bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    !relative
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
}

/// 拼接并复核（第二层拦截）：判据通过后 `join` 到 `root`，再以 `strip_prefix`
/// 复核结果仍在 `root` 之下（判据与 `join` 语义的交叉验证）。
fn resolved_within(root: &Path, relative: &str) -> Option<PathBuf> {
    if !is_repo_relative(relative) {
        return None;
    }
    let joined = root.join(relative);
    // `starts_with` 按分量比较，等价于 `strip_prefix(..).is_ok()`（`join` 遇绝对
    // 路径 / 盘符会丢弃 base，此处复核它没有发生）
    if joined.starts_with(root) {
        Some(joined)
    } else {
        None
    }
}
