# desktop-change-flow-view Delta

## MODIFIED Requirements

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线（delta 流、可停止）；点击已收口的 WorkerAgent 节点 SHALL 打开该会话的转录重放（不要求运行进程存活）。会话寻址 SHALL 记录 id 直查优先：attempt 记录暴露的会话槽位 id（desktop-change-queries）在场时，转录面板 SHALL 按 session id 直查（`session_detail` + `agent_session_transcript`）；槽位缺席（旧数据）时 SHALL 回退既有 sourceRef 定式反查（`<change>/<phase>/<role>/<attempt>`，同 ref 多会话取最近一条）。eval 节点的抽屉联动 SHALL 覆盖三会话：executor / evaluator / decision 三转录 tab（形态由 design 定稿）；decision 槽位缺席时该 tab SHALL 呈空态，MUST NOT 虚构会话或误挂他 attempt 的会话。WorkerAgent 节点 SHALL 提供显式「查看会话」入口（与节点点击同一抽屉转录联动，不建独立会话 route）。转录呈现 SHALL 复用既有会话基建（`AgentTimeline` / 会话重放，desktop-agent-chat-infra），MUST NOT 为 change 场景另建第二套时间线组件。ToolStep / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果（无会话可联动时抽屉展示步骤输出摘要）。

#### Scenario: 运行中节点实时转录

- **WHEN** executor 会话运行中点击其节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），与 Agent 调试页同款组件

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

#### Scenario: 记录 id 直查优先

- **WHEN** 某 attempt 记录携带会话槽位 id，且同 sourceRef 定式下存在多条会话
- **THEN** 转录面板按槽位 id 精确直查该会话，MUST NOT 落入「同 ref 多会话取最近一条」的反查歧义

#### Scenario: 旧数据回退反查

- **WHEN** 打开不含会话槽位的历史 attempt（旧代际数据）
- **THEN** 转录面板回退 sourceRef 定式反查并照常重放，行为与升级前一致

#### Scenario: decision 第三 tab 与空态

- **WHEN** 点击某 eval 节点打开抽屉，该 attempt 的 decision 槽位在场（或缺席）两种形态分别驱动
- **THEN** 抽屉呈 executor / evaluator / decision 三转录 tab；decision 槽位在场时按 id 直查其转录，缺席时 decision tab 呈空态，executor / evaluator 两 tab 不受影响

#### Scenario: 查看会话显式入口

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点提供显式「查看会话」入口，触发与节点点击相同的抽屉转录联动；无独立会话 route 引入

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 会话寻址升级 | 槽位 id 直查优先（`session_detail` + `agent_session_transcript`）；sourceRef 反查兜底；seq 归并与 running 推导不变 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 联动范围扩展 | `selectionRoleRefs` 补 decision；「查看会话」入口接线；单一交互入口不变 |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 三会话 tab | executor / evaluator / decision；空态不虚构；复用 `AgentTimeline` 无第二套时间线 |
| `packages/desktop/src/views/changes/flow/` 节点组件 | 查看会话入口 | WorkerAgent 节点显式入口；ToolStep / Gate 无会话联动不变 |
| 图结构与取数模型（不改） | 零触点 | `PIPELINE_PHASES` 布局、边推导、显式刷新 + Channel 例外口径不变 |
