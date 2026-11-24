//! `parse::toml` 的单元测试：`TomlParser` 全字段反序列化 / 字段面严格性 /
//! 封闭枚举值域 / parser 原文诊断（行 + 列 + 片段 + caret 零加工），以及
//! `line_of` / `column_of` 的字节偏移 → 行列换算。
//!
//! 零 mock：`toml` crate 的原文案 / span 即被测契约（替身会抹掉判据）；本模块
//! 只吃 `&str`，不经盘。

use crate::dto::{DiagnosticKind, ElementKind, GateTier};
use crate::model::ParseOutcome;

use super::toml::{column_of, line_of};
use super::{SourceParser, TomlParser};

/// 解析入口（`SourceParser` 关联函数，无接收者）。
fn parse(source: &str) -> ParseOutcome {
    TomlParser::parse(source)
}

/// 子串所在行号（1 起始）：用于把「行号 = 表头键行」写成可读断言。
fn line_at(source: &str, needle: &str) -> u32 {
    let index = source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("源文本中找不到 {needle:?}"));
    index as u32 + 1
}

/// 仅断言坏点只产出一条诊断并取回它（单坏点源的便捷面）。
fn single_diagnostic(source: &str) -> crate::model::Diagnostic {
    let outcome = parse(source);
    assert_eq!(
        outcome.diagnostics.len(),
        1,
        "单坏点源应只产出一条诊断: {:?}",
        outcome.diagnostics
    );
    outcome
        .diagnostics
        .into_iter()
        .next()
        .expect("已断言恰一条")
}

// ---------------------------------------------------------------------------
// 正向：全字段 + 缺省
// ---------------------------------------------------------------------------

const FULL_SOURCE: &str = concat!(
    "# 顶层注解：注释不参与模型\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"strict\"\n",
    "path = [\"./src/auth/\", \"./src/token/\"]\n",
    "doc = \"./src/auth/README.md\"\n",
    "summary = \"认证域\"\n",
    "metadata = { owner = \"core\", tier = \"1\" }\n",
    "\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "gate = \"warn\"\n",
    "\n",
    "[element.\"App.Legacy\"]\n",
    "kind = \"module\"\n",
    "gate = \"off\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Auth\"\n",
    "to = \"App\"\n",
    "desc = \"认证依赖父包\"\n",
);

/// 正向：proposal 示例全字段 → 元素 / 关系逐字段等于期望；省略 `gate` 的元素取
/// `Off`；缺省 `path` 为空数组、`doc` 为 `None`、`metadata` 为空表。
#[test]
fn 解析全字段源_元素关系逐字段相等且缺省取零值() {
    let outcome = parse(FULL_SOURCE);
    assert!(
        outcome.diagnostics.is_empty(),
        "健康源零诊断: {:?}",
        outcome.diagnostics
    );

    // 元素：按 FQN 字典序（BTreeMap 语义）
    let fqns: Vec<&str> = outcome
        .model
        .elements
        .iter()
        .map(|e| e.fqn.as_str())
        .collect();
    assert_eq!(
        fqns,
        vec!["App", "App.Auth", "App.Billing", "App.Legacy"],
        "四元素齐全且按 FQN 字典序"
    );

    let app = outcome.model.element("App").expect("App 在模型内");
    assert_eq!(app.kind, ElementKind::Package);
    assert_eq!(app.gate, GateTier::Off, "省略 gate 取缺省 Off");
    assert!(app.path.is_empty(), "缺省 path 为空数组");
    assert_eq!(app.doc, None, "缺省 doc 为 None（字段不省略）");
    assert_eq!(app.summary, None, "缺省 summary 为 None");
    assert!(app.metadata.is_empty(), "缺省 metadata 为空表");
    assert_eq!(app.file, "", "单文件解析契约：file 留空待聚合层回填");

    let auth = outcome
        .model
        .element("App.Auth")
        .expect("App.Auth 在模型内");
    assert_eq!(auth.kind, ElementKind::Domain);
    assert_eq!(auth.gate, GateTier::Strict);
    assert_eq!(
        auth.path,
        vec!["./src/auth/".to_owned(), "./src/token/".to_owned()],
        "path 逐项保留（不归一）"
    );
    assert_eq!(
        auth.doc.as_deref(),
        Some("./src/auth/README.md"),
        "doc 原文保留"
    );
    assert_eq!(auth.summary.as_deref(), Some("认证域"));
    assert_eq!(auth.metadata.len(), 2, "metadata 两键均保留");
    assert_eq!(auth.metadata.get("owner").map(String::as_str), Some("core"));
    assert_eq!(auth.metadata.get("tier").map(String::as_str), Some("1"));

    // 其余 kind / gate 枚举值覆盖
    assert_eq!(
        outcome
            .model
            .element("App.Billing")
            .expect("App.Billing 在模型内")
            .gate,
        GateTier::Warn
    );
    assert_eq!(
        outcome
            .model
            .element("App.Legacy")
            .expect("App.Legacy 在模型内")
            .kind,
        ElementKind::Module
    );

    // 关系
    assert_eq!(outcome.model.relations.len(), 1);
    let relation = &outcome.model.relations[0];
    assert_eq!(relation.from, "App.Auth");
    assert_eq!(relation.to, "App");
    assert_eq!(relation.description.as_deref(), Some("认证依赖父包"));
    assert_eq!(relation.file, "", "关系的 file 亦留空");
    assert!(relation.span.is_some(), "关系保留源区间占位");
}

/// 正向：span 精确性（AC-1）——元素 `line` 逐行等于 `[element."…"]` 表头行；
/// `[[relation]]` 的 `line` 等于其 `from` 键行。
#[test]
fn 解析行号精确指向表头行与from键行() {
    let outcome = parse(FULL_SOURCE);
    assert!(outcome.diagnostics.is_empty());
    for fqn in ["App", "App.Auth", "App.Billing", "App.Legacy"] {
        let expected = line_at(FULL_SOURCE, &format!("[element.\"{fqn}\"]"));
        let element = outcome.model.element(fqn).expect("元素在模型内");
        assert_eq!(
            element.line, expected,
            "{fqn} 的 line 应等于其表头行（可直接 edit）"
        );
    }
    let relation = &outcome.model.relations[0];
    assert_eq!(
        relation.line,
        line_at(FULL_SOURCE, "from = \"App.Auth\""),
        "关系 line 等于 from 键行"
    );
}

/// 正向：空字符串 / 仅注释 / 仅空行 → 空模型零诊断。
#[test]
fn 解析空源与仅注释与仅空行_空模型零诊断() {
    for source in ["", "# 只有注释\n", "\n\n   \n\t\n", "# a\n# b\n\n"] {
        let outcome = parse(source);
        assert!(outcome.diagnostics.is_empty(), "{source:?} 不应产出诊断");
        assert!(outcome.model.elements.is_empty(), "{source:?} 零元素");
        assert!(outcome.model.relations.is_empty(), "{source:?} 零关系");
    }
}

// ---------------------------------------------------------------------------
// 异常：parser 免费五类（syntaxError / missingField / invalidValue /
// unknownField / duplicateElement）
// ---------------------------------------------------------------------------

/// 异常：漏闭合引号与漏闭合 `]` → `syntaxError`，`message` 原样含
/// `TOML parse error at line N, column M` + 源码片段 + caret，`line` 取自 parser。
#[test]
fn 语法错误保留parser原文与行列_caret在场() {
    for source in [
        "[element.\"App]\nkind = \"package\"\n",
        "[element.\"App\"\nkind = \"package\"\n",
    ] {
        let diagnostic = single_diagnostic(source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::SyntaxError,
            "非法 TOML 归 syntaxError: {source:?}"
        );
        assert!(
            diagnostic.message.contains("TOML parse error at line"),
            "parser 原文含行列: {}",
            diagnostic.message
        );
        assert!(
            diagnostic.message.contains('^'),
            "原文含 caret 指示: {}",
            diagnostic.message
        );
        assert!(
            diagnostic.message.contains('|'),
            "原文含片段边框: {}",
            diagnostic.message
        );
        assert!(diagnostic.column.is_some(), "parser 给出 span 即带列");
        assert!(diagnostic.line >= 1, "有 span 时行号 1 起始，不伪造 0");
        assert_eq!(diagnostic.file, "", "单文件契约：file 留空");
    }
}

/// 异常：缺 `kind` → 一条 `missingField`；位置未知时 `line == 0` 且 `column ==
/// None`（D5 可机械判别，绝不伪造行号）。
#[test]
fn 缺必填字段归_missing_field_且位置未知形态自洽() {
    let diagnostic = single_diagnostic("[element.\"App\"]\ngate = \"strict\"\n");
    assert_eq!(diagnostic.kind, DiagnosticKind::MissingField);
    assert!(
        diagnostic.message.contains("missing field"),
        "serde 原文保留: {}",
        diagnostic.message
    );
    assert_eq!(
        diagnostic.column.is_none(),
        diagnostic.line == 0,
        "位置未知两字段必须同时成立（line 0 ↔ column None，无 u32::MAX 哨兵）"
    );
    assert!(diagnostic.line != u32::MAX, "不出现 u32::MAX 哨兵值");
}

/// 异常：`invalidValue`——非法 `kind` 列出四个合法值（经 `ElementKind::ALL` 派生），
/// 非法 `gate` 列出三个合法值。
#[test]
fn 非法枚举值归_invalid_value_且列出全部合法值() {
    let kind_diagnostic = single_diagnostic("[element.\"App\"]\nkind = \"pakage\"\n");
    assert_eq!(kind_diagnostic.kind, DiagnosticKind::InvalidValue);
    for kind in ElementKind::ALL {
        assert!(
            kind_diagnostic.message.contains(kind.as_str()),
            "kind 错误文案应列出合法值 {}: {}",
            kind.as_str(),
            kind_diagnostic.message
        );
    }

    let gate_diagnostic =
        single_diagnostic("[element.\"App\"]\nkind = \"package\"\ngate = \"stirct\"\n");
    assert_eq!(gate_diagnostic.kind, DiagnosticKind::InvalidValue);
    for legal in ["off", "warn", "strict"] {
        assert!(
            gate_diagnostic.message.contains(legal),
            "gate 错误文案应列出合法值 {legal}: {}",
            gate_diagnostic.message
        );
    }
}

