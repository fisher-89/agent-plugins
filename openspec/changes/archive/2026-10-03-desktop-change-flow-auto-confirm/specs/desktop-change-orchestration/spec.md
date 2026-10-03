# desktop-change-orchestration Delta

## ADDED Requirements

### Requirement: 自动确认运行模式（auto_confirm）

Desktop run SHALL 支持「自动确认」运行模式：`RunRequest.auto_confirm`（bool）为 run 级属性、发起时定格，经运行控制命令面发起入口透传。`auto_confirm=true` 时，walker SHALL 在 phase 间确认点跳过 `ConfirmWait` 广播与 `wait_confirm` 挂起，直接推进下一相位；auto 模式 run MUST NOT 进入 `WaitingConfirm` 状态、MUST NOT 产生 `ConfirmWait` 事件（`RunUpdate` / `ChangeRunSnapshot` DTO 零改动，重挂恢复路径天然兼容）。`auto_confirm=false`（默认）时既有停等语义不变。

语义边界：

- **ask 中断不自动应答**：auto 模式下决策 agent 的 ask 到来 SHALL 照常进入 `WaitingAsk` 停等用户应答——ask 是重试预算耗尽后的真实提问，需要人的判断；自动确认只作用于 phase 间停等。
- **终态不自动归档**：全相位 pass SHALL 照常以 `completed` 收口（"Ready for archiving"）停给用户，MUST NOT 自动触发归档。

运行中停等节奏 MUST NOT 可切换：节奏变更 SHALL 经停止后重发（自 `active_phase` 续走）；auto 模式下用户干预的合法动作与既有运行面一致（停止）。

#### Scenario: auto run 直通收口

- **WHEN** 以 auto_confirm=true 发起多相位 run（各相位 evaluator 全 pass）
- **THEN** 更新流零 `ConfirmWait` 信封，逐相位自动推进直至 `completed`（reason = "All phases have passed evaluation. Ready for archiving."），快照面全程不经 `waitingConfirm`，归档未自动触发

#### Scenario: 手动模式语义不变

- **WHEN** 以 auto_confirm=false（默认）发起同一 run
- **THEN** `ConfirmWait` 逐相位流出、挂起至 `change_flow_confirm` 应答，proceed=false / 等待期间取消 → 受控 `stopped`（与既有契约逐字一致）

#### Scenario: auto run 的 ask 仍停等人

- **WHEN** auto 模式 run 中决策 agent 输出 ask 动作（重试预算耗尽后的提问）
- **THEN** run 照常进入 `WaitingAsk` 并呈现问题与选项，walker 挂起等应答；MUST NOT 自动选择任何选项，用户应答后 run 继续

#### Scenario: 发起定格与停止重发

- **WHEN** 运行中的 run 需要变更停等节奏
- **THEN** 唯一路径为停止当前 run 后以目标节奏重新发起（自 `active_phase` 续走，已 pass 相位不重头执行）；运行中不存在节奏切换命令或事件

## MODIFIED Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行硬编码的单图相位循环，每相位依序：

1. core/workflow 写面 `phase_next`（进程内）→ next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单；
2. 写面 `phase_start`（开相位、attempt 计时）；
3. spawn executor agent（compose_turn 新会话）；
4. implement / test-gen 相位后执行 static-check（infra spawn 步）；
5. spawn evaluator agent（新会话，不落账）；
6. 解析 verdict → 桌面代调写面 `phase_log`；
7. 写面 `phase_next` → pass 推进 / fail 重试（≤5，与插件 `MAX_RETRY_TIMES` 一致）/ 重试上限唤决策 agent 分叉。

