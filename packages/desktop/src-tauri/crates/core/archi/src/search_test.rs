//! `search` 的单元测试：分词（驼峰 / 数字 / 大小写）、多词 AND、匹配对象面
//! （FQN / `summary` / `metadata` 值 / 文档路径与文件名）、不搜正文、不碰规约语料
//! 与三 scope 收窄。
//!
//! 零 mock：真实临时目录 + 真实 `ModelGraph` / `docs::all_docs` 组合（检索面 =
//! 索引来源之和，替身会抹掉来源口径）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::docs;
use crate::dto::{DocRef, ElementKind, GateTier, SearchScope};
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
        path: Vec::new(),
        docs: Vec::new(),
        summary: None,
        metadata: BTreeMap::new(),
        file: ".dev-team/architecture/models/01-core.toml".to_owned(),
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
        .docs
        .into_iter()
        .map(|doc| doc.path)
        .collect()
}

// ---------------------------------------------------------------------------
// 正向：命中面与稳定序
// ---------------------------------------------------------------------------

/// 正向：`query = "auth"`、`scope = All` → 命中 FQN 含 `auth` 的元素（薄条目由
/// dto 面组装）+ 文件名含 `auth` 的文档（四字段条目）；元素按 FQN、文档按「来源
/// 强度 → `path`」稳定序。
#[test]
fn 检索命中元素与文档并按稳定序返回() {
    let temp = TempDir::new("all-scope");
    temp.write("src/auth/README.md", "# 认证\n");
    temp.write("src/billing/README.md", "# 计费\n");
    let mut auth = element("App.Auth");
    auth.docs = vec!["./src/auth/README.md".to_owned()];
    let mut billing = element("App.Billing");
    billing.docs = vec!["./src/billing/README.md".to_owned()];
    let graph = graph_of(vec![billing, auth]);

    let outcome = search(&graph, temp.root(), "auth", SearchScope::All).expect("检索成功");
    assert_eq!(
        outcome.element_fqns,
        vec!["App.Auth"],
        "只命中 FQN 含 auth 的元素且按字典序"
    );
    assert_eq!(
        outcome
            .docs
            .iter()
            .map(|doc| doc.path.as_str())
            .collect::<Vec<_>>(),
        vec!["src/auth/README.md"],
        "只命中路径 / 文件名含 auth 的文档"
    );
    // 四字段条目（无正文）
    let doc: &DocRef = &outcome.docs[0];
    assert_eq!(doc.title, "认证");
    assert_eq!(doc.source, crate::dto::DocSource::Explicit);
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

/// 正向：匹配对象面——`summary` 命中、`metadata` **值**命中（键名不算）、文档
/// `path` 段命中、文件名命中各一条。
#[test]
fn 匹配对象面覆盖summary与metadata值与文档路径与文件名() {
    let temp = TempDir::new("haystack");
    temp.write("docs/rate-limit.md", "# 限流\n");
    let mut target = element("App.Auth");
    target.summary = Some("令牌收口与限流".to_owned());
    target.metadata = BTreeMap::from([
        ("owner".to_owned(), "billing-team".to_owned()),
        ("secretkey".to_owned(), "zzz".to_owned()),
    ]);
    target.docs = vec!["./docs/rate-limit.md".to_owned()];
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
    // 文档路径段
    assert_eq!(
        doc_hits(&graph, temp.root(), "docs"),
        vec!["docs/rate-limit.md"],
        "文档 path 段参与匹配"
    );
    // 文档文件名（去扩展名）
    assert_eq!(
        doc_hits(&graph, temp.root(), "rate limit"),
        vec!["docs/rate-limit.md"],
        "文件名分词参与匹配"
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
    target.docs = vec!["./docs/guide.md".to_owned()];
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

/// 异常：`openspec/specs/` 不参与——该目录内文件名与正文均含关键词，而元素与
/// 其他文档亦命中 → 结果只来自元素与他人文档路径，`specs` 下路径零出现。
#[test]
fn 规约语料目录不参与检索() {
    let temp = TempDir::new("specs-excluded");
    temp.write(
        "openspec/specs/desktop-archi-model/spec.md",
        "# archi 能力规约\n\n涉及 App.Auth 与 auth 的说明。\n",
    );
    temp.write("docs/auth.md", "# 认证\n");
    let mut target = element("App.Auth");
    target.docs = vec!["./docs/auth.md".to_owned()];
    let graph = graph_of(vec![target]);

    let docs_hit = doc_hits(&graph, temp.root(), "auth");
    assert!(
        docs_hit.iter().all(|path| !path.contains("openspec/specs")),
        "specs 下路径零出现: {docs_hit:?}"
    );
    assert_eq!(docs_hit, vec!["docs/auth.md"], "只来自元素关联文档");
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

/// 边界：scope 收窄——`Elements` 时 `docs` 恒空数组；`Docs` 时 `element_fqns` 恒
/// 空数组；`All` 两数组均可非空。
#[test]
fn scope三值收窄语义() {
    let temp = TempDir::new("scope");
    temp.write("docs/auth.md", "# 认证\n");
    let mut target = element("App.Auth");
    target.docs = vec!["./docs/auth.md".to_owned()];
    let graph = graph_of(vec![target]);

    let elements_only = search(&graph, temp.root(), "auth", SearchScope::Elements).expect("成功");
    assert_eq!(elements_only.element_fqns, vec!["App.Auth"]);
    assert!(elements_only.docs.is_empty(), "Elements 时 docs 恒空");

    let docs_only = search(&graph, temp.root(), "auth", SearchScope::Docs).expect("成功");
    assert!(
        docs_only.element_fqns.is_empty(),
        "Docs 时 element_fqns 恒空"
    );
    assert_eq!(docs_only.docs.len(), 1);

    let all = search(&graph, temp.root(), "auth", SearchScope::All).expect("成功");
    assert!(
        !all.element_fqns.is_empty() && !all.docs.is_empty(),
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
    assert!(outcome.docs.is_empty());

    let empty = graph_of(Vec::new());
    let outcome = search(&empty, temp.root(), "auth", SearchScope::All).expect("空模型仍 Ok");
    assert!(outcome.element_fqns.is_empty() && outcome.docs.is_empty());
}

/// 边界：`str` / 规模面——超长 query（>1000 chars）正常返回；500 元素 / 200 文档
/// → 结果序稳定、两次调用逐字相同。
#[test]
fn 超长query与规模面结果稳定() {
    let temp = TempDir::new("scale");
    for index in 0..200 {
        temp.write(&format!("docs/note{index:03}.md"), "# 文档\n");
    }
    let mut elements: Vec<Element> = Vec::new();
    for index in 0..500 {
        let mut item = element(&format!("App.Auth.Mod{index:03}"));
        if index < 200 {
            item.docs = vec![format!("./docs/note{index:03}.md")];
        }
        elements.push(item);
    }
    let graph = graph_of(elements);

    let long = format!("auth{}", "x".repeat(1200));
    // 超长单段不与任何 FQN 匹配，但必须正常返回（不截断、不 panic）
    let outcome =
        search(&graph, temp.root(), &long, SearchScope::All).expect("超长 query 正常返回");
    assert!(outcome.element_fqns.is_empty(), "超长段无命中");

    let first = search(&graph, temp.root(), "auth", SearchScope::All).expect("成功");
    let second = search(&graph, temp.root(), "auth", SearchScope::All).expect("成功");
    assert_eq!(first, second, "两次调用结果逐字相同");
    assert_eq!(first.element_fqns.len(), 500, "500 元素全部命中 auth");
    assert!(first.docs.is_empty(), "无文档路径含 auth");

    // 文档侧：文件名命中 200 条，序为「来源强度 → 路径」稳定序
    let docs_hit = search(&graph, temp.root(), "note", SearchScope::All).expect("成功");
    assert!(docs_hit.element_fqns.is_empty(), "元素 FQN 不含 note");
    let paths: Vec<&str> = docs_hit.docs.iter().map(|doc| doc.path.as_str()).collect();
    assert_eq!(paths.len(), 200, "200 文档按文件名命中 note");
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted, "文档命中按路径字典序");
}

/// 边界：`docs::all_docs` 与检索同源——检索命中文档即 `all_docs` 的子集（无第二
/// 事实源）。
#[test]
fn 检索文档面为关联面的子集() {
    let temp = TempDir::new("same-source");
    temp.write("src/auth/README.md", "# 认证\n");
    let mut target = element("App.Auth");
    target.path = vec!["./src/auth/".to_owned()];
    let graph = graph_of(vec![target]);

    let all = docs::all_docs(&graph, temp.root());
    let outcome = search(&graph, temp.root(), "auth", SearchScope::Docs).expect("成功");
    for doc in &outcome.docs {
        assert!(
            all.docs.iter().any(|candidate| candidate.path == doc.path),
            "命中条目 {} 应来自关联面",
            doc.path
        );
    }
}
