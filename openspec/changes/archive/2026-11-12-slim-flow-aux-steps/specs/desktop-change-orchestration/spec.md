# desktop-change-orchestration Specification (Delta)

## RENAMED Requirements

### RENAMED: verdict 解析与 phase-log 代写 → evaluator 收口裁决内化与相位收口落账

- **FROM**: `### Requirement: verdict 解析与 phase-log 代写` —— Evaluator 在最终消息输出 checklist JSON，walker 解析该 JSON 并走「解析失败 → 反馈边续注 → 超限升格」，确定性相位走 `step_phase_log` 的独立分支。
- **TO**: `### Requirement: evaluator 收口裁决内化与相位收口落账` —— 内容如下：

Evaluator 会话 SHALL 以 `finish_step` 工具收口（协议定义见 desktop-agent-execution，非本能力射程），其结构化载荷经 `WorkerTurnOutcome.finalized` 抵达 walker。**verdict 裁决 SHALL 内化进 `step_evaluator` 步本体**：读 `finalized` 的 `FinishPayload::Evaluator(checklist)` 臂即「本步 passed、携 `verdict`」，缺席即「本步 failed + `Terminal::failed`」——**不存在独立的 verdict 门步，也不存在独立节点**。MUST NOT 臆测 verdict、MUST NOT 静默跳过落账、MUST NOT 从最终消息文本二次解析。

裁决与终态 emit 的**先后顺序** SHALL 由 `run_worker` 的 `SettleMode` 承载：`SettleMode::Deferred` 只发 `running` 拍、终态由调用方在 `finalized` 判定之后一次性 emit（evaluator 与 decision 两会话步用 `Deferred`，因它们的终态取决于收口载荷是否在场）；`SettleMode::Immediate`（缺省，其余会话步）语义与既有逐字一致。MUST NOT 保留「先 emit passed 再发现无收口载荷」的自相矛盾拍（同实例既有 passed 又有 failed 终态会导致节点终态取决于 emit 序）。

缺席成因 SHALL 分两文案（判据 `transcript` 内是否含 `output_truncated` notice）：**输出被截断** → 「评估输出被截断（output_truncated），未产生 finish_step 收口产物」；否则 → 「未调用 finish_step 收口（相位 evaluator 会话 Completed 但无收口载荷）」。

落账 SHALL 内化为**相位收口动作** `settle_phase`（非计划步、非图节点）：相位内计划步全部走完后由 walker 调用一次——三分流按「产物在否」分离（test-execution 相位走 `test_outcome` + `mutation_outcome` 双槽机械 checklist → 评估相位走 `evaluator_turn` 的 checklist → 无二者（archive）走空 checklist，`derive_verdict([]) = Pass`），语义与既有三分流逐字一致、零 `phase == archive` 特判。`phase` / `attempt` / `skipped` 由 walker 盖戳，`ChecklistItem` 形状（`item` / `pass` / `evidence` 三字段）不变，`item` 为 md 静态清单「检查项」列的权威正文。落账产生的 `RunUpdate::Step` 与 `StepRecord` 审计行 SHALL NOT 存在（收口是相位机的固定动作，不是可观测步）。

#### Scenario: 收口载荷落账并推进

- **WHEN** evaluator 经 `finish_step` 提交合规 checklist（逐项 pass，report 1000 字符）且相位内所有计划步走完
- **THEN** evaluator 节点为其 attempt 内**唯一**实例且 passed（detail 含 `verdict=pass`）；`settle_phase` 以该 checklist 经写面 `phase_log` 落 eval 记录（verdict 由 checklist 推导）；`phase_next` 随后推进；无 `verdict_gate` / `phase_log` 节点、无 `feedback` 边

#### Scenario: 收口产物缺席即失败

- **WHEN** evaluator 会话以 `status = Completed` 结束但 `finalized` 为 `None`（模型自然结束、未调 `finish_step`）
- **THEN** 该 evaluator 节点呈 failed（detail 为「未调用 finish_step 收口」类文案），相位以 `Terminal::failed` 收口；状态库无新增 eval 记录、零反馈边重试、零 `settle_phase` 调用

#### Scenario: 截断成因可区分

- **WHEN** evaluator 输出因 `max_tokens` 截断收场（转录含 `output_truncated` notice）且 `finalized` 缺席
- **THEN** evaluator 节点 failed 的成因文案点明「输出被截断」而非「未调用 finish_step」，两者不共用同一段文案

#### Scenario: 确定性相位三分流互不误判

- **WHEN** 分别驱动评估相位（`evaluator_turn` 产物）、test-execution 相位（`test_outcome` + `mutation_outcome` 双槽产物）与 archive 相位（二者皆无）
- **THEN** `settle_phase` 三分支按产物在否分离，无按 phase 名判断、无跨分支误判；archive 分支落空 checklist 且 `verdict=pass`

#### Scenario: 收口判定先于终态拍

- **WHEN** evaluator 会话以 `status = Completed` 结束且 `finalized` 为 `None`
- **THEN** 该 evaluator 节点只出现一次终态拍且为 failed（`SettleMode::Deferred` 下 `run_worker` 只发 running、终态由 `step_evaluator` 在判定后发出）；MUST NOT 出现同实例 passed 后又被 failed 覆盖的拍序

#### Scenario: 收口不出节点不出审计

- **WHEN** 审查一次完整 run 的 `RunNodeRecord` / `StepRecord`
- **THEN** 无 `phase_log` 节点行、无 `StepKind::PhaseLog` 审计行；相位落账只体现为 `PhaseRecord` + `ChecklistItemRecord` 与 `active_phase` 清位

## MODIFIED Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行**计划图驱动的 step 循环**：每相位依序迭代其 `PhaseDefinition.steps`（`StepDefinition` 序列，kind = exec | agent | gate | git，定义见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），不再在 `drive()` 内硬编码步序。**`steps` SHALL 只含真实业务步**：开相与落账是相位机的固定动作（`enter_phase` / `settle_phase`，见「相位入口与收口内化（计划图仅含真实步）」），MUST NOT 以计划步表达。每步执行后 SHALL 问写面 `phase_next` 决定推进 / 重试 / 决策（路由权威不变）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个门禁布局常量 SHALL 退役（门禁步由 step 定义声明）。initialize SHALL 为 walker 的第 1 个路由相位（两表表头一次性基线检查相位，执行体见「initialize 基线检查相位执行体」）；archive 相位 SHALL 为 walker 的第 11 个路由相位（`REQUIREMENT_TABLE` 第 10 相位，`PhaseDefinition.executor=None` / `evaluator=None`），其 9 步序列按序执行；drive 主循环 MUST NOT 保留 `next.evaluator.is_none()` 的冗余提前检查（真正消费 evaluator 的 `step_evaluator` 自有 None 检查兜底）。

相位内 step 序列的特殊控制流 SHALL 以 step 级语义表达，MUST NOT 以 walker 内 step 类型特判实现：

- **feedback 边**（门禁步失败 → 同一 agent 会话修复 → 重跑）：失败步 `on_fail` 声明的定向反馈循环，独立计数 ≤5，超限升格相位 fail；每次重跑为 `instance_no` 递增的**新节点**，图上以 `feedback` 边表达。注入对象 SHALL 仅 static-check / archi-check 失败注入本相位 executor 会话（initialize 相位例外——其检查步 `on_fail=Terminal`，无反馈边；test-execution / mutation-test 例外——其步 `on_fail=Escalate`，见下条）；
- **失败升格**（门禁步失败 → 零会话落账 → 回 `phase_next` 交决策）：`on_fail=Escalate` 的步（TestExecution / MutationTest）失败即代写一条 fail checklist 落账、截断本相位余下步（`StepFlow::Upgraded`）并回 `phase_next` 进决策分叉；全程 MUST NOT 发起 WorkerAgent 会话。同一步词在不同表项语义不同（initialize 表项的 TestExecution 为 `Terminal`），语义由表项声明承载，walker MUST NOT 按相位名判断；
- **归档步失败**：archive 相位步 `on_fail=None` 的失败即 `Terminal`（不唤决策、不进相位重试）；`Cleanup` 为唯一例外——尽力步恒以 `passed` 收口（失败仅 step detail 警告，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；
- **phase 间 confirm 停等**：仍为 run 级 `auto_next_phase` 开关，并支持「按指定 phase 停止」（archive 为固定停止点）。

红线保持：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份（requirement 10 列含 initialize 与 archive、test-only 7 列），MUST NOT 上位为路由权威。重启 run SHALL 自 active_phase 续走（active_phase 持久化于 db `ChangeRecord`，已 pass 相位 MUST NOT 重头执行，续走由 `phase_next` 依 eval 历史重算）。写面全部落库载体为 workspace 库，walker 对载体无感知。

#### Scenario: 相位内计划步全循环自动走完

- **WHEN** 以假引擎 + 假写面 + 假 vcs port 驱动某相位 run（含 archive 相位 9 步）
- **THEN** walker 依该相位 `PhaseDefinition.steps` 序逐 step 执行（exec / agent / gate / git 按声明），pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: archive 相位确定性承接

- **WHEN** walker 走到 archive 相位且 9 步全 pass
- **THEN** 相位以 Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize 承接（零 executor / evaluator 会话），相位收口以空 checklist 落账 pass 后推进 done

#### Scenario: evaluator 检查下沉不破坏安全不变量

- **WHEN** 审查 drive 主循环与 `step_evaluator`
- **THEN** drive 无 `next.evaluator.is_none()` 检查；声明 Evaluator 步而 `evaluator=None` 的漂移仍被 `step_evaluator` 的 None 检查拦截；archive 相位（`evaluator=None` 且 steps 无 Evaluator 步）零拦截

