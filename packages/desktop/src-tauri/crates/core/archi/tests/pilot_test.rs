//! 契约资产 ↔ 生成物 ↔ 手写实现的三方对拍（crate 级集成测试）。
//!
//! 单测里 `contract::parse` / `schema::build_schema` / `emit::*` 的夹具都是**手写源文**，
//! 只能证明「给定源文时函数行为正确」；本文件补的是**在盘事实**：试点元素
//! `App.Backend.Infra.ArchiRuntime` 的 `.md` / `.schema.json` / `.i.rs` / `INDEX.toml`
//! 与手写 `impl` 是否逐项对齐（AC-13 / AC-2 / AC-10 / AC-17）。
//!
//! 零 mock：真实读盘（仓库内资产）+ 真实 `parse` / `build_schema` / `emit::rust::render`
//! 重算——「生成物 = 派生物」只能由「重算后逐字节相等」表达，替身会让它不可见。
//! 契约目录只读，零写盘。

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use archi::api::contract::{is_constructor, parse, Contract};
use archi::api::emit::rust::render;
use archi::api::emit::{EmitTarget, ModuleRef};
use archi::api::paths::{
    self, contract_path, directory, generated_path, index_path, EntryForm, GeneratedKind,
};
use archi::api::schema::{build_index, build_schema, ApiFormStatus, IndexEntry};
use archi::api::{api_ref, api_ref_of, meta};
use archi::dto::{ApiStatus, ArchiApiRef};
use foundation::layout::archi_api_dir;
use serde_json::Value;

/// 试点元素 FQN。
const FQN: &str = "App.Backend.Infra.ArchiRuntime";

/// 试点元素根（`path[0]` 的归一形态）。
const ELEMENT_ROOT: &str = "packages/desktop/src-tauri/crates/infra/archi";

/// 仓库根（`CARGO_MANIFEST_DIR` 位于 `<仓根>/packages/desktop/src-tauri/crates/core/archi`，
/// 故上溯**六层**：`archi` → `core` → `crates` → `src-tauri` → `desktop` → `packages` → 仓根）。
fn repository_root() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..6 {
        root.pop();
    }
    root
}

/// 试点契约的 repo 相对路径串。
fn contract_rel() -> String {
    format!(".dev-team/architecture/api/Infra/{FQN}.md")
}

/// 试点 schema 生成物的 repo 相对路径串。
fn schema_rel() -> String {
    format!(".dev-team/architecture/api/Infra/{FQN}.schema.json")
}

/// 试点 `.i.rs` 生成物的 repo 相对路径串。
fn generated_rel() -> String {
    format!("{ELEMENT_ROOT}.i.rs")
}

/// 读仓库内文件（缺失即失败并点名绝对路径）。
fn read_repo(relative: &str) -> String {
    let path = repository_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("应可读 {}：{error}", path.display()))
}

/// 解析试点契约（真实源文 + 真实解析器）。
fn pilot_contract() -> Contract {
    let source = read_repo(&contract_rel());
    parse(&source, FQN, &contract_rel()).expect("试点契约应解析通过")
}

/// 试点 schema（真实装配），并断言与盘上生成物逐字节相等（「纯派生」的事实判据）。
fn pilot_schema() -> Value {
    let contract = pilot_contract();
    let schema = build_schema(&contract, &meta()).expect("试点 schema 应装配成功");
    let mut expected = serde_json::to_string_pretty(&schema).expect("schema 应可序列化");
    expected.push('\n');
    assert_eq!(
        expected,
        read_repo(&schema_rel()),
        "真实源文重算的 schema 与盘上生成物逐字节相等（纯派生，MUST NOT 手改）"
    );
    schema
}

/// 仓库 `.dev-team/architecture/api` 目录。
fn api_dir() -> PathBuf {
    archi_api_dir(&repository_root())
}

// ---------------------------------------------------------------------------
// 三方在场：md / schema.json / .i.rs / INDEX.toml
// ---------------------------------------------------------------------------

