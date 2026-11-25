//! 写面反馈判据：把既有判据面的五个词在写面重述为「已落盘、未阻断」的事实。
//!
//! 这组事实**不引入新判据、新词汇或新档位**：`kind` 取自 [`ArchiDriftKind::as_str`] /
//! [`ArchiApiFindingKind::as_str`]，`severity` 取自 [`ArchiApiFindingKind::severity`]
//! （drift 侧两值恒落报告的 `warnings` 桶 ⇒ 恒 `warn`），路径取自
//! [`crate::api::paths::contract_path`]（文档）与 [`crate::api::paths::schema_path`]
//! （接口）。
//!
//! **零 `drift::check`、零 scan、零第二套模型**：推导只用「写工具已持有的字段值 +
//! `Path::is_file` / `exists`」两类零成本事实（写是高频动作）。
//!
//! **降噪不变式**：`action = upsert` 时 `kind` 集合 ⊆ 由 `changed` 诱发的集合
//! （`created` 视为全字段）——`noPath` / `pathNotFound` 只由 `path`（或 `kind`）诱发；
//! `apiUnexpected` 只由 `api` 诱发；`docMissing` 只由 `created` 诱发（派生 md 的存在性与
//! 字段值无关）；**只改 `summary` / `gate` / `metadata` / `doc` ⇒ 恒空**。删除动作只允许
//! 孤儿接口文件一条（由动作本身解释）。
//!
//! **`doc` 字段零读者**：它的任何取值都不产 warning（三条建在 `doc` 上的旧事实随
//! `DocNotFound` 判据值一并删除）。

use std::path::Path;

use serde::Serialize;

use crate::api::paths::{checked_root, contract_path, schema_path};
use crate::drift::report::{ApiFindingSeverity, ArchiApiFindingKind, ArchiDriftKind};
use crate::dto::{ApiStatus, ElementKind};
use crate::model::Element;

/// 一条写面 warning：判据词 + 档位词 + 判据句与下一步。
///
/// `kind` / `severity` 存既有单点的 `as_str()` 返回值——字面量持有者仍是那两个枚举与
/// [`ArchiApiFindingKind::severity`]，写面 MUST NOT 新增第三份词表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditWarning {
    /// 判据词（`ArchiDriftKind` / `ArchiApiFindingKind` 的线格式词）
    pub kind: &'static str,
    /// 档位词（`warn` / `error`）
    pub severity: &'static str,
    /// 判据句 + 事实 + 下一步（不重复 `kind` 字面量）
    pub message: String,
}

/// 元素写回后的 warnings（五条 upsert 判据 + 降噪过滤）。
///
/// `element` 取**写后文件的重解析结果**（写面事实的唯一来源）；`changed` 是该次写回的
/// 参数面字段名子集，`created` 为真时视为全字段变化。
pub fn upsert_warnings(
    root: &Path,
    element: &Element,
    changed: &[String],
    created: bool,
) -> Vec<EditWarning> {
    let path_induced = created
        || changed
            .iter()
            .any(|field| field == "path" || field == "kind");
    let api_induced = created || changed.iter().any(|field| field == "api");
    let mut warnings: Vec<EditWarning> = Vec::new();
    if path_induced {
        path_warnings(root, element, &mut warnings);
    }
    if created {
        doc_warnings(root, element, &mut warnings);
    }
    if api_induced {
        api_warnings(root, element, &mut warnings);
    }
    warnings
}

/// 删除成功后的 warnings：只剩「孤儿接口文件」一条（由动作本身解释，不要求 `changed`）。
pub fn delete_warnings(root: &Path, fqn: &str) -> Vec<EditWarning> {
    if !schema_on_disk(root, fqn) {
        return Vec::new();
    }
    vec![warning(
        ArchiApiFindingKind::ApiUnexpected.as_str(),
        contract_severity(ArchiApiFindingKind::ApiUnexpected),
        format!(
            "{fqn} 已删除但接口定义文件仍在盘：{}——一并清理（否则无人看管）",
            schema_relative(root, fqn)
        ),
    )]
}

