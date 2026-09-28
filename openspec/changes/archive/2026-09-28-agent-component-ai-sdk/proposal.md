# 提案: agent-component-ai-sdk

> **变更**: agent-component-ai-sdk
> **日期**: 2026-09-28
> **状态**: 草案(proposal 阶段)

---

## 问题

packages/desktop 的 agent 会话呈现目前是两份近似双胞胎:视图层各自实现一套渲染与取数——

1. **渲染逻辑重复**: `views/explores/components/explore-conversation.tsx` 与 `views/agent/components/agent-event-timeline.tsx` 各自实现 Thinking 折叠 / 工具卡 / ResultCard / raw 透传 / entry 分发,约 670 行大半语义重复;`components/agent/` 骨架(`agent-messages.tsx` 空壳、`agent-input.tsx` 空文件)手写于此尚未接线。
2. **取数模式重复**: `useAgentRun`(调试)与 `useExploreSession`(探索会话)各自实现 `invoke("agent_start")` + `Channel` 累积 + 链还原重放;toolUse/toolResult 配对靠前端全局扫描(`collectResults` 跨消息按 id 配对,重复 id 取首个)。
3. **无终止能力**: 运行一旦发起只能等 CLI 跑完落库(Rust 侧无停止命令,spec 将「无 kill」列为 MVP 已知限制);前端截流后与 store 状态产生偏差,且寻址无门——run id 在落库时分配,而 `agent_start` 到 run 结束才 resolve。
4. **状态模型无收口**: 事件数组直接作为对话状态,没有统一的前端会话状态模型;今后任何新的 agent 会话页面都要再抄一遍上述双份逻辑。

---

## 提案

按 explore 阶段决策(D1–D7),把 agent 会话收敛为一套统一基建:

- **ai-sdk v7 作为前端状态层**(方案 A): 自定义 `TauriAgentTransport implements ChatTransport` 把 `invoke("agent_start")` + Tauri `Channel<AgentEvent>` 适配为 `ReadableStream<UIMessageChunk>`;`UIMessage` 成为前端对话状态的一等模型。`AgentEvent` 信封「线格式 = 落库格式 = DTO 镜像」三位一体**不动**。
- **适配层为新的缝**: 纯函数 `eventsToUIMessages`(重放重建)与 `eventToChunk`(实时增量)承担事件 → 部件映射;toolUse/toolResult 同 id 配对收口进适配层(消灭 `collectResults` 全局扫描);保真信息(seq、`parentToolUseId`、raw/systemNotice/runStarted/runResult)锁定进 message id / 部件元数据 / `data-*` 部件。
- **headless 基建 hook** `use-agent-chat`: `useChat`(ai v7)+ transport;重放装载(`agent_run_chain` + `agent_run_events` → `eventsToUIMessages` → `setMessages`,重开恢复 = 重放);发送组装收口链参数(`resumeSessionId` / `parentRunId` 从链尾取,经 `ChatRequestOptions.body` 穿透),会话文本组装(stance)留在来源侧。
- **展示组件族(双透镜)**: `components/agent/` 填实——`AgentMessages` 对话透镜(气泡/工具对卡/思考折叠/AskUserQuestion 静态卡)、`AgentTimeline` 保真透镜(seq 序/子代理归因/raw 透传)、`AgentInput` composer(输入/permission-mode/发送/停止)、工具卡注册表;双透镜共享底层块渲染件。**所有** agent 会话页面(explore 会话 / agent 调试 / 未来)统一消费;调试页的 run 表单/历史列表/JSONL 开关是页面级 chrome,包在共享核心外圈。
- **补 `agent_stop`(含 Rust)**: `agent_start` 落库后**提前 resolve** running 记录(id 立即可知),run 转后台任务继续,终态记录经 Channel 以同构部件(`data-run-record`)流出;`agent_stop(run_id)` 按 id 寻址,状态收敛为新增受控终态 `stopped`,尽力击杀 CLI 进程树。`useChat.stop()` 触达后端而非仅截断前端流。流式粒度维持整帧(token 级流式留作未来独立决策)。

---

## 能力

### 新增能力

- **desktop-agent-chat-infra** — agent 会话统一基建: 事件适配层(`eventsToUIMessages` / `eventToChunk` 纯函数)、`TauriAgentTransport`(ai v7 `ChatTransport` 适配)、headless hook `use-agent-chat`(重放装载 + 发送组装 + 停止接线)、展示组件族(`AgentMessages` / `AgentTimeline` / `AgentInput` / 工具卡注册表,双透镜)、ai-sdk 依赖收口纪律。

### 修改的能力

