# 任务: static-check-file-lock

> 覆盖 proposal「变更范围 - 实现文件」9 项，全部为**修改**（零新增文件、零新增依赖、零新 IPC、零 db 迁移、零 config schema 扩展、零 `plugins/**` 改动、零前端改动）= `crates/infra/exec/src/{transient,run,lib}.rs` + `crates/infra/checks/src/{environment,static_check}.rs` + `crates/infra/checks/src/testexec/{runner,mod}.rs` + `crates/core/checks/src/aggregate.rs` + `packages/desktop/package.json`。**本相位不改测试文件**：`transient_test.rs` / `run_test.rs` / `lib_test.rs` / `environment_test.rs` / `static_check_test.rs` / `testexec/{runner,mod}_test.rs` / `aggregate_test.rs` 的用例新增与改写全部归 test-design / test-gen / test-execution 相位（承接清单见阶段七）。
>
> 依赖序要点：**阶段一**（推进判据 + 签名 / 退避常量 + 导出面）解锁**阶段二**（`run.rs` 卡死早停）与**阶段三**（检查域文案取数）；阶段三把 `fault_text` 扩参与**两个调用点**放在同一阶段一次改齐——若拆开，工作区会在两阶段之间不通过编译（`pub(crate)` 函数签名与调用点必须原子变更）。阶段四（跨 plan 预算）与阶段五（覆盖率采集完整性）互不依赖、也与一/二/三互不依赖，可并行。阶段六（版本面）在语义落地后执行。
>
> 不预铺：不动 `RunOptions` / `RunReport` / `RunOutcome` / `RunError` 字段面，不动三 runner trait 签名与 `RunFault` 语义，不动 `attempts_log`（record 形态）、`diagnostics`、合并流顺序、`report_dir` / `exec_id` 回填、复用门、mutation 分流。

## 阶段一：exec 推进判据与策略常量（`transient.rs` + `lib.rs`）

- [x] 修改 `crates/infra/exec/src/transient.rs` 的签名表：第 6 条 `"failed to remove file"` → `"failed to remove"`（`const TRANSIENT_SIGNATURES: [&str; 13]` 长度不变、其余 12 条逐字不变），表上方注释补一句「生产文本为 `failed to remove <path>`（不含 `file`）；该短语只出现在删除失败语境，收窄风险低；旧条目永不命中、功能上被同行 `os error 32` 兜住」
- [x] 同文件把 `RETRY_BACKOFF` 由 `[1, 2, 4, 8] s` 改为 `[5, 10, 20, 40] s` 并重写其理由注释（实测静默窗口可达分钟级；「只剩 rename 收尾」（待编译计数已归零）的尝试最需要耐心；最坏 5 次共 75 s 仍远低于 `RETRY_TOTAL_BUDGET = 1500 s` 硬顶；成功路径零影响）；`RETRY_MAX_ATTEMPTS`（5）与 `RETRY_TOTAL_BUDGET`（1500 s）**零改**
- [x] 同文件同步 `RetryPolicy::transient()` 的 doc（`backoff = [5, 10, 20, 40] s`）；`none()` 的 doc 中「退避取同类常量值（不消费）」表述保持成立（值随常量走，不改措辞）
- [x] 同文件新增 `pub fn compiling_count(stdout: &[u8], stderr: &[u8]) -> u32`：合并 `stdout ++ stderr` → `String::from_utf8_lossy` → `.lines()` 逐行 → `line.trim_start().starts_with("Compiling ")`（大小写**敏感**，含尾随空格）计数；无该字样 → `0`；doc 写明「计数口径单点：rustc / cargo 原文形态；非 cargo 构建形态退化为固定次数语义；零 IO 零 spawn 零 panic」
- [x] 同文件新增 `pub fn progress_series(report: &RunReport) -> Vec<u32>`：`report.retried` 按序 `map(|trace| compiling_count(&trace.stdout, &trace.stderr))` → 末尾 `push(compiling_count(&report.stdout, &report.stderr))`；doc 写明「长度 = `retried.len() + 1`，生产不变量 `retried.len() + 1 == attempts` 下等于 `attempts`；未重试即长度 1」
- [x] 同文件新增 `pub fn is_progress_stalled(series: &[u32]) -> bool`：`split_last()` 取末值与前值；`len < 2` → `false`；否则 `last > 0 && last >= previous`；doc 写明「判据只用于提前放弃，MUST NOT 用于延长预算；`attempts == 1` 无前值不触发；末值 `0`（只剩收尾）恒不触发」
- [x] 同文件模块 doc 补一条纪律：「增量推进判据（`compiling_count` / `progress_series` / `is_progress_stalled`）单点驻本模块，`run.rs` 与检查域各只调用、MUST NOT 重算计数口径」
- [x] 修改 `crates/infra/exec/src/lib.rs`：`pub use transient::{compiling_count, is_progress_stalled, progress_series, is_transient, AttemptTrace, RetryPolicy};`（原行为 `is_transient, AttemptTrace, RetryPolicy`）；头部模块 doc 的「瞬态签名与有限重试」条补「含增量推进判据（`Compiling` 行计数 / 序列 / 卡死判定）」
- [x] 自查 阶段一：`grep -n "failed to remove" crates/infra/exec/src/transient.rs` 仅命中 `failed to remove`（**零** `failed to remove file`）；`grep -n "from_secs(1)\|from_secs(2)\|from_secs(4)\|from_secs(8)" crates/infra/exec/src/transient.rs` 零命中；三个新函数内 `tokio` / `std::fs` / `std::process` 零命中（纯函数）；`grep -n "pub use transient" crates/infra/exec/src/lib.rs` 含三项新名字

