# 测试设计: exec-run-shell

> **变更**: exec-run-shell
> **日期**: 2026-10-21
> **基于**: `proposal.md`（验收标准 AC-1 ~ AC-11；「变更范围 - 实现文件」16 项 +「变更范围 - 测试文件」表）、`design.md`（「新增文件」2 项 /「修改文件」14 项；「公共函数 / API」子表 15 行；「类型定义」子表 9 行；关键设计决策 D1 ~ D15）
> **被测模块来源**: `design.md`「新增文件」2 项（`crates/infra/exec/src/run.rs`、`crates/core/foundation/src/cancel.rs`）+「修改文件」14 项（`crates/**` 12 个 Rust 实现文件、`crates/infra/exec/Cargo.toml`、`packages/desktop/package.json`）；无「删除文件」子表。
> **测试框架**: 沿用仓库既有约定——Rust 侧 `#[test]` / `#[tokio::test]`，测试模块以同目录兄弟文件承载（`#[cfg(test)] #[path = "<name>_test.rs"] mod <name>_test;`，`foundation` 用 `#[cfg(test)] mod <name>_test;`），断言用标准 `assert!` / `assert_eq!` / `assert!(matches!(..))`，测试函数名为中文标识符、断言消息为中文；无外部断言库、无独立测试运行器。

用例表读法：`路径类型` ∈ {正向, 异常, 边界}；`迭代类型` ∈ {单值, 参数化, 表驱动, 遍历枚举}。跨模块链路用例按「链路发起方 / 最上层调用方」挂靠入口模块的 `#### 用例` 表（本变更组合锚分别挂靠 `walker.rs`——AC-7 停止收敛、`mutation/mod.rs`——取消贯穿段执行、`static_check.rs` / `testexec/mod.rs`——exec 执行体消费面），不设独立集成测试章节。

边界取值口径（按 design「类型定义」表的 Rust 类型逐项收敛）：`Duration`（`RunOptions.timeout`）取「极小（1 ms 级，需虚拟时钟）/ 预算常量值 / 必填不可缺省」三档；`i32`（`RunReport.exit_code` / `PlanExecution.exit_code`）取 `0` / 谱内非零（`1` / `3` / `101` / `255`）/ `-1` 哨兵；`Vec<(OsString, OsString)>`（`env`）取「空 / 单元素 / 大清单（≥50 项）/ 重复键覆盖」四档；`Option<PathBuf>`（`cwd`）取 `Some(存在)` / `Some(不存在)` / `None` 三档；`Vec<u8>`（`stdout` / `stderr`）取「空 / 单字节 / 大块（>64 KiB 管道缓冲）/ 非 UTF-8 原始字节」四档；`String` / `&str`（`RunError` 载荷、命令串、诊断文本）取「空 / 全空白 / 超长（>1000 字符）/ 特殊字符（引号、换行、emoji）」四档；enum（`RunOutcome` 三值、`RunError` 两变体、`SegmentFailure` 三变体）逐个取值遍历（封闭集无非法值臂，非法值属编译期不可表达）。`i32` 的 `i32::MAX` / 信号终止形态、真实 `child.wait()` io 失败、unix 进程树击杀等不可稳定构造面记入 `## 不可测试项`。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------|
| AC-1 | `exec::run_shell` 为四类 spawn 唯一执行体：四文件内无裸 `tokio::time::timeout` + `start_kill` 组合、无私有 `read_all` | `packages/desktop/src-tauri/crates/infra/exec/src/run.rs` -> `.../exec/src/run_test.rs`（唯一执行体契约）、`.../crates/infra/checks/src/static_check.rs` -> `.../checks/src/static_check_test.rs`、`.../checks/src/testexec/runner.rs` -> `.../testexec/runner_test.rs`、`.../checks/src/mutation/cargo_mutants.rs` -> `.../mutation/cargo_mutants_test.rs`、`.../checks/src/mutation/stryker.rs` -> `.../mutation/stryker_test.rs`；文本残留面见 `## 不可测试项` 第 11 项 |
| AC-2 | `RunOptions.timeout` 必填（无 `Default`），四调用点预算常量锚定 1200 / 1200 / 1800 / 1800 s | `.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（`RunOptions` 字段面）、`.../checks/src/static_check.rs` -> `.../static_check_test.rs`（`STATIC_CHECK_TIMEOUT`）、`.../testexec/runner.rs` -> `.../runner_test.rs`（`SUITE_COMMAND_TIMEOUT` 600 → 1200）、`.../mutation/cargo_mutants.rs` -> `.../cargo_mutants_test.rs` + `.../mutation/stryker.rs` -> `.../stryker_test.rs`（`SEGMENT_SPAWN_TIMEOUT_MS` 1_800_000）；「无 `Default`」编译期面见 `## 不可测试项` 第 4 项 |
| AC-3 | 超时 → `RunOutcome::TimedOut` + `exit_code == -1`；被 spawn 进程与子孙回收（pid 文件断言）；static-check 到点不再悬挂 | `.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（超时臂 + pid 消失 + 墙钟上界）、`.../checks/src/static_check.rs` -> `.../static_check_test.rs`（1200 s 超时 → `Err`）、`.../mutation/cargo_mutants_test.rs` / `.../mutation/stryker_test.rs`（既有超时用例扩展 pid 断言）；unix 树断言边界见 `## 不可测试项` 第 10 项 |
| AC-4 | 取消 → `RunOutcome::Cancelled` + `exit_code == -1` + 目标 pid 消失；`CancelFlag::never()` 恒 false | `.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（入口短路 + 轮询置位两径）、`.../crates/core/foundation/src/cancel.rs` -> `.../foundation/src/cancel_test.rs`（`never` / `is_set` / `handle` / `set`） |
| AC-5 | 双管道并发排水不互锁不丢字节；`RunOptions` 的 `cwd` / `env` 生效；`SystemCommand` 可 `Clone` | `.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（双管道 + `cwd` + `env`）、`.../exec/src/lib.rs` -> `.../exec/src/lib_test.rs`（`SystemCommand` clone 等价） |
| AC-6 | 取消贯通全链：`request_stop` 双写且无订阅者不丢、`ToolStepRequest.cancel`、三 runner trait 带 `cancel`、`run_tool` 单点透传 `guard.cancel_flag()` | `.../crates/core/orchestration/src/control.rs` -> `.../control_test.rs`（双写 / 幂等 / 无订阅者）、`.../port.rs` -> `.../port_test.rs`（`ToolStepRequest` 与三 trait 形参）、`.../steps.rs` -> `.../steps_test.rs`（三分支透传）、`.../walker.rs` -> `.../walker_test.rs`（`run_tool` 单点填充）、`.../checks/src/{static_check.rs,testexec/mod.rs,mutation/mod.rs}` 的 trait impl 面；编译期随动面见 `## 不可测试项` 第 12 项 |
| AC-7 | 用户 stop 运行中的检查步 → 子进程被回收、run 秒级收敛 failed（文案含「执行已取消」），不做 run stopped | `.../crates/core/orchestration/src/walker.rs` -> `.../walker_test.rs`（组合锚：挂起假 runner + stop + 秒级断言）、`.../checks/src/mutation/mod.rs` -> `.../mutation/mod_test.rs`（取消 → `conclusion = Error` + 反馈文案） |
| AC-8 | 既有语义零变：static-check 命令内 `2>&1` 保留、合并流顺序不变、失败文案逐字不变、报告 / summary / 复用门不受影响 | `.../checks/src/static_check.rs` -> `.../static_check_test.rs`、`.../testexec/runner.rs` -> `.../runner_test.rs`、`.../checks/src/mutation/{cargo_mutants.rs,stryker.rs,mod.rs}` -> 对应 `*_test.rs`、`.../checks/src/testexec/mod.rs` -> `.../testexec/mod_test.rs`（既有用例全绿 + 新增断言） |
| AC-9 | `foundation::cancel` 零 tokio、零新增 workspace 内依赖；`exec` 依赖方向不变（特性面只扩 `io-util` / `time` / `macros`） | `.../crates/core/foundation/src/cancel.rs` -> `.../cancel_test.rs`（纯 std 行为面）；`.../crates/infra/exec/Cargo.toml` 与 `.../crates/core/foundation/src/lib.rs` 无测试路径 → `## 不可测试项` 第 2、3、13 项 |
| AC-10 | 全部既有测试通过（含 13 处假实现签名 + ~26 处 `ToolStepRequest` 字面量随动），覆盖率不低于阈值 | 各文件 -> `*_test.rs`（本文件全部用例）+ 门禁统计面 → `## 不可测试项` 第 14 项 |
| AC-11 | `packages/desktop/package.json` 0.5.19 → 0.5.20；`plugins/**` 零改动零 bump；bindings 零 diff | `packages/desktop/package.json`（数据面，无测试路径）→ `## 不可测试项` 第 1 项 |

> 被测范围边界：`crates/infra/agent/**`（含形态参考 `sdk/bash.rs`）、`crates/core/checks/**`、`crates/core/workflow/**`、`crates/core/orchestration/src/state.rs`、`crates/infra/checks/src/testexec/detect.rs`、`crates/infra/exec/src/{output,shell}.rs`、`crates/infra/vcs/src/{lib,git,bootstrap}.rs`（仅 `lib_test.rs` 假件签名随动）、前端 `packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**` 均为只读红线（design「只读红线零触点」），不纳入被测范围。`exec::kill_tree` / `ProcessOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` 语义零改，故 `exec/src/{output,shell}*` 不出现在本文件任何映射对中。
>
> 路径省略说明：表格内 `.../` 前缀补齐为 `packages/desktop/src-tauri/crates/`；下文 `## 单元测试` 各节标题给全路径。

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/exec/src/run.rs -> packages/desktop/src-tauri/crates/infra/exec/src/run_test.rs

