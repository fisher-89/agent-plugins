//! `api::contract` 的单元测试：`parse` 的五类位置语义（H1 锚点 / 散文区 /
//! `### <op-id>` 段 / 三类 fence 的唯一性）、双向 gate 三条（op 集合 / 类型键 ⊆
//! `$defs` / 透传键集 ≡ 散文列表）与构造器封闭集判据。
//!
//! 零 mock：`parse` / `verify` 接 `source: &str`，夹具以字符串直传（零落盘）；
//! `serde_json` 真实解析 fence 内容（JSON 语法错面即被测语义）；`verify` 的
//! `schema` 入参由真实 `build_schema` 产出，另备手写 `json!` 反例注入缺定义。

use serde_json::{json, Value};

use super::contract::{is_constructor, parse, verify, Contract, ContractOp};
use super::schema::build_schema;
use super::{meta, ApiError};

/// 契约文件（repo 相对 POSIX；错误文案的 `file:` 前缀来源）。
const FILE: &str = ".dev-team/architecture/api/Demo/App.Demo.Item.md";

/// 元素 FQN（= H1 标题 = 文件名去 `.md`）。
const FQN: &str = "App.Demo.Item";

/// 断言 `Err(ApiError::Contract(..))` 并返回文案（可辨错误面 + 文案含定位）。
fn contract_error(result: Result<Contract, ApiError>) -> String {
    match result {
        Ok(contract) => panic!("期望契约错误，实际 Ok：{contract:?}"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 断言 `verify` 报契约错误并返回文案。
fn verify_error(contract: &Contract, schema: &Value) -> String {
    match verify(contract, schema) {
        Ok(()) => panic!("期望双向 gate 报错，实际 Ok"),
        Err(ApiError::Contract(message)) => message,
        Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
    }
}

/// 最小合法契约源文（H1 + 两个散文段 + 1 op + 1 类型）。
fn pilot_source() -> String {
    format!(
        "# {FQN}\n\n## 职能\n\n门禁执行体。\n\n## 不变量\n\n报告写盘失败 MUST NOT 返回 Err。\n\n\
         ## 操作\n\n### file_stem\n\n```json api-op\n{{\"params\": [{{\"name\": \"timestamp\", \
         \"schema\": {{\"type\": \"string\"}}}}], \"returns\": {{\"type\": \"string\"}}}}\n```\n\n\
         ## 类型\n\n```json api-types\n{{\"LocalArchiCheck\": {{\"type\": \"object\", \
         \"properties\": {{}}}}}}\n```\n"
    )
}

/// 只有 H1 + 一个 op 的最小源文（无类型 fence）。
fn source_with_op(op_id: &str) -> String {
    format!(
        "# {FQN}\n\n## 操作\n\n### {op_id}\n\n```json api-op\n{{\"params\": [], \"returns\": \
         {{\"type\": \"string\"}}}}\n```\n"
    )
}

// ---------------------------------------------------------------------------
// 正向：完整契约
// ---------------------------------------------------------------------------

/// 正向（AC-1）：试点形态源文 → `Ok`，各字段逐项落位（`fqn` 取入参、`title` 取
/// H1、`ops[0].id` 取 heading、`ops[0].line` 指向 `###` 行、`types` 键在案、
/// `sections` 两段散文）。
#[test]
fn parse完整契约字段逐项落位() {
    let source = pilot_source();
    let contract = parse(&source, FQN, FILE).expect("合法契约应解析成功");

    assert_eq!(contract.fqn, FQN, "fqn 取入参");
    assert_eq!(contract.title, FQN, "title 取 H1 原文");
    assert_eq!(contract.file, FILE, "file 原样保留");
    assert_eq!(contract.ops.len(), 1, "一个 `### <op-id>` 段一个操作");
    assert_eq!(contract.ops[0].id, "file_stem", "op id 取自 heading");
    assert_eq!(
        contract.ops[0].line,
        line_at(&source, "### file_stem"),
        "op 行号指向 `###` 行（1 起始）"
    );
    assert!(
        contract.types.contains_key("LocalArchiCheck"),
        "api-types 键落位"
    );
    assert!(contract.reexports.is_empty(), "无 api-reexport 即空表");
    assert!(contract.reexport_prose.is_empty(), "无透传列表即空");
    assert_eq!(contract.sections.len(), 2, "两段非空散文（职能 / 不变量）");

    // 双向 gate 在真实 build_schema 产物上通过
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(verify(&contract, &schema), Ok(()), "三条 gate 全过");
}

/// 边界：H1 带前后空白 / 尾随空格 → 归一后仍 `Ok`（不因排版误报）。
#[test]
fn parse的h1空白归一() {
    for heading in [
        format!("# {FQN}"),
        format!("#  {FQN}  "),
        format!("#   {FQN}\t"),
    ] {
        let source = format!("{heading}\n\n## 职能\n\n正文。\n");
        let contract = parse(&source, FQN, FILE).expect("H1 空白归一后应成功");
        assert_eq!(contract.title, FQN, "标题归一为无空白形态");
    }
}

// ---------------------------------------------------------------------------
// 异常：H1 锚点
// ---------------------------------------------------------------------------

/// 异常（AC-1）：H1 与文件名不一致 → `Err`，文案**同时含** H1 原文与文件名期望值。
#[test]
fn parse的h1与文件名不一致() {
    let source = "# App.Demo.Other\n\n## 职能\n\n正文。\n";
    let message = contract_error(parse(source, FQN, FILE));
    assert!(
        message.contains("App.Demo.Other"),
        "文案含 H1 原文：{message}"
    );
    assert!(message.contains(FQN), "文案含文件名期望值：{message}");
    assert!(message.contains(FILE), "文案含文件定位：{message}");
}

/// 异常（AC-1）：无 H1（首行即 `## 职能`）/ H1 空标题（`# `）/ 第二个 H1 → 各 `Err`。
#[test]
fn parse缺h1空h1与多h1() {
    let no_h1 = "## 职能\n\n正文。\n";
    assert!(
        contract_error(parse(no_h1, FQN, FILE)).contains("缺 H1"),
        "无 H1 报缺 H1"
    );

    let empty_title = "# \n\n## 职能\n\n正文。\n";
    let message = contract_error(parse(empty_title, FQN, FILE));
    assert!(message.contains("H1"), "空标题落 H1 锚点判据：{message}");

    let two_h1 = format!("# {FQN}\n\n## 职能\n\n正文。\n\n# 另一个标题\n");
    let message = contract_error(parse(&two_h1, FQN, FILE));
    assert!(message.contains("第二个 H1"), "多 H1 即错：{message}");
}

// ---------------------------------------------------------------------------
// 异常：op heading 与 api-op fence 的归属
// ---------------------------------------------------------------------------

/// 异常（AC-1）：`### foo` 后无 `api-op` fence（直接跟 `## ` 段或到文件结束）→
/// `Err`，文案点名该 op 与 heading 行号。
#[test]
fn parse的op段缺fence() {
    let followed_by_section = format!("# {FQN}\n\n### foo\n\n## 职能\n\n正文。\n");
    let message = contract_error(parse(&followed_by_section, FQN, FILE));
    assert!(message.contains("foo"), "点名缺 fence 的 op：{message}");
    assert!(
        message.contains("api-op"),
        "点名缺失的 fence 语言标：{message}"
    );
    assert!(
        message.contains(&format!(":{}", line_at(&followed_by_section, "### foo"))),
        "文案带 heading 行号：{message}"
    );

    let at_end = format!("# {FQN}\n\n### bar\n");
    assert!(
        contract_error(parse(&at_end, FQN, FILE)).contains("bar"),
        "文件结束仍在等 fence 即错"
    );
}

/// 异常：一个 `###` 后出现**第二个** `api-op` fence → `Err`（归属唯一）；`api-op`
/// fence 出现在任何 `###` 之前 → `Err`（无归属）。
#[test]
fn parse的fence归属唯一() {
    let double_fence = format!(
        "# {FQN}\n\n### foo\n\n```json api-op\n{{\"params\": []}}\n```\n\n\
         ```json api-op\n{{\"params\": []}}\n```\n"
    );
    let message = contract_error(parse(&double_fence, FQN, FILE));
    assert!(
        message.contains("不属于任何") || message.contains("api-op"),
        "第二个 op fence 无归属：{message}"
    );

    let orphan_fence = format!(
        "# {FQN}\n\n```json api-op\n{{\"params\": []}}\n```\n\n### foo\n\n\
         ```json api-op\n{{\"params\": []}}\n```\n"
    );
    let message = contract_error(parse(&orphan_fence, FQN, FILE));
    assert!(
        message.contains("不属于任何"),
        "文件头的 api-op fence 无归属：{message}"
    );
}

/// 异常：`###` 后紧跟 `api-types` fence 而非 `api-op` → 该 op 缺 fence 即 `Err`
/// （本实现把缺 fence 判为解析期结构错，故 `api-types` 亦不落位——`Err` 无部分快照）。
#[test]
fn parse的op段被types_fence顶替() {
    let source = format!(
        "# {FQN}\n\n### foo\n\n```json api-types\n{{\"T\": {{\"type\": \"object\"}}}}\n```\n"
    );
    let message = contract_error(parse(&source, FQN, FILE));
    assert!(message.contains("foo"), "点名缺 api-op 的 op：{message}");
}

/// 异常：两个 `### file_stem`（同 id）→ `Err`（集合语义下重复即错）；
/// `### file_stem` 与 `### File_stem` → 两 op（不做大小写归一）。
#[test]
fn parse的op_id重复与大小写() {
    let duplicated = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"params\": []}}\n```\n\n\
         ### file_stem\n\n```json api-op\n{{\"params\": []}}\n```\n"
    );
    let message = contract_error(parse(&duplicated, FQN, FILE));
    assert!(message.contains("重复"), "重复 op id 即错：{message}");

    let cased = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"params\": []}}\n```\n\n\
         ### File_stem\n\n```json api-op\n{{\"params\": []}}\n```\n"
    );
    let contract = parse(&cased, FQN, FILE).expect("大小写不同视作两个 op");
    assert_eq!(contract.ops.len(), 2, "不做大小写归一");
    assert_eq!(contract.ops[0].id, "file_stem");
    assert_eq!(contract.ops[1].id, "File_stem");
}

/// 边界（D6）：fence 内不自包含 op 名——`id` 取自 heading；块内额外带 `"id"` 也
/// 不覆盖归属。
#[test]
fn parse的op名只取heading() {
    let plain = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"params\": [], \"returns\": \
         {{\"type\": \"string\"}}}}\n```\n"
    );
    let contract = parse(&plain, FQN, FILE).expect("纯签名 JSON 合法");
    assert_eq!(contract.ops[0].id, "file_stem", "id 取自 heading");
    assert_eq!(
        contract.ops[0].signature,
        json!({"params": [], "returns": {"type": "string"}}),
        "fence 原文原样保留（零翻译）"
    );

    let with_id = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"id\": \"other\", \"params\": []}}\n```\n"
    );
    let contract = parse(&with_id, FQN, FILE).expect("块内多键不影响归属");
    assert_eq!(contract.ops[0].id, "file_stem", "块内键不覆盖 heading 归属");
}

