# desktop-workspace-store 变更规格

## MODIFIED Requirements

### Requirement: 全局库与 workspace 库双库布局

store SHALL 按数据维度拆分落盘载体为两组 db 文件：**全局库**注册 user 维度模型组（`WorkspaceRecord` 与 agent 管理两记录 `AgentProviderRecord` / `AgentInstanceRecord`，见「AgentProviderRecord 与 AgentInstanceRecord 模型与管理操作面」）；**workspace 库**按 workspace 分立、每 workspace 恰一个独立 db 文件，注册 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 三个 workspace 维度模型。全局库 SHALL 位于全局数据目录（沿用 `home_dir()/.dev-team/` 根）并启用与旧单库不同的新文件名；workspace 库 SHALL 位于全局数据目录下 `workspaces/` 子树。任一库的模型归属 MUST NOT 混注：workspace 域模型 MUST NOT 出现在全局库注册清单，user 维度模型 MUST NOT 出现在 workspace 库注册清单（维度语义由 db 文件归属承载，见 desktop-data-dimensions）。数据目录根 SHALL 由 desktop-app 注入，store 内零环境解析（既有 `Store::open` 注入式打开约束不变）；静态模型注册 SHALL 拆为与两库一一对应的两组 `Models`。

#### Scenario: 模型注册分组

- **WHEN** 审查 store 的静态模型注册与两组 `Models` 的消费方
- **THEN** 全局库组定义 `WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord`，workspace 库组仅定义 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`，无交叉注册

#### Scenario: 数据分流写入

- **WHEN** `add_workspace` 注册目录后，对该 workspace 执行 `create_explore_record` 与 `begin_agent_run`，并在管理页新建 provider 与 agent
- **THEN** 注册记录与 provider / agent 记录落在全局库，explore 记录与 run 记录落在该 workspace 库，全局库无 workspace 域记录混入

#### Scenario: 注入式打开不变

- **WHEN** 审查 store 公共 API 与打开流程
- **THEN** 两库路径最终均来自调用方注入（数据目录根由 desktop-app 解析），store 源码无 home_dir / app data 目录解析逻辑

## ADDED Requirements

### Requirement: AgentProviderRecord 与 AgentInstanceRecord 模型与管理操作面

store SHALL 以两个新模型承载全局 agent 管理数据（user 维度，desktop-agent-management），落全局库，native_model 分配新 id（既有 1–4 已占用，分配由 design 定稿）：

- `AgentProviderRecord`：稳定 id 主键（写事务内 max+1 分配，与 `AgentRunRecord` / `ExploreRecord` 同语义）、name（全局唯一）、base_url、api_key、`models { high, medium, low }` 三档。SHALL 破例不 derive `Debug`，手写遮蔽 Debug impl（api_key 位输出 `sk-***abc` 形态，机密边界与统一注释标记见 desktop-agent-management「api_key 机密边界」）；测试比较走 `PartialEq`；
- `AgentInstanceRecord`：稳定 id 主键（同上）、name（全局唯一）、engine（`cli` | `sdk`）、provider_id（engine 为 `sdk` 时必填、`cli` 时可空）、默认标记（全局至多一个）。

两记录的 name SHALL 走唯一二级索引（或口径一致的等效约束，实现形态由 design spike 定稿）；重名写入 SHALL 返回 `StoreError`。store SHALL 经全局库实例提供管理操作面：provider / agent 的增、改、删、清单与默认标记设置——标记新默认 SHALL 在写事务内原子清除旧默认（全局恒至多一个）；删被 agent 引用的 provider SHALL 返回 `StoreError` 阻止（MUST NOT 级联删除引用方）；删默认 agent SHALL 在同一事务内清空默认标记后删除。

存量全局库 SHALL additive 兼容：已存在的 `desktop-global.redb` 打开后两新模型即可读写，存量 `WorkspaceRecord` 原样可读，MUST NOT 要求手工迁移或引入迁移层（与「全新文件组冷启动与旧库惰性废弃」的零迁移纪律同轨——本处为打开既有新格式库的加法语义，非旧格式迁移）。新模型 SHALL 随记录信封 API（`list_models` / `scan`）零改动可浏览。

#### Scenario: 回环读写

- **WHEN** 在全局库依次新建 provider（含三档 models）与引用它的 sdk agent，重开同一 db 文件后清单查询
- **THEN** 两记录字段与写入一致（含 id 稳定、默认标记如实），id 经写事务内 max+1 分配

#### Scenario: 重名报错

- **WHEN** 以已有 provider（或 agent）的 name 再次写入同类型记录
- **THEN** 写入返回 `StoreError`（重名），记录数量不变；agent 与 provider 两类的 name 空间相互独立不互斥

#### Scenario: 默认标记至多一个

- **WHEN** agent A 已标记默认后，将 agent B 标记为默认
- **THEN** 同一事务内 B 成为唯一默认、A 的标记被清除；任意时刻清单查询恰有零或一个默认 agent

#### Scenario: 引用删除阻止

- **WHEN** 删除仍被 agent 引用的 provider
- **THEN** 返回 `StoreError`，provider 与引用它的 agent 均原样保留；解除引用（改指他者或删 agent）后删除成功

#### Scenario: 删默认清标记

- **WHEN** 删除被标记默认的 agent
- **THEN** 删除成功且默认标记同事务消失（无顺延）；后续清单查询零默认

#### Scenario: 存量库 additive 打开

- **WHEN** 在含存量 `WorkspaceRecord` 的 `desktop-global.redb` 上以新版本打开
- **THEN** 打开成功，存量记录原样可读，两新模型可立即写入读出；store 无迁移代码路径

#### Scenario: 信封 API 零改动覆盖

- **WHEN** 两新模型注册后调用全局库实例的 `list_models` / `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与 DB 查看器代码无改动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| 静态模型注册（两组 `Models`） | 两库模型清单 | 全局组 `WorkspaceRecord` + `AgentProviderRecord` + `AgentInstanceRecord`；workspace 组 run / 事件 / explore 三模型不变；无交叉注册 |
| `AgentProviderRecord`（模型，新） | provider 实例（user 维度） | id 主键（max+1）/ name 唯一 / base_url / api_key / models 三档；不 derive `Debug`，手写遮蔽 impl；落全局库 |
| `AgentInstanceRecord`（模型，新） | agent 实例（user 维度） | id 主键 / name 唯一 / engine `cli\|sdk` / provider_id（sdk 必填 cli 可空）/ 默认标记至多一；落全局库 |
| 管理操作面（新，全局库实例） | provider / agent CRUD + 默认标记 | 唯一约束报 `StoreError`；标记新默认原子清旧；引用删除阻止；删默认清标记；信封 API 零改动覆盖 |
| `crates/infra/store/src/model.rs` / `store.rs` | 落点 | 模型定义与全局组注册、操作面实现；公共 API 只暴露自有类型（native_db 类型不越 crate 公共面不变） |
