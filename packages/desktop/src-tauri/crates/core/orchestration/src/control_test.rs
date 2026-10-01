//! `control.rs` 的单元测试（test-design「control.rs -> control_test.rs」节）。
//!
//! 装置：无 mock——`#[tokio::test]` 进程内 tokio 原语（broadcast / oneshot /
//! watch / Mutex）真实组合；多任务挂起/唤醒以 `tokio::spawn` + `yield_now`
//! 轮询握手（`confirm` / `answer` 的 `Ok` 返回即 pending 通道登记证明，单次
//! 通道 take 语义）。
//!
//! 注记：`RunGuard::finish` 消费 self——「重复 finish」在类型层不可表达（终
//! 态出口唯一），幂等收口以「终态除名、无残留可改写面」断言承载。

use std::sync::Arc;

use serde_json::json;

use crate::control::ChangeFlowControl;
use crate::state::{ChangeRunStatus, ChangeStepKind, ChangeStepState, ChangeStepStatus, RunUpdate};

/// 测试用 change 寻址键（注册表键 = change 名）。
const CHANGE: &str = "demo-change";

/// 信封出线形态（`RunUpdate` 无 `PartialEq`，等值经线面 JSON 比对）。
fn wire(update: &RunUpdate) -> serde_json::Value {
    serde_json::to_value(update).expect("RunUpdate 出线序列化应成功")
}

/// 步状态行 fixture（emit / publish 断言共用载荷）。
fn sample_step() -> ChangeStepState {
    ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 2,
        step: ChangeStepKind::Executor,
        status: ChangeStepStatus::Running,
        session_id: Some("sess-1".to_owned()),
        detail: None,
    }
}

/// 轮询至单次应答通道登记完成：`Ok` 即登记证明；上限内未成返回 false
/// （当前线程运行时以 yield_now 让位 spawned waiter 起跑）。
async fn until_ok(mut attempt: impl FnMut() -> Result<(), String>) -> bool {
    for _ in 0..1000 {
        if attempt().is_ok() {
            return true;
        }
        tokio::task::yield_now().await;
    }
    false
}

/// begin_run 登记与并行冲突：新 change begin_run 成功返回 RunGuard；同
/// change 二次 begin → Err（并行冲突检测——change_flow_start 前置校验第三
/// 分支的输入面）。
#[tokio::test]
async fn begin_run_registers_and_detects_parallel_conflict() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(CHANGE, "run-1".to_owned())
        .expect("新 change 首次 begin_run 应成功");
    assert!(!guard.cancelled(), "新登记 run 未置取消信号");

    assert_eq!(
        control.snapshot(CHANGE).map(|snap| snap.run_id),
        Some("run-1".to_owned()),
        "登记即快照可见"
    );

    let err = control
        .begin_run(CHANGE, "run-2".to_owned())
        .err()
        .expect("同 change 二次 begin 应 Err");
    assert!(err.contains(CHANGE), "错误显式携带 change 名: {err}");
    assert!(err.contains("已有运行中的 run"), "并行冲突记因: {err}");

    // 冲突被拒后首个 run 不受扰动（登记面稳定）
    assert_eq!(
        control.snapshot(CHANGE).map(|snap| snap.run_id),
        Some("run-1".to_owned())
    );
}

/// 跨 change 并行隔离：两个 change 各自 begin_run 并行互不干扰（键 =
/// change 名，D4）。
#[tokio::test]
async fn cross_change_parallel_isolation() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard_a = control.begin_run("change-a", "run-a".to_owned()).unwrap();
    let guard_b = control.begin_run("change-b", "run-b".to_owned()).unwrap();

    let mut rx_a = control.subscribe("change-a").unwrap();
    let mut rx_b = control.subscribe("change-b").unwrap();

    // A 的状态流不串台到 B
    guard_a.emit(RunUpdate::ConfirmWait {
        phase: "proposal".to_owned(),
    });
    let got_a = rx_a.recv().await.unwrap();
    assert_eq!(
        wire(&got_a),
        json!({ "ipc": "confirmWait", "phase": "proposal" })
    );
    assert!(
        rx_b.try_recv().is_err(),
        "B 的订阅不应收到 A 的信封（broadcast 按 change 隔离）"
    );

    // 停止寻址按 change 独立：A 置位不波及 B
    assert!(control.request_stop("change-a"));
    assert!(guard_a.cancelled(), "A 的取消信号同步观测");
    assert!(!guard_b.cancelled(), "B 不受 A 停止影响");

    // 快照面各自独立
    assert_eq!(
        control.snapshot("change-a").map(|snap| snap.run_id),
        Some("run-a".to_owned())
    );
    assert_eq!(
        control.snapshot("change-b").map(|snap| snap.run_id),
        Some("run-b".to_owned())
    );
}

