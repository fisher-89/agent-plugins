# 桌面端 agent 会话渲染统一组件（sessionId 入参 + 按会话状态挂流）

workflow_type: requirement

## 主题与背景

桌面端归一 agent 会话渲染：前端统一组件，入参 `sessionId`，调通用接口渲染会话；
组件内按会话状态（进行中 / 完成）决定是否监听 stream。

**结论先行**：

1. **通用查询接口与对话透镜已存在**（`sessionDetail` / `agentSessionTranscript` /
   `AgentMessages`），但「按 sessionId 渲染一个会话」在每个视图各自拼装（三种消费形态）。
2. **「按会话状态挂流」没有会话级机制**——实时流是 `agent_start` 发起方独占的 Channel，
   旁观 / 重挂场景无第二出口。
3. **切入点是 ai-sdk 已内建的 `chat.resumeStream()`**（→ `transport.reconnectToStream()`），
   它让旁观路复用同一个 reducer。但实测暴露了四条硬约束（见「核心设计约束」）。
4. **适配层存在真实双实现**（`blocksToParts` vs `blocksToChunks`），且实测证明跨消息
   tool 配对**两路物理上不可能同形** → spec 的「两路同构」须降级为「终态同构 + 过程近似」；
   收敛手段为 IR 化（路线 ②），而非让折叠路复跑 reducer（已证伪）。

## 现状全景：会话渲染的位点

```
                通用查询接口（已存在）
      sessionDetail(root, sessionId)    → SessionSummary(row + stats + turns)
      agentSessionTranscript(root, id)  → Vec<AgentEvent>（密封转录，无 delta）
      agentSessions(root, source, …)    → SessionPage
                  │
   ┌──────────────┼───────────────────────────┐
   ▼              ▼                           ▼
[交互会话 A]    [交互会话 A]              [只读转录 B]              [纯静态 C]
agent-debug     explore-detail-view       session-transcript-     agent-run-history
  useAgentChat    useExploreSession         panel(change 抽屉)      ReplayArea
  (source=debug)  (source=explore)         useSessionTranscript    eventsToUIMessages
     │                │                    (sessionId+refreshKey)   (无轮行交错)
     └─ 自发流(独占) ──┘                         │ 通知→150ms 去抖重查
                  ▼                             ▼
            AgentMessages（4 个渲染位点共用）
```

- `AgentMessages`（`components/agent/`）已是统一渲染件，4 个位点消费；
  `AgentInput` 为 composer（explore / debug 各自适配）。
- **三种形态数据源不同形**：A/B 用 `buildMessages`（事件 + 轮行交错），C 用裸
  `eventsToUIMessages`（无轮行）——同一透镜喂两种形状。

## 关键事实（全部已验证）

### 后端

| 事实 | 位置 |
|---|---|
| kernel `RunningTurn::drive(on_output)` 是事件唯一出口（回调） | `core/agent/src/kernel.rs:191` |
| delta 只上传输面、不落库（ephemeral） | `kernel.rs:229`（`is_delta()` 分支 skip `append_sealed`） |
| 密封事件落库 + 流出同源 | `kernel.rs:264-275` |
| exec 轨道唯一实时出口 = `agent_start` 的 Channel（发起方独占） | `commands/exec/mod.rs:103-116` |
| **无任何广播 / watch 命令**；前端零 `@tauri-apps/api/event`（架构纪律：Channel 直投） | 全仓 grep 确认 |
| `StopRegistry::register` **无重入守卫**（insert 直接覆盖） | `kernel.rs:28` |
| `RunUpdate::SessionEvent { session_id, event }` **进程内本就携带完整载荷（含 delta）**，仅 IPC 面经 `.into()` 降位为 kind-only `RunNotice::SessionEvent` | `orchestration/src/state.rs:277,324` |
| 降位理由原文：「通知仅失效信号，查询结果权威」 | `state.rs:322` |
| change 域广播容量 `UPDATE_CAPACITY = 512`，溢出即 lagged，靠重挂兜底 | `orchestration/src/control.rs:13` |

