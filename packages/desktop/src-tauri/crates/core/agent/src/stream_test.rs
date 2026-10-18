use std::sync::Arc;

use crate::event::{AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole};
use crate::port::TurnOutcome;
use crate::runner::AgentRunStatus;
use crate::stream::{
    SessionStreamItem, SessionStreamRecvError, SessionStreamRegistry, SessionStreamSubscription,
};

// ---------------------------------------------------------------------------
// 装置：真实 broadcast / 真实注册表（零内部 mock）；「仍等待 / 未投递」以
// tokio::task::spawn 投递 recv future + yield_now 轮询 is_finished 断言
//（不引入 tokio time 特性）。
// ---------------------------------------------------------------------------

/// 生产实现的 per-session broadcast 容量（模块私有常量，经行为断言锁定）：
/// 恰此条数未消费不 lagged，再多一条即 lagged。
const CAPACITY: usize = 4096;

fn outcome(status: AgentRunStatus) -> TurnOutcome {
    TurnOutcome {
        turn_id: 7,
        status,
        started_at: 1_727_000_000_000,
        finished_at: 1_727_000_005_000,
        num_turns: Some(2),
        duration_ms: Some(5_000),
        cost_usd: Some(0.25),
        usage: serde_json::json!({ "input_tokens": 10, "output_tokens": 20 }),
        error: None,
        remote_session_id: Some("sdk-9-1727000000009".to_owned()),
    }
}

fn sealed_event(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    )
}

fn delta_event(seq: u64, text: &str) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: None,
            delta: AgentDelta::Text {
                text: text.to_owned(),
            },
        },
    )
}

/// 摘 `Event` 变体（断流或非 Event 即 panic：用例断言面即变体判别）。
fn event_of(result: Result<SessionStreamItem, SessionStreamRecvError>) -> AgentEvent {
    match result.expect("应返回条目（非断流）") {
        SessionStreamItem::Event(event) => event,
        SessionStreamItem::Finished(other) => {
            panic!("期望 Event 变体，实际为 Finished({other:?})")
        }
    }
}

/// 摘 `Finished` 变体载荷（断流或非 Finished 即 panic）。
fn finished_of(result: Result<SessionStreamItem, SessionStreamRecvError>) -> TurnOutcome {
    match result.expect("应返回条目（非断流）") {
        SessionStreamItem::Finished(outcome) => outcome,
        SessionStreamItem::Event(event) => {
            panic!("期望 Finished 变体，实际为 Event(seq={})", event.seq)
        }
    }
}

/// 订阅句柄（条目在位为前提；None 即用例前提被破坏，直接 panic）。
fn subscribe(
    registry: &SessionStreamRegistry,
    session_id: &str,
    since_seq: u64,
) -> SessionStreamSubscription {
    registry
        .subscribe(session_id, since_seq)
        .unwrap_or_else(|| panic!("会话 {session_id:?} 应可订阅（条目在位）"))
}

/// 「仍等待」断言：投递 recv future 后多轮让出，任务未完成即视为未投递。
async fn assert_still_waiting(
    handle: &tokio::task::JoinHandle<Result<SessionStreamItem, SessionStreamRecvError>>,
) {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    assert!(
        !handle.is_finished(),
        "不合格条目应被丢弃后继续等待（不 Err、不返回）"
    );
}

// ---------------------------------------------------------------------------
// new / Default：空表语义
// ---------------------------------------------------------------------------

#[test]
fn new与default均为空表_任意id订阅为none且begin后方some() {
    let fresh = SessionStreamRegistry::new();
    let defaulted = SessionStreamRegistry::default();
    let long_key = "超长会话键".repeat(200);

    for session_id in [
        "",
        long_key.as_str(),
        "特殊\u{0}字符😀\n会话",
        "ses-1",
    ] {
        assert!(
            fresh.subscribe(session_id, 0).is_none(),
            "new() 空表对任意 id（含空串 / 超长 / 特殊字符）订阅为 None"
        );
        assert!(
            defaulted.subscribe(session_id, 0).is_none(),
            "Default 与 new 同语义（空表）"
        );
    }

    fresh.begin("ses-1");
    assert!(fresh.subscribe("ses-1", 0).is_some(), "begin 后即可订阅");
    assert!(
        fresh.subscribe("ses-2", 0).is_none(),
        "仅登记键可订阅（空表语义按条目）"
    );
}

// ---------------------------------------------------------------------------
// begin：登记即订阅 + 覆盖语义 + 键边界 + 多会话隔离
// ---------------------------------------------------------------------------

