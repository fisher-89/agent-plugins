//! `state.rs`（run 状态与步状态类型）的单元测试（test-design「state.rs ->
//! state_test.rs」节）：RunUpdate 五变体 `ipc` 信封线格式、SessionEvent /
//! Finished 载荷、ChangeRunStatus 终态矩阵、ChangeStepKind 二十值词汇（cleanup
//! 归 Git 组——camel 与 snake 同形词面）、ArchivePreflight 八字段停等载荷、
//! ConfirmWait 双态（archive 随行）、ChangeStepState 步状态行、
//! ChangeRunSnapshot / ChangeRunSummary 往返、specta Type 可达。无进程边界
//! 依赖：serde_json / specta 内存构造与出线断言，AgentEvent 载荷以
//! `AgentEvent::stamp` 固定构造（Mock策略：无 mock）。
//!
//! 线面注记：`RunUpdate` 仅 Serialize（IPC 出线信封），线格式以出线 JSON 逐
//! 字段断言承载；`ChangeRunStatus` / `ChangeStepKind` / `ChangeStepState` /
//! `ChangeRunSnapshot` / `ChangeRunSummary` 双向派生，serde 往返逐字段相等。

use agent::{AgentBlock, AgentEventKind, AgentMessageRole};
use serde_json::json;

use crate::state::{
    ArchivePreflight, AskPayload, ChangeRunSnapshot, ChangeRunStatus, ChangeRunSummary,
    ChangeStepKind, ChangeStepState, ChangeStepStatus, RunUpdate,
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
        instance_no: 0,
        status: ChangeStepStatus::Running,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at: 1_726_000_000_000,
        ended_at: None,
        token_input: None,
        token_output: None,
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

    // confirmWait 变体（普通相位停等：archive 载荷缺省 None）
    let wait = serde_json::to_value(&RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
        archive: None,
    })
    .expect("confirmWait 出线应成功");
    assert_eq!(
        wait["ipc"],
        json!("confirmWait"),
        "camelCase 判别值逐字 confirmWait"
    );
    assert_eq!(wait["phase"], json!("test-gen"));
    assert_eq!(
        wait["archive"],
        json!(null),
        "普通相位停等 archive 载荷 None（null 留位不省键）"
    );

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
// ChangeStepKind 二十值词汇（四类可辨，含归档 9 步词与拆步新词 mutationTest）
// ---------------------------------------------------------------------------

/// 归档 9 步词的 `camelCase` 线面（与 snake_case `as_str()` 逐字对应；
/// 词面单点与 workflow `RunStepKind::as_str()` 同源，AC-2）。
#[test]
fn change_step_kind归档九步词camelcase与snake同源() {
    let wire = |kind: ChangeStepKind| serde_json::to_value(kind).expect("词汇出线应成功");
    assert_eq!(wire(ChangeStepKind::Preflight), json!("preflight"));
    assert_eq!(wire(ChangeStepKind::Commit), json!("commit"));
    assert_eq!(wire(ChangeStepKind::Rebase), json!("rebase"));
    assert_eq!(
        wire(ChangeStepKind::ResolveConflicts),
        json!("resolveConflicts"),
        "多词步 camelCase 线格式"
    );
    assert_eq!(wire(ChangeStepKind::Merge), json!("merge"));
    assert_eq!(
        wire(ChangeStepKind::SpecSync),
        json!("specSync"),
        "多词步 camelCase 线格式"
    );
    // cleanup：snake / camel 同形逐字（identity 词——归 Git 步族）
    assert_eq!(
        wire(ChangeStepKind::Cleanup),
        json!("cleanup"),
        "cleanup camel 与 snake 同形逐字（identity 词）"
    );
    assert_eq!(wire(ChangeStepKind::Seal), json!("seal"));
    assert_eq!(wire(ChangeStepKind::Finalize), json!("finalize"));

    // camelCase 线词 ↔ snake_case 身份串词面逐字对应（落库侧同词面可比）
    for (kind, camel, snake) in [
        (ChangeStepKind::Preflight, "preflight", "preflight"),
        (ChangeStepKind::Commit, "commit", "commit"),
        (ChangeStepKind::Rebase, "rebase", "rebase"),
        (
            ChangeStepKind::ResolveConflicts,
            "resolveConflicts",
            "resolve_conflicts",
        ),
        (ChangeStepKind::Merge, "merge", "merge"),
        (ChangeStepKind::SpecSync, "specSync", "spec_sync"),
        (ChangeStepKind::Cleanup, "cleanup", "cleanup"),
        (ChangeStepKind::Seal, "seal", "seal"),
        (ChangeStepKind::Finalize, "finalize", "finalize"),
    ] {
        assert_eq!(wire(kind), json!(camel), "camelCase 线词逐字");
        assert_eq!(kind.as_str(), snake, "snake_case 身份串词逐字");
        // 双向往返（camelCase 入线 → 枚举 → 回出线）
        let back: ChangeStepKind =
            serde_json::from_value(json!(camel)).expect("camelCase 词往返应成功");
        assert_eq!(back, kind, "{camel} 往返逐词可辨");
    }
}

/// 非法词反序列化拒绝（异常）：二十值封闭集之外的词形（snake 词 / kebab 词 /
/// 近似词 / 空串 / 超长串 / 含换行与 emoji 的词）一律 `Err`（AC-2 线面契约守卫
/// ——camelCase 封闭集不吞词；cleanup 扩词不松动防线：clean-up / cleanUp 等
/// 近似词仍拒）。
#[test]
fn change_step_kind非法词反序列化拒绝() {
    let long_word = "mutation".repeat(200);
    for illegal in [
        "resolve_conflicts",
        "spec_sync",
        "spec-sync",
        "phase_start",
        "verdict_gate",
        "preflightstep",
        "git",
        "archive",
        "mutation_test",
        "mutationTestStep",
        "MutationTest",
        "clean-up",
        "cleanUp",
        "cleanupStep",
        "Cleanup",
        "",
        "
",
        "突变🚀test",
        long_word.as_str(),
    ] {
        let back: Result<ChangeStepKind, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "ChangeStepKind 越集词 {illegal:?} 应拒绝");
    }
}