<!-- design.md「公共函数 / API」子表两行（`run_shell`、`RunError::fmt`）+「类型定义」子表四行（`RunOptions` / `RunOutcome` / `RunReport` / `RunError`）均落本文件，故下列待测功能逐行取自两表；测试模块以 `#[cfg(test)] #[path = "run_test.rs"] mod run_test;` 挂载（沿 exec crate 既有 `lib_test` / `shell_test` 形态），私有常量 `CANCEL_POLL_INTERVAL` / `FINISH_GRACE` 与私有助手 `drain` / `wait_cancel` 经该内嵌模块可直接触达。 -->

#### 待测功能

- `run_shell()`（公共函数表，AC-1/AC-3/AC-4/AC-5）：已置位 cancel 短路不 spawn → `into_tokio()` + `cwd` / `envs` + `stdin(null)` + 双管道 + `kill_on_drop(true)` → `spawn`（失败 `Err(Spawn)`）→ `select!`（`biased`：cancel 轮询 → timeout → 退出）→ 被杀两臂 `kill_tree(pid)` + 有界收尸 + 有界排水 → 组 `RunReport`
- `RunError::fmt()`（公共函数表，D1）：`Spawn(e)` → `"拉起失败: {e}"`、`Wait(e)` → `"等待失败: {e}"`，只承载类别词 + `io::Error` 原文，不带调用点语境
- `RunOptions`（类型定义表，AC-2/AC-5）：`cwd: Option<PathBuf>` / `env: Vec<(OsString, OsString)>` / `timeout: Duration` 必填（无 `Default`、无「None 即无限」）/ `cancel: CancelFlag`；`derive(Debug, Clone)`
- `RunOutcome`（类型定义表，AC-3/AC-4）：封闭三值 `Completed` / `TimedOut` / `Cancelled`，`derive(Debug, Clone, Copy, PartialEq, Eq)`
- `RunReport`（类型定义表，AC-3/AC-5）：`outcome` / `exit_code`（`TimedOut` / `Cancelled` 与「无可读退出码」同记 `-1`）/ `stdout` / `stderr`（零改写零截断，不预合流）
- `RunError`（类型定义表，D1）：`Spawn(String)` = 进程未建立、`Wait(String)` = `child.wait()` io 失败，`derive(Debug, Clone, PartialEq, Eq)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run_shell` — 单流正向 | 正向 | `SystemCommand::script("echo hi")` + `timeout = 30 s` + `cwd = None` + `env = []` + `cancel = never()` → `Ok(RunReport { outcome: Completed, .. })`、`exit_code == 0`、`stdout` 含回显文本、`stderr` 空 | 单值 |
| `run_shell` — 退出码谱 | 正向 | 壳内 `exit N`（`N ∈ {0, 1, 3, 255}`）→ `outcome` 恒 `Completed`、`exit_code` 逐值等值透传（哨兵 `-1` 不被非零码误用） | 参数化（4 值） |
| `RunReport` — 双缓冲不合流 | 正向 | shim 分别向 stdout 与 stderr 写不同文本（`exit 0`）→ `stdout` / `stderr` 各自只含本路字节，执行体不做 `2>&1` 也不拼接（D14） | 单值 |
| `RunOptions.cwd` — 生效 | 正向 | `cwd = Some(tempdir)` 且脚本以相对路径读取 `tempdir` 内文件回显 → 回显内容等值（`current_dir` 施加生效）；`cwd = None` → 继承测试进程 cwd（脚本以 `std::env::current_dir()` 下的相对探测为准） | 表驱动（2 形态：Some / None） |
| `RunOptions.env` — 生效 | 正向 | `env = [("RSE_MARK", "on")]` + shim 回显该变量 → `stdout` 含 `on`；父进程环境未出现该键（`RunOptions` 逐次施加，不污染进程） | 单值 |
| `RunOptions.env` — 规模与形态 | 边界 | `env` 取 `[]`（空）/ 单元素 / ≥50 项 / 重复键（后值覆盖前值，shim 回显末值）→ 四种形态均 `Completed` 且回显符合预期 | 表驱动（4 形态） |
| `RunOptions.cwd` — 不存在目录 | 异常 | `cwd = Some(不存在路径)` → `Err(RunError::Spawn(_))`，`to_string()` 以「拉起失败: 」起头（`current_dir` 施加期失败归 `Spawn` 面） | 单值 |
| `run_shell` — 程序不可达 | 异常 | `SystemCommand::bare("no-such-program-xyz")` → `Err(RunError::Spawn(_))`；`Display` 首段恰为「拉起失败: 」且不含任何调用点前缀（D1：语境由调用点拼） | 单值 |
| `RunError` — `Wait` 变体渲染 | 异常 | 直接构造 `RunError::Wait("<io>")` → `to_string() == "等待失败: <io>"`（真实 `child.wait()` io 失败不可稳定构造，见 `## 不可测试项` 第 5 项） | 单值 |
| `RunOutcome` — 三值遍历 | 边界 | `Completed` / `TimedOut` / `Cancelled` 逐个取值：`assert!(matches!(..))` 分支可区分、`Copy` + `Eq` 语义成立（封闭集无非法值臂） | 遍历枚举（3 值） |
| `run_shell` — 空脚本 | 边界 | `SystemCommand::script("")` → `Completed` + 壳退出码（`0`）+ `stdout` / `stderr` 皆空 `Vec`（不 panic） | 单值 |
| `run_shell` — 超长命令串 | 边界 | 命令串 >1000 字符（长 `echo` 串）→ `Completed` 且 `stdout` 逐字全量（不截断、不改写） | 单值 |
| `run_shell` — 特殊字符输出 | 边界 | shim 输出含引号 / 换行 / emoji → `stdout` 字节逐字相等（执行体零转义、零归一） | 参数化（3 形态） |
| `run_shell` — 双管道并发不互锁（AC-5） | 正向 | shim 同时向 stdout 与 stderr 各写 >64 KiB（超过管道缓冲，单线程顺序读会互锁）→ 两路字节数精确等于写量（不丢字节）+ 墙钟上界断言（不互锁） | 单值 |
| `run_shell` — 输出规模边界 | 边界 | `stdout` / `stderr` 取「双空」/「仅 stdout 单字节」/「仅 stderr 单字节」/「双大块」→ `RunReport` 逐字节等值，`Completed` | 表驱动（4 形态） |
| `run_shell` — 非 UTF-8 原始字节 | 边界 | shim 输出裸字节 `0xFF 0xFE …`（非 UTF-8）→ `stdout` 原始字节逐字保留（执行体不做 UTF-8 假设 / lossy 转换） | 单值 |
| `run_shell` — 超时臂（AC-3） | 正向 | 自旋 shim（管道恒开、不退出）+ `timeout = 1 s`（虚拟时钟 `advance` 触发）→ `Ok(outcome = TimedOut, exit_code = -1)`；墙钟上界断言（`kill_tree` + `FINISH_GRACE` 有界收尾，不等满真实时长） | 单值 |
| `run_shell` — 已置位 cancel 短路（AC-4/D4） | 正向 | `cancel = never()` 后先 `handle().set()` 再调 `run_shell` → `Ok(outcome = Cancelled, exit_code = -1)`；shim 零执行（无标记文件 / 无输出落盘），墙钟近零（不依赖 200 ms 轮询时序） | 单值 |
| `run_shell` — 运行中 cancel（AC-4） | 正向 | shim 先落 pid 文件再自旋；`run_shell` 在 `tokio::spawn` 内运行，主任务置位 `CancelHandle::set()` → `Ok(outcome = Cancelled, exit_code = -1)` + 目标 pid 消失 + 墙钟上界 | 单值 |
| `run_shell` — cancel 不误杀 | 边界 | `cancel = never()` 全程不置位 + 正常退出脚本 → `Completed`（轮询臂零副作用，不产生假 `Cancelled`） | 单值 |
| `run_shell` — 被杀路径 pid 回收（AC-3/AC-4） | 边界 | 超时臂与取消臂各跑一次：桩 shim 再拉一个子 shim（两者均落 pid 文件并自旋）→ 收尾后 `#[cfg(windows)]` 断言父子 pid 均消失（`tasklist` 探测，`taskkill /T /F` 整树），`#[cfg(not(windows))]` 断言直接子进程 pid 消失（`kill -9` 语义，R-4） | 表驱动（2 平台臂 × 2 结局臂） |
| `run_shell` — 被杀路径输出不完整可返回（D6） | 边界 | 自旋 shim 在超时前已刷出部分字节 → `TimedOut` 臂 `RunReport` 正常返回（`stdout` / `stderr` 为残余字节，完整性不断言），全过程不 panic、不悬挂 | 单值 |
| 常量锚定 `CANCEL_POLL_INTERVAL` / `FINISH_GRACE` | 边界 | `CANCEL_POLL_INTERVAL == Duration::from_millis(200)`、`FINISH_GRACE == Duration::from_secs(2)`（轮询粒度与有界收尾上界由常量钉死，`Duration` 取值改动即被击杀） | 表驱动（2 常量） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（被测执行体自身） | 不 mock spawn：真实 shim（unix `sh` 脚本 + `chmod +x`；Windows `.cmd` / msys bash 拷贝）经 `SystemCommand::bare(<绝对路径>)` 或 `SystemCommand::script(<平台壳>)` 真实拉起；输出 / pid / 标记文件由 shim 自身落盘或打印 | 全部 spawn 用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `tokio::time::advance`（沿 `mutation/cargo_mutants_test.rs` 既有先例）——超时臂与 `FINISH_GRACE` 有界收尾不真等；取消臂的 200 ms 轮询亦随虚拟时钟推进 | 超时 / 取消 / 有界收尾用例 |
| 文件系统 | 不 mock：`tempfile::TempDir` 真实目录（`cwd` 断言面、pid 文件、shim 脚本与标记文件落位） | `cwd` / 取消 / pid 回收用例 |
| 全局变量 `PATH` / 进程环境 | 不 mock 也不改写：程序一律经 `SystemCommand::bare` 绝对路径或平台壳解析，`RunOptions.env` 由 shim 回显断言；exec crate 无 `TEST_ENV_LOCK`，故用例不触进程级环境改写（并行安全） | `env` / 程序不可达用例 |
| 进程树击杀（`exec::kill_tree`） | 不 mock：真实调用（Windows `taskkill /PID /T /F`、unix `kill -9`），存活探测经 `tasklist` / `kill -0` | 超时臂 / 取消臂 pid 回收用例 |
| 内部模块（`SystemCommand` / `kill_tree` / `foundation::cancel` / 私有 `drain` / `wait_cancel`） | 不 mock：真实组合（最小 mock 原则）；`CancelFlag` 走真实 `handle().set()` 而非替身 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/exec/src/lib.rs -> packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`kill_tree` / `ProcessOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` 逐字零改，`mod run;` + 再导出面属模块接驳）；唯一可测变更取自「类型定义」子表 `SystemCommand` 行（derive 由 `Debug` 扩为 `Debug, Clone`）。既有 `lib_test.rs` 只增 clone 等价用例，既有四例（壳包装 / `raw` / 底座 / argv 形态）零改写。 -->

