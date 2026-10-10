# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的节点 SHALL 呈现运行态（pulse 视觉，与既有 active 节点同款），ToolStep 节点（phase-start / static-check / phase-log / backtrack 相位机步）与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（类型可区分）。运行态呈现 SHALL 与既有节点模型并存：eval（实心 pass 绿 / fail 红）、stale 淡化规则不变；`PIPELINE_PHASES` 列布局、时间序边推导规则与空列恒定 MUST NOT 因运行态呈现改变。

运行收口后，图 SHALL 保持**步节点常驻渲染**（「图回落 ChangeDetail 派生规则」语义退场）：已落库步节点（executor / evaluator / decision / static-check / test-execution，desktop-change-state-store 五词汇）自统一视图的 runs[].steps 派生常驻上图，与运行中实时面走**同一条派生路径**（同一转换函数消费合并查询结果，而非结果碰巧一样）；运行态视觉（pulse）褪去、终态样式呈现。常驻渲染 SHALL 取**全史叠加**（多次 run 的步节点全部保留，非仅最近 run）：attempt 经跨 run max+1 分配不撞号，全史叠加在同一列面、run 归属分层经 run 史可查；在飞 run 的活步叠加其上，步节点键 `phase:attempt:step` 天然不冲突。

#### Scenario: 运行态与两分类并存

- **WHEN** implement#2 相位运行中（executor 会话活跃）且历史上有 fail→retry 与 active_phase 残留
- **THEN** 图上同时呈现：历史 eval 节点（实心三色）、active_phase 的 active 节点（pulse）与当前 executor 节点（pulse 运行态）及同相位的 ToolStep / Gate 节点（类型可辨、状态如实）

#### Scenario: 收口后常驻不回落

- **WHEN** run 以全部相位 pass 收口（或 fail / stopped 收口）后刷新 / 重开详情
- **THEN** 已落库步节点常驻上图（终态样式、无 pulse），派生自统一视图库读史、与运行中同一转换函数；无「回落后步节点消失」的形态跳变

#### Scenario: 全史叠加与在飞叠加

- **WHEN** 某 change 历经两次 run 后第三次 run 运行中
- **THEN** 图上前两 run 的步节点全史常驻，第三 run 活步（pulse）叠加其上；同 phase 节点 attempt 递增不撞键，任一步节点可溯其 run 归属

### Requirement: 取数模型与管线合规不变

视图改造 MUST NOT 改变取数契约：详情更新 SHALL 仅由显式 refresh（或选中 change 变化）触发，MUST NOT 引入 watch 订阅、定时轮询或事件订阅（live 刷图留作后续独立 change）。**例外（desktop-change-flow 升级）**：change run 运行期间，执行视图 SHALL 经运行状态 Channel 接收**变更通知**并触发统一视图重查（执行流通道例外，沿 agent 执行先例，不属轮询取数；通知仅失效信号、查询结果为权威——unify-run-state-persistence 推拉反转，原「Channel 刷新 MUST NOT 触发 `get_change_detail` 全量重取（节点状态为增量并入）」条款随之退场），run 收口后 MUST NOT 保持订阅。前端 run 状态机镜像 SHALL 解散：`applyRunUpdate` 载荷累积、liveEvents 事件缓存与 seq 去重随通知降位退役，客户端态退化为「查询缓存 + 失效重取」；两钩并一（`useChangeFlowRun` 缩为控制动作 + 通知订阅，统一视图取数由详情取数承接——合并查询契约见 desktop-change-orchestration「运行控制命令面」）。desktop 前端 SHALL 维持全管线通过：`vp check --fix`（fmt / lint，含 `max-lines-per-function: 50`）、knip（无未用导出残留）、`vp test` 全绿；测试 SHALL 以 data-testid 为查询挂钩，MUST NOT 以样式类名查询。

#### Scenario: 运行中通知触发重查

- **WHEN** change run 运行期间某相位推进（节点状态变化通知到达）
- **THEN** 执行视图重查统一视图（库读史 ∪ 在飞 run）并重绘，无前端载荷累积 / seq 去重逻辑；run 收口后订阅释放，后续变更回到显式刷新

#### Scenario: 运行外显式刷新仍是唯一更新途径

- **WHEN** 无运行中 change run 时，流程图呈现期间 change 的库状态被外部进程修改
- **THEN** 图不自动变化；点击「刷新详情」后经 `get_change_detail` 重取并重绘

#### Scenario: 全管线通过

