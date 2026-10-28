//! `state.rs` 中性状态类型的单元测试（test-design「state.rs ->
//! state_test.rs」节）：计划图侧类型封闭集（StepNode 二十二值全词汇 +
//! `group()` / `role()` 派生访问器（四分类含 Git 六步——clean 基线恢复步入族）+ OnFail 两变体 + StepDefinition /
//! ArtifactSpec 声明面）+ 执行图侧词汇（RunStepKind 二十二值零过滤 /
//! RunEdgeKind 四值 snake_case）+ 节点与边中性类型序列化往返（身份键
//! `(run_id, phase, attempt, step, instance_no)`、session_id / detail /
//! output_ref 两态）+ ChangeStateRecord 身份面（`id` 首字段为身份锚、`name`
//! 为无唯一约束属性——同名不同 id 不等价）+ 五写命令载荷 `change` ->
//! `change_id` 改名后等值面重证（PartialEq 逐字段可辨）+ run 命令与记录结构两
//! 态构造与等值面（RunFinishCommand 携 nodes + edges，steps 字段退役）+
//! RunFinishCommand 终态三值约束为写面职责（类型层可构造——拦截断言挂
//! write/run_test.rs 与 store_test.rs）+ ChangeStateStore trait 全方法 id 形参编
//! 译面（`list_run_nodes(run_id)` / `list_run_edges(run_id)` 在位、
//! `list_run_steps` 退役）。

use serde_json::json;

use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    AgentRole, ArtifactSpec, BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus,
    OnFail, PhaseLogCommand, PhaseStartState, PhaseStateRecord, RunEdgeEntry, RunEdgeKind,
    RunEdgeStateRecord, RunFinishCommand, RunNodeEntry, RunNodeStateRecord, RunStartCommand,
    RunStartCommand as RunStartCmd, RunStateRecord, RunStatus, RunStepKind, RunStepStatus,
    StepCommand, StepDefinition, StepGroup, StepKind, StepNode, StepStateRecord, StoreFault,
};

/// 身份锚字面量（uuid 形态；一切寻址以 id 为准——name 仅展示属性）。
const CHANGE_ID: &str = "0198f7a0-0000-7000-8000-0000000000a1";
/// 同名异 id 的对拍锚（同 name 不同 id 两记录不等价用）。
const OTHER_ID: &str = "0198f7a0-0000-7000-8000-0000000000a2";

// ---------------------------------------------------------------------------
// run 中性类型线面词汇（封闭集逐字断言）
// ---------------------------------------------------------------------------

/// RunStatus 五值序列化出线恰为 lowercase 词（AC-1/AC-6 线面前置）。
#[test]
fn run_status五值线面lowercase逐字() {
    let wire = |status: RunStatus| serde_json::to_value(status).expect("状态出线应成功");
    assert_eq!(wire(RunStatus::Running), json!("running"));
    assert_eq!(wire(RunStatus::Completed), json!("completed"));
    assert_eq!(wire(RunStatus::Stopped), json!("stopped"));
    assert_eq!(wire(RunStatus::Failed), json!("failed"));
    assert_eq!(
        wire(RunStatus::Interrupted),
        json!("interrupted"),
        "标定值 lowercase 逐字（interrupted 仅启动标定产生）"
    );

    // as_str 单点同源（线格式词与序列化词汇一字不差）
    assert_eq!(RunStatus::Running.as_str(), "running");
    assert_eq!(RunStatus::Completed.as_str(), "completed");
    assert_eq!(RunStatus::Stopped.as_str(), "stopped");
    assert_eq!(RunStatus::Failed.as_str(), "failed");
    assert_eq!(RunStatus::Interrupted.as_str(), "interrupted");
}

/// RunStepKind **二十二值全词汇**出线恰为 snake_case 词（AC-2 词汇单点前置——落库命
/// 令与查询投影同类型消费；流程面 phase_start / phase_log 与三门不再被过滤弃落，
/// 归档 9 步词同面在案（cleanup 尽力步新臂在案）；mutation_test 拆步新词在案，
/// AC-1 / AC-2；initialize 基线恢复步 clean 新臂在案——位序 MutationTest 后）。
#[test]
fn run_step_kind二十二值全词汇snake_case逐字() {
    let wire = |kind: RunStepKind| serde_json::to_value(kind).expect("词汇出线应成功");
    assert_eq!(wire(RunStepKind::Executor), json!("executor"));
    assert_eq!(wire(RunStepKind::Evaluator), json!("evaluator"));
    assert_eq!(wire(RunStepKind::Decision), json!("decision"));
    assert_eq!(
        wire(RunStepKind::StaticCheck),
        json!("static_check"),
        "库读史 snake 词（前端归一单点消费）"
    );
    assert_eq!(wire(RunStepKind::TestExecution), json!("test_execution"));
    assert_eq!(
        wire(RunStepKind::MutationTest),
        json!("mutation_test"),
        "突变门禁步 snake 词（拆步新词——mutation_test ↔ mutationTest 归一在案）"
    );
    assert_eq!(
        wire(RunStepKind::Clean),
        json!("clean"),
        "initialize 基线恢复步 snake 词（serde wire 出线与 as_str 一致；与 cleanup 词面可辨）"
    );
    assert_eq!(wire(RunStepKind::PhaseStart), json!("phase_start"));
    assert_eq!(wire(RunStepKind::PhaseLog), json!("phase_log"));
    assert_eq!(wire(RunStepKind::VerdictGate), json!("verdict_gate"));
    assert_eq!(wire(RunStepKind::RetryGate), json!("retry_gate"));
    assert_eq!(wire(RunStepKind::WhitelistGate), json!("whitelist_gate"));
    // 归档 9 步词（AC-2 新增面；cleanup 尽力步新臂逐字在案）
    assert_eq!(wire(RunStepKind::Preflight), json!("preflight"));
    assert_eq!(wire(RunStepKind::Commit), json!("commit"));
    assert_eq!(wire(RunStepKind::Rebase), json!("rebase"));
    assert_eq!(
        wire(RunStepKind::ResolveConflicts),
        json!("resolve_conflicts"),
        "多词步 snake_case（非 resolveConflicts）"
    );
    assert_eq!(wire(RunStepKind::Merge), json!("merge"));
    assert_eq!(wire(RunStepKind::SpecSync), json!("spec_sync"));
    assert_eq!(
        wire(RunStepKind::Cleanup),
        json!("cleanup"),
        "cleanup 尽力步 snake 词（serde wire 出线与 as_str 一致）"
    );
    assert_eq!(wire(RunStepKind::Seal), json!("seal"));
    assert_eq!(wire(RunStepKind::Finalize), json!("finalize"));
    assert_eq!(
        wire(RunStepKind::ArchiCheck),
        json!("archi_check"),
        "架构漂移门禁步尾词（AC-16 二十二值在案）"
    );

    assert_eq!(RunStepKind::Executor.as_str(), "executor");
    assert_eq!(RunStepKind::Evaluator.as_str(), "evaluator");
    assert_eq!(RunStepKind::Decision.as_str(), "decision");
    assert_eq!(RunStepKind::StaticCheck.as_str(), "static_check");
    assert_eq!(RunStepKind::TestExecution.as_str(), "test_execution");
    assert_eq!(wire(RunStepKind::MutationTest), json!("mutation_test"));
    assert_eq!(RunStepKind::Clean.as_str(), "clean");
    assert_eq!(RunStepKind::PhaseStart.as_str(), "phase_start");
    assert_eq!(RunStepKind::PhaseLog.as_str(), "phase_log");
    assert_eq!(RunStepKind::VerdictGate.as_str(), "verdict_gate");
    assert_eq!(RunStepKind::RetryGate.as_str(), "retry_gate");
    assert_eq!(RunStepKind::WhitelistGate.as_str(), "whitelist_gate");
    assert_eq!(RunStepKind::Preflight.as_str(), "preflight");
    assert_eq!(RunStepKind::Commit.as_str(), "commit");
    assert_eq!(RunStepKind::Rebase.as_str(), "rebase");
    assert_eq!(RunStepKind::ResolveConflicts.as_str(), "resolve_conflicts");
    assert_eq!(RunStepKind::Merge.as_str(), "merge");
    assert_eq!(RunStepKind::SpecSync.as_str(), "spec_sync");
    assert_eq!(RunStepKind::Cleanup.as_str(), "cleanup");
    assert_eq!(RunStepKind::Seal.as_str(), "seal");
    assert_eq!(RunStepKind::Finalize.as_str(), "finalize");
    assert_eq!(RunStepKind::ArchiCheck.as_str(), "archi_check");

    // 二十二值封闭集：全枚举 len==22、wire 词面互异（无遗漏无重复——词汇漂移即断）
    let all = [
        RunStepKind::Executor,
        RunStepKind::Evaluator,
        RunStepKind::Decision,
        RunStepKind::StaticCheck,
        RunStepKind::TestExecution,
        RunStepKind::MutationTest,
        RunStepKind::Clean,
        RunStepKind::PhaseStart,
        RunStepKind::PhaseLog,
        RunStepKind::VerdictGate,
        RunStepKind::RetryGate,
        RunStepKind::WhitelistGate,
        RunStepKind::Preflight,
        RunStepKind::Commit,
        RunStepKind::Rebase,
        RunStepKind::ResolveConflicts,
        RunStepKind::Merge,
        RunStepKind::SpecSync,
        RunStepKind::Cleanup,
        RunStepKind::Seal,
        RunStepKind::Finalize,
        RunStepKind::ArchiCheck,
    ];
    assert_eq!(all.len(), 22, "二十二值穷举无遗漏");
    assert_eq!(
        all.iter().position(|k| *k == RunStepKind::Clean),
        Some(6),
        "Clean 位序在 MutationTest 后（六号位——与 as_str 出线断言同序锚定）"
    );
    let mut words: Vec<&str> = all.iter().map(|kind| kind.as_str()).collect();
    let total = words.len();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), total, "二十二值词面互异（无重复）: {words:?}");
    assert!(
        words.contains(&"clean") && words.contains(&"cleanup"),
        "clean 与 cleanup 两词面同集可辨"
    );

    // StepKind 命令包络八臂（线词面同源——mutation_test 在案，AC-1 审计线词面）
    let wire_kind = |kind: StepKind| serde_json::to_value(kind).expect("包络词出线应成功");
    assert_eq!(wire_kind(StepKind::PhaseNext), json!("phase_next"));
    assert_eq!(wire_kind(StepKind::PhaseStart), json!("phase_start"));
    assert_eq!(wire_kind(StepKind::PhaseLog), json!("phase_log"));
    assert_eq!(wire_kind(StepKind::Backtrack), json!("backtrack"));
    assert_eq!(wire_kind(StepKind::DecisionLog), json!("decision_log"));
    assert_eq!(wire_kind(StepKind::StaticCheck), json!("static_check"));
    assert_eq!(wire_kind(StepKind::TestExecution), json!("test_execution"));
    assert_eq!(
        wire_kind(StepKind::MutationTest),
        json!("mutation_test"),
        "八臂命令包络 mutation_test 线词面"
    );
    assert_eq!(StepKind::MutationTest.as_str(), "mutation_test");
}

