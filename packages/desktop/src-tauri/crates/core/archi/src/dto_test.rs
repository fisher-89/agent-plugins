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
    self, ApiStatus, ArchiApiCheck, ArchiApiMeta, ArchiApiWriteOutcome, ArchiDiagnostic,
    ArchiElement, ArchiFileEntry, ArchiFileTree, ArchiGraph, ArchiRelation, DiagnosticKind,
    DocKind, DocSource, EdgeMark, ElementKind, FileEntryKind, FileTreeMark, GateTier, NodeMark,
    SearchScope, WarningKind,
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
        auth.model_file, ".dev-team/architecture/models/01-core.toml",
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

/// 正向 / 异常 / 边界：薄概览按 FQN 字典序、每条为既有四字段 + `api`（同一模型字段的
/// 纯拷贝）；结构错 `Err`；空模型空数组；单元素 1 条。
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
    assert_eq!(
        keys,
        vec!["api", "fqn", "gate", "kind", "summary"],
        "既有四字段 + api（末尾追加，键集恰五）"
    );

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
            ".dev-team/architecture/decisions/0001-auth.md",
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
        file: ".dev-team/architecture/models/01.toml".to_owned(),
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
        api: ApiStatus::Declared,
        path: Vec::new(),
        summary: None,
        metadata: std::collections::BTreeMap::new(),
        model_file: ".dev-team/architecture/models/01.toml".to_owned(),
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

// ---------------------------------------------------------------------------
// 结构诊断全量常量 `DiagnosticKind::ALL`（知识文本 / 语料对拍的唯一派生源）
//
// 口径：`ALL` 是知识文本与语料对拍的单点派生源——序即 `as_str` 分支序，长度即
// 红灯（新增变体忘补 `ALL` 时本组断言与编译面双红）。词汇文本一侧的「逐值在场」
// 断言住 `crates/infra/agent/src/archi_knowledge_test.rs`（`archi` 不得反向依赖
// `agent-runtime`，见 test-design「不可测试项」）。
// ---------------------------------------------------------------------------

/// 结构诊断九词的期望线格式（`as_str` 分支序 = `ALL` 声明序，D6）。
const DIAGNOSTIC_LINES: [&str; 9] = [
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

/// 正向：`ALL` 九值逐项 `as_str()` 依次为线格式 camelCase 词（序即分支序）。
#[test]
fn 诊断all与as_str逐值对拍() {
    for (index, kind) in DiagnosticKind::ALL.iter().enumerate() {
        assert_eq!(
            kind.as_str(),
            DIAGNOSTIC_LINES[index],
            "ALL[{index}] 的线格式词"
        );
    }
}

/// 边界：`ALL` 长度锚——九值（`[Self; 9]` 长度不符即编译期红；加变体忘补
/// `ALL` 时本断言与编译面双红）。
#[test]
fn 诊断all长度锚九值() {
    assert_eq!(
        DiagnosticKind::ALL.len(),
        9,
        "结构诊断封闭九值（parser 免费五 + 自实现四）"
    );
    assert_eq!(
        DiagnosticKind::ALL.len(),
        DIAGNOSTIC_LINES.len(),
        "常量与线格式清单等长（漏值不可绕过）"
    );
}

/// 异常：`ALL` 无重复 / 全覆盖——去重后长度仍为 9；以「逐变体构造的九项清单」
/// 与 `ALL` 做集合相等比较（缺失值点名报出）。
#[test]
fn 诊断all无重复且全覆盖() {
    let from_all: std::collections::BTreeSet<&str> = DiagnosticKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    assert_eq!(from_all.len(), 9, "ALL 去重后仍为九项（无重复值）");

    let explicit = [
        DiagnosticKind::SyntaxError,
        DiagnosticKind::MissingField,
        DiagnosticKind::InvalidValue,
        DiagnosticKind::UnknownField,
        DiagnosticKind::DuplicateElement,
        DiagnosticKind::Cycle,
        DiagnosticKind::DependsOnAncestor,
        DiagnosticKind::UnknownReference,
        DiagnosticKind::OrphanHierarchy,
    ];
    let from_explicit: std::collections::BTreeSet<&str> =
        explicit.iter().map(|kind| kind.as_str()).collect();
    let missing: Vec<&str> = from_explicit.difference(&from_all).copied().collect();
    assert!(
        missing.is_empty(),
        "ALL 缺变体（点名）：{missing:?}（新增变体必须同补 ALL）"
    );
    let extra: Vec<&str> = from_all.difference(&from_explicit).copied().collect();
    assert!(extra.is_empty(), "ALL 含未知词：{extra:?}");
    assert_eq!(
        from_all, from_explicit,
        "ALL 与逐变体清单集合相等（漏值 / 替换值不可绕过）"
    );
}

/// 边界：`ALL` 序与 `as_str` 同源——逐位断言（打乱 `ALL` 顺序即红——`build()`
/// 派生段的输出序依赖此序）。
#[test]
fn 诊断all序与as_str同源逐位() {
    for (index, kind) in DiagnosticKind::ALL.iter().enumerate() {
        assert_eq!(
            kind.as_str(),
            DIAGNOSTIC_LINES[index],
            "第 {index} 位的序与 as_str 分支序一致"
        );
    }
    // 相邻位互异（序被打乱 / 值重复均可观察）
    for window in DiagnosticKind::ALL.windows(2) {
        assert_ne!(
            window[0].as_str(),
            window[1].as_str(),
            "相邻两位不得同词（序即分支序）"
        );
    }
}

/// 回归：既有全量常量复用面——`ElementKind::ALL` 四值 / `GateTier::ALL` 三值逐值
/// `as_str`；`GateTier::default() == Off`（知识文本 `gate` 段与缺省锚依赖此面，
/// 既有断言不改）。
#[test]
fn 既有全量常量复用面零回归() {
    assert_eq!(ElementKind::ALL.len(), 4, "元素 kind 封闭四值");
    for (index, word) in ["package", "domain", "module", "component"]
        .iter()
        .enumerate()
    {
        assert_eq!(
            ElementKind::ALL[index].as_str(),
            *word,
            "ElementKind::ALL[{index}] 逐值"
        );
    }
    assert_eq!(GateTier::ALL.len(), 3, "gate 封闭三值");
    for (index, word) in ["off", "warn", "strict"].iter().enumerate() {
        assert_eq!(
            GateTier::ALL[index].as_str(),
            *word,
            "GateTier::ALL[{index}] 逐值"
        );
    }
    assert_eq!(
        GateTier::default(),
        GateTier::Off,
        "缺省档位恒 Off（新元素缺省不收紧）"
    );
    assert_eq!(
        GateTier::default().as_str(),
        "off",
        "缺省档线词（知识文本 `gate` 缺省锚的派生源）"
    );
}

// ===========================================================================
// 契约状态投影（`module-api-contract-format` / AC-10 / D22）
//
// 两层口径：模型字段 `element.api`（声明意图）与 `ArchiApiRef.status`（文件在案
// 事实）并列；投影只报文件事实、MUST NOT 回写模型字段。
// ===========================================================================

use crate::api::paths;

/// 四字段契约引用（`Declared` 形态，三路径全 `Some`）的构造器。
fn declared_ref(root: &Path, fqn: &str) -> serde_json::Value {
    let element = crate::dto::ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Warn,
        api: ApiStatus::Declared,
        path: vec!["src/mod/".to_owned()],
        summary: None,
        metadata: std::collections::BTreeMap::new(),
        model_file: ".dev-team/architecture/models/01.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
    };
    let reference = crate::api::api_ref_of(root, fqn, &element.path);
    serde_json::to_value(&reference).expect("序列化成功")
}

