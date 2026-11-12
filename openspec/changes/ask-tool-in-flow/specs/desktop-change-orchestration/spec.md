# desktop-change-orchestration Specification (Delta)

## REMOVED Requirements

### Requirement: 决策会话软拒环与漂移上限

**Reason**：该机制的两条前提已分别消失——① 决策输出的协议校验已改由 `finish_step` 的工具校验**当场回灌**（`is_error` ToolResult，同一 turn 内模型可立即纠正；`parse_decision` 与 `DECISION_DRIFT_LIMIT` 随之删除），跨轮软拒计数无消费者；② `ask` 自本变更起迁出为独立阻塞工具 `AskUserQuestion`（见新增 requirement「流程内 ask 停等与双入口应答」），「ask 轮不计漂移」这一条款的载体不复存在。保留该 requirement 会与「决策协议」条形成两套相互矛盾的纠错语义（软拒两次 vs 一次即停）。**本条是收口协议工具化那次变更已声明的退役面在主基线里的滞后残留**（`openspec/specs/` 尚未吸收该 delta），故本条 REMOVED 与该 delta 的同名 REMOVED 语义等价：若两份 delta 同批合并，后到的一侧为幂等空操作；无论如何合并后本 requirement MUST NOT 以任何形态恢复。

**Migration**：
- **白名单越权**：改由 walker 的 `ensure_backtrack_allowed` **一次即** `Terminal::failed`（文案携 `allowed` 列表；自环文案提示零 agent 步相位不支持重做），并以 failed 的 `WhitelistGate` 节点留图面证据。越权属违反**双处明示**的约束（`decision_prompt` 的白名单段 + `finish_step` decision 形态的 description）。
- **形状漂移 / 未知动作**：由 `finish_step` 的 decision 形态 schema 拒绝（`action` 枚举不含未知值），`Err` 当场回灌，模型在同一 turn 内重试。
- **`ask` 停等**：迁出为 `AskUserQuestion` 工具（`ToolExec::Ask(Flow)`），在 turn 内阻塞、应答后同一 turn 继续，不再消耗一整轮、不再 `Continue` 重开。
- **删除面**：`parse_decision`、`DECISION_DRIFT_LIMIT`、`decision_fix_prompt` 与软拒环的续注分支（`grep` 零命中为判据）；`ensure_backtrack_allowed` 与 `WhitelistGate` 每轮一节点的图面语义逐字保留。

#### Scenario: 软拒环与漂移计数零残留

- **WHEN** 审查合并后的本 spec 与源码
- **THEN** 本 requirement 的正文只以本 REMOVED 块的 Reason / Migration 形态存在（合并时整块移除，不进主基线 requirements）；`parse_decision` / `DECISION_DRIFT_LIMIT` / `decision_fix_prompt` 在产品源码零命中；`ensure_backtrack_allowed` 的越权判定仍一次即 `Terminal::failed` 并留 failed `WhitelistGate` 节点

## MODIFIED Requirements

### Requirement: 决策协议

写面 `phase_next` 返回决策分叉触发（任意相位最新 fail 条目未挂决策会话；或窗口 fail 数达重试上限的标注变体）时，walker SHALL 唤起决策 agent（compose_turn 新会话，`ModelLevel::High`）；walker 自走同相位重跑 SHALL 退役（盲目重跑的会话消耗高于一次决策）。确定性相位（initialize / archive：executor / evaluator 双缺席）不触发决策，fail 落账照常同相位重路由。决策 agent 输入 SHALL 有界且来自 db 状态与写面响应：fail checklist（fail 项 + evidence）、失败条目 `report`（失败报告原文，见「失败升格落账与决策信息面」）、`allowed_backtrack_phases`（写面 `phase_next` 白名单，**随分叉错误响应下发**——walker MUST NOT 跨轮缓存（run 首轮即分叉时无「上一站」可缓存），MUST NOT 自相位表推导；每项 SHALL 携相位特性描述与该相位声明的可操作产物清单，描述 / 产物单源 = 本期相位表 `PhaseDefinition.description` / `PhaseDefinition.artifact_manifest`，随表分派不串味）、候选相位最近一次 eval report、`backtrack_window`（回溯重做窗口集合，透传自写面 error 响应——每项携发起相位 / 目标相位 / 两个泳道序 / 原因原文；walker MUST NOT 自读 store、MUST NOT 自相位表二次推导，判据单源见 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」）。决策 prompt 白名单段 SHALL 逐条渲染 `- <相位 id>（<特性描述>）：<可操作产物段>`，供决策 agent 判断回溯目标（回溯即重做该相位产物；描述空串退化为 id-only 行，零声明产物渲染「无声明产物」）。决策输入 MUST NOT 携相位 retry 语义标记（retry 已退役，无「评估-only / 带 executor」二分支指引）。决策输出 SHALL 为**经 `finish_step` 工具提交**的封闭集**两动作**（本变更把 `ask` 迁出为独立工具，`ask` 不再是决策动作）：

