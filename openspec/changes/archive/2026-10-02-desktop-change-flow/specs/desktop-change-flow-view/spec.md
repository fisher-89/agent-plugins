# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 取数模型与管线合规不变

视图改造 MUST NOT 改变取数契约：详情更新 SHALL 仅由显式 refresh（或选中 change 变化）触发，MUST NOT 引入 watch 订阅、定时轮询或事件订阅（live 刷图留作后续独立 change）。**例外（desktop-change-flow 升级）**：change run 运行期间，执行视图 SHALL 经运行状态 Channel 接收节点状态实时刷新（执行流通道例外，沿 agent 执行先例，不属轮询取数）；Channel 刷新 MUST NOT 触发 `get_change_detail` 全量重取（节点状态为增量并入），run 收口后 MUST NOT 保持订阅。desktop 前端 SHALL 维持全管线通过：`vp check --fix`（fmt / lint，含 `max-lines-per-function: 50`）、knip（无未用导出残留）、`vp test` 全绿；测试 SHALL 以 data-testid 为查询挂钩，MUST NOT 以样式类名查询。

#### Scenario: 运行外显式刷新仍是唯一更新途径

- **WHEN** 无运行中 change run 时，流程图呈现期间 change 的 workflow.json 被外部进程修改
- **THEN** 图不自动变化；点击「刷新详情」后经 `get_change_detail` 重取并重绘

#### Scenario: 运行中 Channel 增量刷新

- **WHEN** change run 运行期间某相位推进（节点状态变化）
- **THEN** 执行视图经运行状态 Channel 增量并入节点状态并重绘，不发起 `get_change_detail` 全量重取；run 收口后订阅释放，后续变更回到显式刷新

#### Scenario: 全管线通过

- **WHEN** 运行 `pnpm -C packages/desktop run client:check` 与 `pnpm -C packages/desktop run test`
- **THEN** fmt / lint / knip / 测试全部通过，`package.json` 与 knip.json 无新增豁免条目

## ADDED Requirements

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的节点 SHALL 呈现运行态（pulse 视觉，与既有 active 节点同款），ToolStep 节点（phase-start / static-check / phase-log / backtrack 相位机步）与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（类型可区分）。运行态呈现 SHALL 与既有三分类模型并存：eval（实心 pass 绿 / fail 红）、interrupted（dashed 灰显）、stale 淡化规则不变；`PIPELINE_PHASES` 列布局、时间序边推导规则与空列恒定 MUST NOT 因运行态呈现改变。运行收口后，图 SHALL 回落到既有派生规则（重新经 `ChangeDetail` 派生），运行态视觉完全褪去。

#### Scenario: 运行态与三分类并存

- **WHEN** implement#2 相位运行中（executor 会话活跃）且历史上有 fail→retry 与 interrupted 留档
- **THEN** 图上同时呈现：历史 eval 节点（实心三色）、interrupted 节点（dashed）、当前 executor 节点（pulse 运行态）与同相位的 ToolStep / Gate 节点（类型可辨、状态如实）

#### Scenario: 收口后回落派生规则

- **WHEN** run 以全部相位 pass 收口
- **THEN** 图经 `get_change_detail` 重新派生，与插件 CLI 直跑产出的执行史图形态一致，无残留运行态视觉

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线（delta 流、可停止）；点击已收口的 WorkerAgent 节点 SHALL 打开该会话的转录重放（不要求运行进程存活）。转录呈现 SHALL 复用既有会话基建（`AgentTimeline` / 会话重放，desktop-agent-chat-infra），MUST NOT 为 change 场景另建第二套时间线组件。ToolStep / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果（无会话可联动时抽屉展示步骤输出摘要）。

#### Scenario: 运行中节点实时转录

- **WHEN** executor 会话运行中点击其节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），与 Agent 调试页同款组件

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

### Requirement: 运行控制入口与 ask 应答

change 详情页 SHALL 提供运行控制入口：发起 run（按 change 名）、停止运行中的 run、phase 间停等点的用户确认（继续 / 终止）。决策协议 `ask` 中断 SHALL 在页面呈现中断卡片（问题 + 选项），用户应答（选项或自由文本）后经应答命令回流驱动 run 继续；停止与发起 SHALL 沿运行控制命令面契约（提前 resolve、停止幂等）。控制入口状态 SHALL 如实反映 run 生命周期（可发起 / 运行中可停止 / ask 等待应答 / phase 间等待确认），MUST NOT 出现状态与可用操作错配（如已收口仍显停止入口为可用主操作）。

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
| `packages/desktop/src/views/changes/flow/graph.ts` + `types.ts` | 转换层扩展 | 节点模型增运行态与节点类型可辨维度；时间序边推导规则、`PIPELINE_PHASES` 列布局、attempt 缺号兜底不变；纯函数直测保持绿 |
| `packages/desktop/src/views/changes/flow/` 节点 / 列组件 | 运行态渲染 | WorkerAgent / ToolStep / Gate 类型可辨；pulse 运行态与既有 active 视觉同款；收口后回落派生规则 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 控制入口组装 | 发起 / 停止 / phase 间确认 / ask 应答卡片；生命周期状态与可用操作对齐 |
| 会话转录联动 | 节点 ↔ 转录 | 复用 `AgentTimeline` / 会话重放基建（desktop-agent-chat-infra），无第二套时间线组件；运行中实时流、收口重放一致 |
| 运行状态 Channel 订阅 | 实时刷新例外 | 执行流通道例外（沿 agent 执行先例）；增量并入不触发 `get_change_detail` 全量重取；收口释放订阅 |
| `src/commands/` change-flow 命令组（desktop-change-orchestration） | IPC 面 | 发起 / 停止 / 应答 / 确认契约见 desktop-change-orchestration「运行控制命令面」 |
