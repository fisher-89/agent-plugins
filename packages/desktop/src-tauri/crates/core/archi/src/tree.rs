//! 文件树投影：磁盘枚举 + 模块根标记（`/architecture` 左栏文件树的数据面）。
//!
//! 纯读盘、**零写盘**、**零 spawn**、零 watch。四条纪律：
//! 1. **归属单源**：归一 / 剥离 / 最长前缀 / 后代计数一律经
//!    [`crate::scan::owner::OwnerIndex`]——本文件 MUST NOT 自带前缀匹配、剥离或归一
//!    实现（否则树与漂移对同一文件给出不同归属）；
//! 2. **枚举单源**：遍历与扩展名判据一律经 [`crate::scan`]（`walk_source_files` /
//!    `is_source_file` / `posix`），排除矩阵一律经 `scan::exclude::ExcludeSet`——树面
//!    = 漂移判定面，MUST NOT 自建遍历或另写扩展名清单；
//! 3. **图单点构建**：模型只经 [`crate::dto::graph_view`] 取（不二次解析 / 二次建图）；
//!    结构诊断非空 → `Err`（停摆态树本就不挂载），空模型是合法起点（全条目 `none`）；
//! 4. **确定性**：节点集合用 `BTreeMap` 承载（天然去重 + 有序），条目按 `path` 字节序
//!    输出——同输入同输出。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::dto::{ArchiFileEntry, ArchiFileTree, FileEntryKind, FileTreeMark};
use crate::scan::exclude::ExcludeSet;
use crate::scan::owner::{Owner, OwnerIndex};
use crate::scan::{is_source_file, posix, walk_source_files};

/// 文件树投影（唯一公共入口）：读模型 → 读架构域配置 `exclude` → 枚举 + 排除 →
/// 模块根表 / 归属 → 目录合成 → `mark` / `owners` / `contains` / `exists` → 字节序。
///
/// 空模型（模型目录缺席或零元素）不报错：条目照常由磁盘枚举得出，`mark` 全 `none`。
pub fn file_tree(root: &Path) -> Result<ArchiFileTree, String> {
    let graph = crate::dto::graph_view(root)?;
    if !graph.diagnostics.is_empty() {
        return Err(crate::dto::graph_diagnostics_error(&graph.diagnostics));
    }
    let known: BTreeSet<String> = graph
        .elements
        .iter()
        .map(|element| element.fqn.clone())
        .collect();
    let config = crate::drift::config::load_config(root, &known)?;
    let exclude = ExcludeSet::build(&config.exclude)?;

    let files = candidate_files(root, &exclude);
    let owners = OwnerIndex::build(&graph.elements);
    let roots = owners.module_roots();
    let nodes = node_set(root, &files, &roots);

    let mut entries: Vec<ArchiFileEntry> = Vec::with_capacity(nodes.len());
    for (path, kind) in nodes {
        let (mark, mark_owners) = mark_of(&path, &roots, &owners);
        let contains = if kind == FileEntryKind::File {
            0
        } else {
            owners.contains_count(&path)
        };
        let exists = exists_of(root, &path, kind, roots.contains_key(path.as_str()));
        entries.push(ArchiFileEntry {
            name: name_of(&path).to_owned(),
            path,
            kind,
            mark,
            owners: mark_owners,
            contains,
            exists,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(ArchiFileTree { entries })
}

/// 枚举面（= 漂移扫描面的 ①②③ 段）：gitignore 感知遍历 → repo 相对 POSIX →
/// 五扩展名过滤 → 排除矩阵（完整矩阵：默认约定在前 + 架构域配置追加）。
fn candidate_files(root: &Path, exclude: &ExcludeSet) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for entry in walk_source_files(root) {
        let Ok(relative) = entry.strip_prefix(root) else {
            continue;
        };
        let rel = posix(&relative.to_string_lossy());
        if !is_source_file(&rel) {
            continue;
        }
        if exclude.matched(&rel).is_some() {
            continue;
        }
        files.push(rel);
    }
    files.sort();
    files.dedup();
    files
}

/// 节点集合 = 枚举文件（`File`）∪ 模块根（盘上探测：目录 / 文件 / 不在盘即 `Dir`）
/// ∪ 两者全部祖先目录（`Dir`）。模块根不过排除矩阵——它是「元素的声明」而非
/// 「枚举结果」，漏掉即让已声明模块在树上静默消失。
fn node_set(
    root: &Path,
    files: &[String],
    roots: &BTreeMap<&str, Vec<&str>>,
) -> BTreeMap<String, FileEntryKind> {
    let mut nodes: BTreeMap<String, FileEntryKind> = BTreeMap::new();
    for file in files {
        for ancestor in ancestors(file) {
            nodes.insert(ancestor.to_owned(), FileEntryKind::Dir);
        }
        nodes.insert(file.clone(), FileEntryKind::File);
    }
    for module_root in roots.keys() {
        for ancestor in ancestors(module_root) {
            nodes.insert(ancestor.to_owned(), FileEntryKind::Dir);
        }
        let kind = if root.join(module_root).is_file() {
            FileEntryKind::File
        } else {
            FileEntryKind::Dir
        };
        nodes.insert((*module_root).to_owned(), kind);
    }
    nodes
}

/// 逐级祖先目录（`a/b/c` → `["a", "a/b"]`；单段路径返回空）。
fn ancestors(path: &str) -> Vec<&str> {
    path.match_indices('/')
        .map(|(index, _)| &path[..index])
        .collect()
}

/// 末段名（无 `/` 时等于入参）。
fn name_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, last)| last)
}

/// 归属定档（判据顺序 ① 模块根表 → ② `OwnerIndex::owner_of` → ③ 后代含根）：
/// 表项恰 1 个 FQN → `elementRoot`、≥ 2 个 → `ambiguous`；否则 `member` /
/// `ambiguous` / `containsElements`（后代含根时）/ `none`。两条 `ambiguous` 来源
/// （同路径多元素声明、同长不同前缀）同走一条分支，源码零特判。
fn mark_of(
    path: &str,
    roots: &BTreeMap<&str, Vec<&str>>,
    owners: &OwnerIndex,
) -> (FileTreeMark, Vec<String>) {
    if let Some(fqns) = roots.get(path) {
        let owned: Vec<String> = fqns.iter().map(|fqn| (*fqn).to_owned()).collect();
        return if owned.len() == 1 {
            (FileTreeMark::ElementRoot, owned)
        } else {
            (FileTreeMark::Ambiguous, owned)
        };
    }
    match owners.owner_of(path) {
        Owner::Element { fqn, .. } => (FileTreeMark::Member, vec![fqn]),
        Owner::Ambiguous(fqns) => (FileTreeMark::Ambiguous, fqns),
        Owner::None => {
            if owners.contains_count(path) > 0 {
                (FileTreeMark::ContainsElements, Vec::new())
            } else {
                (FileTreeMark::None, Vec::new())
            }
        }
    }
}

/// 盘上存在性（只看盘、不掺 `pathNotFound` 一类文档诊断）：仅「`Dir` 且为模块根」
/// 可能为 `false`（声明了但目录不在盘的前向防御分支），其余恒 `true`。
fn exists_of(root: &Path, path: &str, kind: FileEntryKind, is_root: bool) -> bool {
    !(kind == FileEntryKind::Dir && is_root && !root.join(path).is_dir())
}
