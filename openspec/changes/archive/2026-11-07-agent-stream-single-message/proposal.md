# 提案: agent-stream-single-message

> **变更**: agent-stream-single-message
> **日期**: 2026-11-06
> **状态**: draft
> **来源**: `openspec/changes/agent-stream-single-message/explore.md`（requirement 类型；`unified-agent-stream` 拆分后的第二份，编号 β-2）
> **前置**: `agent-event-turn-id` 已合入（`AgentEvent.turn_id` = 出线 `turnId` 已在案，前端本期首次消费）
> **性质**: 纯前端实时流语义重构——实时发射层从「一事件一消息」改为「一轮一条 assistant 消息 + step 分段」，持久重放层从「一事件一消息 + 轮行独立消息」改为「按轮折叠 + 回合元数据」。**Rust 后端零改动**。

---

## 问题

### 两个渲染不一致（用户可见症状）

1. **提问角色漂移**：会话续问时实时流把提问渲染为 `role=assistant`（右侧气泡变成左侧）；退出重进查落库转录，同一条是 `role=user`。
2. **思考跑到提问之前**：实时流把思考消息排在提问消息之上；重放（读转录）顺序正确。

两者都不是数据问题——落库数据一直是对的——而是**实时发射层与持久重放层用了两套建模机制**。

### 根因：ai-sdk 的 chunk reducer 假设「一条流 = 一条消息」

`ai` v7 的 `processUIMessageStream`（`ai/dist/index.js:7469` 起）在流起点构造**单数** `state.message`：

```js
function createStreamingUIMessageState({ lastMessage, messageId }) {
  return {
    message: lastMessage?.role === "assistant" ? lastMessage
             : { id: messageId, role: "assistant", parts: [] },   // ▲ role 硬编码 assistant
    activeTextParts, activeReasoningParts, partialToolCalls,
  };
}
// "start"     : 只改 state.message.id（不新建消息）
// "start-step": parts.push({ type: "step-start" })（step 分段唯一手段）
// "reset-step": 只删「最后一个 step-start 之后」的部件
```

它假设「一次请求 = 一条 assistant 消息、多消息靠多次请求」。我们的语义是「一次运行 = 一整轮（提问 + 思考 + 工具 + 回答），一次 Channel 流推完整轮」。三个派生代价：

| # | 代价 | 机制 |
|---|---|---|
| ① | **role 丢失** | `UIMessageChunk` 联合类型里无 role 字段（`index.d.ts:2477-2615`），reducer 一律造 `assistant` |
| ② | **顺序丢失** | chunk 协议无「位置」概念，折叠后顺序 = push 顺序；`AgentEvent.seq` 在 `eventToChunk` 处蒸发 |
| ③ | **provisional 双份** | delta 消息 id 恒为 `delta-none`、密封消息 id 为 `evt-<seq>`，两者必然是两条消息，只能靠 `reset-step` 剪接 + 收口折叠兜底 |

现状之所以「一事件一消息」，是因为**只发 `reset-step` 从不发 `start-step`**：没有 step 标记时「当前 step」= 整条消息，`reset-step` 等于全清，于是每来一个密封事件只能换新消息，提问事件与思考事件各占一条，两路 push 竞争决定谁在前。

### 折叠侧的四个现存缺陷（同一批修）

- `buildMessages`（`use-agent-session.ts:74`）以 `TurnDone` 出现次数索引轮行：`if (event.kind === 'turnDone' && turnIndex < turns.length)`。停止路径不发 `TurnDone`（`loop.rs:186/198/421` 直接 `return history`；`kernel.rs:308` 另铸 `Stopped` 终态写轮行），因此**无 `TurnDone` 的轮令后续所有轮行错位一格**，补尾逻辑又把剩余轮行塞到列表末尾而非该轮位置。
- 停止轮在 UI 上完全无标记：`agent-error.ts:128` 只认 `status === 'failed'`（`stopped` 不算失败 → 无横幅），消息流里也没有任何东西（无 `TurnDone`、无轮行部件）——用户点停止后界面只是「不再增长」。
- `agent-messages.tsx:215` 的 `parts.length > 0` 过滤会吞掉空内容轮（失败轮 / 停止轮）。
- `agent-adapter.ts:12-70` 的 8 个手写事件镜像无生成物守卫（`AgentEvent` 加变体时不报错）。

---

## 提案

### 一、总决策：方案 D —— 一轮一条 assistant 消息

四象限对照（explore 已裁定，此处只固定结论）：

| | 传输 | 语义层 | 修 ①②③ | 结论 |
|---|---|---|---|---|
| A | Channel | chunk（现状） | 需渲染层打补丁 | 收益薄 |
| B | Channel | AgentEvent 折叠 | 全修 | 放弃 `useChat` reducer |
| C | SSE | chunk | 全不修 | 只换传输，病根继承；且 Tauri custom protocol 的 `UriSchemeResponder::respond` 只收 `Cow<'static, [u8]>`，**无法流式** |
| **D** | Channel | chunk + step 分段 | 全修 | **采用**：一轮一条消息，贴合 ai-sdk 官方形态 |

