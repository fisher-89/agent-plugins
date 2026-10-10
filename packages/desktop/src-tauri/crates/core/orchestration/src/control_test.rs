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

use crate::control::ChangeFlowControl;
use crate::state::{
    ChangeRunStatus, ChangeStepKind, ChangeStepState, ChangeStepStatus, RunNotice, RunUpdate,
};

/// 测试用 change 寻址键（注册表键 = change 名）。
const CHANGE: &str = "demo-change";

/// 复合键 workspace root 段（测试固定值）
const ROOT: &str = "/ws/root-a";

/// 发起时刻（UTC unix 毫秒，`begin_run` 加参后的机械随动固定值）。
const STARTED_AT: i64 = 1_726_000_000_000;

/// 信封出线形态（`RunUpdate` 无 `PartialEq`，等值经线面 JSON 比对）。
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
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .expect("新 change 首次 begin_run 应成功");
    assert!(!guard.cancelled(), "新登记 run 未置取消信号");

    assert_eq!(
        control.snapshot(ROOT, CHANGE).map(|snap| snap.run_id),
        Some("run-1".to_owned()),
        "登记即快照可见"
    );

    let err = control
        .begin_run(ROOT, CHANGE, "run-2".to_owned(), STARTED_AT)
        .err()
        .expect("同 change 二次 begin 应 Err");
    assert!(err.contains(CHANGE), "错误显式携带 change 名: {err}");
    assert!(err.contains("已有运行中的 run"), "并行冲突记因: {err}");

    // 冲突被拒后首个 run 不受扰动（登记面稳定）
    assert_eq!(
        control.snapshot(ROOT, CHANGE).map(|snap| snap.run_id),
        Some("run-1".to_owned())
    );
}

/// 跨 change 并行隔离：两个 change 各自 begin_run 并行互不干扰（键 =
/// change 名，D4）。
#[tokio::test]
async fn cross_change_parallel_isolation() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard_a = control
        .begin_run(ROOT, "change-a", "run-a".to_owned(), STARTED_AT)
        .unwrap();
    let guard_b = control
        .begin_run(ROOT, "change-b", "run-b".to_owned(), STARTED_AT)
        .unwrap();

    let mut rx_a = control.subscribe(ROOT, "change-a").unwrap();
    let mut rx_b = control.subscribe(ROOT, "change-b").unwrap();

    // A 的状态流不串台到 B
    guard_a.emit(RunUpdate::ConfirmWait {
        phase: "proposal".to_owned(),
    });
    let got_a = rx_a.recv().await.unwrap();
    assert!(
        matches!(got_a, RunNotice::ConfirmWait),
        "A 的状态流 kind-only 通知不串台（信封降位，AC-8）"
    );
    assert!(
        rx_b.try_recv().is_err(),
        "B 的订阅不应收到 A 的信封（broadcast 按 change 隔离）"
    );

    // 停止寻址按 change 独立：A 置位不波及 B
    assert!(control.request_stop(ROOT, "change-a"));
    assert!(guard_a.cancelled(), "A 的取消信号同步观测");
    assert!(!guard_b.cancelled(), "B 不受 A 停止影响");

    // 快照面各自独立
    assert_eq!(
        control.snapshot(ROOT, "change-a").map(|snap| snap.run_id),
        Some("run-a".to_owned())
    );
    assert_eq!(
        control.snapshot(ROOT, "change-b").map(|snap| snap.run_id),
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
    assert!(
        !control.request_stop(ROOT, CHANGE),
        "无运行 run 应返回 false"
    );

    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    assert!(!guard.cancelled(), "未停止前取消信号为假");

    assert!(control.request_stop(ROOT, CHANGE), "运行中置位返回 true");
    assert!(guard.cancelled(), "取消信号同步观测（watch 置位）");
}

