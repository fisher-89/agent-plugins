# desktop-agent-chat-infra Delta

## ADDED Requirements

### Requirement: 会话系统提示词快照的只读可见面

会话级统一渲染组件 SHALL 提供会话系统提示词快照的**只读**呈现面，使「这段会话到底被塞了什么系统提示词、其中的 `AGENT.md` 段是什么内容」在界面上可查（快照语义与出线形状归 `desktop-agent-execution`「会话系统提示词快照的拍存与续轮复用」与「session_detail 单查面」；本能力只作呈现消费者）：

- **入口**：`AgentSessionView` 元信息行 SHALL 在 `state.session.preamble !== null` 时渲染「系统提示词」入口（按钮形态、`data-testid="session-meta-preamble"`、带可访问名）；`preamble` 为 `null`（无快照 / 存量会话 / 首轮尚未重放）时 MUST NOT 渲染入口，也 MUST NOT 占位。
- **呈现形态**：点击 SHALL 以标准抽屉（`StandardDrawer`，标题「系统提示词快照」）打开，内容 SHALL 含三段——① 拍存时刻与体量（`savedAt` 按本地时间呈现；体量由 `content` 的 UTF-8 字节数就地计算，MUST NOT 依赖额外出线字段）；② **静态指引文案**（声明本会话系统提示词冻结于会话开始、规则真身为 `AGENT.md`、需要更新时直接要求 agent 重新读取 `AGENT.md`）；③ 快照正文全文（等宽字体、保留换行、可滚动、可选中复制）。
- **数据来源单一**：内容 SHALL 全部取自 `SessionSummary.preamble`（随 `session_detail` 已到手）——零新增 IPC 调用、零轮询、零订阅、零文件读取；内聚与否由本组件自持开合态（不新增 `SessionPanelState` 字段、不改三处来源 hook 的状态面）。
- **零写面**：抽屉内 MUST NOT 出现编辑框 / 提交按钮 / 重载按钮等任何写动作（快照不可变，无重载写面）；MUST NOT 触发会话发起、续轮或停止。
- **不进对话流**：快照正文 MUST NOT 作为消息或部件插入消息列表（它不是历史消息）；消息列表部件数与内容 SHALL 因该面存在而零变化。
- **不预判文案规则**：静态指引文案为组件内常量（与笔记 `note` 槽位无关，MUST NOT 借 `SessionPanelState.note` 承载）。

#### Scenario: 有快照时呈现入口与抽屉

- **WHEN** 打开一条已拍快照会话（`session_detail` 返回的 `preamble` 非空）并点击元信息行的「系统提示词」入口
- **THEN** 抽屉打开并呈现拍存时刻 / 体量、静态指引文案与快照正文全文；正文与首轮 System 消息逐字相同

#### Scenario: 无快照不渲染入口

- **WHEN** 打开一条 `preamble` 为 `null` 的会话（存量会话或首轮尚未收口重放）
- **THEN** 元信息行无「系统提示词」入口、无占位元素；会话其余呈现与既有一致

#### Scenario: 零取数与零写面

- **WHEN** 打开抽屉并停留
- **THEN** 不发起任何新增 IPC 取数、不建立订阅；抽屉内无输入框与提交控件，MUST NOT 触发会话发起 / 续轮 / 停止

#### Scenario: 不进对话流

- **WHEN** 打开与关闭该抽屉
- **THEN** 消息列表的消息数与部件数零变化，快照正文不出现在对话流中

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/components/agent/agent-session-view.tsx` | 元信息行入口 + 只读抽屉 | `SessionMeta` 依 `state.session.preamble` 决定是否渲染入口（`data-testid="session-meta-preamble"`）；抽屉复用 `StandardDrawer`，四段内容全取自 `SessionSummary.preamble`；零取数 / 零订阅 / 零写面 / 不进消息流 |
| `packages/desktop/src/hooks/use-agent-session.ts` | 状态面消费（零新增字段） | `sessionFromTurn` 的 `SessionSummary` 字面补 `preamble: null`（本地轮行镜像无快照面）；重放路径 `applyReplay` 原样落 `session_detail` 的 `preamble`，收口重放后入口自然出现 |
| `packages/desktop/src/components/agent/session-panel.tsx` | 骨架（**零改动**） | `SessionPanelState` 字段面不变；本能力的新面落在 `AgentSessionView` 元信息行，`note` 槽位优先级不受影响 |

### Component: `packages/desktop/src/components/agent/agent-session-view.tsx`

| 项 | 变化 | 描述 |
|------|------|------|
| `SessionMeta` | 增入口 | `summary.preamble !== null` → 渲染「系统提示词」按钮；点击开抽屉（组件内 `useState` 开合态，`onOpenChange` 透传 `StandardDrawer`） |
| 抽屉内容 | 新增（组件内） | 拍存时刻 / 体量（按正文字节数就地算）/ 静态指引 / 正文（`whitespace-pre-wrap` + 等宽 + 可滚动）；只读、零写面 |
| `AgentSessionView` 导出面 | 零变化 | 不新增导出类型或函数（抽屉为组件内部实现），`index.tsx` 桶导出与 `AgentSessionActions` / `AgentSessionViewProps` 零变更 |
