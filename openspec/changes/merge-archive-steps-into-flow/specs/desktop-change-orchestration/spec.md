# desktop-change-orchestration Specification (Delta)

## Purpose

Desktop 后端 change run 编排运行时（薄图 walker，core/orchestration）：对 PGE 工作流执行计划图驱动的 phase → step 循环，四类节点（WorkerAgent / ToolStep / Git / Gate）统一可观测；路由权威驻 core/workflow 写面（walker 零自持转移规则）；提供运行控制命令面（发起 / 停止 / 应答 / 确认）与决策协议。归档作为第 10 个路由相位（确定性相位，10 步序列）并入 walker：walk 主循环覆盖 archive 相位、独立 vcs 执行缝承载 git 步、空 checklist 落账收口，run 收口语义由「8 相位 pass」扩为「8 相位 pass + 归档相位完成」。

## RENAMED Requirements

- FROM: `### Requirement: 三类节点统一 walker 与可观测均一`
- TO: `### Requirement: 四类节点统一 walker 与可观测均一`

## MODIFIED Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行**计划图驱动的 step 循环**：每相位依序迭代其 `PhaseDefinition.steps`（`StepDefinition` 序列，kind = exec | agent | gate | git，定义见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），不再在 `drive()` 内硬编码步序。每步执行后 SHALL 问写面 `phase_next` 决定推进 / 重试 / 决策（路由权威不变）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个门禁布局常量 SHALL 退役（门禁步由 step 定义声明）。archive 相位 SHALL 为 walker 的第 10 个路由相位（`REQUIREMENT_TABLE` 第 9 相位，`PhaseDefinition.executor=None` / `evaluator=None`），其 10 步序列按序执行；drive 主循环 MUST NOT 保留 `next.evaluator.is_none()` 的冗余提前检查（真正消费 evaluator 的 `step_evaluator` / `step_executor` 自有 None 检查兜底）。

相位内 step 序列的特殊控制流 SHALL 以 step 级语义表达，MUST NOT 以 walker 内 step 类型特判实现：

- **feedback 边**（static-check / test-execution 失败 → 同一 agent step 修复 → 重跑）：失败步 `on_fail` 声明的定向反馈循环，独立计数 ≤5，超限升格相位 fail；每次重跑为 `instance_no` 递增的**新节点**，图上以 `feedback` 边表达；
- **verdict 解析失败**：终态失败停给用户，MUST NOT 臆测 verdict、MUST NOT 静默跳过；
- **归档步失败**：archive 相位步 `on_fail=None` 的失败即 `Terminal`（不唤决策、不进相位重试）；
- **phase 间 confirm 停等**：仍为 run 级 `auto_next_phase` 开关，并支持「按指定 phase 停止」（archive 为固定停止点）。

红线保持：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份（10 列含 archive 与 code-analyze 占位列），MUST NOT 上位为路由权威。重启 run SHALL 自 active_phase 续走（active_phase 持久化于 db `ChangeRecord`，已 pass 相位 MUST NOT 重头执行，续走由 `phase_next` 依 eval 历史重算）。写面全部落库载体为 workspace 库，walker 对载体无感知。

#### Scenario: 相位内计划步全循环自动走完

- **WHEN** 以假引擎 + 假写面 + 假 vcs port 驱动某相位 run（含 archive 相位 10 步）
- **THEN** walker 依该相位 `PhaseDefinition.steps` 序逐 step 执行（exec / agent / gate / git 按声明），pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: archive 相位确定性承接

- **WHEN** walker 走到 archive 相位且 10 步全 pass
- **THEN** 相位以 Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog 承接（零 executor / evaluator 会话），末尾 PhaseLog 空 checklist 落账 pass 后推进 done

#### Scenario: evaluator 检查下沉不破坏安全不变量

- **WHEN** 审查 drive 主循环与 `step_evaluator` / `step_executor`
- **THEN** drive 无 `next.evaluator.is_none()` 检查；声明 Evaluator / VerdictGate 步而 `evaluator=None` 的漂移仍被 `step_evaluator` 的 None 检查拦截；archive 相位（`evaluator=None` 且 steps 无 Evaluator 步）零拦截

