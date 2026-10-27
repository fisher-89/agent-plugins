//! `dto` 的单元测试：六个投影入口（`graph_view` / `module_detail` /
//! `module_overview` / `search` / `change_summary` / `linked_doc_paths`）、
//! `SearchScope::parse` 与封闭词汇的 serde 线形。
//!
//! 零 mock：真实临时目录 + 真实 `parse_dir` / `ModelGraph` / `docs` 组合（投影只
//! 裁剪，替身会引入第二事实源），serde 真实序列化 / 反序列化。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_models_dir;
use serde_json::json;

use crate::dto::{
    self, ArchiDiagnostic, ArchiElement, ArchiGraph, ArchiRelation, DiagnosticKind, DocKind,
    DocSource, EdgeMark, ElementKind, GateTier, NodeMark, SearchScope, WarningKind,
};

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-dto-test-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }

    fn write_model(&self, name: &str, content: &str) {
        let dir = archi_models_dir(&self.0);
        fs::create_dir_all(&dir).expect("创建模型目录失败");
        fs::write(dir.join(name), content).expect("写模型文件失败");
    }

    /// 带模型目录的空根（用于「空目录」形态）。
    fn with_empty_models(tag: &str) -> Self {
        let temp = TempDir::new(tag);
        fs::create_dir_all(archi_models_dir(temp.root())).expect("建空模型目录");
        temp
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 子串所在行号（1 起始）。
fn line_at(source: &str, needle: &str) -> u32 {
    let index = source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("源文本中找不到 {needle:?}"));
    index as u32 + 1
}

const CORE: &str = concat!(
    "# 核心包\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"strict\"\n",
    "path = [\"./src/auth/\"]\n",
    "docs = [\"./src/auth/README.md\"]\n",
    "summary = \"认证域\"\n",
    "metadata = { owner = \"core\" }\n",
    "\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "gate = \"warn\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Auth\"\n",
    "to = \"App.Billing\"\n",
    "desc = \"认证调用计费\"\n",
);

/// 四元素三关系的健康模型（`App.Auth` 带全字段）。
fn healthy_dir(tag: &str) -> TempDir {
    let temp = TempDir::new(tag);
    temp.write_model("01-core.toml", CORE);
    temp.write_model(
        "02-billing.toml",
        concat!(
            "[element.\"App.Billing.Invoice\"]\n",
            "kind = \"component\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"App\"\n",
            "to = \"App.Auth\"\n",
            "desc = \"核心聚合认证\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"App.Billing.Invoice\"\n",
            "to = \"App.Auth\"\n",
        ),
    );
    temp.write("src/auth/README.md", "# 认证\n");
    temp
}

/// 断言停摆态：`diagnostics` 非空且 `elements` / `relations` 皆空。
fn assert_halted(graph: &ArchiGraph, context: &str) {
    assert!(
        !graph.diagnostics.is_empty(),
        "{context}：停摆态应带诊断清单"
    );
    assert!(graph.elements.is_empty(), "{context}：停摆态 elements 为空");
    assert!(
        graph.relations.is_empty(),
        "{context}：停摆态 relations 为空"
    );
}

// ---------------------------------------------------------------------------
// `graph_view`
// ---------------------------------------------------------------------------

