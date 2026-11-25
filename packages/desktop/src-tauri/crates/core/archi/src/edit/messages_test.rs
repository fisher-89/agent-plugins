//! `messages` 的单元测试：`EditErrorKind::ALL` ↔ `criterion()` 的穷尽对拍、23 条判据句
//! 文案表（逐字）、`message()` 的「判据句：事实」形态与 facts 边界，以及 `EditError`
//! 的两字段可比性与「只有一条渲染出口」。
//!
//! 零 mock：纯数据面（枚举 + 两字段结构体），零 IO 零进程边界。构造单点走
//! `super::messages::error`（`pub(super)`，全 crate 唯一的 `EditError` 拼装入口，
//! 故不为测试新增导出）。

use std::collections::BTreeSet;

use super::messages::error;
use super::{EditError, EditErrorKind};

/// 判据变体的 ASCII 名：穷尽 `match`，新增变体而未在本处补分支即编译失败
/// （编译期守卫，不复述判据句文案）。
fn variant(kind: EditErrorKind) -> &'static str {
    match kind {
        EditErrorKind::ModelHasDiagnostics => "ModelHasDiagnostics",
        EditErrorKind::ElementNotFound => "ElementNotFound",
        EditErrorKind::ElementNotTable => "ElementNotTable",
        EditErrorKind::ParentMissing => "ParentMissing",
        EditErrorKind::RelationEndpointMissing => "RelationEndpointMissing",
        EditErrorKind::TargetInvalid => "TargetInvalid",
        EditErrorKind::ElementKindMissing => "ElementKindMissing",
        EditErrorKind::ElementAlreadyExists => "ElementAlreadyExists",
        EditErrorKind::FqnInvalid => "FqnInvalid",
        EditErrorKind::SourceReadFailed => "SourceReadFailed",
        EditErrorKind::SourceParseFailed => "SourceParseFailed",
        EditErrorKind::ElementTableMissing => "ElementTableMissing",
        EditErrorKind::ExpectMismatch => "ExpectMismatch",
        EditErrorKind::DeleteHasChildren => "DeleteHasChildren",
        EditErrorKind::DeleteHasRelations => "DeleteHasRelations",
        EditErrorKind::RelationNotFound => "RelationNotFound",
        EditErrorKind::RelationDuplicated => "RelationDuplicated",
        EditErrorKind::RelationDeleteHasFields => "RelationDeleteHasFields",
        EditErrorKind::RelationDuplicateInBatch => "RelationDuplicateInBatch",
        EditErrorKind::CascadeCrossFile => "CascadeCrossFile",
        EditErrorKind::RelationOwnedElsewhere => "RelationOwnedElsewhere",
        EditErrorKind::RenderSelfCheck => "RenderSelfCheck",
        EditErrorKind::ReadBackMismatch => "ReadBackMismatch",
    }
}

/// 判据句文案表（design 数据模型表第二列，23 条逐字；变体名 → 判据句）。
const CRITERIA: [(&str, &str); 23] = [
    ("ModelHasDiagnostics", "模型存在结构诊断，拒绝写回"),
    ("ElementNotFound", "元素不存在"),
    ("ElementNotTable", "元素声明不是表形态"),
    ("ParentMissing", "父元素不在案"),
    ("RelationEndpointMissing", "关系端点元素不存在"),
    ("TargetInvalid", "落点非法"),
    ("ElementKindMissing", "新建元素缺 kind"),
    ("ElementAlreadyExists", "元素已在案"),
    ("FqnInvalid", "元素 FQN 形态非法"),
    ("SourceReadFailed", "模型文件读取失败"),
    ("SourceParseFailed", "模型源解析失败"),
    ("ElementTableMissing", "模型源缺少 element 表"),
    ("ExpectMismatch", "目标文件已变"),
    ("DeleteHasChildren", "元素仍有已声明子元素，拒绝删除"),
    ("DeleteHasRelations", "元素仍被关系引用，拒绝删除"),
    ("RelationNotFound", "关系不存在"),
    ("RelationDuplicated", "关系多处声明"),
    ("RelationDeleteHasFields", "内联删边项带了字段"),
    (
        "RelationDuplicateInBatch",
        "同一批 relations 重复声明同一目标",
    ),
    ("CascadeCrossFile", "级联闭包跨模型文件"),
    ("RelationOwnedElsewhere", "内联边声明在别的模型文件"),
    ("RenderSelfCheck", "写回渲染自证失败"),
    ("ReadBackMismatch", "回读不一致"),
];

