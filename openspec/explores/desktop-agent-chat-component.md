# 桌面端 agent 会话渲染统一组件（sessionId 入参 + 按会话状态挂流）

workflow_type: requirement

## 主题与背景

用户诉求：桌面端归一 agent 会话渲染——前端统一组件，参数 `sessionId`，调通用接口渲染
session 会话；组件内根据会话状态（进行中 / 完成）决定是否监听 stream。

现状结论先行：**通用查询接口与对话透镜组件已经存在且被五处消费**，但「按 sessionId 渲染
会话」这件事在每个视图各自拼装（三种消费形态并存），且「根据会话状态决定是否监听
stream」目前**没有会话级的通用机制**——实时流是 `agent_start` 发起方独占的 Channel，
旁观 / 重挂场景靠重放一次性拉取或 change 域私有的 kind-only 通知重查。归一 = 补一个
会话级组件 + 一个会话级流接入（或通知）机制。

## 现状全景：会话渲染的全部位点

```
                    通用查询接口（已存在，spec 钉死）
        sessionDetail(root, sessionId)      → SessionSummary(row+stats+turns)
        agentSessionTranscript(root, id)    → Vec<AgentEvent>（密封转录，无 delta）
        agentSessions(root, source, ...)    → SessionPage（清单）
                    │
   ┌────────────────┼──────────────────────────────────────────┐
   ▼                ▼                     ▼                     ▼
[形态 A 交互会话]  [形态 A 交互会话]     [形态 B 只读转录]      [形态 C 纯静态]
agent-debug-view   explore-detail-view    session-transcript-   agent-run-history
 useAgentChat       useExploreSession      panel(change 抽屉)    ReplayArea
 (debug,null)      →useAgentChat          useSessionTranscript  eventsToUIMessages
   │                (explore,记录id)       (sessionId+refreshKey)  (events)
   │ 自发流(独占)     │ 自发流(独占)         │ 通知→150ms去抖重查    │ 无刷新
   ▼                ▼                     ▼                     ▼
 AgentMessages + AgentInput/表单        AgentMessages(+SessionMeta) AgentMessages
```

- 形态 A（`useAgentChat`，`src/hooks/use-agent-chat.ts`）：重放装载（sessionId 直查）+
  发送（useChat + `TauriAgentTransport`）+ 停止。流来自**自己发起的** `agent_start` Channel。
- 形态 B（`useSessionTranscript`，`views/changes/hooks/`）：sessionId 直查两命令；
  实时性靠 `refreshKey`（`changeFlowWatch` 的 kind-only `sessionEvent` 通知 → 150ms 去抖
  自增 → 重查转录库）。
- 形态 C：历史抽屉纯静态重放（`agent-run-history.tsx` ReplayArea，且**无轮行交错**——
  纯 `eventsToUIMessages`，与形态 A/B 的 `buildMessages`（事件+轮行交错）不同形）。

## 会话状态判定（「进行中 / 完成」的真源）

- 状态词汇：`AgentRunStatus` = running | completed | failed | stopped（轮行级，
  `TurnSummary.status`）。
- 进行中判定先例：`detail.turns.some((turn) => turn.status === 'running')`
  （use-session-transcript.ts L44）。
- ⚠️ `useAgentChat.running` 是 **chat.status（发送方视角）**，不是库态——旁观 / 重挂
  场景下会话实际在跑而 UI 判定 idle（见缺口 2）。

## stream 监听现状与缺口

```
kernel RunningTurn::drive(on_output)          ← 事件唯一出口（回调）
   delta:    只上传输面（不落库，ephemeral）
   密封事件:  sink.append_sealed 落库 + on_output
   终态:     TurnFinished(outcome) → 轮行
        │
        ▼  exec 轨道命令层（exec/mod.rs agent_start_with）
   spawn 转发任务 → on_event.send(AgentRunMessage)   ← 唯一实时出口
        │                                            = 发起方独占 Channel
        ▼
   发起方前端（useChat 流）          ✗ 无第二出口：无 broadcast、无 watch 命令
                                     ✗ 前端零 @tauri-apps/api/event 全局 listen
                                       （架构纪律：Channel 直投）
```

**缺口 1（无会话级流接入）**：会话在别处发起（explore 页发起后切走再回、change flow
walker 发起、未来任何后台发起）时，挂载中的组件只有一次性重放，后续密封事件 / 轮行
不再到达 UI。spec（desktop-agent-chat-infra）钦定 `reconnectToStream` 恒 null、重连由
重放承担——但重放没有触发器。

