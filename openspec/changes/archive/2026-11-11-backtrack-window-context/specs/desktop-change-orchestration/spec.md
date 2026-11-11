# desktop-change-orchestration Specification（delta: backtrack-window-context）

## MODIFIED Requirements

### Requirement: 相位 prompt 组装面静态化（静态主体 + 动态 append）

executor / evaluator 会话 prompt SHALL 以写面下发的静态角色知识文本（desktop-phase-prompts / desktop-workflow-write-face）为主体；动态面 SHALL 全部 append 而非模板替换：上下文头一行 `change: <name>`、回溯重做窗口段（见「决策协议」与 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」）、static-check / 测试修复失败注入、verdict 解析失败修复注入（evaluator 续注）、输出协议附录（evaluator）、decision 三列表（决策会话）。prompt 组装 MUST NOT 使用 `<change>` / `<phase>` 模板替换。段序 SHALL 为「上下文头 → 静态主体 → 回溯重做窗口段 → 失败反馈段（门禁 / verdict 修复注入）→ 输出协议附录（evaluator）」：窗口段是失败反馈段的上位语境（前者答「为什么这个相位被叫回来」，后者答「本相位这次错在哪」），MUST NOT 倒置于反馈段之后；单行「回溯原因」形式已退役（窗口段完全取代，单点渲染）。窗口段的条目数与裁剪完全由写面下发的集合决定（写面常量 `MAX_BACKTRACK_WINDOWS`：≤ 该上限时全量并集、超限取创建序末位两层；双分支完整表述见「决策协议」与 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」），orchestration 侧 MUST NOT 二次裁剪、亦不定义该常量。`WorkerTurnRequest.provenance.source_ref = <change>/<phase>/<role>/<attempt>` 与 `root` / `model_level` 结构化通道不变——phase / attempt 盖戳不依赖 prompt 文本。

#### Scenario: 上下文头在场

- **WHEN** walker 发起 executor / evaluator 会话
- **THEN** prompt 含一行 `change: <name>`（change 名自写面记录 name 供给）
- **AND** prompt 主体为静态角色知识文本，无 `<change>` / `<phase>` 模板替换痕迹

#### Scenario: 反馈注入仍 append

- **WHEN** static-check / test-execution 失败或 verdict 解析失败走定向反馈边
- **THEN** 修复反馈文本以 append 注入同一 agent 会话 prompt（不重写静态主体）——门禁步注入 executor 会话、verdict 门注入 evaluator 会话
- **AND** 回溯场景改为 append 回溯重做窗口段（单行回溯原因行零残留）

#### Scenario: 段序窗口段先于反馈段

- **WHEN** walker 为一个既落在回溯窗口内、又带 fail 重试反馈的相位组装 evaluator prompt
- **THEN** 序为「上下文头 → 静态主体 → 回溯重做窗口段 → 失败反馈段 → 输出协议附录」；窗口段索引小于失败反馈段索引，输出协议附录仍在最后

### Requirement: 决策协议

写面 `phase_next` 返回决策分叉触发（任意相位最新 fail 条目未挂决策会话；或窗口 fail 数达重试上限的标注变体）时，walker SHALL 唤起决策 agent（compose_turn 新会话，`ModelLevel::High`）；walker 自走同相位重跑 SHALL 退役（盲目重跑的会话消耗高于一次决策）。确定性相位（initialize / archive：executor / evaluator 双缺席）不触发决策，fail 落账照常同相位重路由。决策 agent 输入 SHALL 有界且来自 db 状态与写面响应：fail checklist（fail 项 + evidence）、失败条目 `report`（失败报告原文，见「失败升格落账与决策信息面」）、`allowed_backtrack_phases`（写面 `phase_next` 白名单；每项 SHALL 携相位特性描述与该相位声明的可操作产物清单，描述 / 产物单源 = 本期相位表 `PhaseDefinition.description` / `PhaseDefinition.artifact_manifest`，随表分派不串味）、候选相位最近一次 eval report、`backtrack_window`（回溯重做窗口集合，透传自写面 error 响应——每项携发起相位 / 目标相位 / 两个泳道序 / 原因原文；walker MUST NOT 自读 store、MUST NOT 自相位表二次推导，判据单源见 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」）。决策 prompt 白名单段 SHALL 逐条渲染 `- <相位 id>（<特性描述>）：<可操作产物段>`，供决策 agent 判断回溯目标（回溯即重做该相位产物；描述空串退化为 id-only 行，零声明产物渲染「无声明产物」）。决策输入 MUST NOT 携相位 retry 语义标记（retry 已退役，无「评估-only / 带 executor」二分支指引）。决策输出 SHALL 为封闭集三动作：

