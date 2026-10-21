# 设计: exec-run-shell

> **变更**: exec-run-shell
> **日期**: 2026-10-21
> **基于**: `proposal.md`、`explore.md` 与 `specs/{desktop-exec-runtime,desktop-checks-domain,desktop-test-execution,desktop-mutation-testing,desktop-change-orchestration,desktop-crate-layout}/spec.md`

---

## 提案与规格同步状态

proposal「变更范围 - 实现文件」与六份 capability delta 已定稿，本设计不重复其论证，只做四件事：逐文件落定实现形态（含删除的精确区块与调用点全量清单）、把规格未覆盖的两处形态缺口（`run_shell` 的**拉起失败面**、`run_tool` 的**形参取法**）以决策留痕方式收口、给出取消链的时序与边界、按 AC-1..AC-11 逐条对齐。

起点核实（工作树实况，锚点为改动前行号）：

- **四个 spawn 点与三份私有 `read_all`**：`static_check.rs:63-73`（`command.output()`，零超时、单管道）、`testexec/runner.rs:89-110`（`tokio::time::timeout` + `start_kill`）、`mutation/cargo_mutants.rs:56-94`（同形态）、`mutation/stryker.rs:62-104`（同形态）；`read_all` 分别在 `runner.rs:165`、`cargo_mutants.rs:119`、`stryker.rs:215`。`exec::kill_tree` 已在 `crates/infra/exec/src/lib.rs:242` 落地且仅被 `infra/agent/src/sdk/bash.rs:55` 消费。
- **`SystemCommand` 现为 `#[derive(Debug)]`**（`exec/src/lib.rs:29`），三字段（`program: OsString` / `args: Vec<OsString>` / `raw: bool`）均可 `Clone`。
- **`exec/Cargo.toml` 现依赖面**：`foundation` + `tokio`（features `["process"]`）；dev 仅 `tempfile`。`tokio::process::Command` / `kill_on_drop` / `read_to_end` / `select!` 所需特性面尚未全开。
- **`foundation/Cargo.toml` 现依赖面**：仅 `sha2`；`lib.rs` 仅 `identity` / `layout` / `prune` 三模块。
- **取消链现状**：`RunEntry.cancel: watch::Sender<bool>`（`control.rs:39`）、`request_stop` 只 `send_replace(true)`（`control.rs:120-129`）、`RunGuard::cancelled()` 同步读 watch（`control.rs:356-360`）、`check_cancel` 唯一调用点在 `walker.rs:221`（循环顶部，步边界）。`ToolStepRequest { root, command }`（`port.rs:138-142`，`derive(Debug, Clone, PartialEq, Eq)`）；三 trait 签名在 `port.rs:317-342`；`steps.rs` 三处调用在 `:208` / `:234` / `:272`。
- **`run_tool` 调用点 13 处**（`walker.rs:270/707/728/949/1070/1145/1207/1293/1718/2155/2915/3111/3195`），其中 `step_phase_next`（`:703`，仅 `tools` + `request`）与 `run_phase_log`（`:1713`，8 处调用）当前**拿不到 guard**，需增形参向上取。
- **测试面实量**：`ToolStepRequest` 构造字面量 **26 处**（`port_test` / `steps_test` / `walker_test` / `checks::mutation::mod_test`）；三 trait 的**测试 impl 13 处**（`port_test` 3 / `steps_test` 3 / `walker_test` 4 / `checks::mutation::mod_test` 2 / `vcs::lib_test` 1）+ 生产 impl 3 处——proposal「11 处假实现」为估算，测试相位以 13 处实量为准逐处补 `cancel` 形参。
- **段/超时常量**：`SUITE_COMMAND_TIMEOUT = 600 s`（`runner.rs:13`，doc 现写「CLI `runCommand(cmd, cwd, 600000)` 同值」）、`SEGMENT_SPAWN_TIMEOUT_MS = 1_800_000`（`mutation/mod.rs:49`，两段共用）；static-check **无**超时常量。
- **测试门禁装置**：`checks::mutation::mod_test::SpawnWindow` 是 **PATH 前置假可执行体**（`cargo.exe` / `npx.cmd` / unix shim），spawn 真实发生 → 迁 `run_shell` 后装置零改（不依赖 exec 侧 spawn 缝）；`start_paused` + 手动 `advance` 先例在 `cargo_mutants_test.rs:400-420`。
- **`packages/desktop/package.json` = `0.5.19`**。

**两处规格偏离（须在规格相位回写，见「待决问题」）**：① `desktop-exec-runtime` 的 `run_shell` 箭头写 `-> RunReport`，但拉起失败 / 等待失败必须可表达（否则 AC-8「既有 `Err` / `Fatal` 分流与文案骨架不变」被破）；② `run_tool` 的形参取法在 Module Contract 行（`(tools, root, cancel, command, label)`）与 scenario「各步函数不各自取值」之间有张力。

