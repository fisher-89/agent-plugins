# 设计: change-detail-agent-panel

> **变更**: change-detail-agent-panel
> **日期**: 2026-10-20
> **基于**: `proposal.md`、`explore.md` 与四份 spec delta（`desktop-change-chat` ADDED ×4、`desktop-agent-execution` MODIFIED ×2、`desktop-agent-chat-infra` MODIFIED ×2、`desktop-change-flow-view` MODIFIED ×1）

---

## 提案与规格同步状态

`proposal.md` 五问已拍板、「变更范围 - 实现文件」与四份 capability delta 已定稿。本设计只做四件事：把 proposal 的六个落点落成**精确实现形态**（函数签名 / 状态归属 / 取数时机 / 判据单源），把 proposal 未定的接口级细节定稿（`cwd` 空白串口径、清单取数时机与分页、折叠态真相来源、禁用理由优先级、Esc 兜底），按验收标准 AC-1..AC-8 逐条对齐，并把「不做什么」显式留痕（提示词前导、工具面、store 模型、refreshTick 接线）。

起点核实（工作树实况，锚点为改动前）：

- `agent_start` / `agent_start_with`（`src/commands/exec/mod.rs:32-126`）单 `root` 双用：`stores.for_root(&root)`（:97）+ `SessionCtx.workspace_root = Path::new(&root)`（:85-90），无第二参数面。
- `change_flow_start` 的双 root 先例（`src/commands/change_flow/mod.rs:111-127`）：`exec_root = record.worktree ?? root` + `is_dir` 探测 + 文案「worktree 目录不存在（可能已被手动删除）」，store 半边恒 `for_root(&root)`。
- 归属写侧缺两行：`hooks/use-agent-session.ts:459`（`sourceRef: null` 常量）、`lib/agent-transport.ts:311-331`（`invokeStart` 位置索引 `[1][2][3][4][5][7]` + `null` 占位 sourceRef）。
- 读侧已就绪：`agent_sessions`（`src/commands/exec/mod.rs:205-226`）→ `session_query(store).list_sessions(source, source_ref, offset, limit)`；store 侧 `query_sessions` 对 `source_ref` **精确相等**过滤、`updated_at` 降序、`total` 为过滤集全量（`crates/infra/store/src/store.rs:762-828`），`SessionSummary` 随页带 `stats.turnCount` 与轮行。
- 流程会话 `source_ref` 四段字面量：`format!("{change_id}/{phase}/{role}/{attempt}")`（`crates/core/orchestration/src/walker.rs:3400-3406`，`source = SOURCE_CHANGE`）——与 chat 的两段 `<change id>` 在精确相等下天然不撞。
- 详情页为纵向单列（`change-detail-view.tsx:280-303`），抽屉为模态浮层（`components/standard/drawer.tsx:36` 未透传 `modal`；`components/ui/drawer.tsx:99` 仅 `modal === true` 渲染遮罩、:103 非模态时 Viewport `pointer-events-none`）——非模态能力已在底层，只差上层透传。
- 分栏与折叠 API 在位：`react-resizable-panels` v4 的 `Panel.collapsible` / `collapsedSize` / `panelRef` / `onResize` / `usePanelRef`（`node_modules/react-resizable-panels/dist/react-resizable-panels.d.ts:234-410`、:565-570）；范式先例 = `views/explores/explore-detail-view.tsx:398-406`。
- 注入形态已支持空态草案：`AgentSessionView` 的 `SessionAbsent` 只在**非注入**且 `sessionId === null` 时渲染（`components/agent/agent-session-view.tsx:92-98`），注入形态下空态走「无元信息 + 空对话 + composer」——面板「打开即空态且可发送」零新增渲染分支。

**只读红线（本设计零触点）**：`crates/core/**`（会话内核 / `SessionCtx` / `SessionProvenance` / 清单契约）、`crates/infra/agent/**`（`worker.rs` / `compose.rs` / 沙箱 / `AGENT.md` 注入）、`crates/infra/store/**`（schema / `ChangeRecord` / 查询语义）、`commands/change_flow/**`、`views/agent/**`、`views/explores/**`、`components/ui/drawer.tsx`、`plugins/**`。

---

## 关键设计决策

