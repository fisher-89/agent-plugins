# 设计: steer-running-agent-step

> **变更**: steer-running-agent-step
> **状态**: 设计定稿（基于已定稿 proposal + 3 份 delta spec）
> **基于**: `proposal.md`（问题四现象 / 提案 / D1–D10 / 三层改动 / 变更范围 / AC-1…AC-12 / R1–R11）与 `specs/{desktop-agent-execution,desktop-agent-chat-infra,desktop-change-flow-view}/spec.md`
> **性质**: 新增**会话级单向注入通道**（`SteerRegistry`，与 `StopRegistry` / `AskRegistry` 同址同型）+ 引擎 loop 的**请求边界 drain** + 一条 IPC 命令（`agent_steer`）+ 前端 `steer` 动作面与「一轮多条 user 消息」的适配层规则。其余落库面 / store schema / 工具面 / 编排层零变更。
>
> **本篇边界**: `core/agent`（新注册表 + `AskRegistry` 只读判据 + `AgentSession` 第四件套 + 内核登记 / 除名）、`infra/agent`（建通道 + 泵透传 + loop drain + 组合根注入）、Tauri 壳（注册表托管 + 一条新命令 + 两处 `compose_turn` 实参）、前端（headless 动作面 + 适配层多 user 规则 + `AgentInput` 两可选位 + 抽屉步面板接 composer）+ 版本一行。**零新增 crate、零新增依赖边、零架构模型改动**（`infra/agent` 已依赖 `core/agent`）；**零 store 改动**（补充说明落成既有 `role = user` 密封事件）；**零新出线类型**（`SteerMessage` / `SteerRegistry` 不出线，只出 `agent_steer` 命令）。

---

## 提案与规格同步状态

proposal 的三层改动、D1–D10 十条裁决与 3 份 delta spec 均已定稿。本设计不重复其论证，只做四件事：