**只读红线零触点**：`crates/infra/agent/**`（含 `sdk/bash.rs`）、`crates/infra/vcs/src/{lib,git,bootstrap}.rs`、`crates/core/checks/**`、`crates/core/workflow/**`、`crates/core/orchestration/src/state.rs`、`crates/infra/checks/src/testexec/detect.rs`、`crates/infra/exec/src/{output,shell}.rs`、前端 `packages/desktop/src/**` 与 `src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**`。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | `run_shell` 的失败面 | 返回 **`Result<RunReport, RunError>`**；`RunError` 封闭两值 `Spawn(String)` / `Wait(String)`，`Display` 渲染 `"拉起失败: {io}"` / `"等待失败: {io}"`（只承载类别词 + `io::Error` 原文，**不带调用点语境**） | 三值 `RunOutcome` 无法表达「进程根本没建立」：现有四处对 spawn 失败分别落 `Err`（static-check / testexec）/ `SegmentFailure::Fatal`（cargo-mutants / stryker），若吞成 `Completed{exit_code:-1}` 会把 Fatal 降级为 Interrupted（AC-8 破）。两值分类是为了让调用点各自拼回**逐字同现状**的文案（见 D2） |
| D2 | 四调用点失败文案拼法 | `static_check`：`format!("static-check 命令{failure}")`（拉起臂逐字同现状；等待臂为新可辨文案）；`testexec`：`format!("测试命令{failure}")`（两臂逐字同现状）；`cargo_mutants` / `stryker`：`match` 两臂显式拼（含 cargo-mutants 的「（程序不可达？）… cargo install cargo-mutants」括号与指引） | 每个调用点保留自己的前缀，`RunError` 只提供「拉起失败 / 等待失败」类别词 → spawn 臂文案 4/4 逐字不变；唯一新增面是 static-check 的等待臂（该分支现状不存在，不构成回归） |
| D3 | 取消信号形态与取值 | `foundation::cancel::{CancelFlag, CancelHandle}`；`run_shell` 以 **200 ms 轮询** `is_set()`；不再引 `Notify` | spec「取消原语零 tokio 下沉」既定；两消费者（`RunGuard` 写 / `run_shell` 读）到场，符合 foundation「第二消费者才下沉」纪律；轮询是零 tokio 的唯一形态 |
| D4 | 已置位 cancel 的入口短路 | `run_shell` 进入即查一次 `cancel.is_set()`：真 → **不 spawn**，直接 `RunReport { outcome: Cancelled, exit_code: -1, .. }` | 规格 scenario「取消已置位 → `Err`（零 spawn 或即杀）」；同时让取消用例不依赖 200 ms 轮询时序（R-3） |
| D5 | 杀后时序（R-8） | `kill_tree(pid)` → **有界收尸** `timeout(FINISH_GRACE, child.wait())` → **有界排水** `timeout(FINISH_GRACE, join!(drain, drain))` → 组装 `RunReport`；`FINISH_GRACE = 2 s` | 只 `kill_tree` 不加界会把「超时悬挂」换形为「杀后悬挂」（unix 侧 `kill -9 <pid>` 不杀 rustc 子孙，若其继承管网道的写端则 `read_to_end` 永不 EOF，见 R-4）。有界两跳保证「预算 + 有限收尾」的墙钟上界；`kill_on_drop(true)` 兜底 future 被 drop 的路径 |
| D6 | 被杀路径的输出字节 | 两个输出缓冲区置于 `run_shell` **局部作用域**、由「排水 + 收尸」future **可变借用**（不 `move` 进 future、不用 `tokio::spawn`）→ 杀后仍可继续排入残余字节；被杀路径的输出**不保证完整**并显式留痕 | 四消费点在 `TimedOut` / `Cancelled` 臂均不消费输出（static-check → `Err`；testexec → `output: Vec::new()`；两 mutation 段 → 固定文案），故「不完整」不改变任何落 record 语义（AC-8）。副产品：`exec` 无需 `tokio` 的 `rt` 特性（不 `spawn`） |
| D7 | `CancelFlag` 的相等语义 | 手写 `PartialEq` = `Arc::ptr_eq`（同一底层原子），并 `impl Eq`；`Debug` 手写为 `"CancelFlag(set = {bool})"` | `port::ToolStepRequest` 现状 `derive(Debug, Clone, PartialEq, Eq)`：26 处字面量与既有断言零随动（保 derive 不变）；两个独立铸造的 flag 永不相等属「身份比较非状态比较」，文档明写（无消费者做值比较，已 grep 核实） |
| D8 | `RunGuard::cancel_flag()` 的收口路径 | 注册表里 run 已除名（终态收口后）→ 回落 `CancelFlag::never()`，不 panic 不 `Err` | 与既有 `nodes()` / `edges()` / `cancelled()` 的「除名即空路径」纪律同型；`finish` 消费 self，guard 不可能在被持有期间除名以外的方式失效 |
| D9 | `request_stop` 双写顺序 | 先 `entry.cancel.send_replace(true)`（既有异步观测面：停等 / 应答等待），再 `entry.cancel_flag.handle().set()`（进程面）；两次调用幂等 | watch 臂保既有语义逐字不变（`send_replace` 无订阅者亦不丢）；进程面标志由同一原子承载，重复 `set()` 无副作用 |
| D10 | `run_tool` 的形参取法 | `run_tool(tools, guard: &RunGuard, root: &str, command: ToolCommand, label: &str)`——`ToolStepRequest.cancel` 由 run_tool **单点**取 `guard.cancel_flag()` 填充；`step_phase_next` / `run_phase_log` 增 `guard: &RunGuard` 形参向上取（callers 已有 `ctx.guard` / `guard` 在手） | spec scenario 原文「`cancel` 仅在 `run_tool` 一处从 `guard.cancel_flag()` 取值填充，**各步函数不各自取值**」：传 `guard` 比传 `cancel` 严格满足该句（13 处调用点无一处出现 `.cancel_flag()`）；Module Contract 行的 `cancel` 形参是该契约的等价改写，语义面（单一填充点）不变。若评审以表格字面为准，改回 `cancel: CancelFlag` 只需机械替换 13 处（待决问题 3） |
| D11 | 超时预算常量 | static-check 新增 `pub(crate) const STATIC_CHECK_TIMEOUT: Duration = Duration::from_secs(1200)`；`SUITE_COMMAND_TIMEOUT` 600 → **1200**（doc 口径由「与 CLI 同值」改为「rust suite 实测 340–393 s × 约 3 倍余量，CLI 同值锚定已失效」）；两 mutation 段继续用 `SEGMENT_SPAWN_TIMEOUT_MS = 1_800_000`（转 `Duration::from_millis`） | spec「超时必填与超时预算表」；四值全为代码常量（不进 config schema，不设用户可配项）。static-check 的 1200 s 在模块内只此一处消费，常量即锚 |
| D12 | 检查域取消的收敛口径 | `RunOutcome::Cancelled` → static-check / testexec 返回 `Err("执行已取消")`；两 mutation 段返回 `SegmentFailure::Cancelled(String)` → `conclusion = Error` + 反馈文案「突变执行已取消」+ 已产报告路径；**不引入 run stopped** | proposal 范围裁决「过渡形态」；`Err` 经 `run_tool` 包成 failed（`Terminal::failed("{label} 失败: 执行已取消")` → 节点失败文本含「执行已取消」，满足 AC-7 文案断言）。mutation 侧取消**不走 `Fatal`**（spec：不把用户停止呈现为环境损坏） |
| D13 | `SegmentFailure` 终态面形态 | 新增 `Cancelled(String)`；`execute` 的中断槽由 `Option<String>` 改 `Option<SegmentFailure>`，终端面按变体分文案（`Interrupted` → 既有「突变执行中断：…」；`Cancelled` → 「突变执行已取消：…」），两变体同走 `conclusion = Error` 与 Stryker 临时面保留 | 变体独立（不复用 `Interrupted` 文案）便于后续 stopped 收敛时直接映射（R-9）；Stryker 桶失败面保留纪律对取消面同样适用（spec scenario「取消后临时面保留」） |
| D14 | 输出合流归属 | `run_shell` **不预合流**，只给 `stdout` / `stderr` 两路原始字节；static-check 保留命令内 `2>&1`（`stderr` 恒空、诊断单流逐字不变），testexec / mutation 由调用方拼「stdout 前缀 + stderr 后缀」 | spec「合流顺序归调用方」+ R-2；`parse_cargo_test` 只读 stdout 面，绝不能由执行体代加 `2>&1` |
| D15 | 版本面 | `packages/desktop/package.json` `version` 0.5.19 → 0.5.20（单行） | 用户可见变更（停止 / 超时不再悬挂、suite 预算 1200 s），沿「仅用户可见变更才提升」纪律；`tauri.conf.json` 经 `../package.json` 跟随，`src-tauri/Cargo.toml` 与 `plugins/**` 不动 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 统一执行体 | `run_shell` 单点承担 spawn（`kill_on_drop`）/ 双管道并发排水 / 三值收口（取消 / 超时 / 退出）/ `kill_tree` 整树击杀 / 有界收尸与排水 | `packages/desktop/src-tauri/crates/infra/exec/src/run.rs`（新） | `foundation::cancel::CancelFlag`、`crate::{SystemCommand, kill_tree}`、`tokio`（`process` / `io-util` / `time` / `macros`） | `tokio::process` + `tokio::select!` 三臂竞速 + 200 ms 轮询 |
| 执行体导出面 | `mod run;` + `pub use run::{run_shell, RunError, RunOptions, RunOutcome, RunReport}`；`SystemCommand` 补 `Clone` | `crates/infra/exec/src/lib.rs` | 同上 | 既有 crate 形态 |
| 取消原语 | 写端 `CancelHandle::set` / 读端 `CancelFlag::is_set`，`Clone` 共享同一原子；`never()` 供缺省零值 | `crates/core/foundation/src/cancel.rs`（新） | `std::sync::{Arc, atomic::{AtomicBool, Ordering}}`（**零 tokio、零 workspace 内依赖**） | std-only |
| run 控制注册表（写端） | `RunEntry.cancel_flag` 铸造与双写；`RunGuard::cancel_flag()` 供同步读取 | `crates/core/orchestration/src/control.rs` | `foundation::cancel` | 既有 `Mutex<HashMap>` 注册表 |
| port 契约 | `ToolStepRequest.cancel` + 三 runner trait 各加 `cancel` 形参 | `crates/core/orchestration/src/port.rs` | `foundation::cancel` | 既有进程内缝 |
| 组合根透传 | `LocalToolSteps::execute` 把 `step.cancel` 透传给三 runner（零决策） | `crates/core/orchestration/src/steps.rs` | — | 既有 |
| 透传单点 | `run_tool` 取 `guard.cancel_flag()` 填 `ToolStepRequest.cancel`；`step_phase_next` / `run_phase_log` 增 `guard` 形参 | `crates/core/orchestration/src/walker.rs` | `RunGuard` | 既有 walker |
| 检查域四 spawn 点 | static-check / suite / cargo-mutants 段 / Stryker 段统一经 `run_shell`，各自映射结局到既有失败面 | `crates/infra/checks/src/static_check.rs`、`testexec/runner.rs`、`mutation/{cargo_mutants,stryker}.rs` | `exec::{run_shell, RunOptions, RunOutcome, RunError}`、`foundation::cancel` | 既有 runner 形态（零私有 spawn / 超时 / 排水） |
| 突变失败面 | `SegmentFailure::Cancelled` 贯穿 `execute → run_suite → execute_segment`，取消 → `conclusion = Error` | `crates/infra/checks/src/mutation/mod.rs` | — | 既有 E5 分流形态 |
| testexec port 实现 | `ProcessTestExecution::run` 形参加 `cancel` 并透传 `execute_plan` | `crates/infra/checks/src/testexec/mod.rs` | — | 既有 |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/infra/exec/src/run.rs` | 统一进程执行体：`run_shell` + `RunOptions` / `RunOutcome` / `RunReport` / `RunError`；私有助手 `drain`（`read_to_end` 至 EOF）、`wait_cancel`（200 ms 轮询）、常量 `CANCEL_POLL_INTERVAL = 200 ms` / `FINISH_GRACE = 2 s` | 公共函数表 `run_shell` 行；类型定义表 `RunOptions` / `RunOutcome` / `RunReport` / `RunError` 行 |
| `packages/desktop/src-tauri/crates/core/foundation/src/cancel.rs` | 取消原语：`CancelFlag`（`Clone` / `never` / `is_set` / `handle`）+ `CancelHandle`（`set`），std-only | 公共函数表 `CancelFlag::never` / `is_set` / `handle` 与 `CancelHandle::set` 行；类型定义表 `CancelFlag` / `CancelHandle` 行 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/exec/src/lib.rs` | ① 头部模块 doc 补一条能力项「进程执行体：`run_shell`（超时必填 / 双管道并发排水 / 进程树击杀 / 取消）」；② `mod run;` + `pub use run::{run_shell, RunError, RunOptions, RunOutcome, RunReport};`；③ `SystemCommand` 的 `#[derive(Debug)]` → `#[derive(Debug, Clone)]`（`program` / `args` / `raw` 三字段均可 Clone，无手写 impl） | `kill_tree` / `ProcessOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` / `ExecOutput` 逐字零改（AC-1 / AC-9） |
| `crates/infra/exec/Cargo.toml` | ① `tokio` features `["process"]` → `["process", "io-util", "time", "macros"]`（`io-util` = `read_to_end`；`time` = `sleep` / `timeout`；`macros` = `select!` / `join!`；**不引** `rt` / `rt-multi-thread` / `full`）；② dev-dependencies 补 `tokio = { workspace = true, features = ["rt", "macros", "test-util"] }`（超时 / 取消用例的 `#[tokio::test]` 与 `start_paused` + `advance`） | 依赖方向不变：仍只有 `foundation` + `tokio`（AC-9 / R-7）；特性面逐项注释理由，沿 `checks-runtime` Cargo.toml 既有注释形态 |
| `crates/core/foundation/src/lib.rs` | ① 模块 doc 由「三能力」改为「四能力」（补取消原语一条，并写明「`RunGuard` 写 / `exec::run_shell` 读」的第二消费者正触发）；② `pub mod cancel;` | 依赖面零改（仍只有 `sha2`，AC-9） |
| `crates/core/orchestration/src/control.rs` | ① `use foundation::cancel::CancelFlag;`；② `RunEntry` 增字段 `cancel_flag: CancelFlag`（与 `cancel: watch::Sender<bool>` 并存，注释写明「进程面取消标志」）；③ `begin_run` 铸造 `CancelFlag::never()` 并落条目；④ `request_stop` 双写（`cancel.send_replace(true)` + `cancel_flag.handle().set()`），doc 补「双写 / 幂等 / 无订阅者不丢」；⑤ 新增 `RunGuard::cancel_flag(&self) -> CancelFlag`（除名回落 `never()`） | 既有 `cancelled()` / `cancelled_wait()` / 停等语义零改（D8 / D9） |
| `crates/core/orchestration/src/port.rs` | ① `use foundation::cancel::CancelFlag;`；② `ToolStepRequest` 增 `pub cancel: CancelFlag`（derive 保持不变：`Debug, Clone, PartialEq, Eq`）；③ 三 trait 各加 `cancel: CancelFlag` 形参——`StaticCheckRunner::run(root, name, slot, cancel)`、`TestExecutionRunner::run(root, name, slot, cancel)`、`MutationRunner::run(root, name, files, patch, cancel)`，doc 补「取消经 infra 执行体观测，core 零 spawn」 | `ToolCommand` / `ToolStepOutput` / 各 Outcome 载荷零改（bindings 零 diff，AC-11） |
| `crates/core/orchestration/src/steps.rs` | `execute` 在 `match step.command` 前 `let cancel = step.cancel;`；`StaticCheck` / `TestExecution` / `MutationTest` 三分支各把 `cancel` 移入对应 runner 调用（互斥分支，无需 `clone`）；其余七分支零改 | 「仅做字段透传不做决策」；审计行 / 载荷零改 |
| `crates/core/orchestration/src/walker.rs` | ① `run_tool` 形参插入 `guard: &RunGuard`（`run_tool(tools, guard, root, command, label)`），内部 `ToolStepRequest { root, command, cancel: guard.cancel_flag() }`；② 13 处调用点补实参（`drive` 内传 `guard`，`StepContext` 作用域内传 `ctx.guard`）；③ `step_phase_next(tools, guard, request)`（1 处调用点在 `drive`）、`run_phase_log(tools, guard, request, input)`（8 处调用点，全部在持 `ctx.guard` 的步函数内）增形参并透传；④ doc 补「取消在步边界 `check_cancel` 之外贯通到进程面」 | `check_cancel`（`:221`）、`drive` 循环结构、节点 emit / 边累积零改；`TryFrom<ToolStepOutput>` 转换面零改 |
| `crates/infra/checks/src/static_check.rs` | ① 新增 `pub(crate) const STATIC_CHECK_TIMEOUT: Duration = Duration::from_secs(1200);`（doc：冷构建约 90 s + vp 约 10 s × 3 倍余量）；② `ProcessStaticCheck::run` 形参加 `cancel: CancelFlag`，`execute(root, name, slot, cancel)` 透传；③ spawn 段（现 :63-73）整体替换为 `exec::run_shell(exec::SystemCommand::script(&format!("{command} 2>&1")), exec::RunOptions { cwd: Some(Path::new(root).to_path_buf()), env: Vec::new(), timeout: STATIC_CHECK_TIMEOUT, cancel })`；④ 结局三臂：`RunError` → `Err(format!("static-check 命令{failure}"))`；`Cancelled` → `Err("执行已取消")`；`TimedOut` → `Err(format!("static-check 命令超时（{} 秒）强制终止", STATIC_CHECK_TIMEOUT.as_secs()))`；⑤ `Completed` 臂：`diagnostics = String::from_utf8_lossy(&report.stdout)`（`report.stderr` 恒空，**不读**）、`passed = report.exit_code == 0`、record 取 `report.stdout` | 命令内 `2>&1` **保留**（AC-8 / R-2）；`ensure_program_resolvable` / `report_dir` / 无配置直过分支 / `trim_end` 零改；`std::process::Stdio` 与 `into_tokio` 调用随之删除 |
| `crates/infra/checks/src/testexec/runner.rs` | ① `SUITE_COMMAND_TIMEOUT` 600 s → **1200 s**，doc 由「CLI `runCommand(cmd, cwd, 600000)` 同值」改为「rust suite 实测 340–393 s × 约 3 倍余量（原 600 s 仅 1.7 倍；CLI 同值锚定已失效）」；② `execute_plan(plan, report_dir, cancel: CancelFlag)`；③ spawn + 超时段（现 :89-140）替换为 `run_shell(SystemCommand::script_raw(&command), RunOptions { cwd: Some(plan.cwd.clone()), env: Vec::new(), timeout: SUITE_COMMAND_TIMEOUT, cancel })`；④ 三臂：`RunError` → `Err(format!("测试命令{failure}"))`；`Cancelled` → `return Err("执行已取消".to_owned())`（不落 `execution_error`）；`TimedOut` → `(EXIT_CODE_NO_STATUS, Some(format!("测试命令超时（{} 秒）强制终止", SUITE_COMMAND_TIMEOUT.as_secs())), Vec::new())`；`Completed` → `exit_code = report.exit_code`、`output = report.stdout ++ report.stderr`、非零 → `non_zero_exit_error`；⑤ 删私有 `read_all`（现 :165-170）与 `std::process::Stdio` 依赖 | 合并流顺序（stdout 前缀 + stderr 后缀）与「不加 `2>&1`」逐字保留；`PlanExecution` 字段面 / 工件解析 / 复用门零改（AC-8） |
| `crates/infra/checks/src/testexec/mod.rs` | `impl TestExecutionRunner for ProcessTestExecution` 的 `run(root, change, slot, cancel: CancelFlag)` 增形参；`execute(root, change, slot, cancel)` 形参透传，`runner::execute_plan(plan, &dir, cancel.clone())`（循环内多次 spawn，段间需 clone） | 其余（复用门 / 报告写盘 / `outcome` 装配 / record）零改 |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | ① `run_segment(patch_path: &Path, exec_dir: &Path, cancel: CancelFlag)`；② spawn + 超时段（现 :49-94）替换为 `run_shell(command, RunOptions { cwd: Some(exec_dir.to_path_buf()), env: vec![(OsString::from("CARGO_TARGET_DIR"), exec_dir.join("target").into_os_string())], timeout: Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS), cancel })`；③ 结局四臂：`RunError::Spawn` → `Fatal("cargo mutants 拉起失败（程序不可达？）: {e}；请确认 cargo-mutants 已安装（cargo install cargo-mutants）")`、`RunError::Wait` → `Fatal("cargo-mutants 段执行等待失败: {e}")`（两臂逐字同现状）、`TimedOut` → `Interrupted("cargo-mutants 段执行超时（1800 秒）强制终止")`（文案不变）、`Cancelled` → `Cancelled("cargo-mutants 段执行已取消")`；④ `Completed` 臂：`exit_code = report.exit_code`、`output = stdout ++ stderr`；⑤ 删私有 `read_all`（现 :119-124） | 段报告权威 / 零突变体合成载荷 / `segment_failure_message` 零改；`CARGO_TARGET_DIR` 语义经 `RunOptions.env` 等价承接（AC-8） |
| `crates/infra/checks/src/mutation/stryker.rs` | ① `run_segment(segment: &StrykerSegment<'_>, cancel: CancelFlag)`；② spawn + 超时段（现 :62-104）替换为 `run_shell(SystemCommand::script_raw(&command), RunOptions { cwd: Some(segment.suite_cwd.to_path_buf()), env: Vec::new(), timeout: Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS), cancel })`；③ 结局四臂：`Spawn` → `Fatal("Stryker 命令拉起失败: {e}")`、`Wait` → `Fatal("Stryker 段执行等待失败: {e}")`（逐字同现状）、`TimedOut` → `Interrupted("Stryker 段执行超时（1800 秒）强制终止")`（不变）、`Cancelled` → `Cancelled("Stryker 段执行已取消")`；④ `Completed` 臂同 cargo_mutants 拼流；⑤ 删私有 `read_all`（现 :215-220） | 临时 config 写盘 / 段报告权威 / sandbox 目录纪律零改 |
| `crates/infra/checks/src/mutation/mod.rs` | ① `SegmentFailure` 增第三变体 `Cancelled(String)`，`Display` 三臂同型（单串）；② `impl MutationRunner for ProcessMutation::run(root, name, files, patch, cancel)` 增形参并透传 `execute`；③ `execute(..., cancel)` → `run_suite(..., cancel.clone())` → `execute_segment(..., cancel.clone())` → `cargo_mutants::run_segment(&patch_path, &bucket.exec_dir, cancel)` / `stryker::run_segment(&segment, cancel)`；④ `execute` 的 `interruption: Option<String>` → `Option<SegmentFailure>`，`Err(SegmentFailure::Cancelled(_))` 与 `Interrupted(_)` 同走「retain Stryker 桶临时面 + break」，终端面按变体分文案：`Interrupted` → 既有「突变执行中断：{error}\n已产报告路径：…」，`Cancelled` → 「突变执行已取消：{error}\n已产报告路径：…」，两者 `conclusion = MutationTestConclusion::Error` | 既有 `Fatal` / `Interrupted` 分流、归并块 / 阈值判定 / 复用门 / summary 回填零改（AC-8）；`Fatal` 臂不参与取消（spec scenario「既有分流不变」） |
| `packages/desktop/package.json` | `version` `"0.5.19"` → `"0.5.20"` | AC-11 / D15 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|--------|----------|-----------|----------|------|
| `run_shell` | `crates/infra/exec/src/run.rs`（新；经 `lib.rs` 再导出） | 新增 | `pub async fn run_shell(command: SystemCommand, options: RunOptions) -> Result<RunReport, RunError>` | 唯一执行体。流程：已置位 cancel 短路 → `into_tokio()` + `cwd` / `envs` + `stdin(null)` + 双管道 + `kill_on_drop(true)` → `spawn`（`Err(Spawn)`）→ `select!`（cancel 轮询 / `sleep(timeout)` / 「排水 + `wait()`」future，`biased` 序 cancel → timeout → 退出）→ 被杀两臂 `kill_tree(pid)` + 有界收尸 + 有界排水 → 组 `RunReport` |
| `RunError::fmt`（`Display`） | 同上 | 新增 | `fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result` | `Spawn(e)` → `"拉起失败: {e}"`；`Wait(e)` → `"等待失败: {e}"` |
| `CancelFlag::never` | `crates/core/foundation/src/cancel.rs`（新） | 新增 | `pub fn never() -> CancelFlag` | 恒 false 零值（缺省信号 / 测试默认 / run 已收口回落） |
| `CancelFlag::is_set` | 同上 | 新增 | `pub fn is_set(&self) -> bool` | `Ordering::Relaxed` 读 |
| `CancelFlag::handle` | 同上 | 新增 | `pub fn handle(&self) -> CancelHandle` | 取写端（共享同一原子） |
| `CancelHandle::set` | 同上 | 新增 | `pub fn set(&self)` | `Ordering::Relaxed` 写；幂等 |
| `ChangeFlowControl::begin_run` | `crates/core/orchestration/src/control.rs` | 修改 | `pub fn begin_run(self: &Arc<Self>, root: &str, change_id: &str, run_id: String, started_at: i64) -> Result<RunGuard, String>` | 签名不变；内部铸造 `CancelFlag::never()` 落 `RunEntry.cancel_flag` |
| `ChangeFlowControl::request_stop` | 同上 | 修改 | `pub fn request_stop(&self, root: &str, change_id: &str) -> bool` | 签名不变；双写 watch + `cancel_flag.handle().set()` |
| `RunGuard::cancel_flag` | 同上 | 新增 | `pub fn cancel_flag(&self) -> CancelFlag` | 克隆共享同一原子；run 已除名回落 `CancelFlag::never()` |
| `StaticCheckRunner::run` | `crates/core/orchestration/src/port.rs` | 修改 | `fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture` | 第四形参为取消信号 |
| `TestExecutionRunner::run` | 同上 | 修改 | `fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture` | 同上 |
| `MutationRunner::run` | 同上 | 修改 | `fn run(&self, root: &str, name: &str, files: &[String], patch: &str, cancel: CancelFlag) -> BoxToolFuture` | 末位形参为取消信号 |
| `ProcessStaticCheck::run`（trait impl） | `crates/infra/checks/src/static_check.rs` | 修改 | `fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture` | 透传 `execute` |
| `ProcessTestExecution::run`（trait impl） | `crates/infra/checks/src/testexec/mod.rs` | 修改 | `fn run(&self, root: &str, change: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture` | 透传 `execute` → 逐 plan `execute_plan` |
| `ProcessMutation::run`（trait impl） | `crates/infra/checks/src/mutation/mod.rs` | 修改 | `fn run(&self, root: &str, name: &str, files: &[String], patch: &str, cancel: CancelFlag) -> BoxToolFuture` | 透传 `execute → run_suite → execute_segment` |