// ---------------------------------------------------------------------------
// 异常 / 边界：三类 fence 的唯一性与缺席
// ---------------------------------------------------------------------------

/// 异常：两个 `api-types` / 两个 `api-reexport` → `Err`（同文件各只允许一次）。
#[test]
fn parse的类型与透传fence各只允许一次() {
    let two_types = format!(
        "# {FQN}\n\n```json api-types\n{{\"A\": {{}}}}\n```\n\n\
         ```json api-types\n{{\"B\": {{}}}}\n```\n"
    );
    let message = contract_error(parse(&two_types, FQN, FILE));
    assert!(message.contains("api-types"), "点名重复的 fence：{message}");
    assert!(message.contains("一次"), "点明唯一性：{message}");

    let two_reexports = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"A\": \"App.Demo.A\"}}\n```\n\n\
         ```json api-reexport\n{{\"B\": \"App.Demo.B\"}}\n```\n"
    );
    let message = contract_error(parse(&two_reexports, FQN, FILE));
    assert!(
        message.contains("api-reexport"),
        "点名重复的 fence：{message}"
    );
}

/// 边界：`api-types` 缺席 → `Ok` 且 `types` 空；`api-reexport` 缺席 → `Ok` 且
/// `reexports` / `reexport_prose` 均空。
#[test]
fn parse缺席的fence取空表() {
    let contract = parse(&source_with_op("run"), FQN, FILE).expect("无 api-types 合法");
    assert!(contract.types.is_empty(), "无自有类型即空表");
    assert!(contract.reexports.is_empty(), "无透传 fence 即空表");
    assert!(contract.reexport_prose.is_empty(), "无透传列表即空列表");
}

/// 异常：三类 fence 各注入坏 JSON（缺右括号 / 尾逗号 / 单引号）→ `Err`，文案带
/// `file:line` 与 fence 语言标。
#[test]
fn parse的fence内json语法错() {
    let broken_op = format!("# {FQN}\n\n### foo\n\n```json api-op\n{{\"params\": [}}\n```\n");
    let message = contract_error(parse(&broken_op, FQN, FILE));
    assert!(message.contains(FILE), "文案带文件定位：{message}");
    assert!(message.contains("api-op"), "文案点名 fence：{message}");
    assert!(
        message.contains(&format!(":{}", line_at(&broken_op, "```json api-op"))),
        "文案带 fence 起始行号：{message}"
    );

    let trailing_comma = format!("# {FQN}\n\n```json api-types\n{{\"A\": {{}},}}\n```\n");
    assert!(
        contract_error(parse(&trailing_comma, FQN, FILE)).contains("api-types"),
        "尾逗号是非法 JSON"
    );

    let single_quotes = format!("# {FQN}\n\n```json api-reexport\n{{'A': 'App.Demo.A'}}\n```\n");
    assert!(
        contract_error(parse(&single_quotes, FQN, FILE)).contains("api-reexport"),
        "单引号是非法 JSON"
    );
}

// ---------------------------------------------------------------------------
// 散文透传列表
// ---------------------------------------------------------------------------

/// 正向（AC-11）：`- <导出名> → <子元素 FQN>` 形态的列表项收集为 `reexport_prose`；
/// 含 CJK 的解释句不算（机械分界）。
#[test]
fn parse的散文透传列表() {
    let source = format!(
        "# {FQN}\n\n## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n\
         - AgentSessionView → App.Frontend.Components.Agent.AgentSessionView\n\n\
         以上两项为原样透传（本行是解释句，不算导出名）。\n\n\
         ```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\", \
         \"AgentSessionView\": \"App.Frontend.Components.Agent.AgentSessionView\"}}\n```\n"
    );
    let contract = parse(&source, FQN, FILE).expect("透传形态合法");
    assert_eq!(
        contract.reexport_prose,
        vec!["AgentInput".to_owned(), "AgentSessionView".to_owned()],
        "两个列表项各收一个导出名"
    );
    assert_eq!(contract.reexports.len(), 2, "fence 两键");
}

/// 边界：`## 透传` 段为空 → `reexport_prose` 空；无该段但有 `api-reexport` fence →
/// `reexport_prose` 空（差集留给 `verify` 报）。
#[test]
fn parse的散文列表空形态() {
    let empty_section = format!(
        "# {FQN}\n\n## 透传\n\n## 职能\n\n正文。\n\n```json api-reexport\n\
         {{\"AgentInput\": \"App.Demo.Input\"}}\n```\n"
    );
    let contract = parse(&empty_section, FQN, FILE).expect("空透传段合法");
    assert!(contract.reexport_prose.is_empty(), "空段不产导出名");
    assert_eq!(contract.reexports.len(), 1, "fence 键仍在案");

    let no_section = format!(
        "# {FQN}\n\n## 职能\n\n正文。\n\n```json api-reexport\n\
         {{\"AgentInput\": \"App.Demo.Input\"}}\n```\n"
    );
    let contract = parse(&no_section, FQN, FILE).expect("无透传段合法");
    assert!(contract.reexport_prose.is_empty(), "无列表项");
    // 差集由 verify 报（此处只固化 parse 侧口径）
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains("AgentInput"),
        "verify 点名仅一侧存在的键：{message}"
    );
}

