# desktop-data-dimensions Specification (Delta)

## RENAMED Requirements

- FROM: `### Requirement: workflow 过程数据归 user 维度`
- TO: `### Requirement: change 流程状态归 workspace 维度`

## MODIFIED Requirements

### Requirement: 数据两维度模型

Desktop 后端持久化数据 SHALL 划分为两个维度，且所有落盘设计（db 文件归属、模型归属声明、文件布局）MUST 显式携带维度语义，MUST NOT 隐式混用：

- **user 维度**：跨 workspace 的 app 状态，生命周期跟随用户，落盘于全局数据目录的全局库（native_db 单库，仅注册 user 维度模型）。代表：workspace 注册表（已落地，模型 `WorkspaceRecord`）；agent 管理配置（已落地，模型 `AgentProviderRecord` / `AgentInstanceRecord`，desktop-agent-management——全局 provider / agent 实例与默认标记，不关联 workspace）；未来租户：设置、窗口状态。
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

#### Scenario: 表名前缀退役可考

- **WHEN** 审查现役库文件
- **THEN** 无 `user_*` redb 表残留，维度语义由 db 文件归属与模型注册分组承载且本 requirement 留有适配留痕

### Requirement: change 流程状态归 workspace 维度

change 流程状态（`ChangeRecord` 建档与 active_phase / `PhaseRecord` 相位评估历史 / `ChecklistItemRecord` 评估 checklist / `StepRecord` 步骤审计——原「workflow 过程数据」裁定的 run / phase 记录租户）SHALL 归 **workspace 维度**（2026-10-06 拍板，取代原 user 维度裁定）：落所属 workspace 的独立 db 文件，MUST NOT 落 workspace repo（db 在 app-data，git 零触点），MUST NOT 落全局库。裁定理由：PhaseRecord 会话槽位与该 change 名下 executor / evaluator / decision 会话（`SessionRecord`，workspace 库）构成 join 一等查询，级联与归属治理要求同库收敛（store 惯例禁跨库引用）；「记录在 db、产物在磁盘」与 ExploreRecord 先例同构。原 user 维度裁定的其余对象（phase 背后的完整 agent 执行明细与事件流）本就落 workspace 库（`SessionRecord` 系转录单表），原裁定在其首个写入方到场时整体反转、无存量数据受影响。磁盘 change 目录只承载 markdown 产物；`workflow.json` 退出 desktop 载体面（双向墙，见 desktop-change-state-store）。

#### Scenario: 状态维度声明

- **WHEN** 审查 change 状态四模型的设计与落盘
- **THEN** 模型声明 workspace 维度、落所属 workspace 库，未落 workspace repo 或全局库

#### Scenario: 槽位 join 同库收敛

- **WHEN** 由某 PhaseRecord 的会话槽位 id 反查其 executor / evaluator / decision 会话及转录
- **THEN** join 在同一 workspace 库实例内完成（无跨库引用），详情抽屉逆查为库内一等查询

#### Scenario: 数据不进 repo

- **WHEN** 审查 change 状态四模型的落盘位置
- **THEN** workspace repo（含 `.openspec/` 树与 `openspec/changes/`）内无任何 change 状态 db 落盘数据，磁盘 change 目录只有 markdown 产物

## Module Contract

| 模块/概念 | 维度 | 关键契约 |
|------|------|----------|
| 全局库（全局数据目录，native_db） | user | user 维度落盘载体（单库）；`WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord`；change 流程状态 MUST NOT 落此（原 user 维度裁定 2026-10-06 反转留痕） |
| workspace 库（`workspaces/` 子树，每 root 一文件） | workspace | `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` / `SessionRecord` + change 状态四模型；路径派生单点、确定性、不进 repo；remove 保留文件；零迁移冷启动 |
| `ChangeRecord` 系四模型 | workspace | change 流程状态租户；槽位 join 与级联治理同库收敛；双向墙下 workflow.json 退出载体面 |
| 未来租户归位 | 以维度声明为准 | change 缓存、change 索引、关系图谱等按维度声明落位；跨 workspace 知识网（若出现）须单独声明维度，不默认归入任一维度 |