#### 待测功能

- `SystemCommand`（类型定义表，AC-5）：`#[derive(Debug)]` → `#[derive(Debug, Clone)]`——`program` / `args` / `raw` 三字段均可 `Clone`（无手写 impl），clone 后经 `into_std()` 的 program 与 argv 逐字相等、`raw` 标记随 clone 保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SystemCommand` — clone 等价（壳包装） | 正向 | `script("echo hi")` clone 后两份各自 `into_std()`：`get_program()` 与 `get_args()` 逐字相等；原值 clone 后仍可继续使用（未 move） | 单值 |
| `SystemCommand` — 四形态 clone | 正向 | `bare("cargo").args(["mutants", "--workspace"])` / `script` / `script_raw` / `script_with(GitBash \| Cmd)` 逐形态 clone → program + argv 逐字相等（`raw` 仅 tokio 侧表达，经 `into_tokio().as_std()` 对拍） | 表驱动（4 形态） |
| `SystemCommand` — 空 argv | 边界 | `bare("cargo")`（零参数）clone → program 相等、`get_args()` 为空（不 panic） | 单值 |
| `SystemCommand` — 特殊字符与超长 argv | 边界 | argv 含引号 / 换行 / emoji / 超长（>1000 字符）/ 非 UTF-8 `OsString` → clone 后逐字节相等（`OsStr` 非 UTF-8 面保留） | 参数化（4 形态） |
| `SystemCommand` — `raw` 标记保真 | 边界 | `script_raw("npx ... > \"{x}\"")` clone 后 `into_tokio().as_std()` 的末参与原件一致；`raw == true` 语义不因 clone 退化为 `false`（Windows 臂唯一可观测面） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：全部用例只在 `std::process::Command` / `tokio::process::Command` 的**组装形状**面断言（`get_program` / `get_args`），零 spawn、零环境改写 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/foundation/src/cancel.rs -> packages/desktop/src-tauri/crates/core/foundation/src/cancel_test.rs

<!-- design.md「公共函数 / API」子表四行（`CancelFlag::never` / `is_set` / `handle`、`CancelHandle::set`）+「类型定义」子表两行（`CancelFlag` / `CancelHandle`）落本文件；测试模块以 `#[cfg(test)] mod cancel_test;` 挂载（沿 foundation 既有 `identity_test` / `prune_test` 形态，无 `#[path]`）。 -->

#### 待测功能

- `CancelFlag::never()`（公共函数表，AC-4）：恒 false 零值（缺省信号 / 测试默认 / run 已收口回落）
- `CancelFlag::is_set()`（公共函数表）：`Ordering::Relaxed` 读
- `CancelFlag::handle()`（公共函数表）：取写端（共享同一原子）
- `CancelHandle::set()`（公共函数表）：`Ordering::Relaxed` 写、幂等
- `CancelFlag`（类型定义表，D7）：`Clone` 共享同一原子、手写 `Debug`（`set = {bool}`）、手写 `PartialEq` = `Arc::ptr_eq` + `Eq`
- `CancelHandle`（类型定义表）：`Clone` / `Debug`；构造面收口在读端（仅 `CancelFlag::handle()` 可铸造）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `CancelFlag::never()` | 正向 | `never().is_set() == false`，连续多次读恒 false；`never()` 之间互不影响 | 单值 |
| `CancelHandle::set()` → `is_set()` | 正向 | `flag.handle().set()` → `flag.is_set() == true`（写端与读端同一原子） | 单值 |
| `CancelFlag::clone` 共享原子 | 正向 | `let copy = flag.clone();` → `copy.handle().set()` 后 `flag.is_set()` 与 `copy.is_set()` 均为 true；反向（`flag.handle().set()` 后 `copy` 为 true）同断言 | 单值 |
| `CancelHandle` clone | 正向 | `let handle = flag.handle(); let handle2 = handle.clone();` → 任一 `set()` 后 `flag.is_set()` 为 true | 单值 |
| `CancelHandle::set()` 幂等 | 边界 | 连续 `set()` 3 次 → 恒 true、无 panic、无副作用（同一原子置位） | 参数化（1 / 3 / N 次） |
| 相等语义 — identity | 边界 | `let a = never(); assert_eq!(a.clone(), a)`（`Arc::ptr_eq` 为真）；`assert_ne!(never(), never())`（独立铸造永不相等，身份比较非状态比较，D7）；`a.clone() == a` 反射性成立 | 表驱动（2 形态：同一原子 / 独立铸造） |
| 相等语义 — 置位后仍相等 | 边界 | `a` 与其 clone 分别读取（一 true 一 false 的状态差异不存在，因共享原子）；独立 flag 置位不影响另一 flag 的相等关系（`assert_ne!` 仍成立） | 单值 |
| `Debug` 渲染 | 边界 | `format!("{flag:?}") == "CancelFlag(set = false)"`；置位后为 `"CancelFlag(set = true)"`（手写 Debug，非字段展开） | 参数化（2 形态） |
| 跨线程可见 | 正向 | `std::thread::spawn` 内 `handle.set()` + `join()` → 主线程 `is_set() == true`（纯 std，零 tokio；`Relaxed` 下 `join` 提供 happens-before） | 单值 |
| 不串台 | 边界 | 两个独立 `never()` flag，其中一个 `set()` → 另一个仍 false；同一 flag 的两份 clone 状态一致 | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：纯 std（`Arc` / `AtomicBool`）；跨线程可见性用真实 `std::thread` + `join`，不引入 tokio、不引入假时钟 | 全部用例 |
| 依赖面（零 tokio、零 workspace 内依赖） | 不 mock 也不在用例中断言：属 `Cargo.toml` / crate 图审查面（AC-9），见 `## 不可测试项` 第 13 项 | — |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

<!-- design.md「公共函数 / API」子表三行（`begin_run` / `request_stop` / `RunGuard::cancel_flag`）落本文件；`begin_run` / `request_stop` 签名不变（内部行为扩双写），`RunGuard::cancel_flag` 为新增。既有 `control_test.rs` 装置（无 mock、进程内 tokio 原语真实组合）零改，仅增 cancel_flag 臂。 -->

#### 待测功能

- `ChangeFlowControl::begin_run()`（公共函数表，AC-6）：签名不变，内部铸造 `CancelFlag::never()` 落 `RunEntry.cancel_flag`（新登记的 run 未置位）
- `ChangeFlowControl::request_stop()`（公共函数表，AC-6/D9）：双写 `cancel.send_replace(true)` + `cancel_flag.handle().set()`；miss 幂等 `false`；重复调用幂等
- `RunGuard::cancel_flag()`（公共函数表，AC-6/D8）：克隆共享同一原子；run 在位期间与 `cancelled()` 同源观测

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `begin_run` → `cancel_flag()` 初值 | 正向 | `begin_run` 后 `guard.cancel_flag().is_set() == false` 且 `guard.cancelled() == false`（新 run 零置位，铸造 `never()`） | 单值 |
| `request_stop` 双写（AC-6） | 正向 | 运行中 `request_stop(ROOT, CHANGE_ID)` → 返回 true；同刻 `guard.cancelled() == true`（watch 臂）**且** `guard.cancel_flag().is_set() == true`（进程面臂）——两臂同一调用点置位 | 单值 |
| 无订阅者不丢（AC-6） | 正向 | `begin_run` 后不取任何 `updates` / `cancelled_wait` 订阅即 `request_stop` → `guard.cancel_flag().is_set() == true`（`send_replace` 与原子双写均不依赖订阅者） | 单值 |
| `request_stop` 幂等（D9） | 边界 | 连调两次 → 两次均返回 true、flag 仍 true、无 panic；`set()` 重复无副作用 | 单值 |
| `request_stop` miss | 边界 | 未登记键 → `false`，不 panic，不改任何 run 的 flag；已 `finish` 除名键 → `false`（既有用例扩展 `cancel_flag` 归属断言） | 表驱动（2 形态：未登记 / 已除名） |
| 多 run 隔离 | 边界 | 两条并行 run（异 change id 与异 root 两形态）→ `request_stop` 只置位目标 run 的 `cancel_flag`，另一 run 恒 false（沿用既有「A 停止不影响 B」用例扩展） | 表驱动（2 形态：异 id / 异 root） |
| `RunGuard::cancel_flag()` 同一原子 | 边界 | 同 guard 连续两次 `cancel_flag()` → `assert_eq!`（`Arc::ptr_eq`）；置位后两次调用均读 true；与 `ToolStepRequest` 侧拿到的 flag identity 相等（跨模块身份锚） | 单值 |
| 终态除名后的取消面 | 边界 | `finish(status)` 后 `request_stop` 返回 false、`snapshot` 为 `None`（既有锚）；含 `cancel_flag` 的 `RunEntry` 随去除名，`Arc` 由在跑的 runner 持有至释放（置位不因除名丢失） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（注册表与通道） | 零 mock：`ChangeFlowControl` 真实实例 + 进程内 `watch` / `broadcast` / `Mutex` 真实组合；`#[tokio::test]` 内多任务挂起 / 唤醒沿既有 `tokio::spawn` + `yield_now` 握手装置 | 全部用例 |
| 时间 / 文件系统 | 不涉及（注册表为纯进程内状态，零落盘） | — |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

