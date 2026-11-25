//! 写面反馈判据：把既有判据面的七个词在写面重述为「已落盘、未阻断」的事实。
//!
//! 这组事实**不引入新判据、新词汇或新档位**：`kind` 取自 [`ArchiDriftKind::as_str`] /
//! [`ArchiApiFindingKind::as_str`]，`severity` 取自 [`ArchiApiFindingKind::severity`]
//! （drift 侧两值恒落报告的 `warnings` 桶 ⇒ 恒 `warn`），路径取自
//! [`crate::api::paths::contract_path`] / [`crate::api::paths::doc_path`]。
//!
//! **零 `drift::check`、零 scan、零第二套模型**：推导只用「写工具已持有的字段值 +
//! `Path::is_file` / `exists`」两类零成本事实（写是高频动作）。
//!
//! **降噪不变式**：`action = upsert` 时 `kind` 集合 ⊆ 由 `changed` 诱发的集合
//! （`created` 视为全字段）——`noPath` / `pathNotFound` 只由 `path`（或 `kind`）诱发，
//! 契约三条只由 `api` 或 `doc` 诱发；只改 `summary` / `gate` / `metadata` ⇒ 恒空。
//! 删除动作只允许孤儿契约 md 一条（由动作本身解释）。

use std::path::Path;

use serde::Serialize;

use crate::api::paths::{contract_path, doc_path};
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

/// 元素写回后的 warnings（六条 upsert 判据 + 降噪过滤）。
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
    let api_induced = created || changed.iter().any(|field| field == "api" || field == "doc");
    let mut warnings: Vec<EditWarning> = Vec::new();
    if path_induced {
        path_warnings(root, element, &mut warnings);
    }
    if api_induced {
        api_warnings(root, element, &mut warnings);
    }
    warnings
}

/// 删除成功后的 warnings：只剩「孤儿契约 md」一条（由动作本身解释，不要求 `changed`）。
pub fn delete_warnings(root: &Path, fqn: &str) -> Vec<EditWarning> {
    if !contract_on_disk(root, fqn) {
        return Vec::new();
    }
    vec![warning(
        ArchiApiFindingKind::ApiUnexpected.as_str(),
        contract_severity(ArchiApiFindingKind::ApiUnexpected),
        format!(
            "{fqn} 已删除但契约 md 仍在盘：{}——一并清理（否则无人看管）",
            contract_relative(root, fqn)
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

/// 第 3–6 条：契约面三判据（`api = declared` / `api = none` 两支各一条 `docMissing`
/// 与一条 `apiUnexpected`，`none` 支另有 `docNotFound`）。
fn api_warnings(root: &Path, element: &Element, warnings: &mut Vec<EditWarning>) {
    match element.api {
        ApiStatus::Declared => {
            if !contract_on_disk(root, &element.fqn) {
                warnings.push(warning(
                    ArchiApiFindingKind::DocMissing.as_str(),
                    contract_severity(ArchiApiFindingKind::DocMissing),
                    format!(
                        "{} 承诺契约但 {} 不存在——用 archi_api 的 bootstrap 取草稿",
                        element.fqn,
                        contract_relative(root, &element.fqn)
                    ),
                ));
            }
        }
        ApiStatus::None => {
            match element.doc.as_deref() {
                None => warnings.push(warning(
                    ArchiApiFindingKind::DocMissing.as_str(),
                    contract_severity(ArchiApiFindingKind::DocMissing),
                    format!("{} 声明无 API 但未写 doc——入口文档经 doc 声明", element.fqn),
                )),
                Some(doc) => {
                    let present = doc_path(root, doc).is_some_and(|path| path.is_file());
                    if !present {
                        warnings.push(warning(
                            ArchiApiFindingKind::DocNotFound.as_str(),
                            contract_severity(ArchiApiFindingKind::DocNotFound),
                            format!(
                                "{} 的 doc（{doc}）指不到盘——先建文件或改 declared",
                                element.fqn
                            ),
                        ));
                    }
                }
            }
            if contract_on_disk(root, &element.fqn) {
                warnings.push(warning(
                    ArchiApiFindingKind::ApiUnexpected.as_str(),
                    contract_severity(ArchiApiFindingKind::ApiUnexpected),
                    format!(
                        "{} 声明无 API 但契约 md 在盘：{}——删契约 md 或改 declared",
                        element.fqn,
                        contract_relative(root, &element.fqn)
                    ),
                ));
            }
        }
    }
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

/// 契约侧档位（取 [`ArchiApiFindingKind::severity`] 单点）。
fn contract_severity(kind: ArchiApiFindingKind) -> &'static str {
    kind.severity().as_str()
}

/// 路径类字段（模型 `path` 条目）是否在盘：repo 相对串的归一与越根判据归
/// [`doc_path`]（同一单点，写面 MUST NOT 自拼路径）。
fn path_on_disk(root: &Path, path: &str) -> bool {
    doc_path(root, path).is_some_and(|resolved| resolved.exists())
}

/// 派生契约 md 是否在盘（形态非法的 FQN 一律按「不在盘」处理——它派生不出路径）。
fn contract_on_disk(root: &Path, fqn: &str) -> bool {
    contract_path(root, fqn).is_some_and(|path| path.is_file())
}

/// 契约 md 的 repo 相对展示串（派生不出路径时给一句成因，不静默留空）。
fn contract_relative(root: &Path, fqn: &str) -> String {
    contract_path(root, fqn)
        .map(|path| crate::api::display(root, &path))
        .unwrap_or_else(|| "（契约路径不可派生）".to_owned())
}
