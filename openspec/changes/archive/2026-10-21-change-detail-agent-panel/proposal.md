# 提案: change-detail-agent-panel

> **变更**: change-detail-agent-panel
> **日期**: 2026-10-20
> **状态**: draft

---

## 问题

需求原文（`openspec/todo.md`）：「【Desktop】change 详情增加通用 agent，在 change 环境下运行；通用 agent 可以开启新会话，恢复历史会话」。三条硬需求对应三处真实缺口，前两处各自卡在一个结构性判断点上：

1. **「在 change 环境下运行」没有执行根通道（最贵的一处）**。`agent_start` 的 `root` 一参数双职：既作 store 身份锚（`stores.for_root(&root)`）又作会话 cwd（`SessionCtx.workspace_root`，决定工具路径沙箱根 / bash `current_dir` / 每轮重读的 `AGENT.md`）。change 流程侧已是「双 root 拆分」——`exec_root = record.worktree ?? workspace_root` 只进 `SessionCtx`，store 半边恒 workspace root（`src/commands/change_flow/mod.rs:111-118`，design D9）；通用 agent 面完全没有这条通道。**「把 worktree 当 `root` 传」不是可行折中**：`for_root` 对任意路径都会新建 workspace db（无注册校验），chat 会话会静默落进第二份库，从 workspace 侧永远查不到——这是一次不可见的数据分裂，不是可接受的副作用。
2. **会话归属缺写侧，读侧却已就绪**。`agentSessions(root, source, sourceRef, …)` 的后端过滤是 `source_ref` **精确相等**（已实现），但前端两处把 `sourceRef` 硬编码为 `null`（`lib/agent-transport.ts` 的发起 invoke、`hooks/use-agent-session.ts` 的发送 body），且 `agent-transport` 的会话参数集压根没有读 `sourceRef` 的位置。也就是说「按 change 圈出会话」只差写侧两行透传。同时 `ChangeRecord` **零会话字段**（change 域会话 id 全在 `RunNodeRecord` / `PhaseRecord` 旁支表），故「当前会话指针」不能像 explore 那样从记录直取——而 explore 是 1:1、change chat 是 1:N，那套单指针也承载不了集合。
3. **详情页没有右侧/侧栏先例，且既有抽屉会挡住并读**。`change-detail-view` 是纵向单列（header + 告警 + 动作面板 + `flex-1` 流程区），加侧栏必须改成横向 split；而现有右侧抽屉是**模态浮层**（`fixed inset-0` + 遮罩），一旦常驻面板落位，点节点看转录时左侧 chat 会被遮罩整片挡住——两个 agent 会话无法并读。
4. **禁发/只读语义无处承载**。`AgentInput` 只有 `running`（禁发送按钮 + 呈现停止入口），没有「整体不可发送并给出理由」的态；而本面板需要两类新禁发：流程 run 期间（同 worktree 并发写）与归档 change（worktree 已被 Cleanup 移除）。

用户已拍板五问，其中两个决定实现形状：**change 环境 = worktree 为 cwd**（⇒ 必须动 `agent_start` 参数面，做双 root 拆分）；**不自动恢复上次会话**（⇒ 不需要 `ChangeRecord.chat_session_id`，零 store 模型变更）。

## 提案

一条 cwd 通道 + 一条归属写入 + 一个薄清单命令 + 一个独立面板 + 一张禁发矩阵，全部落在既有会话基建与既有分栏范式上：