| # | 问题 | 定稿 | 理由 | 备选方案 |
|---|------|------|------|----------|
| D1 | 执行根通道的形态 | `agent_start` **尾部**追加 `cwd: Option<String>`（位置索引 8），`agent_start_with` 同步；`SessionCtx.workspace_root` 取 `cwd` 解析值，`stores.for_root(&root)` 逐字不动 | 与 `change_flow_start` 的双 root 拆分同构（exec_root 只进会话 cwd，store 恒 workspace root）；`cwd` 在末位 ⇒ 既有 1–7 位零漂移，调试页 / explore 链零改动 | 新增 `change_agent_start` 薄命令（前端出现第二条发起路径，`agent-transport` 需分叉）；把 worktree 当 `root` 传（`for_root` 会静默铸出第二份 workspace 库，proposal 风险 1） |
| D2 | `cwd` 取值与校验口径 | `None` **或空白串** → 回落 `root`（与 `None → root` 同义，避免前端空串诱发误报）；非空白 → `Path::is_dir` 探测，失败即 `Err("worktree 目录不存在（可能已被手动删除）: {path}；请恢复目录或手动清理后重建 change")`；校验位于 `is_blank_root` 之后、`for_root` / `compose_turn` 之前 | 文案与 `change_flow_start` 逐字同源（同一类故障同一句话）；校验前置 ⇒ 失败不落库、不推流、不产生半天状态（AC-2）；空白串按缺省处理，不让「空串」被当成「不存在的目录」 | 空白串也 `Err`（前端 `worktree ?? null` 才传，无处产出空串，只会把真实故障与脏数据混为一谈）；抽公共 `fn worktree_missing_error()` 供 `change_flow` 复用（`commands/change_flow/**` 在只读红线内，MUST NOT 为共用而改它——字面量在 exec 侧重写一份） |
| D3 | 前端 `cwd` 派生与归档口径 | `cwd = archived ? null : worktree`（`worktree === null` 即 legacy → 后端回落 root，等价 spec 的 `detail.worktree ?? root`）；归档恒不传 | 归档 change 的 worktree 已被 Cleanup 移除（proposal 风险 5）：传悬空路径只会在发送点 `Err`，而归档行本就禁发；不传即零 Err 路径，与「归档只读」自洽 | 归档也传 `worktree`（禁发行下无害但把错误面留在代码里）；前端做 `is_dir` 探测（无 fs 命令面，且与 spec「非归档缺失由命令层 Err」重复判据） |
| D4 | 归属写入的常量位置 | 写侧 `source = "change"` 由前端受控常量提供（`CHANGE_CHAT_SOURCE`，`views/changes/agent/hooks/use-change-chat.ts`）；读侧 `source` 词常量收在命令层（`CHANGE_CHAT_SOURCE: &str = "change"`，`src/commands/exec/mod.rs` 模块私有）。两处各一份字面量，不跨语言共享 | `source` 是自由字符串（`SessionProvenance.source: String`），Rust 侧无枚举可耦合；前端零来源词汇的约束落在「面板不拼 `source_ref`、不直调通用清单命令」上（spec 明文），写侧的 `source` 词必须由面板给出 | 让 `list_change_chat_sessions` 接受 `source` 参数（前端被迫懂来源词汇，spec 明令禁止）；为 `source` 造枚举（`core/agent` 零改动红线） |
| D5 | 清单命令形态 | `list_change_chat_sessions(root, id, offset, limit) -> Result<SessionPage, String>`：`is_blank_root` → **空页信封**（`{ sessions: [], total: 0 }`）；否则 `for_root(&root)` → `session_query(store).list_sessions(Some("change"), Some(&id), offset, limit)`。`id` 原样作 `source_ref`（不 trim / 不规范化），`limit` 越界由 store 侧 `MAX_SESSION_PAGE_LIMIT` 截断 | 三段薄包装（参数转换 → 内核查询 → 错误映射），与 `agent_sessions` 同形；`id` 不加工 ⇒ URL id 即身份键（页内单一身份源），任何规范化都会让「清单与归属」二次解释 | 面板直调 `agentSessions(root, 'change', id, …)`（前端出现来源词汇，spec 明令「MUST NOT 直调通用清单命令以隐式过滤」）；命令内先 `find_change_record` 校验 id（清单是探测面，未建档 id 返回空页比 `Err` 更符合读面语义，也与 `agent_sessions` 口径一致） |
| D6 | transport 位置参数的消险方式 | `invokeStart` 内先 `const { root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd } = session;` 再按生成绑定的**位置签名**依次传参；`AgentStartSessionParams` 的**类型**仍由 `Parameters<typeof commands.agentStart>` 索引派生（不手写镜像） | 类型派生保证「参数面增位即编译报错」的成本为零；运行期改具名解构后，读参错位（proposal 风险 9）在调用点肉眼可核，不再靠数下标 | 一律改对象式 invoke（生成绑定是位置签名，`__TAURI_INVOKE` 的对象键由导出器铸定，改不动）；手写镜像类型（增位即漏） |
| D7 | `sourceRef` / `cwd` 的运行时校验 | 两字段均走既有 `readNullableString(get('sourceRef'), 'sourceRef')` / `readNullableString(get('cwd'), 'cwd')`：`string \| null` 之外一律 `Err`，transport 不改写、不 trim | 与 `sessionId` 同一校验器（同源同口径），非法形态在发起前显式失败（spec「非法形态即 `Err`，不改写」） | 宽容强制 `String(value)`（把非法值静默变成路径串，故障深埋进引擎） |
| D8 | 会话参数面的缺省语义 | `UseAgentSessionParams` 增 `sourceRef?: string \| null` / `cwd?: string \| null`，body 以 `sourceRef: sourceRef ?? null` / `cwd: cwd ?? null` 透传 | 未提供即 `null` ⇒ 调试页 / explore 链行为逐字不变（spec「未提供时缺省不漂移」）；`null` 到 Rust 侧即 `None`（`source_ref` 沿用既有缺省、`cwd` 回落 root） | 在 hook 内按 `root` 兜底填 `cwd`（把「执行根」语义塞进通用基建，来源侧失去显式性） |
| D9 | 空态与首发 | `sessionId = null` 即「草案态」，不参与任何禁发判据；发送时 body 的 `sessionId` 走既有 `mirrors.currentSessionIdRef.current ?? sessionId`（空态 ⇒ `null` ⇒ 后端 `New` 铸会话，id 经 early-resolve 轮行回写镜像） | `useAgentSession` 现有实现已满足（`hooks/use-agent-session.ts:457`），本变更**零改动**即得「首发即 New」；面板只需不自设禁用门 | 空态禁用输入（proposal 已否：「新会话 → 首发」不可达） |
| D10 | 「当前会话」的真相源 | 面板头区回显取镜像 `session.session?.row.id`（`currentSessionId`），本地 `sessionId` 只作寻址输入；面板「是否空态」同源 | 空态首发后本地 `sessionId` 仍为 `null`（id 由后端铸造），只看本地态会呈「正在聊却显示空态」（proposal 风险 10） | 发送后本地把轮行 id 写回 `sessionId`（会让「新会话」与重放 effect 竞争，且与 `sessionId` 语义=寻址键冲突） |
| D11 | 清单取数时机与分页 | 挂载**零清单 invoke**（打开即空态）；取数仅三处：①展开「历史 ▾」时重取首屏、②`currentSessionId` 由 `null` 变非 `null`（会话 id 确立）、③`running` 由 `true` 变 `false`（轮收口）。分页 `CHANGE_CHAT_PAGE_SIZE = 20`，累加式「加载更多」以 `items.length < total` 为可见条件 | 三处正对 spec 的三个事实：首发后清单可见（②③）、打开即空态（挂载零取数）、分页信封（①）。展开时重取使「页面刷新后仍能看到别处新铸的会话」，且不涉 `session_detail` / 转录（AC-5 的 invoke 计数面） | 挂载即预取清单（违「打开即空态、零取数」的字面口径）；轮询式刷新（引入轮询节拍，与「完成态零轮询」纪律冲突） |
| D12 | 实时面纪律 | 面板的交互式会话**不传** `refreshKey`（`useAgentSession` 的 `refreshKey` 参数全程缺省）；只读抽屉（`SessionTranscriptPanel`）继续收 `sessionRefreshKey` | `use-change-detail` 的 `refreshTick` 在 run 期由每条密封事件驱动（300ms 去抖），接上去即整轮重放风暴（proposal 风险 2 / AC-5）；会话状态重评估由 `useAgentSession` 自身的库态推导 + 订阅面承担（与 explore 注入形态同源） | 传 `refreshTick`（AC-5 直接失败）；在面板内自建去抖（第二套计时面） |
| D13 | 禁发判据单源（矩阵落地） | 判据只在 `useChangeChat` 内算一次并向下传：`disabledReason = archived ? '已归档 change 只读' : runActive ? '流程运行中' : null`；`canSend = disabledReason === null`；`canStart = disabledReason === null && !session.running`；可切历史恒 `true`。`sessionId === null` **不入判据**，也不产生第三条理由 | 矩阵四行逐一可验：row1 = `disabledReason` 非空（禁发 + 禁新会话，历史可切）；row2 = `disabledReason` 为空但 `running` 为真（`AgentInput` 既有语义：禁发送 + 停止入口，`canStart` 因 `running` 而假）；row3 = 全通；row4 = `disabledReason` = 只读。空态在 row1/row4 与既有会话同样禁用、在 row3 照常可发（spec「空态不参与判据、也不豁免」） | 各组件自判（spec 明令单源）；把「会话运行中」也塞进 `disabledReason`（会顶替 `running` 的停止入口，与「`disabled` MUST NOT 复用为运行中的第二表达」冲突） |
| D14 | `AgentInput.disabled` 的呈现口径 | 新增 `disabled?: boolean`（缺省 `false`）与 `disabledReason?: string`：`disabled` 时 textarea `disabled`、发送按钮禁用、理由文案呈于工具行上方（`data-testid={idPrefix}-disabled-reason`）、**停止入口不呈**；`disabled === false` 时 `running` 语义逐字不变（禁发送 + 停止入口）；发送禁用条件 = `disabled \|\| running \|\| prompt 空白` | spec 场景「输入框与发送按钮同时禁用、理由文案可见」+「禁用态不呈停止入口，`running` 的既有呈现不被顶替」；两标志语义正交（业务只读态 vs 会话运行态），拼成一个布尔会丢信息 | 复用 `running`（语义混流：归档只读也要显示「停止」，荒谬）；只禁发送不禁输入（用户「打了字发不出去」） |
| D15 | 分栏与折叠态真相 | `DetailLoaded` 出 `ResizablePanelGroup orientation="horizontal"`；左 `ResizablePanel`：`defaultSize="30"` / `minSize="22"` / `collapsible` / `collapsedSize="3rem"` / `panelRef`；折叠态**真相取 Panel 的 `onResize` 回传**（`size.inPixels <= CHAT_PANEL_RAIL_PX(48)` ⇒ collapsed，上抛到视图 state），`DetailHeader` 开关与窄条按钮共用 `toggle()`（依 `panelRef.current.isCollapsed()` 调 `collapse()` / `expand()`）；change id 变化 effect 内 `panelRef.current?.isCollapsed()` 为真则 `expand()` | 单一真相（用户拖拽收起也会被 `onResize` 捕捉，header 文案与窄条内容不会与实际宽度脱节）；`onResize` 首拍（`prevPanelSize === undefined`）置 `false` 与初始展开态一致；`collapsible` + `collapsedSize="3rem"` 即「收起留窄条」，窄条内呈现展开入口（spec「收起留窄条」与 header 开关两入口都不缺） | 折叠态只由按钮维护（拖拽收起后 header 文案错位、窄条渲染成完整面板）；`collapsedSize="0"` + 仅 header 开关（丢「留窄条」的可见入口） |
| D16 | 右栏内部次序 | header（返回 / 刷新 / 标题 / 状态 / worktree / 面板开关）**整宽**留在分栏之上；其下横向 split；**右栏内**依次为 `RefreshStaleNote` → `DetailActionPanels` → `DetailContent`（流程图 + 抽屉装配） | 沿用既有 D1「陈旧横幅在动作面板之上」的相对次序，spec 明定右栏次序（刷新失败提示 → 运行控制面板 → 流程图区）；横幅收进右栏后左栏 chat 不会因刷新失败而位移 | 横幅保持页首整宽（左栏 chat 顶部被横幅顶下去，跨 change 视觉抖动）；横幅进左栏（与 chat 无关的错误塞进会话区） |
| D17 | 抽屉非模态的透传点 | `StandardDrawer` 增 `modal?: boolean = true`，解构后传入 `<Drawer modal={modal}>`（**不**落入 `{...rest}`——`rest` 会被摊到 `DrawerContent`，多出未知 DOM 属性）；`DetailDrawer` 传 `modal={false}`。Esc 关闭以 base-ui 的 dismiss（`open` / `onOpenChange`）为主路径 | 非模态能力全在底层（`components/ui/drawer.tsx:99/102-103`），上层只差透传；缺省 `true` ⇒ 其它调用方零回归（spec「其它调用方仍模态」） | 改 `components/ui/drawer.tsx`（在只读红线内且无必要）；把抽屉改成内嵌分栏（spec 明令「MUST NOT 以抽屉改为内嵌分栏解决」） |
| D18 | Esc 关闭的兜底路径 | 落地先验 base-ui `modal={false}` 下 Esc 是否触发 dismiss；若不触发，在 `DetailDrawer` 内补一个 `useEffect` 监听 `keydown`（`Escape` → `onClose()`，`selection === null` 时不挂），不改 spec 条款 | 关闭入口是 spec 明文（页脚「关闭」+ Esc），不把「库行为未知」升级为验收风险；`onClose` 幂等（置 `selection = null`），与库自身 dismiss 并存无双触发副作用 | 只留页脚按钮（spec 条款落空）；self-made 全局面板状态（第二套展开交互） |
| D19 | 面板空态 / 窄条 / 清单的视觉细节 | 不固化像素：头区文案「通用 Agent」、窄条为竖向文字 + 展开按钮、清单为可展开列表（id 前缀 + 更新时间 + 轮数）——文案与排版留在实现期按观感微调，`data-testid` 挂钩固定（`change-chat-panel` / `change-chat-new` / `change-chat-history` / `change-chat-note`，另增 `change-chat-cwd` / `change-chat-expand` / `change-chat-history-item`） | spec 只固化行为与可见性，像素级细节不构成验收面（proposal 风险 11 第一条）；testid 固定保证行为可验 | 把排版写进 spec（脆且无价值） |
| D20 | 版本与生成物 | `packages/desktop/package.json` `version` `0.5.19` → `0.5.20`；`src/types/generated/bindings.ts` 经 `pnpm run bindings:export` 再生成（禁手改），`bindings:check` 作守卫 | AC-8；生成物是 `all_commands!` + `specta` 的唯一镜像，手改即漂移 | 手写 bindings（守卫必红） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 发起命令参数面（改） | `cwd: Option<String>` 尾部增位；先 `is_blank_root`、再 `cwd` 解析与 `is_dir` 校验、最后 `for_root(&root)` + `compose_turn`；`SessionCtx.workspace_root = cwd 解析值`，store 半边恒 `root`（双 root 不变量） | `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `::agent`（`SessionCtx` / `SessionRef` / `SessionProvenance`）、`agent_runtime::compose_turn`、`store::WorkspaceStores` | Rust、tauri `#[tauri::command]` + `#[specta::specta]`、`std::path::Path` |
| chat 清单命令（新） | `list_change_chat_sessions`：blank root → 空页；否则 `for_root` → `session_query(...).list_sessions(Some("change"), Some(id), offset, limit)` | 同上（`agent_sessions` 相邻位） | `SessionPage`、`agent_runtime::session_query` | Rust、三段薄包装 |
| 命令登记面（改） | `all_commands!` 增 `exec::list_change_chat_sessions`（builder 与 `generate_handler!`、bindings 导出同源） | `packages/desktop/src-tauri/src/commands/mod.rs` | — | 宏 |
| 生成绑定（再生成） | `agentStart` 尾部 `cwd: string \| null`；新增 `listChangeChatSessions` typed 包装 | `packages/desktop/src/types/generated/bindings.ts` | `export-bindings` | 生成物（禁手改） |
| 会话参数透传（改） | `AgentStartSessionParams` 增 `sourceRef` / `cwd`（类型索引派生）；`readSessionParams` 增读两字段（`readNullableString`）；`invokeStart` 改具名解构 | `packages/desktop/src/lib/agent-transport.ts` | `@tauri-apps/api/core`、`ai`、生成绑定 | TypeScript |
| 会话基建参数面（改） | `UseAgentSessionParams` 增 `sourceRef?` / `cwd?`；`sendMessage` body 透传（`?? null`）；原 `sourceRef: null` 常量退役 | `packages/desktop/src/hooks/use-agent-session.ts` | `@ai-sdk/react`、agent-transport | React hook |
| composer 禁用态（改） | `AgentInput` 增 `disabled` / `disabledReason`（textarea 与发送同禁、理由文案、禁用态不呈停止入口） | `packages/desktop/src/components/agent/agent-input.tsx` | — | React |
| change chat 面板（新） | 头区（「新会话」/「历史 ▾」/状态提示行）+ `AgentSessionView` 注入形态（`session` prop）+ composer 插槽 + 折叠窄条；不含任何自有转录查询或消息拼装 | `packages/desktop/src/views/changes/agent/change-chat-panel.tsx` | `components/agent`、同目录 hook | React、TypeScript |
| change chat composer（新） | `AgentInput` 薄适配（`idPrefix="change-chat"`，透传 `running` / `disabled` / `disabledReason`） | `packages/desktop/src/views/changes/agent/change-chat-composer.tsx` | `components/agent/agent-input` | React |
| change chat hook（新） | 当前 `sessionId` 本地态（`null` = 草案态）+ `useAgentSession({ source: 'change', sourceRef, cwd })` + 清单取数/分页/重取 + 新会话 / 历史点选 + change id 重置 + 禁发判据（矩阵单源） | `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | `hooks/use-agent-session`、生成绑定 `commands.listChangeChatSessions` | React hook |
| 分栏原语出口（改） | 追加 `export { usePanelRef } from 'react-resizable-panels'`（折叠命令面出口，保持「视图只 import `@/components/ui/resizable`」的既有约定，不让视图直连原语包） | `packages/desktop/src/components/ui/resizable.tsx` | `react-resizable-panels`（v4，已依赖） | 单行 re-export（该文件在 `vite.config.ts` 的 lint 忽略面内） |
| 详情分栏宿主（改） | `DetailLoaded` 纵向单列 → `ResizablePanelGroup(horizontal)`；左栏 `ChangeChatPanel`（collapsible + 窄条）、右栏 stale note / 动作面板 / 流程图；`DetailHeader` 增面板显隐开关；折叠态真相取 `onResize` | `packages/desktop/src/views/changes/change-detail-view.tsx` | `components/ui/resizable`（`ResizablePanelGroup` / `ResizablePanel` / `ResizableHandle` / `usePanelRef`）、agent 面板 | React、react-resizable-panels v4 |
| 标准抽屉（改） | 增 `modal?: boolean`（缺省 `true`，透传底层 `Drawer`；不落入 `DrawerContent` 的 rest） | `packages/desktop/src/components/standard/drawer.tsx` | `components/ui/drawer` | React |
| 详情抽屉（改） | 传 `modal={false}`（`data-modal="false"`、无遮罩节点）；选中驱动 / 内容分派 / 宽度不变 | `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | `StandardDrawer` | React |
| 版本面 | 桌面端补丁号 0.5.19 → 0.5.20 | `packages/desktop/package.json` | — | JSON 单行 |