/// 正向（AC-10 / D22）：`ArchiApiRef` 四字段 camelCase 键名；`Declared` + 三路径
/// `Some` → 文本三键皆为 repo 相对 POSIX 串。
#[test]
fn archi_api_ref四字段线形() {
    let temp = TempDir::new("api-ref-line");
    let fqn = "App.Core.Mod";
    let schema_path = paths::schema_path(temp.root(), fqn).expect("合法 FQN");
    temp.write(
        &schema_path
            .strip_prefix(temp.root())
            .expect("相对")
            .to_string_lossy()
            .replace('\\', "/"),
        "{\"$defs\": {}}\n",
    );
    let contract_relative = paths::contract_path(temp.root(), fqn)
        .expect("合法 FQN")
        .strip_prefix(temp.root())
        .expect("相对")
        .to_string_lossy()
        .replace('\\', "/");
    temp.write(&contract_relative, &format!("# {fqn}\n"));

    let value = declared_ref(temp.root(), fqn);
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec!["status", "contractPath", "schemaPath", "generatedPath"],
        "四字段 camelCase 键名与顺序"
    );
    assert_eq!(value["status"], json!("declared"), "线词 declared");
    for key in ["contractPath", "schemaPath", "generatedPath"] {
        let text = value[key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} 应为字符串"));
        assert!(!text.starts_with("./"), "{key} 零前导 `./`：{text}");
        assert!(!text.contains('\\'), "{key} 为 POSIX 分隔：{text}");
        assert!(!text.is_empty(), "{key} 非空");
    }

    // 往返后四字段逐字段相等（源文件字段按序列化形）
    let back: crate::dto::ArchiApiRef =
        serde_json::from_value(value.clone()).expect("反序列化成功");
    assert_eq!(back.status, ApiStatus::Declared);
    assert!(back.contract_path.is_some() && back.schema_path.is_some());
}

/// 边界（AC-10 / D22）：`status = None` 时三路径全 `None`（序列化为 `null`），
/// 投影不产出「部分路径」组合。
#[test]
fn archi_api_ref为none时三路径全空() {
    let temp = TempDir::new("api-ref-none");
    let value = declared_ref(temp.root(), "App.Core.MissingMod");
    assert_eq!(value["status"], json!("none"), "md 缺席 → none");
    for key in ["contractPath", "schemaPath", "generatedPath"] {
        assert_eq!(value[key], json!(null), "{key} 显式 null");
        assert!(
            value.as_object().expect("对象").contains_key(key),
            "{key} 不被省略（四字段独立可空但 none 时三者全空）"
        );
    }

    let back: crate::dto::ArchiApiRef = serde_json::from_value(value).expect("反序列化成功");
    assert!(back.contract_path.is_none(), "往返后全 None");
    assert!(back.schema_path.is_none());
    assert!(back.generated_path.is_none());
}

/// 正向（AC-10 / D22）：`module_detail` 的 `api` 为**并列字段**（不在 `docs` 数组内），
/// 键集含 `element` / `dependsOn` / `dependedBy` / `children` / `docs` / `warnings` /
/// `api` 七项。
#[test]
fn module_detail并列api字段() {
    let temp = healthy_dir("detail-api-field");
    let detail = dto::module_detail(temp.root(), "App.Auth").expect("元素在场应 Ok");
    let value = serde_json::to_value(&detail).expect("序列化成功");
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec![
            "element",
            "dependsOn",
            "dependedBy",
            "children",
            "docs",
            "warnings",
            "api"
        ],
        "七字段键序（api 为末位并列字段）"
    );
    // `api` 不在 `docs` 数组内
    let docs = value["docs"].as_array().expect("docs 是数组");
    assert!(
        docs.iter()
            .all(|doc| doc.get("status").is_none() && doc.get("contractPath").is_none()),
        "契约投影 MUST NOT 进 docs 列表：{docs:?}"
    );
}

/// 正向 / 边界（AC-10 / D22）：`module_detail(root, fqn).api.status` 随 md 存在与否
/// 二值切换；容器元素（`path = []`）恒 `None` 且三路径全 `None`。
#[test]
fn module_detail投影随md存在性切换() {
    let temp = healthy_dir("detail-api-switch");
    let before = dto::module_detail(temp.root(), "App.Auth").expect("Ok");
    assert_eq!(before.api.status, ApiStatus::None, "无 md 时 status = none");

    // 落 md 后重投影 → declared
    let fqn = "App.Auth";
    let contract_relative = paths::contract_path(temp.root(), fqn)
        .expect("合法 FQN")
        .strip_prefix(temp.root())
        .expect("相对")
        .to_string_lossy()
        .replace('\\', "/");
    temp.write(
        &contract_relative,
        &format!("# {fqn}\n\n## 职能\n\n正文。\n"),
    );

    let after = dto::module_detail(temp.root(), fqn).expect("Ok");
    assert_eq!(after.api.status, ApiStatus::Declared, "md 在案 → declared");
    assert!(after.api.contract_path.is_some(), "契约路径在案");
    assert!(after.api.schema_path.is_some(), "schema 路径同源给出");

    // 容器元素（path 空）→ 恒 None
    let graph = dto::graph_view(temp.root()).expect("Ok");
    let container = graph
        .elements
        .iter()
        .find(|element| element.path.is_empty())
        .map(|element| element.fqn.clone());
    if let Some(fqn) = container {
        let detail = dto::module_detail(temp.root(), &fqn).expect("Ok");
        assert_eq!(detail.api.status, ApiStatus::None, "容器元素天然无契约");
        assert!(detail.api.contract_path.is_none());
        assert!(detail.api.generated_path.is_none());
    }
}