## 阶段二：exec 卡死早停（`run.rs`，依赖阶段一）

- [x] 修改 `crates/infra/exec/src/run.rs` 的导入：`use crate::transient::{compiling_count, is_progress_stalled, transient_signature, AttemptTrace, RetryPolicy};`
- [x] 同文件 `run_shell` 循环前增 `let mut counts: Vec<u32> = Vec::new();`（注释：按尝试序的待编译计数序列；只用于早停判定，**不进 `RunReport`**），每次 `attempt_once(..)` 返回后**立即** `counts.push(compiling_count(&attempt.stdout, &attempt.stderr));`（放在三臂判定之前，保证每次尝试都记账）
- [x] 同文件在「命中签名（`transient_signature` 为 `Some`）」之后、「上限 / 预算早停」之前插入新判定臂：`if is_progress_stalled(&counts) { return Ok(attempt.into_report(attempts, retried)); }`（注释：末两次计数非降且末值 > 0 = 同一批写不进去 → 提前放弃，不烧满 `max_attempts`；末次不入 `retried`）
- [x] 同文件改写 `run_shell` doc 的循环判定序为「①非 `Completed`（`TimedOut` / `Cancelled`）即返 → ②未命中签名即返（真红 / 绿）→ ③卡死判否（`is_progress_stalled`）即返末次 → ④达尝试上限即返末次 → ⑤总预算耗尽即返末次 → ⑥退避（同样观测 `cancel`）→ ⑦入 `retried`」，并写明「卡死只提前放弃、MUST NOT 延长预算；`attempts == 1` 无前值不触发；末次计数 `0` 不触发」
- [x] 同文件模块 doc 第五能力项「有限重试」补一句「重试间按 `Compiling` 行计数判推进 / 卡死；推进序列的取数面为 `crate::transient::progress_series`」
- [x] 自查 阶段二：`grep -n "is_progress_stalled" crates/infra/exec/src/run.rs` 恰 2 处（导入 + 判定臂）；判定臂位于 `transient_signature(..)` 的 `else` 早返之后、`attempts >= retry.max_attempts` 之前；`grep -n "counts" crates/infra/exec/src/run.rs` 仅 3 处（声明 / push / 判定）；`grep -rn "compiling_count" crates --include=*.rs | grep -v _test` 命中 `transient.rs`（定义 + `progress_series` 内）与 `run.rs`（导入 + push）——检查域零命中（口径单点）

## 阶段三：检查域「重试进展」文案（`environment.rs` + 两调用点，依赖阶段一）

