# Agent 实时流的协议统一：事件为唯一事实源 + 单消息回合

workflow_type: requirement

> 本文档是 `unified-agent-stream` 拆分后的第二份，对应 change β-2 `agent-stream-single-message`。
> 前置契约（`AgentEvent.turn_id`）见 `openspec/explores/agent-event-turn-id.md`；本 change 依赖它已落地。

## 起点：两个渲染不一致

1. 会话继续提问时，stream 渲染的提问消息 `role=assistant`；退出重进查 records 同一条 `role=user`。
2. stream 会把思考消息渲染到提问消息之前；重放查询 record 顺序正确。

两者都不是数据问题，是**实时发射层与持久重放层用了两套建模机制**。

## 根因：ai-sdk 的 chunk reducer 假设「一条流 = 一条消息」

`ai/dist/index.js` 的 `processUIMessageStream`：

```js
function createStreamingUIMessageState({ lastMessage, messageId }) {
  return {
    message: lastMessage?.role === "assistant" ? lastMessage : {
      id: messageId, metadata: undefined, role: "assistant", parts: []
    },                                     //  ▲ 单数 message，role 硬编码
    activeTextParts, activeReasoningParts, partialToolCalls,
  };
}

case "start":       state.message.id = chunk.messageId;   // 改名，不新建消息
case "reset-step":  // 只删「最后一个 step-start 之后」的部件
case "start-step":  state.message.parts.push({ type: "step-start" });
```

| ai-sdk 的假设 | 我们的语义 |
|---|---|
| 一次请求 = 一条 assistant 消息 | 一次运行 = 一整轮（提问 + 思考 + 工具 + 回答） |
| role 只能是 assistant | 轮内既有 user 也有 tool |
| 多消息靠多次请求 | 一次 Channel 流推完整轮 |

三个派生代价：

- **① role 丢失**：`UIMessageChunk` 联合类型里没有 role 字段（`index.d.ts:2477-2615`），reducer 一律造 `assistant`。
- **② 顺序丢失**：chunk 协议没有"位置"概念，折叠后顺序 = push 顺序；`AgentEvent.seq` 在 `eventToChunk` 处蒸发。
- **③ provisional 双份**：delta 消息 id 恒为 `delta-none`、密封消息 id 为 `evt-<seq>`，两者必然成为两条消息，只能靠 `reset-step` 剪接 + 收口折叠兜底。

现状之所以"一事件一消息"，是因为**只发 `reset-step` 从不发 `start-step`**：没有 step 标记时"当前 step"就是整条消息，`reset-step` 等于全清，所以每来一个密封事件只能换新消息。

## 方案对照（四象限）

| | 传输 | 语义层 | 修 ①②③ | 备注 |
|---|---|---|---|---|
| A | Channel | chunk（现状） | 需渲染层打补丁 | 收益薄 |
| B | Channel | AgentEvent 折叠 | 全修 | 放弃 useChat reducer |
| C | SSE | chunk | 全不修 | 只换传输，病根继承 |
| **D** | Channel | chunk + step 分段 | 全修 | 一轮一条消息，贴合 ai-sdk 官方形态 |

关于 SSE：Tauri custom protocol 的 `UriSchemeResponder::respond` 只接受 `Cow<'static, [u8]>`（tauri 2.12.5 `app.rs:2629`），**无法流式**；做 SSE 必须在进程内起 HTTP 服务（Cargo.lock 里 axum 0.8.9 只是 devtools→tonic 的传递依赖）。本仓前提「服务端不支持 stream 就维持事件」成立，故不走 C。

## 决定：D —— 一轮一条 assistant 消息

### 部件粒度

"一条消息"限制气泡数，不限制部件数。思考/正文/工具卡各占一个 part，按**首次到达顺序**排列，不做重排、不刻意合并。

```
一轮 = 一条 assistant 消息（气泡 1 个）
┌────────────────────────────────────────────────────────┐
│ ▸ 思考①   ← part，第 1 个到达                            │
│ 正文①     ← part                                        │
│ ┌ Bash ─┐ ← part（toolUse，后续 output 就地补全）         │
│ ▸ 思考②   ← part，工具轮之后重新到达 → 新开一个            │
│ 正文②     ← part                                        │
└────────────────────────────────────────────────────────┘
```