/// 文案表里某变体对应的判据句（表覆盖不到的变体即测试自身的缺陷）。
fn expected_criterion(kind: EditErrorKind) -> &'static str {
    CRITERIA
        .iter()
        .find(|(name, _)| *name == variant(kind))
        .map(|(_, criterion)| *criterion)
        .unwrap_or_else(|| panic!("文案表应覆盖变体 {}", variant(kind)))
}

/// 正向：`ALL` 恰 23 值、逐值无重复；文案表覆盖的变体集合与 `ALL` 逐值相等。
#[test]
#[allow(non_snake_case)]
fn ALL与criterion分支集合逐值相等() {
    assert_eq!(EditErrorKind::ALL.len(), 23, "ALL 恰 23 值");

    let names: BTreeSet<&str> = EditErrorKind::ALL
        .iter()
        .map(|kind| variant(*kind))
        .collect();
    assert_eq!(names.len(), 23, "ALL 无重复变体");
    assert_eq!(
        names,
        CRITERIA
            .iter()
            .map(|(name, _)| *name)
            .collect::<BTreeSet<&str>>(),
        "文案表的变体集合与 ALL 逐值相等"
    );

    let criteria: BTreeSet<&str> = EditErrorKind::ALL
        .iter()
        .map(|kind| kind.criterion())
        .collect();
    assert_eq!(
        criteria.len(),
        23,
        "criterion 逐值互异（ALL 无重复的等价证明）"
    );
}

/// 正向：逐值与文案表逐字相等（23 条 needle 表驱动）。
#[test]
fn criterion逐值逐字对齐文案表() {
    for kind in EditErrorKind::ALL {
        assert_eq!(
            kind.criterion(),
            expected_criterion(kind),
            "判据句逐字对齐：{}",
            variant(kind)
        );
    }
}

/// 正向：逐值非空、首尾无空白、互异。
#[test]
fn 判据句逐值非空且互异() {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for kind in EditErrorKind::ALL {
        let criterion = kind.criterion();
        assert!(!criterion.is_empty(), "{} 的判据句非空", variant(kind));
        assert_eq!(criterion, criterion.trim(), "{} 首尾无空白", variant(kind));
        assert!(seen.insert(criterion), "判据句互异：{criterion}");
    }
    assert_eq!(seen.len(), 23);
}

/// 异常：逐值零「改档」（操作中立）、零动态占位（判据句恒为 `&'static str`，
/// 事实只能进 `facts`）。
#[test]
fn 判据句零改档且零占位() {
    for kind in EditErrorKind::ALL {
        let criterion = kind.criterion();
        assert!(!criterion.contains("改档"), "判据句操作中立：{criterion}");
        assert!(
            !criterion.contains('{') && !criterion.contains('}'),
            "零占位：{criterion}"
        );
    }
}

/// 正向：遍历 `ALL`，`message()` 恒为「判据句：事实」，恰一个全角冒号分隔。
#[test]
fn message恒为判据句加事实() {
    for kind in EditErrorKind::ALL {
        let message = error(kind, "事实段").message();
        assert_eq!(message, format!("{}：{}", kind.criterion(), "事实段"));
        assert!(
            message.starts_with(kind.criterion()),
            "以判据句起始：{message}"
        );
        assert_eq!(
            message.matches('：').count(),
            1,
            "恰一个全角冒号：{message}"
        );
        let (criterion, facts) = message.split_once('：').expect("含分隔符");
        assert_eq!(criterion, kind.criterion());
        assert_eq!(facts, "事实段", "分隔后即 facts 原文");
    }
}

