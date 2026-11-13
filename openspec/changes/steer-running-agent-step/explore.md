# 变更运行中 Agent Step 的「补充说明」（steer）探索

workflow_type: requirement

## 诉求

变更流程（change flow run）跑到某个 WorkerAgent step 时，打开该 step 的会话窗口，希望**能发消息补充说明**（而不是只能干看 + 停止）。

一句话本质：**对一条正在跑的 agent turn 注入一条 user 消息**，让它在接下来的模型请求里看到这句话。

## 裁决总表（实施前先看这张）

steer 的需求已被反复裁决定型。**下表是全部已定项**，实现时按此为准；下方各节是推导过程与证据。

| # | 议题 | 裁决 | 轮次 |
| --- | --- | --- | --- |
| 1 | UI 落点 | **step 会话窗口**（抽屉 `SessionTranscriptPanel`）；左栏「通用 Agent」面板判据矩阵不动 | 一 |
| 2 | 送达语义 | **收口即 Err**，不排队、不续跑 | 一 |
| 3 | 通道归属 | **通用能力**，键 = core session id（与 `AskRegistry` / `StopRegistry` 同型）；drawer 只是首个消费点 | 一 |
| 4 | 前端判据 | **不加**，靠后端 Err 说话（理由：库态 `running` 在重启后不愈合） | 一 |
| 5 | 落地次序 | 先 `finish_step` 与 ask，再做 steer（**前两者均已归档**） | 一 |
| 6 | ask 阻塞期行为 | **组合 ①**：一律 Err + composer 不加禁用态（三种 Err 成因文案各自独立） | 二 |
| 7 | 多条 steer | **不合并**——每条独立 user 消息 + 独立密封事件 + 独立 ack | 三 |
| 8 | 留痕 | **不加** `SystemNotice{steer}`——既有 user 消息即留痕 | 三 |

**裁决已全部完成**，无未决项。下方各节是推导过程与证据。

## 现状：谁在跑、谁能发

```
                        ┌──────────── 前端（React）─────────────┐
change-detail-view ─┬─► ChangeChatPanel（左栏「通用 Agent」）      ← source=change, source_ref=<changeId>
                    │      └ useAgentSession.sendMessage → agent_start(New/Continue)
                    │      └ runActive=true 时整面板 disabled（理由「流程运行中」）
                    │
                    └─► DetailDrawer（点流程图节点打开）
                           └ DrawerContent
                              ├ WorkerAgent 节点（role!==null）→ SessionTranscriptPanel
                              │     └ AgentSessionView（自持形态，无 composer 插槽）
                              │        ← 只读：不呈发送、不呈停止（spec 明文要求）
                              └ outputRef!==null → StdoutPanel（只读）
```

后端跑 run 的路径（每步一个 turn）：

```
change_flow_start
  └ compose_turn(store=workspace root 库) ─► ComposedTurn{ kernel, runner, query }
  └ KernelWorkerPort::new(composed, sink)  ← WorkerAgentPort
  └ walk_run(...)
        └ run_worker(role, prompt, continue_session)
              └ composed.begin(SessionRef::New | Continue{id}, prompt, ctx, provenance)
                    ├ Continue 校验（存在性 / 引擎匹配 / prior_handle）
                    └ SessionKernel::begin_turn
                          ├ runner.open_session   ← sdk: 建 question/observation 两通道 + spawn session_pump
                          ├ sink.create_session / next_seq / begin_turn
                          ├ StopRegistry.register(session_id, handle)
                          └ SessionStreamRegistry.begin(session_id)
              └ running.drive(on_output)
                    └ 观测循环：stamp(seq) → sink.append_sealed → streams.publish → on_output
                    └ EOF → converge（finish_turn / streams.finish / registry.remove）
              └ 读 outcome（status / transcript / final_message）→ StepSettle + token 统计
```

关键既有约束：