<!-- design.md「公共函数 / API」子表三行（三 runner trait 的 `run` 形参）落本文件；`ToolStepRequest` 的 `cancel` 字段取自「类型定义」子表。三 trait 均为进程内缝，形参演进的运行期可观测面 = 假实现收到的 flag 身份，与 `ToolStepRequest` 的相等 / `Debug` 语义。 -->

#### 待测功能

- `ToolStepRequest`（类型定义表，D7）：增 `pub cancel: CancelFlag`，字段集 `{ root, command, cancel }`；derive 保持 `Debug, Clone, PartialEq, Eq`（相等语义靠 flag 的 identity 相等成立）
- `StaticCheckRunner::run(root, name, slot, cancel)`（公共函数表，AC-6）：第四形参为取消信号
- `TestExecutionRunner::run(root, name, slot, cancel)`（公共函数表，AC-6）：同上
- `MutationRunner::run(root, name, files, patch, cancel)`（公共函数表，AC-6）：末位形参为取消信号

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ToolStepRequest` — 相等语义 | 正向 | 同 `root` + 同 `command` + 同一 flag（clone）的两份请求 `assert_eq!`（identity 相等成立，derive 零手写） | 单值 |
| `ToolStepRequest` — 独立 flag 不等 | 边界 | 同 `root` / `command` 但各自 `CancelFlag::never()` → `assert_ne!`（身份比较非状态比较，文档既定语义） | 单值 |
| `ToolStepRequest` — clone 保持身份 | 边界 | `let b = a.clone(); assert_eq!(a.cancel, b.cancel);`（clone 共享同一原子，非新铸造） | 单值 |
| `ToolStepRequest` — `Debug` 渲染 | 边界 | `format!("{:?}", request)` 含 `cancel: CancelFlag(set = false)`；置位后为 `set = true`（`ToolStepRequest` derive 的 `Debug` 与 `CancelFlag` 手写 `Debug` 协同） | 参数化（2 形态） |
| 三 trait 形参接收（AC-6） | 正向 | 三 trait 各一假实现记录收到的 `cancel` → 调用后 `assert_eq!(记录, 传入)`（同一原子）+ 编译期形参在位（三 trait 逐个） | 表驱动（3 trait） |
| 三 trait 已置位信号不被丢弃 | 边界 | 传入 `never()` 后先 `set()` 再调 → 假实现内 `is_set() == true`（信号原样透传，不重建、不清零） | 表驱动（3 trait） |
| 既有载荷面零改 | 边界 | `ToolCommand` / `ToolStepOutput` / 各 Outcome 载荷字段面与改造前逐字一致（穷尽解构无 `..`；`MutationTestOutcome` 等结构体字面量编译期随动即证明） | 表驱动（3 组载荷） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 三 runner trait 的假实现 | 假实现即被测契约的**显式入参注入依赖**（trait 对象由测试侧实现，非进程边界），允许以 `Mutex` 记录捕获的 flag；不 mock 任何进程 / 文件面 | 三 trait 形参用例 |
| 其余内部模块 | 不 mock：`foundation::cancel::CancelFlag` 真实组合（`never()` / `handle().set()`） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`LocalToolSteps::execute` 签名不变，「仅做字段透传不做决策」）；待测项取自「修改文件」表 `steps.rs` 行（`let cancel = step.cancel;` + 三分支移入对应 runner 调用）与 AC-6 / proposal「变更范围 - 测试文件」表的「三 runner trait 假实现签名」随动项。 -->

#### 待测功能

- `LocalToolSteps::execute`（修改文件行，AC-6）：`match step.command` 前 `let cancel = step.cancel;`，`StaticCheck` / `TestExecution` / `MutationTest` 三分支各把 `cancel` 移入对应 runner 调用（互斥分支，无需 `clone`）
- 其余七分支零改：审计行（`audit_outcome`）与载荷逐字不变，`cancel` 不泄漏到无关分支

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `StaticCheck` 分支透传 | 正向 | `ToolStepRequest { cancel }` 且 flag 已 `set()` → 假 `StaticCheckRunner` 收到的 flag `is_set() == true` 且与 `step.cancel` `assert_eq!`（同一原子） | 单值 |
| `TestExecution` 分支透传 | 正向 | 同型（第四形参）→ 假 `TestExecutionRunner` 收到的 flag 身份相等 | 单值 |
| `MutationTest` 分支透传 | 正向 | 同型（末位形参）→ 假 `MutationRunner` 收到的 flag 身份相等 | 单值 |
| 三分支置位不丢 | 边界 | 三分支各跑一次（`never()` 后 `handle().set()`）→ 三个假件均观测 `is_set() == true` | 表驱动（3 分支） |
| `never()` 零行为变化 | 边界 | 三分支以 `CancelFlag::never()` 走完整执行链 → 假件产出照旧（`StaticCheck` / `TestExecution` / `MutationTest` 载荷逐字段等值），审计行文本与改造前一致 | 表驱动（3 分支） |
| 其余七分支零改 | 边界 | `PhaseStart` / `PhaseNext` / `PhaseLog` / `DecisionLog` / `Archive` 等分支 → 审计行与载荷逐字不变、零 `cancel` 消费（编译期无未使用告警） | 表驱动（7 分支） |
| runner `Err` 原样透传 | 异常 | 假 runner 返回 `Err("执行已取消")` → `execute` 原样返回该串（不加前缀、不改文案、不吞成 `Ok`），审计行记 `Err` 同文 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 三 runner trait（`StaticCheckRunner` / `TestExecutionRunner` / `MutationRunner`） | 假实现注入（`LocalToolSteps` 的构造入参，显式注入依赖），以 `Mutex` 捕获收到的 `CancelFlag` | 全部三分支用例 |
| vcs 缝（`MutationTest` 分支的 `change_edit_set` / `change_edit_patch`） | 假件注入（既有装置），返回固定清单 / 补丁；不 mock spawn | `MutationTest` 分支用例 |
| 内部模块（`vcs` 之外） | 不 mock：`foundation::cancel` 真实、审计写面经真实 tempfile workspace（既有 `steps_test` 装置） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`run_tool` 为模块私有，design 明写「私有 / pub(crate) 函数不入本表」）；待测项取自「修改文件」表 `walker.rs` 行 ①②③（`run_tool` 增 `guard` 形参与单点填充、13 处调用点补实参、`step_phase_next` / `run_phase_log` 增形参）+ D10 + AC-6 / AC-7。本节同时承载 AC-7 的组合锚（链路方向：`walk_run` → `run_tool` → `ToolStepRequest.cancel` → runner → `exec::run_shell`），按「最上层调用方」挂靠本入口模块。 -->

#### 待测功能

- `run_tool(tools, guard, root, command, label)`（修改文件行 ①，AC-6/D10）：`ToolStepRequest.cancel` 单点取 `guard.cancel_flag()` 填充——13 处调用点无一处出现 `.cancel_flag()`
- `step_phase_next(tools, guard, request)` / `run_phase_log(tools, guard, request, input)`（修改文件行 ③）：增 `guard: &RunGuard` 形参向上取并透传（callers 已有 `ctx.guard` / `guard` 在手）
- 组合锚（AC-7）：stop 运行中的检查步 → 子进程被回收、run 秒级收敛 **failed**（过渡形态，文案含「执行已取消」），不做 run stopped
- 零改动面：`check_cancel`（步边界唯一调用点）、`drive` 循环结构、节点 emit / 边累积、`TryFrom<ToolStepOutput>` 转换面

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run_tool` 单点填充 — 未置位 | 正向 | 常态 run（未 stop）→ 假 `ToolStepPort` 收到的每个 `ToolStepRequest.cancel.is_set()` 均 false，且与 `guard.cancel_flag()` `assert_eq!`（同一原子） | 单值 |
| `run_tool` 单点填充 — stop 后（AC-6） | 正向 | `control.request_stop(ROOT, CHANGE_ID)` 后推进到下一次 `run_tool` → 捕获的 `cancel.is_set() == true`（`guard.cancel_flag()` 单点填充生效，无中间重建） | 单值 |
| 多步全链同一原子 | 正向 | 一次 run 覆盖 `StaticCheck` / `TestExecution` / `MutationTest` 各 ≥1 次 → 捕获的全部 flag `assert_eq!`（13 处调用点共用 `guard.cancel_flag()` 克隆） | 表驱动（3 步序形态） |
| AC-7 组合锚：stop 运行中检查步 | 正向 | 可取消假 runner（未置位则永久挂起，置位即立即返回 `Err("执行已取消")`）+ 真实 `ChangeFlowControl` → 运行中 `request_stop` → run 收敛 `ChangeRunStatus::Failed`、`Finished.reason` / 节点失败文本含「执行已取消」、墙钟秒级（不等挂起自然结束） | 单值 |
| AC-7 边界：挂起永不返回 | 边界 | 假 runner 挂起且不观测 cancel → stop 后 run 仍在秒级墙钟内收敛（不挂死；收口由 runner 契约的 `Err` 路径完成） | 单值 |
| 步边界与进程面独立 | 边界 | 停止后仍有在跑工具步：`check_cancel`（步边界）不在工具步中途触发，收敛口径由 runner 返回值决定（本变更不引入中途步级砍断） | 单值 |
| 非取消类 `Err` 不被误叙述 | 异常 | 假 runner 返回 `Err("boom")` → run failed 文案为「{label} 失败: boom」，不含「执行已取消」（取消文案不误用） | 单值 |
| 常态回归锚 | 边界 | `CancelFlag::never()` 全程（既有 walker 用例族）→ 全链行为、步序、节点 emit / 边累积与改造前逐字一致 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 四个内部 port（`WorkerAgentPort` / `ToolStepPort` / `WorkflowSnapshotPort` / `RunHistoryPort`） | 假件注入（`walk_run` 的显式入参依赖，沿既有 `FakeWorker` / `FakeTools` / `StubSnapshot` / 假 history 装置）；`FakeTools` 额外以 `Mutex<Vec<CancelFlag>>` 捕获 `ToolStepRequest.cancel` | 全部用例 |
| 子进程（真实 spawn） | 不 mock 也不引入：AC-7 以「可取消假 runner」在 runner 层收敛，真实子进程回收由 `run_test`（`run_shell` pid 用例）与 `static_check_test` / `mutation/*_test`（段超时 pid 断言）承接——最小 mock 原则：walker 层不重复 spawn 面 | AC-7 组合用例 |
| 控制注册表（`ChangeFlowControl` / `RunGuard`） | 不 mock：真实实例（被测链路的写端与读端同源） | 全部用例 |
| 文件系统 / store | 不 mock：临时 workspace（tempfile）+ 既有 store 缝真实落盘 | 收敛类用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs

