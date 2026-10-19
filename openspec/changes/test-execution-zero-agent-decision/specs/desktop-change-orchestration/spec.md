# desktop-change-orchestration Specification (Delta)

## REMOVED Requirements

### Requirement: MutationTest 步接线与独立反馈预算

**Reason**: test-execution 相位内零 WorkerAgent 会话裁定下，「MutationTest 失败 → 注入本相位 executor 会话补测试杀突变 → 独立计数上限 5」的反馈边②整体退役；TestExecution 的反馈边①同批退役（见 desktop-test-execution「失败即升格交决策（零 agent 修复）」）。该相位两步改为首败即升格、把裁决权交回决策面。

**Migration**: 由「MutationTest 步接线与失败升格」承接：步序列与「绿后才跑突变」时序不变，失败处置改 `OnFail::Escalate`（零会话 + 落账 + 回决策），`feedback_limit(step, "mutation-test")` 计数面删除；`test_outcome` + `mutation_outcome` 双槽机械 checklist 语义不变。

## ADDED Requirements

### Requirement: 决策会话软拒环与漂移上限

决策会话 SHALL 自带白名单门与输出解析的软拒环，MUST NOT 以「一次解析失败 / 一次越权即 `Terminal`」处置模型漂移：每轮 SHALL `run_worker(Decision, High)` → `decision-log` 挂账（幂等覆写）→ `parse_decision` → 白名单门（`ensure_backtrack_allowed` 对 `backtrack` 动作校验，其余动作恒通过）。解析 `Err` 与门 `Err` 均 SHALL 计入漂移计数并以**同一会话续注**（`continue_session = Some(上轮 session_id)`）纠正文案重出一轮；漂移计数超过 `DECISION_DRIFT_LIMIT = 2`（即第 3 次驳回）才 `Terminal::failed`（reason 含「决策漂移」）——前两次驳回均有纠正机会。`ask` 轮次 MUST NOT 计入漂移（用户驱动的停等，非模型漂移）。每轮白名单门 SHALL emit 一个 `WhitelistGate` 节点实例（`instance_no` 递增）：通过呈 passed、驳回呈 failed 并携拒绝文案——驳回在图面可见，MUST NOT 静默丢弃。拒绝文案 SHALL 分两类：**自环**（`to == 失败相位` 且该相位不在白名单）→「本相位为确定性门禁相位（零 agent 步），不支持重做；请从白名单其他相位中选择，或 stop / ask」；**其他越权** → 携 allowed 列表的既有文案。未知动作（含已退役的 `retry`、拼写漂移）SHALL 走解析失败同一路径（软拒一次、同会话续注）。

#### Scenario: 退役动作软拒一次后纠正成功

- **WHEN** 决策 agent 首轮输出 `{"action":"retry"}`，第二轮输出白名单内 `backtrack`
- **THEN** 首轮不计 Terminal：无 `WhitelistGate` 节点（解析即失败）、漂移计数 +1、同会话续注纠正 prompt（`continue_session` 断言为上一轮会话 id）；第二轮 `WhitelistGate` passed、`backtrack` 落账；决策轮次共 2 轮、同一会话（会话数 1）

#### Scenario: 自环被拒并留图面证据

- **WHEN** 失败相位为 test-execution（零 agent 步，自身不在白名单）且决策 agent 输出 `{action:"backtrack", to:"test-execution"}`
- **THEN** `WhitelistGate` 节点呈 failed、detail 为自环文案（提示确定性门禁相位不支持重做、可 stop / ask）；漂移计数 +1 并同会话续注；第 2 次自环仍在纠正期内（计数 2），第 3 次自环使漂移超限 → `Terminal::failed("决策漂移…")`

#### Scenario: ask 轮次不计漂移

- **WHEN** 决策 agent 连续两轮输出 `ask` 并在用户应答后输出合法 `backtrack`
- **THEN** 两轮 ask 各自进入 `WaitingAsk` 停等且漂移计数保持 0；第三轮 `backtrack` 正常通过白名单门——用户驱动的停等不被判为模型漂移

#### Scenario: 白名单门每轮一节点

- **WHEN** 一次决策分叉经历「驳回 → 合法」两轮
- **THEN** 图上 `WhitelistGate` 两个节点实例（`instance_no` 1 failed、2 passed），与决策会话轮次一一对应；节点身份键稳定、可回查