| 约束 | 位置 | 含义 |
| --- | --- | --- |
| **一轮一命** | `infra/agent/src/sdk/runner.rs:session_pump` | 泵只 `recv` 一次 `TurnQuestion`，跑完 `loop::run` 即返回；MUST NOT 回到等待下一轮 |
| **同轮多请求** | `infra/agent/src/sdk/loop.rs:194` | 一个 turn 内部是「请求 → 工具调用 → 工具结果入史 → 再请求」的循环（`turn_index` 递增），**直到模型不再要工具** |
| **单活动轮** | `cmd exec agent_start` + `useAgentSession` | 每轮 `compose_turn → begin → open_session` 新开引擎会话；会话史唯一源 = store 全史转录（`resume::rebuild`） |
| **seq 单调** | `kernel.rs` stamp 单点 + `store.next_session_seq` | 会话级 seq 空间，轮开时询定基点；重复 seq 会撞主键 |
| **抽屉只读** | `session-transcript-panel.tsx` + `desktop-change-flow-view` spec | 明确「不接 composer、不呈发送 / 停止」 |
| **run 期左栏禁发** | `use-change-chat.ts` 判据矩阵 | `activeRun !== null` → disabled，理由「流程运行中」 |

所以现在的物理事实是：**没有任何通道能把一句话送进一个已经开跑的 turn**。

> **为什么 ask 不算「送进 turn」**：ask 已实现（`AskRegistry` + `AskUserQuestion`），
> 它确实是「turn 内等待外部输入」的通道——但方向相反且时机受限：
> - ask 是**模型发起的阻塞**，用户只能回答它问的那个问题，不能主动补一句；
> - ask 只在模型调用它时才存在，其余时间窗口里没有任何入口；
> - ask 阻塞期间 loop 停在工具执行分支，**恰恰是 steer 最难送达的窗口**（见「ask 阻塞期间能不能 steer」）。
>
> 一句话：ask 解决「模型问我答」，steer 解决「我要说，模型得听」。两者不可互相替代。

## 三条候选路径

### A. 同轮内插话（steer / 真正的"补充说明"）

在 pump ↔ loop 之间加一条 steer 通道，loop 在每个**请求边界**把待发的补充说明作为 user 消息塞进 history，然后照常发请求。

```
SteerRegistry (新，仿 StopRegistry)
   键 = core session id → mpsc::Sender<SteerMessage>
        │
agent_steer(root, session_id, text) ──► registry.send
        │
        ▼
  session_pump ──► loop::run(steers: Receiver<SteerMessage>)
                    loop {                        ▲
                      stop 快检                    │ drain:
                      history = defend(...)  ──────┘ 把 steer 文本 push 成 Message::User
                      request = build(history)        并 send(AgentEventKind::Message{role:User})
                      ... 流式 / 工具 / 工具结果入史
                    }
```

- **优点**：step 不感知、walker 零改动——补充说明发生在**同一个 turn、同一个 node、同一个 instance**，token / 耗时 / 会话 id 全部自然归位；steer 的 user 消息走既有 `observations → stamp → append_sealed → streams.publish` 单点，store 与实时流零新增。
- **代价**：`AgentSession` 加一条通道（5 处构造点 + 测试夹具）；新增 `SteerRegistry` + `agent_steer` 命令 + 前端 composer；`loop` 要加 drain 点与"收口前再 drain"逻辑。
- **语义边界**：只在「本轮还有后续请求」时真正生效（模型不再要工具 → turn 立即收口）。收口后送达要显式 Err。

### B. 轮后加问（排队到本轮结束，再 Continue 一轮）

steer 文本排队在 registry，walker 在本轮 outcome 之后检查队列，非空则 `continue_session = Some(sid)` 再跑一轮。

- **优点**：不需要改 pump / loop，只用既有 Continue（store 全史重建）。
- **代价**：walker 的 `run_worker` 要进循环（每步变成「跑到队列空」）；用户的话只在**上一轮完全收口后**才被读；evaluator / decision 步的「最终消息」协议会被第二轮的输出顶掉（`final_assistant_text` 取末条 assistant），verdict 解析会被污染；步节点的语义（一次 step = 一次会话turn）开始漂。

### C. 复用 `agent_start` 带 current sessionId（纯前端"插话"）

不新增后端面，直接 `sendMessage({sessionId: 正在跑的那个})`。

- **结果**：`compose_turn → begin → open_session` 会**再开一个引擎会话**，同一 session 上出现两条并发 drive；两个 drive 各自 `seq_base` 取自 store（都是同一个 max+1）→ **seq 主键撞车**，StopRegistry 同键覆盖（停止错杀），transcript 交错。**这条路必然坏事**，列出来是为了把它钉死在方案外。