#### Scenario: fail 重试与预算上限

- **WHEN** evaluator verdict 为 fail 且重试未超预算
- **THEN** walker 不唤决策 agent 自走重试同相位；连续 fail 达 5 次后停止自走并按决策协议分叉

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 db `ChangeRecord.active_phase`=implement 的下一 attempt 续走，已 pass 的 proposal / dev-design 等相位不重新执行；archive 相位完成后 run completed

### Requirement: 自动确认运行模式（auto_next_phase）

Desktop run SHALL 支持「自动确认」运行模式：`RunRequest.auto_next_phase`（bool）为 run 级属性、发起时定格，经运行控制命令面发起入口透传。`auto_next_phase=true` 时，walker SHALL 在 phase 间确认点跳过 `ConfirmWait` 广播与 `wait_confirm` 挂起，直接推进下一相位；auto 模式 run MUST NOT 进入 `WaitingConfirm` 状态、MUST NOT 产生 `ConfirmWait` 事件。`auto_next_phase=false`（默认）时既有停等语义不变。

`auto_next_phase` SHALL 泛化出「按指定 phase 停止」能力：run SHALL 支持在指定相位前停止（stop-before-phase，形态由 design 定稿——`RunRequest.stop_before: Option<String>` 或等价策略），archive 为该能力的固定停止点。无论 auto / 手动模式，run 走完 8 个路由相位后 SHALL 停在 archive 前（archive 为独立手动执行门）：MUST NOT 自动推进进入 archive 相位；用户显式确认后才进入 archive 相位。

语义边界：

- **ask 中断不自动应答**：auto 模式下决策 agent 的 ask 到来 SHALL 照常进入 `WaitingAsk` 停等用户应答；自动确认只作用于 phase 间停等。
- **归档前停等**：全 8 路由相位 pass 后 SHALL 停在 archive 前（"Ready for archiving" 改写为「等待确认进入归档」），MUST NOT 自动归档；archive 相位完成后 run 才 `completed`。

运行中停等节奏 MUST NOT 可切换：节奏变更 SHALL 经停止后重发（自 `active_phase` 续走）；auto 模式下用户干预的合法动作与既有运行面一致（停止）。

#### Scenario: auto run 直达 archive 前停等

- **WHEN** 以 auto_next_phase=true 发起多相位 run（各相位 evaluator 全 pass）
- **THEN** 更新流逐相位自动推进（零 `ConfirmWait`），至 archive 前停等（stop-before-archive），未确认不进 archive 相位、未 completed；用户确认后 archive 相位执行、run completed

#### Scenario: 手动模式语义不变

- **WHEN** 以 auto_next_phase=false（默认）发起同一 run
- **THEN** `ConfirmWait` 逐相位流出、挂起至 `change_flow_confirm` 应答，proceed=false / 等待期间取消 → 受控 `stopped`（与既有契约逐字一致）

#### Scenario: auto run 的 ask 仍停等人

- **WHEN** auto 模式 run 中决策 agent 输出 ask 动作（重试预算耗尽后的提问）
- **THEN** run 照常进入 `WaitingAsk` 并呈现问题与选项，walker 挂起等应答；MUST NOT 自动选择任何选项，用户应答后 run 继续

#### Scenario: 发起定格与停止重发

- **WHEN** 运行中的 run 需要变更停等节奏
- **THEN** 唯一路径为停止当前 run 后以目标节奏重新发起（自 `active_phase` 续走，已 pass 相位不重头执行）；运行中不存在节奏切换命令或事件

### Requirement: 四类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为四类节点（step 定义声明 kind）：

