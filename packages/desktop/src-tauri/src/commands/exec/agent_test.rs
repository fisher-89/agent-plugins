//! `start_agent_run()` / `drive_agent_run()` 编排的单元 + 「编排 → tee 双路
//! （Channel 实时 + store 落库）与状态收敛」集成关系测试（AC-1/AC-3/AC-4/AC-7，
//! 决策 D1/D2/D5/D7/D8）。
//!
//! 本模块声明于 agent.rs 内（子模块），可触达私有失败收敛助手；dev-team 为
//! 纯 binary crate，共置沿 workspaces/mod_test.rs 既有先例。假 runner 测试
//! 替身注入泛型缝；AppHandle 以 `tauri::test::mock_app()`（MockRuntime）托管
//! Store 与 RunStopRegistry——编排对 runtime 泛型，生产经命令注入 Wry 句柄；
//! Channel 以 `tauri::ipc::Channel::new` 捕获回调（或返回 Err 构造「页面已关」）；
//! store 不 mock（tempdir 真库）。提前 resolve 契约下同步段立即返回 running
//! 记录、终态经 Channel Record 信封流出：tee 双路与 EOF 收敛语义以
//! [`drive_agent_run`] 直驱锁定（同步 await 无竞态），spawn 链路（注册句柄 →
//! 停止信号触达租户 → 后台收敛 → Record 流出 → 除名）以停止信号端到端用例
//! 锁定。真实 claude 进程不引入（见 test-design 不可测试项）。

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::{
    AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRun, AgentRunParams,
    AgentRunStatus, AgentRunner, AgentStartError, RunHandle,
};
use store::{AgentRunRecord, Store};

use super::{
    abort_with_store_failure, drive_agent_run, running_record, start_agent_run,
    start_agent_run_with, AgentRunMessage, RunProvenance, RunStopRegistry,
};

/// PATH 环境变量修改串行化（agent_start 同款用例经 mod_test 共享此锁）。
pub(crate) static PATH_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// 装置：tempdir 真库、mock app 托管态、假 runner、可编程 Channel
// ---------------------------------------------------------------------------

fn temp_store(tag: &str) -> (tempfile::TempDir, Store) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("agent-exec-test-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败");
    let db_path = dir.path().join("test.redb");
    let store = Store::open(&db_path).expect("打开测试 db 失败");
    (dir, store)
}

/// mock app（MockRuntime）托管 tempdir 真库与默认注册表（temp_store 的托管
/// 形态）：dir 由调用方持活于测例作用域，store 经 `app.state::<Store>()` 取
/// 用——编排同步段入参与后台任务取到的托管句柄指向同一实例；注册表须先于
/// start 托管（同步段 `app.state::<RunStopRegistry>()` 未托管会 panic）。
pub(crate) fn temp_app_store(tag: &str) -> (tempfile::TempDir, App<tauri::test::MockRuntime>) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("agent-exec-test-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败");
    let app = tauri::test::mock_app();
    let store = Store::open(&dir.path().join("test.redb")).expect("打开测试 db 失败");
    app.manage(store);
    app.manage(RunStopRegistry::default());
    (dir, app)
}

fn params(cwd: &Path) -> AgentRunParams {
    AgentRunParams {
        prompt: "帮我跑一轮 loop".to_owned(),
        cwd: cwd.to_path_buf(),
        permission_mode: AgentPermissionMode::BypassPermissions,
        resume_session_id: None,
    }
}

fn run_started(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::RunStarted {
            model: Some("claude-opus".to_owned()),
            session_id: Some("s-1".to_owned()),
            tools: vec!["Bash".to_owned()],
            mcp_servers: Vec::new(),
        },
    )
}

fn message(seq: u64, parent: Option<&str>) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: Vec::new(),
            parent_tool_use_id: parent.map(str::to_owned),
        },
    )
}

fn run_result(seq: u64, is_error: bool, num_turns: Option<u64>) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::RunResult {
            subtype: if is_error {
                "error_max_turns"
            } else {
                "success"
            }
            .to_owned(),
            is_error,
            num_turns,
            duration_ms: Some(1234),
            cost_usd: Some(0.5),
            usage: serde_json::json!({ "input_tokens": 10 }),
            session_id: Some("s-1".to_owned()),
        },
    )
}