### 机制：step = 一次模型响应

```
start(turnMsgId)                    ← 每轮仅一次，消息 id 固定
start-step   ┌ 响应 1
  reasoning-start r-<seq>-          ← part id 按首个 delta 的 seq 派生
  text-start      t-<seq>-
  sealed M1 → reset-step            ← 只清本 step 的临时部件
            → 追加 sealed 块（id = t-<seq>-<n>）
  tool result → tool-output-available（就地补全，不开新 step）
start-step   ┌ 响应 2
  reasoning-start r-<seq>-          ← 新部件，不与响应 1 的思考合并
  sealed M2 → reset-step → 追加
finish(messageMetadata = 回合统计)
```

### 提问消息

`sendMessage({ text })` 本地推 user 消息（ai-sdk 官方形态），实时流中跳过后端那条 user `Message` 事件。

- 提问天然是右侧 user 气泡，不再靠 seq 猜角色；
- 提问在 assistant 消息**之前** push，结构上不可能再颠倒（现在"思考在前"是两路 push 竞争）；
- 消息 id 需与折叠侧统一，避免收口时重挂 —— 见「id 锚点」。

### record 归属：回合元数据（R2）

业界定位：消息体只装显示内容；回合统计属于 metadata；汇总态走独立状态。

ai-sdk 原生已支持：`finish` chunk 可携带 `messageMetadata`（`index.js:8070`），合并语义为 `mergeObjects(旧, 新)`（`index.js:1384`）。

```
现在：  turnDone → 插一条 data-run-record 消息 → 渲染时被 system 过滤（白跑一趟）
R2：    turnDone → 该轮 assistant 消息.metadata = TurnSummary
        chain 状态照旧（latestAgentFailure 读 chain，不受影响）
```

连带退役：`runRecordToUIMessage`、`data-run-record` 部件、`buildMessages` 里"事件位后插轮行"的逻辑。

备选（未选）：R3 `{type:'data-run-result', transient:true}` 走 `onData` 不进 parts —— 只解决实时路，折叠侧要另造一条，故弃。

### 删除面

```
provisionalMessageId / deltaPartId(常量语义) / openProvisionalChunks
"每个事件发 start + reset-step" 的整段逻辑
runRecordToUIMessage / data-run-record
agent-adapter.ts 里 8 个手写镜像（AgentRunStartedData 等）→ 可改从 bindings 派生
```

## 影响面

```
生产代码：agent-transport.ts（272 行中大部分）、agent-adapter.ts（635→~350）、
          use-agent-session.ts（buildMessages / onFinish 收口）
测试：    agent-transport.test.ts 1826 行 + agent-adapter.test.ts 1570 行，
          chunk 逐块断言大部分失效，需按 step 语义重写
spec：    desktop-agent-chat-infra 的「增量流式」「provisional」条款；
          desktop-agent-execution 的「终态经 Channel 同构流出」表述
```

## 折叠（fold）的按轮分组

折叠函数从「一事件一消息」改为「按轮分组」：`fold(events, turns) → UIMessage[]`。

### 现存缺陷（重写 fold 时一并修）

`buildMessages`（`use-agent-session.ts:74`）以 `TurnDone` 出现次数索引 `turns`：

```ts
if (event.kind === 'turnDone' && turnIndex < turns.length) {
  messages.push(runRecordToUIMessage(turns[turnIndex]));   // 按出现次序配
}
```

该实现假设「每轮恰有一个 TurnDone」，而停止路径不满足：

```
loop.rs:186/198/421  停止时直接 return history（不发 TurnDone）
kernel.rs:308        Running + stop_requested → 另铸 Stopped 终态写轮行
```

后果：无 `TurnDone` 的轮令后续所有轮行**错位一格**，且补尾逻辑把剩余轮行塞到列表末尾而非该轮位置。第 3 轮起错位会持续传递。

### 分组规则

按 `AgentEvent.turn_id` 分组：同一 `turn_id` 的事件归入同一轮。`TurnDone` 仅用于标记 metadata.status（`completed` / `failed` / `stopped`）。

