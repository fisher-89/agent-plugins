//! `$defs` 规则校验、`schema.json` 装配与 `INDEX.toml` 装配。
//!
//! **生成器只做装配、不做翻译**：`schema.json` 的每一项都有 markdown 出处
//! （`$schema` / `$id` 常量、`title` 取 H1、`$defs` 取 `api-types` + `api-reexport`
//! 透传 alias、`properties` 取 op 键 + 类型 `$ref`、`$comment` 常量），fence 内容
//! 本身就是原生 JSON Schema——`minLength` / `pattern` / `enum` / `anyOf` / `x-*`
//! 全在 md 内可表达。表达不了即生成器能力不足，应修生成器，MUST NOT 开手改口子。
//!
//! **索引零语言词**：`INDEX.toml` 里不出现 `crate` / `Cargo.toml` / `Rust` /
//! `TypeScript`——语言特定只落取数器与渲染器。

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use toml_edit::{DocumentMut, Item, Table, Value as TomlValue};

use super::contract::Contract;
use super::{kind_of, paths, short_digest, ApiError, ApiMeta, DEFS_FRAGMENT, SCHEMA_FILE_SUFFIX};

/// `INDEX.toml` 的契约文件在案状态（封闭二值）：文件事实，与模型意图
/// [`crate::dto::ApiStatus`] 分立——后者是「承诺有契约」，前者是「文件在案」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiFormStatus {
    /// 契约文件在案
    Declared,
    /// `declared` 但文件缺失
    Absent,
}

impl ApiFormStatus {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            ApiFormStatus::Declared => "declared",
            ApiFormStatus::Absent => "absent",
        }
    }
}

/// `INDEX.toml` 的单元素条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// 元素 FQN
    pub fqn: String,
    /// 契约文件事实
    pub status: ApiFormStatus,
    /// `$defs` 条目数
    pub types: usize,
    /// 操作数
    pub ops: usize,
    /// `x-common.degraded` 条目数
    pub degraded: usize,
    /// 分类分布（`category` → 条数；开放值集）
    pub categories: BTreeMap<String, usize>,
    /// schema 生成物的 repo 相对路径（`absent` 时为空串）
    pub schema_rel: String,
    /// 元素生成物的 repo 相对路径（无则 `None`）
    pub generated_rel: Option<String>,
}

/// `schema.json` 装配（六字段，逐项有 md 出处；`serde_json::Map` 保序 ⇒ 幂等 diff）。
///
/// **两处键空间各自唯一**（`serde_json::Map::insert` 会静默覆盖，故两处都先判后写）：
/// ① `$defs` 内 `api-types` 与 `api-reexport` 不得同名（两条来源合并成同一个键空间）；
/// ② `properties` 内 op 键与类型键不得同名（同一对象装两类条目，同名会让「这是操作
/// 还是类型引用」在产物里不可辨）。任一处撞名即 `ApiError::Contract` 并点名该名。
pub fn build_schema(contract: &Contract, meta: &ApiMeta) -> Result<Value, ApiError> {
    let mut defs: Map<String, Value> = Map::new();
    for (name, value) in &contract.types {
        defs.insert(name.clone(), value.clone());
    }
    for (name, target) in &contract.reexports {
        let Some(target_fqn) = target.as_str() else {
            return Err(ApiError::Contract(format!(
                "{}: {name} 的透传目标必须是子元素 FQN 字符串（实际为 {}），\
                 透传条目只表达「导出自哪个直接子元素」",
                contract.file,
                kind_of(target)
            )));
        };
        if defs.contains_key(name) {
            return Err(ApiError::Contract(format!(
                "{}: 类型名 `{name}` 同时出现在 `api-types` 与 `api-reexport`——\
                 `$defs` 的键空间唯一，同名即定义两次（MUST NOT 由后写者静默覆盖）",
                contract.file
            )));
        }
        let prefix = paths::ref_relative(&contract.fqn, target_fqn);
        defs.insert(
            name.clone(),
            serde_json::json!({
                "$ref": format!("{prefix}{target_fqn}{SCHEMA_FILE_SUFFIX}#/$defs/{name}")
            }),
        );
    }

    let mut properties: Map<String, Value> = Map::new();
    for op in &contract.ops {
        // 操作项 = `api-op` fence 原文包进 `x-function`（fence 内即 params / returns /
        // async 三键，块内不自包含 op 名——名字由 heading 提供）
        properties.insert(op.id.clone(), function_schema(&op.signature));
    }
    for name in defs.keys() {
        if properties.contains_key(name) {
            return Err(ApiError::Contract(format!(
                "{}: op id `{name}` 与类型名同名——`properties` 的键空间唯一\
                 （同名会让「操作」与「类型引用」在生成物里不可辨，MUST NOT 静默覆盖）",
                contract.file
            )));
        }
        properties.insert(
            name.clone(),
            serde_json::json!({ "$ref": format!("{DEFS_FRAGMENT}{name}") }),
        );
    }

    let mut schema: Map<String, Value> = Map::new();
    schema.insert(
        "$schema".to_owned(),
        Value::String(meta.schema_uri.to_owned()),
    );
    schema.insert(
        "$id".to_owned(),
        Value::String(format!("{}.schema.json", contract.fqn)),
    );
    schema.insert("title".to_owned(), Value::String(contract.title.clone()));
    schema.insert(
        "$comment".to_owned(),
        Value::String(meta.generated_notice.to_owned()),
    );
    schema.insert("$defs".to_owned(), Value::Object(defs));
    schema.insert("properties".to_owned(), Value::Object(properties));
    Ok(Value::Object(schema))
}

