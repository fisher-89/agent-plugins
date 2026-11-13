use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::ask::AskRegistry;
use crate::event::{AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole};
use crate::kernel::{KernelOutput, SessionKernel, StopRegistry, TurnRequest};
use crate::port::{SessionSink, TurnOutcome};
use crate::runner::{
    AgentPermissionMode, AgentRunStatus, AgentRunner, AgentSession, AgentStartError, ModelLevel,
    RunHandle, SessionCtx, SessionInjections, SessionOpen, SessionRef, TurnQuestion,
};
use crate::session::{NewSessionRow, SessionProvenance};
use crate::steer::SteerRegistry;
use crate::stream::{SessionStreamItem, SessionStreamRegistry, SessionStreamSubscription};

// ---------------------------------------------------------------------------
// 装置：假 sink（调用序 + 可编程失败）、假 runner（预录观察 + 可编程 open Err）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum SinkCall {
    CreateSession(NewSessionRow),
    BeginTurn {
        session_id: String,
        started_at: i64,
    },
    AppendSealed {
        session_id: String,
        seq: u64,
    },
    FinishTurn {
        turn_id: i64,
        status: AgentRunStatus,
    },
    BindRemote {
        session_id: String,
        remote: String,
        updated_at: i64,
    },
}

/// 假 sink：共享调用记录，begin_turn 返回自增轮 id（sink 分配序），可编程
/// 失败点（`append_sealed` 第 n 次调用起失败等，按方法名 + 序号指定）。
/// `next_seq` 按会话追踪已落库最大 seq（镜像 store 续排基点语义）。
struct FakeSink {
    calls: Arc<Mutex<Vec<SinkCall>>>,
    next_turn_id: Mutex<i64>,
    seq_high_water: Mutex<HashMap<String, u64>>,
    append_failures_from: Mutex<Option<usize>>,
    create_session_fails: bool,
    begin_turn_fails: bool,
}

impl FakeSink {
    fn new() -> (Self, Arc<Mutex<Vec<SinkCall>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
                next_turn_id: Mutex::new(1),
                seq_high_water: Mutex::new(HashMap::new()),
                append_failures_from: Mutex::new(None),
                create_session_fails: false,
                begin_turn_fails: false,
            },
            calls,
        )
    }

    fn record(&self, call: SinkCall) -> Result<(), String> {
        self.calls.lock().expect("记录锁不可中毒").push(call);
        Ok(())
    }
}

impl SessionSink for FakeSink {
    fn create_session(&self, session: &NewSessionRow) -> Result<(), String> {
        self.record(SinkCall::CreateSession(session.clone()))?;
        if self.create_session_fails {
            return Err("会话行写入失败".to_owned());
        }
        Ok(())
    }

    fn begin_turn(&self, session_id: &str, started_at: i64) -> Result<i64, String> {
        self.record(SinkCall::BeginTurn {
            session_id: session_id.to_owned(),
            started_at,
        })?;
        if self.begin_turn_fails {
            return Err("轮行写入失败".to_owned());
        }
        let mut next = self.next_turn_id.lock().expect("轮 id 锁不可中毒");
        let id = *next;
        *next += 1;
        Ok(id)
    }

    fn next_seq(&self, session_id: &str) -> Result<u64, String> {
        let high_water = self.seq_high_water.lock().expect("高水位锁不可中毒");
        Ok(match high_water.get(session_id) {
            Some(max) => max.saturating_add(1),
            None => 0,
        })
    }

    fn append_sealed(&self, session_id: &str, event: &AgentEvent) -> Result<(), String> {
        self.record(SinkCall::AppendSealed {
            session_id: session_id.to_owned(),
            seq: event.seq,
        })?;
        {
            let mut high_water = self.seq_high_water.lock().expect("高水位锁不可中毒");
            let entry = high_water.entry(session_id.to_owned()).or_insert(0);
            if event.seq > *entry {
                *entry = event.seq;
            }
        }
        let failures = self.append_failures_from.lock().expect("失败锁不可中毒");
        if let Some(from) = *failures {
            let count = self
                .calls
                .lock()
                .expect("记录锁不可中毒")
                .iter()
                .filter(|call| matches!(call, SinkCall::AppendSealed { .. }))
                .count();
            if count >= from {
                return Err(format!("追加第 {count} 条失败"));
            }
        }
        Ok(())
    }

    fn finish_turn(&self, turn_id: i64, outcome: &TurnOutcome) -> Result<(), String> {
        self.record(SinkCall::FinishTurn {
            turn_id,
            status: outcome.status,
        })
    }

    fn bind_remote_session(
        &self,
        session_id: &str,
        remote: &str,
        updated_at: i64,
    ) -> Result<(), String> {
        self.record(SinkCall::BindRemote {
            session_id: session_id.to_owned(),
            remote: remote.to_owned(),
            updated_at,
        })
    }
}

/// 假 runner：open 入参捕获 + 每次成功 open 产出可从测试侧投喂的观察通道
/// （容量 = 观察洪峰背压断言的通道口径）+ 问题接收端持有任务（ask 语义，
/// 测试侧 drop 前恒活）。
struct FakeRunner {
    captured: Mutex<Vec<SessionOpen>>,
    failure: Option<AgentStartError>,
    capacity: usize,
    feeders: Mutex<Vec<tokio::sync::mpsc::Sender<AgentEventKind>>>,
    /// 补充说明接收入端（持有即「本轮在跑、可送达」；`close_steer_stream`
    /// 单边 drop 即「轮在收口前被抛弃」）
    steer_receivers: Mutex<Vec<tokio::sync::mpsc::UnboundedReceiver<crate::steer::SteerMessage>>>,
    _holders: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl FakeRunner {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
            failure: None,
            capacity,
            feeders: Mutex::new(Vec::new()),
            steer_receivers: Mutex::new(Vec::new()),
            _holders: Mutex::new(Vec::new()),
        }
    }

    fn failing(failure: AgentStartError) -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
            failure: Some(failure),
            capacity: 16,
            feeders: Mutex::new(Vec::new()),
            steer_receivers: Mutex::new(Vec::new()),
            _holders: Mutex::new(Vec::new()),
        }
    }

    /// 投喂未盖戳观察事件（内核盖戳治理的引擎侧半边）。
    async fn feed(&self, events: Vec<AgentEventKind>) {
        let feeder = self
            .feeders
            .lock()
            .expect("投喂锁不可中毒")
            .last()
            .cloned()
            .expect("先 open_session 再投喂");
        for event in events {
            feeder.send(event).await.expect("投喂观察事件");
        }
    }

    /// 关闭观察流（EOF；消费端收尾时序的引擎侧单边决定）。
    fn close_stream(&self) {
        self.feeders.lock().expect("投喂锁不可中毒").pop();
    }

    /// 关闭补充说明接收入端（轮被抛弃的引擎侧半边：其后 `request` 走 `send`
    /// 失败臂）。
    #[allow(dead_code)]
    fn close_steer_stream(&self) {
        self.steer_receivers
            .lock()
            .expect("steer 接收锁不可中毒")
            .pop();
    }

    /// 取出最近一轮的接收入端（逐条送达的观测半边）。
    fn take_steer_receiver(
        &self,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<crate::steer::SteerMessage>> {
        self.steer_receivers
            .lock()
            .expect("steer 接收锁不可中毒")
            .pop()
    }
}

impl AgentRunner for FakeRunner {
    fn open_session(&self, open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        self.captured
            .lock()
            .expect("捕获锁不可中毒")
            .push(open.clone());
        if let Some(failure) = self.failure.clone() {
            return Err(failure);
        }
        let (observation_tx, observation_rx) = tokio::sync::mpsc::channel(self.capacity.max(1));
        let (question_tx, mut question_rx) = tokio::sync::mpsc::channel::<TurnQuestion>(4);
        // 问题接收端持有任务：保持 ask 通道打开（问题投递恒成功），句柄置位
        // 或通道关闭时退出
        let holder_handle =
            tokio::spawn(async move { while question_rx.recv().await.is_some() {} });
        self.feeders
            .lock()
            .expect("投喂锁不可中毒")
            .push(observation_tx);
        self._holders
            .lock()
            .expect("持有锁不可中毒")
            .push(holder_handle);
        let (steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel();
        self.steer_receivers
            .lock()
            .expect("steer 接收锁不可中毒")
            .push(steer_rx);
        Ok(AgentSession {
            observations: observation_rx,
            questions: question_tx,
            steers: steer_tx,
            handle: RunHandle::default(),
        })
    }
}

fn request(session: SessionRef) -> TurnRequest {
    TurnRequest {
        session,
        question: "帮我看看这个目录 🎉".to_owned(),
        injections: SessionInjections {
            preamble: Some("前导".to_owned()),
            tools: None,
            finish: None,
            ask: None,
        },
        ctx: SessionCtx {
            workspace_root: std::path::PathBuf::from("D:\\工作区"),
            permission_mode: AgentPermissionMode::BypassPermissions,
            model_level: ModelLevel::High,
        },
        provenance: SessionProvenance {
            source: "debug".to_owned(),
            source_ref: None,
        },
        config_snapshot: serde_json::json!({ "engine": "sdk" }),
        prior_handle: None,
    }
}

fn run_started_kind(session_id: &str) -> AgentEventKind {
    AgentEventKind::RunStarted {
        model: Some("gpt-x".to_owned()),
        session_id: Some(session_id.to_owned()),
        tools: Vec::new(),
        mcp_servers: Vec::new(),
    }
}

fn text_delta_kind(text: &str) -> AgentEventKind {
    AgentEventKind::MessageDelta {
        parent_tool_use_id: None,
        delta: AgentDelta::Text {
            text: text.to_owned(),
        },
    }
}

fn message_kind() -> AgentEventKind {
    AgentEventKind::Message {
        role: AgentMessageRole::Assistant,
        blocks: Vec::new(),
        parent_tool_use_id: None,
    }
}

fn turn_done_kind(is_error: bool) -> AgentEventKind {
    AgentEventKind::TurnDone {
        subtype: if is_error { "api_error" } else { "success" }.to_owned(),
        is_error,
        num_turns: Some(2),
        duration_ms: Some(800),
        cost_usd: Some(0.05),
        usage: serde_json::json!({ "input_tokens": 10, "output_tokens": 20 }),
        session_id: Some("sdk-9-1727000000009".to_owned()),
    }
}

/// 驱动一轮并回收全部输出（Observation / TurnFinished 分账）。
async fn drive_all(
    running: crate::kernel::RunningTurn,
) -> (Vec<AgentEvent>, Option<TurnOutcome>, TurnOutcome) {
    let outputs = Arc::new(Mutex::new(Vec::new()));
    let sink = outputs.clone();
    let outcome = running
        .drive(move |output| {
            sink.lock().expect("输出锁不可中毒").push(output);
        })
        .await;
    let mut observations = Vec::new();
    let mut finished = None;
    for output in outputs.lock().expect("输出锁不可中毒").drain(..) {
        match output {
            KernelOutput::Observation(event) => observations.push(event),
            KernelOutput::TurnFinished(turn) => finished = Some(turn),
        }
    }
    (observations, finished, outcome)
}

/// 摘 sink 调用记录快照。
fn calls_of(calls: &Arc<Mutex<Vec<SinkCall>>>) -> Vec<SinkCall> {
    calls.lock().expect("记录锁不可中毒").clone()
}

// ---------------------------------------------------------------------------
// StopRegistry：登记 / 置位 / miss 幂等 / 键隔离
// ---------------------------------------------------------------------------

#[test]
fn 注册表登记后置位命中返回true且句柄同步观测为true() {
    let registry = StopRegistry::new();
    let handle = RunHandle::default();

    registry.register("ses-1", handle.clone());

    assert!(
        registry.request_stop("ses-1"),
        "登记后置位命中返回 true（AC-7 停止链置位半边）"
    );
    assert!(
        handle.stop_requested(),
        "RunHandle.stop_requested 同步观测为 true"
    );
}

#[test]
fn 未登记与除名后置位miss幂等返回false不改终态() {
    let registry = StopRegistry::new();
    let handle = RunHandle::default();
    registry.register("ses-1", handle);

    // 未登记 session_id：miss 幂等返回 false
    assert!(!registry.request_stop("ses-404"), "未登记 miss 幂等");

    // 收敛除名：除名后 stop 对该键幂等忽略
    registry.remove("ses-1");
    assert!(!registry.request_stop("ses-1"), "除名后 stop 幂等忽略");
}

#[test]
fn 多session并行登记互不串扰且同键重复登记以新句柄覆盖() {
    let registry = StopRegistry::new();
    let handle_a = RunHandle::default();
    let handle_b = RunHandle::default();
    let replacement = RunHandle::default();

    registry.register("ses-a", handle_a.clone());
    registry.register("ses-b", handle_b.clone());
    registry.register("ses-a", replacement.clone()); // 同键覆盖

    registry.request_stop("ses-b");
    assert!(!handle_a.stop_requested(), "A 句柄不被 B 的置位波及");
    assert!(handle_b.stop_requested(), "B 置位直达本句柄");
    assert!(!replacement.stop_requested(), "覆盖句柄不受旧句柄置位影响");

    registry.request_stop("ses-a");
    assert!(
        replacement.stop_requested() && !handle_a.stop_requested(),
        "同键重复 register 以新 handle 覆盖（旧句柄不再被寻址）"
    );
}

// ---------------------------------------------------------------------------
// begin_turn：New 会话同步段（AC-9 编排下沉半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn begin_turn_new同步段sink调用序恰为建会话_开轮_登记且三字段承接() {
    let (sink, calls) = FakeSink::new();
    let registry = Arc::new(StopRegistry::new());
    let runner = Arc::new(FakeRunner::with_capacity(16));
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin_turn 应成功");