### Requirement: 失败升格落账与决策信息面

失败升格落账（test-execution / mutation-test 首败及此后每次失败）SHALL 写一条 fail PhaseRecord：`report` 携完整失败明细（test-execution = `findings_detail`、mutation-test = `feedback_text`，上限 `MAX_REPORT_CHARS = 2000` 字符，超长截断留省略号），`checklist` 恰一条 `pass=false`，evidence 为诊断摘要（`diagnose_brief` 口径）+ 报告目录，executor / evaluator / decision 三会话槽位恒 `None`（本相位零会话；决策会话槽位由 `decision-log` 另行挂账）。决策有界输入 `DecisionInput` SHALL 增 `fail_report: String`（取失败条目 `report` 原文），并保留 fail checklist（fail 项 + evidence）、`allowed`（白名单）、`candidates`（候选相位最近一次 eval report）；`eval_only` 字段 SHALL 删除（retry 退役后无消费者）。决策 prompt SHALL 增「失败相位报告」段（渲染 `fail_report`），并 SHALL 明示决策 agent 可用 `read` 工具读取报告文件（test-execution 的 `reports/test/summary.json`、突变的段级 / 归并报告）取全量明细。决策输入 MUST NOT 内联全量 findings / 存活突变体清单（有界：报告文本 ≤2000 字符 + checklist 项 + 候选 report）。

#### Scenario: 决策输入携完整失败报告

- **WHEN** test-execution 升格落账后唤起决策会话（findings 明细 1500 字符）
- **THEN** `DecisionInput.fail_report` 为该明细原文、决策 prompt 含「失败相位报告」段与读报告文件指引；`DecisionInput` 无 `eval_only` 字段

#### Scenario: 决策输入有界

- **WHEN** 失败明细超 2000 字符
- **THEN** 落账 `report` 与 `DecisionInput.fail_report` 均按 2000 字符截断（省略号留痕），决策 prompt 不含全量 findings 数组 / 存活突变体清单

#### Scenario: 报告目录可回查

- **WHEN** 决策 agent 依指引 `read` 报告树
- **THEN** evidence 中的报告目录路径可解析到 `summary.json` 与逐 suite / 段级报告（升格落账与 runner 落盘同锚，路径零改写）

### Requirement: MutationTest 步接线与失败升格

walker SHALL 在 test-execution 相位按 `TEST_EXECUTION_STEPS` 步序列（`PhaseStart → TestExecution → MutationTest → PhaseLog`，定义见 desktop-workflow-write-face）顺序执行：TestExecution 结论 pass 后才执行 MutationTest 门禁步（`ToolCommand::MutationTest` 经 `MutationRunner` port 由 checks 边界 infra 实现）；TestExecution fail / error 首败即升格、截断余下步回 `phase_next` 进决策分叉——「测试 / 覆盖率修绿后再执行突变」由「升格截断 + 决策回溯后真重跑」天然保证，walker MUST NOT 在 TestExecution 未 pass 时发起突变 spawn。两步 `on_fail` SHALL 均为 `OnFail::Escalate`：失败即零会话代写 fail checklist 经写面 `phase_log` 落账（report 携完整明细、evidence 携摘要 + 报告目录），随后回 `phase_next` 进决策分叉；相位内 MUST NOT 存在反馈边（`feedback_limit` 的 test-execution / mutation-test 计数面删除）。反馈边注入对象 SHALL 仅剩 static-check 失败注入 executor、verdict 解析失败注入 evaluator。`step_phase_log` 的 test 分支 SHALL 消费 `test_outcome` + `mutation_outcome` 双槽（机械 checklist 双槽，见 desktop-test-execution）；archive 分支双槽恒 None（三分流按「产物在否」分离语义不变）。

#### Scenario: 拆步时序绿后才突变

- **WHEN** TestExecution 首次 fail（升格交决策）后经决策回溯修复重入并通过
- **THEN** MutationTest 仅在 TestExecution pass 后执行；决策等待与修复期零突变进程 spawn，步序列零乱序；重入为独立 attempt

#### Scenario: 突变失败零会话升格