## 推荐 A，改动面清点（已按 ask 落地形态校正）

ask 落地给了一个**可直接照抄的样板**——`AskRegistry`（`core/agent/src/ask.rs`）已经证明了
「会话级阻塞通道住 core/agent、键 = core session id、与 StopRegistry 同址登记/除名」这套形态
在本仓成立。steer 是它的反向版本：

```
AskRegistry     注册表持 Sender（registry → 等待者）  等待者阻塞等答案   ← 已实现
SteerRegistry   注册表持 Sender（registry → loop）    注入者不阻塞      ← 本文提的
                 两者键空间相同、生命周期同址（begin_turn / converge）
```

```
后端（core）
  crates/core/agent/src/steer.rs     SteerRegistry（新建，与 ask.rs 同目录同型）
                                       slots: Mutex<HashMap<String, mpsc::Sender<SteerMessage>>>
                                       request(session_id, msg) -> Result<(), String>  ← miss 即 Err
                                       remove(session_id)
  crates/core/agent/src/runner.rs    AgentSession 增 steer 通道（构造点：sdk runner + 测试夹具）
  crates/core/agent/src/kernel.rs    begin_turn 登记 / converge 除名（与 StopRegistry、
                                       AskRegistry 同一处）——注意 ask 的 register 发生在
                                       工具执行体里，steer 的登记必须在 begin_turn，因为是长期通道

后端（infra）
  crates/infra/agent/src/sdk/runner.rs   建 steer channel，receiver 传进 session_pump → loop
  crates/infra/agent/src/sdk/loop.rs     LoopTurn 增 steer rx；请求边界 drain（见下「drain 点」）

壳层
  src/main.rs                        app.manage(Arc::new(SteerRegistry::default()))
  src/commands/exec/mod.rs           agent_steer(root, session_id, text) -> Result<(), String>
                                       registry.request? → ack.await（drop-Err）
  src/commands/mod.rs + bindings     命令注册 + bindings:export（bindings/mod_test.rs 的
                                       COMMAND_NAMES 同步）

前端
  src/lib/ipc.ts                     自动（生成物驱动）
  src/views/changes/flow/session-transcript-panel.tsx   composer 插槽（只读消费点的破例，需改 spec）
  src/components/agent/agent-input.tsx                  「无 permission-mode 档位」变体 + pending 态
  src/hooks/use-agent-session.ts（或面板侧）            steer send = ipc.agentSteer；
                                                       stop 复用既有 agentStop
```

### drain 点（已对照 loop.rs 实际结构）

```
loop {
  if handle.stop_requested() { return }        ← 轮间快速路径
  drain_steer(&mut history, &sender, &mut rx)  ← ★ drain 插在这里
                                                  history 尾部此刻 = Message::tool_results
  history = defend(...)
  请求 → 流式 → 收口
     ├─ 无 tool_calls → finish_success → return
     └─ 有 tool_calls → 逐个执行（ask 可能在此阻塞）→ push tool_results → 回到 loop 顶
}
```

这个位置有双重正确性：语义上「看到工具输出后补一句」，结构上新 push 的 `Message::user`
紧跟在 `tool_results` 之后——正是 rig 要求的分隔形态（见「必须避开的实现陷阱」）。

**顺带的好消息**：抽屉里的 stop 已经能杀伤——`agent_stop` 直接对 worker 会话 id 请求
终止，walker 收到 `AgentRunStatus::Stopped` → `Terminal::stopped("会话被终止")`。
所以 composer 的停止半边是免费的（与 ask 落地前的结论一致）。

## 时序：steer 落地的四种时机

```
 t →  请求1 ══流式══> 工具调用 → 执行 → 工具结果入史 → 请求2 ══流式══> 收口
                  ▲          ▲            ▲                        ▲
        (1) 流式中到达   (4) ask 阻塞中   (2) 请求边界到达     (3) 收口瞬间到达
             │          （新，已落地）        │                        │
      缓存在 channel   送不进去，立即 Err   drain 后进下个请求    本轮已结束 → 丢 → Err
```