数据流（目标形态）：

```
ChangeDetailView（root / detail / activeRun / refreshTick）
  └─ DetailLoaded ── ResizablePanelGroup(horizontal)
        ├─ 左 Panel(collapsible, panelRef) ── ChangeChatPanel(root, changeId, cwd, readOnly, runActive)
        │      └─ useChangeChat ── useAgentSession(source='change', sourceRef=changeId, cwd=worktree)
        │            ├─ 发送：transport.invokeStart → agent_start(…, sourceRef, cwd) ── SessionCtx.workspace_root = cwd
        │            │                                          └─ stores.for_root(workspace root)（双 root 不变量）
        │            ├─ 重放：session_detail + agent_session_transcript（sessionId 直查，零来源反查）
        │            └─ 清单：list_change_chat_sessions(workspace root, changeId, offset, limit) → SessionPage
        └─ 右 Panel ── RefreshStaleNote → DetailActionPanels → DetailContent(流程图 + DetailDrawer modal={false})
```

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/views/changes/agent/change-chat-panel.tsx` | `ChangeChatPanel`：头区动作面（「新会话」/「历史 ▾」/ 状态提示行 / 执行根文案）+ `AgentSessionView` 注入形态 + `ChangeChatComposer` 插槽 + 折叠窄条；内部私有子组件（`ChatHeaderActions` / `HistoryList` / `StatusNote` / `CollapsedRail`）；公共项见「公共函数 / API」「类型定义」（配套 `change-chat-panel.test.tsx` 归测试相位） |
| `packages/desktop/src/views/changes/agent/change-chat-composer.tsx` | `ChangeChatComposer`：`AgentInput` 薄适配（`idPrefix="change-chat"`、`disabled` / `disabledReason` / `running` / `onSend` / `onStop` 透传）（配套 `change-chat-composer` 断言并入 `change-chat-panel.test.tsx`） |
| `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | `useChangeChat` + `ChangeChatState`：当前 `sessionId` 本地态、`useAgentSession` 注入形态、清单取数 / 分页 / 重取、新会话 / 历史切换、change id 重置、禁发判据；内部私有 `useChangeChatList`（取数面）（配套 `use-change-chat.test.ts` 归测试相位） |

