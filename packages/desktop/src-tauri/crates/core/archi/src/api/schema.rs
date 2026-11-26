//! 接口定义文件的**唯一**校验单点、`$defs` 规则校验与 `INDEX.toml` 装配。
//!
//! **真相方向**：`<叶段>.schema.json` 是**手写的接口真相**（人写、写闸校验），`INDEX.toml`
//! 与 `.i.rs` / `.i.ts` 是生成物。故本模块的公开入口是
//! [`validate_schema`]（写闸与装载期共用的九道闸），而不再是「由 md 装配 schema」。
//!
//! **文件级自洽、零跨元素**：[`validate_schema`] 只吃「本文件 + 本 FQN」。跨元素的一切
//! （`$ref` 目标是否在模型内 / 是否直接子元素 / 目标是否 `api = none` / 目标文件在不在盘）
//! 归 `apiRefMissing` 既有判据面——写闸做它就变成第二份判据。唯一例外是跨元素 `$ref` 的
//! **前缀**：它只需两个 FQN 做纯计算，故留在写闸。
//!
//! **索引零语言词**：`INDEX.toml` 里不出现 `crate` / `Cargo.toml` / `Rust` /
//! `TypeScript`——语言特定只落取数器与渲染器。
//!
//! **`x-class` 的骨架与两个行为面键的判据只在本模块**：[`verify_defs`] 一趟判
//! ① 骨架四判据（`type: "object"` / 无 `additionalProperties` / 无 `required` /
//! `properties` 全为方法）、② 两个新键（`x-constructor` / `x-static`）的形态、③ 它们的落点
//! （只许在 `x-class` 条目顶层，全文档单趟）。名字族与预留名归 [`super::contract::verify`]，
//! 渲染期零判据——同一事实 MUST NOT 有第二份实现。

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use toml_edit::{DocumentMut, Item, Table, Value as TomlValue};

use super::contract::{self, Contract};
use super::{kind_of, paths, short_digest, ApiError, ApiMeta, DEFS_FRAGMENT, SCHEMA_FILE_SUFFIX};

/// **封闭降级词表**（首期四值）：`x-common.degraded` 的取值域。
///
/// | 值 | 语义 |
/// |---|---|
/// | `foreignElement` | 跨域 / 非直接子元素引用 |
/// | `noElement` | 目标无模型元素（如 hooks / 纯工具库 / 裸类型模块） |
/// | `unrepresentable` | 目标语言类型系统表达不了（如 JSX 形态、错误通道走 `Promise` reject） |
/// | `crossLanguage` | 规则表降级（`common.json` 的换算在目标语言侧的表达损失） |
///
/// 新增第五值 MUST 改 `desktop-archi-api` 的「降级词表与真实债表」（与既有「判据词表
/// 封闭」同一条纪律）。**MUST NOT 进 `common.json`**：该资产的语义封闭为**映射规则**，
/// 且其摘要进全部生成物的头注释（改它即批量重生成）。
pub(crate) const DEGRADED_KINDS: [&str; 4] = [
    "foreignElement",
    "noElement",
    "unrepresentable",
    "crossLanguage",
];

/// `INDEX.toml` 的接口文件在案状态（封闭二值）：文件事实，与模型意图
/// [`crate::dto::ApiStatus`] 分立——后者是「承诺有接口」，前者是「文件在案」。
///
/// **值域不动**（`declared` / `absent`）：`present`（`declared` 的判定）改为直判
/// 「`<叶段>.schema.json` 在盘」，命名债（`declared` 读起来像意图）并入清理轮。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiFormStatus {
    /// 接口文件在案
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
    /// 接口文件事实（`declared` = `<叶段>.schema.json` 在盘）
    pub status: ApiFormStatus,
    /// `$defs` 条目数
    pub types: usize,
    /// 操作数
    pub ops: usize,
    /// `x-common.degraded` 标记的**出现次数**（全深度：`$defs` 条目自身、条目第一层
    /// `properties` 的值、`properties` 里的 `x-function.params[].schema` 及更深；与
    /// [`degraded_marks`] 同源，不再是 `$defs` 条目数）
    pub degraded: usize,
    /// 分类分布（`category` → 条数；开放值集）
    pub categories: BTreeMap<String, usize>,
    /// 接口定义文件的 repo 相对路径（`absent` 时为空串）
    pub schema_rel: String,
    /// 元素生成物的 repo 相对路径（无则 `None`）
    pub generated_rel: Option<String>,
}

