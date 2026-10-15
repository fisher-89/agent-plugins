# desktop-workspace-store（delta：快照实例外键 + CLI 写面收窄）

## MODIFIED Requirements

### Requirement: SessionRecord 模型与会话转录单表

store SHALL 以新模型 `SessionRecord`（workspace 维度，落所属 workspace 的独立 db 文件，native_model 分配新 id）承载 agent 会话一等公民：

- `id`：core 铸的会话 id（字符串主键；native_db 对字符串主键的支撑度由 design spike 核实，不支持则退合成数值 id + 会话 id 唯一二级索引，查询形态不变）
- `engine_session_id`：引擎侧标识（双 id 映射的落库半边——**留痕语义**：「这轮落在哪个引擎侧会话」的引擎中立观测/考古事实，每轮随 TurnDone 上报、sdk 每轮铸造；CLI 租户的 claude session id 为历史值）。续会话寻址 SHALL 以会话 id + 全史转录重建为准，MUST NOT 以引擎句柄缺失阻断续会话（寻址硬门槛随 retire-cli-engine 退役）
- `config_snapshot`：装配配置快照（engine / model / 权限档 / **实例外键 `agent_instance_id`**）——SHALL 存快照而非跨库引用（`AgentInstanceRecord` 在全局库、session 在 workspace 库，store 惯例禁跨库引用；实例改名/删除不伤历史会话）。`agent_instance_id: Option<i64>` 为装配时点实例 id 的快照复制值（显式关联键，多 agent 时代的会话↔实例过滤/聚合锚；MUST NOT 引申为跨库 join 或存在性约束）；`engine` 字段保留为历史事实（CLI 租户退役不删字段）
- `source` / `source_ref`：来源归属（`debug` | `explore` | …，缺省 `debug`；explore 指向 `ExploreRecord` 主键）——provenance 持久记录面（后端仍用于来源圈定与 explore 记录级联删除）；前端 SHALL NOT 经 `source_ref` 反查会话（会话寻址与重放走 `sessionId` 直查，见 desktop-agent-execution「session_detail 单查面」）
- `created_at` / `updated_at`（UTC unix 毫秒，同既有时间戳口径）

字段面演进 SHALL 经 native_model 版本链原地升级（`SessionRecord` `version = 2, from = SessionRecordV1`，旧快照 `agent_instance_id` 读出 `None`、其余字段原样；降级半边以缺省占位——只保升级语义真实性），MUST NOT 手工迁移步骤或 legacy 迁移层（`AgentProviderRecordV1→V2`、`ExploreRecord` 三连、`AgentRunRecordV3→V4` 等 6 处先例同模式）。

会话数据 SHALL 收敛为「转录单表 + 轮统计行」：密封事件**直挂 session**（单表，承载模型与键位见「类型化事件模型 AgentEventRecord」）；run 退化为**轮统计行**（每轮一行，挂 session 外键，字段面为 TurnDone 统计口径与时间戳），MUST NOT 再以「单 run 事件 + 链指针」承载会话历史。会话模型 SHALL 随记录信封 API（`list_models` / `scan`）零改动可浏览。

#### Scenario: 会话回环读写

- **WHEN** 建立会话（含 config_snapshot 与来源归属）、追加密封转录与轮统计行后重开同一 db 文件查询
- **THEN** 会话记录、转录与统计行与写入一致（含双 id 映射与时间戳原值）

#### Scenario: 配置快照隔离

- **WHEN** 某会话建立后，其来源 agent 实例在全局库被改名或删除
- **THEN** 会话记录与 config_snapshot 原样可读（快照非引用），历史会话不受影响

#### Scenario: 转录挂 session 与轮统计行

- **WHEN** 审查一个三轮会话的落库形态
- **THEN** 密封事件以 session 为键成单表全史，轮统计行恰三行且各挂该 session，无「按 run 圈定事件」的旧形态残留

#### Scenario: 版本链升级读兼容

