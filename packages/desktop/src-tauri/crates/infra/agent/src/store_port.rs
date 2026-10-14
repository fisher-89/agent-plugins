use std::sync::Arc;

use agent::{
    AgentEvent, NewSessionRow, SessionPage, SessionQuery, SessionSink, SessionStats, SessionSummary,
    TurnOutcome,
};
use store::{SessionConfigSnapshot, SessionRecord, Store};

/// store 适配的持久面（write-through）：持有 workspace 库实例。
pub(crate) struct StoreSink {
    store: Arc<Store>,
}

impl StoreSink {
    /// 由 workspace 库实例构造。
    pub(crate) fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

impl SessionSink for StoreSink {
    fn create_session(&self, session: &NewSessionRow) -> Result<(), String> {
        let record = store_record(session)?;
        self.store
            .create_session(&record)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn begin_turn(&self, session_id: &str, started_at: i64) -> Result<i64, String> {
        self.store
            .begin_agent_turn(session_id, started_at)
            .map(|record| record.id)
            .map_err(|e| e.to_string())
    }

    fn next_seq(&self, session_id: &str) -> Result<u64, String> {
        self.store
            .next_session_seq(session_id)
            .map_err(|e| e.to_string())
    }

    fn append_sealed(&self, session_id: &str, event: &AgentEvent) -> Result<(), String> {
        self.store
            .append_session_events(session_id, std::slice::from_ref(event))
            .map_err(|e| e.to_string())
    }

    fn finish_turn(&self, turn_id: i64, outcome: &TurnOutcome) -> Result<(), String> {
        // 终态字段整组替换（session_id / started_at 由 store 沿用存量行，
        // 此处 shell 记录只承载收口字段）
        let record = store::AgentRunRecord {
            id: turn_id,
            session_id: None,
            status: outcome.status,
            started_at: 0,
            finished_at: Some(outcome.finished_at),
            num_turns: outcome.num_turns,
            cost_usd: outcome.cost_usd,
            duration_ms: outcome.duration_ms,
            error: outcome.error.clone(),
        };
        self.store
            .finish_agent_turn(turn_id, &record)
            .map_err(|e| e.to_string())
    }

    fn bind_remote_session(
        &self,
        session_id: &str,
        remote: &str,
        updated_at: i64,
    ) -> Result<(), String> {
        self.store
            .bind_session_remote(session_id, Some(remote), updated_at)
            .map_err(|e| e.to_string())
    }
}

/// store 适配的查询面：清单聚合 / 转录重放 / 对账重导（聚合与重导语义在
/// store 单点，此处直通）。
pub(crate) struct StoreQuery {
    store: Arc<Store>,
}

impl StoreQuery {
    /// 由 workspace 库实例构造。
    pub(crate) fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    /// workspace 库实例（组合根 Continue 校验消费）。
    pub(crate) fn store(&self) -> &Store {
        &self.store
    }
}

/// 会话域查询面构造（core 查询契约的 store 实现单点）：查询命令直查与内核
/// 组合根装配共用底座。
pub fn session_query(store: Arc<Store>) -> Arc<dyn SessionQuery> {
    Arc::new(StoreQuery::new(store))
}

impl SessionQuery for StoreQuery {
    fn list_sessions(
        &self,
        source: Option<&str>,
        source_ref: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<SessionPage, String> {
        self.store
            .list_sessions_page(source, source_ref, offset, limit)
    }

    /// 单查单点：store 单会话装配面（主键直查 + 本会话轮行 + 本会话事件 →
    /// 聚合组装）直取，miss 映射显式 `Err`（单查语义与清单空态区分）。
    fn find_session_detail(&self, session_id: &str) -> Result<SessionSummary, String> {
        self.store
            .find_session_summary(session_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("会话不存在: id={session_id}"))
    }

    fn transcript(&self, session_id: &str) -> Result<Vec<AgentEvent>, String> {
        self.store.list_session_events(session_id)
    }

    fn reconcile_stats(&self, session_id: &str) -> Result<SessionStats, String> {
        self.store.reconcile_session_stats(session_id)
    }
}

/// core 建行入参 → store 会话记录（建档即双侧同刻；快照自 JSON 形态回收为
/// store 定型三面，非组合根产出的快照在此显式失败——fail fast，不静默降级）。
fn store_record(session: &NewSessionRow) -> Result<SessionRecord, String> {
    let snapshot: SessionConfigSnapshot = serde_json::from_value(session.config_snapshot.clone())
        .map_err(|e| format!("配置快照非法: {e}"))?;
    let now = now_millis();
    Ok(SessionRecord {
        id: session.id.clone(),
        engine_session_id: None,
        config_snapshot: snapshot,
        source: session.provenance.source.clone(),
        source_ref: session.provenance.source_ref.clone(),
        created_at: now,
        updated_at: now,
    })
}

/// UTC unix 毫秒：std 唯一时间源（时钟早于 epoch 时取 0，不 panic）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "store_port_test.rs"]
mod store_port_test;