/// `x-common.degraded` 的**全深度单点扫描**：返回每个标记的 `(JSON pointer, 原值)`。
///
/// 扫描面是 `$defs` 条目自身、条目第一层 `properties` 的值、`properties` 里的
/// `x-function.params[].schema` 以及更深的对象 / 数组。锚点是**对象自身的
/// `x-common.degraded`**，而非任意名为 `degraded` 的键——数据字段名合法可为 `degraded`，
/// 松锚点会把它计成债。pointer 指到标记本身（`$defs.<名>.properties.<字段>.x-common.degraded`，
/// 数组段 `[i]`，根节点段 `$`）。
///
/// [`verify_defs`]（取值校验）与 [`super::index_entries`]（计数）各调一次本函数，
/// MUST NOT 存在第二套遍历。返回值保留 [`Value`] 形态：校验面要能拒收**非字符串**值并
/// 点名其形态，计数面只用 `.len()`。
pub(crate) fn degraded_marks(schema: &Value) -> Vec<(String, &Value)> {
    let mut marks: Vec<(String, &Value)> = Vec::new();
    collect_degraded(schema, "$", &mut marks);
    marks
}

/// [`degraded_marks`] 的递归主体（逐键下沉，对象键段 `.key`、数组段 `[i]`）。
fn collect_degraded<'a>(value: &'a Value, pointer: &str, marks: &mut Vec<(String, &'a Value)>) {
    match value {
        Value::Object(object) => {
            if let Some(mark) = object
                .get("x-common")
                .and_then(Value::as_object)
                .and_then(|common| common.get("degraded"))
            {
                marks.push((format!("{pointer}.x-common.degraded"), mark));
            }
            for (key, inner) in object {
                collect_degraded(inner, &format!("{pointer}.{key}"), marks);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_degraded(item, &format!("{pointer}[{index}]"), marks);
            }
        }
        _ => {}
    }
}

/// 接口定义文件的**唯一**校验入口（写闸与装载期共用，只差错误映射）：九道闸一趟走完
/// 并产出 [`Contract`]。
///
/// 闸序固定（顺序即判据：先便宜后昂贵、先自洽后复用）：
///
/// ```text
/// ① 文本层重复键    serde_json 会把重复键折叠成「后者胜」，手写「同名两处定义」会静默
/// ② JSON 可解析
/// ③ identity        `$id` == `<FQN>.schema.json`、`title` == `<FQN>`（第三处是落点：
///                   分层目录段 + 文件主干由调用方经 `paths::schema_fqn` 反查出 FQN
///                   ⇒ 与 `$id` / `title` 承载的元素身份逐字相同即成立）
/// ④ 键空间          op 键集（带 `x-function` 的 `properties` 键）∩ `$defs` 键集 = ∅
/// ⑤ 形态白名单      `properties.<k>` 恰一键二选一：`{ "x-function": {…} }` 或
///                   `{ "$ref": "#/$defs/<名>" }`（且 `<名> ∈ $defs`）
/// ⑥ 镜像            `$defs` 每个键在 `properties` 有指向自己的 `$ref`
/// ⑦ `$ref` 前缀     跨元素串的文件部分逐字等于 `ref_relative(本FQN, 目标FQN)`
/// ⑧ 顶层键封闭      六键封闭；`$id` / `title` / `$defs` / `properties` 四键必在场
/// ⑨ 复用三闸        [`super::contract::verify`]（七族名字 / 落点（封闭四处）/ `required ⊆ properties`
///                   / `x-class` 条目的三条预留名）
///                   + [`verify_defs`]（五条 `$defs` 规则、`x-class` 骨架四判据与两键形态、
///                   两键落点单趟、降级词表）
/// ```
///
/// `Contract.file` 取接口文件的 repo 相对路径（`<models>/<分层目录…>/<叶段>.schema.json`，
/// 本函数无 root 入参，经 [`paths::schema_rel`] 派生）；装载期在成功后把它覆写为实际
/// 相对路径（报告的 `occurrences.file` 口径）。
pub fn validate_schema(fqn: &str, text: &str) -> Result<Contract, ApiError> {
    let file = paths::schema_rel(fqn).unwrap_or_else(|| format!("{fqn}{SCHEMA_FILE_SUFFIX}"));
    if let Some((key, offset)) = contract::duplicate_keys(text) {
        return Err(contract::duplicate_key_error(&file, &key, offset));
    }
    let value: Value = serde_json::from_str(text).map_err(|error| {
        ApiError::Contract(format!(
            "{file}: 不是合法 JSON：{error}——按上文行列修掉该处语法错误后重新写入"
        ))
    })?;
    if !value.is_object() {
        return Err(ApiError::Contract(format!(
            "{file}: 顶层必须是 JSON 对象（实际为 {}）——接口定义以 `$defs` / `properties` \
             两表承载导出名",
            kind_of(&value)
        )));
    }
    verify_identity(fqn, &value).map_err(|error| locate(&file, error))?;
    verify_keyspace(&value).map_err(|error| locate(&file, error))?;
    verify_shape(&value).map_err(|error| locate(&file, error))?;
    verify_mirror(&value).map_err(|error| locate(&file, error))?;
    verify_ref_prefix(fqn, &value).map_err(|error| locate(&file, error))?;
    verify_top_level_keys(&value).map_err(|error| locate(&file, error))?;
    let contract = Contract::from_schema(fqn, &file, &value);
    contract::verify(&contract, &value)?;
    verify_defs(&value)?;
    Ok(contract)
}