- `{ action: "backtrack", to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；目标为失败相位自身即「重做本相位」（仅当该相位在白名单内）；
- `{ action: "stop", reason }` — 终止 run。

**无法裁决时 SHALL 调用 `ask` 工具**（`AskUserQuestion`，见 desktop-agent-execution「ask 工具协议与阻塞应答通道」与本文「流程内 ask 停等与双入口应答」）——该工具在同一 turn 内阻塞、应答后模型继续，MUST NOT 以决策动作的形式提交提问（`finish_step` 的 decision 形态 `enum` MUST NOT 含 `ask`，`question` / `options` 两个 property 随之删除）。`finish_step` 的 decision 形态 `description` SHALL 明示该指路（两动作 + 「无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）」）。

封闭集外的动作（含已退役的 `retry` 与本变更迁出的 `ask`）与形状漂移 SHALL 由 `finish_step` 的工具校验 `Err` **当场回灌**（is_error ToolResult，模型在同一 turn 内纠正重试）——MUST NOT 一次即打死 run，MUST NOT 跨轮计数。`DecisionAction::Ask` 变体 SHALL 保留（历史 `SystemNotice{finish_step}` 载荷反序列化兼容 + walker 防御臂判据），但**生产路径不可达**（schema 不再产出它）；若该载荷仍流出，walker SHALL `Terminal::failed`（文案指路 ask 工具），MUST NOT 静默忽略、MUST NOT 进入停等。

重做语义 SHALL 由白名单自含承载（单一载体）：`allowed_backtrack_phases` 含失败相位自身 ⟺ 该相位可重做（其 step 序列含 agent 步）；零 agent 步的相位（test-execution / archive）自身不入白名单——决策上下文里不存在「重做本相位」这个选项。

决策 prompt SHALL 增「回溯重做窗口」段：段内容 SHALL 复用写面渲染单点（MUST NOT 在 orchestration 侧另持第二份文案或第二份判据），只渲染各窗口的原因原文、以 `<backtrack reason>` / `</backtrack>` 定界、零相位 id 与零属性；集合为空时整段省略（不留标题）。段位置 SHALL 在 `## 失败相位报告` **之前**（段序最前，紧跟开头一句「相位…评估失败，需要你做出路由决策」之后）——背景信息比失败明细更宏观。该段与「可回溯白名单」段不构成语义混淆：白名单供 agent 选目标（携相位 id 与产物、有硬约束力），窗口段供 agent 知道为什么被叫回来（零相位 id、零约束力）。

段内条目数 SHALL 由写面下发的集合决定，且该集合的规模语义 MUST 写成双分支（与写面同一双分支，MUST NOT 只写其中一支）：**未超过写面上限（`MAX_BACKTRACK_WINDOWS`，0 / 1 / 2 层）时全量并集**（0 层即整段省略，1 / 2 层全部渲染，多窗互不暂停互不覆盖）；**超过上限时取创建序末位两层**，被裁的恒为创建序最早那条（等价于该组中 `from_index` 最大者；「最旧最宽」只是其常见口语后果）。越限裁剪的**唯一实现点**在写面「回溯重做窗口推导与 prompt 携带」——本侧对传入集合逐条渲染，MUST NOT 自行实现一次上限裁剪（第二套裁剪会与写面的并集语义分叉）；该常量定义在写面 `phase_table.rs`，本侧只引用其语义、不另行定义。

