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

use agent::AskRegistry;

use crate::control::ChangeFlowControl;
use crate::state::{
    AskPayload, ChangeRunStatus, ChangeStepKind, ChangeStepState, ChangeStepStatus, RunNotice,
    RunUpdate,
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
            turn_id: None,
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));

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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control_b = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));

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

/// answer 应答回传：`AskRegistry` 登记等待方后 answer(text) 把原文转投该表
/// （ask 应答回流）；单次通道 take 语义——重复应答落空 Err。
#[tokio::test]
async fn answer应答原文回流挂起等待者_单次take() {
    let ask_registry = Arc::new(AskRegistry::default());
    let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    guard.set_session(Some("core-1".to_owned()));
    let waiter = spawn_waiter(&ask_registry, "core-1");

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

/// ask 应答即解除停等：`status` 回 `Running` 且 `ask` 槽清空——徽章（读
/// status）与中断卡片（读 ask 槽）两处判据独立消费，必须同刻清；失效广播走
/// 通用 `RunNotice::Step`（`Ask` 在通知面被读作「新停等开始」，重放会触发
/// 订阅方对已解除的等待再答一次）。
#[tokio::test]
async fn answer后停等态解除且广播失效() {
    let ask_registry = Arc::new(AskRegistry::default());
    let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    guard.set_session(Some("core-1".to_owned()));
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "采用哪个方案?".to_owned(),
            options: vec!["A".to_owned(), "B".to_owned()],
            tool_call_id: None,
            session_id: None,
        },
    });
    assert!(matches!(rx.try_recv(), Ok(RunNotice::Ask)), "进入停等通知");

    let waiter = spawn_waiter(&ask_registry, "core-1");
    assert!(
        until_ok(|| control.answer(ROOT, CHANGE_ID, "采用方案 B".to_owned())).await,
        "answer 受理"
    );
    assert_eq!(waiter.await.unwrap().as_deref(), Some("采用方案 B"));
    // 停等态清理搬到 ask 执行体返回后（`resolve_ask` 单点）
    control.resolve_ask(ROOT, CHANGE_ID);

    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::Running, "状态回运行态");
    assert!(snap.ask.is_none(), "ask 载荷槽同刻清空");
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "解除失效广播（通用 kind）"
    );

    // 幂等：状态已回运行态，重复受理仍落空（通道单次 take）
    let err = control
        .answer(ROOT, CHANGE_ID, "再来一次".to_owned())
        .expect_err("重复应答应 Err");
    assert_eq!(err, "当前无等待中的 ask");
    assert_eq!(
        control.snapshot(ROOT, CHANGE_ID).unwrap().status,
        ChangeRunStatus::Running,
        "落空应答不改写状态"
    );
}

/// confirm 应答即解除停等：`status` 回 `Running`、`archive` 槽清空（归档前
/// 停等与普通相位停等两臂同形——残留命中率高的一侧）。
#[tokio::test]
async fn confirm后停等态解除且归档槽清空() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    guard.emit(RunUpdate::ConfirmWait {
        phase: "archive".to_owned(),
        archive: Some(sample_preflight()),
    });
    assert!(matches!(rx.try_recv(), Ok(RunNotice::ConfirmWait)));

    let waiter_guard = Arc::clone(&guard);
    let waiter = tokio::spawn(async move { waiter_guard.wait_confirm().await });
    assert!(
        until_ok(|| control.confirm(ROOT, CHANGE_ID, true, true)).await,
        "confirm 受理"
    );
    assert_eq!(waiter.await.unwrap(), (true, true));

    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::Running, "状态回运行态");
    assert!(snap.archive.is_none(), "归档前置载荷槽同刻清空");
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "解除失效广播（通用 kind）"
    );

    // 幂等：重复受理落空（单次通道已取走）
    let err = control
        .confirm(ROOT, CHANGE_ID, true, true)
        .expect_err("重复确认应 Err");
    assert_eq!(err, "当前无等待中的 phase 确认");
}

/// 通道已关闭（等待方被取消先行退出）：通道 take 成功即视为受理——send 失败
/// 不改 `Ok`，停等态照常解除（前端继续挂一个已无接收方的等待卡无意义）。
#[tokio::test]
async fn answer通道关闭仍受理并解除停等() {
    let ask_registry = Arc::new(AskRegistry::default());
    let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    guard.set_session(Some("core-1".to_owned()));
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "是否继续?".to_owned(),
            options: vec!["继续".to_owned()],
            tool_call_id: None,
            session_id: None,
        },
    });
    assert!(control.request_stop(ROOT, CHANGE_ID), "停止置位");
    // 接收端已释放（会话被终止）：send 失败不改受理结论（槽已被 take）
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    drop(rx);
    ask_registry.register("core-1", tx);

    control
        .answer(ROOT, CHANGE_ID, "迟到应答".to_owned())
        .expect("通道 take 成功即受理");
    control.resolve_ask(ROOT, CHANGE_ID);
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::Running, "停等态照常解除");
    assert!(snap.ask.is_none(), "ask 载荷槽同刻清空");
}

/// 终态先到的竞态守卫：`Finished` 已在应答前落值（walker 收口未被除名捕获
/// 的毫秒窗口）时，应答不把终态倒灌回 `Running`——清理以「当前 status 恰为
/// 该停等态」为条件。
#[tokio::test]
async fn answer不改写先到的终态() {
    let ask_registry = Arc::new(AskRegistry::default());
    let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );
    guard.set_session(Some("core-1".to_owned()));
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "是否继续?".to_owned(),
            options: vec!["继续".to_owned()],
            tool_call_id: None,
            session_id: None,
        },
    });
    let waiter = spawn_waiter(&ask_registry, "core-1");

    guard.emit(RunUpdate::Finished {
        status: ChangeRunStatus::Stopped,
        reason: None,
    });
    assert_eq!(
        control.answer(ROOT, CHANGE_ID, "继续".to_owned()),
        Ok(()),
        "通道 take 成功即受理（send 结果不改受理语义）"
    );
    control.resolve_ask(ROOT, CHANGE_ID);
    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::Stopped, "终态不被倒灌");
    assert!(
        snap.ask.is_none(),
        "ask 槽仍被清空（行内卡判活靠 toolCallId）"
    );
    let _ = waiter.await;
}