/// 闸 ③：`$id` / `title` 与 FQN 逐字一致。
///
/// **元素身份三处持有**（`$id` / `title` / 落点）：`$id` 与 `title` 都写 FQN，落点由
/// 分层目录段 + 叶段文件主干拼回 FQN（文件名只取叶段，故不再与 `$id` 同串）。漂移会让
/// 反查（`reexport_target` 解 `$ref` 前缀）与报告文案指向不同元素。落点一处由调用方经
/// [`super::paths::schema_fqn`] 反查出 FQN 后与本入参比较，故这里只判两处。
pub fn verify_identity(fqn: &str, value: &Value) -> Result<(), ApiError> {
    let expected_id = format!("{fqn}{SCHEMA_FILE_SUFFIX}");
    match value.get("$id").and_then(Value::as_str) {
        Some(id) if id == expected_id => {}
        Some(id) => {
            return Err(ApiError::Contract(format!(
                "`$id` 是 `{id}`，期望 `{expected_id}`——`$id` / `title` / 落点三处持有同一\
                 元素身份，必须逐字相同（改成 `{expected_id}`）"
            )))
        }
        None => {
            return Err(ApiError::Contract(format!(
                "缺 `$id`（MUST 为 `{expected_id}`）"
            )))
        }
    }
    match value.get("title").and_then(Value::as_str) {
        Some(title) if title == fqn => Ok(()),
        Some(title) => Err(ApiError::Contract(format!(
            "`title` 是 `{title}`，期望 `{fqn}`——`title` 与 FQN 逐字相同（元素身份的\
             报告口径，与 `$id` 并列）"
        ))),
        None => Err(ApiError::Contract(format!("缺 `title`（MUST 为 `{fqn}`）"))),
    }
}

/// 闸 ④：**op 键集 ∩ `$defs` 键集 = ∅**。
///
/// 不是「`properties` 键集与 `$defs` 键集无重名」：⑥ 的镜像要求每个 `$defs` 键在
/// `properties` 有指向自己的 `$ref` ⇒ 两键集**必须**相交。真正要拒的是「同名既是操作
/// 又是类型」——那会让「这是操作还是类型引用」在产物里不可辨。
pub fn verify_keyspace(value: &Value) -> Result<(), ApiError> {
    let defs = defs_of(value);
    for (key, property) in properties_of(value).into_iter().flatten() {
        if property.get(X_FUNCTION).is_none() {
            continue;
        }
        if defs.is_some_and(|defs| defs.contains_key(key)) {
            return Err(ApiError::Contract(format!(
                "键 `{key}` 同时是操作（`properties.{key}.x-function`）与类型（`$defs.{key}`）——\
                 两个键空间必须互斥：操作改名，或把类型放进 `$defs` 的另一个名字下"
            )));
        }
    }
    Ok(())
}

/// 闸 ⑤：`properties.<k>` 的形态白名单（**恰一键**）。
///
/// 合法二选一：`{ "x-function": { … } }`（值须对象）或 `{ "$ref": "#/$defs/<名>" }`
/// （`<名> ∈ $defs`）。其余形态（内联 `object` / 数组 / `oneOf` / 裸 `type`）一律
/// `ApiError`——内联 object 会经 `schema_type` 的 type 分派静默退化为
/// `Record<string, unknown>`（编译通过、类型全丢），这是本闸存在的唯一理由。
/// `x-function` 的**内层**是自由形状（`async` / 将来可能的 `deprecated`）：本闸只判外层。
pub fn verify_shape(value: &Value) -> Result<(), ApiError> {
    let defs = defs_of(value);
    for (key, property) in properties_of(value).into_iter().flatten() {
        let Some(object) = property.as_object() else {
            return Err(shape_error(key));
        };
        if let Some(function) = object.get(X_FUNCTION) {
            if object.len() != 1 {
                return Err(ApiError::Contract(format!(
                    "`properties.{key}` 只允许恰一个键——`x-function` 不得与其他键并列（\
                     `description` 一类注记写进 `x-function` 内层）"
                )));
            }
            if !function.is_object() {
                return Err(ApiError::Contract(format!(
                    "`properties.{key}.x-function` 的值必须是对象（实际为 {}）——\
                     它承载 `params` / `returns` / `async` / `owner` / `description`",
                    kind_of(function)
                )));
            }
            continue;
        }
        if let Some(reference) = object.get("$ref") {
            if object.len() != 1 {
                return Err(ApiError::Contract(format!(
                    "`properties.{key}` 只允许恰一个键——`$ref` 不得与其他键并列"
                )));
            }
            let Some(reference) = reference.as_str() else {
                return Err(ApiError::Contract(format!(
                    "`properties.{key}.$ref` 必须是字符串"
                )));
            };
            let Some(name) = reference
                .strip_prefix(DEFS_FRAGMENT)
                .filter(|name| !name.is_empty() && !name.contains('/'))
            else {
                return Err(ApiError::Contract(format!(
                    "`properties.{key}.$ref` 指向 `{reference}`——`properties` 只允许本文件内的 \
                     `#/$defs/<名>`；跨元素引用写进 `$defs` 的纯 `$ref` alias"
                )));
            };
            if !defs.is_some_and(|defs| defs.contains_key(name)) {
                return Err(ApiError::Contract(format!(
                    "`properties.{key}.$ref` 指向 `{name}`，但本文件 `$defs` 里没有该条目——\
                     先声明类型，或改指向已有条目"
                )));
            }
            continue;
        }
        return Err(shape_error(key));
    }
    Ok(())
}

