//! 归属单点（语言无关）：元素 `path` 条目归一后的**最长前缀匹配**。
//!
//! 三值不做「off 特判」（归属是事实，档位语义由调用侧决定）：
//! - 唯一最长 → [`Owner::Element`]（**含 `off` 元素**——调用侧据此把源文件计入
//!   `unmatchedFiles`，而目标侧仍可作 `to`：`off` 只表示「本模块不参与定级」，
//!   不表示「元素不存在」；`off` 源文件的依赖**照常抽取**，见 `scan` 的归属段）；
//! - 同长多命中 → [`Owner::Ambiguous`]（MUST NOT 任选）；
//! - 无命中 → [`Owner::None`]。

use crate::dto::{ArchiElement, GateTier};

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

/// 单条归属条目（归一后的前缀）。
struct OwnerEntry {
    fqn: String,
    prefix: String,
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
                entries.push(OwnerEntry {
                    fqn: element.fqn.clone(),
                    prefix,
                    gate: element.gate,
                });
            }
        }
        OwnerIndex { entries }
    }

    /// 最长前缀归属：判据 = `rel == prefix` 或 `rel.starts_with(prefix + "/")`
    /// （目录形态），或 `rel == prefix`（文件级 `path`）。
    pub fn owner_of(&self, rel_path: &str) -> Owner {
        let rel = normalize(rel_path);
        let mut best: Option<usize> = None;
        let mut hits: Vec<&OwnerEntry> = Vec::new();
        for entry in &self.entries {
            if !prefix_matches(&rel, &entry.prefix) {
                continue;
            }
            let length = entry.prefix.chars().count();
            match best {
                Some(current) if length < current => {}
                Some(current) if length == current => {
                    if !hits.iter().any(|hit| hit.fqn == entry.fqn) {
                        hits.push(entry);
                    }
                }
                _ => {
                    best = Some(length);
                    hits.clear();
                    hits.push(entry);
                }
            }
        }
        match hits.len() {
            0 => Owner::None,
            1 => Owner::Element {
                fqn: hits[0].fqn.clone(),
                gate: hits[0].gate,
            },
            _ => {
                let mut candidates: Vec<String> = hits.iter().map(|hit| hit.fqn.clone()).collect();
                candidates.sort();
                candidates.dedup();
                Owner::Ambiguous(candidates)
            }
        }
    }
}

/// 前缀判据（目录形态 `prefix + "/"` 与文件级 `path` 的全等形态）。
fn prefix_matches(rel: &str, prefix: &str) -> bool {
    rel == prefix
        || (rel.len() > prefix.len()
            && rel.starts_with(prefix)
            && rel.as_bytes().get(prefix.len()) == Some(&b'/'))
}
