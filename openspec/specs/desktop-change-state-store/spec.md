# desktop-change-state-store Specification

## Purpose

定义 change 流程状态的 workspace 库持久化契约：六状态模型（ChangeRecord（id 主键身份锚） / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunStepRecord）单源承载、与 workflow.json 的双向墙、相位机写面单事务落库、归档双写与步骤审计——desktop 的 change 流程状态不再驻磁盘 JSON 文件。

## Requirements

### Requirement: change 流程状态 workspace 库单源与双向墙

desktop 的 change 流程状态 SHALL 以 workspace 库（app-data `workspaces/` 子树 per-root db 文件）为唯一载体，状态 SHALL NOT 再驻 `openspec/changes/<name>/workflow.json`。store SHALL 维护七个 workspace 维度模型（落所属 workspace 库，native_model 版本治理，时间戳 i64 UTC unix millis 口径）：

- `ChangeRecord`：`id`（String 主键，建档铸出的稳定唯一标识——UUID 形态，身份锚，终身恒定不复用）、`name`（String，change 名，恒裸名——可变属性，MUST NOT 作身份键）、`title`（String，人类可读标题，恒非空——创建 / 升级默认 = name，promote 路径继承 explore.title）、`workflow_type`、`created_at`、`status`（`active` | `archived`）、`archived_at`（Option）、`active_phase`（Option：phase / attempt / start_at）、`worktree`（Option：该 change 分配的 worktree 绝对路径；None = legacy 主 root change）、`base_commit`（Option：创建基线 fork 点）；
- `PhaseRecord`：`id`（i64 主键，写事务内 max+1 分配）、`change_id`（二级索引，指向 `ChangeRecord.id`）、`phase`、`attempt`、`verdict`、`report`（内联 ≤2000 字）、`skipped`、`stale`、`backtrack_to` / `backtrack_reason`、executor / evaluator / decision 三会话槽位（Option）、`start_at`、`timestamp`；
- `ChecklistItemRecord`：打包主键 `(phase_id as u128) << 64 | item_index`、`phase_id` 二级索引、`item` / `pass` / `evidence`；
- `StepRecord`：`run_id`、`change_id`、`step_kind`（封闭集：`phase_next` | `phase_start` | `phase_log` | `backtrack` | `decision_log` | `static_check` | `test_execution`）、`status`、时间戳、有界输出摘要；
- `RunRecord`：`run_id`（String 主键）、`change_id`（二级索引）、`status`（`running` | `completed` | `stopped` | `failed` | `interrupted`）、`reason`（Option）、`started_at` / `finished_at`（Option）；
- `RunNodeRecord`：`run_id`（二级索引）、`change_id`、稳定身份键 `(phase, attempt, step, instance_no)`、`step_kind`（**全词汇封闭集十八值**：`executor` | `evaluator` | `decision` | `static_check` | `test_execution` | `phase_start` | `phase_log` | `verdict_gate` | `retry_gate` | `whitelist_gate` | `preflight` | `commit` | `rebase` | `resolve_conflicts` | `merge` | `spec_sync` | `seal` | `finalize`）、`status`、`session_id`（Option，WorkerAgent 步）、`detail`（Option，有界）、`output_ref`（Option，`ExecOutput::Recorded { exec_id }` 引用）、时间戳；
- `RunEdgeRecord`：`run_id`（二级索引）、`from_node_key`（二级索引）、`to_node_key`、`edge_kind`（封闭集：`forward` | `feedback` | `retry` | `backtrack`）、时间戳。

`ChangeRecord` 的身份换锚（name 主键 → id 主键 + name 属性）SHALL 经 native_model 版本段演进（表名含模型版本与主键段，旧形态表对新读面不可见），旧形态库由 store 格式版本机制整体作废重建；MUST NOT 保留 name → id 的迁移链、解码链或手工迁移步骤。`title` 字段 SHALL 经 native_model 版本升级（v3→v4 decode-only）原地演进。`worktree` 字段为执行锚引用，MUST NOT 反向影响库身份锚定。