<!-- design.md「公共函数 / API」子表一行（`ProcessStaticCheck::run` trait impl，第四形参 `cancel`）落本文件；其余待测项取自「修改文件」表 `static_check.rs` 行 ①~⑤（`STATIC_CHECK_TIMEOUT` 常量、spawn 段迁 `run_shell`、结局四臂、`diagnostics = report.stdout`）+ proposal 表「static-check 超时 → `Err` 且子进程回收；取消已置位 → `Err`」。 -->

#### 待测功能

- `ProcessStaticCheck::run(root, name, slot, cancel)`（公共函数表，AC-6）：第四形参透传 `execute`
- `STATIC_CHECK_TIMEOUT`（修改文件行 ①，AC-2/D11）：`pub(crate) const STATIC_CHECK_TIMEOUT: Duration = Duration::from_secs(1200)`——static-check 首次获得超时预算，模块内单点消费
- spawn 段迁移 `run_shell`（修改文件行 ③，AC-1/AC-3/AC-5）：`cwd = Some(root)`、命令内 `2>&1` **保留**、`env = []`、`timeout = STATIC_CHECK_TIMEOUT`、`cancel` 透传
- 结局四臂映射（修改文件行 ④，AC-3/AC-4/AC-8）：`RunError` → `Err("static-check 命令{failure}")`；`Cancelled` → `Err("执行已取消")`；`TimedOut` → `Err("static-check 命令超时（1200 秒）强制终止")`；`Completed` → `passed = exit_code == 0`、`diagnostics = String::from_utf8_lossy(&report.stdout)`（`report.stderr` 恒空，**不读**）、record 取 `report.stdout`
- 零改面（AC-8）：`ensure_program_resolvable` / `report_dir` 三分支 / 无配置直过分支 / `diagnostics.trim_end()`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 交错流单流语义（AC-8） | 正向 | shim 交替向 stdout / stderr 打行（`2>&1` 在 OS 层合并）→ `diagnostics` 逐字等于交错顺序全文、`StaticCheckOutcome.stderr` 面不存在（`report.stderr` 不读）；`output_ref` 落盘的 `.log` 与 `diagnostics` 同源同文 | 单值 |
| 退出码 → `passed` | 正向 | shim `exit 0` → `passed = true`、`diagnostics` 经 `trim_end`；`exit 1` / `exit 2` → `passed = false` + 诊断文本保留（既有锚回归） | 参数化（3 值：0 / 1 / 2） |
| **超时臂（AC-3）** | 正向 | 自旋 shim + 虚拟时钟 `advance(STATIC_CHECK_TIMEOUT)` → `Err` 文案恰为「static-check 命令超时（1200 秒）强制终止」+ 墙钟上界（不悬挂——本变更修复的现状缺陷）+ 子进程 pid 消失 | 单值 |
| 已置位 cancel（AC-4） | 正向 | `cancel` 先 `set()` 再 `run` → `Err("执行已取消")` 且 shim 零执行（`2>&1` 流无输出、`exec-outputs` 零新文件；D4 入口短路） | 单值 |
| 运行中 cancel（AC-4） | 正向 | shim 落 pid 后自旋，运行中 `handle().set()` → `Err("执行已取消")` + pid 消失 + 墙钟上界（不落 diagnostics、不落 record 引用） | 单值 |
| 程序不可达（既有前置） | 异常 | 配置命令裸名 / 带路径不存在 → `ensure_program_resolvable` 先 `Err` 记因（文案骨架不变；`run_shell` 的 `Spawn` 臂因此不可达，见 `## 不可测试项` 第 9 项） | 表驱动（2 形态：裸名 / 相对路径） |
| `STATIC_CHECK_TIMEOUT` 锚定（AC-2） | 边界 | `assert_eq!(STATIC_CHECK_TIMEOUT, Duration::from_secs(1200))`（预算常量单点消费；`Duration` 改动即被击杀） | 单值 |
| 无配置 / 空命令 | 边界 | `static_analysis` 缺席 / 空白串 → `passed = true`、`diagnostics` 空、`output_ref = None`、`report_dir` 回填、**零 spawn**（该分支先于 `run_shell` 返回） | 表驱动（2 形态：缺席 / 空白） |
| 无配置 + 已置位 cancel | 边界 | 取消已置位且无配置 → 仍走「无配置直过」（取消短路在 `run_shell` 内，分支优先级不变），零 spawn、不 `Err` | 单值 |
| 诊断文本边界 | 边界 | 输出为空 / 超长（>1000 字符）/ 含特殊字符（引号、换行、emoji）→ `diagnostics` 逐字（不截断不转义）、`trim_end` 语义保留、`passed` 判定不受影响 | 表驱动（3 形态） |
| `cwd = root` | 边界 | 命令以相对路径读 `root` 下文件（既有 `write_root_file` 装置）→ 回显等值（`RunOptions.cwd` 施加生效、`current_dir(root)` 语义保留） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（配置的 static-check 命令） | 不 mock spawn：既有 `write_shim` 装置真实落盘平台 shim（`.cmd` / bash，含交错行 / 非零退出 / 自旋三形态），经 `PathPrefix` 裸名解析真实拉起 | 全部 spawn 用例 |
| 全局变量 `PATH` | 不改写目标程序语义，但裸名解析经 PATH：`crate::TEST_ENV_LOCK` + `PathPrefix` RAII（备份 / 恢复）串行化，沿既有装置 | 全部 spawn 用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `tokio::time::advance(STATIC_CHECK_TIMEOUT)`（沿 `mutation/*_test.rs` 既有先例），超时臂不真等 1200 s | 超时臂用例 |
| 文件系统 | 不 mock：`TempWs` 真实 workspace 根（config、root 相对文件、报告树、`exec-outputs` 落盘） | 全部用例 |
| 内部模块（`exec::run_shell` / `record_exec_output` / `config::load` / `layout`） | 不 mock：真实组合（本变更的被测语义正是「换谁 spawn、谁排水」） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`execute_plan` 为 `pub(crate)`，按「私有 / pub(crate) 不入表」排除，形参演进见「修改文件」行）；待测项取自「修改文件」表 `runner.rs` 行 ①~⑤ + proposal「变更范围 - 测试文件」表「`SUITE_COMMAND_TIMEOUT` 常量锚定 600 → 1200」「testexec 取消 → `Err`」。 -->

#### 待测功能

