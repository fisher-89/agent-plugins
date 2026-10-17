# desktop-workspace-store Specification (Delta)

## MODIFIED Requirements

### Requirement: ExploreRecord 模型与 explore 清单操作面

store SHALL 以 `ExploreRecord` 模型承载 explore 清单条目与 agent 会话绑定。字段面 SHALL 含：独立主键 `id`（写事务内 max+1 分配）、`root`（workspace 归属，canonical 口径与 `WorkspaceRecord` 一致）、`name`（笔记文件 stem 寻址键，kebab-case 口径）、`title`（人类可读标题，恒非空——创建与升级默认 = name（占位建档 = 临时标题，见操作面），agent 产出标题经 `set_explore_title` 回填覆盖）、`workflow_type`（工作流类型，合法值 `requirement` | `test-only`——创建与升级默认 = `requirement`，agent 推算类型经 `set_explore_workflow_type` 回填覆盖）、`promoted_to`（`Option<String>`，指向已 promote 的 **change id** 身份锚，None = 草稿态）、`session_id`（`Option<String>`，归属 agent 会话 id——建档同事务预建空会话并回写；存量记录经 `ensure_explore_session` 归位补齐。前端据此以 `session_detail` 直查重放，取代 `source_ref` 反查）、`created_at` / `updated_at`（UTC unix 毫秒，同既有时间戳口径）。维度声明：`ExploreRecord` 归 **workspace 维度**（workspace 数据分离裁定，落所属 workspace 的独立 db 文件，见 desktop-data-dimensions），内容唯一真源在磁盘 `explore.md`（workspace repo），数据三分——记录（workspace 库）/ 内容（磁盘）/ 对话（同 workspace 库的会话转录 + 轮统计行）。

`ExploreRecord` SHALL 经 native_model 版本升级（v1→v2 decode-only：`from = ExploreRecordV1`，旧记录自动升级读出 `title = name`、`promoted_to = None`；v2→v3 decode-only：`from = ExploreRecordV2`，旧记录自动升级读出 `workflow_type = "requirement"`；v3→v4 decode-only：`from = ExploreRecordV3`，旧记录自动升级读出 `session_id = None`——AgentProviderRecord / ChangeRecord 加字段先例同模式）原地演进，MUST NOT 手工迁移步骤或 legacy 迁移层；存量库 SHALL additive 打开。`name` 校验 SHALL 升级为 kebab-case（等价 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`、长度 ≤ 128，与写面 `create` 同口径；store 自持校验，MUST NOT 依赖 workflow）——`create_explore_record` / `rename_explore_record` 的非法名称 / 目标名校验均升级。

store SHALL 提供 explore 清单操作面：`list_explore_records(root)`（按 root 过滤）、`create_explore_record(root, name: Option<&str>)`（`Some` 走既有显式名建档——导入绑定路径，`title` 默认 = name；`None` SHALL 同事务生成占位名与临时标题：占位名 = `draft-<epoch-millis 毫秒串>`（kebab 合法——首段字母、次段数字），生成撞名（同毫秒并发或既有同名记录）SHALL 以写事务内**单调 +1** 兜底重试（MUST NOT 依赖事务外时钟推进）；临时标题由同一时间戳派生为 `<yyyy-MM-dd HH:mm:ss>-分析中`（纯展示装饰，后续经 `set_explore_title` 回填覆盖）；占位建档 MUST NOT 触磁盘——store 层零文件操作不变，占位名从未落盘。占位前缀 `draft-` SHALL 为「未定名」判别口径（promote 门禁与前端 stance 分形消费），字面钉死共享）、`delete_explore_record`（删记录 MUST NOT 触碰磁盘文件；记录名下的**会话及其转录与轮统计行** SHALL 随记录**同事务级联删除**——按**记录 `session_id` 指针**圈定（固定锚：指针指向的那条会话即该记录全部对话，`source_ref` 缺席不豁免）；另留 `source_ref` 归属兜底（同记录 id），拾取指针空缺 / 悬空的存量残留，两路取并集。id 为幸存行上 max+1 的可复用计数，级联清理是悬空引用不被复用 id 错挂的引用完整性保证；MUST NOT 波及 debug 来源或其他 `source_ref` 的会话）。store SHALL 新增 `set_explore_title(root, name, title)`（in-place 写 `title` 并刷新 `updated_at`；`title` 空白 → `Err`）、`set_explore_workflow_type(root, name, workflow_type)`（in-place 写 `workflow_type` 并刷新 `updated_at`；非法值 → `Err`）与 `mark_explore_promoted(root, name, change_id)`（in-place 写 `promoted_to = Some(change_id)` 并刷新 `updated_at`；已 promoted → `Err`）三个操作面。`rename_explore_record` 语义不变（in-place 改名保主键 → 保 `session_id` 指针与会话链）——引擎侧定名工具（desktop-agent-execution「会话级工具面注入与 explore 笔记工具」的 `create_explore_note`）与会话外手动改名共用此操作。孤儿语义 SHALL 为：磁盘文件被删记录保留（不自动清理，预览空态、下次落盘重建——文件是记录的可丢弃投影）；promote 后记录保留（`promoted_to` 打标，MUST NOT 删记录，会话链保留）。新模型 SHALL 随既有记录信封 API（`list_models` / `scan`）零改动可浏览。级联删除与会话还原 SHALL 天然收敛于同一 workspace 库实例内（记录与会话同库，无跨库引用）。

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

- **WHEN** 某 `ExploreRecord`（id=N）名下已有会话（含转录与轮统计行；`source_ref` 可缺席——前端发起会话恒不传），执行 `delete_explore_record` 后再新建记录（幸存行 max+1 复用 id=N）
- **THEN** 被删记录名下会话及其转录/统计行已同事务消失（按记录 `session_id` **指针**圈定，`source_ref` 缺席不豁免），新记录名下会话还原为空（`session_id` 空缺，不捞旧聊天）；debug 来源及指向其他记录 id 的会话原样存活

#### Scenario: 信封 API 零改动覆盖

- **WHEN** `SessionRecord` 注册后调用 `list_models` 与 `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与 DB 查看器代码无改动