- [x] 修改 `crates/infra/checks/src/environment.rs` 的导入：`use exec::{AttemptTrace, ProcessOutput, RetryPolicy, RunOutcome, RunReport};`
- [x] 同文件 `fault_text` 签名扩为 `pub(crate) fn fault_text(attempts: u32, exit_code: i32, output: &[u8], retried: &[AttemptTrace]) -> String`，并在「已自动重试 N 次（每次重试会跳过已完成部分）」行之后、「最后一次诊断：」之前插入：`if let Some(line) = progress_line(&series) { text.push('\n'); text.push_str(&line); }`（`series` 取数见下一条；归因段 / 次数行 / 摘要段 / `ENV_GUIDANCE` 逐字不动）
- [x] 同文件在 `fault_text` 内构造「视图报告」取序列（检查域零计数逻辑）：`let series = exec::progress_series(&RunReport { outcome: RunOutcome::Completed, exit_code, stdout: output.to_vec(), stderr: Vec::new(), attempts, retried: retried.to_vec() });`，并加注释「test-execution 侧传入的已是 `stdout ++ stderr` 合并流，故末次只填 `stdout`、`stderr` 留空（否则同一段字节被计两次）；`outcome` 只为构造视图，计数不看它」
- [x] 同文件新增私有 `fn progress_line(series: &[u32]) -> Option<String>`：`series.len() < 2` → `None`（`attempts == 1` 未重试）；`series.iter().all(|count| *count == 0)` → `None`（非 cargo 构建形态）；否则 `format!("重试进展：待编译 {}", series.iter().map(u32::to_string).collect::<Vec<_>>().join(" → "))`，`exec::is_progress_stalled(series)` 为真时追加 `（末两次无进展，判定为卡死）`
- [x] 同文件 `fault_text` doc 补「`retried` 此前各次尝试的留痕（末次在 `output` 里）；推进序列行长度 `O(attempts)`（至多 5 个数字）」；模块 doc 的两个职责段补第三项「[`progress_line`]：重试进展行（计数口径单点驻 `exec::transient`，本模块不重算）」
- [x] 修改 `crates/infra/checks/src/static_check.rs`：`fault_text` 调用补第四参 `&run.retried`（`run.attempts` / `run.exit_code` / `&run.stdout` 三者不变）
- [x] 修改 `crates/infra/checks/src/testexec/runner.rs`：`fault_text` 调用补第四参 `&run.retried`（`run.attempts` / `run.exit_code` / `&merged` 三者不变，合并流「stdout 前缀 + stderr 后缀」零变）
- [x] 自查 阶段三：`grep -rn "fault_text(" crates --include=*.rs | grep -v _test` 恰 3 处（定义 + 两调用点），两调用点均含 4 个实参且第四参为 `&run.retried`；`grep -n "Compiling " crates/infra/checks/src/environment.rs` 零命中（检查域不重算计数）；`grep -n "attempts_log" crates/infra/checks/src/environment.rs` 段内零字节改动；`grep -n "2>&1" crates/infra/checks/src/static_check.rs` 仍恰一处

## 阶段四：test-execution 跨 plan 总预算（`testexec/mod.rs`）

- [x] 修改 `crates/infra/checks/src/testexec/mod.rs` 的常量组：新增 `const TESTEXEC_TOTAL_BUDGET: Duration = Duration::from_secs(2400);`（doc：单 plan 预算 1200 s × 2 有余；当前形态（1 rust + 1 vite）实测最坏 ≈ 1575 s，留约 1.5 倍余量；代码常量 MUST NOT 进 config schema，suite 数量增长时复核），并补 `use std::time::{Duration, Instant};`
- [x] 同文件 `ProcessTestExecution::run`（`impl TestExecutionRunner::run` 的 `Box::pin` 体）改为 `execute(&root, &change, slot, cancel, TESTEXEC_TOTAL_BUDGET)`
- [x] 同文件 `execute` 签名增第五参 `total_budget: Duration`，在 `let started_at = OffsetDateTime::now_utc();` 紧邻处取 `let budget_started = Instant::now();`（注释：跨 plan 总预算起点，与 `started_at` 同量级）
- [x] 同文件逐 plan 循环改 `for (index, plan) in plans.iter().enumerate()`，循环体首行判定：`if budget_started.elapsed() >= total_budget { if fault.is_none() { fault = Some(RunFault::Environment(format!("测试执行总时长超预算（{} 秒）：余下 {} 个 suite 未执行", total_budget.as_secs(), plans.len() - index))); } break; }`（注释：余下 plan MUST NOT spawn；`fault.is_none()` 守卫使既有 `Stopped` / `Environment` 不被覆盖；首个 plan 边界 `elapsed ≈ 0` 恒不触发）
- [x] 同文件 `execute` doc 补一段「跨 plan 总预算」：进入每个 plan 之前判定一次；超预算沿**既有** `fault_outcome` 路径收口（不写 summary、不跑 `diagnose_findings`、不落报告文件、半套报告不进复用门；`report_dir` / `exec_id` 照旧回填）；已完成 plan 的结果随 fault 收口一并丢弃（工件仍在报告树可人工回查）
- [x] 自查 阶段四：`grep -n "TESTEXEC_TOTAL_BUDGET" crates/infra/checks/src/testexec/mod.rs` 恰 2 处（定义 + `run` 实参）；`grep -n "budget_started" crates/infra/checks/src/testexec/mod.rs` 恰 2 处（取时 + 判定）；`grep -n "fault_outcome" crates/infra/checks/src/testexec/mod.rs` 仍为定义 + 单一调用点（**未新增第三条收口路径**）；`grep -n "summary" crates/infra/checks/src/testexec/mod.rs` 在预算分支内零命中

