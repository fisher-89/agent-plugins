//! `docs` 的单元测试：三源关联（显式 `docs` / 邻近 `path` 扫描 / ADR 正文命中）、
//! 强度排序去重、`docNotFound` 只报不拦、邻近扫描排除与不越界，以及 `all_docs`
//! 并集语义。
//!
//! 零 mock：真实临时目录（含排除目录 / 决策记录目录）、真实 `ignore::WalkBuilder`
//! 与真实 `foundation::layout` 派生——gitignore 与剪枝即被测语义。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_decisions_dir;

use crate::dto::{DocKind, DocSource, ElementKind, GateTier};
use crate::graph::ModelGraph;
use crate::model::{Element, Model};

use crate::docs::{all_docs, element_docs, DocsOutcome};

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-docs-test-{}-{}-{}",
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

    /// 落一个 repo 相对文件（自动建父目录）。
    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }

    /// 落一个决策记录文件。
    fn write_decision(&self, name: &str, content: &str) {
        fs::create_dir_all(archi_decisions_dir(&self.0)).expect("创建决策记录目录失败");
        fs::write(archi_decisions_dir(&self.0).join(name), content).expect("写决策记录失败");
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
        file: "openspec/architecture/models/01-core.toml".to_owned(),
        line: 1,
    }
}

/// 单元素图（借 `ModelGraph::build` 造出可查询的模型）。
fn graph_of(elements: Vec<Element>) -> ModelGraph {
    ModelGraph::build(Model {
        elements,
        relations: Vec::new(),
    })
}

/// 关联条目的路径清单（断言用）。
fn paths(outcome: &DocsOutcome) -> Vec<&str> {
    outcome.docs.iter().map(|doc| doc.path.as_str()).collect()
}

// ---------------------------------------------------------------------------
// 正向：三源各自命中 + 强度序
// ---------------------------------------------------------------------------

/// 正向：`explicit` 命中——`path` 为 repo 相对 `/` 分隔、`title` 取首行 `# 标题`、
/// `kind` 为 `md`。
#[test]
fn explicit命中返回path标题与形态() {
    let temp = TempDir::new("explicit");
    temp.write("src/auth/README.md", "# 认证说明\n\n正文\n");
    let mut target = element("App.Auth");
    target.docs = vec!["./src/auth/README.md".to_owned()];
    let graph = graph_of(vec![target]);

    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert_eq!(outcome.docs.len(), 1);
    let doc = &outcome.docs[0];
    assert_eq!(doc.path, "src/auth/README.md", "repo 相对、/ 分隔、剥 ./");
    assert_eq!(doc.title, "认证说明", "标题取首行 ATX 标题");
    assert_eq!(doc.kind, DocKind::Md);
    assert_eq!(doc.source, DocSource::Explicit);
    assert!(outcome.warnings.is_empty(), "命中即零警告");
}

/// 正向：`adjacent` 命中——目录内两个 `*.md` 各一条；`path` 指向单个文件时按精确
/// 匹配、不经递归（同目录其他 md 不入）。
#[test]
fn adjacent目录扫描与单文件精确匹配() {
    let temp = TempDir::new("adjacent");
    temp.write("src/auth/README.md", "# 认证\n");
    temp.write("src/auth/TOKEN.md", "# 令牌\n");
    temp.write("src/auth/notes.txt", "不是 md，不参与\n");

    let mut dir_target = element("App.Auth");
    dir_target.path = vec!["./src/auth/".to_owned()];
    let graph = graph_of(vec![dir_target]);
    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert_eq!(
        paths(&outcome),
        vec!["src/auth/README.md", "src/auth/TOKEN.md"],
        "目录内 *.md 全量、按路径字典序、.txt 不参与"
    );
    assert!(
        outcome
            .docs
            .iter()
            .all(|doc| doc.source == DocSource::Adjacent),
        "来源标记 adjacent"
    );

    // 单文件形态：精确匹配，不递归同目录
    let mut file_target = element("App.Auth");
    file_target.path = vec!["./src/auth/README.md".to_owned()];
    let graph = graph_of(vec![file_target]);
    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert_eq!(
        paths(&outcome),
        vec!["src/auth/README.md"],
        "单文件形态只取该文件（TOKEN.md 不入）"
    );
}

