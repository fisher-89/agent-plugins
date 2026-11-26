//! `edit::layout` 的单元测试面（写入序规范化：三条不变量 / 最小移动 / 号重分配 /
//! 关系组连续 / `Reorder` 口径与 cap）。
//!
//! 可见性口径：本模块是 `edit` 的子模块、与 `layout` **平级**，故只可达 `layout` 的
//! `pub` / `pub(super)` 项（[`normalize`] / [`Reorder`] / [`REORDER_BLOCKS_CAP`]）。
//! 五个私有 helper（`source_order` / `element_target_order` / `relation_target_order` /
//! `minimal_moved` / `blocks_of`）**只经 `normalize` 的产出观测**——不为测试开路新增导出。
//!
//! 零 mock：真实 `toml_edit::DocumentMut`（解析 + 渲染即被测面）、真实 `TomlParser`
//! （同源断言：重解析序 == 渲染文本序）、真实 `serde_json`（`Reorder` 线面）；仓库面
//! 用例只读在盘模型文件（`CARGO_MANIFEST_DIR` 上溯六层，零写盘），替身会让「在盘事实」
//! 不可见。

use std::fs;
use std::path::PathBuf;

use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

use foundation::layout::archi_graph_model_dir;

use crate::model::Model;
use crate::parse::{SourceParser, TomlParser};

use super::layout::{normalize, Reorder, REORDER_BLOCKS_CAP};

// ---------------------------------------------------------------------------
// 装置：解析 / 规范化 / 渲染 / 重解析
// ---------------------------------------------------------------------------

/// 源文本 → AST（夹具缺陷即测试自身失败，不掩盖）。
fn document_of(source: &str) -> DocumentMut {
    source.parse().expect("夹具应可解析")
}

/// 规范化一次，返回「渲染文本 + 搬迁块回报」——本文件全部断言的唯一观测面。
fn reordered(source: &str) -> (String, Reorder) {
    let mut document = document_of(source);
    let reorder = normalize(&mut document);
    (document.to_string(), reorder)
}

/// 快路径：零搬迁且逐字节相等。
fn assert_fast_path(source: &str) {
    let (rendered, reorder) = reordered(source);
    assert_eq!(reorder, Reorder::default(), "健康文件应零搬迁：{source:?}");
    assert_eq!(rendered, source, "健康文件应逐字节相等：{source:?}");
}

/// 重解析（零诊断）——序断言与解析层同源。
fn reparsed(rendered: &str) -> Model {
    let outcome = TomlParser::parse(rendered);
    assert!(
        outcome.diagnostics.is_empty(),
        "规范化产出应零诊断：{:?}\n{rendered}",
        outcome.diagnostics
    );
    outcome.model
}

/// 渲染文本中的表形态元素 FQN 序（`[element."<FQN>"]` 表头出现序）。
fn element_headers(rendered: &str) -> Vec<String> {
    rendered
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("[element.\"")?;
            Some(rest.strip_suffix("\"]")?.to_owned())
        })
        .collect()
}

/// 去引号（关系条目的 `from` / `to` 取值）。
fn unquote(text: &str) -> String {
    text.trim().trim_matches('"').to_owned()
}

/// 渲染文本中的关系序（`[[relation]]` 条目出现序 → `(from, to)`）。
fn relation_pairs(rendered: &str) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in rendered.lines() {
        if line.trim() == "[[relation]]" {
            if let Some(pair) = current.take() {
                pairs.push(pair);
            }
            current = Some((String::new(), String::new()));
            continue;
        }
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("from = ") {
            if let Some((from, _)) = current.as_mut() {
                *from = unquote(rest);
            }
        } else if let Some(rest) = trimmed.strip_prefix("to = ") {
            if let Some((_, to)) = current.as_mut() {
                *to = unquote(rest);
            }
        }
    }
    if let Some(pair) = current.take() {
        pairs.push(pair);
    }
    pairs
}

/// 三条不变量的机械断言（E1 / E2 / R1 + 区域序）：只在测试侧做，产品源码零命中。
fn assert_invariants(rendered: &str) {
    let elements = element_headers(rendered);
    let relations = relation_pairs(rendered);

    for (index, fqn) in elements.iter().enumerate() {
        if let Some((parent, _)) = fqn.rsplit_once('.') {
            if let Some(parent_index) = elements.iter().position(|other| other == parent) {
                assert!(
                    parent_index < index,
                    "E1 父先于子：{parent} 应在 {fqn} 之前\n{rendered}"
                );
            }
        }
        let descendant_prefix = format!("{fqn}.");
        let subtree: Vec<usize> = elements
            .iter()
            .enumerate()
            .filter(|(_, other)| other.starts_with(&descendant_prefix))
            .map(|(subtree_index, _)| subtree_index)
            .collect();
        if let (Some(first), Some(last)) = (subtree.first(), subtree.last()) {
            assert_eq!(first, &(index + 1), "E2 子树紧邻父后：{fqn}\n{rendered}");
            assert_eq!(
                last - first + 1,
                subtree.len(),
                "E2 子树连续：{fqn}\n{rendered}"
            );
        }
    }

    if let (Some(first_relation), Some(last_element)) =
        (rendered.find("[[relation]]"), rendered.rfind("[element.\""))
    {
        assert!(
            last_element < first_relation,
            "元素区恒在关系区之前\n{rendered}"
        );
    }

    let froms: Vec<String> = relations.iter().map(|(from, _)| from.clone()).collect();
    let mut previous: Option<&str> = None;
    let mut closed: Vec<&str> = Vec::new();
    for from in &froms {
        if previous == Some(from.as_str()) {
            continue;
        }
        assert!(
            !closed.contains(&from.as_str()),
            "R1 同一 from 组连续（{from} 组分裂）\n{rendered}"
        );
        if let Some(previous) = previous {
            closed.push(previous);
        }
        previous = Some(from);
    }
}