**缺口 2（running 判定失真 + 误发风险）**：旁观场景 `chat.status === 'idle'` →
`sendMessage` 守卫（`if (chat.status === 'submitted' || 'streaming') return`）不拦 →
对运行中会话误发 Continue 新轮。后端 `StopRegistry::register` **无重入守卫**（直接
insert 覆盖），第一轮失去停止句柄。前端没有任何「库态 running → 禁发」防线。

**缺口 3（通知机制私有化）**：change run 面已有「进行中才订阅 + 通知驱动重查」完整先例
（`useChangeFlowRun`：`activeRunPresent`（统一查询）为真才 `changeFlowWatch` 补订；
后端 `ChangeFlowControl.subscribe` 返回 broadcast Receiver，命令层 spawn 转发到前端
Channel）——但通知是 **change run 域** kind-only（`RunNotice::SessionEvent` 零载荷），
不是 agent 会话通用基建；且绑定在页面级拼装（use-change-detail 的去抖下传），组件拿不到。

## 归一方向（可跟进线索，按侵入度递增）

- **线索 1（纯前端组件归一，零后端改动）**：新增会话级组件（如
  `components/agent/agent-session-view.tsx`），props `root + sessionId`，内部封装
  「sessionDetail + transcript 装载 → turns.some(running) 判定 → AgentMessages」。
  形态 B/C 三处（change 抽屉、历史 ReplayArea、未来只读面）立即可收编；发送能力以
  slot/组合暴露（explore / debug 的 composer 各异，不内嵌）。
- **线索 2（running 真源化，小改）**：组件内 running 以库态（turns）为准；发送守卫
  同源化（库 running → 禁发），消掉缺口 2 的误发风险。useAgentChat 的 running 语义
  需拆分（「本实例在流」vs「会话在跑」）。
- **线索 3（会话事件广播 + watch 命令，后端）**：agent 域参照 ChangeFlowControl 增会话
  事件注册表（session_id → broadcast）；exec 轨道 drive 转发任务 tee 一份进注册表
  （delta 是否进广播是决策点）；新增 `agentWatch(root, sessionId, Channel)` 命令，进行中
  才订阅（对照 activeRunPresent 先例），终态后通道自然断。StopRegistry 已有
  register/remove 生命周期，可评估挂靠或独立。
- **线索 4（两路同构口径裁定）**：旁观者若收 delta（打字机体验）则「重放无 delta」
  打破 spec「实时路与重放路同构」原则，需修订口径；若只收密封事件（通知或事件直投
  → seq 去重并入，`assembleTranscript` 先例已在），同构保持、延迟 150ms 级。

## 决策点（提案前需拍板）

1. 组件形态：纯只读 + composer slot，还是全功能（含发送/停止）？explore 与 debug 的
   差异（stance 拼接、每跑重置、来源 source）如何外置。
2. 旁观流语义：密封事件级刷新（同构、低带宽、复用通知模式）vs delta 直投（体验好、
   破同构、带宽大）。
3. watch 通道形态：事件直投（Channel<AgentRunMessage> 复用信封）vs kind-only 通知
   （复用 change 先例 + 重查）。
4. 与既有基建关系：useAgentChat / useSessionTranscript 合一进组件，还是组件消费其一 +
  补流？`refreshKey` 去抖逻辑是否随组件内化。
5. spec 面：desktop-agent-chat-infra 组件族 requirement 增补「会话级组件」；
   desktop-agent-execution 增补 watch/广播 requirement（若走线索 3）。
6. change run 面是否随之迁移（session-transcript-panel 收编进统一组件）还是保持
   零触点（有「零触点红线」注释在案）。

## 关键文件索引

- `packages/desktop/src/hooks/use-agent-chat.ts` — 形态 A 基建（重放/发送/流）
- `packages/desktop/src/lib/agent-transport.ts` — Channel → chunk 流（发送方独占）
- `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` — 形态 B（直查 + refreshKey 重查）
- `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` — 「进行中才订阅」先例（changeFlowWatch）
- `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` — change 抽屉只读面（收编候选）
- `packages/desktop/src/views/agent/components/agent-run-history.tsx` — ReplayArea 静态面（收编候选）
- `packages/desktop/src/components/agent/agent-messages.tsx` — 对话透镜（统一渲染件已在）
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` — agent_start 转发任务（tee 挂点）、命令面
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` — change_flow_watch + broadcast 转发先例
- `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` — drive 事件出口、StopRegistry（无重入守卫）
- spec: `openspec/specs/desktop-agent-chat-infra`（组件族/transport 契约）、`desktop-agent-execution`
  （命令面/事件契约）、`desktop-change-flow-view`（通知/去抖口径）

