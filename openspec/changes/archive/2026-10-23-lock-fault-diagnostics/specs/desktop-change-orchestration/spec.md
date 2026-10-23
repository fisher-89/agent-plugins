# desktop-change-orchestration Specification（delta: lock-fault-diagnostics）

## MODIFIED Requirements

### Requirement: 停止贯通到进程执行（cancel 传播）

run 的停止信号 SHALL 由「相位 / 步边界观测」贯通到**运行中的外部进程**：

- **注册表侧**：`RunEntry` SHALL 持取消标志（`foundation::cancel::CancelFlag`，与既有取消 `watch` 同道），`begin_run` 时铸造；`request_stop()` SHALL **双写**——既有 `watch::send_replace(true)` 保留（停等 / 应答等待的异步观测面不变）**且** `cancel_flag.set()`（无订阅者也不丢位）；重复停止幂等。
- **控制柄侧**：`RunGuard` SHALL 暴露 `cancel_flag() -> CancelFlag`（克隆共享同一原子，供同步读取）。
- **请求侧**：`ToolStepRequest` SHALL 增 `cancel: CancelFlag` 字段；三个检查域 runner port 签名 SHALL 各加一参——`StaticCheckRunner::run(root, name, slot, cancel)`、`TestExecutionRunner::run(root, name, slot, cancel)`、`MutationRunner::run(root, name, files, patch, cancel)`。
- **透传侧**：`LocalToolSteps` SHALL 把 `step.cancel` 透传给三 runner（`steps.rs`）；walker 的 `run_tool` SHALL 在**单点**以 `guard.cancel_flag()` 填充该字段（MUST NOT 逐调用点各写一遍取值逻辑）。
- **终态语义（本变更改判，取代过渡形态）**：取消使 runner 以 `fault = Some(RunFault::Stopped)` 正常返回产出（MUST NOT 返回 `Err("执行已取消")`）；walker 按「执行面归因 RunFault 与门禁步分派」把该步节点收敛 `stopped`、run 收敛 `Stopped`——MUST NOT 收敛为 failed。此前「run failed 的过渡形态」表述 SHALL 视为已被本要求取代，MUST NOT 在 spec / 代码注释中留第二套口径。

#### Scenario: 停止运行中的检查步秒级收敛

- **WHEN** user 对正在执行 static-check / test-execution / mutation 步的 run 发停止命令
- **THEN** 取消标志置位 → 执行体终止进程树（不残留 rustc 类子孙）→ runner 返回 `fault = Some(Stopped)` → 该步 `stopped`、run 在秒级收敛 `stopped`（MUST NOT 等命令自然结束，如等 340–393 s 的 suite 跑完）

#### Scenario: 双写幂等且无订阅者不丢

- **WHEN** 对同一运行中 run 连续两次调停止命令，或该 run 当前无任何 watch 订阅者
- **THEN** 两次调用均不报错（幂等），且第二次后 `RunEntry` 的取消标志仍为已置位（`CancelFlag::is_set()` 为真）——置位不因无订阅者丢失

#### Scenario: 停止终态为 stopped

- **WHEN** 停止导致检查步取消（`fault = Stopped`）
- **THEN** 该步节点 status 为 `stopped`、run 终态为 `stopped`（非 `failed`）；图上不呈现「执行已取消」失败文案，MUST NOT 有 failed 节点残留

#### Scenario: 缺省取消信号零行为变化

- **WHEN** 既有测试 / 非取消路径以 `CancelFlag::never()` 构造 `ToolStepRequest` 与 runner 调用
- **THEN** 行为与本变更前逐字等价（执行体走正常完成臂、`fault` 与取消无关），既有 walker / steps / port 测试除新增 `fault` 字段字面量外零改动

#### Scenario: 透传单点

- **WHEN** 审查 walker 中 `ToolStepRequest` 的构造点
- **THEN** `cancel` 仅在 `run_tool` 一处从 `guard.cancel_flag()` 取值填充，各步函数不各自取值；`steps.rs` 仅做字段透传不做决策

## ADDED Requirements

### Requirement: 执行面归因 RunFault 与门禁步分派

walker SHALL 把「执行面归因（检查结论之外的原因）」作为独立载荷承载，与「检查结论」彻底分开：

- **载荷**：`orchestration::port` SHALL 定义 `RunFault`（`Stopped` = 用户停止 / `Environment(String)` = 环境错误摘要），并为 `StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome` 各增 `fault: Option<RunFault>`（`None` = 结论本身有效）。`RunFault` MUST NOT 出线（不进 IPC、不进 `RunUpdate` / `RunNodeRecord` 载荷、不改 `src/types/generated/bindings.ts`）。
- **分派序（fault 先于结论）**：五处门禁步执行体 SHALL 先查 `fault` 再查 `passed` / `conclusion`：

  | 步执行体 | `fault = Stopped` | `fault = Environment` | `fault = None` |
  |---|---|---|---|
  | `step_static_check`（implement / test-gen） | 节点 `stopped` + `Terminal::stopped` | 节点 `failed` + `Terminal::failed` | 既有定向反馈边 / 升格分支 |
  | `step_static_check_terminal`（initialize 基线） | 节点 `stopped` + `Terminal::stopped` | 节点 `failed` + `Terminal::failed` | 既有基线红文案分支 |
  | `step_test_execution` 及其 `escalate` / `terminal` 执行体 | 节点 `stopped` + `Terminal::stopped` | 节点 `failed` + `Terminal::failed` | 既有 pass / 升格 / 基线三态分支 |
  | `step_mutation_test`（escalate） | 节点 `stopped` + `Terminal::stopped` | （mutation 不产此面） | 既有 pass / 升格分支 |