#[test]
fn change_step_kind二十值词汇线格式逐字且四类可辨() {
    let wire = |kind: ChangeStepKind| serde_json::to_value(kind).expect("词汇出线应成功");

    // WorkerAgent 五角色（含归档两会话步）
    assert_eq!(wire(ChangeStepKind::Executor), json!("executor"));
    assert_eq!(wire(ChangeStepKind::Evaluator), json!("evaluator"));
    assert_eq!(wire(ChangeStepKind::Decision), json!("decision"));
    assert_eq!(wire(ChangeStepKind::SpecSync), json!("specSync"));
    assert_eq!(
        wire(ChangeStepKind::ResolveConflicts),
        json!("resolveConflicts")
    );
    // ToolStep 六步（相位机 / 检查（含拆步新词 mutationTest）/ 归档内联）
    assert_eq!(wire(ChangeStepKind::PhaseStart), json!("phaseStart"));
    assert_eq!(wire(ChangeStepKind::StaticCheck), json!("staticCheck"));
    assert_eq!(wire(ChangeStepKind::TestExecution), json!("testExecution"));
    assert_eq!(
        wire(ChangeStepKind::MutationTest),
        json!("mutationTest"),
        "突变门禁步 camelCase 线词（拆步新词——mutationTest ↔ mutation_test 归一在案）"
    );
    assert_eq!(ChangeStepKind::MutationTest.as_str(), "mutation_test");
    assert_eq!(wire(ChangeStepKind::PhaseLog), json!("phaseLog"));
    assert_eq!(wire(ChangeStepKind::Preflight), json!("preflight"));
    assert_eq!(wire(ChangeStepKind::Seal), json!("seal"));
    // Git 五步（cleanup 尽力步入组——cleanup camel 与 snake 同形）
    assert_eq!(wire(ChangeStepKind::Commit), json!("commit"));
    assert_eq!(wire(ChangeStepKind::Rebase), json!("rebase"));
    assert_eq!(wire(ChangeStepKind::Merge), json!("merge"));
    assert_eq!(
        wire(ChangeStepKind::Cleanup),
        json!("cleanup"),
        "cleanup 尽力步词面（归 Git 步族）"
    );
    assert_eq!(ChangeStepKind::Cleanup.as_str(), "cleanup");
    assert_eq!(wire(ChangeStepKind::Finalize), json!("finalize"));
    // Gate 三门
    assert_eq!(wire(ChangeStepKind::VerdictGate), json!("verdictGate"));
    assert_eq!(wire(ChangeStepKind::RetryGate), json!("retryGate"));
    assert_eq!(wire(ChangeStepKind::WhitelistGate), json!("whitelistGate"));

    // 四类可辨（二十值互异）
    let all = [
        ChangeStepKind::Executor,
        ChangeStepKind::Evaluator,
        ChangeStepKind::Decision,
        ChangeStepKind::PhaseStart,
        ChangeStepKind::StaticCheck,
        ChangeStepKind::TestExecution,
        ChangeStepKind::MutationTest,
        ChangeStepKind::PhaseLog,
        ChangeStepKind::VerdictGate,
        ChangeStepKind::RetryGate,
        ChangeStepKind::WhitelistGate,
        ChangeStepKind::Preflight,
        ChangeStepKind::Commit,
        ChangeStepKind::Rebase,
        ChangeStepKind::ResolveConflicts,
        ChangeStepKind::Merge,
        ChangeStepKind::SpecSync,
        ChangeStepKind::Cleanup,
        ChangeStepKind::Seal,
        ChangeStepKind::Finalize,
    ];
    assert_eq!(all.len(), 20, "二十值穷举无遗漏");
    let mut wires: Vec<String> = all
        .iter()
        .map(|kind| wire(*kind).as_str().expect("串值").to_owned())
        .collect();
    wires.sort();
    wires.dedup();
    assert_eq!(wires.len(), 20, "二十值词汇互异（四类节点可辨）");

    // 身份串词面与 workflow 落库侧 `RunStepKind::as_str()` 逐字同源
    for (kind, step_kind) in [
        (
            ChangeStepKind::Preflight,
            workflow::state::RunStepKind::Preflight,
        ),
        (ChangeStepKind::Commit, workflow::state::RunStepKind::Commit),
        (ChangeStepKind::Rebase, workflow::state::RunStepKind::Rebase),
        (
            ChangeStepKind::ResolveConflicts,
            workflow::state::RunStepKind::ResolveConflicts,
        ),
        (ChangeStepKind::Merge, workflow::state::RunStepKind::Merge),
        (
            ChangeStepKind::SpecSync,
            workflow::state::RunStepKind::SpecSync,
        ),
        (
            ChangeStepKind::Cleanup,
            workflow::state::RunStepKind::Cleanup,
        ),
        (ChangeStepKind::Seal, workflow::state::RunStepKind::Seal),
        (
            ChangeStepKind::Finalize,
            workflow::state::RunStepKind::Finalize,
        ),
    ] {
        assert_eq!(
            kind.as_str(),
            step_kind.as_str(),
            "身份串词面跨侧逐字同源（落库零映射）"
        );
    }
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
            "instanceNo": 0,
            "status": "running",
            "sessionId": null,
            "detail": null,
            "outputRef": null,
            "startedAt": 1_726_000_000_000_i64,
            "endedAt": null,
            "tokenInput": null,
            "tokenOutput": null
        }),
        "步状态行十二字段 camelCase 线面（步级时间与 token 增补）"
    );

    // 有值形态 serde 往返无损
    let full = ChangeStepState {
        phase: "dev-design".to_owned(),
        attempt: 3,
        step: ChangeStepKind::Executor,
        instance_no: 0,
        status: ChangeStepStatus::Failed,
        session_id: Some("sess-9".to_owned()),
        detail: Some("会话失败收敛".to_owned()),
        output_ref: None,
        started_at: 1_726_000_000_000,
        ended_at: Some(1_726_000_005_000),
        token_input: Some(1200),
        token_output: Some(300),
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
        archive: None,
        started_at: 1_726_000_000_000,
        nodes: Vec::new(),
        edges: Vec::new(),
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
        archive: None,
        started_at: 1_726_000_000_000,
        nodes: Vec::new(),
        edges: Vec::new(),
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

/// 正向：TestExecution 出线 `testExecution` camelCase 逐字断言（serde
/// rename_all camelCase 线格式；二十值封闭集下专词断言持衡）。
#[test]
fn change_step_kind专词_testexecution出线逐字() {
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

/// 边界：二十值词汇互异（camelCase 判别值集合无碰撞——四类节点可辨不变量
/// 保持：WorkerAgent 五角色 + ToolStep 六步（含 mutationTest）+ Git 五步（含
/// cleanup）+ Gate 三门）。
#[test]
fn change_step_kind二十值词汇互异无碰撞() {
    let all = [
        ChangeStepKind::Executor,
        ChangeStepKind::Evaluator,
        ChangeStepKind::Decision,
        ChangeStepKind::PhaseStart,
        ChangeStepKind::StaticCheck,
        ChangeStepKind::TestExecution,
        ChangeStepKind::MutationTest,
        ChangeStepKind::PhaseLog,
        ChangeStepKind::VerdictGate,
        ChangeStepKind::RetryGate,
        ChangeStepKind::WhitelistGate,
        ChangeStepKind::Preflight,
        ChangeStepKind::Commit,
        ChangeStepKind::Rebase,
        ChangeStepKind::ResolveConflicts,
        ChangeStepKind::Merge,
        ChangeStepKind::SpecSync,
        ChangeStepKind::Cleanup,
        ChangeStepKind::Seal,
        ChangeStepKind::Finalize,
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
        20,
        "二十值词汇互异（无 camelCase 碰撞），实际: {wires:?}"
    );
}

/// cleanup 专词（正向 / 边界，worktree-cleanup-and-active-cap）：`ChangeStepKind::
/// Cleanup` 出线 `"cleanup"` 逐字（camel 与 snake 同形）、`"cleanup"` 序列化 /
/// 反序列化双向派生往返等值（rename_all camelCase 线格式持衡）、与落库侧
/// `workflow::state::RunStepKind::Cleanup` 的 `as_str()` 身份串词面逐字同源
/// （边表端点跨两侧可比）。
#[test]
fn change_step_kind_cleanup专词出线往返与runstepkind同源() {
    // as_str 身份串词面（snake_case 口径）
    assert_eq!(ChangeStepKind::Cleanup.as_str(), "cleanup");
    // serde 出线 = camelCase 线词（cleanup 同形逐字）
    let wire = serde_json::to_value(ChangeStepKind::Cleanup).expect("词汇出线应成功");
    assert_eq!(wire, json!("cleanup"), "camel 与 snake 同形逐字");
    // 往返：出线词 → 反序列化 → 等值（双向派生可用）
    let back: ChangeStepKind = serde_json::from_value(wire).expect("往返应成功");
    assert_eq!(back, ChangeStepKind::Cleanup, "\"cleanup\" 双向往返等值");
    // serde 串形态往返
    let text = serde_json::to_string(&ChangeStepKind::Cleanup).expect("出线应成功");
    assert_eq!(text, "\"cleanup\"", "线串逐字（零包装）");
    let back: ChangeStepKind = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, ChangeStepKind::Cleanup);
    // 词面同源对拍（对拍表补行）：身份串词与落库侧 RunStepKind 逐字一致
    assert_eq!(
        ChangeStepKind::Cleanup.as_str(),
        workflow::state::RunStepKind::Cleanup.as_str(),
        "身份串词面跨侧逐字同源（落库零映射）"
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
        instance_no: 0,
        status: ChangeStepStatus::Running,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at: 1_726_000_000_000,
        ended_at: None,
        token_input: None,
        token_output: None,
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
        output_ref: None,
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
            phase: "test-gen".to_owned(),
            archive: None
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
            archive: None,
        },
        RunUpdate::Finished {
            status: ChangeRunStatus::Stopped,
            reason: None,
        },
    ] {
        let notice = crate::state::RunNotice::from(&update);
        assert_eq!(
            serde_json::to_value(notice).expect("出线应成功")["ipc"],
            json!(update.kind()),
            "Notice 判别词与 RunUpdate::kind() 逐字同源"
        );
    }
}

