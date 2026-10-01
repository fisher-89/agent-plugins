# desktop-workspace-store Delta

## ADDED Requirements

### Requirement: SessionRecord 模型与会话转录单表

store SHALL 以新模型 `SessionRecord`（workspace 维度，落所属 workspace 的独立 db 文件，native_model 分配新 id）承载 agent 会话一等公民：

- `id`：core 铸的会话 id（字符串主键；native_db 对字符串主键的支撑度由 design spike 核实，不支持则退合成数值 id + 会话 id 唯一二级索引，查询形态不变）
- `engine_session_id`：引擎侧句柄（cli: claude session id；sdk: 可缺省），与 core session id 构成双 id 映射的落库半边
- `config_snapshot`：装配配置快照（engine / model / 权限档）——SHALL 存快照而非跨库引用（`AgentInstanceRecord` 在全局库、session 在 workspace 库，store 惯例禁跨库引用；实例改名/删除不伤历史会话）
- `source` / `source_ref`：来源归属（`debug` | `explore` | …，缺省 `debug`；explore 指向 `ExploreRecord` 主键）——会话化后来源圈定的主归属
- `created_at` / `updated_at`（UTC unix 毫秒，同既有时间戳口径）

会话数据 SHALL 收敛为「转录单表 + 轮统计行」：密封事件**直挂 session**（单表，承载模型与键位见「类型化事件模型 AgentEventRecord」delta）；run 退化为**轮统计行**（每轮一行，挂 session 外键，字段面为 TurnDone 统计口径与时间戳），MUST NOT 再以「单 run 事件 + 链指针」承载会话历史。会话模型 SHALL 随记录信封 API（`list_models` / `scan`）零改动可浏览。

#### Scenario: 会话回环读写

- **WHEN** 建立会话（含 config_snapshot 与来源归属）、追加密封转录与轮统计行后重开同一 db 文件查询
- **THEN** 会话记录、转录与统计行与写入一致（含双 id 映射与时间戳原值）

#### Scenario: 配置快照隔离

- **WHEN** 某会话建立后，其来源 agent 实例在全局库被改名或删除
- **THEN** 会话记录与 config_snapshot 原样可读（快照非引用），历史会话不受影响

#### Scenario: 转录挂 session 与轮统计行

- **WHEN** 审查一个三轮会话的落库形态
- **THEN** 密封事件以 session 为键成单表全史，轮统计行恰三行且各挂该 session，无「按 run 圈定事件」的旧形态残留

#### Scenario: 存量库 additive 打开

- **WHEN** 在含存量 workspace 域记录的 workspace 库上以新版本打开
- **THEN** 打开成功，存量记录原样可读，`SessionRecord` 可立即写入读出；store 无迁移代码路径（存量数据处置口径见提案待决问题）

### Requirement: 会话查询与对账 API

store SHALL 经 workspace 库实例提供会话域查询面（core 查询契约的 infra 实现与命令面直查的共用底座）：

- **会话清单**：按 root（实例即上下文）与来源（`source` / `source_ref`）过滤，排序稳定可复现；每会话附聚合统计——轮数、累计墙钟、累计 token **从轮统计行现算**，MUST NOT 维护累计列；统计字段缺席合法（缺省 `None`，降级不违约）
- **会话转录重放**：按会话返回全史密封事件（seq 有序），MUST NOT 要求运行进程存活
- **对账纠偏重导**：SHALL 为显式调用入口（core 查询路径或维护命令触发）——从转录重建统计/索引作误差校正，非主路径；MUST NOT 隐式挂在读路径，MUST NOT 提供绕过 write-through 的常规写入口

#### Scenario: 清单过滤与统计现算

- **WHEN** 某 workspace 库含 debug 与 explore 两来源会话（各若干轮）后按来源查询清单
- **THEN** 仅返回对应来源会话，聚合统计（轮数/累计墙钟/累计 token）与轮统计行现算一致；无 token 口径的轮按缺省计（不报错）

#### Scenario: 转录重放全史

- **WHEN** 对一个三轮会话调用转录重放
- **THEN** 返回三轮全部密封事件（seq 升序、无 delta 记录），应用重启后结果一致

#### Scenario: 对账纠偏收敛

- **WHEN** 轮统计行与转录可重算口径出现偏差后触发对账重导
- **THEN** 统计/索引从转录重建收敛，重导不改动密封转录本身（转录不可变）

