# desktop-change-state-store Specification (Delta)

## MODIFIED Requirements

### Requirement: change 流程状态 workspace 库单源与双向墙

desktop 的 change 流程状态 SHALL 以 workspace 库（app-data `workspaces/` 子树 per-root db 文件）为唯一载体，状态 SHALL NOT 再驻 `openspec/changes/<name>/workflow.json`。store SHALL 维护七个 workspace 维度模型（落所属 workspace 库，native_model 版本治理，时间戳 i64 UTC unix millis 口径）：

- `ChangeRecord`：`id`（String 主键，建档铸出的稳定唯一标识——UUID 形态，身份锚，终身恒定不复用）、`name`（String，change 名，恒裸名——可变属性，MUST NOT 作身份键）、`title`（String，人类可读标题，恒非空——创建 / 升级默认 = name，promote 路径继承 explore.title）、`workflow_type`、`created_at`、`status`（`active` | `archived`）、`archived_at`（Option）、`active_phase`（Option：phase / attempt / start_at）、`worktree`（Option：该 change 分配的 worktree 绝对路径；None = legacy 主 root change；清理后原样保留作历史锚，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）、`base_commit`（Option：创建基线 fork 点）；
- `PhaseRecord`：`id`（i64 主键，写事务内 max+1 分配）、`change_id`（二级索引，指向 `ChangeRecord.id`）、`phase`、`attempt`、`verdict`、`report`（内联 ≤2000 字）、`skipped`、`stale`、`backtrack_to` / `backtrack_reason`、executor / evaluator / decision 三会话槽位（Option）、`start_at`、`timestamp`；
- `ChecklistItemRecord`：打包主键 `(phase_id as u128) << 64 | item_index`、`phase_id` 二级索引、`item` / `pass` / `evidence`；
- `StepRecord`：`run_id`、`change_id`、`step_kind`（封闭集：`phase_next` | `phase_start` | `phase_log` | `backtrack` | `decision_log` | `static_check` | `test_execution` | `mutation_test`）、`status`、时间戳、有界输出摘要；
- `RunRecord`：`run_id`（String 主键）、`change_id`（二级索引）、`status`（`running` | `completed` | `stopped` | `failed` | `interrupted`）、`reason`（Option）、`started_at` / `finished_at`（Option）；
- `RunNodeRecord`：`run_id`（二级索引）、`change_id`、稳定身份键 `(phase, attempt, step, instance_no)`、`step_kind`（**全词汇封闭集二十值**：`executor` | `evaluator` | `decision` | `static_check` | `test_execution` | `mutation_test` | `phase_start` | `phase_log` | `verdict_gate` | `retry_gate` | `whitelist_gate` | `preflight` | `commit` | `rebase` | `resolve_conflicts` | `merge` | `spec_sync` | `cleanup` | `seal` | `finalize`）、`status`、`session_id`（Option，WorkerAgent 步）、`detail`（Option，有界）、`output_ref`（Option，`ExecOutput::Recorded { exec_id }` 引用）、时间戳；
- `RunEdgeRecord`：`run_id`（二级索引）、`from_node_key`（二级索引）、`to_node_key`、`edge_kind`（封闭集：`forward` | `feedback` | `retry` | `backtrack`）、时间戳。

`ChangeRecord` 的身份换锚（name 主键 → id 主键 + name 属性）SHALL 经 native_model 版本段演进（表名含模型版本与主键段，旧形态表对新读面不可见），旧形态库由 store 格式版本机制整体作废重建；MUST NOT 保留 name → id 的迁移链、解码链或手工迁移步骤。`title` 字段 SHALL 经 native_model 版本升级（v3→v4 decode-only）原地演进。`worktree` 字段为执行锚引用，MUST NOT 反向影响库身份锚定。

**执行图 model 重设计（不计历史兼容）**：`RunStepRecord`（`seq` + 五词汇 + 10→5 过滤）整体退役，`RunNodeRecord` / `RunEdgeRecord` 取代之；其后 step_kind 全词汇由十值扩为十八值（新增 8 个归档步词），再扩为十九值（新增 `mutation_test` 突变门禁步词），worktree 清理变更再扩为二十值（新增 `cleanup` 归档清理步词，白名单式扩展），归档节点复用 `RunNodeRecord`（零新表），MUST NOT 保留归档独立落库表。