- **WHEN** 以新版本打开含 v1 `SessionRecord` 行的存量 workspace 库
- **THEN** 记录经 native_model 版本机制自动升级读出（`config_snapshot.agent_instance_id` 为 `None`、其余字段原样），新写行携 `agent_instance_id`；无手工迁移步骤、无 legacy 迁移层

#### Scenario: 存量库 additive 打开

- **WHEN** 在含存量 workspace 域记录的 workspace 库上以新版本打开
- **THEN** 打开成功，存量记录原样可读，`SessionRecord` 可立即写入读出；store 无迁移代码路径（存量数据处置口径见提案待决问题）

### Requirement: AgentProviderRecord 与 AgentInstanceRecord 模型与管理操作面

store SHALL 以两个新模型承载全局 agent 管理数据（user 维度，desktop-agent-management），落全局库，native_model 分配新 id（既有 1–4 已占用，分配由 design 定稿）：

- `AgentProviderRecord`：稳定 id 主键（写事务内 max+1 分配，与 `AgentRunRecord` / `ExploreRecord` 同语义）、name（全局唯一）、base_url、api_key、`models { high, medium, low }` 三档。SHALL 破例不 derive `Debug`，手写遮蔽 Debug impl（api_key 位输出 `sk-***abc` 形态，机密边界与统一注释标记见 desktop-agent-management「api_key 机密边界」）；测试比较走 `PartialEq`；
- `AgentInstanceRecord`：稳定 id 主键（同上）、name（全局唯一）、engine（`cli` | `sdk`）、provider_id、默认标记（全局至多一个）。**engine 值域语义（retire-cli-engine 修订）**：线值 `cli` | `sdk` 双值 SHALL 永久保留——`AgentEngineKind` 变体集与 bincode 变体序号（`Cli`=0 / `Sdk`=1）MUST NOT 增删重排（`Cli` 变体为历史数据的**解码锚**，注释标「解码兼容，不再可选」；删/重排即 `AgentInstanceRecord` + `SessionRecord` 两表报废含 sdk 数据）；写入面 SHALL 仅接受 `engine: sdk`，`upsert_agent_instance` 对 `engine: Cli` 的新建与更新（整行）SHALL 拒绝，`set_default_agent_instance` 对 `engine: Cli` 实例 SHALL 拒绝——存量 Cli 行可读、可删、不可改；provider_id（`sdk` 必填、引用存在——`sdk` 唯一活档；存量 `cli` 行 provider 可空只读保留）。

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

#### Scenario: 变体序号锚定

- **WHEN** 对含存量 `Cli` / `Sdk` 两类行的全局库做读取与 roundtrip（含嵌 `engine` 的 `SessionRecord` 快照）
- **THEN** `AgentEngineKind` bincode 序号不变（`Cli` 编码首字节 0 / `Sdk` 1，锁定测试断言），两行照读无错位污染

#### Scenario: cli 写面拒收

- **WHEN** 新建或更新 `engine: Cli` 的 agent 实例，或对 Cli 实例设默认
- **THEN** 写入返回 `StoreError`（CLI 租户已下线口径），存量行不变；对存量 Cli 行的读取与删除照常可用

#### Scenario: 存量库 additive 打开

- **WHEN** 在含存量 `WorkspaceRecord` 的 `desktop-global.redb` 上以新版本打开
- **THEN** 打开成功，存量记录原样可读（含存量 Cli 行），两新模型可立即写入读出；store 无迁移代码路径

#### Scenario: 信封 API 零改动覆盖

- **WHEN** 两新模型注册后调用全局库实例的 `list_models` / `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与 DB 查看器代码无改动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs` | 模型面 | `SessionConfigSnapshot` 增 `agent_instance_id: Option<i64>`（serde default）；`SessionRecord` `version = 2, from = SessionRecordV1` + 双向 `From`（旧快照 `agent_instance_id = None`）；`AgentEngineKind` 变体集与序号零改动（解码锚注释） |
| `crates/infra/store/src/store.rs` | 写面校验单点 | `upsert_agent_instance` / `set_default_agent_instance` 拒 `engine: Cli`；`bind_session_remote` 留痕写面保留；`SessionRow.remoteSessionId` 出线保留 |
