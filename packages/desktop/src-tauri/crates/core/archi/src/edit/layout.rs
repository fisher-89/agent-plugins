//! 写入序规范化：把「已改完 AST、尚未渲染」的文档收进三条不变量，并回报搬迁块。
//!
//! 三条不变量——**E1 父先于子** / **E2 子树连续**（元素面）、**R1 同一 `from` 组连续**
//! （关系面）。**规范序序敏感**（不取字典序）：元素面 = DFS + 同层保持**首次出现序**；
//! 关系面 = 组连续 + 组间保持该 `from` 首现序 + 组内原样（新边因此落组尾）；元素区恒在
//! 关系区之前。**最小移动** = 源序与目标序的最长公共子序列补集——只搬「不在其父子树连续
//! 区间内」的元素段与「组分裂」的关系段，同层相对序、组内相对序与块的内部字节
//! （键序 / decor / 注释 / 空行）逐字保留。故**对已合规的文件本模块是 no-op**（快路径：
//! 零 AST 改动直接返回），这正是它能挂在每次写出口的前提。
//!
//! 落笔口径取 `toml_edit` 的两条编码器事实：渲染序 = `(path 非空, position)` 的**稳定
//! 排序**，`position` 为 `None` 时继承遍历中前一个表的位号；解析器按表头连续分配
//! `position = 1..N`。于是**元素面 = 一次号重分配**（元素区 `1..k`、关系区 `k+1..N`），
//! **关系面必须真移动 `ArrayOfTables` 成员并同批重编号**——只改一处会让渲染序与读回序
//! 打架（`(from, to)` 定位依赖成员序）。
//!
//! 三条纪律：**零 `Err` 臂**（解析已在前完成）、**零诊断 / warning 构造**、**零文件系统**
//! ——序由测试守，不由运行时守（美观缺陷 MUST NOT 升级为写入失败）。射程两条：**单文件**
//! （元素不跨模型文件搬）；**表形态**（只对 `[element."<FQN>"]` 动手，内联表形态的元素
//! 条目原样保留、不参与重排）。

use std::collections::HashSet;

use serde::Serialize;
use toml_edit::{DocumentMut, Item, Table};

/// `reordered.blocks` 的字段级上限（`moved` 恒为真值、永不截断）。
pub const REORDER_BLOCKS_CAP: usize = 8;

/// 一次写回顺带搬迁的块回报（恒在场：零值也写 `{"moved":0,"blocks":[]}`）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Reorder {
    /// 搬迁的**块数真值**（元素块 + 关系块；永不截断）
    pub moved: usize,
    /// 各块的首元素 FQN（元素面）/ `from` FQN（关系面），按交付文本序、至多 [`REORDER_BLOCKS_CAP`] 项
    pub blocks: Vec<String>,
}

impl Reorder {
    /// 唯一构造点：cap 在此施加，`moved` 保留真值。
    pub fn of(moved: usize, mut blocks: Vec<String>) -> Reorder {
        blocks.truncate(REORDER_BLOCKS_CAP);
        Reorder { moved, blocks }
    }
}

/// 文件内的一个「位置槽」：表形态元素（FQN）或关系条目（条目数组下标）。
///
/// 关系槽带下标而非 `(from, to)`——同一对可以在源内多处声明，下标才是身份。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot {
    Element(String),
    Relation(usize),
}

/// 把交付文本收进三条不变量，并回报本次搬迁的块。
///
/// 一个纯内存判断：读手边 AST 的源序与目标序，只搬 LCS 补集；源序已等于目标序即走快路径
/// （零 AST 改动——健康文件的逐字节 no-op 由此成立）。
pub(super) fn normalize(document: &mut DocumentMut) -> Reorder {
    let source = source_order(document);
    let froms = relation_froms(document);
    let target = target_order(&source, &froms);
    let moved = minimal_moved(&source, &target);
    if moved.is_empty() {
        return Reorder::default();
    }
    let blocks = blocks_of(&moved, &target, &froms);
    let count = blocks.len();
    apply_target(document, &target);
    Reorder::of(count, blocks)
}

// ---------------------------------------------------------------------------
// 读源序（复刻编码器的「自身 position，否则继承前表」语义）
// ---------------------------------------------------------------------------

