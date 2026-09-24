# desktop-workspace-store Specification（delta）

## ADDED Requirements

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
| `ExploreRecord`（模型，新） | explore 清单与绑定记录 | native_model 新 id；PK 独立自增 id；root / name / created_at / updated_at（链锚与路径关联字段 design 定）；user 维度声明；孤儿保留 |
| `AgentRunRecord`（模型，v2） | run 元数据演进 | + `source`（缺省 `debug`）/ `source_ref` / `parent_run_id`；native_model 版本原地演进，无迁移层 |
| `crates/infra/store/src/store.rs`（扩展） | 清单操作 + 链查询 | `list_explore_records(root)` / `create` / `delete`；按 `(source, source_ref)` 的链还原查询单点收口 |
| 记录信封 API（既有，不动） | 模型通用读面 | 新模型注册即被 `list_models` / `scan` 覆盖，零代码 |
