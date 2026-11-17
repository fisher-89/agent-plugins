# 提案: session-midturn-history-overwrite

> **变更**: session-midturn-history-overwrite
> **日期**: 2026-11-17
> **状态**: draft
> **探索**: `openspec/changes/session-midturn-history-overwrite/explore.md`（A1 处置方案与已定裁量在案）

---

## 问题

进程在**任意工具执行中**死亡（关窗 / 崩溃 / 重启），库里的轮永远停在 `running`，而没有任何机制能把它关掉：

```
库态（进程死后仍在）                                内存态（进程死即蒸发）
 turn row: status = running                          AskRegistry:      oneshot::Sender
 transcript: Message(assistant, toolUse{...})        StopRegistry:     RunHandle
   ↑ 无配对 toolResult = 该工具从没返回过             SessionStreamRegistry: broadcast
                                                     ChangeFlowControl: ask 载荷
```

工具执行体的三条动作（注册 / 阻塞 / 回灌）全在内存面 ⇒ **「仍在执行」在库里没有任何 durable 痕迹**；而「已死」也没有任何对账入口：`agent_answer` 命中注册表 miss 只 `Err`、`agent_stop` 命中 miss **静默 `Ok` 且不动行**（`src-tauri/src/commands/exec/mod.rs:195-210`），重启后打开会话该行照旧 `running` ⇒ 前端当它「还在跑」。

同库同打开点**已有先例**：`open_workspace` 调 `calibrate_interrupted_runs`（`crates/infra/store/src/store.rs:428`，实现 `:2105`）把 change run 行 `Running → Interrupted` + `finished_at` + 记因 + 清 `active_phase`；**唯一漏的是 agent 轮行（`AgentRunRecord`）这一族**。

这一个库态谎言以三个看似无关的症状出现：

1. **打开会话时该轮已落库的历史被冲掉**。会话订阅的前端起点播种按 `detail.turns.find(t => t.status === 'running')` 选轮（`src/lib/agent-transport.ts:346`）；残留行排在前面时选到死轮 → 首个实时事件轮次不匹配 → 不播种 → ai-sdk reducer 的 `start` 命中「替换末条」，把该轮已落库的 parts 换成空消息，只剩增量。
2. **未配对的工具卡永久闪光**。四处同形状：`<TraceLine status={output ? (output.isError ? 'error' : undefined) : 'running'}>`（`src/components/agent/tool-cards.tsx:220/266/298/345`）——`running` 挂 shimmer（`src/components/ui/trace.tsx:36`），每个没拿到结果的工具卡看起来都在跑。
3. **ask 卡被当活卡**。活卡兜底分支按 `output === null` 推断（`src/components/agent/agent-messages.tsx:67-69`），崩溃残留的未配对 `tool_use` 同形 ⇒ 卡呈可应答，用户点了才 `Err`。

三者不是三个 bug，而是**同一个库态谎言的三个投影**，所以处置也应当是一条。判据是真事实而非猜测：启动瞬间 `AskRegistry` / `StopRegistry` / `SessionStreamRegistry` 必为空 ⇒ **启动时仍 `running` 的轮在数学上全部可证已死**，不必看它卡在哪个工具（这正是「ask 只是工具之一」的直接推论）。

---

## 提案

一条原则 + 十一处消费点落地，六条线：

