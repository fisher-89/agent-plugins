# desktop-workspace-store Specification (Delta)

## MODIFIED Requirements

### Requirement: 全局库与 workspace 库双库布局

store SHALL 按数据维度拆分落盘载体为两组 db 文件：**全局库**注册 user 维度模型组（`WorkspaceRecord` 与 agent 管理两记录 `AgentProviderRecord` / `AgentInstanceRecord`，见「AgentProviderRecord 与 AgentInstanceRecord 模型与管理操作面」）；**workspace 库**按 workspace 分立、每 workspace 恰一个独立 db 文件，注册 `AgentRunRecord`（轮统计行）/ 事件记录（会话转录）/ `ExploreRecord` / `SessionRecord` / `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` 八个 workspace 维度模型（change 流程状态四模型见「ChangeRecord 状态模型组与 change 域操作面」与 desktop-change-state-store）。全局库 SHALL 位于全局数据目录（沿用 `home_dir()/.dev-team/` 根）并启用与旧单库不同的新文件名；workspace 库 SHALL 位于全局数据目录下 `workspaces/` 子树。任一库的模型归属 MUST NOT 混注：workspace 域模型 MUST NOT 出现在全局库注册清单，user 维度模型 MUST NOT 出现在 workspace 库注册清单（维度语义由 db 文件归属承载，见 desktop-data-dimensions）。数据目录根 SHALL 由 desktop-app 注入，store 内零环境解析（既有 `Store::open` 注入式打开约束不变）；静态模型注册 SHALL 拆为与两库一一对应的两组 `Models`。

#### Scenario: 模型注册分组

- **WHEN** 审查 store 的静态模型注册与两组 `Models` 的消费方
- **THEN** 全局库组定义 `WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord`，workspace 库组仅定义 `AgentRunRecord` / 会话转录事件记录 / `ExploreRecord` / `SessionRecord` / `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord`，无交叉注册

#### Scenario: 数据分流写入

- **WHEN** `add_workspace` 注册目录后，对该 workspace 发起会话运行、新建 change 并推进相位，同时在管理页新建 provider 与 agent
- **THEN** 注册记录与 provider / agent 记录落在全局库，会话/转录/轮统计行与 change 流程状态落在该 workspace 库，全局库无 workspace 域记录混入

#### Scenario: 注入式打开不变

- **WHEN** 审查 store 公共 API 与打开流程
- **THEN** 两库路径最终均来自调用方注入（数据目录根由 desktop-app 解析），store 源码无 home_dir / app data 目录解析逻辑

## ADDED Requirements

### Requirement: ChangeRecord 状态模型组与 change 域操作面

store SHALL 注册 `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` 四模型（workspace 维度，落所属 workspace 库；字段面与状态语义见 desktop-change-state-store「change 流程状态 workspace 库单源与双向墙」），native_model id 分配 MUST NOT 与既有已占用 id 冲突（分配值由 design 定稿）。store SHALL 经 workspace 库实例提供 change 域操作面：建档（`ChangeRecord` 写入）、相位落账（`PhaseRecord` + checklist 子行 + `active_phase` 更新 + stale 翻转的写事务原子编排，见 desktop-change-state-store「相位机写面单事务落库」）、步骤审计追加与查询、status 翻转（归档双写的 db 半边）。主键分配沿既有惯例：`PhaseRecord.id` 为库域内写事务 max+1，`ChecklistItemRecord` 打包主键 `(phase_id as u128) << 64 | item_index`；`(change, phase, attempt)` 唯一性在写事务内查重、冲突返回 `StoreError`。四新模型 SHALL 随记录信封 API（`list_models` / `scan`）零改动可浏览；存量库 SHALL additive 打开（新模型注册后立即可写可读，零迁移代码路径）。

#### Scenario: 四模型回环读写

- **WHEN** 建档 change 并推进相位（含 checklist 多条与一条步骤审计行）后重开同一 db 文件查询
- **THEN** 四模型记录逐字段与写入一致（含打包键序与时间戳原值）

#### Scenario: 查重与主键分配

- **WHEN** 重复落账同 `(change, phase, attempt)` 组合，并连续落两条 PhaseRecord
- **THEN** 前者返回 `StoreError` 零写入；后者 id 为写事务内 max+1 连续递增

#### Scenario: 信封 API 零改动覆盖

- **WHEN** 四新模型注册后调用 workspace 库实例的 `list_models` / `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与 DB 查看器代码无改动

#### Scenario: 存量库 additive 打开

- **WHEN** 在含存量会话 / explore 记录的 workspace 库上以新版本打开
- **THEN** 打开成功，存量记录原样可读，change 状态四模型可立即写入读出；store 无迁移代码路径

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（四模型增量） | change 状态模型面 | `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord`；native_model id 分配 design 定稿不冲突；core 类型 derive-free 纪律不变（change 状态类型直接驻 infra，无 core 嵌装需求） |
| `crates/infra/store/src/store.rs`（workspace 组注册 + 操作面增量） | 双库注册分组与 change 域操作 | workspace 组由四模型扩至八模型；建档 / 相位落账 / 步骤追加 / status 翻转；写事务原子与查重 |
| 记录信封 API（不改） | 通用读面 | 新模型注册即覆盖（零改动可浏览）；只读边界不变 |
