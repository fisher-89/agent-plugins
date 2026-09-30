# desktop-workspace-store Specification

## Purpose

定义 workspace 注册表与 agent 运行记录的 native_db 本地持久化契约：全局库与 workspace 库双库布局、Store 打开（含 legacy redb 一次性迁移）与命令面、canonical path 作 key 的 upsert 语义、记录模型与清单排序、native_db 类型边界与单进程约束。

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

store SHALL 以 `ExploreRecord` 模型（native_model id / version 不变）承载 explore 清单条目与 agent 会话链绑定。字段面 SHALL 至少含：独立主键 `id`（写事务内 max+1 分配，与 `AgentRunRecord` 同语义——身份与文件名解耦，文件改名不破绑定）、`root`（workspace 归属，canonical 口径与 `WorkspaceRecord` 一致）、展示名 `name`、`created_at` / `updated_at`（UTC unix 毫秒，同既有时间戳口径）；链锚与磁盘路径关联字段（`head_run_id` 双写 vs `(source, source_ref)` 派生；路径字段 vs 名字派生）由 dev-design 定夺。维度声明：`ExploreRecord` 归 **workspace 维度**（workspace 数据分离裁定，落所属 workspace 的独立 db 文件，见 desktop-data-dimensions），内容唯一真源在磁盘 `explore.md`（workspace repo），数据三分——记录（workspace 库）/ 内容（磁盘）/ 对话（同 workspace 库的 `AgentRunRecord` + 事件）。

store SHALL 提供 explore 清单操作面：`list_explore_records(root)`（按 root 过滤）、`create_explore_record`、`delete_explore_record`（删记录 MUST NOT 触碰磁盘文件；记录名下的会话 runs 及其事件 SHALL 随记录**同事务级联删除**——按 `(source="explore", source_ref=记录id)` 圈定。id 为幸存行上 max+1 的可复用计数，级联清理是悬空 `source_ref` 不被复用 id 错挂的引用完整性保证；MUST NOT 波及 debug 来源或其他 `source_ref` 的 runs）。孤儿语义 SHALL 为：磁盘文件被删记录保留（不自动清理，预览空态、下次落盘重建——文件是记录的可丢弃投影）；新模型 SHALL 随既有记录信封 API（`list_models` / `scan`）零改动可浏览。级联删除与链还原 SHALL 天然收敛于同一 workspace 库实例内（记录与名下 runs / 事件同库，无跨库引用）。

#### Scenario: 清单按 root 过滤

- **WHEN** store 中存在归属 workspace A 与 B 的 `ExploreRecord`，以 A 的 root 调用 `list_explore_records`
- **THEN** 仅返回归属 A 的记录，两次调用顺序稳定

#### Scenario: 身份与文件名解耦

- **WHEN** 某 `ExploreRecord` 绑定的文件被改名为 `new-name.md`
- **THEN** 记录主键不变、会话链绑定不破；记录更新文件关联后重新可读

#### Scenario: 孤儿保留

- **WHEN** 某 `ExploreRecord` 绑定的磁盘文件被外部删除
- **THEN** 记录仍在清单中；经 `delete_explore_record` 删除记录后磁盘不受影响（文件本已不存在则无操作）

#### Scenario: 删除级联清对话且 id 复用不错链

- **WHEN** 某 `ExploreRecord`（id=N）名下已有 `source="explore"`、`source_ref="N"` 的 runs 及事件，执行 `delete_explore_record` 后再新建记录（幸存行 max+1 复用 id=N）
- **THEN** 被删记录名下 runs 与事件已同事务消失（`(source, source_ref)` 链还原为空），新记录链还原为空不捞旧聊天；debug 来源及指向其他记录 id 的 runs 及其事件原样存活

#### Scenario: 信封 API 零改动覆盖