/// RunStepStatus 四值出线恰为 camelCase 词（步状态线格式）。
#[test]
fn run_step_status四值线面camelcase逐字() {
    let wire = |status: RunStepStatus| serde_json::to_value(status).expect("出线应成功");
    assert_eq!(wire(RunStepStatus::Running), json!("running"));
    assert_eq!(wire(RunStepStatus::Passed), json!("passed"));
    assert_eq!(wire(RunStepStatus::Failed), json!("failed"));
    assert_eq!(wire(RunStepStatus::Stopped), json!("stopped"));
}

/// 非法词反序列化拒绝：越集词反序列化为各自枚举均 Err（封闭集防线——词汇
/// 漂移即败，AC-3）。
#[test]
fn 非法词反序列化拒绝_封闭集防线() {
    // RunStatus：camelCase 词 / 未知词 / 大写词均越集
    for illegal in ["waitingConfirm", "waitingAsk", "RUNNING", "archived", ""] {
        let back: Result<RunStatus, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "RunStatus 越集词 {illegal:?} 应拒绝");
    }
    // RunStepKind：camel 词 / 近似词 / 空串越集（二十二值词汇之外的词形一律拒绝；
    // cleanup 扩词不松动防线——cleanUp / clean-up / cleanups 仍拒；clean 拆步扩词
    // 同不松动——clear / clean step / Clean 大写词形仍拒，"clean" 本词已合法入集）
    for illegal in [
        "verdictGate",
        "staticCheck",
        "testExecution",
        "mutationTest",
        "retryGate",
        "whitelistGate",
        "event",
        "gate",
        "resolveConflicts",
        "specSync",
        "spec-sync",
        "phaseStart",
        "mutation",
        "cleanUp",
        "clean-up",
        "cleanups",
        "Cleanup",
        "clear",
        "clean step",
        "Clean",
        "",
    ] {
        let back: Result<RunStepKind, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "RunStepKind 越集词 {illegal:?} 应拒绝");
    }
    // RunStepStatus：snake 词 / 大写词 / 近似词均越集（合法词恰 "running"/"passed"/"failed"/"stopped"）
    for illegal in ["pass", "PASS", "ok", "cancelled", "stoped"] {
        let back: Result<RunStepStatus, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "RunStepStatus 越集词 {illegal:?} 应拒绝");
    }
}

// ---------------------------------------------------------------------------
// run 命令与记录结构（两态构造与等值面）
// ---------------------------------------------------------------------------

/// RunStateRecord / RunStepStateRecord 字段全集等值断言：reason / finished_at
/// / session_id / detail None 与 Some 两态构造，PartialEq 等值与不等值面成立。
#[test]
fn run记录两态构造与等值面() {
    let base = RunStateRecord {
        run_id: "run-1727000000000".to_owned(),
        change_id: "demo-change".to_owned(),
        status: RunStatus::Running,
        reason: None,
        started_at: 1_727_000_000_000,
        finished_at: None,
    };
    // None 态字段面（running 恒 None 口径）
    assert_eq!(base.reason, None);
    assert_eq!(base.finished_at, None);
    assert_eq!(base.clone(), base, "PartialEq 自反");

    // Some 态（终态收口形态）：逐字段翻转后不等值
    let finished = RunStateRecord {
        status: RunStatus::Completed,
        reason: Some("All phases have passed.".to_owned()),
        finished_at: Some(1_727_000_060_000),
        ..base.clone()
    };
    assert_eq!(finished.reason.as_deref(), Some("All phases have passed."));
    assert_eq!(finished.finished_at, Some(1_727_000_060_000));
    assert_ne!(finished, base, "终态翻转后不等值（逐字段区分面）");

    // serde 往返无损（camelCase 线面）
    let text = serde_json::to_string(&finished).expect("出线应成功");
    assert!(
        text.contains("\"runId\":\"run-1727000000000\""),
        "camelCase 键: {text}"
    );
    assert!(text.contains("\"startedAt\":"), "startedAt 键在场");
    let back: RunStateRecord = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, finished, "run 记录往返无损");

    // 节点史读记录：session_id / detail / output_ref 两态
    let node_none = RunNodeStateRecord {
        run_id: "run-1727000000000".to_owned(),
        change_id: "demo-change".to_owned(),
        phase: "implement".to_owned(),
        attempt: 1,
        step: RunStepKind::StaticCheck,
        instance_no: 0,
        status: RunStepStatus::Passed,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_060_000),
        token_input: None,
        token_output: None,
        timestamp: 1_727_000_060_000,
    };
    assert_eq!(node_none.session_id, None);
    assert_eq!(node_none.detail, None);
    assert_eq!(node_none.output_ref, None, "Transient 模式恒 None");
    let node_some = RunNodeStateRecord {
        step: RunStepKind::Executor,
        status: RunStepStatus::Failed,
        instance_no: 1,
        session_id: Some("ses-1".to_owned()),
        detail: Some("会话失败收敛".to_owned()),
        output_ref: Some("exec-7".to_owned()),
        ..node_none.clone()
    };
    assert_ne!(node_some, node_none, "两态逐字段可辨");
    let text = serde_json::to_string(&node_none).expect("出线应成功");
    let back: RunNodeStateRecord = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, node_none, "节点史记录往返无损");

    // 边史读记录：端点身份串 + 四 kind
    let edge = RunEdgeStateRecord {
        run_id: "run-1727000000000".to_owned(),
        change_id: "demo-change".to_owned(),
        from_node_key: "implement:1:executor:1".to_owned(),
        to_node_key: "implement:1:static_check:0".to_owned(),
        edge_kind: RunEdgeKind::Feedback,
        timestamp: 1_727_000_060_000,
    };
    assert_ne!(
        RunEdgeStateRecord {
            edge_kind: RunEdgeKind::Retry,
            ..edge.clone()
        },
        edge,
        "边 kind 两值可辨（feedback ≠ retry）"
    );
    let text = serde_json::to_string(&edge).expect("出线应成功");
    let back: RunEdgeStateRecord = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, edge, "边史记录往返无损");
}

/// RunStartCommand / RunFinishCommand（含 RunStepEntry）两态构造与等值面：
/// 命令纯 derive（零 serde——进程内写面载荷），PartialEq 逐字段可辨 ——
/// `change` → `change_id` 字段改名后归属键面重证（change_id 单独翻转即不等价）。
#[test]
fn run命令两态构造与等值面() {
    let start = RunStartCommand {
        run_id: "run-1727000000000".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        started_at: 1_727_000_000_000,
    };
    assert_eq!(start.clone(), start);
    assert_ne!(
        RunStartCmd {
            run_id: "run-1727000000001".to_owned(),
            ..start.clone()
        },
        start,
        "run_id 逐字段可辨"
    );
    assert_ne!(
        RunStartCmd {
            change_id: OTHER_ID.to_owned(),
            ..start.clone()
        },
        start,
        "change_id 逐字段可辨（归属键：一切寻址以 id 为准）"
    );
    assert_eq!(
        start.change_id, CHANGE_ID,
        "字段名 change_id 直读（载荷 id 化）"
    );

    let entry = RunNodeEntry {
        phase: "implement".to_owned(),
        attempt: 2,
        step: RunStepKind::Executor,
        instance_no: 1,
        status: RunStepStatus::Passed,
        session_id: Some("ses-1".to_owned()),
        detail: None,
        output_ref: None,
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_060_000),
        token_input: None,
        token_output: None,
    };
    let edge = RunEdgeEntry {
        from_node_key: "implement:2:executor:1".to_owned(),
        to_node_key: "implement:2:phase_log:0".to_owned(),
        edge_kind: RunEdgeKind::Forward,
    };
    let finish = RunFinishCommand {
        run_id: "run-1727000000000".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        status: RunStatus::Completed,
        reason: Some("All phases have passed.".to_owned()),
        finished_at: 1_727_000_060_000,
        nodes: vec![entry.clone()],
        edges: vec![edge.clone()],
    };
    assert_eq!(finish.clone(), finish);
    // change_id / reason / steps 翻转不等值
    assert_ne!(
        RunFinishCommand {
            change_id: OTHER_ID.to_owned(),
            ..finish.clone()
        },
        finish,
        "change_id 逐字段可辨（收口载荷随行归属键）"
    );
    assert_ne!(
        RunFinishCommand {
            reason: None,
            ..finish.clone()
        },
        finish,
        "reason None/Some 可辨"
    );
    assert_ne!(
        RunFinishCommand {
            nodes: Vec::new(),
            ..finish.clone()
        },
        finish,
        "nodes 空包/整包可辨（零步 run 收口边界形态合法）"
    );
    assert_ne!(
        RunFinishCommand {
            edges: Vec::new(),
            ..finish.clone()
        },
        finish,
        "edges 空包/整包可辨（零边 run 收口边界形态合法）"
    );
    // 条目字段逐项透传面
    assert_eq!(entry.instance_no, 1, "稳定身份键实例号");
    assert_eq!(entry.step, RunStepKind::Executor);
    assert_eq!(entry.session_id.as_deref(), Some("ses-1"));
    assert_eq!(edge.edge_kind, RunEdgeKind::Forward);
    assert_eq!(edge.from_node_key, "implement:2:executor:1");
}

