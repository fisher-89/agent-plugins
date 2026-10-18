use std::sync::Arc;

use crate::event::{AgentBlock, AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole};
use crate::kernel::{KernelOutput, RunningTurn, SessionKernel, StopRegistry, TurnRequest};
use crate::port::{SessionQuery, SessionSink, TurnOutcome};
use crate::runner::{
    AgentPermissionMode, AgentRunStatus, AgentRunner, AgentSession, AgentStartError, ModelLevel,
    RunHandle, SessionCtx, SessionInjections, SessionOpen, SessionRef, SessionToolset,
    TurnQuestion,
};
use crate::session::{
    new_session_id, NewSessionRow, SessionPage, SessionProvenance, SessionRow, SessionStats,
    SessionSummary, TurnSummary,
};
use crate::state::{AgentRunState, RunStateMachine};
use crate::stream::{
    SessionStreamItem, SessionStreamRecvError, SessionStreamRegistry, SessionStreamSubscription,
};

/// crate 根 use 可达性锚定：上列导入任一破坏即编译失败。构造形态断言确保
/// 导入非仅「被 use」而确可用。
#[test]
fn crate根导出面锚定_session_port_kernel新模块类型可达() {
    // session 域契约类型构造形态
    let provenance = SessionProvenance {
        source: "debug".to_owned(),
        source_ref: None,
    };
    let row = SessionRow {
        id: new_session_id(),
        remote_session_id: None,
        config_snapshot: serde_json::json!({}),
        provenance: provenance.clone(),
        created_at: 0,
        updated_at: 0,
    };
    let summary = SessionSummary {
        row: row.clone(),
        stats: SessionStats {
            turn_count: 0,
            total_duration_ms: None,
            input_tokens: None,
            output_tokens: None,
        },
        turns: vec![TurnSummary {
            turn_id: 1,
            session_id: row.id.clone(),
            status: AgentRunStatus::Running,
            started_at: 0,
            finished_at: None,
            num_turns: None,
            cost_usd: None,
            duration_ms: None,
            error: None,
        }],
    };
    assert_eq!(summary.row.id, row.id);

    // port / kernel 类型构造形态
    let registry = StopRegistry::new();
    let _kernel = SessionKernel::new(
        Arc::new(NoopSink),
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
    );
    let request = TurnRequest {
        session: SessionRef::New,
        question: String::new(),
        injections: SessionInjections::default(),
        ctx: SessionCtx {
            workspace_root: std::path::PathBuf::from("."),
            permission_mode: AgentPermissionMode::Default,
            model_level: ModelLevel::High,
        },
        provenance,
        config_snapshot: serde_json::json!({}),
        prior_handle: None,
    };
    let _ = format!("{request:?}");

    // 事件词汇与状态机形态
    let event = AgentEvent::stamp(
        0,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: None,
            delta: AgentDelta::Text {
                text: String::new(),
            },
        },
    );
    let mut machine = RunStateMachine::new();
    assert_eq!(machine.apply(&event), AgentRunState::Running);
    assert!(event.kind.is_delta());
    assert!(
        AgentBlock::Text {
            text: String::new()
        } != AgentBlock::Thinking {
            thinking: String::new()
        }
    );

    // 治理面 / 协议面形态
    registry.register("ses-anchor", RunHandle::default());
    let _outcome_shape: Option<TurnOutcome> = None;
    let _kernel_output_shape: Option<KernelOutput> = None;
    let _running_turn_shape: Option<RunningTurn> = None;
    let _session_shape: Option<AgentSession> = None;
    let _open_shape: Option<SessionOpen> = None;
    let _question_shape: Option<TurnQuestion> = None;
    let _new_row_shape: Option<NewSessionRow> = None;
    let _error_shape: Option<AgentStartError> = None;
}

/// crate 根 re-export 的 sink trait 可实现性锚定（外部实现方形态）。
struct NoopSink;

impl SessionSink for NoopSink {
    fn create_session(&self, _session: &NewSessionRow) -> Result<(), String> {
        Ok(())
    }

    fn begin_turn(&self, _session_id: &str, _started_at: i64) -> Result<i64, String> {
        Ok(1)
    }