- **WorkerAgent 节点**（executor / evaluator / decision / spec_sync / resolve_conflicts 的 agent 会话步）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界 + preflight / seal 内联校验与写面步）：无智能；
- **Git 节点**（commit / rebase / merge / finalize vcs 步，落 vcs 边界 `crates/infra/vcs`）：经独立 vcs 缝 `Arc<dyn ArchiveVcsPort>` 进程 spawn 执行；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep / Git MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord`）∪ 进程内注册表在飞 run（`RunEntry` 节点 + 边累积器）** × 按 provenance 反查的 session 集；执行图节点 SHALL 以稳定身份键 `(run_id, phase, attempt, step, instance_no)` 寻址，边 SHALL 以 `RunEdgeRecord` 四值 `edge_kind ∈ { forward, feedback, retry, backtrack }` 显式落库。步词汇 SHALL 为全词汇封闭集（agent 阶段 + 脚本阶段 + 流程面步骤 + 三门 gate + 8 个归档步），落库与回显同一份词汇。已收口 run 的任一节点状态与边 SHALL 可由库读史单独重算得出。

#### Scenario: ToolStep / Git 失败同等可见

- **WHEN** static-check / test-execution spawn 步、vcs 步（commit / rebase / merge / finalize）或写面相位机步以非零退出 / 显式 Err 收场
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: 节点与边两源派生可重算

- **WHEN** 审查编排运行时的持久化面与读路
- **THEN** runs / run_nodes / run_edges 三表在案、在飞 run 节点与边驻进程内注册表（RunEntry）；已收口 run 的任一节点状态与边均可由库读史重算得出，不依赖进程内存

#### Scenario: 归档步词汇上图与边四值可辨

- **WHEN** 一次 run 含 archive 相位 10 步（含 git 步与空 checklist 落账）
- **THEN** 归档 8 个新步词节点全落 `RunNodeRecord`（四类节点类型可辨），串行 forward 边落边表；同 run 若另含 feedback / retry / backtrack，四 kind 边齐备且显式落库

### Requirement: 零 CLI 子进程写通道与 crate 依赖方向

Walker MUST NOT 经任何 CLI 子进程变更 change 流程状态（dev-team CLI 子命令面不存在）：所有 workflow 状态变更 SHALL 经 core/workflow 写面进程内直调（desktop-workflow-write-face 契约，落库 workspace 库），MUST NOT 读写 workflow.json。依赖方向 SHALL 为 orchestration → workflow；`crates/infra/devteam` MUST NOT 存在（整体删除）。进程 spawn 例外家族为检查域门禁（static-check / test-execution，落 `crates/infra/checks`）与 vcs 域归档 git 步（commit / rebase / merge / finalize，落 `crates/infra/vcs`）——两者均经 port 缝下沉、spawn 不进 core（core/orchestration 自身零进程 spawn；检查域见 desktop-checks-domain，vcs 域见 desktop-crate-layout）。

#### Scenario: 零 CLI 写触点

- **WHEN** 审查编排运行时与命令组源码中对 change 流程状态的变更触点
- **THEN** 零 CLI 子进程调用、零 devteam 子进程依赖、零 workflow.json 读写；全部状态变更经进程内写面调用返回确认并落库

#### Scenario: spawn 边界保持（checks 与 vcs 双例外）

- **WHEN** 审查 core/orchestration 源码的进程创建调用
- **THEN** orchestration 模块内零进程 spawn；检查域 spawn（static-check / test-execution）经 port 缝由 `crates/infra/checks` 承载，git 步 spawn 经 `ArchiveVcsPort` 缝由 `crates/infra/vcs` 承载

### Requirement: 会话挂靠 provenance 与节点状态派生

Walker 发起的 executor / evaluator / decision / spec_sync / resolve_conflicts 会话 SHALL 携带 provenance：`source="change"`、`source_ref=<id>/<phase>/<role>/<attempt>`（身份段为 change id——单一身份锚贯穿，name 可变不入寻址键；归档步两会话的 source_ref 定式由 design 定稿，MUST 在同 phase 同 attempt 内唯一，避免 spec_sync 与 resolve_conflicts 撞号）。节点运行态 SHALL 由按 provenance 反查的会话集派生（`agentSessions` 按 source / sourceRef 过滤既有面）；会话转录 SHALL 可观测：运行中实时流、结束后重放一致。会话停止 SHALL 复用 StopRegistry 既有终止面，对已终态目标幂等忽略。

#### Scenario: 按归属反查会话集

- **WHEN** 某 change 的 implement#2 相位已发起 executor 与 evaluator 会话（或 archive 相位已发起 spec_sync 会话）
- **THEN** 按 `source="change"` + `source_ref="<id>/<phase>/<role>/<attempt>"` 等过滤查询可枚举对应会话及其运行态，派生出节点状态，无需任何额外注册表

#### Scenario: 转录可观测与停止

- **WHEN** executor 会话运行中用户请求停止
- **THEN** 该会话经 StopRegistry 收敛为 `stopped`，walker 收到终止终态并按停止收敛处理 run；对已终态会话的重复停止请求幂等忽略

### Requirement: verdict 解析与 phase-log 代写

Evaluator 会话 SHALL 依 prompt 约定在最终消息输出 checklist JSON（不自调 MCP `phase_log`）。Walker SHALL 解析该 JSON 并代调写面 `phase_log`（进程内）落 eval 记录（PhaseRecord + ChecklistItemRecord 原子落库）；解析失败 SHALL 将运行停给用户（呈现原始输出与失败原因），MUST NOT 臆测 verdict、MUST NOT 静默跳过落账。checklist JSON 结构 SHALL 瘦身为 `{ verdict, report, checklist }`（checklist 逐项 `{ item, pass, evidence }`）；phase / attempt / skipped SHALL 由 walker 盖戳落账（evaluator 不回声），`EvaluatorChecklist` 解析类型 MUST NOT 要求 phase / attempt / skipped 字段。确定性相位（test-execution 机械 checklist、archive 空 checklist）SHALL 走 `step_phase_log` 的独立分支，不经 evaluator verdict 解析。

#### Scenario: verdict 代写落账

- **WHEN** evaluator 最终消息输出合法 checklist JSON（verdict=pass，无 phase / attempt / skipped 字段）
- **THEN** walker 以该 checklist 盖 phase / attempt / skipped 后经写面 `phase_log` 落 eval 记录，`phase_next` 随后推进

#### Scenario: 解析失败停给用户

- **WHEN** evaluator 最终消息不含可解析的 checklist JSON（结构漂移或空输出）
- **THEN** run 停止并呈现 evaluator 原始输出与解析失败原因，状态库无新增 eval 记录，用户可显式重试该 attempt

### Requirement: 运行控制命令面

命令面 SHALL 提供 change run 的控制入口，命令体收拢为编排运行时公共 API 的薄包装（沿三件事纪律）：

- **发起**：按 change id 发起 run（id → 记录解析 worktree / name），`auto_next_phase` 参数（bool；false=默认停等节奏，true=自动确认模式）与「按指定 phase 停止」参数（形态 design 定稿；archive 为固定停止点）随发起定格并透传 walker；发起 SHALL 先解析 exec root；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），RunRecord 起始行（status=running）SHALL 随发起建立；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态（archive 相位进行中的停止复用本命令）；对非运行态目标幂等忽略；
- **应答**：ask 中断态下回传用户应答；
- **确认**：phase 间停等点（含 archive 前停等点）的用户确认（继续 / 终止）。

**读路统一（推拉反转）**：`get_change_detail`（或其扩展）SHALL 一次返回「库读史 ∪ 内存在飞 run」统一视图——库面 runs / steps 读史并入在飞 run 的状态 / 停等与 ask 载荷 / RunEntry 步表，合并发生在读时；客户端 MUST NOT 双命令拼接（`useChangeDetail` + `useChangeFlowRun` 双真相源拼缝退场；归档链 `useArchiveFlow` 第三真相源随 archive_flow 命令面退役）。`RunUpdate` SHALL 降位为变更通知：客户端收通知后自行重查统一视图。stop / confirm / answer 控制面 SHALL 维持请求-应答形态不变。

运行状态 Channel SHALL 与 agent 执行流同构。发起前置校验 SHALL 覆盖：目标 change 已建档、无同 workspace 同 change 并行 run——并行冲突键 SHALL 为 `(workspace root, change id)` 复合——无建档的 change SHALL 显式拒绝发起。

#### Scenario: 发起提前 resolve 与通知流

- **WHEN** 前端发起某 change 的 run
- **THEN** invoke 在 run 进入运行态后即 resolve（run 起始行已落），后续节点 / 相位状态变化经 Channel 以变更通知流出，客户端收通知重查统一视图；终态以通知收尾

#### Scenario: 通知触发重查统一视图

- **WHEN** run 运行期间某步状态变化通知到达（含 archive 相位步）
- **THEN** 客户端经单命令统一视图重查（库读史 ∪ 在飞 run 含步表），无第二命令拼接、无前端载荷累积

#### Scenario: 归档前停等确认续走

- **WHEN** run 到达 archive 前停等点，用户在停等面选择继续（或终止）
- **THEN** 确认后 walker 进入 archive 相位执行至收口；终止则 run 收敛为受控终态；停止命令对 archive 相位运行中目标幂等生效

#### Scenario: 停止幂等

- **WHEN** 对已收口（completed / stopped / failed）的 run 调停止命令
- **THEN** 幂等忽略：状态不变、不报错、不产生新事件

#### Scenario: 发起前置校验与复合键并行

- **WHEN** 对无 db 建档的 change 发起 run、或同 workspace 同一 change 已有运行中的 run 再次发起
- **THEN** 发起命令显式 `Err`（呈现未建档或并行冲突原因），不进入运行态、不落任何账；不同 change / 不同 workspace 不受影响

#### Scenario: stop-before-phase 随发起透传

- **WHEN** 前端发起 run（含「按指定 phase 停止」参数），随后另一 run 以不同停止点发起
- **THEN** 两 run 各自按发起参数定格停止点；archive 为固定停止点不随参数省略而自动归档

### Requirement: 计划图与执行图对齐与步粒度回退

计划图（源码，desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）与执行图（db，desktop-change-state-store「run 执行图落库」）SHALL 经 **step kind + phase + attempt + instance_no** 对齐：执行图节点是计划 step 的运行时实例，节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`；执行图边 SHALL 显式落 `RunEdgeRecord`，`edge_kind ∈ { forward, feedback, retry, backtrack }`——archive 相位六段串行（实为 10 步）为 forward 边，无回边。计划图 MUST NOT 落库（定义恒为编译期静态）；执行图 MUST NOT 上位为路由权威。回退（backtrack）的语义单位 SHALL 保持 phase 粒度；本变更 MUST NOT 把回退载荷改为 step 粒度。