/// 第 1、2 条：`noPath`（纯分组容器误当 `package` / `module`）与 `pathNotFound`
/// （逐缺席项一条，与 `drift::check` 的「每条失效 path 各一条」同形）。
fn path_warnings(root: &Path, element: &Element, warnings: &mut Vec<EditWarning>) {
    let container = matches!(element.kind, ElementKind::Package | ElementKind::Module);
    if container && element.path.is_empty() {
        warnings.push(warning(
            ArchiDriftKind::NoPath.as_str(),
            drift_severity(),
            format!(
                "{} 是 {} 但 path 为空——纯分组容器请改 composite",
                element.fqn,
                element.kind.as_str()
            ),
        ));
    }
    for path in &element.path {
        if path_on_disk(root, path) {
            continue;
        }
        warnings.push(warning(
            ArchiDriftKind::PathNotFound.as_str(),
            drift_severity(),
            format!("{} 的 path 不在盘：{path}——先行声明可忽略", element.fqn),
        ));
    }
}

/// 第 3 条：`created` 且派生 `<FQN>.md` 不在盘（**只由 `created` 诱发**，与 `api` 无关）。
///
/// 与判据面的 `docMissing` 同义（单成因：派生 md 不在盘）；只由 `created` 诱发是为了避免
/// 既有元素的每次 upsert 都复读一条存量债（写面是高频动作）。
fn doc_warnings(root: &Path, element: &Element, warnings: &mut Vec<EditWarning>) {
    if contract_on_disk(root, &element.fqn) {
        return;
    }
    warnings.push(warning(
        ArchiApiFindingKind::DocMissing.as_str(),
        contract_severity(ArchiApiFindingKind::DocMissing),
        format!(
            "{} 已创建但派生文档 {} 不在盘——新建元素待补文档：理由写 `.md`、接口写 `.schema.json`（`bootstrap` 取双草稿）",
            element.fqn,
            contract_relative(root, &element.fqn)
        ),
    ));
}

/// 第 4 条：`api = none` 但 `<FQN>.schema.json` 已在盘（留白被违反）。
fn api_warnings(root: &Path, element: &Element, warnings: &mut Vec<EditWarning>) {
    if element.api != ApiStatus::None || !schema_on_disk(root, &element.fqn) {
        return;
    }
    warnings.push(warning(
        ArchiApiFindingKind::ApiUnexpected.as_str(),
        contract_severity(ArchiApiFindingKind::ApiUnexpected),
        format!(
            "{} 声明无接口面但接口定义在盘：{}——删该 schema 或改 declared",
            element.fqn,
            schema_relative(root, &element.fqn)
        ),
    ));
}

/// 一条 warning 的装配：`kind` / `severity` 由调用方从既有两个单点取来（写面零新词表）。
fn warning(kind: &'static str, severity: &'static str, message: String) -> EditWarning {
    EditWarning {
        kind,
        severity,
        message,
    }
}

/// drift 侧两值恒落报告的 `warnings` 桶（`desktop-archi-drift` 的桶定义背书）⇒ 写面恒
/// `warn`；档位词仍复用 [`ApiFindingSeverity`] 一处，不另立档位表。
fn drift_severity() -> &'static str {
    ApiFindingSeverity::Warn.as_str()
}

/// 接口侧档位（取 [`ArchiApiFindingKind::severity`] 单点）。
fn contract_severity(kind: ArchiApiFindingKind) -> &'static str {
    kind.severity().as_str()
}

/// 路径类字段（模型 `path` 条目）是否在盘：归一与越根判据归
/// [`checked_root`]（同一单点，写面 MUST NOT 自拼路径）。
fn path_on_disk(root: &Path, path: &str) -> bool {
    checked_root(path).is_some_and(|relative| root.join(relative).exists())
}

/// 派生 `<FQN>.md` 是否在盘（形态非法的 FQN 一律按「不在盘」处理——它派生不出路径）。
fn contract_on_disk(root: &Path, fqn: &str) -> bool {
    contract_path(root, fqn).is_some_and(|path| path.is_file())
}

/// `<FQN>.schema.json` 是否在盘（形态非法的 FQN 同样按「不在盘」处理）。
fn schema_on_disk(root: &Path, fqn: &str) -> bool {
    schema_path(root, fqn).is_some_and(|path| path.is_file())
}

/// 文档的 repo 相对展示串（派生不出路径时给一句成因，不静默留空）。
fn contract_relative(root: &Path, fqn: &str) -> String {
    contract_path(root, fqn)
        .map(|path| crate::api::display(root, &path))
        .unwrap_or_else(|| "（文档路径不可派生）".to_owned())
}

/// 接口文件的 repo 相对展示串（同上）。
fn schema_relative(root: &Path, fqn: &str) -> String {
    schema_path(root, fqn)
        .map(|path| crate::api::display(root, &path))
        .unwrap_or_else(|| "（接口路径不可派生）".to_owned())
}