(3) 与 (4) 都是「送不进去」，但成因不同：
- (3) 收口竞态：窗口毫秒级，无法靠加 drain 归零，靠 drop-Err 诚实报出；
- (4) **ask 阻塞**：窗口是人答题的时间（几十秒到几分钟），是两者里更大的失败面。详见「ask 阻塞期间能不能 steer」。

两者共用同一条纪律：**不可送达就立即 Err，不排队、不静默**。

## 已裁决（2026-11-06）

用户三问均已拍定：

1. **UI 落点 = step 窗口**（抽屉里的 `SessionTranscriptPanel`），不是左栏「通用 Agent」面板。左栏的 run 期禁发判据矩阵不动。
2. **送达失败 = 收口返回 Err**（不做排队续跑）。配合 drop-Err 语义实现，见「收口 Err 的实现方式」。
3. **steer 做成通用 agent 能力**，键 = core session id，不绑 change 来源；drawer 只是首个消费点。
4. **驱动条件不加判据**：composer 由后端 `SteerRegistry` miss 说话（重启后残留的 `running` 轮行不愈合，见下），前端不预判。
5. **次序**：第 3 点（收口工具）已拆到独立文档 `agent-finalize-tool-for-flow-steps.md`，先落地。

> **现状更新**：该工具已实现并归档，实现名为 **`finish_step`**（非探索期暂用的 `finalize`）；
> ask 工具同样已实现并归档。**steer 是三者中唯一未实现的**。

### 收口 Err 的实现方式（drop-Err，零超时）

```rust
// core/agent：仿 StopRegistry
pub struct SteerMessage { pub text: String, pub ack: oneshot::Sender<()> }

agent_steer(root, session_id, text) -> Result<(), String>
    registry.request(steer)?          // 无 live turn → 立即 Err
    ack.await                         // 泵收口 → Receiver 与 buffered 消息一同 drop
                                      //   → ack Sender drop → await Err
                                      //   → 命令返 Err「未送达（会话已收口）」
```

**不需要超时、不需要额外清理**：`SteerRegistry` 里的 `Sender` 在 converge 处 remove（与 `StopRegistry.remove` 同一处），miss 判据因此成立；泵返回时 `Receiver` 被 drop，缓冲里的 steer 随之 drop，`ack` 自然失效。用户选的 ② 就是这样自动成立的。

> **与已落地的 `AskRegistry` 对比——两者语义刻意不同**：
> `AskRegistry::answer()` 是**受理语义**（`let _ = sender.send(text); Ok(())`——对端已 drop 仍返 Ok，
> 幂等由「槽已移除」承载）。steer 需要的是**送达语义**（ack 确认），所以不能照抄 `answer()` 的
> 返回值处理，必须额外带 `ack` oneshot。这是两者唯一不能互相复制的点。

**代价（已知并接受）**：`ack.await` 的最长等待 = 当前响应剩余时长 + 工具执行时长（drain 只在请求边界发生）。所以 `AgentInput` 需要一个 `pending` 中间态——它现在只有 `running` / `disabled` 两个正交标志，没有第三种。

### 判据不能靠库态（已核实）

`calibrate_interrupted_runs` 只标定 `RunRecord`（run 运行史），**不动 `AgentRunRecord`（轮行）**。桌面端崩溃后那条轮行永远是 `running`：

```
可用性真相分层：
  ① 前端能判的：node.live && node.status === 'running'   （run 活面，进程内真相）
  ② 后端权威：  SteerRegistry miss → Err                 （覆盖崩溃残留）
  ③ 不能用的：  库态 AgentRunRecord.status === 'running'  （重启后不愈合）
```

裁决 4 意味着只走 ②——前端恒显示 composer，靠 Err 说话。

### 顺带：停止是免费的

抽屉里现在没有停止按钮，但 `agent_stop(root, worker_session_id)` 本来就能杀伤：walker 收到 `AgentRunStatus::Stopped` → `Terminal::stopped("会话被终止")`。composer 插进抽屉后，「停止」半边零后端改动。

### 一个必须避开的实现陷阱（已核实 rig 0.43.0）

rig **不合并相邻同角色消息**：anthropic 侧一条 rig `Message::User` → 一条 wire `role:user`；openai 侧 `user_content_to_messages` 只在**单条 rig Message 内部**把相邻非 tool 内容聚成一条。所以 drain 必须 push 一条**新**的 `Message::user(text)`：

