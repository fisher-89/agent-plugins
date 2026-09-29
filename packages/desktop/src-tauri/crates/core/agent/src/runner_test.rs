//! `runner` 的单元测试（AC-1）：`AgentRunner` trait 假实现经 mpsc 交付预录
//! 事件（trait 测试替身，非外部进程 mock）、Send+Sync 注入面、三枚举 serde
//! 线格式值域（含 `AgentRunStatus`；as_str 双轨口径随其退役删除，值域单一
//! 来源 = serde camelCase）、`AgentStartError` Display 文案、`AgentRunParams`
//! cwd 保真。

use std::path::PathBuf;

use serde_json::json;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, AgentEventKind};
use crate::runner::{
    AgentEnvMode, AgentPermissionMode, AgentRun, AgentRunParams, AgentRunStatus, AgentRunner,
    AgentStartError, RunHandle,
};

/// 假 runner：预录事件序列经 mpsc 交付（或直接返回可控启动错误）。
struct FakeRunner {
    events: Vec<AgentEvent>,
    failure: Option<AgentStartError>,
}

impl FakeRunner {
    fn with_events(events: Vec<AgentEvent>) -> Self {
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
        let (sender, receiver) = mpsc::channel(self.events.len().max(1));
        let events = self.events.clone();
        tokio::spawn(async move {
            for event in events {
                let _ = sender.send(event).await;
            }
        });
        Ok(AgentRun {
            events: receiver,
            handle: RunHandle::default(),
        })
    }
}

fn text_event(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    )
}

fn params() -> AgentRunParams {
    AgentRunParams {
        prompt: "你好".to_owned(),
        cwd: PathBuf::from("C:\\work\\demo"),
        permission_mode: AgentPermissionMode::BypassPermissions,
        resume_session_id: None,
    }
}

// ---------------------------------------------------------------------------
// AgentRunner.start（假 runner 事件流）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假runner预录事件经start返回的agentrun按序接收() {
    let prerecorded = vec![
        AgentEvent::stamp(
            0,
            AgentEventKind::RunStarted {
                model: Some("claude-opus".to_owned()),
                session_id: None,
                tools: Vec::new(),
                mcp_servers: Vec::new(),
            },
        ),
        text_event(1),
        AgentEvent::stamp(
            2,
            AgentEventKind::RunResult {
                subtype: "success".to_owned(),
                is_error: false,
                num_turns: Some(1),
                duration_ms: None,
                cost_usd: None,
                usage: serde_json::Value::Null,
                session_id: None,
            },
        ),
    ];
    let runner = FakeRunner::with_events(prerecorded.clone());

    let mut run = runner.start(params()).expect("start 应成功");
    let mut received = Vec::new();
    while let Some(event) = run.events.recv().await {
        received.push(event);
    }

    assert_eq!(received, prerecorded, "事件按预录顺序逐条交付");
}

#[tokio::test]
async fn 假runner可作trait_object注入且满足send_sync边界() {
    // trait object：三租户预留面的注入形态
    let boxed: Box<dyn AgentRunner> = Box::new(FakeRunner::with_events(vec![text_event(0)]));
    let mut run = boxed.start(params()).expect("start 应成功");
    assert!(run.events.recv().await.is_some(), "trait object 事件可达");

    // Send + Sync bound：AgentRunner: Send + Sync，实现随之可跨线程
    fn assert_send_sync<R: AgentRunner>(_: &R) {}
    let runner = FakeRunner::with_events(Vec::new());
    assert_send_sync(&runner);

    // 启动失败路径：start Err 语义（不产生任何事件）
    let failing: Box<dyn AgentRunner> = Box::new(FakeRunner::failing(AgentStartError::CliMissing(
        "未发现".to_owned(),
    )));
    let result = failing.start(params());
    assert!(matches!(result, Err(AgentStartError::CliMissing(_))));
}

// ---------------------------------------------------------------------------
// 三枚举 serde 线格式值域（AC-1：与枚举化前 String 值域逐字一致；值域单一
// 来源 = serde camelCase，as_str 双轨口径已退役）
// ---------------------------------------------------------------------------

