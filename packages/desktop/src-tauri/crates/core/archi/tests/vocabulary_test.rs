//! `archi` 的封闭词汇三面一致性（crate 级集成测试）：进程内诊断集 / DTO 投影串 /
//! 枚举全值词表三面相等，词汇值域封闭，以及「投影零改写」与「空态 vs 停摆态」的
//! 可区分性。
//!
//! 零 mock：真实临时目录 + 真实 `parse_dir` / `ModelGraph::build` / `dto::*` 组合
//! （三面同源即被测语义，任何一层替身都会抹掉判据）；不读 `bindings.ts` 产物、不
//! 依赖 IPC runtime。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use archi::dto::{
    self, DiagnosticKind, DocKind, DocSource, EdgeMark, ElementKind, GateTier, NodeMark,
    SearchScope, WarningKind,
};
use archi::graph::ModelGraph;
use archi::parse;
use serde_json::Value;

// ---------------------------------------------------------------------------
// 真实临时目录（crate 无 tempfile dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-vocabulary-test-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("openspec/architecture/models")).expect("建模型目录失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn write_model(&self, name: &str, content: &str) {
        fs::write(
            self.0.join("openspec/architecture/models").join(name),
            content,
        )
        .expect("写模型文件失败");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 封闭词汇全值清单（测试内字面量镜像：多一值或少一值即红）。
const ELEMENT_KINDS: [&str; 4] = ["package", "domain", "module", "component"];
const GATE_TIERS: [&str; 3] = ["off", "warn", "strict"];
const DIAGNOSTIC_KINDS: [&str; 9] = [
    "syntaxError",
    "missingField",
    "invalidValue",
    "unknownField",
    "duplicateElement",
    "cycle",
    "dependsOnAncestor",
    "unknownReference",
    "orphanHierarchy",
];
const WARNING_KINDS: [&str; 1] = ["docNotFound"];
const DOC_KINDS: [&str; 2] = ["md", "adr"];
const DOC_SOURCES: [&str; 3] = ["explicit", "adjacent", "text"];
const EDGE_MARKS: [&str; 4] = ["none", "unfulfilled", "crossKind", "exempt"];
const NODE_MARKS: [&str; 2] = ["none", "unfulfilled"];
const SEARCH_SCOPES: [&str; 3] = ["elements", "docs", "all"];

/// 枚举线值（`serde_json` 单值序列化）。
fn line_of<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .expect("序列化成功")
        .as_str()
        .expect("线值为字符串")
        .to_owned()
}

/// 全词汇语料：九类结构诊断各一坏点（解析面五 + 图面四）。
fn all_vocabulary_corpus(tag: &str) -> TempDir {
    let temp = TempDir::new(tag);
    temp.write_model("01-syntax.toml", "[element.\"App]\nkind = \"package\"\n");
    temp.write_model(
        "02-missing.toml",
        "[element.\"App.Missing\"]\ngate = \"off\"\n",
    );
    temp.write_model(
        "03-invalid.toml",
        "[element.\"App.Invalid\"]\nkind = \"pakage\"\n",
    );
    temp.write_model(
        "04-unknown.toml",
        "[element.\"App.Unknown\"]\nkind = \"module\"\ngatte = \"x\"\n",
    );
    temp.write_model("05-dup.toml", "[element.\"App.Dup\"]\nkind = \"module\"\n");
    temp.write_model("06-dup.toml", "[element.\"App.Dup\"]\nkind = \"domain\"\n");
    temp.write_model(
        "07-graph.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Cycle1\"]\nkind = \"module\"\n\n",
            "[element.\"App.Cycle2\"]\nkind = \"module\"\n\n",
            "[element.\"App.Ancestor\"]\nkind = \"domain\"\n\n",
            "[element.\"App.Ref\"]\nkind = \"module\"\n\n",
            "[element.\"App.Orphan.Child\"]\nkind = \"component\"\n\n",
            "[[relation]]\nfrom = \"App.Cycle1\"\nto = \"App.Cycle2\"\n\n",
            "[[relation]]\nfrom = \"App.Cycle2\"\nto = \"App.Cycle1\"\n\n",
            "[[relation]]\nfrom = \"App.Ancestor\"\nto = \"App\"\n\n",
            "[[relation]]\nfrom = \"App.Ref\"\nto = \"App.Missing\"\n",
        ),
    );
    temp
}