1. **把 spec 已钉语义、未钉落点的形状收敛到具体签名**（见「关键设计决策」DD1–DD12）：`SteerRegistry::request` 的 Err 归因口径、loop 内 drain 的私有函数形态与三步序、接收端的位置参数落点、前端「提问 / 补充说明」的判据来源、折叠路 parts 构造单点、补充说明的插入锚点、`AgentInput.pending` 的可观测形态、`steering` 的 boolean 口径与错误面保序位置、命令体锁纪律与 `text` 校验口径、参数位置与铸造点、吸附层零改动、命令 async 且不跨 await 持 `State`。
2. **把 proposal「变更范围」的机械计数按工作树实测校准**并留痕（见「起点核实 - 事实校准」）：`SessionKernel::new` 57 处、`compose_turn` 52 处调用、`loop::run` 21 处、`AgentSession` 字面构造 7 处。
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略（测试由独立相位承接）。
4. **把三处「非显然正确」写成可实现形态并留痕**：drain 必须 push **新** `Message::user` 而不并入 `tool_results`（rig 0.43.0 不合并相邻同角色消息）、`drop-Err` 的机制是自动的且 MUST NOT 加超时、`ask` 阻塞期为什么只看 `SteerRegistry` 未命中判不出来。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **core/agent**：`ask.rs:1-58`（`register:30-39` / `answer:41-51`（take 语义）/ `remove:53-57`——三者逐字不动面；表内无只读查询方法，`is_pending` 是新面）；`steer.rs` **不存在**（全仓 `grep -rn "steer\|Steer"` 零命中）；`runner.rs:153-162` `AgentSession { observations, questions, handle }`（三件套，`#[derive(Debug)]`）；`kernel.rs:125-153` `SessionKernel::new(sink, registry, streams, ask_registry)` 四参与四字段；`kernel.rs:196-201` `begin_turn` 的登记序（`registry.register` → `streams.begin`）；`kernel.rs:353-395` `converge`（`registry.remove:387` / `ask_registry.remove:389` 同址、`#[allow(clippy::too_many_arguments)]` 已在）；`lib.rs:1-12` 模块清单与 `:30-52` `#[cfg(test)]` 模块清单（均字母序）。
- **infra/agent**：`sdk/loop.rs:171-178` `run(model, turn, history, sender, handle)` 五参；`:241-249` 主循环顶 + 停止快检（drain 落点 = `:249` 与 `:252` 之间）；`:256` `defend`（续跑循环第一句）；`:996` `execute_ask` 的 `select!`（**零改动面**）；`sdk/runner.rs:244-252` `Ok(AgentSession { observations, questions, handle })`；`:403-411` `session_pump` 七参；`:471-489` `LoopTurn` 铸造单点；`:491-505` 泵 `select!`（`biased`，`loop::run` 为第一臂）；`compose.rs:212-241` `compose_turn` 八参；`:229-234` `SessionKernel::new` 调用；`:425-437` `composed_with_runner` 测试夹具。
- **壳层**：`main.rs:110-129` 三张会话级表同址托管（`StopRegistry` / `SessionStreamRegistry` / `AskRegistry`）；`commands/exec/mod.rs:150-155` 三句柄取出形态（`app.state::<Arc<…>>()` + `Arc::clone(….inner())`）；`:205-227` `agent_answer`（薄包装先例，同步命令）；`commands/change_flow/mod.rs:155-157` 注册表取出 + `:166-177` `compose_turn` 调用；`commands/mod.rs:50-54` `all_commands!` 的 exec 组登记序（`agent_start` → `agent_stop` → `agent_answer` → `agent_session_watch`）。
- **前端**：`lib/agent-adapter.ts:184-186` `messageId(seq) = evt-<seq>`；`:552-559` `assistantId`；`:561-563` `questionId`（`turn-<turnId>-user` / 旧转录 `evt-<seq>`）；`:566-574` `auxMessage`；`:576-587` `questionMessage`；`:591-616` `assistantMessage`（`role === 'user'` 跳过）；`:618-629` `entryToMessages`（`find` 取**组内首条** user，其余静默丢弃——现象四①）；`hooks/use-agent-session.ts:79-82` `questionIdOf`；`:84-96` `questionParts`；`:101-135` `applyUserQuestion`（`messages.some(id)` 即 return——现象四②）；`:262-272` `acceptEvent` 的 user 事件分派；`:607-625` `useAskAnswer`（`actionError` 型先例）；`:686` `error` 三源保序合并；`components/agent/agent-session-view.tsx:15-20` `AgentSessionActions`（四字段）、`:136-141` actions 组装、`:151` composer 插槽；`components/agent/agent-input.tsx:12-38` `AgentInputProps`、`:69-135` `ComposerToolbar`（档位 + 停止 + 发送同排）；`components/agent/index.tsx:1-8` barrel（`AgentInput` / `AgentSessionActions` 已在册，本变更**零新增出口**）；`views/changes/flow/session-transcript-panel.tsx:1-60`（自持形态、零动作面、无 composer 插槽）。
- **架构模型**：`App.Backend.Core.Agent` / `App.Backend.Infra.AgentRuntime` / `App.Backend.Shell` 三元素 `api = none`（`.dev-team/architecture/models/**` 零改动；新增公共面只在 `api:check` 的 `apiFindings` 槽位留痕，不影响 `status` / `passed`）。
- **版本**：`packages/desktop/package.json:3` `version = "0.6.38"`；`src-tauri/tauri.conf.json:4` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml:6` `version = "0.1.0"`（crate 版本行，MUST NOT 随动）。

### 事实校准（proposal 措辞 vs 工作树实测；不改需求意图，只定实现形态）

1. **`SessionKernel::new` 调用点 57 处**（`kernel_test.rs` 52 / `lib_test.rs` 1 / `compose.rs` 2（生产）/ `compose_test.rs` 2）。proposal 的 57 与实测一致。
2. **`compose_turn` 52 处调用**（`compose_test.rs` 41 / `worker_test.rs` 9 / `change_flow/mod.rs` 1 / `exec/mod.rs` 1）+ 1 处定义（`compose.rs:212`）。proposal 的 52 与实测一致。
3. **`loop::run` 调用点 21 处**（`sdk/loop_test.rs` 20 / `sdk/runner.rs` 1（生产））。每处必须补接收端实参——这是本变更加实参面**唯一**需要逐点触碰的测试文件（20 处均为直构，无 helper 可绕）。
4. **`AgentSession` 字面构造 7 处**（`kernel_test.rs` 2 / `runner_test.rs` 1 / `compose_test.rs` 2 / `sdk/runner.rs` 1（生产）/ `worker_test.rs` 1）+ 1 处定义（`runner.rs:153`）。proposal 的 7 与实测一致。
5. **`SessionInjections` / `LoopTurn` / `SessionOpen` 三处零改动**：steer 不消耗任何既有入参位（D9 把接收端放在 `loop::run` 位置参数上），故 proposal R2 列的「46 处 `LoopTurn` 夹具」在本变更**不发生**。
6. **前端 `applyUserQuestion` 的去重发生在 updater 内**（`:110-111` `if (messages.some((message) => message.id === id)) return messages;`）：不走 `pendingId` 的旁观 / 重挂路径同样会被该判据吃掉——这正是现象四②的实现细节，也是 DD4 判据要绕开「只看 pendingId」的原因。
7. **`AgentInputProps.onSend` 的 `permissionMode` 是非可选字段**（`agent-input.tsx:32`）：抽屉插槽的 `onSend = ({ text }) => actions.steer(text)` 只解构 `text`，`permissionMode` 由 `AgentInput` 内部缺省 `bypassPermissions` 提供且下游不消费——类型面零改动。
8. **`tooltip` 内容在未展开时不在 DOM**（`ComposerToolbar` 的发送按钮提示走 `TooltipContent`）：故 `pending` 的「排队中…」若要可观测 / 可断言，**不能**只落在 tooltip 里（DD7）。

---

## 关键设计决策

> D1–D10 见 proposal「裁决表」（已定稿，本篇不重复）。下表 DD1–DD12 是本设计在 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | `SteerRegistry::request` 的 Err 归因 | 一个臂：`slots.get` 未命中 → `Err(「会话已收口，补充说明不可送达」)`；`tx.send(msg)` 失败（接收端已 drop）→ **同一文案同一臂**。MUST NOT 照抄 `AskRegistry::answer` 的「take 后 send 失败仍 `Ok`」 | `answer` 是**受理**语义（幂等由「槽已移除」承载）；steer 是**送达**语义（自带 ack，逐条确认）。若 send 失败还报 `Ok`，用户会看到「发送成功」而消息从未进史——正是现象四要消灭的那类静默失败。两者同归一个臂的理由：从调用方视角「该会话当前没有在跑的轮」是同一个事实（miss 与 drop 都是它） |
| DD2 | drain 的实现形态与三步序 | 私有 `async fn drain_steers(steers: &mut mpsc::UnboundedReceiver<agent::SteerMessage>, history: &mut Vec<Message>, sender: &mpsc::Sender<AgentEventKind>) -> bool`：`while let Ok(steer) = steers.try_recv()` 内依序 ① 发密封事件 ② `history.push(Message::user(text))` ③ `let _ = steer.ack.send(())`；返回 `false` = 事件通道已关闭（调用方 `return history`），此时**不补 ack**（剩余项随接收端 drop 收 `Err`） | 三步序不可换：ack 若先于 ②，命令可能在消息入史前返回 `Ok`（「`Ok` 时该文本必已入该轮史」即不成立）；ack 若先于 ①，转录里可能永远没有该事件。事件通道关闭后继续 drain 毫无意义（消费端已走，落库面不再存在）——`return` 而非 `break` 让剩余项走 drop-Err 的同一机制 |
| DD3 | 接收端的位置与三处零改动 | `loop::run(model, &LoopTurn, history, steers, sender, handle)`（第 4 位置参数）；`LoopTurn` / `PumpPayload` / `EngineFacade` **零改动**；接收端由 `SdkRunner::open_session` 铸通道时创建，经 `session_pump` 新形参（置于 `questions` 之后）透传给 `loop::run` | D9 的落地：`try_recv` 需 `&mut` 接收端，而 `LoopTurn` 以 `&` 携带，放进结构体要把它改成可变借用并波及全部 46 处 `LoopTurn` 夹具的借用形态（收益为零）。`PumpPayload` 也不需要它——引擎侧不认识注册表，登记发生在内核，通道只是「本轮的一条输入线」 |
| DD4 | 实时路「提问 / 补充说明」的判据 | 判据 = **该轮提问气泡是否已在场**：`event.turnId == null`（旧转录）或 `!messages.some(m => m.id === turn-<turnId>-user)` → 按提问处理（沿用既有归一 / 追加）；否则 → 补充说明，id `evt-<seq>` 追加。MUST NOT 以「`pendingQuestionIdRef` 是否在场」为判据 | 判据要同时覆盖两个起点：发起流（发送方本地有副本，走归一路径，消息数不增）与订阅流（旁观 / 重挂，无本地副本）。`pendingId` 只在发起流存在，用它判别会把旁观路拿到的**首条** user 事件误判成补充说明。`turnId == null` 的旧转录不会有补充说明（steer 是本变更新能力），故恒按提问处理即正确 |
| DD5 | 折叠路补充说明消息的 parts 构造 | 复用既有单点 `plansToParts(planBlocks(event.blocks, paired))`（与 `questionMessage` 同一表达式），**不新开**一条「只收 text 块」的分支 | spec 的「parts 为 text 块直映射」在生产数据上与该表达式**逐值等价**（补充说明的 blocks 恒为单个 `Text`），但复用单点意味着后续若 text 块携带 citations / 或载荷加宽，两路不会漂移。新增一条直映射分支 = 第二个语义点，正是适配层要避免的 |
| DD6 | 补充说明的插入锚点 | 与提问**同一 anchor 表达式**：`turnId != null` 时插到 `turn-<turnId>`（该轮 assistant 消息）之前，找不到即追加；重投幂等按自身 id（`evt-<seq>`）判 | 提问先到并已插在同一 anchor 之前，故「插到 assistant 之前」自然得到 提问 → 补充说明 → assistant 的序（spec AC-10）。另立「插到最后一条 user 消息之后」会引出「该轮末条 user 是哪条」的穿线，收益为零 |
| DD7 | `AgentInput.pending` 的可观测形态 | `pending` 时：① 发送按钮 `disabled`（与 `canSend` 合成：`!disabled && !pending && prompt.length > 0`）；② 在工具行**上方**与 `disabled-reason` 同区渲一行可见小字 `data-testid="<idPrefix>-pending"`，文案「排队中…」；输入框与停止入口不受影响 | 事实校准 8：tooltip 内容未展开时不在 DOM，只落 tooltip 等于「不可观测、不可断言」。而 spec 要求「提示文案 SHALL 可辨」，可见文案行是唯一同时满足「用户看得见」与「测试看得见」的形态；位置与既有 `disabled-reason` 同区（同一条竖排信息位），默认变体零变化（`pending` 缺省 false 时不渲） |
| DD8 | `steering` 的口径与错误面位置 | `steering: boolean`（提交置 `true`，成功与 `Err` 收尾都置 `false`），**不引入计数**；`useSteer(mirrors, sessionId, root)` 与 `useAskAnswer` 同型（null 守卫 + `actionError`）；hook 的 `error` 保序链扩为四源：`replay.error ?? chatError ?? ask.actionError ?? steer.actionError` | `pending` 禁发 ⇒ UI 面同时至多一条在途，boolean 足够（后端「不合并」仍是通道契约，供多命令面 / 多消费方并发时依旧成立）。计数会引入「三条在途各自的进度」这一前端状态面，而界面只有一个发送按钮——多出来的状态无人消费。四源 `??` 保序：重放 > 流 > 动作（两个动作源同层，先后只保证确定性） |
| DD9 | 命令体的锁纪律与 `text` 校验 | 三段各自「块内取放、MUST NOT 嵌套」：① `ask.is_pending(&session_id)`（取 ask 锁即释放）② `steer.request(...)`（取 steer 锁即释放；内部 `send` 也在锁内完成）③ `delivered.await`（零锁）。`text` **要**做 blank 校验（`Err("非法 text: 不得为空白")`） | `agent_answer` 不校验 `text`（应答是自由文本，后端不替用户裁量内容）；steer 的 `text` 会落成 `Message::user` 进史并成为下一次请求的输入——空串进去等于给模型塞一条空消息，且序列化层不会替我们兜。锁嵌套面：`execute_ask` 的 `register`（引擎线程持 ask 锁）与本命令的 `request`（持 steer 锁）不构成双向等待的前提是「任一线程不同时持两把」——本命令严格分三段即满足 |
| DD10 | 参数位置与铸造点 | `compose_turn(stores, registry, streams, ask_registry, steer_registry, store, agent, workspace_root, ask_port)`（新实参**紧跟** `ask_registry`，两张会话级注册表成组）；`session_pump(..., questions, steers, observations, handle)`（新形参紧跟 `questions`）；通道在 `open_session` 内 `mpsc::unbounded_channel::<agent::SteerMessage>()` 铸，sender 回填 `AgentSession.steers` | 两处位置都由语义聚合决定：注册表两两成组（同址同型，读代码时一眼看到三张表里哪两张是「会话轮级」）；`questions` 与 `steers` 都是「进 turn 的输入线」，放一起与「出的 `observations`」分列。通道在 `open_session` 铸而非由组合根注入：它是**每轮一条**的实例通道（非进程级共享表），与 `questions` / `observations` 同族 |
| DD11 | 吸附层（一轮两吸顶） | **零改动**：`agent-virtual.ts` 的 `userTurnSpans` 仍按「每条 user 消息」产条，补充说明天然落在既有规则内；MUST NOT 为补充说明另立吸附判据 | 另立判据需要前端知道「该轮哪条才是提问」——实时路要穿轮信息、折叠路要再算一次首条判据，与本变更 D10 的「首条 = 提问」在适配层单点相悖；代价只有一个纯视觉现象（提问提前让位），spec 已明文接受 |
| DD12 | 命令 async 与 `State` 借用 | `agent_steer` 为 `async fn`（必须 `await` ack）；两个 `State` 的 `Arc` 在**首行** clone 出来（`Arc::clone(steer.inner())` / `Arc::clone(ask.inner())`），此后不再触碰 `State`；锁只在 ①② 两段内部短暂持有 | 该命令是本仓第一条「async + `State`」命令（既有 async 命令只吃 `AppHandle`，既有 `State` 命令全同步）。先把 `Arc` 取出即可让三个同步段与 await 段在类型上互不纠缠；锁纪律与 `Arc` 共存不矛盾——锁在 `is_pending` / `request` 的方法体内取放，命令体不持锁跨 await |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 会话级注入注册表（**新增**） | `SteerRegistry { slots: Mutex<HashMap<String, mpsc::UnboundedSender<SteerMessage>>> }`：`new` / `register` / `request`（miss 与 send 失败同归 `Err`，DD1）/ `remove`（miss 幂等）；`SteerMessage { text, ack }` | `crates/core/agent/src/steer.rs`（新）+ `lib.rs` 出口 | `std::sync::{Mutex, HashMap}`、`tokio::sync::{mpsc, oneshot}`（全部既有） | 与 `StopRegistry` / `AskRegistry` 同型的内存表；零编排域类型 |
| ask 只读判据（**修改**） | `AskRegistry::is_pending(&self, session_id: &str) -> bool`（`slots.contains_key`，O(1)）；`register` / `answer` / `remove` 逐字不动 | `crates/core/agent/src/ask.rs` | 既有 | 单行只读方法 |
| 通道挂载（**修改**） | `AgentSession` 增第四字段 `steers: mpsc::UnboundedSender<SteerMessage>` | `crates/core/agent/src/runner.rs` | 既有 | 结构体加位（构造点 7 处由编译器兜住） |
| 内核登记 / 除名（**修改**） | `SessionKernel` 与 `RunningTurn` 各增 `steer_registry: Arc<SteerRegistry>`；`new` 第 5 参；`begin_turn` 在 `streams.begin` 之后 `register(&session_id, agent_session.steers.clone())`；`converge` 内 `remove(&session_id)`（与两张表同址） | `crates/core/agent/src/kernel.rs` | 既有 | 单点收口缝（`converge` 是唯一终态出口） |
| 引擎通道铸造与透传（**修改**） | `open_session` 铸 `unbounded_channel::<agent::SteerMessage>()`；接收端交 `session_pump` 新形参 → `loop::run` 第 4 位置参数；sender 回填 `AgentSession.steers` | `crates/infra/agent/src/sdk/runner.rs` | `agent`（既有） | 既有泵任务（零新任务 / 零新 select 臂） |
| 请求边界 drain（**修改**） | `run(...)` 主循环顶（停止快检之后、`defend` 之前）调 `drain_steers`；单条语义 = 独立密封事件 + 独立 `Message::user` + 独立 ack | `crates/infra/agent/src/sdk/loop.rs` | `agent`（既有） | 既有 async 循环；drain 用 `try_recv`（非 `recv`，不阻塞轮） |
| 组合根注入（**修改**） | `compose_turn` 增 `steer_registry` 实参（紧跟 `ask_registry`）→ `SessionKernel::new` 第 5 参；门面 / `PumpPayload` 零改动 | `crates/infra/agent/src/compose.rs` | 既有 | 纯装配 |
| 壳层托管与命令（**修改**） | `app.manage(Arc::new(SteerRegistry::default()))`；`agent_steer`（薄包装四段，DD9 / DD12）；两处 `compose_turn` 调用补实参；`all_commands!` 登记 | `src-tauri/src/main.rs` + `commands/{exec,change_flow,mod}.rs` | `agent`（既有） | Tauri command 薄包装（三件事纪律） |
| headless 动作面与多 user 规则（**修改**） | `UseAgentSessionState` 增 `steer` / `steering`；`useSteer`；`applyUserMessage` 分化（DD4）；折叠路 `entryToMessages` 首条 / 其余（DD5 / DD6） | `packages/desktop/src/{hooks/use-agent-session.ts,lib/agent-adapter.ts}` | 生成物驱动 IPC（既有） | React hook + 纯函数适配层 |
| composer 变体与消费点（**修改**） | `AgentSessionActions` 增 `steer` / `steering`（additive）；`AgentInputProps` 增 `hidePermissionMode?` / `pending?`（缺省 false）；抽屉步面板接 composer 插槽 | `packages/desktop/src/components/agent/{agent-session-view,agent-input}.tsx` + `views/changes/flow/session-transcript-panel.tsx` | 同上 | 既有组件族加位（默认变体零变化） |

<!-- 无新增 crate、无新增依赖边、无新增架构模型元素：三个既有元素（App.Backend.Core.Agent / App.Backend.Infra.AgentRuntime / App.Backend.Shell）的文件面内部改动；steer.rs 是既有模块内的新文件，不构成新 module 元素（.dev-team/architecture/models/** 零改动）。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀；前端侧写全 `packages/desktop/…`。测试文件与用例归测试相位，本清单只列**实现文件**，另设「编译闭合批」段登记必须随字段面 / 签名收敛的夹具点位。

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `crates/core/agent/src/steer.rs` | `pub struct SteerMessage { pub text: String, pub ack: tokio::sync::oneshot::Sender<()> }`（`#[derive(Debug)]`）+ `#[derive(Default)] pub struct SteerRegistry { slots: Mutex<HashMap<String, mpsc::UnboundedSender<SteerMessage>>> }` 与 `new` / `register` / `request`（DD1）/ `remove`；doc 写明「键 = core 铸会话 id，与 `StopRegistry` / `AskRegistry` 同址同键空间；零编排域类型（无 change id / workspace root）」与「每条消息自带 ack 是『不合并』在类型上的载体」 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/agent/src/ask.rs` | 仅新增 `pub fn is_pending(&self, session_id: &str) -> bool`（`self.slots.lock().expect("ask 注册表锁不可中毒").contains_key(session_id)`）+ doc 一句「ask 阻塞窗口的唯一真相：此刻 `SteerRegistry` 条目仍在场，条目在场不能证明可送达」 | AC-4；`register` / `answer` / `remove` 三方法体**逐字不动**（`git diff` 出现三者的 hunk 即越界） |
| `crates/core/agent/src/runner.rs` | `AgentSession` 增第四字段 `pub steers: tokio::sync::mpsc::UnboundedSender<SteerMessage>`（doc：用户 → turn 的单向注入通道；一轮一命不破） | AC-1；`observations` / `questions` / `handle` 与 `RunHandle` / `AgentRunner` 逐字不动 |
| `crates/core/agent/src/kernel.rs` | ① `SessionKernel` 增 `steer_registry: Arc<SteerRegistry>` 字段，`new` 第 5 参；② `RunningTurn` 增同名私有字段并在 `begin_turn` 的 `Ok(RunningTurn { … })` 回填；③ `begin_turn` 在 `self.streams.begin(&session_id);` 之后加 `self.steer_registry.register(&session_id, agent_session.steers.clone());`；④ `drive` 解构补 `steer_registry`，**3 处** `Self::converge(...)` 调用（提问送达失败 / 落库失败 / 终态出口）与 `converge` 签名各加一参；⑤ `converge` 内紧随 `ask_registry.remove(session_id);` 加 `steer_registry.remove(session_id);` | AC-1 / AC-3 / AC-6；盖戳单点、状态机、`drive` 的「观测通道关闭即唯一收口缝」三处**逐字不动**（只加登记 / 除名两行） |
| `crates/core/agent/src/lib.rs` | `mod steer;`（`mod state;` 与 `mod stream;` 之间，字母序）+ `pub use steer::{SteerMessage, SteerRegistry};`（同位置，`state` 与 `stream` 的出口之间） | 出口面（spec Module Contract）。**不加** `#[cfg(test)] mod steer_test;` 声明——本仓惯例「声明与 sidecar 文件同批」，先加声明会让 `cargo check --all-targets` 因缺文件失败（移交 test-gen 同批加） |
| `crates/infra/agent/src/sdk/runner.rs` | ① `open_session` 内 `let (steer_tx, steer_rx) = mpsc::unbounded_channel::<agent::SteerMessage>();`，`session_pump(...)` 调用多传 `steer_rx`，`Ok(AgentSession { …, steers: steer_tx })`；② `session_pump` 增新形参 `mut steers: mpsc::UnboundedReceiver<agent::SteerMessage>`（紧跟 `questions`，DD10），`loop::run(&model, &turn, payload.history, steers, observations.clone(), pump_handle.clone())` 透传 | AC-1 / AC-7；`PumpPayload` / `EngineFacade` / `SdkRunner` 字段面**零改动**（引擎侧不认识注册表） |
| `crates/infra/agent/src/sdk/loop.rs` | ① `run` 增第 4 位置参数 `mut steers: mpsc::UnboundedReceiver<agent::SteerMessage>`；② 新增私有 `async fn drain_steers(steers, history, sender) -> bool`（DD2 三步序）；③ 主循环顶（停止快检的 `}` 之后、`// 续跑计数轴` 之前）加 `if !drain_steers(&mut steers, &mut history, &sender).await { append_engine_log(…); return history; }` | AC-1 / AC-2 / AC-6 / AC-7；`LoopTurn` / `execute_ask` / 防线 / 收口四步序 / 截断与异常终局**逐字不动** |
| `crates/infra/agent/src/compose.rs` | ① `compose_turn` 增实参 `steer_registry: Arc<agent::SteerRegistry>`（紧跟 `ask_registry`）+ doc 一句「两张会话级注册表成组注入内核；引擎门面不参与」；② `SessionKernel::new(..., Arc::clone(&ask_registry), steer_registry)`；③ `composed_with_runner` 夹具补 `Arc::new(agent::SteerRegistry::default())` | AC-1 / AC-3；`assemble_injections` / `resolve_agent_engine` / `map_protocol` / `ComposedTurn::begin` 逐字不动 |
| `src-tauri/src/main.rs` | `use agent::{AskRegistry, SessionStreamRegistry, SteerRegistry, StopRegistry};`；`setup_hook` 在 `AskRegistry` 之后加一段注释 + `app.manage(Arc::new(SteerRegistry::default()));` | AC-3；`ChangeFlowControl` 构造面零改动（它不认识 steer） |
| `src-tauri/src/commands/exec/mod.rs` | ① `use ::agent::{…, SteerMessage, SteerRegistry, …}`；② `agent_start_with` 取 `let steer_registry = Arc::clone(app.state::<Arc<SteerRegistry>>().inner());` 并在 `compose_turn(...)` 的 `ask_registry` 之后传之；③ 新增 `#[tauri::command] #[specta::specta] pub async fn agent_steer(...) -> Result<(), String>`（DD9 / DD12 四段） | AC-3 / AC-4 / AC-5 / AC-8 |
| `src-tauri/src/commands/change_flow/mod.rs` | `change_flow_start_with` 内加 `let steer_registry = Arc::clone(app.state::<Arc<SteerRegistry>>().inner());`（与 `ask_registry` 同址），`compose_turn(...)` 补该实参（1 处）；`use ::agent::{…, SteerRegistry, …}` | AC-1（流程会话同走该通道）；`ChangeFlowControl` / sink / walker 零改动 |
| `src-tauri/src/commands/mod.rs` | `all_commands!` 内 `$crate::commands::exec::agent_answer,` 之后加 `$crate::commands::exec::agent_steer,` | AC-8（单一登记面，紧随 `agent_answer`） |
| `packages/desktop/src/types/generated/bindings.ts`（生成物） | `agentSteer: (root: string, sessionId: string, text: string) => Promise<null>` 及命令名登记 | `bindings:export` 再生成，**勿手改**（其 `bindings.zod.ts` 同批再生成） |
| `packages/desktop/src/hooks/use-agent-session.ts` | ① `UseAgentSessionState` 增 `steer: (text: string) => void` 与 `steering: boolean`；② 新增 `useSteer(mirrors, sessionId, root)`（DD8）；③ `applyUserQuestion` → 更名 `applyUserMessage` 并分化（DD4，doc 重写为「首条 = 提问归一 / 其余 = 补充说明追加」）；④ hook 返回面 `error` 扩四源、增 `steer` / `steering` | AC-11 / AC-9；`answer` / `stop` / `sendMessage` / 重放 / 订阅调度**逐字不动** |
| `packages/desktop/src/lib/agent-adapter.ts` | ① 新增私有 `steerMessage(event, paired)`（id `messageId(event.seq)`，parts 与 `questionMessage` 同表达式，DD5）；② `entryToMessages` 的 user 分支改「组内首条 → `questionMessage`；其余按 seq 序各一条 `steerMessage`」（DD6）；③ 模块 doc 的「提问（可缺席）+ 恰一条 assistant」口径补「+ 补充说明（各一条）」 | AC-10；`assistantMessage` 的 user 跳过逻辑、`foldSessionMessages`、分组、配对视野、发射层**逐字不动**；`questionId` 只服务首条 |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | `AgentSessionActions` 增 `steer: (text: string) => void` 与 `steering: boolean`（additive）+ doc；`actions` 组装补 `steer: state.steer` / `steering: state.steering` | AC-12；`running` / `loading` / `send` / `stop` 与 `AgentMessages` 透传零改动 |
| `packages/desktop/src/components/agent/agent-input.tsx` | `AgentInputProps` 增 `hidePermissionMode?: boolean` / `pending?: boolean`（皆缺省 false）；`canSend` 合成 `!disabled && !pending && prompt.length > 0`；`ComposerToolbar` 增 `hidePermissionMode`（true 时不渲 `ModeSelect`）；`pending` 的可见文案行渲在工具行上方（与 `disabled-reason` 同区，DD7） | AC-12；默认变体（两位缺省）渲染与交互逐字不变 |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 自持形态 `viewProps` 增 `composer` 插槽：`AgentInput`（`idPrefix="step-session"`、`hidePermissionMode`、`pending={actions.steering}`、`running={actions.running}`、`onSend={({ text }) => actions.steer(text)}`、`onStop={actions.stop}`）；组件 doc 由「只读消费点」改为「接 composer 插槽」 | AC-11（首个消费点）；`roleRef` / `sessionId` 缺席仍空态且不渲染 composer；`askAnswer` 注入位照旧 |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 + 1 patch**（现值 `0.6.38` → `0.6.39`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行）；`dependencies` / `devDependencies` 零变化 | 版本纪律；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` crate 版本行 MUST NOT 随动 |

### 编译闭合批（随字段面 / 签名收敛的夹具点位；MUST NOT 新增用例 / 改断言口径）

> 这些不是「测试编写」，而是「让工作区随签名变化编译通过」的机械适配（漏改即 `E0063` / `E0061`）。

| 面 | 点位（实测计数） | 改动 |
|----|----|------|
| `SessionKernel::new` 调用 | **57 处**：`kernel_test.rs`(52) / `lib_test.rs`(1) / `compose.rs`(2，生产，随实参落地) / `compose_test.rs`(2) | 补第 5 参 `Arc::new(SteerRegistry::default())`（测试侧每用例一条显式实例，**不引入全局静态**——避免测试间串表） |
| `AgentSession` 字面构造 | **7 处**：`sdk/runner.rs`(1，生产) / `kernel_test.rs`(2) / `runner_test.rs`(1) / `compose_test.rs`(2) / `worker_test.rs`(1) | 补 `steers: <trivial sender>`；测试侧就地造 `let (tx, _rx) = mpsc::unbounded_channel();` 后传 `tx`（接收端就地 drop 即「无在途」，与生产语义一致） |
| `loop::run` 调用 | **21 处**：`sdk/loop_test.rs`(20) / `sdk/runner.rs`(1，生产) | 补第 4 实参：既有 20 处传 `mpsc::unbounded_channel::<agent::SteerMessage>().1`（sender 就地 drop ⇒ `try_recv` 立即 `Disconnected` ⇒ 零副作用）；drain 新用例持 sender 推消息（用例归测试相位） |
| `compose_turn` 调用 | **52 处**：`compose_test.rs`(41) / `worker_test.rs`(9) / `change_flow/mod.rs`(1) / `exec/mod.rs`(1) | 测试侧加 `Arc::new(agent::SteerRegistry::default())`；生产两处随阶段三填真值 |
| `AgentInputProps` / `AgentSessionActions` 使用面 | 生产：`agent-session-view.tsx` / `session-transcript-panel.tsx` / 四个既有 composer 薄适配（`archi-chat-composer` / `change-chat-composer` / explore / debug） | 新增字段全部**可选 / additive**，既有构造点无需机械补位（测试面归测试相位） |

> **闭合批的时点不变量**：任一处签名 / 字段面变化，其**全部**调用点（含**生产点**）必须在**同一阶段批次**内收敛——阶段末「工作区可编译 + clippy 全绿」是阶段划分的不变量，MUST NOT 靠收窄自查口径（如只跑 `-p <crate>`）绕过。三处生产点因此必须与引入变化同批：`compose.rs:229` 的 `SessionKernel::new`（第 5 参）、`sdk/runner.rs:248` 的 `Ok(AgentSession { … })`（第四字段 `steers`）、`sdk/runner.rs` 泵 `select!` 里的 `loop::run(...)`（第 4 位置参数）。生产点分支的实际接线（接收端透传、注册表真值）可在后续阶段完成——但**能编译所需的构造补位**不得后置（后置即该阶段末 `E0061` / `E0063` 必红，形成「前置依赖后置」）。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `SteerRegistry::new` | `crates/core/agent/src/steer.rs` | 新增 | `pub fn new() -> Self` | 空注册表（`Default` 派生 + 薄包装，与 `StopRegistry::new` 同形） |
| `SteerRegistry::register` | 同上 | 新增 | `pub fn register(&self, session_id: &str, tx: tokio::sync::mpsc::UnboundedSender<SteerMessage>)` | 内核开轮时登记；同键覆盖即上一轮的接收入端已 drop |
| `SteerRegistry::request` | 同上 | 新增 | `pub fn request(&self, session_id: &str, msg: SteerMessage) -> Result<(), String>` | 投递；miss 与 `send` 失败同归 `Err(「会话已收口，补充说明不可送达」)`（DD1） |
| `SteerRegistry::remove` | 同上 | 新增 | `pub fn remove(&self, session_id: &str)` | 轮收敛除名；miss 幂等 |
| `AskRegistry::is_pending` | `crates/core/agent/src/ask.rs` | 新增 | `pub fn is_pending(&self, session_id: &str) -> bool` | 只读判据（`slots.contains_key`，O(1)，即刻释放）；`register` / `answer` / `remove` 逐字不动 |
| `SessionKernel::new` | `crates/core/agent/src/kernel.rs` | 修改（加第 5 参） | `pub fn new(sink: Arc<dyn SessionSink>, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, ask_registry: Arc<AskRegistry>, steer_registry: Arc<SteerRegistry>) -> Self` | 两张会话级注册表由组合根注入，MUST NOT 内部自铸 |
| `loop::run` | `crates/infra/agent/src/sdk/loop.rs` | 修改（加第 4 位置参数） | `pub(crate) async fn run(model: &DynModel<Completion>, turn: &LoopTurn, mut history: Vec<Message>, mut steers: mpsc::UnboundedReceiver<agent::SteerMessage>, sender: mpsc::Sender<AgentEventKind>, handle: RunHandle) -> Vec<Message>` | 请求边界 drain（DD2 / DD3）；`LoopTurn` 零改动 |
| `compose_turn` | `crates/infra/agent/src/compose.rs` | 修改（加实参） | `pub fn compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, ask_registry: Arc<agent::AskRegistry>, steer_registry: Arc<agent::SteerRegistry>, store: Arc<Store>, agent: Option<i64>, workspace_root: &Path, ask_port: Option<Arc<dyn orchestration::port::AskPort>>) -> Result<ComposedTurn, String>` | 新实参紧跟 `ask_registry`（DD10）；只透传内核 |
| `agent_steer` | `src-tauri/src/commands/exec/mod.rs` | 新增（IPC 命令） | `pub async fn agent_steer(steer: State<'_, Arc<SteerRegistry>>, ask: State<'_, Arc<AskRegistry>>, root: String, session_id: String, text: String) -> Result<(), String>` | 薄包装四段（blank 三参 → ask 判据 → 注册表投递 → ack 等待，DD9 / DD12）；零新开会话 / 轮、零状态机、零广播 |
| `useAgentSession` | `packages/desktop/src/hooks/use-agent-session.ts` | 修改（返回面加位） | `export function useAgentSession(params: UseAgentSessionParams): UseAgentSessionState` | 入参面零改动；返回面增 `steer` / `steering` |
| `UseAgentSessionState.steer` | 同上 | 新增（接口成员） | `steer: (text: string) => void` | `agent_steer(root, sessionId, text)` 薄包装；`root` / `sessionId` 为 null 零 invoke |
| `UseAgentSessionState.steering` | 同上 | 新增（接口成员） | `steering: boolean` | 「已提交、等 ack 回流」的 pending 态（DD8） |
| `AgentSessionActions.steer` | `packages/desktop/src/components/agent/agent-session-view.tsx` | 新增（接口成员） | `steer: (text: string) => void` | composer 插槽动作面（additive） |
| `AgentSessionActions.steering` | 同上 | 新增（接口成员） | `steering: boolean` | 供插槽呈「排队中…」 |
| `AgentInputProps.hidePermissionMode` | `packages/desktop/src/components/agent/agent-input.tsx` | 新增（接口成员） | `hidePermissionMode?: boolean` | 缺省 `false`；`true` 时不呈 permission-mode 档位行（`onSend` 的 `permissionMode` 位保留但下游 MUST NOT 消费） |
| `AgentInputProps.pending` | 同上 | 新增（接口成员） | `pending?: boolean` | 缺省 `false`；`true` 时发送禁用 + 呈「排队中…」可见文案，输入与停止面不受影响（DD7） |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `SteerMessage` | `crates/core/agent/src/steer.rs` | 新增 | `{ pub text: String, pub ack: tokio::sync::oneshot::Sender<()> }`（`#[derive(Debug)]`）；每条一个 ack 是「不合并」的类型载体 |
| `SteerRegistry` | 同上 | 新增 | `{ slots: Mutex<HashMap<String, mpsc::UnboundedSender<SteerMessage>>> }`，`#[derive(Default)]`（壳层 `manage` 需要） |
| `AgentSession` | `crates/core/agent/src/runner.rs` | 修改 | 加 `pub steers: mpsc::UnboundedSender<SteerMessage>`（四件套） |
| `SessionKernel` / `RunningTurn` | `crates/core/agent/src/kernel.rs` | 修改 | 各加私有 `steer_registry: Arc<SteerRegistry>`（无派生面变化） |
| `UseAgentSessionState` | `packages/desktop/src/hooks/use-agent-session.ts` | 修改 | 加 `steer` / `steering` 两成员（DD8） |
| `AgentSessionActions` | `packages/desktop/src/components/agent/agent-session-view.tsx` | 修改 | 加 `steer` / `steering`（additive，既有四字段零改动） |
| `AgentInputProps` | `packages/desktop/src/components/agent/agent-input.tsx` | 修改 | 加 `hidePermissionMode?` / `pending?`（皆缺省 false） |
| `commands.agentSteer`（生成包装） | `packages/desktop/src/types/generated/bindings.ts` | 新增（生成物） | `(root: string, sessionId: string, text: string) => Promise<null>` |

