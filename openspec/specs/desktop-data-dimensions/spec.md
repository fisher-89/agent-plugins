# desktop-data-dimensions Specification

## Purpose

约束 Desktop 后端持久化数据的维度划分与落盘方式：user / workspace 两维度显式化（db 文件归属、模型归属声明、文件布局携带维度语义），workspace 维度数据落盘于 workspace 内并经 foundation layout 解析器收口，未来 workspace 维度持久化须偿还三笔账（redb 独占锁错误语义、gitignore 按数据类型分、克隆可重建）。

## Requirements

### Requirement: 数据两维度模型

Desktop 后端持久化数据 SHALL 划分为两个维度，且所有落盘设计（db 文件归属、模型归属声明、文件布局）MUST 显式携带维度语义，MUST NOT 隐式混用：

- **user 维度**：跨 workspace 的 app 状态，生命周期跟随用户，落盘于 app data dir 的 user db（native_db 单库）。代表：workspace 注册表（已落地，模型 `WorkspaceRecord`，前身 redb 表 `user_workspaces` / `user_meta`）；agent 调试运行记录（已落地，模型 `AgentRunRecord` / `AgentEventRecord`，前身 redb 表 `user_agent_runs` / `user_agent_run_events`——运行事件转录属个人活动历史，不可重建、非派生缓存）；workflow 过程数据（见「workflow 过程数据归 user 维度」，首个待落地租户）；未来租户：设置、窗口状态。
- **workspace 维度**：单 workspace 域内数据（含派生缓存、索引、未来的关系图谱等），生命周期跟随 workspace。代表（未来租户）：change 缓存、change 索引、关系图谱。

维度语义的载体 SHALL 随 store 引擎升级（native-db-store-upgrade）适配：redb 手写表的 `user_` 前缀表命名随引擎退役，维度语义改由 db 文件归属（user db 落 app data dir）与模型归属声明承载；新增任何持久化数据 SHALL 在设计时声明所属维度，MUST NOT 混入非本维度库。

#### Scenario: 维度语义显式

- **WHEN** 审查 user db 的模型注册清单与任何新增持久化数据的设计文档
- **THEN** user db 内全部模型可归入 user 维度；新增数据的设计显式声明 user 或 workspace 维度，不存在维度混用的落盘

#### Scenario: agent 运行记录归 user 维度

- **WHEN** 审查 `AgentRunRecord` / `AgentEventRecord` 两模型
- **THEN** 位于 app data dir 的 user db，未落 workspace repo，未与 workspace 维度数据混用

#### Scenario: workspace 注册表不因维度模型受影响

- **WHEN** 未来新增 workspace 维度数据落盘
- **THEN** user 维度的 workspace 注册表仍在 app data dir 原位，MUST NOT 被迁移或混入 workspace 维度存储

#### Scenario: 表名前缀退役可考

- **WHEN** 审查迁移后的新格式库
- **THEN** 无 `user_*` redb 表残留，维度语义由 db 文件归属承载且本 requirement 留有适配留痕

### Requirement: workflow 过程数据归 user 维度

workflow 过程数据（workflow run 记录、phase 执行记录、phase 背后的完整 agent 执行明细与事件流——评估结论与 file_log 之外的全部执行过程）SHALL 归 **user 维度**：落盘于 app data dir 的 user db，MUST NOT 落 workspace repo（执行明细转录进 git = 噪音 + 体积 + 隐私），MUST NOT 落 workspace 维度库（不可重建的活动历史不满足 workspace 维度"SHALL 作为派生数据可重建"的硬约束，且引入三笔账）。过程记录 SHALL 携带 `workspace_root` 与 `change_name` 字段实现归属；按 workspace / change 检索 SHALL 经二级索引查询，MUST NOT 依赖按 workspace 分库。桌面端尚无 workflow 编排生产者，模型注册（`WorkflowRunRecord` / `PhaseExecutionRecord` 等）SHALL 随首个写入方变更落地，本维度裁定先行约束其设计。

#### Scenario: 过程数据维度声明

- **WHEN** 首个 workflow 过程数据写入方变更落地并设计其模型
- **THEN** 模型声明 user 维度、落 app data dir user db，记录携带 `workspace_root` + `change_name`，未落 workspace repo 或 workspace 维度库

#### Scenario: 执行史索引查询

- **WHEN** 查询某 workspace 某 change 的完整执行史（workflow run → phase 执行 → agent run → 事件流）
- **THEN** 经二级索引链逐级扫描取得，不依赖全表读 + 内存排序，不依赖分库

