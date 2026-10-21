# desktop-checks-domain Specification（delta: exec-run-shell）

## ADDED Requirements

### Requirement: 检查域 spawn 统一经 exec 执行体

`crates/infra/checks` 的四类外部进程 spawn（`static_check.rs` 的 static-check、`testexec/runner.rs` 的 suite 命令、`mutation/cargo_mutants.rs` 的 cargo-mutants 段、`mutation/stryker.rs` 的 Stryker 段）SHALL 一律经 `exec::run_shell`（见 desktop-exec-runtime「统一执行体 run_shell 契约」）执行；MUST NOT 各调用点自持 spawn + 超时 + 管道排水实现（三份私有 `read_all` 助手退役），MUST NOT 出现裸 `tokio::time::timeout` + `start_kill` 组合。

四调用点的能力面 SHALL 齐平为「超时 / 进程树击杀 / 双管道 / 取消」四项：

- **超时**：static-check 由**零超时**改为 1200 s（此前 cargo 卡在文件锁等待时永久悬挂）；test-execution suite 1200 s（见 desktop-test-execution）；两 mutation 段 1800 s（不变）。
- **进程树击杀**：超时与取消两臂经 `exec::kill_tree` 杀整树并收尸（Windows `taskkill /T /F`），不再只杀壳。
- **双管道**：执行体并发排水两路；调用方按各自既有口径消费——static-check 仍以命令内 `2>&1` 把 stderr 并入 stdout（`stderr` 恒空、诊断与 record 单流语义逐字不变）；test-execution 与 mutation 仍以「stdout 前缀 + stderr 后缀」拼接合并流。
- **取消**：`cancel: CancelFlag` 由调用方透传（见 desktop-change-orchestration「停止贯通到进程执行」）。

取消在检查域的收敛口径 SHALL 为过渡形态：`RunOutcome::Cancelled` → runner 返回 `Err("执行已取消")` → walker 将该步收敛为 failed（MUST NOT 在本变更内引入 run stopped；正式收敛另行变更）。`Fatal` / `Interrupted` 等既有失败分流语义 MUST NOT 因迁移而改变。

#### Scenario: 四调用点审查

- **WHEN** 审查 `static_check.rs` / `testexec/runner.rs` / `mutation/cargo_mutants.rs` / `mutation/stryker.rs` 的进程执行面
- **THEN** 四者均调用 `exec::run_shell` 且各传超时预算与 `cancel`；四文件内无 `tokio::time::timeout`、无 `start_kill`、无私有 `read_all`

#### Scenario: static-check 首获超时

- **WHEN** static-check 命令永不退出（如 cargo 阻塞于 `Blocking waiting for file lock`）
- **THEN** 1200 s 到期后执行体收敛 `TimedOut` 并回收进程树，static-check runner 返回 `Err`（MUST NOT 让 run 永久停在 running）

#### Scenario: static-check 交错流语义不变

- **WHEN** static-check 命令交错写 stdout 与 stderr（命令串仍带 `2>&1`）
- **THEN** `StaticCheckOutcome.diagnostics` 与 `<report_dir>/exec-outputs/<exec_id>.log` 逐字等于该交错流，`stderr` 恒空（MUST NOT 变成「stdout 全量 + stderr 全量」分离拼接）

#### Scenario: 合并流顺序零变

- **WHEN** test-execution suite 或 mutation 段命令分别向 stdout 与 stderr 写入
- **THEN** 落 record / 失败消息的合并流仍是「stdout 前缀 + stderr 后缀」，与迁移前逐字一致

#### Scenario: 取消即杀不残留

- **WHEN** 用户停止正在运行的检查步（cancel 置位）
- **THEN** 执行体终止进程树、收尸并返回 `Cancelled`，runner 返回 `Err("执行已取消")`；子进程（含 rustc 类子孙）不再继续写 `target/`

#### Scenario: 既有失败分流不变

- **WHEN** 程序不可达、配置缺失、段报告缺失等既有失败面被触发
- **THEN** static-check / test-execution / mutation 各自维持原有 `Err` / `Fatal` / `Interrupted` 语义与文案骨架（迁移只换执行形态，不改分流）

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/checks/src/static_check.rs` | static-check 执行 | `execute(root, name, slot)` 经 `run_shell(command, RunOptions { cwd: root, timeout: 1200 s, cancel })`；命令串仍为 `{command} 2>&1`；`diagnostics = report.stdout`、`passed = exit_code == 0`、record 取 `report.stdout` |
| `crates/infra/checks/src/testexec/runner.rs` | suite 执行 | `SUITE_COMMAND_TIMEOUT` 600 s → **1200 s**；`execute_plan` 经 `run_shell(cwd = plan.cwd, timeout, cancel)`；`TimedOut` → 既有 `EXIT_CODE_NO_STATUS` / `execution_error` 面；私有 `read_all` 退役 |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | 段执行 | `run_segment(patch_path, exec_dir, cancel)` 经 `run_shell(cwd = exec_dir, env = [CARGO_TARGET_DIR], timeout = 1800 s, cancel)`；`TimedOut` → `SegmentFailure::Interrupted`（文案不变）；`Cancelled` → `SegmentFailure::Cancelled`；私有 `read_all` 退役 |
| `crates/infra/checks/src/mutation/stryker.rs` | 段执行 | `run_segment(segment, cancel)` 经 `run_shell(cwd = suite_cwd, timeout = 1800 s, cancel)`；同 `TimedOut` / `Cancelled` 映射；私有 `read_all` 退役 |
| `crates/core/checks` | 纯层 | 零改动（解析 / 聚合 / 复用门 / 映射表均不变） |

**API 接口**：无（checks 域不暴露 IPC 命令；`ToolStepOutput` 载荷字段零变，bindings 零 diff）。
**CLI 命令**：无（不改 `plugins/**`）。
**前端组件**：无。