1. **轮级不变量**：**进程中断 ⇔ 未收口的轮被标为 `interrupted`**，与轮停在哪个工具无关。由此「轮还在跑吗」只有一个权威答案（库态），三个投影同时归位。
2. **值域（Rust，`crates/core/agent/src/runner.rs:30`）**：`AgentRunStatus` 增 `Interrupted`，**末位追加**（bincode 变体索引只增不改 ⇒ 旧行解码不变、零版本递增、零迁移）；线格式 `"interrupted"`，语义与 change run 域既有 `RunStatus::Interrupted` 同义（客观中断，仅启动标定写入）。**不自造「`stopped` + 记因」**——语义即中断，且新值令 flow 侧的穷尽 `match` 编译报错，强制显式裁量而非静默落进兜底。
3. **标定（store）**：新增 `calibrate_interrupted_agent_turns(now)`（形态与判据抄既有 `calibrate_interrupted_runs`：先读扫、有残留才开写事务），在 `open_workspace` 与既有 change run 标定**同址同层**调用；只扫 `AgentRunRecord.status == Running`，写 `Interrupted` + `finished_at` + 记因定式，幂等、不动已终态、不碰 `RunRecord` / 转录。`interrupted` 由此成为 agent 轮唯一的非运行期异常值，且**只有标定一个写入点**。
4. **flow 侧映射（编译强制）**：`worker_settle`（`crates/core/orchestration/src/walker.rs:3282`）的穷尽 `match` 由编译器强制新增臂——`Interrupted` 按 failed 语义收口（`StepSettle::failed` + `Terminal::failed`，文案「会话被中断」），**归 failed 侧而非 stopped 侧**（中断不是用户主动停止，也不是可重试的模型失败）。编译报错是设计的一部分：该臂在生产路径不可达（进程中断时 walker 一并死亡、标定发生在下次启动），存在即「枚举扩值后强制显式裁决」的落点。**同一个反转面还带出既有明文的一处枚举扩写**——`desktop-change-orchestration` 既有明文以「会话 `status = Failed` / `Stopped`」收口相位内步骤系统异常的分类（`openspec/specs/desktop-change-orchestration/spec.md:912`），新值必须在该枚举里留痕，故该能力需一份 MODIFIED delta（不得只改代码不改明文）。
5. **前端三投影 + 呈现（7 处）**：
   - 症状 1 —— 订阅起点播种键由**首个实时事件自证**（`AgentEvent.turnId`），`detail.turns` 只保留「有无 running 行」的状态门，MUST NOT 用作选轮依据。**播种修复为何仍在范围**：标定调用继承既有 `let _ = store.calibrate_interrupted_runs(...)` 的吞错写法，标定失败即静默 ⇒ 残留行存活 ⇒ 播种仍选错轮；播种修复是**唯一不依赖标定成功的防线**，且改动只有几行。
   - 症状 2 —— 轮已终结时未配对部件的 `TraceLine` MUST NOT 呈 `running`（四处同形状；轮状态由 `metadata.record.status` 从消息下行到卡）。
   - 症状 3 —— 活卡兜底判据由 `output === null` 改读「该轮 `status === 'running'`」；「MUST NOT 以工具结果缺席即认为在等」由此从调用侧纪律变成结构保证。
   - 呈现 —— `filterConversation` 可见性白名单补 `interrupted`（空内容轮也可见，与 failed / stopped 同列）；`TurnStatusLine` 增「因应用重启中断」支；会话历史列表终态文案补「已中断」（沿用 change 流程 `run-control-panel` 的 `RUN_STATUS_LABEL` 词面先例）。
   - **`interrupted` 不进失败横幅**（`latestAgentFailure` 只认 `failed` 与「running 且失败 done」⇒ 默认即不触发，零改动，由测试固化）：每次重启弹红条是噪声，轮内状态行 + 列表「已中断」足够。
6. **版本交付**：`packages/desktop` `0.6.47` → `0.6.48`；`src-tauri/Cargo.toml` 不随动。

**规模**：1 个枚举值 + 1 个 store 函数 + 1 条分类臂 + 11 个消费点 + 4 个能力 9 条 spec delta（8 MODIFIED + 1 ADDED）。**不动 kernel 语义、不动命令签名、不动 loop、不新增事件词汇**；既有明文的反转面**共四处**且全部整段改写留痕（① `worker_settle` 穷尽 match 补臂——由编译器强制；② 同一反转面带出的既有枚举扩写：相位内步骤系统异常的分类「会话 `status = Failed` / `Stopped`」补 `Interrupted`，见 `desktop-change-orchestration` delta；③ 活卡兜底判据由「结果缺席」改读轮状态；④ 轮内状态行与可见性白名单扩 `interrupted`），**MUST NOT 用 ADDED 旁路绕过既有明文**。

---

## 能力

### 新增能力

- 无。新增的 requirement（agent 轮启动标定、终结轮部件态）全部落在既有能力内——机制是既有 change run 标定先例的对称补齐，不是新域。

### 修改的能力

