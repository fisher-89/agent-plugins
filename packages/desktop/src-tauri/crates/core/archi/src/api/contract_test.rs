//! `api::contract` 的单元测试：`verify` 三闸（七族名字 / `x-function` 落点封闭四处 /
//! `required ⊆ properties`）与构造器封闭集判据（`is_constructor`）。
//!
//! 零 mock：`verify` 吃「投影 + schema 值」两个纯值入参（零落盘、零读盘）；
//! `Contract` 由 `Contract::from_schema` 投影，schema 由手写 `json!` 给（`required` /
//! `$defs` / `x-function` 的反例直接注入）。

use serde_json::Value;

use super::contract::{is_constructor, verify, Contract, X_CONSTRUCTOR, X_STATIC};
use super::ApiError;
use crate::api::mod_test::mirrored_schema_text;

/// 契约文件（repo 相对 POSIX；错误文案的 `file:` 前缀来源）。
const FILE: &str = ".dev-team/architecture/models/App.Demo.Item.md";

/// 元素 FQN（= H1 标题 = 文件名去 `.md`）。
const FQN: &str = "App.Demo.Item";

/// 手写接口定义原文 → `Contract` 投影（夹具构造在 `emit/mod_test`，产品读面只认
/// `.schema.json`）。
fn parse(source: &str, fqn: &str, file: &str) -> Contract {
    crate::api::mod_test::prepare(source, fqn, file).0
}

/// 契约 → schema 值（判据面取 `$defs` / `properties` 两表；本测试的取值
/// 适配层，产品侧不存在同名函数）。
fn schema_of(contract: &Contract, _meta: &crate::api::ApiMeta) -> Result<Value, ApiError> {
    Ok(crate::api::mod_test::schema_of(contract))
}