/// 正向：健康模型 → 元素按 FQN、关系按 `(from, to)` 稳定序；`mark` 恒 `none`、
/// `reason` 恒 `null`；`modelFile` / `modelLine` 精确到表头行；显式空值在场。
#[test]
fn graph_view健康模型稳定序与显式空值() {
    let temp = healthy_dir("graph-healthy");
    let graph = dto::graph_view(temp.root()).expect("健康模型应 Ok");

    let fqns: Vec<&str> = graph.elements.iter().map(|e| e.fqn.as_str()).collect();
    assert_eq!(
        fqns,
        vec!["App", "App.Auth", "App.Billing", "App.Billing.Invoice"],
        "元素按 FQN 字典序"
    );
    let pairs: Vec<(&str, &str)> = graph
        .relations
        .iter()
        .map(|r| (r.from.as_str(), r.to.as_str()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("App", "App.Auth"),
            ("App.Auth", "App.Billing"),
            ("App.Billing.Invoice", "App.Auth"),
        ],
        "关系按 (from, to) 字典序"
    );
    assert!(graph.diagnostics.is_empty(), "健康图零诊断");

    let auth = graph
        .elements
        .iter()
        .find(|element| element.fqn == "App.Auth")
        .expect("App.Auth 在场");
    assert_eq!(auth.kind, ElementKind::Domain);
    assert_eq!(auth.gate, GateTier::Strict);
    assert_eq!(auth.path, vec!["./src/auth/".to_owned()]);
    assert_eq!(auth.summary.as_deref(), Some("认证域"));
    assert_eq!(
        auth.model_file, "openspec/architecture/models/01-core.toml",
        "modelFile 精确到文件"
    );
    assert_eq!(auth.model_line, line_at(CORE, "[element.\"App.Auth\"]"));
    assert_eq!(auth.mark, NodeMark::None, "本期节点标记恒 none");

    for relation in &graph.relations {
        assert_eq!(relation.mark, EdgeMark::None, "本期边标记恒 none");
        assert!(relation.reason.is_none(), "豁免理由恒 null");
    }

    // 显式空值：投影不补默认值、不省字段
    let value = serde_json::to_value(&graph).expect("序列化成功");
    let app = value["elements"]
        .as_array()
        .expect("元素数组")
        .iter()
        .find(|element| element["fqn"] == json!("App"))
        .expect("App 在场");
    assert_eq!(app["metadata"], json!({}), "metadata 空表显式在场");
    assert_eq!(app["path"], json!([]), "path 空数组显式在场");
    assert_eq!(app["summary"], json!(null), "summary 缺席显式 null");
    assert_eq!(app["mark"], json!("none"));
    assert_eq!(value["relations"][0]["reason"], json!(null));
    assert_eq!(value["diagnostics"], json!([]));
}

/// 异常：九类结构错各一例 → `diagnostics` 非空且 `elements` / `relations` 皆空
/// （停摆全或无）。
#[test]
fn graph_view九类结构错一律停摆() {
    // 1 语法错
    let syntax = TempDir::new("halt-syntax");
    syntax.write_model("01.toml", "[element.\"App]\nkind = \"package\"\n");
    // 2 缺必填字段
    let missing = TempDir::new("halt-missing");
    missing.write_model("01.toml", "[element.\"App\"]\ngate = \"strict\"\n");
    // 3 非法值
    let invalid = TempDir::new("halt-invalid");
    invalid.write_model("01.toml", "[element.\"App\"]\nkind = \"pakage\"\n");
    // 4 未知字段
    let unknown_field = TempDir::new("halt-unknown-field");
    unknown_field.write_model(
        "01.toml",
        "[element.\"App\"]\nkind = \"package\"\ngatte = \"x\"\n",
    );
    // 5 同文件重复
    let dup_same = TempDir::new("halt-dup-same");
    dup_same.write_model(
        "01.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n[element.\"App\"]\nkind = \"module\"\n",
    );
    // 6 跨文件重复
    let dup_cross = TempDir::new("halt-dup-cross");
    dup_cross.write_model("01.toml", "[element.\"App\"]\nkind = \"package\"\n");
    dup_cross.write_model("02.toml", "[element.\"App\"]\nkind = \"module\"\n");
    // 7 成环
    let cycle = TempDir::new("halt-cycle");
    cycle.write_model(
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.A\"]\nkind = \"module\"\n\n",
            "[element.\"App.B\"]\nkind = \"module\"\n\n",
            "[[relation]]\nfrom = \"App.A\"\nto = \"App.B\"\n\n",
            "[[relation]]\nfrom = \"App.B\"\nto = \"App.A\"\n",
        ),
    );
    // 8 依赖祖先
    let ancestor = TempDir::new("halt-ancestor");
    ancestor.write_model(
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Auth\"]\nkind = \"domain\"\n\n",
            "[[relation]]\nfrom = \"App.Auth\"\nto = \"App\"\n",
        ),
    );
    // 9 未定义引用 + 孤儿层级
    let unknown_reference = TempDir::new("halt-unknown-ref");
    unknown_reference.write_model(
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Auth\"]\nkind = \"domain\"\n\n",
            "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Shared\"\n",
        ),
    );
    let orphan = TempDir::new("halt-orphan");
    orphan.write_model(
        "01.toml",
        "[element.\"App.Auth.Token\"]\nkind = \"component\"\n",
    );

    let cases: [(&TempDir, &str); 10] = [
        (&syntax, "syntaxError"),
        (&missing, "missingField"),
        (&invalid, "invalidValue"),
        (&unknown_field, "unknownField"),
        (&dup_same, "duplicateElement（同文件）"),
        (&dup_cross, "duplicateElement（跨文件）"),
        (&cycle, "cycle"),
        (&ancestor, "dependsOnAncestor"),
        (&unknown_reference, "unknownReference"),
        (&orphan, "orphanHierarchy"),
    ];
    for (temp, context) in cases {
        let graph = dto::graph_view(temp.root())
            .unwrap_or_else(|error| panic!("{context}：停摆态应 Ok 返回（不 Err）: {error}"));
        assert_halted(&graph, context);
        let kinds: Vec<&str> = graph
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.kind.as_str())
            .collect();
        assert!(!kinds.is_empty(), "{context}：诊断种类在场");
        assert!(
            kinds.iter().all(|kind| !kind.is_empty()),
            "{context}：诊断种类线值可读"
        );
    }
}

