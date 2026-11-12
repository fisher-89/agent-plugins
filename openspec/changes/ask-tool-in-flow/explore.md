# 流程内 ask 工具（双端同步应答）探索

workflow_type: requirement

## 本文范围

本文只谈 **ask 工具化**（新增阻塞工具 + 改 `finish_step` 协议 + 通用应答入口）。

停等态残留的两处 bug（ask 应答后不解除 / phase 确认后不解除）已修复并合入
（commit `6708e14875a4`，bump 0.6.35），其探索笔记已随修复完成删除。本文只消费
该修复的产出：**`ask` 槽在应答受理时会被清空**这条契约。

## 当前结论（第八轮定稿：A + Y-1 已定，具备启动条件）

> **第七轮推翻了第五/六轮的定位。** 前几轮把 ask 当成「流程专属能力，通用来源
> 传 None 即可」。用户澄清后方向反转：
>
> - **通用聊天（explore / archi / debug）才是 ask 的基准场景**，效果 = 聊天窗出现
>   选择卡片；
> - **change 流程要覆盖**这个工具（同名替换为流程版，额外改 change 状态）；
> - **个别场景可显式注入「不许用」**（None）。
>
> 这使改动性质和顺序都变了：不是「先做流程版，通用版推迟」，而是**先有通用版，
> 流程版是它的覆盖**。详见「第七轮」。
>
> **第七轮发现的硬缺口**：通用会话（`AgentSession`）没有「turn 内等待应答」通道
> ——只有 observations / questions / handle 三件套，`questions.recv()` 又是一轮一命。
> 所以前几轮「通用 ask 可推迟」的判断错了：这条通道是通用版的地基。
>
> **第八轮已定：选 A（`AskRegistry` 下沉 `core/agent`，键 = session id，与
> `StopRegistry` 同型）。** 通用地基的形状、装配链、三层改动与 AC 草案见「第八轮」。
>
> **第八轮后半已定：Y-1**（`change_flow_answer` 只转投 `AskRegistry`，流程状态清理
> 搬到 ask 执行体）。核实后改动比预想小：`accept_pending` 是泛型模板且 `confirm`
> 仍需它，故**不拆**——只有 `answer()` / `wait_answer()` 两个方法被替换。

## 第七轮：方向修正 —— 通用聊天是基准，change 流程是覆盖

### 澄清后的语义分层

```
通用聊天（explore / archi / debug）
  ask 工具 ← 基准实现
  效果：聊天窗出现「选择卡片」（不是静态卡，是可点的活卡）
  应答：用户在卡片上点选项 / 填自由文本 → 回灌为 tool_result → 同一 turn 继续

change 流程
  ask 工具 ← 同名覆盖：同一个名字 "AskUserQuestion"，流程侧装配时替换为流程版
  额外效果：除聊天窗活卡外，change 状态切 WaitingAsk（详情页 AskCard 同步）
  应答：两处任一入口（change_flow_answer / 通用 agent_answer）都通

个别场景（如某些自动化步）
  显式注入 None → 该会话拿不到 ask 工具（成员校验硬禁用，模型调用即拒）
```

关键点：**覆盖不是「两套工具」，而是「同名工具的两种执行体」**。这样：

- 模型视角零差异（同一个工具名同一个 schema）；
- 历史转录无需区分来源（都是 `AskUserQuestion` 的 tool_use）；
- 前端卡只需看 `tool_call_id` 是否在等待（不需要分辨来源）。

### 真正的架构缺口：通用会话没有「turn 内等待」通道

这是本轮核实的**关键发现**，也是第七轮唯一新增的成本：

```
现状：AgentSession 只有三件套
  observations  引擎 → 内核（事件回流）
  questions     内核 → 引擎（一轮一命：recv 一次即服务完整轮）
  handle        终止信号

缺：引擎 → 等待方（ask 的应答回流）——即「turn 内反向注入」
```

`session_pump` 的代码写得很明确：

```rust
async fn session_pump(...) {
    let Some(question) = questions.recv().await else { return };   // 只收一次
    ... loop::run(...) ...                                         // 跑完整轮
}
```

所以通用会话的 ask 必须在 `loop::run` 内部阻塞等待，而应答要从**外部**（IPC 命令）
送进来。现有 `AgentSession` 没有这条通道——这正是前几轮 Q2 讨论的「AskRegistry vs
扩 RunHandle」，只是前几轮误判它可以推迟，实际上**它是通用版的地基**。

### 于是 Q2 从「可推迟」回到「必须先决」

```
选项 A：AskRegistry（键 = core session id，与 StopRegistry 同址托管）
  kernel.begin_turn 登记 / converge 除名（与 StopRegistry 完全同型）
  应答入口：agent_answer(root, session_id, text)
  流程侧：change_flow_answer 先 current_session() 取 id，再转投 AskRegistry
  代价：新类型 + 装配链 + 5 处测试夹具

选项 B：扩 RunHandle（加一个应答槽）
  RunHandle 已在 AgentSession 与 LoopTurn 里，零构造点波及（derive Default 兜住）
  StopRegistry 加一个 getter 即可寻址
  代价：RunHandle 语义从「终止信号」变成「运行控制面」（改名 RunControl 更诚实）

选项 C：AgentSession 增第四件套（asks: mpsc::Receiver<AskAnswer>）
  与 questions 对称，最贴近现有形状
  代价：mpsc 不适合「请求-应答配对」（需要自带 id 对账），oneshot 表更合适 ⇒ 退回 A
```

**倾向 A**：`StopRegistry` 已证明「session id → 句柄」这个形态可行，且流程侧的
`change_flow_answer` 签名零改动（内部转投）。B 的零波及很诱人，但把终止信号和
应答通道塞进同一个类型，会让「停止」与「应答」的语义边界变模糊。

### 于是改动顺序也反转了

```
第五/六轮（旧）：先做流程版，通用版推迟
第七轮（新）：
  step 1  通用版地基：应答通道（A/B 选型）+ 通用 ask 工具 + 聊天窗活卡
  step 2  流程版覆盖：ChangeFlowSink 实现覆盖执行体 + WaitingAsk + 详情页同步
  step 3  （可选）注入 None 的禁用面
```

step 1 与 step 2 可以同 change 做，但 **step 1 是 step 2 的前提**——没有应答通道，
流程版也无处阻塞。

### 「覆盖」的实现形态

```
mount::definitions(injections, prompt)
  ├ tools 名单过滤
  ├ ask 挂载项：
  │    通用来源     → ToolExec::Ask（执行体：等 AskRegistry 的 oneshot）
  │    流程来源     → ToolExec::AskFlow（执行体：publish(Ask) + 等 oneshot + 清状态）
  │    注入 None    → 不挂（成员校验硬禁用）
  └ finish.is_some() → 追加 finish_step
```

两种执行体共享同一个 `TOOL_NAME` 与同一份 `parameters()`（schema 单源在
`orchestration/ask.rs`），差别只在「是否额外广播流程状态」。这正好延续第二轮定的
「agent 视角无差别」约束。

**但 `SessionInjections` 需要一个位来表达「禁用它」**——这与第六轮「不需新字段」
相反：

```
新增 Option<AskMode>（None = 不挂 / Some(Generic) = 通用 / Some(Flow) = 流程覆盖）
或复用现有形状：tools 名单里不含它即禁用（但 ask 是无条件追加，无法用名单表达）
```

所以这里要么加一个 `ask: Option<AskMode>` 位，要么让「无条件追加」变成「按位追加」。
倾向：加位（`None` 显式禁用与「未装配」可分辨，与 `finish` 同位）。

## 第八轮（定稿）：选 A —— AskRegistry 下沉 core/agent

### 裁决

```
A  AskRegistry（键 = core session id，与 StopRegistry 同址托管）
   流程侧 change_flow_answer 签名零改动：内部 current_session() 取 id → 转投
```

### 形状（与 StopRegistry 完全同型）

```rust
// core/agent/src/ask.rs（新）
pub struct AskRegistry {
    slots: Mutex<HashMap<String, oneshot::Sender<String>>>,   // 键 = core session id
}

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

**为什么 key 是 session id 而非 `(root, change_id)`**：core/agent 不认识 workspace
root 与 change——这是它边界中立契约的一部分。流程侧的两段寻址由壳层翻译：

```
change_flow_answer(root, change_id, text)
  ├ control.current_session(root, change_id) → Option<session_id>
  └ ask_registry.answer(session_id, text)
```

### 装配链（三处注入点，沿 StopRegistry 先例）

```
src/main.rs
  app.manage(Arc::new(StopRegistry::default()));
  app.manage(Arc::new(SessionStreamRegistry::default()));
+ app.manage(Arc::new(AskRegistry::default()));          ← 三层托管同型

compose_turn(..., ask_registry: Arc<AskRegistry>, ...)   ← 签名加一参
  └ SessionKernel::new(sink, registry, streams, ask_registry)

SessionKernel::begin_turn
  登记的是 handle（停止）+ 本轮不登记 ask（ask 是轮内零次或多次，按需登记）

ask 执行体（infra/agent）
  阻塞前：ask_registry.register(session_id, tx)   ← 需要 registry 句柄
  阻塞中：tx 已被 answer() take / 或取消置位
  解除后：无需 remove（answer 的 take 已消费；取消路径由轮收敛 remove 兜底）
