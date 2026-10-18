# 提案: desktop-agent-chat-component

> **变更**: desktop-agent-chat-component
> **日期**: 2026-10-17
> **状态**: draft

---

## 问题

桌面端「按 sessionId 渲染一个 agent 会话」与「按会话状态挂流」两件事都没有单一实现，现状是**三类形态、四个消费点**各自拼装 + 一条物理上不通的实时面：

1. **同一透镜喂三种数据形状**。通用查询接口与对话透镜都已存在（`sessionDetail` / `agentSessionTranscript` / `AgentMessages`），但「取数 + 装配 + 何时订阅」散在各处：
   - A 交互会话（`agent-debug-view` / `explore-detail-view`，经 `useAgentChat`）：`buildMessages`（密封事件 + 轮行交错）+ 自发流独占；
   - B 只读转录（change 抽屉 `useSessionTranscript`）：`eventsToUIMessages` + 通知 → 150ms 去抖重查转录库；
   - C 纯静态（`agent-run-history` 历史重放）：裸 `eventsToUIMessages`，无轮行交错。
   同一透镜被喂三种形状，轮行交错只有 A/B 有，C 缺。
2. **「按会话状态挂流」没有会话级机制**。exec 轨唯一实时出口是 `agent_start` 的 `Channel`（`commands/exec/mod.rs:103-116`），**发起方独占**；agent 会话域无任何广播 / watch 命令（change 域的 `change_flow_watch` 是 per-(root, change_id) 的 kind-only 通知——一次 run 含多会话、且按「通知仅失效信号」设计零载荷），前端零 `@tauri-apps/api/event`（Channel 直投为架构纪律）。旁观视图、页面重挂、第二视图对运行中会话**没有任何出口**。
3. **B 面的「token 级流式」物理上不成立**。`delta` 只上传输面、不落库（`kernel.rs:229` 跳过 `append_sealed`）；而 B 面的实时面是「通知 → 重查转录库」，重查只能看到密封粒度——spec 写的 token 级流式在此面无法成立（`desktop-change-flow-view`「运行中节点实时转录」scenario 与实际能力不符）。
4. **适配层是真实双实现，且「两路同构」是假契约**。`blocksToParts`(:212) 与 `blocksToChunks`(:309) 各写一遍判定；PoC 实测（V2）证明**跨消息 tool 配对两路物理上不可能同形**（reducer 的 `getToolInvocation` fallback 只扫当前消息 parts，折叠路有全局预扫）——这是能力差异不是实现疏漏。现有对拍测试（`agent-adapter.test.ts:536`）的 fixture 恰好处在「能工作」的同消息场景，从未覆盖跨消息分支。
5. **死代码与假契约并存**：`useSessionTranscript` 的 `liveEvents` 参数已无调用方（archive-panel 机制退役）仍带「零触点红线」注释在案；`notifyTranscript` / `transcriptTick` 的 150ms 专用去抖与统一视图 300ms 去抖并行，两套节拍。

用户需求：**前端统一组件，入参 `sessionId`，调通用接口渲染会话；组件内按会话状态（进行中 / 完成）决定是否监听 stream**。

## 提案

一条会话一个 `sessionId`、一个组件、两条实时出口（发起流 / 订阅流）、五条实证约束、一个 IR 层。核心形态七点：