<!-- 私有 / pub(crate) 函数不入本表（按规则仅列模块级导出项）：`execute`（三 runner 内）、`execute_plan`、`run_suite`、`execute_segment`、`run_segment`（两段执行）、`drain` / `wait_cancel` / `read_all`（删）、`run_tool` 的形参演进见「修改文件」行。
     无 CLI 子命令 / HTTP 端点变更：本变更不新增 CLI 命令、不触 `plugins/**`、不新增 Tauri IPC 命令（bindings 零 diff）。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `RunOptions`（struct） | `crates/infra/exec/src/run.rs` | 新增 | 字段：`pub cwd: Option<PathBuf>`、`pub env: Vec<(OsString, OsString)>`、`pub timeout: Duration`、`pub cancel: CancelFlag`。`timeout` **必填**（无 `Default`、无「None 即无限」分支）；`derive(Debug, Clone)`。命名导出见 `lib.rs` |
| `RunOutcome`（enum） | 同上 | 新增 | 封闭三值 `Completed` / `TimedOut` / `Cancelled`；`derive(Debug, Clone, Copy, PartialEq, Eq)` |
| `RunReport`（struct） | 同上 | 新增 | 字段：`pub outcome: RunOutcome`、`pub exit_code: i32`（`TimedOut` / `Cancelled` 与「无可读退出码」同记 `-1`）、`pub stdout: Vec<u8>`、`pub stderr: Vec<u8>`（零改写零截断；不预合流）；`derive(Debug, Clone, PartialEq, Eq)` |
| `RunError`（enum） | 同上 | 新增 | `Spawn(String)` = 进程未建立；`Wait(String)` = `child.wait()` io 失败；`derive(Debug, Clone, PartialEq, Eq)` + 手写 `Display`（D1） |
| `CancelFlag`（struct） | `crates/core/foundation/src/cancel.rs` | 新增 | `pub struct CancelFlag(Arc<AtomicBool>)`；`Clone`（共享同一原子）、手写 `Debug`（`set = {bool}`）、手写 `PartialEq`（`Arc::ptr_eq`）+ `Eq`（D7）；读取经 `is_set()` |
| `CancelHandle`（struct） | 同上 | 新增 | `pub struct CancelHandle(Arc<AtomicBool>)`；`Clone` / `Debug`；写入经 `set()`（仅 `CancelFlag::handle()` 可铸造，构造面收口在读端） |
| `ToolStepRequest`（struct） | `crates/core/orchestration/src/port.rs` | 修改 | 增 `pub cancel: CancelFlag`；字段集变 `{ root: String, command: ToolCommand, cancel: CancelFlag }`；derive 保持不变（`Debug, Clone, PartialEq, Eq`，靠 D7 的 identity 相等成立） |
| `SegmentFailure`（enum） | `crates/infra/checks/src/mutation/mod.rs` | 修改 | 增 `Cancelled(String)`：`Fatal` = 工具不可达 / 配置不支持 / 写盘失败 → runner `Err`；`Interrupted` = 段中断（超时 / 报告缺失 / harness 崩）→ `conclusion = Error`；`Cancelled` = 用户停止 → `conclusion = Error` + 取消文案；`Display` 三臂同型 |
| `SystemCommand`（struct） | `crates/infra/exec/src/lib.rs` | 修改 | derive 由 `Debug` 扩为 `Debug, Clone`（字段与不变量零改） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.20`（原 `0.5.19`） | 用户可见变更提升补丁号（AC-11 / D15）；`tauri.conf.json` 经 `../package.json` 跟随 |
| 超时预算（`STATIC_CHECK_TIMEOUT` / `SUITE_COMMAND_TIMEOUT` / `SEGMENT_SPAWN_TIMEOUT_MS`） | `crates/infra/checks/src/{static_check.rs, testexec/runner.rs, mutation/mod.rs}` | 代码常量（非配置面） | `Duration` / `u64` ms | 1200 s / 1200 s / 1_800_000 ms | spec「超时必填与超时预算表」：**不进** `openspec/config.json`、不设用户可配项；调整属代码常量变更（评审可见）（D11） |
| `openspec/config.json` | — | 否（零改动） | — | 既有声明不变 | 无新增 schema 字段、无阈值调整（proposal「不要修改」） |

---

## 数据模型

持久化面**零结构改动**。本变更只新增一处**进程内运行态**字段与两种内存载荷：

```
core/orchestration::ChangeFlowControl（进程内注册表，不落库）
└── RunEntry
    ├── cancel: watch::Sender<bool>        ← 既有：停等 / 应答等待的异步观测面（零改）
    └── cancel_flag: CancelFlag            ← 新增：进程面取消标志（Arc<AtomicBool>）
          │  Clone ─────────────────────────────────────────────┐
          ▼                                                      ▼
   ToolStepRequest.cancel ──► 三 runner trait 形参 ──► RunOptions.cancel
                                                          │
                                                    exec::run_shell（读端轮询 200 ms）
