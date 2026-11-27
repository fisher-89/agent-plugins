//! `api::schema` 的单元测试：`validate_schema` 的九道闸（identity / 键空间 / 形态 /
//! 镜像 / `$ref` 前缀 / 顶层键封闭 / 复用三闸 + `$defs` 规则）、`verify_defs` 的五条
//! 规则 + 透传纯 `$ref` 校验 + `x-class` 带属性判错、`build_index` 的说明段 / 七字段 /
//! 字典序 / `[entry.categories]` 形状分布与整段省略。
//!
//! 零 mock：接口定义文本手写（`json!` / 原始 `&str`），真实 `serde_json` 解析与真实
//! `paths::ref_relative`；`toml` 反解析真实执行（INDEX 可解析性即被测语义）；`ApiMeta`
//! 以字面量构造真实值（零替身类型）。写闸与装载期共用同一入口，故本文件的判据面即
//! AC-4 / AC-5 的窄面。

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};
use toml_edit::DocumentMut;

use crate::api::contract::Contract;

use super::paths;
use super::schema::{
    build_index, degraded_marks, validate_schema, verify_defs, verify_identity, verify_keyspace,
    verify_mirror, verify_ref_prefix, verify_shape, verify_top_level_keys, DEGRADED_KINDS,
};
use super::{ApiError, ApiFormStatus, ApiMeta, IndexEntry};

/// `$id` 的期望值（= `<FQN>.schema.json`；文件名只取叶段，`$id` 仍写全 FQN）。
const FILE: &str = "App.Backend.Infra.ArchiRuntime.schema.json";

/// 元素 FQN（= `title`；落点由分层目录 + 叶段文件名反查）。
const FQN: &str = "App.Backend.Infra.ArchiRuntime";

/// 装配用载元（`tool_version` 取编译期常量、摘要取真实值）。
fn meta() -> ApiMeta {
    super::meta()
}

