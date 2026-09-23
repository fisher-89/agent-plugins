# desktop-agent-execution — desktop-data-dimensions 变更集

> agent 调试运行记录归 user 维度：`user_agent_runs` / `user_agent_run_events` 两表落 app data dir 既有 db。本 delta 为维度归属注记，落库细节见 `desktop-agent-execution` 能力 spec。

## MODIFIED Requirements

### Requirement: 数据两维度模型

Desktop 后端持久化数据 SHALL 划分为两个维度，且所有落盘设计（store 表命名、文件布局）MUST 显式携带维度语义，MUST NOT 隐式混用：

- **user 维度**：跨 workspace 的 app 状态，生命周期跟随用户，落盘于 app data dir。代表：workspace 注册表（已落地，`user_workspaces` / `user_meta`）；agent 调试运行记录（已落地，`user_agent_runs` / `user_agent_run_events`——运行事件转录属个人活动历史，不可重建、非派生缓存）；未来租户：设置、窗口状态。
- **workspace 维度**：单 workspace 域内数据（含派生缓存、索引、未来的关系图谱等），生命周期跟随 workspace。代表（未来租户）：change 缓存、change 索引、关系图谱。

既有 store 表命名（`user_workspaces` / `user_meta` 的 user 前缀）SHALL 作为维度显式化的既有兑现保持不变；新增任何持久化数据 SHALL 在设计时声明所属维度。

#### Scenario: 维度语义显式

- **WHEN** 审查既有 store 表命名与任何新增持久化数据的设计文档
- **THEN** 既有表携带 user 维度前缀；新增数据的设计显式声明 user 或 workspace 维度，不存在维度混用的落盘

#### Scenario: agent 运行记录归 user 维度

- **WHEN** 审查 `user_agent_runs` / `user_agent_run_events` 两表
- **THEN** 位于 app data dir 既有 db，表名携 user 前缀，未落 workspace repo，未与 workspace 维度数据混用

#### Scenario: workspace 注册表不因维度模型受影响

- **WHEN** 未来新增 workspace 维度数据落盘
- **THEN** user 维度的 workspace 注册表仍在 app data dir 原位，MUST NOT 被迁移或混入 workspace 维度存储

## Module Contract

| 模块/概念 | 维度 | 关键契约 |
|------|------|----------|
| `user_agent_runs` / `user_agent_run_events` | user | agent 调试运行记录（元数据 + (run_id, seq) 事件流）；落 app data dir 既有 db；不落 workspace repo；事件转录不可重建，非派生缓存 |