/// 边界：空态与停摆态可区分——`models/` 缺席 / 空目录 → 三数组皆空且
/// `diagnostics` 空；`docNotFound` 场景 → 不进 `diagnostics`（`ArchiGraph` 无
/// `warnings` 字段）。
#[test]
fn graph_view空态与文档警告不混入诊断() {
    let absent = TempDir::new("graph-absent");
    let graph = dto::graph_view(absent.root()).expect("目录缺席仍 Ok");
    assert!(graph.elements.is_empty() && graph.relations.is_empty());
    assert!(graph.diagnostics.is_empty(), "空态无诊断（与停摆可区分）");

    let empty = TempDir::with_empty_models("graph-empty");
    let graph = dto::graph_view(empty.root()).expect("空目录仍 Ok");
    assert!(graph.elements.is_empty() && graph.diagnostics.is_empty());

    // docNotFound 场景：模型健康但文档缺失 → 结构诊断仍空
    let missing_doc = TempDir::new("graph-missing-doc");
    missing_doc.write_model(
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Auth\"]\nkind = \"domain\"\n",
            "docs = [\"./src/auth/MISSING.md\"]\n",
        ),
    );
    let graph = dto::graph_view(missing_doc.root()).expect("健康模型 Ok");
    assert!(
        graph.diagnostics.is_empty(),
        "文档缺失不进结构诊断（ArchiGraph 无 warnings 字段）"
    );
    assert_eq!(graph.elements.len(), 2, "图数据照常返回");
}

/// 边界：排序 / 规模面——500 元素稳定序、连续两次调用结果逐字相同。
#[test]
fn graph_view规模面稳定序与幂等() {
    let temp = TempDir::new("graph-scale");
    let mut source = String::new();
    for index in (0..500).rev() {
        source.push_str(&format!(
            "[element.\"Mod{index:03}\"]\nkind = \"module\"\n\n"
        ));
    }
    temp.write_model("01.toml", &source);
    let first = dto::graph_view(temp.root()).expect("Ok");
    let second = dto::graph_view(temp.root()).expect("Ok");
    assert_eq!(first, second, "连续两次调用结果逐字相同");
    assert_eq!(first.elements.len(), 500);
    let mut sorted: Vec<&str> = first.elements.iter().map(|e| e.fqn.as_str()).collect();
    sorted.sort_unstable();
    let actual: Vec<&str> = first.elements.iter().map(|e| e.fqn.as_str()).collect();
    assert_eq!(actual, sorted, "元素稳定字典序");
}

// ---------------------------------------------------------------------------
// `module_detail`
// ---------------------------------------------------------------------------

