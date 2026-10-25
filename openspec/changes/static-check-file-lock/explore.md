# 文件锁故障：剩余待办补丁

workflow_type: requirement

> **本篇定位**：`rust-suite-single-build` / `exec-run-shell` / `lock-fault-diagnostics`
> 三篇已进入变更，且**主体已落地**（见 §1）。本篇只保留**尚未实现**的补丁项，
> 作为同一次变更的追加范围。
>
> **核心依据**：失败率 ∝ 待写产物数 N（`碰撞概率 ≈ 1-(1-p)^N`）。失败不回滚已完成
> 产物 → **重试是确定性收敛**，N 单调递减至 0。实测 2–3 次内通过（§2）。
>
> 来源：2026-10-19 现场取证（日志、指纹、进程监视、S1 实测、代码核实）。

---

## 1. 已落地现状（不再需要改动）

代码核实结果（2026-10-19 当前版本）：

| 项 | 位置 | 状态 |
|---|---|---|
| S1 单构建模板 | `testexec/detect.rs` rust 档 shell / cmd 两臂 | ✅ `cargo llvm-cov --workspace ... > "{results_file}"` |
| 删 `applyResultsRedirect` | `testexec/runner.rs` | ✅ 已删（注释明示「展开后零改写」） |
| `exec::run_shell` | `infra/exec/src/run.rs` | ✅ timeout / 双管道 / `kill_tree` / cancel |
| 四类 spawn 迁移 | static_check / testexec / cargo_mutants / stryker | ✅ 全用 `RunOptions` |
| 瞬态签名表 | `exec/src/transient.rs` | ✅ 13 条签名 + `is_transient` |
| 重试策略 | `RetryPolicy::transient()` / `none()` | ✅ 5 次 / 1500s 预算 |
| 锁探针 + 归因 | `exec/src/lockprobe.rs` + `attribution_text` | ✅ 接入 `fault_text` |
| `RunFault` 三载荷 | `orchestration/src/port.rs` | ✅ `Stopped` / `Environment` |
| walker fault 分派 | `walker.rs` 五处 | ✅ 含 `TXT_ENV_STATIC` / `TXT_ENV_TEST` |
| 补丁 D（I1） | `initialize` 两检查步仍 `on_fail=Terminal`，重试在 runner 内消化 | ✅ 天然满足 |

**结论：三篇主体 + 补丁 D 已完成。以下 §3 的 6 项是剩余缺口。**

---

## 2. 依据：增量收敛（为何这些补丁有效）

### 2.1 决定性观测

**不做任何干预，静态检查与测试各重试一次即可通过。**

`archive/2026-10-19-test-execution-zero-agent-decision/reports/baseline/exec-outputs/`
连续 attempt 的 `Compiling` 行数：

| attempt | Compiling | 结果 |
|---|---|---|
| 0 | **264** | 失败 `failed to write …libpotential_utf-…rmeta` |
| 1 | **5** | 失败 |
| 2 | **0** | 失败 `failed to build archive …`（只剩 rename 收尾） |
| 3 | 0 | 失败 |
| 4 | 0 | 失败（`could not compile axum`） |
| 5 | **192** | 失败（新一轮：static-check 的 `cargo run` 又写 `target/debug`） |
| 6 | **82** | 失败 |
| 7 | **2** | 失败 |
| 8 | **0** | ✅ **成功** |

### 2.2 机制

```
rustc 每写一个 .rlib/.rmeta = 一次「写临时文件 → rename」
    └─ 概率 p 被内核 minifilter 拦下 = os error 32

一次构建写 N 个产物
    碰撞概率 ≈ 1 - (1-p)^N

关键：失败不回滚已完成产物
    attempt 0: N=264 → 大面积失败，但前 200+ 个已落盘
    attempt 1: N=5   → 只剩尾部少数
    attempt 2: N=0   → 只剩 rename/清理收尾
    attempt k: 静默窗口无写入 → 成功
```

**重试是「分批填满 target」的确定性收敛，不是碰运气。**

### 2.3 根因（三篇已覆盖，此处仅备查）

