# desktop-workspace-store Specification

## Purpose

定义 workspace 注册表与 agent 运行记录的 native_db 本地持久化契约：Store 打开（含 legacy redb 一次性迁移）与命令面、canonical path 作 key 的 upsert 语义、记录模型与清单排序、native_db 类型边界与单进程约束。

## Requirements

### Requirement: Store 打开与命令面

store crate SHALL 暴露 `Store::open(path: &Path)` 打开（不存在则创建）native_db 数据库文件；db 路径 MUST NOT 在 store crate 内解析（home_dir 依赖 Tauri 上下文），SHALL 由 desktop-app 解析后注入。打开流程 SHALL 先探测文件格式：既有 legacy redb 手写表格式 SHALL 按本能力「legacy redb 库一次性迁移」requirement 自动迁移后以 native_db 打开。store SHALL 提供同步操作面（native_db 构建于 redb 之上，事务/ACID 语义不变，同步调用，无需 async）：

- `add_workspace(root) -> WorkspaceRecord`：canonicalize + upsert
- `list_workspaces() -> Vec<WorkspaceRecord>`：默认序（模型主键 canonical root 自然序升序）
- `remove_workspace(root)`：按 canonical key 删除
- 记录信封查询（`list_models` / `scan`，见「记录信封 API」requirement）

agent 运行相关操作（begin / append / finish / 重放）按 desktop-agent-execution 能力 spec 不变，落库载体随引擎升级平移。MUST NOT 提供「按打开时间刷新/排序」类操作（`touch_workspace` 已移除：sidebar 清单顺序与使用时间无关，不因打开/切换而重排）。

#### Scenario: tempdir 全链路

- **WHEN** 在临时目录打开 Store 依次执行 add / list / remove，并重开同一 db 文件
- **THEN** 各操作返回预期结果，且重开后记录状态与操作结果一致

#### Scenario: db 路径注入

- **WHEN** 审查 store crate 源码
- **THEN** 无 app data 目录解析逻辑，db 文件路径完全来自 `Store::open` 入参

#### Scenario: legacy 格式自动迁移

- **WHEN** `Store::open` 打开一个 legacy redb 手写表格式的存量 db 文件
- **THEN** 迁移自动完成后以 native_db 打开，数据经迁移保留，调用方无感知

### Requirement: canonical path 作 key 的 upsert 语义