/// subscribe 有无运行：有 run → Some(receiver)；无 run → None
/// （change_flow_watch 消费形态）。
#[tokio::test]
async fn subscribe_some_with_run_none_without() {
    let control = Arc::new(ChangeFlowControl::new());
    assert!(control.subscribe(ROOT, CHANGE).is_none(), "无 run → None");

    let _guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    assert!(
        control.subscribe(ROOT, CHANGE).is_some(),
        "有 run → Some(receiver)"
    );
}

/// emit → broadcast 流出：`guard.emit(update)` 经 subscribe receiver 收到
/// 同值信封（RunUpdate 流出口——D9 broadcast 半边）。
#[tokio::test]
async fn emit_flows_to_broadcast_subscriber() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();

    let mut rx = control.subscribe(ROOT, CHANGE).unwrap();
    let update = RunUpdate::Step {
        step: sample_step(),
    };
    guard.emit(update.clone());

    let received = rx.recv().await.unwrap();
    assert_eq!(
        received,
        RunNotice::Step,
        "订阅者收到同拍 kind 通知（信封降位）"
    );
}

/// set_session 槽往返：置 Some/None 往返（停止寻址的当前会话槽），control
/// 侧直写与 guard 侧写入同槽。
#[tokio::test]
async fn session_slot_set_clear_roundtrip() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();

    assert_eq!(control.current_session(ROOT, CHANGE), None, "初值空槽");

    guard.set_session(Some("sess-1".to_owned()));
    assert_eq!(
        control.current_session(ROOT, CHANGE),
        Some("sess-1".to_owned())
    );

    guard.set_session(None);
    assert_eq!(control.current_session(ROOT, CHANGE), None, "置 None 清槽");

    // control 侧直写同槽（命令层 sink 桥在首个会话事件到达时同步）
    control.set_session(ROOT, CHANGE, Some("sess-2".to_owned()));
    assert_eq!(
        control.current_session(ROOT, CHANGE),
        Some("sess-2".to_owned())
    );
}

/// confirm 应答回传：`wait_confirm` 挂起后 confirm(proceed=true/false) 回传
/// 布尔（false 的收敛归 walker）；单次通道 take 语义——重复应答落空 Err。
#[tokio::test]
async fn confirm_resolves_suspended_waiter_single_shot() {
    // proceed=true 臂
    let control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    let waiter_guard = Arc::clone(&guard);
    let waiter = tokio::spawn(async move { waiter_guard.wait_confirm().await });

    assert!(
        until_ok(|| control.confirm(ROOT, CHANGE, true)).await,
        "confirm 的 Ok 即 pending 通道登记证明"
    );
    assert!(waiter.await.unwrap(), "proceed=true 回传 true");

    // take 语义：应答已消费，重复应答落空 Err
    let err = control
        .confirm(ROOT, CHANGE, true)
        .expect_err("重复应答应 Err");
    assert!(
        err.contains("无等待中的 phase 确认"),
        "单次通道已取走: {err}"
    );

    // proceed=false 臂：独立 run，false 原样回传（收敛归 walker）
    let control_b = Arc::new(ChangeFlowControl::new());
    let guard_b = Arc::new(
        control_b
            .begin_run(ROOT, CHANGE, "run-2".to_owned(), STARTED_AT)
            .unwrap(),
    );
    let waiter_b_guard = Arc::clone(&guard_b);
    let waiter_b = tokio::spawn(async move { waiter_b_guard.wait_confirm().await });

    assert!(until_ok(|| control_b.confirm(ROOT, CHANGE, false)).await);
    assert!(!waiter_b.await.unwrap(), "proceed=false 回传 false");
}

/// confirm 无等待方：无运行 run → Err 携 change 名；有 run 无挂起 → Err
/// 「无等待中的 phase 确认」（单次通道语义，change_flow_confirm 错误面输入）。
#[tokio::test]
async fn confirm_without_waiter_errors() {
    let control = Arc::new(ChangeFlowControl::new());

    // 无运行 run
    let err = control
        .confirm(ROOT, CHANGE, true)
        .expect_err("无 run 应 Err");
    assert!(err.contains(CHANGE), "错误显式携带 change 名: {err}");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");

    // 有 run 无等待方
    let _guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let err = control
        .confirm(ROOT, CHANGE, true)
        .expect_err("无挂起应 Err");
    assert_eq!(err, "当前无等待中的 phase 确认");
}

