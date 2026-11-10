//! `model` 的单元测试：`Model::element` 按 FQN 取元素的全字段契约、精确匹配
//! （前缀 / 大小写 / 特殊字符一律不命中）与派生 `PartialEq` / `Eq` 语义。
//!
//! 零 mock：纯数据结构字面量直构 + 真实 `TomlParser`（`api` 两值端到端对拍）；
//! 无 IO / 无 async / 无环境依赖，不经盘。

use std::collections::BTreeMap;

use crate::dto::{ApiStatus, ElementKind, GateTier};
use crate::model::{Element, Model, Relation, SourceSpan};
use crate::parse::{SourceParser, TomlParser};

/// 解析入口（`SourceParser` 关联函数，无接收者）：本文件既有用例为纯数据结构
/// 直构，新增的解析面用例（`api` 两值端到端对拍）复用同一真实 parser，零 mock。
fn parse_with_models(source: &str) -> crate::model::ParseOutcome {
    TomlParser::parse(source)
}

/// 元素构造器：缺省 `module` / `off` / `declared` / 空 path / 空 docs / 无摘要 / 空 metadata。
fn element(fqn: &str) -> Element {
    Element {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Off,
        api: ApiStatus::Declared,
        path: Vec::new(),
        docs: Vec::new(),
        summary: None,
        metadata: BTreeMap::new(),
        file: ".dev-team/architecture/models/01-core.toml".to_owned(),
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
        found.file, ".dev-team/architecture/models/01-core.toml",
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
    moved.elements[1].file = ".dev-team/architecture/models/02-auth.toml".to_owned();
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
        file: ".dev-team/architecture/models/01-core.toml".to_owned(),
        line: 12,
        span: None,
    };
    assert_eq!(
        without_span,
        Relation {
            from: "App.Auth".to_owned(),
            to: "App".to_owned(),
            description: None,
            file: ".dev-team/architecture/models/01-core.toml".to_owned(),
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
            file: ".dev-team/architecture/models/01-core.toml".to_owned(),
            line: 12,
            span: Some(SourceSpan { start: 10, end: 20 }),
        },
        "span 在场与缺席不等"
    );
}

// ===========================================================================
// `Element.api`（`module-api-contract-format` / AC-10 / AC-16）
//
// 契约状态指针：缺省 `declared`（模型未写该键即承诺有契约）；协议正文 MUST NOT 进模型
// ——签名 / 导出符号 / 接口定义写在架构域 `api` 目录的契约资产里。
//
// 本文件是纯数据结构测试（零 IO）：解析面的缺省 / 线形 / 非法值归 `parse/toml_test.rs`。
// ===========================================================================

/// 正向（AC-10）：`Element` 的 `api` 字段缺省值由构造器给出 `Declared`，两态逐值
/// 相等（`Eq` 可比较），且与其余十字段互不干扰。
#[test]
fn element_api两态与其余字段独立() {
    let declared = element("App.Auth");
    assert_eq!(
        declared.api,
        ApiStatus::Declared,
        "缺省 declared（承诺有契约）"
    );

    let mut blank = declared.clone();
    blank.api = ApiStatus::None;
    assert_eq!(blank.api, ApiStatus::None, "none 态可写");
    assert_ne!(blank, declared, "两态不等（Eq 可比较）");
    // 除 api 外全部字段相等（字段面正交）
    assert_eq!(blank.fqn, declared.fqn);
    assert_eq!(blank.kind, declared.kind);
    assert_eq!(blank.gate, declared.gate);
    assert_eq!(blank.path, declared.path);
    assert_eq!(blank.docs, declared.docs);
    assert_eq!(blank.summary, declared.summary);
    assert_eq!(blank.metadata, declared.metadata);
    assert_eq!(blank.file, declared.file);
    assert_eq!(blank.line, declared.line);

    // 值域封闭：两臂穷尽 match，无 `_` 兜底臂
    let words: Vec<&str> = ApiStatus::ALL
        .iter()
        .map(|status| match status {
            ApiStatus::Declared => "declared",
            ApiStatus::None => "none",
        })
        .collect();
    assert_eq!(words, vec!["declared", "none"], "二值穷尽");
}

