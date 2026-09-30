//! sdk 引擎连接配置：三件套形态冻结的结构体 + MVP 硬编码预留位构造。
//!
//! `EngineConfig` 是门面 [`crate::EngineFacade::runner_for`] 的第二参、sdk
//! 引擎组装 rig client 的唯一来源。字段集（api_key / base_url / model）与
//! 类型（三 `String`）为消费面契约：后续迭代换构造源（配置落盘等）时本
//! 结构体与 [`EngineConfig::from_hardcoded_slot`] 的消费方零改动。

/// sdk 引擎连接配置三件套：认证凭据、openai 兼容端点 base_url、模型标识。
///
/// 不进 serde / specta 线面（机密不入库、不出 IPC 面）；不派生 `Debug`
/// 全量输出（防凭据进日志，测试比较用 [`PartialEq`]）。
#[derive(Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// 认证凭据（bearer token）
    pub api_key: String,
    /// openai 兼容端点 base_url（含版本段，如 `https://…/v1`）
    pub base_url: String,
    /// 模型标识
    pub model: String,
}

impl EngineConfig {
    /// MVP 硬编码预留位构造：空缺省值。用户手填真机验证后 sdk 引擎方可
    /// 启动——未手填时启动以 [`agent::AgentStartError::ConfigMissing`]
    /// 显式失败（run 记录不产生，`Err` 抵达前端）。
    ///
    /// 机密 MUST NOT 入库（openspec/config.json 为 git 追踪文件）；配置
    /// 落盘座位（入配置文件 vs app 侧键控）为后续迭代待决问题——届时仅
    /// 替换本构造体内部实现，消费面零改动。
    pub fn from_hardcoded_slot() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
            model: String::new(),
        }
    }

    /// 三字段齐备检查（runner 启动校验用）：任一为空即配置缺失。
    pub fn is_complete(&self) -> bool {
        !self.api_key.is_empty() && !self.base_url.is_empty() && !self.model.is_empty()
    }
}
