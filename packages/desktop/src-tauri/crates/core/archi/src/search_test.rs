//! `search` 的单元测试：分词（驼峰 / 数字 / 大小写）、多词 AND、匹配对象面
//! （FQN / `summary` / `metadata` 值 / 入口文档路径与文件名）、不搜正文、不碰规约
//! 语料与三 scope 收窄。
//!
//! 零 mock：真实临时目录 + 真实 `ModelGraph` 组合（检索语料 = `path` 非空元素的在盘
//! 入口文档载体，替身会抹掉载体派生口径）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::dto::{ApiStatus, ElementKind, GateTier, SearchScope};
use crate::graph::ModelGraph;
use crate::model::{Element, Model};

use crate::search::search;

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-search-test-{}-{}-{}",
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
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 元素构造器。
fn element(fqn: &str) -> Element {
    Element {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Off,
        api: ApiStatus::None,
        entry: None,
        path: Vec::new(),
        doc: None,
        summary: None,
        metadata: BTreeMap::new(),
        file: ".dev-team/architecture/graph/model/01-core.toml".to_owned(),
        line: 1,
    }
}

/// 单元素图（无关系）。
fn graph_of(elements: Vec<Element>) -> ModelGraph {
    ModelGraph::build(Model {
        elements,
        relations: Vec::new(),
    })
}

/// 命中元素 FQN 清单。
fn hits(graph: &ModelGraph, root: &Path, query: &str, scope: SearchScope) -> Vec<String> {
    search(graph, root, query, scope)
        .unwrap_or_else(|error| panic!("检索 {query:?} 失败: {error}"))
        .element_fqns
}

/// 命中文档路径清单。
fn doc_hits(graph: &ModelGraph, root: &Path, query: &str) -> Vec<String> {
    search(graph, root, query, SearchScope::All)
        .expect("检索成功")
        .doc
}

/// 落一份派生文档（`<archi_models_dir>/<分层目录…>/<叶段>.md`）并返回其 repo 相对路径。
fn write_doc(temp: &TempDir, fqn: &str) -> String {
    let path = crate::api::paths::contract_path(temp.root(), fqn).expect("合法 FQN");
    let rel = crate::api::display(temp.root(), &path);
    temp.write(&rel, "# 文档\n\n正文。\n");
    rel
}

/// 落一份接口定义（同目录 `<叶段>.schema.json`）并返回其 repo 相对路径。
fn write_schema(temp: &TempDir, fqn: &str) -> String {
    let path = crate::api::paths::schema_path(temp.root(), fqn).expect("合法 FQN");
    let rel = crate::api::display(temp.root(), &path);
    temp.write(&rel, "{\"$defs\": {}, \"properties\": {}}\n");
    rel
}

// ---------------------------------------------------------------------------
// 正向：命中面与稳定序
// ---------------------------------------------------------------------------

/// 正向：`query = "auth"`、`scope = All` → 命中 FQN 含 `auth` 的元素 + 路径 / 文件主干含
/// `auth` 的派生资产（`<叶段>.md` ∪ `<叶段>.schema.json`）；元素按 FQN、资产按路径字典序。
///
/// 语料 = `path` 非空元素的在盘派生资产（与判据面同一集合），故夹具只落派生文件。
#[test]
fn 检索命中元素与文档并按稳定序返回() {
    let temp = TempDir::new("all-scope");
    let auth_doc = write_doc(&temp, "App.Auth");
    let billing_doc = write_doc(&temp, "App.Billing");
    let mut auth = element("App.Auth");
    auth.path = vec!["./src/auth/".to_owned()];
    let mut billing = element("App.Billing");
    billing.path = vec!["./src/billing/".to_owned()];
    let graph = graph_of(vec![billing, auth]);

    let outcome = search(&graph, temp.root(), "auth", SearchScope::All).expect("检索成功");
    assert_eq!(
        outcome.element_fqns,
        vec!["App.Auth"],
        "只命中 FQN 含 auth 的元素且按字典序"
    );
    assert_eq!(
        outcome.doc,
        vec![auth_doc],
        "只命中路径 / 文件主干含 auth 的派生文档（billing 的文档不命中：{billing_doc}）"
    );
}