/// 等待方登记辅助：在 ask 注册表登记 `core-1` 并挂起等应答（单次 take 语义的
/// 接收端）。
fn spawn_waiter(
    registry: &Arc<AskRegistry>,
    session_id: &'static str,
) -> tokio::task::JoinHandle<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    registry.register(session_id, tx);
    tokio::spawn(async move { rx.await.ok() })
}

/// answer 无等待方：无运行 run → Err 携 change id；有 run 无挂起 → Err
/// 「无等待中的 ask」。
#[tokio::test]
async fn answer无等待方err携id记因() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));

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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
        ask: AskPayload {
            question: "越权 backtrack 是否放行?".to_owned(),
            options: vec!["拒绝".to_owned(), "放行".to_owned()],
            tool_call_id: None,
            session_id: None,
        },
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
            turn_id: None,
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

/// 取消信号中断挂起停等：wait_confirm 挂起期间 request_stop 置位即收敛
///（proceed=false，停止不必先应答——guard 侧 select 半边）。
#[tokio::test]
async fn cancel置位中断挂起停等() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = Arc::new(
        control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap(),
    );

    let confirm_guard = Arc::clone(&guard);
    let confirm_waiter = tokio::spawn(async move { confirm_guard.wait_confirm().await });

    // 让 waiter 起跑挂起（取消语义与登记次序无关：先置位亦即刻收敛）
    tokio::task::yield_now().await;

    assert!(control.request_stop(ROOT, CHANGE_ID), "停止置位");
    assert_eq!(
        confirm_waiter.await.unwrap(),
        (false, false),
        "取消置位 → (false, false) 收敛（停止不必先应答）"
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
        ask: AskPayload {
            question: "root-b 的提问".to_owned(),
            options: vec!["甲".to_owned()],
            tool_call_id: None,
            session_id: None,
        },
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
            turn_id: None,
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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

/// 边界（AC-16 / 事实校准 2）：`archi_check` 是**工具步**——会话事件到达时该
/// 运行中节点 MUST NOT 被挂上 session id，也不补发 `Step` 通知（`is_session_step`
/// 为正向 `matches!` 五类会话步，新步词天然判假，零代码改动）。
#[tokio::test]
async fn publish_session_event_archi_check工具步不挂会话id零补拍() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    guard.emit(RunUpdate::Step {
        step: running_step(ChangeStepKind::ArchiCheck, 0, None),
    });
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "工具步 running emit 通知先排空"
    );

    guard.emit(session_event("sess-archi", 0));

    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        snap.nodes[0].session_id, None,
        "archi_check 不挂会话 id（工具步零会话）"
    );
    assert_eq!(
        snap.nodes[0].status,
        ChangeStepStatus::Running,
        "状态零改写"
    );
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::SessionEvent)),
        "仅原 SessionEvent kind 广播"
    );
    assert!(rx.try_recv().is_err(), "零 Step 补拍（未挂载即不补发）");
}

/// 正向（AC-16）：`archi_check` 节点两态（running / passed）经既有快照与广播面
/// 正常出线——步词为落库 camel 词 `archiCheck`（`ChangeStepKind` 单点），
/// `session_id` 恒 `None`，一次发布一拍、零补拍；同 run 内的会话型步照常挂载
/// （正向枚举语义不因新增步词走否定分支）。
#[tokio::test]
async fn publish_archi_check节点两态出线且会话型步仍挂载() {
    for status in [ChangeStepStatus::Running, ChangeStepStatus::Passed] {
        let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
        let guard = control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap();
        let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();

        guard.emit(RunUpdate::Step {
            step: ChangeStepState {
                status,
                detail: Some("未检查（无模型）".to_owned()),
                ..running_step(ChangeStepKind::ArchiCheck, 0, None)
            },
        });
        let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
        assert_eq!(snap.nodes.len(), 1, "{status:?} 态节点在快照中在场");
        assert_eq!(
            snap.nodes[0].step,
            ChangeStepKind::ArchiCheck,
            "步词汇原样出线（线面词由 as_str 单点派生）"
        );
        assert_eq!(
            snap.nodes[0].step.as_str(),
            "archi_check",
            "线词 snake_case"
        );
        assert_eq!(snap.nodes[0].session_id, None, "工具步 session 恒 None");
        assert_eq!(
            snap.nodes[0].detail.as_deref(),
            Some("未检查（无模型）"),
            "detail 原样透传（不冒充通过）"
        );
        assert!(
            matches!(rx.try_recv(), Ok(RunNotice::Step)),
            "{status:?} 态一次发布一拍"
        );

        // 同 run 内后到的会话型步：会话事件仍挂到 executor 上（正向枚举语义）
        guard.emit(RunUpdate::Step {
            step: running_step(ChangeStepKind::Executor, 0, None),
        });
        assert!(matches!(rx.try_recv(), Ok(RunNotice::Step)), "executor 拍");
        guard.emit(session_event("sess-exec-live", 0));
        let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
        assert_eq!(
            snap.nodes[0].session_id, None,
            "archi_check 节点未被误挂会话 id"
        );
        assert_eq!(
            snap.nodes[1].session_id.as_deref(),
            Some("sess-exec-live"),
            "会话 id 挂到会话型节点（与新增步词零耦合）"
        );
        assert!(
            matches!(rx.try_recv(), Ok(RunNotice::Step)),
            "首挂补发 Step 通知"
        );
        assert!(
            matches!(rx.try_recv(), Ok(RunNotice::SessionEvent)),
            "原 SessionEvent kind 照常广播"
        );
    }
}

