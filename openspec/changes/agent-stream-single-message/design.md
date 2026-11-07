# 设计: agent-stream-single-message

> **变更**: agent-stream-single-message
> **日期**: 2026-11-06
> **基于**: `proposal.md`（1–8 提案条款、变更范围、AC-1..AC-13、R-1..R-11）、`explore.md`（只读）与两份 delta spec（`desktop-agent-chat-infra` 5 requirement / 41 scenario；`desktop-agent-execution` 3 requirement / 19 scenario）
> **性质**: 纯前端实时流语义重构——实时发射层从「一事件一消息」改为「一轮一条 assistant 消息 + step 分段」，折叠层从「一事件一消息 + 轮行独立消息」改为「按轮折叠 + 回合元数据」。**Rust 后端零改动、生成物零改动**。
>
> **本篇边界**（改动 5 个文件，全部在前端 TS 面）：
> `packages/desktop/src/lib/agent-adapter.ts`、`packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/hooks/use-agent-session.ts`、`packages/desktop/src/components/agent/agent-messages.tsx`、`packages/desktop/package.json`（版本一行）。
> **零 diff 面**：`packages/desktop/src-tauri/**`（含 `crates/**` 与 `src/commands/**`）、`src/types/generated/{bindings.ts,bindings.zod.ts}`、`src/types/dto.ts`、`src/components/agent/{agent-error,agent-blocks,agent-input,tool-cards,session-panel,agent-session-view}.tsx|ts`、`src/views/**`、`openspec/specs/**`、`openspec/config.json`、本 change 的 `explore.md`。
> **零新增文件、零新增依赖、零新增 IPC 命令。**

---

## 提案与规格同步状态

proposal 的八条提案条款（总决策 D / step 分段 / 提问侧推 / record 归 metadata / fold 按轮 / id 锚点 / 异常呈现 / 删除面）与 explore 的 Q1–Q5 裁定已定稿，两份 delta spec 已定稿。本设计不重述论证，只做六件事：

1. 把 spec 已钉语义、未钉落点的三处收敛到具体形态：**step 帧的精确触发条件**（含「无 delta 的响应」这一 spec 未点名的分支，D2）、**`eventToChunk` 生成 delta part id 所需的簿记接口**（spec Module Contract 表只列两参，D4 / 待决问题 6）、**轮内消息序**（aux 在前、assistant 恒为该轮末条，D7——这是收口不重挂不重挂的硬前提）。
2. 把 proposal「变更范围」的计数 / 命名 / 落点口径按工作树实测纠正并留痕（`8 个手写镜像` 实为 4 个 interface、`eventToChunk` 需扩第三参、过滤行号区间、`describeAgentError` 不可复用）——见「事实偏差」四则。
3. 给出 `foldSessionMessages` 的**可执行伪码**（分组 / 行匹配 / 三序输出 / 部件聚合 / TurnDone 兜底），使「不依赖 `RunStarted` 计序」「空内容轮恒一条消息」「旧转录顺序回退」三条 AC 一一可落。
4. 给出实时路 `envelopeStream` 的状态机（`messageStarted` / `stepOpen` / `partAnchors` / `currentToolCalls`）与每个信封的 chunk 发射表，使「一次响应一次 `start-step`」「密封前 `reset-step` 且前段存活」「`Record` → `finish(messageMetadata)`」逐条可复核。
5. 逐条 AC 给落点（「验收标准回应」表），**不写测试策略**（用例编写与门禁执行由独立相位承接）。
6. 把四处必须显式留痕的边界写成可实现形态：`parentToolUseId` 在「一轮一条消息」下只能表达消息级单值（D14 / 待决问题 2）、实时路 `step-start` 标记部件在消息内残留（待决问题 3）、`TurnDone` 兜底以合成轮行落地（D9 / 待决问题 4）、订阅流「就地续写」的中途态（待决问题 8）。

### 起点实况核实（工作树实测；路径前缀 `packages/desktop/`，锚点为改动前行号）

- **适配层现状**：`src/lib/agent-adapter.ts`（635 行）——`AgentMessageMetadata`（`:12-16`，两字段）、四份手写事件载荷镜像（`AgentRunStartedData` `:19-26` / `AgentSystemNoticeData` `:28-33` / `AgentRunResultData` `:35-45` / `AgentRawData` `:47-52`）、`AgentToolOutput`（`:56-60`，含 `search` 加位）、`AgentDataParts`（`:64-70`，五成员含 `'run-record'`）、`AgentUIMessage` / `AgentUIMessageChunk`（`:73` / `:76`）、`AgentToolPart`（`:79-92`）、`SealedAgentEvent`（`:95`）、`AuxData`（`:98-102`）、`BlockPlan`（`:116-125`）、`PairingScope.{global,local}`（`:163-186`）、`messageId(seq) => evt-<seq>`（`:195-197`）、`provisionalMessageId`（`:203-205`）、`deltaPartId(key, part)`（`:211-213`）、`auxEventData`（`:216-264`，含 `const _exhaustive: never`）、`planBlock`（`:275-295`，含 `_exhaustive`）、`plansToParts`（`:431-449`）、`uiRole`（`:454-457`）、`eventToUIMessage`（`:460-474`）、`eventsToUIMessages`（`:476-481`）、`runRecordToUIMessage`（`:487-494`）、`textChunks` / `reasoningChunks` / `outputChunk`（`:498-527`）、`plansToChunks`（`:567-587`）、`eventToChunk`（`:590-635`，内部发 `start` + `reset-step` 事件头）。
- **传输层现状**：`src/lib/agent-transport.ts`（344 行）——`deltaPartKind`（`:88-90`）、`partStartChunk`（`:93-97`）、`openProvisionalChunks`（`:101-114`，provisional `start` + `reset-step` + part-start）、`EnvelopeStream`（`:139-148`）、`TauriAgentTransport.sendMessages`（`:165-182`）/ `reconnectToStream`（`:189-214`）、`envelopeStream`（`:220-268`，持有 `openedParts: Map<string, Set<'text'|'thinking'>>` 与 `currentToolCalls`）、`eventChunks`（`:271-310`，以「是否含 `start` chunk」判定是否换消息并重武装簿记）、`enqueueRecord`（`:313-325`，`start` + `reset-step` + `data-run-record` + `finish`）。
- **hook 现状**：`src/hooks/use-agent-session.ts`（543 行）——`buildMessages`（`:74-89`，以 `TurnDone` 出现次数索引 `turns`，尾余补到列表末尾）、`SessionMirrors`（`:113-131`）、`useSessionMirrors`（`:201-252`，`handleEvent` 于 `:227-233`，delta 只推游标不入镜像）、`runReplayLoad`（`:300-319`，成功走 `applyReplay`、失败回落 `buildMessages`）、`useSessionActions.sendMessage`（`:453-476`，`send(undefined, { body })`）、`onFinish` 收口（`:513-516`）。
- **渲染层现状**：`src/components/agent/agent-messages.tsx`（267 行）——`PartView`（`:72-77`，`data-*` 部件返回 `null`）、`ChatBubbleBase`（`:80-103`）、`useHistoryWindow`（`:197-230`，过滤在 `:213-217`：`role !== 'system' && parts.length > 0`）、`AgentMessages`（`:232-267`）。
- **失败横幅现状**：`src/components/agent/agent-error.ts`——`latestAgentFailure`（`:118-145`），判据 `latestTurn.status === 'failed'`（`:128-131` 区间），`describeAgentError` / `ERROR_RULES` 为**模块私有**（`:12-77`）。
- **会话级入口现状**：`src/components/agent/agent-session-view.tsx`——`AgentFailureBanner`（`:77-101`）、`latestAgentFailure(state.events, state.chain)`（`:121-124`）、`<AgentMessages messages={state.messages} …/>`（`:151`）；入参与插槽形状本次不动。
- **出线 / 信封现状**：`src/types/generated/bindings.ts:401-420` —— `AgentEvent = { turnId?: number | null, seq, timestampMs } & AgentEventKind`（前置 change 已合入）；`:562-566` `AgentRunMessage = { ipc:'event', event } | { ipc:'record', record: TurnSummary }`；`:1960-1990` `TurnSummary`（`status: AgentRunStatus`，四值含 `'stopped'`，`error: string | null`）。
- **引擎侧事实（只读核对，零改动）**：`crates/infra/agent/src/sdk/loop.rs:152-170` 发 `RunStarted`、`:171-186` 发 `role = User` 的密封提问 `Message`、`:378-390` 每次成功响应恰发一条 `role = Assistant` 的密封 `Message`、`:391-403` 工具轮；`crates/core/agent/src/kernel.rs:247-249` 唯一盖戳点（`AgentEvent::stamp(Some(turn_id), seq, kind)`）、`:305-320` 停止 → 收敛 `Stopped`、`:333-370` `converge` 铸终态行并经 `streams.finish` 流出 `Record`。→ 「一次运行 = 一整轮（提问 + 思考 + 工具 + 回答）」「停止轮不发 `TurnDone`」「零事件轮仍有轮行」三条前提在本期成立。
- **ai-sdk 事实（`node_modules/ai@7.0.118`，逐行核对）**：`createStreamingUIMessageState`（`dist/index.js:7469-7484`，role 硬编码 `assistant`）；`start` 只改 `state.message.id` 并合并 `messageMetadata`（`:8060-8068`）；`start-step` 只 push `{type:'step-start'}`（`:8039-8041`）；`reset-step` 只删「最后一个 `step-start` 之后」的部件（`:8046-8058`）；`finish` 合并 `messageMetadata`（`:8070-8078`）；`updateMessageMetadata` 走 `mergeObjects`（`:7642-7658`，深合并、非累加，`null` 会覆盖）；`getToolInvocation` 先查当前 step、再**全消息倒序**兜底（`:7509-7532`）；`error` chunk 直接 `onError → throw`（`:8088`，chat 侧 `onError: (error) => { throw error; }`，`:22406`）；`AbstractChat.makeRequest` 的 `write()` 在 `state.message.id === lastMessage?.id` 时**替换末条**、否则 **push 新条**（`:22382-22393`）；`sendMessage({ id?, parts })` 先 `pushMessage`（显式 `id` 生效）再 `makeRequest`（`:22030-22070`）；`reconnectToStream` 走 `state.message.id === lastMessage?.id` 的同一判定（`isContinuation`，`:8208`）。
- **版本现状**：`package.json:3` `version = "0.6.19"`；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行 `0.1.0` **不跟**。