决策面携带窗口的收益 SHALL 如实记为**语义级**而非结构级：窗口段只渲染原因原文（不含 `from` / `to`），故「同一原因再次回跳同一目标」只能靠原因文本的语义理解识别，MUST NOT 宣称其与「渲染 `from` / `to` 的结构判据」等价。该降级已接受；升级路径为渲染 fn 单点加回 `from` / `to`，结构体与判据零改动。

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
- **THEN** `WhitelistGate` 节点呈 failed（detail 携 `allowed` 列表）、run 以 `Terminal::failed` 收口（零软拒续注）；写面 `backtrack` 的二次校验对越权目标仍显式拒绝且状态库零变更

#### Scenario: ask 不再是决策动作

- **WHEN** 决策 agent 尝试经 `finish_step` 提交 `{action:"ask", question, options}`
- **THEN** 工具校验 `Err` 当场回灌（`action` 枚举只含 `backtrack` / `stop`，文案指路调用 `AskUserQuestion` 工具），run 不中断、不进入停等；同一 turn 内模型改为调用 ask 工具后问答照常成立

#### Scenario: ask 载荷仍流出即判漂移

- **WHEN** 因任何原因此轮决策仍产出 `FinishPayload::Decision(DecisionAction::Ask { .. })`（历史载荷回放 / 装配漂移）
- **THEN** walker 以 `Terminal::failed` 收口，reason 指路「ask 应经 `AskUserQuestion` 工具提交」；MUST NOT emit `RunUpdate::Ask`、MUST NOT 进入停等、MUST NOT 开新一轮决策会话

#### Scenario: 决策 ask 往返不再消耗整轮

- **WHEN** 决策会话中模型调用 `AskUserQuestion` 并获用户应答
- **THEN** 该决策 turn 继续（零 `Continue` 重开、零 `decision_answer_prompt`），模型随后在同一 turn 内以 `finish_step` 提交 `backtrack` 或 `stop`；决策会话数 1

#### Scenario: verdict fail 分叉与重做自含

- **WHEN** code-review#1 落 verdict=fail（评估-only 相位，自身在白名单内）且决策 agent 经 `finish_step` 提交 `{action:"backtrack", to:"code-review", reason:...}`
- **THEN** walker 唤起决策 agent 恰一次，回溯落账后同相位以新 attempt 真执行（重做本相位由白名单自含表达，`retry` 动作不存在）；重跑再落新 fail 条目时决策分叉对新条目照常触发。带 executor 相位（如 implement）verdict fail 同式分叉，重做携带失败清单段

#### Scenario: verdict fail 分叉与 retry 兑现

- **WHEN** 审查决策协议的兑现路径（本条为历史 scenario 名的存续面）
- **THEN** 原「`retry` 决议兑现」路径已退役（`DecisionAction::Retry` 删除、未知名由工具 Err 当场回灌），兑现语义改为「每个 fail 条目只触发一次决策」幂等闸 + 决策 backtrack 重入；重做本相位经回溯到自身兑现（见「verdict fail 分叉与重做自含」与「已决条目不重复分叉」）

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

### Requirement: 运行控制命令面

命令面 SHALL 提供 change run 的控制入口，命令体收拢为编排运行时公共 API 的薄包装（沿三件事纪律）：

- **发起**：按 change id 发起 run（id → 记录解析 worktree / name），`auto_next_phase` 参数（bool；false=默认停等节奏，true=自动确认模式）与「按指定 phase 停止」参数（形态 design 定稿；archive 为固定停止点）随发起定格并透传 walker；发起 SHALL 先解析 exec root；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），RunRecord 起始行（status=running）SHALL 随发起建立；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态（archive 相位进行中的停止复用本命令）；对非运行态目标幂等忽略；
- **应答**：ask 中断态下回传用户应答——**通道唯一**：命令 SHALL 经 run 的当前会话 id（`current_session(root, change_id)`）转投会话级 ask 注册表（`AskRegistry`，见 desktop-agent-execution「ask 工具协议与阻塞应答通道」），MUST NOT 维护第二套应答通道（原 `answer_tx` 槽与 `wait_answer()` 退役，`grep` 零命中）。无运行 run / 无当前会话 / 无等待方 SHALL 显式 `Err`（前者文案携 change id，后两者含「无等待中的 ask」）；命令层 MUST NOT 清理停等态（清理单点在 ask 执行体返回后，见「流程内 ask 停等与双入口应答」）；
- **确认**：phase 间停等点（含 archive 前停等点）的用户确认（继续 / 终止）——受理与清理语义逐字不变：同一把注册表锁内先解除停等态（`status` 回 `running`、`archive` 载荷槽同刻清空）、再向等待方投递并广播失效通知，解除条件为「当前 status 恰为该停等态」。

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