#[tokio::test]
async fn begin登记后订阅返some且publish事件可被该句柄收取() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-1");
    let mut subscription = subscribe(&registry, "ses-1", 0);

    registry.publish("ses-1", sealed_event(1));

    assert_eq!(
        event_of(subscription.recv().await).seq,
        1,
        "登记即可订阅（live 事件直投）"
    );
}

#[tokio::test]
async fn begin同键重复登记覆盖旧条目_旧句柄收closed且新句柄收覆盖后事件() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-1");
    let mut old_subscription = subscribe(&registry, "ses-1", 0);

    // 同键覆盖（无重入守卫）：旧 sender 释放 → 旧句柄 Closed
    registry.begin("ses-1");
    assert!(
        registry.subscribe("ses-1", 0).is_some(),
        "覆盖后条目仍在（新通道）"
    );
    let error = old_subscription
        .recv()
        .await
        .expect_err("旧句柄的 sender 释放后应 Closed");
    assert_eq!(error, SessionStreamRecvError::Closed, "覆盖后旧句柄断流");

    // 新句柄只收覆盖后的新事件（旧条目零残留）
    let mut new_subscription = subscribe(&registry, "ses-1", 0);
    registry.publish("ses-1", sealed_event(9));
    assert_eq!(
        event_of(new_subscription.recv().await).seq,
        9,
        "新句柄收覆盖后的新事件"
    );
}

#[tokio::test]
async fn begin键边界_空串超长与特殊字符均可登记与寻址且不panic() {
    let registry = SessionStreamRegistry::new();
    let keys = [
        "".to_owned(),
        "超".repeat(1_100),
        "特殊\u{0}字符😀\n会话\t".to_owned(),
    ];

    for (index, key) in keys.iter().enumerate() {
        registry.begin(key);
        // 先订阅再发布：broadcast 只投订阅后的 live 事件
        let mut subscription = subscribe(&registry, key, 0);
        registry.publish(key, sealed_event(index as u64 + 1));
        assert_eq!(
            event_of(subscription.recv().await).seq,
            index as u64 + 1,
            "键 {index} 逐字保真登记与寻址（HashMap 键不解释，事件不串台）"
        );
    }
}

#[tokio::test]
async fn begin两会话互不串台_publish到a时b句柄仍等待且a收一条() {
    let registry = Arc::new(SessionStreamRegistry::new());
    registry.begin("ses-a");
    registry.begin("ses-b");
    let mut sub_a = subscribe(&registry, "ses-a", 0);
    let sub_b = subscribe(&registry, "ses-b", 0);

    let waiting_b = tokio::task::spawn(async move {
        let mut sub_b = sub_b;
        sub_b.recv().await
    });

    registry.publish("ses-a", sealed_event(1));
    assert_still_waiting(&waiting_b).await;

    assert_eq!(
        event_of(sub_a.recv().await).seq,
        1,
        "A 句柄收 A 条目的一条事件"
    );
    assert!(!waiting_b.is_finished(), "B 句柄零投递（多会话隔离）");
    waiting_b.abort();
}

// ---------------------------------------------------------------------------
// publish：投递 / 无条目 / 无订阅者
// ---------------------------------------------------------------------------

#[tokio::test]
async fn publish连续事件按序投递_seq严格递增且载荷逐字相等() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-1");
    let mut subscription = subscribe(&registry, "ses-1", 0);

    let delta = delta_event(1, "第一片 🎉");
    let sealed = sealed_event(2);
    let notice = AgentEvent::stamp(
        3,
        AgentEventKind::SystemNotice {
            subtype: "api_retry".to_owned(),
            payload: serde_json::json!({ "attempt": 2 }),
        },
    );
    registry.publish("ses-1", delta.clone());
    registry.publish("ses-1", sealed.clone());
    registry.publish("ses-1", notice.clone());

    let first = event_of(subscription.recv().await);
    let second = event_of(subscription.recv().await);
    let third = event_of(subscription.recv().await);
    assert_eq!(
        (first.seq, second.seq, third.seq),
        (1, 2, 3),
        "seq 严格递增"
    );
    assert_eq!(first, delta, "delta 载荷直投不解释");
    assert_eq!(second, sealed);
    assert_eq!(third, notice);
}