**双向墙**：desktop 全链（写面 / 查询 / 编排 / 命令 / 前端）MUST NOT 读或写任何 `workflow.json`；磁盘 change 目录只承载 markdown 产物。存量 CLI change（有 workflow.json 无 db 记录）SHALL 零迁移零触碰。

#### Scenario: 状态回环读写

- **WHEN** 对某 workspace 建档 change 并推进若干相位（含 archive 相位空 checklist 落账）后关闭并重开同一 db 文件查询
- **THEN** ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunNodeRecord / RunEdgeRecord 逐字段与写入一致（含 title、三会话槽位、worktree 与 base_commit、归档 9 个步词（含 cleanup）与 mutation_test、时间戳原值），状态查询不依赖任何磁盘 workflow.json

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

### Requirement: 归档双写

写面 SHALL 提供 `archive` 操作：db `ChangeRecord.status` 翻转为 `archived`（附 `archived_at`）与主仓 active→archive 目录改名（`YYYY-MM-DD-<name>` 日期前缀）SHALL 以同一提交语义完成（双写顺序与失败补偿由 design 定稿）；`ChangeRecord.id` 与 `name` MUST NOT 随目录改名变化（db 身份与磁盘目录名解耦）。对带 worktree 记录的 change，归档 SHALL 以主仓目录在场为前置：主仓 active / archive 两树均未命中该目录时 SHALL 显式拒绝并引导「先 merge worktree 分支回主仓再归档」（merge-first 语义见 desktop-change-worktree；archive 相位对前置的自动满足路径见 desktop-change-archive）；merge 后主仓目录在场，既有双写语义零特判。**双写操作本体** MUST NOT 触碰 worktree / branch（零 `git worktree` / `git branch` 调用——git 触点不进 write 面的红线保持；worktree 清理不属双写本体，由 archive 相位的 Cleanup 步经 orchestration 侧 vcs 缝承载，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；archive 相位（desktop-change-archive）的 Commit / Rebase / ResolveConflicts / Merge / Cleanup 步不属双写本体，且双写收口 SHALL 由 archive 相位的 Seal 步复用（单点复用、语义零改动）。对无 db 记录的 change（id 不可达）MUST NOT 提供归档（显式拒绝）。归档经 run 的 archive 相位收口，独立 IPC 薄命令（`archive_change`）的保留形态由 design 定稿（写面操作本体不变，命令面收编进 `change_flow`）。

#### Scenario: 双写一致

- **WHEN** archive 相位 Seal 步按 id 调用写面归档成功
- **THEN** db status 为 `archived` 且 `archived_at` 在案，主仓磁盘目录已迁移至 archive 树并带日期前缀，两者任一不存在即视为失败

#### Scenario: 主键与 name 不随改名变

- **WHEN** 归档后按 change id 查询 db 与按月分组归档列表
- **THEN** `ChangeRecord.id` 与 `name` 均保持原值（name 恒裸名，MUST NOT 含日期前缀），归档条目经 id 可达、月分组以 `archived_at` 为准

#### Scenario: 未 merge 的 worktree change 归档被引导拒绝

- **WHEN** 对带 worktree 记录且主仓 active / archive 两树均未命中目录的 change 直接调用归档写面
- **THEN** 显式拒绝且错误文案引导先 merge worktree 分支（非泛化「目录未找到」），db 与磁盘零变化

#### Scenario: 双写本体零 worktree / branch 触碰

- **WHEN** archive 相位 Seal 步双写收口执行（其前置 git 步与 Cleanup 步已由 walker 经 vcs 缝完成）
- **THEN** 双写操作自身仅做目录改名与 db 翻转：零 `git worktree` / `git branch` 调用、零 worktree / branch 状态变更（清理走 Cleanup 步，不属双写本体）

#### Scenario: 无建档 id 拒绝归档

- **WHEN** 对仅有 workflow.json 的 CLI change 目录（或任何无 ChangeRecord 的 id）发起归档
- **THEN** 显式拒绝（无 ChangeRecord 不可归档），零落账零目录改动

### Requirement: run 执行图落库

