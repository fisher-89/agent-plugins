//! 写回原语族：纯函数 `apply_*`（源文本进、改写后的完整源文本出）与计划函数
//! `plan_*`（读盘定位 + 纯函数 + 待落盘产物 [`ModelWrite`]）。
//!
//! 两层沿同一个固定形状：`apply_*` 只碰文本（零文件系统），`plan_*` 只读盘
//! （`parse_dir` 结构诊断前置 → 存在性 / 落点 / 端点前置 → 读源 → `expect` → `apply_*`
//! → 填 `hash`）。**零写盘、零子进程、零 Tauri**——模型落盘不在本模块（写盘单点在
//! 壳层的原子写）。
//!
//! 产出顺序固定为「改 AST → 渲染 → 表头计数回填 → 重解析自证零诊断 → 回读逐字段
//! 一致」，且被自证 / 被回读的文本**逐字等于**交出的 `content`（MUST NOT 先验后改）。
//! 渲染经 `toml_edit`：既有键原位不动（更新时沿用原值的 decor），新键追加到元素表尾，
//! 新建元素按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`
//! 规范序整表写出。
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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use foundation::layout::archi_graph_model_dir;
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike, Value};

use crate::drift::file_hash_of;
use crate::dto::{ApiStatus, ElementKind, GateTier};
use crate::graph::ModelGraph;
use crate::model::{Diagnostic, Element, Model, Relation};
use crate::parse::{self, SourceParser, TomlParser};

pub use messages::{EditError, EditErrorKind};

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

/// 元素写回规格：`fqn` + 可写字段的三态 + `kind`（新建判别）+ 落点 + 乐观锁。
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
    /// 新建落点（repo 相对、模型声明目录一级 `*.toml`）；既有元素只允许同值
    pub file: Option<String>,
    /// 目标文件**当前**内容哈希（`None` = 不校验）
    pub expect: Option<String>,
}

// ---------------------------------------------------------------------------
// 纯函数层（零文件系统触碰）
// ---------------------------------------------------------------------------

/// 单文件元素 upsert：元素缺席 ⇒ 按 `kind` 新建；在案 + `kind` 有值 ⇒
/// `ElementAlreadyExists`；在案则逐位三态改写（既有键序与既有键的 decor 不动）。
pub fn apply_element_spec(source: &str, spec: &ElementSpec) -> Result<String, EditError> {
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
    finish(document, |model| verify_element(model, spec))
}