---

# 二轮：漂移防线审计（线索 4 的前置问题）

## 「两路同步」实际是四个层，只有一层是真问题

```
L1 字段面  Rust AgentEventKind ──specta──► bindings.ts      结构同步
L2 语义面  eventToChunk(密封) ─┐                           映射同步
           eventsToUIMessages ─┴─► UIMessage.parts
L3 内容面  delta 累积文本  ≟  密封整块文本                   内容同步
L4 身份面  delta-<key> part ≟ t-<seq>-<n> part             身份同步
```

**关键前提**：delta 与密封在 Rust 侧是**同一枚举的不同变体**（`event.rs` `AgentEventKind`），
`is_delta()` / `is_sealed()` 互补不重叠（`event.rs:115-123`）。故 L1 **结构上不可能漂移**。

## 逐层防线强度

| 层 | 机制 | 证据 | 强度 |
|---|---|---|---|
| L1 | specta 单点派生 + diff 守卫 | `bindings:check` 脚本 | 守卫**未接进 `check`**（`check` 走就地 `bindings:export`，生成物永远最新 → 改名被 tsc 拦、**新增变体/语义变化不拦**） |
| L2 判别 | `is_delta`/`is_sealed` + `SealedAgentEvent = Exclude<...,{kind:'messageDelta'}>` | `agent-adapter.ts:86` | TS 侧 Exclude 是**手写镜像**，ephemeral 集变更靠人工跟 |
| L2 映射 | `auxEventData` **两路共用** | `agent-adapter.ts:144` | 好模式（run-started/notice/result/raw 只写一遍） |
| L2 映射 | `blocksToParts`（:212）vs `blocksToChunks`（:309） | — | **两套独立实现**，四变体语义各写一遍 |
| L3 | 对拍测试「两路终态形状一致」 | `agent-adapter.test.ts:536` | 仅**一个 fixture**（6 事件）；投影 `shapeOf` 是**第三份手写语义** |
| L3 | **`onFinish` 折叠重建** | `use-agent-chat.ts:342` | **唯一「漂移自动愈合」闸**：每轮终止无条件折叠重算 |
| L4 | `messageId(seq)` 两路共用；part id 故意分域 + `PartView key={index}` | `:102` / `:65` | 好 |

## 三个真实缺口（现有防线抓不到）

1. **双实现只靠一个样例对拍**。跨消息 tool 配对本就是双实现的根因（折叠路有全局视野
   `collectToolResults` 预扫，实时路单事件无视野）。
2. **`auxEventData` 的 `default` 分支是静默漂移通道**：Rust 新增密封变体 → TS 编译不报错
   → 运行时静默降级 `data-raw`。spec「未知事件不丢」正确（运行时安全），代价是**无编译期提醒**。
3. **bindings 是「自动同步」而非「拦截」**：`check = server:check && bindings:export && client:check`
   ——就地重导后跑 tsc，故 tsc 永远看最新生成物。

## 收敛闸的边界（本轮最重要的判断）

```
实时流（delta+chunk） ── 过程态：允许粗糙
        │  流结束（含失败/停止）
        ▼
buildMessages(events, turns) ── 终态真相：单点生成，唯一形态
```

实时路**从来不是真相源**，是打字机装饰。但 `onFinish` 闸**只对「我自己发起的这一轮」生效**
——旁观路没有等价触发点。任何让旁观者走 delta 的设计，必须自己再造收敛触发点，
否则漂移从「一轮内瞬时」升级为「永久」。

## 三条加固路线

- **路线 ①（根治，折叠=跑 reducer）**：ai-sdk v7.0.118 确实导出
  `readUIMessageStream({message?, stream})`。若 `eventsToUIMessages` = `按 start 切分
  eventToChunk(event)` + reducer 累积，则双实现消失、`collectToolResults` 预扫可删、
  同构从「被测出来的性质」变成「构造出来的恒等式」。代价：折叠路由同步变异步
  （732 行测试 + 全部调用点）。
- **路线 ②（IR 化，中等）**：`auxEventData` 的共用模式推到块级 ——
  `AgentBlock → BlockPlan`（唯一语义点），两路差异收敛为「配对策略不同」一个显式参数。
