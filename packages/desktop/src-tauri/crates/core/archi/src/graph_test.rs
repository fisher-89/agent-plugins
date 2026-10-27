//! `graph` 的单元测试：唯一建图点（`build` / `model`）、四类邻域查询（出边 /
//! 入边 / 直接子元素 / 顶层）、1-hop 组合，以及四类自实现结构诊断（成环 /
//! 依赖祖先 / 未定义引用 / 孤儿层级）的一次列全与边界。
//!
//! 零 mock：`Model` / `Element` / `Relation` 结构体字面量直构（不经解析），
//! `petgraph` 真实 `DiGraph` + `tarjan_scc`（图算法即被测语义）。

use std::collections::BTreeMap;

use crate::dto::{DiagnosticKind, ElementKind, GateTier};
use crate::model::{Element, Model, Relation};

use crate::graph::ModelGraph;

/// 元素构造器（缺省 `module` / `off` / 空集合）。
fn element(fqn: &str) -> Element {
    Element {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Off,
        path: Vec::new(),
        docs: Vec::new(),
        summary: None,
        metadata: BTreeMap::new(),
        file: "openspec/architecture/models/01-core.toml".to_owned(),
        line: 1,
    }
}

/// 关系构造器（`file` / `line` 供诊断断言用）。
fn relation(from: &str, to: &str) -> Relation {
    Relation {
        from: from.to_owned(),
        to: to.to_owned(),
        description: None,
        file: "openspec/architecture/models/01-core.toml".to_owned(),
        line: 20,
        span: None,
    }
}

/// 带说明的关系（`desc` 透传断言用）。
fn described(from: &str, to: &str, description: &str) -> Relation {
    Relation {
        description: Some(description.to_owned()),
        ..relation(from, to)
    }
}

/// 模型构造器：元素 FQN 清单 + 关系对清单。
fn model_of(fqns: &[&str], relations: &[(&str, &str)]) -> Model {
    Model {
        elements: fqns.iter().map(|fqn| element(fqn)).collect(),
        relations: relations
            .iter()
            .map(|(from, to)| relation(from, to))
            .collect(),
    }
}

/// 三元素两关系图（`App` / `App.Auth` / `App.Billing`）。
fn three_node_model() -> Model {
    model_of(
        &["App", "App.Auth", "App.Billing"],
        &[("App.Auth", "App"), ("App.Billing", "App")],
    )
}

// ---------------------------------------------------------------------------
// 正向：建图 / 模型只读 / 邻域
// ---------------------------------------------------------------------------

/// 正向：3 元素 2 关系的 `Model` → `model()` 逐字段等于入参（声明序保留）；
/// 节点数 = 元素数、边数 = 关系数（经邻接查询可观测）。
#[test]
fn 建图保留模型本体且邻接规模与入参一致() {
    let model = three_node_model();
    let graph = ModelGraph::build(model.clone());
    assert_eq!(
        graph.model(),
        &model,
        "model() 逐字段等于入参（声明序保留）"
    );

    let mut out_degree = 0;
    for fqn in ["App", "App.Auth", "App.Billing"] {
        out_degree += graph.depends_on(fqn).len();
        // 三个 FQN 皆可查（节点数 = 元素数 的可观测面）
        let _ = graph.children(fqn);
    }
    assert_eq!(out_degree, model.relations.len(), "边数 = 关系数");
    assert_eq!(
        graph.model().elements.len(),
        3,
        "节点数 = 元素数（无隐式节点）"
    );
}

/// 边界：空模型 → `model()` 为空、`children` / 邻域全空、`diagnostics()` 空；
/// 只声明一个顶层元素 → 元素表恰一项。
#[test]
fn 空模型与单顶层元素的空面() {
    let empty = ModelGraph::build(Model::empty());
    assert!(empty.model().elements.is_empty());
    assert!(empty.children("App").is_empty());
    assert!(empty.depends_on("App").is_empty());
    assert!(empty.depended_by("App").is_empty());
    assert!(empty.diagnostics().is_empty());

    let single = ModelGraph::build(model_of(&["App"], &[]));
    assert_eq!(single.model().elements.len(), 1, "单顶层元素恰一项");
    assert!(single.children("App").is_empty(), "无子元素");
    assert!(single.diagnostics().is_empty(), "单顶层零诊断");
}

