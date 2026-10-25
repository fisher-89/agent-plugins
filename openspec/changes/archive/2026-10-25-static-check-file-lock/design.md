# 设计: static-check-file-lock

> **变更**: static-check-file-lock
> **日期**: 2026-10-23
> **基于**: `proposal.md`、`explore.md` 与 `specs/{desktop-exec-runtime,desktop-checks-domain}/spec.md`
> **前置**: `rust-suite-single-build` / `exec-run-shell` / `lock-fault-diagnostics` 三篇已合入——单构建模板、`exec::run_shell` 四能力执行体、瞬态重试、锁探针归因、`RunFault` 分派均在位。本篇只在既有落位内**补重试质量与覆盖率口径的六处缺口**：零新模块、零新依赖、零新 IPC、零 port 签名变更、零 fault 语义变更。

---

## 提案与规格同步状态

proposal 的六处补丁（P1–P6）与两份 delta spec 已定稿，本设计只做四件事：逐文件落定实现形态（含规格未定死、必须在此收敛的成形细节：卡死判定在重试循环内的**位置**、`fault_text` 如何在不重算计数口径的前提下拿到序列、`TESTEXEC_TOTAL_BUDGET` 的可测形态、覆盖率采集完整性判据的单点归属）；给出三处新增纯函数与一处签名扩参的精确契约；把 P6 的连带语义变化（`conclusion` 从 `Fail` 升 `Error`、`coverage` 从 `Some(pass:true)` 收敛 `None`）显式留痕；按 AC-1..AC-10 逐条对齐。

起点核实（工作树实况；路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）：

- **瞬态面**：`crates/infra/exec/src/transient.rs:21` 封闭表 `TRANSIENT_SIGNATURES: [&str; 13]`（`failed to remove file` 在 `:28`——**生产文本不含 `file`，该条永不命中**）；`:46` `RETRY_BACKOFF = [1, 2, 4, 8] s`；`:56` `RETRY_TOTAL_BUDGET = 1500 s`；`:62-92` `RetryPolicy`（两构造，字段 `pub`）；`:96-106` `AttemptTrace`；`:114-130` `transient_signature`（`pub(crate)` 核心）；`:134` `is_transient`。
- **重试循环**：`crates/infra/exec/src/run.rs:136-196`——判定序「①非 `Completed` 即返（`:153`）→ ②未命中签名即返（`:157-164`）→ ③上限 / ④预算早停（`:167`）→ ⑤退避（`:171-188`，退避期取消收口 `:180`）→ ⑥入 `retried`（`:189`）」；`RunReport`（`:88-104`，`stdout`/`stderr` 为末次）字段面本次零变更。
- **导出面**：`crates/infra/exec/src/lib.rs:31` `pub use transient::{is_transient, AttemptTrace, RetryPolicy};`。
- **环境文案**：`crates/infra/checks/src/environment.rs:91-117` `fault_text(attempts, exit_code, output) -> String`——归因段（`:93`）→「已自动重试 N 次」（`:106-109`）→「最后一次诊断」（`:110-113`）→ `ENV_GUIDANCE`（`:114-115`）；`attempts_log`（`:34-63`）本次零改。
- **两个调用点**：`crates/infra/checks/src/static_check.rs:100-113`（`2>&1` 交错流，`&run.stdout`）与 `crates/infra/checks/src/testexec/runner.rs:119-136`（`stdout ++ stderr` 合并流）——`fault_text(..)` 各一处在 `:102` / `:130`。
- **test-execution 编排**：`crates/infra/checks/src/testexec/mod.rs:69-75` `ProcessTestExecution::run` → `:85-187` `execute(root, change, slot, cancel)`（`started_at` 在 `:91`）；逐 plan 循环 `:138-153`；fault 收口 `:158-167`；`fault_outcome`（`:230-250`）已回填 `report_dir` / `exec_id` 且**不写 summary / 不跑诊断 / 不落报告**——P5 沿此路径，零新分支。
- **覆盖率聚合**：`crates/core/checks/src/aggregate.rs:105-129` `aggregate_coverage`；`:234-252` `weighted_measured`（`has_any` 短路 = 提案 P6 的缺口所在）；`:429-478` `collect_problems`（execution_error 两臂：`exit_code != 0 && failed == 0`、`failed == 0 && findings 非空`）；`:574-650` `build_summary_report`（`:587` 归并 problems、`:588` 聚合 coverage、`:609-617` `has_execution_error → Error` 判定）。
- **历史实证复现确认**：`aggregate_test.rs:597-604` 的既存语料（vite-plus：`failed = 1`、`coverage = None`，node-test 绿）正是「结论非 pass 且无覆盖率」形态——新判据落地后该语料的 `problems` 会多一条 `execution_error`、`coverage` 由 `Some(pass:true)` 收敛 `None`、`conclusion` 由 `Fail` 升 `Error`（见 D14 与待决问题 1）。
- **版本面**：`packages/desktop/package.json:3` = `0.5.28`。
- **`RunOptions` 构造点四实量**：`checks/src/static_check.rs:83`、`checks/src/testexec/runner.rs:98`、`checks/src/mutation/{cargo_mutants.rs:41,stryker.rs:68}`——本次 `RunOptions` 字段面零变更，四处**零改动**。

