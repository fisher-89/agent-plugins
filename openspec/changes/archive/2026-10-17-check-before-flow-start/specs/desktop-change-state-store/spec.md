# desktop-change-state-store Specification (Delta)

## MODIFIED Requirements

### Requirement: change 流程状态 workspace 库单源与双向墙

desktop 的 change 流程状态 SHALL 以 workspace 库（app-data `workspaces/` 子树 per-root db 文件）为唯一载体，状态 SHALL NOT 再驻 `openspec/changes/<name>/workflow.json`。store SHALL 维护七个 workspace 维度模型（落所属 workspace 库，native_model 版本治理，时间戳 i64 UTC unix millis 口径）：

- `ChangeRecord`：`id`（String 主键，建档铸出的稳定唯一标识——UUID 形态，身份锚，终身恒定不复用）、`name`（String，change 名，恒裸名——可变属性，MUST NOT 作身份键）、`title`（String，人类可读标题，恒非空——创建 / 升级默认 = name，promote 路径继承 explore.title）、`workflow_type`、`created_at`、`status`（`active` | `archived`）、`archived_at`（Option）、`active_phase`（Option：phase / attempt / start_at）、`worktree`（Option：该 change 分配的 worktree 绝对路径；None = legacy 主 root change；清理后原样保留作历史锚，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）、`base_commit`（Option：创建基线 fork 点）；
- `PhaseRecord`：`id`（i64 主键，写事务内 max+1 分配）、`change_id`（二级索引，指向 `ChangeRecord.id`）、`phase`、`attempt`、`verdict`、`report`（内联 ≤2000 字）、`skipped`、`stale`、`backtrack_to` / `backtrack_reason`、executor / evaluator / decision 三会话槽位（Option）、`start_at`、`timestamp`；
- `ChecklistItemRecord`：打包主键 `(phase_id as u128) << 64 | item_index`、`phase_id` 二级索引、`item` / `pass` / `evidence`；
- `StepRecord`：`run_id`、`change_id`、`step_kind`（封闭集：`phase_next` | `phase_start` | `phase_log` | `backtrack` | `decision_log` | `static_check` | `test_execution` | `mutation_test`）、`status`、时间戳、有界输出摘要；
- `RunRecord`：`run_id`（String 主键）、`change_id`（二级索引）、`status`（`running` | `completed` | `stopped` | `failed` | `interrupted`）、`reason`（Option）、`started_at` / `finished_at`（Option）；
- `RunNodeRecord`：`run_id`（二级索引）、`change_id`、稳定身份键 `(phase, attempt, step, instance_no)`、`step_kind`（**全词汇封闭集二十一值**：`executor` | `evaluator` | `decision` | `static_check` | `test_execution` | `mutation_test` | `clean` | `phase_start` | `phase_log` | `verdict_gate` | `retry_gate` | `whitelist_gate` | `preflight` | `commit` | `rebase` | `resolve_conflicts` | `merge` | `spec_sync` | `cleanup` | `seal` | `finalize`）、`status`、`session_id`（Option，WorkerAgent 步）、`detail`（Option，有界）、`output_ref`（Option，`ExecOutput::Recorded { exec_id }` 引用）、时间戳；
- `RunEdgeRecord`：`run_id`（二级索引）、`from_node_key`（二级索引）、`to_node_key`、`edge_kind`（封闭集：`forward` | `feedback` | `retry` | `backtrack`）、时间戳。

`ChangeRecord` 的身份换锚（name 主键 → id 主键 + name 属性）SHALL 经 native_model 版本段演进（表名含模型版本与主键段，旧形态表对新读面不可见），旧形态库由 store 格式版本机制整体作废重建；MUST NOT 保留 name → id 的迁移链、解码链或手工迁移步骤。`title` 字段 SHALL 经 native_model 版本升级（v3→v4 decode-only）原地演进。`worktree` 字段为执行锚引用，MUST NOT 反向影响库身份锚定。

**执行图 model 重设计（不计历史兼容）**：`RunStepRecord`（`seq` + 五词汇 + 10→5 过滤）整体退役，`RunNodeRecord` / `RunEdgeRecord` 取代之；其后 step_kind 全词汇由十值扩为十八值（新增 8 个归档步词），再扩为十九值（新增 `mutation_test` 突变门禁步词），worktree 清理变更再扩为二十值（新增 `cleanup` 归档清理步词，白名单式扩展），基线检查变更再扩为二十一值（新增 `clean` initialize 基线恢复步词，白名单式扩展），归档节点复用 `RunNodeRecord`（零新表），MUST NOT 保留归档独立落库表。

**双向墙**：desktop 全链（写面 / 查询 / 编排 / 命令 / 前端）MUST NOT 读或写任何 `workflow.json`；磁盘 change 目录只承载 markdown 产物与报告工件。存量 CLI change（有 workflow.json 无 db 记录）SHALL 零迁移零触碰。

#### Scenario: 状态回环读写

- **WHEN** 对某 workspace 建档 change 并推进若干相位（含 initialize 基线检查相位与 archive 相位空 checklist 落账）后关闭并重开同一 db 文件查询
- **THEN** ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunNodeRecord / RunEdgeRecord 逐字段与写入一致（含 title、三会话槽位、worktree 与 base_commit、initialize 基线步词 clean、归档 9 个步词（含 cleanup）与 mutation_test、时间戳原值），状态查询不依赖任何磁盘 workflow.json

#### Scenario: 双向墙机械可验

- **WHEN** 扫描 desktop 全部源码对 `workflow.json` 的读与写触点
- **THEN** 零触点（测试夹具内的字面量除外）；parse 模块已删除，desktop 不再解析该文件

#### Scenario: 历史 CLI change 零发现零触碰

- **WHEN** 主仓存在仅有 workflow.json 与 markdown 产物、无 db 记录的存量 change 目录
- **THEN** 目录与其 workflow.json 字节原样未动；清单不入列、详情不可达（零发现），不报错、不建档

#### Scenario: 旧形态数据零可达与解码链退役

- **WHEN** 打开含旧形态 `ChangeRecord` 行（name 主键）或旧 `RunStepRecord` 行的存量 workspace 库并查询
- **THEN** 打开成功且零报错；作废重建后新库为空（旧 change / run / 会话 / 步数据零残留）；模型源码零 name → id 迁移链 / `RunStepRecord` → `RunNodeRecord` 迁移链 / 解码链 / 手工迁移步骤

#### Scenario: worktree 字段不反噬库身份

- **WHEN** 某 workspace 存在带 worktree 记录的 change，重开其 workspace 库
- **THEN** db 文件路径仍由 workspace root 身份派生，worktree 路径仅作为记录字段被读取，从不参与库路径派生

#### Scenario: 存量 v3 记录 title decode-only 升级

- **WHEN** 打开含 v3 形态 `ChangeRecord`（无 `title` 字段）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`title` 读出 = `name`，无手工迁移步骤、无 legacy 迁移层；新建档记录 `title` 按写入回读一致

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（RunNodeRecord step_kind） | 二十一值封闭集 | `clean` 步词白名单式入封闭集（二十 → 二十一）；表结构零变化（step_kind 词面扩展）、零迁移 |
| `crates/core/workflow/src/state.rs`（RunStepKind） | 词面单点 | `RunStepKind::Clean`（`as_str` = `clean`）与 `StepNode` / `ChangeStepKind` 同词面 1:1 |
