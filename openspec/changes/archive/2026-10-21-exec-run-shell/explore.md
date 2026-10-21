# exec 统一执行体：超时 / 进程树击杀 / 取消

workflow_type: requirement

> **一句话**：把「跑一条 shell 行」收进 `infra/exec` 单一实现，四类 spawn
>（static-check、test-execution、cargo-mutants、stryker）统一获得
> **超时、进程树击杀、双管道排水、取消** 四能力。
>
> **状态**：设计完成，未实施。
> **依赖**：无（纯加法 + 四类调用点迁移）。本篇可独立创建 change。
> **不含**：瞬态重试、锁占用诊断、`RunFault` —— 见 `lock-fault-diagnostics.md`。

---

## 1. 问题：四个 spawn 点各写一遍，能力参差

| spawn 点 | 超时 | 进程树击杀 | 双管道 | 取消 |
|---|---|---|---|---|
| `infra/checks/src/static_check.rs:70` | **✗** | **✗** | ✗（单管道 `2>&1`） | ✗ |
| `infra/checks/src/testexec/runner.rs:110` | ✓ 600 s | **✗**（`start_kill()` 只杀壳） | ✓ | ✗ |
| `infra/checks/src/mutation/cargo_mutants.rs:65` | ✓ 1800 s | **✗** | ✓ | ✗ |
| `infra/checks/src/mutation/stryker.rs:62` | ✓ 1800 s | **✗** | ✓ | ✗ |

**参考实现已存在**：`infra/agent/src/sdk/bash.rs` 是正确形态
（`tokio::time::timeout` 罩全段 + `exec::kill_tree(pid)` + 双管道并发排水 + `kill_on_drop`）。

### 1.1 具体后果

| 缺口 | 后果 |
|---|---|
| static-check **零超时** | cargo 卡在 `Blocking waiting for file lock` 时**永久悬挂**，run 永远 running |
| `start_kill()` 只杀壳 | `cmd /C cargo test` 被杀，`cargo` → `rustc` 孙子进程继续写 target；下一轮撞锁 |
| mutation 段超时同样只杀壳 | `cargo mutants` 的 rustc 子孙继续写 `target/debug`（`CARGO_TARGET_DIR = exec_dir/target`，与两检查域**同一棵树**） |
| stop 只在步边界观测（`walker.rs:3452`） | stop 运行中的检查步无效，子进程跑完为止；立刻再发起 → 两条 cargo 同 target |

> 这些是**独立于文件锁**的缺陷：即使没有安全软件，也会因「超时/停止后残留进程」制造同类
> `os error 32`。

---

## 2. 设计

### 2.1 `infra/exec/src/run.rs`（新）

```rust
pub struct RunOptions {
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,   // CARGO_TARGET_DIR 等
    pub timeout: Duration,                // 必填——杜绝零超时悬挂
    pub cancel: CancelFlag,               // 取消信号（缺省 never）
}

pub enum RunOutcome { Completed, TimedOut, Cancelled }

pub struct RunReport {
    pub outcome: RunOutcome,
    pub exit_code: i32,                   // TimedOut/Cancelled 为 -1
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub async fn run_shell(command: SystemCommand, options: RunOptions) -> RunReport;
```

实现要点：

```
spawn（kill_on_drop(true)）
  ├─ 双管道并发 read_to_end
  └─ tokio::select! {
       cancel 轮询（200 ms，CancelFlag::is_set）
       timeout 到期
       wait() 收敛
     }
       ├─ 取消   → exec::kill_tree(pid) + wait 收尸 → Cancelled
       ├─ 超时   → exec::kill_tree(pid) + wait 收尸 → TimedOut
       └─ 正常   → Completed + exit_code
```

- `SystemCommand` 补 `#[derive(Clone)]`（program / args / raw 均可 Clone）。
- 取消用 200 ms 轮询而非 `Notify`：`foundation` 保持零 tokio 依赖。
- `kill_on_drop(true)` 兜底 future 被 drop 的路径。

### 2.2 `foundation::cancel`（新，零依赖）

```rust
#[derive(Clone)]
pub struct CancelFlag(Arc<AtomicBool>);
impl CancelFlag {
    pub fn never() -> Self;          // 测试零值
    pub fn is_set(&self) -> bool;
}
pub struct CancelHandle(CancelFlag);  // request_stop 侧写
impl CancelHandle { pub fn set(&self); }
```

> `foundation` 的既有纪律是「第二个消费者到场才下沉」。本处恰好有两个消费者
>（`RunGuard` 写、`run_shell` 读），成立。

### 2.3 取消贯通

```
core/orchestration/src/control.rs
  RunEntry.cancel_flag: CancelFlag
  request_stop() 双写：watch::send_replace(true) + cancel_flag.set()
  RunGuard::cancel_flag() -> CancelFlag

core/orchestration/src/port.rs
  ToolStepRequest.cancel: CancelFlag
  StaticCheckRunner::run(root, name, slot, cancel)
  TestExecutionRunner::run(root, name, slot, cancel)
  MutationRunner::run(root, name, files, patch, cancel)

core/orchestration/src/steps.rs   → 透传
core/orchestration/src/walker.rs  → 构造时取 guard.cancel_flag()

infra/checks/src/static_check.rs      → run_shell(..., cancel)
infra/checks/src/testexec/runner.rs   → run_shell(..., cancel)
infra/checks/src/mutation/cargo_mutants.rs → run_shell(..., cancel)
infra/checks/src/mutation/stryker.rs  → run_shell(..., cancel)
```