- **WHEN** MutationTest 首次以 `conclusion=Fail`（未达标）或 `conclusion=Error`（段中断）收场
- **THEN** 零 WorkerAgent 会话请求、一条 fail PhaseRecord（「突变测试」一项 `pass=false`，executor / evaluator / decision 三槽位 None）、回 `phase_next` 进决策分叉；MUST NOT 注入修复会话

#### Scenario: 双槽分支分离不误判

- **WHEN** 分别驱动 test-execution 相位（test_outcome + mutation_outcome 双槽产物）与 archive 相位（双槽恒 None）
- **THEN** `step_phase_log` test 分支消费双槽机械 checklist，archive 分支仅命中空 checklist 分支，互不误判

## MODIFIED Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行**计划图驱动的 step 循环**：每相位依序迭代其 `PhaseDefinition.steps`（`StepDefinition` 序列，kind = exec | agent | gate | git，定义见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），不再在 `drive()` 内硬编码步序。每步执行后 SHALL 问写面 `phase_next` 决定推进 / 重试 / 决策（路由权威不变）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个门禁布局常量 SHALL 退役（门禁步由 step 定义声明）。initialize SHALL 为 walker 的第 1 个路由相位（两表表头一次性基线检查相位，执行体见「initialize 基线检查相位执行体」）；archive 相位 SHALL 为 walker 的第 11 个路由相位（`REQUIREMENT_TABLE` 第 10 相位，`PhaseDefinition.executor=None` / `evaluator=None`），其 11 步序列按序执行；drive 主循环 MUST NOT 保留 `next.evaluator.is_none()` 的冗余提前检查（真正消费 evaluator 的 `step_evaluator` / `step_executor` 自有 None 检查兜底）。

相位内 step 序列的特殊控制流 SHALL 以 step 级语义表达，MUST NOT 以 walker 内 step 类型特判实现：

- **feedback 边**（门禁步失败 → 同一 agent 会话修复 → 重跑）：失败步 `on_fail` 声明的定向反馈循环，独立计数 ≤5，超限升格相位 fail；每次重跑为 `instance_no` 递增的**新节点**，图上以 `feedback` 边表达。注入对象分角色：static-check 失败注入本相位 executor 会话，verdict 解析失败注入本相位 evaluator 会话（initialize 相位例外——其检查步 `on_fail=Terminal`，无反馈边；test-execution / mutation-test 例外——其步 `on_fail=Escalate`，见下条）；
- **失败升格**（门禁步失败 → 零会话落账 → 回 `phase_next` 交决策）：`on_fail=Escalate` 的步（TestExecution / MutationTest）失败即代写一条 fail checklist 落账、截断本相位余下步（`StepFlow::Upgraded`）并回 `phase_next` 进决策分叉；全程 MUST NOT 发起 WorkerAgent 会话。同一步词在不同表项语义不同（initialize 表项的 TestExecution 为 `Terminal`），语义由表项声明承载，walker MUST NOT 按相位名判断；
- **verdict 解析失败**：解析错误诊断反馈边续注同一 evaluator 会话修复重跑（MUST NOT 臆测 verdict、MUST NOT 静默跳过）；超限升格相位 fail 进决策分叉；
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
- **THEN** 写面 `phase_next` 不自走重路由同相位（盲目重跑的会话消耗高于一次决策），立即返回决策分叉触发；窗口 fail 数达 `MAX_RETRY_TIMES`（5）时以重试上限变体（`MaxRetriesExceeded`）标注、否则以 verdict fail 变体（`VerdictFail`）标注——两变体同一决策路径，仅标注与文案不同。确定性相位（initialize / archive：executor / evaluator 双缺席）不触发，fail 落账照常同相位重路由

#### Scenario: fail 重试与预算上限

- **WHEN** 同一相位窗口 fail 数达 `MAX_RETRY_TIMES`（5）且 evaluator 在场
- **THEN** `phase_next` 以重试上限变体标注分叉（与 verdict fail 变体同一决策路径）；重做本相位 SHALL 由决策回溯到自身表达（白名单自含），MUST NOT 自走同相位重跑；预算判断先于 verdict 判断，二者均进决策，无自走重跑分支

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 db `ChangeRecord.active_phase`=implement 的下一 attempt 续走，已 pass 的 initialize / proposal / dev-design 等相位不重新执行；archive 相位完成后 run completed

#### Scenario: test-execution 相位确定性承接