### 事实偏差（已在本文档裁定）

1. **「8 个手写事件镜像」实为 4 个 interface**：proposal 第八节与「变更范围」写 `agent-adapter.ts:12-70` 的 8 个手写镜像。实测该区间为 **4 个 interface**（`AgentRunStartedData` / `AgentSystemNoticeData` / `AgentRunResultData` / `AgentRawData`）+ 由它们组成的 `AuxData` 联合（`:98-102`）+ `AgentDataParts` 成员表（`:64-70`）。落地口径：**4 个 interface 改为 `Omit<Extract<AgentEvent, { kind: K }>, 'kind' | 'turnId'>` 派生**，`AuxData` / `AgentDataParts` 随之自动收口（不再手写字段清单）；`AgentToolOutput`（`:56-60`）**不属事件镜像**，保持手写（其形状不由 `AgentEvent` 决定）。结论面不变（信封加变体 / 加字段即编译报错），只是计数与落点以实测为准。
2. **`eventToChunk` 需扩第三参**：spec Module Contract 表列 `(event, currentMessageToolCalls?)`，但 spec Scenario「delta 路映射」要求「part id 由该 step 内首个同类 delta 的 `seq` 派生且后续 delta 复用」——该判定需要「本 step 已见过的首个同类 `seq`」这一输入，纯函数无法自行持有。裁定：追加可选第三参 `deltaAnchor?: DeltaPartAnchor`（缺省 = 恒用本事件 `seq`），由 transport 传入持簿记的解析器（D4）。表里前两参形态与语义逐字保留（登记见待决问题 6，归档合入时补该行说明）。
3. **`agent-messages.tsx` 过滤行号**：proposal 写 `:215`，实测 `const conversationMessages = allMessages.filter(… parts.length > 0)` 在 `:213-217`（判定行 `:216`）。以区间计，结论不变。
4. **`agent-error.ts` 的 `describeAgentError` 不可复用（口径收窄）**：spec ADDED requirement 要求状态行「友好错误标题」，而 `ERROR_RULES` / `describeAgentError` 为 `agent-error.ts` 模块私有，且 AC-13 钉住该文件**零 diff**。裁定：本期状态行取**通用标题**（`failed` → 「Agent 运行失败」；`stopped` → 「已停止」）+ 可展开原文（`metadata.record.error` 与该轮 `api_error` 通知）；不复制 `ERROR_RULES`（两份规则必漂移）。登记见待决问题 1。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 实时路的消息粒度 | 每轮**一条** assistant 消息：`start` 在**该轮首个信封事件**处发且只发一次（`messageId = turn-<turnId>`）；部件（思考 / 正文 / 工具卡）不限条数，按首次到达顺序 push | ai-sdk 的 reducer 本就「一条流 = 一条消息」（`createStreamingUIMessageState` 单数 `state.message`）；`UIMessageChunk` 无 role 字段 ⇒ 多消息只能靠多次请求，而我们一次运行推完整轮。故把「多消息」从消息维度搬到**部件维度**，`start` 单点 = 气泡单点（AC-1）。首个事件发 `start`（即使是 `RunStarted` 或提问事件）保证「零内容轮 / 停止轮」也有一条可挂 `metadata.record` 的 assistant 消息（AC-9 / spec Scenario「空内容轮恒产一条消息」） |
| D2 | step 的判据 | step = 一次模型响应；`start-step` 由 transport 在「新响应的第一个可渲染动作」处发：①该响应有 delta → 首 delta 前；②该响应无 delta（直接来密封 `Message`）→ 密封到达前。`partAnchors` 与 `stepOpen` 同时重置 | `loop.rs:378-390` 保证每次成功响应恰一条密封 assistant `Message`，故「响应 = 一次 `start-step`」两端可数、后端不传 `step_index`。补 ② 是 spec 未点名但**必须**的分支：连续两条密封 `Message` 之间若无 delta，不发 `start-step` 则 `reset-step` 会按「最后一个 `step-start` 之后」把上一段的密封块一并清掉（`index.js:8046-8058`），前段丢部件 |
| D3 | `reset-step` 的触发位置 | **每条密封 assistant `Message` 追加前恒发一次 `reset-step`**（D2 的 ② 先发 `start-step` 再发 `reset-step`，后者此时为空操作）：顺序恒为〔必要时 `start-step`〕→ `reset-step` → 追加整块部件 | ①满足 spec「密封到达时先 `reset-step` 再追加」字面；②`reset-step` 只清当前段内的临时部件，前面 step 的密封块在其之前的 `step-start` 之外，不受波及（AC-3）；③空操作的那一发让「密封前必有 `reset-step`」成为无分支的机械不变式（测试可按位置断言，不必按响应类别分叉） |
| D4 | delta part id 的派生与簿记 | `deltaPartId(seq: number, part: 'text' \| 'thinking') => 't-<seq>' / 'r-<seq>'`；`seq` = **该 step 内首个同类 delta 的 `seq`**。簿记 `partAnchors: Map<'text' \| 'thinking', number>` 归 transport（作用域 = 当前 step，`start-step` 时清空）；`eventToChunk` 通过第三参 `deltaAnchor: (kind, seq) => number` 取得锚值，自身不持状态 | 同一 delta 永远算出同一 id ⇒ 订阅流从任意 `since_seq` 起步也能续上（AC-4）；作用域缩到 step 是 explore T6「删簿记」的落地修订（Q3）：按 `parentToolUseId` 的**跨消息**开件簿记退役，**per-step 最小簿记保留**，否则同一 part 的后续 delta 会算出新 id 并抛 `missing text part`（R-2）。锚解析以**回调**注入而非可变 Map 入参，使 `eventToChunk` 自身保持无状态（适配层「纯函数」纪律不破） |
| D5 | 提问消息的双起点 | transport **对 `role = user` 的密封事件零 chunk**；提问由会话状态侧产生：**发起流**在 `sendMessage` 时刻以显式 `id`（`pending-user-<n>`）本地推入（`pushMessage` 在 `await makeRequest` 之前同步执行 ⇒ 结构上早于任何 chunk），该轮提问事件到达时把该消息 id **归一**为 `turn-<turnId>-user`（不新增消息）；**订阅流**（无本地副本）在提问事件到达时以 `turn-<turnId>-user` 追加（若该轮 assistant 消息已在列表中则插到其**之前**）。两路写消息状态一律用 `setMessages` 的**函数式更新**（`useChat` 的 `setMessages` 支持 updater，内部读 `chat.messages` 现值）——流式期间 ref 快照可能滞后，非函数式会覆盖掉在飞的 assistant 消息。订阅流的提问部件为 text 直映射（提问块恒为单个 text 块；适配层的 `planBlocks` / `plansToParts` 不导出、也不为此开新导出），收口后由 fold 整表替换归一。`turnId` 为 null 时兜底 `evt-<seq>` | chunk 协议无 role 字段（`index.d.ts:2477-2615`），硬造必产第二条 assistant 消息——这正是角色漂移的根因（AC-2 的「不靠 seq 猜角色」）。本地推入使「思考跑到提问之前」在**结构上**不可能（提问先于任何 chunk 入列）；订阅流不跳过则旁观方必丢提问气泡（Q2 的两起点分化）。归一用 id 替换而非删旧插新，保证「同一提问恰一条气泡」（R-3） |
| D6 | record 的承载 | `Record` 信封 → `finish` chunk 的 `messageMetadata: { seq, parentToolUseId: null, record }`（该轮消息尚未建立时先以 `turn-<record.turnId>` 发 `start` 再 `finish`），随后关流；**删除** `enqueueRecord` 的独立消息组与 `data-run-record` 部件 | `finish` 的 metadata 走 `mergeObjects` 深合并（`:7642-7658` / `:8070-8078`），`record` 作为新键写入而不累加；`seq` 显式带同 `start` 的值是因为 `null` 会**覆盖**（mergeObjects 只跳过 `undefined`），必须回填同一值以免丢保真。落 metadata 而非 data 部件：渲染侧本就过滤 system 载体，data 部件白跑一趟（explore R2） |
| D7 | fold 的轮内消息序 | 每轮固定三序：**① aux system 消息（按 `seq` 升序）→ ② 提问消息（可缺席）→ ③ assistant 消息（恒一条）**。assistant **恒为该轮末条** | 硬前提：`AbstractChat.makeRequest` 的 `write()` 只在 `state.message.id === lastMessage?.id` 时替换末条、否则 push 新条（`:22382-22393`）。订阅流 / 收口重挂时 fold 已产出 `turn-<turnId>` 消息，若其后还跟着 aux 消息，`start(turn-<turnId>)` 会被判成新消息 → **整轮重挂 + 双份气泡**（R-4 的机械成因）。aux 前置不影响呈现（对话透镜过滤 system）也不影响保真（`AgentRawStream` 读 `state.events`） |
| D8 | 实时路的 aux 事件载体 | 实时路 aux 事件（`runStarted` / `systemNotice` / `turnDone` / `raw`）以**本轮消息内的 data 部件**流入——不为它们另发 `start`（轮内首个事件的那一次 `start` 例外，见 D1）、不为它们开新 step；收口后由 fold 归一为独立 system 消息 | reducer 造的消息 role 恒 `assistant`，实时路**物理上**产不出 system 角色消息——这是既有事实（现状也是把 aux 塞进 assistant 消息，只是每条一个）。另起消息会破「一轮一条气泡」（AC-1），落 data 部件则零视觉差异（`PartView` 对 data 部件返回 `null`）且 `turnDone → data-run-result` 的转录内保真面得以保留（R-9 的反向钉住点） |
| D9 | `TurnDone` 的兜底形态 | fold 的轮行匹配三支：①`turnId != null` 且命中 `turns` 行 → 用该行；②`turnId == null`（旧转录）→ 按组序取 `turns[nullIdx++]`（等价既有行为）；③①②皆无且组内有 `TurnDone` → **合成一条内存轮行**（`turnId = event.turnId ?? -1`、`status = isError ? 'failed' : 'completed'`、`numTurns` / `durationMs` / `costUsd` / `sessionId` 照搬 `TurnDone`、`startedAt` = 该组首事件 `timestampMs`、`finishedAt` = `TurnDone.timestampMs`、`error = null`——`TurnSummary` **无** `usage` 字段，故 `TurnDone.usage` 不参与合成，保真面仍由 `data-run-result` 部件承载）；三支落空 → `metadata.record = null` | spec 要求「`TurnDone` 仅在该轮轮行缺席时兜底 status」，而 status 的唯一读取位被钉为 `metadata.record.status`。合成轮行使该句可落而**不新增 metadata 字段**（不违 spec 的 `AgentMessageMetadata` 形态）。`-1` 只活在内存（不落库、不参与寻址、不作消息 id），登记于待决问题 4。`TurnDone` **绝不**用于轮边界或轮行计数（AC-7） |
| D10 | 折叠入口的分组实现（单套） | 单遍扫描生成组：`turnId != null` 时按 `turnId` 变化切组；`turnId == null` 时遇 `runStarted` 且当前组非空切组。**不写第二套分组函数、不做数据迁移** | AC-7（不依赖 `RunStarted` 计序：有 `turnId` 就走主路径）与 AC-8（旧转录按顺序分组：无 `turnId` 才退化到 `RunStarted` 边界）由同一循环的两个分支承担；`RunStarted` 只在退化面出现，真实运行事件恒有 `turnId`（前置 change 的唯一盖戳点保证） |
| D11 | 输出序与零事件轮的位置 | 输出序 = `turns` 的行序；某行无对应组（零事件轮）→ 在该位置产出「`parts` 空 + `record` 就位」的 assistant 消息；有组但无行（旧转录 / 行缺失）→ 追加在行序之后，按组序 | 轮行是库态权威（`SessionDetail.turns` 按发起序），以其序输出天然修掉「停止轮令后续轮行错位一格」（AC-7）；零事件轮（ask 失败 / 立即停止）恒有轮行，故必然就位（spec Scenario「空内容轮恒产一条消息」）。proposal 的「补尾到列表末尾」缺陷随 `buildMessages` 一并退役 |
| D12 | 部件聚合范围 | 每轮 assistant 消息的 parts = 该轮全部 `role = assistant` **与** `role = tool` 密封 message 事件的 blocks 顺序拼接后，经 `plansToParts(planBlocks(blocks, paired))` 一次产出；`paired` 由 `PairingScope.global(全部密封事件)` 一次预扫得到（与既有折叠路同口径） | 「一轮一条消息」要求正文、工具调用、工具结果同住一条气泡；`global` 视野把跨消息的 `toolUse ↔ toolResult`（含 `pause_turn` 拆段的 `webSearch` use/result）收敛为单个计划（既有语义，零改动）。`role = tool` 事件不再产独立 system 消息，其配对结果就地补全对应工具部件（AC-10 的「视图层零扫描」保持） |
| D13 | 消息 id / `seq` 保真 | 主路径：assistant `turn-<turnId>`、提问 `turn-<turnId>-user`；`turnId` 为 null：`evt-<该消息首事件 seq>`（assistant 与提问各按自身事件）。`metadata.seq`：assistant = 该轮首事件 `seq`（零事件轮为 `null`）、提问 = 该提问事件 `seq`、aux = 该事件 `seq`。实时路 `start` 携带 `{ seq: 首事件 seq, parentToolUseId: null }`，`finish` 回填同值 + `record` | 两路算出同一 id 是 `isContinuation` 成立的前提（AC-5）；`evt-<seq>` 兜底只在旧转录 / 无轮归属构造面出现，收口以 fold 整表替换对齐（一次重挂，用户无感）。`seq` 进消息 metadata 满足「保真字段可考」（spec Scenario「保真字段可考」） |
| D14 | `parentToolUseId` 的承载层级 | 落在**消息级** `metadata.parentToolUseId`（assistant = 该轮首个 assistant 密封事件的 `parentToolUseId`；aux / 提问按自身事件；零事件轮为 `null`）。**不新增部件级字段** | 引擎侧 `parent_tool_use_id` 恒 `None`（`sdk/normalize.rs` 注释 + `loop.rs` 各构造点），今日唯一消费面是「可考」（无渲染判据依赖）；「一轮一条消息」无法在消息级表达轮内多变值，而部件级归因需要给 text / tool chunk 都挂 `providerMetadata`，属超范围。登记为已知限制（待决问题 2） |
| D15 | 空内容轮的可见性判据（单一来源） | `agent-messages.tsx` 内一个私有判定 `conversationMessages` 过滤：①`role === 'system'` 剔除；②**存在可见部件**（`text` / `reasoning` / `tool-*`）→ 保留；③否则读 `message.metadata?.record?.status`：`failed` / `stopped` → 保留（呈状态行），`completed` / `null` → 剔除。**全文件仅此一处判定**，分页 `remaining` 基于过滤后列表 | 修缺陷 3（`parts.length > 0` 吞掉空内容失败 / 停止轮）；用「可见部件」而非 `parts.length` 也顺带剔除「只有 data 部件 / 只有 `step-start`」的空壳（D8 的实时态）。判据只在一处，spec「过滤单一来源」条款可机械复核 |
| D16 | 轮内状态行的落点与形状 | 住在 `agent-messages.tsx`（`ChatBubbleBase` 内，assistant 且无可见部件时渲染）：`failed` → 标题「Agent 运行失败」+ `<details>` 展开原文；`stopped` → 「已停止」行。**原文来源两个且都在手边**：①`metadata.record.error`（库态权威，两路都有）；②**该消息自身的 `data-system-notice` 部件**里 `subtype === 'api_error'` 的 `payload.error`（实时态 aux 落本轮消息内，见 D8；折叠路这些通知已归一到独立 system 消息，故该支在折叠态天然缺席）。两者去重后拼接（空串丢弃），全空则只显标题。`data-testid`：`turn-status-line`（根，带 `data-status`）+ `turn-status-line-failed` / `turn-status-line-stopped` / `turn-status-line-raw`。`parts` 非空时**不叠加**状态行 | 「在发生的位置看到它」（explore 判据：正文是 content，运行事实是 provenance）；不改 `agent-session-view.tsx` / `agent-error.ts`（AC-13 零 diff），故 `stopped` **不会**触发顶部横幅（既有判据正确，Q4 裁定零改动）。**不引入 `events` 入参**：`AgentMessagesProps` 保持三字段（session-view 零 diff），实时态的 api_error 恰好在同一消息的 data 部件里可读 |
| D17 | 退役与死代码纪律 | 删除 `provisionalMessageId` / `eventsToUIMessages` / `runRecordToUIMessage` / `eventToUIMessage` / `openProvisionalChunks` / `enqueueRecord` / `openedParts` / `buildMessages`；`AgentDataParts` 去掉 `'run-record'` 成员；`deltaPartId` 改签名（旧调用点零残留） | AC-6 的机械判据是 `knip` 零未用导出 + 源码零残留；退役面**不做兼容层**（保留双路会让「一事件一消息」的死代码继续被 knip 视为活引用，也违反「surgical changes」）。`turnDone → data-run-result` **维持**（R-9 的反向约束） |
| D18 | 版本与零 diff 边界 | `packages/desktop/package.json` `version` `0.6.19 → 0.6.20`（一次 patch）；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行**不跟**。零 diff 面见本篇边界段 | AGENT.md 版本纪律；AC-11 / AC-13 的可机械复核面（`git diff --stat` 逐项核对） |
| D19 | 编译闭合批 | 适配层删导出（`eventsToUIMessages` / `runRecordToUIMessage` / `provisionalMessageId` / `uiRole`）与 transport / hook / 渲染层的调用点改动**必须同批落地**：拆开即 `tsc` 报错（无中间可编译态）。批末**判据**（由桌面端静态检查相位执行 `vp check` / `knip`）=「零残留引用 + 零未用导出」；本相位只做源码级自查 | 与前置 change 的「编译闭合批」同款纪律；`tasks.md` 的阶段一~四明确标注为同一闭合批，闭合点只在阶段四末 |
| D20 | 明确不做 | ①不为 aux 事件另起消息；②不给 `AgentEvent` 增字段（`step_index` 不存在也不新增）；③不改 `agent-error.ts` 的 `stopped` 判据、不复制 `ERROR_RULES`；④不做旧转录数据迁移 / 不建第二套分组；⑤不为搜索另立收口路径；⑥不动 `eventToChunk` 的 delta 分支是否产搜索块（无搜索实时 delta，既有语义零改） | 逐条对应 proposal「不要修改」与裁定表 Q1–Q5；越界项一律拒 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 适配层状态部件与类型面（**修改**） | `AgentMessageMetadata` 增 `record`；`AgentDataParts` 去 `'run-record'`；4 份手写事件载荷镜像改从生成绑定派生（`Omit<Extract<AgentEvent,{kind:K}>,'kind'\|'turnId'>`）；`deltaPartId(seq, part)`；退役 `provisionalMessageId` / `runRecordToUIMessage` / `data-run-record` | `packages/desktop/src/lib/agent-adapter.ts` | `ai`（`UIMessage` / `UIMessageChunk` / `ProviderMetadata` 类型）、`../types/dto`（`AgentEvent` / `AgentBlock` / `TurnSummary` 等） | 既有 TS 纯函数三层（语义 IR / 视野 / 发射），零新依赖 |
| 折叠入口（**新增函数，替换既有入口**） | `foldSessionMessages(events, turns)`：按 `turnId` 分组（旧转录按 `RunStarted` 边界）、轮行匹配与 `TurnDone` 兜底、三序输出、单条 assistant 消息聚合 parts、`metadata.record` 挂轮行 | 同上 | `PairingScope.global` / `planBlocks` / `plansToParts` / `auxEventData`（既有内部件） | 同上（零 IO / 零 async） |
| 实时块 chunk 单点（**修改**） | `eventToChunk` 收窄为**只产块 chunk**（delta / 密封块 / 工具就地补全）；`role = user` 零 chunk；不再产 `start` / `reset-step`；新增 `deltaAnchor` 第三参 | 同上 | 同上 | 同上 |
| 传输层 step 分段与收口（**修改**） | 每轮一次 `start`（`turn-<turnId>`）、每响应一次 `start-step`、密封前 `reset-step` + 追加整块、per-step part 簿记、工具结果就地补全、`Record` → `finish(messageMetadata)` 后关流；两出口共用同一 `envelopeStream` 单点 | `packages/desktop/src/lib/agent-transport.ts` | `@tauri-apps/api/core`（`Channel`）、`ai`（`ChatTransport` / `UIMessageChunk`）、`../types/generated/bindings`、`./agent-adapter`、`./ipc` | 既有 `ReadableStream` + `Channel.onmessage`（零新任务 / 零新通道） |
| 会话状态基建（**修改体内**） | `buildMessages` 退役 → 三处收口统一走 `foldSessionMessages`；`sendMessage` 以 `{ id, parts }` 本地推提问并在提问事件到达时归一；订阅流按事件追加 / 前置提问；镜像面（`events` / `chain` / `seq` 游标 / 无进展闸）零语义变更 | `packages/desktop/src/hooks/use-agent-session.ts` | `@ai-sdk/react`（`useChat`）、`../lib/agent-adapter`、`../lib/agent-transport`、`../lib/ipc` | 既有 React hooks（`useChat` 状态层 + ref 镜像双轨） |
| 对话透镜（**修改**） | 可见性判据收为单一来源（system 过滤 + 可见部件 + 轮状态）；空内容 `failed` / `stopped` 轮渲染轮内状态行；`stopped` 不叠加横幅（横幅面不动） | `packages/desktop/src/components/agent/agent-messages.tsx` | `./agent-blocks`（`TextBlock`/`ThinkingBlock`/`ToolPairCard`/`isToolPart`）、`./tool-cards`（`DEFAULT_TOOL_CARDS`）、`@/lib/agent-adapter`（类型） | 既有 React + shadcn `message-scroller`（零新组件库） |
| 版本行（**修改**） | `version` `0.6.19 → 0.6.20` | `packages/desktop/package.json` | — | — |