```

| 面 | 载体 | 语义 / 生命周期 |
|----|------|-----------------|
| run 级取消标志 | `RunEntry.cancel_flag: CancelFlag` | `begin_run` 铸造（`never()`，恒 false）；`request_stop` 置位；`finish` 除名后标志本身仍随 `Arc` 存活至最后一个持有者（在跑的 runner）释放——置位不因除名丢失，反向也不再置位（无运行面） |
| 步级携带 | `ToolStepRequest.cancel: CancelFlag` | 每步构造时从 `guard.cancel_flag()` 克隆（共享同一原子），经 `steps.rs` 原样透传三 runner |
| 执行报告 | `RunReport { outcome, exit_code, stdout, stderr }` | 单次 `run_shell` 的内存产出，不持久化；`exit_code = -1` 只在 `TimedOut` / `Cancelled` / 无可读码三处出现（哨兵，非 0 回落） |
| 输出记录（不变） | `<report_dir>/exec-outputs/<exec_id>.log` | 单一流全量落盘（static-check = `2>&1` 交错流；testexec = 各 plan 输出按序聚合；mutation 段报告仍走 json 文件面）。本变更只换「谁 spawn / 谁排水」，`record_exec_output` / `read_exec_output` / `mint_exec_id` 与 `.log` 形态逐字不变 |
| 报告树（不变） | `reports/<slot>/<plan_id>/{report.json, results.txt, coverage*.json}`、`summary.json` | 字段集零改：`exit_code` 仍为 `-1` 表示超时 / 缺码；`error` 面文案骨架不变；`conclusion` 三值不变。取消面 test-execution **不落** `execution_error`（走 `Err`），mutation 取消面落 `conclusion = Error` + `feedback_text` |

无新增关系 / 无索引 / 无迁移脚本：`RunEntry.cancel_flag` 随 run 生命周期在内存中存在，`Run` 运行史（`run_history`）与 db schema 零改动。

---

<!-- 路由 / API 设计：本变更不涉及。无新增 / 修改 Tauri IPC 命令、无 HTTP 端点、无 CLI 子命令；`RunUpdate` / 节点词汇 / 快照 DTO 零扩展，`src/types/generated/bindings.ts` 零 diff（AC-11）。 -->

---

## 依赖

### 运行时依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| `exec` → `tokio` features | `["process"]` → `["process", "io-util", "time", "macros"]` | `process` = `Command` / `Child` / `ChildStdout`；`io-util` = `AsyncReadExt::read_to_end`；`time` = `sleep` / `timeout`；`macros` = `select!` / `join!`。**不引** `rt` / `rt-multi-thread` / `full`（设计不 `tokio::spawn`，D6） |
| `exec` → `foundation` | 不变（既有边） | `CancelFlag`（取消读端）+ 既有 `layout::ReportSlot` |
| `foundation` | 零新增（仅 std `Arc` / `AtomicBool`） | 取消原语零 tokio、零 workspace 内依赖（AC-9） |
| `orchestration` → `foundation` | 不变（既有边） | `CancelFlag`（写端与 port 载荷） |
| `checks-runtime` → `exec` / `foundation` | 不变（既有边） | 四调用点的执行体与取消信号 |
| 新增第三方 crate | **无** | 取消原语 std-only；执行体复用既有 tokio |

### 构建 / 测试依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| `exec` dev-dependencies | 补 `tokio = { workspace = true, features = ["rt", "macros", "test-util"] }` | `#[tokio::test]`（`rt`）、`select!` 用例（`macros`）、超时 / 取消用例的 `start_paused` + 手动 `advance`（`test-util`，沿 `mutation/*_test.rs` 既有先例）；既有 `tempfile` 保留 |
| `checks-runtime` dev-dependencies | 不变 | 已有 `tokio(test-util)` / `workflow` / `tempfile` |
| `orchestration` dev-dependencies | 不变 | 已有 `store` / `tempfile` / `tokio(rt, macros)` |