/// 闸 ⑤ 的错误面：二选一形态 + 可执行出路（想把类型复用就放进 `$defs`）。
fn shape_error(key: &str) -> ApiError {
    ApiError::Contract(format!(
        "`properties.{key}` 的形态非法：只允许 `{{ \"x-function\": {{ … }} }}`（声明操作）或 \
         `{{ \"$ref\": \"#/$defs/<名>\" }}`（引用本文件类型）——内联 `object` 会静默退化成 \
         `Record<string, unknown>`；想复用类型就放进 `$defs`，想声明操作就用 `x-function`"
    ))
}

/// 闸 ⑥：镜像——`$defs` 每个键在 `properties` 有指向自己的 `$ref`。
///
/// 两侧同表是「类型可见」的表达方式：`$defs` 只定义、`properties` 才导出。缺镜像的
/// 条目在产物里既不可见也无从引用（`emit` 只渲染 `properties` 可达面），静默丢失。
pub fn verify_mirror(value: &Value) -> Result<(), ApiError> {
    let properties = properties_of(value);
    for (name, _) in defs_of(value).into_iter().flatten() {
        let expected = format!("{DEFS_FRAGMENT}{name}");
        let mirrored = properties
            .and_then(|properties| properties.get(name))
            .and_then(|property| property.get("$ref"))
            .and_then(Value::as_str)
            .is_some_and(|reference| reference == expected);
        if !mirrored {
            return Err(ApiError::Contract(format!(
                "`$defs.{name}` 在 `properties` 里没有指向自己的镜像条目——补 \
                 `\"{name}\": {{ \"$ref\": \"{expected}\" }}`（`$defs` 只定义、`properties` \
                 才导出）"
            )));
        }
    }
    Ok(())
}

/// 闸 ⑦：跨元素 `$ref` 的文件部分逐字等于 `ref_relative(本FQN, 目标FQN) + <目标叶段>`。
///
/// 目标 FQN 由「本元素分层目录 + 相对前缀」解出（文件部分带 `.schema.json` 即入判；
/// 解不出——越界 / 带点的段——同样是错）。少一个 `../` 会绕过 `apiRefMissing` 并在
/// `.i.ts` 里 import 一个不存在的路径、到编译期才炸；因此前缀校验留在这里（只需两个
/// FQN 做纯计算，仍是文件级自洽）。文案并列「当前串」与「期望串」。同文件引用
/// （`#/$defs/<名>`）不在射程。
pub fn verify_ref_prefix(fqn: &str, value: &Value) -> Result<(), ApiError> {
    let mut references: Vec<String> = Vec::new();
    collect_refs(value, &mut references);
    for reference in references {
        let Some(file) = reference.split_once('#').map(|(file, _)| file) else {
            continue;
        };
        if file.is_empty() || !file.ends_with(SCHEMA_FILE_SUFFIX) {
            continue;
        }
        let Some(target) = paths::ref_target(fqn, file) else {
            return Err(ApiError::Contract(format!(
                "跨元素 `$ref` 的文件部分 `{file}` 解析不出目标元素（当前引用串 `{reference}`）\
                 ——MUST 是 `<相对前缀><目标叶段>.schema.json`，前缀由 \
                 `ref_relative(本元素, 目标元素)` 逐段求出，MUST NOT 手算层数（用 \
                 `archi_api` 的 `link` 算）"
            )));
        };
        let expected_file = format!(
            "{}{}{SCHEMA_FILE_SUFFIX}",
            paths::ref_relative(fqn, &target),
            paths::leaf_name(&target)
        );
        if file == expected_file {
            continue;
        }
        return Err(ApiError::Contract(format!(
            "跨元素 `$ref` 的前缀非法：当前串 `{reference}`，期望 `{expected_file}#/$defs/<名>`\
             ——前缀由 `ref_relative(本元素, 目标元素)` 逐段求出，MUST NOT 手算层数（用 \
             `archi_api` 的 `link` 算）"
        )));
    }
    Ok(())
}

