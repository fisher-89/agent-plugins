//! `start_agent_run()` / `drive_agent_run()` 编排的单元 + 「编排 → tee 双路
//! （Channel 实时 + store 落库）与状态收敛」集成关系测试。
//!
//! 本模块声明于 agent.rs 内（子模块），可触达私有失败收敛助手；dev-team 为
//! 纯 binary crate，共置沿 workspaces/mod_test.rs 既有先例。假 runner 测试
//! 替身注入泛型缝；AppHandle 以 `tauri::test::mock_app()`（MockRuntime）托管
//! `WorkspaceStores` 与 RunStopRegistry——编排对 runtime 泛型，生产经命令注入
//! Wry 句柄；Channel 以 `tauri::ipc::Channel::new` 捕获回调（或返回 Err 构造
//! 「页面已关」）；store 不 mock（tempdir 真库，双库布局下编排经
//! `for_root` 落 cwd 对应的 workspace 库——start 泛型缝入参为 `WorkspaceStores`，
//! drive 直驱入参仍为 `&Store`）。提前 resolve 契约下同步段立即返回 running
//! 记录、终态经 Channel Record 信封流出。真实 claude 进程不引入。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::{
    AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRun, AgentRunParams,
    AgentRunStatus, AgentRunner, AgentStartError, RunHandle,
};
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord, AgentRunRecord,
    Store, WorkspaceStores,
};

use super::{
    abort_with_store_failure, drive_agent_run, find_events_by_session, resolve_agent_engine,
    running_record, start_agent_run, start_agent_run_with, AgentRunMessage, ResolvedEngine,
    RunProvenance, RunStopRegistry,
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
    // drive_agent_run 直驱用例：workspace 维度库直开（编排落库载体即 workspace 库）
    let store = Store::open_workspace(&db_path).expect("打开测试 workspace 库失败");
    (dir, store)
}

/// mock app（MockRuntime）托管 tempdir 数据根上的真实 WorkspaceStores 与默认
/// 注册表：dir 由调用方持活于测例作用域，stores 经 `app.state::<WorkspaceStores>()`
/// 取用——编排同步段经 `for_root` 解析 workspace 库（cwd 恒为当前 workspace
/// root，须为真实存在的目录，canonical 口径归一）；注册表须先于 start 托管
/// （同步段 `app.state::<RunStopRegistry>()` 未托管会 panic）。
pub(crate) fn temp_app_stores(tag: &str) -> (tempfile::TempDir, App<tauri::test::MockRuntime>) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("agent-exec-test-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败");
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app.manage(RunStopRegistry::default());
    (dir, app)
}

