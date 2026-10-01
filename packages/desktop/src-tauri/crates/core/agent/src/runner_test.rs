use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::json;

use crate::event::AgentEventKind;
use crate::runner::{
    AgentPermissionMode, AgentRunner, AgentSession, AgentStartError, RunHandle, SessionCtx,
    SessionInjections, SessionOpen, SessionRef,
};

/// 假 runner：捕获 SessionOpen 入参（参数转换断言缝）、预录未盖戳事件经
/// mpsc 回流，或返回可编程启动错误。每次 open_session 产出独立通道（互不
/// 共享运行态）。
struct FakeRunner {
    captured: Mutex<Vec<SessionOpen>>,
    prerecorded: Vec<AgentEventKind>,
    failure: Option<AgentStartError>,
}

impl FakeRunner {
    fn capturing() -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
            prerecorded: Vec::new(),
            failure: None,
        }
    }

    fn with_events(events: Vec<AgentEventKind>) -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
            prerecorded: events,
            failure: None,
        }
    }

    fn failing(failure: AgentStartError) -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
            prerecorded: Vec::new(),
            failure: Some(failure),
        }
    }

    /// 已捕获的入参快照（按调用序）。
    fn captured_opens(&self) -> Vec<SessionOpen> {
        self.captured
            .lock()
            .expect("捕获锁不可中毒")
            .clone()
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
        let (observation_tx, observation_rx) = tokio::sync::mpsc::channel(16);
        let (question_tx, _question_rx) = tokio::sync::mpsc::channel::<crate::runner::TurnQuestion>(4);
        let events = self.prerecorded.clone();
        tokio::spawn(async move {
            for event in events {
                let _ = observation_tx.send(event).await;
            }
        });
        Ok(AgentSession {
            observations: observation_rx,
            questions: question_tx,
            handle: RunHandle::default(),
        })
    }
}

/// New 会话的 open 入参底座。
fn new_open() -> SessionOpen {
    SessionOpen {
        injections: SessionInjections {
            preamble: Some("会话前导".to_owned()),
            tools: Some(vec!["read".to_owned(), "grep".to_owned()]),
        },
        ctx: SessionCtx {
            workspace_root: PathBuf::from("D:\\工作区\\demo 🎉"),
            permission_mode: AgentPermissionMode::BypassPermissions,
        },
        session: SessionRef::New,
        prior_handle: None,
    }
}

// ---------------------------------------------------------------------------
// open_session：New 会话建立（injections 与 ctx 原样到达实现）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn open_session_new注入与上下文原样到达实现且返回三件套() {
    let runner = FakeRunner::with_events(vec![AgentEventKind::Raw {
        event_type: "x".to_owned(),
        raw_json: "{}".to_owned(),
    }]);
    let open = new_open();

    let mut session = runner.open_session(open).expect("open 应成功");

    // 三件套：观察回流 + 轮驱动 + 运行句柄
    let received = session.observations.recv().await.expect("观察可达");
    assert!(matches!(received, AgentEventKind::Raw { .. }), "预录观察回流");
    let _ = session
        .questions
        .send(crate::runner::TurnQuestion {
            prompt: "提问".to_owned(),
        })
        .await;
    assert!(!session.handle.stop_requested(), "句柄初值未置位");

    // 捕获缝断言：injections（会话级）与 ctx（轮级）原样到达实现
    let captured = runner.captured_opens();
    assert_eq!(captured.len(), 1, "恰捕获一次 open 入参");
    assert_eq!(
        captured[0].injections.preamble.as_deref(),
        Some("会话前导"),
        "preamble 原样传递（injections/轮分离不变量的会话半边）"
    );
    assert_eq!(
        captured[0].injections.tools,
        Some(vec!["read".to_owned(), "grep".to_owned()]),
        "tools 原样传递"
    );
    assert_eq!(
        captured[0].ctx.workspace_root,
        PathBuf::from("D:\\工作区\\demo 🎉"),
        "workspace_root 原样传递（中文/emoji 不失真）"
    );
    assert_eq!(
        captured[0].ctx.permission_mode,
        AgentPermissionMode::BypassPermissions
    );
    assert!(matches!(captured[0].session, SessionRef::New));
    assert_eq!(captured[0].prior_handle, None, "New 恒 None");
}