- **WHEN** walker 走到 test-execution 相位且 runner 结论 pass
- **THEN** 相位以 runner 门禁步承接（零 executor / evaluator 会话），机械 checklist 代写 `phase_log` 落账 pass 后推进；fail / error 首败即落账升格、回 `phase_next` 进决策分叉（零修复会话，见 desktop-test-execution「失败即升格交决策（零 agent 修复）」）

### Requirement: static-check spawn 步门禁

implement / test-gen 相位的 executor 收口后，walker SHALL 必经 static-check 步（checks 边界 `crates/infra/checks` spawn 执行，承接原 SubagentStop hook 的门禁职责；该步由相位 step 定义声明，非 `STATIC_CHECK_PHASES` 常量注入）；initialize 相位 SHALL 声明同款 static-check 步承载基线绿门（语义分叉见下）。static-check 步 SHALL 经 id → 记录 → name 解析（`store.get_change(id).map(name)`，与 test-execution 分支同式）把 change name 传入 `StaticCheckRunner::run(root, name)`——name 供 infra 侧 output 落盘（recorded，`output_ref` 回填，`<report_dir>/exec-outputs/<exec_id>.log`）；未建档（无 `ChangeRecord`）显式 `Err`。implement / test-gen 相位的 static-check 失败 SHALL 走**定向反馈边**：将捕获的诊断反馈注入同一 executor 会话（Continue）修复，独立计数上限 5 次（沿原 hook `loop_limit` 语义）；每次修复重跑为 `instance_no` 递增的新节点、图上以 `feedback` 边表达。超限 SHALL 升格为相位 fail（walker 以桌面代写的 fail checklist 经写面 `phase_log` 落账，不跑 evaluator，进入决策分叉——重做本相位经决策回溯到自身表达（白名单自含），窗口 fail 计数即预算面）。MUST NOT 将反馈边重试直接计入相位 retry 预算。initialize 相位的 static-check 步失败 SHALL 以 `Terminal` 收敛（`on_fail=Terminal`，无反馈边、零 agent 会话注入——基线的债不属于本次变更，见「initialize 基线检查相位执行体」）。非 implement / test-gen / initialize 相位 MUST NOT 触发 static-check（其 step 序列不含 static-check 步）。

#### Scenario: 修复反馈边

- **WHEN** executor 收口后 static-check 报 lint / fmt 错误
- **THEN** walker 将诊断文本注入同一 executor 会话（Continue）重试修复，反馈边计数 +1，新修复节点 `instance_no` 递增；修复后 static-check 通过则相位继续 evaluator 步

#### Scenario: 反馈边超限升格

- **WHEN** 同一相位内定向反馈边已达 5 次仍不通过
- **THEN** walker 以代写 fail checklist 经写面 `phase_log` 落账（不跑 evaluator），相位按 fail 收场进入决策分叉（重做本相位经决策回溯到自身表达），walker 不再注入反馈边

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

Evaluator 会话 SHALL 依 prompt 约定在最终消息输出 checklist JSON（不自调 MCP `phase_log`）。Walker SHALL 解析该 JSON 并代调写面 `phase_log`（进程内）落 eval 记录（PhaseRecord + ChecklistItemRecord 原子落库）；解析失败 SHALL 走反馈边（解析错误诊断续注同一 evaluator 会话修复重跑，独立计数 ≤5），MUST NOT 臆测 verdict、MUST NOT 静默跳过落账；超限 SHALL 升格相位 fail（桌面代写 fail phase-log）进决策分叉（重做本相位经决策回溯到自身表达）。checklist JSON 结构 SHALL 瘦身为 `{ verdict, report, checklist }`（checklist 逐项 `{ item, pass, evidence }`）；phase / attempt / skipped SHALL 由 walker 盖戳落账（evaluator 不回声），`EvaluatorChecklist` 解析类型 MUST NOT 要求 phase / attempt / skipped 字段。确定性相位（test-execution 机械 checklist、archive 空 checklist）SHALL 走 `step_phase_log` 的独立分支，不经 evaluator verdict 解析。

#### Scenario: verdict 代写落账

- **WHEN** evaluator 最终消息输出合法 checklist JSON（verdict=pass，无 phase / attempt / skipped 字段）
- **THEN** walker 以该 checklist 盖 phase / attempt / skipped 后经写面 `phase_log` 落 eval 记录，`phase_next` 随后推进