### 2.4 取消如何收敛

runner 返回 `Result<ToolStepOutput, String>`，`Err` 会被 `run_tool` 包成 failed。
因此取消**不能**走 `Err`。本篇的收敛口径：

```
run_shell 返回 Cancelled
  → runner 返回 Err("执行已取消")            ← 过渡形态
  → walker 收敛 failed
```

> **正式收敛（`fault=Stopped` → run stopped）由 `lock-fault-diagnostics.md` 承接。**
> 本篇只保证「进程确实被杀掉、不残留」，不改变终态语义。

### 2.5 超时值

| spawn 点 | 现状 | 本篇 |
|---|---|---|
| static-check | **无** | **1200 s**（冷构建约 90 s + vp 约 10 s，3 倍余量） |
| test-execution suite | 600 s | **1200 s**（实测 340–393 s，原阈值仅 1.7 倍余量） |
| cargo-mutants 段 | 1800 s | 1800 s（不变） |
| stryker 段 | 1800 s | 1800 s（不变） |

---

## 3. 改动清单

| 文件 | 改动 |
|---|---|
| `crates/infra/exec/src/run.rs`（新） | `run_shell` / `RunOptions` / `RunReport` / `RunOutcome` |
| `crates/infra/exec/src/lib.rs` | `SystemCommand` 加 `derive(Clone)`；导出 `run_shell` |
| `crates/core/foundation/src/cancel.rs`（新） | `CancelFlag` / `CancelHandle` |
| `crates/core/orchestration/src/control.rs` | `cancel_flag` 双写 + `RunGuard::cancel_flag` |
| `crates/core/orchestration/src/port.rs` | `ToolStepRequest.cancel`；三 trait 签名加 `cancel` |
| `crates/core/orchestration/src/steps.rs` | 透传 cancel |
| `crates/core/orchestration/src/walker.rs` | 构造 cancel 传入 |
| `crates/infra/checks/src/static_check.rs` | 切 `run_shell`（1200 s + cancel）；**保留命令内 `2>&1`** |
| `crates/infra/checks/src/testexec/runner.rs` | 切 `run_shell`（1200 s + cancel） |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | 切 `run_shell`（env `CARGO_TARGET_DIR` + cancel） |
| `crates/infra/checks/src/mutation/stryker.rs` | 切 `run_shell`（cwd = suite_cwd + cancel） |
| `crates/infra/checks/src/mutation/mod.rs` | `SegmentFailure::Cancelled` 新变体 + 贯穿 `execute → run_suite → execute_segment` |
| 测试假件 | **11 个 impl** 加 `cancel` 参数（`port_test` / `steps_test` / `walker_test` / `checks::mutation::mod_test` / `vcs::lib_test`）；~20 处 `ToolStepRequest` 字面量补 `cancel: CancelFlag::never()` |

**不改**：`bootstrap`（已有 900 s + `kill_tree`）、`detect` 版本探测（std 短命令）、
`git` 步（`bare` argv 直传）。

> **与篇 1 的协作**：篇 1 动 `runner.rs` 的 82–92 行，本篇动 99–150 行，
> 同文件不同区块；建议先合入篇 1。

---

## 4. 测试策略

| 层 | 用例 |
|---|---|
| `exec::run_shell` | 正常退出；超时 → `TimedOut` 且 pid 消失（shim 落 pid 文件）；取消 → `Cancelled` 且 pid 消失；env 注入经 shim 回显断言；双管道同时产出不互锁 |
| `foundation::cancel` | `never()` 恒 false；handle.set 后 flag true；clone 共享同一原子 |
| static_check | 超时 shim（睡眠超阈值）→ `Err`，子进程回收；取消已置位 → `Err`，零 spawn 或即杀 |
| testexec | 超时 → 既有 execution_error 语义不变；取消 → `Err` |
| mutation | 取消 → `SegmentFailure::Cancelled` → `conclusion=Error`（过渡）；段超时目标 pid 树消失（`kill_tree` 生效锚） |
| walker | stop 一个运行中的检查步 → 子进程被回收（对比修复前「跑完为止」） |

---

## 5. 验收标准

1. 四个 spawn 点全部经 `exec::run_shell`，代码中不再出现裸 `tokio::time::timeout` + `start_kill` 组合。
2. static-check 有超时（1200 s），到点后 cargo/rustc 进程树消失。
3. 用户 stop 运行中的检查步 → 子进程被回收，run 在秒级收敛（而非等命令自然结束）。
4. `SystemCommand` 在四个调用点行为等价（`2>&1` / `raw_arg` / env / cwd 语义零变化）。
5. 全部既有测试通过（含 11 个假件签名更新）。

---

## 6. 风险

| 风险 | 缓释 |
|---|---|
| `run_shell` 抽象泄漏（各调用点需要不同能力） | `RunOptions` 用 `Option`/缺省值；不预铺未用字段 |
| static_check 改双管道后输出顺序变化 | **命令内保留 `2>&1`**：交错序在 OS 层合并，stderr 恒空，拼接等价 |
| 取消语义在测试里不稳定 | `CancelFlag::never()` 给全部既有测试；取消用例独立 |
| 同文件与篇 1 冲突 | 已标注区块；先合篇 1 |