- **desktop-workspace-store** —— 两条 delta：① MODIFIED「AgentRunRecord 状态字段类型化」：值域末位增 `Interrupted`（TS union / zod 跟随，`TurnSummary.status` 自动透出）；② ADDED「agent 轮启动标定与 running 残留终结」：判据（注册表全空 ⇒ 残留 running 全部可证已死）、扫描面（只 `AgentRunRecord`）、写入（状态 + `finished_at` + 记因，不虚构统计）、幂等（零残留不开写事务）、不动已终态、返回被标定 id 清单、`interrupted` 唯一写入点。
- **desktop-agent-execution** —— 两条 delta：① MODIFIED「ask 工具协议与阻塞应答通道」：补「停等态非 durable 与进程中断归属」——停等态 MUST NOT 落库、进程中断后该轮由启动标定终结、其后的 `agent_answer` 走既有 miss 臂 `Err`、MUST NOT 新增停等持久化 / 恢复面；② MODIFIED「Agent 调试页」：会话列表终态文案覆盖五值（`interrupted` → 「已中断」，MUST NOT 落进失败兜底）。
- **desktop-agent-chat-infra** —— 四条 delta：① MODIFIED「会话级统一渲染组件（sessionId 入参 + 状态驱动挂流）」：播种键 = 首个实时事件自证，`detail.turns` 只作状态门（MUST NOT 用作选轮依据）；② MODIFIED「展示组件族(对话透镜)」：工具部件运行态判据以该轮状态为准——轮已终结时未配对部件 MUST NOT 呈运行态；③ MODIFIED「ask 工具卡活卡与应答动作面」：卡壳运行态跟随轮状态、活卡兜底判据读轮状态（MUST NOT 以结果缺席判活）；④ MODIFIED「轮内状态行与空内容轮可见性」：新增 `interrupted` 状态行（「因应用重启中断」）+ 可见性白名单补 `interrupted`。
- **desktop-change-orchestration** —— 一条 delta：MODIFIED「相位收口失败可辨与相位内系统异常仍标 failed」：① 相位内步骤系统异常的分类枚举由「会话 `status = Failed` / `Stopped`」扩为 `Failed` / `Stopped` / `Interrupted`（既有明文 `spec.md:912` 整段改写留痕）；② `worker_settle` 五变体各设显式臂（无 `_` 兜底），`Interrupted` → `StepSettle::failed` + `Terminal::failed`「会话被中断」，与 `Stopped` 的 stopped 侧归属 MUST NOT 互换；③ 该臂生产路径不可达（进程中断时 walker 一并死亡），MUST NOT 为它新增运行期对账 / 复活逻辑，`Terminal` / `StepSettle` / `RunFault` 变体面零扩展。

### 引用沿用（零 delta）

