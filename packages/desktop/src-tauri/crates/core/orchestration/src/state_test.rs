//! `state.rs`（run 状态与步状态类型）的单元测试（test-design「state.rs ->
//! state_test.rs」节）：RunUpdate 五变体 `ipc` 信封线格式、SessionEvent /
//! Finished 载荷、ChangeRunStatus 终态矩阵、ChangeStepKind 九值词汇、
//! ChangeStepState 步状态行、ChangeRunSnapshot / ChangeRunSummary 往返、
//! specta Type 可达。无进程边界依赖：serde_json / specta 内存构造与出线
//! 断言，AgentEvent 载荷以 `AgentEvent::stamp` 固定构造（Mock策略：无 mock）。
//!
//! 线面注记：`RunUpdate` 仅 Serialize（IPC 出线信封），线格式以出线 JSON 逐
//! 字段断言承载；`ChangeRunStatus` / `ChangeStepKind` / `ChangeStepState` /
//! `ChangeRunSnapshot` / `ChangeRunSummary` 双向派生，serde 往返逐字段相等。

use agent::{AgentBlock, AgentEventKind, AgentMessageRole};
use serde_json::json;

use crate::state::{
    AskPayload, ChangeRunSnapshot, ChangeRunStatus, ChangeRunSummary, ChangeStepKind,
    ChangeStepState, ChangeStepStatus, RunUpdate,
};

/// 密封 AgentEvent fixture（固定盖戳时刻由 stamp 取当前时钟——载荷面断言以
/// kind / seq 为准，时间戳不参与等值断言）。
fn sealed_event(seq: u64) -> agent::AgentEvent {
    agent::AgentEvent::stamp(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: vec![AgentBlock::Text {
                text: "评估结论".to_owned(),
            }],
            parent_tool_use_id: None,
        },
    )
}

/// 步状态行 fixture。
fn sample_step() -> ChangeStepState {
    ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 2,
        step: ChangeStepKind::StaticCheck,
        status: ChangeStepStatus::Running,
        session_id: None,
        detail: None,
    }
}

// ---------------------------------------------------------------------------
// RunUpdate 五变体线格式（tag `ipc`）
// ---------------------------------------------------------------------------

#[test]
fn run_update五变体线格式tag_ipc判别且驼峰键() {
    // step 变体
    let step = serde_json::to_value(&RunUpdate::Step {
        step: sample_step(),
    })
    .expect("step 出线应成功");
    assert_eq!(step["ipc"], json!("step"), "tag=ipc 判别值逐字 step");
    assert_eq!(step["step"]["phase"], json!("implement"));
    assert_eq!(step["step"]["attempt"], json!(2));
    assert_eq!(
        step["step"]["step"],
        json!("staticCheck"),
        "步词汇 camelCase 线格式"
    );
    assert_eq!(step["step"]["status"], json!("running"));
    assert_eq!(
        step["step"]["sessionId"],
        json!(null),
        "缺省字段 null 不省键"
    );
    assert_eq!(step["step"]["detail"], json!(null));

    // ask 变体
    let ask = serde_json::to_value(&RunUpdate::Ask {
        question: "回溯到哪?".to_owned(),
        options: vec!["proposal".to_owned(), "retry".to_owned()],
    })
    .expect("ask 出线应成功");
    assert_eq!(ask["ipc"], json!("ask"));
    assert_eq!(ask["question"], json!("回溯到哪?"));
    assert_eq!(ask["options"], json!(["proposal", "retry"]));

    // confirmWait 变体
    let wait = serde_json::to_value(&RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
    })
    .expect("confirmWait 出线应成功");
    assert_eq!(
        wait["ipc"],
        json!("confirmWait"),
        "camelCase 判别值逐字 confirmWait"
    );
    assert_eq!(wait["phase"], json!("test-gen"));

    // 五变体判别值互异（封闭集可辨）
    let tags = [
        step["ipc"].clone(),
        serde_json::to_value(&RunUpdate::SessionEvent {
            session_id: "s".to_owned(),
            event: sealed_event(0),
        })
        .expect("sessionEvent 出线")["ipc"]
            .clone(),
        ask["ipc"].clone(),
        wait["ipc"].clone(),
        serde_json::to_value(&RunUpdate::Finished {
            status: ChangeRunStatus::Completed,
            reason: None,
        })
        .expect("finished 出线")["ipc"]
            .clone(),
    ];
    assert_eq!(
        tags.to_vec(),
        vec![
            json!("step"),
            json!("sessionEvent"),
            json!("ask"),
            json!("confirmWait"),
            json!("finished")
        ],
        "五变体 camelCase 判别值逐字"
    );
}