```
正确：  assistant{tool_calls} → user{tool_results} → user{steer_text}     ✓ 两侧合法
错误：  assistant{tool_calls} → user{tool_results + steer_text}
                                  └─ openai 展开成 user(text) 先于 tool  └─ 400
```

drain 点（`loop.rs` 顶部请求边界）位置天然正确——此时 history 尾部正是 `Message::tool_results`，语义也正好是「看到工具输出后补一句」。

## ask 阻塞期间能不能 steer（核心边界，已按落地代码核实）

> **前提更新**：ask 已实现并归档（`openspec/changes/archive/2026-11-12-ask-tool-in-flow`），
> 不再是推演对象。以下全部基于实际代码。

### 落地形态（核实值，不是设计意图）

```
core/agent/src/ask.rs          AskRegistry（键 = core session id）
                                 register(session_id, tx) -> bool   ← 返回值是诊断值，非失败面
                                 answer(session_id, text) -> Result<(), String>   ← take 语义
                                 remove(session_id)                 ← 轮收敛兜底

core/agent/src/runner.rs       AskMode::{Generic, Flow}
                               SessionInjections.ask: Option<AskMode>
                                 None = 显式禁用（只建了位，首期未接线）
                                 Some(Generic) = 通用聊天
                                 Some(Flow)    = 流程（额外广播编排状态）

core/orchestration/src/port.rs AskPort { ask_started(AskPayload), ask_resolved() }
                               AskPayload { question, options, tool_call_id, session_id }

infra/agent/src/sdk/loop.rs    execute_ask(ctx, mode, args, tool_call_id, handle)
                                 select! { rx => 答案, handle.wait_requested() => 终止 }
                                 Flow 档：ask_started → 阻塞 → 两臂返回后都 ask_resolved

infra/agent/src/sdk/mount.rs   ToolTier::Protocol（任何 permission_mode 都放行）
                               工具名 = AskUserQuestion（沿用历史词汇）
                               ask 位与 finish 位互不依赖（两条独立 if）
```

### 结论：会被阻塞，且阻塞点非常明确

```
loop {
  ┌─ 请求边界 ← steer 的 drain 点在这里
  │
  ├─ 发请求 → 流式 → 模型调 AskUserQuestion
  │
  ├─ 工具执行 for 循环
  │    └─ execute_ask 内 select! 阻塞     ← ★ steer 在这个窗口里到达
  │         （几十秒到几分钟，取决于人）
  │
  ├─ 应答到达 → tool_result 回灌 → history.push(tool_results)
  └─ 回到请求边界
}
```

`execute_ask` 阻塞期间 loop 停在工具执行分支，够不到顶部 drain。**这一点已由代码确认，不是推演。**

### 一个落地后才看清的实现缝隙

`AskRegistry` 目前只有 `register` / `answer` / `remove` —— **没有「是否正在等」的查询方法**。而 steer 的语义 A2（阻塞期立即 Err）恰恰需要这个判据：

```
需要新增（或在 stage 侧自带）：
  AskRegistry::is_pending(session_id) -> bool
    = slots.contains_key(session_id)
```

好在两者的键空间完全一致（都是 core session id），所以这是一次 O(1) 判断，不需要第二张表。

**也因此有了一个更优的实现形态**：steer 的「能否送达」判据可以完全复用 ask 的槽状态——

```
steer 送达判据（三态合一，无需新增状态机）：
  ask.is_pending(sid)     → Err「等待应答中，补充说明不可送达」
  steer 注册表 miss        → Err「会话已收口」
  否则                    → 正常排队到下一个请求边界
```

这比我在上一轮设想的「loop 进出 ask 时对 steer 注册表置位/清位」更简单：**ask 槽本身就是「当前阻塞中」的真相**，不需要第二份状态同步。

### 一个值得注意的实现机会

`execute_ask` 的 `select!` 已经有两条臂（应答 / 终止）。**加第三条臂接收 steer 是物理可行的**：

```rust
let outcome = tokio::select! {
    answer = rx => ...,
    _ = handle.wait_requested() => ...,
    Some(steer) = ctx.steer.recv() => {  // 新臂
        // 不能直接进 history（此刻 history 还没 push tool_results）
        // 只能缓冲，等工具结果入史后再落
        buffered.push(steer);
        continue_asking();   // 继续等 ask 应答
    }
};
```

