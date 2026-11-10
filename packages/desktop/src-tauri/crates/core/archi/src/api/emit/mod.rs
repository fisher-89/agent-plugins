//! 生成物渲染的语言分派（单契约粒度）：`EmitTarget.kind` → [`rust`] / [`ts`]。
//!
//! **载元类型只有一个** [`super::ApiMeta`]（`schema` / `emit` / 索引三处共用）：本层
//! 只引用，MUST NOT 另立 `EmitMeta`。批量入口在 [`super::render_all`]（含索引与
//! `schema.json`），本层只管一个契约的一份生成物文本。
//!
//! **泛型零支持**：契约写泛型参数即契约错误——[`render_target`] 在分派前扫 `$defs`
//! 与类型名，命中即 `ApiError::Contract`，MUST NOT 产出 `<T>` 形态文本。
//!
//! **`x-class` 反向提示只在本处分派处发**：[`render_target`] 在分派前扫 `$defs` 本地
//! 条目，`properties` 非空且全为 `x-function` 却未标 `x-class` ⇒ 一行 `eprintln!` 警告
//! （每元素恰一次；两侧渲染器各报即双报，故 [`rust`] / [`ts`] 保持纯函数出 `String`、
//! 零 stderr）。正方向的「`x-class` 带属性即契约错误」由
//! [`super::schema::verify_defs`] 拦在装载期。两者都不进 `ArchiApiFindingKind` /
//! `apiFindings`，也不改 `--check` 退出码。
//!
//! **形态与透传命名由调用方给**（D40 / D41）：[`EmitTarget::form`] 来自
//! [`super::paths::entry_form`]（判据是「盘上命中哪个入口文件」），[`EmitTarget::modules`]
//! 是「目标元素 FQN → [`ModuleRef`]」的查表结果（两条值分别经
//! [`super::paths::module_name`] 与 [`super::paths::rust_module_path`] 产出）。渲染器只读
//! 这两个字段、零派生、零读模型——MUST NOT 出现第二套形态判据（如「末段是否含 `.`」）
//! 或第二套命名（如从 FQN 末段派生）。
//!
//! 边界：纯函数出 `String`，**零写盘**（写盘在 `crates/tools/archi-api-gen`）。

pub mod rust;
pub mod ts;

use std::collections::BTreeMap;

use serde_json::Value;

use super::contract::Contract;
use super::paths::{EntryForm, GeneratedKind};
use super::{short_digest, ApiError, ApiMeta};

/// 单份生成物的渲染目标（语言 + 入口形态 + 透传查表）。
///
/// **形态与透传命名由调用方给**（D40 / D41）：渲染器只读字段、零派生、零读模型。
/// 落点路径与元素根**不是渲染入参**（D58）：生成物落点由调用方直接写进
/// [`super::GeneratedFile`]，TS specifier 只由 [`EmitTarget::form`] 与
/// [`ModuleRef::name`] 决定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitTarget {
    /// 生成物语言（决定渲染器）
    pub kind: GeneratedKind,
    /// 入口形态（调用方经 [`super::paths::entry_form`] 求出；渲染器零形态推导）
    pub form: EntryForm,
    /// 透传目标查表（键 = 目标元素 FQN；调用方从元素表逐条解析）
    pub modules: BTreeMap<String, ModuleRef>,
}

/// 一条透传目标的两个事实（D40 的唯一承载形态）：TS 侧读 [`ModuleRef::name`]、
/// Rust 侧读 [`ModuleRef::module_path`]，两侧互不派生。
///
/// **MUST NOT 合并为单字符串**：`name`（目标元素根路径末段）与 `module_path`（该根的
/// `crate::` 形态）是两个不同事实，塞进一个字符串就得让渲染器再切一次——那正是
/// D40 要消灭的二次派生。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRef {
    /// 目标元素根路径末段（由 [`super::paths::module_name`] 产出）
    pub name: String,
    /// 目标元素根的 Rust 模块路径（由 [`super::paths::rust_module_path`] 产出）
    pub module_path: String,
}

/// 按语言分派渲染（单契约粒度；出文本、零写盘）。
pub fn render_target(
    target: &EmitTarget,
    contract: &Contract,
    schema: &Value,
    meta: &ApiMeta,
) -> Result<String, ApiError> {
    reject_generics(contract, schema)?;
    warn_missing_x_class(contract, schema);
    Ok(match target.kind {
        GeneratedKind::Rust => rust::render(contract, schema, meta, target),
        GeneratedKind::TypeScript => ts::render(contract, schema, meta, target),
    })
}

