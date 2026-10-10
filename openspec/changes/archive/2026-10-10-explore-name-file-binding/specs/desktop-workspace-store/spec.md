# desktop-workspace-store Specification (Delta)

## MODIFIED Requirements

### Requirement: ExploreRecord 模型与 explore 清单操作面

store SHALL 以 `ExploreRecord` 模型承载 explore 清单条目与 agent 会话绑定。字段面 SHALL 含：独立主键 `id`（写事务内 max+1 分配）、`root`（workspace 归属，canonical 口径与 `WorkspaceRecord` 一致）、`name`（笔记文件 stem 寻址键，kebab-case 口径）、`title`（人类可读标题，恒非空——创建与升级默认 = name，agent 产出标题经 `set_explore_title` 回填覆盖）、`promoted_to`（`Option<String>`，指向已 promote 的 **change id** 身份锚，None = 草稿态）、`created_at` / `updated_at`（UTC unix 毫秒，同既有时间戳口径）。维度声明：`ExploreRecord` 归 **workspace 维度**（workspace 数据分离裁定，落所属 workspace 的独立 db 文件，见 desktop-data-dimensions），内容唯一真源在磁盘 `explore.md`（workspace repo），数据三分——记录（workspace 库）/ 内容（磁盘）/ 对话（同 workspace 库的会话转录 + 轮统计行）。

`ExploreRecord` SHALL 经 native_model 版本升级（v1→v2 decode-only：`from = ExploreRecordV1`，旧记录自动升级读出 `title = name`、`promoted_to = None`——AgentProviderRecord / ChangeRecord 加字段先例同模式）原地演进，MUST NOT 手工迁移步骤或 legacy 迁移层；存量库 SHALL additive 打开。`name` 校验 SHALL 升级为 kebab-case（等价 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`、长度 ≤ 128，与写面 `create` 同口径；store 自持校验，MUST NOT 依赖 workflow）——`create_explore_record` / `rename_explore_record` 的非法名称 / 目标名校验均升级。

store SHALL 提供 explore 清单操作面：`list_explore_records(root)`（按 root 过滤）、`create_explore_record`、`delete_explore_record`（删记录 MUST NOT 触碰磁盘文件；记录名下的**会话及其转录与轮统计行** SHALL 随记录**同事务级联删除**——按会话归属 `(source="explore", source_ref=记录id)` 圈定。id 为幸存行上 max+1 的可复用计数，级联清理是悬空 `source_ref` 不被复用 id 错挂的引用完整性保证；MUST NOT 波及 debug 来源或其他 `source_ref` 的会话）。store SHALL 新增 `set_explore_title(root, name, title)`（in-place 写 `title` 并刷新 `updated_at`；`title` 空白 → `Err`）与 `mark_explore_promoted(root, name, change_id)`（in-place 写 `promoted_to = Some(change_id)` 并刷新 `updated_at`；已 promoted → `Err`）两个操作面。孤儿语义 SHALL 为：磁盘文件被删记录保留（不自动清理，预览空态、下次落盘重建——文件是记录的可丢弃投影）；promote 后记录保留（`promoted_to` 打标，MUST NOT 删记录，会话链保留）。新模型 SHALL 随既有记录信封 API（`list_models` / `scan`）零改动可浏览。级联删除与会话还原 SHALL 天然收敛于同一 workspace 库实例内（记录与会话同库，无跨库引用）。

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

#### Scenario: 存量 v1 记录 decode-only 升级

- **WHEN** 打开含 v1 形态 `ExploreRecord`（无 `title` / `promoted_to`）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`title` 读出 = `name`、`promoted_to` 读出 `None`，无手工迁移步骤、无 legacy 迁移层；新建档记录 `title` / `promoted_to` 按写入回读一致

#### Scenario: 名称校验升级为 kebab-case

- **WHEN** 分别以 `"My Feature"`（大写与空格）、`"my_new"`（下划线）、`"1abc"`（前导数字）调用 `create_explore_record` 或 `rename_explore_record`
- **THEN** 均返回显式 `Err`，库内记录数量与内容零变化；合法 kebab（如 `api-retry`）照常成功

#### Scenario: title 回填与 promote 打标

- **WHEN** 对已建档 explore 分别调用 `set_explore_title(root, name, "API 重试调研")` 与 `mark_explore_promoted(root, name, "change-uuid")` 后重开库查询
- **THEN** `title` 与 `promoted_to` 逐字一致，`updated_at` 已刷新，`id` / `root` / `name` / `created_at` 不变；空白 title 与已 promoted 的再次打标均返回 `Err`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（ExploreRecord v2） | title / promoted_to 字段与版本升级 | `title: String`（恒非空，`from` 升级默认 = name）、`promoted_to: Option<String>`（change id 身份锚）；native_model v1→v2 decode-only（`ExploreRecordV1` 双向 `From`） |
| `crates/infra/store/src/store.rs`（explore 操作面） | 建档 / 改名 / 删 / 标题 / promote | `create_explore_record` / `rename_explore_record` kebab-case 校验（store 自持，零 workflow 依赖）；`set_explore_title` / `mark_explore_promoted` in-place 写（保主键保链）；`delete_explore_record` 级联语义不变 |