### 二、实时侧机制：step = 一次模型响应

```
start(turnId)                         ← 每轮仅一次，消息 id 固定 turn-<turnId>
start-step   ┌ 响应 1
  reasoning-start r-<seq>-            ← part id 按 step 内首个 delta 的 seq 派生
  text-start      t-<seq>-
  sealed M1 → reset-step              ← 只清本 step 的临时部件
            → 追加整块部件（id = t-<seq>-<n>）
  tool result → tool-output-available ← 就地补全，不开新 step
start-step   ┌ 响应 2
  reasoning-start r-<seq>-            ← 新部件，不与响应 1 的思考合并
  sealed M2 → reset-step → 追加
finish(messageMetadata = { record })  ← 回合统计挂 metadata
```

- **部件粒度**：一条消息限制气泡数，不限制部件数。思考 / 正文 / 工具卡各占一个 part，按**首次到达顺序**排列，不重排、不刻意合并。
- **step 判据**：`loop.rs:379` 保证每次成功响应恰发一条 assistant 密封 `Message`（空响应直接收敛），故「密封 Message 之后再来 delta」即新 step，前端可数、后端不传 `step_index`。
- **`reset-step` 的保留语义**：每次密封到达先 `reset-step`（清本 step 临时部件）再追加整块；因为 `reset-step` 只清「最后一个 `step-start` 之后」的部件，前面 step 的密封块**不被波及**，故模型多响应轮次逐段累积。
- **持久层不变**：step 是纯前端概念，不落库、不加信封字段。

### 三、提问消息：不进 chunk 流，由会话状态侧推入

`chunk` 协议没有 role 字段，硬造必然产出第二条 `assistant` 消息（代价 ① 的根源）。故：

- **transport 两路均 MUST NOT 把 `role=user` 的密封 `Message` 事件转成 chunk**。
- 提问消息由会话状态侧产生：**发起流**在发送时刻本地推入（同步先于任何 chunk，结构上不可能再出现「思考在提问之前」）；**订阅流**（旁观 / 重挂）在收到该轮 `role=user` 密封事件时追加（旁观方无本地副本），其 `turn_id` 就在信封上，故 id 可直接取 `turn-<turnId>-user`。
- 两路 MUST NOT 出现同一提问的两条气泡：发起流的本地副本在该轮提问事件到达时**归一到同一 id**（同一消息，不新增）。

### 四、record 归属：回合元数据（R2）

业界定位：消息体只装显示内容；回合统计属于 metadata；汇总态走独立状态。

ai-sdk 原生已支持：`finish` chunk 可携带 `messageMetadata`（`index.js:8070`），合并语义为 `mergeObjects(旧, 新)`（`index.js:1384`，深合并、非累加）。

```
现在：  Record 信封 → start(turn-<id>) + reset-step + data-run-record 部件 + finish   ← 白跑一趟（渲染时被 system 过滤）
R2：    Record 信封 → finish/chunk 的 messageMetadata = { record: TurnSummary }
        chain 状态照旧（latestAgentFailure 读 chain，不受影响）
```

连带退役：`runRecordToUIMessage`、`data-run-record` 部件、`buildMessages` 里「事件位后插轮行」的整段逻辑。**`turnDone` → `data-run-result` 部件维持不变**（转录内事件的保真面，与 record 是两个层级）。

### 五、折叠：`foldSessionMessages(events, turns)` 按轮分组

```
foldSessionMessages(events, turns) → UIMessage[]
  分组键 ① AgentEvent.turn_id（信封级，主路径）
        ② turn_id 为 null 的旧转录 → 按 RunStarted 顺序分组（组界 = 下一枚 RunStarted 或数组末尾）
  每轮产出：提问消息（该轮 role=user 密封事件，可缺席）
            + assistant 消息（恒一条；parts 可为空）
            + aux system 消息（runStarted / systemNotice / raw / turnDone 逐事件一条）
  record  = 该轮轮行（按 turnId 匹配；缺席 → null）
  TurnDone = 仅用于 status 兜底（轮行缺席时 isError ? failed : completed）
```

- **`RunStarted` 不能作为分组依据**：停止轮 / 失败轮 / 重订中途都可能看不到它，零事件轮（ask 失败）连一枚事件都没有，按它计数会令后续轮整体错位。
- **不做双路径**：能解析 `turn_id` 就只走主路径；旧转录按顺序分组即现有行为，不引入第二套分组逻辑、不做数据迁移。

### 六、id 锚点

实时路与折叠路必须算出**同一个消息 id**，否则收口时 `useChat` 的 `isContinuation` 判定（`index.js:8208`）失配、消息重挂。