/// answer 应答回传：`wait_answer` 挂起后 answer(text) 回传原文（ask 应答回
/// 流）；单次通道 take 语义——重复应答落空 Err。
#[tokio::test]
async fn answer_returns_text_to_suspended_waiter() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    let waiter_guard = Arc::clone(&guard);
    let waiter = tokio::spawn(async move { waiter_guard.wait_answer().await });

    assert!(
        until_ok(|| control.answer(ROOT, CHANGE, "采用方案 B".to_owned())).await,
        "answer 的 Ok 即 pending 通道登记证明"
    );
    assert_eq!(
        waiter.await.unwrap().as_deref(),
        Some("采用方案 B"),
        "应答原文回流 walker"
    );

    // take 语义：应答已消费，重复应答落空 Err
    let err = control
        .answer(ROOT, CHANGE, "再来一次".to_owned())
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
        .answer(ROOT, CHANGE, "应答".to_owned())
        .expect_err("无 run 应 Err");
    assert!(err.contains(CHANGE), "错误显式携带 change 名: {err}");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");

    // 有 run 无等待方
    let _guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let err = control
        .answer(ROOT, CHANGE, "应答".to_owned())
        .expect_err("无挂起应 Err");
    assert_eq!(err, "当前无等待中的 ask");
}

/// finish 终态除名：finish(status) 后 snapshot → None、request_stop →
/// false、begin_run 可重新登记（幂等收口，AC-7「run 仅进程内」半边）；
/// Finished 信封先于除名到达订阅者。
#[tokio::test]
async fn finish_removes_entry_and_allows_rebegin() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE).unwrap();

    guard.finish(ChangeRunStatus::Stopped, Some("用户停止".to_owned()));

    // Finished 信封流出（终态收口唯一出口）
    let finished = rx.recv().await.unwrap();
    assert!(
        matches!(finished, RunNotice::Finished),
        "Finished kind 通知流出（终态收口唯一出口，信封降位）"
    );

    // 终态除名三面
    assert!(
        control.snapshot(ROOT, CHANGE).is_none(),
        "快照面除名 → None"
    );
    assert!(
        !control.request_stop(ROOT, CHANGE),
        "除名后 request_stop 幂等返回 false"
    );
    assert!(
        control.subscribe(ROOT, CHANGE).is_none(),
        "订阅面除名 → None"
    );

    // 可重新登记（run 仅进程内，桌面重启后自然消失）
    control
        .begin_run(ROOT, CHANGE, "run-2".to_owned(), STARTED_AT)
        .expect("终态除名后应可重新登记");
}

/// finish 幂等收口：snapshot 在 finish 前返回 Some（状态机镜像），finish
/// 消费 self（重复 finish 类型层不可表达——终态出口唯一）；除名后无残留
/// 可改写面（迟滞 publish 落空，首个终态不被改写）。
#[tokio::test]
async fn finish_single_shot_consumes_terminal_state() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });

    // finish 前：快照 Some（状态机镜像运行态）
    let before = control
        .snapshot(ROOT, CHANGE)
        .expect("finish 前快照应 Some");
    assert_eq!(before.status, ChangeRunStatus::Running);
    assert_eq!(before.run_id, "run-1");

    guard.finish(ChangeRunStatus::Stopped, None);

    // 除名后迟滞 publish 落空：无残留可改写面（首个终态不被改写）
    control.publish(
        ROOT,
        CHANGE,
        RunUpdate::Finished {
            status: ChangeRunStatus::Failed,
            reason: Some("漂移".to_owned()),
        },
    );
    assert!(
        control.snapshot(ROOT, CHANGE).is_none(),
        "除名后终态不可被改写（快照面无残留）"
    );
}