/// 假 runner：预录事件经 mpsc 交付（后台任务投递），或返回可控启动错误。
/// `pub(crate)`：mod_test 的链路级用例（集成关系 R2/R3）经 `start_agent_run_with`
/// 泛型缝复用同一装置。
pub(crate) struct FakeRunner {
    events: Vec<AgentEvent>,
    failure: Option<AgentStartError>,
    /// start 收到的入参（命令面组装断言的捕获缝，mod_test 复用）。
    captured_params: Mutex<Vec<AgentRunParams>>,
    /// 事件交付后持流等待停止信号：置位时租户不主动 EOF（流保持打开直至
    /// `wait_requested` 返回）——stopped 收敛用例的确定性装置，EOF 时序由
    /// 停止信号单边决定、无投递竞速。
    hold_until_stop: bool,
}

impl FakeRunner {
    pub(crate) fn with_events(events: Vec<AgentEvent>) -> Self {
        Self {
            events,
            failure: None,
            captured_params: Mutex::new(Vec::new()),
            hold_until_stop: false,
        }
    }

    /// 预录事件交付后流保持打开直至停止信号（停止信号端到端用例专用）。
    pub(crate) fn with_events_held_until_stop(events: Vec<AgentEvent>) -> Self {
        Self {
            events,
            failure: None,
            captured_params: Mutex::new(Vec::new()),
            hold_until_stop: true,
        }
    }

    fn failing(failure: AgentStartError) -> Self {
        Self {
            events: Vec::new(),
            failure: Some(failure),
            captured_params: Mutex::new(Vec::new()),
            hold_until_stop: false,
        }
    }

    /// start 已收到的入参快照（按调用序）。
    pub(crate) fn captured_params(&self) -> Vec<AgentRunParams> {
        self.captured_params
            .lock()
            .expect("参数捕获锁不可中毒")
            .clone()
    }
}

impl AgentRunner for FakeRunner {
    fn start(&self, params: AgentRunParams) -> Result<AgentRun, AgentStartError> {
        self.captured_params
            .lock()
            .expect("参数捕获锁不可中毒")
            .push(params);
        if let Some(failure) = self.failure.clone() {
            return Err(failure);
        }
        let (sender, receiver) = tokio::sync::mpsc::channel(self.events.len().max(1));
        let events = self.events.clone();
        let pump_handle = RunHandle::default();
        let hold = self.hold_until_stop;
        let pump_stop = pump_handle.clone();
        tokio::spawn(async move {
            for event in events {
                let _ = sender.send(event).await;
            }
            if hold {
                // 租户泵语义：流保持打开直至终止信号（不主动 EOF）
                pump_stop.wait_requested().await;
            }
            // sender drop → EOF
        });
        Ok(AgentRun {
            events: receiver,
            handle: pump_handle,
        })
    }
}

/// 捕获型 Channel：逐信封收下序列化 JSON（tee 实时路快照）。
pub(crate) fn capturing_channel() -> (Channel<AgentRunMessage>, Arc<Mutex<Vec<serde_json::Value>>>)
{
    let captured = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&captured);
    let channel = Channel::new(move |body: InvokeResponseBody| {
        if let InvokeResponseBody::Json(text) = body {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                sink.lock().expect("捕获锁不可中毒").push(value);
            }
        }
        Ok(())
    });
    (channel, captured)
}

/// 丢弃型 Channel：发送端恒失败（模拟页面已关，D1「发送失败不中断落库」）。
fn failing_channel() -> Channel<AgentRunMessage> {
    Channel::new(|_| {
        Err(tauri::Error::Io(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "页面已关闭",
        )))
    })
}

/// 摘出实时事件体（信封剥壳）：Channel 实时路与落库路对读的事件序列。
pub(crate) fn pushed_event_bodies(
    captured: &Mutex<Vec<serde_json::Value>>,
) -> Vec<serde_json::Value> {
    captured
        .lock()
        .expect("捕获锁不可中毒")
        .iter()
        .filter(|value| value["ipc"] == "event")
        .map(|value| value["event"].clone())
        .collect()
}