```
① AgentEvent.turn_id（信封级）  → assistant = turn-<turnId>，提问 = turn-<turnId>-user   ← 主路径
② turn_id 为 null（旧转录）      → id = evt-<该事件 seq>（assistant 与提问各自按自身事件）  ← 仅兜 id
```

- ① 覆盖全部新数据：真实运行事件由内核 `stamp` 盖 `turn_id`，两路口径唯一。
- ② 正常路径不出现（真实运行事件恒有 `turn_id`）：只兜旧转录与无轮归属构造面；若真落到 ②，收口以 fold 整表替换对齐（一次重挂，用户无感）。
- `turn-<turnId>` 与既有 `turn-${record.turnId}`（`agent-adapter.ts:489`）同族命名。除 id 计算外**不做迁移**：旧转录本就没有轮 id，重放时按 ② 出 id 即可。

### 七、异常原因的呈现：每轮恒产一条 assistant 消息

判据：**模型说的话是 content（parts）；关于这次运行的事实是 provenance（metadata）。**

```
metadata.record = TurnSummary（status / error / 统计）
呈现规则：
  parts 非空                     → 正常气泡
  parts 空 + status = failed     → 状态行（错误标题 + 可展开原文）
  parts 空 + status = stopped    → 「已停止」行
  parts 空 + status = completed  → 不渲染
```

- **不用 ai-sdk 的 `error` chunk**：它不进 parts、直接 `onError → throw` 中断流（`index.js:8087` / chat 侧 `onError` 为 `(error) => { throw error; }`），异常原因只能走 metadata 或 data 部件。
- **顶部 `AgentFailureBanner` 保留**：读 chain（与 `states.events` 同源），示「最新一轮失败」；与轮内状态行不冲突。`stopped` MUST NOT 触发失败横幅（用户主动停止不是失败）——`agent-error.ts` 的既有判据正确，**零改动**。

### 八、删除面

```
provisionalMessageId / 常量语义的 deltaPartId（改 seq 派生）
openProvisionalChunks / 「每个事件发 start + reset-step」的整段逻辑
openedParts（按其 parentToolUseId 键的簿记让位给每 step 最小簿记）
runRecordToUIMessage / data-run-record
buildMessages（并入 foldSessionMessages）
agent-adapter.ts 里 8 个手写镜像（AgentRunStartedData 等 → 从 bindings 派生）
```

---

## 能力

### 新增能力

**无。** 本变更落在两条既有能力内：「前端会话基建」与「agent 执行面」；轮内状态行是 `desktop-agent-chat-infra` 的一个新关注点（ADDED requirement），另立能力会让「会话呈现」与「会话呈现的一个状态行」分居两条。

### 修改的能力

- **desktop-agent-chat-infra**（`MODIFIED` × 4 + `ADDED` × 1）
  - `MODIFIED`「统一会话状态基建(use-agent-chat)」：`eventsToUIMessages` → `foldSessionMessages(events, turns)`；「增量流式」条款改写为「单消息回合 + step 分段」；新增提问消息条款（transport 不转 user 事件、由会话状态侧推入并归一到 `turn-<turnId>-user`）；「停止」与「终态同构」条款把 record 载体从 `data-run-record` 部件改为回合 metadata。既有 8 条 scenario 中 4 条逐字保留（会话参数穿透 / 来源引用与 cwd 穿透 / 未提供时缺省不漂移 / 清单筛选与寻址纪律并存）、3 条改写（重放装载重建 / 订阅流起点与簿记等价 / 停止触达后端）、1 条原位替换并更名（增量流式与密封替换 → 单消息回合与 step 分段）、新增 3 条（提问气泡不靠 seq 猜角色 / 旁观路径的提问气泡 / 旧转录顺序分组回退），共 11 条。
  - `MODIFIED`「事件适配层(纯函数,保真)」：入口改为 `foldSessionMessages`（按轮分组 + record 挂 metadata），`deltaPartId` 改 `(seq, part)`，消息 id 锚点改 `turn-<turnId>` / `turn-<turnId>-user`，保真字段改「`seq` 进消息 metadata」，`turnDone → data-run-result` 与 aux 独立 system 消息维持。既有 12 条 scenario 中 6 条逐字保留（工具同 id 配对 / 引用挂 text 部件元数据 / 未知事件不丢 / 计划层对拍同形 / 视野差异显式锁定 / 变体穷尽编译期可探测）、6 条随口径改写（全变体映射 / delta 路映射 / 实时流跨消息工具结果就地补全 / 搜索块两态与视图层零扫描 / 保真字段可考 / 收口折叠纠正残差双卡——「其他消息」改「其他 step」），新增 3 条（按轮分组不依赖 RunStarted / 空内容轮恒产一条消息 / id 锚点两路一致），共 15 条。
  - `MODIFIED`「展示组件族(对话透镜)」：对话透镜契约补「一轮一条 assistant 气泡」与「空内容轮按轮状态判定可见性」；`run-record` 载体退役（轮行改挂回合元数据、由该轮消息呈现）；scenario「辅助载体不进对话」的载体清单去掉 `run-record`、scenario「只读消费点形态」的轮行表述随之改写。既有 6 条 scenario 中 3 条逐字保留（对话透镜映射 / composer 运行中重复发送与停止 / 整面板禁用与理由呈现）、3 条随口径与命名改写（辅助载体不进对话 / 会话级入口归一（`eventsToUIMessages` → 折叠入口）/ 只读消费点形态），共 6 条。
  - `MODIFIED`「web 搜索活动与引用来源呈现」：`parts.length > 0` 过滤改为按 `metadata.record.status` 判定（原过滤会吞掉空内容失败 / 停止轮），并明确 `pause_turn` 两段响应累积在同一轮同一条消息；scenario「空部件消息不渲染」更名改写为「空内容 completed 轮不渲染」，scenario「搜索卡恒标题与两态」追加「两态在同一轮同一条消息内」，其余 3 条逐字保留，共 5 条。
  - `ADDED`「轮内状态行与空内容轮可见性」：failed → 错误标题 + 可展开原文；stopped → 「已停止」行；`stopped` MUST NOT 触发顶部失败横幅；对话透镜过滤判据单一来源。4 条 scenario。
  
  合计 `desktop-agent-chat-infra` delta：5 条 requirement（4 MODIFIED + 1 ADDED）、41 条 scenario（MODIFIED 面逐条保留未冲突者并改写冲突者，ADDED 面 4 条全新）。
