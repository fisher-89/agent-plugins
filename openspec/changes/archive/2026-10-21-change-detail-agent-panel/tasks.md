# 任务: change-detail-agent-panel

> 有序实现任务（按依赖排序，前置项解锁后续项）。测试文件（`exec/mod_test.rs`、`bindings/mod_test.rs`、`agent-transport.test.ts`、`use-agent-session.test.ts`、`agent-input.test.tsx`、`change-detail-view.test.tsx`、`detail-drawer.test.tsx`、新增面板 / hook 测试）由测试相位承接，不入本清单。

## 阶段 1 — 后端命令面（`cwd` 通道 + chat 清单）

- [x] `src-tauri/src/commands/exec/mod.rs`：新增模块私有常量 `CHANGE_CHAT_SOURCE: &str = "change"`（doc 注明清单筛选的来源词，写侧同词由前端受控常量给出）
- [x] `agent_start` 签名尾部增 `cwd: Option<String>` 并透传 `agent_start_with`（命令体仍只做参数转换，`#[allow(clippy::too_many_arguments)]` 已在位）
- [x] `agent_start_with` 签名尾部增 `cwd: Option<String>`：在 `is_blank_root` 之后插入执行根解析——`cwd` 去空白后为空 → 回落 `root`；`Some(p)` 且 `Path::new(p).is_dir()` → `p.to_owned()`；否则 `Err("worktree 目录不存在（可能已被手动删除）: {p}；请恢复目录或手动清理后重建 change")`（字面量与 `change_flow_start` 同源，不抽公共函数）
- [x] `agent_start_with` 内把 `SessionCtx.workspace_root` 改用解析后的执行根；`stores.for_root(&root)` 逐字不动；同步改写该段注释（原「cwd 恒为当前 workspace root」→ 双 root 表述：store 半边恒 root、执行根可被 `cwd` 覆盖）
- [x] `src-tauri/src/commands/exec/mod.rs` 新增 `list_change_chat_sessions(stores, root, id, offset, limit) -> Result<SessionPage, String>`：`is_blank_root` → `SessionPage { sessions: vec![], total: 0 }`；否则 `for_root(&root)` → `session_query(store).list_sessions(Some(CHANGE_CHAT_SOURCE), Some(&id), offset, limit)`
- [x] `src-tauri/src/commands/mod.rs`：`all_commands!` 增 `$crate::commands::exec::list_change_chat_sessions,`
- [x] 再生成生成物：`pnpm -C packages/desktop run bindings:export`（产出 `src/types/generated/bindings.ts`：`agentStart` 尾部 `cwd: string | null` + 新 `listChangeChatSessions`；禁手改）

## 阶段 2 — 会话参数面透传（transport + 会话基建）

- [x] `lib/agent-transport.ts`：`AgentStartSessionParams` 增 `sourceRef: AgentStartArgs[6]` / `cwd: AgentStartArgs[8]`（类型仍由生成绑定索引派生，保持无手写镜像）
- [x] `lib/agent-transport.ts`：`readSessionParams` 增读 `sourceRef` / `cwd`（复用 `readNullableString`，非法形态 `Err`）
- [x] `lib/agent-transport.ts`：`invokeStart` 改具名解构（`const { root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd } = session`）后按生成绑定的位置签名传参，删除 `null` 占位与位置索引魔法
- [x] `hooks/use-agent-session.ts`：`UseAgentSessionParams` 增 `sourceRef?: string | null` / `cwd?: string | null`
- [x] `hooks/use-agent-session.ts`：`useSessionActions` 解构两字段，`sendMessage` body 以 `sourceRef: sourceRef ?? null` 替换原 `sourceRef: null` 常量、新增 `cwd: cwd ?? null`，依赖数组随动

## 阶段 3 — composer 整面板禁用态

