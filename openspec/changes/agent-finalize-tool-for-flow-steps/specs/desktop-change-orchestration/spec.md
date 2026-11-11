# desktop-change-orchestration Specification (Delta)

## REMOVED Requirements

### Requirement: 决策会话软拒环与漂移上限

**Reason**: 决策输出已由文本 JSON 改为 `finish_step` 工具协议（见 MODIFIED「决策协议」）。协议校验失败（形状 / 动作封闭集 / 缺字段 / reason 超限）由工具体在**同一 turn 内当场回灌** is_error ToolResult，模型立即重试——「跨轮软拒 + 同会话续注 + `DECISION_DRIFT_LIMIT = 2` 计数」的三个组成部分全部失去消费者。保留它等于同时维护两套纠错机制。

**Migration**: ① 白名单越权（唯一留在编排侧的自包含旁门）由「软拒两次后终止」改为**一次即 `Terminal::failed`**——拒绝文案仍携 `allowed` 列表，`WhitelistGate` 节点仍呈 failed 保留图面证据；② `ask` 停等改由「决策会话经 `finish_step` 收口与 ask 停等外移」承载（工具收口后 walker emit `RunUpdate::Ask` → `wait_answer` → 应答开启新一轮决策会话）；③ 漂移计数常量 `DECISION_DRIFT_LIMIT` 与纠正 prompt `decision_fix_prompt` 整体删除，`grep` 零命中为验收口径。

## MODIFIED Requirements

### Requirement: verdict 解析与 phase-log 代写

Evaluator 会话 SHALL 以 **`finish_step` 工具调用**收口（不再在最终消息输出 checklist JSON，不自调 MCP `phase_log`；工具协议定义见 desktop-agent-execution「finish_step 工具协议与本步收口」）。Walker SHALL 从 evaluator 轮产出中读结构化的 `finalized` 载荷（`WorkerTurnOutcome.finalized`，源头为转录末条 `SystemNotice{subtype:"finish_step"}`）并代调写面 `phase_log`（进程内）落 eval 记录（PhaseRecord + ChecklistItemRecord 原子落库）；MUST NOT 再从最终消息文本解析 verdict（`parse_verdict` / `extract_json` 等文本提取面随本变更删除，`grep` 零命中）。`finalized` 缺席 SHALL 判相位失败并 `Terminal::failed`，`report` 文案 SHALL 按成因可分：**输出被截断**（转录含 `output_truncated` notice → 文案点明截断成因，用户可调 `max_tokens`）与**未调用 `finish_step`**（模型自然结束却无收口）两态可区分；MUST NOT 臆测 verdict、MUST NOT 静默跳过落账、MUST NOT 回退到文本解析（显式接受的回退：截断即失败）。checklist 形状 SHALL 为逐项 `{ item, pass, evidence }`（`item` 为 md 静态清单「检查项」列的权威正文，由工具在校验期映射）；phase / attempt / skipped SHALL 由 walker 盖戳落账（evaluator 不回声），`EvaluatorChecklist` MUST NOT 要求 phase / attempt / skipped 字段，且 SHALL NOT 由模型自报 `verdict`（`verdict` 恒由 `derive_verdict(checklist)` 推导，见 desktop-workflow-write-face「verdict 推导单点」）。确定性相位（test-execution 机械 checklist、archive 空 checklist）SHALL 走 `step_phase_log` 的独立分支，不经 evaluator 收口产物。

**verdict-gate 的反馈面整体退役**：`finalized` 缺席即终态 ⇒ 不存在「解析失败 → 续注修复 → 重解析」这条路径，故 `step_verdict_gate` 的反馈环、`feedback_limit` 的 verdict-gate 服务面、`OnFail` 两值在 VerdictGate 步上的分派（`Terminal` 与 `FeedbackLoop` 语义合一，均为「缺席即终态」）SHALL 全部删除。**连带退役**：反馈环超限分支的升格落账面 `step_verdict_gate_upgraded_phase_log`（桌面代写一条 `item = "评估输出格式"` 的 fail 条目）SHALL 整函数删除——其唯一调用点即被删的超限分支，MUST NOT 以「零调用者死代码」形态留存（`grep` 零命中为判据）。walker 的**机械 checklist 代写面** SHALL 收为 7 个条目（静态检查 / 架构检查 / 测试执行 / 突变测试 / suite 结论一致 / 聚合计数一致 / mutation null 自动通过；其中静态检查与架构检查两调用点共用 `step_gate_fail_phase_log` 单点构造），这些条目 MUST NOT 经 `finish_step`、MUST NOT 被要求提供静态清单 ID；`"评估输出格式"` 条目 MUST NOT 出现在任何构造点。