```

关键：ask 执行体需要 `Arc<AskRegistry>` + `session_id`，两者都能进 `LoopTurn`
（session_id 已在，registry 沿 `with_*` 注入缝下行）。

### `SessionInjections` 加位（第七轮结论保留）

```rust
pub struct SessionInjections {
    pub preamble: Option<String>,
    pub tools: Option<Vec<String>>,
    pub finish: Option<FinishSpec>,
+   /// ask 工具档位：None = 不挂（显式禁用）/ Some(Generic) = 通用 / Some(Flow) = 流程覆盖
+   #[serde(default)]
+   pub ask: Option<AskMode>,
}

pub enum AskMode { Generic, Flow }   // Copy + Eq 闭合枚举，与 FinishSpec 同型
```

`compose::assemble_injections` 的置位规则：

```
change 来源（有 RunEventSink 的轨道） → Some(AskMode::Flow)
explore / archi / debug              → Some(AskMode::Generic)
需要禁用的步（首期无，留扩展位）      → None
```

### 执行体分派（覆盖的实现形态）

```
mount.rs
  ToolExec::Ask(AskMode)     ← 一个变体带档位，不做两个变体
                                 （schema 单源，仅执行体行为不同）

loop.rs 分派
  ToolExec::Ask(Generic) => {
      register(session_id, tx);
      select! { r = rx => r, _ = handle.wait_requested() => Err(...) }
  }
  ToolExec::Ask(Flow) => {
      sink.publish(RunUpdate::Ask { question, options });    // → WaitingAsk
      register(session_id, tx);
      select! { r = rx => { sink.publish(解除) ; r }, _ = cancel => Err(...) }
  }
```

`Flow` 的「解除」复用 `flow-waiting-state-residue` 已定的形态：清 status + 载荷槽
（那套逻辑在 `ChangeFlowControl::accept_pending`）。**注意**：`Flow` 路径的应答
若经 `change_flow_answer`，解除已在 `accept_pending` 内完成；若经 `agent_answer`
（通用入口）直达 `AskRegistry`，则**没人清流程状态**。这是一个必须处理的重复入口问题：

```
选项 1  流程会话只允许 change_flow_answer（agent_answer 对流程会话 Err）
         ⇒ 单一入口，解除逻辑单点。简单，但前端要按来源选命令

选项 2  agent_answer 也走同一受理逻辑：先查是否是流程会话
         ⇒ 需要 core/agent 反向认识编排状态（破坏边界）

选项 3  流程 ask 执行体自己负责解除（阻塞返回后清状态），
        两个入口都只投递 oneshot
         ⇒ 边界干净：ask 执行体在解除点上，它知道自己是 Flow
```

**倾向选项 3**：`ToolExec::Ask(Flow)` 的 `select!` 两臂返回后都清状态并广播；
`change_flow_answer` 与 `agent_answer` 都只做「投递」。这样：
- 两个入口天然幂等（`AskRegistry.take` 语义，先到者胜）；
- 流程状态清理与「应答从哪来」解耦；
- 但要注意 `accept_pending` 的清理逻辑会与这里重复——需要**把清理从
  `ChangeFlowControl` 挪到 ask 执行体**，或让 `accept_pending` 只管投递。

这是第八轮唯一新增的待决点（见下节）。

### 待决 Y（新）：`accept_pending` 的清理职责归属

```
现状（6708e14875a4 之后）
  ChangeFlowControl::answer/confirm
    → accept_pending：锁内取 oneshot + 清 status/ask/archive + 广播
    → 锁外 send

A 落地后
  流程 ask 的 oneshot 存在 AskRegistry（不在 ChangeFlowControl.answer_tx）
  ⇒ ChangeFlowControl::answer 要么保留 answer_tx（旧路径），要么改成转投 AskRegistry

若保留 answer_tx：两套通道，ask 工具要选一条（无意义的分叉）
若改成转投：accept_pending 的清理逻辑失去落点，必须搬走
```

三条路：

| 路 | 形态 | 评价 |
| --- | --- | --- |
| Y-1 | `change_flow_answer` 只转投 `AskRegistry`；流程状态清理搬到 ask 执行体（选项 3） | **推荐**：单点清理、两入口幂等 |
| Y-2 | `change_flow_answer` 仍走 `ChangeFlowControl`，ask 工具注册到 `answer_tx` | 通道不统一，通用侧无法复用；且 `answer_tx` 是 `Mutex<Option<>>` 单槽，无法与 AskRegistry 并存 |
| Y-3 | 两者并存的过渡态 | 双真相，必然漂移 |

**已裁决：Y-1。**

### 但核实后发现 Y-1 的改动比预想更小（好消息）

`accept_pending` 是**泛型模板**，`answer` 与 `confirm` 共用：

```rust
fn accept_pending<T>(
    &self, root, change_id,
    slot: impl Fn(&RunEntry) -> &Mutex<Option<oneshot::Sender<T>>>,  // ← 泛型槽位
    waiting: ChangeRunStatus,
    missing: &str,
) -> Result<(oneshot::Sender<T>, broadcast::Sender<RunNotice>), String>
```

所以 Y-1 **不需要拆分它**——只需让 `answer` 不再调用它：

```
answer()   → 改：只转投 AskRegistry（不再碰 answer_tx / 不再清状态）
             清理搬到 ask 执行体（ToolExec::Ask(Flow) 的 select! 之后）
confirm()  → 原样：继续用 accept_pending（confirm 与 ask 无关，
             wait_confirm 机制不变）
```

于是波及面从「重构 accept_pending」缩为：

```
control.rs
  answer()            改：不再 accept_pending，改为 current_session() → AskRegistry
  answer_tx 槽        保留还是删？—— confirm_tx 仍需泛型模板，但 answer_tx 不再被
                      wait_answer 使用（wait_answer 也要删，见下）
  wait_answer()       删：walker 不再用它（ask 阻塞搬进工具执行体）
  accept_pending      **不动**（confirm 继续用）；但它的 `ask = None` 那行
                      对 confirm 路径已无意义（ask 槽不再由它清）
  
walker.rs
  :3294 wait_answer 调用    删（随 finalized==Ask 分支一起删）
  
已合入的 6708e14875a4 影响
  ✅ 保留：confirm 的清理、锁内先清再 send 的顺序、RunNotice::Step 作失效信号
  ⚠️ 改动：answer 的清理移到 ask 执行体；wait_answer / answer_tx 退役
  ⇒ flow-waiting-state-residue 的两条 Scenario 中，
     ask 那条要改述「清理发生在工具执行体返回后」，
     confirm 那条零改动
```

**这比「重构 accept_pending」轻得多**——已合入代码里只有 `answer` / `wait_answer`
两个方法被替换，`confirm` 侧与锁序纪律全部保留。

### 三层改动清单（定稿）

```
第一层：通用地基（step 1，流程版的前提）
core/agent
  ask.rs（新）          AskRegistry：register / answer / remove
  kernel.rs             SessionKernel 持 Arc<AskRegistry>；converge 时 remove
  runner.rs             SessionInjections + ask: Option<AskMode>；AskMode 词汇
  lib.rs                导出 AskRegistry / AskMode
infra/agent
  sdk/mount.rs          ToolExec::Ask(AskMode) + 挂载项（Protocol tier）
  sdk/loop.rs           分派臂（Generic 臂：register + select!）
                        LoopTurn + ask_registry: Option<Arc<AskRegistry>>
  sdk/runner.rs         PumpPayload + ask_registry 透传
  worker.rs / lib.rs    注入缝：EngineFacade::with_ask_registry
  compose.rs            compose_turn 收 Arc<AskRegistry>，assemble_injections 置位
壳层
  main.rs               manage(Arc<AskRegistry>)
  commands/exec         agent_start 传 registry；新增 agent_answer(root, session_id, text)
前端
  types/generated        agent_answer 命令
  components/agent       tool-cards: ask 活卡（点选项 / 输文本 → onAnswer）
                         agent-messages / agent-session-view: toolCardHandlers prop
  hooks/use-agent-session  answer 动作
  views/explores / views/architecture  注入 onAnswer → ipc.agentAnswer

第二层：流程覆盖（step 2）
core/orchestration
  ask.rs（新）          TOOL_NAME / description / parameters / validate
                        MUST NOT use crate::finish（解耦守卫）
  port.rs               AskPort trait（publish + 解除语义）
  control.rs            answer() 改为转投 AskRegistry；删 wait_answer / answer_tx
                        accept_pending **不动**（confirm 继续用）；清理搬到执行体
  finish.rs             Decision schema 收两值 + description 改写
  decision.rs           DecisionAction::Ask 保留（历史兼容），生产不可达
  prompt.rs             decision_prompt 两选一；删 decision_answer_prompt
  walker.rs             删 finalized==Ask 分支；保留防御臂
infra/agent
  sdk/loop.rs           ToolExec::Ask(Flow) 臂：publish + register + select! + 清状态
壳层
  commands/change_flow  ChangeFlowSink 实现 AskPort；change_flow_answer 转投 AskRegistry

第三层：显式禁用（step 3，可留扩展位）
  目前无禁用场景；ask: None 的位与分支先建好，不接线
