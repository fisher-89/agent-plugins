//! `kernel` 的单元测试（AC-2 / AC-5 / AC-6 / AC-7 / AC-9）：SessionKernel
//! begin_turn 同步段（open 失败不落库 → New 建会话行 → 开轮行 → 停止登记 →
//! 提前 resolve）、RunningTurn drive 泵驱动（盖戳 → delta 只上输出 → 密封
//! write-through → 双 id 落库 → TurnDone 统计收口 → 终态除名）与 StopRegistry
//! 治理面。注入依赖为假 runner / 假 sink（内存记录），RunHandle 真实参与，
//! 无文件/网络边界。

use std::sync::{Arc, Mutex};

use crate::event::{AgentDelta, AgentEvent, AgentEventKind};
use crate::kernel::{KernelOutput, SessionKernel, StopRegistry, TurnRequest};
use crate::port::{SessionSink, TurnOutcome};
use crate::runner::{
    AgentPermissionMode, AgentRunStatus, AgentRunner, AgentSession, AgentStartError, RunHandle,
    SessionCtx, SessionInjections, SessionOpen, SessionRef, TurnQuestion,
};
use crate::session::{NewSessionRow, SessionProvenance};

// ---------------------------------------------------------------------------
// 装置：假 sink（调用序 + 可编程失败）、假 runner（预录观察 + 可编程 open Err）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum SinkCall {
    CreateSession(NewSessionRow),
    BeginTurn { session_id: String, started_at: i64 },
    AppendSealed { session_id: String, seq: u64 },
    FinishTurn { turn_id: i64, status: AgentRunStatus },
    BindRemote { session_id: String, remote: String, updated_at: i64 },
}

/// 假 sink：共享调用记录，begin_turn 返回自增轮 id（sink 分配序），可编程
/// 失败点（`append_sealed` 第 n 次调用起失败等，按方法名 + 序号指定）。
struct FakeSink {
    calls: Arc<Mutex<Vec<SinkCall>>>,
    next_turn_id: Mutex<i64>,
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

    fn append_sealed(&self, session_id: &str, event: &AgentEvent) -> Result<(), String> {
        self.record(SinkCall::AppendSealed {
            session_id: session_id.to_owned(),
            seq: event.seq,
        })?;
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
    _holders: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl FakeRunner {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
            failure: None,
            capacity,
            feeders: Mutex::new(Vec::new()),
            _holders: Mutex::new(Vec::new()),
        }
    }

    fn failing(failure: AgentStartError) -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
            failure: Some(failure),
            capacity: 16,
            feeders: Mutex::new(Vec::new()),
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
        let (observation_tx, observation_rx) =
            tokio::sync::mpsc::channel(self.capacity.max(1));
        let (question_tx, mut question_rx) = tokio::sync::mpsc::channel::<TurnQuestion>(4);
        // 问题接收端持有任务：保持 ask 通道打开（问题投递恒成功），句柄置位
        // 或通道关闭时退出
        let holder_handle = tokio::spawn(async move {
            while question_rx.recv().await.is_some() {}
        });
        self.feeders
            .lock()
            .expect("投喂锁不可中毒")
            .push(observation_tx);
        self._holders
            .lock()
            .expect("持有锁不可中毒")
            .push(holder_handle);
        Ok(AgentSession {
            observations: observation_rx,
            questions: question_tx,
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
        },
        ctx: SessionCtx {
            workspace_root: std::path::PathBuf::from("D:\\工作区"),
            permission_mode: AgentPermissionMode::BypassPermissions,
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
        role: "assistant".to_owned(),
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
        usage: serde_json::json!({ "inputTokens": 10, "outputTokens": 20 }),
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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::clone(&registry));

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
    let kernel = SessionKernel::new(Arc::new(sink), registry);

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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::new(StopRegistry::new()));
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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::new(StopRegistry::new()));
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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::clone(&registry));

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
// drive：delta 只上传输面（AC-2 内核半边 / AC-5 盖戳治理单点）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drive增量只上输出回调_盖戳补齐单调且sink零append() {
    let (sink, calls) = FakeSink::new();
    let kernel = SessionKernel::new(Arc::new(sink), Arc::new(StopRegistry::new()));
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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::new(StopRegistry::new()));
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
        observations.iter().map(|event| event.seq).collect::<Vec<_>>(),
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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::clone(&registry));
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
        serde_json::json!({ "inputTokens": 10, "outputTokens": 20 })
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
    *sink
        .append_failures_from
        .lock()
        .expect("失败锁不可中毒") = Some(2); // 第 2 条密封追加起失败
    let registry = Arc::new(StopRegistry::new());
    let kernel = SessionKernel::new(Arc::new(sink), Arc::clone(&registry));
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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::clone(&registry));
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
    assert_eq!(
        outcome.error, None,
        "用户主动终止非失败：error 不记因"
    );
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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::new(StopRegistry::new()));
    let runner = Arc::new(FakeRunner::with_capacity(4));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    runner.close_stream(); // 引擎立即 EOF，无任何观察

    let (observations, _finished, outcome) = drive_all(running).await;

    assert!(observations.is_empty(), "空流零观察");
    assert_eq!(outcome.status, AgentRunStatus::Failed, "无收敛事件兜底 failed");
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
    let kernel = SessionKernel::new(Arc::new(sink), Arc::new(StopRegistry::new()));
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
    assert_eq!(seqs, expected, "seq 全程单调无跳号（delta 占号共享单调空间）");
    assert_eq!(outcome.status, AgentRunStatus::Completed, "洪峰后正常收敛");
}

#[tokio::test]
async fn drive_on_output慢速消费时密封落库与收口时序不受阻塞影响() {
    let (sink, calls) = FakeSink::new();
    let kernel = SessionKernel::new(Arc::new(sink), Arc::new(StopRegistry::new()));
    let runner = Arc::new(FakeRunner::with_capacity(16));

    let running = kernel
        .begin_turn(runner.clone(), request(SessionRef::New))
        .expect("begin 应成功");
    runner.feed(vec![message_kind(), turn_done_kind(false)]).await;
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