1. **后端会话级实时流订阅面（新）**：`core/agent` 增 per-session 流注册表（键 = core session id，`Arc` 托管于壳层、经组合根注入内核，与 `StopRegistry` 同型），有界 broadcast 承载**载荷直投**事件（含 `messageDelta`）。写入点收在**内核 `drive` 单点**（`KernelOutput::Observation` / `TurnFinished`），exec 轨与 change 轨（walker → `KernelWorkerPort` → `drive`）**同源进入同一注册表，消费方零改动**。条目生命周期 = 轮：`begin_turn` 登记、`converge` 投终态后除名。
2. **新命令 `agent_session_watch(on_event, root, session_id, since_seq)`**：薄包装（注册表订阅 → Channel 前向 → 结束释放），只投递 `seq > since_seq` 的事件，轮终态以 `AgentRunMessage::Record` 收尾并**结束命令**（订阅流按轮划分，不长连接跨轮）；无运行轮 / blank root **幂等立即结束**（与 `agent_stop` miss 幂等同口径）；广播溢出（lagged）以**断流**兜底，不投递伪终态。
3. **发起命令的 Channel 保留**（不二选一）：`agent_start` 的 Channel 先于 invoke 建立 → **发送方零空洞**；订阅面在「发起后才订阅」的窗口里可能丢若干 delta（ephemeral 不可补）。两出口**同源同信封**，前端在 transport 层共用同一套开件簿记（`start` + `reset-step` + part-start）与终态收口。
4. **前端统一组件 + headless hook**：`AgentSessionView`（`components/agent/`，入参 `root` / `sessionId`，可选 `refreshKey` 与 `composer` 插槽）= 唯一渲染入口；`useAgentSession`（`hooks/`，收编 `use-agent-chat`）= 重放装载 + **状态驱动挂流** + 单流在挂 + 收口收敛 + 发送/停止动作面。会话「进行中 / 完成」由通用查询面（running 轮行）推导，**MUST NOT 用发送方视角 `chat.status` 替代库态**。
5. **五条硬约束（PoC 实证，逐条落 requirement）**：① 订阅流按轮划分（收到 `Record` 即关流——长连接会同时杀掉 `onFinish` 收敛闸与 status 复位，V4）；② 订阅带 `since_seq`（多消息 `resumeStream` 会全量重复，V5）；③ 订阅流复刻开件簿记（裸 delta 直投 → `status='error'` 且消息丢失，V6）；④ 先全量重放再续订（`onFinish` 折叠依赖镜像全量，只订阅增量会让收敛闸变成数据丢失器）；⑤ 两路契约降级为**终态同构 + 过程近似**（V2）。
6. **适配层 IR 化（路线②）**：`BlockPlan` + `planBlock`（唯一语义点）+ `plansToParts` / `plansToChunks`（纯机械发射）+ `PairingScope`（`global` 折叠路 / `local` 实时路，差异隔离于此）。9 个差异维度里 8 个是发射格式（压进薄 Emitter），只有 1 个是能力差异（配对视野）。收益是性质变化不是数量：判定集中 1 处、漏改变体**编译报错**、对拍下沉计划层（`shapeOf` 第三份手写投影退役）、跨消息差异显式锁定。**终态一致性由「收口以重放收敛」保证（同一来源），不靠两条实现碰巧一致。**
7. **消费面收编（不保留第二套）**：`agent-debug-view`（交互）、`explore-detail-view`（交互）、`session-transcript-panel`（只读抽屉）、`agent-run-history`（只读重放）全部改经 `AgentSessionView`；`useSessionTranscript` 与 `use-agent-chat` 退役，`notifyTranscript` / `transcriptTick`（150ms 转录去抖）与 `liveEvents` 死参数随之下线——`SessionEvent` 通知降为**会话状态重评估信号**（不再当增量数据源）。

**可探测化护栏（路线③，含取舍）**：③a `auxEventData` / `planBlock` 加 `const _exhaustive: never`（新增变体编译期提醒，现状 `default` 静默降级 `data-raw`）；③c 对拍测试参数化矩阵（4 变体 × 边界 × 跨消息视野差异快照）；③d `check` 改走 `bindings:check`（导出 + `git diff --exit-code`），现状 `check` 走就地 `bindings:export` → 生成物永远最新、tsc 从不报漂移。**出范围**：③b（ephemeral 集由 bindings 派生，需 Rust 侧导出 ephemeral 名单）与 ③e（Rust 出真实 JSONL fixture）——收益低于其耦合面，留待后续独立变更（风险 8）。

**附带效应（正向）**：订阅 / 重挂期间 `status='streaming'` → `running` 自然为真 → 既有 `sendMessage` 守卫自动生效，「旁观时误发 Continue」风险消失（此前 `useAgentChat.running` 是发送方视角，旁观恒 idle → 守卫不拦）。

**实施顺序（每步可独立合入 / 回滚）**：

```
步 1  后端：流注册表 + kernel 单点发布 + agent_session_watch + bindings
步 2  前端：use-agent-session 收编 + AgentSessionView + transport 订阅流
步 3  消费面迁移：debug / explore / change 抽屉 / 历史重放；退役旧 hook 与去抖链
步 4  适配层 IR：BlockPlan + planBlock + PairingScope + 两 Emitter（先新增后切换）
步 5  护栏与测试重塑：计划层对拍 + 视野差异快照 + 穷尽守卫 + check 接 bindings:check
```

**留给 design 定稿（不阻塞 proposal）**：注册表模块名与 broadcast 容量常量取值；订阅面是否采用「先补发库内密封快照、再续接 live」（两种实现均须满足 `seq > since_seq` 不重投、不成洞）；composer 插槽形态（ReactNode vs render prop）；`AgentSessionView` 的 `data-testid` 挂钩命名。