- **WHEN** 新增 `ExploreRecord` 注册后调用 `list_models` / `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与 DB 查看器代码无改动

#### Scenario: workspace 库落位隔离

- **WHEN** 审查 explore 记录的落盘位置
- **THEN** 记录与名下会话 runs / 事件位于所属 workspace 的独立 db 文件，全局库与 workspace repo 内无 explore 记录数据

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

### Requirement: AgentRunRecord 状态字段类型化

`AgentRunRecord` 的 `env` / `permission_mode` / `status` 三字段 SHALL 类型化为 core/agent 枚举（`AgentEnvMode` / `AgentPermissionMode` / 新增 `AgentRunStatus`），MUST NOT 再以 `String` 承载受控值域；「受控字符串」模型约定（store 不引本地枚举）SHALL 退役。`AgentRunStatus` SHALL 定义于 `crates/core/agent`（与既有两枚举同列），变体为 `Running` / `Completed` / `Failed` / `Stopped`（camelCase serde rename 与现值域逐字一致）。

serde camelCase 线格式 SHALL 与枚举化前逐字不变（unit variant + `rename_all = "camelCase"`：`AcceptEdits` → `"acceptEdits"` 等）；落库编码形态若因枚举化变化（bincode 的 String 长度前缀与变体索引不等价，变化必然发生），SHALL 经 native_model 版本机制原地演进（`AgentRunRecord` 版本递增，`from` 枚举化前旧形态自动升级），MUST NOT 引入手工迁移步骤或 legacy 迁移层。枚举化前 SHALL 先完成存量库三字段值域扫描，确认无野值；转换遇野值的处理策略（报错 vs 兜底）由 design 定夺。`source` / `source_ref` SHALL 保持 `String`（两侧均为 string，不构成类型降级；收紧另开 change）。

落库写入路径 MUST NOT 再经 `as_str().to_owned()` 字符串降级（直写枚举）；`as_str` 的「与 serde 线格式一致」双轨口径 SHALL 退役。CLI flag 组装等真需字符串值处 SHALL 经单一来源取得，取值行为不变（design 定夺实现形态）。

#### Scenario: 线格式逐字不变

- **WHEN** 对枚举化后的 `AgentRunRecord` 做 serde JSON 序列化并与枚举化前值域比对
- **THEN** `env` / `permission_mode` / `status` 输出逐字一致（`"default"` / `"acceptEdits"` / `"running"` 等），IPC 与前端所见值域不变

#### Scenario: 存量记录自动升级

- **WHEN** 打开含枚举化前形态 `AgentRunRecord` 的存量库并重放历史 run
- **THEN** 记录经 native_model 版本机制自动升级可读，三字段值域正确还原，无手工迁移步骤、无 legacy 迁移层

#### Scenario: 存量值域扫描先行

- **WHEN** 执行枚举化落地
- **THEN** 存量库 `env` / `permission_mode` / `status` 三字段值域扫描先行完成并确认无野值，扫描结论留档

#### Scenario: 写入不再字符串降级

- **WHEN** 审查 run begin / finish 的落库写入路径
- **THEN** 三字段直写枚举，无 `as_str().to_owned()` 降级；`STATUS_RUNNING` 等裸字符串常量随之退役

#### Scenario: 前端 union 不降级

- **WHEN** 审查生成物中 `AgentRunRecord` 的 `status` / `env` / `permissionMode` 类型
- **THEN** 为字面量 union（`'running' | 'completed' | 'failed' | 'stopped'` 等），非 `string`——Rust 枚举成为单一事实源，TS 侧类型强度不降

### Requirement: 全局库与 workspace 库双库布局

store SHALL 按数据维度拆分落盘载体为两组 db 文件：**全局库**仅注册 `WorkspaceRecord`（user 维度注册表，含未来 user 维度租户）；**workspace 库**按 workspace 分立、每 workspace 恰一个独立 db 文件，注册 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 三个 workspace 维度模型。全局库 SHALL 位于全局数据目录（沿用 `home_dir()/.dev-team/` 根）并启用与旧单库不同的新文件名；workspace 库 SHALL 位于全局数据目录下 `workspaces/` 子树。任一库的模型归属 MUST NOT 混注：workspace 域模型 MUST NOT 出现在全局库注册清单，user 维度模型 MUST NOT 出现在 workspace 库注册清单（维度语义由 db 文件归属承载，见 desktop-data-dimensions）。数据目录根 SHALL 由 desktop-app 注入，store 内零环境解析（既有 `Store::open` 注入式打开约束不变）；静态模型注册 SHALL 拆为与两库一一对应的两组 `Models`。

#### Scenario: 模型注册分组

- **WHEN** 审查 store 的静态模型注册与两组 `Models` 的消费方
- **THEN** 全局库组仅定义 `WorkspaceRecord`，workspace 库组仅定义 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`，无交叉注册

#### Scenario: 数据分流写入

- **WHEN** `add_workspace` 注册目录后，对该 workspace 执行 `create_explore_record` 与 `begin_agent_run`
- **THEN** 注册记录落在全局库，explore 记录与 run 记录落在该 workspace 库，全局库无 workspace 域记录混入

