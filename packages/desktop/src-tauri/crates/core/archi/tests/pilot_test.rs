//! 契约资产 ↔ 生成物 ↔ 手写实现的三方对拍（crate 级集成测试）。
//!
//! 单测里 `validate_schema` / `emit::*` 的夹具都是**手写源文**，只能证明「给定源文时
//! 函数行为正确」；本文件补的是**在盘事实**：试点元素 `Backend.Infra.ArchiRuntime` 的
//! `.md` / `.schema.json`（手写接口真相）/ `.i.rs`（生成物）/ `INDEX.toml`（生成物）与手写
//! `impl` 是否逐项对齐（AC-13 / AC-2 / AC-10 / AC-17）。
//!
//! 零 mock：真实读盘（仓库内资产）+ 真实 `validate_schema` / `emit::rust::render` 重算——
//! 「生成物 = 派生物」只能由「重算后逐字节相等」表达，替身会让它不可见。
//! 契约目录只读，零写盘。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use archi::api::contract::{is_constructor, Contract};
use archi::api::emit::rust::render;
use archi::api::emit::{EmitTarget, ModuleRef};
use archi::api::paths::{
    self, contract_path, directory, generated_path, index_path, EntryForm, GeneratedKind,
};
use archi::api::schema::{build_index, ApiFormStatus, IndexEntry};
use archi::api::{api_ref, api_ref_of, display, meta};
use archi::dto::{ApiStatus, ArchiApiRef};
use archi::validate_schema;
use foundation::layout::archi_models_dir;
use serde_json::Value;

/// 试点元素 FQN。
const FQN: &str = "Backend.Infra.ArchiRuntime";

/// 试点元素根（`path[0]` 的归一形态）。
const ELEMENT_ROOT: &str = "packages/desktop/src-tauri/crates/infra/archi";

/// 试点元素的实现入口目录（`<root>/src`）。
const ENTRY_DIR: &str = "packages/desktop/src-tauri/crates/infra/archi/src";

/// 试点元素的实现入口（约定默认 `<root>/src/lib.rs` 命中，故模型不声明 `entry`）。
const ENTRY_RS: &str = "packages/desktop/src-tauri/crates/infra/archi/src/lib.rs";

/// 仓库根（`CARGO_MANIFEST_DIR` 位于 `<仓根>/packages/desktop/src-tauri/crates/core/archi`，
/// 故上溯**六层**：`archi` → `core` → `crates` → `src-tauri` → `desktop` → `packages` → 仓根）。
fn repository_root() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..6 {
        root.pop();
    }
    root
}

/// 元素投影构造器（投影面测试只关心 `fqn` 与 `path`，其余取与真实模型同形的默认值）。
fn pilot_element(fqn: &str, paths: &[&str]) -> archi::dto::ArchiElement {
    archi::dto::ArchiElement {
        fqn: fqn.to_owned(),
        kind: archi::dto::ElementKind::Module,
        gate: archi::dto::GateTier::Warn,
        api: ApiStatus::Declared,
        entry: None,
        path: paths.iter().map(|path| (*path).to_owned()).collect(),
        summary: None,
        metadata: BTreeMap::new(),
        model_file: ".dev-team/architecture/graph/model/03-backend-infra.toml".to_owned(),
        model_line: 1,
        mark: archi::dto::NodeMark::None,
        doc: None,
    }
}

/// 试点契约的 repo 相对路径串（文件名取 FQN 叶段）。
fn contract_rel() -> String {
    format!(
        ".dev-team/architecture/models/Backend/Infra/{}.md",
        paths::leaf_name(FQN)
    )
}

/// 试点 schema 生成物的 repo 相对路径串（与文档同目录、同叶段主干）。
fn schema_rel() -> String {
    format!(
        ".dev-team/architecture/models/Backend/Infra/{}.schema.json",
        paths::leaf_name(FQN)
    )
}

/// 试点 `.i.rs` 生成物的 repo 相对路径串（落**实现入口同级**）。
fn generated_rel() -> String {
    format!("{ENTRY_DIR}/lib.i.rs")
}

/// 读仓库内文件（缺失即失败并点名绝对路径）。
fn read_repo(relative: &str) -> String {
    let path = repository_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("应可读 {}：{error}", path.display()))
}

/// 解析试点契约（真实源文 + 真实解析器）。
fn pilot_contract() -> Contract {
    let source = read_repo(&schema_rel());
    validate_schema(FQN, &source).expect("试点接口定义应通过校验")
}