1. **`agent_start` 尾部追加 `cwd: Option<String>`（位置索引 8）**：`cwd` 只喂 `SessionCtx.workspace_root`（`None` → `root`，既有消费面零语义变化），`root` 的 store 半边恒定 workspace root。`cwd` 非空时做 `is_dir` 探测 + 显式 `Err`（复用 change_flow 的「worktree 目录不存在（可能已被手动删除）」文案，避免错误深埋进引擎工具调用）。`agent_start_with` 泛型测试缝同步；`AgentStartArgs` 位置魔法与 bindings 快照断言随动（`cwd` 在末位意味着既有 1–7 位零漂移）。
2. **会话归属：`source='change'` + `source_ref=<change id>`（两段）**。流程会话的 `source_ref` 是四段 `{changeId}/{phase}/{role}/{attempt}`，精确匹配下与 chat 的两段天然不撞——chat 会话能被 change 圈出，流程会话不会混进 chat 清单。前端把硬编码的 `null` 换成来源侧传入：`use-agent-session` 参数面增 `sourceRef?` 与 `cwd?`，`sendMessage` body 透传，`agent-transport` 的 `readSessionParams` 增读两个字段并按具名解构调用 `agentStart`（顺带把位置索引魔法改具名，避免下次加参数错位）。
3. **读侧新增薄命令 `list_change_chat_sessions(root, id, offset, limit) -> SessionPage`**：等价 `session_query(source="change", source_ref=<id>)` 的分页清单，`source` 词常量收在命令层（前端零词汇），`all_commands!` 登记 + bindings 再生成。**零 store / 零 native model 变更**（`ChangeRecord` 不动，避免 v4→v5 升级链 + 双向 From + DTO 四层穿透）。选中历史会话后一律 `sessionDetail` / `agentSessionTranscript` 以 **sessionId 直查**重放——与既有 spec 的「前端寻址 MUST NOT 经 source / source_ref 反查」不冲突（清单筛选是探测语义，与调试页来源筛选同款，spec 明文允许）。
4. **面板：`views/changes/agent/`（新目录）三件套**：`change-chat-panel.tsx`（会话头 + 「新会话」+ 「历史 ▾」清单 + 禁发/只读提示行）＋ `change-chat-composer.tsx`（`AgentInput` 薄适配，`idPrefix="change-chat"`）＋ `hooks/use-change-chat.ts`（当前 `sessionId` 本地态 + 清单取数 + 新会话语义）。对话体复用 `AgentSessionView` **注入形态**（来源侧建 `useAgentSession`，composer 走插槽），零第二套渲染。**打开即空态且空态可发送**：`sessionId = null` 即「新会话草案态」，与调试页 `useAgentSession({ sessionId: null })` → 发送即 `New` 的既有语义同源（`use-agent-session` 的发送组装本就以 `mirrors.currentSessionIdRef.current ?? sessionId` 取会话引用，null 即 New；会话 id 由后端铸造并经 early-resolve 轮行回写），故空态 MUST NOT 禁用输入 / 发送——否则「新会话 → 首发」不可达。首条消息才真正铸会话行（无空会话堆积），轮行确立会话 id / 首轮收口时重取清单使新会话可重开；「新会话」= 本地 `reset()` + `sessionId = null`（回到同一可发送空态，旧会话仍在历史清单可重开）；历史点选 = `setSessionId` 交给重放装载。面板态（当前会话 / 折叠 / 清单展开）挂 URL id，id 变化即重置。
5. **布局：左 agent + 右详情，与探索页同构**。`DetailLoaded` 的纵向单列改 `ResizablePanelGroup orientation="horizontal"`：左 `ResizablePanel`（`defaultSize 30` / `minSize 22` / `collapsible`，收起留窄条 + header 显隐开关），右栏内保持「stale note → 动作面板 → 流程图」原序。`change-detail-view.tsx` 已 388 行且 `max-lines-per-function` 50/80 ⇒ 面板与 hook 必须独立成文件，视图只做装配。**右侧抽屉保留**（读法 A，不改「单一交互入口」的内容分派），但改**非模态**：`StandardDrawer` 增 `modal?: boolean`（缺省 `true`，既有调用零回归），`DetailDrawer` 传 `modal={false}`——`components/ui/drawer.tsx` 本就在 `modal === true` 才渲染遮罩、非模态时 Viewport 不吃指针事件，故左侧 chat 全程可交互、抽屉仍在右侧浮层且仍由 `selection !== null` 驱动。代价是「点遮罩关闭」不再存在，关闭走页脚「关闭」按钮 / Esc（spec 该条款随之修订）。
6. **禁发与只读矩阵（面板唯一判据源）**：

   | `detail.source` | `activeRun` | 本会话 `running` | 可发 | 可开新会话 | 可切历史 |
   | --- | --- | --- | --- | --- | --- |
   | `'active'` | 非 null | — | ✗ | ✗ | ✓ |
   | `'active'` | null | true | ✗ | ✗ | ✓ |
   | `'active'` | null | false | ✓ | ✓ | ✓ |
   | `'archive'` | — | — | ✗ | ✗ | ✓ |

   实现口径：「运行中禁发」= `activeRun !== null` 与 `session.running` 取或；「只读」= `detail.source === 'archive'`；`AgentInput` 增 `disabled` + 禁用理由文案（"流程运行中" / "已归档 change 只读"）。**`sessionId === null` 不参与判据**：矩阵只看 `detail.source` / `activeRun` / 本会话 `running`，故空态在可发送行下照常可发（首发即 `New`），在禁发行下与既有会话同样禁用——MUST NOT 「空态一律禁发」（那会让「新会话 → 首发」不可达，就与调试页 `sessionId: null` 即可发送的既有语义相反），也 MUST NOT 让空态豁免 run 期 / 归档的禁发。禁发态连「新会话」一并禁，避免「开了新会话还是发不出去」的半可用态。
