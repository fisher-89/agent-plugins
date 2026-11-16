//! 归属单点（语言无关）：元素 `path` 条目归一后的**最长前缀匹配**。
//!
//! 三值不做「off 特判」（归属是事实，档位语义由调用侧决定）：
//! - 唯一最长 → [`Owner::Element`]（**含 `off` 元素**——调用侧据此把源文件计入
//!   `unmatchedFiles`，而目标侧仍可作 `to`：`off` 只表示「本模块不参与定级」，
//!   不表示「元素不存在」；`off` 源文件的依赖**照常抽取**，见 `scan` 的归属段）；
//! - 同长多命中 → [`Owner::Ambiguous`]（MUST NOT 任选）；
//! - 无命中 → [`Owner::None`]。
//!
//! **`.i.<ext>` 生成物按三候选并集判定**：生成物落**实现入口同级**，三种落点形态互不
//! 相交——① **整组剥离**（`<name>.i.<ext>` → `<name>`，目录型根下仍在元素根之内：
//! `crates/infra/archi/src/lib.i.rs`）、② **替换扩展名**（`<stem>.i.<ext>` →
//! `<stem>.<ext>`，既有形态 `src/dir.ts` 根对 `src/dir.i.ts`）、③ **主干对齐**
//! （`<name>` 与**文件型 `path` 的去扩展名形态**全等：`views/welcome-view.i.ts` ↔
//! `views/welcome-view.tsx`——文件型根的落点是根的兄弟文件，前两候选都不命中）。故
//! [`strip_candidates`] 返回 1~3 个候选，`owner_of` 对每个候选各跑一遍最长前缀、
//! 合并去重后按前缀长度定档（**MUST NOT 候选短路**，否则 `src/dir.i.ts` 只走一条
//! 路径）。`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字零变化——
//! 非 `.i.<ext>` 路径的归属结果与改动前完全相同。

use std::collections::{BTreeMap, BTreeSet};

use super::is_source_file;
use crate::dto::{ArchiElement, GateTier};

/// 生成物命名后缀标记（`.i.<ext>` 的中间段）。
const GENERATED_MARKER: &str = "i";

/// 生成物可能的源文件扩展名（`.i.rs` / `.i.ts` / `.i.tsx` / `.i.js` / `.i.jsx`）。
const GENERATED_EXTENSIONS: [&str; 5] = ["rs", "ts", "tsx", "js", "jsx"];

/// 归属结果（封闭三值）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// 唯一最长前缀命中
    Element {
        /// 元素 FQN
        fqn: String,
        /// 元素 gate 档位（定级语义由调用侧裁决）
        gate: GateTier,
    },
    /// 同长多命中（候选元素 FQN 字典序）
    Ambiguous(Vec<String>),
    /// 无元素认领
    None,
}

/// 归属索引（构建一次复用）：归一后的 `(元素 FQN, 前缀, gate)` 条目表。
pub struct OwnerIndex {
    entries: Vec<OwnerEntry>,
}

/// 单条归属条目（归一后的前缀 + 文件型 `path` 的去扩展名形态）。
struct OwnerEntry {
    fqn: String,
    prefix: String,
    /// 文件型 `path` 的去扩展名形态（主干对齐候选的比对目标）；非源文件形态 → `None`
    stem: Option<String>,
    gate: GateTier,
}

/// 路径归一：`\` → `/`、剥前导 `./`、去尾 `/`（元素 `path` 与磁盘相对路径同口径）。
fn normalize(path: &str) -> String {
    let unified = path.replace('\\', "/");
    let mut trimmed = unified.as_str();
    while let Some(rest) = trimmed.strip_prefix("./") {
        trimmed = rest;
    }
    trimmed.trim_end_matches('/').to_owned()
}

impl OwnerIndex {
    /// 构建归属索引：只收 `path` 非空元素；条目归一（空条目丢弃）。
    pub fn build(elements: &[ArchiElement]) -> OwnerIndex {
        let mut entries = Vec::new();
        for element in elements {
            for path in &element.path {
                let prefix = normalize(path);
                if prefix.is_empty() {
                    continue;
                }
                // 「文件型 `path`」的判据复用 `scan` 的扩展名白名单（MUST NOT 另写清单）
                let stem = is_source_file(&prefix).then(|| {
                    prefix
                        .rsplit_once('.')
                        .map_or_else(|| prefix.clone(), |(stem, _)| stem.to_owned())
                });
                entries.push(OwnerEntry {
                    fqn: element.fqn.clone(),
                    prefix,
                    stem,
                    gate: element.gate,
                });
            }
        }
        OwnerIndex { entries }
    }