#### Scenario: ask 应答经会话 id 单通道转投

- **WHEN** 命令层受理一次 ask 应答
- **THEN** 命令先取该 run 的当前会话 id（无会话 → `Err`），再向 ask 注册表投递该 id 与应答文本；投递成功即 `Ok`，等待方收到原文；命令体内零注册表条目操作、零通知广播、零停等态清理（清理由 ask 执行体返回后单点执行）

#### Scenario: 重复应答与迟到应答显式失败

- **WHEN** 同一 ask 被应答两次，或 run 已收口后补答
- **THEN** 第二次与迟到应答均返回显式 `Err`（「当前无等待中的 ask」类文案），MUST NOT 幂等 `Ok`、MUST NOT 改写任何会话状态

#### Scenario: 停等态解除与 confirm 侧不变

- **WHEN** 一次 phase 间停等确认被受理
- **THEN** 与本次变更前逐字一致：同一把锁内先解除停等态（`status` 回 `running`、`archive` 槽清空）、再投递并广播失效通知；解除条件为「当前 status 恰为该停等态」，`Finished` 已先到时终态 MUST NOT 被倒灌回运行中；失效通知复用 `RunNotice::Step`（`Ask` / `ConfirmWait` 在通知面表示「新停等开始」，MUST NOT 用作解除信号）

### Requirement: 相位 prompt 组装面静态化（静态主体 + 动态 append）

executor / evaluator / decision 会话 prompt SHALL 以写面下发的静态角色知识文本（desktop-phase-prompts / desktop-workflow-write-face）为主体；动态面 SHALL 全部 append 而非模板替换：上下文头一行 `change: <name>`、回溯重做窗口段（见「决策协议」）、static-check / 测试修复失败注入、verdict 解析失败修复注入（evaluator 续注）、输出协议附录（evaluator）、decision 两动作与有界输入段（决策会话）与一句「无法裁决时调用 `AskUserQuestion` 工具」指路。prompt 组装 MUST NOT 使用 `<change>` / `<phase>` 模板替换。段序 SHALL 为「上下文头 → 静态主体 → 回溯重做窗口段 → 失败反馈段（门禁 / verdict 修复注入）→ 输出协议附录（evaluator / decision）」：窗口段是失败反馈段的上位语境（前者答「为什么这个相位被叫回来」，后者答「本相位这次错在哪」），MUST NOT 倒置于反馈段之后；单行「回溯原因」形式已退役（窗口段完全取代，单点渲染）。窗口段的条目数与裁剪完全由写面下发的集合决定（写面常量 `MAX_BACKTRACK_WINDOWS`：≤ 该上限时全量并集、超限取创建序末位两层；双分支完整表述见「决策协议」与 desktop-workflow-write-face「回溯重做窗口推导与 prompt 携带」），orchestration 侧 MUST NOT 二次裁剪、亦不定义该常量。

**协议形状不进 prompt 主体**：`finish_step` 与 `AskUserQuestion` 的 description / parameters 形状 SHALL 由**工具挂载面**承载（挂载清单 + 动态 schema），MUST NOT 作为 prompt 正文重复一遍——prompt 侧只留「何时收口 / 何时提问」的各一句指引与禁调写通道红线。verdict 解析失败修复注入 SHALL 退役（工具 Err 文案是唯一反馈面），`decision_answer_prompt`（ask 应答的唯一再提示面）SHALL 随 ask 停等内移一并退役（应答不再开新一轮，`grep` 零命中）。`WorkerTurnRequest.provenance.source_ref = <change>/<phase>/<role>/<attempt>` 与 `root` / `model_level` 结构化通道不变——phase / attempt 盖戳不依赖 prompt 文本。

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

#### Scenario: 决策 prompt 指路 ask 工具而非承载其形状