#### Scenario: 三笔账不触发

- **WHEN** 评审过程数据落 user 维度的三笔账（redb 双开锁 / gitignore 分型 / 克隆重建）
- **THEN** 三笔账均不触发：user db 已在 app data dir 受单进程约束治理、不进 repo 无 gitignore 问题、非派生数据本就无需重建

### Requirement: workspace 维度落盘于 workspace 内

workspace 维度数据 SHALL 落盘于 workspace 内（当前磁盘布局下即 `.openspec/` 树内；具体目录层由未来 workspace 维度变更的 dev-design 定），MUST NOT 采用 home_dir 下按 workspace hash 分目录的方案（该方案使数据不跟随 repo 移动）。落盘路径解析 SHALL 经 foundation layout 解析器收口（沿用既有"磁盘真实路径收口"约束），消费侧 MUST NOT 硬编码路径字符串。

workspace 维度数据的 gitignore 策略 SHALL 按数据类型分别决定：派生缓存、change 索引等 SHALL 默认 ignore；若将来出现值得随 repo 分享的状态，SHALL 分目录放置并单独决定是否提交，MUST NOT 一刀切全 ignore 或全提交。

#### Scenario: 数据跟随 repo

- **WHEN** 用户移动或克隆 workspace 所在 repo 后重新打开该 workspace
- **THEN** workspace 维度数据仍位于 workspace 内（或作为派生数据可重建），MUST NOT 出现数据遗留在旧机器 home_dir 下按 hash 寻址的孤儿状态

#### Scenario: 路径解析收口

- **WHEN** 未来实现 workspace 维度落盘
- **THEN** 落盘路径经 foundation 的 layout 解析器取得，消费侧源码无硬编码目录字符串

#### Scenario: gitignore 分型

- **WHEN** 未来实现 workspace 维度落盘且数据类型含派生缓存与可分享状态两类
- **THEN** 两类数据分目录放置，gitignore 策略按类型分别决定，而非整目录一刀切

### Requirement: workspace 维度持久化的落地约束（三笔账）

未来实现 workspace 维度持久化的变更 SHALL 偿还三笔账，proposal / dev-design MUST 引用本节逐项交代：

1. **redb 独占锁错误语义**：redb 同一 db 文件为单进程写锁；第二个打开者（desktop 双窗口、他进程）MUST 得到显式错误语义（重试或只读降级，届时定），MUST NOT 静默失败或数据竞争。
2. **gitignore 按数据类型分**：见上一 requirement；落地时 SHALL 给出类型-目录-策略对照。
3. **克隆可重建语义**：workspace 维度数据 SHALL 作为派生数据可重建（缓存重算、索引重扫），应用 MUST NOT 依赖其存在性才能工作；克隆到新机器后缺失数据 SHALL 不致错误。

#### Scenario: 独占锁冲突有语义

- **WHEN** 未来 workspace 维度 redb 文件已被另一进程打开，本进程再尝试打开
- **THEN** 得到显式错误（或按既定策略降级为只读），用户可感知、可恢复，无静默失败

#### Scenario: 克隆后缺失可重建

- **WHEN** 新机器 clone repo 后首次打开 workspace，workspace 维度缓存/索引不存在
- **THEN** 应用正常工作并按需重建派生数据，不报错、不白屏

## Module Contract

| 模块/概念 | 维度 | 关键契约 |
|------|------|----------|
| user db（app data dir，native_db） | user | user 维度落盘载体（单库）；workspace 注册表（`WorkspaceRecord`）与 agent 运行记录（`AgentRunRecord` / `AgentEventRecord`）已落地；维度语义由 db 文件归属承载（`user_*` 表名前缀随引擎退役） |
| workflow 过程数据（未来租户） | user | 落 user db；记录携带 `workspace_root` + `change_name`；按 change / workspace 检索经二级索引；不落 workspace repo、不落 workspace 维度库、不触发三笔账 |
| workspace 维度数据 | workspace | 落盘 workspace 内（`.openspec/` 下，具体层 dev-design 定）；路径经 foundation 收口；gitignore 按类型分；独占锁错误语义显式；可克隆重建（既有 requirement 不变） |
| 未来租户归位 | 以 workspace 为主 | change 缓存、change 索引、关系图谱等按维度声明落位；跨 workspace 知识网（若出现）须单独声明维度，不默认归入任一维度 |