/// `$defs` 五条规则 + 透传条目纯 `$ref` 校验 + `x-class` 带属性判错：
///
/// 1. 身份 = 键名（JSON 对象天然键唯一）；
/// 2. 嵌套 `$defs` MUST 为空——名字提升后原名保留，MUST NOT 构成作用域；
/// 3. 所有 `$ref` target MUST 是顶层条目（`#/$defs/<名>`）或外部文件形式
///    （`<相对前缀><FQN>.schema.json#/$defs/<名>`）；
/// 4. 同名只定义一次；
/// 5. **MUST NOT 按结构去重**（本函数不做任何合并动作即满足；相等形状不合并）。
///
/// `x-class: true` 的类型是 trait，**仅允许方法**：条目带 `required`，或某个
/// `properties` 值不是 `x-function`（即声明了数据属性），即契约错误——本条拦在装载期
/// （[`super::load_contracts`] 随即中止、不产出半真报告）；反方向的
/// 「`properties` 全为函数却未标 `x-class`」只是意图提示（警告），落
/// [`super::emit::render_target`] 的语言分派处。
pub fn verify_defs(schema: &Value) -> Result<(), ApiError> {
    let Some(defs) = schema.get("$defs").and_then(Value::as_object) else {
        return Err(ApiError::Contract(
            "schema 缺 `$defs` 对象（每个具名类型 MUST 在顶层定义恰一次）".to_owned(),
        ));
    };
    for (name, definition) in defs {
        let Some(definition) = definition.as_object() else {
            return Err(ApiError::Contract(format!(
                "`$defs.{name}` 的值必须是 JSON 对象（实际为 {}）",
                kind_of(definition)
            )));
        };
        if let Some(nested) = definition.get("$defs") {
            return Err(ApiError::Contract(format!(
                "`$defs.{name}` 内出现嵌套 `$defs`——嵌套定义 MUST 提升到顶层且原名保留（{}）",
                kind_of(nested)
            )));
        }
        if let Some(reference) = definition.get("$ref") {
            let Some(reference) = reference.as_str() else {
                return Err(ApiError::Contract(format!(
                    "`$defs.{name}.$ref` 必须是字符串"
                )));
            };
            if !is_top_level_ref(reference) && !is_external_ref(reference) {
                return Err(ApiError::Contract(format!(
                    "`$defs.{name}.$ref` 指向 `{reference}`——target MUST 是顶层 `$defs` 条目\
                     或外部契约文件，MUST NOT 指向 `properties` / `items` / `allOf/0` 一类无名位置"
                )));
            }
            for disallowed in ["type", "properties", "enum", "const", "items", "oneOf"] {
                if definition.contains_key(disallowed) {
                    return Err(ApiError::Contract(format!(
                        "`$defs.{name}` 同时带 `$ref` 与结构关键字 `{disallowed}`——\
                         透传条目 MUST 是纯 `$ref` alias，携带结构即沦为重复定义"
                    )));
                }
            }
        }
        // `x-class: true` 的 trait **仅允许方法**：带 `required`，或某个 `properties` 值
        // 不是 `x-function`（即声明了数据字段）⇒ 契约错误。排在嵌套 `$defs` 与 `$ref`
        // 两条之后（同一 `$defs` 骨架）。反方向的「`properties` 全为函数却未标
        // `x-class`」是合法意图（函数指针结构体），只产警告，落
        // `emit::render_target` 的语言分派处。
        if definition.get("x-class").and_then(Value::as_bool) == Some(true) {
            if definition.contains_key("required") {
                return Err(ApiError::Contract(format!(
                    "`$defs.{name}` 标了 `x-class` 却同时带 `required`——trait 仅允许方法；\
                     普通对象去掉 `x-class` 即通过"
                )));
            }
            for (field, property) in definition
                .get("properties")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                if property.get("x-function").is_some() {
                    continue;
                }
                return Err(ApiError::Contract(format!(
                    "`$defs.{name}` 标了 `x-class` 却在 `properties.{field}` 上声明了属性——\
                     trait 仅允许方法（每个 `properties` 值 MUST 是 `x-function`）；\
                     普通对象去掉 `x-class` 即通过"
                )));
            }
        }
        collect_nested_refs(name, definition)?;
    }
    Ok(())
}