## 能力

全部为既有能力修改（无新增能力；新关注点用 ADDED，既定表述被推翻处用 MODIFIED）：

| 能力 | delta | 内容 |
|------|-------|------|
| desktop-agent-chat-infra | 1 ADDED + 4 MODIFIED | ADDED：会话级统一渲染组件（`AgentSessionView` + `useAgentSession`，sessionId 入参 + 状态驱动挂流 + 单流在挂 + 收口收敛 + 断流重订 + composer 插槽）；MODIFIED：统一会话状态基建（hook 收编改名、实时双起点、终态以重放收敛）、事件适配层（IR 化三层 + 终态同构/过程近似 + 视野隔离 + 穷尽守卫 + 分层测试）、展示组件族（新增会话级入口、三形态退役）、ai-sdk 收口（基建文件清单） |
| desktop-agent-execution | 1 ADDED + 3 MODIFIED | ADDED：会话级实时流订阅面（注册表 / 写入单点 / `agent_session_watch` / `since_seq` / 按轮收流 / 幂等 / 滞后兜底 / delta 零落库）；MODIFIED：run 事件落库与重放（投递面「持久面 + 实时面」两出口同源）、agent 执行命令面（新增订阅命令、发起命令 Channel 保留）、Agent 调试页（经会话级组件、历史会话运行中可旁观流式） |
| desktop-explore-page | 1 MODIFIED | 对话区呈现：经 `AgentSessionView`；实时流双起点（发送期发起流 / 重挂期订阅流）；重开时运行中续接 |
| desktop-change-flow-view | 1 MODIFIED | 节点会话转录联动：运行中经会话订阅面 token 级流式（不再「通知 → 重查转录库」当实时面）；通知仅作状态重评估；收口以重放收敛 |
| desktop-ipc-type-bindings | 2 MODIFIED | tauri-specta 接入（命令清单以 `all_commands!` 单一登记面为准、新命令 Channel typed）、生成物入库与一致性守卫（`check` 改走 `bindings:check` 守卫） |

## 变更范围

### 实现文件

后端（`packages/desktop/src-tauri`）：

- `crates/core/agent/src/` — 新增会话流注册表模块（per-session 有界 broadcast、订阅 / 发布 / 除名；模块名与容量常量由 design 定稿）+ `lib.rs` re-export
- `crates/core/agent/src/kernel.rs` — `SessionKernel` / `RunningTurn` 注入注册表；`begin_turn` 登记；`drive` 内 `Observation` 单点发布（含 delta）；`converge` 投终态并除名
- `crates/infra/agent/src/compose.rs` — `compose_turn` 增注册表入参（沿 `StopRegistry` 注入先例）
- `src/commands/exec/mod.rs` — 新增 `agent_session_watch`（薄包装 + Channel 前向 + lagged 断流 + 幂等结束）
- `src/commands/mod.rs` — `all_commands!` 登记新命令
- `src/main.rs` — `manage(Arc::new(SessionStreamRegistry::default()))`
- `src/types/generated/bindings.ts` — 再生成（新命令 typed 包装）

前端（`packages/desktop/src`）：

- `lib/agent-adapter.ts` — IR 化：`BlockPlan` / `planBlock` / `PairedResults` / `PairingScope`（global·local）/ `plansToParts` / `plansToChunks`；`auxEventData` / `planBlock` 穷尽守卫；旧双实现删除
- `lib/agent-transport.ts` — `reconnectToStream` 实现订阅流（`body` 透传 `root` / `sessionId` / `sinceSeq`，无运行轮返回 `null`）；`sendMessages` 与订阅流共用开件簿记与终态收口单点
- `hooks/use-agent-session.ts`（新，收编 `use-agent-chat.ts`）— 重放装载 + 状态驱动挂流 + 单流在挂 + 收口以重放收敛（失败回落镜像折叠）+ 断流重订 + 发送 / 停止
- `components/agent/agent-session-view.tsx`（新）+ `components/agent/index.tsx` 导出
- `views/agent/agent-debug-view.tsx`、`views/agent/components/agent-run-history.tsx` — 改经 `AgentSessionView`（交互 / 只读两形态）
- `views/explores/hooks/use-explore-session.ts`、`views/explores/explore-detail-view.tsx`、`views/explores/components/explore-composer.tsx` — 改经 `useAgentSession` + 组件插槽
- `views/changes/flow/session-transcript-panel.tsx`、`views/changes/flow/detail-drawer.tsx` — 改经 `AgentSessionView`（只读），`refreshKey` 语义改为「会话状态重评估」
- `views/changes/hooks/use-change-detail.ts` — `notifyTranscript` / `transcriptTick` 退役，会话事件通知并入统一视图重查 tick
- 删除 `hooks/use-agent-chat.ts`、`views/changes/hooks/use-session-transcript.ts`
- `packages/desktop/package.json` — `version` 0.5.16 → 0.5.17；`check` 改走 `bindings:check`

