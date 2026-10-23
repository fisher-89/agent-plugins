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

/// 固定 change **id** 字面量（注册表键第二段 = change id——name 非键）。
const CHANGE_ID: &str = "1a2b3c4d-0001-4a2b-8c3d-4e5f60718293";

/// 同名 change 的第二个 **id**（同 root 同名异 id 并行语义锚——name 非键）。
const CHANGE_ID_B: &str = "1a2b3c4d-0002-4a2b-8c3d-4e5f60718293";

/// 两条 change 共有的展示名（注册表零 name 触点——注释引语用，非寻址键）。
const NAME: &str = "demo-change";

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
        instance_no: 0,
        status: ChangeStepStatus::Running,
        session_id: Some("sess-1".to_owned()),
        detail: None,
        output_ref: None,
        started_at: STARTED_AT,
        ended_at: None,
        token_input: None,
        token_output: None,
    }
}

/// 运行中步状态行 fixture（指定步词汇 / 实例号 / 会话槽——会话 id 提前挂载
/// 用例输入面）。
fn running_step(
    kind: ChangeStepKind,
    instance_no: u32,
    session_id: Option<&str>,
) -> ChangeStepState {
    ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 2,
        step: kind,
        instance_no,
        status: ChangeStepStatus::Running,
        session_id: session_id.map(str::to_owned),
        detail: None,
        output_ref: None,
        started_at: STARTED_AT,
        ended_at: None,
        token_input: None,
        token_output: None,
    }
}

/// 会话事件载荷 fixture（seq 区分拍次；载荷内容不参与 attach 判定）。
fn session_event(session_id: &str, seq: u64) -> RunUpdate {
    RunUpdate::SessionEvent {
        session_id: session_id.to_owned(),
        event: agent::AgentEvent {
            seq,
            timestamp_ms: 1_726_000_000_000,
            kind: agent::AgentEventKind::Raw {
                event_type: "system".to_owned(),
                raw_json: "{}".to_owned(),
            },
        },
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

/// 复合键 (root, id) 登记与并行冲突：首次 begin_run(root, id) 成功返回
/// RunGuard；同 root 同 id 二次 begin → Err（并行冲突检测——change_flow_start
/// 前置校验第三分支的输入面）；冲突被拒后首个 run 条目不受扰动；RunGuard
/// 终态除名后同键可重入（键构造点逐处置换为 id）。
#[tokio::test]
async fn 复合键登记与并行冲突_同root同id二次begin拒绝() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("新 change 首次 begin_run 应成功");
    assert!(!guard.cancelled(), "新登记 run 未置取消信号");

    assert_eq!(
        control.snapshot(ROOT, CHANGE_ID).map(|snap| snap.run_id),
        Some("run-1".to_owned()),
        "登记即快照可见"
    );

    // 首个 run 已累积一步：冲突拒绝零扰动面（run_id / started_at / 步累积器）
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });

    let err = control
        .begin_run(ROOT, CHANGE_ID, "run-2".to_owned(), STARTED_AT + 1)
        .err()
        .expect("同 root 同 id 二次 begin 应 Err");
    assert!(err.contains(CHANGE_ID), "错误显式携带 change id: {err}");
    assert!(
        err.contains("已有运行中的 run") && err.contains("并行"),
        "并行冲突记因: {err}"
    );

    let snap = control
        .snapshot(ROOT, CHANGE_ID)
        .expect("首个 run 条目在场");
    assert_eq!(snap.run_id, "run-1", "首个 run 条目未被扰动");
    assert_eq!(snap.started_at, STARTED_AT);
    assert_eq!(snap.nodes.len(), 1, "首个 run 节点累积器原值保持");
    assert!(snap.edges.is_empty(), "零边 run 边累积器空");

    // 终态除名后重入可达（键随 RunGuard 除名释放）
    guard.finish(ChangeRunStatus::Stopped, Some("用户停止".to_owned()));
    assert!(control.snapshot(ROOT, CHANGE_ID).is_none(), "终态除名");
    control
        .begin_run(ROOT, CHANGE_ID, "run-3".to_owned(), STARTED_AT + 2)
        .expect("终态除名后同键应可重新登记");
}

/// 跨 change 并行隔离：两个 change 各自 begin_run 并行互不干扰（键 =
/// 复合 (root, change id)，D4）。
#[tokio::test]
async fn 跨change并行隔离_订阅与停止寻址按id不串台() {
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
        archive: None,
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
async fn request_stop置位与miss幂等() {
    let control = Arc::new(ChangeFlowControl::new());

    // miss 半边：无运行 run 幂等返回 false
    assert!(
        !control.request_stop(ROOT, CHANGE_ID),
        "无运行 run 应返回 false"
    );

    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    assert!(!guard.cancelled(), "未停止前取消信号为假");

    assert!(control.request_stop(ROOT, CHANGE_ID), "运行中置位返回 true");
    assert!(guard.cancelled(), "取消信号同步观测（watch 置位）");
}

/// subscribe 有无运行：有 run → Some(receiver)；无 run → None
/// （change_flow_watch 消费形态）。
#[tokio::test]
async fn subscribe有run为some无run为none() {
    let control = Arc::new(ChangeFlowControl::new());
    assert!(
        control.subscribe(ROOT, CHANGE_ID).is_none(),
        "无 run → None"
    );

    let _guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    assert!(
        control.subscribe(ROOT, CHANGE_ID).is_some(),
        "有 run → Some(receiver)"
    );
}

/// emit → broadcast 流出：`guard.emit(update)` 经 subscribe receiver 收到
/// 同值信封（RunUpdate 流出口——D9 broadcast 半边）。
#[tokio::test]
async fn emit经broadcast流出kind通知() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();

    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
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
async fn 会话槽置写与清槽往返() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();

    assert_eq!(control.current_session(ROOT, CHANGE_ID), None, "初值空槽");

    guard.set_session(Some("sess-1".to_owned()));
    assert_eq!(
        control.current_session(ROOT, CHANGE_ID),
        Some("sess-1".to_owned())
    );

    guard.set_session(None);
    assert_eq!(
        control.current_session(ROOT, CHANGE_ID),
        None,
        "置 None 清槽"
    );

    // control 侧直写同槽（命令层 sink 桥在首个会话事件到达时同步）
    control.set_session(ROOT, CHANGE_ID, Some("sess-2".to_owned()));
    assert_eq!(
        control.current_session(ROOT, CHANGE_ID),
        Some("sess-2".to_owned())
    );
}