/// 边界（AC-10 / D22 零改写）：`api_ref` 报 `none` 时模型字段 `element.api` 仍为
/// `declared`（投影只报文件事实，MUST NOT 回写模型字段）。
#[test]
fn module_detail投影零回写() {
    let temp = healthy_dir("detail-api-no-writeback");
    let detail = dto::module_detail(temp.root(), "App.Auth").expect("Ok");
    assert_eq!(
        detail.element.api,
        ApiStatus::Declared,
        "模型字段（模型未写 api 键 → 缺省 declared）不被投影回写"
    );
    assert_eq!(
        detail.api.status,
        ApiStatus::None,
        "文件事实面（无 md）与之并列、互不覆写"
    );

    // 薄概览的 api 与模型字段逐值相等（纯拷贝）
    let briefs = dto::module_overview(temp.root()).expect("Ok");
    for brief in briefs {
        let graph = dto::graph_view(temp.root()).expect("Ok");
        let element = graph
            .elements
            .iter()
            .find(|element| element.fqn == brief.fqn)
            .expect("图里有该元素");
        assert_eq!(
            brief.api, element.api,
            "薄概览 api 为模型字段的纯拷贝（零推导，{}）",
            brief.fqn
        );
    }
}

/// 边界（AC-8）：`ArchiElement` 九字段 / `ArchiElementBrief` 四字段 /
/// `ArchiModuleDetail` 六字段为既有面，`api` 为末位追加；`ArchiGraph` /
/// `ArchiSearchResult` / `ArchiChangeSummary` 零新字段。
#[test]
fn 既有字段面零变化() {
    let temp = healthy_dir("field-faces");
    let graph = dto::graph_view(temp.root()).expect("Ok");
    let graph_value = serde_json::to_value(&graph).expect("序列化成功");
    let graph_keys: Vec<&str> = graph_value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        graph_keys,
        vec!["elements", "relations", "diagnostics"],
        "图三字段零新字段"
    );

    let element_value = serde_json::to_value(&graph.elements[0]).expect("序列化成功");
    let element_keys: Vec<&str> = element_value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        element_keys,
        vec![
            "fqn",
            "kind",
            "gate",
            "path",
            "summary",
            "metadata",
            "modelFile",
            "modelLine",
            "mark",
            "api"
        ],
        "元素十字段（api 为末位追加，既有九字段逐字与顺序不变）"
    );

    let search = dto::search(temp.root(), "Auth", SearchScope::All).expect("Ok");
    if let Some(hit) = search.elements.first() {
        let hit_value = serde_json::to_value(hit).expect("序列化成功");
        let keys: Vec<&str> = hit_value
            .as_object()
            .expect("对象")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            vec!["fqn", "kind", "gate", "api", "summary"],
            "薄条目既有四字段 + api（并列追加，顺序与 struct 字段一致）"
        );
    }
}

// ===========================================================================
// `ApiStatus` 二值封闭与 `ArchiApiRef` 字符串边界（`module-api-contract-format` /
// AC-10 / AC-8 / D22）
//
// `ApiStatus::ALL` 是知识文本与语料对拍的唯一派生源：长度即红灯（新增变体忘补
// `ALL` 时本组断言与编译面双红）。路径是字符串，投影与我们都不做归一。
// ===========================================================================

/// 正向（AC-10）：`ALL.len() == 2`；`Declared` → `"declared"`、`None` → `"none"`；
/// serde 线形与 `as_str` 逐值一致、往返恒等；`ALL` 去重后长度不变、序与 `as_str`
/// 分支序一致。
#[test]
fn api_status二值封闭() {
    assert_eq!(ApiStatus::ALL.len(), 2, "契约状态封闭二值");
    let expected = [(ApiStatus::Declared, "declared"), (ApiStatus::None, "none")];
    for (status, word) in expected {
        assert_eq!(status.as_str(), word, "{status:?} 的线词");
        assert_eq!(
            serde_json::to_value(status).expect("序列化成功"),
            json!(word),
            "{word} 的 serde 线形与 as_str 一致"
        );
        assert_eq!(
            serde_json::from_value::<ApiStatus>(json!(word)).expect("反序列化成功"),
            status,
            "{word} 往返逐值相等"
        );
    }

    let mut words: Vec<&str> = ApiStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect();
    let ordered = words.clone();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), 2, "ALL 无重复值");
    assert_eq!(
        ordered,
        expected
            .iter()
            .map(|(_, word)| *word)
            .collect::<Vec<&str>>(),
        "ALL 顺序与 as_str 分支序一致"
    );
}

/// 边界（AC-10）：`ApiStatus::default() == Declared`（模型未写该键即承诺有契约）；
/// 两值 `Eq` 可比较且互不相等。
#[test]
fn api_status缺省为declared() {
    assert_eq!(
        ApiStatus::default(),
        ApiStatus::Declared,
        "缺省 declared（承诺有契约）"
    );
    assert_ne!(ApiStatus::Declared, ApiStatus::None, "两值互不相等");
    assert_eq!(ApiStatus::ALL[0], ApiStatus::default(), "ALL 首项即缺省值");
}

/// 异常（Enum N+1 / AC-10）：词汇外五形态 → 反序列化全 `Err`（`rename_all =
/// "lowercase"` 严格，零宽松归一）。
#[test]
fn api_status词汇外反序列化即错() {
    for word in ["Declared", "NONE", "none ", "off", ""] {
        let result = serde_json::from_value::<ApiStatus>(json!(word));
        assert!(result.is_err(), "词汇外应 Err（严格封闭）：{word:?}");
    }
    for word in [json!(null), json!(0), json!(true), json!(["declared"])] {
        assert!(
            serde_json::from_value::<ApiStatus>(word.clone()).is_err(),
            "非字符串值应 Err：{word}"
        );
    }
}

/// 正向（AC-10）：`ArchiElement.api` 两态线形——文本含 `"api":"declared"` /
/// `"api":"none"`，往返逐字段相等，且 `api` 为末位追加（既有九字段名与顺序不变）。
#[test]
fn archi_element_api两态线形() {
    let base = ArchiElement {
        fqn: "App.Core.Mod".to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Warn,
        api: ApiStatus::Declared,
        path: vec!["./src/mod/".to_owned()],
        summary: Some("模块摘要".to_owned()),
        metadata: std::collections::BTreeMap::from([("owner".to_owned(), "core".to_owned())]),
        model_file: ".dev-team/architecture/models/01.toml".to_owned(),
        model_line: 7,
        mark: NodeMark::None,
    };
    for (status, word) in [(ApiStatus::Declared, "declared"), (ApiStatus::None, "none")] {
        let element = ArchiElement {
            api: status,
            ..base.clone()
        };
        let text = serde_json::to_string(&element).expect("序列化成功");
        assert!(
            text.contains(&format!("\"api\":\"{word}\"")),
            "线形含 api = {word}：{text}"
        );
        let back: ArchiElement = serde_json::from_str(&text).expect("反序列化成功");
        assert_eq!(back, element, "两态往返逐字段相等");

        let keys: Vec<&str> = text
            .split('"')
            .filter(|segment| {
                matches!(
                    *segment,
                    "fqn"
                        | "kind"
                        | "gate"
                        | "path"
                        | "summary"
                        | "metadata"
                        | "modelFile"
                        | "modelLine"
                        | "mark"
                        | "api"
                )
            })
            .collect();
        assert_eq!(keys.last(), Some(&"api"), "api 为末位追加字段：{text}");
        assert!(
            !element.metadata.contains_key("api"),
            "新增字段 MUST NOT 进 metadata（九字段面零变化）"
        );
    }
}