**执行图 model 重设计（不计历史兼容）**：`RunStepRecord`（`seq` + 五词汇 + 10→5 过滤）整体退役，`RunNodeRecord` / `RunEdgeRecord` 取代之；本变更在此基础上把 `step_kind` 全词汇由十值扩为十八值（新增 8 个归档步词），归档节点复用 `RunNodeRecord`（零新表），MUST NOT 保留归档独立落库表。

**双向墙**：desktop 全链（写面 / 查询 / 编排 / 命令 / 前端）MUST NOT 读或写任何 `workflow.json`；磁盘 change 目录只承载 markdown 产物。存量 CLI change（有 workflow.json 无 db 记录）SHALL 零迁移零触碰。

#### Scenario: 状态回环读写

- **WHEN** 对某 workspace 建档 change 并推进若干相位（含 archive 相位空 checklist 落账）后关闭并重开同一 db 文件查询
- **THEN** ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunNodeRecord / RunEdgeRecord 逐字段与写入一致（含 title、三会话槽位、worktree 与 base_commit、归档 8 个步词、时间戳原值），状态查询不依赖任何磁盘 workflow.json

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

### Requirement: 相位机写面单事务落库

写面 `phase_log` SHALL 在单个 redb 写事务内原子完成：PhaseRecord 落行 + 该条目全部 ChecklistItemRecord 落行 + `ChangeRecord.active_phase` 更新 + backtrack 触发的 stale 翻转传播——任一环节失败 SHALL 整体回滚，MUST NOT 留半截状态。`(change, phase, attempt)` SHALL 在写事务内查重（沿 store 既有唯一性惯例，同 `AgentProviderRecord.name`），重复落账 SHALL 返回 `StoreError`。checklist 落行顺序 SHALL 保持 evaluator 输出序（打包键同相位内自然序）。`report` 内联上限 2000 字与插件 500 字读侧上限彻底解耦（desktop 自有 schema）。

#### Scenario: 单事务原子零残留

- **WHEN** phase_log 落库过程中某环节失败（如 checklist 编码错误注入）
- **THEN** 写事务整体回滚：无 PhaseRecord 行、无 ChecklistItemRecord 行、active_phase 与 stale 标记零变化

#### Scenario: 重复 attempt 被查重拒绝

- **WHEN** 对已存在 `(change, phase, attempt)` 记录的组合再次 phase_log
- **THEN** 返回显式 `StoreError`，库内记录数量与内容零变化

#### Scenario: checklist 顺序保真

- **WHEN** evaluator 输出含多条 checklist 项的 phase_log 落库后按打包键扫描
- **THEN** 条目按 item_index 升序返回且与 evaluator 输出序逐条一致

### Requirement: 归档双写

写面 SHALL 提供 `archive` 操作：db `ChangeRecord.status` 翻转为 `archived`（附 `archived_at`）与主仓 active→archive 目录改名（`YYYY-MM-DD-<name>` 日期前缀）SHALL 以同一提交语义完成（双写顺序与失败补偿由 design 定稿）；`ChangeRecord.id` 与 `name` MUST NOT 随目录改名变化（db 身份与磁盘目录名解耦）。对带 worktree 记录的 change，归档 SHALL 以主仓目录在场为前置：主仓 active / archive 两树均未命中该目录时 SHALL 显式拒绝并引导「先 merge worktree 分支回主仓再归档」（merge-first 语义见 desktop-change-worktree；archive 相位对前置的自动满足路径见 desktop-change-archive）；merge 后主仓目录在场，既有双写语义零特判。**双写操作本体** MUST NOT 触碰 worktree / branch（清理为用户手动边界）；archive 相位（desktop-change-archive）的 Commit / Rebase / ResolveConflicts / Merge 步不属双写本体，且双写收口 SHALL 由 archive 相位的 Seal 步复用（单点复用、语义零改动）。对无 db 记录的 change（id 不可达）MUST NOT 提供归档（显式拒绝）。归档经 run 的 archive 相位收口，独立 IPC 薄命令（`archive_change`）的保留形态由 design 定稿（写面操作本体不变，命令面收编进 `change_flow`）。

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

