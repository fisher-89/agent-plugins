//! 写回原语族：纯函数 `apply_*`（源文本进、改写后的完整源文本出）与计划函数
//! `plan_*`（读盘定位 + 纯函数 + 待落盘产物 [`ModelWrite`] + 反馈面 [`WriteOutcome`]）。
//!
//! 三层形状固定：`apply_*` 只碰文本（零文件系统）；`plan_*` 只读盘
//! （`parse_dir` 结构诊断前置 → 存在性 / 落点 / 端点前置 → 读源 → `expect` → `apply_*`
//! → 填 `hash` → 装反馈面）；反馈面（[`warnings`]）只重述既有判据面的词。
//! **零写盘、零子进程、零 Tauri**——模型落盘不在本模块（落盘单点是 infra 侧的原子写，
//! 壳层命令面与 agent 写工具共用一份实现）。
//!
//! 产出顺序固定为「改 AST → 渲染 → 表头计数回填 → 重解析自证零诊断 → 回读逐字段
//! 一致」，且被自证 / 被回读的文本**逐字等于**交出的 `content`（MUST NOT 先验后改）。
//! 渲染经 `toml_edit`：既有键原位不动（更新时沿用原值的 decor），新键追加到元素表尾，
//! 新建元素按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`
//! 规范序整表写出。
//!
//! **内联出边**：[`ElementSpec::relations`] 在一次元素写回里批量 upsert / delete 出边，
//! 四条约束——同批 `to` 唯一、`action = delete` 项禁带字段、内联边只作用于目标文件
//! （已声明在他处即 [`EditErrorKind::RelationOwnedElsewhere`]）、未列出的既有边零改动。
//!
//! **落点四情形**：既有元素落其声明文件（不同 `file` 即拒绝）；新建省略 `file` 落**父
//! 元素**文件；单段 FQN 省略 `file` 即拒绝；给了 `file` 而目标文件缺席必须同时给
//! [`ElementSpec::create`]（新文件表头由本模块按写后重解析计数生成）。
//!
//! **删除与级联**：删除纯函数只有一个形态 [`apply_element_delete`]（[`DeleteSet`] =
//! 元素集 + 关系对集；单元素删除是集合恰一个元素的退化形）。`cascade` 只承诺**同一模型
//! 文件内**的子树与出入边闭包，闭包跨文件即 [`EditErrorKind::CascadeCrossFile`]。
//!
//! 关系面的落点 = 既有 `(from, to)` 的**声明文件**（可以不是 `from` 元素文件）、新增
//! 关系的 `from` 元素文件；同一 `(from, to)` 多处声明（模型面跨文件 / 同一源内）即
//! fail-closed——改哪条、删哪条都是静默裁决。
//!
//! 三态语义：可清空位以 `Option<Option<T>>`（或「`Some` 空集合」同构）表达——`None`
//! 不动、`Some(None)` 删键（`path` / `metadata` / `apis` 以 `Some(空集合)` 表同一意图）、
//! `Some(Some(v))` 设值；封闭枚举位（`kind` / `gate` / `api`）无清空臂。写面 MUST NOT
//! 产出被 parser 判 `invalidValue` 的形态（渲染空数组即一例）——值面 / 形态判据全归
//! parser，越界值经自证链变成 `RenderSelfCheck`。

mod messages;
pub mod warnings;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use foundation::layout::archi_graph_model_dir;
use serde::Serialize;
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike, Value};

use crate::drift::file_hash_of;
use crate::dto::{ApiStatus, ElementKind, GateTier};
use crate::graph::ModelGraph;
use crate::model::{Diagnostic, Element, Model, Relation};
use crate::parse::{self, SourceParser, TomlParser};

pub use messages::{EditError, EditErrorKind};
pub use warnings::{delete_warnings, upsert_warnings, EditWarning};

use messages::error;

/// 待落盘产物（绝对路径 + 全文 + 写后内容哈希）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelWrite {
    /// 目标模型文件（绝对路径）
    pub file: PathBuf,
    /// 待落盘全文（自证与回读的对象逐字等于它）
    pub content: String,
    /// 写后内容哈希（与 [`crate::drift::file_hash`] 同口径，可直接作下一次 `expect`）
    pub hash: String,
}

/// 元素写回规格：`fqn` + 可写字段的三态 + `kind`（新建判别）+ 内联出边 + 落点 + 乐观锁。
#[derive(Debug, Clone, Default)]
pub struct ElementSpec {
    /// 目标元素完整限定名
    pub fqn: String,
    /// **新建意图**（新建必填）：在案元素给了即 `ElementAlreadyExists`
    pub kind: Option<ElementKind>,
    /// gate 档位（无清空臂：`Some(Off)` 是显式化缺省）
    pub gate: Option<GateTier>,
    /// 契约状态指针（无清空臂）
    pub api: Option<ApiStatus>,
    /// 实现入口指针：`Some(None)` 删键
    pub entry: Option<Option<String>>,
    /// 覆盖路径：`Some(空集合)` 删键
    pub path: Option<Vec<String>>,
    /// 入口文档指针：`Some(None)` 删键
    pub doc: Option<Option<String>>,
    /// 一行摘要：`Some(None)` 删键
    pub summary: Option<Option<String>>,
    /// 自由元数据（内联表写出）：`Some(空集合)` 删键
    pub metadata: Option<BTreeMap<String, String>>,
    /// 内联出边（省略 / 空数组 = 出边不动；批量四条见模块 doc）
    pub relations: Vec<RelationSpec>,
    /// 新建落点（repo 相对、模型声明目录一级 `*.toml`）；既有元素只允许同值
    pub file: Option<String>,
    /// 新建文件的显式许可（目标文件缺席而给了 `file` 时必须为 `true`）
    pub create: bool,
    /// 目标文件**当前**内容哈希（`None` = 不校验）
    pub expect: Option<String>,
}

/// 写面动作（封闭二值）：内联出边的 `action` 与两写工具的顶层 `action` 共用这一份词表。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum WriteAction {
    /// 建或改（缺省）
    #[default]
    Upsert,
    /// 删
    Delete,
}

impl WriteAction {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            WriteAction::Upsert => "upsert",
            WriteAction::Delete => "delete",
        }
    }

    /// 全部合法值（清单单点：参数 `enum` 与执行体值域检查同源）。
    pub const ALL: [WriteAction; 2] = [WriteAction::Upsert, WriteAction::Delete];

    /// 线格式词 → 值（词表外 `None`）。
    pub fn parse(raw: &str) -> Option<WriteAction> {
        WriteAction::ALL
            .into_iter()
            .find(|action| action.as_str() == raw)
    }
}

/// 内联出边的唯一入参形态：`to` + 三态字段（`desc` / `apis`）+ 动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationSpec {
    /// 目标元素 FQN
    pub to: String,
    /// 关系说明：`None` 不动、`Some(None)` 删键
    pub desc: Option<Option<String>>,
    /// 关系的 apis：`None` 不动、`Some(空集合)` 删键
    pub apis: Option<Vec<String>>,
    /// 本条边的动作（缺省 `Upsert`）
    pub action: WriteAction,
}

/// 删除动作的唯一入参形态：待删元素集 + 待删关系对集。
///
/// 单元素删除只是「集合恰含一个 FQN」的退化形——MUST NOT 另有第二种删除形态。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeleteSet {
    /// 待删元素 FQN
    pub elements: BTreeSet<String>,
    /// 待删关系对 `(from, to)`
    pub relations: BTreeSet<(String, String)>,
}