/// 正向：`text` 命中——决策记录正文含元素 FQN → 一条 `kind: adr`、`source: text`。
#[test]
fn text命中决策记录返回adr形态() {
    let temp = TempDir::new("text");
    temp.write_decision(
        "0007-token-sink.md",
        "# 令牌收口\n\n本决策涉及 App.Auth 的令牌下发。\n",
    );
    temp.write_decision("0008-unrelated.md", "# 无关决策\n\n本次不涉及任何元素。\n");
    let graph = graph_of(vec![element("App.Auth")]);

    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert_eq!(outcome.docs.len(), 1, "只有提及该 FQN 的 ADR 命中");
    let doc = &outcome.docs[0];
    assert_eq!(
        doc.path,
        "openspec/architecture/decisions/0007-token-sink.md"
    );
    assert_eq!(doc.kind, DocKind::Adr, "决策记录目录内恒 adr");
    assert_eq!(doc.source, DocSource::Text);
    assert_eq!(doc.title, "令牌收口", "标题同样取首行标题");
}

/// 正向：三源同时命中 → 次序 `explicit` → `adjacent` → `text`；每条严格只含四
/// 字段（无正文 / 摘要 / 行区间）。
#[test]
fn 三源同时命中按强度排序且条目仅四字段() {
    let temp = TempDir::new("three-sources");
    temp.write("src/auth/README.md", "# 认证\n");
    temp.write("src/auth/guide.md", "# 指南\n");
    temp.write("decisions-shaped/ADR.md", "# 决策\n");
    temp.write_decision("0010-auth.md", "涉及 App.Auth\n");

    let mut target = element("App.Auth");
    target.docs = vec!["./src/auth/README.md".to_owned()];
    target.path = vec!["./src/auth/".to_owned()];
    let graph = graph_of(vec![target]);

    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    let sources: Vec<DocSource> = outcome.docs.iter().map(|doc| doc.source).collect();
    let mut sorted = sources.clone();
    sorted.sort();
    assert_eq!(
        sources, sorted,
        "次序按来源强度：explicit → adjacent → text"
    );
    assert_eq!(
        paths(&outcome),
        vec![
            "src/auth/README.md",
            "src/auth/guide.md",
            "openspec/architecture/decisions/0010-auth.md",
        ],
        "强度序内按路径字典序"
    );

    // 四字段形状：序列化键集恰为 {path,title,kind,source}（无正文 / 摘要 / 行区间）
    let value = serde_json::to_value(&outcome.docs[0]).expect("序列化成功");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("条目为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["kind", "path", "source", "title"], "恰四字段");
}

// ---------------------------------------------------------------------------
// 异常：docNotFound 只报不拦
// ---------------------------------------------------------------------------

/// 异常：`docs` 指向不存在的文件 → `warnings` 一条（含元素与路径），`docs` 不含
/// 该条；不产生结构诊断（缺失不影响停摆——本模块不产诊断，仅警告）。
#[test]
fn 文档缺失只报警告不入docs() {
    let temp = TempDir::new("missing-doc");
    let mut target = element("App.Auth");
    target.docs = vec!["./src/auth/MISSING.md".to_owned()];
    let graph = graph_of(vec![target]);

    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert!(outcome.docs.is_empty(), "缺失条目不进 docs");
    assert_eq!(outcome.warnings.len(), 1, "恰一条警告");
    assert_eq!(outcome.warnings[0].element, "App.Auth");
    assert_eq!(outcome.warnings[0].path, "src/auth/MISSING.md");
}

/// 异常：读盘不可用（`docs` 指向一个**目录**）→ 一条 warning、`docs` 不含、
/// 不 panic（D18：读取失败按 docNotFound 处置）。
#[test]
fn 指向目录按文档缺失处置() {
    let temp = TempDir::new("dir-as-doc");
    temp.write("src/auth/README.md", "# 认证\n");
    let mut target = element("App.Auth");
    target.docs = vec!["./src/auth/".to_owned()];
    let graph = graph_of(vec![target]);

    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert!(outcome.docs.is_empty(), "目录不是文档，不入 docs");
    assert_eq!(outcome.warnings.len(), 1, "按 docNotFound 记一条警告");
    assert_eq!(outcome.warnings[0].path, "src/auth", "尾分隔符被归一掉");
}

/// 异常：邻近扫描排除——测试 / 构建 / VCS 目录与架构域目录内的 md 均不入；非 md
/// 不参与。
#[test]
fn 邻近扫描排除内置目录与架构域目录() {
    let temp = TempDir::new("adjacent-exclude");
    temp.write("src/auth/README.md", "# 认证\n");
    temp.write("src/auth/__tests__/fixture.md", "# 测试夹具\n");
    temp.write("src/auth/tests/x.md", "# 测试\n");
    temp.write("src/auth/node_modules/a.md", "# 依赖\n");
    temp.write("src/auth/target/a.md", "# 构建产物\n");
    temp.write("src/auth/.git/a.md", "# VCS\n");
    temp.write("src/auth/dist/a.md", "# 产物\n");
    temp.write("src/auth/notes.txt", "非 md\n");

    let mut target = element("App.Auth");
    target.path = vec!["./src/auth/".to_owned()];
    let graph = graph_of(vec![target]);
    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert_eq!(
        paths(&outcome),
        vec!["src/auth/README.md"],
        "测试 / 构建 / VCS 目录与非 md 均不参与"
    );

    // 架构域目录内的 md 不入（`is_archi_rel_path` 判定）
    temp.write("openspec/architecture/models/notes.md", "# 模型备注\n");
    temp.write("openspec/decisions-shaped/a.md", "# 域根下非架构域\n");
    let mut archi_scope = element("App.Auth");
    archi_scope.path = vec!["./openspec/".to_owned()];
    let graph = graph_of(vec![archi_scope]);
    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert_eq!(
        paths(&outcome),
        vec!["openspec/decisions-shaped/a.md"],
        "架构域目录内的 md 被排除，域根其他目录照常"
    );
}

/// 异常：扫描不越界——元素 `path` 只覆盖 `src/auth/`，`src/order/README.md` 不入。
#[test]
fn 邻近扫描不越出元素path范围() {
    let temp = TempDir::new("no-overreach");
    temp.write("src/auth/README.md", "# 认证\n");
    temp.write("src/order/README.md", "# 订单\n");

    let mut target = element("App.Auth");
    target.path = vec!["./src/auth/".to_owned()];
    let graph = graph_of(vec![target]);
    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert_eq!(
        paths(&outcome),
        vec!["src/auth/README.md"],
        "不扫全仓（越界文件不出现）"
    );
}

// ---------------------------------------------------------------------------
// 边界：同文件合并 / 稳定序 / 空关联 / 标题推导 / 列表边界 / str 面
// ---------------------------------------------------------------------------

/// 边界：同文件双源合并取强——`README.md` 既在 `docs` 又在 `path` 目录内 →
/// 恰一条且 `source = explicit`。
#[test]
fn 同文件双源合并取最强来源() {
    let temp = TempDir::new("merge-sources");
    temp.write("src/auth/README.md", "# 认证\n");
    temp.write("src/auth/other.md", "# 其他\n");

    let mut target = element("App.Auth");
    target.docs = vec!["./src/auth/README.md".to_owned()];
    target.path = vec!["./src/auth/".to_owned()];
    let graph = graph_of(vec![target]);

    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    let merged = outcome
        .docs
        .iter()
        .filter(|doc| doc.path == "src/auth/README.md")
        .collect::<Vec<_>>();
    assert_eq!(merged.len(), 1, "同路径只出现一次");
    assert_eq!(merged[0].source, DocSource::Explicit, "取最强来源");
}

/// 边界：同强度稳定序（按 `path` 字典序）且连续两次调用结果逐字相同。
#[test]
fn 同强度条目按路径字典序且调用幂等() {
    let temp = TempDir::new("stable-order");
    for name in ["c.md", "a.md", "b.md"] {
        temp.write(&format!("src/auth/{name}"), "# 文档\n");
    }
    let mut target = element("App.Auth");
    target.path = vec!["./src/auth/".to_owned()];
    let graph = graph_of(vec![target]);

    let first = element_docs(&graph, temp.root(), "App.Auth");
    let second = element_docs(&graph, temp.root(), "App.Auth");
    assert_eq!(first, second, "连续两次调用结果逐字相同");
    assert_eq!(
        paths(&first),
        vec!["src/auth/a.md", "src/auth/b.md", "src/auth/c.md"],
        "按路径字典序稳定"
    );
}

/// 边界：无任何关联（无 `docs`、无 `path`、无 ADR 提及）→ `docs: []` 且
/// `warnings: []`（不告警）。
#[test]
fn 无关联不告警() {
    let temp = TempDir::new("no-link");
    let graph = graph_of(vec![element("App.Auth")]);
    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    assert!(outcome.docs.is_empty());
    assert!(outcome.warnings.is_empty(), "无关联面不产生警告");
}

/// 边界：`title` 推导四形态——首行标题 / 无标题正文 / 空文件 / 首行为代码围栏。
#[test]
fn 标题推导四形态() {
    let temp = TempDir::new("title");
    temp.write("src/titled.md", "\n\n## 真正的标题\n\n正文\n");
    temp.write("src/plain.md", "只有正文，没有标题\n");
    temp.write("src/empty.md", "");
    temp.write("src/fenced.md", "```rust\nfn main() {}\n```\n");
    let mut target = element("App.Auth");
    target.docs = vec![
        "./src/titled.md".to_owned(),
        "./src/plain.md".to_owned(),
        "./src/empty.md".to_owned(),
        "./src/fenced.md".to_owned(),
    ];
    let graph = graph_of(vec![target]);

    let outcome = element_docs(&graph, temp.root(), "App.Auth");
    let titles: Vec<(&str, &str)> = outcome
        .docs
        .iter()
        .map(|doc| (doc.path.as_str(), doc.title.as_str()))
        .collect();
    assert_eq!(
        titles,
        vec![
            ("src/empty.md", "empty"),
            ("src/fenced.md", "fenced"),
            ("src/plain.md", "plain"),
            ("src/titled.md", "真正的标题"),
        ],
        "有标题取标题文本，否则退回文件名去扩展名"
    );
}

/// 边界：列表边界——`path` / `docs` 为 `[]` / 单元素 / 200 元素 / 同路径重复两项 →
/// 去重后逐条判定、不 panic。
#[test]
fn 列表边界与同路径去重() {
    let temp = TempDir::new("list-bounds");
    temp.write("src/auth/README.md", "# 认证\n");
    for index in 0..200 {
        temp.write(&format!("src/auth/doc{index:03}.md"), "# 文档\n");
    }

    // 空列表
    let empty = element_docs(
        &graph_of(vec![element("App.Auth")]),
        temp.root(),
        "App.Auth",
    );
    assert!(empty.docs.is_empty() && empty.warnings.is_empty());

    // 同路径重复两项 → 去重后恰一条
    let mut duplicated = element("App.Auth");
    duplicated.docs = vec![
        "./src/auth/README.md".to_owned(),
        "src/auth/README.md".to_owned(),
    ];
    let outcome = element_docs(&graph_of(vec![duplicated]), temp.root(), "App.Auth");
    assert_eq!(outcome.docs.len(), 1, "同路径两项合并为一条");

    // 200 项 docs
    let mut many = element("App.Auth");
    many.docs = (0..200)
        .map(|index| format!("./src/auth/doc{index:03}.md"))
        .collect();
    let outcome = element_docs(&graph_of(vec![many]), temp.root(), "App.Auth");
    assert_eq!(outcome.docs.len(), 200, "200 项全量关联");
    assert!(outcome.warnings.is_empty());

    // 200 项 path（同目录重复声明）→ 结果仍去重
    let mut many_path = element("App.Auth");
    many_path.path = (0..200).map(|_| "./src/auth/".to_owned()).collect();
    let outcome = element_docs(&graph_of(vec![many_path]), temp.root(), "App.Auth");
    assert_eq!(
        outcome.docs.len(),
        201,
        "目录内 201 个 md（README + 200）去重后各一条"
    );
}

/// 边界：`str` 面——`fqn` 为 `""` / 超长 / 含 emoji → `docs` 空、`warnings` 空、
/// 不 panic；路径含中文 / emoji / 空格 → 归一 `/` 并逐字返回。
#[test]
fn 特殊字符fqn与路径逐字处理() {
    let temp = TempDir::new("exotic");
    let long = "X".repeat(1200);
    for fqn in ["", "App.🚀Auth", long.as_str()] {
        let outcome = element_docs(&graph_of(vec![element("App.Auth")]), temp.root(), fqn);
        assert!(outcome.docs.is_empty(), "不存在的 fqn 无关联: {fqn:?}");
        assert!(outcome.warnings.is_empty(), "不存在的 fqn 不告警");
    }

    temp.write("文档 目录/认证 说明.md", "# 中文标题\n");
    let mut target = element("App.Auth");
    target.docs = vec!["./文档 目录/认证 说明.md".to_owned()];
    let outcome = element_docs(&graph_of(vec![target]), temp.root(), "App.Auth");
    assert_eq!(
        paths(&outcome),
        vec!["文档 目录/认证 说明.md"],
        "中文 / 空格路径逐字保留"
    );

    // 反斜杠形态归一到 `/`
    let mut backslash = element("App.Auth");
    backslash.docs = vec![".\\文档 目录\\认证 说明.md".to_owned()];
    let outcome = element_docs(&graph_of(vec![backslash]), temp.root(), "App.Auth");
    assert_eq!(
        paths(&outcome),
        vec!["文档 目录/认证 说明.md"],
        "反斜杠归一为 /"
    );

    // `..` 段跳过（不引入越界读取面）
    let mut escaping = element("App.Auth");
    escaping.docs = vec!["../etc/passwd.md".to_owned()];
    let outcome = element_docs(&graph_of(vec![escaping]), temp.root(), "App.Auth");
    assert!(outcome.docs.is_empty(), "`..` 段条目被跳过");
    assert!(outcome.warnings.is_empty(), "跳过而非告警");
}

// ---------------------------------------------------------------------------
// `all_docs`
// ---------------------------------------------------------------------------

/// 正向：3 元素各带 explicit / adjacent / text 命中 → 并集按路径字典序；同路径多
/// 元素命中只出现一次（身份键 = `path`）。
#[test]
fn all_docs并集去重且按路径字典序() {
    let temp = TempDir::new("all-docs");
    temp.write("src/auth/README.md", "# 认证\n");
    temp.write("src/billing/README.md", "# 计费\n");
    temp.write("shared/COMMON.md", "# 公共\n");
    temp.write_decision("0011-auth.md", "涉及 App.Auth\n");

    let mut auth = element("App.Auth");
    auth.docs = vec!["./shared/COMMON.md".to_owned()];
    auth.path = vec!["./src/auth/".to_owned()];
    let mut billing = element("App.Billing");
    billing.path = vec!["./src/billing/".to_owned()];
    billing.docs = vec!["./shared/COMMON.md".to_owned()];
    let mut token = element("App.Auth.Token");
    token.docs = vec!["./src/auth/README.md".to_owned()];

    let graph = graph_of(vec![auth, billing, token]);
    let outcome = all_docs(&graph, temp.root());
    assert_eq!(
        paths(&outcome),
        vec![
            "openspec/architecture/decisions/0011-auth.md",
            "shared/COMMON.md",
            "src/auth/README.md",
            "src/billing/README.md",
        ],
        "并集按路径字典序，同路径只出现一次"
    );
    let unique: std::collections::BTreeSet<&str> =
        outcome.docs.iter().map(|doc| doc.path.as_str()).collect();
    assert_eq!(unique.len(), outcome.docs.len(), "身份键 = path 零重复");
}

/// 边界：空模型 → 空 `docs` + 空 `warnings`；200 元素 + 3 份 ADR → 结果正确且无
/// 重复条目、文件字节不变（零写入副作用）。
#[test]
fn all_docs空模型与大模型边界() {
    let temp = TempDir::new("all-docs-bounds");
    let empty = all_docs(&graph_of(Vec::new()), temp.root());
    assert!(empty.docs.is_empty());
    assert!(empty.warnings.is_empty());

    temp.write("src/auth/README.md", "# 认证\n");
    for index in 0..3 {
        temp.write_decision(&format!("00{index}-adr.md"), "涉及 App.Auth\n");
    }
    let mut elements: Vec<Element> = vec![element("App.Auth")];
    for index in 0..199 {
        let mut item = element(&format!("App.Mod{index:03}"));
        item.docs = vec!["./src/auth/README.md".to_owned()];
        elements.push(item);
    }
    // 200 元素共享同一份显式文档 + 3 份 ADR 同时提及 App.Auth（重复命中面）
    let graph = graph_of(elements);
    let before = fs::read(archi_decisions_dir(temp.root()).join("000-adr.md")).expect("读 ADR");
    let outcome = all_docs(&graph, temp.root());
    let after = fs::read(archi_decisions_dir(temp.root()).join("000-adr.md")).expect("读 ADR");
    assert_eq!(before, after, "决策记录只读，正文字节不变");

    assert_eq!(
        paths(&outcome),
        vec![
            "openspec/architecture/decisions/000-adr.md",
            "openspec/architecture/decisions/001-adr.md",
            "openspec/architecture/decisions/002-adr.md",
            "src/auth/README.md",
        ],
        "200 元素 × 4 来源重复命中后去重为 4 条"
    );
    assert!(outcome.warnings.is_empty());
}

/// 边界：无跨元素反查——`DocRef` 四字段不含元素 FQN（spec 明令不提供反向索引）。
#[test]
fn docref不含元素fqn字段() {
    let temp = TempDir::new("no-reverse-index");
    temp.write("src/auth/README.md", "# 认证\n");
    let mut target = element("App.Auth");
    target.docs = vec!["./src/auth/README.md".to_owned()];
    let outcome = element_docs(&graph_of(vec![target]), temp.root(), "App.Auth");

    let value = serde_json::to_value(&outcome.docs[0]).expect("序列化成功");
    let object = value.as_object().expect("条目为对象");
    for forbidden in ["element", "elementFqn", "fqn", "elements"] {
        assert!(
            !object.contains_key(forbidden),
            "DocRef 不含反向索引字段 {forbidden}"
        );
    }
    let serialized = serde_json::to_string(&outcome.docs[0]).expect("序列化成功");
    assert!(
        !serialized.contains("App.Auth"),
        "条目内不出现元素 FQN: {serialized}"
    );
}