/// 正向（AC-13）：三件资产按 `paths` 单点派生的落点在盘——契约在 `<Domain>/` 下、schema
/// 与它同目录、`.i.rs` 在元素根同级；索引在 api 根（比契约少一段）。
#[test]
fn 试点三件资产按单点派生落点在盘() {
    let root = repository_root();
    let contract = contract_path(&root, FQN).expect("合法 FQN 应可派生");
    let schema = paths::schema_path(&root, FQN).expect("合法 FQN 应可派生");
    let generated =
        generated_path(&root, ELEMENT_ROOT, GeneratedKind::Rust).expect("<root> 形态合法");

    for path in [&contract, &schema, &generated] {
        assert!(path.is_file(), "资产在盘：{}", path.display());
    }
    assert_eq!(
        contract.parent(),
        schema.parent(),
        "契约与 schema 恒同目录（同一个 `<Domain>/`）"
    );
    assert_eq!(
        contract,
        api_dir().join("Infra").join(format!("{FQN}.md")),
        "契约路径 = `<archi_api_dir>/<Domain>/<FQN>.md`（零行内 `api` 段）"
    );
    assert_eq!(
        generated,
        root.join(format!("{ELEMENT_ROOT}.i.rs")),
        "生成物 = `<根同级>.i.rs`（不在元素目录之内）"
    );
    assert_eq!(
        generated.parent(),
        root.join(ELEMENT_ROOT).parent(),
        "生成物与元素根同级"
    );
    assert!(index_path(&root).is_file(), "`INDEX.toml` 全局唯一一份在盘");
    assert_eq!(
        index_path(&root),
        directory(&root).join("INDEX.toml"),
        "索引 = api 目录 + `INDEX.toml`"
    );
    assert_ne!(
        index_path(&root).parent(),
        contract.parent(),
        "索引在 api 根、契约在域目录（两层不混居）"
    );
}

/// 正向（AC-2）：盘上 schema 的六字段与真实源文重算结果逐字节相等，逐字段给出 md 出处
/// （`$schema` / `$id` / `title` / `$defs` / `properties` / `$comment`），头部带生成警示。
#[test]
fn 盘上schema是真实源文的纯派生() {
    let schema = pilot_schema();
    let object = schema.as_object().expect("schema 恒为 JSON 对象");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["$comment", "$defs", "$id", "$schema", "properties", "title"],
        "六字段齐备且无第七键"
    );
    assert_eq!(
        schema["$schema"].as_str(),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "`$schema` 取载元常量（draft 2020-12）"
    );
    assert_eq!(
        schema["title"].as_str(),
        Some(FQN),
        "`title` 取 H1 原文（与文件名同源）"
    );
    assert!(
        schema["$id"]
            .as_str()
            .is_some_and(|id| id == format!("{FQN}.schema.json")),
        "`$id` = `<FQN>.schema.json`（不合成 URI）"
    );
    let comment = schema["$comment"].as_str().unwrap_or_default();
    assert!(
        comment.contains("勿手改") && comment.contains("api:check"),
        "`$comment` 含生成警示（勿手改 / api:check）：{comment}"
    );
    let defs = schema["$defs"].as_object().expect("`$defs` 恒为对象");
    assert!(
        defs.contains_key("LocalArchiCheck"),
        "试点自有类型在 `$defs`：{:?}",
        defs.keys().collect::<Vec<_>>()
    );
    let properties = schema["properties"]
        .as_object()
        .expect("`properties` 恒为对象");
    assert!(
        properties.contains_key("file_stem"),
        "试点操作进 `properties`：{:?}",
        properties.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        properties["LocalArchiCheck"]["$ref"].as_str(),
        Some("#/$defs/LocalArchiCheck"),
        "类型位的 `properties` 值是纯 `$ref`（无结构关键字）"
    );
    assert!(
        read_repo(&schema_rel()).ends_with("}\n"),
        "生成物以单个换行收尾"
    );
}

// ---------------------------------------------------------------------------
// `.i.rs`：纯派生 + 签名见证 + 零透传出口
// ---------------------------------------------------------------------------