WorkspaceRecord 模型 SHALL 以 canonical root path 为 primary key，value SHALL 为记录经 native_model 编码（编码后端由 design 定夺）。`add_workspace` SHALL 即 upsert：同一目录重复添加 MUST NOT 产生第二条记录，原记录 SHALL 原样返回（`added_at` 保留首添值，MUST NOT 刷新任何时间戳）。目录移动 SHALL 视为删旧加新的两次用户操作，store MUST NOT 追踪路径身份连续性（将来需要时再迁 u64 id 方案）。Windows 路径的 canonicalize 口径（UNC `\\?\` 前缀处理、大小写不敏感归一化）SHALL 固定为单一策略并全链路一致：存库 key、去重比较、remove 命中、前端展示同源。

#### Scenario: 等价路径去重

- **WHEN** 对同一目录先后两次 `add_workspace`（路径存在大小写或尾部分隔符等书写差异）
- **THEN** 清单中仅一条记录，第二次调用原样返回首添记录（`added_at` 保留首添值）

#### Scenario: canonical 口径全链路一致

- **WHEN** `add_workspace` 后再以等价路径执行 `remove_workspace`
- **THEN** 命中同一条记录，不产生孤儿条目

### Requirement: WorkspaceRecord 模型与清单排序

`WorkspaceRecord` SHALL 含 `root`（canonical 完整路径）、`name`（目录名最后一段，展示用）、`added_at` 三字段。`list_workspaces` SHALL 按表主键（canonical root）自然序升序返回（默认序）：顺序与添加/打开时间无关、稳定可复现，清单呈现 MUST NOT 因使用而重排。存量库记录中的历史 `lastOpenedAt` 字段在解码时 SHALL 被忽略（serde 默认忽略未知字段），无需数据迁移。

#### Scenario: 默认序与添加顺序无关

- **WHEN** 乱序 add 多个 workspace（如 gamma → alpha → beta）后调用 `list_workspaces`
- **THEN** 返回顺序恒为 canonical root 字典序升序（alpha、beta、gamma），两次调用顺序一致

#### Scenario: name 取目录名最后一段

- **WHEN** `add_workspace("D:\\work\\my-project")`
- **THEN** 记录 `name` 为 `my-project`，`root` 保留完整路径

### Requirement: native_db 类型不泄漏与模型版本治理

store 公共 API SHALL 只暴露自有类型（`WorkspaceRecord` / `AgentRunRecord` / `AgentEventRecord` / `RecordEnvelope` 等）；native_db 与 native_model 类型（`native_db::Database`、`native_model` 派生类型等）MUST NOT 出现在 store 公共签名、desktop-app 命令签名与前端 DTO；legacy 期的 redb 依赖仅限迁移模块内部使用，MUST NOT 出现在迁移模块之外的公共签名。`schema_version` 手工轮账 SHALL 退役：META 表（`user_meta`）随 legacy 迁移删除，模型 shape 演进的版本治理 SHALL 交由 native_model 的类型版本机制承担。db 文件归属（user db，app data dir）SHALL 承载数据维度语义（见 desktop-data-dimensions）。

#### Scenario: 公共 API 无 native_db 类型

- **WHEN** 扫描 store 公共 API、desktop-app 命令签名与前端 `types/dto`
- **THEN** 无任何 native_db / native_model 类型出现，前端 DTO 字段与 store 自有类型一一对应

#### Scenario: schema_version 轮账退役

- **WHEN** 审查 store 源码与迁移后的新格式库
- **THEN** 无 `user_meta` 表与 `schema_version` 写入/校验代码，模型版本经 native_model 版本机制治理

### Requirement: 类型化事件模型 AgentEventRecord

store SHALL 以包装建模类型化 agent 运行事件：包装 struct `AgentEventRecord` 打 native_db derive，嵌装 `agent::AgentEvent` 纯类型作载荷；core `agent` 类型 SHALL 保持 derive-free（native_db/native_model derive 与版本治理全部留在 infra 侧）。主键 SHALL 为 `(run_id, seq)` 复合键（dev-design 前以 spike 验证 native_db 支持度，不支持则退合成键——u128 打包或字符串——并以 `run_id` 二级索引保查询形态）。历史迁移中解不出为 `AgentEvent` 的事件行 SHALL 包成 `AgentEventKind::Raw` 落库（`event_type` 与原文完整保留），MUST NOT 丢弃或迁移失败。

#### Scenario: 嵌装载荷读写回环

- **WHEN** 以含 Message / RunResult / Raw 变体的 `AgentEvent` 序列写入 `AgentEventRecord` 后读出
- **THEN** 读出记录与写入内容一致（含 `seq` / `timestamp_ms` 与变体载荷），事件按 seq 有序可重放

#### Scenario: 旧数据 Raw 兜底

- **WHEN** legacy 迁移读到解不出为 `AgentEvent` 的历史事件行
- **THEN** 该行包成 `Raw` 变体（原文完整保留）写入新库，迁移不报错、不丢行

#### Scenario: core 保持 derive-free

- **WHEN** 审查 `crates/core/agent` 源码与其 Cargo.toml
- **THEN** 无 native_db / native_model derive 与依赖，`AgentEvent` 等类型未为落库目的改动

### Requirement: legacy redb 库一次性迁移

`Store::open` SHALL 在探测到 legacy redb 手写表格式（`user_workspaces` / `user_agent_runs` / `user_agent_run_events` / `user_meta`）时执行一次性迁移：以 read_only 打开旧文件逐表读出 → 事务内写入 native_db 库（事件行解不出落 `Raw`，见「类型化事件模型 AgentEventRecord」）→ 迁移成功后旧文件 SHALL 改名留档（`.bak` 后缀），MUST NOT 删除。迁移 SHALL 原子（单事务或等价机制），失败时旧文件保持原样、打开流程报错。迁移期 store SHALL 同时保留 redb 直依赖（读旧）与 native_db（写新）；该双依赖为过渡态，迁移稳定一个版本后 SHALL 由后续变更收掉 redb。

#### Scenario: 首启自动迁移数据完整

- **WHEN** 以含 workspace / agent run / 事件记录的 legacy fixture 库启动 `Store::open`
- **THEN** 全部记录出现在新库且字段一致（含 `added_at` 等时间戳原值），后续 `list_workspaces` / run 重放与迁移前行为一致

#### Scenario: 损坏行零丢失

- **WHEN** legacy `user_agent_run_events` 中存在解不出的行
- **THEN** 该行以 `Raw` 兜底落库，其余行正常迁移，迁移整体成功

#### Scenario: 旧文件留档

- **WHEN** 迁移成功完成
- **THEN** 原 db 文件以 `.bak` 改名存在于原目录，新库在原路径以 native_db 格式打开

#### Scenario: 迁移失败不破坏旧库

- **WHEN** 迁移过程中发生错误（读旧失败 / 写新失败）
- **THEN** 旧文件保持原样未被改名或截断，`Store::open` 返回错误

### Requirement: 记录信封 API

store SHALL 暴露只读的通用记录信封 API 穿透模型层类型壁垒：

- `list_models() -> Vec<ModelInfo{ name, count }>`：已注册模型清单与记录计数
- `scan(model, offset, limit) -> Vec<RecordEnvelope{ key: Value, value: Value }>`：按模型分页扫描，`key` / `value` 以 JSON Value 呈现

native_db 类型 MUST NOT 越信封（信封值经 serde_json 编解码）；API SHALL 只读，MUST NOT 提供写操作。新注册的模型 SHALL 无需改动信封 API 即可被清单与扫描覆盖（查看器零模型特定代码）。

#### Scenario: 新模型零代码可浏览

- **WHEN** store 新增一个模型注册后调用 `list_models` 与 `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与查看器代码均无改动

