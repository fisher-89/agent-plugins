# 工具运行中进程中断：轮卡在 running 的语义与 A1 处置方案

workflow_type: requirement

## 一、问题：进程在工具执行中死亡，库里的轮永远停在 running

`AskUserQuestion` 只是工具之一。**任意工具**执行中终止进程（关窗 / 崩溃 / 重启），结果都是同一个：

```
库态（进程死后仍在）                                内存态（进程死即蒸发）
 turn row: status = running                          AskRegistry:      oneshot::Sender
 transcript: Message(assistant, toolUse{...})        StopRegistry:     RunHandle
   ↑ 无配对 toolResult = 该工具从没返回过             SessionStreamRegistry: broadcast
                                                     ChangeFlowControl: ask 载荷
```

工具执行体的三条动作（注册 / 阻塞 / 回灌）全在内存面 ⇒ **「仍在执行」没有任何 durable 痕迹**，
而库里的轮行永远停在 `running`。当前架构没有对账机制：

| 动作 | 现状 |
|---|---|
| `agent_answer` | `AskRegistry` miss → `Err`「当前无等待中的 ask」 |
| `agent_stop` | `StopRegistry` miss → **静默 `Ok`，行不动**（`commands/exec/mod.rs:195-210`） |
| 重启后打开会话 | 轮行仍 `running` ⇒ 前端当它「还在跑」 |

即：**用户没有任何入口能把这一行关掉**，它会一直冒充「运行中」。

## 二、一个事实的三个投影

「库态说 running、但人已经死了」会以三种看似无关的症状出现：

1. **打开会话时该轮已落库的历史被冲掉**。会话订阅的前端起点播种按
   `detail.turns.find(t => t.status === 'running')` 选轮（`src/lib/agent-transport.ts:344`）；
   残留行排在前面时会选中死轮 → 首个实时事件轮次不匹配 → 不播种 → ai-sdk reducer 的
   `start` 命中「替换末条」，把该轮已落库的 parts 换成空消息，只剩增量。
2. **未配对的工具卡永久闪光**。四处同一形状：
   `<TraceLine status={output ? (output.isError ? 'error' : undefined) : 'running'}>`
   （`components/agent/tool-cards.tsx:220/266/298/345`），`running` 挂 shimmer
   （`components/ui/trace.tsx:36`）⇒ 每个没拿到结果的工具卡看起来都在跑。
3. **ask 卡被当活卡**。活卡判据的兜底分支按 `output === null` 推断（`agent-messages.tsx:67-69`），
   崩溃残留的未配对 `tool_use` 同形 ⇒ 卡呈可应答，用户点了才 `Err`。

三者不是一个功能的三个 bug，而是**同一个库态谎言的三个投影**——所以处置也应当是一条。

## 三、判据是真事实，不是猜测

- 启动瞬间 `AskRegistry` / `StopRegistry` / `SessionStreamRegistry` 必为空 ⇒
  **启动时仍 `running` 的轮在数学上全部可证已死**，不必看它卡在哪个工具（这正是「ask 只是工具之一」的
  直接推论，故不需要区分「停等中」与「一般工具执行中」）。
- 同库同打开点**已有先例**：`open_workspace` 调 `calibrate_interrupted_runs`（`store.rs:425-430`）把
  change run 行 `Running → Interrupted` + `finished_at` + 记因「重启标定：桌面进程中断…」+ 清
  `active_phase`。**唯一漏的是 agent 轮行（`AgentRunRecord`）这一族**。
- `RunStatus::Interrupted` 的语义先例已在：`interrupted` = 客观终止、仅启动标定写入、运行期写路径不产生。

## 四、A1 方案（本次范围）

### 4.1 原则

轮级不变量：**进程中断 ⇔ 未收口的轮被标为 `interrupted`**，与轮停在哪个工具无关。由此
「轮还在跑吗」只有一个权威答案（库态），三个投影同时归位。

### 4.2 Rust（4 处）

| 文件 | 改动 |
|---|---|
| `crates/core/agent/src/runner.rs:30` | `AgentRunStatus` 增 `Interrupted`（末位追加，bincode tag 增量；旧行解码不变） |
| `crates/core/orchestration/src/walker.rs:3280` | `worker_settle` 的穷尽 match 由编译器强制新增臂：`Interrupted` 按 failed 语义收口（「会话被中断」）；**编译报错是设计的一部分**——强制 flow 侧显式决定，而不是静默落进兜底 |
| `crates/infra/store/src/store.rs` | 新增 `calibrate_interrupted_agent_turns(now)`（抄 `calibrate_interrupted_runs`，`:2105`），在 `open_workspace`（`:425`）同层调用；只扫 `AgentRunRecord.status == Running`，写终态 + `finished_at` + 记因，幂等、不动已终态 |
| 生成物 | `src/types/generated/bindings.ts` / `bindings.zod.ts` 重生成 |

### 4.3 前端（7 处）

| 文件 | 改动 |
|---|---|
| `src/lib/agent-virtual.ts:52` | `filterConversation` 保留 `interrupted`（空内容轮也可见，与 failed / stopped 同列） |
| `src/components/agent/agent-error.ts:134` | `interrupted` 不进失败横幅（走轮内状态行，避免每次重启弹红条） |
| `src/components/agent/agent-messages.tsx` | `TurnStatusLine` 增 `interrupted` 支（「因应用重启中断」）；把该轮 `status` 穿到 `PartView` / `ToolPartView` |
| `src/components/agent/tool-cards.tsx` | 轮已终结时未配对部件的 `TraceLine` 不再 `running`（四处同形状）——症状 2 |
| `src/components/agent/agent-messages.tsx:67` | `isLiveAsk` 兜底分支从 `output === null` 改为「该轮 `status === 'running'`」——症状 3；spec 里「MUST NOT 以工具结果缺席即认为在等」从纪律变成结构保证 |
| `src/views/agent/components/agent-run-history.tsx:53` | `statusLabel` / 颜色增 `interrupted`（「已中断」） |
| `src/lib/agent-transport.ts:344` | 订阅起点播种键改为「首个实时事件自证 `turnId`」——症状 1；见下条保留理由 |

