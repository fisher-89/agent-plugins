use crate::event::{AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole};
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
            role: AgentMessageRole::Assistant,
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    )
}

fn message_delta(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: None,
            delta: AgentDelta::Text {
                text: "增量".to_owned(),
            },
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

fn turn_done(seq: u64, is_error: bool) -> AgentEvent {
    event(
        seq,
        AgentEventKind::TurnDone {
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

// ---------------------------------------------------------------------------
// TurnDone 驱动收敛（apply 匹配变体 RunResult → TurnDone 纯更名适配）
// ---------------------------------------------------------------------------

#[test]
fn new后current为running() {
    let machine = RunStateMachine::new();
    assert_eq!(machine.current(), AgentRunState::Running);
}

#[test]
fn turn_done且is_error为false时收敛completed() {
    let mut machine = RunStateMachine::new();
    let state = machine.apply(&turn_done(0, false));
    assert_eq!(state, AgentRunState::Completed);
    assert_eq!(machine.current(), AgentRunState::Completed);
}

#[test]
fn turn_done且is_error为true时收敛failed() {
    let mut machine = RunStateMachine::new();
    let state = machine.apply(&turn_done(0, true));
    assert_eq!(state, AgentRunState::Failed, "is_error 收敛 failed");
    assert_eq!(machine.current(), AgentRunState::Failed);
}

// ---------------------------------------------------------------------------
// 非收敛事件不改变状态（新增 delta 词汇不驱动收敛）
// ---------------------------------------------------------------------------

#[test]
fn 非turn_done事件不改变状态保持running() {
    let mut machine = RunStateMachine::new();
    for event in [
        run_started(0),
        message_delta(1),
        message(2),
        system_notice(3),
        raw(4),
    ] {
        let state = machine.apply(&event);
        assert_eq!(state, AgentRunState::Running, "seq={} 不收敛", event.seq);
        assert_eq!(machine.current(), AgentRunState::Running);
    }
}

// ---------------------------------------------------------------------------
// stopped 独立分支（running 调 stop 收敛；stopped 后 TurnDone 不改写终态）
// ---------------------------------------------------------------------------

#[test]
fn running状态调用stop收敛stopped且current观测一致() {
    let mut machine = RunStateMachine::new();
    let state = machine.stop();
    assert_eq!(state, AgentRunState::Stopped, "显式终止收敛 Stopped");
    assert_eq!(machine.current(), AgentRunState::Stopped);
}

#[test]
fn stopped收敛后再apply任意事件终态保持stopped() {
    let mut machine = RunStateMachine::new();
    machine.stop();
    assert_eq!(machine.current(), AgentRunState::Stopped);

    // Stopped 终态拒绝一切改写（含 TurnDone）
    for event in [message(1), turn_done(2, true), system_notice(3), raw(4)] {
        let state = machine.apply(&event);
        assert_eq!(
            state,
            AgentRunState::Stopped,
            "Stopped 不被改写（幂等终态）"
        );
        assert_eq!(machine.current(), AgentRunState::Stopped);
    }
}

// ---------------------------------------------------------------------------
// 终态幂等与首收敛生效
// ---------------------------------------------------------------------------

#[test]
fn 收敛后再apply任意事件终态不被改写() {
    let mut machine = RunStateMachine::new();
    machine.apply(&turn_done(0, false));
    assert_eq!(machine.current(), AgentRunState::Completed);

    // 收敛后一切事件（含 is_error=true 的 TurnDone）都不能改写终态
    for event in [message(1), turn_done(2, true), raw(3), system_notice(4)] {
        let state = machine.apply(&event);
        assert_eq!(state, AgentRunState::Completed, "拒绝变更（幂等终态）");
    }

    // failed 终态同理
    let mut failed_machine = RunStateMachine::new();
    failed_machine.apply(&turn_done(0, true));
    let state = failed_machine.apply(&turn_done(1, false));
    assert_eq!(
        state,
        AgentRunState::Failed,
        "failed 不被后续 completed 改写"
    );
}

#[test]
fn 连续两个turn_done时首个收敛生效() {
    let mut machine = RunStateMachine::new();
    let first = machine.apply(&turn_done(0, false));
    let second = machine.apply(&turn_done(1, true));

    assert_eq!(first, AgentRunState::Completed, "首个收敛生效");
    assert_eq!(second, AgentRunState::Completed, "第二个 TurnDone 被拒绝");
}

#[test]
fn 每次apply返回值与随后current观测一致() {
    let mut machine = RunStateMachine::new();

    let applied = machine.apply(&run_started(0));
    assert_eq!(applied, machine.current());

    let applied = machine.apply(&message(1));
    assert_eq!(applied, machine.current());

    let applied = machine.apply(&turn_done(2, true));
    assert_eq!(applied, machine.current());

    let applied = machine.apply(&message(3));
    assert_eq!(applied, machine.current());
}

#[test]
fn 已completed或failed终态调用stop原样返回不改写() {
    let mut completed = RunStateMachine::new();
    completed.apply(&turn_done(0, false));
    assert_eq!(
        completed.stop(),
        AgentRunState::Completed,
        "已 Completed 调 stop 原样返回 Completed"
    );
    assert_eq!(completed.current(), AgentRunState::Completed);

    let mut failed = RunStateMachine::new();
    failed.apply(&turn_done(0, true));
    assert_eq!(
        failed.stop(),
        AgentRunState::Failed,
        "已 Failed 调 stop 原样返回 Failed"
    );
    assert_eq!(failed.current(), AgentRunState::Failed);
}

#[test]
fn 连续两次stop首个收敛生效第二次原样返回stopped() {
    let mut machine = RunStateMachine::new();
    let first = machine.stop();
    let second = machine.stop();

    assert_eq!(first, AgentRunState::Stopped, "首个收敛生效");
    assert_eq!(second, AgentRunState::Stopped, "第二次原样返回 Stopped");
}

#[test]
fn stop与turn_done双向竞态时首个收敛生效且互不改写() {
    // stop 先到：Stopped 定终态，后续 TurnDone（含 is_error）不改写
    let mut stop_first = RunStateMachine::new();
    stop_first.stop();
    assert_eq!(
        stop_first.apply(&turn_done(0, false)),
        AgentRunState::Stopped,
        "stop 先到收敛 → TurnDone 不改写"
    );

    // TurnDone 先到：Completed 定终态，后续 stop 不改写
    let mut done_first = RunStateMachine::new();
    done_first.apply(&turn_done(0, false));
    assert_eq!(
        done_first.stop(),
        AgentRunState::Completed,
        "TurnDone 先到收敛 → stop 不改写"
    );

    // TurnDone（is_error=true）先到：Failed 定终态，后续 stop 不改写
    let mut failed_first = RunStateMachine::new();
    failed_first.apply(&turn_done(0, true));
    assert_eq!(
        failed_first.stop(),
        AgentRunState::Failed,
        "is_error 收敛先到 → stop 不改写"
    );
}
