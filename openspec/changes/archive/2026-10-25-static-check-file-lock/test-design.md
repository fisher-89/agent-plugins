# 测试设计: static-check-file-lock

> **变更**: static-check-file-lock
> **日期**: 2026-10-23
> **基于**: `proposal.md`（验收标准 AC-1 ~ AC-10；「变更范围 - 实现文件」9 项 +「变更范围 - 测试文件」表）、`design.md`（「新增文件」零项 /「修改文件」9 项；「公共函数 / API」子表 8 行；「类型定义」零改动；「配置」子表 3 行；关键设计决策 D1 ~ D16）、`specs/{desktop-exec-runtime,desktop-checks-domain}/spec.md`
> **被测模块来源**: `design.md`「修改文件」9 项——`crates/infra/exec/src/{transient,run,lib}.rs`、`crates/infra/checks/src/{environment,static_check}.rs`、`crates/infra/checks/src/testexec/{runner,mod}.rs`、`crates/core/checks/src/aggregate.rs`、`packages/desktop/package.json`。design 无「新增文件」子表（六处补丁全落既有落位）与「删除文件」子表。
> **测试框架**: 沿用仓库既有约定——Rust 侧 `#[test]` / `#[tokio::test]`（dev-deps `tokio` 的 `rt` / `macros` / `test-util`），测试模块以**同目录兄弟文件**承载：exec / checks 子模块用 `#[cfg(test)] #[path = "<name>_test.rs"] mod <name>_test;`（`transient.rs` / `run.rs` / `environment.rs` / `testexec/runner.rs` 先例）、crate 根模块与 `testexec/mod.rs` 用 `#[cfg(test)] mod <name>_test;`（`exec/src/lib.rs` / `checks/src/testexec/mod.rs` / `core/checks/src/lib.rs` 先例）；断言用标准 `assert!` / `assert_eq!` / `matches!`，测试函数名为中文标识符、断言消息为中文；无外部断言库、无独立测试运行器。本变更**零新增测试文件**（8 个映射对全部为既有 `*_test.rs` 的改写 / 新增用例），故不新增任何仅供测试的 `export`（AGENT.md 纪律）——三处新函数本就有生产调用点。

用例表读法：`路径类型` ∈ {正向, 异常, 边界}；`迭代类型` ∈ {单值, 参数化, 表驱动, 遍历枚举}。跨模块组合用例按「链路发起方 / 最上层调用方」挂靠入口模块的 `#### 用例` 表（本变更组合锚分别挂靠 `run.rs`——重试循环 + 卡死早停全链、`static_check.rs`——推进判据到 fault 文本全链、`testexec/runner.rs`——suite 执行到 fault 文本全链、`testexec/mod.rs`——多 plan 总预算到 `fault_outcome` 全链、`aggregate.rs`——缺测归因到结论 / 复用门全链），不设独立集成测试章节。

边界取值口径（按 design「公共函数 / API」签名逐项收敛）：

| 类型（载体） | 边界取值 | 最少条数 |
|---|---|---|
| `&[u8]`（`compiling_count` 两路入参 / `fault_text.output`） | `[]`（双空）/ 单字节 / 大块（>64 KiB、数千行）/ 非 UTF-8 原始字节（`0xFF 0xFE`）/ CRLF 与 `\r` 残留 / 前导空白（空格 / 制表符 / 混合）/ 特殊字符（引号 / `\0` / emoji） | 7 边界 + 1 正常 |
| `u32`（`compiling_count` 产出 / `RunReport.attempts` / 序列元素） | `0`（非 cargo 形态）/ `1` / `5`（上限形态）/ `264`（现场值）/ `u32::MAX`（防御性直构） | 4 边界 + 1 正常 |
| `Vec<u32>`（`progress_series` 产出 / `is_progress_stalled` 入参） | `[]`（空）/ 长度 1（`attempts == 1`）/ 末两次相等 / 末两次上升 / 严格递减 / 全 `0` / 末值 `0` 且前值非 0 / 长度 5（生产上限）/ 长度 1000（超大列表） | 8 边界 + 1 正常 |
| `Vec<AttemptTrace>`（`progress_series` / `fault_text` 的 `retried`） | `[]`（未重试）/ 1 条 / 3 条 / 4 条（耗尽面）/ 5 条（直构越出生产不变量）；条目内 `transient: Some(原文)` 与 `None` 两形态 | 4 边界 + 1 正常 |
| `Duration`（`backoff` 四段 / `total_budget` / `TESTEXEC_TOTAL_BUDGET`（私有常量经模块内断言）/ 注入预算） | `ZERO` / 毫秒级（1 ms、50 ms）/ `[5 s, 10 s, 20 s, 40 s]` / `1500 s` / `2400 s` | 4 边界 + 1 正常 |
| `Option<CoverageBlock>`（`SubReport.coverage`） | `None`（缺测）/ `Some(pass: true)` / `Some(pass: false)`（有覆盖率未达阈） | 2 边界 + 1 正常 |
| `Option<RunFault>` | `None` / `Some(Stopped)` / `Some(Environment(""))` / `Some(Environment(超长 · 特殊字符))` | 4 边界 + 1 正常 |
| `i32`（`exit_code`） | `0`（绿）/ `1` / `255` / `-1`（`TimedOut` / `Cancelled` 哨兵） | 3 边界 + 1 正常 |
| enum | `RunOutcome` 三值、`RunFault` 两变体、`Conclusion` 三值、`ProblemType` 三值逐个取值遍历（封闭集，非法值属编译期不可表达） | N / N+1 |

路径省略说明：`验收范围` 表内 `.../` 前缀补齐为 `packages/desktop/src-tauri/crates/`；下文 `## 单元测试` 各节标题给全路径。

---

## 验收范围

