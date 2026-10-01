use agent::{AgentEvent, AgentEventKind, AgentRunStatus, RunningTurn, TurnOutcome, TurnSummary};

use super::{outcome_summary, running_summary, AgentRunMessage};

// ---------------------------------------------------------------------------
// 装置：固定值底座
// ---------------------------------------------------------------------------

fn stamped_event(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: None,
            delta: agent::AgentDelta::Text {
                text: "增量".to_owned(),
            },
        },
    )
}

fn sealed_event(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::TurnDone {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(2),
            duration_ms: Some(800),
            cost_usd: None,
            usage: serde_json::json!({ "inputTokens": 10 }),
            session_id: Some("sdk-9-1727000000009".to_owned()),
        },
    )
}

fn outcome() -> TurnOutcome {
    TurnOutcome {
        turn_id: 3,
        status: AgentRunStatus::Completed,
        finished_at: 1727000005000,
        num_turns: Some(2),
        duration_ms: Some(5000),
        cost_usd: Some(0.12),
        usage: serde_json::json!({ "inputTokens": 10, "outputTokens": 20 }),
        error: None,
        remote_session_id: Some("sdk-9-1727000000009".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// AgentRunMessage 信封双变体（IPC 镜像回归）
// ---------------------------------------------------------------------------

#[test]
fn 信封event变体序列化tag为ipc判别event且载荷camelCase() {
    let envelope = AgentRunMessage::Event {
        event: sealed_event(7),
    };

    let value = serde_json::to_value(&envelope).expect("序列化成功");
    assert_eq!(
        value["ipc"],
        serde_json::json!("event"),
        "tag=ipc 判别值逐字 event（TS 镜像放 transport）"
    );
    assert!(value.get("event").is_some(), "载荷键在位");
    // 事件线格式镜像：camelCase 顶层键 + kind 扁平判别
    assert_eq!(value["event"]["seq"], serde_json::json!(7));
    assert_eq!(value["event"]["kind"], serde_json::json!("turnDone"));
    assert_eq!(value["event"]["isError"], serde_json::json!(false));
}

#[test]
fn 信封record变体序列化tag为ipc判别record且轮行camelCase() {
    // running 形态轮行（提前 resolve 信封面）：RunningTurn 构造收内核（字段
    // 私有），此处以 running 形状的同构 TurnSummary 承载信封断言，running
    // 装配本体由 mod_test 提前 resolve 链路断言
    let summary = TurnSummary {
        turn_id: 1,
        session_id: "ses-1-1727000000000".to_owned(),
        status: AgentRunStatus::Running,
        started_at: 1727000000000,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        error: None,
    };
    let envelope = AgentRunMessage::Record {
        record: summary.clone(),
    };

    let value = serde_json::to_value(&envelope).expect("序列化成功");
    assert_eq!(
        value["ipc"],
        serde_json::json!("record"),
        "tag=ipc 判别值逐字 record（终态轮行独立信封臂，不塞事件 usage）"
    );
    assert_eq!(
        value["record"]["turnId"],
        serde_json::json!(summary.turn_id)
    );
    assert_eq!(
        value["record"]["sessionId"],
        serde_json::json!(summary.session_id)
    );
    assert_eq!(value["record"]["status"], serde_json::json!("running"));

    // 两变体信封互异（Event 不含轮行、Record 不含事件）
    let event_envelope = AgentRunMessage::Event {
        event: stamped_event(0),
    };
    let event_value = serde_json::to_value(&event_envelope).expect("序列化成功");
    assert_ne!(event_value["ipc"], value["ipc"]);
    assert!(event_value.get("record").is_none());
    assert!(value.get("event").is_none());
}

// ---------------------------------------------------------------------------
// TurnSummary running 装配签名锚定（提前 resolve 返回值面）
// ---------------------------------------------------------------------------

#[test]
fn running_summary签名锚定_自中性数据装配running形态轮行() {
    // RunningTurn 无公共构造（同步段产物）：装配签名以函数指针类型锚定，
    // running 形态断言由 mod_test 提前 resolve 链路经真实 begin_turn 产物承载
    let _signature: fn(&RunningTurn) -> TurnSummary = running_summary;
}

// ---------------------------------------------------------------------------
// TurnSummary 终态装配（Channel Record 信封；与重放同构前提——AC-8）
// ---------------------------------------------------------------------------

#[test]
fn outcome_summary装配逐字段承接_status与统计与finished_at() {
    let summary = outcome_summary(&outcome(), "ses-outcome-1", 1727000000000);

    assert_eq!(summary.turn_id, 3);
    assert_eq!(summary.session_id, "ses-outcome-1", "会话 id 自轮发起回填");
    assert_eq!(summary.started_at, 1727000000000, "started_at 自轮发起回填");
    assert_eq!(summary.status, AgentRunStatus::Completed);
    assert_eq!(
        summary.finished_at,
        Some(1727000005000),
        "终态落 finished_at"
    );
    assert_eq!(summary.num_turns, Some(2));
    assert_eq!(summary.duration_ms, Some(5000));
    assert_eq!(summary.cost_usd, Some(0.12));
    assert_eq!(summary.error, None);

    // failed 记因形态：error 承接（落库失败收敛的 IPC 面可感知）
    let failed = TurnOutcome {
        status: AgentRunStatus::Failed,
        error: Some("事件落库失败: db: x".to_owned()),
        ..outcome()
    };
    let summary = outcome_summary(&failed, "ses-outcome-1", 1);
    assert_eq!(summary.status, AgentRunStatus::Failed);
    assert_eq!(summary.error.as_deref(), Some("事件落库失败: db: x"));
}

#[test]
fn outcome_summary与turn_summary同构_终态部件与重放同形状() {
    // Channel 终态部件（outcome_summary）与重放轮行（TurnSummary serde 形态）
    // 同构：同字段面、同 serde 线格式
    let assembled = outcome_summary(&outcome(), "ses-iso", 100);
    let value = serde_json::to_value(&assembled).expect("序列化成功");

    let mirrored: TurnSummary =
        serde_json::from_value(value.clone()).expect("TurnSummary 可反序列化（两路同构）");
    assert_eq!(mirrored, assembled);

    let keys: Vec<&str> = value
        .as_object()
        .expect("对象形态")
        .keys()
        .map(String::as_str)
        .collect();
    for key in [
        "turnId",
        "sessionId",
        "status",
        "startedAt",
        "finishedAt",
        "numTurns",
        "costUsd",
        "durationMs",
        "error",
    ] {
        assert!(keys.contains(&key), "IPC 轮行字段 {key} 在场");
    }
}