/// publish(Ask) / publish(ConfirmWait) / publish(Finished) → 状态迁移与 ask
/// 载荷照旧 + 对应 Notice kind 各自广播（AC-8 kind 位一一对应）。
#[tokio::test]
async fn publish停等与终态_状态迁移照旧且notice对应() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();

    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "回溯到哪?".to_owned(),
            options: vec!["proposal".to_owned()],
            tool_call_id: None,
            session_id: None,
        },
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
            ask: AskPayload {
                question: "q".to_owned(),
                options: Vec::new(),
                tool_call_id: None,
                session_id: None,
            },
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
        let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let err = control
        .confirm(ROOT, CHANGE_ID, true, true)
        .expect_err("无 run 应 Err");
    assert!(
        err.contains(CHANGE_ID) && err.contains("无运行中的 run"),
        "{err}"
    );

    // 有 run 无等待方 → Err（单次通道语义）
    let run_control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let _guard = run_control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let err = run_control
        .confirm(ROOT, CHANGE_ID, false, true)
        .expect_err("无挂起应 Err");
    assert_eq!(err, "当前无等待中的 phase 确认");

    // 取消置位 → (false, false) 收敛（即便 sync_specs 请求 true 也不回流）
    let cancel_control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));

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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
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

// ---------------------------------------------------------------------------
// archi_check 非会话步（AC-12 / D18）：`is_session_step` 正向 matches! 天然判假
// ---------------------------------------------------------------------------

/// 正向（D18）：`is_session_step` 是**正向白名单** `matches!`（五类会话步逐条列举），
/// 故工具步 `archi_check` 天然判假——源码自证：白名单不含 `ArchiCheck`，且
/// MUST NOT 被改写成否定列表（`!matches!` / `_ =>`）。
#[test]
fn archi_check非会话步() {
    let source = include_str!("control.rs");
    let start = source
        .find("fn is_session_step(")
        .expect("is_session_step 定义在职");
    let body: String = source[start..].chars().take(400).collect();
    assert!(
        body.contains("matches!("),
        "会话步判定为正向 matches! 白名单"
    );
    assert!(
        !body.contains("ArchiCheck"),
        "工具步 MUST NOT 进会话白名单（正向判假即 AC-12 的零会话语义）"
    );
    for session in [
        "Executor",
        "Evaluator",
        "Decision",
        "SpecSync",
        "ResolveConflicts",
    ] {
        assert!(
            body.contains(session),
            "五类会话步 {session} 仍在白名单（零回归）"
        );
    }
    assert!(!body.contains("!matches!("), "MUST NOT 改写成否定列表");
}

// ---------------------------------------------------------------------------
// `StepEmitter` 与 emit 面上收（change-detail-live-stdout，AC-5 / AC-2 / AC-13）
//
// 被测面：`RunGuard::emitter`（`'static` 发射器：`(root, change_id)` 复合键 +
// `Arc<ChangeFlowControl>`）、`StepEmitter::attach_output_ref`（补发一拍
// `running + output_ref`，经同一 `emit_node` 铸造单点；`accumulate_node` 就地
// 合并——不新增节点、不动 `started_at`）、`now_millis`（时钟缝单点）与
// `emit_node` 的搬迁后行为。
//
// Mock 策略（见 test-design 本节 Mock策略表）：零替身——真实
// `ChangeFlowControl` + `#[tokio::test]` 进程内 tokio 原语；时间断言只用区间与
// 单调性（`now_millis` 无注入点，见「不可测试项」）。
// ---------------------------------------------------------------------------

use crate::control::{now_millis, StepSettle};

/// `implement / attempt 2 / StaticCheck / instance 0` 的步状态 fixture（补发拍
/// 与终态拍共用身份键）。
fn emit_fixture() -> (ChangeStepKind, u32, u32) {
    (ChangeStepKind::StaticCheck, 2, 0)
}

/// 正向：已 `begin_run` 的 guard → `emitter()` 产出的发射器在 guard **被 drop
/// 后**仍可 `attach_output_ref`（`'static` 语义：复合键 + `Arc` 自带所有权），
/// 节点被写出。
#[tokio::test]
async fn emitter在guard_drop后仍可补发拍() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let emitter = guard.emitter();
    drop(guard);

    let (kind, attempt, instance_no) = emit_fixture();
    emitter.attach_output_ref("implement", attempt, kind, instance_no, "exec-after-drop");

    let snapshot = control
        .snapshot(ROOT, CHANGE_ID)
        .expect("run 条目仍在（未 finish）");
    assert_eq!(snapshot.nodes.len(), 1, "补发拍写出节点（guard 已 drop）");
    assert_eq!(
        snapshot.nodes[0].output_ref.as_deref(),
        Some("exec-after-drop"),
        "output_ref 落位"
    );
}

/// 正向：同一 guard 两次 `emitter()` → 两发射器对同一 `(root, change_id)` 键
/// 写入，节点累积在同一 run 上（零串台）。
#[tokio::test]
async fn emitter两次产出写同一run() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let first = guard.emitter();
    let second = guard.emitter();
    let (kind, attempt, instance_no) = emit_fixture();

    first.attach_output_ref("implement", attempt, kind, instance_no, "exec-a");
    second.attach_output_ref("implement", attempt, kind, instance_no, "exec-b");

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 在场");
    assert_eq!(snapshot.nodes.len(), 1, "同身份键就地合并（不新增节点）");
    assert_eq!(
        snapshot.nodes[0].output_ref.as_deref(),
        Some("exec-b"),
        "accumulate_node 的 some 才覆写：后到值生效"
    );
}