/// 边界（AC-10 / AC-16）：`metadata` **不**承载 `api`（同名字符串键与字段互不影响）；
/// `path` 原文写法逐字保留（`./` 前缀 + 尾 `/` 不归一）。
#[test]
fn element_api与metadata路径互不干扰() {
    let mut subject = element("App.Auth");
    subject.path = vec!["./src/auth/".to_owned()];
    subject.metadata.insert("api".to_owned(), "non".to_owned());

    assert_eq!(
        subject.api,
        ApiStatus::Declared,
        "metadata 的同名键不改变 Element.api（职责边界）"
    );
    assert_eq!(
        subject.metadata.get("api").map(String::as_str),
        Some("non"),
        "metadata 原样承载该键（不当状态指针）"
    );
    assert_eq!(
        subject.path,
        vec!["./src/auth/".to_owned()],
        "path 原文写法逐字保留"
    );
}

/// 边界（AC-17）：`model.rs` 上不存在任何签名 / `$defs` / operations 类字段（源码自证）
/// ——`api` 只承载二值，协议正文不进模型。
#[test]
fn element_api协议正文不进模型() {
    let source = include_str!("model.rs");
    for forbidden in ["defs", "signature", "operations", "api_op", "api-types"] {
        assert!(
            !source.contains(forbidden),
            "模型面 MUST NOT 承载协议正文（出现 `{forbidden}`）"
        );
    }
    assert!(
        source.contains("api: ApiStatus"),
        "api 字段类型为封闭二值 ApiStatus"
    );
    assert!(
        source.contains("协议正文 MUST NOT 进模型"),
        "doc 明示协议正文不进模型"
    );
}

/// 边界（AC-10）：300 元素（各带 `api` 值）→ 逐元素字段可读、`Eq` 比较不 panic。
#[test]
fn element_api大模型逐元素可读() {
    let mut model = Model::empty();
    for index in 0..300 {
        let mut item = element(&format!("App.E{index:03}"));
        item.api = if index % 2 == 0 {
            ApiStatus::Declared
        } else {
            ApiStatus::None
        };
        model.elements.push(item);
    }
    assert_eq!(model.elements.len(), 300, "300 元素全部可读");

    let mut declared = 0;
    let mut blank = 0;
    for item in &model.elements {
        match item.api {
            ApiStatus::Declared => declared += 1,
            ApiStatus::None => blank += 1,
        }
        assert_eq!(item.clone(), item.clone(), "Eq 自反不 panic");
        assert!(!item.api.as_str().is_empty(), "线词非空");
    }
    assert_eq!(declared, 150, "declared 半量");
    assert_eq!(blank, 150, "none 半量");
}

// ===========================================================================
// `Element.api` 字段面零扩写（AC-10 / AC-16）
//
// `Element` 字段数 = 11（原 10 + `api`）；新增字段只此一处，MUST NOT 顺手扩面。
// 字段数以源码自证（`pub struct Element { … }` 块内的 `pub ` 行数）表达：结构体字段
// 面无 serde 反射可得，`include_str!` 自证是唯一零新增依赖的机械判据。
// ===========================================================================

/// 边界（AC-10 / AC-16）：`Element` 字段数恰 11（`fqn` / `kind` / `gate` / `api` /
/// `path` / `docs` / `summary` / `metadata` / `file` / `line` = 10 个既有 + `api` 1 个），
/// 且 `api` 是本变更唯一新增字段（`ApiStatus` 类型在场、协议正文四词零命中已在另测覆盖）。
#[test]
fn element字段数恰十一个且api为唯一新增() {
    let source = include_str!("model.rs");
    let block_start = source
        .find("pub struct Element {")
        .expect("model.rs 内应有 `pub struct Element {`");
    let block = &source[block_start..];
    let block_end = block.find("\n}").expect("结构体块应以独立 `}` 行收尾");
    let body = &block[..block_end];

    let fields: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("pub "))
        .collect();
    assert_eq!(
        fields.len(),
        11,
        "Element 字段数 = 11（原 10 + api），实际 {fields:?}"
    );
    for expected in [
        "fqn", "kind", "gate", "api", "path", "docs", "summary", "metadata", "file", "line",
    ] {
        assert!(
            fields.iter().any(|field| field.contains(expected)),
            "字段面应含 {expected}: {fields:?}"
        );
    }
    assert!(
        body.contains("pub api: ApiStatus"),
        "api 字段类型为封闭二值 ApiStatus"
    );
    assert_eq!(
        fields
            .iter()
            .filter(|field| field.contains("api: ApiStatus"))
            .count(),
        1,
        "`api` 字段只此一处（MUST NOT 顺手扩面）"
    );
}