/// 异常：元素表写 `gatte` → `unknownField`（拒绝且位置在案）；同文件
/// `metadata = { unknownKey = "x" }` 被接受（两侧严格性不同且各自可辨）。
#[test]
fn 未知字段被拒而metadata未知键被接受() {
    let diagnostic =
        single_diagnostic("[element.\"App\"]\nkind = \"package\"\ngatte = \"strict\"\n");
    assert_eq!(diagnostic.kind, DiagnosticKind::UnknownField);
    assert!(
        diagnostic.message.contains("gatte"),
        "错误文案点名拼错的键: {}",
        diagnostic.message
    );
    assert!(diagnostic.line >= 1, "有 span 即带行列");

    // 对照组：metadata 内未知键被接受（map 语义），零诊断
    let outcome =
        parse("[element.\"App\"]\nkind = \"package\"\nmetadata = { unknownKey = \"x\" }\n");
    assert!(
        outcome.diagnostics.is_empty(),
        "metadata 未知键被接受: {:?}",
        outcome.diagnostics
    );
    let app = outcome.model.element("App").expect("App 在模型内");
    assert_eq!(
        app.metadata.get("unknownKey").map(String::as_str),
        Some("x"),
        "未知键值逐字保留"
    );
}

/// 正向（行为锚：`metadata` 子表与内联表等价）：内联表源与「子表挂在已声明元素
/// 之后」源各零诊断，`metadata` 键值逐项相同、其余字段逐字段相等，且元素集恰
/// `{App}`（不出现 `…metadata` 形态的元素）。
#[test]
fn metadata子表与内联表等价() {
    let inline = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "path = [\"./src/app/\"]\n",
        "doc = \"./src/app/README.md\"\n",
        "metadata = { owner = \"core\", tier = \"1\" }\n",
    );
    let subtable = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "path = [\"./src/app/\"]\n",
        "doc = \"./src/app/README.md\"\n",
        "\n",
        "[element.\"App\".metadata]\n",
        "owner = \"core\"\n",
        "tier = \"1\"\n",
    );

    let mut models = Vec::new();
    for (label, source) in [("内联表", inline), ("子表", subtable)] {
        let outcome = parse(source);
        assert!(
            outcome.diagnostics.is_empty(),
            "{label}形态应零诊断: {:?}",
            outcome.diagnostics
        );
        let fqns: Vec<&str> = outcome
            .model
            .elements
            .iter()
            .map(|element| element.fqn.as_str())
            .collect();
        assert_eq!(fqns, vec!["App"], "{label}形态元素集恰 {{App}}");
        models.push(outcome.model);
    }

    let left = models[0].element("App").expect("内联表形态 App 在场");
    let right = models[1].element("App").expect("子表形态 App 在场");
    for (key, expected) in [("owner", "core"), ("tier", "1")] {
        assert_eq!(
            left.metadata.get(key).map(String::as_str),
            Some(expected),
            "内联表形态 metadata 键 `{key}` 逐字还原"
        );
        assert_eq!(
            left.metadata.get(key).map(String::as_str),
            right.metadata.get(key).map(String::as_str),
            "两形态的 metadata 键 `{key}` 逐项相同"
        );
    }
    assert_eq!(
        left.metadata.len(),
        right.metadata.len(),
        "两形态 metadata 键数相同（零多余键）"
    );
    assert_eq!(left.kind, right.kind, "kind 逐字段相等");
    assert_eq!(left.path, right.path, "path 逐字段相等");
    assert_eq!(left.doc, right.doc, "doc 逐字段相等");
}

/// 异常（行为锚：值域）：`metadata` 非字符串值 → 拒绝且位置在案（按 spec「非字符
/// 串值即 `invalidValue`」与 D19 的值域判据：serde 的 `invalid type` 与前缀分类学
/// 的 `invalidValue` 同档，文案与位置零加工）。
#[test]
fn metadata非字符串值被拒() {
    for source in [
        "[element.\"App\"]\nkind = \"package\"\nmetadata = { n = 1 }\n",
        "[element.\"App\"]\nkind = \"package\"\nmetadata = { flag = true }\n",
    ] {
        let diagnostic = single_diagnostic(source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::InvalidValue,
            "非字符串 metadata 值是值域违例（invalidValue）: {source:?}"
        );
        assert!(
            diagnostic.message.contains("invalid type"),
            "parser 原文留痕（不做文案加工）: {}",
            diagnostic.message
        );
    }
}

/// 异常（行为锚：`metadata` 子表成为唯一声明）：`[element."App.Auth".metadata]` 是
/// 该 FQN 的唯一声明 → 缺 `kind` 即 `missingField`，且单文件解析全或无（零元素）。
#[test]
fn metadata子表成为唯一声明即缺kind() {
    let source =
        "[element.\"App\"]\nkind = \"package\"\n\n[element.\"App.Auth\".metadata]\nowner = \"core\"\n";
    let diagnostic = single_diagnostic(source);
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::MissingField,
        "唯一声明的子表缺 kind 即 missingField: {}",
        diagnostic.message
    );
    let outcome = parse(source);
    assert!(
        outcome.model.elements.is_empty(),
        "解析失败即全或无：不保留半个模型"
    );
}

/// 异常：未引号点分 `[element.App.Auth]` → `unknownField`（子键 `Auth` 不被
/// `RawElement` 字段面接受），且不与 `[element."App.Auth"]` 合并。
#[test]
fn 未引号点分表头被拒且不与合法形态合并() {
    let diagnostic = single_diagnostic("[element.App.Auth]\nkind = \"domain\"\n");
    assert_eq!(diagnostic.kind, DiagnosticKind::UnknownField);
    assert!(
        diagnostic.message.contains("Auth"),
        "文案点名非法子键: {}",
        diagnostic.message
    );

    // 两形态并存：非法形态使整份源解析失败（全或无），不产生合并后的模型
    let source = concat!(
        "[element.\"App.Auth\"]\n",
        "kind = \"domain\"\n",
        "\n",
        "[element.App.Auth]\n",
        "kind = \"domain\"\n",
    );
    let outcome = parse(source);
    assert_eq!(outcome.diagnostics.len(), 1, "恰一条 unknownField");
    assert!(outcome.model.elements.is_empty(), "非法形态不新增元素");
}

/// 异常：同文件重复 `[element."App.Auth"]` → `duplicateElement`（TOML
/// `duplicate key` 归类），零 panic。
#[test]
fn 同文件重复表头归_duplicate_element() {
    let source = concat!(
        "[element.\"App.Auth\"]\n",
        "kind = \"domain\"\n",
        "\n",
        "[element.\"App.Auth\"]\n",
        "kind = \"module\"\n",
    );
    let diagnostic = single_diagnostic(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::DuplicateElement);
    assert!(
        diagnostic.message.contains("duplicate key"),
        "TOML duplicate key 原文保留: {}",
        diagnostic.message
    );
}

// ---------------------------------------------------------------------------
// `path` 基数按 kind 分档（`package` 至多一条包根；其余 kind 不设上界）
// ---------------------------------------------------------------------------

/// 正向：`package` 恰 1 条 `path`（本仓 19 个元素的形态）→ 零诊断，元素在场且
/// `path` 逐字保留（`./` 前缀 / 尾 `/` 都不归一）。
#[test]
fn package一条path逐字保留且零诊断() {
    let source = concat!(
        "[element.\"Backend\"]\n",
        "kind = \"package\"\n",
        "path = [\"./packages/desktop/src-tauri/crates/\"]\n",
    );
    let outcome = parse(source);
    assert!(
        outcome.diagnostics.is_empty(),
        "1 条包根合法: {:?}",
        outcome.diagnostics
    );
    let element = outcome.model.element("Backend").expect("元素在场");
    assert_eq!(element.kind, ElementKind::Package);
    assert_eq!(
        element.path,
        vec!["./packages/desktop/src-tauri/crates/".to_owned()],
        "path 逐字保留（不归一）"
    );
}

/// 正向：`package` 省略 `path` 与显式 `path = []` 两形态（纯容器的形态）→ 各零
/// 诊断、`path` 为空数组、元素照常入模型（「省略」与「空数组」等价）。
#[test]
fn package零条path合法() {
    let sources = [
        "[element.\"Backend\"]\nkind = \"package\"\n",
        "[element.\"Backend\"]\nkind = \"package\"\npath = []\n",
    ];
    for source in sources {
        let outcome = parse(source);
        assert!(
            outcome.diagnostics.is_empty(),
            "0 条包根合法（纯容器）: {source:?} → {:?}",
            outcome.diagnostics
        );
        let element = outcome.model.element("Backend").expect("元素照常入模型");
        assert!(element.path.is_empty(), "省略与空数组都产空 path");
    }
}

/// 正向：`module` 2 条与 5 条（后者即 `Desktop.Shell` 形态）→ 零诊断、条目数与
/// 逐项值全量保留（不设上界、不截断）。
#[test]
fn module多条path合法() {
    let two = vec!["./src/a/".to_owned(), "./src/b/".to_owned()];
    let five = vec![
        "./src/core/".to_owned(),
        "./src/views/".to_owned(),
        "./src/components/".to_owned(),
        "./src/lib/".to_owned(),
        "./src/main.tsx".to_owned(),
    ];
    for paths in [two, five] {
        let items = paths
            .iter()
            .map(|path| format!("\"{path}\""))
            .collect::<Vec<String>>()
            .join(", ");
        let source = format!("[element.\"Desktop.Shell\"]\nkind = \"module\"\npath = [{items}]\n");
        let outcome = parse(&source);
        assert!(
            outcome.diagnostics.is_empty(),
            "module 不设上界（{} 条）: {:?}",
            paths.len(),
            outcome.diagnostics
        );
        let element = outcome.model.element("Desktop.Shell").expect("元素在场");
        assert_eq!(element.path, paths, "条目与逐项值全量保留");
    }
}

/// 正向：`domain` 与 `composite` 各 2 条 `path` → 两形态各自零诊断（选填容器不设
/// 上界），`path` 逐字保留。
#[test]
fn domain与composite多条path合法() {
    for kind in [ElementKind::Domain, ElementKind::Composite] {
        let source = format!(
            "[element.\"App.Group\"]\nkind = \"{}\"\npath = [\"./src/a/\", \"./src/b/\"]\n",
            kind.as_str()
        );
        let outcome = parse(&source);
        assert!(
            outcome.diagnostics.is_empty(),
            "{} 不设上界: {:?}",
            kind.as_str(),
            outcome.diagnostics
        );
        let element = outcome.model.element("App.Group").expect("元素在场");
        assert_eq!(
            element.path,
            vec!["./src/a/".to_owned(), "./src/b/".to_owned()],
            "{} 的 path 逐字保留",
            kind.as_str()
        );
    }
}

