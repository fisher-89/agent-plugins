# desktop-workspace-store Delta

## ADDED Requirements

### Requirement: AgentRunRecord 状态字段类型化

`AgentRunRecord` 的 `env` / `permission_mode` / `status` 三字段 SHALL 类型化为 core/agent 枚举（`AgentEnvMode` / `AgentPermissionMode` / 新增 `AgentRunStatus`），MUST NOT 再以 `String` 承载受控值域；「受控字符串」模型约定（store 不引本地枚举）SHALL 退役。`AgentRunStatus` SHALL 定义于 `crates/core/agent`（与既有两枚举同列），变体为 `Running` / `Completed` / `Failed` / `Stopped`（camelCase serde rename 与现值域逐字一致）。

serde camelCase 线格式 SHALL 与枚举化前逐字不变（unit variant + `rename_all = "camelCase"`：`AcceptEdits` → `"acceptEdits"` 等）；落库编码形态若因枚举化变化（bincode 的 String 长度前缀与变体索引不等价，变化必然发生），SHALL 经 native_model 版本机制原地演进（`AgentRunRecord` 版本递增，`from` 枚举化前旧形态自动升级），MUST NOT 引入手工迁移步骤或 legacy 迁移层。枚举化前 SHALL 先完成存量库三字段值域扫描，确认无野值；转换遇野值的处理策略（报错 vs 兜底）由 design 定夺。`source` / `source_ref` SHALL 保持 `String`（两侧均为 string，不构成类型降级；收紧另开 change）。

落库写入路径 MUST NOT 再经 `as_str().to_owned()` 字符串降级（直写枚举）；`as_str` 的「与 serde 线格式一致」双轨口径 SHALL 退役。CLI flag 组装等真需字符串值处 SHALL 经单一来源取得，取值行为不变（design 定夺实现形态）。

#### Scenario: 线格式逐字不变

- **WHEN** 对枚举化后的 `AgentRunRecord` 做 serde JSON 序列化并与枚举化前值域比对
- **THEN** `env` / `permission_mode` / `status` 输出逐字一致（`"default"` / `"acceptEdits"` / `"running"` 等），IPC 与前端所见值域不变

#### Scenario: 存量记录自动升级

- **WHEN** 打开含枚举化前形态 `AgentRunRecord` 的存量库并重放历史 run
- **THEN** 记录经 native_model 版本机制自动升级可读，三字段值域正确还原，无手工迁移步骤、无 legacy 迁移层

#### Scenario: 存量值域扫描先行

- **WHEN** 执行枚举化落地
- **THEN** 存量库 `env` / `permission_mode` / `status` 三字段值域扫描先行完成并确认无野值，扫描结论留档

#### Scenario: 写入不再字符串降级

- **WHEN** 审查 run begin / finish 的落库写入路径
- **THEN** 三字段直写枚举，无 `as_str().to_owned()` 降级；`STATUS_RUNNING` 等裸字符串常量随之退役

#### Scenario: 前端 union 不降级

- **WHEN** 审查生成物中 `AgentRunRecord` 的 `status` / `env` / `permissionMode` 类型
- **THEN** 为字面量 union（`'running' | 'completed' | 'failed' | 'stopped'` 等），非 `string`——Rust 枚举成为单一事实源，TS 侧类型强度不降

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/runner.rs`（`AgentRunStatus` 新增） | run 状态枚举契约 | `Running` / `Completed` / `Failed` / `Stopped` 四变体；serde camelCase 与现值域逐字一致；与 `AgentEnvMode` / `AgentPermissionMode` 同列承载 `specta::Type` |
| `crates/infra/store/src/model.rs`（`AgentRunRecord`） | 三字段类型化 + 版本演进 | `env` / `permission_mode` / `status` String → 枚举；native_model 版本原地演进（from 旧形态自动升级）；`source` / `source_ref` 保持 String；无迁移层 |
| 落库写入路径（`commands/exec` 编排） | 写入类型化 | 直写枚举，`as_str().to_owned()` 降级与 `STATUS_*` 裸常量退役；flag 组装字符串值单一来源（行为不变） |