// ---------------------------------------------------------------------------
// 边界：规模 / 特殊字符 / 行尾
// ---------------------------------------------------------------------------

/// 边界：500 个 op 的长文档 → `Ok` 且 `ops.len() == 500`、行号严格单调递增；
/// 超长单行散文（>1000 chars）不 panic。
#[test]
fn parse长文档与超长单行() {
    let mut source = format!("# {FQN}\n\n## 职能\n\n{}\n\n## 操作\n\n", "长".repeat(1200));
    for index in 0..500 {
        source.push_str(&format!(
            "### op{index}\n\n```json api-op\n{{\"params\": [], \"returns\": \
             {{\"type\": \"string\"}}}}\n```\n\n"
        ));
    }
    let contract = parse(&source, FQN, FILE).expect("长文档应解析成功");
    assert_eq!(contract.ops.len(), 500, "逐 op 收齐");
    for window in contract.ops.windows(2) {
        assert!(
            window[0].line < window[1].line,
            "行号严格单调递增（{} < {}）",
            window[0].line,
            window[1].line
        );
    }
}

/// 边界：op id 含 emoji / fence 内容含 `\n` 与 `\0` 转义 → 不 panic、结果可判。
#[test]
fn parse特殊字符不panic() {
    let emoji_op = format!(
        "# {FQN}\n\n### 操作🎯\n\n```json api-op\n{{\"params\": [{{\"name\": \"输入\", \
         \"schema\": {{\"type\": \"string\", \"description\": \"换行\\n与冒号\\u0000\"}}}}]}}\n```\n"
    );
    let contract = parse(&emoji_op, FQN, FILE).expect("emoji op id 合法");
    assert_eq!(contract.ops[0].id, "操作🎯", "非 ASCII op id 原样保留");
    assert!(
        contract.ops[0].signature.to_string().contains("\\n"),
        "转义序列原样进 JSON 值"
    );
}

/// 边界：`\r\n` 行尾版本与 `\n` 版本解析结果集合相等；文件末无换行也不 panic。
#[test]
fn parse的crlf与无尾换行() {
    let lf = source_with_op("run");
    let crlf = lf.replace('\n', "\r\n");

    let from_lf = parse(&lf, FQN, FILE).expect("LF 版本合法");
    let from_crlf = parse(&crlf, FQN, FILE).expect("CRLF 版本合法");
    assert_eq!(
        from_lf.ops.len(),
        from_crlf.ops.len(),
        "行尾归一后 op 数相等"
    );
    assert_eq!(from_lf.ops[0].id, from_crlf.ops[0].id, "op id 相等");
    assert_eq!(
        from_lf.ops[0].line, from_crlf.ops[0].line,
        "行号不因行尾形态漂移"
    );
    assert_eq!(from_lf.types, from_crlf.types, "types 集合相等");

    let no_trailing = format!("# {FQN}\n\n## 职能\n\n正文。");
    let contract = parse(&no_trailing, FQN, FILE).expect("无尾换行合法");
    assert_eq!(contract.sections.len(), 1, "最后一段正常收");
}

/// 异常：fence 未闭合（文件结束仍在 fence 内）→ `Err`。
#[test]
fn parse的fence未闭合() {
    let source = format!("# {FQN}\n\n## 职能\n\n```json api-types\n{{\"A\": {{}}}}\n");
    let message = contract_error(parse(&source, FQN, FILE));
    assert!(message.contains("未闭合"), "点明未闭合：{message}");
}

// ---------------------------------------------------------------------------
// verify：三条 gate
// ---------------------------------------------------------------------------

/// 正向：heading 集合 ≡ op fence 集合、`types` 键 ⊆ `$defs`、`reexports` ≡ 散文
/// 列表 → `Ok(())`。
#[test]
fn verify三条gate全过() {
    let source = pilot_source();
    let contract = parse(&source, FQN, FILE).expect("合法契约");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(verify(&contract, &schema), Ok(()), "零文案即通过");
}

/// 异常（gate a）：同一 op id 在 `Contract.ops` 重复出现 → `Err`，文案点名该 id 与
/// 「heading 集合 ≡ fence 归属集合」语义。
#[test]
fn verify的gate_a重复op_id() {
    let mut contract = parse(&source_with_op("run"), FQN, FILE).expect("合法契约");
    let duplicated = contract.ops[0].clone();
    contract.ops.push(ContractOp {
        id: duplicated.id.clone(),
        line: duplicated.line + 10,
        signature: duplicated.signature,
    });
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(message.contains("run"), "点名重复的 op：{message}");
    assert!(message.contains("重复"), "点明重复语义：{message}");
}

/// 异常（gate b，AC-3）：`api-types` 键 `T` 而 `schema["$defs"]` 无 `T` → `Err` 且
/// 逐名点出；反方向（`$defs` 超集）→ `Ok`（规则是 ⊆，单向）。
#[test]
fn verify的gate_b类型缺定义() {
    let with_types = parse(
        &format!(
            "# {FQN}\n\n```json api-types\n{{\"T\": {{\"type\": \"object\"}}, \
             \"U\": {{\"type\": \"object\"}}}}\n```\n"
        ),
        FQN,
        FILE,
    )
    .expect("合法契约");
    let message = verify_error(&with_types, &json!({"$defs": {}}));
    assert!(
        message.contains('T') && message.contains('U'),
        "逐名点出缺失的类型：{message}"
    );

    let contract = parse(&source_with_op("run"), FQN, FILE).expect("合法契约");
    // ⊆ 方向：`$defs` 含契约未声明的额外键 → 仍 Ok
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let mut superset = schema.clone();
    if let Some(defs) = superset.get_mut("$defs").and_then(Value::as_object_mut) {
        defs.insert("Extra".to_owned(), json!({"type": "object"}));
    }
    assert_eq!(
        verify(&contract, &superset),
        Ok(()),
        "`$defs` 超集方向不报（规则单向）"
    );
}

/// 异常（gate c，AC-11）：fence 两键而散文列表一项 → `Err` 点名缺失项；散文两项而
/// fence 一键 → `Err`；两处键序不同 → `Ok`（集合语义，非序列）。
#[test]
fn verify的gate_c透传键集不等() {
    let two_keys_one_item = format!(
        "# {FQN}\n\n## 透传\n\n- AgentInput → App.Demo.Input\n\n```json api-reexport\n\
         {{\"AgentInput\": \"App.Demo.Input\", \"AgentBlocks\": \"App.Demo.Blocks\"}}\n```\n"
    );
    let contract = parse(&two_keys_one_item, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains("AgentBlocks"),
        "点名仅 fence 存在的键：{message}"
    );

    let one_key_two_items = format!(
        "# {FQN}\n\n## 透传\n\n- AgentInput → App.Demo.Input\n\
         - AgentBlocks → App.Demo.Blocks\n\n```json api-reexport\n\
         {{\"AgentInput\": \"App.Demo.Input\"}}\n```\n"
    );
    let contract = parse(&one_key_two_items, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains("AgentBlocks"),
        "点名仅散文存在的键：{message}"
    );

    let different_order = format!(
        "# {FQN}\n\n## 透传\n\n- AgentBlocks → App.Demo.Blocks\n\
         - AgentInput → App.Demo.Input\n\n```json api-reexport\n\
         {{\"AgentInput\": \"App.Demo.Input\", \"AgentBlocks\": \"App.Demo.Blocks\"}}\n```\n"
    );
    let contract = parse(&different_order, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "键序不同不报（集合语义）"
    );
}