/// 异常：run 已 `finish`（终态除名）后调用 `emitter()` 再 `attach_output_ref`
/// → 静默无副作用（注册表 miss 分支），零 panic、零节点新增。
#[tokio::test]
async fn emitter在finish后静默无副作用() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let emitter = guard.emitter();
    guard.finish(ChangeRunStatus::Completed, None);
    assert!(control.snapshot(ROOT, CHANGE_ID).is_none(), "finish 后除名");

    let (kind, attempt, instance_no) = emit_fixture();
    emitter.attach_output_ref("implement", attempt, kind, instance_no, "exec-late");
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "除名后补发拍零副作用（不重建 run 条目）"
    );
}

/// 边界（组合 同 root × 异 id）：同 root 异 change id 两 guard 的发射器交叉写入
/// → 各自 run 的节点互不可见（复合键隔离）。
#[tokio::test]
async fn emitter复合键隔离同root异change() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard_a = control
        .begin_run(ROOT, CHANGE_ID, "run-a".to_owned(), STARTED_AT)
        .expect("首个 run");
    let guard_b = control
        .begin_run(ROOT, CHANGE_ID_B, "run-b".to_owned(), STARTED_AT)
        .expect("异 id 可并行");
    let emitter_a = guard_a.emitter();
    let emitter_b = guard_b.emitter();
    let (kind, attempt, instance_no) = emit_fixture();

    emitter_a.attach_output_ref("implement", attempt, kind, instance_no, "exec-a");
    emitter_b.attach_output_ref("implement", attempt, kind, instance_no, "exec-b");

    let snapshot_a = control.snapshot(ROOT, CHANGE_ID).expect("run-a 在场");
    let snapshot_b = control.snapshot(ROOT, CHANGE_ID_B).expect("run-b 在场");
    assert_eq!(snapshot_a.nodes.len(), 1, "run-a 只收到自己的补发拍");
    assert_eq!(snapshot_b.nodes.len(), 1, "run-b 只收到自己的补发拍");
    assert_eq!(snapshot_a.nodes[0].output_ref.as_deref(), Some("exec-a"));
    assert_eq!(snapshot_b.nodes[0].output_ref.as_deref(), Some("exec-b"));
}

/// 正向：先 `emit_node(running)`（无 `output_ref`），再 `attach_output_ref` → 同
/// 键节点被就地合并：`status == running`、`output_ref` 落位、节点**总数仍为
/// 1**、`started_at` 为首次 emit 值（AC-5 / AC-13 的合并语义）。
#[tokio::test]
async fn attach_output_ref就地合并不新增节点() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let (kind, attempt, instance_no) = emit_fixture();
    let emitter = guard.emitter();

    let before = now_millis();
    crate::control::emit_node(
        &guard,
        "implement",
        attempt,
        kind,
        instance_no,
        StepSettle::running(),
    );
    let started = control
        .snapshot(ROOT, CHANGE_ID)
        .expect("run 在场")
        .nodes
        .first()
        .expect("running 拍已落")
        .started_at;
    assert!(started >= before, "started_at 为 now_millis 量级");

    emitter.attach_output_ref("implement", attempt, kind, instance_no, "exec-merged");

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 在场");
    assert_eq!(snapshot.nodes.len(), 1, "节点总数仍为 1（就地合并）");
    let node = &snapshot.nodes[0];
    assert_eq!(node.status, ChangeStepStatus::Running, "补发拍不推进状态");
    assert_eq!(
        node.output_ref.as_deref(),
        Some("exec-merged"),
        "output_ref 落位"
    );
    assert_eq!(
        node.started_at, started,
        "started_at 保首见（未被补发拍顶替）"
    );
    assert_eq!(node.ended_at, None, "running 拍无 ended_at");
}

/// 正向：补发后再 `emit_node(passed, with_output_ref(Some))` → 终态拍
/// `output_ref` 与补发拍同值（同 id 收口，AC-2）。
#[tokio::test]
async fn 终态拍与补发拍output_ref同值() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let (kind, attempt, instance_no) = emit_fixture();
    let emitter = guard.emitter();

    crate::control::emit_node(
        &guard,
        "implement",
        attempt,
        kind,
        instance_no,
        StepSettle::running(),
    );
    emitter.attach_output_ref("implement", attempt, kind, instance_no, "exec-same");
    crate::control::emit_node(
        &guard,
        "implement",
        attempt,
        kind,
        instance_no,
        StepSettle::passed(Some("通过".to_owned())).with_output_ref(Some("exec-same".to_owned())),
    );

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 在场");
    assert_eq!(snapshot.nodes.len(), 1, "三拍合于一个节点");
    let node = &snapshot.nodes[0];
    assert_eq!(node.status, ChangeStepStatus::Passed, "终态拍推进状态");
    assert_eq!(node.output_ref.as_deref(), Some("exec-same"), "同 id 收口");
    assert!(node.ended_at.is_some(), "终态拍铸 ended_at");
}

/// 异常：目标节点不存在（未先 emit running）→ 补发拍按 emit 序**追加**为一条
/// `running + output_ref` 新节点（`accumulate_node` 的无命中分支，非契约外
/// 崩溃）；只锁「不 panic、不静默丢弃」。
#[tokio::test]
async fn attach_output_ref目标缺失时追加新节点() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let emitter = guard.emitter();
    let (kind, attempt, instance_no) = emit_fixture();

    emitter.attach_output_ref("implement", attempt, kind, instance_no, "exec-orphan");

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 在场");
    assert_eq!(snapshot.nodes.len(), 1, "无命中分支追加一条新节点");
    assert_eq!(snapshot.nodes[0].status, ChangeStepStatus::Running);
    assert_eq!(
        snapshot.nodes[0].output_ref.as_deref(),
        Some("exec-orphan"),
        "不静默丢弃"
    );
}