/// 边界（AC-8 / AC-10）：路径是字符串，不做归一——`>1000 chars` 的 repo 相对串与
/// 含 emoji / NUL 的 FQN 经 `ArchiApiRef` 往返逐字相等；`status = None` 时三者保持
/// `null`（`none` ⟹ 三路径全空，D22）。
#[test]
fn archi_api_ref超长与特殊字符路径() {
    let long_path = format!("{}/Infra/App.Core.Mod.schema.json", "层".repeat(1000));
    assert!(
        long_path.chars().count() > 1000,
        "构造超长路径（实际 {} 字符）",
        long_path.chars().count()
    );
    let reference = crate::dto::ArchiApiRef {
        status: ApiStatus::Declared,
        contract_path: Some(long_path.clone()),
        schema_path: Some(format!("{long_path}#/$defs/Mod")),
        generated_path: Some("src/mod.i.ts".to_owned()),
    };
    let back: crate::dto::ArchiApiRef =
        serde_json::from_str(&serde_json::to_string(&reference).expect("序列化成功"))
            .expect("反序列化成功");
    assert_eq!(back, reference, "超长路径逐字往返（不做归一 / 不截断）");
    assert_eq!(
        back.contract_path.as_deref(),
        Some(long_path.as_str()),
        "路径原文保留"
    );

    // 合法但形状极端的 FQN（emoji / NUL / 300 段）——线面逐字往返；尾点形态的空末段
    // 按路径形态判据拒收，另在下方单独断言
    let fqns = [
        "App.🎯.模块".to_owned(),
        "App.\u{0}.X".to_owned(),
        format!("{}Tail", "App.".repeat(300)),
    ];
    for fqn in &fqns {
        let fqn = fqn.as_str();
        let mut element = ArchiElement {
            fqn: fqn.to_owned(),
            kind: ElementKind::Module,
            gate: GateTier::Off,
            api: ApiStatus::Declared,
            path: vec!["src/x/".to_owned()],
            summary: None,
            metadata: std::collections::BTreeMap::new(),
            model_file: ".dev-team/architecture/models/01.toml".to_owned(),
            model_line: 1,
            mark: NodeMark::None,
        };
        let relative = crate::api::paths::contract_path(Path::new("/ws"), fqn)
            .expect("合法 FQN")
            .strip_prefix("/ws")
            .expect("相对路径")
            .to_string_lossy()
            .replace('\\', "/");
        let reference = crate::dto::ArchiApiRef {
            status: ApiStatus::Declared,
            contract_path: Some(relative),
            schema_path: None,
            generated_path: None,
        };
        let text = serde_json::to_string(&reference).expect("序列化成功");
        assert!(text.contains("\"schemaPath\":null"), "缺省路径显式 null");
        let back: crate::dto::ArchiApiRef = serde_json::from_str(&text).expect("反序列化成功");
        assert_eq!(back, reference, "特殊字符 FQN 的契约路径逐字往返");
        element.api = ApiStatus::None;
        let text = serde_json::to_string(&element).expect("序列化成功");
        let back: ArchiElement = serde_json::from_str(&text).expect("反序列化成功");
        assert_eq!(back.fqn, fqn, "emoji / NUL / 超长 FQN 逐字往返");
    }

    // 尾点 / 空段的 FQN 形态非法 ⇒ 路径派生 `None`（不 panic、`<Domain>` 不落空串）
    for fqn in ["App.".to_owned(), "App..X".to_owned(), "App.".repeat(300)] {
        assert_eq!(
            crate::api::paths::contract_path(Path::new("/ws"), &fqn),
            None,
            "形态非法的 FQN 不派生路径：{fqn:?}"
        );
        assert_eq!(
            crate::api::paths::schema_path(Path::new("/ws"), &fqn),
            None,
            "schema 路径同拒收：{fqn:?}"
        );
    }
}

/// 正向（AC-10 / AC-9）：`module_detail` 的 `api` 字段与直接调 `api::api_ref_of`
/// **逐字段相等**——`dto` 只做一次定位（元素 `path` 已在手），零拼装、零第二套投影；
/// 模型字段 `element.api` 在投影报 `none` 时仍保持 `declared`（投影零回写，D22）。
#[test]
fn module_detail的api与投影单点逐字段相等() {
    let temp = healthy_dir("detail-api-same-source");
    let fqn = "App.Auth";

    // 形态一：无 md → 两路都是 none 且三路径全 None
    let detail = dto::module_detail(temp.root(), fqn).expect("Ok");
    let direct = crate::api::api_ref_of(temp.root(), fqn, &detail.element.path);
    assert_eq!(detail.api, direct, "无 md 时两路逐字段相等");
    assert_eq!(detail.api.status, ApiStatus::None);
    assert_eq!(detail.element.api, ApiStatus::Declared, "模型字段零回写");

    // 形态二：md 在案 → 两路都是 declared 且三路径逐字相等
    let contract_relative = paths::contract_path(temp.root(), fqn)
        .expect("合法 FQN")
        .strip_prefix(temp.root())
        .expect("相对路径")
        .to_string_lossy()
        .replace('\\', "/");
    temp.write(
        &contract_relative,
        &format!("# {fqn}\n\n## 职能\n\n正文。\n"),
    );

    let detail = dto::module_detail(temp.root(), fqn).expect("Ok");
    let direct = crate::api::api_ref_of(temp.root(), fqn, &detail.element.path);
    assert_eq!(detail.api, direct, "md 在案时两路逐字段相等（投影纯单点）");
    assert_eq!(detail.api.status, ApiStatus::Declared);
    assert!(
        detail.api.generated_path.is_some(),
        "有 path 即有生成物路径"
    );

    // 源码自证：`dto.rs` 只调 `api_ref_of`（元素形态入口），零 `api_ref` 二次读模型
    let source = include_str!("dto.rs");
    assert_eq!(
        source.matches("api_ref_of").count(),
        1,
        "`dto.rs` 内 `api_ref_of` 恰一处（单点调用）"
    );
    assert!(
        !source.contains("crate::api::api_ref("),
        "`dto` MUST NOT 调 `api_ref`（已有元素 path，勿二次读模型）"
    );
    // 结构体定义行必然含 `ArchiApiRef`，故按「构造字面量」形态判定（定义行是 `pub struct`）
    let constructed = source
        .lines()
        .filter(|line| line.contains("ArchiApiRef {"))
        .filter(|line| !line.trim_start().starts_with("pub struct"))
        .count();
    assert!(
        !source.contains("paths::")
            && !source.contains("foundation::layout::archi_api_dir")
            && constructed == 0,
        "`dto` 内零契约路径拼装 / 零 `ArchiApiRef` 构造字面量（全由投影单点产出）"
    );
}

