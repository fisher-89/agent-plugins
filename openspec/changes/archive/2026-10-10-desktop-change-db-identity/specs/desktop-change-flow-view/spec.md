# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 三代际降级

视图 SHALL 按建档状态单态呈现（文档形态分支随清单 db 单源整体退役；inventory 代际词汇保持退役）：

- **已建档**（db `ChangeRecord` 在案——详情按 id 寻址，命中即恒有记录）：SHALL 呈现完整状态面——流程图（eval / active 节点照常）、抽屉与产物区照常；
- **未找到**（db 无该 id 记录）：error / loading / 未找到降级页 SHALL 沿用现状；MUST NOT 呈现文档形态空图占位（`flow-empty`）或其文案。

`detail.status` MUST NOT 再承担建档判别（详情 DTO 恒有记录，`status` 仅表达 active / archived）——「建档两态分流」语义随文档形态退役退场；`flow-empty` 占位分支、「（文档形态：未建档，仅产物清单）」文案与其测试断言 SHALL 一并删除。inventory 代际徽章（V0 / V1 / V2）与 `unparsable`（workflow.json 损坏）警示 SHALL 保持退役：清单与详情的前端词汇清零（组件、DTO 消费与测试断言一并移除）。

#### Scenario: 建档 change 完整状态面

- **WHEN** 打开一个 db 已建档、含多 attempt 与 backtrack 的 change 详情
- **THEN** 流程图正常绘制 eval / active 节点，无代际徽章出现，抽屉与产物区照常

#### Scenario: 归档 change 不落空面

- **WHEN** 打开归档 change 详情（磁盘目录名带日期前缀，经 id 寻址）
- **THEN** 流程图（含 runs 步节点）与产物区照常呈现，MUST NOT 出现「文档形态」空图占位或任何建档判别分支

#### Scenario: 未找到 id 降级

- **WHEN** 打开 db 无记录的 id（含磁盘存在同名目录的情形）
- **THEN** 呈现未找到降级页，无空图占位、无「文档形态」文案，页面不报错

#### Scenario: 代际与警示词汇清零

- **WHEN** 扫描清单 / 详情视图源码与测试对 `inventory` / `unparsable` / 「无法解析」/ `flow-empty` / 「文档形态」的消费
- **THEN** 零消费残留；knip 无新增豁免，全量类型检查绿

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线（delta 流、可停止）——实时面数据源 SHALL 为**转录库重查**（变更通知触发、`use-session-transcript` 既有查询面消费；会话事件流式落库），前端 liveEvents 内存缓存随通知降位退役，实时与重放口径统一到转录库；点击已收口的 WorkerAgent 节点 SHALL 打开该会话的转录重放（不要求运行进程存活）。会话寻址 SHALL 记录 id 直查优先：attempt 记录暴露的会话槽位 id（desktop-change-queries，自 PhaseRecord 三槽位列直读）在场时，转录面板 SHALL 按 session id 直查（`session_detail` + `agent_session_transcript`）；槽位缺席（缺省落账，槽位列 None）时 SHALL 回退既有 sourceRef 定式反查（`<id>/<phase>/<role>/<attempt>`——身份段为 change id，见 desktop-change-orchestration「会话挂靠 provenance 与节点状态派生」；同 ref 多会话取最近一条）。eval 节点与 active 节点的抽屉联动 SHALL 均覆盖三会话：executor / evaluator / decision 三转录 tab（形态由 design 定稿）；decision 槽位缺席时该 tab SHALL 呈空态，MUST NOT 虚构会话或误挂他 attempt 的会话。active 节点 SHALL 经 sourceRef 反查联动（`active_phase` 无会话槽位，sessionId 恒 null；`ActivePhase.attempt` 非空保证定式可组装，身份段自详情 id 取）：会话建档即落库、sealed 事件流式 append，进行中会话的已流出转录经反查重放可见；未开跑角色的 tab SHALL 呈「（暂无该会话转录）」空态；active 节点联动 MUST NOT 依赖实时事件流（实时面统一为转录库重查，实时流由 runtime WorkerAgent overlay 节点承担，本 app run 场景图上并存）。转录呈现 SHALL 复用既有会话基建（`AgentTimeline` / 会话重放，desktop-agent-chat-infra），MUST NOT 为 change 场景另建第二套时间线组件。节点 MUST NOT 提供显式「查看会话」按钮（两列布局后点击节点即见左列转录，active 节点不新增）。ToolStep / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果（左列空态占位、右列展示步骤输出摘要；审计全量见 desktop-change-state-store 步骤审计查询，步节点史见 run_steps 读史）。

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
- **THEN** 转录面板回退 sourceRef 定式反查（身份段 = change id）并照常重放，行为与直查路径一致可靠