#[test]
fn env_mode线格式回归两档仍逐字为default与bare且roundtrip一致() {
    // 加 specta::Type 后线格式零变化（AC-1 边界回归）
    for (mode, expected) in [
        (AgentEnvMode::Default, "default"),
        (AgentEnvMode::Bare, "bare"),
    ] {
        let serialized = serde_json::to_string(&mode).expect("序列化成功");
        assert_eq!(serialized, format!("\"{expected}\""), "线格式逐字一致");
        let roundtrip: AgentEnvMode = serde_json::from_str(&serialized).expect("反序列化成功");
        assert_eq!(roundtrip, mode);
    }
}

#[test]
fn permission_mode线格式回归三档仍为受控驼峰串且roundtrip一致() {
    // 加 specta::Type 后线格式零变化（AC-1 正向回归）
    for (mode, expected) in [
        (AgentPermissionMode::Default, "default"),
        (AgentPermissionMode::AcceptEdits, "acceptEdits"),
        (AgentPermissionMode::BypassPermissions, "bypassPermissions"),
    ] {
        let serialized = serde_json::to_string(&mode).expect("序列化成功");
        assert_eq!(serialized, format!("\"{expected}\""), "线格式逐字一致");
        let roundtrip: AgentPermissionMode =
            serde_json::from_str(&serialized).expect("反序列化成功");
        assert_eq!(roundtrip, mode);
    }
}

#[test]
fn run_status四档序列化逐字为running_completed_failed_stopped且roundtrip一致() {
    // AgentRunStatus（新增枚举）：serde camelCase 值域与枚举化前 String 值域
    // 逐字一致（AC-1 正向）
    for (status, expected) in [
        (AgentRunStatus::Running, "running"),
        (AgentRunStatus::Completed, "completed"),
        (AgentRunStatus::Failed, "failed"),
        (AgentRunStatus::Stopped, "stopped"),
    ] {
        let serialized = serde_json::to_string(&status).expect("序列化成功");
        assert_eq!(serialized, format!("\"{expected}\""), "线格式逐字一致");
        let roundtrip: AgentRunStatus = serde_json::from_str(&serialized).expect("反序列化成功");
        assert_eq!(roundtrip, status);
    }
}

#[test]
fn run_status清单外字符串反序列化返回err值域受控() {
    // 大小写不符 / 前缀撞车 / 旧同义词 / 空串：清单外一律 Err（异常半边）
    for invalid in ["Running", "run", "succeeded", "COMPLETE", "stop", ""] {
        let result = serde_json::from_str::<AgentRunStatus>(invalid);
        assert!(result.is_err(), "status 非法值 {invalid:?} 必须 Err");
    }
}

#[test]
fn 非法档位字符串反序列化返回err枚举边界() {
    // env：大小写不符 / 前缀撞车 / 空串
    for invalid in ["Bare", "DEFAULT", "ba", ""] {
        let result = serde_json::from_str::<AgentEnvMode>(invalid);
        assert!(result.is_err(), "env 非法值 {invalid:?} 必须 Err");
    }
    // permission-mode：非枚举值 / 撞前缀 / 空串
    for invalid in ["bypass", "BypassPermissions", "accept_edits", ""] {
        let result = serde_json::from_str::<AgentPermissionMode>(invalid);
        assert!(
            result.is_err(),
            "permission-mode 非法值 {invalid:?} 必须 Err"
        );
    }
}

// ---------------------------------------------------------------------------
// AgentStartError Display / AgentRunParams cwd 保真
// ---------------------------------------------------------------------------

#[test]
fn start_error两变体display文案携带原因串可直抵前端() {
    let missing = AgentStartError::CliMissing("PATH 上未发现入口".to_owned());
    let spawn = AgentStartError::SpawnFailed("io error".to_owned());

    let missing_text = missing.to_string();
    let spawn_text = spawn.to_string();
    assert!(
        missing_text.contains("CLI") && missing_text.contains("PATH 上未发现入口"),
        "CliMissing 文案携带原因串，实际: {missing_text}"
    );
    assert!(
        spawn_text.contains("启动失败") && spawn_text.contains("io error"),
        "SpawnFailed 文案携带原因串，实际: {spawn_text}"
    );
}

