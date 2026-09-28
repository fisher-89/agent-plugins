# desktop-explore-page Delta

## MODIFIED Requirements

### Requirement: 对话区呈现

对话区 SHALL 以共享 agent 会话组件族（`components/agent/`，见 desktop-agent-chat-infra）呈现，MUST NOT 保留视图层私有的事件渲染实现：状态面经统一会话基建（`use-agent-chat` + `TauriAgentTransport`），实时流经 transport 走 `agent_start` 的 Channel（执行流通道），历史经 `agent_run_chain` + `agent_run_events` 重放装载进 UIMessage 状态（实时与重放同一状态形状）。呈现面 SHALL 以 `AgentMessages` 对话透镜渲染：user / assistant 消息各成气泡；文本块渲染 markdown、思考块折叠呈现、工具调用渲染为可读卡片并与同 id 工具结果成对呈现（配对收口于适配层）；`AskUserQuestion` 的 ToolUse SHALL 呈现为静态卡片（问题与选项原样可读），答案经 composer 文本输入（questionnaire 接线为二期）。composer SHALL 采用 `AgentInput`：prompt 输入与 permission-mode 档位选择（沿用调试页档位语义与默认值，env 不设参数——运行恒为完整环境），运行中忽略重复发送并提供停止入口。滚动 SHALL 沿用 shadcn `message-scroller` 组合（滚动行为由组件负责，消息状态留在应用层）。

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
- **THEN** 经重放装载重建的对话区呈现与离开前实时呈现形状一致（同一 `AgentMessages` 透镜渲染）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts`（改） | 会话基建接线 | 改走 `use-agent-chat`（重放装载 + 链参数经 body 穿透）；stance 文本组装留本 hook（来源侧）；对外状态/行为语义不变 |
| `views/explores/components/explore-conversation.tsx`（删） | — | 私有渲染实现收编，由 `components/agent/AgentMessages` 取代；`explore-conversation.test.tsx` 用例随迁组件族测试 |
| `views/explores/components/explore-composer.tsx`（改） | composer 收口 | 改用 `AgentInput`（发送 / permission-mode / 停止入口）；stance 拼接仍在上游发送组装 |
| `packages/desktop/src/views/explores/explore-detail-view.tsx`（改） | 详情页装配 | 对话区 / composer 换共享组件族；双栏、预览、watch 生命周期不变 |