impl DeleteSet {
    /// 单元素退化形。
    pub fn of(fqn: impl Into<String>) -> DeleteSet {
        let mut elements = BTreeSet::new();
        elements.insert(fqn.into());
        DeleteSet {
            elements,
            relations: BTreeSet::new(),
        }
    }
}

/// 元素 / 关系计数（本文件 `counts` 与全模型 `model` 共用一形）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct WriteCounts {
    /// 元素条数
    pub elements: usize,
    /// 关系条数
    pub relations: usize,
}

/// 内联出边四态计数（四者之和恒等于入参 `relations` 长度）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RelationTally {
    /// 新建
    pub created: usize,
    /// 改值
    pub updated: usize,
    /// 删除
    pub deleted: usize,
    /// 值已相同（幂等）
    pub unchanged: usize,
}

/// 计划层产物：待落盘三元组 + 反馈面（`warnings` 恒在场，空数组 = 无异常）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    /// 待落盘产物（绝对路径 + 全文 + 写后哈希）
    pub write: ModelWrite,
    /// 已落盘、未阻断的事实（七条判据的子集；关系写面恒空）
    pub warnings: Vec<EditWarning>,
    /// 实际变化的参数面字段名（空 = 幂等）
    pub changed: Vec<String>,
    /// 本次是否新建（幂等凭据）
    pub created: bool,
    /// 内联出边四态计数
    pub relations: RelationTally,
    /// 写后本文件计数（重解析结果）
    pub counts: WriteCounts,
    /// 全模型计数（前值 ± 本次增量）
    pub model: WriteCounts,
}

// ---------------------------------------------------------------------------
// 纯函数层（零文件系统触碰）
// ---------------------------------------------------------------------------

/// 单文件元素 upsert：元素缺席 ⇒ 按 `kind` 新建；在案 + `kind` 有值 ⇒
/// `ElementAlreadyExists`；在案则逐位三态改写（既有键序与既有键的 decor 不动）。
///
/// 元素字段写回之后按 [`RelationSpec::action`] 逐条 upsert / delete 内联出边，未列出的
/// 既有边零改动；被回读的对象是**同一份**交付文本（自证与回读共用 [`finish`] 的收口）。
pub fn apply_element_spec(source: &str, spec: &ElementSpec) -> Result<String, EditError> {
    check_relation_batch(&spec.relations)?;
    let mut document = parse_source(source)?;
    let declared = document
        .as_table()
        .get("element")
        .and_then(Item::as_table_like)
        .is_some_and(|table| table.contains_key(&spec.fqn));
    if declared {
        if spec.kind.is_some() {
            return Err(error(
                EditErrorKind::ElementAlreadyExists,
                format!("{} 已声明——改既有元素请省去 kind", spec.fqn),
            ));
        }
    } else {
        check_create(&spec.fqn, spec.kind)?;
    }
    let table = element_table_mut(&mut document)?;
    if declared {
        let entry = table
            .get_mut(&spec.fqn)
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| {
                error(
                    EditErrorKind::ElementNotTable,
                    format!("{}（无法写回）", spec.fqn),
                )
            })?;
        write_element_fields(entry, spec);
    } else {
        let mut entry = Table::new();
        write_element_fields(&mut entry, spec);
        table.insert(&spec.fqn, Item::Table(entry));
    }
    let expected = apply_inline_relations(&mut document, &spec.fqn, &spec.relations)?;
    finish(document, |model| {
        verify_element(model, spec)?;
        verify_inline_relations(model, &spec.fqn, &spec.relations)?;
        verify_relation_count(model, expected)
    })
}

/// 单文件删除：按集合删关系对与元素表。
///
/// 缺 `element` 表 ⇒ `ElementTableMissing`；元素刻意 / 关系缺席 ⇒ `ElementNotFound` /
/// `RelationNotFound`（事实段列该文件现有元素 / 关系对）。子元素与引用前置归
/// [`plan_element_delete`]（跨文件、级联闭包）。
pub fn apply_element_delete(source: &str, set: &DeleteSet) -> Result<String, EditError> {
    let mut document = parse_source(source)?;
    if !set.relations.is_empty() {
        let entries = relation_entries_mut(&mut document)?;
        let mut missing: Vec<String> = Vec::new();
        for (from, to) in &set.relations {
            let hits = relation_indexes(entries, from, to);
            if hits.len() >= 2 {
                return Err(relation_duplicated_in_source(from, to, hits.len()));
            }
            match hits.first().copied() {
                Some(index) => {
                    entries.remove(index);
                }
                None => missing.push(relation_pair_of(from, to)),
            }
        }
        if !missing.is_empty() {
            return Err(relation_absent_error(entries, &missing.join(" / ")));
        }
    }
    if !set.elements.is_empty() {
        let table = document
            .as_table_mut()
            .get_mut("element")
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| error(EditErrorKind::ElementTableMissing, "无从定位元素"))?;
        let mut missing: Vec<String> = Vec::new();
        for fqn in &set.elements {
            if table.remove(fqn).is_none() {
                missing.push(fqn.clone());
            }
        }
        if !missing.is_empty() {
            let existing: Vec<&str> = table.iter().map(|(key, _)| key).collect();
            return Err(error(
                EditErrorKind::ElementNotFound,
                format!(
                    "{}（现有元素：{}）",
                    missing.join(" / "),
                    existing.join(" / ")
                ),
            ));
        }
    }
    finish(document, |model| {
        for fqn in &set.elements {
            if model.element(fqn).is_some() {
                return Err(error(
                    EditErrorKind::ReadBackMismatch,
                    format!("{fqn} 仍可回读——未删净"),
                ));
            }
        }
        for (from, to) in &set.relations {
            verify_relation_absent(model, from, to)?;
        }
        Ok(())
    })
}

/// 单文件关系 upsert：命中 `(from, to)` 走原地三态改写（条数不变），缺席则追加一条
/// `[[relation]]`（键缺席 ⇒ 新建条目数组，不报缺表）。同一源内该对多处声明 ⇒
/// `RelationDuplicated`（改哪条都是静默裁决）。`apis` 的 `Some(&[])` 翻成删键
/// （MUST NOT 渲染空数组——空数组与省略同义，parser 判 `invalidValue`）。
pub fn apply_relation_upsert(
    source: &str,
    from: &str,
    to: &str,
    desc: Option<Option<&str>>,
    apis: Option<&[String]>,
) -> Result<String, EditError> {
    let mut document = parse_source(source)?;
    let entries = relation_entries_mut(&mut document)?;
    let before = entries.len();
    let hits = relation_indexes(entries, from, to);
    if hits.len() >= 2 {
        return Err(relation_duplicated_in_source(from, to, hits.len()));
    }
    let expected = match hits.first() {
        Some(&index) => {
            if let Some(table) = entries.get_mut(index) {
                write_relation_fields(table, desc, apis);
            }
            before
        }
        None => {
            entries.push(new_relation_table(from, to, desc, apis));
            before + 1
        }
    };
    finish(document, |model| {
        verify_relation_upsert(model, from, to, desc, apis)?;
        verify_relation_count(model, expected)
    })
}

/// 单文件删关系条目：命中**唯一**条目即删该条，缺席 ⇒ `RelationNotFound`（事实段列该
/// 文件现有关系对）；同一源内该对多处声明 ⇒ `RelationDuplicated`（一次删多条是静默
/// 裁决）。
pub fn apply_relation_delete(source: &str, from: &str, to: &str) -> Result<String, EditError> {
    let mut document = parse_source(source)?;
    let entries = relation_entries_mut(&mut document)?;
    let before = entries.len();
    let hits = relation_indexes(entries, from, to);
    if hits.len() >= 2 {
        return Err(relation_duplicated_in_source(from, to, hits.len()));
    }
    let Some(index) = hits.first().copied() else {
        return Err(relation_absent_error(entries, &relation_pair_of(from, to)));
    };
    entries.remove(index);
    finish(document, |model| {
        verify_relation_absent(model, from, to)?;
        verify_relation_count(model, before - 1)
    })
}

