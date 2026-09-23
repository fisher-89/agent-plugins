//! `state` 的单元测试（AC-1）：RunResult 驱动收敛、is_error 分向、收敛后
//! 拒绝变更与 apply/current 一致性。无外部依赖，不需要 Mock。

use crate::event::{AgentEvent, AgentEventKind};
use crate::state::{AgentRunState, RunStateMachine};

/// 构造一枚事件（不盖真实时钟也行——状态机只读 kind，stamp 即可）。
fn event(seq: u64, kind: AgentEventKind) -> AgentEvent {
    AgentEvent::stamp(seq, kind)
}

fn run_started(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::RunStarted {
            model: None,
            session_id: None,
            tools: Vec::new(),
            mcp_servers: Vec::new(),
        },
    )
}

fn message(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    )
}

fn system_notice(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::SystemNotice {
            subtype: "api_retry".to_owned(),
            payload: serde_json::json!({ "attempt": 1 }),
        },
    )
}

fn raw(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Raw {
            event_type: "unknown".to_owned(),
            raw_json: "{}".to_owned(),
        },
    )
}

fn run_result(seq: u64, is_error: bool) -> AgentEvent {
    event(
        seq,
        AgentEventKind::RunResult {
            subtype: if is_error {
                "error_max_turns"
            } else {
                "success"
            }
            .to_owned(),
            is_error,
            num_turns: Some(1),
            duration_ms: None,
            cost_usd: None,
            usage: serde_json::Value::Null,
            session_id: None,
        },
    )
}

#[test]
fn new后current为running() {
    let machine = RunStateMachine::new();
    assert_eq!(machine.current(), AgentRunState::Running);
}

#[test]
fn run_result且is_error为false时收敛completed() {
    let mut machine = RunStateMachine::new();
    let state = machine.apply(&run_result(0, false));
    assert_eq!(state, AgentRunState::Completed);
    assert_eq!(machine.current(), AgentRunState::Completed);
}

#[test]
fn run_result且is_error为true时收敛failed() {
    let mut machine = RunStateMachine::new();
    let state = machine.apply(&run_result(0, true));
    assert_eq!(state, AgentRunState::Failed, "AC-1：is_error 收敛 failed");
    assert_eq!(machine.current(), AgentRunState::Failed);
}

#[test]
fn 非run_result事件不改变状态保持running() {
    let mut machine = RunStateMachine::new();
    for event in [run_started(0), message(1), system_notice(2), raw(3)] {
        let state = machine.apply(&event);
        assert_eq!(state, AgentRunState::Running, "seq={} 不收敛", event.seq);
        assert_eq!(machine.current(), AgentRunState::Running);
    }
}

#[test]
fn 收敛后再apply任意事件终态不被改写() {
    let mut machine = RunStateMachine::new();
    machine.apply(&run_result(0, false));
    assert_eq!(machine.current(), AgentRunState::Completed);

    // 收敛后一切事件（含 is_error=true 的 RunResult）都不能改写终态
    for event in [message(1), run_result(2, true), raw(3), system_notice(4)] {
        let state = machine.apply(&event);
        assert_eq!(state, AgentRunState::Completed, "拒绝变更（幂等终态）");
    }

    // failed 终态同理
    let mut failed_machine = RunStateMachine::new();
    failed_machine.apply(&run_result(0, true));
    let state = failed_machine.apply(&run_result(1, false));
    assert_eq!(
        state,
        AgentRunState::Failed,
        "failed 不被后续 completed 改写"
    );
}

#[test]
fn 连续两个run_result时首个收敛生效() {
    let mut machine = RunStateMachine::new();
    let first = machine.apply(&run_result(0, false));
    let second = machine.apply(&run_result(1, true));

    assert_eq!(first, AgentRunState::Completed, "首个收敛生效");
    assert_eq!(second, AgentRunState::Completed, "第二个 RunResult 被拒绝");
}

#[test]
fn 每次apply返回值与随后current观测一致() {
    let mut machine = RunStateMachine::new();

    let applied = machine.apply(&run_started(0));
    assert_eq!(applied, machine.current());

    let applied = machine.apply(&message(1));
    assert_eq!(applied, machine.current());

    let applied = machine.apply(&run_result(2, true));
    assert_eq!(applied, machine.current());

    let applied = machine.apply(&message(3));
    assert_eq!(applied, machine.current());
}