/// confirm 应答回传：`wait_confirm` 挂起后 confirm(proceed=true/false) 回传
/// 布尔（false 的收敛归 walker）；单次通道 take 语义——重复应答落空 Err。
#[tokio::test]
async fn confirm应答回传挂起等待者_单次take() {
    // proceed=true 臂
    let control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    let waiter_guard = Arc::clone(&guard);
    let waiter = tokio::spawn(async move { waiter_guard.wait_confirm().await });

    assert!(
        until_ok(|| control.confirm(ROOT, CHANGE_ID, true, false)).await,
        "confirm 的 Ok 即 pending 通道登记证明"
    );
    assert_eq!(
        waiter.await.unwrap(),
        (true, false),
        "proceed=true 回传 true（sync_specs=false 原样回流）"
    );

    // take 语义：应答已消费，重复应答落空 Err
    let err = control
        .confirm(ROOT, CHANGE_ID, true, false)
        .expect_err("重复应答应 Err");
    assert!(
        err.contains("无等待中的 phase 确认"),
        "单次通道已取走: {err}"
    );

    // proceed=false 臂：独立 run，false 原样回传（收敛归 walker）
    let control_b = Arc::new(ChangeFlowControl::new());
    let guard_b = Arc::new(
        control_b
            .begin_run(ROOT, CHANGE_ID, "run-2".to_owned(), STARTED_AT)
            .unwrap(),
    );
    let waiter_b_guard = Arc::clone(&guard_b);
    let waiter_b = tokio::spawn(async move { waiter_b_guard.wait_confirm().await });

    assert!(until_ok(|| control_b.confirm(ROOT, CHANGE_ID, false, false)).await);
    assert_eq!(
        waiter_b.await.unwrap(),
        (false, false),
        "proceed=false 回传 false"
    );
}

/// confirm 无等待方：无运行 run → Err 携 change id；有 run 无挂起 → Err
/// 「无等待中的 phase 确认」（单次通道语义，change_flow_confirm 错误面输入）。
#[tokio::test]
async fn confirm无等待方err携id记因() {
    let control = Arc::new(ChangeFlowControl::new());

    // 无运行 run
    let err = control
        .confirm(ROOT, CHANGE_ID, true, false)
        .expect_err("无 run 应 Err");
    assert!(err.contains(CHANGE_ID), "错误显式携带 change id: {err}");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");

    // 有 run 无等待方
    let _guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let err = control
        .confirm(ROOT, CHANGE_ID, true, false)
        .expect_err("无挂起应 Err");
    assert_eq!(err, "当前无等待中的 phase 确认");
}

/// answer 应答回传：`wait_answer` 挂起后 answer(text) 回传原文（ask 应答回
/// 流）；单次通道 take 语义——重复应答落空 Err。
#[tokio::test]
async fn answer应答原文回流挂起等待者_单次take() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    let waiter_guard = Arc::clone(&guard);
    let waiter = tokio::spawn(async move { waiter_guard.wait_answer().await });

    assert!(
        until_ok(|| control.answer(ROOT, CHANGE_ID, "采用方案 B".to_owned())).await,
        "answer 的 Ok 即 pending 通道登记证明"
    );
    assert_eq!(
        waiter.await.unwrap().as_deref(),
        Some("采用方案 B"),
        "应答原文回流 walker"
    );

    // take 语义：应答已消费，重复应答落空 Err
    let err = control
        .answer(ROOT, CHANGE_ID, "再来一次".to_owned())
        .expect_err("重复应答应 Err");
    assert!(err.contains("无等待中的 ask"), "单次通道已取走: {err}");
}

/// answer 无等待方：无运行 run → Err 携 change id；有 run 无挂起 → Err
/// 「无等待中的 ask」。
#[tokio::test]
async fn answer无等待方err携id记因() {
    let control = Arc::new(ChangeFlowControl::new());

    // 无运行 run
    let err = control
        .answer(ROOT, CHANGE_ID, "应答".to_owned())
        .expect_err("无 run 应 Err");
    assert!(err.contains(CHANGE_ID), "错误显式携带 change id: {err}");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");

    // 有 run 无等待方
    let _guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let err = control
        .answer(ROOT, CHANGE_ID, "应答".to_owned())
        .expect_err("无挂起应 Err");
    assert_eq!(err, "当前无等待中的 ask");
}

/// finish 终态除名：finish(status) 后 snapshot → None、request_stop →
/// false、subscribe → None（幂等收口，AC-7「run 仅进程内」半边）；Finished
/// 信封先于除名到达订阅者（重入可达断言归复合键登记行）。
#[tokio::test]
async fn 终态除名三面与finished信封() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();

    guard.finish(ChangeRunStatus::Stopped, Some("用户停止".to_owned()));

    // Finished 信封流出（终态收口唯一出口）
    let finished = rx.recv().await.unwrap();
    assert!(
        matches!(finished, RunNotice::Finished),
        "Finished kind 通知流出（终态收口唯一出口，信封降位）"
    );

    // 终态除名三面
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "快照面除名 → None"
    );
    assert!(
        !control.request_stop(ROOT, CHANGE_ID),
        "除名后 request_stop 幂等返回 false"
    );
    assert!(
        control.subscribe(ROOT, CHANGE_ID).is_none(),
        "订阅面除名 → None"
    );
}

/// finish 幂等收口：snapshot 在 finish 前返回 Some（状态机镜像），finish
/// 消费 self（重复 finish 类型层不可表达——终态出口唯一）；除名后无残留
/// 可改写面（迟滞 publish 落空，首个终态不被改写）。
#[tokio::test]
async fn finish单次收口_除名后无残留可改写面() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });

    // finish 前：快照 Some（状态机镜像运行态）
    let before = control
        .snapshot(ROOT, CHANGE_ID)
        .expect("finish 前快照应 Some");
    assert_eq!(before.status, ChangeRunStatus::Running);
    assert_eq!(before.run_id, "run-1");

    guard.finish(ChangeRunStatus::Stopped, None);

    // 除名后迟滞 publish 落空：无残留可改写面（首个终态不被改写）
    control.publish(
        ROOT,
        CHANGE_ID,
        RunUpdate::Finished {
            status: ChangeRunStatus::Failed,
            reason: Some("漂移".to_owned()),
        },
    );
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "除名后终态不可被改写（快照面无残留）"
    );
}