### 修改文件 — 后端（`packages/desktop/src-tauri`）

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `src/commands/exec/mod.rs` | ① 模块私有常量 `const CHANGE_CHAT_SOURCE: &str = "change";`；② `agent_start` 签名尾部增 `cwd: Option<String>` 并透传 `agent_start_with`；③ `agent_start_with` 签名尾部增 `cwd: Option<String>`，`is_blank_root` 后插入执行根解析（`cwd` 去空白后为空 → `root`；`Some(p)` 且 `Path::new(p).is_dir()` → `p.to_owned()`；否则 `Err("worktree 目录不存在（可能已被手动删除）: {p}；请恢复目录或手动清理后重建 change")`），`SessionCtx.workspace_root` 改用解析值；`stores.for_root(&root)` 与本段注释（「cwd 恒为主 root」）一并改写为双 root 表述（store 半边恒 root，exec 半边可被 `cwd` 覆盖）；④ 新增 `list_change_chat_sessions` 命令（三段薄包装，blank root → `SessionPage { sessions: vec![], total: 0 }`） | AC-1 / AC-2 / AC-3；`agent_stop` / `agent_session_watch` / `agent_sessions` / `agent_session_transcript` / `session_detail` 零改动；`clippy::too_many_arguments` 的 `#[allow]` 已在位 |
| `src/commands/mod.rs` | `all_commands!` 增 `$crate::commands::exec::list_change_chat_sessions,`（紧随 `session_detail` 之后） | 单一登记面：builder / `generate_handler!` / bindings 导出同源 |
| `src/types/generated/bindings.ts` | 再生成（`pnpm run bindings:export`）：`agentStart` 尾部增 `cwd: string \| null`；新增 `listChangeChatSessions: (root: string, id: string, offset: number, limit: number) => __TAURI_INVOKE<SessionPage>("list_change_chat_sessions", { root, id, offset, limit })` | 生成物入库、禁手改；`bindings:check` 守卫（AC-8） |
| `src/bindings/mod_test.rs` | 形状守卫断言随动：`agentStart` 逐字签名断言改为以 `cwd: string \| null` 收尾、`invoke` 参数对象含 `cwd` 键；命令清单断言增 `listChangeChatSessions` | proposal 把它列入「实现文件」故在此登记；它是**生成物形状守卫**（非行为测试），其断言改写由测试相位承接，本设计不新增测试章节 |

