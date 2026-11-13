# 提案: steer-running-agent-step

> **变更**: steer-running-agent-step
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/steer-running-agent-step/explore.md`（三轮裁定全部收敛：UI 落点 = 抽屉 step 会话窗口、送达 = 收口即 Err、通道 = 通用能力键 = core session id、前端不加判据、ask 阻塞期一律 Err 且不加禁用态、不合并、不留痕）
> **范围接缝**: 本变更只做**对一条正在跑的 turn 注入一条 user 消息**（steer）：① `core/agent` 落地会话级单向注入通道 `SteerRegistry`（键 = core session id，与 `StopRegistry` / `AskRegistry` 同址同型）+ `AgentSession` 第四件套；② `infra/agent` 的 loop 在**请求边界** drain 并把补充说明逐条推成独立 `Message::user` + 独立密封事件 + 独立 ack；③ 壳层 `agent_steer(root, session_id, text)` 命令与三态不可送达文案；④ 前端 headless `steer` / `steering` + `AgentInput` 补充说明变体 + 抽屉 step 面板接 composer；⑤ 前端适配层与实时路支持「一轮多条 user 消息」（否则 steer 文本会被静默吞掉，见「现象四」）。MUST NOT 做：排队续跑（队列非空即 Continue 重开一轮）、ask 阻塞期排队补投、超时自动补投、把补充说明并入 `tool_results` 那条消息、新增 `SystemNotice` 留痕变体、steer 专用工具（steer 是**命令面**能力，模型侧不可见）、其余会话面（explore / archi / 调试）的 composer 接线。
> **前序**: `finish_step`（`agent-finalize-tool-for-flow-steps`，已归档）与 ask 工具（`ask-tool-in-flow`，已归档）已落地，本变更是 explore「落地次序」中的第三件；`AskRegistry` 的形态与锁纪律为本变更的直接样板（`core/agent/src/ask.rs`、`control.rs::answer`）。

---

## 问题

### 现象一：跑着的那一步只能「干看 + 停止」

变更流程跑到某个 WorkerAgent step 时，用户能打开该步的会话窗口（`DetailDrawer` → `SessionTranscriptPanel` → `AgentSessionView`），但那是一个**只读消费点**：`desktop-change-flow-view`「节点会话转录联动」明文写着「不接 composer 插槽、不呈现发送 / 停止」，组件侧也确实没有插槽接线。另一侧左栏「通用 Agent」面板虽然自带 composer，但 `use-change-chat` 的判据矩阵在 `activeRun !== null` 时整面板 `disabled`（理由「流程运行中」）。

于是用户在最有价值的窗口里只有一个动作：**停止**。而「模型跑偏了，我补一句话」这个需求没有任何物理通道。

### 现象二：ask 不替代它，`finish_step` 更不

三个东西看着都在「轮内」，实际正交：

```
            谁发起        阻塞？   产生 step 产出？   实现状态
steer       用户 → turn   非阻塞   否（纯注入）      ← 未实现（本文）
ask         用户 ↔ turn   阻塞     否（纯问答）      已归档
finish_step turn → 编排   非阻塞   是（收口载荷）    已归档
```

`ask` 是**模型发起**的阻塞：用户只能回答它问的那个问题，不能主动补一句；且它只在模型调用时才存在，其余时间窗口零入口——而它阻塞的那段时间（几十秒到几分钟）恰恰是最想补话的窗口（见「待决 5」的裁决与代价）。

### 现象三：物理缺口恰在 loop 的请求边界

`sdk/loop.rs` 的 turn 内部是「请求 → 工具调用 → 工具结果入史 → 再请求」的循环（`turn_index` 递增），直到模型不再要工具。**turn 内有很多个「正要发请求」的时刻，而层与层之间没有任何一条能塞东西进去的路**：

```
session_pump ──questions(一轮一命: recv 一次)──► loop::run
     ▲                                            │
     │ observations(引擎 → 内核，单向)              │ 请求 / 工具 / 请求 ...
     └────────────────────────────────────────────┘
```

`questions` 是「轮提问」，一轮只收一次（一轮一命），**不是**「轮内追加消息」的通道；`observations` 是引擎 → 内核方向。所以「把一句话送进已开跑的 turn」需要一条**新的单向通道**，方向为 `壳层 → 引擎 loop`。

### 现象四：前端把「每轮一条 user 气泡」写死了（explore 未点明的必需改动）

`explore.md` 的结论是「`AgentSessionView` 的消息渲染零改动——steer 落成普通 user 消息，既有 user 气泡直接呈现」。**本轮核实代码后该结论不成立**，两处都会把 steer 吞掉：

```
① 折叠路（重放 / 收口）packages/desktop/src/lib/agent-adapter.ts: entryToMessages
   const question = group.events.find(role === 'user')   ← 取「组内首个」
   ... 其余 role = 'user' 事件既不进 aux、也不进 assistant 部件聚合 → 静默丢弃

② 实时路 packages/desktop/src/hooks/use-agent-session.ts: applyUserQuestion
   id = turn-<turnId>-user  ← 该 id 已被该轮提问占用
   if (messages.some(m => m.id === id)) return messages  ← 直接 return，steer 气泡不出现
```

后果：用户点了发送、后端 `Ok`、store 里确有这条消息，**但界面上永远看不到**；而收口时折叠整表替换会把它彻底抹平。这不是渲染细节，是本变更的功能主线之一——若不改这两处，「能发消息」与「能看到自己发过消息」会不一致，且没有任何报错。

### 症结

steer 不是「给抽屉加个输入框」，而是**一条跨四层的注入通道**：注册表（core）→ 通道挂载（core kernel + infra 泵）→ 请求边界 drain（infra loop）→ 动作面与多 user 气泡呈现（前端）。其中「多 user 气泡」这一层是既有前端模型的一个隐含假设（每轮一条 user 气泡），本变更必须显式打破它并给出确定性规则（首条 = 提问、其余 = 补充说明，id 不撞）。

---

## 提案

一句话形态：**用户 → turn 的单向非阻塞注入：在 loop 的每个请求边界把在途补充说明逐条 drain 成独立 user 消息（独立密封事件 + 独立 ack），送达不了就 Err，不排队、不续跑。**

```
SteerRegistry (新，仿 AskRegistry/StopRegistry)
   键 = core session id → mpsc::UnboundedSender<SteerMessage{ text, ack }>
        │