```

### AC 草案

```
AC-1  通用会话 ask 工具：模型调 AskUserQuestion → 聊天窗出现选择卡片 →
      用户点选项 → 同一 turn 内 tool_result 回灌 → 模型继续（不重开会话）
AC-2  流程会话覆盖：同一工具名，额外把 change 切 WaitingAsk；详情页 AskCard 与
      聊天窗卡片同源呈现；任一入口应答后两处同步收起且状态回 running
AC-3  禁用面：注入 ask = None 的会话，AskUserQuestion 不在 RunStarted.tools，
      调用即 tool_unavailable
AC-4  解耦：ask 与 finish_step 互不引用（grep 可验）；finish = None 的会话仍有 ask
AC-5  停止：ask 阻塞期间停止 → 轮收敛 stopped，转录留未配对 tool_use，
      由 resume::rebuild 的 INTERRUPTED_RESULT 补齐
AC-6  options 非空：ask 参数校验要求至少一个选项；前端卡片 option 按钮可用，
      同时保留自由文本输入
AC-7  重复应答幂等：同一 ask 二次应答 → Err，不产生第二次回灌
```

### 仍需你拍的一件事

**已裁决 Y-1**，且核实后它的实际改动很小：

```
会动已合入代码的两处
├─ control.rs::answer()      改为转投 AskRegistry（不再 accept_pending）
└─ control.rs::wait_answer()  删（ask 阻塞搬进工具执行体）

零改动
├─ accept_pending             confirm 继续用，原样保留
├─ confirm() / wait_confirm   与 ask 无关
└─ 锁内先清再 send 的顺序纪律   confirm 路径照旧（ask 侧同纪律移到执行体）
```

唯一需要重述的是 `flow-waiting-state-residue` 里 **ask 那一条** Scenario
（清理时点从「应答命令内」改为「执行体返回后」）；confirm 那条零改动。

## 历史结论（第六轮，已被第七轮的定位修正取代）

判据 = 能否把 proposal 的「变更范围 + AC」写完整而不留问号。结论：满足——
核心裁决已定、机制有同位先例、首期边界收敛：

```
已裁决（第五 / 六轮）
├─ X-2  ask 独立成阻塞工具，finish_step 的 enum 收为两值
├─ options 非空（沿用现状校验；用户仍可自由文本作答）
├─ 与 finish_step 彻底解耦（并列模块、互不引用、挂载互不依赖）
├─ 「覆盖自带 ask」= 定义唯一真源 + provider 侧不变量（当前无清除对象）
└─ 聊天窗不渲染可接受（首期 AskPayload 零扩展）

已完成的前置
├─ 停等态残留修复（两处 bug）     已合入 6708e14875a4（ask/confirm 解除，0.6.35）
└─ agent-finalize-tool-for-flow-steps 已归档 8d59f3b8186b（finish_step 工具化）

前置二带来的三个前提变化
├─ action:"ask" 不再是文本协议 → 已由 finish_step 的 enum 提交（parse_decision 已删）
├─ TOOL_NAMES 不再是判定面   → 挂载清单 + ToolTier::Protocol + 动态追加已是现成机制
└─ infra → orchestration 依赖已打通（mount.rs 直接引 orchestration::finish）

于是原 Q1/Q2
├─ Q1（收窄面砍掉 ask）→ 已解：照 finish_step 形状挂载，不进 TOOL_NAMES
└─ Q2（应答通道选型）  → 降级：流程来源可零新增通道，通用来源可整体推迟

可行性（第四轮核实）
└─ AskPort 可驻 core/orchestration（与 finish.rs 对称），应答通道不必下沉
   core/agent —— 原 Q2「AskRegistry vs 扩 RunHandle」的整个选型作废

首期范围（第五 / 六轮定稿）
├─ 做：流程内 ask 工具（decision 角色可用）+ finish_step 收两值
├─ 做：ask 与 finish_step 彻底解耦（grep 可验的零互引）
└─ 不做：通用 ask / 聊天窗行内卡 / agent_answer 命令 / AskPayload 扩展
```

**结论：具备启动条件**——核心机制有同位先例（`finish_step`）可照搬，首期改动
边界收敛到「一个新纯函数模块 + 一个 ToolExec 臂 + 一个 AskPort trait」。
详见「第五轮（定稿）」。

## 来源

用户在 steer 探索的续谈中提出：**「在流程中，使用定制逻辑替换 AskUserQuestion 工具，改为聊天窗口和变更详情同步展示，用户可选择任意一处回复」**。

这条同时回答了 `agent-finalize-tool-for-flow-steps.md` 的待决 3（ask 与 finalize 的冲突）。

## 先纠正一个前提

`AskUserQuestion` 在 SDK 引擎里**不存在**。核实：

```
$ grep -rn "AskUserQuestion" --include=*.rs crates src
（零命中）

$ grep -rn "AskUserQuestion" --include=*.ts --include=*.tsx src
src/components/agent/tool-cards.tsx        ← 静态卡（历史转录渲染）
src/components/agent/tool-cards.test.tsx
src/components/ui/questionnaire.tsx        ← 二期留痕，未接线
```

`openspec/specs/desktop-agent-execution/spec.md` 明文：

> **交互工具限制**：SDK 工具面为封闭清单（无 ask 类工具）——agent 需用户决策时以**问题文本作为本轮最终消息收口**，应答经 Continue（store 全史转录重建）同会话续轮；explore / chat 页对转录中既存 AskUserQuestion ToolUse 的静态可读卡渲染与 composer 文本输入续话呈现不变；questionnaire 组件接线为二期

所以「替换 AskUserQuestion」的准确含义是：**从零建一个流程内 ask 工具**，顺带替换掉那张历史遗留的静态卡（它现在的文案「请在下方输入框作答」在多数页面其实是误导——那个输入框不一定在）。

## 现状：只有 decision 能 ask，且代价极重（**第二轮时的代码状态**）

> 下文图中的 `parse_decision` / `action:"ask"` 文本协议已由
> `agent-finalize-tool-for-flow-steps` 替换为 `finish_step` 工具提交。
> 「只有 decision 能 ask」与「停等挂在 walker 而非 turn」两条结论仍然成立。

```
decision_session（walker.rs）
  └ run_worker → 一轮决策会话
       └ 模型文本输出 {action:"ask", question, options}
            └ parse_decision 得 DecisionAction::Ask
                 └ guard.emit(RunUpdate::Ask{question, options})   ← 此时 turn 已收口
                      └ guard.wait_answer().await                   ← 挂在 walker 里，不是 turn 里
                           └ answer 回流 → prompt = "用户应答：{answer}..."
                                └ continue_session = Some(sid)
                                     └ 再跑一整轮决策会话（store 全史重建）
```

三个代价：

| 代价 | 说明 |
| --- | --- |
| 只有 decision 能 ask | executor / evaluator 遇到歧义只能「在最终消息里写个问题然后收口」，用户看到的是一个已收口的 step，无法中途应答 |
| 停等挂在 walker 而非 turn | `wait_answer` 在 `decision_session` 的循环里，turn 早已结束。所以此时会话库态是「已收口」，转录面板看不到任何「正在等待」的迹象 |
| 应答靠 Continue 重开一轮 | 用户答一句话 → 整轮重建（`resume::rebuild` 全史）+ 一次完整模型请求 |

## 裁决（第二轮）：ask 是通用能力，流程只是额外改状态

用户拍定：

> ① 通用的 ask 是在 agent messages 显示提问卡片，等待用户回答继续 loop，流程内 ask **额外**将 change 切换到等待用户决策的状态；**二者在 agent 视角无差别**。
> ② 删除 `action: "ask"` 协议，仅 ask 工具可用。

这两条把 ask 从「流程专用」升为**会话基建能力**，并砍掉旧文本协议。语义分层因此变得非常干净：

```
                        ┌──────────────────────────────┐
agent（模型）视角：      │  ask(question, options)       │   ← 一个普通工具调用，阻塞
                        │  → 返回答案文本 → 继续 loop   │      无「我是不是流程内」的分支
                        └──────────────┬───────────────┘
                                       │
                        ┌──────────────┴───────────────┐
编排视角：               │ 通用（explore / archi / 调试） │ → 仅 UI 卡片，wait_answer 阻塞
                        │ 流程（change step）            │ → 额外 publish RunUpdate::Ask
                        └──────────────────────────────┘      → change 变 WaitingAsk
                                                              → 详情页 + 聊天窗双端渲染
