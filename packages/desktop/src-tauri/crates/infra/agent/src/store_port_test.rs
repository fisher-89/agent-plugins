//! `store_port`（SessionSink / SessionQuery 的 store 实现）的单元测试（AC-2 /
//! AC-5 / AC-6）：write-through 原子操作（create/bind/轮生命周期/密封追加）、
//! delta 防御性忽略、Err 传播、转录重放（seq 升序空洞容忍 + 跨会话隔离）、
//! 清单聚合现算与对账重导。无 store mock：tempdir 真库 Store 装置，时间戳以
//! 显式 i64 实参注入。

use std::path::PathBuf;
use std::sync::Arc;

use agent::{
    AgentDelta, AgentEvent, AgentEventKind, NewSessionRow, SessionQuery, SessionSink,
    SessionStats, SessionProvenance,
};

use crate::store_port::{StoreQuery, StoreSink};

// ---------------------------------------------------------------------------
// 装置：tempdir 真库 + sink/query 装配
// ---------------------------------------------------------------------------

struct Env {
    _dir: tempfile::TempDir,
    store: Arc<store::Store>,
    sink: StoreSink,
    query: StoreQuery,
}

impl Env {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("store-port-test-{tag}-"))
            .tempdir()
            .expect("创建临时目录失败");
        let db_path = dir.path().join("test.redb");
        let store = Arc::new(
            store::Store::open_workspace(&db_path)
                .unwrap_or_else(|e| panic!("打开测试 workspace 库失败: {e}")),
        );
        let sink = StoreSink::new(Arc::clone(&store));
        let query = StoreQuery::new(Arc::clone(&store));
        Self {
            _dir: dir,
            store,
            sink,
            query,
        }
    }
}

/// sdk 快照的建行入参（store_record 反解 SessionConfigSnapshot 的合法形态）。
fn new_row(id: &str, source: &str, source_ref: Option<&str>) -> NewSessionRow {
    NewSessionRow {
        id: id.to_owned(),
        config_snapshot: serde_json::json!({
            "engine": "sdk",
            "model": "m-high",
            "permissionMode": "bypassPermissions"
        }),
        provenance: SessionProvenance {
            source: source.to_owned(),
            source_ref: source_ref.map(str::to_owned),
        },
    }
}

fn sealed(seq: u64, kind: AgentEventKind) -> AgentEvent {
    AgentEvent::stamp(seq, kind)
}

fn message_kind() -> AgentEventKind {
    AgentEventKind::Message {
        role: "assistant".to_owned(),
        blocks: Vec::new(),
        parent_tool_use_id: None,
    }
}

fn turn_done(seq: u64, input_tokens: u64, duration_ms: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::TurnDone {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(1),
            duration_ms: Some(duration_ms),
            cost_usd: None,
            usage: serde_json::json!({ "inputTokens": input_tokens, "outputTokens": 2 }),
            session_id: Some("sdk-1".to_owned()),
        },
    )
}

fn delta(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: None,
            delta: AgentDelta::Text {
                text: "增量".to_owned(),
            },
        },
    )
}

/// 一轮完整轮行（begin → finish）。
fn finish_turn_ok(sink: &StoreSink, turn_id: i64, status: agent::AgentRunStatus) {
    let outcome = agent::TurnOutcome {
        turn_id,
        status,
        finished_at: 1727000005000,
        num_turns: Some(3),
        duration_ms: Some(4000),
        cost_usd: Some(0.2),
        usage: serde_json::json!({ "inputTokens": 50 }),
        error: None,
        remote_session_id: None,
    };
    sink.finish_turn(turn_id, &outcome)
        .expect("finish_turn 应成功");
}

// ---------------------------------------------------------------------------
// SessionSink：create/bind 往返（双 id 映射落库半边——AC-5）
// ---------------------------------------------------------------------------