/// 正向：出边 / 入边按 `(from, to)` 字典序、`description` 与声明一致；无出边 /
/// 无入边 → 空 `Vec`。
#[test]
fn 出边入边按from_to字典序且无邻接为空() {
    let model = Model {
        elements: vec![
            element("App"),
            element("App.Auth"),
            element("App.Billing"),
            element("App.Legacy"),
        ],
        relations: vec![
            described("App.Auth", "App.Legacy", "第三个声明"),
            described("App.Auth", "App", "第一个声明"),
            described("App.Billing", "App.Auth", "第二元素入边"),
        ],
    };
    let graph = ModelGraph::build(model);

    let out = graph.depends_on("App.Auth");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].to, "App", "字典序：App 在 App.Legacy 之前");
    assert_eq!(out[0].description.as_deref(), Some("第一个声明"));
    assert_eq!(out[1].to, "App.Legacy");
    assert_eq!(out[1].description.as_deref(), Some("第三个声明"));

    let incoming = graph.depended_by("App.Auth");
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from, "App.Billing");
    assert_eq!(incoming[0].description.as_deref(), Some("第二元素入边"));

    assert!(graph.depends_on("App.Legacy").is_empty(), "无出边为空 Vec");
    assert!(
        graph.depended_by("App.Billing").is_empty(),
        "无入边为空 Vec（App.Billing 无入边；App / App.Legacy 各有入边，不适用）"
    );
    assert!(
        graph.depends_on("App.Unknown").is_empty(),
        "未知 FQN 为空 Vec"
    );
}

/// 正向：`children` 取**最长前缀一级**（非 TOML 嵌套）；`App.Auth` 子元素恰
/// `["App.Auth.Token"]`；`App` 子元素含 `App.Auth` 与 `App.Billing`（字典序）。
#[test]
fn children按fqn前缀一级且字典序() {
    let graph = ModelGraph::build(model_of(
        &[
            "App",
            "App.Auth",
            "App.Auth.Token",
            "App.Auth.Token.Sink",
            "App.Billing",
        ],
        &[],
    ));
    assert_eq!(
        graph.children("App.Auth"),
        vec!["App.Auth.Token"],
        "一级子元素（孙元素不入）"
    );
    assert_eq!(
        graph.children("App.Auth.Token"),
        vec!["App.Auth.Token.Sink"],
        "深链逐级下推"
    );
    assert_eq!(
        graph.children("App"),
        vec!["App.Auth", "App.Billing"],
        "App 子元素按字典序"
    );
}

/// 边界：`App.AuthLegacy` 不是 `App.Auth` 的子元素（按 `.` 边界切分，非字符串
/// 前缀）；未知 FQN / `""` / 超长 FQN → 空 `Vec`；无顶层元素时层级仍按前缀判定；
/// 500 子元素 → 全量返回有序。
#[test]
fn children前缀边界与无顶层元素与规模() {
    let graph = ModelGraph::build(model_of(
        &["App", "App.Auth", "App.AuthLegacy", "App.Auth.Token"],
        &[],
    ));
    assert_eq!(
        graph.children("App.Auth"),
        vec!["App.Auth.Token"],
        "App.AuthLegacy 是兄弟而非子元素（. 边界切分）"
    );
    assert_eq!(
        graph.children("App"),
        vec!["App.Auth", "App.AuthLegacy"],
        "两者同为 App 的子元素"
    );

    assert!(graph.children("App.Unknown").is_empty(), "未知 FQN 空");
    assert!(graph.children("").is_empty(), "空串空");
    let long = "A".repeat(1200);
    assert!(graph.children(&long).is_empty(), "超长 FQN 空且不 panic");

    // 无顶层元素：层级仍按最长前缀判定（父未声明亦不改判据）
    let nested = ModelGraph::build(model_of(&["App.Auth", "App.Auth.Token"], &[]));
    assert_eq!(
        nested.children("App"),
        vec!["App.Auth"],
        "无顶层元素时子元素仍按 `.` 边界一级切分"
    );

    // 500 子元素全量有序
    let mut fqns: Vec<String> = vec!["Root".to_owned()];
    for index in (0..500).rev() {
        fqns.push(format!("Root.Child{index:03}"));
    }
    let fqn_refs: Vec<&str> = fqns.iter().map(String::as_str).collect();
    let large = ModelGraph::build(model_of(&fqn_refs, &[]));
    let children = large.children("Root");
    assert_eq!(children.len(), 500, "500 子元素全量返回");
    let mut sorted = children.clone();
    sorted.sort_unstable();
    assert_eq!(children, sorted, "返回序为字典序");
}