<!-- 无新增 crate / 无新增模块目录 / 无新增文件：本变更全部落在既有文件与其内部函数上（轮内状态行住在 agent-messages.tsx，不新开文件）。 -->

---

## 变更清单

> 路径口径：仓库根相对全路径。测试文件与用例归测试相位（proposal「测试文件」/「新增用例」两张表），**不在本清单**。

### 新增文件

**无。** 本变更是既有四文件内部机制的重构：折叠入口是 `agent-adapter.ts` 内一个函数替换、step 分段是 `agent-transport.ts` 内一个状态机、提问侧推是 `use-agent-session.ts` 内两条路径、轮内状态行是 `agent-messages.tsx` 内一个私有组件。新开文件会把「一次语义重构」摊成跨模块跳转，与「surgical changes / 不为单次使用造抽象」相悖。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src/lib/agent-adapter.ts` | ① `AgentMessageMetadata`（`:12-16`）增第三位 `record?: TurnSummary \| null`（doc：回合元数据载体，取代 `data-run-record` 部件）；② 四份手写镜像（`:19-52`）改为派生别名 `type EventPayload<K extends AgentEvent['kind']> = Omit<Extract<AgentEvent, { kind: K }>, 'kind' \| 'turnId'>` + `type AgentRunStartedData = EventPayload<'runStarted'>` 等四行（`AuxData` `:98-102` 与 `AgentDataParts` `:64-70` 随之自动收口）；③ `AgentDataParts` 删 `'run-record': TurnSummary` 成员（`:69`）；④ `deltaPartId`（`:211-213`）签名改 `(seq: number, part: 'text' \| 'thinking') => string`，返回 `t-<seq>` / `r-<seq>`；⑤ 删 `provisionalMessageId`（`:203-205`）；⑥ 删 `eventsToUIMessages`（`:476-481`）/ `eventToUIMessage`（`:460-474`）/ `runRecordToUIMessage`（`:487-494`）与随之失去唯一消费者的 `uiRole`（`:454-457`，`tool → system` 的角色映射只服务于「一事件一消息」；`turnDone` 等 aux 的 system 角色由折叠路直接构造），连带从 `../types/dto` 的类型 import 中移除 `AgentMessageRole`（`noUnusedLocals` 会报未用导入），新增 `foldSessionMessages`（见公共函数表，伪码见「数据模型 - 折叠算法」）；⑦ `eventToChunk`（`:589-635`）收窄：删事件头（`:611-614` 的 `start` + `reset-step`）与 aux 分支的复用逻辑，`role==='user'` 的 `Message` 返回 `[]`，delta 分支改 `deltaPartId(deltaAnchor(kind, event.seq), kind)`，密封 message 分支保留 `mergeableResults` / `isConsumedResult` 的就地补全语义与 `plansToChunks(plans, event.seq)`；新增可选第三参 `deltaAnchor` | AC-1 / AC-4 / AC-6 / AC-8；`PairingScope`（`:163-186`）/ `planBlock`（`:275-295`，含 `_exhaustive`）/ `plansToParts` / `textChunks` / `reasoningChunks` / `outputChunk` / `plansToChunks` / `citationMetadata` / `auxEventData` / `AgentToolOutput` / `AgentToolPart` / `BlockPlan` **逐字不动**（搜索与引用的两态语义、保真视野差异、语义层单点全部保留） |
| `packages/desktop/src/lib/agent-transport.ts` | ① 删 `openProvisionalChunks`（`:101-114`）与 `partStartChunk` 的 `deltaPartId(key, …)` 形态（改按锚 seq 派生）；② `envelopeStream`（`:220-268`）内状态改为 `{ messageStarted, messageId, startSeq, stepOpen, partAnchors: Map<'text'\|'thinking', number>, currentToolCalls: Set<string> }`；③ `eventChunks`（`:271-310`）改为「帧 + 块」两段：`ensureStart`（首个信封事件发一次 `start`）、delta 处按 D2 发 `start-step` 与 part-start、密封 assistant 处按 D3 发〔`start-step`（仅当 `stepOpen === false`）〕+ `reset-step` + 块、`role==='tool'` 只发块（就地补全不开 step）、aux 只发 data 部件；④ `currentToolCalls` 改为**本轮累积**（不再在换消息时清空）；⑤ `enqueueRecord`（`:313-325`）改为 `finish` 携 `messageMetadata: { seq: startSeq, parentToolUseId: null, record }`（无消息时先 `start`）后关流；⑥ `sendMessages` / `reconnectToStream` 签名与 body 参数面**不变** | AC-1 / AC-2 / AC-3 / AC-4 / AC-6；`readSessionParams` / `readStreamParams` / `readPermissionMode` / `readAgentId` / `readNullableString` / `invokeStart` / 状态门（`detail.turns.some(running)`）**逐字不动** |
| `packages/desktop/src/hooks/use-agent-session.ts` | ① 删 `buildMessages`（`:74-89`）；三处调用点（`applyReplay` `:281`、`runReplayLoad` 的 catch `:316`、`onFinish` `:515`）统一改 `foldSessionMessages(events, turns)`；② `SessionMirrors` / `useSessionMirrors` 增 `pendingQuestionIdRef` 与私有 `applyUserQuestion(event)`（由 `handleEvent` 在密封事件入镜像后分派）：提问事件（`kind==='message' && role==='user'`）到达时算 id（`turn-<turnId>-user` / `evt-<seq>`），若本地 pending 存在则**原地换 id**（消息数不增）、否则插到该轮 assistant 消息之前 / 追加末尾；两路写消息一律走 `setMessages` 函数式 updater；③ `sendMessage`（`:453-476`）改 `send({ id: pendingId, parts: [{ type: 'text', text: input.prompt }] }, { body })`（body 面逐字不变），并登记 `pendingQuestionRef`；④ `useSessionMirrors` 增 `setMessagesRef` 入参（`onFinish` 闭包已有同款 ref 手法） | AC-2 / AC-5 / AC-7；`useSessionReplay` / `useSessionStreamSubscription` / `seqCursorRef` / `recordCountRef` / 无进展闸 `blockedKey` / `upsertTurnWrites` / `chainWrites` / `sessionFromTurn` / `withTurns` / `stop` **逐字不动**（镜像面零语义变更） |
| `packages/desktop/src/components/agent/agent-messages.tsx` | ① `useHistoryWindow` 过滤判据（`:213-217`）改为 D15 的单点判定（system 剔除 → 可见部件 → 按 `metadata.record.status` 判空内容轮），`remaining` 基于过滤后列表；② 新增私有 `hasVisibleParts(message)` 与 `turnStatusOf(message)`（读 `metadata.record`），以及私有 `TurnStatusLine({ message, record })`（`failed` → 通用标题 + `<details>` 原文，原文取 `record.error` + 该消息内 `api_error` 通知部件的 `payload.error`；`stopped` → 「已停止」；`data-testid` 见 D16）；③ `ChatBubbleBase`（`:80-103`）assistant 且无可见部件时渲染 `TurnStatusLine` 取代部件列表；④ `PartView` 增 `step-start` 等非呈现部件返回 `null`（现已在 default 分支返回 `null`，仅补注释） | AC-9 / AC-10；`ToolPartView` / `citationsOfPart` / `MessageContainer` / `useTopReachObserver` / `HistoryLoadStatus` / `AgentMessages` 入参与运行标记面**逐字不动** |
| `packages/desktop/package.json` | `version` `0.6.19 → 0.6.20`（一次 patch）；`dependencies` / `devDependencies` **零变化** | AC-12 / D18；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随（零手改），`src-tauri/Cargo.toml` crate 版本行**不跟** |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `foldSessionMessages` | `packages/desktop/src/lib/agent-adapter.ts` | 新增（取代 `eventsToUIMessages`） | `(events: AgentEvent[], turns: TurnSummary[]) => AgentUIMessage[]` | 重放 / 收口折叠唯一入口：只消费密封事件（delta 不进重放路径）；按 `turnId` 分组、旧转录按 `RunStarted` 边界；每轮三序输出（aux → 提问 → assistant）、assistant 恒一条且 parts 可空、`metadata.record` 挂该轮轮行；零 `RunStarted` 计数逻辑 |
| `deltaPartId` | 同上 | 修改（签名） | `(seq: number, part: 'text' \| 'thinking') => string` | `seq` = 该 step 内首个同类 delta 的 `seq`；返回 `t-<seq>` / `r-<seq>`；同一 part 后续 delta 复用，跨 step 不撞（`start-step` 时簿记清空） |
| `DeltaPartAnchor`（type） | 同上 | 新增 | `(kind: 'text' \| 'thinking', seq: number) => number` | delta part 锚解析回调：transport 传入持 `partAnchors` 的实现（首个同类 delta 记锚、后续复用），缺省实现返回入参 `seq` |
| `eventToChunk` | 同上 | 修改（收窄职责 + 扩第三参） | `(event: AgentEvent, currentMessageToolCalls?: ReadonlySet<string>, deltaAnchor?: DeltaPartAnchor) => AgentUIMessageChunk[]` | 只产**块 chunk**：delta（`text-delta` / `reasoning-delta`）、密封块（`plansToChunks`）、工具就地补全（`tool-output-available`）。**不产** `start` / `start-step` / `reset-step` / `finish`；`role = user` 的密封事件返回 `[]`；aux 事件返回单条 data 部件 chunk |
| `provisionalMessageId` | 同上 | **退役** | — | 由 `turn-<turnId>` + per-step `deltaPartId(seq, part)` 取代（proposal 删除面） |
| `eventsToUIMessages` | 同上 | **退役** | — | 由 `foldSessionMessages` 取代（`knip` 零未用导出） |
| `runRecordToUIMessage` | 同上 | **退役** | — | 由 `metadata.record` 承载轮行（`data-run-record` 部件一并退役） |
| `TauriAgentTransport.sendMessages` | `packages/desktop/src/lib/agent-transport.ts` | 修改（体内；签名不变） | `(options: { trigger: 'submit-message' \| 'regenerate-message'; chatId: string; messageId: string \| undefined; messages: AgentUIMessage[]; abortSignal: AbortSignal \| undefined; body?: object }) => Promise<ReadableStream<AgentUIMessageChunk>>` | 发起流：Channel 先于 invoke 建立；`trigger !== 'submit-message'` 仍抛错；会话参数面逐字不变 |
| `TauriAgentTransport.reconnectToStream` | 同上 | 修改（体内；签名不变） | `(options: { chatId: string; abortSignal?: AbortSignal; body?: object }) => Promise<ReadableStream<AgentUIMessageChunk> \| null>` | 订阅流：参数解析与状态门（无 running 轮行 → `null`）不变；流内 step 分段与发起流共用 `envelopeStream` 单点 |
| `useAgentSession` | `packages/desktop/src/hooks/use-agent-session.ts` | 修改（体内；签名不变） | `(params: UseAgentSessionParams) => UseAgentSessionState` | 三处收口改 `foldSessionMessages`；`UseAgentSessionState` 形状不变（`messages` / `events` / `session` / `chain` / `loading` / `running` / `error`） |
| `UseAgentSessionState.sendMessage` | 同上 | 修改（行为；签名不变） | `(input: AgentSessionSendInput) => void` | 以 `{ id: pendingId, parts }` 本地推提问（同步先于任何 chunk），该轮提问事件到达时把 id 归一为 `turn-<turnId>-user`；`body` 会话参数面逐字不变 |
| `AgentMessages` | `packages/desktop/src/components/agent/agent-messages.tsx` | 修改（体内；入参不变） | `(props: AgentMessagesProps) => React.JSX.Element` | `AgentMessagesProps` 三字段（`messages` / `loading?` / `running?`）不变；可见性判据与轮内状态行在此收口 |

<!-- 私有实现件（`foldSessionMessages` 的内部分组 / 行匹配 / 合成轮行、transport 的 `ensureStart` / `frameChunks` / `partStart` 帧发射、`agent-messages.tsx` 的 `hasVisibleParts` / `turnStatusOf` / `TurnStatusLine`）按纪律不列本表，落点见「修改文件」与「数据模型 - 折叠算法」。 -->
<!-- 无 CLI 子命令：本变更不含命令行入口。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `AgentMessageMetadata`（interface） | `packages/desktop/src/lib/agent-adapter.ts` | 修改 | 增 `record?: TurnSummary \| null`（回合元数据：status / error / 统计）。`seq: number \| null` 与 `parentToolUseId: string \| null` 形态不变（D13 / D14） |
| `EventPayload<K>`（type alias，模块私有） | 同上 | 新增 | `Omit<Extract<AgentEvent, { kind: K }>, 'kind' \| 'turnId'>`——事件载荷镜像的唯一派生器（`kind` 不进 data 部件、`turnId` 由消息级承载） |
| `AgentRunStartedData` / `AgentSystemNoticeData` / `AgentRunResultData` / `AgentRawData`（type alias） | 同上 | 修改（interface → 派生别名） | 形态 = `EventPayload<'runStarted'>` / `'systemNotice'` / `'turnDone'` / `'raw'`；信封加变体或加字段时由 `Extract` / `Omit` 编译期强制收敛（AC-6 的守卫） |
| `AgentDataParts`（type alias） | 同上 | 修改 | 删 `'run-record': TurnSummary` 成员；余四成员（`run-started` / `system-notice` / `run-result` / `raw`）载荷随派生别名自动收口 |
| `AuxData`（type alias，模块私有） | 同上 | 修改 | 四成员载荷类型随派生别名收口；成员集合不变 |
| `AgentUIMessage` / `AgentUIMessageChunk`（type alias） | 同上 | 修改（成员随动，形态不变） | 仍为 `UIMessage<AgentMessageMetadata, AgentDataParts>` / `UIMessageChunk<…>`；可承载 `metadata.record`、不可承载 `data-run-record` |
| `DeltaPartAnchor`（type alias） | 同上 | 新增 | 见公共函数表 |
| `AgentToolOutput` / `AgentToolPart` / `BlockPlan` / `PairingScope` / `PairedOutcome` | 同上 | **零改动**（登记防误改） | 搜索两态、引用 metadata、视野差异、工具卡载荷形状全部不变（AC-10 的反向钉住面） |
| `TurnSummary`（生成物） | `packages/desktop/src/types/generated/bindings.ts` | **零改动** | 只被消费为 `metadata.record` 的类型；本期不增字段、不重导出 |

### 配置

<!-- 无配置项：本变更零 KNOWN_KEYS 增删、零 TestFramework 变体、零 openspec/config.json 改动、零 crates/core/config/config.schema.json 镜像改动。package.json 的 version 是版本纪律项，已登记在「修改文件」表。 -->

---

## 数据模型

### 对话消息（`AgentUIMessage`，纯前端内存态）

| 字段 | 类型 | 值域 / 形态 | 说明 |
|------|------|-------------|------|
| `id` | `string` | `turn-<turnId>`（assistant）/ `turn-<turnId>-user`（提问）/ `evt-<seq>`（aux、旧转录兜底） | 两路同源锚点：实时路 `start` 的 `messageId` 与折叠路算出同一值（AC-5）；`turn-` 族与既有 `turn-${record.turnId}` 同族命名 |
| `role` | `'user' \| 'assistant' \| 'system'` | assistant 恒一轮一条；user 为该轮提问；system 为 aux 载体 | 实时路只产 `assistant`（reducer 硬编码），user 由会话状态侧推入（D5），system 只在折叠路出现（D8） |
| `metadata.seq` | `number \| null` | 该消息首事件 `seq`；零事件轮 / record 构造面为 `null` | 保真：`seq` 可考、保序与 key 稳定 |
| `metadata.parentToolUseId` | `string \| null` | 该消息首事件的归因位；引擎侧恒 `null` | 保真（D14：消息级单值，部件级归因本期不落） |
| `metadata.record` | `TurnSummary \| null \| undefined` | 该轮轮行（`status: running \| completed \| failed \| stopped`、`error`、统计） | **回合元数据**：取代 `data-run-record` 部件；实时路由 `finish` 的 `messageMetadata` 写入，折叠路由轮行匹配写入；`null` = 该轮无轮行且无 `TurnDone`；`undefined` = 实时流尚未收口 |
| `parts` | `UIMessagePart[]` | `text` / `reasoning` / `tool-<name>`（呈现部件）；`data-*`（aux，实时态）/ `step-start`（reducer 标记，实时态） | 按首次到达顺序；折叠路不产 `data-*` 在 assistant 消息内、不产 `step-start`（收口整表替换即归一） |

### id 锚点表

| 场景 | assistant id | 提问 id | 依据 |
|------|--------------|---------|------|
| 真实运行事件（`turnId != null`） | `turn-<turnId>` | `turn-<turnId>-user` | 信封 `AgentEvent.turnId`（前置 change 唯一盖戳点） |
| `turnId == null`（旧转录 / 无轮归属构造面） | `evt-<该消息首事件 seq>` | `evt-<该提问事件 seq>` | 仅兜 id；收口以 fold 整表替换对齐（一次重挂，用户无感） |

### 折叠算法（`foldSessionMessages` 可执行伪码）

```
sealed  = events.filter(e => e.kind !== 'messageDelta')
paired  = PairingScope.global(sealed)                     // 既有视野层，零改动
groups  = []
for (e of sealed):                                        // 单遍扫描，两条分支同一循环
  cur = groups.at(-1)
  if (e.turnId != null):
      if (cur == null || cur.turnId !== e.turnId) groups.push({ turnId: e.turnId, events: [] })
  else if (e.kind === 'runStarted' && cur != null && cur.turnId == null):
      groups.push({ turnId: null, events: [] })           // 旧转录：RunStarted 为组界
  else if (cur == null):
      groups.push({ turnId: null, events: [] })
  groups.at(-1).events.push(e)

