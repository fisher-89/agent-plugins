# 提案: ask-tool-in-flow

> **变更**: ask-tool-in-flow
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/ask-tool-in-flow/explore.md`（第八轮定稿：A（`AskRegistry` 下沉 `core/agent`）+ Y-1（`change_flow_answer` 只转投 `AskRegistry`，流程状态清理搬到 ask 执行体）已定，第三层「显式禁用」首期只建位不接线）
> **范围接缝**: 本变更只做**流程内 ask 工具化与双端同步应答**：① `core/agent` 落地会话级阻塞应答通道（`AskRegistry`，键 = core session id），② `ask` 成为挂载清单里一项独立阻塞工具（`ToolTier::Protocol`，GENERIC / FLOW 两档执行体），③ `finish_step(Decision)` 的动作封闭集收为两值（`ask` 迁出为独立工具），④ 聊天窗 ask 活卡 + `agent_answer` 通用应答入口。MUST NOT 做：一次 ask 多问题（单问、多次调用）、`options` 可空（已裁决非空，自由文本应答仍保留）、聊天窗 ask 卡的富交互（下拉 / 多选 / 表单）、`questionnaire` 组件接线、ask 与 steer 通道合并、超时自动应答、禁用场景的实际接线（只建 `ask = None` 位与分支）。
> **前序**: 停等态残留修复（commit `6708e14875a4`，bump 0.6.35）已合入并提供硬依赖契约——**`ask` 槽在应答受理时会被清空**；`agent-finalize-tool-for-flow-steps`（commit `8d59f3b8186b`）已合入 `finish_step` 工具协议与工具挂载单源。

---

## 问题

### 现象一：通用会话没有「turn 内等待应答」通道，ask 无处阻塞

`AgentSession` 只有三件套：`observations`（引擎 → 内核）、`questions`（内核 → 引擎，**一轮一命**）、`handle`（终止信号）。`session_pump` 的形态写死了这条约束：

```rust
let Some(question) = questions.recv().await else { return };  // 只收一次
... loop::run(...) ...                                        // 跑完整轮
```

于是「模型问到一半，用户答一句，模型接着说」这个交互在通用会话（explore / archi / debug）里没有落点：`questions` 一次 `recv` 后不再消费，缺少「引擎 → 等待方」的反向注入通道。同时 `desktop-agent-execution` 的边界条款把这件事写成禁令——「SDK 工具面为封闭清单，**无 ask 类工具**，agent 需用户决策时以问题文本作为本轮最终消息收口，应答经 Continue 重开一轮」。

### 现象二：流程侧 ask 仍靠「收口 + 重开一整轮」实现

现状决策会话的 ask 走的是 `finish_step(Decision)` 的第三个枚举值，语义链是：

```
决策会话 → finish_step{action:"ask"} → 本轮收口（finalized = Ask）
        → walker emit RunUpdate::Ask → guard.wait_answer()
        → 用户应答 → decision_answer_prompt → 同会话 Continue 重开一整轮
```

三个代价同时成立：ask 必消耗一整轮（一次完整模型请求）；停等挂在 walker 而非 turn 内（会话库态已是「已收口」，转录面板看不到「正在等待」）；`finish_step` 的动作集合把「收口」与「提问」两种正交语义压进同一个 enum——「我干完了」与「我卡住了」共用一个出口。

### 现象三：ask 通道绑定在 `(root, change_id)` 上，通用化无路

应答通道 `answer_tx` / `wait_answer` 全部住在 `ChangeFlowControl`（键 = `(root, change_id)`）。`core/agent` 不认识 workspace root 与 change——这是它边界中立契约的一部分。所以「通用会话也能用 ask」不是改一个分支，而是要一条**会话级**通道；`StopRegistry`（键 = core session id，`register` / `request_stop` / `remove` 三方法）已经证明这个形态在本仓成立。

### 现象四：聊天窗里那张 ask 卡是死卡

`components/agent/tool-cards.tsx` 的 `AskUserQuestionCard` 是静态可读卡，文案「请在下方输入框作答」在多数页面其实是误导——那个输入框不一定在。没有任何应答回调能挂上去（`AgentToolCardProps` 只有 `{toolCallId, toolName, state, input, output}`）。流程侧详情页的 `AskCard`（`run-control-panel.tsx`）是唯一可应答入口，聊天窗只能看不能说。

### 症结：ask 是被「流程专用」误定位的通用能力

前几轮把 ask 当成流程专属、通用来源传 `None` 即可。用户澄清后方向反转：**通用聊天（explore / archi / debug）才是基准场景**（效果 = 聊天窗出现可点的选择卡片），**change 流程是它的同名覆盖**（额外把 change 切到等待用户决策态），个别场景可显式注入「不许用」。这条定位决定的不仅是改动顺序，还有形状：同名、同 schema、两种执行体，模型视角与历史转录零差异。

## 提案

把 ask 从「`finish_step` 的一个枚举值」提升为**挂载清单里的独立阻塞工具**，并把应答通道下沉到会话基建：

```
今天                                            之后
────────────────────────────────────────────    ─────────────────────────────────────────────
通用会话（explore/archi/debug）                  通用会话
  无 ask 能力                                       AskUserQuestion（ToolExec::Ask(Generic)）
  需决策 → 问题文本收口 → Continue 重开一轮           阻塞在同一 turn 内 → 聊天窗活卡 → 答完接着说

流程会话（change step）                           流程会话
  ask = finish_step 的一个 enum 值                  AskUserQuestion（ToolExec::Ask(Flow)，同名覆盖）
  → 本轮收口 → walker wait_answer                  额外 publish RunUpdate::Ask → change 切 WaitingAsk
  → 应答 → Continue 重开一轮                        执行体返回后清停等态；两入口（change_flow_answer /
                                                   agent_answer）都只投 AskRegistry

应答通道                                          应答通道
  ChangeFlowControl.answer_tx（键 = root+change）    AskRegistry（键 = core session id，与 StopRegistry 同型）
