# 提案: exec-run-shell

> **变更**: exec-run-shell
> **日期**: 2026-10-21
> **状态**: draft
> **来源**: `explore.md`（requirement 类型，设计完成未实施；**sibling 篇（rust-suite-single-build）已归档合入**，本篇为该探索的主体切片，可独立实施）

---

## 问题

检查域四类外部进程（static-check、test-execution suite、cargo-mutants 段、Stryker 段）各自写一遍 spawn + 超时 + 管道排水，能力参差不齐：

| spawn 点 | 超时 | 进程树击杀 | 双管道 | 取消 |
|---|---|---|---|---|
| `crates/infra/checks/src/static_check.rs:63-73`（`command.output()`） | **✗** | **✗** | ✗（单管道 `2>&1`） | ✗ |
| `crates/infra/checks/src/testexec/runner.rs:89-110` | ✓ `SUITE_COMMAND_TIMEOUT` 600 s | **✗**（超时臂 `start_kill()` 只杀壳） | ✓ | ✗ |
| `crates/infra/checks/src/mutation/cargo_mutants.rs:56-73` | ✓ `SEGMENT_SPAWN_TIMEOUT_MS` 1800 s | **✗** | ✓ | ✗ |
| `crates/infra/checks/src/mutation/stryker.rs:68-83` | ✓ 1800 s | **✗** | ✓ | ✗ |

三份逐字重复的私有 `read_all`（`testexec/runner.rs:165`、`mutation/cargo_mutants.rs:119`、`mutation/stryker.rs:215`）是该重复的直接证据；`exec::kill_tree` 已在 `crates/infra/exec/src/lib.rs:242` 落地，却**只被 `infra/agent/src/sdk/bash.rs` 消费**——检查域一次都没用上。

**参考实现已存在**：`crates/infra/agent/src/sdk/bash.rs` 是正确形态（`tokio::time::timeout` 罩全段 + `exec::kill_tree(pid)` + 双管道并发排水 + `kill_on_drop(true)`）。检查域不消费它的原因不是能力缺失，而是没有统一执行体可收敛。

### 具体后果

| 缺口 | 后果（均可从既有 run 报告取证） |
|---|---|
| static-check **零超时** | cargo 卡在 `Blocking waiting for file lock` 时**永久悬挂**，run 永远 running；用户按停止也无效（见下条） |
| `start_kill()` 只杀壳 | `cmd /C cargo test` 被杀而 `cargo` → `rustc` 孙子进程继续写 target；下一轮同 target 撞锁（`os error 32`） |
| mutation 段超时同样只杀壳 | `cargo mutants` 的 rustc 子孙继续写 `target/debug`（`CARGO_TARGET_DIR = exec_dir/target`，与两检查域**同一棵树**） |
| stop 只在相位 / 步边界观测（`core/orchestration/src/walker.rs:221` 唯一 `check_cancel` 调用点） | stop 运行中的检查步无效（子进程跑完为止，test-execution 实测 340–393 s/次）；立刻再发起 → 两条 cargo 同 target |

> 这些缺陷**独立于文件锁**：即使没有安全软件扫 target，也会因「超时 / 停止后残留进程」制造同类 `os error 32`；反之，锁失败被显式暴露（sibling 篇已办）后，残留进程是仅剩的复发源。

### 预算缺口（独立于实现形态）

`SUITE_COMMAND_TIMEOUT` 600 s 对实测 340–393 s 的 rust suite 只有 **1.7 倍**余量——热态正常、冷态或多 change 并发即触线，触线后又因「只杀壳」留下残党。

## 提案

只改桌面端（`packages/desktop/src-tauri`），CLI / 插件零改动；纯加法 + 四类调用点迁移，五面：

1. **`infra/exec` 增统一执行体**：新模块 `run.rs` 提供

   ```rust
   pub async fn run_shell(command: SystemCommand, options: RunOptions) -> RunReport;

   pub struct RunOptions { cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration /* 必填 */, cancel: CancelFlag }
   pub enum RunOutcome { Completed, TimedOut, Cancelled }
   pub struct RunReport { outcome: RunOutcome, exit_code: i32 /* TimedOut/Cancelled 为 -1 */, stdout: Vec<u8>, stderr: Vec<u8> }
   ```

   实现形态：`spawn`（`kill_on_drop(true)`）→ 双管道并发 `read_to_end` → `tokio::select!` 竞速「取消轮询（200 ms）/ timeout 到期 / `wait()` 收敛」→ 取消与超时两臂统一走 `exec::kill_tree(pid)` + `wait` 收尸（再让排水任务以 EOF 收口）→ 按结局组装 `RunReport`。`SystemCommand` 补 `#[derive(Clone)]`（program / args / raw 均可 Clone），`cwd` / `env` 由 `RunOptions` 施加，命令规格与目标机无关。**超时必填**——编译期杜绝「零超时悬挂」这一现状缺陷。

