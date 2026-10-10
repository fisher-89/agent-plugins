//! `state.rs` 中性状态类型的单元测试（test-design「state.rs ->
//! state_test.rs」节）：ChangeStateRecord 身份面（`id` 首字段为身份锚、`name`
//! 为无唯一约束属性——同名不同 id 不等价）+ 五写命令载荷 `change` →
//! `change_id` 改名后等值面重证（PartialEq 逐字段可辨）+ run 域词汇线面封闭集
//!（RunStatus 五值 lowercase / RunStepKind 五值 snake_case / RunStepStatus 四值
//! camelCase）逐字断言 + 非法词反序列化拒绝（封闭集防线——零触点持衡）+
//! run 命令与记录结构两态构造与等值面（reason / session_id / detail /
//! finished_at None 与 Some）+ RunFinishCommand 终态三值约束为写面职责（类型
//! 层可构造——拦截断言挂 write/run_test.rs 与 store_test.rs）+
//! ChangeStateStore trait 全方法 id 形参编译面（`get_change(id)` /
//! `start_phase(change_id, …)` / `list_runs(change_id)` 等——最小假件实现后可
//! 调；行为断言归 write 各节假件与 infra/store 真件节）。
//!
//! Mock策略（test-design 本节 Mock 表）：无（真实组合）——纯类型 serde 直测与
//! 纯内存等值面，零依赖零 mock。

use serde_json::json;

use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, PhaseStateRecord, RunFinishCommand, RunStartCommand,
    RunStartCommand as RunStartCmd, RunStateRecord, RunStatus, RunStepEntry, RunStepKind,
    RunStepStateRecord, RunStepStatus, StepCommand, StepKind, StepStateRecord, StoreFault,
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

/// RunStepKind 五值出线恰为 snake_case 词（AC-3 词汇单点前置——落库命令与
/// 查询投影同类型消费）。
#[test]
fn run_step_kind五值线面snake_case逐字() {
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

    assert_eq!(RunStepKind::Executor.as_str(), "executor");
    assert_eq!(RunStepKind::Evaluator.as_str(), "evaluator");
    assert_eq!(RunStepKind::Decision.as_str(), "decision");
    assert_eq!(RunStepKind::StaticCheck.as_str(), "static_check");
    assert_eq!(RunStepKind::TestExecution.as_str(), "test_execution");
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
    // RunStepKind：流程面词 / camel 词 / 三门词结构上不可表达
    for illegal in [
        "phase_start",
        "verdictGate",
        "staticCheck",
        "testExecution",
        "gate",
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

    // 步史读记录：session_id / detail 两态
    let step_none = RunStepStateRecord {
        seq: 0,
        run_id: "run-1727000000000".to_owned(),
        phase: "implement".to_owned(),
        attempt: 1,
        step: RunStepKind::StaticCheck,
        status: RunStepStatus::Passed,
        session_id: None,
        detail: None,
        timestamp: 1_727_000_060_000,
    };
    assert_eq!(step_none.session_id, None);
    assert_eq!(step_none.detail, None);
    let step_some = RunStepStateRecord {
        step: RunStepKind::Executor,
        status: RunStepStatus::Failed,
        session_id: Some("ses-1".to_owned()),
        detail: Some("会话失败收敛".to_owned()),
        ..step_none.clone()
    };
    assert_ne!(step_some, step_none, "两态逐字段可辨");
    let text = serde_json::to_string(&step_none).expect("出线应成功");
    let back: RunStepStateRecord = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, step_none, "步史记录往返无损");
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

    let entry = RunStepEntry {
        seq: 3,
        phase: "implement".to_owned(),
        attempt: 2,
        step: RunStepKind::Executor,
        status: RunStepStatus::Passed,
        session_id: Some("ses-1".to_owned()),
        detail: None,
    };
    let finish = RunFinishCommand {
        run_id: "run-1727000000000".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        status: RunStatus::Completed,
        reason: Some("All phases have passed.".to_owned()),
        finished_at: 1_727_000_060_000,
        steps: vec![entry.clone()],
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
            steps: Vec::new(),
            ..finish.clone()
        },
        finish,
        "steps 空包/整包可辨（零步 run 收口边界形态合法）"
    );
    // 条目字段逐项透传面
    assert_eq!(entry.seq, 3);
    assert_eq!(entry.step, RunStepKind::Executor);
    assert_eq!(entry.session_id.as_deref(), Some("ses-1"));
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
            steps: Vec::new(),
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
        fn list_run_steps(&self, run_id: &str) -> Result<Vec<RunStepStateRecord>, StoreFault> {
            assert_eq!(run_id, "run-1");
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
        probe.list_run_steps("run-1").expect("读面应可用"),
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
            steps: Vec::new(),
        })
        .expect("写面应可用");

    // Send + Sync 硬校验（跨线程经 Arc<dyn ChangeStateStore> 注入的编译前提）
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SignatureProbe>();
    assert_send_sync::<ChangeStatus>();
}