### 修改文件 — 前端（`packages/desktop/src`）

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `lib/agent-transport.ts` | `AgentStartSessionParams` 增 `sourceRef: AgentStartArgs[6]` / `cwd: AgentStartArgs[8]`；`readSessionParams` 增读两字段（`readNullableString`）；`invokeStart` 改「具名解构 → 按生成绑定位置签名传参」（原 `null` 占位 sourceRef 与位置索引魔法退役） | AC-3 / AC-4 的写侧前置；transport 仍无状态、零改写（D6 / D7） |
| `hooks/use-agent-session.ts` | `UseAgentSessionParams` 增 `sourceRef?: string \| null` / `cwd?: string \| null`；`useSessionActions` 解构并透传；`sendMessage` body 的 `sourceRef: null` 常量改为 `sourceRef: sourceRef ?? null` 并新增 `cwd: cwd ?? null`（依赖数组随动） | 原 `sourceRef` 恒 null 的唯一残留清除（D8）；未提供时行为逐字不变 |
| `components/agent/agent-input.tsx` | `AgentInputProps` 增 `disabled?: boolean` / `disabledReason?: string`；`AgentInput` 渲染理由行（`data-testid={idPrefix}-disabled-reason`）、textarea `disabled={disabled}`；`ComposerToolbar` 增 `disabled`（发送 `disabled={disabled \|\| running \|\| prompt 空白}`；停止入口条件改 `running && !disabled`） | AC-6 的禁用面（D14）；`running` 语义不变 |
| `views/changes/change-detail-view.tsx` | ① `DetailHeader` 增 `chatCollapsed: boolean` / `onToggleChat: () => void`，渲染开关按钮（`data-testid="detail-chat-toggle"`，文案「隐藏通用 Agent」/「显示通用 Agent」）；② 新增私有 `useChatPanelState()`（`usePanelRef` + `collapsed` state + `toggle` + `setCollapsed`，`toggle` 依 `panelRef.current.isCollapsed()` 分派 `collapse()` / `expand()`）；③ `ChangeDetailView` 增 change id 变化的折叠复位 effect（`isCollapsed()` → `expand()`）；④ `DetailLoadedProps` 增 `chatPanelRef` / `chatCollapsed` / `onChatCollapseChange`；⑤ `DetailLoaded` 纵向单列改 `ResizablePanelGroup orientation="horizontal"`（`className="min-h-0 flex-1"`，header 整宽留在分栏之上），右栏内 `RefreshStaleNote` → `DetailActionPanels` → `DetailContent`；⑥ 左 Panel 挂 `ChangeChatPanel`（`root` / `changeId={id ?? detail.id}` / `cwd={detail.worktree}` / `readOnly={detail.source === 'archive'}` / `runActive={activeRun !== null}` / `collapsed` / `onToggleCollapsed`） | AC-1 / AC-6 / AC-7 的宿主面（D15 / D16）；`DetailContent` / `DetailDrawer` 装配与 `selection` 语义不变 |
| `components/standard/drawer.tsx` | `StandardDrawerProps` 增 `modal?: boolean`；`StandardDrawer` 解构 `modal = true` 并传入 `<Drawer modal={modal} …>`（不进入 `{...rest}`，避免落到 `DrawerContent` 的 DOM 属性面） | AC-7；既有调用方零回归（D17） |
| `components/ui/resizable.tsx` | 追加 `export { usePanelRef } from 'react-resizable-panels';`（`ResizablePanelGroup` / `ResizablePanel` / `ResizableHandle` 三个封装与其余行逐字不动） | 折叠命令面出口：`panelRef` / `collapsible` / `collapsedSize` / `onResize` 已由 `PanelProps` 原样透传，只缺 hook 的出口；保持「视图只 import `@/components/ui/resizable`」的既有约定（`views/**` 无直连原语包的先例，D15） |
| `views/changes/flow/detail-drawer.tsx` | `<StandardDrawer … modal={false}>`（其余 props / 内容分派 / `containerClassName="w-[960px] max-w-[85vw]"` 逐字不动）；文件头注释由「遮罩点击或关闭按钮置 selection 为 null」改为「非模态：页脚关闭按钮 / Esc 置 selection 为 null」 | AC-7；`data-modal="false"` + 无遮罩节点由底层现有机制给出 |
| `components/agent/agent-session-view.tsx` | **无改动（定稿）** | proposal 列该文件为「如需把 `disabled` 经 `AgentSessionActions` 传出，仅扩插槽入参」的条件项：本设计把 `disabled` 由面板侧直接传给 `ChangeChatComposer`（`useChangeChat` 的判据面在插槽外已可用），插槽入参仍只需 `running`，故 `AgentSessionActions` 与组件契约零变化 |
| `packages/desktop/package.json` | `version` `"0.5.19"` → `"0.5.20"` | AC-8（D20）；脚本面不变 |

