# desktop-checks-domain Specification（delta: lock-fault-diagnostics）

## MODIFIED Requirements

### Requirement: 检查域 spawn 统一经 exec 执行体

`crates/infra/checks` 的四类外部进程 spawn（`static_check.rs` 的 static-check、`testexec/runner.rs` 的 suite 命令、`mutation/cargo_mutants.rs` 的 cargo-mutants 段、`mutation/stryker.rs` 的 Stryker 段）SHALL 一律经 `exec::run_shell`（见 desktop-exec-runtime「统一执行体 run_shell 契约」）执行；MUST NOT 各调用点自持 spawn + 超时 + 管道排水实现（三份私有 `read_all` 助手退役），MUST NOT 出现裸 `tokio::time::timeout` + `start_kill` 组合。

四调用点的能力面 SHALL 齐平为「超时 / 进程树击杀 / 双管道 / 取消」四项：

- **超时**：static-check 由**零超时**改为 1200 s（此前 cargo 卡在文件锁等待时永久悬挂）；test-execution suite 1200 s（见 desktop-test-execution）；两 mutation 段 1800 s（不变）。
- **进程树击杀**：超时与取消两臂经 `exec::kill_tree` 杀整树并收尸（Windows `taskkill /T /F`），不再只杀壳。
- **双管道**：执行体并发排水两路；调用方按各自既有口径消费——static-check 仍以命令内 `2>&1` 把 stderr 并入 stdout（`stderr` 恒空、诊断与 record 单流语义逐字不变）；test-execution 与 mutation 仍以「stdout 前缀 + stderr 后缀」拼接合并流。
- **取消**：`cancel: CancelFlag` 由调用方透传（见 desktop-change-orchestration「停止贯通到进程执行」）。

取消在检查域的收敛口径 SHALL 为执行面归因（本变更改判，取代既有过渡形态）：`RunOutcome::Cancelled` → runner 置 `fault = Some(RunFault::Stopped)` 后正常返回产出（MUST NOT 返回 `Err("执行已取消")`）→ walker 以 `Terminal::stopped` 收敛 run `stopped`（见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」）。`Fatal` / `Interrupted` 等既有失败分流语义 MUST NOT 因迁移而改变。

#### Scenario: 四调用点审查

- **WHEN** 审查 `static_check.rs` / `testexec/runner.rs` / `mutation/cargo_mutants.rs` / `mutation/stryker.rs` 的进程执行面
- **THEN** 四者均调用 `exec::run_shell` 且各传超时预算与 `cancel`；四文件内无 `tokio::time::timeout`、无 `start_kill`、无私有 `read_all`

#### Scenario: static-check 首获超时

- **WHEN** static-check 命令永不退出（如 cargo 阻塞于 `Blocking waiting for file lock`）
- **THEN** 1200 s 到期后执行体收敛 `TimedOut` 并回收进程树，static-check runner 以 `fault = Some(Environment(超时文案))` 正常返回（MUST NOT 让 run 永久停在 running，MUST NOT 以 `Err` 收场）

#### Scenario: static-check 交错流语义不变

- **WHEN** static-check 命令交错写 stdout 与 stderr（命令串仍带 `2>&1`）
- **THEN** `StaticCheckOutcome.diagnostics` 与 `<report_dir>/exec-outputs/<exec_id>.log` 逐字等于该交错流，`stderr` 恒空（MUST NOT 变成「stdout 全量 + stderr 全量」分离拼接）

#### Scenario: 合并流顺序零变

- **WHEN** test-execution suite 或 mutation 段命令分别向 stdout 与 stderr 写入
- **THEN** 落 record / 失败消息的合并流仍是「stdout 前缀 + stderr 后缀」，与迁移前逐字一致

#### Scenario: 取消即杀不残留

- **WHEN** 用户停止正在运行的检查步（cancel 置位）
- **THEN** 执行体终止进程树、收尸并返回 `Cancelled`，runner 以 `fault = Some(Stopped)` 返回；子进程（含 rustc 类子孙）不再继续写 `target/`