`turn_id` 为 `null`（旧转录）时按 `RunStarted` 顺序分组——每轮必有 `RunStarted`（`loop.rs:152` 在 ask 成功后立即发送），停止轮 / 零事件轮的闭合靠「下一个 `RunStarted` 或数组末尾」。

**`RunStarted` 不能作为新架构的分组依据**：停止轮、失败轮、重订中途都可能看不到它；且零事件轮（ask 失败）没有 `RunStarted`，按它计数会令后续轮整体错位。

### 兼容策略：不做双路径

旧转录 `turn_id` 为 `null` 时按顺序分组（即现有行为）；能解析则不做专门迁移，也不引入第二套分组逻辑。

### id 锚点

实时路与折叠路必须算出**同一个消息 id**，否则收口时 `useChat` 的 `isContinuation` 判定失配、消息重挂。

```
锚点优先级
 ① AgentEvent.turn_id（信封级，由前置 change 提供）  ← 主路径
 ② 本地时钟兜底（仅兜 id，不参与配对）               ← turn_id 为 null 的新旧交界时

① → assistant 消息 id = turn-<turnId>，提问 = turn-<turnId>-user
② → id = evt-<首事件 seq>，收口 fold 时按 turnId 重新对齐（整表替换，用户无感）
```

`turn-<turnId>` 与现有 `turn-${record.turnId}`（`agent-adapter.ts:489`）同族命名。

去掉 `turn_id` 之后，重订中途（`since_seq` 滤掉 `RunStarted`）也无需 `agent_session_watch` 回传 turnId —— 任何一条事件都自带。

### aux 事件：维持独立 system 消息

`runStarted` / `systemNotice` / `raw` 不并入轮 metadata，继续作为独立 system 角色消息：

- 对话透镜本就过滤 `role === 'system'`（`agent-messages.tsx:243`），不影响气泡；
- 保真透镜 `AgentRawStream` 依赖逐事件回放，并入 metadata 会失去数据源——净损失。

## 异常原因的呈现

### 判据

> 模型说的话是 content（parts）；关于这次运行的事实是 provenance（metadata）。

错误原因是"这次运行为什么没产出内容"，属 provenance；但用户需要在**发生的位置**看到它，这由渲染层解决，不需要伪造 part。

### 不用 ai-sdk 的 error chunk

```js
// ai/dist/index.js:8087
case "error": { onError?.(new Error(chunk.errorText)); break; }
// chat 侧 onError 是：(error) => { throw error; }   // index.js:22400
```

它不进 parts、直接抛异常中断流。异常原因只能走 metadata 或 data 部件。

### 每轮恒产一条 assistant 消息

```
metadata.record = TurnSummary（status / error / 统计）
呈现规则：
  parts 非空                       → 正常气泡
  parts 空 + status=failed         → 状态行（错误标题 + 可展开原文）
  parts 空 + status=stopped        → 「已停止」行
  parts 空 + status=completed      → 不渲染
```

### 现存盲区

```
agent-error.ts:128   latestFailed = latestTurn.status === 'failed'   // stopped 不算失败
                     if (!latestFailed && !runningJustFailed) return null
```

停止轮既不显示横幅，消息流里也什么都没有（无 TurnDone、无轮行部件）——用户点停止后界面只是"不再增长"，没有任何标记。

另：`agent-messages.tsx:243` 的 `parts.length > 0` 过滤会**吞掉空内容轮**。D 之后须改为按 `metadata.record.status` 判定，否则停止轮/失败轮整轮从时间线消失。

顶部 `AgentFailureBanner` 保留作"最新一轮失败"的显性提示（读 chain，与 states.events 同源），与轮内状态行不冲突。

## 实时侧的 step 机制

### 判据：密封之后再来 delta 即新 step

```
delta₁ delta₂ → sealed Message(A₁) → toolUse/toolResult
              → delta₃ delta₄ → sealed Message(A₂)      ← 这一批 delta 属于新 step
```

`loop.rs:379` 保证每次成功响应恰发一条 assistant 密封 `Message`（空响应直接收敛），因此「assistant Message 的序数」就是 step 序号，前端可数，后端不传。

