//! `model` 的单元测试：`Model::element` 按 FQN 取元素的全字段契约、精确匹配
//! （前缀 / 大小写 / 特殊字符一律不命中）与派生 `PartialEq` / `Eq` 语义。
//!
//! 零 mock：纯数据结构字面量直构，无 IO / 无 async / 无环境依赖。

use std::collections::BTreeMap;

use crate::dto::{ElementKind, GateTier};
use crate::model::{Element, Model, Relation, SourceSpan};

/// 元素构造器：缺省 `package` / `off` / 空 path / 空 docs / 无摘要 / 空 metadata。
fn element(fqn: &str) -> Element {
    Element {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Off,
        path: Vec::new(),
        docs: Vec::new(),
        summary: None,
        metadata: BTreeMap::new(),
        file: "openspec/architecture/models/01-core.toml".to_owned(),
        line: 1,
    }
}

/// 三元素模型（`App` / `App.Auth` / `App.Auth.Token`），中间元素带全字段。
fn three_elements() -> Model {
    let mut auth = element("App.Auth");
    auth.kind = ElementKind::Domain;
    auth.gate = GateTier::Strict;
    auth.path = vec!["./src/auth/".to_owned(), "./src/token/".to_owned()];
    auth.docs = vec!["./src/auth/README.md".to_owned()];
    auth.summary = Some("认证域".to_owned());
    auth.metadata = BTreeMap::from([
        ("owner".to_owned(), "core".to_owned()),
        ("tier".to_owned(), "1".to_owned()),
    ]);
    auth.line = 7;
    Model {
        elements: vec![element("App"), auth, element("App.Auth.Token")],
        relations: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// 正向：全字段相等 + 命中与声明序无关
// ---------------------------------------------------------------------------

/// 正向：三元素模型查 `App.Auth` → `Some`，九个字段逐字段等于构造值。
#[test]
fn element按fqn取元素且九字段逐项相等() {
    let model = three_elements();
    let found = model.element("App.Auth").expect("App.Auth 应命中");
    assert_eq!(found.fqn, "App.Auth");
    assert_eq!(found.kind, ElementKind::Domain);
    assert_eq!(found.gate, GateTier::Strict);
    assert_eq!(
        found.path,
        vec!["./src/auth/".to_owned(), "./src/token/".to_owned()],
        "path 逐项保留（不归一、不折叠）"
    );
    assert_eq!(found.docs, vec!["./src/auth/README.md".to_owned()]);
    assert_eq!(found.summary.as_deref(), Some("认证域"));
    assert_eq!(found.metadata.len(), 2, "metadata 多键逐项保留");
    assert_eq!(
        found.metadata.get("owner").map(String::as_str),
        Some("core")
    );
    assert_eq!(
        found.file, "openspec/architecture/models/01-core.toml",
        "声明文件逐字"
    );
    assert_eq!(found.line, 7, "声明行逐字");
}

/// 正向：命中与声明序无关——首个 / 中间 / 末个元素均命中。
#[test]
fn element命中与声明序无关_首中末三位置() {
    let model = three_elements();
    for (index, fqn) in ["App", "App.Auth", "App.Auth.Token"]
        .into_iter()
        .enumerate()
    {
        let found = model
            .element(fqn)
            .unwrap_or_else(|| panic!("第 {index} 位置的 {fqn} 应命中"));
        assert_eq!(found.fqn, fqn, "命中的元素 FQN 逐字");
    }
}

// ---------------------------------------------------------------------------
// 异常 / 边界：精确匹配面
// ---------------------------------------------------------------------------

/// 异常：空模型任意 FQN → `None`，不 panic。
#[test]
fn element空模型任意fqn返回none() {
    let model = Model::empty();
    assert!(model.elements.is_empty());
    for fqn in ["", "App", "App.Auth"] {
        assert!(
            model.element(fqn).is_none(),
            "空模型不应命中 {fqn:?}（不 panic）"
        );
    }
}

/// 边界：FQN 精确匹配——空串 / 前缀 / 超长路径 / 大小写不同均 `None`。
#[test]
fn element仅精确匹配_前缀与超长与大小写均不命中() {
    let model = three_elements();
    // 注：「App」在 fixture 中本身就是元素（顶层），故前缀样本取「App.Aut」——
    // 精确匹配语义以「非完整 FQN 不命中」为判据
    let misses = [
        "",
        "App.Aut",
        "App.Auth.",
        "App.Auth.Token.Extra",
        "app.auth",
        "App.Auth ",
        " App.Auth",
    ];
    for fqn in misses {
        assert!(
            model.element(fqn).is_none(),
            "{fqn:?} 不应命中（精确匹配，不做前缀 / 大小写 / 空白归一）"
        );
    }
    // 对照臂：精确串仍命中
    assert!(model.element("App.Auth").is_some(), "精确串仍命中");
}

/// 边界：`str` 特殊形态——超长（>1000 chars） / 含 `\n` / 含 `\0` / emoji → `None`
/// 且不 panic。
#[test]
fn element特殊字符fqn均不命中且不panic() {
    let model = three_elements();
    let long = "A".repeat(1200);
    let exotic = [
        long.clone(),
        "App\n.Auth".to_owned(),
        "App.Auth\0".to_owned(),
        "App.🚀Auth".to_owned(),
        "🚀".to_owned(),
    ];
    for fqn in &exotic {
        assert!(model.element(fqn).is_none(), "特殊字符 FQN 不命中: {fqn:?}");
    }
    // 同时确认特殊字符元素本身可被精确命中（查找语义不排斥特殊字符）
    let mut special = element("App.🚀Auth");
    special.summary = Some("emoji 元素".to_owned());
    let model = Model {
        elements: vec![special],
        relations: Vec::new(),
    };
    assert!(
        model.element("App.🚀Auth").is_some(),
        "特殊字符 FQN 精确命中"
    );
}

/// 边界：500 元素模型查末个元素 → `Some`（线性扫描不因规模改变语义）。
#[test]
fn element五百元素模型命中末个元素() {
    let elements: Vec<Element> = (0..500)
        .map(|index| element(&format!("App.Mod{index:03}")))
        .collect();
    let model = Model {
        elements,
        relations: Vec::new(),
    };
    let found = model.element("App.Mod499").expect("末个元素应命中");
    assert_eq!(found.fqn, "App.Mod499");
    assert!(model.element("App.Mod500").is_none(), "末个之后不命中");
}

// ---------------------------------------------------------------------------
// 派生语义（`PartialEq` / `Eq`）
// ---------------------------------------------------------------------------

/// 边界：同内容两 `Model` 相等；仅 `file` 或 `line` 不同即不等；`metadata: {}`
/// 与缺省同值相等；`Relation.span == None` 占位可比较。
#[test]
fn 派生相等语义_文件行参与比较与空表同值() {
    // 同内容相等
    assert_eq!(three_elements(), three_elements(), "同内容两模型相等");

    // 仅 file 不同即不等
    let mut moved = three_elements();
    moved.elements[1].file = "openspec/architecture/models/02-auth.toml".to_owned();
    assert_ne!(moved, three_elements(), "仅 file 不同即不等");

    // 仅 line 不同即不等
    let mut shifted = three_elements();
    shifted.elements[1].line = 8;
    assert_ne!(shifted, three_elements(), "仅 line 不同即不等");

    // metadata: {} 与缺省同值相等
    let mut empty_metadata = element("App");
    empty_metadata.metadata = BTreeMap::new();
    assert_eq!(empty_metadata, element("App"), "空 metadata 与缺省同值");

    // Relation.span == None 占位可比较
    let without_span = Relation {
        from: "App.Auth".to_owned(),
        to: "App".to_owned(),
        description: None,
        file: "openspec/architecture/models/01-core.toml".to_owned(),
        line: 12,
        span: None,
    };
    assert_eq!(
        without_span,
        Relation {
            from: "App.Auth".to_owned(),
            to: "App".to_owned(),
            description: None,
            file: "openspec/architecture/models/01-core.toml".to_owned(),
            line: 12,
            span: None,
        },
        "span 占位 None 可比较相等"
    );
    assert_ne!(
        without_span,
        Relation {
            from: "App.Auth".to_owned(),
            to: "App".to_owned(),
            description: None,
            file: "openspec/architecture/models/01-core.toml".to_owned(),
            line: 12,
            span: Some(SourceSpan { start: 10, end: 20 }),
        },
        "span 在场与缺席不等"
    );
}
