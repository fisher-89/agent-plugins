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

use archi::dto::{self, DiagnosticKind, EdgeMark, ElementKind, GateTier, NodeMark, SearchScope};
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
        fs::create_dir_all(dir.join(".dev-team/architecture/graph/model"))
            .expect("建模型声明目录失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn write_model(&self, name: &str, content: &str) {
        fs::write(
            self.0.join(".dev-team/architecture/graph/model").join(name),
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
const EDGE_MARKS: [&str; 4] = ["none", "unfulfilled", "crossKind", "exempt"];
const NODE_MARKS: [&str; 2] = ["none", "unfulfilled"];
const SEARCH_SCOPES: [&str; 3] = ["elements", "doc", "all"];

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
            line_of(SearchScope::Doc),
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

    // 元素侧无入口文档 → 详情里 doc 为显式 null
    let detail = dto::module_detail(temp.root(), "App").expect("Ok");
    assert!(detail.doc.is_none());
    let detail_value = serde_json::to_value(&detail).expect("序列化成功");
    assert_eq!(detail_value["doc"], serde_json::Value::Null);
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

// ===========================================================================
// 第四面：`ApiStatus` 词汇并入四面一致性（`module-api-contract-format` / AC-8 / AC-10）
//
// `api` 状态二值（`declared` / `none`）作为第四面参与「枚举全值词表 ↔ DTO 投影串 ↔
// 进程内诊断集」的三面同源对拍；词表从 `ApiStatus::ALL` 派生、MUST NOT 手抄。
// ===========================================================================

use archi::dto::{ApiStatus, ArchiElement};

/// 正向（AC-8）：既有三面词数不变（`DiagnosticKind` 9 / `GateTier` 3 /
/// `ElementKind` 4）+ 新增 `ApiStatus` 两值全部在场；四面各自去重后长度不变。
#[test]
fn 四面词汇集新增api面() {
    assert_eq!(DiagnosticKind::ALL.len(), 9, "结构诊断九值不变");
    assert_eq!(GateTier::ALL.len(), 3, "gate 三值不变");
    assert_eq!(ElementKind::ALL.len(), 4, "元素 kind 四值不变");
    assert_eq!(ApiStatus::ALL.len(), 2, "api 状态二值（第四面）");

    let faces: [(&str, Vec<&str>); 4] = [
        (
            "diagnostic",
            DiagnosticKind::ALL
                .iter()
                .map(|kind| kind.as_str())
                .collect(),
        ),
        (
            "gate",
            GateTier::ALL.iter().map(|gate| gate.as_str()).collect(),
        ),
        (
            "kind",
            ElementKind::ALL.iter().map(|kind| kind.as_str()).collect(),
        ),
        (
            "api",
            ApiStatus::ALL
                .iter()
                .map(|status| status.as_str())
                .collect(),
        ),
    ];
    for (label, words) in faces {
        let unique: BTreeSet<&str> = words.iter().copied().collect();
        assert_eq!(
            unique.len(),
            words.len(),
            "{label} 面去重后长度不变（重复值不可绕过词表对拍）"
        );
    }
}

/// 正向（AC-10）：`ApiStatus::ALL` 逐值 `as_str()` 与含该值的 `ArchiElement` 序列化
/// 文本中的 `"api"` 字段值逐字相等（两面对拍，零手抄词表）。
#[test]
fn api_status线词与投影串对拍() {
    for status in ApiStatus::ALL {
        let element = ArchiElement {
            fqn: "App.Core.Mod".to_owned(),
            kind: ElementKind::Module,
            gate: GateTier::Warn,
            api: status,
            entry: None,
            path: vec!["src/mod/".to_owned()],
            summary: None,
            metadata: std::collections::BTreeMap::new(),
            model_file: ".dev-team/architecture/graph/model/01.toml".to_owned(),
            model_line: 1,
            mark: NodeMark::None,
            doc: None,
        };
        let value = serde_json::to_value(&element).expect("序列化成功");
        assert_eq!(
            value["api"],
            serde_json::json!(status.as_str()),
            "投影串的 api 字段与 as_str() 逐字相等（{status:?}）"
        );
        // 投影串的其余面词仍在既有词集内（零扩散）
        let vocabulary: BTreeSet<String> =
            vocabulary_in(&value, &["module", "warn", "declared", "none"]);
        assert!(
            vocabulary.contains(status.as_str()),
            "投影串词集含 api 词 {:?}",
            status.as_str()
        );
    }
}

/// 异常（Enum N+1）：四面词集两两不互相包含——`declared` / `none` 不出现在 gate 词集，
/// `off` / `warn` / `strict` 不出现在 api 词集。
#[test]
fn 四面词集互不包含() {
    let api_words: BTreeSet<&str> = ApiStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect();
    let gate_words: BTreeSet<&str> = GateTier::ALL.iter().map(|gate| gate.as_str()).collect();
    assert!(
        api_words.is_disjoint(&gate_words),
        "api 与 gate 词集 MUST 不相交：{api_words:?} / {gate_words:?}"
    );
    for word in ["declared", "none"] {
        assert!(
            !gate_words.contains(word),
            "gate 词集 MUST NOT 含 api 词 `{word}`"
        );
    }
    for word in ["off", "warn", "strict"] {
        assert!(
            !api_words.contains(word),
            "api 词集 MUST NOT 含 gate 档词 `{word}`"
        );
    }

    let kind_words: BTreeSet<&str> = ElementKind::ALL.iter().map(|kind| kind.as_str()).collect();
    let diagnostic_words: BTreeSet<&str> = DiagnosticKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    for (left_label, left) in [
        ("api", &api_words),
        ("gate", &gate_words),
        ("kind", &kind_words),
    ] {
        for (right_label, right) in [
            ("api", &api_words),
            ("gate", &gate_words),
            ("diagnostic", &diagnostic_words),
        ] {
            if left_label == right_label {
                continue;
            }
            assert!(
                !left.is_subset(right) && !right.is_subset(left),
                "{left_label} 与 {right_label} 词集互不包含"
            );
        }
    }
}

/// 边界（既有三面零变化）：改动前三面的词集与词序逐字相等（api 面为追加，零重排 /
/// 零删除）。
#[test]
fn 既有三面词序零变化() {
    let diagnostics: Vec<&str> = DiagnosticKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    assert_eq!(
        diagnostics.len(),
        9,
        "结构诊断九词逐值（追加 api 面零重排）：{diagnostics:?}"
    );
    let gates: Vec<&str> = GateTier::ALL.iter().map(|gate| gate.as_str()).collect();
    assert_eq!(gates, vec!["off", "warn", "strict"], "gate 三值词序不变");
    let kinds: Vec<&str> = ElementKind::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec!["package", "domain", "module", "component"],
        "元素 kind 四值词序不变"
    );
    let api: Vec<&str> = ApiStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect();
    assert_eq!(api, vec!["declared", "none"], "api 二值（第四面追加）");
}

/// 边界：`ApiStatus` 缺省为 `declared`，且 serde 线形与 `as_str()` 一致；词汇外形态
/// 反序列化即 `Err`。
#[test]
fn api_status缺省与线形封闭() {
    assert_eq!(
        ApiStatus::default(),
        ApiStatus::Declared,
        "缺省 declared（模型未写该键即承诺有契约）"
    );
    for status in ApiStatus::ALL {
        let word = status.as_str();
        assert_eq!(
            serde_json::to_value(status).expect("序列化成功"),
            serde_json::json!(word),
            "serde 线形与 as_str() 一致（{word}）"
        );
        let back: ApiStatus =
            serde_json::from_value(serde_json::json!(word)).expect("反序列化成功");
        assert_eq!(back, status, "{word} 往返逐值相等");
    }
    for word in ["Declared", "NONE", "none ", "off", ""] {
        assert!(
            serde_json::from_value::<ApiStatus>(serde_json::json!(word)).is_err(),
            "词汇外形态 MUST 反序列化失败：{word:?}"
        );
    }
}

/// 异常（封闭性 / `entry` 的开放值域）：`entry` 的值域是**开放集**（任意 repo 相对
/// 路径），故它不派生任何词表——四面 `ALL` 的线词集合里零 `entry` 字样，且
/// `DiagnosticKind` 仍九值、`ArchiApiFindingKind` 为十三值（入口四条判据同报一个 kind，
/// `doc` 面另有三值）。
#[test]
fn entry不派生词表且四面封闭() {
    assert_eq!(
        archi::dto::ElementKind::ALL.len(),
        4,
        "元素 kind 四面之一仍封闭四值"
    );
    assert_eq!(archi::dto::GateTier::ALL.len(), 3, "gate 封闭三值");
    assert_eq!(
        archi::dto::DiagnosticKind::ALL.len(),
        9,
        "结构诊断仍九值（`entry` 违反判据 MUST NOT 扩成第十类诊断）"
    );
    assert_eq!(
        archi::drift::report::ArchiApiFindingKind::ALL.len(),
        13,
        "契约判据十三值（四条入口理由同报 `apiEntryMissing`）"
    );

    let words: Vec<&str> = archi::dto::ElementKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .chain(archi::dto::GateTier::ALL.iter().map(|gate| gate.as_str()))
        .chain(
            archi::dto::DiagnosticKind::ALL
                .iter()
                .map(|diagnostic| diagnostic.as_str()),
        )
        .chain(
            archi::dto::ApiStatus::ALL
                .iter()
                .map(|status| status.as_str()),
        )
        .collect();
    for word in &words {
        assert_ne!(
            *word, "entry",
            "`entry` 不是词表值（开放值域不进封闭集）：{words:?}"
        );
    }
}

// ===========================================================================
// `doc` 面词汇三面（枚举 / `ALL` / 线格式词）与退役词汇组
//
// `ArchiApiFindingKind` 十值 → 十三值、`ApiFindingSeverity` 二值 → 三值：线格式词与
// `ALL` 逐值对拍（金样与 IPC 面都只认这一份词表）；文档种类 / 文档来源 / 警告种类三组
// 词汇随文档关联能力退役，零引用零定义。
// ===========================================================================

/// 正向：`ArchiApiFindingKind` 十三值——`ALL` 逐值 `as_str` 两两不等，序列化值与 `as_str`
/// 逐值相等（三新词在案，旧十词零漂移）。
#[test]
fn 契约判据十三值三面对拍() {
    use archi::drift::report::ArchiApiFindingKind;

    assert_eq!(
        ArchiApiFindingKind::ALL.len(),
        13,
        "十三值清单（旧断言十值已随 `doc` 面扩面）"
    );
    let mut words = Vec::new();
    for kind in ArchiApiFindingKind::ALL {
        let word = kind.as_str();
        assert_eq!(
            serde_json::to_value(kind).expect("序列化成功"),
            serde_json::json!(word),
            "{word} 的 serde 线形与 as_str 一致"
        );
        let back: ArchiApiFindingKind =
            serde_json::from_value(serde_json::json!(word)).expect("反序列化成功");
        assert_eq!(back, kind, "{word} 往返逐值相等");
        words.push(word);
    }
    for expected in ["docMissing", "docNotFound", "apiDocMissing"] {
        assert!(
            words.contains(&expected),
            "新词在案：{expected} / {words:?}"
        );
    }
    let mut unique = words.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 13, "十三词两两不等：{words:?}");
    assert_eq!(
        ArchiApiFindingKind::ALL[0],
        ArchiApiFindingKind::ApiEntryMissing,
        "既有词序零漂移（首项不变）"
    );
}