#### Scenario: 注入式打开不变

- **WHEN** 审查 store 公共 API 与打开流程
- **THEN** 两库路径最终均来自调用方注入（数据目录根由 desktop-app 解析），store 源码无 home_dir / app data 目录解析逻辑

### Requirement: workspace 库文件确定性寻址

workspace 库文件路径 SHALL 由 canonical root 经**单点纯函数**确定性派生：同根恒同名（跨重启、跨进程可复现），异根必不同名（哈希成分抗碰撞），文件名 MUST NOT 含路径分隔符或 OS 非法字符。派生函数 SHALL 是 workspace 库文件名的唯一组装点，消费侧（desktop-app 命令层、前端）MUST NOT 出现派生逻辑或路径字符串拼接。哈希算法与文件名组成（可读目录段 + 哈希长度）由 dev-design 定夺；派生入参 SHALL 为已 canonical 化的 root 与注入的数据目录根，复用 store 既有 canonical 口径（UNC 前缀、大小写归一策略同源）。

#### Scenario: 同根跨重开同路径

- **WHEN** 对同一 canonical root 先后两次（重启之间）调用路径派生
- **THEN** 两次返回完全相同的文件路径；对两个仅大小写或尾部分隔符不同的等价路径派生，结果亦一致（canonical 口径归一在前）

#### Scenario: 异根不同名

- **WHEN** 对两个不同 workspace root 调用路径派生
- **THEN** 得到不同文件路径，不存在碰撞覆盖

### Requirement: workspace store 注册表与进程内单开

store SHALL 提供两级解析入口：全局注册操作（`add_workspace` / `list_workspaces` / `remove_workspace` 等）走全局库实例；workspace 域操作（run / 事件 / explore 记录面）经 `for_root(root)` 解析对应 workspace 库实例。同一 workspace db 文件在进程内 MUST NOT 重复打开（redb/native_db 单文件写锁硬约束）：注册表 SHALL 按 root 缓存并复用已打开实例；不同 root 各自独立实例。`Store` 类型与 `Store::open(path)` 单文件句柄语义不变；注册表类型（挂 Tauri State）SHALL 由 store crate 提供、desktop-app 挂载。`add_workspace` SHALL 在注册成功后预开对应 workspace 库（坏文件注册时即报错，fail fast 口径同 setup，不留给首个业务命令）。`remove_workspace` SHALL 仅删注册记录，workspace db 文件 MUST NOT 随之删除或改名（移除注册 ≠ 销毁历史；重新添加同 root 时清单与会话链历史完整恢复）；孤儿文件治理留待二期。缓存淘汰策略（进程生命周期常开 vs LRU）由 dev-design 定夺，默认进程生命周期常开。

#### Scenario: 同 root 实例复用

- **WHEN** 对同一 workspace root 连续多次发起 workspace 域命令
- **THEN** 均复用同一打开实例，不产生对同一 db 文件的第二次打开（无文件锁冲突）

#### Scenario: 异 root 独立实例

- **WHEN** 依次对 workspace A 与 B 发起 workspace 域命令
- **THEN** 两库各自独立打开、数据互不可见（A 的 runs / explore 清单不出现在 B 的任何查询结果中）

#### Scenario: 注册时预开校验

- **WHEN** `add_workspace` 选定一个 workspace 库文件已损坏（不可打开）的目录
- **THEN** 注册返回 `Err`，损坏文件在注册时暴露；健康目录注册成功且库文件就绪

#### Scenario: remove 后文件保留

- **WHEN** `remove_workspace` 移除某注册项后重新 `add_workspace` 同一根目录
- **THEN** remove 后 workspace db 文件仍存在于原位；重新添加后 explore 清单与会话链历史完整可读

### Requirement: 全新文件组冷启动与旧库惰性废弃

存量单库（四模型混居的旧 `desktop-store.redb`）SHALL **零迁移**处置（用户裁定，2026-09-29：无需考虑兼容历史数据）：新布局以全新文件组冷启动，应用启动与运行 MUST NOT 读取旧单库文件，MUST NOT 对其改名、删除或做任何格式探测；store MUST NOT 引入拆分搬移、格式探测或迁移层代码（与既有「不做旧库兼容」立场同轨）。旧文件作为磁盘惰性残留保持原样；新布局首启后全局库与 workspace 库均从空开始写入（workspace 清单从空开始，由用户重新添加目录重建）。

#### Scenario: 旧库存在时冷启动