/// 闸 ⑧：顶层键**封闭六项**（`$schema` / `$id` / `title` / `$comment` / `$defs` /
/// `properties`），其中 `$id` / `title` / `$defs` / `properties` 四键 MUST 在场。
///
/// 自造的 `x-` 前缀键在这一层就被拒：`x-` 面封闭为 `x-function` / `x-class` / `x-common` /
/// `x-constructor` / `x-static` 五项（逐名，MUST NOT 写「等」），且五项都只在条目内部
/// （不在顶层）。
pub fn verify_top_level_keys(value: &Value) -> Result<(), ApiError> {
    let Some(object) = value.as_object() else {
        return Err(ApiError::Contract(
            "顶层必须是 JSON 对象（接口定义以 `$defs` / `properties` 两表承载导出名）".to_owned(),
        ));
    };
    for key in object.keys() {
        if !TOP_LEVEL_KEYS.contains(&key.as_str()) {
            return Err(ApiError::Contract(format!(
                "顶层出现未知键 `{key}`——顶层封闭为 {}；`x-` 面封闭为 `{X_FUNCTION}` / \
                 `x-class` / `x-common` / `{X_CONSTRUCTOR}` / `{X_STATIC}` 五项，且只在条目内部",
                TOP_LEVEL_KEYS.join(" / ")
            )));
        }
    }
    for key in REQUIRED_TOP_LEVEL_KEYS {
        if !object.contains_key(key) {
            return Err(ApiError::Contract(format!(
                "缺顶层键 `{key}`——四键 MUST 在场：{}",
                REQUIRED_TOP_LEVEL_KEYS.join(" / ")
            )));
        }
    }
    for key in ["$defs", "properties"] {
        if !object.get(key).is_some_and(Value::is_object) {
            return Err(ApiError::Contract(format!(
                "顶层键 `{key}` 的值必须是对象（空面写 `{{}}`）"
            )));
        }
    }
    Ok(())
}

/// 顶层键封闭集（[`verify_top_level_keys`] 的唯一持有者）。
const TOP_LEVEL_KEYS: [&str; 6] = ["$schema", "$id", "title", "$comment", "$defs", "properties"];

/// 顶层必在场键（identity / 形态 / 镜像三闸的对象）。
const REQUIRED_TOP_LEVEL_KEYS: [&str; 4] = ["$id", "title", "$defs", "properties"];

/// 函数标记层关键字（单点定义在 [`super::contract`]，形态闸与键名摘要共用）。
const X_FUNCTION: &str = super::contract::X_FUNCTION;

/// 构造入口关键字（单点定义在 [`super::contract`]；形态与落点判据、两个渲染器同调）。
const X_CONSTRUCTOR: &str = super::contract::X_CONSTRUCTOR;

/// 静态方法表关键字（同上）。
const X_STATIC: &str = super::contract::X_STATIC;

/// `$defs` 表（缺省 / 非对象 → `None`）。
fn defs_of(value: &Value) -> Option<&Map<String, Value>> {
    value.get("$defs").and_then(Value::as_object)
}

/// `properties` 表（缺省 / 非对象 → `None`）。
fn properties_of(value: &Value) -> Option<&Map<String, Value>> {
    value.get("properties").and_then(Value::as_object)
}

/// 一棵 JSON 子树内所有 `$ref` 字符串（[`verify_ref_prefix`] 的收集面）。
fn collect_refs(value: &Value, references: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, inner) in object {
                if key == "$ref" {
                    if let Some(reference) = inner.as_str() {
                        references.push(reference.to_owned());
                        continue;
                    }
                }
                collect_refs(inner, references);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_refs(item, references);
            }
        }
        _ => {}
    }
}

/// 错误面统一补文件前缀（第一行自足：文件 + 期望值 + 出路）。
///
/// 带 fqn 的闸（identity / `$ref` 前缀）不必自带文件名，故这里统一补一次即可（本函数
/// 只看前缀是否已在，避免双写）。
fn locate(file: &str, error: ApiError) -> ApiError {
    let message = error.message();
    if message.starts_with(file) {
        return error;
    }
    ApiError::Contract(format!("{file}: {message}"))
}