### 删除文件

<!-- 本变更不删除文件：面板为新增目录，抽屉保留（仅改非模态），无退役组件。 -->

无。

### 公共函数 / API

**Rust — `packages/desktop/src-tauri/src/commands/exec/mod.rs`**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `agent_start` | `exec/mod.rs` | 修改 | `pub async fn agent_start(app: AppHandle, on_event: Channel<AgentRunMessage>, root: String, prompt: String, permission_mode: ::agent::AgentPermissionMode, session_id: Option<String>, source: Option<String>, source_ref: Option<String>, agent: Option<i64>, cwd: Option<String>) -> Result<TurnSummary, String>` | `cwd` 末位（位置 8）：只喂 `SessionCtx.workspace_root`；`for_root(&root)` 恒不动；`cwd` 非空白先 `is_dir` 校验并显式 `Err`（D1 / D2） |
| `agent_start_with` | `exec/mod.rs` | 修改 | `pub(crate) async fn agent_start_with<R: tauri::Runtime>(app: AppHandle<R>, on_event: Channel<AgentRunMessage>, root: String, prompt: String, permission_mode: ::agent::AgentPermissionMode, session_id: Option<String>, source: Option<String>, source_ref: Option<String>, agent: Option<i64>, cwd: Option<String>) -> Result<TurnSummary, String>` | 泛型测试缝同步（生产 / `mock_app` 同源）；编排仍全部在内核 |
| `list_change_chat_sessions` | `exec/mod.rs` | 新增 | `pub fn list_change_chat_sessions(stores: State<'_, WorkspaceStores>, root: String, id: String, offset: u32, limit: u32) -> Result<SessionPage, String>` | chat 清单薄包装：blank root → 空页信封；`source` 常量 `"change"` + `source_ref = id`（D5） |

<!-- 私有函数（执行根解析、`is_blank_root`）不列：无模块级导出面。 -->

**TypeScript — 导出函数 / 导出 class 公开方法 / Hook**

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `TauriAgentTransport.sendMessages` | `lib/agent-transport.ts` | 修改（内部；签名不变） | `sendMessages(options: { trigger: 'submit-message' \| 'regenerate-message'; chatId: string; messageId: string \| undefined; messages: AgentUIMessage[]; abortSignal: AbortSignal \| undefined; body?: object }): Promise<ReadableStream<AgentUIMessageChunk>>` | 读参面增 `sourceRef` / `cwd`（`readSessionParams`），发起 invoke 改具名解构（D6） |
| `TauriAgentTransport.reconnectToStream` | `lib/agent-transport.ts` | 不变 | `reconnectToStream(options: { chatId: string; abortSignal?: AbortSignal; body?: object }): Promise<ReadableStream<AgentUIMessageChunk> \| null>` | 订阅面不携 `cwd` / `sourceRef`（重放与订阅唯 `sessionId` + `root`） |
| `useAgentSession` | `hooks/use-agent-session.ts` | 修改 | `useAgentSession(params: UseAgentSessionParams): UseAgentSessionState` | 参数面增 `sourceRef?` / `cwd?`，发送 body 透传（D8）；重放 / 订阅 / 停止语义不变 |
| `useChangeChat` | `views/changes/agent/hooks/use-change-chat.ts` | 新增 | `useChangeChat(params: ChangeChatParams): ChangeChatState` | 面板唯一状态与判据源（D3 / D9–D13） |
| `ChangeChatPanel` | `views/changes/agent/change-chat-panel.tsx` | 新增 | `ChangeChatPanel(props: ChangeChatPanelProps): React.JSX.Element` | 面板骨架：头区动作 / 状态提示 / 注入形态对话体 / composer 插槽 / 折叠窄条（D19） |
| `ChangeChatComposer` | `views/changes/agent/change-chat-composer.tsx` | 新增 | `ChangeChatComposer(props: ChangeChatComposerProps): React.JSX.Element` | `AgentInput` 薄适配（`idPrefix="change-chat"`） |
| `AgentInput` | `components/agent/agent-input.tsx` | 修改 | `AgentInput(props: AgentInputProps): React.JSX.Element` | 增 `disabled` / `disabledReason`（D14） |
| `StandardDrawer` | `components/standard/drawer.tsx` | 修改 | `StandardDrawer({ title, content, containerClassName, trigger, open: outerOpen, onOpenChange: onOuterOpenChange, modal = true, ...rest }: StandardDrawerProps): React.JSX.Element` | `modal` 透传底层 `Drawer`（D17） |
| `usePanelRef` | `components/ui/resizable.tsx` | 新增（re-export） | `usePanelRef(): React.RefObject<PanelImperativeHandle \| null>` | 折叠命令面出口（`collapse()` / `expand()` / `isCollapsed()` 经 ref 取用；视图不直连 `react-resizable-panels`） |
| `DetailDrawer` | `views/changes/flow/detail-drawer.tsx` | 修改（签名不变） | `DetailDrawer(props: DetailDrawerProps): React.JSX.Element \| null` | 内部传 `modal={false}`（+ D18 的 Esc 兜底，若需） |
| `ChangeDetailView` | `views/changes/change-detail-view.tsx` | 修改（签名不变） | `ChangeDetailView({ root }: { root: string \| null }): React.JSX.Element` | 分栏宿主 + header 开关 + 折叠复位（D15 / D16） |