/// 其余三写命令载荷（PhaseLogCommand / BacktrackCommand / StepCommand）同式：
/// `change_id` 归属键改名后 PartialEq 逐字段可辨（含与其余字段的区分面）。
#[test]
fn 其余三写载荷change_id等值面同式() {
    let phase_log = PhaseLogCommand {
        change_id: CHANGE_ID.to_owned(),
        phase: "dev-design".to_owned(),
        verdict: Verdict::Pass,
        report: "设计与任务拆解齐备".to_owned(),
        skipped: false,
        checklist: vec![ChecklistItem {
            item: "组件表完整".to_owned(),
            pass: true,
            evidence: "事实依据".to_owned(),
        }],
        executor_session_id: Some("ses-exec-1".to_owned()),
        evaluator_session_id: None,
        decision_session_id: None,
        start_at: Some(1_727_000_080_000),
        timestamp: 1_727_000_090_000,
    };
    assert_eq!(phase_log.clone(), phase_log, "PartialEq 自反");
    assert_ne!(
        PhaseLogCommand {
            change_id: OTHER_ID.to_owned(),
            ..phase_log.clone()
        },
        phase_log,
        "PhaseLogCommand.change_id 逐字段可辨"
    );
    assert_ne!(
        PhaseLogCommand {
            verdict: Verdict::Fail,
            ..phase_log.clone()
        },
        phase_log,
        "verdict 逐字段可辨（change_id 之外的字段面零塌缩）"
    );
    assert_ne!(
        PhaseLogCommand {
            start_at: None,
            ..phase_log.clone()
        },
        phase_log,
        "start_at None/Some 两态可辨"
    );

    let backtrack = BacktrackCommand {
        change_id: CHANGE_ID.to_owned(),
        phase: "test-gen".to_owned(),
        to: "proposal".to_owned(),
        reason: "需求基线返工".to_owned(),
        stale_dependents: vec!["dev-design".to_owned(), "test-design".to_owned()],
    };
    assert_eq!(backtrack.clone(), backtrack);
    assert_ne!(
        BacktrackCommand {
            change_id: OTHER_ID.to_owned(),
            ..backtrack.clone()
        },
        backtrack,
        "BacktrackCommand.change_id 逐字段可辨"
    );
    assert_ne!(
        BacktrackCommand {
            stale_dependents: Vec::new(),
            ..backtrack.clone()
        },
        backtrack,
        "stale 闭包空/非空可辨"
    );

    let step = StepCommand {
        run_id: "run-1727000000000".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        step_kind: StepKind::PhaseLog,
        status: "ok".to_owned(),
        summary: "落账步骤".to_owned(),
        reference: None,
        timestamp: 1_727_000_090_000,
    };
    assert_eq!(step.clone(), step);
    assert_ne!(
        StepCommand {
            change_id: OTHER_ID.to_owned(),
            ..step.clone()
        },
        step,
        "StepCommand.change_id 逐字段可辨"
    );
    assert_ne!(
        StepCommand {
            step_kind: StepKind::Backtrack,
            ..step.clone()
        },
        step,
        "step_kind 逐字段可辨"
    );
}

/// ChangeStateRecord 身份面：`id` 首字段为身份锚（终身恒定、MUST NOT 复用）、
/// `name` 为无唯一约束的可变属性——同 name 不同 id 两记录不等价，同 id 不同
/// name 可辨（属性面随动）。
#[test]
fn change_state_record身份面_id为锚name为属性() {
    let base = ChangeStateRecord {
        id: CHANGE_ID.to_owned(),
        name: "demo-change".to_owned(),
        title: "demo-change".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1_727_000_000_000,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    };
    assert_eq!(base.clone(), base, "PartialEq 自反");

    // 同 name 不同 id：身份锚相异 → 两记录不等价（name 零唯一约束）
    let same_name_other_id = ChangeStateRecord {
        id: OTHER_ID.to_owned(),
        ..base.clone()
    };
    assert_eq!(same_name_other_id.name, base.name, "前置：同名两记录");
    assert_ne!(same_name_other_id.id, base.id, "id 各异");
    assert_ne!(
        same_name_other_id, base,
        "同 name 不同 id 两记录不等价（id 为身份锚）"
    );

    // 同 id 不同 name：同一档案（name 为可变属性——改名不换身份）
    let same_id_other_name = ChangeStateRecord {
        name: "renamed-change".to_owned(),
        ..base.clone()
    };
    assert_eq!(same_id_other_name.id, base.id, "身份锚恒定");
    assert_ne!(same_id_other_name, base, "name 逐字段可辨（属性面）");

    // 线面：id / name 双键出线（id 为身份锚、name 恒裸名）
    let text = serde_json::to_string(&base).expect("出线应成功");
    assert!(text.contains(&format!("\"id\":\"{CHANGE_ID}\"")), "{text}");
    assert!(text.contains("\"name\":\"demo-change\""), "{text}");
}

/// ChangeStateRecord title 字段面（新增）：title 独立字段构造 → PartialEq
/// 逐字段可辨（title 与 name 双字段可辨），线面含 title 键。
#[test]
fn change_state_record_title字段面独立可辨() {
    let base = ChangeStateRecord {
        id: CHANGE_ID.to_owned(),
        name: "add-feature".to_owned(),
        title: "新增特性".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1_727_000_000_000,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    };
    assert_eq!(base.clone(), base, "PartialEq 自反");
    assert_eq!(base.title, "新增特性", "title 独立字段直读");
    assert_ne!(base.title, base.name, "title 与 name 双字段可辨");

    // 同 name 不同 title：两记录不等价（title 参与等值面）
    let other_title = ChangeStateRecord {
        title: "另一种标题".to_owned(),
        ..base.clone()
    };
    assert_eq!(other_title.name, base.name, "前置：同名两记录");
    assert_eq!(other_title.id, base.id, "前置：同 id");
    assert_ne!(
        other_title, base,
        "同 name 不同 title 两记录不等价（title 独立字段参与等值）"
    );

    // 线面：title 键出线（camelCase 不变量）
    let value = serde_json::to_value(&base).expect("出线应成功");
    assert_eq!(value["title"], json!("新增特性"), "线面含 title 键");
    assert_eq!(value["name"], json!("add-feature"));
    assert_eq!(value["id"], json!(CHANGE_ID));
}

/// title 与 name 可辨（边界）：title 恒非空（默认 = name 由构造侧单点保证，
/// 中性类型不校验）；同 name 不同 title 不等价；既有字段面逐字不变。
#[test]
fn title与name可辨_中性类型不校验非空且既有字段面持衡() {
    let base = ChangeStateRecord {
        id: CHANGE_ID.to_owned(),
        name: "demo-change".to_owned(),
        title: "demo-change".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1_727_000_000_000,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    };

    // 默认 = name 形态（创建 / 升级路径兜底）；title 恒非空
    assert_eq!(base.title, base.name, "默认 title = name");
    assert!(!base.title.is_empty(), "title 恒非空");

    // 中性类型不校验：空 title 可构造（拦截归写面 / store 单点）
    let blank_title = ChangeStateRecord {
        title: String::new(),
        ..base.clone()
    };
    assert_eq!(blank_title.title, "", "中性类型零校验（非空约束归写面）");
    assert_ne!(blank_title, base, "空 title 与兜底 title 可辨");

    // 既有字段面逐字不变（id / workflow_type / status / archived_at /
    // active_phase / worktree / base_commit）
    assert_eq!(base.id, CHANGE_ID);
    assert_eq!(base.workflow_type, "requirement");
    assert_eq!(base.status, ChangeStatus::Active);
    assert_eq!(base.archived_at, None);
    assert_eq!(base.active_phase, None);
    assert_eq!(base.worktree, None);
    assert_eq!(base.base_commit, None);
    assert_eq!(base.created_at, 1_727_000_000_000);
}

