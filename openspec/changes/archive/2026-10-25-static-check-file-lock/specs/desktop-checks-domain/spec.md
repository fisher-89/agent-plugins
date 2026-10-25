# desktop-checks-domain Specification（delta: static-check-file-lock）

## MODIFIED Requirements

### Requirement: 瞬态重试与环境错误归因（fault 装配）

检查域三 runner SHALL 把「执行面归因」与「检查结论」分开承载：`StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome` 各增 `fault: Option<RunFault>`（`RunFault = Stopped | Environment(String)`，定义见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」），`passed` / `conclusion` **MUST 保持既有取值面**（MUST NOT 升三值、MUST NOT 因 fault 改口径）；fault 在位时该值不参与门禁分派（walker 以 fault 先于结论分派）。

- **重试开面**：`static_check.rs` 与 `testexec/runner.rs` SHALL 以 `RetryPolicy::transient()`（见 desktop-exec-runtime「瞬态签名字典与有限重试策略」）执行其 spawn；`mutation/` 三段 SHALL NOT 接 retry（突变长跑是产品语义，段内重试会放大墙钟且复用门已承担断点续跑）。
- **fault 装配表**（static-check / test-execution 两面；逐 `RunOutcome` / 瞬态判定。mutation 的生命周期见下「mutation 专属」——其段超时有意不进 fault）：

| 来源 | `fault` |
|---|---|
| `TimedOut` | `Environment("…超时（N 秒）…")` |
| `Cancelled` | `Stopped` |
| `Completed` + 重试耗尽 + 命中瞬态签名 | `Environment(占用者点名 + 已重试次数 + 重试进展序列 + 末次诊断摘要 + 环境侧处理指引)` |
| `Completed` + 其余非零（真红，无签名） | `None`（走既有反馈边 / 升格 / 基线红文案） |
| 无配置直过（static-check） | `None` |

- **退出面收窄**：环境面（超时 / 瞬态耗尽）MUST NOT 以 `Err` 表达（`Err` 保留给「进程拉不起 / 程序不可达 / 写盘失败 / 配置不支持」等停给用户的基础设施失败——那些面本就没有可重试的执行）；`Cancelled` MUST NOT 再返回 `Err("执行已取消")`。
- **test-execution 专属**：`conclusion = Error` 的成因面 SHALL 收窄为「结果工件解析失败 / harness 崩溃 / 框架探测失败」；超时与瞬态耗尽 MUST 由 fault 承载，且 MUST NOT 落 plan 级 `execution_error`（那会被当作被测代码问题而产生假失败归因）。多 plan 聚合时 fault SHALL 上抛（任一 plan 有 fault 即 outcome 带 fault；`Stopped` 优先于 `Environment`）。
- **诊断与记录**：命中瞬态签名时 SHALL 调 `exec::lockprobe::attribution_text`（仅此刻；常态路径零探针开销）取占用者点名；输出记录仍为**单一 `.log`**（`exec::ExecOutput::Recorded`），内容 SHALL 为全部尝试按序拼接、每段前加分隔头 `===== attempt N/5 · exit=C · transient=<命中签名> =====`（审计不丢上下文）；`diagnostics` / `findings_*` SHALL 反映末次尝试 + fault 归因段（MUST NOT 只记末次而丢掉重试事实）。
- **重试推进可观测（fault 文案补「推进序列」）**：`environment::fault_text` SHALL 收齐尝试面——签名扩为「`attempts` + 末次 `exit_code` + 末次输出字节 + `retried: &[AttemptTrace]`」，并按 `exec::progress_series` 在归因段之后、诊断摘要之前输出一行 `重试进展：待编译 <c1> → <c2> → … → <cN>`（各次尝试的 `Compiling` 行计数，末次在内，单点取数 MUST NOT 在检查域重算计数口径）。`attempts == 1`（未重试）或序列全为 `0`（非 cargo 构建形态）时 MUST NOT 输出该行（无推进可言）；`exec::is_progress_stalled` 为真时该行 SHALL 附「末两次无进展，判定为卡死」——把「重试确实在推进」与「同一批写不进去」变成用户可见证据，并自证卡死早停判据。
- **mutation 专属**：`SegmentFailure::Cancelled` SHALL 映射 `fault = Stopped`（见 desktop-mutation-testing「段执行取消与进程树击杀」）；`Interrupted`（含段超时）与 `Fatal` 的既有分流 MUST NOT 因 fault 面而改变，且 `Interrupted` 面 SHALL 保持 `fault = None`（段超时有意不进 fault）。

#### Scenario: 重试耗尽判环境错误