#[test]
fn create_session落库后bind_remote_session刷新双id与updated_at() {
    let env = Env::new("create-bind");

    env.sink
        .create_session(&new_row("ses-a", "debug", None))
        .expect("create_session 应成功");
    let before = env
        .store
        .find_session("ses-a")
        .expect("find_session 应成功")
        .expect("会话行在场");
    assert_eq!(before.engine_session_id, None, "建档时未上报");

    env.sink
        .bind_remote_session("ses-a", "sdk-11", 1727000009000)
        .expect("bind_remote_session 应成功");
    let after = env
        .store
        .find_session("ses-a")
        .expect("find_session 应成功")
        .expect("会话行在场");
    assert_eq!(
        after.engine_session_id.as_deref(),
        Some("sdk-11"),
        "remote 承接（直查双 id 映射落库半边）"
    );
    assert_eq!(
        after.updated_at, 1727000009000,
        "updated_at 刷新承接"
    );
    assert_eq!(after.created_at, before.created_at, "建档时间不变");
}

#[test]
fn bind_remote_none仅刷新时间戳且miss会话err传播() {
    let env = Env::new("bind-none");
    env.sink
        .create_session(&new_row("ses-b", "debug", None))
        .expect("create_session 应成功");

    // remote None：仅刷新 updated_at
    env.sink
        .bind_remote_session("ses-b", "", 1727000009001)
        .expect("bind 应成功");

    // 会话不存在：Err 传播
    let error = env
        .sink
        .bind_remote_session("ses-404", "sdk-x", 1)
        .expect_err("不存在会话 bind 必须 Err");
    assert!(!error.is_empty(), "Err 携记因：{error}");
}

// ---------------------------------------------------------------------------
// SessionSink：轮生命周期（begin 分配 → finish 终态整行替换）
// ---------------------------------------------------------------------------

#[test]
fn begin_turn分配轮id且finish_turn终态整行替换挂core会话() {
    let env = Env::new("turn-life");
    env.sink
        .create_session(&new_row("ses-t", "debug", None))
        .expect("create_session 应成功");

    // begin：running 初值行，轮 id 自增
    let first = env
        .sink
        .begin_turn("ses-t", 1727000000000)
        .expect("begin_turn 应成功");
    let second = env
        .sink
        .begin_turn("ses-t", 1727000001000)
        .expect("begin_turn 应成功");
    assert_eq!((first, second), (1, 2), "轮 id 自增分配");

    // finish：终态整行替换（status/finished_at/统计/error），session_id/started_at 沿用
    finish_turn_ok(&env.sink, first, agent::AgentRunStatus::Completed);

    let summaries = env
        .query
        .list_sessions(None, None)
        .expect("list_sessions 应成功");
    assert_eq!(summaries.len(), 1);
    let turns = &summaries[0].turns;
    assert_eq!(turns.len(), 2, "两轮行挂 core 会话随行返回");
    assert_eq!(turns[0].status, agent::AgentRunStatus::Completed);
    assert_eq!(turns[0].finished_at, Some(1727000005000));
    assert_eq!(turns[0].num_turns, Some(3));
    assert_eq!(turns[0].duration_ms, Some(4000));
    assert_eq!(turns[0].cost_usd, Some(0.2));
    assert_eq!(turns[0].session_id, "ses-t", "轮行挂 core 会话");
    assert_eq!(
        turns[1].status,
        agent::AgentRunStatus::Running,
        "未收轮保持 running 初值"
    );
    // started_at 沿用存量行（终态装配侧不携带）
    assert_eq!(turns[0].started_at, 1727000000000);
}

// ---------------------------------------------------------------------------
// SessionSink：append_sealed 密封追加与 delta 防御（AC-2 / AC-6）
// ---------------------------------------------------------------------------

#[test]
fn append_sealed密封追加后transcript按seq升序全量重放() {
    let env = Env::new("append");
    env.sink
        .create_session(&new_row("ses-s", "debug", None))
        .expect("create_session 应成功");

    for seq in [0u64, 2, 7] {
        env.sink
            .append_sealed("ses-s", &sealed(seq, message_kind()))
            .expect("append_sealed 应成功");
    }

    let transcript = env.query.transcript("ses-s").expect("transcript 应成功");
    let seqs: Vec<u64> = transcript.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 2, 7], "seq 升序全量重放（write-through 原子落盘）");
}