byTurn  = Map(groups.where(turnId != null) -> turnId)
rowIds  = Set(turns.map(t => t.turnId))
nullIdx = 0
ordered = []
for (row of turns):                                       // ① 行序即输出序（零事件轮就位）
  ordered.push({ group: byTurn.get(row.turnId) ?? { turnId: row.turnId, events: [] }, record: row })
for (g of groups):                                        // ② 剩余组按组序追加
  if (g.turnId == null)                                   //    旧转录：按组序配 turns 行
      ordered.push({ group: g, record: turns[nullIdx++] ?? synthesizeRow(g) })
  else if (!rowIds.has(g.turnId))                          //    行缺失（新数据但库无行）
      ordered.push({ group: g, record: synthesizeRow(g) })
                                                          //    行已就位的新数据组跳过（①已产出）
emit per entry (record):
  aux events  = group.events.filter(kind ∈ {runStarted, systemNotice, turnDone, raw})
      → 每条一条 system 消息：{ id: evt-<seq>, role: 'system',
                               metadata: { seq, parentToolUseId: null },
                               parts: [auxEventData(event)] }
  user event  = group.events.find(kind === 'message' && role === 'user')
      → 一条 user 消息：id = user event turnId != null ? turn-<turnId>-user : evt-<其 seq>,
                        role: 'user',
                        metadata: { seq: user event seq, parentToolUseId: user event 归因 },
                        parts: plansToParts(planBlocks(userEvent.blocks, paired))
  assistant blocks = group.events
                       .filter(kind === 'message' && role ∈ {assistant, tool})
                       .flatMap(e => e.blocks)        // seq 序，含工具结果
      → 一条 assistant 消息：id = turnId != null ? turn-<turnId> : evt-<组首事件 seq>,
                             role: 'assistant',
                             metadata: { seq: group.events[0]?.seq ?? null,
                                         parentToolUseId: 首个 assistant 事件的归因 ?? null,
                                         record },
                             parts: plansToParts(planBlocks(blocks, paired))   // 可为空