    fn next_seq(&self, _session_id: &str) -> Result<u64, String> {
        Ok(0)
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

/// query trait 同样可达可实现（返回形态 Vec<SessionSummary> / Vec<AgentEvent> /
/// SessionStats 以签名锚定）。
struct NoopQuery;

impl SessionQuery for NoopQuery {
    fn list_sessions(
        &self,
        _source: Option<&str>,
        _source_ref: Option<&str>,
        _offset: u32,
        _limit: u32,
    ) -> Result<SessionPage, String> {
        Ok(SessionPage {
            sessions: Vec::new(),
            total: 0,
        })
    }

    fn find_session_detail(&self, _session_id: &str) -> Result<SessionSummary, String> {
        Err("NoopQuery 不响应单查".to_owned())
    }

    fn transcript(&self, _session_id: &str) -> Result<Vec<AgentEvent>, String> {
        Ok(Vec::new())
    }

    fn reconcile_stats(&self, _session_id: &str) -> Result<SessionStats, String> {
        Ok(SessionStats {
            turn_count: 0,
            total_duration_ms: None,
            input_tokens: None,
            output_tokens: None,
        })
    }
}

/// AgentRunner trait 经 crate 根可达可实现（退役的 start 入口随协议退役，
/// trait 面仅 open_session——旧协议类型标识在本文件不可出现）。
struct NoopRunner;

impl AgentRunner for NoopRunner {
    fn open_session(&self, _open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        Err(AgentStartError::ConfigMissing("noop".to_owned()))
    }
}

#[test]
fn crate根trait导出可实现且退役协议类型不再出现在根面() {
    fn assert_runner<R: AgentRunner>(_: &R) {}
    assert_runner(&NoopRunner);
    fn assert_query<Q: SessionQuery>(_: &Q) {}
    assert_query(&NoopQuery);
    fn assert_sink<S: SessionSink>(_: &S) {}
    assert_sink(&NoopSink);
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（D3 / AC-10）：`pub use usage::sum_usage_tokens`
// 根面锚——唯一实现的根路径可达（AC-10 唯一实现锚的根面半边；语义全量归
// usage_test.rs 节）
// ---------------------------------------------------------------------------

/// 经根路径 `crate::sum_usage_tokens` 持有并调用 → 编译可达即锚；函数项签名
/// 形态 = `fn(&[AgentEvent], &str) -> Option<u64>`（D3 声明）。
#[test]
fn crate根路径sum_usage_tokens可达与签名锚定() {
    // 根路径 re-export 持有（签名形态断言——持有根路径函数项）
    let root_fn: fn(&[AgentEvent], &str) -> Option<u64> = crate::sum_usage_tokens;
    // 与 usage 模块路径同一实现（同一 fn item——函数项指针等值）
    let module_fn: fn(&[AgentEvent], &str) -> Option<u64> = crate::usage::sum_usage_tokens;
    assert_eq!(
        root_fn as usize, module_fn as usize,
        "根路径与 usage 模块路径为同一实现（re-export 零包装）"
    );

    // 行为面轻量锚：空转录 → None
    assert_eq!(root_fn(&[], "input_tokens"), None);

    // 单值转录：根路径调用语义与 usage_test 全量面同源
    let events = vec![AgentEvent::stamp(
        0,
        AgentEventKind::TurnDone {
            subtype: String::new(),
            is_error: false,
            num_turns: None,
            duration_ms: None,
            cost_usd: None,
            usage: serde_json::json!({ "input_tokens": 7 }),
            session_id: None,
        },
    )];
    assert_eq!(root_fn(&events, "input_tokens"), Some(7));
}

/// 缺席 key（`""` / 未知键）经根路径调用 → None；同一转录重复调用两次结果
/// 一致（re-export 零状态零副作用——防御锚）。
#[test]
fn crate根路径sum_usage_tokens缺席key与重复调用稳定() {
    let root_fn: fn(&[AgentEvent], &str) -> Option<u64> = crate::sum_usage_tokens;
    let events = vec![AgentEvent::stamp(
        0,
        AgentEventKind::TurnDone {
            subtype: String::new(),
            is_error: false,
            num_turns: None,
            duration_ms: None,
            cost_usd: None,
            usage: serde_json::json!({ "input_tokens": 7 }),
            session_id: None,
        },
    )];
    assert_eq!(root_fn(&events, ""), None, "空串 key 缺席 → None");
    assert_eq!(root_fn(&events, "total_tokens"), None, "未知键缺席 → None");
    assert_eq!(
        root_fn(&events, "input_tokens"),
        root_fn(&events, "input_tokens"),
        "重复调用结果一致（零状态零副作用）"
    );
}

// ---------------------------------------------------------------------------
// auto-named-explore：SessionToolset 再导出与 SessionInjections 出口回归
// ---------------------------------------------------------------------------

/// `SessionToolset` 再导出可达：经 crate 根路径构造 `Explore` 变体并
/// `PartialEq` 等值锚定（再导出清单增补的词汇出口——破坏即编译失败）。
#[test]
fn crate根路径session_toolset可达且explore变体等值() {
    // 根路径构造 + 等值（词汇本体驻 runner.rs，出口侧仅锚定可达与语义）
    let toolset = crate::SessionToolset::Explore;
    assert_eq!(toolset, SessionToolset::Explore, "根路径与模块路径同词汇");
    // serde 可用性经出口侧复验（derive 随词汇出口可达）
    let line = serde_json::to_string(&toolset).expect("序列化可达");
    assert_eq!(line, r#""explore""#, "线值经出口侧不变");
}

/// 既有再导出面零回归：runner / session / port / kernel / state / event /
/// usage 各域再导出词汇快照逐项可达（增补 `SessionToolset` 不挤掉既有出口
/// ——清单丢失即编译失败，构造形态防「仅被 use」假锚）。
#[test]
fn 既有再导出面快照逐名可达_增补不挤掉既有词汇() {
    // runner 域（含增补的 SessionToolset 与既有词汇；trait 面以指针形态锚定）
    let _runner_word_shapes: Option<(
        AgentPermissionMode,
        AgentRunStatus,
        Box<dyn AgentRunner>,
        AgentSession,
        AgentStartError,
        ModelLevel,
        RunHandle,
        SessionCtx,
        SessionInjections,
        SessionOpen,
        SessionRef,
        SessionToolset,
        TurnQuestion,
    )> = None;
    let _ = SessionToolset::Explore;
    // session / port / kernel / state / event / usage 域既有快照（与文件顶部
    // use 清单一一对应，任一名丢失即编译失败）
    let _session_word_shapes: Option<(
        NewSessionRow,
        SessionPage,
        SessionProvenance,
        SessionRow,
        SessionStats,
        SessionSummary,
        TurnSummary,
    )> = None;
    let _port_word_shapes: Option<(Box<dyn SessionQuery>, Box<dyn SessionSink>, TurnOutcome)> =
        None;
    let _kernel_word_shapes: Option<(KernelOutput, RunningTurn, SessionKernel, StopRegistry, TurnRequest)> =
        None;
    let _state_word_shapes: Option<(AgentRunState, RunStateMachine)> = None;
    let _event_word_shapes: Option<(AgentBlock, AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole)> =
        None;
    let _usage_fn: fn(&[AgentEvent], &str) -> Option<u64> = crate::sum_usage_tokens;

    // 构造形态断言（防「仅被 use」假锚）：增补词汇真实可用
    let default_injections = SessionInjections::default();
    assert_eq!(default_injections.toolset, None, "增补位缺省 None");
}

/// toolset 非法线值经出口反序列化：crate 根路径 `SessionInjections` 喂
/// `{"toolset":"unknown"}` → serde `Err`（再导出面承载的失败语义不吞改，与
/// runner_test 非法线值臂同口径、不同进口）。
#[test]
fn toolset非法线值经出口反序列化err() {
    let result: Result<SessionInjections, _> =
        serde_json::from_str(r#"{"toolset":"unknown"}"#);
    assert!(
        result.is_err(),
        "出口侧非法 toolset 线值应 serde Err，实际: {result:?}"
    );
}

/// SessionInjections 缺省出口锚定：经 crate 根路径 `SessionInjections::default()`
/// → 三位全 `None`（additive 缺省读兼容的出口侧锚定）。
#[test]
fn crate根路径session_injections_default三位全none() {
    let default_injections = SessionInjections::default();
    assert_eq!(default_injections.preamble, None, "preamble None");
    assert_eq!(default_injections.tools, None, "tools None");
    assert_eq!(default_injections.toolset, None, "toolset None（增补位缺省）");
}

// ---------------------------------------------------------------------------
// 会话实时流注册表 re-export（`mod stream` 私有，crate 根为唯一可达路径）：
// 类型出口与行为面同一（出口不引入分支，语义仍由 stream 模块单点决定）
// ---------------------------------------------------------------------------

fn stream_outcome() -> TurnOutcome {
    TurnOutcome {
        turn_id: 1,
        status: AgentRunStatus::Completed,
        started_at: 1_727_000_000_000,
        finished_at: 1_727_000_005_000,
        num_turns: None,
        duration_ms: None,
        cost_usd: None,
        usage: serde_json::Value::Null,
        error: None,
        remote_session_id: None,
    }
}

/// 根路径全链路可用性锚定：构造 → `begin` → `publish` → `subscribe` →
/// `recv` → `finish` 经 `crate::SessionStreamRegistry` 一路走通。
#[tokio::test]
async fn crate根路径会话流注册表全链路可用() {
    let registry: SessionStreamRegistry = crate::SessionStreamRegistry::new();
    registry.begin("ses-root");
    let mut subscription: SessionStreamSubscription = registry
        .subscribe("ses-root", 0)
        .expect("根路径注册后应可订阅");

    registry.publish(
        "ses-root",
        AgentEvent::stamp(
            1,
            AgentEventKind::MessageDelta {
                parent_tool_use_id: None,
                delta: AgentDelta::Text {
                    text: "根路径增量".to_owned(),
                },
            },
        ),
    );
    let first: SessionStreamItem = subscription.recv().await.expect("增量可得");
    assert!(
        matches!(first, SessionStreamItem::Event(ref event) if event.seq == 1),
        "根路径出口的行为面与 stream 模块同一实现"
    );

    registry.finish("ses-root", stream_outcome());
    let terminal = subscription.recv().await.expect("终态可得");
    assert!(
        matches!(terminal, SessionStreamItem::Finished(ref outcome) if outcome.turn_id == 1),
        "终态变体经根路径 match"
    );
    assert!(
        registry.subscribe("ses-root", 0).is_none(),
        "投终态即除名（根路径语义不另立分支）"
    );
}

/// 两变体与收尾错误经根路径比较：`Finished` 载荷可读、`Closed` / `Lagged`
/// 可 `PartialEq` 断言。
#[tokio::test]
async fn crate根路径会话流条目与收尾错误变体可match与比较() {
    let registry = SessionStreamRegistry::new();
    registry.begin("ses-variants");
    let mut subscription = registry.subscribe("ses-variants", 0).expect("可订阅");
    registry.finish("ses-variants", stream_outcome());

    match subscription.recv().await {
        Ok(SessionStreamItem::Finished(outcome)) => {
            assert_eq!(outcome.status, AgentRunStatus::Completed)
        }
        other => panic!("应为 Finished，实际: {other:?}"),
    }
    let closed: SessionStreamRecvError = subscription
        .recv()
        .await
        .err()
        .expect("排空后 Closed");
    assert_eq!(closed, crate::SessionStreamRecvError::Closed);
    assert_ne!(closed, crate::SessionStreamRecvError::Lagged(1));
}

/// `Default` 与 `new()` 等价空表且两实例互不共享条目。
#[test]
fn crate根路径default与new等价空表且两实例互不共享() {
    let first = crate::SessionStreamRegistry::default();
    let second = crate::SessionStreamRegistry::new();

    first.begin("ses-shared");
    assert!(
        first.subscribe("ses-shared", 0).is_some(),
        "登记处可订阅"
    );
    assert!(
        second.subscribe("ses-shared", 0).is_none(),
        "另一实例零条目（空表互不共享）"
    );
    assert!(
        second.subscribe("", 0).is_none(),
        "空表对空白键同样 None"
    );
}

/// 未登记 id 经根路径订阅 → `None`（re-export 不引入行为分支）。
#[test]
fn crate根路径未登记id订阅为none() {
    let registry = crate::SessionStreamRegistry::new();
    assert!(registry.subscribe("ses-404", 0).is_none());
    assert!(registry.subscribe("", 0).is_none());
}