/// 试点接口定义的 JSON 值（手写真相，读盘解析；零「重算比对」——schema 不再是生成物）。
fn pilot_schema() -> Value {
    let source = read_repo(&schema_rel());
    serde_json::from_str(&source).expect("试点接口定义应是合法 JSON")
}

/// 仓库 `.dev-team/architecture/models` 目录。
fn api_dir() -> PathBuf {
    archi_models_dir(&repository_root())
}

// ---------------------------------------------------------------------------
// 三方在场：md / schema.json / .i.rs / INDEX.toml
// ---------------------------------------------------------------------------

/// 正向（AC-13）：三件资产按 `paths` 单点派生的落点在盘——契约在分层目录段
/// （`Backend/Infra/`）下、schema 与它同目录、`.i.rs` 在元素根同级；索引在独立的
/// `graph/` 根（与契约目录分家）。
#[test]
fn 试点三件资产按单点派生落点在盘() {
    let root = repository_root();
    let contract = contract_path(&root, FQN).expect("合法 FQN 应可派生");
    let schema = paths::schema_path(&root, FQN).expect("合法 FQN 应可派生");
    let generated = generated_path(&root, ENTRY_RS, GeneratedKind::Rust).expect("入口形态合法");

    for path in [&contract, &schema, &generated] {
        assert!(path.is_file(), "资产在盘：{}", path.display());
    }
    assert_eq!(
        contract.parent(),
        schema.parent(),
        "契约与 schema 恒同目录（同一段分层目录）"
    );
    assert_eq!(
        contract,
        api_dir()
            .join("Backend")
            .join("Infra")
            .join(format!("{}.md", paths::leaf_name(FQN))),
        "契约路径 = `<archi_models_dir>/<分层目录…>/<叶段>.md`（零行内 `api` 段）"
    );
    assert_eq!(
        generated,
        root.join(format!("{ENTRY_DIR}/lib.i.rs")),
        "生成物 = `<入口主干>.i.rs`（与实现入口同级）"
    );
    assert_eq!(
        generated.parent(),
        Some(root.join(ENTRY_DIR).as_path()),
        "生成物与实现入口同目录"
    );
    // 迁移双向：旧落点（元素根同级的兄弟文件）已删，新落点（入口同级）在盘
    let legacy = root.join(format!("{ELEMENT_ROOT}.i.rs"));
    assert!(
        !legacy.is_file(),
        "旧落点已删：{}（`api:check` 的 orphaned 列由此为空）",
        legacy.display()
    );
    assert!(index_path(&root).is_file(), "`INDEX.toml` 全局唯一一份在盘");
    let index = index_path(&root);
    assert_eq!(
        index.file_name().and_then(|name| name.to_str()),
        Some("INDEX.toml"),
        "索引文件名恒 `INDEX.toml`"
    );
    assert_eq!(
        index.parent(),
        Some(foundation::layout::archi_graph_dir(&root).as_path()),
        "索引家 = `<archi_graph_dir>`（与契约目录分家）"
    );
    assert!(
        !index.starts_with(directory(&root)),
        "索引 MUST NOT 落在契约目录之内：{}",
        index.display()
    );
    assert_ne!(
        index.parent(),
        contract.parent(),
        "索引在 graph 根、契约在域目录（两层不混居）"
    );
}

/// 异常（AC-1）：旧契约目录缺席——`<archi_dir>/api` 目录不存在。旧位置缺席的机械面
/// 只能由目录读数 / 负向断言守（孤儿枚举严格两处派生，`api/` 残留对 `api:check`
/// 不可见）。
#[test]
fn 旧契约目录已退役() {
    let legacy = foundation::layout::archi_dir(&repository_root()).join("api");
    assert!(
        !legacy.exists(),
        "旧契约目录 MUST NOT 残留：{}",
        legacy.display()
    );
}

