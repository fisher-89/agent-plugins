use std::sync::Arc;

use workflow::state::{
    ChangeStateStore, RunEdgeEntry, RunEdgeKind, RunFinishCommand, RunNodeEntry, RunStartCommand,
    RunStatus, RunStepKind, RunStepStatus,
};
use workflow::write::{run_finish as write_run_finish, run_start as write_run_start};

use crate::port::RunHistoryPort;
use crate::state::{
    ChangeRunStatus, ChangeStepEdge, ChangeStepKind, ChangeStepState, ChangeStepStatus, EdgeKind,
};
use crate::walker::RunRequest;

/// 步词汇直映射（**二十二值 1:1 全映射，零过滤**）：编排侧 `ChangeStepKind` 与落
/// 库侧 `RunStepKind` 同词面封闭集逐一对应——`step` 语义无弃落项。
fn mapped_step(kind: ChangeStepKind) -> RunStepKind {
    match kind {
        ChangeStepKind::Executor => RunStepKind::Executor,
        ChangeStepKind::Evaluator => RunStepKind::Evaluator,
        ChangeStepKind::Decision => RunStepKind::Decision,
        ChangeStepKind::StaticCheck => RunStepKind::StaticCheck,
        ChangeStepKind::TestExecution => RunStepKind::TestExecution,
        ChangeStepKind::MutationTest => RunStepKind::MutationTest,
        ChangeStepKind::Clean => RunStepKind::Clean,
        ChangeStepKind::PhaseStart => RunStepKind::PhaseStart,
        ChangeStepKind::PhaseLog => RunStepKind::PhaseLog,
        ChangeStepKind::VerdictGate => RunStepKind::VerdictGate,
        ChangeStepKind::RetryGate => RunStepKind::RetryGate,
        ChangeStepKind::WhitelistGate => RunStepKind::WhitelistGate,
        ChangeStepKind::Preflight => RunStepKind::Preflight,
        ChangeStepKind::Commit => RunStepKind::Commit,
        ChangeStepKind::Rebase => RunStepKind::Rebase,
        ChangeStepKind::ResolveConflicts => RunStepKind::ResolveConflicts,
        ChangeStepKind::Merge => RunStepKind::Merge,
        ChangeStepKind::SpecSync => RunStepKind::SpecSync,
        ChangeStepKind::Cleanup => RunStepKind::Cleanup,
        ChangeStepKind::Seal => RunStepKind::Seal,
        ChangeStepKind::Finalize => RunStepKind::Finalize,
        ChangeStepKind::ArchiCheck => RunStepKind::ArchiCheck,
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

/// 边 kind 直映射（同类型恒等：编排侧 [`EdgeKind`] = `RunEdgeKind`，词汇单点驻
/// `workflow::state`——本函数只为落库面显名，零转换）。
fn persisted_edge_kind(kind: EdgeKind) -> RunEdgeKind {
    kind
}

/// run 收口写命令组装（walker 终态出口单点消费）：累积器节点快照（全词汇 emit
/// 序，稳定身份键）直映射 `RunNodeEntry` 整包（**零过滤**），边快照直映射
/// `RunEdgeEntry` 整包（端点身份串 = `"{phase}:{attempt}:{step}:{instance_no}"`）；
/// session_id / detail / output_ref 与步级起止时间、token 四字段透传；
/// `finished_at` 命令携带（整包同刻，corpus 确定性）。
pub fn finish_command(
    request: &RunRequest,
    status: ChangeRunStatus,
    reason: Option<String>,
    finished_at: i64,
    nodes: &[ChangeStepState],
    edges: &[ChangeStepEdge],
) -> RunFinishCommand {
    let node_entries: Vec<RunNodeEntry> = nodes
        .iter()
        .map(|node| RunNodeEntry {
            phase: node.phase.clone(),
            attempt: node.attempt,
            step: mapped_step(node.step),
            instance_no: node.instance_no,
            status: persisted_status(node.status),
            session_id: node.session_id.clone(),
            detail: node.detail.clone(),
            output_ref: node.output_ref.clone(),
            started_at: node.started_at,
            ended_at: node.ended_at,
            token_input: node.token_input,
            token_output: node.token_output,
        })
        .collect();
    let edge_entries: Vec<RunEdgeEntry> = edges
        .iter()
        .map(|edge| RunEdgeEntry {
            from_node_key: edge.from_node_key(),
            to_node_key: edge.to_node_key(),
            edge_kind: persisted_edge_kind(edge.kind),
        })
        .collect();
    RunFinishCommand {
        run_id: request.run_id.clone(),
        change_id: request.change_id.clone(),
        status: run_status(status),
        reason,
        finished_at,
        nodes: node_entries,
        edges: edge_entries,
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