2. **`foundation::cancel` 增取消原语（零 tokio）**：`CancelFlag(Arc<AtomicBool>)`（`Clone` 共享同一原子 / `never()` 恒 false / `is_set()`）+ `CancelHandle`（`set()`）。零依赖、零 tokio——`foundation` 的「第二个消费者到场才下沉」纪律在此**正触发**（`RunGuard` 写、`run_shell` 读），故 `run_shell` 用 200 ms 轮询而非 `Notify`。

3. **取消贯通到四个 spawn 点**：`core/orchestration` 侧 `RunEntry.cancel_flag` 与 `RunGuard::cancel_flag()` 在位，`request_stop()` **双写**（`watch::send_replace(true)` + `cancel_flag.set()`，无订阅者也不丢）；`ToolStepRequest.cancel` 与三 runner trait 签名（`StaticCheckRunner` / `TestExecutionRunner` / `MutationRunner`）各加一参 `cancel: CancelFlag`；`steps.rs` 透传、`walker.rs` 的 `run_tool` 单点从 `guard.cancel_flag()` 取值。

4. **四 spawn 点迁移 + 预算校准**：

   | spawn 点 | 现状 | 本篇 |
   |---|---|---|
   | static-check | **无超时**、单管道 | `run_shell` + **1200 s** + cancel；**命令内 `2>&1` 保留**（stderr 恒空，诊断 / record 单流语义零变） |
   | test-execution suite | 600 s、只杀壳 | `run_shell` + **1200 s** + cancel（冷构建约 3 倍余量） |
   | cargo-mutants 段 | 1800 s、只杀壳 | `run_shell` + 1800 s（不变）+ cancel（`CARGO_TARGET_DIR` 经 `RunOptions.env`） |
   | Stryker 段 | 1800 s、只杀壳 | `run_shell` + 1800 s（不变）+ cancel（cwd = suite_cwd） |

   三份私有 `read_all` 随迁移退役（管道排水归 `run_shell` 单点）。

5. **mutation 取消面**：`SegmentFailure` 增 `Cancelled` 变体（`Display` 同 `Fatal` / `Interrupted`），贯穿 `execute → run_suite → execute_segment`；取消 → 段中断 → `conclusion = Error`（反馈边即时面沿用）。

### 范围裁决（explore 未决项的落地口径）

- **终态语义保持过渡形态**：runner 契约是 `Result<ToolStepOutput, String>`，`Err` 会被 `run_tool` 包成 failed。故本篇取消的收敛口径为 `Cancelled → Err("执行已取消") → run failed`，**不引入 `fault=Stopped` / run stopped**——那属 explore 声明的 sibling 篇（`lock-fault-diagnostics`，本仓磁盘上无该 explore，需另立 change）。本篇只保证「进程确实被杀掉、不残留、run 秒级收敛」，并在 spec 显式留痕过渡语义，避免后续变更踩到已定稿表述。
- **`infra/agent/src/sdk/bash.rs` 不迁移**：它是本篇的**形态参考**，不是迁移对象（agent 边界自身的 spawn 语义、超时钳位 `[1 s, 600 s]`、`[stderr]` 标记合并口径都与检查域不同）。留待将来「agent 边界也需取消」时再另立。
- **`bootstrap`（`crates/infra/vcs/src/bootstrap.rs`：自带 900 s 轮询 + `child.kill()`）、`testexec/detect.rs` 版本探测（std 短命令）、`infra/vcs` git 步（`bare` argv 直传）不动**：前者属 vcs 边界的 worktree 建置面（不在检查域四 spawn 之列，其「只杀壳」残留与本篇同一族问题，留痕于 R-4 待独立变更），后两者是短命命令 / 非检查门禁面。
- **超时预算不进 config schema**：四个值以代码常量观察落地（与 mutation 既有常量纪律同性），要求用户可配属独立 schema 变更。
- **与 sibling 篇（rust-suite-single-build）的协作已解除**：该篇已归档合入，`runner.rs` 的模板 / 重定向面改动与本篇的 spawn 段（`89-110`）区块不重叠。