### 测试文件

后端：

- `crates/core/agent` — 流注册表单测（订阅 / since_seq 过滤 / 容量与 lagged / 终态除名）+ `kernel_test.rs`（`drive` 单点发布覆盖 delta 与密封、converge 投终态、双轨同源）
- `crates/infra/agent/src/worker_test.rs` — 组合根签名随动（change 轨会话可订阅）
- `src/commands/exec/mod_test.rs` — `agent_session_watch`：旁观订阅 / `since_seq` 过滤 / 终态 Record 收尾 / 幂等无运行 / lagged 断流 / delta 零落库（沿捕获型 Channel 先例）
- `src/commands/exec/agent_test.rs` — 信封与轮行装配随动（如签名有变）

前端：

- `lib/agent-adapter.test.ts` — IR 化重塑：计划层对拍（`plansToParts` vs `reduce(plansToChunks)`，`shapeOf` 退役）+ 视野差异快照（同消息相等 / 跨消息差异锁定）+ 穷尽守卫 + 既有保真场景保留
- `lib/agent-transport.test.ts` — 订阅流（`reconnectToStream`：body 透传、无运行轮返回 null、开件簿记、`Record` 收口）+ 两出口簿记共用
- `hooks/use-agent-session.test.ts`（收编 `use-agent-chat.test.ts`）— 状态驱动挂流 / 单流在挂 / 断流重订 / 收口以重放收敛（失败回落）/ 发送守卫
- `components/agent/agent-session-view.test.tsx`（新）— 挂载渲染 / 完成态零订阅 / 只读形态零动作面 / 空态
- `views/agent/agent-debug-view.test.tsx`、`views/agent/components/agent-run-history.test.tsx`
- `views/explores/explore-detail-view.test.tsx`、`views/explores/hooks/use-explore-session.test.ts`
- `views/changes/flow/session-transcript-panel.test.tsx`、`views/changes/flow/detail-drawer.test.tsx`、`views/changes/hooks/use-change-detail.test.ts`（transcriptTick 退役随动）
- 删除 `views/changes/hooks/use-session-transcript.test.ts`

### 不要修改

- `plugins/dev-team`（插件仓已弃用，零改动；版本保持 2.10.44）
- `crates/core/agent` 的引擎协议词汇（`AgentEvent` 信封、`AgentRunner` trait、run 状态机语义）——本变更只加**传输出口**，不加事件变体、不改落库形态
- `crates/infra/agent/src/sdk/`（SDK 引擎 loop / 工具面 / 上下文防线 / 权限沙箱）与 `crates/infra/agent/src/worker.rs`（change 轨驱动体——写入点在 kernel，worker 零改动）
- store schema 与 `desktop-workspace-store`（会话 / 转录 / 轮统计表零迁移）
- `agent_start` / `agent_stop` / 查询类命令的**参数面与语义**（发起命令的 Channel 保留；不改提前 resolve、不改查询命令）
- change 域 `RunUpdate` / `RunNotice` 变体集与 `ChangeFlowControl` 广播（`SessionEvent` 通知保留，仅消费语义降为状态重评估信号）
- `components/agent/agent-messages.tsx` / `agent-blocks.tsx` / `tool-cards.tsx`（对话透镜与块渲染件保持现状，仅外层新增会话级组件）
- `openspec/explores/`、CLI 工作流与 `workflow.json`（双向墙）

## 验收标准