/// 在文档的元素表尾追加一张表（复刻 `apply_element_spec` 新建元素的落位：键序末位、
/// `position` 缺省——`normalize` 按「继承前表位号」读它）。
fn append_element(document: &mut DocumentMut, fqn: &str, kind: &str) {
    let mut entry = Table::new();
    entry.insert("kind", toml_edit::value(kind));
    document
        .as_table_mut()
        .get_mut("element")
        .and_then(Item::as_table_mut)
        .expect("夹具应有 element 表")
        .insert(fqn, Item::Table(entry));
}

/// 在文档的关系条目数组尾追加一条（复刻 `apply_relation_upsert` 的追加点）。
fn append_relation(document: &mut DocumentMut, from: &str, to: &str) {
    let entries: &mut ArrayOfTables = document
        .as_table_mut()
        .get_mut("relation")
        .and_then(Item::as_array_of_tables_mut)
        .expect("夹具应有关系条目数组");
    let mut table = Table::new();
    table.insert("from", toml_edit::value(from));
    table.insert("to", toml_edit::value(to));
    table.decor_mut().set_prefix("\n");
    entries.push(table);
}

/// 仓库根（`CARGO_MANIFEST_DIR` 位于 `<仓根>/packages/desktop/src-tauri/crates/core/archi`，
/// 故上溯**六层**，沿 `tests/pilot_test.rs::repository_root` 先例）。
fn repository_root() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..6 {
        root.pop();
    }
    root
}

// ---------------------------------------------------------------------------
// 夹具：健康形态（六类）与脏形态
// ---------------------------------------------------------------------------

/// 单元素零关系（`k = 1`、`r = 0`）。
const HEALTHY_SINGLE: &str = concat!("[element.\"Only\"]\n", "kind = \"module\"\n",);

/// 分层序父先于子 + 一条关系。
const HEALTHY_HIERARCHY: &str = concat!(
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Auth\"\n",
    "to = \"App.Billing\"\n",
);

/// 多 `from` 组（`A` 组两条相邻在前、`B` 组在后，组间按首现序）。
const HEALTHY_MULTI_GROUP: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"C\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"C\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"B\"\n",
    "to = \"A\"\n",
);

/// 组内 `to` 手选序（`A → Z` 在 `A → Y` 之前：非字典序，MUST 原样保留）。
const HEALTHY_INNER_ORDER: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Y\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Z\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"Z\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"Y\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"B\"\n",
    "to = \"Y\"\n",
);

/// 只有内联表条目（`element` 表内联值，不参与号分配）。
const HEALTHY_INLINE: &str = concat!("[element]\n", "App = { kind = \"package\" }\n",);

/// `[element]` 显式表头（自身有位号）+ 两张元素表。
const HEALTHY_ELEMENT_HEADER: &str = concat!(
    "[element]\n",
    "\n",
    "[element.\"P\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Q\"]\n",
    "kind = \"module\"\n",
);

/// 子树错位：`Q` 夹在 `P` 与 `P.A` / `P.B` 之间（`moved = 1`、块名 `Q`）。
const DIRTY_SUBTREE: &str = concat!(
    "[element.\"P\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Q\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"P.A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"P.B\"]\n",
    "kind = \"module\"\n",
);

/// 子先于父（平局：保留源序更早的项 ⇒ 块名给 `P` 而非 `P.C`）。
const DIRTY_CHILD_FIRST: &str = concat!(
    "[element.\"P.C\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"P\"]\n",
    "kind = \"module\"\n",
);

/// 同层反字典序：`P.Zeta` 在 `P.Alpha` 之前（首次出现序 MUST NOT 被排序改写）。
const DIRTY_SAME_LEVEL: &str = concat!(
    "[element.\"P.Zeta\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"P.Alpha\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"P\"]\n",
    "kind = \"module\"\n",
);

/// 未搬迁 `06` 形态的合成副本：`Views` 组末尾追加三个 `Changes.*` 子块
/// （父已有后代排更前 ⇒ `moved = 3`）。
fn six_copy_source() -> String {
    let mut source = String::from(
        "# 06 · frontend（12 元素 / 0 关系）\n\n[element.\"Desktop.Views\"]\nkind = \"module\"\ngate = \"off\"\n\n",
    );
    for name in [
        "Agents",
        "Architecture",
        "Changes",
        "Config",
        "DbInspector",
        "Explores",
        "Info",
        "Welcome",
    ] {
        source.push_str(&format!(
            "[element.\"Desktop.Views.{name}\"]\nkind = \"module\"\ngate = \"off\"\n\n"
        ));
    }
    for name in ["Flow", "Renderers", "Agent"] {
        source.push_str(&format!(
            "[element.\"Desktop.Views.Changes.{name}\"]\nkind = \"module\"\ngate = \"off\"\n\n"
        ));
    }
    source
}