/// 边界：空契约（`ops` / `types` / `reexports` 全空 + `$defs` 空）→ `Ok`；`schema`
/// 无 `$defs` 键 → `types` 非空时 `Err`、`types` 空时 `Ok`。
#[test]
fn verify空契约与缺defs() {
    let empty = parse(&format!("# {FQN}\n\n## 职能\n\n正文。\n"), FQN, FILE).expect("空契约合法");
    assert_eq!(
        verify(&empty, &json!({"$defs": {}})),
        Ok(()),
        "空契约无规则冲突"
    );
    assert_eq!(
        verify(&empty, &json!({"properties": {}})),
        Ok(()),
        "$defs 缺席且 types 空 → Ok"
    );

    let with_type = parse(
        &format!("# {FQN}\n\n```json api-types\n{{\"T\": {{\"type\": \"object\"}}}}\n```\n"),
        FQN,
        FILE,
    )
    .expect("合法契约");
    let message = verify_error(&with_type, &json!({"properties": {}}));
    assert!(message.contains('T'), "缺 $defs 时逐名点出：{message}");
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

/// 正向（AC-13）：试点契约里没有构造器 op（`### new` 不存在即 `is_constructor` 零命中
/// 的等价断言）。
#[test]
fn pilot契约不含构造器op() {
    let contract = parse(&pilot_source(), FQN, FILE).expect("合法契约");
    assert!(
        contract.ops.iter().all(|op| !is_constructor(&op.id)),
        "试点 op 集 MUST NOT 含构造器：{:?}",
        contract.ops.iter().map(|op| &op.id).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// 辅助
// ---------------------------------------------------------------------------

/// 子串所在行号（1 起始）。
fn line_at(source: &str, needle: &str) -> u32 {
    let index = source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("源文本中找不到 {needle:?}"));
    index as u32 + 1
}

// ---------------------------------------------------------------------------
// fence 内重复键在文本层判（D37）
// ---------------------------------------------------------------------------

/// 异常（AC-1 / AC-3 规则 1·4 / D37）：三类 fence 各注入一处**对象层级**重复键 →
/// 各 `Err(ApiError::Contract(..))`，文案点名该键 + fence 语言标 + `file:line`。
/// `serde_json` 的折叠语义（后者胜）不构成合法形态——故 MUST NOT 断言「折叠即 `Ok`」。
#[test]
fn parse的fence内重复键各fence报错() {
    let cases = [
        (
            "api-types",
            format!(
                "# {FQN}\n\n```json api-types\n{{\"A\": {{\"type\": \"object\"}}, \
                 \"A\": {{\"enum\": [\"x\"]}}}}\n```\n"
            ),
            "A",
        ),
        (
            "api-op",
            format!(
                "# {FQN}\n\n### run\n\n```json api-op\n{{\"params\": [], \"params\": []}}\n```\n"
            ),
            "params",
        ),
        (
            "api-reexport",
            format!(
                "# {FQN}\n\n```json api-reexport\n{{\"A\": \"App.Demo.A\", \
                 \"A\": \"App.Demo.B\"}}\n```\n"
            ),
            "A",
        ),
    ];

    for (fence, source, key) in cases {
        let message = contract_error(parse(&source, FQN, FILE));
        assert!(
            message.contains(&format!("`{key}`")),
            "{fence} 的文案点名重复键 `{key}`：{message}"
        );
        assert!(
            message.contains(fence),
            "{fence} 的文案点名 fence 语言标：{message}"
        );
        assert!(
            message.contains(FILE),
            "{fence} 的文案带文件定位：{message}"
        );
        // 三处夹具的 fence 内容均为单行，故重复键在该 fence 内偏移为 0 → 行号 = 起始行 + 1
        let expected_line = line_at(&source, &format!("```json {fence}")) + 1;
        assert!(
            message.contains(&format!(":{expected_line}:")),
            "{fence} 的文案带 `file:line`（该 fence 内重复键所在行）：{message}"
        );
        assert!(
            message.contains("重复声明"),
            "{fence} 点明「同名即定义两次」：{message}"
        );
    }
}

/// 边界（D37 判据面）：① 扫描**只覆盖对象**（数组内元素无身份）；② 嵌套对象逐层扫描；
/// ③ `A` 与 `a` 是两个键（零归一、零合并）；④ 同一 JSON 里两处**不同**键 → `Ok`；
/// ⑤ 扫描与解析用**同一份文本**（源码自证：`serde_json::from_str` 恰 2 处且两处之前
/// 各有一处键唯一性扫描）。
#[test]
fn parse的重复键扫描判据面() {
    // ① 数组元素无身份：同一深度两个对象各带一个同名键 → Ok
    let array_elements = format!(
        "# {FQN}\n\n### run\n\n```json api-op\n{{\"params\": [{{\"a\": 1}}, {{\"a\": 2}}]}}\n```\n"
    );
    let contract = parse(&array_elements, FQN, FILE).expect("数组元素各一个 `a`，跨元素不重复");
    assert_eq!(contract.ops.len(), 1);
    assert_eq!(
        contract.ops[0].signature["params"].as_array().map(Vec::len),
        Some(2),
        "两元素各自在案"
    );

    // ② 嵌套对象逐层扫描：内层对象重复 `b` → Err 且点名 `b`（外层无重复）
    let nested = format!(
        "# {FQN}\n\n### run\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"b\": 1, \
         \"b\": 2}}}}\n```\n"
    );
    let message = contract_error(parse(&nested, FQN, FILE));
    assert!(
        message.contains("`b`"),
        "嵌套层的重复键被点名（逐层扫描）：{message}"
    );

    // ③ `A` 与 `a` 是两个键（零归一）
    let cased = format!(
        "# {FQN}\n\n```json api-types\n{{\"A\": {{\"type\": \"object\"}}, \
         \"a\": {{\"type\": \"object\"}}}}\n```\n"
    );
    let contract = parse(&cased, FQN, FILE).expect("大小写不同即两个键");
    assert_eq!(contract.types.len(), 2, "两键各留一份定义（零合并）");

    // ④ 同一 JSON 里两处不同键 → Ok
    let distinct = format!(
        "# {FQN}\n\n```json api-types\n{{\"A\": {{\"type\": \"object\"}}, \
         \"B\": {{\"type\": \"string\"}}}}\n```\n"
    );
    let contract = parse(&distinct, FQN, FILE).expect("不同键不构成重复");
    assert_eq!(contract.types.len(), 2);

    // ⑤ 同一份文本：`serde_json::from_str` 恰 2 处（`parse_object` / `parse_object_value`），
    //    且两处各自之前紧邻一次 `duplicate_keys(body)` 扫描（先扫后解析，不可倒序）
    let source = include_str!("contract.rs");
    // 只数非注释行（doc 注释里的说明文字不算判据面）
    let code_lines: Vec<&str> = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    assert_eq!(
        code_lines
            .iter()
            .filter(|line| line.contains("serde_json::from_str"))
            .count(),
        2,
        "fence 内容解析恰两处（对象形态与值形态各一）"
    );
    assert_eq!(
        code_lines
            .iter()
            .filter(|line| line.contains("duplicate_keys(body)"))
            .count(),
        2,
        "两处解析之前各有一处键唯一性扫描（同一份文本，先扫后解析）"
    );
    // 逐对：每次解析前都先有一处扫描（同一份文本，先扫后解析）
    let scan_indexes: Vec<usize> = code_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("duplicate_keys(body)"))
        .map(|(index, _)| index)
        .collect();
    let parse_indexes: Vec<usize> = code_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("serde_json::from_str(body)"))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(scan_indexes.len(), parse_indexes.len());
    for (scan, parse) in scan_indexes.iter().zip(parse_indexes.iter()) {
        assert!(
            scan < parse,
            "键唯一性扫描 MUST 在解析之前（MUST NOT 先折叠再比对）"
        );
    }
}

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

/// 一份只含单个 op 的契约源文（`params[].name` 可注入）。
fn gate1_op_source(op_id: &str, param_name: Option<&str>) -> String {
    let params = match param_name {
        Some(name) => format!("[{{\"name\": \"{name}\", \"schema\": {{\"type\": \"string\"}}}}]"),
        None => "[]".to_owned(),
    };
    format!(
        "# {FQN}\n\n### {op_id}\n\n```json api-op\n{{\"params\": {params}, \"returns\": \
         {{\"type\": \"string\"}}}}\n```\n"
    )
}