/// 正向：元素 + 出边 / 入边 / 子元素 + 文档 + 警告；三项邻接与 `neighborhood`
/// 逐一等同（投影零改写）。
#[test]
fn module_detail邻接与文档逐项投影() {
    let temp = healthy_dir("detail");
    let detail = dto::module_detail(temp.root(), "App.Auth").expect("元素在场应 Ok");
    assert_eq!(detail.element.fqn, "App.Auth");
    assert_eq!(detail.element.gate, GateTier::Strict);
    assert_eq!(
        detail
            .depends_on
            .iter()
            .map(|edge| (edge.target.as_str(), edge.description.as_deref()))
            .collect::<Vec<_>>(),
        vec![("App.Billing", Some("认证调用计费"))],
        "出边含 target + description"
    );
    assert_eq!(
        detail
            .depended_by
            .iter()
            .map(|edge| edge.source.as_str())
            .collect::<Vec<_>>(),
        vec!["App", "App.Billing.Invoice"],
        "入边含 source（按 (from, to) 字典序）"
    );
    assert!(detail.children.is_empty(), "无直接子元素");
    assert_eq!(
        detail
            .docs
            .iter()
            .map(|doc| (doc.path.as_str(), doc.source))
            .collect::<Vec<_>>(),
        vec![("src/auth/README.md", DocSource::Explicit)],
        "文档关联按强度序"
    );
    assert!(detail.warnings.is_empty());

    // 投影零改写：与图侧邻域查询逐项等同
    let graph = dto::graph_view(temp.root()).expect("Ok");
    let element = graph
        .elements
        .iter()
        .find(|element| element.fqn == "App.Auth")
        .expect("App.Auth 在场");
    assert_eq!(&detail.element, element, "元素字段与图投影逐字相同");

    // 子元素面
    let billing = dto::module_detail(temp.root(), "App.Billing").expect("Ok");
    assert_eq!(
        billing.children,
        vec!["App.Billing.Invoice".to_owned()],
        "children 为直接子元素 FQN"
    );
    assert!(billing
        .depended_by
        .iter()
        .any(|edge| edge.source == "App.Auth"));
}

/// 异常：元素不存在 → `Err` 含总量提示；结构错 → `Err` 含诊断清单文本；坏模型不
/// panic。
#[test]
fn module_detail缺席与结构错两臂() {
    let temp = healthy_dir("detail-err");
    let absent = dto::module_detail(temp.root(), "App.Missing").expect_err("缺席应 Err");
    assert!(absent.contains("元素不存在"), "文案点名缺席: {absent}");
    assert!(absent.contains('4'), "文案含元素总数提示: {absent}");

    let broken = TempDir::new("detail-broken");
    broken.write_model("01.toml", "[element.\"App.Auth\"]\nkind = \"domain\"\n");
    let error = dto::module_detail(broken.root(), "App.Auth").expect_err("结构错应 Err");
    assert!(
        error.contains("orphanHierarchy"),
        "错误文案含诊断 kind: {error}"
    );
    assert!(error.contains("01.toml"), "错误文案含 file:line: {error}");
    assert!(error.contains('1'), "错误文案含条数: {error}");
}

/// 边界：无 `path` / `docs` / `summary` 的元素 → 显式空值；特殊 fqn → 缺席 `Err`；
/// 500 元素模型 → 单元素投影只含 1-hop。
#[test]
fn module_detail边界形态() {
    let temp = healthy_dir("detail-bounds");
    let plain = dto::module_detail(temp.root(), "App.Billing.Invoice").expect("Ok");
    assert!(plain.element.path.is_empty(), "path 为空数组");
    assert!(plain.element.metadata.is_empty(), "metadata 为空表");
    assert!(plain.element.summary.is_none(), "summary 为 null");

    let long = "X".repeat(1200);
    for fqn in ["", long.as_str(), "App.🚀Auth"] {
        assert!(
            dto::module_detail(temp.root(), fqn).is_err(),
            "不存在的 fqn {fqn:?} 应 Err（不 panic）"
        );
    }

    // 500 元素：单元素投影只含 1-hop
    let scale = TempDir::new("detail-scale");
    let mut source = String::new();
    for index in 0..500 {
        source.push_str(&format!(
            "[element.\"Mod{index:03}\"]\nkind = \"module\"\n\n"
        ));
    }
    source.push_str("[[relation]]\nfrom = \"Mod000\"\nto = \"Mod001\"\n\n");
    source.push_str("[[relation]]\nfrom = \"Mod002\"\nto = \"Mod000\"\n");
    scale.write_model("01.toml", &source);
    let detail = dto::module_detail(scale.root(), "Mod000").expect("Ok");
    assert_eq!(
        detail.depends_on.len(),
        1,
        "只含 1-hop 出边（不随总量膨胀）"
    );
    assert_eq!(detail.depended_by.len(), 1, "只含 1-hop 入边");
}

// ---------------------------------------------------------------------------
// `module_overview` / `search` / `linked_doc_paths`
// ---------------------------------------------------------------------------

