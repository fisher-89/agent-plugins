use crate::decision::{CandidateReport, DecisionInput};
use crate::prompt::{decision_prompt, evaluator_prompt};
use workflow::model::{ChecklistItem, Verdict};

#[test]
fn evaluator_prompt协议附录瘦身形状与红线() {
    let phase_prompt = "Evaluate implement phase for change \"c\".";
    let prompt = evaluator_prompt(phase_prompt);

    // 协议附录：禁调 MCP 写通道四件 + 最终消息 checklist JSON 瘦身形状
    assert!(prompt.contains("## 输出协议"), "附录段在场");
    assert!(prompt.contains("禁止调用"), "禁调 MCP 红线前置语在场");
    for tool in ["phase_log", "phase_next", "phase_start", "backtrack"] {
        assert!(prompt.contains(tool), "写通道四件红线含 {tool}");
    }
    assert!(
        prompt.contains("\"verdict\"")
            && prompt.contains("\"report\"")
            && prompt.contains("\"checklist\""),
        "checklist JSON 瘦身形状（verdict / report / checklist）约定在场"
    );
    assert!(prompt.contains("2000"), "report 上限约定在场");
    assert!(
        !prompt.contains("\"phase\"")
            && !prompt.contains("\"attempt\"")
            && !prompt.contains("\"skipped\""),
        "MUST NOT 要求 phase / attempt / skipped 回声（walker 按 provenance 盖戳）"
    );
    // 静态角色知识主体逐字节保真：仅 append 协议附录（零重写 / 零插值）
    assert_eq!(
        prompt,
        format!("{phase_prompt}{}", evaluator_prompt("")),
        "phase_prompt 逐字节保真（仅追加协议附录）"
    );
}

#[test]
fn decision_prompt有界组装五段在场() {
    let input = DecisionInput {
        phase: "test-gen".to_owned(),
        attempt: 3,
        fail_checklist: vec![ChecklistItem {
            item: "覆盖 test-design 全行".to_owned(),
            pass: false,
            evidence: "缺失废弃行承接".to_owned(),
        }],
        allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
        candidates: vec![CandidateReport {
            phase: "dev-design".to_owned(),
            verdict: Some(Verdict::Pass),
            report: Some("设计与任务拆解齐备".to_owned()),
        }],
    };

    let prompt = decision_prompt(&input);

    // 五段逐段在场：失败相位信封 + fail checklist 行 + 白名单行 + 候选 eval
    // report + 四动作封闭集说明（有界输入半边）
    assert!(
        prompt.contains("test-gen") && prompt.contains("3"),
        "失败相位信封"
    );
    assert!(
        prompt.contains("覆盖 test-design 全行") && prompt.contains("缺失废弃行承接"),
        "fail checklist 行（item + evidence）"
    );
    assert!(
        prompt.contains("proposal") && prompt.contains("dev-design"),
        "白名单行"
    );
    assert!(
        prompt.contains("设计与任务拆解齐备") && prompt.contains("pass"),
        "候选相位最近一次评估报告（verdict + report）"
    );
    for action in ["backtrack", "retry", "stop", "ask"] {
        assert!(prompt.contains(action), "四动作封闭集说明含 {action}");
    }
    assert!(prompt.contains("500"), "reason ≤500 约定在场");
    assert!(
        prompt.contains("白名单") && prompt.contains("拒绝"),
        "越权红线说明在场"
    );
}

#[test]
fn decision_prompt空集三态组装不崩段落缺席稳定() {
    // fail_checklist 空 / 白名单空 / candidates 空三态
    let input = DecisionInput {
        phase: "implement".to_owned(),
        attempt: 1,
        fail_checklist: Vec::new(),
        allowed: Vec::new(),
        candidates: Vec::new(),
    };
    let prompt = decision_prompt(&input);
    assert!(prompt.contains("implement"), "相位信封在场");
    assert!(prompt.contains("backtrack"), "封闭集说明不因空集缺席");
    assert!(!prompt.contains("覆盖"), "空 fail checklist 不虚增行");
    // 未评估候选形态：verdict 缺省降级文案
    let with_none = DecisionInput {
        phase: "implement".to_owned(),
        attempt: 1,
        fail_checklist: Vec::new(),
        allowed: vec!["proposal".to_owned()],
        candidates: vec![CandidateReport {
            phase: "proposal".to_owned(),
            verdict: None,
            report: None,
        }],
    };
    let prompt = decision_prompt(&with_none);
    assert!(
        prompt.contains("无记录") && prompt.contains("（无 eval report）"),
        "候选 verdict / report 双 None 降级文案稳定"
    );
}

#[test]
fn prompt确定性_同输入重复组装逐字节一致() {
    let phase_prompt = "固定静态主体（零占位）";
    let input = DecisionInput {
        phase: "implement".to_owned(),
        attempt: 2,
        fail_checklist: vec![ChecklistItem {
            item: "项".to_owned(),
            pass: false,
            evidence: "据".to_owned(),
        }],
        allowed: vec!["proposal".to_owned()],
        candidates: Vec::new(),
    };

    assert_eq!(
        evaluator_prompt(phase_prompt),
        evaluator_prompt(phase_prompt),
        "evaluator prompt 确定性"
    );
    assert_eq!(
        decision_prompt(&input),
        decision_prompt(&input),
        "决策 prompt 确定性"
    );
}