> **delta 决策的含义**：`SessionEvent` 带载荷却不投递，是因为密封事件可重查落库转录。
> **delta 是 ephemeral、无查询结果可依**，故对 ephemeral 数据「查询权威」原则失效，
> 必须改用「载荷权威」。这不是新增 feature，是数据属性决定投递契约的分界线。

### 前端

| 事实 | 位置 |
|---|---|
| `reconnectToStream` 恒返回 null，「重连由重放承担」（spec 明文） | `agent-transport.ts:243` |
| `useAgentChat.running` = `chat.status`（**发送方视角**，非库态） | `use-agent-chat.ts:363` |
| 库态 running 判定先例 = `detail.turns.some(t => t.status === 'running')` | `use-session-transcript.ts:44` |
| **`onFinish` 无条件折叠重建**（`buildMessages(eventsRef, turnsRef)`） | `use-agent-chat.ts:342-344` |
| 折叠重建依赖 `eventsRef` **全量** | 同上 |
| 适配层双实现：`blocksToParts`(:212) vs `blocksToChunks`(:309) | `agent-adapter.ts` |
| `auxEventData`(:144) 两路共用（data part 与 data chunk 碰巧同形） | `agent-adapter.ts` |
| `SealedAgentEvent = Exclude<…, {kind:'messageDelta'}>` 是**手写镜像** | `agent-adapter.ts:86` |
| 对拍测试「两路同形」仅一个 fixture（6 事件） | `agent-adapter.test.ts:536` |
| 全部 part id 断言在 **delta 域**（`t-delta-*`）；密封块 `t-${seq}-${index}` **零断言** | `agent-adapter.test.ts:467,661` |

### PoC 实测（临时测试驱动真实 `useChat` reducer，跑通后删除）

| # | 命题 | 结果 |
|---|---|---|
| V1 | `start` 后紧跟 `reset-step` 是否误清前序消息 parts | ✅ 安全（`getCurrentStepParts` 无 `step-start` 时返回末条起） |
| **V2** | 跨消息 tool 配对两路是否同形 | ❌ **不同形**（见下） |
| **V5** | 多消息 `resumeStream` 连续调用 | ❌ **全量重复** |
| **V4** | 长连接流下 resume / status / onFinish | ⚠️ 三连异常 |
| V6 | 裸 delta 直投 resume 流 | ❌ `status='error'`、消息丢失 |
| V3 | 单消息 resume 连续调用 | ✅ 不重复（多消息无此侥幸） |

**V2 实证**：

```
输入   evt-1 = [toolUse tu_1 (Bash)]      evt-2 = [toolResult tu_1]

折叠路  evt-1: tool-Bash      output-available   ← collectToolResults 全局预扫
       evt-2: []                                  ← 已并入前一条

reducer evt-1: tool-Bash      input-available    ← 等不到 output
       evt-2: tool-工具调用  output-available    ← 重复占位卡

同形？false        （对照：同一条消息内配对 → 同形 true）
```

根因是**能力差异非实现疏漏**：reducer 的 `getToolInvocation` fallback 只扫
`state.message.parts`（**当前消息**），物理上无法跨消息配对；折叠路的预扫有全局视野。
现有对拍测试的 fixture 恰好处在「能工作」的场景，**从未覆盖跨消息分支**。

**V5 机制**：`pushMessage` 不查重（`index.js:22392`）。每遇 `start` 判
`state.message.id === lastMessage?.id` → 相同才 `replaceMessage`，否则 `pushMessage`。
多 `start` 时 `lastMessage` 持续变化 → 每次都 push。
反向验证：历史 `evt-1` 后仅来 `evt-3` → 正确追加，无重复 → **`since_seq` 裁剪是必备**。

**V4 机制**：`onFinish` 在 `makeRequest` 的 `finally` 触发，仅当流 close / error / abort。
只 enqueue `finish` chunk 而不 `close()` → status 永远 `streaming`、`onFinish` 永不到、
`resumeStream()` 的 promise 永不 resolve。

## 核心设计约束（由事实推导，均为硬约束）

