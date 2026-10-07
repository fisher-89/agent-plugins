# desktop-change-orchestration Specification (Delta)

## ADDED Requirements

### Requirement: run 双 root 组合与 exec root 解析

change run SHALL 以双 root 组合执行（能力语义见 desktop-change-worktree）：`change_flow_start` SHALL 解析 exec root——db 记录带 `worktree` → 该 worktree 绝对路径；记录 `worktree=None`（legacy）→ 主 workspace root——`RunRequest.root` SHALL 恒为 exec root，会话 cwd（`turn.root → SessionCtx.workspace_root`）、工件根（`Layout::resolve`）、git diff 与检查器（static-check / test-execution）执行目录 SHALL 全部随 exec root 落位。store / db 半边 SHALL 恒以 workspace root 解析：`compose_turn` 的 store 半边为命令层预解析注入的 workspace root 库实例，worktree 路径 MUST NOT 进入 `for_root`；`StoreSnapshot` 的 fs 半边（产物发现 / detail 装配）取 exec root、db 半边取注入的 store 实例（既有构造形态天然满足）。相位状态机零 fs、会话 resume 链（记录不存 cwd）MUST 保持零改动。SDK 路径沙箱前缀校验以 exec root 为锚（既有机制零改动），worktree change 的文件编辑囚于 worktree 内；worktree 内 `git diff HEAD` 恰为本 change 编辑集（「变更文件上下文降级 git diff」requirement 的 diff 面语义在 worktree 内自然收窄，无模板改动）。

#### Scenario: worktree run 全链落 worktree

- **WHEN** 以带 worktree 记录的 change 发起 run（假引擎驱动一相位）
- **THEN** executor / evaluator 会话 cwd = worktree、static-check 与 test-execution spawn 的执行目录 = worktree、git diff 上下文取自 worktree 且仅含本 change 编辑集

#### Scenario: store 身份恒 workspace root

- **WHEN** 审查 worktree change run 的组合根装配与 compose_turn 调用
- **THEN** store 实例由命令层以 workspace root `for_root` 解析后注入（provider / 会话 / 建档 / StepRecord 落 workspace 库），全程无以 worktree 路径进 `for_root` 的调用点

#### Scenario: legacy run 零变化

- **WHEN** 以 `worktree=None` 的存量建档 change 发起 run
- **THEN** exec root 解析为主 workspace root，既有 run 全链（cwd / Layout / diff / 检查器 / 快照）语义零变化

#### Scenario: 并行 change 各自 worktree

- **WHEN** 同 workspace 两个带各自 worktree 的 change 同时发起 run（假引擎）
- **THEN** 两 run 互不干扰（各自 exec root 的编辑 / diff / 检查隔离），db 写入经既有进程内并发支撑

## MODIFIED Requirements

### Requirement: 运行控制命令面

命令面 SHALL 提供 change run 的控制入口，命令体收拢为编排运行时公共 API 的薄包装（沿三件事纪律）：

- **发起**：按 change 名发起 run，`auto_next_phase` 参数（bool；false=默认停等节奏，true=自动确认模式）随发起定格该 run 的停等节奏并透传 walker；发起 SHALL 先解析 exec root（db 记录 worktree 字段 → worktree 路径，None → 主 root，见「run 双 root 组合与 exec root 解析」）；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），执行经运行状态 Channel 以同构状态部件流出；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态；对非运行态目标幂等忽略，MUST NOT 报错或误改既有终态；
- **应答**：ask 中断态下回传用户应答（选项或自由文本），驱动 run 继续；
- **确认**：phase 间停等点的用户确认（继续 / 终止）。

运行状态 Channel SHALL 与 agent 执行流同构（执行流通道例外，不属轮询取数）。发起前置校验 SHALL 覆盖：目标 change 已建档（db `ChangeRecord` 在案且 `workflow_type=requirement` 相位表在位）、无同 workspace 同 change 并行 run——并行冲突键 SHALL 为 `(workspace root, change)` 复合（修正既有仅按 change 名做键的两 workspace 同名假冲突先例 bug；worktree 隔离解锁同 workspace 多 change 真并行）——无建档的 change（存量 CLI change）SHALL 显式拒绝发起（文档形态 change 不可运行）。

#### Scenario: 发起提前 resolve 与状态流

- **WHEN** 前端发起某 change 的 run
- **THEN** invoke 在 run 进入运行态后即 resolve，节点 / 相位状态变化随后经 Channel 持续流出，终态以同构部件收尾

#### Scenario: 停止幂等

- **WHEN** 对已收口（completed / stopped / failed）的 run 调停止命令
- **THEN** 幂等忽略：状态不变、不报错、不产生新事件

#### Scenario: 发起前置校验与复合键并行

- **WHEN** 对无 db 建档的 change（存量 CLI change）发起 run、或同 workspace 同 change 已有运行中的 run 再次发起、或另一 workspace 存在同名 change 的运行中 run
- **THEN** 前两者发起命令显式 `Err`（呈现未建档或并行冲突原因），不进入运行态、不落任何账；第三者不受影响正常发起（复合键下异 workspace 同名不构成冲突）

#### Scenario: auto_next_phase 随发起透传

- **WHEN** 前端以 auto_next_phase=true 发起某 change 的 run，随后另一 change 以 auto_next_phase=false 发起
- **THEN** 两 run 各自按发起参数定格节奏运行：前者全程无 phase 间停等，后者照常停等确认；参数经命令面透传至 `RunRequest.auto_next_phase`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs` + `RunRequest` | 双 root run 载体 | `RunRequest.root` 恒 exec root（worktree / legacy 主 root）；fs 半边（Layout / diff / 检查 cwd / 快照产物发现）随 root 落位；store 经 port 注入不变 |
| `crates/core/orchestration/src/control.rs` | run 控制注册表 | `begin_run` / `subscribe` 键改 `(workspace root, change)` 复合；`RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 零改动 |
| `src/commands/change_flow/mod.rs` | 发起装配 | 前置校验后解析 exec root（db worktree 字段 → 路径，None → 主 root）；store 实例以 workspace root 解析后注入 compose（拆参形态见 desktop-agent-execution / design）；其余三件事纪律不变 |