7. **不做自动恢复，也不碰「实时面」既有纪律**：面板的交互式会话**不接** detail 的 `refreshTick` 当 `refreshKey`（重放 effect 以 `refreshKey` 为依赖、无在挂流时整轮重放 `sessionDetail` + 全量转录；run 期间 `refreshTick` 由每条密封事件驱动，接上去就是一轮几十次的重放风暴）。会话状态重评估由 `useAgentSession` 自身的订阅面承担（与 explore 注入形态一致）；只读抽屉继续收 `sessionRefreshKey`（它没有自己的流）。

**不在本变更内**（explore 已明确出范围，留风险留痕）：change 提示词前导模板（`lib/change-stance.ts` 类 explore stance 的对称物）；调试页续话的 cwd 兜底（按 `provenance.source_ref` 反查 change worktree）。工具面零改动——`compose` 只特判 `source == "explore"`，change chat 自然拿到 default 七工具（含 write，符合「在 worktree 里干活」）。

## 能力

一个新增能力 + 三个既有能力修改（新关注点用 ADDED，既定表述被推翻处用 MODIFIED）：

| 能力 | delta | 内容 |
|------|-------|------|
| desktop-change-chat（新） | 4 ADDED | 左栏通用 Agent 面板（分栏骨架 / 折叠 / 会话头动作 / 面板态挂 change id）；会话归属与清单（`source='change'` + `source_ref=<id>`、薄清单命令、sessionId 直查重放、打开即空态（可发送草案）不自动恢复、新会话语义、首发后清单可见）；change 环境边界（cwd 恒 `detail.worktree ?? root`、worktree 缺失回落、沙箱根与 AGENT.md 随 cwd）；禁发与只读矩阵（三态判据 + `disabled` 理由，`sessionId === null` 不作禁发理由） |
| desktop-agent-execution | 2 MODIFIED | 命令面：`agent_start` 尾部 `cwd` 参数与双 root 不变量、`source_ref` 由「恒传 null」修订为「可携来源引用（仅归属与清单筛选，MUST NOT 用于寻址/重放）」、新增 `list_change_chat_sessions` 薄命令；会话域内核公共 API：清单筛选（探测语义）显式允许，寻址/重放禁令不变 |
| desktop-agent-chat-infra | 2 MODIFIED | 会话状态基建：会话参数面增 `cwd` / `sourceRef` 透传（transport 具名解构 + body 读取）；展示组件族：`AgentInput` 增 `disabled` 与禁用理由文案（`running` 语义不变） |
| desktop-change-flow-view | 1 MODIFIED | 右侧抽屉单一交互入口：抽屉改非模态形态（无遮罩、关闭经页脚按钮 / Esc、右侧浮层与内容分派不变），使左侧 chat 可并读 |

## 变更范围

### 实现文件

后端（`packages/desktop/src-tauri`）：

- `src/commands/exec/mod.rs` — `agent_start` / `agent_start_with` 增 `cwd: Option<String>`（末位）；`is_dir` 校验 + 显式 `Err`；`SessionCtx.workspace_root = cwd.unwrap_or(root)`，`stores.for_root(&root)` 恒不动；新增 `list_change_chat_sessions`（薄包装：`source` 词常量 + `source_ref = id` → `SessionPage`；blank root → 空页）
- `src/commands/mod.rs` — `all_commands!` 登记 `list_change_chat_sessions`
- `src/bindings/mod_test.rs` — `agentStart` 绑定签名逐字断言（尾部 `cwd`）+ 新命令清单断言随动；`src/types/generated/bindings.ts` 经 `pnpm run bindings:export` 再生成（`bindings:check` 守卫）

前端（`packages/desktop/src`）：