- **WHEN** archive 相位 Seal 步双写收口执行（其前置 git 步已由 walker 完成）
- **THEN** 双写操作自身仅做目录改名与 db 翻转：零 `git worktree` / `git branch` 调用、零 worktree / branch 状态变更（清理仍为用户手动边界）

#### Scenario: 无建档 id 拒绝归档

- **WHEN** 对仅有 workflow.json 的 CLI change 目录（或任何无 ChangeRecord 的 id）发起归档
- **THEN** 显式拒绝（无 ChangeRecord 不可归档），零落账零目录改动

### Requirement: 步骤审计落库可查

写面与编排的相位机步骤（`phase_next` / `phase_start` / `phase_log` / `backtrack` / `decision_log` / `static_check` / `test_execution`）SHALL 逐条落 StepRecord 审计行（`change_id` 归键），行携 `run_id` 使同一 run 的步骤序列可串链；审计查询 SHALL 可按 change id（与 run）枚举步骤历史（新增能力）。StepRecord SHALL 为审计面，MUST NOT 作为 run 恢复依据：run 状态仍驻内存 ChangeFlowControl，重启后 run 由 `phase_next` 依 eval 历史重算。输出摘要 SHALL 有界截断（上限对齐 report ≤2000 口径，具体值 design 定稿），完整输出以引用（checks 报告目录 / 会话 id）出全量，MUST NOT 全文落库。

#### Scenario: 步骤历史可枚举

- **WHEN** 一次 run 走完 proposal 相位（含 phase_next / phase_start / executor / static_check / evaluator / phase_log）后按 change id 查询步骤审计
- **THEN** 各步骤行齐备且按时间序可枚举，同 run 行经 `run_id` 串链

#### Scenario: 审计不承担恢复

- **WHEN** 桌面重启后对遗留 active_phase 的 change 重新发起 run
- **THEN** 续走由 `phase_next` 依 eval 历史重算（StepRecord 不参与状态重建），已 pass 相位不重头执行

#### Scenario: 摘要有界且引用可达

- **WHEN** 某 static_check 步骤输出超长诊断文本后检查其 StepRecord
- **THEN** 摘要字段不超过既定上限且截断留痕，引用字段指向 checks 报告树中的完整输出位置

### Requirement: run 执行图落库

run 状态 SHALL 落 workspace 库（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 三表，`change_id` 归键，全史保留不截 last_run；attempt 经 `phase_start` max+1 分配跨 run 不撞号）。写时序 SHALL 为**每 run 两写**：run 行 SHALL 在 run 发起时以 `status=running` 建立，run 收口时 SHALL 以终态更新 + 该 run 全部节点 + 边整包**单事务**落库；MUST NOT 每步写放大。run 收口同事务处置 `ChangeRecord.active_phase` SHALL 以 `change_id` 定位记录。

执行图节点 SHALL 为**全词汇封闭集单点定义**：`RunNodeRecord` 落全部十八类步词汇（agent 阶段 `executor` / `evaluator` / `decision`、脚本阶段 `static_check` / `test_execution`、流程面步骤 `phase_start` / `phase_log`、三门 gate `verdict_gate` / `retry_gate` / `whitelist_gate`、归档步 `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `seal` / `finalize`），MUST NOT 过滤。节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`（`instance_no` 每次执行递增，feedback 重跑为独立新节点）。`RunNodeRecord` 的 `session_id` / `detail` / `output_ref` SHALL 随行落库。

执行图边 SHALL 单独落 `RunEdgeRecord`：`edge_kind ∈ { forward, feedback, retry, backtrack }`，`from_node_key` / `to_node_key` 指向节点身份键；`run_id` 二级索引与 `from_node_key` 二级索引 SHALL 均在案。前端 `deriveEdges` 列差 Δ 派生 SHALL 退役（读边表）。

`RunNodeRecord` / `RunEdgeRecord` SHALL 与既有 `StepRecord` 审计面**职责分立**：StepRecord 为相位机步骤审计行，RunNodeRecord / RunEdgeRecord 为执行图节点史 + 边史；两表 MUST NOT 互为替代。run 中途崩溃（无 finish 落包）SHALL 丢该 run 节点 / 边史——start 行经启动标定可见，为既定权衡留痕。