1. **入参即会话**：以 `root` + `sessionId` 挂载 `AgentSessionView` 即完整呈现该会话（对话 + 轮行 + 会话元信息），四类消费点（debug / explore / change 抽屉 / 历史重放）全部经该组件，视图层零 `eventsToUIMessages` 直调、零自有转录查询 hook。
2. **按状态挂流**：会话有 running 轮行 → 建立恰一条 `agent_session_watch` 订阅并 token 级流式呈现（`messageDelta` 可见）；会话已收口 → **零订阅、零轮询、零定时刷新**（invoke 计数可断言）。
3. **单流在挂**：发送期（发起流在挂）不再建立订阅流，同轮增量只注入一份（无重复文本 / 无重复气泡）。
4. **轮终态收流**：订阅流在该轮 `Record` 后关闭；收口后消息状态与直接重开该会话的重放形状一致（含跨消息 tool 配对收敛为单卡，无双卡残留）。
5. **双轨同源**：change run 的 walker 驱动会话运行中，change 抽屉可旁观 token 级增量（delta 载荷直投），与 exec 轨会话同一订阅命令、同一信封。
6. **幂等与兜底**：对已收口会话 / 未知 id / blank root 调 `agent_session_watch` 立即成功结束（零事件、零副作用）；广播溢出断流后组件重放重订，不出现「丢段仍装作在线」的假态。
7. **适配层契约可验**：计划层对拍（同一 `BlockPlan[]` 两路同形）通过；视野差异快照锁定「同消息相等 / 跨消息具体差异」；`auxEventData` / `planBlock` 穷尽守卫在位；delta 零落库不变。
8. **全管线绿**：`pnpm -C packages/desktop run check`（含 `bindings:check` 守卫）与 `run test` 全绿；bindings 已重导入库且守卫通过；desktop 版本 0.5.17。

## 风险

1. **广播溢出（最高）**：delta 密度高（每 token 一条），有界窗口在长回复 / 慢消费下会 lagged。缓解：容量常量按轮内突发定价（design 定稿，量级 ≥ 2048）、溢出断流 + 重放重订兜底、`Record` 收口保证终态不缺。残留：溢出窗口内的增量文本不可恢复（ephemeral 无库内备份），表现为流式期文本短暂缺片，收口复位。
2. **订阅建立窗口**：`begin_turn` 成功到客户端订阅之间存在毫秒级窗口，期间 delta 丢失（密封事件可由收口重放修复）。缓解：发送方仍走发起流（零空洞）；旁观者以「收口以重放收敛」兜底；缺口只在过程态可见。若设计选择「快照 + 续订」实现，需保证 `seq > since_seq` 不重投（spec 已约束）。
3. **消费面收编的回归面**：4 个消费点 + 约 5000 行既有测试（`use-agent-chat.test.ts` / `agent-transport.test.ts` / `use-session-transcript.test.ts` 三份 700+ 行）需随动；`useSessionTranscript` 删除牵动 change 详情去抖链（`notifyTranscript` / `transcriptTick`）。缓解：分步合入（步 2/3 先组件与抽屉迁移，步 4 再 IR），每步可独立回滚；测试以 data-testid 挂钩，不改产品源码取巧。
4. **IR 化对拍测试重塑**：`shapeOf` 第三份手写投影退役后，跨消息视野差异必须由**显式快照**承担，若只做计划层对拍会把能力差异藏起来。缓解：视野层快照（相等 + 差异）为独立断言组，spec 明文「MUST NOT 由单一路径 fixture 掩饰」。
5. **`StopRegistry` 无重入守卫（既存缺陷，本变更不修）**：`register` 直接 `insert` 覆盖——同一会话并发两轮时第一轮失去停止句柄。订阅面让「旁观 stop」成为可能（与发送方 stop 同键），但并发两轮不在本变更语义内。留痕：若后续需要并发/多观察者停止语义，另立变更加守卫。
6. **`bindings:check` 接入的首次红灯**：启用 `git diff --exit-code` 守卫后，仓库内若已有生成物漂移会立即失败。缓解：实现首个提交先重导 bindings 再启用守卫；`build` 维持导出以免产物构建被守卫阻塞。
7. **ai-sdk 同形巧合**：`auxEventData` 两路共用依赖 ai-sdk 的 data part 与 data chunk 碰巧同形；跨版本升级须复核（spec 已在适配层 requirement 记入依赖约定）。另：半截文本窗口期（旁观中途接入、首个 delta 无前文）V6 只验证「能归约」，未验证视觉连续性——按「过程近似」接受。
8. **留待后续（显式出范围）**：③b ephemeral 集由 bindings 派生（消手写 `SealedAgentEvent = Exclude<…>` 镜像）与 ③e Rust 出真实 JSONL fixture，均需 Rust 侧导出面扩展；本变更以 `bindings:check` + 穷尽守卫 + 分层测试覆盖漂移风险，两项作为后续独立变更候选。