```
① 订阅流 MUST 按「轮」划分     每轮一个 ReadableStream，收到 Record 信封即 close()
                              理由：长连接会同时杀掉 onFinish 收敛闸与 status 复位（V4）
② 订阅 MUST 带 since_seq      只投递重放末尾之后的 seq
                              理由：多消息 resume 必然全量重复（V5）
③ 订阅流 MUST 复刻开件簿记     配对键首 delta 补发 start + reset-step + part-start
                              理由：裸 delta 直投 status=error（V6）
④ 组件 MUST 先全量重放         填满 eventsRef / turnsRef 后从 seq+1 续订
                              理由：onFinish 折叠重建依赖 eventsRef 全量，只订阅增量
                                    会让收敛闸变成数据丢失器
⑤ 两路同构 MUST 降级为         终态同构 + 过程近似
  「终态同构 + 过程近似」      理由：V2 证明跨消息 tool 配对能力不对称；
                              流式期间会闪现双卡，轮结束由 onFinish 折叠纠正
```

**附带效应**：resume 期间 `status='streaming'` → `running` 自然为真 →
`sendMessage` 守卫（检查 `chat.status`）自动生效，**「旁观时误发 Continue」风险消失**。
（此前 `useAgentChat.running` 是发送方视角，旁观恒 idle → 守卫不拦；
叠加 `StopRegistry` 无重入守卫会让第一轮失去停止句柄。）

> **`onFinish` 闸的定位**：它不是兜底防御，而是「终态同构」这一契约的**唯一实现机制**。
> 现状代码那样写是特性，不是巧合。

## 方案：路线 ② ── 适配层 IR 化

### 差异矩阵：9 个维度，只有 1 个是真差异

| 维度 | 折叠路 `blocksToParts` | 实时路 `blocksToChunks` | 性质 |
|---|---|---|---|
| text | `{type:'text', text, state:'done'}` | `text-start/delta/end` | 发射格式 |
| thinking | `{type:'reasoning', …}` | `reasoning-start/delta/end` | 发射格式 |
| tool 已配对 | 单 part `output-available` | **两个 chunk** | 发射格式 |
| tool 未配对 | 单 part `input-available` | 单 chunk | 发射格式 |
| toolResult 已配对 | 跳过 | 跳过 | 一致 |
| toolResult 孤儿 | `tool-工具调用`, input=null | 同左（两 chunk） | 一致 |
| **配对视野** | **全局预扫** | **仅同消息** | **能力差异（V2）** |
| **part id** | 无（数组序） | 必须显式 `t-${seq}-${index}` | 发射格式 |
| 事件头 | 无 | `start` + `reset-step` | 发射格式 |

**8 个格式差异压进薄 Emitter，1 个能力差异显式隔离。**

### 三层切分

```
[视野层] PairingScope                    ← 差异隔离于此，两路各异
   global(events) → PairedResults        折叠路：跨消息预扫
   local(blocks)  → PairedResults        实时路：单消息内
              │  统一形态 Map<toolCallId, ToolResultBlock>
[语义层] planBlock(block, paired) → BlockPlan | null    ← 唯一语义点
              │                                         （四变体 + 孤儿 + 字段映射）
[发射层] plansToParts(plans) → parts                    ← 机械映射，无判定
         plansToChunks(plans, seq) → chunks[]           ← 机械映射 + id 派生
```

### BlockPlan 定稿

```ts
/** 块呈现计划：块语义的唯一表达（两路共用；不含发射格式、不含 id、不含视野） */
type BlockPlan =
  | { kind: 'text'; text: string }
  | { kind: 'reasoning'; text: string }
  | {
      kind: 'tool';
      toolCallId: string;
      toolName: string;
      input: unknown;
      /** null = 未配对（input-available）；非 null = 已配对（output-available） */
      output: AgentToolOutput | null;
    };

/** 一个 block → 至多一个计划；null = 已配对的 toolResult（不重复呈现） */
function planBlock(block: AgentBlock, paired: PairedResults): BlockPlan | null;
```

**孤儿在 IR 层与正常 tool 合并**：孤儿只是
`{kind:'tool', toolName:'工具调用', input:null, output:{…}}`，与正常 tool 同变体。
现状两路各写一遍孤儿判定，IR 化后成为 `planBlock` 内一个三元表达式 ——
**变体数 5 → 3，语义点 2 处 → 1 处**。

### Emitter 映射表（纯机械）