### part id：按 delta 的 seq 派生

```
现在：  deltaPartId(key, 'thinking') → 'r-delta-'      ← 常量，跨 step 相撞
改后：  r-<首个 delta 的 seq> / t-<首个 delta 的 seq>
```

- 确定性：同一 delta 永远算出同一 id，重订中途可用（不依赖「第几步」）；
- 简化：每个 delta 的 seq 唯一，不再需要 `openedParts: Map<key, Set<kind>>` 去判断该键是否开过件；
- 代价：id 不可读（看不出 step 序号），如需按步做视觉分组则要另算。

### 已知偏差：pause_turn 会把一次响应切成两步

```
pause_turn 续跑（web_search 开启时，最多 6 次）：
  delta₁ → Message(A₁) → delta₂ → Message(A₂) → … → 工具轮
```

从用户视角 A₁/A₂ 是同一次回答的两截；按上述判据它们是两个 step。**不影响 D 的任何目标**（role / 顺序 / 气泡数），部件仍按到达顺序排列；仅在未来"按步画分隔线或按步折叠"时会多切一刀。记录在案，接受。

## 已定决议汇总

| # | 议题 | 结论 |
|---|---|---|
| D1 | 方案 | **D**：一轮一条 assistant 消息；step = 一次模型响应 |
| D2 | 部件顺序 | 按首次到达顺序；不重排、不刻意合并 |
| D3 | 提问消息 | `sendMessage({ text })` 本地推 user 消息，实时流跳过该 user 事件 |
| D4 | record 归属 | 回合元数据（R2）：`finish` chunk 的 `messageMetadata`；`data-run-record` / `runRecordToUIMessage` 退役 |
| D5 | 轮配对 | 依赖前置 change 的信封级 `AgentEvent.turn_id` |
| D6 | id 锚点 | ① `AgentEvent.turn_id` → ② 本地时钟兜底（仅兜 id） |
| D7 | fold 分组 | 按 `turn_id` 分组；旧转录 `turn_id=null` 走顺序分组 |
| D8 | aux 事件 | 维持独立 system 消息（保真透镜数据源） |
| D9 | 异常呈现 | 每轮恒产一条 assistant 消息；parts 空时按 status 渲染状态行 |
| D10 | step 标识 | 不加 `step_index`；前端按本轮 assistant Message 序数计步 |
| D11 | step 判据 | 「密封 Message 之后再来 delta」即新 step；part id 按首个 delta 的 `seq` 派生 |
| D12 | pause_turn 偏差 | 一次响应被切成两步，接受 |

## 已知需随改的现存缺陷

1. `buildMessages` 按 `TurnDone` 计数配轮行 → 停止轮致后续轮行错位。
2. 停止轮在 UI 上完全无标记（无横幅、无消息）。
3. `parts.length > 0` 过滤会吞掉空内容轮。
4. `agent-adapter.ts:12-70` 的 8 个手写镜像无生成物守卫（`AgentEvent` 加变体不报错）。

---

# 落地分解

> 前置：`agent-event-turn-id` 已合入（`AgentEvent.turn_id` 可用）。

## 一、前端类型与生成物

| # | 改动 | 文件 | 说明 |
|---|---|---|---|
| F1 | 重导出 bindings（前置 change 已做，此处确认可用） | — | `AgentEvent.turnId: number \| null` |
| F2 | 手写镜像收口（建议同批） | `src/lib/agent-adapter.ts:12-70` | 8 个 interface 改从 `bindings.ts` 派生（`Extract<AgentEvent, {kind:'runStarted'}>` 等），消除无守卫的重复定义 |

## 二、前端语义层（`agent-adapter.ts`）

| # | 改动 | 说明 |
|---|---|---|
| A1 | `eventsToUIMessages` → `fold(events, turns)` | 按 `turn_id` 分组；组内 user 一条 + assistant 一条 |
| A2 | 提问消息由 fold 直接产出 user 角色 | 不再依赖 `uiRole` 对 stream 的映射 |
| A3 | record 挂 `metadata.record` | 删除 `runRecordToUIMessage`、`data-run-record` 部件 |
| A4 | 删 `provisionalMessageId` / `deltaPartId(常量)` | 换 `deltaPartId(seq, kind)` |
| A5 | `auxEventData` 保留 | aux 事件仍产独立 system 消息（D8） |