// ---------------------------------------------------------------------------
// 计划层（读盘、零写盘）
// ---------------------------------------------------------------------------

/// 元素 upsert 的计划：定位 + 前置 + 读源 + `expect` + [`apply_element_spec`] + `hash`
/// + 反馈面（`changed` / `created` / `relations` / `counts` / `model` / `warnings`）。
///
/// 落点四情形（设计 D-4）：既有元素落其声明文件；新建省略 `file` 落父元素文件；单段
/// FQN 省略 `file` 即拒绝；给了 `file` 而目标文件缺席必须同时给 `create`——新文件表头由
/// 本函数按写后重解析计数生成后再自证。
pub fn plan_element_upsert(root: &Path, spec: &ElementSpec) -> Result<WriteOutcome, EditError> {
    let graph = load_model(root)?;
    let model = graph.model();
    let existing = model.element(&spec.fqn);
    if let (Some(element), Some(_)) = (existing, spec.kind) {
        return Err(error(
            EditErrorKind::ElementAlreadyExists,
            format!(
                "{} 已声明于 {}——改既有元素请省去 kind",
                spec.fqn, element.file
            ),
        ));
    }
    let creating = existing.is_none();
    if creating {
        check_create(&spec.fqn, spec.kind)?;
    }
    let target = check_target(
        root,
        &spec.fqn,
        spec.file.as_deref(),
        spec.create,
        existing,
        model,
    )?;
    check_relation_batch(&spec.relations)?;
    check_inline_relations(root, model, &spec.fqn, &target.file, &spec.relations)?;
    let changed = changed_fields(existing, spec);
    let tally = relation_tally(model, &spec.fqn, &spec.relations);
    let source = read_source(&target.file, creating)?;
    check_expect(&target.file, &source, spec.expect.as_deref())?;
    let body = apply_element_spec(&source, spec)?;
    let content = if target.fresh {
        new_file_content(&target.file, &body)?
    } else {
        body
    };
    let write = write_of(target.file, &content);
    let written = recount(&write.content)?;
    let counts = counts_of(&written);
    let warnings = written
        .element(&spec.fqn)
        .map(|element| upsert_warnings(root, element, &changed, creating))
        .unwrap_or_default();
    Ok(WriteOutcome {
        write,
        warnings,
        changed,
        created: creating,
        relations: tally,
        counts,
        model: model_counts(model, element_delta(creating), relation_delta(tally)),
    })
}

/// 元素删除的计划：元素必须存在。
///
/// `cascade = false`（缺省）有直接子元素 / 被任一关系引用即拒绝（fail-closed，措辞逐字
/// 保留）；`cascade = true` 先算**同一模型文件内**的闭包（子树 ∪ 出入边），跨文件即
/// [`EditErrorKind::CascadeCrossFile`] 且零文本产出。
pub fn plan_element_delete(
    root: &Path,
    fqn: &str,
    cascade: bool,
    expect: Option<&str>,
) -> Result<WriteOutcome, EditError> {
    let graph = load_model(root)?;
    let model = graph.model();
    let element = model.element(fqn).ok_or_else(|| {
        error(
            EditErrorKind::ElementNotFound,
            format!("{fqn}（模型共 {} 个元素）", model.elements.len()),
        )
    })?;
    let declared_file = element.file.clone();
    let set = if cascade {
        cascade_set(&graph, fqn, &declared_file)?
    } else {
        let children = graph.children(fqn);
        if !children.is_empty() {
            return Err(error(
                EditErrorKind::DeleteHasChildren,
                format!("{fqn} 的子元素：{}", children.join(" / ")),
            ));
        }
        let blocking: BTreeSet<String> = graph
            .depends_on(fqn)
            .into_iter()
            .chain(graph.depended_by(fqn))
            .map(relation_pair)
            .collect();
        if !blocking.is_empty() {
            return Err(error(
                EditErrorKind::DeleteHasRelations,
                format!(
                    "{fqn}：{}",
                    blocking.into_iter().collect::<Vec<String>>().join(" / ")
                ),
            ));
        }
        DeleteSet::of(fqn)
    };
    let file = root.join(&declared_file);
    let source = read_source(&file, false)?;
    check_expect(&file, &source, expect)?;
    let content = apply_element_delete(&source, &set)?;
    let write = write_of(file, &content);
    let written = recount(&write.content)?;
    Ok(WriteOutcome {
        write,
        warnings: delete_warnings(root, fqn),
        changed: Vec::new(),
        created: false,
        relations: RelationTally::default(),
        counts: counts_of(&written),
        model: model_counts(
            model,
            -(set.elements.len() as isize),
            -(set.relations.len() as isize),
        ),
    })
}

/// 关系 upsert 的计划：两端点须在案；落点 = 既有 `(from, to)` 的**声明文件**（可以
/// 不是 `from` 元素文件）、新增关系落 `from` 元素声明文件；模型面多处声明 fail-closed。
///
/// 反馈面薄形：`warnings` 恒空、`relations` 计一条、`changed` 为 `desc` / `apis` 的子集。
pub fn plan_relation_upsert(
    root: &Path,
    from: &str,
    to: &str,
    desc: Option<Option<&str>>,
    apis: Option<&[String]>,
    expect: Option<&str>,
) -> Result<WriteOutcome, EditError> {
    let graph = load_model(root)?;
    let model = graph.model();
    let (from_element, _) = endpoints(model, from, to)?;
    let hits = relation_hits(model, from, to);
    check_relation_unique(&hits)?;
    let file = match hits.first() {
        Some(relation) => root.join(&relation.file),
        None => root.join(&from_element.file),
    };
    let created = hits.is_empty();
    let changed = relation_changed(hits.first().copied(), desc, apis);
    let source = read_source(&file, false)?;
    check_expect(&file, &source, expect)?;
    let content = apply_relation_upsert(&source, from, to, desc, apis)?;
    let write = write_of(file, &content);
    let written = recount(&write.content)?;
    Ok(WriteOutcome {
        write,
        warnings: Vec::new(),
        changed: changed.clone(),
        created,
        relations: RelationTally {
            created: usize::from(created),
            updated: usize::from(!created && !changed.is_empty()),
            deleted: 0,
            unchanged: usize::from(!created && changed.is_empty()),
        },
        counts: counts_of(&written),
        model: model_counts(model, 0, isize::from(created)),
    })
}