| AC ID | 验收条件（摘要，原文见 `proposal.md`） | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | 卡死形态（`N_i == N_{i-1} > 0`）提前放弃：`attempts < max_attempts` 即收口，不烧满 5 次 | `.../exec/src/transient.rs` -> `.../exec/src/transient_test.rs`（`is_progress_stalled` 判据 + `compiling_count` 计数）、`.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（卡死 shim 用例：`attempts == 2`、第 3 次零 spawn） |
| AC-2 | 递减形态（`264 → 5 → 0`）不早停、继续到成功或预算耗尽；末次计数 `0` 相等亦不早停 | `.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（递减四段 / 末值归零两臂 + 上升计数对照臂）、`.../exec/src/transient.rs` -> `.../exec/src/transient_test.rs`（判据反例：递减 / 末值 0 / 长度 1）、`.../infra/checks/src/static_check.rs` -> `.../static_check_test.rs`（递减序列组合锚：无卡死判定语） |
| AC-3 | 退避为 `[5, 10, 20, 40] s`（代码常量锚定）、`total_budget = 1500 s` 不变、尝试上限 5 不变；成功路径零额外等待 | `.../exec/src/transient.rs` -> `.../exec/src/transient_test.rs`（`RetryPolicy::transient` 三常量锚定）、`.../exec/src/lib.rs` -> `.../exec/src/lib_test.rs`（再导出面退避四段断言改写）、`.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（退避逐段 5 s / 10 s + 成功路径零等待） |
| AC-4 | remove 形态真实命中：`failed to remove D:\…\x.rcgu.o` 触发瞬态判定并返回表内原文 `failed to remove` | `.../exec/src/transient.rs` -> `.../exec/src/transient_test.rs`（表驱动命中用例 + 13 条封闭表计数不变） |
| AC-5 | 重试耗尽终局文案含占用进程名 + `重试进展：待编译 …` 序列（卡死时含判定语）；未重试（`attempts == 1`）或非 cargo 输出（全 `0`）时无该行 | `.../infra/checks/src/environment.rs` -> `.../environment_test.rs`（两臂 + 抑制两臂 + 行位断言）、`.../infra/checks/src/static_check.rs` -> `.../static_check_test.rs`（组合锚：卡死推进序列到 fault 文本）、`.../infra/checks/src/testexec/runner.rs` -> `.../testexec/runner_test.rs`（组合锚：合并流序列到 fault 文本） |
| AC-6 | 多 suite 时 run 墙钟有上界：超 `TESTEXEC_TOTAL_BUDGET` 后余下 plan 零 spawn、整体以 `Environment` fault 收口（不写 summary、不进复用门），既有 fault 不被覆盖 | `.../infra/checks/src/testexec/mod.rs` -> `.../testexec/mod_test.rs`（常量锚定 + 预算内零变 + 超预算零 spawn + 不覆盖 Stopped / Environment + 复用门不进） |
| AC-7 | 覆盖率缺测不再假通过：rust 缺覆盖率 + vite 达标 → `conclusion == Error` 且 `coverage` 不为 `pass: true`（收敛 `None`）；同 suite 不重复归因 | `.../core/checks/src/aggregate.rs` -> `.../aggregate_test.rs`（`aggregate_coverage` 收敛 + `build_summary_report` 缺测归因 / 去重 / 齐全零变三臂 + 既有语料改写） |
| AC-8 | 既有语义零变：真红零重试 `attempts == 1`；`TimedOut` / `Cancelled` 零重试；static-check `2>&1` 交错流与 `diagnostics`、test-execution 合并流顺序、`report_dir` / `exec_id` 回填、复用门判定、基线与 mutation 分流逐字节不变 | `.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（超时 / 取消 / 真红 / `none()` / 单次等价 / 末次非合流）、`.../exec/src/lib.rs` -> `.../exec/src/lib_test.rs`（既有导出面零变）、`.../infra/checks/src/static_check.rs` -> `.../static_check_test.rs`、`.../infra/checks/src/testexec/runner.rs` -> `.../testexec/runner_test.rs`、`.../infra/checks/src/testexec/mod.rs` -> `.../testexec/mod_test.rs`（复用门 / summary / `exec_id`）、`.../core/checks/src/aggregate.rs` -> `.../aggregate_test.rs`（全测齐全 / 全无覆盖率两形态零变） |
| AC-9 | 硬验收（现场回归，非 CI 可断言）：冷 target 上静态检查 / 测试执行无需人工干预即可通过；重试耗尽时终局文案含占用进程名与推进序列 | 无单元测试路径 → `## 不可测试项` 第 2 项（真实 worktree 人工验收）；文案面原料由 AC-5 的 `environment_test` / `static_check_test` / `runner_test` 覆盖 |
| AC-10 | 门禁：`pnpm -C packages/desktop run check` 全绿、覆盖率 ≥ lines 90 / branches 85 / functions 90、`bindings:check` 零 diff、`version` 0.5.28 → 0.5.29、`plugins/**` 零改动 | 全部 8 个映射对（逐文件 `#### 用例` 表）+ `packages/desktop/package.json` / `plugins/**` / 覆盖率统计面 → `## 不可测试项` 第 1、3、4、5 项 |

