//! `api::contract` 的单元测试：`verify` 三闸（七族名字 / `x-function` 落点封闭二处 /
//! `required ⊆ properties`）与构造器封闭集判据（`is_constructor`）。
//!
//! 零 mock：`verify` 吃「投影 + schema 值」两个纯值入参（零落盘、零读盘）；
//! `Contract` 由 `Contract::from_schema` 投影，schema 由手写 `json!` 给（`required` /
//! `$defs` / `x-function` 的反例直接注入）。

use serde_json::Value;

use super::contract::{is_constructor, verify, Contract};
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
// 落点闸：`x-function` 的合法落点封闭二处（D47 / AC-25）
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
/// 红**——那意味着改成递归遍历任意深度，与 D47「把射程降成可穷举的二处」相反。
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
    let source = mirrored_schema_text(FQN, &format!("\"Widget\": {definition}"), "");
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
            "AgentInput": {"$ref": format!("{prefix}{target}.schema.json#/$defs/AgentInput")},
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
/// `reexport_prose` 零命中；文件体量在设计要约内（≤ 500 行）。
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
        source.lines().count() <= 500,
        "`contract.rs` 体量在设计要约内（≤ 500 行，实际 {}）",
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