/// 健康全值语料：四种元素种类 × 三档 gate × 关系（边 / 节点标记）。
fn healthy_vocabulary_corpus(tag: &str) -> TempDir {
    let temp = TempDir::new(tag);
    temp.write_model(
        "01-core.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\ngate = \"off\"\n\n",
            "[element.\"App.Domain\"]\nkind = \"domain\"\ngate = \"warn\"\n\n",
            "[element.\"App.Domain.Module\"]\nkind = \"module\"\ngate = \"strict\"\n\n",
            "[element.\"App.Domain.Module.Component\"]\nkind = \"component\"\n\n",
            "[element.\"App.Billing\"]\nkind = \"domain\"\n\n",
            "[[relation]]\nfrom = \"App.Domain.Module\"\nto = \"App.Billing\"\n",
        ),
    );
    temp
}

/// 进程内诊断面：解析诊断 ∪ 图自实现诊断 → 线值集合。
fn diagnostic_face(root: &Path) -> BTreeSet<String> {
    let outcome = parse::parse_dir(root);
    let mut kinds: BTreeSet<String> = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.kind.as_str().to_owned())
        .collect();
    let graph = ModelGraph::build(outcome.model.clone());
    kinds.extend(
        graph
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.kind.as_str().to_owned()),
    );
    kinds
}

/// 检索 JSON 串中出现的词汇字面量（值面：只收已知词汇表内的命中）。
fn vocabulary_in(value: &Value, vocabulary: &[&str]) -> BTreeSet<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    fn walk(value: &Value, vocabulary: &[&str], found: &mut BTreeSet<String>) {
        match value {
            Value::String(text) if vocabulary.contains(&text.as_str()) => {
                found.insert(text.clone());
            }
            Value::String(_) => {}
            Value::Array(items) => {
                for item in items {
                    walk(item, vocabulary, found);
                }
            }
            Value::Object(map) => {
                for item in map.values() {
                    walk(item, vocabulary, found);
                }
            }
            _ => {}
        }
    }
    walk(value, vocabulary, &mut found);
    found
}

/// 缺失词汇点名（「新增 kind 而语料未加坏点」的可读失败面）。
fn missing_vocabulary(observed: &BTreeSet<String>, expected: &[&str]) -> Result<(), String> {
    let missing: Vec<&str> = expected
        .iter()
        .copied()
        .filter(|name| !observed.contains(*name))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("语料缺失词汇：{}", missing.join(" / ")))
    }
}

// ---------------------------------------------------------------------------
// 三面词汇集一致
// ---------------------------------------------------------------------------

/// 正向：三面（进程内诊断 / DTO 投影串 / 枚举全值词表）词汇集相等，失败时点名
/// 缺失词汇。
#[test]
fn 三面词汇集相等且缺失即点名() {
    let temp = all_vocabulary_corpus("three-face");
    let process_face = diagnostic_face(temp.root());
    let graph = dto::graph_view(temp.root()).expect("停摆态仍 Ok");
    let dto_face = vocabulary_in(
        &serde_json::to_value(&graph).expect("序列化成功"),
        &DIAGNOSTIC_KINDS,
    );
    let mut vocabulary_face: BTreeSet<String> = BTreeSet::new();
    for kind in [
        DiagnosticKind::SyntaxError,
        DiagnosticKind::MissingField,
        DiagnosticKind::InvalidValue,
        DiagnosticKind::UnknownField,
        DiagnosticKind::DuplicateElement,
        DiagnosticKind::Cycle,
        DiagnosticKind::DependsOnAncestor,
        DiagnosticKind::UnknownReference,
        DiagnosticKind::OrphanHierarchy,
    ] {
        vocabulary_face.insert(kind.as_str().to_owned());
    }

    assert_eq!(
        vocabulary_face.len(),
        DIAGNOSTIC_KINDS.len(),
        "枚举全值数 == 词表条目数"
    );
    assert_eq!(process_face, vocabulary_face, "进程内诊断面 == 枚举词表");
    assert_eq!(dto_face, vocabulary_face, "DTO 投影串面 == 枚举词表");
    assert_eq!(
        missing_vocabulary(&process_face, &DIAGNOSTIC_KINDS),
        Ok(()),
        "全词汇语料覆盖九值"
    );
}