// ===========================================================================
// 文件树出线面（`FileEntryKind` / `FileTreeMark` / `ArchiFileEntry` /
// `ArchiFileTree`）：两值 / 五值封闭、线形、字段声明序、单字段扁平
// ===========================================================================

/// 全字段条目构造器（七字段按声明序）。
fn file_entry() -> ArchiFileEntry {
    ArchiFileEntry {
        path: "crates/core/archi/src/tree.rs".to_owned(),
        name: "tree.rs".to_owned(),
        kind: FileEntryKind::File,
        mark: FileTreeMark::Member,
        owners: vec!["App.Core.Archi".to_owned()],
        contains: 0,
        exists: true,
    }
}

/// 正向（AC-4 / spec Scenario ALL 与 as_str 逐值对拍）：`FileEntryKind` 二值封闭，
/// `as_str` / serde 线形 / 往返三者一致，`ALL` 去重后长度不变。
#[test]
fn 文件条目形态二值封闭与线形一致() {
    assert_eq!(FileEntryKind::ALL.len(), 2, "二值封闭（长度即红灯）");
    let expected = [(FileEntryKind::Dir, "dir"), (FileEntryKind::File, "file")];
    for (value, word) in expected {
        assert_eq!(value.as_str(), word, "{value:?} 的词");
        assert_eq!(
            serde_json::to_value(value).expect("序列化"),
            serde_json::Value::String(word.to_owned()),
            "{value:?} 的 serde 线形与 as_str 一致"
        );
        let back: FileEntryKind =
            serde_json::from_value(serde_json::Value::String(word.to_owned())).expect("反序列化");
        assert_eq!(back, value, "{value:?} 往返恒等");
    }
    let mut words: Vec<&str> = FileEntryKind::ALL
        .iter()
        .map(|value| value.as_str())
        .collect();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), 2, "ALL 内无重复词");
}

/// 正向（AC-4 / spec Scenario ALL 与 as_str 逐值对拍）：`FileTreeMark` 五值封闭，
/// 词形 camelCase，`ALL` 序与 `as_str` 分支序一致，往返恒等。
#[test]
fn 文件树标记五值封闭与线形一致() {
    assert_eq!(FileTreeMark::ALL.len(), 5, "五值封闭（长度即红灯）");
    let expected = [
        (FileTreeMark::ElementRoot, "elementRoot"),
        (FileTreeMark::Ambiguous, "ambiguous"),
        (FileTreeMark::Member, "member"),
        (FileTreeMark::ContainsElements, "containsElements"),
        (FileTreeMark::None, "none"),
    ];
    for (value, word) in expected {
        assert_eq!(value.as_str(), word, "{value:?} 的词");
        assert_eq!(
            serde_json::to_value(value).expect("序列化"),
            serde_json::Value::String(word.to_owned()),
            "{value:?} 的 serde 线形与 as_str 一致"
        );
        let back: FileTreeMark =
            serde_json::from_value(serde_json::Value::String(word.to_owned())).expect("反序列化");
        assert_eq!(back, value, "{value:?} 往返恒等");
    }
    let words: Vec<&str> = FileTreeMark::ALL
        .iter()
        .map(|value| value.as_str())
        .collect();
    let mut deduped = words.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(deduped.len(), 5, "ALL 内无重复词");
    assert_eq!(
        words,
        expected
            .iter()
            .map(|(_, word)| *word)
            .collect::<Vec<&str>>(),
        "ALL 序与 as_str 分支序一致"
    );
}

/// 异常（Enum N+1）：词汇外反序列化一律 `Err`（大小写 / 尾空格 / 空串 / 邻词均
/// 零宽松归一）。
#[test]
fn 文件树词表词汇外反序列化一律失败() {
    for probe in ["Dir", "DIR", "file ", "", "directory"] {
        assert!(
            serde_json::from_value::<FileEntryKind>(serde_json::Value::String(probe.to_owned()))
                .is_err(),
            "FileEntryKind 词外值 {probe:?} 应 Err（rename_all = lowercase 严格）"
        );
    }
    // `NodeMark` 的词在同一位置必须被拒（两词表不互相放行）
    for probe in [
        "ElementRoot",
        "elementroot",
        "none ",
        "",
        "unfulfilled",
        "crossKind",
    ] {
        assert!(
            serde_json::from_value::<FileTreeMark>(serde_json::Value::String(probe.to_owned()))
                .is_err(),
            "FileTreeMark 词外值 {probe:?} 应 Err（rename_all = camelCase 严格）"
        );
    }
    // 反向判据：`NodeMark` 的合法词不被 `FileTreeMark` 接受
    assert!(
        serde_json::from_value::<FileTreeMark>(
            serde_json::to_value(NodeMark::Unfulfilled).expect("序列化")
        )
        .is_err(),
        "两词表互不放行（FileTreeMark MUST NOT 复用 NodeMark 的词）"
    );
    assert!(
        serde_json::from_value::<FileEntryKind>(
            serde_json::to_value(FileTreeMark::Member).expect("序列化")
        )
        .is_err(),
        "FileEntryKind 不接受 FileTreeMark 的词"
    );
}

/// 正向（AC-3 / spec Scenario 条目七字段按声明序出线）：键序恰为七字段的 camelCase
/// 形态，往返逐字段相等。
#[test]
fn 文件条目七字段按声明序出线() {
    let entry = file_entry();
    let value = serde_json::to_value(&entry).expect("序列化");
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec!["path", "name", "kind", "mark", "owners", "contains", "exists"],
        "七字段按声明序出线（camelCase 无损形）"
    );
    let back: ArchiFileEntry = serde_json::from_value(value).expect("反序列化");
    assert_eq!(back, entry, "往返逐字段相等");
}