```

### 裁决表（含被拒备选与理由）

| # | 决策 | 要点 | 被拒备选与理由 |
|---|---|---|---|
| D1 | ask 是**通用能力**，通用聊天是基准场景 | explore / archi / debug 与 change 全都有 ask；差别只在执行体是否额外广播流程状态 | **被拒：只做流程版、通用版推迟**——通用会话缺少 turn 内等待通道，流程版也无处阻塞；且通用场景（澄清歧义）本就是 ask 最高频的用法 |
| D2 | 流程侧是**同名覆盖**，不是第二套工具 | 同一个 `TOOL_NAME`、同一份 `parameters()`（schema 单源在 orchestration），差别只在执行体 | **被拒：流程用另一个工具名**——模型视角出现两个语义相同的工具（选哪个都要 prompt 解释），历史转录需按来源分辨，前端卡要多一路判据 |
| D3 | 应答通道 = `AskRegistry`，键 = **core session id** | 与 `StopRegistry` 完全同型（`register` / `answer` / `remove`），住 `core/agent` | **被拒：扩 `RunHandle`**（把终止信号与应答通道塞进同一类型，两者语义边界变模糊）；**被拒：`AgentSession` 加第四件套 mpsc**（mpsc 不适合请求-应答配对，需自带 id 对账，最终退回 oneshot 表）；**被拒：通道住 orchestration**（通用来源没有 change 键） |
| D4 | `SessionInjections` 加 `ask: Option<AskMode>` 位 | `None` = 显式禁用 / `Some(Generic)` / `Some(Flow)`；与 `finish` 同位同型（`Copy + Eq` 闭合枚举） | **被拒：用 `tools` 名字清单表达禁用**——ask 是按位追加的基线工具（与 `finish_step` 同款），名字清单过滤碰不到它；**被拒：无条件恒挂**——「个别场景不许用」就无处表达 |
| D5 | 流程状态清理搬到 **ask 执行体**（Y-1） | `ToolExec::Ask(Flow)` 的 `select!` 两臂返回后清 status + 载荷槽并广播；两个应答入口都只做「投递」 | **被拒：`change_flow_answer` 内清理**（`agent_answer` 直达 `AskRegistry` 时没人清流程状态，双入口必须幂等）；**被拒：`agent_answer` 反向认识编排状态**（破坏 core/agent 边界）；**被拒：保留 `answer_tx` 双通道**（两套真相必然漂移） |
| D6 | `accept_pending` **不拆**，只有 `answer()` 改投、`wait_answer()` 退役 | `accept_pending` 是泛型模板，`confirm()` 仍在用；锁内先清再 send 的顺序纪律全保留 | **被拒：重构 `accept_pending`**——`confirm` 侧零改动诉求下，拆模板是纯成本；核实后「只换两个方法」即够 |
| D7 | `finish_step(Decision)` 的 enum 收为 **`["backtrack","stop"]`** | 删 `question` / `options` 两个 property；description 改为「无法裁决时调用 ask 工具」 | **被拒：留着 ask 枚举值双轨**——同一诉求两个出口，模型必然随机选一个；两轨的清理路径不同，是 bug 温床 |
| D8 | `DecisionAction::Ask` **保留**，生产不可达 | `validate_decision` 的 `"ask"` 臂保留（历史 notice 反序列化 + 防御臂判据），finish_step schema 不再产出它 | **被拒：硬删变体**——历史转录里既有 `FinishPayload::Decision(Ask)` 会反序列化失败；保留成本为零（一个 enum 值 + 一句注释） |
| D9 | `options` **非空**（`minItems: 1`） | 沿用 `validate_decision` 现有校验；用户仍可用自由文本作答 | **被拒：`options` 可空只留自由输入**——没有选项的「选择卡片」退化为文本框，模型把决策权整个丢回给用户，与该工具的意图相反 |
| D10 | 单问，**多次调用** | 一次 ask 一个 `question` + 一组成员 | **被拒：`questions: [{...}]` 多问**——部分应答的语义（答了 2/4 怎么办）会把两处 UI 同步推到「部分完成」这一复杂态；多次调用天然覆盖 |
| D11 | 停止语义**不新增机制** | 阻塞期间停止 = 外层泵 `select!` drop 整个 loop future；转录留未配对 `tool_use`，由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 补齐 | **被拒：ask 执行体自己 `select!` 停止即回灌 `is_error`**——外层 drop 先于一切，回灌做不到；且外层 select 已是既有机制（零新增） |
| D12 | 载荷 `AskPayload` 扩 `tool_call_id` / `session_id`（additive） | 聊天窗活卡对账（`ask.toolCallId === part.toolCallId`）与转录定位 | **被拒：零扩展**——无 `tool_call_id` 时行内卡只能靠「output 为 null 就是在等」推断，而历史转录里所有未配对 `tool_use` 都是 `input-available`，分不清「正在等」与「上次崩了没回灌」 |
| D13 | 首期只建 `ask = None` 的位与分支，**不接线** | 目前无禁用场景（如某些自动化步），形状先立 | **被拒：当期一并找场景接线**——没有真实需求就接线会造出「为禁用而禁用」的隐式规则；位建好即是可扩展面 |
| D14 | `finish = None` 的会话**仍有 ask** | ask 挂载不依赖 `finish` 位；两模块互不引用（`grep` 可验） | **被拒：ask 随 finish 位一并挂**——语义正交（「我干完了」vs「我卡住了」），绑定会让 explore 类会话永远拿不到 ask |

### 三层改动（本次一次做完，第三层只建位）

```
第一层 · 会话地基（通用版的前提）
core/agent
  ask.rs（新）        AskRegistry：register(key, tx) / answer(key, text) / remove(key)
                      键 = core session id，与 StopRegistry 同型；take 语义（重复应答 Err）
  kernel.rs           SessionKernel 持 Arc<AskRegistry>（第 4 个构造参）；converge 处 remove 兜底
  runner.rs           AskMode { Generic, Flow }（Copy + Eq，serde camelCase）
                      SessionInjections 加 ask: Option<AskMode>（#[serde(default)]）
                      SessionOpen 加 session_id: String（core 铸 id 下传引擎——ask 登记键）
  lib.rs              导出 AskRegistry / AskMode
infra/agent
  sdk/mount.rs        ToolExec::Ask(AskMode) + 挂载铸造（按 injections.ask，逐 tier = Protocol）
  sdk/loop.rs         分派两臂 + LoopTurn.ask: Option<AskContext>（registry + core session id + port）
  sdk/runner.rs       PumpPayload 透传 registry + ask_port；per-turn 铸造 AskContext（按 ask 位）
  lib.rs              EngineFacade::with_ask_registry + with_ask_port 两条注入缝
  compose.rs          compose_turn 收 Arc<AskRegistry> 与 Option<Arc<dyn AskPort>>；
                      assemble_injections 逐臂置 ask 位（debug 等不再走 default()）
壳层
  main.rs             app.manage(Arc::new(AskRegistry::default()))
  commands/exec       agent_start 传 registry；新增 agent_answer(root, session_id, text)