/// 快照扩面出线：steps 累积器逐条 ChangeStepState camelCase 全词汇形态 +
/// startedAt 毫秒（AC-10 重挂步表面——统一视图活面投影源的线面）。
#[test]
fn 快照扩面_startedat与nodes全词汇camelcase出线() {
    let steps = vec![
        ChangeStepState {
            phase: "implement".to_owned(),
            attempt: 1,
            step: ChangeStepKind::Executor,
            instance_no: 0,
            status: ChangeStepStatus::Passed,
            session_id: Some("ses-1".to_owned()),
            detail: Some("实现完成".to_owned()),
            output_ref: None,
            started_at: 1_726_000_000_000,
            ended_at: Some(1_726_000_060_000),
            token_input: Some(12_000),
            token_output: Some(3_000),
        },
        ChangeStepState {
            phase: "implement".to_owned(),
            attempt: 1,
            step: ChangeStepKind::StaticCheck,
            instance_no: 0,
            status: ChangeStepStatus::Failed,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 1_726_000_060_000,
            ended_at: Some(1_726_000_062_000),
            token_input: None,
            token_output: None,
        },
        ChangeStepState {
            phase: "test-execution".to_owned(),
            attempt: 1,
            step: ChangeStepKind::TestExecution,
            instance_no: 0,
            status: ChangeStepStatus::Running,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 1_726_000_062_000,
            ended_at: None,
            token_input: None,
            token_output: None,
        },
        ChangeStepState {
            phase: "dev-design".to_owned(),
            attempt: 2,
            step: ChangeStepKind::PhaseStart,
            instance_no: 0,
            status: ChangeStepStatus::Passed,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 1_726_000_062_000,
            ended_at: Some(1_726_000_062_000),
            token_input: None,
            token_output: None,
        },
        ChangeStepState {
            phase: "dev-design".to_owned(),
            attempt: 2,
            step: ChangeStepKind::VerdictGate,
            instance_no: 0,
            status: ChangeStepStatus::Stopped,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 1_726_000_062_000,
            ended_at: Some(1_726_000_063_000),
            token_input: None,
            token_output: None,
        },
    ];
    let snapshot = ChangeRunSnapshot {
        run_id: "run-9".to_owned(),
        status: ChangeRunStatus::Running,
        phase: Some("test-execution".to_owned()),
        attempt: Some(1),
        ask: None,
        archive: None,
        started_at: 1_726_000_000_000,
        nodes: steps,
        edges: Vec::new(),
    };
    let value = serde_json::to_value(&snapshot).expect("出线应成功");
    assert_eq!(
        value["startedAt"],
        json!(1_726_000_000_000_i64),
        "startedAt 毫秒"
    );
    assert_eq!(value["nodes"].as_array().map(Vec::len), Some(5));
    assert!(
        value["edges"].as_array().is_some_and(Vec::is_empty),
        "快照 edges 键恒在场（零边 run 空数组）"
    );
    // 节点词汇 camelCase 线面逐条（快照透传 ChangeStepState 原生线格式）
    assert_eq!(value["nodes"][0]["step"], json!("executor"));
    assert_eq!(value["nodes"][1]["step"], json!("staticCheck"));
    assert_eq!(value["nodes"][2]["step"], json!("testExecution"));
    assert_eq!(value["nodes"][3]["step"], json!("phaseStart"));
    assert_eq!(value["nodes"][4]["step"], json!("verdictGate"));
    assert_eq!(value["nodes"][1]["status"], json!("failed"));
    assert_eq!(value["nodes"][4]["status"], json!("stopped"));
    // 稳定身份键随节点出线（instanceNo + outputRef）
    assert_eq!(value["nodes"][0]["instanceNo"], json!(0));
    assert_eq!(
        value["nodes"][0]["outputRef"],
        json!(null),
        "null 留位不省键"
    );

    // serde 往返无损（扩面字段随行）
    let text = serde_json::to_string(&snapshot).expect("出线应成功");
    let back: ChangeRunSnapshot = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, snapshot, "快照（含 nodes / edges 累积器）往返无损");
}