- `SUITE_COMMAND_TIMEOUT`（修改文件行 ①，AC-2/D11）：`Duration::from_secs(600)` → **`from_secs(1200)`**，doc 口径由「CLI `runCommand(cmd, cwd, 600000)` 同值」改为「rust suite 实测 340–393 s × 约 3 倍余量（原 600 s 仅 1.7 倍；CLI 同值锚定已失效）」
- `execute_plan(plan, report_dir, cancel)`（修改文件行 ②③，AC-1/AC-3/AC-4）：spawn + 超时段整体替换为 `run_shell(SystemCommand::script_raw(&command), RunOptions { cwd: Some(plan.cwd), env: [], timeout: SUITE_COMMAND_TIMEOUT, cancel })`
- 结局四臂（修改文件行 ④，AC-3/AC-4/AC-8）：`RunError` → `Err("测试命令{failure}")`；`Cancelled` → `return Err("执行已取消")`（**不落** `execution_error`）；`TimedOut` → `(EXIT_CODE_NO_STATUS, Some("测试命令超时（1200 秒）强制终止"), Vec::new())`；`Completed` → `exit_code = report.exit_code`、`output = report.stdout ++ report.stderr`、非零 → `non_zero_exit_error`
- 私有 `read_all` 退役（修改文件行 ⑤，AC-1）：管道排水归 `run_shell` 单点，`std::process::Stdio` 依赖随之删除

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SUITE_COMMAND_TIMEOUT` 锚定（AC-2） | 边界 | 改写既有 `超时常量锚定`：`SUITE_COMMAND_TIMEOUT == Duration::from_secs(1200)`（期望值与注释随动，`600` 口径退役）；`VERSION_DETECT_TIMEOUT` 仍 `30 s`（未涉及面零变） | 表驱动（2 常量） |
| 超时臂（AC-3） | 正向 | 自旋 suite（管道恒开、不退出）+ 虚拟时钟 `advance(SUITE_COMMAND_TIMEOUT)` → `exit_code == -1`、`error` 含「测试命令超时（1200 秒）强制终止」、`output` 为空 `Vec`、墙钟上界（不悬挂）+ pid 消失 | 单值 |
| 运行中 cancel（AC-4） | 正向 | 自旋 suite 运行中 `handle().set()` → `Err("执行已取消")`；`report_dir` 下**零** summary / 子报告写入（取消不走 `execution_error` 语义）+ pid 消失 | 单值 |
| 已置位 cancel（AC-4/D4） | 正向 | `cancel` 先 `set()` 再 `execute_plan` → `Err("执行已取消")` 且零 spawn（无 shim 痕迹；报告目录仍按既有顺序清理重建，不因此 `Err` 之外多写） | 单值 |
| 合并流顺序（AC-8） | 正向 | 替身命令写 stdout（重定向进 `results.txt`）与 stderr 双路 → `PlanExecution.output` 恒「stdout 前缀 + stderr 后缀」逐字节；模板不加 `2>&1`（`parse_cargo_test` 只读 stdout 面） | 单值 |
| 退出码直继承 | 正向 | 替身命令 `exit 0` → `exit_code = 0`、`error = None`；`exit 3` / `exit 101` → `exit_code` 同值透传 + `error` 含「测试命令非零退出（码）」 | 参数化（3 值：0 / 3 / 101） |
| 裸名程序不可达 | 异常 | 不存在程序名 → `ensure_program_resolvable` 前置 `Err` 记因「未找到」（基础设施失败不落报告级 `error`；`run_shell` 的 `Spawn` 臂由此不可达） | 单值 |
| 双管道大块并发 | 边界 | stdout 与 stderr 各 >64 KiB 同时产出 → `output` 全长拼接（不丢字节、顺序稳定）+ 墙钟上界（`run_shell` 并发排水不互锁的消费面锚） | 单值 |
| 输出空 / 单路空 | 边界 | 双空 / 仅 stdout / 仅 stderr → `output` 逐形态等值（`[]` / 前缀 / 后缀），不 panic | 表驱动（3 形态） |
| 退出码缺码哨兵 | 边界 | `EXIT_CODE_NO_STATUS == -1`（既有锚）；与 `run_shell` 的 `TimedOut` 臂同值（超时 → 子报告 `execution_error` 显式失败，不回落 `0` 造假绿） | 单值 |
| 工件解析面零变（AC-8） | 边界 | 结果 / 覆盖工件照旧解析：`cases` 非空、`source_files` 非空、`measured.lines = Some(..)`；工件缺失 → `plan 目录结果文件缺失或不可解析` 记因（`error` 面既有分支） | 单值 |
| 模板展开零改 | 边界 | 五占位符替换逐字（既有 `rust模板展开_双工件路径逐字且重定向恰一次` 等用例）全绿——本变更只换 spawn 面，不改命令串 | 表驱动（既有 4 用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（suite 命令） | 不 mock spawn：真实替身命令（`cp` / `type` / `exit N`）经 `SystemCommand::script_raw` → `cmd /C` / `sh -c` 真实执行；取消 / 超时臂用自旋 shim 脚本（落 pid 后挂起） | 全部 spawn 用例 |
| 全局变量 `PATH` | 不 mock：`EnvWindow`（`crate::TEST_ENV_LOCK` + RAII）串行窗口；可达 / 不可达臂靠程序是否存在区分，不改写 PATH 语义 | 程序解析前置用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `advance(SUITE_COMMAND_TIMEOUT + …)`（既有先例） | 超时臂 / 取消臂 |
| 文件系统 | 不 mock：`tempfile::TempDir` 真实报告目录（`execute_plan` 自身清空重建）+ `std::fs::write` 预置工件与替身脚本 | 全部用例 |
| 内部模块（`expand_template` / `parse_results` / `parse_coverage_artifact` / `exec::run_shell`） | 不 mock：真实组合（解析 / 归因语义属 AC-8 零改面，必须真实链） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

<!-- design.md「公共函数 / API」子表一行（`ProcessTestExecution::run` 的 `cancel` 形参）落本文件；其余待测项取自「修改文件」表 `testexec/mod.rs` 行（`execute` 形参透传 + `execute_plan(plan, &dir, cancel.clone())`）。 -->

#### 待测功能

- `ProcessTestExecution::run(root, change, slot, cancel)`（公共函数表，AC-6）：形参透传 `execute` → 逐 plan `execute_plan(plan, &dir, cancel.clone())`（循环内多次 spawn，段间 clone）
- 零改面（AC-8）：复用门（`read_existing_summary` / `load_disk_sub_reports` / `prune_roots`）、报告写盘、`outcome` 装配、record 单点聚合

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `never()` 缺省回归锚 | 正向 | `ProcessTestExecution::run(root, change, slot, CancelFlag::never())` 走完整执行链（既有 mod_test 用例全绿）→ `outcome` 字段面与改造前逐字段等值，零行为变化 | 单值 |
| 多 plan 中途取消 | 正向 | 两 plan 的 suite：第 1 个 plan 完成后置位 cancel → 第 2 个 plan 返回 `Err("执行已取消")`、整体 `Err`（取消不被吞成部分结论、不落 summary） | 单值 |
| 已置位 cancel | 边界 | `cancel` 先 `set()` 再 `run` → 首个 plan 即 `Err`、零 spawn；复用门在位且输入未变时的「零 spawn 复用」语义不变（复用门先于 spawn 判定） | 单值 |
| 循环内 clone 共享原子 | 边界 | 两 plan 链路捕获的 flag `assert_eq!`（同一原子，`cancel.clone()` 而非重新铸造） | 单值 |
| 既有显式 `Err` 面零改 | 异常 | 不支持框架 / `change` 未建档 / 完整性校验失败 → 既有 `Err` 文案与短路顺序不变 | 表驱动（3 形态） |
| 合并输出聚合 | 边界 | 多 plan 输出按 plan 序聚合落单一 `.log`（`merged_output`）→ 顺序与总量不变（取消臂不参与聚合） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（各 plan 的 suite 命令） | 不 mock spawn：经 config 声明的替身命令 / 上游 `SpawnWindow` 式 shim 真实执行（取消 / 超时臂自旋） | 全部 spawn 用例 |
| 文件系统 / store | 不 mock：tempfile 真实报告树 + 既有 store / config 缝真实落盘（复用门判定依赖真实 mtime / 内容） | 全部用例 |
| 内部模块（`runner::execute_plan` / `detect::detect_plans` / `checks::is_reusable`） | 不 mock：真实组合（本层只做透传与聚合） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`run_segment` 为 `pub(crate)` 且不入表）；待测项取自「修改文件」表 `cargo_mutants.rs` 行 ①~⑤ + proposal「变更范围 - 测试文件」表「段超时目标 pid 树消失（`kill_tree` 生效锚）」。 -->

#### 待测功能

- `run_segment(patch_path, exec_dir, cancel)`（修改文件行 ①，AC-6）：`spawn + 超时` 段整体替换为 `run_shell(command, RunOptions { cwd: exec_dir, env: [(CARGO_TARGET_DIR, exec_dir/target)], timeout: 1800 s, cancel })`
- `RunOptions.env` 承接 `CARGO_TARGET_DIR`（修改文件行 ②，AC-8）：跨段共享同一 target dir 语义等价（增量编译复用，MUST NOT 每段重置）
- 结局四臂（修改文件行 ③，AC-3/AC-4/AC-8）：`RunError::Spawn` → `Fatal("cargo mutants 拉起失败（程序不可达？）: {e}；请确认 cargo-mutants 已安装（cargo install cargo-mutants）")`；`RunError::Wait` → `Fatal("cargo-mutants 段执行等待失败: {e}")`；`TimedOut` → `Interrupted("cargo-mutants 段执行超时（1800 秒）强制终止")`；`Cancelled` → `Cancelled("cargo-mutants 段执行已取消")`
- `Completed` 臂（修改文件行 ④）：`exit_code = report.exit_code`、`output = stdout ++ stderr`；段报告权威 / 零突变体合成载荷 / `segment_failure_message` 零改；私有 `read_all` 退役（修改文件行 ⑤，AC-1）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| argv / cwd / env 等价承接（AC-8） | 正向 | 假 cargo 记录 `argv` / `cwd` / `CARGO_TARGET_DIR` → 与改造前逐字相等（`env` 经 `RunOptions` 施加而非 `Command::env`，观测面不变） | 单值 |
| **超时臂 + pid 回收（AC-3）** | 正向 | 自旋 shim（`set_sleep(1)` 恒开管道）+ 虚拟时钟 `advance(SEGMENT_SPAWN_TIMEOUT_MS + …)` → `Err(SegmentFailure::Interrupted)` 文案仍为「cargo-mutants 段执行超时（1800 秒）强制终止」+ 墙钟上界 + **pid 文件断言**：`#[cfg(windows)]` 父子进程均消失（`taskkill /T /F`），`#[cfg(not(windows))]` 直接子进程消失 | 表驱动（2 平台臂） |
| 运行中 cancel（AC-4） | 正向 | 自旋 shim 运行中 `handle().set()` → `Err(SegmentFailure::Cancelled("cargo-mutants 段执行已取消"))`（不是 `Fatal` / `Interrupted`）+ pid 消失 + 墙钟上界 | 单值 |
| 已置位 cancel（AC-4/D4） | 正向 | `cancel` 先 `set()` 再 `run_segment` → `Cancelled` 且假 cargo 零记录（入口短路不 spawn） | 单值 |
| 拉起失败（`Spawn` 臂） | 异常 | PATH 隔离使 `cargo` 缺席 → `Fatal` 文案逐字（含「（程序不可达？）」与 `cargo install cargo-mutants` 指引） | 单值 |
| 非零退出无报告 | 异常 | 假 cargo 非零退出且不产 `mutants.out/outcomes.json` → `Interrupted(segment_failure_message)`（退出码 + 输出双段，`exec::ProcessOutput` 装配不变） | 参数化（退出码 1 / 101） |
| 零突变体段（既有语义） | 边界 | 退出 0 且无报告（`EmptyDiff` / `NoSourceFiles` / `NoMutants`）→ 返回 `ZERO_MUTANTS_PAYLOAD` 合成载荷（不误判为故障） | 单值 |
| 段报告权威（既有语义） | 边界 | `outcomes.json` 在位且可解析 + 非零退出 → `Ok(raw)`（工具以非零退出表达未达标，不中断） | 单值 |
| 段超时常量锚定 | 边界 | `SEGMENT_SPAWN_TIMEOUT_MS == 1_800_000`（既有 `argv模板常量面锚定` 保留，预算面不变） | 单值 |
| 合并流与大输出 | 边界 | 双管道各 >64 KiB → `output` 全长「stdout ++ stderr」逐字节（取消 / 超时臂不消费输出） | 表驱动（2 形态：空 / 大块） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（`cargo mutants`） | 不 mock spawn：`SpawnWindow` 装置（PATH 前置假 `cargo.exe` / unix shim；记录 argv / cwd / env；可编程输出、自旋挂起、非零退出、ENOENT）——迁 `run_shell` 后装置零改（假可执行体真实 spawn） | 全部 spawn 用例 |
| 全局变量 `PATH` / `FAKE_SPAWN_LOG` | `crate::TEST_ENV_LOCK` + `SpawnWindow` RAII（备份 / 恢复），进程内单窗口互斥（既有装置） | 全部 spawn 用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `advance`（既有先例） | 超时臂 / 取消臂 |
| 文件系统 | 不 mock：`tempfile` 真实执行目录（`mutants.out/outcomes.json` 落位、patch 文件）+ pid / 标记文件 | 全部用例 |
| 进程树击杀（`exec::kill_tree`） | 不 mock：真实 `taskkill` / `kill -9`，pid 存活经 `tasklist` / `kill -0` 探测 | pid 回收用例 |
| 内部模块（`exec::ProcessOutput` / `parse_cargo_mutants_seg` / `mutation_jobs`） | 不 mock：真实组合；`available_parallelism` 不 mock（`jobs` 期望按真实 CPU 重算关系式，沿既有装置） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`run_segment` 为 `pub(crate)` 且不入表）；待测项取自「修改文件」表 `stryker.rs` 行 ①~⑤ + proposal 同表项。 -->