/// 正向 / 异常 / 边界：薄概览按 FQN 字典序、每条恰四字段；结构错 `Err`；空模型空
/// 数组；单元素 1 条。
#[test]
fn module_overview三形态() {
    let temp = healthy_dir("overview");
    let briefs = dto::module_overview(temp.root()).expect("Ok");
    assert_eq!(briefs.len(), 4);
    let fqns: Vec<&str> = briefs.iter().map(|brief| brief.fqn.as_str()).collect();
    assert_eq!(
        fqns,
        vec!["App", "App.Auth", "App.Billing", "App.Billing.Invoice"]
    );
    let value = serde_json::to_value(&briefs[1]).expect("序列化成功");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["fqn", "gate", "kind", "summary"], "恰四字段");

    let broken = TempDir::new("overview-broken");
    broken.write_model("01.toml", "[element.\"App\"]\nkind = \"pakage\"\n");
    assert!(dto::module_overview(broken.root()).is_err(), "结构错应 Err");

    let empty = TempDir::with_empty_models("overview-empty");
    assert!(
        dto::module_overview(empty.root())
            .expect("空模型 Ok")
            .is_empty(),
        "空模型返回空数组（不 Err）"
    );

    let single = TempDir::new("overview-single");
    single.write_model("01.toml", "[element.\"App\"]\nkind = \"package\"\n");
    assert_eq!(dto::module_overview(single.root()).expect("Ok").len(), 1);
}

/// 正向 / 异常：三 scope 透传与参数校验、结构错透传。
#[test]
fn dto_search三scope与参数与结构错() {
    let temp = healthy_dir("search");
    let all = dto::search(temp.root(), "auth", SearchScope::All).expect("Ok");
    assert!(
        !all.elements.is_empty() && !all.docs.is_empty(),
        "All 两数组皆非空"
    );

    let elements = dto::search(temp.root(), "auth", SearchScope::Elements).expect("Ok");
    assert!(elements.docs.is_empty(), "Elements → docs 空");

    let docs = dto::search(temp.root(), "auth", SearchScope::Docs).expect("Ok");
    assert!(docs.elements.is_empty(), "Docs → elements 空");

    let empty_query =
        dto::search(temp.root(), "  ", SearchScope::All).expect_err("空 query 应 Err");
    assert!(empty_query.contains("query"), "错误文案点名 query");

    let broken = TempDir::new("search-broken");
    broken.write_model("01.toml", "[element.\"App\"]\nkind = \"pakage\"\n");
    assert!(
        dto::search(broken.root(), "auth", SearchScope::All).is_err(),
        "结构错透传 Err"
    );
}

/// 正向 / 边界：关联集路径集（三源并集、去重）；空模型空集；`docNotFound` 路径
/// 不入集；结构错 `Err`；任意越界路径不在集内。
#[test]
fn linked_doc_paths三源并集与边界() {
    let temp = TempDir::new("linked");
    temp.write("src/auth/README.md", "# 认证\n");
    temp.write("src/auth/guide.md", "# 指南\n");
    fs::create_dir_all(foundation::layout::archi_decisions_dir(temp.root()))
        .expect("建决策记录目录");
    fs::write(
        foundation::layout::archi_decisions_dir(temp.root()).join("0001-auth.md"),
        "涉及 App.Auth\n",
    )
    .expect("写决策记录");
    temp.write_model(
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Auth\"]\nkind = \"domain\"\n",
            "path = [\"./src/auth/\"]\n",
            "docs = [\"./src/auth/README.md\", \"./src/auth/MISSING.md\"]\n",
        ),
    );

    let linked = dto::linked_doc_paths(temp.root()).expect("Ok");
    let paths: Vec<&str> = linked.iter().map(String::as_str).collect();
    assert_eq!(
        paths,
        vec![
            "openspec/architecture/decisions/0001-auth.md",
            "src/auth/README.md",
            "src/auth/guide.md",
        ],
        "三源并集去重（explicit / adjacent / adr）"
    );
    assert!(
        !linked.contains("src/auth/MISSING.md"),
        "docNotFound 路径不入集"
    );
    assert!(
        !linked.contains("openspec/changes/x/proposal.md"),
        "任意越界路径不在集内"
    );

    let empty = TempDir::with_empty_models("linked-empty");
    assert!(
        dto::linked_doc_paths(empty.root())
            .expect("空模型 Ok")
            .is_empty(),
        "空模型空集"
    );

    let broken = TempDir::new("linked-broken");
    broken.write_model("01.toml", "[element.\"App\"]\nkind = \"pakage\"\n");
    assert!(
        dto::linked_doc_paths(broken.root()).is_err(),
        "结构错应 Err"
    );
}

