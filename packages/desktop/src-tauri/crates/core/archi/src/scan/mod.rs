//! 扫描编排：源文件枚举（gitignore 感知）→ 排除 → 归属 → 逐文件依赖抽取（两套
//! 解析器）→ 跨元素边聚合 → 报告事实成型。
//!
//! **只出事实，不出结论**：文件归属、按目标聚合的跨元素依赖、排除 / 未解析留痕
//! 与内联豁免标记都在这里；对账、定级、豁免裁决与 `status` 推导归
//! [`crate::drift`]（判定单源）。
//!
//! 边界：纯读盘、**零写盘**、**零 spawn**、**零工作区配置读取**。

pub mod exclude;
pub mod owner;
pub mod rust;
pub mod ts;

use std::collections::BTreeMap;
use std::path::Path;

use foundation::cancel::CancelFlag;
use ignore::WalkBuilder;

use crate::drift::report::{
    ArchiDriftEntry, ArchiDriftKind, ArchiDriftMode, ArchiExcludedFile, ArchiMatchedElement,
    ArchiScanReport, ArchiUnresolvedFile,
};
use crate::drift::CheckError;
use crate::dto::{ArchiElement, GateTier};

use exclude::ExcludeSet;
use owner::{Owner, OwnerIndex};

/// 源文件扩展名（四种 TS/JS + `.rs`）。
const SOURCE_EXTENSIONS: [&str; 5] = ["ts", "tsx", "js", "jsx", "rs"];

/// 内联豁免注释前缀（`// archi-ignore-next-line: <reason>`）。
const INLINE_IGNORE_PREFIX: &str = "// archi-ignore-next-line:";

/// 每多少个文件轮询一次取消标志（检查本身毫秒级，取消只在扫描长尾时命中）。
const CANCEL_POLL_INTERVAL: usize = 16;

/// 两套解析器的共同产出形状（单条 import 事实）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFact {
    /// import 声明原文 specifier（Rust 为规范化后的 use 路径）
    pub specifier: String,
    /// 声明起始行（1 起始）
    pub line: u32,
    /// 是否 type-only（只标注，MUST NOT 因它豁免）
    pub type_only: bool,
}

/// 跨元素依赖事实（已剔除同元素内边；目标不可归属的边不在此列）。
///
/// `from` 可以是 `gate = off` 的元素（`off` 只表示「本模块不参与定级」，不表示
/// 「元素不存在」）——这类边照常产出，定级在 [`crate::drift`] 侧按引入方档位处理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEdge {
    /// 引入方元素 FQN
    pub from: String,
    /// 被引入元素 FQN
    pub to: String,
    /// 命中文件（repo 相对 POSIX）
    pub file: String,
    /// 命中行（1 起始）
    pub line: u32,
    /// 原文 specifier
    pub specifier: String,
    /// 是否 type-only
    pub type_only: bool,
    /// 内联豁免理由（`// archi-ignore-next-line: <reason>`；空 reason 视为未标记）
    pub inline_ignore: Option<String>,
}

/// 扫描入参（观察根 + 模式 + 变更集 + 配置排除 + 取消标志 + 已建好的模型元素表）。
///
/// `elements` 由调用方（[`crate::drift::check`]）经 `dto::graph_view` 单点取来后
/// 传入——扫描面 MUST NOT 二次读模型、二次建图（图单点构建纪律）。
pub struct ScanInput<'a> {
    /// 观察根（模型与源码同根）
    pub root: &'a Path,
    /// 检查模式
    pub mode: ArchiDriftMode,
    /// 变更集（增量模式只扫其中的源文件；全量模式忽略）
    pub changed: &'a [String],
    /// 架构域配置的 `exclude` 追加项（默认约定之外的追加，不替换）
    pub exclude: &'a [String],
    /// 模型元素表（归属索引构建源）
    pub elements: &'a [ArchiElement],
    /// 取消标志（枚举与解析循环内轮询）
    pub cancel: CancelFlag,
}

/// 扫描产出（事实面）。
pub struct ScanOutcome {
    /// 扫描面事实
    pub report: ArchiScanReport,
    /// 扫描侧留痕条目（`ambiguousOwner` 事实；`drift` 只按档位归置，不重算归属）
    pub findings: Vec<ArchiDriftEntry>,
    /// 跨元素依赖事实（已剔除同元素内边）
    pub edges: Vec<DependencyEdge>,
}