/// publish 快照面镜像：Step 推进 phase/attempt、ConfirmWait / Ask 迁移状态
/// 并落停等载荷、SessionEvent 零改写（发布单点 `publish` 的快照同步面）。
#[tokio::test]
async fn publish快照面状态机镜像() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();

    // Step：phase / attempt 推进，状态保持 running
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::Running);
    assert_eq!(snap.phase.as_deref(), Some("implement"));
    assert_eq!(snap.attempt, Some(2));
    assert!(snap.ask.is_none());

    // ConfirmWait：waitingConfirm + 停等定位（普通相位停等 archive 槽清空）
    guard.emit(RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
        archive: None,
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingConfirm);
    assert_eq!(snap.phase.as_deref(), Some("test-gen"));
    assert!(
        snap.archive.is_none(),
        "普通相位停等 archive 槽恒 None（停等载荷面）"
    );

    // ConfirmWait（archive 前停等）：前置只读载荷落 entry，快照出线
    guard.emit(RunUpdate::ConfirmWait {
        phase: "archive".to_owned(),
        archive: Some(sample_preflight()),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingConfirm);
    assert_eq!(snap.phase.as_deref(), Some("archive"));
    let preflight = snap.archive.clone().expect("归档前停等应携前置载荷");
    assert_eq!(preflight, sample_preflight());

    // 回退到普通相位停等：archive 槽清空（不残留上一次载荷）
    guard.emit(RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
        archive: None,
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert!(
        snap.archive.is_none(),
        "普通相位停等清空 archive 槽（不残留）"
    );

    // Ask：waitingAsk + 中断载荷
    guard.emit(RunUpdate::Ask {
        question: "越权 backtrack 是否放行?".to_owned(),
        options: vec!["拒绝".to_owned(), "放行".to_owned()],
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
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
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        snap.status,
        ChangeRunStatus::WaitingAsk,
        "会话事件不改写状态"
    );
    assert_eq!(snap.phase.as_deref(), Some("test-gen"));
    assert!(snap.archive.is_none(), "会话事件不改写归档载荷槽");
}

/// 取消信号中断挂起停等：wait_confirm / wait_answer 挂起期间 request_stop
/// 置位即收敛（proceed=false / None，停止不必先应答——guard 侧 select 半边）。
#[tokio::test]
async fn cancel置位中断挂起停等() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );

    let confirm_guard = Arc::clone(&guard);
    let confirm_waiter = tokio::spawn(async move { confirm_guard.wait_confirm().await });
    let answer_guard = Arc::clone(&guard);
    let answer_waiter = tokio::spawn(async move { answer_guard.wait_answer().await });

    // 让 waiter 起跑挂起（取消语义与登记次序无关：先置位亦即刻收敛）
    tokio::task::yield_now().await;

    assert!(control.request_stop(ROOT, CHANGE_ID), "停止置位");
    assert_eq!(
        confirm_waiter.await.unwrap(),
        (false, false),
        "取消置位 → (false, false) 收敛（停止不必先应答）"
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

/// 同名异 id 可并行（name 非键语义锚）：同 root 下两条同名 change（异 id）
/// 各自 begin → 均受理互不误拒；异 workspace 同名同 id 亦不误拒（先例 bug
/// 修正锚——复合键 root 段）；同 root 两不同 id change 并行互不干扰。
#[tokio::test]
async fn 同名异id并行_异workspace同名不误拒() {
    let control = Arc::new(ChangeFlowControl::new());
    const ROOT_B: &str = "/ws/root-b";
    assert_ne!(CHANGE_ID, CHANGE_ID_B, "同名（{NAME}）异 id 形态");

    // (rootA, idA) 登记
    let guard_a = control
        .begin_run(ROOT, CHANGE_ID, "run-a".to_owned(), STARTED_AT)
        .expect("(rootA, idA) 首登应成功");
    // 同 root 同名（异 id）change 并行 → 均受理（name 不参与键）
    let guard_same_name = control
        .begin_run(ROOT, CHANGE_ID_B, "run-same-name".to_owned(), STARTED_AT)
        .expect("同 root 同名异 id change 并行不误拒（name 非键）");
    assert!(
        !guard_same_name.cancelled(),
        "同名异 id 的 run 未受 idA 置位影响"
    );
    // (rootB, idA) 同名同 id 异 workspace → 不误拒（复合键 root 段生效）
    let guard_b = control
        .begin_run(ROOT_B, CHANGE_ID, "run-b".to_owned(), STARTED_AT)
        .expect("异 workspace 同 change id 不误拒（复合键 root 段生效）");
    assert!(
        !guard_b.cancelled(),
        "rootB 的 run 未受 rootA 同名 run 置位影响"
    );
    // 同 workspace 两 change 并行：互不冲突
    let guard_c = control
        .begin_run(ROOT, "other-id", "run-c".to_owned(), STARTED_AT)
        .expect("同 workspace 两 change 并行（真并行解锁）");

    // 四键各自快照可达（同名的两条按 id 分槽）
    assert_eq!(
        control.snapshot(ROOT, CHANGE_ID).map(|snap| snap.run_id),
        Some("run-a".to_owned())
    );
    assert_eq!(
        control.snapshot(ROOT, CHANGE_ID_B).map(|snap| snap.run_id),
        Some("run-same-name".to_owned()),
        "同一 root 下同名异 id 各自成槽"
    );
    assert_eq!(
        control.snapshot(ROOT_B, CHANGE_ID).map(|snap| snap.run_id),
        Some("run-b".to_owned())
    );
    assert_eq!(
        control.snapshot(ROOT, "other-id").map(|snap| snap.run_id),
        Some("run-c".to_owned())
    );
    drop((guard_a, guard_same_name, guard_b, guard_c));
}

/// 订阅隔离：(rootA, x) 与 (rootB, x) 各自 run 运行中——rootA 订阅者收到
/// (rootA, x) 的 publish 而**收不到** (rootB, x) 的任何信封（broadcast 按
/// 复合键寻址不串台）。
#[tokio::test]
async fn 复合键订阅隔离_broadcast按复合键寻址不串台() {
    let control = Arc::new(ChangeFlowControl::new());
    const ROOT_B: &str = "/ws/root-b";

    let guard_a = control
        .begin_run(ROOT, CHANGE_ID, "run-a".to_owned(), STARTED_AT)
        .unwrap();
    let guard_b = control
        .begin_run(ROOT_B, CHANGE_ID, "run-b".to_owned(), STARTED_AT)
        .unwrap();

    let mut rx_a = control.subscribe(ROOT, CHANGE_ID).unwrap();
    let mut rx_b = control.subscribe(ROOT_B, CHANGE_ID).unwrap();

    // 各自 publish：载荷可辨（同一 change id，仅 root 不同）
    guard_a.emit(RunUpdate::ConfirmWait {
        phase: "proposal".to_owned(),
        archive: None,
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
        .begin_run(ROOT, CHANGE_ID, "run-a".to_owned(), STARTED_AT)
        .unwrap();
    let guard_b = control
        .begin_run(ROOT_B, CHANGE_ID, "run-b".to_owned(), STARTED_AT)
        .unwrap();

    guard_a.finish(ChangeRunStatus::Completed, None);

    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "仅 (rootA, x) 键除名"
    );
    assert_eq!(
        control.snapshot(ROOT_B, CHANGE_ID).map(|snap| snap.run_id),
        Some("run-b".to_owned()),
        "(rootB, x) 同名键存活（guard 不误伤他键）"
    );
    // 他键停止面照常可用
    assert!(control.request_stop(ROOT_B, CHANGE_ID), "他键停止面不受扰");
    drop(guard_b);
}

/// DTO 零改动：`RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` 线面
/// JSON 形状键集无 root 渗出（复合键是注册表内部寻址——IPC 面零改动）。
#[tokio::test]
async fn dto线面零root渗出_复合键不出线() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("快照应在场");
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
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).expect("run 在场可订阅");

    let first = ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 1,
        step: ChangeStepKind::Executor,
        instance_no: 0,
        status: ChangeStepStatus::Running,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at: STARTED_AT,
        ended_at: None,
        token_input: None,
        token_output: None,
    };
    guard.emit(RunUpdate::Step {
        step: first.clone(),
    });
    let second = ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 1,
        step: ChangeStepKind::StaticCheck,
        instance_no: 0,
        status: ChangeStepStatus::Passed,
        session_id: None,
        detail: Some("静态检查通过".to_owned()),
        output_ref: None,
        started_at: STARTED_AT,
        ended_at: Some(STARTED_AT + 2_000),
        token_input: None,
        token_output: None,
    };
    guard.emit(RunUpdate::Step {
        step: second.clone(),
    });

    // 快照面：按到达序追加、phase/attempt 随行推进
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        snap.nodes,
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