- **desktop-agent-execution**（`MODIFIED` × 3）
  - `MODIFIED`「agent_stop 终止与提前 resolve」：终态载体表述从「与实时事件同构的状态部件」改为「`Record` 信封（`AgentRunMessage::Record`）经 Channel 收尾」，并显式声明后端 SHALL NOT 承诺前端呈现载体形状；新增「停止轮 SHALL 恒有轮行」（零内容停止轮由内核 `converge` 铸 `stopped` 行，前端据此呈「已停止」）。既有 4 条 scenario 中 1 条逐字保留（幂等忽略）、3 条随口径改写，共 4 条。
  - `MODIFIED`「run 事件落库与重放（workspace 维度）」：新增「轮归属为消费侧轮分组唯一依据」一条——轮分组 SHALL 以信封 `turnId` 为唯一依据，`RunStarted` MUST NOT 作为轮边界推断依据（停止轮 / 失败轮 / 零事件轮 / 重订中途可能不可见）；`TurnDone` 仅作 status 兜底。既有 5 条保留，新增 1 条（三轮含停止轮不错位），共 6 条。
  - `MODIFIED`「Agent 调试页」：事件时间线条目的「delta → provisional 消息连续增长」改写为「delta 累积进本轮唯一 assistant 消息」，场景「流式连续呈现」随之改写；新增「停止轮可观可见」场景。既有 8 条中 7 条逐字保留（参数面默认值 / agent 切换 / loop 可见性 / 运行中停止 / 原始转录切换 / 历史重放 / 历史会话运行中旁观流式）、1 条随口径改写（流式连续呈现），新增 1 条，共 9 条。
  
  合计 `desktop-agent-execution` delta：3 条 MODIFIED requirement、19 条 scenario（既有 17 条逐字保留或随口径改写，新增 2 条）。

### 零改动的能力及理由