#[tokio::test]
async fn publish无条目时静默noop_事后订阅收不到该历史事件() {
    let registry = SessionStreamRegistry::new();

    // 未 begin：静默 no-op 不 panic
    registry.publish("ses-404", sealed_event(1));
    registry.publish("", sealed_event(2));
    registry.publish("ses-1", sealed_event(3));

    // 事后 begin：订阅者收不到 begin 之前的事件（零残留）
    registry.begin("ses-1");
    let mut subscription = subscribe(&registry, "ses-1", 0);
    registry.publish("ses-1", sealed_event(4));
    assert_eq!(
        event_of(subscription.recv().await).seq,
        4,
        "begin 前的事件不回溯（条目为登记后才建的通道）"
    );

    // finish 除名后再 publish：静默 no-op（不因除名而 panic）
    registry.finish("ses-1", outcome(AgentRunStatus::Completed));
    registry.publish("ses-1", sealed_event(5));
    assert!(
        registry.subscribe("ses-1", 0).is_none(),
        "除名后 publish 不重建条目"
    );
}

#[tokio::test]
async fn publish有条目无订阅者时静默丢弃_事后订阅只收live事件() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-1");

    // 无订阅者（send 返回 Err 被吞）：不 panic
    registry.publish("ses-1", sealed_event(1));
    registry.publish("ses-1", delta_event(2, "丢弃的增量"));

    let mut subscription = subscribe(&registry, "ses-1", 0);
    registry.publish("ses-1", sealed_event(3));
    let received = event_of(subscription.recv().await);
    assert_eq!(
        received.seq, 3,
        "broadcast 不回溯历史：订阅后只收 live 事件"
    );
}

// ---------------------------------------------------------------------------
// finish：终态投递 + 除名 + no-op + 载荷逐字透传
// ---------------------------------------------------------------------------

#[tokio::test]
async fn finish先投终态后除名_事件与终态依序可得且字段逐字() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-1");
    let mut subscription = subscribe(&registry, "ses-1", 0);

    let expected = outcome(AgentRunStatus::Failed);
    // since_seq = 0 为「已观测最大 seq」上界：投递者自 seq 1 起（seq 0 被过滤）
    registry.publish("ses-1", sealed_event(1));
    registry.finish("ses-1", expected.clone());

    let first = event_of(subscription.recv().await);
    let terminal = finished_of(subscription.recv().await);
    assert_eq!(first.seq, 1, "事件先于终态");
    assert_eq!(terminal, expected, "终态载荷逐字段透传");
    assert_eq!(
        terminal.status,
        AgentRunStatus::Failed,
        "status 逐字（收口形状自足）"
    );
    assert_eq!(terminal.started_at, 1_727_000_000_000);
    assert_eq!(terminal.finished_at, 1_727_000_005_000);
    assert_eq!(terminal.turn_id, 7);
}

#[tokio::test]
async fn finish投终态即除名_再订阅none且既有句柄排空后closed() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-1");
    let mut subscription = subscribe(&registry, "ses-1", 0);

    registry.finish("ses-1", outcome(AgentRunStatus::Completed));
    assert!(
        registry.subscribe("ses-1", 0).is_none(),
        "投终态即除名（新订阅 None）"
    );

    let terminal = finished_of(subscription.recv().await);
    assert_eq!(terminal.status, AgentRunStatus::Completed);
    let error = subscription
        .recv()
        .await
        .expect_err("排空缓冲后 Closed（条目移除的自然收尾）");
    assert_eq!(error, SessionStreamRecvError::Closed);
}

#[test]
fn finish无条目时静默noop_不产生伪终态() {
    let registry = SessionStreamRegistry::new();

    // 未 begin
    registry.finish("ses-404", outcome(AgentRunStatus::Completed));
    // 重复 finish（首次已除名）
    registry.begin("ses-1");
    registry.finish("ses-1", outcome(AgentRunStatus::Completed));
    registry.finish("ses-1", outcome(AgentRunStatus::Failed));

    assert!(
        registry.subscribe("ses-1", 0).is_none(),
        "重复 finish 不重建条目（无伪终态）"
    );
}

#[tokio::test]
async fn finish终态两极透传_started_at极值与error两形态不归一() {
    let registry = SessionStreamRegistry::new();

    for started_at in [0_i64, -1, i64::MAX, i64::MIN] {
        registry.begin("ses-extreme");
        let mut subscription = subscribe(&registry, "ses-extreme", 0);
        let payload = TurnOutcome {
            started_at,
            error: Some("事件落库失败: db: 注入".to_owned()),
            ..outcome(AgentRunStatus::Failed)
        };
        registry.finish("ses-extreme", payload);

        let terminal = finished_of(subscription.recv().await);
        assert_eq!(
            terminal.started_at, started_at,
            "started_at 逐字透传不归一（实际: {terminal:?}）"
        );
        assert_eq!(
            terminal.error.as_deref(),
            Some("事件落库失败: db: 注入"),
            "error Some 形态逐字"
        );

        // error None 形态（Stopped 收敛不记因）
        registry.begin("ses-extreme");
        let mut subscription = subscribe(&registry, "ses-extreme", 0);
        registry.finish("ses-extreme", outcome(AgentRunStatus::Stopped));
        assert_eq!(
            finished_of(subscription.recv().await).error,
            None,
            "error None 形态保持 None"
        );
    }
}

