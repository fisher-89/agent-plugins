//! `finish.rs`（`finish_step` 工具协议单点）的单元测试：工具名单点、两形态
//! description / parameters、`validate` 的臂分派与 `FinishPayload` 线形态。
//!
//! 不 mock：`checklist_entries` / `validate_checklist` / `validate_decision` /
//! `derive_verdict` 全部真实组合（分派语义即被测对象，替身会掩盖臂错配）；
//! prompt 语料取自 `workflow::write::phase_table` 的 `include_str!` 内嵌文本
//!（编译期，零文件 IO）。

use agent::FinishSpec;
use workflow::model::Verdict;
use workflow::write::phase_table;

use crate::decision::DecisionAction;
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
    for action in ["backtrack", "stop", "ask"] {
        assert!(
            decision.contains(action),
            "decision 形态含动作名 {action}: {decision}"
        );
    }
    assert!(decision.contains("白名单"), "decision 形态含白名单语义");

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
            &[
                "C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8", "C9", "C10",
            ],
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
fn parameters的decision臂三动作形状且prompt零影响() {
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
        vec!["backtrack", "stop", "ask"],
        "三动作封闭集且有序"
    );
    for field in ["to", "reason", "question", "options"] {
        assert!(
            properties.get(field).is_some(),
            "三动作字段 {field} 在 schema 内声明"
        );
    }
    assert_eq!(schema["required"], serde_json::json!(["action"]));

    // prompt 入参对 decision 臂零影响（不依赖 ID 集）
    let first = parameters(FinishSpec::Decision, &synthetic_prompt());
    let second = parameters(FinishSpec::Decision, "完全不同的 prompt 文本 🚀");
    assert_eq!(first, second, "decision 臂与 prompt 无关");
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