这就是语义 B（排队）的最小实现点。**已裁决选 ①（A2 + 无禁用态）**，此臂本期不接——详见下节。

## 已裁决（第二轮 · ask 阻塞期行为）：一律 Err，composer 不加禁用态

用户选定组合 **①**：ask 阻塞期间用户仍可打字、仍可点发送，收到 Err 后自知。

### 为什么这条与「不加前端判据」是同一条纪律

```
已有的裁决：收口即 Err，靠 SteerRegistry miss 说话，前端不预判
本轮裁决：  ask 阻塞即 Err，靠 AskRegistry 槽状态说话，前端不预判

→ 两条合起来才完整：steer 的两种「送不进去」都走同一条后端 Err 路径
   前端恒显示 composer、恒不判据，Err 文案区分成因
```

如果 ask 期间单独加禁用态，就会出现「收口不判、ask 判」的不对称——用户无法预期 composer 何时能点。统一为「永远能点，点了才知道成不成」反而是一致的心智模型。

### 两种 Err 的成因必须可辨（落实现的硬要求）

```rust
agent_steer(root, session_id, text) -> Result<(), String>

  Err 路径三分（文案各自独立，MUST NOT 合并成一句「不可送达」）：
    ① AskRegistry::is_pending(sid)   → 「等待应答中，补充说明不可送达」
    ② SteerRegistry.request miss     → 「会话已收口，补充说明不可送达」
    ③ ack.await 收到 RecvError       → 「未送达（会话在补充说明生效前结束）」
```

判据次序很重要：**ask 判据必须在前**。原因是 ask 阻塞期间 `SteerRegistry` 的条目**仍然在场**（会话没结束，只是 loop 卡在工具分支），所以单看 miss 判不出来——这正是需要 `is_pending` 的原因。

```
时间轴：
  begin_turn ──────────────────────────────────────────── converge
     │              │                    │                    │
  SteerRegistry 登记  ask 开始阻塞      ask 应答/终止       除名
     │              │                    │                    │
     └─ ① miss=false ─┬─ is_pending=TRUE → Err ①             └─ ② miss=TRUE → Err ②
                      └─ is_pending=false → 排队，等 drain
```

### 连带的实现约束（裁决的直接后果）

**A. `AskRegistry` 必须新增 `is_pending`**

现在只有 `register` / `answer` / `remove`。加一个只读查询（`slots.contains_key`，O(1)）：

```rust
pub fn is_pending(&self, session_id: &str) -> bool {
    self.slots.lock().expect("ask 注册表锁不可中毒").contains_key(session_id)
}
```

注意**锁序**——这是已踩过的坑，照抄既有纪律。`control.rs::answer` 的原文：

> 锁序取「先读后投」：`runs` 锁与 ask 注册表锁绝不嵌套（反向嵌套会在引擎侧
> `ask_started` → `publish` 取 `runs` 锁时死锁）

其实现形态是**块内取值、块外投递**：

```rust
pub fn answer(&self, root: &str, change_id: &str, text: String) -> Result<(), String> {
    let session_id = {
        let runs = self.runs.lock()...;          // 锁在块内
        ...
    };                                            // ← 块结束，runs 锁已释放
    self.ask_registry.answer(&session_id, text)   // 再取 ask 锁
}
```

`agent_steer` 必须同式——三步各自独立取放，MUST NOT 任何两段嵌套：

```rust
pub fn agent_steer(/* ... */) -> Result<(), String> {
    // ① 问 ask 是否在等（取 AskRegistry 锁，即刻释放）
    let pending = { ask_registry.is_pending(&session_id) };
    if pending { return Err("等待应答中，补充说明不可送达".to_owned()); }

    // ② 投 steer（取 SteerRegistry 锁，即刻释放）
    let (msg, ack) = ...;
    { steer_registry.request(&session_id, msg)?; }   // miss 即 Err（会话已收口）

    // ③ 等确认（无锁）
    ack.await.map_err(|_| "未送达（会话在补充说明生效前结束）".to_owned())
}
```