- **路线 ③（可探测化，低成本）**：(a) `const _exhaustive: never = event` 穷尽性双轨
  (b) ephemeral 集由 bindings 派生，消手写 `Exclude` (c) 对拍测试参数化矩阵
  (d) `bindings:check` 接进 `check` (e) Rust 导出真实 JSONL fixture 供前端消费。

---

# 三轮：决策拍板「旁观者要 delta」——连锁推演

## 拍板：delta 进入旁观路

带来四项连锁（按严重度）：

1. **delta 不可重查**（ephemeral 不落库）→ 订阅路**必须投递载荷**，kind-only 通知不够。
2. **推翻既有 D3 降位决策**：`RunUpdate::SessionEvent { session_id, event }` 在进程内 seam
   **本就携带完整事件载荷（含 delta）**，仅在 IPC 面被降位为 kind-only `RunNotice::SessionEvent`
   （`state.rs:277/324`，理由「通知仅失效信号，查询结果权威」）。delta 无查询结果可依，该理由
   对 delta 不成立。
3. spec 修订：`desktop-agent-chat-infra` 明写「`reconnectToStream` 返回 null（重连由重放承担）」
   （Module Contract 表）。
4. 漂移面翻倍：旁观路也跑 delta 累积，且没有天然收敛闸。

## 枢纽发现：`chat.resumeStream()` 是一等机制，不是 hack

ai-sdk v7 `AbstractChat.resumeStream()` → `transport.reconnectToStream()`（`index.js:22101`）：

```
resumeStream() ──► makeRequest({trigger:'resume-stream'})
                     ├─ transport.reconnectToStream() → ReadableStream | null
                     ├─ null → status='ready' 直接返回（现状：恒 null）
                     └─ 流 → processUIMessageStream（与 sendMessage 同一 reducer）
                              ├─ lastMessage: undefined（消息从空累积，push 到末尾）
                              ├─ start chunk 覆盖 state.message.id（index.js:8062）
                              └─ 流不 close → status 恒 'streaming'
```

**一举解决三个缺口**：

- 缺口 1（无旁观流）：`reconnectToStream` 返回会话订阅流即可。
- 缺口 2（running 判定失真）：resume 时 `status='streaming'` → `running` **自然为真**，
  无需另设判定；且 `sendMessage` 守卫（`chat.status` 检查）自动生效 → 误发风险消失。
- 双实现风险：resume 与 sendMessage **共用同一 `processUIMessageStream` reducer**，
  不是第三条渲染路径。

## 但 `onFinish` 闸的行为需 PoC 确认

`onFinish` 在流 close 时触发。若订阅流是长连接（不 close），则 `onFinish` 永不到达 →
**旁观路没有收敛闸**。设计上必须显式补：①每次密封事件到达时折叠重算（增量校正）
②lagged/断流时全量重查 ③轮终态（订阅流收到 record 即 close）时折叠重算。

## 五个必须设计的坑

```
① 重复消息    pushMessage 不查重（index.js:22392）
               resume 流的 evt-N 若历史已装载 → 重复气泡
               → 订阅必须带 since_seq（重放锚点），后端按 seq 裁剪

② 广播溢出    broadcast::channel(512)（control.rs:13，既有先例）
               delta 量级：单轮数千条 → 慢订阅者必 Lagged
               → RecvError::Lagged 时重挂（重放全量 + 续订）

③ delta 半截  中途接入首个 delta 无前文（文本从接入时刻起）
               → 密封整块替换校正（现有 openProvisionalChunks 已处理无 start 的裸 delta）
               → 但「半截文本」窗口期 UI 可见，需接受或加「同步中」态

④ 粒度错配    change 广播是 per-(root, change_id)，一次 run 含多会话
               会话订阅需 per-session_id → 需新注册表，不能复用 ChangeFlowControl

⑤ 两轨统一    exec（agent_start）零广播；change（worker）有载荷但粒度/面不同
               → 会话级注册表须两轨共写（kernel drive 回调是唯一共同出口）
```

## 终局图景：从「两路同构」到「一路」

若采纳路线 ①（折叠=跑 reducer），则：

```
重放装载   = processUIMessageStream( chunk流 from seq 0 )      ─┐
                                                              ├─ 同一 reducer、同一实现
resume/旁观 = processUIMessageStream( chunk流 from since_seq )  ─┘
```

**重放与增量只是流的起点不同，不是两套实现**。这才是「归一」的终局——同构不再是需要维护
的性质，而是恒等式。`since_seq` 成为唯一拼接口。