    // sink 调用序：create_session → begin_turn（无 bind/append/finish）
    let recorded = calls_of(&calls);
    assert_eq!(
        recorded.len(),
        2,
        "同步段恰两次 sink 调用，实际: {recorded:?}"
    );
    assert!(matches!(
        &recorded[0],
        SinkCall::CreateSession(row)
            if row.id == running.session_id
                && row.provenance.source == "debug"
                && row.config_snapshot == serde_json::json!({ "engine": "sdk" })
    ));
    assert!(matches!(
        &recorded[1],
        SinkCall::BeginTurn { session_id, .. } if session_id == &running.session_id
    ));

    // 提前 resolve 契约：session_id / turn_id / started_at 逐字段承接
    assert!(running.session_id.starts_with("ses-"), "core 铸 id");
    assert_eq!(running.turn_id, 1, "轮 id 承接 sink 分配序");
    assert!(running.started_at > 0, "started_at 在场");

    // 停止登记：注册表可寻址本会话（键 = core session id）
    assert!(
        registry.request_stop(&running.session_id),
        "begin_turn 后停止句柄已登记"
    );
    // 新会话行 open 捕获：injections 与轮级分离承接
    let captured = runner.captured.lock().expect("捕获锁").clone();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].injections.preamble.as_deref(),
        Some("前导"),
        "injections（会话级）与轮级分离不变量"
    );
}

#[tokio::test]
async fn begin_turn_open失败时err传播且假sink零调用() {
    let (sink, calls) = FakeSink::new();
    let registry = Arc::new(StopRegistry::new());
    let runner = Arc::new(FakeRunner::failing(AgentStartError::ConfigMissing(
        "凭据未配".to_owned(),
    )));
    let kernel = SessionKernel::new(
        Arc::new(sink),
        registry,
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );

    let result = kernel.begin_turn(runner, request(SessionRef::New));

    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("open 失败必须 Err"),
    };
    assert_eq!(
        error,
        AgentStartError::ConfigMissing("凭据未配".to_owned()),
        "启动错误原样传播"
    );
    assert!(
        calls_of(&calls).is_empty(),
        "启动失败不产生任何记录（open 失败不落库）"
    );
}

#[tokio::test]
async fn begin_turn_sink失败时err映射传播且错误映射单点在内核() {
    // create_session 失败
    let (mut sink, _calls) = FakeSink::new();
    sink.create_session_fails = true;
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let error = match kernel.begin_turn(
        Arc::new(FakeRunner::with_capacity(4)),
        request(SessionRef::New),
    ) {
        Err(error) => error,
        Ok(_) => panic!("create_session 失败必须 Err"),
    };
    assert!(
        matches!(error, AgentStartError::SpawnFailed(ref message)
            if message.contains("会话记录落库失败") && message.contains("会话行写入失败")),
        "create_session 失败映射携带记因，实际: {error:?}"
    );

    // begin_turn 失败
    let (mut sink, _calls) = FakeSink::new();
    sink.begin_turn_fails = true;
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let error = match kernel.begin_turn(
        Arc::new(FakeRunner::with_capacity(4)),
        request(SessionRef::New),
    ) {
        Err(error) => error,
        Ok(_) => panic!("begin_turn 失败必须 Err"),
    };
    assert!(
        matches!(error, AgentStartError::SpawnFailed(ref message)
            if message.contains("轮记录落库失败")),
        "begin_turn 失败映射携带记因，实际: {error:?}"
    );
    // 两类失败映射成因互不重合
    assert!(!format!("{error:?}").contains("会话记录落库失败"));
}

#[tokio::test]
async fn begin_turn_continue不重复建会话行且多轮轮id递增承接sink分配序() {
    let (sink, calls) = FakeSink::new();
    let registry = Arc::new(StopRegistry::new());
    let runner = Arc::new(FakeRunner::with_capacity(16));
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );

    let session = SessionRef::Continue {
        id: "ses-exist-1".to_owned(),
    };
    let mut open_request = request(session.clone());
    open_request.prior_handle = Some("sdk-1".to_owned());
    let first = kernel
        .begin_turn(runner.clone(), open_request)
        .expect("Continue 首轮应成功");
    let second = kernel
        .begin_turn(runner.clone(), request(session))
        .expect("Continue 次轮应成功");

    // Continue 不重复建会话行（create_session 不调用），轮行照开
    let recorded = calls_of(&calls);
    assert!(
        !recorded
            .iter()
            .any(|call| matches!(call, SinkCall::CreateSession(_))),
        "Continue 不建会话行，实际: {recorded:?}"
    );
    assert_eq!(first.session_id, "ses-exist-1", "沿用既有会话 id");
    assert_eq!(second.session_id, "ses-exist-1");
    assert_eq!(
        (first.turn_id, second.turn_id),
        (1, 2),
        "同 session 串联多轮：轮 id 递增承接 sink 分配序"
    );
}

