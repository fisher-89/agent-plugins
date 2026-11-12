# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 运行控制入口与 ask 应答

change 详情页 SHALL 提供运行控制入口：发起 run（按 change id——执行锚与磁盘 / git 面经 id → 记录解析，见 desktop-change-orchestration）、停止运行中的 run、phase 间停等点的用户确认（继续 / 终止）。ask 中断 SHALL 由**阻塞工具 `AskUserQuestion`** 产生（不再是决策动作；见 desktop-change-orchestration「流程内 ask 停等与双入口应答」），并在页面呈现为**两处同源卡片**：① 右栏运行控制面板的 `AskCard`（问题 + 选项按钮 + 自由文本输入）；② 聊天窗 / 会话转录面板里 `AskUserQuestion` 工具调用的**活卡**（同一问题与选项，同样可应答）。两处 SHALL 读同一 `AskPayload`（`question` / `options` / `toolCallId`），用户在任一处应答（选项或自由文本）后经应答命令回流驱动 run 继续，**两处同步收起、`activeRun.status` 回 `running`**，MUST NOT 出现「一处已收起另一处仍挂着」或「再点报无等待」的错配。停止与发起 SHALL 沿运行控制命令面契约（提前 resolve、停止幂等）。控制入口状态 SHALL 如实反映 run 生命周期（可发起 / 运行中可停止 / ask 等待应答 / phase 间等待确认），MUST NOT 出现状态与可用操作错配（如已收口仍显停止入口为可用主操作）。

活卡的判据 SHALL 为「`AskPayload.toolCallId` 与该工具部件的 `toolCallId` 相等且该部件仍为无结果态」——MUST NOT 以「工具结果缺席即认为在等」推断（历史转录里崩溃残留的未配对 `tool_use` 同一形态，无 id 对账会误呈为可应答）。

#### Scenario: phase 间确认续走

- **WHEN** walker 完成一相位推进进入 phase 间停等点
- **THEN** 页面呈现等待确认状态；用户确认后 run 继续下一相位，选择终止则 run 收敛为受控终态

#### Scenario: ask 中断卡片应答

- **WHEN** 流程会话调用 `AskUserQuestion`（问题 + 选项）后用户在详情页 `AskCard` 中选择某选项
- **THEN** 应答经命令回流（`change_flow_answer`），run 按应答在同一 turn 内继续，中断卡片消失、控制入口回到运行态

#### Scenario: 两处同源呈现与同步收起

- **WHEN** 某流程会话的 `AskUserQuestion` 正在等待应答，用户打开该步节点的会话转录
- **THEN** 详情页 `AskCard` 与转录面板里的 ask 活卡呈现同一问题与同一组选项；用户在任一处应答后两处同刻收起（`activeRun.ask` 清空、活卡转静态卡，工具结果回灌为应答文本）、`activeRun.status` 回 `running`

#### Scenario: 非当前等待的 ask 部件为静态卡

- **WHEN** 打开一条历史转录，其中某 `AskUserQuestion` 工具的 `toolCallId` 与 `activeRun.ask.toolCallId` 不相等（或 `activeRun.ask` 为 null）
- **THEN** 该部件呈静态只读卡（问题与选项原样可读、零应答入口），MUST NOT 呈现为可点的活卡

#### Scenario: 确认应答后控制入口回运行态

- **WHEN** 用户在 phase 间停等点（含 archive 前停等点）确认「继续」或「终止」
- **THEN** 确认卡片消失、控制入口不再显示等待确认态；同一 `entry.status` / 停等载荷槽的解除对两类停等一致——已确认的卡片 MUST NOT 因后续相位推进而继续挂着，也 MUST NOT 再点报「无等待中的 phase 确认」

#### Scenario: 状态与操作不错配

- **WHEN** run 已 fail 收口（或被用户停止）
- **THEN** 页面控制入口回到「可发起」态，停止入口不再作为可用主操作呈现

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Component / Function） |
|------|------|------|
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx`（零改动） | 详情页 ask 卡片 | `AskCard` 继续读 `activeRun.ask`（`question` / `options`），应答走 `actions.answer` → `ipc.changeFlowAnswer`；`AskPayload` 两新字段不参与其渲染（additive） |
| `packages/desktop/src/views/changes/change-detail-view.tsx`（扩） | 流程侧应答动作面组装 | 构造 `askAnswer = { onAnswer: (text) => ipc.changeFlowAnswer(root, changeId, text), pendingToolCallId: activeRun?.ask?.toolCallId ?? null }` 并下行至 `DetailDrawer` |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx`（扩） | 抽屉透传 | 增可选 prop `askAnswer`，原样透传给 `SessionTranscriptPanel`（未传即零行为变化） |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`（扩） | 只读面板的 ask 例外 | 增可选 prop `askAnswer`；只在场时向下传给 `AgentSessionView`（该面板其余「不接动作面」纪律不变——它是 **ask 应答** 单点例外，不引入发送 / 停止入口） |