- **WHEN** 审查决策会话 prompt 文本与工具 definitions
- **THEN** prompt 只含两动作说明与一句「无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）」；`question` / `options` 的字段形状只出现在该工具的 parameters 中；`decision_answer_prompt` 全仓零命中

## ADDED Requirements

### Requirement: 流程内 ask 停等与双入口应答

change 流程的 ask SHALL 由**阻塞工具** `AskUserQuestion`（`ToolExec::Ask(AskMode::Flow)`，同名覆盖通用档，见 desktop-agent-execution「ask 工具协议与阻塞应答通道」）承载，流程侧只额外负责编排状态广播与停等态清理：

- **工具契约单源**：工具名 / `description` / `parameters` / `validate` SHALL 住 `crates/core/orchestration/src/ask.rs`（与 `finish.rs` 并列的模块），`AskQuestion { question, options }` 为校验产物；`AskUserQuestion` 与 `finish_step` MUST NOT 互相引用——**`ask.rs` 全文 MUST NOT 出现 `finish` 字样**，`finish.rs` 中的 `ask` MUST NOT 出现在代码路径（只允许出现在「无法裁决时调用 `AskUserQuestion`」的文案字符串里）；两模块的挂载也互不依赖（`ask` 位与 `finish` 位独立置位）。该解耦 SHALL 可机械验证（源码自证用例 + `grep` 判据）。
- **`AskPort`（编排侧状态面）**：SHALL 定义 `ask_started(&self, ask: AskPayload)` 与 `ask_resolved(&self)` 两方法（实现驻壳层 `ChangeFlowSink`）；`ask_started` 的载荷 SHALL 为完整 `AskPayload`（含 `question` / `options` / `tool_call_id` / `session_id`），`ask_resolved` 无载荷（解除信号只表达「本条 ask 结束」）。
- **执行体序列（`Flow` 档）**：`ask_started`（携本 tool_use 的 id 与 core session id）→ 登记 ask 注册表 → `select!` 阻塞（应答 / 停止两臂）→ **两臂返回后都** `ask_resolved`。停等态清理（`status` 回 `running`、`ask` 载荷槽清空、广播 `RunNotice::Step`）SHALL 全部落在 `ask_resolved` 的实现里（单点），MUST NOT 由应答命令承担——两入口（`change_flow_answer` / `agent_answer`）因此天然幂等（先到者胜），且清理与「应答从哪来」解耦。
- **双端同步呈现**：`AskPayload` SHALL 扩 `tool_call_id: Option<String>`（行内卡对账：本次 ask 对应哪个 `tool_use`）与 `session_id: Option<String>`（转录定位：提问属哪条会话）两 **additive** 字段（`#[serde(default)]`，旧线缺字段读 `None`）；`RunUpdate::Ask` SHALL 收敛为单载荷形态 `Ask { ask: AskPayload }`（载荷单源——MUST NOT 在变体字段与载荷类型里各存一份同义字段）；`RunNotice` 六变体与 `kind()` 投影 MUST NOT 变更（通知面仍 kind-only）。详情页 `AskCard` 与聊天窗 ask 活卡 SHALL 读**同一** `AskPayload`：两处任一入口应答后两处 SHALL 同步收起、`activeRun.status` SHALL 回 `running`。
- **失效信号**：停等解除 SHALL 广播 `RunNotice::Step`（既有「状态已变、请重查」通用 kind）；`Ask` / `ConfirmWait` 在通知面被读作「新停等已开始」，MUST NOT 用作解除信号（重放会让订阅方对已解除的等待再答一次）。
- **清理时点与终态保护**：清理 SHALL 仅在「当前 `status` 恰为 `WaitingAsk`」时执行——`Finished` 已先到时终态 MUST NOT 被倒灌回 `running`；停止 / 取消路径（`select!` 的停止臂）同样 SHALL 执行 `ask_resolved`（外层泵的 drop 不执行清理代码，故显式臂是唯一落点）。
- **`ask` 槽的必清契约**：应答后 `ask` 载荷槽 MUST NOT 残留（行内活的判据是「该 `tool_call_id` 仍在等待」，残留会让卡片永远判为「正在等」）。该契约由前置修复提供（应答受理即清）并在本变更下改写为「执行体返回即清」，两形态对前端等价。

#### Scenario: 流程 ask 走工具而非决策动作