## 能力

### 新增的能力

- **desktop-exec-runtime** — `infra/exec` 统一进程执行体与取消原语：`run_shell` 契约（三值结局 / 必填超时 / 双管道并发排水 / 收尸）、超时预算表、进程树击杀、`foundation::cancel` 零 tokio 下沉裁决、「输出解读面 `ProcessOutput` 与未迁移 spawn 面零变」边界。

### 修改的能力

- **desktop-checks-domain** — **ADDED**「检查域 spawn 统一经 exec 执行体」：四 spawn 点一律经 `run_shell`（禁私有 spawn / 超时 / 排水），static-check 首获超时，命令内 `2>&1` 保留，取消 → `Err("执行已取消")`（过渡），三份私有 `read_all` 退役。
- **desktop-test-execution** — **ADDED**「suite 执行超时预算与取消」：`SUITE_COMMAND_TIMEOUT` 600 s → **1200 s**（实测 340–393 s，原 1.7 倍余量不足）；超时仍落既有 `execution_error` 语义（`exit_code = -1`、非零 → error、`conclusion = error`），合并流顺序（stdout 前缀 + stderr 后缀）零变；取消 → `Err`。
- **desktop-mutation-testing** — **ADDED**「段执行取消与进程树击杀」：`SegmentFailure::Cancelled` 变体贯穿 `execute → run_suite → execute_segment`，取消 → `conclusion = Error`（过渡）；段超时 1800 s 不变但改由 `TimedOut` 收口 + `kill_tree`；既有 `Fatal` / `Interrupted` 分流与 sandbox 保留纪律不变。
- **desktop-change-orchestration** — **ADDED**「停止贯通到进程执行」：`RunEntry.cancel_flag` + `request_stop` 双写 + `RunGuard::cancel_flag()` + `ToolStepRequest.cancel` + 三 runner trait 签名带 `cancel` + `run_tool` 单点透传；终态过渡语义（failed）显式留痕。
- **desktop-crate-layout** — **MODIFIED**「crate 三层分组与依赖规则」：`foundation` 极小名单纳入 `cancel` 取消原语（第二消费者正触发的下沉；依赖面仍零 workspace 内依赖、零 tokio）。

其余能力零改动，理由：

