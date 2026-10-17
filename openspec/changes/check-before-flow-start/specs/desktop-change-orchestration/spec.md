# desktop-change-orchestration Specification (Delta)

## ADDED Requirements

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

## MODIFIED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs` | initialize 相位执行体 | `step_clean`（vcs 直调 `restore_baseline`，仅 worktree 在场执行；legacy 整相位短路先例同归档步）；static-check / test-execution 工具步 `OnFail::Terminal` 专用分支（`feedback_limit` 不再拒绝工具步 Terminal）；Error / Fail 分文案 Terminal reason；PhaseLog 机械 checklist 沿 `test_execution_phase_log_input`（report 指向 baseline 目录） |
| `crates/core/orchestration/src/port.rs` | vcs 缝扩展 | `ArchiveVcsPort::restore_baseline(worktree, 排除面)`（sync 签名；返回恢复摘要或 Err 带 git 语境）；port 归属消费者先例不变 |
| `crates/core/orchestration/src/steps.rs` | ToolStep 面随动 | `LocalToolSteps` 组合不变；检查 runner 调用透传 baseline 报告槽位（形态 design 定稿） |
| `crates/core/orchestration/src/state.rs` | 步词汇扩展 | `ChangeStepKind::Clean`（`as_str` = `clean`）白名单式入封闭集（20→21）；`RunUpdate::Step` 形态不变 |
| `crates/core/orchestration`（依赖方向不变） | spawn 红线 | orchestration → workflow；零 CLI 子进程；检查域 spawn 经 checks 缝、restore_baseline 经 vcs 缝，core 零 spawn |
