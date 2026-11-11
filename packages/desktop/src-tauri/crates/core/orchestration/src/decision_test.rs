use workflow::model::Verdict;
use workflow::write::{BacktrackOption, BacktrackWindowEntry};

use crate::decision::{
    ensure_backtrack_allowed, parse_decision, CandidateReport, DecisionAction, DecisionInput,
    MAX_REASON_CHARS,
};

/// 白名单项 fixture（id-only；描述缺省空串、产物清单空——预校验只看 id）。
fn allowed_phase(phase: &str) -> BacktrackOption {
    BacktrackOption {
        phase: phase.to_owned(),
        description: String::new(),
        artifacts: Vec::new(),
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
    assert!(
        err.contains("越权") || err.contains("白名单"),
        "越权记因: {err}"
    );

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
        backtrack_window: Vec::new(),
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

// ---------------------------------------------------------------------------
// `DecisionInput.backtrack_window` 字段面（透传面）：值原样取自写面 error
// 响应；walker MUST NOT 自读 store、MUST NOT 自相位表推导。`Vec` 非 `Option`
// ——空集即「无窗口」。行为面（透传与渲染）由 walker_test / prompt_test 覆盖，
// 本章节只锁字段面。
// ---------------------------------------------------------------------------

/// 窗口记录 fixture（字段值全部可辨，逐字段等值断言的事实源）。
fn window_entry(
    from: &str,
    from_index: usize,
    to: &str,
    to_index: usize,
    reason: &str,
) -> BacktrackWindowEntry {
    BacktrackWindowEntry {
        from_phase: from.to_owned(),
        from_index,
        to_phase: to.to_owned(),
        to_index,
        reason: reason.to_owned(),
    }
}

/// 全空窗口（结构面「五字段必给」的基线形态——`{}` 不可构造）。
fn empty_window() -> BacktrackWindowEntry {
    window_entry("", 0, "", 0, "")
}

/// 字段面基座（既有五面置可辨值，窗口面取入参）。
fn input_with_window(backtrack_window: Vec<BacktrackWindowEntry>) -> DecisionInput {
    DecisionInput {
        phase: "implement".to_owned(),
        attempt: 2,
        fail_checklist: Vec::new(),
        fail_report: "失败明细".to_owned(),
        allowed: vec![allowed_phase("proposal")],
        candidates: vec![CandidateReport {
            phase: "proposal".to_owned(),
            verdict: Some(Verdict::Pass),
            report: Some("提案通过".to_owned()),
        }],
        backtrack_window,
    }
}

/// 字段面保真（正向，单次，AC-10）：含 2 条窗口的 `DecisionInput` → 逐字段等值、
/// `PartialEq` 可辨、与既有五面并列且互不扰动。
#[test]
fn decision_input_backtrack_window字段面保真() {
    let windows = vec![
        window_entry("test-gen", 5, "dev-design", 2, "设计返工：缺产物区组件"),
        window_entry("implement", 4, "proposal", 1, "需求返工：目标偏离"),
    ];
    let input = input_with_window(windows.clone());

    assert_eq!(input.backtrack_window.len(), 2, "两条窗口随行");
    for (actual, expected) in input.backtrack_window.iter().zip(windows.iter()) {
        assert_eq!(actual.from_phase, expected.from_phase, "from_phase 逐字段");
        assert_eq!(actual.from_index, expected.from_index, "from_index 逐字段");
        assert_eq!(actual.to_phase, expected.to_phase, "to_phase 逐字段");
        assert_eq!(actual.to_index, expected.to_index, "to_index 逐字段");
        assert_eq!(actual.reason, expected.reason, "reason 逐字段");
        assert_eq!(actual, expected, "整条 PartialEq 等值");
    }

    // 与既有五面并列且互不扰动
    let without_window = input_with_window(Vec::new());
    assert_eq!(input.phase, without_window.phase, "phase 面零扰动");
    assert_eq!(input.attempt, without_window.attempt, "attempt 面零扰动");
    assert_eq!(
        input.fail_checklist, without_window.fail_checklist,
        "fail_checklist 面零扰动"
    );
    assert_eq!(
        input.fail_report, without_window.fail_report,
        "fail_report 面零扰动"
    );
    assert_eq!(input.allowed, without_window.allowed, "allowed 面零扰动");
    assert_eq!(
        input.candidates, without_window.candidates,
        "candidates 面零扰动"
    );
    assert_ne!(
        input, without_window,
        "窗口面独立参与 PartialEq（非空 / 空集可辨）"
    );
    assert_eq!(input.clone(), input, "PartialEq 自反");
}

/// spread 构造自动随动（正向，参数化迭代既有站点形态，AC-10 / R-9）：以
/// `..base` 覆写单面的既有 19 处构造形态编译并全绿——两处完整构造器与 `base`
/// 补字段后既有断言逐条不变（本行以 spread 形态自证随动）。
#[test]
fn decision_input_spread构造窗口面自动随动() {
    let base = input_with_window(vec![window_entry(
        "test-gen",
        5,
        "dev-design",
        2,
        "基座窗口",
    )]);
    let cases = [
        DecisionInput {
            phase: "dev-design".to_owned(),
            ..base.clone()
        },
        DecisionInput {
            attempt: 0,
            ..base.clone()
        },
        DecisionInput {
            fail_report: String::new(),
            ..base.clone()
        },
        DecisionInput {
            allowed: Vec::new(),
            ..base.clone()
        },
        DecisionInput {
            candidates: Vec::new(),
            ..base.clone()
        },
    ];
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(
            case.backtrack_window, base.backtrack_window,
            "形态 {index}：spread 构造保留窗口面（字段补齐自动随动）"
        );
    }
    // 覆写窗口面本身亦随动（新值生效、其余面继承）
    let overridden = DecisionInput {
        backtrack_window: Vec::new(),
        ..base.clone()
    };
    assert!(
        overridden.backtrack_window.is_empty(),
        "覆写窗口面生效（spread 后置字段优先）"
    );
    assert_eq!(overridden.phase, base.phase, "其余面继承不变");
}

/// 空集与非空集可辨（异常，参数化迭代 2 形态）：`assert_ne!` 双向；既有
/// `parse_decision` / `ensure_backtrack_allowed` 两函数语义零耦合（窗口面不参与
/// 判定——以既有判据在非空窗口输入下仍按白名单裁决承载）。
#[test]
fn decision_input_空集与非空集可辨且零语义耦合() {
    let empty = input_with_window(Vec::new());
    let filled = input_with_window(vec![window_entry("test-gen", 5, "dev-design", 2, "R")]);
    assert_ne!(empty, filled, "空集 / 非空集可辨（正向）");
    assert_ne!(filled, empty, "空集 / 非空集可辨（反向）");
    assert!(empty.backtrack_window.is_empty(), "空集即「无窗口」");
    assert_eq!(filled.backtrack_window.len(), 1, "非空集携条数");

    // 零语义耦合：窗口面与 decision 判定两函数无关——非空窗口输入下
    // parse_decision / ensure_backtrack_allowed 仍按各自契约裁决
    let action = parse_decision(r#"{ "action": "backtrack", "to": "proposal", "reason": "任意" }"#)
        .expect("解析不受窗口面影响");
    assert!(
        ensure_backtrack_allowed(&action, "implement", &filled.allowed).is_ok(),
        "白名单命中判据不受窗口面影响"
    );
    let denied = ensure_backtrack_allowed(&action, "implement", &[]).expect_err("空白名单应 Err");
    assert!(
        denied.contains("越权") || denied.contains("白名单"),
        "拒绝文案照常"
    );
    // 退役字段零残留（`eval_only` 等形态不存在：字段面恰为既有六面 + 窗口面）
    let DecisionInput {
        phase: _,
        attempt: _,
        fail_checklist: _,
        fail_report: _,
        allowed: _,
        candidates: _,
        backtrack_window: _,
    } = filled;
}

/// 字段值边界（边界，组合迭代 2 index × 3 reason）：`from_index` / `to_index`
/// 取 `0` 与 `usize::MAX`；`reason` 取 `""` / `MAX_REASON_CHARS` 长文（本 crate
/// 的 500，与写面 1000 是两个常量，本层不裁决长度）/ 特殊字符；`from_phase` /
/// `to_phase` 取 `""` 与 1200 字符超长串 → 逐字保真、不裁剪（截断归写面，本层
/// 纯数据载体）。
#[test]
fn decision_input_backtrack_window字段值边界() {
    let long = "字".repeat(MAX_REASON_CHARS);
    let reasons = [String::new(), long.clone(), "a\n\0🚀".to_owned()];
    for index in [0usize, usize::MAX] {
        for reason in &reasons {
            let entry = window_entry("", index, "", index, reason);
            let input = input_with_window(vec![entry.clone()]);
            let kept = &input.backtrack_window[0];
            assert_eq!(kept.from_index, index, "from_index = {index} 逐字保真");
            assert_eq!(kept.to_index, index, "to_index = {index} 逐字保真");
            assert_eq!(kept.reason, *reason, "reason 原样保真（不裁剪）");
            assert_eq!(kept, &entry, "整条等值（index {index}）");
        }
    }
    // 恰上限 vs 超长：均原样承接（本层零长度门）
    for reason in [long, "x".repeat(1200)] {
        let entry = window_entry("", 0, "", 0, &reason);
        let input = input_with_window(vec![entry]);
        assert_eq!(
            input.backtrack_window[0].reason.chars().count(),
            reason.chars().count(),
            "reason 字符数原样承接（{} 字符，零截断）",
            reason.chars().count()
        );
    }
    // 相位 id 取空串与超长串：本层是纯数据载体，逐字保真不裁剪（截断归写面）
    for phase in [String::new(), "p".repeat(1200)] {
        let entry = window_entry(&phase, 0, &phase, 0, "R");
        let input = input_with_window(vec![entry.clone()]);
        let kept = &input.backtrack_window[0];
        assert_eq!(kept.from_phase, phase, "from_phase 逐字保真（零裁剪）");
        assert_eq!(kept.to_phase, phase, "to_phase 逐字保真（零裁剪）");
        assert_eq!(kept, &entry, "整条等值（相位 id 形态）");
    }
}

/// 集合三形态（边界，参数化迭代 3 形态）：`[]` / 单元素 / 1000 元素 → 可构造、
/// 可比较、不 panic；`None` 语义不可表达（`Vec` 非 `Option`，编译期事实）。
#[test]
fn decision_input_backtrack_window集合三形态() {
    let shapes = [
        Vec::new(),
        vec![empty_window()],
        (0..1000)
            .map(|index| {
                window_entry(
                    &format!("phase-from-{index}"),
                    index,
                    &format!("phase-to-{index}"),
                    index,
                    &format!("R{index}"),
                )
            })
            .collect::<Vec<_>>(),
    ];
    for (index, windows) in shapes.iter().enumerate() {
        let len = windows.len();
        let input = input_with_window(windows.clone());
        assert_eq!(input.backtrack_window.len(), len, "形态 {index} 条数保真");
        assert_eq!(input.backtrack_window, *windows, "形态 {index} 逐条等值");
        assert_eq!(
            input.clone(),
            input,
            "形态 {index} PartialEq 自反（不 panic）"
        );
    }
    // 1000 条形态：首末条可辨（顺序保真，不重排）
    let many = input_with_window(shapes[2].clone());
    assert_eq!(many.backtrack_window[0].reason, "R0", "首条原位");
    assert_eq!(
        many.backtrack_window[999].reason, "R999",
        "末条原位（顺序保真）"
    );
}