- **desktop-ipc-type-bindings** —— `bindings` 再生走既有管线（枚举值经 specta 自动跟随，zod / `client:check` 守卫不变），零词面扩展。
- **desktop-change-state-store** —— change run 域启动标定与 `RunStatus` 五值词面零改动；本变更在**同一打开点并列**新增 agent 轮标定（形态对称，不改既有）。
- **desktop-agent-execution**（`AskRegistry` / `agent_answer` / `agent_stop` 命令面）—— 零改动：注册表空 ⇒ 既有 miss 臂已给显式 `Err`，标定只负责让库态与之一致。
- **desktop-agent-chat-infra**（`use-agent-session` / `agent-error.ts` / 事件适配层）—— 零改动：`running` 派生已是「轮行清单含 `running`」、`latestAgentFailure` 已只认 `failed`，`interrupted` 天然不派生运行中、不触发订阅、不触顶横幅（均由测试固化）。
- **desktop-change-flow-view** —— 流程抽屉经同一 `AgentSessionView` 消费，中断轮自动随 chat-infra delta 呈现；流程侧**前端**零词面扩展（walker 侧的收口分类扩写见上「修改的能力」`desktop-change-orchestration`）。
- **desktop-corpus-regression** —— 语料 golden 不覆盖 agent 轮行（只覆盖 change run 执行图），零 delta。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` — `AgentRunStatus` 末位增 `Interrupted`（doc 注释：语义 / 唯一写入点 / 落库兼容）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `worker_settle` 增 `Interrupted` 臂（按 failed 语义收口：`StepSettle::failed` + `Terminal::failed`，文案「会话被中断」；`match` 仍穷尽、无 `_` 兜底，臂序紧邻 `Stopped` 便于对拍；`worker_settle` / `worker_settle_probe` doc 注释的「四态 / 四臂」字面同步改五态）
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — 新增 `calibrate_interrupted_agent_turns(now)`；`open_workspace` 与既有标定同层追加调用（吞错形态沿用现状，播种修复为独立防线）
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — 仅随枚举更新 `AgentRunRecord.status` doc 注释（字段面零改动）
- `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` — `bindings:export` 再生成（`AgentRunStatus` union 增 `"interrupted"`）
- `packages/desktop/src/lib/agent-transport.ts` — 订阅起点播种键改由首个实时事件自证（`createPendingSeed` / `expectSeed` 契约随之调整；状态门保留）
- `packages/desktop/src/lib/agent-virtual.ts` — `filterConversation` 可见性白名单补 `interrupted`
- `packages/desktop/src/components/agent/agent-messages.tsx` — `isLiveAsk` 兜底读轮状态；`TurnStatusLine` 增 `interrupted` 支；把轮状态穿到 `PartView` / `ToolPartView`
- `packages/desktop/src/components/agent/tool-cards.tsx` — `AgentToolCardProps` 增可选「轮已终结」位；四处 `TraceLine status` 判据加支（终结轮未配对部件取 `pending` / 省略）
- `packages/desktop/src/views/agent/components/agent-run-history.tsx` — `statusLabel` / 行色增 `interrupted`（「已中断」）
- `packages/desktop/package.json` — `version` 0.6.47 → 0.6.48

### 测试文件

- `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` — 标定只动 `running` / 记因文案 / 幂等（二次零写入）/ 不动已终态 / 不误伤 `RunRecord` 与转录
- `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs` — `interrupted` 值域往返（bincode 新 tag）+ 旧行四值解码不变
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` — `worker_settle` 对 `Interrupted` 的终态映射（`StepSettle::failed` + `Terminal::failed`「会话被中断」，步 detail 与终态 reason 同文；`worker_settle四态分类…` 用例与文件头注释同步扩五态、四段既有断言逐字不动；`match` 无 `_` 兜底臂）
- `packages/desktop/src-tauri/src/commands/exec/mod_test.rs` — `session_detail` 返 `interrupted` 轮行；`agent_answer` 对已标定会话 → `Err`
- `packages/desktop/src/lib/agent-transport.test.ts` — **残留 running 行在前 + 直播轮在后的回归**：增量到达后直播轮已落库部件仍在（当前实现必红）；跨轮不串扰
- `packages/desktop/src/hooks/use-agent-session.test.ts` — `interrupted` 轮行不派生 `running`、不触发订阅
- `packages/desktop/src/lib/agent-virtual.test.ts` — `filterConversation` 保留空内容 `interrupted` 轮、仍剔除空内容 `completed` 轮
- `packages/desktop/src/components/agent/agent-messages.test.tsx` — `interrupted` 状态行；无 `pendingToolCallId` 时终结轮 ask 卡静态、`running` 轮照常活
- `packages/desktop/src/components/agent/tool-cards.test.tsx` — 终结轮未配对工具卡不呈 `running`（无 shimmer 标记）；运行中轮不变
- `packages/desktop/src/views/agent/components/agent-run-history.test.tsx` — 终态文案五值映射（`interrupted` → 「已中断」、不落失败兜底）与 `data-status` 钩子
- `packages/desktop/src/components/agent/agent-error.test.ts` — 最新轮 `interrupted`（含 `turnDone` 兜底不早于轮开始）时 `latestAgentFailure` 返 null（顶部失败横幅零呈现），默认零改动由此固化

### 不要修改