/// 正向：驼峰分词——`AuthToken` / `auth token`（空格）/ `AUTH`（大写）三者命中
/// 同一元素（AND 语义 + 大小写不敏感）。
#[test]
fn 驼峰分词与空格与大小写三形态等价() {
    let temp = TempDir::new("camel");
    let graph = graph_of(vec![element("App.Auth.Token")]);
    for query in ["AuthToken", "auth token", "AUTH", "authTOKEN"] {
        assert_eq!(
            hits(&graph, temp.root(), query, SearchScope::Elements),
            vec!["App.Auth.Token"],
            "query {query:?} 应命中 App.Auth.Token"
        );
    }
}

/// 正向：匹配对象面 = FQN / `summary` / `metadata` 值（元素侧）+ 派生资产路径与文件主干
/// （文档 / 接口两侧）；`metadata` 键名不参与。
#[test]
fn 匹配对象面覆盖summary与metadata值与文档路径与文件名() {
    let temp = TempDir::new("match-surface");
    let doc = write_doc(&temp, "App.Auth");
    let schema = write_schema(&temp, "App.Auth");
    let mut target = element("App.Auth");
    target.path = vec!["./docs/".to_owned()];
    target.summary = Some("限流（rate limit）说明".to_owned());
    target.metadata = BTreeMap::from([
        ("owner".to_owned(), "billing-team".to_owned()),
        ("secretkey".to_owned(), "zzz".to_owned()),
    ]);
    let graph = graph_of(vec![target]);

    // summary
    assert_eq!(
        hits(&graph, temp.root(), "限流", SearchScope::Elements),
        vec!["App.Auth"],
        "summary 参与匹配"
    );
    // metadata 值
    assert_eq!(
        hits(&graph, temp.root(), "billing", SearchScope::Elements),
        vec!["App.Auth"],
        "metadata 值参与匹配"
    );
    // metadata 键名不参与
    assert!(
        hits(&graph, temp.root(), "secretkey", SearchScope::Elements).is_empty(),
        "键名不参与匹配"
    );
    // 资产路径段（分层目录 `architecture/models` 参与匹配）
    assert_eq!(
        doc_hits(&graph, temp.root(), "models"),
        vec![doc.clone(), schema.clone()],
        "两份派生资产的路径段都参与匹配且按路径字典序：{doc} / {schema}"
    );
    // 文件主干（`App.Auth` 去扩展名后按分词命中）
    assert_eq!(
        doc_hits(&graph, temp.root(), "app auth"),
        vec![doc, schema],
        "文件主干分词参与匹配（多词 AND）"
    );
}

// ---------------------------------------------------------------------------
// 异常：不搜正文 / 不碰规约语料 / 参数校验
// ---------------------------------------------------------------------------

/// 异常：仅正文命中不算——md 正文含 `auth` 而路径与标题不含 → 不命中（不做全文
/// 搜索）。
#[test]
fn 仅正文含关键词不命中() {
    let temp = TempDir::new("body-only");
    temp.write("docs/guide.md", "# 指南\n\n认证（auth）流程说明。\n");
    let mut target = element("App.Billing");
    target.path = vec!["./docs/".to_owned()];
    target.doc = Some("./docs/guide.md".to_owned());
    let graph = graph_of(vec![target]);

    assert!(
        doc_hits(&graph, temp.root(), "auth").is_empty(),
        "不读正文（正文命中不算）"
    );
    assert!(
        !hits(&graph, temp.root(), "auth", SearchScope::All).contains(&"App.Billing".to_owned()),
        "元素侧亦不因正文命中"
    );
}

/// 异常：`openspec/specs/` 与 `decisions/` 的 md 即便正文与文件名含关键词也不参与——
/// 语料只有 `path` 非空元素的派生资产。
#[test]
fn 规约语料目录不参与检索() {
    let temp = TempDir::new("specs-excluded");
    temp.write(
        "openspec/specs/desktop-archi-model/spec.md",
        "# archi 能力规约\n\n涉及 App.Auth 与 auth 的说明。\n",
    );
    temp.write("decisions/0001-auth.md", "# ADR：auth 的切法\n");
    let doc = write_doc(&temp, "App.Auth");
    let mut target = element("App.Auth");
    target.path = vec!["./docs/".to_owned()];
    let graph = graph_of(vec![target]);

    let docs_hit = doc_hits(&graph, temp.root(), "auth");
    assert!(
        docs_hit
            .iter()
            .all(|path| !path.contains("openspec/specs") && !path.contains("decisions/")),
        "规约 / ADR 目录零出现: {docs_hit:?}"
    );
    assert_eq!(docs_hit, vec![doc], "只来自元素的派生资产（零第二份语料）");
    assert_eq!(
        hits(&graph, temp.root(), "auth", SearchScope::All),
        vec!["App.Auth"],
        "元素侧命中照常"
    );
}