/// publish(SessionEvent) → 已挂载节点的会话事件零快照面突变（steps / status /
/// phase 不动）+ 广播 RunNotice::SessionEvent（转录重查分流键，D8）。
#[tokio::test]
async fn publish_session_event已挂载节点零快照面突变_广播notice() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let before = control.snapshot(ROOT, CHANGE_ID).unwrap();

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

    let after = control.snapshot(ROOT, CHANGE_ID).unwrap();
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

/// publish(SessionEvent) 首拍：把 session id 提前挂到运行中的会话型步节点，
/// 并补发一拍 Step 通知（统一视图重查触发——运行中节点由此按 id 直查实时
/// 转录）；后续事件不重复挂载 / 不重复通知。
#[tokio::test]
async fn publish_session_event首拍提前挂载会话id_补发step通知() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    guard.emit(RunUpdate::Step {
        step: running_step(ChangeStepKind::Executor, 0, None),
    });
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "running emit 通知先排空"
    );

    guard.emit(session_event("sess-live", 0));

    let attached = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        attached.nodes[0].session_id.as_deref(),
        Some("sess-live"),
        "首拍把会话 id 提前挂到运行中节点"
    );
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "首挂补发 Step 通知（前端据此重查统一视图）"
    );
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::SessionEvent)),
        "原 SessionEvent kind 照常广播"
    );

    guard.emit(session_event("sess-live", 1));
    assert_eq!(
        control.snapshot(ROOT, CHANGE_ID).unwrap().nodes,
        attached.nodes,
        "后续会话事件不再改快照面"
    );
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::SessionEvent)),
        "后续会话事件仅 SessionEvent kind"
    );
    assert!(rx.try_recv().is_err(), "后续会话事件零 Step 补发");
}

/// publish(SessionEvent) 边界：无运行中的会话型步节点（工具步在途 / 会话步
/// 已收口）→ 零挂载、零 Step 补发，仅原 kind 广播。
#[tokio::test]
async fn publish_session_event无运行中会话节点_零挂载仅session通知() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    guard.emit(RunUpdate::Step {
        step: running_step(ChangeStepKind::StaticCheck, 0, None),
    });
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "工具步 running emit 通知先排空"
    );

    guard.emit(session_event("sess-live", 0));

    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.nodes[0].session_id, None, "工具步节点不挂会话 id");
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::SessionEvent)),
        "仅原 SessionEvent kind"
    );
    assert!(rx.try_recv().is_err(), "零 Step 补发");
}

/// publish(Ask) / publish(ConfirmWait) / publish(Finished) → 状态迁移与 ask
/// 载荷照旧 + 对应 Notice kind 各自广播（AC-8 kind 位一一对应）。
#[tokio::test]
async fn publish停等与终态_状态迁移照旧且notice对应() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();

    guard.emit(RunUpdate::Ask {
        question: "回溯到哪?".to_owned(),
        options: vec!["proposal".to_owned()],
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingAsk, "ask 状态迁移照旧");
    let ask = snap.ask.expect("ask 载荷照旧");
    assert_eq!(ask.question, "回溯到哪?");
    assert!(matches!(rx.try_recv(), Ok(RunNotice::Ask)));

    guard.emit(RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
        archive: None,
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingConfirm);
    assert_eq!(snap.phase.as_deref(), Some("test-gen"), "停等定位随行");
    assert!(matches!(rx.try_recv(), Ok(RunNotice::ConfirmWait)));

    guard.finish(ChangeRunStatus::Completed, None);
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
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
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.finish(ChangeRunStatus::Stopped, None);

    control.publish(
        ROOT,
        CHANGE_ID,
        RunUpdate::Step {
            step: sample_step(),
        },
    );
    control.publish(
        ROOT,
        CHANGE_ID,
        RunUpdate::Ask {
            question: "q".to_owned(),
            options: Vec::new(),
        },
    );

    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "除名后快照面零复活"
    );
    match control.subscribe(ROOT, CHANGE_ID) {
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
        .begin_run(ROOT, CHANGE_ID, "run-42".to_owned(), started_at)
        .unwrap();

    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.started_at, started_at, "发起时刻原值入档");
    assert_eq!(snap.run_id, "run-42");
    assert_eq!(snap.status, ChangeRunStatus::Running, "running 起步");
}

/// 快照独立性：snapshot() 返回 steps 为克隆——取快照后继续 emit，已取快照
/// 不变、新快照含新步（AC-10 重挂读一致性）。
#[tokio::test]
async fn snapshot_steps克隆语义_取后emit互不扰动() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let taken = control.snapshot(ROOT, CHANGE_ID).unwrap();

    let late = ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 2,
        step: ChangeStepKind::Evaluator,
        instance_no: 0,
        status: ChangeStepStatus::Passed,
        session_id: Some("sess-2".to_owned()),
        detail: None,
        output_ref: None,
        started_at: STARTED_AT,
        ended_at: Some(STARTED_AT + 1_000),
        token_input: None,
        token_output: None,
    };
    guard.emit(RunUpdate::Step { step: late.clone() });

    assert_eq!(
        taken.nodes.len(),
        1,
        "已取快照不被后续 emit 扰动（克隆语义）"
    );
    let fresh = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(fresh.nodes.len(), 2, "新快照含新节点");
    assert_eq!(fresh.nodes[1], late);
}