// ---------------------------------------------------------------------------
// 执行图边（EdgeKind 四值 / ChangeStepEdge 端点身份串 / Edge 信封）
// ---------------------------------------------------------------------------

/// `EdgeKind` 四值 camelCase 线面逐字出线（forward / feedback / retry /
/// backtrack）；非法词反序列化拒绝（封闭集防线，AC-3 / AC-6 / AC-9）。
#[test]
fn edge_kind四值线面camelcase逐字与封闭集拒绝() {
    let wire = |kind: crate::state::EdgeKind| serde_json::to_value(kind).expect("出线应成功");
    assert_eq!(wire(crate::state::EdgeKind::Forward), json!("forward"));
    assert_eq!(wire(crate::state::EdgeKind::Feedback), json!("feedback"));
    assert_eq!(wire(crate::state::EdgeKind::Retry), json!("retry"));
    assert_eq!(wire(crate::state::EdgeKind::Backtrack), json!("backtrack"));

    for illegal in ["feedBack", "Forward", "back_track", "unknown", ""] {
        let back: Result<crate::state::EdgeKind, _> = serde_json::from_value(json!(illegal));
        assert!(back.is_err(), "EdgeKind 越集词 {illegal:?} 应拒绝");
    }
}

/// `ChangeStepEdge` 三字段（from / to / kind）往返可辨；端点身份串恒为
/// `"{phase}:{attempt}:{step}:{instance_no}"`（`from_node_key` / `to_node_key`
/// 与落库侧 `RunNodeRecord::node_id()` 同式，AC-3 / AC-6）。
#[test]
fn change_step_edge往返与端点身份串() {
    let edge = crate::state::ChangeStepEdge {
        from: ("implement".to_owned(), 1, ChangeStepKind::StaticCheck, 0),
        to: ("implement".to_owned(), 1, ChangeStepKind::Executor, 1),
        kind: crate::state::EdgeKind::Feedback,
    };
    assert_eq!(
        edge.from_node_key(),
        "implement:1:static_check:0",
        "源端点身份串（snake_case 步词汇）"
    );
    assert_eq!(edge.to_node_key(), "implement:1:executor:1");
    assert_eq!(
        crate::state::node_id(&edge.from),
        edge.from_node_key(),
        "node_id 组装单点与访问器同式"
    );

    let text = serde_json::to_string(&edge).expect("出线应成功");
    assert!(text.contains("\"kind\":\"feedback\""), "kind 线词: {text}");
    assert!(
        text.contains("[\"implement\",1,\"staticCheck\",0]"),
        "进程内 seam 载荷：端点四元组直出（身份键语义），实际: {text}"
    );
    let back: crate::state::ChangeStepEdge = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, edge, "边往返无损");

    // 三字段逐项可辨：改 kind / 改实例号即不等价（四 kind 与实例号不串扰）
    assert_ne!(
        crate::state::ChangeStepEdge {
            kind: crate::state::EdgeKind::Retry,
            ..edge.clone()
        },
        edge,
        "feedback 与 retry 可辨"
    );
    assert_ne!(
        crate::state::ChangeStepEdge {
            to: ("implement".to_owned(), 1, ChangeStepKind::Executor, 2),
            ..edge.clone()
        },
        edge,
        "目标实例号可辨"
    );
}

/// `RunUpdate::Edge` 变体：kind() 访问器 == "edge"、线面载荷形态固定（边信封
/// 与 step 信封同型判别，AC-6 / AC-9 通知桥前置）。
#[test]
fn run_update_edge变体出线与kind一致() {
    let edge = crate::state::ChangeStepEdge {
        from: ("proposal".to_owned(), 1, ChangeStepKind::Executor, 0),
        to: ("proposal".to_owned(), 1, ChangeStepKind::StaticCheck, 0),
        kind: crate::state::EdgeKind::Forward,
    };
    let update = RunUpdate::Edge { edge: edge.clone() };
    assert_eq!(update.kind(), "edge", "kind 判别词逐字");

    let value = serde_json::to_value(&update).expect("出线应成功");
    assert_eq!(value["ipc"], json!("edge"), "tag=ipc 判别值逐字 edge");
    assert_eq!(
        value["edge"]["from"],
        json!(["proposal", 1, "executor", 0]),
        "源端点四元组随信封出线（进程内 seam 载荷）"
    );
    assert_eq!(value["edge"]["kind"], json!("forward"));
    assert_eq!(
        value["edge"]["to"],
        json!(["proposal", 1, "staticCheck", 0])
    );
}