#### Scenario: 节点身份稳定且 feedback 独立

- **WHEN** 同 phase 同 attempt 内 static-check 失败触发 executor 修复重跑，随后 phase fail 重试该相位
- **THEN** executor 首次与修复各为 `instance_no` 递增的独立节点，feedback 边指向前一节点；phase 重试产生新 attempt，retry 边指向新 attempt 首节点

#### Scenario: 归档步为 forward 边

- **WHEN** archive 相位一次 run 收口后查询执行图
- **THEN** 归档 10 步节点（含 8 个新步词）以 forward 边串行落 `RunEdgeRecord`，零 feedback / retry / backtrack 边；`run_id` / `from_node_key` 两二级索引可达

#### Scenario: 边四值与查询

- **WHEN** 审查一次含前进 / 反馈 / 重试 / 回跳的 run 执行图
- **THEN** `RunEdgeRecord` 四 kind 齐备且 `run_id` / `from_node_key` 两二级索引可达；`backtrack` 边指向白名单内目标相位首节点

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.36` 升级为 `0.4.37`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44` 与三类交付产物）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.37；`plugins/dev-team` 版本与交付产物零改动

## ADDED Requirements

### Requirement: 确定性相位空 checklist 落账

archive 相位（无 evaluator、无 checklist 的确定性相位）SHALL 在末尾 PhaseLog 步以空 checklist 落账：`step_phase_log` 在 test_outcome 分支之后、verdict 分支之前插入 archive 分支——构造 `PhaseLogInput { phase: "archive", report: String::new(), checklist: vec![], skipped: false, executor/evaluator/decision 三槽位 None }` 并直接 `run_phase_log`。落账 SHALL 复用既有 `derive_verdict([])=Pass` 口径（空 checklist = 无条件通过），零 `if phase == archive` 特判、零新增落账函数。三分流按「产物在否」分离：archive 相位无 TestExecution step（test_outcome 恒 None）、无 Evaluator/VerdictGate step（verdict 恒 None），仅命中空 checklist 分支。