```

「agent 视角无差别」这条约束很重要，它决定实现形状：**ask 的实现体不能分叉**。差别只在 `RunEventSink` 有没有被注入——有 sink 就额外发一条 `RunUpdate::Ask`，没有就只是阻塞等应答。

```rust
// infra/agent：单一实现，零来源分支
async fn ask_execute(ctx: &AskContext, question: String, options: Vec<String>) -> Result<String, String> {
    let tool_call_id = ctx.tool_call_id.clone();
    // 有 sink（流程来源）→ 额外广播编排状态；无 sink（通用来源）→ 跳过
    if let Some(sink) = &ctx.sink {
        sink.ask_started(&tool_call_id, &question, &options);   // → RunUpdate::Ask
    }
    let answer = ctx.answer.recv().await;                       // 阻塞
    if let Some(sink) = &ctx.sink {
        sink.ask_resolved(&tool_call_id);                       // → 解除编排停等态（契约见前置 change）
    }
    answer
}
```

## 通用 ask 需要什么（这是新增的最大一块）

现有 `wait_answer` / `answer_tx` 全部住在 **`ChangeFlowControl`**（键 = `(root, change_id)`），通用会话根本没有这个 key。要通用化，得把应答通道下沉到会话基建：

```
core/agent  ── 新增 AskRegistry（键 = core session id，与 StopRegistry 同型）
               register(session_id, oneshot::Sender<String>)
               answer(session_id, text) -> Result<(), String>
               remove(session_id)

               阻塞语义在 loop 内，AskRegistry 只做「找到等待方并投递」
```

于是两条来源的应答入口分岔（但工具实现体不分叉）：

```
通用来源：前端 ipc.agent_answer(root, session_id, text)
              └─► AskRegistry.answer(session_id, text)

流程来源：前端 ipc.change_flow_answer(root, change_id, text)   ← 保持不变
              └─► ChangeFlowControl.answer → oneshot
                    └─►（同一条 oneshot，因为流程的 ask 也注册在 AskRegistry 里？
                         还是流程仍走自己的 answer_tx？）
```

**这里有个必须拍的设计选择**：流程的 ask 有两条寻址（session id / change id）。若两处都注册，就存在「同一 ask 两个入口」的重复投递风险（先到者胜，后者 Err）。若只注册一处，则另一处 UI 得先做 id 映射。

倾向：**唯一真相在 `AskRegistry`（键 = session id）**。`change_flow_answer` 内部先经 `ChangeFlowControl.current_session(root, change_id)` 取到 session id，再转投 `AskRegistry.answer`。这样：
- 单一应答通道，重复投递天然幂等（oneshot take 语义）；
- `change_flow_answer` 命令签名不变（前端零改动）；
- 通用来源直接用 `agent_answer`。

## 删除 `action: "ask"` 的波及面（**已被前置 change 消化，保留作考古**）

> `parse_decision` 已删除；`DecisionAction::Ask` 现存但只经 `finish_step` 的
> `"enum": ["backtrack","stop","ask"]` 进入。下文清点的位置大多已不存在。

```
crates/core/orchestration/src/decision.rs
  DecisionAction 三变体 → 两变体（Backtrack / Stop）

crates/core/orchestration/src/walker.rs
  :3406  DecisionAction::Ask { .. } => {...}        ← 删（决策会话的 ask 分支）
  :3471  Ok(DecisionAction::Ask{..}) => {...}       ← 删（软拒环里的 ask 停等）

crates/core/orchestration/src/decision_test.rs
  :45    parse_decision("action":"ask") 用例         ← 改为断言「ask 已退役应 Err」，
  :50    DecisionAction::Ask{..} 断言                 与现有 retry 退役用例同款
  :206   DecisionAction::Ask{..} 构造
  :413   DecisionAction::Ask{..} 构造

crates/core/orchestration/src/walker_test.rs
  :3021 / :3429 / :4770 / :11170 / :11225
         五处 FakeWorker 用文本 ask 喂决策会话        ← 改为「假 worker 直接触发 ask 工具」
                                                        或改为其他动作（backtrack/stop）

crates/core/orchestration/src/prompt.rs
  decision_prompt 的三动作封闭集说明 → 两动作 + 「需要用户决策时调 ask 工具」
```

**向后兼容的取舍**：旧转录里存在 `{action:"ask"}` 文本，删除解析后这些历史会话无法再被解析。但 `parse_decision` 只用于**运行期决策产出**，历史转录不重放解析——所以**无兼容负担**，可以硬删。这一点与「保留 Ask 变体以兼容旧转录」的上一轮倾向相反，以你的裁决为准。

## 目标形态：ask 成为**通用**的阻塞工具（流程只是额外改状态）

> 第二轮裁决后，ask 不再是流程专用能力——通用会话（explore / 架构 / 调试）同样有，
> 差异只在编排侧是否额外广播 `RunUpdate::Ask`。详见本文末「裁决（第二轮）」。

```
任意流程 agent step（executor / evaluator / decision）
  └ loop 分派到 ask 工具
       ├ 工具执行体内阻塞：tokio::select! { 应答 oneshot, cancel }
       ├ 阻塞期间 emit RunUpdate::Ask（携 tool_call_id + session_id）
       │    └ 变更详情 AskCard 与聊天窗口行内卡**同源渲染**
       │         └ 两处任一 invoke change_flow_answer → oneshot 回传
       ├ 应答到达 → Ok(答案文本) 作为 tool_result 回灌
       └ 模型在同一轮里继续（不重建、不重跑）
```

关键收益：**ask 不再消耗一整轮**。它变成一个普通的工具调用——模型提问、用户答、模型接着说，全在同一个 turn 内。

## 与 finalize 的冲突：自然消解

上一轮我在 finalize 文档里提的最大未决点是：

> `finalize` 一调就收口，ask 若也是 finalize 的分支，那它就再也没机会停等了

现在答案清楚：**ask 不做成 finalize 的 schema 分支，而是独立的阻塞工具**。两者语义正交：

```
finalize：非阻塞，调完即请求收口     ← 「我干完了」
ask：     阻塞，等到应答才返回        ← 「我卡住了」

同一轮里可以 ask 若干次，最后 finalize 一次。
```

所以 finalize 的「本 step 结束」语义**不需要**扩成「产出就绪、等编排处置」——原裁决保持不变。

## 三个实现难点（这是本需求的真正成本）

### 难点 A：阻塞时的停止

`loop` 里工具执行是 `await`；阻塞期间 `handle.stop_requested()` 无法被观测。

```rust
// 必须 select，不能让 ask 无限期挂在停止信号之外
tokio::select! {
    answer = ask.recv() => Ok(answer),
    _ = handle.wait_requested() => Err("用户未应答（会话被终止）"),
}
```

需要确认：停止时返回 `is_error` ToolResult 让模型看见，还是干脆不回灌（直接 `return history`）？倾向回灌——保持 tool_use/tool_result 配对不变量，`resume::rebuild` 才不会插合成中断结果。

### 难点 B：工具执行体拿不到流程控制面

现在 `LoopTurn` 携带的 per-turn 上下文（`explore_note` / `archi` / `protect`）都是 pump 起播时铸造的值。ask 需要的是 **`ChangeFlowControl`**（`entry.ask` 载荷、`wait_answer` 的 oneshot），它在 walker 侧。

可行通道：`KernelWorkerPort` 已持有 `Arc<dyn RunEventSink>`（`worker.rs`），而 `RunEventSink::emit(RunUpdate)` 本身就能触达 control。所以：

```
infra/agent/src/worker.rs
  trait RunEventSink {
      fn emit(&self, update: RunUpdate);
      + fn ask(&self, question, options, tool_call_id) -> BoxFuture<Result<String, String>>;
  }
       │
       └ ChangeFlowSink（src/commands/change_flow/mod.rs）
              └ self.control.publish(RunUpdate::Ask{..})
              └ self.control.await_answer().await      ← 复用既有 oneshot 机制
```

这条路绕开了「工具执行体要认识 orchestration 类型」的问题——`RunEventSink` 本来就在 orchestration 里定义（`port.rs`），infra/agent 已经依赖它。

### 难点 C：载荷扩展（本 change 只加字段，不修清理）

原本这里记着「`ask` 槽只写不清」的 bug 分析。该缺陷已修复合入
（`6708e14875a4`，两处停等残留一并修，含 `confirm`），本文**不再复述**其根因与修法，
只声明本 change 对既有实现的依赖：

```
既有实现（6708e14875a4）已提供：
  answer() / confirm() 经 accept_pending 在应答受理时清 status + 载荷槽并广播

本 change 在此契约上只做两件事：
  ├ AskPayload 追加 tool_call_id / session_id（additive 字段）
  └ 清理时机不变——应答受理即清「当前这条 ask」
```

依赖是硬性的：行内卡的活卡判据是 `ask.toolCallId === part.toolCallId`，
若槽在应答后不清，行内卡会永远判为「正在等」。故**前置 change 未落地时本 change
不可启动**（该条件已满足）。

失效信号用的是 `RunNotice::Step` 而非 `Ask` / `ConfirmWait`：后者在通知面被读作
「新停等已开始」，重放会让既有应答回路对已解除的等待再答一次。本 change 必须沿用
`Step`。

## 载荷要扩展什么

```rust
pub struct AskPayload {
    pub question: String,
    pub options: Vec<String>,
    + pub tool_call_id: Option<String>,   // 行内卡对账：我这个 tool_use 是不是正在等
    + pub session_id: Option<String>,     // 转录面板定位是哪条会话里的提问
}
```

`tool_call_id` 是**两处 UI 共享的关键**。没有它，聊天窗口里的行内卡只能靠「output 为 null 就认为在等」推断——而历史转录里所有未配对的 tool_use 都是 `input-available` 状态，根本分不清「正在等」和「上次崩了没回灌」。

有了它，行内卡的判据变成：

```
part.state === 'input-available' && ask.toolCallId === part.toolCallId   → 活卡（可应答）
否则                                                                     → 静态卡（只读）
```

## 两处 UI 的形状

```
┌─ 变更详情右栏 RunControlPanel ────────────┐
│  AskCard（现存，run-ask-card）              │
│    question + options 按钮 + 自由输入       │
└──────────────┬─────────────────────────────┘
               │  两处 invoke 同一个命令
               ├─► change_flow_answer(root, id, text)
               │      └─► control.answer → oneshot
               │           └─► 广播失效信号 → 两处同时收起