/// `RunNotice::Edge`：kind-only 信封（零载荷、单键出线），`From<&RunUpdate>`
/// 投影一一同型（通知不带查询载荷，AC-6 / AC-9）。
#[test]
fn run_notice_edge变体kind_only与投影同型() {
    let notice = crate::state::RunNotice::Edge;
    assert_eq!(
        serde_json::to_value(notice).expect("通知出线应成功"),
        json!({"ipc": "edge"}),
        "edge kind-only 单键"
    );

    let update = RunUpdate::Edge {
        edge: crate::state::ChangeStepEdge {
            from: ("implement".to_owned(), 1, ChangeStepKind::Executor, 0),
            to: ("implement".to_owned(), 1, ChangeStepKind::Executor, 1),
            kind: crate::state::EdgeKind::Feedback,
        },
    };
    assert!(
        matches!(
            crate::state::RunNotice::from(&update),
            crate::state::RunNotice::Edge
        ),
        "投影一一同型（载荷剥离）"
    );
    // 投影结果零载荷：线面单键（信封形状防线——越权携查询载荷）
    let projected = crate::state::RunNotice::from(&update);
    let value = serde_json::to_value(projected).expect("出线应成功");
    assert_eq!(value.as_object().map(serde_json::Map::len), Some(1));
    assert!(value.get("edge").is_none(), "零载荷：不携边载荷");
}

/// 快照 `edges` 面：零边 run 空数组；多边快照按 emit 序稳定（克隆语义）；
/// 悬空端点形态（数据缺口）序列化不 panic（稳健面，AC-5 / AC-9）。
#[test]
fn 快照edges面零边空数组与多边序稳定() {
    let empty = ChangeRunSnapshot {
        run_id: "run-1".to_owned(),
        status: ChangeRunStatus::Running,
        phase: None,
        attempt: None,
        ask: None,
        archive: None,
        started_at: 1_726_000_000_000,
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    let value = serde_json::to_value(&empty).expect("出线应成功");
    assert!(value["edges"].as_array().is_some_and(Vec::is_empty));

    let edges: Vec<crate::state::ChangeStepEdge> = [
        crate::state::EdgeKind::Forward,
        crate::state::EdgeKind::Feedback,
        crate::state::EdgeKind::Retry,
        crate::state::EdgeKind::Backtrack,
    ]
    .iter()
    .enumerate()
    .map(|(index, kind)| crate::state::ChangeStepEdge {
        from: (
            "implement".to_owned(),
            1,
            ChangeStepKind::Executor,
            index as u32,
        ),
        to: ("implement".to_owned(), 1, ChangeStepKind::PhaseLog, 0),
        kind: *kind,
    })
    .collect();
    let snapshot = ChangeRunSnapshot {
        edges: edges.clone(),
        ..empty
    };
    assert_eq!(snapshot.edges, edges, "多边按 emit 序稳定");
    let back: ChangeRunSnapshot =
        serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).expect("往返应成功");
    assert_eq!(back, snapshot, "多边快照往返无损");
    let value = serde_json::to_value(&snapshot).expect("出线应成功");
    let kinds: Vec<&str> = value["edges"]
        .as_array()
        .expect("edges 数组")
        .iter()
        .map(|edge| edge["kind"].as_str().expect("kind 为串"))
        .collect();
    assert_eq!(kinds, vec!["forward", "feedback", "retry", "backtrack"]);
}

/// `ChangeStepState` 稳定身份键：同 `(phase, attempt, step)` 多实例经
/// `instance_no` 递增可辨；同键同实例号两行身份串相同（碰撞防线，AC-3 / AC-4）。
#[test]
fn change_step_state实例号递增可辨() {
    let first = sample_step();
    let repaired = ChangeStepState {
        instance_no: 1,
        detail: Some("feedback 修复重跑".to_owned()),
        output_ref: Some("exec-1".to_owned()),
        ..first.clone()
    };
    assert_ne!(first, repaired, "实例号递增两节点可辨");
    assert_eq!(first.phase, repaired.phase);
    assert_eq!(first.attempt, repaired.attempt);
    assert_eq!(first.step, repaired.step);

    let key = |state: &ChangeStepState| {
        (
            state.phase.clone(),
            state.attempt,
            state.step,
            state.instance_no,
        )
    };
    assert_ne!(key(&first), key(&repaired), "身份键四元组互异");

    // 同 (phase, attempt, step, instance_no) 两节点身份串相同 → 碰撞可察
    let collision = ChangeStepState {
        status: ChangeStepStatus::Failed,
        ..first.clone()
    };
    assert_eq!(key(&collision), key(&first), "同键同实例号身份串碰撞可察");

    // output_ref 两态：None（Transient 即弃）与 Some(exec_id) 可辨
    assert_eq!(first.output_ref, None);
    assert_eq!(repaired.output_ref.as_deref(), Some("exec-1"));
    assert_ne!(
        ChangeStepState {
            output_ref: Some(String::new()),
            ..first.clone()
        },
        first,
        "空串（落盘失败空串口径）与合法 exec_id 可辨"
    );
}

// ---------------------------------------------------------------------------
// ArchivePreflight 停等载荷与 ConfirmWait 双态（AC-7 / D2）
// ---------------------------------------------------------------------------

/// 归档前置只读载荷 fixture（八字段；停等面与 cursor 消费同一像）。
fn sample_preflight() -> ArchivePreflight {
    ArchivePreflight {
        name: "demo-change".to_owned(),
        completed: true,
        incomplete_phases: Vec::new(),
        missing_artifacts: Vec::new(),
        delta_specs: vec!["specs/desktop-flow/spec.md".to_owned()],
        worktree: Some("/tmp/wt/demo-change".to_owned()),
        branch: Some("change/demo-change".to_owned()),
        merge_target: Some("main".to_owned()),
    }
}

/// ArchivePreflight 形状（正向）：八字段 camelCase 线面逐字段可辨，serde
/// 往返无损，specta `Type` 可达（AC-7 / D2 停等载荷）。
#[test]
fn archive_preflight八字段线面与往返可辨() {
    let preflight = sample_preflight();
    let value = serde_json::to_value(&preflight).expect("出线应成功");
    assert_eq!(value["name"], json!("demo-change"));
    assert_eq!(value["completed"], json!(true));
    assert_eq!(value["incompletePhases"], json!([]));
    assert_eq!(value["missingArtifacts"], json!([]));
    assert_eq!(value["deltaSpecs"], json!(["specs/desktop-flow/spec.md"]));
    assert_eq!(value["worktree"], json!("/tmp/wt/demo-change"));
    assert_eq!(value["branch"], json!("change/demo-change"));
    assert_eq!(value["mergeTarget"], json!("main"));

    let text = serde_json::to_string(&preflight).expect("出线应成功");
    let back: ArchivePreflight = serde_json::from_str(&text).expect("往返应成功");
    assert_eq!(back, preflight, "归档前置载荷往返无损");

    // 逐字段可辨（八字段任一翻转即不等价）
    assert_ne!(
        ArchivePreflight {
            completed: false,
            ..preflight.clone()
        },
        preflight,
        "completed 两态可辨"
    );
    assert_ne!(
        ArchivePreflight {
            merge_target: None,
            ..preflight.clone()
        },
        preflight,
        "mergeTarget 两态可辨"
    );
    assert_ne!(
        ArchivePreflight {
            delta_specs: Vec::new(),
            ..preflight.clone()
        },
        preflight,
        "deltaSpecs 空/非空可辨"
    );
}