#[test]
fn run_params的cwd含中文空格与尾分隔符时字段保真() {
    // AgentRunParams 为逻辑入参（非线格式类型，derive 仅 Debug/Clone/PartialEq）：
    // 保真口径为字段级——cwd 经 OsString 原样持有，含中文/空格/尾分隔符不失真
    let raw_cwd = "D:\\项目 目录\\demo\\";
    let cwd = PathBuf::from(raw_cwd);
    let params = AgentRunParams {
        prompt: "含 空格 与\n换行的提示词 🎉".to_owned(),
        cwd: cwd.clone(),
        permission_mode: AgentPermissionMode::AcceptEdits,
        resume_session_id: None,
    };

    assert_eq!(params.cwd, cwd, "cwd 原样持有（中文/空格/尾分隔符）");
    assert_eq!(
        params.cwd.to_string_lossy(),
        raw_cwd,
        "lossy 视图逐字符一致"
    );
    // clone 后仍保真（编排函数按值/引用传递参数的语义前提）
    let cloned = params.clone();
    assert_eq!(cloned, params);
    assert_eq!(cloned.cwd, PathBuf::from(raw_cwd));
    // 构造面完整：prompt / permission_mode 三字段齐备（json 形态仅作形状示意）
    let _shape = json!({ "prompt": params.prompt });
}

// ---------------------------------------------------------------------------
// RunHandle：逻辑终止信号（置位 / 同步观测 / 异步等待 / Clone 共享）。
// 无外部依赖；tokio sync（Notify/AtomicBool）以真实实现参与，不需要 Mock。
// 等待语义以「让步自旋 + JoinHandle::is_finished」断言（workspace tokio 无
// time 特性，不引入 timeout）。
// ---------------------------------------------------------------------------

#[test]
fn 初始句柄未置位停止信号() {
    let handle = RunHandle::default();
    assert!(!handle.stop_requested(), "初始态 stop_requested 为 false");
    let cloned = handle.clone();
    assert!(!cloned.stop_requested(), "Clone 出的初始句柄同样未置位");
}

#[test]
fn request_stop置位后stop_requested为true且重复置位幂等() {
    let handle = RunHandle::default();
    handle.request_stop();
    assert!(handle.stop_requested(), "request_stop 后同步观测为 true");

    // 重复置位幂等：不 panic、状态不变
    handle.request_stop();
    assert!(handle.stop_requested());
}

#[test]
fn clone句柄共享信号置位双方可见() {
    let handle = RunHandle::default();
    let cloned = handle.clone();

    // 租户泵侧的 Clone 置位 → 编排侧原句柄可见（共享 AtomicBool 语义）
    cloned.request_stop();
    assert!(handle.stop_requested(), "Clone 置位后原句柄可观测");
    handle.request_stop();
    assert!(cloned.stop_requested(), "原句柄置位后 Clone 侧可观测");
}

#[tokio::test]
async fn 已置位时wait_requested经先复查短路立即返回() {
    let handle = RunHandle::default();
    handle.request_stop();
    // 先复查短路：直接 await 即完成（未短路会永久挂起，测试无法通过）
    handle.wait_requested().await;
}

#[tokio::test]
async fn 未请求终止时wait_requested持续挂起不完成() {
    let handle = RunHandle::default();
    let waiter = tokio::spawn({
        let handle = handle.clone();
        async move { handle.wait_requested().await }
    });

    // 多轮让步后仍未完成：未置位时等待方挂起（不误唤醒）
    for _ in 0..64 {
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished(), "未置位时等待方不得完成");
    }
    waiter.abort();
}

#[tokio::test]
async fn 挂起的wait_requested被request_stop唤醒且重复置位不二次异常唤醒() {
    let handle = RunHandle::default();
    let waiter = tokio::spawn({
        let handle = handle.clone();
        async move { handle.wait_requested().await }
    });

    // 让 waiter 先运行注册等待（wait_requested 内部「先注册、再复查」关闭
    // 置位与注册的竞态：即便置位先落，复查也会短路完成）
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;
    assert!(!waiter.is_finished(), "置位前等待方保持挂起");

    handle.request_stop();
    handle.request_stop(); // 幂等：重复置位不 panic、不产生副作用

    let mut spins = 0;
    while !waiter.is_finished() {
        tokio::task::yield_now().await;
        spins += 1;
        assert!(spins < 10_000, "request_stop 后等待方应被唤醒");
    }
    waiter.await.expect("等待任务正常结束");
}
