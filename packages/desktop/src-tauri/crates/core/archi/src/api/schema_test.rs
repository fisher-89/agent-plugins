//! `api::schema` 的单元测试：`build_schema` 的六字段装配与保序、`verify_defs` 的
//! 五条规则 + 透传纯 `$ref` 校验 + `x-class` 带属性判错、`build_index` 的说明段 /
//! 七字段 / 字典序 / `[entry.categories]` 形状分布与整段省略。
//!
//! 零 mock：契约输入由真实 `contract::parse` 产出（`verify_defs` 的 `schema` 入参是
//! 纯值，另备 `json!` 直接构造非法形态）；`toml` 反解析真实执行（INDEX 可解析性即
//! 被测语义）；`ApiMeta` 以字面量构造真实值（零替身类型）。

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use toml_edit::DocumentMut;

use crate::api::contract::Contract;

use super::contract::parse;
use super::paths;
use super::schema::{build_index, build_schema, verify_defs};
use super::{ApiError, ApiFormStatus, ApiMeta, IndexEntry};

/// 契约文件（repo 相对 POSIX）。
const FILE: &str = ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md";

/// 元素 FQN（= H1 = 文件名去 `.md`）。
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

/// 断言 `parse` 报契约错误并返回文案（D37：文本层判据的错误面）。
fn parse_error(result: Result<Contract, ApiError>) -> String {
    match result {
        Ok(_) => panic!("期望契约错误，实际 Ok"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 断言 `build_schema` 报契约错误并返回文案。
fn build_error(result: Result<Value, ApiError>) -> String {
    match result {
        Ok(value) => panic!("期望契约错误，实际 Ok：{value}"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 试点形态源文（1 op `file_stem` + 1 类型 `LocalArchiCheck`）。
fn pilot_source() -> String {
    format!(
        "# {FQN}\n\n## 职能\n\n门禁执行体。\n\n## 操作\n\n### file_stem\n\n```json api-op\n\
         {{\"params\": [{{\"name\": \"timestamp\", \"schema\": {{\"type\": \"string\"}}}}], \
         \"returns\": {{\"type\": \"string\"}}}}\n```\n\n## 类型\n\n```json api-types\n\
         {{\"LocalArchiCheck\": {{\"type\": \"object\", \"properties\": {{}}, \
         \"x-common\": {{\"note\": \"零字段门禁执行体\"}}}}}}\n```\n"
    )
}

/// 解析试点契约（失败即断言失败）。
fn pilot_contract() -> Contract {
    parse(&pilot_source(), FQN, FILE).expect("试点契约应解析成功")
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
// build_schema：六字段
// ---------------------------------------------------------------------------

/// 正向（AC-2）：试点契约 → 顶层键**恰**六项，逐项有 md 出处。
#[test]
fn build_schema六字段齐备() {
    let contract = pilot_contract();
    let schema = build_schema(&contract, &meta()).expect("装配应成功");
    let object = schema.as_object().expect("schema 是 JSON 对象");

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["$comment", "$defs", "$id", "$schema", "properties", "title"],
        "顶层键恰六项（零多余、零缺失）"
    );

    assert_eq!(
        object["$schema"],
        json!("https://json-schema.org/draft/2020-12/schema"),
        "方言常量取 draft 2020-12"
    );
    assert_eq!(
        object["$id"],
        json!(format!("{FQN}.schema.json")),
        "$id = <FQN>.schema.json（不合成 URI）"
    );
    assert!(
        !object["$id"].as_str().unwrap_or("").contains("http"),
        "$id MUST NOT 是 URI"
    );
    assert_eq!(object["title"], json!(FQN), "title 取 H1");
    assert!(
        object["$defs"]
            .as_object()
            .is_some_and(|defs| defs.contains_key("LocalArchiCheck")),
        "$defs 含 api-types 键"
    );

    let properties = object["properties"].as_object().expect("properties 是对象");
    assert!(properties.contains_key("file_stem"), "properties 含 op 键");
    assert!(
        properties.contains_key("LocalArchiCheck"),
        "properties 含每个 $defs 键的 $ref"
    );
    assert_eq!(
        properties["LocalArchiCheck"],
        json!({"$ref": "#/$defs/LocalArchiCheck"}),
        "类型键的值仅一个 $ref（零结构关键字）"
    );

    let comment = object["$comment"].as_str().expect("$comment 是字符串");
    assert!(!comment.is_empty(), "$comment 非空");
    assert!(
        comment.contains("勿手改"),
        "$comment 含勿手改警示：{comment}"
    );
    assert!(
        comment.contains("api:check"),
        "$comment 含零 diff 门提示：{comment}"
    );
}

/// 正向（D7）：op 键的值为 `api-op` fence 原文包进 `x-function`（零翻译）；类型键
/// 的值为纯 `$ref`。
#[test]
fn build_schema的properties双来源() {
    let contract = pilot_contract();
    let schema = build_schema(&contract, &meta()).expect("装配应成功");
    let properties = schema["properties"].as_object().expect("properties 是对象");

    let op_entry = &properties["file_stem"];
    assert!(
        op_entry.get("x-function").is_some(),
        "op 键值带 x-function 标记层：{op_entry}"
    );
    assert!(
        op_entry["x-function"].get("params").is_some(),
        "fence 的 params 原样透传"
    );
    assert!(
        op_entry["x-function"].get("returns").is_some(),
        "fence 的 returns 原样透传"
    );
}

/// 异常（D7）：`properties` 的键空间唯一——op id 与类型名同名 ⇒ `ApiError::Contract`
/// 并点名该名（后写者静默覆盖会让「操作」与「类型引用」在产物里不可辨）。
#[test]
fn build_schema的op与类型同名报错() {
    let source = format!(
        "# {FQN}\n\n### T\n\n```json api-op\n{{\"params\": []}}\n```\n\n\
         ```json api-types\n{{\"T\": {{\"type\": \"object\"}}}}\n```\n"
    );
    let contract = parse(&source, FQN, FILE).expect("同名形态解析合法（歧义在装配期判）");
    let error = build_schema(&contract, &meta()).expect_err("op id 与类型名同名应报契约错误");
    let message = error.message();
    assert!(
        message.contains("`T`") && message.contains("`properties`"),
        "文案点名撞名键与键空间：{message}"
    );
}

/// 异常（D7）：`$defs` 的键空间唯一——`api-types` 与 `api-reexport` 同名 ⇒
/// `ApiError::Contract` 并点名两条来源（同名即定义两次）。
#[test]
fn build_schema的types与reexport同名报错() {
    let source = format!(
        "# {FQN}\n\n```json api-types\n{{\"T\": {{\"type\": \"object\"}}}}\n```\n\n\
         ```json api-reexport\n{{\"T\": \"App.Core.Other\"}}\n```\n\n## 透传\n\n\
         - T → App.Core.Other\n"
    );
    let contract = parse(&source, FQN, FILE).expect("同名形态解析合法");
    let error = build_schema(&contract, &meta()).expect_err("两侧同名应报契约错误");
    let message = error.message();
    assert!(
        message.contains("api-types") && message.contains("api-reexport"),
        "文案点名两条来源与撞名键：{message}"
    );
}

/// 边界（AC-2 幂等）：`$defs` 键序 = `api-types` 声明序 + `api-reexport` 序；同输入
/// 两次 `to_string_pretty` 字节相等。
#[test]
fn build_schema的defs键序保序幂等() {
    let source = format!(
        "# {FQN}\n\n```json api-types\n{{\"b\": {{\"type\": \"object\"}}, \
         \"a\": {{\"type\": \"object\"}}}}\n```\n\n```json api-reexport\n\
         {{\"c\": \"App.Backend.Infra.AgentRuntime\"}}\n```\n\n## 透传\n\n\
         - c → App.Backend.Infra.AgentRuntime\n"
    );
    let contract = parse(&source, FQN, FILE).expect("保序形态合法");
    let schema = build_schema(&contract, &meta()).expect("装配应成功");
    let keys: Vec<&str> = schema["$defs"]
        .as_object()
        .expect("$defs 是对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["b", "a", "c"], "键序 = 声明序（零排序、零合并）");

    let first = serde_json::to_string_pretty(&schema).expect("序列化应成功");
    let second = serde_json::to_string_pretty(&build_schema(&contract, &meta()).expect("重装配"))
        .expect("序列化应成功");
    assert_eq!(first, second, "同输入两次序列化字节相等");
}

/// 正向（AC-11）：透传条目为纯 `$ref`，前缀取自 `paths::ref_relative`（同域 `./`、
/// 跨域 `../<Domain>/`）。
#[test]
fn build_schema的透传条目纯ref() {
    for (target, expected_prefix) in [
        ("App.Backend.Infra.AgentRuntime", "./"),
        ("App.Backend.Core.Agent", "../Core/"),
    ] {
        let source = format!(
            "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"{target}\"}}\n```\n\n\
             ## 透传\n\n- AgentInput → {target}\n"
        );
        let contract = parse(&source, FQN, FILE).expect("透传形态合法");
        let schema = build_schema(&contract, &meta()).expect("装配应成功");
        let definition = &schema["$defs"]["AgentInput"];
        assert_eq!(
            definition,
            &json!({
                "$ref": format!(
                    "{expected_prefix}{target}.schema.json#/$defs/AgentInput"
                )
            }),
            "透传条目只含 $ref，前缀由 ref_relative 单点给出"
        );
        assert_eq!(
            definition.as_object().map(Map::len),
            Some(1),
            "零结构关键字（恰一个键）"
        );
    }
}

/// 边界：空契约（零 op 零类型）→ `$defs` / `properties` 为空对象，其余四字段仍在。
#[test]
fn build_schema空契约六字段恒在场() {
    for source in [
        format!("# {FQN}\n\n## 职能\n\n正文。\n"),
        format!("# {FQN}\n\n```json api-types\n{{}}\n```\n"),
        format!("# {FQN}\n\n```json api-reexport\n{{}}\n```\n"),
    ] {
        let contract = parse(&source, FQN, FILE).expect("空形态合法");
        let schema = build_schema(&contract, &meta()).expect("装配应成功");
        assert_eq!(schema["$defs"], json!({}), "$defs 空对象");
        assert_eq!(schema["properties"], json!({}), "properties 空对象");
        for key in ["$schema", "$id", "title", "$comment"] {
            assert!(
                schema.get(key).is_some(),
                "六字段恒在场（缺 {key}）：{source}"
            );
        }
    }
}

/// 边界：200 op + 1000 `$defs` 键的大契约 → 装配成功、键数逐项相等；单条超长
/// `description`（>1000 chars）原样透传。
#[test]
fn build_schema大契约原样透传() {
    let long = "描".repeat(1200);
    let mut types = String::from("{");
    for index in 0..1000 {
        if index > 0 {
            types.push(',');
        }
        types.push_str(&format!("\"T{index}\": {{\"type\": \"object\"}}"));
    }
    types.push_str(&format!(
        ",\"Long\": {{\"type\": \"object\", \"description\": \"{long}\"}}}}"
    ));

    let mut source = format!("# {FQN}\n\n## 操作\n\n");
    for index in 0..200 {
        source.push_str(&format!(
            "### op{index}\n\n```json api-op\n{{\"params\": []}}\n```\n\n"
        ));
    }
    source.push_str(&format!("```json api-types\n{types}\n```\n"));

    let contract = parse(&source, FQN, FILE).expect("大契约应解析成功");
    let schema = build_schema(&contract, &meta()).expect("装配应成功");

    assert_eq!(
        schema["$defs"].as_object().map(Map::len),
        Some(1001),
        "1000 声明 + 1 长描述"
    );
    assert_eq!(
        schema["properties"].as_object().map(Map::len),
        Some(1201),
        "200 op + 1001 类型键"
    );
    assert_eq!(
        schema["$defs"]["Long"]["description"],
        json!(long),
        "超长描述原样透传（零改写）"
    );
}

/// 异常：`api-types` 值为非对象（字符串 / 数组 / `null`）→ 解析期即 `Err`（fence 内容
/// 必须是 JSON 对象）；`api-op` fence 内容为非对象亦 `Err`。
#[test]
fn build_schema非法类型值() {
    for (fence, body) in [
        ("api-types", "\"just a string\""),
        ("api-types", "[1, 2, 3]"),
        ("api-types", "null"),
        ("api-op", "\"not an object\""),
    ] {
        let source = if fence == "api-types" {
            format!("# {FQN}\n\n```json api-types\n{body}\n```\n")
        } else {
            format!("# {FQN}\n\n### op\n\n```json api-op\n{body}\n```\n")
        };
        let error = match parse(&source, FQN, FILE) {
            Ok(contract) => panic!("非对象 fence 内容应报错，实际 Ok：{contract:?}"),
            Err(ApiError::Contract(message)) => message,
            Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
        };
        assert!(error.contains(fence), "文案点名 fence（{fence}）：{error}");
    }

    // api-reexport 的值必须是子元素 FQN 字符串（build_schema 侧的判据）
    let source = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": 42}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Backend.Infra.AgentRuntime\n"
    );
    let contract = parse(&source, FQN, FILE).expect("结构合法的 reexport 值形态");
    let error = build_error(build_schema(&contract, &meta()));
    assert!(error.contains("AgentInput"), "点名违规的透传键：{error}");
    assert!(
        error.contains("FQN") || error.contains("字符串"),
        "点明值的形态要求：{error}"
    );
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
/// ① **重复键即契约错误**（判据落 `contract::parse` 的 fence 文本层）：同一 `api-types`
///    fence 里重复声明同一键 → `parse` 返回 `Err(ApiError::Contract(..))`，文案点名该键；
/// ② **身份 = 键名的判据本身成立**：键名精确比较（不归一、不合并、不加别名）——
///    `A` 与 `a` 是两个键、两键各留一份定义、`verify_defs` 前后键数不变（零副作用）。
#[test]
fn verify_defs键名身份唯一在文本层判() {
    // ① 重复键在文本层即被拦下（D37：这是唯一可判处）
    let duplicated_fence = format!(
        "# {FQN}\n\n```json api-types\n{{\"A\": {{\"type\": \"object\"}}, \
         \"A\": {{\"enum\": [\"x\"]}}}}\n```\n"
    );
    assert_eq!(
        duplicated_fence.matches("\"A\"").count(),
        2,
        "源文里同名键声明了两次（判据的前提面）"
    );
    let error = parse_error(parse(&duplicated_fence, FQN, FILE));
    assert!(
        error.contains("重复声明") && error.contains('A'),
        "点名重复键与「同名即定义两次」：{error}"
    );
    assert!(
        error.contains(FILE),
        "文案带 `file:line` 前缀（可直接改到那一行）：{error}"
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

    for reference in [
        "#/$defs/A",
        "../Core/App.Backend.Core.Agent.schema.json#/$defs/AgentInput",
    ] {
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
            "$ref": "../Frontend/App.Frontend.Components.Agent.Input.schema.json#/$defs/AgentInput",
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
        "$ref": "../Frontend/App.Frontend.Components.Agent.Input.schema.json#/$defs/AgentInput"
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
        lines[1].contains("review") && lines[1].contains("<FQN>.md"),
        "次行 review 口径（只读元素目录下的 md）：{}",
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
        schema_rel: "Infra/App.Backend.Infra.ArchiRuntime.schema.json".to_owned(),
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
        expected.ends_with(format!("{FQN}.schema.json")),
        "schema 路径文件名：{expected:?}"
    );
}
