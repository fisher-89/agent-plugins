# desktop-change-state-store Specification (Delta)

## MODIFIED Requirements

### Requirement: change 流程状态 workspace 库单源与双向墙

desktop 的 change 流程状态 SHALL 以 workspace 库（app-data `workspaces/` 子树 per-root db 文件）为唯一载体，状态 SHALL NOT 再驻 `openspec/changes/<name>/workflow.json`。store SHALL 维护七个 workspace 维度模型（落所属 workspace 库，native_model 版本治理，时间戳 i64 UTC unix millis 口径）：

- `ChangeRecord`：`id`（String 主键，建档铸出的稳定唯一标识——UUID 形态，身份锚，终身恒定不复用）、`name`（String，change 名，恒裸名——可变属性，MUST NOT 作身份键）、`title`（String，人类可读标题，恒非空——创建 / 升级默认 = name，promote 路径继承 explore.title）、`workflow_type`、`created_at`、`status`（`active` | `archived`）、`archived_at`（Option）、`active_phase`（Option：phase / attempt / start_at）、`worktree`（Option：该 change 分配的 worktree 绝对路径；None = legacy 主 root change，存量语义，见 desktop-change-worktree）、`base_commit`（Option：创建基线 fork 点，调试 / UI 价值）；
- `PhaseRecord`：`id`（i64 主键，写事务内 max+1 分配）、`change_id`（二级索引，指向 `ChangeRecord.id`）、`phase`、`attempt`、`verdict`、`report`（内联 ≤2000 字）、`skipped`、`stale`、`backtrack_to` / `backtrack_reason`、executor / evaluator / decision 三会话槽位（Option）、`start_at`、`timestamp`；
- `ChecklistItemRecord`：打包主键 `(phase_id as u128) << 64 | item_index`（同相位内 item 序即自然序）、`phase_id` 二级索引、`item` / `pass` / `evidence`；
- `StepRecord`：`run_id`、`change_id`、`step_kind`（封闭集：`phase_next` | `phase_start` | `phase_log` | `backtrack` | `decision_log` | `static_check` | `test_execution`）、`status`、时间戳、有界输出摘要；
- `RunRecord`：`run_id`（String 主键，即 walker 既有 run id 词汇）、`change_id`（二级索引）、`status`（`running` | `completed` | `stopped` | `failed` | `interrupted`）、`reason`（Option）、`started_at` / `finished_at`（Option）——run 运行史主行，形态细则见「run 执行图落库」；
- `RunNodeRecord`：`run_id`（二级索引）、`change_id`、稳定身份键 `(phase, attempt, step, instance_no)`、`step_kind`（**全词汇封闭集**：`executor` | `evaluator` | `decision` | `static_check` | `test_execution` | `phase_start` | `phase_log` | `verdict_gate` | `retry_gate` | `whitelist_gate`）、`status`、`session_id`（Option，WorkerAgent 步）、`detail`（Option，有界）、`output_ref`（Option，`ExecOutput::Recorded { exec_id }` 引用）、时间戳——run 执行图节点行，形态细则见「run 执行图落库」；
- `RunEdgeRecord`：`run_id`（二级索引）、`from_node_key`（二级索引，指向源节点身份键）、`to_node_key`、`edge_kind`（封闭集：`forward` | `feedback` | `retry` | `backtrack`）、时间戳——run 执行图边行，形态细则见「run 执行图落库」。

`ChangeRecord` 的身份换锚（name 主键 → id 主键 + name 属性）SHALL 经 native_model 版本段演进（表名含模型版本与主键段，旧形态表对新读面不可见），旧形态库由 store 格式版本机制整体作废重建（见 desktop-workspace-store「workspace 库格式版本与旧库作废重建」）；MUST NOT 保留 name → id 的迁移链、解码链或手工迁移步骤（旧形态行整体作废，无迁移语义承载对象），`ChangeRecordV1` 缺列读解码链随身份换锚退役。`title` 字段 SHALL 经 native_model 版本升级（v3→v4 decode-only：`from = ChangeRecordV3`，旧记录自动升级读出 `title = name`，AgentProviderRecord / ExploreRecord 加字段先例同模式）原地演进，MUST NOT 手工迁移或 legacy 迁移层；存量库 SHALL additive 打开。`worktree` 字段为执行锚引用（读面与 run 组合消费，见 desktop-change-worktree / desktop-change-orchestration），MUST NOT 反向影响库身份锚定：workspace 库路径派生仍恒以 workspace root 为锚。

**执行图 model 重设计（不计历史兼容）**：`RunStepRecord`（`seq` + 五词汇 + 10→5 过滤）整体退役，`RunNodeRecord` / `RunEdgeRecord` 取代之；旧形态行经 store 格式版本 bump 整体作废重建，MUST NOT 保留 `RunStepRecord` → `RunNodeRecord` 迁移链 / 解码链 / 手工迁移步骤。