- **desktop-agent-execution**：`bash.rs` 不迁移（见范围裁决），agent 租户的 spawn / 超时钳位 / `[stderr]` 合并口径零改动。
- **desktop-checks-domain 的纯层 / 输出记录 requirement**：`crates/core/checks`（解析 / 聚合 / 复用门）与 `ProcessOutput`、`record_exec_output` 全部零改动——本篇只换「谁 spawn」，不换「怎么判 / 怎么记」。
- **desktop-crate-layout 的依赖方向其余条款**：`exec` 仍是 infra 唯一 spawn 配置点（新增 tokio 特性面 `io-util` / `time` / `macros` 属 crate 内部实现细节，依赖方向不变）。
- **前端 / bindings / 插件**：无载荷字段变化、无新 IPC 命令、无 `plugins/**` 改动。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/infra/exec/src/run.rs`（新） | `run_shell` / `RunOptions` / `RunReport` / `RunOutcome`；`kill_on_drop(true)` + 双管道并发排水 + `select!`（取消轮询 200 ms / timeout / wait）→ `kill_tree` + 收尸 |
| `crates/infra/exec/src/lib.rs` | `mod run;` + `pub use run::{run_shell, RunOptions, RunReport, RunOutcome}`；`SystemCommand` 加 `#[derive(Clone)]` |
| `crates/infra/exec/Cargo.toml` | tokio 特性面补 `io-util` / `time` / `macros`（`process` 已在）；dev-dependencies 补 `rt` / `macros` / `test-util`（超时 / 取消用例） |
| `crates/core/foundation/src/cancel.rs`（新） | `CancelFlag`（`Clone` / `never()` / `is_set()`）+ `CancelHandle`（`set()`）；std-only |
| `crates/core/foundation/src/lib.rs` | `pub mod cancel;`（零依赖面不变：仅 `sha2`） |
| `crates/core/orchestration/src/control.rs` | `RunEntry.cancel_flag: CancelFlag`；`begin_run` 铸造；`request_stop` 双写；`RunGuard::cancel_flag()` |
| `crates/core/orchestration/src/port.rs` | `ToolStepRequest.cancel: CancelFlag`；`StaticCheckRunner::run(root, name, slot, cancel)` / `TestExecutionRunner::run(root, name, slot, cancel)` / `MutationRunner::run(root, name, files, patch, cancel)` |
| `crates/core/orchestration/src/steps.rs` | 三缝调用透传 `step.cancel` |
| `crates/core/orchestration/src/walker.rs` | `run_tool` 增 cancel 入参（取 `guard.cancel_flag()`），`ToolStepRequest` 构造单点填 `cancel` |
| `crates/infra/checks/src/static_check.rs` | 切 `run_shell`（`cwd = root`、**命令内保留 `2>&1`**、1200 s、cancel）；`diagnostics = report.stdout` |
| `crates/infra/checks/src/testexec/runner.rs` | 切 `run_shell`（1200 s + cancel）；删私有 `read_all`；合并流顺序（stdout 后 stderr）不变 |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | 切 `run_shell`（`env CARGO_TARGET_DIR`、1800 s、cancel）；删私有 `read_all` |
| `crates/infra/checks/src/mutation/stryker.rs` | 切 `run_shell`（`cwd = suite_cwd`、1800 s、cancel）；删私有 `read_all` |
| `crates/infra/checks/src/mutation/mod.rs` | `SegmentFailure::Cancelled` 变体（`Display` 同型）；`execute → run_suite → execute_segment` 贯穿；取消面 `conclusion = Error` + 反馈文案 |
| `packages/desktop/package.json` | `version` 0.5.19 → 0.5.20（用户可见：停止/超时不再悬挂、超时预算 1200 s；沿「仅用户可见变更才提升」纪律） |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| `run_shell` 正常退出 / 双管道并发不互锁 / `cwd` 与 `env` 注入 | `crates/infra/exec/src/run_test.rs`（新） | 假 shell（`SystemCommand::script`）回显断言；双管道各写大块输出验不互锁 |
| `run_shell` 超时 → `RunOutcome::TimedOut` + `exit_code == -1` | `run_test.rs` | 睡眠 shim（`start_paused` + 手动 `advance`，沿 `mutation/*_test.rs` 既有先例） |
| `run_shell` 取消 → `RunOutcome::Cancelled` + pid 消失 | `run_test.rs` | shim 先落 pid 文件再自旋；`CancelHandle::set()` 后断言 pid 不在 |
| `SystemCommand` clone 等价 | `crates/infra/exec/src/lib_test.rs` | 补 `clone` 后 `into_std` 的 program / argv 逐字相等 |
| `CancelFlag::never()` / `set` 后 clone 共享 / 零 tokio 依赖 | `crates/core/foundation/src/cancel_test.rs`（新） | 纯 std 断言 + `Cargo.toml` 依赖面审查 |
| static-check 超时 → `Err` 且子进程回收；取消已置位 → `Err`（零 spawn 或即杀） | `crates/infra/checks/src/static_check_test.rs` | 新增两条（超时 shim / 取消 shim） |
| `SUITE_COMMAND_TIMEOUT` 常量锚定 600 → **1200** | `crates/infra/checks/src/testexec/runner_test.rs:1196`（`超时常量锚定`） | 期望值与注释随动（CLI 同值口径失效，改为「实测 340–393 s × 3 倍余量」） |
| testexec 取消 → `Err` | `runner_test.rs` | 新增（假 suite 自旋 + cancel 置位） |
| mutation 取消 → `SegmentFailure::Cancelled` → `conclusion = Error` | `crates/infra/checks/src/mutation/mod_test.rs` | 新增（`SpawnWindow` 假执行体自旋 + cancel） |
| 段超时目标 pid 树消失（`kill_tree` 生效锚） | `mutation/cargo_mutants_test.rs` / `mutation/stryker_test.rs` | 既有超时用例（`超时中断_强制终止不挂死`）扩展 pid 文件断言；文案断言不变 |
| 三 runner trait 假实现签名 + `ToolStepRequest` 字面量 | `core/orchestration/src/{port_test,steps_test,walker_test}.rs`、`infra/checks/src/mutation/mod_test.rs`、`infra/vcs/src/lib_test.rs` | **11 处假实现**（含 `UnusedSeamRunner` 两个 trait）加 `cancel` 参数；**~26 处** `ToolStepRequest` 构造字面量补 `cancel: CancelFlag::never()` |
| `request_stop` 双写 / 无订阅者不丢 | `core/orchestration/src/control_test.rs` | 新增（stop 后 `cancel_flag.is_set()` 为真；同刻幂等） |
| walker：stop 运行中的检查步 → 子进程被回收、秒级收敛 | `core/orchestration/src/walker_test.rs` | 新增（可取消假 runner：未置位则挂起，置位即返回 `Err("执行已取消")`；断言 run 收敛 failed 且不等挂起结束） |

