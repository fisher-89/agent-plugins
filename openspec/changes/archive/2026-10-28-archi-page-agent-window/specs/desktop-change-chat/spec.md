# desktop-change-chat Specification（delta: archi-page-agent-window）

## ADDED Requirements

### Requirement: 面板骨架归一（共享 SessionPanel）

change 详情左栏面板的**骨架**（头区动作 / 状态行 / 历史清单 / 折叠窄条）SHALL 抽为共享组件 `SessionPanel`（`components/agent/session-panel.tsx`，经 `components/agent` 桶导出），并由本能力与架构页通用会话（`desktop-archi-chat`）共同消费——它是两处**唯一**的面板骨架实现，MUST NOT 在任一视图侧另存第二套头区动作 / 历史清单 / 折叠窄条：

- **行为零变化**：抽骨架 MUST NOT 改变 change 面板的任何可观察行为——`data-testid` 集合（`change-chat-panel` / `-actions` / `-new` / `-history` / `-note` / `-history-list` / `-history-item` / `-history-more` / `-history-empty` / `-history-error` / `-rail` / `-expand` / `-session`）与「头区动作 → 状态行 → 历史清单 → 会话体」的渲染次序 SHALL 逐字保留；既有行为级用例 SHALL 原样通过。
- **testid 前缀参数化**：骨架 SHALL 以 `testIdPrefix` 生成其全部查询挂钩（`change-chat` / `archi-chat` 两套前缀各命中一次），MUST NOT 在骨架内写死单一前缀。
- **业务判据不外移**：可发送 / 可开新会话 / 只读矩阵与禁用理由 SHALL 留在各自来源侧 hook（change 侧仍为 `detail.source` / `activeRun` / 本会话 `running` 三判据），骨架 MUST NOT 自判禁发、MUST NOT 引入第二套判据源。
- **状态行只渲染 `note`**：状态行文案 SHALL 由来源侧 hook 供 `note: string | null`（change 侧 = `disabledReason ??（空态 ? 草案文案 : null）`；`null` = 不占位），骨架 MUST NOT 自行推导文案生成规则。
- **会话流不抽**：骨架 MUST NOT 自建会话流 / 自调 `useAgentSession`——会话状态面由消费者以注入形态传入（既有「对话体单一实现」纪律零变化）。
- **折叠入参可选**：骨架 SHALL 以可选入参 `collapsed` / `onToggleCollapsed` 表达折叠分支；不传时 MUST NOT 呈折叠窄条与展开入口（explore 详情左栏等无会话管理维度的消费面因此零变化，且本变更 MUST NOT 让它们消费骨架）。

#### Scenario: change 面板经骨架渲染且 testid 不变

- **WHEN** 打开 change 详情左栏面板并审查渲染树
- **THEN** 头区动作 / 状态行 / 历史清单 / 折叠窄条来自共享骨架组件，`change-chat-*` testid 集合与文案逐字与抽骨架前一致；既有行为级用例全绿（零用例语义修改）

#### Scenario: 两处共用同一实现

- **WHEN** 检索 change 详情与架构页两处面板的实现
- **THEN** 两处均消费同一骨架组件（仅 `testIdPrefix` / 文案 / 注入的状态面不同），仓库内不存在第二份头区动作或历史清单实现

#### Scenario: 骨架不自判禁发

- **WHEN** 审查骨架组件源码
- **THEN** 其中零 `detail.source` / `activeRun` / `canSend` / `disabledReason` 的业务判定，零 `useAgentSession` 调用；判据与 `note` 全部来自传入的状态面

#### Scenario: 折叠入参可选

- **WHEN** 以不传 `collapsed` 的方式消费骨架（如未来的只读浏览面）
- **THEN** 不呈折叠窄条与展开入口，面板始终为展开形态；explore 详情左栏在本变更前后行为一致（不消费骨架）

## Module Contract

### Component: `packages/desktop/src/components/agent/session-panel.tsx`（新增）

| 组件 | Props | 描述 |
|------|-------|------|
| `SessionPanel` | `{ state: SessionPanelState; title: string; testIdPrefix: string; composer: (actions: AgentSessionActions) => React.ReactNode; collapsed?: boolean; onToggleCollapsed?: () => void }` | 会话面板骨架唯一实现：折叠窄条 / 头区动作（新会话 + 历史 ▾）/ 状态行（`state.note`）/ 历史清单（分页 + 加载 / 空 / 错误态）/ `AgentSessionView` 注入形态 + composer 插槽；全部查询挂钩由 `testIdPrefix` 生成；不含业务判据、不建会话流 |
| `SessionPanelState` | `session` / `sessions` / `total` / `listOpen` / `listLoading` / `listError` / `toggleList` / `loadMore` / `newSession` / `selectSession` / `canSend` / `canStart` / `disabledReason` / `note` / `send` / `stop` | 两处面板的公共子集（会话面 + 清单面 + 动作面 + `note`）；类型命名由 design 定型，语义契约不变 |

### Component: `packages/desktop/src/components/agent/index.tsx`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| 桶导出 | 增 `SessionPanel` / `SessionPanelState` | 既有 `AgentInput` / `AgentSessionView` 导出零变化 |

### Function: `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts`（改）

| 项 | 签名 | 描述 |
|------|------|------|
| `ChangeChatState`（增位） | `note: string \| null` | `disabledReason ??（currentSessionId === null ? 草案文案 : null）`；判据矩阵、清单取数、新会话 / 历史切换、会话面零改动 |

### Component: `packages/desktop/src/views/changes/agent/change-chat-panel.tsx`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `ChangeChatPanel` | 自持骨架 → 骨架消费者 | props 面零变化；内部改为 `SessionPanel`（`testIdPrefix = "change-chat"`）+ `useChangeChat` 状态面 + `ChangeChatComposer` 插槽；折叠真相仍取分栏 `onResize` 回传 |