/// 关系删除的计划：两端点须在案；既有 `(from, to)` 的**声明文件**即落点（可以不是
/// `from` 元素文件）；模型面缺席 / 多处声明都在读盘前 fail-closed。
///
/// 反馈面同薄形：`warnings` 恒空、`relations` 计一条（`deleted`）、`changed` 空。
pub fn plan_relation_delete(
    root: &Path,
    from: &str,
    to: &str,
    expect: Option<&str>,
) -> Result<WriteOutcome, EditError> {
    let graph = load_model(root)?;
    let model = graph.model();
    endpoints(model, from, to)?;
    let hits = relation_hits(model, from, to);
    check_relation_unique(&hits)?;
    let Some(relation) = hits.first() else {
        let existing: Vec<String> = graph
            .depends_on(from)
            .into_iter()
            .map(relation_pair)
            .collect();
        let listed = if existing.is_empty() {
            "（该元素没有出边关系）".to_owned()
        } else {
            format!("（现有关系：{}）", existing.join(" / "))
        };
        return Err(error(
            EditErrorKind::RelationNotFound,
            format!("{from} → {to}{listed}"),
        ));
    };
    let file = root.join(&relation.file);
    let source = read_source(&file, false)?;
    check_expect(&file, &source, expect)?;
    let content = apply_relation_delete(&source, from, to)?;
    let write = write_of(file, &content);
    let written = recount(&write.content)?;
    Ok(WriteOutcome {
        write,
        warnings: Vec::new(),
        changed: Vec::new(),
        created: false,
        relations: RelationTally {
            created: 0,
            updated: 0,
            deleted: 1,
            unchanged: 0,
        },
        counts: counts_of(&written),
        model: model_counts(model, 0, -1),
    })
}

// ---------------------------------------------------------------------------
// 私有 helper
// ---------------------------------------------------------------------------

/// 源文本 → AST（唯一解析点）。
fn parse_source(source: &str) -> Result<DocumentMut, EditError> {
    source
        .parse::<DocumentMut>()
        .map_err(|parse_error| error(EditErrorKind::SourceParseFailed, parse_error.to_string()))
}

/// 待落盘产物装配（`hash` 在文本定稿后算，MUST NOT 让执行体写完再读盘）。
fn write_of(file: PathBuf, content: &str) -> ModelWrite {
    ModelWrite {
        hash: file_hash_of(&file, content),
        file,
        content: content.to_owned(),
    }
}

/// 自证链收口：渲染 → 重解析取计数（诊断非空即早失败）→ 表头回填 → 重解析自证 →
/// `verify` 回读。被自证 / 被回读的文本逐字等于返回值。
fn finish(
    document: DocumentMut,
    verify: impl FnOnce(&Model) -> Result<(), EditError>,
) -> Result<String, EditError> {
    let rendered = document.to_string();
    let counted = TomlParser::parse(&rendered);
    if !counted.diagnostics.is_empty() {
        return Err(self_check_failure(&counted.diagnostics));
    }
    let content = backfill_header_counts(
        &rendered,
        counted.model.elements.len(),
        counted.model.relations.len(),
    );
    let verified = TomlParser::parse(&content);
    if !verified.diagnostics.is_empty() {
        return Err(self_check_failure(&verified.diagnostics));
    }
    verify(&verified.model)?;
    Ok(content)
}

/// 自证失败（重解析出诊断）：零文本产出。
fn self_check_failure(diagnostics: &[Diagnostic]) -> EditError {
    error(
        EditErrorKind::RenderSelfCheck,
        format!(
            "重解析出 {} 条诊断，未产出文本：{}",
            diagnostics.len(),
            diagnostic_kinds(diagnostics)
        ),
    )
}

/// 诊断种类清单（稳定声明序，` / ` 分隔）。
fn diagnostic_kinds(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.kind.as_str())
        .collect::<Vec<&str>>()
        .join(" / ")
}

/// 首部注释块（从首行起连续的空行 / `#` 注释行）内的**第一处**
/// `（<数字> 元素 / <数字> 关系）` 回填两个数字；无该括号即原样返回（MUST NOT 新增），
/// 块外零改动。
fn backfill_header_counts(rendered: &str, elements: usize, relations: usize) -> String {
    let header = &rendered[..header_block_end(rendered)];
    let mut cursor = 0;
    while let Some(offset) = header[cursor..].find('（') {
        let open = cursor + offset;
        let body = open + '（'.len_utf8();
        let Some(element_end) = digit_end(header, body) else {
            cursor = body;
            continue;
        };
        let Some(after_mark) = header[element_end..]
            .strip_prefix(" 元素 / ")
            .map(|_| element_end + " 元素 / ".len())
        else {
            cursor = body;
            continue;
        };
        let Some(relation_end) = digit_end(header, after_mark) else {
            cursor = body;
            continue;
        };
        if !header[relation_end..].starts_with(" 关系）") {
            cursor = body;
            continue;
        }
        // 拼接域是**整份渲染文本**：`header` 只是它的前缀，尾段从 `relation_end` 一路
        // 带出首部注释块之后的全部正文（只收窄到 `header` 会截断整个模型表）。
        let mut output = String::with_capacity(rendered.len() + 8);
        output.push_str(&rendered[..body]);
        output.push_str(&elements.to_string());
        output.push_str(" 元素 / ");
        output.push_str(&relations.to_string());
        output.push_str(&rendered[relation_end..]);
        return output;
    }
    rendered.to_owned()
}

/// 首部注释块的结束字节偏移（首行起连续的空行 / `#` 注释行；遇首个其它行即止）。
fn header_block_end(rendered: &str) -> usize {
    let mut end = 0;
    for line in rendered.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !(trimmed.is_empty() || trimmed.starts_with('#')) {
            break;
        }
        end += line.len();
    }
    end
}

/// 从 `start` 起连续 ASCII 数字的结束偏移（零个数字即 `None`）。
fn digit_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = start;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    (index > start).then_some(index)
}

/// `element` 表（缺席即建隐式外层表，渲染成 `[element."<FQN>"]` 单层表头）。
fn element_table_mut(document: &mut DocumentMut) -> Result<&mut dyn TableLike, EditError> {
    document
        .as_table_mut()
        .entry("element")
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        })
        .as_table_like_mut()
        .ok_or_else(|| error(EditErrorKind::SourceParseFailed, "element 键不是表"))
}

/// `relation` 条目数组（键缺席即建空数组——零关系文件不是「缺表」）。
fn relation_entries_mut(document: &mut DocumentMut) -> Result<&mut ArrayOfTables, EditError> {
    document
        .as_table_mut()
        .entry("relation")
        .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| error(EditErrorKind::SourceParseFailed, "relation 键不是条目数组"))
}

/// 元素表逐字段三态改写（既有键沿原 decor 原位改，新键追加到表尾）。
///
/// 调用序即新建元素的规范字段序——MUST NOT 另立第二份字段序常量。
fn write_element_fields(table: &mut dyn TableLike, spec: &ElementSpec) {
    if let Some(kind) = spec.kind {
        write_value(table, "kind", Value::from(kind.as_str()));
    }
    if let Some(gate) = spec.gate {
        write_value(table, "gate", Value::from(gate.as_str()));
    }
    if let Some(api) = spec.api {
        write_value(table, "api", Value::from(api.as_str()));
    }
    write_optional_string(table, "entry", spec.entry.as_ref());
    match spec.path.as_ref() {
        None => {}
        Some(values) if values.is_empty() => {
            table.remove("path");
        }
        Some(values) => write_value(table, "path", string_array(values)),
    }
    write_optional_string(table, "doc", spec.doc.as_ref());
    write_optional_string(table, "summary", spec.summary.as_ref());
    match spec.metadata.as_ref() {
        None => {}
        Some(values) if values.is_empty() => {
            table.remove("metadata");
        }
        Some(values) => write_value(table, "metadata", inline_table(values)),
    }
}

/// 关系条目逐字段三态改写（`apis = Some(&[])` 翻成删键）。
fn write_relation_fields(
    table: &mut dyn TableLike,
    desc: Option<Option<&str>>,
    apis: Option<&[String]>,
) {
    match desc {
        None => {}
        Some(None) => {
            table.remove("desc");
        }
        Some(Some(description)) => write_value(table, "desc", Value::from(description)),
    }
    match apis {
        None => {}
        Some([]) => {
            table.remove("apis");
        }
        Some(names) => write_value(table, "apis", string_array(names)),
    }
}