## MODIFIED Requirements

### Requirement: Store 打开与命令面

store crate SHALL 暴露 `Store::open(path: &Path)` 打开（不存在则创建）native_db 数据库文件；db 路径 MUST NOT 在 store crate 内解析（home_dir 依赖 Tauri 上下文），SHALL 由 desktop-app 解析后注入。打开流程 SHALL 先探测文件格式：既有 legacy redb 手写表格式 SHALL 按本能力「legacy redb 库一次性迁移」requirement 自动迁移后以 native_db 打开。store SHALL 提供同步操作面（native_db 构建于 redb 之上，事务/ACID 语义不变，同步调用，无需 async）：

- `add_workspace(root) -> WorkspaceRecord`：canonicalize + upsert
- `list_workspaces() -> Vec<WorkspaceRecord>`：默认序（模型主键 canonical root 自然序升序）
- `remove_workspace(root)`：按 canonical key 删除
- 记录信封查询（`list_models` / `scan`，见「记录信封 API」requirement）

agent 会话域操作（会话建立 / 密封追加 / 轮统计收尾 / 会话查询与对账）按 desktop-agent-execution 能力 delta 演进为 write-through 原子操作面，落库载体随会话化平移（见「SessionRecord 模型与会话转录单表」）。MUST NOT 提供「按打开时间刷新/排序」类操作（`touch_workspace` 已移除：sidebar 清单顺序与使用时间无关，不因打开/切换而重排）。

#### Scenario: tempdir 全链路

- **WHEN** 在临时目录打开 Store 依次执行 add / list / remove，并重开同一 db 文件
- **THEN** 各操作返回预期结果，且重开后记录状态与操作结果一致

#### Scenario: db 路径注入

- **WHEN** 审查 store crate 源码
- **THEN** 无 app data 目录解析逻辑，db 文件路径完全来自 `Store::open` 入参

#### Scenario: legacy 格式自动迁移

- **WHEN** `Store::open` 打开一个 legacy redb 手写表格式的存量 db 文件
- **THEN** 迁移自动完成后以 native_db 打开，数据经迁移保留，调用方无感知

### Requirement: 类型化事件模型 AgentEventRecord

store SHALL 以包装建模类型化 agent 会话事件：包装 struct（承载模型沿 `AgentEventRecord` 形态或按会话化更名，由 design 定稿）打 native_db derive，嵌装 `agent::AgentEvent` 纯类型作载荷（**密封事件 only**：`MessageDelta` 永不落库）；core `agent` 类型 SHALL 保持 derive-free（native_db/native_model derive 与版本治理全部留在 infra 侧）。会话化后主键 SHALL 以 **session 为圈定单位**（转录单表：session id + seq 的复合键——形态由 design spike 验证 native_db 支持度，不支持则退合成键并以 session id 二级索引保查询形态）；轮统计行（run）与转录的归属经 session 外键关联。会话化演进中既有事件行的处置（随零迁移冷启动废弃，或演进挂接）由 design 裁定（提案待决问题）；挂接路径下解不出为密封 `AgentEvent` 的事件行 SHALL 包成 `AgentEventKind::Raw` 落库（`event_type` 与原文完整保留），MUST NOT 丢弃或迁移失败。

#### Scenario: 嵌装载荷读写回环

- **WHEN** 以含 Message / SystemNotice / Raw 变体的密封 `AgentEvent` 序列按 session 写入后读出
- **THEN** 读出记录与写入内容一致（含 `seq` / `timestamp_ms` 与变体载荷），事件按 seq 有序可重放且同会话全史连续

#### Scenario: delta 零落库

- **WHEN** 以含 `MessageDelta` 的事件序列尝试走 store sink
- **THEN** sink 拒绝/忽略 delta（不产生记录），仅密封事件落库——store 中不存在任何 delta 行

#### Scenario: core 保持 derive-free

- **WHEN** 审查 `crates/core/agent` 源码与其 Cargo.toml
- **THEN** 无 native_db / native_model derive 与依赖，`AgentEvent` 等类型未为落库目的改动

### Requirement: AgentRunRecord 来源归属与 resume 链字段

会话一等公民落地后，`AgentRunRecord` SHALL 退化为**轮统计行**（挂 session 外键；字段面 = TurnDone 统计口径 + 起止时间戳 + 状态），以 native_model 版本机制原地演进：