### 类型定义

**Rust**

<!-- 本变更零 native model / 零会话契约演进：`SessionCtx`（`workspace_root` / `permission_mode` / `model_level`）、`SessionProvenance`、`SessionPage`、`SessionSummary`、`SessionRef`、`TurnSummary` 全部逐字不变；`agent_start` 的参数面变化是函数签名而非类型定义，故本子表省略。 -->

无。

**TypeScript**

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `AgentStartSessionParams` | `lib/agent-transport.ts` | 修改（模块私有） | `{ root: AgentStartArgs[1]; prompt: AgentStartArgs[2]; permissionMode: AgentStartArgs[3]; sessionId: AgentStartArgs[4]; source: AgentStartArgs[5]; sourceRef: AgentStartArgs[6]; agent: AgentStartArgs[7]; cwd: AgentStartArgs[8] }`（类型仍由生成绑定派生，无手写镜像） |
| `UseAgentSessionParams` | `hooks/use-agent-session.ts` | 修改 | `{ root: string \| null; sessionId: string \| null; source: string; sourceRef?: string \| null; cwd?: string \| null; refreshKey?: number }` |
| `AgentInputProps` | `components/agent/agent-input.tsx` | 修改 | `{ idPrefix: string; label?: string; placeholder?: string; running: boolean; disabled?: boolean; disabledReason?: string; onSend: (input: { text: string; permissionMode: AgentPermissionMode }) => void; onStop: () => void }` |
| `StandardDrawerProps` | `components/standard/drawer.tsx` | 修改 | `{ title: string; content: React.JSX.Element; containerClassName?: string; trigger?: React.JSX.Element; open?: boolean; onOpenChange?: (open: boolean) => void; modal?: boolean }`（`modal` 缺省 `true`） |
| `ChangeChatParams` | `views/changes/agent/hooks/use-change-chat.ts` | 新增 | `{ root: string \| null; changeId: string \| null; worktree: string \| null; runActive: boolean; archived: boolean }` |
| `ChangeChatState` | `views/changes/agent/hooks/use-change-chat.ts` | 新增 | `{ session: UseAgentSessionState; sessionId: string \| null; currentSessionId: string \| null; cwd: string \| null; sessions: SessionSummary[]; total: number; listOpen: boolean; listLoading: boolean; listError: string \| null; toggleList: () => void; loadMore: () => void; newSession: () => void; selectSession: (sessionId: string) => void; send: (input: { text: string; permissionMode: AgentPermissionMode }) => void; stop: () => void; canSend: boolean; canStart: boolean; disabledReason: string \| null }` |
| `ChangeChatPanelProps` | `views/changes/agent/change-chat-panel.tsx` | 新增 | `{ root: string \| null; changeId: string \| null; cwd: string \| null; readOnly: boolean; runActive: boolean; collapsed: boolean; onToggleCollapsed: () => void }` |
| `ChangeChatComposerProps` | `views/changes/agent/change-chat-composer.tsx` | 新增 | `{ running: boolean; disabled: boolean; disabledReason: string \| null; onSend: (input: { text: string; permissionMode: AgentPermissionMode }) => void; onStop: () => void }` |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `CHANGE_CHAT_SOURCE` | `src-tauri/src/commands/exec/mod.rs` | 新增（模块私有常量） | `&'static str` | `"change"` | 清单命令的来源词常量（前端零来源词汇，D4 / D5） |
| `CHANGE_CHAT_SOURCE` | `views/changes/agent/hooks/use-change-chat.ts` | 新增（模块常量） | `string` | `"change"` | 写侧归属来源词（`useAgentSession({ source })`） |
| `CHANGE_CHAT_PAGE_SIZE` | `views/changes/agent/hooks/use-change-chat.ts` | 新增（模块常量） | `number` | `20` | 清单分页页宽（`list_change_chat_sessions` 的 `limit`；「加载更多」按此累加） |
| `CHAT_PANEL_DEFAULT_SIZE` / `CHAT_PANEL_MIN_SIZE` / `CHAT_PANEL_COLLAPSED_SIZE` / `CHAT_PANEL_RAIL_PX` | `views/changes/change-detail-view.tsx` | 新增（模块常量） | `string` / `string` / `string` / `number` | `"30"` / `"22"` / `"3rem"` / `48` | 左栏 Panel 尺寸与窄条阈值（`onResize` 折叠判据 `inPixels <= CHAT_PANEL_RAIL_PX`，D15） |
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.20`（原 `0.5.19`） | 用户可见变更提升补丁号（AC-8） |

---

## 数据模型

### 会话归属（零 schema 变更、零迁移）

```
SessionRecord（既有旁支表，workspace 库）
├── id
├── source        = "change"
│                     ├── chat 会话：source_ref = <change id>（两段，本变更新写侧）
│                     └── 流程会话：source_ref = "<changeId>/<phase>/<role>/<attempt>"（四段，既有 walker 写）
├── source_ref
├── updated_at    ← 清单排序键（query_sessions 内 updated_at 降序 + id 降序稳定序）
└── config_snapshot（既有；**不**新增 cwd 字段，见「待决问题 2」）

关系
change 1 ── N chat 会话     （source="change" ∧ source_ref = <change id>）
change 1 ── N 流程会话      （source="change" ∧ source_ref = 四段；精确匹配下与 chat 不撞）