#### Scenario: 分页扫描

- **WHEN** 对任一模型以 `scan(model, offset, limit)` 分页请求
- **THEN** 返回对应区间的记录信封（key/value 为 JSON Value），翻页拼接覆盖全量且不重不漏

#### Scenario: 信封 API 只读

- **WHEN** 审查 store 公共 API 与 db 轨道命令
- **THEN** 信封面仅有 list_models / scan 读操作，无任何写入口

### Requirement: 单进程约束显式化

redb 面向单进程嵌入场景，双开（如 dev 与已装正式版指向同一 db 文件）SHALL 列为已知约束：store MUST NOT 自行实现跨进程锁兜底；进程单实例治理（tauri-plugin-single-instance 等）SHALL 不进本变更范围；该约束 SHALL 以代码注释或 crate 文档显式声明。

#### Scenario: 约束声明可见

- **WHEN** 审查 store crate 文档或代码注释
- **THEN** 单进程模型与双开风险被显式说明，且无自研跨进程锁代码

### Requirement: ExploreRecord 模型与 explore 清单操作面

store SHALL 新增 `ExploreRecord` 模型（native_model 新 id，version 1），承载 explore 清单条目与 agent 会话链绑定。字段面 SHALL 至少含：独立主键 `id`（写事务内 max+1 分配，与 `AgentRunRecord` 同语义——身份与文件名解耦，文件改名不破绑定）、`root`（workspace 归属，canonical 口径与 `WorkspaceRecord` 一致）、展示名 `name`、`created_at` / `updated_at`（UTC unix 毫秒，同既有时间戳口径）；链锚与磁盘路径关联字段（`head_run_id` 双写 vs `(source, source_ref)` 派生；路径字段 vs 名字派生）由 dev-design 定夺。维度声明：`ExploreRecord` 归 **user 维度**（registry / 绑定元数据，同 `WorkspaceRecord` 先例落 app data dir user db），内容唯一真源在磁盘 `explore.md`（workspace repo），数据三分——记录（DB）/ 内容（磁盘）/ 对话（`AgentRunRecord` + 事件）。

store SHALL 提供 explore 清单操作面：`list_explore_records(root)`（按 root 过滤）、`create_explore_record`、`delete_explore_record`（删记录 MUST NOT 触碰磁盘文件）。孤儿语义 SHALL 为：磁盘文件被删记录保留（不自动清理，预览空态、下次落盘重建——文件是记录的可丢弃投影）；新模型 SHALL 随既有记录信封 API（`list_models` / `scan`）零改动可浏览。

#### Scenario: 清单按 root 过滤

- **WHEN** store 中存在归属 workspace A 与 B 的 `ExploreRecord`，以 A 的 root 调用 `list_explore_records`
- **THEN** 仅返回归属 A 的记录，两次调用顺序稳定

#### Scenario: 身份与文件名解耦

- **WHEN** 某 `ExploreRecord` 绑定的文件被改名为 `new-name.md`
- **THEN** 记录主键不变、会话链绑定不破；记录更新文件关联后重新可读

#### Scenario: 孤儿保留

