# desktop-change-orchestration Specification（delta: test-exec-argv-budget）

## MODIFIED Requirements

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
- **终局文案（本变更改为按成因分型）**：环境面 SHALL 有专用文案常量 `TXT_ENV_STATIC`（static-check）与 `TXT_ENV_TEST`（test-execution），内容 MUST 含：① 门禁名 + 「无法完成（环境错误）」定性；② `{fault}` 摘要；③ 报告目录（test-execution 面）。`{fault}` 摘要面 SHALL **按成因分型**，两类各自齐备且 MUST NOT 互相套用：

  | 成因 | `{fault}` 摘要 MUST 含 | MUST NOT 含 |
  |---|---|---|
  | 文件锁 / 瞬态耗尽 / 超时（既有面） | 占用进程名清单或「未定位占用者 + 可能为内核过滤驱动」、被锁文件路径、已自动重试次数（「每次重试会跳过已完成部分」）、环境侧处理指引（请 IT 在安全软件中排除 workspace **父目录**，逐 worktree 排除无效） | —— |
  | 命令行超平台长度上限（本变更新增，见 desktop-test-execution「命令展开长度预算与安全降级」） | 「harness 限制、非被测代码问题」定性、展开长度 / 平台预算 / 文件数三个数字、「收窄 suite includes 或拆分 suite」动作 | 占用者点名、被锁文件、重试次数、安全软件排除指引（成因不同，误引会把用户引向环境侧排查） |

  环境面文案与基线红文案（`TXT_BASELINE_STATIC_FAILED` / `TXT_BASELINE_TEST_FAILED` / `TXT_BASELINE_TEST_ERROR`）MUST 可辨：前者归类「环境错误」，后者引导「在基线上修复后重新发起」，MUST NOT 复用同一段文案。分型只影响 `{fault}` 摘要的装配方（checks 域单点），walker 侧两个模板常量与 `settle_fault` 逻辑 MUST 零改动。

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

#### Scenario: 命令行超限文案按成因分型

- **WHEN** test-execution 因展开长度超平台预算且不可安全降级以 `fault = Some(Environment(…))` 收场
- **THEN** run 收敛 `Terminal::failed`、节点 `failed`（零 WorkerAgent、零决策分叉、零 fail checklist 代写），终态 reason 展开为 `TXT_ENV_TEST` 且 `{fault}` 段含长度 / 预算 / 文件数与「收窄 includes 或拆分 suite」动作；MUST NOT 出现占用者点名、被锁文件、「已自动重试 N 次」或安全软件排除指引

#### Scenario: fault 不出线

- **WHEN** 审查 IPC 载荷与生成绑定
- **THEN** `RunFault` 未出现在 `RunUpdate` / `RunNodeRecord` / `bindings.ts` 中，`bindings:check` 零 diff

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs` | 分派单点（零代码改动） | `settle_fault` 与 `TXT_ENV_STATIC` / `TXT_ENV_TEST` 逐字不变；命令行超限经同一 `Environment` 臂收敛 `Terminal::failed`——分型只发生在 `{fault}` 摘要的装配方（checks 域），walker MUST NOT 因成因不同新增分支 |
| `crates/core/orchestration/src/port.rs` | `RunFault` 面（零扩面） | 变体封闭集仍为 `{Stopped, Environment(String)}`（MUST NOT 为命令行超限新增变体）；`Environment(String)` 的摘要文本由 checks 域按成因分型装配 |