/// publish 快照面镜像：Step 推进 phase/attempt、ConfirmWait / Ask 迁移状态
/// 并落停等载荷、SessionEvent 零改写（发布单点 `publish` 的快照同步面）。
#[tokio::test]
async fn publish_mirrors_snapshot_state_machine() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();

    // Step：phase / attempt 推进，状态保持 running
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let snap = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::Running);
    assert_eq!(snap.phase.as_deref(), Some("implement"));
    assert_eq!(snap.attempt, Some(2));
    assert!(snap.ask.is_none());

    // ConfirmWait：waitingConfirm + 停等定位
    guard.emit(RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
    });
    let snap = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingConfirm);
    assert_eq!(snap.phase.as_deref(), Some("test-gen"));

    // Ask：waitingAsk + 中断载荷
    guard.emit(RunUpdate::Ask {
        question: "越权 backtrack 是否放行?".to_owned(),
        options: vec!["拒绝".to_owned(), "放行".to_owned()],
    });
    let snap = control.snapshot(ROOT, CHANGE).unwrap();
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
    let snap = control.snapshot(ROOT, CHANGE).unwrap();
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
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );

    let confirm_guard = Arc::clone(&guard);
    let confirm_waiter = tokio::spawn(async move { confirm_guard.wait_confirm().await });
    let answer_guard = Arc::clone(&guard);
    let answer_waiter = tokio::spawn(async move { answer_guard.wait_answer().await });

    // 让 waiter 起跑挂起（取消语义与登记次序无关：先置位亦即刻收敛）
    tokio::task::yield_now().await;

    assert!(control.request_stop(ROOT, CHANGE), "停止置位");
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

// ---------------------------------------------------------------------------
// 复合键并行（design D10）：异 workspace 同名不误拒 / 订阅隔离 / guard 除名
// 不误伤他键 / DTO 键集零 root 渗出
// ---------------------------------------------------------------------------

/// 异 workspace 同名不误拒（先例 bug 修正锚）：(rootA, x) run 运行中 →
/// (rootB, x) begin 成功（复合键含 workspace root 段——同名 change 在异
/// workspace 互不冲突）；同 workspace 两 change 并行互不干扰。
#[tokio::test]
async fn 复合键异workspace同名不误拒_同workspace两change并行() {
    let control = Arc::new(ChangeFlowControl::new());
    const ROOT_B: &str = "/ws/root-b";

    // (rootA, x) 登记
    let guard_a = control
        .begin_run(ROOT, CHANGE, "run-a".to_owned(), STARTED_AT)
        .expect("(rootA, x) 首登应成功");
    // (rootB, x) 同名 change 异 workspace → 不误拒
    let guard_b = control
        .begin_run(ROOT_B, CHANGE, "run-b".to_owned(), STARTED_AT)
        .expect("异 workspace 同名 change 不误拒（复合键 root 段生效）");
    assert!(
        !guard_b.cancelled(),
        "rootB 的 run 未受 rootA 同名 run 置位影响"
    );
    // 同 workspace 两 change 并行：互不冲突
    let guard_c = control
        .begin_run(ROOT, "other-change", "run-c".to_owned(), STARTED_AT)
        .expect("同 workspace 两 change 并行（真并行解锁）");

    // 三键各自快照可达
    assert_eq!(
        control.snapshot(ROOT, CHANGE).map(|snap| snap.run_id),
        Some("run-a".to_owned())
    );
    assert_eq!(
        control.snapshot(ROOT_B, CHANGE).map(|snap| snap.run_id),
        Some("run-b".to_owned())
    );
    assert_eq!(
        control
            .snapshot(ROOT, "other-change")
            .map(|snap| snap.run_id),
        Some("run-c".to_owned())
    );
    drop((guard_a, guard_b, guard_c));
}