┌──────────────┴─────────────────────────────┐
│  聊天窗口（SessionTranscriptPanel）          │
│    ask 工具的 tool_use 部件                  │
│      └─ 特化卡（新）：question + options     │
│           + 输入框（可选：直接点选项）        │
└────────────────────────────────────────────┘
```

难点：`AgentToolCardProps` 现在只有 `{toolCallId, toolName, input, output}`——**没有应答回调**。要给卡传入 `onAnswer`，得扩这个接口。但 `BUILD_IN_TOOL_CARDS` 是全局注册表，聊天窗口的卡需要来源侧注入的回调。

两条路：
- **A**：`AgentSessionView` / `AgentMessages` 增一个可选的 `toolCardHandlers` prop（只读消费点不传，天然退化为静态卡）——与 composer 插槽同一种「可选动作面」思路；
- **B**：卡内部直接 `ipc.changeFlowAnswer`——但那样组件族就认识了 change 域，破坏「组件族不认识业务」的边界。

倾向 A。

## 改动面清点（按第二轮裁决重写，**已被第四轮「范围重估」取代**）

```
core/agent                       ← 新增：应答通道下沉到会话基建
  runner.rs         AgentSession + asks: mpsc::Sender<AskRequest>（或独立 AskRegistry 句柄）
  kernel.rs         AskRegistry（键 = session id；register / answer / remove）
                    begin_turn 登记、converge 除名（与 StopRegistry 同址）
  event.rs          无需改（ask 是 tool_use/tool_result 一对，复用 Message 信封）

infra/agent
  sdk/tools.rs      ask 工具定义（不进七通用 TOOL_NAMES——它是基线交互面，与 read/bash 并列？
                    还是独立？见待决 A）
  sdk/policy.rs     ask 放行档位（协议出口，任何档位放行）
  sdk/loop.rs       分派臂：阻塞 select! { 应答, handle.wait_requested() }
                    单实现体，零「是否流程来源」分支
  worker.rs         KernelWorkerPort 把 sink 的 ask 能力装进 LoopTurn（有 sink 才多广播）

core/orchestration
  state.rs          AskPayload + tool_call_id / session_id（仅 additive 两字段）
                    RunUpdate / RunNotice 零变体（解除停等复用既有 kind，归前置 change）
  control.rs        零改动（清理点归前置 change）
  port.rs           RunEventSink + ask_started / ask_resolved（编排侧钩子）
  decision.rs       DecisionAction 删 Ask 变体（收敛为 Backtrack / Stop）
  walker.rs         :3406 / :3471 两处 Ask 分支删除
                    run_worker 装配 ask 上下文（把 sink 透传给 worker port）
  prompt.rs         decision_prompt 三动作说明 → 两动作 +「需用户决策时调 ask 工具」

壳层
  commands/exec     agent_answer(root, session_id, text)          ← 新增（通用入口）
  commands/change_flow
                    change_flow_answer 签名不变，内部改为
                    current_session(root, change_id) → AskRegistry.answer(session_id)
                    ChangeFlowSink 实现 ask_started / ask_resolved

前端
  types/generated/bindings.ts               AskPayload 扩展 + agent_answer
  lib/ipc.ts                                自动（生成物驱动）
  components/agent/tool-cards.tsx           ask 特化卡（活卡 + 静态卡两态）
  components/agent/agent-messages.tsx       toolCardHandlers prop（只读消费点不传 → 静态卡）
  components/agent/agent-session-view.tsx   同上透传
  hooks/use-agent-session.ts                answer 动作（通用来源）
  views/explores / views/architecture       注入 onAnswer → ipc.agentAnswer
  views/changes/flow/session-transcript-panel.tsx   注入 onAnswer → ipc.changeFlowAnswer
  views/changes/flow/run-control-panel.tsx          AskCard 按 toolCallId 对账（收起逻辑归前置 change）
```

**测试面**（删除协议部分）：
- `decision_test.rs` 4 处 `DecisionAction::Ask` 用例改为「已退役应 Err」；
- `walker_test.rs` 5 处（:3021 / :3429 / :4770 / :11170 / :11225）文本 ask 夹具改为工具触发或换动作；
- `prompt_test.rs` 决策三动作断言（约 6 处）改为两动作。

## 待决问题（第二轮的清单，Q1/Q3/Q5 状态见第四轮）

1. ~~**ask 是否也进 `SessionToolset`？**~~ → **已裁决：ask 是通用能力**，所有会话可用（不进 `SessionToolset`——它不是「工具面收窄」，而是基线能力；`SessionToolset` 继续只表达 Explore / Archi 这类专用面）。流程内 ask 额外把 change 切到 `WaitingAsk`；agent 视角两者无差别。

2. ~~**decision 的双协议并存怎么办？**~~ → **已裁决：删除 `action: "ask"` 协议，仅 ask 工具可用**。`DecisionAction` 收敛为两变体（Backtrack / Stop）。`parse_decision` 只用于运行期产出解析，历史转录不重放解析，故**无兼容负担**，可硬删（波及面已核实，见下节）。

3. **ask 进不进七通用 `TOOL_NAMES`？**（新）
   现有 `TOOL_NAMES: [&str; 7] = ["read","grep","glob","ls","write","edit","bash"]` 是「七工具封闭集」，`compose` 的注入校验、`RunStarted.tools` 上报、`policy` 放行表全部锚它。
   - 加进第七位之后的第八位：`TOOL_NAMES` 变成 8 项，`[&str; 7]` 字面量要改，且「七工具」的注释与 spec 叙述全部失效；
   - 或像 `create_explore_note` / `archi_module` 那样做「专用工具，经 toolset / 注入装配追加」，但 ask 是**基线能力**不该随来源开关；
   - 或承认它是第八个通用工具（最简单，代价是改一批「七」字面）。
   倾向第三条：**ask 就是第八个通用工具**。「七」这个数字本来只是当时的封闭集长度，现在该变成 8。

4. **一次 ask 是否允许多问题？**
   `AskUserQuestion` 原生形状是 `questions: [{question, options}]` 数组（`tool-cards.tsx` 的静态卡就是这么解析的）。新工具要不要支持多问？`AskPayload` 现在是单 `question`。
   → 多问会让「两处 UI 同步」更复杂（部分应答怎么办）。倾向单问，多次调用。

5. **应答是否允许「不选选项的自由文本」？**
   现状 `AskCard` 两者都有。工具化后，`options` 若为空则只有自由输入——这已经在 UI 上成立，只需要在工具 schema 里让 `options` 可空。

## 第三轮：启动条件盘点（**已被第四轮取代，保留作推导对照**）

> 本节结论「尚不具备，差 2 个必拍（Q1/Q2）」在第四轮被推翻——两个前置 change 落地后
> Q1 有现成答案、Q2 紧迫性下降。**以第四轮为准**；保留本节是为了记录当时的代码状态
> 与推导路径（现在 `TOOL_NAMES` 已退居定义作者处，不再是判定面）。

判据：能否把 proposal 的「变更范围 + AC」写完整而不留问号。
（编号用 Q，与已修复的停等态 bug 编号 B1 / B2 区分。）

```
已裁决（不可再议）                    必拍（决定代码形状）
├─ ① ask 是通用能力，不进 Toolset     ├─ Q1 收窄面会砍掉 ask？      ← 第四轮已答
└─ ② 删 action:"ask"，仅工具可用      └─ Q2 应答通道挂哪（新表 vs RunHandle）  ← 第四轮部分答
可留 design（非阻塞）
├─ 多问 vs 单问（单问 + 多次调用）      （原 Q3「清理与通知」已由
└─ options 可空（自由文本）              6708e14875a4 修复）
```

## 第四轮（当前结论）：前置落地后前提已变 —— 可启动，范围大幅缩小

### 落地事实（本轮核实）

前置一：停等态残留修复已合入（commit `6708e14875a4`，bump 0.6.35）。
`accept_pending` 单一受理入口，锁内先清再 send，**失效通知用 `RunNotice::Step`**。
最后这条比本文倾向更强——我原以为复用 `Ask`/`ConfirmWait` 等价，实际不然：这两个
kind 在通知面被读作「新停等已开始」，重放会让既有应答回路对已解除的等待再答一次
（实现时实测 4 个 walker 用例）。**ask 工具化必须沿用 `Step` 作解除信号。**

前置二：`agent-finalize-tool-for-flow-steps` 已合入（`8d59f3b8186b` 归档）。这一条
**重写本文多个前提**：

| 本文原假设 | 落地后的事实 |
| --- | --- |
| 「`action:"ask"` 是**文本协议**，靠 `parse_decision` 解析最终消息」 | **已消失**。决策动作改由 `finish_step` 工具提交，`FinishPayload::Decision(DecisionAction)` 经工具参数 schema 校验；`parse_decision` 已删除 |
| 「ask 是 finalize 的 schema 分支会收口，两者冲突」 | **不成立**。`finish_step` 一调即收口，而 ask 是它的一个**枚举值**，走的是「收口后 walker 停等」的老路 |
| 「删 `action:"ask"` 会波及 `parse_decision` / 软拒环 / 4+5 处测试」 | **已由前置二完成**。`DecisionAction::Ask` 仍存在，但只经 `finish_step` 的 `"enum": ["backtrack","stop","ask"]` 进入 |
| 「`TOOL_NAMES` 是七项封闭集，ask 要当第八个，会破坏收窄面」 | **已过时**。`TOOL_NAMES` 退为「定义作者处」常量，判定全查 `mount::registry()` 挂载清单（十项 = 七内建 + explore_note + 两 archi），`finish_step` 用 `ToolTier::Protocol`（三档全放行）+ `definitions` 按 `finish` 位动态追加 |

### 于是 Q1 / Q2 都有了现成答案

**Q1（收窄面会不会砍掉 ask）→ 不会，且不必进 `TOOL_NAMES`。**

新一代工具面机制是「名字清单 + 挂载清单查表」：

```
compose::injected(names, preamble, finish, provenance)
   ├ names   = 本轮下发的工具名字清单（explore 六名 / archi 九名 / change 七名+矩阵）
   ├ finish  = 按 (phase, role) 置位：evaluator → EvaluatorChecklist
   │                         decision → Decision
   │                         其余      → None
   └ 每个名字都过 mount::lookup() 成员校验（挂载清单外 → Err）

