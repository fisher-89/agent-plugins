# 任务: exec-run-shell

> 覆盖 proposal「变更范围 - 实现文件」15 项 + 本设计补入的 `crates/infra/checks/src/testexec/mod.rs`（port 实现签名随动）= 16 项（2 新增 + 14 修改）。**本相位不改测试文件**：`run_test.rs` / `cancel_test.rs` 的新建、`static_check_test.rs` / `runner_test.rs` / `mutation/*_test.rs` / `control_test.rs` / `walker_test.rs` 的用例改写、26 处 `ToolStepRequest` 字面量与 13 处测试 trait impl 的形参补齐，全部归 test-design / test-gen / test-execution 相位（清单见 design「提案与规格同步状态」末段）。
>
> 依赖序要点：**阶段一**（`foundation::cancel`）解锁二、三；**阶段二**（`exec::run_shell`）与**阶段三**（取消贯通）互不依赖但都先于**阶段四**（四 spawn 点迁移——同时需要 `run_shell` 与带 `cancel` 的 trait 签名）。阶段三完成、阶段四未完成之间工作区**不通过编译**（trait 形参先行、实现未随动），属既定中间态，不得为「编译绿」回退签名。
>
> 不预铺：无 config schema 扩展、无 bindings 重生成、无 `plugins/**` 改动、无前端改动、无 db 迁移。

## 阶段一：foundation 取消原语（`cancel.rs`）

- [x] 新建 `packages/desktop/src-tauri/crates/core/foundation/src/cancel.rs`：`CancelFlag(Arc<AtomicBool>)`（`#[derive(Clone)]` + 手写 `Debug`（`CancelFlag(set = {bool})`）+ 手写 `PartialEq`（`Arc::ptr_eq`）+ `Eq`）与 `CancelHandle(Arc<AtomicBool>)`（`#[derive(Clone)]` / `Debug`）
- [x] 同文件实现 `CancelFlag::never() -> CancelFlag`、`CancelFlag::is_set(&self) -> bool`（`Ordering::Relaxed`）、`CancelFlag::handle(&self) -> CancelHandle`（克隆同一 `Arc`）；`CancelHandle::set(&self)`（`Ordering::Relaxed`，幂等）
- [x] 同文件写模块 doc：为何落 `foundation`（「第二个消费者到场才下沉」正触发 = `RunGuard` 写 / `exec::run_shell` 读）、为何是 std-only 轮询而非 `Notify`（零 tokio 纪律，D3）、`PartialEq` 是**身份比较非状态比较**（D7）
- [x] 修改 `crates/core/foundation/src/lib.rs`：`pub mod cancel;` + 模块 doc 由「三能力」改为「四能力」（补取消原语一条，注明两消费者）
- [x] 自查 阶段一：`crates/core/foundation/Cargo.toml` **零改动**（仍只有 `sha2`）；`grep -n "tokio\|Arc<Mutex" crates/core/foundation/src/cancel.rs` 零命中（AC-9）

## 阶段二：exec 统一执行体（`run.rs` + 导出面 + 依赖面，依赖阶段一）

