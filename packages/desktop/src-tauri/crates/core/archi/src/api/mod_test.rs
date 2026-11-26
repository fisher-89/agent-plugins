//! `api`（装配层）的单元测试：接口面启用判定（`declared` ∧ `path` 非空 ∧
//! `<叶段>.schema.json` 在盘）、`load_contracts` 全链（schema 校验 → `Contract` 投影 →
//! 三表装配）、`render_all` 三类生成物 / 幂等 / `INDEX` 覆盖与 `categories` 形状分布、
//! `check` 的未启用短路与稳定排序、`bootstrap` 的双草稿形态与零写盘、`api_ref` 的三路径
//! 独立投影、`ApiError` 三值文案。
//!
//! 零 mock：自建 `TempDir` RAII + `AtomicU64`（本 crate 无 dev-dependency）、真实
//! `foundation::layout` 派生、真实校验 / 投影 / 装配 / 判据 / 渲染 / 取数组合——编排语义即被测
//! 对象，替身会引入第二事实源。
//!
//! IO 错误面以「非法 UTF-8 字节的接口文件」注入，不改权限；「接口未写」（schema 不在盘）
//! 不进任何清单（判据面零对象）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_models_dir;
use serde_json::Value;

use crate::api::contract::Contract;
use crate::api::extract;
use crate::api::paths;
use crate::drift::report::{ArchiApiFinding, ArchiApiFindingKind};
use crate::dto::{ApiStatus, ArchiElement, ElementKind, GateTier, NodeMark};

use super::{
    api_ref, bootstrap, check, enabled, findings, load_contracts, meta, render_all, ApiCheckInput,
    ApiError, ApiSnapshot,
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
        self.write_absolute(&self.0.join(relative), content);
    }

    /// 落一个绝对路径文件（应生成物 / 越根形态语料用；自动建父目录）。
    fn write_absolute(&self, path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(path, content).expect("写文件失败");
    }

    /// 落模型（`.dev-team/architecture/graph/model/01-test.toml`）。
    ///
    /// `content` 只写叶元素；祖先链由 [`declare_ancestors`] 自动补齐——`elements_of`
    /// 经 `dto::graph_view` 取数，模型缺祖先即 `orphanHierarchy` 结构错（fail-closed），
    /// 而本文件的历史夹具清一色只声明叶元素。
    fn write_model(&self, content: &str) {
        self.write(
            ".dev-team/architecture/graph/model/01-test.toml",
            &declare_ancestors(content),
        );
    }

    /// 落契约（`<archi_models_dir>/<分层目录…>/<叶段>.md`）。
    fn write_contract(&self, fqn: &str, content: &str) {
        let path = paths::contract_path(self.root(), fqn).expect("测试 FQN 恒为合法形态");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建契约目录失败");
        }
        fs::write(&path, content).expect("写契约失败");
    }

    /// 落接口定义（`<archi_models_dir>/<分层目录…>/<叶段>.schema.json`）。
    fn write_schema(&self, fqn: &str, content: &str) {
        let path = paths::schema_path(self.root(), fqn).expect("测试 FQN 恒为合法形态");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建接口目录");
        }
        fs::write(&path, content).expect("写接口定义失败");
    }

    /// 落**两资产**（接口 + 文档）：文档只为「在盘」这一独立事实（`docMissing` 的反面），
    /// 接口取试点形态的手写 schema 文本。
    ///
    /// 渲染 / 判据用例的输入面是接口定义，同时需要「文档在盘」这一独立事实，故两资产成对
    /// 落盘；两者互不派生（读面只认 schema，文档正文零回落）。
    fn write_pair(&self, fqn: &str) {
        self.write_contract(fqn, &doc_of(fqn));
        self.write_schema(fqn, &pilot_schema(fqn));
    }

    /// 落两资产，接口面换成给定 schema 文本（文档仍取 [`doc_of`]）。
    fn write_pair_schema(&self, fqn: &str, schema: &str) {
        self.write_contract(fqn, &doc_of(fqn));
        self.write_schema(fqn, schema);
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

// ---------------------------------------------------------------------------
// 手写接口定义夹具（兄弟 / 后代测试模块共用）
// ---------------------------------------------------------------------------

/// 接口定义文本构造器：`$defs` / `properties` 两表**片段直传**（`{` `}` 由本函数补），
/// 身份三键（`$schema` / `$id` / `title`）按 FQN 补齐——九闸用例只替换一表，其余闸保持绿
/// （各反例因此**单点**归因）。
pub(crate) fn schema_text(fqn: &str, defs: impl AsRef<str>, properties: impl AsRef<str>) -> String {
    let (defs, properties) = (defs.as_ref(), properties.as_ref());
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": format!("{fqn}.schema.json"),
        "title": fqn,
        "$defs": Value::Object(fragment(defs)),
        "properties": Value::Object(fragment(properties)),
    })
    .to_string()
}

/// 两表片段 → JSON 对象（空片段即空对象；片段带外层大括号时按整对象解析）。
fn fragment(text: &str) -> serde_json::Map<String, Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return serde_json::Map::new();
    }
    let parsed = serde_json::from_str::<Value>(&format!("{{{trimmed}}}"))
        .or_else(|_| serde_json::from_str::<Value>(trimmed))
        .expect("夹具 schema 片段必须是合法 JSON");
    parsed
        .as_object()
        .cloned()
        .expect("夹具 schema 片段必须是对象")
}

/// 同 [`schema_text`]，另为 `$defs` 每条在 `properties` 侧补 `#/$defs/<名>` 镜像 `$ref`
/// （写闸 ⑥ 的合法形态）——两资产夹具与渲染 / 判据用例的输入面。
pub(crate) fn mirrored_schema_text(
    fqn: &str,
    defs: impl AsRef<str>,
    properties: impl AsRef<str>,
) -> String {
    let (defs, properties) = (defs.as_ref(), properties.as_ref());
    let defs_value = fragment(defs);
    let mut properties_value = fragment(properties);
    for name in defs_value.keys() {
        properties_value
            .entry(name.clone())
            .or_insert_with(|| serde_json::json!({ "$ref": format!("#/$defs/{name}") }));
    }
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": format!("{fqn}.schema.json"),
        "title": fqn,
        "$defs": Value::Object(defs_value),
        "properties": Value::Object(properties_value),
    })
    .to_string()
}

/// 零 op / 零类型的空接口定义（`api = none` 留白元素的合法形态）。
pub(crate) fn empty_schema(fqn: &str) -> String {
    mirrored_schema_text(fqn, "", "")
}

/// 单 op 接口定义（`x-function` 原文直传，零 `$defs`）。
pub(crate) fn op_schema(fqn: &str, op: impl AsRef<str>, signature: impl AsRef<str>) -> String {
    let (op, signature) = (op.as_ref(), signature.as_ref());
    mirrored_schema_text(
        fqn,
        "",
        format!("\"{op}\": {{ \"x-function\": {signature} }}"),
    )
}

/// 跨元素透传 alias 的 `$defs` 片段：`$ref` 前缀经 [`paths::ref_relative`] 单点求出、
/// 文件名取目标叶段（夹具 MUST NOT 手算 `../` 层数或写全 FQN 文件名）。
pub(crate) fn reexport_def(fqn: &str, name: impl AsRef<str>, target: impl AsRef<str>) -> String {
    let (name, target) = (name.as_ref(), target.as_ref());
    format!(
        "\"{name}\": {{ \"$ref\": \"{}{}.schema.json#/$defs/{name}\" }}",
        paths::ref_relative(fqn, target),
        paths::leaf_name(target)
    )
}

/// 解析 + 投影（失败即断言失败）：手写接口定义文本 → `(Contract, schema 值)`。
pub(crate) fn prepare(schema: &str, fqn: &str, file: &str) -> (Contract, Value) {
    let value: Value = serde_json::from_str(schema).expect("夹具接口定义必须是合法 JSON");
    let contract = Contract::from_schema(fqn, file, &value);
    (contract, value)
}

/// `Contract` → 供判据面取数的 schema（`$defs` / `properties` 两表重建：`apiMissing` /
/// `apiUndeclared` / `apiRefMissing` / `apiShadowing` 的读取面）。
pub(crate) fn schema_of(contract: &Contract) -> Value {
    let mut defs = serde_json::Map::new();
    for (name, definition) in &contract.types {
        defs.insert(name.clone(), definition.clone());
    }
    for (name, target) in &contract.reexports {
        let target = target.as_str().unwrap_or_default();
        defs.insert(
            name.clone(),
            serde_json::json!({
                "$ref": format!(
                    "{}{}.schema.json#/$defs/{name}",
                    paths::ref_relative(&contract.fqn, target),
                    paths::leaf_name(target)
                )
            }),
        );
    }
    let mut properties = serde_json::Map::new();
    for op in &contract.ops {
        properties.insert(
            op.id.clone(),
            serde_json::json!({ "x-function": op.signature }),
        );
    }
    for name in defs.keys() {
        properties
            .entry(name.clone())
            .or_insert_with(|| serde_json::json!({ "$ref": format!("#/$defs/{name}") }));
    }
    serde_json::json!({
        "$id": format!("{}.schema.json", contract.fqn),
        "title": contract.fqn,
        "$defs": defs,
        "properties": properties,
    })
}

/// 元素构造器（`path` 原文直传）。
fn element(fqn: &str, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Warn,
        api: ApiStatus::Declared,
        entry: None,
        path: paths.iter().map(|path| (*path).to_owned()).collect(),
        summary: None,
        metadata: BTreeMap::new(),
        model_file: ".dev-team/architecture/graph/model/01-test.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
        doc: None,
    }
}

/// 派生 `<叶段>.md`（理由面）：只为「文档在盘」这一独立事实，零 fence（历史 `api-*`
/// 形态已退场——读面不读文档正文）。
pub(crate) fn doc_of(fqn: &str) -> String {
    format!("# {fqn}\n\n## 职能\n\n门禁执行体。\n")
}

/// 历史形态文档（含 `api-op` fence）：只用于「零 md 回落」用例证明读面不解析正文。
fn legacy_doc_with_fences(fqn: &str) -> String {
    op_schema(fqn, "file_stem", "{}")
}

/// 试点形态接口定义（H1 = 文件名主干 = FQN，1 op `file_stem`，1 类型
/// `LocalArchiCheck`）：可直接过 `validate_schema` 的九道闸。
fn pilot_schema(fqn: &str) -> String {
    format!(
        r##"{{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "{fqn}.schema.json",
  "title": "{fqn}",
  "$defs": {{ "LocalArchiCheck": {{ "type": "object", "properties": {{}} }} }},
  "properties": {{
    "file_stem": {{
      "x-function": {{
        "description": "",
        "owner": "LocalArchiCheck",
        "params": [{{ "name": "timestamp", "schema": {{ "type": "string" }} }}],
        "returns": {{ "type": "string" }}
      }}
    }},
    "LocalArchiCheck": {{ "$ref": "#/$defs/LocalArchiCheck" }}
  }}
}}
"##
    )
}

/// 空快照（`render_all` 的空输入）。
fn empty_snapshot() -> ApiSnapshot {
    ApiSnapshot {
        contracts: Vec::new(),
        schemas: BTreeMap::new(),
        symbols: BTreeMap::new(),
        doc_present: BTreeMap::new(),
    }
}

// ---------------------------------------------------------------------------
// enabled：declared ∧ path 非空 ∧ schema 在盘
// ---------------------------------------------------------------------------

/// 正向（D-5）：启用判据五形态——`declared` ∧ `path` 非空 ∧ `<叶段>.schema.json` 在盘 ⇒
/// `true`；同条件 schema 缺席（文档在盘）⇒ `false`；`api = none` ∧ schema 在盘 ⇒ `false`
/// （越界契约不是启用证据）；`path = []` ∧ schema 在盘 ⇒ `false`；空元素表 ⇒ `false`。
#[test]
fn 启用只认declared且schema在盘() {
    let fqn = "App.Infra.Rt";

    let live = TempDir::new("enabled-schema");
    live.write_schema(fqn, &pilot_schema(fqn));
    assert!(
        enabled(live.root(), &[element(fqn, &["src/rt/"])]),
        "declared + path 非空 + schema 在盘 → 启用"
    );

    // 同一份内容落在文档位：接口事实只认 schema，文档在盘零启用证据
    let doc_only = TempDir::new("enabled-doc-only");
    doc_only.write_contract(fqn, &doc_of(fqn));
    assert!(
        !enabled(doc_only.root(), &[element(fqn, &["src/rt/"])]),
        "只有文档（即便带历史 fence）→ 未启用"
    );

    let blank = TempDir::new("enabled-none");
    blank.write_schema(fqn, &pilot_schema(fqn));
    let mut undeclared = element(fqn, &["src/rt/"]);
    undeclared.api = ApiStatus::None;
    assert!(
        !enabled(blank.root(), &[undeclared]),
        "`api = none` 的越界 schema → 未启用"
    );

    let container = TempDir::new("enabled-container");
    container.write_schema(fqn, &pilot_schema(fqn));
    assert!(
        !enabled(container.root(), &[element(fqn, &[])]),
        "`path = []` 元素零参与"
    );
    assert!(!enabled(container.root(), &[]), "空元素表 → 未启用");
}

/// 异常（D-5）：散落文件不构成启用证据——目录缺席 / 空目录 / `<archi_models_dir>` 一级的
/// 笔记 / 与任何元素无关的 `<叶段>.schema.json`，以及元素 `doc` 字段指向的在盘文档，一律不算。
#[test]
fn 散落文件不构成启用证据() {
    let bare = TempDir::new("enabled-bare");
    assert!(
        !enabled(bare.root(), &[element("App.X", &["src/x/"])]),
        "无 .dev-team → false（零 panic）"
    );

    let empty = TempDir::new("enabled-empty");
    fs::create_dir_all(archi_models_dir(empty.root())).expect("建空契约目录");
    assert!(
        !enabled(empty.root(), &[element("App.X", &["src/x/"])]),
        "空契约目录 → false"
    );

    let stray = TempDir::new("enabled-stray");
    stray.write(
        ".dev-team/architecture/models/notes.md",
        &doc_of("App.Notes"),
    );
    stray.write(
        ".dev-team/architecture/models/Other.schema.json",
        &pilot_schema("App.Other"),
    );
    assert!(
        !enabled(stray.root(), &[element("App.X", &["src/x/"])]),
        "与元素无关的文件（含 schema）→ 未启用（MUST NOT 退化为递归任意 *.schema.json）"
    );

    // `doc` 字段零参与：即便指向一份在盘的文档，也不构成启用证据（D-23 的零判据读者）
    let doc_field = TempDir::new("enabled-doc-field");
    doc_field.write("guides/auth.md", &doc_of("App.Auth"));
    let mut with_doc = element("App.Auth", &["src/auth/"]);
    with_doc.doc = Some("guides/auth.md".to_owned());
    assert!(
        !enabled(doc_field.root(), &[with_doc]),
        "`doc` 是约定字段，零启用证据"
    );
}

/// 边界：`path` 空元素零参与——即便其 `doc` 指向一份带 fence 的文件，也不构成启用证据。
#[test]
fn enabled容器元素零参与() {
    let temp = TempDir::new("enabled-container");
    temp.write("README.md", &legacy_doc_with_fences("README.md"));
    let mut container = element("App", &[]);
    container.api = ApiStatus::None;
    container.doc = Some("README.md".to_owned());
    assert!(
        !enabled(temp.root(), &[container]),
        "path 空元素不入载体装配"
    );
}

// ---------------------------------------------------------------------------
// load_contracts：全链 / 缺 md / 跳过 / 路径空 / 异常面
// ---------------------------------------------------------------------------

/// 正向（AC-1 + AC-9）：单元素 `path` 非空 + 同目录 `<叶段>.schema.json`（1 op + 1 类型、
/// 零文档）→ `Ok`；签名即 `x-function` 解包值、`file` 覆写为 repo 相对 schema 路径、三表
/// 与文档事实各就位。
#[test]
fn load_contracts全链() {
    let temp = TempDir::new("load-chain");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_schema(fqn, &pilot_schema(fqn));
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
    assert_eq!(
        snapshot.contracts[0].ops[0].line, 0,
        "schema 无行号 ⇒ line 恒 0"
    );
    let mut expected = snapshot.schemas[fqn]["properties"]["file_stem"]["x-function"].clone();
    expected
        .as_object_mut()
        .expect("x-function 是对象")
        .remove("description");
    assert_eq!(
        snapshot.contracts[0].ops[0].signature, expected,
        "签名即 `x-function` 解包值（`description` 丢弃）"
    );
    assert_eq!(
        snapshot.contracts[0].file,
        ".dev-team/architecture/models/App/Backend/Infra/ArchiRuntime.schema.json",
        "`file` 覆写为 repo 相对 schema 路径（分层目录 + 叶段文件名）"
    );
    assert_eq!(snapshot.contracts[0].types.len(), 1, "一个自有类型");
    assert!(snapshot.schemas.contains_key(fqn), "schema 装配在案");
    assert!(snapshot.symbols.contains_key(fqn), "符号集 key = FQN");
    assert!(
        !snapshot.symbols[fqn].is_empty(),
        "取数器从入口抽到符号：{:?}",
        snapshot.symbols[fqn]
    );
    assert_eq!(
        snapshot.doc_present.get(fqn),
        Some(&false),
        "文档事实与装载同批产出（零文档 ⇒ false）"
    );
}

/// 异常（AC-1 / Q4）：**零 md 回落**——只有派生 `<叶段>.md`（含历史 `api-op` fence）而 schema
/// 缺席 ⇒ 该元素零装载且 `schemas` 无该键；两者都不在 ⇒ 同样零条目。
#[test]
fn load_contracts缺md零装载() {
    let temp = TempDir::new("load-missing-md");
    let fqn = "App.Backend.Core.Agent";
    // 历史形态文档（fence 完整）：读面只认 schema，故零装载
    temp.write_contract(fqn, &doc_of(fqn));
    temp.write_contract(
        "App.Frontend.Shell",
        &legacy_doc_with_fences("App.Frontend.Shell"),
    );
    let elements = [
        element(fqn, &["crates/core/agent/"]),
        element("App.Frontend.Shell", &["src/shell/"]),
        element("App.Frontend.Empty", &["src/empty/"]),
    ];
    let snapshot = load_contracts(temp.root(), &elements).expect("接口未写不报错");
    assert!(snapshot.contracts.is_empty(), "零契约（零 md 回落）");
    assert!(snapshot.schemas.is_empty(), "零 schema");
    assert!(snapshot.symbols.is_empty(), "接口未写的元素不进取数面");
    assert_eq!(
        snapshot.doc_present.get(fqn),
        Some(&true),
        "文档事实照实报（与接口装载分立）"
    );
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
    assert_eq!(
        fields, 4,
        "ApiSnapshot 四分量（`doc_facts` 进场，仍无缺契约清单）"
    );

    let input = source
        .split_once("pub struct ApiCheckInput<'a> {")
        .expect("ApiCheckInput 定义在案")
        .1
        .split_once("\n}")
        .expect("结构体收口")
        .0;
    let input_fields = input.matches("    pub ").count();
    assert_eq!(
        input_fields, 7,
        "ApiCheckInput 七字段（`doc_facts` 进场，仍无 `absent`）"
    );
}