/// 断言 `verify` 报契约错误并返回文案。
fn verify_error(contract: &Contract, schema: &Value) -> String {
    match verify(contract, schema) {
        Ok(()) => panic!("期望双向 gate 报错，实际 Ok"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// is_constructor：封闭集（精确六名 + 两前缀）
// ---------------------------------------------------------------------------

/// 正向（AC-13 / D11）：六个精确名 + `from_` / `with_` 前缀（含空前缀后无字符的
/// 边界）→ 全 `true`。
#[test]
fn is_constructor命中集() {
    let expected = [
        "new",
        "default",
        "build",
        "empty",
        "never",
        "open",
        "from_str",
        "from_",
        "with_capacity",
        "with_",
    ];
    for name in expected {
        assert!(is_constructor(name), "构造器封闭集应命中：{name:?}");
    }
    assert_eq!(expected.len(), 10, "六精确 + 两前缀（各两形态）");
}

/// 异常：`file_stem` / `compose_text` / `parse` / `run` / `fromage` / `newer` /
/// `without_x` / `default_impl` → 全 `false`（前缀判据须匹配 `from_` / `with_`
/// 字面量，不做词干启发）。
#[test]
fn is_constructor未命中集() {
    for name in [
        "file_stem",
        "compose_text",
        "parse",
        "run",
        "fromage",
        "newer",
        "without_x",
        "default_impl",
    ] {
        assert!(!is_constructor(name), "非构造器名：{name:?}");
    }
}

/// 边界：大小写与空白不归一（`NEW` / `New` / `"  new  "` / 空串 → `false`）；
/// 超长输入不 panic。
#[test]
fn is_constructor词形边界() {
    for name in ["", "NEW", "New", "  new  ", "New "] {
        assert!(!is_constructor(name), "封闭集精确匹配、零归一：{name:?}");
    }
    assert!(!is_constructor(&"n".repeat(1200)), "超长输入不 panic");
}

// ---------------------------------------------------------------------------
// 辅助
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// fence 内重复键在文本层判（D37）
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 契约格式留白：模型 `entry` 字段不扩契约格式
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 闸一：七族名字的 ASCII 标识符判据（D45 / AC-23 ①）
// ---------------------------------------------------------------------------

/// 取源码内顶层函数体（自 `signature` 起的花括号配对区段；未命中即断言失败）。
fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("源码内应有 `{signature}`"));
    let open = start + source[start..].find('{').expect("函数体起手花括号");
    let mut depth = 0usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open..open + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("`{signature}` 的函数体未闭合");
}

/// 产出**落点错误文案**的顶层函数名清单（判据侧不绑死实现形态：实现可把守卫拆成多层、
/// 也可改名——只要「产落点文案的函数」都在 `verify` 体内被调用即合格）。
///
/// 判定形态：逐个**顶层**定义行（列 0 处的 `fn <name>(`）取其函数体，体内含定形族别串
/// `x-function 落点` 或 `JSON pointer` 即为守卫链成员；嵌套闭包 / 内联块不在扫描面。
fn position_guards(source: &str) -> Vec<String> {
    let mut guards: Vec<String> = Vec::new();
    for line in source.lines() {
        // 只认列 0 的顶层定义行（缩进行是体内语句，不在扫描面）
        if !line.starts_with("fn ") && !line.starts_with("pub fn ") {
            continue;
        }
        let Some(name) = fn_name_of(line) else {
            continue;
        };
        let signature = format!("fn {name}(");
        if source.find(&signature).is_none() {
            continue;
        }
        let body = function_body(source, &signature);
        if body.contains("x-function 落点") || body.contains("JSON pointer") {
            guards.push(name);
        }
    }
    guards
}

/// 从 `body` 出发的调用闭包（逐个标识符 + `(` 的调用形态，递归展开被调函数的函数体）。
fn reachable_calls(source: &str, body: &str) -> Vec<String> {
    let mut reached: Vec<String> = Vec::new();
    let mut queue: Vec<&str> = vec![body];
    while let Some(current) = queue.pop() {
        for (index, _) in current.match_indices('(') {
            let head: String = current[..index]
                .chars()
                .rev()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect::<Vec<char>>()
                .into_iter()
                .rev()
                .collect();
            if head.is_empty() || reached.contains(&head) {
                continue;
            }
            let signature = format!("fn {head}(");
            if !source.contains(&signature) {
                continue;
            }
            reached.push(head);
            let callee = function_body(source, &signature);
            queue.push(callee);
        }
    }
    reached
}

/// 单行内的 `fn <name>` 定义名（无定义即 `None`）。
fn fn_name_of(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("pub fn ")
        .or_else(|| trimmed.strip_prefix("fn "))?;
    let name: String = rest
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

// ---------------------------------------------------------------------------
// 落点闸：`x-function` 的合法落点封闭四处（D47 / AC-25）
// ---------------------------------------------------------------------------

/// 边界（**AC-25 ①** / D47）：实现形态二选一、判据不绑死——**硬判据只取三条**：
/// ① 定形族别串 `x-function 落点` 在产品侧 `contract.rs` 内 ≥1；
/// ② pointer 文案 `JSON pointer` ≥1；③ **守卫函数的调用点在 `verify` 函数体内**
/// （落点闸与七族闸一同时机、MUST NOT 拆第二个公开入口）。
/// 容器词（`items` / `additionalProperties` / `oneOf` / `anyOf`）只作**留痕面**，
/// **MUST NOT** 作硬判据——父级白名单式实现下它恒 0、实现正确也恒红。
#[test]
fn verify落点闸判据不绑死实现形态() {
    let source = include_str!("contract.rs");
    assert!(
        source.matches("x-function 落点").count() >= 1,
        "定形族别串 `x-function 落点` 在场（判据锚它，MUST NOT 靠散文近似措辞）"
    );
    assert!(
        source.matches("JSON pointer").count() >= 1,
        "pointer 文案在场"
    );

    // ③ 守卫的调用点在 `verify` 体内（**唯一硬判据**，且不绑死函数名——凡产出落点错误
    // 文案的函数（守卫链）都必须在 `verify` 体内被调用；实现可改名 / 拆分，行为面不变）
    let verify_body = function_body(source, "pub fn verify(");
    let guards = position_guards(source);
    assert!(
        !guards.is_empty(),
        "落点闸的守卫链在案（产 `x-function 落点` 文案的函数 ≥ 1）"
    );
    // 可达性判据（不绑死分层）：从 `verify` 体内出发求调用闭包，每个产落点文案的函数都
    // 必须**可达**——实现可把守卫拆成多层（`verify` → 守卫 → 扫描 → 错误构造），只要整条
    // 链从 `verify` 体内起手即合格；不可达即意味着另立了第二个入口。
    let reachable = reachable_calls(source, verify_body);
    for guard in &guards {
        assert!(
            reachable.contains(guard),
            "守卫 `{guard}` 自 `verify` 体内可达（同函数、同趟收口）：已触及 {reachable:?}"
        );
    }
    // 公开面未因此扩张（`pub fn` 仍为三）：落点闸并入既有 `verify`、零新增导出
    assert_eq!(
        source.matches("\npub fn ").count(),
        3,
        "`contract.rs` 的公开面仍三处（`parse` / `verify` / `is_constructor`；落点闸零新增导出）"
    );
    // 树内零守卫函数定义在 `verify` 之外被调用（`schema.rs` 看不到 `api-op` 顶层签名）
    let schema_source = include_str!("schema.rs");
    assert!(
        !schema_source.contains("verify_x_function_positions"),
        "落点闸 MUST NOT 落 `schema::verify_defs`（它看不到 `api-op` 顶层签名）"
    );

    // 留痕面（不作硬判据）：容器黑名单式实现下四容器词应在场
    // 留痕面（**刻意不写成硬判据**）：容器黑名单式实现下四容器词应在场；父级白名单式
    // 实现下它们可为 0——两条路径行为面等价，任一形态都不得判红，故此处只核一条共同的
    // 事实：守卫的判据是「父容器是否合法」而非「容器词是否出现」。
    let container_words = ["items", "additionalProperties", "oneOf", "anyOf"];
    let present: usize = container_words
        .iter()
        .filter(|word| source.contains(**word))
        .count();
    assert!(
        source.contains("只许出现在") || source.contains("MUST NOT"),
        "守卫的文案点明「合法落点封闭」而非容器词枚举（容器词计数仅作留痕，实际 {present} 个）"
    );
}

/// 边界（**AC-25 ③** / D47）：收集面不扩、零递归——`fn function_param_names` 恰 1 处定义、
/// 调用点恰 2 处（`op.signature` 与 `methods_of` 的顶层 `properties` 值）⇒ 带括号计数
/// 恰 **3**；`methods_of(` 恰 **2**（定义 + `verify` 内唯一调用点）。**增至 3 处调用即
/// 红**——那意味着改成递归遍历任意深度，与 D47「把射程降成可穷举的四处」相反。
#[test]
fn verify落点闸收集面不扩零递归() {
    let source = include_str!("contract.rs");
    assert_eq!(
        source.matches("fn function_param_names").count(),
        1,
        "`function_param_names` 恰 1 处定义"
    );
    assert_eq!(
        source.matches("function_param_names(").count(),
        3,
        "带括号计数恰 3 = 定义 1 + 调用 2（`op.signature` 与 `methods_of` 的顶层 `properties` 值）"
    );
    assert_eq!(
        source.matches("methods_of(").count(),
        2,
        "`methods_of(` 恰 2 处（定义 + `verify` 内唯一调用点；带括号计数以避开 doc 链接）"
    );

    // 两处调用点都在 `verify` 函数体内（不散落到别处）：
    // ① `op.signature`（`api-op` 顶层签名）② `methods_of` 的顶层 `properties` 值
    let verify_body = function_body(source, "pub fn verify(");
    assert_eq!(
        verify_body.matches("function_param_names(").count(),
        2,
        "收集点的两处调用都在 `verify` 体内（`op.signature` + `methods_of` 的顶层 `properties` 值）"
    );
    assert_eq!(
        verify_body.matches("methods_of(").count(),
        1,
        "`methods_of` 在 `verify` 体内恰 1 处调用（`$defs` 条目第一层 `properties` 值的唯一入口）"
    );
}

// ---------------------------------------------------------------------------
// Contract 字段面（D51 / AC-28）与闸一合法面补充（D45 边界）
// ---------------------------------------------------------------------------

/// 边界（**D59** 与本文件的交汇）：`contract.rs` 内 `choice_payload` **零命中**（该私有
/// 函数已整条删除，⑦ `$ref` 末段名族的注释改指 `super::ref_name`）；`contract_test.rs` 的同一
/// 处注释词面同批改指（归测试相位，D53 第四类归属二分）；七族族别串本身**逐字不变**
/// （判据面不动，只改注释里的函数名）。
#[test]
fn choice_payload词面零残留() {
    let source = include_str!("contract.rs");
    assert_eq!(
        source.matches("choice_payload").count(),
        0,
        "`contract.rs` 内零 `choice_payload`（D59 已删该函数，注释改指 `value_type`）"
    );
    assert!(
        source.contains("super::ref_name"),
        "注释词面改指 `super::ref_name`（同一映射单点）"
    );
    // 七族族别串逐字不变（判据面不动）
    for family in [
        "$defs 键名",
        "透传导出名",
        "参数名",
        "properties 数据字段键",
        "owner 键",
        "$ref 末段名",
    ] {
        assert!(
            source.contains(family),
            "族别串 `{family}` 逐字不变（判据面不动）"
        );
    }
}

// ---------------------------------------------------------------------------
// 自洽闸：`required` ⊆ `properties`（AC-4 / D2）
// ---------------------------------------------------------------------------

/// 单条目 `api-types` 契约（`$defs.Widget` 取给定 JSON 原文）。
fn widget_contract(definition: &str) -> Contract {
    let source = mirrored_schema_text(FQN, format!("\"Widget\": {definition}"), "");
    parse(&source, FQN, FILE)
}

/// `$defs.Widget` 取给定原文的契约经 `schema_of` 投影后的 `verify` 结果。
fn verify_definition(definition: &str) -> Result<(), ApiError> {
    let contract = widget_contract(definition);
    let schema = schema_of(&contract, &super::meta()).expect("schema 应装配成功");
    verify(&contract, &schema)
}

/// 断言 `verify_definition` 报契约错误并返回文案。
fn verify_definition_error(definition: &str) -> String {
    match verify_definition(definition) {
        Ok(()) => panic!("期望自洽闸报错，实际 Ok：{definition}"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 正向（AC-4，表驱动）：`required ⊆ properties` 四形态——全字段必填 / 部分必填 /
/// `required = []` / 无 `required` → 全 `Ok`。
#[test]
fn verify自洽闸四种required形态全过() {
    let cases = [
        (
            "全字段必填",
            r#"{"type": "object", "properties": {"name": {"type": "string"}, "total": {"type": "integer"}}, "required": ["name", "total"]}"#,
        ),
        (
            "部分必填",
            r#"{"type": "object", "properties": {"name": {"type": "string"}, "total": {"type": "integer"}}, "required": ["name"]}"#,
        ),
        (
            "required 为空的数组",
            r#"{"type": "object", "properties": {"name": {"type": "string"}}, "required": []}"#,
        ),
        (
            "无 required 键",
            r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#,
        ),
    ];
    for (label, definition) in cases {
        assert_eq!(
            verify_definition(definition),
            Ok(()),
            "{label}：自洽契约应通过"
        );
    }
}

/// 正向（AC-6 判据面侧）：现网契约形态（`properties: {}` 且零 `required`）→ `Ok`，
/// 零误伤。
#[test]
fn verify自洽闸不误伤现网空属性形态() {
    assert_eq!(
        verify_definition(r#"{"type": "object", "properties": {}}"#),
        Ok(()),
        "`properties` 为空对象且零 `required` → Ok（现网契约形态）"
    );
    assert_eq!(
        verify_definition(r#"{"type": "object"}"#),
        Ok(()),
        "无 `properties` 且无 `required` → Ok（零键可判）"
    );
}

/// 异常（AC-4 关键反例）：`properties` 有 `name` 而 `required: ["nmae"]` → `Err`，文案
/// 含 `$defs.Widget.required[0]` 与悬垂名 `nmae`。
#[test]
fn verify自洽闸点名悬垂required的pointer() {
    let message = verify_definition_error(
        r#"{"type": "object", "properties": {"name": {"type": "string"}}, "required": ["nmae"]}"#,
    );
    assert!(
        message.contains("$defs.Widget.required[0]"),
        "文案点名 JSON pointer：{message}"
    );
    assert!(message.contains("nmae"), "文案点名悬垂元素：{message}");
    assert!(message.contains(FILE), "文案带文件定位：{message}");
    assert!(
        message.contains("required") && message.contains("properties"),
        "文案点明 `required` 与 `properties` 的闭合关系：{message}"
    );
    assert!(
        !message.contains("ASCII 标识符"),
        "集合关系错与名字形态错是两类错误面（MUST NOT 借用闸一措辞）：{message}"
    );
}

/// 异常（表驱动）：`required` 元素非字符串（`[42]` / `[true]` / `[null]`）→ `Err`，
/// 文案点名 `required[<i>]` 并经 `kind_of` 报形态（数字 / 布尔 / null）。
#[test]
fn verify自洽闸的required元素非字符串() {
    let cases = [
        (r#"[42]"#, "数字"),
        (r#"[true]"#, "布尔"),
        (r#"[null]"#, "null"),
    ];
    for (required, kind) in cases {
        let message = verify_definition_error(&format!(
            r#"{{"type": "object", "properties": {{"name": {{"type": "string"}}}}, "required": {required}}}"#
        ));
        assert!(
            message.contains("$defs.Widget.required[0]"),
            "{required}：文案点名 pointer：{message}"
        );
        assert!(
            message.contains(kind),
            "{required}：文案经 `kind_of` 报形态 `{kind}`：{message}"
        );
    }
}

/// 异常（表驱动）：条目缺 `properties` 而 `required` 非空 → `Err` 且文案含
/// `required[0]`；首层属性键 `data-testid` → `Err` 且文案含
/// `$defs.Widget.properties.data-testid` 与「`data-testid` 一类键不进契约」出路句。
#[test]
fn verify自洽闸缺属性与非法属性键() {
    let missing = verify_definition_error(r#"{"type": "object", "required": ["name"]}"#);
    assert!(
        missing.contains("$defs.Widget.required[0]"),
        "缺 `properties` 时仍点名 pointer：{missing}"
    );

    let dotted = verify_definition_error(
        r#"{"type": "object", "properties": {"data-testid": {"type": "string"}}}"#,
    );
    assert!(
        dotted.contains("$defs.Widget.properties.data-testid"),
        "非法属性键点名 pointer：{dotted}"
    );
    assert!(
        dotted.contains("`data-testid` 一类键不进接口定义"),
        "文案带可执行出路句：{dotted}"
    );
    assert!(
        dotted.contains("properties 数据字段键"),
        "族别串在案（⑤ 族）：{dotted}"
    );
}

/// 异常（AC-4 边界）：同一 `$defs` 条目同时含非法属性键与悬垂 `required` → 报 ⑤ 族
/// （同趟先判名字、后判闭合性，错误面稳定不漂移）。
#[test]
fn verify非法属性键先于悬垂required() {
    let message = verify_definition_error(
        r#"{"type": "object", "properties": {"data-testid": {"type": "string"}}, "required": ["nmae"]}"#,
    );
    assert!(
        message.contains("properties 数据字段键"),
        "同趟先判名字（⑤ 族）：{message}"
    );
    assert!(
        message.contains("$defs.Widget.properties.data-testid"),
        "⑤ 族的 pointer 在案：{message}"
    );
    assert!(
        !message.contains("required[0]"),
        "后判的闭合性不抢先报（错误面稳定）：{message}"
    );
}

/// 边界（design 待决问题 1 的显式留白）：`required` 整体非数组（`"required": "name"`）
/// → 维持既有静默（`Ok`）——本期不加判据。
#[test]
fn verify自洽闸的required非数组维持静默() {
    assert_eq!(
        verify_definition(
            r#"{"type": "object", "properties": {"name": {"type": "string"}}, "required": "name"}"#
        ),
        Ok(()),
        "`required` 非数组不在本期判据面（显式留白，零误报）"
    );
}

/// 边界：`required` 指向 `x-function` 方法名（键在同条目 `properties` 全集内）→ `Ok`
/// （闭合性按 `properties` 键集判，与 ① 族方法名判据互不吞并）。
#[test]
fn verify自洽闸的方法名闭合() {
    assert_eq!(
        verify_definition(
            r#"{"type": "object", "properties": {"onAnswer": {"x-function": {"params": [], "returns": {"type": "boolean"}}}}, "required": ["onAnswer"]}"#
        ),
        Ok(()),
        "函数指针字段名在 `properties` 全集内即闭合"
    );

    let missing = verify_definition_error(
        r#"{"type": "object", "properties": {"onAnswer": {"x-function": {"params": [], "returns": {"type": "boolean"}}}}, "required": ["onAnswr"]}"#,
    );
    assert!(
        missing.contains("$defs.Widget.required[0]") && missing.contains("onAnswr"),
        "拼错的方法名仍报悬垂：{missing}"
    );
}

/// 边界（AC-4 零回归）：七族判据仍逐族在案（族别串齐备）、公开面仍恰 3 个 `pub fn`
/// （不新增族编号、不拆第二个公开入口）。
#[test]
fn verify七族与公开面零回归() {
    let source = include_str!("contract.rs");
    for family in [
        "op id",
        "$defs 键名",
        "透传导出名",
        "参数名",
        "properties 数据字段键",
        "owner 键",
        "$ref 末段名",
    ] {
        assert!(
            source.contains(family),
            "族别串 `{family}` 在案（七族不因自洽闸而增删）"
        );
    }
    assert_eq!(
        source.matches("\npub fn ").count(),
        3,
        "公开面仍恰 3 个 `pub fn`（parse / verify / is_constructor）"
    );
    assert_eq!(
        source.matches("pub fn verify(").count(),
        1,
        "自洽闸与七族闸、落点闸同函数（MUST NOT 拆第二个公开入口）"
    );
}
// ---------------------------------------------------------------------------
// Contract::from_schema：schema → 投影（R2 回归锚）
// ---------------------------------------------------------------------------

/// 正向（AC-1 / R2 回归锚）：夹具 schema 1 op（`x-function` 带 `owner` / `params` /
/// `returns`）⇒ `signature` 与 `x-function` 逐键相等（包裹态已解）、`line` 恒 `0`。
#[test]
fn 签名即x_function解包值() {
    let schema = serde_json::json!({
        "$defs": {},
        "properties": {
            "run": {
                "x-function": {
                    "description": "跑一次",
                    "owner": "Runner",
                    "params": [{"name": "input", "schema": {"type": "string"}}],
                    "returns": {"type": "boolean"}
                }
            }
        }
    });
    let contract = Contract::from_schema(FQN, FILE, &schema);
    assert_eq!(contract.ops.len(), 1, "一个 op");
    assert_eq!(contract.ops[0].id, "run");
    assert_eq!(contract.ops[0].line, 0, "schema 无行号 ⇒ 恒 0");
    let declared = &schema["properties"]["run"]["x-function"];
    assert_eq!(
        contract.ops[0].signature["owner"], declared["owner"],
        "`owner` 逐键相等"
    );
    assert_eq!(
        contract.ops[0].signature["params"], declared["params"],
        "`params` 逐键相等"
    );
    assert_eq!(
        contract.ops[0].signature["returns"], declared["returns"],
        "`returns` 逐键相等（包裹态已解）"
    );
    assert_eq!(contract.fqn, FQN);
    assert_eq!(contract.file, FILE, "`file` 取入参");
}

/// 正向：`x-function.description` 在场 / 缺席两臂 ⇒ `signature` 均无 `description` 键，
/// 其余键（`owner` / `params` / `returns` / `async`）逐字保留。
#[test]
fn 描述不进签名() {
    for with_description in [true, false] {
        let mut function = serde_json::json!({
            "owner": "Runner",
            "params": [],
            "returns": {"type": "string"},
            "async": true
        });
        if with_description {
            function["description"] = serde_json::json!("调用方要知道的语义");
        }
        let schema = serde_json::json!({ "properties": { "run": { "x-function": function } } });
        let contract = Contract::from_schema(FQN, FILE, &schema);
        let signature = contract.ops[0].signature.as_object().expect("对象");
        assert!(
            !signature.contains_key("description"),
            "`description` 不进签名（在场臂 = {with_description}）：{signature:?}"
        );
        for key in ["owner", "params", "returns", "async"] {
            assert_eq!(
                signature.get(key),
                function.get(key),
                "`{key}` 逐字保留（在场臂 = {with_description}）"
            );
        }
    }
}

/// 正向：`$defs` 三形态——跨元素纯 `$ref` alias（带目标文件主干）⇒ `reexports` 值 ==
/// 目标 FQN；本地 alias `#/$defs/<名>` ⇒ `types`；带 `properties` 的定义 ⇒ `types`。
#[test]
fn 透传与类型按纯alias二分() {
    let target = "App.Demo.Input";
    let prefix = super::paths::ref_relative(FQN, target);
    let schema = serde_json::json!({
        "$defs": {
            "AgentInput": {"$ref": format!("{prefix}{}.schema.json#/$defs/AgentInput", super::paths::leaf_name(target))},
            "Local": {"$ref": "#/$defs/Widget"},
            "Widget": {"type": "object", "properties": {}}
        }
    });
    let contract = Contract::from_schema(FQN, FILE, &schema);
    assert_eq!(
        contract.reexports.get("AgentInput").and_then(Value::as_str),
        Some(target),
        "跨元素纯 alias ⇒ `reexports`（值 = 目标 FQN）"
    );
    assert!(
        !contract.types.contains_key("Local"),
        "本地纯 alias 不是透传也不是自有类型（`types` 只收非纯 alias）"
    );
    assert!(
        contract.types.contains_key("Widget"),
        "带 `properties` 的定义归 `types`"
    );
    assert_eq!(contract.reexports.len(), 1, "透传恰一条");
}

/// 边界：`$defs {}` + `properties {}` ⇒ 三表全空；`fqn` / `title` / `file` 取入参逐字。
#[test]
fn 空schema投影为零() {
    let schema = serde_json::json!({"$defs": {}, "properties": {}});
    let contract = Contract::from_schema(FQN, FILE, &schema);
    assert!(contract.ops.is_empty(), "零 op");
    assert!(contract.types.is_empty(), "零类型");
    assert!(contract.reexports.is_empty(), "零透传");
    assert_eq!(contract.fqn, FQN);
    assert_eq!(
        contract.title, FQN,
        "`title` 缺席时退化为入参 FQN（投影不做身份判）"
    );
    assert_eq!(contract.file, FILE);
}

/// 边界（投影零过滤）：`properties` 含非 op 键（无 `x-function` 的纯 `$ref` 镜像）⇒ 不进
/// `ops`；1000 键规模稳定；`$defs` 名含 Unicode ⇒ 原样保留（八族闸在 `verify`，不在投影）。
#[test]
fn 投影零过滤() {
    let mut properties = serde_json::Map::new();
    for index in 0..1000 {
        properties.insert(
            format!("op{index:04}"),
            serde_json::json!({"x-function": {"params": [], "returns": {"type": "string"}}}),
        );
    }
    properties.insert(
        "Mirror".to_owned(),
        serde_json::json!({"$ref": "#/$defs/Mirror"}),
    );
    let schema = serde_json::json!({
        "$defs": {"Mirror": {"type": "object"}, "带 空格": {"type": "object"}},
        "properties": properties
    });
    let contract = Contract::from_schema(FQN, FILE, &schema);
    assert_eq!(contract.ops.len(), 1000, "非 op 键不进 `ops`");
    assert!(
        contract.ops.iter().all(|op| op.id.starts_with("op")),
        "`ops` 只含带 `x-function` 的键"
    );
    assert!(
        contract.types.contains_key("带 空格"),
        "Unicode 名原样保留（名字闸在 `verify`，不在投影）"
    );
    assert_eq!(contract.types.len(), 2, "两类型逐项落位");
}

// ---------------------------------------------------------------------------
// 退役面与可见性（源码锚）
// ---------------------------------------------------------------------------

/// 异常（AC-1）：md 解析器整体退场——`parse` / `FENCE_OP` / `FENCE_TYPES` /
/// `collect_reexport_prose` / `collect_surface_table` / `ContractSection` /
/// `reexport_prose` 零命中；文件体量在设计要约内（≤ 640 行）。
#[test]
fn md解析器整体退场() {
    let source = include_str!("contract.rs");
    for retired in [
        "pub fn parse",
        "FENCE_OP",
        "FENCE_TYPES",
        "FENCE_REEXPORT",
        "collect_reexport_prose",
        "collect_surface_table",
        "struct ContractSection",
        "reexport_prose",
        "sections",
    ] {
        assert_eq!(
            source.matches(retired).count(),
            0,
            "`{retired}` 已随 md 解析器退役"
        );
    }
    assert!(
        source.lines().count() <= 640,
        "`contract.rs` 体量在设计要约内（≤ 640 行，实际 {}）",
        source.lines().count()
    );
    assert!(
        source.contains("pub fn from_schema("),
        "投影入口 `from_schema` 在案"
    );
}

/// 边界（死闸零残留）：`verify` 体内零 `reexport_prose` / `sections` / 散文表格闸；三闸
/// （七族名字 / 落点 / `required ⊆ properties`）各恰一处调用。
#[test]
fn 三闸保留其余退场() {
    let source = include_str!("contract.rs");
    let body = function_body(source, "pub fn verify(contract: &Contract, schema: &Value)");
    for retired in ["reexport_prose", "sections", "surface_table"] {
        assert!(
            !body.contains(retired),
            "`verify` 体内零 `{retired}`（死闸已退场）"
        );
    }
    assert!(
        body.contains("verify_x_function_positions("),
        "落点闸在 `verify` 同趟调用"
    );
    assert_eq!(
        body.matches("fn verify_x_function_positions(").count(),
        0,
        "落点闸是独立函数（体内不内联第二份）"
    );
    assert!(
        body.contains("identifier_error("),
        "七族名字闸在 `verify` 体内"
    );
}

/// 边界（可见性供写闸与 link 复用）：`duplicate_keys` / `is_identifier` 经
/// `super::contract::{…}` 直调成功（编译期即判）；`duplicate_keys` 对重复键文本返回首个
/// 重复键与行偏移、无重复 ⇒ `None`；`is_identifier` 六形态逐值。
#[test]
fn 可见性供写闸与link复用() {
    use super::contract::{duplicate_keys, is_identifier};

    let duplicated = "{\n  \"A\": 1,\n  \"A\": 2\n}";
    assert_eq!(
        duplicate_keys(duplicated),
        Some(("A".to_owned(), 2)),
        "首个重复键与其 0 起始行偏移"
    );
    assert_eq!(duplicate_keys("{\n  \"A\": 1\n}"), None, "无重复 ⇒ None");

    for valid in ["a", "_x", "A1", "run_2"] {
        assert!(is_identifier(valid), "合法标识符：{valid}");
    }
    for invalid in ["a-b", "1a", "", "🎯", "带 空格"] {
        assert!(!is_identifier(invalid), "非法标识符：{invalid:.10?}");
    }
    let long = "a".repeat(1200);
    assert!(
        is_identifier(&long),
        "超长纯 ASCII 仍是合法标识符（无长度上限）"
    );
}

/// 异常（七族名字闸唯一判据）：非法 op 名四形态（中划线 / 空格 / emoji / 空串）⇒
/// `verify` 各一条 `Err` 点名该 token 与族别；合法 ASCII 标识符（含超长）⇒ `Ok`。
#[test]
fn 七族名字闸唯一判据() {
    for name in ["带-连字符", "带 空格", "🎯", ""] {
        let mut properties = serde_json::Map::new();
        properties.insert(
            name.to_owned(),
            serde_json::json!({"x-function": {"params": [], "returns": {"type": "string"}}}),
        );
        let schema = serde_json::json!({ "properties": properties });
        let contract = Contract::from_schema(FQN, FILE, &schema);
        let message = verify_error(&contract, &schema);
        assert!(
            message.contains("op id") && message.contains("ASCII 标识符"),
            "非法 op 名 `{name:.10?}` ⇒ 点名族别与判据：{message}"
        );
    }

    let schema = serde_json::json!({
        "properties": {"run": {"x-function": {"params": [], "returns": {"type": "string"}}}}
    });
    let contract = Contract::from_schema(FQN, FILE, &schema);
    assert_eq!(verify(&contract, &schema), Ok(()), "合法 ASCII 标识符 ⇒ Ok");
}

// ---------------------------------------------------------------------------
// x-class 的构造入口与静态方法（AC-4 / AC-6）：名字族、预留名、落点封闭四处
// ---------------------------------------------------------------------------

/// 单条目 `$defs.<名>` 取给定 JSON 原值的（契约, schema）对（镜像 `$ref` 自动补齐）。
fn class_pair(name: &str, definition: Value) -> (Contract, Value) {
    let mut defs = serde_json::Map::new();
    defs.insert(name.to_owned(), definition);
    let mut properties = serde_json::Map::new();
    properties.insert(
        name.to_owned(),
        serde_json::json!({ "$ref": format!("#/$defs/{name}") }),
    );
    let schema = serde_json::json!({
        "$id": format!("{FQN}.schema.json"),
        "title": FQN,
        "$defs": Value::Object(defs),
        "properties": Value::Object(properties),
    });
    let contract = Contract::from_schema(FQN, FILE, &schema);
    (contract, schema)
}

/// 单条目契约经 `verify` 的结果。
fn class_verify(name: &str, definition: Value) -> Result<(), ApiError> {
    let (contract, schema) = class_pair(name, definition);
    verify(&contract, &schema)
}

/// 断言单条目契约报契约错误并返回文案。
fn class_error(name: &str, definition: Value) -> String {
    match class_verify(name, definition.clone()) {
        Ok(()) => panic!("期望契约错误，实际 Ok：{definition}"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 断言 `verify_defs` 报契约错误并返回文案（跨函数的落点分工用）。
fn defs_error(schema: &Value) -> String {
    match crate::api::schema::verify_defs(schema) {
        Ok(()) => panic!("期望 `verify_defs` 报错，实际 Ok"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 正向（AC-4 正例面）：四处合法落点——模块 op 顶层签名 + 类条目的 `properties` 方法 +
/// `x-constructor.x-function` + `x-static.<名>.x-function` ⇒ 全 `Ok`；`x-static` 成员
/// `returns` 指向本类型（工厂）⇒ `Ok`；三种 pointer 形态下零错误文案。
#[test]
fn verify类条目四处落点合法() {
    let definition = serde_json::json!({
        "type": "object",
        "x-class": true,
        "properties": {"run": {"x-function": {"params": []}}},
        "x-constructor": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}]}},
        "x-static": {
            "open": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}], "returns": {"$ref": "#/$defs/Ship"}}},
            "default_path": {"x-function": {"returns": {"type": "string"}}},
        },
    });
    assert_eq!(class_verify("Ship", definition), Ok(()), "四处落点全合法");

    let mut defs = serde_json::Map::new();
    defs.insert(
        "Ship".to_owned(),
        serde_json::json!({"type": "object", "x-class": true, "properties": {"run": {"x-function": {}}}}),
    );
    let mut properties = serde_json::Map::new();
    properties.insert(
        "run".to_owned(),
        serde_json::json!({"x-function": {"params": [{"name": "input", "schema": {"type": "string"}}], "returns": {"type": "boolean"}}}),
    );
    properties.insert(
        "Ship".to_owned(),
        serde_json::json!({"$ref": "#/$defs/Ship"}),
    );
    let schema =
        serde_json::json!({"$defs": Value::Object(defs), "properties": Value::Object(properties)});
    let contract = Contract::from_schema(FQN, FILE, &schema);
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "模块 op + 类条目同场合法"
    );
}

/// 异常（AC-4）：合法位之外的落点——`items` 内 / `additionalProperties` 内 / `oneOf`
/// 分支内 / `x-function` 自身子树 / 构造器 `returns` 内 ⇒ 各 `Err` 点名 JSON pointer 与
/// 定形族别串 `x-function 落点`。
#[test]
fn verify落点四处之外各报错() {
    let cases = [
        (
            "items 内",
            serde_json::json!({"type": "object", "properties": {"m": {"x-function": {"params": [{"name": "xs", "schema": {"type": "array", "items": {"x-function": {}}}}]}}}}),
        ),
        (
            "additionalProperties 内",
            serde_json::json!({"type": "object", "additionalProperties": {"x-function": {}}}),
        ),
        (
            "oneOf 分支内",
            serde_json::json!({"oneOf": [{"x-function": {}}]}),
        ),
        (
            "x-function 自身子树",
            serde_json::json!({"type": "object", "x-class": true, "x-static": {"open": {"x-function": {"params": [{"name": "p", "schema": {"x-function": {}}}]}}}}),
        ),
        (
            "构造器 x-function 的 returns 内",
            serde_json::json!({"type": "object", "x-class": true, "x-constructor": {"x-function": {"params": [], "returns": {"x-function": {}}}}}),
        ),
    ];
    for (label, definition) in cases {
        let message = class_error("Widget", definition);
        assert!(
            message.contains("x-function 落点"),
            "{label} ⇒ 定形族别串：{message}"
        );
        assert!(
            message.contains("JSON pointer"),
            "{label} ⇒ 点名 pointer：{message}"
        );
        assert!(
            !message.contains("ASCII 标识符"),
            "{label} ⇒ 落点错与名字错是两类错误面：{message}"
        );
    }
}

/// 边界（跨函数分工）：非类条目上的 `x-constructor` 经 `verify` ⇒ `Ok`（落点错不抢先、
/// `verify` 内零骨架复核）；同一 schema 经 `verify_defs` ⇒ `Err`——两函数分工可红绿。
#[test]
fn verify非类条目落点错不抢先() {
    let definition = serde_json::json!({"type": "object", "x-constructor": {"x-function": {}}});
    assert_eq!(
        class_verify("Plain", definition.clone()),
        Ok(()),
        "`verify` 内零骨架复核（落点归 `verify_defs`）"
    );
    let (_, schema) = class_pair("Plain", definition);
    let message = defs_error(&schema);
    assert!(
        message.contains("$defs.Plain.x-constructor"),
        "`verify_defs` 报落点：{message}"
    );
}

/// 异常（AC-6 / ③ 族并入）：`x-static` 名族——`open-it` / `""` / `🎯` / `带 空格` /
/// `带\n换行` ⇒ 各 `Err` 点名「方法名」族与该名字；1200 字符合法 ASCII ⇒ `Ok`（族别数
/// 仍七）。
#[test]
fn verify的x_static名字族() {
    for name in ["open-it", "", "🎯", "带 空格", "带\n换行"] {
        let mut statics = serde_json::Map::new();
        statics.insert(name.to_owned(), serde_json::json!({"x-function": {}}));
        let definition = serde_json::json!({
            "type": "object",
            "x-class": true,
            "x-static": Value::Object(statics),
        });
        let message = class_error("Ship", definition);
        assert!(message.contains("方法名"), "点名族别：{message}");
        assert!(message.contains("ASCII 标识符"), "点名判据：{message}");
    }

    let long = "a".repeat(1200);
    let mut statics = serde_json::Map::new();
    statics.insert(long.clone(), serde_json::json!({"x-function": {}}));
    let definition = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": Value::Object(statics),
    });
    assert_eq!(
        class_verify("Ship", definition),
        Ok(()),
        "超长合法 ASCII 名仍是标识符"
    );
}

/// 异常（AC-6 签名族）：`x-static` 成员的 `params[].name` 非法 ⇒ `Err`（③ 族的参数面）；
/// `owner` 非法 ⇒ `Err`（⑤ 族）；`description` 任意文本 ⇒ `Ok`（自由形状）。
#[test]
fn verify的x_static签名族() {
    let illegal_param = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": {"open": {"x-function": {"params": [{"name": "bad-name"}]}}},
    });
    let message = class_error("Ship", illegal_param);
    assert!(message.contains("参数名"), "点名参数族：{message}");
    assert!(message.contains("bad-name"), "点名非法名：{message}");

    let illegal_owner = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": {"open": {"x-function": {"owner": "not-an-owner"}}},
    });
    let message = class_error("Ship", illegal_owner);
    assert!(message.contains("owner 键"), "点名 owner 族：{message}");

    let free_form = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": {"open": {"x-function": {"description": "随便写的 描述 🎯", "params": [], "returns": {"type": "string"}}}},
    });
    assert_eq!(
        class_verify("Ship", free_form),
        Ok(()),
        "`description` 是自由文本"
    );
}