持久化
- 库位：workspace root 所属库（`stores.for_root(&root)`）——`cwd` MUST NOT 参与寻址（双 root 不变量）
- 迁移：无（`ChangeRecord` 不加字段、native model 版本不动、`desktop-workspace-store` 零 delta）
- 新会话行：仅在首条消息发出时由内核铸造（无预建空会话、点「新会话」零落库）
```

- **归属不撞性**：chat 两段 / 流程四段依赖 `query_sessions` 的 `source_ref` **精确相等**过滤（`store.rs:770-778`）而非前缀匹配——`"<change id>"` 永不等于 `"<change id>/<phase>/…"`。这是字面约定而非机制（proposal 风险 8），落实为 store 侧的固化断言由测试相位承接。
- **「当前会话」非库字段**：change 侧零会话指针字段（`ChangeRecord` / `ChangeDetail` 不动），「当前会话」= 前端面板态 + 会话镜像（`SessionSummary.row.id`）；不自动恢复最近一条（spec 明文），故无需库内指针。

### 前端面板态（非持久化，`useChangeChat` + 视图）

```
useChangeChat
├── sessionId: string | null            ← 面板本地态：寻址输入；null = 草案态（可发送，不作禁发理由）
├── list: { open; items: SessionSummary[]; total; loading; error }   ← 清单探测面（分页累加）
└── session: UseAgentSessionState        ← 注入形态会话面（messages / chain / running / loading / …）
      └── currentSessionId = session.session?.row.id ?? null         ← 「当前会话」真相（D10）

change-detail-view
└── chatCollapsed: boolean               ← 左 Panel onResize 回传（inPixels <= 48），折叠态单一真相（D15）

重置面（挂 URL change id）
├── useChangeChat：changeId 变化 → sessionId = null、list.open = false、清单重取（关）
└── change-detail-view：changeId 变化 → 若 Panel 处于折叠则 expand()（onResize 自动回传 false）
```

- **草案态合法性**：`sessionId === null` 时 `useAgentSession` 的重放 effect 早退（零取数、零订阅）；发送时 body `sessionId` 为 `null` ⇒ 后端 `New`（会话 id 经 early-resolve 轮行回写镜像，`currentSessionId` 随之为非 null ⇒ 触发清单重取）。
- **在挂流不变式**：沿用 `useAgentSession` 的「一条判据挡住双流与中途重放」，面板零新增并发面。

---

## 路由 / API 设计

无 HTTP 路由；Tauri IPC 命令面「一改一增」。

| method（IPC 命令） | path | 说明 | 输入 | 输出 | auth |
|--------------------|------|------|------|------|------|
| `agent_start`（改） | — | 执行 / 查询命令面里的发起命令：参数转换 → 内核 `begin` → 后台转发（Channel 实时出口 + early-resolve 契约不变） | `onEvent: Channel<AgentRunMessage>`（typed）、`root: string`、`prompt: string`、`permissionMode: AgentPermissionMode`、`sessionId: string \| null`、`source: string \| null`、`sourceRef: string \| null`、`agent: number \| null`、`cwd: string \| null`（新增末位） | `Result<TurnSummary, String>`（running 态轮行；`cwd` 缺失 ⇒ `Err` 携路径，不落库不推流） | 无（本地应用，沿既有命令口径） |
| `list_change_chat_sessions`（新） | — | 某 change 的 chat 会话清单（探测语义，分页信封） | `root: string`、`id: string`（change id）、`offset: number`（Rust `u32`）、`limit: number`（Rust `u32`） | `Result<SessionPage, String>`：`{ sessions: SessionSummary[]; total: number }`（`updated_at` 降序；blank root ⇒ 空页） | 无 |

既有命令零改动：`agent_stop` / `agent_session_watch` / `agent_sessions` / `agent_session_transcript` / `session_detail`（重放与续话仍以 `sessionId` 直查，零来源反查）。

参数面漂移守卫：`cwd` 位于**末位** ⇒ 生成绑定里既有 1–7 位逐字不变（`bindings/mod_test.rs` 的逐字断言在测试相位收紧到 `cwd` 收尾）。

---

## 依赖

运行时依赖：**无新增**。

| 依赖 | 变更 | 用途 |
|------|------|------|
| `react-resizable-panels`（v4） | 不变 | `Panel` 的 `collapsible` / `collapsedSize` / `panelRef` / `onResize` 与 `usePanelRef` 供分栏与折叠（`components/ui/resizable.tsx` 已封装 `ResizablePanelGroup` / `ResizablePanel` / `ResizableHandle`） |
| `@base-ui/react`（`drawer`） | 不变 | `Drawer` 的 `modal` 已支持非模态（底层零改动，只差上层透传） |
| `ai` / `@ai-sdk/react` | 不变 | `ChatTransport.sendMessages` 的 `body` 透传通道已在 v7 面内 |
| `tauri` / `tauri-specta` | 不变 | 新增命令与 `cwd` 末位参数的 typed 绑定（`all_commands!` 登记后自动纳入） |
| `std::path::Path` | 不变 | `cwd` 的 `is_dir` 探测（已在 `exec/mod.rs` 引入） |

构建 / 测试依赖：**无新增**（沿用 `vp check` / `knip` / `cargo clippy` / `vitest` / `cargo test`）。

---

## 待决问题

1. **base-ui 非模态的 Esc 行为**：`modal={false}` + `swipeDirection="right"` 下 Esc 是否仍触发 dismiss 需实测（预期由 dismiss 逻辑支持）。若不支持，按 D18 在 `DetailDrawer` 补 Escape keydown 兜底（页脚「关闭」按钮始终可用，spec 条款不变）。
2. **续话的 cwd 一致性缺口（出范围留痕）**：`cwd` 不落 `SessionRecord.config_snapshot`，故从调试页按 `change` 筛选打开同一 chat 会话续话时不会带 cwd，工具跑回主 root（不报错、口径不一致）。本变更只保证「来源侧恒显式传 cwd」（proposal 风险 3）；按 `provenance.source_ref` 反查 worktree 的隐式兜底**显式留作后续统一**，不在此实现。
3. **面板视觉细节**：头区文案 / 窄条宽度（`3rem` 起始值）/ 历史清单展开形态（是否带「加载更多」按钮以外的虚拟滚动）按 D19 留实现期微调；行为面（可见性、禁用理由、testid）已固化，不动 spec。
4. **change 提示词前导（stance）与工具面只读化**：本变更只落 cwd 层，change chat 恒 default 七工具（含 write）。若产品后续要求「change 环境只读 / change 专属提示词」，需新 `SessionToolset` 变体与 `lib/change-stance.ts`，另立变更（proposal 已显式出范围）。
5. **归档 change 的 worktree 复活路径**：归档 Cleanup 后 `ChangeRecord.worktree` 不清空，若该路径被人工重建，面板仍按 `detail.source === 'archive'` 只读（矩阵只认 `source`，不认路径可写性）。若产品希望「归档 change 可恢复对话」，需先定义归档态语义，另立变更。