- **desktop-agent-execution** — 新增 `agent_stop` 命令与 `agent_start` 提前 resolve 契约演进;状态机增 `stopped` 受控终态与显式收敛分支;调试页时间线改走 `AgentTimeline` 保真透镜并提供停止入口;MVP 边界「无 kill」限制偿还。
- **desktop-explore-page** — 对话区与 composer 改走共享组件族(`AgentMessages` / `AgentInput`)与统一会话基建;实时/历史数据面经 transport + 重放装载表述;用户可见行为(气泡映射、stance、双恢复、续话)不变。

---

## 变更范围

### 实现文件

- `packages/desktop/package.json` — 新增 `ai`(v7)依赖
- `packages/desktop/src/lib/agent-adapter.ts`(新)— `eventsToUIMessages` / `eventToChunk` 纯函数
- `packages/desktop/src/lib/agent-transport.ts`(新)— `TauriAgentTransport implements ChatTransport`
- `packages/desktop/src/hooks/use-agent-chat.ts`(新)— headless 会话基建 hook
- `packages/desktop/src/components/agent/` — 填实骨架: `agent-messages.tsx`(对话透镜)、`agent-timeline.tsx`(新,保真透镜)、`agent-input.tsx`(composer)、工具卡注册表(AskUserQuestionCard 等收编)、`index.tsx` 导出面
- `packages/desktop/src/views/explores/hooks/use-explore-session.ts` — 改走 `use-agent-chat`;stance 文本组装留来源侧
- `packages/desktop/src/views/explores/explore-detail-view.tsx` + `components/explore-composer.tsx` — 改消费 `AgentMessages` / `AgentInput`
- `packages/desktop/src/views/agent/agent-debug-view.tsx` — 时间线改 `AgentTimeline`;增停止入口;chrome 外圈保留
- Rust `packages/desktop/src-tauri/crates/core/agent` — 状态机 `stopped` 终态与显式收敛分支(具体文件 design 定)
- Rust `commands::exec` + `run_agent()` 编排 — `agent_stop` 命令、`agent_start` 提前 resolve(后台任务持有 Channel 与 store 收尾)、命令注册;进程树击杀机制 design 定稿

### 测试文件

- `packages/desktop/src/lib/agent-adapter.test.ts`(新)— 全变体映射 / 工具配对 / 保真字段 / 未知事件不丢
- `packages/desktop/src/lib/agent-transport.test.ts`(新)— 假 Channel/invoke: body 穿透、chunk 流出、finish 收流
- `packages/desktop/src/hooks/use-agent-chat.test.ts`(新)— 重放装载重建、链参数组装、运行中禁发、stop 触达
- `packages/desktop/src/components/agent/*.test.tsx`(新)— 双透镜渲染 / composer 禁发与停止;吸收 `explore-conversation.test.tsx` 与 `agent-event-timeline.test.tsx` 用例
- Rust 侧 — `agent_stop` 收敛 / 提前 resolve / 幂等编排测试

### 删除文件

- `packages/desktop/src/views/explores/components/explore-conversation.tsx`(+ `.test.tsx`)— 渲染收编进 `components/agent/`
- `packages/desktop/src/views/agent/components/agent-event-timeline.tsx`(+ `.test.tsx`)— 由 `components/agent/agent-timeline.tsx` 取代

### 不要修改