#### Scenario: 空 checklist 落账 pass

- **WHEN** archive 相位 10 步走完进入 PhaseLog 步
- **THEN** `run_phase_log` 以空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord，`active_phase` 清位（phase_log 单事务），`phase_next` 随后路由 done

#### Scenario: 三分流互不误判

- **WHEN** 分别驱动评估相位（verdict 产物）、test-execution 相位（test_outcome 产物）与 archive 相位（空 checklist）
- **THEN** `step_phase_log` 三分支按产物在否分离，无按 phase 名判断、无跨分支误判

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration`（运行时域，tokio） | 薄图 walker：相位循环、四类节点执行、Gate 分支 | 红线：零相位转移规则（每步过渡问写面 phase-next）；ToolStep / Git 节点化（不内联）；节点状态纯派生（db 状态 × provenance 反查）；模块内零进程 spawn（checks / vcs 经 port 缝下沉）；依赖方向 orchestration → workflow；对落库载体无感知 |
| `crates/core/orchestration/src/port.rs` | 「相位机 + 工具步 + vcs」三缝 | ToolStepPort 进程内直调 core/workflow 写面；WorkerAgentPort 契约不变；新增 `ArchiveVcsPort`（git 步进程 spawn，实现驻 infra/vcs）；测试假引擎 + 假写面 + 假 vcs 三缝直驱 |
| `crates/core/orchestration/src/steps.rs`（LocalToolSteps） | 相位机步进程内直调写面 | 直调面签名不变；每步结果落 StepRecord 审计行（经写面）；Preflight / Seal 内联步落此 |
| `crates/core/orchestration/src/walker.rs` | 相位循环与步分派 | `run_step` 增 8 个归档步分支（preflight / commit / rebase / resolve_conflicts / merge / spec_sync / seal / finalize）；`StepContext` 增 `vcs: Arc<dyn ArchiveVcsPort>`；`PhaseCursor` 增 `rebase_conflicts`；删除 `next.evaluator.is_none()` 检查；`step_phase_log` 增空 checklist archive 分支；收口语义改写 |
| `crates/core/orchestration/src/state.rs` | 执行图步词汇 | `ChangeStepKind` + 8 个归档步词（与 `RunStepKind::as_str` 同词面）；`ChangeStepState` / `NodeKey` 语义不变 |
| `crates/core/orchestration/src/snapshot.rs` | ChangeDetail 只读装配 | 读源改 workspace 库 change 状态（经 queries）；磁盘扫描保留产物发现 |
| `crates/core/orchestration/src/verdict.rs` + `decision.rs`（不改） | 结构化输出解析 | 封闭 schema（checklist 信封 + 决策四动作封闭集）；解析失败显式失败停给用户 |
| `crates/core/orchestration/src/prompt.rs` | prompt 组装与运行时插值 | executor / evaluator 模板取自 core/workflow 相位表单源；spec 同步 / 解冲突 prompt 迁入 walker 步（prompt_ref 静态引用） |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现 | compose_turn 新会话 + StopRegistry 终止 + 密封转录；provenance `source="change"`；spawn 不进 core |
| `src/commands/` change_flow 命令组 | 运行控制 IPC 面 | 发起前置校验改 db 建档校验；发起 / 停止 / 应答 / 确认契约不变；归档发起 / 停止收编（archive_flow 命令面退役）；「按指定 phase 停止」参数透传 |
| `crates/core/workflow`（写面 + 读面） | change 状态域权威 | 读写同屋檐、落库 workspace 库；walker 经进程内直调消费，不复制路由语义 |
| `crates/infra/store`（复用） | 会话记录与反查 + change 状态落库 | `agentSessions` 按 source / sourceRef 过滤派生节点状态；不建 archive 表 |
| `crates/infra/vcs`（vcs-runtime） | git 步进程执行 | `commit_all` / `rebase_branch` / `rebase_continue` / `rebase_abort` / `ff_merge` / `commit_paths` / `current_branch` / `branch_merged` / 干净探测；允许 spawn，禁 Tauri |
| `src/types/generated/bindings.ts`（重导） | IPC 类型跟随 | 命令面增删与 DTO 演进再生；`bindings:check` 守卫拦截过期生成物 |