- **WHEN** 某 `ExploreRecord` 绑定的磁盘文件被外部删除
- **THEN** 记录仍在清单中；经 `delete_explore_record` 删除记录后磁盘不受影响（文件本已不存在则无操作）

#### Scenario: 信封 API 零改动覆盖

- **WHEN** 新增 `ExploreRecord` 注册后调用 `list_models` / `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与 DB 查看器代码无改动

### Requirement: AgentRunRecord 来源归属与 resume 链字段

`AgentRunRecord` SHALL 以 native_model 版本机制原地演进（version 1 → 2），新增三字段：

- `source: String`：来源受控字符串（`debug` | `explore` | `change` | `archi` | …），缺省 `debug`（既有调试链路写入语义不变）；
- `source_ref: Option<String>`：来源内定位——explore 指向 `ExploreRecord` 主键，change 名等未来来源后续扩展；
- `parent_run_id: Option<i64>`：resume 链显式指针（每次 resume 产生新 run 行，指针指向上游 run；显式指针优于依赖 `session_id` 相等——规避 CLI resume 的 fork 语义歧义）。

既有 v1 记录 SHALL 经 native_model 版本机制自动升级读取（`source` 填缺省 `debug`，其余为 `None`），MUST NOT 要求手工迁移或引入 legacy 迁移层。store SHALL 提供按来源的单链还原查询入口：按 `(source, source_ref)` 检索该来源全部 run（`started_at` 有序），链还原（沿 `parent_run_id` 回溯取整链）SHALL 收口在 store 查询或其直接消费层单点，MUST NOT 由多个前端 hook 各自拼链。事件表（`AgentEventRecord`）SHALL 不动——全局 `run_id` 二级索引已锚定统一 run 身份。

#### Scenario: v1 存量记录自动升级

- **WHEN** 打开含 v1 `AgentRunRecord` 的存量库并重放一条历史 run
- **THEN** 记录可读、事件重放完整，`source` 缺省 `debug`、`source_ref` / `parent_run_id` 为 `None`，无手工迁移步骤

#### Scenario: resume 链还原

- **WHEN** 某 explore 会话链含 run1 → run2 → run3（后者 `parent_run_id` 指向前者）且均携带 `source="explore"` + 同一 `source_ref`
- **THEN** 按 `(source, source_ref)` 查询返回三行且按发起顺序排列；沿 `parent_run_id` 可完整回溯链，`session_id` 与 `parent_run_id` 口径一致

#### Scenario: 事件重放不受演进影响

- **WHEN** 对 v2 演进后的 run 调用 `agent_run_events` 重放
- **THEN** 事件序列完整（seq 升序），与演进前行为一致（事件表结构未变）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store`（crate 名 `store`） | native_db 本地库，db 边界第一成员 | `Store::open(path)` 注入式打开 + legacy 探测迁移；workspace 三操作 + 信封 API；公共 API 仅自有类型；零 Tauri；仅依赖 `agent` 纯类型作嵌装载荷；迁移期保留 redb（迁移模块内部）；explore 清单操作（list/create/delete）与 `(source, source_ref)` 链还原查询单点收口 |
| `WorkspaceRecord`（模型） | workspace 注册记录 | PK = canonical root；root / name / added_at；native_model 编码 |
| `ExploreRecord`（模型） | explore 清单与绑定记录 | native_model 新 id；PK 独立自增 id；root / name / created_at / updated_at；user 维度声明；孤儿保留 |
| `AgentRunRecord`（模型，v2） | agent 运行元数据 | PK = id（自增语义不变）；+ `source`（缺省 `debug`）/ `source_ref` / `parent_run_id`；native_model 版本原地演进，无迁移层 |
| `AgentEventRecord`（模型，新） | 类型化事件记录 | PK = `(run_id, seq)`（spike 裁定，退路合成键）；二级索引 `run_id`；嵌装 `agent::AgentEvent`（含 Raw 逃生舱）；core 类型 derive-free |
| legacy 迁移模块（新） | 一次性迁移 | read_only 读旧 redb → 事务写 native_db → 旧文件 `.bak` 留档；失败不破坏旧库；过渡期后收 redb |
| 记录信封 API（新） | 模型层通用读面 | `list_models()` / `scan(model, offset, limit)`；JSON Value 信封；只读；新模型注册即覆盖 |
| user db 文件（app data dir） | user 维度落盘载体 | 维度语义由 db 文件归属承载（user 前缀表名随引擎退役，见 desktop-data-dimensions） |
