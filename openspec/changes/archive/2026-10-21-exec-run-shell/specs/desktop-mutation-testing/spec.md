# desktop-mutation-testing Specification（delta: exec-run-shell）

## ADDED Requirements

### Requirement: 段执行取消与进程树击杀

突变段执行 SHALL 经 `exec::run_shell`（见 desktop-exec-runtime），并在既有 [`SegmentFailure`] 二分（`Fatal` = 工具不可达 / 配置不支持 / 写盘失败 → runner `Err` 停给用户；`Interrupted` = 段中断 → `conclusion = Error` 即时反馈）之上新增第三变体 `Cancelled`：

- `SegmentFailure::Cancelled` SHALL 由 `RunOutcome::Cancelled` 映射产生，并沿 `execute → run_suite → execute_segment` 三层贯穿；`Display` 形态与另两变体同型（单串）。
- 取消 SHALL NOT 走 `Fatal`（那会把用户停止呈现为「环境坏了停给用户」）；取消的收敛口径 SHALL 为段中断面——`conclusion = Error`、`feedback_text` 记「突变执行已取消」+ 已产报告路径清单（过渡形态：MUST NOT 在本变更内引入 run stopped）。
- 段超时预算 SHALL 维持 1800 s（`SEGMENT_SPAWN_TIMEOUT_MS` 不变），但超时臂 SHALL 改为 `RunOutcome::TimedOut` + `exec::kill_tree` 整树回收（此前只杀壳，`cargo mutants` 的 rustc 子孙继续写 `CARGO_TARGET_DIR`——与两检查域同一棵树）；`TimedOut` 仍映射 `SegmentFailure::Interrupted`，超时文案逐字不变。
- 失败桶临时面保留纪律（失败 / 中断的 Stryker 桶临时 config 与 `_stryker-tmp` 不清理、成功照清）SHALL 对取消面同样适用。

#### Scenario: 取消映射为段中断

- **WHEN** 对正在执行的 mutation 段置位 cancel（用户停止 run）
- **THEN** 执行体返回 `Cancelled`，段执行返回 `SegmentFailure::Cancelled`，runner 以 `conclusion = Error` 收场且 `feedback_text` 含取消文案与已产报告路径；MUST NOT 返回 `Err`（`Fatal`）

#### Scenario: 段超时进程树消失

- **WHEN** 某段超过 1800 s 预算未退出（rustc 子孙仍在写 target）
- **THEN** 执行体 `kill_tree` 整树回收（Windows 上壳进程与 rustc 子孙的 pid 均不可再观测），段以 `Interrupted` 收场、文案仍为「段执行超时（1800 秒）强制终止」

#### Scenario: 既有分流不变

- **WHEN** 工具不可达 / 临时 config 写盘失败 / 段报告缺失等既有失败面被触发
- **THEN** 仍分别映射 `Fatal` / `Interrupted`（文案骨架与呈现口径零变），`Cancelled` 变体不参与既有面

#### Scenario: 取消后临时面保留

- **WHEN** Stryker 桶因取消（`Cancelled`）中断
- **THEN** 该桶临时 config 与 `_stryker-tmp` 按失败面保留（反馈面附路径供单段复现），成功桶照常清理

#### Scenario: 复用门与断点续跑不受影响

- **WHEN** 取消后重入同一 change 的 MutationTest（输入未变）
- **THEN** 已完段仍走段级复用门（零 spawn），未完成段重跑；段报告与归并块口径零变

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/checks/src/mutation/mod.rs` | 段调度与失败面 | `enum SegmentFailure { Fatal(String), Interrupted(String), Cancelled(String) }`（`Display` 同型）；`execute(root, change, files, patch, cancel)` / `run_suite(..., cancel)` / `execute_segment(..., cancel)` 贯穿 cancel；`Cancelled` → `conclusion = Error` + 取消文案 |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | 段执行（rust） | `run_segment(patch_path, exec_dir, cancel)` 经 `run_shell`（`env CARGO_TARGET_DIR`、1800 s）；`TimedOut` → `Interrupted`（文案不变）、`Cancelled` → `Cancelled` |
| `crates/infra/checks/src/mutation/stryker.rs` | 段执行（js 族） | `run_segment(segment, cancel)` 经 `run_shell`（`cwd = suite_cwd`、1800 s）；同 `TimedOut` / `Cancelled` 映射 |
| `crates/core/orchestration/src/port.rs` | port 契约 | `MutationRunner::run(&self, root: &str, name: &str, files: &[String], patch: &str, cancel: CancelFlag) -> BoxToolFuture` |
| `crates/infra/checks/src/mutation/mod.rs`（port 实现） | runner 实现 | `impl MutationRunner for ProcessMutation`：签名加 `cancel` 并透传 `execute` |
| `crates/core/checks` | 纯层 | 零改动（`parse_*_seg` / score / 归并 / 阈值判定不变） |

**API 接口**：无（`MutationTestOutcome` 载荷字段零变；载荷零 Serialize 不进 IPC 的既有约束不变）。
**CLI 命令**：无。
**前端组件**：无。
