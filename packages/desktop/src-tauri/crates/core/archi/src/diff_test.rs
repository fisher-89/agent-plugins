//! `diff` 的单元测试：以 FQN / `(from, to)` 为身份键的元素增删、关系增删计数与
//! `gate` 变更清单，以及 `ModelDiff::is_empty` 的零占位判据。
//!
//! 零 mock：`Model` 结构体字面量直构（纯函数零 IO，不经盘、不经 `parse_dir`）。

use std::collections::BTreeMap;

use crate::diff::{model_diff, GateChange, ModelDiff};
use crate::dto::{ElementKind, GateTier};
use crate::model::{Element, Model, Relation};

/// 元素构造器（缺省 `module` / `off`；`file` / `line` 参与「非身份」断言）。
fn element(fqn: &str) -> Element {
    Element {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Off,
        path: Vec::new(),
        docs: Vec::new(),
        summary: None,
        metadata: BTreeMap::new(),
        file: ".dev-team/architecture/models/01-core.toml".to_owned(),
        line: 1,
    }
}

/// 带档位的元素。
fn gated(fqn: &str, gate: GateTier) -> Element {
    Element {
        gate,
        ..element(fqn)
    }
}

/// 关系构造器。
fn relation(from: &str, to: &str) -> Relation {
    Relation {
        from: from.to_owned(),
        to: to.to_owned(),
        description: None,
        file: ".dev-team/architecture/models/01-core.toml".to_owned(),
        line: 2,
        span: None,
    }
}

/// 模型构造器。
fn model_of(elements: Vec<Element>, relations: Vec<Relation>) -> Model {
    Model {
        elements,
        relations,
    }
}

// ---------------------------------------------------------------------------
// 正向：spec 场景 + 等价模型
// ---------------------------------------------------------------------------

/// 正向：spec 场景——新增 `App.Billing` / `App.Billing.Invoice`、删除 `App.Legacy`、
/// 关系 +3 −1、`App.Auth` 档位 `off` → `strict` → 五组逐项断言。
///
/// `gate` 变更以「两侧同名元素」为前提（`model_diff` 先取基线同名元素再比较）：
/// 新增元素即便带非缺省档位也只进 `elements_added`（无旧值可比），故 spec 场景的
/// 档位变更落在既有元素上。
#[test]
fn 结构diff命中spec场景五组逐项() {
    let base = model_of(
        vec![element("App"), element("App.Auth"), element("App.Legacy")],
        vec![relation("App.Auth", "App"), relation("App.Legacy", "App")],
    );
    let next = model_of(
        vec![
            element("App"),
            gated("App.Auth", GateTier::Strict),
            gated("App.Billing", GateTier::Strict),
            element("App.Billing.Invoice"),
        ],
        vec![
            relation("App.Auth", "App"),
            relation("App.Auth", "App.Billing"),
            relation("App.Billing.Invoice", "App.Billing"),
            relation("App.Billing", "App"),
        ],
    );

    let diff = model_diff(&base, &next);
    assert_eq!(
        diff.elements_added,
        vec!["App.Billing".to_owned(), "App.Billing.Invoice".to_owned()],
        "新增元素按字典序"
    );
    assert_eq!(diff.elements_removed, vec!["App.Legacy".to_owned()]);
    assert_eq!(diff.relations_added, 3, "关系 +3");
    assert_eq!(diff.relations_removed, 1, "关系 −1");
    assert_eq!(
        diff.gate_changes,
        vec![GateChange {
            fqn: "App.Auth".to_owned(),
            from: GateTier::Off,
            to: GateTier::Strict,
        }],
        "gate 变更恰一条（既有元素旧缺省 off → strict；新增的 App.Billing 不进本组）"
    );
    assert!(!diff.is_empty(), "有改动即非空");
}