/// `$defs` 五条规则 + 透传条目纯 `$ref` 校验 + `x-class` 的骨架 / 形态 / 落点判据：
///
/// 1. 身份 = 键名（JSON 对象天然键唯一）；
/// 2. 嵌套 `$defs` MUST 为空——名字提升后原名保留，MUST NOT 构成作用域；
/// 3. 所有 `$ref` target MUST 是顶层条目（`#/$defs/<名>`）或外部文件形式
///    （`<相对前缀><叶段>.schema.json#/$defs/<名>`）；
/// 4. 同名只定义一次；
/// 5. **MUST NOT 按结构去重**（本函数不做任何合并动作即满足；相等形状不合并）；
/// 6. 任意对象位的 `x-common.degraded` 取值 MUST ∈ 封闭四值词表 [`DEGRADED_KINDS`]
///    （**非字符串亦拒收**），错误点名该标记的 JSON pointer 并列四值；`x-common.note`
///    不受校验、不进债表（自由注记）。
///
/// `x-class: true` 的类型是 trait，**仅允许方法**，且骨架与行为面分两处声明。判序固定
/// （判序即错误面）：
///
/// ① 条目缺 `type: "object"`（骨架不在场：标准校验器视作任意值，且 `type: "string"` 一类会被
/// 静默吞成 trait）→ 错误；
/// ② 条目带 `additionalProperties`（对象 / 布尔形态同拒：字典断言与「仅允许方法的 trait」
/// 语义互斥，放行即静默丢，且会反噬出 `impl <trait>`）→ 错误；
/// ③ 条目带 `required` → 错误（既有）；
/// ④ 某个 `properties` 值不是 `x-function`（声明了数据属性）→ 错误（既有）；
/// ⑤ `x-constructor` / `x-static` 的形态（[`verify_class_members`]）。
///
/// 逐条目循环之后另有**两个新键的落点单趟**（[`verify_class_key_positions`]），最后是降级
/// 词表扫描。全部拦在装载期（[`super::load_contracts`] 随即中止、不产出半真报告）；
/// 反方向的「`properties` 全为函数却未标 `x-class`」只是意图提示（警告），落
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
        // `x-class: true` 的 trait **仅允许方法**，且骨架与行为面分两处声明。判序固定（判序即
        // 错误面）：`type: "object"` → 无 `additionalProperties` → 无 `required` →
        // `properties` 全为方法 → 两键形态。①② 两条排在既有两条**之前**——「你不是 object」
        // 与「你是字典」都比「你不能有 required」更根本；排在嵌套 `$defs` 与 `$ref` 两条之后
        // （外层规则优先，MUST NOT 遮蔽既有五条 `$defs` 规则）。
        if definition.get("x-class").and_then(Value::as_bool) == Some(true) {
            if definition.get("type").and_then(Value::as_str) != Some("object") {
                let actual = match definition.get("type") {
                    Some(Value::String(text)) => format!("`{text}`"),
                    Some(other) => format!("{} `{other}`", kind_of(other)),
                    None => "缺省".to_owned(),
                };
                return Err(ApiError::Contract(format!(
                    "`$defs.{name}.type` 必须是 `\"object\"`（实际为 {actual}）——`x-class` 声明\
                     行为面，骨架（标准 JSON Schema 能看见的形状）MUST 同时在场：补 \
                     `\"type\": \"object\"` 即通过（不写 `type` 的条目在标准校验器里是「任意值」，\
                     而 `type: \"string\"` 一类会被静默吞成 trait）"
                )));
            }
            if definition.contains_key("additionalProperties") {
                return Err(ApiError::Contract(format!(
                    "`$defs.{name}` 标了 `x-class` 却带 `additionalProperties`——它断言「键任意、\
                     值同型、无行为」，与「仅允许方法的 trait」语义互斥，放行即静默丢。两条出路：\
                     ① 去掉 `x-class` 把它变纯数据（注意：条目层的 map 形态本期不渲染，键值对容器\
                     写在值位——`x-function.params[].schema` / `returns` / 字段位）；\
                     ② 若本意是「方法集合」，去掉 `additionalProperties`——trait 没有实例数据面"
                )));
            }
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
            verify_class_members(name, definition)?;
        }
        collect_nested_refs(name, definition)?;
    }
    // 两个新键的落点（全文档单趟；排在逐条目循环之后 ⇒ 外层规则先命中，`degraded` 词表仍在最后）
    verify_class_key_positions(schema)?;
    // 降级词表（与计数同源的 `degraded_marks` 单点扫描）：取值 MUST ∈ 四值词表，非字符串
    // 亦拒收——`true` 不携带任何原因信息，债表不可读。错误面沿用本函数既有形态
    // （`$defs.<名>…` 的 pointer 文案，签名只有 schema）。
    for (pointer, mark) in degraded_marks(schema) {
        if mark
            .as_str()
            .is_some_and(|kind| DEGRADED_KINDS.contains(&kind))
        {
            continue;
        }
        let shown = match mark.as_str() {
            Some(kind) => format!("`{kind}`"),
            None => format!("{} `{mark}`", kind_of(mark)),
        };
        return Err(ApiError::Contract(format!(
            "`{pointer}` 的取值必须是降级词表内的字符串（实际为 {shown}）——词表封闭为 {}；\
             非字符串与词表外值都让债表不可读（`true` 不携带任何原因信息），\
             新增第五值 MUST 改「降级词表与真实债表」",
            DEGRADED_KINDS.join(" / ")
        )));
    }
    Ok(())
}