#[test]
fn append_sealed批次混入delta时返回ok且零记录产生() {
    let env = Env::new("delta-defense");
    env.sink
        .create_session(&new_row("ses-d", "debug", None))
        .expect("create_session 应成功");

    // 混入 delta 的批次：Ok 且 delta 零记录（store 中不存在 delta 记录——AC-2）
    env.sink
        .append_sealed("ses-d", &delta(1))
        .expect("delta 防御性忽略返回 Ok");
    env.sink
        .append_sealed("ses-d", &sealed(0, message_kind()))
        .expect("密封追加应成功");

    let transcript = env.query.transcript("ses-d").expect("transcript 应成功");
    assert_eq!(transcript.len(), 1, "store 中不存在 delta 记录");
    assert!(transcript[0].kind.is_sealed());
}

#[test]
fn append与begin不校验会话存在而bind与finish的err面传播() {
    let env = Env::new("err-propagate");

    // 转录单表 / 轮统计行无外键约束：append / begin 对未知会话不报错（孤儿行
    // 语义；Err 面收 bind 与 finish 两口——内核 failed 收敛记因的消费前提）
    env.sink
        .append_sealed("ses-ghost", &sealed(0, message_kind()))
        .expect("append 无外键不报错");
    env.sink
        .begin_turn("ses-ghost", 1)
        .expect("begin 无外键不报错");

    // bind：会话不存在 Err 传播
    let bind_error = env
        .sink
        .bind_remote_session("ses-404", "sdk-x", 1)
        .expect_err("不存在会话 bind 必须 Err");
    assert!(!bind_error.is_empty(), "bind Err 携记因: {bind_error}");

    // finish：轮行不存在 Err 传播
    env.sink
        .create_session(&new_row("ses-real", "debug", None))
        .expect("create_session 应成功");
    let turn = env.sink.begin_turn("ses-real", 1).expect("begin 应成功");
    let outcome = agent::TurnOutcome {
        turn_id: turn + 100, // 不存在的轮 id
        status: agent::AgentRunStatus::Completed,
        finished_at: 1,
        num_turns: None,
        duration_ms: None,
        cost_usd: None,
        usage: serde_json::Value::Null,
        error: None,
        remote_session_id: None,
    };
    assert!(
        env.sink.finish_turn(turn + 100, &outcome).is_err(),
        "不存在轮行 finish 必须 Err"
    );
}

// ---------------------------------------------------------------------------
// SessionQuery：transcript 重放（AC-2 查询半边）
// ---------------------------------------------------------------------------

#[test]
fn transcript跨多轮重放seq升序且空洞容忍不破坏有序() {
    let env = Env::new("transcript-holes");
    env.sink
        .create_session(&new_row("ses-h", "debug", None))
        .expect("create_session 应成功");
    // 轮 1：delta 占 seq 1、密封占 0/2；轮 2：密封占 3/5（4 为 delta 占位）
    let _ = env.sink.begin_turn("ses-h", 1).expect("begin 应成功");
    env.sink
        .append_sealed("ses-h", &sealed(0, message_kind()))
        .expect("append 应成功");
    env.sink
        .append_sealed("ses-h", &turn_done(2, 10, 100))
        .expect("append 应成功");
    let _ = env.sink.begin_turn("ses-h", 200).expect("begin 应成功");
    env.sink
        .append_sealed("ses-h", &sealed(3, message_kind()))
        .expect("append 应成功");
    env.sink
        .append_sealed("ses-h", &turn_done(5, 20, 200))
        .expect("append 应成功");

    let transcript = env.query.transcript("ses-h").expect("transcript 应成功");
    let seqs: Vec<u64> = transcript.iter().map(|event| event.seq).collect();
    assert_eq!(
        seqs,
        vec![0, 2, 3, 5],
        "密封 seq 空洞（delta 占 1/4）容忍不破坏有序（不要求运行进程存活）"
    );
    // 全史含两轮 TurnDone（会话全史 = 重放与 resume 重建唯一来源）
    assert_eq!(
        transcript
            .iter()
            .filter(|event| matches!(event.kind, AgentEventKind::TurnDone { .. }))
            .count(),
        2
    );
}

