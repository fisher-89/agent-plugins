//! `run_agent()` / `run_agent_with()` 的单元 + 「编排 → tee 双路（Channel 实时
//! + store 落库）与状态收敛」集成关系测试（AC-1/AC-3/AC-4，决策 D1/D2/D5/D7/D8）。
//!
//! 本模块声明于 agent.rs 内（子模块），可触达私有失败收敛助手；dev-team 为
//! 纯 binary crate，共置沿 workspaces/mod_test.rs 既有先例。假 runner 测试
//! 替身注入泛型缝；Channel 以 `tauri::ipc::Channel::new` 捕获回调（或返回
//! Err 构造「页面已关」）；store 不 mock（tempdir 真库）。真实 claude 进程
//! 不引入（见 test-design 不可测试项 4）。

use std::path::Path;
use std::sync::{Arc, Mutex};

use tauri::ipc::{Channel, InvokeResponseBody};

use ::agent::{
    AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRun, AgentRunParams,
    AgentRunner, AgentStartError, RunHandle,
};
use store::Store;

use super::{
    abort_with_store_failure, run_agent, run_agent_with, STATUS_COMPLETED, STATUS_FAILED,
    STATUS_RUNNING,
};

/// PATH 环境变量修改串行化（agent_start 同款用例经 mod_test 共享此锁）。
pub(crate) static PATH_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// 装置：tempdir 真库、假 runner、可编程 Channel
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