/// 轮询捕获 Channel 直至终态 Record 信封流出（提前 resolve 契约的观测半边：
/// 后台任务无句柄可 join，终态以 Channel Record 为准；后台任务在 tauri 全局
/// 运行时上执行、假 runner 泵在测试运行时上执行，轮询以让步推进两侧行进）。
/// 超时 panic（后台任务未收敛）。
pub(crate) async fn wait_terminal_record(
    captured: &Mutex<Vec<serde_json::Value>>,
) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let found = captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .find(|value| value["ipc"] == "record")
            .cloned();
        if let Some(record) = found {
            return record;
        }
        assert!(
            Instant::now() < deadline,
            "等待终态 Record 超时（后台任务未收敛）"
        );
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// 泛型缝发起 + 等待终态（提前 resolve 契约的完整驱动）：start 即返回 running
/// 记录（id 立即可用），随后轮询捕获 Channel 至终态 Record 信封流出。
/// 返回 [发起时 running 记录, 终态记录]。
pub(crate) async fn start_and_wait_terminal<R: AgentRunner + 'static>(
    app: &App<tauri::test::MockRuntime>,
    store: &Store,
    runner: &R,
    captured: &Mutex<Vec<serde_json::Value>>,
    channel: Channel<AgentRunMessage>,
    params: AgentRunParams,
    provenance: RunProvenance,
) -> (AgentRunRecord, AgentRunRecord) {
    let running = start_agent_run_with(
        app.handle().clone(),
        store,
        runner,
        channel,
        params,
        provenance,
    )
    .expect("start 提前 resolve running 记录");
    let terminal_value = wait_terminal_record(captured).await;
    let terminal: AgentRunRecord =
        serde_json::from_value(terminal_value["record"].clone()).expect("Record 信封携带终态记录");
    (running, terminal)
}

/// drive 直驱用例的底座（编排前半边替身，`start_agent_run_with` 同款 begin
/// 语义）：running 形态记录 begin 落库分配 id（三字段直写契约枚举）。
fn begin_running(store: &Store) -> AgentRunRecord {
    let running = AgentRunRecord {
        id: 0,
        prompt: "帮我跑一轮 loop".to_owned(),
        cwd: "C:\\ws".to_owned(),
        env: AgentEnvMode::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
        status: AgentRunStatus::Running,
        started_at: 1727000000000,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
        source: "debug".to_owned(),
        source_ref: None,
        parent_run_id: None,
    };
    store.begin_agent_run(&running).expect("begin 应成功")
}

// ---------------------------------------------------------------------------
// drive_agent_run：预录会话 tee 双 sink + 状态收敛（AC-4 tee 双 sink）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 预录completed会话经tee双sink全链路落库且channel逐事件一致() {
    let (_dir, store) = temp_store("tee-ok");
    let prerecorded = vec![
        run_started(0),
        message(1, None),
        message(2, Some("tu_1")),
        run_result(3, false, Some(3)),
    ];
    let runner = FakeRunner::with_events(prerecorded);
    let registry = RunStopRegistry::default();
    let (channel, captured) = capturing_channel();

    let run = runner
        .start(params(Path::new("C:\\ws")))
        .expect("假 runner 启动成功");
    let record = begin_running(&store);
    registry.register(record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    assert_eq!(
        record.status,
        AgentRunStatus::Completed,
        "RunResult 驱动收敛 completed"
    );
    assert_eq!(record.num_turns, Some(3), "汇总字段摘自 RunResult 事件");
    assert_eq!(record.cost_usd, Some(0.5));
    assert_eq!(record.duration_ms, Some(1234));
    assert_eq!(record.session_id.as_deref(), Some("s-1"));
    assert!(record.finished_at.is_some(), "终态落 finished_at");

    // store sink：事件全量落库，key (run_id, seq)，seq 升序
    let stored = store.list_agent_run_events(record.id).unwrap();
    assert_eq!(stored.len(), 4, "四事件全量落库");
    let stored_seqs: Vec<u64> = stored.iter().map(|event| event.seq).collect();
    assert_eq!(stored_seqs, vec![0, 1, 2, 3]);

    // Channel sink：逐信封剥壳后逐事件一致（同 seq 同内容，两路 seq 一致）
    assert_eq!(
        serde_json::Value::Array(pushed_event_bodies(&captured)),
        serde_json::to_value(&stored).unwrap(),
        "Channel 事件序列与落库事件序列逐条一致"
    );

    // Channel Record 信封 JSON 逐字 "completed"（提前 resolve 契约的终态半边）
    let terminal = wait_terminal_record(&captured).await;
    assert_eq!(
        terminal["record"]["status"],
        serde_json::json!("completed"),
        "信封出线 status 逐字 completed（serde camelCase 值域）"
    );

    // 落库 run 行为 completed 终态；注册表终态除名（停止寻址幂等 miss）
    let listed = store.list_agent_runs().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, record.id);
    assert_eq!(listed[0].status, AgentRunStatus::Completed);
    assert!(!registry.request_stop(record.id), "终态除名后停止寻址 miss");
}