若把 ① ② 合成一个块（同时持两把锁），就与本仓已有的 `answer()` 纪律相悖，且与 `execute_ask` 的 `register`（引擎线程持 ask 锁）形成双向等待面。

**B. `pending` 中间态仍然必需**

组合 ① 只是「不加禁用态」，不是「不加 pending 态」。用户点发送后：

```
点发送 → 按钮转「排队中…」（ack 未回）
       ├─ 到达请求边界 → drain → ack 回 → 变「已送达」（短暂）
       ├─ ask 阻塞中   → 立即 Err → 变回可点 + Err 提示
       └─ 会话收口     → ack Err   → 变回可点 + Err 提示
```

`AgentInput` 现在只有 `running` / `disabled` 两个正交标志，需要第三个 `pending`。这是组合 ① 唯一新增的 UI 面。

**C. Err 文案是用户唯一的反馈面**

既然没有禁用态做预防，文案就是全部。三条文案必须**说清是哪一种**、并给出用户可采取的动作（例如「等待应答中」→ 用户知道该先去答题）。MUST NOT 用统一的「发送失败」。

### 已知代价（接受）

```
场景：模型正在 bash 跑 5 分钟测试，用户打字发补充说明
     → 不阻塞（不是 ask），正常排队 → 5 分钟后测试跑完、请求边界 drain → 送达
     → 用户等了 5 分钟才看到「已送达」，期间一直是「排队中…」

场景：模型调了 ask，用户等不及想补一句
     → 立即 Err「等待应答中」→ 用户只能先答题
     → 这是组合 ① 的明确取舍：不做排队，用户得多一步
```

第一场景的长时间 pending 是 drain 点位置的固有结果（不是本裁决引入的），第二场景的多一步是本裁决主动选择的。

## 已裁决（第三轮）：不合并、不留痕

两条看似小，但对 drain 的实现形状有直接影响。

### drain 的单条语义（「不合并」的直接后果）

```
drain 点（loop.rs 顶部请求边界）:

  while let Ok(steer) = rx.try_recv() {
      history.push(Message::user(steer.text));          // 每条一条独立 user 消息
      sender.send(AgentEventKind::Message {             // 每条一条独立密封事件
          role: AgentMessageRole::User,
          blocks: vec![AgentBlock::Text { text: steer.text, citations: None }],
          parent_tool_use_id: None,
      }).await;
      let _ = steer.ack.send(());                        // 逐条确认
  }
```

**为什么用 `try_recv` 循环而不是 `recv` 一次**：不合并要求「一次边界把所有在途 steer 全部取走」。若只 `recv` 一条，剩下的会滞留到下一个请求边界（可能几十秒后），违背「不合并 = 每条都要立刻送」的意图。

**每条独立 ack 的意义**：三条 steer 排队，就绪后逐条 `ack.send(())`，前端的三个「排队中…」分别变「已送达」。这就是「不合并」在产品面的可见形态——如果合并成一段，三条只能共用一个 ack，用户看到的是「一条也没确认 / 全部确认」的二值态。

### 与 rig 消息序的关系（已核实，无冲突）

```
assistant{tool_calls} → user{tool_results} → user{steer_1} → user{steer_2} → ...
                                              └─ 连续多条 user，rig 不合并（已核实 0.43.0）
```

多条连续 user 在 anthropic / openai 两侧都合法（`user_content_to_messages` 只在单条 rig Message 内部聚合）。所以「不合并」不会触发上一节那个 400 陷阱——那个陷阱只在**把 steer 文本塞进 tool_results 那条消息**时才发生。

### 「不留痕」的边界：转录里仍能区分

不需要 `SystemNotice{steer}` 不等于转录里分不清。既有区分手段已足够：

```
walker 组装的 prompt   → 每轮第一条 user 消息（loop.rs 开头 push 的 turn.question）
用户 steer            → 轮中途出现的 user 消息（tool_results 之后）
用户正常多轮对话       → 由 turnDone / run-record 部件分段
```

时间顺序本身就是判据：**turn 中段出现的 user 消息只可能是 steer**（一轮一命，且 walker 只在轮首发 question）。若将来需要更强判据，加 `SystemNotice` 是纯增量改动，不影响本次实现。