fn params(cwd: &Path) -> AgentRunParams {
    AgentRunParams {
        prompt: "帮我跑一轮 loop".to_owned(),
        cwd: cwd.to_path_buf(),
        env: AgentEnvMode::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
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
/// `pub(crate)`：mod_test 的链路级用例（集成关系 R3）经 `run_agent_with` 泛型缝
/// 复用同一装置。
pub(crate) struct FakeRunner {
    events: Vec<AgentEvent>,
    failure: Option<AgentStartError>,
}

impl FakeRunner {
    pub(crate) fn with_events(events: Vec<AgentEvent>) -> Self {
        Self {
            events,
            failure: None,
        }
    }

    fn failing(failure: AgentStartError) -> Self {
        Self {
            events: Vec::new(),
            failure: Some(failure),
        }
    }
}

impl AgentRunner for FakeRunner {
    fn start(&self, _params: AgentRunParams) -> Result<AgentRun, AgentStartError> {
        if let Some(failure) = self.failure.clone() {
            return Err(failure);
        }
        let (sender, receiver) = tokio::sync::mpsc::channel(self.events.len().max(1));
        let events = self.events.clone();
        tokio::spawn(async move {
            for event in events {
                let _ = sender.send(event).await;
            }
        });
        Ok(AgentRun {
            events: receiver,
            handle: RunHandle,
        })
    }
}

/// 捕获型 Channel：逐事件收下序列化 JSON（tee 实时路快照）。
fn capturing_channel() -> (Channel<AgentEvent>, Arc<Mutex<Vec<serde_json::Value>>>) {
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
fn failing_channel() -> Channel<AgentEvent> {
    Channel::new(|_| {
        Err(tauri::Error::Io(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "页面已关闭",
        )))
    })
}

// ---------------------------------------------------------------------------
// 正向：预录会话 tee 全链路（AC-4 tee 双 sink + 状态收敛）
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
    let runner = FakeRunner::with_events(prerecorded.clone());
    let (channel, captured) = capturing_channel();

    let result = run_agent_with(&store, &runner, channel, params(Path::new("C:\\ws"))).await;

    let record = result.expect("completed 会话返回 Ok");
    assert_eq!(
        record.status, STATUS_COMPLETED,
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

    // Channel sink：逐事件一致（同 seq 同内容，两路 seq 一致）
    let pushed = captured.lock().expect("捕获锁不可中毒").clone();
    assert_eq!(
        serde_json::Value::Array(pushed),
        serde_json::to_value(&stored).unwrap(),
        "Channel 事件序列与落库事件序列逐条一致"
    );

    // 落库 run 行为 completed 终态
    let listed = store.list_agent_runs().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, record.id);
    assert_eq!(listed[0].status, STATUS_COMPLETED);
}

#[tokio::test]
async fn 预录is_error的result时返回ok的failed记录而非err() {
    let (_dir, store) = temp_store("tee-failed");
    let runner = FakeRunner::with_events(vec![run_result(0, true, Some(2))]);
    let (channel, _captured) = capturing_channel();

    let result = run_agent_with(&store, &runner, channel, params(Path::new("C:\\ws"))).await;

    let record = result.expect("D7：in-band 失败返回 Ok(failed 记录)");
    assert_eq!(record.status, STATUS_FAILED);
    assert_eq!(record.num_turns, Some(2));
    assert!(record.finished_at.is_some());
    assert_eq!(store.list_agent_runs().unwrap()[0].status, STATUS_FAILED);
}

// ---------------------------------------------------------------------------
// 异常：启动失败不留行（D5）与 store 写失败收敛（D1）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假runner启动失败时返回err且store零run行() {
    let (_dir, store) = temp_store("tee-start-err");
    let runner = FakeRunner::failing(AgentStartError::CliMissing("PATH 上未发现".to_owned()));
    let (channel, captured) = capturing_channel();

    let result = run_agent_with(&store, &runner, channel, params(Path::new("C:\\ws"))).await;

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
async fn channel接收端先行关闭时落库继续完整且命令返回最终记录() {
    let (_dir, store) = temp_store("tee-channel-closed");
    let prerecorded = vec![
        run_started(0),
        message(1, None),
        run_result(2, false, Some(1)),
    ];
    let runner = FakeRunner::with_events(prerecorded);
    // 接收端恒失败（页面已关）：tee 不得中断落库
    let channel = failing_channel();

    let result = run_agent_with(&store, &runner, channel, params(Path::new("C:\\ws"))).await;

    let record = result.expect("Channel 发送失败不影响命令返回");
    assert_eq!(record.status, STATUS_COMPLETED);
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
    let (channel, captured) = capturing_channel();

    let record = run_agent_with(&store, &runner, channel, params(Path::new("C:\\ws")))
        .await
        .expect("编排成功");

    // Channel 透传不重排：与预录顺序一致
    let pushed = captured.lock().unwrap().clone();
    let pushed_seqs: Vec<u64> = pushed
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
// 薄入口：隔离 PATH 走完整链（组装 ClaudeCliRunner → start 失败）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_agent隔离path时走薄入口全链返回err且store无run行() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let (_dir, store) = temp_store("thin-entry");
    let empty_path = tempfile::tempdir().expect("创建空 PATH 目录失败");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", empty_path.path());
    let result = run_agent(&store, capturing_channel().0, params(Path::new("C:\\ws"))).await;
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
        .begin_agent_run(&running_record())
        .expect("begin 应成功");

    let record = abort_with_store_failure(
        &store,
        run.clone(),
        "事件落库失败: db: 模拟写入失败".to_owned(),
    );

    assert_eq!(record.status, STATUS_FAILED, "run 收敛为 failed");
    assert_eq!(
        record.error.as_deref(),
        Some("事件落库失败: db: 模拟写入失败"),
        "error 记因"
    );
    assert!(record.finished_at.is_some(), "终态落 finished_at");

    let listed = store.list_agent_runs().unwrap();
    assert_eq!(listed.len(), 1, "终态行已尽力落库");
    assert_eq!(listed[0].status, STATUS_FAILED);
    assert_eq!(
        listed[0].error.as_deref(),
        Some("事件落库失败: db: 模拟写入失败")
    );

    // 终态替换语义：failed 终态行覆盖 running 行（整行替换，非追加）
    assert_ne!(listed[0].status, STATUS_RUNNING);
}

/// 收敛助手用例的底座记录（running 形态，id 已由 begin 分配）。
fn running_record() -> store::AgentRunRecord {
    store::AgentRunRecord {
        id: 0,
        prompt: "落库失败场景".to_owned(),
        cwd: "C:\\ws".to_owned(),
        env: "default".to_owned(),
        permission_mode: "bypassPermissions".to_owned(),
        status: STATUS_RUNNING.to_owned(),
        started_at: 1727000000000,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
    }
}