- **WHEN** 运行 `pnpm -C packages/desktop run client:check` 与 `pnpm -C packages/desktop run test`
- **THEN** fmt / lint / knip / 测试全部通过，`package.json` 与 knip.json 无新增豁免条目

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线（delta 流、可停止）——实时面数据源 SHALL 为**转录库重查**（变更通知触发、`use-session-transcript` 既有查询面消费；会话事件流式落库），前端 liveEvents 内存缓存随通知降位退役，实时与重放口径统一到转录库；点击已收口的 WorkerAgent 节点 SHALL 打开该会话的转录重放（不要求运行进程存活）。会话寻址 SHALL 记录 id 直查优先：attempt 记录暴露的会话槽位 id（desktop-change-queries，自 PhaseRecord 三槽位列直读）在场时，转录面板 SHALL 按 session id 直查（`session_detail` + `agent_session_transcript`）；槽位缺席（缺省落账，槽位列 None）时 SHALL 回退既有 sourceRef 定式反查（`<change>/<phase>/<role>/<attempt>`，同 ref 多会话取最近一条）。eval 节点与 active 节点的抽屉联动 SHALL 均覆盖三会话：executor / evaluator / decision 三转录 tab（形态由 design 定稿）；decision 槽位缺席时该 tab SHALL 呈空态，MUST NOT 虚构会话或误挂他 attempt 的会话。active 节点 SHALL 经 sourceRef 反查联动（`active_phase` 无会话槽位，sessionId 恒 null；`ActivePhase.attempt` 非空保证定式可组装）：会话建档即落库、sealed 事件流式 append，进行中会话的已流出转录经反查重放可见；未开跑角色的 tab SHALL 呈「（暂无该会话转录）」空态；active 节点联动 MUST NOT 依赖实时事件流（实时面统一为转录库重查，实时流由 runtime WorkerAgent overlay 节点承担，本 app run 场景图上并存）。转录呈现 SHALL 复用既有会话基建（`AgentTimeline` / 会话重放，desktop-agent-chat-infra），MUST NOT 为 change 场景另建第二套时间线组件。节点 MUST NOT 提供显式「查看会话」按钮（两列布局后点击节点即见左列转录，active 节点不新增）。ToolStep / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果（左列空态占位、右列展示步骤输出摘要；审计全量见 desktop-change-state-store 步骤审计查询，步节点史见 run_steps 读史）。

#### Scenario: 运行中节点实时转录经转录库

- **WHEN** executor 会话运行中点击其 WorkerAgent 节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），数据经转录库重查呈现（通知触发重查，非内存 liveEvents 累积），与 Agent 调试页同款组件

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

#### Scenario: 记录 id 直查优先

- **WHEN** 某 attempt 记录携带会话槽位 id，且同 sourceRef 定式下存在多条会话
- **THEN** 转录面板按槽位 id 精确直查该会话，MUST NOT 落入「同 ref 多会话取最近一条」的反查歧义

#### Scenario: 槽位缺席回退反查

- **WHEN** 打开槽位列 None 的 attempt 记录（缺省落账）
- **THEN** 转录面板回退 sourceRef 定式反查并照常重放，行为与直查路径一致可靠

#### Scenario: decision 第三 tab 与空态

- **WHEN** 点击某 eval 节点打开抽屉，该 attempt 的 decision 槽位在场（或缺席）两种形态分别驱动
- **THEN** 抽屉呈 executor / evaluator / decision 三转录 tab；decision 槽位在场时按 id 直查其转录，缺席时 decision tab 呈空态，executor / evaluator 两 tab 不受影响

#### Scenario: active 节点三会话反查联动

- **WHEN** 打开一个残留 active_phase 的 change（如 CLI 外部跑的 run、刷新重挂或 crash 残留），点击 active 节点
- **THEN** 抽屉左列呈 executor / evaluator / decision 三转录 tab，已开跑角色经 sourceRef 反查重放已落库转录（含进行中会话已流出部分），未开跑角色呈「（暂无该会话转录）」空态；全程不订阅实时事件流，亦不虚构未开跑角色的会话

#### Scenario: 查看会话按钮不呈现

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点上无「查看会话」按钮，点击节点本体即打开抽屉查看左列转录；无独立会话 route 引入

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/graph.ts` + 转换层 | 步节点派生统一 | runStepNodes 数据源 = 统一视图（库读史 ∪ 在飞 run）同一路径；键 `phase:attempt:step`；全史叠加 + 在飞活步；收口后常驻（回落分支删除）；纯函数直测保持 |
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts` + `use-change-flow-run.ts` | 两钩并一 | 详情取数承接统一视图（查询缓存 + 失效重取）；flow-run 缩为控制动作（start / stop / confirm / answer）+ 通知订阅；liveEvents / seq 去重 / applyRunUpdate 镜像解散 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 拼缝与回落拆除 | `useRunViewEffects` 终态 refresh 回落分支删除；`buildFlowGraph` 双源合成收单源；常驻渲染组装 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | reducer 收缩 | 状态机镜像 / liveEvents / seq 去重分支删除（文件收缩保留，非整删）；收缩后直测同步 |
| 会话转录实时面 | 转录库重查 | 复用 `use-session-transcript` 查询面与 `AgentTimeline`（desktop-agent-chat-infra）；通知触发重查；无第二套时间线组件 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型再生跟随 | runs / steps / 通知信封演进后再生成；tsc 全量类型检查拦截前端消费漂移 |