<!-- 配置：本变更不新增 / 不修改任何配置键——`.dev-team/config.json`、`config.schema.json`、
     引擎 provider 配置与 `AgentInput` 的 permission-mode 选项集均零改动（隐藏档位行只是
     呈现面开关，不产生新配置项）。 -->

---

## 数据模型

### 注入注册表（内存态，`SteerRegistry`）

| 项 | 形态 |
|----|------|
| 键 | core 铸会话 id（`new_session_id()` 产出，进程内唯一） |
| 值 | `mpsc::UnboundedSender<SteerMessage>`（无容量失败面——D8） |
| 通道容量 | 无界（`unbounded`）：`request` 同步 `send`，不存在「队列满」的第四种 Err 成因 |
| 登记 | 内核 `begin_turn`（紧随 `streams.begin`），sender 来自本轮 `AgentSession.steers` |
| 消费 | 引擎 `loop::run` 在每个请求边界 `try_recv` 循环取走（全部在途项一次取净） |
| 除名 | ① `converge`（与 `StopRegistry` / `AskRegistry` 同址）；② 接收端随 `loop::run` 返回 / future drop 自动 drop（通道内残留项与其 ack 随之 drop → 命令侧 `Err`） |
| 生命周期 | 进程内、一轮一存（应用重启即清空，与三张同族表一致） |

