# desktop-change-orchestration Specification (Delta)

## MODIFIED Requirements

### Requirement: 会话挂靠 provenance 与节点状态派生

Walker 发起的 executor / evaluator / decision 会话 SHALL 携带 provenance：`source="change"`、`source_ref=<id>/<phase>/<role>/<attempt>`（身份段为 change id——单一身份锚贯穿，name 可变不入寻址键；归档链两会话面见 desktop-change-archive「spec 同步 agent 与归档链语义自持」）。节点运行态 SHALL 由按 provenance 反查的会话集派生（`agentSessions` 按 source / sourceRef 过滤既有面）；会话转录 SHALL 可观测：运行中实时流、结束后重放一致。会话停止 SHALL 复用 StopRegistry 既有终止面，对已终态目标幂等忽略。

#### Scenario: 按归属反查会话集

- **WHEN** 某 change 的 implement#2 相位已发起 executor 与 evaluator 会话
- **THEN** 按 `source="change"` + `source_ref="<id>/implement/executor/2"` 等过滤查询可枚举对应会话及其运行态，派生出节点状态，无需任何额外注册表

#### Scenario: 转录可观测与停止

- **WHEN** executor 会话运行中用户请求停止
- **THEN** 该会话经 StopRegistry 收敛为 `stopped`，walker 收到终止终态并按停止收敛处理 run；对已终态会话的重复停止请求幂等忽略

### Requirement: 运行控制命令面

命令面 SHALL 提供 change run 的控制入口，命令体收拢为编排运行时公共 API 的薄包装（沿三件事纪律）：

- **发起**：按 change id 发起 run（id → 记录解析 worktree / name；磁盘与 git 面经 name 供给），`auto_next_phase` 参数（bool；false=默认停等节奏，true=自动确认模式）随发起定格该 run 的停等节奏并透传 walker；发起 SHALL 先解析 exec root（按 id 读取的 db 记录 worktree 字段 → worktree 路径，None → 主 root，见「run 双 root 组合与 exec root 解析」）；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），RunRecord 起始行（status=running）SHALL 随发起建立（desktop-change-state-store「run 运行史落库」；写落位——命令层装配 vs walker 起点直调——由 design 定稿）；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态；对非运行态目标幂等忽略，MUST NOT 报错或误改既有终态；
- **应答**：ask 中断态下回传用户应答（选项或自由文本），驱动 run 继续；
- **确认**：phase 间停等点的用户确认（继续 / 终止）。

**读路统一（推拉反转）**：`get_change_detail`（或其扩展）SHALL 一次返回「库读史 ∪ 内存在飞 run」统一视图——库面 runs / steps 读史（desktop-change-queries）并入在飞 run 的状态 / 停等与 ask 载荷 / RunEntry 步表，合并发生在读时（零每步写放大前提不变）；客户端 MUST NOT 双命令拼接（useChangeDetail + useChangeFlowRun 双真相源拼缝退场）。`RunUpdate` SHALL 降位为变更通知（无步 / 会话事件载荷或最小载荷，信封形状 design 定稿）：客户端收通知后自行重查统一视图，通知仅失效信号、查询结果为权威；高频通知（每步 / 每会话事件）SHALL 经客户端合并去抖或通知侧 coalesce 抑制重查风暴（策略 design 定稿）。stop / confirm / answer 控制面 SHALL 维持请求-应答形态不变（非推送）。

运行状态 Channel SHALL 与 agent 执行流同构（执行流通道例外，不属轮询取数）。发起前置校验 SHALL 覆盖：目标 change 已建档（db `ChangeRecord` 在案且 `workflow_type=requirement` 相位表在位）、无同 workspace 同 change 并行 run——并行冲突键 SHALL 为 `(workspace root, change id)` 复合（三维隔离叠加：workspace × change 身份；worktree 隔离解锁同 workspace 多 change 真并行，同名 change 各持 id 互不构成冲突）——无建档的 change（id 不可达，含存量 CLI change）SHALL 显式拒绝发起（不可运行）。

#### Scenario: 发起提前 resolve 与通知流

- **WHEN** 前端发起某 change 的 run
- **THEN** invoke 在 run 进入运行态后即 resolve（run 起始行已落），后续节点 / 相位状态变化经 Channel 以变更通知流出，客户端收通知重查统一视图；终态以通知收尾、客户端重取定局

#### Scenario: 通知触发重查统一视图

- **WHEN** run 运行期间某步状态变化通知到达
- **THEN** 客户端经单命令统一视图重查（库读史 ∪ 在飞 run 含步表），无第二命令拼接、无前端载荷累积

#### Scenario: 控制面请求-应答不变

- **WHEN** 用户对 waitingAsk 态应答、对 phase 间停等确认、对运行中 run 停止
- **THEN** 三控制命令沿既有请求-应答契约执行（提前 resolve / 幂等 / 回流驱动语义不变），不经通知通道

#### Scenario: 停止幂等

- **WHEN** 对已收口（completed / stopped / failed）的 run 调停止命令
- **THEN** 幂等忽略：状态不变、不报错、不产生新事件

#### Scenario: 发起前置校验与复合键并行

- **WHEN** 对无 db 建档的 change（id 不可达，含存量 CLI change）发起 run、或同 workspace 同一 change（同 id）已有运行中的 run 再次发起、或同一 workspace 的另一 change / 另一 workspace 的 change 运行中
- **THEN** 前两者发起命令显式 `Err`（呈现未建档或并行冲突原因），不进入运行态、不落任何账；第三者不受影响正常发起（复合键按 id 隔离，同名 change 各持 id 互不构成冲突）

#### Scenario: auto_next_phase 随发起透传

- **WHEN** 前端以 auto_next_phase=true 发起某 change 的 run，随后另一 change 以 auto_next_phase=false 发起
- **THEN** 两 run 各自按发起参数定格节奏运行：前者全程无 phase 间停等，后者照常停等确认；参数经命令面透传至 `RunRequest.auto_next_phase`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/control.rs` | run 控制注册表键换锚 | `runs: HashMap<(String, String), RunEntry>` 键 `(workspace root, change)` → `(workspace root, change id)`（12 处构造点随动）；`RunUpdate` / `RunNotice` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 零改动（通知零载荷） |
| `crates/core/orchestration/src/walker.rs` | run 载体与 provenance | `RunRequest` / `RunStartCommand` / `RunFinishCommand` / `ToolCommand` 载荷 change → change id；provenance 定式 `<id>/<phase>/<role>/<attempt>`；`SessionAnchors` 键 `(change id, run_id)` |
| `crates/core/orchestration/src/port.rs` + `steps.rs` + `run_history.rs` + `snapshot.rs` | 载荷 id 化 | 相位机步直调写面携 change id；`TestExecutionRunner` / `WorkflowSnapshotPort` 签名随动（`detail(root, id)` → 经 queries id 寻址） |
| `src/commands/change_flow/mod.rs` | 五命令 id 化 | start / stop / answer / confirm / watch 定位参数 == change id；`ChangeFlowSink` 键 `(root, id)`；前置校验（建档 / 互斥 / exec root 解析）按 id 读取记录 |
| `crates/core/workflow/src/write/phase_next.rs`（SessionAnchors） | 进程内锚点键 | `(change, run_id)` → `(change id, run_id)`；锚点语义（续走不重头）不变 |