#### Scenario: verdict fail 直接决策分叉

- **WHEN** 任意带评估步或升格门禁步的相位落 verdict=fail 条目（最新 fail 条目未挂决策会话）
- **THEN** 写面 `phase_next` 不自走重路由同相位（盲目重跑的会话消耗高于一次决策），立即返回决策分叉触发；窗口 fail 数达 `MAX_RETRY_TIMES`（5）时以重试上限变体（`MaxRetriesExceeded`）标注、否则以 verdict fail 变体（`VerdictFail`）标注——两变体同一决策路径，仅标注与文案不同。判据 SHALL 取「`evaluator.is_some()` 或 `steps` 含 `OnFail::Escalate` 步」：test-execution 为零角色 spec 的确定性相位，其门禁步的 `Escalate` 声明即「首败交决策」的自描述，故**照常触发**分叉。无评估步且无升格步的确定性相位（initialize / archive）不触发，fail 落账照常同相位重路由

#### Scenario: fail 重试与预算上限

- **WHEN** 同一相位窗口 fail 数达 `MAX_RETRY_TIMES`（5）且该相位在决策分叉面内（带评估步或升格门禁步）
- **THEN** `phase_next` 以重试上限变体标注分叉（与 verdict fail 变体同一决策路径）；重做本相位 SHALL 由决策回溯到自身表达（白名单自含），MUST NOT 自走同相位重跑；预算判断先于 verdict 判断，二者均进决策，无自走重跑分支

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 db `ChangeRecord.active_phase`=implement 的下一 attempt 续走，已 pass 的 initialize / proposal / dev-design 等相位不重新执行；archive 相位完成后 run completed

#### Scenario: test-execution 相位确定性承接

- **WHEN** walker 走到 test-execution 相位且 runner 结论 pass
- **THEN** 相位以 runner 门禁步承接（零 executor / evaluator 会话），机械 checklist 由相位收口落账 pass 后推进；fail / error 首败即落账升格、回 `phase_next` 进决策分叉（零修复会话，见 desktop-test-execution「失败即升格交决策（零 agent 修复）」）

### Requirement: 四类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为四类节点词汇（step 定义声明 kind）——辅助步退场后**新写面只产前三类**（WorkerAgent / ToolStep / Git），Gate 类为零产出的存量解码词汇：

- **WorkerAgent 节点**（executor / evaluator / decision / spec_sync / resolve_conflicts 的 agent 会话步）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（static-check / test-execution / mutation-test / archi-check 检查域 spawn 步落 checks 边界 + preflight / seal 内联校验与写面步）：无智能；
- **Git 节点**（clean / commit / rebase / merge / cleanup / finalize vcs 步，落 vcs 边界 `crates/infra/vcs`）：经独立 vcs 缝 `Arc<dyn ArchiveVcsPort>` 进程 spawn 执行；initialize 相位 `clean` 为基线恢复步（`restore_baseline`，见「initialize 基线检查相位执行体」）；`cleanup` 为尽力步——失败仅 step detail 警告、步恒 `passed`（不呈失败态，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；
- **Gate 节点**（`verdict_gate` / `retry_gate` / `whitelist_gate`）：**存量解码专用词汇**——辅助步退场后新写面零产出（verdict 裁决内化进 evaluator 步、动作裁决写 decision 节点 detail），三词与 `StepGroup::Gate` 分支为旧库行解码必需而保留，MUST NOT 删除变体或重排。