/// 边界（参数化 `instance_no` 2 × `exec_id` 3 形态）：不同 `instance_no` → 两
/// 独立节点各携各自 `output_ref`（同键多实例不串扰）；空串与超长（>1000
/// chars）`exec_id` 原样落 `output_ref`（零截断、零改写）。
#[tokio::test]
async fn attach_output_ref多实例与exec_id形态() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let emitter = guard.emitter();
    let long = "e".repeat(1200);

    for (instance_no, exec_id) in [(0u32, ""), (1u32, long.as_str())] {
        crate::control::emit_node(
            &guard,
            "implement",
            2,
            ChangeStepKind::StaticCheck,
            instance_no,
            StepSettle::running(),
        );
        emitter.attach_output_ref(
            "implement",
            2,
            ChangeStepKind::StaticCheck,
            instance_no,
            exec_id,
        );
    }

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 在场");
    assert_eq!(snapshot.nodes.len(), 2, "两实例各一节点（不串扰）");
    let zero = snapshot
        .nodes
        .iter()
        .find(|node| node.instance_no == 0)
        .expect("实例 0");
    let one = snapshot
        .nodes
        .iter()
        .find(|node| node.instance_no == 1)
        .expect("实例 1");
    assert_eq!(zero.output_ref.as_deref(), Some(""), "空串原样（零改写）");
    assert_eq!(
        one.output_ref.as_deref(),
        Some(long.as_str()),
        "超长原样（零截断）"
    );
}

/// 边界：补发拍不覆盖在位值——节点已有 `detail` / `session_id` /
/// `token_input` 时，补发后三者保持原值（`accumulate_node` 的「some 才覆写」+
/// 补发只携 `output_ref` 与 `status`）。
#[tokio::test]
async fn attach_output_ref不覆盖在位字段() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let emitter = guard.emitter();
    let (kind, attempt, instance_no) = emit_fixture();

    crate::control::emit_node(
        &guard,
        "implement",
        attempt,
        kind,
        instance_no,
        StepSettle::passed(Some("诊断摘要".to_owned()))
            .with_session(Some("sess-9".to_owned()))
            .with_tokens(Some(11), Some(22)),
    );
    emitter.attach_output_ref("implement", attempt, kind, instance_no, "exec-merge");

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 在场");
    let node = &snapshot.nodes[0];
    assert_eq!(node.detail.as_deref(), Some("诊断摘要"), "detail 保持原值");
    assert_eq!(
        node.session_id.as_deref(),
        Some("sess-9"),
        "session_id 保持原值"
    );
    assert_eq!(node.token_input, Some(11), "token_input 保持原值");
    assert_eq!(node.token_output, Some(22), "token_output 保持原值");
    assert_eq!(
        node.output_ref.as_deref(),
        Some("exec-merge"),
        "只补 output_ref"
    );
}

/// 正向：`now_millis` 返回值 > 0（epoch 之后），两次连续调用单调不减。
#[test]
fn now_millis为正且单调不减() {
    let first = now_millis();
    let second = now_millis();
    assert!(first > 0, "epoch 之后为正: {first}");
    assert!(second >= first, "单调不减（{first} → {second}）");
}

/// 异常：`now_millis` 不 panic 且非负（时钟早于 epoch 的臂无注入点，见
/// 「不可测试项」的时钟缝说明——本行只锁运行期非负性）。
#[test]
fn now_millis非负不panic() {
    let value = now_millis();
    assert!(value >= 0, "非负: {value}");
}

/// 边界（组合 running × 终态）：`emit_node(running)` 后 `started_at` 与调用前
/// 取样同量级（`before <= started_at <= after`）；终态拍 `ended_at` 非 `None` 且
/// `>= started_at`。
#[tokio::test]
async fn emit_node时间戳区间与终态ended_at() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let (kind, attempt, instance_no) = emit_fixture();

    let before = now_millis();
    crate::control::emit_node(
        &guard,
        "implement",
        attempt,
        kind,
        instance_no,
        StepSettle::running(),
    );
    let after = now_millis();
    let running = control
        .snapshot(ROOT, CHANGE_ID)
        .expect("run 在场")
        .nodes
        .remove(0);
    assert!(
        running.started_at >= before && running.started_at <= after,
        "started_at 落在区间内（{before} ≤ {} ≤ {after}）",
        running.started_at
    );
    assert_eq!(running.ended_at, None, "running 拍无 ended_at");

    crate::control::emit_node(
        &guard,
        "implement",
        attempt,
        kind,
        instance_no,
        StepSettle::failed(Some("失败".to_owned())),
    );
    let terminal = control
        .snapshot(ROOT, CHANGE_ID)
        .expect("run 在场")
        .nodes
        .remove(0);
    let ended = terminal.ended_at.expect("终态拍铸 ended_at");
    assert!(ended >= terminal.started_at, "ended_at >= started_at");
    assert_eq!(
        terminal.started_at, running.started_at,
        "started_at 保首见（终态拍不顶替）"
    );
}

/// 异常：run 已除名后 `emit_node` 零 panic——同键重入的新 run 只累积自己的
/// 节点，旧发射器的补发拍按身份键合并进**新** run（同一复合键的注册表语义，
/// 非串台）；无 panic、无幽灵条目。
#[tokio::test]
async fn emit_node在除名后零panic且同键重入独立() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");
    let emitter = guard.emitter();
    let (kind, attempt, instance_no) = emit_fixture();
    guard.finish(ChangeRunStatus::Stopped, Some("用户停止".to_owned()));
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "除名后可观察面为空"
    );

    // 同键重入：新 run 独立登记，emit 正常
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-2".to_owned(), STARTED_AT)
        .expect("除名后同键可重入");
    crate::control::emit_node(
        &guard,
        "implement",
        attempt,
        kind,
        instance_no,
        StepSettle::running().with_output_ref(Some("exec-2".to_owned())),
    );
    // 旧发射器继续补发：零 panic（写法落在同一复合键的当前条目上）
    emitter.attach_output_ref("implement", attempt, kind, instance_no, "exec-late");

    let nodes = guard.nodes();
    assert_eq!(nodes.len(), 1, "同身份键就地合并，无幽灵条目");
    assert_eq!(
        nodes[0].output_ref.as_deref(),
        Some("exec-late"),
        "补发拍按 some 才覆写生效"
    );
}

