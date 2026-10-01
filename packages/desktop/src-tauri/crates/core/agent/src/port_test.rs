//! `port` 的单元测试（AC-6）：SessionSink / SessionQuery trait 面的假实现
//! 注入（trait 本身即注入面，object safety 与方法签名编译期锚定）、
//! TurnOutcome 共享收口类型的 serde camelCase 往返、Err(String) 语义传播与
//! Send+Sync 边界。真实组合在 infra 侧 store_port_test 承载。

use std::sync::{Arc, Mutex};

use crate::event::{AgentEvent, AgentEventKind, AgentMessageRole};
use crate::port::{SessionQuery, SessionSink, TurnOutcome};
use crate::runner::AgentRunStatus;
use crate::session::{NewSessionRow, SessionProvenance, SessionRow, SessionStats, SessionSummary};

// ---------------------------------------------------------------------------
// 假 sink / 假 query：内存记录调用序与载荷（trait 本身即注入面）
// ---------------------------------------------------------------------------

/// sink 调用记录（五方法一维序列，调用序即落库纪律的观测面）。
#[derive(Debug, Clone, PartialEq)]
enum SinkCall {
    CreateSession(NewSessionRow),
    BeginTurn { session_id: String, started_at: i64 },
    AppendSealed { session_id: String, seq: u64 },
    FinishTurn { turn_id: i64 },
    BindRemote { session_id: String, remote: String },
}

/// 假 sink：共享调用记录（Arc<dyn> 注入后测试侧仍可读回），可编程失败点
/// （fail_on 序号处返回 Err）。
struct FakeSink {
    calls: Arc<Mutex<Vec<SinkCall>>>,
    fail_on: Option<usize>,
}

impl FakeSink {
    fn new() -> (Self, Arc<Mutex<Vec<SinkCall>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
                fail_on: None,
            },
            calls,
        )
    }

    fn failing_on(index: usize) -> (Self, Arc<Mutex<Vec<SinkCall>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
                fail_on: Some(index),
            },
            calls,
        )
    }

    fn fail_result(&self) -> Result<(), String> {
        let index = self.calls.lock().expect("记录锁不可中毒").len() - 1;
        if self.fail_on == Some(index) {
            Err(format!("存储失败: 调用 #{index}"))
        } else {
            Ok(())
        }
    }
}

impl SessionSink for FakeSink {
    fn create_session(&self, session: &NewSessionRow) -> Result<(), String> {
        self.calls
            .lock()
            .expect("记录锁不可中毒")
            .push(SinkCall::CreateSession(session.clone()));
        self.fail_result()
    }

    fn begin_turn(&self, session_id: &str, started_at: i64) -> Result<i64, String> {
        self.calls
            .lock()
            .expect("记录锁不可中毒")
            .push(SinkCall::BeginTurn {
                session_id: session_id.to_owned(),
                started_at,
            });
        self.fail_result()?;
        Ok(1)
    }

    fn append_sealed(&self, session_id: &str, event: &AgentEvent) -> Result<(), String> {
        self.calls
            .lock()
            .expect("记录锁不可中毒")
            .push(SinkCall::AppendSealed {
                session_id: session_id.to_owned(),
                seq: event.seq,
            });
        self.fail_result()
    }

    fn finish_turn(&self, turn_id: i64, _outcome: &TurnOutcome) -> Result<(), String> {
        self.calls
            .lock()
            .expect("记录锁不可中毒")
            .push(SinkCall::FinishTurn { turn_id });
        self.fail_result()
    }

    fn bind_remote_session(
        &self,
        session_id: &str,
        remote: &str,
        _updated_at: i64,
    ) -> Result<(), String> {
        self.calls
            .lock()
            .expect("记录锁不可中毒")
            .push(SinkCall::BindRemote {
                session_id: session_id.to_owned(),
                remote: remote.to_owned(),
            });
        self.fail_result()
    }
}

/// 假 query：三方法返回固定形态（查询契约的返回形态锚定）。
struct FakeQuery {
    summaries: Vec<SessionSummary>,
    events: Vec<AgentEvent>,
    stats: SessionStats,
}

impl SessionQuery for FakeQuery {
    fn list_sessions(
        &self,
        _source: Option<&str>,
        _source_ref: Option<&str>,
    ) -> Result<Vec<SessionSummary>, String> {
        Ok(self.summaries.clone())
    }