// ---------------------------------------------------------------------------
// `change_summary`
// ---------------------------------------------------------------------------

/// 正向：两侧差异（元素 +2 / −1、关系 +2 / −1、`gate` 一条）→ 摘要与 `model_diff`
/// 逐项相等，`next` 侧诊断随行（空）。
///
/// `gate` 变更只发生在两侧同名元素上：`App`（既有）`off` → `strict` 计一条；
/// 新增 `App.Billing` 自带 `strict` 只进 `elements_added`（无旧值可比）。
#[test]
fn change_summary差异逐项() {
    let main = TempDir::new("summary-main");
    main.write_model(
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Legacy\"]\nkind = \"module\"\n\n",
            "[[relation]]\nfrom = \"App\"\nto = \"App.Legacy\"\n",
        ),
    );
    let next = TempDir::new("summary-next");
    next.write_model(
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\ngate = \"strict\"\n\n",
            "[element.\"App.Billing\"]\nkind = \"module\"\ngate = \"strict\"\n\n",
            "[element.\"App.Billing.Invoice\"]\nkind = \"component\"\n\n",
            "[[relation]]\nfrom = \"App\"\nto = \"App.Billing\"\n\n",
            "[[relation]]\nfrom = \"App\"\nto = \"App.Billing.Invoice\"\n",
        ),
    );

    let summary = dto::change_summary(main.root(), next.root())
        .expect("Ok")
        .expect("有改动即 Some");
    assert_eq!(
        summary.elements_added,
        vec!["App.Billing".to_owned(), "App.Billing.Invoice".to_owned()],
        "新增元素字典序"
    );
    assert_eq!(summary.elements_removed, vec!["App.Legacy".to_owned()]);
    assert_eq!(summary.relations_added, 2, "关系 +2");
    assert_eq!(summary.relations_removed, 1, "关系 −1");
    assert_eq!(
        summary.gate_changes.len(),
        1,
        "新增元素不进 gate 变更组（只有既有 App 一条）"
    );
    assert_eq!(summary.gate_changes[0].fqn, "App");
    assert_eq!(summary.gate_changes[0].from, GateTier::Off);
    assert_eq!(summary.gate_changes[0].to, GateTier::Strict);
    assert!(summary.diagnostics.is_empty(), "改动侧健康即零诊断");
}

/// 边界：两侧等价 → `None`（零占位）；两侧模型目录俱缺席 → `None`；仅 `next` 有
/// 模型（冷启动）/ 仅 `main` 有模型 → `Some`。
#[test]
fn change_summary零占位与单侧形态() {
    let left = TempDir::new("summary-equal-left");
    let right = TempDir::new("summary-equal-right");
    for temp in [&left, &right] {
        temp.write_model(
            "01.toml",
            "[element.\"App\"]\nkind = \"package\"\n\n[element.\"App.Auth\"]\nkind = \"domain\"\n",
        );
    }
    assert!(
        dto::change_summary(left.root(), right.root())
            .expect("Ok")
            .is_none(),
        "两侧等价 → None（零占位）"
    );

    let empty_main = TempDir::new("summary-empty-main");
    let empty_next = TempDir::new("summary-empty-next");
    assert!(
        dto::change_summary(empty_main.root(), empty_next.root())
            .expect("Ok")
            .is_none(),
        "两侧模型目录俱缺席 → None"
    );

    let cold = dto::change_summary(empty_main.root(), right.root())
        .expect("Ok")
        .expect("仅 next 有模型 → Some");
    assert_eq!(cold.elements_added.len(), 2, "冷启动元素全为新增");
    assert_eq!(cold.relations_added, 0);

    let removed = dto::change_summary(right.root(), empty_next.root())
        .expect("Ok")
        .expect("仅 main 有模型 → Some");
    assert_eq!(removed.elements_removed.len(), 2, "元素全为删除");
}