/// 边界（容器与数值映射）：`owners` 空 / 单元素 / 超大 / 非字典序（DTO 零重排）、
/// `contains` 取 `0` / `u32::MAX`、`exists` 两值、`kind` / `mark` 全枚举值。
#[test]
fn 文件条目字段值边界往返() {
    let owners_cases: Vec<Vec<String>> = vec![
        vec![],
        vec!["App.A".to_owned()],
        (0..1000).map(|index| format!("App.M{index:04}")).collect(),
        vec!["App.Z".to_owned(), "App.A".to_owned()],
    ];
    for owners in owners_cases {
        for contains in [0u32, u32::MAX] {
            for exists in [true, false] {
                for kind in FileEntryKind::ALL {
                    for mark in FileTreeMark::ALL {
                        let entry = ArchiFileEntry {
                            path: "a/b/c.rs".to_owned(),
                            name: "c.rs".to_owned(),
                            kind,
                            mark,
                            owners: owners.clone(),
                            contains,
                            exists,
                        };
                        let value = serde_json::to_value(&entry).expect("序列化");
                        let back: ArchiFileEntry =
                            serde_json::from_value(value.clone()).expect("反序列化");
                        assert_eq!(back, entry, "组合边界往返逐字段相等");
                        // DTO 零加工：owners 顺序原样出线（排序归投影侧）
                        let Some(serde_json::Value::Array(out)) = value.get("owners") else {
                            panic!("owners 应为数组");
                        };
                        assert_eq!(out.len(), entry.owners.len(), "owners 数量原样（零裁剪）");
                    }
                }
            }
        }
    }
}

/// 边界（字符串映射）：`path` / `name` 取空串（越签名脏值）、超长、含换行 / NUL /
/// emoji 时序列化不 panic 且往返逐字相等。
#[test]
fn 文件条目路径与名字字符串边界() {
    let long = "长".repeat(1200);
    let cases: [(&str, &str); 4] = [
        ("", ""),
        ("a/b/c.rs", "c.rs"),
        (long.as_str(), long.as_str()),
        ("dir/c\nx\0🚀.ts", "c\nx\0🚀.ts"),
    ];
    for (path, name) in cases {
        let entry = ArchiFileEntry {
            path: path.to_owned(),
            name: name.to_owned(),
            kind: FileEntryKind::Dir,
            mark: FileTreeMark::None,
            owners: Vec::new(),
            contains: 0,
            exists: true,
        };
        let value = serde_json::to_value(&entry).expect("脏字符串不阻序列化");
        let back: ArchiFileEntry = serde_json::from_value(value).expect("反序列化");
        assert_eq!(back, entry, "字符串边界往返逐字相等");
    }
}

/// 正向（AC-3）：`ArchiFileTree` 单字段扁平——空数组出线恰 `{"entries":[]}`，
/// 三条条目为扁平数组、无 `children` 键、顺序即入参顺序。
#[test]
fn 文件树单字段扁平与顺序零重排() {
    let empty = ArchiFileTree {
        entries: Vec::new(),
    };
    assert_eq!(
        serde_json::to_string(&empty).expect("序列化"),
        "{\"entries\":[]}",
        "空树恰单键单数组"
    );

    let three = ArchiFileTree {
        entries: vec![
            ArchiFileEntry {
                path: "b.ts".to_owned(),
                name: "b.ts".to_owned(),
                ..file_entry()
            },
            ArchiFileEntry {
                path: "a/c.ts".to_owned(),
                name: "c.ts".to_owned(),
                ..file_entry()
            },
            file_entry(),
        ],
    };
    let value = serde_json::to_value(&three).expect("序列化");
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["entries"], "ArchiFileTree 单字段");
    let entries = value["entries"].as_array().expect("entries 是数组");
    assert_eq!(entries.len(), 3, "扁平数组（无嵌套）");
    assert!(
        entries.iter().all(|entry| !entry
            .as_object()
            .expect("条目对象")
            .contains_key("children")),
        "零嵌套 children 键"
    );
    let paths: Vec<&str> = entries
        .iter()
        .map(|entry| entry["path"].as_str().expect("字符串"))
        .collect();
    assert_eq!(
        paths,
        vec!["b.ts", "a/c.ts", "crates/core/archi/src/tree.rs"],
        "顺序即入参顺序（DTO 零重排，排序归投影侧）"
    );
    let back: ArchiFileTree = serde_json::from_value(value).expect("反序列化");
    assert_eq!(back, three, "往返相等");
}

/// 正向（AC-3 / AC-14）：`specta::Type` 出线名与 Rust 类型名一致（四新类型各在册）。
#[test]
fn 四新类型的出线名与字段面() {
    use specta::Type;

    fn registered<T: Type>() -> Vec<String> {
        let types = specta::Types::default().register::<T>();
        types
            .into_sorted_iter()
            .map(|named| named.name.to_string())
            .collect()
    }

    for (name, names) in [
        ("ArchiFileTree", registered::<ArchiFileTree>()),
        ("ArchiFileEntry", registered::<ArchiFileEntry>()),
    ] {
        assert!(
            names.iter().any(|registered| registered == name),
            "{name} 应注册进 specta 类型表：{names:?}"
        );
    }
    for (name, names) in [
        ("FileEntryKind", registered::<FileEntryKind>()),
        ("FileTreeMark", registered::<FileTreeMark>()),
    ] {
        assert!(
            names.iter().any(|registered| registered == name),
            "{name} 应注册进 specta 类型表：{names:?}"
        );
    }
}

/// 枚举线值清单（`serde_json` 单值序列化；字面量镜像即尺寸判据）。
fn line_words<T: serde::Serialize + Copy>(values: Vec<T>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| {
            serde_json::to_value(value)
                .expect("序列化")
                .as_str()
                .expect("线值为字符串")
                .to_owned()
        })
        .collect()
}