**保持不变的另一面**：`AgentSessionView` 的消息渲染零改动——steer 落成普通 user 消息，`AgentMessages` 的既有 user 气泡直接呈现，无需新部件类型、无需 adapter 分支。

## 待决问题（全部已裁）

1. ~~**UI 落点**~~ → **已裁决：抽屉里的 step 会话窗口**（左栏面板判据矩阵不动）。
2. ~~**可用窗口**~~ → **已裁决：不加前端判据**，靠 `SteerRegistry` miss 的 Err 说话（理由见「判据不能靠库态」）。
3. ~~**送达语义**~~ → **已裁决：收口即 Err**，用 drop-Err 实现，不排队不续跑。
4. ~~**consecutive user message 风险**~~ → **已核实**：rig 0.43.0 不合并相邻同角色消息，故必须 push 新 `Message::user`，不能并入 tool_results（见上「必须避开的实现陷阱」）。
5. ~~**与 ask 阻塞的关系**~~ → **已裁决：组合 ①（ask 阻塞期一律 Err，composer 不加禁用态）**。定性分析见「ask 阻塞期间能不能 steer」；裁决与连锁约束见「已裁决（第二轮）」。
6. ~~**一次请求里多条 steer**~~ → **已裁决：不合并**。每条 steer 独立成一条 `Message::user` + 一条密封 `Message{role:User}` 事件，保留「分几次说」的边界（见下「drain 的单条语义」）。
7. ~~**是否需要留痕**~~ → **已裁决：不需要**额外 `SystemNotice`。steer 的 user 消息自然落转录，转录用既有 user 消息呈现即可，零新事件类型、零新 adapter 分支。

## 与别处的相邻性

- `openspec/todo.md` 同族待办（"运行中 step 的活面"这一簇）：
  - 「归一Agent会话发送」——**本质就是 steer 要做的**；steer 落地后应一并关闭，不再另做
  - 「进行中的agent step，统计实时消耗的token」——同族（实时面），未实现
  - 「修复：变更流程决策时等待用户应答，回答后流程继续，但是变更依然显示等待应答窗口」——**已由 ask 落地修复**（核实：`control.rs::resolve_ask` 无条件清 `entry.ask`，且 status 仅在恰为 `WaitingAsk` 时回 `Running`，失效信号复用 `RunNotice::Step`）。todo 勾选状态待用户确认
- 若 steer 做成通用的"对运行中会话注入文本"，它天然也适用于 explore 会话与架构 Agent（同一张 `SteerRegistry`），scope 到 change flow 只是 UI 层的判据。

## 相关文档

- `agent-finalize-tool-for-flow-steps.md` — 第 3 点（收口工具）拆出；**已实现并归档**
  - 实现名 `finish_step`（`orchestration/src/finish.rs` 的 `TOOL_NAME` 单点）
  - `FinishSpec::{EvaluatorChecklist, Decision}` + `SessionInjections.finish`
  - `WorkerTurnOutcome.finalized: Option<FinishPayload>`
  - `step_verdict_gate` 步**整体退役**（verdict 裁决内化进 `step_evaluator`）
  - `decision_session` 的循环结构整体退役（单轮收口为常态）
- **ask 工具：已实现并归档** → `openspec/changes/archive/2026-11-12-ask-tool-in-flow/`
  （原 `openspec/explores/ask-tool-in-flow.md` 已随变更消解，不再单独存在）
  - `AskRegistry` 落 `core/agent/src/ask.rs`（键 = core session id）
  - `AskMode::{Generic, Flow}` + `SessionInjections.ask: Option<AskMode>`
  - `AskPort` 落 `orchestration/src/port.rs`（`ask_started` / `ask_resolved`）
  - `execute_ask` 落 `infra/agent/src/sdk/loop.rs`（两档共用阻塞骨架）
  - `AskUserQuestion` 挂 `ToolTier::Protocol`（任何 permission_mode 放行）
  - `finish_step(Decision)` 动作集已收为两值（`action:"ask"` 协议删除）

三者坐标：

```
            谁发起        阻塞？    产生 step 产出？   实现状态
steer       用户 → turn   非阻塞    否（纯注入）       ← 未实现（本文）
ask         用户 ↔ turn   阻塞      否（纯问答）       已归档
finish_step turn → 编排   非阻塞    是（收口载荷）     已归档
```