- **`Environment` 的收敛语义**：`Environment` MUST 一律收敛 `Terminal::failed`，且 MUST NOT 升格相位 fail（MUST NOT 代写 fail `phase_log`、MUST NOT 回 `phase_next` 进决策分叉 / 重试）、MUST NOT 注回 executor（MUST NOT 发起任何 WorkerAgent 会话、MUST NOT 消耗 static-check 反馈边计数）。理由：环境坏的相位重试必然再失败，把预算花在 agent 修复上是纯消耗。
- **`Stopped` 的收敛语义**：`Stopped` MUST 收敛 `Terminal::stopped`（run `stopped`），节点 status 为 `stopped`（非 `failed`），详情仅记「用户请求停止」类停因——MUST NOT 呈现为失败。
- **终局文案**：环境面 SHALL 有专用文案常量 `TXT_ENV_STATIC`（static-check）与 `TXT_ENV_TEST`（test-execution），内容 MUST 含：① 门禁名 + 「无法完成（环境错误）」定性；② fault 摘要（占用进程名清单或「未定位占用者 + 可能为内核过滤驱动」、被锁文件路径）；③ 已自动重试次数（「每次重试会跳过已完成部分」）；④ 环境侧处理指引（请 IT 在安全软件中排除 workspace **父目录**，并说明逐 worktree 排除无效）；⑤ 报告目录（test-execution 面）。环境面文案与基线红文案（`TXT_BASELINE_STATIC_FAILED` / `TXT_BASELINE_TEST_FAILED` / `TXT_BASELINE_TEST_ERROR`）MUST 可辨：前者引导「排除安全软件路径」，后者引导「在基线上修复后重新发起」，MUST NOT 复用同一段文案。

#### Scenario: 环境错误不烧预算

- **WHEN** static-check 以 `fault = Some(Environment(…))` 收场（瞬态耗尽）
- **THEN** walker 不发任何 WorkerAgent 请求（修复会话计数为 0）、不代写 fail checklist、不调 `phase_next` 分叉；该步节点 `failed`、run 收敛 `failed`，终态 reason 含「已自动重试 N 次」与占用进程名

#### Scenario: 基线环境错误同样停给用户

- **WHEN** initialize 相位的基线 static-check / test-execution 以 `fault = Some(Environment(…))` 收场
- **THEN** run 以 `Terminal::failed` 收敛，文案为 `TXT_ENV_STATIC` / `TXT_ENV_TEST`（携占用者与重试次数），MUST NOT 落基线红文案（不误导用户去修基线）

#### Scenario: 停止收敛 stopped 而非 failed

- **WHEN** 用户停止运行中的检查步，runner 以 `fault = Some(Stopped)` 返回
- **THEN** 该步节点 status 为 `stopped`（非 `failed`）、run 终态为 `stopped`，图上无红节点、无「失败」措辞

#### Scenario: 真红分派零变

- **WHEN** static-check / test-execution 的产出 `fault == None` 且结论为红（`passed == false` / `conclusion != Pass`）
- **THEN** 走既有分支：static-check 定向反馈边（计数 +1、≤5 次后升格）、test-execution 升格交决策；行为与既有 spec 逐字一致

#### Scenario: 文案可辨

- **WHEN** 分别触发基线 static-check 环境错误与基线 static-check 检查红
- **THEN** 前者文案引导「排除 workspace 父目录（安全软件）」（含占用进程名），后者文案引导「在基线上修复后重新发起」；两段文案不共用同一句引导

#### Scenario: fault 不出线

- **WHEN** 审查 IPC 载荷与生成绑定
- **THEN** `RunFault` 未出现在 `RunUpdate` / `RunNodeRecord` / `bindings.ts` 中，`bindings:check` 零 diff

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/orchestration/src/port.rs` | 归因载荷 | `enum RunFault { Stopped, Environment(String) }`；`StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome` 各加 `fault: Option<RunFault>`；零 Serialize（不出线） |
| `crates/core/orchestration/src/walker.rs`（`step_static_check` / `step_static_check_terminal`） | static-check 分派 | fault 前置分支（`stopped` / `failed(env)`）先于 `passed` / 反馈边 / 基线红文案 |
| `crates/core/orchestration/src/walker.rs`（`step_test_execution` / `_escalate` / `_terminal`） | test-execution 分派 | fault 前置分支先于 `conclusion` 三分支；`Environment` 不进决策分叉 |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test_escalate`） | mutation 分派 | `Stopped` 前置分支（`Terminal::stopped`）先于 pass / 升格；runner `Err → Terminal` 不变 |
| `crates/core/orchestration/src/walker.rs`（文案常量） | 终局文案 | `TXT_ENV_STATIC` / `TXT_ENV_TEST`（含占用者、被锁文件、重试次数、排除父目录指引、报告目录）；与 `TXT_BASELINE_*` 不共用文案 |
| `crates/core/orchestration/src/walker.rs`（`Terminal` / `StepSettle`） | 收敛面 | `Terminal::stopped` / `StepSettle::stopped` 承载 `Stopped`；`Terminal::failed` 承载 `Environment`；节点词汇零扩展 |
| `crates/core/orchestration/src/state.rs` | 状态词汇 | 零改动（`ChangeRunStatus` / `ChangeStepStatus` / `RunUpdate` 不变） |