/// request_stop 置位与幂等：运行中置位 → true 且 `guard.cancelled()` 同步
/// 观测；无运行 run → false 幂等不报错（change_flow_stop 幂等语义的输入面
/// ——AC-7 停止半边）。
#[tokio::test]
async fn request_stop_sets_cancel_flag_and_miss_is_idempotent() {
    let control = Arc::new(ChangeFlowControl::new());

    // miss 半边：无运行 run 幂等返回 false
    assert!(!control.request_stop(CHANGE), "无运行 run 应返回 false");

    let guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();
    assert!(!guard.cancelled(), "未停止前取消信号为假");

    assert!(control.request_stop(CHANGE), "运行中置位返回 true");
    assert!(guard.cancelled(), "取消信号同步观测（watch 置位）");
}

/// subscribe 有无运行：有 run → Some(receiver)；无 run → None
/// （change_flow_watch 消费形态）。
#[tokio::test]
async fn subscribe_some_with_run_none_without() {
    let control = Arc::new(ChangeFlowControl::new());
    assert!(control.subscribe(CHANGE).is_none(), "无 run → None");

    let _guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();
    assert!(
        control.subscribe(CHANGE).is_some(),
        "有 run → Some(receiver)"
    );
}

/// emit → broadcast 流出：`guard.emit(update)` 经 subscribe receiver 收到
/// 同值信封（RunUpdate 流出口——D9 broadcast 半边）。
#[tokio::test]
async fn emit_flows_to_broadcast_subscriber() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();

    let mut rx = control.subscribe(CHANGE).unwrap();
    let update = RunUpdate::Step {
        step: sample_step(),
    };
    guard.emit(update.clone());

    let received = rx.recv().await.unwrap();
    assert_eq!(wire(&received), wire(&update), "订阅者收到同值信封");
}

/// set_session 槽往返：置 Some/None 往返（停止寻址的当前会话槽），control
/// 侧直写与 guard 侧写入同槽。
#[tokio::test]
async fn session_slot_set_clear_roundtrip() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();

    assert_eq!(control.current_session(CHANGE), None, "初值空槽");

    guard.set_session(Some("sess-1".to_owned()));
    assert_eq!(control.current_session(CHANGE), Some("sess-1".to_owned()));

    guard.set_session(None);
    assert_eq!(control.current_session(CHANGE), None, "置 None 清槽");

    // control 侧直写同槽（命令层 sink 桥在首个会话事件到达时同步）
    control.set_session(CHANGE, Some("sess-2".to_owned()));
    assert_eq!(control.current_session(CHANGE), Some("sess-2".to_owned()));
}

/// confirm 应答回传：`wait_confirm` 挂起后 confirm(proceed=true/false) 回传
/// 布尔（false 的收敛归 walker）；单次通道 take 语义——重复应答落空 Err。
#[tokio::test]
async fn confirm_resolves_suspended_waiter_single_shot() {
    // proceed=true 臂
    let control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(control.begin_run(CHANGE, "run-1".to_owned()).unwrap());
    let waiter_guard = Arc::clone(&guard);
    let waiter = tokio::spawn(async move { waiter_guard.wait_confirm().await });

    assert!(
        until_ok(|| control.confirm(CHANGE, true)).await,
        "confirm 的 Ok 即 pending 通道登记证明"
    );
    assert!(waiter.await.unwrap(), "proceed=true 回传 true");

    // take 语义：应答已消费，重复应答落空 Err
    let err = control.confirm(CHANGE, true).expect_err("重复应答应 Err");
    assert!(
        err.contains("无等待中的 phase 确认"),
        "单次通道已取走: {err}"
    );

    // proceed=false 臂：独立 run，false 原样回传（收敛归 walker）
    let control_b = Arc::new(ChangeFlowControl::new());
    let guard_b = Arc::new(control_b.begin_run(CHANGE, "run-2".to_owned()).unwrap());
    let waiter_b_guard = Arc::clone(&guard_b);
    let waiter_b = tokio::spawn(async move { waiter_b_guard.wait_confirm().await });

    assert!(until_ok(|| control_b.confirm(CHANGE, false)).await);
    assert!(!waiter_b.await.unwrap(), "proceed=false 回传 false");
}