/// 异常：`package` 写 2 条与 3 条 `path` → 恰一条诊断（`Option` 即「至多一条」，
/// 不逐条铺开）：`invalidValue`、位置为第 2 条（非第 3 条、非表头行）、带列、
/// 文案含「至多一条包根」并给出两句出路。
#[test]
fn package多条path被拒且位置命中第二条() {
    let two = concat!(
        "[element.\"Backend\"]\n",
        "kind = \"package\"\n",
        "path = [\"./a/\", \"./b/\"]\n",
    );
    let diagnostic = single_diagnostic(two);
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::InvalidValue,
        "包根越界是值域违例"
    );
    assert_eq!(
        diagnostic.line,
        line_at(two, "./b/"),
        "位置取第 2 条 path 所在行"
    );
    assert_ne!(
        diagnostic.line,
        line_at(two, "[element.\"Backend\"]"),
        "不落到元素表头行"
    );
    let column = diagnostic.column.expect("基数诊断带列");
    assert!(column >= 1, "列 1 起始：{column}");
    assert_eq!(diagnostic.file, "", "单文件契约：file 留空");
    for piece in ["至多一条包根", "纯容器可留空", "module"] {
        assert!(
            diagnostic.message.contains(piece),
            "文案含出路 `{piece}`: {}",
            diagnostic.message
        );
    }

    // 3 条：仍恰一条，且位置恒为第 2 条（跨行布局下与第 3 条可辨）
    let three = concat!(
        "[element.\"Backend\"]\n",
        "kind = \"package\"\n",
        "path = [\n",
        "  \"./a/\",\n",
        "  \"./b/\",\n",
        "  \"./c/\",\n",
        "]\n",
    );
    let diagnostic = single_diagnostic(three);
    assert_eq!(diagnostic.line, line_at(three, "./b/"), "3 条仍只报第 2 条");
    assert_ne!(diagnostic.line, line_at(three, "./c/"), "不铺开到第 3 条");
}

/// 异常：`package` 写 200 条 `path`（超大清单）→ 仍恰一条诊断，位置恒为第 2 条
/// （`Option` 即「至多一条」，不随条目数增长）。
#[test]
fn package两百条path仍恰一条诊断() {
    let items = (0..200)
        .map(|index| format!("  \"./p{index:03}/\",\n"))
        .collect::<String>();
    let source = format!("[element.\"Backend\"]\nkind = \"package\"\npath = [\n{items}]\n");
    let diagnostic = single_diagnostic(&source);
    assert_eq!(diagnostic.kind, DiagnosticKind::InvalidValue);
    assert_eq!(
        diagnostic.line,
        line_at(&source, "./p001/"),
        "200 条仍只报第 2 条（位置不随规模漂移）"
    );
}

/// 边界：`package` 2 条 `path` 与 2 个健康元素同文件 → 诊断恰一条且 **3 个元素全部
/// 入模型**（`parse` 层不跳过坏元素；「不跳过」的纪律只在 `graph_view` 的停摆分支）。
#[test]
fn package越界诊断不跳过任何元素() {
    let source = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "path = [\"./a/\", \"./b/\"]\n",
        "\n",
        "[element.\"App.Mod\"]\n",
        "kind = \"module\"\n",
        "path = [\"./src/mod/\"]\n",
        "\n",
        "[element.\"App.Domain\"]\n",
        "kind = \"domain\"\n",
    );
    let diagnostic = single_diagnostic(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::InvalidValue);
    let outcome = parse(source);
    let fqns: Vec<&str> = outcome
        .model
        .elements
        .iter()
        .map(|element| element.fqn.as_str())
        .collect();
    assert_eq!(
        fqns,
        vec!["App", "App.Domain", "App.Mod"],
        "坏元素与健康元素全部入模型（不跳过）"
    );
}

/// 边界：第 2 条 `path` 的取值矩阵（`""` 空串 / >1000 字符超长 / 含 `\n` 与 emoji
/// 转义 / 目录尾 `/`）→ 判据只按**基数**不按内容：各自仍恰一条，位置恒在第 2 条。
#[test]
fn package第二条path取值矩阵只按基数() {
    let long = format!("\"{}\"", "x".repeat(1200));
    let values = [
        "\"\"".to_owned(),
        long,
        r#""a\nb🚀""#.to_owned(),
        "\"./c/\"".to_owned(),
    ];
    for value in values {
        let source = format!("[element.\"App\"]\nkind = \"package\"\npath = [\"./a/\", {value}]\n");
        let diagnostic = single_diagnostic(&source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::InvalidValue,
            "按基数判、不按内容: {value}"
        );
        assert_eq!(diagnostic.line, 3, "位置恒为第 2 条所在行: {value}");
        let outcome = parse(&source);
        let element = outcome.model.element("App").expect("元素照常入模型");
        assert_eq!(element.path.len(), 2, "两条 path 逐字保留（不截断）");
    }
}

/// 边界：两条 `path` 的布局矩阵（同行数组 / 每条一行）→ 行号分别为该行与第 2 条
/// 自身行，两形态都不落到表头行（span 精确性）。
#[test]
fn package两条path布局矩阵行号不落表头() {
    let same_line = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "path = [\"./a/\", \"./b/\"]\n",
    );
    let diagnostic = single_diagnostic(same_line);
    assert_eq!(
        diagnostic.line,
        line_at(same_line, "path = ["),
        "同行数组：位置即该行"
    );
    assert_ne!(
        diagnostic.line,
        line_at(same_line, "[element.\"App\"]"),
        "同行数组：不落到表头行"
    );

    let multi_line = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "path = [\n",
        "  \"./a/\",\n",
        "  \"./b/\",\n",
        "]\n",
    );
    let diagnostic = single_diagnostic(multi_line);
    assert_eq!(
        diagnostic.line,
        line_at(multi_line, "./b/"),
        "每条一行：位置为第 2 条自身行"
    );
    assert_ne!(
        diagnostic.line,
        line_at(multi_line, "[element.\"App\"]"),
        "每条一行：不落到表头行"
    );
}