agent_steer(root, session_id, text)  ──► ① ask.is_pending?        → Err（等待应答中）
  （壳层薄包装）                        ② steer.request            → Err（会话已收口）
                                        ③ ack.await                → Err（生效前结束）
        │
        ▼
  begin_turn: 内核登记 sender（与 StopRegistry 同址）     converge: 除名
        │
        ▼
  loop::run(..., steers: UnboundedReceiver<...>, ...)
     loop {
       stop 快检
       drain: while try_recv → 密封 Message{role:User} → history.push(Message::user) → ack.send(())
       defend
       请求 → 流式 → 收口 / 工具执行（ask 可能阻塞，够不到 drain）
     }
```

### 裁决表（全部已定，含被拒备选）

| # | 议题 | 裁决 | 被拒备选与理由 |
|---|---|---|---|
| D1 | UI 落点 | **抽屉 step 会话窗口**（`SessionTranscriptPanel` 接 composer）；左栏「通用 Agent」面板的 run 期禁发判据矩阵不动 | **被拒：左栏面板**——run 期整面板 `disabled` 的判据是为「不许在流程跑的时候另起 chat 轮」立的，放开它会造出「同一 change 两条并发会话」的第二真相 |
| D2 | 送达语义 | **不可送达即 Err**（收口 / ask 阻塞 / 竞态三种），不排队、不续跑 | **被拒：轮后排队续跑**——walker 的 `run_worker` 要进化成「跑到队列空」，step 语义（一次 step = 一次会话轮）开始漂，且 evaluator / decision 的「末条消息」协议会被第二轮输出污染 |
| D3 | 通道归属 | **通用能力落 `core/agent`**，键 = core session id，不绑 change 来源 | **被拒：通道住 orchestration**（通用会话没有 change 键）；**被拒：复用 `agent_start` 带 current sessionId**（会在同一会话上开第二条并发 drive，`seq_base` 双取同值撞主键、Stop 同键覆盖误杀——方案外） |
| D4 | 前端判据 | **不加**（composer 恒可用；靠后端 Err 说话） | **被拒：按库态 `running` 判可用**——`calibrate_interrupted_runs` 只标定 `RunRecord`，不动 `AgentRunRecord`（轮行），崩溃残留的轮行永远是 `running`，判据会长期错向 |
| D5 | ask 阻塞期 | **一律 Err**，composer 不加禁用态；三条 Err 文案各自独立 | **被拒：在 `execute_ask` 的 `select!` 加第三条臂收 steer**（物理可行，但等于把「不排队」改成「缓冲到工具结果入史后落下」的排队语义，与 D2 冲突） |
| D6 | 多条 steer | **不合并**——每条独立 `Message::user` + 独立密封事件 + 独立 ack | **被拒：合并成一条**——用户「分几次说」的边界丢失，前端只能有一个二值 ack（全成 / 全不成） |
| D7 | 留痕 | **不加** `SystemNotice{steer}`——既有 user 消息即留痕 | **被拒：加通知变体**——`AgentEventKind` 加变体的成本（穷尽性、适配层分派、前端 data 部件）换不来信息量：轮中段的 user 事件本身就是判据 |
| D8 | 通道实现 | `mpsc::UnboundedSender`（`request` 同步 `send`，无容量失败面） | **被拒：bounded + `try_send`**——多出「队列满」第四种 Err 成因，而人打字的量级下该面永不可达，白增一条文案与一个测试分支 |
| D9 | 接收端位置 | `loop::run` 的**位置参数**（不进 `LoopTurn`） | **被拒：`LoopTurn.steers`**——`try_recv` 需 `&mut` 接收端而 `LoopTurn` 以 `&` 携带；为此把 `LoopTurn` 改成可变借用会波及 46 处构造面与整个 loop 的借用形态，收益为零 |
| D10 | 前端呈现 | **必须改适配层与实时路**（首条 user = 提问、其余 = 补充说明，id `evt-<seq>`） | **被拒：前端零改动**（explore 原结论）——核实后会被丢弃 / 去重（见「现象四」），是本变更的功能缺口而非打磨项 |

### 三层改动（本次一次做完）

```
第一层 · 会话地基（core/agent）
  steer.rs（新）      SteerRegistry { slots: Mutex<HashMap<String, UnboundedSender<SteerMessage>>> }
                       new / register / request（miss 即 Err）/ remove（miss 幂等）
                      SteerMessage { text: String, ack: oneshot::Sender<()> }
  ask.rs（扩）        is_pending(session_id) -> bool（slots.contains_key，O(1)）
                      —— ask 阻塞窗口的唯一真相：此刻 SteerRegistry 条目仍在场，只看 miss 判不出来
  runner.rs（扩）     AgentSession 增第四件套 steers: UnboundedSender<SteerMessage>
  kernel.rs（扩）     SessionKernel::new 第 5 参 Arc<SteerRegistry>；
                      begin_turn 登记（紧随 StopRegistry / streams）→ converge 除名
  lib.rs（扩）        导出 SteerRegistry / SteerMessage

第二层 · 引擎与壳层
  infra/agent/src/sdk/runner.rs   open_session 建 unbounded channel，接收端交 session_pump → loop::run，
                                  sender 回填 AgentSession（PumpPayload / EngineFacade 零改动）
  infra/agent/src/sdk/loop.rs     run(..., steers, ...)：请求边界 drain（单条语义 + 逐条 ack）
  infra/agent/src/compose.rs      compose_turn 增 steer_registry（只给内核）
  src/main.rs                     app.manage(Arc::new(SteerRegistry::default()))
  src/commands/exec/mod.rs        agent_steer(...) -> Result<(), String>（三态判据 + ack）
  src/commands/mod.rs             all_commands! 登记 agent_steer
  packages/desktop/src/types/generated/bindings.ts   生成物