// ---------------------------------------------------------------------------
// subscribe：幂等判据 + since_seq 三值 + seq 极值
// ---------------------------------------------------------------------------

#[test]
fn subscribe未知已除名与blank_id全部none() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-live");
    registry.finish("ses-live", outcome(AgentRunStatus::Completed));

    for session_id in ["ses-404", "ses-live", "", "   ", "特殊\u{0}😀"] {
        assert!(
            registry.subscribe(session_id, 0).is_none(),
            "无运行轮（未知 / 已除名 / blank id）一律 None（幂等立即结束判据）"
        );
    }
}

#[tokio::test]
async fn subscribe_since_seq三值_零全投_已发最大不重投_max只等终态() {
    let registry = SessionStreamRegistry::new();

    // since_seq = 0：全投
    registry.begin("ses-zero");
    let mut zero = subscribe(&registry, "ses-zero", 0);
    registry.publish("ses-zero", sealed_event(1));
    assert_eq!(event_of(zero.recv().await).seq, 1, "since_seq=0 全投");

    // since_seq = 已发最大 seq：旧事件全丢、新事件照投
    registry.begin("ses-max");
    let mut at_max = subscribe(&registry, "ses-max", 5);
    registry.publish("ses-max", sealed_event(5));
    registry.publish("ses-max", sealed_event(6));
    assert_eq!(
        event_of(at_max.recv().await).seq,
        6,
        "since_seq=已发最大时旧事件不重投、严格更大者照投"
    );

    // since_seq = u64::MAX：事件永不投递，终态仍恒投
    registry.begin("ses-cap");
    let mut cap = subscribe(&registry, "ses-cap", u64::MAX);
    registry.publish("ses-cap", sealed_event(u64::MAX));
    registry.finish("ses-cap", outcome(AgentRunStatus::Completed));
    let terminal = finished_of(cap.recv().await);
    assert_eq!(
        terminal.status,
        AgentRunStatus::Completed,
        "since_seq=u64::MAX 时事件全滤，终态恒投"
    );
}

#[tokio::test]
async fn subscribe_seq极值事件_u64_max不溢出且按since_seq严格比较() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-edge");

    // since_seq = u64::MAX：seq = u64::MAX 的事件不投（严格大于才投）
    let mut at_max = subscribe(&registry, "ses-edge", u64::MAX);
    registry.publish("ses-edge", sealed_event(u64::MAX));
    registry.finish("ses-edge", outcome(AgentRunStatus::Completed));
    assert_eq!(
        finished_of(at_max.recv().await).status,
        AgentRunStatus::Completed,
        "since_seq=u64::MAX 时 u64::MAX 事件被丢弃（比较不溢出）"
    );

    // since_seq = u64::MAX - 1：同一事件投递
    registry.begin("ses-edge");
    let mut below = subscribe(&registry, "ses-edge", u64::MAX - 1);
    registry.publish("ses-edge", sealed_event(u64::MAX));
    assert_eq!(
        event_of(below.recv().await).seq,
        u64::MAX,
        "since_seq=u64::MAX-1 时 u64::MAX 事件投递"
    );
}

// ---------------------------------------------------------------------------
// recv：过滤后继续等待 / lagged / closed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn recv过滤不合格条目后继续等待而不err() {
    let registry = Arc::new(SessionStreamRegistry::new());
    registry.begin("ses-1");
    let mut subscription = subscribe(&registry, "ses-1", 2);

    let publisher = Arc::clone(&registry);
    let waiting = tokio::task::spawn(async move { subscription.recv().await });

    publisher.publish("ses-1", sealed_event(1));
    publisher.publish("ses-1", sealed_event(2));
    assert_still_waiting(&waiting).await;

    publisher.publish("ses-1", sealed_event(3));
    let received = waiting.await.expect("任务正常结束");
    assert_eq!(
        event_of(received).seq,
        3,
        "丢弃不合格条目后继续等待，命中即返回"
    );
}