/// `INDEX.toml` 文本装配：顶部说明段（生成警示 / review 口径 / 生成器标识）+
/// 按 FQN 字典序的条目。
pub fn build_index(entries: &[IndexEntry], meta: &ApiMeta) -> String {
    let mut ordered: Vec<&IndexEntry> = entries.iter().collect();
    ordered.sort_by(|left, right| left.fqn.cmp(&right.fqn));

    let mut document = DocumentMut::new();
    let contract_dir = foundation::layout::archi_api_rel_dir();
    let digest = short_digest(&meta.common_digest);
    let header = format!(
        "由 archi-api-gen 生成 · 勿手改\n\
         review 口径：只读元素目录下的 `<FQN>.md`；本文件与 `*.schema.json` 的 diff 视为机械噪声。\n\
         契约面：目录 {} · 生成器 archi-api {} · 映射规则摘要 {}\n",
        contract_dir, meta.tool_version, digest
    );
    document.decor_mut().set_prefix(format!(
        "{}
",
        comment_block(&header)
    ));
    let mut array = toml_edit::ArrayOfTables::new();
    for entry in ordered {
        let mut table = Table::new();
        table["fqn"] = TomlValue::from(entry.fqn.clone()).into();
        table["status"] = TomlValue::from(entry.status.as_str()).into();
        table["types"] = TomlValue::from(entry.types as i64).into();
        table["ops"] = TomlValue::from(entry.ops as i64).into();
        table["degraded"] = TomlValue::from(entry.degraded as i64).into();
        table["schema"] = TomlValue::from(entry.schema_rel.clone()).into();
        table["generated"] =
            TomlValue::from(entry.generated_rel.clone().unwrap_or_default()).into();
        let mut categories = Table::new();
        for (category, count) in &entry.categories {
            categories[category.as_str()] = TomlValue::from(*count as i64).into();
        }
        if !categories.is_empty() {
            table["categories"] = Item::Table(categories);
        }
        array.push(table);
    }
    document["entry"] = Item::ArrayOfTables(array);
    document.to_string()
}

/// `#` 注释块（逐行前缀化；末尾补换行）。
fn comment_block(text: &str) -> String {
    let mut output = String::new();
    for line in text.lines() {
        output.push_str("# ");
        output.push_str(line);
        output.push('\n');
    }
    output
}

/// 操作项装配：`api-op` fence 原文 → `{ x-function: { params, returns, async? } }`。
///
/// 块内允许两种等价写法——带 `params` / `returns` 键的完整形态，或直接给
/// `{ "x-function": … }`（生成器不猜测、只补标记层）。
fn function_schema(signature: &Value) -> Value {
    match signature.get("x-function") {
        Some(inner) => serde_json::json!({ "x-function": inner.clone() }),
        None => serde_json::json!({ "x-function": signature.clone() }),
    }
}

/// 顶层 `$defs` 引用的形态判据（`#/$defs/<名>`）。
fn is_top_level_ref(reference: &str) -> bool {
    reference
        .strip_prefix(DEFS_FRAGMENT)
        .is_some_and(|name| !name.is_empty() && !name.contains('/'))
}

/// 外部契约文件引用（`<相对前缀><FQN>.schema.json#/$defs/<名>`）。
fn is_external_ref(reference: &str) -> bool {
    match reference.split_once(DEFS_FRAGMENT) {
        Some((file, name)) => {
            file.ends_with(SCHEMA_FILE_SUFFIX) && !name.is_empty() && !name.contains('/')
        }
        None => false,
    }
}

/// 递归检查 `$ref` 只出现在合法位置（嵌套 `$defs` 之外，`items` / `allOf` 内的
/// `$ref` 允许但 target 仍须顶层或外部；本函数不合并、不改名）。
fn collect_nested_refs(name: &str, value: &Map<String, Value>) -> Result<(), ApiError> {
    for (key, nested) in value {
        if key == "$ref" {
            continue;
        }
        walk_refs(name, nested)?;
    }
    Ok(())
}

/// 递归走值：任何 `$ref` 都必须是顶层或外部引用。
fn walk_refs(name: &str, value: &Value) -> Result<(), ApiError> {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                if key == "$ref" {
                    let Some(reference) = nested.as_str() else {
                        return Err(ApiError::Contract(format!(
                            "`$defs.{name}` 内出现非字符串 `$ref`"
                        )));
                    };
                    if !is_top_level_ref(reference) && !is_external_ref(reference) {
                        return Err(ApiError::Contract(format!(
                            "`$defs.{name}` 内的 `$ref` 指向 `{reference}`——target MUST 是顶层 \
                             `$defs` 条目或外部契约文件"
                        )));
                    }
                    continue;
                }
                walk_refs(name, nested)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                walk_refs(name, item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