/// RunGuard.nodes() / edges() 与 control.snapshot().nodes / edges 等值
///（walker 第二写取累积器的读面，AC-2/AC-5 组装输入一致性）。
#[tokio::test]
async fn run_guard读面与快照等值() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    assert!(
        guard.nodes().is_empty() && guard.edges().is_empty(),
        "零 emit 节点 / 边累积器空（空整包合法形态）"
    );

    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let late = ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 2,
        step: ChangeStepKind::StaticCheck,
        instance_no: 0,
        status: ChangeStepStatus::Failed,
        session_id: None,
        detail: Some("诊断".to_owned()),
        output_ref: None,
        started_at: STARTED_AT,
        ended_at: Some(STARTED_AT + 1_000),
        token_input: None,
        token_output: None,
    };
    guard.emit(RunUpdate::Step { step: late });

    assert_eq!(
        guard.nodes(),
        control.snapshot(ROOT, CHANGE_ID).unwrap().nodes,
        "guard 节点读面与快照面等值（第二写组装输入一致性）"
    );
    assert_eq!(
        guard.edges(),
        control.snapshot(ROOT, CHANGE_ID).unwrap().edges,
        "guard 边读面与快照面等值"
    );
}

// ---------------------------------------------------------------------------
// 执行图边累积（RunUpdate::Edge，四 kind 显式落边）
// ---------------------------------------------------------------------------

/// `publish(RunUpdate::Edge)` 边累积：四 kind 边按 emit 序追加、快照 `edges`
/// 随行；节点侧 `instance_no` 递增（同 `(phase, attempt, step)` 多实例）且与边
/// 累积互不串扰（AC-3 / AC-4）。
#[tokio::test]
async fn publish边累积四kind且节点实例号递增() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();

    let key = |instance_no: u32| {
        (
            "implement".to_owned(),
            1,
            ChangeStepKind::Executor,
            instance_no,
        )
    };
    let edges = [
        crate::state::EdgeKind::Forward,
        crate::state::EdgeKind::Feedback,
        crate::state::EdgeKind::Retry,
        crate::state::EdgeKind::Backtrack,
    ];
    for (index, kind) in edges.iter().enumerate() {
        guard.emit(RunUpdate::Edge {
            edge: crate::state::ChangeStepEdge {
                from: key(index as u32),
                to: ("implement".to_owned(), 1, ChangeStepKind::StaticCheck, 0),
                kind: *kind,
            },
        });
    }
    // 节点侧：同键两实例（feedback 重跑）
    for instance_no in [0_u32, 1] {
        let mut step = sample_step();
        step.step = ChangeStepKind::Executor;
        step.instance_no = instance_no;
        step.status = ChangeStepStatus::Passed;
        guard.emit(RunUpdate::Step { step });
    }

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 条目在场");
    assert_eq!(snapshot.edges.len(), 4, "四 kind 边齐备");
    let kinds: Vec<crate::state::EdgeKind> = snapshot.edges.iter().map(|edge| edge.kind).collect();
    assert_eq!(kinds, edges.to_vec(), "边按 emit 序稳定累积");
    assert_eq!(
        snapshot.edges[1].from_node_key(),
        "implement:1:executor:1",
        "feedback 回边端点 = 新实例身份串"
    );
    assert_eq!(snapshot.nodes.len(), 2, "同键两实例为两节点行");
    let instances: Vec<u32> = snapshot.nodes.iter().map(|node| node.instance_no).collect();
    assert_eq!(instances, vec![0, 1], "实例号递增不串扰");

    // guard 读面与快照面逐字段等值（第二写组装输入一致性）
    assert_eq!(guard.edges(), snapshot.edges);
    assert_eq!(guard.nodes(), snapshot.nodes);
}

/// 零边 run（单节点）→ 快照 `edges` 空数组；未 begin_run 就 publish → 注册表
/// 零残留（不 panic 不落半包状态）。
#[tokio::test]
async fn 零边快照空数组与未登记publish零残留() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: sample_step(),
    });
    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 条目在场");
    assert_eq!(snapshot.nodes.len(), 1);
    assert!(snapshot.edges.is_empty(), "零边 run → 空数组");

    // 未登记键（异 change id）：publish 静默零残留、不 panic
    control.publish(
        ROOT,
        CHANGE_ID_B,
        RunUpdate::Edge {
            edge: crate::state::ChangeStepEdge {
                from: ("x".to_owned(), 1, ChangeStepKind::Executor, 0),
                to: ("x".to_owned(), 1, ChangeStepKind::PhaseLog, 0),
                kind: crate::state::EdgeKind::Forward,
            },
        },
    );
    assert!(
        control.snapshot(ROOT, CHANGE_ID_B).is_none(),
        "未登记 change 无快照（不残留半包状态）"
    );

    guard.finish(ChangeRunStatus::Stopped, Some("收口".to_owned()));
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "终态除名（收口唯一出口）"
    );
}

// ---------------------------------------------------------------------------
// 归档前停等扩展（AC-7 / D2）：sync_specs 回流四组合、archive 槽两态、
// 终态除名后 snapshot 恒 None、广播零载荷
// ---------------------------------------------------------------------------

