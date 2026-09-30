# desktop-data-dimensions 变更规格

## MODIFIED Requirements

### Requirement: 数据两维度模型

Desktop 后端持久化数据 SHALL 划分为两个维度，且所有落盘设计（db 文件归属、模型归属声明、文件布局）MUST 显式携带维度语义，MUST NOT 隐式混用：

- **user 维度**：跨 workspace 的 app 状态，生命周期跟随用户，落盘于全局数据目录的全局库（native_db 单库，仅注册 user 维度模型）。代表：workspace 注册表（已落地，模型 `WorkspaceRecord`）；agent 管理配置（已落地，模型 `AgentProviderRecord` / `AgentInstanceRecord`，desktop-agent-management——全局 provider / agent 实例与默认标记，不关联 workspace）；workflow 过程数据（见「workflow 过程数据归 user 维度」，首个待落地租户）；未来租户：设置、窗口状态。
- **workspace 维度**：单 workspace 域内数据（活动历史、绑定元数据、未来的派生缓存与索引等），生命周期跟随 workspace，落盘于全局数据目录 `workspaces/` 子树下按 workspace 分立的独立 db 文件（见「workspace 维度落盘于全局目录 per-workspace db」）。代表：agent 运行历史（已落地，模型 `AgentRunRecord` / `AgentEventRecord`——每条 run 的 cwd 恒为当前 workspace root，运行事件转录随所属 workspace 归档治理）；explore 清单（已落地，模型 `ExploreRecord`——记录自带 root 归属）；未来租户：change 缓存、change 索引、关系图谱。

维度语义的载体 SHALL 为 db 文件归属（全局库 vs per-workspace 库）与模型注册分组（两组 `Models` 与库一一对应，见 desktop-workspace-store「全局库与 workspace 库双库布局」）；redb 手写表的 `user_` 前缀表命名随引擎升级（native-db-store-upgrade）退役的留痕不变。新增任何持久化数据 SHALL 在设计时声明所属维度，MUST NOT 混入非本维度库。

#### Scenario: 维度语义显式

- **WHEN** 审查全局库与 workspace 库的模型注册清单与任何新增持久化数据的设计文档
- **THEN** 全局库内全部模型可归入 user 维度，各 workspace 库内全部模型可归入 workspace 维度；新增数据的设计显式声明 user 或 workspace 维度，不存在维度混用的落盘

#### Scenario: agent 运行历史归 workspace 维度

- **WHEN** 审查 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 三模型的落盘位置
- **THEN** 位于所属 workspace 的独立 db 文件（全局目录 `workspaces/` 子树），未落全局库、未落 workspace repo

#### Scenario: agent 管理配置归 user 维度

- **WHEN** 审查 `AgentProviderRecord` / `AgentInstanceRecord` 两模型的落盘位置与消费方
- **THEN** 位于全局库（不关联 workspace，任一壳态可用），未落任何 workspace 库与 workspace repo；未来 workspace 关联 agent 引用的是其稳定 id，记录本体不随 workspace 迁移

#### Scenario: workspace 注册表不因维度模型受影响

- **WHEN** 任一 workspace 维度数据写入、清理或其 db 文件损坏
- **THEN** user 维度的 workspace 注册表仍在全局库原位完好，MUST NOT 被迁移或混入 workspace 维度存储

#### Scenario: 表名前缀退役可考

- **WHEN** 审查现役库文件
- **THEN** 无 `user_*` redb 表残留，维度语义由 db 文件归属与模型注册分组承载且本 requirement 留有适配留痕

## Module Contract

| 模块/概念 | 维度 | 关键契约 |
|------|------|----------|
| 全局库（全局数据目录，native_db，新文件名） | user | user 维度落盘载体（单库）；注册 `WorkspaceRecord` 与 `AgentProviderRecord` / `AgentInstanceRecord`（agent 管理配置，desktop-agent-management）；workflow 过程数据落此（`workspace_root` + `change_name` 字段 + 二级索引检索，裁定不变） |
| workspace 库（`workspaces/` 子树，每 root 一文件） | workspace | workspace 维度落盘载体；`AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 已落地；路径派生单点、确定性、不进 repo；remove 保留文件；零迁移冷启动 |
| 三笔账 | 已退役 | gitignore 分型与克隆可重建随「数据不进 repo」消灭；独占锁由 store 单进程约束承接 |
| 未来租户归位 | 以维度声明为准 | change 缓存、change 索引、关系图谱等按维度声明落位；跨 workspace 知识网（若出现）须单独声明维度，不默认归入任一维度 |