## 阶段五：覆盖率采集完整性（`core/checks/src/aggregate.rs`）

- [x] 修改 `crates/core/checks/src/aggregate.rs`，在 problems 归并段（`collect_problems` 之前）新增私有判据：`fn misses_coverage(report: &SubReport) -> bool`——`let non_pass = report.exit_code != 0 || report.summary.failed > 0 || report.coverage.as_ref().is_some_and(|block| !block.pass);` → `non_pass && report.coverage.is_none()`（doc：采集完整性判据单点——归因（`collect_problems`）与收敛（`aggregate_coverage`）共用，MUST NOT 各写一份）
- [x] 同文件 `aggregate_coverage` 函数体首行插 `if sub_reports.iter().any(misses_coverage) { return None; }`，doc 补「采集不完整（任一 suite 结论非 pass 且无覆盖率块）→ `None`（与「无实测数据 → `None`」同源）：部分 suite 的达标数值 MUST NOT 被呈现为整体通过」；`weighted_measured` / `compute_overrides` / 阈值与 pass 计算**零改**
- [x] 同文件 `collect_problems` 逐 report 循环增局部 `let mut has_execution_error = false;`：既有两臂（`exit_code != 0 && failed == 0`、`failed == 0 && findings 非空`）命中即置 `true`；两臂之后追加 `if !has_execution_error && misses_coverage(report) { problems.push(Problem { framework: report.framework.clone(), problem_type: ProblemType::ExecutionError, message: format!("覆盖率采集缺失（退出码 {}）：该 suite 结论非通过且未产出覆盖率数据", report.exit_code) }); }`（每 suite 至多一条、与既有 `execution_error` 去重、既有条目消息面零改写）
- [x] 同文件 `collect_problems` doc 补「含覆盖率采集缺失归因（`ProblemType::ExecutionError`，每 suite 至多一条，与既有 `execution_error` 去重）；`determine_conclusion` 的 `has_execution_error → Error` 映射零改」
- [x] 自查 阶段五：`grep -n "misses_coverage" crates/core/checks/src/aggregate.rs` 恰 3 处（定义 + `aggregate_coverage` + `collect_problems`）；`grep -n "determine_conclusion\|coverage_meets_thresholds\|format_coverage_failure" crates/core/checks/src/aggregate.rs` 三函数体零改（签名的调用点不变）；`grep -n "has_execution_error" crates/core/checks/src/aggregate.rs` 含 `collect_problems` 的局部标记与 `build_summary_report` 的既有取值（后者零改）

## 阶段六：版本面

