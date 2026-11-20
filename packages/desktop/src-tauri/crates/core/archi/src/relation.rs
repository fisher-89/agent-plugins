//! 单条关系的按需事实取样（`archi_relation` 命令的领域本体）。
//!
//! 三件事：**模型面**（在 [`crate::dto::load`] 的同一份装载里找关系与声明行）→
//! **事实面**（只扫 `from` 元素 `path` 下的源文件，目标归属走既有
//! [`OwnerIndex`]）→ **两桶**（求差调 [`crate::drift::api_buckets`]，与漂移对账共用
//! 同一纯函数）。
//!
//! 边界：纯读盘、**零写盘**、**零缓存**、**零 `Report` 组装**——这是一次按需事实
//! 取样，不是一次检查（不产出 `status` / `passed`，`findings` / `unmatchedFiles` /
//! `unresolved` 都不进响应）。

use std::collections::BTreeSet;
use std::path::Path;

use foundation::cancel::CancelFlag;

use crate::drift::{self, report::ArchiDriftOccurrence, ArchiDriftMode};
use crate::dto::{self, ArchiElement, ArchiRelationDetail, EdgeMark};
use crate::scan::{self, owner::Owner, owner::OwnerIndex, ScanInput};

/// 单条关系的详情（声明 + 事实 + 两桶 + 命中点）。
///
/// 错误面（`String`，与命令面口径一致）：模型结构错（附诊断清单）/ 关系缺席 /
/// 起点元素缺席 / 看护配置非法 / 扫描硬失败——文案点名对象与规模提示。
pub fn relation_detail(root: &Path, from: &str, to: &str) -> Result<ArchiRelationDetail, String> {
    let loaded = dto::load(root);
    if !loaded.diagnostics.is_empty() {
        return Err(dto::diagnostics_error(&loaded.diagnostics));
    }
    let model = loaded.graph.model();
    if model.element(from).is_none() {
        return Err(format!(
            "元素不存在：{from}（模型共 {} 个元素）",
            model.elements.len()
        ));
    }
    let relation = model
        .relations
        .iter()
        .find(|relation| relation.from == from && relation.to == to)
        .ok_or_else(|| {
            format!(
                "关系不存在：{from} → {to}（模型共 {} 条关系）",
                model.relations.len()
            )
        })?;
    let elements: Vec<ArchiElement> = model.elements.iter().map(dto::to_element).collect();

    // 看护配置与 drift 同源（MUST NOT 第二份配置解析）：排除面在取样前生效
    let known: BTreeSet<String> = elements.iter().map(|entry| entry.fqn.clone()).collect();
    let config = drift::config::load_config(root, &known)?;

    // 定向取样：`changed` 白名单的字面语义就是「只扫其中的源文件」
    let changed = from_side_sources(root, &elements, from);
    let outcome = scan::scan(ScanInput {
        root,
        mode: ArchiDriftMode::Incremental,
        changed: &changed,
        exclude: &config.exclude,
        elements: &elements,
        cancel: CancelFlag::never(),
    })
    .map_err(|error| error.message())?;

    // 只取该元素对的命中边：名字并集与命中点都由 `drift` 的同一组口径给出
    let edges: Vec<&scan::DependencyEdge> = outcome
        .edges
        .iter()
        .filter(|edge| edge.from == from && edge.to == to)
        .collect();
    let used = drift::edge_names(&edges);
    let (used_not_declared, declared_not_used) = drift::api_buckets(&relation.apis, &used);
    let occurrences: Vec<ArchiDriftOccurrence> = drift::occurrences_of(&edges);

    Ok(ArchiRelationDetail {
        from: relation.from.clone(),
        to: relation.to.clone(),
        description: relation.description.clone(),
        mark: EdgeMark::None,
        reason: None,
        model_file: relation.file.clone(),
        model_line: relation.line,
        declared: relation.apis.clone(),
        used,
        used_not_declared,
        declared_not_used,
        occurrences,
    })
}

/// `from` 元素 `path` 下的源文件白名单（repo 相对 POSIX、字典序）。
///
/// 归属规则复用 [`OwnerIndex`]（MUST NOT 第二份前缀归一）：单次
/// [`scan::walk_source_files`] 遍历 + 扩展名过滤 + 归属为 `from` 者入白名单。
fn from_side_sources(root: &Path, elements: &[ArchiElement], from: &str) -> Vec<String> {
    let owners = OwnerIndex::build(elements);
    let mut sources: Vec<String> = Vec::new();
    for file in scan::walk_source_files(root) {
        let Ok(rel) = file.strip_prefix(root) else {
            continue;
        };
        let rel = scan::posix(&rel.to_string_lossy());
        if !scan::is_source_file(&rel) {
            continue;
        }
        if let Owner::Element { fqn, .. } = owners.owner_of(&rel) {
            if fqn == from {
                sources.push(rel);
            }
        }
    }
    sources.sort();
    sources.dedup();
    sources
}