**只读红线零触点**：`crates/infra/exec/src/{output,shell,lockprobe}.rs` 与 `kill_tree` / `ProcessOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` 本体、`RunOptions` / `RunReport` / `RunOutcome` / `RunError` 字段面、`crates/infra/checks/src/mutation/**`、`crates/core/orchestration/**`、`crates/core/foundation/**`、`crates/core/checks` 其余纯层、前端 `packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`packages/desktop/src-tauri/src/main.rs` 的 `DATA_DIR`（P7 未决）、`plugins/**`。

---

## 六处补丁 → 落点

| # | 补丁 | 落点 | 关键决策 |
|---|------|------|----------|
| P1 | 增量推进判据 + 卡死早停 | `transient.rs`（三函数）、`run.rs`（循环内早停）、`lib.rs`（导出） | D1 / D2 / D3 / D4 |
| P2 | 退避上调 `[5, 10, 20, 40] s` | `transient.rs`（一个常量） | D5 |
| P3 | 环境文案补「推进序列」 | `environment.rs`（`fault_text` 扩参）、`static_check.rs` / `testexec/runner.rs`（调用点各加一参） | D6 / D7 |
| P4 | 签名修正 `failed to remove file` → `failed to remove` | `transient.rs`（表内一个字符串） | D8 |
| P5 | 跨 plan 总预算 `2400 s` | `testexec/mod.rs`（常量 + 进 plan 前判定，沿既有 `fault_outcome`） | D9 / D10 |
| P6 | 覆盖率采集完整性 | `core/checks/src/aggregate.rs`（判据单点 + problems 追加 + 聚合收敛 `None`） | D11 / D12 / D13 |

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 计数单点与三函数形态 | `crates/infra/exec/src/transient.rs` 内三个**纯函数**（零 IO / 零 spawn / 零 async）：`pub fn compiling_count(stdout: &[u8], stderr: &[u8]) -> u32`（合并 `stdout ++ stderr` → `String::from_utf8_lossy` → `.lines()` 逐行 → `line.trim_start().starts_with("Compiling ")` 计数；`lines()` 天然吃掉 CRLF 的 `\r`，与 Rust `str::lines` 既有口径一致）、`pub fn progress_series(report: &RunReport) -> Vec<u32>`、`pub fn is_progress_stalled(series: &[u32]) -> bool`。三函数均**不**碰 `RunReport` 字段语义、不新增字段 | spec「计数与卡死判据单点驻 `exec::transient`（纯函数）」「检查域 MUST NOT 重算计数口径」：把「什么是一个待编译单位」这条易漂移的口径钉在一处；`lines()` 与既有 `transient_signature` 的「整串包含」不冲突（签名判定未变，计数是新面） |
| D2 | `progress_series` 的长度口径 | `report.retried` 按序在前（各取 `compiling_count(&trace.stdout, &trace.stderr)`）+ 末次一条（`compiling_count(&report.stdout, &report.stderr)`）→ 长度 = `retried.len() + 1`。生产不变量 `retried.len() + 1 == attempts`（`run.rs` 的入栈动作在退避成功之后）使长度等于 `attempts`；未重试即长度 1 | spec「长度等于 `attempts`」在全部生产路径上成立（含入口取消短路 `attempts = 1 && retried 空`、上限 / 预算 / 卡死早停「末次不入 `retried`」、退避期取消 `attempts` = 已完成次数）；直构报告（测试语料）可能违反不变量，故 doc 明写「长度 = `retried.len() + 1`，生产不变量下等于 `attempts`」 |
| D3 | `is_progress_stalled` 的判据边界 | `split_last()` 取末值与前值：`series.len() < 2` → `false`；否则 `last > 0 && last >= previous`。即「末两次**非降**且末值 `> 0`」——相等（`50 → 50`）与上升（`5 → 9`）都判卡死，末值 `0`（只剩 rename / 清理收尾）恒判否 | spec 逐字；「非降」而非「相等」覆盖「新增 crate 数抵消已完成 crate 数」的上升形态（R-1 的对照组）；末值 `0` 不早停是「最该再试的一档」 |
| D4 | 卡死早停的**位置**与「只用于提前放弃」 | 插在 `run.rs` 循环判定序的**②之后、③之前**：`counts.push(compiling_count(&attempt.stdout, &attempt.stderr))` 在**每次尝试产出后立即**执行（先于三臂判定），随后「①非 `Completed` 即返 → ②未命中签名即返 → **③卡死判否即返末次结果** → ④上限 / ⑤预算早停 → ⑥退避 → ⑦入 `retried`」。`counts: Vec<u32>` 为循环局部变量（**不进 `RunReport`**）；早停返回 `attempt.into_report(attempts, retried)`（`Completed` + 非零退出码 + 末次字节，末次不入 `retried`）。判据**只早退**，绝不延长 `total_budget` | ①「非 `Completed`」必须最先判：`TimedOut` / `Cancelled` 零重试面逐字节不变（`attempts == 1`），且被杀两臂的输出不保证完整——先计数再判不改变任何行为，但把「计数」放进被杀的路径会误导维护者；③ 在 ② 之后才谈得上「命中签名之后」；④⑤ 与 ③ 同属「早停族」，③ 在前使「卡死」优先于「上限 / 预算」的语义可读（三者在 `attempts == max_attempts` 面同返末次，观测无差）。`counts` 不进报告：`RunReport` 字段面零变是 proposal 硬约束，读侧由 `progress_series` 反推，两处共用 `compiling_count` 故值必然一致（字节零改写零截断是前提） |
| D5 | 退避上调的取值与可见面 | `RETRY_BACKOFF` 由 `[1, 2, 4, 8] s` → `[5, 10, 20, 40] s`（`RETRY_MAX_ATTEMPTS = 5` / `RETRY_TOTAL_BUDGET = 1500 s` 零改）；`RetryPolicy::{none,transient}` 两构造的字段面与签名零改，`none()` 仍取同类常量值（不消费） | spec「退避取慢档」：实测静默窗口分钟级，秒级退避在「只剩 rename 收尾」（计数已归零）的尝试上大概率撞同一扫描窗口；成功路径零影响（未命中签名不重试），最坏多等 75 s 而 `total_budget` 硬顶不变 |
| D6 | `fault_text` 的扩参与行位 | `pub(crate) fn fault_text(attempts: u32, exit_code: i32, output: &[u8], retried: &[AttemptTrace]) -> String`——文本顺序：归因段（不变）→「已自动重试 N 次（每次重试会跳过已完成部分）」（不变）→ **新增「重试进展」行** →「最后一次诊断：+摘要」（不变）→ `ENV_GUIDANCE`（不变）。追加行由私有 `fn progress_line(series: &[u32]) -> Option<String>` 装配 | spec「在归因段之后、诊断摘要之前」+ 既有断言（次数行 / 摘要段 / 指引尾）逐字不动：把新行夹在「次数行之后、摘要之前」是唯一同时满足两条件的缝；`attempts_log` 零改（record 内容形态不变） |
| D7 | 序列取数：不重算口径 | `fault_text` 内以「视图报告」调 `exec::progress_series`：`progress_series(&RunReport { outcome: RunOutcome::Completed, exit_code, stdout: output.to_vec(), stderr: Vec::new(), attempts, retried: retried.to_vec() })`——测试执行侧传的是合并流（`stdout ++ stderr`），按 `stdout` 单路喂入与按两路喂入在 `compiling_count` 的合并口径下**等价**（末次 `stderr` 已含在 `output` 内，不得再传一遍，否则重复计数）。行文本 `重试进展：待编译 {c1} → … → {cN}`（`series.iter().map(u32::to_string).collect::<Vec<_>>().join(" → ")`）；`exec::is_progress_stalled(series)` 为真时同一行追加 `（末两次无进展，判定为卡死）` | spec「按 `exec::progress_series`…单点取数 MUST NOT 在检查域重算计数口径」：检查域零计数逻辑（连「序列怎么拼」都不重写）；代价是 fault 路径上一次 `retried` 字节 clone（仅瞬态耗尽 / 卡死收口时发生，`fault_text` 本就要为 `ProcessOutput` clone 一份末次输出），备选（给 exec 增 `progress_series_of(previous, last)` 变体）属单次使用的抽象扩面，按「简单优先」不取 |
| D8 | 推进序列行的抑制条件 | `progress_line` 返回 `None`（不输出该行）当且仅当：`series.len() < 2`（`attempts == 1`，未重试）**或** `series.iter().all(\|count\| *count == 0)`（非 cargo 构建形态）。其余情形一律输出 | spec 两条抑制条件逐字；「全 `0`」按**整条序列**判（`[264, 0]` 仍输出「重试进展：待编译 264 → 0」，因为确有推进可读） |
| D9 | 跨 plan 总预算的判定位与收口路径 | `testexec/mod.rs` 新增代码常量 `const TESTEXEC_TOTAL_BUDGET: Duration = Duration::from_secs(2400)`（doc：为何 2400 = 单 plan 1200 × 2 有余、当前形态最坏 ≈ 1575 s 留约 1.5 倍余量、MUST NOT 进 config schema）；`execute(...)` 增第五参 `total_budget: Duration`（`ProcessTestExecution::run` 传 `TESTEXEC_TOTAL_BUDGET`），与 `started_at`（`:91`）**相邻**取 `let budget_started = Instant::now();`；逐 plan 循环改 `for (index, plan) in plans.iter().enumerate()`，**进入每个 plan 之前**判定 `budget_started.elapsed() >= total_budget` → 置 fault 后 `break`（余下 plan **零 spawn**）。收口走**既有** `if let Some(fault) = fault { return Ok(fault_outcome(..)) }`（`:158-167`）——不写 summary、不跑 `diagnose_findings`、不落报告文件、`report_dir` / `exec_id` 照旧回填 | spec「沿既有 fault 路径（MUST NOT 新写分支）」：P5 不引入第三条收口路径；「已完成 plan 结果随 fault 收口一并丢弃」与既有口径同源（半套报告会被 `check_integrity` 当权威而永久复用），工件仍在报告树可人工回查 |
| D10 | 预算 fault 的文案与聚合优先级 | 文案 `format!("测试执行总时长超预算（{} 秒）：余下 {} 个 suite 未执行", total_budget.as_secs(), plans.len() - index)`（生产即「（2400 秒）」）；**仅当 `fault.is_none()` 时**才写入（`if fault.is_none() { fault = Some(RunFault::Environment(..)) }`），既有 `Stopped` / `Environment` 不被覆盖。`total_budget` 作参数而非直接读常量，唯一目的是让 `mod_test` 以毫秒级预算直驱全链（生产零配置面） | spec「预算不覆盖既有 fault（`Stopped` 优先于 `Environment`）」：`Stopped` 面已 `break`，预算判定不可达；`Environment` 面靠 `is_none()` 守卫。参数化不构成 config 扩展（`run` 之外无调用方，常量仍单点） |
| D11 | 覆盖率采集完整性判据（单点） | `aggregate.rs` 新增私有 `fn misses_coverage(report: &SubReport) -> bool`：`non_pass = report.exit_code != 0 \|\| report.summary.failed > 0 \|\| report.coverage.as_ref().is_some_and(\|block\| !block.pass)`；返回 `non_pass && report.coverage.is_none()`。`collect_problems` 与 `aggregate_coverage` **共用**该判据（不各写一份） | spec 三条件与「`coverage == null`」同时成立的合取；单点保证「归因」与「收敛」两处口径同源（否则会出现「报了缺失却不收敛」或反之） |
| D12 | problems 追加与去重形态 | `collect_problems` 的逐 report 循环改为「局部 `let mut has_execution_error = false;`」：既有两臂（`exit_code != 0 && failed == 0` / `failed == 0 && findings 非空`）命中即置 `true`；两臂之后 `if !has_execution_error && misses_coverage(report) { problems.push(Problem { framework: report.framework.clone(), problem_type: ProblemType::ExecutionError, message: format!("覆盖率采集缺失（退出码 {}）：该 suite 结论非通过且未产出覆盖率数据", report.exit_code) }) }`。每 sub-report 至多一条；既有条目的消息面**零改写** | spec「每 suite 至多一条、与该 suite 已有 `execution_error` 去重、消息面不改写既有条目」：per-report 布尔是最小去重面（既有两臂都在同一循环体内），不必引入跨 report 的 `(framework, problem_type)` 索引；`determine_conclusion` 的 `has_execution_error → Error` 映射零改 |
| D13 | 聚合 coverage 的收敛 | `aggregate_coverage` 首行插 `if sub_reports.iter().any(misses_coverage) { return None; }`（在 `weighted_measured` 之前）——采集不完整即不可测，与「无任何实测数据 → None」同源；加权算法（`weighted_measured` / `aggregate_measured`）、阈值、per-suite override 口径**零改** | spec「聚合 `coverage` 块 MUST NOT 为 `pass == true`，采集不完整时 SHALL 收敛 `None`」：`None` 是既有 schema 的合法值（`CoverageBlock` 为 `Option`），前端与 `diagnose_findings` 已有 `None` 分支，故「不可测」不引入新词汇 |
| D14 | P6 的连带语义变化（显式留痕） | 「自身结论非 pass 且 `coverage == None`」的 suite 从此把整份 summary 的 `conclusion` 从 `Fail` 提升为 `Error`，且 `coverage` 从 `Some(pass: true)` 收敛 `None`（历史现场：rust suite 零覆盖率 + vite 96.21% 达标时聚合面曾报「覆盖率通过」）。这是**预期修复**，不是回归：结论语义从「测试失败」变成「不可测（含失败）」 | proposal P6 与 AC-7 的直接后果；`problems` 里点名了缺失的 suite（framework + 退出码），用户 / 决策 agent 不会只看到一句无主的 `Error`（R-7 缓解）。既有语料（`aggregate_test` 的 `汇总组装逐字段相等`）将随之改写，登记给 test 相位 |
| D15 | `lib.rs` 导出面 | `pub use transient::{compiling_count, is_progress_stalled, progress_series, is_transient, AttemptTrace, RetryPolicy};`（新增三项，其余保持）；模块 doc 的「瞬态签名与有限重试」条补一句「含增量推进判据（`Compiling` 行计数 / 序列 / 卡死判定）」 | 检查域要装配「重试进展」文案，必须有跨 crate 的取数面；`kill_tree` / `ProcessOutput` / `exec_output` 家族语义零变 |
| D16 | 版本面 | `packages/desktop/package.json` `version` `0.5.28` → `0.5.29` | 用户可见变更（卡死不再烧满重试 + 终局文案含推进序列 + 覆盖率不再假通过），沿「仅用户可见变更才提升」纪律；`tauri.conf.json` 经 `../package.json` 跟随；`plugins/**` 零改动零 bump |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 推进判据单点 | `compiling_count`（`Compiling ` 行计数）/ `progress_series`（报告 → 序列）/ `is_progress_stalled`（末两次非降且末值 > 0）；签名表 remove 措辞收窄；退避常量上调 | `packages/desktop/src-tauri/crates/infra/exec/src/transient.rs`（修改） | `crate::run::{RunOutcome, RunReport}`、`std::time::Duration` | std-only（零 tokio、零第三方、零 IO） |
| 执行体重试循环 | 循环内维护按尝试序的计数序列；命中签名后按 `is_progress_stalled` 判卡死并返回末次结果（`attempts < max_attempts`） | `crates/infra/exec/src/run.rs`（修改） | `transient::{compiling_count, is_progress_stalled, transient_signature, AttemptTrace, RetryPolicy}`、`tokio`（既有特性面） | tokio `select!` + `Instant` 预算（既有骨架零改） |
| exec 导出面 | 三项新纯函数经 crate 根可达 | `crates/infra/exec/src/lib.rs`（修改） | 同上 | 既有 `pub use` 形态 |
| 检查域环境文案 | `fault_text` 扩参 `retried`；新增「重试进展」行（含卡死判定附语）+ 私有 `progress_line` | `crates/infra/checks/src/environment.rs`（修改） | `exec::{ProcessOutput, RetryPolicy, RunOutcome, RunReport, AttemptTrace}`（类型导入）+ `exec::progress_series` / `exec::is_progress_stalled`（全路径调用，无需导入） | 纯函数（零 io / 零 async） |
| static-check 调用点 | `fault_text` 补 `&run.retried`；`2>&1` 交错流 / `diagnostics` / `passed` 口径零变 | `crates/infra/checks/src/static_check.rs`（修改） | 同上 + `RunFault` | 既有 runner 形态 |
| suite 执行调用点 | `fault_text` 补 `&run.retried`；合并流「stdout 前缀 + stderr 后缀」零变 | `crates/infra/checks/src/testexec/runner.rs`（修改） | 同上 | 既有 |
| test-execution 跨 plan 总预算 | 常量 `TESTEXEC_TOTAL_BUDGET`；`execute` 收预算参数、逐 plan 前判定、超预算置 `Environment` fault 并走既有 `fault_outcome` | `crates/infra/checks/src/testexec/mod.rs`（修改） | `std::time::{Duration, Instant}`、`RunFault` | 既有编排形态（零新分支） |
| 覆盖率采集完整性 | 判据 `misses_coverage`（单点）/ problems 追加去重 / 聚合收敛 `None` | `crates/core/checks/src/aggregate.rs`（修改） | `crate::model::*`（既有） | 纯函数（既有） |

---

## 变更清单

> 路径口径：修改文件表与 proposal「变更范围 - 实现文件」同口径，`crates/...` 前缀省略 `packages/desktop/src-tauri/`。

<!-- 新增文件：本变更零新增文件（六处补丁全落既有落位：exec 三文件 + checks 三文件 + core/checks 一文件 + package.json），故整段省略。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/exec/src/transient.rs` | ① 签名表第 6 条 `"failed to remove file"` → `"failed to remove"`（`const` 的 `[&str; 13]` 长度不变），表上方注释补一句「生产文本为 `failed to remove <path>`，不含 `file`；收窄风险低（该短语只出现在删除失败语境）」；② `RETRY_BACKOFF` 改 `[5, 10, 20, 40] s` 并改写其理由注释（静默窗口分钟级 / 只剩收尾最需耐心 / 最坏 75 s 仍远低于 1500 s 硬顶）；③ `RetryPolicy::transient` 的 doc 由「`[1, 2, 4, 8] s`」改为「`[5, 10, 20, 40] s`」；④ 新增 `pub fn compiling_count(stdout: &[u8], stderr: &[u8]) -> u32`、`pub fn progress_series(report: &RunReport) -> Vec<u32>`、`pub fn is_progress_stalled(series: &[u32]) -> bool`（D1–D3，各带「零 IO / 零 spawn / 计数口径单点」doc）；⑤ 模块 doc 补一条纪律「推进判据只用于**提前放弃**，MUST NOT 用于延长预算」 | P1 / P2 / P4；模块 doc 的既有三条纪律（字典单点 / 刻意不收裸 `failed to write` / `TimedOut`·`Cancelled` 永不命中）逐字保留 |
| `crates/infra/exec/src/run.rs` | ① `use crate::transient::{compiling_count, is_progress_stalled, transient_signature, AttemptTrace, RetryPolicy};`；② `run_shell` 循环前增 `let mut counts: Vec<u32> = Vec::new();`，每次 `attempt_once` 返回后**立即** `counts.push(compiling_count(&attempt.stdout, &attempt.stderr));`（先于三臂判定）；③ 判定序插新臂：命中签名之后、上限 / 预算之前 `if is_progress_stalled(&counts) { return Ok(attempt.into_report(attempts, retried)); }`（D4）；④ `run_shell` 的 doc 循环判定序改写为「①非 `Completed` → ②未命中签名 → ③卡死判否（末两次计数非降且末值 > 0）→ ④上限 / ⑤预算 → ⑥退避 → ⑦入 `retried`」并写明「卡死只早退、不延预算；`attempts == 1` 无前值不触发；末次计数 `0` 不触发」；⑤ 模块 doc 第五能力项「有限重试」补一句「重试间按 `Compiling` 行计数判推进 / 卡死」 | P1；`RunOptions` / `RunReport` / `RunOutcome` / `RunError` **零字段变更**；`attempt_once` / `Race` / `await_exit` / `drain` / `wait_cancel` / 两常量**零改** |
| `crates/infra/exec/src/lib.rs` | `pub use transient::{compiling_count, is_progress_stalled, progress_series, is_transient, AttemptTrace, RetryPolicy};`（原行为 `is_transient, AttemptTrace, RetryPolicy`）；头部模块 doc 的「瞬态签名与有限重试」条补「含增量推进判据（`Compiling` 行计数 / 序列 / 卡死判定）」 | P1 / D15；`kill_tree` / `ProcessOutput` / `ExecOutput` / `record_exec_output` / `read_exec_output` / `read_exec_output_in` / `mint_exec_id` / `SystemCommand` 语义零变 |
| `crates/infra/checks/src/environment.rs` | ① `use exec::{AttemptTrace, ProcessOutput, RetryPolicy, RunOutcome, RunReport};`；② `fault_text` 签名扩为 `pub(crate) fn fault_text(attempts: u32, exit_code: i32, output: &[u8], retried: &[AttemptTrace]) -> String`，在「已自动重试 N 次」行之后插入 `if let Some(line) = progress_line(&series) { text.push('\n'); text.push_str(&line); }`（D6，D7）；③ 新增私有 `fn progress_line(series: &[u32]) -> Option<String>`（D8：`len < 2` 或全 `0` → `None`；否则 `重试进展：待编译 {join(" → ")}`，卡死时追加 `（末两次无进展，判定为卡死）`）；④ 序列经 `exec::progress_series(&RunReport { .. })` 视图取数（D7），归因段 / 摘要段 / `ENV_GUIDANCE` / `clip_chars` / `attempts_log` **零改**；⑤ 模块 doc 补「重试进展」一项与「计数口径单点驻 `exec::transient`，本模块不重算」 | P3；`DIAGNOSIS_EXCERPT_LIMIT` / `ENV_GUIDANCE` 两常量零改；`attempts_log` 零改（record 形态不变） |
| `crates/infra/checks/src/static_check.rs` | `fault_text` 调用补第四参 `&run.retried`（`:102-106` 一处） | P3；`2>&1` 交错流、`diagnostics`（只读 `stdout` + `trim_end`）、`passed = exit_code == 0`、`report_dir` / `record_exec_output` 零变 |
| `crates/infra/checks/src/testexec/runner.rs` | `fault_text` 调用补第四参 `&run.retried`（`:130-134` 一处） | P3；合并流「stdout 前缀 + stderr 后缀」、`non_zero_exit_error`、工件解析、`PlanExecution` 字段面零变 |
| `crates/infra/checks/src/testexec/mod.rs` | ① 新增 `const TESTEXEC_TOTAL_BUDGET: Duration = Duration::from_secs(2400)`（doc：依据 + 不进 config schema，放在现有常量组末尾）；② `use std::time::{Duration, Instant};`；③ `ProcessTestExecution::run`（`:69-75`）调用改 `execute(&root, &change, slot, cancel, TESTEXEC_TOTAL_BUDGET)`；④ `execute` 增第五参 `total_budget: Duration`，在 `started_at`（`:91`）旁取 `let budget_started = Instant::now();`；⑤ 逐 plan 循环改 `enumerate`，**每 plan 之前**判 `budget_started.elapsed() >= total_budget` → `if fault.is_none() { fault = Some(RunFault::Environment(格式串)) }` 后 `break`（D9 / D10）；⑥ `execute` doc 补「跨 plan 总预算：超预算余下 plan 不 spawn、以既有 fault 路径收口（不写 summary / 不进复用门）」 | P5；`fault_outcome` / `outcome` / `build_sub_report` / `findings_*` / 复用门（`read_existing_summary` / `load_disk_sub_reports` / `plan_ids_match` / `is_reusable`）/ `newest_input_mtime_ms` / `prune_roots` **零改**；既有 fault 聚合优先级不变 |
| `crates/core/checks/src/aggregate.rs` | ① 新增私有 `fn misses_coverage(report: &SubReport) -> bool`（D11，放在 problems 归并段之前并注释「采集完整性判据单点：归因与收敛共用」）；② `aggregate_coverage` 首行插 `if sub_reports.iter().any(misses_coverage) { return None; }` 并补 doc 一句（D13）；③ `collect_problems` 逐 report 增局部 `has_execution_error` 标记 + 尾部追加覆盖率缺失的 `Problem { problem_type: ExecutionError, framework: 该 suite, message: 「覆盖率采集缺失（退出码 N）：该 suite 结论非通过且未产出覆盖率数据」 }`（D12）；④ `collect_problems` doc 补「含覆盖率采集缺失归因（每 suite 至多一条，与既有 execution_error 去重）」 | P6；`determine_conclusion` / `weighted_measured` / `aggregate_measured` / `coverage_meets_thresholds` / `compute_overrides` / `format_coverage_failure` / `build_mutation_block` / `merge_mutation_blocks` / `in_suite_scope` 家族**零改** |
| `packages/desktop/package.json` | `version` `"0.5.28"` → `"0.5.29"`（单行） | D16；`plugins/**` 零改动、`src-tauri/Cargo.toml` 不随动 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|--------|----------|-----------|----------|------|
| `compiling_count` | `crates/infra/exec/src/transient.rs`（经 `lib.rs` 再导出） | 新增 | `pub fn compiling_count(stdout: &[u8], stderr: &[u8]) -> u32` | 合并 `stdout ++ stderr`（UTF-8 lossy）后，去前导空白以 `Compiling ` 为前缀（大小写**敏感**）的行数；无该字样 → `0`；零 IO / 零 spawn / 零 panic（D1） |
| `progress_series` | 同上 | 新增 | `pub fn progress_series(report: &RunReport) -> Vec<u32>` | `retried` 按序在前 + 末次在后，长度 = `retried.len() + 1`（生产不变量下 == `attempts`；未重试即长度 1）（D2） |
| `is_progress_stalled` | 同上 | 新增 | `pub fn is_progress_stalled(series: &[u32]) -> bool` | `series.len() < 2` → `false`；否则 `末值 > 0 && 末值 >= 前值`（D3） |
| `RetryPolicy::transient` | 同上 | 修改（常量为值，签名不变） | `pub fn transient() -> RetryPolicy` | `max_attempts = 5`、`backoff = [5, 10, 20, 40] s`、`total_budget = 1500 s`（D5） |
| `run_shell` | `crates/infra/exec/src/run.rs` | 修改（签名不变，语义加早停臂） | `pub async fn run_shell(command: SystemCommand, options: RunOptions) -> Result<RunReport, RunError>` | 判定序新增「卡死早停」（D4）；`RetryPolicy::none()` / 未命中签名 / `TimedOut` / `Cancelled` 四条既有面逐字节不变 |
| `fault_text` | `crates/infra/checks/src/environment.rs` | 修改（`pub(crate)`，增第四参 + 新行） | `pub(crate) fn fault_text(attempts: u32, exit_code: i32, output: &[u8], retried: &[AttemptTrace]) -> String` | 归因段 + 「已自动重试 N 次」+ 可选「重试进展」行 + 末次诊断摘要 + `ENV_GUIDANCE`（D6–D8） |
| `aggregate_coverage` | `crates/core/checks/src/aggregate.rs` | 修改（签名不变，语义加采集完整性收敛） | `pub fn aggregate_coverage(sub_reports: &[SubReport], suites: &[TestSuite]) -> Option<CoverageBlock>` | 采集不完整（任一 suite 结论非 pass 且 `coverage == None`）→ `None`，MUST NOT 为 `pass: true`（D13） |
| `build_summary_report` | 同上 | 修改（签名不变，problems 增一类归因） | `pub fn build_summary_report(sub_reports: &[SubReport], suites: &[TestSuite], command: &str, started_at: OffsetDateTime) -> SummaryReport` | 经 `collect_problems` 追加覆盖率缺失的 `ExecutionError`（每 suite 至多一条、去重）；`conclusion` 映射零改（D12 / D14） |

<!-- 私有 / pub(crate) 不列：`transient_signature`（transient.rs 判定核心）、`progress_line`（environment.rs）、`fault_text` 内的「视图报告」构造（environment.rs）、`misses_coverage`（aggregate.rs）、`collect_problems`（aggregate.rs）、`execute`（testexec/mod.rs，私有，签名增第五参 `total_budget: Duration`，见修改文件行）、`fault_outcome` / `outcome`（testexec/mod.rs）、`attempt_once`（run.rs）。
     三 runner trait 方法（`StaticCheckRunner::run` / `TestExecutionRunner::run` / `MutationRunner::run`）签名零改；`ProcessTestExecution::run` 签名零改（仅内部调用点补常量实参）。
     无 CLI 子命令 / 无 HTTP 端点 / 无 Tauri IPC 命令变更。 -->

### 类型定义

<!-- 类型定义：本变更零新增、零修改类型。`RunOptions` / `RunReport` / `RunOutcome` / `RunError` / `AttemptTrace` / `RetryPolicy`（exec）、`StaticCheckOutcome` / `TestExecutionOutcome` / `RunFault`（orchestration port）、`PlanExecution`（testexec/runner）、`SubReport` / `SummaryReport` / `Problem` / `ProblemType` / `CoverageBlock`（core/checks model）字段集与变体集全部逐字不变（proposal 硬约束）；新增的三函数只产 `u32` / `Vec<u32>` / `bool`，不引入类型。故整段省略。 -->

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.29`（原 `0.5.28`） | 用户可见变更提升补丁号（D16 / AC-10） |
| `RETRY_BACKOFF` | `crates/infra/exec/src/transient.rs` | 代码常量（非配置面） | `[Duration; 4]` | `[5 s, 10 s, 20 s, 40 s]`（原 `[1, 2, 4, 8] s`） | 与 `RETRY_MAX_ATTEMPTS = 5` / `RETRY_TOTAL_BUDGET = 1500 s` 同族；MUST NOT 进 config schema（P2 / D5） |
| `TESTEXEC_TOTAL_BUDGET` | `crates/infra/checks/src/testexec/mod.rs` | 代码常量（新增） | `Duration` | `2400 s` | 跨 plan 总预算；MUST NOT 进 config schema（P5 / D9） |
| `openspec/config.json` | — | 否（零改动） | — | — | 预算 / 退避参数零 schema 扩展（AC-10） |

---

## 数据模型

持久化面**零结构改动**（无表、无索引、无迁移、无 schema 字段增删）。本变更涉及三类**进程内载体**与两处**既有落盘内容的值域变化**：

```
exec::run_shell（每次调用）
├── RunReport{ outcome, exit_code, stdout, stderr, attempts, retried }   ← 字段集零变
└── 循环局部 counts: Vec<u32>            ← 新增（内存 / 不持久化 / 不进报告）
      └── 每次尝试 push(compiling_count(&attempt.stdout, &attempt.stderr))
      └── 命中签名后交 is_progress_stalled 判卡死（只早退）

读侧（检查域）
progress_series(&RunReport) = [count(retried[i]) … count(末次)]   ← 与 run.rs 的 counts 同源同值
      └── fault_text → 「重试进展：待编译 c1 → … → cN」（可选「（末两次无进展，判定为卡死）」）

编排侧
test-execution execute(.., total_budget)
├── budget_started: Instant            ← 新增（与 started_at 相邻取时）
└── fault: Option<RunFault>            ← 既有面；超预算时置 Environment（不覆盖已有 fault）

聚合侧（落盘内容的值域变化，结构零变）
summary.json
├── problems[]   ← 可能新增 { problem_type: "execution_error", framework: <suite>, message: 覆盖率采集缺失… }
├── coverage     ← 采集不完整时由 Some(pass) 收敛为 null（既有 schema 已允许）
└── conclusion   ← 随 problems 的 execution_error 升为 "error"（映射零改）
```

| 面 | 载体 | 语义 / 生命周期 |
|----|------|-----------------|
| 尝试计数序列 | `run.rs` 的循环局部 `counts: Vec<u32>` | 单次 `run_shell` 的内存中间量，随函数返回释放；**不进 `RunReport`**（字段面零变）；与读侧 `progress_series` 共用 `compiling_count`，字节零改写零截断是两者等值的前提 |
| 推进判据 | `is_progress_stalled(&[u32]) -> bool` | 纯函数，无状态；「末两次非降且末值 > 0」= 同一批写不进去 |
| 重试进展文案 | `RunFault::Environment(String)` 内的一行 | 长度 `O(attempts)`（至多 5 个数字）；只进 `TXT_ENV_STATIC` / `TXT_ENV_TEST` 终局文案，不占 `findings_detail` 的 2000 字符预算（R-4） |
| 跨 plan 预算 | `testexec::execute` 的局部 `budget_started: Instant` | 单次执行的内存量；超预算走既有 fault 收口（不写 summary、不进复用门），已完成 plan 的工件仍在报告树可人工回查（R-5） |
| 覆盖率采集完整性 | `SubReport` 既有字段的组合判据（`exit_code` / `summary.failed` / `coverage`） | 零新增字段；判据为纯函数 `misses_coverage`，归因（problems）与收敛（`aggregate_coverage`）两处共用 |
| 报告树结构（零变） | `reports/<slot>/<plan_id>/{report.json, results.txt, coverage*.json}`、`summary.json`、`exec-outputs/<exec_id>.log` | 字段集与文件名零改；`exec-outputs` 内容形态零改（`attempts_log` 未动） |

关系面零新增外键 / 零跨表引用：`RunFault` 与 `run_history` 表无交集，`ChangeStepStatus` / `ChangeRunStatus` 词汇零扩展。

---

<!-- 路由 / API 设计：本变更不涉及。无新增 / 修改 Tauri IPC 命令、无 HTTP 端点、无 CLI 子命令；`RunUpdate` / `RunNodeRecord` / 快照 DTO 零扩展，三 outcome 均不出线，`src/types/generated/bindings.ts` 应零 diff（AC-10）。 -->

---

## 依赖

### 运行时依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| 新增第三方 crate | **无** | 三处推进判据 / 文案装配 / 预算判定 / 采集完整性判据全为 std 纯函数 |
| `exec` → `tokio` | 不变（`process, io-util, time, macros`） | 循环早停复用既有 `select!` / `sleep` / `timeout`；不引 `rt`（仍不 `tokio::spawn`） |
| `exec` → `foundation` | 不变（既有边） | `CancelFlag` 逐次观测 |
| `checks-runtime` → `exec` / `orchestration` | 不变（既有边） | 取数面（`progress_series` / `is_progress_stalled`）+ `ProcessOutput` 摘要 + `RunFault` |
| `core/checks` → `config` / `time` | 不变（既有边） | 采集完整性判据只用既有 `SubReport` 字段 |

### 构建 / 测试依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| `exec` dev-dependencies | 不变（`tempfile` + `tokio{rt,macros,test-util}`） | 纯函数用例零装置直构 `RunReport`；卡死早停沿用既有 shim + `fast_policy` / `start_paused` 先例 |
| `checks-runtime` dev-dependencies | 不变（`tokio(test-util)` / `workflow` / `tempfile` 等既有面） | 预算用例经模块内私有 `execute(.., total_budget)` 注入毫秒级预算；`fault_text` 用例纯函数直驱 |
| `core/checks` dev-dependencies | 不变 | 采集完整性用例 fixtures 直驱（零 spawn） |

---

## 验收标准回应

| AC | 设计落点 |
|----|----------|
| AC-1 | `is_progress_stalled` 判据（D3）+ `run.rs` 早停臂位置（D4）：末两次相同 `Compiling` 计数 → 第 2 次尝试后返回末次结果（`attempts == 2 < max_attempts`），不烧满 5 次 |
| AC-2 | 递减序列不触发判据（`last >= previous` 为假）；末值 `0` 时 `last > 0` 为假 → 继续重试至成功 / 上限 / 预算（D3） |
| AC-3 | `RETRY_BACKOFF = [5, 10, 20, 40] s`（配置表）+ `max_attempts = 5` / `total_budget = 1500 s` 零改（D5）；成功路径零额外等待（未命中签名不进退避） |
| AC-4 | 签名表第 6 条改 `failed to remove`（修改文件 transient.rs ①）：`failed to remove D:\…\x.rcgu.o` 命中并返回表内原文；表长仍 13，其余 12 条零改 |
| AC-5 | `fault_text` 扩参 + 新行（D6 / D7 / D8）：含占用进程名（既有归因段）+ `重试进展：待编译 …` 序列，卡死时含判定附语；`attempts == 1` 或序列全 `0` 时无该行；两调用点各补 `&run.retried` |
| AC-6 | `TESTEXEC_TOTAL_BUDGET = 2400 s` + 逐 plan 前判定 + 既有 `fault_outcome` 收口（D9 / D10）：超预算余下 plan 零 spawn、不写 summary、`report_dir` / `exec_id` 回填、既有 fault 不被覆盖 |
| AC-7 | `misses_coverage` 单点判据 + problems 去重追加 + 聚合收敛 `None`（D11 / D12 / D13）：缺测 suite → `conclusion == Error` 且 `coverage` 不为 `pass: true`；同 suite 不重复归因 |
| AC-8 | `TimedOut` / `Cancelled` / 真红三面判定序在最前（D4），行为逐字节不变；`RunOptions` / `RunReport` / `RunOutcome` / `RunError` 零字段变更；`attempts_log` / `diagnostics` / 合并流顺序 / `report_dir` / `exec_id` / 复用门 / 基线·mutation 分流零触点（修改文件表逐行标注） |
| AC-9 | 现场回归（非 CI 可断言）：`fault_text` 的推进序列行是「重试确实在推进」的可见证据（D6–D8）；冷 target 自愈能力由既有瞬态重试 + P2 慢档退避共同提升——登记为交付后人工验收项（待决问题 6） |
| AC-10 | `package.json` 0.5.28 → 0.5.29（配置表 / D16）；`plugins/**` 零改动；`bindings:check` 零 diff（零载荷字段出线，见「路由 / API 设计」注释）；覆盖率阈值门禁要求三处新纯函数全覆盖（R-9，由 test 相位承接） |

---

## 待决问题

1. **P6 连带语义变化的既有语料改写（须在 test 相位承接）**。`aggregate_test.rs` 的 `汇总组装逐字段相等`（vite-plus：`failed = 1`、`coverage = None`）在新判据下会多一条 `execution_error`、`coverage` 收敛 `None`、`conclusion` 由 `Fail` 升 `Error`。本设计判定这是 AC-7 的**预期**结果（D14），不改判据去迁就既有断言；建议 test 相位把该用例拆成「全测齐全（逐字节零变）」与「缺测归因」两臂，并新增 P6 三臂用例（缺测 / 去重 / 齐全零变）。若评审认为「failed > 0 且无覆盖率」不应升 `Error`，需回 proposal 收窄判据（三条件取真子集），本设计不擅自收窄。
2. **`fault_text` 以「视图报告」取序列（D7）**。spec 的 Module Contract 固定了 `fault_text` 的四参签名与「按 `exec::progress_series` 取数」两句；两者同时成立只能由检查域构造一次性 `RunReport` 视图（代价：fault 路径上一次 `retried` 字节 clone）。备选是给 exec 增 `progress_series_of(retried, last_stdout, last_stderr)` 变体（零 clone，但扩公开面且属单次使用抽象）。若评审优先零拷贝，采纳备选即可（检查域两行改动）。
3. **卡死早停与上限 / 预算判定的相对序（D4：③ 在 ④ 前）**。三者在 `attempts == max_attempts` 面同返末次结果，纯观测无差；取「卡死优先」是为让 fault 文案的「判定为卡死」附语在「最后一次尝试恰为上限次」时仍成立。若评审要求上限 / 预算优先，移位一行即可，验收面不变。
4. **`execute` 增第五参 `total_budget`（D9 / D10）**。唯一目的是让预算用例以毫秒级预算直驱全链（否则 AC-6 的「余下 plan 零 spawn」不可机械证明：真跑 2400 s 不可行）。生产唯一调用方 `ProcessTestExecution::run` 传常量，故不构成配置面。若评审要求 `execute` 签名零变，退化为「纯谓词 + 常量锚定」测试，AC-6 的「零 spawn」面将失去机械证据。
5. **「重试进展」行的拼接格式**。spec 只给样例 `重试进展：待编译 264 → 5 → 0 → 0` 与卡死附语原文；本设计取「同一行内以全角括号追加 `（末两次无进展，判定为卡死）`」。若产品希望附语另起一行或改用不同标点，属纯文案调整（一行）。
6. **AC-9 的现场回归不可 CI 断言**。冷 target（新建 worktree）自愈 + 终局文案可读性需真实环境一次 `pnpm run check` 人工验收；本设计不铺自动化钩子，登记为交付后验收项（与 R-8 的 P7 分离实验正交）。
7. **P7（worktree 根 `C:\Users` vs D 盘）仍未分离**。proposal 明确本次不动 `DATA_DIR`；若 `mklink /J` 实验证伪 H2 而立住 H1，则该常量变更优先级高于本篇全部补丁，需另立变更（本篇六处补丁与之正交，不因之作废）。
