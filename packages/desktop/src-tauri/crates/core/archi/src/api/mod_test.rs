//! `api`（装配层）的单元测试：契约面启用判定（递归扫 `*.md`、深度上限、符号链接不跟随）、
//! `load_contracts` 全链（解析 → 装配 → 双向 gate → `$defs` 规则 → 取数）、
//! `render_all` 四类生成物 / 幂等 / `INDEX` 覆盖与 `categories` 形状分布、`check` 的未启用
//! 短路与稳定排序、`bootstrap` 的草稿形态与零写盘、`api_ref` 的双层投影口径、`ApiError`
//! 三值文案。
//!
//! 零 mock：自建 `TempDir` RAII + `AtomicU64`（本 crate 无 dev-dependency）、真实
//! `foundation::layout` 派生、真实解析 / 装配 / 判据 / 渲染 / 取数组合——编排语义即被测
//! 对象，替身会引入第二事实源。
//!
//! IO 错误面以「非法 UTF-8 字节的契约文件」注入，不改权限；「缺 md」不进任何清单
//! （缺口由判据 ② 从模型意图与 `contracts` 的差集直接得出）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_api_dir;
use serde_json::Value;

use crate::api::contract::parse;
use crate::api::extract::{self, Visibility};
use crate::api::paths;
use crate::dto::{ApiStatus, ArchiElement, ElementKind, GateTier, NodeMark};

use super::{
    api_ref, bootstrap, check, enabled, load_contracts, meta, render_all, ApiCheckInput, ApiError,
    ApiSnapshot,
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
            "archi-api-mod-test-{}-{}-{}",
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

    /// 落模型（`.dev-team/architecture/models/01-test.toml`）。
    ///
    /// `content` 只写叶元素；祖先链由 [`declare_ancestors`] 自动补齐——`elements_of`
    /// 经 `dto::graph_view` 取数，模型缺祖先即 `orphanHierarchy` 结构错（fail-closed），
    /// 而本文件的历史夹具清一色只声明叶元素。
    fn write_model(&self, content: &str) {
        self.write(
            ".dev-team/architecture/models/01-test.toml",
            &declare_ancestors(content),
        );
    }

    /// 落契约（`<archi_api_dir>/<Domain>/<FQN>.md`）。
    fn write_contract(&self, fqn: &str, content: &str) {
        let path = paths::contract_path(self.root(), fqn).expect("测试 FQN 恒为合法形态");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建契约目录失败");
        }
        fs::write(&path, content).expect("写契约失败");
    }

    /// 全量字节快照（零写盘对拍）。
    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            let Ok(read) = fs::read_dir(dir) else {
                return;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, out);
                    continue;
                }
                let relative = path
                    .strip_prefix(base)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, fs::read(&path).expect("读文件失败")));
            }
        }
        let mut entries = Vec::new();
        walk(&self.0, &self.0, &mut entries);
        entries.sort();
        entries
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 给模型源文补齐祖先元素声明（缺者按 `package` / `domain` 补，已有者不重声明）。
///
/// 契约面的元素表经 `dto::graph_view` 取数（单点供给），而模型纪律要求「子元素已声明 ⇒
/// 祖先须显式声明」——缺祖先即 `orphanHierarchy` 结构诊断，契约面随即 fail-closed。历史
/// 夹具清一色只写叶元素，故统一在落盘前补齐祖先链，而不是让被测函数对坏模型宽容（那会
/// 让「坏模型 ⇒ 不生成」这条判据永远测不到）。兄弟 / 后代测试模块经
/// `crate::api::mod_test::declare_ancestors` 共用同一装置。
pub(crate) fn declare_ancestors(model: &str) -> String {
    use std::collections::BTreeSet;

    let mut declared: BTreeSet<String> = BTreeSet::new();
    for line in model.lines() {
        if let Some(rest) = line.trim().strip_prefix("[element.\"") {
            if let Some(fqn) = rest.strip_suffix("\"]") {
                declared.insert(fqn.to_owned());
            }
        }
    }
    let mut missing: BTreeSet<String> = BTreeSet::new();
    for fqn in &declared {
        let segments: Vec<&str> = fqn.split('.').collect();
        for depth in 1..segments.len() {
            let ancestor = segments[..depth].join(".");
            if !declared.contains(&ancestor) {
                missing.insert(ancestor);
            }
        }
    }
    let mut output = model.to_owned();
    // 浅层在前（`App` → `App.Backend` → …），声明序与仓库模型一致。
    let mut ordered: Vec<String> = missing.into_iter().collect();
    ordered.sort_by_key(|fqn| (fqn.matches('.').count(), fqn.clone()));
    for fqn in ordered {
        let kind = if fqn.contains('.') {
            "domain"
        } else {
            "package"
        };
        output = format!(
            "{output}\n[element.\"{fqn}\"]\nkind = \"{kind}\"\ngate = \"off\"\npath = []\n"
        );
    }
    output
}

/// 元素构造器（`path` 原文直传）。
fn element(fqn: &str, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Warn,
        api: ApiStatus::Declared,
        path: paths.iter().map(|path| (*path).to_owned()).collect(),
        summary: None,
        metadata: BTreeMap::new(),
        model_file: ".dev-team/architecture/models/01-test.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
    }
}

/// 试点形态契约源文（H1 = `fqn`，1 op `file_stem`，1 类型 `LocalArchiCheck`）。
fn pilot_markdown(fqn: &str) -> String {
    format!(
        "# {fqn}\n\n## 职能\n\n门禁执行体。\n\n## 不变量\n\n写盘失败不阻断结论。\n\n\
         ## 操作\n\n### file_stem\n\n```json api-op\n{{\"owner\": \"LocalArchiCheck\", \
         \"params\": [{{\"name\": \"timestamp\", \"schema\": {{\"type\": \"string\"}}}}], \
         \"returns\": {{\"type\": \"string\"}}}}\n```\n\n## 类型\n\n```json api-types\n\
         {{\"LocalArchiCheck\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    )
}

/// 空快照（`render_all` 的空输入）。
fn empty_snapshot() -> ApiSnapshot {
    ApiSnapshot {
        contracts: Vec::new(),
        schemas: BTreeMap::new(),
        symbols: BTreeMap::new(),
    }
}

// ---------------------------------------------------------------------------
// enabled：递归扫描与启用条件
// ---------------------------------------------------------------------------

/// 正向（AC-7 前置）：`<archi_api_dir>/Infra/App.X.md` → `true`；同一棵树只有
/// `INDEX.toml` / `*.schema.json` 而无 `*.md` → `false`。
#[test]
fn enabled递归扫描到md() {
    let temp = TempDir::new("enabled-recursive");
    temp.write(".dev-team/architecture/api/Infra/App.X.md", "# App.X\n");
    assert!(enabled(temp.root()), "递归含 md 即启用");

    let only_index = TempDir::new("enabled-no-md");
    only_index.write(".dev-team/architecture/api/INDEX.toml", "# 空索引\n");
    only_index.write(
        ".dev-team/architecture/api/Infra/App.X.schema.json",
        "{\"$defs\": {}}\n",
    );
    assert!(
        !enabled(only_index.root()),
        "只有 INDEX 与 schema 而无 md → 未启用（判据只看 *.md）"
    );
}

/// 边界（D17）：根下无 `.dev-team` → `false`（零 panic）；契约目录存在但为空 → `false`；
/// 两层深（`<Domain>/<Sub>/x.md`）→ `true`。
#[test]
fn enabled目录缺席与空目录() {
    let bare = TempDir::new("enabled-bare");
    assert!(!enabled(bare.root()), "无 .dev-team → false（零 panic）");

    let empty = TempDir::new("enabled-empty");
    fs::create_dir_all(archi_api_dir(empty.root())).expect("建空契约目录");
    assert!(!enabled(empty.root()), "空契约目录 → false");

    let nested = TempDir::new("enabled-nested");
    nested.write(
        ".dev-team/architecture/api/Infra/Sub/x.md",
        "# App.Infra.Sub.X\n",
    );
    assert!(enabled(nested.root()), "两层深仍递归命中");
}

/// 边界：`README.md` → `true`（判据只看 `*.md` 在场，命名合法性归 `load_contracts`）；
/// `x.MD` / `x.md.txt` / 同名目录 `x.md/` → `false`。
#[test]
fn enabled文件名边界() {
    let readme = TempDir::new("enabled-readme");
    readme.write(".dev-team/architecture/api/README.md", "# 说明\n");
    assert!(enabled(readme.root()), "README.md 仍算 md 在场");

    let upper = TempDir::new("enabled-upper");
    upper.write(".dev-team/architecture/api/x.MD", "# x\n");
    assert!(!enabled(upper.root()), "x.MD 大小写敏感 → false");

    let doubled = TempDir::new("enabled-doubled");
    doubled.write(".dev-team/architecture/api/x.md.txt", "# x\n");
    assert!(!enabled(doubled.root()), "x.md.txt 不以 .md 结尾 → false");

    let dir_named_md = TempDir::new("enabled-dir-md");
    fs::create_dir_all(archi_api_dir(dir_named_md.root()).join("x.md")).expect("建同名目录");
    assert!(!enabled(dir_named_md.root()), "同名目录不是文件 → false");
}

/// 边界（D17 实现纪律 / D34 同源口径）：符号链接**不跟随**（不 panic、不无限递归、
/// 不因链接指向的树而有 md 命中）；深度判据按 `MAX_API_WALK_DEPTH`（= 4）的边界两侧
/// 分别判定——扫描自 `<api 目录>` 起 depth 0，因此 `<sub>^k/x.md` 的 depth 为 k，
/// `k <= 4` 命中、`k > 4` 不命中（后者须返回 `false` 而不栈溢出）。
#[test]
fn enabled符号链接不跟随与深度上限两侧边界() {
    // 符号链接：链接指向一棵含 md 的树，但扫描 MUST NOT 跟随 → 零 md → false。
    let linked = TempDir::new("enabled-symlink");
    let target = TempDir::new("enabled-symlink-target");
    target.write("x.md", "# x\n");
    fs::create_dir_all(archi_api_dir(linked.root())).expect("建契约目录");
    let link = archi_api_dir(linked.root()).join("link");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target.root(), &link).expect("建符号链接目录");
        assert!(
            !enabled(linked.root()),
            "符号链接目录不跟随：链接内的 md 不构成启用"
        );
    }
    #[cfg(windows)]
    {
        // Windows 上目录符号链接常需特权：建不成就只断言「不 panic、结果可判」。
        if std::os::windows::fs::symlink_dir(target.root(), &link).is_ok() {
            assert!(
                !enabled(linked.root()),
                "符号链接目录不跟随：链接内的 md 不构成启用"
            );
        } else {
            let _ = enabled(linked.root());
        }
    }

    // 深度上限：`k` 段子目录（depth = k）。
    fn nested_at(tag: &str, subdirs: usize) -> TempDir {
        let temp = TempDir::new(tag);
        let mut relative = String::from(".dev-team/architecture/api");
        for _ in 0..subdirs {
            relative.push_str("/d");
        }
        temp.write(&format!("{relative}/x.md"), "# App.D\n");
        temp
    }

    let at_limit = nested_at("enabled-depth-at-limit", 4);
    assert!(
        enabled(at_limit.root()),
        "depth 4 == MAX_API_WALK_DEPTH → 仍在扫面内，命中"
    );

    let beyond_limit = nested_at("enabled-depth-beyond", 5);
    assert!(
        !enabled(beyond_limit.root()),
        "depth 5 > MAX_API_WALK_DEPTH → 不命中（返回 false 而不栈溢出）"
    );
}

// ---------------------------------------------------------------------------
// load_contracts：全链 / 缺 md / 跳过 / 路径空 / 异常面
// ---------------------------------------------------------------------------

/// 正向（AC-1 + AC-9）：单元素 `path` 非空 + md 在案 → `Ok`，`contracts` / `schemas` /
/// `symbols` 各就位，双向 gate 与 `$defs` 规则全过。
#[test]
fn load_contracts全链() {
    let temp = TempDir::new("load-chain");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("全链应成功");

    assert_eq!(snapshot.contracts.len(), 1, "一份契约");
    assert_eq!(snapshot.contracts[0].fqn, fqn);
    assert_eq!(snapshot.contracts[0].ops.len(), 1, "一个 op");
    assert_eq!(snapshot.contracts[0].ops[0].id, "file_stem");
    assert!(snapshot.schemas.contains_key(fqn), "schema 装配在案");
    assert!(snapshot.symbols.contains_key(fqn), "符号集 key = FQN");
    assert!(
        !snapshot.symbols[fqn].is_empty(),
        "取数器从入口抽到符号：{:?}",
        snapshot.symbols[fqn]
    );

    let schema = &snapshot.schemas[fqn];
    for key in ["$schema", "$id", "title", "$comment", "$defs", "properties"] {
        assert!(schema.get(key).is_some(), "schema 六字段齐备（缺 {key}）");
    }
}

/// 边界（AC-7）：`api = declared` 元素无 md → `Ok` 且两元素都不进 `contracts` /
/// `schemas`（缺口交 `apiMissing` 判据，装载器不另记清单）。
#[test]
fn load_contracts缺md零装载() {
    let temp = TempDir::new("load-missing-md");
    let elements = [
        element("App.Backend.Core.Agent", &["crates/core/agent/"]),
        element("App.Frontend.Shell", &["src/shell/"]),
    ];
    let snapshot = load_contracts(temp.root(), &elements).expect("缺 md 不报错");
    assert!(snapshot.contracts.is_empty(), "零契约");
    assert!(snapshot.schemas.is_empty(), "零 schema");
    assert!(snapshot.symbols.is_empty(), "缺 md 的元素不进取数面");
}

