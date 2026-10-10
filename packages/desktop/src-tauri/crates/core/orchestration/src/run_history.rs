//! run 落库 port 缝适配（unify-run-state-persistence）：步词汇 10→5 过滤
//! 单点 + finish 组装 helper + store 适配器。
//!
//! **过滤单点纪律（D2 / AC-3）**：词汇本体 = `workflow::state::RunStepKind`
//! 五值封闭集，落库命令 `RunStepEntry.step` 即该类型——store 结构上收不到
//! 忽略集，10→5 映射的唯一消费点 = 本模块 [`persisted_step`]；`walker.rs` /
//! `control.rs` / `steps.rs` 源码零过滤逻辑（grep 可验）。

use std::sync::Arc;

use workflow::state::{
    ChangeStateStore, RunFinishCommand, RunStartCommand, RunStatus, RunStepEntry, RunStepKind,
    RunStepStatus,
};
use workflow::write::{run_finish as write_run_finish, run_start as write_run_start};

use crate::port::RunHistoryPort;
use crate::state::{ChangeRunStatus, ChangeStepKind, ChangeStepState, ChangeStepStatus};
use crate::walker::RunRequest;

/// 步词汇 10→5 过滤（**唯一映射消费点**）：executor / evaluator / decision /
/// static_check / test_execution 落库，phase_start / phase_log /
/// verdict_gate / retry_gate / whitelist_gate 弃——流程面步骤与三门纯 Rust
/// 分支无会话无独立产物，图史价值为零（attempt 级评估史已由 PhaseRecord 承
/// 载）。词汇本体在 `workflow::state::RunStepKind`，本函数是唯一映射消费点。
pub fn persisted_step(kind: ChangeStepKind) -> Option<RunStepKind> {
    match kind {
        ChangeStepKind::Executor => Some(RunStepKind::Executor),
        ChangeStepKind::Evaluator => Some(RunStepKind::Evaluator),
        ChangeStepKind::Decision => Some(RunStepKind::Decision),
        ChangeStepKind::StaticCheck => Some(RunStepKind::StaticCheck),
        ChangeStepKind::TestExecution => Some(RunStepKind::TestExecution),
        ChangeStepKind::PhaseStart
        | ChangeStepKind::PhaseLog
        | ChangeStepKind::VerdictGate
        | ChangeStepKind::RetryGate
        | ChangeStepKind::WhitelistGate => None,
    }
}

/// 步状态直映射（四值词汇两域同形，一一同型对应）。
fn persisted_status(status: ChangeStepStatus) -> RunStepStatus {
    match status {
        ChangeStepStatus::Running => RunStepStatus::Running,
        ChangeStepStatus::Passed => RunStepStatus::Passed,
        ChangeStepStatus::Failed => RunStepStatus::Failed,
        ChangeStepStatus::Stopped => RunStepStatus::Stopped,
    }
}

/// run 收口写命令组装（walker 终态出口单点消费）：累积器快照（全词汇 emit
/// 序）经 [`persisted_step`] 过滤落 5 类子集，`seq` = 全词汇 emit 序盖戳（累
/// 积器下标即 emit 序——被过滤步占号产生的库内空洞为排序键语义合法形态），
/// session_id / detail 透传；`finished_at` 命令携带（整包同刻，corpus 确定
/// 性）。
pub fn finish_command(
    request: &RunRequest,
    status: ChangeRunStatus,
    reason: Option<String>,
    finished_at: i64,
    steps: &[ChangeStepState],
) -> RunFinishCommand {
    let mut entries: Vec<RunStepEntry> = Vec::with_capacity(steps.len());
    for (seq, step) in steps.iter().enumerate() {
        let Some(kind) = persisted_step(step.step) else {
            continue; // 流程面步骤与三门：不落库（占号产生库内空洞，合法）
        };
        entries.push(RunStepEntry {
            seq: seq as u64,
            phase: step.phase.clone(),
            attempt: step.attempt,
            step: kind,
            status: persisted_status(step.status),
            session_id: step.session_id.clone(),
            detail: step.detail.clone(),
        });
    }
    RunFinishCommand {
        run_id: request.run_id.clone(),
        change: request.change.clone(),
        status: run_status(status),
        reason,
        finished_at,
        steps: entries,
    }
}

/// run 终态映射：`ChangeRunStatus` 三终态 → `RunStatus` 同名词汇（停等两态
/// 不可达——finish 只在终态出口调用；漂移形态防御性收敛 Failed）。
fn run_status(status: ChangeRunStatus) -> RunStatus {
    match status {
        ChangeRunStatus::Completed => RunStatus::Completed,
        ChangeRunStatus::Stopped => RunStatus::Stopped,
        ChangeRunStatus::Failed => RunStatus::Failed,
        ChangeRunStatus::Running
        | ChangeRunStatus::WaitingConfirm
        | ChangeRunStatus::WaitingAsk => RunStatus::Failed,
    }
}

/// [`RunHistoryPort`] 的 store 适配器（组合根装配）：写面校验与落库全归
/// `workflow::write` 单点（run_start / run_finish），本适配器零业务逻辑。
pub struct StoreRunHistory {
    store: Arc<dyn ChangeStateStore>,
}

impl StoreRunHistory {
    /// 绑定 store 缝构造（`for_root` 实例克隆，与写面 / 快照源同实例）。
    pub fn new(store: Arc<dyn ChangeStateStore>) -> Self {
        Self { store }
    }
}

impl RunHistoryPort for StoreRunHistory {
    fn run_started(&self, command: &RunStartCommand) -> Result<(), String> {
        write_run_start(self.store.as_ref(), command)
    }

    fn run_finished(&self, command: &RunFinishCommand) -> Result<(), String> {
        write_run_finish(self.store.as_ref(), command)
    }
}

const _: () = {
    // port 契约 Send + Sync 硬校验（跨线程经 Arc<dyn RunHistoryPort> 注入）
    const fn assert_port<T: RunHistoryPort>() {}
    assert_port::<StoreRunHistory>();
};