/// 边界（AC-17 / 类型单点纪律）：新增类型为并列追加——既有九组词汇尺寸与既有
/// 出线 DTO 的键序列逐字不变，两词表零交叉引用。
#[test]
fn 既有词汇与出线类型零变化() {
    // 九组既有封闭词汇：有 `ALL` 常量的取长度，其余取 serde 线值——两份都钉死尺寸
    assert_eq!(ElementKind::ALL.len(), 4, "ElementKind 仍四值");
    assert_eq!(GateTier::ALL.len(), 3, "GateTier 仍三档");
    assert_eq!(DiagnosticKind::ALL.len(), 9, "DiagnosticKind 仍九值");
    assert_eq!(ApiStatus::ALL.len(), 2, "ApiStatus 仍二值");
    assert_eq!(
        line_words([NodeMark::None, NodeMark::Unfulfilled].to_vec()),
        vec!["none", "unfulfilled"],
        "NodeMark 仍两值"
    );
    assert_eq!(
        line_words(
            [
                EdgeMark::None,
                EdgeMark::Unfulfilled,
                EdgeMark::CrossKind,
                EdgeMark::Exempt,
            ]
            .to_vec()
        ),
        vec!["none", "unfulfilled", "crossKind", "exempt"],
        "EdgeMark 仍四值"
    );
    assert_eq!(
        line_words([WarningKind::DocNotFound].to_vec()),
        vec!["docNotFound"],
        "WarningKind 仍单值"
    );
    assert_eq!(
        line_words([DocKind::Md, DocKind::Adr].to_vec()),
        vec!["md", "adr"],
        "DocKind 仍两值"
    );
    assert_eq!(
        line_words([DocSource::Explicit, DocSource::Adjacent, DocSource::Text].to_vec()),
        vec!["explicit", "adjacent", "text"],
        "DocSource 仍三值（声明序即强度序）"
    );
    assert_eq!(
        line_words([SearchScope::Elements, SearchScope::Docs, SearchScope::All].to_vec()),
        vec!["elements", "docs", "all"],
        "SearchScope 仍三值"
    );

    // 既有出线 DTO 键序列逐字不变（新增类型并列，不入既有 DTO）
    let graph = serde_json::to_value(ArchiGraph {
        elements: Vec::new(),
        relations: Vec::new(),
        diagnostics: Vec::new(),
    })
    .expect("序列化");
    let keys: Vec<&str> = graph
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["elements", "relations", "diagnostics"]);
    let element_value = serde_json::to_value(ArchiElement {
        fqn: "App".to_owned(),
        kind: ElementKind::Package,
        gate: GateTier::Off,
        api: ApiStatus::None,
        path: Vec::new(),
        summary: None,
        metadata: std::collections::BTreeMap::new(),
        model_file: "a.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
    })
    .expect("序列化");
    let element_keys: Vec<&str> = element_value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        element_keys,
        vec![
            "fqn",
            "kind",
            "gate",
            "path",
            "summary",
            "metadata",
            "modelFile",
            "modelLine",
            "mark",
            "api"
        ],
        "ArchiElement 既有键序零变化"
    );

    // 源码自证：两词表在 `dto.rs` 内各恰一处定义、零映射
    let source = include_str!("dto.rs");
    assert_eq!(
        source.matches("pub enum FileTreeMark").count(),
        1,
        "FileTreeMark 恰一处定义"
    );
    assert_eq!(
        source.matches("pub enum NodeMark").count(),
        1,
        "NodeMark 定义零变化（仍恰一处）"
    );
    assert_eq!(
        source.matches("pub enum FileEntryKind").count(),
        1,
        "FileEntryKind 恰一处定义"
    );
    assert!(
        !source.contains("NodeMark::Unfulfilled => FileTreeMark")
            && !source.contains("FileTreeMark::None => NodeMark"),
        "两词表零映射（MUST NOT 复用）"
    );
}

// ---------------------------------------------------------------------------
// 本变更新增三 DTO（ArchiApiCheck / ArchiApiMeta / ArchiApiWriteOutcome）
// ---------------------------------------------------------------------------

/// 五字段 DTO 夹具（全字段给定值）。
fn api_check_fixture() -> ArchiApiCheck {
    ArchiApiCheck {
        enabled: true,
        meta: ArchiApiMeta {
            tool_version: "0.6.40".to_owned(),
            common_digest: format!("sha256:{}", "a".repeat(64)),
        },
        mismatched: vec!["crates/core/archi.i.rs".to_owned()],
        orphaned: vec!["src/loose.i.ts".to_owned()],
        missing: vec![".dev-team/architecture/api/INDEX.toml".to_owned()],
    }
}

/// 正向（AC-3 / spec「DTO 形状」）：五字段声明序出线（camelCase），序列化往返逐字段相等。
#[test]
fn api_check五字段声明序出线() {
    let dto = api_check_fixture();
    let value = serde_json::to_value(&dto).expect("序列化成功");
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec!["enabled", "meta", "mismatched", "orphaned", "missing"],
        "五字段声明序（camelCase）"
    );

    let back: ArchiApiCheck = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(back, dto, "往返逐字段相等");
}

/// 正向（AC-3）：`ArchiApiMeta` 二字段（`toolVersion` / `commonDigest`）；摘要为
/// `sha256:<hex>` 形态且不截断（长度 ≥ 71）。
#[test]
fn api_meta二字段形态() {
    let meta = api_check_fixture().meta;
    let value = serde_json::to_value(&meta).expect("序列化成功");
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["toolVersion", "commonDigest"], "二字段声明序");
    assert_eq!(meta.tool_version, "0.6.40", "版本串逐字");
    assert!(
        meta.common_digest.starts_with("sha256:"),
        "摘要形如 sha256:<hex>"
    );
    assert!(
        meta.common_digest.len() >= 71,
        "摘要不截断：{}",
        meta.common_digest
    );

    let back: ArchiApiMeta = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(back, meta, "往返逐字段相等");
}

/// 正向（AC-3）：`ArchiApiWriteOutcome` 二字段（`written` / `unchanged`）；两列不交叠。
#[test]
fn api_write_outcome二字段() {
    let outcome = ArchiApiWriteOutcome {
        written: vec!["a.i.rs".to_owned(), "b.i.ts".to_owned()],
        unchanged: vec!["c.schema.json".to_owned()],
    };
    let value = serde_json::to_value(&outcome).expect("序列化成功");
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["written", "unchanged"], "二字段声明序");
    for entry in &outcome.written {
        assert!(
            !outcome.unchanged.contains(entry),
            "两列不交叠：{entry} 同时出现在两列"
        );
    }

    let back: ArchiApiWriteOutcome = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(back, outcome, "往返逐字段相等");
}