/// 正向（内部接线的收集面）：`x-class` 条目含 `properties.m` + `x-constructor` +
/// `x-static.{open, default_path}` ⇒ 四个名字全入 ③ 族——逐个把该面弄非法即各报一条，
/// 证明收集面确实覆盖三处。
#[test]
fn methods_of收集面覆盖四名字() {
    let full = serde_json::json!({
        "type": "object",
        "x-class": true,
        "properties": {"m": {"x-function": {}}},
        "x-constructor": {"x-function": {"params": []}},
        "x-static": {"open": {"x-function": {}}, "default_path": {"x-function": {}}},
    });
    assert_eq!(class_verify("Ship", full), Ok(()), "四名字全合法 ⇒ Ok");

    let illegal_method = serde_json::json!({
        "type": "object",
        "x-class": true,
        "properties": {"m-1": {"x-function": {}}},
    });
    assert!(
        class_error("Ship", illegal_method).contains("方法名"),
        "properties 方法名入族"
    );

    let illegal_new_param = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-constructor": {"x-function": {"params": [{"name": "bad-name"}]}},
    });
    assert!(
        class_error("Ship", illegal_new_param).contains("参数名"),
        "构造器入族（名字固定 new、参数面同判）"
    );

    let illegal_static = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": {"open-it": {"x-function": {}}},
    });
    assert!(
        class_error("Ship", illegal_static).contains("方法名"),
        "x-static 成员名入族"
    );

    let illegal_static_param = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": {"open": {"x-function": {"params": [{"name": "bad-name"}]}}},
    });
    assert!(
        class_error("Ship", illegal_static_param).contains("参数名"),
        "x-static 成员参数入族"
    );
}

