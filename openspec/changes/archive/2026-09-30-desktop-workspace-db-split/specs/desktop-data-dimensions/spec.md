# desktop-data-dimensions 变更（desktop-workspace-db-split）

## ADDED Requirements

### Requirement: workspace 维度落盘于全局目录 per-workspace db

workspace 维度数据 SHALL 落盘于**全局数据目录下按 workspace 分立的 db 文件**（`home_dir()/.dev-team/workspaces/` 子树，每 workspace 恰一个独立 db 文件），MUST NOT 落 workspace repo（数据不进 repo，gitignore 分型与克隆可重建两笔约束随之消灭），MUST NOT 落全局库与 workspace 维度库混居。文件路径 SHALL 由 canonical root 经单点纯函数确定性派生（同根恒同名、异根不同名，见 desktop-workspace-store「workspace 库文件确定性寻址」）；派生收口约束替代原「foundation layout 解析器收口」在本维度的适用（layout 解析器约束 openspec 域目录树，与 db 落位分属两域，互不越界）。workspace 库文件的生命周期 SHALL 跟随 workspace 注册：`remove_workspace` MUST NOT 删除库文件（重加同 root 历史完整恢复），孤儿文件治理留待二期。存量数据 SHALL 零迁移处置（用户裁定，2026-09-29：无需考虑兼容历史数据），旧单库惰性废弃（见 desktop-workspace-store「全新文件组冷启动与旧库惰性废弃」）。

#### Scenario: 数据不进 repo

- **WHEN** 审查任一 workspace 维度模型的落盘位置
- **THEN** 位于全局目录 `workspaces/` 子树对应 workspace 的 db 文件，workspace repo（含 `.openspec/` 树）内无任何 db 落盘数据

#### Scenario: 每 workspace 物理隔离

- **WHEN** 应用注册并使用两个 workspace 后审查磁盘
- **THEN** `workspaces/` 子树下存在两个独立 db 文件，各自仅含本 workspace 的记录；任一文件损坏不波及另一 workspace 与全局库

#### Scenario: 移除注册文件保留

- **WHEN** `remove_workspace` 移除某注册项
- **THEN** 对应 workspace db 文件保留原位，重新添加同根目录后历史完整恢复

## MODIFIED Requirements

### Requirement: 数据两维度模型

Desktop 后端持久化数据 SHALL 划分为两个维度，且所有落盘设计（db 文件归属、模型归属声明、文件布局）MUST 显式携带维度语义，MUST NOT 隐式混用：

- **user 维度**：跨 workspace 的 app 状态，生命周期跟随用户，落盘于全局数据目录的全局库（native_db 单库，仅注册 user 维度模型）。代表：workspace 注册表（已落地，模型 `WorkspaceRecord`）；workflow 过程数据（见「workflow 过程数据归 user 维度」，首个待落地租户）；未来租户：设置、窗口状态。
- **workspace 维度**：单 workspace 域内数据（活动历史、绑定元数据、未来的派生缓存与索引等），生命周期跟随 workspace，落盘于全局数据目录 `workspaces/` 子树下按 workspace 分立的独立 db 文件（见「workspace 维度落盘于全局目录 per-workspace db」）。代表：agent 运行历史（已落地，模型 `AgentRunRecord` / `AgentEventRecord`——每条 run 的 cwd 恒为当前 workspace root，运行事件转录随所属 workspace 归档治理）；explore 清单（已落地，模型 `ExploreRecord`——记录自带 root 归属）；未来租户：change 缓存、change 索引、关系图谱。

维度语义的载体 SHALL 为 db 文件归属（全局库 vs per-workspace 库）与模型注册分组（两组 `Models` 与库一一对应，见 desktop-workspace-store「全局库与 workspace 库双库布局」）；redb 手写表的 `user_` 前缀表命名随引擎升级（native-db-store-upgrade）退役的留痕不变。新增任何持久化数据 SHALL 在设计时声明所属维度，MUST NOT 混入非本维度库。

#### Scenario: 维度语义显式

- **WHEN** 审查全局库与 workspace 库的模型注册清单与任何新增持久化数据的设计文档
- **THEN** 全局库内全部模型可归入 user 维度，各 workspace 库内全部模型可归入 workspace 维度；新增数据的设计显式声明 user 或 workspace 维度，不存在维度混用的落盘

#### Scenario: agent 运行历史归 workspace 维度

- **WHEN** 审查 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 三模型的落盘位置
- **THEN** 位于所属 workspace 的独立 db 文件（全局目录 `workspaces/` 子树），未落全局库、未落 workspace repo

#### Scenario: workspace 注册表不因维度模型受影响

- **WHEN** 任一 workspace 维度数据写入、清理或其 db 文件损坏
- **THEN** user 维度的 workspace 注册表仍在全局库原位完好，MUST NOT 被迁移或混入 workspace 维度存储

#### Scenario: 表名前缀退役可考

- **WHEN** 审查现役库文件
- **THEN** 无 `user_*` redb 表残留，维度语义由 db 文件归属与模型注册分组承载且本 requirement 留有适配留痕

## REMOVED Requirements

### Requirement: workspace 维度落盘于 workspace 内

**Reason**: 用户裁定反转落盘方案（2026-09-29）：workspace 维度数据改落全局数据目录下按 workspace 分立的 db 文件，依然放在全局目录下；原「workspace 内（`.openspec/` 树内）落盘」与「MUST NOT 采用 home_dir 下按 workspace hash 分目录」的裁定随之退役。数据不进 repo 后，该 requirement 的立论前提（数据跟随 repo 移动 / 克隆）不复存在。

**Migration**: 由本变更 ADDED「workspace 维度落盘于全局目录 per-workspace db」承接；路径收口由 desktop-workspace-store 的派生单点承接（foundation layout 解析器约束域回归 openspec 域目录树）。该 requirement 为未来租户约束，无存量 repo 内落盘数据需迁移。

### Requirement: workspace 维度持久化的落地约束（三笔账）

**Reason**: 三笔账中的 gitignore 按数据类型分与克隆可重建两笔以「数据落 workspace repo 内」为前提，前提随落盘方案反转消灭（数据不进 repo，无 gitignore 问题；数据不在 repo 中，克隆缺失语义不再适用）；redb 独占锁一笔不以 repo 为前提，由 desktop-workspace-store 既有「单进程约束显式化」与本变更新增的「workspace store 注册表与进程内单开」（进程内单开硬约束）承接。

**Migration**: 独占锁约束承接处已列（显式错误语义、单实例插件、进程内复用实例）；无数据迁移需求（存量处置为零迁移冷启动）。

## Module Contract

| 模块/概念 | 维度 | 关键契约 |
|------|------|----------|
| 全局库（全局数据目录，native_db，新文件名） | user | user 维度落盘载体（单库）；仅注册 `WorkspaceRecord` 及未来 user 维度租户；workflow 过程数据落此（`workspace_root` + `change_name` 字段 + 二级索引检索，裁定不变） |
| workspace 库（`workspaces/` 子树，每 root 一文件） | workspace | workspace 维度落盘载体；`AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 已落地；路径派生单点、确定性、不进 repo；remove 保留文件；零迁移冷启动 |
| 三笔账 | 已退役 | gitignore 分型与克隆可重建随「数据不进 repo」消灭；独占锁由 store 单进程约束承接 |
| 未来租户归位 | 以维度声明为准 | change 缓存、change 索引、关系图谱等按维度声明落位；跨 workspace 知识网（若出现）须单独声明维度，不默认归入任一维度 |
