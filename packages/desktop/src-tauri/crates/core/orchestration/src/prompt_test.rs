use crate::decision::{CandidateReport, DecisionInput};
use crate::prompt::{decision_prompt, evaluator_prompt, executor_prompt};
use workflow::model::{ChecklistItem, Verdict};

#[test]
fn executor_prompt三段组装角色要点与prompt主体() {
    let phase_prompt =
        "Implement the code for change \"桌面变更\".\n第二行中文\n**markdown 标记** <特殊> & 字符";

    let prompt = executor_prompt("__CALL_AGENT:implementation-generator__", phase_prompt);

    // 三段齐备：角色要点前导（剥离 __CALL_AGENT:…__ 取角色名）+ prompt 主体
    assert!(
        prompt.contains("implementation-generator"),
        "角色要点前导在场（角色名自令牌剥离）: {prompt}"
    );
    assert!(
        prompt.contains("__CALL_AGENT:") == false,
        "令牌不透传 prompt（剥离约定）"
    );
    assert!(
        prompt.contains(phase_prompt),
        "已插值 phase prompt 主体保真（换行 / 中文 / markdown / 特殊字符不损）"
    );

    // 未收录角色落通用要点兜底
    let fallback = executor_prompt("__CALL_AGENT:未知角色__", "p");
    assert!(
        fallback.contains("未知角色") && fallback.contains("角色要点"),
        "未收录角色落通用要点兜底不崩"
    );
    // 裸 agent_type 原样作角色名
    let bare = executor_prompt("test-design-planner", "p");
    assert!(bare.contains("test-design-planner"), "裸 agent_type 原样");
}

#[test]
fn evaluator_prompt协议附录与checklist_json形状() {
    let phase_prompt = "Evaluate implement phase for change \"c\".";
    let prompt = evaluator_prompt(phase_prompt);

    // 协议附录：禁调 MCP 写通道四件 + 最终消息 checklist JSON 形状约定
    assert!(prompt.contains("## 输出协议"), "附录段在场");
    assert!(
        prompt.contains("phase_log") && prompt.contains("禁止调用"),
        "禁调 MCP phase-log 指令在场（写通道唯一红线）"
    );
    assert!(prompt.contains("phase_next") && prompt.contains("backtrack"));
    assert!(
        prompt.contains("\"verdict\"") && prompt.contains("\"checklist\""),
        "checklist JSON 形状约定在场"
    );
    assert!(prompt.contains("2000"), "report 上限约定在场");
    assert!(
        prompt.contains(phase_prompt),
        "已插值 phase prompt 主体保真（AC-2 evaluator 输出协议 + AC-3 prompt 半边）"
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
    // report + 四动作封闭集说明（AC-2 有界输入半边）
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
    let phase_prompt = "固定模板 <占位不处理>";
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
        executor_prompt("__CALL_AGENT:implementation-generator__", phase_prompt),
        executor_prompt("__CALL_AGENT:implementation-generator__", phase_prompt),
        "executor prompt 无时钟 / 随机参与"
    );
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