// ---------------------------------------------------------------------------
// drive：同会话多轮续排（反馈修复边形态——转录主键（会话, seq）唯一）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 同会话次轮盖戳自库内最大seq续排_转录主键永不复用() {
    let (sink, calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    // 首轮（New）：密封占 0/1，TurnDone 收敛
    let first = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("首轮 begin 应成功");
    let session_id = first.session_id.clone();
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();
    let (_, _, first_outcome) = drive_all(first).await;
    assert_eq!(first_outcome.status, AgentRunStatus::Completed);

    // 次轮（Continue 同会话——反馈修复边续注形态）：基点 = 库内最大 seq + 1，
    // 轮级归零会与首轮行撞（会话, seq）主键（曾致修复边必死）
    let second = kernel
        .begin_turn(
            runner.clone(),
            request(SessionRef::Continue {
                id: session_id.clone(),
            }),
        )
        .expect("次轮 begin 应成功");
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();
    let (_, _, second_outcome) = drive_all(second).await;
    assert_eq!(second_outcome.status, AgentRunStatus::Completed);

    let appends: Vec<u64> = calls_of(&calls)
        .iter()
        .filter_map(|call| match call {
            SinkCall::AppendSealed { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(
        appends,
        vec![0, 1, 2, 3],
        "同会话两轮 seq 全程单调续排，次轮基点 = 前轮末位 + 1（撞主键回归锚）"
    );
}

// ---------------------------------------------------------------------------
// drive：delta 只上传输面（AC-2 内核半边 / AC-5 盖戳治理单点）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drive增量只上输出回调_盖戳补齐单调且sink零append() {
    let (sink, calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    runner
        .feed(vec![
            text_delta_kind("第一片"),
            text_delta_kind("第二片 🎉"),
            AgentEventKind::MessageDelta {
                parent_tool_use_id: None,
                delta: AgentDelta::Thinking {
                    thinking: "思考增量".to_owned(),
                },
            },
        ])
        .await;
    runner.close_stream();

    let (observations, _finished, outcome) = drive_all(running).await;

    // 增量全量上输出：盖戳后 KernelOutput::Observation
    assert_eq!(observations.len(), 3, "三枚增量全量流出");
    for (index, event) in observations.iter().enumerate() {
        assert_eq!(event.seq, index as u64, "seq 由内核补齐、单调无跳号");
        assert!(event.timestamp_ms > 0, "时间戳由内核盖戳");
        assert!(event.kind.is_delta(), "流出物仍为增量词汇");
    }

    // store 永不见 delta：append_sealed 零调用
    let recorded = calls_of(&calls);
    assert!(
        !recorded
            .iter()
            .any(|call| matches!(call, SinkCall::AppendSealed { .. })),
        "增量不落库（AC-2 内核半边），实际: {recorded:?}"
    );
    // 空流兜底：无收敛事件即 failed 记因
    assert_eq!(outcome.status, AgentRunStatus::Failed);
    assert_eq!(
        outcome.error.as_deref(),
        Some("轮结束但未产出收敛事件"),
        "空观察流兜底 failed 记因"
    );
}

// ---------------------------------------------------------------------------
// drive：密封 write-through（AC-6 单点）与双 id 映射（AC-5 半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drive密封事件write_through恰一次且载荷为盖戳后事件() {
    let (sink, calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    runner
        .feed(vec![
            run_started_kind("sdk-9-1727000000009"),
            message_kind(),
            message_kind(),
        ])
        .await;
    runner.close_stream();

    let (observations, _finished, outcome) = drive_all(running).await;

    // append_sealed 逐条按序恰一次（多密封事件按序落盘）
    let appends: Vec<u64> = calls_of(&calls)
        .iter()
        .filter_map(|call| match call {
            SinkCall::AppendSealed { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(appends, vec![0, 1, 2], "三密封事件按序逐条落盘");

    // 载荷为盖戳后事件：传输面观察与落库 seq 同源
    assert_eq!(observations.len(), 3);
    assert_eq!(
        observations
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>(),
        appends,
        "传输与落库两路 seq 同源同序"
    );

    // 双 id 映射：RunStarted 上报即绑定 + updated_at 刷新（尽力语义）
    let recorded = calls_of(&calls);
    let binds: Vec<&SinkCall> = recorded
        .iter()
        .filter(|call| matches!(call, SinkCall::BindRemote { .. }))
        .collect();
    assert_eq!(binds.len(), 1, "RunStarted 单次上报恰一次绑定");
    match binds[0] {
        SinkCall::BindRemote {
            session_id: sid,
            remote,
            updated_at,
        } => {
            assert_eq!(sid, &session_id);
            assert_eq!(remote, "sdk-9-1727000000009");
            assert!(*updated_at >= 0, "updated_at 取当前时钟毫秒");
        }
        other => panic!("应为 BindRemote，实际: {other:?}"),
    }

    // 无 TurnDone → EOF 兜底 failed；finish_turn 恰一次
    assert_eq!(outcome.status, AgentRunStatus::Failed);
    assert!(
        matches!(calls_of(&calls).last(), Some(SinkCall::FinishTurn { .. })),
        "收口路径以 finish_turn 结束"
    );
}

// ---------------------------------------------------------------------------
// drive：TurnDone 统计收口（AC-5 统计口径半边）与终态幂等
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drive_turn_done统计收口_唯一口径承接_除名流出终态() {
    let (sink, calls) = FakeSink::new();
    let registry = Arc::new(StopRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    runner
        .feed(vec![
            message_kind(),
            run_started_kind("sdk-9-1727000000009"),
            turn_done_kind(false),
        ])
        .await;
    runner.close_stream();

    let (observations, finished, outcome) = drive_all(running).await;

    // TurnDone 统计字段唯一口径承接（引擎无第二口径）
    assert_eq!(outcome.status, AgentRunStatus::Completed);
    assert_eq!(outcome.num_turns, Some(2));
    assert_eq!(outcome.duration_ms, Some(800));
    assert_eq!(outcome.cost_usd, Some(0.05));
    assert_eq!(
        outcome.usage,
        serde_json::json!({ "input_tokens": 10, "output_tokens": 20 })
    );
    assert_eq!(
        outcome.remote_session_id.as_deref(),
        Some("sdk-9-1727000000009"),
        "TurnDone 上报的双 id 映射半边"
    );
    assert!(outcome.error.is_none());
    assert!(outcome.finished_at > 0);

    // finish_turn 恰一次；注册表除名（remove 被调）；TurnFinished 流出
    let recorded = calls_of(&calls);
    let finishes: Vec<&SinkCall> = recorded
        .iter()
        .filter(|call| matches!(call, SinkCall::FinishTurn { .. }))
        .collect();
    assert_eq!(finishes.len(), 1, "收口恰一次（终态幂等）");
    assert!(
        !registry.request_stop(&session_id),
        "终态除名后停止寻址 miss"
    );
    let turn = finished.expect("TurnFinished 流出");
    assert_eq!(turn, outcome, "TurnFinished 与返回终态同构（与实时同构）");

    // TurnDone 后 EOF：无二次 sink 写入（最后一次密封即 TurnDone 自身）
    let appends: Vec<u64> = calls_of(&calls)
        .iter()
        .filter_map(|call| match call {
            SinkCall::AppendSealed { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(appends, vec![0, 1, 2], "TurnDone 后不再有追加写入");
    // 传输面：delta 无、观察 3 枚（TurnDone 在内），最后一枚为密封 TurnDone
    assert_eq!(observations.len(), 3);
    assert!(observations[2].kind.is_sealed());
}

// ---------------------------------------------------------------------------
// drive：append 失败收敛（落库失败不可静默）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drive_append失败时failed收敛记因_finish尽力_除名后流出终态() {
    let (sink, calls) = FakeSink::new();
    *sink.append_failures_from.lock().expect("失败锁不可中毒") = Some(2); // 第 2 条密封追加起失败
    let registry = Arc::new(StopRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();

    let (observations, finished, outcome) = drive_all(running).await;

    assert_eq!(
        outcome.status,
        AgentRunStatus::Failed,
        "落库失败显式失败收敛"
    );
    let recorded_error = outcome.error.clone().unwrap_or_default();
    assert!(
        recorded_error.contains("事件落库失败"),
        "error 记因携带落库失败，实际: {recorded_error}"
    );
    // finish_turn 尽力调用；除名后 TurnFinished 流出
    assert!(
        matches!(calls_of(&calls).last(), Some(SinkCall::FinishTurn { .. })),
        "失败收敛仍尽力收口落库"
    );
    assert!(!registry.request_stop(&session_id), "除名后 miss");
    assert!(finished.is_some(), "TurnFinished 流出（尽力流出终态）");
    // 失败即终止 tee：失败点之后不再流出观察（第 2 条密封未流出）
    assert_eq!(
        observations.len(),
        1,
        "落库失败即终止：失败密封事件不流入传输面，实际: {}",
        observations.len()
    );
}

// ---------------------------------------------------------------------------
// drive：停止收敛 stopped（AC-7）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drive停止置位后观察流关闭_stopped收敛不记因() {
    let (sink, calls) = FakeSink::new();
    let registry = Arc::new(StopRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();

    // 已产出的密封事件先入通道，随后观察流关闭 + 停止置位（编排侧显式收敛链）
    runner.feed(vec![message_kind()]).await;
    runner.close_stream();
    assert!(registry.request_stop(&session_id), "停止链置位");

    let (observations, finished, outcome) = drive_all(running).await;

    assert_eq!(
        outcome.status,
        AgentRunStatus::Stopped,
        "状态机未收敛但停止信号已置位 → 显式收敛 stopped"
    );
    assert_eq!(outcome.error, None, "用户主动终止非失败：error 不记因");
    assert!(outcome.finished_at > 0);
    assert!(finished.is_some(), "TurnFinished 流出");
    // 已产出密封事件原样落库后收口
    let appends: Vec<u64> = calls_of(&calls)
        .iter()
        .filter_map(|call| match call {
            SinkCall::AppendSealed { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(appends, vec![0], "已产出事件原样保留");
    assert_eq!(observations.len(), 1);
    assert!(!registry.request_stop(&session_id), "除名后 miss");
}

#[tokio::test]
async fn drive空观察流无停止信号时兜底failed且不悬挂() {
    let (sink, calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(4));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    runner.close_stream(); // 引擎立即 EOF，无任何观察

    let (observations, _finished, outcome) = drive_all(running).await;

    assert!(observations.is_empty(), "空流零观察");
    assert_eq!(
        outcome.status,
        AgentRunStatus::Failed,
        "无收敛事件兜底 failed"
    );
    assert_eq!(
        outcome.error.as_deref(),
        Some("轮结束但未产出收敛事件"),
        "兜底记因（不悬挂）"
    );
    assert!(matches!(
        calls_of(&calls).last(),
        Some(SinkCall::FinishTurn { .. })
    ));
}

// ---------------------------------------------------------------------------
// drive：有界通道背压与 on_output 解耦
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drive观察洪峰经背压全量送达零丢失且seq全程单调() {
    let (sink, _calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8)); // 容量 8 < 洪峰 64

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    // 生产端在独立任务上投喂洪峰（背压阻塞 send），随后 EOF
    let feeder_runner = runner.clone();
    let producer = tokio::spawn(async move {
        let mut events = Vec::with_capacity(64);
        for index in 0..64 {
            events.push(text_delta_kind(&format!("片{index}")));
        }
        events.push(message_kind());
        events.push(turn_done_kind(false));
        feeder_runner.feed(events).await;
        feeder_runner.close_stream();
    });

    let (observations, _finished, outcome) = drive_all(running).await;
    producer.await.expect("生产任务正常结束");

    assert_eq!(observations.len(), 66, "洪峰全量送达零丢失");
    let seqs: Vec<u64> = observations.iter().map(|event| event.seq).collect();
    let expected: Vec<u64> = (0..66).collect();
    assert_eq!(
        seqs, expected,
        "seq 全程单调无跳号（delta 占号共享单调空间）"
    );
    assert_eq!(outcome.status, AgentRunStatus::Completed, "洪峰后正常收敛");
}

#[tokio::test]
async fn drive_on_output慢速消费时密封落库与收口时序不受阻塞影响() {
    let (sink, calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();

    // 慢速输出回调（tee 双路独立：回调耗时不得影响落库与收口完整性）
    let outcome = running
        .drive(|output| {
            if matches!(output, KernelOutput::Observation(_)) {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        })
        .await;

    assert_eq!(outcome.status, AgentRunStatus::Completed);
    let recorded = calls_of(&calls);
    assert_eq!(
        recorded
            .iter()
            .filter(|call| matches!(call, SinkCall::AppendSealed { .. }))
            .count(),
        2,
        "密封 write-through 全量完成（tee 双路独立）"
    );
    assert!(
        matches!(recorded.last(), Some(SinkCall::FinishTurn { .. })),
        "收口时序完整"
    );
}

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`SessionKernel` 第 4 参（`Arc<AskRegistry>`）——id 下传与
// 轮收敛除名兜底
// ---------------------------------------------------------------------------

/// 新会话 id 下传（AC-1 的一半）：`SessionRef::New` 发起 → 假 runner 捕获的
/// `SessionOpen.session_id` 等于 `RunningTurn.session_id`（core 铸 id，非 `sdk-`
/// 引擎侧形态）；Continue 形态沿用既有会话 id（同会话同 id）。
#[tokio::test]
async fn begin_turn把core会话id下传open且continue沿用同id() {
    // New 形态
    let (sink, _calls) = FakeSink::new();
    let runner = Arc::new(FakeRunner::with_capacity(4));
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("New 应成功");
    let captured = runner.captured.lock().expect("捕获锁不可中毒").clone();
    assert_eq!(
        captured[0].session_id, running.session_id,
        "New：open 的 session_id == RunningTurn.session_id（core 铸 id 下传）"
    );
    assert!(
        captured[0].session_id.starts_with("ses-"),
        "core 铸 id 形态（非引擎侧 sdk- 形态），实际: {}",
        captured[0].session_id
    );

    // Continue 形态：沿用既有会话 id
    let (sink, _calls) = FakeSink::new();
    let runner = Arc::new(FakeRunner::with_capacity(4));
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let running = kernel
        .begin_turn(
            runner.clone(),
            request(SessionRef::Continue {
                id: "ses-7-1727000000007".to_owned(),
            }),
        )
        .expect("Continue 应成功");
    let captured = runner.captured.lock().expect("捕获锁不可中毒").clone();
    assert_eq!(
        captured[0].session_id, "ses-7-1727000000007",
        "Continue：session_id == 引用 id（同会话同 id）"
    );
    assert_eq!(running.session_id, "ses-7-1727000000007");
}

/// 除名兜底（AC-5，组合迭代 2 收敛形态）：轮中预置注册表登记（模拟引擎侧
/// 执行体登记）→ 驱动至收敛 → 其后 `answer` → `Err`；同时断言 `StopRegistry`
/// 该键亦已除名（两表同一出口）。
#[tokio::test]
async fn drive收敛后ask与stop两表同址除名() {
    for (label, events, expect_status) in [
        (
            "Completed",
            vec![message_kind(), turn_done_kind(false)],
            AgentRunStatus::Completed,
        ),
        ("Stopped", vec![message_kind()], AgentRunStatus::Stopped),
    ] {
        let (sink, _calls) = FakeSink::new();
        let stops = Arc::new(StopRegistry::new());
        let asks = Arc::new(AskRegistry::default());
        let kernel = SessionKernel::new(
            Arc::new(sink),
            Arc::clone(&stops),
            Arc::new(SessionStreamRegistry::new()),
            Arc::clone(&asks),
            Arc::new(SteerRegistry::default()),
        );
        let runner = Arc::new(FakeRunner::with_capacity(8));
        let running = kernel
            .begin_turn(runner.clone(), request(SessionRef::New))
            .expect("begin 应成功");
        let session_id = running.session_id.clone();

        // 轮中预置 ask 登记（引擎侧执行体进入阻塞前的形态）
        let (tx, _rx) = tokio::sync::oneshot::channel::<String>();
        assert!(asks.register(&session_id, tx), "轮中登记该会话的等待方");

        if label == "Stopped" {
            assert!(stops.request_stop(&session_id), "停止置位");
        }
        runner.feed(events).await;
        runner.close_stream();
        let (_observations, _finished, outcome) = drive_all(running).await;

        assert_eq!(outcome.status, expect_status, "{label} 收敛");
        assert!(
            asks.answer(&session_id, "迟到应答".to_owned()).is_err(),
            "{label}：收敛后 ask 槽已除名（answer → Err）"
        );
        assert!(
            !stops.request_stop(&session_id),
            "{label}：StopRegistry 该键亦已除名（两表同一出口）"
        );
    }
}

/// 只除本会话键（边界，组合迭代 2 会话 × 2 收敛序）：两会话并行登记 → 会话 A
/// 收敛后 `answer(A)` → `Err`、`answer(B)` → `Ok`（按 core session id 隔离）。
#[tokio::test]
async fn 收敛只除本会话ask键不波及并行会话() {
    // 两会话各用独立假 runner（feed 寻址按 runner 隔离），两种收敛序各跑一遍
    for first in ["A", "B"] {
        let (sink, _calls) = FakeSink::new();
        let asks = Arc::new(AskRegistry::default());
        let kernel = SessionKernel::new(
            Arc::new(sink),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::clone(&asks),
            Arc::new(SteerRegistry::default()),
        );
        let runner_a = Arc::new(FakeRunner::with_capacity(8));
        let runner_b = Arc::new(FakeRunner::with_capacity(8));

        let running_a = kernel
            .begin_turn(runner_a.clone(), request(SessionRef::New))
            .expect("A begin 应成功");
        let session_a = running_a.session_id.clone();
        let running_b = kernel
            .begin_turn(runner_b.clone(), request(SessionRef::New))
            .expect("B begin 应成功");
        let session_b = running_b.session_id.clone();
        assert_ne!(session_a, session_b, "两会话 id 互异");

        let (tx_a, mut rx_a) = tokio::sync::oneshot::channel::<String>();
        let (tx_b, mut rx_b) = tokio::sync::oneshot::channel::<String>();
        asks.register(&session_a, tx_a);
        asks.register(&session_b, tx_b);

        // 按序收敛首会话（另一会话保持运行以验证键隔离）
        let (first_running, first_runner, first_id, second_runner) = if first == "A" {
            (
                running_a,
                runner_a.clone(),
                session_a.clone(),
                runner_b.clone(),
            )
        } else {
            (
                running_b,
                runner_b.clone(),
                session_b.clone(),
                runner_a.clone(),
            )
        };
        first_runner
            .feed(vec![message_kind(), turn_done_kind(false)])
            .await;
        first_runner.close_stream();
        let (_obs, _fin, outcome) = drive_all(first_running).await;
        assert_eq!(outcome.status, AgentRunStatus::Completed, "{first} 收敛");

        assert!(
            asks.answer(&first_id, "迟到应答".to_owned()).is_err(),
            "{first} 收敛后除名"
        );
        if first == "A" {
            assert!(
                asks.answer(&session_b, "B 应答".to_owned()).is_ok(),
                "B 仍可应答（未收敛，键隔离）"
            );
            assert_eq!(rx_b.try_recv().expect("B 收到"), "B 应答");
        } else {
            assert!(
                asks.answer(&session_a, "A 应答".to_owned()).is_ok(),
                "A 仍可应答（B 的收敛未波及，键隔离）"
            );
            assert_eq!(rx_a.try_recv().expect("A 收到"), "A 应答");
        }
        // 未收敛会话的观察流保持打开（不影响上面断言）
        drop(second_runner);
    }
}

/// 观察流异常终止（无 `TurnDone`）兜底 `Failed` 路径同样除名（异常）：关闭
/// 观察流后 `answer` → `Err`。
#[tokio::test]
async fn 观察流异常终止兜底failed路径同样除名ask键() {
    let (sink, _calls) = FakeSink::new();
    let asks = Arc::new(AskRegistry::default());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::clone(&asks),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));
    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();

    let (tx, _rx) = tokio::sync::oneshot::channel::<String>();
    asks.register(&session_id, tx);
    // 无 TurnDone：投喂普通消息后直接关闭流
    runner.feed(vec![message_kind()]).await;
    runner.close_stream();

    let (_obs, _fin, outcome) = drive_all(running).await;
    assert_eq!(
        outcome.status,
        AgentRunStatus::Failed,
        "异常终止兜底 failed"
    );
    assert!(
        asks.answer(&session_id, "迟到应答".to_owned()).is_err(),
        "兜底 failed 路径同样除名"
    );
}

/// 送达失败臂（异常，引擎侧已终止）：提问 `send` 失败 → `Failed` 收敛且除名
///（既有错误文案零改动，新字段不参与）。
#[tokio::test]
async fn ask送达失败臂failed收敛后同样除名ask键() {
    let (sink, _calls) = FakeSink::new();
    let asks = Arc::new(AskRegistry::default());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::clone(&asks),
        Arc::new(SteerRegistry::default()),
    );
    let running = kernel
        .begin_turn(Arc::new(AskFailingRunner), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();

    let (tx, _rx) = tokio::sync::oneshot::channel::<String>();
    asks.register(&session_id, tx);

    let (observations, _finished, outcome) = drive_all(running).await;
    assert_eq!(outcome.status, AgentRunStatus::Failed);
    assert!(observations.is_empty(), "ask 送达失败零伪事件");
    assert!(
        asks.answer(&session_id, "迟到应答".to_owned()).is_err(),
        "送达失败臂同样除名"
    );
}

/// 启动失败四臂不污染注册表（异常，参数化 4 形态）：`open_session` Err /
/// `create_session` Err / `begin_turn` Err / `next_seq` Err → 返回
/// `AgentStartError`，`answer(<任何 id>)` 仍 `Err`、无 panic。
#[tokio::test]
async fn 启动失败四臂不污染ask注册表() {
    struct NextSeqFailingSink {
        inner: FakeSink,
    }

    impl SessionSink for NextSeqFailingSink {
        fn create_session(&self, session: &NewSessionRow) -> Result<(), String> {
            self.inner.create_session(session)
        }
        fn begin_turn(&self, session_id: &str, started_at: i64) -> Result<i64, String> {
            self.inner.begin_turn(session_id, started_at)
        }
        fn next_seq(&self, _session_id: &str) -> Result<u64, String> {
            Err("序号读取失败".to_owned())
        }
        fn append_sealed(&self, _session_id: &str, _event: &AgentEvent) -> Result<(), String> {
            Ok(())
        }
        fn finish_turn(&self, _turn_id: i64, _outcome: &TurnOutcome) -> Result<(), String> {
            Ok(())
        }
        fn bind_remote_session(
            &self,
            _session_id: &str,
            _remote: &str,
            _updated_at: i64,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    let mut failures: Vec<(String, Result<crate::kernel::RunningTurn, AgentStartError>)> =
        Vec::new();

    // ① open_session Err
    {
        let (sink, _calls) = FakeSink::new();
        let asks = Arc::new(AskRegistry::default());
        let kernel = SessionKernel::new(
            Arc::new(sink),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::clone(&asks),
            Arc::new(SteerRegistry::default()),
        );
        let result = kernel.begin_turn(
            Arc::new(FakeRunner::failing(AgentStartError::ConfigMissing(
                "凭据未配".to_owned(),
            ))),
            request(SessionRef::New),
        );
        assert!(asks.answer("任何 id", "x".to_owned()).is_err());
        failures.push(("open_session".to_owned(), result));
    }

    // ② create_session Err
    {
        let (sink, _calls) = FakeSink::new();
        let mut sink = sink;
        sink.create_session_fails = true;
        let asks = Arc::new(AskRegistry::default());
        let kernel = SessionKernel::new(
            Arc::new(sink),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::clone(&asks),
            Arc::new(SteerRegistry::default()),
        );
        let result = kernel.begin_turn(
            Arc::new(FakeRunner::with_capacity(4)),
            request(SessionRef::New),
        );
        assert!(asks.answer("任何 id", "x".to_owned()).is_err());
        failures.push(("create_session".to_owned(), result));
    }

    // ③ begin_turn Err
    {
        let (sink, _calls) = FakeSink::new();
        let mut sink = sink;
        sink.begin_turn_fails = true;
        let asks = Arc::new(AskRegistry::default());
        let kernel = SessionKernel::new(
            Arc::new(sink),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::clone(&asks),
            Arc::new(SteerRegistry::default()),
        );
        let result = kernel.begin_turn(
            Arc::new(FakeRunner::with_capacity(4)),
            request(SessionRef::New),
        );
        assert!(asks.answer("任何 id", "x".to_owned()).is_err());
        failures.push(("begin_turn".to_owned(), result));
    }

    // ④ next_seq Err
    {
        let (sink, _calls) = FakeSink::new();
        let asks = Arc::new(AskRegistry::default());
        let kernel = SessionKernel::new(
            Arc::new(NextSeqFailingSink { inner: sink }),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::clone(&asks),
            Arc::new(SteerRegistry::default()),
        );
        let result = kernel.begin_turn(
            Arc::new(FakeRunner::with_capacity(4)),
            request(SessionRef::New),
        );
        assert!(asks.answer("任何 id", "x".to_owned()).is_err());
        failures.push(("next_seq".to_owned(), result));
    }

    for (label, result) in failures {
        // open_session 臂原样传播引擎侧错误类型；其余三臂包装为 SpawnFailed
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("{label} 臂应 Err（启动失败四臂）"),
        };
        assert!(
            matches!(
                (label.as_str(), &error),
                ("open_session", AgentStartError::ConfigMissing(_))
                    | (
                        "create_session" | "begin_turn" | "next_seq",
                        AgentStartError::SpawnFailed(_)
                    )
            ),
            "{label} 臂的错误类型与既有语义一致，实际: {error:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 会话实时流注册表接线（D2 单点发布）：构造三参 / begin 轮登记先于驱动 /
// drive 同源发布与终态投递除名 / 落库失败与空流兜底的终态一致性
// ---------------------------------------------------------------------------

/// 订阅句柄排空：事件序列与终态（终态恒为末条；断流视为用例失败）。
async fn drain_stream(
    mut subscription: SessionStreamSubscription,
) -> (Vec<AgentEvent>, TurnOutcome) {
    let mut events = Vec::new();
    loop {
        match subscription.recv().await {
            Ok(SessionStreamItem::Event(event)) => events.push(event),
            Ok(SessionStreamItem::Finished(outcome)) => return (events, outcome),
            Err(error) => panic!("不应断流（事件与终态齐备）：{error:?}"),
        }
    }
}

/// 轮提问通道已关闭的 runner（ask 送达失败臂：零伪事件、零伪成功终态）。
struct AskFailingRunner;

impl AgentRunner for AskFailingRunner {
    fn open_session(&self, _open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        // 问题通道接收端立即释放：send 返回 Err
        let (questions, questions_rx) = tokio::sync::mpsc::channel::<TurnQuestion>(1);
        drop(questions_rx);
        // 观察流同样已 EOF（ask 失败先于观察消费）
        let (observations, observations_rx) = tokio::sync::mpsc::channel::<AgentEventKind>(1);
        drop(observations);
        let (steer_tx, _steer_rx) = tokio::sync::mpsc::unbounded_channel();
        Ok(AgentSession {
            observations: observations_rx,
            questions,
            steers: steer_tx,
            handle: RunHandle::default(),
        })
    }
}

#[tokio::test]
async fn 内核三参构造后begin_turn使stop与streams两表各自命中且互不替代() {
    let (sink, _calls) = FakeSink::new();
    let stops = Arc::new(StopRegistry::new());
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&stops),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(4));

    let running = kernel
        .begin_turn(runner, request(SessionRef::New))
        .expect("三参构造后 begin_turn 应成功");

    assert!(
        streams.subscribe(&running.session_id, 0).is_some(),
        "streams 条目由 begin_turn 建立（订阅可即刻接入）"
    );
    assert!(
        stops.request_stop(&running.session_id),
        "stop 登记同样命中（两表互不替代）"
    );
    assert!(
        streams.subscribe(&running.session_id, 0).is_some(),
        "stop 命中不影响 streams 条目"
    );
}

#[test]
fn 内核未begin_turn时任意id订阅为none() {
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let _ = kernel;

    for session_id in ["ses-1", "", "特殊\u{0}😀"] {
        assert!(
            streams.subscribe(session_id, 0).is_none(),
            "新内核不预置条目（构造不产生任何登记）"
        );
    }
}

#[tokio::test]
async fn 两内核共享同一streams时各自登记条目并存互不覆盖() {
    let (sink_a, _calls_a) = FakeSink::new();
    let (sink_b, _calls_b) = FakeSink::new();
    let stops = Arc::new(StopRegistry::new());
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel_a = SessionKernel::new(
        Arc::new(sink_a),
        Arc::clone(&stops),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let kernel_b = SessionKernel::new(
        Arc::new(sink_b),
        Arc::clone(&stops),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(4));

    let first = kernel_a
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("exec 轨首轮应成功");
    let second = kernel_b
        .begin_turn(runner, request(SessionRef::New))
        .expect("change 轨首轮应成功");

    assert_ne!(first.session_id, second.session_id, "两会话各自铸 id");
    assert!(
        streams.subscribe(&first.session_id, 0).is_some(),
        "首条条目不被后者覆盖"
    );
    assert!(
        streams.subscribe(&second.session_id, 0).is_some(),
        "后者条目并存"
    );
}

#[tokio::test]
async fn begin_turn返回前已登记_new与continue两形态均登记() {
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));

    // New 形态
    let fresh = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("New 轮应成功");
    assert!(
        streams.subscribe(&fresh.session_id, 0).is_some(),
        "返回即订阅 Some（登记先于驱动）"
    );

    // Continue 形态：沿用既有会话 id 同样登记（同会话次轮覆盖旧条目）
    let continued = kernel
        .begin_turn(
            runner,
            request(SessionRef::Continue {
                id: fresh.session_id.clone(),
            }),
        )
        .expect("Continue 轮应成功");
    assert_eq!(continued.session_id, fresh.session_id, "沿用既有会话 id");
    assert!(
        streams.subscribe(&continued.session_id, 0).is_some(),
        "覆盖后仍可订阅（无重入守卫）"
    );
}

#[tokio::test]
async fn begin_turn三类失败零条目() {
    let streams = Arc::new(SessionStreamRegistry::new());

    // open 失败
    let (sink, _calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let failed_open = kernel.begin_turn(
        Arc::new(FakeRunner::failing(AgentStartError::ConfigMissing(
            "凭据未配".to_owned(),
        ))),
        request(SessionRef::New),
    );
    assert!(failed_open.is_err(), "open 失败必须 Err");

    // 建会话行失败
    let (mut sink, _calls) = FakeSink::new();
    sink.create_session_fails = true;
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let failed_create = kernel.begin_turn(
        Arc::new(FakeRunner::with_capacity(4)),
        request(SessionRef::New),
    );
    assert!(failed_create.is_err(), "建会话行失败必须 Err");

    // 开轮行失败
    let (mut sink, _calls) = FakeSink::new();
    sink.begin_turn_fails = true;
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let failed_turn = kernel.begin_turn(
        Arc::new(FakeRunner::with_capacity(4)),
        request(SessionRef::New),
    );
    assert!(failed_turn.is_err(), "开轮行失败必须 Err");

    // 全败零条目（不留半成品）
    for session_id in ["ses-1", "", "ses-any"] {
        assert!(
            streams.subscribe(session_id, 0).is_none(),
            "启动失败不产生注册表条目（{session_id:?}）"
        );
    }
}

#[tokio::test]
async fn drive的delta与密封同源发布_订阅端序列与输出回调逐条相等() {
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    let started_at = running.started_at;
    let subscription = streams.subscribe(&session_id, 0).expect("订阅方可即刻接入");

    runner
        .feed(vec![
            run_started_kind("sdk-9-1727000000009"),
            text_delta_kind("第一片"),
            text_delta_kind("第二片 🎉"),
            message_kind(),
            turn_done_kind(false),
        ])
        .await;
    runner.close_stream();

    let (observations, _finished, outcome) = drive_all(running).await;
    let (streamed, terminal) = drain_stream(subscription).await;

    // since_seq = 0 为「已观测最大 seq」上界：只投严格更大者（首事件 seq 0 由重放
    // 装载覆盖，实时面自 seq 1 起）
    assert_eq!(
        streamed,
        observations[1..],
        "订阅端序列与 on_output 逐条相等（含 delta，载荷逐字）"
    );
    assert_eq!(
        streamed.iter().map(|event| event.seq).collect::<Vec<_>>(),
        vec![1, 2, 3, 4],
        "seq 严格递增（盖戳单点同源）且大于 since_seq"
    );
    assert_eq!(observations[0].seq, 0, "被过滤的恰为首枚 seq 0 事件");
    assert!(
        streamed.iter().any(|event| event.kind.is_delta()),
        "delta 可见"
    );
    assert_eq!(terminal, outcome, "终态与 drive 返回同构");
    assert_eq!(terminal.started_at, started_at, "终态携轮起点");
    assert_eq!(
        terminal.status,
        AgentRunStatus::Completed,
        "status 与 on_output 的 TurnFinished 相等"
    );
    assert!(
        streams.subscribe(&session_id, 0).is_none(),
        "drive 后条目除名"
    );
}

#[tokio::test]
async fn drive_append失败时订阅端得failed终态且此前delta已投递且条目除名() {
    let (sink, _calls) = FakeSink::new();
    *sink.append_failures_from.lock().expect("失败锁不可中毒") = Some(1);
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    let subscription = streams.subscribe(&session_id, 0).expect("订阅方可接入");
    runner
        .feed(vec![
            text_delta_kind("首片增量"),
            text_delta_kind("先到的增量"),
            message_kind(),
        ])
        .await;
    runner.close_stream();

    let (_observations, _finished, outcome) = drive_all(running).await;
    let (streamed, terminal) = drain_stream(subscription).await;

    assert_eq!(
        streamed.iter().map(|event| event.seq).collect::<Vec<_>>(),
        vec![1],
        "落库失败前的增量已投递（seq 1；seq 0 被 since_seq 过滤）"
    );
    assert!(streamed[0].kind.is_delta(), "先到的为增量事件");
    assert_eq!(terminal.status, AgentRunStatus::Failed);
    assert_eq!(terminal, outcome, "订阅端终态与 drive 返回同构");
    let error = terminal.error.clone().unwrap_or_default();
    assert!(
        error.starts_with("事件落库失败: "),
        "记因前缀逐字，实际: {error}"
    );
    assert!(
        streams.subscribe(&session_id, 0).is_none(),
        "失败收口同样除名"
    );
}

#[tokio::test]
async fn drive_ask送达失败时订阅端仅得failed终态零伪事件() {
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );

    let running = kernel
        .begin_turn(Arc::new(AskFailingRunner), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    let subscription = streams.subscribe(&session_id, 0).expect("订阅方可接入");

    let (_observations, _finished, outcome) = drive_all(running).await;
    let (streamed, terminal) = drain_stream(subscription).await;

    assert!(streamed.is_empty(), "ask 失败零伪事件");
    assert_eq!(terminal.status, AgentRunStatus::Failed);
    assert_eq!(terminal, outcome);
    assert_eq!(
        terminal.error.as_deref(),
        Some("轮提问送达失败（引擎侧已终止）"),
        "记因逐字"
    );
    assert!(
        streams.subscribe(&session_id, 0).is_none(),
        "条目除名（不悬挂）"
    );
}

#[tokio::test]
async fn drive空观察流无停止信号时投递failed兜底终态后除名() {
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(4));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    let subscription = streams.subscribe(&session_id, 0).expect("订阅方可接入");
    runner.close_stream();

    let (_observations, _finished, _outcome) = drive_all(running).await;
    let (streamed, terminal) = drain_stream(subscription).await;

    assert!(streamed.is_empty(), "空观察流零事件");
    assert_eq!(terminal.status, AgentRunStatus::Failed);
    assert_eq!(
        terminal.error.as_deref(),
        Some("轮结束但未产出收敛事件"),
        "兜底记因逐字"
    );
    assert!(
        streams.subscribe(&session_id, 0).is_none(),
        "投递后除名（不悬挂）"
    );
}

#[tokio::test]
async fn drive无订阅者全程发布静默且容量内不消费不阻塞驱动() {
    // 无订阅者：全程发布静默不 panic
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));
    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    runner
        .feed(vec![
            text_delta_kind("无人接收"),
            message_kind(),
            turn_done_kind(false),
        ])
        .await;
    runner.close_stream();
    let (_, _, outcome) = drive_all(running).await;
    assert_eq!(outcome.status, AgentRunStatus::Completed, "无订阅者不阻塞");

    // 有订阅者但不消费（容量内）：驱动照常完成，事后仍可逐条排空
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));
    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let subscription = streams
        .subscribe(&running.session_id, 0)
        .expect("订阅方可接入");
    runner
        .feed(vec![
            text_delta_kind("滞留增量"),
            message_kind(),
            turn_done_kind(false),
        ])
        .await;
    runner.close_stream();
    let (_, _, outcome) = drive_all(running).await;
    assert_eq!(
        outcome.status,
        AgentRunStatus::Completed,
        "同步 send 不等待消费端"
    );
    let (streamed, terminal) = drain_stream(subscription).await;
    assert_eq!(
        streamed.len(),
        2,
        "事后仍可排空容量内事件（seq 0 被 since_seq=0 过滤，恰余两条）"
    );
    assert_eq!(terminal.status, AgentRunStatus::Completed);
}

#[tokio::test]
async fn drive同会话次轮_以首轮最大seq为since_seq时只收次轮新增事件() {
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));

    // 首轮：两密封事件（seq 0 / 1）
    let first = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("首轮 begin 应成功");
    let session_id = first.session_id.clone();
    let first_subscription = streams.subscribe(&session_id, 0).expect("首轮订阅");
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();
    let (_observations, _finished, _outcome) = drive_all(first).await;
    let (first_streamed, _) = drain_stream(first_subscription).await;
    let first_max = first_streamed
        .iter()
        .map(|event| event.seq)
        .max()
        .unwrap_or(0);
    assert_eq!(first_max, 1, "首轮已投递最大 seq");

    // 次轮（Continue 同会话）：覆盖后 seq 空间续排
    let second = kernel
        .begin_turn(
            runner.clone(),
            request(SessionRef::Continue {
                id: session_id.clone(),
            }),
        )
        .expect("次轮 begin 应成功");
    let second_subscription = streams
        .subscribe(&session_id, first_max)
        .expect("次轮订阅（覆盖后条目）");
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();
    let (_observations, _finished, _outcome) = drive_all(second).await;
    let (second_streamed, terminal) = drain_stream(second_subscription).await;
    let second_seqs: Vec<u64> = second_streamed.iter().map(|event| event.seq).collect();
    assert_eq!(
        second_seqs,
        vec![first_max + 1, first_max + 2],
        "只收次轮新增事件（不重投首轮）"
    );
    assert_eq!(terminal.status, AgentRunStatus::Completed);
}

#[tokio::test]
async fn drive_delta零落库不变而订阅端可见() {
    let (sink, calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let subscription = streams
        .subscribe(&running.session_id, 0)
        .expect("订阅方可接入");
    runner
        .feed(vec![
            text_delta_kind("增量一"),
            text_delta_kind("增量二"),
            message_kind(),
        ])
        .await;
    runner.close_stream();
    drive_all(running).await;
    let (streamed, _terminal) = drain_stream(subscription).await;

    // 持久面：append_sealed 调用序列只含密封事件
    let appended: Vec<u64> = calls_of(&calls)
        .iter()
        .filter_map(|call| match call {
            SinkCall::AppendSealed { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(appended.len(), 1, "只落一条密封事件，实际: {appended:?}");
    // 实时面：同轮 delta 在订阅端可见（实时与持久分流）
    let delta_seqs: Vec<u64> = streamed
        .iter()
        .filter(|event| event.kind.is_delta())
        .map(|event| event.seq)
        .collect();
    assert_eq!(
        delta_seqs,
        vec![1],
        "增量在实时面可见（seq 0 被 since_seq=0 过滤，seq 1 起可见）"
    );
    assert_eq!(appended, vec![2], "密封事件 seq 为 2（增量零落库）");
}

#[tokio::test]
async fn drive停止置位收敛stopped_订阅端终态error为none() {
    let (sink, _calls) = FakeSink::new();
    let registry = Arc::new(StopRegistry::new());
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&registry),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    let subscription = streams.subscribe(&session_id, 0).expect("订阅方可接入");
    assert!(registry.request_stop(&session_id), "停止置位命中");
    runner.close_stream();

    let (_observations, _finished, outcome) = drive_all(running).await;
    let (_streamed, terminal) = drain_stream(subscription).await;

    assert_eq!(outcome.status, AgentRunStatus::Stopped, "停止收敛 stopped");
    assert_eq!(terminal.status, AgentRunStatus::Stopped);
    assert_eq!(terminal.error, None, "用户主动终止不记因");
}

// ---------------------------------------------------------------------------
// 盖戳治理单点的轮归属（agent-event-turn-id）
//
// 判据：真实运行事件的轮归属只在 `drive` 的盖戳单点产生（`Some(turn_id)`），
// 一轮内 delta 与密封同值、两实时出口同源同值、续轮各自恒定（AC-2 / AC-3 /
// AC-4）。装置沿用既有 `FakeSink` / `FakeRunner` / `drive_all` / `drain_stream`。
// ---------------------------------------------------------------------------

/// 摘出全部 Observation 的轮归属（按流出序）。
fn observed_turn_ids(observations: &[AgentEvent]) -> Vec<Option<i64>> {
    observations.iter().map(|event| event.turn_id).collect()
}

#[tokio::test]
async fn drive流出事件全部携本轮轮归属且无一枚为none() {
    let (sink, _calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let turn_id = running.turn_id;
    runner
        .feed(vec![
            run_started_kind("sdk-9-1727000000009"),
            text_delta_kind("第一片"),
            text_delta_kind("第二片 🎉"),
            message_kind(),
            AgentEventKind::SystemNotice {
                subtype: "api_retry".to_owned(),
                payload: serde_json::json!({ "attempt": 2 }),
            },
            turn_done_kind(false),
        ])
        .await;
    runner.close_stream();

    let (observations, _finished, outcome) = drive_all(running).await;

    assert_eq!(observations.len(), 6, "混合流全量流出");
    assert_eq!(
        observed_turn_ids(&observations),
        vec![Some(turn_id); 6],
        "每枚观察事件的轮归属恒等于开轮 id（无一枚 None——漏盖即静默 null）"
    );
    assert!(
        observations.iter().all(|event| event.turn_id.is_some()),
        "真实运行事件不存在无轮归属形态"
    );
    assert_eq!(outcome.turn_id, turn_id, "终态轮 id 与开轮 id 同源");
    assert_eq!(outcome.status, AgentRunStatus::Completed);
}

#[tokio::test]
async fn drive的delta组与密封组turn_id集合各为单元素集且相等() {
    let (sink, _calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let turn_id = running.turn_id;
    runner
        .feed(vec![
            text_delta_kind("增量一"),
            text_delta_kind("增量二"),
            message_kind(),
            turn_done_kind(false),
        ])
        .await;
    runner.close_stream();

    let (observations, _finished, _outcome) = drive_all(running).await;

    let collect = |delta: bool| -> std::collections::BTreeSet<Option<i64>> {
        observations
            .iter()
            .filter(|event| event.kind.is_delta() == delta)
            .map(|event| event.turn_id)
            .collect()
    };
    let delta_ids = collect(true);
    let sealed_ids = collect(false);

    assert_eq!(delta_ids.len(), 1, "delta 组轮归属单一");
    assert_eq!(sealed_ids.len(), 1, "密封组轮归属单一");
    assert_eq!(
        delta_ids, sealed_ids,
        "delta 亦带轮归属（不做「delta 允许缺省」的豁免）"
    );
    assert_eq!(
        delta_ids.iter().next().copied().flatten(),
        Some(turn_id),
        "两组同值且等于开轮 id"
    );
}

#[tokio::test]
async fn drive两实时出口投递同一轮归属值() {
    let (sink, _calls) = FakeSink::new();
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let turn_id = running.turn_id;
    let subscription = streams
        .subscribe(&running.session_id, 0)
        .expect("订阅方可接入");
    runner
        .feed(vec![
            run_started_kind("sdk-9-1727000000009"),
            text_delta_kind("第一片"),
            message_kind(),
            turn_done_kind(false),
        ])
        .await;
    runner.close_stream();

    let (observations, _finished, _outcome) = drive_all(running).await;
    let (streamed, _terminal) = drain_stream(subscription).await;

    assert_eq!(
        streamed,
        observations[1..],
        "注册表发布与输出回调投递同一枚事件（逐字段相等，含 turn_id）"
    );
    assert_eq!(
        observed_turn_ids(&streamed),
        vec![Some(turn_id); streamed.len()],
        "订阅端的轮归属与输出回调同源同值"
    );
    assert_eq!(
        observed_turn_ids(&streamed),
        observed_turn_ids(&observations[1..]),
        "两出口 turn_id 序列逐条相等"
    );
}

#[tokio::test]
async fn continue续轮两轮轮归属各自恒定且跨轮互异() {
    let (sink, _calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let first = kernel
        .begin_turn(
            runner.clone(),
            request(SessionRef::Continue {
                id: "ses-exist-1".to_owned(),
            }),
        )
        .expect("首轮 begin 应成功");
    let first_turn_id = first.turn_id;
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();
    let (first_observations, _finished, _outcome) = drive_all(first).await;

    let second = kernel
        .begin_turn(
            runner.clone(),
            request(SessionRef::Continue {
                id: "ses-exist-1".to_owned(),
            }),
        )
        .expect("次轮 begin 应成功");
    let second_turn_id = second.turn_id;
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();
    let (second_observations, _finished, second_outcome) = drive_all(second).await;

    assert_ne!(first_turn_id, second_turn_id, "续轮轮 id 递增互异");
    assert_eq!(
        observed_turn_ids(&first_observations),
        vec![Some(first_turn_id); 2],
        "首轮事件轮归属恒定"
    );
    assert_eq!(
        observed_turn_ids(&second_observations),
        vec![Some(second_turn_id); 2],
        "次轮事件轮归属为新轮 id（不串轮）"
    );
    assert_ne!(
        first_observations[0].turn_id, second_observations[0].turn_id,
        "跨轮互不污染"
    );
    assert_eq!(second_outcome.turn_id, second_turn_id);
}

#[tokio::test]
async fn drive_append失败前已投递观察的轮归属不降级为none() {
    let (sink, _calls) = FakeSink::new();
    *sink.append_failures_from.lock().expect("失败锁不可中毒") = Some(1);
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let turn_id = running.turn_id;
    runner
        .feed(vec![text_delta_kind("先到的增量"), message_kind()])
        .await;
    runner.close_stream();

    let (observations, finished, outcome) = drive_all(running).await;

    assert_eq!(outcome.status, AgentRunStatus::Failed, "落库失败显式收敛");
    assert_eq!(observations.len(), 1, "失败前已投递的增量一枚");
    assert_eq!(
        observations[0].turn_id,
        Some(turn_id),
        "失败路径不把轮归属降级为 None"
    );
    assert_eq!(outcome.turn_id, turn_id);
    let terminal = finished.expect("TurnFinished 尽力流出");
    assert_eq!(terminal.turn_id, turn_id, "终态轮归属与开轮 id 相等");
}

#[tokio::test]
async fn drive_ask送达失败零事件轮不产生补盖戳伪事件() {
    let (sink, _calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );

    let running = kernel
        .begin_turn(Arc::new(AskFailingRunner), request(SessionRef::New))
        .expect("begin 应成功");
    let turn_id = running.turn_id;

    let (observations, finished, outcome) = drive_all(running).await;

    assert!(
        observations.is_empty(),
        "零事件轮零观察（不为轮归属造事件）"
    );
    assert_eq!(outcome.status, AgentRunStatus::Failed);
    assert_eq!(
        outcome.turn_id, turn_id,
        "终态仍携开轮 id（轮归属不因零事件丢失）"
    );
    assert_eq!(finished.map(|turn| turn.turn_id), Some(turn_id));
}

#[tokio::test]
async fn drive空观察流兜底failed不合成事件且终态携开轮id() {
    let (sink, _calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(4));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let turn_id = running.turn_id;
    runner.close_stream();

    let (observations, finished, outcome) = drive_all(running).await;

    assert!(observations.is_empty(), "兜底不合成盖戳事件");
    assert_eq!(outcome.status, AgentRunStatus::Failed);
    assert_eq!(
        outcome.error.as_deref(),
        Some("轮结束但未产出收敛事件"),
        "兜底记因不变"
    );
    assert_eq!(outcome.turn_id, turn_id);
    assert_eq!(finished.map(|turn| turn.turn_id), Some(turn_id));
}

/// 轮 id 三形态（含负值与极大值）：`Option<i64>` 不做符号归一 / 不夹取，且不与
/// seq 混淆。`FakeSink` 是同文件既有装置，`begin_turn` 前预置 `next_turn_id` 即可
/// 指定 sink 分配值（无需新增公开 API）。
///
/// 极大值取 `i64::MAX - 1`：`FakeSink::begin_turn` 分配后执行 `*next += 1`，预置
/// `i64::MAX` 会在 debug 下溢出 panic（那是装置的自增臂事实，不是被测契约）。
#[tokio::test]
async fn drive轮id三形态逐字落盖戳且不与seq混淆() {
    for expected in [1_i64, -1, i64::MAX - 1] {
        let (sink, _calls) = FakeSink::new();
        *sink.next_turn_id.lock().expect("轮 id 锁不可中毒") = expected;
        let kernel = SessionKernel::new(
            Arc::new(sink),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::new(AskRegistry::default()),
            Arc::new(SteerRegistry::default()),
        );
        let runner = Arc::new(FakeRunner::with_capacity(8));

        let running = kernel
            .begin_turn(runner.clone(), request(SessionRef::New))
            .expect("begin 应成功");
        assert_eq!(running.turn_id, expected, "sink 分配值原样承接");
        runner
            .feed(vec![text_delta_kind("片"), message_kind()])
            .await;
        runner.close_stream();

        let (observations, _finished, outcome) = drive_all(running).await;

        assert_eq!(
            observed_turn_ids(&observations),
            vec![Some(expected), Some(expected)],
            "流出事件轮归属逐字等于 sink 分配值（轮 id = {expected}）"
        );
        assert_eq!(
            observations
                .iter()
                .map(|event| event.seq)
                .collect::<Vec<_>>(),
            vec![0, 1],
            "轮归属与 seq 正交（seq 自基点独立递增）"
        );
        assert_eq!(outcome.turn_id, expected);
    }
}

#[tokio::test]
async fn 同会话两轮seq续排且两轮轮归属集合互异() {
    let (sink, calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let first = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("首轮 begin 应成功");
    let session_id = first.session_id.clone();
    let first_turn_id = first.turn_id;
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();
    let (first_observations, _finished, _outcome) = drive_all(first).await;

    let second = kernel
        .begin_turn(
            runner.clone(),
            request(SessionRef::Continue {
                id: session_id.clone(),
            }),
        )
        .expect("次轮 begin 应成功");
    let second_turn_id = second.turn_id;
    runner
        .feed(vec![message_kind(), turn_done_kind(false)])
        .await;
    runner.close_stream();
    let (second_observations, _finished, _outcome) = drive_all(second).await;

    // 既有口径：两轮各 2 枚密封，seq 全程续排
    let appends: Vec<u64> = calls_of(&calls)
        .iter()
        .filter_map(|call| match call {
            SinkCall::AppendSealed { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(appends, vec![0, 1, 2, 3], "同会话两轮 seq 全程续排");

    // 轮归属与 seq 空间正交：新字段不进 next_seq、不改打包键
    let first_ids: std::collections::BTreeSet<Option<i64>> =
        observed_turn_ids(&first_observations).into_iter().collect();
    let second_ids: std::collections::BTreeSet<Option<i64>> =
        observed_turn_ids(&second_observations)
            .into_iter()
            .collect();
    assert_eq!(
        first_ids,
        [Some(first_turn_id)]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );
    assert_eq!(
        second_ids,
        [Some(second_turn_id)]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );
    assert_ne!(first_ids, second_ids, "两轮轮归属集合互异");
}

#[tokio::test]
async fn drive观察洪峰全部事件携本轮轮归属且seq无跳号() {
    let (sink, _calls) = FakeSink::new();
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8)); // 容量 8 < 洪峰 64

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let turn_id = running.turn_id;
    let feeder_runner = runner.clone();
    let producer = tokio::spawn(async move {
        let mut events = Vec::with_capacity(66);
        for index in 0..64 {
            events.push(text_delta_kind(&format!("片{index}")));
        }
        events.push(message_kind());
        events.push(turn_done_kind(false));
        feeder_runner.feed(events).await;
        feeder_runner.close_stream();
    });

    let (observations, _finished, outcome) = drive_all(running).await;
    producer.await.expect("生产任务正常结束");

    assert_eq!(observations.len(), 66, "洪峰全量送达零丢失");
    assert_eq!(
        observations
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>(),
        (0..66).collect::<Vec<u64>>(),
        "seq 仍 0..66 单调无跳号（背压不丢轮归属值）"
    );
    assert_eq!(
        observed_turn_ids(&observations),
        vec![Some(turn_id); 66],
        "背压下轮归属恒为本轮 id"
    );
    assert_eq!(outcome.status, AgentRunStatus::Completed);
}

#[tokio::test]
async fn drive停止置位收敛零新增事件且既有密封轮归属不变() {
    let (sink, _calls) = FakeSink::new();
    let registry = Arc::new(StopRegistry::new());
    let streams = Arc::new(SessionStreamRegistry::new());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&registry),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::new(SteerRegistry::default()),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let turn_id = running.turn_id;
    let subscription = streams
        .subscribe(&running.session_id, 0)
        .expect("订阅方可接入");
    runner.feed(vec![message_kind(), message_kind()]).await;
    runner.close_stream();
    assert!(registry.request_stop(&running.session_id), "停止链置位");

    let (observations, _finished, outcome) = drive_all(running).await;
    let (streamed, terminal) = drain_stream(subscription).await;

    assert_eq!(outcome.status, AgentRunStatus::Stopped, "停止收敛 stopped");
    assert_eq!(observations.len(), 2, "收敛路径零新增事件");
    assert_eq!(
        observed_turn_ids(&observations),
        vec![Some(turn_id); 2],
        "置位前流出的密封事件轮归属不变"
    );
    // 订阅端 since_seq=0 只投严格更大者（seq 0 由重放装载覆盖）
    assert_eq!(
        streamed.len(),
        1,
        "订阅端只余 seq 1 一条（stopped 不合成事件）"
    );
    assert_eq!(streamed[0].turn_id, Some(turn_id), "订阅端轮归属同值");
    assert_eq!(terminal.turn_id, turn_id, "终态携开轮 id");
    assert_eq!(terminal.error, None, "用户主动终止不记因");
}

// ---------------------------------------------------------------------------
// steer 注入通道：内核半边的登记（键 = core 铸会话 id）与收口除名
//
// 表本体语义（miss / `send` 失败同臂 / 一条一 ack）归 `steer_test.rs`；本节只锁
// 「登记 / 除名与 StopRegistry、AskRegistry 同址同键空间」这一内核事实。
// ---------------------------------------------------------------------------

/// 补充说明载荷（ack 用就地 drop 的接收端：内核用例不断言 ack，投递成败即判据）。
fn steer_message(text: &str) -> crate::steer::SteerMessage {
    crate::steer::SteerMessage {
        text: text.to_owned(),
        ack: tokio::sync::oneshot::channel().0,
    }
}

/// 登记（正向，AC-1 的内核半边）：组合根注入的**同一** `Arc<SteerRegistry>` 上
/// `request(&RunningTurn.session_id)` → `Ok`，且该条落在本轮的接收入端。
#[tokio::test]
async fn begin_turn后同一实例请求ok且落在本轮接收端() {
    let (sink, _calls) = FakeSink::new();
    let steers = Arc::new(SteerRegistry::default());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::clone(&steers),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));
    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let mut rx = runner.take_steer_receiver().expect("本轮接收入端在场");

    steers
        .request(&running.session_id, steer_message("先跑 lint"))
        .expect("登记键 = RunningTurn.session_id，运行中可送达");
    let delivered = rx.try_recv().expect("该条落在本轮接收入端");
    assert_eq!(delivered.text, "先跑 lint", "正文逐字");
    assert!(
        steers
            .request(
                &format!("sdk-{}", running.session_id),
                steer_message("错键")
            )
            .is_err(),
        "引擎侧 sdk- 标识不是本表键（键 = core 铸会话 id）"
    );
}

/// 键形态（正向，参数化 2 形态）：`SessionRef::New` 的登记键为 `ses-` 前缀的
/// core 铸 id；`SessionRef::Continue{id}` 的登记键 = 引用 id（同会话同键）。
#[tokio::test]
async fn begin_turn登记键形态_new为core铸id_continue沿用引用id() {
    // 形态①：New → `ses-` 前缀
    let (sink, _calls) = FakeSink::new();
    let steers = Arc::new(SteerRegistry::default());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::clone(&steers),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));
    let fresh = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("New begin 应成功");
    assert!(
        fresh.session_id.starts_with("ses-"),
        "New 形态键为 core 铸 id（非引擎侧 sdk- 形态），实际: {}",
        fresh.session_id
    );
    steers
        .request(&fresh.session_id, steer_message("New 轮"))
        .expect("New 形态可送达");

    // 形态②：Continue{id} → 键 = 引用 id
    let continued = kernel
        .begin_turn(
            runner.clone(),
            request(SessionRef::Continue {
                id: "ses-既有-1".to_owned(),
            }),
        )
        .expect("Continue begin 应成功");
    assert_eq!(continued.session_id, "ses-既有-1", "沿用引用 id");
    steers
        .request("ses-既有-1", steer_message("Continue 轮"))
        .expect("Continue 形态以引用 id 为键可送达");
}

/// 两收敛态除名（正向 / 异常，组合迭代 2 形态：Completed / Stopped）：收敛后
/// `request` → `Err`（文案逐字），且 `StopRegistry.request_stop` 同键 miss（两表
/// 同一出口）。
#[tokio::test]
async fn drive收敛后steer与stop两表同址除名() {
    for (label, events, stopping) in [
        (
            "Completed",
            vec![message_kind(), turn_done_kind(false)],
            false,
        ),
        ("Stopped", vec![message_kind()], true),
    ] {
        let (sink, _calls) = FakeSink::new();
        let stops = Arc::new(StopRegistry::new());
        let steers = Arc::new(SteerRegistry::default());
        let kernel = SessionKernel::new(
            Arc::new(sink),
            Arc::clone(&stops),
            Arc::new(SessionStreamRegistry::new()),
            Arc::new(AskRegistry::default()),
            Arc::clone(&steers),
        );
        let runner = Arc::new(FakeRunner::with_capacity(8));
        let running = kernel
            .begin_turn(runner.clone(), request(SessionRef::New))
            .expect("begin 应成功");
        let session_id = running.session_id.clone();
        steers
            .request(&session_id, steer_message("轮中可送达"))
            .expect("运行中可送达");

        if stopping {
            assert!(stops.request_stop(&session_id), "停止置位");
        }
        runner.feed(events).await;
        runner.close_stream();
        let (_observations, _finished, outcome) = drive_all(running).await;
        assert_eq!(
            outcome.status,
            if stopping {
                AgentRunStatus::Stopped
            } else {
                AgentRunStatus::Completed
            },
            "{label} 收敛"
        );

        let error = steers
            .request(&session_id, steer_message("迟到补充说明"))
            .expect_err("{label}：收敛后请求 Err");
        assert_eq!(error, "会话已收口，补充说明不可送达", "{label} 文案逐字");
        assert!(
            !stops.request_stop(&session_id),
            "{label}：StopRegistry 该键亦已除名（两表同一出口）"
        );
    }
}

/// 只除本会话键（边界，组合迭代 2 会话 × 2 收敛序）：两会话各持独立假 runner
/// 并行 begin（共享同一张 steer 表）→ 首会话收敛后 `request(A)` → `Err`、
/// `request(B)` → `Ok`。
#[tokio::test]
async fn 收敛只除本会话steer键不波及并行会话() {
    for first in ["A", "B"] {
        let (sink, _calls) = FakeSink::new();
        let steers = Arc::new(SteerRegistry::default());
        let kernel = SessionKernel::new(
            Arc::new(sink),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::new(AskRegistry::default()),
            Arc::clone(&steers),
        );
        let runner_a = Arc::new(FakeRunner::with_capacity(8));
        let runner_b = Arc::new(FakeRunner::with_capacity(8));
        let running_a = kernel
            .begin_turn(runner_a.clone(), request(SessionRef::New))
            .expect("A begin 应成功");
        let session_a = running_a.session_id.clone();
        let running_b = kernel
            .begin_turn(runner_b.clone(), request(SessionRef::New))
            .expect("B begin 应成功");
        let session_b = running_b.session_id.clone();
        assert_ne!(session_a, session_b, "两会话 id 互异");

        let (first_running, first_runner, first_id) = if first == "A" {
            (running_a, runner_a.clone(), session_a.clone())
        } else {
            (running_b, runner_b.clone(), session_b.clone())
        };
        first_runner
            .feed(vec![message_kind(), turn_done_kind(false)])
            .await;
        first_runner.close_stream();
        let (_obs, _fin, outcome) = drive_all(first_running).await;
        assert_eq!(outcome.status, AgentRunStatus::Completed, "{first} 收敛");

        assert!(
            steers.request(&first_id, steer_message("迟到")).is_err(),
            "{first} 收敛后除名"
        );
        let survivor = if first == "A" { &session_b } else { &session_a };
        steers
            .request(survivor, steer_message("仍可送达"))
            .expect("并行会话仍在跑（键隔离）");
    }
}

/// 实例不共享（边界）：两个 `SteerRegistry` 实例各建一 kernel → A 的
/// `begin_turn` 后，B 实例上 `request` → `Err`（组合根注入的才是可送达的表）。
#[tokio::test]
async fn 注入实例不共享_异实例请求err() {
    let (sink, _calls) = FakeSink::new();
    let injected = Arc::new(SteerRegistry::default());
    let other = Arc::new(SteerRegistry::default());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::clone(&injected),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));
    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");

    injected
        .request(&running.session_id, steer_message("注入表"))
        .expect("注入实例即内核登记的那张表");
    let error = other
        .request(&running.session_id, steer_message("第二张表"))
        .expect_err("异实例不命中");
    assert_eq!(error, "会话已收口，补充说明不可送达");
}

/// 开轮失败不登记（异常，参数化 2 失败点）：假 runner `open` Err 与轮行写入
/// 失败 → `begin_turn` 返 `Err` → 该会话键零条目（`request` → `Err`，无残留）。
#[tokio::test]
async fn begin_turn失败时该会话键零条目() {
    let known = "ses-已知-1";
    // 形态①：open Err
    let (sink, _calls) = FakeSink::new();
    let steers = Arc::new(SteerRegistry::default());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::clone(&steers),
    );
    let failed_open = kernel.begin_turn(
        Arc::new(FakeRunner::failing(AgentStartError::ConfigMissing(
            "缺 provider".to_owned(),
        ))),
        request(SessionRef::Continue {
            id: known.to_owned(),
        }),
    );
    assert!(failed_open.is_err(), "open Err 传播");
    assert!(
        steers.request(known, steer_message("无主")).is_err(),
        "open 失败零登记"
    );

    // 形态②：轮行写入失败（sink begin_turn Err）
    let (mut failing_sink, _calls_b) = FakeSink::new();
    failing_sink.begin_turn_fails = true;
    let steers_b = Arc::new(SteerRegistry::default());
    let kernel_b = SessionKernel::new(
        Arc::new(failing_sink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(AskRegistry::default()),
        Arc::clone(&steers_b),
    );
    let failed_turn = kernel_b.begin_turn(
        Arc::new(FakeRunner::with_capacity(8)),
        request(SessionRef::Continue {
            id: known.to_owned(),
        }),
    );
    assert!(failed_turn.is_err(), "轮行写入失败传播");
    assert!(
        steers_b.request(known, steer_message("无主")).is_err(),
        "开轮失败零登记（无残留）"
    );
}

/// 登记序与既有同址（边界）：`begin_turn` 后 stop / streams / steer 三表同键
/// 在场（三步登记各在位），且除名点唯一（`converge` 收口后同键同址消失）。
#[tokio::test]
async fn begin_turn三步登记在位且converge唯一除名点() {
    let (sink, _calls) = FakeSink::new();
    let stops = Arc::new(StopRegistry::new());
    let streams = Arc::new(SessionStreamRegistry::new());
    let steers = Arc::new(SteerRegistry::default());
    let kernel = SessionKernel::new(
        Arc::new(sink),
        Arc::clone(&stops),
        Arc::clone(&streams),
        Arc::new(AskRegistry::default()),
        Arc::clone(&steers),
    );
    let runner = Arc::new(FakeRunner::with_capacity(8));
    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    let session_id = running.session_id.clone();

    assert!(
        streams.subscribe(&session_id, 0).is_some(),
        "streams 条目在位"
    );
    assert!(
        stops.request_stop(&session_id),
        "stop 条目在位（三步登记同址）"
    );
    steers
        .request(&session_id, steer_message("三表同键"))
        .expect("steer 条目在位");

    runner.feed(vec![message_kind()]).await;
    runner.close_stream();
    let (_obs, _fin, outcome) = drive_all(running).await;
    assert_eq!(outcome.status, AgentRunStatus::Stopped, "停止置位收敛");

    assert!(
        !stops.request_stop(&session_id),
        "converge 后 stop 除名（唯一除名点）"
    );
    assert!(
        steers.request(&session_id, steer_message("迟到")).is_err(),
        "converge 后 steer 除名（同址同出口）"
    );
}
