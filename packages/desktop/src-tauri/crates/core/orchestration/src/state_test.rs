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
        started_at: 1_726_000_000_000,
        steps: Vec::new(),
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
        started_at: 1_726_000_000_000,
        steps: Vec::new(),
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

// ---------------------------------------------------------------------------
// test-execution 扩展节（desktop-checks-domain）
// ---------------------------------------------------------------------------

/// 正向：TestExecution 出线 `testExecution` camelCase 逐字断言（九值词汇
/// 测试扩为十值——serde rename_all camelCase 线格式）。
#[test]
fn change_step_kind十值词汇_testexecution出线逐字() {
    let wire = serde_json::to_value(ChangeStepKind::TestExecution).expect("词汇出线应成功");
    assert_eq!(
        wire,
        json!("testExecution"),
        "testExecution camelCase 出线逐字（bindings 再生成直出线格式）"
    );

    // serde 往返无损
    let text = serde_json::to_string(&ChangeStepKind::TestExecution).expect("出线应成功");
    let back: ChangeStepKind = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, ChangeStepKind::TestExecution, "新变体双向派生可用");
}

/// 边界：十值词汇互异（camelCase 判别值集合无碰撞——三类节点可辨不变量
/// 保持：WorkerAgent 三角色 + ToolStep 三步一门禁 + Gate 三门）。
#[test]
fn change_step_kind十值词汇互异无碰撞() {
    let all = [
        ChangeStepKind::Executor,
        ChangeStepKind::Evaluator,
        ChangeStepKind::Decision,
        ChangeStepKind::PhaseStart,
        ChangeStepKind::StaticCheck,
        ChangeStepKind::TestExecution,
        ChangeStepKind::PhaseLog,
        ChangeStepKind::VerdictGate,
        ChangeStepKind::RetryGate,
        ChangeStepKind::WhitelistGate,
    ];
    let wires: Vec<String> = all
        .iter()
        .map(|kind| {
            serde_json::to_value(kind)
                .expect("出线应成功")
                .as_str()
                .expect("串值")
                .to_owned()
        })
        .collect();
    let mut sorted = wires.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        10,
        "十值词汇互异（无 camelCase 碰撞），实际: {wires:?}"
    );
}

/// 正向：步状态行携 TestExecution 步 + detail 摘要经 RunUpdate::Step 出线
///（步可观测 IPC 面——前端流程视图步骤渲染直接吃；session_id 工具步恒缺省）。
#[test]
fn 步状态行携testexecution步经run_update出线() {
    let running = ChangeStepState {
        phase: "test-execution".to_owned(),
        attempt: 1,
        step: ChangeStepKind::TestExecution,
        status: ChangeStepStatus::Running,
        session_id: None,
        detail: None,
    };
    let value = serde_json::to_value(&RunUpdate::Step {
        step: running.clone(),
    })
    .expect("出线应成功");
    assert_eq!(value["ipc"], json!("step"));
    assert_eq!(
        value["step"]["step"],
        json!("testExecution"),
        "步词汇 camelCase"
    );
    assert_eq!(value["step"]["phase"], json!("test-execution"));
    assert_eq!(value["step"]["status"], json!("running"));
    assert_eq!(
        value["step"]["sessionId"],
        json!(null),
        "工具步会话槽恒缺省（绿跑零 agent）"
    );

    // 通过态携 detail 摘要（conclusion + 四计数摘要——walker 步 detail 面）
    let passed = ChangeStepState {
        status: ChangeStepStatus::Passed,
        detail: Some("conclusion=pass total=5 passed=4 failed=0 skipped=1".to_owned()),
        ..running
    };
    let value = serde_json::to_value(&RunUpdate::Step {
        step: passed.clone(),
    })
    .expect("出线应成功");
    assert_eq!(value["step"]["status"], json!("passed"));
    assert_eq!(
        value["step"]["detail"],
        json!("conclusion=pass total=5 passed=4 failed=0 skipped=1"),
        "detail 摘要出线（步状态上图输入）"
    );

    // serde 往返无损
    let text = serde_json::to_string(&passed).expect("出线应成功");
    let back: ChangeStepState = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, passed, "步状态行（TestExecution 步）往返无损");
}

// ---------------------------------------------------------------------------
// RunNotice 线面（unify-run-state-persistence D3 通知降位）：kind-only 零载荷
// ---------------------------------------------------------------------------

/// 五变体序列化出线恰为 `{"ipc":"…"}` 单键零载荷（通知仅失效信号——载荷剥离
/// 后的 IPC 信封形态，AC-8）：step / sessionEvent / ask / confirmWait /
/// finished 五 kind 逐字 camelCase。
#[test]
fn run_notice五变体kind_only零载荷单键出线() {
    let wire =
        |notice: &crate::state::RunNotice| serde_json::to_value(notice).expect("通知出线应成功");
    assert_eq!(
        wire(&crate::state::RunNotice::Step),
        json!({"ipc": "step"}),
        "step kind-only 单键"
    );
    assert_eq!(
        wire(&crate::state::RunNotice::SessionEvent),
        json!({"ipc": "sessionEvent"}),
        "sessionEvent camelCase 判别值逐字"
    );
    assert_eq!(wire(&crate::state::RunNotice::Ask), json!({"ipc": "ask"}));
    assert_eq!(
        wire(&crate::state::RunNotice::ConfirmWait),
        json!({"ipc": "confirmWait"})
    );
    assert_eq!(
        wire(&crate::state::RunNotice::Finished),
        json!({"ipc": "finished"})
    );
}