#### Scenario: 两写时序与零写放大

- **WHEN** 假引擎驱动一相位 run 走完（含 executor / static-check / evaluator 多步与 feedback 边）
- **THEN** 库写事务恰两次：发起时 RunRecord（running）落行、收口时同事务完成终态更新与全部 RunNodeRecord + RunEdgeRecord 整包落库；期间零每步写事务

#### Scenario: 节点与边整包原子

- **WHEN** finish 落包过程中某环节失败（如节点或边行编码错误注入）
- **THEN** 写事务整体回滚：无 RunNodeRecord / RunEdgeRecord 半包、RunRecord 终态与 finished_at 零变化

#### Scenario: 全词汇与稳定身份键

- **WHEN** 一次 run 覆盖全部十八类步词汇（含 phase_start / phase_log / 三门 gate、8 个归档步与 feedback 重跑）后查询 run_nodes / run_edges
- **THEN** 十八类词汇全落 RunNodeRecord（零过滤）；身份键 `(run_id, phase, attempt, step, instance_no)` 稳定唯一，feedback 重跑为 `instance_no` 递增新节点；边表 feedback / retry / backtrack / forward 四 kind 齐备

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

### Requirement: 启动标定与 active_phase 悬挂处置

store SHALL 提供启动标定操作：启动（workspace 库打开 / 应用装配）时发现库中 `RunRecord.status=running` 残留行 SHALL 标定为 `interrupted`（重启后 run 客观已死，库 MUST NOT 继续声称 running）；`interrupted` SHALL 仅由启动标定产生，运行期写路径 MUST NOT 写入该值。库中 run 状态词汇 SHALL 为 `running` + 终态三值（`completed` / `stopped` / `failed`）+ `interrupted`；停等态（waitingAsk / waitingConfirm）SHALL NOT 落库（应答通道活在进程内，见 desktop-change-orchestration）。

run 收口的 finish 落包 SHALL 在同一事务内处置 `ChangeRecord.active_phase`（杀悬挂）：run 死亡（终态收口）后 `active_phase` MUST NOT 停留 `Some` 悬挂——处置形态（清位 vs 转「上次运行停在 X」静态记录）由 design 定稿，但处置后详情状态面 MUST NOT 在无运行 run 时呈「运行中」假象（见 desktop-change-queries「change 详情聚合」）。active_phase 处置 MUST NOT 破坏续走语义：重新发起 run SHALL 自续走锚点推进、已 pass 相位 MUST NOT 重头执行（锚点与处置形态的关系由 design 定稿论证）。

#### Scenario: 残留 running 标定

- **WHEN** 构造库中 status=running 的残留 RunRecord（模拟中途死亡）后启动装配执行标定
- **THEN** 该行翻为 interrupted（附标定记因），再次启动无 running 残留可标定（幂等）

#### Scenario: 悬挂杀除

- **WHEN** run 在相位中途被停止（phase_log 未达、active_phase 在位）收口后查询详情
- **THEN** active_phase 已按 design 定稿形态处置，详情头部不呈「运行中 · phase · attempt」；库中无「active_phase=Some 且无在飞 run」的记录形态残留

#### Scenario: 续走不重头

- **WHEN** 悬挂处置后的 change 重新发起 run
- **THEN** 续走自锚点推进（已 pass 相位不重头执行），attempt 序延续不撞号

### Requirement: change 身份锚与 name 属性分离

建档 SHALL 为 change 铸出稳定唯一 `id`（String，UUID 形态，v4 / v7 由 design 定稿）作 `ChangeRecord` 主键与全部 per-change 数据（PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunNodeRecord / RunEdgeRecord 与 decision 槽位）的归属键；`id` MUST 在 change 全生命周期恒定（建档、推进、归档、目录改名均不变）、MUST NOT 复用（删除后同 id 不得再次铸出）。`name` SHALL 为普通属性（String，无唯一约束）：恒裸名（MUST NOT 含归档日期前缀或其他目录装饰）、可变（库内无任何寻址 / 关联 / 注册以 name 为身份锚——name 改写 MUST NOT 破坏任何归属关系）。