/// 边界：`ElementKind::ALL` 四值 × 2 条 `path` 全矩阵（四值经 `ALL` 派生，不手抄
/// 字面量）→ 仅 `Package` 出诊断，`Domain` / `Module` / `Composite` 零诊断。
#[test]
fn path基数按kind分档四值全矩阵() {
    for kind in ElementKind::ALL {
        let source = format!(
            "[element.\"App\"]\nkind = \"{}\"\npath = [\"./a/\", \"./b/\"]\n",
            kind.as_str()
        );
        let outcome = parse(&source);
        match kind {
            ElementKind::Package => {
                assert_eq!(
                    outcome.diagnostics.len(),
                    1,
                    "Package 越界恰一条: {:?}",
                    outcome.diagnostics
                );
                assert_eq!(outcome.diagnostics[0].kind, DiagnosticKind::InvalidValue);
            }
            other => {
                assert!(
                    outcome.diagnostics.is_empty(),
                    "{} 不设上界: {:?}",
                    other.as_str(),
                    outcome.diagnostics
                );
                let element = outcome.model.element("App").expect("元素在场");
                assert_eq!(element.kind, other);
                assert_eq!(
                    element.path,
                    vec!["./a/".to_owned(), "./b/".to_owned()],
                    "{} 的 path 逐字保留",
                    other.as_str()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 边界：str / int / 规模
// ---------------------------------------------------------------------------

/// 边界：`str` 值边界——超长（>1000 chars）摘要逐字保留；`\n` / `\u0000` / emoji
/// 转义值逐字保留；`path` 1 项与 200 项、`doc` 单值均正确。
#[test]
fn 字符串值边界_超长与转义与数组规模均逐字保留() {
    let long = "摘".repeat(1200);
    let source = format!(
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "summary = \"{long}\"\n",
            "metadata = {{ note = \"行1\\n行2\", nil = \"a\\u0000b\", emoji = \"🚀\" }}\n",
            "path = [\"./a/\"]\n",
            "doc = \"./a/README.md\"\n",
        ),
        long = long
    );
    let outcome = parse(&source);
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let app = outcome.model.element("App").expect("App 在模型内");
    assert_eq!(app.summary.as_deref(), Some(long.as_str()), "超长摘要逐字");
    assert_eq!(
        app.metadata.get("note").map(String::as_str),
        Some("行1\n行2"),
        "换行转义逐字还原"
    );
    assert_eq!(
        app.metadata.get("nil").map(String::as_str),
        Some("a\u{0}b"),
        "NUL 转义逐字还原"
    );
    assert_eq!(
        app.metadata.get("emoji").map(String::as_str),
        Some("🚀"),
        "emoji 逐字"
    );
    assert_eq!(app.path, vec!["./a/".to_owned()], "单元素 path");
    assert_eq!(app.doc.as_deref(), Some("./a/README.md"), "单元素 doc");

    // 200 项 path：`module` 不设上界（`package` 至多一条包根，见下方 path 基数用例）
    let items = (0..200)
        .map(|index| format!("\"./p{index:03}/md.md\""))
        .collect::<Vec<String>>()
        .join(", ");
    let source = format!("[element.\"App\"]\nkind = \"module\"\npath = [{items}]\n");
    let outcome = parse(&source);
    assert!(outcome.diagnostics.is_empty());
    let app = outcome.model.element("App").expect("App 在模型内");
    assert_eq!(app.path.len(), 200, "200 项 path 全量保留");
    assert_eq!(app.path[199], "./p199/md.md", "末项逐字");
}

/// 边界：`int` 面——`Diagnostic.line` 为 `u32` 且 1 起始；`column` 为 `Option<u32>`
/// 且未知即 `None`；`line == 0` 与 `column == None` 并存（无哨兵值）。
#[test]
fn 诊断行列为u32与可选列_未知形态无哨兵() {
    let outcome = parse("[element.\"App]\nkind = \"package\"\n");
    let diagnostic = &outcome.diagnostics[0];
    let line: u32 = diagnostic.line;
    let column: Option<u32> = diagnostic.column;
    assert!(line >= 1, "有 span 即 1 起始");
    assert_eq!(column, Some(column.unwrap()), "列在场为 u32");
    assert_ne!(line, 0, "有 span 的行号不为 0");

    // 位置未知形态：line 0 与 column None 并存
    let unknown = single_diagnostic("[element.\"App\"]\ngate = \"strict\"\n");
    if unknown.column.is_none() {
        assert_eq!(unknown.line, 0, "位置未知时 line 为 0");
    }
    assert_ne!(unknown.line, u32::MAX);
}

/// 边界：规模面——500 元素 + 500 关系 → 零诊断，且逐元素 `line` 正确（防 span
/// 溢出 / 错位）。
#[test]
fn 规模面五百元素五百关系_零诊断且行号正确() {
    let mut source = String::new();
    for index in 0..500 {
        source.push_str(&format!(
            "[element.\"App.Mod{index:03}\"]\nkind = \"module\"\n\n"
        ));
    }
    for index in 0..500 {
        source.push_str(&format!(
            "[[relation]]\nfrom = \"App.Mod{index:03}\"\nto = \"App.Mod{:03}\"\n\n",
            (index + 1) % 500
        ));
    }
    let outcome = parse(&source);
    assert!(
        outcome.diagnostics.is_empty(),
        "规模面零诊断: {:?}",
        outcome.diagnostics.first()
    );
    assert_eq!(outcome.model.elements.len(), 500);
    assert_eq!(outcome.model.relations.len(), 500);

    let probe = outcome
        .model
        .element("App.Mod123")
        .expect("探针元素在模型内");
    let expected = line_at(&source, "[element.\"App.Mod123\"]");
    assert_eq!(probe.line, expected, "500 元素源中行号不漂移");

    let relation = &outcome.model.relations[123];
    assert_eq!(relation.from, "App.Mod123");
    assert_eq!(
        relation.line,
        line_at(&source, "from = \"App.Mod123\""),
        "500 关系源中关系行号不漂移"
    );
}

// ---------------------------------------------------------------------------
// 边界：line_of / column_of 换算
// ---------------------------------------------------------------------------

/// 边界：byte `0` → 行 1 / 列 1；行首 / 行尾 / 换行符字节本身均按 `\n` 计数、列
/// 1 起始且不 panic。
#[test]
fn 行列换算_行首行尾与换行符字节() {
    let source = "ab\ncd\n";
    assert_eq!(line_of(source, 0), 1, "byte 0 为行 1");
    assert_eq!(column_of(source, 0), 1, "byte 0 为列 1");
    assert_eq!(line_of(source, 1), 1, "行内偏移仍在行 1");
    assert_eq!(column_of(source, 1), 2, "行内偏移列 2");
    assert_eq!(line_of(source, 2), 1, "换行符字节本身属前一行");
    assert_eq!(column_of(source, 2), 3, "换行符字节列 3");
    assert_eq!(line_of(source, 3), 2, "换行符之后为行 2");
    assert_eq!(column_of(source, 3), 1, "行 2 行首列 1");
    assert_eq!(line_of(source, 4), 2);
    assert_eq!(column_of(source, 4), 2);
}

/// 边界：`byte == source.len()`（EOF）→ 末行 / 末列，不 panic 不越界；
/// `source == ""`（byte 0 == len）→ 不 panic。
#[test]
fn 行列换算_eof与空源不越界() {
    let source = "ab\ncd";
    assert_eq!(line_of(source, source.len()), 2, "EOF 落在末行");
    assert_eq!(column_of(source, source.len()), 3, "EOF 落在末列之后一位");
    assert_eq!(line_of("", 0), 1, "空源 byte 0 不 panic");
    assert_eq!(column_of("", 0), 1, "空源 byte 0 列 1");
    // 越界偏移亦回退到源尾（不 panic）
    assert_eq!(line_of(source, source.len() + 50), 2);
    assert_eq!(column_of("", 100), 1);
}

/// 边界：多字节面——CJK（三字节） / emoji（四字节）行前后取偏移 → 行号正确、列号
/// 按 **char** 计（字节数大于字符数，与 parser caret 同单位）。
#[test]
fn 行列换算按_char_计列_cjk_与_emoji() {
    let source = "中文\n🚀x";
    // '中' 0..3, '文' 3..6, '\n' 6, '🚀' 7..11, 'x' 11..12
    assert_eq!(column_of(source, 0), 1);
    assert_eq!(
        column_of(source, 3),
        2,
        "第二个 CJK 字符为列 2（按 char 计）"
    );
    assert_eq!(line_of(source, 3), 1);
    assert_eq!(line_of(source, 7), 2, "emoji 行在行 2");
    assert_eq!(column_of(source, 7), 1, "emoji 行行首列 1");
    assert_eq!(
        column_of(source, 11),
        2,
        "emoji 之后为列 2（4 字节只算 1 列）"
    );
    assert_eq!(line_of(source, 12), 2);
    // 落在多字节字符中间：回退到最近 char 边界，不 panic
    assert_eq!(column_of(source, 1), 1, "落在 CJK 中间回退到字符起点");
    assert_eq!(column_of(source, 9), 1, "落在 emoji 中间回退到字符起点");
}

// ===========================================================================
// `RawElement.api`（`module-api-contract-format` / AC-10）
//
// `#[serde(default)] api: ApiStatus`——缺省 `declared`；拼写错经既有 `classify` 归
// `invalidValue` 并列出全合法值（`classify()` 本身零改动）。
// ===========================================================================

/// 正向（AC-10）：未写 `api` 键 → 元素 `api == Declared`（`serde(default)` 生效）；
/// 显式两值 → 模型字段逐值相等。
#[test]
fn 解析api键缺省与显式两值() {
    let default_outcome = parse("[element.\"App\"]\nkind = \"package\"\n");
    assert!(default_outcome.diagnostics.is_empty(), "缺省形态零诊断");
    let element = default_outcome.model.element("App").expect("元素在案");
    assert_eq!(
        element.api,
        crate::dto::ApiStatus::Declared,
        "未写 api 键即缺省 declared"
    );

    for (word, expected) in [
        ("declared", crate::dto::ApiStatus::Declared),
        ("none", crate::dto::ApiStatus::None),
    ] {
        let source = format!("[element.\"App\"]\nkind = \"package\"\napi = \"{word}\"\n");
        let outcome = parse(&source);
        assert!(outcome.diagnostics.is_empty(), "合法值零诊断（{word}）");
        assert_eq!(
            outcome.model.element("App").expect("元素在案").api,
            expected,
            "api = \"{word}\" 逐值相等"
        );
    }
}

/// 异常（Enum N+1）：非法 `api` 值（大小写变体 / 空串 / 其他词 / 非字符串类型）→
/// `invalidValue` 且文案列出全合法值（`declared` + `none`）。
#[test]
fn 解析非法api值归invalid_value并列出合法值() {
    for word in ["Declared", "NONE", "non", "off", ""] {
        let source = format!("[element.\"App\"]\nkind = \"package\"\napi = \"{word}\"\n");
        let diagnostic = single_diagnostic(&source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::InvalidValue,
            "非法 api 值 {word:?} 归 invalidValue：{diagnostic:?}"
        );
        for legal in ["declared", "none"] {
            assert!(
                diagnostic.message.contains(legal),
                "错误文案应列出合法值 {legal}（{word:?}）：{}",
                diagnostic.message
            );
        }
    }

    // 非字符串类型：toml 层直接拒绝（`wanted string or table` → parser 免费语法错，
    // 不进 serde 分类学），仍须产出非零诊断并点名位置
    for literal in ["1", "true", "[\"declared\"]"] {
        let source = format!("[element.\"App\"]\nkind = \"package\"\napi = {literal}\n");
        let outcome = parse(&source);
        assert_eq!(
            outcome.diagnostics.len(),
            1,
            "非字符串 api 值 {literal} 应产出一条诊断：{:?}",
            outcome.diagnostics
        );
        let diagnostic = &outcome.diagnostics[0];
        assert!(
            matches!(
                diagnostic.kind,
                DiagnosticKind::SyntaxError | DiagnosticKind::InvalidValue
            ),
            "非字符串 api 值 {literal} 归语法错或值域违例：{diagnostic:?}"
        );
        assert!(diagnostic.line > 0, "位置在案（{literal}）");
    }
}

/// 边界：`api` 与其余字段正交——同元素同时带 `api = "none"` 与全字段（path / doc /
/// summary / metadata）→ 各自逐值保留；`api` 键不改变其它字段的解析结果。
#[test]
fn 解析api与其余字段正交() {
    let source = "[element.\"App.Auth\"]\n\
                  kind = \"domain\"\n\
                  api = \"none\"\n\
                  path = [\"./src/auth/\"]\n\
                  doc = \"src/auth/README.md\"\n\
                  summary = \"认证\"\n\n\
                  [element.\"App.Auth\".metadata]\n\
                  owner = \"auth-team\"\n";
    let outcome = parse(source);
    assert!(outcome.diagnostics.is_empty(), "全字段合法源零诊断");
    let element = outcome.model.element("App.Auth").expect("元素在案");
    assert_eq!(element.api, crate::dto::ApiStatus::None, "api 值保留");
    assert_eq!(
        element.path,
        vec!["./src/auth/".to_owned()],
        "path 原文保留"
    );
    assert_eq!(
        element.doc.as_deref(),
        Some("src/auth/README.md"),
        "doc 原文保留"
    );
    assert_eq!(element.summary.as_deref(), Some("认证"), "summary 保留");
    assert_eq!(
        element.metadata.get("owner").map(String::as_str),
        Some("auth-team"),
        "metadata 保留"
    );
}

/// 边界：`api` 与 `metadata` 同名键互不干扰——`metadata.api = "non"` 不改变
/// `Element.api`；该键照常进 metadata（未知键接受面）。
#[test]
fn 解析metadata的api键不当状态指针() {
    let source = "[element.\"App\"]\n\
                  kind = \"package\"\n\n\
                  [element.\"App\".metadata]\n\
                  api = \"non\"\n";
    let outcome = parse(source);
    assert!(outcome.diagnostics.is_empty(), "metadata 未知键被接受");
    let element = outcome.model.element("App").expect("元素在案");
    assert_eq!(
        element.api,
        crate::dto::ApiStatus::Declared,
        "metadata 的 api 键不改变 Element.api"
    );
    assert_eq!(
        element.metadata.get("api").map(String::as_str),
        Some("non"),
        "该键原样进 metadata"
    );
}

/// 边界：300 元素逐带 `api` 值 → 零诊断、逐元素状态可读、计数与声明一致。
#[test]
fn 解析api规模面三百元素() {
    let mut source = String::new();
    for index in 0..300 {
        let word = if index % 2 == 0 { "declared" } else { "none" };
        source.push_str(&format!(
            "[element.\"App.E{index:03}\"]\nkind = \"domain\"\napi = \"{word}\"\n\n"
        ));
    }
    let outcome = parse(&source);
    assert!(outcome.diagnostics.is_empty(), "规模面零诊断");
    assert_eq!(outcome.model.elements.len(), 300, "300 元素");
    let declared = outcome
        .model
        .elements
        .iter()
        .filter(|element| element.api == crate::dto::ApiStatus::Declared)
        .count();
    let blank = outcome
        .model
        .elements
        .iter()
        .filter(|element| element.api == crate::dto::ApiStatus::None)
        .count();
    assert_eq!(declared, 150, "declared 半量");
    assert_eq!(blank, 150, "none 半量");
}

// ===========================================================================
// 拼错键面（`deny_unknown_fields` 逐形态）
//
// 既有 `未知字段被拒而metadata未知键被接受` 只覆盖 `gatte` 一形态；本组补 `appi`
// 拼错（`api` 的错拼，最易混淆面）与 `api` / `API` 并存（大小写变体不归一——
// `deny_unknown_fields` 是精确字段名匹配）。
// ===========================================================================

/// 异常（AC-10）：`api` 的错拼键 `appi = "none"` → `unknownField` 一条，文案点名该
/// 键且行号落在拼错键行；模型不产出元素（全或无）。对照组：正确拼写零诊断且逐值相等。
#[test]
fn 拼错键appi被拒而api正确拼写被接受() {
    let source = "[element.\"App\"]\nkind = \"package\"\nappi = \"none\"\n";
    let diagnostic = single_diagnostic(source);
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::UnknownField,
        "拼错键 appi 归 unknownField: {diagnostic:?}"
    );
    assert!(
        diagnostic.message.contains("appi"),
        "错误文案点名拼错的键: {}",
        diagnostic.message
    );
    assert_eq!(
        diagnostic.line,
        line_at(source, "appi"),
        "行号 = 拼错键所在行"
    );
    assert!(
        parse(source).model.elements.is_empty(),
        "解析失败即全或无：不静默保留半个模型"
    );

    // 对照组：正确拼写被接受，字段逐值相等
    let accepted = parse("[element.\"App\"]\nkind = \"package\"\napi = \"none\"\n");
    assert!(accepted.diagnostics.is_empty(), "正确拼写零诊断");
    assert_eq!(
        accepted.model.element("App").expect("元素在案").api,
        crate::dto::ApiStatus::None,
        "api = \"none\" 逐值相等"
    );
}

/// 异常（AC-10）：`api` 与全大写变体 `API` 并存 → 未知键 `API` 被拒（字段名匹配
/// 大小写精确、不做归一），且合法 `api` 值不被部分采纳（全或无）。
#[test]
fn api与全大写变体并存时未知键被拒() {
    let source = "[element.\"App\"]\nkind = \"package\"\napi = \"none\"\nAPI = \"declared\"\n";
    let diagnostic = single_diagnostic(source);
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::UnknownField,
        "大写 API 是未知键（精确字段名匹配）: {diagnostic:?}"
    );
    assert!(
        diagnostic.message.contains("API"),
        "错误文案点名未知键 API: {}",
        diagnostic.message
    );
    assert!(
        diagnostic.line >= 1,
        "位置在案（未知键或元素表头行）: {diagnostic:?}"
    );
    assert!(
        parse(source).model.elements.is_empty(),
        "共存形态不被部分采纳：不静默保留半个模型"
    );

    // 对照：单写大写 `API`（无小写 api）同样归未知键
    let lone = single_diagnostic("[element.\"App\"]\nkind = \"package\"\nAPI = \"none\"\n");
    assert_eq!(
        lone.kind,
        DiagnosticKind::UnknownField,
        "孤立大写 API 亦未知键: {lone:?}"
    );
    assert!(
        lone.message.contains("API"),
        "文案点名 API: {}",
        lone.message
    );
}

// ===========================================================================
// 协议正文键被拒（AC-17：模型只存状态指针）
//
// `defs` / `operations` / `signature` 三类协议正文键写进元素表 → 全报未知键；
// 元素表字段面 MUST NOT 因新增 `api` 而放宽（`deny_unknown_fields` 原位）。
// ===========================================================================

/// 异常（AC-17）：`defs` / `operations` / `signature` 三形态 → 各 `unknownField`
/// 一条、文案点名该键、位置在案、模型空（不静默保留半个模型）。
#[test]
fn 协议正文键defs_operations_signature被拒() {
    for (fragment, key) in [
        ("defs = { LocalArchiCheck = { type = \"object\" } }", "defs"),
        (
            "operations = [{ id = \"check\", returns = \"bool\" }]",
            "operations",
        ),
        ("signature = \"pub fn check(&self) -> bool\"", "signature"),
    ] {
        let source = format!("[element.\"App\"]\nkind = \"package\"\n{fragment}\n");
        let diagnostic = single_diagnostic(&source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::UnknownField,
            "协议正文键 {key} 归 unknownField: {diagnostic:?}"
        );
        assert!(
            diagnostic.message.contains(key),
            "错误文案点名协议正文键 {key}: {}",
            diagnostic.message
        );
        assert!(diagnostic.line > 0, "位置在案（{key}）");
        assert!(
            parse(&source).model.elements.is_empty(),
            "协议正文键 {key} 使整源全或无：不静默保留半个模型"
        );
    }
}

// ===========================================================================
// `RawElement.api` 内联子表形态补面
//
// 既有 `解析非法api值归invalid_value并列出合法值` 覆盖 `1` / `true` / `[\"declared\"]`
// 三形态；本组补 `api = { v = \"none\" }` 内联表（serde 的 `invalid type: map` → 既有
// `classify` 归 `InvalidValue`，零分类改动）。
// ===========================================================================

/// 异常（AC-10 / `classify` 零改动）：`api = { v = "none" }` 内联表 → `InvalidValue`
/// 一条。表形态被 serde 读成外部标记枚举的变体名（`v`）→ parser 报 `unknown variant`
/// 并列出全合法值；两种归类标记（`unknown variant` / `invalid type`）同落既有
/// `classify` 的 `InvalidValue` 臂，此处并置断言。位置在案、模型空。
#[test]
fn api子表形态与标量形态同归invalid_value() {
    let table_source = "[element.\"App\"]\nkind = \"package\"\napi = { v = \"none\" }\n";
    let table_diagnostic = single_diagnostic(table_source);
    assert_eq!(
        table_diagnostic.kind,
        DiagnosticKind::InvalidValue,
        "内联表形态归 invalidValue（variant / type 两类标记均落既有 classify）: {table_diagnostic:?}"
    );
    assert!(
        table_diagnostic.message.contains("unknown variant")
            || table_diagnostic.message.contains("invalid type"),
        "parser 原文留痕（归类标记零加工）: {}",
        table_diagnostic.message
    );
    for legal in ["declared", "none"] {
        assert!(
            table_diagnostic.message.contains(legal),
            "错误文案应列出合法值 {legal}: {}",
            table_diagnostic.message
        );
    }
    assert!(table_diagnostic.line > 0, "位置在案（表形态）");
    assert!(
        parse(table_source).model.elements.is_empty(),
        "解析失败即全或无：不静默保留半个模型"
    );

    // 对照：标量形态在 toml 层即被拒（`wanted string or table` → parser 免费语法错，
    // 不进 serde 分类学）；与表形态同属「非成功且位置在案」，归类标记各按既有口径
    let scalar = single_diagnostic("[element.\"App\"]\nkind = \"package\"\napi = 1\n");
    assert!(
        matches!(
            scalar.kind,
            DiagnosticKind::SyntaxError | DiagnosticKind::InvalidValue
        ),
        "标量形态归语法错或值域违例（既有口径）: {scalar:?}"
    );
    assert!(scalar.line > 0, "标量形态位置在案");
    assert!(
        parse("[element.\"App\"]\nkind = \"package\"\napi = 1\n")
            .model
            .elements
            .is_empty(),
        "标量形态同样全或无"
    );
}

// ===========================================================================
// 规模补面：千行注释前缀下行号 + 300 元素混合 `api` 形态
//
// 既有 `规模面五百元素五百关系_零诊断且行号正确` 与 `解析api规模面三百元素` 各覆盖
// 半面；本组把「超长注释前缀」与「元素 / 关系行号」交叉（span 行号不得被注释行污染），
// 并把 300 元素中「写 api / 不写 api」两形态混合解析。
// ===========================================================================

/// 边界：文件首部 1000 行注释 → 元素 / 关系行号仍精确指向声明行（`span` 按字节偏移
/// 换算，注释不参与模型但计入行号）；元素 `api` 值跨千行距后仍逐值相等。
#[test]
fn 千行注释前缀下行号仍准确() {
    let mut source = String::with_capacity(16 * 1024);
    for index in 0..1000 {
        source.push_str(&format!("# 注释第 {index} 行\n"));
    }
    let element_line = 1000 + 1; // 第 1001 行：`[element."App"]`
    source.push_str("[element.\"App\"]\nkind = \"package\"\napi = \"none\"\n\n");
    let relation_line = 1000 + 6; // 第 1006 行：`from = "App"`（行号取 from 键行）
    source.push_str("[[relation]]\nfrom = \"App\"\nto = \"App\"\n");

    let outcome = parse(&source);
    assert!(
        outcome.diagnostics.is_empty(),
        "千行注释零诊断: {:?}",
        outcome.diagnostics
    );
    assert_eq!(
        outcome.model.elements.len(),
        1,
        "注释不产出元素（只有 1 个真实元素）"
    );
    let element = outcome.model.element("App").expect("App 在案");
    assert_eq!(
        element.line, element_line,
        "元素行号不被 1000 行注释污染（行号 = 表头行）"
    );
    assert_eq!(
        element.api,
        crate::dto::ApiStatus::None,
        "跨千行距后 api 值仍逐值相等"
    );
    assert_eq!(outcome.model.relations.len(), 1, "1 条关系");
    assert_eq!(
        outcome.model.relations[0].line, relation_line,
        "关系行号 = from 键行（第 1006 行）"
    );
    assert_ne!(element.line, 0, "行号非「位置未知」哨兵");
}

/// 边界（AC-10）：300 元素混合形态（每三个一循环：显式 `declared` / 显式 `none` /
/// 省略 `api`）→ 零诊断，三形态各 100 个，省略形态全部落到缺省 `Declared`。
#[test]
fn 三百元素混合api形态逐元素可判() {
    let mut source = String::with_capacity(32 * 1024);
    for index in 0..300 {
        source.push_str(&format!(
            "[element.\"App.E{index:03}\"]\nkind = \"domain\"\n"
        ));
        match index % 3 {
            0 => source.push_str("api = \"declared\"\n"),
            1 => source.push_str("api = \"none\"\n"),
            _ => {}
        }
        source.push('\n');
    }

    let outcome = parse(&source);
    assert!(
        outcome.diagnostics.is_empty(),
        "混合形态零诊断: {:?}",
        outcome.diagnostics
    );
    assert_eq!(outcome.model.elements.len(), 300, "300 元素");

    let mut declared_explicit = 0;
    let mut blank = 0;
    let mut omitted = 0;
    for index in 0..300 {
        let element = outcome
            .model
            .element(&format!("App.E{index:03}"))
            .expect("每个元素都在模型内");
        match index % 3 {
            0 => {
                assert_eq!(
                    element.api,
                    crate::dto::ApiStatus::Declared,
                    "显式 declared"
                );
                declared_explicit += 1;
            }
            1 => {
                assert_eq!(element.api, crate::dto::ApiStatus::None, "显式 none");
                blank += 1;
            }
            _ => {
                assert_eq!(
                    element.api,
                    crate::dto::ApiStatus::Declared,
                    "省略 api 键 → 缺省 declared"
                );
                omitted += 1;
            }
        }
    }
    assert_eq!(declared_explicit, 100, "显式 declared 一百");
    assert_eq!(blank, 100, "显式 none 一百");
    assert_eq!(omitted, 100, "省略形态一百");
}

// ===========================================================================
// `entry`：单值实现入口指针（缺省 `None` / 逐字保留 / 类型分类 / 拼错键 / 特殊字符 /
// 与 `api` `path` `gate` 正交 / `metadata` 同名键互不干扰）
// ===========================================================================

/// 正向：省略 `entry` → `None`；显式值逐字保留（`./` 前导不剥——归一归 `api::paths`，
/// 解析层零归一）。
#[test]
fn 解析entry键缺省与显式逐字保留() {
    let omitted = parse("[element.\"App.Auth\"]\nkind = \"domain\"\npath = [\"src/auth/\"]\n");
    assert!(omitted.diagnostics.is_empty(), "健康源零诊断");
    assert_eq!(
        omitted.model.element("App.Auth").expect("元素在案").entry,
        None,
        "省略 entry → None（不是空串）"
    );

    for raw in [
        "packages/desktop/src-tauri/src/lib.rs",
        "./packages/desktop/src-tauri/src/lib.rs",
    ] {
        let source = format!(
            "[element.\"App.Auth\"]\nkind = \"domain\"\nentry = \"{raw}\"\npath = [\"src/auth/\"]\n"
        );
        let outcome = parse(&source);
        assert!(
            outcome.diagnostics.is_empty(),
            "健康源零诊断（{raw}）：{:?}",
            outcome.diagnostics
        );
        assert_eq!(
            outcome.model.element("App.Auth").expect("元素在案").entry,
            Some(raw.to_owned()),
            "解析层逐字保留（不做第二次归一）：{raw}"
        );
    }
}

/// 异常（类型分类）：数组 / 内联表形态 → `invalidValue`，零元素产出，文案含类型信息。
#[test]
fn 解析entry数组与表形态归invalid_value() {
    for literal in ["[\"a\"]", "{ path = \"x\" }"] {
        let source = format!("[element.\"App\"]\nkind = \"package\"\nentry = {literal}\n");
        let outcome = parse(&source);
        assert_eq!(
            outcome.diagnostics.len(),
            1,
            "非单值 entry 形态 {literal} 应产出一条诊断：{:?}",
            outcome.diagnostics
        );
        let diagnostic = &outcome.diagnostics[0];
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::InvalidValue,
            "单值面被违反归 invalidValue：{diagnostic:?}"
        );
        assert!(
            !diagnostic.message.is_empty(),
            "文案非空（含类型信息）：{diagnostic:?}"
        );
        assert!(
            outcome.model.elements.is_empty(),
            "值域违例零元素产出（fail-closed）：{:?}",
            outcome.model.elements
        );
    }
}

/// 异常（严格字段面）：拼错键 `entri` / 复数 `entries` 仍被 `deny_unknown_fields` 拒——
/// 新字段不放松既有的未知键判据。
#[test]
fn 解析拼错entry键归unknown_field() {
    for key in ["entri", "entries"] {
        let source = format!("[element.\"App\"]\nkind = \"package\"\n{key} = \"src/lib.rs\"\n");
        let diagnostic = single_diagnostic(&source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::UnknownField,
            "拼错键 {key} 归 unknownField：{diagnostic:?}"
        );
    }
    let accepted = parse("[element.\"App\"]\nkind = \"package\"\nentry = \"src/lib.rs\"\n");
    assert!(
        accepted.diagnostics.is_empty(),
        "正确拼写被接受：{:?}",
        accepted.diagnostics
    );
}

/// 边界（str 四边）：空串 / 超长（>1000 chars）/ 换行与 emoji / `\0` 转义 → 逐字保留、
/// 零诊断、不 panic（形态判据归 `api::paths`，解析层不筛）。
#[test]
fn 解析entry特殊字符逐字保留零诊断() {
    let long = "a".repeat(1200);
    for raw in [
        String::new(),
        long.clone(),
        "src/\n模块🎯.tsx".to_owned(),
        "src/\u{0}.rs".to_owned(),
    ] {
        let source = format!(
            "[element.\"App.X\"]\nkind = \"module\"\nentry = \"{}\"\n",
            raw.replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('\0', "\\u0000")
        );
        let outcome = parse(&source);
        assert!(
            outcome.diagnostics.is_empty(),
            "特殊字符零诊断（{raw:?}）：{:?}",
            outcome.diagnostics
        );
        assert_eq!(
            outcome.model.element("App.X").expect("元素在案").entry,
            Some(raw.clone()),
            "逐字保留：{raw:?}"
        );
    }
}

/// 边界（正交四组合）：`entry` 有值 ↔ 缺省，`api = none` ↔ `declared`，`path` / `gate`
/// 各自保留——两指针互不吞并，`entry` 不改变其它字段的解析结果。
#[test]
fn 解析entry与api_path_gate正交() {
    let cases: [(&str, Option<&str>, &str); 4] = [
        ("declared", None, "strict"),
        ("declared", Some("src/a/lib.rs"), "warn"),
        ("none", None, "off"),
        ("none", Some("src/a/main.rs"), "strict"),
    ];
    for (api, entry, gate) in cases {
        let entry_line = entry
            .map(|value| format!("entry = \"{value}\"\n"))
            .unwrap_or_default();
        let source = format!(
            "[element.\"App.Core.X\"]\nkind = \"module\"\ngate = \"{gate}\"\n\
             api = \"{api}\"\n{entry_line}path = [\"./src/a/\"]\n"
        );
        let outcome = parse(&source);
        assert!(
            outcome.diagnostics.is_empty(),
            "四组合零诊断（{api} / {entry:?} / {gate}）：{:?}",
            outcome.diagnostics
        );
        let element = outcome.model.element("App.Core.X").expect("元素在案");
        assert_eq!(
            element.api,
            if api == "none" {
                crate::dto::ApiStatus::None
            } else {
                crate::dto::ApiStatus::Declared
            },
            "api 逐值可读"
        );
        assert_eq!(element.entry, entry.map(str::to_owned), "entry 逐值可读");
        assert_eq!(element.path, vec!["./src/a/".to_owned()], "path 原文保留");
        assert_eq!(
            element.gate,
            match gate {
                "strict" => GateTier::Strict,
                "warn" => GateTier::Warn,
                _ => GateTier::Off,
            },
            "gate 逐值可读"
        );
    }
}

/// 边界：`entry` 与 `metadata` 同名键互不干扰——`metadata.entry` 不改变 `Element.entry`，
/// 该键原样进 metadata（未知键接受面）。
#[test]
fn 解析metadata的entry键不当入口指针() {
    let source = "[element.\"App\"]\n\
                  kind = \"package\"\n\n\
                  [element.\"App\".metadata]\n\
                  entry = \"src/not-an-entry.rs\"\n";
    let outcome = parse(source);
    assert!(outcome.diagnostics.is_empty(), "metadata 未知键被接受");
    let element = outcome.model.element("App").expect("元素在案");
    assert_eq!(
        element.entry, None,
        "metadata 的 entry 键不改变 Element.entry"
    );
    assert_eq!(
        element.metadata.get("entry").map(String::as_str),
        Some("src/not-an-entry.rs"),
        "该键原样进 metadata"
    );
}

use crate::dto::ApiStatus;

// ===========================================================================
// `doc` 键（单值入口文档指针）与 `deny_unknown_fields` 白名单同步
//
// 解析面只做「单值 vs 非单值」的形态判定：`doc` 是单值，数组 / 内联表 / 数字一律
// `invalidValue`（MUST NOT 静默取首条）；旧的多值键已出白名单，写了即 `unknownField`
// ——模型停摆的反向判据。
// ===========================================================================

/// 正向：`doc` 单值逐字进模型（`./` 前缀不剥——归一归 `api::paths`）；`api = "none"`
/// 与 `doc` 同元素并存合法。
#[test]
fn 解析doc单值逐字进模型() {
    for (raw, expected) in [
        ("src/auth/README.md", "src/auth/README.md"),
        ("./src/auth/README.md", "./src/auth/README.md"),
    ] {
        let source =
            format!("[element.\"App.Auth\"]\nkind = \"domain\"\napi = \"none\"\ndoc = \"{raw}\"\n");
        let outcome = parse(&source);
        assert!(
            outcome.diagnostics.is_empty(),
            "合法源零诊断：{:?}",
            outcome.diagnostics
        );
        let element = outcome.model.element("App.Auth").expect("元素在案");
        assert_eq!(
            element.doc.as_deref(),
            Some(expected),
            "原文逐字保留（解析面零改写）"
        );
        assert_eq!(element.api, ApiStatus::None, "`api` 与 `doc` 并存合法");
    }
}

/// 异常：数组 / 内联表 / 数字三种非单值形态一律 `invalidValue`，且不产出元素
/// （单文件解析全或无，绝不静默取首条）。
#[test]
fn 解析doc非单值形态归invalid_value() {
    for (index, fragment) in ["doc = [\"a.md\"]", "doc = { path = \"a.md\" }", "doc = 1"]
        .into_iter()
        .enumerate()
    {
        let source = format!("[element.\"App\"]\nkind = \"package\"\n{fragment}\n");
        let diagnostic = single_diagnostic(&source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::InvalidValue,
            "第 {index} 形态（{fragment}）应归一值域违例：{}",
            diagnostic.message
        );
        let outcome = parse(&source);
        assert!(
            outcome.model.elements.is_empty(),
            "解析失败即空模型（{fragment}）"
        );
    }
}

/// 边界：省略 `doc` → `None`；`doc = ""` → `Some("")`；超长单值与含 `\n` / emoji 的转义
/// 值逐字保留；`path` 空 + `doc` 非空合法。
#[test]
fn 解析doc边界面() {
    let omitted = parse("[element.\"App\"]\nkind = \"package\"\n");
    assert!(omitted.diagnostics.is_empty(), "省略键零诊断");
    assert_eq!(
        omitted.model.element("App").expect("元素在案").doc,
        None,
        "省略 `doc` 即 `None`"
    );

    let empty = parse("[element.\"App\"]\nkind = \"package\"\ndoc = \"\"\n");
    assert!(empty.diagnostics.is_empty(), "空串零诊断");
    assert_eq!(
        empty.model.element("App").expect("元素在案").doc.as_deref(),
        Some(""),
        "空串逐字保留（形态判据归 `api::paths`）"
    );

    let long = format!("guides/{}.md", "a".repeat(1200));
    let source = format!("[element.\"App\"]\nkind = \"package\"\ndoc = \"{long}\"\n");
    let outcome = parse(&source);
    assert!(outcome.diagnostics.is_empty(), "超长值零诊断");
    let element = outcome.model.element("App").expect("元素在案");
    assert_eq!(
        element.doc.as_deref(),
        Some(long.as_str()),
        "超长单值逐字保留"
    );
    assert!(element.path.is_empty(), "`path` 空 + `doc` 非空合法");

    let escaped = parse("[element.\"App\"]\nkind = \"package\"\ndoc = \"docs/行1\\n行2🚀.md\"\n");
    assert!(escaped.diagnostics.is_empty(), "转义值零诊断");
    assert_eq!(
        escaped
            .model
            .element("App")
            .expect("元素在案")
            .doc
            .as_deref(),
        Some("docs/行1\n行2🚀.md"),
        "转义后的原文逐字保留"
    );
}

/// 正向：`doc` 在白名单内（含 `doc` 的源零诊断），其余键的解析结果零变化。
#[test]
fn 解析doc键在白名单内() {
    let outcome = parse(
        "[element.\"App.Auth\"]\n\
         kind = \"domain\"\ngate = \"strict\"\napi = \"none\"\n\
         summary = \"认证域\"\npath = [\"./src/auth/\"]\n\
         doc = \"guides/auth.md\"\nmetadata = { owner = \"core\" }\n",
    );
    assert!(
        outcome.diagnostics.is_empty(),
        "含 `doc` 的源零诊断：{:?}",
        outcome.diagnostics
    );
    let element = outcome.model.element("App.Auth").expect("元素在案");
    assert_eq!(element.kind, ElementKind::Domain, "kind 零变化");
    assert_eq!(element.gate, GateTier::Strict, "gate 零变化");
    assert_eq!(element.summary.as_deref(), Some("认证域"), "summary 零变化");
    assert_eq!(element.path, vec!["./src/auth/".to_owned()], "path 零变化");
    assert_eq!(
        element.metadata.get("owner").map(String::as_str),
        Some("core"),
        "metadata 零变化"
    );
}

/// 异常（AC-1 反向判据）：旧键 `docs` 与两个拼错形态 → `unknownField`（停摆形态真实
/// 可复现，不静默忽略）。
#[test]
fn 解析旧docs键与拼错键归unknown_field() {
    for (index, fragment) in [
        "docs = [\"./README.md\"]",
        "dc = \"a.md\"",
        "docss = [\"a.md\"]",
    ]
    .into_iter()
    .enumerate()
    {
        let source = format!("[element.\"App\"]\nkind = \"package\"\n{fragment}\n");
        let diagnostic = single_diagnostic(&source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::UnknownField,
            "第 {index} 形态（{fragment}）应归未知字段：{}",
            diagnostic.message
        );
        assert!(
            diagnostic.line >= 1,
            "诊断行号指向出错行：{:?}",
            diagnostic.line
        );
    }
}

/// 边界：白名单与结构体字段同步换面（源码自证）——`toml.rs` 内 `docs` 零命中、`doc`
/// 至少两处（结构体字段 + 传递点），面包屑不留旧键。
#[test]
fn 解析doc白名单源码自证() {
    let source = include_str!("toml.rs");
    assert!(
        !source.contains("docs"),
        "白名单与结构体字段同步清除旧键（`toml.rs` 内 `docs` 零命中）"
    );
    assert!(
        source.matches("doc").count() >= 2,
        "`doc` 在结构体字段与传递点各一处以上：{}",
        source.matches("doc").count()
    );
    assert!(source.contains("doc: Option<String>"), "字段面为单值可选");
}

// ===========================================================================
// `[[relation]].apis`（`relation-api-dependency-check` / AC-2 / AC-3 / D3 / D4）
//
// 第四键的值面五拒（空串 / 含空白 / 含 `::` / 同名重复 / 显式空数组）在 `to_model`
// 期产出带位置的 `invalidValue`——serde 层只有类型错的 span，拿不到逐项位置。
// 缺省（未写该键）与显式空数组必须可辨：前者 = 不声明 API 粒度（零诊断），后者 = 值
// 面违例。
// ===========================================================================

/// 单条关系源的便捷面（`from` / `to` 固定，尾巴由 `tail` 给出）。
fn relation_source(tail: &str) -> String {
    format!("[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Billing\"\n{tail}")
}

/// 正向：`apis = ["AgentInput", "SessionPanel"]` → 逐名入模型且**原文序**（零排序、
/// 零去重），零诊断。
#[test]
fn 解析apis逐名原文序零诊断() {
    let source = relation_source("apis = [\"AgentInput\", \"SessionPanel\"]\n");
    let outcome = parse(&source);
    assert!(
        outcome.diagnostics.is_empty(),
        "健康 apis 零诊断：{:?}",
        outcome.diagnostics
    );
    assert_eq!(
        outcome.model.relations[0].apis,
        vec!["AgentInput".to_owned(), "SessionPanel".to_owned()],
        "名字逐项入模型且保原文序"
    );
}

/// 正向：省略 `apis` → `Relation.apis` 为空向量且零诊断（缺省不产诊断）。
#[test]
fn 解析apis缺省为空且零诊断() {
    let outcome = parse(&relation_source(""));
    assert!(outcome.model.relations[0].apis.is_empty(), "缺省即空");
    assert!(
        outcome.diagnostics.is_empty(),
        "缺省不是违例：{:?}",
        outcome.diagnostics
    );
}

/// 边界：单元素数组与「标量形态」互为区分的两侧（标量形态另见类型面违例表）。
#[test]
fn 解析apis单元素与标量形态可辨() {
    let outcome = parse(&relation_source("apis = [\"AgentInput\"]\n"));
    assert_eq!(
        outcome.model.relations[0].apis,
        vec!["AgentInput".to_owned()],
        "单元素数组是一名"
    );
    assert!(outcome.diagnostics.is_empty(), "单元素不是违例");
}

/// 边界：显式空数组 → 恰一条 `invalidValue`，文案点名「空数组与省略同义」，位置落在
/// 该关系（`from` 键行）而非文件首行。
#[test]
fn 解析apis显式空数组归invalid_value() {
    let source = relation_source("apis = []\n");
    let diagnostic = single_diagnostic(&source);
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::InvalidValue,
        "空数组是值面违例"
    );
    assert!(
        diagnostic.message.contains("空数组与省略同义"),
        "文案点名纪律：{}",
        diagnostic.message
    );
    assert_eq!(
        diagnostic.line,
        line_at(&source, "from ="),
        "位置落在该关系声明的键行"
    );
}

