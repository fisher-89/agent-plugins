# desktop-change-state-store Specification (Delta)

## Purpose

定义 change 流程状态的 workspace 库持久化契约：七状态模型单源承载、与 workflow.json 的双向墙、相位机写面单事务落库、归档双写与步骤审计。执行图节点全词汇由十八值扩为十九值（新增 `mutation_test` 突变门禁步词）。

## MODIFIED Requirements

### Requirement: change 流程状态 workspace 库单源与双向墙

desktop 的 change 流程状态 SHALL 以 workspace 库（app-data `workspaces/` 子树 per-root db 文件）为唯一载体，状态 SHALL NOT 再驻 `openspec/changes/<name>/workflow.json`。store SHALL 维护七个 workspace 维度模型（落所属 workspace 库，native_model 版本治理，时间戳 i64 UTC unix millis 口径）：

- `ChangeRecord`：`id`（String 主键，建档铸出的稳定唯一标识——UUID 形态，身份锚，终身恒定不复用）、`name`（String，change 名，恒裸名——可变属性，MUST NOT 作身份键）、`title`（String，人类可读标题，恒非空——创建 / 升级默认 = name，promote 路径继承 explore.title）、`workflow_type`、`created_at`、`status`（`active` | `archived`）、`archived_at`（Option）、`active_phase`（Option：phase / attempt / start_at）、`worktree`（Option：该 change 分配的 worktree 绝对路径；None = legacy 主 root change）、`base_commit`（Option：创建基线 fork 点）；
- `PhaseRecord`：`id`（i64 主键，写事务内 max+1 分配）、`change_id`（二级索引，指向 `ChangeRecord.id`）、`phase`、`attempt`、`verdict`、`report`（内联 ≤2000 字）、`skipped`、`stale`、`backtrack_to` / `backtrack_reason`、executor / evaluator / decision 三会话槽位（Option）、`start_at`、`timestamp`；
- `ChecklistItemRecord`：打包主键 `(phase_id as u128) << 64 | item_index`、`phase_id` 二级索引、`item` / `pass` / `evidence`；
- `StepRecord`：`run_id`、`change_id`、`step_kind`（封闭集：`phase_next` | `phase_start` | `phase_log` | `backtrack` | `decision_log` | `static_check` | `test_execution` | `mutation_test`）、`status`、时间戳、有界输出摘要；
- `RunRecord`：`run_id`（String 主键）、`change_id`（二级索引）、`status`（`running` | `completed` | `stopped` | `failed` | `interrupted`）、`reason`（Option）、`started_at` / `finished_at`（Option）；
- `RunNodeRecord`：`run_id`（二级索引）、`change_id`、稳定身份键 `(phase, attempt, step, instance_no)`、`step_kind`（**全词汇封闭集十九值**：`executor` | `evaluator` | `decision` | `static_check` | `test_execution` | `mutation_test` | `phase_start` | `phase_log` | `verdict_gate` | `retry_gate` | `whitelist_gate` | `preflight` | `commit` | `rebase` | `resolve_conflicts` | `merge` | `spec_sync` | `seal` | `finalize`）、`status`、`session_id`（Option，WorkerAgent 步）、`detail`（Option，有界）、`output_ref`（Option，`ExecOutput::Recorded { exec_id }` 引用）、时间戳；
- `RunEdgeRecord`：`run_id`（二级索引）、`from_node_key`（二级索引）、`to_node_key`、`edge_kind`（封闭集：`forward` | `feedback` | `retry` | `backtrack`）、时间戳。

`ChangeRecord` 的身份换锚（name 主键 → id 主键 + name 属性）SHALL 经 native_model 版本段演进（表名含模型版本与主键段，旧形态表对新读面不可见），旧形态库由 store 格式版本机制整体作废重建；MUST NOT 保留 name → id 的迁移链、解码链或手工迁移步骤。`title` 字段 SHALL 经 native_model 版本升级（v3→v4 decode-only）原地演进。`worktree` 字段为执行锚引用，MUST NOT 反向影响库身份锚定。

**执行图 model 重设计（不计历史兼容）**：`RunStepRecord`（`seq` + 五词汇 + 10→5 过滤）整体退役，`RunNodeRecord` / `RunEdgeRecord` 取代之；其后 step_kind 全词汇由十值扩为十八值（新增 8 个归档步词），本变更再扩为十九值（新增 `mutation_test` 突变门禁步词，白名单式扩展），归档节点复用 `RunNodeRecord`（零新表），MUST NOT 保留归档独立落库表。

**双向墙**：desktop 全链（写面 / 查询 / 编排 / 命令 / 前端）MUST NOT 读或写任何 `workflow.json`；磁盘 change 目录只承载 markdown 产物。存量 CLI change（有 workflow.json 无 db 记录）SHALL 零迁移零触碰。

#### Scenario: 状态回环读写

- **WHEN** 对某 workspace 建档 change 并推进若干相位（含 archive 相位空 checklist 落账）后关闭并重开同一 db 文件查询
- **THEN** ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunNodeRecord / RunEdgeRecord 逐字段与写入一致（含 title、三会话槽位、worktree 与 base_commit、归档 8 个步词与 mutation_test、时间戳原值），状态查询不依赖任何磁盘 workflow.json

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