/// RunFinishCommand.status 携 Running / Interrupted 可构造（类型层不拦——
/// 终态三值约束为写面校验职责：write/run_test.rs「运行期写路径不产生
/// interrupted」与 store_test.rs 收口前置各自承载，AC-4 交叉注记锚）。
#[test]
fn run_finish命令status五值可构造_拦截归写面() {
    for status in [
        RunStatus::Running,
        RunStatus::Interrupted,
        RunStatus::Completed,
        RunStatus::Stopped,
        RunStatus::Failed,
    ] {
        let command = RunFinishCommand {
            run_id: "run-1".to_owned(),
            change_id: "demo-change".to_owned(),
            status,
            reason: None,
            finished_at: 1_727_000_060_000,
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        assert_eq!(command.status, status, "五值均可构造（类型层零拦截）");
    }
}

// ---------------------------------------------------------------------------
// ChangeStateStore 全方法：trait id 形参编译面（行为断言归实现侧测试承载）
// ---------------------------------------------------------------------------

/// trait 全方法 id 形参编译面：最小假件按 `get_change(id)` /
/// `list_phase_records(change_id)` / `start_phase(change_id, …)` /
/// `amend_decision_session(change_id, …)` / `set_archived(id, …)` /
/// `list_runs(change_id)` 等 id 形参实现后可调（探针内断言 id 实参抵达——非
/// 空且逐字为调用方所传；本节不重复行为断言——写面假件行为挂 write 各节、
/// infra/store 真件行为挂 store_test.rs / change_port_test.rs）。
#[test]
fn change_state_store全方法id形参编译面() {
    /// 编译面探针：全方法可调 + id 形参抵达断言。
    struct SignatureProbe;

    impl ChangeStateStore for SignatureProbe {
        fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
            assert_eq!(id, CHANGE_ID, "id 形参逐字抵达");
            Ok(None)
        }
        fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
            Ok(Vec::new())
        }
        fn list_phase_records(&self, change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
            assert_eq!(change_id, CHANGE_ID, "change_id 形参逐字抵达");
            Ok(Vec::new())
        }
        fn list_steps(
            &self,
            change_id: &str,
            run_id: Option<&str>,
        ) -> Result<Vec<StepStateRecord>, StoreFault> {
            assert_eq!(change_id, CHANGE_ID);
            assert_eq!(run_id, Some("run-1"));
            Ok(Vec::new())
        }
        fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault> {
            assert_eq!(record.id, CHANGE_ID, "建档记录携身份锚");
            Ok(())
        }
        fn delete_change_record(&self, id: &str) -> Result<bool, StoreFault> {
            assert_eq!(id, CHANGE_ID, "补偿删除按 id");
            Ok(false)
        }
        fn start_phase(
            &self,
            change_id: &str,
            phase: &str,
            now: i64,
        ) -> Result<PhaseStartState, StoreFault> {
            assert_eq!(change_id, CHANGE_ID);
            assert_eq!(phase, "proposal");
            Ok(PhaseStartState {
                attempt: 1,
                start_at: now,
            })
        }
        fn log_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreFault> {
            assert_eq!(command.change_id, CHANGE_ID, "落账载荷 change_id 抵达");
            Ok(1)
        }
        fn apply_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreFault> {
            assert_eq!(command.change_id, CHANGE_ID, "回跳载荷 change_id 抵达");
            Ok(())
        }
        fn amend_decision_session(
            &self,
            change_id: &str,
            phase: &str,
            session_id: &str,
        ) -> Result<(), StoreFault> {
            assert_eq!(
                (change_id, phase, session_id),
                (CHANGE_ID, "proposal", "ses-1")
            );
            Ok(())
        }
        fn set_archived(&self, id: &str, archived_at: i64) -> Result<(), StoreFault> {
            assert_eq!((id, archived_at), (CHANGE_ID, 1_727_000_060_000));
            Ok(())
        }
        fn append_step(&self, command: &StepCommand) -> Result<(), StoreFault> {
            assert_eq!(command.change_id, CHANGE_ID, "步骤审计载荷 change_id 抵达");
            Ok(())
        }
        fn list_runs(&self, change_id: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
            assert_eq!(change_id, CHANGE_ID, "run 清单按 id 归属");
            Ok(Vec::new())
        }
        fn list_run_nodes(&self, run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreFault> {
            assert_eq!(run_id, "run-1", "节点读面按 run_id 圈定");
            Ok(Vec::new())
        }
        fn list_run_edges(&self, run_id: &str) -> Result<Vec<RunEdgeStateRecord>, StoreFault> {
            assert_eq!(run_id, "run-1", "边读面按 run_id 圈定");
            Ok(Vec::new())
        }
        fn run_start(&self, command: &RunStartCommand) -> Result<(), StoreFault> {
            assert_eq!(command.change_id, CHANGE_ID, "run 发起载荷 change_id 抵达");
            Ok(())
        }
        fn run_finish(&self, command: &RunFinishCommand) -> Result<(), StoreFault> {
            assert_eq!(command.change_id, CHANGE_ID, "run 收口载荷 change_id 抵达");
            Ok(())
        }
    }

    let probe = SignatureProbe;
    assert_eq!(probe.get_change(CHANGE_ID).expect("读面应可用"), None);
    assert_eq!(
        probe
            .list_phase_records(CHANGE_ID)
            .expect("读面应可用")
            .len(),
        0
    );
    assert_eq!(
        probe
            .list_steps(CHANGE_ID, Some("run-1"))
            .expect("读面应可用"),
        Vec::new(),
        "miss 非错误（空数组）"
    );
    probe
        .create_change_record(ChangeStateRecord {
            id: CHANGE_ID.to_owned(),
            name: "demo-change".to_owned(),
            title: "demo-change".to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: 1_727_000_000_000,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("写面应可用");
    assert!(!probe.delete_change_record(CHANGE_ID).expect("写面应可用"));
    assert_eq!(
        probe
            .start_phase(CHANGE_ID, "proposal", 1_727_000_000_000)
            .expect("写面应可用")
            .attempt,
        1
    );
    assert_eq!(
        probe
            .log_phase(&PhaseLogCommand {
                change_id: CHANGE_ID.to_owned(),
                phase: "proposal".to_owned(),
                verdict: Verdict::Pass,
                report: "报告".to_owned(),
                skipped: false,
                checklist: Vec::new(),
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
                start_at: None,
                timestamp: 1_727_000_090_000,
            })
            .expect("写面应可用"),
        1
    );
    probe
        .apply_backtrack(&BacktrackCommand {
            change_id: CHANGE_ID.to_owned(),
            phase: "dev-design".to_owned(),
            to: "proposal".to_owned(),
            reason: "返工".to_owned(),
            stale_dependents: Vec::new(),
        })
        .expect("写面应可用");
    probe
        .amend_decision_session(CHANGE_ID, "proposal", "ses-1")
        .expect("写面应可用");
    probe
        .set_archived(CHANGE_ID, 1_727_000_060_000)
        .expect("写面应可用");
    probe
        .append_step(&StepCommand {
            run_id: "run-1".to_owned(),
            change_id: CHANGE_ID.to_owned(),
            step_kind: StepKind::PhaseNext,
            status: "ok".to_owned(),
            summary: "路由步骤".to_owned(),
            reference: None,
            timestamp: 1_727_000_090_000,
        })
        .expect("写面应可用");
    assert_eq!(probe.list_runs(CHANGE_ID).expect("读面应可用"), Vec::new());
    assert_eq!(
        probe.list_run_nodes("run-1").expect("读面应可用"),
        Vec::new(),
        "miss 非错误（空数组）"
    );
    assert_eq!(
        probe.list_run_edges("run-1").expect("读面应可用"),
        Vec::new(),
        "miss 非错误（空数组）"
    );
    probe
        .run_start(&RunStartCommand {
            run_id: "run-1".to_owned(),
            change_id: CHANGE_ID.to_owned(),
            started_at: 1_727_000_000_000,
        })
        .expect("写面应可用");
    probe
        .run_finish(&RunFinishCommand {
            run_id: "run-1".to_owned(),
            change_id: CHANGE_ID.to_owned(),
            status: RunStatus::Completed,
            reason: None,
            finished_at: 1_727_000_060_000,
            nodes: Vec::new(),
            edges: Vec::new(),
        })
        .expect("写面应可用");

    // Send + Sync 硬校验（跨线程经 Arc<dyn ChangeStateStore> 注入的编译前提）
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SignatureProbe>();
    assert_send_sync::<ChangeStatus>();
}

// ---------------------------------------------------------------------------
// 计划图侧类型（StepNode 二十二值全词汇 + 派生访问器 + OnFail / StepDefinition /
// ArtifactSpec）
// ---------------------------------------------------------------------------

/// `StepNode` 二十二值全词汇 snake_case 逐字出线（executor / evaluator / decision /
/// phase_start / static_check / test_execution / mutation_test / clean / phase_log /
/// verdict_gate / retry_gate / whitelist_gate + 归档九步 preflight / commit /
/// rebase / resolve_conflicts / merge / spec_sync / cleanup / seal / finalize）
/// ——词汇本体单点（计划图声明与执行图落库同词面 1:1 全映射，AC-2 全词汇前置；
/// mutation_test 拆步新词在案，AC-1；cleanup 尽力步新臂在案；initialize 基线
/// 恢复步 clean 新臂在案——位序 MutationTest 后）。
#[test]
fn step_node二十二值全词汇snake_case逐字() {
    let wire = |node: StepNode| serde_json::to_value(node).expect("词汇出线应成功");
    assert_eq!(wire(StepNode::Executor), json!("executor"));
    assert_eq!(wire(StepNode::Evaluator), json!("evaluator"));
    assert_eq!(wire(StepNode::Decision), json!("decision"));
    assert_eq!(wire(StepNode::PhaseStart), json!("phase_start"));
    assert_eq!(wire(StepNode::StaticCheck), json!("static_check"));
    assert_eq!(wire(StepNode::TestExecution), json!("test_execution"));
    assert_eq!(
        wire(StepNode::MutationTest),
        json!("mutation_test"),
        "突变门禁步拆步新词（AC-1 / AC-2）"
    );
    assert_eq!(
        wire(StepNode::Clean),
        json!("clean"),
        "initialize 基线恢复步词面逐字（serde wire 与 as_str 一致）"
    );
    assert_eq!(wire(StepNode::PhaseLog), json!("phase_log"));
    assert_eq!(wire(StepNode::VerdictGate), json!("verdict_gate"));
    assert_eq!(wire(StepNode::RetryGate), json!("retry_gate"));
    assert_eq!(wire(StepNode::WhitelistGate), json!("whitelist_gate"));
    assert_eq!(wire(StepNode::Preflight), json!("preflight"));
    assert_eq!(wire(StepNode::Commit), json!("commit"));
    assert_eq!(wire(StepNode::Rebase), json!("rebase"));
    assert_eq!(wire(StepNode::ResolveConflicts), json!("resolve_conflicts"));
    assert_eq!(wire(StepNode::Merge), json!("merge"));
    assert_eq!(wire(StepNode::SpecSync), json!("spec_sync"));
    assert_eq!(
        wire(StepNode::Cleanup),
        json!("cleanup"),
        "cleanup 尽力步词面逐字（serde wire 与 as_str 一致）"
    );
    assert_eq!(wire(StepNode::Seal), json!("seal"));
    assert_eq!(wire(StepNode::Finalize), json!("finalize"));
    assert_eq!(
        wire(StepNode::ArchiCheck),
        json!("archi_check"),
        "架构漂移门禁工具步尾词（AC-16 二十二值在案）"
    );

    // as_str 单点同源（线格式词与序列化词汇一字不差）
    assert_eq!(StepNode::Executor.as_str(), "executor");
    assert_eq!(StepNode::Evaluator.as_str(), "evaluator");
    assert_eq!(StepNode::Decision.as_str(), "decision");
    assert_eq!(StepNode::PhaseStart.as_str(), "phase_start");
    assert_eq!(StepNode::StaticCheck.as_str(), "static_check");
    assert_eq!(StepNode::TestExecution.as_str(), "test_execution");
    assert_eq!(StepNode::MutationTest.as_str(), "mutation_test");
    assert_eq!(StepNode::Clean.as_str(), "clean");
    assert_eq!(StepNode::PhaseLog.as_str(), "phase_log");
    assert_eq!(StepNode::VerdictGate.as_str(), "verdict_gate");
    assert_eq!(StepNode::RetryGate.as_str(), "retry_gate");
    assert_eq!(StepNode::WhitelistGate.as_str(), "whitelist_gate");
    assert_eq!(StepNode::Preflight.as_str(), "preflight");
    assert_eq!(StepNode::Commit.as_str(), "commit");
    assert_eq!(StepNode::Rebase.as_str(), "rebase");
    assert_eq!(StepNode::ResolveConflicts.as_str(), "resolve_conflicts");
    assert_eq!(StepNode::Merge.as_str(), "merge");
    assert_eq!(StepNode::SpecSync.as_str(), "spec_sync");
    assert_eq!(StepNode::Cleanup.as_str(), "cleanup");
    assert_eq!(StepNode::Seal.as_str(), "seal");
    assert_eq!(StepNode::Finalize.as_str(), "finalize");
    assert_eq!(StepNode::ArchiCheck.as_str(), "archi_check");
}

/// `StepNode` 与 `RunStepKind` 逐字同源（22 值 1:1 全映射；线格式词一字不差，
/// AC-2 / AC-6 落库零过滤前置；cleanup 对拍行补齐——两域词面零错位；clean 基线
/// 恢复步对拍行补齐）。
#[test]
fn step_node与run_step_kind逐字同源二十二值() {
    for (node, kind, word) in [
        (StepNode::Executor, RunStepKind::Executor, "executor"),
        (StepNode::Evaluator, RunStepKind::Evaluator, "evaluator"),
        (StepNode::Decision, RunStepKind::Decision, "decision"),
        (StepNode::PhaseStart, RunStepKind::PhaseStart, "phase_start"),
        (
            StepNode::StaticCheck,
            RunStepKind::StaticCheck,
            "static_check",
        ),
        (
            StepNode::TestExecution,
            RunStepKind::TestExecution,
            "test_execution",
        ),
        (
            StepNode::MutationTest,
            RunStepKind::MutationTest,
            "mutation_test",
        ),
        (StepNode::Clean, RunStepKind::Clean, "clean"),
        (StepNode::PhaseLog, RunStepKind::PhaseLog, "phase_log"),
        (
            StepNode::VerdictGate,
            RunStepKind::VerdictGate,
            "verdict_gate",
        ),
        (StepNode::RetryGate, RunStepKind::RetryGate, "retry_gate"),
        (
            StepNode::WhitelistGate,
            RunStepKind::WhitelistGate,
            "whitelist_gate",
        ),
        (StepNode::Preflight, RunStepKind::Preflight, "preflight"),
        (StepNode::Commit, RunStepKind::Commit, "commit"),
        (StepNode::Rebase, RunStepKind::Rebase, "rebase"),
        (
            StepNode::ResolveConflicts,
            RunStepKind::ResolveConflicts,
            "resolve_conflicts",
        ),
        (StepNode::Merge, RunStepKind::Merge, "merge"),
        (StepNode::SpecSync, RunStepKind::SpecSync, "spec_sync"),
        (StepNode::Cleanup, RunStepKind::Cleanup, "cleanup"),
        (StepNode::Seal, RunStepKind::Seal, "seal"),
        (StepNode::Finalize, RunStepKind::Finalize, "finalize"),
        (StepNode::ArchiCheck, RunStepKind::ArchiCheck, "archi_check"),
    ] {
        assert_eq!(node.as_str(), word, "{node:?} 计划图侧词面");
        assert_eq!(kind.as_str(), word, "{kind:?} 执行图侧词面");
        assert_eq!(
            serde_json::to_value(node).expect("出线应成功"),
            serde_json::to_value(kind).expect("出线应成功"),
            "{word} 两侧线面逐字一致"
        );
    }
}

/// `StepNode::group()` 四分类（Exec / Agent / Git / Gate）与 `role()`（Executor /
/// Evaluator / Decision / SpecSync / ResolveConflicts）派生访问器逐值一致——同类内
/// 与跨类皆可辨（AC-2 派生访问器零冗余字段；D4 分派前置；cleanup 归 Git 步族、
/// role 恒 None；clean 基线恢复步归 Git 步族、role 恒 None）。
#[test]
fn step_node分组与角色派生访问器一致() {
    // Exec 组：相位机步 + 检查步（含拆步新词 mutation_test）+ 归档内联步
    //（preflight / seal）
    for node in [
        StepNode::PhaseStart,
        StepNode::StaticCheck,
        StepNode::TestExecution,
        StepNode::MutationTest,
        StepNode::ArchiCheck,
        StepNode::PhaseLog,
        StepNode::Preflight,
        StepNode::Seal,
    ] {
        assert_eq!(node.group(), StepGroup::Exec, "{node:?} 属 Exec");
        assert_eq!(node.role(), None, "非 agent 步无角色");
    }
    // Agent 组：五角色与 role() 一一对应（含归档两会话步）
    for (node, role) in [
        (StepNode::Executor, AgentRole::Executor),
        (StepNode::Evaluator, AgentRole::Evaluator),
        (StepNode::Decision, AgentRole::Decision),
        (StepNode::SpecSync, AgentRole::SpecSync),
        (StepNode::ResolveConflicts, AgentRole::ResolveConflicts),
    ] {
        assert_eq!(node.group(), StepGroup::Agent, "{node:?} 属 Agent");
        assert_eq!(node.role(), Some(role));
    }
    // Git 组：六 vcs 步（clean 基线恢复步与 cleanup 尽力步和 Commit / Rebase /
    // Merge / Finalize 同组；role 恒 None——非 agent 步零 prompt 零角色）
    for node in [
        StepNode::Clean,
        StepNode::Commit,
        StepNode::Rebase,
        StepNode::Merge,
        StepNode::Cleanup,
        StepNode::Finalize,
    ] {
        assert_eq!(node.group(), StepGroup::Git, "{node:?} 属 Git");
        assert_eq!(node.role(), None, "git 步无 agent 角色");
    }
    assert_eq!(StepNode::Clean.role(), None, "Clean 恒 None（非 agent 步）");
    assert_eq!(
        StepNode::Cleanup.role(),
        None,
        "Cleanup 恒 None（非 agent 步）"
    );
    // Gate 组：三门
    for node in [
        StepNode::VerdictGate,
        StepNode::RetryGate,
        StepNode::WhitelistGate,
    ] {
        assert_eq!(node.group(), StepGroup::Gate, "{node:?} 属 Gate");
        assert_eq!(node.role(), None, "门步无角色");
    }
    // 四分类互斥：Exec / Agent / Git / Gate 覆盖全词汇（22 值无遗漏无重叠）
    let groups: Vec<StepGroup> = [
        StepNode::Executor,
        StepNode::Evaluator,
        StepNode::Decision,
        StepNode::PhaseStart,
        StepNode::StaticCheck,
        StepNode::TestExecution,
        StepNode::MutationTest,
        StepNode::Clean,
        StepNode::PhaseLog,
        StepNode::VerdictGate,
        StepNode::RetryGate,
        StepNode::WhitelistGate,
        StepNode::Preflight,
        StepNode::Commit,
        StepNode::Rebase,
        StepNode::ResolveConflicts,
        StepNode::Merge,
        StepNode::SpecSync,
        StepNode::Cleanup,
        StepNode::Seal,
        StepNode::Finalize,
        StepNode::ArchiCheck,
    ]
    .iter()
    .map(|node| node.group())
    .collect();
    assert_eq!(groups.len(), 22, "二十二值穷举无遗漏");
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Exec).count(), 8);
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Agent).count(), 5);
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Git).count(), 6);
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Gate).count(), 3);
}