/// 正向（AC-13 / D29）：盘上 `.i.rs` 与「真实源文 + 真实渲染器」重算结果逐字节相等；头注释
/// 带工具版本与 `common.json` 摘要、交代 impl 分离；两行签名见证在案；试点零透传 ⇒ 生成物
/// 零 `pub use`（被检文件是**生成物**，与 `runner_test` 对 `runner.rs` 的零再导出断言分列
/// 两条判据，互不替代）。
#[test]
fn 盘上生成物是纯派生且见证在本crate() {
    let contract = pilot_contract();
    let schema = pilot_schema();
    let target = EmitTarget {
        kind: GeneratedKind::Rust,
        form: EntryForm::Crate,
        modules: BTreeMap::<String, ModuleRef>::new(),
    };
    let rendered = render(&contract, &schema, &meta(), &target);
    let on_disk = read_repo(&generated_rel());
    assert_eq!(
        rendered, on_disk,
        "盘上 `.i.rs` 与真实源文重算逐字节相等（MUST NOT 手改）"
    );

    let meta = meta();
    assert!(
        on_disk.contains(&format!("archi-api {}", meta.tool_version)),
        "头注释带工具版本（`CARGO_PKG_VERSION`）"
    );
    assert!(
        on_disk.contains("common.json@sha256:"),
        "头注释带映射规则摘要短形（`sha256:<12 位>`）"
    );
    assert!(
        on_disk.contains("impl 块写在本模块任意手写文件"),
        "头注释交代 impl 分离（生成物的手写面在哪）"
    );
    assert!(
        on_disk.contains("pub struct LocalArchiCheck;"),
        "试点类型由生成物提供（手写侧零第二份定义）"
    );
    assert!(
        on_disk.contains("pub fn new() -> Self"),
        "零字段 `object` 的自动构造器在案"
    );
    assert!(
        on_disk.contains("const _: fn(&str) -> String = super::LocalArchiCheck::file_stem;"),
        "签名见证走 `super::<Owner>::<op>` 形态（裸名会报 E0425）"
    );
    assert!(
        on_disk.contains("const _: fn() -> super::LocalArchiCheck = super::LocalArchiCheck::new;"),
        "构造器见证在案"
    );
    assert!(
        !on_disk.contains("pub use "),
        "试点契约零 `api-reexport` ⇒ 生成物零透传出口"
    );
    assert!(
        !on_disk.contains("pub struct LocalArchiCheck {"),
        "零字段类型不产带字段结构体（`pub struct LocalArchiCheck;` 形态）"
    );
}

// ---------------------------------------------------------------------------
// 手写实现面：无 `### new`、`impl Default` 共存、字面量 `#[path]` 编排
// ---------------------------------------------------------------------------

/// 正向（AC-13 / D23 / D38）：契约零构造器 op（`### new` 不存在）；生成物与手写
/// `impl Default` 共存——入口以**字面量** `#[path]` 把生成物编进本 crate，`mod_path_attr`
/// 是那份字面量的等值真值源；手写侧零第二份 `pub struct`（否则 E0255）、零再导出（否定
/// 断言锚**全串 `pub use`**，不写成只在特定排版下成立的复合串）。
#[test]
fn 契约零构造器且生成物与手写impl共存() {
    let contract = pilot_contract();
    let ops: Vec<&str> = contract.ops.iter().map(|op| op.id.as_str()).collect();
    assert_eq!(ops, vec!["file_stem"], "试点只有 `file_stem` 一个操作");
    assert!(
        !read_repo(&contract_rel()).contains("### new"),
        "`### new` 不存在（构造器由生成物产出，契约 MUST NOT 声明）"
    );
    for op in &contract.ops {
        assert!(
            !is_constructor(&op.id),
            "试点 op `{}` 不在构造器封闭集内",
            op.id
        );
    }

    let lib = read_repo("packages/desktop/src-tauri/crates/infra/archi/src/lib.rs");
    let attribute = lib
        .lines()
        .find_map(|line| line.trim().strip_prefix("#[path = \""))
        .and_then(|tail| tail.strip_suffix("\"]"))
        .expect("入口行含 `#[path = \"…\"]` 字面量");
    assert_eq!(
        attribute,
        paths::mod_path_attr(EntryForm::Crate, "archi"),
        "`#[path]` 字面量与 `paths::mod_path_attr` 输出逐字相等（D38 等值真值源）"
    );
    assert!(
        lib.contains("pub use api_gen::*;"),
        "`LocalArchiCheck` 由生成物经 crate 根单点再导出"
    );

    let runner = read_repo("packages/desktop/src-tauri/crates/infra/archi/src/runner.rs");
    assert!(
        !runner.contains("pub struct LocalArchiCheck"),
        "手写侧零 `pub struct` 定义（否则与生成物撞名，E0255）"
    );
    assert!(
        runner.contains("impl Default for LocalArchiCheck"),
        "手写 `impl Default` 保留"
    );
    assert!(
        runner.contains("Self::new()"),
        "`Default` 委托生成物提供的构造器（零第二份构造逻辑）"
    );
    assert!(
        runner.contains("pub fn file_stem"),
        "`file_stem` 迁入 `impl` 并提升为 `pub`（可见性提升是唯一代码形状改动）"
    );
    assert!(
        !runner.contains("pub use"),
        "`runner.rs` 零再导出（`LocalArchiCheck` 的唯一出口在 crate 根）"
    );
    assert!(
        runner.contains("fn compose_text") && !runner.contains("pub fn compose_text"),
        "`compose_text` 保持私有、不进契约"
    );
    let report = read_repo("packages/desktop/src-tauri/crates/infra/archi/src/report.rs");
    assert!(
        report.contains("LocalArchiCheck::file_stem"),
        "落盘调用点经 `LocalArchiCheck::file_stem`"
    );
    assert!(
        !report.contains("fn file_stem"),
        "`report.rs` 内零 `fn file_stem`（实现体迁往 `runner.rs`，非复制）"
    );
}