**双向墙**：desktop 全链（写面 / 查询 / 编排 / 命令 / 前端）MUST NOT 读或写任何 `workflow.json`——db-backed change 对 CLI 工作流不可见，反向亦然；磁盘 change 目录只承载 markdown 产物（proposal / design / tasks / specs / reports / explore；对 worktree change 该目录位于其 worktree 内，merge 前不在主仓）。存量 CLI change（有 workflow.json 无 db 记录）SHALL 零迁移零触碰——目录与文件字节原样；desktop MUST NOT 发现、入列或展示其任何形态（清单与详情均为 db 单源，见 desktop-change-queries），MUST NOT 为其建档。

#### Scenario: 状态回环读写

- **WHEN** 对某 workspace 建档 change（携铸出 id、裸名与 title）并推进若干相位（phase_log 落 eval 与 checklist、backtrack 落 stale 标记）后关闭并重开同一 db 文件查询
- **THEN** ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunNodeRecord / RunEdgeRecord 逐字段与写入一致（含 title、三会话槽位、worktree 与 base_commit、时间戳原值），状态查询不依赖任何磁盘 workflow.json

#### Scenario: 双向墙机械可验

- **WHEN** 扫描 desktop 全部源码（core / infra / commands / views）对 `workflow.json` 的读与写触点
- **THEN** 零触点（测试夹具内的字面量除外）；parse 模块已删除，desktop 不再解析该文件

#### Scenario: 历史 CLI change 零发现零触碰

- **WHEN** 主仓存在仅有 workflow.json 与 markdown 产物、无 db 记录的存量 change 目录
- **THEN** 目录与其 workflow.json 字节原样未动；清单不入列、详情不可达（零发现），不报错、不建档

#### Scenario: 旧形态数据零可达与解码链退役

- **WHEN** 打开含旧形态 `ChangeRecord` 行（name 主键）或旧 `RunStepRecord` 行的存量 workspace 库并查询
- **THEN** 打开成功且零报错；作废重建后新库为空（旧 change / run / 会话 / 步数据零残留，语义见 desktop-workspace-store）；模型源码零 name → id 迁移链 / `RunStepRecord` → `RunNodeRecord` 迁移链 / 解码链 / 手工迁移步骤

#### Scenario: 存量 v3 记录 title decode-only 升级

- **WHEN** 打开含 v3 形态 `ChangeRecord`（无 `title` 字段）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`title` 读出 = `name`，无手工迁移步骤、无 legacy 迁移层；新建档记录 `title` 按写入回读一致

#### Scenario: worktree 字段不反噬库身份

- **WHEN** 某 workspace 存在带 worktree 记录的 change，重开其 workspace 库
- **THEN** db 文件路径仍由 workspace root 身份派生（`workspaces/` 子树），worktree 路径仅作为记录字段被读取，从不参与库路径派生

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

## RENAMED Requirements

### RENAMED: run 运行史落库 → run 执行图落库

- **FROM**: `### Requirement: run 运行史落库` —— run 状态落 `RunRecord` / `RunStepRecord` 两表（五词汇 + `seq` + 10→5 过滤），每 run 两写。
- **TO**: `### Requirement: run 执行图落库` —— 内容如下：

run 状态 SHALL 落 workspace 库（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 三表，`change_id` 归键，全史保留不截 last_run——多次 run 全量留存为运行史审计面；attempt 经 `phase_start` max+1 分配跨 run 不撞号）。写时序 SHALL 为**每 run 两写**：run 行 SHALL 在 run 发起时以 `status=running` 建立（支撑启动标定），run 收口时 SHALL 以终态更新（status / reason / finished_at）+ 该 run 全部节点 + 边整包**单事务**落库；MUST NOT 每步写放大（无每步写事务）。run 收口同事务处置 `ChangeRecord.active_phase` SHALL 以 `change_id` 定位记录（归键换锚不改处置语义）。