/// 异常：空 query（`""` 与全空白）→ `Err`（不返回全量）；含 `\n` / `\0` / emoji
/// 的 query 正常搜索、不 panic。
#[test]
fn 空query返回err且特殊字符query不panic() {
    let temp = TempDir::new("empty-query");
    let graph = graph_of(vec![element("App.Auth")]);
    for query in ["", "   ", "\t\n", "🚀"] {
        let result = search(&graph, temp.root(), query, SearchScope::All);
        assert!(
            result.is_err(),
            "空 / 无字母数字的 query 应 Err（不返回全量）: {query:?}"
        );
        assert!(
            result.unwrap_err().contains("query"),
            "错误文案点名 query 参数"
        );
    }
    for query in ["auth\n", "auth\0", "auth🚀", "🚀auth"] {
        let outcome = search(&graph, temp.root(), query, SearchScope::All)
            .unwrap_or_else(|error| panic!("{query:?} 应可检索: {error}"));
        assert_eq!(outcome.element_fqns, vec!["App.Auth"], "{query:?} 正常命中");
    }
}

// ---------------------------------------------------------------------------
// 边界：scope / 多词 / 分词 / 空结果 / 规模
// ---------------------------------------------------------------------------

#[test]
fn scope三值收窄语义() {
    let temp = TempDir::new("scope");
    let doc = write_doc(&temp, "App.Auth");
    let mut target = element("App.Auth");
    target.path = vec!["./docs/".to_owned()];
    let graph = graph_of(vec![target]);

    let elements_only = search(&graph, temp.root(), "auth", SearchScope::Elements).expect("成功");
    assert_eq!(elements_only.element_fqns, vec!["App.Auth"]);
    assert!(elements_only.doc.is_empty(), "Elements 时 doc 恒空");

    let docs_only = search(&graph, temp.root(), "auth", SearchScope::Doc).expect("成功");
    assert!(
        docs_only.element_fqns.is_empty(),
        "Doc 时 element_fqns 恒空"
    );
    assert_eq!(docs_only.doc, vec![doc], "Doc 只回资产路径");

    let all = search(&graph, temp.root(), "auth", SearchScope::All).expect("成功");
    assert!(
        !all.element_fqns.is_empty() && !all.doc.is_empty(),
        "All 两数组皆非空"
    );
}

/// 边界：多词 AND——`"auth legacy"` 只命中同时含两段的元素，只含其一的元素不命中。
#[test]
fn 多词and语义() {
    let temp = TempDir::new("and-semantics");
    let graph = graph_of(vec![
        element("App.Auth.Legacy"),
        element("App.Auth"),
        element("App.Legacy"),
    ]);
    assert_eq!(
        hits(&graph, temp.root(), "auth legacy", SearchScope::Elements),
        vec!["App.Auth.Legacy"],
        "只命中同时含两段的元素"
    );
    assert_eq!(
        hits(&graph, temp.root(), "auth", SearchScope::Elements),
        vec!["App.Auth", "App.Auth.Legacy"],
        "单词命中二者"
    );
}

/// 边界：分词边界——`HTTPServer` → `http` + `server`；`v2Token` → `v2` + `token`；
/// 全大写 `AUTHTOKEN` 不切分（无小写边界）；重复段去重后行为不变。
#[test]
fn 分词边界四形态() {
    let temp = TempDir::new("tokenize");
    let graph = graph_of(vec![
        element("App.HTTPServer"),
        element("App.V2Token"),
        element("App.Auth.Token"),
    ]);
    assert_eq!(
        hits(&graph, temp.root(), "http server", SearchScope::Elements),
        vec!["App.HTTPServer"],
        "HTTPServer 分词为 http + server"
    );
    assert_eq!(
        hits(&graph, temp.root(), "v2 token", SearchScope::Elements),
        vec!["App.V2Token"],
        "v2Token 分词为 v2 + token"
    );
    assert!(
        hits(&graph, temp.root(), "AUTHTOKEN", SearchScope::Elements).is_empty(),
        "全大写不切分（无小写边界）"
    );
    assert_eq!(
        hits(&graph, temp.root(), "AuthAuth", SearchScope::Elements),
        hits(&graph, temp.root(), "auth", SearchScope::Elements),
        "重复段去重后与单段等价"
    );
}