### 补充说明在会话史上的形态

| 项 | 形态 |
|----|------|
| 事件 | `AgentEventKind::Message { role: User, blocks: [Text { text, citations: None }], parent_tool_use_id: None }` |
| 盖戳 / 落库 | 走既有单点：`observations → AgentEvent::stamp(Some(turn_id), seq) → append_sealed → streams.publish → on_output`（**零新增落库路径**） |
| seq | 会话级单调空间（`begin_turn` 的 `seq_base` 起算），与同轮其它密封事件同源 |
| 请求史 | 一条**新的** `Message::user(text)` 追加在 `history` 尾部（此时尾部正是 `Message::tool_results`）——MUST NOT 并入该条：rig 0.43.0 不合并相邻同角色消息，openai 侧展开会把 user 文本排到 tool 之前（provider 400） |
| 关系与持久化 | 无新表、无新列、无外键、无迁移；store schema 零变更（见下） |

### 前端消息 id 空间（两路同源）

| 消息 | id 规则 | 归属 | 出现条件 |
|------|---------|------|----------|
| 提问 | `turn-<turnId>-user`（旧转录兜 `evt-<seq>`） | 该轮**首条** `role = user` 密封事件 | 至少一条 user 事件 |
| 补充说明 | `evt-<seq>` | 该轮**其余** `role = user` 密封事件（按 seq 升序） | 轮中段注入过 steer |
| assistant | `turn-<turnId>`（旧转录兜 `evt-<首事件 seq>`） | 该轮非 user 密封事件聚合 | 恒一条 |