#[test]
fn session_event变体携session_id与密封事件载荷保真() {
    let event = sealed_event(7);
    let value = serde_json::to_value(&RunUpdate::SessionEvent {
        session_id: "sess-flow-1".to_owned(),
        event: event.clone(),
    })
    .expect("出线应成功");

    assert_eq!(value["ipc"], json!("sessionEvent"));
    assert_eq!(value["sessionId"], json!("sess-flow-1"), "camelCase 键");
    // 密封 AgentEvent 载荷保真（kind 扁平判别 + camelCase）
    assert_eq!(value["event"]["seq"], json!(7));
    assert_eq!(value["event"]["kind"], json!("message"));
    assert_eq!(value["event"]["role"], json!("assistant"));
    assert_eq!(
        value["event"]["blocks"][0]["kind"],
        json!("text"),
        "块模型 tag=kind camelCase"
    );
    assert_eq!(value["event"]["blocks"][0]["text"], json!("评估结论"));
}

#[test]
fn finished变体reason双形态出线() {
    // reason=None 形态（completed 收口）
    let done = serde_json::to_value(&RunUpdate::Finished {
        status: ChangeRunStatus::Completed,
        reason: None,
    })
    .expect("出线应成功");
    assert_eq!(done["ipc"], json!("finished"));
    assert_eq!(done["status"], json!("completed"));
    assert_eq!(done["reason"], json!(null), "None 出线 null 不省键");

    // reason=Some 形态（stopped / failed 记因收口）
    let stopped = serde_json::to_value(&RunUpdate::Finished {
        status: ChangeRunStatus::Stopped,
        reason: Some("用户请求停止".to_owned()),
    })
    .expect("出线应成功");
    assert_eq!(stopped["status"], json!("stopped"));
    assert_eq!(stopped["reason"], json!("用户请求停止"));
}

// ---------------------------------------------------------------------------
// ChangeRunStatus 终态矩阵
// ---------------------------------------------------------------------------

#[test]
fn change_run_status六值线格式逐字断言() {
    let wire = |status: ChangeRunStatus| serde_json::to_value(status).expect("状态出线应成功");
    assert_eq!(wire(ChangeRunStatus::Running), json!("running"));
    assert_eq!(
        wire(ChangeRunStatus::WaitingConfirm),
        json!("waitingConfirm")
    );
    assert_eq!(wire(ChangeRunStatus::WaitingAsk), json!("waitingAsk"));
    assert_eq!(wire(ChangeRunStatus::Completed), json!("completed"));
    assert_eq!(wire(ChangeRunStatus::Stopped), json!("stopped"));
    assert_eq!(wire(ChangeRunStatus::Failed), json!("failed"));
}

#[test]
fn 终态判别矩阵is_terminal互补() {
    // 终局三态
    assert!(ChangeRunStatus::Completed.is_terminal());
    assert!(ChangeRunStatus::Stopped.is_terminal());
    assert!(ChangeRunStatus::Failed.is_terminal());
    // 运行期三态（互补矩阵）
    assert!(!ChangeRunStatus::Running.is_terminal());
    assert!(!ChangeRunStatus::WaitingConfirm.is_terminal());
    assert!(!ChangeRunStatus::WaitingAsk.is_terminal());
}

// ---------------------------------------------------------------------------
// ChangeStepKind 九值词汇（三类可辨）
// ---------------------------------------------------------------------------

#[test]
fn change_step_kind九值词汇线格式逐字且三类可辨() {
    let wire = |kind: ChangeStepKind| serde_json::to_value(kind).expect("词汇出线应成功");

    // WorkerAgent 三角色
    assert_eq!(wire(ChangeStepKind::Executor), json!("executor"));
    assert_eq!(wire(ChangeStepKind::Evaluator), json!("evaluator"));
    assert_eq!(wire(ChangeStepKind::Decision), json!("decision"));
    // ToolStep 三步
    assert_eq!(wire(ChangeStepKind::PhaseStart), json!("phaseStart"));
    assert_eq!(wire(ChangeStepKind::StaticCheck), json!("staticCheck"));
    assert_eq!(wire(ChangeStepKind::PhaseLog), json!("phaseLog"));
    // Gate 三门
    assert_eq!(wire(ChangeStepKind::VerdictGate), json!("verdictGate"));
    assert_eq!(wire(ChangeStepKind::RetryGate), json!("retryGate"));
    assert_eq!(wire(ChangeStepKind::WhitelistGate), json!("whitelistGate"));

    // 三类可辨（九值互异）
    let all = [
        ChangeStepKind::Executor,
        ChangeStepKind::Evaluator,
        ChangeStepKind::Decision,
        ChangeStepKind::PhaseStart,
        ChangeStepKind::StaticCheck,
        ChangeStepKind::PhaseLog,
        ChangeStepKind::VerdictGate,
        ChangeStepKind::RetryGate,
        ChangeStepKind::WhitelistGate,
    ];
    let mut wires: Vec<String> = all
        .iter()
        .map(|kind| wire(*kind).as_str().expect("串值").to_owned())
        .collect();
    wires.sort();
    wires.dedup();
    assert_eq!(wires.len(), 9, "九值词汇互异（三类节点可辨）");
}