/// ArchivePreflight 边界（边界）：`completed` 两态（true / false）；
/// `incomplete_phases` / `missing_artifacts` / `delta_specs` 空 Vec / 单元素 /
/// 超长列表；`worktree` / `branch` / `merge_target` None 与 Some 两态
///（AC-7 未完成警告面）。
#[test]
fn archive_preflight边界两态与列表形态() {
    // 未完成形态：completed=false + 未过相位清单
    let incomplete = ArchivePreflight {
        name: "demo-change".to_owned(),
        completed: false,
        incomplete_phases: vec!["implement".to_owned()],
        missing_artifacts: vec!["design.md".to_owned()],
        delta_specs: Vec::new(),
        worktree: None,
        branch: None,
        merge_target: None,
    };
    let value = serde_json::to_value(&incomplete).expect("出线应成功");
    assert_eq!(value["completed"], json!(false));
    assert_eq!(value["incompletePhases"], json!(["implement"]));
    assert_eq!(value["missingArtifacts"], json!(["design.md"]));
    assert_eq!(value["worktree"], json!(null));
    assert_eq!(value["branch"], json!(null));
    assert_eq!(value["mergeTarget"], json!(null));

    // 超长列表边界（128 项）不 panic、逐条透出
    let long: Vec<String> = (0..128).map(|index| format!("phase-{index:03}")).collect();
    let wide = ArchivePreflight {
        incomplete_phases: long.clone(),
        delta_specs: long.clone(),
        ..incomplete.clone()
    };
    let value = serde_json::to_value(&wide).expect("出线应成功");
    assert_eq!(
        value["incompletePhases"].as_array().map(Vec::len),
        Some(128),
        "超长未过相位清单逐条透出"
    );
    assert_eq!(value["deltaSpecs"].as_array().map(Vec::len), Some(128));
    let back: ArchivePreflight =
        serde_json::from_str(&serde_json::to_string(&wide).expect("x")).expect("往返应成功");
    assert_eq!(back, wide, "超长列表往返无损");

    // 空串 name 边界（中性类型零校验——拦截归构造侧）
    let blank_name = ArchivePreflight {
        name: String::new(),
        ..incomplete.clone()
    };
    assert_eq!(blank_name.name, "");
    assert_ne!(blank_name, incomplete, "空 name 与实名可辨");
}

/// ArchivePreflight 异常（异常）：缺失必填字段（name / completed）反序列化
/// 拒绝；多余字段被忽略（线面契约守卫，AC-7）。
#[test]
fn archive_preflight缺失必填字段拒绝() {
    let value = serde_json::to_value(sample_preflight()).expect("出线应成功");

    let mut no_name = value.clone();
    no_name.as_object_mut().expect("对象").remove("name");
    assert!(
        serde_json::from_value::<ArchivePreflight>(no_name).is_err(),
        "缺 name 必填字段应拒绝"
    );

    let mut no_completed = value.clone();
    no_completed
        .as_object_mut()
        .expect("对象")
        .remove("completed");
    assert!(
        serde_json::from_value::<ArchivePreflight>(no_completed).is_err(),
        "缺 completed 必填字段应拒绝"
    );

    // 字段类型不匹配（completed 传串）拒绝
    let mut wrong_type = value.clone();
    wrong_type["completed"] = json!("yes");
    assert!(
        serde_json::from_value::<ArchivePreflight>(wrong_type).is_err(),
        "completed 类型错配应拒绝"
    );

    // 多余字段被忽略（向前兼容消费）
    let mut extra = value.clone();
    extra["futureField"] = json!("ignored");
    let back: ArchivePreflight = serde_json::from_value(extra).expect("多余字段应忽略而不拒绝");
    assert_eq!(back, sample_preflight());
}

/// RunUpdate::ConfirmWait 换形（正向）：archive=None（普通相位停等）与
/// archive=Some(preflight)（archive 前停等）两态出线可辨；`kind()` 仍
/// `"confirmWait"`、`RunNotice::ConfirmWait` 零载荷投影不变（AC-7 / D2）。
#[test]
fn confirm_wait双态归档载荷随行且kind不变() {
    let plain = RunUpdate::ConfirmWait {
        phase: "acceptance".to_owned(),
        archive: None,
    };
    let archive_wait = RunUpdate::ConfirmWait {
        phase: "archive".to_owned(),
        archive: Some(sample_preflight()),
    };

    let plain_value = serde_json::to_value(&plain).expect("出线应成功");
    let archive_value = serde_json::to_value(&archive_wait).expect("出线应成功");
    assert_eq!(plain_value["ipc"], json!("confirmWait"));
    assert_eq!(archive_value["ipc"], json!("confirmWait"));
    assert_eq!(plain_value["phase"], json!("acceptance"));
    assert_eq!(archive_value["phase"], json!("archive"));
    assert_eq!(plain_value["archive"], json!(null));
    assert_eq!(
        archive_value["archive"]["completed"],
        json!(true),
        "archive 前停等随行前置只读载荷"
    );
    assert_eq!(archive_value["archive"]["name"], json!("demo-change"));
    assert_ne!(plain_value, archive_value, "两态线面可辨");

    assert_eq!(plain.kind(), "confirmWait");
    assert_eq!(archive_wait.kind(), "confirmWait");

    use crate::state::RunNotice;
    assert!(matches!(RunNotice::from(&plain), RunNotice::ConfirmWait));
    assert!(matches!(
        RunNotice::from(&archive_wait),
        RunNotice::ConfirmWait
    ));
    assert_eq!(
        serde_json::to_value(RunNotice::from(&archive_wait)).expect("出线应成功"),
        json!({"ipc": "confirmWait"}),
        "广播零载荷：停等载荷不出进程"
    );
}