- `{ action: "backtrack", to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；目标为失败相位自身即「重做本相位」（仅当该相位在白名单内）；
- `{ action: "stop", reason }` — 终止 run；
- `{ action: "ask", question, options[] }` — 无法裁决，UI 中断提问。

未在封闭集内的动作（含已退役的 `retry`）SHALL 视为模型漂移，走「决策会话软拒环与漂移上限」的软拒路径，MUST NOT 一次即打死 run。重做语义 SHALL 由白名单自含承载（单一载体）：`allowed_backtrack_phases` 含失败相位自身 ⟺ 该相位可重做（其 step 序列含 agent 步）；零 agent 步的相位（test-execution / archive）自身不入白名单——决策上下文里不存在「重做本相位」这个选项。

决策 prompt SHALL 增「回溯重做窗口」段：段内容 SHALL 复用写面渲染单点（MUST NOT 在 orchestration 侧另持第二份文案或第二份判据），只渲染各窗口的原因原文、以 `<backtrack reason>` / `</backtrack>` 定界、零相位 id 与零属性；集合为空时整段省略（不留标题）。段位置 SHALL 在 `## 失败相位报告` **之前**（段序最前，紧跟开头一句「相位…评估失败，需要你做出路由决策」之后）——背景信息比失败明细更宏观。该段与「可回溯白名单」段不构成语义混淆：白名单供 agent 选目标（携相位 id 与产物、有硬约束力），窗口段供 agent 知道为什么被叫回来（零相位 id、零约束力）。

段内条目数 SHALL 由写面下发的集合决定，且该集合的规模语义 MUST 写成双分支（与写面同一双分支，MUST NOT 只写其中一支）：**未超过写面上限（`MAX_BACKTRACK_WINDOWS`，0 / 1 / 2 层）时全量并集**（0 层即整段省略，1 / 2 层全部渲染，多窗互不暂停互不覆盖）；**超过上限时取创建序末位两层**，被裁的恒为创建序最早那条（等价于该组中 `from_index` 最大者；「最旧最宽」只是其常见口语后果）。越限裁剪的**唯一实现点**在写面「回溯重做窗口推导与 prompt 携带」——本侧对传入集合逐条渲染，MUST NOT 自行实现一次上限裁剪（第二套裁剪会与写面的并集语义分叉）；该常量定义在写面 `phase_table.rs`，本侧只引用其语义、不另行定义。

决策面携带窗口的收益 SHALL 如实记为**语义级**而非结构级：窗口段只渲染原因原文（不含 `from` / `to`），故「同一原因再次回跳同一目标」只能靠原因文本的语义理解识别，MUST NOT 宣称其与「渲染 `from` / `to` 的结构判据」等价。该降级已接受（现状决策 prompt 零历史回溯信息，弱化后仍显著优于现状；深层重复回跳仍由 `MAX_ROUNDS=20` 兜底）；升级路径为渲染 fn 单点加回 `from` / `to`，结构体与判据零改动。

决策决议的兑现 SHALL 依赖「每个 fail 条目只触发一次决策」语义：决策会话挂账（decision_session_id）后的同一条目 MUST NOT 重复触发决策分叉（该条目已进入过决策，不再重复分叉）。唯一合法重入路径 SHALL 为决策 backtrack——写面改写最新条目 `backtrack_to` → 下一轮 `phase_next` 的回溯分支先手接管路由（走不到两道分叉判断）→ 目标相位真执行 → 新落账 fail 条目无决策标记，触发点对新条目照常生效。MUST NOT 以「放行同相位重跑」作为兑现面：删掉该幂等闸后，fail 过的相位在重做 pass 后仍会因最新 fail 条目再次分叉、相位永不执行，直烧到 `MAX_ROUNDS=20` 硬失败。

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验（工具侧兜底）：越权目标 SHALL 被拒绝且不落任何越权回跳账（状态库零变更）。决策输出解析失败 / 越权由「决策会话软拒环与漂移上限」处置（软拒纠正、漂移超限 Terminal）。

#### Scenario: 白名单条目携相位特性描述与可操作产物

- **WHEN** 决策 prompt 渲染可回溯白名单
- **THEN** 完整决策 prompt 的每条白名单行形如 `- <相位 id>（<相位特性描述>）：<可操作产物段>`（描述取本期相位表 `PhaseDefinition.description`、产物取 `PhaseDefinition.artifact_manifest`，随 workflow_type 分派——requirement / test-only 同名相位描述与产物不串味），产物段逐项渲染 `` `<相对路径>`（<产物标题>） ``、多项以「、」分隔，相位零声明产物时显式渲染「无声明产物」；评估 checklist MUST NOT 出现在产物段（PhaseRecord 投影、非相位声明产物），失败相位自身条目在描述后追加「（= 重做本相位）」；软拒纠正 prompt 以 `id（描述） / id（描述）` 一行重申同一组目标（纠正面只重申目标、不展开产物段）

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


### Requirement: 失败升格落账与决策信息面