/// 归档前置只读载荷 fixture（八字段；与 walker / 命令层消费同一像）。
fn sample_preflight() -> crate::state::ArchivePreflight {
    crate::state::ArchivePreflight {
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

/// confirm `sync_specs` 回流（正向）：`(proceed, sync_specs)` 四组合逐组合
/// 原样回流（`wait_confirm` 返回二元组；AC-7 / D2）。
#[tokio::test]
async fn confirm回流四组合proceed与sync_specs逐组合() {
    for (proceed, sync_specs) in [(true, true), (true, false), (false, true), (false, false)] {
        let control = Arc::new(ChangeFlowControl::new());
        let guard = Arc::new(
            control
                .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
                .unwrap(),
        );
        let waiter_guard = Arc::clone(&guard);
        let waiter = tokio::spawn(async move { waiter_guard.wait_confirm().await });

        assert!(
            until_ok(|| control.confirm(ROOT, CHANGE_ID, proceed, sync_specs)).await,
            "confirm 应抵达挂起等待者（proceed={proceed} sync_specs={sync_specs}）"
        );
        assert_eq!(
            waiter.await.unwrap(),
            (proceed, sync_specs),
            "二元组逐组合原样回流（proceed={proceed} sync_specs={sync_specs}）"
        );
    }
}

/// confirm 异常面（异常）：无运行 run / 无等待方 → `Err` 记因；取消信号置位
/// → `wait_confirm` 以 `(false, false)` 收敛（停止不必先应答，AC-7）。
#[tokio::test]
async fn confirm异常面miss记因与取消收敛() {
    // 无运行 run → Err 携 id
    let control = Arc::new(ChangeFlowControl::new());
    let err = control
        .confirm(ROOT, CHANGE_ID, true, true)
        .expect_err("无 run 应 Err");
    assert!(
        err.contains(CHANGE_ID) && err.contains("无运行中的 run"),
        "{err}"
    );

    // 有 run 无等待方 → Err（单次通道语义）
    let run_control = Arc::new(ChangeFlowControl::new());
    let _guard = run_control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let err = run_control
        .confirm(ROOT, CHANGE_ID, false, true)
        .expect_err("无挂起应 Err");
    assert_eq!(err, "当前无等待中的 phase 确认");

    // 取消置位 → (false, false) 收敛（即便 sync_specs 请求 true 也不回流）
    let cancel_control = Arc::new(ChangeFlowControl::new());
    let guard = Arc::new(
        cancel_control
            .begin_run(ROOT, CHANGE_ID, "run-2".to_owned(), STARTED_AT)
            .unwrap(),
    );
    let waiter_guard = Arc::clone(&guard);
    let waiter = tokio::spawn(async move { waiter_guard.wait_confirm().await });
    tokio::task::yield_now().await;
    assert!(cancel_control.request_stop(ROOT, CHANGE_ID));
    assert_eq!(
        waiter.await.unwrap(),
        (false, false),
        "取消置位 → (false, false)（停止不必先应答）"
    );
}

/// snapshot 归档槽两态（正向）：`archive: Some(preflight)` / `None` 与 publish
/// 落值一致；终态除名后 snapshot 恒 None（archive 槽随之缺席，AC-7 / AC-10）。
#[tokio::test]
async fn snapshot_archive槽两态与除名缺席() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();

    // 初始：running 恒 None
    let snap = control.snapshot(ROOT, CHANGE_ID).expect("run 在册");
    assert!(snap.archive.is_none(), "running 态 archive 槽恒 None");

    // archive 前停等：载荷在场
    guard.emit(RunUpdate::ConfirmWait {
        phase: "archive".to_owned(),
        archive: Some(sample_preflight()),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).expect("run 在册");
    assert_eq!(snap.status, ChangeRunStatus::WaitingConfirm);
    assert_eq!(snap.archive, Some(sample_preflight()));

    // 终态除名：snapshot 恒 None（归档槽随之缺席）
    let guard = control
        .begin_run(ROOT, "change-c", "run-c".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::ConfirmWait {
        phase: "archive".to_owned(),
        archive: Some(sample_preflight()),
    });
    guard.finish(ChangeRunStatus::Completed, None);
    assert!(
        control.snapshot(ROOT, "change-c").is_none(),
        "终态除名后 snapshot 恒 None"
    );
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_some(),
        "他键 run 不受影响（复合键除名粒度）"
    );
}

/// 广播零载荷（边界）：`ConfirmWait` 携带 archive 载荷但 `RunNotice::ConfirmWait`
/// 仍 kind-only 零载荷（停等载荷不出进程——客户端经统一视图重查；AC-7 / D2）。
#[tokio::test]
async fn 归档停等广播零载荷kind_only() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).expect("订阅应成功");

    guard.emit(RunUpdate::ConfirmWait {
        phase: "archive".to_owned(),
        archive: Some(sample_preflight()),
    });

    let notice = rx.recv().await.expect("通知应到达");
    assert!(matches!(notice, RunNotice::ConfirmWait));
    assert_eq!(
        serde_json::to_value(notice).expect("出线应成功"),
        serde_json::json!({"ipc": "confirmWait"}),
        "通知单键零载荷（archive 载荷不出进程）"
    );
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（D4 / AC-4）：accumulate_node 合并单点（经
// begin_run → guard.emit → snapshot 真件读面承载）——`started_at` 无条件保
// 首见、`ended_at` / token some 才覆写、None 臂首见落值；同键多 emit 快照与
// 合并规则一致。零 mock——双 emit 序列 `ChangeStepState` 字面量直构。
// ---------------------------------------------------------------------------

/// 同键步状态行 fixture（稳定身份键 = (phase, attempt, step, instance_no)；
/// 四字段逐参显式注入——running / 终态两态共用构造器）。
#[allow(clippy::too_many_arguments)]
fn metrics_step(
    phase: &str,
    attempt: u32,
    step: ChangeStepKind,
    instance_no: u32,
    status: ChangeStepStatus,
    started_at: i64,
    ended_at: Option<i64>,
    token_input: Option<u64>,
    token_output: Option<u64>,
) -> ChangeStepState {
    ChangeStepState {
        phase: phase.to_owned(),
        attempt,
        step,
        instance_no,
        status,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at,
        ended_at,
        token_input,
        token_output,
    }
}

/// 正向：同键 running → 终态双 emit（终态行携相异 started 值）→ snapshot 节点
/// started = running 首见值（终态同刻值被丢弃，AC-2 / AC-4）。
#[tokio::test]
async fn accumulate_node_started保首见终态同刻值被丢弃() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Running,
            STARTED_AT,
            None,
            None,
            None,
        ),
    });
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Passed,
            STARTED_AT + 9_999,
            Some(STARTED_AT + 10_000),
            Some(1_200),
            Some(300),
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.nodes.len(), 1, "同键就地合并不追加");
    assert_eq!(
        snap.nodes[0].started_at, STARTED_AT,
        "started = running 首见值（终态行相异 started 值被丢弃）"
    );
}

/// 正向：双 emit 后 snapshot——ended_at / token 两值 = 终态行值（some 才覆写
/// 正向半边）。
#[tokio::test]
async fn accumulate_node_ended与token终态行值覆写() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Running,
            STARTED_AT,
            None,
            None,
            None,
        ),
    });
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Passed,
            STARTED_AT,
            Some(STARTED_AT + 5_000),
            Some(12_000),
            Some(3_400),
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.nodes[0].ended_at, Some(STARTED_AT + 5_000));
    assert_eq!(snap.nodes[0].token_input, Some(12_000));
    assert_eq!(snap.nodes[0].token_output, Some(3_400));
    assert_eq!(snap.nodes[0].status, ChangeStepStatus::Passed);
}