/// `StepGroup` 四分类线面 lowercase 逐字出线（exec / agent / git / gate）；
/// 非法词反序列化拒绝（AC-2 新增 `git` 词面）。
#[test]
fn step_group四值线面lowercase逐字() {
    let wire = |group: StepGroup| serde_json::to_value(group).expect("出线应成功");
    assert_eq!(wire(StepGroup::Exec), json!("exec"));
    assert_eq!(wire(StepGroup::Agent), json!("agent"));
    assert_eq!(wire(StepGroup::Git), json!("git"));
    assert_eq!(wire(StepGroup::Gate), json!("gate"));
}

/// `AgentRole` 五值线面 snake_case 逐字出线（executor / evaluator / decision /
/// spec_sync / resolve_conflicts）——与 orchestration `WorkerRole::as_str()` 同源
/// 词面（AC-2 / D3）。
#[test]
fn agent_role五值线面snake_case逐字() {
    let wire = |role: AgentRole| serde_json::to_value(role).expect("出线应成功");
    assert_eq!(wire(AgentRole::Executor), json!("executor"));
    assert_eq!(wire(AgentRole::Evaluator), json!("evaluator"));
    assert_eq!(wire(AgentRole::Decision), json!("decision"));
    assert_eq!(wire(AgentRole::SpecSync), json!("spec_sync"));
    assert_eq!(
        wire(AgentRole::ResolveConflicts),
        json!("resolve_conflicts")
    );

    for (role, word) in [
        (AgentRole::Executor, "executor"),
        (AgentRole::Evaluator, "evaluator"),
        (AgentRole::Decision, "decision"),
        (AgentRole::SpecSync, "spec_sync"),
        (AgentRole::ResolveConflicts, "resolve_conflicts"),
    ] {
        let back: AgentRole = serde_json::from_value(json!(word)).expect("合法词往返应成功");
        assert_eq!(back, role, "{word} 往返逐词可辨");
    }
}