- [x] `components/agent/agent-input.tsx`：`AgentInputProps` 增 `disabled?: boolean` / `disabledReason?: string`
- [x] `components/agent/agent-input.tsx`：`AgentInput` 渲染理由行（`data-testid={idPrefix}-disabled-reason`，仅 `disabled && disabledReason` 时呈现）、textarea 加 `disabled={disabled}`
- [x] `components/agent/agent-input.tsx`：`ComposerToolbar` 增 `disabled` 入参——发送按钮 `disabled={disabled || running || prompt 空白}`，停止入口条件改 `running && !disabled`（`running` 既有语义零变化）

## 阶段 4 — change chat hook（状态与判据单源）

- [x] 新建 `views/changes/agent/hooks/use-change-chat.ts`：模块常量 `CHANGE_CHAT_SOURCE = 'change'` / `CHANGE_CHAT_PAGE_SIZE = 20` 与类型 `ChangeChatParams` / `ChangeChatState`
- [x] 内部 `useChangeChatList`：`listChangeChatSessions(root, changeId, offset, limit)` 取数（`items` / `total` / `loading` / `error`），支持首屏重取与「加载更多」累加（`items.length < total`）；`root` 或 `changeId` 为 null 时零 invoke
- [x] `useChangeChat`：`session = useAgentSession({ root, sessionId, source: CHANGE_CHAT_SOURCE, sourceRef: changeId, cwd, })`——`cwd = archived ? null : worktree`，**不传** `refreshKey`
- [x] `useChangeChat`：清单取数时机收敛为三处——①`toggleList` 展开时重取首屏 ②`currentSessionId`（`session.session?.row.id ?? null`）由 null 变非 null 时重取 ③`running` 由 true 变 false 时重取（`wasRunning` ref 记录转折）；挂载零清单 invoke
- [x] `useChangeChat`：`newSession()` = `session.reset()` + `sessionId = null`（回可发送草案态、零落库）；`selectSession(id)` = 置 `sessionId` 并收起清单；`changeId` 变化 effect 重置 `sessionId = null` / `listOpen = false` / 清空清单
- [x] `useChangeChat`：判据面 `disabledReason = archived ? '已归档 change 只读' : runActive ? '流程运行中' : null`、`canSend = disabledReason === null`、`canStart = disabledReason === null && !session.running`（`sessionId === null` 不入判据、不产生第三条理由）
- [x] `useChangeChat`：`send` / `stop` 包装——`send` 在 `disabledReason !== null` 或 `session.loading` 时早退，否则透传 `session.sendMessage({ prompt, permissionMode })`；`stop` 透传 `session.stop`

## 阶段 5 — 面板与 composer 组件

- [x] 新建 `views/changes/agent/change-chat-composer.tsx`：`ChangeChatComposer`（`AgentInput` 薄适配，`idPrefix="change-chat"`，透传 `running` / `disabled` / `disabledReason` / `onSend` / `onStop`）
- [x] 新建 `views/changes/agent/change-chat-panel.tsx`：`ChangeChatPanelProps` 与 `ChangeChatPanel` 骨架（`data-testid="change-chat-panel"`），内部调 `useChangeChat`（`worktree = cwd`、`archived = readOnly`、`runActive`）
- [x] `change-chat-panel.tsx`：头区动作面——「新会话」（`data-testid="change-chat-new"`，`disabled={!canStart}`）、「历史 ▾」开关（`data-testid="change-chat-history"`，恒可用）
- [x] `change-chat-panel.tsx`：状态提示行（`data-testid="change-chat-note"`）——禁发/只读理由（`disabledReason`）优先，否则草案态文案「发送首条消息即开启新会话」；另起一行执行根文案（`data-testid="change-chat-cwd"`：`cwd ?? root`，不承诺跨根可读）
- [x] `change-chat-panel.tsx`：对话体走 `AgentSessionView` 注入形态（`session={state.session}`）+ composer 插槽 `<ChangeChatComposer running disabled={!canSend} disabledReason=… onSend=… onStop=… />`；面板内零 `eventsToUIMessages` 直调、零自有转录查询
- [x] `change-chat-panel.tsx`：历史清单（`listOpen` 时展开；每项 `data-testid="change-chat-history-item"`，呈 id 前缀 / `row.updatedAt` / `stats.turnCount`；空态 / loading / error 文案；`items.length < total` 时呈「加载更多」调 `loadMore`）
- [x] `change-chat-panel.tsx`：折叠窄条——`collapsed` 时只渲染竖向标题 + 展开按钮（`data-testid="change-chat-expand"`）调 `onToggleCollapsed`