mount::definitions(injections, prompt)
   ├ tools = None   → registry 前七项（内建七件）
   ├ tools = Some   → 按名过滤
   └ finish = Some  → 追加 finish_step（ToolTier::Protocol，执行体 FinishStep(spec)）
```

所以 ask 有三条现成位置可选，且**都不需要碰 `TOOL_NAMES`**：

| 路 | 形态 | 评价 |
| --- | --- | --- |
| **A（推荐）** | 挂载清单新增一项，tier = `ToolTier::Protocol`，exec = 新 `ToolExec::Ask`；`definitions` 与 `finish_step` **同款**无条件追加 | 零收窄风险（不进 `tools` 名字清单，`Some(names)` 过滤碰不到它）；与 `finish_step` 对称，机制复用 |
| B | 进 `TOOL_NAMES` 成第八项 | 需改 explore 六名清单让位，且「七内建」语义（`tools: None` 取前七）被打碎 |
| C | 走 `toolset` 枚举追加 | ask 是基线能力，不该随来源开关 |

**Q2（应答通道挂哪）→ 有了第三个、也是最好的选项。**

上文对比的是「新增 `AskRegistry`（键 = session id）」vs「扩 `RunHandle`」。但前置二
已经把桥搭好了：`LoopTurn` 现在经 `SessionInjections.finish` 拿到了编排侧的收口能力，
而 `finish_step` 的执行体是 `ToolExec::FinishStep(spec)`——**`spec` 是 core 词汇，
执行体在 orchestration 的 `finish` 模块**。也就是说 infra → orchestration 这条依赖
已在挂载清单里打通（`mount.rs` 直接引 `orchestration::finish::{TOOL_NAME, description,
parameters}`）。

于是 ask 可以走同一形状：`ToolExec::Ask` 的执行体调编排侧，编排侧再回落到现有
`ChangeFlowControl` 的 oneshot——**通用来源走哪条路仍未定，但流程来源可以零新增通道**：

```
流程来源（有 RunEventSink）：
  ask 执行体 → sink.ask(question, options) → ChangeFlowControl.publish(Ask) + wait_answer()
  ← 与现状完全同机制，只是从「walker 停等」搬进「工具执行体内阻塞」

通用来源（无 sink）：
  需要一条 session 级通道 —— 这里的选型仍未定（新 AskRegistry vs 扩 RunHandle）
```

**这削弱了 Q2 的紧迫性**：若首期只做「流程内 ask 工具化」（本文标题的原始诉求），
通用来源可以整体留到后续 change。这正好把最大的一块成本切出去。

### 范围重估：从「15+ 文件跨 5 层」缩到「一个工具 + 一个通道」

本文改动的真实剩余：

```
必需（流程内 ask）
  infra/agent  mount.rs     挂载项 + ToolExec::Ask + tier Protocol
               loop.rs      分派臂：阻塞 select!（外层泵已有 drop 兜底，见下）
  core/orchestration
               port.rs      RunEventSink::ask（或复用既有 wait_answer 机制）
               walker.rs    :3287-3303 的 Ask 分支改由工具内闭合
               decision.rs  DecisionAction::Ask 的产出方从 finish_step 枚举变为工具
                            （或两者并存，见待决）
  core/agent   SessionInjections 若需新位（可能不需要——基线能力，无条件追加）

可推迟（通用 ask）
  core/agent   AskRegistry 或 RunHandle 扩展 + kernel 登记
  壳层         agent_answer 命令
  前端         toolCardHandlers / 双端 onAnswer 注入
```

**「双端同步展示」也需重新定位**：`6708e14875a4` 已把「详情页 AskCard 随应答收起」
修好。所以「双端同步」在流程内的价值只剩「聊天窗里有行内卡」——
不再是「一方应答另一方不消失」的正确性问题。这进一步说明它可以推迟。

### Q1：`tools` 收窄面会把 ask 砍掉（**第四轮已答，见下节；本段是当时的推导**）

> 当时成立的漏洞，在 `agent-finalize-tool-for-flow-steps` 落地后换了形态：
> `TOOL_NAMES` 退为「定义作者处」常量，判定全查 `mount::registry()`，`finish_step`
> 用 `ToolTier::Protocol` + 动态追加——`Some(names)` 过滤碰不到它。结论：ask 照
> `finish_step` 的形状挂载即可，不必进 `TOOL_NAMES`。

前两轮待决 3 的「倾向：ask 就是第八个通用工具」在（当时的）代码里不成立：

```
compose.rs::assemble_injections（explore 臂）
    tools = Some(["read","grep","glob","ls","edit","bash"])   ← 白名单收窄

tools.rs::definitions
    Some(names) => TOOL_NAMES.iter().filter(|n| names.contains(n))   ← 只留交集
```

ask 进 `TOOL_NAMES` 后，explore 会话（六名白名单）拿不到 ask——直接违反裁决①。三条出路：

| 路 | 形态 | 代价 |
| --- | --- | --- |
| **A（推荐）** | `TOOL_NAMES`（7，可收窄）+ `BASELINE_TOOLS`（1，不可收窄）；definitions = 收窄集 ∪ 基线集 ∪ toolset 追加 | 改 `definitions` 组装顺序契约 + `RunStarted.tools` 断言；语义最干净 |
| B | ask 无条件追加，与 toolset 专用工具并列但独立分支 | 与「专用工具仅经枚举注入」封闭原则擦边（它是基线不是专用） |
| C | explore 子集六 → 含 ask | 违反 spec「六工具收窄（write 移除）」+ 一批 AC |

连带面：`七工具` 字面量在 `.rs` 产品代码 3 处（`core/agent/src/runner.rs:69` doc、`compose.rs` 错误串与 doc、`tools.rs` doc）+ 6 处 spec 段落 + 测试断言若干。

### Q2：应答通道——「唯一 AskRegistry」比笔记估的贵（**第四轮给出第三条路**）

> 第四轮发现：`mount.rs` 已直接引 `orchestration::finish`，infra → orchestration
> 这条依赖在挂载清单里已打通。于是流程来源可零新增通道（`ToolExec::Ask` 转投既有
> `ChangeFlowControl`），只有**通用来源**需要新通道——而通用来源可整体推迟。

```
AskRegistry 要进 loop，只能随 LoopTurn 或 AgentSession 下来
  ├─ 走 AgentSession：runner.rs:222 构造点 + kernel.rs begin_turn 登记
  │                    + compose 装配链 + 5 处测试夹具
  └─ 走 LoopTurn：infra runner 每轮铸造要拿到 Arc<AskRegistry>
                  → KernelWorkerPort 得从命令层把它传进来
```

更省的一条：`RunHandle` **已经在 `LoopTurn` 里**（`loop.rs` 的 `handle` 参数），而 `StopRegistry` 已是「session id → RunHandle」表：

```
agent_answer(root, session_id, text)
   └─ StopRegistry.handle(session_id)      ← 需补 getter（现只有 request_stop）
        └─ RunHandle.answer(text) → oneshot → ask 执行体