/// 异常（超大与非类）：`x-static` 100 个成员 + 1 个非法名 ⇒ 恰报那一个；非类条目带
/// `x-static`（落点非法）⇒ `verify` 零收集、零名字错（不抢先）。
#[test]
fn methods_of超大与非类() {
    let mut statics = serde_json::Map::new();
    for index in 0..100 {
        statics.insert(format!("m{index}"), serde_json::json!({"x-function": {}}));
    }
    statics.insert("bad-name".to_owned(), serde_json::json!({"x-function": {}}));
    let definition = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": Value::Object(statics),
    });
    let message = class_error("Ship", definition);
    assert!(message.contains("bad-name"), "点名那一个非法名：{message}");
    assert!(!message.contains("m0"), "合法成员零误报：{message}");

    let non_class = serde_json::json!({
        "type": "object",
        "x-static": {"open": {"x-function": {}}},
    });
    assert_eq!(
        class_verify("Plain", non_class),
        Ok(()),
        "非类条目的新键整条跳过（落点归 `verify_defs`）"
    );
}

/// 边界（收集面三臂）：空面（无 ctor / 无 `x-static` / `x-static: {}` / `x-class: false`）
/// ⇒ 收集面零新增、`Ok`；1200 字符合法名仍入 ③ 族（其参数面同判即证明该成员被收集到）；
/// `properties` 键与 `x-static` 名同名 ⇒ 两路收集都可见（N-2 文案同时点名两张表）。
#[test]
fn methods_of收集面边界() {
    for definition in [
        serde_json::json!({"type": "object", "x-class": true, "properties": {"run": {"x-function": {}}}}),
        serde_json::json!({"type": "object", "x-class": true, "properties": {"run": {"x-function": {}}}, "x-static": {}}),
        serde_json::json!({
            "type": "object",
            "x-class": false,
            "properties": {"run": {"x-function": {}}},
            "x-static": {"open": {"x-function": {}}},
        }),
    ] {
        assert_eq!(
            class_verify("Ship", definition.clone()),
            Ok(()),
            "空面零新增、非类条目零收集：{definition}"
        );
    }

    let long = "a".repeat(1200);
    let mut statics = serde_json::Map::new();
    statics.insert(
        long.clone(),
        serde_json::json!({"x-function": {"params": [{"name": "bad-name"}]}}),
    );
    let definition = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": Value::Object(statics),
    });
    let message = class_error("Ship", definition);
    assert!(
        message.contains("参数名"),
        "1200 字符合法名仍入 ③ 族（参数面同判）：{message}"
    );
    assert!(message.contains("bad-name"), "点名非法参数：{message}");

    let same_name = serde_json::json!({
        "type": "object",
        "x-class": true,
        "properties": {"open": {"x-function": {}}},
        "x-static": {"open": {"x-function": {}}},
    });
    let message = class_error("Ship", same_name);
    assert!(
        message.contains("$defs.Ship.x-static.open"),
        "两路收集都可见（static 侧 pointer）：{message}"
    );
    assert!(
        message.contains("properties.open"),
        "两路收集都可见（properties 侧键位）：{message}"
    );
}