/// 在数据根下创建真实 workspace 目录（for_root canonicalize 的 cwd 基准）。
pub(crate) fn real_ws_dir(data_root: &Path) -> PathBuf {
    let ws = data_root.join("ws");
    fs::create_dir_all(&ws).expect("创建 workspace 目录失败");
    ws
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
/// `pub(crate)`：mod_test 的链路级用例（集成关系）经 `start_agent_run_with`
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

/// 丢弃型 Channel：发送端恒失败（模拟页面已关，「发送失败不中断落库」）。
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
    stores: &WorkspaceStores,
    runner: &R,
    captured: &Mutex<Vec<serde_json::Value>>,
    channel: Channel<AgentRunMessage>,
    params: AgentRunParams,
    provenance: RunProvenance,
) -> (AgentRunRecord, AgentRunRecord) {
    let running = start_agent_run_with(
        app.handle().clone(),
        stores,
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
// drive_agent_run：预录会话 tee 双 sink + 状态收敛
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
    registry.register("C:\\ws", record.id, RunHandle::default());

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
    assert!(
        !registry.request_stop("C:\\ws", record.id),
        "终态除名后停止寻址 miss"
    );
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
    registry.register("C:\\ws", record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    assert_eq!(
        record.status,
        AgentRunStatus::Failed,
        "in-band 失败收敛 failed 记录"
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
    registry.register("C:\\ws", record.id, RunHandle::default());

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
    registry.register("C:\\ws", record.id, RunHandle::default());

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
    assert!(
        !registry.request_stop("C:\\ws", record.id),
        "终态除名后停止寻址 miss"
    );
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
    registry.register("C:\\ws", record.id, RunHandle::default());

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
fn 假runner启动失败时返回err且workspace库零run行() {
    let dir = tempfile::Builder::new()
        .prefix("agent-exec-test-start-err-")
        .tempdir()
        .expect("创建临时目录失败");
    // 启动失败路径不触达托管状态与库解析（Err 先于 for_root 与 begin），仅取句柄
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(dir.path()).expect("打开测试全局库失败");
    let runner = FakeRunner::failing(AgentStartError::CliMissing("PATH 上未发现".to_owned()));
    let (channel, captured) = capturing_channel();

    let result = start_agent_run_with(
        app.handle().clone(),
        &stores,
        &runner,
        channel,
        params(&real_ws_dir(dir.path())),
        RunProvenance::debug(),
    );

    let err = result.expect_err("启动阶段失败必须 Err");
    assert!(
        err.contains("CLI"),
        "Err(String) 携带启动失败原因，实际: {err}"
    );
    assert!(
        stores
            .for_root(&real_ws_dir(dir.path()).to_string_lossy())
            .expect("for_root 应成功")
            .list_agent_runs()
            .unwrap()
            .is_empty(),
        "启动失败不留 run 行"
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
    registry.register("C:\\ws", record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    assert_eq!(record.status, AgentRunStatus::Completed);
    let stored = store.list_agent_run_events(record.id).unwrap();
    assert_eq!(stored.len(), 3, "落库完整不因 Channel 失败而中断");
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
    registry.register("C:\\ws", record.id, RunHandle::default());

    let record = drive_agent_run(&store, channel, run, record, &registry).await;

    // Channel 透传不重排：与预录顺序一致（信封剥壳后取事件 seq）
    let pushed_seqs: Vec<u64> = pushed_event_bodies(&captured)
        .iter()
        .map(|value| value["seq"].as_u64().expect("seq 为数值"))
        .collect();
    assert_eq!(pushed_seqs, vec![5, 1, 9, 2], "tee 透传不重排");

    // 落库 key 与事件自带 seq 一致：重放按 key 升序（而非推送序）
    let stored = store.list_agent_run_events(record.id).unwrap();
    let stored_seqs: Vec<u64> = stored.iter().map(|event| event.seq).collect();
    assert_eq!(stored_seqs, vec![1, 2, 5, 9], "复合键升序重放");
}

// ---------------------------------------------------------------------------
// spawn 链路端到端：提前 resolve running 记录 + 注册表停止信号触达租户 →
// 后台收敛 stopped → Record 流出 → 除名
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stop请求经注册表触达租户后后台收敛stopped并流出record且除名() {
    let (dir, app) = temp_app_stores("stop-e2e");
    let stores = app.state::<WorkspaceStores>();
    let registry = app.state::<RunStopRegistry>();
    // cwd 恒为当前 workspace root：for_root canonicalize 要求真实目录
    let ws_dir = real_ws_dir(dir.path());
    let root = ws_dir.to_string_lossy().into_owned();
    let runner = FakeRunner::with_events_held_until_stop(vec![run_started(0)]);
    let (channel, captured) = capturing_channel();

    // 提前 resolve：start 即返回 running 记录（id 立即可用，终态未落）
    let running = start_agent_run_with(
        app.handle().clone(),
        stores.inner(),
        &runner,
        channel,
        params(&ws_dir),
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

    // 停止寻址命中（同步段已登记 (root, run id) 复合键句柄）：信号触达租户泵
    assert!(
        registry.request_stop(&root, running.id),
        "注册表已登记运行中句柄"
    );

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
        stores
            .for_root(&root)
            .expect("for_root 应成功")
            .list_agent_runs()
            .unwrap()[0]
            .status,
        AgentRunStatus::Stopped,
        "终态整行替换落库（该 root 的 workspace 库）"
    );
    assert!(
        !registry.request_stop(&root, running.id),
        "终态除名后停止寻址 miss"
    );
}

// ---------------------------------------------------------------------------
// 薄入口：隔离 PATH 走完整链（组装 ClaudeCliRunner → start 失败）。尾参
// `engine: EngineKind::Cli` 随 DEFAULT_ENGINE 退役更新为 resolved 尾参（断言
// 保留）。
// ---------------------------------------------------------------------------

#[test]
fn start_agent_run隔离path时走薄入口全链返回err且store无run行() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let dir = tempfile::Builder::new()
        .prefix("agent-exec-test-thin-entry-")
        .tempdir()
        .expect("创建临时目录失败");
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(dir.path()).expect("打开测试全局库失败");
    let empty_path = tempfile::tempdir().expect("创建空 PATH 目录失败");

    // resolved 尾参（Cli 形态产物：kind 直选 + 空占位配置，CLI 臂不消费）
    let resolved = ResolvedEngine {
        kind: agent_runtime::EngineKind::Cli,
        config: agent_runtime::EngineConfig::empty(),
    };
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", empty_path.path());
    let result = start_agent_run(
        app.handle().clone(),
        &stores,
        capturing_channel().0,
        params(&real_ws_dir(dir.path())),
        RunProvenance::debug(),
        resolved,
    );
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let err = result.expect_err("CLI 不可发现必须 Err");
    assert!(
        err.contains("CLI"),
        "Err(String) 透传 AgentStartError 文案，实际: {err}"
    );
    assert!(
        stores
            .for_root(&real_ws_dir(dir.path()).to_string_lossy())
            .expect("for_root 应成功")
            .list_agent_runs()
            .unwrap()
            .is_empty(),
        "启动失败不留 run 行"
    );
}

// ---------------------------------------------------------------------------
// store 写失败的失败收敛助手（落库兜底失败不可静默）
// ---------------------------------------------------------------------------

#[test]
fn store写失败的收敛助手将run收敛failed且error记因并尽力落终态行() {
    // 说明：redb 拒绝同文件双开写句柄（DatabaseAlreadyOpen），「以并行写
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
// running_record 初值与 AgentRunMessage 信封线格式（信封 serde 形态零变化是
// tee 双路对读断言的前提不变式）
// ---------------------------------------------------------------------------

#[test]
fn running_record初值三字段直写枚举且携带provenance来源() {
    // 初值三字段直写枚举：env 恒 Default、permission_mode 取入参档位变体、
    // status 恒 Running
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
    // 信封 serde JSON 形态零变化：tag `ipc` camelCase 双变体
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

// ---------------------------------------------------------------------------
// dyn 注入缝（v2 编排签名）：`Box<dyn AgentRunner>` 经 `&dyn` 缝注入编排，
// tee 双 sink 与终态流出行为与原泛型形态一致（AC-1 / AC-8 编排零改动证据）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn box_dyn_runner经dyn缝注入编排_tee双sink与终态record流出与泛型形态一致() {
    let (dir, app) = temp_app_stores("dyn-seam");
    let stores = app.state::<WorkspaceStores>();
    let ws_dir = real_ws_dir(dir.path());
    let root = ws_dir.to_string_lossy().into_owned();
    let seeded = vec![
        run_started(0),
        message(1, None),
        run_result(2, false, Some(1)),
    ];
    // 门面产 `Box<dyn AgentRunner>`；假 runner 经同缝（&dyn）注入
    let boxed: Box<dyn AgentRunner> = Box::new(FakeRunner::with_events(seeded.clone()));
    let (channel, captured) = capturing_channel();

    let running = start_agent_run_with(
        app.handle().clone(),
        stores.inner(),
        boxed.as_ref(),
        channel,
        params(&ws_dir),
        RunProvenance::debug(),
    )
    .expect("dyn 缝发起成功（提前 resolve running）");
    assert_eq!(running.status, AgentRunStatus::Running);

    let record_value = wait_terminal_record(&captured).await;
    let terminal: AgentRunRecord =
        serde_json::from_value(record_value["record"].clone()).expect("Record 信封携带终态记录");
    assert_eq!(
        terminal.status,
        AgentRunStatus::Completed,
        "RunResult 驱动收敛"
    );

    // store sink：全量落库，重放逐字段保真
    let store = stores.for_root(&root).expect("for_root 应成功");
    let stored = store.list_agent_run_events(terminal.id).expect("读回事件");
    assert_eq!(stored, seeded, "dyn 缝下落库全量一致");
    // Channel sink：逐事件一致
    assert_eq!(
        serde_json::Value::Array(pushed_event_bodies(&captured)),
        serde_json::to_value(&stored).unwrap(),
        "dyn 缝下 Channel 逐事件与落库一致"
    );
}

// ---------------------------------------------------------------------------
// 编排引擎中立（AC-8「编排代码无引擎分支」）：同一编排体分别驱动 CLI 形态与
// SDK 形态假 runner（均经 dyn 缝），completed / stopped / failed 三收敛分支
// 语义不因引擎形态漂移
// ---------------------------------------------------------------------------

/// CLI 形态预录事件（RunStarted 报 CLI 侧形状）。
fn cli_shaped_events(is_error: bool) -> Vec<AgentEvent> {
    vec![
        run_started(0),
        message(1, None),
        run_result(2, is_error, Some(1)),
    ]
}

/// SDK 形态预录事件（`sdk-` 前缀会话 + ToolUse/ToolResult 块形状）。
fn sdk_shaped_events(is_error: bool) -> Vec<AgentEvent> {
    vec![
        AgentEvent::stamp(
            0,
            AgentEventKind::RunStarted {
                model: Some("rig-model".to_owned()),
                session_id: Some("sdk-0-1727000000000".to_owned()),
                tools: vec!["read".to_owned(), "write".to_owned()],
                mcp_servers: Vec::new(),
            },
        ),
        AgentEvent::stamp(
            1,
            AgentEventKind::Message {
                role: "assistant".to_owned(),
                blocks: vec![::agent::AgentBlock::ToolUse {
                    id: "tu_sdk".to_owned(),
                    name: "read".to_owned(),
                    input: serde_json::json!({ "path": "a.txt" }),
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            2,
            AgentEventKind::Message {
                role: "user".to_owned(),
                blocks: vec![::agent::AgentBlock::ToolResult {
                    id: "tu_sdk".to_owned(),
                    content: "内容".to_owned(),
                    is_error: false,
                }],
                parent_tool_use_id: None,
            },
        ),
        run_result(3, is_error, Some(2)),
    ]
}

#[tokio::test]
async fn 同一编排体驱动cli与sdk两形态假runner_三收敛分支语义不漂移() {
    let (_dir, store) = temp_store("engine-neutral");
    let registry = RunStopRegistry::default();

    // completed / failed：两形态各自的 RunResult 驱动收敛一致
    let cases: [(String, Vec<AgentEvent>, AgentRunStatus); 4] = [
        (
            "cli-completed".to_owned(),
            cli_shaped_events(false),
            AgentRunStatus::Completed,
        ),
        (
            "sdk-completed".to_owned(),
            sdk_shaped_events(false),
            AgentRunStatus::Completed,
        ),
        (
            "cli-failed".to_owned(),
            cli_shaped_events(true),
            AgentRunStatus::Failed,
        ),
        (
            "sdk-failed".to_owned(),
            sdk_shaped_events(true),
            AgentRunStatus::Failed,
        ),
    ];
    for (label, events, expected) in cases {
        let boxed: Box<dyn AgentRunner> = Box::new(FakeRunner::with_events(events));
        let (channel, captured) = capturing_channel();
        let run = boxed
            .start(params(Path::new("C:\\ws")))
            .expect("假 runner 启动成功");
        let record = begin_running(&store);
        registry.register("C:\\ws", record.id, RunHandle::default());

        let final_record = drive_agent_run(&store, channel, run, record, &registry).await;
        assert_eq!(
            final_record.status, expected,
            "{label} 收敛分支不因引擎形态漂移"
        );
        let terminal = wait_terminal_record(&captured).await;
        assert_eq!(
            terminal["record"]["status"],
            serde_json::to_value(&expected).unwrap(),
            "{label} 终态信封出线一致"
        );
    }

    // stopped：停止信号置位后两形态同收敛 stopped（不因引擎形态漂移）
    for (label, mut events) in [
        ("cli-stopped", cli_shaped_events(false)),
        ("sdk-stopped", sdk_shaped_events(false)),
    ] {
        events.pop(); // 去掉 RunResult：流保持打开直至停止（无 result 可收敛）
        let boxed: Box<dyn AgentRunner> = Box::new(FakeRunner::with_events_held_until_stop(events));
        let (channel, _captured) = capturing_channel();
        let run = boxed
            .start(params(Path::new("C:\\ws")))
            .expect("假 runner 启动成功");
        run.handle.request_stop();
        let record = begin_running(&store);
        registry.register("C:\\ws", record.id, RunHandle::default());

        let final_record = drive_agent_run(&store, channel, run, record, &registry).await;
        assert_eq!(
            final_record.status,
            AgentRunStatus::Stopped,
            "{label} 停止收敛语义一致"
        );
        assert_eq!(final_record.error, None, "{label} 停止不记因");
    }
}

// ---------------------------------------------------------------------------
// resolve_agent_engine 解析单点（AC-6 / AC-7 / AC-8）：缺省路径（None →
// default_agent_instance）、显式路径（Some(id) → find_agent_instance）、sdk
// 臂 provider 组装、两臂 kind 映射。读全局库（真实组合 WorkspaceStores，
// store 不 mock）：provider / agent 记录经管理操作面真实写入构造。
// ---------------------------------------------------------------------------

/// 全局库写入一份 provider 记录并返回（经管理操作面真实构造）。
fn seed_provider(
    stores: &WorkspaceStores,
    name: &str,
    base_url: &str,
    api_key: &str,
    high: &str,
) -> AgentProviderRecord {
    let provider = AgentProviderRecord::new(
        name.to_owned(),
        base_url.to_owned(),
        api_key.to_owned(),
        AgentModelTiers {
            high: high.to_owned(),
            medium: format!("{high}-medium"),
            low: format!("{high}-low"),
        },
    );
    stores
        .global()
        .upsert_agent_provider(provider)
        .expect("provider 写入应成功")
}

/// 全局库写入一份 agent 实例记录并返回。
fn seed_agent(
    stores: &WorkspaceStores,
    name: &str,
    engine: AgentEngineKind,
    provider_id: Option<i64>,
) -> AgentInstanceRecord {
    stores
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            name.to_owned(),
            engine,
            provider_id,
        ))
        .expect("agent 写入应成功")
}

#[test]
fn resolve缺省路径_none加全局库默认sdk_agent解析为sdk形态_config自引用provider且model恰取high档() {
    let (dir, app) = temp_app_stores("resolve-default");
    let stores = app.state::<WorkspaceStores>();
    // provider 三档可区分：解析消费半边断言 model 恰取 high 档（medium / low 不入 config）
    let provider = seed_provider(
        stores.inner(),
        "解析端点",
        "https://api.example.com/v1",
        "sk-live-1234567890",
        "m-high",
    );
    let agent = seed_agent(
        stores.inner(),
        "默认实例",
        AgentEngineKind::Sdk,
        Some(provider.id),
    );
    stores
        .global()
        .set_default_agent_instance(agent.id)
        .unwrap();
    let _ws = real_ws_dir(dir.path()); // cwd 基准（本用例只解析，不落库）

    let resolved = resolve_agent_engine(stores.inner(), None).expect("缺省路径应命中默认 agent");

    // kind 映射 Sdk 臂 + config 三字段自引用 provider 组装
    assert_eq!(
        resolved.kind,
        agent_runtime::EngineKind::Sdk,
        "sdk agent → EngineKind::Sdk"
    );
    assert_eq!(
        resolved.config.api_key, "sk-live-1234567890",
        "api_key 自引用 provider"
    );
    assert_eq!(
        resolved.config.base_url, "https://api.example.com/v1",
        "base_url 自引用 provider"
    );
    assert_eq!(
        resolved.config.model, "m-high",
        "model 恰取 models.high（AC-6 消费半）"
    );
    assert_ne!(resolved.config.model, "m-high-medium", "medium 不入 config");
    assert_ne!(resolved.config.model, "m-high-low", "low 不入 config");
    assert!(resolved.config.is_complete(), "齐备产物 is_complete 翻正");
}

#[test]
fn resolve缺省路径_无默认agent返回err含引导管理页文案_不落库不推流不回退硬编码引擎() {
    let (dir, app) = temp_app_stores("resolve-no-default");
    let stores = app.state::<WorkspaceStores>();
    let _ws = real_ws_dir(dir.path());

    // 空全局库（无任何 agent）：缺省路径 MUST NOT 静默回退硬编码引擎
    let resolved = resolve_agent_engine(stores.inner(), None);

    let err = match resolved {
        Err(message) => message,
        Ok(_) => panic!("无默认 agent 必须 Err"),
    };
    assert!(
        err.contains("Agent 管理页") && err.contains("/agents"),
        "Err 引导管理页文案，实际: {err}"
    );
    // 不落库：全局库与 workspace 库零写入
    assert!(stores.global().list_agent_instances().unwrap().is_empty());
    let ws_dir = real_ws_dir(dir.path());
    assert!(
        stores
            .for_root(&ws_dir.to_string_lossy())
            .expect("for_root 应成功")
            .list_agent_runs()
            .unwrap()
            .is_empty(),
        "解析失败零落库"
    );
}

#[test]
fn resolve显式路径_some命中cli_agent得cli形态_config为empty占位_some命中sdk同缺省组装形态() {
    let (_dir, app) = temp_app_stores("resolve-explicit");
    let stores = app.state::<WorkspaceStores>();
    let provider = seed_provider(
        stores.inner(),
        "显式端点",
        "https://api.example.com/v1",
        "sk-live-1234567890",
        "m-high",
    );
    let cli_agent = seed_agent(stores.inner(), "cli实例", AgentEngineKind::Cli, None);
    let sdk_agent = seed_agent(
        stores.inner(),
        "sdk实例",
        AgentEngineKind::Sdk,
        Some(provider.id),
    );

    // Some(id) 命中 cli agent：kind=Cli 且 config=EngineConfig::empty()（CLI 臂不消费占位）
    let cli_resolved =
        resolve_agent_engine(stores.inner(), Some(cli_agent.id)).expect("显式 cli 路径应命中");
    assert_eq!(cli_resolved.kind, agent_runtime::EngineKind::Cli);
    assert!(
        cli_resolved.config == agent_runtime::EngineConfig::empty(),
        "CLI 臂空占位（EngineConfig 不派生 Debug，比较走 PartialEq）"
    );

    // Some(id) 命中 sdk agent：同缺省路径组装形态
    let sdk_resolved =
        resolve_agent_engine(stores.inner(), Some(sdk_agent.id)).expect("显式 sdk 路径应命中");
    assert_eq!(sdk_resolved.kind, agent_runtime::EngineKind::Sdk);
    assert_eq!(sdk_resolved.config.model, "m-high");
    assert_eq!(sdk_resolved.config.base_url, "https://api.example.com/v1");
}

#[test]
fn resolve显式路径_some不存在id_err含agent不存在语境() {
    let (dir, app) = temp_app_stores("resolve-miss");
    let stores = app.state::<WorkspaceStores>();
    let _ws = real_ws_dir(dir.path());

    let resolved = resolve_agent_engine(stores.inner(), Some(404));

    let err = match resolved {
        Err(message) => message,
        Ok(_) => panic!("不存在的 agent id 应 Err"),
    };
    assert!(
        err.contains("agent 不存在") && err.contains("404"),
        "Err 含 agent 不存在语境与 id，实际: {err}"
    );
}

/// 预置「sdk agent 引用悬空 provider」的存量全局库（native_db 裸构造三模型组
/// 直接插入，绕过 store 的引用完整性校验——进程内外部一致性防线的构造前提）。
fn preset_global_with_dangling_default_agent(path: &std::path::Path) {
    use native_db::Models;

    let mut models = Models::new();
    models
        .define::<store::WorkspaceRecord>()
        .expect("定义 WorkspaceRecord 失败");
    models
        .define::<AgentProviderRecord>()
        .expect("定义 AgentProviderRecord 失败");
    models
        .define::<AgentInstanceRecord>()
        .expect("定义 AgentInstanceRecord 失败");
    let db = native_db::Builder::new()
        .create(&models, path)
        .expect("预置悬空引用库失败");
    let rw = db.rw_transaction().expect("开启写事务失败");
    rw.insert(AgentInstanceRecord {
        id: 1,
        name: "悬空实例".to_owned(),
        engine: AgentEngineKind::Sdk,
        provider_id: Some(999),
        is_default: true,
    })
    .expect("写入悬空 agent 失败");
    rw.commit().expect("提交悬空预置事务失败");
}

#[test]
fn resolve边界_sdk引用的provider缺失时err不panic_两臂kind与engine线值同域() {
    let dir = tempfile::Builder::new()
        .prefix("agent-exec-test-resolve-dangling-")
        .tempdir()
        .expect("创建临时目录失败");
    // 预置文件名与全局库文件名同源（desktop-global.redb，GLOBAL_DB_FILE_NAME
    // 单点），WorkspaceStores::open 才能以 additive 打开触达悬空数据
    let global_path = dir.path().join("desktop-global.redb");
    preset_global_with_dangling_default_agent(&global_path);
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(dir.path()).expect("additive 打开悬空引用库应成功");
    app.manage(stores);
    let stores = app.state::<WorkspaceStores>();

    // 悬空数据（绕过 store 校验）：解析 Err 不 panic（进程内外部一致性防线）
    let resolved = resolve_agent_engine(stores.inner(), None);
    let err = match resolved {
        Err(message) => message,
        Ok(_) => panic!("悬空 provider 引用必须 Err 而非 panic"),
    };
    assert!(
        err.contains("provider 不存在") && err.contains("999"),
        "Err 含悬空引用语境，实际: {err}"
    );

    // 两臂映射：AgentEngineKind → EngineKind 线值同域（"cli" / "sdk"）
    for (local, expected) in [(AgentEngineKind::Cli, "cli"), (AgentEngineKind::Sdk, "sdk")] {
        let local_wire = serde_json::to_string(&local).expect("序列化成功");
        let facade_kind = match local {
            AgentEngineKind::Cli => agent_runtime::EngineKind::Cli,
            AgentEngineKind::Sdk => agent_runtime::EngineKind::Sdk,
        };
        let facade_wire = serde_json::to_string(&facade_kind).expect("序列化成功");
        assert_eq!(local_wire, format!("\"{expected}\""));
        assert_eq!(
            local_wire, facade_wire,
            "store 本地枚举与门面 EngineKind 同线值域"
        );
    }
}

// ---------------------------------------------------------------------------
// start_agent_run 尾参换源（定夺 9）：`engine: EngineKind` → `resolved:
// ResolvedEngine` 解析产物。runner_for 收解析产物、提前 resolve running、
// tee 双 sink 与终态流出（装置入参形态同步更新；Cli 形态产物经薄入口的
// 隔离 PATH 回归由上方「隔离 PATH 薄入口」用例以 resolved 尾参承载）。
// ---------------------------------------------------------------------------

#[test]
fn start_agent_run尾参换源_sdk形态空配置产物走薄入口以config_missing显式失败() {
    let dir = tempfile::Builder::new()
        .prefix("agent-exec-test-resolved-empty-")
        .tempdir()
        .expect("创建临时目录失败");
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(dir.path()).expect("打开测试全局库失败");
    let ws_dir = real_ws_dir(dir.path());
    let root = ws_dir.to_string_lossy().into_owned();
    let (channel, captured) = capturing_channel();

    // 解析产物（Sdk 形态 + 空配置）：runner_for 收解析产物——启动校验即以
    // resolved.config 判缺失，证明尾参换源后配置面照常流入引擎构造
    let resolved = ResolvedEngine {
        kind: agent_runtime::EngineKind::Sdk,
        config: agent_runtime::EngineConfig::empty(),
    };
    let result = start_agent_run(
        app.handle().clone(),
        &stores,
        channel,
        params(&ws_dir),
        RunProvenance::debug(),
        resolved,
    );

    let err = result.expect_err("空配置 sdk 产物必须显式失败");
    assert!(
        err.contains("配置缺失") && err.contains("api_key"),
        "Err 为 ConfigMissing 文案（启动校验即失败不触网络），实际: {err}"
    );
    assert!(
        stores
            .for_root(&root)
            .expect("for_root 应成功")
            .list_agent_runs()
            .unwrap()
            .is_empty(),
        "启动校验失败不留 run 行"
    );
    assert!(
        captured.lock().expect("捕获锁不可中毒").is_empty(),
        "启动校验失败零推送"
    );
}

#[tokio::test]
async fn start_agent_run尾参换源_sdk形态齐备产物经薄入口全链_提前resolve与tee双sink与终态流出() {
    let (dir, app) = temp_app_stores("resolved-full-chain");
    let stores = app.state::<WorkspaceStores>();
    let ws_dir = real_ws_dir(dir.path());
    let root = ws_dir.to_string_lossy().into_owned();
    let (channel, captured) = capturing_channel();

    // 解析产物（Sdk 形态 + 齐备配置；base_url 指向不可达端点——start 即成
    // 功，请求期才暴露，泵随停止信号收敛）
    let resolved = ResolvedEngine {
        kind: agent_runtime::EngineKind::Sdk,
        config: agent_runtime::EngineConfig {
            api_key: "sk-resolved-890".to_owned(),
            base_url: "http://127.0.0.1:9/v1".to_owned(),
            model: "m-high".to_owned(),
        },
    };

    let running = start_agent_run(
        app.handle().clone(),
        stores.inner(),
        channel,
        params(&ws_dir),
        RunProvenance::debug(),
        resolved,
    )
    .expect("齐备产物薄入口发起成功");

    // 提前 resolve：running 记录 id 立即可用（编排语义与换源前一致）
    assert_eq!(
        running.status,
        AgentRunStatus::Running,
        "提前 resolve running"
    );
    assert_ne!(running.id, 0, "begin 已分配 id");
    assert_eq!(
        stores
            .for_root(&root)
            .expect("for_root 应成功")
            .list_agent_runs()
            .unwrap()
            .len(),
        1,
        "run 行落该 root 的 workspace 库"
    );

    // 停止信号触达（同步段已注册句柄）→ 后台收敛 → 终态 Record 流出
    let registry = app.state::<RunStopRegistry>();
    assert!(
        registry.request_stop(&root, running.id),
        "解析产物发起的 run 已注册停止句柄"
    );
    let record_value = wait_terminal_record(&captured).await;
    let terminal_status = record_value["record"]["status"]
        .as_str()
        .expect("终态 status 出线")
        .to_owned();
    assert!(
        terminal_status == "stopped" || terminal_status == "failed",
        "终态经 Channel Record 信封流出（sdk 泵随停止/端点失败收敛），实际: {terminal_status}"
    );
}

// ---------------------------------------------------------------------------
// find_events_by_session 辅助（store 既有 API 组合）与 ResumeTranscript
// loader 组装（sdk 引擎续会话解析的唯一数据面）
// ---------------------------------------------------------------------------

/// 落一条带 session_id 的 run（begin + finish，落 workspace 库）。
fn seed_run_with_session(store: &Store, session_id: &str, cwd: &str) -> store::AgentRunRecord {
    let mut requested = abort_base_record();
    requested.cwd = cwd.to_owned();
    requested.session_id = Some(session_id.to_owned());
    let record = store.begin_agent_run(&requested).expect("begin 应成功");
    store
        .finish_agent_run(record.id, &record)
        .expect("finish 应成功");
    record
}

#[test]
fn find_events_by_session命中返回转录_未命中返回none() {
    let (_dir, store) = temp_store("find-session");
    let record = seed_run_with_session(&store, "sdk-1-1727000000000", "C:\\ws");
    let seeded = vec![run_started(0), message(1, None)];
    store
        .append_agent_run_events(record.id, &seeded)
        .expect("append 应成功");

    // 命中：返回该 run 的事件转录（store 既有 API 组合，无 schema / API 变化）
    let found = find_events_by_session(&store, "sdk-1-1727000000000").expect("读取应成功");
    assert_eq!(found.expect("命中"), seeded, "转录逐字段保真");

    // 无命中（不存在 session）：None（loader None 语义 = 会话不存在 / 非 SDK 产出）
    let miss = find_events_by_session(&store, "sdk-404").expect("读取应成功");
    assert!(miss.is_none(), "不存在 session 返回 None");

    // 非 workspace 域的裸库（同型 Store 无该 run）同样 None
    let (_other_dir, other_store) = temp_store("find-session-other");
    let other = find_events_by_session(&other_store, "sdk-1-1727000000000").expect("读取应成功");
    assert!(other.is_none(), "他库无该 session 返回 None");
}

#[test]
fn resume_transcript_loader组装_sdk前缀命中转录_非sdk前缀返回none() {
    let (_dir, store) = temp_store("loader-assembly");
    let record = seed_run_with_session(&store, "sdk-7-1727000000000", "C:\\ws");
    let seeded = vec![run_started(0), message(1, None)];
    store
        .append_agent_run_events(record.id, &seeded)
        .expect("append 应成功");

    // 组装（start_agent_run 薄入口同款闭包形态）：store 按值捕获进 Arc
    let loader: agent_runtime::ResumeTranscript = {
        let store = std::sync::Arc::new(store);
        std::sync::Arc::new(move |session_id: &str| find_events_by_session(&store, session_id))
    };

    // `sdk-` 前缀 run 事件读回 Vec<AgentEvent> 转录
    let transcript = loader("sdk-7-1727000000000")
        .expect("loader 读取应成功")
        .expect("sdk- 前缀命中");
    assert_eq!(transcript, seeded);

    // 非 `sdk-` 前缀 session_id → None（`sdk-` 前缀校验落点；store 侧以
    // session_id 归属承载，非 sdk 产出会话不落 sdk 前缀 id 自然 miss）
    let non_sdk = loader("cli-legacy-1").expect("loader 读取应成功");
    assert!(non_sdk.is_none(), "非 sdk 前缀（无对应 run）返回 None");
    // v2 起 explore 链 run 亦产 `sdk-` 前缀：同形 `sdk-` id 经组装 loader
    // 同样命中（归属标记中立， loader 不解释来源）
    let explore = loader("sdk-99-1727000000999").expect("loader 读取应成功");
    assert!(explore.is_none(), "未落库的 sdk- 形态 id 同样走 None 语义");
}