```

不变量：①assistant**恒一条**且为轮内末条（D7）；②零事件轮产 `parts` 空的 assistant（D11）；③`TurnDone` 不参与分组与计数，仅在行缺失时经 `synthesizeRow` 兜底 status（D9）；④未知事件经 `auxEventData` 的 `data-raw` 分支原文透传（既有两层收口）。

### 实时路状态机（`envelopeStream` 内，两出口共用）

```
state = { messageStarted: false, messageId: null, startSeq: null,
          stepOpen: false, partAnchors: Map(), currentToolCalls: Set() }

accept(envelope):
  event → onEvent(event); enqueue(frameChunks(event))
  record → onRecord(record)
           finishRecord(record)                           // 先 start（若尚无）再 finish
           close                                          // 单一 finish + 关流

ensureStart(turnId: number | null, seq: number | null):
  if (messageStarted) return
  id = turnId != null ? `turn-${turnId}` : `evt-${seq}`
  enqueue({ type: 'start', messageId: id, messageMetadata: { seq, parentToolUseId: null } })
  messageStarted = true; messageId = id; startSeq = seq

frameChunks(event):
  ensureStart(event.turnId ?? null, event.seq)            // 轮内首事件即发 start（含 RunStarted / 提问）
  delta:   kind = text | thinking
           if (!stepOpen) → enqueue({type:'start-step'}); stepOpen = true; partAnchors.clear()
           if (!partAnchors.has(kind)) → partAnchors.set(kind, event.seq)
                                         enqueue(part-start(deltaPartId(event.seq, kind)))
           enqueue(...eventToChunk(event, currentToolCalls, anchorOf))
  message/assistant:
           if (!stepOpen) enqueue({type:'start-step'})     // 该响应无 delta：开新段（D2）
           enqueue({type:'reset-step'}); stepOpen = false; partAnchors.clear()   // D3
           enqueue(...eventToChunk(event, currentToolCalls, anchorOf))
           currentToolCalls += event 的 toolUse id          // 本轮累积，跨 step 不清（D4/R-2）
  message/user:  enqueue(...eventToChunk(...)) = []        // 零 chunk（D5）
  message/tool:  enqueue(...eventToChunk(event, currentToolCalls, anchorOf))   // 仅就地补全，无帧
  aux:           enqueue(...eventToChunk(event, ...))      // 单条 data 部件（D8）