#### 待测功能

- `run_segment(segment, cancel)`（修改文件行 ①，AC-6）：`spawn + 超时` 段替换为 `run_shell(SystemCommand::script_raw(&command), RunOptions { cwd: Some(suite_cwd), env: [], timeout: 1800 s, cancel })`
- 结局四臂（修改文件行 ③，AC-3/AC-4/AC-8）：`Spawn` → `Fatal("Stryker 命令拉起失败: {e}")`；`Wait` → `Fatal("Stryker 段执行等待失败: {e}")`；`TimedOut` → `Interrupted("Stryker 段执行超时（1800 秒）强制终止")`；`Cancelled` → `Cancelled("Stryker 段执行已取消")`
- `Completed` 臂（修改文件行 ④）：`exit_code` + `output = stdout ++ stderr`
- 零改面：临时 config 写盘 / 段报告权威 / sandbox 目录纪律 / `ensure_on_path("npx")` 前置；私有 `read_all` 退役（修改文件行 ⑤，AC-1）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 命令串 / cwd 不变（AC-8） | 正向 | 假 `npx` 记录 → 展开命令串 `npx --prefix "<suite_cwd>" stryker run "<config>"` 与 `cwd == suite_cwd` 逐字不变（`raw` 保真） | 单值 |
| 临时 config 写盘 | 正向 | 段执行前 `mutation_dir` 下临时 config 落盘且 `mutate` / `tempDirName` / `concurrency` 字段不变（取消臂同样先写盘） | 单值 |
| **超时臂 + pid 回收（AC-3）** | 正向 | 自旋 shim + `advance(SEGMENT_SPAWN_TIMEOUT_MS + …)` → `Interrupted("Stryker 段执行超时（1800 秒）强制终止")` 文案不变 + 墙钟上界 + pid 断言（Windows 整树 / unix 直接子进程） | 表驱动（2 平台臂） |
| 运行中 cancel（AC-4） | 正向 | 自旋 shim 运行中置位 → `Cancelled("Stryker 段执行已取消")` + pid 消失 + 临时 config / sandbox 面保留（取消不清理，交由调用方保留纪律） | 单值 |
| 已置位 cancel（AC-4/D4） | 正向 | `cancel` 先 `set()` → `Cancelled` 且假 `npx` 零记录（临时 config 已写、进程未拉） | 单值 |
| `npx` 不可达 | 异常 | PATH 隔离 → `ensure_on_path("npx")` 前置 `Fatal` 文案（`Stryker 命令拉起失败` 的 spawn 臂由此不可达） | 单值 |
| 段报告缺失 / 损坏 | 异常 | 假 `npx` 退出 0 但不产段报告 / 产损坏 JSON → `Interrupted(segment_failure_message)`（退出码 + 输出双段；报告文件面归因） | 表驱动（2 形态） |
| 段超时常量锚定 | 边界 | `SEGMENT_SPAWN_TIMEOUT_MS == 1_800_000`（两段共用常量，预算不变） | 单值 |
| 合并流与大输出 | 边界 | 双管道各 >64 KiB → `output` 全长「stdout ++ stderr」逐字节 | 表驱动（2 形态：空 / 大块） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（`npx --prefix … stryker run …`） | 不 mock spawn：`SpawnWindow` 装置（PATH 前置假 `npx`；记录 argv / cwd；可编程输出 / 自旋 / 非零退出 / ENOENT） | 全部 spawn 用例 |
| 全局变量 `PATH` / `FAKE_SPAWN_LOG` | 同 `cargo_mutants_test`（`crate::TEST_ENV_LOCK` + `SpawnWindow` RAII 共享装置） | 全部 spawn 用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `advance`（既有先例） | 超时臂 / 取消臂 |
| 文件系统 | 不 mock：`tempfile` 真实 sandbox / 临时 config / 段报告落位 | 全部用例 |
| 进程树击杀 | 不 mock：真实 `taskkill` / `kill -9` + pid 探测 | pid 回收用例 |
| 内部模块（`report::segment_report_path` / `build_config` / `to_posix` / `parse_stryker_seg`） | 不 mock：真实组合（写盘与解析语义属 AC-8 零改面） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs

<!-- design.md「公共函数 / API」子表一行（`ProcessMutation::run` 的 `cancel` 形参）+「类型定义」子表一行（`SegmentFailure` 增 `Cancelled(String)`）落本文件；其余待测项取自「修改文件」表 `mutation/mod.rs` 行 ①~④（`Display` 三臂同型、`execute → run_suite → execute_segment` 贯穿、`interruption: Option<SegmentFailure>` 分变体文案）+ D13 + AC-7。 -->

#### 待测功能