## 待验证（PoC 清单）

1. `resumeStream()` 连续调用（流结束后再调）是否累积重复 message（受 ① 影响）。
2. 长连接流下 `onFinish` 是否真的不触发（决定收敛闸补法）。
3. `readUIMessageStream` 对 `reset-step` / `messageMetadata` / 跨消息 tool 配对的行为
   （决定路线 ① 可行性）。
4. broadcast 512 在 delta 负载下 lagged 的实际触发频率（决定容量与重挂策略）。

## 本轮新增关键文件索引

- `packages/desktop/node_modules/ai/dist/index.js:22101`（resumeStream）
  `:22286-22352`（resume 分支）`:22392`（pushMessage 不查重）`:8062`（start 覆盖 id）
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs:277-345`
  — `RunUpdate::SessionEvent{event}` 带载荷 / `RunNotice` kind-only 降位
- `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs:60-73`
  — change 域会话事件出口（载荷已带 delta，`is_sealed()` 只影响累积不影响透传）
- `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs:13,78`
  — `UPDATE_CAPACITY=512` broadcast 容量与 lagged 兜底注释

---

# 四轮：PoC 实测（临时测试跑通后删除）——三条结论推翻两条原判

验证方式：临时 `src/lib/poc-drift-verify.test.ts`，用真实 `eventToChunk` / `eventsToUIMessages`
驱动真实 `useChat` reducer（jsdom + renderHook），跑完删除，仓库无残留。

## 结论总表

| # | 命题 | 结果 | 影响 |
|---|---|---|---|
| V1 | `start` 后紧跟 `reset-step` 是否误清前序消息 parts | ✅ 安全 | 逐事件 `start+reset-step` 模式可用 |
| V2 | 跨消息 tool 配对两路是否同形 | ❌ **不同形** | **路线 ① 不可行**；「两路同构」须降级 |
| V3 | 单消息 `resumeStream` 连续调用是否重复 | ✅ 不重复 | — |
| V5 | 多消息 `resumeStream` 连续调用是否重复 | ❌ **必重复** | `since_seq` 是**必需**非优化 |
| V4 | 长连接流下 `resumeStream` / `status` / `onFinish` | ⚠️ 三连异常 | 订阅流**须按轮划分** |
| V6 | 裸 delta 直投 resume 流 | ❌ status=error、消息丢失 | transport 必须补开件组 |

## V2（决定性）：跨消息 tool 配对，两路物理上不可能同形

```
输入： evt-1 = [toolUse tu_1 (Bash)]
       evt-2 = [toolResult tu_1]

折叠路  evt-1: tool-Bash  output-available  ← 预扫配对（collectToolResults 有全局视野）
        evt-2: []                            ← result 已并入前一条

reducer evt-1: tool-Bash        input-available   ← 无 output（等不到）
        evt-2: tool-工具调用    output-available  ← 重复占位卡