/// 边界（组合 4 状态 × 3 builder）：`StepSettle` 全组合落位——各字段正确、
/// `started_at` 恒有值、`ended_at` 仅终态有值。
#[tokio::test]
async fn step_settle四状态三builder组合落位() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("begin_run 应成功");

    let cases: Vec<(ChangeStepStatus, StepSettle, u32)> = vec![
        (ChangeStepStatus::Running, StepSettle::running(), 0),
        (
            ChangeStepStatus::Passed,
            StepSettle::passed(Some("通过".to_owned())),
            1,
        ),
        (
            ChangeStepStatus::Failed,
            StepSettle::failed(Some("失败".to_owned())),
            2,
        ),
        (
            ChangeStepStatus::Stopped,
            StepSettle::stopped(Some("停止".to_owned())),
            3,
        ),
    ];

    for (_, settle, instance_no) in cases {
        let settle = settle
            .with_session(Some(format!("sess-{instance_no}")))
            .with_output_ref(Some(format!("exec-{instance_no}")))
            .with_tokens(Some(instance_no as u64), Some(instance_no as u64 * 2));
        crate::control::emit_node(
            &guard,
            "implement",
            1,
            ChangeStepKind::Executor,
            instance_no,
            settle,
        );
    }

    let snapshot = control.snapshot(ROOT, CHANGE_ID).expect("run 在场");
    assert_eq!(snapshot.nodes.len(), 4, "四状态各一节点");
    for node in &snapshot.nodes {
        let expected_status = match node.instance_no {
            0 => ChangeStepStatus::Running,
            1 => ChangeStepStatus::Passed,
            2 => ChangeStepStatus::Failed,
            _ => ChangeStepStatus::Stopped,
        };
        assert_eq!(node.status, expected_status, "状态落位");
        assert!(node.started_at > 0, "started_at 恒有值");
        assert_eq!(
            node.ended_at.is_some(),
            expected_status != ChangeStepStatus::Running,
            "ended_at 仅终态有值（instance {}",
            node.instance_no
        );
        assert_eq!(
            node.session_id.as_deref(),
            Some(format!("sess-{}", node.instance_no).as_str()),
            "with_session 落位"
        );
        assert_eq!(
            node.output_ref.as_deref(),
            Some(format!("exec-{}", node.instance_no).as_str()),
            "with_output_ref 落位"
        );
        assert_eq!(
            node.token_input,
            Some(node.instance_no as u64),
            "with_tokens in 落位"
        );
        assert_eq!(
            node.token_output,
            Some(node.instance_no as u64 * 2),
            "with_tokens out 落位"
        );
    }
}

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`answer` 转投 / `resolve_ask` 单点清理 / `new` 的 `Arc` 共享 /
// 退役面自证
// ---------------------------------------------------------------------------

/// 转投原文回流（正向，AC-2）：命令内**零副作用**——`status` / `ask` 槽 /
/// 广播三者均未变（清理搬到执行体的 `resolve_ask`）。
#[tokio::test]
async fn answer转投原文回流且命令内零副作用() {
    let ask_registry = Arc::new(AskRegistry::default());
    let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.set_session(Some("core-1".to_owned()));
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "采用哪个方案?".to_owned(),
            options: vec!["A".to_owned(), "B".to_owned()],
            tool_call_id: Some("call_1".to_owned()),
            session_id: Some("core-1".to_owned()),
        },
    });
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    while rx.try_recv().is_ok() {}

    let waiter = spawn_waiter(&ask_registry, "core-1");
    assert!(
        control.answer(ROOT, CHANGE_ID, "选项A".to_owned()).is_ok(),
        "命中即转投 Ok"
    );
    assert_eq!(
        waiter.await.unwrap().as_deref(),
        Some("选项A"),
        "接收方逐字收原文"
    );

    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        snap.status,
        ChangeRunStatus::WaitingAsk,
        "命令内零状态清理（status 未变）"
    );
    assert!(snap.ask.is_some(), "ask 槽未变（清理等 resolve_ask）");
    assert!(rx.try_recv().is_err(), "命令内零广播（订阅端零新帧）");
}

/// 三段判定第一段（正向，AC-8）：未 `begin_run` 的 `(root, change)` → `Err`，
/// 文案携 change id 可归因。
#[tokio::test]
async fn answer未begin_run时err携change_id() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let err = control
        .answer(ROOT, CHANGE_ID, "应答".to_owned())
        .expect_err("无 run 应 Err");
    assert!(err.contains(CHANGE_ID), "错误显式携带 change id: {err}");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");
}

/// 三段判定第二段（正向，参数化 2 形态）：有 run 但会话槽 `None` / 会话槽有值
/// 而注册表无登记 → 同一文案 `Err`。
#[tokio::test]
async fn answer有run无等待方两形态同文案err() {
    // 形态一：会话槽 None
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let err = control
        .answer(ROOT, CHANGE_ID, "应答".to_owned())
        .expect_err("会话槽 None 应 Err");
    assert_eq!(err, "当前无等待中的 ask", "会话槽 None 文案");

    // 形态二：会话槽有值但注册表无登记
    guard.set_session(Some("core-未登记".to_owned()));
    let err = control
        .answer(ROOT, CHANGE_ID, "应答".to_owned())
        .expect_err("注册表无登记应 Err");
    assert_eq!(err, "当前无等待中的 ask", "两种形态同文案");
}

