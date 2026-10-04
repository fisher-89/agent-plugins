use serde::{Deserialize, Serialize};

use crate::event::AgentEvent;
use crate::runner::AgentRunStatus;
use crate::session::{NewSessionRow, SessionStats, SessionSummary};

/// 轮收口中性数据：store 轮行落库与 IPC 终态装配共用的唯一收口形状。统计
/// 字段面与 TurnDone 事件一致（唯一口径）；`usage` 为 TurnDone 原值透传
/// （转录已含密封 TurnDone 事件，此处仅随行携带，不另立落库形态）。serde
/// camelCase 线格式（收口形状的镜像面，与事件信封同口径）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnOutcome {
    /// 轮 id（sink begin_turn 分配）
    pub turn_id: i64,
    /// 收敛状态
    pub status: AgentRunStatus,
    /// 收敛时刻（UTC unix 毫秒）
    pub finished_at: i64,
    /// 收敛轮数（TurnDone 字段面；缺席合法）
    pub num_turns: Option<u64>,
    /// 运行时长毫秒（TurnDone 字段面；缺席合法）
    pub duration_ms: Option<u64>,
    /// 总成本美元（TurnDone 字段面；缺席合法）
    pub cost_usd: Option<f64>,
    /// 用量原值（TurnDone usage 透传；缺席合法）
    pub usage: serde_json::Value,
    /// 失败原因（failed 收敛记因；其余为 None）
    pub error: Option<String>,
    /// 引擎侧会话标识（TurnDone 上报的双 id 映射半边；缺席合法）
    pub remote_session_id: Option<String>,
}

/// write-through 持久面（core 定契约、infra 实现）：内核收到密封事件 / 轮
/// 收口即经本契约原子落盘；增量（`MessageDelta`）永不见于本契约——实现方
/// 仍应防御性忽略（纵深，与内核泵分类互补）。
pub trait SessionSink: Send + Sync {
    /// 建会话行（id 来自 core 铸造；时间戳由实现侧取值）。
    fn create_session(&self, session: &NewSessionRow) -> Result<(), String>;

    /// 开轮：写事务内分配轮 id 并落 running 初值行，返回轮 id。
    fn begin_turn(&self, session_id: &str, started_at: i64) -> Result<i64, String>;

    /// 密封事件追加（转录单表，write-through 单点）。增量防御性忽略不产生
    /// 记录。
    fn append_sealed(&self, session_id: &str, event: &AgentEvent) -> Result<(), String>;

    /// 轮收尾：status / finished_at / 统计 / error 落轮行。
    fn finish_turn(&self, turn_id: i64, outcome: &TurnOutcome) -> Result<(), String>;

    /// 双 id 映射落库半边 + `updated_at` 刷新（引擎侧标识上报时调用）。
    fn bind_remote_session(
        &self,
        session_id: &str,
        remote: &str,
        updated_at: i64,
    ) -> Result<(), String>;
}

/// 查询契约（core 定接口、infra 实现）：会话清单聚合 / 转录重放 / 对账重导。
/// 查询面不要求运行进程存活；统计与附带字段缺席合法缺省（降级不违约）。
pub trait SessionQuery: Send + Sync {
    /// 会话清单（来源过滤；`updated_at` 降序稳定序）+ 聚合统计现算 + 轮行
    /// 随行返回。
    fn list_sessions(
        &self,
        source: Option<&str>,
        source_ref: Option<&str>,
    ) -> Result<Vec<SessionSummary>, String>;

    /// 按 session id 单查会话（行 + 聚合统计 + 轮行，复用
    /// [`SessionSummary`] 聚合形状，零新 DTO）。查无此 id 显式 `Err`（单查
    /// 语义与清单空态区分，查无此 id 视为调用方错误）；运行状态自轮行推导
    ///（存在 running 轮行即 running，否则取终态）。
    fn find_session_detail(&self, session_id: &str) -> Result<SessionSummary, String>;

    /// 会话全史转录重放（密封事件 seq 升序、容忍库内空洞）。
    fn transcript(&self, session_id: &str) -> Result<Vec<AgentEvent>, String>;

    /// 对账纠偏重导（显式入口）：从转录重算聚合，不改密封转录、不隐式挂
    /// 读路径。
    fn reconcile_stats(&self, session_id: &str) -> Result<SessionStats, String>;
}