| BlockPlan | → parts | → chunks |
|---|---|---|
| `text` | `{type:'text', text, state:'done'}` | `text-start`/`text-delta`/`text-end`（id=`t-${seq}-${i}`） |
| `reasoning` | `{type:'reasoning', text, state:'done'}` | `reasoning-start`/`delta`/`end`（id=`r-${seq}-${i}`） |
| `tool`, output=null | `{type:'tool-'+name, toolCallId, input, state:'input-available'}` | `tool-input-available` |
| `tool`, output≠null | `{…, state:'output-available', input, output}` | `tool-input-available` + `tool-output-available` |

`plansToChunks` 只产块相关 chunk；**事件头（`start`/`reset-step`）仍由 `eventToChunk` 负责**
（与现状一致，现有 `chunks.slice(2)` 断言零破坏）。

**part id 规则可自由调整（已验证）**：全部 `t-`/`r-` 断言都在 delta 域（由 `deltaPartId`
生产），密封块 id 零断言 → 索引从「blocks 下标」改为「plans 下标」无测试耦合。

### 佐证：视图层早已是 IR 形态

```tsx
// components/agent/agent-blocks.tsx — ToolPairCard props
{ name: string; toolCallId: string; input: unknown; output: AgentToolOutput | null }
```

与 `BlockPlan.tool` **字段级同构**（`output: null` 即 input-available 态）。
**IR 化不是新发明，是把视图层已有抽象上移到适配层**，让 adapter 不再自持平行语义。

### 收益（诚实量化）

**不能说「减少改动点数量」**——新增 `AgentBlock` 变体仍需改 3 处
（`planBlock` 分支 → 可能加 BlockPlan 变体 → 两 Emitter 各加分支）。真实收益是性质变化：

| 现状 | IR 化后 |
|---|---|
| 2 处各自含判定，漏一处**类型系统不抓** | 判定集中 1 处；Emitter 穷尽 switch 使漏改**编译报错** |
| 对拍靠 `shapeOf`（第三份手写语义投影） | 对拍下沉计划层：`plansToParts` vs `reduce(plansToChunks)`，**无需 shapeOf** |
| 跨消息差异隐性存在（V2 静默不同形） | 隔离在 `PairingScope`，测试显式锁定其形态 |
| 孤儿判定写 2 遍 | 孤儿 = tool 变体 + 三元表达式，1 遍 |

### 测试形态：把「假装同构」换成「分层诚实」

```
计划层对拍（可达）   同一 BlockPlan[] 喂两 Emitter → 断言同形
                     差异只剩「视野如何产出 paired」，与发射无关
视野层快照（显式）   同消息配对：global ≡ local  → 断言相等
                     跨消息配对：global ≠ local  → 断言差异具体形态（防扩大）
```

### 实施顺序（每步可独立合入 / 回滚）

```
步 1  引入 BlockPlan + planBlock + PairedResults（新增，不改调用方）
步 2  视野逻辑收成命名函数 collectPairing / localPairing（行为零变化）
步 3  blocksToChunks → plansToChunks（eventToChunk 调用点切换）
步 4  blocksToParts → plansToParts（eventsToUIMessages 调用点切换）
步 5  删旧双实现；补计划层对拍 + 视野差异快照测试
```

验判据：
1. 现有 adapter 测试全绿（`shapeOf` 相关随步 5 一并重塑）
2. 新增计划层对拍测试通过
3. 新增视野差异快照测试通过
4. `planBlock` / `auxEventData` 含 `const _exhaustive: never`（与路线 ③a 合并）
5. knip 无新增未用导出（AGENT.md 红线：不做 test-only export）

## 配套：路线 ③ ── 可探测化

| # | 项 | 目的 |
|---|---|---|
| a | `const _exhaustive: never = event` 于 `auxEventData` / `planBlock` | 新增变体**编译期提醒**（现状：`default` 静默降级 `data-raw`） |
| b | ephemeral 集由 bindings 派生，消手写 `SealedAgentEvent = Exclude<…>` | 消手写镜像 |
| c | 对拍测试参数化矩阵（4 变体 × 边界 × 交叉） | 补 V2 暴露的覆盖盲区 |
| d | **`bindings:check`（含 `git diff --exit-code`）接进 `check`** | 现状 `check` 走就地 `bindings:export`，生成物永远最新 → tsc 从不报漂移 |
| e | Rust 导出真实 JSONL fixture 供前端消费 | 防「落库形态 vs DTO 镜像」漂移 |