#### Scenario: 既有失败分流不变

- **WHEN** 程序不可达、配置缺失、段报告缺失等既有失败面被触发
- **THEN** static-check / test-execution / mutation 各自维持原有 `Err` / `Fatal` / `Interrupted` 语义与文案骨架（迁移只换执行形态，不改分流）

## ADDED Requirements

### Requirement: 瞬态重试与环境错误归因（fault 装配）

检查域三 runner SHALL 把「执行面归因」与「检查结论」分开承载：`StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome` 各增 `fault: Option<RunFault>`（`RunFault = Stopped | Environment(String)`，定义见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」），`passed` / `conclusion` **MUST 保持既有取值面**（MUST NOT 升三值、MUST NOT 因 fault 改口径）；fault 在位时该值不参与门禁分派（walker 以 fault 先于结论分派）。

- **重试开面**：`static_check.rs` 与 `testexec/runner.rs` SHALL 以 `RetryPolicy::transient()`（见 desktop-exec-runtime「瞬态签名字典与有限重试策略」）执行其 spawn；`mutation/` 三段 SHALL NOT 接 retry（突变长跑是产品语义，段内重试会放大墙钟且复用门已承担断点续跑）。
- **fault 装配表**（static-check / test-execution 两面；逐 `RunOutcome` / 瞬态判定。mutation 的生命周期见下「mutation 专属」——其段超时有意不进 fault）：

| 来源 | `fault` |
|---|---|
| `TimedOut` | `Environment("…超时（N 秒）…")` |
| `Cancelled` | `Stopped` |
| `Completed` + 重试耗尽 + 命中瞬态签名 | `Environment(末次诊断摘要 + 占用者点名 + 已重试次数 + 环境侧处理指引)` |
| `Completed` + 其余非零（真红，无签名） | `None`（走既有反馈边 / 升格 / 基线红文案） |
| 无配置直过（static-check） | `None` |

- **退出面收窄**：环境面（超时 / 瞬态耗尽）MUST NOT 以 `Err` 表达（`Err` 保留给「进程拉不起 / 程序不可达 / 写盘失败 / 配置不支持」等停给用户的基础设施失败——那些面本就没有可重试的执行）；`Cancelled` MUST NOT 再返回 `Err("执行已取消")`。
- **test-execution 专属**：`conclusion = Error` 的成因面 SHALL 收窄为「结果工件解析失败 / harness 崩溃 / 框架探测失败」；超时与瞬态耗尽 MUST 由 fault 承载，且 MUST NOT 落 plan 级 `execution_error`（那会被当作被测代码问题而产生假失败归因）。多 plan 聚合时 fault SHALL 上抛（任一 plan 有 fault 即 outcome 带 fault；`Stopped` 优先于 `Environment`）。
- **诊断与记录**：命中瞬态签名时 SHALL 调 `exec::lockprobe::attribution_text`（仅此刻；常态路径零探针开销）取占用者点名；输出记录仍为**单一 `.log`**（`exec::ExecOutput::Recorded`），内容 SHALL 为全部尝试按序拼接、每段前加分隔头 `===== attempt N/5 · exit=C · transient=<命中签名> =====`（审计不丢上下文）；`diagnostics` / `findings_*` SHALL 反映末次尝试 + fault 归因段（MUST NOT 只记末次而丢掉重试事实）。
- **mutation 专属**：`SegmentFailure::Cancelled` SHALL 映射 `fault = Stopped`（见 desktop-mutation-testing「段执行取消与进程树击杀」）；`Interrupted`（含段超时）与 `Fatal` 的既有分流 MUST NOT 因 fault 面而改变，且 `Interrupted` 面 SHALL 保持 `fault = None`（段超时有意不进 fault）。

#### Scenario: 重试耗尽判环境错误