执行图节点 SHALL 为**全词汇封闭集单点定义**：`RunNodeRecord` 落全部十类步词汇（agent 阶段 `executor` / `evaluator` / `decision`、脚本阶段 `static_check` / `test_execution`、流程面步骤 `phase_start` / `phase_log`、三门 gate `verdict_gate` / `retry_gate` / `whitelist_gate`），MUST NOT 过滤（`persisted_step` 10→5 过滤与 `RunStepRecord.seq` 退役）。节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`（`instance_no` 每次执行递增，feedback 重跑为独立新节点）。`RunNodeRecord` 的 `session_id`（WorkerAgent 步）/ `detail`（人读记因 / 摘要，有界截断）/ `output_ref`（`ExecOutput::Recorded { exec_id }` 引用，见 desktop-checks-domain）SHALL 随行落库。

执行图边 SHALL 单独落 `RunEdgeRecord`：`edge_kind ∈ { forward, feedback, retry, backtrack }`，`from_node_key` / `to_node_key` 指向节点身份键；`run_id` 二级索引（按 run 全量读边）与 `from_node_key` 二级索引（按 from_node 读后继）SHALL 均在案。前端 `deriveEdges` 列差 Δ 派生 SHALL 退役（读边表）。

`RunNodeRecord` / `RunEdgeRecord` SHALL 与既有 `StepRecord` 审计面**职责分立**：StepRecord 为相位机步骤审计行（职责与词汇不变，见「步骤审计落库可查」），RunNodeRecord / RunEdgeRecord 为执行图节点史 + 边史；两表 MUST NOT 互为替代，词汇重叠项（`static_check` / `test_execution` / decision 面）SHALL 各自单点、MUST NOT 双写同一语义行。run 中途崩溃（无 finish 落包）SHALL 丢该 run 节点 / 边史——start 行经启动标定可见（见「启动标定与 active_phase 悬挂处置」），为既定权衡留痕，MUST NOT 以每步写透补偿。

#### Scenario: 两写时序与零写放大

- **WHEN** 假引擎驱动一相位 run 走完（含 executor / static-check / evaluator 多步与 feedback 边）
- **THEN** 库写事务恰两次：发起时 RunRecord（running）落行、收口时同事务完成终态更新与全部 RunNodeRecord + RunEdgeRecord 整包落库；期间零每步写事务

#### Scenario: 节点与边整包原子

- **WHEN** finish 落包过程中某环节失败（如节点或边行编码错误注入）
- **THEN** 写事务整体回滚：无 RunNodeRecord / RunEdgeRecord 半包、RunRecord 终态与 finished_at 零变化

#### Scenario: 全词汇与稳定身份键

- **WHEN** 一次 run 覆盖全部十类步词汇（含 phase_start / phase_log / 三门 gate 与 feedback 重跑）后查询 run_nodes / run_edges
- **THEN** 十类词汇全落 RunNodeRecord（零过滤）；身份键 `(run_id, phase, attempt, step, instance_no)` 稳定唯一，feedback 重跑为 `instance_no` 递增新节点；边表 feedback / retry / backtrack / forward 四 kind 齐备

#### Scenario: 边表两向查询

- **WHEN** 按 `run_id` 全量读某 run 的边、按 `from_node_key` 读某节点后继
- **THEN** 两二级索引均可达且结果一致；`from_node_key` 后继查询能枚举 feedback / forward / retry / backtrack 出边

#### Scenario: 全史保留与跨 run 不撞号

- **WHEN** 同一 change（同一 id）先后发起两次 run（第二次自续走锚点推进）
- **THEN** 两 run 的 RunRecord 与各自 RunNodeRecord / RunEdgeRecord 全量在库，同 phase 的 attempt 经 max+1 分配递增不撞号，按 `(run_id, phase, attempt, step, instance_no)` 可稳定分层枚举

#### Scenario: 崩溃丢节点边史为既定权衡

- **WHEN** run 运行中途进程被 kill（finish 落包未达）后重启查询
- **THEN** 该 run 的 RunRecord 经启动标定为 interrupted、其 RunNodeRecord / RunEdgeRecord 零行（过程史不可回放），attempt 级评估史仍可达自 PhaseRecord；库中无每步写透补偿痕迹

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（run 执行图换锚） | 节点 + 边模型 | `RunNodeRecord`（全词汇 + 稳定身份键 `(run_id, phase, attempt, step, instance_no)` + `output_ref`）/ `RunEdgeRecord`（`edge_kind` 四值 + 两二级索引）；`RunStepRecord` 退役；native_model 版本段演进 |
| `crates/infra/store/src/store.rs`（run 域操作面重设计） | 节点 / 边落库与查询 | `run_start`（建 running 行）/ `run_finish`（终态 + 节点整包 + 边整包单事务 + active_phase 处置）/ `run_nodes` / `run_edges` 查询（`run_id` / `from_node_key` 索引）；启动标定 sweep |
| `crates/core/orchestration/src/run_history.rs` | 节点 + 边投影 | `persisted_step` 10→5 过滤退役；`RunNodeRecord` / `RunEdgeRecord` 载荷投影 |
| `crates/core/orchestration/src/control.rs` | 节点 + 边累积器 | `RunEntry` 自持 nodes + edges（emit 序、`instance_no` 分配）；进程内零 IO 不变量保持 |
| `crates/core/workflow/src/queries/detail.rs` | 读面投影 | `runs[].nodes` / `runs[].edges` 出线（全词汇、稳定身份键、边 kind 四值），取代 `steps` |
| `crates/infra/exec` | 输出记录 | `ExecOutput::Recorded { exec_id }` → `RunNodeRecord.output_ref` 引用（见 desktop-checks-domain） |