/// `x-class` 条目的两个行为面键的**形态**判点（单个对象 + 对象型 `x-function` + 禁项）。
///
/// | 键 | 外层 | 允许的内层 | 禁项 |
/// |---|---|---|---|
/// | `x-constructor` | **单个对象**（不是「名字 → 签名」表） | `x-function: { params?, … }` | `returns` MUST NOT 出现（返回面由语义固定为本类型）；`async` MUST NOT 为 `true` |
/// | `x-static` | 「名字 → `{ "x-function": { … } }`」对象 | `params` / `returns` / `async` / `description`（自由形状） | 成员缺对象型 `x-function`（缺包裹即静默丢） |
///
/// `params` 可省（= 零参）；**外层另加键不判**（沿用「`x-function` 内层自由形状、不设未知键闸」
/// 的既有纪律）。名字合法性 / 保留名不在这里——那是 [`super::contract::verify`] 的名字族与
/// 预留名判据（名字错与形态错分属两个函数，文案互不吞并）。
fn verify_class_members(name: &str, definition: &Map<String, Value>) -> Result<(), ApiError> {
    if let Some(constructor) = definition.get(X_CONSTRUCTOR) {
        let Some(constructor) = constructor.as_object() else {
            return Err(ApiError::Contract(format!(
                "`$defs.{name}.{X_CONSTRUCTOR}` 的值必须是对象（实际为 {}）——构造入口是**单个** \
                 `{{ \"x-function\": {{ \"params\": [ … ] }} }}`（不是「名字 → 签名」表；名字固定 \
                 `new`，别名构造写给 `{X_STATIC}` 并返回本类型）",
                kind_of(constructor)
            )));
        };
        let Some(function) = constructor.get(X_FUNCTION).and_then(Value::as_object) else {
            let actual = match constructor.get(X_FUNCTION) {
                Some(value) => format!("{} `{value}`", kind_of(value)),
                None => "缺省".to_owned(),
            };
            return Err(ApiError::Contract(format!(
                "`$defs.{name}.{X_CONSTRUCTOR}` 缺对象型 `{X_FUNCTION}`（实际为 {actual}）——\
                 构造入口的入参表写在 `{{ \"x-function\": {{ \"params\": [ … ] }} }}` 内\
                 （`params` 可省 = 零参）；直接写 `params` 会被静默丢"
            )));
        };
        if function.contains_key("returns") {
            return Err(ApiError::Contract(format!(
                "`$defs.{name}.{X_CONSTRUCTOR}.{X_FUNCTION}` 带 `returns`——构造器的返回面由语义\
                 固定为本类型（MUST NOT 声明 `returns`）；要别名构造（`open` / `from_path`）就写给 \
                 `{X_STATIC}` 并返回本类型"
            )));
        }
        if function.get("async").and_then(Value::as_bool) == Some(true) {
            return Err(ApiError::Contract(format!(
                "`$defs.{name}.{X_CONSTRUCTOR}.{X_FUNCTION}` 带 `\"async\": true`——构造器是个值\
                 而非 future（两语言都没有异步构造，放行只能静默丢该标志）；去掉该键，\
                 异步初始化写给 `{X_STATIC}` 并返回本类型"
            )));
        }
    }
    let Some(statics) = definition.get(X_STATIC) else {
        return Ok(());
    };
    let Some(statics) = statics.as_object() else {
        return Err(ApiError::Contract(format!(
            "`$defs.{name}.{X_STATIC}` 的值必须是对象（实际为 {}）——它是「名字 → \
             `{{ \"x-function\": {{ … }} }}`」的类型级静态方法表",
            kind_of(statics)
        )));
    };
    for (member, value) in statics {
        let Some(item) = value.as_object() else {
            return Err(ApiError::Contract(format!(
                "`$defs.{name}.{X_STATIC}.{member}` 的值必须是对象（实际为 {}）——每个静态成员 \
                 MUST 用 `{{ \"x-function\": {{ … }} }}` 包裹",
                kind_of(value)
            )));
        };
        if item.get(X_FUNCTION).is_some_and(Value::is_object) {
            continue;
        }
        let actual = match item.get(X_FUNCTION) {
            Some(value) => format!("{} `{value}`", kind_of(value)),
            None => "缺省".to_owned(),
        };
        return Err(ApiError::Contract(format!(
            "`$defs.{name}.{X_STATIC}.{member}` 缺对象型 `{X_FUNCTION}`（实际为 {actual}）——\
             每个静态成员的签名 MUST 用 `{{ \"x-function\": {{ … }} }}` 包裹（缺包裹即静默丢）"
        )));
    }
    Ok(())
}