/// 可清空字符串键的三态改写。
fn write_optional_string(table: &mut dyn TableLike, key: &str, change: Option<&Option<String>>) {
    match change {
        None => {}
        Some(None) => {
            table.remove(key);
        }
        Some(Some(text)) => write_value(table, key, Value::from(text.as_str())),
    }
}

/// 写一个值：既有键沿用原值的 decor（行内注释 / 缩进 / 相对位置零重排），新键用默认
/// decor 追加到表尾。
fn write_value(table: &mut dyn TableLike, key: &str, value: Value) {
    let Some((mut existing_key, existing_item)) = table.get_key_value_mut(key) else {
        table.insert(key, Item::Value(value));
        return;
    };
    let leaf_decor = existing_key.leaf_decor().clone();
    let dotted_decor = existing_key.dotted_decor().clone();
    let value_decor = existing_item.as_value().map(|value| value.decor().clone());
    *existing_item = Item::Value(value);
    if let (Some(decor), Some(updated)) = (value_decor, existing_item.as_value_mut()) {
        *updated.decor_mut() = decor;
    }
    *existing_key.leaf_decor_mut() = leaf_decor;
    *existing_key.dotted_decor_mut() = dotted_decor;
}

/// 字符串清单 → 内联数组值。
fn string_array(items: &[String]) -> Value {
    let mut array = Array::new();
    for item in items {
        array.push(item.as_str());
    }
    Value::Array(array)
}

/// 字符串表 → 内联表值（语料同形；子表形态会新增一层表头并改变后续键的相对序）。
fn inline_table(values: &BTreeMap<String, String>) -> Value {
    let mut table = InlineTable::new();
    for (key, value) in values {
        table.insert(key, Value::from(value.as_str()));
    }
    Value::InlineTable(table)
}

/// 新关系条目（键序 `from` / `to` / `desc` / `apis`，表头前缀与既有条目同形）。
fn new_relation_table(
    from: &str,
    to: &str,
    desc: Option<Option<&str>>,
    apis: Option<&[String]>,
) -> Table {
    let mut table = Table::new();
    table.insert("from", toml_edit::value(from));
    table.insert("to", toml_edit::value(to));
    if let Some(Some(description)) = desc {
        table.insert("desc", toml_edit::value(description));
    }
    if let Some(names) = apis.filter(|names| !names.is_empty()) {
        table.insert("apis", Item::Value(string_array(names)));
    }
    table.decor_mut().set_prefix("\n");
    table
}

/// 关系条目是否命中 `(from, to)`。
fn relation_matches(table: &Table, from: &str, to: &str) -> bool {
    table.get("from").and_then(Item::as_str) == Some(from)
        && table.get("to").and_then(Item::as_str) == Some(to)
}

/// 同一源内的命中条目下标（文件面的多处声明判据取它的长度）。
fn relation_indexes(entries: &ArrayOfTables, from: &str, to: &str) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, table)| relation_matches(table, from, to))
        .map(|(index, _)| index)
        .collect()
}

/// 文件面重复声明（同一源内多处）：零文本产出的判据。
fn relation_duplicated_in_source(from: &str, to: &str, count: usize) -> EditError {
    error(
        EditErrorKind::RelationDuplicated,
        format!(
            "{} 在同一源内多处声明（{count} 处）——先删除重复条目",
            relation_pair_of(from, to)
        ),
    )
}

/// 模型面按 `(from, to)` 取全部声明（身份键 = `from` / `to` 双等，与 `diff` 的关系
/// 身份键同口径）。
fn relation_hits<'a>(model: &'a Model, from: &str, to: &str) -> Vec<&'a Relation> {
    model
        .relations
        .iter()
        .filter(|relation| relation.from == from && relation.to == to)
        .collect()
}

/// 模型面重复声明的唯一性判据：跨文件多处即 fail-closed（不静默裁决改 / 删哪条）。
fn check_relation_unique(hits: &[&Relation]) -> Result<(), EditError> {
    let Some((first, rest)) = hits.split_first() else {
        return Ok(());
    };
    if rest.is_empty() {
        return Ok(());
    }
    let files: Vec<&str> = hits.iter().map(|relation| relation.file.as_str()).collect();
    Err(error(
        EditErrorKind::RelationDuplicated,
        format!(
            "{} 在 {} 多处声明（{} 处）——先删除重复条目",
            relation_pair(first),
            files.join(" / "),
            hits.len()
        ),
    ))
}

/// 关系条目 → 「from → to」（诊断与事实段的展示形态）。
fn relation_pair(relation: &Relation) -> String {
    relation_pair_of(&relation.from, &relation.to)
}

/// 关系条目表 → 「from → to」（条目数组里取同形展示，含 `relation` 键缺席的空面）。
fn table_pair(table: &Table) -> String {
    let from = table.get("from").and_then(Item::as_str).unwrap_or_default();
    let to = table.get("to").and_then(Item::as_str).unwrap_or_default();
    relation_pair_of(from, to)
}

/// 关系对展示形态（无 `Relation` 在手时的同形装配）。
fn relation_pair_of(from: &str, to: &str) -> String {
    format!("{from} → {to}")
}

/// 模型面按 `(from, to)` 取关系（回读用；身份键与 [`relation_hits`] 同一处）。
fn find_relation<'a>(model: &'a Model, from: &str, to: &str) -> Option<&'a Relation> {
    relation_hits(model, from, to).into_iter().next()
}

/// 回读逐字段比对的错误装配。
fn read_back(
    field: &str,
    actual: impl std::fmt::Display,
    expected: impl std::fmt::Display,
) -> EditError {
    error(
        EditErrorKind::ReadBackMismatch,
        format!("{field}为 {actual}，目标为 {expected}"),
    )
}

/// 关系条数回读：区分「原条目更新」与「追加了一条副本」（既有 ⇒ 不变 / 追加 ⇒ +1 /
/// 删除 ⇒ −1）。
fn verify_relation_count(model: &Model, expected: usize) -> Result<(), EditError> {
    if model.relations.len() == expected {
        return Ok(());
    }
    Err(read_back("关系条数", model.relations.len(), expected))
}

/// 可空字符串的展示形态。
fn show_optional(text: &Option<String>) -> String {
    match text {
        Some(text) => format!("\"{text}\""),
        None => "（未写）".to_owned(),
    }
}

