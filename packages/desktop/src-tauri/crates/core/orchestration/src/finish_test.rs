//! `finish.rs`（`finish_step` 工具协议单点）的单元测试：工具名单点、两形态
//! description / parameters、`validate` 的臂分派与 `FinishPayload` 线形态。
//!
//! 不 mock：`checklist_entries` / `validate_checklist` / `validate_decision` /
//! `derive_verdict` 全部真实组合（分派语义即被测对象，替身会掩盖臂错配）；
//! prompt 语料取自 `workflow::write::phase_table` 的 `include_str!` 内嵌文本
//!（编译期，零文件 IO）。

use agent::FinishSpec;
use workflow::model::Verdict;
use workflow::write::{backtrack_options, phase_table};

use crate::decision::{ensure_backtrack_allowed, DecisionAction};
use crate::finish::{description, parameters, validate, FinishPayload, TOOL_NAME};
use crate::verdict::{EvaluatorChecklist, MAX_REPORT_CHARS};

// ---------------------------------------------------------------------------
// 装置：8 份 evaluator 语料自相位表取得（编译期内嵌，零 fs::read）
// ---------------------------------------------------------------------------

/// 两表全部 evaluator prompt（proposal / test-design / test-gen 两表共用同一
/// `include_str!` 常量，按文本去重后恰 8 份）。
fn evaluator_prompts() -> Vec<&'static str> {
    let mut prompts: Vec<&'static str> = Vec::new();
    for table_key in ["requirement", "test-only"] {
        let table = phase_table(table_key).expect("相位表键合法");
        for definition in table {
            if let Some(spec) = &definition.evaluator {
                if !prompts.contains(&spec.prompt) {
                    prompts.push(spec.prompt);
                }
            }
        }
    }
    prompts
}

/// 单份语料按相位 id 取得（逐份对拍用例的语料源）。
fn evaluator_prompt_of(phase: &str) -> &'static str {
    for table_key in ["requirement", "test-only"] {
        let table = phase_table(table_key).expect("相位表键合法");
        if let Some(definition) = table.iter().find(|definition| definition.id == phase) {
            return definition
                .evaluator
                .as_ref()
                .map(|spec| spec.prompt)
                .unwrap_or_else(|| panic!("相位 {phase} 应有 evaluator prompt"));
        }
    }
    panic!("相位 {phase} 在两表中缺席");
}

/// 两行静态清单 prompt（校验 / 分派用例的合成语料）。
fn synthetic_prompt() -> String {
    "## 静态清单\n\n| ID | 检查项 | 判断依据 |\n|---|---|---|\n\
     | C1 | 变更代码中无安全漏洞 | 检查注入面 |\n\
     | C2 | 变更代码有对应测试 | 检查测试文件 |\n"
        .to_owned()
}

/// 合法 evaluator 参数（覆盖合成清单全 ID）。
fn valid_checklist_args() -> serde_json::Value {
    serde_json::json!({
        "report": "评估报告正文",
        "items": [
            { "id": "C1", "pass": true, "evidence": "src/a.rs:10" },
            { "id": "C2", "pass": false, "evidence": "缺测试" }
        ]
    })
}

/// 源码自证读产品源文件（`CARGO_MANIFEST_DIR` 相对）。
fn source_of(relative: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("读取 {relative} 失败: {error}"))
}