- **WHEN** change 流程某决策会话需要用户裁决
- **THEN** 模型调用 `AskUserQuestion`（与通用会话同一工具名与 schema）；`run` 在同一 turn 内阻塞，`activeRun.status` 切到等待用户决策态、`activeRun.ask` 携 `question` / `options` / `toolCallId` / `sessionId`；决策会话 MUST NOT 因此收口

#### Scenario: 双入口应答幂等

- **WHEN** 用户分别经详情页 `AskCard`（`change_flow_answer`）与聊天窗活卡（`agent_answer`）应答同一 ask
- **THEN** 先到的一路投递成功、等待方收到该原文并继续；后到的一路返回 `Err`（「当前无等待中的 ask」）；两处卡片同步收起、`activeRun.status` 回 `running`、`ask` 槽清空，MUST NOT 出现第二次回灌

#### Scenario: 清理单点在执行体

- **WHEN** 审查 `Flow` 档 ask 执行体的返回路径与应答命令实现
- **THEN** `ask_resolved` 在应答臂与停止臂返回后各执行一次（respond 路径 `grep` 计数恰 1）；应答命令体内零状态清理、零通知广播；`ChangeFlowControl` 无 `answer_tx` 槽、无 `wait_answer()`（`grep` 零命中）

#### Scenario: 停止期间停等态不残留

- **WHEN** ask 阻塞期间用户停止 run
- **THEN** `select!` 停止臂返回 → `ask_resolved` 执行 → 停等态解除（`status` 回到运行态或随 run 收口为 stopped，`ask` 槽清空）；`AskRegistry` 该会话槽由轮收敛除名兜底（其后 `answer` → `Err`）

#### Scenario: 终态不被倒灌

- **WHEN** `Finished` 通知先于 `ask_resolved` 到达（收口与清理的竞态窗口）
- **THEN** 清理的「当前 status 恰为等待态」条件不成立、`status` 保持终态；`ask` 载荷槽仍被清空

#### Scenario: 与 finish_step 解耦

- **WHEN** 审查 `orchestration/ask.rs` 与 `orchestration/finish.rs`
- **THEN** `ask.rs` 全文零 `finish` 字样（无 `use crate::finish`、无共享类型）；`finish.rs` 的 `ask` 只出现在 description 的文案字符串；挂载面两条追加分支互不引用；`finish = None` 的会话仍下发 `AskUserQuestion`、`ask = None` 的会话仍下发 `finish_step`（若 `finish` 位在）

#### Scenario: 通知面 kind 零变更