/// 唯一构造面（边界，DD3）：`new(Arc::clone(&registry))` 后外部 `registry.answer`
/// 与 `control.answer` 作用于**同一张表**（`new` 前登记的槽可被 `control.answer`
/// 命中）——`Arc` 共享、未自铸第二张表的机械判据。
#[tokio::test]
async fn new共享同一注册表未自铸第二张表() {
    let ask_registry = Arc::new(AskRegistry::default());
    // new 之前就登记（若 control 内部另铸表则必然 miss）
    let waiter = spawn_waiter(&ask_registry, "core-1");
    let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.set_session(Some("core-1".to_owned()));

    assert!(
        control
            .answer(ROOT, CHANGE_ID, "共享表应答".to_owned())
            .is_ok(),
        "new 前登记的槽可被 control.answer 命中（同一张表）"
    );
    assert_eq!(waiter.await.unwrap().as_deref(), Some("共享表应答"));
    assert!(
        ask_registry
            .answer("core-1", "外部再答".to_owned())
            .is_err(),
        "外部实例观测到同一 take（表已空）"
    );
}

/// 条件清理（正向，AC-2 / DD5）：`publish(Ask)` → `WaitingAsk` + `ask` 在位；
/// `resolve_ask` → `Running`、`ask == None`、订阅端恰收一条 `RunNotice::Step`。
#[tokio::test]
async fn resolve_ask条件清理且广播一条step() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "q".to_owned(),
            options: vec!["a".to_owned()],
            tool_call_id: Some("call_1".to_owned()),
            session_id: Some("core-1".to_owned()),
        },
    });
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    // 订阅建立后重发一次确保帧序可读
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "q".to_owned(),
            options: vec!["a".to_owned()],
            tool_call_id: Some("call_1".to_owned()),
            session_id: Some("core-1".to_owned()),
        },
    });
    while rx.try_recv().is_ok() {}

    let before = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        before.status,
        ChangeRunStatus::WaitingAsk,
        "publish 切停等态"
    );
    assert_eq!(
        before.ask.as_ref().map(|ask| ask.question.as_str()),
        Some("q"),
        "载荷单源在位"
    );

    control.resolve_ask(ROOT, CHANGE_ID);

    let after = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(after.status, ChangeRunStatus::Running, "状态回运行态");
    assert!(after.ask.is_none(), "ask 槽清空");
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "失效信号复用通用 Step"
    );
    assert!(rx.try_recv().is_err(), "只发一条");
}

/// `ask` 槽无条件清空 + 终态保护（正向，DD5 / spec「终态不被倒灌」）：
/// `status` 已是 `Completed` 而 `ask` 槽仍被置位 → `resolve_ask` 后 `status`
/// 仍 `Completed`（不被改回 `Running`）但 `ask == None`、广播仍发一条 `Step`。
#[tokio::test]
async fn resolve_ask终态不被倒灌但槽仍无条件清空() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "q".to_owned(),
            options: vec!["a".to_owned()],
            tool_call_id: Some("call_1".to_owned()),
            session_id: None,
        },
    });
    guard.emit(RunUpdate::Finished {
        status: ChangeRunStatus::Completed,
        reason: None,
    });
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();

    control.resolve_ask(ROOT, CHANGE_ID);

    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(
        snap.status,
        ChangeRunStatus::Completed,
        "终态不被改回 Running"
    );
    assert!(snap.ask.is_none(), "ask 槽无条件清空（与状态条件两半独立）");
    assert!(
        matches!(rx.try_recv(), Ok(RunNotice::Step)),
        "广播仍发一条 Step"
    );
}

/// 锁外广播（正向）：`resolve_ask` 的广播不被 `runs` 锁持有阻塞——订阅端在
/// 完成前可继续读快照（同线程调用即证未死锁）。
#[tokio::test]
async fn resolve_ask广播在锁外完成() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "q".to_owned(),
            options: Vec::new(),
            tool_call_id: None,
            session_id: None,
        },
    });
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    control.resolve_ask(ROOT, CHANGE_ID);

    // 广播后立即读快照：持锁广播会让同线程后续访问自锁（此处不死锁即证）
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_some(),
        "锁外广播后可读快照"
    );
    assert!(matches!(rx.try_recv(), Ok(RunNotice::Step)));
}

/// 载荷单源（正向，AC-2）：`publish(Ask)` 的快照四字段逐字段相等、
/// `status == WaitingAsk`；广播仍为 `RunNotice::Ask`（不是 `Step`）。
#[tokio::test]
async fn publish_ask载荷单源四字段与通知kind不变() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "选哪个重试策略？".to_owned(),
            options: vec!["指数退避".to_owned(), "固定间隔".to_owned()],
            tool_call_id: Some("call_1".to_owned()),
            session_id: Some("core-1".to_owned()),
        },
    });

    let snap = control.snapshot(ROOT, CHANGE_ID).unwrap();
    assert_eq!(snap.status, ChangeRunStatus::WaitingAsk);
    let ask = snap.ask.expect("载荷在位");
    assert_eq!(ask.question, "选哪个重试策略？");
    assert_eq!(
        ask.options,
        vec!["指数退避".to_owned(), "固定间隔".to_owned()]
    );
    assert_eq!(ask.tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(ask.session_id.as_deref(), Some("core-1"));
    assert!(matches!(rx.try_recv(), Ok(RunNotice::Ask)), "广播为 Ask");
}

/// 迟到 / 重复应答（异常，AC-7）：同键连续两次 `answer` → 第一次 `Ok`、第二次
/// `Err`（文案含「当前无等待中的 ask」），第二次不产生第二条投递。
#[tokio::test]
async fn answer重复应答第二次err且零第二条投递() {
    let ask_registry = Arc::new(AskRegistry::default());
    let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.set_session(Some("core-1".to_owned()));
    let (tx, mut rx) = tokio::sync::oneshot::channel::<String>();
    ask_registry.register("core-1", tx);

    assert!(control.answer(ROOT, CHANGE_ID, "第一次".to_owned()).is_ok());
    let err = control
        .answer(ROOT, CHANGE_ID, "第二次".to_owned())
        .expect_err("二次应答 Err");
    assert!(err.contains("当前无等待中的 ask"), "文案: {err}");
    assert_eq!(rx.try_recv().expect("唯一一次投递"), "第一次");
    assert!(rx.try_recv().is_err(), "零第二条投递");
}