/// 单文件删元素表：缺 `element` 表 ⇒ `ElementTableMissing`、元素缺席 ⇒
/// `ElementNotFound`（事实段列该文件现有元素）。子元素 / 关系前置归
/// [`plan_element_delete`]（跨文件）。
pub fn apply_element_delete(source: &str, fqn: &str) -> Result<String, EditError> {
    let mut document = parse_source(source)?;
    let table = document
        .as_table_mut()
        .get_mut("element")
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| error(EditErrorKind::ElementTableMissing, "无从定位元素"))?;
    if table.remove(fqn).is_none() {
        let existing: Vec<&str> = table.iter().map(|(key, _)| key).collect();
        return Err(error(
            EditErrorKind::ElementNotFound,
            format!("{fqn}（现有元素：{}）", existing.join(" / ")),
        ));
    }
    finish(document, |model| match model.element(fqn) {
        None => Ok(()),
        Some(_) => Err(error(
            EditErrorKind::ReadBackMismatch,
            format!("{fqn} 仍可回读——未删净"),
        )),
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
        let existing: Vec<String> = entries.iter().map(table_pair).collect();
        let listed = if existing.is_empty() {
            "（该文件没有关系）".to_owned()
        } else {
            format!("（该文件现有关系：{}）", existing.join(" / "))
        };
        return Err(error(
            EditErrorKind::RelationNotFound,
            format!("{from} → {to}{listed}"),
        ));
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

/// 元素 upsert 的计划：定位 + 前置 + 读源 + `expect` + [`apply_element_spec`] + `hash`。
pub fn plan_element_upsert(root: &Path, spec: &ElementSpec) -> Result<ModelWrite, EditError> {
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
    let file = check_target(root, &spec.fqn, spec.file.as_deref(), existing)?;
    if creating {
        if let Some((parent, _)) = spec.fqn.rsplit_once('.') {
            if model.element(parent).is_none() {
                return Err(error(
                    EditErrorKind::ParentMissing,
                    format!(
                        "{} 的父 {parent} 未声明——祖先须显式声明，先声明父元素",
                        spec.fqn
                    ),
                ));
            }
        }
    }
    let source = read_source(&file, creating)?;
    check_expect(&file, &source, spec.expect.as_deref())?;
    let content = apply_element_spec(&source, spec)?;
    Ok(write_of(file, &content))
}

/// 元素删除的计划：元素必须存在，且有直接子元素 / 被任一关系引用即拒绝（fail-closed）。
pub fn plan_element_delete(
    root: &Path,
    fqn: &str,
    expect: Option<&str>,
) -> Result<ModelWrite, EditError> {
    let graph = load_model(root)?;
    let model = graph.model();
    let element = model.element(fqn).ok_or_else(|| {
        error(
            EditErrorKind::ElementNotFound,
            format!("{fqn}（模型共 {} 个元素）", model.elements.len()),
        )
    })?;
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
    let file = root.join(&element.file);
    let source = read_source(&file, false)?;
    check_expect(&file, &source, expect)?;
    let content = apply_element_delete(&source, fqn)?;
    Ok(write_of(file, &content))
}

/// 关系 upsert 的计划：两端点须在案；落点 = 既有 `(from, to)` 的**声明文件**（可以
/// 不是 `from` 元素文件）、新增关系落 `from` 元素声明文件；模型面多处声明 fail-closed。
pub fn plan_relation_upsert(
    root: &Path,
    from: &str,
    to: &str,
    desc: Option<Option<&str>>,
    apis: Option<&[String]>,
    expect: Option<&str>,
) -> Result<ModelWrite, EditError> {
    let graph = load_model(root)?;
    let (from_element, _) = endpoints(graph.model(), from, to)?;
    let hits = relation_hits(graph.model(), from, to);
    check_relation_unique(&hits)?;
    let file = match hits.first() {
        Some(relation) => root.join(&relation.file),
        None => root.join(&from_element.file),
    };
    let source = read_source(&file, false)?;
    check_expect(&file, &source, expect)?;
    let content = apply_relation_upsert(&source, from, to, desc, apis)?;
    Ok(write_of(file, &content))
}

/// 关系删除的计划：两端点须在案；既有 `(from, to)` 的**声明文件**即落点（可以不是
/// `from` 元素文件）；模型面缺席 / 多处声明都在读盘前 fail-closed。
pub fn plan_relation_delete(
    root: &Path,
    from: &str,
    to: &str,
    expect: Option<&str>,
) -> Result<ModelWrite, EditError> {
    let graph = load_model(root)?;
    endpoints(graph.model(), from, to)?;
    let hits = relation_hits(graph.model(), from, to);
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
    Ok(write_of(file, &content))
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

/// 落点判据（最小版）：既有元素落其声明文件（给了不同 `file` 即拒绝），新建元素落
/// 调用方显式给出的模型声明目录一级 `*.toml`（目录缺席不代建）。
fn check_target(
    root: &Path,
    fqn: &str,
    file: Option<&str>,
    existing: Option<&Element>,
) -> Result<PathBuf, EditError> {
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
        return Ok(root.join(&element.file));
    }
    let Some(file) = file else {
        return Err(error(
            EditErrorKind::TargetInvalid,
            format!("{fqn} 未给 file——新建必须显式给出模型声明目录一级的 *.toml"),
        ));
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
    Ok(dir.join(file))
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