- `SegmentFailure::Cancelled(String)`（类型定义表，D13）：第三变体，语义 = 用户停止 → `conclusion = Error` + 取消文案；`Display` 三臂同型（单串）
- `ProcessMutation::run(root, name, files, patch, cancel)`（公共函数表，AC-6）：形参透传 `execute`
- `execute → run_suite → execute_segment` 贯穿 cancel（修改文件行 ③，AC-4）：三段各加形参并 `cancel.clone()` 下传，最终达 `cargo_mutants::run_segment` / `stryker::run_segment`
- 中断面分变体（修改文件行 ④，AC-7/D13）：`interruption: Option<SegmentFailure>`，`Cancelled` 与 `Interrupted` 同走「retain Stryker 桶临时面 + break」，终端面按变体分文案（`Cancelled` → 「突变执行已取消：{error}\n已产报告路径：…」，`Interrupted` → 既有「突变执行中断：…」），两者 `conclusion = MutationTestConclusion::Error`
- 零改面（AC-8）：`Fatal` 臂仍 `Err`、归并块 / 阈值判定 / 复用门 / summary 回填

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SegmentFailure::Display` 三臂同型（D13） | 正向 | `Fatal("a")` / `Interrupted("b")` / `Cancelled("c")` 逐个 `to_string()` 等于载荷串（单串同型，无变体前缀） | 遍历枚举（3 变体） |
| **取消 → `conclusion = Error`（AC-7 组合）** | 正向 | 假 spawn 自旋 + 运行中 `handle().set()` → `MutationTestOutcome { conclusion: Error, skip: None, .. }`、`feedback_text` 含「突变执行已取消：」+「已产报告路径：」（非 `Fatal`，不呈现为环境损坏） | 单值 |
| 取消后 Stryker 临时面保留（D13） | 正向 | Stryker 桶取消 → `retained_artifacts_section` 附保留路径、`_stryker-tmp` 与临时 config 未被清理（与 `Interrupted` / `Fatal` 保留纪律同型） | 单值 |
| `Interrupted` 文案不混用（D13） | 正向 | 段超时臂 → `feedback_text` 含「突变执行中断：…」且**不含**「已取消」（两变体文案独立，R-9） | 单值 |
| `Fatal` 分流零改（AC-8） | 异常 | 段报告目录创建失败 / 工具不可达 → `execute` 返回 `Err`（`conclusion` 不落位），取消面不走 `Fatal` | 表驱动（2 形态：写盘失败 / 工具不可达） |
| 已置位 cancel 的入口面 | 边界 | `cancel` 先 `set()` 再 `run` → 首段即 `Cancelled`、后续桶不执行（段串行 break）、墙钟上界；跳过面（`EmptyDiff` / `NoExecutor`）仍先返回 `skip`（顺序零改） | 表驱动（2 形态：可执行桶 / 跳过面） |
| 多桶中途取消 | 边界 | rust + stryker 双桶：第 1 桶完成后置位 → 第 2 桶 `Cancelled`、第 1 桶块不进 summary（中断面不回填，既有语义）+ 报告路径清单含第 1 桶 | 单值 |
| 循环内 clone 共享原子 | 边界 | 双桶链路捕获的 flag `assert_eq!`（`cancel.clone()` 逐层下传，同一原子） | 单值 |
| `never()` 缺省回归锚 | 边界 | 既有 mod_test 全量用例（bucket / 归并 / 阈值 / 复用门 / summary 回填 / 常量锚定）在 `CancelFlag::never()` 下全绿（零行为变化） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（两段执行体） | 不 mock spawn：`SpawnWindow` 假执行体（PATH 前置 `cargo.exe` / `npx.cmd` / unix shim；自旋 + 记录），与 `cargo_mutants_test` / `stryker_test` 共享同一装置（`super::super::mod_test` 引用） | 全部段执行用例 |
| 全局变量 `PATH` / `FAKE_SPAWN_LOG` | `crate::TEST_ENV_LOCK` + `SpawnWindow` RAII 串行 | 全部段执行用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `advance` | 取消 / 超时臂 |
| 文件系统 | 不 mock：真实 `tempfile` 报告树（`change_test_reports` 布局真实落盘、sandbox / 临时 config / 段报告 / summary 回填断言） | 全部用例 |
| 内部模块（`bucket_suites` / `merge_mutation_blocks` / `report::*` / `checks` 解析器） | 不 mock：真实组合（归并与阈值判定属 AC-8 零改面）；`available_parallelism` 不 mock（jobs 期望按真实 CPU 重算关系式） | 全部用例 |

---

## 不可测试项

| 项 | 原因 | 承接方式 |
|---|---|---|
| 1. `packages/desktop/package.json` 版本 `0.5.19` → `0.5.20` | 数据面（JSON 单字段），无运行时测试路径 | 版本 diff 复核（AC-11）+ `tauri.conf.json` 经 `../package.json` 跟随的静态确认 |
| 2. `packages/desktop/src-tauri/crates/infra/exec/Cargo.toml`（tokio 特性面补 `io-util` / `time` / `macros`，dev-deps 补 `rt` / `macros` / `test-util`） | 构建配置，无单元测试断言面 | `Cargo.toml` 逐项注释审查 + `cargo tree -p exec`（AC-9 / R-7）；特性缺失即编译失败（`read_to_end` / `sleep` / `select!` 不可用） |
| 3. `packages/desktop/src-tauri/crates/core/foundation/src/lib.rs`（`pub mod cancel;` + 模块 doc 改「四能力」） | 仅模块声明与文档文本，`design.md`「公共函数 / API」子表无该文件行；foundation 无 `lib_test.rs` | 行为面由 `cancel.rs -> cancel_test.rs` 全覆盖；依赖面零改由 AC-9 审查承接 |
| 4. `RunOptions.timeout` 必填、无 `Default` 的「缺失字段不可表达」 | 编译期约束；负向编译测试（`trybuild` 类）不在本仓库门禁内 | 代码审查 + 编译期签名（四调用点字面量必须给 `timeout`）；预算面由四常量锚定用例承接（AC-2） |
| 5. `RunError::Wait` 的真实触发臂（`child.wait()` io 失败） | io 失败不可稳定构造（需进程句柄损坏等平台相关条件） | `Display` 渲染由 `run_test` 的变体直构用例覆盖；四调用点的 `Wait` 文案拼法由代码审查 + `run_test` 的 `Display` 锚定承接 |
| 6. `run_shell` 的 `kill_on_drop(true)` 兜底路径（future 在收尾前被 drop） | 需在 `select!` 外部 drop future，进程回收时序不可稳定断言（且 `drop` 语义与运行时调度相关） | 代码审查 + 「被杀臂 pid 消失」用例承接主路径（D5 的 `kill_tree` + 有界收尾） |
| 7. `RunGuard::cancel_flag()` 的「run 已除名 → 回落 `CancelFlag::never()`」分支（D8） | 公开 API 上不可达：`finish(self)` 消费 guard，除名后无 guard 可调用该方法；`begin_run` 同键重复登记被拒，故不存在第二个 guard | 代码审查；邻近语义由 `control_test` 的「除名后 `request_stop` 返 false」「flag 由在跑 runner 持有至释放」用例承载——**建议实施相位复核该回落分支是否需随实现一并调整（或显式留痕不覆盖）** |
| 8. 「无输出退出码」（Unix 信号终止）→ `RunReport.exit_code = -1` 的分支 | Windows `ExitStatus::code()` 恒 `Some`；Unix 信号终止构造不稳定（门禁仅在 Windows 跑） | 既有 `EXIT_CODE_NO_STATUS == -1` 值锚定（`runner_test`）+ `TimedOut` / `Cancelled` 两臂的 `exit_code == -1` 断言（AC-3 / AC-4） |
| 9. `static_check.rs` 的 `RunError::Spawn` 臂（拉起失败文案） | `ensure_program_resolvable` 前置已挡住裸名 / 相对路径不可达；spawn 期失败（如目标文件非可执行）跨平台不稳定 | `RunError::Spawn` 的 `Display` 前缀「拉起失败: 」由 `run_test` 锚定；调用点拼法（`format!("static-check 命令{failure}")`）由代码审查承接（D2） |
| 10. Unix 侧的「子孙进程全数回收」（进程树击杀） | `kill_tree` unix 臂为 `kill -9 <pid>`（只杀直接子进程，R-4 既定风险，本篇不改）；当前门禁 / 测试仅在 Windows 跑 | AC-3 的 pid **树**断言以 `#[cfg(windows)]` 为准（`taskkill /T /F`）；Unix 只断言直接子进程消失；进程组补强（`setsid` + `kill -9 -pgid`）留独立 change |
| 11. AC-1 的「四文件无裸 `tokio::time::timeout` + `start_kill` 组合、无私有 `read_all`」 | 源码文本面，运行时不可断言（`read_all` 删尽后无符号可引用，`start_kill` 亦然） | `grep -rn "async fn read_all" crates/infra/checks/src/`（零命中）+ `grep -rn "start_kill" crates/`（零命中）+ `cargo clippy`（`dead_code` / `unused_imports` 随删除暴露） |
| 12. AC-6 的「13 处测试 trait impl + ~26 处 `ToolStepRequest` 字面量」随动 | 编译期一致性，无独立运行时断言（不随动即编译失败） | `cargo clippy --all-targets` / `cargo test --no-run`（编译期即证）+ `grep -rn "cancel_flag()" crates/core/orchestration/src/walker.rs`（仅 `run_tool` 一处填充） |
| 13. AC-9 的 `foundation` 零 tokio / 零 workspace 内依赖、`exec` 依赖方向不变 | crate 图与依赖清单，非运行时面 | `Cargo.toml` 审查 + `cargo tree -p foundation -p exec`（`foundation` 仅 `sha2`；`exec` 仅 `foundation` + `tokio`，特性面 `process` / `io-util` / `time` / `macros`） |
| 14. AC-10 的覆盖率阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90）与「全部既有测试通过」 | 门禁统计面（由桌面端静态脚本 / test-execution 门禁执行，agent 内不重复执行） | `packages/desktop` 门禁：`pnpm run check` + `pnpm run server:test` + 覆盖率报告；新增两模块（`run.rs` / `cancel.rs`）用例按 R-6 要求先行全覆盖后再计入 |
| 15. AC-11 的 `plugins/**` 零改动零 bump、bindings 零 diff | 数据 / 生成面 | `git diff --stat plugins/`（零条目）+ `bindings:check`（零 diff） |
| 16. 段超时 / 取消用例在 CI 慢机上的稳定性（R-3） | 自旋 shim 与 200 ms 轮询的竞态属环境相关；非确定性失败无法在用例内完全消除 | 取消用例独立成条（不与既有用例共享装置）+ 入口短路（D4）承载「不依赖轮询时序」的确定路径 + 虚拟时钟（`start_paused`）消解大部分墙钟依赖 |
