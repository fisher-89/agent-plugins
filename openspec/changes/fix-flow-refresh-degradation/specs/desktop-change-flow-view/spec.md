# desktop-change-flow-view Specification (Delta)

## ADDED Requirements

### Requirement: 通知重查失败降级与有界重试

「通知仅失效信号、查询结果权威」的取数模型（见「取数模型与管线合规不变」）维持不变；本 requirement 追加**失败轮的降级契约**：

**失败轮保 last-good（信号界限主条款）**：通知（或显式刷新）触发的统一视图重查失败轮（`get_change_detail` invoke reject——读 `Err`，见 desktop-change-queries「详情查询错误语义分流（None / Err）」）在 detail 在手时 SHALL 保 last-good：`detail` / `activeRun` / `artifacts` SHALL NOT 被清空或回退占位（无 nodes=0 空图、无加载占位塌面、无「未找到该 change」兜底页）；陈旧窗口内 SHALL 以**内联「刷新失败」提示**如实标示（呈现位置与 data-testid 挂钩由 design 定夺），MUST NOT 伪造新鲜态（无提示地呈现旧图如新）。保 last-good 期间以**最后一次成功查询**为权威：面板动作（停止 / phase 间确认 / ask 应答）SHALL 照常可用（动作为独立 invoke，不依赖前端态），运行控制入口状态 SHALL 与 last-good `activeRun` 一致。首载失败（detail 不在手）SHALL 维持既有整页错误兜底页；重查成功且返回 `None`（真「未找到」）SHALL 维持清面兜底页语义不变。

**有界重试**：失败轮 SHALL 自带重试——最多 3 次、退避 1s / 2s / 4s（常量单点）。重试为「自动补发的刷新意图」：任一时刻至多一个待生效的刷新意图，重试计时器与通知去抖 SHALL 共用同一条取消通道——任何 tick 变化（新通知到达、手动刷新、换 root / id、卸载）即作废挂起的重试并重置退避序列，新意图天然取代旧重试（不产生交错重复查询）；重试期间用户动作（停止 / 确认 / 应答）不受取消语义影响。重试穷尽后 SHALL 静默回落「靠下一条通知兜底」。终态竞态洞由内建重试封堵：walker 写序 `run_finished`（整包落库）→ `guard.finish`（发通知），`Finished` 为 run 最后一条通知——终态重查若失败且只靠通知恢复则永无下一条，故重试必须自带。

**陈旧窗口有界（时间界限辅条款）**：保 last-good 的陈旧窗口 SHALL 有界——≤ 3 次退避（1s / 2s / 4s）内自愈，或下一条通知到达即恢复，最终一致。

**订阅粘性**：失败轮保 last-good 期间 `activeRun` SHALL 维持在场值（不清）→ `activeRunPresent` 保持真值，`change_flow_watch` 订阅 MUST NOT 因单轮失败被 effect cleanup 拆除（订阅生命周期口径不变，粘性随保 last-good 自然达成；不做「mount 即订阅」等独立改造）。

**立法辨析**：desktop-change-queries「详情标示『运行中』SHALL 与实际在飞 run 一致 / MUST NOT 假象」条款强度不变——其禁杀的是**无界、静默**的假态（archive 后挂「运行中」的旧 bug）；本例外豁免的是**有界、带「刷新失败」内联提示、收敛保证**的过渡态，措辞上不稀释原条款。

#### Scenario: 失败轮保 last-good 与内联提示

- **WHEN** 运行中某通知触发的统一视图重查轮 invoke reject（store 读 `Err`）且 detail 在手（上一轮成功）
- **THEN** 图面 / 徽章 / 停止入口维持上一成功轮状态（nodes 不清零、无空图占位），页面呈现内联「刷新失败」提示（data-testid 可查）；MUST NOT 出现「未找到该 change」兜底页、MUST NOT 无提示呈现旧图如新

#### Scenario: 重试自愈吃回更新

- **WHEN** 失败轮后第 1 次退避重试（1s）成功，其间步迁移已落库
- **THEN** 内联提示消失、图面更新到最新查询结果（迁移间更新不被吃掉），`loading` 不闪烁整页占位

#### Scenario: 退避节奏与穷尽静默

- **WHEN** 连续失败轮注入（首败 + 3 次重试全败）
- **THEN** 自动补发的重试间隔依次为 1s / 2s / 4s 且至多 3 次；穷尽后无第 4 次自动查询（静默回落），内联提示保持，等待下一条通知或显式刷新兜底

#### Scenario: 新意图取代旧重试

- **WHEN** 重试计时器挂起期间新通知到达（或用户手动刷新、切换 root / id、卸载页面）
- **THEN** 挂起的旧重试作废、退避序列重置，以新意图触发的重查为准；全程不出现交错重复查询（invoke 计数可断言），卸载后零遗留计时器

#### Scenario: 终态重查失败自行收口

- **WHEN** `Finished` 通知（run 最后一条通知）触发的终态重查失败
- **THEN** 有界重试自行恢复终局（不依赖「下一条通知」——已无下一条）；恢复前 last-good 运行中态保持（有内联提示、有界、收敛）

#### Scenario: 首载失败兜底不变

- **WHEN** 首次加载 `get_change_detail` 即失败（detail 不在手）
- **THEN** 整页错误兜底页（既有语义）——保 last-good 仅适用于 detail 在手的刷新轮

#### Scenario: 真 None 清面兜底不变

- **WHEN** 重查成功但返回 `None`（真「未找到」——change 记录已不存在）
- **THEN** 清面呈现「未找到该 change」兜底页（既有语义）——`None` 与 `Err` 分流不混同

#### Scenario: 降级窗口动作可用

- **WHEN** 保 last-good 窗口内 run 处于 ask 停等（或 phase 间确认 / 运行中可停止）
- **THEN** 应答 / 确认 / 停止动作照常可发且生效（独立 invoke 不依赖前端降级态），控制入口状态与 last-good `activeRun` 一致、无状态与操作错配

#### Scenario: 订阅粘性不闪拆

- **WHEN** 失败轮保 last-good 期间（`activeRun` 维持在场值）
- **THEN** `activeRunPresent` 不翻 false，`change_flow_watch` 订阅不被 effect cleanup 拆除，下一通知照常抵达并触发重查（零重订、零丢通知）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 失败轮降级与重试单点 | `loadUnifiedView` 失败分流（reject 且 detail 在手 → 保 last-good + 内联失败态 + 退避重试调度；首载失败维持整页 error 兜底）；重试计时器与 tick / 通知去抖同生命周期（新意图取代旧重试、卸载清计时器）；退避常量（3 次 / 1s / 2s / 4s）单点；`None` 清面语义不变 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 内联提示与兜底分流 | 内联「刷新失败」提示（data-testid 挂钩）随 last-good 态呈现 / 成功轮消失；`error` 分支收敛为首载语义（detail 在手的刷新失败不再整页 `DetailFallback`）；首载错误 / 真 `None` 兜底页分支不变 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts`（零实现改动） | 订阅粘性验证 | `activeRunPresent` 派生源（统一查询 activeRun）不清 → 订阅不闪拆；仅补测试断言，订阅生命周期实现不动 |