第三层 · 前端
  hooks/use-agent-session.ts      steer(text) + steering（pending）+ applyUserQuestion 分化
  lib/agent-adapter.ts            entryToMessages：首条 user → 提问；其余 → 补充说明（evt-<seq>）
  components/agent/agent-session-view.tsx   AgentSessionActions 增 steer / steering
  components/agent/agent-input.tsx          hidePermissionMode + pending 两个可选位
  views/changes/flow/session-transcript-panel.tsx   composer 插槽（补充说明 + 停止）
```

### 通道形状与键空间（与 `StopRegistry` / `AskRegistry` 同址）

```rust
// core/agent/src/steer.rs（新）
pub struct SteerMessage { pub text: String, pub ack: oneshot::Sender<()> }

#[derive(Default)]
pub struct SteerRegistry { slots: Mutex<HashMap<String, mpsc::UnboundedSender<SteerMessage>>> }

impl SteerRegistry {
    pub fn new() -> Self;
    pub fn register(&self, session_id: &str, tx: mpsc::UnboundedSender<SteerMessage>);
    /// miss / send 失败同归 `Err`（两者都是「该会话当前没有在跑的轮」）
    pub fn request(&self, session_id: &str, msg: SteerMessage) -> Result<(), String>;
    pub fn remove(&self, session_id: &str);   // miss 幂等
}
```

三张会话级表（`StopRegistry` / `AskRegistry` / `SteerRegistry`）键空间相同、生命周期同址（`begin_turn` 登记、`converge` 除名），互不引用。语义刻意不同：

```
AskRegistry.answer   受理语义（槽 take 后 send 失败仍 Ok）—— 幂等由「槽已移除」承载
SteerRegistry.request 送达语义（自带 ack，逐条确认）—— 所以不能照抄 answer 的返回值处理
```

### 三态判据与锁纪律

```
agent_steer(root, session_id, text):
  ① let pending = { ask.is_pending(sid) }        ← 取 ask 锁即刻释放
     if pending → Err「等待应答中，补充说明不可送达」
  ② { steer.request(sid, msg)? }                  ← 取 steer 锁即刻释放（miss → Err「会话已收口，…」）
  ③ ack.await.map_err(|_| 「未送达（会话在补充说明生效前结束）」)

判据次序不可交换：ask 阻塞期间 SteerRegistry 条目仍在场（会话没结束，只是 loop 卡在工具分支），
只看 ② 的 miss 判不出来——is_pending 是那个窗口的唯一真相。
三段各自「块内取放」，MUST NOT 任何两段嵌套（沿 control.rs::answer 的既有纪律；反向嵌套会与
execute_ask 的 register（引擎线程持 ask 锁）构成双向等待面）。
```

### drain 点与单条语义（已对照 `loop.rs` 实际结构）

```
loop {
  if handle.stop_requested() { return }              ← 轮间快速路径
  while let Ok(steer) = steers.try_recv() {          ← ★ drain（在 defend 与发请求之前）
      sender.send(Message{role: User, blocks:[Text], parent_tool_use_id: None}).await
      history.push(Message::user(steer.text))        ← 一条新的 user 消息
      let _ = steer.ack.send(())
  }
  history = defend(...)                              ← L2 prune / L3 compaction
  请求 → 流式 → 收口（无 tool_calls → finish_success → return）
       └→ 工具执行（ask 可能阻塞，够不到 drain）→ push tool_results → 回到 loop 顶
}
```

为什么用 `try_recv` **循环**：不合并要求「一次边界把所有在途项全部取走」；只 `recv` 一条会让后几条滞留到下一个边界（可能几十秒后），违背「每条都立刻送」。逐条 `ack.send(())` 就是「不合并」在产品面的可见形态——三条各自的「排队中…」分别复位。

**必须避开的实现陷阱（已核实 rig 0.43.0）**：rig 不合并相邻同角色消息（anthropic 侧一条 rig `Message::User` → 一条 wire `role:user`；openai 侧的聚合只在**单条** rig Message 内部发生）。所以 drain 必须 push 一条**新**的 `Message::user`：

```
正确：  assistant{tool_calls} → user{tool_results} → user{steer}           ✓ 两侧合法
错误：  assistant{tool_calls} → user{tool_results + steer}                ✗ openai 展开后 user 先于 tool → 400
```

drain 点的位置天然满足该约束（此刻 `history` 尾部正是 `Message::tool_results`），语义上也正好是「看到工具输出后补一句」。

### 送达三态与「不排队」

```
 t →  请求1 ══流式══> 工具调用 → 执行 → 工具结果入史 → 请求2 ══流式══> 收口
                  ▲          ▲            ▲                        ▲
        (1) 流式中到达   (4) ask 阻塞中   (2) 请求边界到达     (3) 收口瞬间到达
             │          （一律 Err）        │                        │
        缓存于通道       够不到 drain     drain 后进下个请求      本轮已结束 → Err
```

(3) 与 (4) 都是「送不进去」，但成因不同：(3) 窗口毫秒级（`converge` 与 `ack` 的竞态，无法靠加 drain 归零，靠 drop-Err 诚实报出）；(4) 窗口是人答题的时间（几十秒到几分钟），是本变更主动放弃的最大失败面——用户须先答题再补话。

`drop-Err` 的机制是自动的、不需要超时：轮收口时 `loop::run` 返回 → 接收端 drop → 通道缓冲消息 drop → `ack` sender drop → `ack.await` 收 `RecvError` → 命令返 `Err`。

### 前端呈现：首条 = 提问，其余 = 补充说明

```
折叠路（entryToMessages）
  组内首条 role=user  → 提问消息 id `turn-<turnId>-user`
  组内其余 role=user  → 补充说明消息 id `evt-<seq>`，按 seq 序排在提问之后、assistant 之前
  assistant 聚合仍跳过 user 事件（既有逻辑不变）

