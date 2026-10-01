use serde::{Deserialize, Serialize};
use specta::Type;

use crate::runner::AgentRunStatus;

/// 会话 id 进程内原子计数（配合毫秒时戳保证同毫秒不重号）。
static SESSION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 铸造新会话 id：`ses-<进程内计数>-<毫秒时戳>`（core 铸造单点；id 即会话
/// 寻址与落库主键，进程内全局唯一免复合键）。
pub fn new_session_id() -> String {
    let counter = SESSION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let millis = crate::event::now_millis();
    format!("ses-{counter}-{millis}")
}

/// 来源归属：来源受控字符串（debug | explore | …，缺省 debug）+ 来源内定位
/// （explore 指向探索记录主键的十进制串）。会话级来源，消费方经此寻址。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionProvenance {
    /// 来源受控字符串
    pub source: String,
    /// 来源内定位（调试会话为 None）
    pub source_ref: Option<String>,
}

/// 会话记录契约：id 为 core 铸造 id（字符串主键）；`remote_session_id` 为
/// 引擎侧句柄（双 id 映射的内存半边，落库半边归 store）；`config_snapshot`
/// 为装配配置快照的 JSON 形态（快照不透明——core 不解释装配概念，infra 侧
/// 定型字段面），存快照而非跨库引用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    /// core 铸会话 id（主键）
    pub id: String,
    /// 引擎侧句柄（cli / sdk 各自上报的会话标识；未上报为 None）
    pub remote_session_id: Option<String>,
    /// 装配配置快照（JSON 形态，core 不解释）
    pub config_snapshot: serde_json::Value,
    /// 来源归属
    pub provenance: SessionProvenance,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒）
    pub updated_at: i64,
}

/// 建行入参形态：id 由内核铸造后传入；时间戳由 sink 落库侧取值（core 契约
/// 不携带时钟语义）。
#[derive(Debug, Clone, PartialEq)]
pub struct NewSessionRow {
    /// core 铸会话 id
    pub id: String,
    /// 装配配置快照（JSON 形态）
    pub config_snapshot: serde_json::Value,
    /// 来源归属
    pub provenance: SessionProvenance,
}

/// 轮行 DTO（IPC 面）：轮统计行（每轮一行）的契约形态，取代旧 run 记录的
/// IPC 面。`agent_start` 提前 resolve 返回 running 态行、终态经 Channel 以
/// 同构部件流出、重放按轮序对齐交错——实时与重放两路同构。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TurnSummary {
    /// 轮 id（workspace 库域内自增）
    pub turn_id: i64,
    /// 所属会话 id
    pub session_id: String,
    /// 轮状态（running | completed | failed | stopped）
    pub status: AgentRunStatus,
    /// 开始时间（UTC unix 毫秒）
    pub started_at: i64,
    /// 结束时间；运行中为 None
    pub finished_at: Option<i64>,
    /// 收敛轮数（来自 TurnDone 事件）
    pub num_turns: Option<u64>,
    /// 总成本美元（来自 TurnDone 事件；缺席合法缺省）
    pub cost_usd: Option<f64>,
    /// 运行时长毫秒（来自 TurnDone 事件；缺席合法缺省）
    pub duration_ms: Option<u64>,
    /// 失败原因（落库失败收敛 / 无 TurnDone 异常终止时填因）
    pub error: Option<String>,
}

/// 聚合统计：MVP 从轮行 / 转录现算，不维护累计列；统计与查询附带字段缺席
/// 合法缺省（降级不违约）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    /// 轮数（轮统计行行数）
    pub turn_count: u64,
    /// 累计墙钟毫秒（轮行时长求和；全部缺席为 None）
    pub total_duration_ms: Option<u64>,
    /// 累计输入 token（TurnDone usage 鸭子类型求和；缺席合法缺省）
    pub input_tokens: Option<u64>,
    /// 累计输出 token（TurnDone usage 鸭子类型求和；缺席合法缺省）
    pub output_tokens: Option<u64>,
}

/// 会话清单项：会话记录 + 现算聚合统计 + 轮行随行返回（重放部件与历史展示
/// 共用；core 契约类型，不引用 store 模型）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    /// 会话记录
    pub row: SessionRow,
    /// 聚合统计
    pub stats: SessionStats,
    /// 轮统计行（发起顺序）
    pub turns: Vec<TurnSummary>,
}