```

`RunHandle` 是 `derive(Default, Clone)`，加一个 `Arc<Mutex<Option<oneshot::Sender<String>>>>` 槽零构造点波及；代价是语义从「终止信号」扩成「运行控制面」（改 doc 或改名 `RunControl`）。收益：与 steer 的 `InteractionRegistry` 天然同源。

**这条决定 `core/agent` 是「新增类型 + 多处装配」还是「扩既有类型 + 一个 getter」，波及量差一个数量级。**

### Q3 → 已移交（本文不再有此项）

原先是「修 todo#65 是否新增 IPC 变体」。拆分后该问题整体归
已修复的停等态 bug（`6708e14875a4`：三候选选「命令层清理 + 复用既有 kind」，零新变体）。

对本 change 只剩一条**输入契约**：`ask` 槽在应答后会被清空。
本 change 自己只加 `AskPayload.tool_call_id` / `session_id` 两个 additive 字段。

### 新发现：难点 A（阻塞时的停止）不存在

泵外层已有 select，停止直接 drop 整个 loop future：

```rust
// sdk/runner.rs::session_pump
tokio::select! {
    biased;
    _ = r#loop::run(...) => { … }
    _ = pump_handle.wait_requested() => { /* loop future 轮中 drop */ }
}
```

于是：

- ask 执行体**不需要**再写 `select! { 应答, wait_requested }`；
- 「停止时回灌 is_error 让模型看见」做不到（future 已 drop），转录留未配对 `tool_use`；
- 该代价由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 补齐兜住（`resume.rs:13` 逐字存在）。

故难点 A 从「实现难点」降为「spec 里如实写一句：停止即 drop，配对由 resume 补齐」。

### spec delta 面（按第四轮范围重估）

**仍改（流程内 ask 最小版）**：

```
desktop-agent-execution
  ← 「交互工具限制：工具面为封闭清单（无 ask 类工具）」整段改写
  ← 「工具挂载单源与 tier 准入」新增 ask 项（tier = Protocol）
  ← RunStarted.tools 断言随动
desktop-change-orchestration
  ← 决策 ask 的产出路径：README 里「三动作封闭集」表述与 finish_step 的 enum 关系需理清
     （ask 是 finish_step 的一个枚举值，还是要搬成独立工具？——见待决 X）
desktop-change-flow-view
  ← 「只读消费点不呈现发送 / 停止」需增 ask 内联应答例外（轻）
```

**已不改（前置二 / 前置一已消化）**：

```
desktop-agent-chat-infra     静态卡 → 活卡：仅在「通用 ask」或「聊天窗行内卡」进入范围时才改
desktop-explore-page         同上，可推迟
desktop-change-state-store   停等态不落库表述不变
desktop-app-shell            仅当新增 agent_answer 命令才需改，可推迟
```

### 待决 X（新，第四轮产生）：`action:"ask"` 与 `finish_step` 的关系

这是本轮唯一新增的、必须拍的结构问题：

```
现状：ask 是 finish_step(Decision) 的一个 enum 值
      → 模型调 finish_step{action:"ask"} → 工具返回"已收口" → walker 收口
      → walker 读 finalized == Ask → publish(Ask) → wait_answer → 续一轮决策会话

选项 1：保持现状（ask 仍经 finish_step 提交）
  优点：零改动；决策协议不变
  缺点：「ask 是阻塞工具」的诉求落空——ask 仍会结束本步（finish_step 一调即收口），
        仍消耗一整轮，仍靠 Continue 重开。第四轮的收益（不消耗整轮）拿不到。

选项 2：ask 独立于 finish_step（真正的阻塞工具）
  优点：达成原始诉求——ask 在同一 turn 内阻塞、答完继续，不重开会话
  缺点：finish_step(Decision) 的 enum 要去掉 ask，改为「需用户决策时调 ask 工具」
        措辞；walker 的 finalized==Ask 分支删除；decision_answer_prompt 退役
        需要 ask 工具对 decision 角色也下发（现在 decision 只挂 finish_step）
```

**这个是本 change 的核心裁决，且只有选 2 才能兑现标题里的诉求。** 若选 1，本 change
退化为「什么都没改」——因为现状已经就是「ask 经工具提交」了。

顺带一条落地细节（选 2 时必改）：现状 `finish.rs` 的 ask 校验要求 `options` **非空**
（`validate_decision` 的 `"ask"` 臂显式拒绝空数组）。本议题第二轮待决 5 倾向「options
可空、只给自由输入」，两者冲突——选 2 时要一并决定空 options 是否合法。


## 第五轮（定稿）：X-2 形状与 options 非空

### 裁决

```
X-2   ask 独立成阻塞工具，finish_step 的 enum 收为两值
      ⇒ ask 变成"同一 turn 内的阻塞调用"，答完模型接着说，不重开会话

options 非空
      ⇒ 沿用 finish.rs 现有校验（validate_decision 的 "ask" 臂已显式拒绝空数组）
      ⇒ 第二轮待决 5「options 可空」作废：工具 schema 要求 minItems 1
      ⇒ AskCard 的自由文本输入保留（用户可自由作答），但**模型必须给选项**
```

### 核心机制：ask 执行体阻塞，怎么拿到应答

这是 X-2 的唯一真难点。现状 `wait_answer()` 住在 `RunGuard`（walker 侧），
而工具执行体在 infra 的 loop 里——两者之间隔着 `WorkerAgentPort` 边界。

**但这条依赖已经通了**：`agent-finalize-tool-for-flow-steps` 让 `mount.rs` 直接引
`orchestration::finish::{TOOL_NAME, description, parameters, validate}`，
`ToolExec::FinishStep(spec)` 已证明「infra 挂载项 → orchestration 纯函数」可行。

于是 ask 走同一形状，且**应答通道不必下沉到 core/agent**（原 Q2 的整个选型作废）：

```
Layer 1  core/orchestration（新模块 ask.rs，与 finish.rs 对称）
           TOOL_NAME = "ask"
           description() / parameters()        ← 纯函数
           validate(input) -> AskQuestion      ← question + 非空 options 校验

Layer 2  infra/agent/sdk/mount.rs
           ToolExec::Ask 新变体（无 spec 参数——与 FinishStep 不同，ask 无档位）
           definitions() 无条件追加（Protocol tier，与 finish_step 同款）

Layer 3  infra/agent/sdk/loop.rs
           分派臂：ToolExec::Ask => turn.ask.ask(&input).await
           turn.ask: Option<AskContext>（与 explore_note / archi 同列的 per-turn 上下文）

Layer 4  infra/agent/worker.rs + sdk/runner.rs
           KernelWorkerPort 已持 Arc<dyn RunEventSink>
           → 需构造 AskContext 塞进 LoopTurn
           ⇒ 通道：compose_turn 收一个 Arc<dyn AskPort>，经 EngineFacade 下行
              （与 with_workspace_root / with_explore_note_store 同款注入缝）

Layer 5  壳层 ChangeFlowSink 实现 AskPort
           ask(question, options) -> Result<String, String>
             = control.publish(RunUpdate::Ask{..}) + guard.wait_answer()
```

关键：**`AskPort` 是编排侧 trait，不是 core/agent 的注册表**。通用来源（explore /
archi / debug）传 `None`，ask 工具调用以 is_error 回灌——这正是「通用 ask 可推迟」
的落地形态，也是本文第二轮「agent 视角无差别」约束的实现：差别只在 `LoopTurn.ask`
是不是 `None`。

### 阻塞期间的停止：外层泵已兜住

```rust
// sdk/runner.rs::session_pump
tokio::select! {
    biased;
    _ = loop::run(...) => { ... }
    _ = pump_handle.wait_requested() => { /* loop future 轮中 drop */ }
}
```

停止信号直接 drop 整个 loop future，ask 的 await 一并消失。所以：

- ask 执行体**不需要**写 `select! { 应答, wait_requested }`；
- 但**需要**让 `wait_answer` 自身的取消半边生效（它已有 `cancelled_wait()`）；
- 转录留未配对 `tool_use`，由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 补齐。

### finish_step 的收窄（必改）

```
core/orchestration/finish.rs
  parameters(Decision)：  enum: ["backtrack","stop","ask"] → ["backtrack","stop"]
                          删 question / options 两个 property
  description(Decision)：「三动作封闭集」→「两动作」+「需要用户决策时调用 ask 工具」
  validate()：           走 validate_decision，其 "ask" 臂**保留**（决策协议仍可
                         反序列化历史 JSON；但 finish_step 的 schema 不再产出它）

core/orchestration/decision.rs
  DecisionAction::Ask      保留枚举（历史转录兼容 + 下方 walker 分支兜底）
  validate_decision 的 "ask" 臂保留 → 但 finish_step 的 enum 不含它，
  故生产路径不可达（模型若硬编 action:"ask" 会被 schema 校验拒绝）

core/orchestration/prompt.rs
  decision_prompt 末段：三选一 → 两选一，新增一句「无法裁决时调用 ask 工具」
  decision_answer_prompt  **删除**（应答不再开新一轮；同一 turn 内继续）

core/orchestration/walker.rs
  :3287-3303  finalized == Ask 分支**删除**（不可达）
  :3223       DecisionAction::Ask 的兜底臂保留（防御：若真流出来即漂移）
```

### `AskPayload` 取舍（**已由第六轮裁决：零扩展**）

X-2 之后 ask 仍是「流程内要额外改 change 状态」，所以 `RunUpdate::Ask` 与
`AskPayload` **保留**。但 `tool_call_id` / `session_id` 两字段的取舍变了：

```
若首期只做「详情页 AskCard」（现状已有）
  ⇒ AskPayload 不变，零扩展。ask 工具的行内卡留到通用 ask change

若首期也要「聊天窗行内卡」
  ⇒ AskPayload 扩 tool_call_id（判活卡）+ session_id（定位会话）
  ⇒ 并且 AskPort 要把 tool_call_id 传下去（loop 侧知道，编排侧不知道）