/// 订阅隔离：(rootA, x) 与 (rootB, x) 各自 run 运行中——rootA 订阅者收到
/// (rootA, x) 的 publish 而**收不到** (rootB, x) 的任何信封（broadcast 按
/// 复合键寻址不串台）。
#[tokio::test]
async fn 复合键订阅隔离_broadcast按复合键寻址不串台() {
    let control = Arc::new(ChangeFlowControl::new());
    const ROOT_B: &str = "/ws/root-b";

    let guard_a = control
        .begin_run(ROOT, CHANGE, "run-a".to_owned(), STARTED_AT)
        .unwrap();
    let guard_b = control
        .begin_run(ROOT_B, CHANGE, "run-b".to_owned(), STARTED_AT)
        .unwrap();

    let mut rx_a = control.subscribe(ROOT, CHANGE).unwrap();
    let mut rx_b = control.subscribe(ROOT_B, CHANGE).unwrap();

    // 各自 publish：载荷可辨（同一 change 名，仅 root 不同）
    guard_a.emit(RunUpdate::ConfirmWait {
        phase: "proposal".to_owned(),
    });
    guard_b.emit(RunUpdate::Ask {
        question: "root-b 的提问".to_owned(),
        options: vec!["甲".to_owned()],
    });

    // rootA 订阅者：仅收到 (rootA, x) 的信封
    let got_a = rx_a.recv().await.unwrap();
    assert!(
        matches!(got_a, RunNotice::ConfirmWait),
        "A 的状态流 kind-only 通知不串台（信封降位，AC-8）"
    );
    // rootB 信封未串台（下一帧为空）
    assert!(
        rx_a.try_recv().is_err(),
        "rootA 订阅者收不到 (rootB, x) 的任何信封"
    );
    // rootB 订阅者：收到自己的 Ask
    let got_b = rx_b.recv().await.unwrap();
    assert!(
        matches!(got_b, RunNotice::Ask),
        "rootB 订阅者收到 (rootB, x) 自己的 kind 通知"
    );
    drop((guard_a, guard_b));
}

/// RunGuard 复合键除名：(rootA, x) guard finish → 仅该键除名（snapshot
/// None）；(rootB, x) 同名键存活（snapshot 仍在）——guard 不误伤他键。
#[tokio::test]
async fn runguard复合键除名_仅本键除名他键存活() {
    let control = Arc::new(ChangeFlowControl::new());
    const ROOT_B: &str = "/ws/root-b";

    let guard_a = control
        .begin_run(ROOT, CHANGE, "run-a".to_owned(), STARTED_AT)
        .unwrap();
    let guard_b = control
        .begin_run(ROOT_B, CHANGE, "run-b".to_owned(), STARTED_AT)
        .unwrap();

    guard_a.finish(ChangeRunStatus::Completed, None);

    assert!(
        control.snapshot(ROOT, CHANGE).is_none(),
        "仅 (rootA, x) 键除名"
    );
    assert_eq!(
        control.snapshot(ROOT_B, CHANGE).map(|snap| snap.run_id),
        Some("run-b".to_owned()),
        "(rootB, x) 同名键存活（guard 不误伤他键）"
    );
    // 他键停止面照常可用
    assert!(control.request_stop(ROOT_B, CHANGE), "他键停止面不受扰");
    drop(guard_b);
}

/// DTO 零改动：`RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` 线面
/// JSON 形状键集无 root 渗出（复合键是注册表内部寻址——IPC 面零改动）。
#[tokio::test]
async fn dto线面零root渗出_复合键不出线() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });

    let snapshot = control.snapshot(ROOT, CHANGE).expect("快照应在场");
    let wire = serde_json::to_value(&snapshot).expect("快照序列化应成功");
    for key in wire.as_object().expect("快照为对象").keys() {
        assert!(
            !key.to_lowercase().contains("root") && !key.to_lowercase().contains("workspace"),
            "快照键集无 root / workspace 渗出（复合键不出线），实际键: {key}"
        );
    }
    assert_eq!(
        wire.get("runId"),
        Some(&serde_json::Value::String("run-1".to_owned())),
        "快照形状不变（runId 在场）"
    );

    let update = serde_json::to_value(RunUpdate::Finished {
        status: ChangeRunStatus::Stopped,
        reason: None,
    })
    .expect("信封序列化应成功");
    assert!(
        !update.to_string().to_lowercase().contains("root"),
        "RunUpdate 信封零 root 渗出，实际: {update}"
    );
}