- 来源归属主平移：`source` / `source_ref` 的圈定主归属移至 `SessionRecord`（会话级来源，见「SessionRecord 模型与会话转录单表」）；轮统计行 SHALL 携所属 session 引用，其自身 `source` 系字段随会话化退役或保留为冗余投影由 design 定稿；
- 链指针语义取代：`parent_run_id` 的 resume 链指针语义由**会话归属**取代（同 session 的轮序列即链；`started_at` 有序），是否保留为轮序指针由 design 定稿。

既有记录 SHALL 经 native_model 版本机制自动升级读取（无会话归属的历史 run 行按缺省处置——处置策略随存量数据裁定落 design），MUST NOT 要求手工迁移或引入 legacy 迁移层。单链还原 SHALL 收敛为会话转录重放（store 查询单点），MUST NOT 由多个前端 hook 各自按 run 事件拼链。

#### Scenario: 既有记录自动升级

- **WHEN** 打开含会话化前形态 `AgentRunRecord` 的存量库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，无手工迁移步骤、无 legacy 迁移层（处置策略见 design 裁定）

#### Scenario: 会话归属还原

- **WHEN** 某 explore 会话含三轮（三行轮统计行同属一个 session）
- **THEN** 按会话归属（source + source_ref → session）查询返回该会话及其三轮统计行（发起顺序排列），转录重放给出全史

#### Scenario: 事件重放走会话转录

- **WHEN** 对会话化后的任一会话调用转录重放
- **THEN** 密封事件序列完整（seq 升序），不再经 run 事件表拼链

### Requirement: ExploreRecord 模型与 explore 清单操作面

store SHALL 以 `ExploreRecord` 模型（native_model id / version 不变）承载 explore 清单条目与 agent 会话绑定。字段面 SHALL 至少含：独立主键 `id`（写事务内 max+1 分配）、`root`（workspace 归属，canonical 口径与 `WorkspaceRecord` 一致）、展示名 `name`、`created_at` / `updated_at`（UTC unix 毫秒，同既有时间戳口径）；链锚与磁盘路径关联字段（`head_run_id` 双写 vs `(source, source_ref)` 派生；路径字段 vs 名字派生）由 dev-design 定稿（会话化后链锚语义收敛为会话归属）。维度声明：`ExploreRecord` 归 **workspace 维度**（workspace 数据分离裁定，落所属 workspace 的独立 db 文件，见 desktop-data-dimensions），内容唯一真源在磁盘 `explore.md`（workspace repo），数据三分——记录（workspace 库）/ 内容（磁盘）/ 对话（同 workspace 库的会话转录 + 轮统计行）。

store SHALL 提供 explore 清单操作面：`list_explore_records(root)`（按 root 过滤）、`create_explore_record`、`delete_explore_record`（删记录 MUST NOT 触碰磁盘文件；记录名下的**会话及其转录与轮统计行** SHALL 随记录**同事务级联删除**——按会话归属 `(source="explore", source_ref=记录id)` 圈定。id 为幸存行上 max+1 的可复用计数，级联清理是悬空 `source_ref` 不被复用 id 错挂的引用完整性保证；MUST NOT 波及 debug 来源或其他 `source_ref` 的会话）。孤儿语义 SHALL 为：磁盘文件被删记录保留（不自动清理，预览空态、下次落盘重建——文件是记录的可丢弃投影）；新模型 SHALL 随既有记录信封 API（`list_models` / `scan`）零改动可浏览。级联删除与会话还原 SHALL 天然收敛于同一 workspace 库实例内（记录与会话同库，无跨库引用）。

#### Scenario: 清单按 root 过滤

- **WHEN** store 中存在归属 workspace A 与 B 的 `ExploreRecord`，以 A 的 root 调用 `list_explore_records`
- **THEN** 仅返回归属 A 的记录，两次调用顺序稳定

#### Scenario: 身份与文件名解耦

- **WHEN** 某 `ExploreRecord` 绑定的文件被改名为 `new-name.md`
- **THEN** 记录主键不变、会话绑定不破；记录更新文件关联后重新可读

#### Scenario: 孤儿保留

- **WHEN** 某 `ExploreRecord` 绑定的磁盘文件被外部删除
- **THEN** 记录仍在清单中；经 `delete_explore_record` 删除记录后磁盘不受影响（文件本已不存在则无操作）