#[test]
fn change_step_state四档状态与缺省字段往返无损() {
    let wire = |status: ChangeStepStatus| serde_json::to_value(status).expect("出线应成功");
    assert_eq!(wire(ChangeStepStatus::Running), json!("running"));
    assert_eq!(wire(ChangeStepStatus::Passed), json!("passed"));
    assert_eq!(wire(ChangeStepStatus::Failed), json!("failed"));
    assert_eq!(wire(ChangeStepStatus::Stopped), json!("stopped"));

    // 缺省形态（sessionId / detail None）出线 null 不省键、字段形状固定
    let value = serde_json::to_value(sample_step()).expect("出线应成功");
    assert_eq!(
        value,
        json!({
            "phase": "implement",
            "attempt": 2,
            "step": "staticCheck",
            "status": "running",
            "sessionId": null,
            "detail": null
        }),
        "步状态行六字段 camelCase 线面"
    );

    // 有值形态 serde 往返无损
    let full = ChangeStepState {
        phase: "dev-design".to_owned(),
        attempt: 3,
        step: ChangeStepKind::Executor,
        status: ChangeStepStatus::Failed,
        session_id: Some("sess-9".to_owned()),
        detail: Some("会话失败收敛".to_owned()),
    };
    let text = serde_json::to_string(&full).expect("出线应成功");
    let back: ChangeStepState = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, full, "步状态行往返无损");
}

// ---------------------------------------------------------------------------
// ChangeRunSnapshot / ChangeRunSummary 与 specta Type 可达
// ---------------------------------------------------------------------------

#[test]
fn 快照与摘要全字段serde往返() {
    // 重挂快照：waitingAsk 形态（ask 载荷在场）
    let snapshot = ChangeRunSnapshot {
        run_id: "run-42".to_owned(),
        status: ChangeRunStatus::WaitingAsk,
        phase: Some("implement".to_owned()),
        attempt: Some(2),
        ask: Some(AskPayload {
            question: "是否回溯?".to_owned(),
            options: vec!["dev-design".to_owned(), "retry".to_owned()],
        }),
    };
    let text = serde_json::to_string(&snapshot).expect("出线应成功");
    assert!(
        text.contains("\"runId\":\"run-42\""),
        "camelCase 键: {text}"
    );
    assert!(text.contains("\"waitingAsk\""));
    let back: ChangeRunSnapshot = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, snapshot, "快照往返无损");

    // running 形态（ask 缺省 None）
    let running = ChangeRunSnapshot {
        run_id: "run-1".to_owned(),
        status: ChangeRunStatus::Running,
        phase: None,
        attempt: None,
        ask: None,
    };
    let text = serde_json::to_string(&running).expect("出线应成功");
    assert!(
        text.contains("\"phase\":null") && text.contains("\"ask\":null"),
        "缺省 null 不省键: {text}"
    );

    // 发起摘要：run_id 立即可知 + running 态
    let summary = ChangeRunSummary {
        run_id: "run-7".to_owned(),
        status: ChangeRunStatus::Running,
    };
    let value = serde_json::to_value(&summary).expect("出线应成功");
    assert_eq!(value, json!({"runId": "run-7", "status": "running"}));
    let back: ChangeRunSummary =
        serde_json::from_str(&serde_json::to_string(&summary).expect("x")).expect("往返应成功");
    assert_eq!(back, summary);
}

/// specta Type 可达：上述类型经 specta 导出面编译期锚定（IPC 绑定生成前提，
/// bindings 零 diff 链路的类型半边）。
#[test]
fn specta_type可达编译期锚定() {
    fn assert_type<T: specta::Type>() {}
    assert_type::<ChangeRunStatus>();
    assert_type::<ChangeStepKind>();
    assert_type::<ChangeStepStatus>();
    assert_type::<ChangeStepState>();
    assert_type::<AskPayload>();
    assert_type::<ChangeRunSnapshot>();
    assert_type::<ChangeRunSummary>();
    assert_type::<RunUpdate>();
}