/// 边界（空面）：`$defs: {}` / 条目无 ctor / 无 `x-static` / `x-static: {}` ⇒ `Ok`；
/// `x-class: "true"`（字符串）条目 ⇒ 不触发三条预留名判据；`properties` / `$defs` 缺省
/// ⇒ `Ok`（投影为零）。
#[test]
fn verify空面与投影为零() {
    for definition in [
        serde_json::json!({"type": "object", "x-class": true, "properties": {}}),
        serde_json::json!({"type": "object", "x-class": true, "x-static": {}}),
        serde_json::json!({"type": "object", "x-class": true}),
    ] {
        assert_eq!(class_verify("Ship", definition), Ok(()), "缺省面应过");
    }

    let string_flag = serde_json::json!({
        "type": "object",
        "x-class": "true",
        "properties": {"new": {"x-function": {}}},
    });
    assert_eq!(
        class_verify("Ship", string_flag),
        Ok(()),
        "字符串 x-class 不算类条目 ⇒ 不触发预留名判据"
    );

    let schema = serde_json::json!({});
    let contract = Contract::from_schema(FQN, FILE, &schema);
    assert_eq!(verify(&contract, &schema), Ok(()), "零投影 ⇒ Ok");
}

/// 异常（AC-6 预留名 N-3 / N-1 / N-2）：`x-static` 名 = `new` ⇒ `Err` 点名保留名与出路；
/// 与 `x-constructor` 并存时仍报该错；`properties` 键 = `new` ⇒ `Err`；两表重名 ⇒ `Err`
/// 点名重名方法与「改名或合并」出路。
#[test]
fn verify类条目三条预留名() {
    let static_new = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-static": {"new": {"x-function": {}}},
    });
    let message = class_error("Ship", static_new.clone());
    assert!(message.contains("保留名"), "点名保留名：{message}");
    assert!(
        message.contains("x-constructor"),
        "出路指向构造入口：{message}"
    );

    let with_constructor = serde_json::json!({
        "type": "object",
        "x-class": true,
        "x-constructor": {"x-function": {}},
        "x-static": {"new": {"x-function": {}}},
    });
    assert!(
        class_error("Ship", with_constructor).contains("保留名"),
        "构造器在场不豁免 x-static 侧的保留名判据"
    );

    let field_new = serde_json::json!({
        "type": "object",
        "x-class": true,
        "properties": {"new": {"x-function": {}}},
    });
    let message = class_error("Ship", field_new);
    assert!(
        message.contains("properties.new"),
        "点名 properties 侧键位：{message}"
    );
    assert!(message.contains("保留名"), "点名保留名：{message}");

    let shared = serde_json::json!({
        "type": "object",
        "x-class": true,
        "properties": {"open": {"x-function": {}}},
        "x-static": {"open": {"x-function": {}}},
    });
    let message = class_error("Ship", shared);
    assert!(message.contains("重名"), "点名重名：{message}");
    assert!(
        message.contains("改名或把两者合并"),
        "含可执行出路：{message}"
    );
}