- **WHEN** static-check 命令每次都以 `failed to rename archive file` + `os error 32` 退出 1，且输出不含 `Compiling ` 行（待编译计数恒 `0`，不触发卡死早停，走满 5 次）
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

#### Scenario: 环境文案含重试推进序列

- **WHEN** static-check 各次尝试的合并输出分别含 264 / 5 / 5 行 `Compiling <crate> v…` 且均命中锁签名，重试面以卡死早停收口（`attempts == 3`）
- **THEN** `fault` 文本含 `重试进展：待编译 264 → 5 → 5`，并附「末两次无进展，判定为卡死」；同文仍含占用者点名、被锁文件路径、「已自动重试 2 次」与环境侧处理指引

#### Scenario: 未重试无推进序列行

- **WHEN** 环境文案的 `attempts == 1`（首次尝试即耗尽预算 / 无重试面），或序列各次计数全为 `0`（非 cargo 构建形态，如 Node / 链接器报错）
- **THEN** `fault` 文本 MUST NOT 含「重试进展」行；其余归因段（占用者 / 已重试次数 / 诊断摘要 / 指引）照旧装配

#### Scenario: 瞬态耗尽不落 execution_error

- **WHEN** test-execution 某 suite 命令连续 5 次命中瞬态签名并耗尽重试
- **THEN** `TestExecutionOutcome.fault == Some(Environment(…))`；该 plan 的 `execution_error` 为空（不把环境面写成「被测代码问题」），`conclusion` 值不构成门禁结论

#### Scenario: 解析失败仍为 Error 且无 fault

- **WHEN** suite 命令退出 0 但结果工件缺失 / 不可解析
- **THEN** `conclusion == Error`、`fault == None`（真环境面：工件解析 / harness），既有 `execution_error` 归因链不变

#### Scenario: mutation 不接重试且只映射取消

- **WHEN** 审查 `crates/infra/checks/src/mutation/` 的 spawn 调用与 `SegmentFailure` 映射
- **THEN** 三段 spawn 均以 `RetryPolicy::none()` 语义执行（零重试面）；`Cancelled → fault = Stopped`、`Interrupted`（含段超时）→ `fault = None` + `conclusion = Error`、`Fatal` → `Err`


## ADDED Requirements

### Requirement: test-execution 跨 plan 总预算

test-execution runner 的 `execute()` 层 SHALL 设**跨 plan 总预算**（代码常量 `TESTEXEC_TOTAL_BUDGET = 2400 s`，MUST NOT 进 config schema）：起点 SHALL 为本次执行开始时刻（与 `started_at` 同一次取时 / 同量级），**进入每个 plan 之前**判定一次——`elapsed >= TESTEXEC_TOTAL_BUDGET` 时余下 plan MUST NOT spawn（含首个 plan 的边界：`elapsed ≈ 0` 恒不触发）。

超预算的收口 SHALL 沿既有 fault 路径（MUST NOT 新写分支）：置 `fault = Some(RunFault::Environment("测试执行总时长超预算（2400 秒）…"))` 并提前 break，整体以 `fault_outcome` 返回——不写 summary、不跑诊断、不落报告文件（半套报告不进复用门）、`report_dir` 与 `exec_id` 照旧回填；已完成 plan 的结果随 fault 收口一并丢弃（与既有 fault 收口同口径，MUST NOT 引入「部分结果落盘」的第三条路径）。既有 fault 聚合规则 MUST NOT 因预算面改变：当前已有 fault 时 SHALL 保留既有 fault（`Stopped` 优先于 `Environment`），预算不覆盖它。

#### Scenario: 多 plan 总时长有上界

- **WHEN** tests[] 声明三个 suite，前两个各耗时 900 s 且均通过（含各自的瞬态重试），进入第三个 plan 时累计已超 2400 s
- **THEN** 第三个 plan 零 spawn，整体以 `fault == Some(Environment(总时长超预算文案))` 收口；`reports/test` 树无本次 summary（半套报告不进复用门），`report_dir` / `exec_id` 仍回填

#### Scenario: 预算内零行为变化

- **WHEN** 本次执行的各 plan 累计耗时未触及 2400 s（常规 1 rust + 1 vite 形态）
- **THEN** 逐 plan 照常执行、聚合与报告写盘零变化；`fault == None`、结论与 count 与既有逐字节一致

#### Scenario: 预算不覆盖既有 fault

- **WHEN** 某 plan 已产出 `fault = Stopped`（用户停止）后执行循环 break（或已记 `Environment` fault）后累计耗时亦超预算
- **THEN** 最终 fault 仍为既有那个（`Stopped` 优先），MUST NOT 被总预算文案覆盖