/// 正向（AC-2）：盘上 `.schema.json` 是**手写接口真相**——六字段齐备（`$schema` / `$id` /
/// `title` / `$defs` / `properties` / `$comment`），逐字段给出语义锚点（「纯派生」一侧的
/// 重算比对已随真相方向倒置退役）。
#[test]
fn 盘上schema是手写接口真相() {
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
        "`title` 取 FQN 原文（与 `$id` 同源；文件名只取叶段）"
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
/// `impl Default` 共存——入口以**字面量** `#[path]` 把生成物编进本 crate；该字面量的等值
/// 真值源是 `paths::generated_path(...)` 的**文件名**（入口同级后恒无目录段）；手写侧零
/// 第二份 `pub struct`（否则 E0255）、零再导出（否定断言锚**全串 `pub use`**，不写成只在
/// 特定排版下成立的复合串）。
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
    let expected_attribute = generated_path(&repository_root(), ENTRY_RS, GeneratedKind::Rust)
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .expect("生成物路径可派生");
    assert_eq!(
        attribute, expected_attribute,
        "`#[path]` 字面量与 `paths::generated_path` 的文件名逐字相等（入口同级后恒为文件名）"
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
        fqn: "Backend.Infra.Absent".to_owned(),
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
        text.contains("<叶段>.md"),
        "说明段给出 review 口径（只读 `<叶段>.md`）"
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
    let element = pilot_element(FQN, &[ELEMENT_ROOT]);
    let projection: ArchiApiRef = api_ref_of(&root, std::slice::from_ref(&element), &element);
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
        "schema 路径同目录同叶段主干（接口文件与文档成对）"
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
            .is_some_and(|path| path.starts_with(".dev-team/architecture/models/")),
        "路径串是 repo 相对 POSIX（零反斜杠、零前导 `./`）"
    );
}

/// 边界（AC-10 / D22）：容器元素（`path` 空）仍有契约可枚举 → `contractPath` /
/// `schemaPath` 给出、`generatedPath` 为 `None`；未声明的 FQN → `none` + 三路径全 `None`
/// （投影只报文件事实，零回写模型字段）。
#[test]
fn 投影对容器元素与未声明fqn可辨() {
    let root = repository_root();
    let container_element = pilot_element(FQN, &[]);
    let container = api_ref_of(
        &root,
        std::slice::from_ref(&container_element),
        &container_element,
    );
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
    let empty_root = pilot_element(FQN, &[""]);
    assert_eq!(
        container,
        api_ref_of(&root, std::slice::from_ref(&empty_root), &empty_root),
        "空串元素根与空 `path` 同形（都无生成物）"
    );

    let undeclared_element = pilot_element("Backend.Infra.NotExists", &[ELEMENT_ROOT]);
    let undeclared = api_ref_of(
        &root,
        std::slice::from_ref(&undeclared_element),
        &undeclared_element,
    );
    assert_eq!(undeclared.status, ApiStatus::None, "接口文件缺席即 none");
    assert_eq!(undeclared.contract_path, None, "文档不在盘 ⇒ null");
    assert_eq!(undeclared.schema_path, None, "接口不在盘 ⇒ null");
    assert_eq!(
        undeclared.generated_path.as_deref(),
        Some(generated_rel().as_str()),
        "生成物路径是入口的函数（有入口即给路径串，与资产在盘无关）"
    );
}

/// 异常（AC-3 机械锚点）：试点测试内零旧名引用（旧 `#[path]` 真值源与旧入口形态函数），
/// 等值真值源已改为 `paths::generated_path(...).file_name()`。
#[test]
fn 旧名零残留且等值源为生成物文件名() {
    let own = include_str!("pilot_test.rs");
    for removed in [["mod_path", "_attr"].concat(), ["entry", "_form"].concat()] {
        assert_eq!(
            own.matches(removed.as_str()).count(),
            0,
            "`pilot_test.rs` 内零旧名引用 `{removed}`（真值源已换为生成物文件名）"
        );
    }
    let lib = read_repo("packages/desktop/src-tauri/crates/infra/archi/src/lib.rs");
    let attribute = lib
        .lines()
        .find_map(|line| line.trim().strip_prefix("#[path = \""))
        .and_then(|tail| tail.strip_suffix("\"]"))
        .expect("入口行含 `#[path = \"…\"]` 字面量");
    assert!(
        !attribute.contains('/') && !attribute.contains(".."),
        "字面量恒为文件名（零目录段、零 `../`）：{attribute}"
    );
}

// ===========================================================================
// 组合链路（`parse` → `build_schema` → `verify`）在真实语料上的全线通过
//
// 被测链路挂靠 `api/contract.rs`（第四闸所在处、`api::load_contracts` 的收口点）。零替身：
// 真实读盘 + 真实链路，路径串一律经 `archi::api::display` 归一（跨 crate 单点，MUST NOT
// 自写第二份 `replace`），份数不钉死——由「非留白 / 留白两档各至少一份通过」兜住漂移。
// ===========================================================================

fn walk_schema(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("应可读目录 {}：{error}", dir.display()))
        .map(|entry| entry.expect("目录项应可读").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk_schema(&path, out);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".schema.json"))
        {
            out.push(path);
        }
    }
}