run 状态 SHALL 落 workspace 库（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 三表，`change_id` 归键，全史保留不截 last_run；attempt 经 `phase_start` max+1 分配跨 run 不撞号）。写时序 SHALL 为**每 run 两写**：run 行 SHALL 在 run 发起时以 `status=running` 建立，run 收口时 SHALL 以终态更新 + 该 run 全部节点 + 边整包**单事务**落库；MUST NOT 每步写放大。run 收口同事务处置 `ChangeRecord.active_phase` SHALL 以 `change_id` 定位记录。

执行图节点 SHALL 为**全词汇封闭集单点定义**：`RunNodeRecord` 落全部二十类步词汇（agent 阶段 `executor` / `evaluator` / `decision`、脚本阶段 `static_check` / `test_execution` / `mutation_test`、流程面步骤 `phase_start` / `phase_log`、三门 gate `verdict_gate` / `retry_gate` / `whitelist_gate`、归档步 `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `cleanup` / `seal` / `finalize`），MUST NOT 过滤。节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`（`instance_no` 每次执行递增，feedback 重跑为独立新节点）。`RunNodeRecord` 的 `session_id` / `detail` / `output_ref` SHALL 随行落库；节点 `session_id` SHALL 为前端会话转录的 sessionId 直查键（WorkerAgent 步在案、非 WorkerAgent 步为空；前端零 provenance `source_ref` 反查，会话 `source_ref` 留痕仅供后端 provenance，见 desktop-change-orchestration「会话挂靠 provenance 与转录 session id 直查」）。

执行图边 SHALL 单独落 `RunEdgeRecord`：`edge_kind ∈ { forward, feedback, retry, backtrack }`，`from_node_key` / `to_node_key` 指向节点身份键；`run_id` 二级索引与 `from_node_key` 二级索引 SHALL 均在案。前端 `deriveEdges` 列差 Δ 派生 SHALL 退役（读边表）。

`RunNodeRecord` / `RunEdgeRecord` SHALL 与既有 `StepRecord` 审计面**职责分立**：StepRecord 为相位机步骤审计行，RunNodeRecord / RunEdgeRecord 为执行图节点史 + 边史；两表 MUST NOT 互为替代。run 中途崩溃（无 finish 落包）SHALL 丢该 run 节点 / 边史——start 行经启动标定可见，为既定权衡留痕。

#### Scenario: 两写时序与零写放大

- **WHEN** 假引擎驱动一相位 run 走完（含 executor / static-check / evaluator 多步与 feedback 边）
- **THEN** 库写事务恰两次：发起时 RunRecord（running）落行、收口时同事务完成终态更新与全部 RunNodeRecord + RunEdgeRecord 整包落库；期间零每步写事务

#### Scenario: 节点与边整包原子

- **WHEN** finish 落包过程中某环节失败（如节点或边行编码错误注入）
- **THEN** 写事务整体回滚：无 RunNodeRecord / RunEdgeRecord 半包、RunRecord 终态与 finished_at 零变化

#### Scenario: 全词汇与稳定身份键

- **WHEN** 一次 run 覆盖全部二十类步词汇（含 phase_start / phase_log / 三门 gate、mutation_test、9 个归档步与 feedback 重跑）后查询 run_nodes / run_edges
- **THEN** 二十类词汇全落 RunNodeRecord（零过滤）；身份键 `(run_id, phase, attempt, step, instance_no)` 稳定唯一，feedback 重跑为 `instance_no` 递增新节点；边表 feedback / retry / backtrack / forward 四 kind 齐备

#### Scenario: 归档步复用 RunNodeRecord

- **WHEN** archive 相位一次 run 收口后查询
- **THEN** 归档 9 个步词节点（含 cleanup）与 PhaseStart / PhaseLog 全落 RunNodeRecord（零新表、零 archive 独立落库），串行 forward 边落 RunEdgeRecord

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
| `crates/infra/store/src/model.rs`（RunNodeRecord step_kind） | 二十值封闭集 | `cleanup` 步词白名单式入封闭集（十九 → 二十）；表结构零变化（step_kind 词面扩展）、零迁移 |
| `crates/core/workflow/src/state.rs`（RunStepKind） | 词面单点 | `RunStepKind::Cleanup`（`as_str` = `cleanup`）与 `StepNode` / `ChangeStepKind` 同词面 1:1 |
