# desktop-change-orchestration Specification（delta: backtrack-window-feedback-overlap）

## Purpose

本 delta 只把「失败反馈段在同源格被写面整段省略」这条组装面契约登记进编排侧，**零代码改动**：

- 反馈段的**在场性**完全由写面下发的 prompt 决定：同源格（当前相位 = 某开放窗口的发起相位）里写面已省略失败反馈段，orchestration MUST NOT 依据 `backtrack_window` 自行抑制或补回（零本地判据、零二次推导）。
- 段序契约改为条件表述：**两段并存时**窗口段先于反馈段；同源格下反馈段缺席，序退化。
- 决策 prompt 不受同源抑制：决策输入仍携 fail checklist / `fail_report` / `backtrack_window` 三面（决策面与相位 prompt 是两个渲染面）。
- 窗口段条目数、双分支裁剪、零二次裁剪与既有一切纪律逐字不变。

## MODIFIED Requirements

### Requirement: 相位 prompt 组装面静态化（静态主体 + 动态 append）

executor / evaluator 会话 prompt SHALL 以写面下发的静态角色知识文本（desktop-phase-prompts / desktop-workflow-write-face）为主体；动态面 SHALL 全部 append 而非模板替换：上下文头一行 `change: <name>`、回溯重做窗口段（见「决策协议」与 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」）、失败反馈段（门禁修复注入 / fail 重试注入；**同源格由写面整段省略**，见下）、输出协议附录（evaluator）、decision 两动作与有界输入段（决策会话）与一句「无法裁决时调用 `ask_user_question` 工具」指路。prompt 组装 MUST NOT 使用 `<change>` / `<phase>` 模板替换。段序 SHALL 为「上下文头 → 静态主体 → 回溯重做窗口段 → 失败反馈段（门禁修复注入）→ 输出协议附录（evaluator）」：窗口段是失败反馈段的上位语境（前者答「为什么这个相位被叫回来」，后者答「本相位这次错在哪」），MUST NOT 倒置于反馈段之后；单行「回溯原因」形式已退役（窗口段完全取代，单点渲染）。该段序 SHALL 以**两段并存**为前置条件：同源格（写面判据：覆盖当前相位的开放窗口集合中存在 `from_phase == 当前相位`）下失败反馈段整段缺席，序退化为「上下文头 → 静态主体 → 回溯重做窗口段 →（evaluator）输出协议附录」。反馈段的在场性 SHALL 完全由写面下发的 prompt 文本决定：orchestration MUST NOT 依据 `PhaseNextOutcome.backtrack_window` 自行抑制、裁剪或补回反馈段，MUST NOT 另持一份「同源」判据（判据单源 = desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」）。窗口段的条目数与裁剪完全由写面下发的集合决定（写面常量 `MAX_BACKTRACK_WINDOWS`：≤ 该上限时全量并集、超限取创建序末位两层；双分支完整表述见「决策协议」与 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」），orchestration 侧 MUST NOT 二次裁剪、亦不定义该常量。**verdict 裁决内化后动态面 SHALL NOT 含任何「verdict 解析失败修复注入」**——纠错由 `finish_step` 工具在同一 turn 内闭合，不产生跨轮反馈注入。**协议形状不进 prompt 主体**：`finish_step` 与 `ask_user_question` 的 description / parameters 形状 SHALL 由**工具挂载面**承载（挂载清单 + 动态 schema），MUST NOT 作为 prompt 正文重复一遍——prompt 侧只留「何时收口 / 何时提问」的各一句指引与落账归属红线。`decision_answer_prompt`（ask 应答的唯一再提示面）SHALL 随 ask 停等内移一并退役（应答不再开新一轮，`grep` 零命中）。`WorkerTurnRequest.provenance.source_ref = <change>/<phase>/<role>/<attempt>` 与 `root` / `model_level` 结构化通道不变——phase / attempt 盖戳不依赖 prompt 文本。

#### Scenario: 上下文头在场

- **WHEN** walker 发起 executor / evaluator 会话
- **THEN** prompt 含一行 `change: <name>`（change 名自写面记录 name 供给）
- **AND** prompt 主体为静态角色知识文本，无 `<change>` / `<phase>` 模板替换痕迹

#### Scenario: 反馈注入仍 append

- **WHEN** static-check / archi-check 失败走定向反馈边
- **THEN** 修复反馈文本以 append 注入同一 executor 会话 prompt（不重写静态主体）
- **AND** 回溯场景改为 append 回溯重做窗口段（单行回溯原因行零残留）

#### Scenario: 段序窗口段先于反馈段