---

## 验收标准回应

| AC | 设计落点 |
|----|----------|
| AC-1 | 四调用点 spawn 段整体替换为 `exec::run_shell`（「修改文件」四行）；`read_all` 三处删除；四文件内不留 `tokio::time::timeout` / `start_kill`（`kill_tree` 单点驻 exec） |
| AC-2 | `RunOptions.timeout: Duration` 必填、`RunOptions` 无 `Default`（类型定义表）；四预算常量 = 1200 / 1200 / 1800 / 1800 s（D11、配置表） |
| AC-3 | `TimedOut` → `exit_code = -1`（`RunReport` 行）；超时臂 `kill_tree(pid)` + 有界收尸 / 排水（D5）→ Windows `taskkill /T /F` 整树回收；static-check 首获 1200 s 预算（D11） |
| AC-4 | `Cancelled` → `exit_code = -1`；入口短路 + 轮询置位两径（D3 / D4）；`CancelFlag::never()` 恒 false（`never` 行） |
| AC-5 | 双管道并发 `join!(drain, drain)` 排水（`run_shell` 行）；`cwd` / `env` 经 `RunOptions` 施加、`SystemCommand` 可 `Clone`（类型定义表） |
| AC-6 | `request_stop` 双写 + `RunEntry.cancel_flag` + `RunGuard::cancel_flag()`（D8 / D9）；`ToolStepRequest.cancel`（类型表）；三 trait 形参（公共函数表）；`run_tool` 单点填充（D10） |
| AC-7 | 取消 → `Err("执行已取消")` → `run_tool` 包 `Terminal::failed("{label} 失败: 执行已取消")` → 该步 failed、run failed（D12）；进程面秒级收敛由 `kill_tree` + 有界收尾保证（D5） |
| AC-8 | static-check 命令内 `2>&1` 保留、`stderr` 不读（static_check 修改行）；testexec / mutation 合流顺序由调用方拼（D14）；四调用点失败文案逐字保持（D2）；`ProcessOutput` / `record_exec_output` / `report.json` / `summary.json` / 复用门零改（数据模型表） |
| AC-9 | `foundation` 零 tokio 零 workspace 内依赖（依赖表）；`exec` 特性面只扩四项（R-7 留痕于 Cargo.toml 注释） |
| AC-10 | 签名演进清单完整（公共函数表 15 项）；测试面实量 = 26 处 `ToolStepRequest` 字面量 + 13 处测试 trait impl（「提案与规格同步状态」末段，测试相位承接） |
| AC-11 | `package.json` 0.5.19 → 0.5.20（配置表）；`plugins/**` 与 bindings 零改动（只读红线段） |