两套企业级安全软件的**内核 minifilter**：

| 持有者 | 实体 |
|---|---|
| `HipsDaemon.exe` / `usysdiag.exe` | 火绒**企业版**（`ESEndpoint`，服务端策略托管） |
| `TBProtect.exe` + `tbprotect64.sys` | 云枢 YunShu（含 Sophos AV 引擎、HIDS） |
| `System [pid 4]` | 上述驱动的内核态持有 |

三个动词全部沦陷：`rename` / `write` / `remove` —— 共同点是需要 **DELETE 共享语义**。

环境侧排除（云枢 + 火绒企业版，排除 `C:\Users\wps\.dev-team` 父目录）是根治 **p** 的手段，
需 IT；代码侧只能压低 **N** 的影响。

---

## 3. 剩余待办补丁

### P1（最高）增量推进判据：区分「在推进」与「卡死」

**现状**：`exec/src/run.rs:167` 只有两个早停条件

```rust
if attempts >= retry.max_attempts || started.elapsed() >= retry.total_budget {
    return Ok(attempt.into_report(attempts, retried));
}
```

固定 5 次，**不区分「还在推进」与「同一批写不进去」**。实测两种形态都存在：

```
N_i < N_{i-1}         264 → 5 → 0     ← 在推进，值得继续
N_i == N_{i-1} > 0    例如 50 → 50    ← 卡死，继续重试是浪费
```

**改动**：

```rust
// exec/src/transient.rs（新）
/// 从合并输出解析「待编译」计数：`Compiling <crate> v…` 行数。
/// 无该字样 → 0（非 cargo 构建形态，退化为固定次数语义）。
pub fn compiling_count(stdout: &[u8], stderr: &[u8]) -> u32;

// exec/src/run.rs（重试循环内）
// 连续两次命中签名且 compiling 计数未下降且 > 0 → 判卡死，提前收口
let previous = compiling_count_of_last_attempt(...);
let current  = compiling_count(&attempt.stdout, &attempt.stderr);
if current > 0 && current >= previous {
    return Ok(attempt.into_report(attempts, retried));   // 卡死早停
}
```

**注意**：
- `attempt == 1` 时无 `previous`，不触发早停。
- `current == 0` 不触发早停 —— 「只剩收尾」恰恰是最该再试的形态（实测 attempt 2/3/4 都是 N=0）。
- 判据只用于**提前放弃**，不用于延长预算（预算仍是 1500s 硬顶）。

**验收**：shim 制造「连续两次相同 Compiling 计数」→ `attempts < max_attempts` 即收口；
「计数递减」→ 继续到成功或预算耗尽。

---

### P2 退避参数上调

**现状**（`exec/src/transient.rs:47-53`）：

```rust
const RETRY_BACKOFF: [Duration; 4] = [1s, 2s, 4s, 8s];
```

**改为**：

```rust
const RETRY_BACKOFF: [Duration; 4] = [5s, 10s, 20s, 40s];
```

**理由**：实测静默窗口可达分钟级。1 s 退避在「只剩 rename 收尾」（N=0）的 attempt 上
大概率撞同一个扫描窗口 —— 而这正是最需要耐心的一档。

**副作用**：成功路径零影响（不命中签名不重试）；只有真需要重试时多等 4+9+16+32 = 61 s
（最坏 5 次），可接受。

---

### P3 诊断文案补「推进序列」

**现状**（`infra/checks/src/environment.rs:91` `fault_text`）：

```
被锁文件：...
已自动重试 4 次（每次重试会跳过已完成部分）
最后一次诊断：
...
环境侧处理：请 IT 在【火绒企业版（服务端策略）】与【云枢】中排除 ...
```

**缺一行**（与 P1 同源，`RunReport.retried` 已带全部字节）：

```
重试进展：待编译 264 → 5 → 0 → 0（3 次后仍失败，判定为卡死）
```

**改动**：`fault_text` 增加 `run.retried` 参数（当前签名只有 `attempts` / `exit_code` / `output`），
按序调 `compiling_count` 生成序列；`attempts == 1` 时不输出该行（未重试，无序列可言）。

