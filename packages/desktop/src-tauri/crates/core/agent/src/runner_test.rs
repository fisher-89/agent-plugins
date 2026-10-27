use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::json;

use crate::event::AgentEventKind;
use crate::runner::{
    AgentPermissionMode, AgentRunner, AgentSession, AgentStartError, ModelLevel, RunHandle,
    SessionCtx, SessionInjections, SessionOpen, SessionRef, SessionToolset,
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
        self.captured.lock().expect("捕获锁不可中毒").clone()
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
        let (question_tx, _question_rx) =
            tokio::sync::mpsc::channel::<crate::runner::TurnQuestion>(4);
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
            toolset: None,
        },
        ctx: SessionCtx {
            workspace_root: PathBuf::from("D:\\工作区\\demo 🎉"),
            permission_mode: AgentPermissionMode::BypassPermissions,
            // 非 High 档入参（passthrough 断言与缺省档可区分）
            model_level: ModelLevel::Low,
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
    assert!(
        matches!(received, AgentEventKind::Raw { .. }),
        "预录观察回流"
    );
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
    assert_eq!(
        captured[0].ctx.model_level,
        ModelLevel::Low,
        "model_level 原样传递（轮级上下文的档位半边）"
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
            model_level: ModelLevel::High,
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
    let boxed: Box<dyn AgentRunner> =
        Box::new(FakeRunner::with_events(vec![AgentEventKind::Raw {
            event_type: "x".to_owned(),
            raw_json: "{}".to_owned(),
        }]));
    let mut session = boxed
        .open_session(new_open())
        .expect("trait object open 应成功");
    assert!(
        session.observations.recv().await.is_some(),
        "trait object 观察可达"
    );

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
        model_level: ModelLevel::High,
    };

    let value = serde_json::to_value(&ctx).expect("序列化成功");
    assert_eq!(value["workspaceRoot"], json!("D:\\工作区"), "驼峰键");
    assert_eq!(value["permissionMode"], json!("acceptEdits"), "档位线值");
    assert_eq!(value["modelLevel"], json!("high"), "模型档位线值");

    // 不加 deny_unknown_fields：多余 JSON 键反序列化成功（additive 演进）
    let mut additive = value.clone();
    additive["futureField"] = json!({ "any": true });
    let roundtrip: SessionCtx = serde_json::from_value(additive).expect("未知字段忽略");
    assert_eq!(roundtrip, ctx);

    let back: SessionCtx = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(back, ctx);

    // 旧形态 JSON（无 modelLevel 键）反序列化承接缺省 High（additive 演进
    // 不破线格式——serde(default) 半边）
    let legacy = serde_json::json!({
        "workspaceRoot": "D:\\工作区",
        "permissionMode": "acceptEdits",
    });
    let legacy_ctx: SessionCtx = serde_json::from_value(legacy).expect("旧形态承接");
    assert_eq!(legacy_ctx.model_level, ModelLevel::High, "缺省 High 档");
}

#[test]
fn session_injections默认形态tools与preamble均为none() {
    let default_injections = SessionInjections::default();
    assert_eq!(default_injections.preamble, None, "preamble None 即不注入");
    assert_eq!(
        default_injections.tools, None,
        "tools None 即引擎默认工具面"
    );

    // tools None 与有值两形态可区分（切片③预留接口位的契约面）
    let narrowed = SessionInjections {
        preamble: None,
        tools: Some(vec!["read".to_owned()]),
        toolset: None,
    };
    assert_ne!(default_injections, narrowed);
}