#[tokio::test]
async fn 预录is_error的result时返回ok的failed记录而非err() {
    let (_dir, store) = temp_store("tee-failed");
    let runner = FakeRunner::with_events(vec![run_result(0, true, Some(2))]);
    let registry = RunStopRegistry::default();
    let (channel, _captured) = capturing_channel();

    let run = runner
        .start(params(Path::new("C:\\ws")))
        .expect("假 runner 启动成功");
    let record = begin_running(&store);
    registry.register(record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    assert_eq!(
        record.status,
        AgentRunStatus::Failed,
        "D7：in-band 失败收敛 failed 记录"
    );
    assert_eq!(record.num_turns, Some(2));
    assert!(record.finished_at.is_some());
    assert_eq!(
        store.list_agent_runs().unwrap()[0].status,
        AgentRunStatus::Failed
    );
}

// ---------------------------------------------------------------------------
// drive_agent_run：EOF 收敛三分支（状态机为准 / 停止信号 stopped / 兜底 failed）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eof时状态机已由run_result收敛则以状态机为准stop请求晚到不改写() {
    let (_dir, store) = temp_store("drive-machine-first");
    let runner = FakeRunner::with_events(vec![run_result(0, false, Some(1))]);
    let registry = RunStopRegistry::default();
    let (channel, _captured) = capturing_channel();

    let run = runner
        .start(params(Path::new("C:\\ws")))
        .expect("假 runner 启动成功");
    run.handle.request_stop();
    let record = begin_running(&store);
    registry.register(record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    assert_eq!(
        record.status,
        AgentRunStatus::Completed,
        "EOF 时状态机已收敛：以状态机为准，停止信号不改写"
    );
}

#[tokio::test]
async fn eof无result但停止信号已置位时显式收敛stopped且不记因() {
    let (_dir, store) = temp_store("drive-stopped");
    let runner = FakeRunner::with_events(vec![run_started(0)]);
    let registry = RunStopRegistry::default();
    let (channel, captured) = capturing_channel();

    let run = runner
        .start(params(Path::new("C:\\ws")))
        .expect("假 runner 启动成功");
    run.handle.request_stop();
    let record = begin_running(&store);
    registry.register(record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    assert_eq!(
        record.status,
        AgentRunStatus::Stopped,
        "停止信号显式收敛 stopped"
    );
    assert_eq!(record.error, None, "用户主动终止非失败：error 不记因");
    assert!(record.finished_at.is_some(), "终态落 finished_at");
    // 终态 Record 信封流出（提前 resolve 契约的终态半边）
    let pushed = captured.lock().expect("捕获锁不可中毒").clone();
    assert_eq!(pushed.len(), 2, "一事件一终态 Record");
    assert_eq!(pushed[1]["ipc"], "record", "收尾信封为 Record");
    assert_eq!(
        pushed[1]["record"]["status"],
        serde_json::json!("stopped"),
        "信封出线值逐字 stopped（用户主动终止非失败语义不变）"
    );
    // 注册表终态除名
    assert!(!registry.request_stop(record.id), "终态除名后停止寻址 miss");
}

#[tokio::test]
async fn 既无result又无stop请求的eof兜底收敛failed且error记因() {
    let (_dir, store) = temp_store("drive-fallback");
    let runner = FakeRunner::with_events(vec![run_started(0)]);
    let registry = RunStopRegistry::default();
    let (channel, _captured) = capturing_channel();

    let run = runner
        .start(params(Path::new("C:\\ws")))
        .expect("假 runner 启动成功");
    let record = begin_running(&store);
    registry.register(record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    assert_eq!(
        record.status,
        AgentRunStatus::Failed,
        "进程异常终止无 result：兜底收敛 failed"
    );
    assert_eq!(
        record.error.as_deref(),
        Some("进程结束但未产出 result 事件"),
        "兜底失败记因"
    );
}

// ---------------------------------------------------------------------------
// 启动失败：泛型缝 Err 不留行（D5）与 Channel 已关 / seq 乱序透传（D1/D8）
// ---------------------------------------------------------------------------

#[test]
fn 假runner启动失败时返回err且store零run行() {
    let (_dir, store) = temp_store("tee-start-err");
    // 启动失败路径不触达托管状态（Err 先于 begin 与注册表访问），仅取句柄
    let app = tauri::test::mock_app();
    let runner = FakeRunner::failing(AgentStartError::CliMissing("PATH 上未发现".to_owned()));
    let (channel, captured) = capturing_channel();

    let result = start_agent_run_with(
        app.handle().clone(),
        &store,
        &runner,
        channel,
        params(Path::new("C:\\ws")),
        RunProvenance::debug(),
    );

    let err = result.expect_err("启动阶段失败必须 Err");
    assert!(
        err.contains("CLI"),
        "Err(String) 携带启动失败原因，实际: {err}"
    );
    assert!(
        store.list_agent_runs().unwrap().is_empty(),
        "D5：启动失败不留 run 行"
    );
    assert!(
        captured.lock().unwrap().is_empty(),
        "启动失败不推送任何事件"
    );
}

#[tokio::test]
async fn channel接收端先行关闭时落库继续完整且编排返回最终记录() {
    let (_dir, store) = temp_store("tee-channel-closed");
    let prerecorded = vec![
        run_started(0),
        message(1, None),
        run_result(2, false, Some(1)),
    ];
    let runner = FakeRunner::with_events(prerecorded);
    let registry = RunStopRegistry::default();
    // 接收端恒失败（页面已关）：tee 不得中断落库
    let channel = failing_channel();

    let run = runner
        .start(params(Path::new("C:\\ws")))
        .expect("假 runner 启动成功");
    let record = begin_running(&store);
    registry.register(record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    assert_eq!(record.status, AgentRunStatus::Completed);
    let stored = store.list_agent_run_events(record.id).unwrap();
    assert_eq!(stored.len(), 3, "D1：落库完整不因 Channel 失败而中断");
}

#[tokio::test]
async fn seq缺口乱序时tee透传不重排落库key与事件自带seq一致() {
    let (_dir, store) = temp_store("tee-seq-order");
    let prerecorded = vec![
        message(5, None),
        message(1, None),
        message(9, None),
        run_result(2, false, Some(1)),
    ];
    let runner = FakeRunner::with_events(prerecorded);
    let registry = RunStopRegistry::default();
    let (channel, captured) = capturing_channel();

    let run = runner
        .start(params(Path::new("C:\\ws")))
        .expect("假 runner 启动成功");
    let record = begin_running(&store);
    registry.register(record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    // Channel 透传不重排：与预录顺序一致（信封剥壳后取事件 seq）
    let pushed_seqs: Vec<u64> = pushed_event_bodies(&captured)
        .iter()
        .map(|value| value["seq"].as_u64().expect("seq 为数值"))
        .collect();
    assert_eq!(pushed_seqs, vec![5, 1, 9, 2], "D8：tee 透传不重排");

    // 落库 key 与事件自带 seq 一致：重放按 key 升序（而非推送序）
    let stored = store.list_agent_run_events(record.id).unwrap();
    let stored_seqs: Vec<u64> = stored.iter().map(|event| event.seq).collect();
    assert_eq!(stored_seqs, vec![1, 2, 5, 9], "复合键升序重放");
}

// ---------------------------------------------------------------------------
// spawn 链路端到端：提前 resolve running 记录 + 注册表停止信号触达租户 →
// 后台收敛 stopped → Record 流出 → 除名（AC-7）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stop请求经注册表触达租户后后台收敛stopped并流出record且除名() {
    let (_dir, app) = temp_app_store("stop-e2e");
    let state = app.state::<Store>();
    let registry = app.state::<RunStopRegistry>();
    let runner = FakeRunner::with_events_held_until_stop(vec![run_started(0)]);
    let (channel, captured) = capturing_channel();

    // 提前 resolve：start 即返回 running 记录（id 立即可用，终态未落）
    let running = start_agent_run_with(
        app.handle().clone(),
        state.inner(),
        &runner,
        channel,
        params(Path::new("C:\\ws")),
        RunProvenance::debug(),
    )
    .expect("start 提前 resolve running 记录");
    assert_eq!(
        running.status,
        AgentRunStatus::Running,
        "返回即 running（提前 resolve）"
    );
    assert_ne!(running.id, 0, "begin 已分配 id，agent_stop 可寻址");
    assert_eq!(running.finished_at, None, "调用方不被阻塞至终态");

    // 停止寻址命中（同步段已登记句柄）：信号触达租户泵（wait_requested 半边）
    assert!(registry.request_stop(running.id), "注册表已登记运行中句柄");

    let record_value = wait_terminal_record(&captured).await;
    let record: AgentRunRecord =
        serde_json::from_value(record_value["record"].clone()).expect("Record 信封携带终态记录");
    assert_eq!(
        record.status,
        AgentRunStatus::Stopped,
        "停止信号收敛 stopped"
    );
    assert_eq!(record.error, None, "用户主动终止不记因");
    assert!(record.finished_at.is_some(), "终态落 finished_at");
    assert_eq!(
        state.list_agent_runs().unwrap()[0].status,
        AgentRunStatus::Stopped,
        "终态整行替换落库"
    );
    assert!(
        !registry.request_stop(running.id),
        "终态除名后停止寻址 miss"
    );
}

// ---------------------------------------------------------------------------
// 薄入口：隔离 PATH 走完整链（组装 ClaudeCliRunner → start 失败）
// ---------------------------------------------------------------------------

#[test]
fn start_agent_run隔离path时走薄入口全链返回err且store无run行() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let (_dir, store) = temp_store("thin-entry");
    let app = tauri::test::mock_app();
    let empty_path = tempfile::tempdir().expect("创建空 PATH 目录失败");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", empty_path.path());
    let result = start_agent_run(
        app.handle().clone(),
        &store,
        capturing_channel().0,
        params(Path::new("C:\\ws")),
        RunProvenance::debug(),
    );
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let err = result.expect_err("CLI 不可发现必须 Err（AC-4）");
    assert!(
        err.contains("CLI"),
        "Err(String) 透传 AgentStartError 文案，实际: {err}"
    );
    assert!(
        store.list_agent_runs().unwrap().is_empty(),
        "D5：不留 run 行"
    );
}

// ---------------------------------------------------------------------------
// store 写失败的失败收敛助手（D1：落库兜底失败不可静默）
// ---------------------------------------------------------------------------

#[test]
fn store写失败的收敛助手将run收敛failed且error记因并尽力落终态行() {
    // 说明：redb 4.3 拒绝同文件双开写句柄（DatabaseAlreadyOpen），「以并行写
    // 事务独占 redb 文件」不可构造；此处直接驱动 tee 循环的失败收敛助手，
    // 锁定其收敛语义（status=failed / error 记因 / finished_at 落值 / 终态行
    // 尽力落库），tee 侧「Channel 已送达事件保留」由 channel_closed 用例承载。
    let (_dir, store) = temp_store("abort-failure");
    let run = store
        .begin_agent_run(&abort_base_record())
        .expect("begin 应成功");

    let record = abort_with_store_failure(
        &store,
        run.clone(),
        "事件落库失败: db: 模拟写入失败".to_owned(),
    );

    assert_eq!(record.status, AgentRunStatus::Failed, "run 收敛为 failed");
    assert_eq!(
        record.error.as_deref(),
        Some("事件落库失败: db: 模拟写入失败"),
        "error 记因"
    );
    assert!(record.finished_at.is_some(), "终态落 finished_at");

    let listed = store.list_agent_runs().unwrap();
    assert_eq!(listed.len(), 1, "终态行已尽力落库");
    assert_eq!(listed[0].status, AgentRunStatus::Failed);
    assert_eq!(
        listed[0].error.as_deref(),
        Some("事件落库失败: db: 模拟写入失败")
    );

    // 终态替换语义：failed 终态行覆盖 running 行（整行替换，非追加）
    assert_ne!(listed[0].status, AgentRunStatus::Running);
}

/// 收敛助手用例的底座记录（running 形态，id 已由 begin 分配；三字段直写契约枚举）。
fn abort_base_record() -> store::AgentRunRecord {
    store::AgentRunRecord {
        id: 0,
        prompt: "落库失败场景".to_owned(),
        cwd: "C:\\ws".to_owned(),
        env: AgentEnvMode::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
        status: AgentRunStatus::Running,
        started_at: 1727000000000,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
        source: "debug".to_owned(),
        source_ref: None,
        parent_run_id: None,
    }
}

// ---------------------------------------------------------------------------
// running_record 初值与 AgentRunMessage 信封线格式（枚举化跟改：直写枚举、
// as_str().to_owned() + STATUS_* 常量组装退役；信封 serde 形态零变化是 tee
// 双路对读断言的前提不变式）
// ---------------------------------------------------------------------------

#[test]
fn running_record初值三字段直写枚举且携带provenance来源() {
    // 初值三字段直写枚举（原 as_str().to_owned() + STATUS_RUNNING 组装退役）：
    // env 恒 Default、permission_mode 取入参档位变体、status 恒 Running
    let mut params = params(Path::new("C:\\ws"));
    params.permission_mode = AgentPermissionMode::AcceptEdits;
    let provenance = RunProvenance {
        source: "explore".to_owned(),
        source_ref: Some("7".to_owned()),
        parent_run_id: Some(12),
    };

    let record = running_record(&params, provenance);

    assert_eq!(
        record.env,
        AgentEnvMode::Default,
        "env 恒完整档（无 UI 输入口）"
    );
    assert_eq!(
        record.permission_mode,
        AgentPermissionMode::AcceptEdits,
        "permission_mode 直写入参档位变体"
    );
    assert_eq!(record.status, AgentRunStatus::Running, "初值恒 running");
    assert_eq!(record.source, "explore", "初值携带 provenance 来源");
    assert_eq!(record.source_ref.as_deref(), Some("7"));
    assert_eq!(record.parent_run_id, Some(12));
    assert_eq!(record.finished_at, None, "running 行无结束时间");
    assert_eq!(record.error, None);
}

#[test]
fn agent_run_message信封serde线格式双变体逐字不变() {
    // 加 specta::Type 后信封 serde JSON 形态零变化：tag `ipc` camelCase 双变体
    // {"ipc":"event","event":…} / {"ipc":"record","record":…}（tee 双路对读的
    // 前提不变式）
    let event = run_started(3);
    let event_value = serde_json::to_value(AgentRunMessage::Event {
        event: event.clone(),
    })
    .expect("Event 信封序列化应成功");
    assert_eq!(event_value["ipc"], serde_json::json!("event"));
    assert_eq!(
        event_value["event"],
        serde_json::to_value(&event).unwrap(),
        "Event 信封载荷与事件本体 JSON 同构"
    );

    let (_dir, store) = temp_store("envelope-record");
    let record = begin_running(&store);
    let record_value = serde_json::to_value(AgentRunMessage::Record {
        record: record.clone(),
    })
    .expect("Record 信封序列化应成功");
    assert_eq!(record_value["ipc"], serde_json::json!("record"));
    assert_eq!(
        record_value["record"],
        serde_json::to_value(&record).unwrap(),
        "Record 信封载荷与记录本体 JSON 同构"
    );
    assert_eq!(
        record_value["record"]["status"],
        serde_json::json!("running"),
        "记录内枚举出线为受控 camelCase 串"
    );
}
