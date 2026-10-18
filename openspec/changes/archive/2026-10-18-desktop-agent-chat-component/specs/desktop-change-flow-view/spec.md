# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision / spec_sync / resolve_conflicts）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线，点击已收口的 WorkerAgent 节点 SHALL 打开转录重放。会话寻址 SHALL 为节点 `sessionId` 直查：节点记录暴露的会话槽位 id（`sessionId`）经会话查询面精确寻址（`sessionDetail` + `agentSessionTranscript`），前端 MUST NOT 再以 `agentSessions` 按 `source_ref` 过滤反查会话（sourceRef 反查路径整体退役）。呈现 SHALL 经会话级统一组件 `AgentSessionView`（desktop-agent-chat-infra，只读消费点：不接 composer 插槽、不呈现发送 / 停止）：运行中会话的事件增量 SHALL 经会话订阅面 `agent_session_watch`（载荷直投，含 delta）呈现 token 级流式，MUST NOT 以「通知 → 时间去抖重查转录库」作为实时面（delta 不落库，重查只能见密封粒度）；轮收口 SHALL 以转录库重放收敛（收口后形状与重开一致）。变更通知中的会话事件（`SessionEvent`）SHALL 仅作会话状态重评估信号（触发详情重查 → 组件重评估是否挂流），MUST NOT 作为增量数据源。槽位缺席（`role !== null` 而 `sessionId === null`）时 SHALL 呈「此步无会话记录」步面空态占位，MUST NOT 回退反查、MUST NOT 虚构会话、MUST NOT 建立任何订阅。归档步节点（spec_sync / resolve_conflicts）SHALL 同样经其节点 `sessionId` 直查转录（无 attempt 三槽位概念）。后端 provenance `source_ref` 定式（`<id>/<phase>/<role>/<attempt>`）仍由 walker 在会话创建时写入并持久化于会话记录（见 desktop-change-orchestration「会话挂靠 provenance 与转录 session id 直查」），但前端转录面板不再以其作反向寻址依据。

eval 节点与 active 节点的三会话联动（executor / evaluator / decision 三 tab）SHALL 整体退役（eval / active 节点已移除）；ToolStep / Git / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果——无 role 且无 `outputRef` 的 ToolStep / Git / Gate 节点不可点击（`detail` 短文本已在节点内显示）；带 `outputRef` 的 static-check / test-execution 节点经 stdout 面板呈现输出（见「节点点击判据与 stdout 输出面板」）。

#### Scenario: 归档步节点转录直查

- **WHEN** 点击 archive 相位的 SpecSync 或 ResolveConflicts 节点（`session_id` 在案）
- **THEN** 抽屉单列经节点 `session_id` 直查该会话转录（实时流 / 重放一致）

#### Scenario: 槽位缺席呈步面空态

- **WHEN** 打开 WorkerAgent step 节点且其 `sessionId` 缺席（缺省落账）
- **THEN** 转录面板呈「此步无会话记录」空态占位（react 面零取数），MUST NOT 回退 sourceRef 定式反查、MUST NOT 发起 `agent_sessions` 反向查询

#### Scenario: 运行中节点实时转录经会话订阅面

- **WHEN** executor 会话运行中点击其 WorkerAgent 节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），数据经会话订阅面 `agent_session_watch` 载荷直投呈现（含 `messageDelta`），与 Agent 调试页同款组件；收口后以转录库重放收敛

#### Scenario: 通知仅作状态重评估

- **WHEN** 运行中收到会话事件通知（`SessionEvent`）而该会话订阅流已在挂
- **THEN** 通知只触发会话状态重评估（详情重查 / 是否重挂判断），MUST NOT 触发转录库重查以「追增量」、MUST NOT 重建第二条订阅流

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

#### Scenario: 记录 id 精确直查

- **WHEN** 某节点记录携带会话槽位 id
- **THEN** 转录面板按槽位 id 精确直查该会话，MUST NOT 落入「同 ref 多会话取最近一条」的反查歧义（前端不再持有 sourceRef 定式）

#### Scenario: decision 第三 tab 与空态

- **WHEN** 点击某 attempt 的 decision 槽位节点，该槽位在场（或缺席）两种形态分别驱动
- **THEN** 该槽位在场时按 id 直查其转录，缺席时呈「此步无会话记录」空态占位，其余 role 节点不受影响

#### Scenario: 会话槽位缺席不虚构

- **WHEN** 打开 `role !== null` 而 `session_id` 为空（未开跑 / 缺省落账）的 WorkerAgent 步节点
- **THEN** 抽屉呈「此步无会话记录」空态占位，MUST NOT 经 provenance 反查虚构会话、全程不订阅实时事件流

#### Scenario: 查看会话按钮不呈现

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点上无「查看会话」按钮，点击节点本体即打开抽屉查看转录；无独立会话 route 引入

## Module Contract

### Component: `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`（改）

| 组件 | Props | 描述 |
|------|------|------|
| `SessionTranscriptPanel` | `root`；`roleRef`；`refreshKey?`（会话状态重评估信号） | 抽屉单列会话面：`roleRef.sessionId` 直查 + `AgentSessionView` 只读渲染（无 composer）；槽位缺席呈空态占位且零订阅 |
| `SessionMeta` | `summary`；`running` | 会话 id / 运行中徽章 / 轮数 / token 摘要（自组件状态读取，不再自带查询） |

### 退役: `packages/desktop/src/views/changes/hooks/use-session-transcript.ts`（删）

| 项 | 变化 | 描述 |
|------|------|------|
| `useSessionTranscript` | 收编进 `AgentSessionView` / `useAgentSession` | 会话直查与重放装配由统一组件承担；`liveEvents` 死参数与 `sourceRef` 退役叙述随之删除，MUST NOT 保留第二套转录查询 hook |

### Function: `packages/desktop/src/views/changes/hooks/use-change-detail.ts`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `notifyTranscript` / `transcriptTick` | 退役 | 转录专用 150ms 去抖重查键删除；会话事件通知改走统一视图重查 tick（组件据 `refreshKey` 重评估会话状态） |
| `notifyRefresh` | 不变 | 300ms 尾随去抖 + 有界重试语义不动（见「通知重查失败降级与有界重试」） |