失败升格落账（test-execution / mutation-test 首败及此后每次失败）SHALL 写一条 fail PhaseRecord：`report` 携完整失败明细（test-execution = `findings_detail`、mutation-test = `feedback_text`，上限 `MAX_REPORT_CHARS = 2000` 字符，超长截断留省略号），`checklist` 恰一条 `pass=false`，evidence 为诊断摘要（`diagnose_brief` 口径）+ 报告目录，executor / evaluator / decision 三会话槽位恒 `None`（本相位零会话；决策会话槽位由 `decision-log` 另行挂账）。决策有界输入 `DecisionInput` SHALL 增 `fail_report: String`（取失败条目 `report` 原文），并保留 fail checklist（fail 项 + evidence）、`allowed`（白名单，项 = 相位 id + 特性描述）、`candidates`（候选相位最近一次 eval report）、`backtrack_window`（回溯重做窗口集合，透传自 error 响应；渲染与判据归「决策协议」与 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」，本条只登记它是决策输入的第五个有界面）；`eval_only` 字段 SHALL 删除（retry 退役后无消费者）。决策 prompt SHALL 增「失败相位报告」段（渲染 `fail_report`），并 SHALL 明示决策 agent 可用 `read` 工具读取报告文件（test-execution 的 `reports/test/summary.json`、突变的段级 / 归并报告）取全量明细。决策输入 MUST NOT 内联全量 findings / 存活突变体清单（有界：报告文本 ≤2000 字符 + checklist 项 + 候选 report）。

#### Scenario: 决策输入携完整失败报告

- **WHEN** test-execution 升格落账后唤起决策会话（findings 明细 1500 字符）
- **THEN** `DecisionInput.fail_report` 为该明细原文、决策 prompt 含「失败相位报告」段与读报告文件指引；`DecisionInput` 无 `eval_only` 字段

#### Scenario: 决策输入有界

- **WHEN** 失败明细超 2000 字符
- **THEN** 落账 `report` 与 `DecisionInput.fail_report` 均按 2000 字符截断（省略号留痕），决策 prompt 不含全量 findings 数组 / 存活突变体清单

#### Scenario: 报告目录可回查

- **WHEN** 决策 agent 依指引 `read` 报告树
- **THEN** evidence 中的报告目录路径可解析到 `summary.json` 与逐 suite / 段级报告（升格落账与 runner 落盘同锚，路径零改写）

#### Scenario: 决策输入窗口字段与既有面并列

- **WHEN** 升格落账后唤起决策会话且失败相位落在开放窗口内
- **THEN** `DecisionInput` 同时携 fail checklist / `fail_report` / `allowed` / `candidates` / `backtrack_window` 五面；新增字段不改变其余四面的取值与有界纪律（报告 ≤2000 字符、候选 report 不内联全量 findings）


## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/orchestration/src/decision.rs`（扩） | 决策有界输入 | `DecisionInput` 增 `pub backtrack_window: Vec<workflow::write::BacktrackWindowEntry>`（字段 doc 注明透传自 error 响应、walker 不自行推导）；`DecisionAction` 三动作封闭集 / `parse_decision` / `ensure_backtrack_allowed` / `MAX_REASON_CHARS = 500` 逐字不变 |
| `crates/core/orchestration/src/prompt.rs`（扩） | 决策 prompt 组装 | `decision_prompt` 增「回溯重做窗口」段：调用写面窗口段渲染单点（零第二份文案 / 零第二份判据 / 零二次裁剪——上限裁剪唯一实现在写面），逐条渲染写面下发集合（≤ 上限时全量并集、超限时写面已取末位两层），空集合整段省略，插入位置 = `## 失败相位报告` 之前；`spec_sync_prompt` / `merge_conflict_prompt`（archive 两步）MUST NOT 携带该段；`decision_fix_prompt` 文本不变 |
| `crates/core/orchestration/src/walker.rs`（收窄） | 决策输入透传 | `build_decision_input` 透传 `next.backtrack_window`（零 store 读取、零相位表推导）；`resolve_deadlock` 调用点随行；`step_executor` doc 注释同步（不再写「回溯原因后缀全在写面」）。零新 port / 零新 step / 零 route 规则 |
| `crates/core/orchestration/src/{port.rs,steps.rs,control.rs,state.rs}`（零改动） | 工具步与状态面 | `ToolStepOutput::PhaseNext(Box<PhaseNextOutcome>)` 载荷自动携新字段；零新 port、零新步词、零新通知类型、零新状态值 |
| `crates/core/orchestration/src/prompt_test.rs` | 决策 prompt 语料 | 窗口段多条按创建序渲染 / 空集合整段省略 / 段位置先于失败相位报告 / 零相位 id 泄漏 / 2 条与 3 条入参均逐条渲染（证明本侧零二次裁剪）；`base_input()` 与两处完整构造器补字段（其余 `..base_input()` 自动随动） |
| `crates/core/orchestration/src/{port_test.rs,walker_test.rs,decision_test.rs}` | 结构体字面量随动 | `PhaseNextOutcome` 6 处 / `DecisionInput` 完整构造器 3 处补字段（编译器兜住，零运行期静默） |
| `crates/core/workflow/src/write/phase_next.rs` + `phase_table.rs`（见 desktop-workflow-write-face） | 窗口判据与文案单源 | 本能力只消费写面下发的 `backtrack_window` 与渲染单点，零判据副本 |
