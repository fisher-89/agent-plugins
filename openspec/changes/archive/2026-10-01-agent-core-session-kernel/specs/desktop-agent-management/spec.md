# desktop-agent-management Delta

## MODIFIED Requirements

### Requirement: 默认 agent 与运行发起解析

agent 实例 SHALL 支持「默认」标记，全局至多一个；标记新默认 SHALL 原子清除旧默认（切换语义，无需先手动取消）。

运行发起的引擎与连接配置 SHALL 由**内核组合根**单点解析（desktop-agent-execution 会话内核落地，解析点自命令层下沉——解析语义与消费面承诺不变）：

- **缺省路径**（explore 页等一切不显式指定 agent 的发起）：解析默认 agent；无默认 agent SHALL 以 `Err` 显式失败并提示前往管理页配置，MUST NOT 静默回退硬编码引擎或空配置；
- **显式路径**（调试页 agent 选择器）：按所选 agent 解析；
- **解析产物**：`agent.engine` → `EngineKind`（`cli` | `sdk`）；engine 为 `sdk` 时由引用 provider 组装 `EngineConfig{api_key, base_url, model}`，model 固定取 provider 三档中的 **high** 档（档位可调点由 design 定稿）；engine 为 `cli` 时不消费 `EngineConfig`（门面既有约定不变）。

解析 SHALL 收在内核组合根单点：`EngineFacade::runner_for` 签名、`EngineConfig` 三字段结构体 MUST NOT 变化（既有换源零改动承诺平移为「换解析落点零改动」）；命令层 SHALL 无引擎/agent 解析残留。`crates/core/agent` MUST NOT 出现 agent / engine 概念：协议参数面无 agent 字段，`AgentStartError` 不加变体（连接配置不齐走既有 `ConfigMissing` 显式启动失败）。

#### Scenario: 缺省解析默认 agent

- **WHEN** explore 页等不显式指定 agent 发起运行，且存在已标记默认的 sdk agent
- **THEN** 内核组合根解析该 agent 并经门面构造 SDK runner（`EngineConfig` 来自其引用 provider，model 取 high 档），发起路径与显式选择该 agent 完全一致

#### Scenario: 无默认显式报错

- **WHEN** 无默认 agent 时以缺省路径发起运行
- **THEN** 命令返回 `Err` 且提示前往管理页配置，不产生会话/轮记录、不推流、不静默回退

#### Scenario: cli 默认 agent 解析

- **WHEN** 默认 agent 为 `cli` 引擎时以缺省路径发起运行
- **THEN** 走 CLI 引擎且 provider 引用不被消费（可为空）；CLI 不可发现时按既有 `CliMissing` 口径显式失败

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现、命令层与 `crates/core/agent` 源码
- **THEN** 解析语义与下沉前一致（默认/显式/sdk 组装/high 档/无默认 `Err`），`runner_for` 签名与 `EngineConfig` 结构体零 diff、命令层无解析残留；core 无 agent / engine 字样且 `AgentStartError` 变体集不变

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store`（管理记录） | provider / agent 持久化 | 语义不变（本 change 不动管理数据模型与操作面） |
| `commands/agents/` | 管理命令面 | 语义不变（CRUD + 默认标记；本 change 不触碰） |
| 内核组合根（新，落点 design 定稿） | 运行发起解析单点（自 `commands/exec` 下沉） | 缺省解析默认 agent / 显式 agent → (`EngineKind`, `EngineConfig`)；model 取 high 档；无默认 `Err` 引导管理页；`runner_for` / `EngineConfig` 零 diff |
| `packages/desktop/src/views/agents/` | 管理页 | 语义不变；无默认发起的错误呈现经既有 error 态 |