/// 边界：超长名（> 1000 chars、无空白无 `::`）→ 零诊断（值面只四查，不设长度上限）。
#[test]
fn 解析apis超长名零诊断() {
    let long = "N".repeat(1200);
    let source = relation_source(&format!("apis = [\"{long}\"]\n"));
    let outcome = parse(&source);
    assert!(
        outcome.diagnostics.is_empty(),
        "长度不是值面判据：{:?}",
        outcome.diagnostics
    );
    assert_eq!(outcome.model.relations[0].apis, vec![long], "原样入模型");
}

/// 边界：特殊字符名（emoji / 引号）无空白无 `::` 即通过并原样入模型。
#[test]
fn 解析apis特殊字符名原样通过() {
    for name in ["Agent🙂", "a\"b"] {
        let escaped = name.replace('"', "\\\"");
        let source = relation_source(&format!("apis = [\"{escaped}\"]\n"));
        let outcome = parse(&source);
        assert!(
            outcome.diagnostics.is_empty(),
            "{name} 不是值面违例：{:?}",
            outcome.diagnostics
        );
        assert_eq!(
            outcome.model.relations[0].apis,
            vec![name.to_owned()],
            "{name} 原样入模型"
        );
    }
}

/// 异常：空串名 → 恰一条 `invalidValue`，文案点名「只写名字」，`line` / `column` 指向
/// 该名字面（不是关系首键行）。
#[test]
fn 解析apis空串名归invalid_value带位置() {
    let source = relation_source("apis = [\"\"]\n");
    let diagnostic = single_diagnostic(&source);
    assert_eq!(diagnostic.kind, DiagnosticKind::InvalidValue);
    assert!(
        diagnostic.message.contains("只写名字"),
        "文案点名「只写名字」：{}",
        diagnostic.message
    );
    assert_eq!(
        diagnostic.line,
        line_at(&source, "apis ="),
        "位置指向该名字所在行"
    );
    assert!(
        diagnostic.column.is_some(),
        "逐项 span 换算列：{:?}",
        diagnostic.column
    );
}