- **WHEN** 全局数据目录下存在旧四模型单库文件，以新布局启动应用
- **THEN** 启动照常成功，旧文件字节原样未动（未读、未改名、未删除），新库文件从空开始按新布局写入

#### Scenario: 零迁移代码

- **WHEN** 审查 store 源码
- **THEN** 无拆分搬移、格式探测、迁移模块或旧格式读取代码路径

### Requirement: workspace 域命令 root 寻址解析入口

workspace 域命令（run / 事件 / explore 记录面）SHALL 携 root 寻址：命令层以 root 调 `for_root` 解析所属 workspace 库后调用 store 操作，store 操作面签名不变（实例即上下文）。blank root 纪律不变（空/空白 root 不进入解析与 store 链路，直接空结果语义）。全局轨命令（workspace 注册三命令）SHALL 经全局库实例执行，MUST NOT 误路由至 workspace 库。db 查看命令的 scope 寻址见 desktop-db-inspector 能力。

#### Scenario: 命令路由正确

- **WHEN** 分别以 workspace A 与 B 的 root 调用 `list_explore_records` 与 `agent_runs`
- **THEN** 各自返回对应 workspace 库的数据，路由无串库

#### Scenario: blank root 直达空结果

- **WHEN** 以空白 root 调用任一 workspace 域命令
- **THEN** 不触发库解析与打开，直接返回空结果语义（既有口径不变）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store`（crate 名 `store`） | native_db 本地库，db 边界第一成员 | 双库布局：全局库（`WorkspaceRecord`）+ `workspaces/` 子树 per-workspace 库（`AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`）；`Store::open(path)` 注入式打开不变；数据目录根注入、零环境解析；零迁移（旧单库惰性废弃） |
| 静态模型注册（拆两组） | 两库各自的 `Models` | 全局组仅 `WorkspaceRecord`；workspace 组仅 run / 事件 / explore 三模型；模型 id / version / 字段面全部不变 |
| workspace 库路径派生（新，单点） | canonical root → db 文件路径 | 纯函数、确定性、抗碰撞、无路径成分；store 内唯一组装点，消费侧零派生逻辑 |
| `WorkspaceStores`（新） | 两级解析与实例缓存 | `global()` → 全局库；`for_root(root)` → workspace 库（per-root 缓存复用，进程内单开）；挂 Tauri State；`add_workspace` 预开校验；`remove_workspace` 文件保留 |
| `WorkspaceRecord`（模型） | workspace 注册记录（user 维度） | PK = canonical root；root / name / added_at；落全局库 |
| `ExploreRecord`（模型） | explore 清单与绑定记录（workspace 维度） | 落所属 workspace 库；PK 独立自增 id；级联删除与链还原同库收敛；孤儿保留；其余语义不变 |
| `AgentRunRecord`（模型，v2） | agent 运行元数据（workspace 维度） | PK = id（workspace 库域内自增）；+ `source`（缺省 `debug`）/ `source_ref` / `parent_run_id`；native_model 版本原地演进，无迁移层 |
| `AgentEventRecord`（模型，新） | 类型化事件记录（workspace 维度） | PK = `(run_id, seq)`（spike 裁定，退路合成键）；二级索引 `run_id`；嵌装 `agent::AgentEvent`（含 Raw 逃生舱）；core 类型 derive-free |
| legacy 迁移模块（新） | 一次性迁移 | read_only 读旧 redb → 事务写 native_db → 旧文件 `.bak` 留档；失败不破坏旧库；过渡期后收 redb |
| 记录信封 API（新） | 模型层通用读面 | `list_models()` / `scan(model, offset, limit)` 按实例生效（每库各自清单与计数）；只读；新模型注册即覆盖 |
| `crates/core/agent/src/runner.rs`（`AgentRunStatus` 新增） | run 状态枚举契约 | `Running` / `Completed` / `Failed` / `Stopped` 四变体；serde camelCase 与现值域逐字一致；与 `AgentEnvMode` / `AgentPermissionMode` 同列承载 `specta::Type` |
| `crates/infra/store/src/model.rs`（`AgentRunRecord`） | 三字段类型化 + 版本演进 | `env` / `permission_mode` / `status` String → 枚举；native_model 版本原地演进（from 旧形态自动升级）；`source` / `source_ref` 保持 String；无迁移层 |
| 落库写入路径（`commands/exec` 编排） | 写入类型化 | 直写枚举，`as_str().to_owned()` 降级与 `STATUS_*` 裸常量退役；flag 组装字符串值单一来源（行为不变） |
