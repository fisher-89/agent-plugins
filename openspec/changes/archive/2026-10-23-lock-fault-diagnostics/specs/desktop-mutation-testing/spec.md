# desktop-mutation-testing Specification（delta: lock-fault-diagnostics）

## MODIFIED Requirements

### Requirement: 段执行取消与进程树击杀

突变段执行 SHALL 经 `exec::run_shell`（见 desktop-exec-runtime），并在既有 `SegmentFailure` 二分（`Fatal` = 工具不可达 / 配置不支持 / 写盘失败 → runner `Err` 停给用户；`Interrupted` = 段中断 → `conclusion = Error` 即时反馈）之上保留第三变体 `Cancelled`：

- `SegmentFailure::Cancelled` SHALL 由 `RunOutcome::Cancelled` 映射产生，并沿 `execute → run_suite → execute_segment` 三层贯穿；`Display` 形态与另两变体同型（单串）。
- 取消 SHALL NOT 走 `Fatal`（那会把用户停止呈现为「环境坏了停给用户」）；取消的收敛口径 SHALL 为**执行面归因**（本变更改判，取代既有「段中断 + `conclusion = Error`」过渡形态）：runner SHALL 返回 `MutationTestOutcome { fault: Some(RunFault::Stopped), … }`，MUST NOT 以 `conclusion = Error` + 取消文案的升格面收场，MUST NOT 代写 fail checklist——由 walker 以 `Terminal::stopped` 收敛 run `stopped`（见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」）。
- **突变面不接 retry**：三段 spawn SHALL 以 `RetryPolicy::none()` 语义执行（突变长跑是产品语义，段内重试会放大墙钟；断点续跑已由段级复用门承担）。瞬态签名命中 MAY 记入段诊断文本，MUST NOT 触发重试。
- **段超时不进 fault**：段超时预算 SHALL 维持 1800 s（`SEGMENT_SPAWN_TIMEOUT_MS` 不变），超时臂 SHALL 为 `RunOutcome::TimedOut` + `exec::kill_tree` 整树回收（此前只杀壳，`cargo mutants` 的 rustc 子孙继续写 `CARGO_TARGET_DIR`——与两检查域同一棵树）；`TimedOut` 仍映射 `SegmentFailure::Interrupted`、`conclusion = Error`、`fault = None`、超时文案逐字不变——突变超时属配置问题而非外部环境占用，MUST NOT 与两检查域共用环境错误口径。
- 失败桶临时面保留纪律（失败 / 中断的 Stryker 桶临时 config 与 `_stryker-tmp` 不清理、成功照清）SHALL 对取消面同样适用。

#### Scenario: 取消映射为 fault=Stopped

- **WHEN** 对正在执行的 mutation 段置位 cancel（用户停止 run）
- **THEN** 执行体返回 `Cancelled`，段执行返回 `SegmentFailure::Cancelled`，runner 以 `fault = Some(Stopped)` 正常返回（MUST NOT 返回 `Err`、MUST NOT 产 `conclusion = Error` 的升格面）；walker 把该步节点收敛 `stopped`、run 收敛 `stopped`

#### Scenario: 段超时进程树消失

- **WHEN** 某段超过 1800 s 预算未退出（rustc 子孙仍在写 target）
- **THEN** 执行体 `kill_tree` 整树回收（Windows 上壳进程与 rustc 子孙的 pid 均不可再观测），段以 `Interrupted` 收场、`fault == None`、文案仍为「段执行超时（1800 秒）强制终止」

#### Scenario: 突变面零重试

- **WHEN** 段命令输出中含瞬态签名文本（如 `os error 32`）并退出非零
- **THEN** 该段 `attempts == 1`（零重试），`SegmentFailure` 分流按既有面（`Interrupted`）收场；MUST NOT 因签名命中而追加段尝试

#### Scenario: 既有分流不变

- **WHEN** 工具不可达 / 临时 config 写盘失败 / 段报告缺失等既有失败面被触发
- **THEN** 仍分别映射 `Fatal` / `Interrupted`（文案骨架与呈现口径零变），`Cancelled` 变体不参与既有面

#### Scenario: 取消后临时面保留

- **WHEN** Stryker 桶因取消（`Cancelled`）中断
- **THEN** 该桶临时 config 与 `_stryker-tmp` 按失败面保留（供单段复现），成功桶照常清理

#### Scenario: 复用门与断点续跑不受影响

- **WHEN** 取消后重入同一 change 的 MutationTest（输入未变）
- **THEN** 已完段仍走段级复用门（零 spawn），未完成段重跑；段报告与归并块口径零变

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/checks/src/mutation/mod.rs` | 段调度与失败面 | `enum SegmentFailure { Fatal(String), Interrupted(String), Cancelled(String) }`（`Display` 同型）；`execute(root, change, files, patch, cancel)` / `run_suite(..., cancel)` / `execute_segment(..., cancel)` 贯穿 cancel；`Cancelled` → `fault = Some(Stopped)`（不产 `conclusion = Error` 升格面）、`Interrupted` → `conclusion = Error` + `fault = None`、`Fatal` → `Err` |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | 段执行（rust） | `run_segment(patch_path, exec_dir, cancel)` 经 `run_shell`（`env CARGO_TARGET_DIR`、1800 s、`RetryPolicy::none()`）；`TimedOut` → `Interrupted`（文案不变）、`Cancelled` → `Cancelled` |
| `crates/infra/checks/src/mutation/stryker.rs` | 段执行（js 族） | `run_segment(segment, cancel)` 经 `run_shell`（`cwd = suite_cwd`、1800 s、`RetryPolicy::none()`）；同 `TimedOut` / `Cancelled` 映射 |
| `crates/core/orchestration/src/port.rs` | 载荷 | `MutationTestOutcome { conclusion, skip, score, threshold, counts, report_paths, feedback_text, fault: Option<RunFault> }`（仍零 Serialize，不出线） |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test_escalate`） | 步分派 | `fault = Stopped` 前置分支（`Terminal::stopped`）先于 pass / 升格；`fault == None` 时既有 pass / 升格链逐字不变 |
| `crates/core/checks` | 纯层 | 零改动（`parse_*_seg` / score / 归并 / 阈值判定不变） |