anchorOf(kind, seq) = partAnchors.get(kind) ?? seq

finishRecord(record):
  ensureStart(record.turnId, null)                         // 零事件轮：此刻才建消息
  enqueue({ type: 'finish', messageMetadata: { seq: startSeq, parentToolUseId: null, record } })
                                                           // seq 显式回填：mergeObjects 会以 null 覆盖
```

两条与 reducer 交互的硬事实（设计据此定形，非猜测）：①提交流（`sendMessage`）时 `pushMessage(user)` 先于 `makeRequest`，故 reducer 的 `start` 与新 assistant 消息**恒排在本地提问之后**（AC-2 的结构保证）；②`resume-stream` 时 reducer 以「新消息」起步（`lastMessage: void 0`），首个 `write` 因 `start` 已把 id 改成 `turn-<turnId>` 而命中「替换末条」——即**就地续写**（`isContinuation` 成立，AC-5），其代价是该轮此前由折叠补出的部件在流式期间被 reducer 的当前 parts 覆盖，收口由 fold 整表替换恢复（登记于待决问题 8）。

### 持久化（**本变更零落库语义**）

| 项 | 现状 | 本变更 |
|----|------|--------|
| 密封事件转录（`SessionEventRecord`） | `Box<RawValue>` 原文 + `(session_id, seq)` 打包键 | **零改动**：新字段（`turnId`）由前置 change 已入案，本期只消费 |
| 轮行（`TurnSummary` / `run` 表） | 每轮一行，四态状态机 | **零改动**：`running` / `completed` / `failed` / `stopped` 语义不变，停止轮由内核 `converge` 铸行 |
| `metadata.record` | 不存在（轮行走 `data-run-record` 部件 / 独立消息） | **纯前端内存态，MUST NOT 落库**：不新增列、不新增表、不新增 IPC 字段、不做迁移；重放时由 fold 按 `turnId` 与轮行重新挂载（库是唯一权威） |
| `step` / `start-step` / `reset-step` / `partAnchors` | — | **纯前端实时概念，MUST NOT 落库**：无信封字段、无 `step_index`；重放路径不出现任何 step 标记 |
| 提问消息临时 id（`pending-user-<n>`） | — | **纯前端内存态**：提问事件到达即归一，收口由 fold 整表替换；MUST NOT 出现在任何持久面或 IPC 载荷 |
| `synthesizeRow` 的 `turnId = -1` | — | **纯前端内存态**：只用一次渲染周期，不写库、不作消息 id、不参与寻址（D9 / 待决问题 4） |

**结论：本变更不含任何 `native_model` 版本演进、迁移链、查询面变化与索引变化。** 关系面亦无变化（无新表 / 无外键 / 无跨模型引用）。

---

## 路由 / API 设计

**本变更零 IPC / 零后端改动**：不新增命令、不增删信封字段、不改参数面。下表只声明「出线面不变」与「消费口径变化发生在前端」：

| 面 | 现状签名 / 载荷 | 本变更 | auth |
|----|----------------|--------|------|
| `agent_start`（发起流） | `(onEvent: Channel<AgentRunMessage>, root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd) -> TurnSummary` | **签名与载荷零改动**；前端把该 Channel 的信封按 D2/D3 分段、按 D6 把 `Record` 收进 `finish` | 无（本地 IPC，进程内） |
| `agent_session_watch`（订阅流） | `(onEvent: Channel<AgentRunMessage>, root, sessionId, sinceSeq) -> null` | **零改动**；两出口共用同一 `envelopeStream` 单点（step 分段与 part 簿记不各写一份） | 无 |
| `agent_stop`（终止） | `(root, sessionId) -> null` | **零改动**；终态经在挂流以 `Record` 流出，前端挂 `metadata.record`（`status = stopped`） | 无 |
| `session_detail`（会话单查 + 轮行） | `(root, sessionId) -> SessionSummary \| null` | **零改动**；`turns` 是 fold 的行序权威 | 无 |
| `agent_session_transcript`（转录重放） | `(root, sessionId) -> AgentEvent[]` | **零改动**；元素含 `turnId`（新数据有值、旧数据 `null`），fold 只消费密封事件 | 无 |

- **`finish` chunk 不是 IPC 面**：它是 transport 产出的 `ReadableStream<UIMessageChunk>` 元素，只在进程内前端流转（`metadata.record` 因此天然不落库）。
- **生成物零 diff**：本变更零信封 / 零 DTO / 零命令签名增量 ⇒ `bindings.ts` / `bindings.zod.ts` 由 `bindings:export` 再生成后仍应**零 diff**（AC-11 / AC-12 的机械判据）。
- **运行期校验**：`src/lib/ipc.ts` 的 Channel 信封 soft-report 依赖既有 `AgentRunMessageSchema`；本变更不改载荷形状，故无 schema 漂移风险。

---

## 依赖

### 运行时依赖（**零新增**）

| 依赖 | 版本（既有） | 用途 |
|------|--------------|------|
| `ai` | `^7.0.118` | ①类型：`UIMessage` / `UIMessageChunk` / `ChatTransport` / `ProviderMetadata`；②chunk 协议语义：`start`（改 id + 合并 metadata）、`start-step`（push `step-start` 标记）、`reset-step`（删「最后一个 `step-start` 之后」）、`finish`（`messageMetadata` 深合并）；③`mergeObjects` 的「深合并、非累加、`null` 覆盖」语义（决定 D6 的 `seq` 回填）；④`isContinuation` 判定（`state.message.id === lastMessage?.id`）——D7 的轮内序约束由此而来 |
| `@ai-sdk/react` | `^4.0.121` | `useChat`：`sendMessage({ id, parts })` 本地推提问（同步入列先于任何 chunk）、`setMessages` 整表替换（折叠收口）、`resumeStream`（订阅第二起点）、`status`（`isStreaming` 单流在挂判据）、`throttle`（10fps 渲染节流，零改动） |
| `@tauri-apps/api` | 既有 | `Channel<AgentRunMessage>` 信封流（发起流 / 订阅流同一信封类型） |
| `react` | 既有 | 组件与 hooks（零新增 hook 形态） |

### 构建 / 测试依赖（**零新增**）

| 依赖 / 工具 | 用途 |
|-------------|------|
| `typescript`（`tsc --noEmit`，经 `vp check`） | 严格类型门禁：`Extract` / `Omit` 派生的事件载荷与 `AgentDataParts` 成员收窄都在编译期强制（AC-6） |
| `vite-plus`（`vp check` / `vp test`） | 静态检查与测试运行（机械步骤由桌面端执行） |
| `knip` | 死代码扫描：`provisionalMessageId` / `eventsToUIMessages` / `runRecordToUIMessage` / `data-run-record` 退役后**零未用导出**（AC-6 / AC-12） |
| `@testing-library/react` + `vitest`（既有） | 组件与 hook 断言（用例编写归测试相位） |

**无 `pnpm-lock.yaml` / `Cargo.lock` 变化**（零依赖增删）。

---

## 验收标准回应

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 一轮一条气泡 | D1（`start` 单点、部件维度承载）+ transport `ensureStart` / `frameChunks`（附录状态机）+ fold 的「assistant 恒一条」（D11 / 伪码）；实时路辅助证据：`start-step` 不新开消息 |
| AC-2 提问角色不靠 seq 猜 | D5（transport `role = user` 零 chunk + 发起流本地推 + 订阅流事件追加 + id 归一到 `turn-<turnId>-user`）；`useAgentSession.sendMessage` 以 `{ id, parts }` 形态（`pushMessage` 先于 `makeRequest`，结构上先于任何 chunk） |
| AC-3 step 分段 | D2 / D3（每响应一次 `start-step`、密封前恒一次 `reset-step`、前段密封块在标记之外故存活）+ transport 状态机；工具结果只投 `tool-output-available`（`eventToChunk` 的 `mergeableResults` 语义零改动，无帧） |
| AC-4 part id 确定性 | D4（`deltaPartId(seq, part)` + per-step `partAnchors` + `anchorOf`）；`eventToChunk` 第三参使两起点共用同一实现（订阅流从任意 `since_seq` 起步亦不产 `missing part`） |
| AC-5 id 锚点一致 | D13（两路同源 `turn-<turnId>` / `turn-<turnId>-user`；`turnId` 为 null 时 `evt-<seq>` 兜底）+ D7（fold 的 assistant 恒末条使 `isContinuation` 成立，收口不重挂） |
| AC-6 record 归 metadata | D6（`finish` 携 `messageMetadata`）+ D17（`data-run-record` / `runRecordToUIMessage` / `provisionalMessageId` 退役）+ 类型表（`AgentDataParts` 去成员）+ `knip` 零未用导出 |
| AC-7 轮分组不依赖 RunStarted | D10（`turnId` 主路径）+ D11（行序输出，零事件轮就位）+ D9（`TurnDone` 只兜 status）；fold 伪码内零 `RunStarted` 计数逻辑（`RunStarted` 只作旧转录组界） |
| AC-8 旧转录可读 | D10 的第二分支（`turnId == null` 时 `RunStarted` 为组界，同一循环）+ D9 的按组序行匹配；零第二套分组函数、零迁移 |
| AC-9 空内容轮可见 / 状态行 | D15（单一判据：system 剔除 → 可见部件 → 按 `record.status`）+ D16（`failed` 标题 + 展开原文；`stopped` 「已停止」；`completed` / `null` 不渲染；`parts` 非空不叠加）+ `agent-error.ts` 零改动（`stopped` 不触发横幅，Q4） |
| AC-10 搜索卡两态回退不变 | D12（`PairingScope.global` 与 `plansToParts` / `plansToChunks` / `planBlock` 零改动；`pause_turn` 两段响应累积在同一轮同一条消息内）+ D15（空内容 `completed` 轮不渲染）+ 类型表「`AgentToolOutput` / `BlockPlan` 零改动」 |
| AC-11 零后端改动 | 本篇边界段 + 「路由 / API 设计」表（5 条命令签名与载荷零改动）+ D18（零 diff 面清单）；`cargo nextest` 零改动自然绿（机械步骤由桌面端执行） |
| AC-12 门禁与版本 | D18（`0.6.19 → 0.6.20`，`Cargo.toml` 不跟）+ D17 / D19（`vp check` + `knip` 的判据是「零未用导出」而非绝对计数）+ 「依赖」节（零新增依赖 ⇒ `pnpm-lock.yaml` 零 diff） |
| AC-13 边界纪律 | 本篇边界段（`agent-error.ts` / `agent-blocks.tsx` / `agent-input.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx` / `src/views/**` / 生成目录 / `explore.md` 零 diff）+ 修改文件表恰好 5 个文件（含版本行）；D16 解释了「状态行只能住 `agent-messages.tsx`」的边界原因 |

---

## 待决问题

1. **轮内状态行的「友好错误标题」深度（已知、接受）**：`ERROR_RULES` / `describeAgentError` 在 `agent-error.ts` 单点且模块私有，AC-13 钉住该文件零 diff，故本期状态行只能取**通用标题**（`failed` → 「Agent 运行失败」、`stopped` → 「已停止」）+ 可展开原文；spec ADDED requirement 的「友好错误标题」在本期字面满足（有标题且可展开原文），但**未**按错误类型细分。若后续需要（如「Provider 认证失败」级标题住在轮内），正确做法是把 `ERROR_RULES` 抽到独立模块并让横幅与状态行共用——本期不做（复制规则必漂移，改 `agent-error.ts` 违 AC-13）。
2. **`parentToolUseId` 的部件级归因未落（已知限制）**：`AgentMessageMetadata.parentToolUseId` 只能表达消息级单值。子代理归因当前恒 `None`（引擎侧事实），故无功能损失；若将来出现「轮内多条子代理消息需分别归因」，需给 text / tool chunk 挂 `providerMetadata.agent.parentToolUseId` 并在 fold 侧落到部件元数据——属后继变更，本期只保证「消息级可考」。
3. **实时路 `step-start` 标记部件在消息内残留（已知、接受）**：`start-step` 必然在 `message.parts` 里留一个 `{ type: 'step-start' }`（reducer 只在 `reset-step` 清「最后一个标记之后」）。它不被呈现（`PartView` 返回 `null`），收口后由 fold 整表替换消失；两路形状对拍需以「过滤 `step-start` 后的部件序」为准。反过来说，它也是未来「按步画分隔线」的现成锚点。
4. **`TurnDone` 兜底以「合成轮行」落地（需评审确认）**：spec 要求 `TurnDone` 在该轮轮行缺席时兜底 status，但 status 的唯一读取位被钉为 `metadata.record.status`，且 `AgentMessageMetadata` 形态被 spec 钉住（不能加字段）。本设计的解是合成一条只活在内存的 `TurnSummary`（`turnId = -1`）。若评审认为「合成轮行」越界，退路是 `record = null` 且该轮不渲染——代价是边缘场景（库态行缺失）的失败轮不可见。
5. **旧转录的轮行配对方式（退化面，登记防误改）**：`turnId == null` 的组按**组序**匹配 `turns[index]`（等价既有行为）。该退化面下若旧数据里存在停止轮（无 `TurnDone`），其状态行仍可能缺失——与变更前行为一致，不引入第二套分组、不做数据迁移。
6. **`eventToChunk` 第三参与 spec Module Contract 表的表述差**：表内该行只列 `(event, currentMessageToolCalls?)`；本设计追加可选 `deltaAnchor` 以满足 Scenario「delta 路映射」的 per-step 复用语义（事实偏差 2）。归档合入基线时，该表行需补第三参说明；**两参形态与语义零变化**（既有调用点不必改，缺省行为 = 逐 delta 独立 part）。
7. **`synthesizeRow` 的 `startedAt` 口径**：取该组首事件 `timestampMs`（组内无事件则不可能合成）。这与轮行的 `started_at`（`begin_turn` 时刻）可能有毫秒级差异；因该行只服务于 status 兜底、不参与任何寻址或统计聚合，接受。
8. **订阅流「就地续写」会短暂覆盖该轮已折叠出的部件（已知、接受）**：`resume-stream` 的 reducer 以 `lastMessage: void 0` 起新消息状态，靠 `start(turn-<turnId>)` 命中 `state.message.id === lastMessage?.id` 而**替换末条**（`isContinuation` 成立 ⇒ 不重挂、不双气泡，AC-5）。但 reducer 的 `state.message.parts` 从空起步，故流式期间该轮此前由 fold 补出的部件（如 `pause_turn` 第一段正文）在界面上会暂时消失，随后由新 delta 重建，轮收口再由 fold 整表替换恢复完整形状。**影响面**：仅「旁观一条 running 会话」的中途态（发起流路径由本地提问 + 实时 delta 自然累积，无此现象）；终态一致性由「收口以重放重建」保证，MUST NOT 为消除中途态另立第二套收口路径（proposal 明令）。若后续需要消除，正解是 `resumeStream` 前先 `setMessages` 把该轮裁剪到「仅 aux + 提问」（让 reducer 从干净态起步），属后继变更。