> **被测范围边界**：`crates/infra/exec/src/{output,shell,lockprobe}.rs` 与 `record_exec_output` / `read_exec_output` / `mint_exec_id` / `kill_tree` 本体、`RunOptions` / `RunReport` / `RunOutcome` / `RunError` 字段面、`crates/infra/checks/src/{lib.rs,mutation/**}`、`crates/infra/checks/src/testexec/{detect,report}.rs`、`crates/core/orchestration/**`、`crates/core/foundation/**`、`crates/core/checks` 其余纯层（parser / reuse / diagnose / model / globmatch）、`packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`packages/desktop/src-tauri/src/main.rs`（`DATA_DIR`）、`plugins/**` 均为 design「只读红线零触点」，不纳入被测范围。
>
> **既有测试语料的连带改写**（design 待决问题 1 / tasks 阶段七 ⑦）：`aggregate_test.rs` 的 `汇总组装逐字段相等` 在新判据下必然改写（vite-plus `failed = 1` + `coverage = None` 会多一条 `execution_error`、`coverage` 收敛 `None`、`conclusion` 由 `Fail` 升 `Error`，见 D14）；本设计把它拆为「全测齐全（零变）」与「缺测归因」两臂（见 `aggregate.rs` 节）。`transient_test.rs` 两条常量锚定用例与 `lib_test.rs` 的退避断言随 P2 改值；`run_test.rs` 的退避逐段用例改取策略常量（1 s / 2 s 硬编码作废）。

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/exec/src/transient.rs -> packages/desktop/src-tauri/crates/infra/exec/src/transient_test.rs

<!-- design.md「公共函数 / API」子表四行（`compiling_count` / `progress_series` / `is_progress_stalled` / `RetryPolicy::transient`，「所在文件」列为本文件）落本节。测试模块以 `#[cfg(test)] #[path = "transient_test.rs"] mod transient_test;` 挂载（既有形态）；`pub(crate) fn transient_signature(..)` 为私有判定核心，经该内嵌模块可直驱。P4 的签名表改写经公共面 `is_transient` 断言（design 公共函数表未列 `is_transient`——其签名零改，故不作独立待测功能条目）。 -->

#### 待测功能

- `compiling_count(stdout: &[u8], stderr: &[u8]) -> u32`（公共函数表，P1/D1）：合并 `stdout ++ stderr`（UTF-8 lossy）后按 `.lines()` 逐行、`line.trim_start().starts_with("Compiling ")`（大小写**敏感**、含尾随空格）计数；无该字样 → `0`；零 IO / 零 spawn / 零 panic
- `progress_series(report: &RunReport) -> Vec<u32>`（公共函数表，P1/D2）：`retried` 按序在前（各取该次两路的 `compiling_count`）+ 末次一条（`report.stdout` / `report.stderr`）；长度 = `retried.len() + 1`（生产不变量 `retried.len() + 1 == attempts` 下等于 `attempts`；未重试即长度 1）
- `is_progress_stalled(series: &[u32]) -> bool`（公共函数表，P1/D3）：`series.len() < 2` → `false`；否则 `末值 > 0 && 末值 >= 前值`（相等与上升都判卡死；末值 `0` 恒判否）
- `RetryPolicy::transient() -> RetryPolicy`（公共函数表，P2/D5，签名不变仅常量改值）：`max_attempts = 5`、`backoff = [5 s, 10 s, 20 s, 40 s]`、`total_budget = 1500 s`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `compiling_count` — 多行计数 | 正向 | `stdout = "   Compiling a v0.1.0\n   Compiling b v0.2.0\n"`（rustc 缩进形态）→ `2`；`stderr` 侧另含 1 行 `Compiling c` → `3`（两路合并口径）；末行 `Finished dev …` 不计数 | 表驱动（3 形态） |
| `compiling_count` — 非 cargo 输出为 `0` | 异常 | 仅锁错误行（`error: failed to rename archive file: …: os error 32`）/ 普通编译错误（`error[E0308]`）/ `compiling a`（全小写）/ `Compiling`（无尾随空格）/ `Compilingx`（前缀不成立）/ 行中 `error: Compiling failed`（非行首）→ 均 `0` | 表驱动（6 形态） |
| `compiling_count` — 前导空白与行内形态 | 边界 | 前导空格 / 制表符 / 混合空白 + `Compiling ` → 计数；`Compiling ` 后直接换行（无 crate 名）→ 计数；行首 `\0` / emoji 前置 → 不计数（`trim_start` 只去空白）；行中缩进多级仍计数 | 表驱动（5 形态） |
| `compiling_count` — 字节面边界 | 边界 | 双空 `[]` + `[]` → `0`；单字节 `b"x"` → `0`；`>64 KiB` 且含 5000 行 `Compiling ` → `5000`（不截断）；非 UTF-8 前缀字节（`0xFF 0xFE\nCompiling x\n`）→ `1`（lossy 不 panic）；纯 CRLF 行尾 → 逐行计数（`lines()` 吃 `\r`） | 表驱动（5 形态） |
| `progress_series` — 未重试长度 1 | 正向 | `RunReport { attempts: 1, retried: [], stdout: 5 行 Compiling, stderr: 空 }` → `vec![5]`（长度 1 == `attempts`） | 单值 |
| `progress_series` — 序与长度（生产形态） | 正向 | `retried = [264 行, 5 行, 0 行]` + 末次 `0 行`、`attempts = 4` → `[264, 5, 0, 0]`（按尝试序，末次在后，长度 == `attempts`） | 单值 |
| `progress_series` — 耗尽与越界长度 | 边界 | `retried` 4 条 + 末次 → 长度 5（`transient()` 上限形态）；`retried` 5 条直构（越出生产不变量）→ 长度 6、零 panic | 表驱动（2 形态） |
| `progress_series` — 直构违反不变量 | 边界 | `attempts = 3` 但 `retried = []`（直构语料）→ 长度 1（doc 口径「长度 = `retried.len() + 1`」，不按 `attempts` 补齐）；`attempts = 0` 直构同样不 panic | 表驱动（2 形态） |
| `progress_series` — 两路合并与字节形态 | 边界 | 某 `retried` 条目的 `Compiling` 行仅在 `stderr` → 计入该位；末次 `stderr` 侧含 `Compiling` 行 → 计入末位；非 UTF-8 / emoji / `\0` 混排 → lossy 计数不 panic | 表驱动（3 形态） |
| `is_progress_stalled` — 判卡死 | 正向 | `[50, 50]` / `[264, 5, 5]` / `[5, 9]`（上升）/ `[u32::MAX, u32::MAX]` → `true` | 表驱动（4 形态） |
| `is_progress_stalled` — 推进或收尾不卡死 | 异常 | `[264, 5]` / `[264, 5, 0]` / `[264, 5, 0, 0]` / `[9, 5]`（递减）/ `[10, 0]`（末值归零）→ `false` | 表驱动（5 形态） |
| `is_progress_stalled` — 空与单值 | 边界 | `[]` → `false`；`[50]` → `false`（`attempts == 1` 无前值）；`[0]` → `false` | 表驱动（3 形态） |
| `is_progress_stalled` — 全 0 与大列表 | 边界 | `[0, 0]` / `[0, 0, 0, 0]` → `false`（末值 `0` 恒判否）；`[0, 3]` → `true`（前值 0、末值 > 0 仍判卡死）；1000 项全等 → `true`（超大列表不退化） | 表驱动（3 形态） |
| `RetryPolicy::transient()` — 三常量锚定 | 正向 | `max_attempts == 5`、`backoff == [5 s, 10 s, 20 s, 40 s]`、`total_budget == 1500 s`（改值即被击杀，AC-3） | 单值 |
| `RetryPolicy` — 常量关系与两构造 | 边界 | `backoff` 四段严格递增且总和 `75 s < total_budget`；`none()` 的 `backoff` / `total_budget` 取同类常量（不被消费）、`max_attempts == 1`；`none() != transient()`、`clone` / `Debug` 面保留 | 表驱动（3 形态） |
| 签名表 remove 措辞（P4，经 `is_transient` 公共面） | 正向 | `completed` + `exit 1` + `failed to remove D:\w\target\debug\deps\x.rcgu.o`（同一行无 `os error` 码）→ `Some("failed to remove")`（返回**表内原文**）；`failed to remove file` 旧措辞输入同样返回 `"failed to remove"`（表中已无 `file` 分支） | 表驱动（2 形态） |
| 签名表封闭性（P4 连带） | 边界 | 逐条硬编码 13 条（第 6 条为 `failed to remove`）→ 逐条命中且返回原文（删条目 / 改写法即被击杀）；大小写不敏感 / 两路合并 / 多行噪声 / `TimedOut`·`Cancelled` 恒 `None` 既有断言保留 | 表驱动（13 条封闭表） |
| 输出字节边界（回归锚） | 边界 | `stdout` / `stderr` 双空 / 单字节 / `>64 KiB` 且签名在末尾 / 非 UTF-8（`0xFF 0xFE` + 签名）4 形态的既有 `is_transient` 断言全绿（P4 不动判定口径） | 表驱动（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：`RunReport` / `AttemptTrace` 以**结构体字面量直构**（被测 API 的显式入参），三新函数为纯 std 判定（零 IO / 零 spawn / 零时钟 / 零 async） | 全部用例 |
| 文件系统 / 子进程 / 环境变量 | 不涉及：本模块 std-only（design 组件表「std-only（零 tokio、零第三方、零 IO）」） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/exec/src/run.rs -> packages/desktop/src-tauri/crates/infra/exec/src/run_test.rs

<!-- design.md「公共函数 / API」子表一行（`run_shell`，签名不变、判定序加卡死早停臂，「所在文件」列为本文件）落本节。测试模块以 `#[cfg(test)] #[path = "run_test.rs"] mod run_test;` 挂载（既有形态）；`attempt_once` / `Race` / `await_exit` / `drain` / `wait_cancel` 与两常量零改，既有约 1800 行用例（三值结局 / 双管道 / `cwd` / `env` / pid 回收 / 取消）在 `counts` 全 0（无 `Compiling` 行）的 shim 下全绿——新臂不改变其行为。 -->

#### 待测功能

- `run_shell(command: SystemCommand, options: RunOptions) -> Result<RunReport, RunError>`（公共函数表，P1/D4，签名不变）：循环判定序改为「每次尝试后立即 `counts.push(compiling_count(..))` → ①非 `Completed` 即返 → ②未命中签名即返 → ③`is_progress_stalled(&counts)` 为真即返末次结果（`attempts < max_attempts`）→ ④上限 → ⑤预算 → ⑥退避（观测 `cancel`）→ ⑦入 `retried`」；卡死只提前放弃、不延长 `total_budget`；`counts` 为循环局部变量**不进** `RunReport`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run_shell` — 卡死早停（AC-1 组合锚） | 正向 | shim 每次都输出同一份 50 行 `Compiling <crate> v…` + 锁文本并 `exit 1`，`retry = transient()` → 第 2 次尝试后收敛：`attempts == 2 < 5`、`retried.len() == 1`（`exit_code == 1`、`transient == Some("os error 32")`）、末次 `Completed` + `exit_code == 1`、shim 计数 == 2（第 3 次零 spawn）、虚拟墙钟仅消费 `backoff[0]`（5 s） | 单值 |
| `run_shell` — 递减继续重试（AC-2 组合锚） | 正向 | shim 第 1 次 264 行、第 2 次 5 行、第 3 次 0 行 `Compiling` + 锁文本 `exit 1`，第 4 次 `exit 0` → 不早停：`attempts == 4`、`retried.len() == 3`、`exit_code == 0`、shim 计数 == 4 | 单值 |
| `run_shell` — 退避逐段（AC-3） | 正向 | 三次尝试均命中签名后成功；按段推进虚拟时钟断言 `backoff[0] == 5 s`（推进 5 s 前无第 2 次、满 5 s 即拉起）与 `backoff[1] == 10 s`（同法）；20 s / 40 s 段未被消费，全程虚拟墙钟 ∈ [15 s, 15.5 s]；断言取 `RetryPolicy::transient().backoff` 常量而非字面量（1 s / 2 s 硬编码作废） | 单值 |
| `run_shell` — 成功路径零额外等待（AC-3） | 正向 | 首跑即 `exit 0`（无签名）→ 虚拟时钟零推进（不进退避）、`attempts == 1`、`retried` 空 | 单值 |
| `run_shell` — 末值归零不早停（AC-2 抑制臂） | 异常 | 第 1 次 264 行 `Compiling` + 锁文本，此后每次都仅锁文本（计数 0）→ 序列 `[264, 0, 0, 0, 0]` 末值 `0` 不触发卡死 → 跑满上限 `attempts == 5`、`retried.len() == 4`（`fast_policy(5)` 以 20 ms 退避表达次数面） | 单值 |
| `run_shell` — 非 cargo 全 0 形态（既有改写锚） | 异常 | shim 恒锁文本（无 `Compiling` 行）→ 计数全 `0` 不触发早停 → `attempts == 5`（既有 `持续锁冲突耗尽_上限五次` 断言零变，注明「新臂在位但不触发」） | 单值 |
| `run_shell` — 上升计数判卡死（R-1 对照臂） | 异常 | 第 1 次 5 行、第 2 次 9 行 `Compiling` + 锁文本 → 判据为「非降」而非「相等」→ `attempts == 2`（第 3 次零 spawn） | 单值 |
| `run_shell` — 首跑不触发（AC-1 边界） | 边界 | 首跑 50 行 `Compiling` + 锁文本，`max_attempts = 1`（直构策略）→ `attempts == 1`、`retried` 空（无前值不判卡死） | 单值 |
| `run_shell` — 零重试面逐字节不变（AC-8） | 边界 | `TimedOut`（自旋 shim + 虚拟时钟推进）→ `attempts == 1` + `retried` 空；`Cancelled`（入口置位 / 运行中置位 / 退避期置位）→ `Cancelled` + `attempts` 记已完成次数 + `retried` 不含该末次；真红（`error[E0308]`）→ `attempts == 1`；`none()` 单次语义；单次等价回归（`outcome` / `exit_code` / 两路字节逐字节同改造前） | 表驱动（6 路径） |
| `run_shell` — `attempts` / `retried` 不变量 | 边界 | 全路径（绿 / 真红 / 超时 / 取消 / 卡死早停 / 上限 / 预算 / 退避取消）`attempts >= 1` 恒成立；成功、上限、预算、卡死四面 `retried.len() == attempts - 1`；退避取消面按 D4 口径逐面显式断言（不写模糊不等式） | 表驱动（8 路径） |
| `run_shell` — 卡死臂末次不入 `retried` | 边界 | 卡死早停（`attempts == 2`）时 `retried.len() == 1` 且 `retried[0]` 携第 1 次字节与签名原文；末次字节留在 `stdout` / `stderr`（既有不回滚语义） | 单值 |
| `run_shell` — 预算判定不被新臂干扰（AC-8） | 边界 | 既有 `预算耗尽早停不发起新尝试`（每次尝试虚拟 600 s、计数全 0）仍 `attempts == 4`、第 5 次零 spawn；`零预算首跑后即耗尽` 仍 `attempts == 1` 且虚拟时钟零推进（③ 位于 ⑤ 之前但全 0 序列判否） | 表驱动（2 形态） |
| `run_shell` — 大输出在早停面不丢字节 | 边界 | 每次尝试 `stdout` / `stderr` 各 `>64 KiB` 且含 1 行 `Compiling` + 签名 → 卡死早停后 `retried[0]` 字节数精确等于第 1 次写量、末次同样全量（排水语义与单次一致） | 单值 |
| 既有零改面（AC-8） | 边界 | `CANCEL_POLL_INTERVAL == 200 ms`、`FINISH_GRACE == 2 s` 锚定；双缓冲不合流 / 双管道并发不互锁 / `cwd` / `env` / pid 回收既有断言全绿；`RunReport` 字面量直构仅 6 字段（无 `counts` / 无序列字段，字段面零变） | 表驱动（2 常量 + 字段面） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（被测执行体自身） | 不 mock spawn：既有 POSIX shim 装置（`ShimWs` 落盘脚本 + `interpreter_command` 经 `resolve_shell` 探测解释器）真实拉起；**计数分支 shim**：按尝试计数文件决定本次输出 N 行 `Compiling` 与 / 或锁文本 / 成功（264 → 5 → 0 → 成功、50 → 50、5 → 9 等序列由 shim 自身表达），尝试次数亦由 shim 落盘（沿用既有 `attempt_count` / `retry_shim` 装置） | 全部 spawn 用例 |
| 时钟（退避 / 预算） | `#[tokio::test(start_paused = true)]` + `ClockGate`（抑制空闲自动推进）+ 手动 `tokio::time::advance` / `wait_into_backoff`（沿既有 `run_test` 先例）——5 / 10 / 20 / 40 s 退避与 1500 s 预算以虚拟时钟表达，不真等；取消臂沿用真实时钟（语义本就是轮询观测） | 退避 / 预算 / 卡死 / 超时用例 |
| 文件系统 | 不 mock：`tempfile::TempDir` 真实 workspace（shim 脚本 / 计数文件 / payload / pid 文件） | 全部 spawn 用例 |
| 重试策略 | 不 mock：`RetryPolicy` 结构体直构（`fast_policy` 压退避表达次数面；`transient()` / `none()` 表达取值面），字段 `pub` 为既有装置 | 次数 / 上限 / 预算用例 |

---

### packages/desktop/src-tauri/crates/infra/exec/src/lib.rs -> packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs

<!-- design.md 未声明该文件的公共 API 变更——「公共函数 / API」子表三新函数的「所在文件」列为 `transient.rs`（D15 仅追加 `pub use` 再导出项，非独立 API 行）。本节测试对象取自 design「修改文件」表 `lib.rs` 行（`pub use transient::{compiling_count, is_progress_stalled, progress_series, …}`）与 D15，不另行从源码推导导出声明。测试模块以 `#[cfg(test)] #[path = "lib_test.rs"] mod lib_test;` 挂载（既有形态）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`lib.rs` 行只是 `pub use` 再导出面随动，三新函数的签名行归属 `transient.rs`）。本节不列独立功能条目；用例表的测试对象按上条注释的 design 出处组织，均经 crate 根公共面断言（既有 `再导出*` 用例形态）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 再导出 `compiling_count` / `progress_series` / `is_progress_stalled`（修改文件行 `lib.rs`，D15） | 正向 | 经 `crate::compiling_count(b"   Compiling a\n", b"")` / `crate::progress_series(&report)` / `crate::is_progress_stalled(&[50, 50])` 调用（而非 `transient::` 私有路径）→ 与模块内结果逐值一致（导出的是同一函数，非包装） | 表驱动（3 项） |
| 再导出面退避四段（AC-3） | 正向 | 经 crate 根 `RetryPolicy::transient()` → `backoff == [5 s, 10 s, 20 s, 40 s]`、`max_attempts == 5`、`total_budget == 1500 s`（既有断言改写为慢档值） | 单值 |
| 再导出面边界输入一致性 | 异常 | 经 crate 根对退化输入（空序列 `&[]` / 双空字节 / `[0, 0]`）取值与模块内直调逐值一致（`false` / `0` / `false`），不 panic | 表驱动（3 形态） |
| 既有导出面零变（AC-8） | 边界 | `is_transient` / `AttemptTrace` / `lockprobe` 四项 / `ProcessOutput` / `kill_tree(None)` / `record_exec_output` → `read_exec_output` 家族既有断言全绿（`exec_output` 落点与内容形态零改） | 表驱动（4 项） |
| 导出面缺失即编译失败（编译期） | 边界 | `checks` 侧 `use exec::{progress_series, is_progress_stalled};`（environment.rs 的视图取数）解析成功；任一新项未再导出即编译失败——以「用例内 `crate::` 路径解析成功」表达 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（再导出面） | 零 mock：纯函数直调 + `RunReport` 字面量直构，零 spawn / 零 IO | 全部再导出用例 |
| 文件系统（记录面回归） | 不 mock：既有 `tempfile` 装置（`record_exec_output` / `read_exec_output` 真实落盘与回查） | 既有导出面零变用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/environment.rs -> packages/desktop/src-tauri/crates/infra/checks/src/environment_test.rs

<!-- design.md「公共函数 / API」子表一行（`fault_text`，签名扩第四参 + 新增「重试进展」行，「所在文件」列为本文件）落本节。私有 `progress_line`（D8 抑制条件）与「视图报告」构造（D7）经 `#[cfg(test)] #[path = "environment_test.rs"] mod environment_test;` 子模块可直驱，但断言只走 `fault_text` 公共（`pub(crate)`）面。`attempts_log` 零改动，其既有用例全部保留。 -->

#### 待测功能

- `fault_text(attempts: u32, exit_code: i32, output: &[u8], retried: &[AttemptTrace]) -> String`（公共函数表，P3/D6–D8）：文本顺序——归因段（占用者 / 被锁文件，取自 `exec::attribution_text`）→「已自动重试 N 次（每次重试会跳过已完成部分）」→ **可选「重试进展」行**（`重试进展：待编译 {c1} → … → {cN}`，序列经 `exec::progress_series` 视图取数，卡死时同一行追加 `（末两次无进展，判定为卡死）`）→「最后一次诊断：+摘要」→ `ENV_GUIDANCE`；`series.len() < 2`（未重试）或序列全 `0`（非 cargo 形态）时不输出该行；检查域零计数逻辑（MUST NOT 重算 `Compiling ` 口径）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `fault_text` — 推进序列行（AC-5） | 正向 | `attempts = 4`、`retried = [264 行 / 5 行 / 0 行 Compiling + 锁文本]`、末次 `output` 仅锁文本 → 文本含 `重试进展：待编译 264 → 5 → 0 → 0`；行位：在「已自动重试 3 次…」之后、「最后一次诊断：」之前（`find` 索引顺序断言）；不含卡死判定语 | 单值 |
| `fault_text` — 卡死判定附语（AC-5） | 正向 | `attempts = 2`、两次输出各 50 行 `Compiling` + 锁文本 → 含 `重试进展：待编译 50 → 50（末两次无进展，判定为卡死）` | 单值 |
| `fault_text` — 既有归因段保留（AC-5/AC-8） | 正向 | 既有三用例（重试次数与指引 / 占用者原料 / 末次诊断摘要）补第四参 `retried` 后断言零变：含「已自动重试 4 次」+ `ENV_GUIDANCE` 结尾 + 被锁文件路径 + （「占用进程：」或「未定位占用者」） | 表驱动（3 用例） |
| `fault_text` — `attempts == 1` 无该行（AC-5 抑制臂） | 异常 | `attempts = 1`、`retried = []`、`output` 含 264 行 `Compiling` + 锁文本 → 无「重试进展」字样；归因段 / 次数行（「已自动重试 0 次」）/ 摘要 / 指引照旧 | 单值 |
| `fault_text` — 序列全 `0` 无该行（AC-5 抑制臂） | 异常 | `attempts = 3`、`retried` 两条仅锁文本、末次仅锁文本（非 cargo 形态）→ 无「重试进展」；其余段零变 | 单值 |
| `fault_text` — 部分归零仍输出（D8 边界） | 边界 | 序列 `[264, 0]`（`attempts = 2`）→ 输出 `重试进展：待编译 264 → 0`（「确有推进可读」），且不含卡死判定语 | 单值 |
| `fault_text` — 耗尽面五数字（AC-5 上限） | 边界 | `attempts = 5`、序列 `[10, 8, 3, 1, 0]` → 行含 5 个数字、长度与 `attempts` 同阶；不含判定语（末值 0） | 单值 |
| `fault_text` — 直构违反不变量 | 边界 | `attempts = 3` 但 `retried = []`（直构语料）→ 视图序列长度 1 → 无「重试进展」、不 panic（抑制条件按序列而非 `attempts` 判） | 单值 |
| `fault_text` — 诊断与序列共存字节面 | 边界 | `output` 含 emoji / 引号 / 换行 / `\0` / 非 UTF-8（`0xFF 0xFE`）+ `Compiling` 行 → lossy 不 panic、序列行逐字在案、摘要按 `DIAGNOSIS_EXCERPT_LIMIT` 截断、`ENV_GUIDANCE` 结尾逐字；`attempts = 5` + `>1000` 字符诊断两形态 | 表驱动（3 形态） |
| `fault_text` — `attempts` 大值防御 | 边界 | `attempts = u32::MAX`（直构）→ 「已自动重试 4294967294 次」不溢出、不 panic、无空括号 | 单值 |
| 常量锚定（零改） | 边界 | `ENV_GUIDANCE` 含「父目录」/「逐 worktree」且与基线红文案零共用句；`DIAGNOSIS_EXCERPT_LIMIT > 0`（既有断言保留） | 表驱动（2 常量） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（文案装配面） | 零 mock：`AttemptTrace` / `RunReport` 以字面量直构（被测 API 的显式入参），`fault_text` 为纯函数（零 io / 零 async） | 全部用例 |
| `exec::lockprobe::attribution_text` / `ProcessOutput` | 不 mock：真实组合（归因原料与摘要口径正是被测语义，且 D7 要求序列经 `exec::progress_series` 单点取数——mock 掉即测不到口径单点） | 归因段 / 摘要 / 序列行用例 |
| 文件系统 | 不 mock：`tempfile::TempDir` 只为提供真实（且未被锁）的诊断路径，使探针零阻塞返回 | 占用者原料用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs

<!-- design.md 未声明该文件的公共 API 变更——`ProcessStaticCheck::run` 为 trait impl（签名零改），`execute` 为私有；改动仅为 `fault_text` 调用点补 `&run.retried`。本节测试对象取自 design「修改文件」表 `static_check.rs` 行（调用点补参，`2>&1` / `diagnostics` / `passed` 零变）与 AC-5 / AC-8。本节同时承载 AC-5 的组合锚（链路方向：`static_check::execute` → `exec::run_shell`（重试 + 卡死早停）→ `environment::fault_text` → `StaticCheckOutcome.fault`），按链路发起方挂靠本模块。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（trait 方法 `ProcessStaticCheck::run` 签名零改、`execute` 私有）。用例表的测试对象按上条注释的 design 出处组织。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 卡死推进序列到 fault（AC-5 组合锚） | 正向 | shim 按尝试计数分支：第 1 次 264 行、第 2 次 5 行、第 3 次 5 行 `Compiling` + 锁文本 + `exit 1` → 卡死早停 → `passed == false`、`fault == Some(Environment(text))`，`text` 含 `重试进展：待编译 264 → 5 → 5` + `（末两次无进展，判定为卡死）` + 「已自动重试 2 次」+ 被锁文件路径 + 指引；输出记录恰 3 段 attempt 分隔头；`diagnostics` 为末次原文（`trim_end`） | 单值 |
| 递减推进无判定语（AC-2 组合锚） | 正向 | shim 第 1 / 2 / 3 次分别 264 / 5 / 0 行 `Compiling` + 锁文本、第 4 次 `exit 1` → 序列 4 个数字、无判定语；`attempts` 由 4 段分隔头表达 | 单值 |
| 非 cargo 耗尽无序列行（AC-5 抑制臂） | 异常 | 既有 `瞬态耗尽_fault为环境错误且log五段`（恒锁文本、无 `Compiling` 行）补断言：`text` **不含**「重试进展」；其余断言（5 段分隔头 / 点名 / 被锁路径 / 指引 / 「已自动重试 4 次」）零变 | 单值 |
| 真红与无配置面（AC-8） | 异常 | 真红（无签名）→ `fault == None`、`attempts == 1`；无配置直过 → `passed == true`、`fault == None`、零 spawn；程序不可达 → `Err`（既有断言零变） | 表驱动（3 形态） |
| 超时 / 取消面逐字不变（AC-8） | 边界 | `TimedOut` → `Environment("static-check 命令超时（1200 秒）强制终止")`；`Cancelled` → `Stopped`；两者均不 `Err`、不产「重试进展」行 | 表驱动（2 形态） |
| 自愈转绿（既有回归锚） | 边界 | 首次锁文本 `exit 1`、第 2 次 `exit 0` → `fault == None`、`passed == true`、`.log` 两段（首段 `transient=os error 32`、末段 `transient=-`）；退避由 5 s 起步后仍在虚拟时钟推进范围内收敛 | 单值 |
| 交错流与诊断口径零变（AC-8） | 边界 | `2>&1` 仍恰一处、`diagnostics` 只读 `stdout` + `trim_end`、`passed == exit_code == 0`；输出记录内容形态（`attempts_log`）零改 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（static-check 命令） | 不 mock：既有 `TempWs` + `write_shim` 装置真实拉起（`cmd /C` 与 POSIX 双形态），推进行数与退出码由 shim 脚本表达；新增「按尝试计数分支」shim 以表达 264 / 5 / 5 序列 | 全部 spawn 用例 |
| 全局环境（PATH / 临时目录） | 不 mock：既有 `env_window()`（持 crate 级 `TEST_ENV_LOCK`）与 `PathPrefix` 串行化；用例内不新增进程全局写入 | 全部 spawn 用例 |
| 时钟（退避） | `#[tokio::test(start_paused = true)]` + `ClockGate` + 既有 `advance_until`（100 ms 步进）——5 / 10 / 20 s 退避以虚拟时钟表达；卡死臂仅消费一段（5 s） | 卡死 / 耗尽 / 自愈用例 |
| `exec::run_shell` / `environment::fault_text` | 不 mock：组合链正是被测语义（推进判据 → fault 文本），mock 即失去 AC-5 的组合证据 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs

<!-- design.md 未声明该文件的公共 API 变更——`execute_plan` 为私有、`PlanExecution` 字段面零改；改动仅为 `fault_text` 调用点补 `&run.retried`（合并流「stdout 前缀 + stderr 后缀」零变）。本节测试对象取自 design「修改文件」表 `runner.rs` 行与 AC-5 / AC-8；组合锚链路：`runner::execute_plan` → `exec::run_shell` → `environment::fault_text`（合并流）→ `PlanExecution.fault`。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`execute_plan` 私有、trait 方法签名零改）。用例表的测试对象按上条注释的 design 出处组织。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 合并流推进序列到 fault（AC-5 组合锚） | 正向 | suite 模板按尝试分支输出 N 行 `Compiling`（stdout）+ 锁文本（stderr 侧），多 plan 装置直驱 `execute_plan` → `fault == Some(Environment(text))`，`text` 含 `重试进展：待编译 c1 → … → cN`（末次 `stderr` 已并入 `output`，不得重复计数）；`output` 分隔头段数与 `attempts` 一致 | 单值 |
| 卡死判定语（AC-5） | 正向 | 两次尝试同为 50 行 `Compiling` + 锁文本 → `text` 含 `（末两次无进展，判定为卡死）`；`execution.output` 恰 2 段 | 单值 |
| 非 cargo 耗尽无序列行（AC-5 抑制臂） | 异常 | 既有 `瞬态耗尽_fault为环境错误且零execution_error`（恒锁文本）补断言：`text` 不含「重试进展」；既有断言（`error == None`、`exit_code == 1`、5 段、工件解析跳过）零变 | 单值 |
| 合并流顺序零变（AC-8） | 异常 | `stdout` 前缀 + `stderr` 后缀的既有合并流顺序断言、`error` 面派生、`source_files` / `cases` 解析面全部零变（P3 只多传一个参数） | 表驱动（3 形态） |
| 超时 / 取消面逐字不变（AC-8） | 边界 | `TimedOut` → `Environment("测试命令超时（1200 秒）强制终止")`、`exit_code == -1` 哨兵、pid 消失；`Cancelled` → `Stopped` 且零报告写入 | 表驱动（2 形态） |
| `PlanExecution` 字段面零变（AC-8） | 边界 | `exit_code` / `duration_ms` / `cases` / `source_files` / `measured` / `error` / `output` / `fault` 八字段既有断言零变（字段集无新增） | 单值 |
| 自愈面（既有回归锚） | 边界 | 前 1 次锁文本、第 2 次 `exit 0` → `fault == None`、`error == None`；`output` 两段（首段 `transient=os error 32`、末段 `transient=-`）；退避 5 s 虚拟推进后仍收敛 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（suite 命令） | 不 mock：既有 `TempDir` + 平台双形态 shim（`.cmd` / `.sh`）真实拉起；推进行数与锁文本由模板串表达 | 全部 spawn 用例 |
| 全局环境（PATH） | 不 mock：既有 `env_window()`（crate 级 `TEST_ENV_LOCK`）串行化 | 全部 spawn 用例 |
| 时钟（退避） | `#[tokio::test(start_paused = true)]` + `ClockGate` + 既有 `advance_until`——退避以虚拟时钟表达，不真等 5 / 10 / 20 / 40 s | 卡死 / 耗尽 / 自愈用例 |
| `exec::run_shell` / `environment::fault_text` / `attempts_log` | 不 mock：组合链与被测口径正是语义本体 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更——`execute` 为私有（新增第五参 `total_budget: Duration`，D9/D10），`ProcessTestExecution::run`（trait impl）签名零改（仅内部调用点补常量实参）；`TESTEXEC_TOTAL_BUDGET` 为私有代码常量。本节测试对象取自 design「修改文件」表 `mod.rs` 行与「配置」表 `TESTEXEC_TOTAL_BUDGET` 行、AC-6。组合锚链路：`ProcessTestExecution::run` → `execute(.., budget)` → 逐 plan `execute_plan`（零 spawn 判定）→ `fault_outcome`（不写 summary / 不进复用门 / `report_dir` + `exec_id` 回填）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`execute` 私有、trait 方法签名零改、常量不在「公共函数 / API」子表）。用例表的测试对象按上条注释的 design 出处组织；预算经私有 `execute(.., total_budget)` 第五参注入（D10：生产唯一调用方传常量，不构成配置面）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `TESTEXEC_TOTAL_BUDGET` 常量锚定（AC-6） | 正向 | 模块内断言 `TESTEXEC_TOTAL_BUDGET == Duration::from_secs(2400)`（改值即被击杀）；生产唯一调用方 `ProcessTestExecution::run` 传该常量（经 `execute(.., TESTEXEC_TOTAL_BUDGET)` 全链绿跑表达） | 单值 |
| 预算内零行为变化（AC-6 正向） | 正向 | `execute(.., TESTEXEC_TOTAL_BUDGET)` 绿跑全链（既有一 / 两 plan 装置）→ `conclusion == Pass`、四计数、summary.json 与 planId/report.json 落盘、`fault == None`（与既有 `绿跑全链` 断言逐字一致） | 表驱动（1 / 2 / 3 plan） |
| 超预算余下 plan 零 spawn（AC-6 组合锚） | 正向 | 两 plan 装置 + 1 ms 级预算（量级选取使「首 plan 边界不触发」与「第二边界必越」确定成立：首个 plan 的真实 spawn 耗时 ≫ 预算，非时序抖动依赖）→ marker 仅 1 行（余下 plan **零 spawn**）、`fault == Some(Environment(text))` 且 `text` 含「测试执行总时长超预算」与「余下 1 个 suite 未执行」、`conclusion == Fail` 占位、四计数 0、`findings_brief` / `findings_detail` 为空串、`report_dir` / `exec_id` 回填、`reports/test` 无 `summary.json` | 单值 |
| 零预算边界 | 边界 | `execute(.., Duration::ZERO)` → 首个 plan 边界即触发（注入面边界形态；生产常量下 `elapsed ≈ 0 ≪ 2400 s` 恒不触发）：全部 plan 零 spawn（marker 0 行）、`text` 含「余下 N 个 suite 未执行」且 `N == plans.len()`、无 summary、`exec_id` 仍回填（空合并流落空 `.log` 可回查） | 单值 |
| 预算不覆盖既有 `Stopped`（AC-6 异常臂） | 异常 | 预置 cancel + 毫秒级预算 → `fault == Some(Stopped)`（不是超预算文案）；余下 plan 零 spawn（Stopped 面既有 break 先于预算判定） | 单值 |
| 预算不覆盖既有 `Environment`（AC-6 异常臂） | 异常 | 三 plan 混合装置（中间 jest 档瞬态耗尽 → `Environment(锁文案)`，`fault.is_none()` 守卫使其不被覆盖）+ 毫秒级预算 → 最终 `fault` 仍为首个 `Environment`（文案含锁文本、**不含**「超预算」）；进入后续 plan 边界时零 spawn | 单值 |
| 复用门不进（AC-6 连带 + AC-8） | 边界 | 预算收口后 `reports/test` 无 summary → 同一输入再跑（全预算）→ 不复用（marker 增长，重新 spawn）并写 summary；既有复用门用例（输入未变零 spawn 复用 / 输入失效重跑 / 跨树零复用）零变 | 表驱动（3 形态） |
| 已完成 plan 工件可回查（R-5） | 边界 | 预算收口面：首个 plan 的 `results.txt` / `report.json` 仍在报告树内可读（工件不删），仅 summary 缺失；`exec-outputs` 记录含已完成 plan 的合并流 | 单值 |
| 槽位与记录面零变（AC-8） | 边界 | `run_slot(baseline / test)` 分树断言、`report_dir` 与 `exec_id` 同锚口径、结论三值映射、`findings_*` 装配口径既有断言零变 | 表驱动（2 槽位） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（suite 命令） | 不 mock：既有 `ChainWs` 装置（真实 `openspec/config.json` + `node` / `npx` shim 经 PATH 解析）真实 spawn；spawn 次数以 `SHIM_MARKER` 行数观测（「零 spawn」即 marker 不增长） | 全部执行面用例 |
| 全局环境（PATH / `SHIM_*`） | 不 mock：既有 `PathWindowGuard`（持 crate 级 `TEST_ENV_LOCK`，窗口收口恢复 PATH 并清 `SHIM_*`）串行化进程全局变量边界 | 全部执行面用例 |
| 跨 plan 预算 | **参数注入**而非 mock：私有 `execute(.., total_budget)` 第五参注入毫秒级 / `ZERO` / 生产常量三档（D10 的可测性落点；生产零配置面） | 预算全部用例 |
| 时钟 | 不 mock `std::time::Instant`（预算判定按真实墙钟）：以「毫秒级预算 + 真实耗时越过」表达超预算，以「生产常量 + 常规耗时」表达预算内；需虚拟退避的瞬态耗尽臂沿 `ClockGate` 先例（既有 `static_check_test` / `runner_test` 装置） | 超预算 / 不覆盖 `Environment` 用例 |
| 文件系统 | 不 mock：`tempfile::TempDir` 真实 workspace 与报告树（summary 缺失 / 工件可回查均为真读盘断言） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs -> packages/desktop/src-tauri/crates/core/checks/src/aggregate_test.rs

<!-- design.md「公共函数 / API」子表两行（`aggregate_coverage`、`build_summary_report`，「所在文件」列为本文件）落本节。私有 `misses_coverage`（D11 单点判据）/ `collect_problems`（D12 追加与去重）经 `#[cfg(test)] mod aggregate_test;` 子模块可直驱，但断言只走两公共函数面。既有语料改写：`汇总组装逐字段相等` 拆为「全测齐全零变」与「缺测归因」两臂（D14 / design 待决问题 1）。 -->

#### 待测功能

- `aggregate_coverage(sub_reports: &[SubReport], suites: &[TestSuite]) -> Option<CoverageBlock>`（公共函数表，P6/D13，签名不变）：任一子报告「非 pass（`exit_code != 0` / `summary.failed > 0` / `coverage.pass == false`）且 `coverage == None`」在场 → `None`（MUST NOT 为 `pass: true`）；加权算法 / 阈值 / per-suite override 口径零改
- `build_summary_report(sub_reports: &[SubReport], suites: &[TestSuite], command: &str, started_at: OffsetDateTime) -> SummaryReport`（公共函数表，P6/D12，签名不变）：经 `collect_problems` 为每个缺测 suite 追加一条 `ProblemType::ExecutionError`（`framework` = 该 suite 框架，消息含「覆盖率采集缺失」与退出码；每 suite 至多一条，与该 suite 既有 `execution_error` 去重、既有条目消息零改写）；`determine_conclusion` 映射零改（`has_execution_error → Error`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `build_summary_report` — 全测齐全零变（AC-7 分支 a，改写既有语料的「齐全」臂） | 正向 | 两 suite 均产覆盖率（rust 92% / vite 96%，均达阈）→ `coverage == Some(pass: true)`、`measured` 加权逐值、`problems` 逐字节与既有相同（无新增 `execution_error`）、`conclusion == Pass`；计数合并 / plans 索引 / `phase` / `command` 透传 / mutation 恒 `None` 既有断言零变 | 单值 |
| `build_summary_report` — 缺测归因（AC-7 组合锚，改写既有语料的「缺测」臂） | 正向 | rust suite `exit_code = 1` + `coverage = None`、vite-plus `96.21%` 达标 → `conclusion == Error`、`coverage == None`（**不为** `Some(pass: true)`）、`problems` 含 1 条 `ExecutionError`（`framework == "rust"`、消息含「覆盖率采集缺失」与退出码 1）；`problems` 顺序按 suite 迭代序 | 单值 |
| `aggregate_coverage` — 采集完整性收敛 | 正向 | 混合两报告（一缺测、一有覆盖率且达标）→ `None`（既有实现会报 `Some(pass: true)`，本用例即击杀点）；两报告均有覆盖率 → `Some` 且加权 / override / 阈值逐值同改造前（既有 `多子报告全局聚合与per_suite_override组` 保留） | 表驱动（2 形态） |
| `build_summary_report` — 去重不追加第二条（AC-7 异常臂） | 异常 | suite `exit_code = 1` + `failed == 0` + `coverage == None`（既有 `execution_error` 臂命中）→ 该 suite `execution_error` **恰 1 条**、消息仍为既有形态（`Exit code 1` 或既有 findings 首条，零改写） | 单值 |
| `build_summary_report` — 失败用例 + 缺测双归因（D14 形态） | 异常 | suite `exit_code = 0` + `failed = 2` + `coverage = None` → `problems` 含 2 条 `TestFailure` + 1 条 `ExecutionError`（覆盖率缺失）、`conclusion == Error`（由 `Fail` 升格，留痕断言）、`coverage == None` | 单值 |
| `build_summary_report` — 全体缺覆盖率且全绿 | 异常 | 全部子报告 `exit 0` / `failed 0` / `coverage == None`（均非 pass 判否）→ 无新增 problem、`coverage == None`（既有口径）、`conclusion == Pass`（口径不变） | 单值 |
| `build_summary_report` — 单子报告与有覆盖率未达阈 | 边界 | 单子报告 `coverage == None` → `aggregate_coverage` 恒 `None`（既有短路，不属「缺测」新增面）；单子报告 `coverage == Some(pass: false)` + `exit_code != 0` → 不判缺测（有覆盖率）、`coverage` 保留 `Some(pass: false)`、既有 `CoverageFailure` problem 断言零变 | 表驱动（2 形态） |
| `build_summary_report` — 超大列表 | 边界 | 50 条 sub_reports 中恰 1 条缺测 → `coverage == None`、新增 `execution_error` 恰 1 条、其余 49 条问题面零改 | 单值 |
| 判据边界值 | 边界 | `exit_code = -1`（哨兵）+ `coverage == None` → 计入缺测（非零）；`failed = u64::MAX` + `coverage == None` → 计入且计数不溢出；`exit_code = 255` + `coverage == Some(pass: true)` → 不计入（有覆盖率） | 表驱动（3 形态） |
| `build_summary_report` — 空集与序列化面 | 边界 | 零子报告 → 空汇总（四计数 0、problems 空、`coverage` / `mutation` 为 `None`、`conclusion == Pass`）不 panic；缺测归因面 summary 可序列化且 `coverage: null`、`problems[].type == "execution_error"`（落盘面消费形态） | 表驱动（2 形态） |
| 既有语料 golden 零漂移（回归锚） | 边界 | `crates/core/checks/tests/corpus_golden_test.rs` 8 套语料（green-* / red-run / mutation-block / unknown-fields / corrupt-*）经 `aggregate_coverage` 重算的投影与 golden 逐字节一致（语料中无「非 pass 且 `coverage == None`」形态，故 P6 不引起漂移） | 遍历枚举（8 套语料） |
| 组合链路 aggregate → reuse（入口 = 本模块） | 边界 | 缺测归因产出的 summary 经 `checks::is_reusable(&summary, None)` → `false`（`conclusion == Error` 的既有判定，零代码改动）：半可信报告不被复用；全测齐全的 summary → `true`（既有口径） | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯层） | 零 mock：`SubReport` / `TestSuite` / `CoverageBlock` / `CaseSummary` 由 fixtures 构造器（`sub_report` / `suite` / `block` / `entry` / `counts`）直构，纯函数零 spawn / 零 IO | 全部用例 |
| 时间 | 不 mock：`OffsetDateTime::now_utc()` / `UNIX_EPOCH` 直传（既有装置），断言不锚墙钟（`duration_seconds` 只断存在性） | `build_summary_report` 用例 |
| 文件系统 | 仅 golden 回归锚读入仓 fixtures（既有 `corpus_golden_test` 装置），生产逻辑零 IO | golden 零漂移用例 |

---

## 不可测试项

| 项 | 原因 | 承接方式 |
|---|---|---|
| 1. `packages/desktop/package.json` 版本 `0.5.28` → `0.5.29`（D16） | 数据面（JSON 单字段），无运行时测试路径 | 版本 diff 复核（AC-10）；`tauri.conf.json` 经 `../package.json` 跟随的静态确认 |
| 2. AC-9 硬验收（冷 target 自愈 + 终局文案可读） | 需真实 worktree + 现场安全软件环境；CI 不可断言（design 待决问题 6） | 交付后人工跑一次 `pnpm run check` 并读终局文案；文案原料面由 AC-5 的三个 `*_test` 覆盖 |
| 3. AC-10 门禁全绿与覆盖率阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90） | 门禁统计面（由桌面端静态脚本 / test-execution 门禁执行，agent 内不重复执行） | `packages/desktop` 门禁（`pnpm run check` + 覆盖率报告）；三处新纯函数按 R-9 先全覆盖（本文件 `transient.rs` 节）再计入 |
| 4. `bindings:check` 零 diff / 零新 IPC / 零前端载荷 | 生成面与编译期属性（`RunFault` / 三 outcome 均不出线） | `pnpm -C packages/desktop run bindings:check`（零 diff）+ `git diff --stat packages/desktop/src`（零条目） |
| 5. `plugins/**` 零改动零 bump | 数据 / 目录面 | `git diff --stat plugins/`（零条目）+ 版本号 grep 复核 |
| 6. `openspec/config.json` 零 schema 扩展（`RETRY_BACKOFF` / `total_budget` / `TESTEXEC_TOTAL_BUDGET` 均为代码常量） | 数据面；参数化仅为 `execute` 私有第五参（D10） | `git diff openspec/config.json`（零条目）+ 常量锚定（`transient_test` 三常量 / `mod_test` 的 `TESTEXEC_TOTAL_BUDGET`） |
| 7. 「计数口径单点」结构面（检查域 MUST NOT 重算 `Compiling ` 行计数） | 源码结构面，非运行时（即使重算也难被行为断言区分） | 源码 grep：`grep -n "Compiling " crates/infra/checks/src`（零命中）+ `grep -rn "compiling_count" crates --include=*.rs` 的命中行排除 `_test` 后只剩 `transient.rs`（定义 + `progress_series` 内）与 `run.rs`（导入 + push） |
| 8. 「`execute` 第五参仅可测性用途、生产唯一调用方传常量」 | 结构面 | `grep -rn "execute(" crates/infra/checks/src/testexec`（生产恰一处调用：`ProcessTestExecution::run`）+ `grep -n "TESTEXEC_TOTAL_BUDGET"`（恰 2 处：定义 + 实参） |
| 9. `fault_text` 视图报告构造（D7）的「零拷贝优先」与 `progress_series_of` 备选（design 待决问题 2） | 实现形态选择，行为面等价（两方案 `compiling_count` 口径相同） | 行为面由 `environment_test` 序列行用例钉住；形态选择留评审（design 待决问题 2） |
| 10. 模块 doc / doc 注释改写（`transient.rs` 纪律条、`run.rs` 判定序、`environment.rs` 职责段、`mod.rs` 预算段） | 文档文本，无运行期断言 | 源码审查（与 design「修改文件」表逐条对读） |
| 11. `RunReport.attempts == 0` 与 `run_shell` 的 `RunError::Wait` 臂 | 类型不变量外状态 / `child.wait()` io 失败不可稳定构造（既有 `run_test` 同口径） | 全路径 `attempts >= 1` 断言（`run_test`）+ `RunError` 两变体 `Display` 直构用例（既有） |
| 12. 非 Windows 平台臂（`cmd` shim / `tasklist` / `powershell` 装置） | 当前门禁平台为 Windows；跨平台行为不可在同一次运行内断言 | `#[cfg]` 双形态 shim 已按其平台保留；unix 臂由 CI 平台补齐时启用 |
| 13. 退避上调后的真实墙钟（最坏多 75 s，R-2） | 时间性能面，测试内以虚拟时钟 / `fast_policy` 表达，不测真实等待 | `transient_test` 常量锚定 + `run_test` 虚拟时钟逐段断言；线上体感由 AC-9 人工验收观察 |
| 14. `is_progress_stalled` 的 `coverage.pass == false` 与 `coverage == None` 合取不可达 | 逻辑面：`coverage.pass == false` 要求 `coverage` 为 `Some`，与「`coverage == None`」互斥（该析取项在合取中恒不可满足，属防御性写法） | 由 `aggregate_test` 的「有覆盖率未达阈」边界臂覆盖实际可达面（有覆盖率 → 不判缺测）；spec 三条件的可达子集 = `exit_code != 0` / `failed > 0` 两支 |
