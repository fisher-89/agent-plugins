//! `decision.rs`（决策动作校验器 + 越权白名单门）的单元测试：`validate_decision`
//! 的 tag 封闭集 / 三动作必填字段 / `reason` 上限 / `options` 非空，`DecisionAction`
//! 线形态，以及 `ensure_backtrack_allowed` 的既有回归锚定。
//! 旧 `parse_decision`（文本 JSON 提取）用例随被测函数退役整体删除。
//!
//! 不 mock：`args` 为内存 `serde_json::json!` 字面量，白名单项以内存
//! `BacktrackOption` 字面量构造——纯函数零进程边界依赖。

use workflow::model::Verdict;
use workflow::write::{BacktrackOption, BacktrackWindowEntry};

use crate::decision::{
    ensure_backtrack_allowed, validate_decision, CandidateReport, DecisionAction, DecisionInput,
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

// ---------------------------------------------------------------------------
// `validate_decision`：三动作合法面
// ---------------------------------------------------------------------------

#[test]
fn validate_decision三动作合法逐字段相等() {
    let backtrack = validate_decision(&serde_json::json!({
        "action": "backtrack",
        "to": "dev-design",
        "reason": "设计返工",
    }))
    .expect("合法 backtrack");
    assert_eq!(
        backtrack,
        DecisionAction::Backtrack {
            to: "dev-design".to_owned(),
            reason: "设计返工".to_owned(),
        },
        "backtrack 字段逐值相等"
    );

    let stop = validate_decision(&serde_json::json!({
        "action": "stop",
        "reason": "受控终止",
    }))
    .expect("合法 stop");
    assert_eq!(
        stop,
        DecisionAction::Stop {
            reason: "受控终止".to_owned(),
        }
    );

    let ask = validate_decision(&serde_json::json!({
        "action": "ask",
        "question": "选哪个？",
        "options": ["甲", "乙"],
    }))
    .expect("合法 ask");
    assert_eq!(
        ask,
        DecisionAction::Ask {
            question: "选哪个？".to_owned(),
            options: vec!["甲".to_owned(), "乙".to_owned()],
        }
    );
}

// ---------------------------------------------------------------------------
// 异常：tag 封闭集（Enum 边界 N+1）
// ---------------------------------------------------------------------------

#[test]
fn validate_decision的tag封闭集五形态() {
    let cases: [(&str, serde_json::Value, &str); 5] = [
        (
            "retry（已退役动作）",
            serde_json::json!({ "action": "retry" }),
            "retry",
        ),
        (
            "PascalCase",
            serde_json::json!({ "action": "Backtrack" }),
            "Backtrack",
        ),
        (
            "未知值",
            serde_json::json!({ "action": "unknown" }),
            "unknown",
        ),
        ("空串", serde_json::json!({ "action": "" }), "action"),
        ("tag 缺失", serde_json::json!({ "reason": "x" }), "action"),
    ];
    for (label, args, needle) in cases {
        let error = validate_decision(&args).expect_err(&format!("形态 {label} 应 Err"));
        assert!(
            error.contains(needle),
            "形态 {label} 文案含实际收到值 {needle}: {error}"
        );
        for action in ["backtrack", "stop", "ask"] {
            assert!(
                error.contains(action),
                "形态 {label} 文案含封闭集值 {action}: {error}"
            );
        }
    }
}

#[test]
fn validate_decision非对象入参err且零panic() {
    for args in [
        serde_json::Value::Null,
        serde_json::json!(["backtrack"]),
        serde_json::json!("backtrack"),
    ] {
        let error = validate_decision(&args).expect_err("非对象入参应 Err");
        assert!(
            error.contains("对象") || error.contains("参数形状"),
            "非对象入参文案含形状说明: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// 异常：必填字段缺失 / 类型错（dict 边界）
// ---------------------------------------------------------------------------

#[test]
fn validate_decision必填字段缺失八形态() {
    let cases: [(&str, serde_json::Value, &str, &str); 8] = [
        (
            "backtrack 缺 to",
            serde_json::json!({ "action": "backtrack", "reason": "r" }),
            "to",
            "backtrack",
        ),
        (
            "backtrack 缺 reason",
            serde_json::json!({ "action": "backtrack", "to": "dev-design" }),
            "reason",
            "backtrack",
        ),
        (
            "stop 缺 reason",
            serde_json::json!({ "action": "stop" }),
            "reason",
            "stop",
        ),
        (
            "ask 缺 question",
            serde_json::json!({ "action": "ask", "options": ["甲"] }),
            "question",
            "ask",
        ),
        (
            "ask 缺 options",
            serde_json::json!({ "action": "ask", "question": "q" }),
            "options",
            "ask",
        ),
        (
            "to 非字符串",
            serde_json::json!({ "action": "backtrack", "to": 42, "reason": "r" }),
            "to",
            "backtrack",
        ),
        (
            "reason 非字符串",
            serde_json::json!({ "action": "stop", "reason": null }),
            "reason",
            "stop",
        ),
        (
            "options 非数组",
            serde_json::json!({ "action": "ask", "question": "q", "options": "甲" }),
            "options",
            "ask",
        ),
    ];
    for (label, args, field, action) in cases {
        let error = validate_decision(&args).expect_err(&format!("形态 {label} 应 Err"));
        assert!(
            error.contains(field),
            "形态 {label} 文案含字段名 {field}: {error}"
        );
        // options 缺失的文案以字段名为主（不重复动作名），其余形态携动作名
        assert!(
            error.contains(action) || field == "options",
            "形态 {label} 文案含动作名 {action}（options 形态以字段名为主）: {error}"
        );
    }
}

#[test]
fn validate_decision的options非字符串元素err() {
    let error = validate_decision(&serde_json::json!({
        "action": "ask",
        "question": "q",
        "options": ["甲", 42],
    }))
    .expect_err("非字符串元素应 Err");
    assert!(
        error.contains("options") && error.contains("字符串"),
        "文案定位于 options 的字符串期望: {error}"
    );
}

// ---------------------------------------------------------------------------
// 边界：options 非空 / reason 上限 / 文本特殊字符
// ---------------------------------------------------------------------------

#[test]
fn validate_decision的options四形态() {
    let error = validate_decision(&serde_json::json!({
        "action": "ask",
        "question": "q",
        "options": [],
    }))
    .expect_err("空 options 应 Err");
    assert!(
        error.contains("非空") || error.contains("至少一个"),
        "文案含非空语义: {error}"
    );

    let single = validate_decision(&serde_json::json!({
        "action": "ask",
        "question": "q",
        "options": ["甲"],
    }))
    .expect("单元素合法");
    assert_eq!(
        single,
        DecisionAction::Ask {
            question: "q".to_owned(),
            options: vec!["甲".to_owned()],
        }
    );

    // 空串元素：元素内容不校验（只判数组非空）
    validate_decision(&serde_json::json!({
        "action": "ask",
        "question": "q",
        "options": ["", ""],
    }))
    .expect("空串元素合法（元素内容不校验）");

    // 1000 元素：不 panic
    let many: Vec<String> = (0..1000).map(|index| format!("选项{index}")).collect();
    let parsed = validate_decision(&serde_json::json!({
        "action": "ask",
        "question": "q",
        "options": many,
    }))
    .expect("千元素不 panic");
    let DecisionAction::Ask { options, .. } = parsed else {
        panic!("Ask 变体");
    };
    assert_eq!(options.len(), 1000);
}

#[test]
fn validate_decision的reason上限四形态() {
    let make = |reason: String| serde_json::json!({ "action": "stop", "reason": reason });

    validate_decision(&make("r".repeat(MAX_REASON_CHARS))).expect("恰上限应通过");

    let error =
        validate_decision(&make("r".repeat(MAX_REASON_CHARS + 1))).expect_err("超一字符应 Err");
    assert!(
        error.contains(&MAX_REASON_CHARS.to_string()),
        "文案含上限 500: {error}"
    );
    assert!(error.contains("501"), "文案含当前值 501: {error}");
    assert!(error.contains("超出 1"), "文案含超出量 1: {error}");

    let error = validate_decision(&make("r".repeat(5000))).expect_err("超长应 Err");
    assert!(error.contains("5000"), "文案含当前值 5000: {error}");
    assert!(error.contains("4500"), "文案含超出量 4500: {error}");

    // 空串 / 纯空白 reason → Err（比照缺失，文案携空白拒因）
    for blank in [String::new(), "   ".to_owned()] {
        let error = validate_decision(&make(blank)).expect_err("空白 reason 应 Err");
        assert!(error.contains("空白"), "文案标明空白拒因: {error}");
    }
}

#[test]
fn validate_decision文本特殊字符逐字保真() {
    let special = "第一行\n第二行\0🚀 findings";
    let parsed = validate_decision(&serde_json::json!({
        "action": "backtrack",
        "to": "dev-design\n尾",
        "reason": special,
    }))
    .expect("特殊字符合法");
    let DecisionAction::Backtrack { to, reason } = parsed else {
        panic!("Backtrack 变体");
    };
    assert_eq!(reason, special, "reason 特殊字符逐字保真");
    assert_eq!(to, "dev-design\n尾", "to 特殊字符逐字保真");

    // question 空串 / 纯空白 → Err（必填字符串的空白比照缺失）
    for blank in ["", "   "] {
        let error = validate_decision(&serde_json::json!({
            "action": "ask",
            "question": blank,
            "options": ["甲"],
        }))
        .expect_err("空白 question 应 Err");
        assert!(error.contains("question"), "文案定位于 question: {error}");
    }
    // 超长 question（5000）→ Ok（无上限）
    let parsed = validate_decision(&serde_json::json!({
        "action": "ask",
        "question": "q".repeat(5000),
        "options": ["🚀", "含 "],
    }))
    .expect("超长 question 合法（无上限）");
    let DecisionAction::Ask { question, options } = parsed else {
        panic!("Ask 变体");
    };
    assert_eq!(question.chars().count(), 5000, "超长 question 不截断");
    assert_eq!(options, vec!["🚀".to_owned(), "含 ".to_owned()]);
}

// ---------------------------------------------------------------------------
// 边界：DecisionAction 线形态
// ---------------------------------------------------------------------------

#[test]
fn decision_action线形态与往返等值() {
    let backtrack = DecisionAction::Backtrack {
        to: "dev-design".to_owned(),
        reason: "设计返工".to_owned(),
    };
    let value = serde_json::to_value(&backtrack).expect("序列化");
    assert_eq!(
        value,
        serde_json::json!({
            "action": "backtrack",
            "to": "dev-design",
            "reason": "设计返工",
        }),
        "tag + camelCase 线形态零变"
    );
    assert_eq!(
        serde_json::from_value::<DecisionAction>(value).expect("反序列化"),
        backtrack,
        "backtrack 往返逐字段相等"
    );

    for action in [
        DecisionAction::Stop {
            reason: "停".to_owned(),
        },
        DecisionAction::Ask {
            question: "选".to_owned(),
            options: vec!["甲".to_owned()],
        },
    ] {
        let value = serde_json::to_value(&action).expect("序列化");
        assert_eq!(
            serde_json::from_value::<DecisionAction>(value).expect("反序列化"),
            action,
            "三变体往返逐字段相等"
        );
    }

    assert!(
        serde_json::from_value::<DecisionAction>(serde_json::json!({ "action": "retry" })).is_err(),
        "退役 tag 反序列化 Err"
    );

    // 派生面：Debug / Clone / PartialEq / Eq
    let cloned = backtrack.clone();
    assert_eq!(cloned, backtrack);
    assert!(!format!("{backtrack:?}").is_empty());
}

// ---------------------------------------------------------------------------
// 正向 / 异常：ensure_backtrack_allowed 回归锚定
// ---------------------------------------------------------------------------

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

#[test]
fn 自环拒绝文案与通用越权文案不相等() {
    let self_loop = DecisionAction::Backtrack {
        to: "test-execution".to_owned(),
        reason: "重跑测试".to_owned(),
    };
    let allowed = vec![allowed_phase("proposal"), allowed_phase("test-gen")];
    let self_loop_err = ensure_backtrack_allowed(&self_loop, "test-execution", &allowed)
        .expect_err("零 agent 步自环应 Err");
    assert!(
        self_loop_err.contains("确定性门禁相位") && self_loop_err.contains("零 agent 步"),
        "自环文案显式说明不可重做: {self_loop_err}"
    );
    assert!(
        self_loop_err.contains("stop") && self_loop_err.contains("ask"),
        "自环文案携两条出路（stop / ask）: {self_loop_err}"
    );

    let overreach = DecisionAction::Backtrack {
        to: "test-gen".to_owned(),
        reason: "任意".to_owned(),
    };
    let allowed = vec![allowed_phase("proposal"), allowed_phase("dev-design")];
    let overreach_err =
        ensure_backtrack_allowed(&overreach, "implement", &allowed).expect_err("越权应 Err");
    assert!(
        overreach_err.contains("test-gen") && overreach_err.contains("越权"),
        "错误显式携带越权目标: {overreach_err}"
    );
    assert!(
        overreach_err.contains("proposal") && overreach_err.contains("dev-design"),
        "文案携 allowed 列表原文: {overreach_err}"
    );

    assert_ne!(
        self_loop_err, overreach_err,
        "两成因文案可区分（自环 ≠ 通用越权）"
    );
}

#[test]
fn 空白名单任何backtrack均err无跳转出口() {
    let action = DecisionAction::Backtrack {
        to: "proposal".to_owned(),
        reason: "任意".to_owned(),
    };
    let err = ensure_backtrack_allowed(&action, "implement", &[]).expect_err("空白名单应 Err");
    assert!(
        err.contains("越权") || err.contains("白名单"),
        "越权记因: {err}"
    );

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

// ---------------------------------------------------------------------------
// 边界：DecisionInput 字段面（回归锚定）
// ---------------------------------------------------------------------------

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
/// `validate_decision` / `ensure_backtrack_allowed` 两函数语义零耦合（窗口面不参与
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
    // validate_decision / ensure_backtrack_allowed 仍按各自契约裁决
    let action = validate_decision(&serde_json::json!({
        "action": "backtrack",
        "to": "proposal",
        "reason": "任意",
    }))
    .expect("校验不受窗口面影响");
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

// ---------------------------------------------------------------------------
// 异常：退役面零残留（源码自证）
// ---------------------------------------------------------------------------

#[test]
fn 退役面零残留且白名单门逐字不动() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decision.rs"),
    )
    .expect("读取 decision.rs");
    assert!(
        !own.contains("parse_decision"),
        "decision.rs 内 parse_decision 零命中（文本解析面已退役）"
    );
    assert_eq!(MAX_REASON_CHARS, 500, "reason 上限常量仍在场");
    assert!(
        own.contains("pub fn ensure_backtrack_allowed"),
        "白名单门函数仍在场（逐字未改）"
    );
    for wording in [
        "确定性门禁相位（零 agent 步），不支持重做",
        "越权 backtrack：目标",
    ] {
        assert!(own.contains(wording), "既有拒绝文案逐字在场: {wording}");
    }
}

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`DecisionAction::Ask` 保留变体（生产不可达）+ `"ask"` 臂历史
// 兼容面逐字保留
// ---------------------------------------------------------------------------

/// 反序列化兼容（正向，历史 notice）：`DecisionAction` 的 `"ask"` tag 仍可读
///（历史 `FinishPayload::Decision(Ask{..})` 载荷），逐字段相等。
#[test]
fn decision_action_ask变体反序列化兼容历史载荷() {
    let parsed: DecisionAction = serde_json::from_value(serde_json::json!({
        "action": "ask",
        "question": "q",
        "options": ["a"],
    }))
    .expect("历史 ask 载荷应可反序列化");
    assert_eq!(
        parsed,
        DecisionAction::Ask {
            question: "q".to_owned(),
            options: vec!["a".to_owned()],
        },
        "历史载荷逐字段承接"
    );

    // 往返等值（线形态 action tag 逐字 ask）
    let value = serde_json::to_value(&parsed).expect("序列化");
    assert_eq!(value["action"], "ask", "线形态 tag 逐字 ask");
    let back: DecisionAction = serde_json::from_value(value).expect("反序列化");
    assert_eq!(back, parsed, "往返逐字段相等");
}

/// `"ask"` 臂三个 Err 面逐字保留（异常，参数化 3 形态）：`question` 缺失 /
/// `options` 非数组 / `options` 空数组 → 三文案与改动前逐字相同（历史兼容面
/// MUST NOT 随本变更改文案）。
#[test]
fn validate_decision的ask臂三err面逐字保留() {
    // 三文案与改动前逐字相同（`options` 两形态以字段名为主，不重复动作名）
    let cases: [(&str, serde_json::Value, &[&str]); 3] = [
        (
            "question 缺失",
            serde_json::json!({ "action": "ask", "options": ["甲"] }),
            &["question", "ask"],
        ),
        (
            "options 非数组",
            serde_json::json!({ "action": "ask", "question": "q", "options": "甲" }),
            &["options"],
        ),
        (
            "options 空数组",
            serde_json::json!({ "action": "ask", "question": "q", "options": [] }),
            &["options"],
        ),
    ];
    for (label, args, needles) in cases {
        let error = validate_decision(&args).expect_err(&format!("{label} 应 Err"));
        for needle in needles {
            assert!(
                error.contains(needle),
                "{label}：文案须含「{needle}」，实际: {error}"
            );
        }
    }
}

/// 三变体可辨（边界，参数化 3 变体）：`Backtrack` / `Stop` / `Ask` 两两 `!=`；
/// serde tag 逐字 `backtrack` / `stop` / `ask`（线值与 `finish_step` schema 的
/// 收窄无关——`DecisionAction` 是载荷类型，不是 schema）。
#[test]
fn decision_action三变体可辨且tag逐字() {
    let variants = [
        (
            "backtrack",
            DecisionAction::Backtrack {
                to: "proposal".to_owned(),
                reason: "r".to_owned(),
            },
        ),
        (
            "stop",
            DecisionAction::Stop {
                reason: "r".to_owned(),
            },
        ),
        (
            "ask",
            DecisionAction::Ask {
                question: "q".to_owned(),
                options: vec!["a".to_owned()],
            },
        ),
    ];
    for (tag, variant) in &variants {
        let value = serde_json::to_value(variant).expect("序列化");
        assert_eq!(value["action"], serde_json::json!(tag), "tag 逐字 {tag}");
    }
    for outer in 0..variants.len() {
        for inner in (outer + 1)..variants.len() {
            assert_ne!(
                variants[outer].1, variants[inner].1,
                "变体 {} 与 {} 两两可辨",
                variants[outer].0, variants[inner].0
            );
        }
    }
}

/// 生产不可达的文档面自证（异常，源码自证）：`decision.rs` 含「生产不可达」
/// 类注释且落在 `Ask` 变体上方（doc 注释落位）。
#[test]
fn decision_rs的ask变体带生产不可达注释() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decision.rs"),
    )
    .expect("读取 decision.rs");

    let ask_position = own.find("Ask {").expect("Ask 变体在场");
    let before = &own[..ask_position];
    // 取 Ask 变体上方的最近 3 行（doc 注释落位窗口）
    let window = before.lines().rev().take(8).collect::<Vec<_>>().join(
        "
",
    );
    assert!(
        window.contains("生产不可达"),
        "Ask 变体上方的 doc 注释含「生产不可达」说明，实际: {window}"
    );
    assert!(
        window.contains("AskUserQuestion"),
        "注释指明提问已迁出为独立工具 AskUserQuestion，实际: {window}"
    );
}