/// 异常：含空白三形态（空格 / 制表 / 换行）各产出恰一条 `invalidValue`。
#[test]
fn 解析apis含空白三形态各一条() {
    for (probe, tuple) in [
        ("a b", "\"a b\""),
        ("a\tb", "\"a\\tb\""),
        ("a\nb", "\"a\\nb\""),
    ] {
        let source = relation_source(&format!("apis = [{tuple}]\n"));
        let diagnostic = single_diagnostic(&source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::InvalidValue,
            "{probe:?} 是值面违例"
        );
        assert!(
            diagnostic.message.contains("含空白"),
            "{probe:?} 的文案点名空白纪律：{}",
            diagnostic.message
        );
    }
}

/// 异常：含 `::` 两形态（相对 / `crate::` 前缀）各产出恰一条 `invalidValue`（路径形态
/// 不入值面）。
#[test]
fn 解析apis含双冒号两形态各一条() {
    for name in ["a::b", "crate::A"] {
        let source = relation_source(&format!("apis = [\"{name}\"]\n"));
        let diagnostic = single_diagnostic(&source);
        assert_eq!(diagnostic.kind, DiagnosticKind::InvalidValue);
        assert!(
            diagnostic.message.contains("``::``") || diagnostic.message.contains("`::`"),
            "文案点名 `::` 纪律：{}",
            diagnostic.message
        );
    }
}

