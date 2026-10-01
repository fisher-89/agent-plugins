//! `decision.rs`（决策解析与白名单预校验）的单元测试（test-design「decision.rs
//! -> decision_test.rs」节）：四动作封闭集解析（回归锚）、围栏容忍（回归锚）、
//! 越权 backtrack 预校验（walker 第一道闸）、空白名单出口、结构漂移显式失败、
//! reason 超长解析层透传（分层锚——≤500 拒绝单点归写面 backtrack）、
//! DecisionInput 有界组装（回归锚）。
//! 无进程边界依赖：决策文本与 DecisionInput / 白名单 fixture 内存构造——
//! Mock策略：无 mock。

use workflow::model::{ChecklistItem, Verdict};

use crate::decision::{
    ensure_backtrack_allowed, parse_decision, CandidateReport, DecisionAction, DecisionInput,
};
use crate::verdict::MAX_REPORT_CHARS;

#[test]
fn 四动作合法解析封闭集逐变体() {
    // backtrack { to, reason }
    let action = parse_decision(
        r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计缺产物区组件" }"#,
    )
    .expect("backtrack 应解析");
    assert_eq!(
        action,
        DecisionAction::Backtrack {
            to: "dev-design".to_owned(),
            reason: "设计缺产物区组件".to_owned(),
        },
        "封闭集 backtrack 变体"
    );

    // retry（无载荷）
    let action = parse_decision(r#"{ "action": "retry" }"#).expect("retry 应解析");
    assert_eq!(action, DecisionAction::Retry);

    // stop { reason }
    let action = parse_decision(r#"{ "action": "stop", "reason": "需求基线漂移，人工介入" }"#)
        .expect("stop 应解析");
    assert_eq!(
        action,
        DecisionAction::Stop {
            reason: "需求基线漂移，人工介入".to_owned(),
        }
    );

    // ask { question, options[] }
    let action = parse_decision(
        r#"{ "action": "ask", "question": "回溯目标选哪个?", "options": ["proposal", "dev-design"] }"#,
    )
    .expect("ask 应解析");
    assert_eq!(
        action,
        DecisionAction::Ask {
            question: "回溯目标选哪个?".to_owned(),
            options: vec!["proposal".to_owned(), "dev-design".to_owned()],
        }
    );
}

#[test]
fn 围栏代码块与混杂叙述容忍提取解析成功() {
    let json = r#"{ "action": "retry" }"#;
    let fenced = format!("```json\n{json}\n```");
    assert_eq!(
        parse_decision(&fenced).expect("围栏包裹应解析"),
        DecisionAction::Retry,
        "与 verdict 同款围栏容忍"
    );

    let noisy = format!("无法在白名单内裁决。\n\n```json\n{json}\n```\n（以上）");
    assert_eq!(
        parse_decision(&noisy).expect("混杂叙述应解析"),
        DecisionAction::Retry
    );
}

#[test]
fn 越权backtrack预校验err_walker第一道闸() {
    let action = DecisionAction::Backtrack {
        to: "test-gen".to_owned(),
        reason: "任意".to_owned(),
    };
    // 白名单 = 写面下发 Vec<String> 字面集
    let allowed = vec!["proposal".to_owned(), "dev-design".to_owned()];
    let err = ensure_backtrack_allowed(&action, &allowed).expect_err("越权应 Err");
    assert!(
        err.contains("test-gen") && err.contains("越权"),
        "错误显式携带越权目标，实际: {err}"
    );
}

#[test]
fn 白名单命中放行_写面二次校验仍兜底() {
    let action = DecisionAction::Backtrack {
        to: "dev-design".to_owned(),
        reason: "设计返工".to_owned(),
    };
    let allowed = vec!["proposal".to_owned(), "dev-design".to_owned()];
    assert!(
        ensure_backtrack_allowed(&action, &allowed).is_ok(),
        "命中白名单放行（写面 backtrack 二次校验仍兜底——双层防线锚定）"
    );

    // 非 backtrack 动作恒通过（预校验只看 backtrack 变体）
    assert!(ensure_backtrack_allowed(&DecisionAction::Retry, &[]).is_ok());
    assert!(ensure_backtrack_allowed(
        &DecisionAction::Stop {
            reason: "停".to_owned(),
        },
        &[],
    )
    .is_ok());
}

#[test]
fn 空白名单任何backtrack均err无跳转出口() {
    let action = DecisionAction::Backtrack {
        to: "proposal".to_owned(),
        reason: "任意".to_owned(),
    };
    let err = ensure_backtrack_allowed(&action, &[]).expect_err("空白名单应 Err");
    assert!(
        err.contains("越权") || err.contains("白名单"),
        "无跳转出口记因: {err}"
    );
}

#[test]
fn 结构漂移显式失败均err停给用户() {
    // 未知 action
    let err = parse_decision(r#"{ "action": "escalate", "reason": "任意" }"#)
        .expect_err("未知 action 应 Err");
    assert!(
        err.contains("结构漂移") || err.contains("JSON"),
        "记因: {err}"
    );

    // backtrack 缺 to
    let err =
        parse_decision(r#"{ "action": "backtrack", "reason": "任意" }"#).expect_err("缺 to 应 Err");
    assert!(
        err.contains("结构漂移") || err.contains("JSON"),
        "记因: {err}"
    );

    // backtrack 缺 reason
    let err = parse_decision(r#"{ "action": "backtrack", "to": "proposal" }"#)
        .expect_err("缺 reason 应 Err");
    assert!(
        err.contains("结构漂移") || err.contains("JSON"),
        "记因: {err}"
    );

    // options 非数组
    let err =
        parse_decision(r#"{ "action": "ask", "question": "选哪个?", "options": "dev-design" }"#)
            .expect_err("options 非数组应 Err");
    assert!(
        err.contains("结构漂移") || err.contains("JSON"),
        "记因: {err}"
    );

    // 非 JSON 文本 / 纯叙述
    assert!(
        parse_decision("直接重试吧。").is_err(),
        "非 JSON 文本显式失败"
    );
    assert!(parse_decision("").is_err(), "空消息显式失败");
}

/// reason 超长的解析层承接（分层锚）：>500 字符解析层原样承接不崩——超长
/// 拒绝由写面 backtrack 单点承载（≤500 约定的唯一拒绝点，backtrack_test
/// 「backtrack reason 超长拒绝」行为对端行；解析层不设第二道闸）。
#[test]
fn reason超长解析层原样承接不崩() {
    let long_reason = "因".repeat(MAX_REPORT_CHARS + 1);

    // backtrack reason >500：解析成功且逐字符保真（不崩、不拒、不截断）
    let json =
        format!(r#"{{ "action": "backtrack", "to": "proposal", "reason": "{long_reason}" }}"#);
    let action = parse_decision(&json).expect("解析层透传超长 reason，不设长度门");
    assert_eq!(
        action,
        DecisionAction::Backtrack {
            to: "proposal".to_owned(),
            reason: long_reason.clone(),
        },
        "超长 reason 原样承接"
    );

    // stop 动作的 reason 同为透传（解析层不区分动作设门）
    let json = format!(r#"{{ "action": "stop", "reason": "{long_reason}" }}"#);
    assert_eq!(
        parse_decision(&json).expect("stop reason 同样透传"),
        DecisionAction::Stop {
            reason: long_reason,
        }
    );

    // 恰 500 字符边界同样通过（解析层无长度概念）
    let exact = "因".repeat(MAX_REPORT_CHARS);
    let json = format!(r#"{{ "action": "backtrack", "to": "proposal", "reason": "{exact}" }}"#);
    assert!(parse_decision(&json).is_ok(), "恰 500 字符边界通过");
}

/// DecisionInput 有界组装（回归锚）：fail_checklist 空 / 多行、candidates
/// verdict / report 双 None（候选相位从未评估）形态构造合法；白名单已换
/// Vec<String>（决策 prompt 组装输入面）。
#[test]
fn decision_input有界组装边界形态构造合法() {
    // 空集形态
    let empty = DecisionInput {
        phase: "implement".to_owned(),
        attempt: 5,
        fail_checklist: Vec::new(),
        allowed: Vec::new(),
        candidates: Vec::new(),
    };
    assert_eq!(empty.phase, "implement");
    assert!(empty.fail_checklist.is_empty() && empty.allowed.is_empty());

    // 多行 fail checklist + 候选双 None 形态
    let full = DecisionInput {
        phase: "test-gen".to_owned(),
        attempt: 2,
        fail_checklist: vec![
            ChecklistItem {
                item: "覆盖 test-design 全行".to_owned(),
                pass: false,
                evidence: "缺失废弃行承接".to_owned(),
            },
            ChecklistItem {
                item: "中文描述".to_owned(),
                pass: true,
                evidence: "通过".to_owned(),
            },
        ],
        allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
        candidates: vec![CandidateReport {
            phase: "proposal".to_owned(),
            verdict: Some(Verdict::Pass),
            report: Some("提案通过".to_owned()),
        }],
    };
    assert_eq!(full.fail_checklist.len(), 2);
    assert_eq!(
        full.allowed,
        vec!["proposal".to_owned(), "dev-design".to_owned()]
    );
    assert_eq!(full.candidates[0].verdict, Some(Verdict::Pass));

    // 候选相位从未评估：verdict / report 双 None 构造合法
    let unevaluated = CandidateReport {
        phase: "dev-design".to_owned(),
        verdict: None,
        report: None,
    };
    assert!(unevaluated.verdict.is_none() && unevaluated.report.is_none());
}