### Requirement: 覆盖率采集完整性（缺测不假通过）

汇总报告组装 SHALL 判定**覆盖率采集完整性**：任一子报告「自身结论非 pass」（`exit_code != 0` 或 `summary.failed > 0` 或 `coverage.pass == false`）且 `coverage == null`（覆盖率采集缺失）在场时——

- 该 suite MUST NOT 被静默跳过加权：SHALL 为其追加一条 `Problem { problem_type: ExecutionError, framework: <该 suite 框架> }`（消息说明该 suite 覆盖率采集缺失），每 suite 至多一条，且与该 suite 已有问题**去重**（同 suite 已因非零退出落 `execution_error` 时不重复追加）；`determine_conclusion` 既有映射（`has_execution_error → Error`）零改动；
- 聚合 `coverage` 块 MUST NOT 为 `pass == true`：采集不完整时 SHALL 收敛为 `None`（与「无任何实测数据 → None」同源的不可测语义）——部分 suite 的达标数值 MUST NOT 被呈现为整体通过。

「全部非 pass 子报告均有覆盖率」或「无任何非 pass 子报告」或「全部子报告均无覆盖率（本就 `None`）」三形态 SHALL 维持既有加权与判定口径零变。覆盖率数值口径（加权算法 / 阈值 / override）与 `problems` 的其余归并规则 MUST NOT 变化。

#### Scenario: 缺测 suite 不再报覆盖率通过

- **WHEN** rust suite 以非零退出收场且 `coverage == null`（llvm-cov 未产工件），同时 vite-plus suite 覆盖率达标（lines 96.21）
- **THEN** `summary.conclusion == Error`（execution_error 归因在位）、`summary.coverage == None`（MUST NOT 为 `Some(pass: true)`）；`problems` 含该 rust suite 的覆盖率缺失归因

#### Scenario: 去重不重复归因

- **WHEN** 某 suite 非零退出且 `summary.failed == 0`、同时 `coverage == null`（既有 `collect_problems` 已为该 suite 落一条 `execution_error`）
- **THEN** 该 suite 的 `execution_error` 条目仍只有一条（覆盖率缺失归因不追加第二条），消息面不改写既有条目

#### Scenario: 全测齐全时判定零变

- **WHEN** 全部 suite 均产出覆盖率且达标（真件回归形态）
- **THEN** 聚合 `coverage` 为 `Some(pass: true)`、`conclusion` 与既有实现逐字节一致，`problems` MUST NOT 新增条目

#### Scenario: 全无覆盖率时口径不变

- **WHEN** 全部子报告均无覆盖率数据（`coverage == None`，含结论非 pass 的 suite）
- **THEN** 聚合 `coverage` 仍为 `None`（既有口径），`conclusion` 由 `failed` / `execution_error` 定型；既有 `execution_error` 归因条目逐字节不变

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/checks/src/environment.rs` | 环境文案装配（单点） | `fault_text(attempts: u32, exit_code: i32, output: &[u8], retried: &[AttemptTrace]) -> String`——归因段 + 「已自动重试 N 次」+「重试进展：待编译 c1 → … → cN」+ 末次诊断摘要 + 处置指引；`attempts == 1` 或计数全 `0` 时无推进序列行；`is_progress_stalled` 为真时附卡死判定语；`attempts_log` 零改动 |
| `crates/infra/checks/src/static_check.rs` | 调用点 | `fault_text(run.attempts, run.exit_code, &run.stdout, &run.retried)`（`2>&1` 交错流不变） |
| `crates/infra/checks/src/testexec/runner.rs` | 调用点 | `fault_text(run.attempts, run.exit_code, &merged, &run.retried)`（合并流「stdout 前缀 + stderr 后缀」不变） |
| `crates/infra/checks/src/testexec/mod.rs` | 跨 plan 总预算 | `TESTEXEC_TOTAL_BUDGET: Duration = 2400 s`（代码常量）；`execute()` 记录起点、进入每 plan 前判定、超预算置 `Environment` fault 并走既有 `fault_outcome`（不写 summary / 不进复用门）；既有 fault 聚合优先级不变 |
| `crates/core/checks/src/aggregate.rs` | 覆盖聚合完整性 | 汇总组装检出「非 pass 且 `coverage == None`」子报告 → 追加去重后的 `ProblemType::ExecutionError`；采集不完整时 `aggregate_coverage` 收敛 `None`（MUST NOT `pass == true`）；`determine_conclusion` / 加权算法 / 阈值 / override 零改动 |