// ---------------------------------------------------------------------------
// INDEX.toml：条目、语言无关纪律与索引的纯投影
// ---------------------------------------------------------------------------

/// 正向（AC-2 / AC-17）：`INDEX.toml` 文本零语言词（`crate` / `Cargo.toml` / `Rust` /
/// `TypeScript`）、说明段在场、条目计数与真实源文同源；`[entry.categories]` 是 `$defs`
/// 形状分布（`sum == types`），`absent` 条目整段省略。
#[test]
fn 索引零语言词且计数与真实源文同源() {
    let contract = pilot_contract();
    let schema = pilot_schema();
    let defs = schema["$defs"].as_object().expect("`$defs` 恒为对象");
    let declared = IndexEntry {
        fqn: FQN.to_owned(),
        status: ApiFormStatus::Declared,
        types: defs.len(),
        ops: contract.ops.len(),
        degraded: defs
            .values()
            .filter(|definition| definition.get("x-common").is_some())
            .count(),
        categories: BTreeMap::from([("object".to_owned(), defs.len())]),
        schema_rel: schema_rel(),
        generated_rel: Some(generated_rel()),
    };
    let absent = IndexEntry {
        fqn: "App.Backend.Infra.Absent".to_owned(),
        status: ApiFormStatus::Absent,
        types: 0,
        ops: 0,
        degraded: 0,
        categories: BTreeMap::new(),
        schema_rel: String::new(),
        generated_rel: None,
    };
    let text = build_index(&[declared, absent], &meta());

    // `INDEX.toml` 的注释头部挂在文档 decor 上，`toml::Value` 直解会报「unexpected content」，
    // 故沿用生产侧的 `toml_edit` 解析面（文档模型才是该文本的合法读面）
    let parsed: toml_edit::DocumentMut = text.parse().expect("`INDEX.toml` 文本恒可解析");
    let entries: Vec<&toml_edit::Table> = parsed["entry"]
        .as_array_of_tables()
        .expect("数组表 `[[entry]]`")
        .iter()
        .collect();
    assert_eq!(entries.len(), 2, "纯投影：入参两条即两条（不多不少）");
    // 条目按 FQN 字典序（函数内再排序一次，调用方序不影响结果）
    let first = entries[1];
    let absent_entry = entries[0];
    assert_eq!(first["fqn"].as_str(), Some(FQN));
    assert_eq!(
        first["status"].as_str(),
        Some("declared"),
        "契约在案即 declared"
    );
    assert_eq!(
        first["types"].as_integer(),
        Some(defs.len() as i64),
        "types = `$defs` 条目数（与真实源文同源）"
    );
    assert_eq!(
        first["ops"].as_integer(),
        Some(contract.ops.len() as i64),
        "ops = 操作数"
    );
    assert_eq!(
        first["degraded"].as_integer(),
        Some(1),
        "`degraded` = `$defs` 条目里带 `x-common` 的个数（试点类型的 `x-common.note` 计数）"
    );
    assert_eq!(first["schema"].as_str(), Some(schema_rel().as_str()));
    assert_eq!(first["generated"].as_str(), Some(generated_rel().as_str()));
    let categories = first["categories"]
        .as_table()
        .expect("declared 条目带 `[entry.categories]` 子表");
    assert_eq!(
        categories["object"].as_integer(),
        Some(defs.len() as i64),
        "形状分布：试点类型 `type` 为 object ⇒ `object` 计数"
    );
    assert_eq!(categories.len(), 1, "试点只有一种形状（`sum == types`）");
    assert_eq!(
        absent_entry["status"].as_str(),
        Some("absent"),
        "文件缺失即 absent"
    );
    assert!(
        absent_entry.get("categories").is_none(),
        "`absent` 条目的 `categories` 整段省略（MUST NOT 输出空表）"
    );
    assert_eq!(
        absent_entry["schema"].as_str(),
        Some(""),
        "absent 的 schema 为空串"
    );
    assert_eq!(
        absent_entry["generated"].as_str(),
        Some(""),
        "absent 的 generated 为空串"
    );

    // 语言词禁令：注释头部与条目值都不得出现语言特定词。`crate` 按**词边界**判——
    // 生成物路径里恰好含 `crates/` 目录段（仓库路径，不是语言词），故以空白 / 标点 /
    // 引号边界识别「独立成词」的形态。
    for forbidden in ["Cargo.toml", "Rust", "TypeScript"] {
        assert!(
            !text.contains(forbidden),
            "`INDEX.toml` 零语言词（命中 `{forbidden}`）：\n{text}"
        );
    }
    for (index, _) in text.match_indices("crate") {
        let before = text[..index].chars().next_back();
        let after = text[index + 5..].chars().next();
        let boundary =
            |character: Option<char>| character.is_none_or(|value| !value.is_ascii_alphanumeric());
        assert!(
            !(boundary(before) && boundary(after)),
            "`INDEX.toml` 零语言词（`crate` 独立成词，位置 {index}）：\n{text}"
        );
    }
    assert!(
        text.lines().next().unwrap_or_default().starts_with('#'),
        "首行是说明段注释"
    );
    assert!(
        text.contains("<FQN>.md"),
        "说明段给出 review 口径（只读 `<FQN>.md`）"
    );
    assert!(
        text.contains(&format!("archi-api {}", meta().tool_version)),
        "说明段带生成器标识"
    );
}