/// 边界（射程边界）：非 `x-class` 条目的 `properties.new`（函数指针字段）⇒ `verify` `Ok`
/// ——条目面判据与模块面判据分立；另一条目的 `x-static.open` 与本条目 `properties.open`
/// ⇒ `Ok`（文件级自洽、零跨元素）。
#[test]
fn verify预留名射程边界() {
    let plain = serde_json::json!({
        "type": "object",
        "properties": {"new": {"x-function": {}}},
    });
    assert_eq!(
        class_verify("Plain", plain),
        Ok(()),
        "非类条目的 properties.new 不触发预留名判据"
    );

    let mut defs = serde_json::Map::new();
    defs.insert(
        "Ship".to_owned(),
        serde_json::json!({"type": "object", "x-class": true, "properties": {"open": {"x-function": {}}}}),
    );
    defs.insert(
        "Other".to_owned(),
        serde_json::json!({"type": "object", "x-class": true, "x-static": {"open": {"x-function": {}}}}),
    );
    let mut properties = serde_json::Map::new();
    for name in ["Ship", "Other"] {
        properties.insert(
            name.to_owned(),
            serde_json::json!({"$ref": format!("#/$defs/{name}")}),
        );
    }
    let schema =
        serde_json::json!({"$defs": Value::Object(defs), "properties": Value::Object(properties)});
    let contract = Contract::from_schema(FQN, FILE, &schema);
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "跨条目的同名不算重名（文件级自洽、零跨元素）"
    );

    let false_flag = serde_json::json!({
        "type": "object",
        "x-class": false,
        "properties": {"new": {"x-function": {}}},
        "x-static": {"open": {"x-function": {}}},
    });
    assert_eq!(
        class_verify("Plain", false_flag.clone()),
        Ok(()),
        "非类条目不触发三条预留名判据"
    );
    let (_, schema) = class_pair("Plain", false_flag);
    assert!(
        defs_error(&schema).contains("$defs.Plain.x-static"),
        "非类条目的新键由落点闸报"
    );
}

