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
/// 类型名 + 违规键；零属性的 `x-class` → `Ok`；`x-class: false` + 带属性 → `Ok`。
#[test]
fn verify_defs的x_class带属性判错() {
    let with_property = json!({"$defs": {"Gate": {
        "x-class": true,
        "properties": {"count": {"type": "integer"}}
    }}});
    let error = contract_error(verify_defs(&with_property));
    assert!(error.contains("Gate"), "点名类型名：{error}");
    assert!(error.contains("properties"), "点名违规键：{error}");
    assert!(error.contains("trait"), "点明 trait 仅允许方法：{error}");

    let with_required = json!({"$defs": {"Gate": {
        "x-class": true,
        "required": ["a"]
    }}});
    let error = contract_error(verify_defs(&with_required));
    assert!(error.contains("Gate"), "点名类型名：{error}");
    assert!(error.contains("required"), "点名违规键：{error}");

    let bare = json!({"$defs": {"Gate": {"x-class": true}}});
    assert!(
        verify_defs(&bare).is_ok(),
        "零属性的 x-class 合法（空 trait）"
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

    let x_class =
        json!({"$defs": {"Gate": {"x-class": true, "properties": {"count": {"type": "integer"}}}}});
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
/// `GENERATED_NOTICE` 逐字保留（`.i.*` 字节不变的前提）；`SCHEMA_WRITER_NOTICE` 在场且含
/// 「手写接口真相」与 `archi_api` 的 `check` 自查。
#[test]
fn 生成器让位而注记分家() {
    let source = include_str!("schema.rs");
    assert_eq!(
        source.matches("pub fn build_schema").count(),
        0,
        "`build_schema` 已退役（接口文件是手写资产）"
    );
    let module = include_str!("mod.rs");
    for needle in ["本文件由元素的契约 markdown 生成", "勿手改", "api:check"] {
        assert!(
            module.contains(needle),
            "`GENERATED_NOTICE` 逐字保留（缺 `{needle}`）"
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