- [x] 修改 `crates/infra/exec/Cargo.toml`：`tokio` features `["process"]` → `["process", "io-util", "time", "macros"]`（逐项注释理由：`read_to_end` / `sleep`+`timeout` / `select!`+`join!`；显式注明不引 `rt` 与 `rt-multi-thread`——设计不 `tokio::spawn`）；dev-dependencies 补 `tokio = { workspace = true, features = ["rt", "macros", "test-util"] }`
- [x] 新建 `packages/desktop/src-tauri/crates/infra/exec/src/run.rs` 的类型面：`RunOptions { cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration, cancel: CancelFlag }`（`derive(Debug, Clone)`，**不实现 `Default`**，`timeout` 无缺省语义）、`RunOutcome { Completed, TimedOut, Cancelled }`、`RunReport { outcome, exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8> }`、`RunError { Spawn(String), Wait(String) }` + 手写 `Display`（`"拉起失败: {e}"` / `"等待失败: {e}"`，D1）
- [x] 同文件实现 `run_shell(command: SystemCommand, options: RunOptions) -> Result<RunReport, RunError>`：① 入口 `cancel.is_set()` 短路（零 spawn → `Cancelled` / `exit_code = -1`，D4）；② `command.into_tokio()` 上施加 `cwd` / `envs` / `stdin(Stdio::null())` / 双管道 / `kill_on_drop(true)`；③ `spawn()` 失败 → `Err(RunError::Spawn(error.to_string()))`；④ `select!`（`biased` 序：cancel 轮询 → `sleep(timeout)`（`tokio::pin!`）→「`join!(drain, drain)` + `child.wait()`」future）三臂竞速；⑤ 被杀两臂 `exec::kill_tree(pid)` → 有界收尸 `timeout(FINISH_GRACE, child.wait())` → 有界排水 `timeout(FINISH_GRACE, join!(drain, drain))` → `RunReport { outcome, exit_code: -1, stdout, stderr }`（D5）；⑥ 正常臂 `exit_code = status.code().unwrap_or(-1)`，`wait()` 失败 → `Err(RunError::Wait(...))`
- [x] 同文件实现私有助手与常量：`drain(pipe: &mut Option<impl AsyncRead + Unpin>, buffer: &mut Vec<u8>)`（`read_to_end` 至 EOF，错误忽略——沿三份退役 `read_all` 语义）、`wait_cancel(cancel: &CancelFlag)`（`while !is_set() { sleep(CANCEL_POLL_INTERVAL) }`）、`CANCEL_POLL_INTERVAL = 200 ms`、`FINISH_GRACE = 2 s`（各带理由注释）
- [x] 关键形态守住：两个输出缓冲区与 `child` / 两条管道**落 `run_shell` 局部**（future 只持可变借用，不 `move`）→ 杀后仍可收尸与继续排水，且 `exec` 无需 `rt` 特性（D6）
- [x] 修改 `crates/infra/exec/src/lib.rs`：`mod run;` + `pub use run::{run_shell, RunError, RunOptions, RunOutcome, RunReport};`；`SystemCommand` `#[derive(Debug)]` → `#[derive(Debug, Clone)]`；头部模块 doc 补一条能力项「进程执行体 `run_shell`」
- [x] 自查 阶段二：`RunOptions` 无 `Default`、`timeout` 无缺省 / 无「None 即无限」分支（AC-2）；`run.rs` 内 `tokio::spawn` 零命中；`kill_tree` / `ProcessOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` **零字节改动**（AC-1 / AC-8）；`cargo tree -p exec` 仍只有 `foundation` + `tokio`（AC-9）

## 阶段三：取消贯通（control → port → steps → walker，依赖阶段一）

- [x] 修改 `crates/core/orchestration/src/control.rs`：`use foundation::cancel::CancelFlag;`；`RunEntry` 增 `cancel_flag: CancelFlag`（注释「进程面取消标志——与 `cancel` watch 同道，无订阅者也不丢位」）；`begin_run` 铸造 `CancelFlag::never()` 落条目；`request_stop` 双写（`entry.cancel.send_replace(true)` + `entry.cancel_flag.handle().set()`）并补 doc（双写 / 幂等 / 无订阅者不丢，D9）；新增 `RunGuard::cancel_flag(&self) -> CancelFlag`（克隆共享原子；run 已除名回落 `CancelFlag::never()`，D8）
- [x] 修改 `crates/core/orchestration/src/port.rs`：`use foundation::cancel::CancelFlag;`；`ToolStepRequest` 增 `pub cancel: CancelFlag`（derive 保持 `Debug, Clone, PartialEq, Eq`，D7）；三 trait 各加末位形参——`StaticCheckRunner::run(root, name, slot, cancel)`、`TestExecutionRunner::run(root, name, slot, cancel)`、`MutationRunner::run(root, name, files, patch, cancel)`，doc 补「取消信号经 infra 执行体观测，core 零 spawn」
- [x] 修改 `crates/core/orchestration/src/steps.rs`：`execute` 在 `match step.command` 之前 `let cancel = step.cancel;`；`StaticCheck` / `TestExecution` / `MutationTest` 三分支把 `cancel` 移入对应 runner 调用（互斥分支，零 `clone`）；其余七分支与审计行零改
- [x] 修改 `crates/core/orchestration/src/walker.rs` 的透传单点：`run_tool` 形参插入 `guard: &RunGuard`（`run_tool<V>(tools, guard, root, command, label)`），内部 `ToolStepRequest { root: root.to_owned(), command, cancel: guard.cancel_flag() }`（D10）；doc 补「取消在步边界 `check_cancel` 之外贯通到进程面」
- [x] 同文件补齐 13 处 `run_tool` 调用点实参（`:270` / `:707` / `:728` / `:949` / `:1070` / `:1145` / `:1207` / `:1293` / `:1718` / `:2155` / `:2915` / `:3111` / `:3195`）：`drive` 内传 `guard`，`StepContext` 作用域内传 `ctx.guard`
- [x] 同文件 `step_phase_next(tools, guard: &RunGuard, request)`（`:703`，`drive` 内 1 处调用补实参）与 `run_phase_log(tools, guard: &RunGuard, request, input)`（`:1713`，8 处调用点 `:853/:1541/:1563/:1596/:1761/:1914/:1987/:2052` 全部补 `ctx.guard`）增形参并透传给 `run_tool`
- [x] 自查 阶段三：`grep -n "run_tool(" crates/core/orchestration/src/walker.rs` 命中数与 `ToolStepRequest {` 在 walker 内**恰一处**（单点填充，AC-6）；`grep -n "cancel_flag()" crates/core/orchestration/src/walker.rs` 只在 `run_tool` 体内命中；`control.rs` 的 `cancelled()` / `cancelled_wait()` / `wait_confirm` / `wait_answer` 零语义改动