/// 正向（D-1 单点持有）：`X_CONSTRUCTOR` / `X_STATIC` 两常量逐字取值；`contract.rs` 内各
/// 恰一处定义；`schema.rs` / 两渲染器内零**带引号字面量**（经常量名引用）。
#[test]
fn x_constructor与x_static常量单点() {
    assert_eq!(X_CONSTRUCTOR, "x-constructor", "构造入口关键字逐字");
    assert_eq!(X_STATIC, "x-static", "静态方法表关键字逐字");

    let contract = include_str!("contract.rs");
    assert_eq!(
        contract.matches("pub(crate) const X_CONSTRUCTOR").count(),
        1,
        "`X_CONSTRUCTOR` 单点定义"
    );
    assert_eq!(
        contract.matches("pub(crate) const X_STATIC").count(),
        1,
        "`X_STATIC` 单点定义"
    );

    for (label, source) in [
        ("schema.rs", include_str!("schema.rs")),
        ("emit/rust.rs", include_str!("emit/rust.rs")),
        ("emit/ts.rs", include_str!("emit/ts.rs")),
    ] {
        for literal in ["\"x-constructor\"", "\"x-static\""] {
            assert!(
                !source.contains(literal),
                "{label} 内零散落字面量 {literal}（经常量名引用）"
            );
        }
    }
}