---

## 待决问题

1. **规格修订：`run_shell` 的失败面（须在规格相位回写）**。`desktop-exec-runtime` 的 Module Contract 行与「统一执行体 run_shell 契约」均写 `-> RunReport`，与本设计的 `-> Result<RunReport, RunError>` 不一致（D1）。建议修订：签名改 `-> Result<RunReport, RunError>`，`RunError` 补入类型清单，并新增 scenario「进程拉起失败 → `Err(RunError::Spawn)`（调用点各自映射既有 `Err` / `Fatal` 文案）」。若评审坚持 `-> RunReport`：备选形态是在 `RunReport` 增 `spawn_error: Option<String>`（改动更大，且弱化「三值收口 + 哨兵」的解读单点）——需裁决。
2. **`FINISH_GRACE` 取值**。拟定 2 s（收尸与排水各自有界）。若评审偏好与 `sdk/bash.rs` 完全同形（杀后无界 `wait` / 排水），则 R-8「杀后悬挂」风险回归，本设计不建议；若 2 s 对慢机偏紧可上调，属常量取值（不影响任何验收面）。
3. **`run_tool` 形参取法**（D10）。现取 `guard: &RunGuard`（严格满足 scenario「各步函数不各自取值」）；若评审以 Module Contract 行字面为准，改 `cancel: CancelFlag` 为机械替换 13 处调用点，语义不变。
4. **unix 侧进程组补强**（R-4）。`kill_tree` unix 臂为 `kill -9 <pid>`（只杀直接子进程）；proposal 裁决「本篇不改、留痕」。若评审希望本篇顺手补（`setsid` + `kill -9 -pgid`），属 `exec::kill_tree` 契约变更、需回到提案范围裁决，本设计不预铺。
5. **static-check 超时文案**。拟 `"static-check 命令超时（1200 秒）强制终止"`（与 testexec / mutation 两处既有「…超时（N 秒）强制终止」同型，界面可见）。若产品侧希望与 testexec 的「测试命令超时（1200 秒）强制终止」区分度更高，仅改字面量。