    fn transcript(&self, _session_id: &str) -> Result<Vec<AgentEvent>, String> {
        Ok(self.events.clone())
    }

    fn reconcile_stats(&self, _session_id: &str) -> Result<SessionStats, String> {
        Ok(self.stats.clone())
    }
}

fn new_session_row() -> NewSessionRow {
    NewSessionRow {
        id: "ses-1-1727000000000".to_owned(),
        config_snapshot: serde_json::json!({ "engine": "sdk" }),
        provenance: SessionProvenance {
            source: "debug".to_owned(),
            source_ref: None,
        },
    }
}

fn sealed_event(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    )
}

fn outcome() -> TurnOutcome {
    TurnOutcome {
        turn_id: 2,
        status: AgentRunStatus::Completed,
        finished_at: 1727000005000,
        num_turns: Some(4),
        duration_ms: Some(5000),
        cost_usd: Some(0.12),
        usage: serde_json::json!({ "inputTokens": 10, "outputTokens": 20 }),
        error: None,
        remote_session_id: Some("sdk-3-1727000000009".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// SessionSink / SessionQuery trait 面（Arc<dyn> 注入 + 调用序与载荷记录）
// ---------------------------------------------------------------------------

#[test]
fn 假sink经arc_dyn注入五方法调用序与载荷原样记录() {
    let (fake, calls) = FakeSink::new();
    let sink: Arc<dyn SessionSink> = Arc::new(fake);
    let event = sealed_event(5);

    sink.create_session(&new_session_row())
        .expect("create 应成功");
    let turn_id = sink
        .begin_turn("ses-1-1727000000000", 1727000000000)
        .expect("begin 应成功");
    sink.append_sealed("ses-1-1727000000000", &event)
        .expect("append 应成功");
    sink.finish_turn(turn_id, &outcome())
        .expect("finish 应成功");
    sink.bind_remote_session("ses-1-1727000000000", "sdk-3-1727000000009", 1727000006000)
        .expect("bind 应成功");

    assert_eq!(
        *calls.lock().expect("记录锁不可中毒"),
        vec![
            SinkCall::CreateSession(new_session_row()),
            SinkCall::BeginTurn {
                session_id: "ses-1-1727000000000".to_owned(),
                started_at: 1727000000000,
            },
            SinkCall::AppendSealed {
                session_id: "ses-1-1727000000000".to_owned(),
                seq: 5,
            },
            SinkCall::FinishTurn { turn_id: 1 },
            SinkCall::BindRemote {
                session_id: "ses-1-1727000000000".to_owned(),
                remote: "sdk-3-1727000000009".to_owned(),
            },
        ],
        "五方法调用序与载荷原样记录（write-through 契约面）"
    );
}

#[test]
fn 假query经arc_dyn注入三方法返回形态编译锚定() {
    let query: Arc<dyn SessionQuery> = Arc::new(FakeQuery {
        summaries: vec![SessionSummary {
            row: SessionRow {
                id: "ses-1-1727000000000".to_owned(),
                remote_session_id: None,
                config_snapshot: serde_json::json!({}),
                provenance: SessionProvenance {
                    source: "debug".to_owned(),
                    source_ref: None,
                },
                created_at: 1,
                updated_at: 1,
            },
            stats: SessionStats {
                turn_count: 0,
                total_duration_ms: None,
                input_tokens: None,
                output_tokens: None,
            },
            turns: Vec::new(),
        }],
        events: vec![sealed_event(0)],
        stats: SessionStats {
            turn_count: 1,
            total_duration_ms: Some(10),
            input_tokens: None,
            output_tokens: None,
        },
    });

    let summaries = query
        .list_sessions(Some("debug"), None)
        .expect("list_sessions 应成功");
    let events = query
        .transcript("ses-1-1727000000000")
        .expect("transcript 应成功");
    let stats = query
        .reconcile_stats("ses-1-1727000000000")
        .expect("reconcile_stats 应成功");

    assert_eq!(summaries.len(), 1, "Vec<SessionSummary> 返回形态");
    assert_eq!(events.len(), 1, "Vec<AgentEvent> 返回形态");
    assert_eq!(stats.turn_count, 1, "SessionStats 返回形态");
    assert_eq!(summaries[0].row.id, "ses-1-1727000000000");
}

// ---------------------------------------------------------------------------
// TurnOutcome：serde camelCase 往返（全字段 / 缺席两极）
// ---------------------------------------------------------------------------

#[test]
fn turn_outcome全字段驼峰线格式往返逐字段相等() {
    let value = serde_json::to_value(outcome()).expect("序列化成功");
    assert_eq!(value["turnId"], serde_json::json!(2));
    assert_eq!(value["status"], serde_json::json!("completed"));
    assert_eq!(value["finishedAt"], serde_json::json!(1727000005000_i64));
    assert_eq!(value["numTurns"], serde_json::json!(4));
    assert_eq!(value["durationMs"], serde_json::json!(5000));
    assert_eq!(value["costUsd"], serde_json::json!(0.12));
    assert_eq!(
        value["usage"]["inputTokens"],
        serde_json::json!(10),
        "usage 原值透传"
    );
    assert_eq!(value["error"], serde_json::Value::Null);
    assert_eq!(
        value["remoteSessionId"],
        serde_json::json!("sdk-3-1727000000009")
    );

    let roundtrip: TurnOutcome = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, outcome());
}

#[test]
fn turn_outcome缺席两极与status四值线格式往返() {
    // 全 null 极
    let bare = TurnOutcome {
        turn_id: 1,
        status: AgentRunStatus::Stopped,
        finished_at: 1727000001000,
        num_turns: None,
        duration_ms: None,
        cost_usd: None,
        usage: serde_json::Value::Null,
        error: None,
        remote_session_id: None,
    };
    let value = serde_json::to_value(&bare).expect("序列化成功");
    assert_eq!(value["usage"], serde_json::Value::Null);
    assert_eq!(value["remoteSessionId"], serde_json::Value::Null);
    let roundtrip: TurnOutcome = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, bare);

    // error 记因极
    let failed = TurnOutcome {
        status: AgentRunStatus::Failed,
        error: Some("事件落库失败: db: x".to_owned()),
        ..outcome()
    };
    let value = serde_json::to_value(&failed).expect("序列化成功");
    assert_eq!(value["status"], serde_json::json!("failed"));
    assert_eq!(value["error"], serde_json::json!("事件落库失败: db: x"));
    let roundtrip: TurnOutcome = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, failed);

    // status 四值线格式
    for (status, expected) in [
        (AgentRunStatus::Running, "running"),
        (AgentRunStatus::Completed, "completed"),
        (AgentRunStatus::Failed, "failed"),
        (AgentRunStatus::Stopped, "stopped"),
    ] {
        let value = serde_json::to_value(TurnOutcome {
            status,
            ..outcome()
        })
        .expect("序列化成功");
        assert_eq!(value["status"], serde_json::json!(expected));
    }
}