/// 异常（近形键）：`$defs` 条目顶层写 `x-construct` / `x-Statics` / `x_constructor` ⇒ 不命中
/// 新键判据、不新增错误面（当作普通未知键由既有闸处置）。
#[test]
fn 近形键不误判() {
    for key in ["x-construct", "x-Statics", "x_constructor"] {
        let mut definition = serde_json::Map::new();
        definition.insert("type".to_owned(), serde_json::json!("object"));
        definition.insert(key.to_owned(), serde_json::json!({"params": []}));
        let definition = Value::Object(definition);
        assert_eq!(
            class_verify("Plain", definition.clone()),
            Ok(()),
            "近形键 `{key}` 不被当新键"
        );
        let (_, schema) = class_pair("Plain", definition);
        assert_eq!(
            crate::api::schema::verify_defs(&schema),
            Ok(()),
            "近形键 `{key}` 不新增落点错误面"
        );
    }
}

/// 异常（AC-4 文案面）：定形族别串 `x-function 落点` + `JSON pointer` + 「封闭四处」+
/// 四条 pointer 形态 + 出路「去掉标记层，或把该形状提升为具名 `$defs` 条目」。
#[test]
fn x_function_position_error文案面() {
    let definition = serde_json::json!({
        "type": "object",
        "properties": {"m": {"x-function": {"params": [{"name": "xs", "schema": {"type": "array", "items": {"x-function": {}}}}]}}},
    });
    let message = class_error("Widget", definition);
    assert!(message.contains("x-function 落点"), "定形族别串：{message}");
    assert!(message.contains("JSON pointer"), "点名 pointer：{message}");
    for pointer_form in [
        "properties.<k>",
        "$defs.<T>.properties.<k>",
        "x-constructor",
        "x-static.<名>",
    ] {
        assert!(
            message.contains(pointer_form),
            "四条 pointer 形态在内（缺 `{pointer_form}`）：{message}"
        );
    }
    assert!(
        message.contains("去掉标记层，或把该形状提升为具名 `$defs` 条目"),
        "含可执行出路：{message}"
    );
}

/// 边界（四种 pointer 形态可读）：`properties.<k>` / `$defs.<T>.properties.<k>` /
/// `$defs.<T>.x-constructor` / `$defs.<T>.x-static.<名>` 各产出可读 pointer（数组下标
/// `[0]` 形态亦在案）；全部经 `verify` 的错误文案观测（私有函数零直调、零新增导出）。
#[test]
fn x_function_position_error四种pointer形态() {
    let cases = [
        (
            "properties.<k>",
            "properties.run.params[0].schema",
            serde_json::json!({"properties": {"run": {"x-function": {"params": [{"name": "p", "schema": {"x-function": {}}}]}}}}),
            false,
        ),
        (
            "$defs.<T>.properties.<k>",
            "$defs.Widget.properties.m.x-function.params[0].schema",
            serde_json::json!({"type": "object", "properties": {"m": {"x-function": {"params": [{"name": "p", "schema": {"x-function": {}}}]}}}}),
            true,
        ),
        (
            "$defs.<T>.x-constructor",
            "$defs.Widget.x-constructor.x-function.params[0].schema",
            serde_json::json!({"type": "object", "x-class": true, "x-constructor": {"x-function": {"params": [{"name": "p", "schema": {"x-function": {}}}]}}}),
            true,
        ),
        (
            "$defs.<T>.x-static.<名>",
            "$defs.Widget.x-static.open.x-function.params[0].schema",
            serde_json::json!({"type": "object", "x-class": true, "x-static": {"open": {"x-function": {"params": [{"name": "p", "schema": {"x-function": {}}}]}}}}),
            true,
        ),
    ];
    for (label, pointer, definition, single) in cases {
        let message = if single {
            class_error("Widget", definition)
        } else {
            let contract = Contract::from_schema(FQN, FILE, &definition);
            verify_error(&contract, &definition)
        };
        assert!(
            message.contains(pointer),
            "{label} ⇒ pointer 可读（期望 `{pointer}`）：{message}"
        );
        assert!(
            message.contains("[0]"),
            "{label} ⇒ 数组下标形态在案：{message}"
        );
    }
}