/// 正向：等价模型（元素 / 关系 / 档位相同，仅文件划分与行序不同）→ 五组皆空、
/// `is_empty() == true`（文件与行号不作身份）。
#[test]
fn 等价模型零差异_文件与行号不参与身份() {
    let base = model_of(
        vec![element("App"), gated("App.Auth", GateTier::Warn)],
        vec![relation("App.Auth", "App")],
    );
    let mut moved_auth = gated("App.Auth", GateTier::Warn);
    moved_auth.file = ".dev-team/architecture/models/02-auth.toml".to_owned();
    moved_auth.line = 42;
    let mut moved_relation = relation("App.Auth", "App");
    moved_relation.file = ".dev-team/architecture/models/02-auth.toml".to_owned();
    moved_relation.line = 50;
    let next = model_of(vec![element("App"), moved_auth], vec![moved_relation]);

    let diff = model_diff(&base, &next);
    assert_eq!(diff, ModelDiff::default(), "五组皆空");
    assert!(diff.is_empty(), "is_empty 为真（零占位判据）");
}

// ---------------------------------------------------------------------------
// 异常：身份键不同则互不抵消
// ---------------------------------------------------------------------------

/// 异常：同名不同 FQN（`App.Auth` vs `App.Auth2`）→ 一加一删互不抵消；关系
/// `A→B` 变 `A→C` → 删 1 加 1。
#[test]
fn 身份键不同则加减互不抵消() {
    let base = model_of(
        vec![element("App.Auth"), element("App.A"), element("App.B")],
        vec![relation("App.A", "App.B")],
    );
    let next = model_of(
        vec![element("App.Auth2"), element("App.A"), element("App.C")],
        vec![relation("App.A", "App.C")],
    );
    let diff = model_diff(&base, &next);
    assert_eq!(
        diff.elements_added,
        vec!["App.Auth2".to_owned(), "App.C".to_owned()],
        "App.Auth2（改名侧）与 App.C（新增侧）各一条"
    );
    assert_eq!(
        diff.elements_removed,
        vec!["App.Auth".to_owned(), "App.B".to_owned()],
        "App.Auth（旧名）与 App.B（删除侧）各一条"
    );
    assert_eq!(diff.relations_added, 1, "A→C 为新增");
    assert_eq!(diff.relations_removed, 1, "A→B 为删除");
}

// ---------------------------------------------------------------------------
// 边界：空侧 / gate / 关系身份 / 规模 / str
// ---------------------------------------------------------------------------

/// 边界：两侧空模型 → 五组皆空 + `is_empty`；单侧空（base 空 / next 空）→ 全量
/// 增 / 删。
#[test]
fn 空侧三形态() {
    let empty = Model::empty();
    let diff = model_diff(&empty, &empty);
    assert_eq!(diff, ModelDiff::default());
    assert!(diff.is_empty());

    let three = model_of(
        vec![element("App"), element("App.Auth"), element("App.Billing")],
        vec![relation("App.Auth", "App"), relation("App.Billing", "App")],
    );
    let added = model_diff(&empty, &three);
    assert_eq!(
        added.elements_added,
        vec![
            "App".to_owned(),
            "App.Auth".to_owned(),
            "App.Billing".to_owned()
        ],
        "冷启动：元素全为新增（字典序）"
    );
    assert_eq!(added.relations_added, 2);
    assert_eq!(added.elements_removed.len(), 0);

    let removed = model_diff(&three, &empty);
    assert_eq!(removed.elements_removed.len(), 3, "反向：元素全为删除");
    assert_eq!(removed.relations_removed, 2);
    assert!(removed.elements_added.is_empty());
}

/// 边界：`gate` 变更——同值不计入；`Off → Warn` 与 `Warn → Strict` 各一条；新增
/// 元素带非缺省档位只进 `elements_added`、不进 `gate_changes`。
#[test]
fn gate变更三形态() {
    let base = model_of(
        vec![
            element("App.A"),
            element("App.B"),
            gated("App.C", GateTier::Warn),
        ],
        Vec::new(),
    );
    let next = model_of(
        vec![
            element("App.A"),                 // 同值 Off → Off
            gated("App.B", GateTier::Warn),   // Off → Warn
            gated("App.C", GateTier::Strict), // Warn → Strict
            gated("App.D", GateTier::Strict), // 新增元素
        ],
        Vec::new(),
    );
    let diff = model_diff(&base, &next);
    assert_eq!(
        diff.gate_changes,
        vec![
            GateChange {
                fqn: "App.B".to_owned(),
                from: GateTier::Off,
                to: GateTier::Warn,
            },
            GateChange {
                fqn: "App.C".to_owned(),
                from: GateTier::Warn,
                to: GateTier::Strict,
            },
        ],
        "同值不计入；两处变更按 FQN 字典序；新增元素不入 gate_changes"
    );
    assert_eq!(diff.elements_added, vec!["App.D".to_owned()]);
    assert!(
        diff.gate_changes.iter().all(|change| change.fqn != "App.D"),
        "新增元素无旧值，不进 gate 变更"
    );
}