/// 非法词反序列化拒绝：越集词反序列化为 `StepNode` / `StepGroup` /
/// `AgentRole` 均 Err（封闭集防线）；`OnFail` 三变体往返可辨、缺字段拒绝。
#[test]
fn step_node与on_fail封闭集防线() {
    for illegal in [
        "exec",
        "gate",
        "agent",
        "verdict",
        "phaseStart",
        "git",
        "archive",
        "spec-sync",
        "resolveConflicts",
        "phaseLog",
        "cleanUp",
        "clean-up",
        "clear",
        "clean step",
        "Clean",
        "",
    ] {
        let back: Result<StepNode, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "StepNode 越集词 {illegal:?} 应拒绝");
    }
    for illegal in ["execs", "Gate", "WORKER_AGENT", ""] {
        let back: Result<StepGroup, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "StepGroup 越集词 {illegal:?} 应拒绝");
    }
    for illegal in ["exec", "Role", "decisioner", ""] {
        let back: Result<AgentRole, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "AgentRole 越集词 {illegal:?} 应拒绝");
    }

    // OnFail 三变体往返可辨：FeedbackLoop { limit }（camelCase 载荷）/ Terminal /
    // Escalate（单元变体，失败升格交决策）
    let feedback = OnFail::FeedbackLoop { limit: 5 };
    let wire = serde_json::to_value(feedback).expect("出线应成功");
    assert_eq!(wire, json!({ "feedbackLoop": { "limit": 5 } }));
    let back: OnFail = serde_json::from_value(wire).expect("往返应成功");
    assert_eq!(back, feedback);

    let terminal = OnFail::Terminal;
    let wire = serde_json::to_value(terminal).expect("出线应成功");
    assert_eq!(wire, json!("terminal"));
    let back: OnFail = serde_json::from_value(wire).expect("往返应成功");
    assert_eq!(back, terminal);
    assert_ne!(feedback, terminal, "两变体可辨");

    let escalate = OnFail::Escalate;
    let wire = serde_json::to_value(escalate).expect("出线应成功");
    assert_eq!(
        wire,
        json!("escalate"),
        "单元变体线词逐字（rename_all = camelCase 派生零手写）"
    );
    let back: OnFail = serde_json::from_value(wire).expect("往返应成功");
    assert_eq!(back, escalate, "escalate 往返等值还原");
    assert_ne!(escalate, terminal, "Escalate / Terminal 两单元变体可辨");
    assert_ne!(escalate, feedback, "Escalate / FeedbackLoop 可辨");

    for illegal in [
        "feedbackloop",
        "FeedbackLoop",
        "terminal_fail",
        "escalateLoop",
        "ESCALATE",
        "Escalate",
        "",
    ] {
        let back: Result<OnFail, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "OnFail 越集词 {illegal:?} 应拒绝");
    }
    // null 亦拒绝（缺省语义由 Option 承载，封闭集本身不接受 null）
    let back: Result<OnFail, _> = serde_json::from_value(serde_json::Value::Null);
    assert!(back.is_err(), "OnFail 不接受 null（缺省由 Option 表达）");
}

/// `OnFail` 三变体表驱动往返（正向）：`Escalate` / `Terminal` /
/// `FeedbackLoop { limit: 5 }` 逐变体 serialize → deserialize 等值还原，且三变体
/// 两两可辨（封闭集值域恰三态 + 缺省 None）。
#[test]
fn on_fail三变体表驱动往返() {
    let cases: [OnFail; 3] = [
        OnFail::Escalate,
        OnFail::Terminal,
        OnFail::FeedbackLoop { limit: 5 },
    ];
    let mut wires: Vec<serde_json::Value> = Vec::new();
    for on_fail in cases {
        let wire = serde_json::to_value(on_fail).expect("出线应成功");
        let back: OnFail = serde_json::from_value(wire.clone()).expect("往返应成功");
        assert_eq!(back, on_fail, "变体 {on_fail:?} 往返等值还原");
        wires.push(wire);
    }
    assert_eq!(wires.len(), 3);
    for (idx, wire) in wires.iter().enumerate() {
        for other in wires.iter().skip(idx + 1) {
            assert_ne!(wire, other, "三变体线面两两可辨");
        }
    }
    // 缺省形态：None 与三变体互不可辨——Option::None 不参与封闭集
    let default: Option<OnFail> = None;
    assert!(default.is_none(), "缺省 on_fail = None");
    assert_ne!(
        Some(OnFail::Escalate),
        default,
        "Some(Escalate) 与 None 可辨"
    );
}

/// `StepDefinition.on_fail` 缺省形态（边界）：`None` 与 `Some(OnFail::Escalate)`
/// 可辨；`Copy` / `PartialEq` 派生保留（既有结构体字面量零改动可构造）。
#[test]
fn step_definition_on_fail缺省形态与copy语义() {
    let default = StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    };
    let escalate = StepDefinition {
        node: StepNode::TestExecution,
        prompt_ref: None,
        on_fail: Some(OnFail::Escalate),
    };
    assert!(default.on_fail.is_none(), "缺省步零失败语义声明");
    assert_eq!(escalate.on_fail, Some(OnFail::Escalate), "升格声明逐字段");
    assert_ne!(default.on_fail, escalate.on_fail, "None / Some 可辨");
    // Copy：结构体字面量零改动可复制（const 表内联前提）
    let copied = escalate;
    let again = escalate;
    assert_eq!(copied, again, "Copy 后等值");
    assert_eq!(copied.on_fail, Some(OnFail::Escalate), "Copy 保真");
    assert_ne!(default, escalate, "两态构造可辨");
}

/// `FeedbackLoop` 计数上限为 u32 边界：limit=0 / 1 / u32::MAX 可构造不 panic
///（语义面由 walker 消费，AC-2 / AC-4）。
#[test]
fn on_fail计数上限u32边界可构造() {
    for limit in [0_u32, 1, 5, u32::MAX] {
        let feedback = OnFail::FeedbackLoop { limit };
        assert_eq!(
            feedback,
            OnFail::FeedbackLoop { limit },
            "limit={limit} 可构造且自反"
        );
        assert_ne!(
            feedback,
            OnFail::FeedbackLoop {
                limit: limit.wrapping_add(1)
            },
            "limit 逐字段可辨"
        );
    }
}

/// `StepDefinition` 三字段两态构造（prompt_ref None / Some、on_fail None /
/// Some）+ `ArtifactSpec` 三字段声明面（AC-1）。
#[test]
fn step_definition与artifact_spec声明面() {
    static PROMPT: &str = "静态角色知识（include_str! 编译期装配）";
    let agent_step = StepDefinition {
        node: StepNode::Executor,
        prompt_ref: Some(PROMPT),
        on_fail: None,
    };
    let gate_step = StepDefinition {
        node: StepNode::StaticCheck,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop { limit: 5 }),
    };
    assert_eq!(agent_step.node, StepNode::Executor);
    assert_eq!(agent_step.prompt_ref, Some(PROMPT));
    assert_eq!(agent_step.on_fail, None);
    assert_eq!(gate_step.node, StepNode::StaticCheck);
    assert_eq!(gate_step.prompt_ref, None, "非 agent 步恒 None");
    assert_eq!(gate_step.on_fail, Some(OnFail::FeedbackLoop { limit: 5 }));
    assert_ne!(agent_step, gate_step, "两态构造逐字段可辨");
    assert_eq!(agent_step.clone(), agent_step, "PartialEq 自反");
    // Copy 语义（const 表内联前提）
    let copied = agent_step;
    assert_eq!(copied, agent_step);

    let spec = ArtifactSpec {
        kind: "markdown-doc",
        path: "specs/**",
        title: "规格",
    };
    assert_eq!(spec.kind, "markdown-doc");
    assert_eq!(spec.path, "specs/**");
    assert_eq!(spec.title, "规格");
    assert!(!spec.path.starts_with('/'), "清单路径恒相对");
}

// ---------------------------------------------------------------------------
// 执行图边词汇（RunEdgeKind 四值 + 端点身份串口径）
// ---------------------------------------------------------------------------