// ---------------------------------------------------------------------------
// 步累积器与通知降位（unify-run-state-persistence）：快照面与广播面同拍
// ---------------------------------------------------------------------------

/// publish(Step) → snapshot().steps 按到达序追加（phase / attempt 随行推进）
/// + 订阅端收到 RunNotice::Step（快照面与广播面同拍，AC-8/AC-10）。
#[tokio::test]
async fn publish_step快照面按到达序追加_广播run_notice_step() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE).expect("run 在场可订阅");

    let first = ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 1,
        step: ChangeStepKind::Executor,
        status: ChangeStepStatus::Running,
        session_id: None,
        detail: None,
    };
    guard.emit(RunUpdate::Step {
        step: first.clone(),
    });
    let second = ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 1,
        step: ChangeStepKind::StaticCheck,
        status: ChangeStepStatus::Passed,
        session_id: None,
        detail: Some("静态检查通过".to_owned()),
    };
    guard.emit(RunUpdate::Step {
        step: second.clone(),
    });

    // 快照面：按到达序追加、phase/attempt 随行推进
    let snap = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(
        snap.steps,
        vec![first, second],
        "步累积器按 emit 到达序追加（全词汇照收——过滤归 run_history 单点）"
    );
    assert_eq!(snap.phase.as_deref(), Some("implement"));
    assert_eq!(snap.attempt, Some(1));

    // 广播面：RunNotice::Step kind-only（同拍——发布单点 publish 一并发出）
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "步更新广播 kind-only Notice（载荷剥离）"
    );
}

/// publish(SessionEvent) → 快照面零变化（steps / status / phase 不动）+ 广播
/// RunNotice::SessionEvent（转录重查分流键，D8）。
#[tokio::test]
async fn publish_session_event快照面零变化_广播notice() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE).unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let before = control.snapshot(ROOT, CHANGE).unwrap();

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

    let after = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(
        after, before,
        "会话事件零快照面突变（steps / status / phase 不动）"
    );
    // 前置 Step 的通知先排空（同拍流内序），再验 SessionEvent kind
    assert!(matches!(rx.try_recv(), Ok(RunNotice::Step)));
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::SessionEvent)),
        "会话事件广播 SessionEvent kind（转录重查分流键）"
    );
}

/// publish(Ask) / publish(ConfirmWait) / publish(Finished) → 状态迁移与 ask
/// 载荷照旧 + 对应 Notice kind 各自广播（AC-8 kind 位一一对应）。
#[tokio::test]
async fn publish停等与终态_状态迁移照旧且notice对应() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE).unwrap();

    guard.emit(RunUpdate::Ask {
        question: "回溯到哪?".to_owned(),
        options: vec!["proposal".to_owned()],
    });
    let snap = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingAsk, "ask 状态迁移照旧");
    let ask = snap.ask.expect("ask 载荷照旧");
    assert_eq!(ask.question, "回溯到哪?");
    assert!(matches!(rx.try_recv(), Ok(RunNotice::Ask)));

    guard.emit(RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
    });
    let snap = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingConfirm);
    assert_eq!(snap.phase.as_deref(), Some("test-gen"), "停等定位随行");
    assert!(matches!(rx.try_recv(), Ok(RunNotice::ConfirmWait)));

    guard.finish(ChangeRunStatus::Completed, None);
    assert!(
        control.snapshot(ROOT, CHANGE).is_none(),
        "终态除名（收口后注册表无条目）"
    );
    assert!(matches!(rx.try_recv(), Ok(RunNotice::Finished)));
}