/// 正向：`neighborhood` 三项与 `depends_on` / `depended_by` / `children` 逐一相等；
/// 非邻域元素不出现。
#[test]
fn neighborhood三项与单项查询一致() {
    let graph = ModelGraph::build(Model {
        elements: vec![
            element("App"),
            element("App.Auth"),
            element("App.Auth.Token"),
            element("App.Billing"),
            element("App.Other"),
        ],
        relations: vec![
            relation("App.Auth", "App.Billing"),
            relation("App.Other", "App.Auth"),
        ],
    });
    let neighborhood = graph.neighborhood("App.Auth");
    assert_eq!(
        neighborhood.depends_on,
        graph.depends_on("App.Auth"),
        "出边与单项查询逐一相等"
    );
    assert_eq!(
        neighborhood.depended_by,
        graph.depended_by("App.Auth"),
        "入边与单项查询逐一相等"
    );
    assert_eq!(
        neighborhood.children,
        graph.children("App.Auth"),
        "子元素与单项查询逐一相等"
    );
    let targets: Vec<&str> = neighborhood
        .depends_on
        .iter()
        .map(|relation| relation.to.as_str())
        .collect();
    assert_eq!(targets, vec!["App.Billing"], "非邻域元素不出现");
    assert_eq!(neighborhood.children, vec!["App.Auth.Token"]);
}

// ---------------------------------------------------------------------------
// 正向 / 异常：四类自实现结构诊断
// ---------------------------------------------------------------------------

/// 正向：四类诊断各一例——环 / 依赖祖先 / 未定义引用 / 孤儿层级（各带 `file` +
/// `line`），且孤儿层级不补建隐式祖先。
#[test]
fn 四类结构诊断逐类命中且不补建隐式祖先() {
    // 环：App.A → App.B → App.A（祖先 App 显式声明，避免与孤儿层级混判）
    let cycle_graph = ModelGraph::build(model_of(
        &["App", "App.A", "App.B"],
        &[("App.A", "App.B"), ("App.B", "App.A")],
    ));
    let cycle = cycle_graph.diagnostics();
    assert_eq!(cycle.len(), 1, "一个环一条诊断");
    assert_eq!(cycle[0].kind, DiagnosticKind::Cycle);
    assert!(
        cycle[0].message.contains("App.A -> App.B -> App.A"),
        "含环路径: {}",
        cycle[0].message
    );
    assert_eq!(cycle[0].file, "openspec/architecture/models/01-core.toml");
    assert_eq!(cycle[0].line, 1, "诊断位置取环上首元素声明位置");

    // 依赖祖先：App.Auth → App
    let ancestor_graph = ModelGraph::build(model_of(&["App", "App.Auth"], &[("App.Auth", "App")]));
    let ancestor = ancestor_graph.diagnostics();
    assert_eq!(ancestor.len(), 1);
    assert_eq!(ancestor[0].kind, DiagnosticKind::DependsOnAncestor);
    assert!(
        ancestor[0].message.contains("App.Auth") && ancestor[0].message.contains("App"),
        "指明元素与祖先: {}",
        ancestor[0].message
    );
    assert_eq!(ancestor[0].line, 20, "诊断位置取关系声明位置（from 键行）");

    // 未定义引用：App.Auth → App.Shared（未声明）
    let unknown_graph = ModelGraph::build(model_of(
        &["App", "App.Auth"],
        &[("App.Auth", "App.Shared")],
    ));
    let unknown = unknown_graph.diagnostics();
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].kind, DiagnosticKind::UnknownReference);
    assert!(
        unknown[0].message.contains("App.Auth") && unknown[0].message.contains("App.Shared"),
        "指明引用方与缺失目标: {}",
        unknown[0].message
    );

    // 孤儿层级：只声明 App.Auth.Token，且不补建 App / App.Auth
    let orphan_graph = ModelGraph::build(model_of(&["App.Auth.Token"], &[]));
    let orphan = orphan_graph.diagnostics();
    assert_eq!(orphan.len(), 1);
    assert_eq!(orphan[0].kind, DiagnosticKind::OrphanHierarchy);
    assert!(
        orphan[0].message.contains("App.Auth.Token"),
        "指明孤儿元素: {}",
        orphan[0].message
    );
    assert_eq!(orphan[0].file, "openspec/architecture/models/01-core.toml");
    assert_eq!(orphan[0].line, 1, "位置指向该元素声明");
    assert!(
        orphan_graph.model().element("App").is_none(),
        "不补建隐式祖先（模型内不出现 App）"
    );
    assert!(
        orphan_graph.children("App").is_empty(),
        "不补建隐式子元素（children 不出现 App.Auth）"
    );
}

