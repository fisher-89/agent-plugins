# desktop-change-orchestration Specification

## Purpose

Desktop 后端 change run 编排运行时（薄图 walker，core/orchestration）：对 PGE 工作流执行硬编码单图相位循环，三类节点（WorkerAgent / ToolStep / Gate）统一可观测；路由权威驻 core/workflow 写面（walker 零自持转移规则）；提供运行控制命令面（发起 / 停止 / 应答 / 确认）与决策协议。

## Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行**计划图驱动的 step 循环**：每相位依序迭代其 `PhaseDefinition.steps`（`StepDefinition` 序列，kind = exec | agent | gate | git，定义见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），不再在 `drive()` 内硬编码步序。每步执行后 SHALL 问写面 `phase_next` 决定推进 / 重试 / 决策（路由权威不变）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个门禁布局常量 SHALL 退役（门禁步由 step 定义声明）。initialize SHALL 为 walker 的第 1 个路由相位（两表表头一次性基线检查相位，执行体见「initialize 基线检查相位执行体」）；archive 相位 SHALL 为 walker 的第 11 个路由相位（`REQUIREMENT_TABLE` 第 10 相位，`PhaseDefinition.executor=None` / `evaluator=None`），其 11 步序列按序执行；drive 主循环 MUST NOT 保留 `next.evaluator.is_none()` 的冗余提前检查（真正消费 evaluator 的 `step_evaluator` / `step_executor` 自有 None 检查兜底）。

相位内 step 序列的特殊控制流 SHALL 以 step 级语义表达，MUST NOT 以 walker 内 step 类型特判实现：

- **feedback 边**（门禁步失败 → 同一 agent 会话修复 → 重跑）：失败步 `on_fail` 声明的定向反馈循环，独立计数 ≤5，超限升格相位 fail；每次重跑为 `instance_no` 递增的**新节点**，图上以 `feedback` 边表达。注入对象分角色：static-check / test-execution 失败注入本相位 executor 会话，verdict 解析失败注入本相位 evaluator 会话（initialize 相位例外——其检查步 `on_fail=Terminal`，无反馈边）；
- **verdict 解析失败**：解析错误诊断反馈边续注同一 evaluator 会话修复重跑（MUST NOT 臆测 verdict、MUST NOT 静默跳过）；超限升格相位 fail 进相位重试 / 决策分叉；
- **归档步失败**：archive 相位步 `on_fail=None` 的失败即 `Terminal`（不唤决策、不进相位重试）；`Cleanup` 为唯一例外——尽力步恒以 `passed` 收口（失败仅 step detail 警告，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；
- **phase 间 confirm 停等**：仍为 run 级 `auto_next_phase` 开关，并支持「按指定 phase 停止」（archive 为固定停止点）。

红线保持：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份（requirement 10 列含 initialize 与 archive、test-only 7 列），MUST NOT 上位为路由权威。重启 run SHALL 自 active_phase 续走（active_phase 持久化于 db `ChangeRecord`，已 pass 相位 MUST NOT 重头执行，续走由 `phase_next` 依 eval 历史重算）。写面全部落库载体为 workspace 库，walker 对载体无感知。

#### Scenario: 相位内计划步全循环自动走完

- **WHEN** 以假引擎 + 假写面 + 假 vcs port 驱动某相位 run（含 archive 相位 11 步）
- **THEN** walker 依该相位 `PhaseDefinition.steps` 序逐 step 执行（exec / agent / gate / git 按声明），pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: archive 相位确定性承接

- **WHEN** walker 走到 archive 相位且 11 步全 pass
- **THEN** 相位以 Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog 承接（零 executor / evaluator 会话），末尾 PhaseLog 空 checklist 落账 pass 后推进 done

#### Scenario: evaluator 检查下沉不破坏安全不变量

- **WHEN** 审查 drive 主循环与 `step_evaluator` / `step_executor`
- **THEN** drive 无 `next.evaluator.is_none()` 检查；声明 Evaluator / VerdictGate 步而 `evaluator=None` 的漂移仍被 `step_evaluator` 的 None 检查拦截；archive 相位（`evaluator=None` 且 steps 无 Evaluator 步）零拦截

#### Scenario: verdict fail 直接决策分叉

- **WHEN** 任意带 evaluator 相位落 verdict=fail 条目（最新 fail 条目未挂决策会话）
- **THEN** 写面 `phase_next` 不自走重路由同相位（盲目重跑的会话消耗高于一次决策），立即返回 verdict fail 决策分叉触发；窗口 fail 数达上限（含决策 retry 兑现的重跑）时以重试上限变体标注。确定性相位（initialize / archive：executor / evaluator 双缺席）不触发，fail 落账照常同相位重路由

#### Scenario: fail 重试与预算上限

- **WHEN** evaluator verdict 为 fail 且重试未超预算
- **THEN** walker 不唤决策 agent 自走重试同相位；连续 fail 达 5 次后停止自走并按决策协议分叉

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 db `ChangeRecord.active_phase`=implement 的下一 attempt 续走，已 pass 的 initialize / proposal / dev-design 等相位不重新执行；archive 相位完成后 run completed

#### Scenario: test-execution 相位确定性承接

- **WHEN** walker 走到 test-execution 相位且 runner 结论 pass
- **THEN** 相位以 runner 门禁步承接（零 executor / evaluator 会话），机械 checklist 代写 `phase_log` 落账 pass 后推进；fail / error 走反馈边语义（见 desktop-test-execution）

### Requirement: 自动确认运行模式（auto_next_phase）

Desktop run SHALL 支持「自动确认」运行模式：`RunRequest.auto_next_phase`（bool）为 run 级属性、发起时定格，经运行控制命令面发起入口透传。`auto_next_phase=true` 时，walker SHALL 在 phase 间确认点跳过 `ConfirmWait` 广播与 `wait_confirm` 挂起，直接推进下一相位；auto 模式 run MUST NOT 进入 `WaitingConfirm` 状态、MUST NOT 产生 `ConfirmWait` 事件。`auto_next_phase=false`（默认）时既有停等语义不变。

`auto_next_phase` SHALL 泛化出「按指定 phase 停止」能力：run SHALL 支持在指定相位前停止（stop-before-phase，形态由 design 定稿——`RunRequest.stop_before: Option<String>` 或等价策略），archive 为该能力的固定停止点。无论 auto / 手动模式，run 走完全部前序路由相位（一次性 initialize + 内容相位——requirement 9 个、test-only 6 个）后 SHALL 停在 archive 前（archive 为独立手动执行门）：MUST NOT 自动推进进入 archive 相位；用户显式确认后才进入 archive 相位。

语义边界：

- **ask 中断不自动应答**：auto 模式下决策 agent 的 ask 到来 SHALL 照常进入 `WaitingAsk` 停等用户应答；自动确认只作用于 phase 间停等。
- **归档前停等**：全部前序路由相位（含一次性 initialize）pass 后 SHALL 停在 archive 前（"Ready for archiving" 改写为「等待确认进入归档」），MUST NOT 自动归档；archive 相位完成后 run 才 `completed`。

运行中停等节奏 MUST NOT 可切换：节奏变更 SHALL 经停止后重发（自 `active_phase` 续走）；auto 模式下用户干预的合法动作与既有运行面一致（停止）。

#### Scenario: auto run 直达 archive 前停等

- **WHEN** 以 auto_next_phase=true 发起多相位 run（各相位 evaluator 全 pass）
- **THEN** 更新流逐相位自动推进（零 `ConfirmWait`，initialize 收口后照常直进），至 archive 前停等（stop-before-archive），未确认不进 archive 相位、未 completed；用户确认后 archive 相位执行、run completed

#### Scenario: 手动模式语义不变

- **WHEN** 以 auto_next_phase=false（默认）发起同一 run
- **THEN** `ConfirmWait` 逐相位流出（initialize 收口后照常停等）、挂起至 `change_flow_confirm` 应答，proceed=false / 等待期间取消 → 受控 `stopped`（与既有契约逐字一致）

