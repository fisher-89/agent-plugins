use agent::{RunningTurn, TurnOutcome, TurnSummary};
use serde::Serialize;
use specta::Type;

/// `agent_start` Channel 的消息信封（app 层 IPC 类型，非 core 契约）：实时
/// 事件与终态轮行双变体，tag `ipc` 判别（TS 镜像放 transport，camelCase
/// 对齐）。信封不含 record 语义——终态轮行塞进事件 usage 是反模式。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "ipc", rename_all = "camelCase")]
pub enum AgentRunMessage {
    /// 实时事件（落库与流出同源同构）
    Event { event: agent::AgentEvent },
    /// 终态轮行（提前 resolve 契约的终态流出半边，与重放两路同构）
    Record { record: TurnSummary },
}

/// running 态轮行装配（提前 resolve 返回值；自内核中性数据 `RunningTurn`
/// 装配，编排体已下沉内核）。
pub(crate) fn running_summary(running: &RunningTurn) -> TurnSummary {
    TurnSummary {
        turn_id: running.turn_id,
        session_id: running.session_id.clone(),
        status: agent::AgentRunStatus::Running,
        started_at: running.started_at,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        error: None,
    }
}

/// 终态轮行装配（Channel `Record` 信封；自内核中性数据 `TurnOutcome` 装配，
/// 会话 id 与 `started_at` 自轮发起时刻回填——终态收口不携带起点）。
pub(crate) fn outcome_summary(
    outcome: &TurnOutcome,
    session_id: &str,
    started_at: i64,
) -> TurnSummary {
    TurnSummary {
        turn_id: outcome.turn_id,
        session_id: session_id.to_owned(),
        status: outcome.status,
        started_at,
        finished_at: Some(outcome.finished_at),
        num_turns: outcome.num_turns,
        cost_usd: outcome.cost_usd,
        duration_ms: outcome.duration_ms,
        error: outcome.error.clone(),
    }
}

#[cfg(test)]
#[path = "agent_test.rs"]
pub(crate) mod agent_test;