/// 异常：三错并存（环 + 未定义引用 + 孤儿）→ 三类同时在场、诊断数 == 坏点数
/// （不首个短路，AC-2）。
#[test]
fn 三类坏点同时在场不短路() {
    // A ↔ B 成环 + C 引用未声明的 Missing + `Orphan.Child` 祖先未声明
    let graph = ModelGraph::build(model_of(
        &["A", "B", "C", "Orphan.Child"],
        &[("A", "B"), ("B", "A"), ("C", "Missing")],
    ));
    let diagnostics = graph.diagnostics();
    let kinds: Vec<DiagnosticKind> = diagnostics.iter().map(|d| d.kind).collect();
    assert_eq!(kinds.len(), 3, "三类坏点各一条: {diagnostics:?}");
    assert!(kinds.contains(&DiagnosticKind::OrphanHierarchy));
    assert!(kinds.contains(&DiagnosticKind::Cycle));
    assert!(kinds.contains(&DiagnosticKind::UnknownReference));
    assert_eq!(diagnostics.len(), kinds.len(), "诊断数 == 坏点数（不短路）");
}

/// 异常：自环 → 一条 `cycle`（单点 SCC 落法）；双向 `A ↔ B` → 一条 `cycle`
/// （每组 SCC 一条，非每边一条）；两组独立环 → 恰 2 条。
#[test]
fn 环诊断按scc计_自环与双向与多组() {
    let self_loop = ModelGraph::build(model_of(&["App", "App.A"], &[("App.A", "App.A")]));
    let diagnostics = self_loop.diagnostics();
    assert_eq!(diagnostics.len(), 1, "自环一条诊断");
    assert_eq!(diagnostics[0].kind, DiagnosticKind::Cycle);

    let bidirectional = ModelGraph::build(model_of(
        &["App", "App.A", "App.B"],
        &[("App.A", "App.B"), ("App.B", "App.A")],
    ));
    assert_eq!(
        bidirectional.diagnostics().len(),
        1,
        "双向为一组 SCC，非每边一条"
    );

    let two_groups = ModelGraph::build(model_of(
        &["App", "App.A", "App.B", "App.C", "App.D"],
        &[
            ("App.A", "App.B"),
            ("App.B", "App.A"),
            ("App.C", "App.D"),
            ("App.D", "App.C"),
        ],
    ));
    let diagnostics = two_groups.diagnostics();
    assert_eq!(diagnostics.len(), 2, "两组独立环各一条");
    assert!(diagnostics.iter().all(|d| d.kind == DiagnosticKind::Cycle));
}

/// 异常：依赖祖先边界——跨两级命中；兄弟不命中；名字相近非祖先不命中；自指归
/// `cycle` 而非 `dependsOnAncestor`。
#[test]
fn 依赖祖先判据边界_跨级命中与兄弟与自指() {
    let cross = ModelGraph::build(model_of(
        &["App", "App.Auth", "App.Auth.Token"],
        &[("App.Auth.Token", "App")],
    ));
    let kinds: Vec<DiagnosticKind> = cross.diagnostics().iter().map(|d| d.kind).collect();
    assert!(
        kinds.contains(&DiagnosticKind::DependsOnAncestor),
        "跨两级依赖祖先命中: {kinds:?}"
    );

    let sibling = ModelGraph::build(model_of(
        &["App", "App.Auth", "App.Billing"],
        &[("App.Auth", "App.Billing")],
    ));
    assert!(sibling.diagnostics().is_empty(), "兄弟依赖不命中（非祖先）");

    let similar = ModelGraph::build(model_of(
        &["App", "App.Auth", "App.AuthX"],
        &[("App.Auth", "App.AuthX")],
    ));
    assert!(similar.diagnostics().is_empty(), "名字相近非祖先不命中");

    let self_reference =
        ModelGraph::build(model_of(&["App", "App.Auth"], &[("App.Auth", "App.Auth")]));
    let kinds: Vec<DiagnosticKind> = self_reference
        .diagnostics()
        .iter()
        .map(|d| d.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![DiagnosticKind::Cycle],
        "自指归 cycle，不重复计祖先"
    );
}

/// 边界：`unknownReference` 两端——`to` 未声明与 `from` 未声明各一条，均不静默
/// 建节点；引用方 / 缺失目标均在 `message` 内。
#[test]
fn 未定义引用两端各一条且不静默建节点() {
    let missing_target = ModelGraph::build(model_of(
        &["App", "App.Auth"],
        &[("App.Auth", "App.Shared")],
    ));
    let diagnostics = missing_target.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, DiagnosticKind::UnknownReference);
    assert!(diagnostics[0].message.contains("App.Shared"));
    assert!(
        missing_target.depended_by("App.Shared").is_empty(),
        "缺失目标不静默建节点（两端未齐的边不入图）"
    );
    assert_eq!(
        missing_target.model().elements.len(),
        2,
        "元素表仍只由已声明元素决定（缺失目标不建节点，既不增也不减）"
    );

    let missing_source =
        ModelGraph::build(model_of(&["App", "App.Auth"], &[("App.Ghost", "App.Auth")]));
    let diagnostics = missing_source.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, DiagnosticKind::UnknownReference);
    assert!(
        diagnostics[0].message.contains("App.Ghost"),
        "点名缺失引用方: {}",
        diagnostics[0].message
    );
    assert!(
        missing_source.depended_by("App.Auth").is_empty(),
        "两端未齐的边不入图（入边为空）"
    );
}