/// confirm 无等待方：无运行 run → Err 携 change 名；有 run 无挂起 → Err
/// 「无等待中的 phase 确认」（单次通道语义，change_flow_confirm 错误面输入）。
#[tokio::test]
async fn confirm_without_waiter_errors() {
    let control = Arc::new(ChangeFlowControl::new());

    // 无运行 run
    let err = control.confirm(CHANGE, true).expect_err("无 run 应 Err");
    assert!(err.contains(CHANGE), "错误显式携带 change 名: {err}");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");

    // 有 run 无等待方
    let _guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();
    let err = control.confirm(CHANGE, true).expect_err("无挂起应 Err");
    assert_eq!(err, "当前无等待中的 phase 确认");
}

/// answer 应答回传：`wait_answer` 挂起后 answer(text) 回传原文（ask 应答回
/// 流）；单次通道 take 语义——重复应答落空 Err。
#[tokio::test]
async fn answer_returns_text_to_suspended_waiter() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(control.begin_run(CHANGE, "run-1".to_owned()).unwrap());
    let waiter_guard = Arc::clone(&guard);
    let waiter = tokio::spawn(async move { waiter_guard.wait_answer().await });

    assert!(
        until_ok(|| control.answer(CHANGE, "采用方案 B".to_owned())).await,
        "answer 的 Ok 即 pending 通道登记证明"
    );
    assert_eq!(
        waiter.await.unwrap().as_deref(),
        Some("采用方案 B"),
        "应答原文回流 walker"
    );

    // take 语义：应答已消费，重复应答落空 Err
    let err = control
        .answer(CHANGE, "再来一次".to_owned())
        .expect_err("重复应答应 Err");
    assert!(err.contains("无等待中的 ask"), "单次通道已取走: {err}");
}

/// answer 无等待方：无运行 run → Err 携 change 名；有 run 无挂起 → Err
/// 「无等待中的 ask」。
#[tokio::test]
async fn answer_without_waiter_errors() {
    let control = Arc::new(ChangeFlowControl::new());

    // 无运行 run
    let err = control
        .answer(CHANGE, "应答".to_owned())
        .expect_err("无 run 应 Err");
    assert!(err.contains(CHANGE), "错误显式携带 change 名: {err}");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");

    // 有 run 无等待方
    let _guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();
    let err = control
        .answer(CHANGE, "应答".to_owned())
        .expect_err("无挂起应 Err");
    assert_eq!(err, "当前无等待中的 ask");
}

/// finish 终态除名：finish(status) 后 snapshot → None、request_stop →
/// false、begin_run 可重新登记（幂等收口，AC-7「run 仅进程内」半边）；
/// Finished 信封先于除名到达订阅者。
#[tokio::test]
async fn finish_removes_entry_and_allows_rebegin() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();
    let mut rx = control.subscribe(CHANGE).unwrap();

    guard.finish(ChangeRunStatus::Stopped, Some("用户停止".to_owned()));

    // Finished 信封流出（终态收口唯一出口）
    let finished = rx.recv().await.unwrap();
    assert_eq!(
        wire(&finished),
        json!({ "ipc": "finished", "status": "stopped", "reason": "用户停止" })
    );

    // 终态除名三面
    assert!(control.snapshot(CHANGE).is_none(), "快照面除名 → None");
    assert!(
        !control.request_stop(CHANGE),
        "除名后 request_stop 幂等返回 false"
    );
    assert!(control.subscribe(CHANGE).is_none(), "订阅面除名 → None");

    // 可重新登记（run 仅进程内，桌面重启后自然消失）
    control
        .begin_run(CHANGE, "run-2".to_owned())
        .expect("终态除名后应可重新登记");
}