## 阶段 6 — 详情分栏与抽屉非模态

- [x] `views/changes/change-detail-view.tsx`：新增模块常量 `CHAT_PANEL_DEFAULT_SIZE` / `CHAT_PANEL_MIN_SIZE` / `CHAT_PANEL_COLLAPSED_SIZE` / `CHAT_PANEL_RAIL_PX`
- [x] `components/ui/resizable.tsx`：追加 `export { usePanelRef } from 'react-resizable-panels';`（折叠命令面出口，视图仍只 import `@/components/ui/resizable`）
- [x] `change-detail-view.tsx`：新增私有 `useChatPanelState()`（`usePanelRef()` + `collapsed` state + `toggle` + `setCollapsed`；`toggle` 依 `panelRef.current.isCollapsed()` 分派 `collapse()` / `expand()`）
- [x] `change-detail-view.tsx`：`DetailHeader` 增 `chatCollapsed` / `onToggleChat` 两参，渲染开关按钮（`data-testid="detail-chat-toggle"`，文案「隐藏通用 Agent」/「显示通用 Agent」），既有返回 / 刷新 / 标题 / 状态 / worktree 呈现不变
- [x] `change-detail-view.tsx`：`ChangeDetailView` 增 change id 变化的折叠复位 effect（`panelRef.current?.isCollapsed()` 为真则 `expand()`）
- [x] `change-detail-view.tsx`：`DetailLoadedProps` 增 `chatPanelRef` / `chatCollapsed` / `onChatCollapseChange`；`DetailLoaded` 改 `ResizablePanelGroup orientation="horizontal"`（`className="min-h-0 flex-1"`）——header 整宽留在分栏之上
- [x] `change-detail-view.tsx`：左 `ResizablePanel`（`defaultSize={CHAT_PANEL_DEFAULT_SIZE}` / `minSize={CHAT_PANEL_MIN_SIZE}` / `collapsible` / `collapsedSize={CHAT_PANEL_COLLAPSED_SIZE}` / `panelRef` / `onResize={(size) => onChatCollapseChange(size.inPixels <= CHAT_PANEL_RAIL_PX)}`）+ `ResizableHandle withHandle` + 右 `ResizablePanel`
- [x] `change-detail-view.tsx`：左栏挂 `ChangeChatPanel`（`root` / `changeId={id ?? detail.id}` / `cwd={detail.worktree}` / `readOnly={detail.source === 'archive'}` / `runActive={activeRun !== null}` / `collapsed` / `onToggleCollapsed={toggle}`）；右栏内保持 `RefreshStaleNote` → `DetailActionPanels` → `DetailContent` 次序（`DetailContent` / `DetailDrawer` 装配与 `selection` 语义不变）
- [x] `components/standard/drawer.tsx`：`StandardDrawerProps` 增 `modal?: boolean`，`StandardDrawer` 解构缺省 `true` 并传入 `<Drawer modal={modal}>`（不进入 `{...rest}`，避免落到 `DrawerContent` 的 DOM 属性面）
- [x] `views/changes/flow/detail-drawer.tsx`：`<StandardDrawer … modal={false}>`；文件头注释改述非模态关闭入口（页脚按钮 / Esc）；实测 base-ui 在 `modal={false}` 下 Esc 是否 dismiss，若不触发则在本文件补 Escape keydown 兜底（`selection === null` 时不挂监听）

## 阶段 7 — 版本面

- [x] `packages/desktop/package.json`：`version` `"0.5.19"` → `"0.5.20"`
