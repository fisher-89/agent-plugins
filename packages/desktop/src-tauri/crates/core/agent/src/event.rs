use serde::{Deserialize, Serialize};
use specta::Type;

/// UTC unix 毫秒：std 唯一时间源（时钟早于 epoch 时取 0，不 panic）。
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 消息内块：Text / Thinking / ToolUse / ToolResult / WebSearch 五变体
/// （tag `kind`，camelCase）。
///
/// 词汇仍**协议中立**（core 不认识 anthropic / rig）：`WebSearch` 是 provider
/// 服务端工具块的通用表达，`citations` 是其正文引用——
/// - `raw_use` / `raw_result` 为**回放唯一事实源**（`encrypted_content` 等不可
///   推导字段原样保全，展示字段 MUST NOT 参与重建）；
/// - `citations` 与 `WebSearch` 均为新变体 / 带默认值字段，事件 payload 是自
///   描述 JSON，落库**无需 store schema 版本升级**；旧事件缺 `citations` 读出
///   `None`。
///
/// 已知代价（边界条 15）：旧客户端读到含 `WebSearch` 块的事件会在**整条事件
/// 解码**时失败（不是少显示一张卡），仓库不提供降级分支。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentBlock {
    /// 文本块（assistant 正文 / user 输入）：`citations` 为正文引用
    /// （`None` = 无引用；**序列化落 `null`，键恒在**——旧 JSON 缺键仍读出
    /// `None`）。属性面只标 `default`：`skip_serializing_if` 无法在 unified
    /// 相位模型下表达，加则 bindings 分裂为相位伴生类型。
    Text {
        text: String,
        #[serde(default)]
        citations: Option<Vec<AgentCitation>>,
    },
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
    /// 云端 `web_search` 服务端工具块：一次 `server_tool_use` 与其配对 result
    /// 的**原子表达**（`id` 同两侧配对键）。
    ///
    /// `query` / `results` / `error` 为展示字段（不参与回放重建）；
    /// `raw_use` / `raw_result` 为回放唯一事实源。use 与 result 被拆进两条
    /// assistant 消息时投两块（use 侧 `raw_use` 有值、result 侧为 `None`）。
    ///
    /// 展示字段与 raw 字段**一律落键、值可 `null`**（`results` 缺省落 `[]`）：
    /// 属性面只标 `default`，`skip_serializing_if` 会令 bindings 分裂出相位
    /// 伴生类型（统一形态的前提，见 D1）。
    WebSearch {
        /// 服务端工具调用 id（= 配对 result 的 `tool_use_id`）
        id: String,
        /// 查询词（`server_tool_use.input.query`；孤儿 result 块为空串）
        query: String,
        /// 来源列表（展示字段；空数组落 `[]`，键恒在）
        #[serde(default)]
        results: Vec<WebSearchSource>,
        /// 搜索错误码（`web_search_tool_result_error.error_code`）
        #[serde(default)]
        error: Option<String>,
        /// 原始 wire 值（`server_tool_use` 原文；回放源）
        #[serde(default)]
        raw_use: Option<serde_json::Value>,
        /// 原始 wire 值（`web_search_tool_result` 原文；回放源）
        #[serde(default)]
        raw_result: Option<serde_json::Value>,
    },
}

/// 正文引用（服务端 citations 的一条）：`url` / `title` / `cited_text` 为展示
/// 字段（可缺省）；`raw` 为原始 wire 值（回放用，反序列化再序列化会规范化，
/// 故 MUST 原值保全）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentCitation {
    /// 来源地址
    #[serde(default)]
    pub url: Option<String>,
    /// 来源标题
    #[serde(default)]
    pub title: Option<String>,
    /// 被引用的原文片段
    #[serde(default)]
    pub cited_text: Option<String>,
    /// 原始 wire 值（回放源；不经类型往返）
    pub raw: serde_json::Value,
}

/// 单条搜索来源（纯展示字段，不参与回放）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchSource {
    /// 来源地址
    pub url: String,
    /// 来源标题
    #[serde(default)]
    pub title: Option<String>,
    /// 页面时效（服务端给出的 `page_age` 原样）
    #[serde(default)]
    pub page_age: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentMessageRole {
    /// 用户言语
    User,
    /// 模型回应
    Assistant,
    /// 引擎工具结果
    Tool,
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
    /// 对话消息（user 言语 / assistant 回应 / tool 结果管道）
    Message {
        role: AgentMessageRole,
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
    /// 单调序号，会话级续排
    pub seq: u64,
    /// 事件盖戳时刻（UTC unix 毫秒）
    pub timestamp_ms: i64,
    /// 事件类别（serde 内部 tag 扁平）
    #[serde(flatten)]
    pub kind: AgentEventKind,
}

impl AgentEvent {
    /// 盖当前时钟毫秒的统一生产入口：seq 由调用方（事件生产者）传入
    pub fn stamp(seq: u64, kind: AgentEventKind) -> Self {
        Self {
            seq,
            timestamp_ms: now_millis(),
            kind,
        }
    }
}