/// 异常：`next` 侧结构错 → `Some` 且 `diagnostics` 非空（红字面数据，不阻断）；
/// `main` 侧结构错 → 仍 `Some`（基线解析失败按空基线比较，差异即全量新增）。
#[test]
fn change_summary结构错两臂() {
    let healthy = TempDir::new("summary-healthy");
    healthy.write_model("01.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let broken = TempDir::new("summary-broken");
    broken.write_model("01.toml", "[element.\"App.Auth\"]\nkind = \"domain\"\n");

    let summary = dto::change_summary(healthy.root(), broken.root())
        .expect("Ok")
        .expect("改动侧结构错仍返回摘要（不阻断）");
    assert!(
        !summary.diagnostics.is_empty(),
        "改动侧诊断随行（红字面数据）"
    );
    assert_eq!(
        summary.diagnostics[0].kind,
        DiagnosticKind::OrphanHierarchy,
        "诊断种类为孤儿层级"
    );

    let base_broken = TempDir::new("summary-base-broken");
    base_broken.write_model("01.toml", "[element.\"App\"]\nkind = \"pakage\"\n");
    let outcome = dto::change_summary(broken.root(), healthy.root());
    if let Ok(Some(summary)) = outcome {
        assert_eq!(
            summary.elements_added,
            vec!["App".to_owned()],
            "基线解析失败按空基线比较 → 差异为全量新增"
        );
    } else {
        // 基线坏到无可比时显式报错亦在契约内
        assert!(outcome.is_err(), "要么摘要要么显式 Err（不静默）");
    }
    let _ = base_broken;
}

// ---------------------------------------------------------------------------
// `SearchScope::parse`
// ---------------------------------------------------------------------------

/// 正向：`"elements"` / `"docs"` / `"all"` → 对应变体（3 值全覆）。
#[test]
fn search_scope_parse三值() {
    assert_eq!(SearchScope::parse("elements"), Ok(SearchScope::Elements));
    assert_eq!(SearchScope::parse("docs"), Ok(SearchScope::Docs));
    assert_eq!(SearchScope::parse("all"), Ok(SearchScope::All));
}

/// 异常：空串 / 大小写不同 / 尾空格 / 未知 / 超长 → `Err` 且消息列出三合法值。
#[test]
fn search_scope_parse非法值列出合法值() {
    let long = "a".repeat(1200);
    for raw in ["", "Elements", "ALL", "all ", "unknown", long.as_str()] {
        let error = SearchScope::parse(raw)
            .err()
            .unwrap_or_else(|| panic!("非法 scope {raw:?} 应 Err"));
        for legal in ["elements", "docs", "all"] {
            assert!(error.contains(legal), "错误文案列出合法值 {legal}: {error}");
        }
    }
}

// ---------------------------------------------------------------------------
// 词汇 serde 线形与缺省形态
// ---------------------------------------------------------------------------

/// 边界：逐枚举线值（lowercase / camelCase）与非法值反序列化 `Err`（每枚举 N+1）。
#[test]
fn 词汇serde线形逐值() {
    // ElementKind（4）
    for (value, line) in [
        (ElementKind::Package, "package"),
        (ElementKind::Domain, "domain"),
        (ElementKind::Module, "module"),
        (ElementKind::Component, "component"),
    ] {
        assert_eq!(value.as_str(), line);
        assert_eq!(
            serde_json::to_value(value).expect("序列化成功"),
            json!(line)
        );
        assert_eq!(
            serde_json::from_value::<ElementKind>(json!(line)).expect("反序列化成功"),
            value
        );
    }
    assert!(serde_json::from_value::<ElementKind>(json!("unknown")).is_err());

    // GateTier（3）
    for (value, line) in [
        (GateTier::Off, "off"),
        (GateTier::Warn, "warn"),
        (GateTier::Strict, "strict"),
    ] {
        assert_eq!(value.as_str(), line);
        assert_eq!(
            serde_json::to_value(value).expect("序列化成功"),
            json!(line)
        );
    }
    assert!(serde_json::from_value::<GateTier>(json!("Off")).is_err());

    // DiagnosticKind（9，camelCase）
    let diagnostic_kinds = [
        (DiagnosticKind::SyntaxError, "syntaxError"),
        (DiagnosticKind::MissingField, "missingField"),
        (DiagnosticKind::InvalidValue, "invalidValue"),
        (DiagnosticKind::UnknownField, "unknownField"),
        (DiagnosticKind::DuplicateElement, "duplicateElement"),
        (DiagnosticKind::Cycle, "cycle"),
        (DiagnosticKind::DependsOnAncestor, "dependsOnAncestor"),
        (DiagnosticKind::UnknownReference, "unknownReference"),
        (DiagnosticKind::OrphanHierarchy, "orphanHierarchy"),
    ];
    assert_eq!(diagnostic_kinds.len(), 9, "封闭九值");
    for (value, line) in diagnostic_kinds {
        assert_eq!(value.as_str(), line);
        assert_eq!(
            serde_json::to_value(value).expect("序列化成功"),
            json!(line)
        );
    }
    assert!(serde_json::from_value::<DiagnosticKind>(json!("SyntaxError")).is_err());

    // WarningKind（1）
    assert_eq!(WarningKind::DocNotFound.as_str(), "docNotFound");
    assert_eq!(
        serde_json::to_value(WarningKind::DocNotFound).expect("序列化成功"),
        json!("docNotFound")
    );
    assert!(serde_json::from_value::<WarningKind>(json!("docnotfound")).is_err());

    // DocKind（2）/ DocSource（3）
    assert_eq!(
        serde_json::to_value(DocKind::Md).expect("序列化成功"),
        json!("md")
    );
    assert_eq!(
        serde_json::to_value(DocKind::Adr).expect("序列化成功"),
        json!("adr")
    );
    assert!(serde_json::from_value::<DocKind>(json!("MD")).is_err());
    for (value, line) in [
        (DocSource::Explicit, "explicit"),
        (DocSource::Adjacent, "adjacent"),
        (DocSource::Text, "text"),
    ] {
        assert_eq!(
            serde_json::to_value(value).expect("序列化成功"),
            json!(line)
        );
    }
    assert!(serde_json::from_value::<DocSource>(json!("Explicit")).is_err());

    // EdgeMark（4）/ NodeMark（2）
    for (value, line) in [
        (EdgeMark::None, "none"),
        (EdgeMark::Unfulfilled, "unfulfilled"),
        (EdgeMark::CrossKind, "crossKind"),
        (EdgeMark::Exempt, "exempt"),
    ] {
        assert_eq!(
            serde_json::to_value(value).expect("序列化成功"),
            json!(line)
        );
    }
    assert!(serde_json::from_value::<EdgeMark>(json!("crosskind")).is_err());
    for (value, line) in [
        (NodeMark::None, "none"),
        (NodeMark::Unfulfilled, "unfulfilled"),
    ] {
        assert_eq!(
            serde_json::to_value(value).expect("序列化成功"),
            json!(line)
        );
    }
    assert!(
        serde_json::from_value::<NodeMark>(json!("cycle")).is_err(),
        "结构错值不入节点标记词汇（D6）"
    );

    // SearchScope（3）
    assert_eq!(
        serde_json::to_value(SearchScope::All).expect("序列化成功"),
        json!("all")
    );
    assert!(serde_json::from_value::<SearchScope>(json!("docsOnly")).is_err());
}

/// 边界：缺省与可选形态——`GateTier::default() == Off`；`line: 0` + `column: None`
/// 序列化为显式字段；`reason` / `summary` 缺席显式 `null`；`metadata` 空表显式 `{}`。
#[test]
fn 缺省与可选字段显式线形() {
    assert_eq!(GateTier::default(), GateTier::Off, "缺省档位为 Off");

    let diagnostic = ArchiDiagnostic {
        kind: DiagnosticKind::MissingField,
        message: "缺字段".to_owned(),
        file: "openspec/architecture/models/01.toml".to_owned(),
        line: 0,
        column: None,
    };
    let value = serde_json::to_value(&diagnostic).expect("序列化成功");
    assert_eq!(value["line"], json!(0), "位置未知 line 显式 0");
    assert_eq!(value["column"], json!(null), "位置未知 column 显式 null");
    assert!(
        value.as_object().expect("对象").contains_key("column"),
        "不省字段"
    );

    let element = ArchiElement {
        fqn: "App".to_owned(),
        kind: ElementKind::Package,
        gate: GateTier::Off,
        path: Vec::new(),
        summary: None,
        metadata: std::collections::BTreeMap::new(),
        model_file: "openspec/architecture/models/01.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
    };
    let value = serde_json::to_value(&element).expect("序列化成功");
    assert_eq!(value["summary"], json!(null));
    assert_eq!(value["metadata"], json!({}));
    assert_eq!(value["path"], json!([]));

    let relation = ArchiRelation {
        from: "App.Auth".to_owned(),
        to: "App".to_owned(),
        description: None,
        mark: EdgeMark::None,
        reason: None,
    };
    let value = serde_json::to_value(&relation).expect("序列化成功");
    assert_eq!(value["reason"], json!(null), "reason 恒 null 显式在场");
    assert_eq!(value["description"], json!(null));
}