- **WHEN** static-check 命令每次都以 `failed to rename archive file` + `os error 32` 退出 1（重试 5 次后耗尽）
- **THEN** `StaticCheckOutcome.passed == false` 且 `fault == Some(Environment(…))`；fault 文本含占用者点名（或「未定位占用者」降级说明）、被锁文件路径、「已自动重试 4 次」与环境侧处理指引；`.log` 内含 5 段带 attempt 分隔头的输出

#### Scenario: 真红不产 fault

- **WHEN** 静态检查因真实 lint 错误退出 1（输出不含任何瞬态签名）
- **THEN** `passed == false`、`fault == None`、`attempts == 1`；既有定向反馈边 / 基线红分支照旧

#### Scenario: 无配置直过无 fault

- **WHEN** workspace 未配置 static-check 命令
- **THEN** `passed == true`、`fault == None`、零 spawn、零探针

#### Scenario: 超时与取消走 fault

- **WHEN** static-check 或 test-execution 分别以 `TimedOut` / `Cancelled` 收场
- **THEN** 前者 `fault == Some(Environment(超时文案))`、后者 `fault == Some(Stopped)`；两者均不返回 `Err`；test-execution 的 summary 不记 plan 级 `execution_error`

#### Scenario: 瞬态耗尽不落 execution_error

- **WHEN** test-execution 某 suite 命令连续 5 次命中瞬态签名并耗尽重试
- **THEN** `TestExecutionOutcome.fault == Some(Environment(…))`；该 plan 的 `execution_error` 为空（不把环境面写成「被测代码问题」），`conclusion` 值不构成门禁结论

#### Scenario: 解析失败仍为 Error 且无 fault

- **WHEN** suite 命令退出 0 但结果工件缺失 / 不可解析
- **THEN** `conclusion == Error`、`fault == None`（真环境面：工件解析 / harness），既有 `execution_error` 归因链不变

#### Scenario: mutation 不接重试且只映射取消

- **WHEN** 审查 `crates/infra/checks/src/mutation/` 的 spawn 调用与 `SegmentFailure` 映射
- **THEN** 三段 spawn 均以 `RetryPolicy::none()` 语义执行（零重试面）；`Cancelled → fault = Stopped`、`Interrupted`（含段超时）→ `fault = None` + `conclusion = Error`、`Fatal` → `Err`

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/checks/src/static_check.rs` | static-check 装配 | `RunOptions.retry = RetryPolicy::transient()`；`execute` 按 fault 装配表填 `StaticCheckOutcome.fault`；record 拼全部尝试（attempt 分隔头）；命中签名时调 `lockprobe::attribution_text` |
| `crates/infra/checks/src/testexec/runner.rs` | suite 执行装配 | 同上；`PlanExecution` 增 `fault: Option<RunFault>`；`TimedOut` 不产 plan `execution_error`；`Cancelled` 不 `Err`；合并流顺序零变 |
| `crates/infra/checks/src/testexec/mod.rs` | 结论与 fault 上传 | 多 plan fault 聚合（`Stopped` 优先）；`conclusion = Error` 成因面收窄为解析 / harness / 框架探测失败 |
| `crates/infra/checks/src/mutation/mod.rs` | 段执行失败面 | 不接 retry；`SegmentFailure::Cancelled` → `fault = Some(Stopped)`；`Interrupted` → `conclusion = Error` + `fault = None`；`Fatal` → `Err` |
| `crates/core/orchestration/src/port.rs` | 载荷 | 三 outcome 各加 `fault: Option<RunFault>`；`passed` / `conclusion` 取值面不变 |
| `crates/infra/exec/src/transient.rs` / `lockprobe.rs` | 消费面 | `RetryPolicy::transient()` / `none()`；`attribution_text(diagnostics) -> Option<String>`（占用者 + 被锁文件 + 降级说明） |
| `crates/core/checks` | 纯层 | 零改动（解析 / 聚合 / 复用门 / 映射表不变） |