/// 异常：同名重复 `["A", "A"]` → 恰**一条** `invalidValue`（不静默去重、不按重复次数
/// 铺开），且模型内两名原样保留（归一留给判定侧）。
#[test]
fn 解析apis同名重复恰一条() {
    let source = relation_source("apis = [\"A\", \"A\"]\n");
    let diagnostic = single_diagnostic(&source);
    assert_eq!(diagnostic.kind, DiagnosticKind::InvalidValue);
    assert!(
        diagnostic.message.contains("重复"),
        "文案点名重复纪律：{}",
        diagnostic.message
    );
    assert_eq!(
        outcome_apis(&source),
        vec!["A".to_owned(), "A".to_owned()],
        "值面不静默去重（模型原文保留）"
    );
}

/// 边界：诊断文案含关系上下文（`from → to` 定位串）——五类违例各一条，均可回指该条
/// 关系（不是无名报错）。
#[test]
fn 解析apis诊断文案含关系上下文() {
    for tail in [
        "apis = [\"\"]\n",
        "apis = [\"a b\"]\n",
        "apis = [\"a::b\"]\n",
        "apis = [\"A\", \"A\"]\n",
        "apis = []\n",
    ] {
        let diagnostic = single_diagnostic(&relation_source(tail));
        assert!(
            diagnostic.message.contains("App.Auth")
                && diagnostic.message.contains("App.Billing")
                && diagnostic.message.contains("→"),
            "{tail:?} 的文案须含关系两端（from → to）：{}",
            diagnostic.message
        );
    }
}