/// 「`properties` 全为函数却未标 `x-class`」的提示（**警告，非错误**）。
///
/// `x-class` 消灭「形状推导误判 trait」的静默故障，但函数指针结构体是**合法意图**，
/// 不能一刀切拒绝——故按 spec 产警告：单点在语言分派处（每元素恰一次），不进报告面、
/// 不改 `--check` 退出码。
fn warn_missing_x_class(contract: &Contract, schema: &Value) {
    for (name, definition) in local_defs(schema) {
        let Some(object) = definition.as_object() else {
            continue;
        };
        if object.get("x-class").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let Some(properties) = object.get("properties").and_then(Value::as_object) else {
            continue;
        };
        if properties.is_empty()
            || !properties
                .values()
                .all(|property| property.get("x-function").is_some())
        {
            continue;
        }
        eprintln!(
            "警告：{}: 类型 {} 的 properties 全为函数却未标 x-class —— 若本意是 trait，请加 \"x-class\": true",
            contract.file, name
        );
    }
}

/// 泛型零支持判据：`$defs` 值含 `typeParameters` / `typeArguments`，或类型名带
/// `<`…`>` 形态，即契约错误并点名该类型。
fn reject_generics(contract: &Contract, schema: &Value) -> Result<(), ApiError> {
    let Some(defs) = schema.get("$defs").and_then(Value::as_object) else {
        return Ok(());
    };
    for (name, definition) in defs {
        if name.contains('<') || name.contains('>') {
            return Err(ApiError::Contract(format!(
                "{}: 类型名 `{name}` 含泛型参数——泛型零支持（契约 MUST NOT 声明泛型）",
                contract.file
            )));
        }
        if let Some(definition) = definition.as_object() {
            for marker in ["typeParameters", "typeArguments"] {
                if definition.contains_key(marker) {
                    return Err(ApiError::Contract(format!(
                        "{}: 类型 `{name}` 含 `{marker}`——泛型零支持（契约 MUST NOT 声明泛型）",
                        contract.file
                    )));
                }
            }
        }
    }
    Ok(())
}

/// 契约里的操作清单**剔除构造器**（构造器由 `$defs` 条目自动产出，MUST NOT 由契约
/// 声明，也不在本层的产物里重复输出）。
pub(crate) fn declared_ops(contract: &Contract) -> Vec<&super::contract::ContractOp> {
    contract
        .ops
        .iter()
        .filter(|op| !super::contract::is_constructor(&op.id))
        .collect()
}

/// `$defs` 的透传条目（纯 `$ref` alias）与本地定义分流：返回（名字，目标引用）。
pub(crate) fn reexport_defs(schema: &Value) -> Vec<(String, String)> {
    let Some(defs) = schema.get("$defs").and_then(Value::as_object) else {
        return Vec::new();
    };
    defs.iter()
        .filter_map(|(name, definition)| {
            let object = definition.as_object()?;
            let reference = object.get("$ref")?.as_str()?;
            Some((name.clone(), reference.to_owned()))
        })
        .collect()
}

/// 本地定义（非纯 `$ref` 条目）的名字清单（`$defs` 声明序，零排序、零合并）。
pub(crate) fn local_defs(schema: &Value) -> Vec<(String, Value)> {
    let Some(defs) = schema.get("$defs").and_then(Value::as_object) else {
        return Vec::new();
    };
    defs.iter()
        .filter(|(_, definition)| {
            definition
                .as_object()
                .map(|object| !object.contains_key("$ref"))
                .unwrap_or(true)
        })
        .map(|(name, definition)| (name.clone(), definition.clone()))
        .collect()
}

/// 头注释块（`.i.rs` / `.i.ts` 共用文案；语言注释前缀由各渲染器给）。
pub(crate) fn notice_lines(meta: &ApiMeta) -> Vec<String> {
    vec![
        format!(
            "生成于 archi-api {} · common.json@{} —— 勿手改",
            meta.tool_version,
            short_digest(&meta.common_digest)
        ),
        "手改将在下次 generate 时被覆盖，且 api:check 零 diff 门会失败。".to_owned(),
        "类型定义与自动构造器由契约派发；impl 块写在本模块任意手写文件。".to_owned(),
        meta.generated_notice.to_owned(),
    ]
}

#[cfg(test)]
mod mod_test;
#[cfg(test)]
mod rust_test;
#[cfg(test)]
mod ts_test;