    /// 最长前缀归属：判据 = `rel == prefix` 或 `rel.starts_with(prefix + "/")`
    /// （目录形态），或 `rel == prefix`（文件级 `path`）。
    ///
    /// 生成物候选集合并定档：**三候选并集**（整组剥离 ∪ 替换扩展名 ∪ 主干对齐），每个
    /// 候选各跑一遍最长前缀、合并去重后取最大档；任一条候选都不得提前返回
    /// （`src/dir.i.ts` 的整组剥离候选命中 `src/dir`、替换扩展名候选命中
    /// `src/dir.ts`，短路即与并集语义不符）。同长仍归 [`Owner::Ambiguous`]。
    pub fn owner_of(&self, rel_path: &str) -> Owner {
        let mut merged: BTreeMap<String, (usize, GateTier)> = BTreeMap::new();
        for candidate in strip_candidates(rel_path) {
            for (fqn, length, gate) in self.longest_prefix_hits(&candidate) {
                let slot = merged.entry(fqn).or_insert((length, gate));
                if length > slot.0 {
                    *slot = (length, gate);
                }
            }
        }
        let Some(best) = merged.values().map(|(length, _)| *length).max() else {
            return Owner::None;
        };
        let hits: Vec<(&str, GateTier)> = merged
            .iter()
            .filter(|(_, (length, _))| *length == best)
            .map(|(fqn, (_, gate))| (fqn.as_str(), *gate))
            .collect();
        match hits.as_slice() {
            [(fqn, gate)] => Owner::Element {
                fqn: (*fqn).to_owned(),
                gate: *gate,
            },
            _ => Owner::Ambiguous(hits.iter().map(|(fqn, _)| (*fqn).to_owned()).collect()),
        }
    }

    /// 模块根表：归一路径 → FQN 列表（每表项字典序去重）。`len == 1` 判
    /// `elementRoot`、`len >= 2` 判 `ambiguous`；文件树构造模块根节点的唯一来源。
    pub(crate) fn module_roots(&self) -> BTreeMap<&str, Vec<&str>> {
        let mut roots: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for entry in &self.entries {
            let fqns = roots.entry(entry.prefix.as_str()).or_default();
            if !fqns.contains(&entry.fqn.as_str()) {
                fqns.push(entry.fqn.as_str());
            }
        }
        for fqns in roots.values_mut() {
            fqns.sort_unstable();
        }
        roots
    }

    /// 后代模块数：模块根表中路径严格以 `"{path}/"` 开头的元素数（按 FQN 去重、
    /// 不含自身——自身 `prefix` 恰为 `path`，不满足严格前缀）。与 `mark` 正交。
    pub(crate) fn contains_count(&self, path: &str) -> u32 {
        let target = normalize(path);
        if target.is_empty() {
            return 0;
        }
        let prefix = format!("{target}/");
        let descendants: BTreeSet<&str> = self
            .entries
            .iter()
            .filter(|entry| entry.prefix.starts_with(&prefix))
            .map(|entry| entry.fqn.as_str())
            .collect();
        descendants.len() as u32
    }

