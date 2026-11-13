# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision / spec_sync / resolve_conflicts）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线，点击已收口的 WorkerAgent 节点 SHALL 打开转录重放。会话寻址 SHALL 为节点 `sessionId` 直查：节点记录暴露的会话槽位 id（`sessionId`）经会话查询面精确寻址（`sessionDetail` + `agentSessionTranscript`），前端 MUST NOT 再以 `agentSessions` 按 `source_ref` 过滤反查会话（sourceRef 反查路径整体退役）。呈现 SHALL 经会话级统一组件 `AgentSessionView`（desktop-agent-chat-infra）：WorkerAgent 步面板 SHALL 接 **composer 插槽**，呈「补充说明输入 + 停止」两动作（补充说明 = `agent_steer(root, sessionId, text)` 薄包装，见 desktop-agent-execution「运行中会话的补充说明注入（steer 通道）」；停止复用既有 `agent_stop`，终止该步会话使 walker 收 `AgentRunStatus::Stopped`），MUST NOT 呈现 permission-mode 档位（本面板不发起新轮）、MUST NOT 呈现新会话 / 会话切换入口；补充说明输入的可用性 MUST NOT 由前端判据预判（MUST NOT 以库态 `running`、节点状态或「该步已收口」禁用输入与发送——三种不可送达各由后端 `Err` 文案说话，见「运行控制入口与 ask 应答」的同款纪律）。其余只读面（历史重放、无 composer 的消费点）形态不变。运行中会话的事件增量 SHALL 经会话订阅面 `agent_session_watch`（载荷直投，含 delta）呈现 token 级流式，MUST NOT 以「通知 → 时间去抖重查转录库」作为实时面（delta 不落库，重查只能见密封粒度）；轮收口 SHALL 以转录库重放收敛（收口后形状与重开一致）。变更通知中的会话事件（`SessionEvent`）SHALL 仅作会话状态重评估信号（触发详情重查 → 组件重评估是否挂流），MUST NOT 作为增量数据源。槽位缺席（`role !== null` 而 `sessionId === null`）时 SHALL 呈「此步无会话记录」步面空态占位（不渲染 composer——无会话可注入），MUST NOT 回退反查、MUST NOT 虚构会话、MUST NOT 建立任何订阅。归档步节点（spec_sync / resolve_conflicts）SHALL 同样经其节点 `sessionId` 直查转录（无 attempt 三槽位概念）。后端 provenance `source_ref` 定式（`<id>/<phase>/<role>/<attempt>`）仍由 walker 在会话创建时写入并持久化于会话记录（见 desktop-change-orchestration「会话挂靠 provenance 与转录 session id 直查」），但前端转录面板不再以其作反向寻址依据。

eval 节点与 active 节点的三会话联动（executor / evaluator / decision 三 tab）SHALL 整体退役（eval / active 节点已移除）；ToolStep / Git / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果——无 role 且无 `outputRef` 的 ToolStep / Git / Gate 节点不可点击（`detail` 短文本已在节点内显示）；带 `outputRef` 的 static-check / test-execution / mutation-test 节点经 stdout 面板呈现输出（见「节点点击判据与 stdout 输出面板」）。

#### Scenario: 归档步节点转录直查

- **WHEN** 点击 archive 相位的 SpecSync 或 ResolveConflicts 节点（`session_id` 在案）
- **THEN** 抽屉单列经节点 `session_id` 直查该会话转录（实时流 / 重放一致）

#### Scenario: 槽位缺席呈步面空态

- **WHEN** 打开 WorkerAgent step 节点且其 `sessionId` 缺席（缺省落账）
- **THEN** 转录面板呈「此步无会话记录」空态占位（react 面零取数），MUST NOT 回退 sourceRef 定式反查、MUST NOT 发起 `agent_sessions` 反向查询

#### Scenario: 运行中节点实时转录经会话订阅面

- **WHEN** executor 会话运行中点击其 WorkerAgent 节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），数据经会话订阅面 `agent_session_watch` 载荷直投呈现（含 `messageDelta`），与 Agent 调试页同款组件；收口后以转录库重放收敛

#### Scenario: 步会话窗口补充说明

- **WHEN** executor 会话运行中，用户在抽屉步面板输入一句补充说明并提交
- **THEN** 提交经 `agent_steer(root, sessionId, text)` 抵达后端；送达后该句以一条 user 气泡出现在该会话时间线中段（id `evt-<seq>`），发送按钮在 ack 回流前呈「排队中…」、回流后复位；该步会话与 run 的状态零变化（同一轮、同一 node、同一 instance，token / 耗时归该步）

#### Scenario: 步面板无可预判的禁用态

- **WHEN** 打开一个 `sessionId` 在案但该步已收口的 WorkerAgent 节点，输入并提交补充说明
- **THEN** 输入与发送按钮未被前端禁用，调用到达后端并返回 `Err`（「会话已收口」类文案），该文案呈现在会话错误面；MUST NOT 以「已收口」为由隐藏或禁用 composer

#### Scenario: 步面板停止杀伤该步会话

- **WHEN** 运行中在抽屉步面板点停止
- **THEN** 触发 `agent_stop(root, sessionId)`，该步会话收敛 `stopped`、walker 相应收敛为受控终态；停止入口随库态 `running` 消失

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

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Component） |
|------|------|------|
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`（扩） | 步会话窗口（首个补充说明消费点） | 自持形态 `AgentSessionView` 增 `composer` 插槽：插槽内 `AgentInput`（`idPrefix = "step-session"`、`hidePermissionMode`、`pending = actions.steering`、`running = actions.running`、`onSend = ({ text }) => actions.steer(text)`、`onStop = actions.stop`）；`askAnswer` 注入位照旧（ask 例外不变）；槽位缺席（`roleRef` 缺席 / `sessionId === null`）仍呈空态且**不渲染 composer**（无会话可注入） |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx`（零改动） | 抽屉透传 | 继续只透传 `root` / `roleRef` / `refreshKey` / `askAnswer`；补充说明动作面由面板内部从 `AgentSessionActions` 取，不经抽屉 |
| `packages/desktop/src/views/changes/change-detail-view.tsx`（零改动） | 页组装 | `askAnswer` 等既有组装零改动；补充说明不需要新增页级动作面（面板自足） |
| `crates/infra/store` / `crates/core/orchestration`（零改动） | 落库与编排 | 补充说明只落在会话转录（既有 `role = user` 密封事件），step 的 token / 耗时归位天然成立；`ChangeFlowControl` / walker 零改 |