/// 边界（边界 / str 与 list 映射）：`enabled` 取两值；字符串取空串 / 超长 / 含 `\n` 与
/// emoji；三列与两列取 `[]` / 单元素 / 1000 条 / **非字典序输入**（DTO 零重排）→ 往返后
/// 逐字段相等。
#[test]
fn 三新dto字段值边界() {
    for enabled in [true, false] {
        let mut dto = api_check_fixture();
        dto.enabled = enabled;
        let back: ArchiApiCheck =
            serde_json::from_value(serde_json::to_value(&dto).expect("序列化")).expect("反序列化");
        assert_eq!(back.enabled, enabled, "enabled 两值往返");
    }

    let dirty_strings = [
        String::new(),
        "x".repeat(1100),
        "含换行\n与制表\t与控制\u{0}与 emoji 🎯".to_owned(),
    ];
    for dirty in dirty_strings {
        let mut dto = api_check_fixture();
        dto.meta.tool_version = dirty.clone();
        dto.meta.common_digest = dirty.clone();
        let back: ArchiApiCheck =
            serde_json::from_value(serde_json::to_value(&dto).expect("序列化")).expect("反序列化");
        assert_eq!(back.meta.tool_version, dirty, "越签名脏串往返不 panic");
        assert_eq!(back.meta.common_digest, dirty, "脏串往返不 panic");
    }

    let column_forms: [Vec<String>; 4] = [
        Vec::new(),
        vec!["single.i.rs".to_owned()],
        (0..1000)
            .map(|index| format!("gen/o{index:04}.i.ts"))
            .collect(),
        vec![
            "zzz.i.rs".to_owned(),
            "aaa.i.ts".to_owned(),
            "mmm.schema.json".to_owned(),
        ],
    ];
    for column in column_forms {
        let mut dto = api_check_fixture();
        dto.mismatched = column.clone();
        dto.orphaned = column.clone();
        dto.missing = column.clone();
        let back: ArchiApiCheck =
            serde_json::from_value(serde_json::to_value(&dto).expect("序列化")).expect("反序列化");
        assert_eq!(back, dto, "列容器形态往返（零重排）");
        assert_eq!(back.mismatched, column, "列序逐字保留");
    }

    let outcome = ArchiApiWriteOutcome {
        written: vec!["zzz.i.rs".to_owned(), "aaa.i.ts".to_owned()],
        unchanged: (0..1000).map(|index| format!("u{index:04}.i.ts")).collect(),
    };
    let back: ArchiApiWriteOutcome =
        serde_json::from_value(serde_json::to_value(&outcome).expect("序列化")).expect("反序列化");
    assert_eq!(back, outcome, "写盘结果两列往返");
}

/// 异常（异常 / 对象映射）：缺必填键 / `null` / 类型错 → `Err`；多余字段按既有口径
/// **忽略**（结构体无 `deny_unknown_fields`，用例钉死实际行为）。
#[test]
fn 三新dto词汇外与缺字段反序列化() {
    let base = serde_json::to_value(api_check_fixture()).expect("序列化");

    let mut missing_meta = base.clone();
    missing_meta.as_object_mut().expect("对象").remove("meta");
    assert!(
        serde_json::from_value::<ArchiApiCheck>(missing_meta).is_err(),
        "缺 meta 键 → Err"
    );

    let mut null_meta = base.clone();
    null_meta["meta"] = json!(null);
    assert!(
        serde_json::from_value::<ArchiApiCheck>(null_meta).is_err(),
        "meta 为 null → Err"
    );

    for column in ["mismatched", "orphaned", "missing"] {
        let mut null_column = base.clone();
        null_column[column] = json!(null);
        assert!(
            serde_json::from_value::<ArchiApiCheck>(null_column).is_err(),
            "{column} 为 null → Err"
        );
    }

    let mut wrong_type = base.clone();
    wrong_type["enabled"] = json!("true");
    assert!(
        serde_json::from_value::<ArchiApiCheck>(wrong_type).is_err(),
        "enabled 传字符串 → Err"
    );

    let mut extra = base.clone();
    extra["extra"] = json!(1);
    let with_extra: ArchiApiCheck =
        serde_json::from_value(extra).expect("多余字段按既有口径忽略（无 deny_unknown_fields）");
    assert_eq!(with_extra, api_check_fixture(), "多余字段不影响已知字段");
}

// ---------------------------------------------------------------------------
// graph_diagnostics_error（提权后的文案单点）
// ---------------------------------------------------------------------------

/// 诊断构造器（`line = 0` 表示位置未知）。
fn diagnostic(kind: DiagnosticKind, file: &str, line: u32) -> ArchiDiagnostic {
    ArchiDiagnostic {
        kind,
        message: "原文文案".to_owned(),
        file: file.to_owned(),
        line,
        column: None,
    }
}

/// 正向（AC-15 的错误面）：0 / 1 / 3 条诊断 → 首行「模型结构错（共 N 条）：」，随后逐条
/// `kind @ file:line`（`line = 0` 时只给 `kind @ file`）；空输入也是非空固定文案。
#[test]
fn graph_diagnostics_error文案形态() {
    let empty = dto::graph_diagnostics_error(&[]);
    assert!(!empty.is_empty(), "空诊断仍是非空文案：{empty}");
    assert!(
        empty.contains("模型结构错（共 0 条）："),
        "空诊断的固定文案：{empty}"
    );

    let one = dto::graph_diagnostics_error(&[diagnostic(DiagnosticKind::Cycle, "01-a.toml", 7)]);
    assert!(one.contains("模型结构错（共 1 条）："), "条数 1：{one}");
    assert!(one.contains("cycle @ 01-a.toml:7"), "位置形态：{one}");

    let sample = [
        diagnostic(DiagnosticKind::Cycle, "01-a.toml", 7),
        diagnostic(DiagnosticKind::OrphanHierarchy, "02-b.toml", 0),
        diagnostic(DiagnosticKind::UnknownReference, "03-c.toml", 12),
    ];
    let three = dto::graph_diagnostics_error(&sample);
    assert!(three.contains("模型结构错（共 3 条）："), "条数 3：{three}");
    assert!(
        three.contains("orphanHierarchy @ 02-b.toml"),
        "line = 0 时只给文件：{three}"
    );
    assert_eq!(three.lines().count(), 4, "首行 + 三条诊断");

    let again = dto::graph_diagnostics_error(&sample);
    assert_eq!(again, three, "纯函数：同入参两次调用逐字相同");
}

/// 边界（边界）：提权后可从 crate 根出口调用；诊断含超长 `message` / 含 `\n` 与 emoji
/// → 不 panic 且文案只投影 `kind` 与位置（诊断原文不进文案）。
#[test]
fn graph_diagnostics_error提权与脏值() {
    let dirty = [
        ArchiDiagnostic {
            kind: DiagnosticKind::InvalidValue,
            message: "y".repeat(1200),
            file: "含 emoji 🎯 的文件.toml".to_owned(),
            line: 1,
            column: Some(3),
        },
        ArchiDiagnostic {
            kind: DiagnosticKind::SyntaxError,
            message: "换行\n与制表\t".to_owned(),
            file: "a".repeat(1100),
            line: 2,
            column: None,
        },
    ];
    let text = dto::graph_diagnostics_error(&dirty);
    assert!(text.contains("模型结构错（共 2 条）："), "条数：{text}");
    assert!(text.contains("🎯"), "文件名原样投影：{text}");
    assert!(
        !text.contains("y".repeat(20).as_str()),
        "超长诊断原文不进文案（只投影 kind 与位置）"
    );

    let exported = crate::graph_diagnostics_error(&dirty);
    assert_eq!(exported, text, "出口面与 dto 面同一函数");
}
