//! 内核持久 / 查询 port 的 store 适配：core 契约（[`agent::SessionSink`] /
//! [`agent::SessionQuery`]）的 infra 实现落点。
//!
//! write-through 原子操作、聚合现算与对账重导全部委托 store 操作面（store
//! 单点），本模块只做 core 契约类型 ↔ store 记录类型的映射；增量防御由
//! store sink 兜底（纵深，内核泵已分类）。

use std::sync::Arc;

use agent::{
    AgentEvent, NewSessionRow, SessionQuery, SessionSink, SessionStats, SessionSummary, TurnOutcome,
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
    ) -> Result<Vec<SessionSummary>, String> {
        self.store.list_sessions(source, source_ref)
    }

    /// 单查两段式：`find_session` 主键直查锚定存在性（miss → 显式 `Err`，不
    /// 触全表）→ 清单面收敛取单条聚合形状（轮行只经 `list_sessions` 可达，
    /// store 无单会话轮行读面）。第二段未命中为类型完备性兜底（会话无删除
    /// 路径），同词 `Err`。
    fn find_session_detail(&self, session_id: &str) -> Result<SessionSummary, String> {
        if self
            .store
            .find_session(session_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("会话不存在: id={session_id}"));
        }
        self.store
            .list_sessions(None, None)?
            .into_iter()
            .find(|summary| summary.row.id == session_id)
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