/// 字符串清单的展示形态。
fn show_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("\"{item}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

/// 自由元数据表的展示形态。
fn show_metadata(values: &BTreeMap<String, String>) -> String {
    if values.is_empty() {
        return "（未写）".to_owned();
    }
    let pairs: Vec<String> = values
        .iter()
        .map(|(key, value)| format!("{key}=\"{value}\""))
        .collect();
    format!("{{{}}}", pairs.join(", "))
}

/// 元素回读：逐**指定字段**比对（未指定的位不动，也就无从比对）。
fn verify_element(model: &Model, spec: &ElementSpec) -> Result<(), EditError> {
    let Some(element) = model.element(&spec.fqn) else {
        return Err(error(
            EditErrorKind::ReadBackMismatch,
            format!("找不到元素 {}", spec.fqn),
        ));
    };
    if let Some(kind) = spec.kind {
        if element.kind != kind {
            return Err(read_back("kind", element.kind.as_str(), kind.as_str()));
        }
    }
    if let Some(gate) = spec.gate {
        if element.gate != gate {
            return Err(read_back("档位", element.gate.as_str(), gate.as_str()));
        }
    }
    if let Some(api) = spec.api {
        if element.api != api {
            return Err(read_back("api", element.api.as_str(), api.as_str()));
        }
    }
    if let Some(entry) = &spec.entry {
        if &element.entry != entry {
            return Err(read_back(
                "entry",
                show_optional(&element.entry),
                show_optional(entry),
            ));
        }
    }
    if let Some(path) = &spec.path {
        if &element.path != path {
            return Err(read_back("path", show_list(&element.path), show_list(path)));
        }
    }
    if let Some(doc) = &spec.doc {
        if &element.doc != doc {
            return Err(read_back(
                "doc",
                show_optional(&element.doc),
                show_optional(doc),
            ));
        }
    }
    if let Some(summary) = &spec.summary {
        if &element.summary != summary {
            return Err(read_back(
                "summary",
                show_optional(&element.summary),
                show_optional(summary),
            ));
        }
    }
    if let Some(metadata) = &spec.metadata {
        if &element.metadata != metadata {
            return Err(read_back(
                "metadata",
                show_metadata(&element.metadata),
                show_metadata(metadata),
            ));
        }
    }
    Ok(())
}

/// 关系回读：`(from, to)` 必须在案，`desc` / `apis` 按指定值比对。
fn verify_relation_upsert(
    model: &Model,
    from: &str,
    to: &str,
    desc: Option<Option<&str>>,
    apis: Option<&[String]>,
) -> Result<(), EditError> {
    let Some(relation) = find_relation(model, from, to) else {
        return Err(error(
            EditErrorKind::ReadBackMismatch,
            format!("找不到关系 {}", relation_pair_of(from, to)),
        ));
    };
    if let Some(desc) = desc {
        let expected = desc.map(str::to_owned);
        if relation.description.as_deref() != expected.as_deref() {
            return Err(read_back(
                "关系说明",
                show_optional(&relation.description),
                show_optional(&expected),
            ));
        }
    }
    if let Some(apis) = apis {
        if relation.apis.as_slice() != apis {
            return Err(read_back(
                "关系的 apis",
                show_list(&relation.apis),
                show_list(apis),
            ));
        }
    }
    Ok(())
}

/// 关系回读：`(from, to)` 必须缺席。
fn verify_relation_absent(model: &Model, from: &str, to: &str) -> Result<(), EditError> {
    match find_relation(model, from, to) {
        None => Ok(()),
        Some(_) => Err(error(
            EditErrorKind::ReadBackMismatch,
            format!("关系 {} 仍可回读——未删净", relation_pair_of(from, to)),
        )),
    }
}

/// 写后交付文本的重解析（计数与写后元素事实的**唯一**来源，D-2）。
///
/// 自证链已解析一次，本函数再解析一次是为了让 `counts` / `model` / `warnings` 与「被
/// 交付的文本」同源——MUST NOT 另写一份计数实现。
fn recount(content: &str) -> Result<Model, EditError> {
    let parsed = TomlParser::parse(content);
    if !parsed.diagnostics.is_empty() {
        return Err(self_check_failure(&parsed.diagnostics));
    }
    Ok(parsed.model)
}

/// 本文件计数（写后重解析结果）。
fn counts_of(model: &Model) -> WriteCounts {
    WriteCounts {
        elements: model.elements.len(),
        relations: model.relations.len(),
    }
}

/// 全模型计数（前值 ± 本次增量）。
fn model_counts(prior: &Model, element_delta: isize, relation_delta: isize) -> WriteCounts {
    WriteCounts {
        elements: shift(prior.elements.len(), element_delta),
        relations: shift(prior.relations.len(), relation_delta),
    }
}

/// 计数平移（下界零；越界只会出现在「模型被并发改动」这一不可达路径）。
fn shift(base: usize, delta: isize) -> usize {
    (base as isize + delta).max(0) as usize
}

/// 元素增量：新建 +1，改写在案 0。
fn element_delta(creating: bool) -> isize {
    isize::from(creating)
}

/// 关系增量（`created − deleted`）。
fn relation_delta(tally: RelationTally) -> isize {
    tally.created as isize - tally.deleted as isize
}

/// 新建文件的表头前置（D-5）：渲染完成后按写后重解析计数起手，再解析最终文本自证零诊断
/// ——被交付的文本恒等于被自证的文本。
///
/// `<名>` = 文件名去 `.toml` 后缀的**字面**（不剥序号、不猜语义）。
fn new_file_content(file: &Path, body: &str) -> Result<String, EditError> {
    let counted = TomlParser::parse(body);
    if !counted.diagnostics.is_empty() {
        return Err(self_check_failure(&counted.diagnostics));
    }
    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = name.strip_suffix(".toml").unwrap_or(name);
    let content = format!(
        "# {stem}（{} 元素 / {} 关系）\n\n{body}",
        counted.model.elements.len(),
        counted.model.relations.len()
    );
    let verified = TomlParser::parse(&content);
    if !verified.diagnostics.is_empty() {
        return Err(self_check_failure(&verified.diagnostics));
    }
    Ok(content)
}

/// `changed` = 参数面字段名的子集（实际变化者；`created` 时 = 实际给出的字段名）。
///
/// 字段序与参数面一致（`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` /
/// `metadata`），使「同值重放 ⇒ 空数组」成为幂等凭据。
fn changed_fields(existing: Option<&Element>, spec: &ElementSpec) -> Vec<String> {
    let created = existing.is_none();
    let mut changed: Vec<String> = Vec::new();
    if created && spec.kind.is_some() {
        changed.push("kind".to_owned());
    }
    if let Some(gate) = spec.gate {
        if created || existing.map(|element| element.gate) != Some(gate) {
            changed.push("gate".to_owned());
        }
    }
    if let Some(api) = spec.api {
        if created || existing.map(|element| element.api) != Some(api) {
            changed.push("api".to_owned());
        }
    }
    if let Some(entry) = &spec.entry {
        if created || existing.map(|element| &element.entry) != Some(entry) {
            changed.push("entry".to_owned());
        }
    }
    if let Some(path) = &spec.path {
        if created || existing.map(|element| &element.path) != Some(path) {
            changed.push("path".to_owned());
        }
    }
    if let Some(doc) = &spec.doc {
        if created || existing.map(|element| &element.doc) != Some(doc) {
            changed.push("doc".to_owned());
        }
    }
    if let Some(summary) = &spec.summary {
        if created || existing.map(|element| &element.summary) != Some(summary) {
            changed.push("summary".to_owned());
        }
    }
    if let Some(metadata) = &spec.metadata {
        if created || existing.map(|element| &element.metadata) != Some(metadata) {
            changed.push("metadata".to_owned());
        }
    }
    changed
}

/// 内联出边四态计数（按每对 `to` 的**写前**状态；四者之和恒等于入参长度）。
fn relation_tally(model: &Model, from: &str, relations: &[RelationSpec]) -> RelationTally {
    let mut tally = RelationTally::default();
    for spec in relations {
        match spec.action {
            WriteAction::Delete => tally.deleted += 1,
            WriteAction::Upsert => match find_relation(model, from, &spec.to) {
                None => tally.created += 1,
                Some(relation) => {
                    let desc_same = match spec.desc.as_ref() {
                        None => true,
                        Some(None) => relation.description.is_none(),
                        Some(Some(text)) => relation.description.as_deref() == Some(text.as_str()),
                    };
                    let apis_same = match &spec.apis {
                        None => true,
                        Some(names) => &relation.apis == names,
                    };
                    if desc_same && apis_same {
                        tally.unchanged += 1;
                    } else {
                        tally.updated += 1;
                    }
                }
            },
        }
    }
    tally
}

/// 关系 upsert 的 `changed`（`desc` / `apis` 的子集）。
fn relation_changed(
    existing: Option<&Relation>,
    desc: Option<Option<&str>>,
    apis: Option<&[String]>,
) -> Vec<String> {
    let mut changed: Vec<String> = Vec::new();
    if let Some(change) = desc {
        let expected = change.map(str::to_owned);
        if existing.map(|relation| &relation.description) != Some(&expected) {
            changed.push("desc".to_owned());
        }
    }
    if let Some(names) = apis {
        if existing.map(|relation| relation.apis.as_slice()) != Some(names) {
            changed.push("apis".to_owned());
        }
    }
    changed
}

/// 批量约束的纯文本面两条：同批 `to` 唯一、`action = delete` 项禁带字段。
///
/// 错误定位用 **1 基下标**且与 `action` / `to` 同时出现（设计 D-12「一次报全」的第一处）。
fn check_relation_batch(relations: &[RelationSpec]) -> Result<(), EditError> {
    let mut seen: Vec<&str> = Vec::with_capacity(relations.len());
    for (index, spec) in relations.iter().enumerate() {
        if seen.contains(&spec.to.as_str()) {
            return Err(error(
                EditErrorKind::RelationDuplicateInBatch,
                format!(
                    "relations[{}]（action = {}，to = {:?}）与前项重复——同一 to 在一批里只出现一次",
                    index + 1,
                    spec.action.as_str(),
                    spec.to
                ),
            ));
        }
        seen.push(&spec.to);
        if spec.action == WriteAction::Delete && (spec.desc.is_some() || spec.apis.is_some()) {
            return Err(error(
                EditErrorKind::RelationDeleteHasFields,
                format!(
                    "relations[{}]（action = delete，to = {:?}）不得带 desc / apis——删除动作与字段赋值互斥",
                    index + 1,
                    spec.to
                ),
            ));
        }
    }
    Ok(())
}

/// 内联出边的落点与端点前置：跨文件声明即点名并指向 `archi_relation`，端点缺席即
/// [`EditErrorKind::RelationEndpointMissing`]。
fn check_inline_relations(
    root: &Path,
    model: &Model,
    from: &str,
    target: &Path,
    relations: &[RelationSpec],
) -> Result<(), EditError> {
    if relations.is_empty() {
        return Ok(());
    }
    let target_relative = crate::api::display(root, target);
    for spec in relations {
        let hits = relation_hits(model, from, &spec.to);
        check_relation_unique(&hits)?;
        if let Some(relation) = hits.first() {
            if relation.file != target_relative {
                return Err(error(
                    EditErrorKind::RelationOwnedElsewhere,
                    format!(
                        "{} 现声明于 {}——内联边只作用于目标文件 {target_relative}；这条改用 archi_relation（落其声明文件）",
                        relation_pair_of(from, &spec.to),
                        relation.file
                    ),
                ));
            }
        }
        if model.element(&spec.to).is_none() {
            return Err(error(
                EditErrorKind::RelationEndpointMissing,
                format!("{from} → {}：{} 未声明", spec.to, spec.to),
            ));
        }
    }
    Ok(())
}

/// 内联出边的批量写回：`action` 逐条 upsert / delete，返回写后该文件的关系条数。
///
/// 复用独立的 [`write_relation_fields`] / [`new_relation_table`] / [`relation_indexes`]
/// ——MUST NOT 另写第二套关系渲染。
fn apply_inline_relations(
    document: &mut DocumentMut,
    from: &str,
    relations: &[RelationSpec],
) -> Result<usize, EditError> {
    if relations.is_empty() {
        // 无内联边 ⇒ 不碰 `relation` 键（新建空条目数组会改动渲染文本）
        return Ok(document
            .as_table()
            .get("relation")
            .and_then(Item::as_array_of_tables)
            .map_or(0, ArrayOfTables::len));
    }
    let entries = relation_entries_mut(document)?;
    let mut count = entries.len();
    for spec in relations {
        let hits = relation_indexes(entries, from, &spec.to);
        if hits.len() >= 2 {
            return Err(relation_duplicated_in_source(from, &spec.to, hits.len()));
        }
        match spec.action {
            WriteAction::Upsert => {
                let desc = spec.desc.as_ref().map(Option::as_deref);
                let apis = spec.apis.as_deref();
                match hits.first().copied() {
                    Some(index) => {
                        if let Some(table) = entries.get_mut(index) {
                            write_relation_fields(table, desc, apis);
                        }
                    }
                    None => {
                        entries.push(new_relation_table(from, &spec.to, desc, apis));
                        count += 1;
                    }
                }
            }
            WriteAction::Delete => {
                let Some(index) = hits.first().copied() else {
                    return Err(relation_absent_error(
                        entries,
                        &relation_pair_of(from, &spec.to),
                    ));
                };
                entries.remove(index);
                count = count.saturating_sub(1);
            }
        }
    }
    Ok(count)
}

/// 内联出边的回读：upsert 逐对校验 `desc` / `apis`，delete 校验缺席（同一自证链）。
fn verify_inline_relations(
    model: &Model,
    from: &str,
    relations: &[RelationSpec],
) -> Result<(), EditError> {
    for spec in relations {
        match spec.action {
            WriteAction::Upsert => verify_relation_upsert(
                model,
                from,
                &spec.to,
                spec.desc.as_ref().map(Option::as_deref),
                spec.apis.as_deref(),
            )?,
            WriteAction::Delete => verify_relation_absent(model, from, &spec.to)?,
        }
    }
    Ok(())
}

/// 级联闭包（**同一模型文件内**）：子树递归 ∪ 任一端在闭包内的关系；跨文件即
/// [`EditErrorKind::CascadeCrossFile`] 且零文本产出（一次计划只承载一个文件、一次原子写）。
fn cascade_set(graph: &ModelGraph, fqn: &str, declared_file: &str) -> Result<DeleteSet, EditError> {
    let model = graph.model();
    let mut elements: BTreeSet<String> = BTreeSet::new();
    collect_subtree(graph, fqn, &mut elements);
    let mut relations: BTreeSet<(String, String)> = BTreeSet::new();
    for relation in &model.relations {
        if elements.contains(&relation.from) || elements.contains(&relation.to) {
            relations.insert((relation.from.clone(), relation.to.clone()));
        }
    }
    let mut crossing: Vec<String> = Vec::new();
    for member in &elements {
        if let Some(element) = model.element(member) {
            if element.file != declared_file {
                crossing.push(format!("{member}（{}）", element.file));
            }
        }
    }
    for (from, to) in &relations {
        for relation in relation_hits(model, from, to) {
            if relation.file != declared_file {
                crossing.push(format!("{}（{}）", relation_pair(relation), relation.file));
            }
        }
    }
    if !crossing.is_empty() {
        crossing.sort();
        crossing.dedup();
        return Err(error(
            EditErrorKind::CascadeCrossFile,
            format!(
                "{fqn} 的级联闭包跨出 {declared_file}：{}——cascade 只作用于同一模型文件，跨文件请逐文件删除",
                crossing.join(" / ")
            ),
        ));
    }
    Ok(DeleteSet {
        elements,
        relations,
    })
}

/// 子树递归收集（含自身；`children` 是 FQN 最长前缀推导的单点）。
fn collect_subtree(graph: &ModelGraph, fqn: &str, out: &mut BTreeSet<String>) {
    if !out.insert(fqn.to_owned()) {
        return;
    }
    for child in graph.children(fqn) {
        collect_subtree(graph, child, out);
    }
}

/// 关系缺席的错误装配（独立删边与内联删边共用同一文案族——MUST NOT 为内联路径另写
/// 「不存在就静默跳过」的宽松版）。
fn relation_absent_error(entries: &ArrayOfTables, pair: &str) -> EditError {
    let existing: Vec<String> = entries.iter().map(table_pair).collect();
    let listed = if existing.is_empty() {
        "（该文件没有关系）".to_owned()
    } else {
        format!("（该文件现有关系：{}）", existing.join(" / "))
    };
    error(EditErrorKind::RelationNotFound, format!("{pair}{listed}"))
}

/// 结构诊断前置 + 建图（层级与邻接判据全走 [`ModelGraph`]，MUST NOT 自写前缀扫描）。
fn load_model(root: &Path) -> Result<ModelGraph, EditError> {
    let outcome = parse::parse_dir(root);
    if !outcome.diagnostics.is_empty() {
        return Err(error(
            EditErrorKind::ModelHasDiagnostics,
            format!(
                "{} 条（先修好模型）：{}",
                outcome.diagnostics.len(),
                diagnostic_kinds(&outcome.diagnostics)
            ),
        ));
    }
    Ok(ModelGraph::build(outcome.model))
}

/// 新建元素的写面卫生前置：FQN 形态（要渲染成键，含这些字符会产出不可读的键）→
/// `kind` 必填。只作用于**新建**——既有元素的 FQN 形态仍只由 parser 判。
fn check_create(fqn: &str, kind: Option<ElementKind>) -> Result<(), EditError> {
    let invalid = fqn.is_empty()
        || fqn
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '[' | ']' | '#'));
    if invalid {
        return Err(error(
            EditErrorKind::FqnInvalid,
            format!("{fqn:?} 含空白 / 引号 / 方括号 / # / 换行（写面要把它渲染成键）"),
        ));
    }
    if kind.is_none() {
        return Err(error(
            EditErrorKind::ElementKindMissing,
            format!("{fqn} 不在案——新建必须给出 kind"),
        ));
    }
    Ok(())
}