/// 正向：瞬步单 emit（首见即终态行）→ 节点 started == ended 自然成立（无需
/// 区分首 emit 是否终态，D4）。
#[tokio::test]
async fn accumulate_node_瞬步单emit首见即真值() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::PhaseStart,
            0,
            ChangeStepStatus::Passed,
            STARTED_AT,
            Some(STARTED_AT),
            None,
            None,
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.nodes.len(), 1);
    assert_eq!(snap.nodes[0].started_at, STARTED_AT);
    assert_eq!(snap.nodes[0].ended_at, Some(STARTED_AT), "瞬步同刻自然成立");
}

/// 正向：多键混排（phase / attempt / step / instanceNo 各异）→ 各键独立合并
/// 不串扰。
#[tokio::test]
async fn accumulate_node_多键混排独立合并不串扰() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    // 键甲（implement:1:executor:0）running → 终态；键乙（instance_no=1）
    // running；键丙（test-gen:1:phaseStart:0）终态单 emit
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            1,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Running,
            STARTED_AT,
            None,
            None,
            None,
        ),
    });
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            1,
            ChangeStepKind::Executor,
            1,
            ChangeStepStatus::Running,
            STARTED_AT + 100,
            None,
            None,
            None,
        ),
    });
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            1,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Passed,
            STARTED_AT,
            Some(STARTED_AT + 500),
            Some(10),
            Some(20),
        ),
    });
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "test-gen",
            1,
            ChangeStepKind::PhaseStart,
            0,
            ChangeStepStatus::Passed,
            STARTED_AT + 900,
            Some(STARTED_AT + 900),
            None,
            None,
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.nodes.len(), 3, "三键三节点（合并仅同键就地）");
    assert_eq!(snap.nodes[0].started_at, STARTED_AT, "键甲 started 首见");
    assert_eq!(snap.nodes[0].token_input, Some(10), "键甲终态 token 落值");
    assert_eq!(
        snap.nodes[1].started_at,
        STARTED_AT + 100,
        "键乙不受键甲终态行波及"
    );
    assert_eq!(snap.nodes[1].ended_at, None, "键乙仍在途");
    assert_eq!(snap.nodes[2].started_at, STARTED_AT + 900, "键丙独立");
    assert_eq!(snap.nodes[2].ended_at, Some(STARTED_AT + 900));
}

/// 边界：终态行 ended_at / token 全 None（running 残留收口形态）→ snapshot
/// 保持首见 None（None 不覆写半边）。
#[tokio::test]
async fn accumulate_node_终态行全none不覆写首见() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Running,
            STARTED_AT,
            None,
            None,
            None,
        ),
    });
    // 终态行全 None（崩溃残留收口形态——终态 emit 时间字段缺席的防御形态）
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Passed,
            STARTED_AT,
            None,
            None,
            None,
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        snap.nodes[0].status,
        ChangeStepStatus::Passed,
        "状态覆写不受 None 门限"
    );
    assert_eq!(
        snap.nodes[0].ended_at, None,
        "ended None 不覆写（首见 None 保持）"
    );
    assert_eq!(snap.nodes[0].token_input, None);
    assert_eq!(snap.nodes[0].token_output, None);
}

/// 边界：终态行 token 单侧在场（input Some / output None）→ 在场侧覆写、
/// 缺席侧保首见。
#[tokio::test]
async fn accumulate_node_token单侧在场单侧覆写() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    // 首见：input None / output Some(60)
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Running,
            STARTED_AT,
            None,
            None,
            Some(60),
        ),
    });
    // 终态：input Some(12_000) / output None → input 覆写、output 保首见
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Passed,
            STARTED_AT,
            Some(STARTED_AT + 1_000),
            Some(12_000),
            None,
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.nodes[0].token_input, Some(12_000), "在场侧覆写");
    assert_eq!(snap.nodes[0].token_output, Some(60), "缺席侧保首见");
}

/// 边界：同键三 emit 序列（running → running 重发 → 终态）→ started 仍首见、
/// ended 取终态值（AC-4「同键多 emit 后快照与合并规则一致」）。
#[tokio::test]
async fn accumulate_node_三emit序列started仍首见ended取终态值() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Running,
            STARTED_AT,
            None,
            None,
            None,
        ),
    });
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Running,
            STARTED_AT + 7,
            None,
            None,
            None,
        ),
    });
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Failed,
            STARTED_AT + 8,
            Some(STARTED_AT + 2_000),
            None,
            None,
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.nodes.len(), 1, "三 emit 一节点");
    assert_eq!(snap.nodes[0].started_at, STARTED_AT, "started 仍首见");
    assert_eq!(
        snap.nodes[0].ended_at,
        Some(STARTED_AT + 2_000),
        "ended 取终态行值"
    );
    assert_eq!(snap.nodes[0].status, ChangeStepStatus::Failed);
}

/// 异常：终态行携逆序时间（ended < started）→ 原样覆写零改写（合并不做语义
/// 校验——防御值直透）。
#[tokio::test]
async fn accumulate_node_逆序时间原样覆写零改写() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Running,
            STARTED_AT + 10_000,
            None,
            None,
            None,
        ),
    });
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Passed,
            STARTED_AT + 10_000,
            Some(STARTED_AT),
            None,
            None,
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        snap.nodes[0].ended_at,
        Some(STARTED_AT),
        "逆序 ended 原样覆写（合并不校验不修正）"
    );
    assert_eq!(
        snap.nodes[0].started_at,
        STARTED_AT + 10_000,
        "started 首见不被逆序行顶替"
    );
}

/// 异常：首见行即终态且 ended None → started 落值、ended None（合并层不铸造
/// 不补值——构造值直透）。
#[tokio::test]
async fn accumulate_node_首见终态无ended零补值() {
    let control = Arc::new(ChangeFlowControl::new());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Step {
        step: metrics_step(
            "implement",
            2,
            ChangeStepKind::Executor,
            0,
            ChangeStepStatus::Stopped,
            STARTED_AT + 42,
            None,
            None,
            None,
        ),
    });
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.nodes[0].status, ChangeStepStatus::Stopped);
    assert_eq!(
        snap.nodes[0].started_at,
        STARTED_AT + 42,
        "started 构造值直透落值"
    );
    assert_eq!(snap.nodes[0].ended_at, None, "ended 首见 None 零补值零铸造");
}

// ---------------------------------------------------------------------------
// 取消双写（test-design「control.rs -> control_test.rs」节，AC-6 / D8 / D9）：
// `begin_run` 铸造零值 flag、`request_stop` 双写（watch + 进程面原子）、无订阅者
// 不丢、幂等、miss 面与多 run 隔离、`RunGuard::cancel_flag()` 同一原子。
// ---------------------------------------------------------------------------