## 三、传输层（`agent-transport.ts`）

| # | 改动 | 说明 |
|---|---|---|
| T1 | 每轮只发一次 `start` | messageId = `turn-<turnId>`（来自首个事件的 `turn_id`） |
| T2 | step 分组：sealed 后首 delta 发 `start-step` | 替换现在的"每事件 start + reset-step" |
| T3 | sealed Message 到达：`reset-step` + 追加整块部件 | 清本 step 临时部件，前面的 step 保留 |
| T4 | `finish` chunk 携 `messageMetadata: { record }` | 删 `enqueueRecord` 的独立消息组 |
| T5 | 提问 user 消息不进流 | 由 hook 侧 `sendMessage({ text })` 本地推 |
| T6 | 删除 `openedParts` 簿记 | part id 由 delta seq 派生后不再需要 |

## 四、hook 层（`use-agent-session.ts`）

| # | 改动 | 说明 |
|---|---|---|
| H1 | `sendMessage` 用 `{ text }` 形态 | 本地推 user 消息，`body` 不变 |
| H2 | 实时流中跳过 `role=user` 的 Message 事件 | 与 H1 配套，避免重复 |
| H3 | `buildMessages` 换 `fold(events, turns)` | 删除 `TurnDone` 计数配轮行逻辑（缺陷 1） |
| H4 | `onFinish` 收口仍走 `fold` | 保持"库为准"的最后一致性 |
| H5 | 重新评估 `pendingRef` / 断流重订 | part id 改为 seq 派生后，重订无需步概念 |

## 五、渲染层（`agent-messages.tsx` 等）

| # | 改动 | 说明 |
|---|---|---|
| V1 | 过滤条件改按 `metadata.record.status` | 修缺陷 3：空 parts + failed/stopped 仍要渲染状态行 |
| V2 | 轮内状态行组件 | failed → 错误标题 + 可展开原文；stopped → 「已停止」 |
| V3 | `AgentFailureBanner` 保留 | 顶部"最新一轮失败"提示，与轮内状态行并存 |
| V4 | `agent-error.ts:128` 补 stopped | 缺陷 2：增加 stopped 的呈现判据（或由 V2 承接） |

## 六、测试重写

| 文件 | 现状 | 处理 |
|---|---|---|
| `agent-transport.test.ts` | 1826 行，chunk 逐块断言 | 按 step 语义重写；保留"两路同构"对拍（改为 fold vs reducer） |
| `agent-adapter.test.ts` | 1570 行 | fold 分组断言；删除 provisional 相关用例 |
| `use-agent-session.test.ts` | 1152 行 | 第二轮回放断言；`buildMessages` 相关用例改 fold |
| `agent-messages.test.tsx` | — | 新增空 parts + failed/stopped 渲染用例 |

## 七、spec 更新（归档前）

| spec | 条目 |
|---|---|
| `desktop-agent-chat-infra` | 「增量流式」条款：provisional 消息语义退役，改为「一轮一条消息 + step 分段」；`TauriAgentTransport` 描述中的 `UIMessageChunk` 映射重写 |
| `desktop-agent-execution` | 「事件投递」条款：轮分组依据改为信封级 `turn_id`；`RunStarted` 不再是轮边界依据 |
| `desktop-agent-execution` | 「终态经 Channel 同构流出」：record 改为 metadata 载体，不再是独立消息 |

## 八、验证顺序

```
1. vp test run src/lib/agent-adapter     ← A1~A5
2. vp test run src/lib/agent-transport   ← T1~T6
3. vp test run src/hooks                 ← H1~H5
4. vp test run src/components/agent      ← V1~V4
5. pnpm client:test                      ← 全量
6. vp check && knip                      ← 死代码（退役的导出应被 knip 报出）
```

## 明确不做

- SSE / 本地 HTTP 服务。
- `MessageDelta` 增加 block_id / index。
- 用户气泡隐藏 explore stance 前导（prompt 展示问题，产品决定，暂不处理）。