// ---------------------------------------------------------------------------
// 契约错误语义：Err(String) 原样传播（内核泵 failed 收敛记因的消费前提）
// ---------------------------------------------------------------------------

#[test]
fn 假sink返回err时result语义原样传播不静默() {
    // 第 3 次调用（index 2 = append_sealed）失败
    let (fake, _calls) = FakeSink::failing_on(2);
    let sink: Arc<dyn SessionSink> = Arc::new(fake);
    sink.create_session(&new_session_row())
        .expect("create 应成功");
    sink.begin_turn("ses-1", 1).expect("begin 应成功");

    let result = sink.append_sealed("ses-1", &sealed_event(0));
    let error = result.expect_err("编程失败点必须 Err");
    assert_eq!(error, "存储失败: 调用 #2", "Err(String) 原样传播");
    // 失败后调用照常可达（Result 语义，不中断 trait 面使用）
    sink.append_sealed("ses-1", &sealed_event(1))
        .expect("后续调用应成功");
}

// ---------------------------------------------------------------------------
// Send + Sync 边界（内核泵跨 await 持有的前提）
// ---------------------------------------------------------------------------

#[test]
fn dyn_session_sink与dyn_session_query满足send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    assert_send_sync::<dyn SessionSink>();
    assert_send_sync::<dyn SessionQuery>();
    assert_send_sync::<TurnOutcome>();

    // Arc<dyn> 跨线程移动可用（内核持 sink 句柄跨 await 的形态）
    let (fake, _calls) = FakeSink::new();
    let sink: Arc<dyn SessionSink> = Arc::new(fake);
    let moved = std::thread::spawn(move || {
        sink.begin_turn("ses-cross", 1)
            .expect("跨线程 begin 应成功")
    })
    .join()
    .expect("线程正常结束");
    assert_eq!(moved, 1);
}