#### Scenario: verdict 代写落账

- **WHEN** evaluator 经 `finish_step` 提交合规 checklist（逐项 pass，report 1000 字符）
- **THEN** walker 以该载荷经写面 `phase_log` 落 eval 记录（verdict 由 checklist 推导），`phase_next` 随后推进；`VerdictGate` 节点 detail 含 `verdict=pass|fail`，`feedback` 边零产出

#### Scenario: 收口产物缺席即失败

- **WHEN** evaluator 会话以 `status = Completed` 结束但 `finalized` 为 `None`（模型自然结束、未调 `finish_step`）
- **THEN** `VerdictGate` 节点呈 failed（detail 为「未调用 finish_step 收口」类文案），相位以 `Terminal::failed` 收口；状态库无新增 eval 记录、零反馈边重试

#### Scenario: 截断成因可区分

- **WHEN** evaluator 输出因 `max_tokens` 截断收场（转录含 `output_truncated` notice）且 `finalized` 缺席
- **THEN** `VerdictGate` failed 的成因文案点明「输出被截断」而非「未调用 finish_step」；不触发文本解析回退

#### Scenario: 反馈升格面整体退役

- **WHEN** 审查 verdict-gate 路径的源码与导出面
- **THEN** `step_verdict_gate` 无反馈环、无 `run_worker(Evaluator, …, EdgeKind::Feedback)` 调用、无 `feedback_limit(step, "verdict-gate")` 调用点；`step_verdict_gate_upgraded_phase_log` 与 `"评估输出格式"` 条目全仓 `grep` 零命中；`step_verdict_gate` 与 `step_verdict_gate_terminal` 已合并为单一裁决（无 `on_fail` 分派）

#### Scenario: 机械代写面收为七条目

- **WHEN** 审查 walker 的全部 `ChecklistItem` 构造点
- **THEN** 恰 6 处构造点、7 个条目文案（静态检查 / 架构检查 / 测试执行 / 突变测试 / suite 结论一致 / 聚合计数一致 / mutation null 自动通过），其中前两条经 `step_gate_fail_phase_log` 的 `GateFailLog` 参数化共用一处构造；不存在 `"评估输出格式"` 条目；这些条目全不经 `finish_step` 校验器，零 ID 要求

### Requirement: 决策协议

写面 `phase_next` 返回决策分叉触发（任意相位最新 fail 条目未挂决策会话；或窗口 fail 数达重试上限的标注变体）时，walker SHALL 唤起决策 agent（compose_turn 新会话，`ModelLevel::High`）；walker 自走同相位重跑 SHALL 退役（盲目重跑的会话消耗高于一次决策）。确定性相位（initialize / archive：executor / evaluator 双缺席）不触发决策，fail 落账照常同相位重路由。决策 agent 输入 SHALL 有界且来自 db 状态与写面响应：fail checklist（fail 项 + evidence）、失败条目 `report`（失败报告原文，见「失败升格落账与决策信息面」）、`allowed_backtrack_phases`（写面 `phase_next` 白名单；每项 SHALL 携相位特性描述与该相位声明的可操作产物清单，描述 / 产物单源 = 本期相位表 `PhaseDefinition.description` / `PhaseDefinition.artifact_manifest`，随表分派不串味）、候选相位最近一次 eval report。决策 prompt 白名单段 SHALL 逐条渲染 `- <相位 id>（<特性描述>）：<可操作产物段>`，供决策 agent 判断回溯目标（回溯即重做该相位产物；描述空串退化为 id-only 行，零声明产物渲染「无声明产物」）。决策输入 MUST NOT 携相位 retry 语义标记（retry 已退役，无「评估-only / 带 executor」二分支指引）。决策输出 SHALL 为**经 `finish_step` 工具提交**的封闭集三动作（不再在最终消息输出 JSON）：