两路同形？ false
```

**根因**：reducer 的 `getToolInvocation` fallback 只扫 `state.message.parts`（**当前消息**），
物理上无法跨消息配对；折叠路的 `collectToolResults` 有全局预扫视野。这是**能力差异**，
不是实现疏漏。

**对照实验（V2b）**：toolUse 与 toolResult 同处一条消息时 → **同形 = true**。
即现有对拍测试 `agent-adapter.test.ts:536` 的 fixture 恰好处在"能工作"的场景，从未覆盖跨消息分支。

**由此推翻两条原判**：

1. **路线 ①（折叠 = 跑 reducer）不可行** —— 它不是"消除双实现"，而是**把折叠路降级**到
   reducer 能力（丢失跨消息配对），直接违反 spec「工具同 id 配对收口于适配层」requirement。
   `readUIMessageStream` 更是**单消息语义**（"each stream part is a different state of the
   same message"），连列表重建都要手写切分，叠加上述能力缺失，终局方案作废。
2. **spec 的「两路同构」是不可达目标** —— 须降级为「**终态同构 + 过程近似**」：
   折叠路是终态真相，实时路是过程近似；跨消息 tool 在流式期间**会闪现双卡**，轮结束时
   `onFinish` 折叠重建纠正。这一表述比现状 spec 诚实，也给了 `onFinish` 闸正当地位。

## V5（架构级）：多消息 resume 必然重复

```
after 1st resume = ["evt-1","evt-2"]
after 2nd resume = ["evt-1","evt-2","evt-1","evt-2"]   ← 全量重复
```

机制（`index.js:22392`）：`pushMessage` 不查重；每遇 `start` chunk 判 `state.message.id ===
lastMessage?.id` → 相同才 `replaceMessage`，否则 `pushMessage`。多个 start 时 `lastMessage`
持续变化 → **每次都 push**。

单消息场景（V3）幸免：`start` 覆盖 id 后恰与 lastMessage 相同 → replace 覆盖。
**多消息场景无此侥幸** → 订阅流 MUST 只投递重放末尾之后的 seq（`since_seq` 裁剪）。

V5b 反向验证：历史 `evt-1` 后 resume 只来 `evt-3` → 正确追加
`["<user-msg>","evt-1","evt-3"]`，无重复。**这就是 `since_seq` 的必备性证明。**

## V4（设计约束）：长连接的三个异常

```
resumeStream() 的 promise       永不 resolve   → 组件内 MUST fire-and-forget（void）
status                          恒 'streaming' → 缺口 2（running 判定）天然解决
onFinish                        恒不触发（即使 push 了 finish chunk）
```

**`finish` chunk ≠ 流结束**：`onFinish` 在 `makeRequest` 的 `finally` 触发，仅当
`consumeStream` 结束（流 close / error / abort）。只 enqueue `finish` 而不 `close()`，
status 永远停在 `streaming`、`onFinish` 永远不到。

**由此得出核心设计决策**：

> **订阅流 MUST 按「轮」划分**——每轮一个 `ReadableStream`，轮终态（收到 `Record` 信封）
> 即 `close()`。这样 `onFinish` 每轮触发一次 → `buildMessages` 折叠重建（收敛闸）保住，
> 且 status 回 `ready` → `running` 转 false，可发送态正确恢复。

跨轮不 close 的长连接会同时杀掉收敛闸与发送态复位，**MUST NOT 采用**。

## V6（实现细则）：裸 delta 直投会炸

```
resume 流 = [text-delta(id=t-delta-none)]   →  status = 'error'，messages = []
resume 流 = [start, reset-step, text-start, text-delta] →  messages = ["delta-none"] ✅
```

reducer 要求 part 已开（`missing text part`）。现有 transport 的 `openProvisionalChunks`
（`agent-transport.ts:94`）正是为此存在——**订阅流实现 MUST 复刻这套开件簿记**
（配对键首 delta 补发 start + reset-step + part-start；同键新 kind 仅补 part-start；
密封/aux 事件清空簿记）。

## 新发现坑 ⑥（本轮最隐蔽）：折叠重建依赖 eventsRef 全量

`onFinish` → `buildMessages(mirrors.eventsRef.current, mirrors.turnsRef.current)`。
若旁观组件**只订阅增量**而不先重放全量，`eventsRef` 只有接入后的事件 →
**onFinish 折叠重建会把历史抹掉**。

设计约束：
- 组件挂载 MUST 先做全量重放装载（`sessionDetail` + `agentSessionTranscript`）填满
  `eventsRef` / `turnsRef`；
- 订阅 MUST 从重放末尾的 `seq + 1` 续接（`since_seq`），增量 append 进同一 ref；
- 两者缺一，收敛闸即成为"数据丢失器"。

## 修正后的可行性判断

```
路线 ① 折叠=跑 reducer      ✗ 作废（V2 能力差异不可逾越 + readUIMessageStream 单消息语义）
路线 ② IR 化（BlockPlan）   ✓ 保留，且现在是唯一能收敛双实现的路
路线 ③ 可探测化             ✓ 保留（穷尽性 / 参数化对拍 / bindings 守卫 / 共用 fixture）
新增：spec 表述修订          ✓ 「两路同构」→「终态同构 + 过程近似」
新增：订阅流形态             ✓ 按轮划分 + since_seq 裁剪 + 复刻开件簿记 + 先全量重放
```

`onFinish` 闸的定位也随之明确：**它不是"兜底防御"，而是「终态同构」这一契约的
唯一实现机制**。这反过来解释了现状代码为何那样写——它是特性，不是巧合。

## 待定（PoC 未覆盖，实现期需定）

1. 半截文本窗口期（中途接入首个 delta 无前文）是否可见异常——V6 只验证"能否归约"，
   未验证视觉连续性。
2. 轮内 lagged（broadcast 溢出）时的重挂时序：重放快照与续订之间的事件窗口如何补。
3. `turn` 边界与 `Record` 信封的对应：exec 轨道 `Record` 即轮终态，可直接用作 close 信号。