/// 闸一报错的统一断言：`Err(ApiError::Contract(..))`、文案含名字与族别词。
fn gate1_error(source: &str, family: &str, name: &str) -> String {
    let contract = parse(source, FQN, FILE).expect("结构合法（闸一在 verify 判，不在 parse）");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains(name),
        "文案点名违规名字 `{name}`：{message}"
    );
    assert!(
        message.contains(family),
        "文案点名族别 `{family}`（七族可辨）：{message}"
    );
    assert!(
        message.contains("ASCII 标识符"),
        "点明判据（`[A-Za-z_][A-Za-z0-9_]*`）：{message}"
    );
    assert!(message.contains(FILE), "文案带文件定位：{message}");
    message
}

/// 正向（AC-23 ① / D45）：**七族**的合法形态各 `Ok(())`——① op id `file_stem` /
/// `parse2` / `_private` / `A`、② `$defs` 键 `LocalArchiCheck`、③ 透传键 `AgentInput`、
/// ④ `params[].name` `input` / `a1` / `_x`、⑤ `properties` 数据字段键 `count` / `a1`、
/// ⑥ `api-op` 的 `owner` = `LocalArchiCheck`、⑦ `$ref` 末段名 `SummaryReport`。
#[test]
fn verify闸一七族合法名字全过() {
    // ① op id 族
    for op_id in ["file_stem", "parse2", "_private", "A"] {
        let contract =
            parse(&gate1_op_source(op_id, Some("input")), FQN, FILE).expect("合法契约结构");
        let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
        assert_eq!(
            verify(&contract, &schema),
            Ok(()),
            "op id `{op_id}` 是合法 ASCII 标识符"
        );
    }

    // ④ 参数名族
    for param in ["input", "a1", "_x"] {
        let contract =
            parse(&gate1_op_source("run", Some(param)), FQN, FILE).expect("合法契约结构");
        let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
        assert_eq!(
            verify(&contract, &schema),
            Ok(()),
            "参数名 `{param}` 是合法 ASCII 标识符"
        );
    }

    // ② `$defs` 键 + ⑤ `properties` 数据字段键两族同判（同一 fence 内各一合法形态）
    for field in ["count", "a1"] {
        let with_types = format!(
            "# {FQN}\n\n```json api-types\n{{\"LocalArchiCheck\": {{\"type\": \"object\", \
             \"properties\": {{\"{field}\": {{\"type\": \"integer\"}}}}}}}}\n```\n"
        );
        let contract = parse(&with_types, FQN, FILE).expect("合法契约结构");
        let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
        assert_eq!(
            verify(&contract, &schema),
            Ok(()),
            "`$defs` 键与 `properties` 字段键 `{field}` 均合法"
        );
    }

    // ③ 透传键族
    let with_reexport = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    );
    let contract = parse(&with_reexport, FQN, FILE).expect("合法契约结构");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(verify(&contract, &schema), Ok(()), "透传键合法");

    // ⑥ `owner` 族 + ⑦ `$ref` 末段族（同一 api-op fence 内各一合法形态）
    let owner_source = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"owner\": \"LocalArchiCheck\", \
         \"params\": [{{\"name\": \"input\", \"schema\": {{\"type\": \"string\"}}}}], \
         \"returns\": {{\"$ref\": \"#/$defs/SummaryReport\"}}}}\n```\n"
    );
    let contract = parse(&owner_source, FQN, FILE).expect("合法契约结构");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "`owner` 与 `$ref` 末段名均合法"
    );
}

/// 异常（AC-23 ① 关键反例 / D45）：**七族**各拒非标识符，且**七条文案的族别词互不
/// 相同**（可辨）；`emit/{rust,ts}.rs` 内零 `is_identifier` 调用（渲染期不重复校验）。
#[test]
fn verify闸一七族各拒非标识符() {
    // ① op id 族
    let mut op_messages: Vec<String> = Vec::new();
    for invalid in ["带-连字符的 op", "file-stem", "a.b", "1abc"] {
        let message = gate1_error(&gate1_op_source(invalid, None), "op id", invalid);
        op_messages.push(message);
    }

    // ② `$defs` 键族
    let key_source = format!(
        "# {FQN}\n\n```json api-types\n{{\"Local-ArchiCheck\": {{\"type\": \"object\", \
         \"properties\": {{}}}}}}\n```\n"
    );
    let key_message = gate1_error(&key_source, "$defs 键名", "Local-ArchiCheck");

    // ③ 透传键族：散文取词面只认 ASCII 标识符，故含空格的键会先被 gate (c) 拦下——
    // 本用例构造「gate 全过、闸一唯一可判处」的形态（闸一负责名字合法性，两者分工）。
    // 注：真实 `build_schema` 会为该键产 `#/$defs/<键>` 形态的 `$ref`，其**末段即同名** ⇒
    // ⑦ 族按求值序先判（同名事实由先判者报，属在案形态）；此处以手写空 `$defs` 把 ⑦ 面
    // 摘除，使 ③ 族成为唯一可判处，两向各断言一次。
    let reexport_source = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"Agent Input\": \"App.Frontend.Components.Agent.Input\"}}\n```\n"
    );
    let mut reexport_contract = parse(&reexport_source, FQN, FILE).expect("结构合法");
    reexport_contract.reexport_prose = reexport_contract.reexports.keys().cloned().collect();
    let reexport_message = {
        let message = verify_error(&reexport_contract, &json!({"$defs": {}}));
        assert!(message.contains("Agent Input"), "点名违规名字：{message}");
        assert!(message.contains("透传导出名"), "点名透传键族：{message}");
        message
    };
    // 对照：真实装配面下同一键由 ⑦ 族（`$ref` 末段名）先报——两族互不遮蔽、各报各的
    let reexport_schema = build_schema(&reexport_contract, &meta()).expect("schema 应装配成功");
    let by_ref_family = verify_error(&reexport_contract, &reexport_schema);
    assert!(
        by_ref_family.contains("$ref 末段名") && by_ref_family.contains("Agent Input"),
        "同名事实在同一 key 上出现时由先判者（⑦）报出：{by_ref_family}"
    );

    // ④ 参数名族
    let param_message = gate1_error(&gate1_op_source("run", Some("in-put")), "参数名", "in-put");

    // ⑤ `properties` 数据字段键族（`struct_block` / `interface_block` 直插 `{field}`）
    let field_source = format!(
        "# {FQN}\n\n```json api-types\n{{\"LocalArchiCheck\": {{\"type\": \"object\", \
         \"properties\": {{\"a-b\": {{\"type\": \"integer\"}}}}}}}}\n```\n"
    );
    let field_message = gate1_error(&field_source, "properties 数据字段键", "a-b");

    // ⑥ `api-op` fence 的 `owner` 族（`op_witness` 直插 `super::{owner}::{op.id}`）
    let owner_source = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"owner\": \"带-连字符\", \
         \"params\": [], \"returns\": {{\"type\": \"string\"}}}}\n```\n"
    );
    let owner_message = gate1_error(&owner_source, "owner 键", "带-连字符");

    // ⑦ `$ref` 末段名族（两侧 `value_type` 直插类型位置；`choice_payload` 已按 D59 删除）
    let reference_source = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"params\": [{{\"name\": \"input\", \
         \"schema\": {{\"$ref\": \"#/$defs/带 空格\"}}}}], \"returns\": {{\"type\": \"string\"}}}}\n```\n"
    );
    let reference_message = gate1_error(&reference_source, "$ref 末段名", "带 空格");

    // 七族族别词互不相同（可辨错误面）
    let families = [
        ("op id", &op_messages[0]),
        ("$defs 键名", &key_message),
        ("透传导出名", &reexport_message),
        ("参数名", &param_message),
        ("properties 数据字段键", &field_message),
        ("owner 键", &owner_message),
        ("$ref 末段名", &reference_message),
    ];
    assert_eq!(families.len(), 7, "闸一射程七族");
    for (family, message) in families {
        assert!(
            message.contains(family),
            "`{family}` 族文案含该族别词：{message}"
        );
    }
    for (index, (family, _)) in families.iter().enumerate() {
        for (other_index, (other, _)) in families.iter().enumerate() {
            if index >= other_index {
                continue;
            }
            assert_ne!(family, other, "七族族别词互不相同");
        }
    }

    // 渲染期不重复校验同一事实（D45 ②）
    for (path, source) in [
        ("emit/rust.rs", include_str!("emit/rust.rs")),
        ("emit/ts.rs", include_str!("emit/ts.rs")),
    ] {
        assert!(
            !source.contains("is_identifier"),
            "`{path}` 内零 `is_identifier`（渲染期不重复闸一）"
        );
    }
}