/// 边界：`facts` 四形态逐字保留（不归一、不截断、不二次转义）。
#[test]
fn facts边界逐字保留() {
    let kind = EditErrorKind::ElementNotFound;
    let long = "长".repeat(1200);
    let cases: [String; 4] = [
        String::new(),
        "第一行\n第二行".to_owned(),
        "🚀 元素不存在".to_owned(),
        long,
    ];
    for facts in &cases {
        let message = error(kind, facts.clone()).message();
        let (criterion, tail) = message.split_once('：').expect("含分隔符");
        assert_eq!(criterion, kind.criterion());
        assert_eq!(tail, facts.as_str(), "facts 逐字保留");
    }

    let empty = error(kind, "").message();
    assert!(empty.ends_with('：'), "空 facts 以分隔符收尾：{empty}");
    assert!(empty.starts_with(kind.criterion()), "仍以判据句起始");
}

/// 边界：同 `kind` + 同 `facts` 两值相等；任一字段差一即不等；`Debug` 含两字段名。
#[test]
#[allow(non_snake_case)]
fn EditError逐字段可比() {
    let base = error(EditErrorKind::ExpectMismatch, "同一事实");
    assert_eq!(base, error(EditErrorKind::ExpectMismatch, "同一事实"));
    assert_ne!(
        base,
        error(EditErrorKind::ExpectMismatch, "同一事实!"),
        "facts 差一字符即不等"
    );
    assert_ne!(
        base,
        error(EditErrorKind::RelationNotFound, "同一事实"),
        "kind 差一值即不等"
    );

    let debug = format!("{base:?}");
    assert!(debug.contains("kind"), "Debug 含 kind：{debug}");
    assert!(debug.contains("facts"), "Debug 含 facts：{debug}");
}

/// 异常（源码锚）：错误渲染只有 `message()` 一个出口——零 `Display` / 零 `Error` 实现。
#[test]
#[allow(non_snake_case)]
fn 零Display与Error实现() {
    let source = include_str!("messages.rs");
    assert!(!source.contains("Display for"), "零 Display 实现");
    assert!(!source.contains("Error for"), "零 std::error::Error 实现");
    assert!(!source.contains("impl std::error"), "零 Error trait 实现");
}

/// 边界：`error(kind, "字面量")` 与 `error(kind, String)` 两入参形态产出的值相等。
#[test]
fn 构造单点两入参形态同值() {
    let from_literal: EditError = error(EditErrorKind::TargetInvalid, "落点事实");
    let from_string = error(EditErrorKind::TargetInvalid, String::from("落点事实"));
    assert_eq!(from_literal, from_string, "`impl Into<String>` 两臂同值");
}

/// 正向：四个新变体在册——`RelationDeleteHasFields` / `RelationDuplicateInBatch` /
/// `CascadeCrossFile` / `RelationOwnedElsewhere` 逐值在 `ALL` 中且 `criterion()` 非空。
#[test]
#[allow(non_snake_case)]
fn 四新变体在册() {
    for kind in [
        EditErrorKind::RelationDeleteHasFields,
        EditErrorKind::RelationDuplicateInBatch,
        EditErrorKind::CascadeCrossFile,
        EditErrorKind::RelationOwnedElsewhere,
    ] {
        assert!(
            EditErrorKind::ALL.contains(&kind),
            "{} 在 ALL 中",
            variant(kind)
        );
        assert!(
            !kind.criterion().trim().is_empty(),
            "{} 的判据句非空",
            variant(kind)
        );
        assert_eq!(
            kind.criterion(),
            expected_criterion(kind),
            "{} 的判据句与文案表逐字相等",
            variant(kind)
        );
    }
}