#### Scenario: auto run 的 ask 仍停等人

- **WHEN** auto 模式 run 中决策 agent 输出 ask 动作（verdict fail 决策分叉的提问）
- **THEN** run 照常进入 `WaitingAsk` 并呈现问题与选项，walker 挂起等应答；MUST NOT 自动选择任何选项，用户应答后 run 继续

#### Scenario: 发起定格与停止重发

- **WHEN** 运行中的 run 需要变更停等节奏
- **THEN** 唯一路径为停止当前 run 后以目标节奏重新发起（自 `active_phase` 续走，已 pass 相位不重头执行）；运行中不存在节奏切换命令或事件

### Requirement: 四类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为四类节点（step 定义声明 kind）：

- **WorkerAgent 节点**（executor / evaluator / decision / spec_sync / resolve_conflicts 的 agent 会话步）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界 + preflight / seal 内联校验与写面步）：无智能；
- **Git 节点**（clean / commit / rebase / merge / cleanup / finalize vcs 步，落 vcs 边界 `crates/infra/vcs`）：经独立 vcs 缝 `Arc<dyn ArchiveVcsPort>` 进程 spawn 执行；initialize 相位 `clean` 为基线恢复步（`restore_baseline`，见「initialize 基线检查相位执行体」）；`cleanup` 为尽力步——失败仅 step detail 警告、步恒 `passed`（不呈失败态，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep / Git MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord`）∪ 进程内注册表在飞 run（`RunEntry` 节点 + 边累积器）**；节点行 SHALL 直接携 `session_id`（会话转录经 session id 直查，不再按 provenance 反查会话集）；执行图节点 SHALL 以稳定身份键 `(run_id, phase, attempt, step, instance_no)` 寻址，边 SHALL 以 `RunEdgeRecord` 四值 `edge_kind ∈ { forward, feedback, retry, backtrack }` 显式落库。步词汇 SHALL 为全词汇封闭集（agent 阶段 + 脚本阶段 + 流程面步骤 + 三门 gate + 9 个归档步 + initialize 基线步 clean），落库与回显同一份词汇。已收口 run 的任一节点状态与边 SHALL 可由库读史单独重算得出。

#### Scenario: ToolStep / Git 失败同等可见

- **WHEN** static-check / test-execution spawn 步、vcs 步（clean / commit / rebase / merge / finalize）或写面相位机步以非零退出 / 显式 Err 收场
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: Cleanup 尽力步警告可见不呈失败

- **WHEN** archive 相位 Cleanup 步的 `git worktree remove` 非零退出
- **THEN** 该节点 `passed` 收口、`detail` 携警告（worktree 路径 + 手动清理指引）在图上可读；run 续走 Seal / Finalize / PhaseLog 正常收口，链不因清理失败停摆

#### Scenario: 节点与边两源派生可重算

- **WHEN** 审查编排运行时的持久化面与读路
- **THEN** runs / run_nodes / run_edges 三表在案、在飞 run 节点与边驻进程内注册表（RunEntry）；已收口 run 的任一节点状态与边均可由库读史重算得出，不依赖进程内存

#### Scenario: 归档步词汇上图与边四值可辨

- **WHEN** 一次 run 含 initialize 相位 5 步与 archive 相位 11 步（含 git 步与空 checklist 落账）
- **THEN** initialize 步词节点（phase_start / clean / static_check / test_execution / phase_log）与归档 9 个步词节点全落 `RunNodeRecord`（四类节点类型可辨），串行 forward 边落边表；同 run 若另含 feedback / retry / backtrack，四 kind 边齐备且显式落库

### Requirement: 零 CLI 子进程写通道与 crate 依赖方向

Walker MUST NOT 经任何 CLI 子进程变更 change 流程状态（dev-team CLI 子命令面不存在）：所有 workflow 状态变更 SHALL 经 core/workflow 写面进程内直调（desktop-workflow-write-face 契约，落库 workspace 库），MUST NOT 读写 workflow.json。依赖方向 SHALL 为 orchestration → workflow；`crates/infra/devteam` MUST NOT 存在（整体删除）。进程 spawn 例外家族为检查域门禁（static-check / test-execution，落 `crates/infra/checks`）与 vcs 域 git 步（initialize 基线恢复 `restore_baseline` + 归档 commit / rebase / merge / cleanup / finalize，落 `crates/infra/vcs`）——两者均经 port 缝下沉、spawn 不进 core（core/orchestration 自身零进程 spawn；检查域见 desktop-checks-domain，vcs 域见 desktop-crate-layout）。

#### Scenario: 零 CLI 写触点

- **WHEN** 审查编排运行时与命令组源码中对 change 流程状态的变更触点
- **THEN** 零 CLI 子进程调用、零 devteam 子进程依赖、零 workflow.json 读写；全部状态变更经进程内写面调用返回确认并落库

#### Scenario: spawn 边界保持（checks 与 vcs 双例外）

- **WHEN** 审查 core/orchestration 源码的进程创建调用
- **THEN** orchestration 模块内零进程 spawn；检查域 spawn（static-check / test-execution）经 port 缝由 `crates/infra/checks` 承载，git 步 spawn（含 initialize 相位 `restore_baseline` 的基线恢复与 cleanup 的 worktree 移除）经 `ArchiveVcsPort` 缝由 `crates/infra/vcs` 承载

### Requirement: 会话挂靠 provenance 与转录 session id 直查

Walker 发起的 executor / evaluator / decision / spec_sync / resolve_conflicts 会话 SHALL 携带 provenance：`source="change"`、`source_ref=<id>/<phase>/<role>/<attempt>`（身份段为 change id——单一身份锚贯穿，name 可变不入寻址键；归档步两会话的 source_ref 定式由 design 定稿，MUST 在同 phase 同 attempt 内唯一，避免 spec_sync 与 resolve_conflicts 撞号）。该 `source_ref` SHALL 由 walker 在会话创建时写入并随会话记录持久化（`SessionProvenance` 字段面保持），作为来源归属留痕。节点运行态与转录寻址 SHALL 经 run 节点行的 `session_id` 直查（run 节点行携该步会话 id，见 desktop-change-state-store「run 执行图落库」；前端不再按 provenance 反查会话集）。节点 `session_id` SHALL 在会话首个事件到达时提前挂到运行中的会话型步节点（running emit 不携 id、终态 emit 才回写；首挂补发一拍 `step` 通知触发统一视图重查），确保运行中节点按 id 直查实时转录；后续会话事件 MUST NOT 重复挂载 / 重复补发 `step` 通知。会话转录 SHALL 可观测：运行中实时流、结束后重放一致。会话停止 SHALL 复用 StopRegistry 既有终止面，对已终态目标幂等忽略。

#### Scenario: provenance 留痕与 session id 直查

- **WHEN** 某 change 的 implement#2 相位已发起 executor 与 evaluator 会话（或 archive 相位已发起 spec_sync 会话）
- **THEN** 两会话记录各自 `SessionProvenance.source="change"` 且 `source_ref="<id>/<phase>/<role>/<attempt>"` 逐字在案（归档步两会话 source_ref 同 phase 同 attempt 内唯一）；节点转录经其 run 节点行 `session_id` 直查（无需 `agentSessions` 按 source / sourceRef 反查）

#### Scenario: 运行中节点 session id 提前挂载

- **WHEN** executor 会话的 running 节点已在图上（running emit 未携 session id），该会话首个事件经 run 事件出口到达
- **THEN** 该节点快照的 `session_id` 即刻落值并补发一拍 `step` 通知（前端重查统一视图后按 id 直查实时转录）；后续会话事件仅广播 `sessionEvent`，不重复挂载、不重复补发 `step` 通知

#### Scenario: 转录可观测与停止

- **WHEN** executor 会话运行中用户请求停止
- **THEN** 该会话经 StopRegistry 收敛为 `stopped`，walker 收到终止终态并按停止收敛处理 run；对已终态会话的重复停止请求幂等忽略

### Requirement: 变更文件上下文降级 git diff

Desktop run 的 executor / evaluator prompt 组装 SHALL 以 git diff（工作区变更面）提供变更文件上下文（承接原 file_log「让 agent 识别变更文件」的既有用途）。file_log 概念 SHALL 随双向墙整体退役：desktop 全链 MUST NOT 读写任何 `file_log`（不提取转录写路径、不调 `change-files`、不展示 file_log 条目）；存量 skill 路径 workflow.json 内的 file_log 数据随其文件留档不动，desktop 不再解析与展示。

#### Scenario: desktop 全链零 file_log 触点

- **WHEN** 扫描 desktop 源码（core / infra / commands / views）对 file_log 的读与写触点
- **THEN** 零触点；既有 skill 路径产生的 file_log 数据不被触碰（workflow.json 原样留档）

### Requirement: static-check spawn 步门禁

implement / test-gen 相位的 executor 收口后，walker SHALL 必经 static-check 步（checks 边界 `crates/infra/checks` spawn 执行，承接原 SubagentStop hook 的门禁职责；该步由相位 step 定义声明，非 `STATIC_CHECK_PHASES` 常量注入）；initialize 相位 SHALL 声明同款 static-check 步承载基线绿门（语义分叉见下）。static-check 步 SHALL 经 id → 记录 → name 解析（`store.get_change(id).map(name)`，与 test-execution 分支同式）把 change name 传入 `StaticCheckRunner::run(root, name)`——name 供 infra 侧 output 落盘（recorded，`output_ref` 回填，`<report_dir>/exec-outputs/<exec_id>.log`）；未建档（无 `ChangeRecord`）显式 `Err`。implement / test-gen 相位的 static-check 失败 SHALL 走**定向反馈边**：将捕获的诊断反馈注入同一 executor 会话（Continue）修复，独立计数上限 5 次（沿原 hook `loop_limit` 语义）；每次修复重跑为 `instance_no` 递增的新节点、图上以 `feedback` 边表达。超限 SHALL 升格为相位 fail（walker 以桌面代写的 fail checklist 经写面 `phase_log` 落账，不跑 evaluator，进入决策分叉——重跑经决策 retry 决议兑现，窗口 fail 计数即预算面）。MUST NOT 将反馈边重试直接计入相位 retry 预算。initialize 相位的 static-check 步失败 SHALL 以 `Terminal` 收敛（`on_fail=Terminal`，无反馈边、零 agent 会话注入——基线的债不属于本次变更，见「initialize 基线检查相位执行体」）。非 implement / test-gen / initialize 相位 MUST NOT 触发 static-check（其 step 序列不含 static-check 步）。

#### Scenario: 修复反馈边

- **WHEN** executor 收口后 static-check 报 lint / fmt 错误
- **THEN** walker 将诊断文本注入同一 executor 会话（Continue）重试修复，反馈边计数 +1，新修复节点 `instance_no` 递增；修复后 static-check 通过则相位继续 evaluator 步

#### Scenario: 反馈边超限升格

- **WHEN** 同一相位内定向反馈边已达 5 次仍不通过
- **THEN** walker 以代写 fail checklist 经写面 `phase_log` 落账（不跑 evaluator），相位按 fail 收场进入决策分叉（重跑经决策 retry 决议兑现），walker 不再注入反馈边

#### Scenario: 门禁相位限定

- **WHEN** proposal / dev-design 等非门禁（implement / test-gen / initialize）相位 executor 收口
- **THEN** walker 不发起 static-check spawn（该相位 step 序列不含 static-check 步），直接进入 evaluator 步

#### Scenario: static-check 步 name 解析与 recorded

- **WHEN** walker 走到某已建档 change 的 static-check 步（假 store 供给 name）
- **THEN** `StaticCheckRunner::run(root, name)` 收到该 change 的裸名，`StaticCheckOutcome.output_ref` 回填（passed 与 failed 两分支均携 `output_ref`），节点可经 stdout 面板回查；change 未建档则显式 `Err`

#### Scenario: initialize 基线步无反馈边

- **WHEN** initialize 相位的 static-check 步失败
- **THEN** 步以 `Terminal` 收敛 run failed（零反馈边、零 executor 会话注入），文案引导在基线上修复后重新发起；不消耗 FeedbackLoop 预算

### Requirement: verdict 解析与 phase-log 代写

Evaluator 会话 SHALL 依 prompt 约定在最终消息输出 checklist JSON（不自调 MCP `phase_log`）。Walker SHALL 解析该 JSON 并代调写面 `phase_log`（进程内）落 eval 记录（PhaseRecord + ChecklistItemRecord 原子落库）；解析失败 SHALL 走反馈边（解析错误诊断续注同一 evaluator 会话修复重跑，独立计数 ≤5），MUST NOT 臆测 verdict、MUST NOT 静默跳过落账；超限 SHALL 升格相位 fail（桌面代写 fail phase-log）进决策分叉（重跑经决策 retry 决议兑现）。checklist JSON 结构 SHALL 瘦身为 `{ verdict, report, checklist }`（checklist 逐项 `{ item, pass, evidence }`）；phase / attempt / skipped SHALL 由 walker 盖戳落账（evaluator 不回声），`EvaluatorChecklist` 解析类型 MUST NOT 要求 phase / attempt / skipped 字段。确定性相位（test-execution 机械 checklist、archive 空 checklist）SHALL 走 `step_phase_log` 的独立分支，不经 evaluator verdict 解析。

#### Scenario: verdict 代写落账

- **WHEN** evaluator 最终消息输出合法 checklist JSON（verdict=pass，无 phase / attempt / skipped 字段）
- **THEN** walker 以该 checklist 盖 phase / attempt / skipped 后经写面 `phase_log` 落 eval 记录，`phase_next` 随后推进

#### Scenario: 解析失败反馈边修复

- **WHEN** evaluator 最终消息不含可解析的 checklist JSON（结构漂移或空输出）
- **THEN** 门步节点呈失败态，解析失败原因注入同一 evaluator 会话（Continue，`feedback` 边）修复后重解析（`instance_no` 递增新节点）；当轮修复期间状态库无新增 eval 记录

#### Scenario: 解析失败超限升格

- **WHEN** 同 attempt 内解析失败经 5 次反馈修复仍未收敛
- **THEN** walker 代写 fail phase-log（report 携升格记因、fail checklist 携解析错误、executor + evaluator 双槽位）升格相位 fail，回写面 `phase_next` 进决策分叉（重跑经决策 retry 决议兑现）

### Requirement: 决策协议

写面 `phase_next` 返回决策分叉触发（任意带 evaluator 相位的 verdict fail——最新 fail 条目未挂决策会话；或窗口 fail 数达重试上限的标注变体）时，walker SHALL 唤起决策 agent（compose_turn 新会话）；walker 自走同相位重跑 SHALL 退役（盲目重跑的会话消耗高于一次决策——重试修复一律经决策 retry 决议兑现）。决策 agent 输入 SHALL 有界且来自 db 状态与写面响应：fail checklist（fail 项 + evidence）、`allowed_backtrack_phases`（写面 `phase_next` 白名单）、候选相位最近一次 eval report、相位 retry 语义标记（决策 prompt 指引段按形态分派：评估-only 相位 retry 仅重评不修复、修复须回退；带 executor 相位 retry 携失败清单重跑修复、根因上游仍应回退）。决策输出 SHALL 为封闭集：

- `{ action: "backtrack", backtrack_to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；
- `{ action: "retry" }` — 同相位重试；
- `{ action: "stop", reason }` — 终止 run；
- `{ action: "ask", question, options[] }` — 无法裁决，UI 中断提问。

决策 retry 决议的兑现 SHALL 依赖「已决条目放行恰一次」语义：决策会话挂账（decision_session_id）后的同一条目 MUST NOT 重复触发决策分叉，写面 `phase_next` 对其正常路由回同相位放行恰一次重跑（新落账 fail 条目无决策标记，触发点对新条目照常生效）——否则 retry 决议会陷入 phase-next ↔ 决策空转，相位永不重执行。

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验（工具侧兜底）：越权目标 SHALL 被拒绝且 run 停给用户，MUST NOT 落任何越权回跳账（状态库零变更）。决策输出解析失败显式失败停给用户（决策会话不设反馈边，区别于 verdict 解析失败）。

#### Scenario: 白名单内自治回跳

- **WHEN** test-execution#1 fail 达重试上限且决策 agent 输出 `{action:"backtrack", backtrack_to:"test-gen", reason:...}`（test-gen 在白名单内）
- **THEN** walker 经写面 `backtrack` 落回跳记录（落库），`phase_next` 随后路由至 test-gen 重做，reason 随边标签可读

#### Scenario: 越权决议被兜底拦截

- **WHEN** 决策 agent 输出的 backtrack_to 不在 allowed_backtrack_phases 内
- **THEN** 写面 `backtrack` 二次校验拒绝（状态库零变更），run 停给用户并呈现越权详情

#### Scenario: ask 中断提问

- **WHEN** 决策 agent 输出 `{action:"ask", question:..., options:[...]}`
- **THEN** run 进入中断等待态，UI 呈现问题与选项；用户应答后 run 按应答继续（应答经运行控制命令面回流 walker）

#### Scenario: verdict fail 分叉与 retry 兑现

- **WHEN** code-review#1 落 verdict=fail（评估-only 相位，prompt 携「retry 仅重评」指引段）且决策 agent 输出 `{action:"retry"}`
- **THEN** walker 唤起决策 agent 恰一次，retry 决议后同相位放行恰一次重跑（evaluator 重评）；重跑再落新 fail 条目时决策分叉对新条目照常触发。带 executor 相位（如 implement）verdict fail 同式分叉（prompt 携「retry 携失败清单重跑修复」指引段），retry 决议后 executor 携失败反馈段重跑

### Requirement: run 双 root 组合与 exec root 解析

change run SHALL 以双 root 组合执行（能力语义见 desktop-change-worktree）：`change_flow_start` SHALL 解析 exec root——db 记录带 `worktree` → 该 worktree 绝对路径；记录 `worktree=None`（legacy）→ 主 workspace root——`RunRequest.root` SHALL 恒为 exec root，会话 cwd（`turn.root → SessionCtx.workspace_root`）、工件根（`Layout::resolve`）、git diff 与检查器（static-check / test-execution）执行目录 SHALL 全部随 exec root 落位。store / db 半边 SHALL 恒以 workspace root 解析：`compose_turn` 的 store 半边为命令层预解析注入的 workspace root 库实例，worktree 路径 MUST NOT 进入 `for_root`；`StoreSnapshot` 的 fs 半边（产物发现 / detail 装配）取 exec root、db 半边取注入的 store 实例（既有构造形态天然满足）。相位状态机零 fs、会话 resume 链（记录不存 cwd）MUST 保持零改动。SDK 路径沙箱前缀校验以 exec root 为锚（既有机制零改动），worktree change 的文件编辑囚于 worktree 内；worktree 内 `git diff HEAD` 恰为本 change 编辑集（「变更文件上下文降级 git diff」requirement 的 diff 面语义在 worktree 内自然收窄，无模板改动）。

#### Scenario: worktree run 全链落 worktree

- **WHEN** 以带 worktree 记录的 change 发起 run（假引擎驱动一相位）
- **THEN** executor / evaluator 会话 cwd = worktree、static-check 与 test-execution spawn 的执行目录 = worktree、git diff 上下文取自 worktree 且仅含本 change 编辑集

#### Scenario: store 身份恒 workspace root

- **WHEN** 审查 worktree change run 的组合根装配与 compose_turn 调用
- **THEN** store 实例由命令层以 workspace root `for_root` 解析后注入（provider / 会话 / 建档 / StepRecord 落 workspace 库），全程无以 worktree 路径进 `for_root` 的调用点

#### Scenario: legacy run 零变化

- **WHEN** 以 `worktree=None` 的存量建档 change 发起 run
- **THEN** exec root 解析为主 workspace root，既有 run 全链（cwd / Layout / diff / 检查器 / 快照）语义零变化

#### Scenario: 并行 change 各自 worktree

- **WHEN** 同 workspace 两个带各自 worktree 的 change 同时发起 run（假引擎）
- **THEN** 两 run 互不干扰（各自 exec root 的编辑 / diff / 检查隔离），db 写入经既有进程内并发支撑

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

#### Scenario: 控制面请求-应答不变

- **WHEN** 用户对 waitingAsk 态应答、对 phase 间停等确认、对运行中 run 停止
- **THEN** 三控制命令沿既有请求-应答契约执行（提前 resolve / 幂等 / 回流驱动语义不变），不经通知通道

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **引擎随默认 agent 解析（SDK 承载）**：change 执行会话经组合根缺省解析（默认 agent）发起——CLI 租户已随 retire-cli-engine 退役，change 执行由 SDK 租户承载。原「引擎恒 CLI（插件 dev-team 天然加载）」依据随桌面相位知识静态化（desktop-phase-prompts：14 个纯静态 prompt、无插件 Agent / MCP 工具假设）与插件废弃失效；原「SDK 引擎 MUST NOT 承载 change 执行（无插件加载机制）」禁令同步退役。change run 发起自此依赖 SDK 连接配置（默认 sdk agent + provider），缺失时按既有显式 `Err` 面失败并引导管理页，MUST NOT 静默回退。
2. **工作流类型**：requirement 与 test-only 工作流全链；bug-fix 相位表后续独立 change。
3. **计划图静态编译期、非通用图引擎**：计划图（workflow → phase → step 序列 + 依赖边 + artifact 清单）SHALL 为编译期 const 表 + `include_str!` 静态装配，MUST NOT 实现运行时加载的通用图引擎或图定义文件格式；执行图（节点 + 边）为 db 落库的运行记录，MUST NOT 上位为路由权威（路由仍问 `phase_next`）。原「walker 只硬编码 walk 本相位循环图」表述随本变更翻案显式立项（step 序列下沉为 `PhaseDefinition.steps`）。
4. **file_log 载体退役**：desktop run 与视图全链零 file_log 触点（git diff 供变更上下文）为既定裁定；外部归档对账（openspec CLI 归档改名后的 db status 翻转时机）留 design 定稿（见 proposal 待决问题）。
5. **protect-files 护栏缺口（原过渡依赖失效）**：步前实时护栏原过渡期依赖插件 hook（仅 CLI 会话内生效）；CLI 租户退役后该路径失去载体，缺口自本变更起**显式打开**——写保护面仅剩 SDK 路径沙箱（workspace root 前缀校验）。桌面原生护栏（内核 permission 层方向）的独立 change 为缺口出口（插件真下线前落地），本条更新为缺口显式留痕，不由实现隐式吸收。

（原「sessionAnchors 精度损失」「sweep-phase 缺口」两条边界随写面进程内锚点复活而消失，不再留痕；原「file_log 不记录」边界随本变更升级为载体退役，不再是单侧不写。）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含计划图静态编译期翻案留痕与护栏缺口显式留痕），后续变更无需重新论证是否知情

#### Scenario: 引擎边界生效

- **WHEN** 审查 walker 发起 WorkerAgent 会话的引擎解析路径
- **THEN** 走组合根缺省解析（默认 agent）构造引擎，SDK 租户承载、无 CLI 分支；默认 agent 缺失或 SDK 连接配置不齐时显式 `Err`（管理页引导），change run 发起依赖 SDK 连接配置

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.37` 升级为 `0.4.38`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44` 与三类交付产物）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成（归档时执行版本升级）
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.38；`plugins/dev-team` 版本与交付产物零改动

### Requirement: run 落库写缝与累积器驻服务端

run 落库写缝 SHALL 收在 walker 侧单点：`walk_run` 终态出口（`guard.finish` 处）一次完成终态更新 + 节点整包 + 边整包 + active_phase 处置（同事务，desktop-change-state-store「run 执行图落库」「启动标定与 active_phase 悬挂处置」）；`ChangeFlowControl` SHALL 保持进程内零 IO 不变量（不持 store 句柄，store 写经 walker 侧 port / 命令层装配注入）。累积器 SHALL 移驻服务端并升级为**节点 + 边双累积**：`RunEntry`（注册表条目）自持节点列表（全词汇、emit 序、`instance_no` 分配）与边列表（feedback / retry / backtrack 显式落边），walker 经 `RunGuard::emit` 间达零变化；重挂快照 SHALL 含 nodes + edges（重挂恢复不再恒空）。启动装配 SHALL 执行启动标定（残留 running → interrupted，操作面见 desktop-change-state-store）。停止 / 会话失败 / 决策解析失败 / 相位预算耗尽等一切 run 死亡路径 SHALL 汇聚同一 finish 落包出口（终态出口唯一既有不变量）。

#### Scenario: 写缝单点与 control 零 IO

- **WHEN** 审查 walker 终态路径与 control 源码
- **THEN** 落包仅出现在 walk_run 终态出口一处；control 无 store 句柄、无 IO 调用（进程内纯状态不变量保持）

#### Scenario: 全死亡路径同一出口

- **WHEN** run 分别以 completed / stopped（用户停止）/ failed（会话失败、决策解析失败）收口
- **THEN** 三路径均经同一 finish 落包出口：终态 + reason + 节点整包 + 边整包 + active_phase 处置单事务落库，库读史与内存面终态一致

#### Scenario: 重挂恢复节点与边完整

- **WHEN** run 运行中（已 emit 多节点与 feedback 边）视图重挂后读取快照
- **THEN** 快照 nodes 与 edges 与已 emit 逐条一致（emit 序、`instance_no` 稳定），后续通知继续触发重读；不依赖前端累积

### Requirement: 相位 prompt 组装面静态化（静态主体 + 动态 append）

executor / evaluator 会话 prompt SHALL 以写面下发的静态角色知识文本（desktop-phase-prompts / desktop-workflow-write-face）为主体；动态面 SHALL 全部 append 而非模板替换：上下文头一行 `change: <name>`、回溯原因行、static-check / 测试修复失败注入、verdict 解析失败修复注入（evaluator 续注）、输出协议附录（evaluator）、decision 三列表（决策会话）。prompt 组装 MUST NOT 使用 `<change>` / `<phase>` 模板替换。`WorkerTurnRequest.provenance.source_ref = <change>/<phase>/<role>/<attempt>` 与 `root` / `model_level` 结构化通道不变——phase / attempt 盖戳不依赖 prompt 文本。

#### Scenario: 上下文头在场

- **WHEN** walker 发起 executor / evaluator 会话
- **THEN** prompt 含一行 `change: <name>`（change 名自写面记录 name 供给）
- **AND** prompt 主体为静态角色知识文本，无 `<change>` / `<phase>` 模板替换痕迹

#### Scenario: 反馈注入仍 append

- **WHEN** static-check / test-execution 失败或 verdict 解析失败走定向反馈边
- **THEN** 修复反馈文本以 append 注入同一 agent 会话 prompt（不重写静态主体）——门禁步注入 executor 会话、verdict 门注入 evaluator 会话
- **AND** 回溯场景的回溯原因行照常 append

### Requirement: 角色要点静态化（role_brief 退役）

`core/orchestration/src/prompt.rs` SHALL 删除 `role_brief` 15 行静态表与 `strip_call_agent`（角色自述已溶解进 desktop-phase-prompts 各 md）。`executor_prompt` SHALL 不再前置「你以角色…」+ role_brief 段，改为直出静态 prompt 主体（不再消费 `agent_type`）；`evaluator_prompt` SHALL 仅追加输出协议附录。`prompt.rs` MUST NOT 残留按角色名分发要点的逻辑。

#### Scenario: role_brief 退役

- **WHEN** 审查 `prompt.rs` 与 executor prompt 组装
- **THEN** 无 `role_brief` 表、无 `strip_call_agent`、无 `__CALL_AGENT` 解析
- **AND** executor prompt 主体直出静态 md 文本（角色自述已含于其中）

#### Scenario: evaluator 协议仍唯一追加

- **WHEN** 组装 evaluator prompt
- **THEN** 仅追加输出协议附录，无角色要点前导、无 `__CALL_AGENT` 相关处理

### Requirement: evaluator 输出协议瘦身

`evaluator_protocol` 输出协议附录 SHALL 要求 evaluator 最终消息输出且仅输出一个 checklist JSON 对象，形状为 `{ verdict: "pass" | "fail", report: <≤2000 字符>, checklist: [{ item, pass, evidence }] }`；MUST NOT 要求 phase / attempt / skipped 回声（由 walker 按 provenance 盖戳）。协议 SHALL 保留禁调 MCP 写通道红线（phase_log / phase_next / phase_start / backtrack，落账由桌面编排代写）；checklist 至少一项、pass 为布尔、evidence 必须给事实依据。

#### Scenario: 协议形状瘦身

- **WHEN** 读取 evaluator 输出协议附录文本
- **THEN** 要求形状仅含 verdict / report / checklist 三键
- **AND** 无「输出 phase 字段」「输出 attempt 字段」「输出 skipped 字段」要求
- **AND** 仍含禁调 MCP 写通道红线

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

### Requirement: 确定性相位空 checklist 落账

archive 相位（无 evaluator、无 checklist 的确定性相位）SHALL 在末尾 PhaseLog 步以空 checklist 落账：`step_phase_log` 在 test_outcome 分支之后、verdict 分支之前插入 archive 分支——构造 `PhaseLogInput { phase: "archive", report: String::new(), checklist: vec![], skipped: false, executor/evaluator/decision 三槽位 None }` 并直接 `run_phase_log`。落账 SHALL 复用既有 `derive_verdict([])=Pass` 口径（空 checklist = 无条件通过），零 `if phase == archive` 特判、零新增落账函数。三分流按「产物在否」分离：archive 相位无 TestExecution step（test_outcome 恒 None）、无 Evaluator/VerdictGate step（verdict 恒 None），仅命中空 checklist 分支。

#### Scenario: 空 checklist 落账 pass

- **WHEN** archive 相位 10 步走完进入 PhaseLog 步
- **THEN** `run_phase_log` 以空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord，`active_phase` 清位（phase_log 单事务），`phase_next` 随后路由 done

#### Scenario: 三分流互不误判

- **WHEN** 分别驱动评估相位（verdict 产物）、test-execution 相位（test_outcome 产物）与 archive 相位（空 checklist）
- **THEN** `step_phase_log` 三分支按产物在否分离，无按 phase 名判断、无跨分支误判

### Requirement: 发起前置校验类型分派（严格拒绝未知类型）

`change_flow_start` 前置校验 SHALL 保留「`phase_table(record.workflow_type)` 返回 `None` → 显式拒绝」的门：`"requirement"` / `"test-only"` 出线放行（test-only 进表后自然放行），其余（含空白 / bug-fix）SHALL 显式 `Err`。错误文案 SHALL 通吃 `{requirement, test-only}`，MUST NOT 再写死「V1 仅 requirement」、MUST NOT 引入「未知类型静默跑 requirement」的 fallback。walker 全程 SHALL 不新增 workflow_type 分支——相位序 / 步序列仍由写面 `phase_next` 依 `record.workflow_type` 下发的相位表承载。

#### Scenario: test-only 发起放行

- **WHEN** 对已建档 `workflow_type="test-only"` 的 change 调用 `change_flow_start` 前置校验
- **THEN** 校验通过（test-only 相位表在位），run 按 `TEST_ONLY_TABLE` 相位序走（proposal → code-analyze → … → archive）

#### Scenario: 未知类型显式拒绝

- **WHEN** 对 `workflow_type="bug-fix"`（或空白）的 change 调用 `change_flow_start`
- **THEN** 显式 `Err`（文案含 `{requirement, test-only}` 支持面），不进入运行态、不落任何账

### Requirement: MutationTest 步接线与独立反馈预算

walker SHALL 在 test-execution 相位按 `TEST_EXECUTION_STEPS` 步序列（`PhaseStart → TestExecution → MutationTest → PhaseLog`，定义见 desktop-workflow-write-face）顺序执行：TestExecution 结论 pass 后才执行 MutationTest 门禁步（`ToolCommand::MutationTest` 经 `MutationRunner` port 由 checks 边界 infra 实现），TestExecution fail / error 走既有反馈边修复重入——「测试 / 覆盖率修绿后再执行突变」由步序列顺序执行天然保证，walker MUST NOT 在 TestExecution 未 pass 时发起突变 spawn。MutationTest 失败（中断 / 未达标）SHALL 走**独立定向反馈边**（反馈边②，报告指引式口径，见 desktop-mutation-testing）：注入本相位 executor 会话修复（补测试杀突变）——反馈边注入对象分角色清单随本条扩为 static-check / test-execution / mutation-test 失败注入 executor、verdict 解析失败注入 evaluator。反馈边② SHALL 经 `feedback_limit(step, "mutation-test")` 独立计数、上限 5（与 static-check / test-execution 反馈计数器分立，各自计满各自升格），MUST NOT 计入相位 retry 预算；超限 walker SHALL 代写 fail checklist 经写面 `phase_log` 升格相位 fail（不跑 evaluator，进入决策分叉——重跑经决策 retry 决议兑现）。`step_phase_log` 的 test 分支 SHALL 消费 `test_outcome` + `mutation_outcome` 双槽（机械 checklist 双槽，见 desktop-test-execution）；archive 分支双槽恒 None（三分流按「产物在否」分离语义不变）。

#### Scenario: 拆步时序绿后才突变

- **WHEN** TestExecution 首次 fail、经反馈边修复重入 pass 后 walker 推进
- **THEN** MutationTest 仅在 TestExecution pass 后执行；修复期间零突变进程 spawn，步序列零乱序

#### Scenario: 反馈边②独立计数与超限升格

- **WHEN** MutationTest 未达标连续反馈修复 5 次仍不达标
- **THEN** 反馈边②计数独立（与 static-check / test-execution 分立）且不消耗相位 retry 预算；第 5 次后 walker 代写 fail checklist 升格相位 fail，不再注入反馈边

#### Scenario: 双槽分支分离不误判

- **WHEN** 分别驱动 test-execution 相位（test_outcome + mutation_outcome 双槽产物）与 archive 相位（双槽恒 None）
- **THEN** `step_phase_log` test 分支消费双槽机械 checklist，archive 分支仅命中空 checklist 分支，互不误判

### Requirement: 步级时间戳与 token 消耗随行

步状态行 `ChangeStepState` SHALL 增四字段并全链随行：`started_at: i64`（UTC unix 毫秒，域内 `now_millis()` 口径）、`ended_at: Option<i64>`（running 恒 None）、`token_input: Option<u64>` / `token_output: Option<u64>`（本步 turn 增量，仅 WorkerAgent 步终态有值，缺席合法缺省）。`StepSettle` SHALL 同步扩四字段（emit 载荷收窄面）。

**时间铸造 SHALL 收 `emit_node` 单点**（复用既有 `now_millis()`，MUST NOT 新增时钟注入缝——收口时刻铸造只在生产路径消费，corpus 确定性由测试直构 `ChangeStepState` 注入定值承载，沿 `walker.rs` `now_millis` 既有注释口径）：

- running emit：`started_at = now`、`ended_at = None`、token 两值 None；
- 终态 emit（passed / failed / stopped）：`ended_at = now`，随行 `started_at = now`（该值是否被采信由合并侧裁定）——`phaseStart` 等单 emit 瞬步由此自然得 `started_at == ended_at` 同刻。

**合并语义（`accumulate_node`）SHALL 为**：`started_at` 无条件保首见（running emit 落值后不被终态 emit 顶替，终态行携带的同刻值被丢弃；瞬步单 emit 首见即真值）；`ended_at` / `token_input` / `token_output` 与 `session_id` 同款「some 才覆写」。节点累积器（`RunEntry.nodes`）、活面快照（`ChangeRunSnapshot` / `ActiveRunView`）与收口整包（`finish_command` → `RunNodeEntry`）SHALL 逐字段同值透传，零逻辑分支。

**token 求和 SHALL 在 `run_worker` 终态 emit 前单次落值**：对 `outcome.transcript` 的 `AgentEventKind::TurnDone.usage` 求 in / out 两键之和（`worker.run(turn).await` 收口后 transcript 已含 TurnDone——终态单次落值即会话结束后累计结果，MUST NOT 建二次刷新通道）；usage 缺席（引擎未返回）= None；工具步 / Gate 步 / Git 步恒 None。token 求和函数 SHALL 唯一实现于 core/agent（`AgentEventKind::TurnDone` 定义所在 crate，orchestration 已依赖；agent crate 禁 workspace 内依赖而函数仅吃 `&[AgentEvent]` 天然合规），infra/store 的会话统计 SHALL 改引同一实现（依赖方向 infra → core 不可反向，MUST NOT 保留双实现）。`RunUpdate::Step` 载荷随 `ChangeStepState` 自动扩展，IPC 通知 kind-only 降位形态（载荷不出进程、客户端收通知重查统一视图）MUST NOT 改变。

#### Scenario: 双 emit 起止落值与首见保留

- **WHEN** 某 WorkerAgent 步经 running emit（时刻 T0）后于时刻 T1 以 passed 收口（终态 emit）
- **THEN** 合并后节点 `started_at == T0`（终态行携带的 T1 起始值被丢弃）、`ended_at == T1`；活面快照与收口整包同值

#### Scenario: 瞬步同刻

- **WHEN** `phaseStart` 等单 emit 直 passed 的瞬步收口（时刻 T）
- **THEN** 节点 `started_at == ended_at == T`（耗时 0ms），无 running 先行 emit 要求、无特判分支

#### Scenario: token 本步增量落值

- **WHEN** `run_worker` 收口的 `outcome.transcript` 含两条 TurnDone（usage 各 `input_tokens` / `output_tokens`）
- **THEN** 终态节点 `token_input` / `token_output` = 本步 transcript 内 TurnDone usage 各键求和；usage 全缺席的会话两值为 None；同 run 的 static-check 工具步节点两值恒 None

#### Scenario: 停止与崩溃面

- **WHEN** 用户停止 run 使在飞 WorkerAgent 步以 stopped 收口（或进程被杀、终态 emit 未达）
- **THEN** stopped 路径同有 `ended_at`；被杀路径节点留 running 形态（`ended_at` 恒 None），零补值逻辑

#### Scenario: 求和函数单源

- **WHEN** 审查 orchestration 与 infra/store 对 TurnDone usage 求和的实现触点
- **THEN** 唯一实现居 core/agent，两消费方同源引用；store 会话统计输出与上移前逐字段一致；orchestration 依赖面零 infra 新增

#### Scenario: 通知面零影响

- **WHEN** 运行中步状态变化触发 Channel 通知
- **THEN** IPC 通知仍为 kind-only（载荷不出进程），客户端经统一视图重查取到含四字段的节点行；通知契约零变化

### Requirement: initialize 基线检查相位执行体

Walker SHALL 以两表表头一次性 `initialize` 相位（`INITIALIZE_STEPS`，见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）承接变更流程启动前的基线绿门：步序 `PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`，全部检查与恢复 SHALL 在 exec root（worktree）内执行（主仓未提交改动不进 worktree，基线语义纯净）。initialize 相位 SHALL 为确定性相位（零 executor / evaluator / decision 会话，全程零 WorkerAgent）：

- **Clean 步**（vcs 直调先例同归档 git 步，不经 ToolCommand 枚举）：walker 经 `ArchiveVcsPort::restore_baseline(worktree, 排除面)` 恢复 worktree 至基线态——tracked 修改 reset、untracked 清除，排除面 = foundation 共享的依赖与工具产物树名单（node_modules / dist / build / target / coverage / .nyc_output + dot 目录，见 desktop-checks-domain「依赖与工具产物树名单共享单源」）+ `openspec/` 工件树（proposal 等过程文档与报告树为未跟踪面，MUST NOT 被 clean 删除）。Clean 步 MUST 仅在 worktree 在场时执行；失败即 `Terminal`（`on_fail=None`）。
- **StaticCheck / TestExecution 步**：复用既有检查域 runner（`StaticCheckRunner` / `TestExecutionRunner`，报告槽位为 baseline 变体，见 desktop-test-execution「baseline 报告槽位与树隔离」），MUST NOT 执行 mutation（基线检查明确不含突变）。两步声明 `on_fail=Terminal`：失败 SHALL 以 `Terminal` 收敛 run failed，MUST NOT 走反馈边（无 agent 会话可注入，重试基线无意义）——walker 工具步执行体 SHALL 为 `OnFail::Terminal` 提供专用分支（现状 `feedback_limit` 把工具步 Terminal 声明视为计划图漂移拒绝；verdict-gate 已有 Terminal 分支先例）。失败文案 SHALL 区分：test-execution `conclusion=Error`（环境 / 配置问题——引导查配置与环境后重新发起）与 `conclusion=Fail`（测试红——引导在基线上修复测试后重新发起）；static-check 失败引导在基线上修复后重新发起；文案 SHALL 携 baseline 报告目录路径供回查。
- **PhaseLog 步**：机械 checklist 纯桌面代写（零 agent，沿 `test_execution_phase_log_input` 先例——steps 含 TestExecution 步则 `test_outcome` 自动挂载、`step_phase_log` 三分支天然分派），report 指向 baseline 报告目录，三会话槽位恒 `None`。

一次性语义 SHALL 由既有落账机制天然成立：任一步 `Terminal` 失败时 PhaseRecord 零 initialize 条目（图节点照常落 `RunNodeRecord` 留痕可回查）→ 再发起时 `phase_next` 仍路由 initialize、Clean 步清残留后干净重试；全绿 pass 落 PhaseRecord（机械 checklist，verdict=pass）→ 持久跳过（后续 run 不再重跑基线检查）。initialize 收口后的 phase 间停等 SHALL 与全相位一致（`auto_next_phase=false` 时照常 `ConfirmWait` 停等，零特判豁免）。legacy 记录（`ChangeRecord.worktree=None`）SHALL 整相位短路：步序列零执行、零 spawn、主仓 MUST NOT 被 reset 或 clean，以 skipped 落账收口（跳过因注明 legacy 主仓执行无 worktree 基线，一次性持久同 pass 语义）。

#### Scenario: 全绿序曲与一次性跳过

- **WHEN** 新建 requirement change 发起 run（基线 static-check 与 test-execution 全绿，auto_next_phase=true）
- **THEN** initialize 相位先于 proposal 执行：Clean / StaticCheck / TestExecution / PhaseLog 四节点按序上图（零 WorkerAgent 会话），报告落 `reports/baseline`；PhaseLog 机械 checklist 落账 pass 后后续相位照常推进；同 change 再次发起 run 时 initialize 被持久跳过（零重跑）

#### Scenario: 基线 static-check 红 Terminal 停止

- **WHEN** 基线 static-check 失败（真件 shim 配置为红）
- **THEN** run 以 `Terminal` 收敛 failed，文案引导「在基线上修复后重新发起」并携 baseline 报告目录；PhaseRecord 零 initialize 条目、零反馈边注入；再发起时 Clean 步恢复基线后重试

#### Scenario: test-execution Error 与 Fail 分文案

- **WHEN** 基线 test-execution 分别以 `conclusion=Error`（框架探测失败 / spawn 失败等环境问题）与 `conclusion=Fail`（测试红）收场
- **THEN** 两者均以 `Terminal` 停止 run，但终态 reason 文案可辨：Error 引导查配置 / 环境，Fail 引导修基线测试；两态不共用同一段文案

#### Scenario: clean 恢复基线且排除面保留

- **WHEN** 上一次 run 失败后 worktree 残留未提交代码改动与 untracked 文件，再发起进入 initialize 的 Clean 步
- **THEN** tracked 修改被 reset、untracked 残留被清除，worktree 恢复基线态；node_modules / dist / build / target / coverage / .nyc_output 与 dot 目录、`openspec/` 工件树原样保留（依赖与过程文档不丢，后续基线测试可跑、已落产物可读）

#### Scenario: legacy 整相位短路

- **WHEN** 以 `worktree=None` 的存量记录发起 run
- **THEN** initialize 步序列零执行（零 vcs / 检查 spawn），主仓零 reset 零 clean，相位以 skipped 落账（跳过因可见），后续相位照常路由

#### Scenario: 收口停等与全相位一致

- **WHEN** 手动档（auto_next_phase=false）run 的 initialize 相位 PhaseLog 落账 pass
- **THEN** run 照常进入 phase 间停等（`ConfirmWait`），用户确认后续走 proposal；无「发起即确认」特判豁免

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration`（运行时域，tokio） | 薄图 walker：相位循环、三类节点执行、Gate 分支 | 红线：零相位转移规则（每步过渡问写面 phase-next）；ToolStep 节点化（不内联）；节点状态纯派生（库读史 run 节点行 ∪ 在飞 run，节点行携 `session_id`）；模块内零进程 spawn；依赖方向 orchestration → workflow；对落库载体无感知 |
| `crates/core/orchestration/src/port.rs` | 「相位机 + 工具步」进程内缝（语义换血） | ToolStepPort 进程内直调 core/workflow 写面；ToolCommand 封闭集收缩（ChangeFiles 出局）；WorkerAgentPort 契约不变；测试假引擎 + 假写面双缝直驱 |
| `crates/core/orchestration/src/steps.rs`（LocalToolSteps） | 相位机步进程内直调写面 | 直调面签名不变（载体在写面内换血）；每步结果落 StepRecord 审计行（经写面） |
| `crates/core/orchestration/src/snapshot.rs` | ChangeDetail 只读装配 | 读源改 workspace 库 change 状态（经 queries）；磁盘扫描保留产物发现 |
| `crates/core/orchestration/src/verdict.rs` + `decision.rs`（不改） | 结构化输出解析 | 封闭 schema（checklist 信封 + 决策四动作封闭集）；解析失败显式 Err 上抛——处置归 walker 按步声明分叉（verdict 反馈边 / 决策终态） |
| `crates/core/orchestration/src/prompt.rs`（不改） | prompt 组装与运行时插值 | 模板取自 core/workflow 相位表单源 |
| `crates/core/orchestration/src/transcript.rs`（砍半） | 会话转录消费 | 仅保留 `final_assistant_text`（verdict 提取）；`extract_write_paths` 出局 |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现 + static-check spawn 缝 | compose_turn 新会话 + StopRegistry 终止 + 密封转录；provenance `source="change"`；spawn 不进 core |
| `src/commands/` change_flow 命令组 | 运行控制 IPC 面 | 发起前置校验改 db 建档校验（workflow.json 可解析校验退役）；发起 / 停止 / 应答 / 确认契约不变 |
| `crates/core/workflow`（写面 + 读面，见 desktop-workflow-write-face） | change 状态域权威 | 读写同屋檐、落库 workspace 库；walker 经进程内直调消费，不复制路由语义 |
| `crates/infra/store`（复用） | 会话记录 + change 状态落库 | `SessionRecord` 携 `SessionProvenance`（`source` / `source_ref` 留痕，会话创建时写入）；run 节点行携 `session_id` 供会话直查；不建 flow_runs 表 |
| `crates/infra/devteam` | **（删除）** | 整体删除（discover / runner），零遗留 |
| 既有 hooks（复用，不改） | 步前护栏与无外围账 | protect-files 原样生效（过渡期）；record-files / sweep-phase 未绑定 no-op（fail-open 刻意设计）；static-check 桌面模式由 spawn 步承接 |
| `crates/core/orchestration/src/walker.rs` | 停等节奏分支 | `RunRequest` + `auto_next_phase` 字段（run 级、发起定格）；drive() 确认点 `if !auto_next_phase` 才 emit `ConfirmWait` + `wait_confirm`；auto 模式不进 `WaitingConfirm`、零新事件；头注释口径同步 |
| `src/commands/change_flow/mod.rs` | 发起参数透传 | `change_flow_start` / `change_flow_start_with` 签名 + `auto_next_phase: bool`，透传 `RunRequest`；`Result<T, String>` 模板、前置校验、提前 resolve 不变 |
| `src/types/generated/bindings.ts`（重导） | IPC 类型跟随 | `changeFlowStart` + `autoNextPhase` 参数；`bindings:check` 守卫拦截过期生成物 |
| `crates/core/orchestration/src/control.rs` / `state.rs`（不改） | 零触点红线 | confirm 应答通道 / `RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 不动 |
| `crates/core/orchestration/src/port.rs` | 「相位机 + 工具步」进程内缝（封闭集扩展） | `ToolCommand` 增 `TestExecution { change, … }` 变体；`ToolStepOutput` 增 `TestExecution(最小载荷)`；`TestExecutionRunner` port 与 `StaticCheckRunner` 并列（spawn 不进 core）；实现落 checks 边界 `crates/infra/checks`，core/orchestration 零依赖 checks |
| `crates/core/orchestration/src/walker.rs` | 检查域门禁步接入 | static-check 反馈边沿用（`STATIC_CHECK_FEEDBACK_LIMIT` 不变）；test-execution 相位 runner 门禁步 + 独立反馈预算（计数器分立、上限 5、超限升格相位 fail） |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现（收窄） | compose_turn 新会话 + StopRegistry 终止 + 密封转录 + provenance `source="change"`；static-check spawn 缝移出（落 `crates/infra/checks`，见 desktop-checks-domain） |
| `crates/core/orchestration/src/walker.rs` + `RunRequest` | 双 root run 载体 | `RunRequest.root` 恒 exec root（worktree / legacy 主 root）；fs 半边（Layout / diff / 检查 cwd / 快照产物发现）随 root 落位；store 经 port 注入不变 |
| `crates/core/orchestration/src/control.rs` | run 控制注册表键换锚 | `runs: HashMap<(String, String), RunEntry>` 键 `(workspace root, change)` → `(workspace root, change id)`（12 处构造点随动）；`RunUpdate` / `RunNotice` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 零改动（通知零载荷） |
| `crates/core/orchestration/src/walker.rs` | run 载体与 provenance | `RunRequest` / `RunStartCommand` / `RunFinishCommand` / `ToolCommand` 载荷 change → change id；provenance 定式 `<id>/<phase>/<role>/<attempt>`；`SessionAnchors` 键 `(change id, run_id)` |
| `crates/core/orchestration/src/port.rs` + `steps.rs` + `run_history.rs` + `snapshot.rs` | 载荷 id 化 | 相位机步直调写面携 change id；`TestExecutionRunner` / `WorkflowSnapshotPort` 签名随动（`detail(root, id)` → 经 queries id 寻址） |
| `crates/core/workflow/src/write/phase_next.rs`（SessionAnchors） | 进程内锚点键 | `(change, run_id)` → `(change id, run_id)`；锚点语义（续走不重头）不变 |
| `src/commands/change_flow/mod.rs` | 五命令 id 化 | start / stop / answer / confirm / watch 定位参数 == change id；`ChangeFlowSink` 键 `(root, id)`；前置校验（建档 / 互斥 / exec root 解析）按 id 读取记录；store 实例以 workspace root 解析后注入 compose（拆参形态见 desktop-agent-execution / design）；其余三件事纪律不变 |
| `crates/core/orchestration/src/walker.rs`（step_cleanup，新） | Cleanup 计划步执行 | 记录解析（id → 记录 → worktree 字段）；缺席 / legacy 幂等跳过；remove 失败吞为 detail 警告（步恒 `passed`、链必收口）；经 `ArchiveVcsPort` 缝调用 |
| `crates/core/orchestration/src/state.rs`（ChangeStepKind） | 步词面 | `Cleanup`（`as_str` = `cleanup`）入封闭集；与 `RunStepKind` 同词面 |
| `crates/core/orchestration/src/run_history.rs` | 词面映射 | `ChangeStepKind::Cleanup → RunStepKind::Cleanup` |
| `crates/core/orchestration/src/port.rs`（ArchiveVcsPort 清理面，扩） | vcs 缝扩展 | worktree 在场探测 + 移除（`git worktree remove [--force]`）；实现驻 `crates/infra/vcs`；`Err` 面带手动清理引导 |
| `crates/core/orchestration/src/state.rs`（`ChangeStepState`） | 活面契约单点 | 增 `started_at: i64` / `ended_at: Option<i64>` / `token_input: Option<u64>` / `token_output: Option<u64>`；doc 注释携口径与合并语义指针 |
| `crates/core/orchestration/src/walker.rs`（`StepSettle` + `emit_node` + `run_worker`） | 铸造与 token 落值 | `StepSettle` 扩四字段；`emit_node` 按 status 铸造单点（零新时钟缝）；`run_worker` 终态 emit 前 transcript TurnDone.usage 求和（in / out） |
| `crates/core/orchestration/src/control.rs`（`accumulate_node`） | 合并语义 | `started_at` 无条件保首见；`ended_at` / `token_input` / `token_output` some 才覆写（与 `session_id` 同款） |
| `crates/core/orchestration/src/run_history.rs`（`finish_command`） | 收口整包透传 | 四字段直复制进 `RunNodeEntry`（零过滤零改写） |
| `crates/core/agent/src/`（`sum_usage_tokens` 上移落位，design 定稿） | token 求和单源 | `pub fn`（`(&[AgentEvent], &str) -> Option<u64>` 同签名）；orchestration / infra/store 两消费方同源引用 |
| `crates/infra/store/src/store.rs`（会话统计） | 消费方改引 | 删除私有 `sum_usage_tokens`，改引 core/agent 实现；聚合行为零变化 |
| `crates/core/orchestration/src/walker.rs` | initialize 相位执行体 | `step_clean`（vcs 直调 `restore_baseline`，仅 worktree 在场执行；legacy 整相位短路先例同归档步）；static-check / test-execution 工具步 `OnFail::Terminal` 专用分支（`feedback_limit` 不再拒绝工具步 Terminal）；Error / Fail 分文案 Terminal reason；PhaseLog 机械 checklist 沿 `test_execution_phase_log_input`（report 指向 baseline 目录） |
| `crates/core/orchestration/src/port.rs` | vcs 缝扩展 | `ArchiveVcsPort::restore_baseline(worktree, 排除面)`（sync 签名；返回恢复摘要或 Err 带 git 语境）；port 归属消费者先例不变 |
| `crates/core/orchestration/src/steps.rs` | ToolStep 面随动 | `LocalToolSteps` 组合不变；检查 runner 调用透传 baseline 报告槽位（形态 design 定稿） |
| `crates/core/orchestration/src/state.rs` | 步词汇扩展 | `ChangeStepKind::Clean`（`as_str` = `clean`）白名单式入封闭集（20→21）；`RunUpdate::Step` 形态不变 |
| `crates/core/orchestration`（依赖方向不变） | spawn 红线 | orchestration → workflow；零 CLI 子进程；检查域 spawn 经 checks 缝、restore_baseline 经 vcs 缝，core 零 spawn |