「一个 id 只对应一条消息」：补充说明 MUST NOT 复用 `turn-<turnId>-user`（否则两条消息撞 id → 收口整表替换时重挂）。

### 关系与持久化

| 项 | 现状 | 本变更 |
|----|------|--------|
| store 模型（会话 / 轮 / 转录） | 既有三表 | **零改动**（`crates/infra/store/**` 零 diff） |
| 转录写入 | `SessionSink::append_sealed` | 复用同一入口（补充说明是普通 user 密封事件） |
| 轮统计 | `TurnStats`（`num_turns` / `duration_ms` / `usage`） | 零改动——steer 不新开轮、不产生第二个 `RunStarted` / `TurnDone` |
| 编排状态 | `ChangeFlowControl` / walker / run 状态机 | 零改动（补充说明不产生编排事件、不改 run 状态、不改 step 落账） |
| 停等态（ask） | `AskRegistry`（进程内） | 零改动（只加只读查询） |

---

## 路由 / API 设计

**IPC 命令面**（Tauri `invoke`；`Result<T, String>` 错误模板沿用既有约定）：

| method | path（命令） | 说明 | 输入 | 输出 | auth |
|--------|--------------|------|------|------|------|
| IPC | `agent_steer` | 运行中会话的补充说明注入（**新增**）：薄包装四段（blank 三参 → `AskRegistry::is_pending` 判据 → `SteerRegistry::request` → `ack.await`），零新开会话 / 轮、零状态机、零广播 | `root: string` / `sessionId: string` / `text: string` | `null`（`Ok`，此时文本必已入该轮史）或 `Err(String)` | 无（本机桌面应用；root 即寻址） |