/// 边界（D36）：缺少 md 的元素**不记任何清单**——`ApiSnapshot` / `ApiCheckInput` 上没有
/// 「缺契约清单」字段（缺口由判据 ② 从模型意图与 `contracts` 的差集直接判），故源码面
/// MUST 双零残留（字段面 `absent:` + 读取面 `.absent`）。`Absent` 词面只允许出现在
/// `ApiFormStatus::Absent` 的 `as_str` 与 doc 里。
#[test]
fn 快照无缺契约清单字段() {
    let source = include_str!("mod.rs");
    assert!(
        !source.contains("absent:"),
        "`api/mod.rs` 内零 `absent:` 字段声明（D36：死数据已删）"
    );
    assert!(
        !source.contains(".absent"),
        "`api/mod.rs` 内零 `.absent` 读取面（第二事实源已删）"
    );

    // 两个类型的字段面穷尽可数（快照三分量 / 判据入参六字段）
    let snapshot = source
        .split_once("pub struct ApiSnapshot {")
        .expect("ApiSnapshot 定义在案")
        .1
        .split_once("\n}")
        .expect("结构体收口")
        .0;
    let fields = snapshot.matches("    pub ").count();
    assert_eq!(fields, 3, "ApiSnapshot 恰三分量（无第四项清单）");

    let input = source
        .split_once("pub struct ApiCheckInput<'a> {")
        .expect("ApiCheckInput 定义在案")
        .1
        .split_once("\n}")
        .expect("结构体收口")
        .0;
    let input_fields = input.matches("    pub ").count();
    assert_eq!(input_fields, 6, "ApiCheckInput 恰六字段（无 `absent`）");
}

/// 边界（D16 ⑤）：`api = none` 且 md 不存在 → 不进 `contracts`；`api = none`
/// 且 md 存在 → 本函数仍跳过该元素（不解析），留 `apiUnexpected` 判据。
#[test]
fn load_contracts跳过api_none() {
    let temp = TempDir::new("load-none");
    let mut skipped = element("App.Backend.Core.Agent", &["crates/core/agent/"]);
    skipped.api = ApiStatus::None;

    let snapshot = load_contracts(temp.root(), &[skipped.clone()]).expect("api=none 零噪声");
    assert!(snapshot.contracts.is_empty());
    assert!(snapshot.schemas.is_empty(), "api=none 不装配 schema");

    // md 存在时本函数仍跳过（坏契约也不报错——判据面自会点出 apiUnexpected）
    temp.write_contract("App.Backend.Core.Agent", "# 与文件名不一致的标题\n");
    let snapshot = load_contracts(temp.root(), &[skipped]).expect("api=none 不解析 md");
    assert!(snapshot.contracts.is_empty(), "api=none 不装载契约");
    assert!(snapshot.schemas.is_empty(), "api=none 不装配 schema");
}

/// 边界：容器元素（`path = []`）零参与；`elements = []` → `Ok` 空快照。
#[test]
fn load_contracts路径空元素零参与() {
    let temp = TempDir::new("load-empty-path");
    let container = element("App.Backend", &[]);
    let snapshot = load_contracts(temp.root(), &[container]).expect("容器元素零参与");
    assert!(snapshot.contracts.is_empty());

    let snapshot = load_contracts(temp.root(), &[]).expect("空元素表应成功");
    assert!(snapshot.contracts.is_empty());
    assert!(snapshot.schemas.is_empty() && snapshot.symbols.is_empty());
}

/// 异常（AC-1）：坏契约即 `Err(ApiError::Contract(..))`，文案含 `file` 定位；四形态
/// （H1 与文件名不一致 / `### <op-id>` 后缺 `api-op` fence / fence 内 JSON 语法错 /
/// 第二个 `api-types` fence）各报错；**MUST NOT** 返回部分快照。
///
/// 设计 `:95` 的「`api-types` 键未在 `$defs` 出现」一形态经本函数**结构上不可达**——
/// `build_schema` 恒把 `contract.types` 的每个键落进 `$defs`，故 gate (b) 的 ⊆ 关系自动
/// 成立；该形态以「手写 `$defs` 反例」在 `contract_test` 的 gate (b) 用例覆盖。本用例
/// 改以同源可达的 `$defs` 规则违例（`x-class` 带 `required`）补齐第四条。
#[test]
fn load_contracts坏契约即报错() {
    let temp = TempDir::new("load-broken");
    let fqn = "App.Backend.Infra.ArchiRuntime";

    let cases: [(&str, String); 4] = [
        (
            "H1 与文件名不一致",
            "# App.Backend.Infra.Other\n\n## 职能\n\n正文。\n".to_owned(),
        ),
        (
            "op heading 后缺 api-op fence",
            format!("# {fqn}\n\n## 操作\n\n### file_stem\n\n正文而已。\n"),
        ),
        (
            "fence 内 JSON 语法错",
            format!("# {fqn}\n\n```json api-types\n{{\"A\": {{\"type\": \"object\"}}\n```\n"),
        ),
        (
            "第二个 api-types fence",
            format!(
                "# {fqn}\n\n```json api-types\n{{\"A\": {{\"type\": \"object\"}}}}\n```\n\n\
                 ```json api-types\n{{\"B\": {{\"type\": \"object\"}}}}\n```\n"
            ),
        ),
    ];

    for (label, source) in cases {
        temp.write_contract(fqn, &source);
        let error = match load_contracts(temp.root(), &[element(fqn, &["crates/infra/archi/"])]) {
            Ok(snapshot) => panic!("坏契约（{label}）MUST NOT 返回 Ok：{snapshot:?}"),
            Err(error) => error,
        };
        match &error {
            ApiError::Contract(message) => {
                assert!(message.contains(fqn), "{label} 的文案含文件定位：{message}");
                assert!(
                    message.contains(':'),
                    "{label} 的文案带 `file:line` 形态：{message}"
                );
            }
            other => panic!("{label} 期望 ApiError::Contract，实际：{other:?}"),
        }
    }

    // 同源可达的 `$defs` 规则违例：`x-class` 带 `required`（D27 错误侧，拦在装载期）
    temp.write_contract(
        fqn,
        &format!(
            "# {fqn}\n\n```json api-types\n{{\"Hooks\": {{\"x-class\": true, \"required\": [\"a\"], \
             \"properties\": {{}}}}}}\n```\n"
        ),
    );
    let error = load_contracts(temp.root(), &[element(fqn, &["crates/infra/archi/"])])
        .expect_err("`x-class` 带 `required` 应报契约错误");
    assert!(
        matches!(error, ApiError::Contract(_)),
        "`$defs` 规则违例归 Contract：{error:?}"
    );
    assert!(
        error.message().contains("Hooks"),
        "点名违规类型：{}",
        error.message()
    );
}

/// 异常（AC-1 / AC-23 / D45）：闸一违例（**七族**任一族非 ASCII 标识符）在
/// `contract::verify` 期即中止——`load_contracts` 是本仓装载期唯一中止点，五形态各
/// `Err(ApiError::Contract(..))` 并点名各自族别，文案含 `file:line`；**MUST NOT** 返回
/// 部分快照。
#[test]
fn load_contracts闸一违例即中止() {
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let cases: [(&str, &str, String); 5] = [
        (
            "op id 族",
            "op id",
            format!(
                "# {fqn}\n\n### 带-连字符\n\n```json api-op\n{{\"params\": [], \"returns\": \
                 {{\"type\": \"string\"}}}}\n```\n"
            ),
        ),
        (
            "参数名族",
            "参数名",
            format!(
                "# {fqn}\n\n### run\n\n```json api-op\n{{\"params\": [{{\"name\": \"in-put\", \
                 \"schema\": {{\"type\": \"string\"}}}}], \"returns\": {{\"type\": \"string\"}}}}\n```\n"
            ),
        ),
        (
            "properties 数据字段族",
            "properties 数据字段键",
            format!(
                "# {fqn}\n\n```json api-types\n{{\"LocalArchiCheck\": {{\"type\": \"object\", \
                 \"properties\": {{\"a-b\": {{\"type\": \"integer\"}}}}}}}}\n```\n"
            ),
        ),
        (
            "owner 族",
            "owner 键",
            format!(
                "# {fqn}\n\n### file_stem\n\n```json api-op\n{{\"owner\": \"带-连字符\", \
                 \"params\": [], \"returns\": {{\"type\": \"string\"}}}}\n```\n"
            ),
        ),
        (
            "$ref 末段族",
            "$ref 末段名",
            format!(
                "# {fqn}\n\n### file_stem\n\n```json api-op\n{{\"params\": [{{\"name\": \"input\", \
                 \"schema\": {{\"$ref\": \"#/$defs/带 空格\"}}}}], \"returns\": \
                 {{\"type\": \"string\"}}}}\n```\n"
            ),
        ),
    ];

    for (label, family, source) in cases {
        let temp = TempDir::new("load-gate1");
        temp.write_contract(fqn, &source);
        let error = match load_contracts(temp.root(), &[element(fqn, &["crates/infra/archi/"])]) {
            Ok(snapshot) => panic!("闸一违例（{label}）MUST NOT 返回部分快照：{snapshot:?}"),
            Err(error) => error,
        };
        match &error {
            ApiError::Contract(message) => {
                assert!(
                    message.contains(family),
                    "{label} 的文案点名族别 `{family}`：{message}"
                );
                assert!(
                    message.contains("ASCII 标识符"),
                    "{label} 点明判据：{message}"
                );
                assert!(
                    message.contains(fqn) && message.contains(':'),
                    "{label} 的文案带 `file:line` 定位：{message}"
                );
            }
            other => panic!("{label} 期望 ApiError::Contract，实际：{other:?}"),
        }
    }
}

/// 异常（AC-1 / **AC-25** / D47）：**落点闸违例在装载期即中止**——`x-function` 写在四种
/// 越界位置（`$defs.T.properties.call.items` / `additionalProperties` / `oneOf` 分支 /
/// `$defs.T` 自身顶层）各 `Err(ApiError::Contract(..))` 并点名 **JSON pointer**，
/// **MUST NOT 走到渲染期**（否则深层 `params[].name` 会沿 `value_type` →
/// `function_type` / `method_signature` 裸插，而闸一 ④ 族从未判过它）；MUST NOT 返回
/// 部分快照。`load_contracts` 是本仓装载期唯一中止点。
#[test]
fn load_contracts落点闸违例即中止() {
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let cases: [(&str, &str, String); 4] = [
        (
            "properties 的 items",
            "$defs.T.properties.call.items",
            format!(
                "# {fqn}\n\n```json api-types\n{{\"T\": {{\"type\": \"object\", \"properties\": \
                 {{\"call\": {{\"type\": \"array\", \"items\": {{\"x-function\": {{\"params\": [], \
                 \"returns\": {{\"type\": \"string\"}}}}}}}}}}}}}}\n```\n"
            ),
        ),
        (
            "additionalProperties",
            "$defs.T.additionalProperties",
            format!(
                "# {fqn}\n\n```json api-types\n{{\"T\": {{\"type\": \"object\", \
                 \"additionalProperties\": {{\"x-function\": {{\"params\": [], \"returns\": \
                 {{\"type\": \"string\"}}}}}}}}}}\n```\n"
            ),
        ),
        (
            "oneOf 分支",
            "$defs.T.oneOf[1]",
            format!(
                "# {fqn}\n\n```json api-types\n{{\"T\": {{\"oneOf\": [{{\"type\": \"string\"}}, \
                 {{\"x-function\": {{\"params\": [], \"returns\": {{\"type\": \
                 \"string\"}}}}}}]}}}}\n```\n"
            ),
        ),
        (
            "$defs 条目自身顶层",
            "$defs.T",
            format!(
                "# {fqn}\n\n```json api-types\n{{\"T\": {{\"x-function\": {{\"params\": [], \
                 \"returns\": {{\"type\": \"string\"}}}}}}}}\n```\n"
            ),
        ),
    ];

    for (label, pointer, source) in cases {
        let temp = TempDir::new("load-position");
        temp.write_contract(fqn, &source);
        let error = match load_contracts(temp.root(), &[element(fqn, &["crates/infra/archi/"])]) {
            Ok(snapshot) => {
                panic!("落点闸违例（{label}）MUST NOT 返回部分快照：{snapshot:?}")
            }
            Err(error) => error,
        };
        match &error {
            ApiError::Contract(message) => {
                assert!(
                    message.contains("x-function 落点"),
                    "{label} 的文案含定形族别串：{message}"
                );
                assert!(
                    message.contains(pointer),
                    "{label} 的文案点名 JSON pointer `{pointer}`：{message}"
                );
                assert!(
                    message.contains(fqn) && message.contains(':'),
                    "{label} 的文案带 `file:line` 定位：{message}"
                );
            }
            other => panic!("{label} 期望 ApiError::Contract，实际：{other:?}"),
        }
    }

    // 合法二处不受影响：试点形态（`api-op` 顶层签名 + 零字段 `properties`）仍 `Ok`
    let temp = TempDir::new("load-position-legal");
    temp.write_contract(fqn, &pilot_markdown(fqn));
    let snapshot = load_contracts(temp.root(), &[element(fqn, &["crates/infra/archi/"])])
        .expect("试点契约仍 Ok");
    assert_eq!(snapshot.contracts.len(), 1, "合法落点不被误拦");
}