#[tokio::test]
async fn recv订阅不消费而溢出时lagged且不伪装终态() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-1");
    let mut subscription = subscribe(&registry, "ses-1", 0);

    for seq in 0..=(CAPACITY as u64) {
        registry.publish("ses-1", sealed_event(seq));
    }

    match subscription.recv().await {
        Err(SessionStreamRecvError::Lagged(skipped)) => {
            assert!(skipped >= 1, "溢出断流信号携丢弃条数，实际: {skipped}");
        }
        other => panic!("溢出应 Lagged，实际: {other:?}"),
    }
}

#[tokio::test]
async fn recv容量边界_恰容量不lag且超一条即lag() {
    // 恰 CAPACITY 条未消费：仍可逐条消费（不 lagged）
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-exact");
    let mut exact = subscribe(&registry, "ses-exact", 0);
    for seq in 1..=(CAPACITY as u64) {
        registry.publish("ses-exact", sealed_event(seq));
    }
    for seq in 1..=(CAPACITY as u64) {
        assert_eq!(
            event_of(exact.recv().await).seq,
            seq,
            "恰容量未消费仍逐条可得（第 {seq} 条）"
        );
    }

    // 第 CAPACITY + 1 条未消费即 lagged
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-over");
    let mut over = subscribe(&registry, "ses-over", 0);
    for seq in 1..=(CAPACITY as u64 + 1) {
        registry.publish("ses-over", sealed_event(seq));
    }
    assert!(
        matches!(over.recv().await, Err(SessionStreamRecvError::Lagged(_))),
        "超出容量一条即 lagged（SESSION_STREAM_CAPACITY 行为边界）"
    );
}

#[tokio::test]
async fn recv_closed与lagged互不混同() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-closed");
    let mut closed = subscribe(&registry, "ses-closed", 0);
    registry.finish("ses-closed", outcome(AgentRunStatus::Completed));
    assert!(matches!(
        closed.recv().await,
        Ok(SessionStreamItem::Finished(_))
    ));
    let closed_error = closed.recv().await.expect_err("排空后 Closed");
    assert_eq!(
        closed_error,
        SessionStreamRecvError::Closed,
        "Closed 变体（条目除名后的断流原因）"
    );

    let registry = SessionStreamRegistry::new();
    registry.begin("ses-lagged");
    let mut lagged = subscribe(&registry, "ses-lagged", 0);
    for seq in 0..=(CAPACITY as u64) {
        registry.publish("ses-lagged", sealed_event(seq));
    }
    let lagged_error = lagged.recv().await.expect_err("溢出 Lagged");
    assert!(
        matches!(lagged_error, SessionStreamRecvError::Lagged(_)),
        "Lagged 变体（溢出断流原因），实际: {lagged_error:?}"
    );
    assert_ne!(lagged_error, closed_error, "两变体互不相等");
}

#[test]
fn recv_error变体契约_copy_clone与部分相等() {
    let lagged = SessionStreamRecvError::Lagged(3);
    let copied = lagged; // Copy：不吞载荷
    let cloned = lagged.clone();
    assert_eq!(lagged, copied, "Copy 后可重复断言丢弃条数");
    assert_eq!(lagged, cloned, "Clone 保留载荷");
    assert_eq!(
        SessionStreamRecvError::Closed,
        SessionStreamRecvError::Closed
    );
    assert_ne!(
        SessionStreamRecvError::Lagged(3),
        SessionStreamRecvError::Lagged(4),
        "丢弃条数参与相等性"
    );
    assert_ne!(lagged, SessionStreamRecvError::Closed, "两变体互不相等");
}

// ---------------------------------------------------------------------------
// 多订阅者扇出：互不吞噬，单句柄释放不影响其余
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 多订阅者扇出各收一份且单句柄释放不影响其余() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-fan");
    let mut first = subscribe(&registry, "ses-fan", 0);
    let mut second = subscribe(&registry, "ses-fan", 0);
    let mut third = subscribe(&registry, "ses-fan", 0);

    registry.publish("ses-fan", sealed_event(1));
    for subscription in [&mut first, &mut second, &mut third] {
        assert_eq!(
            event_of(subscription.recv().await).seq,
            1,
            "三个句柄各收一份（互不吞噬）"
        );
    }

    drop(second);
    registry.publish("ses-fan", sealed_event(2));
    assert_eq!(event_of(first.recv().await).seq, 2, "句柄释放不阻塞投递");
    assert_eq!(event_of(third.recv().await).seq, 2, "其余句柄继续收");

    registry.finish("ses-fan", outcome(AgentRunStatus::Completed));
    assert_eq!(
        finished_of(first.recv().await).status,
        AgentRunStatus::Completed
    );
    assert_eq!(
        finished_of(third.recv().await).status,
        AgentRunStatus::Completed
    );
}