/// `RunEdgeKind` 四值 snake_case 逐字出线（forward / feedback / retry /
/// backtrack）；非法词反序列化拒绝（封闭集防线，AC-3）。
#[test]
fn run_edge_kind四值线面snake_case逐字() {
    let wire = |kind: RunEdgeKind| serde_json::to_value(kind).expect("词汇出线应成功");
    assert_eq!(wire(RunEdgeKind::Forward), json!("forward"));
    assert_eq!(wire(RunEdgeKind::Feedback), json!("feedback"));
    assert_eq!(wire(RunEdgeKind::Retry), json!("retry"));
    assert_eq!(wire(RunEdgeKind::Backtrack), json!("backtrack"));
    assert_eq!(RunEdgeKind::Forward.as_str(), "forward");
    assert_eq!(RunEdgeKind::Feedback.as_str(), "feedback");
    assert_eq!(RunEdgeKind::Retry.as_str(), "retry");
    assert_eq!(RunEdgeKind::Backtrack.as_str(), "backtrack");

    // 四值逐值 serialize → deserialize 往返无损（值域仍四值；`retry` 变体存在性
    // 即存量解码契约——旧行信封走 serde_json，删变体将使历史 run 打不开）
    for kind in [
        RunEdgeKind::Forward,
        RunEdgeKind::Feedback,
        RunEdgeKind::Retry,
        RunEdgeKind::Backtrack,
    ] {
        let wire = serde_json::to_value(kind).expect("出线应成功");
        let back: RunEdgeKind = serde_json::from_value(wire.clone()).expect("往返应成功");
        assert_eq!(back, kind, "四值逐值往返无损");
        assert_eq!(kind.as_str(), wire.as_str().expect("线词为字符串"));
    }

    for illegal in ["feed_back", "Feedback", "back_track", "unknown", ""] {
        let back: Result<RunEdgeKind, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "RunEdgeKind 越集词 {illegal:?} 应拒绝");
    }
}

// ---------------------------------------------------------------------------
// 执行图节点 / 边中性类型（序列化往返与两态边界）
// ---------------------------------------------------------------------------

/// `RunNodeStateRecord` 全字段序列化往返：身份键 phase / attempt / step /
/// instance_no 与 status / session_id / detail / output_ref / timestamp 逐字段
/// 可辨（AC-3 / AC-6 线面前置）。
#[test]
fn run_node_state_record全字段往返可辨() {
    let base = RunNodeStateRecord {
        run_id: "run-1727000000000".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        phase: "implement".to_owned(),
        attempt: 1,
        step: RunStepKind::StaticCheck,
        instance_no: 0,
        status: RunStepStatus::Failed,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_060_000),
        token_input: None,
        token_output: None,
        timestamp: 1_727_000_060_000,
    };
    assert_eq!(base.clone(), base, "PartialEq 自反");
    let text = serde_json::to_string(&base).expect("出线应成功");
    assert!(text.contains("\"runId\":\"run-1727000000000\""), "{text}");
    assert!(text.contains("\"instanceNo\":0"), "camelCase 键：{text}");
    let back: RunNodeStateRecord = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, base, "节点中性行往返无损");

    // outputRef / sessionId / detail 三态两态（None 与 Some 逐字段可辨）
    let some = RunNodeStateRecord {
        session_id: Some("ses-1".to_owned()),
        detail: Some("static-check 未过".to_owned()),
        output_ref: Some("exec-7".to_owned()),
        ..base.clone()
    };
    assert_ne!(some, base, "两态逐字段可辨");
    let value = serde_json::to_value(&some).expect("出线应成功");
    assert_eq!(value["outputRef"], json!("exec-7"));
    assert_eq!(value["sessionId"], json!("ses-1"));
    let value = serde_json::to_value(&base).expect("出线应成功");
    assert_eq!(value["outputRef"], json!(null), "None → null 不省键");

    // instance_no 0 / 1 / u32::MAX 三态可构造（身份键边界）
    for instance_no in [0_u32, 1, u32::MAX] {
        let record = RunNodeStateRecord {
            instance_no,
            ..base.clone()
        };
        assert_eq!(record.instance_no, instance_no, "边界值可构造");
        if instance_no == 0 {
            assert_eq!(record, base, "同实例号等值");
        } else {
            assert_ne!(record, base, "instance_no={instance_no} 与首次执行可辨");
        }
    }

    // 二十二值全词汇 step 出线（节点行 zero filter 前置；mutation_test 拆步新词、
    // cleanup 尽力步新臂与 clean 基线恢复步新臂在案）
    for (kind, word) in [
        (RunStepKind::Executor, "executor"),
        (RunStepKind::Evaluator, "evaluator"),
        (RunStepKind::Decision, "decision"),
        (RunStepKind::StaticCheck, "static_check"),
        (RunStepKind::TestExecution, "test_execution"),
        (RunStepKind::MutationTest, "mutation_test"),
        (RunStepKind::Clean, "clean"),
        (RunStepKind::PhaseStart, "phase_start"),
        (RunStepKind::PhaseLog, "phase_log"),
        (RunStepKind::VerdictGate, "verdict_gate"),
        (RunStepKind::RetryGate, "retry_gate"),
        (RunStepKind::WhitelistGate, "whitelist_gate"),
        (RunStepKind::Preflight, "preflight"),
        (RunStepKind::Commit, "commit"),
        (RunStepKind::Rebase, "rebase"),
        (RunStepKind::ResolveConflicts, "resolve_conflicts"),
        (RunStepKind::Merge, "merge"),
        (RunStepKind::SpecSync, "spec_sync"),
        (RunStepKind::Cleanup, "cleanup"),
        (RunStepKind::Seal, "seal"),
        (RunStepKind::Finalize, "finalize"),
    ] {
        let value = serde_json::to_value(RunNodeStateRecord {
            step: kind,
            ..base.clone()
        })
        .expect("出线应成功");
        assert_eq!(value["step"], json!(word));
    }
}

/// `RunEdgeStateRecord` 全字段序列化往返：from_node_key / to_node_key /
/// edge_kind 逐字段可辨（端点身份串口径 `phase:attempt:step:instance_no`）。
#[test]
fn run_edge_state_record全字段往返可辨() {
    let base = RunEdgeStateRecord {
        run_id: "run-1727000000000".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        from_node_key: "implement:1:executor:0".to_owned(),
        to_node_key: "implement:1:static_check:0".to_owned(),
        edge_kind: RunEdgeKind::Feedback,
        timestamp: 1_727_000_060_000,
    };
    assert_eq!(base.clone(), base);
    let text = serde_json::to_string(&base).expect("出线应成功");
    assert!(
        text.contains("\"fromNodeKey\":\"implement:1:executor:0\""),
        "{text}"
    );
    assert!(text.contains("\"edgeKind\":\"feedback\""), "{text}");
    let back: RunEdgeStateRecord = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, base, "边中性行往返无损");

    assert_ne!(
        RunEdgeStateRecord {
            to_node_key: "implement:1:static_check:1".to_owned(),
            ..base.clone()
        },
        base,
        "to_node_key 逐字段可辨（实例号段）"
    );
    assert_ne!(
        RunEdgeStateRecord {
            edge_kind: RunEdgeKind::Retry,
            ..base.clone()
        },
        base,
        "edge_kind 逐字段可辨（feedback 与 retry 不混淆）"
    );
    // 非法 edge_kind 词反序列化拒绝（封闭集防线）
    let mut value = serde_json::to_value(&base).expect("出线应成功");
    value["edgeKind"] = json!("feed_back");
    let back: Result<RunEdgeStateRecord, _> = serde_json::from_value(value);
    assert!(back.is_err(), "边行非法 kind 词应拒绝");
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（AC-1 / AC-5）：RunNodeStateRecord /
// RunNodeEntry 四字段（startedAt / endedAt / tokenInput / tokenOutput）——
// camelCase 线面 / 往返无损 / null 不省键 / 0 值原样 / 旧形态拒绝 / 写读同一
// 份字段集 / 防御值直存零钳制
// ---------------------------------------------------------------------------

/// 正向：四字段全值 `serde_json::to_string` → camelCase 四键逐键等值（AC-1）。
#[test]
fn run_node_state_record四字段全值线面camelcase逐键() {
    let record = RunNodeStateRecord {
        run_id: "run-1727000000000".to_owned(),
        change_id: "demo-change".to_owned(),
        phase: "implement".to_owned(),
        attempt: 1,
        step: RunStepKind::Executor,
        instance_no: 0,
        status: RunStepStatus::Passed,
        session_id: Some("ses-1".to_owned()),
        detail: None,
        output_ref: None,
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_200_000),
        token_input: Some(12_000),
        token_output: Some(3_400),
        timestamp: 1_727_000_300_000,
    };
    let value = serde_json::to_value(&record).expect("出线应成功");
    assert_eq!(
        value["startedAt"],
        json!(1_727_000_000_000_i64),
        "startedAt camelCase 键数值口径"
    );
    assert_eq!(value["endedAt"], json!(1_727_000_200_000_i64));
    assert_eq!(value["tokenInput"], json!(12_000));
    assert_eq!(value["tokenOutput"], json!(3_400));
}

/// 正向：to_string → from_str 往返 → 四字段逐字段还原无损（既有往返用例
/// 随动扩断言）。
#[test]
fn run_node_state_record四字段往返还原无损() {
    let record = RunNodeStateRecord {
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_200_000),
        token_input: Some(12_000),
        token_output: Some(3_400),
        ..RunNodeStateRecord {
            run_id: "run-rt".to_owned(),
            change_id: "demo-change".to_owned(),
            phase: "implement".to_owned(),
            attempt: 1,
            step: RunStepKind::Evaluator,
            instance_no: 1,
            status: RunStepStatus::Failed,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 0,
            ended_at: None,
            token_input: None,
            token_output: None,
            timestamp: 1,
        }
    };
    let text = serde_json::to_string(&record).expect("出线应成功");
    let back: RunNodeStateRecord = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, record, "四字段往返无损");
    assert_eq!(back.started_at, 1_727_000_000_000);
    assert_eq!(back.ended_at, Some(1_727_000_200_000));
    assert_eq!(back.token_input, Some(12_000));
    assert_eq!(back.token_output, Some(3_400));
}