/// 边界（D45）：非 ASCII / 空名族全 `Err`；`x-class` 方法名与 op id 同判据；**缺名仍
/// 允许**（`params[]` 项无 `name` 键时闸一不报——「缺名与非法名是两件事」）。
#[test]
fn verify闸一非ascii与空名族() {
    // 非 ASCII 名（中文 / 带重音 / emoji）：结构合法，闸一在 `verify` 判
    for invalid in ["校验", "café", "🚀"] {
        let message = gate1_error(&gate1_op_source(invalid, None), "op id", invalid);
        assert!(
            message.contains("ASCII 标识符"),
            "非 ASCII 名字也走同一判据（语言无关）：{message}"
        );
    }

    // 空名族：`### ` / `###   ` 的 heading 去尾空白后无标题文本，属**结构**不合法
    // （`parse` 即 `Err(ApiError::Contract(..))`）——两种入口都止于契约错误面
    for invalid in ["", "  "] {
        let message = match parse(&gate1_op_source(invalid, None), FQN, FILE) {
            Ok(contract) => {
                let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
                verify_error(&contract, &schema)
            }
            Err(ApiError::Contract(message)) => message,
            Err(other) => panic!("期望 ApiError::Contract，实际：{other:?}"),
        };
        assert!(!message.is_empty(), "空名族的错误面非空：{message}");
    }

    // `x-class` 的方法名与 op id 同判据（不另立第二套）
    let method_source = format!(
        "# {FQN}\n\n```json api-types\n{{\"Runner\": {{\"x-class\": true, \"properties\": \
         {{\"run-it\": {{\"x-function\": {{\"params\": [], \"returns\": {{\"type\": \"boolean\"}}}}}}}}}}}}\n```\n"
    );
    let message = gate1_error(&method_source, "方法名", "run-it");
    assert!(
        message.contains("方法名"),
        "x-class 方法名族单独点名：{message}"
    );

    // `x-class` 方法内的参数名同族报错
    let method_param_source = format!(
        "# {FQN}\n\n```json api-types\n{{\"Runner\": {{\"x-class\": true, \"properties\": \
         {{\"run\": {{\"x-function\": {{\"params\": [{{\"name\": \"in-put\", \"schema\": \
         {{\"type\": \"string\"}}}}], \"returns\": {{\"type\": \"boolean\"}}}}}}}}}}}}\n```\n"
    );
    let message = gate1_error(&method_param_source, "参数名", "in-put");
    assert!(
        message.contains("in-put"),
        "方法内参数名同族报错：{message}"
    );

    // 缺名仍允许：`params[]` 项无 `name` 键 → verify 不报（兜底 `arg` 属渲染期）
    let nameless = format!(
        "# {FQN}\n\n### run\n\n```json api-op\n{{\"params\": [{{\"schema\": {{\"type\": \
         \"string\"}}}}], \"returns\": {{\"type\": \"string\"}}}}\n```\n"
    );
    let contract = parse(&nameless, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "缺名与非法名是两件事（缺名不报，渲染期兜底 `arg`）"
    );
}

/// 边界（AC-23 ① 源码自证 / D45）：`is_identifier` 在 `api/` 产品面恰 1 处定义且在
/// `verify` 体内被消费；**七族**（八个族别串——`方法名` 是 ① op id 族在 `x-class`
/// 方法面上的同判据子面）各有调用点、`$ref` 末段切分复用 `api::ref_name`、
/// 「插值点 ↔ 来源族」枚举表在场；`emit/{rust,ts}.rs` 与 `schema.rs` 内零命中；
/// 闸一与既有三条 gate 互不遮蔽。
#[test]
fn verify闸一判据单点() {
    let source = include_str!("contract.rs");
    assert_eq!(
        source.matches("fn is_identifier").count(),
        1,
        "`is_identifier` 在产品面恰 1 处定义"
    );
    assert!(
        source.contains("fn is_identifier") && source.contains("fn verify("),
        "定义与闸一消费点同文件"
    );
    // 定义行在产品 `.rs` 内唯一（其余 `api/` 文件零命中）
    for (path, other) in [
        ("emit/rust.rs", include_str!("emit/rust.rs")),
        ("emit/ts.rs", include_str!("emit/ts.rs")),
        ("schema.rs", include_str!("schema.rs")),
        ("check.rs", include_str!("check.rs")),
        ("paths.rs", include_str!("paths.rs")),
    ] {
        assert!(
            !other.contains("fn is_identifier"),
            "`{path}` 内零 `is_identifier` 定义（闸一单点在 contract.rs）"
        );
    }

    // 七族的调用点全在 `verify` 函数体内，且七个族别串各在案
    let verify_body = function_body(source, "pub fn verify(");
    let families = [
        "op id",
        "$defs 键名",
        "透传导出名",
        "参数名",
        "properties 数据字段键",
        "owner 键",
        "$ref 末段名",
    ];
    assert_eq!(families.len(), 7, "闸一射程七族");
    for family in families {
        assert!(
            verify_body.contains(family),
            "七族族别串 `{family}` 出现在 `verify` 体内（闸一的判据落点）"
        );
    }
    // `x-class` 方法名是 ① 族在方法面上的同判据子面（族别串可辨，不另立第二套判据）
    assert!(
        verify_body.contains("方法名"),
        "`x-class` 方法名族在 `verify` 体内同判"
    );

    // 判据函数在 `verify` 体内的调用点数 ≥ 七（七族各至少一处；`参数名` 与 `$ref 末段名`
    // 两侧面共用同一族别串，故调用点多于七）
    let gate1_calls = verify_body.matches("is_identifier(").count();
    assert!(
        gate1_calls >= 7,
        "闸一在 `verify` 体内的调用点覆盖七族（实际 {gate1_calls} 处）"
    );

    // ⑦ 族的末段切分复用 `api::ref_name`（本文件零第二份切分实现）
    assert_eq!(
        source.matches("fn ref_name").count(),
        0,
        "本文件零 `fn ref_name` 定义（复用 `super::ref_name`）"
    );
    assert!(
        source.matches("super::ref_name(").count() >= 1,
        "`$ref` 末段名经 `super::ref_name` 单一实现产出"
    );

    // D45 的「插值点 ↔ 来源族」枚举表在场（①…⑦ 七个族编号各在案）
    for marker in ["①", "②", "③", "④", "⑤", "⑥", "⑦"] {
        assert!(
            verify_body.contains(marker),
            "枚举表的族编号 `{marker}` 在案（缺任一行即该插值点无人守）"
        );
    }

    // 互不遮蔽（两向）：gate 违例与闸一违例各自独立报错
    // ① 只有闸一违例（gate 全过）→ 闸一报错
    let only_gate1 = format!(
        "# {FQN}\n\n### file-stem\n\n```json api-op\n{{\"params\": [], \"returns\": \
         {{\"type\": \"string\"}}}}\n```\n"
    );
    let contract = parse(&only_gate1, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains("file-stem") && message.contains("ASCII 标识符"),
        "闸一违例独立报出：{message}"
    );

    // ② 只有 gate (b) 违例（名字全合法）→ gate 报错，文案不借用闸一措辞
    let only_gate_b =
        format!("# {FQN}\n\n```json api-types\n{{\"T\": {{\"type\": \"object\"}}}}\n```\n");
    let contract = parse(&only_gate_b, FQN, FILE).expect("结构合法");
    let message = verify_error(&contract, &json!({"$defs": {}}));
    assert!(message.contains('T'), "gate (b) 违例独立报出：{message}");
    assert!(
        !message.contains("ASCII 标识符"),
        "gate (b) 的文案不借用闸一措辞（互不遮蔽）：{message}"
    );
}

