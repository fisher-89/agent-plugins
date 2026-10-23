use workflow::model::Verdict;
use workflow::write::BacktrackOption;

use crate::decision::{
    ensure_backtrack_allowed, parse_decision, CandidateReport, DecisionAction, DecisionInput,
    MAX_REASON_CHARS,
};

/// 白名单项 fixture（id-only；描述缺省空串——预校验只看 id）。
fn allowed_phase(phase: &str) -> BacktrackOption {
    BacktrackOption {
        phase: phase.to_owned(),
        description: String::new(),
    }
}

/// 三动作合法解析（正向，表驱动三变体）：`backtrack`（含 to + reason）/
/// `stop` / `ask`（含 options 列表）逐变体解析成功。
#[test]
fn 三动作合法解析封闭集逐变体() {
    let backtrack = parse_decision(
        r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计缺产物区组件" }"#,
    )
    .expect("backtrack 应解析");
    assert_eq!(
        backtrack,
        DecisionAction::Backtrack {
            to: "dev-design".to_owned(),
            reason: "设计缺产物区组件".to_owned(),
        },
        "封闭集 backtrack 变体"
    );

    let stop = parse_decision(r#"{ "action": "stop", "reason": "需求基线漂移，人工介入" }"#)
        .expect("stop 应解析");
    assert_eq!(
        stop,
        DecisionAction::Stop {
            reason: "需求基线漂移，人工介入".to_owned(),
        }
    );

    let ask = parse_decision(
        r#"{ "action": "ask", "question": "回溯目标选哪个?", "options": ["proposal", "dev-design"] }"#,
    )
    .expect("ask 应解析");
    assert_eq!(
        ask,
        DecisionAction::Ask {
            question: "回溯目标选哪个?".to_owned(),
            options: vec!["proposal".to_owned(), "dev-design".to_owned()],
        }
    );

    // 三变体两两可辨（值域恰三态）
    assert_ne!(backtrack, stop);
    assert_ne!(stop, ask);
    assert_ne!(backtrack, ask);
}

/// 围栏代码块与混杂叙述容忍提取（正向）：与 verdict 同款载体提取——```json
/// 围栏包裹与前后混杂叙述均解析成功。
#[test]
fn 围栏代码块与混杂叙述容忍提取解析成功() {
    let json = r#"{ "action": "stop", "reason": "任意" }"#;
    let fenced = format!("```json\n{json}\n```");
    assert_eq!(
        parse_decision(&fenced).expect("围栏包裹应解析"),
        DecisionAction::Stop {
            reason: "任意".to_owned(),
        },
        "与 verdict 同款围栏容忍"
    );

    let noisy = format!("无法在白名单内裁决。\n\n```json\n{json}\n```\n（以上）");
    assert_eq!(
        parse_decision(&noisy).expect("混杂叙述应解析"),
        DecisionAction::Stop {
            reason: "任意".to_owned(),
        }
    );
}

/// 已退役动作（异常）：`{"action":"retry"}` 走 serde 结构漂移 `Err`——不产出
/// 变体（软拒环由此计数，不一次打死 run）。
#[test]
fn 已退役retry动作解析err() {
    let err = parse_decision(r#"{ "action": "retry" }"#).expect_err("retry 已退役应 Err");
    assert!(
        err.contains("结构漂移") || err.contains("JSON"),
        "结构漂移记因: {err}"
    );
    // 携载荷形态同样拒绝（不静默降级为任一在役变体）
    assert!(
        parse_decision(r#"{ "action": "retry", "reason": "再试一次" }"#).is_err(),
        "retry 携 reason 同拒"
    );
}

/// 非法结构（异常，参数化 5 值）：未知名 / 拼写漂移 / 缺 action / 空串 /
/// 纯空白 → 均 `Err`（封闭集防线不被放宽）。
#[test]
fn 非法结构五形态解析err() {
    for illegal in [
        r#"{ "action": "explode" }"#,
        r#"{ "action": "Retry" }"#,
        r#"{ "reason": "缺 action" }"#,
        "",
        "   \n\t ",
    ] {
        let outcome = parse_decision(illegal);
        assert!(outcome.is_err(), "非法结构 {illegal:?} 应 Err");
    }
}

#[test]
fn reason超长解析层原样承接不崩() {
    for len in [MAX_REASON_CHARS + 1, 2000] {
        let reason = "因".repeat(len);
        let json =
            format!(r#"{{ "action": "backtrack", "to": "proposal", "reason": "{reason}" }}"#);
        let action = parse_decision(&json).expect("解析层透传超长 reason，不设长度门");
        assert_eq!(
            action,
            DecisionAction::Backtrack {
                to: "proposal".to_owned(),
                reason: reason.clone(),
            },
            "reason {len} 字符原样承接（逐字符保真）"
        );

        // stop 动作的 reason 同为透传（解析层不区分动作设门）
        let json = format!(r#"{{ "action": "stop", "reason": "{reason}" }}"#);
        assert_eq!(
            parse_decision(&json).expect("stop reason 同样透传"),
            DecisionAction::Stop { reason }
        );
    }
}

/// 字段缺失 / 冗余（边界，表驱动 3 形态）：backtrack 缺 `reason` → `Err`；
/// `to` 为空串 → 解析成功（交给门判定，解析层不校验内容）；含多余字段 →
/// 解析成功（serde 宽容未知键）。
#[test]
fn 字段缺失与冗余三形态() {
    // 缺 reason → 结构漂移
    assert!(
        parse_decision(r#"{ "action": "backtrack", "to": "proposal" }"#).is_err(),
        "backtrack 缺 reason 应 Err"
    );
    // 缺 to → 结构漂移
    assert!(
        parse_decision(r#"{ "action": "backtrack", "reason": "任意" }"#).is_err(),
        "backtrack 缺 to 应 Err"
    );

    // to 空串 → 解析成功（内容面由门判定）
    assert_eq!(
        parse_decision(r#"{ "action": "backtrack", "to": "", "reason": "任意" }"#)
            .expect("空 to 解析层放行"),
        DecisionAction::Backtrack {
            to: String::new(),
            reason: "任意".to_owned(),
        }
    );

    // 多余字段 → 解析成功（未知键不拒）
    assert_eq!(
        parse_decision(
            r#"{ "action": "stop", "reason": "停", "confidence": 0.9, "extra": [1, 2] }"#
        )
        .expect("冗余字段应容忍"),
        DecisionAction::Stop {
            reason: "停".to_owned(),
        }
    );
}

/// 白名单命中放行（正向，表驱动 3 动作）：`to` 在白名单 → `Ok`；`Stop` /
/// `Ask` 动作恒 `Ok`（不看白名单）。
#[test]
fn 白名单命中放行_非backtrack恒ok() {
    let allowed = vec![
        allowed_phase("proposal"),
        allowed_phase("dev-design"),
        allowed_phase("implement"),
    ];
    let backtrack = DecisionAction::Backtrack {
        to: "dev-design".to_owned(),
        reason: "设计返工".to_owned(),
    };
    assert!(
        ensure_backtrack_allowed(&backtrack, "implement", &allowed).is_ok(),
        "命中白名单放行（写面 backtrack 二次校验仍兜底——双层防线锚定）"
    );

    let stop = DecisionAction::Stop {
        reason: "停".to_owned(),
    };
    assert!(
        ensure_backtrack_allowed(&stop, "implement", &[]).is_ok(),
        "stop 恒通过（空白名单亦不拦）"
    );

    let ask = DecisionAction::Ask {
        question: "选哪个?".to_owned(),
        options: vec!["proposal".to_owned()],
    };
    assert!(
        ensure_backtrack_allowed(&ask, "test-execution", &[]).is_ok(),
        "ask 恒通过（不看白名单）"
    );
}

/// 自环（正向 / 异常面）：`to == failed_phase` 且不在白名单 → `Err`，文案含
/// 「确定性门禁相位（零 agent 步）」与 `stop` / `ask` 出路。
#[test]
fn 自环非可重做相位backtrack被拒携出路() {
    let action = DecisionAction::Backtrack {
        to: "test-execution".to_owned(),
        reason: "重跑测试".to_owned(),
    };
    let allowed = vec![allowed_phase("proposal"), allowed_phase("test-gen")];
    let err = ensure_backtrack_allowed(&action, "test-execution", &allowed)
        .expect_err("零 agent 步自环应 Err");
    assert!(
        err.contains("确定性门禁相位") && err.contains("零 agent 步"),
        "自环文案显式说明不可重做: {err}"
    );
    assert!(
        err.contains("stop") && err.contains("ask"),
        "自环文案携两条出路（stop / ask）: {err}"
    );
}

/// 越权（异常）：`to` 在白名单外且 ≠ `failed_phase` → `Err`，文案携 `allowed`
/// 列表原文。
#[test]
fn 越权backtrack预校验err携白名单原文() {
    let action = DecisionAction::Backtrack {
        to: "test-gen".to_owned(),
        reason: "任意".to_owned(),
    };
    let allowed = vec![allowed_phase("proposal"), allowed_phase("dev-design")];
    let err = ensure_backtrack_allowed(&action, "implement", &allowed).expect_err("越权应 Err");
    assert!(
        err.contains("test-gen") && err.contains("越权"),
        "错误显式携带越权目标，实际: {err}"
    );
    assert!(
        err.contains("proposal") && err.contains("dev-design"),
        "文案携 allowed 列表原文: {err}"
    );
}

/// 空白名单（异常，表驱动 2 例）：`allowed = []` 时 backtrack 任意目标 →
/// `Err`（自环与越权两类文案各一例，无静默放行出口）。
#[test]
fn 空白名单任何backtrack均err无跳转出口() {
    // 越权类文案（to ≠ failed_phase）
    let action = DecisionAction::Backtrack {
        to: "proposal".to_owned(),
        reason: "任意".to_owned(),
    };
    let err = ensure_backtrack_allowed(&action, "implement", &[]).expect_err("空白名单应 Err");
    assert!(err.contains("越权") || err.contains("白名单"), "越权记因: {err}");

    // 自环类文案（to == failed_phase 且空白名单）
    let action = DecisionAction::Backtrack {
        to: "implement".to_owned(),
        reason: "任意".to_owned(),
    };
    let err = ensure_backtrack_allowed(&action, "implement", &[]).expect_err("空白名单自环应 Err");
    assert!(
        err.contains("确定性门禁相位") || err.contains("不支持重做"),
        "自环记因: {err}"
    );
}

/// 白名单自含即重做（边界）：`to == failed_phase` 且在白名单（可重做相位）→
/// `Ok`（白名单自含 = 重做本相位，零新字段）。
#[test]
fn 白名单自含即重做放行() {
    let action = DecisionAction::Backtrack {
        to: "implement".to_owned(),
        reason: "重做实现".to_owned(),
    };
    let allowed = vec![
        allowed_phase("proposal"),
        allowed_phase("dev-design"),
        allowed_phase("implement"),
    ];
    assert!(
        ensure_backtrack_allowed(&action, "implement", &allowed).is_ok(),
        "可重做相位自含 → 重做本相位合法"
    );
}

/// 匹配边界（边界，参数化 4 形态）：`allowed` 含重复项 / 50 项超大列表 /
/// `failed_phase` 空串 / 大小写差异 → 判定不误放行。
#[test]
fn 白名单匹配边界四形态() {
    // 形态一：allowed 含重复项（命中即放行，重复不误拒）
    let action = DecisionAction::Backtrack {
        to: "proposal".to_owned(),
        reason: "任意".to_owned(),
    };
    let allowed = vec![
        allowed_phase("proposal"),
        allowed_phase("proposal"),
        allowed_phase("dev-design"),
    ];
    assert!(
        ensure_backtrack_allowed(&action, "implement", &allowed).is_ok(),
        "重复项命中放行"
    );

    // 形态二：50 项超大列表（末项命中）
    let mut allowed: Vec<BacktrackOption> = (0..49)
        .map(|index| allowed_phase(&format!("phase-{index}")))
        .collect();
    allowed.push(allowed_phase("implement"));
    let action = DecisionAction::Backtrack {
        to: "implement".to_owned(),
        reason: "任意".to_owned(),
    };
    assert!(
        ensure_backtrack_allowed(&action, "implement", &allowed).is_ok(),
        "50 项列表末项命中放行"
    );

    // 形态三：failed_phase 空串 + to 空串且不在白名单 → 自环文案（不静默放行）
    let action = DecisionAction::Backtrack {
        to: String::new(),
        reason: "任意".to_owned(),
    };
    let err = ensure_backtrack_allowed(&action, "", &[allowed_phase("proposal")])
        .expect_err("空串自环应 Err");
    assert!(err.contains("确定性门禁相位"), "空串按自环拒绝: {err}");

    // 形态四：大小写差异（"Implement" ≠ "implement"）
    let action = DecisionAction::Backtrack {
        to: "Implement".to_owned(),
        reason: "任意".to_owned(),
    };
    let allowed = vec![allowed_phase("implement")];
    assert!(
        ensure_backtrack_allowed(&action, "test-gen", &allowed).is_err(),
        "大小写差异不误放行"
    );
}

/// `DecisionInput.fail_report` 字段面（正向，参数化 3 形态）：空串 / 2000
/// 字符 / 含 emoji 与换行 → `PartialEq` 等值可辨、字段独立于 `candidates`。
#[test]
fn decision_input_fail_report字段面() {
    let base = DecisionInput {
        phase: "implement".to_owned(),
        attempt: 2,
        fail_checklist: Vec::new(),
        fail_report: String::new(),
        allowed: vec![allowed_phase("proposal")],
        candidates: vec![CandidateReport {
            phase: "proposal".to_owned(),
            verdict: Some(Verdict::Pass),
            report: Some("提案通过".to_owned()),
        }],
    };
    let shapes = [
        String::new(),
        "报".repeat(2000),
        "失败\n🚀 findings: a\0b".to_owned(),
    ];
    let mut variants: Vec<DecisionInput> = Vec::new();
    for fail_report in shapes {
        let input = DecisionInput {
            fail_report: fail_report.clone(),
            ..base.clone()
        };
        assert_eq!(input.fail_report, fail_report, "fail_report 逐字符保真");
        // 字段独立于 candidates：候选面零扰动
        assert_eq!(input.candidates, base.candidates);
        variants.push(input);
    }
    assert_ne!(variants[0], variants[1], "空串 / 2000 字符可辨");
    assert_ne!(variants[1], variants[2], "长文 / 特殊字符可辨");
    assert_eq!(variants[0].clone(), variants[0], "PartialEq 自反");
}

/// `DecisionAction` 封闭集边界（边界，参数化 4 形态）：`Backtrack` 的 `to` /
/// `reason` 取空串与 1200 字符超长值可构造且不 panic（长度门在写面与门层，
/// 不在类型层）。
#[test]
fn decision_action封闭集边界可构造() {
    let long = "x".repeat(1200);
    let cases = [
        DecisionAction::Backtrack {
            to: String::new(),
            reason: String::new(),
        },
        DecisionAction::Backtrack {
            to: long.clone(),
            reason: long.clone(),
        },
        DecisionAction::Stop {
            reason: long.clone(),
        },
        DecisionAction::Ask {
            question: long.clone(),
            options: vec![String::new(), long.clone()],
        },
    ];
    for (index, case) in cases.iter().enumerate() {
        let cloned = case.clone();
        assert_eq!(&cloned, case, "形态 {index} 可构造且自反");
    }
    // 空串与超长值两两可辨
    assert_ne!(cases[0], cases[1]);
}