### Requirement: run 执行图落库

run 状态 SHALL 落 workspace 库（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 三表，`change_id` 归键，全史保留不截 last_run；attempt 经 `phase_start` max+1 分配跨 run 不撞号）。写时序 SHALL 为**每 run 两写**：run 行 SHALL 在 run 发起时以 `status=running` 建立，run 收口时 SHALL 以终态更新 + 该 run 全部节点 + 边整包**单事务**落库；MUST NOT 每步写放大。run 收口同事务处置 `ChangeRecord.active_phase` SHALL 以 `change_id` 定位记录。

执行图节点 SHALL 为**全词汇封闭集单点定义**：`RunNodeRecord` 落全部十九类步词汇（agent 阶段 `executor` / `evaluator` / `decision`、脚本阶段 `static_check` / `test_execution` / `mutation_test`、流程面步骤 `phase_start` / `phase_log`、三门 gate `verdict_gate` / `retry_gate` / `whitelist_gate`、归档步 `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `seal` / `finalize`），MUST NOT 过滤。节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`（`instance_no` 每次执行递增，feedback 重跑为独立新节点）。`RunNodeRecord` 的 `session_id` / `detail` / `output_ref` SHALL 随行落库。

执行图边 SHALL 单独落 `RunEdgeRecord`：`edge_kind ∈ { forward, feedback, retry, backtrack }`，`from_node_key` / `to_node_key` 指向节点身份键；`run_id` 二级索引与 `from_node_key` 二级索引 SHALL 均在案。前端 `deriveEdges` 列差 Δ 派生 SHALL 退役（读边表）。

`RunNodeRecord` / `RunEdgeRecord` SHALL 与既有 `StepRecord` 审计面**职责分立**：StepRecord 为相位机步骤审计行，RunNodeRecord / RunEdgeRecord 为执行图节点史 + 边史；两表 MUST NOT 互为替代。run 中途崩溃（无 finish 落包）SHALL 丢该 run 节点 / 边史——start 行经启动标定可见，为既定权衡留痕。

#### Scenario: 两写时序与零写放大

- **WHEN** 假引擎驱动一相位 run 走完（含 executor / static-check / evaluator 多步与 feedback 边）
- **THEN** 库写事务恰两次：发起时 RunRecord（running）落行、收口时同事务完成终态更新与全部 RunNodeRecord + RunEdgeRecord 整包落库；期间零每步写事务

#### Scenario: 节点与边整包原子

- **WHEN** finish 落包过程中某环节失败（如节点或边行编码错误注入）
- **THEN** 写事务整体回滚：无 RunNodeRecord / RunEdgeRecord 半包、RunRecord 终态与 finished_at 零变化

#### Scenario: 全词汇与稳定身份键

- **WHEN** 一次 run 覆盖全部十九类步词汇（含 phase_start / phase_log / 三门 gate、mutation_test、8 个归档步与 feedback 重跑）后查询 run_nodes / run_edges
- **THEN** 十九类词汇全落 RunNodeRecord（零过滤）；身份键 `(run_id, phase, attempt, step, instance_no)` 稳定唯一，feedback 重跑为 `instance_no` 递增新节点；边表 feedback / retry / backtrack / forward 四 kind 齐备

#### Scenario: 归档步复用 RunNodeRecord

- **WHEN** archive 相位一次 run 收口后查询
- **THEN** 归档 8 个新步词节点与 PhaseStart / PhaseLog 全落 RunNodeRecord（零新表、零 archive 独立落库），串行 forward 边落 RunEdgeRecord

#### Scenario: 边表两向查询

- **WHEN** 按 `run_id` 全量读某 run 的边、按 `from_node_key` 读某节点后继
- **THEN** 两二级索引均可达且结果一致；`from_node_key` 后继查询能枚举 feedback / forward / retry / backtrack 出边

#### Scenario: 全史保留与跨 run 不撞号

- **WHEN** 同一 change（同一 id）先后发起两次 run（第二次自续走锚点推进）
- **THEN** 两 run 的 RunRecord 与各自 RunNodeRecord / RunEdgeRecord 全量在库，同 phase 的 attempt 经 max+1 分配递增不撞号

#### Scenario: 崩溃丢节点边史为既定权衡

- **WHEN** run 运行中途进程被 kill（finish 落包未达）后重启查询
- **THEN** 该 run 的 RunRecord 经启动标定为 interrupted、其 RunNodeRecord / RunEdgeRecord 零行；attempt 级评估史仍可达自 PhaseRecord；库中无每步写透补偿痕迹

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（API） | 步词汇封闭集扩展 | `RunNodeRecord.step_kind` 十九值（增 `mutation_test`）；`StepRecord.step_kind` 封闭集增 `mutation_test`；零 model 版本演进（词汇为枚举序列化词面，非字段形态变化） |
| `crates/core/orchestration/src/state.rs`（API） | 词汇同源 | `ChangeStepKind::MutationTest` 与 `RunStepKind::MutationTest` 1:1 同词面 |