/// 落点判断产物：目标文件绝对路径 + 是否**新建文件**（后者决定表头前置）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetPlan {
    file: PathBuf,
    fresh: bool,
}

/// 落点四情形：既有元素落其声明文件（给了不同 `file` 即拒绝）；新建省略 `file` 落
/// **父元素**文件；单段 FQN 省略 `file` 即拒绝（新分层的落点必是人的判断）；给了
/// `file` 而目标文件缺席必须同时给 `create`（防拼错名字造出幽灵文件）。
///
/// 祖先须显式声明是既有前置：多段 FQN 的父元素不在案即 [`EditErrorKind::ParentMissing`]，
/// 与落点是否显式给出无关（否则会造出 `orphanHierarchy`）。MUST NOT 按 `kind` / 首段 FQN
/// 推落点——文件名里的语义序号不可推导。
fn check_target(
    root: &Path,
    fqn: &str,
    file: Option<&str>,
    create: bool,
    existing: Option<&Element>,
    model: &Model,
) -> Result<TargetPlan, EditError> {
    if let Some(element) = existing {
        if let Some(given) = file {
            if given != element.file {
                return Err(error(
                    EditErrorKind::TargetInvalid,
                    format!(
                        "{fqn} 现声明于 {}，不接受改落点 {given}（迁文件属独立语义）",
                        element.file
                    ),
                ));
            }
        }
        return Ok(TargetPlan {
            file: root.join(&element.file),
            fresh: false,
        });
    }
    let parent_file = match fqn.rsplit_once('.') {
        Some((parent, _)) => {
            let parent_element = model.element(parent).ok_or_else(|| {
                error(
                    EditErrorKind::ParentMissing,
                    format!("{fqn} 的父 {parent} 未声明——祖先须显式声明，先声明父元素"),
                )
            })?;
            Some(parent_element.file.clone())
        }
        None => None,
    };
    let Some(file) = file else {
        let Some(parent_file) = parent_file else {
            return Err(error(
                EditErrorKind::TargetInvalid,
                format!(
                    "{fqn} 是单段 FQN（无父元素）——新建必须给出 file（模型声明目录一级的 *.toml）"
                ),
            ));
        };
        return Ok(TargetPlan {
            file: root.join(&parent_file),
            fresh: false,
        });
    };
    if !is_single_model_file_name(file) {
        return Err(error(
            EditErrorKind::TargetInvalid,
            format!("{file} 不在模型声明目录一级"),
        ));
    }
    let dir = archi_graph_model_dir(root);
    if !dir.is_dir() {
        return Err(error(
            EditErrorKind::TargetInvalid,
            format!("{} 不存在（写面不代建目录）", dir.display()),
        ));
    }
    let path = dir.join(file);
    let fresh = !path.is_file();
    if fresh && !create {
        return Err(error(
            EditErrorKind::TargetInvalid,
            format!("{file} 不存在——新建文件需给 create = true（防拼错名字造出幽灵文件）"),
        ));
    }
    Ok(TargetPlan { file: path, fresh })
}