/// 边界：同一关系内「重复 + 含空白」叠加 → 两条纪律各被点名（逐项 `continue`，不因
/// 首条即返回而漏掉第二条纪律）；两种先后序都成立。
#[test]
fn 解析apis重复与含空白叠加各点名() {
    for tail in [
        "apis = [\"A\", \"A\", \"b c\"]\n",
        "apis = [\"b c\", \"A\", \"A\"]\n",
    ] {
        let outcome = parse(&relation_source(tail));
        assert_eq!(
            outcome.diagnostics.len(),
            2,
            "{tail:?} 两条违例两条诊断：{:?}",
            outcome.diagnostics
        );
        assert!(
            outcome
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.kind == DiagnosticKind::InvalidValue),
            "{tail:?} 均归 invalidValue"
        );
        let messages: Vec<&str> = outcome
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect();
        assert!(
            messages.iter().any(|message| message.contains("重复")),
            "{tail:?} 点名重复纪律：{messages:?}"
        );
        assert!(
            messages.iter().any(|message| message.contains("含空白")),
            "{tail:?} 点名空白纪律：{messages:?}"
        );
    }
}

/// 异常：类型面违例（标量 / 非字符串项）→ serde `invalid type` 归 `invalidValue` 且
/// `Model::empty()`（整体失败不取值面）。
#[test]
fn 解析apis类型面违例归invalid_value并空模型() {
    for tail in ["apis = \"A\"\n", "apis = [1, 2]\n"] {
        let source = relation_source(tail);
        let diagnostic = single_diagnostic(&source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::InvalidValue,
            "{tail:?} 归 invalidValue"
        );
        assert!(
            diagnostic.message.contains("invalid type"),
            "文案为 serde 原文：{}",
            diagnostic.message
        );
        assert!(
            parse(&source).model.relations.is_empty(),
            "整体失败 ⇒ 零关系（Model::empty 形态）"
        );
    }
}

/// 边界：拼错键 `api = ["A"]`（单数）→ 仍 `unknownField`（`deny_unknown_fields` 逐字
/// 保留，不因新增第四键而放宽）。
#[test]
fn 解析关系拼错键api归unknown_field() {
    let source = relation_source("api = [\"A\"]\n");
    let diagnostic = single_diagnostic(&source);
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::UnknownField,
        "单数键仍是未知键"
    );
    assert!(
        diagnostic.message.contains("unknown field"),
        "serde 原文在案：{}",
        diagnostic.message
    );
}

/// 边界：多条关系各带 `apis`——一条违例不影响其余关系入模型，诊断条数 = 违例条数
/// （不是「首错短路」）。
#[test]
fn 解析apis多关系违例不互相影响() {
    let single = concat!(
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Billing\"\napis = [\"A\"]\n\n",
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Legacy\"\napis = [\"\"]\n",
    );
    let outcome = parse(single);
    assert_eq!(outcome.diagnostics.len(), 1, "单违例源恰一条诊断");
    assert_eq!(outcome.model.relations.len(), 2, "违例不影响其余关系入模型");
    assert_eq!(outcome.model.relations[0].apis, vec!["A".to_owned()]);
    assert_eq!(outcome.model.relations[1].apis, vec![String::new()]);

    let double = concat!(
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Billing\"\napis = [\"\"]\n\n",
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Legacy\"\napis = [\"a::b\"]\n",
    );
    let outcome = parse(double);
    assert_eq!(
        outcome.diagnostics.len(),
        2,
        "双违例源两条诊断（不首错短路）"
    );
    assert_eq!(
        outcome.model.relations.len(),
        2,
        "两条关系仍都入模型（值面违例不吞模型）"
    );
}

/// 边界：关系缺 `to`（必填面）→ 既有 `missingField` 语义不变（`apis` 不改变必填面）。
#[test]
fn 解析apis不改必填面() {
    let source = "[[relation]]\nfrom = \"App.Auth\"\napis = [\"A\"]\n";
    let diagnostic = single_diagnostic(source);
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::MissingField,
        "缺 to 仍归 missingField"
    );
}

/// 正向：健康源（含 `apis`）→ 诊断空、关系逐条齐备（`ParseOutcome` 两字段形状不变）。
#[test]
fn 解析含apis健康源诊断空且关系齐备() {
    let source = concat!(
        "[element.\"App.Auth\"]\nkind = \"domain\"\n\n",
        "[element.\"App.Billing\"]\nkind = \"module\"\n\n",
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Billing\"\napis = [\"A\", \"B\"]\n",
    );
    let outcome = parse(source);
    assert!(outcome.diagnostics.is_empty(), "健康源零诊断");
    assert_eq!(outcome.model.elements.len(), 2);
    assert_eq!(outcome.model.relations.len(), 1);
    assert_eq!(
        outcome.model.relations[0].apis,
        vec!["A".to_owned(), "B".to_owned()]
    );
}

/// 边界：语法错源（缺括号 / 重复键）→ **恰一条** serde 诊断且零关系（合并面不追加
/// 值面诊断）。
#[test]
fn 解析语法错源单诊断且不追加值面() {
    for broken in [
        "[[relation]\nfrom = \"A\"\nto = \"B\"\napis = [\"\"]\n",
        "[[relation]]\nfrom = \"A\"\nfrom = \"C\"\nto = \"B\"\napis = [\"\"]\n",
    ] {
        let outcome = parse(broken);
        assert_eq!(
            outcome.diagnostics.len(),
            1,
            "语法面整体失败只出一条诊断：{:?}",
            outcome.diagnostics
        );
        assert!(
            outcome.model.relations.is_empty(),
            "整体失败 ⇒ 零关系（不取值面）"
        );
    }
}

/// 异常：值面多违例（两条关系各一违例）→ 两条诊断，`file` 留空（聚合层回填），
/// 各自 `line` 独立。
#[test]
fn 解析apis多违例file留空行号独立() {
    let source = concat!(
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Billing\"\napis = [\"\"]\n\n",
        "[[relation]]\nfrom = \"App.Billing\"\nto = \"App.Legacy\"\napis = [\"A\", \"A\"]\n",
    );
    let outcome = parse(source);
    assert_eq!(outcome.diagnostics.len(), 2, "两条违例两条诊断");
    for diagnostic in &outcome.diagnostics {
        assert_eq!(diagnostic.kind, DiagnosticKind::InvalidValue);
        assert_eq!(diagnostic.file, "", "file 由聚合层回填");
        assert!(diagnostic.line > 0, "位置在案：{:?}", diagnostic.line);
    }
    assert_ne!(
        outcome.diagnostics[0].line, outcome.diagnostics[1].line,
        "两条违例各自行号独立（不共用首键行）"
    );
}

/// 边界：同文件两条关系（一条全字段带 `apis`、一条仅三键）→ 各自独立解析、
/// `apis` 分别非空 / 空。
#[test]
fn 解析apis同文件两关系各自独立() {
    let source = concat!(
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Billing\"\ndesc = \"认证依赖结算\"\n",
        "apis = [\"Token\", \"Session\"]\n\n",
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Legacy\"\n",
    );
    let outcome = parse(source);
    assert!(outcome.diagnostics.is_empty(), "两条均合法");
    assert_eq!(
        outcome.model.relations[0].apis,
        vec!["Token".to_owned(), "Session".to_owned()]
    );
    assert!(outcome.model.relations[1].apis.is_empty(), "仅三键者为零");
    assert_eq!(
        outcome.model.relations[0].description.as_deref(),
        Some("认证依赖结算"),
        "既有 desc 面零变化"
    );
}

/// 辅助：取单条关系源的 `apis`（模型侧原文）。
fn outcome_apis(source: &str) -> Vec<String> {
    parse(source).model.relations[0].apis.clone()
}