**错误语义（六条文案各自可辨，MUST NOT 归一）**：

```
blank root        → Err("非法 root: 不得为空白")
blank session_id  → Err("非法 session_id: 不得为空白")
blank text        → Err("非法 text: 不得为空白")
ask 阻塞中         → Err("等待应答中，补充说明不可送达")          ← 次序第一，不可与下条互换
无在跑轮 / 已收口   → Err("会话已收口，补充说明不可送达")          ← miss 与 send 失败同臂（DD1）
ack 前结束         → Err("未送达（会话在补充说明生效前结束）")     ← drop-Err，零超时（DD2 / spec）
命中              → Ok(())
```

**内部调用链（决定送达语义）**：

```
抽屉步面板 composer ── actions.steer(text) ──► hook useSteer ──► invoke agent_steer(root, sessionId, text)
                                                                     │
                        ① ask.is_pending(sid) ──true──► Err（等待应答中）
                        ② steer.request(sid, {text, ack}) ──miss/drop──► Err（会话已收口）
                        ③ ack.await ──RecvError──► Err（生效前结束）

engine loop（同一 turn 内）
  请求边界 drain: 密封 Message{role:User} → history.push(Message::user) → ack.send(())
  → 下一个请求带上该消息（模型看到补充说明）
```

<!-- 无 HTTP 路由：本应用无 HTTP 服务面。 -->