/// 边界：关系身份去重——同 `(from, to)` 重复声明两次按 1 计；同一 `from` 指向不
/// 同 `to` 为多条独立。
#[test]
fn 关系按from_to身份计数() {
    let base = model_of(
        Vec::new(),
        vec![
            relation("App.A", "App.B"),
            relation("App.A", "App.B"),
            relation("App.A", "App.C"),
        ],
    );
    let next = model_of(Vec::new(), vec![relation("App.A", "App.B")]);
    let diff = model_diff(&base, &next);
    assert_eq!(diff.relations_removed, 2, "重复声明按身份键 1 计 → 删 1");
    assert_eq!(diff.relations_added, 0, "同一条关系在两侧皆在场");
    assert!(!diff.is_empty());
}

/// 边界：规模面——500 元素 / 1000 关系 → 计数正确、清单字典序；空组字段为 `[]`。
#[test]
fn 规模面五百元素千关系() {
    let base = model_of(
        (0..500)
            .map(|index| element(&format!("App.Base{index:03}")))
            .collect(),
        (0..1000)
            .map(|index| relation(&format!("App.Base{:03}", index % 500), "App.Anchor"))
            .collect(),
    );
    let next = model_of(
        (0..500)
            .map(|index| element(&format!("App.Next{index:03}")))
            .collect(),
        (0..1000)
            .map(|index| relation("App.Anchor", &format!("App.Next{:03}", index % 500)))
            .collect(),
    );
    let diff = model_diff(&base, &next);
    assert_eq!(diff.elements_added.len(), 500);
    assert_eq!(diff.elements_removed.len(), 500);
    assert_eq!(diff.relations_added, 1000);
    assert_eq!(diff.relations_removed, 1000);
    let mut sorted = diff.elements_added.clone();
    sorted.sort();
    assert_eq!(diff.elements_added, sorted, "清单字典序");
    assert!(diff.gate_changes.is_empty(), "空组字段为空数组（非 null）");
}

/// 边界：`str` 面——超长 FQN（>1000 chars）/ 含 emoji / 含 `\n` 的 FQN → 清单逐字
/// 保留、不 panic。
#[test]
fn 特殊字符fqn逐字保留() {
    let long = "X".repeat(1200);
    let base = model_of(vec![element(&long), element("App.🚀Auth")], Vec::new());
    let next = model_of(
        vec![element("App.Auth\nB"), element("App.🚀Auth")],
        Vec::new(),
    );
    let diff = model_diff(&base, &next);
    assert_eq!(diff.elements_removed, vec![long], "超长 FQN 逐字保留");
    assert_eq!(
        diff.elements_added,
        vec!["App.Auth\nB".to_owned()],
        "含换行的 FQN 逐字保留"
    );
    assert!(
        diff.gate_changes.is_empty(),
        "emoji 元素两侧同值不入 gate 变更"
    );
}

// ---------------------------------------------------------------------------
// `ModelDiff::is_empty`
// ---------------------------------------------------------------------------

/// 边界：单组非空即 `false`（元素增 / 元素删 / 关系计数 / `gate` 变更各一例）。
#[test]
fn is_empty单组非空即为假() {
    let only_added = ModelDiff {
        elements_added: vec!["App.A".to_owned()],
        ..ModelDiff::default()
    };
    assert!(!only_added.is_empty());

    let only_removed = ModelDiff {
        elements_removed: vec!["App.A".to_owned()],
        ..ModelDiff::default()
    };
    assert!(!only_removed.is_empty());

    let only_relations = ModelDiff {
        relations_added: 1,
        relations_removed: 0,
        ..ModelDiff::default()
    };
    assert!(!only_relations.is_empty());

    let only_gate = ModelDiff {
        gate_changes: vec![GateChange {
            fqn: "App.A".to_owned(),
            from: GateTier::Off,
            to: GateTier::Strict,
        }],
        ..ModelDiff::default()
    };
    assert!(!only_gate.is_empty());

    assert!(ModelDiff::default().is_empty(), "全零即空");
}