/// 关系组分裂：`A` 组被 `B → Y` 切开（R1 违例）。
const DIRTY_RELATION_SPLIT: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"X\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Y\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Z\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"X\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"B\"\n",
    "to = \"Y\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"Z\"\n",
);

/// 同一 `from` 的两段被切开（并回后归**一块**）。
const DIRTY_RELATION_SEGMENTS: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"X\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Y\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Z\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"X\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"B\"\n",
    "to = \"Y\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"Z\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"B\"\n",
    "to = \"X\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"Y\"\n",
);

/// 区域混插：`[[relation]]` 夹在两张元素表之间。
const DIRTY_MIXED_REGIONS: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"B\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
);

/// 块内字节面：首部注释块 / 表内说明行 / 行尾注释 / 数组值 / 块间空行齐备。
const DIRTY_BLOCK_BYTES: &str = concat!(
    "# 文件头（3 元素 / 1 关系）\n",
    "[element.\"P\"]\n",
    "# 说明行\n",
    "kind = \"module\" # 行尾注释\n",
    "path = [\"./a/\", \"./b/\"]\n",
    "\n",
    "[element.\"Q\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"P.A\"]\n",
    "kind = \"module\"\n",
);

/// 大文件规模（12 元素 + 3 关系，健康序）。
fn large_source() -> String {
    let mut source = String::new();
    source.push_str("[element.\"Root\"]\nkind = \"package\"\n\n");
    for layer in 0..2 {
        for index in 0..3 {
            source.push_str(&format!(
                "[element.\"Root.L{layer}{index}\"]\nkind = \"module\"\n\n"
            ));
            source.push_str(&format!(
                "[element.\"Root.L{layer}{index}.Leaf\"]\nkind = \"module\"\n\n"
            ));
        }
    }
    for index in 0..3 {
        source.push_str(&format!(
            "[[relation]]\nfrom = \"Root.L0{index}.Leaf\"\nto = \"Root.L1{index}.Leaf\"\n\n"
        ));
    }
    source
}

// ---------------------------------------------------------------------------
// 正向：快路径 no-op（六类形态）
// ---------------------------------------------------------------------------

/// 正向（AC-1 / D-5）：六类健康形态逐一——返回 `Reorder::default()` 且渲染文本与源
/// 逐字节相等（快路径零 AST 改动）。
#[test]
fn 六类健康形态走快路径零字节差异() {
    for source in [
        HEALTHY_SINGLE,
        HEALTHY_HIERARCHY,
        HEALTHY_MULTI_GROUP,
        HEALTHY_INNER_ORDER,
        HEALTHY_INLINE,
        HEALTHY_ELEMENT_HEADER,
    ] {
        assert_fast_path(source);
    }
    assert_fast_path(&large_source());

    let (rendered, reorder) = reordered(HEALTHY_HIERARCHY);
    assert_eq!(reorder.moved, 0);
    assert!(reorder.blocks.is_empty(), "零搬迁 ⇒ 空块清单");
    assert_eq!(
        element_headers(&rendered),
        vec!["App", "App.Auth", "App.Billing"]
    );
    assert_eq!(
        relation_pairs(&rendered),
        vec![("App.Auth".to_owned(), "App.Billing".to_owned())]
    );
}

// ---------------------------------------------------------------------------
// 正向：目标序（元素面）
// ---------------------------------------------------------------------------

/// 正向（AC-2）：同层源序取**首次出现序**——`P.Zeta` 在 `P.Alpha` 之前时保持该序
///（MUST NOT 变字典序），且每个元素的全部后代聚成紧邻区间（E1 + E2）。
#[test]
fn 同层序取首次出现而非字典序() {
    let (rendered, reorder) = reordered(DIRTY_SAME_LEVEL);
    assert_eq!(reorder.moved, 1, "父上移一块");
    assert_eq!(reorder.blocks, vec!["P".to_owned()], "块名取被搬祖先");
    assert_eq!(
        element_headers(&rendered),
        vec!["P", "P.Zeta", "P.Alpha"],
        "同层序沿用源内首次出现序"
    );
    assert_invariants(&rendered);

    // 重解析面不参与序断言：`TomlParser` 的元素表是 `BTreeMap`（按 FQN 排序），
    // 元素的交付序只由渲染文本承载——故只核对元素集，序由渲染面断言。
    let model = reparsed(&rendered);
    let mut fqns: Vec<&str> = model
        .elements
        .iter()
        .map(|element| element.fqn.as_str())
        .collect();
    fqns.sort_unstable();
    assert_eq!(fqns, vec!["P", "P.Alpha", "P.Zeta"], "元素集不因重排增减");
}

/// 正向（AC-2 / AC-3）：子树错位的最小移动——`[P, Q, P.A, P.B]` 收成
/// `[P, P.A, P.B, Q]`，只报一块 `Q`（不得报「两个子元素各一块」）。
#[test]
fn 子树错位最小移动只报一块且全文逐字() {
    let (rendered, reorder) = reordered(DIRTY_SUBTREE);
    assert_eq!(reorder.moved, 1, "只搬一块");
    assert_eq!(reorder.blocks, vec!["Q".to_owned()]);
    assert_eq!(
        rendered,
        concat!(
            "[element.\"P\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"P.A\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"P.B\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"Q\"]\n",
            "kind = \"module\"\n",
        ),
        "渲染全文逐字"
    );
    assert_invariants(&rendered);
}