- [x] 修改 `packages/desktop/package.json`：`version` `"0.5.28"` → `"0.5.29"`（单行；`tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 与 `plugins/**` **不**随动）

## 阶段七：边界与语义自查（grep / diff 核对）

- [x] 核对 `git status` 仅含 9 项实现文件（全为修改，见 design「修改文件」）——`crates/infra/exec/src/{output,shell,lockprobe}.rs`、`crates/core/orchestration/**`、`crates/core/foundation/**`、`crates/core/checks` 其余文件、`crates/infra/checks/src/{lib.rs,mutation/**}`、`crates/infra/checks/src/testexec/detect.rs`、`packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`packages/desktop/src-tauri/src/main.rs`、`plugins/**` 全部零改动
- [x] 确认执行体契约零变：`grep -n "pub struct RunOptions\|pub struct RunReport\|pub enum RunOutcome\|pub enum RunError" -A 24 crates/infra/exec/src/run.rs` 字段集与改动前逐字一致（`RunReport` 无 `counts` / 无序列字段）；`RunOptions` 仍无 `Default`；`attempt_once` / `Race` / `await_exit` / `drain` / `wait_cancel` / `CANCEL_POLL_INTERVAL` / `FINISH_GRACE` 零字节改动
- [x] 确认判定序未被破坏：`grep -n "RunOutcome::Completed" crates/infra/exec/src/run.rs` 复核「非 `Completed` 即返」仍为循环第一臂；`TimedOut` / `Cancelled` 零重试面与退避期取消收口（`Cancelled` + `attempts` 记已完成次数）逐字不变
- [x] 确认口径单点：计数只出现在 `crates/infra/exec/src/transient.rs`（`compiling_count`）；`grep -rn "Compiling " crates --include=*.rs | grep -v _test` 仅命中 `transient.rs` 的判定处与 doc
- [x] 确认 fault 面零变：`grep -rn "RunFault" crates/core/orchestration/src/port.rs` 变体集仍 `Stopped` / `Environment(String)`；`grep -rn "TXT_ENV_" crates/core/orchestration/src/walker.rs` 模板零改（推进序列经 `{fault}` 嵌入，模板不新增行）；`grep -rn "RunFault" packages/desktop/src/types/generated/bindings.ts` 零命中（`bindings:check` 预期零 diff，不由本相位执行）
- [x] 确认记录面零变：`grep -n "===== attempt" crates/infra/checks/src/environment.rs` 仍仅 `push_attempt` 一处；`grep -n "attempts_log" crates/infra/checks/src/{static_check.rs,testexec/runner.rs}` 各一处且未改；`exec-outputs` 落点与 `exec_id` 回填零改
- [x] 确认死代码面：三处新 `pub` 函数均有生产调用点（`compiling_count` ← `progress_series` / `run.rs`；`progress_series` ← `environment.rs`；`is_progress_stalled` ← `run.rs` / `environment.rs`）；`misses_coverage` / `progress_line` 为私有且各有 ≥1 生产调用（不新增仅供测试的 `export`——AGENT.md 纪律）
- [x] 登记测试相位承接清单（不在本相位执行，逐项与 proposal「变更范围 - 测试文件」对齐）：① `transient_test.rs`——三新函数形态（多行 / 前导空白 / 无 `Compiling ` → 0；序列长度与序；卡死判据 `len < 2` / 相等 / 上升 / 末值 0 / 下降），签名表 remove 用例改写（旧 `failed to remove file` 断言作废）、13 条封闭表计数不变、`transient()` 退避常量锚定改 `[5, 10, 20, 40]`；② `run_test.rs`——卡死 shim（`attempts == 2 < max_attempts`）、递减序列（264 → 5 → 0 跑到底）、末值归零不早停、退避逐段用例改取 `RetryPolicy::transient().backoff`（1 s / 2 s 硬编码断言作废，`瞬态自愈_三次尝试后转绿` 的 `transient()` 墙钟随 5 s / 10 s 上升）、`fast_policy` 保留；③ `lib_test.rs`——退避四段再导出断言改值；④ `environment_test.rs`——7 处 `fault_text(..)` 调用补 `retried`（`&[]` / 字面量 `AttemptTrace`），新增推进序列两臂（含序列 / 卡死附语 / `attempts == 1` 无行 / 全 0 无行）；⑤ `static_check_test.rs` / `testexec/runner_test.rs`——瞬态耗尽面 fault 文本补推进序列断言；⑥ `testexec/mod_test.rs`——预算用例（`super::execute(.., 毫秒级预算)`）零 spawn / `Environment(总时长超预算)` / 无 summary / 预算不覆盖既有 fault，加常量锚定；⑦ `aggregate_test.rs`——缺测归因 / 去重 / 齐全零变三臂，**并改写既有 `汇总组装逐字段相等`**（vite-plus `failed = 1` + `coverage = None` 在新判据下会多一条 `execution_error`、`coverage` 收敛 `None`、`conclusion` 升 `Error`，见 design 待决问题 1）
- [x] 把待决问题 1（P6 连带语料改写）、2（`fault_text` 视图报告 vs `progress_series_of` 备选）、4（`execute` 第五参的可测性）登记给 test-design / 评审，不阻塞实现；`AC-9` 的现场回归（冷 target 自愈 + 终局文案可读）登记为交付后人工验收项
