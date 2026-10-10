//! `state.rs` run 运行史中性类型的单元测试（test-design「state.rs ->
//! state_test.rs」节）：run 域词汇线面封闭集（RunStatus 五值 lowercase /
//! RunStepKind 五值 snake_case / RunStepStatus 四值 camelCase）逐字断言 +
//! 非法词反序列化拒绝（封闭集防线，AC-3 词汇单点前置）+ run 命令与记录结构
//! 两态构造与等值面（reason / session_id / detail / finished_at None 与
//! Some）+ RunFinishCommand 终态三值约束为写面职责（类型层可构造——拦截断
//! 言挂 write/run_test.rs 与 store_test.rs）+ ChangeStateStore run 域四方法
//! trait 编译面（行为断言归 write/run_test.rs 假件与 infra/store 真件节）。
//!
//! Mock策略（test-design 本节 Mock 表）：无（真实组合）——纯类型 serde 直测，
//! 零依赖零 mock。

use serde_json::json;

use crate::state::{
    ChangeStateStore, ChangeStatus, RunFinishCommand, RunStartCommand,
    RunStartCommand as RunStartCmd, RunStateRecord, RunStatus, RunStepEntry, RunStepKind,
    RunStepStateRecord, RunStepStatus, StoreFault,
};

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
        change: "demo-change".to_owned(),
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
/// 命令纯 derive（零 serde——进程内写面载荷），PartialEq 逐字段可辨。
#[test]
fn run命令两态构造与等值面() {
    let start = RunStartCommand {
        run_id: "run-1727000000000".to_owned(),
        change: "demo-change".to_owned(),
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
        change: "demo-change".to_owned(),
        status: RunStatus::Completed,
        reason: Some("All phases have passed.".to_owned()),
        finished_at: 1_727_000_060_000,
        steps: vec![entry.clone()],
    };
    assert_eq!(finish.clone(), finish);
    // reason / steps 两态翻转不等值
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
            change: "demo-change".to_owned(),
            status,
            reason: None,
            finished_at: 1_727_000_060_000,
            steps: Vec::new(),
        };
        assert_eq!(command.status, status, "五值均可构造（类型层零拦截）");
    }
}

// ---------------------------------------------------------------------------
// ChangeStateStore run 域四方法：trait 编译面（行为断言归实现侧测试承载）
// ---------------------------------------------------------------------------

/// trait 四方法签名编译面：最小假件实现后 list_runs / list_run_steps /
/// run_start / run_finish 可调（本节不重复行为断言——写面假件行为挂
/// write/run_test.rs、infra/store 真件行为挂 store_test.rs / change_port_test.rs）。
#[test]
fn change_state_store_run域四方法trait编译面() {
    struct SignatureProbe;

    impl ChangeStateStore for SignatureProbe {
        fn get_change(
            &self,
            _name: &str,
        ) -> Result<Option<crate::state::ChangeStateRecord>, StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn list_change_records(&self) -> Result<Vec<crate::state::ChangeStateRecord>, StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn list_phase_records(
            &self,
            _change: &str,
        ) -> Result<Vec<crate::state::PhaseStateRecord>, StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn list_steps(
            &self,
            _change: &str,
            _run_id: Option<&str>,
        ) -> Result<Vec<crate::state::StepStateRecord>, StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn create_change_record(
            &self,
            _record: crate::state::ChangeStateRecord,
        ) -> Result<(), StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn delete_change_record(&self, _name: &str) -> Result<bool, StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn start_phase(
            &self,
            _change: &str,
            _phase: &str,
            _now: i64,
        ) -> Result<crate::state::PhaseStartState, StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn log_phase(&self, _command: &crate::state::PhaseLogCommand) -> Result<u32, StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn apply_backtrack(
            &self,
            _command: &crate::state::BacktrackCommand,
        ) -> Result<(), StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn amend_decision_session(
            &self,
            _change: &str,
            _phase: &str,
            _session_id: &str,
        ) -> Result<(), StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn set_archived(&self, _name: &str, _archived_at: i64) -> Result<(), StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn append_step(&self, _command: &crate::state::StepCommand) -> Result<(), StoreFault> {
            unimplemented!("编译面探针不可达")
        }
        fn list_runs(&self, _change: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
            Ok(Vec::new())
        }
        fn list_run_steps(&self, _run_id: &str) -> Result<Vec<RunStepStateRecord>, StoreFault> {
            Ok(Vec::new())
        }
        fn run_start(&self, _command: &RunStartCommand) -> Result<(), StoreFault> {
            Ok(())
        }
        fn run_finish(&self, _command: &RunFinishCommand) -> Result<(), StoreFault> {
            Ok(())
        }
    }

    let probe = SignatureProbe;
    assert_eq!(
        probe.list_runs("demo-change").expect("读面应可用"),
        Vec::new()
    );
    assert_eq!(
        probe.list_run_steps("run-1").expect("读面应可用"),
        Vec::new(),
        "miss 非错误（空数组）"
    );
    probe
        .run_start(&RunStartCommand {
            run_id: "run-1".to_owned(),
            change: "demo-change".to_owned(),
            started_at: 1_727_000_000_000,
        })
        .expect("写面应可用");
    probe
        .run_finish(&RunFinishCommand {
            run_id: "run-1".to_owned(),
            change: "demo-change".to_owned(),
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