#### Scenario: decision 第三 tab 与空态

- **WHEN** 点击某 eval 节点打开抽屉，该 attempt 的 decision 槽位在场（或缺席）两种形态分别驱动
- **THEN** 抽屉呈 executor / evaluator / decision 三转录 tab；decision 槽位在场时按 id 直查其转录，缺席时 decision tab 呈空态，executor / evaluator 两 tab 不受影响

#### Scenario: active 节点三会话反查联动

- **WHEN** 打开一个残留 active_phase 的 change（如 CLI 外部跑的 run、刷新重挂或 crash 残留），点击 active 节点
- **THEN** 抽屉左列呈 executor / evaluator / decision 三转录 tab，已开跑角色经 sourceRef 反查（身份段 = change id）重放已落库转录（含进行中会话已流出部分），未开跑角色呈「（暂无该会话转录）」空态；全程不订阅实时事件流，亦不虚构未开跑角色的会话

#### Scenario: 查看会话按钮不呈现

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点上无「查看会话」按钮，点击节点本体即打开抽屉查看左列转录；无独立会话 route 引入

### Requirement: 运行控制入口与 ask 应答

change 详情页 SHALL 提供运行控制入口：发起 run（按 change id——执行锚与磁盘 / git 面经 id → 记录解析，见 desktop-change-orchestration）、停止运行中的 run、phase 间停等点的用户确认（继续 / 终止）。决策协议 `ask` 中断 SHALL 在页面呈现中断卡片（问题 + 选项），用户应答（选项或自由文本）后经应答命令回流驱动 run 继续；停止与发起 SHALL 沿运行控制命令面契约（提前 resolve、停止幂等）。控制入口状态 SHALL 如实反映 run 生命周期（可发起 / 运行中可停止 / ask 等待应答 / phase 间等待确认），MUST NOT 出现状态与可用操作错配（如已收口仍显停止入口为可用主操作）。

#### Scenario: phase 间确认续走

- **WHEN** walker 完成一相位推进进入 phase 间停等点
- **THEN** 页面呈现等待确认状态；用户确认后 run 继续下一相位，选择终止则 run 收敛为受控终态

#### Scenario: ask 中断卡片应答

- **WHEN** 决策 agent 输出 ask 动作（问题 + 选项）后用户在卡片中选择某选项
- **THEN** 应答经命令回流 walker，run 按应答继续，中断卡片消失、控制入口回到运行态

#### Scenario: 状态与操作不错配

- **WHEN** run 已 fail 收口（或被用户停止）
- **THEN** 页面控制入口回到「可发起」态，停止入口不再作为可用主操作呈现

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/change-detail-view.tsx` | id 寻址与单态呈现 | `useParams<'id'>`；`flow-empty` 文档形态分支删除；页内身份源统一（URL id），无 `detail.name` 作寻址残留；归档 change 照常呈图 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | sourceRef 定式组装 | 身份段改 change id（`<id>/<phase>/<role>/<attempt>` 与 active 三 role 反查同式）；会话槽位 id 直查优先不变 |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 转录反查 | `agent_sessions(root, 'change', sourceRef)` 的 sourceRef 身份段 = change id；seq 归并逻辑不变 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` + `flow/run-control-panel.tsx` | 运行控制面 id 透传 | 发起 / 停止 / 确认 / 应答命令携 change id；面板展示面（名称文案）不变 |
| `packages/desktop/src/views/changes/flow/`（graph.ts / attachments.ts / run-state.ts） | 零触点 | 转换层纯函数、挂载规则、reducer 不涉 change 身份；`PIPELINE_PHASES` 布局与边推导不变 |