/// 边界（AC-23 ① 后三族边界 / D45）：⑤ `properties` 为空对象 → `Ok`（无键可判）；
/// ⑥ `api-op` 无 `owner` 键 → `Ok`（该键非必填，缺省不出见证）；⑦ 外部文件形态
/// `./X.schema.json#/$defs/T` → **末段 `T` 参与判定、前缀不判**（`Ok`），而末段非法时
/// 同族报错（前缀本身永不被判）。
#[test]
fn verify闸一后三族边界形态() {
    // ⑤ 空 `properties`
    let empty_properties = format!(
        "# {FQN}\n\n```json api-types\n{{\"LocalArchiCheck\": {{\"type\": \"object\", \
         \"properties\": {{}}}}}}\n```\n"
    );
    let contract = parse(&empty_properties, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "`properties` 为空对象即无键可判"
    );

    // ⑥ 无 `owner` 键（自由函数见证）
    let no_owner = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"params\": [], \"returns\": \
         {{\"type\": \"string\"}}}}\n```\n"
    );
    let contract = parse(&no_owner, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "`owner` 非必填（缺省不出 `super::<Type>::` 见证）"
    );

    // ⑦ 外部文件形态：前缀不判、末段参与
    let external = format!(
        "# {FQN}\n\n```json api-types\n{{\"LocalArchiCheck\": {{\"type\": \"object\", \
         \"properties\": {{\"summary\": {{\"$ref\": \"../Mod/App.Mod.schema.json#/$defs/SummaryReport\"}}}}}}}}\n```\n"
    );
    let contract = parse(&external, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "外部文件形态只判末段名（`SummaryReport` 合法；前缀不被判）"
    );

    let external_bad = format!(
        "# {FQN}\n\n```json api-types\n{{\"LocalArchiCheck\": {{\"type\": \"object\", \
         \"properties\": {{\"summary\": {{\"$ref\": \"../Mod/App.Mod.schema.json#/$defs/带 空格\"}}}}}}}}\n```\n"
    );
    let message = gate1_error(&external_bad, "$ref 末段名", "带 空格");
    assert!(
        !message.contains("../Mod/"),
        "文案只点名末段名，不把前缀当名字：{message}"
    );
}

// ---------------------------------------------------------------------------
// 落点闸：`x-function` 的合法落点封闭二处（D47 / AC-25）
// ---------------------------------------------------------------------------

/// 越界形态的 `api-types` 源文（`definition` = `$defs.T` 的值）。
fn types_source(definition: &str) -> String {
    format!("# {FQN}\n\n```json api-types\n{{\"T\": {definition}}}\n```\n")
}

/// 断言 `verify` 报**落点**契约错误：定形族别串 `x-function 落点` + 指定 JSON pointer +
/// 文件定位，且文案点名合法二处（可辨「位置错」与「名字错」两类错误面）。
fn position_error(source: &str, pointer: &str) -> String {
    let contract = parse(source, FQN, FILE).expect("结构合法（落点闸在 verify 判，不在 parse）");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains("x-function 落点"),
        "文案含定形族别串 `x-function 落点`：{message}"
    );
    assert!(
        message.contains("JSON pointer"),
        "文案点明 JSON pointer：{message}"
    );
    assert!(
        message.contains(pointer),
        "文案逐个点名越界位置的 pointer `{pointer}`：{message}"
    );
    assert!(message.contains(FILE), "文案带文件定位：{message}");
    assert!(
        !message.contains("ASCII 标识符"),
        "落点错与名字错是两类错误面（MUST NOT 借用闸一措辞）：{message}"
    );
    message
}

/// 异常（**AC-25 行为判据** / D47）：`x-function` 写在四种越界位置 ⇒ 各
/// `Err(ApiError::Contract(..))`，文案逐形态点名 JSON pointer 且含定形族别串
/// `x-function 落点`；对照：同一形态把 `x-function` 搬到合法二处 → 各 `Ok(())`。
#[test]
fn verify落点闸四种越界形态各报契约错误() {
    // ① `$defs.T.properties.call` 的 `items`（数组元素上）
    let in_items = types_source(
        "{\"type\": \"object\", \"properties\": {\"call\": {\"type\": \"array\", \"items\": \
         {\"x-function\": {\"params\": [], \"returns\": {\"type\": \"string\"}}}}}}",
    );
    position_error(&in_items, "$defs.T.properties.call.items");

    // ① 对照（合法二处的第二处）：`x-function` 直接作为 `properties` 的值
    let legal_property = types_source(
        "{\"type\": \"object\", \"properties\": {\"call\": {\"x-function\": {\"params\": [], \
         \"returns\": {\"type\": \"string\"}}}}}",
    );
    let contract = parse(&legal_property, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "`$defs` 条目第一层 `properties` 的**值**是合法落点②"
    );

    // ② `additionalProperties` 上
    let in_additional = types_source(
        "{\"type\": \"object\", \"additionalProperties\": {\"x-function\": {\"params\": [], \
         \"returns\": {\"type\": \"string\"}}}}",
    );
    position_error(&in_additional, "$defs.T.additionalProperties");

    // ③ `oneOf` 分支上
    let in_one_of = types_source(
        "{\"oneOf\": [{\"type\": \"string\"}, {\"x-function\": {\"params\": [], \"returns\": \
         {\"type\": \"string\"}}}]}",
    );
    position_error(&in_one_of, "$defs.T.oneOf[1]");

    // ④ `$defs` 条目**自身顶层**（整份条目就是 `x-function`）
    let at_definition_top =
        types_source("{\"x-function\": {\"params\": [], \"returns\": {\"type\": \"string\"}}}");
    position_error(&at_definition_top, "$defs.T");
}

/// 异常（**AC-25 行为判据** / D47 第五形态）：`api-reexport` 的 fence 内出现
/// `x-function` → 同形 `Err`（合法落点不含透传面）。
#[test]
fn verify落点闸拒api_reexport内的x_function() {
    // `build_schema` 会先拦「透传目标必须是 FQN 字符串」这一形态（装载期序在 `verify`
    // 之前），故本形态以「已装配 schema 上的 `Contract` 直构造」表达——落点闸的判据本身
    // 仍只在 `verify` 内被观测（MUST NOT 为跑通用例把闸下移或上移）。
    let source = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Demo.AgentInput\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Demo.AgentInput\n"
    );
    let contract = parse(&source, FQN, FILE).expect("结构合法");
    let mut contract = contract;
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    contract.reexports.insert(
        "AgentInput".to_owned(),
        json!({"x-function": {"params": [], "returns": {"type": "string"}}}),
    );
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains("x-function 落点") && message.contains("JSON pointer"),
        "透传面内的 `x-function` 点明定形族别串与 pointer：{message}"
    );
    assert!(
        message.contains("api-reexport.AgentInput"),
        "pointer 指向该透传条目：{message}"
    );

    // 对照：透传条目取合法形态（值为目标 FQN 字符串）→ 落点面零命中
    let legal = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Demo.AgentInput\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Demo.AgentInput\n"
    );
    let contract = parse(&legal, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "透传条目是字符串 FQN，落点面不参与"
    );
}

/// 正向（**AC-25 合法二处** / D47）：`x-function` 的两种书写形态都落在合法落点①
/// （`api-op` 顶层签名）——裸形态与显式 `{"x-function": …}` 包裹形态各 `Ok(())`。
#[test]
fn verify落点闸合法二处不受影响() {
    // 落点① 裸形态（`api-op` fence 内容即签名本身）
    let bare = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"params\": [{{\"name\": \"timestamp\", \
         \"schema\": {{\"type\": \"string\"}}}}], \"returns\": {{\"type\": \"string\"}}}}\n```\n"
    );
    let contract = parse(&bare, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "`api-op` 顶层签名的裸形态是合法落点①"
    );

    // 落点① 显式包裹形态（`{"x-function": …}` 两书写等价）
    let wrapped = format!(
        "# {FQN}\n\n### file_stem\n\n```json api-op\n{{\"x-function\": {{\"params\": [], \
         \"returns\": {{\"type\": \"string\"}}}}}}\n```\n"
    );
    let contract = parse(&wrapped, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "`api-op` 的显式 `x-function` 包裹形态同为合法落点①"
    );

    // 落点②：`x-class` 的 trait 方法（`$defs` 条目第一层 `properties` 的值）
    let class = format!(
        "# {FQN}\n\n```json api-types\n{{\"Runner\": {{\"x-class\": true, \"properties\": \
         {{\"run\": {{\"x-function\": {{\"params\": [], \"returns\": {{\"type\": \
         \"boolean\"}}}}}}}}}}}}\n```\n"
    );
    let contract = parse(&class, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "`x-class` 的方法（`properties` 的值）是合法落点②"
    );

    // 合法落点内部的更深层 `x-function` 也非法（函数作为值的再一层表达不了）
    let nested_inside_legal = types_source(
        "{\"type\": \"object\", \"properties\": {\"call\": {\"x-function\": {\"params\": \
         [{\"name\": \"cb\", \"schema\": {\"x-function\": {\"params\": [], \"returns\": \
         {\"type\": \"string\"}}}}], \"returns\": {\"type\": \"string\"}}}}}",
    );
    let message = position_error(&nested_inside_legal, "$defs.T.properties.call.x-function");
    assert!(
        message.contains("properties.call.x-function"),
        "合法落点内部的第二层同样点出 pointer：{message}"
    );
}

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