/// 边界（D16 ⑤）：`api = none` 元素即便 `<叶段>.schema.json` 在盘也不装载（越界事实由
/// `apiUnexpected` 判据报，装载器不替它建契约）。
#[test]
fn load_contracts跳过api_none() {
    let temp = TempDir::new("load-none");
    let mut skipped = element("App.Backend.Core.Agent", &["crates/core/agent/"]);
    skipped.api = ApiStatus::None;

    let snapshot =
        load_contracts(temp.root(), std::slice::from_ref(&skipped)).expect("api=none 零噪声");
    assert!(snapshot.contracts.is_empty());
    assert!(snapshot.schemas.is_empty(), "api=none 不装配 schema");

    // schema 存在时本函数仍跳过（坏接口也不报错——判据面自会点出 apiUnexpected）
    temp.write_schema("App.Backend.Core.Agent", "{ 不是 JSON }");
    let snapshot = load_contracts(temp.root(), &[skipped]).expect("api=none 不读接口文件");
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

/// 异常（AC-1 / AC-5）：坏接口文件三形态（JSON 语法错 / `$id` 与文件名不符 / op 名与类型名
/// 撞名）各一条 `Err(ApiError::Contract(..))`，文案含文件名与期望值；**MUST NOT** 返回半真
/// 快照——`findings` 与 `artifact_check` 同源返回 `Err`。
#[test]
fn load_contracts坏契约即报错() {
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let cases: [(&str, String, &str); 3] = [
        (
            "JSON 语法错",
            format!(
                "{{\n  \"$id\": \"{fqn}.schema.json\"\n  \"title\": \"{fqn}\",\n  \"$defs\": \
                 {{}},\n  \"properties\": {{}}\n}}\n"
            ),
            "不是合法 JSON",
        ),
        (
            "`$id` 与文件名不符",
            format!(
                "{{\"$schema\": \"https://json-schema.org/draft/2020-12/schema\", \"$id\":                  \"App.Backend.Infra.Other.schema.json\", \"title\": \"{fqn}\", \"$defs\":                  {{}}, \"properties\": {{}}}}
"
            ),
            "App.Backend.Infra.Other.schema.json",
        ),
        (
            "op 键与 `$defs` 键撞名",
            schema_text(
                fqn,
                "\"run\": { \"type\": \"object\", \"properties\": {} }",
                "\"run\": { \"x-function\": { \"params\": [], \"returns\": { \"type\": \"string\" } } }",
            ),
            "两个键空间必须互斥",
        ),
    ];

    for (label, source, expected) in cases {
        let temp = TempDir::new("load-broken");
        temp.write_schema(fqn, &source);
        let element = element(fqn, &["crates/infra/archi/"]);
        let error = match load_contracts(temp.root(), std::slice::from_ref(&element)) {
            Ok(snapshot) => panic!("坏接口（{label}）MUST NOT 返回 Ok：{snapshot:?}"),
            Err(error) => error,
        };
        match &error {
            ApiError::Contract(message) => {
                assert!(
                    message.contains(&paths::schema_rel(fqn).expect("合法 FQN")),
                    "{label} 的文案含文件名：{message}"
                );
                assert!(
                    message.contains(expected),
                    "{label} 的文案点名期望值 `{expected}`：{message}"
                );
            }
            other => panic!("{label} 期望 ApiError::Contract，实际：{other:?}"),
        }

        // 无半真报告：判据装配与生成预览同源停摆
        assert!(
            findings(temp.root(), std::slice::from_ref(&element)).is_err(),
            "{label}：`findings` MUST 同源停摆"
        );
        temp.write_model(&format!(
            "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"crates/infra/archi/\"]\n"
        ));
        assert!(
            super::artifact_check(temp.root()).is_err(),
            "{label}：`artifact_check` MUST 同源停摆"
        );
    }
}

/// 异常（AC-1 / D45）：名字闸违例（**七族**任一族非 ASCII 标识符）在 `validate_schema`
/// 期即中止——`load_contracts` 是本仓装载期唯一中止点，五形态各
/// `Err(ApiError::Contract(..))` 并点名各自族别，文案含接口文件名；**MUST NOT** 返回部分
/// 快照。
#[test]
fn load_contracts闸一违例即中止() {
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let mirror = "\"LocalArchiCheck\": { \"$ref\": \"#/$defs/LocalArchiCheck\" }";
    let cases: [(&str, &str, String); 5] = [
        (
            "op id 族",
            "op id",
            schema_text(
                fqn,
                "",
                "\"带-连字符\": { \"x-function\": { \"params\": [], \"returns\": { \"type\": \"string\" } } }",
            ),
        ),
        (
            "参数名族",
            "参数名",
            schema_text(
                fqn,
                "",
                "\"run\": { \"x-function\": { \"params\": [{ \"name\": \"in-put\", \"schema\": { \"type\": \"string\" } }], \"returns\": { \"type\": \"string\" } } }",
            ),
        ),
        (
            "properties 数据字段族",
            "properties 数据字段键",
            schema_text(
                fqn,
                "\"LocalArchiCheck\": { \"type\": \"object\", \"properties\": { \"a-b\": { \"type\": \"integer\" } } }",
                mirror,
            ),
        ),
        (
            "owner 族",
            "owner 键",
            schema_text(
                fqn,
                "",
                "\"file_stem\": { \"x-function\": { \"owner\": \"带-连字符\", \"params\": [], \"returns\": { \"type\": \"string\" } } }",
            ),
        ),
        (
            "$ref 末段族",
            "$ref 末段名",
            schema_text(
                fqn,
                "",
                "\"file_stem\": { \"x-function\": { \"params\": [{ \"name\": \"input\", \"schema\": { \"$ref\": \"#/$defs/带 空格\" } }], \"returns\": { \"type\": \"string\" } } }",
            ),
        ),
    ];

    for (label, family, source) in cases {
        let temp = TempDir::new("load-gate1");
        temp.write_schema(fqn, &source);
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
                    message.contains(&paths::schema_rel(fqn).expect("合法 FQN")),
                    "{label} 的文案带文件名定位：{message}"
                );
            }
            other => panic!("{label} 期望 ApiError::Contract，实际：{other:?}"),
        }
    }
}

/// 异常（AC-1 / D47）：**落点闸违例在装载期即中止**——`x-function` 写在四种越界位置
/// （`$defs.T.properties.call.items` / `additionalProperties` / `oneOf` 分支 / `$defs.T`
/// 自身顶层）各 `Err(ApiError::Contract(..))` 并点名 **JSON pointer**，**MUST NOT** 走到
/// 渲染期；MUST NOT 返回部分快照。
#[test]
fn load_contracts落点闸违例即中止() {
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let mirror = "\"T\": { \"$ref\": \"#/$defs/T\" }";
    let function = "{ \"params\": [], \"returns\": { \"type\": \"string\" } }";
    let cases: [(&str, &str, String); 4] = [
        (
            "properties 的 items",
            "$defs.T.properties.call.items",
            schema_text(
                fqn,
                format!(
                    "\"T\": {{ \"type\": \"object\", \"properties\": {{ \"call\": {{ \"type\": \"array\", \"items\": {{ \"x-function\": {function} }} }} }} }}"
                ),
                mirror,
            ),
        ),
        (
            "additionalProperties",
            "$defs.T.additionalProperties",
            schema_text(
                fqn,
                format!(
                    "\"T\": {{ \"type\": \"object\", \"additionalProperties\": {{ \"x-function\": {function} }} }}"
                ),
                mirror,
            ),
        ),
        (
            "oneOf 分支",
            "$defs.T.oneOf[1]",
            schema_text(
                fqn,
                format!(
                    "\"T\": {{ \"oneOf\": [{{ \"type\": \"string\" }}, {{ \"x-function\": {function} }}] }}"
                ),
                mirror,
            ),
        ),
        (
            "$defs 条目自身顶层",
            "$defs.T",
            schema_text(
                fqn,
                format!("\"T\": {{ \"x-function\": {function} }}"),
                mirror,
            ),
        ),
    ];

    for (label, pointer, source) in cases {
        let temp = TempDir::new("load-position");
        temp.write_schema(fqn, &source);
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
                    message.contains(&paths::schema_rel(fqn).expect("合法 FQN")),
                    "{label} 的文案带文件名定位：{message}"
                );
            }
            other => panic!("{label} 期望 ApiError::Contract，实际：{other:?}"),
        }
    }

    // 合法二处不受影响：试点形态（`properties.x-function` + `$defs` 零字段类型）仍 `Ok`
    let temp = TempDir::new("load-position-legal");
    temp.write_schema(fqn, &pilot_schema(fqn));
    let snapshot = load_contracts(temp.root(), &[element(fqn, &["crates/infra/archi/"])])
        .expect("试点接口仍 Ok");
    assert_eq!(snapshot.contracts.len(), 1, "合法落点不被误拦");
}

/// 异常（D20）：接口文件在案但不可解码（非法 UTF-8 字节）→ `Err(ApiError::Io(..))`，
/// `message()` 非空且含路径线索。目录占位形态走「文件不在案」分支（`is_file()` 为假），
/// 故 IO 面以「文件在案但读取失败」表达。
#[test]
fn load_contracts的io错误面() {
    let temp = TempDir::new("load-io");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let path = paths::schema_path(temp.root(), fqn).expect("测试 FQN 恒为合法形态");
    fs::create_dir_all(path.parent().expect("父目录")).expect("建接口目录");
    fs::write(&path, [0xff_u8, 0xfe, 0x00, 0x01]).expect("写非法 UTF-8 字节");

    let error = match load_contracts(temp.root(), &[element(fqn, &["crates/infra/archi/"])]) {
        Ok(snapshot) => panic!("不可解码的接口文件应报 IO 错误：{snapshot:?}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, ApiError::Io(_)),
        "读取失败 → ApiError::Io，实际：{error:?}"
    );
    let message = error.message();
    assert!(!message.is_empty(), "文案非空");
    assert!(
        message.contains("ArchiRuntime") || message.contains("接口定义"),
        "文案含路径线索：{message}"
    );
}

/// 边界：300 元素（其中 100 有接口文件）→ `Ok`，`contracts.len() == 100`（其余 200 个
/// 「接口未写」，判据面零对象）；1000 键 `$defs` 规模零截断；同输入两次调用逐字段相等。
#[test]
fn load_contracts规模边界() {
    let temp = TempDir::new("load-scale");
    let mut elements = Vec::new();
    for index in 0..300 {
        let fqn = format!("App.Core.E{index:03}");
        elements.push(element(&fqn, &[&format!("src/e{index:03}/")]));
        if index < 100 {
            temp.write_schema(&fqn, &pilot_schema(&fqn));
        }
    }
    let snapshot = load_contracts(temp.root(), &elements).expect("规模输入应成功");
    assert_eq!(snapshot.contracts.len(), 100, "100 份接口");
    assert_eq!(snapshot.schemas.len(), 100, "100 份 schema");
    assert_eq!(snapshot.symbols.len(), 100, "100 个元素的取数面");
    assert!(
        snapshot
            .contracts
            .windows(2)
            .all(|window| window[0].fqn <= window[1].fqn),
        "契约按 FQN 字典序"
    );
    let again = load_contracts(temp.root(), &elements).expect("二次装载应成功");
    assert_eq!(snapshot, again, "同输入幂等（零随机序）");

    // 第二规模：1000 键 `$defs` 的接口 → 装配成功、键数逐项相等、零 panic
    let big = TempDir::new("load-scale-defs");
    let big_fqn = "App.Core.Big";
    let mut defs = String::new();
    let mut properties = String::new();
    for index in 0..1000 {
        if index > 0 {
            defs.push_str(", ");
            properties.push_str(", ");
        }
        defs.push_str(&format!("\"T{index:04}\": {{\"type\": \"object\"}}"));
        properties.push_str(&format!(
            "\"T{index:04}\": {{\"$ref\": \"#/$defs/T{index:04}\"}}"
        ));
    }
    big.write_schema(big_fqn, &schema_text(big_fqn, &defs, &properties));
    let snapshot = load_contracts(big.root(), &[element(big_fqn, &["src/big/"])])
        .expect("1000 键接口应装配成功");
    let schema = snapshot.schemas.get(big_fqn).expect("schema 在案");
    assert_eq!(
        schema["$defs"].as_object().map(|defs| defs.len()),
        Some(1000),
        "1000 个 `$defs` 键逐项落位（零截断、零 panic）"
    );
    assert_eq!(snapshot.contracts[0].types.len(), 1000, "投影逐项落位");
}

// ---------------------------------------------------------------------------
// render_all：三类生成物 / 幂等 / INDEX 覆盖 / categories
// ---------------------------------------------------------------------------

/// 正向（AC-12 / D-13）：试点形态 → 文件集含 `INDEX.toml` 与 `<root>.i.rs`，且**不含**
/// `<叶段>.schema.json`（手写真相不进应生成集）；`.i.ts` 按元素语言缺席；每份 `path` 与
/// `paths` 单点产出值逐字相等。
#[test]
fn render_all三类生成物齐备() {
    let temp = TempDir::new("render-all");
    temp.write_model(
        "[element.\"App.Backend.Infra.ArchiRuntime\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/infra/archi/\"]
",
    );
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_pair(fqn);
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    let paths_set: Vec<&Path> = files.iter().map(|file| file.path.as_path()).collect();
    assert!(
        !paths_set.contains(
            &paths::schema_path(temp.root(), fqn)
                .expect("合法 FQN")
                .as_path()
        ),
        "`<叶段>.schema.json` 是手写真相，MUST NOT 出现在应生成集"
    );
    assert!(
        paths_set.contains(&paths::index_path(temp.root()).as_path()),
        "INDEX 落点 = paths::index_path"
    );
    let generated = paths::generated_path(
        temp.root(),
        "crates/infra/archi/src/lib.rs",
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
    assert_eq!(files.len(), 2, "一份 `.i.rs` + 一份 `INDEX.toml`");
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
    temp.write_pair_schema(
        backend,
        &mirrored_schema_text(backend, reexport_def(backend, "AgentView", frontend), ""),
    );
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }
",
    );

    // B：文件型根（根文件自身即入口 ⇒ SingleFile），透传目标为 A
    temp.write_pair_schema(
        frontend,
        &mirrored_schema_text(
            frontend,
            format!(
                "{}, {}",
                reexport_def(frontend, "Accept", backend),
                r#""Item": {"type": "object", "properties": {}}"#
            ),
            "",
        ),
    );
    temp.write(
        "views/welcome-view.tsx",
        "export function view(): string { return ''; }
",
    );

    // 无入口元素：`path` 非空但四条约定候选全不落盘（D41：无可用入口 ⇒ 不产 `.i.*`）
    temp.write_pair_schema(
        no_entry,
        &mirrored_schema_text(
            no_entry,
            r#""Ghost": {"type": "object", "properties": {}}"#,
            "",
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
        "crates/infra/archi/src/lib.rs",
        paths::GeneratedKind::Rust,
    )
    .expect("Rust 生成物路径");
    let backend_file = files
        .iter()
        .find(|file| file.path == backend_generated)
        .expect("A 的 .i.rs 在案");
    assert_eq!(
        match paths::entry_state(temp.root(), &elements, &elements[0]) {
            paths::EntryState::Resolved { form, .. } => Some(form),
            _ => None,
        },
        Some(paths::EntryForm::Crate),
        "A 的形态由 paths::entry_state 的 Resolved 给出（非渲染器推导）"
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
        "views/welcome-view.tsx",
        paths::GeneratedKind::TypeScript,
    )
    .expect("TS 生成物路径");
    let frontend_file = files
        .iter()
        .find(|file| file.path == frontend_generated)
        .expect("B 的 .i.ts 在案");
    // B 是文件型根 ⇒ 形态位 `SingleFile`（同目录 specifier）
    let expected_specifier = paths::ts_import_specifier(
        paths::EntryForm::SingleFile,
        &paths::module_name("crates/infra/archi"),
    );
    assert_eq!(expected_specifier, "./archi.i");
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

    assert!(
        !files.iter().any(|file| file
            .path
            .strip_prefix(temp.root())
            .is_ok_and(|rel| rel.starts_with("crates/core/no-entry"))),
        "无可用入口的元素不产 `.i.*`（D41 / D12：无入口即无生成物路径）"
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
    temp.write_pair_schema(
        container,
        &mirrored_schema_text(
            container,
            r#""Boxed": {"type": "object", "properties": {}}"#,
            "",
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
    temp.write_pair_schema(fqn, &empty_schema(fqn));
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

/// 异常（AC-3 落点迁移的反向面）：有契约有内容但**无可用入口**（`Absent` /
/// `Invalid(NotOnDisk)`）→ 零 `.i.*`；盘上既有 `.i` 文件也不使其复位为「有入口」；
/// `INDEX` 的 `generated` 留空串。
#[test]
fn render_all无可用入口不产生成物且index留空() {
    let temp = TempDir::new("render-all-no-entry");
    let absent = "App.Core.NoEntry";
    let not_on_disk = "App.Core.Gone";
    temp.write_model(&format!(
        "[element.\"{absent}\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/core/no-entry/\"]

         [element.\"{not_on_disk}\"]
kind = \"module\"
gate = \"warn\"
entry = \"crates/core/gone/src/lib.rs\"
path = [\"crates/core/gone/\"]
"
    ));
    temp.write_pair(absent);
    temp.write_pair(not_on_disk);
    // 盘上既有生成物（旧落点与新落点各一份）都 MUST NOT 充当入口证据
    temp.write("crates/core/gone.i.rs", "// 旧落点生成物\n");
    temp.write("crates/core/gone/src/lib.i.rs", "// 既有生成物\n");

    let elements = [
        element(absent, &["crates/core/no-entry/"]),
        element(not_on_disk, &["crates/core/gone/"]),
    ];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    assert!(
        !files.iter().any(|file| {
            file.path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".i.rs") || name.ends_with(".i.ts"))
        }),
        "无可用入口即无 `.i.*`：{:?}",
        files.iter().map(|file| &file.path).collect::<Vec<_>>()
    );

    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 仍产出");
    assert_eq!(
        index.content.matches("generated = \"\"").count(),
        2,
        "无入口元素的 `generated` 留空串：\n{}",
        index.content
    );
    assert!(
        index.content.contains(absent) && index.content.contains(not_on_disk),
        "两元素仍进 INDEX（缺的是落点，不是条目）：\n{}",
        index.content
    );
}

/// 边界（AC-3 落点迁移的正向面）：`INDEX` 的 `generated` 列给出**入口同级**的落点串，
/// 与 `paths::generated_path` 逐字相等。
#[test]
fn render_all的index落点逐字等于入口同级派生() {
    let temp = TempDir::new("render-all-index-generated");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"crates/infra/archi/\"]\n"
    ));
    temp.write_pair(fqn);
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let content = index_text(temp.root(), &snapshot);
    let generated = paths::generated_path(
        temp.root(),
        "crates/infra/archi/src/lib.rs",
        paths::GeneratedKind::Rust,
    )
    .expect("入口形态合法");
    let expected = super::display(temp.root(), &generated);
    assert_eq!(
        expected, "crates/infra/archi/src/lib.i.rs",
        "落点串是入口同级的 repo 相对形态"
    );
    assert!(
        content.contains(&format!("generated = \"{expected}\"")),
        "INDEX 的 generated 列 = 入口同级派生：\n{content}"
    );
}

/// 正向（AC-3 / D41）：目录型根的第二候选 `<root>/mod.rs` 命中 → `form = SubdirModule`、
/// 落点 `<root>/mod.i.rs`（入口同级）。落点入参是**入口全串**——元素根 `src/mod/` 既拼
/// 不出 `mod.i.rs`，也拼不出同级层数。
#[test]
fn render_all的目录型根落点() {
    let temp = TempDir::new("render-all-subdir-module");
    let fqn = "App.Backend.Core.Agent";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/mod/\"]\n"
    ));
    temp.write_pair(fqn);
    temp.write(
        "src/mod/mod.rs",
        "pub fn run() -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["src/mod/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    assert_eq!(
        paths::entry_state(temp.root(), &elements, &elements[0]),
        paths::EntryState::Resolved {
            form: paths::EntryForm::SubdirModule,
            entry: "src/mod/mod.rs".to_owned(),
        },
        "`src/lib.rs` 不在盘 ⇒ 第二候选 `<root>/mod.rs` 命中（形态位 SubdirModule）"
    );
    let expected = paths::generated_path(temp.root(), "src/mod/mod.rs", paths::GeneratedKind::Rust)
        .expect("入口形态合法");
    assert_eq!(
        super::display(temp.root(), &expected),
        "src/mod/mod.i.rs",
        "落点 = `<入口目录>/<入口主干>.i.rs`（入口同级）"
    );
    let generated = files
        .iter()
        .find(|file| file.path == expected)
        .expect("目录型根的 `.i.rs` 在案");
    assert!(
        generated.content.contains("LocalArchiCheck"),
        "生成物内容非空（类型段在案）：
{}",
        generated.content
    );
}

/// 正向（AC-1 ② / AC-3）：多文件根的 `path` 首条即元素根（`app.tsx`），文件型根命中
/// 「根文件自身」→ 落点 `app.i.ts`。`path` 其余条目 MUST NOT 参与落点派生。
#[test]
fn render_all多文件根取首条为入口() {
    let temp = TempDir::new("render-all-multi-root");
    let fqn = "App.Frontend.App";
    let paths_declared = ["app.tsx", "app.css", "main.ts"];
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"app.tsx\", \"app.css\", \"main.ts\"]\n"
    ));
    temp.write_pair(fqn);
    temp.write("app.tsx", "export function view(): string { return ''; }\n");

    let elements = [element(fqn, &paths_declared)];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");

    assert_eq!(
        paths::entry_state(temp.root(), &elements, &elements[0]),
        paths::EntryState::Resolved {
            form: paths::EntryForm::SingleFile,
            entry: "app.tsx".to_owned(),
        },
        "多文件根只取 `path.first()` 作根（文件型根 ⇒ 根文件自身）"
    );
    let expected = paths::generated_path(temp.root(), "app.tsx", paths::GeneratedKind::TypeScript)
        .expect("入口形态合法");
    assert_eq!(
        super::display(temp.root(), &expected),
        "app.i.ts",
        "落点是入口同级的仓根文件（无目录段不丢文件名）"
    );
    let generated = files
        .iter()
        .find(|file| file.path == expected)
        .expect("多文件根的 `.i.ts` 在案");
    assert!(
        generated
            .content
            .contains("export interface LocalArchiCheck"),
        "生成物内容非空（类型段在案）：
{}",
        generated.content
    );

    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 仍产出");
    assert!(
        index.content.contains("generated = \"app.i.ts\""),
        "INDEX 的 generated 列同为新落点：
{}",
        index.content
    );
}