```

**裁决（第六轮）：首期零扩展**——用户明确「可以接受聊天窗口内没有渲染（有更好）」。
把「双端同步渲染」整体归入后续的通用 ask change。理由：X-2 的核心收益是「不消耗
整轮」，与 UI 有几处渲染无关；而 `flow-waiting-state-residue` 已把「应答后卡片消失」
修好，单端渲染不存在正确性缺口。

### 改动面（X-2 首期，定稿）

```
core/orchestration
  ask.rs（新）      TOOL_NAME / description / parameters / validate
                    MUST NOT use crate::finish（解耦守卫，grep 可验）
  lib.rs            模块导出
  finish.rs          Decision schema 收两值 + description 改写
                    MUST NOT use crate::ask（仅文案里提 ask 工具名）
  decision.rs        DecisionAction::Ask 保留（历史兼容），注释说明生产不可达
  prompt.rs          decision_prompt 两选一 + ask 工具指路；删 decision_answer_prompt
  walker.rs          删 finalized==Ask 分支；保留防御臂
  port.rs            AskPort trait（ask(question, options) -> Result<String,String>）

infra/agent
  sdk/mount.rs      ToolExec::Ask + 挂载项（Protocol tier）
                    definitions 中 ask 无条件追加、finish_step 仍按 finish 位
                    provider_tool 挂载点留一条「原生 ask 不挂 = 覆盖」不变量注释
  sdk/loop.rs       分派臂 + LoopTurn.ask 字段
  sdk/runner.rs     PumpPayload.ask 透传 + per-turn 铸造
  worker.rs         KernelWorkerPort 构造 AskContext（持 sink）
  lib.rs            EngineFacade::with_ask_port
  compose.rs        compose_turn 收 Arc<dyn AskPort>

壳层
  commands/change_flow/mod.rs   ChangeFlowSink 实现 AskPort（publish + wait_answer）
  commands/exec/mod.rs          agent_start 传 None（通用来源无 ask）
  （compose_turn 调用点两处同改）

测试
  finish_test.rs    decision schema enum 断言三→二；question/options property 删除断言
  decision_test.rs  validate_decision 的 ask 臂保留（单元测照旧）
  prompt_test.rs    三选一断言 → 两选一 + ask 工具指路断言
  walker_test.rs    决策 ask 往返用例改为「ask 工具触发」（夹具换形状）
  loop_test.rs      新增：ask 工具阻塞 → 应答回灌 → 同 turn 续跑
                    新增：finish 位为 None 的会话仍下发 ask（解耦验证）
  ask（新）_test.rs core/orchestration 的 validate 单测（question 必填 / options 非空）
                    解耦守卫：ask.rs 全文零 "finish" 字样

spec
  desktop-agent-execution      「工具挂载单源与 tier 准入」新增 ask 项
                                「交互工具限制（无 ask 类工具）」整段改写
  desktop-change-orchestration 决策协议三动作 → 两动作 + ask 工具；删 decision_answer_prompt
  desktop-change-flow-view     ask 中断卡片的触发源从「决策 ask 动作」改为「ask 工具调用」
  desktop-explore-page 等       零改动（通用来源无 ask）
```

**零改动面（第六轮确认）**：
```
AskPayload          零扩展（不加 tool_call_id / session_id）
components/agent/*  零改动（聊天窗无行内卡，tool_use 走默认对卡）
views/changes/flow/run-control-panel.tsx  零改动（AskCard 已能渲染）
bindings            除 finish_step schema 描述变化外零漂移
```

**版本**：`packages/desktop/package.json` patch +1。

### 用户裁决补充（第六轮）：ask 覆盖 agent 自带 ask，且与 finish_step 彻底解耦

用户原话：

> 流程中向 agent 注入 ask 工具，**覆盖 agent 自带 ask**；可以接受聊天窗口内没有渲染
> （有更好），但要实现**独立 ask 和工具，和 finish_step 解耦**。

三条落地含义：

**① 「覆盖自带 ask」——当前无清除对象，但要写成不变量**

核实：rig 0.43 与 provider 侧都没有内置 ask 工具（`grep -rn "AskUserQuestion|ask_user"`
在 rig 源码只命中两个测试用的 `test:ask_user` 字面量；本仓 `.rs` 零命中）。所以「覆盖」
不是删一个既有工具，而是**三件事**：

```
a) 服务端工具面（provider_tool）：目前只有 web_search 一处挂载，
   additional_params 恒 None —— 若未来 provider 提供原生 ask，
   必须在挂载前显式钳掉（不挂 = 覆盖），此处留一条不变量注释
b) 客户端工具面：ask 进挂载清单后，同名冲突不可达（挂载清单是唯一真源，
   模型无法调用清单外的工具——成员校验已硬禁用）
c) 历史转录里既存的 AskUserQuestion ToolUse：只读静态卡渲染（tool-cards.tsx 已有），
   不参与执行 —— 现状即此，零改动
```

即：**「覆盖」在本期是「定义唯一真源 + 留一条 provider 侧不变量」，不是新增删除逻辑。**

**② 「不渲染也接受」——确认了首期 `AskPayload` 零扩展**

这条把上一节「待定：AskPayload 是否仍需要」的倾向钉成裁决：

```
首期  AskPayload 不扩 tool_call_id / session_id
      聊天窗无行内卡（转录里就是一条普通 tool_use，未配对 → 呈静态形态）
      详情页 AskCard 照常（现有能力，flow-waiting-state-residue 已修好收起）
后续  若要做聊天窗行内卡 → 随通用 ask change 一起扩字段
```

**③ 「与 finish_step 彻底解耦」——比 X-2 更彻底**

X-2 只要求「ask 不是 finish_step 的 enum 值」。**「彻底解耦」还应包含**：

| 解耦面 | X-2 初稿 | 彻底解耦（本轮追加） |
| --- | --- | --- |
| 定义归属 | ask 定义住 `orchestration/ask.rs`，与 `finish.rs` 并列 | ✅ 相同（并列模块，互不引用） |
| 挂载时机 | 无条件追加（Protocol tier） | ✅ 相同，但**不依赖 `finish` 位**——`finish: None` 的会话同样有 ask |
| 代码引用 | `mount.rs` 同时引 `orchestration::finish` 与 `orchestration::ask` | ✅ 两条独立引用，无共享类型 |
| 执行体 | 各自 `ToolExec` 变体 | ✅ `ToolExec::Ask` 不持 spec、不读 finish 状态 |
| decision 角色 | finish = `Some(Decision)` + ask 同时在场 | **新增约束**：decision 步两个工具并存，finish 收路由、ask 问用户，语义互斥但**机制无关** |
| 依赖方向 | — | **新增约束**：`ask.rs` MUST NOT `use crate::finish`，反之亦然（可用 grep 守卫） |

最后一行是可机械验证的：`grep -n "finish" orchestration/src/ask.rs` 应为零，
`grep -n "ask" orchestration/src/finish.rs` 应只剩「指路 ask 工具」的文案字符串。

### 由此产生的挂载规则（比原方案更简单）

```
mount::definitions(injections, prompt)
  ├ tools 名单过滤（内建七 / explore 六 / archi 九 / change 七+矩阵）
  ├ 无条件追加 ask（Protocol tier）        ← 不依赖任何位
  └ finish.is_some() 时追加 finish_step    ← 与原状一致
```

`SessionInjections` **不需要新字段**（无 `ask: bool`）——ask 是基线能力，恒在场。
这比第四轮设想的「可能需新位」更干净。

### 首期不做（明确边界）

```
├ 通用 ask（explore / archi / debug 会话）—— AskRegistry 选型整体推迟
├ 聊天窗行内卡 + toolCardHandlers —— 需 AskPayload.tool_call_id，随通用 ask
├ agent_answer 命令 —— 通用来源的应答入口
├ 一次 ask 多问题 —— 单问，多次调用
└ options 可空 —— 已裁决非空
```

## 与 steer / finalize 的关系

```
steer：   用户 → 运行中 turn      （反向注入，非阻塞）
ask：     用户 ↔ turn 阻塞问答     （双向，阻塞）
finalize：turn → 编排             （单向，非阻塞，触发收口）

三者的共同点：都是「turn 内可嵌入的外部交互点」
三者的差别：谁发起、是否阻塞、是否产生 step 产出
```

一个有意思的推论：**steer 与 ask 可以共用同一条「turn 内注入 / 取出」通道**。ask 阻塞等一条消息，steer 非阻塞塞一条消息——如果通道是双向的，两者是同一个原语的两个方向。若最终做成一张表（`InteractionRegistry`），steer 落地时可以直接复用 ask 的通道层。

## 已确认可直接复用的设施

- `RunHandle::wait_requested()`（`core/agent/src/runner.rs:168`）已是「先注册 → 再复查 → 再挂起」的无竞态实现，ask 的 `select!` 直接用它，无需自造；
- `AskUserQuestion` 静态卡（`tool-cards.tsx`）的解析逻辑（`parseAskInput` / `parseOptions`）可原地改造成新工具的卡，`tool-call_id` 判据接上即成为活卡；
- `change_flow_answer` 命令签名不需要动。