/// 异常（D20）：md 文件在案但不可解码（非法 UTF-8 字节）→ `Err(ApiError::Io(..))`，
/// `message()` 非空且含路径线索。目录占位形态走「文件不在案」分支（`is_file()` 为假），
/// 故 IO 面以「文件在案但读取失败」表达。
#[test]
fn load_contracts的io错误面() {
    let temp = TempDir::new("load-io");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let path = paths::contract_path(temp.root(), fqn).expect("测试 FQN 恒为合法形态");
    fs::create_dir_all(path.parent().expect("父目录")).expect("建契约目录");
    fs::write(&path, [0xff_u8, 0xfe, 0x00, 0x01]).expect("写非法 UTF-8 字节");

    let error = match load_contracts(temp.root(), &[element(fqn, &["crates/infra/archi/"])]) {
        Ok(snapshot) => panic!("不可解码的契约应报 IO 错误：{snapshot:?}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, ApiError::Io(_)),
        "读取失败 → ApiError::Io，实际：{error:?}"
    );
    let message = error.message();
    assert!(!message.is_empty(), "文案非空");
    assert!(
        message.contains("ArchiRuntime") || message.contains("契约"),
        "文案含路径线索：{message}"
    );
}

/// 边界：300 元素（其中 100 有 md）→ `Ok`，`contracts.len() == 100`（其余 200 个不进
/// 契约面，缺口由判据 ② 报）。
#[test]
fn load_contracts规模边界() {
    let temp = TempDir::new("load-scale");
    let mut elements = Vec::new();
    for index in 0..300 {
        let fqn = format!("App.Core.E{index:03}");
        elements.push(element(&fqn, &[&format!("src/e{index:03}/")]));
        if index < 100 {
            temp.write_contract(&fqn, &pilot_markdown(&fqn));
        }
    }
    let snapshot = load_contracts(temp.root(), &elements).expect("规模输入应成功");
    assert_eq!(snapshot.contracts.len(), 100, "100 份契约");
    assert_eq!(snapshot.schemas.len(), 100, "100 份 schema");
    assert_eq!(snapshot.symbols.len(), 100, "100 个元素的取数面");
    assert!(
        snapshot
            .contracts
            .windows(2)
            .all(|window| window[0].fqn <= window[1].fqn),
        "契约按 FQN 字典序"
    );
    // 第二规模：1000 键 `$defs` 的契约 → 装配成功、键数逐项相等、零 panic
    let big = TempDir::new("load-scale-defs");
    let big_fqn = "App.Core.Big";
    let mut defs = String::new();
    for index in 0..1000 {
        if index > 0 {
            defs.push_str(", ");
        }
        defs.push_str(&format!("\"T{index:04}\": {{\"type\": \"object\"}}"));
    }
    big.write_contract(
        big_fqn,
        &format!("# {big_fqn}\n\n```json api-types\n{{{defs}}}\n```\n"),
    );
    let snapshot = load_contracts(big.root(), &[element(big_fqn, &["src/big/"])])
        .expect("1000 键契约应装配成功");
    let schema = snapshot.schemas.get(big_fqn).expect("schema 在案");
    assert_eq!(
        schema["$defs"].as_object().map(|defs| defs.len()),
        Some(1000),
        "1000 个 `$defs` 键逐项落位（零截断、零 panic）"
    );
}

// ---------------------------------------------------------------------------
// render_all：四类生成物 / 幂等 / INDEX 覆盖 / categories
// ---------------------------------------------------------------------------

/// 正向（AC-12）：试点形态 → 文件集含 `<FQN>.schema.json`、`INDEX.toml`、`<root>.i.rs`；
/// `.i.ts` 按元素语言缺席；每份 `path` 与 `paths` 单点产出值逐字相等。
#[test]
fn render_all四类生成物齐备() {
    let temp = TempDir::new("render-all");
    temp.write_model(
        "[element.\"App.Backend.Infra.ArchiRuntime\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/infra/archi/\"]
",
    );
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    let paths_set: Vec<&Path> = files.iter().map(|file| file.path.as_path()).collect();
    assert!(
        paths_set.contains(
            &paths::schema_path(temp.root(), fqn)
                .expect("合法 FQN")
                .as_path()
        ),
        "schema 生成物落点 = paths::schema_path"
    );
    assert!(
        paths_set.contains(&paths::index_path(temp.root()).as_path()),
        "INDEX 落点 = paths::index_path"
    );
    let generated = paths::generated_path(
        temp.root(),
        elements[0].path.first().expect("非空 root"),
        paths::GeneratedKind::Rust,
    )
    .expect("Rust 生成物路径");
    assert!(
        paths_set.contains(&generated.as_path()),
        "元素生成物落点 = paths::generated_path（Rust）"
    );
    assert!(
        !paths_set
            .iter()
            .any(|path| path.extension().is_some_and(|ext| ext == "ts")),
        "试点为 Rust 元素，无 .i.ts"
    );

    for file in &files {
        assert!(!file.content.is_empty(), "生成物内容非空：{:?}", file.path);
    }
    let schema_file = files
        .iter()
        .find(|file| file.path == paths::schema_path(temp.root(), fqn).expect("合法 FQN"))
        .expect("schema 生成物在案");
    assert!(schema_file.content.ends_with('\n'), "schema 末尾有换行");
    assert!(
        schema_file.content.contains("\"LocalArchiCheck\""),
        "schema 含类型定义"
    );
}

/// 正向（D40 / D41）：`render_all` 建 `EmitTarget` 的两次求值——A（crate 形态，入口命中
/// `<dir>/src/lib.rs`）与 B（前端目录形态，入口命中 `<root>/index.ts`，`<root>` 带 `.tsx`
/// 故语言面为 TypeScript）。B 的 specifier 经 `paths::ts_import_specifier` 产出（零手算
/// 层数）；入口全不命中的元素**不产 `.i.*`**（D41：拿不到形态即不生成）。
///
/// **A 的透传目标形态是反例面（路径安全边界）**：A 的 `api-reexport` 指向的 B 的元素根是
/// `views/welcome-view.tsx`，末段 `welcome-view.tsx` 不是合法模块段 ⇒ 该目标不进 A 的透传
/// 查表，产物只留 `// 透传自 <FQN>` 注释、**不产 `pub use`**（`crate::welcome-view.tsx` 是
/// 不可编译文本）。这是「路径派生的模块名进目标语言源码前必须过形态闸」的端到端见证；合法
/// 形态一侧的 `pub use` 断言见 `emit::rust` 的 `render的透传pub_use`。
#[test]
fn render_all建emit_target的两次求值() {
    let temp = TempDir::new("render-all-emit-target");
    let backend = "App.Backend.Infra.ArchiRuntime";
    let frontend = "App.Frontend.Views.Welcome";
    let no_entry = "App.Core.NoEntry";
    temp.write_model(&format!(
        "[element.\"{backend}\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/infra/archi/\"]

         [element.\"{frontend}\"]
kind = \"module\"
gate = \"warn\"
path = [\"views/welcome-view.tsx\"]

         [element.\"{no_entry}\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/core/no-entry/\"]
"
    ));

    // A：crate 形态入口（`<dir>/src/lib.rs`），透传目标为 B
    temp.write_contract(
        backend,
        &format!(
            "# {backend}

## 职能

正文。

```json api-reexport
{{\"AgentView\": \"{frontend}\"}}
```

## 透传

- AgentView → {frontend}
"
        ),
    );
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }
",
    );

    // B：目录形态入口（`<root>/index.ts`，四分支第一支 = SubdirModule），透传目标为 A
    temp.write_contract(
        frontend,
        &format!(
            "# {frontend}

## 职能

正文。

```json api-reexport
{{\"Accept\": \"{backend}\"}}
```

## 透传

- Accept → {backend}

```json api-types
{{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}
```
"
        ),
    );
    temp.write(
        "views/welcome-view.tsx/index.ts",
        "export function view(): string { return ''; }
",
    );

    // 无入口元素：`path` 非空但三分支 / 四分支全不命中（D41：拿不到形态 ⇒ 不产 `.i.*`）
    temp.write_contract(
        no_entry,
        &format!(
            "# {no_entry}

## 职能

正文。

```json api-types
{{\"Ghost\": {{\"type\": \"object\", \"properties\": {{}}}}}}
```
"
        ),
    );

    let elements = [
        element(backend, &["crates/infra/archi/"]),
        element(frontend, &["views/welcome-view.tsx"]),
        element(no_entry, &["crates/core/no-entry/"]),
    ];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    // A 的 `.i.rs`：B 的末段（`welcome-view.tsx`）不是合法模块段 ⇒ 不入 A 的透传查表 ⇒ 只留
    // 透传注释、不产 `pub use`（路径安全边界的行为面；`crate::welcome-view.tsx` 不可编译）。
    // D40 的意图仍在：注释里是**目标 FQN**，不是由它派生的模块路径。
    let backend_generated = paths::generated_path(
        temp.root(),
        elements[0].path.first().expect("非空 root"),
        paths::GeneratedKind::Rust,
    )
    .expect("Rust 生成物路径");
    let backend_file = files
        .iter()
        .find(|file| file.path == backend_generated)
        .expect("A 的 .i.rs 在案");
    assert_eq!(
        paths::entry_form(temp.root(), &elements[0]).map(|(form, _)| form),
        Some(paths::EntryForm::Crate),
        "A 的形态由 paths::entry_form 判（非渲染器推导）"
    );
    let target_name = paths::module_name("views/welcome-view.tsx");
    assert!(
        !paths::is_module_segment(&target_name),
        "目标末段不是合法模块段（本形态的前提断言，末段 = {target_name}）"
    );
    assert!(
        backend_file
            .content
            .contains(&format!("// 透传自 {frontend}")),
        "非法末段的目标只留透传注释（FQN 原文，非派生路径）：
{}",
        backend_file.content
    );
    assert!(
        !backend_file.content.contains("pub use "),
        "MUST NOT 产出不可编译的 `pub use crate::welcome-view.tsx::…`：
{}",
        backend_file.content
    );

    // B 的 `.i.ts`：specifier 逐字取自 `paths::ts_import_specifier`
    let frontend_generated = paths::generated_path(
        temp.root(),
        elements[1].path.first().expect("非空 root"),
        paths::GeneratedKind::TypeScript,
    )
    .expect("TS 生成物路径");
    let frontend_file = files
        .iter()
        .find(|file| file.path == frontend_generated)
        .expect("B 的 .i.ts 在案");
    let expected_specifier = paths::ts_import_specifier(
        paths::EntryForm::SubdirModule,
        &paths::module_name("crates/infra/archi"),
    );
    assert_eq!(expected_specifier, "../archi.i");
    assert!(
        frontend_file.content.contains("export interface Item"),
        "B 的类型段在案：
{}",
        frontend_file.content
    );
    assert!(
        frontend_file
            .content
            .contains(&format!("from '{expected_specifier}'")),
        "B 的 specifier 逐字取自 paths::ts_import_specifier：
{}",
        frontend_file.content
    );

    let no_entry_generated = paths::generated_path(
        temp.root(),
        elements[2].path.first().expect("非空 root"),
        paths::GeneratedKind::Rust,
    )
    .expect("路径可派生（形态才不可得）");
    assert!(
        !files.iter().any(|file| file.path == no_entry_generated),
        "入口全不命中的元素不产 `.i.*`（D41：形态拿不到即不生成）"
    );
}

/// 边界（D42 源码自证 + 行为）：`api/mod.rs` 内零 `ArchiElement` 构造
/// （元素表经 `dto::graph_view` 单点取数，本文件不再自行投影；`generated_path` 只收
/// `<root>` 字符串，容器元素由 `element.path.first()?` 判，`paths` 层不判容器）；
/// 容器元素（`path = []`）→ `generated_rel` 为 `None`、**不产** `.i.*`、零 panic。
#[test]
fn render_all零dummy元素构造() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("ArchiElement {").count(),
        0,
        "零 `ArchiElement {{` 构造：元素表经 `dto::graph_view` 单点取数，MUST NOT 自抄投影"
    );
    assert!(
        !source.contains("fqn: String::new()"),
        "零占位元素的 fqn 构造（D42）"
    );

    let temp = TempDir::new("render-all-container");
    temp.write_model(
        "[element.\"App.Core.Container\"]
kind = \"module\"
gate = \"warn\"
path = []
",
    );
    let container = "App.Core.Container";
    temp.write_contract(
        container,
        &format!(
            "# {container}

## 职能

正文。

```json api-types
{{\"Boxed\": {{\"type\": \"object\", \"properties\": {{}}}}}}
```
"
        ),
    );

    let elements = [element(container, &[])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("容器元素不 panic");

    assert!(
        !files.iter().any(|file| file
            .path
            .extension()
            .is_some_and(|ext| ext == "rs" || ext == "ts")),
        "容器元素不产 `.i.*`：{:?}",
        files.iter().map(|file| &file.path).collect::<Vec<_>>()
    );
    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 恒产出");
    assert!(
        !index.content.contains(container),
        "容器元素不进 INDEX（零参与，D17）：
{}",
        index.content
    );
}

/// 边界（事实校准 6）：契约零 op 零类型 → 无 `.i.rs` / `.i.ts` 条目；`INDEX.toml` 仍产出
/// 且该元素 `ops = 0` / `types = 0`。
#[test]
fn render_all零内容不生成i() {
    let temp = TempDir::new("render-all-empty");
    temp.write_model(
        "[element.\"App.Backend.Core.Agent\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/core/agent/\"]
",
    );
    let fqn = "App.Backend.Core.Agent";
    temp.write_contract(fqn, &format!("# {fqn}\n\n## 职能\n\n正文。\n"));
    temp.write("crates/core/agent/src/lib.rs", "pub fn run() {}\n");

    let elements = [element(fqn, &["crates/core/agent/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    assert!(
        !files.iter().any(|file| {
            file.path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".i.rs") || name.ends_with(".i.ts"))
        }),
        "零内容不生成 .i.*：{:?}",
        files.iter().map(|file| &file.path).collect::<Vec<_>>()
    );

    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 仍产出");
    assert!(index.content.contains("ops = 0"), "ops = 0 仍输出键");
    assert!(index.content.contains("types = 0"), "types = 0 仍输出键");
}

/// 边界（AC-2 / AC-12）：同输入两次调用 → 两次 `Vec<GeneratedFile>` 逐字段相等。
#[test]
fn render_all幂等() {
    let temp = TempDir::new("render-all-idempotent");
    temp.write_model(
        "[element.\"App.Backend.Infra.ArchiRuntime\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/infra/archi/\"]
",
    );
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let first = render_all(temp.root(), &snapshot).expect("首次渲染");
    let second = render_all(temp.root(), &snapshot).expect("二次渲染");
    assert_eq!(first, second, "两次渲染逐字段相等（含 INDEX 文本与头注释）");
}

/// 边界（D17）：3 个 `path` 非空元素（1 有契约 / 2 缺）→ INDEX 三条 entry，`status`
/// 分别 `declared` / `absent` / `absent`，按 FQN 字典序；`path` 空元素不出现。
#[test]
fn render_all的index覆盖面() {
    let temp = TempDir::new("render-all-index");
    temp.write_model(
        "[element.\"App.Core.Present\"]
kind = \"module\"
gate = \"warn\"
path = [\"src/alpha/\"]

         [element.\"App.Core.Missing\"]
kind = \"module\"
gate = \"warn\"
path = [\"src/missing/\"]

         [element.\"App.Core.Absent\"]
kind = \"module\"
gate = \"warn\"
path = [\"src/absent/\"]

         [element.\"App.Core.Container\"]
kind = \"package\"
gate = \"off\"
",
    );
    // 契约文件名必须与 FQN 逐字一致，故契约落在 Present 元素上
    temp.write_contract("App.Core.Present", &pilot_markdown("App.Core.Present"));
    temp.write(
        "src/alpha/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [
        element("App.Core.Present", &["src/alpha/"]),
        element("App.Core.Missing", &["src/missing/"]),
        element("App.Core.Absent", &["src/absent/"]),
        element("App.Core.Container", &[]),
    ];

    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");

    assert!(
        !index.content.contains("App.Core.Container"),
        "path 空元素不出现：\n{}",
        index.content
    );
    let present = index
        .content
        .find("fqn = \"App.Core.Present\"")
        .expect("有契约元素在案");
    assert!(
        index.content[present..].starts_with("fqn = \"App.Core.Present\"\nstatus = \"declared\""),
        "有契约元素 status = declared：\n{}",
        &index.content[present..]
    );
    for absent in ["App.Core.Absent", "App.Core.Missing"] {
        let at = index
            .content
            .find(&format!("fqn = \"{absent}\""))
            .unwrap_or_else(|| panic!("缺契约元素 {absent} 应在案"));
        assert!(
            index.content[at..].starts_with(&format!("fqn = \"{absent}\"\nstatus = \"absent\"")),
            "{absent} 的 status = absent：\n{}",
            &index.content[at..]
        );
    }

    let order: Vec<&str> = index
        .content
        .lines()
        .filter(|line| line.starts_with("fqn = "))
        .collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted, "条目按 FQN 字典序");
}

/// 正向（AC-2 / D26）：`$defs` 一 object + 一个 `api-reexport` 别名 → entry 的
/// `categories` 为 `object = 1` / `reexport = 1`（`sum == types == 2`）；`absent` 元素无
/// `categories` 段；单形状（多 object）计数正确。
#[test]
fn render_all的index_categories() {
    let temp = TempDir::new("render-all-categories");
    temp.write_model(
        "[element.\"App.Core.Alpha\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n\n         [element.\"App.Core.Missing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/missing/\"]\n",
    );
    let fqn = "App.Core.Alpha";
    temp.write_contract(
        fqn,
        &format!(
            "# {fqn}\n\n```json api-types\n{{\"Alpha\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n\n             ```json api-reexport\n{{\"AgentInput\": \"App.Core.Beta\"}}\n```\n\n             ## 透传\n\n- AgentInput → App.Core.Beta\n"
        ),
    );
    temp.write("src/alpha/lib.rs", "pub fn f() {}\n");

    let elements = [
        element(fqn, &["src/alpha/"]),
        element("App.Core.Missing", &["src/missing/"]),
    ];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");

    assert!(
        index.content.contains("[entry.categories]"),
        "有形状分布的 entry 输出 categories 子表：\n{}",
        index.content
    );
    assert!(
        index.content.contains("object = 1"),
        "object 形状计数：\n{}",
        index.content
    );
    assert!(
        index.content.contains("reexport = 1"),
        "reexport 形状计数：\n{}",
        index.content
    );
    assert!(
        index.content.contains("types = 2"),
        "types = 2（object + reexport）：\n{}",
        index.content
    );
    assert_eq!(
        index.content.matches("[entry.categories]").count(),
        1,
        "absent 元素不输出 categories 段：\n{}",
        index.content
    );
    // 单形状（五个 object）→ `object = 5` 且 `types = 5`（同一推导路径的第二形态）
    temp.write_contract(
        fqn,
        &format!(
            "# {fqn}\n\n```json api-types\n{{\"A\": {{\"type\": \"object\"}}, \
             \"B\": {{\"type\": \"object\"}}, \"C\": {{\"type\": \"object\"}}, \
             \"D\": {{\"type\": \"object\"}}, \"E\": {{\"type\": \"object\"}}}}\n```\n"
        ),
    );
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");
    assert!(
        index.content.contains("object = 5"),
        "五条 object → object = 5：\n{}",
        index.content
    );
    assert!(
        index.content.contains("types = 5"),
        "types 与形状计数之和同源（均为 5）：\n{}",
        index.content
    );
}

/// 边界（D26 / 阶段九判序）：六形状经 `render_all` → `categories_of` 驱动（不直接调私有
/// 函数）；并存形态按判序取先者；非 object 值退 `object` 不 panic。
#[test]
fn render_all的def_shape六形状() {
    let temp = TempDir::new("render-all-shapes");
    temp.write_model(
        "[element.\"App.Core.Shapes\"]
kind = \"module\"
gate = \"warn\"
path = [\"src/shapes/\"]
",
    );
    let fqn = "App.Core.Shapes";
    temp.write_contract(
        fqn,
        &format!(
            "# {fqn}\n\n```json api-types\n{{\
             \"Reexport\": {{\"$ref\": \"#/$defs/Alpha\"}}, \
             \"Class\": {{\"x-class\": true}}, \
             \"Variant\": {{\"oneOf\": [{{\"$ref\": \"#/$defs/Alpha\"}}]}}, \
             \"Choice\": {{\"enum\": [\"a\", \"b\"]}}, \
             \"Scalar\": {{\"type\": \"integer\"}}, \
             \"Object\": {{\"type\": \"object\", \"properties\": {{}}}}, \
             \"Both\": {{\"x-class\": true, \"oneOf\": [{{\"$ref\": \"#/$defs/Alpha\"}}]}}, \
             \"Alpha\": {{\"type\": \"object\", \"properties\": {{}}}}\
             }}\n```\n"
        ),
    );
    temp.write("src/shapes/lib.rs", "pub fn f() {}\n");

    let elements = [element(fqn, &["src/shapes/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");

    // 逐形状断言（判序：$ref → x-class → oneOf/anyOf → enum → 标量 → object）
    let expected_counts = [
        ("class", 2),    // Class + Both（x-class 先于 oneOf）
        ("enum", 1),     // Choice
        ("object", 2),   // Object + Alpha
        ("reexport", 1), // Reexport（纯 $ref alias）
        ("scalar", 1),   // Scalar
        ("variant", 1),  // Variant
    ];
    for (shape, expected) in expected_counts {
        assert!(
            index.content.contains(&format!("{shape} = {expected}")),
            "形状 {shape} 计数应为 {expected}：\n{}",
            index.content
        );
    }
    assert!(
        index.content.contains("types = 8"),
        "types 为 $defs 键数（8）：\n{}",
        index.content
    );
    // sum(categories) == types（形状分布与类型数同源）
    let sum: usize = expected_counts.iter().map(|(_, count)| count).sum();
    assert_eq!(sum, 8, "六形状计数之和 == types");
}

/// 边界（D26 / 索引七字段）：`degraded` 计数由 `render_all` 从 `$defs` 的
/// `x-common.degraded` 条目数派生（`index_entries` 单点，零第二套算法）——两条带标记
/// 的定义 → `degraded = 2`；同一契约去掉这两条标记 → `degraded = 0`（键仍输出「0」而非
/// 省略，见 `build_index` 章节）。
#[test]
fn render_all的index_degraded派生() {
    let temp = TempDir::new("render-all-degraded");
    temp.write_model(
        "[element.\"App.Core.Marked\"]
kind = \"module\"
gate = \"warn\"
path = [\"src/marked/\"]
",
    );
    let fqn = "App.Core.Marked";
    // 两条带 `x-common.degraded` 的定义 + 一条不带（计数只数带标记者）
    temp.write_contract(
        fqn,
        &format!(
            "# {fqn}\n\n```json api-types\n{{\"Degraded\": {{\"type\": \"object\", \
             \"x-common\": {{\"degraded\": true}}}}, \
             \"Rejected\": {{\"type\": \"object\", \"x-common\": {{\"degraded\": true}}}}, \
             \"Plain\": {{\"type\": \"object\"}}}}\n```\n"
        ),
    );
    temp.write("src/marked/lib.rs", "pub fn f() {}\n");

    let elements = [element(fqn, &["src/marked/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");
    assert!(
        index.content.contains("types = 3"),
        "三条定义（含标记的两条）：\n{}",
        index.content
    );
    assert!(
        index.content.contains("degraded = 2"),
        "`x-common.degraded` 两条 → degraded = 2（派生单点在 `index_entries`）：\n{}",
        index.content
    );

    // 第二形态：同一契约去掉两条标记 → degraded = 0（键仍输出 0，不省略）
    temp.write_contract(
        fqn,
        &format!(
            "# {fqn}\n\n```json api-types\n{{\"Degraded\": {{\"type\": \"object\"}}, \
             \"Rejected\": {{\"type\": \"object\"}}, \"Plain\": {{\"type\": \"object\"}}}}\n```\n"
        ),
    );
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");
    assert!(
        index.content.contains("degraded = 0"),
        "无标记 → degraded 输出 0（不省略键）：\n{}",
        index.content
    );
}
/// 异常（D26 反例）：判定面人为多报不影响 `render_all` 产出的 INDEX（INDEX 是纯投影，
/// 只随契约改动变化）；`api/mod.rs` 内零 `category_of` 引用。
#[test]
fn render_all的categories不借判据分类() {
    let temp = TempDir::new("render-all-no-judge");
    temp.write_model(
        "[element.\"App.Core.Alpha\"]
kind = \"module\"
gate = \"warn\"
path = [\"src/alpha/\"]
",
    );
    let fqn = "App.Core.Alpha";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write(
        "src/alpha/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["src/alpha/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let baseline = render_all(temp.root(), &snapshot).expect("首次渲染");
    let baseline_index = baseline
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案")
        .content
        .clone();

    // 判定面多报（不同的 input 只影响 findings，不影响 INDEX）
    let extra = ApiCheckInput {
        root: temp.root(),
        elements: &elements,
        contracts: &snapshot.contracts,
        schemas: &snapshot.schemas,
        symbols: &snapshot.symbols,
        enabled: true,
    };
    let _ = check(extra);

    let again = render_all(temp.root(), &snapshot).expect("二次渲染");
    let again_index = again
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案")
        .content
        .clone();
    assert_eq!(baseline_index, again_index, "INDEX 文本不随判定面变化");

    let source = include_str!("mod.rs");
    assert!(
        !source.contains("category_of("),
        "形状分布不借判据分类（mod.rs 零 category_of 调用）"
    );
    assert!(
        source.contains("fn def_shape"),
        "打标单点是本文件的私有 def_shape"
    );
}

/// 边界：空快照 → 只含 `INDEX.toml`（零 entry）；`root` 指向不存在路径 → 纯推导不 panic。
#[test]
fn render_all空快照() {
    let temp = TempDir::new("render-all-empty-snapshot");
    let files = render_all(temp.root(), &empty_snapshot()).expect("空快照应成功");
    assert_eq!(files.len(), 1, "只有 INDEX：{:?}", files);
    assert_eq!(files[0].path, paths::index_path(temp.root()));
    assert!(
        !files[0].content.contains("[[entry]]"),
        "零 entry：\n{}",
        files[0].content
    );

    let missing = Path::new("/definitely/not/exists");
    let files = render_all(missing, &empty_snapshot()).expect("纯推导不 panic");
    assert_eq!(files.len(), 1, "不存在 root 仍只出 INDEX");
}

/// 异常（单点取数 / fail-closed）：模型含结构诊断（祖先未声明）⇒ `render_all` /
/// `bootstrap` 报 `ApiError::Element`，**零生成物**。
///
/// 这是「坏模型不得被固化成入库生成物」的端到端判据：元素表取自 `dto::graph_view`
/// （与画布 / 漂移门禁同一入口），该入口在结构错时给空表 + 诊断——若契约面自行
/// `parse_dir` 抄一份投影，就会在停摆模型上照出元素，把 `INDEX.toml` 静默改写成
/// 全体 `absent`。夹具故意绕过 `declare_ancestors` 直接落盘坏模型。
#[test]
fn 结构诊断下render_all与bootstrap拒绝生成() {
    let temp = TempDir::new("render-all-broken-model");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write(
        ".dev-team/architecture/models/01-test.toml",
        &format!(
            "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
             path = [\"crates/infra/archi/\"]\n"
        ),
    );
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载不看模型结构");
    let error = render_all(temp.root(), &snapshot).expect_err("结构错 ⇒ 拒绝生成");
    assert!(
        matches!(error, ApiError::Element(_)),
        "错误面为 Element（元素表来源不可用），实际：{error:?}"
    );
    assert!(
        error.message().contains("orphanHierarchy"),
        "文案点名结构诊断：{}",
        error.message()
    );

    let bootstrap_error = bootstrap(temp.root(), fqn).expect_err("结构错 ⇒ 不起草");
    assert!(
        matches!(bootstrap_error, ApiError::Element(_)),
        "bootstrap 同口径，实际：{bootstrap_error:?}"
    );
}

// ---------------------------------------------------------------------------
// check：短路 / 稳定排序 / 委托单点
// ---------------------------------------------------------------------------

/// 边界（D17）：`enabled = false` + 其它字段全非空 → 空 `Vec`（启用条件优先）。
#[test]
fn check未启用短路() {
    let temp = TempDir::new("check-disabled");
    let fqn = "App.Core.Alpha";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    let elements = [element(fqn, &["src/alpha/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");

    let findings = check(ApiCheckInput {
        root: temp.root(),
        elements: &elements,
        contracts: &snapshot.contracts,
        schemas: &snapshot.schemas,
        symbols: &snapshot.symbols,
        enabled: false,
    });
    assert!(findings.is_empty(), "未启用即空集，不因数据存在而产出");
}

/// 正向（D18）：构造多元素多 kind 命中（`apiEntryMissing` ×2 + `apiMissing` ×2 +
/// `apiConstructorDeclared` ×1）→ 返回序按 `(kind, element, target)` 升序；**两次运行
/// 逐字段相等**（可逐字 diff 的报告）。元素输入序故意与期望序相反（乱序注入）。
#[test]
fn check稳定排序() {
    let temp = TempDir::new("check-sorted");
    let constructor_fqn = "App.Core.Constructor";
    temp.write_contract(
        constructor_fqn,
        &format!(
            "# {constructor_fqn}\n\n### new\n\n```json api-op\n{{\"params\": [], \"returns\": \
             {{\"type\": \"object\"}}}}\n```\n"
        ),
    );
    temp.write(
        "src/constructor/src/lib.rs",
        "pub fn new() -> Self { Self }\n",
    );
    // apiMissing 的两个元素：有入口、无契约
    temp.write("src/present/src/lib.rs", "pub fn present() {}\n");
    temp.write("src/present2/src/lib.rs", "pub fn present2() {}\n");

    let elements = [
        element("App.Core.Present", &["src/present/"]),
        element("App.Core.NoEntryB", &["src/missing-b/"]),
        element(constructor_fqn, &["src/constructor/"]),
        element("App.Core.NoEntryA", &["src/missing-a/"]),
        element("App.Core.Present2", &["src/present2/"]),
    ];

    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    assert_eq!(
        snapshot.contracts.len(),
        1,
        "五个元素里只有一个有契约：{:?}",
        snapshot
            .contracts
            .iter()
            .map(|c| &c.fqn)
            .collect::<Vec<_>>()
    );

    let input = || ApiCheckInput {
        root: temp.root(),
        elements: &elements,
        contracts: &snapshot.contracts,
        schemas: &snapshot.schemas,
        symbols: &snapshot.symbols,
        enabled: true,
    };

    let first = check(input());
    let second = check(input());
    assert_eq!(first, second, "两次运行逐字段相等");

    let kinds: Vec<&str> = first.iter().map(|finding| finding.kind.as_str()).collect();
    let count = |kind: &str| kinds.iter().filter(|value| **value == kind).count();
    assert_eq!(count("apiEntryMissing"), 2, "两个无入口元素：{kinds:?}");
    assert_eq!(
        count("apiMissing"),
        2,
        "两个有入口无契约的元素（无入口者不再判 ②）：{kinds:?}"
    );
    assert_eq!(
        count("apiConstructorDeclared"),
        1,
        "契约声明构造器恰一条：{kinds:?}"
    );
    assert_eq!(first.len(), 5, "共五条命中：{kinds:?}");

    let keys: Vec<(String, Option<String>, Option<String>)> = first
        .iter()
        .map(|finding| {
            (
                finding.kind.as_str().to_owned(),
                finding.element.clone(),
                finding.target.clone(),
            )
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(
        keys, sorted,
        "返回序按 (kind, element, target) 升序：{keys:?}"
    );

    // 同 kind 内按 element 升序（乱序输入不影响输出）
    let entry_elements: Vec<Option<String>> = first
        .iter()
        .filter(|finding| finding.kind.as_str() == "apiEntryMissing")
        .map(|finding| finding.element.clone())
        .collect();
    assert_eq!(
        entry_elements,
        vec![
            Some("App.Core.NoEntryA".to_owned()),
            Some("App.Core.NoEntryB".to_owned())
        ],
        "同 kind 内按 element 字典序"
    );
}

/// 边界（AC-9）：`api::check(input)` 与 `api::check::check(input)` 的结果逐字段相等且条数
/// 相等（对外入口只做短路 + 排序）。
#[test]
fn check委托单点() {
    let temp = TempDir::new("check-delegates");
    let elements = [element("App.Core.NoEntry", &["src/missing/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");

    let build = || ApiCheckInput {
        root: temp.root(),
        elements: &elements,
        contracts: &snapshot.contracts,
        schemas: &snapshot.schemas,
        symbols: &snapshot.symbols,
        enabled: true,
    };
    let outer = check(build());
    let mut inner = super::check::check(build());
    inner.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then_with(|| left.element.cmp(&right.element))
            .then_with(|| left.target.cmp(&right.target))
    });
    assert_eq!(outer.len(), inner.len(), "条数相等");
    assert_eq!(outer, inner, "逐字段相等（对外入口零第二套判据）");
}

// ---------------------------------------------------------------------------
// bootstrap：草稿形态 / 零写盘 / 异常面
// ---------------------------------------------------------------------------

/// 正向（AC-12）：入口含 `pub fn a(&str) -> String` + `pub struct T` → 骨架文本 H1 = FQN、
/// 含两个空散文段、`### a` + `api-op` fence、一个 `api-types` fence 且键含 `T`。
#[test]
fn bootstrap草稿形态() {
    let temp = TempDir::new("bootstrap-draft");
    let fqn = "App.Core.Alpha";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
    ));
    temp.write(
        "src/alpha/src/lib.rs",
        "pub struct T;\npub fn a(input: &str) -> String { String::new() }\n",
    );

    let text = bootstrap(temp.root(), fqn).expect("起草应成功");
    assert!(text.starts_with(&format!("# {fqn}")), "H1 = FQN：\n{text}");
    assert!(text.contains("## 职能"), "含职能段：\n{text}");
    assert!(text.contains("## 不变量"), "含不变量段：\n{text}");
    assert!(text.contains("### a"), "含 op heading：\n{text}");
    assert!(text.contains("```json api-op"), "含 api-op fence：\n{text}");
    assert!(
        text.contains("```json api-types"),
        "含 api-types fence：\n{text}"
    );
    assert!(text.contains("\"T\""), "api-types 键含 T：\n{text}");
    assert!(
        text.contains("external") || text.contains("crateOnly") || text.contains("testOnly"),
        "每符号附 visibility 注释：\n{text}"
    );
}

/// 边界（AC-12）：调用前后 tempdir 全量字节快照相等（零写盘）；`entry_form` 全不命中 →
/// `Err(ApiError::Element(..))` 且文案含 FQN。
#[test]
fn bootstrap只出字符串() {
    let temp = TempDir::new("bootstrap-no-write");
    let fqn = "App.Core.Alpha";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
    ));
    temp.write(
        "src/alpha/src/lib.rs",
        "pub fn a() -> String { String::new() }\n",
    );

    let before = temp.snapshot();
    let _ = bootstrap(temp.root(), fqn).expect("起草应成功");
    assert_eq!(temp.snapshot(), before, "调用前后字节快照相等（零写盘）");

    let no_entry = "App.Core.NoEntry";
    temp.write_model(&format!(
        "[element.\"{no_entry}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/missing/\"]\n"
    ));
    let error = bootstrap(temp.root(), no_entry).expect_err("无入口应报错");
    assert!(
        matches!(error, ApiError::Element(_)),
        "无入口 → ApiError::Element，实际：{error:?}"
    );
    assert!(
        error.message().contains(no_entry),
        "文案含 FQN：{}",
        error.message()
    );
}

/// 边界（D19-b 留白 / S-10）：骨架文本中每符号的注记行只含 `visibility @ <entry>:<line>`
/// 形态，**零 `used_by` / 「引用方」字段**（本期留白，补齐需 import 反向图）；且草稿的
/// 候选符号集与 `extract_element` 的输出符号集逐项相等（共用取数面，零第二套抽取）。
#[test]
fn bootstrap不产used_by() {
    let temp = TempDir::new("bootstrap-no-used-by");
    let fqn = "App.Core.Alpha";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
    ));
    temp.write(
        "src/alpha/src/lib.rs",
        "pub struct Thing;\npub fn alpha(input: &str) -> String {\n    String::new()\n}\n",
    );

    let text = bootstrap(temp.root(), fqn).expect("起草应成功");

    // 否定断言：`used_by` / 「引用方」本期留白（S-10）
    assert!(
        !text.contains("used_by"),
        "骨架 MUST NOT 产出 `used_by`（D19-b 留白）：\n{text}"
    );
    assert!(
        !text.contains("引用方"),
        "骨架 MUST NOT 产出「引用方」字段：\n{text}"
    );
    assert!(
        !text.contains("usedBy"),
        "骨架 MUST NOT 产出 `usedBy`（camelCase 变体同禁）：\n{text}"
    );

    // 正例：注记行只含 `visibility @ <entry>:<line>` 形态，visibility 取自封闭词表
    let annotations: Vec<&str> = text
        .lines()
        .filter(|line| line.trim_start().starts_with("<!--"))
        .collect();
    assert!(!annotations.is_empty(), "每 op 附一条注记行：\n{text}");
    for annotation in &annotations {
        assert!(
            annotation.contains(" @ "),
            "注记行形态为 `visibility @ file:line`：{annotation}"
        );
        assert!(
            Visibility::ALL
                .iter()
                .any(|value| annotation.contains(value.as_str())),
            "注记行的 visibility 取自封闭四值词表：{annotation}"
        );
    }
    let type_notes: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("待剪枝："))
        .collect();
    assert!(!type_notes.is_empty(), "类型条目附位置注记：\n{text}");
    for note in &type_notes {
        assert!(
            note.contains(" @ ") && note.contains(':'),
            "类型注记含 `@ file:line` 位置线索：{note}"
        );
        assert!(
            Visibility::ALL
                .iter()
                .any(|value| note.contains(value.as_str())),
            "类型注记的 visibility 取自封闭四值词表：{note}"
        );
    }

    // 两向对拍：草稿候选符号集 ≡ extract_element 的符号集
    let symbols = extract::extract_element(temp.root(), "src/alpha/src/lib.rs");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(names, vec!["Thing", "alpha"], "取数面符号集：{names:?}");
    let ops: Vec<&str> = symbols
        .iter()
        .filter(|symbol| symbol.kind == extract::ApiSymbolKind::Function)
        .filter(|symbol| !crate::api::contract::is_constructor(&symbol.name))
        .map(|symbol| symbol.name.as_str())
        .collect();
    for op in &ops {
        assert!(
            text.contains(&format!("### {op}")),
            "草稿含 op `{op}`：\n{text}"
        );
    }
    for symbol in symbols
        .iter()
        .filter(|symbol| symbol.kind != extract::ApiSymbolKind::Function)
    {
        assert!(
            text.contains(&format!("\"{}\"", symbol.name)),
            "草稿 api-types 含取数面类型 `{}`：\n{text}",
            symbol.name
        );
    }
}

/// 异常：元素 FQN 不存在 / 空串 / 含 emoji → `Err(ApiError::Element(..))`，零 panic。
#[test]
fn bootstrap元素面异常() {
    let temp = TempDir::new("bootstrap-element-errors");
    let fqn = "App.Core.Alpha";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
    ));
    temp.write("src/alpha/src/lib.rs", "pub fn a() {}\n");

    for candidate in ["App.Nope", "", "App.Nope.🎯"] {
        match bootstrap(temp.root(), candidate) {
            Err(ApiError::Element(message)) => {
                assert!(!message.is_empty(), "错误文案非空（{candidate:?}）");
            }
            other => panic!("{candidate:?} 应报 Element 错误，实际：{other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// api_ref：双层投影口径
// ---------------------------------------------------------------------------

/// 正向（AC-10 / D22）：md 在案 → `status == Declared`；三路径均为 repo 相对 POSIX 串，
/// 且分别以 `.md` / `.schema.json` / `.i.rs` 结尾；`schema` / `.i` 尚未生成时仍给路径串。
#[test]
fn api_ref_declared三路径() {
    let temp = TempDir::new("api-ref-declared");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"crates/infra/archi/\"]\n"
    ));

    let reference = api_ref(temp.root(), fqn);
    assert_eq!(reference.status, ApiStatus::Declared, "md 在案 → declared");

    let contract_path = reference.contract_path.expect("契约路径应有");
    assert!(
        contract_path.ends_with(".md"),
        "契约路径以 .md 结尾：{contract_path}"
    );
    assert!(
        !contract_path.starts_with("./"),
        "零前导 ./：{contract_path}"
    );
    assert!(!contract_path.contains('\\'), "POSIX 分隔：{contract_path}");

    let schema_path = reference.schema_path.expect("schema 路径应有");
    assert!(
        schema_path.ends_with(".schema.json"),
        "schema 路径形态：{schema_path}"
    );

    let generated_path = reference.generated_path.expect("生成物路径应有");
    assert!(
        generated_path.ends_with(".i.rs"),
        "Rust 元素的生成物以 .i.rs 结尾：{generated_path}"
    );

    // `schema` 与 `.i` 尚未生成时仍给路径串（记录应生成物）
    assert!(
        !temp.root().join(&schema_path).exists(),
        "schema 尚未生成但路径已给"
    );
    assert!(
        !temp.root().join(&generated_path).exists(),
        "生成物尚未生成但路径已给"
    );
}

/// 边界（AC-10 / D22）：同一 root + fqn 下「按 FQN 自取 path」与「调用方直接给 path」
/// 两分支逐字段相等；`path` 空且 md 在案 → `generated_path = None` 而契约 / schema 路径
/// 仍 `Some`。
#[test]
fn api_ref两形态同源() {
    let temp = TempDir::new("api-ref-two-forms");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./crates/infra/archi/\"]\n"
    ));

    let from_model = api_ref(temp.root(), fqn);
    let direct = super::api_ref_of(temp.root(), fqn, &["crates/infra/archi/".to_owned()]);
    assert_eq!(from_model, direct, "两形态同源（投影语义不得变形）");

    // 容器元素（`path` 空）但 md 在案
    let container_fqn = "App.Backend.Infra";
    temp.write_contract(container_fqn, &pilot_markdown(container_fqn));
    let container = super::api_ref_of(temp.root(), container_fqn, &[]);
    assert_eq!(container.status, ApiStatus::Declared);
    assert!(container.generated_path.is_none(), "容器元素无生成物路径");
    assert!(container.contract_path.is_some(), "md 存在即可枚举契约");
    assert!(container.schema_path.is_some(), "schema 路径同源给出");
}

/// 边界：md 缺席 → `status == None` 且三路径全 `None`；单段 FQN（`App`）→ 目录退化
/// `Root`，路径可判。
#[test]
fn api_ref为none时三路径全空() {
    let temp = TempDir::new("api-ref-none");
    temp.write_model(
        "[element.\"App.Core.Agent\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/agent/\"]\n",
    );

    let reference = api_ref(temp.root(), "App.Core.Agent");
    assert_eq!(reference.status, ApiStatus::None, "md 缺席 → none");
    assert!(reference.contract_path.is_none(), "契约路径 None");
    assert!(reference.schema_path.is_none(), "schema 路径 None");
    assert!(reference.generated_path.is_none(), "生成物路径 None");

    // 单段 FQN：目录退化 Root
    let single = paths::contract_path(temp.root(), "App").expect("单段 FQN 仍合法");
    assert!(
        single.to_string_lossy().contains("Root"),
        "单段 FQN 退化 Root 目录：{single:?}"
    );
    let reference = super::api_ref_of(temp.root(), "App", &[]);
    assert_eq!(reference.status, ApiStatus::None);
}

/// 边界（AC-10 / D22）：`api_ref_of` 的元素形态边界——`element_path = []`（容器元素）+ md
/// 在案 → 契约 / schema 路径仍 `Some`、`generated_path = None`（与 `api_ref` 逐字段相等）；
/// 多段 `element_path` → 只取 `path[0]`（D2：`<root>` 取首条），结果与单段形态逐字相等；
/// `fqn` 与 `element_path` 冲突 → 文档路径按 FQN 派生、生成物路径按 path 派生（各自单点，
/// 不互相污染）。
#[test]
fn api_ref_of元素形态边界() {
    let temp = TempDir::new("api-ref-of-edges");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_contract(fqn, &pilot_markdown(fqn));

    let single = super::api_ref_of(temp.root(), fqn, &["crates/infra/archi/".to_owned()]);
    let multi = super::api_ref_of(
        temp.root(),
        fqn,
        &[
            "crates/infra/archi/".to_owned(),
            "crates/infra/archi/src/".to_owned(),
        ],
    );
    assert_eq!(single, multi, "多段只取 path[0]（D2），结果逐字相等");

    // fqn 与 element_path 冲突：文档路径按 FQN、生成物路径按 path
    let conflicting = super::api_ref_of(temp.root(), fqn, &["crates/core/other/".to_owned()]);
    assert_eq!(
        conflicting.status, single.status,
        "文档在案判定只看 FQN 派生的契约路径"
    );
    assert_eq!(
        conflicting.contract_path, single.contract_path,
        "契约路径按 FQN 派生（不受 element_path 影响）"
    );
    assert_eq!(
        conflicting.schema_path, single.schema_path,
        "schema 路径按 FQN 派生"
    );
    assert_ne!(
        conflicting.generated_path, single.generated_path,
        "生成物路径按 element_path 派生（两者各自单点，互不污染）"
    );
    assert!(
        conflicting
            .generated_path
            .as_deref()
            .is_some_and(|path| path.starts_with("crates/core/other.i.")),
        "生成物路径跟着 element_path 走（根同级形态）：{:?}",
        conflicting.generated_path
    );
}

/// 异常：`ApiError` 三值 `message()` 非空且互不相等；文案不外泄「检查已执行」语义。
#[test]
fn api_error三值与message() {
    let errors = [
        ApiError::Io("契约文件读取失败（.dev-team/.../X.md）：权限不足".to_owned()),
        ApiError::Contract("X.md:3: H1 标题不一致".to_owned()),
        ApiError::Element("元素不存在：App.Nope".to_owned()),
    ];
    let messages: Vec<String> = errors.iter().map(ApiError::message).collect();
    for message in &messages {
        assert!(!message.is_empty(), "文案非空");
        assert!(
            !message.contains("检查已执行"),
            "资产非法 ⇒ 检查不执行（文案不外泄执行语义）：{message}"
        );
    }
    for (index, left) in messages.iter().enumerate() {
        for right in &messages[index + 1..] {
            assert_ne!(left, right, "三值文案互不相等（错误面可辨）");
        }
    }
}

// ---------------------------------------------------------------------------
// 装配层辅助口径
// ---------------------------------------------------------------------------

/// 边界（D29）：`render_all` 输出的 `.i.rs` 是 `rustfmt` 正常形、`.i.ts` 一律单引号（判据
/// 以「零尾随空白 + 无 tab + 单引号数组」表达，不 spawn 格式化器）。
#[test]
fn render_all生成物为格式化器形态() {
    let temp = TempDir::new("render-all-format");
    temp.write_model(
        "[element.\"App.Backend.Infra.ArchiRuntime\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/infra/archi/\"]
",
    );
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    for file in &files {
        for line in file.content.lines() {
            assert_eq!(
                line,
                line.trim_end(),
                "零尾随空白（{:?}）：{line:?}",
                file.path
            );
            assert!(
                !line.contains('\t'),
                "零 tab 缩进（{:?}）：{line:?}",
                file.path
            );
        }
        assert!(
            file.content.ends_with('\n'),
            "末尾以换行收束：{:?}",
            file.path
        );
    }
}

/// 边界：`meta()` 三常量稳定（`tool_version` = 编译期常量、摘要 `sha256:<64 hex>` 且与
/// **独立重算** `common.json` 的 `sha256` 逐字相等、短形恰 12 位十六进制、方言常量 =
/// draft 2020-12、警示文案含「勿手改」语义）。
#[test]
fn meta三常量形态() {
    use sha2::{Digest, Sha256};

    let meta = meta();
    assert_eq!(meta.tool_version, env!("CARGO_PKG_VERSION"));
    assert!(meta.common_digest.starts_with("sha256:"), "摘要带算法前缀");

    // 独立对 common.json 原文重算 sha256（期望值不从被测函数取）
    let mut hasher = Sha256::new();
    hasher.update(super::COMMON_JSON.as_bytes());
    let expected: String = format!(
        "sha256:{}",
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(
        meta.common_digest, expected,
        "摘要与独立重算的 common.json sha256 逐字相等"
    );
    assert_eq!(
        meta.common_digest.len(),
        "sha256:".len() + 64,
        "sha256 十六进制 64 位"
    );

    let short = super::short_digest(&meta.common_digest);
    let hex = short
        .split_once(':')
        .map(|(_, hex)| hex)
        .expect("短形保留算法前缀");
    assert_eq!(hex.len(), 12, "短形恰 12 位：{short}");
    assert!(
        hex.chars().all(|character| character.is_ascii_hexdigit()),
        "短形为十六进制串：{short}"
    );

    assert_eq!(
        meta.schema_uri,
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert!(
        meta.generated_notice.contains("勿手改") && meta.generated_notice.contains("api:check"),
        "警示文案含「勿手改」与零 diff 门语义：{}",
        meta.generated_notice
    );
}

/// 边界（D12 幂等）：连续两次 `meta()` 逐字段相等（同载元即同生成物，`api:check` 零
/// diff 的前提）；`schema` / `emit` / `INDEX` 三处产出物都从**同一份载元**派生——`schema`
/// 的 `$comment` 逐字等于 `meta.generated_notice`、`emit` 与 `INDEX` 的工具版本行与摘要
/// 短串**逐字相同**（零第二份载元来源）。
#[test]
fn meta幂等且三处产出物载元一致() {
    let first = meta();
    let second = meta();
    assert_eq!(first, second, "同载元即同生成物（幂等）");
    assert_eq!(first.tool_version, second.tool_version);
    assert_eq!(first.common_digest, second.common_digest);
    assert_eq!(first.schema_uri, second.schema_uri);
    assert_eq!(first.generated_notice, second.generated_notice);

    let temp = TempDir::new("meta-shares");
    temp.write_model(
        "[element.\"App.Backend.Infra.ArchiRuntime\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"crates/infra/archi/\"]\n",
    );
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    let version_line = format!("archi-api {}", env!("CARGO_PKG_VERSION"));
    let digest = meta().common_digest;
    let (algorithm, hex) = digest.split_once(':').expect("摘要含算法前缀");
    let short = format!("{algorithm}:{}", &hex[..12]);

    let schema_file = files
        .iter()
        .find(|file| file.path == paths::schema_path(temp.root(), fqn).expect("合法 FQN"))
        .expect("schema 在案");
    assert!(
        schema_file
            .content
            .contains(&format!("\"$comment\": \"{}\"", meta().generated_notice)),
        "schema 的 $comment 逐字取同一份载元的 generated_notice：\n{}",
        schema_file.content
    );

    let index_file = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");
    let generated = paths::generated_path(
        temp.root(),
        elements[0].path.first().expect("非空 root"),
        paths::GeneratedKind::Rust,
    )
    .expect("Rust 生成物路径");
    let module_file = files
        .iter()
        .find(|file| file.path == generated)
        .expect(".i.rs 在案");

    for (label, text) in [
        ("INDEX", &index_file.content),
        (".i.rs", &module_file.content),
    ] {
        assert!(
            text.contains(&version_line),
            "{label} 含同一工具版本行：\n{text}"
        );
        assert!(
            text.contains(&short),
            "{label} 含同一摘要短串 `{short}`：\n{text}"
        );
    }
}

/// 异常（AC-12）：`meta()` 纯计算零 IO 零 spawn——调用前后 tempdir 全量字节快照相等；
/// 源码自证 `meta` 体内零 `fs::` / `read_to_string` / `std::process`（`common.json` 经
/// `include_str!` 编译期内嵌，不是运行期读盘）。
#[test]
fn meta零io零spawn() {
    let temp = TempDir::new("meta-no-io");
    temp.write(".dev-team/architecture/models/01-test.toml", "# 模型占位\n");
    temp.write(".dev-team/architecture/api/INDEX.toml", "# 索引占位\n");
    temp.write_contract("App.Core.Alpha", &pilot_markdown("App.Core.Alpha"));

    let before = temp.snapshot();
    let meta = meta();
    assert!(!meta.tool_version.is_empty());
    assert_eq!(
        temp.snapshot(),
        before,
        "meta() 调用前后字节快照相等（零 IO）"
    );

    let source = include_str!("mod.rs");
    // 函数体取「`pub fn meta` 起、到下一个 `pub fn` 之前」的窗口（不依赖行尾形态）
    let body = source
        .split_once("pub fn meta() -> ApiMeta {")
        .expect("meta 函数在案")
        .1
        .split_once("pub fn check(")
        .expect("紧随其后的下一个函数")
        .0;
    for forbidden in ["fs::", "read_to_string", "std::process", "Command::new"] {
        assert!(
            !body.contains(forbidden),
            "meta 体内零 `{forbidden}`（纯计算、零 spawn）：\n{body}"
        );
    }
    assert!(
        source.contains("include_str!(\"common.json\")"),
        "映射规则资产经 include_str! 编译期内嵌（不是运行期读盘）"
    );
    assert!(
        body.contains("Sha256") && body.contains("env!(\"CARGO_PKG_VERSION\")"),
        "载元由编译期常量 + sha2 摘要算出：\n{body}"
    );
}

/// 边界（D36 ⑤ 单点抽取）：`ref_name` 把任何 `$ref` 形态归一成**类型名**——同文件
/// `#/$defs/<名>` 与外部文件 `<相对前缀><FQN>.schema.json#/$defs/<名>` 都取 `$defs`
/// 之后那一段；无 `$defs` 片段时退化为路径末段；空串原样返回（不 panic）。
///
/// 该函数是 `check`（签名归一）与 `emit::{rust, ts}`（类型映射）的**唯一**切分点，
/// 故三处消费面在此两侧对拍：同一 `$ref` 在判据面与渲染面得到同一名字。
#[test]
fn ref_name归一类型名单点() {
    let same_file = "#/$defs/AgentInput";
    let external = "../Core/App.Backend.Core.Agent.schema.json#/$defs/AgentInput";
    assert_eq!(super::ref_name(same_file), "AgentInput");
    assert_eq!(super::ref_name(external), "AgentInput", "外部形态取末段");
    assert_eq!(
        super::ref_name(same_file),
        super::ref_name(external),
        "两种形态归一为同一名字（同一单点）"
    );

    // 无 `$defs` 片段 → 路径末段；单段串原样；空串不 panic
    assert_eq!(super::ref_name("a/b/c"), "c");
    assert_eq!(super::ref_name("Plain"), "Plain");
    assert_eq!(super::ref_name(""), "");

    // 源码自证：`rsplit_once(DEFS_FRAGMENT)` 恰一处（禁第二份切分）
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("rsplit_once(DEFS_FRAGMENT)").count(),
        1,
        "`$ref` → 名字的切分点单点"
    );
    assert_eq!(
        source.matches("pub(crate) fn ref_name").count(),
        1,
        "定义恰一处"
    );
    for (file, text) in [
        ("check.rs", include_str!("check.rs")),
        ("emit/rust.rs", include_str!("emit/rust.rs")),
        ("emit/ts.rs", include_str!("emit/ts.rs")),
    ] {
        assert_eq!(
            text.matches("fn ref_name").count(),
            0,
            "{file} 零本地 `ref_name` 定义（只引用单点）"
        );
        assert!(
            !text.contains("rsplit_once(DEFS_FRAGMENT)") && !text.contains("\"#/$defs/\""),
            "{file} 零第二份 `$defs` 切分"
        );
    }
}

/// 边界（D36 ⑤ 单点抽取）：`reexport_target` 只对**外部文件引用**返回目标元素 FQN；
/// 同文件引用（`#/$defs/<名>`）、无 `.schema.json` 后缀、无 `#` 片段、空串 → `None`
/// （同文件条目不是透传，零猜测）。
#[test]
fn reexport_target只认外部文件引用() {
    assert_eq!(
        super::reexport_target("../Core/App.Backend.Core.Agent.schema.json#/$defs/AgentInput"),
        Some("App.Backend.Core.Agent".to_owned()),
        "外部形态取中间段的目标 FQN"
    );
    for reference in [
        "#/$defs/AgentInput",
        "../Core/App.Backend.Core.Agent#/$defs/AgentInput",
        "../Core/App.Backend.Core.Agent.schema.json",
        "",
    ] {
        assert_eq!(
            super::reexport_target(reference),
            None,
            "非外部文件引用 → None：{reference:?}"
        );
    }

    // 源码自证：后缀常量单点持有（`SCHEMA_FILE_SUFFIX` 与 `paths::schema_path` 同源）
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("strip_suffix(SCHEMA_FILE_SUFFIX)").count(),
        1,
        "后缀剥法单点"
    );
    for (file, text) in [
        ("check.rs", include_str!("check.rs")),
        ("emit/rust.rs", include_str!("emit/rust.rs")),
        ("emit/ts.rs", include_str!("emit/ts.rs")),
    ] {
        assert!(
            !text.contains("strip_suffix(\".schema.json\")"),
            "{file} 零第二份后缀切分"
        );
    }
}

/// 边界（D36 ⑤ 单点抽取）：`kind_of` 的六值词表与 JSON 值的六个变体一一对应——
/// 契约解析与 schema 装配共用同一份词表（错误文案里的「实际为 X」由此而来）；
/// 非 JSON 值不可表达，故词表封闭即完备。
#[test]
fn kind_of六值词表() {
    use serde_json::json;

    let cases = [
        (json!(null), "null"),
        (json!(true), "布尔"),
        (json!(1), "数字"),
        (json!("s"), "字符串"),
        (json!([]), "数组"),
        (json!({}), "对象"),
    ];
    assert_eq!(cases.len(), 6, "JSON 六变体全覆盖");
    for (value, expected) in cases {
        assert_eq!(super::kind_of(&value), expected, "值 {value} 的类型名");
    }

    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("pub(crate) fn kind_of").count(),
        1,
        "定义恰一处"
    );
    for (file, text) in [
        ("contract.rs", include_str!("contract.rs")),
        ("schema.rs", include_str!("schema.rs")),
    ] {
        assert_eq!(
            text.matches("kind_of(").count(),
            text.matches("super::kind_of(").count()
                + text.matches("kind_of(&other)").count()
                + text.matches("kind_of(target)").count()
                + text.matches("kind_of(definition)").count()
                + text.matches("kind_of(nested)").count(),
            "{file} 内 `kind_of` 只以单点引用形态出现（零本地副本）"
        );
        assert!(
            !text.contains("Value::Null => \"null\""),
            "{file} 零第二份词表"
        );
    }
}

/// 边界：`short_digest` 短形单点（`sha256:<hex>` → 前 12 位；无前缀形态取前 12 字符）。
#[test]
fn short_digest短形() {
    let digest = "sha256:0123456789abcdef0123456789abcdef";
    assert_eq!(super::short_digest(digest), "sha256:0123456789ab");
    assert_eq!(super::short_digest("0123456789abcdef"), "0123456789ab");
}

/// 正向（**D55 / AC-33 ①**）：`display` 剥 `root` 前缀、`\` 归一为 `/`、剥前导 `./`；
/// `strip_prefix` 失败即 `unwrap_or(path)`（不 panic、不造相对化）。
#[test]
fn display路径归一语义() {
    let root = Path::new("/ws");
    assert_eq!(
        super::display(root, &root.join("a/b")),
        "a/b",
        "剥 root 前缀（repo 相对 POSIX 串）"
    );
    assert_eq!(
        super::display(root, Path::new(r"/ws/a\b")),
        "a/b",
        "Windows 分隔符归一为 `/`"
    );
    assert_eq!(
        super::display(root, Path::new("/ws/./a/b.md")),
        "a/b.md",
        "前导 `./` 剥净"
    );
    assert_eq!(
        super::display(root, Path::new("/other/a.md")),
        "/other/a.md",
        "root 外路径原样（不造相对化、不 panic）"
    );
}

/// 边界（str 边界）：`path == root` → 空串；`root` 为 `/` 或 `""` → 结果可判、不越界
/// 切片；非 ASCII / emoji / 尾斜杠 → 原样保留（零归一化）；超长路径不 panic。
#[test]
fn display边界值() {
    let root = Path::new("/ws");
    assert_eq!(super::display(root, root), "", "path == root → 空串");
    assert_eq!(
        super::display(Path::new("/"), Path::new("/a/b")),
        "a/b",
        "root = `/` 可判"
    );
    assert_eq!(
        super::display(Path::new(""), Path::new("/a/b")),
        "/a/b",
        "root = 空串可判（不越界切片）"
    );
    assert_eq!(
        super::display(Path::new("/other"), Path::new("x/🎯/中文/")),
        "x/🎯/中文/",
        "非 ASCII / emoji / 尾斜杠原样保留（零归一化）"
    );
    let long = format!("/other/{}", "a".repeat(1200));
    assert_eq!(
        super::display(Path::new("/ws"), Path::new(&long)),
        long.replace('\\', "/"),
        "超长路径不 panic"
    );
}

/// 源码自证（**D55 / AC-33 ①** 跨 crate 单点）：`api/mod.rs` 内 `pub fn display` 恰 1
/// （不再是 `pub(crate)`）；`lib.rs` 的 `pub use api::display` 成行恰 1（分组块会令判据
/// 恒 0）；工具侧行首 `fn display` 零命中、`archi::display` ≥ 1；`short_digest` 仍
/// `pub(crate)`（不随 D55 扩大出口面）。
#[test]
fn display跨crate单点面() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("pub fn display").count(),
        1,
        "`display` 是 `pub`（D55 的跨 crate 单点）"
    );
    assert_eq!(
        source.matches("pub(crate) fn display").count(),
        0,
        "不再是 `pub(crate)`"
    );
    let lib = include_str!("../lib.rs");
    assert_eq!(
        lib.matches("pub use api::display").count(),
        1,
        "crate 根出口 `pub use api::display` 成行恰 1（分组块会让判据恒 0）"
    );

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tools_main = manifest.join("../../tools/archi-api-gen/src/main.rs");
    let tool_source = fs::read_to_string(&tools_main).expect("读 CLI 源文");
    assert_eq!(
        tool_source
            .lines()
            .filter(|line| line.starts_with("fn display"))
            .count(),
        0,
        "工具侧零本地 `fn display`（D55：本地逐字实现已删）"
    );
    assert!(
        tool_source.matches("archi::display").count() >= 1,
        "调用面改指 `archi::display`"
    );
    assert_eq!(
        source.matches("pub(crate) fn short_digest").count(),
        1,
        "`short_digest` 仍 `pub(crate)`（不随 D55 扩大出口面）"
    );
}

/// 边界：`parse` + `build_schema` + `verify` + `verify_defs` 的整链在试点形态上通过（跨模块
/// 组合用例挂靠本节）。
#[test]
fn 整链路装配通过() {
    let meta = meta();
    let contract = parse(
        &pilot_markdown(FQN_PILOT),
        FQN_PILOT,
        &format!("Infra/{FQN_PILOT}.md"),
    )
    .expect("解析应成功");
    let schema: Value = super::schema::build_schema(&contract, &meta).expect("装配应成功");
    super::contract::verify(&contract, &schema).expect("双向 gate 应通过");
    super::schema::verify_defs(&schema).expect("$defs 规则应通过");
    assert_eq!(
        schema["$id"],
        serde_json::json!(format!("{FQN_PILOT}.schema.json"))
    );
}

/// 试点 FQN（整链用例）。
const FQN_PILOT: &str = "App.Backend.Infra.ArchiRuntime";

// ---------------------------------------------------------------------------
// 装配面的四参单形（D46 / AC-24 ③）与关键字目标的拒绝（D48 / D50 / AC-27）
// ---------------------------------------------------------------------------

/// 边界（**D46 / AC-24 ③** 源码自证）：`render_all` 只经 `emit::render_target` 分派——
/// 本文件内 `emit::{rust,ts}::render` **零直连**；`render_target(` 的实参形态为四元且
/// `target` 在首位（与 `emit/mod.rs` 的签名一致，MUST NOT 主张与渲染器同序）。
#[test]
fn render_all按四参单形调render_target() {
    let source = include_str!("mod.rs");
    for direct in ["rust::render(", "ts::render("] {
        assert!(
            !source.contains(direct),
            "本文件零 `{direct}` 直连（批量入口只经 `render_target`）"
        );
    }
    assert_eq!(
        source.matches("emit::render_target(").count(),
        1,
        "`emit::render_target(` 恰 1 处调用（唯一分派入口）"
    );

    // 实参形态：`&target, contract, schema_value, &meta`（`target` 首位，四元）
    let start = source.find("emit::render_target(").expect("分派调用在案");
    let call = &source[start..];
    let end = call.find(')').expect("调用收尾");
    let call = &call[..end];
    for (index, argument) in ["&target", "contract", "schema_value", "&meta"]
        .iter()
        .enumerate()
    {
        assert!(
            call.contains(argument),
            "第 {} 个实参 `{argument}` 在案：{call}",
            index + 1
        );
    }
    let order: Vec<usize> = ["&target", "contract", "schema_value", "&meta"]
        .iter()
        .map(|argument| call.find(argument).expect("实参在案"))
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "实参序为 `(target, contract, schema, meta)`（`target` 首位）：{call}"
    );
}

/// 异常（**D48 / D50 / AC-27**）：透传目标的元素根末段是 Rust 关键字（`loop`）⇒ 该目标
/// **不入** `EmitTarget.modules`，`.i.rs` 产物只含 `// 透传自 <FQN>` 注释而**不含**
/// `pub use`（`crate::loop` 是不可编译文本）；对照合法末段（`foundation`）⇒ 入表且
/// `name` / `module_path` 与 `paths::{module_name, rust_module_path}` 逐字相等。
#[test]
fn render_all拒绝关键字末段的rust透传目标() {
    let temp = TempDir::new("render-all-keyword-target");
    let consumer = "App.Backend.Infra.ArchiRuntime";
    let keyword_target = "App.Backend.Core.Loop";
    let valid_target = "App.Backend.Core.Foundation";
    temp.write_model(&format!(
        "[element.\"{consumer}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/infra/archi/\"]\n\n\
         [element.\"{keyword_target}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/core/loop/\"]\n\n\
         [element.\"{valid_target}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/core/foundation/\"]\n"
    ));
    temp.write_contract(
        consumer,
        &format!(
            "# {consumer}\n\n## 职能\n\n正文。\n\n\
             ```json api-reexport\n{{\"Loop\": \"{keyword_target}\", \
             \"Base\": \"{valid_target}\"}}\n```\n\n\
             ## 透传\n\n- Loop → {keyword_target}\n- Base → {valid_target}\n\n\
             ```json api-types\n{{\"Thing\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
        ),
    );
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [
        element(consumer, &["crates/infra/archi/"]),
        element(keyword_target, &["crates/core/loop/"]),
        element(valid_target, &["crates/core/foundation/"]),
    ];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    let generated = paths::generated_path(
        temp.root(),
        "crates/infra/archi/",
        paths::GeneratedKind::Rust,
    )
    .expect("Rust 生成物路径");
    let file = files
        .iter()
        .find(|file| file.path == generated)
        .expect("消费方的 .i.rs 在案");

    // 前提：`loop` 是 Rust 关键字 ⇒ 不是合法模块段
    assert!(
        !paths::is_module_segment("loop"),
        "`loop` 是 Rust 关键字 ⇒ 不是合法模块段（本用例的前提）"
    );
    assert!(
        file.content
            .contains(&format!("// 透传自 {keyword_target}")),
        "关键字末段的目标只留透传注释（FQN 原文）：\n{}",
        file.content
    );
    assert!(
        !file.content.contains("pub use crate::loop"),
        "MUST NOT 产出 `pub use crate::loop::…`（不可编译文本）：\n{}",
        file.content
    );

    // 对照：合法末段入表且两值与 `paths` 单点逐字相等
    let expected_name = paths::module_name("crates/core/foundation/");
    let expected_path = paths::rust_module_path("crates/core/foundation/");
    assert_eq!(expected_name, "foundation");
    assert_eq!(expected_path, "crate::foundation");
    assert!(
        file.content.contains("pub use crate::foundation::Base;"),
        "合法末段仍走 `pub use <module_path>::<name>`（闸只拒关键字，不误伤）：\n{}",
        file.content
    );
}

/// 正向（**D58 / AC-34 ①**）：`render_all` 内 `EmitTarget {` 字面量恰 1 处且实参为**三值**
/// `kind` / `form` / `modules`（`path` / `element_root` 零命中）；`path` 与 `scope` 保留为
/// **局部变量**并直接进 `GeneratedFile`；行为面：同一快照两次调用逐字段相等（删两字段零
/// 可观测影响）。
#[test]
fn render_all的emit_target三值构造() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("EmitTarget {").count(),
        1,
        "`EmitTarget {{` 字面量恰 1 处（`render_all` 的三值构造）"
    );
    let body = source
        .split_once("pub fn render_all(")
        .expect("render_all 在案")
        .1
        .split_once("\n}\n")
        .expect("函数体收口")
        .0;
    for field in ["kind,", "form,", "modules: module_refs("] {
        assert!(body.contains(field), "三值构造的 `{field}` 在案：\n{body}");
    }
    // `EmitTarget` 字面量本身只三值（`INDEX` 的 `path:` 属 `GeneratedFile`，不在此列）
    let literal = body
        .split_once("let target = EmitTarget {")
        .expect("三值构造字面量在案")
        .1
        .split_once("};")
        .expect("字面量收口")
        .0;
    for removed in ["path:", "element_root"] {
        assert!(
            !literal.contains(removed),
            "`EmitTarget` 的 `{removed}` 已删（D58）：
{literal}"
        );
    }
    // `path` / `scope` 仍是局部变量（落点与派生入参，不成为渲染器入参）
    assert!(
        body.contains("let scope = element.path.first()"),
        "`scope` 是局部变量：\n{body}"
    );
    assert!(
        body.contains("let Some(path) = paths::generated_path(root, scope, kind)"),
        "`path` 是局部变量且 `scope` 只作派生入参：\n{body}"
    );
    assert!(
        body.contains("GeneratedFile { path, content }"),
        "`path` 直接进 `GeneratedFile`（不经渲染目标）：\n{body}"
    );

    // 行为面：两次调用逐字段相等（两字段若被产品面读取，删后行为不可能不变）
    let temp = TempDir::new("render-all-three-fields");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/infra/archi/\"]\n"
    ));
    temp.write_contract(fqn, &pilot_markdown(fqn));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let first = render_all(temp.root(), &snapshot).expect("首次渲染");
    let second = render_all(temp.root(), &snapshot).expect("二次渲染");
    assert_eq!(first, second, "同一快照两次渲染逐字段相等");
}

/// 边界（**D58** 行为面）：产物文本只随 `kind` / `form` / `modules` 变化——固定契约下
/// `form` 决定 specifier（`SingleFile` ⇒ `./<name>.i`、`SubdirModule` ⇒ `../<name>.i`，
/// 两值均取自 `paths::ts_import_specifier`），`modules` 非空令透传行出现；`path` /
/// `element_root` 已不在结构上，编译器层面即排除该变量。
#[test]
fn render_all与emit_target三字段协同() {
    let temp = TempDir::new("render-all-three-fields-coop");
    let consumer = "App.Frontend.Views.Consumer";
    let target = "App.Frontend.Components.Agent.Input";
    let model = format!(
        "[element.\"{consumer}\"]
kind = \"module\"
gate = \"warn\"
         path = [\"views/consumer.tsx\"]

         [element.\"{target}\"]
kind = \"module\"
gate = \"warn\"
         path = [\"views/agent/Input/\"]
"
    );
    temp.write_model(&model);
    temp.write_contract(
        consumer,
        &format!(
            "# {consumer}

## 职能

正文。

             ```json api-reexport
{{\"AgentInput\": \"{target}\"}}
```

             ## 透传

- AgentInput → {target}

             ```json api-types
{{\"Thing\": {{\"type\": \"object\", \"properties\": {{}}}}}}
```
"
        ),
    );
    temp.write(
        "views/consumer.tsx/index.ts",
        "export function view(): string { return ''; }
",
    );

    // 变量一：`form`（由盘上入口求出，渲染器零推导）——`index.ts` 命中 ⇒ `../<name>.i`
    let with_target = [
        element(consumer, &["views/consumer.tsx"]),
        element(target, &["views/agent/Input/"]),
    ];
    let form = paths::entry_form(temp.root(), &with_target[0])
        .map(|(form, _)| form)
        .expect("入口在案");
    assert_eq!(
        form,
        paths::EntryForm::SubdirModule,
        "`form` 来自 `paths::entry_form`（D41）"
    );
    let name = paths::module_name("views/agent/Input/");
    let expected = paths::ts_import_specifier(form, &name);
    let snapshot = load_contracts(temp.root(), &with_target).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    let generated = files
        .iter()
        .find(|file| file.path.extension().is_some_and(|ext| ext == "ts"))
        .expect("TS 生成物在案");
    assert!(
        generated.content.contains(&format!("from '{expected}'")),
        "specifier 只由 `form` + `ModuleRef.name` 决定：
{}",
        generated.content
    );
    assert_ne!(
        expected,
        paths::ts_import_specifier(paths::EntryForm::SingleFile, &name),
        "另一形态的单点值不同（`form` 是唯一差异源）"
    );
    assert!(
        generated
            .content
            .contains("export type { AgentInput } from"),
        "`modules` 非空 ⇒ 透传行出现：
{}",
        generated.content
    );

    // 变量二：`modules`（目标元素不在模型里 ⇒ 查表空 ⇒ 只出注释、不产 specifier）
    //
    // 元素表由 `render_all` 经 `dto::graph_view` 从模型单点取数（入参数组不作数），故换一
    // 棵只声明消费方的树——同一契约、零目标元素。
    let bare_temp = TempDir::new("render-all-three-fields-coop-bare");
    bare_temp.write_model(&format!(
        "[element.\"{consumer}\"]
kind = \"module\"
gate = \"warn\"
         path = [\"views/consumer.tsx\"]
"
    ));
    bare_temp.write_contract(
        consumer,
        &format!(
            "# {consumer}

## 职能

正文。

             ```json api-reexport
{{\"AgentInput\": \"{target}\"}}
```

             ## 透传

- AgentInput → {target}

             ```json api-types
{{\"Thing\": {{\"type\": \"object\", \"properties\": {{}}}}}}
```
"
        ),
    );
    bare_temp.write(
        "views/consumer.tsx/index.ts",
        "export function view(): string { return ''; }
",
    );
    let bare_elements = [element(consumer, &["views/consumer.tsx"])];
    let bare = load_contracts(bare_temp.root(), &bare_elements).expect("装载应成功");
    let bare_files = render_all(bare_temp.root(), &bare).expect("渲染应成功");
    let bare_generated = bare_files
        .iter()
        .find(|file| file.path.extension().is_some_and(|ext| ext == "ts"))
        .expect("TS 生成物在案");
    assert!(
        bare_generated
            .content
            .contains(&format!("// 透传自 {target}")),
        "查表为空 ⇒ 只出注释（不退化到 FQN 末段）：
{}",
        bare_generated.content
    );
    assert!(
        !bare_generated.content.contains(" from '"),
        "查表为空 ⇒ 零 specifier：
{}",
        bare_generated.content
    );
}

/// 边界（**D60 同口径的出口面射程**，design「修改文件」表 `api/mod.rs` 行）：`api/mod.rs`
/// 的 `pub use` 面**整体归零**——出口面的唯一落点是 crate 根的 `lib.rs` 逐名出口。
///
/// 判据串锚**全串**行首 `pub use `，不是单名复合串：`category_of` 一行删掉之后，单名判据
/// 对「兄弟名被加回」与「分组块形态加回」两类回归均不可见（前者是单一子串、后者连子串都
/// 对不上）⇒ 空转。判据力由本用例的两条注入自证承担（与 `runner_test` 的全串 `pub use`
/// 先例同型）。
#[test]
fn api出口面归零且判据可判红() {
    /// 行首 `pub use ` 计数（`grep -cE '^pub use '` 的等价求值）。
    fn line_start_pub_use(text: &str) -> usize {
        text.lines()
            .filter(|line| line.starts_with("pub use "))
            .count()
    }

    let source = include_str!("mod.rs");
    assert_eq!(
        line_start_pub_use(source),
        0,
        "`api/mod.rs` 零 `pub use`（出口面唯一落点是 crate 根的逐名出口）"
    );

    // 判据力自证 ①：单名形态被加回 ⇒ 本判据必红（旧形态的单名判据只在这一点上重合）
    let reexported = format!("{source}pub use check::category_of;\n");
    assert_eq!(
        line_start_pub_use(&reexported),
        1,
        "单名出口加回即命中（判据非空转）"
    );

    // 判据力自证 ②：分组块形态同样命中（单名复合串锚不上这一形态 ⇒ 全串锚是唯一有效形态）
    let grouped = format!("{source}pub use contract::{{ContractOp, ContractSection}};\n");
    assert_eq!(
        line_start_pub_use(&grouped),
        1,
        "分组块形态同样命中（全串锚的唯一性）"
    );

    // 九名零消费者不在**行首 `use` 声明面**（逐名与分组块两形态皆覆盖；限定路径调用点如
    // `schema::build_schema(` 不是 import 面，正是「走模块路径」的形态本身）
    for removed in [
        "ContractOp",
        "ContractSection",
        "schema_type",
        "signature_of",
        "ApiParam",
        "Position",
        "Visibility",
        "build_schema",
        "verify_defs",
    ] {
        assert!(
            !source
                .lines()
                .any(|line| line.starts_with("use ") && line.contains(removed)),
            "`{removed}` 不在行首 `use` 声明面（零消费者，D60 同口径）"
        );
    }

    // 反面锚：出口面本身没丢，只是搬到了 crate 根——收窄不是缩小可用面
    let lib = include_str!("../lib.rs");
    assert!(
        lib.contains("pub use api::{"),
        "出口面唯一落点在 crate 根（逐名成组的 `pub use api::{{…}}`）"
    );
    assert!(
        lib.contains("pub use api::display"),
        "跨 crate 单点仍由 crate 根出口（D55）"
    );
}

/// 边界（**D39 / D52**）：`bootstrap` 不读 knip、不产 `unused`——源码自证 `api/mod.rs` 内
/// `knip` **零命中**、只经 `extract_element` 取数（`extract::extract(` 零调用）；行为面：
/// 草稿文本内零 `unused` 词（`Visibility::Unused` 生产面不可达）。
#[test]
fn bootstrap不读knip且不产unused() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("knip").count(),
        0,
        "`api/mod.rs` 内零 `knip`（降级接口本期未接线，D39）"
    );
    assert_eq!(
        source.matches("extract::extract(").count(),
        0,
        "`api/mod.rs` 内零 `extract::extract(` 调用（唯一取数面是 `extract_element`）"
    );
    assert!(
        source.contains("extract::extract_element("),
        "取数面经 `extract_element`（判据与草稿同源）"
    );

    // 行为面：草稿文本零 `unused` 词
    let temp = TempDir::new("bootstrap-no-unused");
    let fqn = "App.Core.Alpha";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
    ));
    temp.write(
        "src/alpha/src/lib.rs",
        "pub struct T;\npub fn a(input: &str) -> String { String::new() }\n",
    );
    let text = bootstrap(temp.root(), fqn).expect("起草应成功");
    assert!(
        !text.contains("unused"),
        "草稿文本零 `unused`（`Visibility::Unused` 生产面不可达）：\n{text}"
    );
}

// ---------------------------------------------------------------------------
// 跨语言映射规则资产（D54 / D57）：唯一解析点 + 协议键换算
// ---------------------------------------------------------------------------

/// `parse_common` 正向：资产原样解析出协议名 → 双语言槽位 + 可选留痕。
#[test]
fn parse_common解析资产为协议名到语言槽位() {
    let rules = super::parse_common(super::COMMON_JSON).expect("真实资产应解析成功");
    let outcome = rules
        .get("Outcome")
        .expect("资产键 = 协议名字 `Outcome`（D57 ①）");
    assert!(
        outcome.rust.contains("{}"),
        "rust 槽位含 `{{}}` 占位符：{}",
        outcome.rust
    );
    assert!(
        outcome.typescript.contains("{}"),
        "typescript 槽位含 `{{}}` 占位符：{}",
        outcome.typescript
    );
    assert_eq!(
        outcome.degraded.as_deref(),
        Some("typescript"),
        "错误通道在 TS 类型系统外 ⇒ 记 degraded"
    );
    assert!(outcome.note.is_some(), "注记在场");
}

/// `parse_common` 的四种坏形态各自错误且文案点名键与语言。
#[test]
fn parse_common四类坏形态各自错误并点名() {
    let cases: [(&str, &str, &str); 4] = [
        ("非法 JSON", "{", "不是合法 JSON"),
        ("顶层非对象", "[]", "顶层必须是对象"),
        (
            "缺语言槽位",
            r#"{"Outcome": {"rust": "Result<{}, String>"}}"#,
            "typescript",
        ),
        (
            "槽内零占位符",
            r#"{"Outcome": {"rust": "Result", "typescript": "{}"}}"#,
            "占位符",
        ),
    ];
    for (label, source, expected) in cases {
        let error = super::parse_common(source).expect_err(label);
        let message = error.message();
        assert!(
            message.contains(expected),
            "{label}：文案应含 `{expected}`，实得：{message}"
        );
        assert!(message.contains("common.json"), "{label}：文案点名资产");
    }
}

/// 缺语言的文案 MUST 点名**具体**那门语言（不写「缺槽位」了事）。
#[test]
fn parse_common缺槽位文案点名该语言() {
    let missing_rust =
        super::parse_common(r#"{"Outcome": {"typescript": "{}"}}"#).expect_err("缺 rust");
    assert!(
        missing_rust.message().contains("rust"),
        "文案点名缺的语言：{}",
        missing_rust.message()
    );
}

/// `protocol_key` 换算单点：资产键（协议名）→ 承载键（小写）。
#[test]
fn protocol_key协议名换承载键() {
    assert_eq!(super::protocol_key("Outcome"), "outcome");
    assert_eq!(super::protocol_key("outcome"), "outcome");
}

/// 资产键 MUST NOT 被改成小写：`Outcome` 在用、`outcome` 不在（D57 ①）。
#[test]
fn 资产键为协议名字而非小写承载键() {
    let rules = super::parse_common(super::COMMON_JSON).expect("资产应解析成功");
    assert!(rules.contains_key("Outcome"), "资产键 = 协议名字");
    assert!(
        !rules.contains_key("outcome"),
        "资产键 MUST NOT 是小写承载键（改成小写即与 ApiType::Outcome / #/$defs/Outcome 失配）"
    );
    assert_eq!(
        super::protocol_key("Outcome"),
        "outcome",
        "两套口径经换算单点连接"
    );
}

/// `meta()` 携带映射表（D54 ②）：`ApiMeta.common` 与 `parse_common` 同源。
#[test]
fn meta携带映射表且与解析点同源() {
    let meta = meta();
    let parsed = super::parse_common(super::COMMON_JSON).expect("资产应解析成功");
    assert_eq!(
        meta.common, parsed,
        "载元里的规则表与唯一解析点逐项相等（零第二份来源）"
    );
    assert!(!meta.common.is_empty(), "资产非空");
}

/// `load_contracts` 复核同一份资产（D54 ②）：形态坏即 `ApiError::Contract`。
#[test]
fn load_contracts的资产复核与解析点共用一函数() {
    let source = include_str!("mod.rs");
    let body = source
        .split_once("pub fn load_contracts(")
        .expect("load_contracts 在案")
        .1
        .split_once("\n}\n")
        .expect("函数体收口")
        .0;
    assert!(
        body.contains("parse_common(COMMON_JSON)?"),
        "装载期以同一 `parse_common` 复核资产形态（静默跳过不允许）：\n{body}"
    );
}