/// 接口定义目录的在盘收面：`(repo 相对路径串, fqn)` 升序；fqn = 相对契约目录的
/// 分层目录段 + 文件主干（经 `paths::schema_fqn` 反查，与写闸同一单点）。
fn contract_files() -> Vec<(String, String)> {
    let root = repository_root();
    let mut files: Vec<PathBuf> = Vec::new();
    walk_schema(&api_dir(), &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(api_dir())
                .expect("收面只含契约目录内的文件");
            let fqn = paths::schema_fqn(relative).expect("收面路径应可反查 FQN");
            (display(&root, &path), fqn)
        })
        .collect()
}

/// 一份在盘契约走完整装载链的结果：路径串、fqn、是否留白面（`ops` / `types` /
/// `reexports` 三项全空 = 第四闸的豁免面）。
struct LoadedContract {
    rel: String,
    fqn: String,
    blank: bool,
}

/// 逐份装载（`parse` → `build_schema` → `verify`）：失败即带 repo 相对路径串返回文案，供
/// 调用方 `panic!`——循环内 MUST NOT `continue` 静默跳过（否则「零误伤」不可判）。
fn load_contract(rel: &str, fqn: &str, source: &str) -> Result<bool, String> {
    let contract = validate_schema(fqn, source)
        .map_err(|error| format!("接口定义 `{rel}` 校验失败：{}", error.message()))?;
    Ok(contract.ops.is_empty() && contract.types.is_empty() && contract.reexports.is_empty())
}

/// 契约目录全量装载：任一份失败即 `panic!`（消息含 repo 相对路径串与错误文案，失败可定位）。
fn load_all_contracts() -> Vec<LoadedContract> {
    contract_files()
        .into_iter()
        .map(|(rel, fqn)| {
            let source = read_repo(&rel);
            let blank =
                load_contract(&rel, &fqn, &source).unwrap_or_else(|message| panic!("{message}"));
            LoadedContract { rel, fqn, blank }
        })
        .collect()
}

/// 正向（组合链路 · AC-3 留白豁免 + 第四闸生效）：`archi_models_dir` 递归全部 `*.md` 逐份
/// 走完 `parse` → `build_schema` → `verify` ⇒ 全 `Ok(())`；**不钉死份数**，改断言「至少一份
/// 非留白契约（闸生效面）与至少一份留白契约（豁免面）各有一份通过」。
#[test]
fn 契约目录全量装载全线通过且两档各有通过项() {
    let files = contract_files();
    let loaded = load_all_contracts();

    assert_eq!(
        loaded.len(),
        files.len(),
        "收面内的每一份 `*.md` 都被装载（不按文件名白名单豁免、不静默跳过）"
    );
    assert!(!loaded.is_empty(), "契约目录应至少有一份 `*.md`");
    assert!(
        loaded.iter().any(|item| !item.blank),
        "至少一份非留白契约通过（第四闸生效面）"
    );
    assert!(
        loaded.iter().any(|item| item.blank),
        "至少一份留白契约通过（三项全空的豁免面）"
    );
}

/// 正向（组合链路 · 后端两份）：`Backend.Core.Foundation` /
/// `Backend.Infra.ArchiRuntime` 落在通过集内，且接口定义至少声明一个操作（非留白面）。
#[test]
fn 组合链路后端两份散文零表格行() {
    let loaded = load_all_contracts();
    for fqn in ["Backend.Core.Foundation", "Backend.Infra.ArchiRuntime"] {
        let item = loaded
            .iter()
            .find(|item| item.fqn == fqn)
            .unwrap_or_else(|| panic!("`{fqn}` 应在全量装载的通过集内"));
        assert!(
            !item.blank,
            "`{fqn}` 是非留白接口面（自有 op / 类型非空），实际判为留白"
        );
        let source = read_repo(&item.rel);
        let value: Value = serde_json::from_str(&source).expect("接口定义是合法 JSON");
        let properties = value["properties"]
            .as_object()
            .expect("`properties` 是对象");
        assert!(
            properties
                .values()
                .any(|property| property.get("x-function").is_some()),
            "`{fqn}` 至少声明一个操作：{properties:?}"
        );
    }
}

/// 正向（组合链路 · 前端留白面）：`Desktop/` 下的接口定义全部落在通过集内且判留白——
/// 空 `$defs {}` + 空 `properties {}` 是可落盘的合法留白形态（与「接口还没写」严格可分）。
#[test]
fn 组合链路前端留白契约全量通过() {
    let loaded = load_all_contracts();
    let frontend: Vec<&LoadedContract> = loaded
        .iter()
        .filter(|item| {
            item.rel
                .starts_with(".dev-team/architecture/models/Desktop/")
        })
        .collect();
    assert!(!frontend.is_empty(), "前端组件接口面应在收面内");
    for item in frontend {
        assert!(
            item.blank,
            "`{}` 三项全空即留白接口面（合法：无可导出符号）",
            item.rel
        );
        let source = read_repo(&item.rel);
        let value: Value = serde_json::from_str(&source).expect("接口定义是合法 JSON");
        assert_eq!(
            value["$defs"].as_object().map(|defs| defs.len()),
            Some(0),
            "`{}` 的 `$defs` 为空对象（留白形态）",
            item.rel
        );
    }
}