ToolStep / Git MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord`）∪ 进程内注册表在飞 run（`RunEntry` 节点 + 边累积器）**；节点行 SHALL 直接携 `session_id`（会话转录经 session id 直查，不再按 provenance 反查会话集）；执行图节点 SHALL 以稳定身份键 `(run_id, phase, attempt, step, instance_no)` 寻址，边 SHALL 以 `RunEdgeRecord` 四值 `edge_kind ∈ { forward, feedback, retry, backtrack }` 显式落库（`retry` 为**存量解码专用**：新写面零产出、查询面投影前屏蔽，见 desktop-change-queries；决策回溯到自身产生的重做入口边为 `backtrack`）。步词汇 SHALL 为全词汇封闭集（agent 阶段 + 脚本阶段 + 流程面步骤 + 三门 gate（存量解码）+ 9 个归档步 + initialize 基线步 clean），落库与回显同一份词汇，二十二值冻结。已收口 run 的任一节点状态与边 SHALL 可由库读史单独重算得出。

#### Scenario: ToolStep / Git 失败同等可见

- **WHEN** static-check / test-execution spawn 步、vcs 步（clean / commit / rebase / merge / finalize）或内联校验步以非零退出 / 显式 Err 收场
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: Cleanup 尽力步警告可见不呈失败

- **WHEN** archive 相位 Cleanup 步的 `git worktree remove` 非零退出
- **THEN** 该节点 `passed` 收口、`detail` 携警告（worktree 路径 + 手动清理指引）在图上可读；run 续走 Seal / Finalize 与相位收口正常完成，链不因清理失败停摆

#### Scenario: 节点与边两源派生可重算

- **WHEN** 审查编排运行时的持久化面与读路
- **THEN** runs / run_nodes / run_edges 三表在案、在飞 run 节点与边驻进程内注册表（RunEntry）；已收口 run 的任一节点状态与边均可由库读史重算得出，不依赖进程内存

#### Scenario: 归档步词汇上图与边四值可辨

- **WHEN** 一次 run 含 initialize 相位 3 步与 archive 相位 9 步（含 git 步与相位收口）
- **THEN** initialize 步词节点（clean / static_check / test_execution）与归档 9 个步词节点全落 `RunNodeRecord`（四类节点类型可辨），串行 forward 边落边表；同 run 若另含 feedback / backtrack，该二 kind 边齐备且显式落库（`retry` 仅存量库行可达、新写面零产出）

### Requirement: 决策协议

写面 `phase_next` 返回决策分叉触发（任意相位最新 fail 条目未挂决策会话；或窗口 fail 数达重试上限的标注变体）时，walker SHALL 唤起决策 agent（compose_turn 新会话，`ModelLevel::High`）；walker 自走同相位重跑 SHALL 退役（盲目重跑的会话消耗高于一次决策）。确定性相位（initialize / archive：executor / evaluator 双缺席）不触发决策，fail 落账照常同相位重路由。决策 agent 输入 SHALL 有界且来自 db 状态与写面响应：fail checklist（fail 项 + evidence）、失败条目 `report`（失败报告原文，见「失败升格落账与决策信息面」）、`allowed_backtrack_phases`（写面 `phase_next` 白名单，**随分叉错误响应下发**——walker MUST NOT 跨轮缓存（run 首轮即分叉时无「上一站」可缓存），MUST NOT 自相位表推导；每项 SHALL 携相位特性描述与该相位声明的可操作产物清单，描述 / 产物单源 = 本期相位表 `PhaseDefinition.description` / `PhaseDefinition.artifact_manifest`，随表分派不串味）、候选相位最近一次 eval report、`backtrack_window`（回溯重做窗口集合，透传自写面 error 响应——每项携发起相位 / 目标相位 / 两个泳道序 / 原因原文；walker MUST NOT 自读 store、MUST NOT 自相位表二次推导，判据单源见 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」）。决策 prompt 白名单段 SHALL 逐条渲染 `- <相位 id>（<特性描述>）：<可操作产物段>`，供决策 agent 判断回溯目标（回溯即重做该相位产物；描述空串退化为 id-only 行，零声明产物渲染「无声明产物」）。决策输入 MUST NOT 携相位 retry 语义标记（retry 已退役，无「评估-only / 带 executor」二分支指引）。决策输出 SHALL 为**经 `finish_step` 工具提交**的封闭集三动作（不在最终消息输出 JSON）：

- `{ action: "backtrack", to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；目标为失败相位自身即「重做本相位」（仅当该相位在白名单内）；
- `{ action: "stop", reason }` — 终止 run；
- `{ action: "ask", question, options[] }` — 无法裁决，UI 中断提问。

封闭集外的动作（含已退役的 `retry`）与形状漂移 SHALL 由 `finish_step` 的工具校验 `Err` **当场回灌**（is_error ToolResult，模型在同一 turn 内纠正重试）——MUST NOT 一次即打死 run，MUST NOT 跨轮计数。重做语义 SHALL 由白名单自含承载（单一载体）：`allowed_backtrack_phases` 含失败相位自身 ⟺ 该相位可重做（其 step 序列含 agent 步）；零 agent 步的相位（test-execution / archive）自身不入白名单——决策上下文里不存在「重做本相位」这个选项。

决策 prompt SHALL 增「回溯重做窗口」段：段内容 SHALL 复用写面渲染单点（MUST NOT 在 orchestration 侧另持第二份文案或第二份判据），只渲染各窗口的原因原文、以 `<backtrack reason>` / `</backtrack>` 定界、零相位 id 与零属性；集合为空时整段省略（不留标题）。段位置 SHALL 在 `## 失败相位报告` **之前**（段序最前，紧跟开头一句「相位…评估失败，需要你做出路由决策」之后）——背景信息比失败明细更宏观。该段与「可回溯白名单」段不构成语义混淆：白名单供 agent 选目标（携相位 id 与产物、有硬约束力），窗口段供 agent 知道为什么被叫回来（零相位 id、零约束力）。

段内条目数 SHALL 由写面下发的集合决定，且该集合的规模语义 MUST 写成双分支（与写面同一双分支，MUST NOT 只写其中一支）：**未超过写面上限（`MAX_BACKTRACK_WINDOWS`，0 / 1 / 2 层）时全量并集**（0 层即整段省略，1 / 2 层全部渲染，多窗互不暂停互不覆盖）；**超过上限时取创建序末位两层**，被裁的恒为创建序最早那条（等价于该组中 `from_index` 最大者；「最旧最宽」只是其常见口语后果）。越限裁剪的**唯一实现点**在写面「回溯重做窗口推导与 prompt 携带」——本侧对传入集合逐条渲染，MUST NOT 自行实现一次上限裁剪（第二套裁剪会与写面的并集语义分叉）；该常量定义在写面 `phase_table.rs`，本侧只引用其语义、不另行定义。

决策面携带窗口的收益 SHALL 如实记为**语义级**而非结构级：窗口段只渲染原因原文（不含 `from` / `to`），故「同一原因再次回跳同一目标」只能靠原因文本的语义理解识别，MUST NOT 宣称其与「渲染 `from` / `to` 的结构判据」等价。该降级已接受（现状决策 prompt 零历史回溯信息，弱化后仍显著优于现状；深层重复回跳仍由 `MAX_ROUNDS=20` 兜底）；升级路径为渲染 fn 单点加回 `from` / `to`，结构体与判据零改动。

决策决议的兑现 SHALL 依赖「每个 fail 条目只触发一次决策」语义：决策会话挂账（decision_session_id）后的同一条目 MUST NOT 重复触发决策分叉（该条目已进入过决策，不再重复分叉）。唯一合法重入路径 SHALL 为决策 backtrack——写面改写最新条目 `backtrack_to` → 下一轮 `phase_next` 的回溯分支先手接管路由（走不到两道分叉判断）→ 目标相位真执行 → 新落账 fail 条目无决策标记，触发点对新条目照常生效。MUST NOT 以「放行同相位重跑」作为兑现面：删掉该幂等闸后，fail 过的相位在重做 pass 后仍会因最新 fail 条目再次分叉、相位永不执行，直烧到 `MAX_ROUNDS=20` 硬失败。

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验（工具侧兜底）：越权目标 SHALL 被拒绝且不落任何越权回跳账（状态库零变更）。决策收口后 walker SHALL 仍执行 `ensure_backtrack_allowed`（自包含校验无法覆盖的运行时门：白名单随 `phase_next` 动态下发）：通过则返回动作；越权 SHALL **一次即** `Terminal::failed`（文案携 `allowed` 列表；自环文案提示零 agent 步相位不支持重做）——动作裁决 SHALL 写 decision 节点 detail（见「驳回与动作裁决经节点 detail 可见」），MUST NOT 另立 gate 节点。

#### Scenario: 白名单条目携相位特性描述与可操作产物

- **WHEN** 决策 prompt 渲染可回溯白名单
- **THEN** 完整决策 prompt 的每条白名单行形如 `- <相位 id>（<相位特性描述>）：<可操作产物段>`（描述取本期相位表 `PhaseDefinition.description`、产物取 `PhaseDefinition.artifact_manifest`，随 workflow_type 分派——requirement / test-only 同名相位描述与产物不串味），产物段逐项渲染 `` `<相对路径>`（<产物标题>） ``、多项以「、」分隔，相位零声明产物时显式渲染「无声明产物」；评估 checklist MUST NOT 出现在产物段（PhaseRecord 投影、非相位声明产物），失败相位自身条目在描述后追加「（= 重做本相位）」

#### Scenario: 白名单内自治回跳

- **WHEN** test-execution#1 fail 且决策 agent 输出 `{action:"backtrack", backtrack_to:"test-gen", reason:...}`（test-gen 在白名单内）
- **THEN** walker 经写面 `backtrack` 落回跳记录（落库），`phase_next` 随后路由至 test-gen 重做，reason 随边标签可读；重做相位首节点 incoming 边为 `backtrack`

#### Scenario: 越权决议被兜底拦截

- **WHEN** 决策 agent 输出的 backtrack_to 不在 allowed_backtrack_phases 内（或写面 `backtrack` 被直接以越权目标调用）
- **THEN** walker 侧 `ensure_backtrack_allowed` 一次即判：decision 节点终态 failed、detail 携拒绝文案与 `allowed` 列表，run `Terminal::failed`（无第二轮纠正、无独立 gate 节点）；写面 `backtrack` 的二次校验对越权目标仍显式拒绝且状态库零变更（不落任何越权回跳账）

#### Scenario: ask 中断提问

- **WHEN** 决策 agent 经 `finish_step` 提交 `{action:"ask", question:..., options:[...]}`
- **THEN** run 进入中断等待态，UI 呈现问题与选项；用户应答后 run 按应答继续——应答开启一轮**新的**决策会话（`Continue` resume 全史，prompt 为 `decision_answer_prompt` 单点）

#### Scenario: verdict fail 分叉与重做自含

- **WHEN** code-review#1 落 verdict=fail（评估-only 相位，自身在白名单内）且决策 agent 输出 `{action:"backtrack", to:"code-review", reason:...}`
- **THEN** walker 唤起决策 agent 恰一次，回溯落账后同相位以新 attempt 真执行（重做本相位由白名单自含表达，`retry` 动作不存在）；重跑再落新 fail 条目时决策分叉对新条目照常触发。带 executor 相位（如 implement）verdict fail 同式分叉，重做携带失败清单段

#### Scenario: verdict fail 分叉与 retry 兑现

- **WHEN** 审查决策协议的兑现路径（本条为历史 scenario 名的存续面）
- **THEN** 原「`retry` 决议兑现」路径已退役（`DecisionAction::Retry` 删除、未知名由 `finish_step` 校验 Err 当场回灌），兑现语义改为「每个 fail 条目只触发一次决策」幂等闸 + 决策 backtrack 重入；重做本相位经回溯到自身兑现（见「verdict fail 分叉与重做自含」与「已决条目不重复分叉」）

#### Scenario: 已决条目不重复分叉

- **WHEN** 某相位最新 fail 条目已挂决策会话（decision_session_id 在案）后 `phase_next` 再被调用
- **THEN** 该条目不触发第二次决策分叉（幂等闸生效）；决策回溯到上游后重入产生的新 fail 条目（无决策槽位）照常分叉

#### Scenario: 决策 prompt 携回溯重做窗口段

- **WHEN** 失败相位落在某开放回溯窗口内且 walker 组装决策 prompt
- **THEN** prompt 含 `## 回溯重做窗口` 段，段内按创建序逐条渲染 `<backtrack reason>` / `</backtrack>` 包裹的原因原文；段索引小于 `## 失败相位报告` 索引；`input.backtrack_window` 的 `from_phase` / `to_phase` 字面量在 prompt 正文零命中

#### Scenario: 决策 prompt 的条目数与双分支一致且零二次裁剪

- **WHEN** 决策 prompt 用例分别以 2 条与 3 条窗口的 `DecisionInput.backtrack_window` 组装（生产路径上写面已按上限裁剪，3 条为直接构造的边界用例）
- **THEN** 2 条时两条原因全部渲染（≤ 上限的全量并集，按创建序升序）；3 条时同样逐条渲染输入集合、本侧不做任何裁剪（orchestration 只消费写面集合，`MAX_BACKTRACK_WINDOWS` 的裁剪不在此处发生）
- **AND** 生产路径不变式：写面下发的集合长度恒 ≤ `MAX_BACKTRACK_WINDOWS`，故 ≥3 层时决策 prompt 实际只见创建序末位两条（创建序最早那条、即该组 `from_index` 最大者已在写面被裁）

#### Scenario: 空窗口整段省略

- **WHEN** `DecisionInput.backtrack_window` 为空集合
- **THEN** 决策 prompt 零 `## 回溯重做窗口` 字样、零空标题残留；其余段序与内容逐字不变

#### Scenario: 窗口背景由写面透传而非本地推导

- **WHEN** 审查 `build_decision_input` 的取数面
- **THEN** 字段值原样取自写面 error 响应的 `backtrack_window`；函数体内零 store 读取、零相位表遍历推导窗口

### Requirement: 相位 prompt 组装面静态化（静态主体 + 动态 append）

executor / evaluator 会话 prompt SHALL 以写面下发的静态角色知识文本（desktop-phase-prompts / desktop-workflow-write-face）为主体；动态面 SHALL 全部 append 而非模板替换：上下文头一行 `change: <name>`、回溯重做窗口段（见「决策协议」与 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」）、static-check / archi-check / 测试修复失败注入、输出协议附录（evaluator）、decision 三动作段（决策会话）。prompt 组装 MUST NOT 使用 `<change>` / `<phase>` 模板替换。段序 SHALL 为「上下文头 → 静态主体 → 回溯重做窗口段 → 失败反馈段（门禁修复注入）→ 输出协议附录（evaluator）」：窗口段是失败反馈段的上位语境（前者答「为什么这个相位被叫回来」，后者答「本相位这次错在哪」），MUST NOT 倒置于反馈段之后；单行「回溯原因」形式已退役（窗口段完全取代，单点渲染）。窗口段的条目数与裁剪完全由写面下发的集合决定（写面常量 `MAX_BACKTRACK_WINDOWS`：≤ 该上限时全量并集、超限取创建序末位两层；双分支完整表述见「决策协议」与 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」），orchestration 侧 MUST NOT 二次裁剪、亦不定义该常量。**verdict 裁决内化后动态面 SHALL NOT 含任何「verdict 解析失败修复注入」**——纠错由 `finish_step` 工具在同一 turn 内闭合，不产生跨轮反馈注入。`WorkerTurnRequest.provenance.source_ref = <change>/<phase>/<role>/<attempt>` 与 `root` / `model_level` 结构化通道不变——phase / attempt 盖戳不依赖 prompt 文本。

#### Scenario: 上下文头在场

- **WHEN** walker 发起 executor / evaluator 会话
- **THEN** prompt 含一行 `change: <name>`（change 名自写面记录 name 供给）
- **AND** prompt 主体为静态角色知识文本，无 `<change>` / `<phase>` 模板替换痕迹

#### Scenario: 反馈注入仍 append

- **WHEN** static-check / archi-check 失败走定向反馈边
- **THEN** 修复反馈文本以 append 注入同一 executor 会话 prompt（不重写静态主体）
- **AND** 回溯场景改为 append 回溯重做窗口段（单行回溯原因行零残留）

#### Scenario: 段序窗口段先于反馈段

- **WHEN** walker 为一个既落在回溯窗口内、又带 fail 重试反馈的相位组装 evaluator prompt
- **THEN** 序为「上下文头 → 静态主体 → 回溯重做窗口段 → 失败反馈段 → 输出协议附录」；窗口段索引小于失败反馈段索引，输出协议附录仍在最后

### Requirement: evaluator 输出协议瘦身

`evaluator_protocol` 输出协议附录 SHALL 要求 evaluator **调用 `finish_step` 工具收口**（形状与落账红线由工具参数给出；MUST NOT 要求在最终消息输出任何 JSON 对象、MUST NOT 说明 `verdict` / `phase` / `attempt` / `skipped` 字段——verdict 由 checklist 推导、其余由 walker 按 provenance 盖戳）。协议 SHALL 保留禁调 MCP 写通道红线（phase_log / phase_next / phase_start / backtrack，落账由桌面编排代写）；checklist 至少一项、逐项含 `id`（取自 prompt「静态清单」表首列）+ `pass`（布尔）+ `evidence`（事实依据，含 `file:line`）；`report` ≤ `MAX_REPORT_CHARS`。

#### Scenario: 协议改为工具收口

- **WHEN** 读取 evaluator 输出协议附录文本
- **THEN** 要求「调用 `finish_step` 收口」
- **AND** 无「输出一个 checklist JSON 对象」「形状为 `{ verdict, report, checklist }`」类要求，无「输出 phase / attempt / skipped 字段」要求
- **AND** 仍含禁调 MCP 写通道红线与 `report` 上限

#### Scenario: 退役修复注入零命中

- **WHEN** 审查 `prompt.rs` 的动态面组装
- **THEN** 无「verdict 解析失败」相关文案与注入分支（`verdict_fix_prompt` / `decision_fix_prompt` 已退役，`grep` 零命中）

### Requirement: 确定性相位空 checklist 落账

archive 相位（无 evaluator、无 checklist 的确定性相位）SHALL 在**相位收口动作** `settle_phase` 以空 checklist 落账：`settle_phase` 在 test_outcome 分支之后、evaluator 分支之前插入 archive 分支——构造 `PhaseLogInput { phase: "archive", report: String::new(), checklist: vec![], skipped: false, executor/evaluator/decision 三槽位 None }` 并直接 `run_phase_log`。落账 SHALL 复用既有 `derive_verdict([])=Pass` 口径（空 checklist = 无条件通过），零 `if phase == archive` 特判、零新增落账函数。三分流按「产物在否」分离：archive 相位无 TestExecution 步（test_outcome 恒 None）、无 Evaluator 步（evaluator 产物恒 None），仅命中空 checklist 分支。

#### Scenario: 空 checklist 落账 pass

- **WHEN** archive 相位 9 步走完进入相位收口
- **THEN** `run_phase_log` 以空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord，`active_phase` 清位（phase_log 单事务），`phase_next` 随后路由 done

#### Scenario: 三分流互不误判

- **WHEN** 分别驱动评估相位（evaluator 产物）、test-execution 相位（test_outcome 产物）与 archive 相位（空 checklist）
- **THEN** `settle_phase` 三分支按产物在否分离，无按 phase 名判断、无跨分支误判

### Requirement: 计划图与执行图对齐与步粒度回退

计划图（源码，desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）与执行图（db，desktop-change-state-store「run 执行图落库」）SHALL 经 **step kind + phase + attempt + instance_no** 对齐：执行图节点是计划 step 的运行时实例，节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`；执行图边 SHALL 显式落 `RunEdgeRecord`，`edge_kind ∈ { forward, feedback, retry, backtrack }`——archive 相位九步串行为 forward 边，无回边；`retry` 为存量解码专用（新写面 MUST NOT 产出，查询面投影前屏蔽），决策回溯到自身的重做入口边为 `backtrack`。计划图 MUST NOT 落库（定义恒为编译期静态）；执行图 MUST NOT 上位为路由权威。回退（backtrack）的语义单位 SHALL 保持 phase 粒度；本变更 MUST NOT 把回退载荷改为 step 粒度。**开相与相位收口 SHALL NOT 出现在任一侧**：`enter_phase` / `settle_phase` 是相位机的固定动作（无计划步、无节点、无审计行），故辅助步退场后计划图与执行图的步集合仍逐一对齐；`phase_start` / `phase_log` 两词仅存量节点行可达。

#### Scenario: 节点身份稳定且 feedback 独立

- **WHEN** 同 phase 同 attempt 内 static-check 失败触发 executor 修复重跑，随后决策回溯到该相位自身（重做）
- **THEN** executor 首次与修复各为 `instance_no` 递增的独立节点，feedback 边指向前一节点；重做产生新 attempt，其相位入口边为 `backtrack`（新写面 MUST NOT 产出 `retry` 边）

#### Scenario: 归档步为 forward 边

- **WHEN** archive 相位一次 run 收口后查询执行图
- **THEN** 归档 9 步节点以 forward 边串行落 `RunEdgeRecord`，零 feedback / retry / backtrack 边；`run_id` / `from_node_key` 两二级索引可达

#### Scenario: 边四值与查询

- **WHEN** 审查一次含前进 / 反馈 / 回跳的 run 执行图
- **THEN** forward / feedback / backtrack 三 kind 齐备且 `run_id` / `from_node_key` 两二级索引可达（`retry` 仅存量解码面可达、投影面屏蔽）；`backtrack` 边指向白名单内目标相位首节点（含指向失败相位自身相位的重做入口）

### Requirement: MutationTest 步接线与失败升格

walker SHALL 在 test-execution 相位按 `TEST_EXECUTION_STEPS` 步序列（`TestExecution → MutationTest`，定义见 desktop-workflow-write-face）顺序执行：TestExecution 结论 pass 后才执行 MutationTest 门禁步（`ToolCommand::MutationTest` 经 `MutationRunner` port 由 checks 边界 infra 实现）；TestExecution fail / error 首败即升格、截断余下步回 `phase_next` 进决策分叉——「测试 / 覆盖率修绿后再执行突变」由「升格截断 + 决策回溯后真重跑」天然保证，walker MUST NOT 在 TestExecution 未 pass 时发起突变 spawn。两步 `on_fail` SHALL 均为 `OnFail::Escalate`：失败即零会话代写 fail checklist 经写面 `phase_log` 落账（report 携完整明细、evidence 携摘要 + 报告目录），随后回 `phase_next` 进决策分叉；相位内 MUST NOT 存在反馈边（`feedback_limit` 的 test-execution / mutation-test 计数面删除）。反馈边注入对象 SHALL 仅剩 static-check / archi-check 失败注入 executor。`settle_phase` 的 test 分支 SHALL 消费 `test_outcome` + `mutation_outcome` 双槽（机械 checklist 双槽，见 desktop-test-execution）；archive 分支双槽恒 None（三分流按「产物在否」分离语义不变）。

#### Scenario: 拆步时序绿后才突变

- **WHEN** TestExecution 首次 fail（升格交决策）后经决策回溯修复重入并通过
- **THEN** MutationTest 仅在 TestExecution pass 后执行；决策等待与修复期零突变进程 spawn，步序列零乱序；重入为独立 attempt

#### Scenario: 突变失败零会话升格

- **WHEN** MutationTest 首次以 `conclusion=Fail`（未达标）或 `conclusion=Error`（段中断）收场
- **THEN** 零 WorkerAgent 会话请求、一条 fail PhaseRecord（「突变测试」一项 `pass=false`，executor / evaluator / decision 三槽位 None）、回 `phase_next` 进决策分叉；MUST NOT 注入修复会话

#### Scenario: 双槽分支分离不误判

- **WHEN** 分别驱动 test-execution 相位（test_outcome + mutation_outcome 双槽产物）与 archive 相位（双槽恒 None）
- **THEN** `settle_phase` test 分支消费双槽机械 checklist，archive 分支仅命中空 checklist 分支，互不误判

### Requirement: initialize 基线检查相位执行体

Walker SHALL 以两表表头一次性 `initialize` 相位（`INITIALIZE_STEPS`，见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）承接变更流程启动前的基线绿门：步序 `Clean → StaticCheck → TestExecution`（开相与相位收口不在 `steps` 内，由 `enter_phase` / `settle_phase` 承载），全部检查与恢复 SHALL 在 exec root（worktree）内执行（主仓未提交改动不进 worktree，基线语义纯净）。initialize 相位 SHALL 为确定性相位（零 executor / evaluator / decision 会话，全程零 WorkerAgent）：

- **Clean 步**（vcs 直调先例同归档 git 步，不经 ToolCommand 枚举）：walker 经 `ArchiveVcsPort::restore_baseline(worktree, 排除面)` 恢复 worktree 至基线态——tracked 修改 reset、untracked 清除，排除面 = foundation 共享的依赖与工具产物树名单（node_modules / dist / build / target / coverage / .nyc_output + dot 目录，见 desktop-checks-domain「依赖与工具产物树名单共享单源」）+ `openspec/` 工件树（proposal 等过程文档与报告树为未跟踪面，MUST NOT 被 clean 删除）。Clean 步 MUST 仅在 worktree 在场时执行；失败即 `Terminal`（`on_fail=None`）。
- **StaticCheck / TestExecution 步**：复用既有检查域 runner（`StaticCheckRunner` / `TestExecutionRunner`，报告槽位为 baseline 变体，见 desktop-test-execution「baseline 报告槽位与树隔离」），MUST NOT 执行 mutation（基线检查明确不含突变）。两步声明 `on_fail=Terminal`：失败 SHALL 以 `Terminal` 收敛 run failed，MUST NOT 走反馈边（无 agent 会话可注入，重试基线无意义）。失败文案 SHALL 区分：test-execution `conclusion=Error`（环境 / 配置问题——引导查配置与环境后重新发起）与 `conclusion=Fail`（测试红——引导在基线上修复测试后重新发起）；static-check 失败引导在基线上修复后重新发起；文案 SHALL 携 baseline 报告目录路径供回查。

相位收口 SHALL 由 `settle_phase` 以机械 checklist 纯桌面代写（零 agent，沿 `test_execution_phase_log_input` 先例——steps 含 TestExecution 步则 `test_outcome` 自动挂载、三分支天然分派），report 指向 baseline 报告目录，三会话槽位恒 `None`。

一次性语义 SHALL 由既有落账机制天然成立：任一步 `Terminal` 失败时 PhaseRecord 零 initialize 条目（图节点照常落 `RunNodeRecord` 留痕可回查）→ 再发起时 `phase_next` 仍路由 initialize、Clean 步清残留后干净重试；全绿 pass 落 PhaseRecord（机械 checklist，verdict=pass）→ 持久跳过（后续 run 不再重跑基线检查）。initialize 收口后的 phase 间停等 SHALL 与全相位一致（`auto_next_phase=false` 时照常 `ConfirmWait` 停等，零特判豁免）。legacy 记录（`ChangeRecord.worktree=None`）SHALL 整相位短路：步序列零执行、零 spawn、主仓 MUST NOT 被 reset 或 clean，以 `enter_phase` → `settle_phase(skipped=true)` 收口（跳过因注明 legacy 主仓执行无 worktree 基线，一次性持久同 pass 语义）。

#### Scenario: 全绿序曲与一次性跳过

- **WHEN** 新建 requirement change 发起 run（基线 static-check 与 test-execution 全绿，auto_next_phase=true）
- **THEN** initialize 相位先于 proposal 执行：Clean / StaticCheck / TestExecution 三节点按序上图（零 WorkerAgent 会话），报告落 `reports/baseline`；相位收口机械 checklist 落账 pass 后后续相位照常推进；同 change 再次发起 run 时 initialize 被持久跳过（零重跑）

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
- **THEN** initialize 步序列零执行（零 vcs / 检查 spawn），主仓零 reset 零 clean，相位经 `enter_phase` + `settle_phase(skipped=true)` 落账（跳过因可见），后续相位照常路由

#### Scenario: 收口停等与全相位一致

- **WHEN** 手动档（auto_next_phase=false）run 的 initialize 相位收口落账 pass
- **THEN** run 照常进入 phase 间停等（`ConfirmWait`），用户确认后续走 proposal；无「发起即确认」特判豁免

### Requirement: ArchiCheck 步接线与反馈边

Walker SHALL 在 implement 相位执行架构漂移门禁步（`StepNode::ArchiCheck` / `ToolCommand::ArchiCheck`，相位序列见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）。该步 SHALL 经 `ArchiCheckRunner` port（定义落 `crates/core/orchestration::port`，实现落 `crates/infra/archi`，见 desktop-archi-gate「ArchiCheck runner port 与报告落盘」）取结论，产出走既有 `ToolStepOutput` 封闭集变体 `ArchiCheck(ArchiCheckOutcome)`；`workflow` 侧 MUST NOT 出现任何 `archi` 依赖或 `archi_check` 字面判据（`workflow` ⇄ `archi` 互不依赖，本步的语义只由相位表与 `on_fail` 声明承载）。

分派语义 SHALL 与既有门禁步同型：

- **fault 先于结论**：`ArchiCheckOutcome.fault` 在位时早退——`Environment` → 步节点 `failed` + `Terminal::failed`，`Stopped` → 步节点 `stopped` + `Terminal::stopped`；两臂均 MUST NOT 发起 WorkerAgent 会话、MUST NOT 消耗反馈边计数。架构检查零进程 spawn（见 desktop-archi-gate），故 `fault` 在生产路径上恒 `None`；字段与分派臂保留是为了让 walker 的第四个 runner 与既有三步同形，MUST NOT 为它写特例分支。
- **通过**：`passed = true` → 步节点 `passed`（detail 携检查模式与状态词）→ 继续 evaluator 步。
- **不通过**：`passed = false` → 步节点 `failed`（detail 为诊断摘要，经既有 `diagnose_brief` 口径截断）→ 独立计数 `feedback`（上限取步声明的 `FeedbackLoop { limit: 3 }`，与 static-check 的 5 次互不占用）→ 未超限时把违例诊断注入**同一 executor 会话**（`continue_session`，`WorkerRole::Executor`，`EdgeKind::Feedback`，修复节点为 `instance_no` 递增的新节点）后重跑本步；已超限时 SHALL 升格相位 fail：经相位收口 `settle_phase(PhaseSettle::Fail(..))` 代写 fail checklist（不跑 evaluator、仅携 executor 槽位），置 `cursor.failed` 并回 `phase_next` 进决策分叉。
- **反馈文案有界**：注入 executor 的文本 SHALL 由 runner 侧装配（违例按 `(from, to)` 聚合、每条附档位与出现次数与至多两处 `file:line`，结构性错误单列），整体 MUST 不超过 `MAX_REPORT_CHARS` 量级——MUST NOT 逐行铺开全部出现点（同一元素对在 40 个文件各命中一次时注入 40 行流水账会挤掉执行指令）。

步节点 SHALL 以 `ChangeStepKind::ArchiCheck` 上图与落库（`RunStepKind::ArchiCheck` 尾部追加，见 desktop-change-state-store「run 执行图落库」），报告路径随步 detail 呈现，MUST NOT 走节点 `output_ref`（该字段是 exec-outputs 回查通道，架构报告是 JSON 文件、非 exec 输出）。

**本变更为本 requirement 带来的改动点（仅一处，逐条自足）**：辅助步退场 SHALL NOT 触及本步的存在性、座位、runner、报告槽位与反馈上限——① `ArchiCheck` 步 SHALL NOT 在退场范围内（implement 相位仍为 `Executor → StaticCheck → ArchiCheck → Evaluator` 四步，本步仍在 static-check 之后、evaluator 之前）；② 反馈边语义（`FeedbackLoop { limit: 3 }`、注入同一 executor 会话、独立计数）逐字不变；③ **唯一改动**是「超限升格」的落账入口：原经写面 `phase_log` 直调的落账改由相位收口动作 `settle_phase(PhaseSettle::Fail(PhaseFail { item: "架构检查", .. }))` 承载（相位内的落账面合一，见「相位入口与收口内化（计划图仅含真实步）」），`PhaseFail` 的 `report` / `evidence` / `executor_session` 装配逐字不变。ArchiCheck 与 static-check 同类（检测型门禁、反馈边计数独立）：本变更 MUST NOT 合并二者、MUST NOT 改动其 runner / 报告槽位 / 反馈上限。

#### Scenario: 违例注入同一 executor 会话

- **WHEN** implement 相位 executor 收口后 ArchiCheck 步报一条 `undeclaredDependency`（`App.Billing` → `App.Auth`，引入方 `strict`）
- **THEN** 步节点 `failed`（detail 含该违例摘要），反馈边计数 +1，`WorkerRole::Executor` 以 `continue_session` = 首次 executor 会话 id 发起一轮修复会话，新修复节点 `instance_no` 递增并以 `feedback` 边连回；修复后重跑本步

#### Scenario: 反馈边超限升格相位 fail

- **WHEN** 同一 attempt 内 ArchiCheck 连续 3 次不通过（修复后仍违例）
- **THEN** walker 不再发起第 4 次修复会话，经相位收口代写 fail checklist（item「架构检查」、report 携汇总诊断、`skipped=false`、仅 executor 槽位），相位按 fail 收场回 `phase_next` 进决策分叉；evaluator 步不执行

#### Scenario: 通过后进入 evaluator

- **WHEN** ArchiCheck 步以 `passed = true` 收场（`errors` 与 `structuralErrors` 皆空）
- **THEN** 步节点 `passed`、零反馈会话，walker 顺序进入 evaluator 步；静态检查步的反馈计数不受影响

#### Scenario: 零进程零会话的失败面

- **WHEN** 审查 `step_archi_check` 与 `ArchiCheckRunner` 实现的进程触点与会话触点
- **THEN** 除反馈边注入的 executor 续注外零 WorkerAgent 会话、零进程 spawn；`fault` 恒 `None`，`settle_fault` 分支仅作为同形保留（不产生任何新增环境文案常量）

### Requirement: 步级时间戳与 token 消耗随行

步状态行 `ChangeStepState` SHALL 增四字段并全链随行：`started_at: i64`（UTC unix 毫秒，域内 `now_millis()` 口径）、`ended_at: Option<i64>`（running 恒 None）、`token_input: Option<u64>` / `token_output: Option<u64>`（本步 turn 增量，仅 WorkerAgent 步终态有值，缺席合法缺省）。`StepSettle` SHALL 同步扩四字段（emit 载荷收窄面）。

**时间铸造 SHALL 收 `emit_node` 单点**（复用既有 `now_millis()`，MUST NOT 新增时钟注入缝——收口时刻铸造只在生产路径消费）：

- running emit：`started_at = now`、`ended_at = None`、token 两值 None；
- 终态 emit（passed / failed / stopped）：`ended_at = now`，随行 `started_at = now`（该值是否被采信由合并侧裁定）。

**合并语义（`accumulate_node`）SHALL 为**：`started_at` 无条件保首见（running emit 落值后不被终态 emit 顶替，终态行携带的同刻值被丢弃）；`ended_at` / `token_input` / `token_output` 与 `session_id` 同款「some 才覆写」。节点累积器（`RunEntry.nodes`）、活面快照（`ChangeRunSnapshot` / `ActiveRunView`）与收口整包（`finish_command` → `RunNodeEntry`）SHALL 逐字段同值透传，零逻辑分支。辅助步退场后生产路径**已无双 emit 之外的瞬步形态**（`enter_phase` / `settle_phase` 零 emit），故「单 emit 首见即真值」这一半语义 SHALL 仅由直构用例承载，MUST NOT 因无生产例而删除该合并分支。

**token 求和 SHALL 在 `run_worker` 终态 emit 前单次落值**：对 `outcome.transcript` 的 `AgentEventKind::TurnDone.usage` 求 in / out 两键之和（`worker.run(turn).await` 收口后 transcript 已含 TurnDone——终态单次落值即会话结束后累计结果，MUST NOT 建二次刷新通道）；usage 缺席（引擎未返回）= None；工具步 / Git 步恒 None。token 求和函数 SHALL 唯一实现于 core/agent（`AgentEventKind::TurnDone` 定义所在 crate，orchestration 已依赖；agent crate 禁 workspace 内依赖而函数仅吃 `&[AgentEvent]` 天然合规），infra/store 的会话统计 SHALL 引同一实现（依赖方向 infra → core 不可反向，MUST NOT 保留双实现）。`RunUpdate::Step` 载荷随 `ChangeStepState` 自动扩展，IPC 通知 kind-only 降位形态（载荷不出进程、客户端收通知重查统一视图）MUST NOT 改变。

#### Scenario: 双 emit 起止落值与首见保留

- **WHEN** 某 WorkerAgent 步经 running emit（时刻 T0）后于时刻 T1 以 passed 收口（终态 emit）
- **THEN** 合并后节点 `started_at == T0`（终态行携带的 T1 起始值被丢弃）、`ended_at == T1`；活面快照与收口整包同值

#### Scenario: 单 emit 首见即真值

- **WHEN** 以直构 `ChangeStepState` 注入单条终态 emit（时刻 T，无 running 先行拍）
- **THEN** 合并后节点 `started_at == ended_at == T`（耗时 0ms），无特判分支；该分支语义不因辅助步退场而无生产例而删除

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

## REMOVED Requirements

### Requirement: 决策会话软拒环与漂移上限

**Reason**: 该 requirement 描述的三件机制均已失去载体：① 输出解析软拒环——决策输出改 `finish_step` 工具协议后，形状 / 动作封闭集 / 缺字段 / reason 超限全部由工具体在同一 turn 内 `is_error` 回灌，`parse_decision` 已退役（`grep` 零命中）；② 漂移计数上限 `DECISION_DRIFT_LIMIT` 与 `decision_fix_prompt`——随软拒环退役而整体删除；③ 白名单门每轮一 `WhitelistGate` 节点——本变更删除该节点（白名单门降为一次纯函数判定 `ensure_backtrack_allowed`，越权一次即 `Terminal`），故「每轮一节点、驳回后仍有纠正轮」的形态在新写面上不可达。

**Migration**: ① 协议层纠错由 `finish_step` 工具 `Err` 当场回灌（同一 turn 内，零跨轮计数）；② 白名单越权改为**一次即** `Terminal::failed`，拒绝文案仍携 `allowed` 列表，留痕改由 decision 节点 `detail` 承载（见 ADDED「驳回与动作裁决经节点 detail 可见」）；③ `ask` 停等在决策会话之外闭合（应答后新开一轮决策会话，`instance_no` 递增于 decision 节点，轮数可由节点实例数直读）；④ `WhitelistGate` / `RetryGate` 两词与 `StepGroup::Gate` 分支保留为**存量解码词汇**（旧库行渲染必需），新写面零产出——`grep` 判据改为「`begin_node(.., ChangeStepKind::WhitelistGate ..)` / `..RetryGate ..` 零调用点」，而非「变体零存在」。

## ADDED Requirements

### Requirement: 相位入口与收口内化（计划图仅含真实步）

相位机 SHALL 以两个固定工程动作承载开相与落账，两者 MUST NOT 是计划步、MUST NOT emit 图节点、MUST NOT 产 `StepRecord` 审计行：

- **`enter_phase(tools, request, phase) -> Result<u32, Terminal>`**：相位入口——经写面既有 `phase_start` 能力单事务写 `active_phase`（phase / attempt / start_at），零 emit、零审计、零计划步。`attempt` 推导口径见「attempt 推导口径与 round 口径分离」。写失败 SHALL 收敛 `Terminal::failed`（store 坏 / 编程错误，重启由启动标定自愈），MUST NOT 虚构 attempt、MUST NOT 继续走步循环。
- **`settle_phase(tools, request, cursor, phase, settle: PhaseSettle) -> Result<u32, Terminal>`**：相位收口——单入口三形态：

  ```
  enum PhaseSettle<'a> {
      Pass,                     // 正常收口：三分流取自 PhaseCursor（机械双槽 / evaluator checklist / 空 checklist）
      Fail(PhaseFail<'a>),      // 门禁升格与归档步失败：代写 fail checklist（item / report / evidence / executor_session）
      Skipped(&'a str),         // legacy initialize 整相位短路：skipped=true + 跳过因
  }
  ```

  三形态共用同一落账单点（`run_phase_log` + `active_phase` 清位，单事务），零 emit、零审计。落账失败 SHALL 收敛 `Terminal::failed`（见「相位收口失败可辨与相位内系统异常仍标 failed」）。

`drive` 的单相位序列 SHALL 为 **`enter_phase` → 迭代 `next.steps`（`run_step`）→ `settle_phase(PhaseSettle::Pass)`**；`StepFlow::Upgraded`（门禁升格）SHALL 截断余下步并跳过 pass 收口（升格路径已由门禁执行体经 `PhaseSettle::Fail` 落账），直接回 `phase_next` 进决策分叉。`next.steps.is_empty()` SHALL 仍按计划图漂移 `Terminal::failed`（相位定义恒含至少一个真实步）。

#### Scenario: 开相不出节点不出审计

- **WHEN** 驱动任一相位 run 并审查该 attempt 的图节点与审计行
- **THEN** 节点序列以该相位首个真实步起手（如 proposal 的 executor、initialize 的 clean、archive 的 preflight），无 `phase_start` 节点、无 `StepKind::PhaseStart` 审计行；`active_phase` 的 attempt / start_at 在相位运行期间可读

#### Scenario: 收口时序与升格跳过

- **WHEN** 分别驱动（a）全步 pass 的相位与（b）门禁步升格的相位
- **THEN**（a）走完 `steps` 后调用 `settle_phase(PhaseSettle::Pass)` 落账一次并清位；（b）在升格点截断余下步，门禁执行体自行调 `settle_phase(PhaseSettle::Fail(..))` 落 fail 账，`drive` 不再调用 `Pass` 收口，直接回 `phase_next`

#### Scenario: 不可达步词显式失败

- **WHEN** 计划图漂移使 `next.steps` 含 `PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` / `Decision` 任一词
- **THEN** `run_step` 的不可达臂返回 `Terminal::failed`（文案点明该步词不出现在计划步序列内），MUST NOT 静默 `Proceed`（原空臂形态会把计划图漂移吞成一次空操作）；`StepNode` 仍 22 变体、`match` 保持穷尽

#### Scenario: 空步序列仍显式失败

- **WHEN** 写面下发的 `next.steps` 为空
- **THEN** `Terminal::failed`（文案点名相位缺计划步序列），MUST NOT 空转回 `phase_next`（活锁防护保持）

#### Scenario: 开相写失败即终态

- **WHEN** `enter_phase` 的 `phase_start` 返回 `Err`
- **THEN** run 以 `Terminal::failed` 收口，MUST NOT 进 `steps` 循环、MUST NOT 落任何 PhaseRecord；重启后由启动标定处置残留 `active_phase`

### Requirement: 相位收口失败可辨与相位内系统异常仍标 failed

run 的失败面 SHALL 分两型且文案可辨，MUST NOT 混同：

- **相位内步骤系统异常**（`run_worker` 返回 `Err`、会话 `status = Failed` / `Stopped`、门禁 runner `Err`、vcs 步失败）：**该步节点 SHALL 呈 failed（或 stopped）**，run 随之 `failed`（或 `stopped`）。红节点是用户定位失败步的唯一入口，MUST NOT 为统一形态而把节点染成 passed。
- **相位收口落账失败**（相位内全部步皆 passed，`settle_phase` 的 `phase_log` 返回 `Err`）：**节点 SHALL 保持全绿**（它们确实都通过了），run SHALL `failed`，终态 reason SHALL 以固定前缀「相位 `<phase>` 全部步通过但落账失败: `<err>`」点明成因，MUST NOT 呈现为步失败。

两型的 reason 文案 MUST 可区分（前者含步骤 / 会话语境，后者含「全部步通过但落账失败」前缀）。该降级（全绿 + failed）为「落账不再是可观测步」的显式代价，已接受；缓解手段是原因文案可归因。

#### Scenario: 落账失败节点全绿

- **WHEN** 某相位全部计划步 passed 后 `log_phase` 返回 `Err`（store 故障注入）
- **THEN** 该相位全部节点终态为 passed；run 终态 `failed`，reason 前缀「相位 `<phase>` 全部步通过但落账失败」并携 `Err` 文案；状态库无该 attempt 的 PhaseRecord

#### Scenario: 步骤异常仍是红节点

- **WHEN** 相位内 executor 会话以 `status = Failed` 收场（API 错 / 模型错）
- **THEN** 该 executor 节点呈 failed 并携失败诊断摘要，run `failed`；MUST NOT 出现「节点全绿但 run 失败」的形态，MUST NOT 调用 `settle_phase`

#### Scenario: 两型文案对拍

- **WHEN** 分别触发（a）步骤系统异常与（b）相位收口落账失败
- **THEN** 两终态 reason 不含同一句引导语，（b）必含「全部步通过但落账失败」前缀，（a）不含该前缀

### Requirement: 驳回与动作裁决经节点 detail 可见

决策分叉的动作裁决 SHALL 由图面节点与 `detail` 承载，MUST NOT 以独立 gate 节点实例计数表达：一次决策分叉 SHALL 产 decision 节点（`WorkerRole::Decision` 的会话步），`instance_no` 按决策轮次递增；节点终态与 `detail` SHALL 为：

- **通过**（`ensure_backtrack_allowed` 放行）→ 节点 passed，`detail` 携动作摘要（`backtrack → <目标相位>` / `stop` / `ask`）；
- **越权**（白名单门拒绝 / `backtrack` 目标不在 `allowed`）→ 节点 failed，`detail` 携拒绝文案与 `allowed` 列表，run `Terminal::failed`（一次即停，无第二轮纠正）。

`WhitelistGate` / `RetryGate` 节点 SHALL 零产出（`begin_node` 零调用点；两词保留为存量解码词汇）。`ensure_backtrack_allowed` SHALL 保持纯函数（入参动作 + 失败相位 + 白名单），MUST NOT 读运行时状态。`ToolCommand::DecisionLog` 保留（会话槽位挂账为业务数据，非图节点）。

#### Scenario: 越权一次即停且可归因

- **WHEN** 决策 agent 提交 `{action:"backtrack", to:"archive"}` 而 archive 不在 `allowed`
- **THEN** 本次决策的 decision 节点呈 failed、`detail` 含拒绝文案与 `allowed` 相位清单；run 以 `Terminal::failed` 收口（文案携 `allowed`）；图上零 `whitelist_gate` 节点

#### Scenario: 通过裁决写 detail

- **WHEN** 决策 agent 提交白名单内 `{action:"backtrack", to:"test-gen", reason:...}`
- **THEN** decision 节点 passed、`detail` 为 `backtrack → test-gen`；写面 `backtrack` 落账后目标相位首节点 incoming 边为 `backtrack`；图上零 `retry_gate` 节点

#### Scenario: ask 应答后新轮实例递增

- **WHEN** 决策 agent 首轮提交 `ask`，用户应答后新一轮提交白名单内 `backtrack`
- **THEN** 图上两个 decision 节点实例（`instance_no` 1 → 2）；首轮 `detail` 携 `ask` 与问题摘要、终态 passed（用户驱动的停等不计失败）；第二轮 `detail` 携回溯目标

### Requirement: attempt 推导口径与 round 口径分离

相位 attempt 的推导 SHALL 为**全量含 stale** 口径：

```
attempt = store 内该 (change_id, phase) 的 PhaseRecord 全量条数（含 stale）+ 1
```

该式子 SHALL 由写面单点承载（`start_change_phase` 与 `log_change_phase` 同源，零新语义），walker MUST NOT 自推导、MUST NOT 取 `detail.pipeline[phase].attempts.len() + 1`——`queries/detail.rs` 装配前会过滤 stale 条目，而 backtrack 把下游 pass 条目标 stale 但保留行，用 pipeline 条数推导会让回溯导致 attempt 回退，节点身份键 `(phase, attempt, step, instance_no)` 与历史行撞号。

`phase_next` 的 `round`（回溯窗口口径，`entries.len() - anchor + 1`）与 attempt 是**两个不同口径**，MUST NOT 合并、MUST NOT 互相代入。`active_phase.attempt` 字段 SHALL 保留写入（落账即清位，事后无法推导该相位刚结束的是第几次 attempt；`queries/detail.rs` / `list.rs` 直接读该字段）——`enter_phase` 沿用 `start_phase` 既有写面（含 attempt 推导），`infra/store` 零改动。

#### Scenario: 回溯不使 attempt 回退

- **WHEN** implement#2 pass 后被决策 backtrack 标 stale（行保留），随后重做 implement
- **THEN** 新 attempt = 该相位 PhaseRecord 全量条数（含被标 stale 的 #2）+ 1 = 3；节点身份键 `(implement, 3, …)` 与历史 #1 / #2 不撞号；图面不出现同键节点覆盖

#### Scenario: 跨 run 不撞号

- **WHEN** 同一 change 先后两次 run 均走 implement 相位
- **THEN** 第二次 run 的 attempt 接着第一次的条数递增（写面单点推导跨 run 生效），MUST NOT 从 1 重来

#### Scenario: round 与 attempt 各算各的

- **WHEN** 审查 `phase_next` 的 `round` 计算与 `enter_phase` 的 `attempt` 取值
- **THEN** `round` 由回溯窗口 entries 与 anchor 派生（窗口口径），`attempt` 由 PhaseRecord 全量条数派生（历史口径）；两处零共享变量、零互相代入

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs`（`enter_phase`，新） | 相位入口固定动作 | 经 `ToolCommand::PhaseStart` 写 active_phase + attempt（全量含 stale + 1）；零 `begin_node` / 零 `emit_node` / 零审计；`Err → Terminal::failed` |
| `crates/core/orchestration/src/walker.rs`（`settle_phase` / `PhaseSettle` / `PhaseFail`，新） | 相位收口固定动作 | 单入口三形态 `PhaseSettle::{Pass, Fail(PhaseFail), Skipped(&str)}`；`Pass` 的三分流（`test_outcome` + 双槽 / `evaluator_turn` checklist / 空 checklist）原样搬入；`run_phase_log` 后清位；落账 `Err → Terminal::failed`「相位 X 全部步通过但落账失败」；零 emit |
| `crates/core/orchestration/src/walker.rs`（`EvaluatorTurn`，新） | evaluator 收口产物 | `{ outcome: WorkerTurnOutcome, checklist: EvaluatorChecklist }`——会话结果与载荷同源，消「有 outcome 无 checklist」非法态；`PhaseCursor` 的 `eval_outcome` + `verdict` 两槽合一为 `evaluator_turn: Option<EvaluatorTurn>` |
| `crates/core/orchestration/src/walker.rs`（`run_worker` + `SettleMode`，扩） | 终态 emit 时机 | 新增 `pub enum SettleMode { Immediate, Deferred }` 形参：`Deferred` 只发 `running` 拍、返回时不给终态（终态由调用方 emit）；evaluator / decision 两调用点传 `Deferred`，其余（executor / spec_sync / resolve_conflicts）传 `Immediate`，语义与既有逐字一致；token 求和与 `fault` 分派不变 |
| `crates/core/orchestration/src/walker.rs`（`step_evaluator`） | 裁决内化 | 读 `outcome.finalized`；`Some(Evaluator(..))` → 同实例 passed + `verdict=…` detail + 写 `cursor.evaluator_turn`；缺席 → 同实例 failed + `Terminal::failed`（截断 / 未收口两文案）；零反馈边、零独立门步 |
| `crates/core/orchestration/src/walker.rs`（`drive`） | 单相位序列 | `enter_phase` → 迭代 `steps` → `settle_phase`；`Upgraded` 截断余下步且跳过 `settle_phase`；legacy initialize 短路改走 `enter_phase` + `settle_phase(skipped)`；`steps.is_empty()` 仍 `Terminal::failed` |
| `crates/core/orchestration/src/walker.rs`（`run_step`） | 分派臂收窄 | `StepNode` 冻结 22 变体 ⇒ `match` 仍须穷尽：`PhaseStart` / `VerdictGate` / `PhaseLog` 三臂与 `Decision \| RetryGate \| WhitelistGate => Proceed` 空臂合并为**单一不可达臂**，对六变体返回 `Terminal::failed`（计划图漂移文案：该六词 MUST NOT 出现在任何 `steps` 内）；其余臂逐字不动（含 `ArchiCheck`） |
| `crates/core/orchestration/src/walker.rs`（`open_step` / `close_step`，新） | 归档步节点辅助合一 | 由 `begin_archive_node` / `settle_archive_node` 合流（二者只差「是否先发 running」）；调用点语义零变 |
| `crates/core/orchestration/src/walker.rs`（`decision_session`） | 动作裁决写 detail | 删 `WhitelistGate` 节点（`begin_node` / 三次 `emit_node`）；通过与越权写 decision 节点 `detail`；`ensure_backtrack_allowed` 调用面不变（越权一次即 `Terminal`）；ask 应答后新轮 `instance_no` 递增 |
| `crates/core/orchestration/src/walker.rs`（`resolve_deadlock`） | 节点面收敛 | 删 `RetryGate` 节点的三次 emit；`trace.incoming = Backtrack` 与写面 `backtrack` 落账逐字不变 |
| `crates/core/orchestration/src/walker.rs`（落账面） | 四处合流一处 | `step_phase_log` / `step_initialize_skipped_phase_log` 删除（职责并入 `settle_phase` 的 `Pass` / `Skipped` 形态）；`step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` 删除（并入 `PhaseSettle::Fail`）；`step_gate_fail_phase_log` 与 `GateFailLog` 参数化保留为**载荷构造辅助**（static-check / archi-check 共用），但函数体只装配 `PhaseFail` 并调 `settle_phase(PhaseSettle::Fail(..))`，原 `begin_node` / `emit_node` 两处删除 |
| `crates/core/orchestration/src/steps.rs`（`resolve_change_name`，新） | id → name 解析合流 | 四个门禁臂（StaticCheck / TestExecution / MutationTest / ArchiCheck）各写一遍的 `store.get_change(&change_id)?.map(\|r\| r.name)` 合为单函数；`ToolCommand::PhaseStart` / `PhaseLog` 命令与写面直调逐字不动 |
| `crates/core/workflow/src/write/phase_table.rs` | 计划步序列收窄 | 11 张 `*_STEPS` 删 30 处辅助步声明：initialize 3 / proposal 2 / dev-design 2 / test-design 2 / implement 4 / test-gen 3 / test-execution 2 / code-review 1 / acceptance 1 / archive 9 / code-analyze 2；doc 注释步序串同步 |
| `crates/core/workflow/src/state.rs`（`StepNode` / `StepGroup`） | 词汇冻结 | 二十二值保持（`PhaseStart` / `PhaseLog` / `VerdictGate` 标 `decode-only`）；`StepGroup::Gate` 分支保留（辅助步退场后无 StepNode 归属亦不删） |
| `crates/core/orchestration/src/state.rs`（`ChangeStepKind`） | 词汇冻结 | 二十二值保持、序不变（出线词与节点身份键的存量行依赖）；三个辅助步变体标 `decode-only` |
| `crates/core/orchestration/src/{port,control,snapshot,run_history,finish,verdict,decision,prompt,transcript}.rs` | 零改动 | 四个 runner port / `ToolCommand` 与 `ToolStepOutput` 封闭集 / `accumulate_node` 合并语义 / 快照装配 / `finish_step` 协议面（校验器 / 动态 schema / 载荷提取）/ `ensure_backtrack_allowed` 均不涉辅助步退场 |
| `crates/infra/store/**` | 零改动 | `PhaseRecord` / `StepRecord` / `RunNodeRecord` 形状与 `StepKind` 词面不变；`start_change_phase` / `log_change_phase` 的 attempt 推导式子逐字不动（词汇冻结即零迁移） |
| `packages/desktop/src/**` | 零改动（存量解码保留） | `run-state.ts` 的 `STEP_LABEL` 穷尽 Record 与 gate 分组保留三词（旧库行解码必需）；`change-flow-graph.tsx` 静默节点判据对「不存在的节点」天然无害 |
| `crates/core/orchestration/src/{walker_test,steps_test,control_test}.rs` + `crates/core/workflow/src/{write/phase_table_test,state_test}.rs` | 直测 | 形状面断言改锚点（`node_of(PhaseStart)` → 首个真实步；`step_instances(WhitelistGate)` → `step_instances(Decision)`；`node.step == PhaseLog` → `phase_log_input` 载荷 / pipeline attempts）；新增 `enter_phase` / `settle_phase` / 双失败面 / 词汇冻结守卫用例；6 处 `walk_run` 整链期望值重定 |

## 被本 delta 取代的主基线表述

主基线 `openspec/specs/desktop-change-orchestration/spec.md` 中以下片段在辅助步退场后不再成立；归档合并后 SHALL 以本 delta 的对应 requirement 替代（`grep -F` 旧行片段仍在即说明合并漏项）：

| 主基线旧行（片段） | 取代为 |
|---|---|
| `:11`「其 11 步序列按序执行」 | 9 步（MODIFIED「薄图 walker 相位循环与路由红线」） |
| `:25`「含 archive 相位 11 步」 / `:30`「11 步全 pass」 / `:31`「Preflight → … → PhaseLog 承接」 | 9 步 + 相位收口（同上） |
| `:36`「声明 Evaluator / VerdictGate 步而 `evaluator=None`」 | 去 `VerdictGate` 词（同上） |
| `:15`「verdict 解析失败注入本相位 evaluator 会话」 / `:17`「**verdict 解析失败**：解析错误诊断反馈边续注…超限升格相位 fail」 | 整条退役：裁决内化进 evaluator 步，缺席即 `Terminal::failed`，零反馈边（MODIFIED「evaluator 收口裁决内化与相位收口落账」） |
| `:17`「**verdict 解析失败**」bullet（整条）与 `:96`「ToolStep 节点（phase-start / phase-log / backtrack 相位机步…）」 | 前者删除（裁决内化进 evaluator 步）；后者去 `phase-start` / `phase-log` 并把 `backtrack` 归写明（MODIFIED「四类节点统一 walker 与可观测均一」） |
| `:98`「Gate 节点（verdict 解析 / 白名单校验 / 决策动作兑现…）」 | `Gate` 三门转存量解码词汇（同上） |
| `:100`「步词汇 SHALL 为全词汇封闭集（… + 三门 gate + 9 个归档步 …）」 | 保留但标注三门 gate 为存量解码（同上） |
| `:119`「initialize 相位 5 步与 archive 相位 11 步」 | 3 步与 9 步（同上 scenario） |
| `:198-215`「verdict 解析与 phase-log 代写」全文（含「解析失败反馈边修复」「解析失败超限升格」两 scenario） | RENAMED + 重写为「evaluator 收口裁决内化与相位收口落账」 |
| `:225`「未在封闭集内的动作…走「决策会话软拒环与漂移上限」的软拒路径」 | 改「由 `finish_step` 工具校验 `Err` 当场回灌」（MODIFIED「决策协议」） |
| `:235`「决策输出解析失败 / 越权由「决策会话软拒环与漂移上限」处置」 | 改「`ensure_backtrack_allowed` 一次即 `Terminal::failed`」（同上） |
| `:250`「walker 侧白名单门软拒（failed `WhitelistGate` 节点留图面证据、同会话续注纠正）」 | 改「decision 节点 failed + detail 携拒绝文案（一次即停）」（同上 scenario） |
| `:293-315`「决策会话软拒环与漂移上限」全文（含四条 scenario） | REMOVED（见 REMOVED Requirements）+ ADDED「驳回与动作裁决经节点 detail 可见」 |
| `:475`「动态面 SHALL 全部 append…verdict 解析失败修复注入（evaluator 续注）…」 | 去该注入项（MODIFIED「相位 prompt 组装面静态化」） |
| `:511-518`「evaluator 输出协议瘦身」全文（要求最终消息输出 checklist JSON 形状） | 改「调 `finish_step` 收口」（MODIFIED 同名 requirement） |
| `:522`「archive 相位六段串行（实为 10 步）」 / `:532`「归档 10 步节点」 | 9 步（MODIFIED「计划图与执行图对齐与步粒度回退」） |
| `:541-551`「确定性相位空 checklist 落账」中「在末尾 PhaseLog 步…`step_phase_log` 三分流」 | 改称 `settle_phase` 三分流（MODIFIED 同名 requirement） |
| `:569`「`PhaseStart → TestExecution → MutationTest → PhaseLog`」 | 「`TestExecution → MutationTest`」（MODIFIED「MutationTest 步接线与失败升格」） |
| `:569`「反馈边注入对象 SHALL 仅剩 static-check 失败注入 executor、verdict 解析失败注入 evaluator」 | 「仅 static-check / archi-check 失败注入 executor」（同上） |
| `:584`「`step_phase_log` test 分支消费双槽」 | 「`settle_phase` test 分支」（同上 scenario） |
| `:772`「已超限时 SHALL 升格相位 fail：经写面 `phase_log` 代写 fail checklist」 | 改「经相位收口 `settle_phase(PhaseSettle::Fail(..))` 代写」（MODIFIED「ArchiCheck 步接线与反馈边」；本步座位 / runner / 报告槽位 / 反馈上限均不变） |
| `:771`「与 static-check 的 5 次、verdict 的 5 次互不占用」 | 去 verdict 计数项（verdict 反馈环已退役）：仅「与 static-check 的 5 次互不占用」（同上） |
| `:593`「`phaseStart` 等单 emit 瞬步由此自然得 `started_at == ended_at` 同刻」 / `:604-607`「瞬步同刻」scenario | 单 emit 语义仅由直构用例承载（MODIFIED「步级时间戳与 token 消耗随行」） |
| `:631`「步序 `PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`」 / `:635`「**PhaseLog 步**：机械 checklist 纯桌面代写…」 / `:642`「四节点按序上图」 | 3 步 + 相位收口（MODIFIED「initialize 基线检查相位执行体」） |
| `:637`「以 skipped 落账收口」 | `enter_phase` → `settle_phase(skipped=true)`（同上） |
| Module Contract `:841`「`steps.rs`（LocalToolSteps）…每步结果落 StepRecord 审计行」 | 审计面随 `phase_start` / `phase_log` 退场而收窄：`ToolCommand::PhaseStart` / `PhaseLog` 不再有 walker 调用点，故不再产 `StepKind::PhaseStart` / `PhaseLog` 审计行；其余命令（`phase_next` / `backtrack` / `decision_log` / 三个检查 runner）照常留审计 |
| Module Contract `:843`「`verdict.rs` + `decision.rs`（不改）…解析失败显式 Err 上抛——处置归 walker 按步声明分叉（verdict 反馈边 / 决策终态）」 | 该行描述的是文本解析时代形态（`parse_verdict` / `parse_decision` 已在 `agent-finalize-tool-for-flow-steps` 退役）；本变更后两文件只保留 `Value` 入参校验器与 `ensure_backtrack_allowed`，处置归 `finish_step` 工具 Err 当场回灌与 decision 节点 detail |
| Module Contract `:882`「`decision_session` 决策会话软拒环…每轮 emit `WhitelistGate` 实例」 | 改「动作裁决写 decision 节点 detail」（见本 delta Module Contract） |