/// 正向：`begin_run` 后 `cancel_flag()` 初值——新 run 零置位，且与 `cancelled()`
/// 同源观测一致。
#[tokio::test]
async fn begin_run后cancel_flag初值零置位() {
    let control = Arc::new(ChangeFlowControl::default());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-flag-init".to_owned(), STARTED_AT)
        .expect("首次发起应成功");

    assert!(
        !guard.cancel_flag().is_set(),
        "新登记 run 的进程面标志零置位（铸造 never()）"
    );
    assert!(!guard.cancelled(), "watch 面同样未置位");
}

/// 正向（AC-6）：`request_stop` 双写——同一调用点同时置位 watch 面与进程面
///（`guard.cancelled()` 与 `cancel_flag().is_set()` 同刻为真）。
#[tokio::test]
async fn request_stop双写_watch面与进程面同刻置位() {
    let control = Arc::new(ChangeFlowControl::default());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-double-write".to_owned(), STARTED_AT)
        .expect("首次发起应成功");

    assert!(control.request_stop(ROOT, CHANGE_ID), "运行中停止应命中");
    assert!(guard.cancelled(), "watch 臂置位（send_replace）");
    assert!(guard.cancel_flag().is_set(), "进程面臂置位（原子 set）");
}

/// 正向（AC-6）：无订阅者不丢——不取任何 `updates` / `cancelled_wait` 订阅即
/// `request_stop` → 进程面标志仍为真（`send_replace` 与原子双写均不依赖订阅者）。
#[tokio::test]
async fn request_stop无订阅者不丢位() {
    let control = Arc::new(ChangeFlowControl::default());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-no-subscriber".to_owned(), STARTED_AT)
        .expect("首次发起应成功");

    // 刻意不 subscribe 任何通道
    assert!(control.request_stop(ROOT, CHANGE_ID));
    assert!(
        guard.cancel_flag().is_set(),
        "无订阅者仍置位（watch send_replace + 原子双写）"
    );
}

/// 边界（D9）：`request_stop` 幂等——连调两次均返回 true，标志仍为真，无 panic。
#[tokio::test]
async fn request_stop幂等_连调两次均命中() {
    let control = Arc::new(ChangeFlowControl::default());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-idempotent".to_owned(), STARTED_AT)
        .expect("首次发起应成功");

    assert!(control.request_stop(ROOT, CHANGE_ID), "首次命中断言");
    assert!(
        control.request_stop(ROOT, CHANGE_ID),
        "重复停止仍命中（幂等）"
    );
    assert!(guard.cancel_flag().is_set(), "重复 set 无副作用");
}

/// 边界（表驱动 2 形态）：miss 面——未登记键 / 已 `finish` 除名键 → `false`，
/// 不 panic（已除名 run 的 flag 由在跑 runner 持有，置位与否不再经注册表）。
#[tokio::test]
async fn request_stop_miss面两形态() {
    let control = Arc::new(ChangeFlowControl::default());

    // 形态一：未登记
    assert!(
        !control.request_stop(ROOT, CHANGE_ID),
        "未登记键 → false（不 panic）"
    );

    // 形态二：已 finish 除名
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-finish-miss".to_owned(), STARTED_AT)
        .expect("首次发起应成功");
    let flag = guard.cancel_flag();
    guard.finish(ChangeRunStatus::Completed, None);
    assert!(
        !control.request_stop(ROOT, CHANGE_ID),
        "已收口除名键 → false"
    );
    assert!(
        !flag.is_set(),
        "除名后的停止不再置位旧 flag（已由 runner 持有）"
    );
}

/// 边界（表驱动 2 形态：异 change id / 异 root）：多 run 隔离——`request_stop`
/// 只置位目标 run 的 `cancel_flag`，另一 run 恒 false。
#[tokio::test]
async fn request_stop多run隔离() {
    // 形态一：同 root 异 change id
    let control = Arc::new(ChangeFlowControl::default());
    let first = control
        .begin_run(ROOT, CHANGE_ID, "run-a".to_owned(), STARTED_AT)
        .expect("A 发起应成功");
    let second = control
        .begin_run(ROOT, CHANGE_ID_B, "run-b".to_owned(), STARTED_AT)
        .expect("B 发起应成功");
    assert!(control.request_stop(ROOT, CHANGE_ID));
    assert!(first.cancel_flag().is_set(), "目标 run A 置位");
    assert!(!second.cancel_flag().is_set(), "另一 run B 不受影响");

    // 形态二：异 root 同 change id
    let control = Arc::new(ChangeFlowControl::default());
    let here = control
        .begin_run(ROOT, CHANGE_ID, "run-root-a".to_owned(), STARTED_AT)
        .expect("root-a 发起应成功");
    let there = control
        .begin_run("/ws/root-b", CHANGE_ID, "run-root-b".to_owned(), STARTED_AT)
        .expect("root-b 发起应成功（异 root 不撞键）");
    assert!(control.request_stop("/ws/root-b", CHANGE_ID));
    assert!(there.cancel_flag().is_set(), "root-b 的 run 置位");
    assert!(!here.cancel_flag().is_set(), "root-a 的 run 不受影响");
}

/// 边界（D8）：`RunGuard::cancel_flag()` 同一原子——同 guard 连续两次取值相等
///（`Arc::ptr_eq`），置位后两次取值均读 true。
#[tokio::test]
async fn run_guard_cancel_flag同一原子() {
    let control = Arc::new(ChangeFlowControl::default());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-same-arc".to_owned(), STARTED_AT)
        .expect("首次发起应成功");

    let first = guard.cancel_flag();
    let second = guard.cancel_flag();
    assert_eq!(first, second, "两次取值同一原子（身份相等）");

    first.handle().set();
    assert!(second.is_set(), "一处置位两处可见");
    assert_eq!(second, guard.cancel_flag(), "置位后仍身份相等");
}

/// 边界：终态除名后的取消面——`finish` 后 `request_stop` 返回 false、
/// `snapshot` 为 `None`；含 flag 的 `RunEntry` 随去除名，`Arc` 由在跑 runner
/// 持有至释放（除名不丢置位能力）。
#[tokio::test]
async fn finish除名后flag仍可由runner侧置位() {
    let control = Arc::new(ChangeFlowControl::default());
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-after-finish".to_owned(), STARTED_AT)
        .expect("首次发起应成功");
    let held = guard.cancel_flag();

    guard.finish(ChangeRunStatus::Completed, None);
    assert!(control.snapshot(ROOT, CHANGE_ID).is_none(), "终态除名");
    assert!(!control.request_stop(ROOT, CHANGE_ID), "除名后停止 miss");

    // 在跑 runner 持有的句柄仍可置位（Arc 生命周期不随注册表除名终止）
    held.handle().set();
    assert!(held.is_set(), "除名后 flag 仍可置位（runner 持有至释放）");
}