/// 边界：token / ended None 形态 → 线面三键 `null` 不省键（沿 node_none 两态
/// 式样随动）。
#[test]
fn run_node_state_record_none形态三键null不省键() {
    let record = RunNodeStateRecord {
        ended_at: None,
        token_input: None,
        token_output: None,
        ..RunNodeStateRecord {
            run_id: "run-none".to_owned(),
            change_id: "demo-change".to_owned(),
            phase: "implement".to_owned(),
            attempt: 1,
            step: RunStepKind::StaticCheck,
            instance_no: 0,
            status: RunStepStatus::Running,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 1_727_000_000_000,
            ended_at: Some(1),
            token_input: Some(1),
            token_output: Some(1),
            timestamp: 1,
        }
    };
    let value = serde_json::to_value(&record).expect("出线应成功");
    assert_eq!(value["endedAt"], json!(null));
    assert_eq!(value["tokenInput"], json!(null));
    assert_eq!(value["tokenOutput"], json!(null));
    let object = value.as_object().expect("对象线面");
    assert!(
        object.contains_key("endedAt")
            && object.contains_key("tokenInput")
            && object.contains_key("tokenOutput"),
        "三键在场（null 留位不省键）"
    );
}

/// 边界：started_at = 0 → 线面 `0` 原样（port 读面缺省语义值零改写）。
#[test]
fn run_node_state_record_started_at零值线面原样() {
    let record = RunNodeStateRecord {
        started_at: 0,
        ..RunNodeStateRecord {
            run_id: "run-zero".to_owned(),
            change_id: "demo-change".to_owned(),
            phase: "implement".to_owned(),
            attempt: 1,
            step: RunStepKind::PhaseStart,
            instance_no: 0,
            status: RunStepStatus::Passed,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 9,
            ended_at: None,
            token_input: None,
            token_output: None,
            timestamp: 1,
        }
    };
    let value = serde_json::to_value(&record).expect("出线应成功");
    assert_eq!(value["startedAt"], json!(0), "0 缺省语义值原样出线");
}

/// 异常：缺四键旧形态 JSON 反序列化 → Err（port 读面无 `#[serde(default)]`
/// ——旧行兼容仅落库 `RunNodeRecord` 面承载，两载体语义分立锚，AC-5）。
#[test]
fn run_node_state_record缺四键旧形态反序列化拒绝() {
    let legacy = json!({
        "runId": "run-legacy",
        "changeId": "demo-change",
        "phase": "implement",
        "attempt": 1,
        "step": "executor",
        "instanceNo": 0,
        "status": "passed",
        "sessionId": null,
        "detail": null,
        "outputRef": null,
        "timestamp": 1_727_000_300_000_i64
    });
    let back: Result<RunNodeStateRecord, _> = serde_json::from_value(legacy);
    assert!(
        back.is_err(),
        "缺四键旧形态应 Err（非 Option 的 startedAt 缺席即拒——port 读面无 default）"
    );
}

/// 正向（写面 / 读面同一份字段集锚，AC-1）：RunNodeEntry 四字段构造 → 与
/// RunNodeStateRecord 同名字段集逐字段可对齐。
#[test]
fn run_node_entry与state_record同名字段集逐字段可对齐() {
    let entry = RunNodeEntry {
        phase: "implement".to_owned(),
        attempt: 1,
        step: RunStepKind::Executor,
        instance_no: 0,
        status: RunStepStatus::Passed,
        session_id: Some("ses-1".to_owned()),
        detail: Some("实现完成".to_owned()),
        output_ref: None,
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_200_000),
        token_input: Some(12_000),
        token_output: Some(3_400),
    };
    let record = RunNodeStateRecord {
        run_id: "run-align".to_owned(),
        change_id: "demo-change".to_owned(),
        phase: entry.phase.clone(),
        attempt: entry.attempt,
        step: entry.step,
        instance_no: entry.instance_no,
        status: entry.status,
        session_id: entry.session_id.clone(),
        detail: entry.detail.clone(),
        output_ref: entry.output_ref,
        started_at: entry.started_at,
        ended_at: entry.ended_at,
        token_input: entry.token_input,
        token_output: entry.token_output,
        timestamp: 1_727_000_300_000,
    };
    // 四字段逐字段对齐（写面载荷 = 读面快照同名字段集）
    assert_eq!(entry.started_at, record.started_at);
    assert_eq!(entry.ended_at, record.ended_at);
    assert_eq!(entry.token_input, record.token_input);
    assert_eq!(entry.token_output, record.token_output);
}

/// 边界：四字段 0 / None 边界形态构造 → 直存零转换零默认（payload 面零逻辑
/// 分支）。
#[test]
fn run_node_entry边界形态直存零转换零默认() {
    let entry = RunNodeEntry {
        started_at: 0,
        ended_at: None,
        token_input: None,
        token_output: None,
        ..RunNodeEntry {
            phase: "implement".to_owned(),
            attempt: 1,
            step: RunStepKind::VerdictGate,
            instance_no: 0,
            status: RunStepStatus::Stopped,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 9,
            ended_at: Some(9),
            token_input: Some(9),
            token_output: Some(9),
        }
    };
    assert_eq!(entry.started_at, 0, "0 直存（零转换）");
    assert_eq!(entry.ended_at, None, "None 直存（零默认）");
    assert_eq!(entry.token_input, None);
    assert_eq!(entry.token_output, None);
}

/// 异常：防御值构造（started_at 负值 / token u64::MAX）→ 直存零钳制零 panic
///（载荷面零校验——校验归铸造侧锚）。
#[test]
fn run_node_entry防御值直存零钳制零panic() {
    let entry = RunNodeEntry {
        started_at: -1,
        ended_at: Some(-2),
        token_input: Some(u64::MAX),
        token_output: Some(u64::MAX - 1),
        ..RunNodeEntry {
            phase: "implement".to_owned(),
            attempt: 1,
            step: RunStepKind::Executor,
            instance_no: 0,
            status: RunStepStatus::Passed,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 0,
            ended_at: None,
            token_input: None,
            token_output: None,
        }
    };
    assert_eq!(entry.started_at, -1, "负值直存零钳制");
    assert_eq!(entry.ended_at, Some(-2));
    assert_eq!(entry.token_input, Some(u64::MAX), "u64::MAX 直存零钳制");
    assert_eq!(entry.token_output, Some(u64::MAX - 1));
}

// ---------------------------------------------------------------------------
// archi_check 词面（AC-16）：三枚举尾部追加 + 派生访问器
// ---------------------------------------------------------------------------

/// 正向（AC-16）：`StepNode::ArchiCheck` 五面——`as_str` 词面、serde 线词、
/// `group()` 归 `Exec`、`role()` 为 `None`、末位下标（R-8 尾部追加）。
#[test]
fn step_node_archi_check五面() {
    assert_eq!(StepNode::ArchiCheck.as_str(), "archi_check", "线格式词面");
    let wire = serde_json::to_value(StepNode::ArchiCheck).expect("节点出线应成功");
    assert_eq!(wire, json!("archi_check"), "serde 与 as_str 一字不差");
    assert_eq!(
        StepNode::ArchiCheck.group(),
        StepGroup::Exec,
        "工具步归 Exec 组"
    );
    assert_eq!(StepNode::ArchiCheck.role(), None, "工具步无 agent 角色");
    assert_eq!(
        StepNode::ArchiCheck as usize,
        21,
        "尾部追加：既有 0..=20 下标零位移"
    );
}

/// 正向：`RunStepKind::ArchiCheck` 词面与末位下标。
#[test]
fn run_step_kind_archi_check词面与末位() {
    assert_eq!(
        RunStepKind::ArchiCheck.as_str(),
        "archi_check",
        "落库 snake_case 词面"
    );
    let wire = serde_json::to_value(RunStepKind::ArchiCheck).expect("出线应成功");
    assert_eq!(wire, json!("archi_check"), "serde 线词同源");
    assert_eq!(RunStepKind::ArchiCheck as usize, 21, "尾部追加末位");
}

/// 正向（AC-16）：`StepNode::ArchiCheck` 与 `RunStepKind::ArchiCheck` 逐字同源
/// （词面 1:1 对应）。
#[test]
fn step_node与run_step_kind的archi_check同源() {
    assert_eq!(
        StepNode::ArchiCheck.as_str(),
        RunStepKind::ArchiCheck.as_str(),
        "两侧词面一字不差"
    );
}

/// 正向：审计包络 `StepKind::ArchiCheck` 九臂词面 + 末位下标。
#[test]
fn step_kind_archi_check词面与末位() {
    assert_eq!(StepKind::ArchiCheck.as_str(), "archi_check", "审计词面");
    assert_eq!(StepKind::ArchiCheck as usize, 8, "八臂 → 九臂末位");
    // 八既有臂逐字不变
    for (kind, word) in [
        (StepKind::PhaseNext, "phase_next"),
        (StepKind::PhaseStart, "phase_start"),
        (StepKind::PhaseLog, "phase_log"),
        (StepKind::Backtrack, "backtrack"),
        (StepKind::DecisionLog, "decision_log"),
        (StepKind::StaticCheck, "static_check"),
        (StepKind::TestExecution, "test_execution"),
        (StepKind::MutationTest, "mutation_test"),
    ] {
        assert_eq!(kind.as_str(), word, "既有臂词面零回归");
    }
}

/// 异常（snake_case 封闭）：`ArchiCheck` / `archiCheck` / `archi-check` / 空串在
/// `StepNode` 与 `RunStepKind` 上均反序列化失败。
#[test]
fn archi_check词汇外形态拒绝() {
    for raw in [
        "\"ArchiCheck\"",
        "\"archiCheck\"",
        "\"archi-check\"",
        "\"\"",
    ] {
        let node: Result<StepNode, _> = serde_json::from_str(raw);
        assert!(node.is_err(), "StepNode 词集外的 {raw} 必须 Err");
        let kind: Result<RunStepKind, _> = serde_json::from_str(raw);
        assert!(kind.is_err(), "RunStepKind 词集外的 {raw} 必须 Err");
    }
}