实时路（applyUserQuestion）
  首条 → 沿用「本地副本原地换 id」归一路径（消息数不增）
  其余 → 以 `evt-<seq>` **追加**（MUST NOT 因同轮已有同名 id 而丢弃）
```

「一个 id 只对应一条消息」是这两路的共同前提：`turn-<turnId>-user` 恒属该轮提问，补充说明不得复用。收口时折叠整表替换会把两路收敛成同一形状。

### 停止是免费的

抽屉原本没有停止入口，但 `agent_stop(root, worker_session_id)` 本就能杀伤：walker 收到 `AgentRunStatus::Stopped` → `Terminal::stopped("会话被终止")`。composer 插进抽屉后，「停止」半边**零后端改动**——composer 插槽的 `AgentSessionActions` 里 `stop` / `running` 已在场。

### 版本与交付纪律

`packages/desktop/package.json` 的 `version` 按「落地时 `HEAD` 现值 +1 patch」相对递增（现值 `0.6.38` → 落地写 `0.6.39`；MUST NOT 写死字面号作判据）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。新增命令 ⇒ `bindings:export` 生成物一并提交。

---

## 能力

### 新增的能力

**无。** steer 全部落在既有能力面内：通道是会话基建的一件设施（与 `StopRegistry` / `AskRegistry` 同址），命令落 `commands/exec` 既有轨道，呈现走既有 `AgentSessionView` / `AgentMessages` / `AgentInput`，抽屉只是首个消费点。

### 修改的能力

- **desktop-agent-execution** —
  - **ADDED**「运行中会话的补充说明注入（steer 通道）」：`SteerRegistry`（键 = core session id，`register` / `request` / `remove`）+ `SteerMessage{text, ack}`、`AgentSession` 第四件套 `steers`、内核 `begin_turn` 登记 / `converge` 除名、`loop::run` 请求边界 drain 的单条语义（密封事件 → 入史 → ack）、drop-Err 送达语义、三态判据（ask 优先，`AskRegistry::is_pending`）、锁纪律、停止与留痕边界、一轮一命不破。
  - **MODIFIED**「agent 执行命令面」：新增 `agent_steer(root, session_id, text) -> Result<(), String>` 薄包装（blank 三参校验 → ask 槽判据 → 注册表投递 → ack 等待；三条不可送达文案各自独立；零新开会话 / 轮、零状态机、零广播），落 `commands/exec` 轨道并登记于单一命令清单。
  - **MODIFIED**「引擎泵一轮一命与 headless 无问收口」：补一条——补充说明注入在**轮内**闭合，泵不复活、不收第二条提问、不新开轮；泵返回即送达终点（剩余项收 `Err`，MUST NOT 排队到下一轮）。
  - **MODIFIED**「MVP 边界与已知限制」：增第 6 条三条边界——送达只在请求边界（pending 可能持续数分钟）、ask 阻塞期不送达且不排队、收口后不续跑；首期消费点 = 抽屉 step 窗口。
- **desktop-agent-chat-infra** —
  - **MODIFIED**「事件适配层(纯函数,保真)」：`foldSessionMessages` 的按轮产出由「提问（该轮 `role = user`）+ 恰一条 assistant」细化为「提问（**组内首条** user）+ **补充说明（组内其余各一条，id `evt-<seq>`，按 seq 序排在提问之后、assistant 之前）** + 恰一条 assistant + aux」；`id 锚点` 段增补充说明 id 规则（两路同源、不得复用 `turn-<turnId>-user`）。
  - **ADDED**「补充说明动作面与 composer 变体（steer）」：实时路的轮中段 user 消息分工（首条归一 / 其余追加，不吞）、headless `steer` + `steering`（null 守卫、`Err` 进既有错误面）、`AgentSessionActions` 增 `steer` / `steering`（additive）、`AgentInput` 增 `hidePermissionMode` / `pending` 两个可选位（默认变体零变化）、判据不预判。
- **desktop-change-flow-view** —
  - **MODIFIED**「节点会话转录联动」：WorkerAgent 步面板由「只读消费点：不接 composer 插槽、不呈现发送 / 停止」改为「接 composer 插槽，呈补充说明输入 + 停止；不呈 permission-mode 档位与新会话入口；可用性不预判，三态由后端 `Err` 说话」；其余只读面（历史重放 / 无 composer 的消费点）形态不变。

### 其余能力零改动（含理由）

- **desktop-agent-chat-infra「会话级统一渲染组件」/「展示组件族」/「统一会话状态基建」正文**：`AgentSessionActions` 的加位是既有要求的明文预留（Module Contract「如需向 composer 插槽传禁用态，仅在 `AgentSessionActions` 上增位（additive）」），`AgentInput` 变体以新增可选位表达（默认变体逐字不变），headless 的 `steer` / `steering` 与既有 `answer` 同型并列——三处均无与本变更冲突的条款，故不整段重写。
- **desktop-change-orchestration / desktop-change-queries / desktop-change-chat / desktop-explore-page / desktop-archi-chat**：walker、编排状态机、详情查询面与其余会话面板零改动——steer 发生在同一 turn 内，不产生编排事件、不改 run 状态；左栏 chat 面板的 run 期禁发判据逐字保留（D1）。
- **desktop-workspace-store / desktop-change-state-store**：store schema 零变更——补充说明落成既有 `role = user` 密封事件（既有 seq 空间与转录单表），无新表、无新列、不新增落库路径。
- **desktop-phase-prompts**：16 份 md 零改动——steer 是命令面能力，模型侧工具面零新增（`RunStarted.tools` 不含任何 steer 名）。
- **desktop-ipc-type-bindings / desktop-app-shell**：命令模板与命名纪律不变，`agent_steer` 落既有 `commands/exec` 轨道；生成物由 `bindings:export` 再生成。
- **repo-archi-model / desktop-crate-layout**：零新 crate、零新依赖边（`core/agent` 用既有 `tokio::sync`；`infra/agent` 已依赖 `core/agent`）。

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/agent/src/steer.rs`（新） | `SteerMessage { text: String, ack: oneshot::Sender<()> }`；`SteerRegistry { slots: Mutex<HashMap<String, UnboundedSender<SteerMessage>>> }` + `new` / `register` / `request`（miss 与 `send` 失败同归 `Err`，文案「会话已收口，补充说明不可送达」）/ `remove`（miss 幂等）；`Default` 派生 |
| `crates/core/agent/src/ask.rs`（扩） | 增 `pub fn is_pending(&self, session_id: &str) -> bool`（`slots.lock()...contains_key`，O(1)，即刻释放）；`register` / `answer`（take 语义）/ `remove` 逐字不动 |
| `crates/core/agent/src/runner.rs`（扩） | `AgentSession` 增 `pub steers: tokio::sync::mpsc::UnboundedSender<SteerMessage>`（doc：用户 → turn 单向注入，一轮一命不破） |
| `crates/core/agent/src/kernel.rs`（扩） | `SessionKernel::new(sink, registry, streams, ask_registry, steer_registry: Arc<SteerRegistry>)`（第 5 参）；`RunningTurn` 增 `steer_registry` 字段；`begin_turn` 在 `registry.register` / `streams.begin` 之后 `steer_registry.register(&session_id, agent_session.steers.clone())`；`converge` 内 `steer_registry.remove(&session_id)`（与 `registry.remove` / `ask_registry.remove` 同址） |
| `crates/core/agent/src/lib.rs`（扩） | `pub use steer::{SteerMessage, SteerRegistry};` |
| `crates/infra/agent/src/sdk/runner.rs`（扩） | `open_session` 内建 `mpsc::unbounded_channel::<agent::SteerMessage>()`；接收端作 `session_pump` 新参并透传给 `loop::run`；`AgentSession { steers: steer_tx, .. }`。`PumpPayload` / `EngineFacade` **零改动**（登记发生在内核，引擎不需要注册表） |
| `crates/infra/agent/src/sdk/loop.rs`（扩） | `pub(crate) async fn run(model, turn, history, steers: mpsc::UnboundedReceiver<agent::SteerMessage>, sender, handle)`；主循环顶部（`stop_requested` 快检之后、`defend` 之前）drain：密封 `AgentEventKind::Message{role: User, blocks:[Text], parent_tool_use_id: None}` → `history.push(Message::user(text))` → `ack.send(())`；事件通道关闭即停止 drain 并返回。`LoopTurn` **零改动**（替代方案见 D9） |
| `crates/infra/agent/src/compose.rs`（扩） | `compose_turn(stores, registry, streams, ask_registry, steer_registry: Arc<agent::SteerRegistry>, store, agent, workspace_root, ask_port)`（新实参紧跟 `ask_registry`——两张会话级注册表成组；只透传内核，门面 `EngineFacade` 零改动） |
| `src-tauri/src/main.rs`（扩） | `app.manage(Arc::new(SteerRegistry::default()))`（与 `StopRegistry` / `SessionStreamRegistry` / `AskRegistry` 同址） |
| `src-tauri/src/commands/exec/mod.rs`（扩） | `#[tauri::command] pub async fn agent_steer(steer: State<'_, Arc<SteerRegistry>>, ask: State<'_, Arc<AskRegistry>>, root: String, session_id: String, text: String) -> Result<(), String>`：blank 三参 `Err` → `ask.is_pending` 判据 → `steer.request` → `ack.await`（两个 `State` 先 `Arc::clone`，不跨 await 持借用）；`agent_start_with` 以 `Arc::clone(app.state::<Arc<SteerRegistry>>().inner())` 取出后作 `compose_turn` 新实参（与 `ask_registry` 取法同址同式） |
| `src-tauri/src/commands/change_flow/mod.rs`（扩） | `change_flow_start_with` 的 `compose_turn` 调用（1 处）补 `steer_registry` 实参（取出式同上；worktree 与会话执行根不走此通道）；`ChangeFlowControl` / sink / walker 零改动 |
| `src-tauri/src/commands/mod.rs`（扩） | `all_commands!` 登记 `agent_steer`（紧随 `agent_answer`） |
| `packages/desktop/src/types/generated/bindings.ts`（生成物） | `agent_steer` 命令包装（`bindings:export` 再生成，勿手改） |
| `packages/desktop/src/hooks/use-agent-session.ts`（扩） | `UseAgentSessionState` 增 `steer: (text: string) => void` 与 `steering: boolean`；新增 `useSteer(mirrors, sessionId, root)`（与 `useAskAnswer` 同型：null 守卫、`pending` 置位 / 复位、`Err` 进 `actionError` 保序链）；`applyUserQuestion` 分化「首条归一提问 / 其余以 `evt-<seq>` 追加」 |
| `packages/desktop/src/lib/agent-adapter.ts`（扩） | `entryToMessages` 的 user 分支：组内首条 → `questionMessage`；其余 → 补充说明消息（id `evt-<seq>`、parts 为 text 块直映射、按 seq 序排在提问之后、assistant 之前）；`questionId` 只服务首条 |
| `packages/desktop/src/components/agent/agent-session-view.tsx`（扩） | `AgentSessionActions` 增 `steer: (text: string) => void` / `steering: boolean`（由 `state.steer` / `state.steering` 组装后进 composer render prop） |
| `packages/desktop/src/components/agent/agent-input.tsx`（扩） | `AgentInputProps` 增 `hidePermissionMode?: boolean` / `pending?: boolean`（皆缺省 `false`）；`hidePermissionMode` 时不呈档位行；`pending` 时发送呈「排队中…」并禁用、输入与停止面不受影响；默认渲染零变化 |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`（扩） | 自持形态 `AgentSessionView` 增 `composer` 插槽：`AgentInput`（`idPrefix = "step-session"`、`hidePermissionMode`、`pending = actions.steering`、`running = actions.running`、`onSend = ({ text }) => actions.steer(text)`、`onStop = actions.stop`）；`roleRef` / `sessionId` 缺席时仍空态且不渲染 composer |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 + 1 patch**（现值 `0.6.38` → `0.6.39`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行，MUST NOT 写死字面号） |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| `SteerRegistry`：`register` 后 `request` 命中（sender 收到原文）/ `remove` 后 miss `Err` / miss 幂等 / 接收端已 drop 时 `request` 仍 `Err`（send 失败归同臂） | `crates/core/agent/src/steer_test.rs`（新） | 新建 |
| `AskRegistry::is_pending`：登记 → `true`；`answer` 后 → `false`；`remove` 后 → `false`；未登记 → `false`（O(1) 只读，不改既有 take 语义用例） | `crates/core/agent/src/ask_test.rs` | 新增断言（既有用例零改动） |
| `AgentSession` 夹具补 `steers` 字段（`unbounded_channel` 就地造） | `crates/core/agent/src/runner_test.rs` / `kernel_test.rs` / `crates/infra/agent/src/compose_test.rs` / `worker_test.rs` | 夹具随动（构造点 7 处，编译器兜住） |
| `SessionKernel::new` 第 5 参夹具（57 处调用点随动）；`begin_turn` 后 `SteerRegistry` 可查该键、`converge` 后不可查（与 `StopRegistry` 同址断言） | `crates/core/agent/src/kernel_test.rs` | 改造 + 新增断言 |
| drain 单条语义：一次边界两条在途 → 两条密封 user 事件（`role = user`、seq 单调）+ 两条 `Message::user` + 两次 ack；`history` 尾部 `tool_results` 之后是新 user 消息（MUST NOT 并入同一条） | `crates/infra/agent/src/sdk/loop_test.rs` | 新增（`loop::run` 21 处调用点补接收端实参） |
| 无在途时零副作用（既有用例不回归） | 同上 | 复用（零改动断言） |
| `compose_turn` 第 5 参（紧接 `ask_registry`）夹具（52 处调用点随动），注册表透传到内核 | `crates/infra/agent/src/compose_test.rs` / `worker_test.rs` | 夹具随动 |
| `agent_steer` 四态：blank root / blank session_id / blank text → `Err`（携字段名）；ask 登记在场 → `Err`「等待应答中」；无在跑轮 → `Err`「会话已收口」；命中 → ack 回流后 `Ok` 且等待方收到原文 | `src-tauri/src/commands/exec/mod_test.rs` | 新增 |
| ack 竞态：投递后立刻 drop 接收端 → 命令收 `Err`（「生效前结束」） | 同上 | 新增 |
| `COMMAND_NAMES` / `COMMAND_WRAPPERS` 各 50 → 51 且含 `agent_steer` / `agentSteer`（条目数与锚点随动） | `src-tauri/src/bindings/mod_test.rs` | 改造 |
| `change_flow_start_with` 的 `compose_turn` 调用面（1 处） | `src-tauri/src/commands/change_flow/mod_test.rs` | 夹具随动 |
| `applyUserQuestion` 分化：首条 user → `turn-<turnId>-user` 归一；第二条 user（`seq = 9`）→ 新增 `evt-9` 气泡、消息数 +1、不丢 | `packages/desktop/src/hooks/use-agent-session.test.ts` | 新增 |
| `useAgentSession.steer`：`ipc.agentSteer(root, sessionId, text)` 三参逐字；`root` / `sessionId` 为 null 零 invoke；`steering` 置位 / 成功复位 / `Err` 复位并进 `error` | 同上 | 新增 |
| 折叠路：组内两条 user 事件 → 提问（`turn-<turnId>-user`）+ 补充说明（`evt-9`）+ 恰一条 assistant，序正确；id 两两不等 | `packages/desktop/src/lib/agent-adapter.test.ts` | 新增 |
| `AgentInput`：`hidePermissionMode` 无档位行；`pending` 发送禁用且呈排队中文案、停止面照常；默认变体渲染零变化（既有用例零改动通过） | `packages/desktop/src/components/agent/agent-input.test.tsx` | 新增 |
| `AgentSessionActions` 增 `steer` / `steering` 并从 `state` 组装（既有字段零变化） | `packages/desktop/src/components/agent/agent-session-view.test.tsx` | 新增断言 |
| 抽屉步面板：composer 在场（`step-session-send` / `step-session-stop`）、`onSend` → `actions.steer`、`onStop` → `actions.stop`、无档位行；槽位缺席不渲染 composer（既有空态断言零改动） | `packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx` | 新增 |

### 不要修改

- **`crates/core/agent/src/ask.rs` 的 `register` / `answer` / `remove` 三个方法体**：本变更只加只读查询；三者出现 diff 即越界（take 语义与受理口径是 ask 的既有契约）。
- **`crates/infra/agent/src/sdk/loop.rs` 的 `execute_ask`**：ask 阻塞臂零改动——本期**不**在它的 `select!` 加 steer 臂（D5），该函数出现 diff 即越界。
- **`crates/core/agent/src/kernel.rs` 的盖戳 / 状态机 / 收口缝**：`stamp` 单点、`drive` 的「观测通道关闭即唯一收口缝」逐字不动；只加登记 / 除名两行。
- **`crates/infra/agent/src/sdk/{mount,tools,protect,policy,sandbox,context,compact,resume,normalize,web_search,bash}.rs`**：工具面、写入保护、权限、沙箱、防线、重建、归一化全部零改动——steer 不是工具，`RunStarted.tools` 不含新名。
- **`crates/core/orchestration/**`**：walker / step / control / port / state 零改动——补充说明不产生编排事件、不改 run 状态、不改 step 落账。
- **`crates/infra/store/**`**：schema 与写入面零改动（复用既有 `role = user` 密封事件与 seq 空间）。
- **`crates/core/workflow/**`**：相位表与 16 份 prompt md 零改动。
- **`packages/desktop/src/views/changes/change-detail-view.tsx` / `flow/detail-drawer.tsx`**：页组装与抽屉透传零改动（动作面由 `SessionTranscriptPanel` 内部从 `AgentSessionActions` 取）。
- **`packages/desktop/src/views/changes/hooks/use-change-chat.ts` 与左栏 `ChangeChatPanel`**：run 期禁发判据矩阵逐字保留（D1）。
- **`packages/desktop/src/components/agent/{agent-messages,agent-blocks,tool-cards,session-panel}.tsx`**：对话透镜、块渲染件、工具卡、面板骨架零改动（补充说明落成普通 user 气泡，走既有 user 渲染）。
- **`.dev-team/architecture/models/**` 与 `.dev-team/architecture/api/**`**：零新元素、零新依赖边；`App.Backend.Core.Agent` 等元素的契约文件按现状（`status = "absent"`）不动——本变更新增的公共面（`SteerRegistry` / `agent_steer`）只可能在漂移报告的 `apiFindings` 槽位留痕，该槽位不影响 `status` / `passed`（`api:check` 只比对盘上既有契约与生成物，未写契约的元素不产生新生成物）。
- **`openspec/changes/steer-running-agent-step/explore.md`**：只读。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | **同轮送达**：一轮内注入的补充说明在下个请求边界进入本轮史（`Message::user` 出现在 `tool_results` 之后，MUST NOT 并入那条消息），转录出现 `role = user` 的密封事件（seq 单调、由内核单点盖戳），会话 id / 轮 id 不变，零新会话 | `loop_test`（脚本化流缝：一次工具轮 → 注入 → 下一请求史断言）+ `kernel_test`（登记 / 除名） |
| AC-2 | **不合并**：同一边界前到达三条 → 三条独立密封 user 事件 + 三条独立 `Message::user` + 三次 ack（三条命令各自 `Ok`）；无滞留到下一请求边界的项 | `loop_test` |
| AC-3 | **收口即 Err**：轮已收口（`converge` 后 / 接收端已 drop）再注入 → `Err`「会话已收口，补充说明不可送达」；不排队、不 Continue 重开、会话状态零改动 | `exec/mod_test` + `steer_test` + `kernel_test` |
| AC-4 | **ask 优先判据**：ask 阻塞期间注入 → `Err`「等待应答中，补充说明不可送达」，且该文本不入通道、应答后不补投；`AskRegistry::is_pending` 为唯一判据（`SteerRegistry` 条目此刻仍在场） | `exec/mod_test`（三态文案逐条）+ `ask_test`（`is_pending` 四态） |
| AC-5 | **ack 竞态诚实**：投递成功但 ack 前轮收口 → `Err`「未送达（会话在补充说明生效前结束）」；零超时机制、零额外清理表项 | `exec/mod_test` + `steer_test` |
| AC-6 | **停止不改写转录**：补充说明在途时停止 → 轮收敛 `stopped`、命令收 `Err`、转录无该文本事件、注册表键已除名（后续 `request` → `Err`） | `loop_test`（停止臂）+ `kernel_test` |
| AC-7 | **一轮一命不破**：全程恰一个泵、恰一个提问（`questions.recv` 一次）、恰一个轮终态；`EngineFacade` / `PumpPayload` / `LoopTurn` 三处零改动（`git diff` 可验） | `loop_test` + 源码复核 |
| AC-8 | **命令面纪律**：`agent_steer` 为四段薄包装（blank 三参校验 → ask 判据 → 注册表投递 → ack），blank 臂携字段名；命令体零新开会话 / 轮、零状态机、零广播；登记在 `all_commands!` 与 `COMMAND_NAMES`（50 → 51）；`bindings:export` 后再生成零 diff | `exec/mod_test` + `bindings/mod_test` + `bindings:export` 后 `git diff` |
| AC-9 | **前端可见（实时路）**：运行中收到轮中段 `role = user` 密封事件 → 新增 `evt-<seq>` user 气泡（MUST NOT 被「同轮已有同名 id」丢弃），提问气泡仍恰一条 | `use-agent-session.test` |
| AC-10 | **前端可见（折叠路）**：含补充说明的转录重放 → 该轮产出 提问（`turn-<turnId>-user`）+ 补充说明（`evt-<seq>`）+ 恰一条 assistant，序为 提问 → 补充说明 → assistant；两路 id 同源、两两不等 | `agent-adapter.test` |
| AC-11 | **动作面与判据不预判**：headless `steer` 三参逐字、null 守卫零 invoke、`steering` 提交置位 / 成功与 `Err` 均复位、`Err` 进既有错误面；已收口步仍可点发送并由后端 `Err` 说话（前端零判据） | `use-agent-session.test` + `session-transcript-panel.test` |
| AC-12 | **composer 变体加位**：`hidePermissionMode` 时无档位行；`pending` 时发送禁用且呈「排队中…」、输入可编辑、停止入口照常；默认变体（两位缺省）渲染与交互逐字不变（既有用例零改动通过） | `agent-input.test.tsx` + `agent-session-view.test.tsx` |

> **验收表口径**：AC-1…AC-12 是本变更的**能力验收面**（每条对应 `specs/` 下某个 delta requirement 的行为判据）。静态检查链、全量测试与版本递增是全仓既有的交付门，由主基线承载而非本变更引入（见下「验证命令」）。

### 验证命令

**能力验收**（逐包定向测试）：

```powershell
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p dev-team
pnpm -C packages/desktop exec vp test run src/hooks/use-agent-session.test.ts src/lib/agent-adapter.test.ts src/components/agent src/views/changes/flow
```

**交付门**（全仓既有门，非本变更能力判据；版本行按「落地时 HEAD +1 patch」相对递增核对）：

```powershell
cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml --all-targets
pnpm -C packages/desktop run bindings:export
pnpm -C packages/desktop run check      # server:check → bindings:export → api:check → client:check
pnpm -C packages/desktop test
git diff HEAD -- packages/desktop/package.json   # version 行恰 +1 patch
```

> **验收面口径**：验收止于**单元测试**。真机 E2E（真实 provider 端点下的「运行中补一句」往返）不属本工作流自动化验证面；`loop_test` 以 rig `test-utils` 流缝脚本化覆盖「drain 入史 → 下个请求可见 / 多条的逐条 ack / 停止截停 / 收口 Err」四条路径，这四条即本变更的全部行为面。

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 前端「每轮一条 user 气泡」的隐含假设被打破**（`entryToMessages` 取 `find`、`applyUserQuestion` 按 id 去重） | 不改则 steer 文本永久不可见（无报错），用户会认为「发送失败/没生效」；改错则提问 id 与补充说明 id 相撞 → 收口整表替换时消息重挂 | 高 | AC-9 / AC-10 双向锁定（实时路 + 折叠路各一条用例）；id 规则写进 spec（提问恒 `turn-<turnId>-user`、补充说明恒 `evt-<seq>`）；`assistantMessage` 的 user 跳过逻辑零改动（补充说明不进 assistant 部件） |
| **R2 契约面波及：`SessionKernel::new` 57 处 / `compose_turn` 52 处 / `loop::run` 21 处 / `AgentSession` 构造 7 处** | 机械改动量大，易引入无关断言漂移 | 高 | 全部为加实参（编译器兜住，零运行期静默）；`loop::run` 的接收端用 `unbounded_channel()` 就地造（不引入共享静态，避免测试间串表）；不把接收端放进 `LoopTurn`（D9，否则要动 46 处 `LoopTurn` 夹具与借用形态） |
| **R3 ask 与 steer 的锁序** | `agent_steer` 若把 ask 判据与 steer 投递写进同一块（同时持两把锁），会与 `execute_ask` 的 `register`（引擎线程持 ask 锁）构成双向等待面 | 中 | spec 明写「三段各自块内取放、MUST NOT 嵌套」，沿 `control.rs::answer` 既有纪律；命令体分三段显式书写；`is_pending` 只读含 `contains_key`（无锁内回调） |
| **R4 pending 可能持续数分钟**（模型正在跑长 bash） | 用户点了发送后长时间「排队中…」，容易被当成卡死 | 中 | 这是 drain 点位置的固有结果（已在 spec「MVP 边界」第 6 条留痕）；`pending` 只禁用发送按钮（输入可继续编辑、停止可用），且有明确文案；不做轮询、不加超时（超时会造出「假 Err」） |
| **R5 补充说明被上下文防线裁掉**（drain 之后紧接 `defend`） | 极端水位下刚补的话可能在下一请求前被 prune 成占位 | 低 | drain 位于 `defend` 之前、尾部追加（prune 保首条 + 轮对裁剪，尾部最新内容优先级最高）；若真发生，转录里仍有原文（密封事件先于 prune），用户可核对；列为已知代价，不引入「steer 免裁」特权（那会破坏防线单点） |
| **R6 与 ask 阻塞窗口的失败面** | ask 阻塞期（人答题的几十秒到几分钟）一律 `Err`，用户须先答题再补话 | 中 | 裁决已定（D5）：不做排队、不在 `execute_ask` 加 steer 臂；三条 `Err` 文案各自可辨（成因 + 用户下一步），前端不预判不隐藏入口 |
| **R7 与 `finish_step` 收口的交互** | 收口后残留的补充说明若被「补投」会造出第二次请求，破坏「一轮一收口」与 evaluator / decision 的末条消息协议 | 低 | `converge` 除名 + 接收端随 `loop::run` 返回 drop（双保险），残留一律 `Err`；`finish_step` 收口路径**就地 return**（不回循环顶），该批残留永远不会被 drain |
| **R8 `bindings` 与命令清单门** | 新增命令未补录 → 交付门红 | 低 | `COMMAND_NAMES` / `COMMAND_WRAPPERS` 条目数 50 → 51 与两条 contains 断言同批更新；`bindings:export` 后生成物一并提交（`agent_steer` 紧随 `agent_answer`） |
| **R9 版本与生成物纪律** | 交付门红 | 低 | `package.json` 按「落地时 HEAD 现值 +1 patch」相对递增（现值 `0.6.38` → `0.6.39`，MUST NOT 写死字面号作判据）；`Cargo.toml` crate 版本行 MUST NOT 随动；零新 crate ⇒ 架构模型零改动 |
| **R10 首期只有抽屉一个消费点** | 通道是通用能力但只有一处接线；后续会话面（explore / archi / 调试）接线时若各自复制 composer 逻辑，会偏离组件族单源 | 低 | spec 明写「通道通用、首期消费点 = 抽屉 step 窗口」；变体能力（`hidePermissionMode` / `pending`）收在 `AgentInput` 内，后续消费点只需接线一行 `steer` 动作面 |
| **R11 吸附层：一轮内两个吸顶** | 补充说明是第二条 `role = 'user'` 消息，既有「每条 user 消息无条件吸附」契约会让一轮内出现两次吸顶（提问被顶走、补充说明接续）。不是 bug 但视觉上「提问提前消失」 | 中 | 明确**接受**（零改动）：`agent-virtual.ts` 的 `userTurnSpans` 本就按「每条 user 消息」产条（`span = 下一 user 消息 start − 本条 start`），补充说明天然落在同一规则内；spec 明写 MUST NOT 为它另立吸附判据（另立会引出「哪条才是该轮提问」的前端轮信息穿线，收益为零） |