/// 断言 `Err(ApiError::Contract(..))` 并返回文案。
fn contract_error(result: Result<(), ApiError>) -> String {
    match result {
        Ok(()) => panic!("期望契约错误，实际 Ok"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 断言接口定义校验报契约错误并返回文案。
fn parse_error(result: Result<Contract, ApiError>) -> String {
    match result {
        Ok(_) => panic!("期望契约错误，实际 Ok"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 接口定义文本构造器：`$defs` / `properties` 两表片段直传，其余四顶层键按 FQN 补齐。
///
/// 九闸用例只替换一表，其余闸保持绿——反例因此**单点归因**（例外：闸序用例刻意同时违反
/// 多闸，见 `闸序固定`）。
fn schema_text(defs: &str, properties: &str) -> String {
    format!(
        "{{\n  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n  \"$id\": \
         \"{FQN}.schema.json\",\n  \"title\": \"{FQN}\",\n  \"$comment\": \"手写接口真相\",\n  \
         \"$defs\": {{{defs}}},\n  \"properties\": {{{properties}}}\n}}\n"
    )
}

/// 单条 INDEX 条目（`Declared` 形态的缺省值）。
fn entry(fqn: &str) -> IndexEntry {
    IndexEntry {
        fqn: fqn.to_owned(),
        status: ApiFormStatus::Declared,
        types: 0,
        ops: 0,
        degraded: 0,
        categories: BTreeMap::new(),
        schema_rel: String::new(),
        generated_rel: None,
    }
}

/// INDEX 文本 → TOML 文档（解析失败即断言失败：可解析性是被测语义）。
fn index_document(text: &str) -> DocumentMut {
    text.parse::<DocumentMut>()
        .unwrap_or_else(|error| panic!("INDEX.toml 应可解析：{error}\n{text}"))
}

/// 说明段行数（以 `#` 开头的行）。
fn comment_lines(text: &str) -> Vec<String> {
    text.lines()
        .take_while(|line| line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

// ---------------------------------------------------------------------------
// validate_schema：九闸
// ---------------------------------------------------------------------------

/// 正向（闸 ③–⑨ 全过）：空壳——`$defs {}` + `properties {}` 两空表、四顶层键齐备 ⇒ `Ok`，
/// `Contract` 三表全空、`file` == 接口文件的 repo 相对路径（分层目录 + 叶段主干）。
#[test]
fn 空壳合法() {
    let contract = validate_schema(FQN, &schema_text("", "")).expect("空壳应合法");
    assert_eq!(contract.fqn, FQN, "FQN 取入参");
    assert_eq!(contract.title, FQN, "`title` 逐字");
    assert_eq!(
        contract.file,
        paths::schema_rel(FQN).expect("测试 FQN 恒为合法形态"),
        "`file` = 接口文件的 repo 相对路径（分层目录 + 叶段文件名）"
    );
    assert!(contract.ops.is_empty() && contract.types.is_empty() && contract.reexports.is_empty());
}

/// 正向：1 op + 1 类型（镜像 `$ref`）+ 1 跨元素透传 alias ⇒ `Ok`；签名解包（`description`
/// 不进投影）、`types` / `reexports` 各 1（纯 alias 二分单点）。
#[test]
fn 合法面全过() {
    let target = "App.Backend.Core.Agent";
    let prefix = paths::ref_relative(FQN, target);
    let text = schema_text(
        &format!(
            "\"Local\": {{ \"type\": \"object\", \"properties\": {{}} }}, \
             \"AgentInput\": {{ \"$ref\": \"{prefix}{}.schema.json#/$defs/AgentInput\" }}",
            paths::leaf_name(target)
        ),
        concat!(
            r##""run": { "x-function": { "description": "跑", "params": [], "returns": "##,
            r##"{ "type": "string" } } }, "Local": { "$ref": "#/$defs/Local" }, "##,
            r##""AgentInput": { "$ref": "#/$defs/AgentInput" }"##,
        ),
    );
    let contract = validate_schema(FQN, &text).expect("合法面应全过");
    assert_eq!(contract.ops.len(), 1, "一个 op");
    assert_eq!(contract.ops[0].id, "run");
    assert_eq!(contract.ops[0].line, 0, "schema 无行号");
    assert!(
        contract.ops[0].signature.get("description").is_none(),
        "`description` 不进投影：{:?}",
        contract.ops[0].signature
    );
    assert!(contract.types.contains_key("Local"), "本地类型");
    assert_eq!(
        contract.reexports.get("AgentInput").and_then(Value::as_str),
        Some(target),
        "跨元素纯 alias 落 `reexports`（值 = 目标 FQN）"
    );
}

/// 异常（九闸逐闸反例）：① 文本层重复键 ② 非法 JSON ③ `$id` / `title` / 文件主干三条不符
/// ④ op 名与类型名撞名 ⑤ `properties.<k>` 兄弟键 / `$ref` 指向 `$defs` 外 ⑥ `$defs` 键无镜像
/// ⑦ 跨元素 `$ref` 前缀差一字符 ⑧ 未知顶层键 ⑨ `x-function.params[].required` 越界 ⇒ 各一条
/// `Err`，文案点名文件与期望值。
#[test]
fn 九闸各自独立可判() {
    let target = "App.Backend.Core.Agent";
    let prefix = paths::ref_relative(FQN, target);
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "① 重复键",
            format!(
                "{{\n  \"$id\": \"{FQN}.schema.json\",\n  \"title\": \"{FQN}\",\n  \
                 \"$defs\": {{\"A\": {{}}, \"A\": {{}} }},\n  \"properties\": {{}}\n}}\n"
            ),
            "重复声明",
        ),
        ("② 非法 JSON", "{\n  \"$id\": ".to_owned(), "不是合法 JSON"),
        (
            "③ `$id` 不符",
            format!(
                "{{\n  \"$id\": \"App.Other.schema.json\",\n  \"title\": \"{FQN}\",\n  \
                 \"$defs\": {{}},\n  \"properties\": {{}}\n}}\n"
            ),
            "App.Other.schema.json",
        ),
        (
            "③ `title` 不符",
            format!(
                "{{\n  \"$id\": \"{FQN}.schema.json\",\n  \"title\": \"App.Other\",\n  \
                 \"$defs\": {{}},\n  \"properties\": {{}}\n}}\n"
            ),
            "App.Other",
        ),
        (
            "④ op 与类型撞名",
            schema_text(
                "\"run\": { \"type\": \"object\", \"properties\": {} }",
                "\"run\": { \"x-function\": { \"params\": [], \"returns\": { \"type\": \"string\" } } }",
            ),
            "两个键空间必须互斥",
        ),
        (
            "⑤ `x-function` 与兄弟键并列",
            schema_text(
                "",
                "\"run\": { \"x-function\": { \"params\": [] }, \"description\": \"x\" }",
            ),
            "只允许恰一个键",
        ),
        (
            "⑤ `$ref` 指向 `$defs` 外",
            schema_text("", "\"Item\": { \"$ref\": \"#/$defs/Ghost\" }"),
            "本文件 `$defs` 里没有该条目",
        ),
        (
            "⑥ `$defs` 键无镜像",
            schema_text("\"Local\": { \"type\": \"object\" }", ""),
            "没有指向自己的镜像条目",
        ),
        (
            "⑦ 跨元素前缀非规范形",
            schema_text(
                &format!(
                    "\"AgentInput\": {{ \"$ref\": \"{prefix}./{}.schema.json#/$defs/AgentInput\" }}",
                    paths::leaf_name(target)
                ),
                "\"AgentInput\": { \"$ref\": \"#/$defs/AgentInput\" }",
            ),
            "跨元素 `$ref` 的前缀非法",
        ),
        (
            "⑧ 未知顶层键",
            format!(
                "{{\n  \"$id\": \"{FQN}.schema.json\",\n  \"title\": \"{FQN}\",\n  \
                 \"$defs\": {{}},\n  \"properties\": {{}},\n  \"type\": \"object\"\n}}\n"
            ),
            "顶层出现未知键 `type`",
        ),
        (
            "⑨ `required` 越界",
            schema_text(
                "\"Local\": { \"type\": \"object\", \"properties\": {}, \"required\": [\"ghost\"] }",
                "\"Local\": { \"$ref\": \"#/$defs/Local\" }",
            ),
            "required 元素在同条目 `properties` 里没有对应键",
        ),
    ];

    for (label, text, expected) in cases {
        let error = parse_error(validate_schema(FQN, &text));
        assert!(
            error.contains(&paths::schema_rel(FQN).expect("测试 FQN 恒为合法形态")),
            "{label} 的文案点名接口文件：{error}"
        );
        assert!(
            error.contains(expected),
            "{label} 的文案含期望串 `{expected}`：{error}"
        );
    }
}

/// 边界（闸序固定）：①–② 的坏文本同时违反 ③（重复键 + `$id` 不符）⇒ 文案落在 ①（先
/// 发生）；③–⑧ 同时违反多闸 ⇒ 文案按固定闸序取最前（identity 先于顶层键）。
#[test]
fn 闸序固定() {
    let duplicated = concat!(
        r#"{"#,
        r#""$id": "App.Other.schema.json","#,
        r#""title": "App.Other","#,
        r#""$defs": {"A": {}, "A": {}},"#,
        r#""properties": {}"#,
        "}\n",
    );
    let error = parse_error(validate_schema(FQN, duplicated));
    assert!(error.contains("重复声明"), "① 先于 ③：{error}");
    assert!(
        !error.contains("`$id` 是"),
        "identity 文案不出现（被 ① 短路）：{error}"
    );

    // ③ 与 ⑧ 同时违反：identity 先发生
    let both = concat!(
        r#"{"#,
        r#""$id": "App.Other.schema.json","#,
        r#""title": "App.Other","#,
        r#""$defs": {},"#,
        r#""properties": {},"#,
        r#""type": "object""#,
        "}\n",
    );
    let error = parse_error(validate_schema(FQN, both));
    assert!(error.contains("`$id` 是"), "③ 先于 ⑧：{error}");
    assert!(!error.contains("未知键"), "顶层键闸被短路：{error}");
}

/// 边界（文本边界不 panic）：空串 / 纯空白 / 1 MB 超长 / emoji / 含 `\0` / 带 BOM ⇒ 全
/// `Err` 且不 panic（`\0` 与 BOM 由 ② 兜）。
#[test]
fn 文本边界不panic() {
    let long = "a".repeat(1024 * 1024);
    let cases = [
        "",
        "   \n\t  ",
        long.as_str(),
        "🎯🎯🎯",
        "{\u{0}}",
        "\u{feff}{}",
    ];
    for text in cases {
        let error = parse_error(validate_schema(FQN, text));
        assert!(!error.is_empty(), "错误文案非空：{text:.20?}");
    }
}

/// 异常（闸 ③ 三合一）：`$id` 缺 / 非字符串 / 不等于 `<FQN>.schema.json`；`title` ≠ FQN；
/// 落点（分层目录 + 叶段）反查 ⇒ 各一条 `Err` 并列期望值。
#[test]
fn identity三合一() {
    let cases: [(&str, Value, &str); 5] = [
        ("`$id` 缺", json!({"title": FQN}), "缺 `$id`"),
        (
            "`$id` 非字符串",
            json!({"$id": 1, "title": FQN}),
            "缺 `$id`",
        ),
        (
            "`$id` 不符",
            json!({"$id": "App.Other.schema.json", "title": FQN}),
            "期望 `App.Backend.Infra.ArchiRuntime.schema.json`",
        ),
        ("`title` 缺", json!({"$id": FILE}), "缺 `title`"),
        (
            "`title` 不符",
            json!({"$id": FILE, "title": "App.Other"}),
            "期望 `App.Backend.Infra.ArchiRuntime`",
        ),
    ];
    for (label, value, expected) in cases {
        let error = contract_error(verify_identity(FQN, &value));
        assert!(
            error.contains(expected),
            "{label} 的文案并列期望值 `{expected}`：{error}"
        );
    }
    // 落点的反查单点：分层目录段 + 叶段主干 → FQN，形态不符 ⇒ None
    assert_eq!(
        paths::schema_fqn(Path::new("App/Backend/Infra/ArchiRuntime.schema.json")),
        Some(FQN.to_owned()),
        "落点路径反查"
    );
}

/// 正向（闸 ③ 正例与逐字判据）：三者一致（四段 / 单段两形态）⇒ `Ok`；大小写差异 / 前后
/// 空白 / 全角字符 ⇒ `Err`（逐字相等，零归一）。
#[test]
fn identity正例() {
    for fqn in [FQN, "App"] {
        let value = json!({ "$id": format!("{fqn}.schema.json"), "title": fqn });
        assert!(verify_identity(fqn, &value).is_ok(), "三者一致 ⇒ Ok：{fqn}");
    }
    for (label, id, title) in [
        (
            "大小写差异",
            "app.backend.infra.archiruntime.schema.json",
            FQN,
        ),
        (
            "前后空白",
            " App.Backend.Infra.ArchiRuntime.schema.json",
            FQN,
        ),
        (
            "全角字符",
            "Ａpp.Backend.Infra.ArchiRuntime.schema.json",
            FQN,
        ),
    ] {
        let value = json!({ "$id": id, "title": title });
        let error = contract_error(verify_identity(FQN, &value));
        assert!(!error.is_empty(), "{label} 逐字判据 ⇒ Err：{error}");
    }
}

/// 异常（闸 ④）：同名既是 op（带 `x-function`）又是 `$defs` 键 ⇒ `Err` 点名两键空间；
/// 正例：两键集不相交、`$defs` 空 / `properties` 空、仅 `properties` 有非 op 的 `$ref`
/// 镜像键 ⇒ 全 `Ok`（判的是「同名两用」，不是「properties 与 `$defs` 无重名」）。
#[test]
fn 同名不成两用() {
    let collision = json!({
        "$defs": {"run": {"type": "object"}},
        "properties": {"run": {"x-function": {"params": []}}},
    });
    let error = contract_error(verify_keyspace(&collision));
    assert!(
        error.contains("run") && error.contains("互斥"),
        "点名撞名键与两键空间：{error}"
    );

    for (label, value) in [
        (
            "不相交",
            json!({"$defs": {"T": {"type": "object"}}, "properties": {"run": {"x-function": {}}}}),
        ),
        (
            "镜像键同名（合法）",
            json!({"$defs": {"T": {"type": "object"}}, "properties": {"T": {"$ref": "#/$defs/T"}}}),
        ),
        ("两表皆空", json!({"$defs": {}, "properties": {}})),
    ] {
        assert!(verify_keyspace(&value).is_ok(), "{label} ⇒ Ok");
    }
}

/// 异常（闸 ⑤ 白名单）：`{x-function: 非对象}` / `{x-function, $ref}` 双键 / `$ref` 指向
/// `$defs` 外 / `properties.<k>` 空对象 / 为数组 ⇒ 各一条 `Err`；正例：`x-function` 内层
/// 自由（含将来未知键）、`$ref` 指向在册 `$defs`、`properties {}` 空 ⇒ `Ok`。
#[test]
fn 形态恰一键() {
    let cases: [(&str, Value, &str); 5] = [
        (
            "x-function 非对象",
            json!({"properties": {"run": {"x-function": "not-object"}}}),
            "必须是对象",
        ),
        (
            "双键并列",
            json!({"properties": {"run": {"x-function": {}, "$ref": "#/$defs/T"}}}),
            "只允许恰一个键",
        ),
        (
            "$ref 越界",
            json!({"$defs": {}, "properties": {"T": {"$ref": "#/$defs/Ghost"}}}),
            "没有该条目",
        ),
        ("空对象", json!({"properties": {"T": {}}}), "形态非法"),
        ("数组形态", json!({"properties": {"T": []}}), "形态非法"),
    ];
    for (label, value, expected) in cases {
        let error = contract_error(verify_shape(&value));
        assert!(error.contains(expected), "{label} ⇒ {expected}：{error}");
    }

    assert!(
        verify_shape(&json!({"properties": {"run": {"x-function": {"deprecated": true}}}})).is_ok(),
        "x-function 内层自由"
    );
    assert!(
        verify_shape(&json!({"$defs": {"T": {}}, "properties": {"T": {"$ref": "#/$defs/T"}}}))
            .is_ok(),
        "指向在册 `$defs` ⇒ Ok"
    );
    assert!(
        verify_shape(&json!({"properties": {}})).is_ok(),
        "空表 ⇒ Ok"
    );
}

/// 异常（闸 ⑥）：`$defs.<名>` 无 `properties.<名>` 的自我 `$ref` / `$ref` 指向别的名字 ⇒
/// 各一条 `Err`；正例：全镜像 ⇒ `Ok`，`$defs {}` ⇒ `Ok`（无类型即无镜像要求）。
#[test]
fn defs每键须自我镜像() {
    let missing = json!({"$defs": {"T": {"type": "object"}}, "properties": {}});
    let error = contract_error(verify_mirror(&missing));
    assert!(
        error.contains("T") && error.contains("镜像"),
        "点名缺镜像的 `$defs` 键：{error}"
    );

    let wrong = json!({
        "$defs": {"T": {"type": "object"}, "U": {"type": "object"}},
        "properties": {"T": {"$ref": "#/$defs/U"}, "U": {"$ref": "#/$defs/U"}},
    });
    assert!(
        contract_error(verify_mirror(&wrong)).contains("T"),
        "指向别的名字 ⇒ Err"
    );

    assert!(
        verify_mirror(
            &json!({"$defs": {"T": {"type": "object"}}, "properties": {"T": {"$ref": "#/$defs/T"}}})
        )
        .is_ok(),
        "全镜像 ⇒ Ok"
    );
    assert!(
        verify_mirror(&json!({"$defs": {}})).is_ok(),
        "无类型无镜像要求"
    );
}

/// 异常（闸 ⑦）：同 crate 兄弟 / 跨 crate / 越根三形态——前缀 == `ref_relative` 输出 ⇒ `Ok`；
/// 差一字符 / 少一层 `../` / 多一段目录 ⇒ `Err` 且文案并列当前串与期望串。
#[test]
fn 前缀由ref_relative单点决定() {
    let siblings = [
        "App.Backend.Infra.AgentRuntime",
        "App.Backend.Core.Agent",
        "App.Frontend.Views.Welcome",
    ];
    for target in siblings {
        let prefix = paths::ref_relative(FQN, target);
        let leaf = paths::leaf_name(target);
        let ok = json!({"$defs": {"X": {"$ref": format!("{prefix}{leaf}.schema.json#/$defs/X")}}});
        assert!(
            verify_ref_prefix(FQN, &ok).is_ok(),
            "前缀 == ref_relative 输出 ⇒ Ok：{target} / {prefix}"
        );

        for bad in [
            format!("././{leaf}.schema.json#/$defs/X"),
            format!("{prefix}{leaf}/../{leaf}.schema.json#/$defs/X"),
            format!("{prefix}extra/../{leaf}.schema.json#/$defs/X"),
        ] {
            let value = json!({"$defs": {"X": {"$ref": bad}}});
            let error = contract_error(verify_ref_prefix(FQN, &value));
            assert!(
                error.contains("当前串") && error.contains("期望"),
                "文案并列当前串与期望串：{error}"
            );
        }

        // 越界前缀（`..` 多于层数）解不出目标 ⇒ 同样是错
        let escaped =
            json!({"$defs": {"X": {"$ref": format!("../../../../{leaf}.schema.json#/$defs/X")}}});
        let error = contract_error(verify_ref_prefix(FQN, &escaped));
        assert!(
            error.contains("解析不出目标元素"),
            "越界前缀点明解析失败：{error}"
        );
    }
}

/// 边界（闸 ⑦ 不判面）：本模块内 `#/$defs/<名>`（零文件部分）⇒ 不作跨元素判（`Ok`）；
/// `$ref` 非字符串（形态闸管）与无 `.schema.json` 后缀的目标 ⇒ 不进入前缀判。
#[test]
fn 本模块不判前缀() {
    assert!(
        verify_ref_prefix(FQN, &json!({"$defs": {"X": {"$ref": "#/$defs/X"}}})).is_ok(),
        "本模块引用零文件部分 ⇒ Ok"
    );
    assert!(
        verify_ref_prefix(FQN, &json!({"$defs": {"X": {"$ref": 1}}})).is_ok(),
        "非字符串 `$ref` 不在本闸射程"
    );
    assert!(
        verify_ref_prefix(
            FQN,
            &json!({"$defs": {"X": {"$ref": "App.Other#/$defs/X"}}})
        )
        .is_ok(),
        "无 `.schema.json` 后缀 ⇒ 不是跨元素引用"
    );
}

/// 异常（闸 ⑧）：六键外任一（`additionalProperties` / `definitions` / `type`）⇒ `Err`；
/// 缺 `$id` / `title` / `$defs` / `properties` 各一臂 ⇒ `Err`；边界：`$schema` / `$comment`
/// 可选（在场 / 缺席两臂）⇒ `Ok`；顶层值非对象 ⇒ `Err`。
#[test]
fn 顶层键封闭() {
    for extra in ["additionalProperties", "definitions", "type"] {
        let mut value = json!({"$id": FILE, "title": FQN, "$defs": {}, "properties": {}});
        value[extra] = json!({});
        let error = contract_error(verify_top_level_keys(&value));
        assert!(error.contains(extra), "点名未知键 `{extra}`：{error}");
    }
    for missing in ["$id", "title", "$defs", "properties"] {
        let mut value = json!({"$id": FILE, "title": FQN, "$defs": {}, "properties": {}});
        value.as_object_mut().expect("对象").remove(missing);
        let error = contract_error(verify_top_level_keys(&value));
        assert!(error.contains(missing), "点名缺键 `{missing}`：{error}");
    }
    for with_optional in [
        json!({"$id": FILE, "title": FQN, "$defs": {}, "properties": {}}),
        json!({"$schema": "draft", "$comment": "注", "$id": FILE, "title": FQN, "$defs": {}, "properties": {}}),
    ] {
        assert!(
            verify_top_level_keys(&with_optional).is_ok(),
            "可选键缺省/在场两臂"
        );
    }
    for illegal in [json!([1, 2]), json!("x")] {
        let error = contract_error(verify_top_level_keys(&illegal));
        assert!(
            error.contains("顶层必须是 JSON 对象"),
            "非对象 ⇒ Err：{error}"
        );
    }
}

// ---------------------------------------------------------------------------
// verify_defs：五条规则
// ---------------------------------------------------------------------------

/// 异常（规则 1 / 规则 4 交叠）：`$defs` 值非对象 → `Err` 并点名该键（键名即身份，
/// 值形态即定义形态）。
#[test]
fn verify_defs键值必须是对象() {
    let schema = json!({"$defs": {"A": "not an object"}});
    let error = contract_error(verify_defs(&schema));
    assert!(error.contains('A'), "点名违规键：{error}");
    assert!(error.contains("对象"), "点明值的形态要求：{error}");
}

/// 边界（规则 1 / 规则 4；D37）：`$defs` 的**身份 = 键名**，故「同名两处定义」必须在
/// **文本层**判——`serde_json::from_str` 在解析阶段即把重复键折叠（后者胜、前者消失），
/// `verify_defs` 的 `&Value` 入参结构上不可达该形态。本用例钉住两侧：
///
/// ① **重复键即契约错误**（判据落文本层 `duplicate_keys`）：同一 `$defs` 对象里重复声明
///    同一键 → `validate_schema` 返回 `Err(ApiError::Contract(..))`，文案点名该键；
/// ② **身份 = 键名的判据本身成立**：键名精确比较（不归一、不合并、不加别名）——
///    `A` 与 `a` 是两个键、两键各留一份定义、`verify_defs` 前后键数不变（零副作用）。
#[test]
fn verify_defs键名身份唯一在文本层判() {
    // ① 重复键在文本层即被拦下（D37：这是唯一可判处；`serde_json` 会折叠重复键，
    //    故夹具必须是**原始文本**而非解析后的值）
    let duplicated = format!(
        "{{\"$id\": \"{FQN}.schema.json\", \"title\": \"{FQN}\", \"$defs\": \
         {{\"A\": {{\"type\": \"object\"}}, \"A\": {{\"enum\": [\"x\"]}}}}, \
         \"properties\": {{}}}}\n"
    );
    assert_eq!(
        duplicated.matches("\"A\":").count(),
        2,
        "文本里同名键声明了两次（判据的前提面）"
    );
    let error = parse_error(super::schema::validate_schema(FQN, &duplicated));
    assert!(
        error.contains("重复声明") && error.contains('A'),
        "点名重复键与「同名即定义两次」：{error}"
    );
    assert!(
        error.contains(&paths::schema_rel(FQN).expect("测试 FQN 恒为合法形态")),
        "文案带文件名与行号前缀（可直接改到那一行）：{error}"
    );

    // ② 身份 = 键名的判据成立：键名精确比较、零归一、零合并、零别名
    let cased = json!({"$defs": {
        "A": {"type": "object"},
        "a": {"type": "object"}
    }});
    let before = cased["$defs"].as_object().map(Map::len);
    assert!(
        verify_defs(&cased).is_ok(),
        "`A` 与 `a` 是两个键（身份 = 键名，不做大小写归一）"
    );
    assert_eq!(before, Some(2), "两键各留一份定义（零合并）");
    assert_eq!(
        cased["$defs"].as_object().map(Map::len),
        Some(2),
        "`verify_defs` 零副作用（不合并、不改名、不加别名）"
    );
}

/// 异常（规则 2，AC-3）：`$defs.A` 内含嵌套 `$defs` → `Err` 且点明「须提升且原名
/// 保留」；提升到顶层（原名）→ `Ok`。
#[test]
fn verify_defs嵌套defs须提升() {
    let nested = json!({"$defs": {"A": {"type": "object", "$defs": {"B": {"type": "object"}}}}});
    let error = contract_error(verify_defs(&nested));
    assert!(error.contains("嵌套"), "点明嵌套违规：{error}");
    assert!(error.contains('A'), "点名违规类型：{error}");

    let promoted = json!({"$defs": {
        "A": {"type": "object"},
        "B": {"type": "object"}
    }});
    let result = verify_defs(&promoted);
    assert!(result.is_ok(), "提升后原名保留即 Ok：{result:?}");

    // 两个不同名的顶层键均 Ok（禁静默改名/别名）
    let renamed = json!({"$defs": {
        "A_B": {"type": "object"},
        "B": {"type": "object"}
    }});
    let result = verify_defs(&renamed);
    assert!(
        result.is_ok(),
        "顶层两键并存 → Ok（不生成别名）：{result:?}"
    );
}

/// 异常（规则 3，AC-3）：`$ref` 指向 `properties` / `items` / `allOf/0` /
/// `$defs/A/properties/y` → 全 `Err` 并给原文；`#/$defs/A` 与外部文件形态 → `Ok`。
#[test]
fn verify_defs的ref只能指顶层() {
    for reference in [
        "#/properties/x",
        "#/items",
        "#/allOf/0",
        "#/$defs/A/properties/y",
    ] {
        let schema = json!({"$defs": {"A": {"$ref": reference}}});
        let error = contract_error(verify_defs(&schema));
        assert!(
            error.contains(reference),
            "文案给原文 `{reference}`：{error}"
        );
        assert!(error.contains("顶层"), "点明 target 形态：{error}");
    }

    for reference in ["#/$defs/A", "../Core/Agent.schema.json#/$defs/AgentInput"] {
        let schema = json!({"$defs": {"A": {"$ref": reference}}});
        let result = verify_defs(&schema);
        assert!(
            result.is_ok(),
            "顶层与外部文件形态合法（{reference}）：{result:?}"
        );
    }
}

/// 边界（规则 4）：两个 `$ref` 指向同一顶层条目 → `Ok`（多次引用、一次定义）；
/// `A` 与 `a` 两键 → `Ok`（键名精确比较，不做归一）。
#[test]
fn verify_defs同类型只定义一次() {
    let multi_reference = json!({"$defs": {
        "A": {"type": "object"},
        "B": {"$ref": "#/$defs/A"},
        "C": {"$ref": "#/$defs/A"}
    }});
    assert!(
        verify_defs(&multi_reference).is_ok(),
        "多引用一次定义是正确形态"
    );

    let cased = json!({"$defs": {
        "A": {"type": "object"},
        "a": {"type": "object"}
    }});
    assert!(verify_defs(&cased).is_ok(), "大小写不同即两个键（零归一）");
}

/// 边界（规则 5）：同形不同义的两类型并列 → `Ok` 且两键都保留（本函数零合并副作用）。
#[test]
fn verify_defs禁按结构去重() {
    let schema = json!({"$defs": {
        "ArchiDependsOn": {"type": "object", "properties": {"target": {"type": "string"}}},
        "ArchiDependedBy": {"type": "object", "properties": {"target": {"type": "string"}}}
    }});
    let before = schema["$defs"].as_object().map(Map::len);
    assert!(verify_defs(&schema).is_ok(), "同形不同义并列合法");
    let after = schema["$defs"].as_object().map(Map::len);
    assert_eq!(before, after, "verify_defs 零副作用（不合并）");
    assert_eq!(before, Some(2), "两键都保留");
}

/// 异常（AC-11）：透传条目带结构关键字（六形态逐项）→ `Err` 并点名键与关键字；
/// 纯 `$ref` 条目 → `Ok`。
#[test]
fn verify_defs透传条目不得带结构关键字() {
    for disallowed in ["type", "properties", "enum", "const", "items", "oneOf"] {
        let value = match disallowed {
            "type" => json!("object"),
            "properties" => json!({}),
            "enum" => json!(["a"]),
            "const" => json!("fixed"),
            "items" => json!({"type": "string"}),
            _ => json!([{"type": "string"}]),
        };
        let schema = json!({"$defs": {"AgentInput": {
            "$ref": "../Frontend/Agent.schema.json#/$defs/AgentInput",
            disallowed: value
        }}});
        let error = contract_error(verify_defs(&schema));
        assert!(error.contains("AgentInput"), "点名违规条目：{error}");
        assert!(
            error.contains(disallowed),
            "点名违规关键字 `{disallowed}`：{error}"
        );
    }

    let pure = json!({"$defs": {"AgentInput": {
        "$ref": "../Frontend/Agent.schema.json#/$defs/AgentInput"
    }}});
    assert!(verify_defs(&pure).is_ok(), "纯 $ref alias 合法");
}

/// 边界：`$defs` 缺席 → `Err`（本实现把缺 `$defs` 视为契约错误——每个具名类型 MUST
/// 在顶层定义恰一次）；`$defs = {}` → `Ok`；标准关键字（`anyOf` / `minLength` /
/// `pattern`）→ `Ok`。
#[test]
fn verify_defs空与边缘形态() {
    let no_defs = json!({"properties": {}});
    let error = contract_error(verify_defs(&no_defs));
    assert!(error.contains("$defs"), "缺 $defs 即点名：{error}");

    assert!(verify_defs(&json!({"$defs": {}})).is_ok(), "空 $defs 合法");

    let standard = json!({"$defs": {"A": {
        "anyOf": [{"type": "string"}, {"type": "integer"}],
        "minLength": 1,
        "pattern": "^[a-z]+$"
    }}});
    assert!(
        verify_defs(&standard).is_ok(),
        "原生 JSON Schema 关键字全可表达"
    );
}

// ---------------------------------------------------------------------------
// verify_defs：x-class 带属性判错（D27）
// ---------------------------------------------------------------------------

/// 异常（AC-5 / D27）：`x-class: true` 带 `properties` / `required` → `Err` 并点名
/// 类型名 + 违规键；**缺 `type: "object"` 的骨架**（`{"x-class": true}`）同样 `Err` 并给出路
/// （补 `"type": "object"`）；`x-class: false` + 带属性 → `Ok`（不参与判定）。
#[test]
fn verify_defs的x_class带属性判错() {
    let with_property = json!({"$defs": {"Gate": {
        "type": "object",
        "x-class": true,
        "properties": {"count": {"type": "integer"}}
    }}});
    let error = contract_error(verify_defs(&with_property));
    assert!(error.contains("Gate"), "点名类型名：{error}");
    assert!(error.contains("properties"), "点名违规键：{error}");
    assert!(error.contains("trait"), "点明 trait 仅允许方法：{error}");

    let with_required = json!({"$defs": {"Gate": {
        "type": "object",
        "x-class": true,
        "required": ["a"]
    }}});
    let error = contract_error(verify_defs(&with_required));
    assert!(error.contains("Gate"), "点名类型名：{error}");
    assert!(error.contains("required"), "点名违规键：{error}");

    // 缺骨架：`x-class` MUST 带 `type: "object"`（不写 `type` 的条目在标准校验器里是「任意值」，
    // 且 `type: "string"` 一类会被静默吞成 trait）——本臂由正例翻负例（AC-5 / AC-11）
    let bare = json!({"$defs": {"Gate": {"x-class": true}}});
    let bare_error = contract_error(verify_defs(&bare));
    assert!(
        bare_error.contains("type") && bare_error.contains("$defs.Gate.type"),
        "缺 `type: \"object\"` 即 `Err` 并点名 $defs.Gate.type：{bare_error}"
    );
    assert!(
        bare_error.contains(r#""type": "object""#),
        "出路是补 `\"type\": \"object\"`：{bare_error}"
    );

    let false_flag = json!({"$defs": {"Gate": {
        "x-class": false,
        "properties": {"count": {"type": "integer"}}
    }}});
    assert!(
        verify_defs(&false_flag).is_ok(),
        "x-class: false 不参与判定"
    );
}

/// 边界：`x-class` 判据不遮蔽既有五规则——嵌套 `$defs` 违例与 `x-class` 带属性违例
/// 并存时各自可判（单独去掉其一后另一条仍 `Err`）。
#[test]
fn verify_defs的x_class不吞并既有规则() {
    let nested = json!({"A": {"type": "object", "$defs": {"B": {"type": "object"}}}});
    let x_class = json!({"Gate": {
        "type": "object",
        "x-class": true,
        "properties": {"count": {"type": "integer"}}
    }});

    let mut combined = Map::new();
    combined.insert("A".to_owned(), nested["A"].clone());
    combined.insert("Gate".to_owned(), x_class["Gate"].clone());
    let both = json!({"$defs": combined});
    let message = contract_error(verify_defs(&both));

    // 单独去掉 x-class 违例 → 嵌套违例仍报
    let mut only_nested = combined.clone();
    only_nested.remove("Gate");
    let nested_only = json!({"$defs": only_nested});
    let nested_message = contract_error(verify_defs(&nested_only));
    assert!(
        nested_message.contains("嵌套") || nested_message.contains("$defs"),
        "嵌套违例独立可判：{nested_message}"
    );

    // 单独去掉嵌套违例 → x-class 违例仍报
    let mut only_x_class = combined.clone();
    only_x_class.remove("A");
    let x_class_only = json!({"$defs": only_x_class});
    let x_class_message = contract_error(verify_defs(&x_class_only));
    assert!(
        x_class_message.contains("x-class"),
        "x-class 违例独立可判：{x_class_message}"
    );
    // 并存时按固定判序先报先命中者（此处嵌套 `$defs` 排在 `x-class` 之前），
    // 关键是两条判据各自独立可判、不互相吞并
    assert!(
        message.contains("嵌套") || message.contains("x-class"),
        "并存文案指向首命中违例：{message}"
    );
    assert_ne!(
        nested_message, x_class_message,
        "两违例的单独文案互不相等（可辨）"
    );
}

/// 边界：判错返回的 `ApiError` 仍是三变体之一（`Contract`）——`x-class` 错误不新增
/// 第四变体；判据不写进 `check.rs`（源码自证十值不外扩）。
#[test]
fn verify_defs判错只出contract变体() {
    let schema = json!({"$defs": {"Gate": {
        "type": "object",
        "x-class": true,
        "properties": {"count": {"type": "integer"}}
    }}});
    match verify_defs(&schema) {
        Err(ApiError::Contract(_)) => {}
        other => panic!("x-class 判错必须是 ApiError::Contract，实际：{other:?}"),
    }

    // 源码自证：schema.rs 内只出现三变体，且 x-class 判定只在此处
    let source = include_str!("schema.rs");
    for variant in ["ApiError::Io", "ApiError::Contract", "ApiError::Element"] {
        let _ = variant;
    }
    assert!(
        !source.contains("ApiError::Element") && !source.contains("ApiError::Io"),
        "schema.rs 只产 Contract 变体（Io / Element 不在本文件）"
    );
    assert!(
        source.contains("x-class"),
        "x-class 的**错误**判据落点在本文件"
    );
}

// ---------------------------------------------------------------------------
// build_index：说明段 / 七字段 / 字典序
// ---------------------------------------------------------------------------

/// 正向（AC-17 / D13）：说明段首三行——生成警示 / review 口径 / 生成器标识；语言词
/// 禁令（文本零 `crate` / `Cargo.toml` / `Rust` / `TypeScript`）。
#[test]
fn build_index说明段三行与语言词禁令() {
    let text = build_index(&[entry("App.Backend.Infra.ArchiRuntime")], &meta());
    let lines = comment_lines(&text);
    assert_eq!(lines.len(), 3, "说明段恰三行：{lines:?}");

    assert!(lines[0].contains("勿手改"), "首行生成警示：{}", lines[0]);
    assert!(
        lines[0].contains("由 archi-api 生成"),
        "首行生成器为产品名（不是 CLI 裸名）：{}",
        lines[0]
    );
    for command in ["archi-api-gen", "api:check", "api:export", "pnpm"] {
        assert!(
            !lines[0].contains(command),
            "首行零本仓入口名 `{command}`：{}",
            lines[0]
        );
    }
    assert!(
        lines[1].contains("review") && lines[1].contains("<叶段>.md"),
        "次行 review 口径（只读同目录的叶段 md）：{}",
        lines[1]
    );
    assert!(
        lines[1].contains("<叶段>.schema.json"),
        "次行把接口定义文件（手写真相）与文档并列：{}",
        lines[1]
    );
    assert!(
        lines[1].contains("手写真相"),
        "次行点明 schema 是手写真相、生成物才是机械噪声：{}",
        lines[1]
    );
    assert!(
        lines[1].contains("declared") && lines[1].contains("接口文件在案"),
        "次行说明 status 的 declared = 接口文件在案（值域命名债的注解）：{}",
        lines[1]
    );
    assert!(
        lines[1].contains("机械噪声"),
        "次行点明 diff 是机械噪声：{}",
        lines[1]
    );

    let generator_line = &lines[2];
    assert!(
        generator_line.contains(&format!("archi-api {}", env!("CARGO_PKG_VERSION"))),
        "生成器标识含工具版本：{generator_line}"
    );
    let digest = &meta().common_digest;
    let short = digest
        .split_once(':')
        .map(|(algorithm, hex)| format!("{algorithm}:{}", &hex[..12]))
        .expect("摘要含算法前缀");
    assert!(
        generator_line.contains(&short),
        "生成器标识含 common.json 摘要前 12 位（{short}）：{generator_line}"
    );
    assert!(
        generator_line.contains("映射规则"),
        "点名映射规则资产：{generator_line}"
    );

    for forbidden in ["crate", "Cargo.toml", "Rust", "TypeScript"] {
        assert!(
            !text.contains(forbidden),
            "INDEX.toml MUST NOT 含语言词 `{forbidden}`"
        );
    }
}

/// 正向：七字段可被 `toml` 解析回等价结构；`status = Absent` → 线词 `absent` 且
/// `schema` / `generated` 为空串。
#[test]
fn build_index条目七字段() {
    let declared = IndexEntry {
        fqn: FQN.to_owned(),
        status: ApiFormStatus::Declared,
        types: 1,
        ops: 1,
        degraded: 0,
        categories: BTreeMap::new(),
        schema_rel: "Infra/ArchiRuntime.schema.json".to_owned(),
        generated_rel: Some("packages/desktop/src-tauri/crates/infra/archi.i.rs".to_owned()),
    };
    let absent = IndexEntry {
        fqn: "App.Backend.Core.Agent".to_owned(),
        status: ApiFormStatus::Absent,
        ..entry("App.Backend.Core.Agent")
    };

    let text = build_index(&[declared.clone(), absent], &meta());
    let document = index_document(&text);
    let entries = document["entry"]
        .as_array_of_tables()
        .expect("[[entry]] 是表数组");
    assert_eq!(entries.len(), 2, "两条 entry");

    // 表数组按 FQN 字典序（`App.Backend.Core.Agent` < `App.Backend.Infra.ArchiRuntime`）
    let declared_table = entries
        .iter()
        .find(|table| table["fqn"].as_str() == Some(declared.fqn.as_str()))
        .expect("declared 条目在案");
    let first = declared_table;
    assert_eq!(first["status"].as_str(), Some("declared"));
    assert_eq!(first["types"].as_integer(), Some(1));
    assert_eq!(first["ops"].as_integer(), Some(1));
    assert_eq!(first["degraded"].as_integer(), Some(0));
    assert_eq!(first["schema"].as_str(), Some(declared.schema_rel.as_str()));
    assert_eq!(
        first["generated"].as_str(),
        declared.generated_rel.as_deref()
    );

    assert_eq!(entries.len(), 2, "两条 entry（按 FQN 字典序）");
    let absent_table = entries
        .iter()
        .find(|table| table["fqn"].as_str() == Some("App.Backend.Core.Agent"))
        .expect("absent 条目在案");
    let second = absent_table;
    assert_eq!(second["status"].as_str(), Some("absent"), "线词 absent");
    assert_eq!(second["schema"].as_str(), Some(""), "absent 时 schema 空串");
    assert_eq!(
        second["generated"].as_str(),
        Some(""),
        "absent 时 generated 空串"
    );
}

/// 正向（AC-2 / D26）：`categories` 非空 → 文本含 `[entry.categories]` 子表且可反解析；
/// `sum(categories) == types` 对每个 `Declared` 条目成立。
#[test]
fn build_index的categories子表() {
    let single = IndexEntry {
        fqn: FQN.to_owned(),
        types: 1,
        ops: 1,
        categories: BTreeMap::from([("object".to_owned(), 1)]),
        ..entry(FQN)
    };
    let multi = IndexEntry {
        fqn: "App.Frontend.Views.AgentDebug".to_owned(),
        types: 3,
        categories: BTreeMap::from([("object".to_owned(), 2), ("reexport".to_owned(), 1)]),
        ..entry("App.Frontend.Views.AgentDebug")
    };

    let text = build_index(&[single.clone(), multi.clone()], &meta());
    assert!(
        text.contains("[entry.categories]"),
        "非空 categories 输出子表：\n{text}"
    );

    let document = index_document(&text);
    let entries = document["entry"]
        .as_array_of_tables()
        .expect("[[entry]] 是表数组");
    for (index, expected) in [single, multi].iter().enumerate() {
        let table = entries.get(index).expect("逐条 entry");
        let categories = table["categories"].as_table().expect("categories 是子表");
        assert_eq!(
            categories.len(),
            expected.categories.len(),
            "子表键数逐项相等"
        );
        let mut total = 0;
        for (key, value) in &expected.categories {
            assert_eq!(
                table["categories"][key.as_str()]
                    .as_integer()
                    .map(|n| n as usize),
                Some(*value),
                "categories.{key} 逐项相等"
            );
            total += value;
        }
        assert_eq!(
            total,
            table["types"].as_integer().map(|n| n as usize).unwrap_or(0),
            "sum(categories) == types"
        );
    }
}

/// 边界（D26）：`categories` 为空表 → 文本零 `categories` 段（MUST NOT 输出空表）；
/// `entries = []` → 全文零 `categories`。
#[test]
fn build_index的categories空表整段省略() {
    let absent = entry("App.Backend.Core.Agent");
    let empty_contract = IndexEntry {
        fqn: "App.Backend.Core.Archi".to_owned(),
        types: 0,
        ..entry("App.Backend.Core.Archi")
    };

    for entries in [
        vec![absent.clone()],
        vec![empty_contract.clone()],
        Vec::new(),
    ] {
        let text = build_index(&entries, &meta());
        assert!(
            !text.contains("categories"),
            "空 categories MUST NOT 输出段：\n{text}"
        );
    }
}

/// 边界（D26）：六形状各一条 → `types = 6`、六键各 1、键序为 `BTreeMap` 字典序；
/// 全部同形状（5 条 `object`）→ `types = 5`。
#[test]
fn build_index的categories与types同源() {
    let six = IndexEntry {
        fqn: FQN.to_owned(),
        types: 6,
        categories: BTreeMap::from([
            ("object".to_owned(), 1),
            ("enum".to_owned(), 1),
            ("variant".to_owned(), 1),
            ("class".to_owned(), 1),
            ("scalar".to_owned(), 1),
            ("reexport".to_owned(), 1),
        ]),
        ..entry(FQN)
    };
    let text = build_index(&[six], &meta());
    let document = index_document(&text);
    let categories = document["entry"]
        .as_array_of_tables()
        .expect("表数组")
        .get(0)
        .expect("首条")["categories"]
        .as_table()
        .expect("子表")
        .clone();
    let keys: Vec<String> = categories.iter().map(|(key, _)| key.to_owned()).collect();
    assert_eq!(
        keys,
        vec![
            "class".to_owned(),
            "enum".to_owned(),
            "object".to_owned(),
            "reexport".to_owned(),
            "scalar".to_owned(),
            "variant".to_owned(),
        ],
        "键序 = BTreeMap 字典序"
    );

    let same_shape = IndexEntry {
        fqn: FQN.to_owned(),
        types: 5,
        categories: BTreeMap::from([("object".to_owned(), 5)]),
        ..entry(FQN)
    };
    let document = index_document(&build_index(&[same_shape], &meta()));
    let first = document["entry"]
        .as_array_of_tables()
        .expect("表数组")
        .get(0)
        .expect("首条")
        .clone();
    assert_eq!(first["types"].as_integer(), Some(5));
    assert_eq!(first["categories"]["object"].as_integer(), Some(5));
}

/// 边界（D26）：`categories` 键空间开放——六值外的字符串、含 `-` 与 emoji 的键名
/// 经 TOML 转义后仍可反解析回原键。
#[test]
fn build_index的categories键空间开放() {
    let open = IndexEntry {
        fqn: FQN.to_owned(),
        types: 3,
        categories: BTreeMap::from([
            ("未来新形状".to_owned(), 1),
            ("kebab-case".to_owned(), 1),
            ("🎯".to_owned(), 1),
        ]),
        ..entry(FQN)
    };
    let text = build_index(std::slice::from_ref(&open), &meta());
    let document = index_document(&text);
    let categories = document["entry"]
        .as_array_of_tables()
        .expect("表数组")
        .get(0)
        .expect("首条")["categories"]
        .as_table()
        .expect("子表")
        .clone();
    for (key, value) in &open.categories {
        assert_eq!(
            categories.get(key).and_then(|item| item.as_integer()),
            Some(*value as i64),
            "开放的键名往返：{key}"
        );
    }
}

/// 异常（D26 反例）：形状键集与判据分类六值**无交集**（`reexport` 是同名者，但是纯
/// `$ref` alias 计数而非判据条数）；`build_index` 入参无 finding，故文本不随债数变化。
#[test]
fn build_index的categories不取判据分类() {
    let shape_keys = ["object", "enum", "variant", "class", "scalar", "reexport"];
    let judge_categories = [
        "entry",
        "contract",
        "signature",
        "reexport",
        "nesting",
        "constructor",
    ];
    let shared: Vec<&&str> = shape_keys
        .iter()
        .filter(|key| judge_categories.contains(key))
        .collect();
    assert_eq!(shared.len(), 1, "唯一同名键是 reexport");
    assert_eq!(*shared[0], "reexport", "同名者是纯 $ref alias 计数");

    // 结构保证：同 entries 输入两次输出逐字相等（finding 不参与入参）
    let entry = IndexEntry {
        fqn: FQN.to_owned(),
        types: 2,
        categories: BTreeMap::from([("object".to_owned(), 1), ("reexport".to_owned(), 1)]),
        ..entry(FQN)
    };
    let first = build_index(std::slice::from_ref(&entry), &meta());
    let second = build_index(&[entry], &meta());
    assert_eq!(first, second, "INDEX 文本不随外部状态变化");
}

/// 边界：入参乱序 → 输出按 FQN 字典序；同输入两次逐字相等。
#[test]
fn build_index字典序与幂等() {
    let entries = vec![entry("C"), entry("A"), entry("B")];
    let text = build_index(&entries, &meta());
    let document = index_document(&text);
    let order: Vec<String> = document["entry"]
        .as_array_of_tables()
        .expect("表数组")
        .iter()
        .map(|table| table["fqn"].as_str().expect("fqn 是字符串").to_owned())
        .collect();
    assert_eq!(order, vec!["A".to_owned(), "B".to_owned(), "C".to_owned()]);

    let again = build_index(&entries, &meta());
    assert_eq!(text, again, "同输入两次逐字相等");
}

/// 边界：`entries = []` → 说明段三行在场、零 `[[entry]]`、`toml` 解析为空表；
/// 1000 条 → 全部在场、序稳定。
#[test]
fn build_index空条目与规模() {
    let empty = build_index(&[], &meta());
    assert_eq!(comment_lines(&empty).len(), 3, "说明段仍在场");
    assert!(!empty.contains("[[entry]]"), "零条目时无表数组段");
    let document = index_document(&empty);
    assert!(document.get("entry").is_none(), "空条目不产出 entry 键");

    let many: Vec<IndexEntry> = (0..1000)
        .map(|index| entry(&format!("App.E{index:04}")))
        .collect();
    let text = build_index(&many, &meta());
    let document = index_document(&text);
    assert_eq!(
        document["entry"]
            .as_array_of_tables()
            .expect("表数组")
            .len(),
        1000,
        "1000 条全部在场"
    );
}

/// 边界：`types = 0` / `ops = 0` / `degraded = 0` 输出 `0` 而非省略；极大计数不饱和。
#[test]
fn build_index计数边界() {
    let zero = IndexEntry {
        fqn: "App.Backend.Core.Agent".to_owned(),
        types: 0,
        ops: 0,
        degraded: 0,
        ..entry("App.Backend.Core.Agent")
    };
    let document = index_document(&build_index(&[zero], &meta()));
    let table = document["entry"]
        .as_array_of_tables()
        .expect("表数组")
        .get(0)
        .expect("首条")
        .clone();
    for key in ["types", "ops", "degraded"] {
        assert_eq!(
            table[key].as_integer(),
            Some(0),
            "计数为 0 时仍输出键 `{key}`"
        );
    }

    // usize::MAX 经 as i64 转换后无饱和（本仓 64 位平台下 i64 可容纳 usize 上界以下的计数）
    let large = IndexEntry {
        fqn: FQN.to_owned(),
        types: u32::MAX as usize,
        ops: u32::MAX as usize,
        degraded: 2,
        ..entry(FQN)
    };
    let text = build_index(&[large], &meta());
    assert!(
        text.contains(&u32::MAX.to_string()),
        "大计数原样输出（无饱和）：\n{text}"
    );
    let document = index_document(&text);
    assert_eq!(
        document["entry"]
            .as_array_of_tables()
            .expect("表数组")
            .get(0)
            .expect("首条")["degraded"]
            .as_integer(),
        Some(2),
        "degraded 计数原样"
    );
}

/// 边界：FQN 含 emoji / 值含 `\n` 与 `"` → TOML 转义后可反解析回原值；超长 FQN 不
/// panic。
#[test]
fn build_index特殊字符转义() {
    let emoji = IndexEntry {
        fqn: "App.🎯.Target".to_owned(),
        schema_rel: "Infra/含 \"引号\" 与\n换行.json".to_owned(),
        ..entry("App.🎯.Target")
    };
    let document = index_document(&build_index(std::slice::from_ref(&emoji), &meta()));
    let table = document["entry"]
        .as_array_of_tables()
        .expect("表数组")
        .get(0)
        .expect("首条")
        .clone();
    assert_eq!(table["fqn"].as_str(), Some(emoji.fqn.as_str()));
    assert_eq!(table["schema"].as_str(), Some(emoji.schema_rel.as_str()));

    let long = format!("App.{}", "x".repeat(1200));
    let text = build_index(&[entry(&long)], &meta());
    assert!(
        text.contains(&long),
        "超长 FQN 原样落盘（不 panic、不截断）"
    );
}

/// 边界：`degraded` 计数只反映入参（`x-common.degraded` 条目的派生物由 `render_all`
/// 侧给出——本函数不重算）。
#[test]
fn build_index的degraded取入参() {
    let degraded = IndexEntry {
        fqn: FQN.to_owned(),
        degraded: 2,
        ..entry(FQN)
    };
    let document = index_document(&build_index(&[degraded], &meta()));
    assert_eq!(
        document["entry"]
            .as_array_of_tables()
            .expect("表数组")
            .get(0)
            .expect("首条")["degraded"]
            .as_integer(),
        Some(2),
        "2 条降级留痕原样输出"
    );
}

/// 边界：`ApiFormStatus::as_str` 两值线词（索引的唯一写出面）。
#[test]
fn api_form_status线词二值() {
    assert_eq!(ApiFormStatus::Declared.as_str(), "declared");
    assert_eq!(ApiFormStatus::Absent.as_str(), "absent");
}

/// 边界：`paths::schema_path` 与 INDEX 的 `schema_rel` 同源（目录单点）。
#[test]
fn index_schema_rel与paths同源() {
    let root = std::path::Path::new("/ws");
    let expected = paths::schema_path(root, FQN).expect("测试 FQN 恒为合法形态");
    assert!(
        expected.ends_with(format!("{}.schema.json", paths::leaf_name(FQN))),
        "schema 路径文件名取叶段：{expected:?}"
    );
}

// ---------------------------------------------------------------------------
// 降级词表与全深度标记扫描（AC-5 / D11 / D12）
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`DEGRADED_KINDS` 逐值断言顺序与内容（词表封闭四值）。
#[test]
fn degraded_kinds逐值封闭() {
    assert_eq!(
        DEGRADED_KINDS,
        [
            "foreignElement",
            "noElement",
            "unrepresentable",
            "crossLanguage"
        ],
        "词表顺序与内容逐值钉住（新增第五值 MUST 改 spec）"
    );
}

/// 边界（AC-5）：长度恰 4、零重复、每值为非空 ASCII 驼峰词（无空格 / 连字符）——常量
/// 本身无异常面（取值合法性由 `verify_defs` 用例承担）。
#[test]
fn degraded_kinds形态与长度() {
    assert_eq!(DEGRADED_KINDS.len(), 4, "词表恰四值");
    let mut unique: Vec<&str> = DEGRADED_KINDS.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 4, "零重复");
    for kind in DEGRADED_KINDS {
        assert!(!kind.is_empty(), "值非空");
        assert!(
            kind.chars()
                .all(|character| character.is_ascii_alphabetic()),
            "`{kind}` 是纯 ASCII 驼峰词（无空格 / 连字符 / 数字）"
        );
        assert!(
            kind.chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase()),
            "`{kind}` 首字母小写（驼峰词形态）"
        );
    }
}

/// 正向（AC-5）：`$defs` 条目自身标记 → pointer 指到标记本身、值取原文 `&Value`；
/// 属性位标记 → pointer 到属性值（旧口径看不见的落点）。
#[test]
fn degraded_marks条目自身与属性位pointer() {
    let own = json!({"$defs": {"Widget": {"x-common": {"degraded": "noElement"}}}});
    let marks = degraded_marks(&own);
    assert_eq!(marks.len(), 1, "条目自身一处标记：{marks:?}");
    assert_eq!(
        marks[0].0, "$.$defs.Widget.x-common.degraded",
        "pointer 指到标记本身"
    );
    assert_eq!(
        marks[0].1,
        &json!("noElement"),
        "值取原文 `&Value`（不转写、不猜语义）"
    );

    let property = json!({"$defs": {"Widget": {
        "type": "object",
        "properties": {"session": {"x-common": {"degraded": "foreignElement"}}}
    }}});
    let marks = degraded_marks(&property);
    assert_eq!(marks.len(), 1, "属性位一处标记：{marks:?}");
    assert_eq!(
        marks[0].0, "$.$defs.Widget.properties.session.x-common.degraded",
        "pointer 到属性值（旧条目级口径看不见的落点）"
    );
}

/// 正向（表驱动）：深层标记——`properties.x.items`、`x-function.params[0].schema`、
/// `additionalProperties` 三处各扫出，pointer 含 `.items` 段与数组段 `[0]`。
#[test]
fn degraded_marks深层标记() {
    let cases = [
        (
            "items",
            json!({"$defs": {"W": {"type": "array", "items": {"x-common": {"degraded": "noElement"}}}}}),
            "$.$defs.W.items.x-common.degraded",
        ),
        (
            "params[0].schema",
            json!({"$defs": {"W": {"type": "object", "properties": {"send": {"x-function": {
                "params": [{"name": "input", "schema": {"x-common": {"degraded": "crossLanguage"}}}],
                "returns": {"type": "boolean"}
            }}}}}}),
            "$.$defs.W.properties.send.x-function.params[0].schema.x-common.degraded",
        ),
        (
            "additionalProperties",
            json!({"$defs": {"W": {"type": "object", "additionalProperties": {"x-common": {"degraded": "unrepresentable"}}}}}),
            "$.$defs.W.additionalProperties.x-common.degraded",
        ),
    ];
    for (label, schema, expected) in cases {
        let marks = degraded_marks(&schema);
        assert_eq!(marks.len(), 1, "{label}：一处标记：{marks:?}");
        assert_eq!(marks[0].0, expected, "{label}：pointer 逐段可读");
    }
}

/// 异常（表驱动）：根节点自身标记 → pointer `$.x-common.degraded`；标量 / 数组根 →
/// 空 `Vec`（零 panic）。
#[test]
fn degraded_marks根节点与标量根() {
    let root = json!({"x-common": {"degraded": "noElement"}, "$defs": {}});
    let marks = degraded_marks(&root);
    assert_eq!(marks.len(), 1, "根节点自身一处标记：{marks:?}");
    assert_eq!(marks[0].0, "$.x-common.degraded", "根节点段的 pointer");

    for schema in [json!(null), json!(42), json!("text"), json!([1, 2])] {
        assert!(
            degraded_marks(&schema).is_empty(),
            "标量 / 标量数组根 → 空 `Vec`（零 panic）：{schema}"
        );
    }
}

/// 异常（表驱动）：`x-common` 非对象（字符串 / 数组 / null）或对象但无 `degraded` 键
/// → 不计入（锚点是对象自身的 `x-common.degraded`）。
#[test]
fn degraded_marks的x_common非对象() {
    for schema in [
        json!({"$defs": {"W": {"x-common": "noElement"}}}),
        json!({"$defs": {"W": {"x-common": ["degraded"]}}}),
        json!({"$defs": {"W": {"x-common": null}}}),
        json!({"$defs": {"W": {"x-common": {"note": "待剪枝：自由注记"}}}}),
        json!({"$defs": {"W": {"x-common": {}}}}),
    ] {
        assert!(
            degraded_marks(&schema).is_empty(),
            "非 `x-common.degraded` 形态不计入：{schema}"
        );
    }
}

/// 边界（AC-5 关键反例）：数据字段名恰为 `degraded`（`properties.degraded`）→ 不计入
/// （锚点判据；松锚点会把数据字段计成债）。
#[test]
fn degraded_marks数据字段名degraded不计入() {
    let schema = json!({"$defs": {"W": {
        "type": "object",
        "properties": {"degraded": {"type": "string"}}
    }}});
    assert!(
        degraded_marks(&schema).is_empty(),
        "数据字段名 `degraded` 不是标记（锚点是 `x-common.degraded`）：{schema}"
    );
}

/// 边界（表驱动）：标记值为 `true` / 数字 / `null` → 仍被扫出（扫描不看取值类型；
/// 合法性归 `verify_defs`）；`x-common.note` 永不计入。
#[test]
fn degraded_marks非字符串取值仍扫出() {
    for value in [json!(true), json!(42), json!(null)] {
        let schema = json!({"$defs": {"W": {"x-common": {"degraded": value}}}});
        let marks = degraded_marks(&schema);
        assert_eq!(marks.len(), 1, "取值 {value} 仍被扫出：{marks:?}");
        assert_eq!(marks[0].1, &value, "值原样带出");
    }

    let note_only =
        json!({"$defs": {"W": {"x-common": {"note": "待剪枝：AgentSessionSendInput"}}}});
    assert!(
        degraded_marks(&note_only).is_empty(),
        "`x-common.note` 永不计入：{note_only}"
    );
}

/// 边界（表驱动）：同一 schema 多处标记 → 逐处各一条，pointer 集合与出现点逐一对应；
/// 同一数组内按索引序（`[0]` 先于 `[1]`）；零标记 → 空 `Vec`。
#[test]
fn degraded_marks多处标记与索引序() {
    let schema = json!({"$defs": {"W": {
        "x-common": {"degraded": "noElement"},
        "type": "object",
        "properties": {
            "a": {"x-common": {"degraded": "foreignElement"}},
            "b": {"type": "array", "items": {"x-common": {"degraded": "unrepresentable"}}}
        }
    }}});
    let mut pointers: Vec<String> = degraded_marks(&schema)
        .into_iter()
        .map(|(pointer, _)| pointer)
        .collect();
    assert_eq!(pointers.len(), 3, "三处标记逐处各一条：{pointers:?}");
    pointers.sort();
    assert_eq!(
        pointers,
        vec![
            "$.$defs.W.properties.a.x-common.degraded".to_owned(),
            "$.$defs.W.properties.b.items.x-common.degraded".to_owned(),
            "$.$defs.W.x-common.degraded".to_owned(),
        ],
        "pointer 集合与出现点逐一对应"
    );

    let array = json!({"$defs": {"W": {"oneOf": [
        {"x-common": {"degraded": "noElement"}},
        {"x-common": {"degraded": "crossLanguage"}}
    ]}}});
    let ordered: Vec<String> = degraded_marks(&array)
        .into_iter()
        .map(|(pointer, _)| pointer)
        .collect();
    assert_eq!(
        ordered,
        vec![
            "$.$defs.W.oneOf[0].x-common.degraded".to_owned(),
            "$.$defs.W.oneOf[1].x-common.degraded".to_owned(),
        ],
        "同一数组内按索引序"
    );

    assert!(
        degraded_marks(&json!({"$defs": {"W": {"type": "object"}}})).is_empty(),
        "零标记 → 空 `Vec`"
    );
}

// ---------------------------------------------------------------------------
// verify_defs 的第 6 条规则：降级取值词表（AC-5 / D11）
// ---------------------------------------------------------------------------

/// 正向（AC-5，逐值）：四值逐个通过（同一契约的条目自身 + 属性位各一）。
#[test]
fn verify_defs降级四值逐个通过() {
    for kind in DEGRADED_KINDS {
        let schema = json!({"$defs": {"W": {
            "x-common": {"degraded": kind},
            "type": "object",
            "properties": {"session": {"x-common": {"degraded": kind}}}
        }}});
        assert_eq!(
            verify_defs(&schema),
            Ok(()),
            "词表内取值 `{kind}`（条目自身 + 属性位）应通过"
        );
    }
}

/// 正向（表驱动）：零标记 / 仅 `x-common.note` / 仅 `x-common.unit` → `Ok`；`note` 零
/// 校验（自由注记）。
#[test]
fn verify_defs零标记与仅note() {
    let cases = [
        ("零标记", json!({"$defs": {"W": {"type": "object"}}})),
        (
            "仅 note",
            json!({"$defs": {"W": {"x-common": {"note": "待剪枝：AgentSessionSendInput"}}}}),
        ),
        (
            "仅 unit",
            json!({"$defs": {"W": {"x-common": {"unit": "毫秒"}}}}),
        ),
    ];
    for (label, schema) in cases {
        assert_eq!(
            verify_defs(&schema),
            Ok(()),
            "{label}：应通过（`note` 零校验）"
        );
    }
}

/// 异常（AC-5 关键反例）：`x-common.degraded = "typo"` → `Err`，文案含该标记 pointer 并
/// 并列四值。
#[test]
fn verify_defs词表外值报错() {
    let schema = json!({"$defs": {"Widget": {"x-common": {"degraded": "typo"}}}});
    let error = contract_error(verify_defs(&schema));
    assert!(
        error.contains("$.$defs.Widget.x-common.degraded"),
        "文案点名该标记的 pointer：{error}"
    );
    assert!(error.contains("typo"), "文案点名非法取值：{error}");
    for kind in DEGRADED_KINDS {
        assert!(error.contains(kind), "文案并列四值（缺 `{kind}`）：{error}");
    }
}

/// 异常（表驱动）：`x-common.degraded = true` → `Err`，文案含 pointer、`kind_of` 形态
/// （布尔）与四值；`null` / 数字 / 数组 / 对象同拒。
#[test]
fn verify_defs非字符串取值报错() {
    let cases = [
        (json!(true), "布尔"),
        (json!(null), "null"),
        (json!(7), "数字"),
        (json!(["noElement"]), "数组"),
        (json!({"kind": "noElement"}), "对象"),
    ];
    for (value, kind) in cases {
        let schema = json!({"$defs": {"Widget": {"x-common": {"degraded": value}}}});
        let error = contract_error(verify_defs(&schema));
        assert!(
            error.contains("$.$defs.Widget.x-common.degraded"),
            "取值 {value}：文案含 pointer：{error}"
        );
        assert!(
            error.contains(kind),
            "取值 {value}：文案经 `kind_of` 报形态 `{kind}`：{error}"
        );
        assert!(
            error.contains("crossLanguage"),
            "取值 {value}：文案并列四值：{error}"
        );
    }
}

/// 异常（表驱动）：深层非法值（`items` / `params[].schema` 内）→ `Err` 并点名该
/// pointer（校验面与计数面同源）。
#[test]
fn verify_defs深层非法值() {
    let cases = [
        (
            json!({"$defs": {"W": {"type": "array", "items": {"x-common": {"degraded": "typo"}}}}}),
            "$.$defs.W.items.x-common.degraded",
        ),
        (
            json!({"$defs": {"W": {"type": "object", "properties": {"send": {"x-function": {
                "params": [{"name": "input", "schema": {"x-common": {"degraded": true}}}],
                "returns": {"type": "boolean"}
            }}}}}}),
            "$.$defs.W.properties.send.x-function.params[0].schema.x-common.degraded",
        ),
    ];
    for (schema, pointer) in cases {
        let error = contract_error(verify_defs(&schema));
        assert!(error.contains(pointer), "深层非法值点名该 pointer：{error}");
    }
}

/// 边界（表驱动，既有规则零回归）：嵌套 `$defs` / `$ref` 非顶层 / 同名两次 / `x-class`
/// 带属性 / 透传条目带结构关键字各自仍报既有错误（新增取值校验不遮蔽）。
#[test]
fn verify_defs既有规则零回归() {
    let nested = json!({"$defs": {"A": {"type": "object", "$defs": {"B": {"type": "object"}}}}});
    let nested_error = contract_error(verify_defs(&nested));
    assert!(
        nested_error.contains("嵌套"),
        "嵌套 `$defs`：{nested_error}"
    );

    let non_top = json!({"$defs": {"A": {"$ref": "#/properties/x"}}});
    let non_top_error = contract_error(verify_defs(&non_top));
    assert!(
        non_top_error.contains("顶层"),
        "`$ref` 非顶层：{non_top_error}"
    );

    let duplicated = format!(
        "{{\"$id\": \"{FQN}.schema.json\", \"title\": \"{FQN}\", \"$defs\": \
         {{\"A\": {{\"type\": \"object\"}}, \"A\": {{\"enum\": [\"x\"]}}}}, \
         \"properties\": {{}}}}\n"
    );
    let duplicate_error = parse_error(super::schema::validate_schema(FQN, &duplicated));
    assert!(
        duplicate_error.contains("重复声明"),
        "同名两次（文本层判据）：{duplicate_error}"
    );

    let x_class = json!({"$defs": {"Gate": {"type": "object", "x-class": true, "properties": {"count": {"type": "integer"}}}}});
    let x_class_error = contract_error(verify_defs(&x_class));
    assert!(
        x_class_error.contains("x-class") && x_class_error.contains("trait"),
        "`x-class` 带属性：{x_class_error}"
    );

    let reexport = json!({"$defs": {"A": {
        "$ref": "../Core/Agent.schema.json#/$defs/AgentInput",
        "type": "object"
    }}});
    let reexport_error = contract_error(verify_defs(&reexport));
    assert!(
        reexport_error.contains("结构关键字") && reexport_error.contains("type"),
        "透传条目带结构关键字：{reexport_error}"
    );
}

/// 边界：同契约既有五条违例 + 非法 `degraded` 并存 → 报既有规则（取值校验排在既有规则
/// 之后，错误面稳定）；缺 `$defs` / 空对象 → 既有错误不变。
#[test]
fn verify_defs取值校验排在既有规则之后() {
    let both = json!({"$defs": {"A": {
        "type": "object",
        "$defs": {"B": {"type": "object"}},
        "x-common": {"degraded": "typo"}
    }}});
    let message = contract_error(verify_defs(&both));
    assert!(
        message.contains("嵌套"),
        "先报既有规则（嵌套 `$defs`）：{message}"
    );
    assert!(
        !message.contains("crossLanguage"),
        "取值校验不抢先报（错误面稳定）：{message}"
    );

    let missing = contract_error(verify_defs(&json!({"properties": {}})));
    assert!(
        missing.contains("$defs"),
        "缺 `$defs` 的既有错误不变：{missing}"
    );
    assert_eq!(
        verify_defs(&json!({"$defs": {}})),
        Ok(()),
        "空 `$defs` 仍合法（零标记可校验）"
    );
}

/// 正向（AC-2）：说明段第三行以「契约面：目录」起句并含
/// `foundation::layout::archi_models_rel_dir()` 的逐字结果；索引目录（`graph`）与旧方位
/// 串（`architecture/api`）零命中——索引与契约分家后的方向判据。
#[test]
fn build_index说明段取契约资产目录() {
    let text = build_index(&[entry(FQN)], &meta());
    let lines = comment_lines(&text);
    assert_eq!(lines.len(), 3, "说明段恰三行：{lines:?}");

    let contracts = foundation::layout::archi_models_rel_dir();
    let index_dir = foundation::layout::archi_graph_rel_dir();
    let generator_line = &lines[2];
    assert!(
        generator_line.contains("契约面：目录"),
        "第三行以「契约面：目录」起句：{generator_line}"
    );
    assert!(
        generator_line.contains(&contracts),
        "含 `archi_models_rel_dir()` 逐字结果（{contracts}）：{generator_line}"
    );
    assert!(
        !generator_line.contains(&index_dir),
        "说明段 MUST NOT 取索引自身目录（{index_dir}）：{generator_line}"
    );
    assert!(
        !text.contains("architecture/api"),
        "旧方位串 `architecture/api` 零残留"
    );
}

/// 边界（AC-2）：说明段与条目规模无关——单条目与 41 条两规模下说明段逐字相同（确定性 /
/// 零状态依赖）。
#[test]
fn build_index说明段与条目规模无关() {
    let single = comment_lines(&build_index(&[entry("App.Core.Alpha")], &meta()));
    let many: Vec<IndexEntry> = (0..41)
        .map(|index| entry(&format!("App.E{index:02}")))
        .collect();
    let wide = comment_lines(&build_index(&many, &meta()));

    assert_eq!(single.len(), 3, "单条目说明段三行：{single:?}");
    assert_eq!(wide.len(), 3, "41 条说明段三行：{wide:?}");
    assert_eq!(single, wide, "说明段不随条目数变化（同输入同输出）");
}

// ---------------------------------------------------------------------------
// 退役面：生成器让位而注记分家
// ---------------------------------------------------------------------------

/// 异常（AC-2 / AC-5 源码自证）：`build_schema` 零命中（生成器不再产接口文件）；
/// `GENERATED_NOTICE` 仍含「勿手改」与生成物检查语义且零本仓脚本名；`SCHEMA_WRITER_NOTICE`
/// 在场且含「手写接口真相」与 `archi_api` 的 `check` 自查。
#[test]
fn 生成器让位而注记分家() {
    let source = include_str!("schema.rs");
    assert_eq!(
        source.matches("pub fn build_schema").count(),
        0,
        "`build_schema` 已退役（接口文件是手写资产）"
    );
    let module = include_str!("mod.rs");
    for needle in ["本文件由元素的契约 markdown 生成", "勿手改", "生成物检查"] {
        assert!(module.contains(needle), "`GENERATED_NOTICE` 含 `{needle}`");
    }
    for command in ["api:check", "api:export", "pnpm"] {
        assert!(
            !module.contains(command),
            "`GENERATED_NOTICE` 零本仓脚本名 `{command}`"
        );
    }
    for needle in ["手写接口真相", "`archi_api` 的 `check`"] {
        assert!(
            module.contains(needle),
            "`SCHEMA_WRITER_NOTICE` 含 `{needle}`"
        );
    }
    assert!(
        module.matches("SCHEMA_WRITER_NOTICE").count() >= 2,
        "写者纪律注在案并被骨架使用"
    );
}

// ---------------------------------------------------------------------------
// x-class 骨架 / 构造入口 / 静态方法（AC-4 / AC-5 / AC-6 / AC-7 / AC-8）
// ---------------------------------------------------------------------------

/// `$defs` 片段 → `{"$defs": …}` schema 值（判据面只吃 `$defs`，其余顶层键不参与）。
fn defs_value(defs: &str) -> Value {
    serde_json::from_str(&format!("{{\"$defs\": {defs}}}"))
        .unwrap_or_else(|error| panic!("夹具 `$defs` 片段必须是合法 JSON：{error}\n{defs}"))
}

/// `$defs.Gate` 取给定定义原文的 schema 值。
fn gate(definition: &str) -> Value {
    defs_value(&format!("{{\"Gate\": {definition}}}"))
}

/// `$defs.Gate` 的 `x-class` 骨架（`type` + `x-class` 固定在场，`rest` 逐字追加）。
fn class_gate(rest: &str) -> Value {
    let body = if rest.trim().is_empty() {
        r#"{"type": "object", "x-class": true}"#.to_owned()
    } else {
        format!(r#"{{"type": "object", "x-class": true, {rest}}}"#)
    };
    gate(&body)
}

/// 正向（AC-5 正例面 / 骨架四判据全过）：空 `properties`（空 trait）、全 `x-function`
/// 方法、`x-class: false` + 带属性、`x-class` 为字符串 `"true"`（非 bool 不命中 class
/// 分支）四臂 ⇒ 全 `Ok`。
#[test]
fn verify_defs合法骨架面全过() {
    let cases = [
        (
            "空 properties 的空 trait",
            r#"{"Gate": {"type": "object", "x-class": true, "properties": {}}}"#,
        ),
        (
            "全 x-function 方法的 trait",
            r#"{"Gate": {"type": "object", "x-class": true, "properties": {"run": {"x-function": {"params": [], "returns": {"type": "string"}}}}}}"#,
        ),
        (
            "x-class: false 带属性",
            r#"{"Gate": {"type": "object", "x-class": false, "properties": {"count": {"type": "integer"}}}}"#,
        ),
        (
            "x-class 值为字符串 \"true\"",
            r#"{"Gate": {"type": "object", "x-class": "true", "properties": {"count": {"type": "integer"}}}}"#,
        ),
    ];
    for (label, defs) in cases {
        assert_eq!(
            verify_defs(&defs_value(defs)),
            Ok(()),
            "{label}：骨架四判据应全过"
        );
    }
}

/// 异常（AC-5）：缺骨架两主形态——`{"x-class": true}`（`type` 缺省）与
/// `{"type":"string","x-class":true}` ⇒ 各 `Err`，文案点名 `$defs.Gate.type`、报实际形态、
/// 含出路「补 `\"type\": \"object\"`」。
#[test]
fn verify_defs缺骨架两主形态报错() {
    let bare = contract_error(verify_defs(&gate(r#"{"x-class": true}"#)));
    assert!(bare.contains("$defs.Gate.type"), "点名键位：{bare}");
    assert!(bare.contains("缺省"), "报实际形态（缺省）：{bare}");
    assert!(bare.contains(r#""type": "object""#), "含可执行出路：{bare}");

    let scalar = contract_error(verify_defs(&gate(r#"{"type": "string", "x-class": true}"#)));
    assert!(scalar.contains("$defs.Gate.type"), "点名键位：{scalar}");
    assert!(scalar.contains("`string`"), "报实际形态：{scalar}");
    assert!(
        scalar.contains(r#""type": "object""#),
        "含可执行出路：{scalar}"
    );
    assert_ne!(bare, scalar, "两形态文案互不相等（逐条可辨）");
}

/// 异常（AC-5，str 边界四值）：`type` 为 `"Object"`（大小写）/ `"object "`（尾空格）/
/// `""`（空串）/ `null` ⇒ 各 `Err`（不归一、不容错）。
#[test]
fn verify_defs的type四边界值全报错() {
    for actual in [r#""Object""#, r#""object ""#, r#""""#, "null"] {
        let defs = format!(r#"{{"Gate": {{"type": {actual}, "x-class": true}}}}"#);
        let message = contract_error(verify_defs(&defs_value(&defs)));
        assert!(
            message.contains("$defs.Gate.type"),
            "`type` = {actual} ⇒ 点名键位：{message}"
        );
        assert!(
            message.contains(r#""type": "object""#),
            "`type` = {actual} ⇒ 含出路：{message}"
        );
    }
}

/// 异常（AC-5 / 语义互斥）：`additionalProperties` 对象 / 布尔 `false` / 布尔 `true` /
/// 空对象四形态 ⇒ 各 `Err`，文案点名该键并含两条可执行出路（去掉 `x-class` 变纯数据且
/// 说明条目层 map 本期不渲染；方法集合则去掉该键）。
#[test]
fn verify_defs的additional_properties四形态互斥() {
    let cases = [
        ("对象形态", r#"{"type": "string"}"#),
        ("布尔 false", "false"),
        ("布尔 true", "true"),
        ("空对象", "{}"),
    ];
    for (label, declaration) in cases {
        let message = contract_error(verify_defs(&class_gate(&format!(
            r#""additionalProperties": {declaration}"#
        ))));
        assert!(
            message.contains("additionalProperties"),
            "{label} ⇒ 点名该键：{message}"
        );
        assert!(
            message.contains("去掉 `x-class`") && message.contains("不渲染"),
            "{label} ⇒ 出路一侧（变纯数据 + map 不渲染）：{message}"
        );
        assert!(
            message.contains("去掉 `additionalProperties`"),
            "{label} ⇒ 出路另一侧（方法集合）：{message}"
        );
    }
}

/// 边界（AC-8 反向断言：只治一侧）：无 `x-class` 的 `additionalProperties` 字典 ⇒ `Ok`；
/// 同形态再带 `properties` ⇒ 仍 `Ok`（本期不改它的静默）。
#[test]
fn verify_defs良性对照只治x_class一侧() {
    let dictionary =
        defs_value(r#"{"Dict": {"type": "object", "additionalProperties": {"type": "string"}}}"#);
    assert_eq!(
        verify_defs(&dictionary),
        Ok(()),
        "无 x-class 的字典仍被写闸放行"
    );

    let with_fields = defs_value(
        r#"{"Dict": {"type": "object", "additionalProperties": {"type": "boolean"}, "properties": {"size": {"type": "integer"}}}}"#,
    );
    assert_eq!(
        verify_defs(&with_fields),
        Ok(()),
        "带 properties 的字典仍放行"
    );
}

/// 边界（判序固定，判序即错误面）：`additionalProperties` + `required` + 非函数属性并存
/// ⇒ 报 `additionalProperties`；`type` 缺失 + 其余并存 ⇒ 报 `type`；有 `type` 时
/// `required` + 非函数属性 ⇒ 报 `required`；只剩非函数属性 ⇒ 报 `properties`——四条文案
/// 互不相等（逐条可辨）。
#[test]
fn verify_defs骨架四判据判序固定() {
    let dictionary = contract_error(verify_defs(&gate(
        r#"{"type": "object", "x-class": true, "additionalProperties": false, "required": ["a"], "properties": {"a": {"type": "integer"}}}"#,
    )));
    assert!(
        dictionary.contains("additionalProperties"),
        "首选判据是 additionalProperties：{dictionary}"
    );

    let skeleton = contract_error(verify_defs(&gate(
        r#"{"x-class": true, "required": ["a"], "properties": {"a": {"type": "integer"}}}"#,
    )));
    assert!(
        skeleton.contains("$defs.Gate.type"),
        "次选判据是 type：{skeleton}"
    );

    let required = contract_error(verify_defs(&gate(
        r#"{"type": "object", "x-class": true, "required": ["a"], "properties": {"a": {"type": "integer"}}}"#,
    )));
    assert!(
        required.contains("required") && !required.contains("$defs.Gate.type"),
        "第三判据是 required：{required}"
    );

    let properties = contract_error(verify_defs(&gate(
        r#"{"type": "object", "x-class": true, "properties": {"a": {"type": "integer"}}}"#,
    )));
    assert!(
        properties.contains("properties.a"),
        "末位判据是 properties：{properties}"
    );

    let messages = [
        dictionary.clone(),
        skeleton.clone(),
        required.clone(),
        properties.clone(),
    ];
    for (index, left) in messages.iter().enumerate() {
        for right in messages.iter().skip(index + 1) {
            assert_ne!(left, right, "四判据文案互不相等（逐条可辨）");
        }
    }
}

/// 回归（AC-5 既有面）：新判据不遮蔽既有五条 `$defs` 规则——嵌套 `$defs` 与缺 `type`
/// 并存 ⇒ 先报嵌套（外层规则优先）。
#[test]
fn verify_defs新判据不遮蔽既有规则() {
    let nested =
        defs_value(r#"{"Gate": {"x-class": true, "$defs": {"Inner": {"type": "object"}}}}"#);
    let message = contract_error(verify_defs(&nested));
    assert!(
        message.contains("嵌套"),
        "外层规则先命中（缺 type 不抢先）：{message}"
    );
    assert!(
        !message.contains("$defs.Gate.type"),
        "MUST NOT 遮蔽既有规则：{message}"
    );
}

/// 边界（str 边界 + 超长）：`type` 含 `\n` / `\0` / emoji ⇒ 各 `Err`（零 panic 的精确
/// 拒绝）；键名 emoji 与 `x-static` 成员名含空格 / 连字符 ⇒ 本文件不判（名字族归
/// `contract::verify`）故 `Ok`；落点单趟仍排在 `degraded` 词表扫描之前。
#[test]
fn verify_defs特殊字符与超长值零误判() {
    for actual in [r#""object\n""#, r#""object\u0000""#, r#""object🎯""#] {
        let defs = format!(r#"{{"Gate": {{"type": {actual}, "x-class": true}}}}"#);
        let message = contract_error(verify_defs(&defs_value(&defs)));
        assert!(
            message.contains("$defs.Gate.type"),
            "特殊字符的 type 仍精确拒绝：{message}"
        );
    }

    let exotic_name =
        defs_value(r#"{"🎯": {"type": "object", "x-class": true, "properties": {}}}"#);
    assert_eq!(
        verify_defs(&exotic_name),
        Ok(()),
        "键名形态归名字闸，本文件不判"
    );

    let exotic_members =
        class_gate(r#""x-static": {"带 空格": {"x-function": {}}, "open-it": {"x-function": {}}}"#);
    assert_eq!(
        verify_defs(&exotic_members),
        Ok(()),
        "成员名形态归名字闸，本文件不判"
    );

    let ordered = defs_value(
        r#"{"Plain": {"type": "object", "x-static": {"open": {"x-function": {}}}, "x-common": {"degraded": "bogus"}}}"#,
    );
    let message = contract_error(verify_defs(&ordered));
    assert!(
        message.contains("落点非法"),
        "落点单趟排在 degraded 词表之前：{message}"
    );
    assert!(!message.contains("降级词表"), "降级词表仍最后：{message}");
}

/// 正向（AC-6 正例面 / 构造器形态）：`params` 缺省 / `[]` / 单参 / 100 参、`async: false`、
/// 外层另加键（`note` / `description`）⇒ 全 `Ok`（外层不设未知键闸）。
#[test]
fn verify_class_members构造器形态正例() {
    for rest in [
        r#""x-constructor": {"x-function": {}}"#,
        r#""x-constructor": {"x-function": {"params": []}}"#,
        r#""x-constructor": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}]}}"#,
        r#""x-constructor": {"x-function": {"async": false}}"#,
        r#""x-constructor": {"x-function": {}, "note": "注", "description": "注"}"#,
    ] {
        assert_eq!(
            verify_defs(&class_gate(rest)),
            Ok(()),
            "构造器合法形态应过：{rest}"
        );
    }

    let params: Vec<Value> = (0..100)
        .map(|index| json!({"name": format!("p{index}"), "schema": {"type": "string"}}))
        .collect();
    let wide = json!({"$defs": {"Gate": {
        "type": "object",
        "x-class": true,
        "x-constructor": {"x-function": {"params": params}},
    }}});
    assert_eq!(
        verify_defs(&wide),
        Ok(()),
        "100 参构造器仍合法（list 超大边界）"
    );
}

/// 异常（AC-6 / N-3）：构造器形态负例——值为数组 / 字符串 / `null` / `{}`（缺
/// `x-function`）/ `x-function` 为标量 ⇒ 各 `Err` 点名 `$defs.Gate.x-constructor`；
/// 带 `returns` ⇒ `Err` 点名 `returns` 与出路；`async: true` ⇒ `Err`（构造器是值而非
/// future）。
#[test]
fn verify_class_members构造器形态负例() {
    for declaration in ["[]", r#""oops""#, "null", "{}", r#"{"x-function": 42}"#] {
        let message = contract_error(verify_defs(&class_gate(&format!(
            r#""x-constructor": {declaration}"#
        ))));
        assert!(
            message.contains("$defs.Gate.x-constructor"),
            "点名键位（{declaration}）：{message}"
        );
    }

    let returns = contract_error(verify_defs(&class_gate(
        r#""x-constructor": {"x-function": {"returns": {"type": "string"}}}"#,
    )));
    assert!(returns.contains("returns"), "点名禁项 returns：{returns}");
    assert!(
        returns.contains("别名构造") && returns.contains("x-static"),
        "出路指向 x-static：{returns}"
    );

    let asynchronous = contract_error(verify_defs(&class_gate(
        r#""x-constructor": {"x-function": {"async": true}}"#,
    )));
    assert!(
        asynchronous.contains("async") && asynchronous.contains("future"),
        "点名禁项 async: true 与成因：{asynchronous}"
    );
}

/// 异常（AC-6 静态成员形态）：`x-static` 值为数组 / 字符串 / `null` ⇒ 各 `Err`；成员缺
/// `x-function` / `x-function` 为标量 ⇒ `Err`（缺包裹即静默丢）；100 个合法成员 ⇒ `Ok`。
#[test]
fn verify_class_members静态成员形态负例() {
    for declaration in ["[]", r#""oops""#, "null"] {
        let message = contract_error(verify_defs(&class_gate(&format!(
            r#""x-static": {declaration}"#
        ))));
        assert!(
            message.contains("$defs.Gate.x-static"),
            "点名表位（{declaration}）：{message}"
        );
    }
    for declaration in [r#"{"open": {}}"#, r#"{"open": {"x-function": 42}}"#] {
        let message = contract_error(verify_defs(&class_gate(&format!(
            r#""x-static": {declaration}"#
        ))));
        assert!(
            message.contains("$defs.Gate.x-static.open"),
            "点名成员位（{declaration}）：{message}"
        );
        assert!(message.contains("x-function"), "点名缺包裹：{message}");
    }

    let members: Map<String, Value> = (0..100)
        .map(|index| (format!("m{index}"), json!({"x-function": {}})))
        .collect();
    let many = json!({"$defs": {"Gate": {
        "type": "object",
        "x-class": true,
        "x-static": Value::Object(members),
    }}});
    assert_eq!(
        verify_defs(&many),
        Ok(()),
        "100 个合法静态成员仍过（dict 超大边界）"
    );
}

/// 边界（Optional 缺省面）：`x-static: {}`（零成员）⇒ `Ok`；两键都缺省 ⇒ `Ok`；成员内层
/// 自由形状（`params` / `returns` / `async` / `description` 任意组合）⇒ `Ok`。
#[test]
fn verify_class_members缺省与空面() {
    for rest in [
        r#""x-static": {}"#,
        "",
        r#""x-static": {"open": {"x-function": {"params": [], "returns": {"type": "string"}, "async": true, "description": "开"}}}"#,
    ] {
        assert_eq!(
            verify_defs(&class_gate(rest)),
            Ok(()),
            "缺省 / 空面应过：{rest}"
        );
    }
}

/// 正向（AC-4 正例面）：合法位——类条目顶层单独 `x-constructor` / 单独 `x-static` /
/// 两键并存 / 带方法 + ctor + static 的完整正例 ⇒ 全 `Ok`。
#[test]
fn verify_class_key_positions合法位() {
    for rest in [
        r#""x-constructor": {"x-function": {}}"#,
        r#""x-static": {"open": {"x-function": {}}}"#,
        r#""x-constructor": {"x-function": {}}, "x-static": {"open": {"x-function": {}}}"#,
        r##""properties": {"run": {"x-function": {}}}, "x-constructor": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}]}}, "x-static": {"open": {"x-function": {"returns": {"$ref": "#/$defs/Gate"}}}}"##,
    ] {
        assert_eq!(
            verify_defs(&class_gate(rest)),
            Ok(()),
            "类条目顶层的两键合法：{rest}"
        );
    }
}

/// 异常（AC-4 落点）：非类条目四形态——纯 `object` / 封闭枚举（`oneOf` 全 `const`）/ 标量 /
/// 纯 `$ref` alias 上写 `x-constructor` ⇒ 各 `Err` 点名 `$defs.<T>.x-constructor` 与出路；
/// `x-static` 写进封闭枚举 / 纯 alias 两臂同断言。
#[test]
fn verify_class_key_positions非类条目四形态() {
    let constructor = r#""x-constructor": {"x-function": {}}"#;
    let statics = r#""x-static": {"open": {"x-function": {}}}"#;
    let cases = [
        (
            "纯 object",
            format!(r#"{{"Plain": {{"type": "object", {constructor}}}}}"#),
        ),
        (
            "封闭枚举",
            format!(
                r#"{{"Plain": {{"oneOf": [{{"const": "a"}}, {{"const": "b"}}], {constructor}}}}}"#
            ),
        ),
        (
            "标量",
            format!(r#"{{"Plain": {{"type": "string", {constructor}}}}}"#),
        ),
        (
            "纯 $ref alias",
            format!(
                r##"{{"Other": {{"type": "object"}}, "Plain": {{"$ref": "#/$defs/Other", {constructor}}}}}"##
            ),
        ),
    ];
    for (label, defs) in cases {
        let message = contract_error(verify_defs(&defs_value(&defs)));
        assert!(
            message.contains("$defs.Plain.x-constructor"),
            "{label} ⇒ 点名键位与条目：{message}"
        );
        assert!(
            message.contains("只有 `x-class` 条目可声明构造入口与静态方法"),
            "{label} ⇒ 含出路：{message}"
        );
    }

    for (label, defs) in [
        (
            "封闭枚举",
            format!(r#"{{"Plain": {{"oneOf": [{{"const": "a"}}], {statics}}}}}"#),
        ),
        (
            "纯 $ref alias",
            format!(
                r##"{{"Other": {{"type": "object"}}, "Plain": {{"$ref": "#/$defs/Other", {statics}}}}}"##
            ),
        ),
    ] {
        let message = contract_error(verify_defs(&defs_value(&defs)));
        assert!(
            message.contains("$defs.Plain.x-static"),
            "{label} 的 x-static ⇒ 点名键位：{message}"
        );
    }
}

/// 异常（AC-4 深层与模块位）：`properties.<k>.x-function` 内写新键 / `$defs` 条目内更深层
/// / `items` 内 / `x-static.open.x-static`（嵌套两键）⇒ 各 `Err` 点名 JSON pointer；
/// 模块 `properties.<k>` 直接写新键 ⇒ 既有 ⑤ 形态白名单 `Err`。
#[test]
fn verify_class_key_positions深层与模块位() {
    let inside_function = json!({
        "$defs": {},
        "properties": {"k": {"x-function": {"x-constructor": {"x-function": {}}}}},
    });
    let message = contract_error(verify_defs(&inside_function));
    assert!(
        message.contains("properties.k.x-function.x-constructor"),
        "函数内层写新键 ⇒ 点名 pointer：{message}"
    );

    let deeper = class_gate(
        r#""properties": {"m": {"x-function": {}, "x-static": {"open": {"x-function": {}}}}}"#,
    );
    let message = contract_error(verify_defs(&deeper));
    assert!(
        message.contains("$defs.Gate.properties.m.x-static"),
        "条目内更深层 ⇒ 点名 pointer：{message}"
    );

    let in_items = class_gate(
        r#""properties": {"m": {"x-function": {"params": [{"name": "xs", "schema": {"type": "array", "items": {"x-static": {"open": {"x-function": {}}}}}}]}}}"#,
    );
    let message = contract_error(verify_defs(&in_items));
    assert!(
        message.contains("items.x-static") && message.contains("params[0]"),
        "items 内 ⇒ 点名 pointer（含数组段）：{message}"
    );

    let nested = class_gate(
        r#""x-static": {"open": {"x-function": {}, "x-static": {"close": {"x-function": {}}}}}"#,
    );
    let message = contract_error(verify_defs(&nested));
    assert!(
        message.contains("$defs.Gate.x-static.open.x-static"),
        "嵌套两键 ⇒ 点名 pointer：{message}"
    );

    let module_level = json!({"properties": {"k": {"x-constructor": {}}}});
    let message = contract_error(verify_shape(&module_level));
    assert!(
        message.contains("properties.k") && message.contains("形态非法"),
        "模块 properties 位的形态仍由既有 ⑤ 拒：{message}"
    );
}

/// 边界（递归体退化输入）：`{}`（无 `$defs`）、`$defs` 值为标量 / 数组、`$defs: {}` 空表、
/// `$defs` 键超长 ⇒ 零 panic（首三者按既有规则 `Err`，空表与超长键 `Ok`）；
/// `x-class: "true"`（字符串）条目的顶层新键按非法落点 `Err`。
#[test]
fn verify_class_key_positions递归体退化输入() {
    let no_defs = contract_error(verify_defs(&json!({})));
    assert!(
        no_defs.contains("$defs"),
        "缺 $defs 仍报既有文案：{no_defs}"
    );

    let scalar_defs = contract_error(verify_defs(&json!({"$defs": "x"})));
    assert!(
        scalar_defs.contains("$defs"),
        "`$defs` 非对象：{scalar_defs}"
    );

    for definition in [json!([]), json!(42)] {
        let message = contract_error(verify_defs(&json!({"$defs": {"A": definition}})));
        assert!(message.contains("A"), "条目值非对象 ⇒ 点名键：{message}");
    }

    assert_eq!(
        verify_defs(&json!({"$defs": {}})),
        Ok(()),
        "空 $defs 表 ⇒ Ok"
    );

    let long_name = "a".repeat(1200);
    let long_key = json!({"$defs": {long_name.clone(): {"type": "object", "x-class": true, "properties": {}}}});
    assert_eq!(verify_defs(&long_key), Ok(()), "超长键名零 panic 且不越界");

    let string_flag = defs_value(
        r#"{"Gate": {"type": "object", "x-class": "true", "x-constructor": {"x-function": {}}}}"#,
    );
    let message = contract_error(verify_defs(&string_flag));
    assert!(
        message.contains("$defs.Gate.x-constructor"),
        "字符串 x-class 不算类条目 ⇒ 顶层新键是非法落点：{message}"
    );
}

/// 异常（AC-4 顶层键）：顶层新 `x-` 键 `x-note` / `x-constructor` / `x-static` ⇒ 各 `Err`，
/// 文案含封闭六项与 `x-` 面五项逐名（`x-function` / `x-class` / `x-common` /
/// `x-constructor` / `x-static`）。
#[test]
fn verify_top_level_keys五面逐名() {
    for unknown in ["x-note", "x-constructor", "x-static"] {
        let mut value = json!({"$id": FILE, "title": FQN, "$defs": {}, "properties": {}});
        value[unknown] = json!({});
        let message = contract_error(verify_top_level_keys(&value));
        assert!(message.contains(unknown), "点名新键 `{unknown}`：{message}");
        for needle in [
            "x-function",
            "x-class",
            "x-common",
            "x-constructor",
            "x-static",
        ] {
            assert!(
                message.contains(needle),
                "`x-` 面逐名在场（缺 `{needle}`）：{message}"
            );
        }
        for top in ["$schema", "$comment", "$defs", "properties"] {
            assert!(
                message.contains(top),
                "封闭六项逐名在场（缺 `{top}`）：{message}"
            );
        }
    }
}

/// 边界（闸 ⑧ 形态）：顶层值非对象（`[]` / `"x"` / `42` / `null`）⇒ `Err`「顶层必须是
/// JSON 对象」；`$defs` / `properties` 非对象 ⇒ `Err`。
#[test]
fn verify_top_level_keys形态边界() {
    for illegal in [json!([1, 2]), json!("x"), json!(42), Value::Null] {
        let message = contract_error(verify_top_level_keys(&illegal));
        assert!(
            message.contains("顶层必须是 JSON 对象"),
            "非对象顶层 ⇒ Err：{message}"
        );
    }
    for key in ["$defs", "properties"] {
        let mut value = json!({"$id": FILE, "title": FQN, "$defs": {}, "properties": {}});
        value[key] = json!([1, 2]);
        let message = contract_error(verify_top_level_keys(&value));
        assert!(
            message.contains(key) && message.contains("必须是对象"),
            "`{key}` 非对象 ⇒ 点名其形态：{message}"
        );
    }
}

/// 正向（AC-1 投影面）：合法「类 + ctor + static」整链经写闸 ⇒ `Ok`，且 `Contract` 投影的
/// `ops` / `types` 键集 / `reexports` 与补两键前逐字段相等（新键不进模块 op 投影）。
#[test]
fn validate_schema合法类整链() {
    let defs_with_keys = concat!(
        r#""Store": {"type": "object", "x-class": true, "properties": {"count": {"x-function": {}}}, "#,
        r#""x-constructor": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}]}}, "#,
        r##""x-static": {"open": {"x-function": {"params": [{"name": "path", "schema": {"type": "string"}}], "returns": {"$ref": "#/$defs/Store"}}}}}"##,
    );
    let defs_without_keys = "\"Store\": {\"type\": \"object\", \"x-class\": true, \"properties\": {\"count\": {\"x-function\": {}}}}";
    let with_keys = validate_schema(
        FQN,
        &schema_text(defs_with_keys, r##""Store": {"$ref": "#/$defs/Store"}"##),
    )
    .expect("类 + ctor + static 整链应合法");
    let without_keys = validate_schema(
        FQN,
        &schema_text(defs_without_keys, r##""Store": {"$ref": "#/$defs/Store"}"##),
    )
    .expect("补两键前的同形契约应合法");

    assert_eq!(with_keys.ops, without_keys.ops, "新键不进模块 op 投影");
    let mut with_types: Vec<&String> = with_keys.types.keys().collect();
    let mut without_types: Vec<&String> = without_keys.types.keys().collect();
    with_types.sort();
    without_types.sort();
    assert_eq!(with_types, without_types, "类型面键集逐字段相等");
    assert_eq!(with_keys.reexports, without_keys.reexports, "透传面零变化");
}

/// 异常（AC-5 跨函数分工）：缺骨架经写闸 ⇒ `Err(ApiError::Contract)`，文案与
/// `verify_defs` 单点同源；源码锚：`validate_schema` 体内闸调用序 ①–⑧ + `contract::verify`
/// + `verify_defs`，闸数仍九（零第十道闸）。
#[test]
fn validate_schema缺骨架写闸同源() {
    let text = schema_text(
        r#""Gate": {"x-class": true}"#,
        r##""Gate": {"$ref": "#/$defs/Gate"}"##,
    );
    let message = parse_error(validate_schema(FQN, &text));
    let value: Value = serde_json::from_str(&text).expect("夹具必须是合法 JSON");
    assert_eq!(
        message,
        contract_error(verify_defs(&value)),
        "写闸与 `verify_defs` 单点同源（零第二份文案）"
    );

    let source = include_str!("schema.rs");
    let body_start = source
        .find("pub fn validate_schema(")
        .expect("源码内应有 `validate_schema`");
    let body = &source[body_start..];
    let mut cursor = 0usize;
    for call in [
        "verify_identity",
        "verify_keyspace",
        "verify_shape",
        "verify_mirror",
        "verify_ref_prefix",
        "verify_top_level_keys",
        "Contract::from_schema",
        "contract::verify",
        "verify_defs",
    ] {
        let found = body[cursor..]
            .find(call)
            .unwrap_or_else(|| panic!("`validate_schema` 体内应有闸调用 `{call}`"));
        cursor += found + call.len();
    }
    assert_eq!(
        source.matches("fn validate_schema").count(),
        1,
        "写闸与装载期共用同一入口（零孪生）"
    );
}

/// 边界（空 / 规模 / 分工）：空壳 ⇒ `Ok`；`$defs` 含 100 个类条目（每类带
/// `"type": "object"` 且 `properties` 有镜像 `$ref`）⇒ `Ok` 且不 panic；`x-class` 条目带
/// ctor 但 `params[].name` 非法 ⇒ 名字闸（`contract::verify`）先报，`verify_defs` 不再报。
#[test]
fn validate_schema空壳规模与分工() {
    assert!(
        validate_schema(FQN, &schema_text("", "")).is_ok(),
        "空壳合法"
    );

    let definitions: Vec<String> = (0..100)
        .map(|index| {
            format!(r#""T{index}": {{"type": "object", "x-class": true, "properties": {{}}}}"#)
        })
        .collect();
    let mirrors: Vec<String> = (0..100)
        .map(|index| format!(r##""T{index}": {{"$ref": "#/$defs/T{index}"}}"##))
        .collect();
    assert!(
        validate_schema(
            FQN,
            &schema_text(&definitions.join(", "), &mirrors.join(", "))
        )
        .is_ok(),
        "100 个类条目（超大文档）⇒ Ok 且不 panic"
    );

    let text = schema_text(
        r#""Gate": {"type": "object", "x-class": true, "x-constructor": {"x-function": {"params": [{"name": "bad-name"}]}}}"#,
        r##""Gate": {"$ref": "#/$defs/Gate"}"##,
    );
    let message = parse_error(validate_schema(FQN, &text));
    assert!(
        message.contains("参数名") && message.contains("bad-name"),
        "名字闸先报：{message}"
    );
    assert!(
        !message.contains("$defs.Gate.x-constructor"),
        "形态正确时 `verify_defs` 不再报：{message}"
    );
}

/// 回归（源码锚 D-1 / 闸 ⑨ doc 同批）：两条别名常量经 `super::contract::X_*` 引用（协议词汇
/// 单点持有），`validate_schema` 的闸 ⑨ doc 描述「五条 `$defs` 规则 + 骨架四判据 + 两键形态
/// 与落点」。
#[test]
fn schema闸九别名与doc随动() {
    let source = include_str!("schema.rs");
    assert_eq!(
        source
            .matches("const X_CONSTRUCTOR: &str = super::contract::X_CONSTRUCTOR;")
            .count(),
        1,
        "构造入口别名引用 `super::contract`（零散落字面量）"
    );
    assert_eq!(
        source
            .matches("const X_STATIC: &str = super::contract::X_STATIC;")
            .count(),
        1,
        "静态方法表别名引用 `super::contract`"
    );
    for needle in ["骨架四判据", "两键形态", "落点"] {
        assert!(
            source.contains(needle),
            "闸 ⑨ 的 doc 含 `{needle}`（与判据同批）"
        );
    }
}

// ---------------------------------------------------------------------------
// 封闭词汇的三判：顶层 `enum` / 裸 `const` / 不可 raw 的键名（AC-4）
// ---------------------------------------------------------------------------

/// 正向（AC-4）：`oneOf` / `anyOf` 全 `const` 形态放行（零 `Err`）；`oneOf` 分支内的
/// `const` 正常通过。
#[test]
fn verify_defs的oneof与const分支放行() {
    for key in ["oneOf", "anyOf"] {
        let schema = json!({"$defs": {"Mode": {key: [
            {"const": "a", "description": "取值 a"},
            {"const": "b", "description": "取值 b"}
        ]}}});
        assert!(
            verify_defs(&schema).is_ok(),
            "`{key}` 全 `const` 分支是唯一合法形态：{key}"
        );
    }

    for constant in [json!("text"), json!(1), json!(true), json!(null)] {
        let schema = json!({"$defs": {"Mode": {"oneOf": [{"const": constant}]}}});
        assert!(
            verify_defs(&schema).is_ok(),
            "`oneOf` 分支内的标量 `const` 放行：{constant}"
        );
    }
}

/// 异常（AC-4）：条目顶层 `enum` 数组 ⇒ `Err`，文案点名 `$defs.Plain.enum` 且含可执行出路
/// （完整 `oneOf` 样例片段，MUST NOT 只写「非法」）。
#[test]
fn verify_defs拒收顶层enum数组() {
    let schema = json!({"$defs": {"Plain": {"enum": ["a", "b"]}}});
    let error = contract_error(verify_defs(&schema));
    assert!(
        error.contains("$defs.Plain.enum"),
        "文案点名 pointer：{error}"
    );
    assert!(
        error.contains("\"oneOf\"") && error.contains("\"const\""),
        "文案给出可执行出路（`oneOf` + `const` 样例片段）：{error}"
    );
    assert!(
        error.contains("description"),
        "出路含逐值语义的落点（`description`）：{error}"
    );
}

/// 边界（AC-4）：`enum: []`（空数组）同样拒收（只认键在场）；`$ref` 与 `enum` 并列时既有
/// 「透传条目不得带结构关键字」先命中（判序单点归因，新三判不遮蔽既有规则）。
#[test]
fn verify_defs的顶层enum判序与空数组() {
    let empty = json!({"$defs": {"Plain": {"enum": []}}});
    let error = contract_error(verify_defs(&empty));
    assert!(
        error.contains("$defs.Plain.enum"),
        "空数组同样拒收（判据只认键在场）：{error}"
    );

    let tied = json!({"$defs": {"AgentInput": {
        "$ref": "../Frontend/Agent.schema.json#/$defs/AgentInput",
        "enum": ["a"]
    }}});
    let tied_error = contract_error(verify_defs(&tied));
    assert!(
        tied_error.contains("结构关键字"),
        "既有 `$ref` 纯度规则先命中（判序在外层）：{tied_error}"
    );
    assert!(
        !tied_error.contains("退役写法"),
        "新三判不遮蔽既有规则：{tied_error}"
    );
}

/// 异常（AC-4）：条目顶层裸 `const` ⇒ `Err` 并点名 `$defs.T.const`，文案写明「`const` 只在
/// `oneOf` / `anyOf` 分支内且 MUST 是标量」。
#[test]
fn verify_defs拒收顶层裸const() {
    let schema = json!({"$defs": {"T": {"const": "a"}}});
    let error = contract_error(verify_defs(&schema));
    assert!(error.contains("$defs.T.const"), "文案点名 pointer：{error}");
    assert!(
        error.contains("oneOf") && error.contains("标量"),
        "文案写明落点与标量约束：{error}"
    );
}

/// 边界（AC-4）：`const: 1` / `const: false` / `const: null` 三非字符串值同样拒收（判据只认
/// 键在场）；字段位 `properties.<f>.const` 当前不判（Q-8 已知窗口——放行并登记为窗口）。
#[test]
fn verify_defs的顶层const只看键在场() {
    for constant in [json!(1), json!(false), json!(null)] {
        let schema = json!({"$defs": {"T": {"const": constant}}});
        let error = contract_error(verify_defs(&schema));
        assert!(
            error.contains("$defs.T.const"),
            "非字符串顶层 `const` 同样拒收（{constant}）：{error}"
        );
    }

    let field_position = json!({"$defs": {"T": {
        "type": "object",
        "properties": {"f": {"const": "a"}}
    }}});
    assert!(
        verify_defs(&field_position).is_ok(),
        "字段位 `const` 不在本判据射程（Q-8 已知窗口，不虚构判据）"
    );
}

/// 正向（AC-4 / AC-5）：可 raw 的关键字键（`type` / `in` / `abstract` / `match` / `move`）
/// 放行——`r#` 形态可达。
#[test]
fn verify_defs放行可raw的关键字键() {
    let schema = json!({"$defs": {"T": {
        "type": "object",
        "properties": {
            "type": {"type": "string"},
            "in": {"type": "integer"},
            "abstract": {"type": "string"},
            "match": {"type": "boolean"},
            "move": {"type": "boolean"}
        }
    }}});
    assert!(
        verify_defs(&schema).is_ok(),
        "五个可 raw 的关键字键全部放行（`r#` 形态可达）"
    );
}

/// 异常（AC-4 / AC-5）：`properties` 键 ∈ `{self, Self, super, crate}` 四词各 `Err`，文案点名
/// `$defs.<T>.properties.<f>` 并给出路「改名」；`self` 与 `Self` 大小写两值分别可判。
#[test]
fn verify_defs拒收不可raw的键名() {
    for word in ["self", "Self", "super", "crate"] {
        let schema = json!({"$defs": {"T": {
            "type": "object",
            "properties": {word: {"type": "string"}}
        }}});
        let error = contract_error(verify_defs(&schema));
        assert!(
            error.contains(&format!("$defs.T.properties.{word}")),
            "文案点名字段位（`{word}`）：{error}"
        );
        assert!(
            error.contains("改名"),
            "出路是改名（`r#{word}` 是非法 Rust）：{error}"
        );
    }
    assert_ne!(
        contract_error(verify_defs(&json!({"$defs": {"T": {
            "type": "object",
            "properties": {"self": {"type": "string"}}
        }}}))),
        contract_error(verify_defs(&json!({"$defs": {"T": {
            "type": "object",
            "properties": {"Self": {"type": "string"}}
        }}}))),
        "`self` 与 `Self` 大小写两值分别可判（零归一）"
    );
}

/// 边界（AC-4）：四词出现在第二层（`properties.a.properties.self`）不判（只判第一层
/// `properties` 的键）；四词出现在 `required` 数组里不判（判据只认 `properties` 键）。
#[test]
fn verify_defs的四词只在第一层生效() {
    let nested = json!({"$defs": {"T": {
        "type": "object",
        "properties": {"a": {
            "type": "object",
            "properties": {"self": {"type": "string"}}
        }}
    }}});
    assert!(
        verify_defs(&nested).is_ok(),
        "第二层 `properties.self` 不在射程（只判第一层键）"
    );

    let in_required = json!({"$defs": {"T": {
        "type": "object",
        "properties": {"own": {"type": "string"}},
        "required": ["self"]
    }}});
    assert!(
        verify_defs(&in_required).is_ok(),
        "`required` 里的四词不判（判据只认 `properties` 键）"
    );
}

/// 异常（AC-4 判序）：同时违反 `$ref` 纯度与顶层 `enum` ⇒ 文案归因 `$ref` 结构关键字；
/// `x-class` 带属性与顶层 `enum` 同时在场 ⇒ 归因 `x-class` 骨架规则（新三判排在既有规则
/// 之后，零改判）。
#[test]
fn verify_defs的新三判排在既有规则之后() {
    let tied_ref = json!({"$defs": {"Plain": {
        "$ref": "#/$defs/Other",
        "enum": ["a"]
    }}});
    let ref_error = contract_error(verify_defs(&tied_ref));
    assert!(
        ref_error.contains("结构关键字"),
        "`$ref` 结构关键字先命中：{ref_error}"
    );

    let tied_class = json!({"$defs": {"Plain": {
        "type": "object",
        "x-class": true,
        "properties": {"count": {"type": "integer"}},
        "enum": ["a"]
    }}});
    let class_error = contract_error(verify_defs(&tied_class));
    assert!(
        class_error.contains("x-class"),
        "`x-class` 骨架规则先命中：{class_error}"
    );
    assert!(
        !class_error.contains("退役写法"),
        "新三判不遮蔽既有的 `x-class` 判据：{class_error}"
    );
}

/// 回归（AC-4）：写闸只有一份校验——`validate_schema`（同一实现）对顶层 `enum` / 裸 `const`
/// / 不可 raw 键名三形态同样 `Err`（零第二份判据）。
#[test]
fn validate_schema与verify_defs同源() {
    let cases = [
        (
            r#""Plain": {"enum": ["a", "b"]}"#,
            "Plain",
            "$defs.Plain.enum",
        ),
        (r#""T": {"const": "a"}"#, "T", "$defs.T.const"),
        (
            r#""T": {"type": "object", "properties": {"self": {"type": "string"}}}"#,
            "T",
            "$defs.T.properties.self",
        ),
    ];
    for (defs, name, pointer) in cases {
        // 镜像条目补齐（闸 ⑥ 先过），使本用例真正落到封闭词汇三判
        let properties = format!("\"{name}\": {{\"$ref\": \"#/$defs/{name}\"}}");
        let text = schema_text(defs, &properties);
        let message = parse_error(validate_schema(FQN, &text));
        assert!(
            message.contains(pointer),
            "写闸与装载期同源（同一实现报同一 pointer `{pointer}`）：{message}"
        );
    }
}
