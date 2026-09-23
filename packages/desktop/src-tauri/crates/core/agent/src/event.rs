//! 事件信封：五变体逻辑事件模型 + 块模型 + 盖戳构造。纯类型，无 IO。
//!
//! 信封是三租户（CLI / SDK / API）共享的逻辑事件模型，非任何线上格式直译；
//! serde camelCase 线格式同时是落库形态与前端 DTO 镜像基准。
//! [`AgentEventKind::Raw`] 刻意承接未识别事件透传：永不丢事件、永不炸解析。

use serde::{Deserialize, Serialize};

/// UTC unix 毫秒：std 唯一时间源（时钟早于 epoch 时取 0，不 panic）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 消息内块：Text / Thinking / ToolUse / ToolResult 四变体（tag `kind`，camelCase）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// 事件类别五变体：`kind` 为 serde 内部 tag（camelCase 值），经
/// [`AgentEvent`] 扁平进线格式。字段表与归一化映射见能力 spec
/// `specs/desktop-agent-execution/spec.md`（路径相对域根）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// 对话消息（源自 assistant / user）：块序列 + 子代理归因
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
    /// run 收敛事件：唯一驱动状态机收敛的变体
    RunResult {
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

/// 统一事件信封：每事件携带 `seq`（单调序号，入库排序键）与时间戳；
/// `kind` 扁平进线格式（线格式 = 落库形态 = 前端 DTO 基准）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    /// 单调序号，每 run 从 0 递增；空白行跳过不占 seq
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