/// 正向（AC-1）：未搬迁 `06` 形态 —— `Views` 组后追加三个 `Changes.*` 子块 ⇒
/// `moved = 3`、块清单恰三个 FQN（按交付文本序），文本收成子树连续。
#[test]
fn 未搬迁六号形态报三块且按交付文本序() {
    let source = six_copy_source();
    let (rendered, reorder) = reordered(&source);
    assert_eq!(reorder.moved, 3, "三个子块各计一块");
    assert_eq!(
        reorder.blocks,
        vec![
            "Desktop.Views.Changes.Flow".to_owned(),
            "Desktop.Views.Changes.Renderers".to_owned(),
            "Desktop.Views.Changes.Agent".to_owned(),
        ],
        "块清单按交付文本序"
    );
    let headers = element_headers(&rendered);
    let changes = headers
        .iter()
        .position(|fqn| fqn == "Desktop.Views.Changes")
        .expect("Changes 在案");
    assert_eq!(
        &headers[changes + 1..changes + 4],
        &[
            "Desktop.Views.Changes.Flow".to_owned(),
            "Desktop.Views.Changes.Renderers".to_owned(),
            "Desktop.Views.Changes.Agent".to_owned(),
        ],
        "三个子块紧随 Changes（子树连续）：{headers:?}"
    );
    assert!(
        rendered.starts_with("# 06 · frontend"),
        "首部注释块随首表原位"
    );
    assert_invariants(&rendered);
}

/// 正向（AC-1 / D-5）：新建元素（表尾 append、`position` 缺省）落在父子树中段 ⇒
/// 渲染文本收成子树连续、`moved = 1` 且块名为新 FQN；落在文件尾则零搬迁。
#[test]
fn 表尾追加的新元素按位置收进或零搬迁() {
    let parents = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "\n",
        "[element.\"App.Auth\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"App.Auth.Session\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"App.Billing\"]\n",
        "kind = \"module\"\n",
    );
    let mut document = document_of(parents);
    append_element(&mut document, "App.Auth.Token", "module");
    let reorder = normalize(&mut document);
    let rendered = document.to_string();
    assert_eq!(reorder.moved, 1, "新元素落在兄弟之后 ⇒ 一块");
    assert_eq!(reorder.blocks, vec!["App.Auth.Token".to_owned()]);
    assert_eq!(
        element_headers(&rendered),
        vec![
            "App",
            "App.Auth",
            "App.Auth.Session",
            "App.Auth.Token",
            "App.Billing"
        ],
        "新元素收进父的子树区间"
    );
    assert_invariants(&rendered);

    let tail = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "\n",
        "[element.\"App.Billing\"]\n",
        "kind = \"module\"\n",
    );
    let mut document = document_of(tail);
    append_element(&mut document, "App.Billing.Invoice", "module");
    assert_eq!(
        normalize(&mut document),
        Reorder::default(),
        "子树区间已是文件尾 ⇒ 零搬迁"
    );
}

// ---------------------------------------------------------------------------
// 正向：目标序（关系面）与区域序
// ---------------------------------------------------------------------------

/// 正向（AC-5 / D-2）：关系组分裂并回 —— `[A→X, B→Y, A→Z]` 收成
/// `[A→X, A→Z, B→Y]`，`moved = 1`、块名 `A`（并回后出现的那一段归组）。
#[test]
fn 关系组分裂并回报一块且全文逐字() {
    let (rendered, reorder) = reordered(DIRTY_RELATION_SPLIT);
    assert_eq!(reorder.moved, 1);
    assert_eq!(reorder.blocks, vec!["A".to_owned()], "块名 = 该段的 from");
    assert_eq!(
        relation_pairs(&rendered),
        vec![
            ("A".to_owned(), "X".to_owned()),
            ("A".to_owned(), "Z".to_owned()),
            ("B".to_owned(), "Y".to_owned()),
        ],
        "同 from 组连续、组间按首现序"
    );
    assert_invariants(&rendered);
}

/// 正向（AC-2 / AC-5）：组间先后 = 该 `from` 的首现位置；组内 `to` 手选序原样保留；
/// AoT 尾 append 的同 `from` 新边归位到该组**末成员**（不是条目数组尾）。
#[test]
fn 组间首现序与组内手选序及新边归组尾() {
    assert_fast_path(HEALTHY_INNER_ORDER);
    let (rendered, _) = reordered(HEALTHY_INNER_ORDER);
    assert_eq!(
        relation_pairs(&rendered),
        vec![
            ("A".to_owned(), "Z".to_owned()),
            ("A".to_owned(), "Y".to_owned()),
            ("B".to_owned(), "Y".to_owned()),
        ],
        "组内手选序（A → Z 先于 A → Y）原样保留"
    );

    let mut document = document_of(HEALTHY_MULTI_GROUP);
    append_relation(&mut document, "A", "A");
    let reorder = normalize(&mut document);
    let rendered = document.to_string();
    assert_eq!(reorder.moved, 1, "新边并回即一块");
    assert_eq!(reorder.blocks, vec!["A".to_owned()]);
    assert_eq!(
        relation_pairs(&rendered),
        vec![
            ("A".to_owned(), "B".to_owned()),
            ("A".to_owned(), "C".to_owned()),
            ("A".to_owned(), "A".to_owned()),
            ("B".to_owned(), "A".to_owned()),
        ],
        "新边落在 A 组组尾（不是条目数组尾）"
    );
    assert_invariants(&rendered);
}

