use std::sync::Arc;

use crate::event::{AgentBlock, AgentDelta, AgentEvent, AgentEventKind};
use crate::kernel::{KernelOutput, RunningTurn, SessionKernel, StopRegistry, TurnRequest};
use crate::port::{SessionQuery, SessionSink, TurnOutcome};
use crate::runner::{
    AgentPermissionMode, AgentRunStatus, AgentRunner, AgentSession, AgentStartError, RunHandle,
    SessionCtx, SessionInjections, SessionOpen, SessionRef, TurnQuestion,
};
use crate::session::{
    new_session_id, NewSessionRow, SessionProvenance, SessionRow, SessionStats, SessionSummary,
    TurnSummary,
};
use crate::state::{AgentRunState, RunStateMachine};

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
    let _kernel = SessionKernel::new(Arc::new(NoopSink), Arc::new(StopRegistry::new()));
    let request = TurnRequest {
        session: SessionRef::New,
        question: String::new(),
        injections: SessionInjections::default(),
        ctx: SessionCtx {
            workspace_root: std::path::PathBuf::from("."),
            permission_mode: AgentPermissionMode::Default,
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
    assert!(AgentBlock::Text { text: String::new() } != AgentBlock::Thinking {
        thinking: String::new()
    });

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
    ) -> Result<Vec<SessionSummary>, String> {
        Ok(Vec::new())
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
