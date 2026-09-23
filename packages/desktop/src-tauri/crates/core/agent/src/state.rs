//! run 状态机：running → completed | failed，由 `RunResult.is_error` 驱动
//! 收敛；收敛后拒绝一切状态变更（幂等终态）。

use crate::event::{AgentEvent, AgentEventKind};

/// run 生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRunState {
    /// 运行中（初态）
    Running,
    /// 正常收敛（result 事件 is_error=false）
    Completed,
    /// 失败收敛（result 事件 is_error=true）
    Failed,
}

/// 收敛状态机：`apply` 按事件流推进，[`AgentRunState::Running`] 之后由首个
/// `RunResult` 事件一次性收敛，此后 `apply` 不再改变状态。
#[derive(Debug, Clone)]
pub struct RunStateMachine {
    state: AgentRunState,
}

impl RunStateMachine {
    /// 新状态机：初始 [`AgentRunState::Running`]。
    pub fn new() -> Self {
        Self {
            state: AgentRunState::Running,
        }
    }

    /// 应用一个事件并返回应用后的状态：仅 `RunResult` 驱动收敛（is_error
    /// → Failed / 否则 Completed），其余事件不改变状态；已收敛则原样返回。
    pub fn apply(&mut self, event: &AgentEvent) -> AgentRunState {
        if self.state == AgentRunState::Running {
            if let AgentEventKind::RunResult { is_error, .. } = &event.kind {
                self.state = if *is_error {
                    AgentRunState::Failed
                } else {
                    AgentRunState::Completed
                };
            }
        }
        self.state
    }

    /// 当前状态观测。
    pub fn current(&self) -> AgentRunState {
        self.state
    }
}

impl Default for RunStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