---

## 依赖

- **运行时依赖**：**零新增**。`core/agent` 的 `steer.rs` 只用 `std::collections` / `std::sync` 与既有 `tokio::sync::{mpsc, oneshot}`（`crates/core/agent/Cargo.toml` 已有 `tokio = { workspace = true }`，workspace 层含 `sync`）；`infra/agent` 只用既有 `agent` / `tokio` 导入面（`loop.rs` 已有 `tokio::sync::{mpsc, oneshot}`）；壳层与前端零新依赖。全部 `Cargo.toml` 与 `package.json` 的依赖段**零 diff**。
- **依赖方向零变化**：`infra/agent → agent`（既有）／`src-tauri → agent`（既有）。`core/agent` 仍零 workspace 内依赖。**零新依赖边**（架构模型 `dependsOn` 零变化）。
- **构建 / 测试依赖**：**零新增**。drain 的行为面用既有 `rig` `test-utils` 流缝脚本化（`loop_test` 既有装置）；注册表两态用例为纯内存对拍，零进程边界。
- **无 feature 开关变化**：`rig` facade 锁版与 features 面逐字不动。

---

## 验收标准落点

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 同轮送达 | `open_session` 铸通道 + `AgentSession.steers` + 内核 `begin_turn` 登记 + `loop::run` 的 drain（密封事件 → `Message::user` 追加在 `tool_results` 之后 → ack）+ 既有点（`stamp` / `append_sealed` / `publish`）零改动 + 同一 `session_id`（零新会话：drain 不开 `SessionRef::Continue`） |
| AC-2 不合并 | `drain_steers` 的 `while let Ok(...) = try_recv()` 循环（一次边界取净）+ 每条独立 `ack.send(())` + `SteerMessage` 一条一 ack（DD2） |
| AC-3 收口即 Err | `SteerRegistry::request` 的 miss / send 失败同臂 `Err`（DD1）+ `converge` 内 `steer_registry.remove` + 接收端随 `loop::run` 返回 drop（双保险）；命令体零排队、零 Continue |
| AC-4 ask 优先判据 | `agent_steer` 的 ① 段（`ask.is_pending` 先于 `steer.request`，次序不可换）+ `AskRegistry::is_pending`（`contains_key`，不改 take 语义）+ 文案「等待应答中…」独立 |
| AC-5 ack 竞态诚实 | `agent_steer` 的 ③ 段 `delivered.await.map_err(「未送达（会话在补充说明生效前结束）」)`；零超时机制、零额外清理表项（drop-Err 自动成立：接收端 drop → 缓冲消息 drop → ack sender drop） |
| AC-6 停止不改写转录 | drain 在停止快检**之后**（停止即 `return history`，残留项零事件）+ `converge` 除名（其后 `request` → `Err`）+ `AgentSession.questions` / `RunHandle` 零改动（停止仍经既有 `agent_stop` → `request_stop`） |
| AC-7 一轮一命不破 | `session_pump` 仍只 `questions.recv()` 一次、`loop::run` 仍单次调用、无新 select 臂；`LoopTurn` / `PumpPayload` / `EngineFacade` 三处**零 diff**（`git diff` 可验） |
| AC-8 命令面纪律 | `agent_steer` 四段薄包装（blank 三参各携字段名 → ask 判据 → 投递 → ack）+ `all_commands!` 登记（紧随 `agent_answer`）+ 生成物由 `bindings:export` 再生成（零手改）；`COMMAND_NAMES` / `COMMAND_WRAPPERS` 50 → 51 的条目与断言属测试相位 |
| AC-9 前端可见（实时路） | `applyUserMessage` 的分化（DD4：判据 = 提问气泡是否已在场）+ `evt-<seq>` 追加（不被「同轮已有同名 id」丢弃）+ 提问气泡恒一条 |
| AC-10 前端可见（折叠路） | `entryToMessages` 的 user 分支（首条 → `questionMessage`；其余 → `steerMessage`，id `evt-<seq>`）+ `steerMessage` 的 parts 复用 `plansToParts(planBlocks(...))`（DD5）+ 插入锚点（DD6）+ `assistantMessage` 的 user 跳过零改动 |
| AC-11 动作面与判据不预判 | `useSteer`（null 守卫零 invoke、`steering` 置位 / 两收尾复位、`Err` 进 `actionError` 并合入 `error` 四源链，DD8）+ `AgentSessionActions.steer` / `steering` 组装 + 抽屉 composer 插槽（`onSend → actions.steer`，前端零可用性判据） |
| AC-12 composer 变体加位 | `AgentInputProps` 两可选位 + `canSend` 合成式（`pending` 时禁发）+ `ComposerToolbar` 的 `hidePermissionMode`（不渲档位）+ `pending` 的可见文案行 `data-testid="<idPrefix>-pending"`（DD7）；默认变体（两位缺省）零变化 |