## 阶段四：四 spawn 点迁移 + 预算校准（依赖阶段二、三）

- [x] 修改 `crates/infra/checks/src/static_check.rs`：新增 `pub(crate) const STATIC_CHECK_TIMEOUT: Duration = Duration::from_secs(1200);`（doc：冷构建约 90 s + vp 约 10 s × 3 倍余量）；`ProcessStaticCheck::run` 形参加 `cancel: CancelFlag`；`execute(root, name, slot, cancel)` 透传；spawn 段（现 `:63-73`）整体换为 `exec::run_shell(exec::SystemCommand::script(&format!("{command} 2>&1")), exec::RunOptions { cwd: Some(Path::new(root).to_path_buf()), env: Vec::new(), timeout: STATIC_CHECK_TIMEOUT, cancel })`
- [x] 同文件补结局三臂：`Err(failure)` → `Err(format!("static-check 命令{failure}"))`；`Cancelled` → `Err("执行已取消".to_owned())`；`TimedOut` → `Err(format!("static-check 命令超时（{} 秒）强制终止", STATIC_CHECK_TIMEOUT.as_secs()))`；`Completed` 臂 `diagnostics = String::from_utf8_lossy(&report.stdout)`（**不读** `report.stderr`）、`passed = report.exit_code == 0`、record 取 `report.stdout`；删除 `std::process::Stdio` import 与 `into_tokio` / `.output()` 调用
- [x] 修改 `crates/infra/checks/src/testexec/runner.rs`：`SUITE_COMMAND_TIMEOUT` → `Duration::from_secs(1200)` 并改写 doc（「实测 340–393 s × 约 3 倍余量」；「CLI `runCommand(…, 600000)` 同值」锚定口径退役）；`execute_plan(plan, report_dir, cancel: CancelFlag)`；spawn + 超时段（现 `:89-140`）换为 `run_shell(script_raw(&command), RunOptions { cwd: Some(plan.cwd.clone()), env: Vec::new(), timeout: SUITE_COMMAND_TIMEOUT, cancel })`
- [x] 同文件补结局三臂：`Err(failure)` → `Err(format!("测试命令{failure}"))`；`Cancelled` → `return Err("执行已取消".to_owned())`（**不落** `execution_error`）；`TimedOut` → `(EXIT_CODE_NO_STATUS, Some(format!("测试命令超时（{} 秒）强制终止", SUITE_COMMAND_TIMEOUT.as_secs())), Vec::new())`；`Completed` → `exit_code = report.exit_code`、`output = report.stdout ++ report.stderr`、非零 → `non_zero_exit_error`；删除私有 `read_all`（现 `:165-170`）与 `Stdio` import
- [x] 修改 `crates/infra/checks/src/testexec/mod.rs`：`ProcessTestExecution::run(root, change, slot, cancel)` 增形参；`execute(root, change, slot, cancel)` 透传，`execute_plan(plan, &dir, cancel.clone())`（逐 plan 循环内多次 spawn）
- [x] 修改 `crates/infra/checks/src/mutation/cargo_mutants.rs`：`run_segment(patch_path, exec_dir, cancel)`；spawn + 超时段（现 `:49-94`）换为 `run_shell(command, RunOptions { cwd: Some(exec_dir.to_path_buf()), env: vec![(OsString::from("CARGO_TARGET_DIR"), exec_dir.join("target").into_os_string())], timeout: Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS), cancel })`
- [x] 同文件补结局四臂（文案逐字同现状）：`RunError::Spawn(e)` → `Fatal("cargo mutants 拉起失败（程序不可达？）: {e}；请确认 cargo-mutants 已安装（cargo install cargo-mutants）")`、`RunError::Wait(e)` → `Fatal("cargo-mutants 段执行等待失败: {e}")`、`TimedOut` → `Interrupted("cargo-mutants 段执行超时（1800 秒）强制终止")`、`Cancelled` → `Cancelled("cargo-mutants 段执行已取消")`；`Completed` 臂拼 `stdout ++ stderr`、`exit_code = report.exit_code`；删除私有 `read_all`（现 `:119-124`）与 `Stdio` import
- [x] 修改 `crates/infra/checks/src/mutation/stryker.rs`：`run_segment(segment, cancel)`；spawn + 超时段（现 `:62-104`）换为 `run_shell(script_raw(&command), RunOptions { cwd: Some(segment.suite_cwd.to_path_buf()), env: Vec::new(), timeout: Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS), cancel })`；结局四臂 `Spawn` → `Fatal("Stryker 命令拉起失败: {e}")` / `Wait` → `Fatal("Stryker 段执行等待失败: {e}")` / `TimedOut` → `Interrupted("Stryker 段执行超时（1800 秒）强制终止")` / `Cancelled` → `Cancelled("Stryker 段执行已取消")`；`Completed` 臂拼流；删除私有 `read_all`（现 `:215-220`）与 `Stdio` import
- [x] 修改 `crates/infra/checks/src/mutation/mod.rs`：`SegmentFailure` 增第三变体 `Cancelled(String)`，`Display` 三臂同型（单串）；`ProcessMutation::run(root, name, files, patch, cancel)` 增形参并透传 `execute`；`execute(..., cancel)` → `run_suite(..., cancel.clone())` → `execute_segment(..., cancel.clone())` → 两段 `run_segment(..., cancel)`
- [x] 同文件把中断槽 `interruption: Option<String>` 改 `Option<SegmentFailure>`：`Err(SegmentFailure::Cancelled(_))` 与 `Interrupted(_)` 同走「retain Stryker 桶临时面 + break」，终端面按变体分文案（`Cancelled` → 「突变执行已取消：{error}\n已产报告路径：{…}{retained}」；`Interrupted` → 既有「突变执行中断：…」），二者 `conclusion = MutationTestConclusion::Error`；`Fatal` 臂零改（不参与取消）
- [x] 自查 阶段四：`grep -rn "async fn read_all\|start_kill\|tokio::time::timeout" crates/infra/checks/src/{static_check.rs,testexec/runner.rs,mutation/cargo_mutants.rs,mutation/stryker.rs}` **零命中**（AC-1）；四文件均出现 `exec::run_shell` 且各传 `RunOptions`；static-check 命令串仍为 `{command} 2>&1`（AC-8）