/// 边界（互不遮蔽）：同一契约里一处落点越界 + 一处 `x-class` 带属性（D27 违例）→
/// `verify` 先报落点 `Err`；单独去掉落点违例后 `verify_defs` 的 `x-class` 违例仍各自
/// `Err`（两条判据互不吞并）；落点闸不因 `$defs` 值含泛型描述而改变判定（泛型判据在
/// `emit` 侧，两件事分离）。
#[test]
fn 落点闸与x_class泛型判据互不遮蔽() {
    use super::schema::verify_defs;

    // ① 两违例并存形态 → `verify` 先报落点错（装载期序在前）
    let both = types_source(
        "{\"type\": \"object\", \"properties\": {\"call\": {\"type\": \"array\", \"items\": \
         {\"x-function\": {\"params\": [], \"returns\": {\"type\": \"string\"}}}}}}",
    );
    let contract = parse(&both, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains("x-function 落点") && message.contains("$defs.T.properties.call.items"),
        "`verify` 先报落点错（装载期序在前）：{message}"
    );

    // ② 单独去掉落点违例 → `x-class` 带属性的违例仍被 `verify_defs` 报出（互不吞并）
    let only_class = format!(
        "# {FQN}\n\n```json api-types\n{{\"Hooks\": {{\"x-class\": true, \"properties\": \
         {{\"count\": {{\"type\": \"integer\"}}}}}}}}\n```\n"
    );
    let contract = parse(&only_class, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "`x-function` 不在场 → 落点面零命中"
    );
    match verify_defs(&schema) {
        Err(ApiError::Contract(message)) => {
            assert!(
                message.contains("Hooks") && message.contains("properties.count"),
                "`x-class` 带属性的判据独立报出：{message}"
            );
        }
        other => panic!("期望 `x-class` 违例报契约错误，实际：{other:?}"),
    }

    // ③ 泛型描述在场不改变落点判定（泛型归 `emit::reject_generics`，两件事分离）
    let with_generics = types_source(
        "{\"type\": \"object\", \"typeParameters\": [\"T\"], \"properties\": {\"call\": \
         {\"type\": \"array\", \"items\": {\"x-function\": {\"params\": [], \"returns\": \
         {\"type\": \"string\"}}}}}}",
    );
    let contract = parse(&with_generics, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    let message = verify_error(&contract, &schema);
    assert!(
        message.contains("x-function 落点"),
        "落点闸的判定与泛型描述无关：{message}"
    );
}

// ---------------------------------------------------------------------------
// Contract 字段面（D51 / AC-28）与闸一合法面补充（D45 边界）
// ---------------------------------------------------------------------------

/// 边界（**D51 / AC-28**）：`Contract` 恰 8 字段（`fqn` / `title` / `file` / `ops` /
/// `types` / `reexports` / `reexport_prose` / `sections`）；源码自证零 `types_line` /
/// `reexports_line`（两条**分开判**，MUST NOT 写成一条正则——竖线会破坏表格行结构）、
/// `^pub struct Contract {` 恰 1（**MUST 用带 `{` 的定形串**——裸 `pub struct Contract`
/// 会把 `ContractOp` / `ContractSection` 一并命中）；`ContractOp.line` 的 doc 明写
/// 「heading 所在行」。
#[test]
fn contract字段面仍八且零行号死字段() {
    let source = include_str!("contract.rs");
    assert_eq!(
        source.matches("pub struct Contract {").count(),
        1,
        "`pub struct Contract {{` 恰 1 处（定形串带 `{{`，避开 `ContractOp` / `ContractSection`）"
    );

    // 字段面逐项：八个字段名各恰 1 处声明
    let start = source.find("pub struct Contract {").expect("结构体在案");
    let body = &source[start..];
    let end = body.find("\n}").expect("结构体收尾");
    let fields: Vec<&str> = body[..end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.trim())
        .collect();
    assert_eq!(
        fields,
        vec![
            "fqn",
            "title",
            "file",
            "ops",
            "types",
            "reexports",
            "reexport_prose",
            "sections"
        ],
        "`Contract` 恰 8 字段且序稳定（D51 不为凑行号扩公共类型面）"
    );

    // 零行号死字段（两条分开判）
    assert_eq!(
        source.matches("types_line").count(),
        0,
        "`contract.rs` 内零 `types_line`（MUST NOT 为凑行号扩 `Contract`）"
    );
    assert_eq!(
        source.matches("reexports_line").count(),
        0,
        "`contract.rs` 内零 `reexports_line`"
    );

    // `ContractOp.line` 的 doc 明写语义（heading 所在行）
    let op_start = source
        .find("pub struct ContractOp {")
        .expect("`ContractOp` 在案");
    let op_body = &source[op_start..];
    let op_end = op_body.find("\n}").expect("`ContractOp` 收尾");
    assert!(
        op_body[..op_end].contains("heading 所在行"),
        "`ContractOp.line` 的 doc 明写「heading 所在行」"
    );

    // 行为面：`parse` 产出的 `line` 指向 `### <op-id>` 那一行
    let source_text = pilot_source();
    let contract = parse(&source_text, FQN, FILE).expect("试点源文应解析成功");
    let heading_line = source_text
        .lines()
        .position(|line| line.starts_with("### file_stem"))
        .expect("heading 在案") as u32
        + 1;
    assert_eq!(
        contract.ops[0].line, heading_line,
        "`ContractOp.line` 即 heading 行（1 起始）"
    );
}

/// 边界（**D59** 与本文件的交汇）：`contract.rs` 内 `choice_payload` **零命中**（该私有
/// 函数已整条删除，⑦ `$ref` 末段名族的注释改指 `value_type`）；`contract_test.rs` 的同一
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
        source.contains("value_type"),
        "注释词面改指 `value_type`（同一映射单点）"
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

/// 边界（**D45** 边界）：闸一的名字合法面不被误伤——`_x1` / `A` / `a9` / 超长纯 ASCII
/// 标识符 → `Ok`（形态合法即放行，**无长度上限**）；`A` 与 `a`（大小写不同）→ 两名字
/// 均合法（零归一、零合并）。
#[test]
fn verify闸一名字合法面不被误伤() {
    for op_id in ["_x1", "A", "a9", &"a".repeat(1200)] {
        let contract = parse(&source_with_op(op_id), FQN, FILE).expect("结构合法");
        let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
        assert_eq!(
            verify(&contract, &schema),
            Ok(()),
            "形态合法的名字放行（无长度上限、零归一）：{}",
            &op_id[..op_id.len().min(16)]
        );
    }

    // 大小写不同 ≠ 同一名字：两 op 各在案，零归一合并
    let two_cases = format!(
        "# {FQN}\n\n## 操作\n\n### Run\n\n```json api-op\n{{\"params\": [], \"returns\": \
         {{\"type\": \"string\"}}}}\n```\n\n### run\n\n```json api-op\n{{\"params\": [], \
         \"returns\": {{\"type\": \"string\"}}}}\n```\n"
    );
    let contract = parse(&two_cases, FQN, FILE).expect("结构合法");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    assert_eq!(contract.ops.len(), 2, "大小写不同视作两 op（零归一）");
    assert_eq!(
        verify(&contract, &schema),
        Ok(()),
        "两名字均合法（`Run` 与 `run` 各自在案）"
    );
}
