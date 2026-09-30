//! `sdk` 引擎：进程内直连 openai 兼容端点的 agent 租户（不依赖本机 claude
//! CLI，开箱即跑）。
//!
//! rig-core 只作 provider client（openai chat completions + 自定义 base_url）
//! 与消息/工具类型层；多轮 agent loop 手搓自控（`AgentEvent` 信封、policy
//! 插值、`RunResult` 填充语义不交 rig 托管）。模块分层：
//! - [`config`]：连接配置三件套（MVP 硬编码预留位构造）
//! - [`policy`]：静态权限三档纯决策表
//! - [`sandbox`]：路径沙箱纯函数（canonicalize + workspace root 前缀校验）
//! - [`tools`]：六工具面（read / grep / glob / ls / write / edit）的定义
//!   与执行体；bash 不进 MVP 工具面
//! - [`normalize`]：rig 归一化流项 → [`agent::AgentEventKind`] 纯函数
//! - [`resume`]：store 转录 → rig 对话历史重建纯函数（`sdk-` 前缀归属校验）
//! - [`loop`]：多轮 agent loop（流式 → 事件 → policy/sandbox 检查 → 工具
//!   执行回灌 → 续轮；收敛与 `RunResult` 填充全自控）
//! - [`runner`]：[`agent::AgentRunner`] 契约实现（启动校验 → rig client
//!   组装 → 泵任务 spawn）
//!
//! 能力 spec：`specs/desktop-agent-execution/spec.md`（路径相对域根）。

pub(crate) mod config;
// `loop` 为 Rust 关键字：文件名按 design 定为 `loop.rs`，模块名以 raw
// identifier 挂载（`r#loop`）
pub(crate) mod r#loop;
pub(crate) mod normalize;
pub(crate) mod policy;
pub(crate) mod resume;
pub(crate) mod runner;
pub(crate) mod sandbox;
pub(crate) mod tools;

#[cfg(test)]
mod config_test;
#[cfg(test)]
mod normalize_test;
#[cfg(test)]
mod policy_test;
#[cfg(test)]
mod resume_test;
#[cfg(test)]
mod runner_test;
#[cfg(test)]
mod sandbox_test;
#[cfg(test)]
mod tools_test;