**价值**：把「重试确实在推进」变成用户可见证据，同时是 P1 判据的自证材料。

---

### P4 签名表死条目修正

**现状**（`exec/src/transient.rs:33`）：

```rust
"failed to remove file",   // ← 永不命中
```

实际错误文本是 `failed to remove C:\...\xxx.rcgu.o`，**不含 "file" 字样**。
功能上被同一行内的 `os error 32` 兜住，但会误导维护者以为 remove 形态有独立覆盖。

**改为**：

```rust
"failed to remove",   // 收窄风险低：该短语只出现在删除失败语境
```

**验收**：`transient_test` 补一条「`failed to remove <path>: 另一个程序…` 命中」的表驱动用例
（现有用例只覆盖 `failed to remove file`，与生产文本不符）。

---

### P5 test-execution 跨 plan 总预算

**现状**：`testexec/mod.rs` 逐 plan 调 `execute_plan`，每个 plan 各自
`RetryPolicy::transient()`（per-plan 1500 s），**无跨 plan 总闸**：

```
rust suite   393s × 最多 5 次 ≈ 1965s（受 per-plan 1500s 预算约束）
vite-plus     ~15s × 5 次      ≈   75s
────────────────────────────────────────
最坏 wall clock ≈ 1575s，walker 侧无总超时
```

当前配置（1 rust + 1 vite）尚可承受；suite 数量增长后会变成隐患（一个 run 可能挂 40 分钟）。

**改动**：`execute()` 层记录起点，进入下一 plan 前判定：

```rust
const TESTEXEC_TOTAL_BUDGET: Duration = Duration::from_secs(2400);
// 超预算 → 剩余 plan 不 spawn，整体以 fault=Environment("测试执行总时长超预算") 收口
```

**注意**：不要把已完成的 plan 结果丢掉 —— 沿用现有 `fault_outcome` 路径（它本就不写
summary、不进复用门）。

---

### P6 覆盖率聚合「假通过」

**现状**（`core/checks/src/aggregate.rs:234` `weighted_measured`）：

```rust
let has_any = sub_reports.iter().any(|report| { ... coverage.is_some() ... });
if !has_any { return None; }
// 有任意一个 suite 有覆盖率 → 只用它们的加权，缺覆盖率的 suite 被静默跳过
```

历史实证（`archive/2026-10-18-desktop-agent-chat-component/reports/baseline/summary.json`）：

```json
"conclusion": "error",
"coverage": { "pass": true, "measured": { "lines": 96.21, ... } }   // 数值全来自 vite-plus
```

rust suite 一个覆盖率数都没产出，聚合面却报 `pass: true`。门禁结论靠 `problems` 兜住了，
但「覆盖率通过」这句话是假的。

**改动**：某 suite 结论非 pass 且 `coverage == null` 时，向 `problems` 追加一条
`ProblemType::ExecutionError`（`determine_conclusion` 已把它映射为 `Error`，无需改判定函数）。

**验收**：rust 覆盖率缺失 + vite 覆盖率达标 → `conclusion = error` 且 `coverage` 不入
`pass: true`（或 `coverage` 为 `None`）。

---

### P7（探索项）worktree 根位置

**现状**（`src-tauri/src/main.rs:18`）：

```rust
const DATA_DIR: &str = ".dev-team";
let data_root: PathBuf = app.path().home_dir()?.join(DATA_DIR);
// → C:\Users\wps\.dev-team\worktrees\<身份段>\<change>
```