红线：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`（路由权威与 walker 同进程仍不自持规则）；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份，MUST NOT 上位为路由权威。推进节奏 SHALL 为 phase 内自动、phase 间停等用户确认后继续（`auto_confirm` 发起参数开启时 phase 间停等跳过，语义见「自动确认运行模式」）；重启 run SHALL 自 `active_phase` 续走（已 pass 相位 MUST NOT 重头执行）。

#### Scenario: 相位内全循环自动走完

- **WHEN** 以假引擎（预录 AgentEvent 序列）+ 假写面（fake port，预录 phase-next / phase-start / phase-log 响应）驱动某相位 run
- **THEN** walker 依序执行 ①→⑦：executor 会话发起、evaluator 会话发起、`phase_log` 以解析 verdict 代调，pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: fail 重试与预算上限

- **WHEN** evaluator verdict 为 fail 且重试未超预算
- **THEN** walker 不唤决策 agent 自走重试同相位；连续 fail 达 5 次后停止自走并按决策协议分叉

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 active_phase=implement 的下一 attempt 续走，已 pass 的 proposal / dev-design 等相位不重新执行；中断相位经进程内锚点（sessionAnchors 复活）可感知并被标定

### Requirement: 运行控制命令面

命令面 SHALL 提供 change run 的控制入口，命令体收拢为编排运行时公共 API 的薄包装（沿三件事纪律）：

- **发起**：按 change 名发起 run，`auto_confirm` 参数（bool；false=默认停等节奏，true=自动确认模式）随发起定格该 run 的停等节奏并透传 walker；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），执行经运行状态 Channel 以同构状态部件流出；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态；对非运行态目标幂等忽略，MUST NOT 报错或误改既有终态；
- **应答**：ask 中断态下回传用户应答（选项或自由文本），驱动 run 继续；
- **确认**：phase 间停等点的用户确认（继续 / 终止）。

运行状态 Channel SHALL 与 agent 执行流同构（执行流通道例外，不属轮询取数）。发起前置校验 SHALL 覆盖：目标 change 存在且 workflow.json 可解析、无同 change 并行 run（CLI 可发现校验随子进程写通道消失）。

#### Scenario: 发起提前 resolve 与状态流

- **WHEN** 前端发起某 change 的 run
- **THEN** invoke 在 run 进入运行态后即 resolve，节点 / 相位状态变化随后经 Channel 持续流出，终态以同构部件收尾

#### Scenario: 停止幂等

- **WHEN** 对已收口（completed / stopped / failed）的 run 调停止命令
- **THEN** 幂等忽略：状态不变、不报错、不产生新事件

#### Scenario: 发起前置校验

- **WHEN** 对 workflow.json 损坏（unparsable）的 change 发起 run，或该 change 已有运行中的 run
- **THEN** 发起命令显式 `Err`（呈现损坏警示或并行冲突），不进入运行态

#### Scenario: auto_confirm 随发起透传

- **WHEN** 前端以 auto_confirm=true 发起某 change 的 run，随后另一 change 以 auto_confirm=false 发起
- **THEN** 两 run 各自按发起参数定格节奏运行：前者全程无 phase 间停等，后者照常停等确认；参数经命令面透传至 `RunRequest.auto_confirm`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs` | 停等节奏分支 | `RunRequest` + `auto_confirm` 字段（run 级、发起定格）；drive() 确认点 `if !auto_confirm` 才 emit `ConfirmWait` + `wait_confirm`；auto 模式不进 `WaitingConfirm`、零新事件；头注释口径同步 |
| `src/commands/change_flow/mod.rs` | 发起参数透传 | `change_flow_start` / `change_flow_start_with` 签名 + `auto_confirm: bool`，透传 `RunRequest`；`Result<T, String>` 模板、前置校验、提前 resolve 不变 |
| `src/types/generated/bindings.ts`（重导） | IPC 类型跟随 | `changeFlowStart` + `autoConfirm` 参数；`bindings:check` 守卫拦截过期生成物 |
| `crates/core/orchestration/src/control.rs` / `state.rs`（不改） | 零触点红线 | confirm 应答通道 / `RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 不动 |