/// 剔除注释行（文档 / 行注释不承载字面量作者面，自证只看代码行）。
fn code_lines(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("///") || trimmed.starts_with("//"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 源码扫描用的四枚字符常量（避免转义字面量噪声）。
const NEWLINE: char = '\n';
const BACKSLASH: char = '\\';
const QUOTE: char = '"';
const SLASH: char = '/';

// ---------------------------------------------------------------------------
// 正向：工具名单点
// ---------------------------------------------------------------------------

#[test]
fn 工具名单点_finish_step且字面量单处() {
    assert_eq!(TOOL_NAME, "finish_step", "工具名单点线值");

    // 源码自证：`"finish_step"` 字面量在代码行（注释除外）只此一处。
    let own = code_lines(&source_of("src/finish.rs"));
    assert_eq!(
        own.matches("\"finish_step\"").count(),
        1,
        "finish.rs 代码行内字面量恰一处"
    );
    assert!(
        own.contains("pub const TOOL_NAME"),
        "字面量作者处为 TOOL_NAME 常量"
    );
}

// ---------------------------------------------------------------------------
// 正向：description 两形态
// ---------------------------------------------------------------------------

#[test]
fn description两形态_各含关键语义且互不相等且不膨胀() {
    let evaluator = description(FinishSpec::EvaluatorChecklist);
    let decision = description(FinishSpec::Decision);

    assert!(!evaluator.is_empty(), "evaluator 形态描述非空");
    assert!(evaluator.contains("清单"), "evaluator 形态含清单语义");
    assert!(
        evaluator.contains(&MAX_REPORT_CHARS.to_string()),
        "evaluator 形态含 report 上限 2000: {evaluator}"
    );

    assert!(!decision.is_empty(), "decision 形态描述非空");
    for action in ["backtrack", "stop"] {
        assert!(
            decision.contains(action),
            "decision 形态含两动作名 {action}: {decision}"
        );
    }
    assert!(decision.contains("白名单"), "decision 形态含白名单语义");
    // 指路文案（AC-12）：含工具名字面 + 无法裁决语义；不含 ask 动作词形态
    assert!(
        decision.contains("AskUserQuestion"),
        "decision 形态指路 ask 工具（工具名字面）: {decision}"
    );
    assert!(
        decision.contains("无法裁决"),
        "decision 形态含「无法裁决」语义: {decision}"
    );
    for forbidden in ["ask（", "/ ask"] {
        assert!(
            !decision.contains(forbidden),
            "decision 形态不含三动作枚举式表述 {forbidden}: {decision}"
        );
    }
    // 形状不复述：形状归工具 schema
    for forbidden in ["minItems", "properties", "required"] {
        assert!(
            !decision.contains(forbidden),
            "decision 形态不复述 JSON Schema 词 {forbidden}"
        );
    }

    assert_ne!(evaluator, decision, "两形态可辨（档位不混）");
    for (label, text) in [("evaluator", evaluator), ("decision", decision)] {
        assert!(
            text.chars().count() < 500,
            "{label} 形态描述 <500 字符（无 prompt 膨胀），实际 {}",
            text.chars().count()
        );
    }
}

// ---------------------------------------------------------------------------
// 正向：parameters evaluator 臂 enum 逐值对拍（8 份语料）
// ---------------------------------------------------------------------------

#[test]
fn parameters的evaluator臂enum逐值对拍八份语料() {
    let prompts = evaluator_prompts();
    assert_eq!(prompts.len(), 8, "8 份 evaluator 语料去重后不重复");

    // 逐份相位 ID 序（只断总数挡不住 A 少一项 B 多一项）
    let expected_ids: [(&str, &[&str]); 8] = [
        (
            "proposal",
            &[
                "R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9", "R10", "R11",
            ],
        ),
        (
            "dev-design",
            &["D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10"],
        ),
        (
            "test-design",
            &["T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9"],
        ),
        ("implement", &["I1", "I2", "I3", "I4", "I5", "I6", "I7"]),
        (
            "test-gen",
            &["G1", "G2", "G3", "G4", "G5", "G6", "G7", "G8"],
        ),
        (
            "code-review",
            &["C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8", "C9", "C10"],
        ),
        (
            "code-analyze",
            &["CA1", "CA2", "CA3", "CA4", "CA5", "CA6", "CA7", "CA8"],
        ),
        ("acceptance", &["A1", "A2"]),
    ];
    for (phase, ids) in expected_ids {
        let prompt = evaluator_prompt_of(phase);
        let actual: Vec<String> = workflow::model::checklist_entries(prompt)
            .into_iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(actual, ids, "相位 {phase} 的静态清单 ID 序逐值且有序");
    }

    for prompt in prompts {
        let expected: Vec<String> = workflow::model::checklist_entries(prompt)
            .into_iter()
            .map(|entry| entry.id)
            .collect();
        assert!(!expected.is_empty(), "语料静态清单非空");

        let schema = parameters(FinishSpec::EvaluatorChecklist, prompt);
        let ids = schema["properties"]["items"]["items"]["properties"]["id"]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("enum 数组在场，实际: {schema}"));
        let actual: Vec<&str> = ids
            .iter()
            .map(|value| value.as_str().expect("enum 元素为字符串"))
            .collect();
        assert_eq!(
            actual,
            expected.iter().map(String::as_str).collect::<Vec<_>>(),
            "enum 逐值有序等于 prompt 静态清单 ID 序"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：parameters evaluator 臂结构
// ---------------------------------------------------------------------------

#[test]
fn parameters的evaluator臂结构六断言() {
    let schema = parameters(FinishSpec::EvaluatorChecklist, &synthetic_prompt());
    let properties = &schema["properties"];

    assert_eq!(
        properties["report"]["type"], "string",
        "report 为字符串字段"
    );
    assert_eq!(properties["items"]["type"], "array", "items 为数组");
    assert_eq!(properties["items"]["minItems"], 1, "items 至少一项");
    assert_eq!(
        properties["items"]["items"]["required"],
        serde_json::json!(["id", "pass", "evidence"]),
        "逐项三字段必填"
    );
    assert_eq!(
        properties["items"]["items"]["properties"]["pass"]["type"], "boolean",
        "pass 为布尔"
    );
    assert_eq!(
        schema["required"],
        serde_json::json!(["report", "items"]),
        "顶层两字段必填"
    );
    assert!(
        properties.get("verdict").is_none(),
        "verdict 键零命中（模型不可提交 verdict）: {schema}"
    );
}

#[test]
fn parameters的evaluator臂退化面_空id集省略enum键() {
    for prompt in [
        "",
        "## 静态清单\n\n| ID | 检查项 | 判断依据 |\n|---|---|---|\n",
    ] {
        let schema = parameters(FinishSpec::EvaluatorChecklist, prompt);
        let id_schema = &schema["properties"]["items"]["items"]["properties"]["id"];
        assert!(
            id_schema.get("enum").is_none(),
            "ID 集为空时省略 enum 键（空数组是非法 JSON Schema 形态），实际: {id_schema}"
        );
        assert_eq!(id_schema["type"], "string", "其余结构仍在");
        assert_eq!(schema["required"], serde_json::json!(["report", "items"]));
    }
}

// ---------------------------------------------------------------------------
// 正向：parameters decision 臂
// ---------------------------------------------------------------------------

#[test]
fn parameters的decision臂两动作形状且prompt零影响() {
    let schema = parameters(FinishSpec::Decision, "");
    let properties = &schema["properties"];
    let actions: Vec<&str> = properties["action"]["enum"]
        .as_array()
        .expect("action 枚举为数组")
        .iter()
        .map(|value| value.as_str().expect("枚举元素为字符串"))
        .collect();
    assert_eq!(
        actions,
        vec!["backtrack", "stop"],
        "两动作封闭集且有序（ask 已迁出为独立工具）"
    );
    assert_eq!(actions.len(), 2, "元素个数恰 2");
    assert!(!actions.contains(&"ask"), "集合内不含 ask");

    // 保留字段面：to / reason 仍在；新增必填零扩
    for field in ["to", "reason"] {
        assert!(
            properties.get(field).is_some(),
            "保留字段 {field} 在 schema 内声明"
        );
    }
    assert_eq!(schema["required"], serde_json::json!(["action"]));
    assert_eq!(properties["action"]["type"], "string", "action 为字符串");

    // prompt 入参对 decision 臂零影响（不依赖 ID 集）
    let first = parameters(FinishSpec::Decision, &synthetic_prompt());
    let second = parameters(FinishSpec::Decision, "完全不同的 prompt 文本 🚀");
    assert_eq!(first, second, "decision 臂与 prompt 无关");
}

/// 删面零残留（异常，AC-4）：decision 形态的 `properties` 键集**恰**
/// `{action, to, reason}`（`question` / `options` 零命中）。
#[test]
fn parameters的decision臂键集恰三项_question与options零命中() {
    let schema = parameters(FinishSpec::Decision, "");
    let mut keys: Vec<&str> = schema["properties"]
        .as_object()
        .expect("properties 对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["action", "reason", "to"],
        "键集恰三项（question / options 已删除）"
    );
    let text = serde_json::to_string(&schema).expect("序列化");
    for removed in ["question", "options"] {
        assert!(
            !text.contains(removed),
            "decision schema 全文零 {removed} 命中"
        );
    }
}

/// 两形态互不渗透（边界，参数化 2 形态）：decision 形态全文零 `report` /
/// `items` / `evidence`；evaluator 形态全文零 `action` / `backtrack` / `stop` /
/// `to` 键。
#[test]
fn parameters两形态互不渗透() {
    let decision_text =
        serde_json::to_string(&parameters(FinishSpec::Decision, "")).expect("序列化");
    for forbidden in ["report", "items", "evidence"] {
        assert!(
            !decision_text.contains(forbidden),
            "decision 形态零 evaluator 词汇 {forbidden}"
        );
    }

    let prompt = synthetic_prompt();
    let evaluator_text =
        serde_json::to_string(&parameters(FinishSpec::EvaluatorChecklist, &prompt))
            .expect("序列化");
    for forbidden in ["action", "backtrack", "stop", "\"to\""] {
        assert!(
            !evaluator_text.contains(forbidden),
            "evaluator 形态零 decision 词汇 {forbidden}"
        );
    }
}

/// `description` 体量与稳定性（边界）：两形态各 <500 字符；`description(Decision)`
/// 连调两次逐字相等（`&'static str`，零动态段）。
#[test]
fn description两形态体量受控且decision稳定() {
    for (label, spec) in [
        ("evaluator", FinishSpec::EvaluatorChecklist),
        ("decision", FinishSpec::Decision),
    ] {
        let text = description(spec);
        assert!(
            text.chars().count() < 500,
            "{label} 形态描述 <500 字符，实际 {}",
            text.chars().count()
        );
    }
    assert_eq!(
        description(FinishSpec::Decision),
        description(FinishSpec::Decision),
        "decision 描述连调两次逐字相等"
    );
}

/// `ask` 不再产出但历史兼容（异常，AC-12）：`{"action":"ask", …}` → `validate`
/// 仍可由 `validate_decision` 的 `"ask"` 臂通过（历史 notice 兼容），而
/// `parameters` 的 enum 不含 `"ask"` ⇒ 模型侧不可见（schema 拒绝是契约面、
/// validate 的兼容是历史面，两者并存且不矛盾）。
#[test]
fn ask动作schema不可见但validate历史兼容() {
    let args = serde_json::json!({
        "action": "ask",
        "question": "q",
        "options": ["a"],
    });
    let payload = validate(FinishSpec::Decision, &args, "")
        .expect("历史 ask 载荷仍可经 validate_decision 通过");
    assert!(
        matches!(
            &payload,
            FinishPayload::Decision(DecisionAction::Ask { question, options })
                if question == "q" && options == &vec!["a".to_owned()]
        ),
        "ask 臂逐字段承接，实际: {payload:?}"
    );

    let enum_values = parameters(FinishSpec::Decision, "")["properties"]["action"]["enum"]
        .as_array()
        .expect("枚举数组")
        .clone();
    assert!(
        !enum_values.contains(&serde_json::json!("ask")),
        "parameters 的 enum 不含 ask（模型侧不可见）"
    );
}

/// 解耦反向半边（异常，源码自证，AC-4）：`finish.rs` 的 `ask` 字样只出现在
/// **字符串字面量与注释**中（剔除后代码行零 `ask`）；且全文零 `use crate::ask`
/// / `crate::ask::` 路径（两模块互不引用）。
#[test]
fn finish_rs的ask只在文案与注释里() {
    let own = source_of("src/finish.rs");
    assert!(
        own.contains("AskUserQuestion"),
        "description 里以文案指路 ask 工具（工具名出现在字符串内）"
    );

    // 剔除注释（含 doc 注释）与字符串字面量（含跨行续行）后，代码零 `ask` 字样
    let mut code = String::new();
    let mut chars = own.chars().peekable();
    let mut in_string = false;
    let mut in_comment = false;
    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == NEWLINE {
                in_comment = false;
                code.push(NEWLINE);
            }
            continue;
        }
        if in_string {
            if ch == BACKSLASH {
                chars.next();
            } else if ch == QUOTE {
                in_string = false;
            }
            continue;
        }
        match ch {
            QUOTE => in_string = true,
            SLASH if chars.peek() == Some(&SLASH) => {
                in_comment = true;
                chars.next();
            }
            _ => code.push(ch),
        }
    }
    assert!(
        !code.to_lowercase().contains("ask"),
        "finish.rs 代码（剔除注释与字符串）零 ask 字样"
    );
    // 两模块零交叉引用
    for forbidden in ["use crate::ask", "crate::ask::"] {
        assert!(
            !own.contains(forbidden),
            "finish.rs 零 {forbidden}（与 ask 模块互不引用）"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：幂等与确定性
// ---------------------------------------------------------------------------

#[test]
fn parameters幂等且键序稳定() {
    let prompt = synthetic_prompt();
    let first = parameters(FinishSpec::EvaluatorChecklist, &prompt);
    let second = parameters(FinishSpec::EvaluatorChecklist, &prompt);
    assert_eq!(first, second, "同入参两次调用逐字段相等（零内部状态）");
    assert_eq!(
        serde_json::to_string(&first).expect("序列化"),
        serde_json::to_string(&second).expect("序列化"),
        "键序稳定（serde_json::Map 保序，可逐字 diff）"
    );
}

// ---------------------------------------------------------------------------
// 正向：validate 两臂分派
// ---------------------------------------------------------------------------

#[test]
fn validate分派evaluator臂_verdict由推导且item取md正文() {
    let prompt = synthetic_prompt();
    let payload = validate(
        FinishSpec::EvaluatorChecklist,
        &valid_checklist_args(),
        &prompt,
    )
    .expect("合法参数应通过");
    let FinishPayload::Evaluator(checklist) = &payload else {
        panic!("分派到 evaluator 臂，实际: {payload:?}");
    };
    assert_eq!(
        checklist.verdict,
        Verdict::Fail,
        "含 fail 项 → verdict Fail（由 derive_verdict 推导）"
    );
    assert_eq!(checklist.checklist.len(), 2);
    assert_eq!(
        checklist.checklist[0].item, "变更代码中无安全漏洞",
        "item 映射为 md 第二列正文（非 ID）"
    );
    assert_eq!(checklist.report, "评估报告正文", "report 原文保真");

    // 外层 tag 为 camelCase 外部 tag（分派命中的臂可辨）
    let value = serde_json::to_value(&payload).expect("序列化");
    assert!(
        value.get("evaluator").is_some(),
        "线形态外层 tag 为 evaluator，实际: {value}"
    );
}

#[test]
fn validate分派decision臂_外层tag为decision() {
    let args = serde_json::json!({ "action": "stop", "reason": "受控终止" });
    let payload = validate(FinishSpec::Decision, &args, "").expect("合法决策参数应通过");
    let FinishPayload::Decision(DecisionAction::Stop { reason }) = &payload else {
        panic!("分派到 decision / Stop 臂，实际: {payload:?}");
    };
    assert_eq!(reason, "受控终止");
    let value = serde_json::to_value(&payload).expect("序列化");
    assert!(
        value.get("decision").is_some(),
        "线形态外层 tag 为 decision，实际: {value}"
    );
}

/// 两动作各可通过（正向，`allowed` 列表逐项）：`backtrack` 的白名单内 `to`
/// 逐项 `Ok` 且 `to` 逐字回读；`stop` 形态并存（`validate_decision` 逐字未改的
/// 路径证据）。
///
/// 白名单来源 = 相位表 `backtrack_options`（路由白名单单一来源），不手写相位名
/// 清单——相位表变更时本用例自动跟随。
#[test]
fn validate分派decision臂两动作各可通过_白名单to逐项回读() {
    let table = phase_table("requirement").expect("相位表键合法");
    let allowed = backtrack_options(table, "test-gen");
    assert!(!allowed.is_empty(), "test-gen 的回溯白名单非空");

    for option in &allowed {
        let args = serde_json::json!({
            "action": "backtrack",
            "to": option.phase,
            "reason": "重做本相位",
        });
        let payload = validate(FinishSpec::Decision, &args, "").expect("白名单内 to 应通过");
        assert!(
            matches!(
                &payload,
                FinishPayload::Decision(DecisionAction::Backtrack { to, reason })
                    if to == &option.phase && reason == "重做本相位"
            ),
            "backtrack 逐字段回读（to = {}），实际: {payload:?}",
            option.phase
        );
        // 白名单闸门同向放行（越权面由 decision_test / walker_test 锁定）
        ensure_backtrack_allowed(
            &DecisionAction::Backtrack {
                to: option.phase.clone(),
                reason: "重做本相位".to_owned(),
            },
            "test-gen",
            &allowed,
        )
        .expect("白名单内目标恒放行");
    }

    let stop = validate(
        FinishSpec::Decision,
        &serde_json::json!({ "action": "stop", "reason": "受控终止" }),
        "",
    )
    .expect("stop 应通过");
    assert!(
        matches!(
            &stop,
            FinishPayload::Decision(DecisionAction::Stop { reason }) if reason == "受控终止"
        ),
        "stop 逐字段回读，实际: {stop:?}"
    );
}

#[test]
fn validate两臂err透传且文案可归因() {
    let prompt = synthetic_prompt();

    // evaluator 臂：缺 report → Err 含字段名 report
    let evaluator_err = validate(
        FinishSpec::EvaluatorChecklist,
        &serde_json::json!({ "items": [{ "id": "C1", "pass": true, "evidence": "e" }] }),
        &prompt,
    )
    .expect_err("缺 report 应 Err");
    assert!(
        evaluator_err.contains("report"),
        "evaluator 臂 Err 含字段名: {evaluator_err}"
    );

    // decision 臂：非法 action → Err 含封闭集三值
    let decision_err = validate(
        FinishSpec::Decision,
        &serde_json::json!({ "action": "unknown" }),
        "",
    )
    .expect_err("非法 action 应 Err");
    for action in ["backtrack", "stop", "ask"] {
        assert!(
            decision_err.contains(action),
            "decision 臂 Err 含封闭集值 {action}: {decision_err}"
        );
    }

    // 两臂文案互不为前缀（可归因到臂）
    assert!(
        !evaluator_err.starts_with(&decision_err) && !decision_err.starts_with(&evaluator_err),
        "两臂 Err 文案互不为前缀"
    );
}

// ---------------------------------------------------------------------------
// 边界：FinishPayload 线形态与派生面
// ---------------------------------------------------------------------------

#[test]
fn finish_payload线形态两线值三非法且往返等值() {
    let evaluator = EvaluatorChecklist {
        verdict: Verdict::Pass,
        report: "报告".to_owned(),
        checklist: vec![workflow::model::ChecklistItem {
            item: "项".to_owned(),
            pass: true,
            evidence: "据".to_owned(),
        }],
    };
    let payload = FinishPayload::Evaluator(evaluator.clone());
    let value = serde_json::to_value(&payload).expect("序列化");
    assert_eq!(
        value["evaluator"]["verdict"], "pass",
        "外层 tag evaluator + 内层 verdict 线值小写"
    );

    let back: FinishPayload = serde_json::from_value(value).expect("反序列化");
    assert_eq!(back, payload, "evaluator 臂双向 round-trip 逐字段相等");

    let decision = FinishPayload::Decision(DecisionAction::Ask {
        question: "选哪个？".to_owned(),
        options: vec!["甲".to_owned(), "乙".to_owned()],
    });
    let value = serde_json::to_value(&decision).expect("序列化");
    assert_eq!(value["decision"]["action"], "ask", "decision 臂线形态");
    let back: FinishPayload = serde_json::from_value(value).expect("反序列化");
    assert_eq!(back, decision, "decision 臂双向 round-trip 逐字段相等");

    // 三非法线值 → Err
    for invalid in [
        serde_json::json!({ "finish": {} }),
        serde_json::json!({}),
        serde_json::Value::Null,
    ] {
        assert!(
            serde_json::from_value::<FinishPayload>(invalid.clone()).is_err(),
            "非法线值 {invalid} 应 Err"
        );
    }

    // 派生面锚定：Debug / Clone / PartialEq / Eq
    let cloned = payload.clone();
    assert_eq!(cloned, payload, "Clone + PartialEq 等值");
    assert!(!format!("{payload:?}").is_empty(), "Debug 非空");

    // 源码自证：收口词汇不出线（零 specta::Type）
    let own = source_of("src/finish.rs");
    assert!(
        !own.contains("specta::Type"),
        "finish.rs 内零 specta::Type（FinishPayload 不出线）"
    );
}