/// 终态除名后 publish 零累积零广播（既有语义回归——run_finish 后注册表无
/// 条目，迟滞 publish 无落点）。
#[tokio::test]
async fn 终态除名后publish零累积零广播() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.finish(ChangeRunStatus::Stopped, None);

    control.publish(
        ROOT,
        CHANGE,
        RunUpdate::Step {
            step: sample_step(),
        },
    );
    control.publish(
        ROOT,
        CHANGE,
        RunUpdate::Ask {
            question: "q".to_owned(),
            options: Vec::new(),
        },
    );

    assert!(
        control.snapshot(ROOT, CHANGE).is_none(),
        "除名后快照面零复活"
    );
    match control.subscribe(ROOT, CHANGE) {
        // 除名后 subscribe 落 None（无运行 run）——通知面无从订阅即零广播
        None => {}
        Some(mut rx) => {
            assert!(rx.try_recv().is_err(), "迟滞 publish 零广播（无条目可发）");
        }
    }
}

// ---------------------------------------------------------------------------
// begin_run 扩参与快照独立性（started_at 入档 / 克隆语义 / guard 读面）
// ---------------------------------------------------------------------------

/// begin_run 携 started_at → snapshot().started_at 等值入档（与 RunRequest
/// 同值锚，D5——统一视图活面 startedAt 投影源）。
#[tokio::test]
async fn begin_run_started_at等值入档快照() {
    let control = Arc::new(ChangeFlowControl::new());
    let started_at = 1_726_000_123_456_i64;
    let _guard = control
        .begin_run(ROOT, CHANGE, "run-42".to_owned(), started_at)
        .unwrap();

    let snap = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(snap.started_at, started_at, "发起时刻原值入档");
    assert_eq!(snap.run_id, "run-42");
    assert_eq!(snap.status, ChangeRunStatus::Running, "running 起步");
}

/// 同 (root, change) 二次 begin → Err 并行冲突且首个 run 快照面不受扰动
///（复合键冲突检测——首个 run 的 steps / started_at 原值保持）。
#[tokio::test]
async fn begin_run二次并行err_首个run快照面不受扰动() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });

    let err = control
        .begin_run(ROOT, CHANGE, "run-2".to_owned(), STARTED_AT + 1)
        .err()
        .expect("同复合键二次 begin 应 Err");
    assert!(
        err.contains(CHANGE) && err.contains("并行"),
        "并行冲突记因，实际: {err}"
    );

    let snap = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(snap.run_id, "run-1", "首个 run 条目未被扰动");
    assert_eq!(snap.started_at, STARTED_AT);
    assert_eq!(snap.steps.len(), 1, "首个 run 步累积器原值保持");
}

/// 快照独立性：snapshot() 返回 steps 为克隆——取快照后继续 emit，已取快照
/// 不变、新快照含新步（AC-10 重挂读一致性）。
#[tokio::test]
async fn snapshot_steps克隆语义_取后emit互不扰动() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let taken = control.snapshot(ROOT, CHANGE).unwrap();

    let late = ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 2,
        step: ChangeStepKind::Evaluator,
        status: ChangeStepStatus::Passed,
        session_id: Some("sess-2".to_owned()),
        detail: None,
    };
    guard.emit(RunUpdate::Step { step: late.clone() });

    assert_eq!(
        taken.steps.len(),
        1,
        "已取快照不被后续 emit 扰动（克隆语义）"
    );
    let fresh = control.snapshot(ROOT, CHANGE).unwrap();
    assert_eq!(fresh.steps.len(), 2, "新快照含新步");
    assert_eq!(fresh.steps[1], late);
}

/// RunGuard.steps() 与 control.snapshot().steps 等值（walker 第二写取累积器
/// 的读面，AC-2 组装输入一致性）。
#[tokio::test]
async fn run_guard_steps读面与快照等值() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    assert!(
        guard.steps().is_empty(),
        "零 emit 步累积器空（空整包合法形态）"
    );

    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let late = ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 2,
        step: ChangeStepKind::StaticCheck,
        status: ChangeStepStatus::Failed,
        session_id: None,
        detail: Some("诊断".to_owned()),
    };
    guard.emit(RunUpdate::Step { step: late });

    assert_eq!(
        guard.steps(),
        control.snapshot(ROOT, CHANGE).unwrap().steps,
        "guard 读面与快照面等值（第二写组装输入一致性）"
    );
}
