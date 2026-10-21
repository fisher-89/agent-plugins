# desktop-test-execution Specification（delta: exec-run-shell）

## ADDED Requirements

### Requirement: suite 执行超时预算与取消

test-execution suite 命令的执行 SHALL 经 `exec::run_shell`（见 desktop-exec-runtime），并把 suite 命令预算常量 `SUITE_COMMAND_TIMEOUT` 由 **600 s 提升到 1200 s**（取值口径：rust suite 实测 340–393 s 的约 3 倍余量；原 600 s 仅 1.7 倍，热态正常、冷态或多 change 并发即触线）。

- **超时语义零变**：`RunOutcome::TimedOut` SHALL 映射为既有形态——`exit_code` 取 `EXIT_CODE_NO_STATUS`（`-1`）哨兵、`error` 面记「测试命令超时（1200 秒）强制终止」、`output` 为空、summary 问题记 `execution_error`（`conclusion = error`）；MUST NOT 因超时改判 pass。
- **取消语义**：`cancel: CancelFlag` 由 port 透传；`RunOutcome::Cancelled` SHALL 使 runner 返回 `Err("执行已取消")`（基础设施失败面，停给用户 / 收敛 run），MUST NOT 落为 suite 级 `execution_error`（那会被当作被测代码问题而产生假失败归因）。
- **合并流顺序零变**：`PlanExecution.output` 仍为「stdout 前缀 + stderr 后缀」；MUST NOT 加 `2>&1`（会污染 `parse_cargo_test` 的结果工件解析面）。
- 私有管道排水助手（`read_all`）SHALL 随迁移退役，管道排水单点驻 `exec`。

#### Scenario: 超时预算常量锚定

- **WHEN** 审查 `runner.rs` 的 `SUITE_COMMAND_TIMEOUT`
- **THEN** 值为 1200 s，且单元测试以常量断言钉住（原「与 CLI `runCommand(cmd, cwd, 600000)` 同值」的锚定口径退役，改为「实测 340–393 s × 3 倍余量」）

#### Scenario: 超时错误面语义不变

- **WHEN** suite 命令超过预算未退出
- **THEN** 该 plan 的 `exit_code == -1`、`error` 非空且含超时文案、`output` 为空；summary 记 `execution_error` 且 `conclusion` 为 `error`（不产假 pass、不产用例行）

#### Scenario: 取消返回 Err 而非 execution_error

- **WHEN** 用户停止 run 期间 suite 命令正在执行
- **THEN** 执行体终止进程树并返回 `Cancelled`，runner 返回 `Err("执行已取消")`；MUST NOT 落为某 plan 的 `execution_error`，MUST NOT 继续跑余下 plan

#### Scenario: 合并流与工件解析不受影响

- **WHEN** rust 档模板把 stdout 重定向进结果工件、编译错误留在 stderr
- **THEN** `PlanExecution.output` 仍为 stdout + stderr 前后拼接（编译错误面进 record 可回查），结果工件解析仍只读 stdout 面；`parse_cargo_test` 零改动

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/checks/src/testexec/runner.rs` | suite 执行 | `SUITE_COMMAND_TIMEOUT: Duration = 1200 s`；`execute_plan(plan, report_dir, cancel)` 经 `run_shell`；`TimedOut` → `EXIT_CODE_NO_STATUS` + 超时 error 文案；`Cancelled` → `Err("执行已取消")` |
| `crates/infra/checks/src/testexec/mod.rs` | runner port 实现 | `impl TestExecutionRunner for ProcessTestExecution`：`run(root, name, slot, cancel)` 签名加 `cancel: CancelFlag` 并透传 `execute_plan` |
| `crates/core/orchestration/src/port.rs` | port 契约 | `TestExecutionRunner::run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture` |
| `crates/core/checks` | 纯层 | 零改动（解析 / 聚合 / 复用门 / 阈值判定不变） |

**API 接口**：无（`TestExecutionOutcome` 载荷字段零变，无新 IPC 命令，bindings 零 diff）。
**CLI 命令**：无。
**前端组件**：无。