- `{ action: "backtrack", to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；目标为失败相位自身即「重做本相位」（仅当该相位在白名单内）；
- `{ action: "stop", reason }` — 终止 run；
- `{ action: "ask", question, options[] }` — 无法裁决，UI 中断提问。

封闭集外的动作（含已退役的 `retry`）与形状漂移 SHALL 由 `finish_step` 的工具校验 `Err` **当场回灌**（is_error ToolResult，模型在同一 turn 内纠正重试）——MUST NOT 一次即打死 run，MUST NOT 跨轮计数。重做语义 SHALL 由白名单自含承载（单一载体）：`allowed_backtrack_phases` 含失败相位自身 ⟺ 该相位可重做（其 step 序列含 agent 步）；零 agent 步的相位（test-execution / archive）自身不入白名单——决策上下文里不存在「重做本相位」这个选项。

决策决议的兑现 SHALL 依赖「每个 fail 条目只触发一次决策」语义：决策会话挂账（decision_session_id）后的同一条目 MUST NOT 重复触发决策分叉（该条目已进入过决策，不再重复分叉）。唯一合法重入路径 SHALL 为决策 backtrack——写面改写最新条目 `backtrack_to` → 下一轮 `phase_next` 的回溯分支先手接管路由（走不到两道分叉判断）→ 目标相位真执行 → 新落账 fail 条目无决策标记，触发点对新条目照常生效。MUST NOT 以「放行同相位重跑」作为兑现面：删掉该幂等闸后，fail 过的相位在重做 pass 后仍会因最新 fail 条目再次分叉、相位永不执行，直烧到 `MAX_ROUNDS=20` 硬失败。

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验（工具侧兜底）：越权目标 SHALL 被拒绝且不落任何越权回跳账（状态库零变更）。决策收口后 walker SHALL 仍执行 `ensure_backtrack_allowed`（自包含校验无法覆盖的运行时门：白名单随 `phase_next` 动态下发），越权 SHALL 一次即 `Terminal::failed`（文案携 `allowed` 列表；自环文案提示零 agent 步相位不支持重做）。

#### Scenario: 白名单条目携相位特性描述与可操作产物

- **WHEN** 决策 prompt 渲染可回溯白名单
- **THEN** 完整决策 prompt 的每条白名单行形如 `- <相位 id>（<相位特性描述>）：<可操作产物段>`（描述取本期相位表 `PhaseDefinition.description`、产物取 `PhaseDefinition.artifact_manifest`，随 workflow_type 分派——requirement / test-only 同名相位描述与产物不串味），产物段逐项渲染 `` `<相对路径>`（<产物标题>） ``、多项以「、」分隔，相位零声明产物时显式渲染「无声明产物」；评估 checklist MUST NOT 出现在产物段（PhaseRecord 投影、非相位声明产物），失败相位自身条目在描述后追加「（= 重做本相位）」

#### Scenario: 白名单内自治回跳

- **WHEN** test-execution#1 fail 且决策 agent 经 `finish_step` 提交 `{action:"backtrack", to:"test-gen", reason:...}`（test-gen 在白名单内）
- **THEN** walker 经写面 `backtrack` 落回跳记录（落库），`phase_next` 随后路由至 test-gen 重做，reason 随边标签可读；重做相位首节点 incoming 边为 `backtrack`

#### Scenario: 越权决议一次即停

- **WHEN** 决策 agent 经 `finish_step` 提交的 `to` 不在 allowed_backtrack_phases 内（或写面 `backtrack` 被直接以越权目标调用）
- **THEN** `WhitelistGate` 节点呈 failed（detail 携 `allowed` 列表）、run 以 `Terminal::failed` 收口（无第二轮软拒续注）；写面 `backtrack` 的二次校验对越权目标仍显式拒绝且状态库零变更

#### Scenario: ask 中断提问

- **WHEN** 决策 agent 经 `finish_step` 提交 `{action:"ask", question:..., options:[...]}`
- **THEN** 该轮决策会话以 ask 收口，run 进入中断等待态，UI 呈现问题与选项；用户应答后 run 以新一轮决策会话（`Continue` 全史）继续（见「决策会话经 finish_step 收口与 ask 停等外移」）

#### Scenario: verdict fail 分叉与重做自含

- **WHEN** code-review#1 落 verdict=fail（评估-only 相位，自身在白名单内）且决策 agent 经 `finish_step` 提交 `{action:"backtrack", to:"code-review", reason:...}`
- **THEN** walker 唤起决策 agent 恰一次，回溯落账后同相位以新 attempt 真执行（重做本相位由白名单自含表达，`retry` 动作不存在）；重跑再落新 fail 条目时决策分叉对新条目照常触发。带 executor 相位（如 implement）verdict fail 同式分叉，重做携带失败清单段

#### Scenario: verdict fail 分叉与 retry 兑现

- **WHEN** 审查决策协议的兑现路径（本条为历史 scenario 名的存续面）
- **THEN** 原「`retry` 决议兑现」路径已退役（`DecisionAction::Retry` 删除、未知名由工具 Err 当场回灌），兑现语义改为「每个 fail 条目只触发一次决策」幂等闸 + 决策 backtrack 重入；重做本相位经回溯到自身兑现（见「verdict fail 分叉与重做自含」与「已决条目不重复分叉」）

#### Scenario: 已决条目不重复分叉

- **WHEN** 某相位最新 fail 条目已挂决策会话（decision_session_id 在案）后 `phase_next` 再被调用
- **THEN** 该条目不触发第二次决策分叉（幂等闸生效）；决策回溯到上游后重入产生的新 fail 条目（无决策槽位）照常分叉

### Requirement: 相位 prompt 组装面静态化（静态主体 + 动态 append）

executor / evaluator 会话 prompt SHALL 以写面下发的静态角色知识文本（desktop-phase-prompts / desktop-workflow-write-face）为主体；动态面 SHALL 全部 append 而非模板替换：上下文头一行 `change: <name>`、回溯原因行、static-check / 测试修复失败注入、输出协议附录（evaluator 的 `finish_step` 收口指引）、decision 三动作与有界输入段（决策会话）。`finish_step` 的 description / parameters 形状 SHALL 由**工具挂载面**承载（挂载清单 + 动态 schema），MUST NOT 作为 prompt 正文重复一遍——prompt 侧只留「何时收口」的一句指引。verdict 解析失败修复注入 SHALL 退役（工具 Err 文案是唯一反馈面，见 desktop-agent-execution「finish_step 工具协议与本步收口」）。prompt 组装 MUST NOT 使用 `<change>` / `<phase>` 模板替换。`WorkerTurnRequest.provenance.source_ref = <change>/<phase>/<role>/<attempt>` 与 `root` / `model_level` 结构化通道不变——phase / attempt 盖戳不依赖 prompt 文本。

#### Scenario: 上下文头在场

- **WHEN** walker 发起 executor / evaluator 会话
- **THEN** prompt 含一行 `change: <name>`（change 名自写面记录 name 供给）
- **AND** prompt 主体为静态角色知识文本，无 `<change>` / `<phase>` 模板替换痕迹

#### Scenario: 反馈注入仍 append

- **WHEN** static-check / archi-check / 测试执行失败走定向反馈边
- **THEN** 修复反馈文本以 append 注入同一 agent 会话 prompt（不重写静态主体）——门禁步注入 executor 会话
- **AND** 回溯场景的回溯原因行照常 append
- **AND** verdict 解析失败注入面已退役（`verdict_fix_prompt` / `decision_fix_prompt` 全仓零命中）

#### Scenario: 协议形状不进 prompt

- **WHEN** 审查 evaluator / decision 会话的 prompt 文本与工具 definitions
- **THEN** checklist / 三动作的字段形状只出现在 `finish_step` 的 parameters 中；prompt 侧仅有「以 `finish_step` 收口」一句指引与禁调写通道红线

### Requirement: evaluator 输出协议瘦身

`evaluator_protocol` 输出协议附录 SHALL 要求 evaluator 以 **`finish_step` 工具调用**收口（工具参数为逐项 checklist + report，形状由工具 parameters 给出，见 desktop-agent-execution「finish_step 工具协议与本步收口」）；MUST NOT 要求最终消息输出 JSON 对象、MUST NOT 要求回声 `verdict`（verdict 由 checklist 推导）、MUST NOT 要求 phase / attempt / skipped 回声（由 walker 按 provenance 盖戳）。协议 SHALL 保留禁调 MCP 写通道红线（phase_log / phase_next / phase_start / backtrack，落账由桌面编排代写）与 `report` ≤2000 字符上限；SHALL 说明 checklist 逐项需 ID + pass + evidence（ID 取自本 prompt 的静态清单）。

#### Scenario: 协议形状改指工具

- **WHEN** 读取 evaluator 输出协议附录文本
- **THEN** 要求为「调用 `finish_step` 收口」+ report 上限 + 逐项 pass / evidence 要求
- **AND** 不再出现 checklist JSON 形状定义、不再要求输出 verdict / phase / attempt / skipped 字段
- **AND** 仍含禁调 MCP 写通道红线

#### Scenario: 两个跨轮修复面退役

- **WHEN** 审查 `prompt.rs` 的导出面
- **THEN** `verdict_fix_prompt` 与 `decision_fix_prompt` 零存在（`grep` 零命中）；ask 应答的唯一再提示面为 `decision_answer_prompt`

## ADDED Requirements

### Requirement: 决策会话经 finish_step 收口与 ask 停等外移

决策会话 SHALL 以单轮收口为常态（`finish_step` 一调即本 step 结束，一轮一命拓扑下即本 turn 收口）：

- **单轮收口**：决策 agent 经 `finish_step(Decision)` 提交三动作之一 → 该轮决策会话收口（`WorkerTurnOutcome.finalized = Decision(..)`）→ walker 读 `finalized` 后执行 `ensure_backtrack_allowed` 白名单门（越权一次即 `Terminal::failed`）→ 按动作兑现（backtrack 经写面、stop 终止 run、ask 见下）。`decision-log` 挂账 SHALL 保持每轮一次、幂等覆写（置于动作兑现之前——stop / 拒收路径同样留痕，转录仍在库内可经调试页 change 筛选回查）。
- **ask 停等外移**：`ask` 收口后 SHALL 由 walker（不再由决策会话内部）执行停等——emit `RunUpdate::Ask { question, options }` → `guard.wait_answer().await`（取消置位 → `Terminal::stopped`「ask 停等被终止」）→ 应答后**新开一轮决策会话**（`SessionRef::Continue`，resume 全史；prompt 单点为 `decision_answer_prompt(answer)`），新轮同样以 `finish_step` 收口。ask 轮 SHALL NOT 计入任何失败 / 漂移计数（用户驱动的停等，非模型漂移）。
- **每轮图面节点**：每轮决策收口后 SHALL emit 一个 `WhitelistGate` 节点实例（`instance_no` 递增）：通过呈 passed、驳回呈 failed 并携拒绝文案——图面可见，MUST NOT 静默丢弃。ask 轮不 emit `WhitelistGate`（未产生动作裁决）。
- **零跨轮纠错**：协议层纠错全部在工具内闭合（is_error 当场回灌），决策会话 MUST NOT 内置软拒环、MUST NOT 维护跨轮漂移计数（`DECISION_DRIFT_LIMIT` 已删）。

#### Scenario: 单轮决策收口

- **WHEN** 决策 agent 首轮即以 `finish_step` 提交合规 `backtrack`（白名单内）
- **THEN** 决策会话数 1、轮数 1、`decision-log` 挂账 1 次、`WhitelistGate` passed 1 个；写面 `backtrack` 落账后 `phase_next` 重路由

#### Scenario: ask 外移与应答续轮

- **WHEN** 决策 agent 首轮以 `finish_step` 提交 `ask`，用户应答后第二轮提交合规 `backtrack`
- **THEN** 首轮 `RunUpdate::Ask` 送出、run 进入等待态、首轮决策会话收口（会话数 1）；应答后以同一会话 `Continue` 开第二轮（prompt 为 `decision_answer_prompt(answer)`），第二轮收口后 `WhitelistGate` passed 1 个、`backtrack` 落账；全程零漂移计数、零 `decision_fix_prompt`

#### Scenario: 取消置位终止停等

- **WHEN** ask 停等期间用户终止 run
- **THEN** 以 `Terminal::stopped`（reason「ask 停等被终止」）收口；MUST NOT 留悬挂等待、MUST NOT 产出 `WhitelistGate` 节点

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/finish.rs`（新） | 工具协议单点 | `FinishPayload { Evaluator(EvaluatorChecklist), Decision(DecisionAction) }`（`Serialize + Deserialize`）；`description(FinishSpec) -> &'static str`；`parameters(FinishSpec, prompt) -> Value`（evaluator 臂 `items[].id.enum` 由 `checklist_entries(prompt)` 派生）；`validate(FinishSpec, &Value, prompt) -> Result<FinishPayload, String>`（纯函数，Err 文案自足） |
| `crates/core/orchestration/src/verdict.rs` | checklist 校验 | 删 `extract_json` / `fenced_block` / `brace_slice` / `parse_verdict`；`EvaluatorChecklist` 加 `Serialize`；新增 `validate_checklist(&Value, prompt) -> Result<EvaluatorChecklist, String>`（形状 / ID 集合比对 / `item` 映射 / report 上限 / verdict 由 `derive_verdict` 推导）；`MAX_REPORT_CHARS = 2000` 保留（`infra/archi` 消费者在场） |
| `crates/core/orchestration/src/decision.rs` | 动作校验 | 删 `parse_decision`；新增 `validate_decision(&Value) -> Result<DecisionAction, String>`；`DecisionAction` 加 `Serialize`；`ensure_backtrack_allowed` 逐字不动 |
| `crates/core/orchestration/src/prompt.rs` | prompt 组装 | `evaluator_protocol` 改「调 `finish_step` 收口」；`decision_prompt` 协议段改工具协议；删 `verdict_fix_prompt` / `decision_fix_prompt`；新增 `decision_answer_prompt(answer: &str) -> String`（ask 应答唯一再提示面） |
| `crates/core/orchestration/src/port.rs` | 轮产出 | `WorkerTurnOutcome` 增 `finalized: Option<FinishPayload>` |
| `crates/core/orchestration/src/transcript.rs` | 载荷提取 | 新增 `finish_payload(&[AgentEvent]) -> Option<FinishPayload>`（末条 `SystemNotice{subtype:"finish_step"}` 的 payload 反序列化，缺失 / 损坏 → `None`） |
| `crates/core/orchestration/src/walker.rs` | 门与决策 | `step_verdict_gate` 与 `step_verdict_gate_terminal` 合并为单一裁决（读 `finalized`；缺席按截断 / 未收口两文案 `Terminal::failed`）；verdict-gate 的反馈环、`on_fail` 分派与 `feedback_limit` 的 verdict-gate 调用点删除（`feedback_limit` 本体保留，只服务 static-check / archi-check）；**`step_verdict_gate_upgraded_phase_log` 整函数删除**（含 `"评估输出格式"` fail 条目，唯一调用点在被删的超限分支）；`decision_session` 改单轮收口 + ask 停等外移 + drift 环删除；`PhaseCursor.verdict` 由 `finalized` 填；其余四处机械 checklist 代写点零改动 |
| `crates/core/orchestration/src/lib.rs` | 出口 | 再导出 `finish::{FinishPayload, description, parameters, validate}`；`WorkerTurnOutcome` 随 `port` 出口携 `finalized` |
| `crates/core/orchestration/src/{prompt_test,verdict_test,decision_test,walker_test,port_test,transcript_test}.rs` + `finish_test.rs`（新） | 直测 | 退役函数的段落断言删除；`FakeWorker` 产出夹具改注 `SystemNotice{finish_step}`（沿 `with_transcripts()` 注入口）；**`verdict解析失败反馈边修复_续注同evaluator会话重解析` 与 `真实写面组合_verdict解析失败反馈边超限_升格双槽位决策stop` 两条用例整体删除**（被测路径随反馈环与 `step_verdict_gate_upgraded_phase_log` 消失，`item == "评估输出格式"` 的落账断言无替代面）；新增校验器 / 动态 schema / 载荷提取 / 单轮与 ask 三形态用例 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `crates/core/workflow/src/write/phase_table.rs` | 零改动：步序列、`include_str!` 装配与 `VerdictGate` 步的 `on_fail` 声明保持原位（声明成惰性字段，清理留独立变更） |
| `crates/core/orchestration/src/{state,snapshot,control,run_history,steps}.rs` | 零改动：图面节点词 / 快照 / 停等通道 / 写面直调 / 工具步执行体均不涉协议变更 |
| `crates/infra/store/**` | 零改动：`ChecklistItemRecord` 形状与落账形态不变 |
| `crates/core/orchestration/src/walker.rs` 的其余四处机械 checklist 代写点 | 零改动：`step_gate_fail_phase_log`（`GateFailLog` 单点，静态检查 / 架构检查两调用点共用）、`test_execution_phase_log_input`（三条目）、`step_test_execution_upgraded_phase_log`、`step_mutation_test_upgraded_phase_log`——合计 7 个条目文案逐个保留原位，不经 `finish_step`、零 ID 要求（见 desktop-agent-execution「finish_step 工具协议与本步收口」校验边界） |