- `core/agent` 的 `AgentEvent` 信封五变体与序列化(线格式 = 落库格式 = DTO 镜像三位一体)
- store schema(`user_agent_runs` / `user_agent_run_events`)与重放查询语义(`agent_run_chain` / `agent_run_events`)
- `agent-cli` 的 JSONL 解析与 flag 组装(stream-json 唯一格式;不加 `--include-partial-messages`)
- 其余页面(change-flow、db-inspector 等)与「刷新取数模型」纪律
- 不引入 assistant-ui 等额外组件库(组件族基于既有 shadcn `message` / `message-scroller`)

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 适配层 `eventsToUIMessages` / `eventToChunk` | 单测覆盖五变体映射、toolUse/toolResult 同 id 配对成单 tool 部件、seq 与 `parentToolUseId` 保真可考、未知事件 `data-raw` 透传不丢 |
| AC-2 | `TauriAgentTransport` | 假 Channel/invoke 测试: body 链参数原样透传 `agent_start`、事件逐条转 chunk、终态部件 + finish 后流关闭 |
| AC-3 | `use-agent-chat` | 测试: 重放装载重建的 UIMessage 与实时形状一致;发送携带链尾 `resumeSessionId` / `parentRunId`;运行中重复发送忽略;`stop` 触达 `agent_stop` |
| AC-4 | `components/agent/` 组件族 | 组件测试: 对话透镜四变体渲染、保真透镜 seq 序与子代理归因分组、composer 运行中禁发且停止入口可用 |
| AC-5 | explore 页接线 | 既有会话测试迁移通过: 重开双恢复、stance 拼接、链尾续话、四变体气泡呈现与迁移前一致 |
| AC-6 | debug 页接线 | loop 可见性场景保持(时间线经 `AgentTimeline`);运行中呈现停止入口且触发后事件流与状态收敛 |
| AC-7 | Rust `agent_stop` + 提前 resolve | cargo 测试: `agent_start` 落库后 resolve running 记录;`agent_stop` 收敛 `stopped` 并经 Channel 流出终态部件;对非 running run 幂等 |
| AC-8 | ai-sdk 依赖收口 | `ai` 锚定 v7;视图层(`views/`)与 Rust/store 无 ai-sdk import 或概念,仅基建层(适配层 / transport / 组件族 / hook)持有 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| ai-sdk v7 API 漂移(以当时 stable 为准) | 适配层返工 | 中 | 版本锁定;`ChatTransport` 签名已对 main 源码核实;适配层即隔离带,签名变动只改基建层 |
| `agent_start` 提前 resolve 属契约演进 | 既有 invoke 消费方语义变化 | 中 | 两个 hook 同批改造走新基建;既有测试随迁;Rust 测试锁编排时序 |
| Windows `cmd /C` 包装下进程树击杀不可靠 | 孤儿 claude 进程残留 | 中 | design 阶段核实 `taskkill /T` 或等价;失败情形留痕已知限制 |
| UIMessage 保真不足 | debug 透镜 / 子代理归因信息缺失 | 低 | 保真字段由 spec 锁定 + 适配层纯函数测试 |
| 双渲染收编引发回归 | explore 对话区呈现偏差 | 低 | 既有组件测试用例迁移;`MessageScroller` 组合与气泡映射不变 |
| knip 死代码报告污染 | `client:check` 失败 | 低 | 不新增 test-only 导出,一律经公共 API 测试 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| ai-sdk 引入角色 | 方案 A: 前端状态层接管(`TauriAgentTransport` + `UIMessage` 状态模型) | 信封三位一体不动;双 sink ⇒ 实时流与重放天然同构,UIMessage 可从事件序列重建 | B: Rust 直出 UIMessageChunk(三格式一体裂开 + Rust 镜像 TS schema 版本耦合);C: 组件层不依赖 ai-sdk(什么都没赚到) |
| 流式粒度 | 维持整帧渲染,不加 `--include-partial-messages` | 信封 / flag 纪律不动;token 级流式留作未来独立决策 | 增量流 |
| 链模型穿透 | `ChatRequestOptions.body` 经 `sendMessages` 穿透 | v7 签名已核实;transport 保持无状态转换器,链状态归应用层 | 构造闭包注入 / ref 插线 |
| `agent_stop` 寻址与契约 | 方案 a: `agent_start` 落库后提前 resolve running 记录,终态经 Channel 流出 | 与终态同构(D7)一致;run id 立即可用于 stop;免进程注册表;IPC 形状干净 | b: invoke 长驻 + 旁路进程注册表按 `(root, source, sourceRef)` / 客户端 token 寻址 |
| 停止收敛语义 | 新增受控终态字符串 `stopped` | 用户主动终止与 CLI 失败语义不同;error 文案编码状态是反模式;受控字符串扩展无 schema 迁移 | `failed` + `error="用户停止"` |
| 复用范围 | 一套状态模型 + 双透镜组件族,debug 页共用( chrome 留外圈) | 约 670 行双胞胎语义重复;所有 agent 会话统一基建 | 各页面独立维护双份渲染 |
| ai-sdk 版本 | 锚定 v7 | 适配层为版本变动隔离带 | v5 |

### 待决问题

- 进程树击杀机制: Windows `cmd /C` 包装下 `taskkill /T` 或等价方案的 design 核实。
- `data-*` 部件命名空间与 `useChat<UIMessage<...>>` 泛型形状。
- session hook 取链尾的具体机制(onFinish 回调 vs 状态订阅)。
- debug 页 chrome(run 表单 / 历史列表 / JSONL 开关)保留现状还是吸进组件族(当前倾向保留外圈)。
- `agent_start` 后台任务的结构形态(`commands/exec` 编排改造: 后台任务持有 Channel 与 store 收尾)。
- `stopped` 终态在 store 状态校验面的受控字符串清单扩展点。

---