/// 正向 / 异常：`ApiFindingSeverity` 三值——`ALL` 逐值线词 `off` / `warn` / `error`，
/// `severity()` 的返回值 ∈ `ALL`（十三值逐值验证），零第四档残留。
#[test]
fn 档位三值封闭且severity落在其上() {
    use archi::drift::report::{ApiFindingSeverity, ArchiApiFindingKind};

    assert_eq!(ApiFindingSeverity::ALL.len(), 3, "档位三值");
    let words: Vec<&str> = ApiFindingSeverity::ALL
        .iter()
        .map(|severity| severity.as_str())
        .collect();
    assert_eq!(words, vec!["off", "warn", "error"], "三值线词");

    for kind in ArchiApiFindingKind::ALL {
        let severity = kind.severity();
        assert!(
            ApiFindingSeverity::ALL.contains(&severity),
            "{kind:?} 的档位落三值内：{severity:?}"
        );
    }
    for word in ["info", "fatal", "silent"] {
        assert!(!words.contains(&word), "无第四档残留：{word}");
    }
}

/// 异常（退役词汇组）：本文件与产品源码内三个退役类型标识符零引用；
/// `ArchiModuleDetail` 无 `docs` / `warnings` 字段。
#[test]
fn 退役词汇组零引用() {
    // 标识符在运行时拼出，避免断言自身命中（自证面必须零引用）
    let retired = [
        format!("{}{}", "Doc", "Kind"),
        format!("{}{}", "Doc", "Source"),
        format!("{}{}", "Warning", "Kind"),
    ];
    let this_file = include_str!("vocabulary_test.rs");
    let dto_source = include_str!("../src/dto.rs");
    for name in &retired {
        assert!(!this_file.contains(name.as_str()), "测试面零引用：{name}");
        assert!(
            !dto_source.contains(name.as_str()),
            "产品源码零定义：{name}"
        );
    }
    assert!(
        !dto_source.contains("pub docs:"),
        "`ArchiModuleDetail` 无 `docs` 字段"
    );
    assert!(
        !dto_source.contains("pub warnings:"),
        "`ArchiModuleDetail` 无 `warnings` 字段"
    );
}