- **WHEN** walker 为一个既落在回溯窗口内、又带 fail 重试反馈、且**不是**该窗口发起相位的相位（异源并存格）组装 evaluator prompt
- **THEN** 序为「上下文头 → 静态主体 → 回溯重做窗口段 → 失败反馈段 → 输出协议附录」；窗口段索引小于失败反馈段索引，输出协议附录仍在最后

#### Scenario: 决策 prompt 指路 ask 工具而非承载其形状

- **WHEN** 审查决策会话 prompt 文本与工具 definitions
- **THEN** prompt 只含两动作说明与一句「无法裁决时调用 `ask_user_question` 工具（问题 + 非空选项）」；`question` / `options` 的字段形状只出现在该工具的 parameters 中；`decision_answer_prompt` 全仓零命中

#### Scenario: 同源格反馈段由写面省略

- **WHEN** 写面路由回某开放窗口的发起相位（其最新条目即回溯标记行），`PhaseNextOutcome.executor` / `evaluator` 的 prompt 已不含 `⚠️ 上次评估未通过` 段
- **THEN** walker 原样下发该 prompt（零补写、零依据 `backtrack_window` 的本地抑制 / 补回分支），executor / evaluator 请求的 prompt 中窗口段在场、反馈段缺席
- **AND** 源码审查确认 orchestration 侧零「同源」判据、零 `from_phase ==` 类比较、零 `retry_feedback` 概念

#### Scenario: 决策 prompt 不受同源抑制

- **WHEN** 失败相位自身即某开放窗口的发起相位，决策分叉进入决策会话
- **THEN** 决策 prompt 仍携失败清单段、`## 失败相位报告` 段与 `## 回溯重做窗口` 段（决策面消费 `DecisionInput.fail_report` / checklist / `backtrack_window` 三面，同源抑制只作用于相位 prompt 渲染层）

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs`（零改动） | 相位 prompt 消费方 | `step_executor` / `step_evaluator` 原样下发写面 `ResolvedPhaseSpec.prompt`；零依据 `next.backtrack_window` 的抑制 / 补回分支、零同源判据、零新 route 规则、零新 port |
| `crates/core/orchestration/src/prompt.rs`（零改动） | 组装面 | `executor_prompt` / `evaluator_prompt` 仍只追加静态主体与输出协议附录；`decision_prompt` 的失败清单 / `fail_report` / 窗口段三面逐字不变（同源抑制不作用于决策面） |
| `crates/core/workflow/src/write/phase_next.rs`（见 desktop-workflow-write-face） | 判据单源 | 同源抑制的唯一实现点与唯一判据；本能力只消费其结果，零判据副本 |
| `crates/core/orchestration/src/{port.rs,steps.rs,control.rs,state.rs}`（零改动） | 工具步与状态面 | `ToolStepOutput::PhaseNext(Box<PhaseNextOutcome>)` 载荷不变；零新步词、零新通知类型、零新状态值 |

### Function: `crates/core/orchestration/src/walker.rs` / `prompt.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `step_executor` / `step_evaluator` | 零改动 | 会话 prompt 取自写面 `PhaseNextOutcome.executor` / `evaluator`；同源格少一段反馈文本是数据面差异，调用面零分支 |
| `evaluator_prompt(...)` | 零改动 | 仍只追加输出协议附录；同源格下最终序退化为「上下文头 → 静态主体 → 窗口段 → 输出协议附录」 |
| `decision_prompt(...)` | 零改动 | 失败清单段 / `## 失败相位报告` / `## 回溯重做窗口` 三面与段序不变（同源抑制不触达决策面） |
| `build_decision_input(...)` | 零改动 | 仍透传 `next.backtrack_window`；零 store 读取、零相位表推导 |

### API: `DecisionInput` / `PhaseNextOutcome`

| 项 | 变化 | 描述 |
|---|---|---|
| `DecisionInput` | 零改动 | `fail_report` / `allowed` / `candidates` / `backtrack_window` 字段面与有界纪律不变；同源抑制不改变任何字段值 |
| `PhaseNextOutcome` | 零改动 | 字段面与填充不变；唯一可观测差异在 `executor` / `evaluator` 的 `prompt` 文本（同源格少一段） |
| bindings | 零变化 | 两者均不出 bindings，`bindings:export` 零 diff |

### CLI / Component: 零变化

| 项 | 变化 | 描述 |
|---|---|---|
| 运行控制命令面（`src/commands/change_flow/**`） | 零改动 | `start` / `stop` / `answer` / `confirm` / `watch` 签名与事件面不变 |
| 前端 `packages/desktop/src/**` | 零改动 | 无新类型、无新视图；prompt 文本不投影到前端 |
