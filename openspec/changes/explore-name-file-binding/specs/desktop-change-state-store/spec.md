# desktop-change-state-store Specification (Delta)

## MODIFIED Requirements

### Requirement: change 流程状态 workspace 库单源与双向墙

desktop 的 change 流程状态 SHALL 以 workspace 库（app-data `workspaces/` 子树 per-root db 文件）为唯一载体，状态 SHALL NOT 再驻 `openspec/changes/<name>/workflow.json`。store SHALL 维护六个 workspace 维度模型（落所属 workspace 库，native_model 版本治理，时间戳 i64 UTC unix millis 口径）：

- `ChangeRecord`：`id`（String 主键，建档铸出的稳定唯一标识——UUID 形态，身份锚，终身恒定不复用）、`name`（String，change 名，恒裸名——可变属性，MUST NOT 作身份键）、`title`（String，人类可读标题，恒非空——创建 / 升级默认 = name，promote 路径继承 explore.title）、`workflow_type`、`created_at`、`status`（`active` | `archived`）、`archived_at`（Option）、`active_phase`（Option：phase / attempt / start_at）、`worktree`（Option：该 change 分配的 worktree 绝对路径；None = legacy 主 root change，存量语义，见 desktop-change-worktree）、`base_commit`（Option：创建基线 fork 点，调试 / UI 价值）；
- `PhaseRecord`：`id`（i64 主键，写事务内 max+1 分配）、`change_id`（二级索引，指向 `ChangeRecord.id`）、`phase`、`attempt`、`verdict`、`report`（内联 ≤2000 字）、`skipped`、`stale`、`backtrack_to` / `backtrack_reason`、executor / evaluator / decision 三会话槽位（Option）、`start_at`、`timestamp`；
- `ChecklistItemRecord`：打包主键 `(phase_id as u128) << 64 | item_index`（同相位内 item 序即自然序）、`phase_id` 二级索引、`item` / `pass` / `evidence`；
- `StepRecord`：`run_id`、`change_id`、`step_kind`（封闭集：`phase_next` | `phase_start` | `phase_log` | `backtrack` | `decision_log` | `static_check` | `test_execution`）、`status`、时间戳、有界输出摘要；
- `RunRecord`：`run_id`（String 主键，即 walker 既有 run id 词汇）、`change_id`（二级索引）、`status`（`running` | `completed` | `stopped` | `failed` | `interrupted`）、`reason`（Option）、`started_at` / `finished_at`（Option）——run 运行史主行，形态细则见「run 运行史落库」；
- `RunStepRecord`：`run_id`（二级索引）、`phase` / `attempt` / `step`（封闭集五词汇：`executor` | `evaluator` | `decision` | `static_check` | `test_execution`）、`status`、`seq`（emit 序）、`session_id`（Option）、`detail`（Option，有界）、时间戳——run 步节点史行，形态细则见「run 运行史落库」。

`ChangeRecord` 的身份换锚（name 主键 → id 主键 + name 属性）SHALL 经 native_model 版本段演进（表名含模型版本与主键段，旧形态表对新读面不可见），旧形态库由 store 格式版本机制整体作废重建（见 desktop-workspace-store「workspace 库格式版本与旧库作废重建」）；MUST NOT 保留 name → id 的迁移链、解码链或手工迁移步骤（旧形态行整体作废，无迁移语义承载对象），`ChangeRecordV1` 缺列读解码链随身份换锚退役。`title` 字段 SHALL 经 native_model 版本升级（v3→v4 decode-only：`from = ChangeRecordV3`，旧记录自动升级读出 `title = name`，AgentProviderRecord / ExploreRecord 加字段先例同模式）原地演进，MUST NOT 手工迁移或 legacy 迁移层；存量库 SHALL additive 打开。`worktree` 字段为执行锚引用（读面与 run 组合消费，见 desktop-change-worktree / desktop-change-orchestration），MUST NOT 反向影响库身份锚定：workspace 库路径派生仍恒以 workspace root 为锚。

**双向墙**：desktop 全链（写面 / 查询 / 编排 / 命令 / 前端）MUST NOT 读或写任何 `workflow.json`——db-backed change 对 CLI 工作流不可见，反向亦然；磁盘 change 目录只承载 markdown 产物（proposal / design / tasks / specs / reports / explore；对 worktree change 该目录位于其 worktree 内，merge 前不在主仓）。存量 CLI change（有 workflow.json 无 db 记录）SHALL 零迁移零触碰——目录与文件字节原样；desktop MUST NOT 发现、入列或展示其任何形态（清单与详情均为 db 单源，见 desktop-change-queries），MUST NOT 为其建档。

#### Scenario: 状态回环读写

- **WHEN** 对某 workspace 建档 change（携铸出 id、裸名与 title）并推进若干相位（phase_log 落 eval 与 checklist、backtrack 落 stale 标记）后关闭并重开同一 db 文件查询
- **THEN** ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunStepRecord 逐字段与写入一致（含 title、三会话槽位、worktree 与 base_commit、时间戳原值），状态查询不依赖任何磁盘 workflow.json

#### Scenario: 双向墙机械可验

- **WHEN** 扫描 desktop 全部源码（core / infra / commands / views）对 `workflow.json` 的读与写触点
- **THEN** 零触点（测试夹具内的字面量除外）；parse 模块已删除，desktop 不再解析该文件

#### Scenario: 历史 CLI change 零发现零触碰

- **WHEN** 主仓存在仅有 workflow.json 与 markdown 产物、无 db 记录的存量 change 目录
- **THEN** 目录与其 workflow.json 字节原样未动；清单不入列、详情不可达（零发现），不报错、不建档

#### Scenario: 旧形态数据零可达与解码链退役

- **WHEN** 打开含旧形态 `ChangeRecord` 行（name 主键）的存量 workspace 库并查询
- **THEN** 打开成功且零报错；作废重建后新库为空（旧 change / run / 会话数据零残留，语义见 desktop-workspace-store）；模型源码零 name → id 迁移链 / 解码链 / 手工迁移步骤

#### Scenario: 存量 v3 记录 title decode-only 升级

- **WHEN** 打开含 v3 形态 `ChangeRecord`（无 `title` 字段）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`title` 读出 = `name`，无手工迁移步骤、无 legacy 迁移层；新建档记录 `title` 按写入回读一致

#### Scenario: worktree 字段不反噬库身份

- **WHEN** 某 workspace 存在带 worktree 记录的 change，重开其 workspace 库
- **THEN** db 文件路径仍由 workspace root 身份派生（`workspaces/` 子树），worktree 路径仅作为记录字段被读取，从不参与库路径派生

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（ChangeRecord v4） | title 字段与版本升级 | `title: String`（恒非空，`from` 升级默认 = name）；native_model v3→v4 decode-only（`ChangeRecordV3` 双向 `From`）；id 主键 / name 裸名属性不变 |
| `crates/infra/store/src/store.rs` | 记录 ↔ 中性类型映射单点 | `change_state()` / `create_change_record` 映射 title；全操作面寻址仍 id |
| `crates/core/workflow/src/state.rs` | 中性状态快照 | `ChangeStateRecord` 增 `title`（port 流量像） |