/// 正向（AC-2）：区域序理顺 —— 混插源渲染后元素区全在关系区之前（全文逐字）。
#[test]
fn 区域混插渲染后元素区恒在关系区之前() {
    let (rendered, reorder) = reordered(DIRTY_MIXED_REGIONS);
    assert_eq!(reorder.moved, 1);
    assert_eq!(reorder.blocks, vec!["B".to_owned()]);
    assert_eq!(
        rendered,
        concat!(
            "[element.\"A\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"B\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"A\"\n",
            "to = \"B\"\n",
        ),
        "渲染全文逐字"
    );
    assert_invariants(&rendered);
}

// ---------------------------------------------------------------------------
// 正向：号分配三类交界（不断言 position 取值）
// ---------------------------------------------------------------------------

/// 正向（AC-2 / R-3）：三类交界的号分配——单元素（`k = 1`、`r = 0`）/ 零关系 /
/// 混插；断言渲染全文逐字 + 重解析出的元素序与关系序，**不断言 `position` 取值**
///（位号是实现细节，交付面只看序）。
#[test]
fn 号分配三类交界只断言序不断言位号() {
    // 单元素：`k = 1`、`r = 0`
    assert_fast_path(HEALTHY_SINGLE);
    assert_eq!(element_headers(HEALTHY_SINGLE), vec!["Only"]);
    assert!(relation_pairs(HEALTHY_SINGLE).is_empty());

    // 零关系：元素 `1..=k`，关系区空
    let zero_relation = concat!(
        "[element.\"A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"A.B\"]\n",
        "kind = \"module\"\n",
    );
    assert_fast_path(zero_relation);
    let rendered = reordered(zero_relation).0;
    assert!(relation_pairs(&rendered).is_empty(), "零关系不建空条目数组");
    assert!(!rendered.contains("[[relation]]"));

    // 混插：元素 `1..=k`、关系 `k+1..=k+r`（区域序理顺；重解析的关系序 == 文本序）
    let (rendered, reorder) = reordered(DIRTY_MIXED_REGIONS);
    assert_eq!(reorder.moved, 1);
    let model = reparsed(&rendered);
    assert_eq!(model.elements.len(), 2, "元素计数不因重排变化");
    let relation_order: Vec<(String, String)> = model
        .relations
        .iter()
        .map(|relation| (relation.from.clone(), relation.to.clone()))
        .collect();
    assert_eq!(relation_order, vec![("A".to_owned(), "B".to_owned())]);
    assert_eq!(
        rendered,
        concat!(
            "[element.\"A\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"B\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"A\"\n",
            "to = \"B\"\n",
        )
    );
}