### 被本 delta 取代的主基线表述

主基线 `openspec/specs/desktop-change-orchestration/spec.md` 下述片段描述的是已退役的文本协议 / 反馈环形态；归档合并后 SHALL 以本 delta 的对应 requirement 替代（`grep -F` 旧行片段仍在即说明合并漏项）：

| 主基线旧行（片段） | 取代为 |
|---|---|
| `:15`「static-check 失败注入本相位 executor 会话，verdict 解析失败注入本相位 evaluator 会话」 | 该列举去「verdict 解析失败注入 evaluator」一半：verdict-gate 无反馈边、零 evaluator 续注；static-check 一半逐字保留（archi-check 的注入面由主基线「ArchiCheck 步接线与反馈边」自持，本变更不触碰） |
| `:17`「**verdict 解析失败**：解析错误诊断反馈边续注同一 evaluator 会话修复重跑…超限升格相位 fail 进决策分叉」 | 该 bullet 整体退役：`finalized` 缺席即 `Terminal::failed`，零反馈边重试、零升格落账（同上） |
| `:198-215`「verdict 解析与 phase-log 代写」全文（含「解析失败反馈边修复」「解析失败超限升格」两 scenario） | MODIFIED「verdict 解析与 phase-log 代写」（`finish_step` 收口 + 反馈面退役） |
| `:266-288`「决策会话软拒环与漂移上限」全文 | REMOVED（见本文件 REMOVED Requirements）+ ADDED「决策会话经 finish_step 收口与 ask 停等外移」 |
| `:438`「动态面…verdict 解析失败修复注入（evaluator 续注）…」 | MODIFIED「相位 prompt 组装面静态化」（动态面去该注入） |
| `:527`（MutationTest 步接线）「反馈边注入对象 SHALL 仅剩 static-check 失败注入 executor、verdict 解析失败注入 evaluator」 | 该句收为「**仅 static-check 失败注入 executor**」（verdict-gate 反馈面退役；archi-check 注入面由主基线「ArchiCheck 步接线与反馈边」自持，不在该句射程内）；该句其余内容（两步 `Escalate`、`feedback_limit` 的 test / mutation 计数面删除、`step_phase_log` 三槽分流）逐字保留 |
| `:801`「`verdict.rs` + `decision.rs`（不改）…解析失败显式 Err 上抛——处置归 walker 按步声明分叉（verdict 反馈边 / 决策终态）」 | 两文件改 `Value` 入参校验器（见 Module Contract）；verdict 反馈边退役、决策改工具 Err 当场回灌 |
| `:842` / `:865`（walker 门禁步执行体行） | 逐字保留（static-check / archi-check / test / mutation 四步全不动），仅 verdict-gate 行按上述 MODIFIED 替代 |