/// 异常（AC-5 源码自证）：`render_all` 体内零 `let scope = element.path.first()`；
/// 落点入参唯一来源是 `entry_state` 的 `Resolved.entry`；`EmitTarget {` 字面量仍恰 1 处。
#[test]
fn render_all落点入参唯一来源为入口() {
    let source = include_str!("mod.rs");
    let code: Vec<&str> = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    assert!(
        !code
            .iter()
            .any(|line| line.contains("let scope = element.path.first()")),
        "落点面零 `let scope = element.path.first()`"
    );
    assert_eq!(
        code.iter()
            .filter(|line| line.contains("EmitTarget {"))
            .count(),
        1,
        "`EmitTarget {{` 字面量恰 1 处"
    );
    assert!(
        code.iter()
            .any(|line| line.contains("paths::entry_state(root, &elements, element)")),
        "入口经 `paths::entry_state` 单点（形态与落点同源）"
    );
    assert!(
        code.iter()
            .any(|line| line.contains("paths::generated_path(root, &entry, kind)")),
        "落点入参是 `Resolved.entry`（不是元素根）"
    );
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
    temp.write_pair(fqn);
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
    temp.write_pair("App.Core.Present");
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
    temp.write_pair_schema(
        fqn,
        &mirrored_schema_text(
            fqn,
            format!(
                "{}, {}",
                r#""Alpha": {"type": "object", "properties": {}}"#,
                reexport_def(fqn, "AgentInput", "App.Core.Beta")
            ),
            "",
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
    temp.write_pair_schema(
        fqn,
        &mirrored_schema_text(fqn, r#""A": {"type": "object"}, "B": {"type": "object"}, "C": {"type": "object"}, "D": {"type": "object"}, "E": {"type": "object"}"#, ""),
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
    temp.write_pair_schema(
        fqn,
        &mirrored_schema_text(fqn, r##""Reexport": {"$ref": "#/$defs/Alpha"}, "Class": {"type": "object", "x-class": true}, "Variant": {"oneOf": [{"$ref": "#/$defs/Alpha"}]}, "Choice": {"enum": ["a", "b"]}, "Scalar": {"type": "integer"}, "Object": {"type": "object", "properties": {}}, "Both": {"type": "object", "x-class": true, "oneOf": [{"$ref": "#/$defs/Alpha"}]}, "Alpha": {"type": "object", "properties": {}}"##, ""),
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

/// 正向（AC-5 / D13）：`INDEX.toml` 的 `degraded` 计数口径是**标记出现次数**（全深度，
/// `index_entries` 单点经 `schema::degraded_marks`，零第二套算法）——两条条目自身带词表
/// 标记的定义 → `degraded = 2`；同一契约去掉这两条标记（只留仅 `note` 的草稿同形） →
/// `degraded = 0`（键仍输出「0」而非省略，见 `build_index` 章节）。
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
    // 两条带词表取值 `x-common.degraded` 的定义 + 一条不带（计数只数带标记者）
    temp.write_pair_schema(
        fqn,
        &mirrored_schema_text(fqn, r#""Degraded": {"type": "object", "x-common": {"degraded": "noElement"}}, "Rejected": {"type": "object", "x-common": {"degraded": "crossLanguage"}}, "Plain": {"type": "object"}"#, ""),
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
        "两处标记 → degraded = 2（派生单点在 `index_entries`）：\n{}",
        index.content
    );

    // 第二形态：同一契约去掉两条标记 → degraded = 0（键仍输出 0，不省略）
    temp.write_pair_schema(
        fqn,
        &mirrored_schema_text(fqn, r#""Degraded": {"type": "object"}, "Rejected": {"type": "object"}, "Plain": {"type": "object"}"#, ""),
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
    temp.write_pair(fqn);
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
        doc_present: &snapshot.doc_present,
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
        ".dev-team/architecture/graph/model/01-test.toml",
        &format!(
            "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
             path = [\"crates/infra/archi/\"]\n"
        ),
    );
    temp.write_pair(fqn);
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
    temp.write_pair(fqn);
    let elements = [element(fqn, &["src/alpha/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");

    let findings = check(ApiCheckInput {
        root: temp.root(),
        elements: &elements,
        contracts: &snapshot.contracts,
        schemas: &snapshot.schemas,
        symbols: &snapshot.symbols,
        doc_present: &snapshot.doc_present,
        enabled: false,
    });
    assert!(findings.is_empty(), "未启用即空集，不因数据存在而产出");
}

/// 正向（D18）：构造多元素多 kind 命中（`docMissing` ×2 + `apiEntryMissing` ×1 +
/// `apiMissing` ×1 + `apiConstructorDeclared` ×1）→ 返回序按 `(kind, element, target)`
/// 升序；**两次运行逐字段相等**（可逐字 diff 的报告）。元素输入序故意与期望序相反
/// （乱序注入）。
#[test]
fn check稳定排序() {
    let temp = TempDir::new("check-sorted");
    // ① 契约声明构造器（入口可解析 ⇒ 走到一致性面）
    let constructor_fqn = "App.Core.Constructor";
    temp.write_pair_schema(
        constructor_fqn,
        &op_schema(
            constructor_fqn,
            "new",
            r#"{"params": [], "returns":              {"type": "object"}}"#,
        ),
    );
    temp.write(
        "src/constructor/src/lib.rs",
        "pub fn new() -> Self { Self }
",
    );
    // ② 有约定无兑现：契约声明 `ghost`，代码里没有
    let missing_fqn = "App.Core.Missing";
    temp.write_pair_schema(
        missing_fqn,
        &op_schema(
            missing_fqn,
            "ghost",
            r#"{"params": [], "returns":              {"type": "string"}}"#,
        ),
    );
    temp.write(
        "src/missing/src/lib.rs",
        "pub fn present() {}
",
    );
    // ③ 载体在盘但入口不可解析
    let entry_fqn = "App.Core.NoEntry";
    temp.write_pair(entry_fqn);
    // ④ 留白之外的两份无载体元素（`docMissing`）
    temp.write(
        "src/plain/src/lib.rs",
        "pub fn plain() {}
",
    );

    let elements = [
        element("App.Core.Plain", &["src/plain/"]),
        element("App.Core.NoEntryB", &["src/missing-b/"]),
        element(constructor_fqn, &["src/constructor/"]),
        element(missing_fqn, &["src/missing/"]),
        element(entry_fqn, &["src/no-entry/"]),
        element("App.Core.NoEntryA", &["src/missing-a/"]),
    ];

    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    assert_eq!(
        snapshot.contracts.len(),
        3,
        "六元素里三份契约在案：{:?}",
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
        doc_present: &snapshot.doc_present,
        enabled: true,
    };

    let first = check(input());
    let second = check(input());
    assert_eq!(first, second, "两次运行逐字段相等");

    let kinds: Vec<&str> = first.iter().map(|finding| finding.kind.as_str()).collect();
    let count = |kind: &str| kinds.iter().filter(|value| **value == kind).count();
    assert_eq!(count("apiEntryMissing"), 1, "一个无入口元素：{kinds:?}");
    assert_eq!(
        count("apiMissing"),
        1,
        "一个「有约定无兑现」元素：{kinds:?}"
    );
    assert_eq!(
        count("apiConstructorDeclared"),
        1,
        "契约声明构造器恰一条：{kinds:?}"
    );
    assert_eq!(
        count("docMissing"),
        3,
        "三份无载体元素各一条（`Plain` / `NoEntryA` / `NoEntryB`）：{kinds:?}"
    );
    assert_eq!(first.len(), 6, "共六条命中：{kinds:?}");

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
    let doc_missing_elements: Vec<Option<String>> = first
        .iter()
        .filter(|finding| finding.kind.as_str() == "docMissing")
        .map(|finding| finding.element.clone())
        .collect();
    assert_eq!(
        doc_missing_elements,
        vec![
            Some("App.Core.NoEntryA".to_owned()),
            Some("App.Core.NoEntryB".to_owned()),
            Some("App.Core.Plain".to_owned())
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
        doc_present: &snapshot.doc_present,
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

/// 边界（AC-12）：调用前后 tempdir 全量字节快照相等（零写盘）；入口解析链全不命中 →
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

/// 正向（AC-7 / D-14）：`bootstrap` 返回 `BootstrapDrafts { schema, doc }` 双草稿——schema
/// 可直接过写闸（每 op 一项 `x-function`、`description` 为空串、`$comment` 为写者纪律注、
/// `$id` 与文件名同形）；doc 只有 H1 + 一句，零 `##` 段名、零 fence。
#[test]
fn 双草稿可直接落盘() {
    let temp = TempDir::new("bootstrap-drafts");
    let fqn = "App.Core.Alpha";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
    ));
    temp.write(
        "src/alpha/src/lib.rs",
        "pub fn run(input: &str) -> String { String::new() }\npub struct Config;\n",
    );

    let drafts = bootstrap(temp.root(), fqn).expect("起草应成功");

    // schema：与写闸同一入口校验即可通过（草稿即过闸）
    let contract =
        crate::api::schema::validate_schema(fqn, &drafts.schema).expect("草稿 MUST 直接过写闸");
    assert_eq!(contract.fqn, fqn, "草稿的 FQN = 入参");
    assert!(
        contract.ops.iter().any(|op| op.id == "run"),
        "函数符号落 properties：{:?}",
        contract
            .ops
            .iter()
            .map(|op| op.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        contract.types.contains_key("Config"),
        "类型符号落 `$defs`：{:?}",
        contract.types.keys().collect::<Vec<_>>()
    );

    let value: Value = serde_json::from_str(&drafts.schema).expect("草稿是合法 JSON");
    for key in ["$schema", "$id", "title", "$comment", "$defs", "properties"] {
        assert!(value.get(key).is_some(), "骨架六顶层键齐备（缺 {key}）");
    }
    assert_eq!(
        value["$comment"].as_str(),
        Some(super::SCHEMA_WRITER_NOTICE),
        "`$comment` 取写者纪律注（与 `GENERATED_NOTICE` 分家）"
    );
    assert_eq!(
        value["properties"]["run"]["x-function"]["description"].as_str(),
        Some(""),
        "`description` 落空串（零「待补」提示语）"
    );

    // doc：H1 + 一句，零段名零 fence
    assert!(
        drafts.doc.starts_with(&format!("# {fqn}\n")),
        "doc 骨架以 H1 起头：{:?}",
        drafts.doc
    );
    assert_eq!(
        drafts
            .doc
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        2,
        "doc 骨架只有 H1 与一句指引：{:?}",
        drafts.doc
    );
    assert!(!drafts.doc.contains("## "), "零段名：{:?}", drafts.doc);
    assert!(!drafts.doc.contains("```"), "零 fence：{:?}", drafts.doc);
}

/// 边界（AC-7）：元素无可抽取符号 ⇒ `$defs {}` + `properties {}` 两空对象且仍过写闸；
/// doc 仍两行形态（零段名）。
#[test]
fn 留白写空schema() {
    let temp = TempDir::new("bootstrap-empty");
    let fqn = "App.Core.Empty";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/empty/\"]\n"
    ));
    temp.write("src/empty/src/lib.rs", "fn private() {}\n");

    let drafts = bootstrap(temp.root(), fqn).expect("零符号仍可起草");
    let contract =
        crate::api::schema::validate_schema(fqn, &drafts.schema).expect("空骨架仍过写闸");
    assert!(contract.ops.is_empty(), "零 op");
    assert!(contract.types.is_empty(), "零类型");
    assert!(contract.reexports.is_empty(), "零透传");

    let value: Value = serde_json::from_str(&drafts.schema).expect("合法 JSON");
    assert_eq!(value["$defs"], serde_json::json!({}), "`$defs` 空对象");
    assert_eq!(
        value["properties"],
        serde_json::json!({}),
        "`properties` 空对象"
    );
    assert_eq!(
        drafts
            .doc
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        2,
        "留白形态的 doc 仍只两行"
    );
}

/// 边界（AC-6 类型面）：`BootstrapDrafts` 两字段声明序 `schema` → `doc`；`ApiSnapshot` 的
/// 文档事实收成单布尔 `doc_present: BTreeMap<String, bool>`，`DocFact` / `doc_facts` 零残留。
#[test]
fn 载体事实收成单布尔() {
    let source = include_str!("mod.rs");
    let drafts = source
        .split_once("pub struct BootstrapDrafts {")
        .expect("`BootstrapDrafts` 在案")
        .1
        .split_once("\n}\n")
        .expect("结构体收口")
        .0;
    let schema_pos = drafts.find("pub schema: String").expect("`schema` 字段");
    let doc_pos = drafts.find("pub doc: String").expect("`doc` 字段");
    assert!(
        schema_pos < doc_pos,
        "字段声明序 `schema` → `doc`：{drafts}"
    );

    assert!(
        source.contains("pub doc_present: BTreeMap<String, bool>"),
        "`ApiSnapshot` 的文档事实是单布尔表"
    );
    for retired in ["DocFact", "doc_facts"] {
        assert_eq!(
            source.matches(retired).count(),
            0,
            "`{retired}` 已随载体事实收面退役"
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

/// 异常（R-3 / D41）：`Absent` → `ApiError::Element`，文案列出四条约定候选（目录型根）；
/// `path` 空 → 明写「无候选可派生」（不是同一句话的含糊版本）。
#[test]
fn bootstrap无入口文案列候选() {
    let temp = TempDir::new("bootstrap-absent-candidates");
    let dir_root = "App.Core.AbsentDir";
    let container = "App.Core.AbsentContainer";
    temp.write_model(&format!(
        "[element.\"{dir_root}\"]
kind = \"module\"
gate = \"warn\"
path = [\"crates/core/absent/\"]

         [element.\"{container}\"]
kind = \"module\"
gate = \"warn\"
path = []
"
    ));

    match bootstrap(temp.root(), dir_root) {
        Err(ApiError::Element(message)) => {
            for candidate in [
                "crates/core/absent/src/lib.rs",
                "crates/core/absent/mod.rs",
                "crates/core/absent/index.ts",
                "crates/core/absent/index.tsx",
            ] {
                assert!(
                    message.contains(candidate),
                    "Absent 文案列出约定候选 {candidate}：{message}"
                );
            }
            assert!(
                message.contains("未声明 `entry`"),
                "Absent 文案点明未声明：{message}"
            );
            assert!(
                message.contains("无法起草接口定义"),
                "文案收口到「无法起草接口定义」：{message}"
            );
        }
        other => panic!("{dir_root} 应报 Element 错误，实际：{other:?}"),
    }

    match bootstrap(temp.root(), container) {
        Err(ApiError::Element(message)) => assert!(
            message.contains("无候选可派生"),
            "`path` 空 → 明写无候选：{message}"
        ),
        other => panic!("{container} 应报 Element 错误，实际：{other:?}"),
    }
}

/// 异常（R-3）：`Invalid` 四理由各带理由措辞——形态非法值 / 越界目标 / 缺失文件 /
/// 冲突方（与 `check` 的文案同源，同一 `EntryProblem`）。
#[test]
fn bootstrap入口四理由文案可判() {
    let temp = TempDir::new("bootstrap-entry-problems");
    temp.write_model(
        "[element.\"App.Core.Malformed\"]
kind = \"module\"
gate = \"warn\"
         entry = \"/abs/x.rs\"
path = [\"src/alpha/\"]

         [element.\"App.Core.Outside\"]
kind = \"module\"
gate = \"warn\"
         entry = \"src/beta/src/lib.rs\"
path = [\"src/alpha/\"]

         [element.\"App.Core.NotOnDisk\"]
kind = \"module\"
gate = \"warn\"
         entry = \"src/alpha/missing.rs\"
path = [\"src/alpha/\"]

         [element.\"App.Core.GammaOwner\"]
kind = \"module\"
gate = \"warn\"
         path = [\"src/gamma/\"]

         [element.\"App.Core.Duplicated\"]
kind = \"module\"
gate = \"warn\"
         entry = \"src/gamma/mod.rs\"
path = [\"src/gamma/\"]
",
    );
    temp.write(
        "src/alpha/src/lib.rs",
        "pub fn a() {}
",
    );
    temp.write(
        "src/beta/src/lib.rs",
        "pub fn b() {}
",
    );
    temp.write(
        "src/gamma/src/lib.rs",
        "pub fn g() {}
",
    );
    temp.write(
        "src/gamma/mod.rs",
        "pub fn gm() {}
",
    );

    for (fqn, phrase) in [
        ("App.Core.Malformed", "形态非法"),
        ("App.Core.Outside", "path` 之外"),
        ("App.Core.NotOnDisk", "不在盘上"),
        ("App.Core.Duplicated", "同一个文件"),
    ] {
        match bootstrap(temp.root(), fqn) {
            Err(ApiError::Element(message)) => {
                assert!(
                    message.contains(phrase),
                    "{fqn} 的文案带理由措辞 {phrase}：{message}"
                );
                assert!(
                    message.contains("无可用入口"),
                    "{fqn} 的文案点名「无可用入口」：{message}"
                );
            }
            other => panic!("{fqn} 应报 Element 错误，实际：{other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// api_ref：双层投影口径
// ---------------------------------------------------------------------------

/// 正向（D-11）：三路径**各自按自身在盘事实**出线——文档与接口都在盘 ⇒ 两条路径都出，
/// 生成物路径是入口的函数（即便 `.i.rs` 尚未生成也给路径串）；`status` 只认接口文件。
#[test]
fn api_ref_declared三路径() {
    let temp = TempDir::new("api-ref-declared");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_pair(fqn);
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"crates/infra/archi/\"]\n"
    ));
    // 生成物路径是**入口**的函数：入口落盘后投影才给 `generatedPath`
    temp.write("crates/infra/archi/src/lib.rs", "pub fn f() {}\n");

    let reference = api_ref(temp.root(), fqn);
    assert_eq!(
        reference.status,
        ApiStatus::Declared,
        "schema 在盘 → declared"
    );

    let contract_path = reference.contract_path.expect("文档路径应有");
    assert!(
        contract_path.ends_with(".md"),
        "文档路径以 .md 结尾：{contract_path}"
    );
    assert!(
        !contract_path.starts_with("./"),
        "零前导 ./：{contract_path}"
    );
    assert!(!contract_path.contains('\\'), "POSIX 分隔：{contract_path}");

    let schema_path = reference.schema_path.expect("接口路径应有");
    assert!(
        schema_path.ends_with(".schema.json"),
        "接口路径形态：{schema_path}"
    );

    let generated_path = reference.generated_path.expect("生成物路径应有");
    assert!(
        generated_path.ends_with(".i.rs"),
        "Rust 元素的生成物以 .i.rs 结尾：{generated_path}"
    );

    assert!(
        temp.root().join(&schema_path).is_file(),
        "接口路径对应盘上文件（在盘才出线）"
    );
    assert!(
        !temp.root().join(&generated_path).exists(),
        "生成物尚未生成但路径已给（记录应生成物）"
    );
}

/// 边界（AC-10 / D22）：同一 root + fqn 下「按 FQN 自取 path」与「调用方直接给 path」
/// 两分支逐字段相等；`path` 空且 md 在案 → `generated_path = None` 而契约 / schema 路径
/// 仍 `Some`。
#[test]
fn api_ref两形态同源() {
    let temp = TempDir::new("api-ref-two-forms");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_pair(fqn);
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./crates/infra/archi/\"]\n"
    ));
    temp.write("crates/infra/archi/src/lib.rs", "pub fn f() {}\n");

    let from_model = api_ref(temp.root(), fqn);
    let caller = element(fqn, &["crates/infra/archi/"]);
    let direct = super::api_ref_of(temp.root(), std::slice::from_ref(&caller), &caller);
    assert_eq!(from_model, direct, "两形态同源（投影语义不得变形）");

    // 容器元素（`path` 空）但 md 在案
    let container_fqn = "App.Backend.Infra";
    temp.write_pair(container_fqn);
    let container_element = element(container_fqn, &[]);
    let container = super::api_ref_of(
        temp.root(),
        std::slice::from_ref(&container_element),
        &container_element,
    );
    assert_eq!(container.status, ApiStatus::Declared);
    assert!(container.generated_path.is_none(), "容器元素无生成物路径");
    assert!(container.contract_path.is_some(), "md 存在即可枚举契约");
    assert!(container.schema_path.is_some(), "schema 路径同源给出");
}

/// 边界（`ref_projection` 的两态合一）：元素缺席（模型里没有该 FQN）与模型结构错
/// （元素表退化为空）⇒ 契约 / schema 事实**仍报**（`status = declared`、两路径 `Some`），
/// 生成物路径退 `None`——投影层只报文件事实，MUST NOT 为凑生成物路径构造占位元素。
#[test]
fn api_ref元素缺席仍报契约事实() {
    let absent_fqn = "App.Core.Ghost";
    let declared_fqn = "App.Core.Alpha";

    // ① 元素缺席：模型只声明另一元素（其入口落盘 ⇒ 模型可读、正常面不受影响）
    let temp = TempDir::new("api-ref-element-absent");
    temp.write_model(&format!(
        "[element.\"{declared_fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
    ));
    temp.write(
        "src/alpha/src/lib.rs",
        "pub fn a() -> String { String::new() }\n",
    );
    temp.write_pair(absent_fqn);
    temp.write_pair(declared_fqn);

    let absent = api_ref(temp.root(), absent_fqn);
    assert_eq!(
        absent.status,
        ApiStatus::Declared,
        "契约 md 在案 ⇒ declared（判定只看文件事实）"
    );
    assert!(absent.contract_path.is_some(), "契约路径仍报文件事实");
    assert!(absent.schema_path.is_some(), "schema 路径同源给出");
    assert!(
        absent.generated_path.is_none(),
        "元素缺席 ⇒ 无入口 ⇒ 生成物路径 None（不构造占位元素）"
    );

    let declared = api_ref(temp.root(), declared_fqn);
    assert_eq!(
        declared.generated_path.as_deref(),
        Some("src/alpha/src/lib.i.rs"),
        "模型可读面照常（缺席判定不是「模型整体不可读」的副作用）"
    );

    // ② 模型结构错（叶子缺祖先链 ⇒ orphanHierarchy）：元素表为空，文件事实仍报
    let broken = TempDir::new("api-ref-broken-model");
    broken.write(
        ".dev-team/architecture/graph/model/01-test.toml",
        &format!(
            "[element.\"{absent_fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
             path = [\"src/ghost/\"]\n"
        ),
    );
    broken.write(
        "src/ghost/src/lib.rs",
        "pub fn ghost() -> String { String::new() }\n",
    );
    broken.write_pair(absent_fqn);

    let stopped = api_ref(broken.root(), absent_fqn);
    assert_eq!(
        stopped.status,
        ApiStatus::Declared,
        "结构错时元素表为空 ⇒ 走「元素缺席」同一分支（零 panic、零错误面）"
    );
    assert!(stopped.contract_path.is_some(), "契约路径仍报");
    assert!(stopped.schema_path.is_some(), "schema 路径仍报");
    assert!(
        stopped.generated_path.is_none(),
        "结构错 ⇒ 无元素 ⇒ 无入口 ⇒ 生成物路径 None"
    );
}

/// 边界：md 缺席 → `status == None` 且三路径全 `None`；单段 FQN（`App`）→ 落契约目录根，
/// 路径可判。
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

    // 单段 FQN：落契约目录根（无分层段）
    let single = paths::contract_path(temp.root(), "App").expect("单段 FQN 仍合法");
    assert_eq!(
        single.parent(),
        Some(paths::directory(temp.root()).as_path()),
        "单段 FQN 落契约目录根：{single:?}"
    );
    let app = element("App", &[]);
    let reference = super::api_ref_of(temp.root(), std::slice::from_ref(&app), &app);
    assert_eq!(reference.status, ApiStatus::None);
}

/// 边界（AC-10 / D22）：`api_ref_of` 的元素形态边界——多段 `path` 只取 `path[0]`
/// （D2：入口解析同样只看首条），结果与单段形态逐字相等；`fqn` 与 `path` 冲突时文档路径
/// 按 FQN 派生、生成物路径按入口派生（各自单点，不互相污染）。
#[test]
fn api_ref_of元素形态边界() {
    let temp = TempDir::new("api-ref-of-edges");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_pair(fqn);
    temp.write("crates/infra/archi/src/lib.rs", "pub fn f() {}\n");
    temp.write("crates/core/other/src/lib.rs", "pub fn g() {}\n");

    let single_element = element(fqn, &["crates/infra/archi/"]);
    let single = super::api_ref_of(
        temp.root(),
        std::slice::from_ref(&single_element),
        &single_element,
    );
    let multi_element = element(fqn, &["crates/infra/archi/", "crates/infra/archi/src/"]);
    let multi = super::api_ref_of(
        temp.root(),
        std::slice::from_ref(&multi_element),
        &multi_element,
    );
    assert_eq!(single, multi, "多段只取 path[0]（D2），结果逐字相等");

    // fqn 与 path 冲突：文档路径按 FQN、生成物路径按入口
    let conflicting_element = element(fqn, &["crates/core/other/"]);
    let conflicting = super::api_ref_of(
        temp.root(),
        std::slice::from_ref(&conflicting_element),
        &conflicting_element,
    );
    assert_eq!(
        conflicting.status, single.status,
        "文档在案判定只看 FQN 派生的契约路径"
    );
    assert_eq!(
        conflicting.contract_path, single.contract_path,
        "契约路径按 FQN 派生（不受 path 影响）"
    );
    assert_eq!(
        conflicting.schema_path, single.schema_path,
        "schema 路径按 FQN 派生"
    );
    assert_ne!(
        conflicting.generated_path, single.generated_path,
        "生成物路径按入口派生（两者各自单点，互不污染）"
    );
    assert_eq!(
        conflicting.generated_path.as_deref(),
        Some("crates/core/other/src/lib.i.rs"),
        "生成物路径跟着入口走（入口同级形态）"
    );
}

/// 边界（test-design:182 逐值）：`api_ref_of` 的入口两态退化——`Absent`（目录型根四候选
/// 全不落盘）与 `Invalid`（形态非法 / 声明值不在盘）都退 `generated_path = None`，契约与
/// schema 仍按文件事实报（`status = declared`）；盘上既有的 `src/lib.rs` 不把 `Invalid`
/// 复位成「有入口」（零占位元素、零回写模型字段）。
#[test]
fn api_ref_of入口缺席与非法均无生成物路径() {
    let temp = TempDir::new("api-ref-of-entry-states");
    let absent_fqn = "App.Core.Absent";
    let malformed_fqn = "App.Core.Malformed";
    let not_on_disk_fqn = "App.Core.Missing";
    for fqn in [absent_fqn, malformed_fqn, not_on_disk_fqn] {
        temp.write_pair(fqn);
    }
    // `Absent` 侧：`src/absent/` 下四个约定候选全不落盘；`Invalid` 侧：同目录 `src/lib.rs`
    // 在盘，但两条 `Invalid` 都不受它影响
    temp.write(
        "src/missing/src/lib.rs",
        "pub fn m() -> String { String::new() }\n",
    );

    let absent = element(absent_fqn, &["src/absent/"]);
    let reference = super::api_ref_of(temp.root(), std::slice::from_ref(&absent), &absent);
    assert_eq!(
        reference.generated_path, None,
        "入口 Absent ⇒ 生成物路径 None（不构造占位元素）"
    );
    assert_eq!(
        reference.status,
        ApiStatus::Declared,
        "生成物缺席不改契约事实（md 在案 ⇒ declared）"
    );
    assert!(reference.contract_path.is_some(), "契约路径仍报文件事实");
    assert!(reference.schema_path.is_some(), "schema 路径同源给出");

    let mut malformed = element(malformed_fqn, &["src/malformed/"]);
    malformed.entry = Some("/abs/x.rs".to_owned());
    let reference = super::api_ref_of(temp.root(), std::slice::from_ref(&malformed), &malformed);
    assert_eq!(
        reference.generated_path, None,
        "入口形态非法（Malformed）⇒ 生成物路径 None"
    );
    assert_eq!(
        reference.status,
        ApiStatus::Declared,
        "契约事实不受判据理由影响"
    );

    let mut not_on_disk = element(not_on_disk_fqn, &["src/missing/"]);
    not_on_disk.entry = Some("src/missing/ghost.rs".to_owned());
    let reference = super::api_ref_of(
        temp.root(),
        std::slice::from_ref(&not_on_disk),
        &not_on_disk,
    );
    assert_eq!(
        reference.generated_path, None,
        "声明值不在盘（NotOnDisk）⇒ 生成物路径 None（同目录 src/lib.rs 在盘也不回落）"
    );
    assert_eq!(
        reference.status,
        ApiStatus::Declared,
        "契约事实仍按文件事实报"
    );
}

/// 异常（test-design:183 逐值）：传入元素与元素表不同源时互不污染——① 元素**不在元素表
/// 内**（表里只有别的元素）：契约与 schema 按 `element.fqn` 派生、生成物按该元素自己的
/// 入口派生；② 表内有**同 FQN 的另一元素**：唯一判据按 FQN 排除同 FQN 者（不假阳成
/// `Duplicated`），生成物仍取传入元素的入口。两例都 MUST NOT 静默改判。
#[test]
fn api_ref_of元素与表不同源互不污染() {
    let temp = TempDir::new("api-ref-of-foreign-element");
    let ghost_fqn = "App.Core.Ghost";
    let shared_fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_pair(ghost_fqn);
    temp.write_pair(shared_fqn);
    temp.write(
        "src/ghost/src/lib.rs",
        "pub fn g() -> String { String::new() }\n",
    );
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    temp.write(
        "crates/core/other/src/lib.rs",
        "pub fn o() -> String { String::new() }\n",
    );

    // ① 元素不在元素表内：表里只有另一个元素
    let neighbour = element("App.Core.Neighbour", &["crates/core/other/"]);
    let ghost = element(ghost_fqn, &["src/ghost/"]);
    let reference = super::api_ref_of(temp.root(), std::slice::from_ref(&neighbour), &ghost);
    assert_eq!(
        reference.generated_path.as_deref(),
        Some("src/ghost/src/lib.i.rs"),
        "生成物按传入元素的入口派生（表内元素不参与）"
    );
    assert_ne!(
        reference.generated_path.as_deref(),
        Some("crates/core/other/src/lib.i.rs"),
        "表内元素的入口 MUST NOT 泄漏到传入元素的投影"
    );
    assert_eq!(
        reference.status,
        ApiStatus::Declared,
        "契约按 element.fqn 派生"
    );
    assert!(
        reference
            .contract_path
            .as_deref()
            .is_some_and(|path| path.contains("App/Core/Ghost")),
        "契约路径按传入元素 FQN 派生：{:?}",
        reference.contract_path
    );

    // ② 表内有同 FQN 的另一元素：唯一判据按 FQN 排除同 FQN 者，生成物取传入元素的入口
    let table_twin = element(shared_fqn, &["crates/core/other/"]);
    let passed = element(shared_fqn, &["crates/infra/archi/"]);
    let reference = super::api_ref_of(temp.root(), std::slice::from_ref(&table_twin), &passed);
    assert_eq!(
        reference.generated_path.as_deref(),
        Some("crates/infra/archi/src/lib.i.rs"),
        "生成物按传入元素入口派生（同 FQN 的另一元素不把它判成 Duplicated）"
    );
    assert_ne!(
        reference.generated_path.as_deref(),
        Some("crates/core/other/src/lib.i.rs"),
        "表内同 FQN 元素的入口不参与本元素的落点派生"
    );
    let by_fqn = api_ref(temp.root(), shared_fqn);
    assert_eq!(
        reference.contract_path, by_fqn.contract_path,
        "契约路径按 FQN 派生（与元素形态同源）"
    );
    assert_eq!(
        reference.schema_path, by_fqn.schema_path,
        "schema 路径按 FQN 派生（与元素形态同源）"
    );
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
    temp.write_pair(fqn);
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
    temp.write_pair(fqn);
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

    // 接口文件是**手写真相**，不在应生成集（让它进来等于每次 generate 覆写手写内容）
    assert!(
        !files
            .iter()
            .any(|file| file.path == paths::schema_path(temp.root(), fqn).expect("合法 FQN")),
        "`.schema.json` 零生成（三类产物之外）"
    );

    let index_file = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");
    let generated = paths::generated_path(
        temp.root(),
        "crates/infra/archi/src/lib.rs",
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
    assert!(
        module_file.content.contains(meta().generated_notice),
        "`.i.rs` 头注释逐字取同一份载元的 generated_notice：\n{}",
        module_file.content
    );
}

/// 异常（AC-12）：`meta()` 纯计算零 IO 零 spawn——调用前后 tempdir 全量字节快照相等；
/// 源码自证 `meta` 体内零 `fs::` / `read_to_string` / `std::process`（`common.json` 经
/// `include_str!` 编译期内嵌，不是运行期读盘）。
#[test]
fn meta零io零spawn() {
    let temp = TempDir::new("meta-no-io");
    temp.write(
        ".dev-team/architecture/graph/model/01-test.toml",
        "# 模型占位\n",
    );
    temp.write(".dev-team/architecture/graph/INDEX.toml", "# 索引占位\n");
    temp.write_pair("App.Core.Alpha");

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
/// `#/$defs/<名>` 与外部文件 `<相对前缀><叶段>.schema.json#/$defs/<名>` 都取 `$defs`
/// 之后那一段；无 `$defs` 片段时退化为路径末段；空串原样返回（不 panic）。
///
/// 该函数是 `check`（签名归一）与 `emit::{rust, ts}`（类型映射）的**唯一**切分点，
/// 故三处消费面在此两侧对拍：同一 `$ref` 在判据面与渲染面得到同一名字。
#[test]
fn ref_name归一类型名单点() {
    let same_file = "#/$defs/AgentInput";
    let external = "../Core/Agent.schema.json#/$defs/AgentInput";
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
/// 目标由「本元素分层目录 + 相对前缀」解出（文件名只剩叶段）。同文件引用
/// （`#/$defs/<名>`）、无 `.schema.json` 后缀、无 `#` 片段、越界 / 带点的段、空串 →
/// `None`（同文件条目不是透传，零猜测）。
#[test]
fn reexport_target只认外部文件引用() {
    const FROM: &str = "App.Backend.Core.Foundation";
    assert_eq!(
        super::reexport_target(FROM, "../Core/Agent.schema.json#/$defs/AgentInput"),
        Some("App.Backend.Core.Agent".to_owned()),
        "同层跨子树：相对前缀解出目标 FQN"
    );
    assert_eq!(
        super::reexport_target(FROM, "../Infra/ArchiRuntime.schema.json#/$defs/X"),
        Some("App.Backend.Infra.ArchiRuntime".to_owned()),
        "兄弟层目标同式"
    );
    for reference in [
        "#/$defs/AgentInput",
        "../Core/Agent#/$defs/AgentInput",
        "../Core/Agent.schema.json",
        "../../../../Agent.schema.json#/$defs/AgentInput",
        "../Core/Agent.Input.schema.json#/$defs/AgentInput",
        "",
    ] {
        assert_eq!(
            super::reexport_target(FROM, reference),
            None,
            "非外部文件引用 → None：{reference:?}"
        );
    }

    // 源码自证：后缀剥法单点在 `paths::schema_stem`（`schema_fqn` / `ref_target` 共用）
    let paths_source = include_str!("paths.rs");
    assert_eq!(
        paths_source
            .matches("strip_suffix(SCHEMA_FILE_SUFFIX)")
            .count(),
        1,
        "后缀剥法单点（`paths::schema_stem`）"
    );
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("paths::ref_target(").count(),
        1,
        "`mod.rs` 经 `paths::ref_target` 反查（零自写后缀切分）"
    );
    assert!(!source.contains("strip_suffix("), "`mod.rs` 内零后缀切分");
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
        assert!(
            !text.contains("fn kind_of"),
            "{file} 内零本地 `kind_of` 副本（只经 `use super::…` 引用单点）"
        );
        assert!(
            text.contains("kind_of("),
            "{file} 确有引用（词表不在此处复制）"
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

/// 源码自证（**D2 / D11** 路径归一单点）：`api/mod.rs` 内 `pub fn display` 恰 1
/// （不再是 `pub(crate)`）；`lib.rs` 的 `pub use api::display` 成行恰 1（分组块会令判据
/// 恒 0）；工具侧行首 `fn display` 与本地归一（`replace("\\\\", "/")` / `strip_prefix`）
/// 均零命中——CLI 的路径串经 `api::compare` 的应生成集比对取得，归一不在 CLI 侧；
/// `short_digest` 仍 `pub(crate)`（不扩大出口面）。
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
        "工具侧零本地 `fn display`（D2/D11：归一单点在 `api::display`）"
    );
    for local in ["replace(\"\\\\\", \"/\")", "strip_prefix("] {
        assert_eq!(
            tool_source.matches(local).count(),
            0,
            "工具侧零本地路径归一（{local}）：归一单点在 `api::display`"
        );
    }
    assert_eq!(
        tool_source.matches("compare(").count(),
        1,
        "`--check` 的路径串经 `archi::api` 的比对取得（判定单源）"
    );
    assert_eq!(
        tool_source.matches("workspace_render(").count(),
        1,
        "`generate` 的应生成集经 `archi::api` 的渲染取得（渲染单源）"
    );
    assert_eq!(
        source.matches("pub(crate) fn short_digest").count(),
        1,
        "`short_digest` 仍 `pub(crate)`（不随 D55 扩大出口面）"
    );
}

/// 边界：`validate_schema` 的整链（九道闸 + 投影）在试点形态上通过（跨模块组合用例挂靠
/// 本节）；同一份文本经 `validate_schema` 后 `Contract` 的四段与 `$id` 逐字可读。
#[test]
fn 整链路装配通过() {
    let contract =
        super::schema::validate_schema(FQN_PILOT, &pilot_schema(FQN_PILOT)).expect("校验应成功");
    assert_eq!(contract.fqn, FQN_PILOT);
    assert_eq!(contract.title, FQN_PILOT);
    assert_eq!(
        contract.file,
        paths::schema_rel(FQN_PILOT).expect("试点 FQN 合法"),
        "`Contract.file` 取接口文件的 repo 相对路径（装载期再覆写为实际路径）"
    );
    assert_eq!(contract.ops.len(), 1, "一个操作（`file_stem`）");
    assert_eq!(contract.ops[0].id, "file_stem");
    assert_eq!(
        contract.ops[0].line, 0,
        "接口定义无条目级行号（位置未知口径）"
    );
    assert!(
        contract.ops[0].signature.get("description").is_none(),
        "投影丢弃 `description`"
    );
    assert_eq!(contract.types.len(), 1, "一个本地类型（`LocalArchiCheck`）");
    assert!(contract.reexports.is_empty(), "零透传");
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
    temp.write_pair_schema(
        consumer,
        &mirrored_schema_text(
            consumer,
            format!(
                "{}, {}, {}",
                reexport_def(consumer, "Loop", keyword_target),
                reexport_def(consumer, "Base", valid_target),
                r#""Thing": {"type": "object", "properties": {}}"#
            ),
            "",
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
        "crates/infra/archi/src/lib.rs",
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
    // `entry` / `path` 仍是局部变量（入口解析结果与落点，不成为渲染器入参）
    assert!(
        body.contains("paths::entry_state(root, &elements, element)"),
        "入口由局部变量承接（`Resolved` 的 `form` / `entry`）：\n{body}"
    );
    assert!(
        body.contains("let Some(path) = paths::generated_path(root, &entry, kind)"),
        "`path` 是局部变量且 `entry` 只作派生入参：\n{body}"
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
    temp.write_pair(fqn);
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
    temp.write_pair_schema(
        consumer,
        &mirrored_schema_text(
            consumer,
            format!(
                "{}, {}",
                reexport_def(consumer, "AgentInput", target),
                r#""Thing": {"type": "object", "properties": {}}"#
            ),
            "",
        ),
    );
    temp.write(
        "views/consumer.tsx",
        "export function view(): string { return ''; }
",
    );

    // 变量一：`form`（由盘上入口求出，渲染器零推导）——文件型根 ⇒ `SingleFile`
    let with_target = [
        element(consumer, &["views/consumer.tsx"]),
        element(target, &["views/agent/Input/"]),
    ];
    let form = match paths::entry_state(temp.root(), &with_target, &with_target[0]) {
        paths::EntryState::Resolved { form, .. } => form,
        other => panic!("入口应在案（Resolved），实际：{other:?}"),
    };
    assert_eq!(
        form,
        paths::EntryForm::SingleFile,
        "`form` 来自 `paths::entry_state` 的 `Resolved`（D41）"
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
        paths::ts_import_specifier(paths::EntryForm::SubdirModule, &name),
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
    bare_temp.write_pair_schema(
        consumer,
        &mirrored_schema_text(
            consumer,
            format!(
                "{}, {}",
                reexport_def(consumer, "AgentInput", target),
                r#""Thing": {"type": "object", "properties": {}}"#
            ),
            "",
        ),
    );
    bare_temp.write(
        "views/consumer.tsx",
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

// ---------------------------------------------------------------------------
// artifact_check / findings / gating_findings（本变更新增三出口）
// ---------------------------------------------------------------------------

/// 纳管语料契约源文：1 op `count_all`（**返回位声明为 `integer`**，与代码的 `String` 不符）
/// + 1 类型。用于驱动一条 `apiSignatureMismatch`（纳管子集的语料前提）。
fn mismatch_schema(fqn: &str) -> String {
    mirrored_schema_text(
        fqn,
        r#""LocalCheck": {"type": "object", "properties": {}}"#,
        r#""count_all": { "x-function": {"owner": "LocalCheck", "params": [{"name": "input", "schema": {"type": "string"}}], "returns": {"type": "integer"}} }"#,
    )
}

/// 代码侧源文（`count_all` 返回 `String` ⇒ 与契约的 `integer` 不符；`LocalCheck` 类型
/// 在案 ⇒ ⑧「有约定无兑现」不命中，判据落到 ⑨ 一致性面）。
const MISMATCH_SOURCE: &str =
    "pub struct LocalCheck;\n\npub fn count_all(input: &str) -> String { input.to_owned() }\n";

/// 判据构造器（`gating_findings` 只吃 `element` 与序，其余字段取稳定缺省）。
fn api_finding(element: Option<&str>, kind: ArchiApiFindingKind) -> ArchiApiFinding {
    ArchiApiFinding {
        kind,
        category: "contract".to_owned(),
        element: element.map(str::to_owned),
        target: Some("count_all".to_owned()),
        message: "语料判据".to_owned(),
        occurrences: Vec::new(),
    }
}

#[test]
fn artifact_check三列与compare逐列相等() {
    let temp = TempDir::new("artifact-check-eq");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/infra/archi/\"]\n"
    ));
    temp.write_pair(fqn);
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    let expected = super::render::workspace_render(temp.root()).expect("渲染应成功");
    for file in &expected {
        temp.write_absolute(&file.path, &file.content);
    }
    // 制造三列：改一份 `.i.rs`（不符）/ 删索引（缺失）/ 散落一份孤儿 `.i.rs`
    let generated = paths::generated_path(
        temp.root(),
        "crates/infra/archi/src/lib.rs",
        paths::GeneratedKind::Rust,
    )
    .expect("Rust 生成物路径");
    temp.write_absolute(&generated, "// 被手改的生成物\n");
    let index = paths::index_path(temp.root());
    std::fs::remove_file(&index).expect("删索引制造缺失");
    temp.write("src/loose.i.rs", "// 孤儿\n");

    let dto = super::artifact_check(temp.root()).expect("检查即生成预览应成功");
    let diff = super::diff::compare(temp.root()).expect("比对应成功");
    assert_eq!(dto.mismatched, diff.mismatched, "mismatched 逐列相等");
    assert_eq!(dto.orphaned, diff.orphaned, "orphaned 逐列相等");
    assert_eq!(dto.missing, diff.missing, "missing 逐列相等");
    let elements = super::elements_of(temp.root()).expect("模型在场");
    assert_eq!(
        dto.enabled,
        super::enabled(temp.root(), &elements),
        "enabled 同源"
    );
    assert_eq!(dto.orphaned, vec!["src/loose.i.rs".to_owned()], "孤儿一条");
    // 手写接口文件不在孤儿枚举面（进枚举即「全红」）
    let schema_rel = super::display(
        temp.root(),
        &paths::schema_path(temp.root(), fqn).expect("合法 FQN"),
    );
    assert!(
        !dto.orphaned.contains(&schema_rel) && !dto.mismatched.contains(&schema_rel),
        "`<叶段>.schema.json` 不进三列：{dto:?}"
    );
    assert_eq!(
        dto.mismatched,
        vec![super::display(temp.root(), &generated)],
        "mismatched 恰是被手改的生成物"
    );
}

/// 正向（AC-3 / D5 / spec「DTO 形状」）：`meta` 投影——`toolVersion` 取 crate 版本、
/// `commonDigest` 为全量 `sha256:` + 64 位小写 hex；内部载元不出线（键集恰五 / 二）。
#[test]
fn artifact_check的meta投影() {
    let temp = TempDir::new("artifact-check-meta");
    let dto = super::artifact_check(temp.root()).expect("空仓库也 Ok");
    assert_eq!(dto.meta.tool_version, env!("CARGO_PKG_VERSION"), "版本同源");
    let meta = meta();
    assert_eq!(dto.meta.common_digest, meta.common_digest, "摘要逐字相等");
    let (algorithm, hex) = dto
        .meta
        .common_digest
        .split_once(':')
        .expect("摘要形如 sha256:<hex>");
    assert_eq!(algorithm, "sha256", "算法段");
    assert_eq!(hex.len(), 64, "全量 64 位不截断：{hex}");
    assert!(
        hex.chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase()),
        "小写 hex：{hex}"
    );

    let value = serde_json::to_value(&dto).expect("序列化应成功");
    let keys: Vec<&str> = value
        .as_object()
        .expect("对象形态")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec!["enabled", "meta", "mismatched", "orphaned", "missing"],
        "五字段声明序"
    );
    let meta_keys: Vec<&str> = value["meta"]
        .as_object()
        .expect("meta 对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        meta_keys,
        vec!["toolVersion", "commonDigest"],
        "meta 二字段"
    );
    for absent in ["schemaUri", "schema_uri", "generatedNotice", "common"] {
        assert!(
            !value.as_object().expect("对象").contains_key(absent)
                && !value["meta"]
                    .as_object()
                    .expect("meta")
                    .contains_key(absent),
            "内部载元不出线：{absent}"
        );
    }
}

/// 边界（AC-10 的数据侧）：契约面未启用（① 目录缺席② 目录存在但零 md）→ `Ok`、
/// `enabled == false`、`missing` 恰 `INDEX.toml` 一条；MUST NOT `Err`、MUST NOT 三列全空。
#[test]
fn artifact_check未启用契约面仍ok() {
    let absent = TempDir::new("artifact-check-absent");
    let empty_dir = TempDir::new("artifact-check-empty-dir");
    fs::create_dir_all(archi_models_dir(empty_dir.root())).expect("建空契约目录");

    for (tag, temp) in [("目录缺席", &absent), ("零 md", &empty_dir)] {
        let dto = super::artifact_check(temp.root()).expect("未启用仍 Ok");
        assert!(!dto.enabled, "{tag}：enabled == false");
        assert_eq!(dto.missing.len(), 1, "{tag}：missing 恰一条");
        assert!(
            dto.missing[0].ends_with("INDEX.toml"),
            "{tag}：缺口是索引：{:?}",
            dto.missing
        );
        assert!(dto.mismatched.is_empty(), "{tag}：零不符");
        assert!(dto.orphaned.is_empty(), "{tag}：零孤儿");
    }
}

/// 异常（AC-3 / D5）：结构错模型 → `Err`（无 DTO 产出）；前后零写盘。
#[test]
fn artifact_check结构错模型即err() {
    let temp = TempDir::new("artifact-check-broken");
    temp.write(
        ".dev-team/architecture/graph/model/01-test.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n\
         [element.\"App.A\"]\nkind = \"module\"\n\n\
         [element.\"App.B\"]\nkind = \"module\"\n\n\
         [[relation]]\nfrom = \"App.A\"\nto = \"App.B\"\n\n\
         [[relation]]\nfrom = \"App.B\"\nto = \"App.A\"\n",
    );
    let before = temp.snapshot();
    let error = super::artifact_check(temp.root()).expect_err("结构错 ⇒ Err");
    assert!(matches!(error, ApiError::Element(_)), "错误面为 Element");
    assert!(error.message().contains("模型结构错"), "文案点名结构诊断");
    assert_eq!(before, temp.snapshot(), "Err 前后零写盘");
}

/// 边界（AC-10）：应生成集逐份落盘且一致 → 三列全空且 `enabled == true`。
#[test]
fn artifact_check零差异的成功态() {
    let temp = TempDir::new("artifact-check-clean");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/infra/archi/\"]\n"
    ));
    temp.write_pair(fqn);
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    for file in super::render::workspace_render(temp.root()).expect("渲染应成功") {
        temp.write_absolute(&file.path, &file.content);
    }

    let dto = super::artifact_check(temp.root()).expect("应成功");
    assert!(dto.enabled, "契约面启用");
    assert!(dto.mismatched.is_empty(), "零不符");
    assert!(dto.orphaned.is_empty(), "零孤儿");
    assert!(dto.missing.is_empty(), "零缺失");
}

/// 正向（AC-6 / D6）：`api = declared` 且契约 md 在案的元素判据入选；返回序与入参序一致。
#[test]
fn gating_findings契约在案的declared元素判据入选() {
    let temp = TempDir::new("gating-hit");
    let fqn = "App.Core.Alpha";
    temp.write_pair_schema(fqn, &empty_schema(fqn));
    let elements = [element(fqn, &["src/alpha/"])];
    let findings = vec![
        api_finding(Some(fqn), ArchiApiFindingKind::ApiMissing),
        api_finding(Some(fqn), ArchiApiFindingKind::ApiSignatureMismatch),
    ];

    let gated = super::gating_findings(temp.root(), &elements, &findings);
    assert_eq!(gated.len(), 2, "两条均入选");
    assert_eq!(gated, findings, "返回序与入参序一致（零重排）");
}

/// 异常（AC-6 / D6）：元素 `api = declared` 但契约 md 缺席 → 判据落选；补上 md 后同一判据
/// 入选（自激活，无白名单）。
#[test]
fn gating_findings无契约文件落选() {
    let temp = TempDir::new("gating-missing-contract");
    let fqn = "App.Core.Alpha";
    let elements = [element(fqn, &["src/alpha/"])];
    let findings = vec![api_finding(Some(fqn), ArchiApiFindingKind::ApiMissing)];

    assert!(
        super::gating_findings(temp.root(), &elements, &findings).is_empty(),
        "契约 md 缺席 ⇒ 落选"
    );

    temp.write_pair_schema(fqn, &empty_schema(fqn));
    assert_eq!(
        super::gating_findings(temp.root(), &elements, &findings).len(),
        1,
        "补上契约 md 后同一判据入选"
    );
}

/// 正向（纳管口径单支）：`api = declared` ∧ schema 在盘 ⇒ 承诺已成立 ⇒ 纳管；
/// `api = none` 元素即便 schema 在盘也不纳管（留白违约是判据面的债，不是承诺未兑现）。
#[test]
fn gating_findings留白元素不纳管() {
    let temp = TempDir::new("gating-none-no-gate");
    let fqn = "App.Core.Blank";
    temp.write_schema(fqn, &pilot_schema(fqn));
    let mut blank = element(fqn, &["src/x/"]);
    blank.api = ApiStatus::None;
    blank.doc = Some("guides/nowhere.md".to_owned());
    let findings = vec![api_finding(Some(fqn), ArchiApiFindingKind::ApiUnexpected)];
    assert!(
        super::gating_findings(temp.root(), std::slice::from_ref(&blank), &findings).is_empty(),
        "`api = none` 带 schema 不纳管（`apiUnexpected` 是留白违约，warn 且不进 status）"
    );

    let mut declared = element(fqn, &["src/x/"]);
    declared.api = ApiStatus::Declared;
    declared.doc = None;
    assert_eq!(
        super::gating_findings(temp.root(), std::slice::from_ref(&declared), &findings).len(),
        1,
        "改 `declared` 后同一判据入选（接口文件已在盘）"
    );
}

/// 异常（AC-6 / D6）：`element = None` 的判据落选；同一调用里带归属且契约在案的另一条
/// 入选（同场不互相带过）。
#[test]
fn gating_findings元素归属为none落选() {
    let temp = TempDir::new("gating-none-element");
    let fqn = "App.Core.Alpha";
    temp.write_pair_schema(fqn, &empty_schema(fqn));
    let elements = [element(fqn, &["src/alpha/"])];
    let findings = vec![
        api_finding(None, ArchiApiFindingKind::ApiMissing),
        api_finding(Some(fqn), ArchiApiFindingKind::ApiMissing),
    ];

    let gated = super::gating_findings(temp.root(), &elements, &findings);
    assert_eq!(gated.len(), 1, "无归属者落选、有归属者入选");
    assert_eq!(gated[0].element.as_deref(), Some(fqn));
}

/// 边界（边界 / list 映射）：①`findings = []`②`elements = []`③契约面未启用 → 三种均返回
/// `[]` 且不 panic。
#[test]
fn gating_findings空列表与零元素() {
    let temp = TempDir::new("gating-empty");
    let fqn = "App.Core.Alpha";
    let elements = [element(fqn, &["src/alpha/"])];
    let findings = vec![api_finding(Some(fqn), ArchiApiFindingKind::ApiMissing)];

    assert!(
        super::gating_findings(temp.root(), &elements, &[]).is_empty(),
        "空判据"
    );
    assert!(
        super::gating_findings(temp.root(), &[], &findings).is_empty(),
        "零元素"
    );
    assert!(
        super::gating_findings(temp.root(), &elements, &findings).is_empty(),
        "契约面未启用"
    );
}

/// 边界（边界）：1000 条判据（半数命中）→ 命中条数与序稳定；连调两次逐条相等。
#[test]
fn gating_findings超大列表与去重稳定() {
    let temp = TempDir::new("gating-large");
    let hit = "App.Core.Hit";
    let miss = "App.Core.Miss";
    temp.write_pair_schema(hit, &empty_schema(hit));
    let elements = [element(hit, &["src/hit/"]), element(miss, &["src/miss/"])];
    let findings: Vec<ArchiApiFinding> = (0..1000)
        .map(|index| {
            let owner = if index % 2 == 0 { hit } else { miss };
            api_finding(Some(owner), ArchiApiFindingKind::ApiMissing)
        })
        .collect();

    let first = super::gating_findings(temp.root(), &elements, &findings);
    let second = super::gating_findings(temp.root(), &elements, &findings);
    assert_eq!(first.len(), 500, "半数命中");
    assert_eq!(first, second, "两次调用逐条相等");
    assert!(
        first
            .iter()
            .all(|finding| finding.element.as_deref() == Some(hit)),
        "只留契约在案元素"
    );
}

/// 正向（AC-6 / D7）：契约签名与代码签名不符 → `findings` 返回该条，且与
/// `check(ApiCheckInput)` 直出逐字段相等（同一函数、零第二份装配）。
#[test]
fn findings正常装配产出判据() {
    let temp = TempDir::new("findings-assembly");
    let fqn = "App.Core.Alpha";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/alpha/\"]\n"
    ));
    temp.write_pair_schema(fqn, &mismatch_schema(fqn));
    // 入口形态取 `<element_root>/src/lib.rs`（`entry_form` 的 crate 分支）：缺入口会被
    // ① 结构前提抢先判成 `apiEntryMissing`，签名判据就无从产出
    temp.write("src/alpha/src/lib.rs", MISMATCH_SOURCE);

    let elements = [element(fqn, &["src/alpha/"])];
    let findings = super::findings(temp.root(), &elements).expect("装配应成功");
    assert!(!findings.is_empty(), "非空判据：{findings:?}");
    let mismatch: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiSignatureMismatch)
        .collect();
    assert_eq!(mismatch.len(), 1, "一条签名不符：{findings:?}");
    assert_eq!(mismatch[0].category, "signature", "分类由单点打标");
    assert_eq!(mismatch[0].element.as_deref(), Some(fqn), "归属元素");

    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let direct = check(ApiCheckInput {
        root: temp.root(),
        elements: &elements,
        contracts: &snapshot.contracts,
        schemas: &snapshot.schemas,
        symbols: &snapshot.symbols,
        doc_present: &snapshot.doc_present,
        enabled: true,
    });
    assert_eq!(findings, direct, "与判据单点逐字段相等");
}

/// 边界（AC-6 / D7）：契约面未启用（① 缺席 ② 零载体）→ `Ok(vec![])`；源码面自证
/// `findings` 体内零独立启用判定（启用由快照的载体事实派生，同一 `load_contracts` 装配）。
#[test]
fn findings未启用短路() {
    let absent = TempDir::new("findings-absent");
    let empty_dir = TempDir::new("findings-empty-dir");
    fs::create_dir_all(archi_models_dir(empty_dir.root())).expect("建空契约目录");
    let elements = [element("App.Core.Alpha", &["src/alpha/"])];

    for (tag, temp) in [("目录缺席", &absent), ("零 md", &empty_dir)] {
        assert_eq!(
            super::findings(temp.root(), &elements).expect("未启用不报错"),
            Vec::new(),
            "{tag}：空集"
        );
    }

    let source = include_str!("mod.rs");
    let body = source
        .split_once("pub fn findings(")
        .expect("findings 在案")
        .1
        .split_once("\n}\n")
        .expect("函数体收口")
        .0;
    assert_eq!(
        body.matches("enabled(root, elements)").count(),
        1,
        "启用判据经唯一本体 `enabled`（零第二份判定）：\n{body}"
    );
    assert_eq!(
        body.matches("load_contracts(root, elements)?").count(),
        1,
        "装载单点恰一处：\n{body}"
    );
}

#[test]
fn findings契约非法即err() {
    let temp = TempDir::new("findings-bad-json");
    let fqn = "App.Core.Alpha";
    temp.write_schema(fqn, "{\n  \"$id\": \"App.Core.Alpha.schema.json\"\n}\n");
    temp.write(
        "src/alpha/src/lib.rs",
        "pub fn a() -> String { String::new() }\n",
    );
    let error =
        findings(temp.root(), &[element(fqn, &["src/alpha/"])]).expect_err("非法接口文件：应 Err");
    assert!(
        matches!(error, ApiError::Contract(_)),
        "非法接口归 Contract：{error:?}"
    );
    assert!(
        error
            .message()
            .contains(&paths::schema_rel(fqn).expect("合法 FQN")),
        "文案点名文件：{}",
        error.message()
    );
}

/// 异常（AC-6 / D7）：`mod.rs` 自证装配单点——三新函数各恰一处、`is_file` 只出现在
/// `gating_findings`、`pub mod diff;` / `pub mod render;` 按字母序插入。
#[test]
fn findings装配单点与子模块声明序() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("pub fn findings(").count(),
        1,
        "装配单点恰一处"
    );
    assert_eq!(
        source.matches("pub fn gating_findings(").count(),
        1,
        "筛选单点恰一处"
    );
    assert_eq!(
        source.matches("pub fn artifact_check(").count(),
        1,
        "投影单点恰一处"
    );

    let gating = source
        .split_once("pub fn gating_findings(")
        .expect("gating_findings 在案")
        .1
        .split_once("\n}\n")
        .expect("函数体收口")
        .0;
    assert!(
        gating.contains("schema_on_disk("),
        "接口文件在案的判据经 `schema_on_disk`（`gating_findings` 内）"
    );
    assert_eq!(
        gating.matches("schema_on_disk(").count(),
        1,
        "筛单点内只有一处接口文件在案判据"
    );

    let contract = source.find("pub mod contract;").expect("contract 声明");
    let diff = source.find("pub mod diff;").expect("diff 声明");
    let emit = source.find("pub mod emit;").expect("emit 声明");
    assert!(
        contract < diff && diff < emit,
        "`pub mod diff;` 置 contract 与 emit 之间（字母序）"
    );
    let paths_pos = source.find("pub mod paths;").expect("paths 声明");
    let render = source.find("pub mod render;").expect("render 声明");
    let schema = source.find("pub mod schema;").expect("schema 声明");
    assert!(
        paths_pos < render && render < schema,
        "`pub mod render;` 置 paths 与 schema 之间（字母序）"
    );
}

// ---------------------------------------------------------------------------
// INDEX 的 degraded 口径：全深度标记出现次数（AC-5 / D13）
// ---------------------------------------------------------------------------

/// 演练草案形态的 `api-types` 原文（explore D.1 改写为降级词表形态）：三处标记分别落在
/// `send` 参数位、`session` 属性位、`sessions.items` 元素位，`$defs` 条目自身零标记。
fn rehearsal_types() -> &'static str {
    r##"{
  "AskAnswerActions": {
    "type": "object",
    "properties": {
      "onAnswer": {
        "x-function": {
          "params": [{ "name": "text", "schema": { "type": "string" } }],
          "returns": { "type": "boolean" }
        }
      },
      "pendingToolCallId": { "$ref": "#/$defs/SessionSummary" }
    },
    "required": ["onAnswer"]
  },
  "AgentSessionActions": {
    "type": "object",
    "properties": {
      "running": { "type": "boolean" },
      "send": {
        "x-function": {
          "params": [
            {
              "name": "input",
              "schema": { "type": ["string", "null"], "x-common": { "degraded": "crossLanguage" } }
            }
          ],
          "returns": { "type": "boolean" }
        }
      }
    },
    "required": ["running", "send"]
  },
  "AgentSessionViewProps": {
    "type": "object",
    "properties": {
      "root": { "type": "string" },
      "session": {
        "anyOf": [{ "$ref": "#/$defs/SessionSummary" }, { "type": "null" }],
        "x-common": { "degraded": "noElement" }
      }
    }
  },
  "SessionPanelState": {
    "type": "object",
    "properties": {
      "sessions": {
        "type": "array",
        "items": {
          "anyOf": [{ "$ref": "#/$defs/SessionSummary" }, { "type": "null" }],
          "x-common": { "degraded": "noElement" }
        }
      },
      "total": { "type": "integer" }
    },
    "required": ["sessions"]
  },
  "SessionSummary": {
    "type": "object",
    "properties": { "id": { "type": "string" } },
    "required": ["id"]
  }
}"##
}

/// 演练草案形态的接口定义（`$defs` 取 [`rehearsal_types`]，零 op）。
fn rehearsal_schema(fqn: &str) -> String {
    mirrored_schema_text(fqn, rehearsal_types(), "")
}

/// 落一个 Rust 元素（模型 + 实现入口 + 可选契约），返回其元素投影。
fn write_element(temp: &TempDir, fqn: &str, root: &str, contract: Option<&str>) -> ArchiElement {
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"{root}\"]\n"
    ));
    if let Some(content) = contract {
        temp.write_pair_schema(fqn, content);
    }
    temp.write(&format!("{root}src/lib.rs"), "pub fn f() {}\n");
    element(fqn, &[root])
}

/// 取 `render_all` 产出里的 INDEX 文本。
fn index_text(root: &Path, snapshot: &ApiSnapshot) -> String {
    let files = render_all(root, snapshot).expect("渲染应成功");
    files
        .iter()
        .find(|file| file.path == paths::index_path(root))
        .expect("INDEX 在案")
        .content
        .clone()
}

/// 正向（AC-5 关键 / D13）：演练草案形态 fixture（三处标记全在属性 / 参数位、条目自身零
/// 标记）→ `degraded = 3`——旧条目级口径会给出 0，故本用例钉住「全深度标记出现次数」。
#[test]
fn index_degraded计全深度标记出现次数() {
    let temp = TempDir::new("index-degraded-depth");
    let fqn = "App.Frontend.Components.Agent";
    let elements = [write_element(
        &temp,
        fqn,
        "src/agent/",
        Some(&rehearsal_schema(fqn)),
    )];

    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let index = index_text(temp.root(), &snapshot);
    assert!(
        index.contains("degraded = 3"),
        "三处标记（send 参数位 / session 属性位 / sessions.items）→ degraded = 3：\n{index}"
    );
    assert!(
        index.contains("types = 5"),
        "`$defs` 键数五项（计数口径不改变 `types`）：\n{index}"
    );
    assert!(
        index.contains("status = \"declared\""),
        "契约在案：\n{index}"
    );
}

/// 异常（表驱动，AC-5）：词表外值（`"typo"`）与非字符串值（`true`）→ `load_contracts`
/// 直接 `Err(ApiError::Contract)`（零半真报告、零生成物）。
#[test]
fn load_contracts拒收非法degraded取值() {
    for (label, value) in [("词表外值", "\"typo\""), ("非字符串值", "true")] {
        let temp = TempDir::new("load-illegal-degraded");
        let fqn = "App.Core.Marked";
        let contract = mirrored_schema_text(
            fqn,
            format!(r#""W": {{"type": "object", "x-common": {{"degraded": {value}}}}}"#),
            "",
        );
        let elements = [write_element(&temp, fqn, "src/marked/", Some(&contract))];

        match load_contracts(temp.root(), &elements) {
            Err(ApiError::Contract(message)) => {
                assert!(
                    message.contains("$.$defs.W.x-common.degraded"),
                    "{label}：文案点名该标记 pointer：{message}"
                );
                assert!(
                    message.contains("crossLanguage"),
                    "{label}：文案并列四值：{message}"
                );
            }
            other => panic!("{label}：期望 ApiError::Contract，实际：{other:?}"),
        }
        assert!(
            !paths::index_path(temp.root()).exists(),
            "{label}：装载失败即零生成物（本 crate 零写盘）"
        );
    }
}

/// 边界（表驱动）：零标记 → `degraded = 0` 且键仍输出（不省略）；仅 `x-common.note` 的
/// bootstrap 草稿 → `0` 且契约装载成功（`note` 是自由注记，不进债表）。
#[test]
fn index_degraded零标记与仅note() {
    let cases = [
        ("零标记", r#"{"W": {"type": "object"}}"#),
        (
            "仅 note 的草稿",
            r#"{"W": {"type": "object", "x-common": {"note": "待剪枝：AgentSessionSendInput"}}}"#,
        ),
    ];
    for (label, types) in cases {
        let temp = TempDir::new("index-degraded-zero");
        let fqn = "App.Core.Plain";
        let contract = mirrored_schema_text(fqn, types, "");
        let elements = [write_element(&temp, fqn, "src/plain/", Some(&contract))];

        let snapshot = load_contracts(temp.root(), &elements)
            .unwrap_or_else(|error| panic!("{label}：契约应装载成功，实际 Err：{error:?}"));
        let index = index_text(temp.root(), &snapshot);
        assert!(
            index.contains("degraded = 0"),
            "{label}：`degraded` 键仍输出 0（不省略）：\n{index}"
        );
    }
}

/// 边界（AC-5）：同一元素多处标记（条目自身 + 属性位 + `params[].schema` 深层）→ 计数
/// 等于出现次数（累加，不是条目数）；同一快照两次 `render_all` 逐字相等。
#[test]
fn index_degraded累加且渲染幂等() {
    let temp = TempDir::new("index-degraded-sum");
    let fqn = "App.Core.Deep";
    let contract = mirrored_schema_text(
        fqn,
        r#""W": {"type": "object", "x-common": {"degraded": "noElement"}, "properties": {"deep": {"type": "object", "x-common": {"degraded": "foreignElement"}}, "send": {"x-function": {"params": [{"name": "input", "schema": {"x-common": {"degraded": "crossLanguage"}}}], "returns": {"type": "boolean"}}}}}"#,
        "",
    );
    let elements = [write_element(&temp, fqn, "src/deep/", Some(&contract))];

    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let first = index_text(temp.root(), &snapshot);
    let second = index_text(temp.root(), &snapshot);
    assert_eq!(first, second, "同一快照两次 `render_all` 逐字相等");
    assert!(
        first.contains("degraded = 3"),
        "三处标记累加为 3（不是条目数 1）：\n{first}"
    );
    assert!(
        first.contains("types = 1"),
        "条目数仍为 1（计数口径与 `types` 互不混用）：\n{first}"
    );

    // 计数与校验同源（零第二套遍历）：`index_entries` 只经 `schema::degraded_marks` 取数
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("degraded_marks(").count(),
        1,
        "`mod.rs` 内 `degraded_marks(` 恰 1 处（零第二套扫描）"
    );
    assert!(
        !source.contains("\"x-common\""),
        "`mod.rs` 内零 `x-common` 键字面量（锚点只在 `schema::degraded_marks`）"
    );
}

/// 边界（AC-5）：`absent`（契约文件缺失）元素 → `degraded = 0`，键仍输出（口径改动不
/// 引入缺键形态）。
#[test]
fn index_degraded对absent元素仍输出() {
    let temp = TempDir::new("index-degraded-absent");
    let fqn = "App.Core.Absent";
    let elements = [write_element(&temp, fqn, "src/absent/", None)];

    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功（缺契约不算错）");
    let index = index_text(temp.root(), &snapshot);
    assert!(
        index.contains("status = \"absent\""),
        "契约缺失即 `absent`：\n{index}"
    );
    assert!(
        index.contains("degraded = 0"),
        "`absent` 元素仍输出 `degraded = 0`：\n{index}"
    );
}

/// 正向（AC-6 零回归）：现网契约形态（`properties` 空对象、零 `required`、零
/// `degraded`）→ `types` / `ops` / `degraded` 与改动前逐字相同；该元素的 `.i.rs` /
/// `.schema.json` 渲染文本零 nullable 相关变化。
#[test]
fn index与生成物对现网契约零回归() {
    let temp = TempDir::new("index-degraded-current");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let elements = [write_element(
        &temp,
        fqn,
        "crates/infra/archi/",
        Some(&pilot_schema(fqn)),
    )];

    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    let index = files
        .iter()
        .find(|file| file.path == paths::index_path(temp.root()))
        .expect("INDEX 在案");
    for expected in ["types = 1", "ops = 1", "degraded = 0"] {
        assert!(
            index.content.contains(expected),
            "现网契约的索引三项逐字不变（缺 `{expected}`）：\n{}",
            index.content
        );
    }

    // 手写接口文件不进应生成集：三类生成物之外零 `.schema.json`
    assert!(
        !files.iter().any(|file| {
            file.path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".schema.json"))
        }),
        "运行期零 `.schema.json` 产出：{:?}",
        files.iter().map(|file| &file.path).collect::<Vec<_>>()
    );

    let generated = files
        .iter()
        .find(|file| {
            file.path
                == paths::generated_path(
                    temp.root(),
                    "crates/infra/archi/src/lib.rs",
                    paths::GeneratedKind::Rust,
                )
                .expect("Rust 生成物路径")
        })
        .unwrap_or_else(|| {
            panic!(
                ".i.rs 生成物在案：{:?}",
                files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect::<Vec<_>>()
            )
        });
    for forbidden in ["Option<", "serde_json::Value", "unknown"] {
        assert!(
            !generated.content.contains(forbidden),
            "现网 `.i.rs` 零 nullable 相关变化（`{forbidden}` 零命中）：\n{}",
            generated.content
        );
    }
}

// ---------------------------------------------------------------------------
// 注记真值源：注记行取 `ApiSymbol.file`（入口退出注记面）
// ---------------------------------------------------------------------------

/// 合成 barrel 语料：父元素 + 4 子元素 + 1 跨元素兄弟（源码与模型同落盘）。
///
/// barrel 6 条语句含「同一目标两条具名语句」形态与 1 条跨元素被拒形态；子元素目录里
/// 的 `*ExtraProps` 不在名单里，用于钉「具名名单不虚增」。
fn write_closure_barrel(temp: &TempDir) {
    temp.write(
        "views/agent/index.ts",
        "export { useAgentInput, AgentInputProps } from './input/agent-input';\n\
         export { useSession } from './session/session-panel';\n\
         export type { SessionPanelState, SessionPanelActions } from './session/session-panel';\n\
         export { AgentMessages } from './messages/agent-messages';\n\
         export type { MessageList } from './messages/agent-messages';\n\
         export { Other } from '../other/other';\n",
    );
    temp.write(
        "views/agent/input/agent-input.ts",
        "export function useAgentInput(): boolean { return true; }\n\
         export interface AgentInputProps { value: string }\n\
         export interface AgentInputExtraProps { extra: string }\n",
    );
    temp.write(
        "views/agent/session/session-panel.ts",
        "export function useSession(): boolean { return true; }\n\
         export interface SessionPanelState { open: boolean }\n\
         export interface SessionPanelActions { close(): void }\n",
    );
    temp.write(
        "views/agent/messages/agent-messages.ts",
        "export function AgentMessages(): boolean { return true; }\n\
         export type MessageList = string[];\n",
    );
    temp.write(
        "views/other/other.ts",
        "export function Other(): string { return ''; }\n",
    );
    temp.write_model(&format!(
        "[element.\"{agent}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"views/agent/\"]\n\n\
         [element.\"{input}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"views/agent/input/\"]\n\n\
         [element.\"{session}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"views/agent/session/\"]\n\n\
         [element.\"{messages}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"views/agent/messages/\"]\n\n\
         [element.\"{panel}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"views/agent/panel/\"]\n\n\
         [element.\"{other}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"views/other/\"]\n",
        agent = CLOSURE_AGENT,
        input = "App.Frontend.Components.Agent.Input",
        session = "App.Frontend.Components.Agent.Session",
        messages = "App.Frontend.Components.Agent.Messages",
        panel = "App.Frontend.Components.Agent.Panel",
        other = "App.Frontend.Components.Other",
    ));
}

/// 合成 barrel 的父元素 FQN。
const CLOSURE_AGENT: &str = "App.Frontend.Components.Agent";

/// 合成 barrel 的元素表（与 [`write_closure_barrel`] 同源，供 `load_contracts` /
/// `findings` / `extract_element` 直接入参）。
fn closure_elements() -> Vec<ArchiElement> {
    vec![
        element(CLOSURE_AGENT, &["views/agent/"]),
        element(
            "App.Frontend.Components.Agent.Input",
            &["views/agent/input/"],
        ),
        element(
            "App.Frontend.Components.Agent.Session",
            &["views/agent/session/"],
        ),
        element(
            "App.Frontend.Components.Agent.Messages",
            &["views/agent/messages/"],
        ),
        element(
            "App.Frontend.Components.Agent.Panel",
            &["views/agent/panel/"],
        ),
        element("App.Frontend.Components.Other", &["views/other/"]),
    ]
}

/// 合成 barrel 的契约源文：2 个 op + 2 个 `$defs` 类型（均在闭包符号集内）。
fn closure_contract(fqn: &str) -> String {
    mirrored_schema_text(
        fqn,
        r#""AgentInputProps": {"type": "object", "properties": {}},"MessageList": {"type": "array", "items": {"type": "string"}}"#,
        format!(
            "{}, {}",
            r#""useAgentInput": { "x-function": {"params": [], "returns": {"type": "boolean"}} }"#,
            r#""useSession": { "x-function": {"params": [], "returns": {"type": "boolean"}} }"#
        ),
    )
}

/// 正向（AC-5 / AC-8 组合链路）：对有契约文件的 TS barrel 元素跑 `load_contracts` ⇒
/// `snapshot.symbols[fqn]` == `extract_element` 的闭包产出（逐字段相等，含 `file`）——
/// `element_symbols` 补参生效、零第二套取数。
#[test]
fn load_contracts组合闭包取数面() {
    let temp = TempDir::new("load-closure-symbols");
    write_closure_barrel(&temp);
    temp.write_pair_schema(CLOSURE_AGENT, &closure_contract(CLOSURE_AGENT));

    let elements = closure_elements();
    let snapshot = load_contracts(temp.root(), &elements).expect("全链应成功");
    assert_eq!(snapshot.contracts.len(), 1, "只有父元素有契约文件");
    assert_eq!(snapshot.contracts[0].fqn, CLOSURE_AGENT);

    let expected =
        extract::extract_element(temp.root(), &elements, &elements[0], "views/agent/index.ts");
    assert_eq!(
        snapshot.symbols[CLOSURE_AGENT], expected,
        "`snapshot.symbols` == 闭包产出（逐字段相等，含 file）"
    );
    assert_eq!(snapshot.symbols[CLOSURE_AGENT].len(), 7, "闭包 7 名在案");
}

/// 正向（AC-8 组合链路）：契约声明闭包内 2 个 op + 2 个 `$defs` 类型 ⇒ 该 4 项零
/// `apiMissing`；`apiUndeclared` 仍 `off`（零条目）、闭包只减不增。
#[test]
fn findings组合闭包只减不增() {
    let temp = TempDir::new("findings-closure");
    write_closure_barrel(&temp);
    temp.write_pair_schema(CLOSURE_AGENT, &closure_contract(CLOSURE_AGENT));

    let elements = closure_elements();
    let found = findings(temp.root(), &elements).expect("判据装配应成功");
    let kinds: Vec<ArchiApiFindingKind> = found.iter().map(|finding| finding.kind).collect();
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == ArchiApiFindingKind::ApiMissing)
            .count(),
        0,
        "闭包覆盖了契约声明的 op 与 `$defs` 类型（零 apiMissing）：{:?}",
        found.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == ArchiApiFindingKind::ApiUndeclared)
            .count(),
        0,
        "`apiUndeclared` 档位 off ⇒ 零新增条目"
    );
    assert_eq!(
        ArchiApiFindingKind::ApiUndeclared.severity(),
        crate::drift::report::ApiFindingSeverity::Off,
        "档位口径不变（off 是本变更的既有事实）"
    );
    for finding in &found {
        assert_eq!(
            finding.kind,
            ArchiApiFindingKind::DocMissing,
            "父元素之外的五份无载体元素是唯一余下条目：{}",
            finding.message
        );
    }
}

/// 边界（AC-5 注记面）：`api/mod.rs` 内 `@ {entry}` 与 `@ 入口` 零命中；骨架由
/// `schema_draft(fqn, symbols)` 与 `doc_draft(fqn)` 两段拼装（`bootstrap_text` / 段模板常量
/// 已随双草稿退役）；`bootstrap` 公开签名改返回 [`BootstrapDrafts`]。
#[test]
fn 注记面entry退出源码自证() {
    let source = include_str!("mod.rs");
    for forbidden in ["@ {entry}", "@ 入口"] {
        assert!(
            !source.contains(forbidden),
            "注记面 MUST NOT 用入口串拼接：`{forbidden}`"
        );
    }
    // 骨架由 `schema_draft` / `doc_draft` 两段拼装（`bootstrap_text` 已随双草稿退役）
    assert_eq!(
        source
            .matches("fn schema_draft(fqn: &str, symbols: &[ApiSymbol]) -> String")
            .count(),
        1,
        "`schema_draft` 形参为 `(fqn, symbols)`（`entry` 退出）"
    );
    assert_eq!(
        source.matches("fn doc_draft(fqn: &str) -> String").count(),
        1,
        "`doc_draft` 只吃 FQN（文档无结构、零符号依赖）"
    );
    assert_eq!(
        source
            .matches(
                "pub fn bootstrap(root: &Path, element: &str) -> Result<BootstrapDrafts, ApiError>"
            )
            .count(),
        1,
        "`bootstrap` 返回双草稿"
    );
    assert!(
        !source.contains("BootstrapNote") && !source.contains("BOOTSTRAP_NOTE"),
        "段模板常量已退役"
    );
}

/// 边界（AC-7）：两处产品调用点各恰 1 次 `extract_element(` 且实参含元素表 / 元素——
/// 取数面单点，零第三处。
#[test]
fn bootstrap与element_symbols调用点机械锚() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("extract::extract_element(").count(),
        2,
        "`api/` 内产品侧恰 2 处调用点（`element_symbols` 与 `bootstrap`）"
    );
    assert!(
        source.contains("extract::extract_element(root, elements, element, &entry)"),
        "`element_symbols` 传元素表与元素（逐字）"
    );
    assert!(
        source.contains("extract::extract_element(root, &elements, found, &entry)"),
        "`bootstrap` 传元素表与元素（逐字）"
    );
}

// ---------------------------------------------------------------------------
// enabled：模型文件不是启用证据（`module-entry-docs-semantics` / AC-4）
//
// 判据 = `models/`（== 契约资产目录）递归含 ≥1 份 `*.md`；`graph/model/` 一级的
// `NN-<域>.toml` 是模型声明，**不是**启用证据——「有模型、零契约 md」必须是未启用。
// ---------------------------------------------------------------------------

/// 异常（AC-4 反向判据）：`graph/model/` 一级恰 6 个 `NN-<域>.toml`（外加 schema 与
/// 索引），而 `models/` 递归零 `*.md` → 未启用。
#[test]
fn enabled模型文件不是启用证据() {
    let temp = TempDir::new("enabled-models-only");
    for name in [
        "01-app",
        "02-backend",
        "03-infra",
        "04-frontend",
        "05-checks",
        "06-tools",
    ] {
        temp.write(
            &format!(".dev-team/architecture/graph/model/{name}.toml"),
            "[element.\"App\"]\nkind = \"package\"\n",
        );
    }
    temp.write(
        ".dev-team/architecture/models/Backend/Infra/App.Backend.Infra.ArchiRuntime.schema.json",
        "{\"$defs\": {}}\n",
    );
    temp.write(".dev-team/architecture/graph/INDEX.toml", "# 空索引\n");

    assert!(
        !enabled(temp.root(), &[element("App.Core.Alpha", &["src/alpha/"])]),
        "6 份模型 toml + schema + 索引而零载体 ⇒ 未启用（模型文件不是启用证据）"
    );

    let elements = [element("App.Core.Alpha", &["src/alpha/"])];
    assert_eq!(
        findings(temp.root(), &elements).expect("未启用不报错"),
        Vec::new(),
        "有模型、零契约 md ⇒ 契约判据空集（与 enabled 同一条判据）"
    );
}

/// 正向（AC-4）：模型声明 toml 与契约 md 并存 → 启用（两目录分家零干扰）；`models/`
/// 与 `graph/` 两目录都在但零 md 的冷启动态 → 未启用且零 panic。
#[test]
fn enabled模型与契约混居与冷启动态() {
    let mixed = TempDir::new("enabled-mixed");
    mixed.write(
        ".dev-team/architecture/graph/model/01-app.toml",
        "[element.\"App\"]\nkind = \"package\"\n",
    );
    let fqn = "App.Backend.Infra.ArchiRuntime";
    mixed.write_pair(fqn);
    let carriers = [element(fqn, &["crates/infra/archi/"])];
    assert!(
        enabled(mixed.root(), &carriers),
        "模型 toml + 含 fence 的载体并存 ⇒ 启用（模型文件不遮蔽载体）"
    );

    let cold = TempDir::new("enabled-cold-start");
    fs::create_dir_all(archi_models_dir(cold.root())).expect("建契约目录");
    let index = paths::index_path(cold.root());
    fs::create_dir_all(index.parent().expect("索引有父目录")).expect("建索引目录");
    assert!(
        !enabled(cold.root(), &carriers),
        "两目录在、零载体 ⇒ 未启用（零 panic）"
    );
}

/// 边界（D-5 单点判定自证）：启用判据本体只在 `enabled` 一处（`schema_path` + `is_file`，
/// 零 `contract_path`）；`artifact_check` 与 `findings` 各恰一处调用它；旧 fence 面判据零残留。
#[test]
fn enabled判定单点自证() {
    let source = include_str!("mod.rs");
    let start = source.find("pub fn enabled(").expect("`enabled` 在案");
    let rest = &source[start..];
    let end = rest.find("\n}\n").expect("函数体收尾");
    let body = &rest[..end];
    assert!(
        body.contains("schema_on_disk("),
        "启用判据本体经 `schema_on_disk` 单点：{body}"
    );
    assert!(
        !body.contains("contract_path("),
        "文档在盘与否零参与（`docMissing` 永不纳管）：{body}"
    );
    for retired in ["has_api_fence", "facts_enabled", "doc_facts"] {
        assert_eq!(
            source.matches(retired).count(),
            0,
            "旧判据 `{retired}` 已整体退役"
        );
    }
    // 两处消费方各恰一处调用（生成预览 + 判据装配），零第三处
    for caller in ["artifact_check", "findings"] {
        let start = source.find(&format!("pub fn {caller}(")).expect("函数在案");
        let rest = &source[start..];
        let end = rest.find("\n}\n").expect("函数体收尾");
        assert_eq!(
            rest[..end].matches("enabled(root").count(),
            1,
            "`{caller}` 恰好经 `enabled` 一次"
        );
    }
}

/// 正向（AC-4 读数守恒）：契约 md 与模型 toml 同住 `models/`（md 在 `<分层目录…>/` 子目录）
/// 时启用，`load_contracts` 正常出三表——模型文件混居零结构诊断。
#[test]
fn load_contracts双身份混居仍出三表() {
    let temp = TempDir::new("mixed-dual-identity");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&format!(
        "[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"crates/infra/archi/\"]\n"
    ));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    temp.write_pair(fqn);

    let elements = [element(fqn, &["crates/infra/archi/"])];
    assert!(
        enabled(temp.root(), &elements),
        "md 落 `models/<分层目录…>/` 子目录仍启用"
    );
    let snapshot = load_contracts(temp.root(), &elements).expect("混居不产出结构诊断");
    assert_eq!(snapshot.contracts.len(), 1, "contracts 就位");
    assert_eq!(snapshot.schemas.len(), 1, "schemas 就位");
    assert!(
        snapshot.symbols.contains_key(fqn) && !snapshot.symbols[fqn].is_empty(),
        "symbols 就位（模型 toml 混居不影响取数面）"
    );
}

/// 边界（AC-4 读数守恒）：`apiFindings` 的计数只由契约判据决定——有契约元素零判据
/// （`apiEntryMissing 0`）、有入口无契约者各一条 `apiMissing`，模型 toml 与契约子树
/// 混居同一目录不产生附加判据（fixture 级等价于真实仓库的 `0 + N` 分桶）。
#[test]
fn findings计数与模型文件混居无关() {
    let temp = TempDir::new("findings-counts-stable");
    let with_contract = "App.Core.Alpha";
    let without = "App.Core.Beta";
    temp.write_model(&format!(
        "[element.\"{with_contract}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"src/alpha/\"]\n\n\
         [element.\"{without}\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"src/beta/\"]\n"
    ));
    temp.write("src/alpha/src/lib.rs", "pub fn alpha() {}\n");
    temp.write("src/beta/src/lib.rs", "pub fn beta() {}\n");
    // 含 `api-types` fence 且类型在代码在案：声明集已兑现 ⇒ 该元素零判据，同时把契约面
    // 置为启用（否则 `enabled == false` 一律空集）。
    temp.write("src/alpha/src/lib.rs", "pub struct LocalCheck;\n");
    temp.write_pair_schema(
        with_contract,
        &mirrored_schema_text(
            with_contract,
            r#""LocalCheck": {"type": "object", "properties": {}}"#,
            "",
        ),
    );

    let elements = [
        element(with_contract, &["src/alpha/"]),
        element(without, &["src/beta/"]),
    ];
    let found = findings(temp.root(), &elements).expect("判据应成功");
    let count = |kind: &str| -> usize {
        found
            .iter()
            .filter(|finding| finding.kind.as_str() == kind)
            .count()
    };
    assert_eq!(count("apiEntryMissing"), 0, "两元素入口均可解析");
    assert_eq!(
        count("apiMissing"),
        0,
        "改判后不再有「契约缺失」形态的 apiMissing：{:?}",
        found
            .iter()
            .map(|finding| (finding.kind.as_str(), finding.element.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        count("apiDocMissing"),
        0,
        "契约 md 带 fence ⇒ 零 apiDocMissing：{:?}",
        found
            .iter()
            .map(|finding| (finding.kind.as_str(), finding.element.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        count("docMissing"),
        1,
        "无契约 md 的元素 ⇒ 一条 docMissing（warn 档）：{:?}",
        found
            .iter()
            .map(|finding| (finding.kind.as_str(), finding.element.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        found.len(),
        1,
        "模型 toml 与契约子树混居零附加判据：{:?}",
        found
            .iter()
            .map(|finding| finding.kind.as_str())
            .collect::<Vec<_>>()
    );
}

/// 边界（AC-5 · 离线可推读数）：真实仓库的 `path` 非空元素 44 个，其中 `api = none`
/// 2 个（留白未声明 `doc`）、契约在案且已兑现 1 个（零判据），其余 41 个入口文档未就位
/// ⇒ `docMissing 43`（`41` 份改名 + `2` 个新入面）、`apiMissing 0`、分母 44。换根只改
/// 契约落点，该算术不变，故把它写成 fixture 级机械事实：判据计数只由「元素声明 + 载体
/// 事实」决定，与契约目录叫什么名字无关。
#[test]
fn findings离线可推读数四十四减二减一() {
    let temp = TempDir::new("findings-offline-counts");
    let mut elements: Vec<ArchiElement> = Vec::new();
    for index in 0..44 {
        let fqn = format!("App.Core.E{index:02}");
        let path = format!("src/e{index:02}/");
        temp.write(&format!("{path}src/lib.rs"), "pub fn body() {}\n");
        let mut declared = element(&fqn, &[path.as_str()]);
        if index >= 42 {
            declared.api = ApiStatus::None;
        }
        elements.push(declared);
    }
    let in_place = "App.Core.E00";
    temp.write("src/e00/src/lib.rs", "pub struct LocalCheck;\n");
    temp.write_pair_schema(
        in_place,
        &mirrored_schema_text(
            in_place,
            r#""LocalCheck": {"type": "object", "properties": {}}"#,
            "",
        ),
    );

    // 夹具自证：44 个 `path` 非空元素、`api = none` 恰 2 个、契约在案恰 1 份。
    assert_eq!(
        elements
            .iter()
            .filter(|element| !element.path.is_empty())
            .count(),
        44,
        "`path` 非空元素恰 44 个"
    );
    assert_eq!(
        elements
            .iter()
            .filter(|element| element.api == ApiStatus::None)
            .count(),
        2,
        "`api = none` 的元素恰 2 个（留白不入判据面）"
    );
    assert_eq!(
        elements
            .iter()
            .filter(|element| paths::contract_path(temp.root(), &element.fqn)
                .is_some_and(|path| path.is_file()))
            .count(),
        1,
        "契约文件在案恰 1 份（落在 `models/Core/` 下）"
    );

    let found = findings(temp.root(), &elements).expect("判据应成功");
    let count = |kind: &str| -> usize {
        found
            .iter()
            .filter(|finding| finding.kind.as_str() == kind)
            .count()
    };
    assert_eq!(
        count("apiEntryMissing"),
        0,
        "44 个入口全部可解析 ⇒ apiEntryMissing 0"
    );
    assert_eq!(
        count("apiMissing"),
        0,
        "改判后零 apiMissing（契约缺失不再是本判据）：{:?}",
        found
            .iter()
            .map(|finding| (finding.kind.as_str(), finding.element.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        count("apiDocMissing"),
        0,
        "契约在案且带 fence ⇒ 零 apiDocMissing：{:?}",
        found
            .iter()
            .map(|finding| (finding.kind.as_str(), finding.element.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        count("docMissing"),
        43,
        "44 − 1 = 43 条 docMissing（41 份改名 + 2 个留白未声明 doc）：{:?}",
        found
            .iter()
            .map(|finding| (finding.kind.as_str(), finding.element.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        found.len(),
        43,
        "判据总数 43（`docMissing 43` 独占，分母 44 减已兑现 1 份）"
    );
}

// ===========================================================================
// 启用判据：`<叶段>.schema.json` 在盘
// ===========================================================================

/// 正向：两份元素其一 `<叶段>.schema.json` 在盘即启用；元素顺序与 `path` 段数无关。
#[test]
fn enabled任一份载体含fence即启用() {
    let temp = TempDir::new("enabled-any-carrier");
    let live = "App.Core.Live";
    let dark = "App.Core.Dark";
    temp.write_schema(live, &pilot_schema(live));
    let elements = [element(live, &["src/live/"]), element(dark, &["src/dark/"])];
    assert!(
        enabled(temp.root(), &elements),
        "两份元素其一 schema 在盘即启用"
    );

    // 深度无关：任意分层深度的 `<叶段>.schema.json` 都按 FQN 派生落点判定
    for index in ["App.Core.Deep", "App.Backend.Infra.Agent.Session"] {
        let temp = TempDir::new(&format!("enabled-depth-{index}"));
        temp.write_schema(index, &pilot_schema(index));
        assert!(
            enabled(temp.root(), &[element(index, &["src/pos/"])]),
            "{index} 的 schema 在盘仍命中"
        );
    }
}

/// 异常（AC-7 反向判据）：旧「递归任意 `*.md` / 任意 `*.schema.json`」形态一律**不**构成
/// 启用证据——文档位的历史 fence、`models/README.md`、`models/` 仅 `*.toml`、目录缺席。
#[test]
fn enabled旧判据残留形态一律不启用() {
    let fqn = "App.Core.Candidate";

    // ① 文档位（历史形态：正文含 `api-op` fence）不是启用证据
    let doc_block = TempDir::new("enabled-plain-block");
    doc_block.write_contract(fqn, &legacy_doc_with_fences(fqn));
    assert!(
        !enabled(doc_block.root(), &[element(fqn, &["src/b/"])]),
        "文档位的 fence 不是接口事实"
    );

    // ② `models/README.md`（旧判据的「任意 md」形态）
    let readme = TempDir::new("enabled-readme");
    readme.write(".dev-team/architecture/models/README.md", "# 说明\n");
    assert!(
        !enabled(readme.root(), &[element(fqn, &["src/b/"])]),
        "README.md 不是任何元素的接口文件"
    );

    // ③ `graph/model/` 仅 `*.toml`（模型声明不是启用证据）
    let toml_only = TempDir::new("enabled-toml-only");
    toml_only.write(
        ".dev-team/architecture/graph/model/01-app.toml",
        "[element.\"App\"]\nkind = \"package\"\n",
    );
    assert!(
        !enabled(toml_only.root(), &[element(fqn, &["src/b/"])]),
        "模型文件不是启用证据"
    );

    // ④ `models/` 缺席（冷启动）+ 零元素
    let bare = TempDir::new("enabled-absent-models");
    assert!(
        !enabled(bare.root(), &[element(fqn, &["src/b/"])]),
        "契约目录缺席 ⇒ 未启用（零 panic）"
    );
    assert!(!enabled(bare.root(), &[]), "零元素 ⇒ 未启用（零读盘）");
}

/// 边界：启用后缺派生 `<叶段>.md` 的 declared 元素 ⇒ 恰一条 `docMissing`（warn 档，不是
/// `Err`；单因单义，与接口文件在盘与否无关）；空元素表 → 空 `Vec`；两次调用逐条相等。
#[test]
fn findings边界_单条docmissing与去重稳定() {
    let temp = TempDir::new("findings-docmissing");
    // 启用证据：declared 元素的接口文件在盘
    let live = "App.Core.Live";
    temp.write_pair(live);
    // live 的入口与代码面齐备（文档在盘 ⇒ 零判据）
    temp.write(
        "src/live/src/lib.rs",
        "pub struct LocalArchiCheck;
impl LocalArchiCheck {
    pub fn file_stem(timestamp: &str) -> String { String::new() }
}
",
    );
    // 未写文档的 declared 元素：派生 `<叶段>.md` 不在盘 ⇒ 恰一条 `docMissing`
    let no_doc = "App.Core.NoDoc";
    let elements = [
        element(live, &["src/live/"]),
        element(no_doc, &["src/nodoc/"]),
    ];

    let first = findings(temp.root(), &elements).expect("缺文档不是错误");
    assert_eq!(first.len(), 1, "一条 docMissing：{first:?}");
    assert_eq!(first[0].kind, ArchiApiFindingKind::DocMissing, "判据种类");
    assert_eq!(first[0].category, "doc", "分类 doc");
    assert_eq!(
        first[0].element.as_deref(),
        Some(no_doc),
        "判据归属缺文档的元素"
    );
    let second = findings(temp.root(), &elements).expect("第二次");
    assert_eq!(first, second, "两次调用逐条相等（报告面可重复）");

    assert!(
        findings(temp.root(), &[]).expect("空元素表").is_empty(),
        "空元素表 ⇒ 空 Vec"
    );
}

/// 异常（D-5）：`findings` 体内零独立启用判定——`load_contracts(` 与 `enabled(root, elements)`
/// 各恰 1 次（两处同源单点），文档事实经快照读取，零直接 `is_file(` 判定。
#[test]
fn findings启用判据不独立装配() {
    let source = include_str!("mod.rs");
    let start = source.find("pub fn findings(").expect("`findings` 在案");
    let rest = &source[start..];
    let end = rest.find("\n}\n").expect("函数体收尾");
    let body = &rest[..end];
    assert_eq!(
        body.matches("load_contracts(").count(),
        1,
        "装载单点恰一处：{body}"
    );
    assert_eq!(
        body.matches("enabled(root, elements)").count(),
        1,
        "启用判据经 `enabled` 单点（不自行判定）：{body}"
    );
    assert!(
        body.contains("doc_present: &snapshot.doc_present"),
        "文档事实经快照读取（零第二份装配）：{body}"
    );
    assert!(
        !body.contains("is_file("),
        "`findings` 体内零直接读盘判定（存在性判定只在 `enabled` / `doc_presence`）：{body}"
    );
}

/// 异常：`api = none` 元素的 `doc` 载体存在但不可读 ⇒ 按「无 fence」处置、`enabled`
/// 为假、不触发 `ApiError::Io`（探针与契约解析两条错误面分工）。
#[test]
fn findings不可读载体走未启用而非io错误() {
    let temp = TempDir::new("findings-unreadable");
    let fqn = "App.Core.Bin";
    let carrier = temp.root().join("guides/binary.md");
    fs::create_dir_all(carrier.parent().expect("指南目录")).expect("建指南目录");
    fs::write(&carrier, [0xff_u8, 0x00, 0x1f]).expect("写非法 UTF-8");

    let mut blank = element(fqn, &["src/bin/"]);
    blank.api = ApiStatus::None;
    blank.doc = Some("guides/binary.md".to_owned());

    let produced = match findings(temp.root(), &[blank]) {
        Ok(produced) => produced,
        Err(error) => panic!("不可读载体不得打成 Io 错误：{error:?}"),
    };
    assert!(produced.is_empty(), "启用判据为假 ⇒ 空判据集：{produced:?}");
}

/// 异常：无模型（`.dev-team` 缺席）→ `artifact_check` 仍 `Ok` 且 `enabled = false`
/// （冷启动态），不产出半真结果。
#[test]
fn artifact_check冷启动ok且未启用() {
    let temp = TempDir::new("artifact-cold-start");
    let dto = super::artifact_check(temp.root()).expect("无模型不是错误");
    assert!(!dto.enabled, "冷启动 ⇒ 未启用");
    assert!(dto.mismatched.is_empty(), "零不符");
    assert!(dto.orphaned.is_empty(), "零孤儿");
    assert!(
        !dto.missing.is_empty(),
        "索引生成物仍照实报缺失（应生成集恒含 INDEX.toml）"
    );
}

/// 边界：`artifact_check` 的元素表只取一次（源码自证 `elements_of(root)?` 恰 1 处），
/// 且 `enabled` 由元素表派生、字段形状零变化。
#[test]
fn artifact_check元素表取数单点() {
    let source = include_str!("mod.rs");
    let start = source
        .find("pub fn artifact_check(")
        .expect("`artifact_check` 在案");
    let rest = &source[start..];
    let end = rest.find("\n}\n").expect("函数体收尾");
    let body = &rest[..end];
    assert_eq!(
        body.matches("elements_of(root)?").count(),
        1,
        "元素表只取一次：{body}"
    );
    assert!(
        body.contains("enabled: enabled(root, &elements)"),
        "`enabled` 由元素表派生：{body}"
    );
}

/// 异常：判据归属的元素不在 `elements` 表内 ⇒ 落选（无承诺可对）；`docMissing` 两支
/// （`declared` 载体不在盘 / `none` 未声明）都不纳管。
#[test]
fn gating_findings不在表内与两条docmissing均落选() {
    let temp = TempDir::new("gating-not-in-table");
    let fqn = "App.Core.Alpha";
    temp.write_pair_schema(fqn, &empty_schema(fqn));
    let elements = [element(fqn, &["src/alpha/"])];
    let stranger = vec![api_finding(
        Some("App.Core.Stranger"),
        ArchiApiFindingKind::ApiMissing,
    )];
    assert!(
        super::gating_findings(temp.root(), &elements, &stranger).is_empty(),
        "元素不在表内 ⇒ 落选"
    );

    let mut blank = element("App.Core.Blank", &["src/blank/"]);
    blank.api = ApiStatus::None;
    // 本臂的 declared 元素**契约 md 不在盘**（`docMissing` 的另一支）
    let in_play = [element("App.Core.NoContract", &["src/nc/"]), blank];
    let doc_missing = vec![
        api_finding(Some("App.Core.Blank"), ArchiApiFindingKind::DocMissing),
        api_finding(Some("App.Core.NoContract"), ArchiApiFindingKind::DocMissing),
    ];
    assert!(
        super::gating_findings(temp.root(), &in_play, &doc_missing).is_empty(),
        "docMissing 两支都不纳管（承诺尚未成立）"
    );
}

/// 边界：`declared` 元素写了 `doc` 且载体在盘（判据面报 `apiUnexpected`）→ 仍纳管
/// （承诺已成立即纳管，矛盾本身照常进 status）；条目顺序与输入顺序一致。
#[test]
fn gating_findings留白矛盾仍纳管且保序() {
    let temp = TempDir::new("gating-unexpected");
    let fqn = "App.Core.Alpha";
    temp.write_pair_schema(fqn, &empty_schema(fqn));
    let mut subject = element(fqn, &["src/alpha/"]);
    subject.doc = Some("guides/a.md".to_owned());
    let findings = vec![
        api_finding(Some(fqn), ArchiApiFindingKind::ApiUnexpected),
        api_finding(Some(fqn), ArchiApiFindingKind::ApiUnexpected),
        api_finding(Some(fqn), ArchiApiFindingKind::ApiUnexpected),
    ];
    let gated = super::gating_findings(temp.root(), &[subject], &findings);
    assert_eq!(gated.len(), 3, "载体在盘 ⇒ 矛盾判据仍纳管");
    assert_eq!(gated, findings, "过滤不改序");
}

// ===========================================================================
// `entry_problem_reason`：四值成因措辞单点（`resolvedEntry.problem` 与
// `bootstrap` 失败文案同源）
// ===========================================================================

/// 正向（表驱动）：四变体逐值 → 文案含对应关键片段（入口串 / 另一 FQN / 「形态非法」
/// /「不在盘」/「之外」）。
#[test]
fn entry_problem_reason四变体关键片段() {
    use crate::api::paths::EntryProblem;

    let cases: [(EntryProblem, &str); 4] = [
        (
            EntryProblem::Malformed {
                entry: "C:/abs/lib.rs".to_owned(),
            },
            "形态非法",
        ),
        (
            EntryProblem::OutsidePath {
                entry: "src/other/lib.rs".to_owned(),
            },
            "之外",
        ),
        (
            EntryProblem::NotOnDisk {
                entry: "src/auth/lib.rs".to_owned(),
            },
            "不在盘",
        ),
        (
            EntryProblem::Duplicated {
                entry: "src/auth/lib.rs".to_owned(),
                other_fqn: "App.AuthCopy".to_owned(),
            },
            "同一个文件",
        ),
    ];
    for (problem, needle) in cases {
        let reason = super::entry_problem_reason(&problem);
        assert!(reason.contains(needle), "成因含「{needle}」：{reason}");
        match &problem {
            EntryProblem::Duplicated {
                other_fqn, entry, ..
            } => {
                assert!(reason.contains(other_fqn), "冒泡另一元素 FQN：{reason}");
                assert!(reason.contains(entry), "含入口串：{reason}");
            }
            EntryProblem::Malformed { entry }
            | EntryProblem::OutsidePath { entry }
            | EntryProblem::NotOnDisk { entry } => {
                assert!(reason.contains(entry), "含入口串：{reason}");
            }
        }
    }
}

/// 边界：同值两次调用文案逐字相等（纯函数确定性）。
#[test]
fn entry_problem_reason确定性() {
    use crate::api::paths::EntryProblem;

    let problem = EntryProblem::NotOnDisk {
        entry: "src/auth/lib.rs".to_owned(),
    };
    assert_eq!(
        super::entry_problem_reason(&problem),
        super::entry_problem_reason(&problem),
        "同值两次调用逐字相等"
    );
}

/// 边界（表驱动）：入口串含特殊字符（`\n` / emoji / `：`）→ 原样插入、不转义不 panic。
#[test]
fn entry_problem_reason特殊字符原样插入() {
    use crate::api::paths::EntryProblem;

    for entry in ["src/a\nb/lib.rs", "src/认证🎉/lib.rs", "src/a：b/lib.rs"] {
        let reason = super::entry_problem_reason(&EntryProblem::NotOnDisk {
            entry: entry.to_owned(),
        });
        assert!(reason.contains(entry), "原样插入：{reason}");
        assert!(
            !reason.contains("\\n") && !reason.contains("\\\\"),
            "不转义特殊字符：{reason}"
        );
    }
}

/// 异常：入口串为空串 → 文案仍完整（`` `` `` 包裹形态），零 panic。
#[test]
fn entry_problem_reason空入口串仍完整() {
    use crate::api::paths::EntryProblem;

    let reason = super::entry_problem_reason(&EntryProblem::OutsidePath {
        entry: String::new(),
    });
    assert!(reason.contains("``"), "空串以反引号包裹：{reason}");
    assert!(reason.contains("之外"), "结构完整：{reason}");
}

/// 组合：四值措辞与 `check::entry_missing_message` 同源——两处文案的关键片段一致
/// （既有措辞面零漂移）。
#[test]
fn entry_problem_reason与check文案关键片段一致() {
    let check_source = include_str!("check.rs");
    let reason_source = include_str!("mod.rs");
    for (variant, needle) in [
        ("Malformed", "形态非法"),
        ("OutsidePath", "之外"),
        ("NotOnDisk", "不在盘"),
        ("Duplicated", "同一个文件"),
    ] {
        assert!(
            check_source.contains(&format!("EntryProblem::{variant}")),
            "check 侧覆盖 {variant}"
        );
        assert!(
            reason_source.contains(&format!("EntryProblem::{variant}")),
            "reason 侧覆盖 {variant}"
        );
        assert!(check_source.contains(needle), "check 侧文案含「{needle}」");
        assert!(
            reason_source.contains(needle),
            "reason 侧文案含「{needle}」"
        );
    }
}

// ===========================================================================
// 出口面（crate 根）：五名在册 / 退役名零出线 / 可达性（`lib.rs` 文件级被测面）
// ===========================================================================

/// 正向（AC-7 / AC-15 出口纪律）：`lib.rs` 的出口面按**既有形态**扩面——五名
/// （`BootstrapDrafts` / `validate_schema` / `schema_fqn` / `is_fqn` / `is_identifier`）
/// 在 crate 根 `pub use` 面各恰一次（分组块成员或 `pub use api::<模块>::…` 的既有逐名形态，
/// 零通配、零重复）；`pub use api::display` 单行与 `pub use api::{…}` 分组块形态零改。
#[test]
fn 出口面按分组块扩面() {
    /// 摘出文件内全部 `pub use …;` 声明（跨行，逐条一行）。
    fn reexport_face(source: &str) -> String {
        let mut face = String::new();
        let mut rest = source;
        while let Some(start) = rest.find("pub use ") {
            let tail = &rest[start..];
            let end = tail.find(';').expect("`pub use` 声明以 `;` 收口");
            face.push_str(&tail[..end]);
            face.push('\n');
            rest = &tail[end..];
        }
        face
    }

    /// 标识符逐词匹配（避开 `is_fqn` 命中 `IsFqnHelper` 一类子串）。
    fn token_count(face: &str, name: &str) -> usize {
        face.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|token| *token == name)
            .count()
    }

    let source = include_str!("../lib.rs");
    let face = reexport_face(source);
    for name in [
        "BootstrapDrafts",
        "validate_schema",
        "schema_fqn",
        "is_fqn",
        "is_identifier",
    ] {
        assert_eq!(
            token_count(&face, name),
            1,
            "`{name}` 在 crate 根 `pub use` 面恰一次：{face}"
        );
    }
    assert_eq!(
        source
            .lines()
            .filter(|line| *line == "pub use api::display;")
            .count(),
        1,
        "跨 crate 单点 `pub use api::display` 单行恰一次"
    );
    assert!(
        face.contains("pub use api::{"),
        "`pub use api::{{…}}` 分组块形态保留"
    );
    assert!(!face.contains("pub use api::*"), "逐名出口纪律（零通配）");
}

/// 异常（退役名零出线）：`DocFact` / `carrier_path` / `doc_path` / `build_schema` /
/// `ContractSection` 在 `lib.rs` 零命中（零 `pub use`、零分组块成员）。
#[test]
fn 退役名不出线() {
    let source = include_str!("../lib.rs");
    for retired in [
        "DocFact",
        "carrier_path",
        "doc_path",
        "build_schema",
        "ContractSection",
    ] {
        assert_eq!(
            source.matches(retired).count(),
            0,
            "退役名 `{retired}` 不出线（`lib.rs` 零命中）"
        );
    }
}

/// 正向（可达性，编译期即判）：五名可经 crate 根直接调用——零测试专用导出。
#[test]
fn 五名可经crate根调用() {
    let drafts = crate::BootstrapDrafts {
        schema: String::new(),
        doc: String::new(),
    };
    assert!(drafts.schema.is_empty() && drafts.doc.is_empty());
    let text = mirrored_schema_text("App.Core.Export", "", "");
    crate::validate_schema("App.Core.Export", &text).expect("空壳应过九闸");
    assert_eq!(
        crate::schema_fqn(std::path::Path::new("App/Core/Export.schema.json")),
        Some("App.Core.Export".to_owned())
    );
    assert!(crate::is_fqn("App.Core.Export"));
    assert!(crate::is_identifier("file_stem"));
}

/// 边界（源码锚 · 三路径派生单点）：`ref_projection` 体内三条路径各取一次派生单点
/// （`paths::contract_path(` / `schema_path(` / `generated_path(`），零手拼
/// `format!("<叶段>.md")`；`api_ref` 与 `api_ref_of` 同源（两入口都委托
/// `ref_projection`，装配面恰一处）。
#[test]
fn 三路径派生单点() {
    /// 取某函数体（从签名所在行到首个 `\n}\n`）。
    fn body_of<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("缺少函数签名 `{signature}`"));
        let rest = &source[start..];
        let end = rest.find("\n}\n").map_or(rest.len(), |index| index + 1);
        &rest[..end]
    }

    let source = include_str!("mod.rs");
    let projection = body_of(source, "fn ref_projection(");
    for derive in [
        "paths::contract_path(",
        "paths::schema_path(",
        "paths::generated_path(",
    ] {
        assert_eq!(
            projection.matches(derive).count(),
            1,
            "`{derive}` 在 `ref_projection` 内恰一处"
        );
    }
    assert!(
        !projection.contains(".md\"") && !projection.contains(".schema.json\""),
        "零手拼落点名（落点派生单点在 `paths`）"
    );

    // 两入口同源：`api_ref` 委托 `api_ref_of`，后者与前者共用 `ref_projection`
    assert!(
        body_of(source, "pub fn api_ref(").contains("api_ref_of("),
        "`api_ref` 委托 `api_ref_of`"
    );
    assert_eq!(
        source.matches("ref_projection(").count(),
        3,
        "装配面单点：定义 1 + 调用 2（`api_ref` 的缺席臂 + `api_ref_of`）"
    );
}

/// 异常（生成器让位）：即使 `snapshot.schemas` 有值（含坏形态的键），`render_all` 的输出集
/// 零 `.schema.json`——手写真相永不进应生成集；源码锚：`render_all` 体内零
/// `SCHEMA_FILE_SUFFIX` / `.schema.json` 字面。
#[test]
fn schema不再由生成器产出() {
    fn body_of<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("缺少函数签名 `{signature}`"));
        let rest = &source[start..];
        let end = rest.find("\n}\n").map_or(rest.len(), |index| index + 1);
        &rest[..end]
    }

    let fqn = "App.Backend.Infra.ArchiRuntime";
    let temp = TempDir::new("render-no-schema");
    temp.write_model(&format!(
        "[element.\"App\"]\nkind = \"package\"\n\n[element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"crates/infra/archi/\"]\n"
    ));
    temp.write_pair(fqn);
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let elements = [element(fqn, &["crates/infra/archi/"])];
    let snapshot = load_contracts(temp.root(), &elements).expect("装载应成功");
    assert!(
        !snapshot.schemas.is_empty(),
        "夹具的 schema 已装配在案（生成器仍不读它）"
    );
    let files = render_all(temp.root(), &snapshot).expect("渲染应成功");
    assert!(!files.is_empty(), "三类生成物仍产出");
    for file in &files {
        let path = file.path.to_string_lossy().replace('\\', "/");
        assert!(
            !path.ends_with(".schema.json"),
            "应生成集零 `.schema.json`：{path}"
        );
    }

    let source = include_str!("mod.rs");
    let render = body_of(source, "pub fn render_all(");
    assert!(
        !render.contains("SCHEMA_FILE_SUFFIX"),
        "`render_all` 不读 `SCHEMA_FILE_SUFFIX`（手写真相不在生成面）"
    );
    assert!(
        !render.contains(".schema.json"),
        "`render_all` 内零 `.schema.json` 字面"
    );
}