/// 正向（AC-5 / D-7）：重建后 `ArrayOfTables` 成员序 == 渲染文本中的关系序 ==
/// `TomlParser` 重解析出的 `model.relations` 序（三面同源）。
#[test]
fn 条目成员序与文本序与重解析序三面同源() {
    for source in [DIRTY_RELATION_SPLIT, DIRTY_RELATION_SEGMENTS] {
        let mut document = document_of(source);
        normalize(&mut document);
        let rendered = document.to_string();

        let members: Vec<(String, String)> = document
            .as_table()
            .get("relation")
            .and_then(Item::as_array_of_tables)
            .expect("夹具应有条目数组")
            .iter()
            .map(|table| {
                (
                    table
                        .get("from")
                        .and_then(Item::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    table
                        .get("to")
                        .and_then(Item::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                )
            })
            .collect();
        let textual = relation_pairs(&rendered);
        let parsed: Vec<(String, String)> = reparsed(&rendered)
            .relations
            .iter()
            .map(|relation| (relation.from.clone(), relation.to.clone()))
            .collect();

        assert_eq!(members, textual, "成员序 == 文本序");
        assert_eq!(textual, parsed, "文本序 == 重解析序");
        assert_invariants(&rendered);
    }

    // 元素面要素：重排只动位置——重解析的元素集与计数不变（`TomlParser` 的元素表按
    // FQN 排序，元素的交付序只由渲染文本承载，故此处不拿重解析序对拍表头序）。
    let mut document = document_of(DIRTY_SUBTREE);
    normalize(&mut document);
    let rendered = document.to_string();
    let headers = element_headers(&rendered);
    let mut parsed: Vec<String> = reparsed(&rendered)
        .elements
        .iter()
        .map(|element| element.fqn.clone())
        .collect();
    parsed.sort_unstable();
    let mut expected = headers.clone();
    expected.sort_unstable();
    assert_eq!(parsed, expected, "元素集不因重排增减");
}

// ---------------------------------------------------------------------------
// 正向：幂等与最小移动口径
// ---------------------------------------------------------------------------

/// 正向（AC-3）：同一脏输入连排两次——第二次返回 `Reorder::default()`，两次渲染
/// 文本逐字节相等。
#[test]
fn 同一脏输入连排两次幂等() {
    for source in [
        DIRTY_SUBTREE,
        DIRTY_CHILD_FIRST,
        DIRTY_RELATION_SPLIT,
        DIRTY_MIXED_REGIONS,
    ] {
        let (first, reorder) = reordered(source);
        assert!(reorder.moved >= 1, "首排应搬迁：{source:?}");
        let (second, again) = reordered(&first);
        assert_eq!(again, Reorder::default(), "第二次应零搬迁：{first:?}");
        assert_eq!(second, first, "两次渲染文本逐字节相等");
    }

    let six = six_copy_source();
    let (first, reorder) = reordered(&six);
    assert_eq!(reorder.moved, 3);
    let (second, again) = reordered(&first);
    assert_eq!(again, Reorder::default());
    assert_eq!(second, first);
}

/// 正向（AC-2 / AC-9 零改写面）：只动位置面——被搬迁块的键序 / 值形态 / 行内注释 /
/// 块间空行 / 首部注释块与表头计数逐字保留。
#[test]
fn 块内字节零改写只动位置面() {
    let (rendered, reorder) = reordered(DIRTY_BLOCK_BYTES);
    assert_eq!(reorder.moved, 1);
    assert_eq!(reorder.blocks, vec!["P.A".to_owned()]);
    assert_eq!(
        rendered,
        concat!(
            "# 文件头（3 元素 / 1 关系）\n",
            "[element.\"P\"]\n",
            "# 说明行\n",
            "kind = \"module\" # 行尾注释\n",
            "path = [\"./a/\", \"./b/\"]\n",
            "\n",
            "[element.\"P.A\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"Q\"]\n",
            "kind = \"module\"\n",
        ),
        "块内字节逐字保留（只动位置）"
    );
    for needle in [
        "# 文件头（3 元素 / 1 关系）",
        "# 说明行",
        "# 行尾注释",
        "path = [\"./a/\", \"./b/\"]",
    ] {
        assert!(rendered.contains(needle), "块内字节零改写：{needle}");
    }
}

/// 正向（D-2 / Q-1）：平局推进目标序——`[P.C, P]` 收成 `[P, P.C]` 且块名给 `P`
///（保留源序更早项，不给 `P.C`）。
#[test]
fn 平局保留源序更早项块名给父() {
    let (rendered, reorder) = reordered(DIRTY_CHILD_FIRST);
    assert_eq!(reorder.moved, 1);
    assert_eq!(reorder.blocks, vec!["P".to_owned()], "块名给 P 而非 P.C");
    assert_eq!(
        element_headers(&rendered),
        vec!["P", "P.C"],
        "父先于子（块内字节随块搬）"
    );
    assert_invariants(&rendered);
}

/// 正向（D-2 块口径）：一棵被移动子树含两个后代 ⇒ 归**一块**（取最上层被移动祖先）；
/// 关系面连续同 `from` 的两段并回 ⇒ 同样只报一块。
#[test]
fn 多后代与多段各归一块() {
    // 元素面：父在子之后，父与两个后代一起搬 ⇒ 一块
    let parent_after_children = concat!(
        "[element.\"P.A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"P.B\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"P\"]\n",
        "kind = \"module\"\n",
    );
    let (rendered, reorder) = reordered(parent_after_children);
    assert_eq!(reorder.moved, 1, "两个后代不各计一块");
    assert_eq!(reorder.blocks, vec!["P".to_owned()]);
    assert_eq!(element_headers(&rendered), vec!["P", "P.A", "P.B"]);

    // 关系面：同 from 的两段（A 组被切两刀）并回 ⇒ 一块
    let (rendered, reorder) = reordered(DIRTY_RELATION_SEGMENTS);
    assert_eq!(reorder.moved, 1, "多段并回报一块");
    assert_eq!(reorder.blocks, vec!["A".to_owned()]);
    assert_invariants(&rendered);
}

// ---------------------------------------------------------------------------
// 正向：仓库面只读
// ---------------------------------------------------------------------------

/// 正向（AC-1 的在盘事实）：仓库六个模型文件逐一 `normalize` ⇒ `Reorder::default()`
/// 且逐字节相等；只读、零写盘（读前读后字节全等）。
#[test]
fn 仓库六模型文件逐一no_op且零写盘() {
    let dir = archi_graph_model_dir(&repository_root());
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("模型声明目录应在盘")
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| name.ends_with(".toml"))
        .collect();
    names.sort();
    assert_eq!(names.len(), 6, "仓库模型文件恰六个：{names:?}");

    for name in names {
        let path = dir.join(&name);
        let before = fs::read(&path).expect("读模型文件失败");
        let source = String::from_utf8(before.clone()).expect("模型文件应是 UTF-8");
        let mut document = document_of(&source);
        let reorder = normalize(&mut document);
        assert_eq!(reorder, Reorder::default(), "{name} 应零搬迁");
        assert!(reorder.blocks.is_empty(), "{name} 块清单应空");
        assert_eq!(document.to_string(), source, "{name} 渲染文本应逐字节相等");
        assert_eq!(
            fs::read(&path).expect("读模型文件失败"),
            before,
            "{name} 只读零写盘"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：`Reorder` 口径（cap 与零值线面）
// ---------------------------------------------------------------------------

/// 边界（AC-8 / D-3）：cap 只作用于 `blocks`——长度 0 / 1 / 8 / 9 / 100 逐一：保留
/// **前** 8 项且按入参序，`moved` 保留真值（含 `moved = 0` 与 `moved = 9 > blocks.len()`
/// 的截断可见性）。
#[test]
fn of施加cap且moved保留真值() {
    for len in [0usize, 1, 8, 9, 100] {
        let names: Vec<String> = (0..len).map(|index| format!("块{index}")).collect();
        let reorder = Reorder::of(len, names.clone());
        assert!(
            reorder.blocks.len() <= REORDER_BLOCKS_CAP,
            "blocks 不得超过上限：{}",
            reorder.blocks.len()
        );
        assert_eq!(reorder.moved, len, "moved 恒为真值（不截断）");
        let kept = len.min(REORDER_BLOCKS_CAP);
        assert_eq!(
            reorder.blocks,
            names[..kept].to_vec(),
            "保留前 {kept} 项且按入参序"
        );
    }

    let truncated = Reorder::of(9, (0..9).map(|index| format!("B{index}")).collect());
    assert_eq!(truncated.blocks.len(), REORDER_BLOCKS_CAP);
    assert!(
        truncated.moved > truncated.blocks.len(),
        "截断可见：moved(9) > blocks.len(8)"
    );

    let over_cap = Reorder::of(100, (0..100).map(|index| format!("C{index}")).collect());
    assert_eq!(over_cap.blocks.len(), REORDER_BLOCKS_CAP);
    assert_eq!(over_cap.blocks.last().map(String::as_str), Some("C7"));
    assert_eq!(over_cap.moved, 100);
}

/// 边界（AC-8）：零值线面恰 `{"moved":0,"blocks":[]}`（字段 MUST NOT 省略）；
/// `REORDER_BLOCKS_CAP == 8` 为编译期单点。
#[test]
fn 零值线面字段恒在场且cap为八() {
    assert_eq!(REORDER_BLOCKS_CAP, 8, "字段级上限单点");
    assert_eq!(
        serde_json::to_value(Reorder::default()).expect("序列化应成功"),
        serde_json::json!({ "moved": 0, "blocks": [] }),
        "零值也写两字段（零值不省略）"
    );
    assert_eq!(
        serde_json::to_value(Reorder::of(2, vec!["A".to_owned(), "B".to_owned()]))
            .expect("序列化应成功"),
        serde_json::json!({ "moved": 2, "blocks": ["A", "B"] })
    );
    assert_eq!(
        Reorder::of(0, Vec::new()),
        Reorder::default(),
        "空清单 + 零搬迁即 `Default`"
    );
}

// ---------------------------------------------------------------------------
// 边界：射程（表形态 / 显式表头 / 内联表 / 空文档 / 极值 FQN）
// ---------------------------------------------------------------------------

/// 边界（AC-2 射程）：`element` 表内联值条目（`Item::Value`）原样保留、不参与号分配；
/// 同文件表形态条目仍编 `1..=k`。
#[test]
fn 射程内联表条目不参与重排() {
    assert_fast_path(HEALTHY_INLINE);

    let inline_dirty = concat!(
        "[element]\n",
        "App = { kind = \"package\" }\n",
        "\n",
        "[element.\"App.A.X\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"App.A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"App.B\"]\n",
        "kind = \"module\"\n",
    );
    let (rendered, reorder) = reordered(inline_dirty);
    assert_eq!(reorder.moved, 1);
    assert_eq!(reorder.blocks, vec!["App.A".to_owned()]);
    assert_eq!(
        rendered,
        concat!(
            "[element]\n",
            "App = { kind = \"package\" }\n",
            "\n",
            "[element.\"App.A\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"App.A.X\"]\n",
            "kind = \"module\"\n",
            "\n",
            "[element.\"App.B\"]\n",
            "kind = \"module\"\n",
        ),
        "内联条目原样保留（原位、不改写）"
    );
    assert_eq!(
        element_headers(&rendered),
        vec!["App.A", "App.A.X", "App.B"],
        "表形态条目参与重排（内联条目不在表头序内）"
    );
}

/// 边界（C1 附带形态）：`element` 自身写成显式表头（自身有位号）⇒ 不进入重编号区间、
/// 文本稳定（快路径 no-op）。
#[test]
fn 射程显式element表头保持稳定() {
    assert_fast_path(HEALTHY_ELEMENT_HEADER);
    let (rendered, reorder) = reordered(HEALTHY_ELEMENT_HEADER);
    assert_eq!(reorder, Reorder::default());
    assert!(rendered.starts_with("[element]\n"), "显式表头原位");
    assert_eq!(element_headers(&rendered), vec!["P", "Q"]);
}

/// 边界（list / dict 的空值与缺失形态）：空文档 / 只有 `[[relation]]` 的片段 /
/// `element` 表为空 ⇒ 不 panic、零重排、`moved = 0`，且 `relation` 键缺席时 MUST NOT
/// 建空条目数组。
#[test]
fn 空与退化文档零重排不建空数组() {
    for source in ["", "\n", "[element]\n"] {
        let mut document = document_of(source);
        assert_eq!(normalize(&mut document), Reorder::default());
        assert_eq!(document.to_string(), source, "退化文档原样返回：{source:?}");
    }

    // 只有关系条目：元素区空、关系区原样
    assert_fast_path("[[relation]]\nfrom = \"A\"\nto = \"B\"\n");

    // 元素齐备而 `relation` 键缺席：MUST NOT 建空条目数组
    let without_relation = concat!(
        "[element.\"A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"A.B\"]\n",
        "kind = \"module\"\n",
    );
    let (rendered, reorder) = reordered(without_relation);
    assert_eq!(reorder, Reorder::default());
    assert!(
        !rendered.contains("[[relation]]"),
        "缺席的 relation 键不得被建出"
    );
    assert_eq!(rendered, without_relation);
}

/// 边界（str 极值与特殊字符）：FQN 极值量级 × 9 块 ⇒ `moved = 9`、`blocks` 恰 8 项；
/// emoji 段 FQN 的块名原样进 `blocks`。
#[test]
fn 极值长度与emoji块名原样进清单() {
    let long: String = std::iter::repeat_n('A', 250).collect();
    let mut source = String::new();
    for index in 0..9 {
        source.push_str(&format!(
            "[element.\"L{index}.{long}.C\"]\nkind = \"module\"\n\n"
        ));
    }
    for index in 0..9 {
        source.push_str(&format!(
            "[element.\"L{index}.{long}\"]\nkind = \"module\"\n\n"
        ));
    }
    let (rendered, reorder) = reordered(&source);
    assert_eq!(reorder.moved, 9, "九块真值不截断");
    assert_eq!(reorder.blocks.len(), REORDER_BLOCKS_CAP, "块清单恰 8 项");
    for (index, block) in reorder.blocks.iter().enumerate() {
        assert!(
            block.starts_with(&format!("L{index}.")),
            "保留前 8 项且按交付文本序：{block}"
        );
        assert!(block.len() > 200, "超长 FQN 原样进清单：{} B", block.len());
    }
    assert_invariants(&rendered);

    let emoji = concat!(
        "[element.\"App\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"App.🚀.Inner\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"App.🚀\"]\n",
        "kind = \"module\"\n",
    );
    let (rendered, reorder) = reordered(emoji);
    assert_eq!(reorder.moved, 1);
    assert_eq!(reorder.blocks, vec!["App.🚀".to_owned()], "emoji 段原样");
    assert_eq!(
        element_headers(&rendered),
        vec!["App", "App.🚀", "App.🚀.Inner"]
    );
}

// ---------------------------------------------------------------------------
// 异常：零 `Err` 臂 / 零依赖面
// ---------------------------------------------------------------------------

/// 异常（AC-6 / AC-7）：缺 `element` 键、`element` 为非表形态、`relation` 键形态异常
/// ⇒ **恒返回 `Reorder`**（不 panic、不造诊断、不引入 IO、无失败分支）。
#[test]
fn 形态异常恒返回搬迁块不panic() {
    // 缺 `element` 键：纯关系文件
    let mut document = document_of("[[relation]]\nfrom = \"A\"\nto = \"B\"\n");
    let reorder = normalize(&mut document);
    assert_eq!(reorder, Reorder::default());
    assert_eq!(
        document.to_string(),
        "[[relation]]\nfrom = \"A\"\nto = \"B\"\n"
    );

    // `element` 为非表形态（值）：零重排、原文原样
    let mut document = document_of("element = 3\n");
    assert_eq!(normalize(&mut document), Reorder::default());
    assert_eq!(document.to_string(), "element = 3\n");

    // `relation` 键形态异常（值而非条目数组）+ 元素错位：仍返回搬迁块，不 panic
    let weird = concat!(
        "[element.\"P\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"Q\"]\n",
        "kind = \"module\"\n",
        "\n",
        "[element.\"P.A\"]\n",
        "kind = \"module\"\n",
        "\n",
        "relation = \"x\"\n",
    );
    let mut document = document_of(weird);
    let reorder = normalize(&mut document);
    assert_eq!(reorder.moved, 1, "元素面照常规范化");
    assert_eq!(reorder.blocks, vec!["P.A".to_owned()]);
    let rendered = document.to_string();
    for needle in [
        "[element.\"P\"]",
        "[element.\"P.A\"]",
        "[element.\"Q\"]",
        "relation = \"x\"",
    ] {
        assert!(rendered.contains(needle), "形态异常不得丢内容：{needle}");
    }
    assert!(
        rendered.find("[element.\"P.A\"]") < rendered.find("[element.\"Q\"]"),
        "元素面照常收成子树连续：\n{rendered}"
    );
}

/// 异常（AC-6 / AC-7 源码锚）：`layout.rs` 零诊断 / 判据 / IO 依赖面；`normalize`
/// 签名无 `Result`；`Reorder` 的构造点唯一（`Reorder::of`）。
#[test]
fn 源码锚零依赖面且构造点唯一() {
    let source = include_str!("layout.rs");
    for forbidden in [
        "DiagnosticKind",
        "EditErrorKind",
        "drift::",
        "fs::",
        "spawn",
        "sha2",
        "assert!",
        "debug_assert",
        "Result<",
    ] {
        assert!(
            !source.contains(forbidden),
            "layout.rs 内零 `{forbidden}`（序由测试守，不由运行时守）"
        );
    }
    assert_eq!(source.matches("fn normalize(").count(), 1, "规范化单点");
    assert!(
        source.contains("pub(super) fn normalize(document: &mut DocumentMut) -> Reorder"),
        "签名无 `Result`（零 Err 臂）"
    );
    assert_eq!(
        source.matches("Reorder { moved, blocks }").count(),
        1,
        "`Reorder` 字面构造恰一处（唯一构造点 `Reorder::of`）"
    );
    assert_eq!(
        source.matches("Reorder::of(").count(),
        1,
        "`Reorder::of` 是唯一构造调用点（`normalize` 出口）"
    );
    assert!(
        source.contains("pub fn of(moved: usize, mut blocks: Vec<String>) -> Reorder"),
        "唯一构造点签名"
    );
    assert!(
        source.contains("blocks.truncate(REORDER_BLOCKS_CAP)"),
        "cap 在构造点施加"
    );
    assert_eq!(
        source
            .matches("pub const REORDER_BLOCKS_CAP: usize = 8;")
            .count(),
        1,
        "上限单点字面"
    );
}