/// `From<&RunUpdate>` kind 投影封闭集：五变体一一同型对应（载荷剥离单点——
/// `ChangeFlowControl::publish` 广播侧消费的映射面）。注记：`RunNotice` 为
/// Serialize-only IPC 出线信封（零 Deserialize），封闭集防线以线格式逐字
/// 钉死与投影映射穷尽承载，无入站反序列化面。
#[test]
fn run_notice自run_update投影五变体一一同型() {
    let projection = |update: &RunUpdate| crate::state::RunNotice::from(update);
    assert!(matches!(
        projection(&RunUpdate::Step {
            step: sample_step()
        }),
        crate::state::RunNotice::Step
    ));
    assert!(matches!(
        projection(&RunUpdate::SessionEvent {
            session_id: "s".to_owned(),
            event: sealed_event(0)
        }),
        crate::state::RunNotice::SessionEvent
    ));
    assert!(matches!(
        projection(&RunUpdate::Ask {
            question: "q".to_owned(),
            options: Vec::new()
        }),
        crate::state::RunNotice::Ask
    ));
    assert!(matches!(
        projection(&RunUpdate::ConfirmWait {
            phase: "test-gen".to_owned()
        }),
        crate::state::RunNotice::ConfirmWait
    ));
    assert!(matches!(
        projection(&RunUpdate::Finished {
            status: ChangeRunStatus::Completed,
            reason: None
        }),
        crate::state::RunNotice::Finished
    ));

    // 投影与 kind() 判别词一一同型（两词汇源同封闭集）
    for update in [
        RunUpdate::Step {
            step: sample_step(),
        },
        RunUpdate::SessionEvent {
            session_id: "s".to_owned(),
            event: sealed_event(1),
        },
        RunUpdate::Ask {
            question: "q".to_owned(),
            options: Vec::new(),
        },
        RunUpdate::ConfirmWait {
            phase: "p".to_owned(),
        },
        RunUpdate::Finished {
            status: ChangeRunStatus::Stopped,
            reason: None,
        },
    ] {
        let notice = crate::state::RunNotice::from(&update);
        assert_eq!(
            serde_json::to_value(&notice).expect("出线应成功")["ipc"],
            json!(update.kind()),
            "Notice 判别词与 RunUpdate::kind() 逐字同源"
        );
    }
}

/// 快照扩面出线：steps 累积器逐条 ChangeStepState camelCase 全词汇形态 +
/// startedAt 毫秒（AC-10 重挂步表面——统一视图活面投影源的线面）。
#[test]
fn 快照扩面_startedat与steps全词汇camelcase出线() {
    let steps = vec![
        ChangeStepState {
            phase: "implement".to_owned(),
            attempt: 1,
            step: ChangeStepKind::Executor,
            status: ChangeStepStatus::Passed,
            session_id: Some("ses-1".to_owned()),
            detail: Some("实现完成".to_owned()),
        },
        ChangeStepState {
            phase: "implement".to_owned(),
            attempt: 1,
            step: ChangeStepKind::StaticCheck,
            status: ChangeStepStatus::Failed,
            session_id: None,
            detail: None,
        },
        ChangeStepState {
            phase: "test-execution".to_owned(),
            attempt: 1,
            step: ChangeStepKind::TestExecution,
            status: ChangeStepStatus::Running,
            session_id: None,
            detail: None,
        },
        ChangeStepState {
            phase: "dev-design".to_owned(),
            attempt: 2,
            step: ChangeStepKind::PhaseStart,
            status: ChangeStepStatus::Passed,
            session_id: None,
            detail: None,
        },
        ChangeStepState {
            phase: "dev-design".to_owned(),
            attempt: 2,
            step: ChangeStepKind::VerdictGate,
            status: ChangeStepStatus::Stopped,
            session_id: None,
            detail: None,
        },
    ];
    let snapshot = ChangeRunSnapshot {
        run_id: "run-9".to_owned(),
        status: ChangeRunStatus::Running,
        phase: Some("test-execution".to_owned()),
        attempt: Some(1),
        ask: None,
        started_at: 1_726_000_000_000,
        steps,
    };
    let value = serde_json::to_value(&snapshot).expect("出线应成功");
    assert_eq!(
        value["startedAt"],
        json!(1_726_000_000_000_i64),
        "startedAt 毫秒"
    );
    assert_eq!(value["steps"].as_array().map(Vec::len), Some(5));
    // 步词汇 camelCase 线面逐条（快照透传 ChangeStepState 原生线格式）
    assert_eq!(value["steps"][0]["step"], json!("executor"));
    assert_eq!(value["steps"][1]["step"], json!("staticCheck"));
    assert_eq!(value["steps"][2]["step"], json!("testExecution"));
    assert_eq!(value["steps"][3]["step"], json!("phaseStart"));
    assert_eq!(value["steps"][4]["step"], json!("verdictGate"));
    assert_eq!(value["steps"][1]["status"], json!("failed"));
    assert_eq!(value["steps"][4]["status"], json!("stopped"));

    // serde 往返无损（扩面字段随行）
    let text = serde_json::to_string(&snapshot).expect("出线应成功");
    let back: ChangeRunSnapshot = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, snapshot, "快照（含 steps 累积器）往返无损");
}