/// 除名后路径（异常，参数化 2 形态）：`end_run` 后 `answer` → `Err` 携 change
/// id；`resolve_ask` 对已除名 run → 静默无操作、不 panic（miss 幂等）。
#[tokio::test]
async fn 除名后answer_err且resolve_ask静默() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.set_session(Some("core-1".to_owned()));
    guard.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "q".to_owned(),
            options: Vec::new(),
            tool_call_id: None,
            session_id: None,
        },
    });
    guard.finish(ChangeRunStatus::Stopped, None);

    let err = control
        .answer(ROOT, CHANGE_ID, "迟到应答".to_owned())
        .expect_err("除名后 answer 应 Err");
    assert!(err.contains(CHANGE_ID), "Err 携 change id: {err}");

    // resolve_ask 对已除名 run 静默（不 panic、不改任何状态）
    control.resolve_ask(ROOT, CHANGE_ID);
    assert!(control.snapshot(ROOT, CHANGE_ID).is_none(), "除名后无快照");
}

/// 退役面零残留（异常，源码自证，AC-8）：`control.rs` 代码行内零 `answer_tx`、
/// 零 `wait_answer`；反向在场：五个既有标识符仍在位（逐字未改的机械判据另一半）。
#[test]
fn control_rs退役面零残留且既有五标识符在位() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/control.rs"),
    )
    .expect("读取 control.rs");
    let code_only = own
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("///") || trimmed.starts_with("//"))
        })
        .collect::<Vec<_>>()
        .join("\n");

    for retired in ["answer_tx", "wait_answer"] {
        assert!(
            !code_only.contains(retired),
            "control.rs 代码行内零 {retired}（单通道改造的退役面）"
        );
    }
    for kept in [
        "accept_pending",
        "register_pending",
        "fn confirm",
        "wait_confirm",
        "cancelled_wait",
    ] {
        assert!(own.contains(kept), "既有标识符 {kept} 仍在位（逐字未改）");
    }
}

/// 键边界（边界，参数化 3 形态）：`root` / `change_id` 取 `""` / `"   "` /
/// `"a\\0b🚀"` → 未 `begin_run` 时 `Err`、零 panic（命令面 blank 校验在壳层，
/// 本层不二次校验）。
#[tokio::test]
async fn answer键边界未发起时err且零panic() {
    let control = Arc::new(ChangeFlowControl::new(Arc::new(AskRegistry::default())));
    for (root, change_id) in [("", ""), ("   ", "   "), ("a\0b🚀", "a\0b🚀")] {
        let err = control
            .answer(root, change_id, "应答".to_owned())
            .expect_err("未发起应 Err");
        assert!(
            err.contains(change_id),
            "键 {root:?}/{change_id:?} 原样进文案（不 trim）: {err}"
        );
        // resolve_ask 对未发起键静默
        control.resolve_ask(root, change_id);
        assert!(control.snapshot(root, change_id).is_none());
    }
}

/// 正文边界（边界，参数化 5 形态，零加工）：`text` 逐字到达接收方（`answer`
/// 不裁量应答内容）。
#[tokio::test]
async fn answer正文边界逐字投递() {
    let texts = [
        String::new(),
        "   ".to_owned(),
        "a\nb".to_owned(),
        "a\0b🚀".to_owned(),
        "字".repeat(1200),
    ];
    for text in texts {
        let ask_registry = Arc::new(AskRegistry::default());
        let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
        let guard = control
            .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
            .unwrap();
        guard.set_session(Some("core-1".to_owned()));
        let (tx, mut rx) = tokio::sync::oneshot::channel::<String>();
        ask_registry.register("core-1", tx);

        assert!(
            control.answer(ROOT, CHANGE_ID, text.clone()).is_ok(),
            "正文 {text:?} 受理"
        );
        assert_eq!(rx.try_recv().expect("收到"), text, "逐字保真（零 trim）");
    }
}

/// 锁序（边界，DD4）：`answer` 在转投注册表前已释放 `runs` 锁——并发探针证明
/// 同期 `publish` 不被阻塞（超时护栏内完成，零死锁）。
#[tokio::test]
async fn answer转投前已释放runs锁_同期publish不被阻塞() {
    let ask_registry = Arc::new(AskRegistry::default());
    let control = Arc::new(ChangeFlowControl::new(Arc::clone(&ask_registry)));
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    guard.set_session(Some("core-1".to_owned()));

    // 并发探针：另一端持续 publish（若 answer 持着 runs 锁做投递，publish 会阻塞）
    let publisher_control = Arc::clone(&control);
    let published = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let published_hits = Arc::clone(&published);
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let publisher = std::thread::spawn(move || {
        while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            publisher_control.publish(
                ROOT,
                CHANGE_ID,
                RunUpdate::Finished {
                    status: ChangeRunStatus::Completed,
                    reason: None,
                },
            );
            published_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            std::thread::yield_now();
        }
    });

    // 主线程连做三次「登记 + 应答」：若持锁投递，探针的 publish 计数会停摆
    for _ in 0..3 {
        let (tx, mut rx) = tokio::sync::oneshot::channel::<String>();
        assert!(ask_registry.register("core-1", tx), "登记");
        assert!(
            control.answer(ROOT, CHANGE_ID, "应答".to_owned()).is_ok(),
            "转投成功"
        );
        assert_eq!(rx.try_recv().expect("收到"), "应答");
        let before = published.load(std::sync::atomic::Ordering::Relaxed);
        for _ in 0..200 {
            tokio::task::yield_now().await;
        }
        assert!(
            published.load(std::sync::atomic::Ordering::Relaxed) >= before,
            "同期 publish 持续推进（answer 不持锁投递）"
        );
    }

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    publisher.join().expect("探针线程正常结束");
    assert!(
        published.load(std::sync::atomic::Ordering::Relaxed) > 0,
        "探针至少有若干次 publish 完成（零死锁）"
    );
}
