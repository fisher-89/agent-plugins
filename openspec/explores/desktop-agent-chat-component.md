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