/// 边界：clean 图零诊断——5 元素 4 关系的完整层级（无环 / 无祖先依赖 / 无未定义
/// 引用）/ 空模型 / 只声明 `App`。
#[test]
fn 健康图零诊断三形态() {
    let clean = ModelGraph::build(model_of(
        &[
            "App",
            "App.Auth",
            "App.Auth.Token",
            "App.Billing",
            "App.Billing.Invoice",
        ],
        &[
            // 兄弟 / 跨子树依赖：均不指向自身祖先，也不成环（DAG）
            ("App.Auth", "App.Billing"),
            ("App.Auth.Token", "App.Billing"),
            ("App.Billing.Invoice", "App.Auth.Token"),
            ("App.Billing.Invoice", "App.Auth"),
        ],
    ));
    assert!(
        clean.diagnostics().is_empty(),
        "健康图零诊断: {:?}",
        clean.diagnostics()
    );

    assert!(ModelGraph::build(Model::empty()).diagnostics().is_empty());
    assert!(ModelGraph::build(model_of(&["App"], &[]))
        .diagnostics()
        .is_empty());
}

/// 边界：规模面——500 节点链（无环）→ 零诊断；500 节点成环 → 一条 `cycle` 且
/// `message` 含环上 FQN 序列（超长消息不 panic）。
#[test]
fn 规模面五百节点链与五百节点环() {
    let chain_fqns: Vec<String> = (0..500).map(|index| format!("Node{index:03}")).collect();
    let chain_refs: Vec<&str> = chain_fqns.iter().map(String::as_str).collect();
    let mut chain_relations: Vec<(&str, &str)> = Vec::new();
    for index in 0..499 {
        chain_relations.push((chain_refs[index], chain_refs[index + 1]));
    }
    let chain = ModelGraph::build(model_of(&chain_refs, &chain_relations));
    assert!(chain.diagnostics().is_empty(), "链无环即零诊断");

    let mut ring_relations: Vec<(&str, &str)> = Vec::new();
    for index in 0..500 {
        ring_relations.push((chain_refs[index], chain_refs[(index + 1) % 500]));
    }
    let ring = ModelGraph::build(model_of(&chain_refs, &ring_relations));
    let diagnostics = ring.diagnostics();
    assert_eq!(diagnostics.len(), 1, "一个大环只一条诊断");
    assert_eq!(diagnostics[0].kind, DiagnosticKind::Cycle);
    assert!(
        diagnostics[0].message.contains("Node000") && diagnostics[0].message.contains("Node499"),
        "超长消息含环上 FQN 序列（不 panic）"
    );
    assert!(diagnostics[0].message.len() > 1000, "消息确实很长");
}

/// 边界：`str` 面——FQN 含 emoji / 全中文 / 超长（>1000 chars）→ 诊断消息逐字含
/// 该 FQN，不 panic。
#[test]
fn 特殊字符fqn在诊断消息中逐字保留() {
    let long = format!("Root.{}", "X".repeat(1100));
    let cases = ["App.🚀Auth", "应用.认证", long.as_str()];
    for fqn in cases {
        let orphan = ModelGraph::build(model_of(&[fqn], &[]));
        let diagnostics = orphan.diagnostics();
        assert_eq!(diagnostics.len(), 1, "{fqn:?} 应报孤儿层级");
        assert_eq!(diagnostics[0].kind, DiagnosticKind::OrphanHierarchy);
        assert!(
            diagnostics[0].message.contains(fqn),
            "消息逐字含 FQN（不 panic）: {}",
            diagnostics[0].message
        );
    }

    // 环路径中的 emoji FQN 亦逐字
    let graph = ModelGraph::build(model_of(
        &["App", "App.🚀A", "App.🚀B"],
        &[("App.🚀A", "App.🚀B"), ("App.🚀B", "App.🚀A")],
    ));
    let diagnostics = graph.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0].message.contains("App.🚀A"),
        "环路径逐字: {}",
        diagnostics[0].message
    );
}