#### Scenario: workspace 库落位隔离

- **WHEN** 审查 explore 记录与会话的落盘位置
- **THEN** 记录与名下会话转录/统计行位于所属 workspace 的独立 db 文件，全局库与 workspace repo 内无此类数据

#### Scenario: 存量 v1 记录 decode-only 升级

- **WHEN** 打开含 v1 形态 `ExploreRecord`（无 `title` / `promoted_to` / `workflow_type` / `session_id`）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`title` 读出 = `name`、`promoted_to` 读出 `None`、`workflow_type` 读出 `"requirement"`、`session_id` 读出 `None`，无手工迁移步骤、无 legacy 迁移层；新建档记录四字段按写入回读一致

#### Scenario: 存量 v2 记录 workflow_type decode-only 升级

- **WHEN** 打开含 v2 形态 `ExploreRecord`（无 `workflow_type`）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`workflow_type` 读出 `"requirement"`（默认），`title` / `promoted_to` 原值不变，无手工迁移步骤

#### Scenario: 存量 v3 记录 session_id decode-only 升级

- **WHEN** 打开含 v3 形态 `ExploreRecord`（无 `session_id`）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`session_id` 读出 `None`（缺列默认），`title` / `workflow_type` / `promoted_to` 原值不变，无手工迁移步骤

#### Scenario: session_id 归位与直查重放

- **WHEN** 建档 explore（同事务预建空会话并回写 `session_id`）后发起 agent 会话并完成，随后重开库查询
- **THEN** 记录 `session_id` 为最近一次会话 id、`updated_at` 已刷新，`id` / `root` / `name` / `title` / `promoted_to` 不变；前端以该 `session_id` 经 `session_detail` 直查重放（不再经 `source_ref` 反查）

#### Scenario: 名称校验升级为 kebab-case

- **WHEN** 分别以 `"My Feature"`（大写与空格）、`"my_new"`（下划线）、`"1abc"`（前导数字）调用 `create_explore_record` 或 `rename_explore_record`
- **THEN** 均返回显式 `Err`，库内记录数量与内容零变化；合法 kebab（如 `api-retry`）照常成功

#### Scenario: title 回填与 promote 打标

- **WHEN** 对已建档 explore 分别调用 `set_explore_title(root, name, "API 重试调研")` 与 `mark_explore_promoted(root, name, "change-uuid")` 后重开库查询
- **THEN** `title` 与 `promoted_to` 逐字一致，`updated_at` 已刷新，`id` / `root` / `name` / `created_at` 不变；空白 title 与已 promoted 的再次打标均返回 `Err`

#### Scenario: workflow_type 回填

- **WHEN** 对已建档 explore 调用 `set_explore_workflow_type(root, name, "test-only")` 后重开库查询
- **THEN** `workflow_type` 读出 `"test-only"`、`updated_at` 已刷新，`id` / `root` / `name` / `title` / `created_at` / `promoted_to` 不变；非法值（如 `"bug-fix"`）返回 `Err`、库内零变化

#### Scenario: 占位建档（None 入参）

- **WHEN** 以 `name = None` 调用 `create_explore_record(root)`
- **THEN** 返回记录 `name` 匹配 `draft-<毫秒串>`（kebab 合法）、`title` 为同一时间戳派生的 `<yyyy-MM-dd HH:mm:ss>-分析中`、`session_id` 已预建回写、`workflow_type = "requirement"`；库外零文件操作（`openspec/explores/` 不出现任何新文件）；以 `name = Some("api-retry")` 建档仍走既有路径（`title = "api-retry"`）

#### Scenario: 占位名撞名单调兜底

- **WHEN** 同一毫秒内两次以 `name = None` 建档（或占位名撞既有同名记录）
- **THEN** 两次均成功且占位名互异（第二次经写事务内单调 +1 兜底），无「记录已存在」错误逃逸、无事务外时钟依赖

## Module Contract

| 模块 | 类别 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/store.rs`（`create_explore_record`） | Function | 签名 `name: Option<&str>`；`None` 分支同事务生成 `draft-<epoch-millis>` 占位名 + `<时间戳>-分析中` 临时标题 + 预建空会话；撞名写事务内单调 +1 兜底；`Some` 分支既有行为零变化；store 层零磁盘操作不变 |
| `crates/infra/store/src/store.rs`（`rename_explore_record`） | Function | 语义不变（in-place 保主键保会话链）；新增消费方为引擎 `create_explore_note` 执行体（desktop-agent-execution），命令面仍不接线 |