/// 边界：封闭词汇值域逐组断言（多一值 / 少一值即红；`NodeMark` 不含结构错值）。
#[test]
fn 封闭词汇值域九组() {
    let element_kinds: Vec<&str> = [
        ElementKind::Package,
        ElementKind::Domain,
        ElementKind::Module,
        ElementKind::Component,
    ]
    .into_iter()
    .map(ElementKind::as_str)
    .collect();
    assert_eq!(element_kinds, ELEMENT_KINDS.to_vec());

    let gate_tiers: Vec<&str> = [GateTier::Off, GateTier::Warn, GateTier::Strict]
        .into_iter()
        .map(GateTier::as_str)
        .collect();
    assert_eq!(gate_tiers, GATE_TIERS.to_vec());
    assert_eq!(GateTier::ALL.len(), 3, "GateTier::ALL 与词表同长");

    let diagnostics: Vec<&str> = [
        DiagnosticKind::SyntaxError,
        DiagnosticKind::MissingField,
        DiagnosticKind::InvalidValue,
        DiagnosticKind::UnknownField,
        DiagnosticKind::DuplicateElement,
        DiagnosticKind::Cycle,
        DiagnosticKind::DependsOnAncestor,
        DiagnosticKind::UnknownReference,
        DiagnosticKind::OrphanHierarchy,
    ]
    .into_iter()
    .map(DiagnosticKind::as_str)
    .collect();
    assert_eq!(diagnostics, DIAGNOSTIC_KINDS.to_vec());

    assert_eq!(WarningKind::DocNotFound.as_str(), WARNING_KINDS[0]);

    assert_eq!(
        vec![line_of(DocKind::Md), line_of(DocKind::Adr)],
        DOC_KINDS.to_vec()
    );
    assert_eq!(
        vec![
            line_of(DocSource::Explicit),
            line_of(DocSource::Adjacent),
            line_of(DocSource::Text)
        ],
        DOC_SOURCES.to_vec()
    );
    assert_eq!(
        vec![
            line_of(EdgeMark::None),
            line_of(EdgeMark::Unfulfilled),
            line_of(EdgeMark::CrossKind),
            line_of(EdgeMark::Exempt)
        ],
        EDGE_MARKS.to_vec()
    );
    assert_eq!(
        vec![line_of(NodeMark::None), line_of(NodeMark::Unfulfilled)],
        NODE_MARKS.to_vec(),
        "节点标记恰两值（结构错不入标记，D6）"
    );
    assert!(
        !NODE_MARKS.contains(&"cycle") && !NODE_MARKS.contains(&"orphanHierarchy"),
        "结构错值不在节点标记词汇内"
    );
    assert_eq!(
        vec![
            line_of(SearchScope::Elements),
            line_of(SearchScope::Docs),
            line_of(SearchScope::All)
        ],
        SEARCH_SCOPES.to_vec()
    );
}

/// 异常：语料只含 8 类诊断而词表有 9 值 → 集合不等即红，错误消息列出缺失词汇。
#[test]
fn 新增词汇未同步即点名缺失() {
    let temp = all_vocabulary_corpus("missing-vocab");
    let process_face = diagnostic_face(temp.root());
    let mut observed = process_face.clone();
    observed.remove("orphanHierarchy");
    let error = missing_vocabulary(&observed, &DIAGNOSTIC_KINDS).expect_err("缺一值应红");
    assert!(
        error.contains("orphanHierarchy"),
        "错误消息列出缺失词汇: {error}"
    );
    assert_eq!(
        missing_vocabulary(&process_face, &DIAGNOSTIC_KINDS),
        Ok(()),
        "对照臂：全量语料无缺失"
    );
}

/// 正向：健康全值语料 → 元素种类四值、gate 三档、边 / 节点标记 `none` 均经 DTO
/// 投影串可观测（第三面在元素 / 关系维度的等价断言）。
#[test]
fn 投影串覆盖元素种类与档位与标记词汇() {
    let temp = healthy_vocabulary_corpus("healthy-vocab");
    let graph = dto::graph_view(temp.root()).expect("健康模型 Ok");
    assert!(graph.diagnostics.is_empty(), "健康语料零诊断");
    let value = serde_json::to_value(&graph).expect("序列化成功");

    let kinds = vocabulary_in(&value, &ELEMENT_KINDS);
    assert_eq!(
        kinds,
        ELEMENT_KINDS.iter().map(|k| (*k).to_owned()).collect()
    );
    let tiers = vocabulary_in(&value, &GATE_TIERS);
    assert_eq!(
        tiers,
        vec!["off".to_owned(), "warn".to_owned(), "strict".to_owned()]
            .into_iter()
            .collect()
    );
    let marks = vocabulary_in(&value, &["none"]);
    assert_eq!(marks.len(), 1, "本期边 / 节点标记恒 none");
}