/// 异常（组合链路 · 失败可见）：装载失败时消息含 repo 相对路径串与错误文案（可定位），
/// MUST NOT 落成「静默跳过」。
#[test]
fn 组合链路失败可见且不静默跳过() {
    let rel = ".dev-team/architecture/models/Backend/Demo/Broken.schema.json";
    let broken_json = "{\n  \"$id\": \"Backend.Demo.Broken.schema.json\",\n";
    let message = load_contract(rel, "Backend.Demo.Broken", broken_json)
        .expect_err("截断的 JSON ⇒ 应报装载期错误");
    assert!(message.contains(rel), "消息含 repo 相对路径串：{message}");
    assert!(
        message.contains("不是合法 JSON"),
        "消息含解析期错误文案：{message}"
    );

    let mismatched_id = concat!(
        r#"{"$id": "Backend.Demo.Other.schema.json", "#,
        r#""title": "Backend.Demo.Broken", "#,
        r#""$defs": {}, "properties": {}}"#,
        "\n",
    );
    let broken_id = load_contract(rel, "Backend.Demo.Broken", mismatched_id)
        .expect_err("`$id` 与文件名不符 ⇒ 应报装载期错误");
    assert!(
        broken_id.contains(rel) && broken_id.contains("校验失败"),
        "校验期失败同样点名路径：{broken_id}"
    );
}

/// 边界（组合链路 · 收面口径）：收面恰为 `<archi_models_dir>/**/*.schema.json`——只认
/// `.schema.json`、路径串经 `archi::api::display` 归一、`graph/` 是兄弟目录不在收面。
#[test]
fn 组合链路收面为契约目录下全部md() {
    let files = contract_files();
    assert!(!files.is_empty(), "收面应至少一份 `*.schema.json`");

    for (rel, fqn) in &files {
        assert!(rel.ends_with(".schema.json"), "只认 `*.schema.json`：{rel}");
        assert!(
            rel.starts_with(".dev-team/architecture/models/"),
            "路径串经 `archi::api::display` 归一为 repo 相对 POSIX：{rel}"
        );
        assert!(!rel.contains('\\'), "路径串零反斜杠：{rel}");
        assert!(
            !rel.contains("graph/"),
            "`graph/` 是兄弟目录，MUST NOT 进接口收面：{rel}"
        );
        assert!(
            !fqn.is_empty() && !fqn.ends_with(".schema.json"),
            "fqn = 分层目录段 + 文件主干：{fqn}"
        );
    }

    let graph = repository_root().join(".dev-team/architecture/graph");
    assert!(graph.is_dir(), "`graph/` 目录在盘（与接口目录分家）");
    assert!(
        !files.iter().any(|(rel, _)| rel.contains("/graph/")),
        "不递归第二根（`graph/model/*.toml` 天然不在收面）"
    );
}

/// 边界（组合链路 · 夹具单点复用）：与同文件既有夹具共存且互不遮蔽——路径串归一走
/// `archi::api::display`（跨 crate 单点），本文件内零第二份 `replace`、上溯层数仍单点。
#[test]
fn 组合链路夹具单点复用不另写路径串() {
    let root = repository_root();
    for (rel, _) in contract_files() {
        assert_eq!(
            display(&root, &root.join(&rel)),
            rel,
            "`archi::api::display` 是路径串归一的跨 crate 单点"
        );
    }

    let text = include_str!("pilot_test.rs");
    // 判据串分段拼接：避免断言文本自身命中（分段后本文件内不存在连续的该调用形态）
    let replace_needle = [
        ".replace", "(", "'", "\\", "\\", "'", ",", " ", "\"/\"", ")",
    ]
    .concat();
    assert_eq!(
        text.matches(&replace_needle).count(),
        0,
        "MUST NOT 自写第二份路径串 `replace`（走 `archi::api::display`）：{replace_needle}"
    );
    let pop_needle = [".", "pop", "(", ")", ";"].concat();
    assert_eq!(
        text.matches(&pop_needle).count(),
        1,
        "上溯层数是单点（MUST NOT 出现第二份上溯实现）"
    );
}