- `lib/agent-transport.ts` — `AgentStartArgs` 位置魔法（1/2/3/4/5/7）改具名解构；`AgentStartSessionParams` 增 `cwd` / `sourceRef`；`readSessionParams` 增读两字段（`readNullableString` 复用）；`invokeStart` 追加 `cwd`
- `hooks/use-agent-session.ts` — `UseAgentSessionParams` 增 `cwd?: string | null` / `sourceRef?: string | null`；`sendMessage` body 透传（原 `sourceRef: null` 常量退役）
- `views/changes/agent/change-chat-panel.tsx`（新）— 面板骨架：会话头（「新会话」/「历史 ▾」/ 提示行）、`AgentSessionView` 注入形态 + composer 插槽、空态提示行（发送即开新会话，不做禁用门）、折叠条
- `views/changes/agent/change-chat-composer.tsx`（新）— `AgentInput` 薄适配（`idPrefix="change-chat"`、禁用理由透传）
- `views/changes/agent/hooks/use-change-chat.ts`（新）— 当前 `sessionId` 本地态（`null` = 可发送空态，不作禁发理由）+ `list_change_chat_sessions` 清单取数（含首发后重取）+ 新会话（`reset()` + 置 null）/ 历史切换 / 面板态重置；`cwd` 派生（`detail.worktree`），禁发判据（`activeRun` / `running` / `detail.source`）
- `components/agent/agent-input.tsx` — 增 `disabled?: boolean` + `disabledReason?: string`（textarea 禁用、发送禁用、理由文案呈现；`running` 语义不变）
- `components/agent/agent-session-view.tsx` — 注入形态继续由来源侧建 hook（本变更零契约改动；如需把 `disabled` 经 `AgentSessionActions` 传出，仅扩插槽入参）
- `views/changes/change-detail-view.tsx` — `DetailLoaded` 纵向单列改 `ResizablePanelGroup`（左 agent 面板 collapsible / 右 stale note + 动作面板 + 流程图）；`DetailHeader` 增面板显隐开关
- `components/standard/drawer.tsx` — 增 `modal?: boolean`（缺省 `true`，透传 `Drawer`）
- `views/changes/flow/detail-drawer.tsx` — 传 `modal={false}`（并读所需；内容分派与宽度不变）
- `packages/desktop/package.json` — `version` 0.5.19 → 0.5.20

### 测试文件

后端：

- `src/commands/exec/mod_test.rs` — `agent_start_with` 的 `cwd` 面（`None` 回落 root / `Some` 进 `SessionCtx` / 非目录显式 `Err` / store 半边恒 root——断言落在 workspace 库而非 cwd 派生库）；`list_change_chat_sessions`（chat 归属命中 / 流程四段 sourceRef 不误入 / blank root 空页 / 分页信封）
- `src/bindings/mod_test.rs` — 绑定签名与命令清单断言随动（既有逐字断言改后仍锁形状）
- `crates/infra/store`（若补「两段 vs 四段不撞」的固化断言）— `query_sessions` 精确匹配语义的直测

前端：

- `lib/agent-transport.test.ts` — `cwd` / `sourceRef` 读出与透传、非法值拒绝、具名解构后位置参数仍按序到达
- `hooks/use-agent-session.test.ts` — `sourceRef` / `cwd` 进入发送 body（原 `null` 常量断言改写）
- `views/changes/agent/change-chat-panel.test.tsx`（新）— 空态（输入与发送可用：首条消息以 `sessionId: null` 发出 → 铸会话；空态下 run 期 / 归档仍禁用）/ 新会话重置（回空态仍可发）/ 历史清单点选切会话 / 禁发与只读提示 / 折叠与展开
- `views/changes/agent/hooks/use-change-chat.test.ts`（新）— 清单取数（按 change id、分页、首发后重取）/ 新会话清 `sessionId` 且不禁发 / id 变化重置 / 禁发判据矩阵四行（含空态在禁发行同样禁用）
- `components/agent/agent-input.test.tsx` — `disabled` + 理由文案（textarea 与发送同时禁用）
- `views/changes/change-detail-view.test.tsx` — 分栏骨架（左面板 / 右详情并存）、header 显隐开关；既有抽屉挂载 / 卸载断言保持不变
- `views/changes/flow/detail-drawer.test.tsx` — 增 `modal={false}`（`data-modal="false"`、无遮罩节点）断言；内容分派既有断言不变

### 不要修改