### 交付门（非本变更能力判据）

全仓既有门的**登记面**，本设计与 tasks 均不派生新判据。源码级静态面（`cargo clippy --all-targets`、前端 `vp check` / `knip`、`grep` 零命中、结构复核、`git diff` 边界比对、版本行相对 `HEAD` +1 patch）可在 tasks 的自查项中自判；`pnpm -C packages/desktop run check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**由桌面端 static-check / test-execution 相位承接，在 tasks 中只以「交接 …」条目登记。

---

## 待决问题

1. **其余会话面的接线时机**：`explore` / `archi` / `change-chat` / 调试页的后端通道同样可用，但本期只接抽屉步面板（D1 / spec「MVP 边界」第 6 条）。接线成本 = 各面板 composer 一行 `onSend → actions.steer`（变体能力已收在 `AgentInput` 内）；是否放开由各面板的「run 期禁发」判据自己决定，本设计不改其判据。
2. **`steering` 若将来要支持并发多条**：本期 boolean 成立的唯一前提是 `pending` 禁发（DD8）。若某消费点要在 pending 期继续发，需把 `steering` 换成「在途条数」或每条一个 ack 状态，并同时给 UI 一个「第 n 条在等」的形态——届时是消费点驱动的增量，不在本期预埋。
3. **drain 是否加引擎日志**（`append_engine_log` 记「补充说明入史 text_len / turn_index」）：本期**不加**——密封事件本身就是最权威的留痕（转录可见），而日志在长轮里每个边界都可能写一行；若后续出现「补充说明没进史」的排查实例，再加一条（`loop.rs` 既有格式可直接沿用）。
4. **上下文防线裁掉刚补的补充说明**（proposal R5）：drain 位于 `defend` 之前、尾部追加，prune 的「保首条 + 轮对裁剪」使尾部最新内容优先级最高；理论仍可能在水位极端时被裁成占位。本期接受为已知代价，MUST NOT 引入「steer 免裁」特权（那会破坏防线单点）；转录里仍有原文可核对。
5. **折叠路是否改用「只收 text 块」的直映射**（DD5）：本期复用 `plansToParts(planBlocks(...))`。若后续补充说明允许携带非 text 块（本变更不允许，spec 钉死 text 单一来源），复用式自动覆盖；反之若要把补充说明的呈现与提问彻底分开，需在适配层引入第二个语义点——不建议。