#### Scenario: 解析失败反馈边修复

- **WHEN** evaluator 最终消息不含可解析的 checklist JSON（结构漂移或空输出）
- **THEN** 门步节点呈失败态，解析失败原因注入同一 evaluator 会话（Continue，`feedback` 边）修复后重解析（`instance_no` 递增新节点）；当轮修复期间状态库无新增 eval 记录

#### Scenario: 解析失败超限升格

- **WHEN** 同 attempt 内解析失败经 5 次反馈修复仍未收敛
- **THEN** walker 代写 fail phase-log（report 携升格记因、fail checklist 携解析错误、executor + evaluator 双槽位）升格相位 fail，回写面 `phase_next` 进决策分叉（重做本相位经决策回溯到自身表达）

### Requirement: 决策协议

写面 `phase_next` 返回决策分叉触发（任意相位最新 fail 条目未挂决策会话；或窗口 fail 数达重试上限的标注变体）时，walker SHALL 唤起决策 agent（compose_turn 新会话，`ModelLevel::High`）；walker 自走同相位重跑 SHALL 退役（盲目重跑的会话消耗高于一次决策）。确定性相位（initialize / archive：executor / evaluator 双缺席）不触发决策，fail 落账照常同相位重路由。决策 agent 输入 SHALL 有界且来自 db 状态与写面响应：fail checklist（fail 项 + evidence）、失败条目 `report`（失败报告原文，见「失败升格落账与决策信息面」）、`allowed_backtrack_phases`（写面 `phase_next` 白名单）、候选相位最近一次 eval report。决策输入 MUST NOT 携相位 retry 语义标记（retry 已退役，无「评估-only / 带 executor」二分支指引）。决策输出 SHALL 为封闭集三动作：

