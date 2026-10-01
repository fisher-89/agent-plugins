use serde::{Deserialize, Serialize};
use specta::Type;

/// UTC unix 毫秒：std 唯一时间源（时钟早于 epoch 时取 0，不 panic）。
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 消息内块：Text / Thinking / ToolUse / ToolResult 四变体（tag `kind`，camelCase）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentBlock {
    /// 文本块（assistant 正文 / user 输入）
    Text { text: String },
    /// 思考块
    Thinking { thinking: String },
    /// 工具调用块（`input` 为原 JSON，结构由具体工具自解释）
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// 工具结果块（`content` 已扁平化为字符串；`is_error` 缺失视作 false）
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
}

/// token 级增量载荷（思考/回复可辨；serde 内部 tag camelCase，与块模型同式）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentDelta {
    /// 文本增量
    Text { text: String },
    /// 思考增量
    Thinking { thinking: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentEventKind {
    /// run 启动（源自 system/init）：模型、会话、工具与 MCP 服务器清单
    RunStarted {
        model: Option<String>,
        session_id: Option<String>,
        tools: Vec<String>,
        mcp_servers: Vec<String>,
    },
    /// token 级增量（ephemeral）：只上传输面，store 永不见；配对键沿用
    /// `parent_tool_use_id` 词汇（子代理归因一处两用）
    MessageDelta {
        parent_tool_use_id: Option<String>,
        delta: AgentDelta,
    },
    /// 对话消息（源自 assistant / user）：块序列 + 子代理归因。密封层：唯一
    /// 落库与重放单元，密封粒度 = 引擎一次 assistant 回应（全部块收进）
    Message {
        role: String,
        blocks: Vec<AgentBlock>,
        parent_tool_use_id: Option<String>,
    },
    /// system 通知（permission_denial / api_retry 等，subtype 不做封闭枚举）
    SystemNotice {
        subtype: String,
        payload: serde_json::Value,
    },
    /// 轮收敛事件：唯一驱动状态机收敛的变体，字段面 = 统计唯一口径（由内核
    /// 收口写轮行，引擎无第二口径）
    TurnDone {
        subtype: String,
        is_error: bool,
        num_turns: Option<u64>,
        duration_ms: Option<u64>,
        cost_usd: Option<f64>,
        usage: serde_json::Value,
        session_id: Option<String>,
    },
    /// 未识别事件透传（未知 type / 非 JSON 行）：原文完整保留
    Raw {
        event_type: String,
        raw_json: String,
    },
}

impl AgentEventKind {
    /// 增量判别：仅 [`AgentEventKind::MessageDelta`] 为真（内核泵分类与
    /// store sink 防御共用——增量只上传输面，永不落库）。
    pub fn is_delta(&self) -> bool {
        matches!(self, Self::MessageDelta { .. })
    }

    /// 密封判别：增量以外的五变体为真（durable 词汇面，与 [`Self::is_delta`]
    /// 互补不重叠）。
    pub fn is_sealed(&self) -> bool {
        !self.is_delta()
    }
}

/// 统一事件信封：每事件携带 `seq`（单调序号，入库排序键）与时间戳；
/// `kind` 扁平进线格式（线格式 = 落库形态（密封） = 前端 DTO 基准）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    /// 单调序号，每轮从 0 递增；空白行跳过不占 seq。共享单调 seq 空间：
    /// 增量同样占号（盖戳治理单点、传输/落库两路 seq 可比对），库内重放为
    /// 密封事件 seq 升序、容忍空洞（排序键语义合法）
    pub seq: u64,
    /// 事件盖戳时刻（UTC unix 毫秒）
    pub timestamp_ms: i64,
    /// 事件类别（serde 内部 tag 扁平）
    #[serde(flatten)]
    pub kind: AgentEventKind,
}

impl AgentEvent {
    /// 盖当前时钟毫秒的统一生产入口：seq 由调用方（事件生产者）传入，
    /// 每 run 从 0 单调递增；时间戳取本函数调用时刻。
    pub fn stamp(seq: u64, kind: AgentEventKind) -> Self {
        Self {
            seq,
            timestamp_ms: now_millis(),
            kind,
        }
    }
}