/// 边界：无命中 → 两数组皆空且不 `Err`；空模型 → 空结果。
#[test]
fn 无命中与空模型返回空数组不报错() {
    let temp = TempDir::new("no-hit");
    let graph = graph_of(vec![element("App.Auth")]);
    let outcome = search(&graph, temp.root(), "zzz", SearchScope::All).expect("无命中仍 Ok");
    assert!(outcome.element_fqns.is_empty());
    assert!(outcome.doc.is_empty());

    let empty = graph_of(Vec::new());
    let outcome = search(&empty, temp.root(), "auth", SearchScope::All).expect("空模型仍 Ok");
    assert!(outcome.element_fqns.is_empty() && outcome.doc.is_empty());
}

/// 边界：500 元素（200 有派生文档）→ 规模面稳定、两次调用逐字相同；超长 query 正常返回、
/// 零命中；资产命中按路径字典序。
#[test]
fn 超长query与规模面结果稳定() {
    let temp = TempDir::new("long-query");
    let mut elements = Vec::new();
    for index in 0..500 {
        let mut item = element(&format!("App.Auth.M{index:03}"));
        item.path = vec![format!("src/m{index:03}/")];
        if index < 200 {
            write_doc(&temp, &item.fqn);
        }
        elements.push(item);
    }
    let graph = graph_of(elements);

    let long = format!("auth{}", "x".repeat(1200));
    // 超长单段不与任何 FQN 匹配，但必须正常返回（不截断、不 panic）
    let outcome =
        search(&graph, temp.root(), &long, SearchScope::All).expect("超长 query 正常返回");
    assert!(outcome.element_fqns.is_empty(), "超长段无命中");
    assert!(outcome.doc.is_empty(), "资产侧同样零命中");

    let first = search(&graph, temp.root(), "auth", SearchScope::All).expect("成功");
    let second = search(&graph, temp.root(), "auth", SearchScope::All).expect("成功");
    assert_eq!(first, second, "两次调用结果逐字相同");
    assert_eq!(first.element_fqns.len(), 500, "500 元素全部命中 auth");
    assert_eq!(first.doc.len(), 200, "200 份派生文档按路径命中 auth");

    // 资产侧：同目录段（`architecture`）命中全部派生资产且元素侧零命中
    let docs_hit = search(&graph, temp.root(), "architecture", SearchScope::All).expect("成功");
    assert!(
        docs_hit.element_fqns.is_empty(),
        "元素 FQN 不含 architecture"
    );
    let paths: Vec<&str> = docs_hit.doc.iter().map(String::as_str).collect();
    assert_eq!(paths.len(), 200, "200 份资产按目录段命中");
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted, "资产命中按路径字典序");
}

/// 边界：语料 = `path` 非空元素的**在盘**派生资产（文档 ∪ 接口）——`path` 空的元素不入
/// 语料，资产不在盘亦不入语料。
#[test]
fn 检索文档语料为path非空元素的在盘载体() {
    let temp = TempDir::new("same-source");
    let doc = write_doc(&temp, "App.Auth");
    let schema = write_schema(&temp, "App.Auth");
    let mut target = element("App.Auth");
    target.path = vec!["./src/auth/".to_owned()];
    // `path` 为空：即便派生路径上有文件，也不入语料
    let container = element("App");
    write_doc(&temp, "App");
    write_schema(&temp, "App");
    let graph = graph_of(vec![target, container]);

    let outcome = search(&graph, temp.root(), "app", SearchScope::Doc).expect("成功");
    assert_eq!(
        outcome.doc,
        vec![doc, schema],
        "只取 `path` 非空元素的在盘资产（容器元素的派生文件不入语料）"
    );

    // 资产不在盘：不入语料
    let missing = graph_of(vec![{
        let mut only = element("App.Missing");
        only.path = vec!["src/missing/".to_owned()];
        only
    }]);
    let outcome = search(&missing, temp.root(), "app", SearchScope::Doc).expect("成功");
    assert!(outcome.doc.is_empty(), "资产不在盘 ⇒ 零条目");
}