/// 两个新键的**落点**单趟（全文档、含递归）：合法 pointer 集合恰为「`$defs.<T>` 且该条目
/// `x-class: true` 的顶层」。
///
/// 覆盖四种非法位：非类 `$defs` 条目（object / enum / 标量 / 纯 `$ref` alias）的顶层、条目内
/// 更深层、模块 `properties` 子树、`x-function` 内层——后者是 `x-function` 内层**自由形状**里
/// 唯一被禁的两个**协议词汇**（禁的是词，不是未知键闸）。顶层与 `properties.<k>` 两处仍由既有
/// 闸 ⑧ / ⑤ 先拒（本函数结构上排在它们之后）。
///
/// 排在逐条目循环之后、`degraded` 词表扫描之前：外层 `$defs` 规则先命中，降级词表仍最后。
fn verify_class_key_positions(schema: &Value) -> Result<(), ApiError> {
    scan_class_key_positions(schema, "$", false)
}

/// [`verify_class_key_positions`] 的递归体：`class_top` 只在该 `$defs.<T>` 条目顶层为真。
fn scan_class_key_positions(value: &Value, pointer: &str, class_top: bool) -> Result<(), ApiError> {
    match value {
        Value::Object(object) => {
            for (key, inner) in object {
                if !class_top && (key == X_CONSTRUCTOR || key == X_STATIC) {
                    return Err(ApiError::Contract(format!(
                        "`{pointer}.{key}` 的落点非法——只有 `x-class` 条目可声明构造入口与静态\
                         方法（`{X_CONSTRUCTOR}` / `{X_STATIC}` 只许出现在 `$defs.<T>` 且该条目 \
                         `x-class: true` 的顶层；`object` / `enum` / 标量 / 纯 `$ref` alias 都是\
                         纯数据或非类形态，实例由 `properties` + `required` 的字面量构造）。\
                         出路：把该键移到该类条目的顶层；若本意是给纯数据类型声明行为，先补 \
                         `\"type\": \"object\"` 与 `\"x-class\": true`（类与数据的分界 = 有没有方法）"
                    )));
                }
                if pointer == "$" && key == "$defs" {
                    if let Some(defs) = inner.as_object() {
                        for (name, definition) in defs {
                            let nested_pointer = format!("$defs.{name}");
                            // 类条目判据的严格布尔形态（字符串 `"true"` 不算——那种写法走
                            // 「未标 class」的既有警告面，不是错误面）
                            let class_top =
                                definition.get("x-class").and_then(Value::as_bool) == Some(true);
                            scan_class_key_positions(definition, &nested_pointer, class_top)?;
                        }
                        continue;
                    }
                }
                scan_class_key_positions(inner, &format!("{pointer}.{key}"), false)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                scan_class_key_positions(item, &format!("{pointer}[{index}]"), false)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// `INDEX.toml` 文本装配：顶部说明段（生成警示 / review 口径 / 生成器标识）+
/// 按 FQN 字典序的条目。
///
/// 说明段承载两件事：**手写 / 生成分界**（`<叶段>.schema.json` 与 `<叶段>.md` 是手写资产，
/// 本文件与 `.i.*` 是生成物）与 P6 的命名债注解（`status` 的 `declared` = 接口文件在案，
/// 值域不动）。
pub fn build_index(entries: &[IndexEntry], meta: &ApiMeta) -> String {
    let mut ordered: Vec<&IndexEntry> = entries.iter().collect();
    ordered.sort_by(|left, right| left.fqn.cmp(&right.fqn));

    let mut document = DocumentMut::new();
    // 说明段取契约资产所在目录，MUST NOT 取索引自身目录（索引与契约已分家）。
    let contract_dir = foundation::layout::archi_models_rel_dir();
    let digest = short_digest(&meta.common_digest);
    let header = format!(
        "由 archi-api-gen 生成 · 勿手改\n\
         review 口径：读 `<叶段>.schema.json`（接口，手写真相）与 `<叶段>.md`（理由；同目录、叶段即 FQN 末段）；status 的 declared = 接口文件在案；本文件与 `*.i.*` 的 diff 视为机械噪声。\n\
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

/// 顶层 `$defs` 引用的形态判据（`#/$defs/<名>`）。
fn is_top_level_ref(reference: &str) -> bool {
    reference
        .strip_prefix(DEFS_FRAGMENT)
        .is_some_and(|name| !name.is_empty() && !name.contains('/'))
}

/// 外部契约文件引用（`<相对前缀><叶段>.schema.json#/$defs/<名>`）。
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