/// 扫描一次：枚举 → 排除 → 归属 → 抽取 → 聚合。
///
/// 取消置位 → `Err(CheckError::Stopped)`（遍历 / 解析循环内轮询）；配置排除非法
/// → `Err(CheckError::Scan)`。
pub fn scan(input: ScanInput<'_>) -> Result<ScanOutcome, CheckError> {
    let exclusions = ExcludeSet::build(input.exclude).map_err(CheckError::Scan)?;
    let changed: std::collections::BTreeSet<String> =
        input.changed.iter().map(|entry| posix(entry)).collect();
    let changed_files = if input.mode == ArchiDriftMode::Incremental {
        changed.iter().filter(|entry| is_source_file(entry)).count() as u32
    } else {
        0
    };

    // ① 枚举（gitignore 感知；只收五种源文件扩展名）
    let mut candidates: Vec<String> = Vec::new();
    for (index, entry) in walk_source_files(input.root).into_iter().enumerate() {
        if index.is_multiple_of(CANCEL_POLL_INTERVAL) && input.cancel.is_set() {
            return Err(CheckError::Stopped);
        }
        let rel = match entry.strip_prefix(input.root) {
            Ok(rel) => posix(&rel.to_string_lossy()),
            Err(_) => continue,
        };
        if !is_source_file(&rel) {
            continue;
        }
        // ② 模式过滤（增量只留变更集内的源文件；全量忽略变更集）
        if input.mode == ArchiDriftMode::Incremental && !changed.contains(&rel) {
            continue;
        }
        candidates.push(rel);
    }
    candidates.sort();
    candidates.dedup();

    // ③ 排除矩阵（逐文件匹配 → 逐条留痕）
    let mut excluded: Vec<ArchiExcludedFile> = Vec::new();
    let mut kept: Vec<String> = Vec::new();
    for rel in candidates {
        match exclusions.matched(&rel) {
            Some(rule) => excluded.push(ArchiExcludedFile { file: rel, rule }),
            None => kept.push(rel),
        }
    }

    // ④ 归属（唯一最长前缀）：`off` 元素覆盖的文件计入 `unmatchedFiles[]`（本模块
    //    不参与定级），但**依赖照常抽取**——抽取是事实面、定级才是结论面。把 `off`
    //    折进「不抽取」会让 gate 全 `off` 的工程把所有声明边误报成「声明未兑现」。
    let owners = OwnerIndex::build(input.elements);
    let mut unmatched_files: Vec<String> = Vec::new();
    let mut findings: Vec<ArchiDriftEntry> = Vec::new();
    let mut matched_files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut owned: Vec<(String, String)> = Vec::new();
    for rel in &kept {
        match owners.owner_of(rel) {
            Owner::Element { fqn, gate } => {
                if gate == GateTier::Off {
                    // `off` = 本模块不参与定级 → 与无主同视（零违例）；依赖仍抽取
                    unmatched_files.push(rel.clone());
                } else {
                    matched_files
                        .entry(fqn.clone())
                        .or_default()
                        .push(rel.clone());
                }
                owned.push((rel.clone(), fqn));
            }
            Owner::Ambiguous(candidates) => {
                unmatched_files.push(rel.clone());
                findings.push(ArchiDriftEntry {
                    kind: ArchiDriftKind::AmbiguousOwner,
                    from: None,
                    to: None,
                    gate: None,
                    message: format!(
                        "归属不确定：{} 同时命中 {} 个元素的同长 path 前缀（{}）——不任选，\
                         不参与判定",
                        rel,
                        candidates.len(),
                        candidates.join(" / ")
                    ),
                    occurrences: vec![crate::drift::report::ArchiDriftOccurrence {
                        file: rel.clone(),
                        line: 0,
                        specifier: None,
                        type_only: false,
                    }],
                });
            }
            Owner::None => unmatched_files.push(rel.clone()),
        }
    }

    // ⑤ 逐文件抽取（按扩展名选解析器）→ specifier 落盘 → 目标归属 → 成边
    let mut ts = ts::TsResolver::new(input.root);
    let rust = rust::RustResolver::new(input.root).map_err(CheckError::Scan)?;
    let mut unresolved: Vec<ArchiUnresolvedFile> = rust.unresolved().to_vec();
    let mut edges: Vec<DependencyEdge> = Vec::new();
    for (position, (rel, from)) in owned.iter().enumerate() {
        if position % CANCEL_POLL_INTERVAL == 0 && input.cancel.is_set() {
            return Err(CheckError::Stopped);
        }
        let full = input.root.join(rel);
        let source = match std::fs::read_to_string(&full) {
            Ok(source) => source,
            Err(error) => {
                unresolved.push(ArchiUnresolvedFile {
                    file: rel.clone(),
                    specifier: None,
                    message: format!("源文件读取失败：{error}"),
                });
                continue;
            }
        };
        let (facts, mut file_unresolved) = if rel.ends_with(".rs") {
            rust.extract(rel, &source)
        } else {
            ts.extract(rel, &source)
        };
        unresolved.append(&mut file_unresolved);
        for fact in facts {
            let Some(target_rel) = resolve(&mut ts, &rust, rel, &fact.specifier) else {
                continue;
            };
            let to = match owners.owner_of(&target_rel) {
                // 目标归属：唯一命中即可作 `to`（含 `off` 元素——只表示本模块不
                // 参与定级，不表示元素不存在；定级只看引入方）
                Owner::Element { fqn, .. } => fqn,
                // 目标不可归属（不确定 / 无命中）→ 丢弃该边（看不见的依赖不该
                // 变成红点；来源文件面已在 unmatched 留痕）
                Owner::Ambiguous(_) | Owner::None => continue,
            };
            if &to == from {
                // 同元素内不报（跨元素边界才是判定对象）
                continue;
            }
            edges.push(DependencyEdge {
                from: from.clone(),
                to,
                file: rel.clone(),
                line: fact.line,
                specifier: fact.specifier.clone(),
                type_only: fact.type_only,
                inline_ignore: inline_ignore(&source, fact.line),
            });
        }
    }

    // ⑥ 归属聚合（元素 FQN 字典序、`files` 路径字典序）
    let matched: Vec<ArchiMatchedElement> = matched_files
        .into_iter()
        .map(|(element, mut files)| {
            files.sort();
            files.dedup();
            ArchiMatchedElement { element, files }
        })
        .collect();
    unmatched_files.sort();
    unmatched_files.dedup();

    Ok(ScanOutcome {
        report: ArchiScanReport {
            mode: input.mode,
            changed_files,
            scanned_files: kept.len() as u32,
            excluded,
            unmatched_files,
            matched,
            unresolved,
        },
        findings,
        edges,
    })
}

