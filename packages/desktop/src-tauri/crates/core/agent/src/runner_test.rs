//! `runner` 的单元测试（AC-1）：`AgentRunner` trait 假实现经 mpsc 交付预录
//! 事件（trait 测试替身，非外部进程 mock）、Send+Sync 注入面、两枚举 serde
//! 线格式与 as_str 口径、`AgentStartError` Display 文案、`AgentRunParams`
//! cwd 保真。

use std::path::PathBuf;

use serde_json::json;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, AgentEventKind};
use crate::runner::{
    AgentEnvMode, AgentPermissionMode, AgentRun, AgentRunParams, AgentRunner, AgentStartError,
    RunHandle,
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
            handle: RunHandle,
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
        env: AgentEnvMode::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
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
// AgentEnvMode / AgentPermissionMode serde 与 as_str
// ---------------------------------------------------------------------------

#[test]
fn env_mode两档序列化为default与bare且as_str同值() {
    for (mode, expected) in [
        (AgentEnvMode::Default, "default"),
        (AgentEnvMode::Bare, "bare"),
    ] {
        let serialized = serde_json::to_string(&mode).expect("序列化成功");
        assert_eq!(serialized, format!("\"{expected}\""));
        assert_eq!(mode.as_str(), expected, "落库口径与线格式一致");
        let roundtrip: AgentEnvMode = serde_json::from_str(&serialized).expect("反序列化成功");
        assert_eq!(roundtrip, mode);
    }
}

#[test]
fn permission_mode三档序列化为受控驼峰串且as_str同值() {
    for (mode, expected) in [
        (AgentPermissionMode::Default, "default"),
        (AgentPermissionMode::AcceptEdits, "acceptEdits"),
        (AgentPermissionMode::BypassPermissions, "bypassPermissions"),
    ] {
        let serialized = serde_json::to_string(&mode).expect("序列化成功");
        assert_eq!(serialized, format!("\"{expected}\""));
        assert_eq!(mode.as_str(), expected, "落库口径与线格式一致");
        let roundtrip: AgentPermissionMode =
            serde_json::from_str(&serialized).expect("反序列化成功");
        assert_eq!(roundtrip, mode);
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
        env: AgentEnvMode::Bare,
        permission_mode: AgentPermissionMode::AcceptEdits,
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
    // 构造面完整：prompt / env / permission_mode 三字段齐备（json 形态仅作形状示意）
    let _shape = json!({ "prompt": params.prompt, "env": params.env.as_str() });
}