**② 与 ③ 不是二选一**：② 给结构，③ 给护栏。单做 ② 依赖开发者记忆改 3 处；
单做 ③ 仍在 2 处重复判定上打补丁。

## 已证伪的分支（留档，避免重走）

**路线 ①「折叠路 = 跑一遍 reducer」** —— 曾设想用 `readUIMessageStream` 让
`eventsToUIMessages` 复用同一 reducer，使同构成为恒等式。**双重否决**：

1. `readUIMessageStream` 是**单消息语义**（"each stream part is a different state of the
   same message"），列表重建仍要手写按 `start` 切分；
2. V2 证明 reducer 缺跨消息 tool 配对能力 —— 复用即**把折叠路降级**，直接违反 spec
   「工具同 id 配对收口于适配层」requirement。

即「消除双实现」的代价是丢失能力，不可接受。

## 遗留隐患

- **`auxEventData` 两路共用是巧合**：因 ai-sdk 的 data part 与 data chunk 碰巧同形。
  建议不改（写两遍更脆），但在 spec 记一句「依赖同形约定，ai-sdk v8 升级须复核」。
- 半截文本窗口期（中途接入首个 delta 无前文）V6 只验证「能否归约」，未验证视觉连续性。
- 轮内 lagged（broadcast 512 溢出）时的重挂时序：重放快照与续订之间的事件窗口如何补。

## 决策点

1. **组件形态**：纯只读 + composer slot，还是全功能（含发送/停止）？explore 与 debug 差异
   （stance 拼接 / 每跑重置 / source）如何外置。
2. **通道形态**：事件直投（`Channel<AgentRunMessage>` 复用信封）vs kind-only 通知 + 重查
   ——delta 决定了**必须直投载荷**（通知无法承载 ephemeral）。
3. **注册表归属**：per-session_id 的新注册表（change 广播是 per-(root, change_id)，
   一次 run 含多会话，不能复用）；exec / change 两轨共写点 = `kernel drive` 回调。
4. **与既有基建关系**：`useAgentChat` / `useSessionTranscript` 合一进组件，还是组件消费其一 +
   补流？`refreshKey` 去抖逻辑是否内化。
5. **spec 修订面**：`desktop-agent-chat-infra`（组件族增「会话级组件」；Module Contract 中
   `reconnectToStream` 返回 null 的表述须改；「两路同构」降级为「终态同构 + 过程近似」）；
   `desktop-agent-execution`（增补 watch / 广播 requirement）。
6. **change run 面是否迁移**（`session-transcript-panel` 收编进统一组件）还是保持零触点
   （代码内有「零触点红线」注释在案）。

## 关键文件索引

- `packages/desktop/src/lib/agent-adapter.ts` — **IR 化主战场**（`blocksToParts`:212 /
  `blocksToChunks`:309 / `auxEventData`:144 / `SealedAgentEvent`:86 / `eventToChunk`:349）
- `packages/desktop/src/lib/agent-adapter.test.ts` — 732 行，对拍在 :536
- `packages/desktop/src/lib/agent-transport.ts` — `reconnectToStream`:243（改挂点）、
  `openProvisionalChunks`:94（订阅流须复刻的簿记）
- `packages/desktop/src/hooks/use-agent-chat.ts` — `onFinish` 收敛闸 :342、`running` :363
- `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` — 库态 running :44
- `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` — 「进行中才订阅」先例
- `packages/desktop/src/components/agent/agent-blocks.tsx` — `ToolPairCard` props = IR 佐证
- `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` — `drive`:191、delta skip:229、
  `StopRegistry::register`:28（无重入守卫）
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` — `agent_start_with`:103-116（tee 挂点）
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` — `RunUpdate::SessionEvent`
  :277 带载荷 / `RunNotice` :324 kind-only 降位
- `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` — `UPDATE_CAPACITY`:13
- spec：`desktop-agent-chat-infra`、`desktop-agent-execution`、`desktop-change-flow-view`、
  `desktop-ipc-type-bindings`