/// 内联豁免判定：取该行与其上一行（1 起始），`trim` 后匹配前缀
/// `// archi-ignore-next-line:`，冒号后非空即 reason（空 reason 视为未标记）。
pub(crate) fn inline_ignore(source: &str, line: u32) -> Option<String> {
    if line == 0 {
        return None;
    }
    let lines: Vec<&str> = source.lines().collect();
    let index = line as usize - 1;
    // 该行与其上一行（顺序：上一行优先——注释写在 import 的上一行是主用法）
    for candidate in [index.checked_sub(1), Some(index)].into_iter().flatten() {
        let Some(text) = lines.get(candidate) else {
            continue;
        };
        let trimmed = text.trim();
        if let Some(rest) = trimmed.strip_prefix(INLINE_IGNORE_PREFIX) {
            let reason = rest.trim();
            if !reason.is_empty() {
                return Some(reason.to_owned());
            }
        }
    }
    None
}

/// gitignore 感知的目录遍历（跳过隐藏项与构建 / 依赖产物树）。MUST NOT 逐目录
/// `read_dir` 自建遍历。
fn walk_source_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .parents(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                "target" | "node_modules" | "dist" | "build" | ".git"
            )
        })
        .build();
    for entry in walker.flatten() {
        if entry.file_type().is_some_and(|kind| kind.is_file()) {
            files.push(entry.into_path());
        }
    }
    files
}

/// 扩展名是否属于扫描面（四种 TS/JS + `.rs`）。
fn is_source_file(rel: &str) -> bool {
    match rel.rsplit_once('.') {
        Some((_, extension)) => SOURCE_EXTENSIONS.contains(&extension),
        None => false,
    }
}

/// 路径归一为 repo 相对 POSIX 形态（`\` → `/`、剥前导 `./`）。
fn posix(path: &str) -> String {
    let unified = path.replace('\\', "/");
    let mut trimmed = unified.as_str();
    while let Some(rest) = trimmed.strip_prefix("./") {
        trimmed = rest;
    }
    trimmed.to_owned()
}

/// 按扩展名分派两套解析器的 specifier → **repo 相对 POSIX 路径**（不在观察根内
/// → `None`：越界依赖不进判定面）。
fn resolve(
    ts: &mut ts::TsResolver,
    rust: &rust::RustResolver,
    file_rel: &str,
    specifier: &str,
) -> Option<String> {
    if file_rel.ends_with(".rs") {
        rust.resolve(file_rel, specifier)
    } else {
        ts.resolve(file_rel, specifier)
    }
}

#[cfg(test)]
mod exclude_test;
#[cfg(test)]
mod mod_test;
#[cfg(test)]
mod owner_test;
#[cfg(test)]
mod rust_test;
#[cfg(test)]
mod ts_test;