| 能力 | 理由 |
|---|---|
| **desktop-workspace-store / desktop-ipc-type-bindings** | 本变更零落库语义改动、零命令面改动、零信封字段改动；`turnId` 于前置 change 已出线并入库，本期只是首次消费。`bindings.ts` / `bindings.zod.ts` MUST NOT 重导出（`git diff` 应零 diff） |
| **desktop-agent-execution 的其余条款**（信封与 loop 可见性 / 会话级实时流订阅面 / 引擎侧契约） | 信封、`stamp` 单点、注册表、`agent_session_watch`、`since_seq` 过滤、按轮收流全部不动；本变更只改前端如何消化同一批信封 |
| **desktop-change-chat / desktop-archi-chat / desktop-explore-page** | 三条能力的契约都是「对话经 `AgentSessionView` 注入形态渲染、composer 经插槽注入、无自有转录查询 hook」——本变更不改组件入参、不改 composer 插槽、不改消费点接线（轮行改由 metadata 承载，仍由同一组件呈现） |
| **desktop-agent-management / desktop-write-protection / desktop-workspace-config / desktop-crate-layout** | 零配置面、零依赖方向、零 crate 布局变化（纯前端 TS 面重构） |
| **ai-sdk 依赖收口条款** | 仍锚定 v7，仍收口于基建层；本次只是把 `UIMessageChunk` 的用法从「一事件一消息」改成「step 分段 + metadata」 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src/lib/agent-adapter.ts` | ① `eventsToUIMessages` → `foldSessionMessages(events, turns)`：按 `turn_id` 分组（旧转录 `turn_id=null` 按 `RunStarted` 顺序），每轮产提问 + 一条 assistant（id `turn-<turnId>`）+ 逐事件 aux system 消息，`metadata.record` 挂该轮轮行；② `deltaPartId(seq, part)` 取代常量形态（`t-<seq>` / `r-<seq>`）；③ `provisionalMessageId` / `runRecordToUIMessage` / `data-run-record` 成员退役；④ 8 个手写事件镜像（`AgentRunStartedData` 等）改从 `bindings` 派生（`Extract<AgentEvent, {kind:'runStarted'}>` 等），消除无守卫重复定义；⑤ `eventToChunk` 收敛为**只产块 chunk**（delta / 块 / 工具），事件头与 step 帧（`start` / `start-step` / `reset-step` / `finish`）归 transport 单点；`role=user` 的密封事件零 chunk |
| `packages/desktop/src/lib/agent-transport.ts` | ① 每轮只发一次 `start`（`messageId = turn-<turnId>`，`turnId` 取自首个事件的信封）；② step 分段：`start-step` 在新响应首个 delta 前发，密封 `Message` 到达时 `reset-step` + 追加整块；③ part id 按 step 内首个 delta 的 `seq` 派生（每 step 一份最小簿记：kind → 首 delta seq）；④ toolResult 就地补全（`tool-output-available`，不开新 step）；⑤ `Record` 信封 → 携 `messageMetadata: { record }` 的 `finish`（该轮消息尚未存在时先 `start` 建消息再 `finish`），删 `enqueueRecord` 独立消息组；⑥ 删 `openedParts` / `openProvisionalChunks` / `provisionalMessageId` 用法 |
| `packages/desktop/src/hooks/use-agent-session.ts` | ① `buildMessages` 删除，重放 / 收口统一走 `foldSessionMessages(events, turns)`；② `sendMessage` 用 `{ parts }` / `{ text }` 形态本地推提问消息，并在该轮提问事件到达时把本地副本归一到 `turn-<turnId>-user`；③ 订阅流（旁观）路径由提问事件追加 `turn-<turnId>-user`（不重复两条气泡）；④ 镜像面（`events` / `chain` / seq 游标 / 无进展闸）零语义变更 |
| `packages/desktop/src/components/agent/agent-messages.tsx` | ① `useHistoryWindow` 过滤判据从 `parts.length > 0` 改为「system 过滤 + 按 `metadata.record.status` 判空内容轮」；② 新增轮内状态行渲染（failed → 错误标题 + 可展开原文；stopped → 「已停止」；completed 空内容不渲染） |
| `packages/desktop/package.json` | `version` 0.6.19 → **0.6.20**（一次 patch；`src-tauri/Cargo.toml` 的 crate 版本行**不跟**） |

### 测试文件

| 文件 | 现状 | 处理 |
|---|---|---|
| `src/lib/agent-transport.test.ts` | 1826 行 / 68 用例，chunk 逐块断言（`reset-step` 命中 73 处） | 按 step 语义重写：每轮一次 `start`、`start-step` / `reset-step` 位置、part id 派生、`finish` 携 metadata、user 事件零 chunk；保留「两路同构」对拍（改为「发起流 vs 订阅流对同一事件序列归约一致」） |
| `src/lib/agent-adapter.test.ts` | 1570 行 / 71 用例（`deltaPartId` / `runRecordToUIMessage` 相关 40 处） | fold 分组断言（`turnId` 分组 / 旧转录顺序回退 / 停止轮不错位 / 空内容轮恒一条）；删 provisional 与 `data-run-record` 用例 |
| `src/hooks/use-agent-session.test.ts` | 1152 行 / 45 用例（含 `['evt-0','evt-1','evt-2','turn-11']` 类 id 断言） | id 断言全部改为 `turn-<turnId>` / `turn-<turnId>-user`；`buildMessages` 用例改 fold；补「本地提问副本归一」「订阅流提问追加」两组 |
| `src/hooks/use-agent-session.throttle.test.tsx` | 节流窗断言 | 随 id 锚点改消息 id 断言 |
| `src/components/agent/agent-messages.test.tsx` | 604 行 / 21 用例（含 `runRecordToUIMessage` 构造） | 新增空 parts + failed / stopped 渲染用例；删 `data-run-record` 用例；空态判定改按 status |
| `src/components/agent/agent-session-view.test.tsx`、`agent-render-memo.test.tsx`、`index.test.tsx`、`session-panel.test.tsx` | 夹具以「一事件一消息」的 id / parts 构造 | 消息夹具按轮构造（`turn-<turnId>` + `metadata.record`），断言按新形状 |
| `src/views/**` 的会话面板套件（`agent-debug-view` / `change-chat-panel` / `archi-chat-panel` / `explore-detail-pane` / `session-transcript-panel` / `use-change-chat` / `use-archi-chat`） | 部分断言气泡数 / `evt-<seq>` id / chunk 序列 | 只改受影响断言（气泡数与 id 形状）；**这些套件里「面板无自有转录查询 hook」「注入形态渲染」的结构性断言零改动** |

### 新增用例（覆盖新契约面）

| 位置 | 覆盖点 |
|---|---|
| `src/lib/agent-transport.test.ts` | ① 一轮两次响应：`start` 仅一次、`start-step` 两次、密封到达时 `reset-step` 只清本 step（第一段部件存活）；② 工具结果只投 `tool-output-available`、不开新 step；③ part id 由首个同类 delta 的 `seq` 派生且同 step 内复用；④ 订阅流从任意 `since_seq` 起步与发起流归约一致（无 `missing part`）；⑤ `role = user` 密封事件零 chunk；⑥ `Record` → `finish` 携 `messageMetadata.record`（该轮无消息时先 `start` 建消息） |
| `src/lib/agent-adapter.test.ts` | ⑦ 按 `turnId` 分组：三轮（正常 / 停止 / 正常）轮行逐一就位、零错位；⑧ `turnId` 全 null 的旧转录按 `RunStarted` 顺序分组、形状等价；⑨ 零 assistant 密封消息的轮仍产一条 `parts` 空的 assistant 消息；⑩ 实时路与折叠路 id 均为 `turn-<turnId>` / `turn-<turnId>-user`；⑪ 手写镜像退役后 `Extract<AgentEvent, …>` 派生类型与 bindings 同形 |
| `src/hooks/use-agent-session.test.ts` | ⑫ 发起流：本地提问副本在该轮提问事件到达时归一到 `turn-<turnId>-user`（消息总数不增）；⑬ 订阅流：提问事件到达即以 `turn-<turnId>-user` 追加（旁观方有提问气泡）；⑭ 收口前后 id 序列一致（`isContinuation` 成立） |
| `src/components/agent/agent-messages.test.tsx` | ⑮ `parts` 空 + `failed` 呈状态行（标题 + 可展开原文）；⑯ `parts` 空 + `stopped` 呈「已停止」；⑰ `parts` 空 + `completed` 不渲染且不计入 `remaining`；⑱ `parts` 非空 + `stopped` 不叠加状态行 |
| `src/components/agent/agent-session-view.test.tsx` | ⑲ `stopped` 轮 MUST NOT 触发顶部失败横幅，同轮位置有「已停止」行 |

### 不要修改

- **Rust 后端**：`packages/desktop/src-tauri/**` 全部零改动——信封已有 `turn_id`、`Record` 信封已存在、`agent_session_watch` 与 `since_seq` 过滤语义不变、`stamp` 单点不变。本变更 MUST NOT 新增命令、MUST NOT 增删信封字段、MUST NOT 动 `loop.rs` / `kernel.rs`。
- **生成物**：`src/types/generated/bindings.ts` / `bindings.zod.ts` MUST NOT 重导出、MUST NOT 手改（`git diff` 应零 diff）；`src/types/dto.ts` 为 `export type *` shim，自动跟随。
- **`src/components/agent/agent-error.ts`**：`latestAgentFailure` 的 `stopped` 判据维持「不算失败」（用户主动停止不是失败）；`stopped` 的可见性由轮内状态行承接，MUST NOT 改造成失败横幅。
- **`src/components/agent/agent-blocks.tsx` / `agent-input.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx`**：块渲染件、composer、工具卡注册表、面板骨架、会话级入口的入参与插槽形状零改动。
- **`src/views/**` 源码**：消费点接线零改动（仍以 `useAgentSession` / `AgentSessionView` 为唯一入口；`AgentRawStream` 继续消费 `state.events` 镜像）。
- **`openspec/specs/**` 基线**：只在 change 目录出 delta（归档时合入）；`openspec/config.json` 零改动。
- **`openspec/changes/agent-stream-single-message/explore.md`**：只读。

### explore 偏差与落地裁定

| # | explore 原文 | 本提案裁定 |
|---|---|---|
| Q1 | 折叠入口名为 `fold(events, turns)` | 导出名定为 **`foldSessionMessages(events, turns)`**：`fold` 与通用数组词汇撞名、无检索性；语义与签名不变 |
| Q2 | D3「`sendMessage({ text })` 本地推 user 消息，实时流中跳过后端那条 user `Message` 事件」 | **按两起点分化**：发起流本地推（同步先于任何 chunk）+ 该轮提问事件到达时把本地副本归一到 `turn-<turnId>-user`；**订阅流 MUST NOT 跳过**该事件（旁观方无本地副本，跳过即丢提问气泡）——由 hook 在事件到达时以 `turn-<turnId>-user` 追加。两路共同不变式：同一提问恰一条气泡 |
| Q3 | T6「删除 `openedParts` 簿记（part id 由 delta seq 派生后不再需要）」 | 部分成立：**按 `parentToolUseId` 键的跨消息开件簿记**退役，但每个 step 内仍需一份**最小簿记**（kind → 首个 delta 的 seq），否则同一 part 的后续 delta 会算出新 id 并触发 `missing text part`。簿记作用域从「整条流」缩到「当前 step」 |
| Q4 | V4「`agent-error.ts:128` 补 `stopped`」 | **不改**：`stopped` 由轮内状态行承接；`latestAgentFailure` 只认 `failed` 是正确的产品语义（主动停止不得显示「Agent 运行失败」） |
| Q5 | 第八节验证顺序列出 `pnpm client:test` 等命令 | 机械步骤由桌面端执行；本提案只钉验收判据与测试文件面 |

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | **一轮一条气泡**：一轮含「提问 + 思考 + 工具调用 + 回答」时，实时流归约后 `messages` 中该轮恰一条 `assistant` 消息（部件数不限，按首次到达顺序），MUST NOT 出现同一轮的第二个 assistant 气泡 | `agent-transport.test.ts` / `use-agent-session.test.ts` 断言消息条数与部件序列 |
| AC-2 | **提问角色不再靠 seq 猜**：续问一轮，实时流中的提问消息 `role === 'user'`，且排在 assistant 消息**之前**（结构上保证，非概率）；提问消息 id 为 `turn-<turnId>-user` | 同上 + 手工验证（续问后思考不再跑到提问之上） |
| AC-3 | **step 分段**：一次模型响应发一次 `start-step`；密封 `Message` 到达时先 `reset-step` 再追加整块部件，前面 step 的密封块不被清除；工具结果只投 `tool-output-available`、不开新 step | `agent-transport.test.ts` chunk 序列断言（`start-step` / `reset-step` 位置 + 前 step 部件存活） |
| AC-4 | **part id 确定性**：同一 step 内同一 kind 的 delta 共用「首个 delta 的 seq」派生的 id（`t-<seq>` / `r-<seq>`），跨 step 不撞；订阅流从任意 `since_seq` 起步亦不产生 `missing part` 错误 | `agent-transport.test.ts`（两起点对同一事件序列归约一致） |
| AC-5 | **id 锚点一致**：实时路 assistant 消息 id 与折叠路同为 `turn-<turnId>`；收口 `isContinuation` 成立（MUST NOT 出现收口后整段重挂成新消息） | `use-agent-session.test.ts` 收口前后 id 序列断言 |
| AC-6 | **record 归 metadata**：`Record` 信封到达时该轮 assistant 消息 `metadata.record` 为该轮轮行，MUST NOT 再产 `data-run-record` 部件或独立 system 消息；`data-run-record` / `runRecordToUIMessage` 在源码与生成物外零残留（`knip` 零未用导出） | `agent-transport.test.ts` + `vp check && knip` |
| AC-7 | **轮分组不依赖 RunStarted**：以「无 `TurnDone` 的停止轮 + 其后正常轮」的转录折叠，轮行 MUST NOT 错位（第 2 轮轮行落在第 2 轮）；零事件轮的轮行亦就位 | `agent-adapter.test.ts` 折叠用例（停止轮场景） |
| AC-8 | **旧转录可读**：`turn_id` 全为 null 的存量转录按 `RunStarted` 顺序分组折叠，消息数与本变更前形状等价（无缺轮、不抛错）；MUST NOT 出现第二套分组逻辑 | `agent-adapter.test.ts` 旧转录用例 |
| AC-9 | **空内容轮可见**：`parts` 空 + `status = failed` 呈状态行（标题 + 可展开原文）；`parts` 空 + `status = stopped` 呈「已停止」；`parts` 空 + `status = completed` 不渲染；`stopped` MUST NOT 触发顶部失败横幅 | `agent-messages.test.tsx` + `agent-session-view.test.tsx` |
| AC-10 | **搜索卡两态回退不变**：`pause_turn` 拆分段在同一轮消息内累积（不再产生第二段 assistant 消息），收口后由重放折叠补出来源列表；`tool-web_search` 卡形状零变 | 既有 web-search 用例在新形状下全绿 |
| AC-11 | **零后端改动**：`packages/desktop/src-tauri/**` 与 `src/types/generated/**` `git diff` 零 diff；`cargo nextest run --workspace` 全绿（零改动应自然绿） | `git diff --stat` + rust 套件 |
| AC-12 | **门禁与版本**：`vp check`（含 knip）零未用导出、`vp test` 覆盖率 lines ≥ 90 / branches ≥ 85 / functions ≥ 90、`pnpm run bindings:export` 后 `bindings.*` 零 diff；`package.json` 0.6.19 → 0.6.20，`src-tauri/Cargo.toml` crate 版本行不跟 | `pnpm -C packages/desktop run check` + 覆盖率报告 + 版本行审查 |
| AC-13 | **边界纪律**：`agent-error.ts` / `agent-blocks.tsx` / `agent-input.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx` / `src/views/**` 源码零 diff；生成目录零 diff；`explore.md` 零 diff | `git diff --stat` 审查 |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | **ai-sdk reducer 行为假设不成立**（`reset-step` 只清最后一个 `step-start` 之后的部件、`finish` 的 metadata 为深合并非覆盖） | step 分段失效：前面 step 的密封块被清或 record 被覆盖，导致部件丢段 / 统计错值 | 中 | 两条假设已在 `ai/dist/index.js`（`getCurrentStepParts` / `mergeObjects`）逐行核对并入案；`agent-transport.test.ts` 以真实 reducer 归约断言「两 step 的密封块共存」「start 与 finish 的 metadata 合并」 |
| R-2 | **step 内最小簿记被误删**（explore T6 字面「删除簿记」） | 同一 part 的后续 delta 算出新 id → reducer 抛 `missing text part`，流中断 | 高 | 裁定 Q3 明确「按 `parentToolUseId` 的簿记退役、per-step 最小簿记保留」；AC-4 以「订阅流任意 `since_seq` 起步不抛 `missing part`」为机械判据 |
| R-3 | **提问气泡重复或丢失**（发起流本地推 + 订阅流事件追加 + 收口 fold 三方叠加） | 同一提问出现两条气泡，或旁观路丢提问 | 高 | 三处不变式写进 requirement（同一提问恰一条气泡、发起流归一到同一 id、订阅流由事件追加）；AC-2 + 两起点各一组用例；收口以 fold 整表替换兜底 |
| R-4 | **收口消息重挂**（`isContinuation` 失配） | 收口瞬间整段消息重新挂载：滚动位置跳、思考折叠态复位 | 中 | id 锚点双路同源（`turn-<turnId>`）+ AC-5 机械断言；`turn_id` 为 null 的交界按 ② 兜 id 并接受一次整表替换（无旧转录轮次滚动位置保证） |
| R-5 | **旧转录回归**（`turn_id` 为 null 的存量库） | 老会话打开后缺轮 / 顺序错乱 / 抛错 | 中 | fold 的 ② 分支按 `RunStarted` 顺序分组（等价既有行为），并显式声明「新事件 MUST NOT 依赖该退化」；AC-8 以存量形态 fixture 直测 |
| R-6 | **轮行错位未真正修好**（停止轮 / 零事件轮仍在 fold 边界之外） | 用户看到的轮统计与对话错位一格，症状与本变更的「修缺陷」承诺相悖 | 中 | 分组键为信封 `turn_id`（停止轮与零事件轮都有轮行、事件侧或空）；`TurnDone` 降级为 status 兜底；AC-7 以「停止轮 + 后续正常轮」双轮 fixture 锁定 |
| R-7 | **测试改写量失控**（两个 1800 / 1500 行套件逐块断言失效） | 评审噪声巨大、误删真断言（保真 / 配对 / 视野差异成批丢失） | 高 | 只重写「chunk 逐块断言」与「一事件一消息 id 断言」两类；保真（`seq` 可考 / `parentToolUseId`）、视野差异、计划层对拍、搜索卡两态等语义断言**逐条保留**（AC-6 / AC-10 反向钉住）；改写按文件分批、每批独立可绿 |
| R-8 | **旁观路体验退化**（订阅流起点晚于提问事件时，收口前只见回答） | 旁观者短时间内看不到提问气泡 | 低 | 提问事件在每轮首部发出（`loop.rs` 顺序：`RunStarted` → user `Message` → 响应），订阅 `since_seq` 落到窗口内即可见；落入窗口外时由收口 fold 补出（AC-5 同一 fold 入口） |
| R-9 | **`data-run-record` 退役被误扩**（顺手把 `turnDone → data-run-result` 也删掉） | 转录内事件的保真面丢失（`usage` / `subtype` 在消息层不可见） | 中 | 裁定表与「不要修改」双向钉住：`turnDone → data-run-result` 维持；record 与 turnDone 是两个层级（后端终态轮行 vs 转录内收敛事件） |
| R-10 | **覆盖率门禁**（新增分支：status 三态 + id 锚点两分支 + 旧转录回退） | 门禁红，逼迫补无意义用例 | 低 | 新增断言全部落在真实契约面（三态状态行、两起点归约一致、旧转录 fixture），不为覆盖率造用例 |
| R-11 | **版本纪律**（前端改动忘记 bump） | 发版流水线版本未递增 | 低 | AC-12 以 `0.6.19 → 0.6.20`（一次 patch）为机械判据；`src-tauri/Cargo.toml` crate 版本行不跟（既有纪律） |