支撑数据：失败行 **21 条全在 `C:\`**，`D:\Projects`（主仓）**0 条**。

但两个假设未分离：

```
H1 路径   C:\Users 被安全软件扫描更激进（N 相同，p 更大）
H2 冷热   worktree 每次新建 → target 冷 → N 大（与盘符无关）
```

**先做分离实验（5 分钟，不改代码）**：

```powershell
cd "C:\Users\wps\.dev-team\worktrees\<hash>\<change>\packages\desktop\src-tauri"
move target target.bak
cmd /c mklink /J target D:\devteam-target\<change>
pnpm run check
# 收尾：cmd /c rmdir target && move target.bak target
```

| 结果 | 结论 | 后续 |
|---|---|---|
| 成功 | H1 成立 | 改 `DATA_DIR` 或 target 落位到非用户目录（一个常量） |
| 仍失败 | H2 成立 | 只能靠重试 + 环境排除 |

> 若 H1 成立，这是成本最低的根治手段（不依赖 IT），优先级可升到 P1。

---

## 4. 实施顺序与验收

| 顺序 | 补丁 | 改动规模 | 验收 |
|---|---|---|---|
| 1 | **P1** 增量判据 | `transient.rs` +`compiling_count`；`run.rs` 重试循环 +8 行 | 卡死形态提前收口；递减形态跑到底 |
| 2 | **P3** 推进序列文案 | `environment.rs` `fault_text` 加参 | 终局文案含 `待编译 264 → 5 → 0` |
| 3 | **P2** 退避上调 | 一个常量数组 | 慢档重试不再撞同一窗口 |
| 4 | **P4** 签名修正 | 一个字符串 + 测试 | remove 形态真实命中 |
| 5 | **P5** 跨 plan 总预算 | `testexec/mod.rs` +常量 | 多 suite 时 run 墙钟有上界 |
| 6 | **P6** 覆盖率假通过 | `aggregate.rs` +problem | 缺覆盖率的失败 suite 不再报 pass |
| 7 | **P7** worktree 根 | 先实验，后决定 | 视 H1/H2 结果 |

**P1–P4 属同一主题**（重试质量），建议一起做；**P5 / P6 独立**；**P7 待实验**。

### 硬验收（整体）

1. 冷 target（新建 worktree）上，静态检查 / 测试执行**不需人工干预**即可通过。
2. 重试耗尽时终局文案含占用进程名 + `Compiling` 推进序列。
3. `N_i == N_{i-1} > 0` 时提前放弃，不烧满 5 次预算。
4. 真红（无签名）零重试，行为与现状一致。
5. 覆盖率缺失的失败 suite 不再让 `coverage` 报 `pass`。

---

## 5. 已废弃 / 已实现项（不再实施）

| 项 | 处置 |
|---|---|
| S1 模板合并 + 覆盖率口径（25 → 209 文件） | ✅ 已实现（篇 1） |
| `exec::run_shell` / `CancelFlag` / 四类 spawn 迁移 | ✅ 已实现（篇 2） |
| 精确锁探针 + Restart Manager 点名 | ✅ 已实现（`lockprobe.rs`） |
| `RunFault` 语义 + walker 五处分派 + 环境文案 | ✅ 已实现（篇 3） |
| 补丁 D（`initialize` 自动重入 I2） | **废弃** —— 重试已在 runner 内消化（I1），实测 2–3 次足够，无需要改相位机 |
| target 隔离（注入 `CARGO_TARGET_DIR` / IDE 让路） | **废弃** —— 内核 minifilter 扫任意路径，换目录无效 |
| 杀软白名单（用户侧） | **降级** —— 云枢未排除 + 火绒企业版服务端策略，仍应推动 IT，但不作为代码方案 |
| 「关 VSCode 就成功」作为因果 | **废弃** —— 真主因是 N（增量推进），VSCode 仅影响写入速率 |

---

## 6. 附：已知但不在本次范围

| # | 项 | 说明 |
|---|---|---|
| 1 | rust suite 仍跑两遍测试 | S1 已合并为一条 `cargo llvm-cov`，但 llvm-cov 内部仍会跑测试——属工具行为，无进一步优化空间 |
| 2 | `export-bindings` 覆盖率 0% | `crates/tools` 无测试；计入分母但不拖累（实测 lines 96.64 ≥ 90）。补测试可另立变更 |
| 3 | 云枢是否提供本地排除 UI | `TBProtect.exe` 无版本信息，可能云控下发，需 IT 确认 |
| 4 | 火绒企业版排除是否即时生效 | minifilter 通常需重启驱动 |