/// 边界（`entry` 零敏感面）：两元素仅 `entry` 不同 → 命中集与文档条目逐值相同（检索面
/// 只看 FQN / `summary` / `metadata` / 入口文档路径，不搜 `path` / `entry`）。
#[test]
fn 检索面对entry零敏感() {
    let temp = TempDir::new("search-entry-blind");
    let plain = element("App.Auth");
    let mut declared = element("App.Auth");
    declared.entry = Some("packages/desktop/src/auth/token.ts".to_owned());

    let baseline = search(
        &graph_of(vec![plain]),
        temp.root(),
        "Auth",
        SearchScope::All,
    )
    .expect("检索应成功");
    let decorated = search(
        &graph_of(vec![declared]),
        temp.root(),
        "Auth",
        SearchScope::All,
    )
    .expect("检索应成功");

    assert_eq!(
        baseline.element_fqns, decorated.element_fqns,
        "元素命中集与 `entry` 无关"
    );
    assert_eq!(baseline.doc, decorated.doc, "文档条目与 `entry` 无关");

    // 反例校准：`entry` 的文本不是检索目标（搜入口串的末段不命中元素）
    let by_entry = search(
        &graph_of(vec![{
            let mut element = element("App.Auth");
            element.entry = Some("packages/desktop/src/auth/token.ts".to_owned());
            element
        }]),
        temp.root(),
        "token",
        SearchScope::Elements,
    )
    .expect("检索应成功");
    assert!(
        by_entry.element_fqns.is_empty(),
        "检索面不含 `entry` 文本：{:?}",
        by_entry.element_fqns
    );
}

// ===========================================================================
// `doc` 语料的投影单点 / `file_stem` 形态 / ADR 退役
// ===========================================================================

/// 边界：语料派生的源码单点——零 `carrier_path` / `doc_path` 复活；`contract_path` 与
/// `schema_path` 各恰一处；零手拼 `<叶段>.md`；`api` 状态与 `doc` 字段零参与。
#[test]
fn 语料投影走元素单点() {
    let source = include_str!("search.rs");
    assert!(
        !source.contains("carrier_path(") && !source.contains("doc_path("),
        "零第二份载体派生（`carrier_path` / `doc_path` 已退役）"
    );
    assert_eq!(
        source.matches("contract_path(").count(),
        1,
        "文档落点经 `paths::contract_path` 恰一处"
    );
    assert_eq!(
        source.matches("schema_path(").count(),
        1,
        "接口落点经 `paths::schema_path` 恰一处"
    );
    assert!(
        !source.contains("format!(\"{}.md"),
        "零手拼 `<叶段>.md`（落点派生单点）"
    );
    assert!(
        !source.contains("element.doc"),
        "`doc` 字段零参与语料（约定字段，非资产路径）"
    );
    assert!(
        !source.contains("element.api"),
        "`api` 状态零参与语料（两条固定规则只由在盘事实决定）"
    );
}

#[test]
fn 文件名主干迁入本文件且形态不变() {
    let source = include_str!("search.rs");
    assert_eq!(
        source.matches("fn file_stem(").count(),
        1,
        "`file_stem` 在检索面恰一处（迁移后零第二份）"
    );

    // 多点 FQN（`App.A.B`）的派生名 `App.A.B.md`：主干取法不丢文件名（只剥一层扩展名）
    let temp = TempDir::new("stem-forms");
    let dotted = write_doc(&temp, "App.A.B");
    let schema = write_schema(&temp, "App.A.B");
    let mut target = element("App.A.B");
    target.path = vec!["./docs/".to_owned()];
    let graph = graph_of(vec![target]);

    let hits = doc_hits(&graph, temp.root(), "a b");
    assert_eq!(
        hits,
        vec![dotted.clone(), schema.clone()],
        "多点主干按分词命中（零归一、不丢文件名）"
    );
    let hits = doc_hits(&graph, temp.root(), "b.md");
    assert!(
        hits.contains(&dotted) && !hits.contains(&schema),
        "`md` 只出现在文档后缀（接口文件不被误当文档命中）：{hits:?}"
    );
}