/// finish 幂等收口：snapshot 在 finish 前返回 Some（状态机镜像），finish
/// 消费 self（重复 finish 类型层不可表达——终态出口唯一）；除名后无残留
/// 可改写面（迟滞 publish 落空，首个终态不被改写）。
#[tokio::test]
async fn finish_single_shot_consumes_terminal_state() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });

    // finish 前：快照 Some（状态机镜像运行态）
    let before = control.snapshot(CHANGE).expect("finish 前快照应 Some");
    assert_eq!(before.status, ChangeRunStatus::Running);
    assert_eq!(before.run_id, "run-1");

    guard.finish(ChangeRunStatus::Stopped, None);

    // 除名后迟滞 publish 落空：无残留可改写面（首个终态不被改写）
    control.publish(
        CHANGE,
        RunUpdate::Finished {
            status: ChangeRunStatus::Failed,
            reason: Some("漂移".to_owned()),
        },
    );
    assert!(
        control.snapshot(CHANGE).is_none(),
        "除名后终态不可被改写（快照面无残留）"
    );
}

/// publish 快照面镜像：Step 推进 phase/attempt、ConfirmWait / Ask 迁移状态
/// 并落停等载荷、SessionEvent 零改写（发布单点 `publish` 的快照同步面）。
#[tokio::test]
async fn publish_mirrors_snapshot_state_machine() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control.begin_run(CHANGE, "run-1".to_owned()).unwrap();

    // Step：phase / attempt 推进，状态保持 running
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let snap = control.snapshot(CHANGE).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::Running);
    assert_eq!(snap.phase.as_deref(), Some("implement"));
    assert_eq!(snap.attempt, Some(2));
    assert!(snap.ask.is_none());

    // ConfirmWait：waitingConfirm + 停等定位
    guard.emit(RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
    });
    let snap = control.snapshot(CHANGE).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingConfirm);
    assert_eq!(snap.phase.as_deref(), Some("test-gen"));

    // Ask：waitingAsk + 中断载荷
    guard.emit(RunUpdate::Ask {
        question: "越权 backtrack 是否放行?".to_owned(),
        options: vec!["拒绝".to_owned(), "放行".to_owned()],
    });
    let snap = control.snapshot(CHANGE).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingAsk);
    let ask = snap.ask.expect("waitingAsk 应携中断载荷");
    assert_eq!(ask.question, "越权 backtrack 是否放行?");
    assert_eq!(ask.options, vec!["拒绝".to_owned(), "放行".to_owned()]);

    // SessionEvent：透传不改写快照面
    guard.emit(RunUpdate::SessionEvent {
        session_id: "sess-1".to_owned(),
        event: agent::AgentEvent {
            seq: 0,
            timestamp_ms: 1_726_000_000_000,
            kind: agent::AgentEventKind::Raw {
                event_type: "system".to_owned(),
                raw_json: "{}".to_owned(),
            },
        },
    });
    let snap = control.snapshot(CHANGE).unwrap();
    assert_eq!(
        snap.status,
        ChangeRunStatus::WaitingAsk,
        "会话事件不改写状态"
    );
    assert_eq!(snap.phase.as_deref(), Some("test-gen"));
}

/// 取消信号中断挂起停等：wait_confirm / wait_answer 挂起期间 request_stop
/// 置位即收敛（proceed=false / None，停止不必先应答——guard 侧 select 半边）。
#[tokio::test]
async fn cancel_interrupts_pending_waits() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(control.begin_run(CHANGE, "run-1".to_owned()).unwrap());

    let confirm_guard = Arc::clone(&guard);
    let confirm_waiter = tokio::spawn(async move { confirm_guard.wait_confirm().await });
    let answer_guard = Arc::clone(&guard);
    let answer_waiter = tokio::spawn(async move { answer_guard.wait_answer().await });

    // 让 waiter 起跑挂起（取消语义与登记次序无关：先置位亦即刻收敛）
    tokio::task::yield_now().await;

    assert!(control.request_stop(CHANGE), "停止置位");
    assert!(
        !confirm_waiter.await.unwrap(),
        "取消置位 → proceed=false 收敛"
    );
    assert_eq!(
        answer_waiter.await.unwrap(),
        None,
        "取消置位 → None（walker 收敛 stopped）"
    );
}