#[tokio::test]
async fn open_session_continue携会话id与prior_handle原样传递() {
    let runner = FakeRunner::capturing();
    let open = SessionOpen {
        injections: SessionInjections::default(),
        ctx: SessionCtx {
            workspace_root: PathBuf::from("C:\\ws"),
            permission_mode: AgentPermissionMode::AcceptEdits,
        },
        session: SessionRef::Continue {
            id: "ses-1-1727000000000".to_owned(),
        },
        prior_handle: Some("sdk-7-1727000000001".to_owned()),
    };

    runner
        .open_session(open)
        .expect("Continue 引用的 open 应成功");

    let captured = runner.captured_opens();
    assert!(
        matches!(
            &captured[0].session,
            SessionRef::Continue { id } if id == "ses-1-1727000000000"
        ),
        "会话 id 原样传递（双 id 映射的回供半边）"
    );
    assert_eq!(
        captured[0].prior_handle.as_deref(),
        Some("sdk-7-1727000000001"),
        "引擎侧先行句柄原样回供引擎"
    );
}

// ---------------------------------------------------------------------------
// open_session：启动失败三变体（变体集零改动回归）
// ---------------------------------------------------------------------------

#[test]
fn open_session三变体启动失败err原样传播且互不重合() {
    let failures = [
        AgentStartError::ConfigMissing("凭据未配".to_owned()),
        AgentStartError::CliMissing("PATH 上未发现".to_owned()),
        AgentStartError::SpawnFailed("io error".to_owned()),
    ];
    for failure in failures {
        let runner = FakeRunner::failing(failure.clone());
        let result = runner.open_session(new_open());
        assert_eq!(
            result.expect_err("启动失败必须 Err"),
            failure,
            "Err 原样传播（含载荷逐字）"
        );
    }

    // 变体集零改动回归：三变体互不重合（match 闭包可全枚举）
    let variant_of = |error: &AgentStartError| match error {
        AgentStartError::CliMissing(_) => "cli_missing",
        AgentStartError::SpawnFailed(_) => "spawn_failed",
        AgentStartError::ConfigMissing(_) => "config_missing",
    };
    assert_eq!(
        variant_of(&AgentStartError::ConfigMissing("x".to_owned())),
        "config_missing"
    );
    assert_eq!(
        variant_of(&AgentStartError::CliMissing("x".to_owned())),
        "cli_missing"
    );
    assert_eq!(
        variant_of(&AgentStartError::SpawnFailed("x".to_owned())),
        "spawn_failed"
    );
}

// ---------------------------------------------------------------------------
// trait object 形态与运行态隔离
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假runner可作trait_object注入且满足send_sync边界() {
    let boxed: Box<dyn AgentRunner> = Box::new(FakeRunner::with_events(vec![AgentEventKind::Raw {
        event_type: "x".to_owned(),
        raw_json: "{}".to_owned(),
    }]));
    let mut session = boxed.open_session(new_open()).expect("trait object open 应成功");
    assert!(session.observations.recv().await.is_some(), "trait object 观察可达");

    // Send + Sync bound：AgentRunner: Send + Sync，实现随之可跨线程
    fn assert_send_sync<R: AgentRunner>(_: &R) {}
    assert_send_sync(&FakeRunner::capturing());
}

#[tokio::test]
async fn 重复open_session互不共享运行态_两会话观察各自独立() {
    let runner = FakeRunner::with_events(vec![
        AgentEventKind::Raw {
            event_type: "first".to_owned(),
            raw_json: "{}".to_owned(),
        },
        AgentEventKind::Raw {
            event_type: "second".to_owned(),
            raw_json: "{}".to_owned(),
        },
    ]);

    let mut first = runner.open_session(new_open()).expect("第一次 open 应成功");
    let mut second = runner.open_session(new_open()).expect("第二次 open 应成功");

    // 各自独立的观察回流：first 恰收到其一、second 收到其二，互不串流
    let _ = first.observations.recv().await.expect("第一会话观察可达");
    let _ = second.observations.recv().await.expect("第二会话观察可达");
    assert_eq!(
        runner.captured_opens().len(),
        2,
        "两次 open 各自捕获（无共享运行态合并）"
    );
}

// ---------------------------------------------------------------------------
// 协议类型 serde 线格式（SessionCtx 驼峰 + 未知字段忽略；SessionInjections
// tools None 形态）
// ---------------------------------------------------------------------------