/// ConfirmWait 载荷形态守卫（异常 / 边界）：`archive` 键恒随行出线（缺键即
/// 线面漂移——进程内 seam 载荷无入站反序列化面，契约以出线键集与文档面钉
/// 死）；六变体 `RunNotice` 不变（零新 kind，AC-7 / D2）。
#[test]
fn confirm_wait载荷archive键恒在场且notice六变体不变() {
    let legacy = RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
        archive: None,
    };
    let value = serde_json::to_value(&legacy).expect("出线应成功");
    let keys: Vec<&str> = value
        .as_object()
        .expect("ConfirmWait 为对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys.len(),
        3,
        "ConfirmWait 恰三键（archive 为恒在场键、零键省形态）: {keys:?}"
    );
    assert!(
        keys.contains(&"archive") && keys.contains(&"phase") && keys.contains(&"ipc"),
        "键集 = ipc / phase / archive: {keys:?}"
    );
    assert_eq!(
        value["archive"],
        json!(null),
        "缺省 archive 出线 null（留位不省键）"
    );

    // 六变体 kind 词封闭集不变（Step / Edge / SessionEvent / Ask /
    // ConfirmWait / Finished）
    use crate::state::RunNotice;
    let notices = [
        RunNotice::Step,
        RunNotice::Edge,
        RunNotice::SessionEvent,
        RunNotice::Ask,
        RunNotice::ConfirmWait,
        RunNotice::Finished,
    ];
    let wires: Vec<String> = notices
        .iter()
        .map(|notice| {
            serde_json::to_value(notice).expect("出线应成功")["ipc"]
                .as_str()
                .expect("串值")
                .to_owned()
        })
        .collect();
    assert_eq!(
        wires,
        vec![
            "step",
            "edge",
            "sessionEvent",
            "ask",
            "confirmWait",
            "finished"
        ],
        "六变体 kind 词封闭集零新增"
    );
}

/// ChangeRunSnapshot.archive 出线（正向）：archive=Some(preflight) 与 None 两态
/// 直出线，终态除名后恒 None（AC-7 / AC-10 活面投影源）。
#[test]
fn 快照archive出线两态与none留位() {
    let with_archive = ChangeRunSnapshot {
        run_id: "run-archive".to_owned(),
        status: ChangeRunStatus::WaitingConfirm,
        phase: Some("archive".to_owned()),
        attempt: Some(1),
        ask: None,
        archive: Some(sample_preflight()),
        started_at: 1_726_000_000_000,
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    let value = serde_json::to_value(&with_archive).expect("出线应成功");
    assert_eq!(value["archive"]["name"], json!("demo-change"));
    assert_eq!(value["archive"]["completed"], json!(true));
    assert_eq!(value["archive"]["mergeTarget"], json!("main"));

    let without = ChangeRunSnapshot {
        archive: None,
        ..with_archive.clone()
    };
    let value = serde_json::to_value(&without).expect("出线应成功");
    assert_eq!(
        value["archive"],
        json!(null),
        "非归档停等 archive 槽恒 null（留位不省键）"
    );
    assert_eq!(
        serde_json::to_string(&without)
            .expect("出线应成功")
            .matches("\"archive\":")
            .count(),
        1,
        "archive 键恒在场（载荷槽 null 留位）"
    );

    let back: ChangeRunSnapshot =
        serde_json::from_str(&serde_json::to_string(&with_archive).expect("x"))
            .expect("往返应成功");
    assert_eq!(back, with_archive, "携带归档载荷的快照往返无损");
}

/// 正向：`ChangeStepKind` ↔ `RunStepKind` 1:1 对拍（20 对全等，双射无两词映
/// 一）——camelCase 活面词汇与 snake_case 落库词汇同源（`as_str()` 逐字可比，
/// AC-2 归一表在案：`mutation_test` ↔ `mutationTest`、`cleanup` ↔ `cleanup`）。
#[test]
fn change_step_kind与run_step_kind二十对同源对拍() {
    use workflow::state::RunStepKind;

    let pairs = [
        (ChangeStepKind::Executor, RunStepKind::Executor),
        (ChangeStepKind::Evaluator, RunStepKind::Evaluator),
        (ChangeStepKind::Decision, RunStepKind::Decision),
        (ChangeStepKind::PhaseStart, RunStepKind::PhaseStart),
        (ChangeStepKind::StaticCheck, RunStepKind::StaticCheck),
        (ChangeStepKind::TestExecution, RunStepKind::TestExecution),
        (ChangeStepKind::MutationTest, RunStepKind::MutationTest),
        (ChangeStepKind::PhaseLog, RunStepKind::PhaseLog),
        (ChangeStepKind::VerdictGate, RunStepKind::VerdictGate),
        (ChangeStepKind::RetryGate, RunStepKind::RetryGate),
        (ChangeStepKind::WhitelistGate, RunStepKind::WhitelistGate),
        (ChangeStepKind::Preflight, RunStepKind::Preflight),
        (ChangeStepKind::Commit, RunStepKind::Commit),
        (ChangeStepKind::Rebase, RunStepKind::Rebase),
        (
            ChangeStepKind::ResolveConflicts,
            RunStepKind::ResolveConflicts,
        ),
        (ChangeStepKind::Merge, RunStepKind::Merge),
        (ChangeStepKind::SpecSync, RunStepKind::SpecSync),
        (ChangeStepKind::Cleanup, RunStepKind::Cleanup),
        (ChangeStepKind::Seal, RunStepKind::Seal),
        (ChangeStepKind::Finalize, RunStepKind::Finalize),
    ];
    assert_eq!(pairs.len(), 20, "二十对全等对拍");

    let mut snake_words: Vec<&str> = Vec::new();
    for (kind, persisted) in pairs {
        assert_eq!(
            kind.as_str(),
            persisted.as_str(),
            "{kind:?} 两域 snake 词面逐字同源"
        );
        snake_words.push(kind.as_str());
    }
    // 双射：20 词互异（无两词映一——同源不漂移）
    let total = snake_words.len();
    snake_words.sort_unstable();
    snake_words.dedup();
    assert_eq!(snake_words.len(), total, "双射无两词映一: {snake_words:?}");
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（AC-1）：ChangeStepState 四字段（startedAt /
// endedAt / tokenInput / tokenOutput）wire 出线——camelCase 四键逐键、Option
// 缺席 null 不省键、0 值原样、极值保真、往返无损、旧形态拒绝
// ---------------------------------------------------------------------------

/// 正向：四字段全值步出线 → 线面四 camelCase 键逐键等值（数值口径，AC-1）。
#[test]
fn change_step_state四字段全值线面camelcase逐键等值() {
    let step = ChangeStepState {
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_200_000),
        token_input: Some(12_000),
        token_output: Some(3_400),
        ..sample_step()
    };
    let value = serde_json::to_value(&step).expect("出线应成功");
    assert_eq!(
        value["startedAt"],
        json!(1_727_000_000_000_i64),
        "startedAt camelCase 键数值口径"
    );
    assert_eq!(value["endedAt"], json!(1_727_000_200_000_i64));
    assert_eq!(value["tokenInput"], json!(12_000));
    assert_eq!(value["tokenOutput"], json!(3_400));
}

/// 正向：`RunUpdate::Step` 变体出线 → `step["step"]` 载荷四键在场（既有
/// run_update 线面用例随动扩断言——载荷自动跟随）。
#[test]
fn run_update_step变体出线携四键载荷自动跟随() {
    let step = ChangeStepState {
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_060_000),
        token_input: Some(500),
        token_output: Some(60),
        ..sample_step()
    };
    let value = serde_json::to_value(&RunUpdate::Step { step }).expect("出线应成功");
    assert_eq!(value["ipc"], json!("step"));
    assert_eq!(value["step"]["startedAt"], json!(1_727_000_000_000_i64));
    assert_eq!(value["step"]["endedAt"], json!(1_727_000_060_000_i64));
    assert_eq!(value["step"]["tokenInput"], json!(500));
    assert_eq!(value["step"]["tokenOutput"], json!(60));
}