### 不要修改

- `crates/infra/agent/src/sdk/bash.rs`（形态参考，不在迁移范围）；`crates/core/agent` / `crates/infra/agent` 任何文件。
- `crates/infra/vcs/src/lib.rs` 等生产源码（git 步 `bare` argv 直传，短命命令不需执行体）；`infra/vcs` 仅动 `lib_test.rs` 的假件签名。
- `crates/core/checks`（parser / aggregate / diagnose / reuse 纯层）；`exec::ProcessOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` / `kill_tree` 本体语义。
- `testexec/detect.rs`（框架注册表 / 版本探测 / 剪枝）；`crates/infra/vcs/src/bootstrap.rs`（worktree 建置安装命令：自带 900 s 轮询 + `child.kill()`，属 vcs 边界，不在本篇迁移面）。
- `crates/core/workflow`（步声明 / phase_table / 写面）；`crates/core/orchestration/src/state.rs`（`RunUpdate` / 节点词汇零扩展）。
- 前端 `packages/desktop/src/**` 与 `src/types/generated/bindings.ts`（无载荷字段变化，无需重生成）；`openspec/config.json`（schema 零扩展）。
- `plugins/**`（插件源已弃用，零改动零 bump）；`openspec/changes/exec-run-shell/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `exec::run_shell` 为四类 spawn 唯一执行体：`static_check.rs` / `testexec/runner.rs` / `mutation/cargo_mutants.rs` / `mutation/stryker.rs` 内无裸 `tokio::time::timeout` + `start_kill` 组合、无私有 `read_all` | `grep` 复核（`async fn read_all` 零残留、四文件无 `start_kill`）+ `cargo clippy` |
| AC-2 | `RunOptions.timeout` 为必填字段（无 `Default` / 无缺省值），四调用点预算常量锚定 static-check 1200 s / suite 1200 s / cargo-mutants 1800 s / Stryker 1800 s | `run_test` + `runner_test` / `cargo_mutants_test` 常量锚定用例 |
| AC-3 | 超时 → `RunOutcome::TimedOut`、`exit_code == -1`；Windows 上被 spawn 进程与其子孙全数回收（pid 文件断言）；静态检查步到点不再悬挂 | `run_test` 超时用例 + `static_check_test` 超时用例 + `cargo_mutants_test` / `stryker_test` pid 断言 |
| AC-4 | 取消 → `RunOutcome::Cancelled`、`exit_code == -1`、目标 pid 消失；`CancelFlag::never()` 恒 false | `run_test` 取消用例 + `cancel_test` |
| AC-5 | 双管道并发排水：stdout / stderr 各自大块输出同时产出时不互锁、不丢字节；`RunOptions` 的 `cwd` / `env` 生效（shim 回显断言） | `run_test` 双管道 + env / cwd 用例 |
| AC-6 | 取消贯通全链：`request_stop` 双写（watch + `cancel_flag`）且无订阅者不丢；`ToolStepRequest.cancel`；三 runner trait 带 `cancel`；`run_tool` 单点透传 `guard.cancel_flag()` | `control_test` + 编译期签名（11 处假件更新）+ `grep` 复核 `run_tool` 透传点 |
| AC-7 | 用户 stop 运行中的检查步 → 子进程被回收、run 在秒级收敛（不再「跑完为止」）；收敛终态为 **failed**（过渡形态，文案含「执行已取消」），不做 run stopped | `walker_test` 停止用例（挂起假 runner + stop + 秒级断言） |
| AC-8 | 既有语义零变：static-check 命令内 `2>&1` 保留（`stderr` 恒空、`diagnostics` 逐字等交错流）；testexec / mutation 合并流顺序（stdout 前缀 + stderr 后缀）不变；段报告 / summary 回填 / 复用门不受影响 | 既有 `static_check_test` / `runner_test` / `mutation/*_test` 全绿 + 新增断言 |
| AC-9 | `foundation::cancel` 零 tokio、零新增 workspace 内依赖；`exec` 依赖方向不变（仍仅 `foundation` + tokio，特性面只扩 `io-util` / `time` / `macros`） | `Cargo.toml` 审查 + `cargo tree` |
| AC-10 | 全部既有测试通过（含 11 处假件签名与 ~26 处 `ToolStepRequest` 字面量更新），覆盖率不低于阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90，基线 96.95 / 93.02 / 93.35） | `packages/desktop` 门禁：`pnpm run check`（`server:check` + `bindings:check` + `client:check`）+ `pnpm run server:test` + test-execution 门禁覆盖率 |
| AC-11 | `packages/desktop/package.json` 0.5.19 → 0.5.20；`plugins/**` 零改动零 bump；bindings 零 diff | 版本 diff + `bindings:check` |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | `run_shell` 抽象泄漏——各调用点需要不同能力（static-check 单流、testexec 双流合并、mutation env / cwd） | 抽象变重、回归面扩大 | 中 | `RunOptions` 只用四字段（`cwd` / `env` / `timeout` / `cancel`），不预铺未用字段；输出形态统一 `stdout` + `stderr` 两 `Vec<u8>`，合并顺序留调用方（既有语义逐字保留） |
| R-2 | static-check 由单管道改双管道后输出顺序变化 | 诊断文本与 record `.log` 不再还原终端交错 | 低 | **命令内保留 `2>&1`**：两 fd 指向同一管道，交错序在 OS 层合并、stderr 恒空，拼接与现状逐字等价（AC-8 钉住） |
| R-3 | 取消用例在 CI / 慢机上不稳定（自旋 shim 与轮询 200 ms 的竞态） | 偶发红 | 中 | 取消用例独立成条（不与既有用例共享装置）；全部既有用例以 `CancelFlag::never()` 缺省（零行为变化）；超时用例沿既有 `start_paused` + 手动 `advance` 先例 |
| R-4 | **unix 侧 `kill_tree` 语义弱于 Windows，且 `bootstrap` / 未来其它 spawn 面仍是「只杀壳」**：`kill -9 <pid>` 只杀直接子进程，cargo 的 rustc 子孙仍可能残留在 `target/` | 「进程树击杀」在 macOS / Linux 不成立 | 中 | 既有实现不改、本篇不扩（改它属 `exec` 契约变更）；spec scenario 明确 Windows `taskkill /T /F` 杀整树 / unix 杀直接子进程；进程组补强（`setsid` + `kill -9 -pgid`）与 `bootstrap` 的 `child.kill()` 收敛留作独立 change 的风险留痕。当前门禁 / 测试仅在 Windows 跑，故 AC-3 的 pid 树断言以 Windows 为准 |
| R-5 | 停止后 run 收敛为 **failed** 而非 stopped（用户可见语义） | 用户按停止后图上显示失败 | 高（本篇既定） | 过渡口径显式写进 spec scenario + 本篇范围裁决；`lock-fault-diagnostics` 落地时改判 stopped（另立 change，不属本篇） |
| R-6 | 新增 `exec::run.rs` 与 `foundation::cancel.rs` 稀释覆盖率 | functions / lines 跌破阈值（functions 距阈值余量 ~3.4 点） | 中 | 两模块均以单元测试全覆盖（正常 / 超时 / 取消 / 双管道 / clone / never）后再计入；`run_test` 与 `cancel_test` 是本篇测试主体的硬要求 |
| R-7 | `exec` 新增 tokio 特性面（`io-util` / `time` / `macros`） | 依赖面被质疑（crate 图审查） | 低 | 特性面仅扩实现所需（不引入 `rt-multi-thread` / `full`）；`Cargo.toml` 逐项注释理由，AC-9 钉住依赖方向 |
| R-8 | 取消在 `kill_tree` 与排水任务之间的时序处理不当 → `wait` / `read_to_end` 悬挂 | run 仍可能挂死（把「超时悬挂」换成「杀后悬挂」） | 中 | 收尸在前、排水任务以 kill 关闭管道后的 EOF 收口；`run_test` 的取消 / 超时用例断言墙钟上界（沿 `超时中断_强制终止不挂死` 既有上界断言形态） |
| R-9 | `SegmentFailure::Cancelled` 与 `Interrupted` 语义重叠，后续 stopped 收敛时改判成本 | 二次改动 | 低 | 变体显式独立（不复用 `Interrupted` 文案），`Display` 同型便于统一呈现；过渡文案「执行已取消」可被后续 change 直接映射为 stopped |