## 阶段五：版本面

- [x] 修改 `packages/desktop/package.json`：`version` `"0.5.19"` → `"0.5.20"`（单行；`tauri.conf.json` 经 `../package.json` 跟随，`src-tauri/Cargo.toml` 与 `plugins/**` 不随动）

## 阶段六：边界与语义自查（grep / diff 核对）

- [x] 核对 `git status` 仅含 16 项实现文件（2 新增 + 14 修改，见 design「新增文件」/「修改文件」）——`crates/infra/agent/**`、`crates/infra/vcs/**`（除测试相位）、`crates/core/checks/**`、`crates/core/workflow/**`、`crates/core/orchestration/src/state.rs`、`crates/infra/checks/src/testexec/detect.rs`、`crates/infra/exec/src/{output,shell}.rs`、`packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**` 全部零改动
- [x] 确认 `RunOptions` 无 `Default`、四调用点预算常量分别锚定 1200 / 1200 / 1800 / 1800 s（AC-2）；`SEGMENT_SPAWN_TIMEOUT_MS` 值未变（1_800_000）
- [x] 确认取消链 grep 面：`ToolStepRequest {` 在 `walker.rs` 恰一处且含 `cancel: guard.cancel_flag()`；`request_stop` 内 `send_replace(true)` 与 `cancel_flag.handle().set()` 双写并存；三 trait 形参 `cancel: CancelFlag` 齐备（AC-6）
- [x] 确认 `foundation` 依赖面零改动、`exec` 依赖方向仍只有 `foundation` + `tokio`（`cargo tree -p exec -p foundation`）；`openspec/config.json` 阈值与 suite 声明零改（AC-9）
- [x] 确认载荷零变 → `bindings:check` 预期零 diff（`ToolStepOutput` / 各 Outcome 字段集零改；`SegmentFailure` 不进 IPC，`RunReport` 不出 crate）（AC-11）
- [x] 记录测试相位承接清单（不在本相位执行）：`run_test.rs` / `cancel_test.rs` 新建；`超时常量锚定` 期望 600 → 1200 且注释改写；26 处 `ToolStepRequest` 字面量补 `cancel: CancelFlag::never()`；13 处测试 trait impl 补 `cancel` 形参；`static_check_test` 的超时 / 取消两条；`runner_test` 的取消一条；`mutation/mod_test` 的取消一条；两段测试的 pid 断言扩展；`control_test` 双写一条；`walker_test` 停止秒级收敛一条
- [x] 记录待决问题 1 的规格回写请求（`run_shell -> Result<RunReport, RunError>` + `RunError` 类型 + 拉起失败 scenario），供规格相位处理；本条不阻塞实现