/// 边界：running 形态（ended_at / token 两值全 None）→ 线面三键 `null` 不省键
///（与既有 sessionId / detail 同式）。
#[test]
fn change_step_state_running形态三键null不省键() {
    let step = ChangeStepState {
        status: ChangeStepStatus::Running,
        started_at: 1_727_000_000_000,
        ended_at: None,
        token_input: None,
        token_output: None,
        ..sample_step()
    };
    let value = serde_json::to_value(&step).expect("出线应成功");
    assert_eq!(value["endedAt"], json!(null), "endedAt null 留位不省键");
    assert_eq!(value["tokenInput"], json!(null));
    assert_eq!(value["tokenOutput"], json!(null));
    assert!(
        value.as_object().expect("对象线面").contains_key("endedAt")
            && value.as_object().unwrap().contains_key("tokenInput")
            && value.as_object().unwrap().contains_key("tokenOutput"),
        "三键在场（null 留位不省键）"
    );
}

/// 边界：started_at = 0（缺省语义值）→ 线面 `0` 原样（i64 非可选零改写）。
#[test]
fn change_step_state_started_at零值线面原样() {
    let step = ChangeStepState {
        started_at: 0,
        ..sample_step()
    };
    let value = serde_json::to_value(&step).expect("出线应成功");
    assert_eq!(value["startedAt"], json!(0), "0 缺席语义值原样出线");
}

/// 边界：token u64::MAX 邻近值 → 线面数值保真（JSON number 面不截断不 panic）。
#[test]
fn change_step_state_token极值线面数值保真() {
    let step = ChangeStepState {
        token_input: Some(u64::MAX),
        token_output: Some(u64::MAX - 1),
        ..sample_step()
    };
    let value = serde_json::to_value(&step).expect("出线应成功");
    assert_eq!(value["tokenInput"], json!(u64::MAX), "u64::MAX 数值保真");
    assert_eq!(value["tokenOutput"], json!(u64::MAX - 1));
}

/// 异常：线面 JSON 回读（`from_value`）→ 四字段逐字段还原（Option `null` →
/// None 还原无损）。
#[test]
fn change_step_state线面回读四字段还原无损() {
    // 全值形态
    let full = ChangeStepState {
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_200_000),
        token_input: Some(12_000),
        token_output: Some(3_400),
        ..sample_step()
    };
    let value = serde_json::to_value(&full).expect("出线应成功");
    let back: ChangeStepState = serde_json::from_value(value).expect("回读应成功");
    assert_eq!(back, full, "全值形态往返无损");

    // None 形态：线面 null → None 还原
    let running = ChangeStepState {
        status: ChangeStepStatus::Running,
        started_at: 1_727_000_000_000,
        ended_at: None,
        token_input: None,
        token_output: None,
        ..sample_step()
    };
    let value = serde_json::to_value(&running).expect("出线应成功");
    assert_eq!(value["endedAt"], json!(null));
    let back: ChangeStepState = serde_json::from_value(value).expect("回读应成功");
    assert_eq!(back, running, "None 形态往返无损");
}

/// 异常：缺四键旧形态 JSON 反序列化 → Err（活面契约无 `#[serde(default)]`——
/// 旧行兼容仅落库 `RunNodeRecord` 面承载，两载体语义分立锚，AC-5）。
#[test]
fn change_step_state缺四键旧形态反序列化拒绝() {
    let legacy = json!({
        "phase": "implement",
        "attempt": 1,
        "step": "executor",
        "instanceNo": 0,
        "status": "passed",
        "sessionId": null,
        "detail": null,
        "outputRef": null
    });
    let back: Result<ChangeStepState, _> = serde_json::from_value(legacy);
    assert!(back.is_err(), "缺四键旧形态应 Err（活面契约无 default）");

    // 拒绝锚 = 非 Option 的 `started_at`（i64 非可选）：仅缺 startedAt 的形态
    // 亦 Err——活面契约的时间字段恒在场（Option 键缺省 None 由 serde 语义承
    // 带，非 #[serde(default)] 声明面）
    let missing_started = json!({
        "phase": "implement",
        "attempt": 1,
        "step": "executor",
        "instanceNo": 0,
        "status": "passed",
        "sessionId": null,
        "detail": null,
        "outputRef": null,
        "endedAt": null,
        "tokenInput": null,
        "tokenOutput": null
    });
    let back: Result<ChangeStepState, _> = serde_json::from_value(missing_started);
    assert!(back.is_err(), "缺 startedAt 形态应 Err（i64 非可选）");
}