#[test]
fn transcript不存在会话返回空vec且跨session隔离() {
    let env = Env::new("isolation");
    env.sink
        .create_session(&new_row("ses-a", "debug", None))
        .expect("create_session 应成功");
    env.sink
        .create_session(&new_row("ses-b", "debug", None))
        .expect("create_session 应成功");
    env.sink
        .append_sealed("ses-a", &sealed(0, message_kind()))
        .expect("append 应成功");

    assert!(
        env.query
            .transcript("ses-404")
            .expect("不存在会话空 Vec")
            .is_empty(),
        "不存在会话返回空 Vec"
    );
    assert!(
        env.query.transcript("ses-b").expect("transcript 应成功").is_empty(),
        "B 会话转录不含 A 的事件"
    );
    assert_eq!(env.query.transcript("ses-a").expect("非空").len(), 1);
}

// ---------------------------------------------------------------------------
// SessionQuery：list_sessions 聚合现算（AC-2 清单半边 / AC-6 降级不违约）
// ---------------------------------------------------------------------------

#[test]
fn list_sessions聚合现算轮数累计墙钟累计token与降序稳定() {
    let env = Env::new("aggregate");
    // 两个会话：ses-new（updated_at 大）与 ses-old
    env.sink
        .create_session(&new_row("ses-new", "debug", None))
        .expect("create_session 应成功");
    env.sink
        .create_session(&new_row("ses-old", "debug", None))
        .expect("create_session 应成功");

    // ses-new：两轮，TurnDone 带时长与 token（usage 鸭子类型求和口径）
    env.sink
        .append_sealed("ses-new", &turn_done(0, 10, 100))
        .expect("append 应成功");
    let turn = env.sink.begin_turn("ses-new", 1727000000000).expect("begin");
    finish_turn_ok(&env.sink, turn, agent::AgentRunStatus::Completed);
    env.sink
        .bind_remote_session("ses-new", "sdk-1", 1727000009000)
        .expect("bind 应成功");

    // ses-old：updated_at 较小
    env.sink
        .bind_remote_session("ses-old", "sdk-0", 1727000001000)
        .expect("bind 应成功");

    let summaries = env
        .query
        .list_sessions(None, None)
        .expect("list_sessions 应成功");
    assert_eq!(summaries.len(), 2);
    assert_eq!(
        summaries[0].row.id, "ses-new",
        "updated_at 降序（并列按 id 降序稳定）"
    );

    let stats = &summaries[0].stats;
    assert_eq!(stats.turn_count, 1, "轮数 = 轮统计行行数");
    assert_eq!(
        stats.total_duration_ms,
        Some(4000),
        "累计墙钟 = 轮行时长求和（轮统计行口径）"
    );
    assert_eq!(
        stats.input_tokens,
        Some(10),
        "累计 token 自转录 TurnDone usage 鸭子类型求和（camelCase 键）"
    );
    assert_eq!(summaries[0].turns.len(), 1, "轮统计行随行返回");
}