- **WHEN** 审查 `RunUpdate` / `RunNotice` 的变体与投影
- **THEN** `RunUpdate::Ask` 载荷为单个 `AskPayload`；`RunNotice` 仍六变体且 `kind()` 逐字不变（`ask` 串不变）；失效信号仍为 `RunNotice::Step`

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/orchestration/src/ask.rs`（新） | ask 工具定义与校验单点 | `pub const TOOL_NAME: &str = "AskUserQuestion"`；`pub fn description() -> &'static str`；`pub fn parameters() -> serde_json::Value`（`question` 必填、`options` `minItems: 1`）；`pub fn validate(args: &Value) -> Result<AskQuestion, String>`（question 非空 / options 非空字符串数组，Err 文案自足）；`pub struct AskQuestion { pub question: String, pub options: Vec<String> }`。**MUST NOT `use crate::finish`**（全文零 `finish` 字样，源码自证守卫） |
| `crates/core/orchestration/src/finish.rs`（收窄） | decision 形态两动作 | `parameters(FinishSpec::Decision)` 的 `action.enum` == `["backtrack","stop"]`，删除 `question` / `options` 两个 property；`description(Decision)` 改写为两动作 + 「无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）」；`validate` 仍走 `validate_decision`；evaluator 形态与 `FinishPayload` 逐字不变 |
| `crates/core/orchestration/src/decision.rs`（收窄 + 保留面） | 动作封闭集与历史兼容 | `DecisionAction::Ask` **保留**（doc 补「生产不可达：`finish_step` schema 已不含该值，历史 notice 反序列化仍可读，walker 防御臂据此判漂移」）；`validate_decision` 的 `"ask"` 臂逐字保留；`ensure_backtrack_allowed` / `MAX_REASON_CHARS` 逐字不变 |
| `crates/core/orchestration/src/prompt.rs`（改） | 决策 prompt 两动作 + ask 指路 | `decision_prompt` 的协议段由三选一改两选一 + 一句 ask 工具指路（形状不复述）；`decision_answer_prompt` **删除**（`grep` 零命中）；`evaluator_protocol` 零改动 |
| `crates/core/orchestration/src/walker.rs`（改） | 决策 ask 分支退役 | `decision_session` 删除 `FinishPayload::Decision(Ask { .. })` 的 `emit` / `wait_answer` / `Continue` 分支，改为单一裁决：该载荷出现即 `Terminal::failed`（指路 ask 工具）；`resolve_deadlock` 的 `DecisionAction::Ask` 防御臂逐字保留；`WhitelistGate` 每轮一节点语义不变 |
| `crates/core/orchestration/src/control.rs`（改 + 保留面） | 应答通道转投与 confirm 不动 | `answer(root, change_id, text)` 改为 `current_session()` → `AskRegistry::answer(session_id, text)`（无 run → `Err` 携 change id；无会话 / 无等待方 → `Err`）；删除 `answer_tx` 槽字段与 `wait_answer()`；**`accept_pending` / `confirm` / `wait_confirm` / 锁序纪律逐字不动**；`publish` 的 `RunUpdate::Ask` 臂随变体收敛改为 `entry.ask = Some(ask)`（其余四臂逐字不动）；`ChangeFlowControl::new(ask_registry: Arc<AskRegistry>)`；新增 `resolve_ask(root, change_id)`（`ask_resolved` 的注册表侧实现：条件清 status + `ask` 槽 + 广播 `RunNotice::Step`） |
| `crates/core/orchestration/src/port.rs`（扩） | 编排侧 ask 状态面 | `pub trait AskPort: Send + Sync { fn ask_started(&self, ask: AskPayload); fn ask_resolved(&self); }` |
| `crates/core/orchestration/src/state.rs`（扩） | 载荷扩展与变体收敛 | `AskPayload` 增 `#[serde(default)] pub tool_call_id: Option<String>` / `pub session_id: Option<String>`（additive）；`RunUpdate::Ask { question, options }` → `Ask { ask: AskPayload }`（`kind()` == `"ask"`、`From<&RunUpdate> for RunNotice` 投影逐字不变） |
| `crates/infra/agent/src/sdk/loop.rs`（扩） | `Flow` 档执行体 | `ToolExec::Ask(AskMode::Flow)` 臂：`port.ask_started(AskPayload { question, options, tool_call_id: Some(id), session_id: Some(core_session_id) })` → `registry.register` → `select!(oneshot, handle.wait_requested())` → `port.ask_resolved()`（两臂都执行）；`port` 缺席 → is_error 回灌 |
| `src-tauri/src/commands/change_flow/mod.rs`（扩） | 状态面实现与命令转投 | `impl AskPort for ChangeFlowSink`（`ask_started` → `control.publish(RunUpdate::Ask { ask })`；`ask_resolved` → `control.resolve_ask(root, change_id)`）；`change_flow_answer_with` 改调 `control.answer`（签名与前端参数面零改动）；`change_flow_start_with` 把 `Arc<AskRegistry>` 与 `Arc<dyn AskPort>`（该 sink 自身）传入 `compose_turn` |
| `packages/desktop/src/components/agent/tool-cards.tsx` + `agent-messages.tsx` + `agent-session-view.tsx`（扩） | 聊天窗 ask 活卡 | `AgentToolCardProps.askAnswer?: { onAnswer: (text: string) => void }`；`AgentMessagesProps.askAnswer?: { onAnswer: (text: string) => void; pendingToolCallId?: string \| null }`；`AgentSessionViewProps.askAnswer?`（自持 / 注入两形态同透传）；未传即全卡静态（现状零变化） |
| `packages/desktop/src/views/changes/flow/{session-transcript-panel,detail-drawer,change-detail-view}.tsx`（扩） | 流程抽屉接线 | `askAnswer = { onAnswer: (text) => ipc.changeFlowAnswer(root, id, text), pendingToolCallId: activeRun?.ask?.toolCallId ?? null }` 逐层透传；`run-control-panel.tsx` 的 `AskCard` 零改动 |