- `crates/infra/store` schema 与 `ChangeRecord` 模型（本变更零 native model 升级、零 store 迁移；`desktop-workspace-store` 零 delta）
- `crates/core/agent` / `crates/infra/agent`（会话内核、引擎、工具面、`compose` 注入装配、沙箱与 AGENT.md 注入逻辑零改动——`cwd` 只改壳命令层喂给 `SessionCtx` 的值；change chat 走 default 七工具，不新增 `SessionToolset` 变体）
- change 流程编排面（`commands/change_flow/**`、walker、`RunUpdate` / `RunNotice` 变体集、`ChangeFlowControl`）：本变更只读 run 活面，不改 run 语义
- `views/changes/flow/` 流程图转换层与节点渲染（`graph.ts` / `run-state.ts` / `attachments.ts` / `run-step-node.tsx` / `change-flow-graph.tsx`）与抽屉内容分派（`session-transcript-panel.tsx` 的只读形态）
- `views/changes/hooks/use-change-detail.ts`（通知去抖 / 失败降级链）与 `use-change-flow-run.ts`（订阅生命周期）
- `components/ui/drawer.tsx`（非模态能力已存在，只在上层 `StandardDrawer` 透传）
- `views/agent/**`、`views/explores/**`（调试页与探索页消费面零改动；`sourceRef` 缺省仍走 `null` → 行为不变）
- `plugins/dev-team/**`（插件仓已弃用，零改动）与仓库根插件版本

## 验收标准

1. **环境落到 worktree**：非归档且带 worktree 的 change，面板发出的会话以 `detail.worktree` 为 cwd（工具路径沙箱根 / bash `current_dir` / 每轮 `AGENT.md` 读取点均在 worktree 内可验），而会话行落在 **workspace 库**（同一 root 的 `agentSessions` 可查、workspace 目录下无第二个 `workspaces/*.redb` 新生）。
2. **双 root 不变量可验**：`cwd` 传非存在的目录 → 发起点显式 `Err`（文案含路径），不进入引擎、不产生会话行；`cwd` 省略（调试页 / explore 链）→ 行为与变更前逐字一致。
3. **归属与清单**：面板发出的会话在库内 `source='change'`、`source_ref=<change id>`；`list_change_chat_sessions` 只返回该 change 的 chat 会话（流程四段 `source_ref` 一条不漏地不出现）；调试页 `change` 来源筛选照常能看见这些会话。
4. **新会话 / 历史会话**：打开面板即空态（zero 会话行、zero 取数）**且可发送**——空态发出首条消息时 invoke 的 `sessionId` 为 null（`New` 语义），会话行在此刻铸造并带 `source='change'` / `source_ref=<change id>`，面板随后以该会话呈现且它出现在历史清单；「新会话」在已开会话情形下 `reset()` 回空态且**立即可再发**，旧会话仍可从历史清单重开；历史点选后对话与元信息经 `sessionDetail` + 转录取数直查得到（invoke 断言：选中路径零 `source`/`source_ref` 反查）。
5. **不自动恢复 + 无重放风暴**：页面刷新后回到空态（不自动选最近一条）；一次 flow run 全程面板的 `session_detail` / 转录 invoke 次数不随 `refreshTick` 增长（重放只在 `sessionId` 变化或收口收敛时发生）。
6. **禁发与只读**：矩阵四行逐行可验——run 期间与归档 change 下输入框与发送均禁用且呈理由文案、「新会话」禁用、历史切换仍可用；无 run 的 active change 正常收发，**空态亦照常可发（首发即 `New`）**，且 `sessionId === null` 在任何行都不单独构成禁用理由（空态在两行禁发态下与既有会话同样禁用，不豁免）。
7. **并读成立**：点开节点 / 列头后右侧抽屉为**非模态**（页面无遮罩节点、`data-modal="false"`），左侧 chat 输入与发送全程可交互；抽屉仍是唯一交互入口（列头 / 节点同入口、内容分派不变），关闭经页脚「关闭」按钮或 Esc。
8. **全管线绿**：`pnpm -C packages/desktop run check`（含 `bindings:check` 守卫）与 `run test` 全绿；bindings 已重导入库；desktop 版本 0.5.20。

## 风险