**播种修复为何仍在范围**：标定调用继承 `let _ = store.calibrate_interrupted_runs(...)`（`store.rs:428`）的
吞错写法，标定失败即静默 → 残留行存活 → 播种仍选错轮 → 历史仍被冲。**播种修复是唯一不依赖标定成功的
防线**，且改动只有几行（`detail.turns` 只当「有没有 running 行」的状态门，身份交首个实时事件）。

### 4.4 spec 修订面（5 条，零明文反转）

| spec | requirement | 类型 | 内容 |
|---|---|---|---|
| desktop-workspace-store | AgentRunRecord 状态字段类型化 | MODIFIED | 值域增 `interrupted`（TS union / zod 随之） |
| desktop-workspace-store | 会话查询与对账 API | MODIFIED | 新增 agent 轮标定；触发点精确表述为「所属 workspace 库首次打开」；幂等 / 记因定式 |
| desktop-agent-execution | ask 工具协议与阻塞应答通道 | MODIFIED | 补留痕：停等态非 durable；进程中断后该轮由启动标定终结，应答 SHALL `Err` |
| desktop-agent-chat-infra | 订阅起点部件重建 | MODIFIED | 播种键 = 首个实时事件自证；`detail.turns` 只作状态门（MUST NOT 用作选轮依据） |
| desktop-agent-chat-infra | ask 活卡判据 + 轮内状态行 | MODIFIED | 活卡判据读轮状态；新增 `interrupted` 状态行；轮已终结时未配对部件 MUST NOT 呈运行态 |

### 4.5 测试面（7 处）

| 文件 | 关键用例 |
|---|---|
| `crates/infra/store/src/store_test.rs` | 标定只动 running / 记因文案 / 幂等 / 不动已终态 / 不误伤 change run 行 |
| `crates/infra/store/src/model_test.rs` | `interrupted` 值域往返（bincode 新 tag）+ 旧行解码不变 |
| `crates/core/orchestration/src/walker_test.rs` | `worker_settle` 对 `Interrupted` 的终态映射 |
| `src-tauri/src/commands/exec/mod_test.rs` | `session_detail` 返 `interrupted` 轮行；`agent_answer` 对已标定会话 → `Err` |
| `src/lib/agent-transport.test.ts` | **残留行在前 + 直播轮在后的回归**：增量到达后直播轮已落库部件仍在（当前实现必红） |
| `src/hooks/use-agent-session.test.ts` | `interrupted` 不派生 `running`、不触发订阅 |
| `src/components/agent/{tool-cards,agent-messages}.test.tsx` | 终结轮未配对工具卡不呈 `running`；失效 ask 卡零应答入口；`interrupted` 状态行 |

### 4.6 已定裁量

- **`AgentRunStatus::Interrupted` 新值**，不自造「`stopped` + 记因」。理由：语义即「中断」；
  `RunStatus::Interrupted` 有同义先例；新值令 `worker_settle` 穷尽 match 编译报错，强制 flow 侧显式裁。
- **`interrupted` 不进失败横幅**（默认取），只走轮内状态行 + 历史列表「已中断」。若需更醒目
  （如启动时一次性提示），属纯前端取舍。

### 4.7 规模

1 个枚举值 + 1 个 store 函数 + 11 个消费点 + 5 条 spec delta；
**不动 kernel 语义、不动命令签名、不动 `loop`、不反转任何既有明文。**

## 五、显式不在本次范围

- **会话接续**（`resume_turn` / loop 第三入口 / cwd 入档）——本次只处理「ask 跨进程失效」这一错误面；
  中断的轮按客观终止处理，用户重发即可；
- change flow 停等态落库（需 walker 可恢复状态机）；
- ask 取消通路（`declined` 预留位照旧不开）。

## 六、关键文件索引

- 标定先例：`crates/infra/store/src/store.rs:425-430`（`open_workspace`）、`:2105`（`calibrate_interrupted_runs`）
- 轮状态：`crates/core/agent/src/runner.rs:30`；内核收口 `crates/core/agent/src/kernel.rs:337-345`
- flow 侧映射：`crates/core/orchestration/src/walker.rs:3280`
- 三个内存注册表：`crates/core/agent/src/ask.rs`、`kernel.rs:20`（`StopRegistry`）、`stream.rs`
- 启动托管：`src/main.rs:100-135`（`setup_hook`）
- 命令面：`src-tauri/src/commands/exec/mod.rs:195`（`agent_stop`）、`:212`（`agent_answer`）
- 前端租户：`src/lib/agent-transport.ts:344`、`src/components/agent/agent-messages.tsx:67,188`、
  `src/components/agent/tool-cards.tsx:220`、`src/lib/agent-virtual.ts:52`、`src/lib/agent-error.ts:134`
- 中断调用的合成结果（既有兜底，本次不动）：`crates/infra/agent/src/sdk/resume.rs:13-15`（`INTERRUPTED_RESULT`）