// ===========================================================================
// `Element.api` 显式两值（AC-10 正向补面）
//
// 既有 `element_api两态与其余字段独立` 用直构字面量表达；本组补「解析面 → 模型面」
// 的端到端两值对拍，并断言省略 `gate` / `path` / `docs` / `summary` 的零值缺省不因
// 新增字段而变（AC-16 既有字段面零变化）。
// ===========================================================================

/// 正向（AC-10）：`api = "declared"` / `api = "none"` 两态经解析进入模型面，逐值相等；
/// 省略键即缺省 `Declared`。
#[test]
fn element_api两值经解析面逐值相等() {
    for (fragment, expected) in [
        ("api = \"declared\"\n", ApiStatus::Declared),
        ("api = \"none\"\n", ApiStatus::None),
        ("", ApiStatus::Declared),
    ] {
        let outcome = parse_with_models(&format!(
            "[element.\"App.Auth\"]\nkind = \"domain\"\n{fragment}"
        ));
        assert!(
            outcome.diagnostics.is_empty(),
            "合法源零诊断（{fragment:?}）: {:?}",
            outcome.diagnostics
        );
        let found = outcome.model.element("App.Auth").expect("元素在案");
        assert_eq!(found.api, expected, "api 值逐值相等（{fragment:?}）");
        assert_eq!(found.kind, ElementKind::Domain, "kind 逐值相等");
        assert_eq!(found.gate, GateTier::Off, "省略 gate 即缺省 off");
        assert!(found.path.is_empty(), "省略 path 即空数组");
        assert!(found.docs.is_empty(), "省略 docs 即空数组");
        assert_eq!(found.summary, None, "省略 summary 即 None");
    }
}

/// 正向（AC-16）：`path` 原文写法逐字保留（`./` 前缀 + 尾 `/` 不归一），且与 `api`
/// 字段正交——新增字段不改变既有键的解析结果。
#[test]
fn element_path原文在api字段新增后逐字保留() {
    let outcome = parse_with_models(
        "[element.\"App.Auth\"]\n\
         kind = \"domain\"\n\
         api = \"none\"\n\
         path = [\"./src/auth/\", \"src/token\"]\n\
         docs = [\"./src/auth/README.md\"]\n",
    );
    assert!(
        outcome.diagnostics.is_empty(),
        "合法源零诊断: {:?}",
        outcome.diagnostics
    );
    let found = outcome.model.element("App.Auth").expect("元素在案");
    assert_eq!(
        found.path,
        vec!["./src/auth/".to_owned(), "src/token".to_owned()],
        "path 原文写法逐字保留（不归一）"
    );
    assert_eq!(
        found.docs,
        vec!["./src/auth/README.md".to_owned()],
        "docs 原文写法逐字保留"
    );
    assert_eq!(found.api, ApiStatus::None, "api 与 path 正交");
}

/// 边界（AC-10）：`metadata` 与 `api` 同名字符串键 —— 解析面**不报错**、`Element.api`
/// 仍为缺省 `Declared`（`metadata` 是自由字符串表），该键原样停留在 metadata。
#[test]
fn element_api不被metadata同名键改变() {
    let outcome = parse_with_models(
        "[element.\"App\"]\n\
         kind = \"package\"\n\
         metadata = { api = \"non\" }\n",
    );
    assert!(
        outcome.diagnostics.is_empty(),
        "metadata 内同名键零校验零诊断: {:?}",
        outcome.diagnostics
    );
    let found = outcome.model.element("App").expect("元素在案");
    assert_eq!(
        found.api,
        ApiStatus::Declared,
        "metadata 的同名键不改变 Element.api（想留白须写顶层 api = \"none\"）"
    );
    assert_eq!(
        found.metadata.get("api").map(String::as_str),
        Some("non"),
        "该键原样停在 metadata（静默无效的在案反例）"
    );
}
