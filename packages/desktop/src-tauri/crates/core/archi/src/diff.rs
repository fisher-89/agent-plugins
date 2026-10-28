use std::collections::{BTreeMap, BTreeSet};

use crate::dto::GateTier;
use crate::model::Model;

/// 两模型结构差异（全零即等价）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelDiff {
    /// 新增元素 FQN（字典序）
    pub elements_added: Vec<String>,
    /// 删除元素 FQN（字典序）
    pub elements_removed: Vec<String>,
    /// 新增关系数
    pub relations_added: u32,
    /// 删除关系数
    pub relations_removed: u32,
    /// `gate` 变更清单（FQN 字典序）
    pub gate_changes: Vec<GateChange>,
}

/// 单元素 `gate` 变更（旧值 → 新值）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateChange {
    /// 元素 FQN
    pub fqn: String,
    /// 旧档位
    pub from: GateTier,
    /// 新档位
    pub to: GateTier,
}

impl ModelDiff {
    /// 三组皆空（「模型无改动零占位」的判据）。
    pub fn is_empty(&self) -> bool {
        self.elements_added.is_empty()
            && self.elements_removed.is_empty()
            && self.relations_added == 0
            && self.relations_removed == 0
            && self.gate_changes.is_empty()
    }
}

/// 结构 diff：元素身份 = FQN、关系身份 = `(from, to)`（多重集合差，只计数量）、
/// `gate` 变更 = 同名元素档位不等（含缺省 `off` 的显式比较）。
pub fn model_diff(base: &Model, next: &Model) -> ModelDiff {
    let base_fqns: BTreeSet<&str> = base
        .elements
        .iter()
        .map(|element| element.fqn.as_str())
        .collect();
    let next_fqns: BTreeSet<&str> = next
        .elements
        .iter()
        .map(|element| element.fqn.as_str())
        .collect();
    let elements_added: Vec<String> = next_fqns
        .difference(&base_fqns)
        .map(|fqn| (*fqn).to_owned())
        .collect();
    let elements_removed: Vec<String> = base_fqns
        .difference(&next_fqns)
        .map(|fqn| (*fqn).to_owned())
        .collect();

    let base_relations = relation_counts(base);
    let next_relations = relation_counts(next);
    let mut relations_added: u32 = 0;
    let mut relations_removed: u32 = 0;
    for (key, count) in &next_relations {
        let base_count = base_relations.get(key).copied().unwrap_or(0);
        relations_added += count.saturating_sub(base_count);
    }
    for (key, count) in &base_relations {
        let next_count = next_relations.get(key).copied().unwrap_or(0);
        relations_removed += count.saturating_sub(next_count);
    }

    let mut gate_changes: Vec<GateChange> = Vec::new();
    for element in &next.elements {
        let Some(baseline) = base.element(&element.fqn) else {
            continue;
        };
        if baseline.gate != element.gate {
            gate_changes.push(GateChange {
                fqn: element.fqn.clone(),
                from: baseline.gate,
                to: element.gate,
            });
        }
    }
    gate_changes.sort_by(|left, right| left.fqn.cmp(&right.fqn));

    ModelDiff {
        elements_added,
        elements_removed,
        relations_added,
        relations_removed,
        gate_changes,
    }
}

/// 关系多重集合（`(from, to)` → 出现次数）。
fn relation_counts(model: &Model) -> BTreeMap<(String, String), u32> {
    let mut counts: BTreeMap<(String, String), u32> = BTreeMap::new();
    for relation in &model.relations {
        *counts
            .entry((relation.from.clone(), relation.to.clone()))
            .or_insert(0) += 1;
    }
    counts
}