#[test]
fn list_sessions来源过滤生效且缺席缺省空库空清单() {
    let env = Env::new("source-filter");
    env.sink
        .create_session(&new_row("ses-debug", "debug", None))
        .expect("create_session 应成功");
    env.sink
        .create_session(&new_row("ses-explore", "explore", Some("42")))
        .expect("create_session 应成功");

    // source 过滤
    let explores = env
        .query
        .list_sessions(Some("explore"), None)
        .expect("list_sessions 应成功");
    assert_eq!(
        explores.iter().map(|s| s.row.id.as_str()).collect::<Vec<_>>(),
        vec!["ses-explore"],
        "source 过滤生效"
    );
    // source_ref 过滤
    let ref42 = env
        .query
        .list_sessions(Some("explore"), Some("42"))
        .expect("list_sessions 应成功");
    assert_eq!(ref42.len(), 1);
    let ref99 = env
        .query
        .list_sessions(Some("explore"), Some("99"))
        .expect("list_sessions 应成功");
    assert!(ref99.is_empty(), "source_ref 不匹配过滤为空");

    // 无轮行会话：统计缺省（降级不违约）
    let summaries = env
        .query
        .list_sessions(None, None)
        .expect("list_sessions 应成功");
    for summary in &summaries {
        assert_eq!(summary.stats.turn_count, 0);
        assert_eq!(summary.stats.total_duration_ms, None);
        assert_eq!(summary.stats.input_tokens, None);
        assert_eq!(summary.stats.output_tokens, None);
        assert!(summary.turns.is_empty());
    }

    // 空库空清单
    let empty_env = Env::new("empty-list");
    assert!(
        empty_env
            .query
            .list_sessions(None, None)
            .expect("空库清单应成功")
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// SessionQuery：reconcile_stats 对账重导（AC-6 对账半边）
// ---------------------------------------------------------------------------

#[test]
fn reconcile_stats从转录重算校正篡改的轮行统计且转录逐字节不变() {
    let env = Env::new("reconcile");
    env.sink
        .create_session(&new_row("ses-r", "debug", None))
        .expect("create_session 应成功");
    // 转录：两轮 TurnDone（token 10 + 20、时长 100 + 200）
    env.sink
        .append_sealed("ses-r", &turn_done(0, 10, 100))
        .expect("append 应成功");
    env.sink
        .append_sealed("ses-r", &turn_done(1, 20, 200))
        .expect("append 应成功");
    let turn = env.sink.begin_turn("ses-r", 1).expect("begin 应成功");
    finish_turn_ok(&env.sink, turn, agent::AgentRunStatus::Completed);

    // 篡改轮统计行（直写 store：finish 成假数据）
    let tampered = store::AgentRunRecord {
        id: turn,
        session_id: Some("ses-r".to_owned()),
        status: agent::AgentRunStatus::Completed,
        started_at: 1,
        finished_at: Some(2),
        num_turns: Some(99),
        cost_usd: Some(9.9),
        duration_ms: Some(99_999),
        error: None,
    };
    env.store
        .finish_agent_turn(turn, &tampered)
        .expect("篡改轮行");

    // 重导：从转录 TurnDone 重算校正
    let stats = env
        .query
        .reconcile_stats("ses-r")
        .expect("reconcile_stats 应成功");
    assert_eq!(stats.turn_count, 2, "轮数自转录重算（非轮行行数）");
    assert_eq!(stats.total_duration_ms, Some(300), "累计墙钟校正");
    assert_eq!(stats.input_tokens, Some(30), "累计 token 校正");
    assert_eq!(stats.output_tokens, Some(4));

    // 不改转录纪律：密封转录逐字节不变
    let transcript = env.query.transcript("ses-r").expect("transcript 应成功");
    assert_eq!(transcript.len(), 2);
    assert_eq!(
        transcript.iter().map(|event| event.seq).collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn reconcile_stats无turn_done转录时缺省统计不报错() {
    let env = Env::new("reconcile-empty");
    env.sink
        .create_session(&new_row("ses-re", "debug", None))
        .expect("create_session 应成功");
    env.sink
        .append_sealed("ses-re", &sealed(0, message_kind()))
        .expect("append 应成功");

    let stats: SessionStats = env
        .query
        .reconcile_stats("ses-re")
        .expect("无 TurnDone 不报错");
    assert_eq!(stats.turn_count, 0, "缺省统计");
    assert_eq!(stats.total_duration_ms, None);
    assert_eq!(stats.input_tokens, None);
    assert_eq!(stats.output_tokens, None);
}

// ---------------------------------------------------------------------------
// 查询面构造：session_query 单点（crate 根可达形态）
// ---------------------------------------------------------------------------

#[test]
fn session_query构造返回core查询契约实现() {
    let env = Env::new("session-query-ctor");
    let query: Arc<dyn SessionQuery> = crate::session_query(Arc::clone(&env.store));
    let summaries = query
        .list_sessions(None, None)
        .expect("经 trait object 查询应成功");
    assert!(summaries.is_empty(), "空库空清单");
}

/// 路径锚定：store 定型快照类型在 port 层反解（快照非法 fail fast 的对照形态）。
#[test]
fn create_session快照定型反解非法json显式失败() {
    let env = Env::new("bad-snapshot");
    let bad = NewSessionRow {
        id: "ses-bad".to_owned(),
        config_snapshot: serde_json::json!({ "engine": 123 }), // engine 非字符串
        provenance: SessionProvenance {
            source: "debug".to_owned(),
            source_ref: None,
        },
    };
    let error = env
        .sink
        .create_session(&bad)
        .expect_err("非法快照必须显式失败（fail fast，不静默降级）");
    assert!(
        error.contains("配置快照非法"),
        "快照反解失败记因，实际: {error}"
    );
    // 目录路径仅用于装置（无额外文件边界）
    let _ = PathBuf::from("unused");
}