// ---------------------------------------------------------------------------
// 投影面：`archi_module` 的 `api` 字段与文件事实一致
// ---------------------------------------------------------------------------

/// 正向（AC-10 / AC-13）：元素形态投影对试点元素给 `declared` + 三路径，三条路径逐字等于
/// `paths` 单点派生值（repo 相对 POSIX）；「按 FQN 自取 path」的形态与之逐字段相等
/// （投影两形态同源）。
#[test]
fn 投影三路径与单点派生逐字相等() {
    let root = repository_root();
    let element_path = vec![format!("./{ELEMENT_ROOT}/")];
    let projection: ArchiApiRef = api_ref_of(&root, FQN, &element_path);
    assert_eq!(
        projection.status,
        ApiStatus::Declared,
        "契约在案即 `declared`"
    );
    assert_eq!(
        projection.contract_path.as_deref(),
        Some(contract_rel().as_str()),
        "契约路径 = `paths::contract_path` 的 repo 相对串"
    );
    assert_eq!(
        projection.schema_path.as_deref(),
        Some(schema_rel().as_str()),
        "schema 路径同目录同名（记录应生成物）"
    );
    assert_eq!(
        projection.generated_path.as_deref(),
        Some(generated_rel().as_str()),
        "生成物路径按元素根派生（`./` 前缀与尾 `/` 归一后逐字相等）"
    );

    // 「按 FQN 自取 path」形态：仓库模型里试点元素的 `path[0]` 指向同一根
    let by_fqn = api_ref(&root, FQN);
    assert_eq!(
        by_fqn, projection,
        "两形态同源：模型内 `path[0]` 与夹具根指向同一元素根，故投影逐字段相等"
    );
    assert!(
        projection
            .contract_path
            .as_deref()
            .is_some_and(|path| path.starts_with(".dev-team/architecture/api/")),
        "路径串是 repo 相对 POSIX（零反斜杠、零前导 `./`）"
    );
}

/// 边界（AC-10 / D22）：容器元素（`path` 空）仍有契约可枚举 → `contractPath` /
/// `schemaPath` 给出、`generatedPath` 为 `None`；未声明的 FQN → `none` + 三路径全 `None`
/// （投影只报文件事实，零回写模型字段）。
#[test]
fn 投影对容器元素与未声明fqn可辨() {
    let root = repository_root();
    let container = api_ref_of(&root, FQN, &[]);
    assert_eq!(
        container.status,
        ApiStatus::Declared,
        "md 在案即可枚举契约（与元素是否容器无关）"
    );
    assert_eq!(
        container.contract_path.as_deref(),
        Some(contract_rel().as_str())
    );
    assert_eq!(
        container.schema_path.as_deref(),
        Some(schema_rel().as_str())
    );
    assert_eq!(container.generated_path, None, "容器元素天然无生成物");
    assert_eq!(
        container,
        api_ref_of(&root, FQN, &[String::new()]),
        "空串元素根与空 `path` 同形（都无生成物）"
    );

    let undeclared = api_ref_of(
        &root,
        "App.Backend.Infra.NotExists",
        &[ELEMENT_ROOT.to_owned()],
    );
    assert_eq!(undeclared.status, ApiStatus::None, "md 缺席即 none");
    assert_eq!(undeclared.contract_path, None);
    assert_eq!(undeclared.schema_path, None);
    assert_eq!(undeclared.generated_path, None);
}
