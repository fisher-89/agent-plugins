//! 门面连接配置：协议 + 凭据 + 端点 + 双档模型（连接必需项）。
//!
//! 可空调参 `context_length` / `max_tokens` MUST NOT 进 [`EngineConfig`]——
//! 它们经门面旁路 builder（`EngineFacade::with_context_window` /
//! `with_max_tokens`）抵达引擎。

/// 模型协议（门面词汇，映射自 store `AgentProviderProtocol`；两臂 match 于
/// `sdk/runner::build_model`）。**刻意不派生 serde / specta**：引擎内部词汇，
/// 不出 crate、不进 IPC bindings（与 `EngineKind` 的区别：后者是 `agent_start`
/// 参数值域）。无凭据，可派生 `Debug`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Protocol {
    /// openai-compatible 端点（缺省）
    #[default]
    Openai,
    /// Anthropic Messages 端点及其兼容网关
    Anthropic,
}

impl Protocol {
    /// 日志 / 诊断面线值（`protocol=openai|anthropic`）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
        }
    }
}

/// 连接配置字段面（协议 + 凭据 + 端点 + 双档模型）：`protocol` 置首——协议
/// 决定 `base_url` 语义与 wire。刻意不派生 `Debug`（凭据不入日志）。
#[derive(Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// 模型协议（`base_url` 语义与请求参数由此决定）
    pub protocol: Protocol,
    /// 认证凭据（bearer token / `x-api-key`，由 rig 按协议内建）
    pub api_key: String,
    /// 协议对应端点 base_url（openai 含版本段，如 `https://…/v1`；anthropic
    /// 的 `/v1` / `/messages` 后缀由 rig 归一化）
    pub base_url: String,
    /// 模型标识（管理数据解析取 provider 三档 model 的 high/low 档；可带
    /// `[1m]` 后缀，解析归引擎侧单点）
    pub model_high: String,
    pub model_low: String,
}

impl EngineConfig {
    pub fn empty() -> Self {
        Self {
            protocol: Protocol::Openai,
            api_key: String::new(),
            base_url: String::new(),
            model_high: String::new(),
            model_low: String::new(),
        }
    }

    /// 连接配置字段面齐备检查（runner 启动校验用）：协议恒有值，不参与校验。
    pub fn is_complete(&self) -> bool {
        !self.api_key.is_empty()
            && !self.base_url.is_empty()
            && !self.model_high.is_empty()
            && !self.model_low.is_empty()
    }
}