1. **store 身份分裂（最高，一次事故就不可见）**：`for_root` 对任意路径都会新建 workspace db。若实现时把 worktree 当 `root` 传（或让 `cwd` 参与 `for_root`），chat 会话会静默落进第二份库、workspace 侧永远查不到，而 UI 无任何异常。缓解：spec 明文双 root 不变量 + 命令层测试直断「会话行在 root 库、cwd 路径不派生新库」；代码路径上 `cwd` 只出现一次（赋 `SessionCtx.workspace_root`）。
2. **重放风暴（最易漏的性能坑）**：`useAgentSession` 的重放 effect 以 `refreshKey` 为依赖，而 `use-change-detail` 的 `refreshTick` 在 run 期间由每条密封事件驱动（300ms 去抖）。把 `refreshTick` 传给 chat 会话 ⇒ 一轮几十次全量重放。缓解：面板**不传** `refreshKey`（只读抽屉继续传 `sessionRefreshKey`）；验收标准 5 以 invoke 计数固化。
3. **`cwd` 未落 `SessionRecord.config_snapshot`（已知缺口）**：调试页按 `change` 筛选打开同一 chat 会话续话时不会带 cwd，工具跑回主 root（口径不一致但不报错）。缓解：来源侧恒带 cwd（change 详情页恒取 `detail.worktree`）；按 `provenance.source_ref` 反查 worktree 的隐式兜底显式留作后续统一，并在 spec 记入。
4. **沙箱根收缩的语义边界**：cwd = worktree 时全部路径工具被限在 worktree 内，看不到主工作区未提交改动。这是「change 环境」应有的边界而非缺陷，但用户可能预期「能看到主仓正在改的东西」。缓解：面板提示行点明执行根（worktree 路径），文案不承诺跨根可读。
5. **归档 change 的悬空 worktree**：归档 Cleanup 只 `remove_worktree`、`ChangeRecord.worktree` 不会被清空 ⇒ 归档后该路径恒缺失。缓解：档案 change 一律只读（禁发 + 禁新会话），且 cwd 派生做 `is_dir` 回落（worktree 不存在 → 不发起点 Err，直接进只读态）；仅历史浏览可用。
6. **run 期并发写同一 worktree**：flow executor 与 chat agent 同目录改代码，无锁无互斥。本变更只以「运行中禁发」覆盖 run 期（`activeRun !== null`）；**非 run 期**用户手动开新会话与 flow 交错仍无保护——文档 / 提示行留痕，不声称已互斥。
7. **非模态抽屉的交互退化**：无遮罩即无「点遮罩关闭」（改页脚按钮 / Esc），且 960px + `max-w-[85vw]` 在窄窗口仍可能盖住左侧 chat（读法 A 的既定取舍）。另需落地时验证：base-ui Drawer 在 `modal={false}` 下 Esc 是否仍能关闭（预期由 dismiss 逻辑支持；若不支持则仅剩页脚按钮，可接受）。缓解：`StandardDrawer` 的 `modal` 缺省 `true`，既有调用零回归；`detail-drawer` 增测试断言 `data-modal="false"`。
8. **归属不撞性依赖约定而非机制**：chat 用两段 `source_ref`、流程用四段，精确匹配下不撞——但这是**字面约定**，未来若流程改 `source_ref` 形态即可能串味。缓解：落地补一条 store 侧直测（同一 change 下两类 `source_ref` 互不命中）把不撞性固化为断言。
9. **bindings 与位置参数面的漂移**：`cwd` 加在末位才保证既有 1–7 位不漂；`agent-transport` 的位置索引魔法（1/2/3/4/5/7）本就脆，本次改具名解构顺带消险，但改错即错位读参（类型不报错时最难查）。缓解：`bindings:check` + transport 单测直断「两字段按序到达 invoke」。
10. **空态首发的会话 id 与清单可见性时序**：空态发送时 `sessionId` 为 null，会话 id 由后端 early-resolve 轮行回写进 `useAgentSession` 的镜像（`currentSessionIdRef`），面板本地 `sessionId` 仍是 null——若不在会话 id 确立 / 首轮收口时重取清单，新铸会话不会出现在「历史 ▾」里，用户会以为会话丢了；同理面板头区若只凭本地 `sessionId` 判空态，会出现「正在聊却显示空态」的假象。缓解：`use-change-chat` 在轮行确立会话 id 与轮收口两处重取清单；「当前会话」回显取会话镜像（`session?.row.id`）而非本地 `sessionId`；首发只在 `sessionId` 仍为 null 且无在挂流时允许（沿用 `isStreaming` 忽略重复发送）。
11. **相邻遗留待确认项（不阻塞）**：
    - 面板空态与新会话的视觉细节（头区文案、历史清单展开形态、折叠条宽度）留给 design / 实现定稿，不在 spec 固化像素；
    - change 提示词前导（stance）与「change 环境」的提示词层、工具面层（是否只读 change 环境）显式**出范围**——本变更只落 cwd 层，若产品后续要求「只读 change 环境」需新 `SessionToolset` 变体，另立变更。