#[test]
fn session_ctx序列化驼峰键且未知字段忽略往返无损() {
    let ctx = SessionCtx {
        workspace_root: PathBuf::from("D:\\工作区"),
        permission_mode: AgentPermissionMode::AcceptEdits,
    };

    let value = serde_json::to_value(&ctx).expect("序列化成功");
    assert_eq!(value["workspaceRoot"], json!("D:\\工作区"), "驼峰键");
    assert_eq!(value["permissionMode"], json!("acceptEdits"), "档位线值");

    // 不加 deny_unknown_fields：多余 JSON 键反序列化成功（additive 演进）
    let mut additive = value.clone();
    additive["futureField"] = json!({ "any": true });
    let roundtrip: SessionCtx = serde_json::from_value(additive).expect("未知字段忽略");
    assert_eq!(roundtrip, ctx);

    let back: SessionCtx = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(back, ctx);
}

#[test]
fn session_injections默认形态tools与preamble均为none() {
    let default_injections = SessionInjections::default();
    assert_eq!(default_injections.preamble, None, "preamble None 即不注入");
    assert_eq!(default_injections.tools, None, "tools None 即引擎默认工具面");

    // tools None 与有值两形态可区分（切片③预留接口位的契约面）
    let narrowed = SessionInjections {
        preamble: None,
        tools: Some(vec!["read".to_owned()]),
    };
    assert_ne!(default_injections, narrowed);
}

#[test]
fn session_ref两形态相等语义与互异() {
    assert_eq!(SessionRef::New, SessionRef::New);
    assert_eq!(
        SessionRef::Continue { id: "ses-1".to_owned() },
        SessionRef::Continue { id: "ses-1".to_owned() }
    );
    assert_ne!(SessionRef::New, SessionRef::Continue { id: "ses-1".to_owned() });
    assert_ne!(
        SessionRef::Continue { id: "ses-1".to_owned() },
        SessionRef::Continue { id: "ses-2".to_owned() },
        "Continue 会话 id 参与相等语义"
    );
}

// ---------------------------------------------------------------------------
// RunHandle：逻辑终止信号（置位 / 同步观测 / Clone 共享 / wait_requested
// 短路与唤醒）——零改动承诺回归锁定
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

    handle.request_stop();
    assert!(handle.stop_requested(), "重复置位幂等：状态不变不 panic");
}

#[test]
fn clone句柄共享信号置位双方可见() {
    let handle = RunHandle::default();
    let cloned = handle.clone();

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

// ---------------------------------------------------------------------------
// AgentStartError 三变体 Display（变体集 MUST NOT 再增的回归锚）
// ---------------------------------------------------------------------------

#[test]
fn start_error三变体display文案携带原因串() {
    let cases = [
        (AgentStartError::CliMissing("PATH 上未发现入口".to_owned()), "CLI 未找到"),
        (AgentStartError::SpawnFailed("io error".to_owned()), "启动失败"),
        (AgentStartError::ConfigMissing("api_key 未配".to_owned()), "配置缺失"),
    ];
    for (error, prefix) in cases {
        let text = error.to_string();
        assert!(
            text.contains(prefix) && text.ends_with(|c: char| !c.is_control()),
            "Display 文案携带前缀与原因串，实际: {text}"
        );
    }
    // 逐字前缀回归
    assert_eq!(
        AgentStartError::CliMissing("原因甲".to_owned()).to_string(),
        "CLI 未找到: 原因甲"
    );
    assert_eq!(
        AgentStartError::SpawnFailed("原因乙".to_owned()).to_string(),
        "启动失败: 原因乙"
    );
    assert_eq!(
        AgentStartError::ConfigMissing("原因丙".to_owned()).to_string(),
        "配置缺失: 原因丙"
    );
    // 三变体 Display 互不重合（成因可区分）
    let texts = [
        AgentStartError::CliMissing("x".to_owned()).to_string(),
        AgentStartError::SpawnFailed("x".to_owned()).to_string(),
        AgentStartError::ConfigMissing("x".to_owned()).to_string(),
    ];
    assert_ne!(texts[0], texts[1]);
    assert_ne!(texts[1], texts[2]);
    assert_ne!(texts[0], texts[2]);
    // PartialEq 按载荷精确匹配
    assert_eq!(
        AgentStartError::ConfigMissing("同一载荷".to_owned()),
        AgentStartError::ConfigMissing("同一载荷".to_owned())
    );
}