#[test]
fn session_ref两形态相等语义与互异() {
    assert_eq!(SessionRef::New, SessionRef::New);
    assert_eq!(
        SessionRef::Continue {
            id: "ses-1".to_owned()
        },
        SessionRef::Continue {
            id: "ses-1".to_owned()
        }
    );
    assert_ne!(
        SessionRef::New,
        SessionRef::Continue {
            id: "ses-1".to_owned()
        }
    );
    assert_ne!(
        SessionRef::Continue {
            id: "ses-1".to_owned()
        },
        SessionRef::Continue {
            id: "ses-2".to_owned()
        },
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
        (
            AgentStartError::CliMissing("PATH 上未发现入口".to_owned()),
            "CLI 未找到",
        ),
        (
            AgentStartError::SpawnFailed("io error".to_owned()),
            "启动失败",
        ),
        (
            AgentStartError::ConfigMissing("api_key 未配".to_owned()),
            "配置缺失",
        ),
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

// ---------------------------------------------------------------------------
// auto-named-explore：SessionToolset 受控枚举 serde 线值 + SessionInjections
// toolset 位（additive 挂起激活）
// ---------------------------------------------------------------------------

/// SessionToolset 序列化线值：`Explore` → `"explore"`（camelCase 口径），且
/// round-trip 等值。
#[test]
fn session_toolset_explore序列化为explore线值且roundtrip等值() {
    let value = serde_json::to_value(SessionToolset::Explore).expect("序列化成功");
    assert_eq!(value, json!("explore"), "线值逐字 explore");

    let roundtrip: SessionToolset = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, SessionToolset::Explore, "round-trip 等值");
}

/// SessionToolset 反序列化：线值 `"explore"` → `SessionToolset::Explore`。
#[test]
fn session_toolset_explore线值反序列化为explore变体() {
    let parsed: SessionToolset =
        serde_json::from_value(json!("explore")).expect("合法线值应反序列化成功");
    assert_eq!(parsed, SessionToolset::Explore);
}

/// SessionToolset 非法线值：大小写敏感的 `"Explore"` / 未知 `"unknown"` / 空串
/// 反序列化一律 `Err`（受控枚举封闭集，不静默兜底）。
#[test]
fn session_toolset非法线值反序列化err_大小写敏感与未知与空串() {
    for line in ["Explore", "EXPLORE", "unknown", ""] {
        let result = serde_json::from_value::<SessionToolset>(json!(line));
        assert!(
            result.is_err(),
            "非法线值 {line:?} 应反序列化 Err（大小写敏感封闭集）"
        );
    }
}

/// SessionInjections 全字段序列化：preamble / tools / toolset 三位齐备 →
/// `toolset` 线值为 `"explore"`（位名逐字 toolset）。
#[test]
fn session_injections全字段序列化_toolset线值为explore() {
    let injections = SessionInjections {
        preamble: Some("前导".to_owned()),
        tools: Some(vec![
            "read".to_owned(),
            "grep".to_owned(),
            "glob".to_owned(),
            "ls".to_owned(),
            "edit".to_owned(),
            "bash".to_owned(),
        ]),
        toolset: Some(SessionToolset::Explore),
    };

    let value = serde_json::to_value(&injections).expect("序列化成功");
    assert!(value.is_object(), "序列化为对象");
    assert_eq!(value["preamble"], json!("前导"), "preamble 位在场");
    assert!(value["tools"].is_array(), "tools 位为数组");
    assert_eq!(value["toolset"], json!("explore"), "toolset 线值 explore");

    let roundtrip: SessionInjections = serde_json::from_value(value).expect("round-trip 成功");
    assert_eq!(roundtrip, injections, "三位齐备 round-trip 逐字段等值");
}

/// SessionInjections 旧线 JSON additive：无 `toolset` 字段 → 反序列化成功且
/// `toolset == None`（serde default 缺省读兼容——旧线不破）。
#[test]
fn session_injections旧线json无toolset字段读出none() {
    let legacy = json!({
        "preamble": "会话前导",
        "tools": ["read", "grep"],
    });
    let parsed: SessionInjections = serde_json::from_value(legacy).expect("旧线应可读");
    assert_eq!(parsed.preamble.as_deref(), Some("会话前导"));
    assert_eq!(
        parsed.tools,
        Some(vec!["read".to_owned(), "grep".to_owned()]),
        "旧线 tools 位照常承接"
    );
    assert_eq!(
        parsed.toolset, None,
        "缺席 toolset 位 → None（additive 兼容）"
    );
}

/// SessionInjections 空对象 `{}` → 三位全 `None`，与 `default()` 等价。
#[test]
fn session_injections空对象反序列化三位全none且与default等价() {
    let parsed: SessionInjections = serde_json::from_value(json!({})).expect("空对象应可读");
    assert_eq!(parsed, SessionInjections::default(), "空对象 == default()");
    assert_eq!(parsed.preamble, None);
    assert_eq!(parsed.tools, None);
    assert_eq!(parsed.toolset, None);
}

/// SessionInjections `toolset: null` → 读出 `None`（显式 null 与缺席同语义）。
#[test]
fn session_injections_toolset显式null读出none() {
    let parsed: SessionInjections =
        serde_json::from_value(json!({ "toolset": null })).expect("null 位应可读");
    assert_eq!(parsed.toolset, None, "显式 null → None");
    assert_eq!(parsed.preamble, None);
    assert_eq!(parsed.tools, None);
}

/// SessionInjections 子集组合 round-trip：`tools` = 探索六名子集 + `toolset` =
/// Explore → 序列化后反序列化逐字段等值（下发面载荷的线格式保真）。
#[test]
fn session_injections六名子集加explore组合roundtrip逐字段等值() {
    let injections = SessionInjections {
        preamble: None,
        tools: Some(vec![
            "read".to_owned(),
            "grep".to_owned(),
            "glob".to_owned(),
            "ls".to_owned(),
            "edit".to_owned(),
            "bash".to_owned(),
        ]),
        toolset: Some(SessionToolset::Explore),
    };

    let line = serde_json::to_string(&injections).expect("序列化成功");
    let parsed: SessionInjections = serde_json::from_str(&line).expect("反序列化成功");
    assert_eq!(parsed, injections, "round-trip 逐字段等值");
    assert_eq!(parsed.tools.as_ref().expect("六名在场").len(), 6);
    assert!(
        !parsed
            .tools
            .expect("六名在场")
            .contains(&"write".to_owned()),
        "探索子集不含 write"
    );
    assert_eq!(parsed.toolset, Some(SessionToolset::Explore));
}

// ---------------------------------------------------------------------------
// archi 章节：`SessionToolset::Archi` 线值与旧线兼容
// ---------------------------------------------------------------------------

/// 正向：`Archi` 四组合序列化线值（camelCase 外层变体名 + 内层布尔字段）。
#[test]
fn session_toolset_archi四组合线值() {
    let cases = [
        (
            (true, true),
            json!({"archi": {"module": true, "search": true}}),
        ),
        (
            (true, false),
            json!({"archi": {"module": true, "search": false}}),
        ),
        (
            (false, true),
            json!({"archi": {"module": false, "search": true}}),
        ),
        (
            (false, false),
            json!({"archi": {"module": false, "search": false}}),
        ),
    ];
    for ((module, search), expected) in cases {
        let value =
            serde_json::to_value(SessionToolset::Archi { module, search }).expect("序列化成功");
        assert_eq!(
            value, expected,
            "Archi 线值逐字段（module={module}, search={search}）"
        );
    }
}

/// 正向：四线值反序列化 round-trip 等值；`Explore` 线值逐字不变。
#[test]
fn session_toolset_archi反序列化roundtrip() {
    for (module, search) in [(true, true), (true, false), (false, true), (false, false)] {
        let value = json!({"archi": {"module": module, "search": search}});
        let parsed: SessionToolset = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(parsed, SessionToolset::Archi { module, search });
    }
    let parsed: SessionToolset =
        serde_json::from_value(json!("explore")).expect("Explore 线值反序列化成功");
    assert_eq!(parsed, SessionToolset::Explore, "Explore 臂零变化");
}

/// 边界：旧线兼容——`SessionInjections` 旧 JSON 无 `toolset` 字段 / `toolset: null`
/// → 读出 `None`（`#[serde(default)]`）。
#[test]
fn session_injections旧线无toolset字段读出none() {
    let legacy = json!({ "preamble": null, "tools": null });
    let injections: SessionInjections = serde_json::from_value(legacy).expect("旧线应可反序列化");
    assert_eq!(injections.toolset, None, "缺字段读出 None");

    let explicit_null = json!({ "preamble": null, "tools": null, "toolset": null });
    let injections: SessionInjections =
        serde_json::from_value(explicit_null).expect("null 线值应可反序列化");
    assert_eq!(injections.toolset, None, "null 读出 None");

    // 新线：Archi 变体可回读
    let archi = json!({
        "preamble": null,
        "tools": null,
        "toolset": { "archi": { "module": true, "search": false } }
    });
    let injections: SessionInjections = serde_json::from_value(archi).expect("新线应可反序列化");
    assert_eq!(
        injections.toolset,
        Some(SessionToolset::Archi {
            module: true,
            search: false
        })
    );
}

/// 异常：非法线值一律 `Err`（大小写敏感 / 缺布尔字段 / 非 bool / 未知变体 / 空串）。
#[test]
fn session_toolset_archi非法线值err() {
    for line in [
        json!("Archi"),
        json!({"archi": {}}),
        json!({"archi": {"module": 1, "search": true}}),
        json!({"archi": {"module": "true", "search": false}}),
        json!({"unknown": {}}),
        json!("unknown"),
        json!(""),
    ] {
        let result = serde_json::from_value::<SessionToolset>(line.clone());
        assert!(result.is_err(), "非法线值 {line} 应反序列化 Err");
    }
}

/// 回归：`Explore` 臂序列化逐字不变；新增变体不进入 `tools` 名单表达。
#[test]
fn session_toolset_explore臂零回归与tools名单分离() {
    assert_eq!(
        serde_json::to_value(SessionToolset::Explore).expect("序列化成功"),
        json!("explore"),
        "Explore 线值逐字"
    );
    let injections = SessionInjections {
        preamble: None,
        tools: Some(vec!["read".to_owned()]),
        toolset: Some(SessionToolset::Archi {
            module: true,
            search: true,
        }),
    };
    let value = serde_json::to_value(&injections).expect("序列化成功");
    assert_eq!(
        value["tools"],
        json!(["read"]),
        "`tools` 名单只承载七工具子集"
    );
    assert_eq!(
        value["toolset"],
        json!({"archi": {"module": true, "search": true}}),
        "枚举位是 archi 工具面的唯一入口"
    );
}

/// 边界：crate 根与模块路径同词汇（`crate::SessionToolset::Archi` 可构造）。
#[test]
fn session_toolset再导出锚定() {
    let from_root: crate::SessionToolset = crate::SessionToolset::Archi {
        module: true,
        search: false,
    };
    let from_runner: SessionToolset = SessionToolset::Archi {
        module: true,
        search: false,
    };
    assert_eq!(from_root, from_runner, "两路径同词汇同值");
}