一切 change 寻址（库查询、命令面、前端路由、run / 归档注册表、provenance source_ref）SHALL 以 id 为准；磁盘与 git 面（change 目录定位、归档目录改名、worktree 落位、branch `change/<name>`）SHALL 由 **id → 记录 → name** 的分辨率单点供给——name 与磁盘目录名不一致（归档前缀 / 后续改名）SHALL 为合法常态。本变更 MUST NOT 引入改 name 命令（属性化为结构性收益；改名能力留后续变更，届时应协同磁盘目录改名）。

#### Scenario: id 恒定跨归档

- **WHEN** 建档 change、推进相位、归档（磁盘目录改名带 `YYYY-MM-DD-` 前缀）后按同一 id 查询
- **THEN** 记录、相位条目与 run 史经该 id 全部可达，id 逐字未变；磁盘目录名可含前缀而 name 恒裸名

#### Scenario: name 改写不破身份

- **WHEN** 库内记录 name 被改写（测试直改或后续改名能力）
- **THEN** id 不变，PhaseRecord / StepRecord / RunRecord 归属不破，列表与详情按 id 寻址照常可达

#### Scenario: id → name 分辨率供给磁盘面

- **WHEN** 对归档 change（磁盘目录 `YYYY-MM-DD-<name>`）读取产物或执行归档后目录解析
- **THEN** 解析经 id → 记录 → name 供给 `locate_change`（status + archive 树后缀扫描单点）命中带前缀目录，零 name 猜形

#### Scenario: 同名共存不顶撞

- **WHEN** 归档 change 与新建同名 active change 并存
- **THEN** 两条记录 id 相异、均可经 id 完整寻址（清单行键 id，checklist / run 史 / 会话归属互不串扰）；name 无唯一约束不构成写入阻碍

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（ChangeRecord 换锚） | 身份主键 id + name 可变属性 | `id`（String UUID 主键）/ `name`（无唯一约束属性）；native_model 版本段演进（表名 `9_2_name` → 新版本 + id 键段）；`ChangeRecordV1` 解码链退役；`new` 构造携 id 入参 |
| `crates/infra/store/src/model.rs`（per-change 表） | 归属键换锚 | `PhaseRecord.change_id` / `StepRecord.change_id` / `RunRecord.change_id`（二级索引，指向 `ChangeRecord.id`）；`ChecklistItemRecord` / `RunStepRecord` 间接归属链不变 |
| `crates/infra/store/src/store.rs`（change 域操作面 id 化） | 建档 / 落账 / run 史 / 归档 / 标定 | 全操作面签名 change → change_id；create 前置冲突查重改 name 扫描（name 无唯一索引；仅拒同名 active）；`finish_change_run` / `calibrate_interrupted_runs` 按 id join 与清位；打开路径格式版本探测与旧库作废重建（见 desktop-workspace-store） |
| `crates/infra/store/src/change_port.rs` + `crates/core/workflow/src/state.rs`（port 缝） | `ChangeStateStore` 契约 id 化 | 全方法 id 形参；`ChangeStateRecord` 增 `id`、name 降属性；命令载荷（PhaseLogCommand / BacktrackCommand / StepCommand / RunStartCommand / RunFinishCommand）`change_id` |
| `crates/infra/store/src/envelope.rs` | 信封 key 投影 | change 信封 key 随主键改呈 `id`（查看器零模型代码，只读边界不变） |
| `crates/core/workflow/src/write/`（落库 port 缝，新） | 写面 → store 的进程内缝 | core/workflow 零 infra 依赖（port 由壳层装配 store 实现）；sync 签名；事务边界由 port 语义承载 |
| 归档路径（写面 `archive` + 命令薄包装） | 双写语义 + worktree 前置引导 | id → 记录 → name 后执行目录改名；id / name 均不随改名变；无建档 id 显式拒绝；双写本体零 worktree / branch 触碰不变 |
| 时间口径 | db / 出线两段 | db i64 unix millis；出线 ISO 串 + null，转换收 queries 层单点 |
| StepRecord 审计查询（新） | 步骤历史面 | 按 change id / run 枚举；审计 only 不做恢复；摘要有界 + 引用出全量 |