/// 交付前的文本序：按有效位号升序（元素区 / 关系区同池排序）。
///
/// `position` 为 `None` 的表（手工新建、尚未编号）继承遍历中前一个表的位号——这正是
/// 编码器的规则，照抄它才能让「源序 == 目标序 ⇔ 文本 no-op」成立。
fn source_order(document: &DocumentMut) -> Vec<Slot> {
    let mut cursor = 0isize;
    let mut slots: Vec<(isize, u8, Slot)> = Vec::new();
    if let Some(Item::Table(table)) = document.as_table().get("element") {
        if let Some(position) = table.position() {
            cursor = position;
        }
        for (fqn, entry) in table.iter() {
            let Item::Table(entry) = entry else {
                continue;
            };
            if let Some(position) = entry.position() {
                cursor = position;
            }
            slots.push((cursor, 0, Slot::Element(fqn.to_owned())));
        }
    }
    if let Some(entries) = document
        .as_table()
        .get("relation")
        .and_then(Item::as_array_of_tables)
    {
        for index in 0..entries.len() {
            let Some(entry) = entries.get(index) else {
                continue;
            };
            if let Some(position) = entry.position() {
                cursor = position;
            }
            slots.push((cursor, 1, Slot::Relation(index)));
        }
    }
    slots.sort_by_key(|(position, face, _)| (*position, *face));
    slots.into_iter().map(|(_, _, slot)| slot).collect()
}

