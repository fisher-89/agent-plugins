# desktop-data-dimensions Specification（delta: backend-notification-hub）

## Purpose

本 delta 只把「通知收件箱」登记进 user 维度代表清单并补一条维度判据；两维度模型的定义、workspace 维度落盘于 `workspaces/` 子树、change 流程状态归 workspace 维度、模式间零迁移等陈述逐字不动。收件箱的模型与读写面归 `desktop-workspace-store` / `desktop-notification-hub` 持有，本 delta 只声明它的**维度归属**。

## MODIFIED Requirements

### Requirement: 数据两维度模型

Desktop 后端持久化数据 SHALL 划分为两个维度，且所有落盘设计（db 文件归属、模型归属声明、文件布局）MUST 显式携带维度语义，MUST NOT 隐式混用：

- **user 维度**：跨 workspace 的 app 状态，生命周期跟随用户，落盘于全局数据目录的全局库（native_db 单库，仅注册 user 维度模型）。代表：workspace 注册表（已落地，模型 `WorkspaceRecord`）；agent 管理配置（已落地，模型 `AgentProviderRecord` / `AgentInstanceRecord`，desktop-agent-management——全局 provider / agent 实例与默认标记，不关联 workspace）；通知收件箱（已落地，模型 `NotificationRecord`，desktop-notification-hub——跨工作区单一收件箱，切换工作区不丢未读，记录自带 `root` 归属字符串）；未来租户：设置、窗口状态。
- **workspace 维度**：单 workspace 域内数据（活动历史、绑定元数据、未来的派生缓存与索引等），生命周期跟随 workspace，落盘于全局数据目录 `workspaces/` 子树下按 workspace 分立的独立 db 文件（见「workspace 维度落盘于全局目录 per-workspace db」）。代表：agent 运行历史（已落地，模型 `AgentRunRecord` / `AgentEventRecord`——每条 run 的 cwd 恒为当前 workspace root，运行事件转录随所属 workspace 归档治理）；explore 清单（已落地，模型 `ExploreRecord`——记录自带 root 归属）；change 流程状态（已裁定 2026-10-06，模型 `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord`，见「change 流程状态归 workspace 维度」，落所属 workspace 库）；未来租户：change 缓存、change 索引、关系图谱。

维度语义的载体 SHALL 为 db 文件归属（全局库 vs per-workspace 库）与模型注册分组（两组 `Models` 与库一一对应，见 desktop-workspace-store「全局库与 workspace 库双库布局」）；redb 手写表的 `user_` 前缀表命名随引擎升级（native-db-store-upgrade）退役的留痕不变。新增任何持久化数据 SHALL 在设计时声明所属维度，MUST NOT 混入非本维度库。

#### Scenario: 维度语义显式

- **WHEN** 审查全局库与 workspace 库的模型注册清单与任何新增持久化数据的设计文档
- **THEN** 全局库内全部模型可归入 user 维度，各 workspace 库内全部模型可归入 workspace 维度；新增数据的设计显式声明 user 或 workspace 维度，不存在维度混用的落盘

#### Scenario: agent 运行历史归 workspace 维度

- **WHEN** 审查 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 三模型的落盘位置
- **THEN** 位于所属 workspace 的独立 db 文件（全局目录 `workspaces/` 子树），未落全局库、未落 workspace repo

#### Scenario: change 流程状态归 workspace 维度

- **WHEN** 审查 `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` 四模型的落盘位置
- **THEN** 位于所属 workspace 的独立 db 文件，与该 change 名下会话转录同库（槽位 join 同库收敛），未落全局库、未落 workspace repo

#### Scenario: agent 管理配置归 user 维度

- **WHEN** 审查 `AgentProviderRecord` / `AgentInstanceRecord` 两模型的落盘位置与消费方
- **THEN** 位于全局库（不关联 workspace，任一壳态可用），未落任何 workspace 库与 workspace repo；未来 workspace 关联 agent 引用的是其稳定 id，记录本体不随 workspace 迁移

#### Scenario: 通知收件箱归 user 维度

- **WHEN** 审查 `NotificationRecord` 的落盘位置与消费方
- **THEN** 位于全局库（跨 workspace 单一收件箱：切换当前根不丢未读、别的工作区的事件同样可见），未落任何 workspace 库与 workspace repo；工作区归属由记录自带 `root` 字符串承载，MUST NOT 依赖跨库 join

#### Scenario: workspace 注册表不因维度模型受影响

- **WHEN** 任一 workspace 维度数据写入、清理或其 db 文件损坏
- **THEN** user 维度的 workspace 注册表仍在全局库原位完好，MUST NOT 被迁移或混入 workspace 维度存储

#### Scenario: 表名前缀退役可考

- **WHEN** 审查现役库文件
- **THEN** 无 `user_*` redb 表残留，维度语义由 db 文件归属与模型注册分组承载且本 requirement 留有适配留痕

## Module Contract

### 模型增量：通知收件箱的维度归属（backend-notification-hub）

| 模块/概念 | 维度 | 关键契约 |
|---|---|---|
| 全局库（全局数据目录，native_db） | user | 用户维度落盘载体；本变更补 `NotificationRecord`（通知收件箱），与 `WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord` 同库 |
| `NotificationRecord` | user | 跨工作区通知历史与未读；`root` 字符串承载工作区归属（禁跨库 join）；字段面见 desktop-notification-hub，库归属见 desktop-workspace-store |
| workspace 库（`workspaces/` 子树） | workspace | 零改动：收件箱 MUST NOT 落此维度；跨工作区可见性正是选 user 维度的理由 |