前端
  types/generated     agent_answer 命令 + AskPayload 两字段（生成物）
  hooks/use-agent-session   answer 动作
  components/agent    tool-cards 活卡（注册表卡两态）+ agent-messages / agent-session-view 动作面透传
  views/*             各消费点注入 askAnswer（通用侧走 agent_answer，流程侧走 change_flow_answer）

第二层 · 流程覆盖
core/orchestration
  ask.rs（新）        TOOL_NAME = "AskUserQuestion" / description / parameters / validate
                      MUST NOT use crate::finish（解耦守卫，grep 可验）
  port.rs             AskPort trait（ask_started(AskPayload) / ask_resolved()）
  state.rs            AskPayload 扩 tool_call_id / session_id；RunUpdate::Ask 载荷收敛为
                      `Ask { ask: AskPayload }`（单源，避免变体字段与载荷类型两份真相）
  control.rs          answer() 改为转投 AskRegistry（不再 accept_pending）；删 answer_tx 槽与
                      wait_answer()；accept_pending / confirm() / wait_confirm() 逐字不动
  finish.rs           decision 形态 enum 收两值 + 删 question/options + description 改写
  decision.rs         DecisionAction::Ask 保留 + 注释说明生产不可达
  prompt.rs           decision_prompt 三选一 → 两选一 + 「无法裁决时调用 ask 工具」指路；
                      删 decision_answer_prompt
  walker.rs           删 finalized == Ask 分支（归入单裁决，Ask 载荷出现即 Terminal::failed 防御）；
                      删 wait_answer 调用
infra/agent
  sdk/loop.rs         ToolExec::Ask(Flow) 臂：ask_started → register → select!(应答, 停止) →
                      ask_resolved（两臂都清）
壳层
  commands/change_flow  ChangeFlowSink 实现 AskPort；change_flow_answer 经 control.current_session()
                        取 id → AskRegistry.answer

第三层 · 显式禁用（只建位）
  SessionInjections.ask = None → definitions 不挂 ask → 模型调用即 tool_unavailable
  首期无禁用场景，位与分支建好不接线（D13）
```

### 「覆盖」的实现形态：一个变体带档位

```rust
// infra/agent/sdk/mount.rs
pub(crate) enum ToolExec {
    Pure,
    ExploreNote,
    Archi,
    FinishStep(agent::FinishSpec),
    /// ask 工具：一个变体带档位（schema 单源，仅执行体行为不同）
    Ask(agent::AskMode),
}
```

`mount.rs` 的 `definitions(injections, prompt)` 追加规则（与 `finish_step` 同位但**互不依赖**）：

```
tools 名字清单过滤（内建七 / explore 六 / archi 九 / change 七+矩阵）
ask.is_some()    → 追加 AskUserQuestion（ToolTier::Protocol，exec = Ask(mode)）
finish.is_some() → 追加 finish_step（ToolTier::Protocol，exec = FinishStep(spec)）
```

两行彼此无引用：`finish: None` 的 explore 会话照样有 ask，`ask: None` 的会话照样有 finish_step。

### 装配位的置位规则（`assemble_injections` 单点）

```
source = "explore"                → Some(AskMode::Generic)
source = "archi"                  → Some(AskMode::Generic)
source = "change"（任意 role）     → Some(AskMode::Flow)
其余来源（debug 等）               → Some(AskMode::Generic)   ← 不再走 SessionInjections::default()
需要禁用的步（首期无）             → None
```

`SessionInjections::default()` 的 `ask = None` 自此是**「未装配即禁用」的 fail-closed 形态**：装配层 MUST 显式置位（含 debug 臂不再用 `default()` 而是显式构造）。旧线 JSON 缺字段读 `None` 同义。

### 应答通道的形状（与 `StopRegistry` 同型）

```rust
// core/agent/src/ask.rs（新）
pub struct AskRegistry { slots: Mutex<HashMap<String, oneshot::Sender<String>>> }

impl AskRegistry {
    pub fn new() -> Self;
    /// 登记等待方（ask 执行体进入阻塞前调用；同键覆盖 = 上一问已被放弃）
    pub fn register(&self, session_id: &str, tx: oneshot::Sender<String>) -> bool;
    /// 投递应答（take 语义：单次应答，重复落空 → Err）
    pub fn answer(&self, session_id: &str, text: String) -> Result<(), String>;
    /// 除名（轮收敛时清理；miss 幂等）
    pub fn remove(&self, session_id: &str);
}
```

**为什么键是 core session id**：`core/agent` 不认识 workspace root 与 change。流程侧的两段寻址由壳层翻译：

```
change_flow_answer(root, change_id, text)
  ├ control.current_session(root, change_id) → Option<session_id>
  └ ask_registry.answer(session_id, text)

agent_answer(root, session_id, text)
  └ ask_registry.answer(session_id, text)
```

两个入口都只做「投递」，天然幂等（先到者胜，后者 `Err`）。

### core session id 的下传（本变更新增的必要缝）

`AskRegistry` 的键是 core 铸的会话 id，但引擎侧当前看不到它（`SessionOpen` 只有 `SessionRef::{New, Continue{id}}`——`New` 时 core 铸的 id 从未告知引擎）。**这是 explore 未点明、但落地必需的一条缝**：`SessionKernel::begin_turn` 里 session id 在 `open_session` 之前就已算出，故 `SessionOpen` 增 `session_id: String` 一行即可（`TurnQuestion` 不动——会话级标识在会话建立时一次可知，逐轮重复携带同一 id 只会多一处漂移面）。

注意与 `LoopTurn.session_id` 的区别：后者是**引擎侧** `sdk-<counter>-<millis>` id（`RunStarted.session_id` 上报、store 双 id 映射用），MUST NOT 当作注册表键；ask 上下文携带的必须是 core 铸 id（经 `SessionOpen.session_id` 下传的那一个）。两者同名不同义，实现时以「ask 上下文另存一个字段」隔离（不做名字复用）。

### 停等态清理的时点（Y-1）

```
change_flow_answer / agent_answer      只投递（AskRegistry.answer）
        ⇓
ask 执行体（ToolExec::Ask(Flow)）       select!(应答, 停止) 返回后 → AskPort::ask_resolved()
                                         → 清 status + ask 槽 + 广播 RunNotice::Step
```

于是流程状态清理与「应答从哪来」解耦，两入口幂等；`control.rs::answer()` 不再触碰 `answer_tx`，也不再清状态。**`accept_pending` 泛型模板、`confirm()`、`wait_confirm()` 与「锁内先清再 send」的顺序纪律逐字不动**（`confirm` 与 ask 无关）。

清理时点从「应答命令内」后移到「执行体返回后」，窗口在毫秒级（同一 `select!` 的返回路径上）；前端本就以 `RunNotice::Step` 触发统一视图重查，这一帧延迟不可观测。停止路径同理走 `select!` 的停止臂（`handle.wait_requested()`）——**显式臂是必要的**：外层泵的 `select!` 是 drop loop future，drop 不执行清理代码。

### `finish_step` 的收窄

```
core/orchestration/finish.rs
  parameters(Decision)：enum ["backtrack","stop","ask"] → ["backtrack","stop"]
                        删 question / options 两个 property
  description(Decision)：「三动作封闭集」→「两动作」+「无法裁决时调用 AskUserQuestion 工具」
  validate()：          仍走 validate_decision（其 "ask" 臂保留——历史 notice 可反序列化）

core/orchestration/decision.rs
  DecisionAction::Ask      保留（历史兼容 + walker 防御臂判据），注释标明生产不可达

core/orchestration/prompt.rs
  decision_prompt          三选一 → 两选一，新增一句「无法裁决时调用 AskUserQuestion」
  decision_answer_prompt   删除（应答不再开新一轮：ask 在同一 turn 内闭合）

core/orchestration/walker.rs
  decision_session         删 finalized == Ask 的停等分支；Ask 载荷出现即 Terminal::failed
                           （「决策漂移：ask 应经 AskUserQuestion 工具提交」）——生产不可达的防御面
  resolve_deadlock         既有 DecisionAction::Ask 防御臂保留
```

### 解耦的机械判据（可 `grep` 验）

```
crates/core/orchestration/src/ask.rs    零 "finish" 字样（MUST NOT use crate::finish）
crates/core/orchestration/src/finish.rs 的 "ask" 只出现在「指路 ask 工具」的文案字符串里
infra mount 的两条追加分支互不引用（无 `finish.is_some() && ask.is_some()` 一类合取）
```

### 停止、幂等与边界

| 场景 | 行为 |
|---|---|
| ask 阻塞期间用户停止 | 外层泵 `select!` drop loop future → 轮收敛 `stopped`；转录留未配对 `tool_use`，`resume::rebuild` 的 `INTERRUPTED_RESULT` 补齐（既有机制）；`AskRegistry` 槽由 `converge` 的 `remove` 兜底 |
| 同一 ask 二次应答 | `AskRegistry.answer` take 语义 → 第二次 `Err`（「当前无等待中的 ask」），不产生第二次回灌 |
| 会话已收口后应答 | `AskRegistry` 槽已除名 → `Err`；`change_flow_answer` 仍在无 run 时显式 `Err` 携 change id |
| ask 阻塞是否触发 API 流空闲超时 | 否——ask 阻塞发生在工具执行段，不在流读取段（`StreamLiveness.frame` 只作用于流帧间隔） |
| `finish = None` 的会话 | 仍有 ask（`ask` 位独立） |
| 历史转录里的 `AskUserQuestion` ToolUse | 照旧静态只读卡（无 `askAnswer` 注入即静态） |

## 能力

### 新增的能力

**无。** 本变更全部落在既有能力面内：ask 是挂载清单里的一个挂载项（与 `finish_step` 同列），应答通道是会话基建的一件设施（与 `StopRegistry` 同址），载荷走既有 `RunUpdate::Ask` / `AskPayload`，前端走既有工具卡注册表与 `AgentSessionView` 动作面注入位。

### 修改的能力

- **desktop-agent-execution** —
  - **MODIFIED**「会话级工具面注入与 explore 笔记工具」：`SessionInjections` 形态由三位扩为**四位**（`preamble` / `tools` / `finish` / `ask`）；`ask: Option<AskMode>` 为 `#[serde(default)]` 的闭合枚举位（`Generic` / `Flow` / `None`=显式禁用），装配层逐臂显式置位（explore / archi / debug 等 → `Generic`，change → `Flow`）；`definitions()` 按 `ask` 位追加 `AskUserQuestion` 挂载项（`ToolTier::Protocol`），与 `finish` 位**互不依赖**；`RunStarted.tools` 随实际下发面。
  - **MODIFIED**「SDK 引擎（rig 进程内租户）」：工具面口径补一条——ask 是**基线交互工具**（与 read / bash 并列的能力，不随名字清单收窄开关），其定义与 tier 也归 infra 挂载清单；「注入位词面」由三位改四位。
  - **MODIFIED**「agent 执行命令面」：新增应答命令 `agent_answer(root, session_id, text)`（薄包装：blank 参数显式 `Err`、miss 显式 `Err`「当前无等待中的 ask」，不幂等——与 `agent_stop` 的 miss 幂等是不同语义）。
  - **MODIFIED**「引擎泵一轮一命与 headless 无问收口」：删「MUST NOT 经 ask 类工具中途提问」的禁令与对应 scenario，改为「ask 为阻塞工具，应答经会话级 `AskRegistry` 在同一 turn 内回流；一轮一命不受影响（阻塞发生在轮内、不新开轮）」。
  - **MODIFIED**「MVP 边界与已知限制」第 4 条「交互工具限制」：由「工具面为封闭清单（无 ask 类工具），需决策时以问题文本收口 + Continue 续轮」改写为「ask 为基线阻塞工具（`Generic` / `Flow` 两档执行体、`None` 可显式禁用）；无 ask 的原形态退役；questionnaire 组件仍不接线」。
  - **ADDED**「ask 工具协议与阻塞应答通道」：`AskRegistry`（键 = core session id，`register` / `answer` / `remove`，take 语义）、`SessionOpen.session_id` 下传、`AskMode` 两档执行体（`Generic` 只阻塞 / `Flow` 额外 `AskPort` 广播）、`ToolExec::Ask(mode)` 分派与 `LoopTurn.ask` 上下文、停止语义（外层 drop + `resume` 补齐）、Err 文案自足与重复应答 `Err`。
- **desktop-change-orchestration** —
  - **REMOVED**「决策会话软拒环与漂移上限」：该机制的两条前提已分别消失——① 决策输出的协议校验已改由 `finish_step` 的工具校验**当场回灌**（`is_error` ToolResult，模型在同一 turn 内可立即纠正；`parse_decision` 与 `DECISION_DRIFT_LIMIT` 随之删除），跨轮软拒计数无消费者；② `ask` 自本变更起迁出为独立阻塞工具 `AskUserQuestion`，主基线「`ask` 轮不计漂移」这条的载体不复存在。保留它会与收窄后的「决策协议」条形成两套相互矛盾的纠错语义（软拒两次 vs 一次即停）。**本条是 `agent-finalize-tool-for-flow-steps` 已声明的退役面在主基线里的滞后残留**（`openspec/specs/` 尚未吸收该 delta），故与本变更同批合并时两侧互为幂等空操作，合并后 MUST NOT 以任何形态恢复。**Migration**：白名单越权改由 `ensure_backtrack_allowed` **一次即** `Terminal::failed`（文案携 `allowed` 列表，留 failed `WhitelistGate` 节点作图面证据）；形状漂移 / 未知动作由 `finish_step` decision 形态的 schema `Err` 当场回灌、同 turn 内重试；`ask` 停等迁出为 `AskUserQuestion`（`ToolExec::Ask(Flow)`，turn 内阻塞、答完接着说）；删除面 = `parse_decision` / `DECISION_DRIFT_LIMIT` / `decision_fix_prompt` 与软拒环续注分支（`grep` 零命中为判据），`ensure_backtrack_allowed` 与 `WhitelistGate` 每轮一节点的图面语义逐字保留。
  - **MODIFIED**「决策协议」：决策动作封闭集由三值收为**两值**（`backtrack` / `stop`），`ask` 迁出为独立工具 `AskUserQuestion`（同名覆盖，`ToolExec::Ask(Flow)`）；`finish_step` 的 decision 形态 description 与 parameters 随之收窄（删 `question` / `options`），无法裁决时指路 ask 工具；决策会话单轮收口（ask 不再消耗整轮、不再 `Continue` 重开）。
  - **MODIFIED**「运行控制命令面」：`answer` 应答通道由 `ChangeFlowControl.answer_tx` 转为 `AskRegistry`（键 = core session id，经 `current_session()` 翻译）；停等态解除时点由「应答命令内（`accept_pending`）」改为「ask 执行体返回后（`AskPort::ask_resolved`）」；`confirm` 侧的受理与清理语义逐字不变（`accept_pending` 保留给 `confirm`）。
  - **MODIFIED**「相位 prompt 组装面静态化（静态主体 + 动态 append）」：动态 append 清单里决策会话项的「三动作」改两动作 + 一句 ask 工具指路；`decision_answer_prompt` 退役（应答不再开新一轮）。同步吸收 `finish_step` 已落地的现状表述（工具形状由挂载面承载，协议文本不进 prompt 主体）。
  - **ADDED**「流程内 ask 停等与双入口应答」：`AskPort`（`ask_started` / `ask_resolved`）、`ToolExec::Ask(Flow)` 执行体序列（`ask_started` → `register` → `select!` → `ask_resolved`，两臂都清）、`AskPayload` 扩 `tool_call_id` / `session_id` 两 additive 字段、`RunUpdate::Ask` 载荷收敛为 `Ask { ask: AskPayload }`、两入口（`change_flow_answer` / `agent_answer`）幂等与失效信号沿用 `RunNotice::Step`、`ask` 槽在应答后必清（依赖前置契约）与停止/取消两臂的清理纪律。
- **desktop-change-flow-view** —
  - **MODIFIED**「运行控制入口与 ask 应答」：ask 中断卡片的触发源由「决策 agent 输出 `ask` 动作」改为「**ask 工具调用**（`AskUserQuestion` 阻塞于 turn 内）」；新增「聊天窗 ask 活卡与详情页 `AskCard` **同源呈现**」——两处任一入口应答后两处同步收起、`activeRun.status` 回 `running`；`AskCard` 与聊天内卡都读同一 `AskPayload`（`question` / `options` / `toolCallId`）；控制入口状态机（可发起 / 运行中可停止 / ask 等待应答 / phase 间等待确认）与「状态与操作不错配」语义逐字保留。
- **desktop-agent-chat-infra** —
  - **MODIFIED**「会话级统一渲染组件（sessionId 入参 + 状态驱动挂流）」：入参增第二个可选动作面注入位 `askAnswer`（与 `composer` 插槽**互不前置**——只读消费点可只接 ask 应答）；headless 基建暴露 `answer(text)`（`agent_answer` 薄包装，`root` / `sessionId` 为 null 不 invoke）；两形态（自持 / 注入）同透传。
  - **MODIFIED**「展示组件族(对话透镜)」：`AskUserQuestionCard` 由「静态可读卡」改为**双态卡**（未注入应答动作面 → 静态只读；注入 → 活卡，呈选项按钮 + 自由文本输入），`AgentToolCardProps` 增可选动作面；只读消费点默认形态不变。
  - **ADDED**「ask 工具卡活卡与应答动作面」：动作面经 `AgentSessionView` → `AgentMessages` → 工具部件的**可选注入位**下行（`askAnswer: { onAnswer, pendingToolCallId }`）；活卡判据（注入在场 + `state === 'input-available'`，流程侧按 `pendingToolCallId` 精确对账）；应答经既有状态面（`useAgentSession.answer` → `agent_answer`）或来源侧覆盖（流程抽屉 → `change_flow_answer`）；未注入即静态卡，`ToolPairCard` 回退与卡根钩子零改动。

### 其余能力零改动（含理由）

- **desktop-agent-execution（除上述六条外）**：静态权限三档与路径沙箱的 tier 表已含 `ToolTier::Protocol`（`Protocol | Read` 恒放行），ask 归 Protocol 即为该表既有分支；`verdict` / `finish_step` 收口面、`resume`、上下文防线、`web_search`、`pause_turn`、截断终局全部零改动。
- **desktop-change-orchestration（除上述五条外）**：verdict 解析与 phase-log 代写、失败升格落账、双 root、停止贯通、步级时间戳、ArchiCheck 接线等零改动；机械 checklist 代写面零改动。
- **desktop-agent-chat-infra（除上述三条外）**：`use-agent-session` 的重放装载 / 状态驱动挂流 / 单流在挂 / 断流重订契约零改动；适配层 `agent-adapter.ts` / `agent-transport.ts` 零改动（ask 的 tool_use / tool_result 走既有工具部件管线）；`finish_step` 卡零改动。
- **desktop-phase-prompts**：14 份 md 零改动——ask 是**基线工具**（不经名字清单开关），其形状由工具 schema 承载，md 的「只引用桌面七工具 + 矩阵命中的 archi 工具」纪律不变（md MUST NOT 许诺未下发工具面；ask 由注入面下发，不属于「md 引用面」）。
- **desktop-explore-page / desktop-archi-chat / desktop-change-chat**：零改动——三处会话面的 composer / 只读形态 / 禁发矩阵语义不变；ask 活卡的应答接线经组件族动作面注入位完成（一行接线，不改变页面契约）。
- **desktop-change-queries / desktop-app-shell / desktop-ipc-type-bindings**：零改动——`AskPayload` 两字段随统一视图自动透出（additive）；`agent_answer` 落 `commands/exec/` 既有轨道（轨道纪律与命名纪律不变），`bindings` 由 `bindings:export` 再生成（提交含生成物）。
- **desktop-change-state-store**：零改动——停等态仍不落库（`ask` 槽活在进程内 `ChangeFlowControl` / `AskRegistry`）。
- **repo-archi-model / desktop-crate-layout**：零新 crate、零新元素、零新依赖边（`infra/agent` 已依赖 `core/agent` 与 `core/orchestration`；`AskRegistry` 用 `core/agent` 既有 `tokio::sync`）。

### 主基线文本与代码现实的一处偏差（必读）

`openspec/specs/` 下 `desktop-agent-execution` / `desktop-change-orchestration` / `desktop-archi-agent` / `desktop-agent-chat-infra` / `desktop-phase-prompts` / `desktop-workflow-write-face` 的主基线**尚未吸收** `agent-finalize-tool-for-flow-steps` 的归档 delta（`grep -rn "finish_step" openspec/specs` 零命中，而代码里 `finish_step` 已在产线运行）。

因此本 change 的 MODIFIED requirement 正文**以「主基线原文 + finalize 归档 delta 的合并后形态」为底稿**再叠加本次改动——否则归档时整块替换会把 `finish_step` 的全部条款冲掉。归档顺序上两份 delta 互不冲突（finalize 描述收口协议，本 change 描述 ask 分家与应答通道），但 **finalize 的 delta 必须先合并或同批合并**（见风险 R1）。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/agent/src/ask.rs`（新） | `AskRegistry { slots: Mutex<HashMap<String, oneshot::Sender<String>>> }` + `new` / `register(session_id, tx) -> bool` / `answer(session_id, text) -> Result<(), String>` / `remove(session_id)`（take 语义；`answer` 文案「当前无等待中的 ask」与既有口径一致） |
| `crates/core/agent/src/lib.rs` | 导出 `AskRegistry`（`AskMode` 随 `runner` 出口） |
| `crates/core/agent/src/runner.rs` | 新增 `AskMode { Generic, Flow }`（`Copy + Eq`，`#[serde(rename_all = "camelCase")]`，**不派生 `specta::Type`**——引擎侧会话词汇不出线 IPC）；`SessionInjections` 增 `ask: Option<AskMode>`（`#[serde(default)]`，键恒在）；`SessionOpen` 增 `session_id: String`（core 铸 id 下传，doc 注明用途 = ask 登记键与引擎侧归属） |
| `crates/core/agent/src/kernel.rs` | `SessionKernel` 增第 4 字段 `ask_registry: Arc<AskRegistry>`；`new` 签名加一参；`RunningTurn` 携带该 `Arc` 并在 `converge` 内 `remove(session_id)`（与 `registry.remove` 同址兜底）；`begin_turn` 把 session id 填进 `SessionOpen` |
| `crates/core/orchestration/src/ask.rs`（新） | `TOOL_NAME = "AskUserQuestion"`；`description() -> &'static str`；`parameters() -> Value`（`question` 必填、`options` `minItems: 1`、单问单次）；`validate(args) -> Result<AskQuestion, String>`（问题非空 / options 非空且元素为字符串，Err 文案自足）；`AskQuestion { question, options }`。**MUST NOT `use crate::finish`**（解耦守卫） |
| `crates/core/orchestration/src/lib.rs` | 导出 `ask::{TOOL_NAME, description, parameters, validate, AskQuestion}` |
| `crates/core/orchestration/src/finish.rs` | decision 形态 `parameters` 的 `enum` 收为 `["backtrack","stop"]`、删 `question` / `options` property；`description(Decision)` 改两动作 + ask 工具指路（**文案里出现工具名，非代码引用**） |
| `crates/core/orchestration/src/decision.rs` | `DecisionAction::Ask` 保留，doc 补「生产不可达：`finish_step` 的 enum 已不含它，历史 notice 反序列化仍可读」；`validate_decision` 的 `"ask"` 臂逐字保留 |
| `crates/core/orchestration/src/prompt.rs` | `decision_prompt` 的「决策协议」段由三选一改两选一 + 一句「无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）」；删 `decision_answer_prompt` |
| `crates/core/orchestration/src/walker.rs` | `decision_session` 删 `finalized == Ask` 的 `emit` / `wait_answer` / `Continue` 分支——`FinishPayload::Decision(Ask { .. })` 归入单一裁决分支并 `Terminal::failed`（「决策漂移：ask 应经 AskUserQuestion 工具提交」）；`resolve_deadlock` 的 `DecisionAction::Ask` 防御臂逐字保留 |
| `crates/core/orchestration/src/control.rs` | `answer()` 改为 `current_session(root, change_id)` → `ask_registry.answer(session_id, text)`（无会话 / 无等待方 → `Err`）；删 `answer_tx` 槽字段与 `wait_answer()`；`publish` 的 `RunUpdate::Ask` 臂随变体收敛改为 `entry.ask = Some(ask)`（其余四臂逐字不动）；`accept_pending` 的泛型签名与 `register_pending`、`confirm()`、`wait_confirm()`、锁序纪律逐字不动；`ChangeFlowControl::new(ask_registry: Arc<AskRegistry>)` 增一参（`Arc` 共享，MUST NOT 内部自铸——否则与引擎侧登记的表不是同一张）；新增 `resolve_ask(root, change_id)`（`ask_resolved` 的注册表侧实现：条件清 status + `ask` 槽 + 广播 `RunNotice::Step`） |
| `crates/core/orchestration/src/port.rs` | 新增 `AskPort` trait：`fn ask_started(&self, ask: AskPayload)` / `fn ask_resolved(&self)`（编排侧状态面契约，实现驻壳层） |
| `crates/core/orchestration/src/state.rs` | `AskPayload` 增 `tool_call_id: Option<String>` / `session_id: Option<String>`（additive，`#[serde(default)]`）；`RunUpdate::Ask { question, options }` 收敛为 `Ask { ask: AskPayload }`（载荷单源；`kind()` 与 `RunNotice` 投影零变） |
| `crates/infra/agent/src/sdk/mount.rs` | `ToolExec` 增 `Ask(agent::AskMode)`；`definitions()` 在 `injections.ask` 为 `Some(mode)` 时追加 `AskUserQuestion` 挂载项（`description` / `parameters` 取自 `orchestration::ask`，`tier = Protocol`）；与 `finish` 位两段独立 `if`（无合取）；doc 补一条 provider 侧不变量（原生 ask 工具一律不挂 = 覆盖） |
| `crates/infra/agent/src/sdk/loop.rs` | `LoopTurn` 增 `ask: Option<AskContext>`（`AskContext { registry: Arc<AskRegistry>, session_id: String /* core 铸 id，**不是** `LoopTurn.session_id`（后者是引擎侧 `sdk-…` id）*/, port: Option<Arc<dyn AskPort>> }`）；执行分派增 `ToolExec::Ask(mode)` 两臂：`Generic` = `register` + `select!(oneshot, handle.wait_requested())`；`Flow` = `ask_started` + `register` + `select!` + `ask_resolved`（两臂返回后都清）；`port` 缺席或 `register` 失败 → `is_error` 回灌 |
| `crates/infra/agent/src/sdk/runner.rs` | `PumpPayload` 增 `ask_registry: Arc<AskRegistry>` 与 `ask_port: Option<Arc<dyn AskPort>>`；per-turn 铸造 `AskContext`（按 `injections.ask` 位；`Flow` 档把 `ask_port` 一并放入，缺席即 `is_error` 回灌） |
| `crates/infra/agent/src/lib.rs` | `EngineFacade::with_ask_registry(Arc<AskRegistry>)` + `with_ask_port(Option<Arc<dyn AskPort>>)` 两条注入缝（与 `with_explore_note_store` 同型），`SdkRunner` 持有并传入 `PumpPayload` |
| `crates/infra/agent/src/compose.rs` | `compose_turn` 增两参 `ask_registry: Arc<AskRegistry>` / `ask_port: Option<Arc<dyn AskPort>>`（前者透传内核 + 门面，后者仅门面——编排侧状态面的实现方是壳层 sink，不在内核侧）；`assemble_injections` 逐臂置 `ask` 位（explore / archi / debug → `Generic`，change → `Flow`） |
| `src-tauri/src/main.rs` | `app.manage(Arc::new(AskRegistry::default()))`（与 `StopRegistry` / `SessionStreamRegistry` 三层同址）；`ChangeFlowControl` 构造改为传入 `Arc<AskRegistry>` |
| `src-tauri/src/commands/exec/mod.rs` | `agent_start` 取 `AskRegistry` 句柄传入 `compose_turn`；新增 `agent_answer(registry: State<Arc<AskRegistry>>, root, session_id, text) -> Result<(), String>` 薄包装（blank 参数 `Err`、miss `Err`） |
| `src-tauri/src/commands/mod.rs` | `all_commands!` 登记 `agent_answer`（`commands/exec` 轨道） |
| `src-tauri/src/commands/change_flow/mod.rs` | `ChangeFlowSink` 实现 `AskPort`（`ask_started` → `control.publish(RunUpdate::Ask { ask })`；`ask_resolved` → `control.resolve_ask(root, change_id)`）；`change_flow_answer_with` 改为 `control.answer`（内部转投 `AskRegistry`），签名与前端参数面零改动；`change_flow_start_with` 把 `AskRegistry` 与 `Arc<dyn AskPort>`（即该 sink 自身）传入 `compose_turn` |
| `packages/desktop/src/types/generated/bindings.ts`（生成物） | `agent_answer` 命令 + `AskPayload` 两字段 |
| `packages/desktop/src/hooks/use-agent-session.ts` | 状态面增 `answer: (text: string) => void`（`ipc.agentAnswer(root, sessionId, text)`；`sessionId` / `root` 为 null 即不发） |
| `packages/desktop/src/components/agent/tool-cards.tsx` | `AgentToolCardProps` 增可选 `askAnswer?: { onAnswer: (text: string) => void }`（在场即可应答）；`AskUserQuestionCard` 双态：静态（现状）与活卡（选项按钮 + 自由文本输入 + 提交，提交后本地禁用防连点，文案不再写「请在下方输入框作答」） |
| `packages/desktop/src/components/agent/agent-messages.tsx` | `AgentMessagesProps` 增可选 `askAnswer?: AskAnswerActions`（`{ onAnswer: (text: string) => void; pendingToolCallId?: string | null }`），`ToolPartView` 按卡名与判据透传；未传即零变化（静态卡） |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | `AgentSessionViewProps` 增可选 `askAnswer?: AskAnswerActions`，自持 / 注入两形态同透传 `AgentMessages` |
| `packages/desktop/src/components/agent/session-panel.tsx` | 从 `state.session.answer` 合成 `askAnswer` 并下传 `AgentSessionView`（`SessionPanelState` 形状零改动——`session` 字段本就是 `UseAgentSessionState`；archi / change-chat 两面板零改动即得活卡） |
| `packages/desktop/src/views/explores/explore-detail-pane.tsx` | `ChatPane` 注入 `askAnswer={{ onAnswer: (text) => session.answer(text) }}` |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 注入同款 `askAnswer`（调试页通用会话可应答） |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 增可选 prop `askAnswer`（由 `detail-drawer` 传入）；注入时流程抽屉的 ask 卡可应答 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 增可选 prop `askAnswer` 并透传 `SessionTranscriptPanel` |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 组装流程侧 `askAnswer`：`onAnswer` → `ipc.changeFlowAnswer(root, id, text)`，`pendingToolCallId` → `activeRun?.ask?.toolCallId ?? null`；传给 `DetailDrawer` |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 + 1 patch**（现值 `0.6.35`；MUST NOT 写死字面号，判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| `AskRegistry` 三方法：`register` 成功 / 同键覆盖 / `answer` 投递原文 / take 语义（二次 `answer` `Err`）/ `remove` 后 `answer` `Err` / miss 幂等 | `crates/core/agent/src/ask_test.rs`（新） | 新建 |
| `AskMode` serde 线值（`generic` / `flow`）、`ask` 缺省读 `None`、`SessionInjections` 四字段 round-trip 与 `Eq` 链完好、`SessionOpen.session_id` 构造面 | `crates/core/agent/src/runner_test.rs` | 改造（既有三字段用例扩位） |
| `SessionKernel::new` 四参夹具（`ask_registry` 新增）；`converge` 后 `AskRegistry` 该键除名（`answer` → `Err`）；`begin_turn` 把 core session id 填进 `SessionOpen` | `crates/core/agent/src/kernel_test.rs` | 改造（`new` 调用点 42 处夹具随动 + 新增两条断言） |
| ask 参数校验：`question` 缺失 / 空白、`options` 缺失 / 非数组 / 元素非字符串 / **空数组**、合法入参 | `crates/core/orchestration/src/ask_test.rs`（新） | 新建 |
| 解耦守卫：`ask.rs` 全文零 `"finish"` 字样；`finish.rs` 的 `"ask"` 仅出现在文案字符串（源码自证用例，沿既有 grep 型守卫先例） | 同上 | 新建 |
| decision 形态 `enum` == `["backtrack","stop"]`、无 `question` / `options` property、description 含 ask 工具指路；evaluator 形态零改动 | `crates/core/orchestration/src/finish_test.rs` | 改造（三值断言 → 两值） |
| `validate_decision` 的 `"ask"` 臂仍可通过（历史 notice 反序列化 + 防御臂判据） | `crates/core/orchestration/src/decision_test.rs` | 保留（零删除） |
| `decision_prompt` 两动作 + ask 工具指路断言；`decision_answer_prompt` 全仓零命中；`evaluator_protocol` 零改动 | `crates/core/orchestration/src/prompt_test.rs` | 改造（三动作段落断言删除） |
| `answer()` 转投：`AskRegistry` 登记后 `control.answer` 投递原文；无 run → `Err` 携 change id；有 run 无等待方 → `Err`「当前无等待中的 ask」；`wait_answer` 零命中（退役） | `crates/core/orchestration/src/control_test.rs` | 改造（`answer应答原文回流挂起等待者_单次take` / `answer后停等态解除且广播失效` / `answer通道关闭仍受理并解除停等` / `answer不改写先到的终态` 四条重写；`confirm` 四条零改动） |
| `confirm` 侧受理与清理（`confirm后停等态解除且归档槽清空`） | 同上 | 保留（零改动——D6 的机械判据） |
| `AskPayload` 两新字段 serde（缺省读 `None`）；`RunUpdate::Ask { ask }` 投影与 `kind()` / `RunNotice` 不变 | `crates/core/orchestration/src/state_test.rs` | 新增 + 既有 `RunUpdate::Ask` 字面构造随变体收敛（全仓 21 处：`control` 1 / `control_test` 7 / `state` 2 / `state_test` 3 / `walker` 2 / `walker_test` 1 / 两命令测试 5） |
| `ChangeFlowControl::new(ask_registry)` 构造面换位（254 处：`control_test` 63 / `walker_test` 188 / `changes/mod_test` 1 / `change_flow/mod_test` 1 / `main.rs` 1） | 上述文件 | 夹具随动（编译器兜住，零运行期静默） |
| 决策会话：`finish_step{action:"ask"}` 不再产出（schema 拒绝）；`finalized = Ask` 载荷 → `Terminal::failed` 防御文案；决策单轮收口零 `Continue` | `crates/core/orchestration/src/walker_test.rs` | 改造（三条 ask 往返用例重写为「防御臂」与「ask 工具往返不在此层」） |
| 挂载面：`ask = None` 不追加；`Some(Generic)` / `Some(Flow)` 追加且 tier == Protocol；`finish` 位与 `ask` 位四组合独立（`finish=None, ask=Some` 仍有 ask） | `crates/infra/agent/src/sdk/mount_test.rs` | 新增 |
| `loop` 两臂：`Generic` = 阻塞 → 应答回灌 → **同一 turn 续跑**（零重开会话）；`Flow` = 先 `ask_started`（`AskPayload.tool_call_id` == 该 tool_use 的 id）→ 应答 → `ask_resolved` 恰一次；停止臂 → `Err` 且 `ask_resolved` 仍执行（两臂都清）；`AskContext` 缺席 → `is_error` 回灌 | `crates/infra/agent/src/sdk/loop_test.rs` | 新增 + 既有 `SessionInjections` 构造面扩位（11 处） |
| per-turn 铸造：`ask` 位为 `Some` 时铸 `AskContext`（`None` 不铸）；`Flow` 且无 `AskPort` 时降级为 `is_error` 回灌 | `crates/infra/agent/src/sdk/runner_test.rs` | 新增 + 6 处构造换位 |
| `assemble_injections` 逐臂 `ask` 位：explore / archi / debug → `Generic`、change（任意 role）→ `Flow`；非法名 `Err` 语义零改 | `crates/infra/agent/src/compose_test.rs` | 改造（36 处 `compose_turn` 调用加参 + 三臂断言扩位） |
| `AskPort` 透传：change 来源 sink 实现 `AskPort` 后 `Flow` 臂拿到句柄；非 change 来源为 `None` | `crates/infra/agent/src/worker_test.rs` | 新增 |
| `agent_answer`：blank root / blank session_id → `Err`；命中登记 → `Ok` 且等待方收到原文；未登记 → `Err`（文案「当前无等待中的 ask」）；重复应答 → 第二次 `Err` | `src-tauri/src/commands/exec/mod_test.rs` | 新增 |
| `change_flow_answer` 转投：`current_session()` 取 id → `AskRegistry` 投递；无 run → `Err` 携 change id；`ask` 槽由 `AskPort::ask_resolved` 清理（命令内零清理） | `src-tauri/src/commands/change_flow/mod_test.rs` | 改造（既有 `wait_answer` 驱动用例重写） |
| `bindings` 反向锚定清单：`AskMode` / `SessionInjections` 不出线 IPC（`AskPayload` 出线且含两新字段） | `src-tauri/src/bindings/mod_test.rs` | 改造（否则编译期 / 断言红） |
| ask 活卡：静态态（无 `askAnswer`）零按钮零输入；活卡态呈选项按钮与输入框、点选项即 `onAnswer(<选项>)`、输入提交即 `onAnswer(<文本>)`、空串不提交、提交后按钮禁用；`pendingToolCallId` 不匹配 → 该卡回静态；未注册工具与 `ToolPairCard` 回退零变 | `packages/desktop/src/components/agent/tool-cards.test.tsx` | 新增 |
| 动作面透传：`AgentMessages` 收到 `askAnswer` → 仅 ask 部件拿到 `onAnswer`（其余工具卡零影响）；未传 → 全卡静态 | `packages/desktop/src/components/agent/agent-messages.test.tsx` | 新增 |
| `AgentSessionView` 两形态（自持 / 注入）都透传 `askAnswer`；未传零行为变化 | `packages/desktop/src/components/agent/agent-session-view.test.tsx` | 新增 |
| `useAgentSession.answer`：`ipc.agentAnswer(root, sessionId, text)` 参数逐字；`root` / `sessionId` 为 null 时不 invoke；`Err` 进 `error` 态 | `packages/desktop/src/hooks/use-agent-session.test.ts` | 新增 |
| 流程抽屉接线：`onAnswer` → `ipc.changeFlowAnswer(root, id, text)`；`pendingToolCallId` 取 `activeRun.ask?.toolCallId` | `packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx`（+ `change-detail-view` 侧断言） | 新增 |
| 面板骨架合成：`SessionPanel` 从 `state.session.answer` 合成 `askAnswer` 下传（archi / change-chat 零改动即得活卡） | `packages/desktop/src/components/agent/session-panel.test.tsx` | 新增断言（既有用例零改动） |
| `AskCard` 既有渲染与应答路径零回归（`activeRun.ask` 仍由统一视图承载，含 `options` 按钮与自由文本） | `packages/desktop/src/views/changes/flow/run-control-panel.test.tsx` | 复用（零改动断言） |

### 不要修改

- **`crates/core/orchestration/src/control.rs::accept_pending` / `confirm()` / `wait_confirm()`**：D6 的机械判据——该泛型模板与 confirm 路径出现 diff 即说明实现越界（只有 `answer()` / `answer_tx` / `wait_answer()` 是变更面）。
- **`crates/core/orchestration/src/verdict.rs` / `transcript.rs` / `finish.rs::FinishPayload`**：evaluator 收口面与载荷提取面零改动（ask 分家不触及 checklist 收口）。
- **`crates/infra/agent/src/sdk/protect/**`**：写入保护闸的按名分派表零改动（ask 无路径 / 无写入面，无需登记；该目录出现 diff 即越界）。
- **`crates/infra/agent/src/sdk/{sandbox,context,compact,resume,normalize,web_search,bash,model_spec}.rs`**：sandbox / L1-L3 防线 / 回放 / 归一化 / server 工具 / 执行体 / 模型名解析零改动（`resume::rebuild` 的 `INTERRUPTED_RESULT` 是既有机制，只消费不改写）。
- **`crates/infra/agent/src/sdk/tools.rs`**：七通用工具的执行体与 `TOOL_NAMES` 零改动（ask 不属七工具封闭集）。
- **`crates/core/workflow/**`**：相位表、14 份 prompt md、`include_str!` 装配零改动（ask 不进 md）。
- **`crates/infra/store/**`**：store schema 与 `ChecklistItemRecord` 形状零变更；停等态仍不落库。
- **`packages/desktop/src/lib/agent-adapter.ts` / `agent-transport.ts`**：ask 的 `tool_use` / `tool_result` 走既有工具部件管线，零改动。
- **`packages/desktop/src/views/changes/flow/run-control-panel.tsx`**：`AskCard` 的渲染与应答路径零改动（本 change 只保证 `AskPayload` 多两个 additive 字段不破坏它）。
- **`packages/desktop/src/components/agent/agent-blocks.tsx`**：`FinishStepCard` / `ToolPairCard` / `WebSearchCard` 零改动（活卡住 `tool-cards.tsx`）。
- **`.dev-team/architecture/models/**`**：零新元素、零新依赖边。
- **`openspec/changes/ask-tool-in-flow/explore.md`**：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | **通用会话同 turn 阻塞应答**：`source = "explore"`（或 archi / debug）会话里模型调 `AskUserQuestion` → 该轮**不收敛**、`tool_use` 保持未配对 → `agent_answer` 投递 → `tool_result` 回灌 → 模型在同一 turn 内继续（`turn_index` 递增、零新会话 / 零 `SessionRef::Continue`）；`AskRegistry` 键 = 该会话 core session id | `loop_test`（脚本化流缝：ask 调用 → 等待 → 应答 → 续跑）+ `mount_test`（explore / archi / debug 三臂 `ask = Some(Generic)`） |
| AC-2 | **流程覆盖与双端同步**：`source = "change"` 会话拿下发的仍是同一个 `AskUserQuestion`（名称 / schema 逐字同源），额外把 `activeRun` 切 `WaitingAsk` 且 `AskPayload.tool_call_id` == 该 `tool_use` 的 id、`session_id` == 该会话 core session id；`change_flow_answer` 与 `agent_answer` 任一入口应答后：两处卡片同步收起、`activeRun.status` 回 `running`、`ask` 槽清空 | `loop_test`（Flow 臂：`ask_started` → 应答 → `ask_resolved` 恰一次）+ `change_flow/mod_test`（两入口各一条）+ `control_test`（快照两面） |
| AC-3 | **显式禁用面**：`ask = None` 的会话 `RunStarted.tools` 不含 `AskUserQuestion`、`definitions()` 无该挂载项；模型硬调即 `tool_unavailable`（`SystemNotice` + is_error ToolResult，run 不中断） | `mount_test`（四组合）+ `loop_test`（幻觉调用被拒） |
| AC-4 | **解耦**：`orchestration/ask.rs` 全文零 `finish` 字样；`finish.rs` 的 `"ask"` 仅出现在文案字符串；`mount.rs` 的两条追加分支互不引用；`finish = None` 的会话仍有 ask、`ask = None` 的会话仍有 `finish_step`（若 `finish` 位在）；`finish_step(Decision)` 的 `enum` == `["backtrack","stop"]` 且无 `question` / `options` property | `ask_test`（源码自证）+ `finish_test` + `mount_test` 四组合 |
| AC-5 | **停止语义**：ask 阻塞期间停止 → 轮收敛 `stopped`；转录留下该 `tool_use` 的未配对记录，`resume::rebuild` 补 `INTERRUPTED_RESULT`（续会话可重建）；`AskRegistry` 槽由 `converge` 除名（停止后 `answer` → `Err`）；`Flow` 臂的取消分支仍执行 `ask_resolved`（停等态不残留） | `kernel_test`（converge 除名）+ `loop_test`（停止臂）+ `resume_test`（既有补齐用例不回归） |
| AC-6 | **参数校验**：`question` 必填非空；`options` 必须为**非空**字符串数组（`minItems: 1`）；校验 `Err` 文案自足（字段名 + 期望 + 当前值）；`AskCard` 与聊天活卡的自由文本应答不受影响 | `ask_test`（六态）+ 前端活卡用例（选项 + 文本两路并存） |
| AC-7 | **重复应答幂等**：同一 ask 二次应答 → `Err`（「当前无等待中的 ask」），等待方只收到一次原文，不产生第二次回灌；`remove` 后的迟滞应答同样 `Err` | `ask_test`（take 语义）+ `loop_test`（单次回灌）+ `exec/mod_test`（命令面二次调用） |
| AC-8 | **单通道**：`ChangeFlowControl` 不再有 `answer_tx` 槽与 `wait_answer()`（`grep` 零命中）；`answer()` 与 `agent_answer` 都只投 `AskRegistry`；`accept_pending` / `confirm()` / `wait_confirm()` 逐字未改（`confirm` 侧四条既有用例零改动通过） | `control_test` + `grep` 复核 + `change_flow/mod_test` |
| AC-9 | **载荷 additive 与出线**：`AskPayload` 增 `toolCallId` / `sessionId`（两字段出线，缺省 `null`）；`RunUpdate::Ask { ask }` 为载荷单源；`RunNotice` 六变体与 `kind()` 零变；`bindings:export` 后再生成零 diff；`AskMode` / `SessionInjections` 不出线 IPC | `state_test` + `bindings/mod_test` + `bindings:export` 后 `git diff` |
| AC-10 | **命令面**：`agent_answer(root, session_id, text)` 为薄包装；blank 参数 `Err`；未登记 `Err`；命中 `Ok`；命令落在 `commands/exec` 轨道并登记于 `all_commands!` | `exec/mod_test`（四态）+ 源码复核（`all_commands!` 含该命令） |
| AC-11 | **前端双态卡**：注入应答动作面 → ask 卡可点（选项按钮与自由文本输入，提交后本地禁用防连点）；未注入 → 静态只读卡（与改造前逐字相同，旧文案「请在下方输入框作答」不再出现）；`pendingToolCallId` 给出时仅该 id 的卡为活卡；未注册工具与 `ToolPairCard` 回退零变；`AgentMessages` / `AgentSessionView` 未传动作面时全卡静态 | `tool-cards.test.tsx` + `agent-messages.test.tsx` + `agent-session-view.test.tsx` |
| AC-12 | **决策协议收窄**：决策会话单轮收口（ask 不再开新一轮）；`finish_step(Decision)` schema 不含 `ask`（模型硬编被校验拒绝）；`finalized = Ask` 载荷出现即 `Terminal::failed`（防御文案指路 ask 工具）；`decision_prompt` 两动作 + ask 工具指路；`decision_answer_prompt` 全仓零命中；`DecisionAction::Ask` 与 `validate_decision` 的 `"ask"` 臂保留（历史 notice 可反序列化） | `walker_test`（防御臂 + 单轮收口）+ `finish_test` + `prompt_test` + `grep` |
| AC-13 | **消费点接线**：explore / archi / change-chat / debug 四处通用会话可应答（走 `agent_answer`）；change 流程抽屉可应答（走 `change_flow_answer`，`pendingToolCallId` 取 `activeRun.ask.toolCallId`）；只读历史重放零应答入口 | 各视图用例（`explore-detail-pane` / `session-panel` / `agent-debug-view` / `session-transcript-panel`） |

> **验收表口径**：AC-1…AC-13 是本变更的**能力验收面**（每条对应 `specs/` 下某个 delta requirement 的行为判据）。静态检查链、全量测试与版本递增**不进验收表**——它们是全仓既有的交付门，由主基线承载而非本变更引入（见下「验证命令」）。

### 验证命令

**能力验收**（逐包定向测试）：

```powershell
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p dev-team
pnpm -C packages/desktop exec vp test run src/components/agent src/hooks/use-agent-session.test.ts src/views/changes/flow
```

**交付门**（全仓既有门，非本变更能力判据；版本行按「落地时 HEAD +1 patch」相对递增核对）：

```powershell
cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml --all-targets
pnpm -C packages/desktop run bindings:export
pnpm -C packages/desktop run check      # server:check → bindings:export → api:check → client:check
pnpm -C packages/desktop test
git diff HEAD -- packages/desktop/package.json   # version 行恰 +1 patch
```

> **验收面口径**：验收止于**单元测试**。真机 E2E（真实 provider 端点下的 ask 往返）不属本工作流自动化验证面；`loop_test` 以 rig `test-utils` 流缝脚本化覆盖「阻塞 → 应答回灌 → 同 turn 续跑 / 停止截停 / Flow 覆盖清态 / 上下文缺席回灌」四条路径，这四条即本变更的全部行为面。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 主基线未吸收 `finish_step` 归档 delta**（`grep -rn "finish_step" openspec/specs` 零命中，但代码已在产线） | 归档时 MODIFIED 整块替换会把 `finish_step` 全部条款冲掉；若按纯基线原文撰写 delta，合并出的 spec 与代码不一致 | 高 | 本 delta 的 MODIFIED 正文**以「主基线原文 + finalize 归档 delta 合并后形态」为底稿**再叠加本次改动（见「能力」末节）；归档顺序要求 finalize 的 delta 先合并或同批合并；评审时以「合并后文本 vs 代码」为准而非「delta vs 基线」 |
| **R2 core session id 下传是 explore 未点明的必需缝**（`SessionOpen` 加 `session_id`） | `SessionOpen` 字面构造 28 处（core 5 / infra 23）随动；若漏改则编译期全暴露（无运行期静默） | 高 | 该字段是引擎侧登记 ask 的唯一来源（`StopRegistry` 的键由 core 自己持，ask 的登记方在引擎侧）；编译器兜住全部构造点；proposal 明写选择理由（`TurnQuestion` 不动，会话级标识一次下传） |
| **R3 双入口与清理单点**（`change_flow_answer` / `agent_answer` 都投 `AskRegistry`，清理搬到执行体） | 清理若两处都做 → 双广播 / 状态倒灌；若都不做 → 停等态残留（正是前置 change 刚修的 bug 模式） | 中 | D5 / D6：命令层零清理、执行体单点清（`ask_resolved` 恰一次）；`accept_pending` 与 `confirm` 零改动（`control_test` 四条既有用例零改动通过即判据）；`loop_test` 断言 `ask_resolved` 调用次数恰 1 |
| **R3b `ChangeFlowControl::new` 加参波及 254 处构造点**（`control_test` 63 / `walker_test` 188 / 两命令测试各 1 / `main.rs` 1） | 机械改动量大，易引入无关断言漂移 | 高 | 构造面只加一个 `Arc` 实参，编译器兜住全部点；`control_test` 与 `walker_test` 用文件内共享夹具（`fn control() -> Arc<ChangeFlowControl>`）压平，改写时保持每用例一条显式 `Arc<AskRegistry>`（不引入全局静态，避免测试间串表） |
| **R4 停止期间 `select!` 与清理的耦合** | 外层泵 drop loop future 时不执行清理代码 → `Flow` 停等态可能残留到 run 收口 | 中 | `ToolExec::Ask(Flow)` 的 `select!` 显式带停止臂（`handle.wait_requested()`），两臂返回后都调 `ask_resolved`；`AskRegistry` 槽另由 `converge` 的 `remove` 兜底（双保险） |
| **R5 `finish_step(Decision)` 收窄对历史转录的影响** | 旧转录里的 `finish_step{action:"ask"}` 记录不再产出，但历史 notice 反序列化仍需可读 | 低 | D8：`DecisionAction::Ask` 与 `validate_decision` 的 `"ask"` 臂保留；walker 防御臂保留（载荷出现即 `Terminal::failed` 而非 panic / 静默） |
| **R6 前端活卡判据的误判面** | 只有 `state === 'input-available'` 时，历史崩溃残留的未配对 `tool_use` 也会被呈为活卡；用户点了会收到 `Err` | 中 | 流程侧用 `pendingToolCallId`（来自 `AskPayload.tool_call_id`）精确对账，仅正在等的那个 `tool_call_id` 为活卡；通用侧不注入 `pendingToolCallId` 时按未配对判活（该会话的等待以「会话 running + 工具未配对」为准，停止 / 收口后 `running` 为 false，`AgentSessionView` 不注入应答面即回静态）；应答 `Err` 以既有错误横幅呈现（不静默） |
| **R7 `AskRegistry` 与 `StopRegistry` 双表的一致性** | 两会话表在 `begin_turn` / `converge` 两处登记除名，若只加一半则出现「能答不能停」 | 低 | 两张表共用同一 `session_id` 与同一 `converge` 出口（`registry.remove` 旁增一行 `ask_registry.remove`）；`kernel_test` 断言轮收口后两表都查不到该键 |
| **R8 装配位漏置导致 ask 静默消失**（`SessionInjections::default()` 的 `ask = None`） | 某来源会话拿不到 ask，而模型仍按 md / schema 期待它（表现为「工具不可用」而不是崩溃） | 中 | `assemble_injections` 逐臂显式置位（含 debug 臂不再用 `default()`）；`compose_test` 三臂 + debug 臂的 `ask` 位逐条断言；`None` 语义在 spec 明写为「显式禁用」而非「未装配」 |
| **R9 版本与绑定纪律** | 交付门红 | 低 | `package.json` `version` 按「落地时 HEAD 现值 +1 patch」相对递增（MUST NOT 写死字面号）；`AskPayload` 两字段出线 → `bindings:export` 后再生成物一并提交；`AskMode` / `SessionInjections` 不出线 IPC（反向锚定守）；零新 crate ⇒ 架构模型零改动 |
| **R10 首期第三层（显式禁用）只建位不接线** | 若后续有人按「已实现的禁用面」假设去接线，可能发现动作面缺装配（如 `None` 时前端仍注入了应答卡） | 低 | 位与分支在 mount / loop 两处到位并有四组合用例；spec 明写「首期无禁用场景，位建好不接线」；前端活卡只在 `definitions` 下发了该工具时才可能出现（无下发即无 `tool_use`） |