- `{ action: "backtrack", to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；目标为失败相位自身即「重做本相位」（仅当该相位在白名单内）；
- `{ action: "stop", reason }` — 终止 run；
- `{ action: "ask", question, options[] }` — 无法裁决，UI 中断提问。

未在封闭集内的动作（含已退役的 `retry`）SHALL 视为模型漂移，走「决策会话软拒环与漂移上限」的软拒路径，MUST NOT 一次即打死 run。重做语义 SHALL 由白名单自含承载（单一载体）：`allowed_backtrack_phases` 含失败相位自身 ⟺ 该相位可重做（其 step 序列含 agent 步）；零 agent 步的相位（test-execution / archive）自身不入白名单——决策上下文里不存在「重做本相位」这个选项。

决策决议的兑现 SHALL 依赖「每个 fail 条目只触发一次决策」语义：决策会话挂账（decision_session_id）后的同一条目 MUST NOT 重复触发决策分叉（该条目已进入过决策，不再重复分叉）。唯一合法重入路径 SHALL 为决策 backtrack——写面改写最新条目 `backtrack_to` → 下一轮 `phase_next` 的回溯分支先手接管路由（走不到两道分叉判断）→ 目标相位真执行 → 新落账 fail 条目无决策标记，触发点对新条目照常生效。MUST NOT 以「放行同相位重跑」作为兑现面：删掉该幂等闸后，fail 过的相位在重做 pass 后仍会因最新 fail 条目再次分叉、相位永不执行，直烧到 `MAX_ROUNDS=20` 硬失败。

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验（工具侧兜底）：越权目标 SHALL 被拒绝且不落任何越权回跳账（状态库零变更）。决策输出解析失败 / 越权由「决策会话软拒环与漂移上限」处置（软拒纠正、漂移超限 Terminal）。

#### Scenario: 白名单内自治回跳

- **WHEN** test-execution#1 fail 且决策 agent 输出 `{action:"backtrack", backtrack_to:"test-gen", reason:...}`（test-gen 在白名单内）
- **THEN** walker 经写面 `backtrack` 落回跳记录（落库），`phase_next` 随后路由至 test-gen 重做，reason 随边标签可读；重做相位首节点 incoming 边为 `backtrack`

#### Scenario: 越权决议被兜底拦截

- **WHEN** 决策 agent 输出的 backtrack_to 不在 allowed_backtrack_phases 内（或写面 `backtrack` 被直接以越权目标调用）
- **THEN** walker 侧白名单门软拒（failed `WhitelistGate` 节点留图面证据、同会话续注纠正），漂移超限后 run 停给用户并呈现越权详情；写面 `backtrack` 的二次校验对越权目标仍显式拒绝且状态库零变更（不落任何越权回跳账）

#### Scenario: ask 中断提问

- **WHEN** 决策 agent 输出 `{action:"ask", question:..., options:[...]}`
- **THEN** run 进入中断等待态，UI 呈现问题与选项；用户应答后 run 按应答继续（应答经运行控制命令面回流 walker，同一决策会话续注）

#### Scenario: verdict fail 分叉与重做自含

- **WHEN** code-review#1 落 verdict=fail（评估-only 相位，自身在白名单内）且决策 agent 输出 `{action:"backtrack", to:"code-review", reason:...}`
- **THEN** walker 唤起决策 agent 恰一次，回溯落账后同相位以新 attempt 真执行（重做本相位由白名单自含表达，`retry` 动作不存在）；重跑再落新 fail 条目时决策分叉对新条目照常触发。带 executor 相位（如 implement）verdict fail 同式分叉，重做携带失败清单段

#### Scenario: verdict fail 分叉与 retry 兑现

- **WHEN** 审查决策协议的兑现路径（本条为历史 scenario 名的存续面）
- **THEN** 原「`retry` 决议兑现」路径已退役（`DecisionAction::Retry` 删除、未知名软拒），兑现语义改为「每个 fail 条目只触发一次决策」幂等闸 + 决策 backtrack 重入；重做本相位经回溯到自身兑现（见「verdict fail 分叉与重做自含」与「已决条目不重复分叉」）

#### Scenario: 已决条目不重复分叉

- **WHEN** 某相位最新 fail 条目已挂决策会话（decision_session_id 在案）后 `phase_next` 再被调用
- **THEN** 该条目不触发第二次决策分叉（幂等闸生效）；决策回溯到上游后重入产生的新 fail 条目（无决策槽位）照常分叉

### Requirement: 四类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为四类节点（step 定义声明 kind）：

- **WorkerAgent 节点**（executor / evaluator / decision / spec_sync / resolve_conflicts 的 agent 会话步）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界 + preflight / seal 内联校验与写面步）：无智能；
- **Git 节点**（clean / commit / rebase / merge / cleanup / finalize vcs 步，落 vcs 边界 `crates/infra/vcs`）：经独立 vcs 缝 `Arc<dyn ArchiveVcsPort>` 进程 spawn 执行；initialize 相位 `clean` 为基线恢复步（`restore_baseline`，见「initialize 基线检查相位执行体」）；`cleanup` 为尽力步——失败仅 step detail 警告、步恒 `passed`（不呈失败态，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；
- **Gate 节点**（verdict 解析 / 白名单校验 / 决策动作兑现——`retry_gate` 为「决策动作兑现门」的历史命名，IPC 词面冻结）：纯 Rust 分支。

ToolStep / Git MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord`）∪ 进程内注册表在飞 run（`RunEntry` 节点 + 边累积器）**；节点行 SHALL 直接携 `session_id`（会话转录经 session id 直查，不再按 provenance 反查会话集）；执行图节点 SHALL 以稳定身份键 `(run_id, phase, attempt, step, instance_no)` 寻址，边 SHALL 以 `RunEdgeRecord` 四值 `edge_kind ∈ { forward, feedback, retry, backtrack }` 显式落库（`retry` 为**存量解码专用**：新写面零产出、查询面投影前屏蔽，见 desktop-change-queries；决策回溯到自身产生的重做入口边为 `backtrack`）。步词汇 SHALL 为全词汇封闭集（agent 阶段 + 脚本阶段 + 流程面步骤 + 三门 gate + 9 个归档步 + initialize 基线步 clean），落库与回显同一份词汇。已收口 run 的任一节点状态与边 SHALL 可由库读史单独重算得出。

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
- **THEN** initialize 步词节点（phase_start / clean / static_check / test_execution / phase_log）与归档 9 个步词节点全落 `RunNodeRecord`（四类节点类型可辨），串行 forward 边落边表；同 run 若另含 feedback / backtrack，该二 kind 边齐备且显式落库（`retry` 仅存量库行可达、新写面零产出）

### Requirement: 计划图与执行图对齐与步粒度回退