/// 名称是否「模型声明目录一级的 `*.toml`」（单段、非绝对）。
fn is_single_model_file_name(file: &str) -> bool {
    let path = Path::new(file);
    !file.is_empty()
        && file.ends_with(".toml")
        && !path.is_absolute()
        && !file.contains('/')
        && !file.contains('\\')
        && path.components().count() == 1
}

/// 读源：新建且目标文件尚不存在 ⇒ 空源（新文件的合法起点）；其余读失败即
/// `SourceReadFailed`。
fn read_source(file: &Path, allow_missing: bool) -> Result<String, EditError> {
    match std::fs::read_to_string(file) {
        Ok(source) => Ok(source),
        Err(_) if allow_missing && !file.exists() => Ok(String::new()),
        Err(read_error) => Err(error(
            EditErrorKind::SourceReadFailed,
            format!("{}（{read_error}）", file.display()),
        )),
    }
}

/// 乐观锁：`None` 不校验；给了 `expect` 而目标文件缺席同判不匹配（不存在「期望缺席」
/// 这一臂）；否则拿**本次读到的字节**比对（不另读一次盘）。
fn check_expect(file: &Path, source: &str, expect: Option<&str>) -> Result<(), EditError> {
    let Some(expected) = expect else {
        return Ok(());
    };
    if !file.exists() {
        return Err(error(
            EditErrorKind::ExpectMismatch,
            format!("{} 不存在（期望 {expected}）", file.display()),
        ));
    }
    let actual = file_hash_of(file, source);
    if actual != expected {
        return Err(error(
            EditErrorKind::ExpectMismatch,
            format!("期望 {expected}，实际 {actual}——重读后再试"),
        ));
    }
    Ok(())
}

/// 关系两端点在案即返回两侧元素，否则点名缺失端。
fn endpoints<'a>(
    model: &'a Model,
    from: &str,
    to: &str,
) -> Result<(&'a Element, &'a Element), EditError> {
    match (model.element(from), model.element(to)) {
        (Some(from_element), Some(to_element)) => Ok((from_element, to_element)),
        (left, right) => {
            let missing: Vec<&str> = [(from, left.is_some()), (to, right.is_some())]
                .into_iter()
                .filter(|(_, declared)| !declared)
                .map(|(fqn, _)| fqn)
                .collect();
            Err(error(
                EditErrorKind::RelationEndpointMissing,
                format!("{from} → {to}：{} 未声明", missing.join(" / ")),
            ))
        }
    }
}

#[cfg(test)]
mod mod_test;

#[cfg(test)]
mod messages_test;

#[cfg(test)]
mod warnings_test;