    /// 单候选的最长前缀命中集（`best` / `hits` 语义与最长前缀匹配同源）：按命中目标的
    /// 字符数取最大档，档内按 FQN 去重（字典序，同 FQN 多 `path` 条目只计一条）。
    ///
    /// 比对目标由候选种类定：[`StrippedCandidate::Prefix`] 比元素 `path` 归一前缀，
    /// [`StrippedCandidate::Stem`] 比**文件型 `path` 的去扩展名形态**（目录型 `path` 无
    /// 该形态，故不参与主干对齐）。
    fn longest_prefix_hits(&self, candidate: &StrippedCandidate) -> Vec<(String, usize, GateTier)> {
        let rel = match candidate {
            StrippedCandidate::Prefix(text) | StrippedCandidate::Stem(text) => normalize(text),
        };
        let mut best: Option<usize> = None;
        let mut hits: BTreeMap<String, (usize, GateTier)> = BTreeMap::new();
        for entry in &self.entries {
            let target = match candidate {
                StrippedCandidate::Prefix(_) => entry.prefix.as_str(),
                StrippedCandidate::Stem(_) => match entry.stem.as_deref() {
                    Some(stem) => stem,
                    None => continue,
                },
            };
            if !prefix_matches(&rel, target) {
                continue;
            }
            let length = target.chars().count();
            match best {
                Some(current) if length < current => {}
                Some(current) if length == current => {
                    hits.insert(entry.fqn.clone(), (length, entry.gate));
                }
                _ => {
                    best = Some(length);
                    hits.clear();
                    hits.insert(entry.fqn.clone(), (length, entry.gate));
                }
            }
        }
        hits.into_iter()
            .map(|(fqn, (length, gate))| (fqn, length, gate))
            .collect()
    }
}

/// 剥离候选的比对目标（私有；不产生第二份剥离函数）。
///
/// 第三候选的**文本**与第一候选相同（`<name>.i.<ext>` → `<name>`），差异只在比对目标：
/// [`Prefix`](StrippedCandidate::Prefix) 比元素 `path` 的归一前缀（候选 1 / 2），
/// [`Stem`](StrippedCandidate::Stem) 比**文件型 `path` 的去扩展名形态**（候选 3）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum StrippedCandidate {
    /// 整组剥离 / 替换扩展名的结果，比对元素 `path` 归一前缀
    Prefix(String),
    /// 主干对齐的结果，比对文件型 `path` 的去扩展名形态
    Stem(String),
}

/// 生成物候选集：白名单外返回单候选（归一后的原路径，归属与改动前逐值相同）；
/// 末段形如 `<stem>.i.<ext>`（`ext ∈ {rs, ts, tsx, js, jsx}`）时返回三候选——
/// 整组剥离 `<name>`（`archi.i.rs` → `archi`）、替换扩展名 `<stem>.<ext>`
/// （`archi.i.rs` → `archi.rs`）与主干对齐（同文本 `<name>`，但比对目标是文件型
/// `path` 的去扩展名形态）。三种落点形态互不相交（目录型根之内 / 文件型根同目录），
/// 故取并集。
fn strip_candidates(rel_path: &str) -> Vec<StrippedCandidate> {
    let unified = rel_path.replace('\\', "/");
    let segments: Vec<&str> = unified.split('/').collect();
    let Some(last) = segments.last() else {
        return vec![StrippedCandidate::Prefix(unified)];
    };
    let Some((stem, extension)) = last.rsplit_once('.') else {
        return vec![StrippedCandidate::Prefix(unified)];
    };
    if !GENERATED_EXTENSIONS.contains(&extension) {
        return vec![StrippedCandidate::Prefix(unified)];
    }
    let Some((module, marker)) = stem.rsplit_once('.') else {
        return vec![StrippedCandidate::Prefix(unified)];
    };
    if marker != GENERATED_MARKER || module.is_empty() {
        return vec![StrippedCandidate::Prefix(unified)];
    }
    let parent = &segments[..segments.len() - 1];
    let mut stripped: Vec<String> = parent.iter().map(|segment| (*segment).to_owned()).collect();
    stripped.push(module.to_owned());
    let mut replaced: Vec<String> = parent.iter().map(|segment| (*segment).to_owned()).collect();
    replaced.push(format!("{module}.{extension}"));
    let stripped = stripped.join("/");
    vec![
        StrippedCandidate::Prefix(stripped.clone()),
        StrippedCandidate::Prefix(replaced.join("/")),
        StrippedCandidate::Stem(stripped),
    ]
}

/// 前缀判据（目录形态 `prefix + "/"` 与文件级 `path` 的全等形态）。
fn prefix_matches(rel: &str, prefix: &str) -> bool {
    rel == prefix
        || (rel.len() > prefix.len()
            && rel.starts_with(prefix)
            && rel.as_bytes().get(prefix.len()) == Some(&b'/'))
}