- `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` — 轮收口路径与 `loop` 零改动：`Interrupted` MUST NOT 由运行期产生（唯一写入点是启动标定）；标定挂在既有的 `open_workspace` 打开点，不改 `src-tauri/src/main.rs` 启动托管
- `packages/desktop/src-tauri/crates/core/agent/src/ask.rs`（`AskRegistry`）与三张内存注册表（`AskRegistry` / `StopRegistry` / `SessionStreamRegistry`）形态 —— 零改动
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs`（`INTERRUPTED_RESULT` 合成结果）—— 既有兜底，本次不动
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` 既有 `calibrate_interrupted_runs` 与 change 域 `RunStatus` 词面 —— 零改动（只在其旁新增并列的 agent 轮标定）
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`（`RunFault`）与 `Terminal` / `StepSettle` / `ChangeStepStatus` / `ChangeRunStatus` 变体面 —— 零扩展：中断复用既有 failed 面，MUST NOT 新增 `Terminal::interrupted` / `StepSettle::interrupted` / 新 `RunFault` 变体（详见 `desktop-change-orchestration` delta 的零改动面）
- 相位机其余面（`enter_phase` / `settle_phase` / `drive` / `run_step` / `run_worker` 与门禁 / 决策 / 反馈边分支）—— 零改动：`desktop-change-orchestration` 侧只扩 `worker_settle` 分类单点的一条臂
- 命令签名面（`agent_start` / `agent_stop` / `agent_answer` / `agent_steer` / `agent_session_watch` / `session_detail`）——零签名改动、零新命令
- 事件词汇 `AgentEventKind` —— 零新增变体（中断是库态事实，不是事件）
- **会话接续**（`resume_turn` / loop 第三入口 / cwd 入档）、change flow 停等态落库、ask 取消通路（`declined` 预留位）——显式不在本次范围；中断的轮按客观终止处理，用户重发即可
- `plugins` 侧与 `openspec/specs/**` 既有基线 spec —— 本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 枚举值全链在场 | `AgentRunStatus` 增 `Interrupted`（末位、线格式 `"interrupted"`）；`bindings.ts` / `bindings.zod.ts` 再生成后 ts union 与 zod literal 双双含该值；`TurnSummary.status` / `AgentRunRecord.status` 自动跟随，命令签名零改动 |
| AC-2 | 旧行零迁移 | `interrupted` 值域往返（写读）保真；中断值引入前落库的旧行解码得值与原四值逐一相同（bincode 变体索引不位移），无版本递增、无迁移步骤 |
| AC-3 | 标定语义 | `calibrate_interrupted_agent_turns` 只翻 `AgentRunRecord.status == Running` 的行：状态 `Interrupted` + `finished_at` + 记因定式；统计三字段不虚构；幂等（二次零变化、零写事务）；`completed`/`failed`/`stopped` 行零改写；`RunRecord` 与转录零触碰 |
| AC-4 | 标定触发点 | 所属 workspace 库首次打开（`open_workspace`）即触发，与既有 change run 标定同址；标定后 `session_detail` 直接返 `interrupted` 轮行；无残留时不产生写事务 |
| AC-5 | flow 侧收口 | `worker_settle` 对 `Interrupted` 显式落臂（`StepSettle::failed` + `Terminal::failed`，文案「会话被中断」，步 detail 与 run 终态同文）；穷尽 `match` 保持穷尽（五变体全显式、新增变体由编译器强制覆盖，无 `_` 兜底臂）；`Terminal` / `StepSettle` / `RunFault` 变体面零扩展；既有明文「相位内步骤系统异常」分类的会话状态枚举（`openspec/specs/desktop-change-orchestration/spec.md:912`）在 delta 中扩为 `Failed` / `Stopped` / `Interrupted` 并整段留痕，`desktop-change-orchestration` 列入「修改的能力」 |
| AC-6 | 症状 1 回归 | 残留 `running` 行在前、实时流属其后新轮的场景：播种按首个事件的 `turnId` 选轮，增量到达后新轮已落库部件仍在（回归测试在当前实现必红、实施后必绿）；残留轮零预投、零串扰 |
| AC-7 | 症状 2 呈现 | 终结轮（`interrupted`/`failed`/`stopped`）内未配对工具部件的 `TraceLine` 不呈 `running`（无 shimmer 标记）；`running` 轮内同类部件仍呈运行态 |
| AC-8 | 症状 3 判据 | 无 `pendingToolCallId` 时活卡判据读该轮 `status === 'running'`：终结轮的无结果 ask 卡零应答入口（提交/输入不产生 `onAnswer`、无 invoke）；`running` 轮照常活卡；有 `pendingToolCallId` 时精确对账语义不变 |
| AC-9 | 状态行与可见性 | 空内容 `interrupted` 轮在时间线可见并呈「因应用重启中断」行（`data-testid` + `data-status="interrupted"`）；顶部失败横幅 MUST NOT 出现；空内容 `completed` 轮仍被剔除 |
| AC-10 | 列表文案 | 会话历史列表对 `interrupted` 轮行呈「已中断」（不落 `failed` 兜底）；`running`/`completed`/`stopped`/`failed` 词面不变 |
| AC-11 | 后端拒绝面可归因 | 标定后的 `agent_answer` 返回显式 `Err`（含「无等待中的 ask」）并进入既有错误面；`agent_stop` miss 仍幂等 `Ok`（行为不变） |
| AC-12 | 版本与门禁 | `packages/desktop` version 0.6.47 → 0.6.48（`src-tauri/Cargo.toml` 不随动）；`vp check`（fmt / lint 含 50 行函数限）/ `vp test` / `knip` / `client:check`（含 `bindings:export` 无 stale diff、zod 一致性）/ rust nextest 全绿；基线（8976 例 / 覆盖率阈值）不回退 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 标定写错面：误把已终态轮或 change run 行翻掉 | 历史轮状态被污染、change 流程史错乱 | 低 | 标定只按 `AgentRunRecord.status == Running` 过滤且只写 `AgentRunRecord`；AC-3 断言「不动已终态 / 不误伤 `RunRecord` 与转录」三例；形态抄既有 `calibrate_interrupted_runs`（已在生产跑通） |
| R2 枚举变体追加破旧行解码 | 存量库读不出历史轮 → 会话页空白 | 低 | 变体**末位追加**（bincode 索引只增不改），AC-2 专项断言旧行四值解码不变；不开版本递增，避免引入迁移 |
| R3 标定调用吞错（继承 `let _ =`）导致残留行存活 | 症状 1 复发、会话页仍当它「在跑」 | 中 | **播种修复不依赖标定成功**（症状 1 的第二道防线，AC-6 独立回归）；残留行最坏表现退化为「列表显运行中、订阅一次零进展后自封」（`blockedRef` 已防重订环，不产生忙轮询） |
| R4 活卡判据改读轮状态后，无轮行的历史转录（旧数据）不再可应答 | 旧会话的 ask 卡变成静态 | 低-中 | 无轮行 ⇒ 不判活是**更安全**的缺省（旧数据本来就没有权威「在等」事实，误呈可应答只会给 `Err`）；判据缺省值（无 `metadata.record`）在 design 定稿并留痕 |
| R5 轮状态穿透多一层 prop 破 `ChatBubble` memo / 虚拟列表渲染预算 | 流式期间整树重渲染（性能回退） | 低 | 只穿一个布尔/枚举值（引用稳定，非新对象）；`ToolPartView` 侧仍按值比较；`agent-render-memo.test.tsx` 既有断言不让步 |
| R6 一处 `TraceLine` 遗漏（四处同形状） | 个别工具卡仍永久闪光 | 中 | AC-7 覆盖四张特化卡 + 默认对卡的测试；实现时按形状全量 grep 收口 |
| R7 `interrupted` 语义泄漏成第二个「失败」 | 用户把重启中断当故障、反复重跑 | 低 | 词面三处统一（状态行「因应用重启中断」、列表「已中断」、不进失败横幅），MUST NOT 复用失败标题与错误展开面；AC-9/AC-10 断言 |
| R8 spec delta 形态与 explore 表格不完全一致（标定落 ADDED 而非改写「会话查询与对账 API」） | 下游 design 需按本提案口径而非 explore 表格索引 | 低 | 本提案「过程 / 决策」留痕：标定是写路径而该 requirement 明文只约束查询与重导面，故新立 requirement；行为覆盖面不变 |
| R9 反转面漏项：改了代码却没在既有明文里留痕（`desktop-change-orchestration` 的分类枚举、能力章节与 delta 计数三处需同步） | 出现「spec 说三值、代码四臂」的空洞；下游 design / 归档按旧枚举理解 flow 侧行为 | 中（本次回溯即因漏列该能力与 912 行枚举） | `desktop-change-orchestration` delta 与实现、测试**同批**在案；AC-5 把「明文枚举扩写留痕」写进验收条件，能力章节与规模计数（4 能力 9 delta / 反转面四处）与该 delta 一一对齐；口径检查用 `grep` 验三处字面一致 |
| R10 `worker_settle` 的 `Interrupted` 臂被误当运行期可达，派生第四种「轮中断 → 节点处置」通道 | 与启动标定职责重复、语义分叉 | 低 | delta 明文「生产路径不可达（进程中断时 walker 一并死亡，标定发生在下次启动）」+「MUST NOT 为它新增运行期对账 / 复活逻辑」；`RunFault` 变体面零扩展由 AC-5 断言 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 是否新增枚举值 | `AgentRunStatus::Interrupted`（末位追加） | 语义即中断，`RunStatus::Interrupted` 有同义先例；新值令 `worker_settle` 穷尽匹配编译报错，强制 flow 侧显式裁量 | 「`stopped` + 记因」（语义混淆：用户主动停止 vs 进程死亡不可辨） |
| 标定的判据 | 启动瞬间内存注册表全空 ⇒ 残留 `running` 全部可证已死 | 「ask 只是工具之一」的直接推论，故不需要区分「停等中」与「一般工具执行中」 | 按工具类型 / 停等态分别对账（需停等态落库，反把内存态持久化） |
| 标定落位 | `open_workspace` 与既有 change run 标定同址同层 | 同库同打开点的既有先例；不新增启动钩子、不动 `main.rs` | 独立命令 / 启动托管新钩子（第二处打开点语义，重复且易漏） |
| 播种修复是否仍在范围 | 在（且**不依赖标定成功**） | 标定调用吞错即静默；播种修复是唯一不依赖标定成功的防线，改动数行 | 只靠标定（失败即回退到症状 1） |
| 播种键来源 | 首个实时事件自证（`turnId`） | `detail.turns` 的 running 行可能是残留、也可能与实时轮并发，按行选轮不可靠；事件自带权威轮次 | 保留按行选轮（选错即冲历史） |
| 活卡兜底判据 | 读该轮 `status === 'running'` | 把「MUST NOT 以结果缺席判活」从调用侧纪律升级为结构保证 | 保留 `output === null`（崩溃残留同形，误呈可应答） |
| 失败横幅归属 | `interrupted` 不进失败横幅（默认取），只走轮内状态行 + 列表「已中断」 | 每次重启弹红条是噪声；中断不是用户需要处置的故障 | 启动时一次性横幅（纯前端取舍，可后续单独提） |
| 标定的 spec 落位 | 新立 requirement（ADDED），不改写「会话查询与对账 API」 | 后者明文只约束查询 / 重导面，标定是写路径；新 requirement 与 change 域「启动标定与 active_phase 悬挂处置」对称 | 改写对账 requirement（把写路径塞进查询面词条，语义错位） |
| flow 侧 `Interrupted` 臂归属 | 归 **failed 侧**（`StepSettle::failed` + `Terminal::failed`，文案「会话被中断」） | 该步没有可信产出（会话被客观中断、不可续），且中断不是用户主动停止——归 stopped 会误导用户以为是自己停的，归 passed 会让 run 带未完成相位继续前进 | 归 stopped 侧（复用「会话被终止」语义，与用户停止不可辨）；静默落进 `_` 兜底臂（编译器强制的显式裁量被绕过） |
| flow 侧 reversal 的 spec 落位 | 改写 `desktop-change-orchestration` 既有 requirement（MODIFIED，整段改写留痕），不新立 ADDED | 既有明文 `spec.md:912` 以「会话 `status = Failed` / `Stopped`」收口相位内步骤系统异常的分类，新值是该枚举的第四项；用 ADDED 旁路会让同一分类存在两份词条 | 只改代码不改明文（留「spec 说三值、代码四臂」空洞）；把该能力移出本变更（`match` 仍不穷尽，代码侧仍必须落臂） |

### 待决问题

- 播种键自证的实现形态：`expectSeed` 去掉 `turnId` 入参改为「登记镜像 + 游标」再在首个事件处过滤，vs 保留入参由 transport 内部改派——design 定稿。
- 「轮已终结」下行位的类型：布尔 `turnRunning` vs 透传 `AgentRunStatus` vs 仅在 `ChatBubbleBase` 内算——design 定稿（倾向布尔，避免卡层认识状态词表）。
- 终结轮未配对部件的 `TraceLine` 具体态：`pending`（`opacity-60`）vs 省略 `status`——design 定稿，测试只断言「无 running 标记」。
- 无轮行历史转录（`metadata.record` 缺席）时活卡判据的缺省值（按运行中 vs 全静态）——design 会同 explore 坑复核后定稿。
- 标定记因文案的最终字面（「重启标定：桌面进程中断，会话轮客观已终止」为提案口径，design 可微调措辞但 MUST 与 change run 记因同格式）——design 定稿。
