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
    "docs = [\"./src/auth/README.md\"]\n",
    "summary = \"认证域\"\n",
    "metadata = { owner = \"core\", tier = \"1\" }\n",
    "\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "gate = \"warn\"\n",
    "\n",
    "[element.\"App.Legacy\"]\n",
    "kind = \"component\"\n",
    "gate = \"off\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Auth\"\n",
    "to = \"App\"\n",
    "desc = \"认证依赖父包\"\n",
);

/// 正向：proposal 示例全字段 → 元素 / 关系逐字段等于期望；省略 `gate` 的元素取
/// `Off`；缺省 `path` / `docs` 为空数组、`metadata` 为空表。
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
    assert!(app.docs.is_empty(), "缺省 docs 为空数组");
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
    assert_eq!(auth.docs, vec!["./src/auth/README.md".to_owned()]);
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
        ElementKind::Component
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

/// 异常：`invalidValue`——非法 `kind` 列出四个合法值，非法 `gate` 列出三个合法值。
#[test]
fn 非法枚举值归_invalid_value_且列出全部合法值() {
    let kind_diagnostic = single_diagnostic("[element.\"App\"]\nkind = \"pakage\"\n");
    assert_eq!(kind_diagnostic.kind, DiagnosticKind::InvalidValue);
    for legal in ["package", "domain", "module", "component"] {
        assert!(
            kind_diagnostic.message.contains(legal),
            "kind 错误文案应列出合法值 {legal}: {}",
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

/// 异常：`metadata` 非字符串值 → 拒绝且位置在案（按 spec「非字符串值即
/// `invalidValue`」与 D19 的值域判据：serde 的 `invalid type` 与前缀分类学的
/// `invalidValue` 同档，文案与位置零加工）；子表形态
/// `[element."App.Auth".metadata]` 无法构成合法元素（缺 `kind`）→ `missingField`，
/// 且不产出任何元素（单文件解析全或无）。
#[test]
fn metadata非字符串值被拒且子表形态不静默成元数据() {
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

    // 子表形态：被读成元素 App.Auth.metadata 的父表，缺 kind 即 missingField
    let diagnostic = single_diagnostic(
        "[element.\"App\"]\nkind = \"package\"\n\n[element.\"App.Auth\".metadata]\nowner = \"core\"\n",
    );
    assert_eq!(diagnostic.kind, DiagnosticKind::MissingField);
    let outcome = parse(
        "[element.\"App\"]\nkind = \"package\"\n\n[element.\"App.Auth\".metadata]\nowner = \"core\"\n",
    );
    assert!(
        outcome.model.elements.is_empty(),
        "解析失败即全或无：不静默保留半个模型"
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
// 边界：str / int / 规模
// ---------------------------------------------------------------------------

/// 边界：`str` 值边界——超长（>1000 chars）摘要逐字保留；`\n` / `\u0000` / emoji
/// 转义值逐字保留；`path` / `docs` 各 1 项与各 200 项均正确。
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
            "docs = [\"./a/README.md\"]\n",
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
    assert_eq!(app.docs, vec!["./a/README.md".to_owned()], "单元素 docs");

    // 200 项 path / docs
    let items = (0..200)
        .map(|index| format!("\"./p{index:03}/md.md\""))
        .collect::<Vec<String>>()
        .join(", ");
    let source =
        format!("[element.\"App\"]\nkind = \"package\"\npath = [{items}]\ndocs = [{items}]\n");
    let outcome = parse(&source);
    assert!(outcome.diagnostics.is_empty());
    let app = outcome.model.element("App").expect("App 在模型内");
    assert_eq!(app.path.len(), 200, "200 项 path 全量保留");
    assert_eq!(app.docs.len(), 200, "200 项 docs 全量保留");
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