计划图（源码，desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）与执行图（db，desktop-change-state-store「run 执行图落库」）SHALL 经 **step kind + phase + attempt + instance_no** 对齐：执行图节点是计划 step 的运行时实例，节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`；执行图边 SHALL 显式落 `RunEdgeRecord`，`edge_kind ∈ { forward, feedback, retry, backtrack }`——archive 相位六段串行（实为 10 步）为 forward 边，无回边；`retry` 为存量解码专用（新写面 MUST NOT 产出，查询面投影前屏蔽），决策回溯到自身的重做入口边为 `backtrack`。计划图 MUST NOT 落库（定义恒为编译期静态）；执行图 MUST NOT 上位为路由权威。回退（backtrack）的语义单位 SHALL 保持 phase 粒度；本变更 MUST NOT 把回退载荷改为 step 粒度。

#### Scenario: 节点身份稳定且 feedback 独立

- **WHEN** 同 phase 同 attempt 内 static-check 失败触发 executor 修复重跑，随后决策回溯到该相位自身（重做）
- **THEN** executor 首次与修复各为 `instance_no` 递增的独立节点，feedback 边指向前一节点；重做产生新 attempt，其相位入口边为 `backtrack`（新写面 MUST NOT 产出 `retry` 边）

#### Scenario: 归档步为 forward 边

- **WHEN** archive 相位一次 run 收口后查询执行图
- **THEN** 归档 10 步节点（含 8 个新步词）以 forward 边串行落 `RunEdgeRecord`，零 feedback / retry / backtrack 边；`run_id` / `from_node_key` 两二级索引可达

#### Scenario: 边四值与查询

- **WHEN** 审查一次含前进 / 反馈 / 回跳的 run 执行图
- **THEN** forward / feedback / backtrack 三 kind 齐备且 `run_id` / `from_node_key` 两二级索引可达（`retry` 仅存量解码面可达、投影面屏蔽）；`backtrack` 边指向白名单内目标相位首节点（含指向失败相位自身相位的重做入口）

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/orchestration/src/decision.rs` | 决策动作封闭集 | `DecisionAction::{ Backtrack { to, reason }, Stop { reason }, Ask { question, options } }`（`Retry` 变体删除；取代 Module Contract 原「决策四动作封闭集」表述）；`DecisionInput` 删 `eval_only`、增 `fail_report: String` |
| `crates/core/orchestration/src/prompt.rs`（`decision_prompt`） | 决策 prompt 组装 | 三动作协议段；白名单条目对可重做相位自注记「（= 重做本相位）」；「失败相位报告」段渲染 `fail_report`；读报告文件指引；删 retry 二分支指引 |
| `crates/core/orchestration/src/walker.rs`（`decision_session`） | 决策会话软拒环 | 每轮 `run_worker(Decision, High)` → `decision-log` → `parse_decision` → 白名单门；`DECISION_DRIFT_LIMIT = 2`；ask 不计数；每轮 emit `WhitelistGate` 实例 |
| `crates/core/orchestration/src/walker.rs`（`resolve_deadlock`） | 决议兑现 | 删 `DecisionAction::Retry` 分支；backtrack 经写面落账后置 `trace.incoming = Backtrack`；白名单门从 resolve_deadlock 迁入 `decision_session` |
| `crates/core/orchestration/src/walker.rs`（`step_test_execution` / `step_mutation_test`） | 门禁步执行体 | 删反馈循环与 `fix_session`；`on_fail == Some(Escalate)` 前置分支（代写 fail 落账 → `StepFlow::Upgraded`）；保留 `Terminal` 前置分支与 runner `Err → Terminal` |
| `crates/core/workflow/src/write/phase_next.rs` | 路由与幂等闸 | `fail_decided` 保留、注释改为「该 fail 条目已进入过决策，不再重复分叉」；`eval_only` 派生删除、`PhaseNextError` 两变体去字段；`MaxRetriesExceeded` 文案改「本相位已连续失败 N 次」 |
| `crates/core/orchestration/src/state.rs` | Gate 节点词面 | `ChangeStepKind::RetryGate` 变体名与线词不变，注释改为「决策动作兑现门」 |
| `crates/core/workflow/src/state.rs` | 边词汇 | `RunEdgeKind::Retry` 变体保留（解码契约），注释改为「存量解码专用、新写面零产出、查询面投影前屏蔽」 |
