# desktop-test-execution Specification（delta: lock-fault-diagnostics）

## MODIFIED Requirements

### Requirement: suite 执行超时预算与取消

test-execution suite 命令的执行 SHALL 经 `exec::run_shell`（见 desktop-exec-runtime），并把 suite 命令预算常量 `SUITE_COMMAND_TIMEOUT` 由 **600 s 提升到 1200 s**（取值口径：rust suite 实测 340–393 s 的约 3 倍余量；原 600 s 仅 1.7 倍，热态正常、冷态或多 change 并发即触线）。

- **瞬态重试**：suite 命令 SHALL 以 `RetryPolicy::transient()` 执行（见 desktop-exec-runtime「瞬态签名字典与有限重试策略」——构建产物被安全软件占用时的 `os error 32` / `failed to rename archive file` 等签名）——重试利用 cargo 增量进度自愈（已建成 crate 不重建，剩余待写文件递减）；`TimedOut` / `Cancelled` 零重试。
- **超时语义（本变更改判为执行面归因）**：`RunOutcome::TimedOut` SHALL 映射为 `fault = Some(RunFault::Environment("测试命令超时（1200 秒）强制终止"))`，`exit_code` 仍取 `EXIT_CODE_NO_STATUS`（`-1`）哨兵、`output` 为空；该 plan MUST NOT 记 `execution_error`（超时不是被测代码问题），summary MUST NOT 因此产 `conclusion = error` 的假失败归因；MUST NOT 因超时改判 pass。
- **瞬态耗尽**：重试耗尽且末次仍命中瞬态签名 → `fault = Some(RunFault::Environment(…))`（文本含占用者点名 / 被锁文件 / 已重试次数 / 处理指引）；该 plan MUST NOT 记 `execution_error`（同上，环境面不落「被测代码问题」归因）。
- **取消语义（本变更改判）**：`cancel: CancelFlag` 由 port 透传；`RunOutcome::Cancelled` SHALL 使 runner 以 `fault = Some(RunFault::Stopped)` 正常返回（MUST NOT 返回 `Err("执行已取消")`），MUST NOT 落为 suite 级 `execution_error`、MUST NOT 继续跑余下 plan。
- **`conclusion = Error` 收窄**：成因面 SHALL 仅限「结果工件解析失败 / harness 崩溃 / 框架探测失败 / 程序不可达及配置不支持（后者仍为 `Err`）」；超时与瞬态耗尽 MUST 由 fault 承载（fault 在位时 `conclusion` 值不参与门禁分派）。
- **合并流顺序零变**：`PlanExecution.output` 仍为「stdout 前缀 + stderr 后缀」；重试场景下 SHALL 为各次尝试按序拼接、每段前加分隔头 `===== attempt N/5 · exit=C · transient=<命中签名> =====`（审计不丢上下文）；MUST NOT 加 `2>&1`（会污染 `parse_cargo_test` 的结果工件解析面）。
- 私有管道排水助手（`read_all`）SHALL 随迁移退役，管道排水单点驻 `exec`。

#### Scenario: 超时预算常量锚定

- **WHEN** 审查 `runner.rs` 的 `SUITE_COMMAND_TIMEOUT`
- **THEN** 值为 1200 s，且单元测试以常量断言钉住（原「与 CLI `runCommand(cmd, cwd, 600000)` 同值」的锚定口径退役，改为「实测 340–393 s × 3 倍余量」）

#### Scenario: 超时入 fault 而不再落 execution_error

- **WHEN** suite 命令超过预算未退出
- **THEN** 该 plan 的 `exit_code == -1`、`output` 为空、`fault == Some(Environment(超时文案))`，`error` 面不含「被测代码」归因、summary 不记 `execution_error`；`TestExecutionOutcome.fault` 上抛，`conclusion` 值不构成门禁结论

#### Scenario: 瞬态耗尽入 fault

- **WHEN** 某 suite 命令连续 5 次以锁文本退出 1（重试耗尽）
- **THEN** `fault == Some(Environment(…))` 且文本含占用者点名与已重试次数；`PlanExecution.output` 含 5 段 attempt 分隔头的输出；summary 不记 `execution_error`

#### Scenario: 取消返回 fault 而非 Err

- **WHEN** 用户停止 run 期间 suite 命令正在执行
- **THEN** 执行体终止进程树并返回 `Cancelled`，runner 以 `fault = Some(Stopped)` 正常返回；MUST NOT 落为某 plan 的 `execution_error`，MUST NOT 继续跑余下 plan，MUST NOT 返回 `Err`

#### Scenario: 真红零变

- **WHEN** suite 命令以真实测试失败退出（输出无瞬态签名）
- **THEN** `attempts == 1`、`fault == None`、`conclusion == Fail`，既有 `execution_error` / 用例行解析链与升格路径逐字不变

#### Scenario: 合并流与工件解析不受影响

- **WHEN** rust 档模板把 stdout 重定向进结果工件、编译错误留在 stderr
- **THEN** `PlanExecution.output` 仍为 stdout + stderr 前后拼接（编译错误面进 record 可回查），结果工件解析仍只读 stdout 面；`parse_cargo_test` 零改动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/checks/src/testexec/runner.rs` | suite 执行与归因 | `run_shell(cwd = plan.cwd, timeout = 1200 s, cancel, retry = RetryPolicy::transient())`；`TimedOut` / 瞬态耗尽 → `fault = Environment(…)`（不产 plan `execution_error`）；`Cancelled` → `fault = Stopped`；`PlanExecution { …, fault: Option<RunFault> }`；尝试分隔头拼接 |
| `crates/infra/checks/src/testexec/mod.rs` | 聚合与上传 | 多 plan fault 聚合（`Stopped` 优先于 `Environment`）；`TestExecutionOutcome.fault`；`conclusion = Error` 成因面收窄 |
| `crates/core/orchestration/src/port.rs` | 载荷 | `TestExecutionOutcome { conclusion, total, passed, failed, skipped, findings_brief, findings_detail, report_dir, exec_id, fault: Option<RunFault> }` |
| `crates/core/orchestration/src/walker.rs` | 步分派 | fault 前置分支（见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」）；`conclusion` 三分支仅在 `fault == None` 时命中 |
| `crates/core/checks` | 纯层 | 零改动（聚合 / 解析 / 复用门 / 阈值判定不变） |
