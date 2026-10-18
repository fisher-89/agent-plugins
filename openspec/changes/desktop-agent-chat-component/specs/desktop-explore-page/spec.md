# desktop-explore-page Specification (Delta)

## MODIFIED Requirements

### Requirement: 对话区呈现

对话区 SHALL 以会话级统一组件 `AgentSessionView`（`components/agent/`，见 desktop-agent-chat-infra）呈现，MUST NOT 保留视图层私有的事件渲染实现、自有转录查询 hook 或自有消息拼装：状态面经统一会话基建（`use-agent-session` + `TauriAgentTransport`），实时流双起点——发送期走 `agent_start` 的 Channel（执行流通道）、旁观 / 重挂期走会话订阅面 `agent_session_watch`（按会话状态决定是否挂流），历史经 `session_detail(root, sessionId)` 单查 + `agent_session_transcript` 重放装载进 UIMessage 状态（收口以重放收敛，终态同构）。呈现面 SHALL 以 `AgentMessages` 对话透镜渲染：user / assistant 消息各成气泡；文本块渲染 markdown、思考块折叠呈现、工具调用渲染为可读卡片并与同 id 工具结果成对呈现（配对收口于适配层）；`AskUserQuestion` 的 ToolUse SHALL 呈现为静态卡片（问题与选项原样可读），答案经 composer 文本输入（questionnaire 接线为二期）。composer SHALL 采用 `AgentInput`：prompt 输入与 permission-mode 档位选择（沿用调试页档位语义与默认值，env 不设参数——运行恒为完整环境），运行中忽略重复发送并提供停止入口。滚动 SHALL 沿用 shadcn `message-scroller` 组合（滚动行为由组件负责，消息状态留在应用层）。

#### Scenario: 四变体气泡映射

- **WHEN** 一次含文本、思考、工具调用与工具结果的运行完成
- **THEN** 对话区按序呈现 markdown 气泡、可展开思考块、工具卡片与成对结果，AskUserQuestion 卡片问题/选项可读

#### Scenario: composer 发起运行

- **WHEN** 用户在 composer 输入内容并以默认 bypassPermissions 档发送
- **THEN** 以当前 workspace root 为 cwd 发起 `agent_start`，事件实时流入对话区，result 汇总（num_turns / cost / duration / session_id）可读

#### Scenario: 运行中停止

- **WHEN** 会话运行中用户点击 composer 的停止入口
- **THEN** 触发 `agent_stop`，事件流以终止终态收尾且呈现与停止前实时流一致，随后恢复可发送

#### Scenario: 重开重放形状一致

- **WHEN** 用户离开后重新打开该 explore 详情页
- **THEN** 经重放装载重建的对话区呈现与离开前实时呈现形状一致（同一 `AgentSessionView` / `AgentMessages` 透镜渲染）

#### Scenario: 重开时会话仍在运行

- **WHEN** 用户离开后重新打开该 explore 详情页，而该记录会话仍有 running 轮行
- **THEN** 组件先重放已落库转录、再以 `since_seq` 订阅会话实时流续接（token 级增量不缺失于订阅点之后），收口后以重放收敛；MUST NOT 出现重复气泡或缺轮

## Module Contract

### Component: `packages/desktop/src/views/explores/explore-detail-view.tsx`（改）

| 组件 | Props / 用法 | 描述 |
|------|------|------|
| `AgentSessionView` | `root`；`sessionId`（记录回写的会话 id）；`composer` = `<ExploreComposer />` | 对话区唯一呈现入口；实时流双起点（发送期发起流 / 旁观期订阅流）；滚动与消息状态归组件 |
| `ExploreComposer` | `running`；`onSend`；`onStop` | 文案与 testid 前缀（`explore-*`），内部仍为 `AgentInput` 薄适配 |

### Function: `packages/desktop/src/views/explores/hooks/use-explore-session.ts`（改）

| 函数 | 签名 | 描述 |
|------|------|------|
| `useExploreSession` | `(root, record, autoPrompt?) => { sessionId, session, send, stop, ... }` | 改走 `useAgentSession`；stance 文本组装与 auto-send 闸门留本 hook（来源侧）；会话 id 经记录回写 / `ensureExploreSession` 补齐后交组件挂流 |