#### Scenario: 删除级联清会话且 id 复用不错链

- **WHEN** 某 `ExploreRecord`（id=N）名下已有 `source="explore"`、`source_ref="N"` 的会话（含转录与轮统计行），执行 `delete_explore_record` 后再新建记录（幸存行 max+1 复用 id=N）
- **THEN** 被删记录名下会话及其转录/统计行已同事务消失（按会话归属圈定），新记录名下会话还原为空不捞旧聊天；debug 来源及指向其他记录 id 的会话原样存活

#### Scenario: 信封 API 零改动覆盖

- **WHEN** `SessionRecord` 注册后调用 `list_models` 与 `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与 DB 查看器代码无改动

#### Scenario: workspace 库落位隔离

- **WHEN** 审查 explore 记录与会话的落盘位置
- **THEN** 记录与名下会话转录/统计行位于所属 workspace 的独立 db 文件，全局库与 workspace repo 内无此类数据

### Requirement: 全局库与 workspace 库双库布局

store SHALL 按数据维度拆分落盘载体为两组 db 文件：**全局库**注册 user 维度模型组（`WorkspaceRecord` 与 agent 管理两记录 `AgentProviderRecord` / `AgentInstanceRecord`，见「AgentProviderRecord 与 AgentInstanceRecord 模型与管理操作面」）；**workspace 库**按 workspace 分立、每 workspace 恰一个独立 db 文件，注册 `AgentRunRecord`（轮统计行）/ 事件记录（会话转录）/ `ExploreRecord` / `SessionRecord` 四个 workspace 维度模型。全局库 SHALL 位于全局数据目录（沿用 `home_dir()/.dev-team/` 根）并启用与旧单库不同的新文件名；workspace 库 SHALL 位于全局数据目录下 `workspaces/` 子树。任一库的模型归属 MUST NOT 混注：workspace 域模型 MUST NOT 出现在全局库注册清单，user 维度模型 MUST NOT 出现在 workspace 库注册清单（维度语义由 db 文件归属承载，见 desktop-data-dimensions）。数据目录根 SHALL 由 desktop-app 注入，store 内零环境解析（既有 `Store::open` 注入式打开约束不变）；静态模型注册 SHALL 拆为与两库一一对应的两组 `Models`。

#### Scenario: 模型注册分组

- **WHEN** 审查 store 的静态模型注册与两组 `Models` 的消费方
- **THEN** 全局库组定义 `WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord`，workspace 库组仅定义 `AgentRunRecord` / 会话转录事件记录 / `ExploreRecord` / `SessionRecord`，无交叉注册

#### Scenario: 数据分流写入

- **WHEN** `add_workspace` 注册目录后，对该 workspace 发起会话运行并在管理页新建 provider 与 agent
- **THEN** 注册记录与 provider / agent 记录落在全局库，会话/转录/轮统计行落在该 workspace 库，全局库无 workspace 域记录混入

#### Scenario: 注入式打开不变

- **WHEN** 审查 store 公共 API 与打开流程
- **THEN** 两库路径最终均来自调用方注入（数据目录根由 desktop-app 解析），store 源码无 home_dir / app data 目录解析逻辑

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs` | 模型面 | `SessionRecord`（新，workspace 维度：core 铸 id / engine_session_id / config_snapshot / source / source_ref / 时间戳）；`AgentRunRecord` 轮统计行化（native_model 版本演进）；事件记录 session 键化（spike 定形态）；core 类型 derive-free 不变 |
| `crates/infra/store/src/store.rs` | 操作面 | 会话域 write-through 原子操作（会话建立/密封追加/轮收尾）；旧 begin/append/finish 平移退役；静态模型注册 workspace 组增 `SessionRecord` |
| 会话查询与对账面（新） | core 查询契约的 infra 实现 | 会话清单（来源过滤 + 聚合现算 + 缺省合法）；转录重放（全史 seq 序）；对账重导（显式入口、误差校正、不动转录） |
| 记录信封 API | 模型层通用读面 | `SessionRecord` 注册即覆盖，零改动可浏览 |
| `ExploreRecord` 级联 | 引用完整性 | 级联圈定从 runs 平移至会话（转录 + 轮统计行同事务删）；`source_ref` 复用不错链 |
| 双库布局 | 维度承载 | workspace 库组四模型；无交叉注册；注入式打开与零迁移纪律不变 |
