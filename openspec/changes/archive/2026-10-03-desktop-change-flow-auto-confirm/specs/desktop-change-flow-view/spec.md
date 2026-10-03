# desktop-change-flow-view Delta

## ADDED Requirements

### Requirement: 自动确认发起开关

change 详情页运行控制面板 SHALL 在发起操作区提供「自动确认」开关（默认关）：开启后发起 run 以 `autoConfirm=true` 调运行控制命令面（`useChangeFlowRun.start(autoConfirm)` 传参），关闭则 `false`。开关 SHALL 为发起参数（发起时定格）：运行期间 MUST NOT 可切换（禁用或隐藏的呈现形态由 design 定夺）；节奏变更 = 停止后重发（自 `active_phase` 续走）。auto 模式 run MUST NOT 呈现 phase 间确认卡片（无 `ConfirmWait` 事件到达，`run-state.ts` 零改动）；ask 中断卡片 SHALL 照常呈现与应答（ask 不自动应答）。开关旁 SHALL 提供一行说明文案轻提示（auto 模式将跳过 phase 间确认连续推进，文案措辞由 design 定夺），MUST NOT 引入阻断式确认弹窗。开关 SHALL 以 data-testid 提供测试挂钩，测试 MUST NOT 以样式类名查询。

#### Scenario: 开关默认关与发起传参

- **WHEN** 打开运行控制面板（无运行 run / 终局后），开关保持默认关并点击发起
- **THEN** `changeFlowStart` 携 `autoConfirm=false`，run 照常在相位落账后停等确认（确认卡片呈现）；开启开关后再发起则携 `autoConfirm=true`

#### Scenario: auto run 不出确认卡片

- **WHEN** auto 模式 run 相位落账 pass 推进下一相位
- **THEN** 页面无确认卡片、状态徽章不经 `waitingConfirm`，图与步流照常增量刷新；run 直至终态收口

#### Scenario: auto run 的 ask 卡片照常

- **WHEN** auto 模式 run 中决策 ask 到来
- **THEN** ask 中断卡片照常呈现（问题 + 选项 + 自由文本），用户应答后 run 继续；确认卡片不因 auto 模式对 ask 生效

#### Scenario: 运行期定格与收口回归

- **WHEN** auto 模式 run 运行中尝试切换开关
- **THEN** 开关不可切换（定格为发起值）；run 收口后控制入口回到发起区，可按任意节奏再次发起

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | 发起开关与轻提示 | 发起区「自动确认」开关默认关 + 一行说明文案；运行期定格；data-testid 挂钩；确认卡片 / ask 卡片呈现逻辑不变 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | start 传参 | `start(autoConfirm: boolean)` → `commands.changeFlowStart(channel, root, change, autoConfirm)`；订阅生命周期不变 |
| `packages/desktop/src/views/changes/flow/run-state.ts`（不改） | 零触点 | 无新事件词汇，纯 reducer 与 `ChangeFlowRunState` 模型不动 |
| `src/types/generated/bindings.ts`（重导，见 desktop-change-orchestration delta） | IPC 类型跟随 | `changeFlowStart` 签名 + `autoConfirm`；tsc 全量类型检查拦截消费处漂移 |