// ---------------------------------------------------------------------------
// 三面同源计数 + 投影零改写 + 空态可区分
// ---------------------------------------------------------------------------

/// 正向：同一语料下 DTO 元素字段与图查询逐一相同（单点构建 + 投影零改写）。
#[test]
fn 三面同源计数与声明位置逐字相同() {
    let temp = healthy_vocabulary_corpus("same-source");
    let detail = dto::module_detail(temp.root(), "App.Domain.Module").expect("元素在场");
    let outcome = parse::parse_dir(temp.root());
    let graph = ModelGraph::build(outcome.model.clone());
    let element = graph
        .model()
        .element("App.Domain.Module")
        .expect("模型侧元素在场");

    assert_eq!(
        detail.depends_on.len(),
        graph.depends_on("App.Domain.Module").len(),
        "出边条数同源"
    );
    assert_eq!(
        detail.depended_by.len(),
        graph.depended_by("App.Domain.Module").len(),
        "入边条数同源"
    );
    assert_eq!(
        detail.children.len(),
        graph.children("App.Domain.Module").len(),
        "子元素条数同源"
    );
    assert_eq!(
        detail.element.model_file, element.file,
        "modelFile 与 Model::element 字段逐字相同"
    );
    assert_eq!(
        detail.element.model_line, element.line,
        "modelLine 与 Model::element 字段逐字相同"
    );
    assert_eq!(detail.element.fqn, element.fqn);
    assert_eq!(detail.element.gate, element.gate);
}

/// 边界：元素无 `path` / `docs` / `summary` / `metadata` 时 DTO 串含显式
/// `[]` / `null` / `{}`（字段不省略、不填占位串）。
#[test]
fn 投影零改写显式空值四字段() {
    let temp = healthy_vocabulary_corpus("explicit-empty");
    let graph = dto::graph_view(temp.root()).expect("Ok");
    let value = serde_json::to_value(&graph).expect("序列化成功");
    let empty = value["elements"]
        .as_array()
        .expect("元素数组")
        .iter()
        .find(|element| element["fqn"] == serde_json::json!("App"))
        .expect("App 在场");
    assert_eq!(empty["path"], serde_json::json!([]), "path 显式空数组");
    assert_eq!(
        empty["metadata"],
        serde_json::json!({}),
        "metadata 显式空表"
    );
    assert_eq!(
        empty["summary"],
        serde_json::json!(null),
        "summary 显式 null"
    );
    assert!(
        empty.as_object().expect("对象").contains_key("summary"),
        "字段不省略"
    );

    // 元素侧无文档关联 → 详情里 docs / warnings 为显式空数组
    let detail = dto::module_detail(temp.root(), "App").expect("Ok");
    assert!(detail.docs.is_empty() && detail.warnings.is_empty());
    let detail_value = serde_json::to_value(&detail).expect("序列化成功");
    assert_eq!(detail_value["docs"], serde_json::json!([]));
    assert_eq!(detail_value["warnings"], serde_json::json!([]));
}

/// 边界：空模型（零元素零关系零诊断）→ 三数组皆空且 `diagnostics` 空；含一个坏点
/// → 三数组皆空但 `diagnostics` 非空（判据 = `diagnostics` 非空，无布尔位）。
#[test]
fn 空模型与停摆态可区分() {
    let empty = TempDir::new("empty-model");
    let graph = dto::graph_view(empty.root()).expect("Ok");
    assert!(graph.elements.is_empty() && graph.relations.is_empty());
    assert!(graph.diagnostics.is_empty(), "空模型零诊断");

    let halted = TempDir::new("halted-model");
    halted.write_model("01.toml", "[element.\"App.Auth\"]\nkind = \"domain\"\n");
    let graph = dto::graph_view(halted.root()).expect("Ok");
    assert!(graph.elements.is_empty() && graph.relations.is_empty());
    assert!(!graph.diagnostics.is_empty(), "停摆判据 = diagnostics 非空");

    let value = serde_json::to_value(&graph).expect("序列化成功");
    assert_eq!(
        value["diagnostics"]
            .as_array()
            .expect("诊断数组")
            .first()
            .expect("首条诊断")["kind"],
        serde_json::json!("orphanHierarchy"),
        "线面 kind 为 camelCase 词"
    );
    assert!(
        value.get("halted").is_none(),
        "无布尔停摆位（判据只有 diagnostics）"
    );
}