/// 异常：`decisions/*.md`（ADR）即便正文与文件名含关键词 → 零条目（旧 `text` 源随文档
/// 关联能力退役）。
#[test]
fn adr语料不参与检索() {
    let temp = TempDir::new("adr-retired");
    temp.write(
        ".dev-team/architecture/decisions/0001-auth-choice.md",
        "# auth 决策\n",
    );
    let mut target = element("App.Billing");
    target.path = vec!["./src/billing/".to_owned()];
    let graph = graph_of(vec![target]);

    assert!(
        doc_hits(&graph, temp.root(), "auth").is_empty(),
        "ADR 文件不入语料（`text` 源已退役）"
    );
}

// ===========================================================================
// 语料面：命中只由在盘事实决定 / `doc` 字段零参与
// ===========================================================================

/// 正向（四态 × 两值 `api`，八格）：语料恰为「在盘 `<叶段>.md`」∪「在盘
/// `<叶段>.schema.json`」——只有文档 / 只有接口 / 两者都在 / 都没有四态的命中集逐格固定，
/// 且 `api = declared` 与 `api = none` 两臂逐值相等（`api` 状态零参与语料）。
#[test]
fn 命中只由在盘事实决定() {
    let temp = TempDir::new("corpus-four-states");
    let states = ["OnlyDoc", "OnlySchema", "Both", "Neither"];
    let mut expected: Vec<String> = Vec::new();
    let mut declared: Vec<Element> = Vec::new();
    let mut none: Vec<Element> = Vec::new();
    for state in states {
        let fqn = format!("App.{state}.Cell");
        let mut cell = element(&fqn);
        cell.path = vec![format!("./src/{state}/")];
        match state {
            "OnlyDoc" => expected.push(write_doc(&temp, &fqn)),
            "OnlySchema" => expected.push(write_schema(&temp, &fqn)),
            "Both" => {
                expected.push(write_doc(&temp, &fqn));
                expected.push(write_schema(&temp, &fqn));
            }
            _ => {}
        }
        let mut without_api = cell.clone();
        cell.api = ApiStatus::Declared;
        without_api.api = ApiStatus::None;
        declared.push(cell);
        none.push(without_api);
    }
    expected.sort();

    let declared_hits = doc_hits(&graph_of(declared), temp.root(), "app");
    assert_eq!(
        declared_hits, expected,
        "四态逐格：只有文档 / 只有接口 / 两者都在 / 都没有"
    );
    let none_hits = doc_hits(&graph_of(none), temp.root(), "app");
    assert_eq!(
        none_hits, declared_hits,
        "`api = none` 与 `declared` 两臂逐值相等（`api` 零参与语料）"
    );
}

/// 边界（`doc` 字段退出语料，两形态）：同一批元素写 `doc` 值 / 不写 `doc` 值 ⇒ 命中集
/// 逐元素相等；`doc` 指不到盘也不改变语料（`carrier_path` 已退役，零第二份载体派生）。
#[test]
fn doc字段退出语料() {
    let temp = TempDir::new("doc-field-out");
    let doc = write_doc(&temp, "App.Auth");
    let schema = write_schema(&temp, "App.Auth");

    let mut with_doc = element("App.Auth");
    with_doc.path = vec!["./src/auth/".to_owned()];
    with_doc.doc = Some("guides/nowhere-auth.md".to_owned());
    let mut without_doc = with_doc.clone();
    without_doc.doc = None;

    let with_field = search(
        &graph_of(vec![with_doc]),
        temp.root(),
        "auth",
        SearchScope::All,
    )
    .expect("检索成功");
    let without_field = search(
        &graph_of(vec![without_doc]),
        temp.root(),
        "auth",
        SearchScope::All,
    )
    .expect("检索成功");
    assert_eq!(
        with_field, without_field,
        "写 / 不写 `doc` 值 ⇒ 命中集逐元素相等"
    );
    assert_eq!(
        with_field.doc,
        vec![doc, schema],
        "语料仍只有在盘派生资产（`doc` 指不到盘亦不入语料）"
    );

    // `doc` 指向的既有文件也不入语料（散落 md 非派生资产）
    temp.write("guides/nowhere-auth.md", "# auth 指南\n");
    let again = search(
        &graph_of(vec![{
            let mut element = element("App.Auth");
            element.path = vec!["./src/auth/".to_owned()];
            element.doc = Some("guides/nowhere-auth.md".to_owned());
            element
        }]),
        temp.root(),
        "auth",
        SearchScope::Doc,
    )
    .expect("检索成功");
    assert_eq!(again.doc, with_field.doc, "`doc` 指向的 md 不入语料");
}