/// 关系条目的 `from`（按条目数组下标；缺键取空串，与展示 / 判据面同款兜底）。
fn relation_froms(document: &DocumentMut) -> Vec<String> {
    document
        .as_table()
        .get("relation")
        .and_then(Item::as_array_of_tables)
        .map(|entries| {
            (0..entries.len())
                .map(|index| {
                    entries
                        .get(index)
                        .and_then(|entry| entry.get("from"))
                        .and_then(Item::as_str)
                        .unwrap_or_default()
                        .to_owned()
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// 算目标序
// ---------------------------------------------------------------------------

/// 目标序：元素区（DFS）全部在前，关系区（组连续）全部在后——混插形态由此被理顺。
fn target_order(source: &[Slot], froms: &[String]) -> Vec<Slot> {
    let mut elements: Vec<&str> = Vec::new();
    let mut relations: Vec<usize> = Vec::new();
    for slot in source {
        match slot {
            Slot::Element(fqn) => elements.push(fqn.as_str()),
            Slot::Relation(index) => relations.push(*index),
        }
    }
    let mut target: Vec<Slot> = element_order(&elements)
        .into_iter()
        .map(|fqn| Slot::Element(fqn.to_owned()))
        .collect();
    target.extend(
        relation_order(&relations, froms)
            .into_iter()
            .map(Slot::Relation),
    );
    target
}

/// FQN 的父（末段之前）；单段 FQN 无父。
fn parent_of(fqn: &str) -> Option<&str> {
    fqn.rsplit_once('.').map(|(parent, _)| parent)
}

/// 父是否声明在**本文件**元素集内（不在即当顶级——元素不跨模型文件搬）。
fn has_parent_in(fqn: &str, present: &HashSet<&str>) -> bool {
    parent_of(fqn).is_some_and(|parent| present.contains(parent))
}

/// 元素目标序：DFS + 同层保持首次出现序。
///
/// 直接子按 FQN 前缀现算（`<父FQN>.` 且余段不含 `.`）——MUST NOT 借
/// `ModelGraph::children`，它 `sort_unstable()`，用了即把同层序变成字典序。
fn element_order<'a>(fqns: &[&'a str]) -> Vec<&'a str> {
    let present: HashSet<&'a str> = fqns.iter().copied().collect();
    let mut order: Vec<&'a str> = Vec::with_capacity(fqns.len());
    for fqn in fqns.iter().copied() {
        if !has_parent_in(fqn, &present) {
            push_subtree(fqn, fqns, &mut order);
        }
    }
    order
}

/// 先自身，再按首次出现序递归展开直接子。
fn push_subtree<'a>(fqn: &'a str, fqns: &[&'a str], order: &mut Vec<&'a str>) {
    order.push(fqn);
    for child in fqns.iter().copied() {
        if parent_of(child) == Some(fqn) {
            push_subtree(child, fqns, order);
        }
    }
}

/// 关系目标序：同 `from` 聚成一段，段的先后 = 该 `from` 的首现位置，段内保持源序
/// （新边因此落在组尾）。返回源下标序 = 重建条目数组的置换。
fn relation_order(indexes: &[usize], froms: &[String]) -> Vec<usize> {
    let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
    for index in indexes.iter().copied() {
        let from = froms.get(index).map(String::as_str).unwrap_or_default();
        match groups.iter_mut().find(|(key, _)| *key == from) {
            Some((_, members)) => members.push(index),
            None => groups.push((from, vec![index])),
        }
    }
    groups
        .into_iter()
        .flat_map(|(_, members)| members)
        .collect()
}

// ---------------------------------------------------------------------------
// 最小移动（LCS 补集）与块归组
// ---------------------------------------------------------------------------

/// 最小移动集合 = 源序与目标序的 LCS 补集；平局**推进 T**（保留源序更早的项）。
///
/// 空集 ⇔ 源序 == 目标序 ⇔ 规范化对文本是 no-op（快路径的依据）。
fn minimal_moved(source: &[Slot], target: &[Slot]) -> Vec<Slot> {
    let (n, m) = (source.len(), target.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if source[i] == target[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut moved: Vec<Slot> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if source[i] == target[j] {
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] > lcs[i][j + 1] {
            moved.push(source[i].clone());
            i += 1;
        } else {
            j += 1;
        }
    }
    moved.extend(source[i..].iter().cloned());
    moved
}

/// 块口径：元素块按**最上层被移动祖先**归组、关系块按**连续同 `from`** 归段；
/// 元素块在前，各自按目标序。
fn blocks_of(moved: &[Slot], target: &[Slot], froms: &[String]) -> Vec<String> {
    let mut moved_elements: HashSet<&str> = HashSet::new();
    let mut moved_relations: HashSet<usize> = HashSet::new();
    for slot in moved {
        match slot {
            Slot::Element(fqn) => {
                moved_elements.insert(fqn.as_str());
            }
            Slot::Relation(index) => {
                moved_relations.insert(*index);
            }
        }
    }
    let declared: HashSet<&str> = target
        .iter()
        .filter_map(|slot| match slot {
            Slot::Element(fqn) => Some(fqn.as_str()),
            Slot::Relation(_) => None,
        })
        .collect();
    let mut blocks: Vec<String> = Vec::new();
    for slot in target {
        let Slot::Element(fqn) = slot else {
            continue;
        };
        if !moved_elements.contains(fqn.as_str()) {
            continue;
        }
        let root = moved_root(fqn, &moved_elements, &declared);
        if !blocks.iter().any(|existing| existing == &root) {
            blocks.push(root);
        }
    }
    let mut previous: Option<&str> = None;
    for slot in target {
        let Slot::Relation(index) = slot else {
            continue;
        };
        if !moved_relations.contains(index) {
            continue;
        }
        let from = froms.get(*index).map(String::as_str).unwrap_or_default();
        if previous != Some(from) {
            blocks.push(from.to_owned());
            previous = Some(from);
        }
    }
    blocks
}

/// 被移动元素的块根：向上取最上层「也在此次移动集合内且声明在本文件」的祖先。
fn moved_root(fqn: &str, moved: &HashSet<&str>, declared: &HashSet<&str>) -> String {
    let mut root = fqn;
    let mut current = fqn;
    while let Some(parent) = parent_of(current) {
        if !declared.contains(parent) || !moved.contains(parent) {
            break;
        }
        root = parent;
        current = parent;
    }
    root.to_owned()
}

// ---------------------------------------------------------------------------
// 落笔（只动位置面）
// ---------------------------------------------------------------------------

/// 按目标序重编号：元素区 `1..k`，关系区 `k+1..N`；关系面**真移动**成员后再编号
/// （成员整体搬迁，内部字节零改写）。
fn apply_target(document: &mut DocumentMut, target: &[Slot]) {
    let mut elements: Vec<&str> = Vec::new();
    let mut relations: Vec<usize> = Vec::new();
    for slot in target {
        match slot {
            Slot::Element(fqn) => elements.push(fqn.as_str()),
            Slot::Relation(index) => relations.push(*index),
        }
    }
    if let Some(table) = document
        .as_table_mut()
        .get_mut("element")
        .and_then(Item::as_table_mut)
    {
        for (offset, fqn) in elements.iter().enumerate() {
            if let Some(entry) = table.get_mut(fqn).and_then(Item::as_table_mut) {
                entry.set_position(Some(offset as isize + 1));
            }
        }
    }
    let base = elements.len() as isize;
    if let Some(entries) = document
        .as_table_mut()
        .get_mut("relation")
        .and_then(Item::as_array_of_tables_mut)
    {
        let count = entries.len();
        let mut taken: Vec<Option<Table>> = (0..count).map(|_| Some(entries.remove(0))).collect();
        for (offset, index) in relations.iter().enumerate() {
            if let Some(mut table) = taken.get_mut(*index).and_then(Option::take) {
                table.set_position(Some(base + offset as isize + 1));
                entries.push(table);
            }
        }
    }
}
