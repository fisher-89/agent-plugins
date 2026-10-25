# 提案: static-check-file-lock

> **变更**: static-check-file-lock
> **日期**: 2026-10-23
> **状态**: draft
> **来源**: `explore.md`（requirement 类型，2026-10-19 现场取证：日志 / 指纹 / 进程监视 / S1 实测 / 代码核实；本篇只承接其中**尚未实现**的补丁项）
> **前置依赖**: `rust-suite-single-build` / `exec-run-shell` / `lock-fault-diagnostics` 三篇已归档合入——单构建模板、`exec::run_shell` 四能力执行体、瞬态重试、锁探针归因、`RunFault` 分派均在位；本篇**只在其上补重试质量与覆盖率口径的六处缺口**，不改任何 runner port 签名、不改 fault 语义

---

## 问题

### 现象：失败率 ∝ 待写产物数 N，重试是确定性收敛而非碰运气

现场取证（`archive/2026-10-19-test-execution-zero-agent-decision/reports/baseline/exec-outputs/`）连续 attempt 的 `Compiling` 行数：

| attempt | `Compiling` 行数 | 结果 |
|---|---|---|
| 0 | **264** | 失败 `failed to write …libpotential_utf-….rmeta` |
| 1 | **5** | 失败 |
| 2 | **0** | 失败 `failed to build archive …`（只剩 rename 收尾） |
| 3 | **0** | 失败 |
| 4 | **0** | 失败（`could not compile axum`） |
| 5 | **192** | 失败（新一轮：static-check 的 `cargo run` 又写 `target/debug`） |
| 8 | **0** | ✅ 成功 |

机制：rustc 每写一个 `.rlib` / `.rmeta` 就是一次「写临时文件 → rename」，每次都有概率 `p` 被企业安全软件的**内核 minifilter** 拦下（`os error 32`）；一次构建写 N 个产物，碰撞概率 `≈ 1-(1-p)^N`。关键性质是**失败不回滚已完成产物**——attempt 0 大面积失败但前 200+ 个已落盘，后续 attempt 只剩尾部少数，于是 `N` 单调递减至 0，重试是「分批填满 target」的确定性收敛。

根因（三篇已覆盖，此处仅备查）：`HipsDaemon.exe` / `usysdiag.exe`（火绒企业版，服务端策略托管）、`TBProtect.exe` + `tbprotect64.sys`（云枢 YunShu）、`System [pid 4]`（上述驱动内核态持有）；`rename` / `write` / `remove` 三个动词全部沦陷，共同点是需要 **DELETE 共享语义**。环境侧排除（IT 行动项）才是压低 `p` 的根治手段，代码侧只能压低 `N` 的影响面。

### 现状代码的六处缺口（本次补丁）

| # | 缺口 | 代码位置 | 后果 |
|---|---|---|---|
| P1 | 只有「尝试上限 / 总预算」两个早停条件，**不区分「还在推进」与「同一批写不进去」** | `crates/infra/exec/src/run.rs`（重试循环的 `attempts >= max_attempts \|\| elapsed >= total_budget` 分支） | 卡死形态烧满 5 次（实测两种形态都存在：`264 → 5 → 0` 在推进；`50 → 50` 卡死） |
| P2 | 退避为秒级（`[1, 2, 4, 8] s`） | `crates/infra/exec/src/transient.rs`（`RETRY_BACKOFF`） | 实测静默窗口可达分钟级；「只剩 rename 收尾」（N=0）的尝试大概率撞同一扫描窗口——而那正是最需要耐心的一档 |
| P3 | 环境文案缺「推进序列」，重试事实只有一句次数 | `crates/infra/checks/src/environment.rs`（`fault_text`，签名只有 `attempts` / `exit_code` / `output`） | 「重试确实在推进」不可见，P1 的判据也无法向用户自证 |
| P4 | 签名 `failed to remove file` **永不命中** | `crates/infra/exec/src/transient.rs`（`TRANSIENT_SIGNATURES`） | 生产文本为 `failed to remove C:\…\xxx.rcgu.o`（不含 `file`）；功能上被同行 `os error 32` 兜住，但会误导维护者以为 remove 形态有独立覆盖 |
| P5 | test-execution 逐 plan 各自 1500 s 重试预算，**无跨 plan 总闸** | `crates/infra/checks/src/testexec/mod.rs`（`execute()` 逐 plan 调 `execute_plan`） | 最坏墙钟 ≈ 1575 s 且无上界；suite 数量增长后一个 run 可能挂 40 分钟 |
| P6 | 覆盖率聚合「假通过」 | `crates/core/checks/src/aggregate.rs`（`weighted_measured` 的 `has_any` 短路） | 历史实证（`archive/2026-10-18-desktop-agent-chat-component/reports/baseline/summary.json`）：`conclusion=error` 而 `coverage.pass=true`，数值全来自 vite-plus——rust suite 一个覆盖率数都没产出，聚合面却报「覆盖率通过」 |

门禁结论原本靠 `problems` 兜住（P6 不会放过假绿），但「覆盖率通过」这句话是假的：用户与决策 agent 都会据此误判。

### 探索项未决（本次不入实现）

P7「worktree 根位置」：失败行 21 条全在 `C:\`，主仓 `D:\Projects` 0 条，但两个假设未分离——H1（`C:\Users` 被安全软件扫描更激进，N 相同而 p 更大）与 H2（worktree 每次新建 → target 冷 → N 大，与盘符无关）。分离实验（`move target` + `mklink /J` 到 D 盘，5 分钟、不改代码）未做，故 `src-tauri/src/main.rs` 的 `DATA_DIR` 本次**不动**；若 H1 成立，改一个常量即可根治（不依赖 IT），优先级高于本篇全部补丁——见「风险 R-8」与「遗留观察项」。

## 提案

六处补丁，均在既有落位内追加，无新模块、无新依赖、无新 IPC：

1. **P1 增量推进判据（`exec::transient` + `exec::run`）**：新增「待编译计数」单点 `compiling_count(stdout, stderr) -> u32`（去前导空白后以 `Compiling ` 为前缀的行数，大小写敏感；无该字样 → `0`，非 cargo 构建形态退化为固定次数语义）、序列面 `progress_series(&RunReport) -> Vec<u32>`、判据 `is_progress_stalled(&[u32]) -> bool`（末两次非降且末值 `> 0`）。重试循环在**命中签名之后**按判据早停：`N_i == N_{i-1} > 0` → 立即返回末次结果（`attempts < max_attempts`）；`N_i < N_{i-1}` → 继续；`N_i == 0` → 不早停（「只剩收尾」最该再试）。`attempts == 1` 无前值不触发。判据只用于**提前放弃**，不延长预算。

2. **P2 退避上调（`transient.rs` 一个常量）**：`RETRY_BACKOFF` 由 `[1, 2, 4, 8] s` 改为 `[5, 10, 20, 40] s`。成功路径零影响（不命中签名不重试）；只有真需要重试时多等（最坏 5 次共 75 s），`total_budget = 1500 s` 不变。

3. **P3 诊断文案补「推进序列」（`environment.rs` 单点）**：`fault_text` 签名扩为「`attempts` + `exit_code` + 末次输出 + `retried: &[AttemptTrace]`」，在归因段之后输出一行 `重试进展：待编译 264 → 5 → 0 → 0`；`attempts == 1` 或序列全 `0` 时不输出该行；`is_progress_stalled` 为真时附「末两次无进展，判定为卡死」。计数口径**只在 `exec` 一处**（检查域 MUST NOT 重算）。两个调用点（`static_check.rs` / `testexec/runner.rs`）只多传一个参数。

4. **P4 签名修正（`transient.rs` 一个字符串 + 表驱动用例）**：`failed to remove file` → `failed to remove`（该短语只出现在删除失败语境，收窄风险低）；`transient_test` 补一条「`failed to remove <path>: …` 命中」用例（现有用例只覆盖与生产文本不符的 `failed to remove file`）。

5. **P5 跨 plan 总预算（`testexec/mod.rs` 一个常量 + 判定）**：`TESTEXEC_TOTAL_BUDGET = 2400 s`（代码常量，不进 config schema）；`execute()` 记录起点、**进入每个 plan 之前**判定一次，超预算则余下 plan **不 spawn**，整体以 `fault = Some(Environment("测试执行总时长超预算（2400 秒）…"))` 收口。收口**沿既有 `fault_outcome` 路径**（不写 summary、不跑诊断、不进复用门、`report_dir` / `exec_id` 照旧回填），不新增第三条路径；既有 fault 聚合优先级不变（`Stopped` 优先，预算不覆盖已有 fault）。

6. **P6 覆盖率采集完整性（`core/checks::aggregate`）**：汇总组装检出「子报告自身结论非 pass（`exit_code != 0` / `failed > 0` / `coverage.pass == false`）且 `coverage == None`」→ 为该 suite 追加一条 `ProblemType::ExecutionError`（每 suite 至多一条，与该 suite 已有 `execution_error` **去重**），`determine_conclusion` 既有映射（`has_execution_error → Error`）零改动；同时聚合 `coverage` 块**不得**为 `pass: true`——采集不完整即收敛为 `None`（与「无实测数据 → None」同源的不可测语义）。加权算法 / 阈值 / override 口径零变。

### 范围裁决（explore 未决项与实现口径）

- **判据口径归属**：计数与卡死判据单点驻 `exec::transient`（纯函数），`run.rs` 与 `environment.rs` 各只调一次——MUST NOT 在 `checks` 侧重算计数（否则两处口径漂移，P1 的判据与该判据的自证材料会对不上）。
- **卡死早停的边界**：只在「命中签名」且已有前值且末值 `> 0` 且非降时触发；`TimedOut` / `Cancelled` / 真红（无签名）三条既有零重试面**行为逐字节不变**。
- **P5 不保留部分结果**：explore 的「不要把已完成的 plan 结果丢掉」按「沿既有 `fault_outcome` 路径」落地——该路径本就不写 summary、不进复用门（半套报告会被 `check_integrity` 当权威而永久复用）；已完成 plan 的工件（`results.txt` / coverage）仍在报告树内可人工回查，输出经 record 单点保留。
- **P6 不扩 P7**：不引入「按框架声明覆盖率为可选」的配置面——本变更只把「缺测却报通过」改成「不可测 + 显式 ExecutionError 归因」。
- **预算参数零 schema**：`RETRY_BACKOFF` / `total_budget` / `TESTEXEC_TOTAL_BUDGET` 与既有超时预算同性质，均为代码常量（观察期后再议用户可配）。
- **fault 文案长度**：推进序列行长度为 `O(attempts)`（至多 5 个数字），只进 `TXT_ENV_STATIC` / `TXT_ENV_TEST` 终局文案，不占 `findings_detail` 的 2000 字符明细预算。

## 能力

### 新增的能力

无。六处补丁全落既有能力边界：退出面与重试质量在 `crates/infra/exec`（**desktop-exec-runtime**），检查域装配与聚合判定在 `crates/infra/checks` + `crates/core/checks`（**desktop-checks-domain**）。另立能力会把「执行体契约」「诊断装配」与「聚合判定」拆成总是一起改的三块。

### 修改的能力

- **desktop-exec-runtime** — **MODIFIED**「瞬态签名字典与有限重试策略」：签名表收窄 remove 措辞（P4）、退避 `[5, 10, 20, 40] s`（P2）、新增增量推进判据与卡死早停（P1，含 `compiling_count` / `progress_series` / `is_progress_stalled` 三函数契约）。「统一执行体 run_shell 契约」「超时必填与超时预算表」「进程树击杀」「取消原语」「输出解读面」「锁占用精确探针」六条 requirement **零改动**（`RunOptions` / `RunReport` / `RunOutcome` / `RunError` 字段面与探针面不动）。
- **desktop-checks-domain** — **MODIFIED**「瞬态重试与环境错误归因（fault 装配）」（fault 文案补推进序列、`fault_text` 收 `retried`）；**ADDED**「test-execution 跨 plan 总预算」（P5）；**ADDED**「覆盖率采集完整性（缺测不假通过）」（P6）。

其余能力零改动，理由：

- **desktop-test-execution**：本轮不改结论判定函数、不扩 summary / 子报告 schema、不动复用门与报告落点——P5 走既有 fault 收口（该能力已写「环境面由 fault 承载、不写 summary」），P6 只改聚合判定分支（该能力的 requirement 已声明「缺覆盖率按 null 感知跳过，不产生假绿」，P6 是把这句的口径钉死）。故其 spec 零改动。
- **desktop-change-orchestration**：`RunFault` 载荷、walker 五处分派、`TXT_ENV_*` 文案骨架均不变（fault 文本只是多了内容行）。
- **desktop-mutation-testing**：突变三段不接 retry，`is_transient` / 退避 / 早停均不作用于它们。
- **desktop-checks-domain 的其余 requirement**（边界分流 / 纯层零进程 / 依赖规则 / 输出记录 / baseline 槽位 / 名单共享 / spawn 统一）：零改动。
- **前端 / bindings / 插件 / config schema**：零新载荷、零新 IPC 命令、零 `plugins/**`、零 schema 扩展。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/infra/exec/src/transient.rs` | 签名表 `failed to remove file` → `failed to remove`；`RETRY_BACKOFF` → `[5, 10, 20, 40] s`；新增 `compiling_count(stdout, stderr) -> u32`、`progress_series(&RunReport) -> Vec<u32>`、`is_progress_stalled(&[u32]) -> bool`（纯函数、零 IO） |
| `crates/infra/exec/src/run.rs` | 重试循环维护按尝试序的计数序列，命中签名之后按 `is_progress_stalled` 判卡死并返回末次结果（`attempts < max_attempts`）；`RunOptions` / `RunReport` / `RunOutcome` / `RunError` 零字段变更 |
| `crates/infra/exec/src/lib.rs` | 导出面补 `compiling_count` / `progress_series` / `is_progress_stalled`；`kill_tree` / `ProcessOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` 语义零变 |
| `crates/infra/checks/src/environment.rs` | `fault_text(attempts, exit_code, output, retried: &[AttemptTrace])`：归因段之后追加「重试进展：待编译 c1 → … → cN」行（`attempts == 1` / 全 `0` 不出；卡死判定时附判定语）；`attempts_log` 零改动 |
| `crates/infra/checks/src/static_check.rs` | `fault_text` 调用补 `&run.retried`（`2>&1` 交错流、`diagnostics`、`passed` 口径零变） |
| `crates/infra/checks/src/testexec/runner.rs` | `fault_text` 调用补 `&run.retried`（合并流「stdout 前缀 + stderr 后缀」零变） |
| `crates/infra/checks/src/testexec/mod.rs` | 新增 `TESTEXEC_TOTAL_BUDGET = 2400 s`；`execute()` 记录起点、进入每 plan 前判定、超预算置 `Environment` fault 并走既有 `fault_outcome` |
| `crates/core/checks/src/aggregate.rs` | 覆盖率采集完整性：检出「非 pass 且 `coverage == None`」子报告 → 追加去重后的 `ProblemType::ExecutionError`；采集不完整时 `aggregate_coverage` 收敛 `None`（不得 `pass == true`）；`determine_conclusion` / 加权 / 阈值 / override 零改动 |
| `packages/desktop/package.json` | `version` 0.5.28 → 0.5.29（用户可见：卡死不再烧满重试 + 终局文案含推进序列 + 覆盖率不再假通过；沿「仅用户可见变更才提升」纪律） |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| `compiling_count` 三形态（多行 / 前导空白 / 无 `Compiling ` → `0`）、`progress_series` 长度与序（含未重试长度 1）、`is_progress_stalled` 边界（`len < 2` / 相等 / 上升 / 末值 0 / 下降） | `crates/infra/exec/src/transient_test.rs` | 新增（纯函数，`RunReport` 字面量直构） |
| 签名表：`failed to remove <path>` 命中（返回表内原文）；常量锚定改 `[5, 10, 20, 40] s`（含 13 条封闭表计数不变） | 同上 | 改写（`failed to remove file` 旧断言作废） |
| 卡死早停：shim 连续两次相同 `Compiling` 计数 → `attempts == 2 < max_attempts`；递减序列（264 → 5 → 0）→ 跑到底（第 4 次成功，`attempts == 4`）；末次计数 `0` 相等 → 不早停；退避边界按 `RetryPolicy::transient().backoff`（5 s / 10 s）断言 | `crates/infra/exec/src/run_test.rs` | 新增 + 既有「退避逐段」用例改为取策略常量（1 s / 2 s 硬编码断言作废） |
| 再导出与常量：`transient.backoff == [5, 10, 20, 40] s`、`total_budget == 1500 s`；新三函数经 crate 根可达 | `crates/infra/exec/src/lib_test.rs` | 改写（退避四段断言） |
| `fault_text`：含 `重试进展：待编译 264 → 5 → 0 → 0` 与卡死判定语；`attempts == 1` / 全 `0` 时无该行；既有归因段（占用者 / 已重试次数 / 指引）断言保留 | `crates/infra/checks/src/environment_test.rs` | 改写（签名多一参 + 新增两臂；既有「`Compiling` 前缀 + 锁末行」用例补 `retried`） |
| 跨 plan 总预算：多 plan 超 2400 s → 余下 plan 零 spawn、`fault = Environment(总时长超预算)`、报告树无 summary；预算内零行为变化；预算不覆盖既有 fault | `crates/infra/checks/src/testexec/mod_test.rs` | 新增（沿既有 shim 装置与 `PathWindowGuard` 先例；预算用例注入小预算或以可测常量替身，实现细节由 design/tasks 定） |
| 覆盖率采集完整性：rust 缺覆盖率 + vite 达标 → `conclusion == Error` 且 `coverage == None`；同 suite 去重不追加第二条；全测齐全时判定逐字节不变 | `crates/core/checks/src/aggregate_test.rs` | 新增（fixtures 直驱，零 spawn） |
| `fault_text` 调用点：static-check 瞬态耗尽 / test-execution 瞬态耗尽两面的 fault 文本断言补推进序列 | `crates/infra/checks/src/static_check_test.rs`、`crates/infra/checks/src/testexec/runner_test.rs` | 补断言（既有用例结构不变） |

### 不要修改

- `crates/infra/exec/src/{output,shell,lockprobe}.rs` 与 `record_exec_output` / `read_exec_output` / `mint_exec_id` / `kill_tree` 本体语义；`RunOptions` / `RunReport` / `RunOutcome` / `RunError` 字段面。
- `crates/core/checks` 其余纯层（parser / reuse / diagnose / model schema / globmatch）；`crates/core/orchestration/**`（`RunFault` 载荷与 walker 五处分派）；`crates/infra/checks/src/mutation/**`（不接 retry）。
- `crates/core/foundation/**`（layout / cancel / 名单）。
- 前端 `packages/desktop/src/**` 与 `src/types/generated/bindings.ts`（零载荷字段出线，`bindings:check` 应零 diff）；`openspec/config.json`（预算 / 退避参数零 schema 扩展）。
- `packages/desktop/src-tauri/src/main.rs` 的 `DATA_DIR`（P7 未决，见风险 R-8）。
- `plugins/**`（插件源已弃用，零改动零 bump）；`openspec/changes/static-check-file-lock/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 卡死形态（`N_i == N_{i-1} > 0`）提前放弃：`attempts < max_attempts` 即收口，不烧满 5 次预算 | `run_test` 卡死 shim 用例（`attempts == 2`） |
| AC-2 | 递减形态（`264 → 5 → 0`）不早停，继续到成功或预算耗尽；末次计数 `0` 相等亦不早停 | `run_test` 递减 / 末值归零两臂 |
| AC-3 | 退避为 `[5, 10, 20, 40] s`（代码常量锚定）、`total_budget = 1500 s` 不变、尝试上限 5 不变；成功路径零额外等待 | `transient_test` + `lib_test` + `run_test` 退避逐段用例 |
| AC-4 | remove 形态真实命中：`failed to remove D:\…\x.rcgu.o` 触发瞬态判定并返回表内原文 `failed to remove` | `transient_test` 表驱动用例 |
| AC-5 | 重试耗尽终局文案含占用进程名 + `重试进展：待编译 …` 序列（卡死时含判定语）；未重试（`attempts == 1`）或非 cargo 输出（全 `0`）时无该行 | `environment_test` 两臂 + `static_check_test` / `runner_test` 文案断言 |
| AC-6 | 多 suite 时 run 墙钟有上界：超 `TESTEXEC_TOTAL_BUDGET` 后余下 plan 零 spawn、整体以 `Environment` fault 收口（不写 summary、不进复用门），既有 fault 不被覆盖 | `testexec/mod_test` 预算用例 |
| AC-7 | 覆盖率缺测不再假通过：rust 缺覆盖率 + vite 达标 → `conclusion == Error` 且 `coverage` 不为 `pass: true`（收敛 `None`）；同 suite 不重复归因 | `aggregate_test` fixtures 用例 |
| AC-8 | 既有语义零变：真红（无签名）零重试 `attempts == 1`；`TimedOut` / `Cancelled` 零重试；static-check `2>&1` 交错流与 `diagnostics`、test-execution 合并流顺序、`report_dir` / `exec_id` 回填、复用门判定、基线与 mutation 分流全部逐字节不变 | 既有 `run_test` / `static_check_test` / `runner_test` / `mod_test` / `mutation/*_test` / `aggregate_test` 全绿 |
| AC-9 | 硬验收（现场回归，非 CI 可断言）：冷 target（新建 worktree）上静态检查 / 测试执行**不需人工干预**即可通过；重试耗尽时终局文案含占用进程名与推进序列 | 真实 worktree 跑一次 `pnpm run check` + 人工读终局文案 |
| AC-10 | 门禁：`pnpm -C packages/desktop run check` 全绿、覆盖率 ≥ lines 90 / branches 85 / functions 90、`bindings:check` 零 diff、`version` 0.5.28 → 0.5.29、`plugins/**` 零改动 | desktop 门禁 + 版本 diff + bindings 检查 |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **计数启发式误判**：非 cargo 输出恒 `0`（退化为固定次数，良性）；但若某次真实推进恰好产出与上次相同数量的 `Compiling` 行（如新增 crate 与已完成 crate 数相抵）会被误判为卡死 | 少拿 1–3 次机会 | 中 | 判据收窄为「末值 `> 0` 且非降」；实测收敛序列严格递减；文案明写「判定为卡死」可辨；用户可重新发起（复用门因输入未变而复用，重跑成本可控） |
| R-2 | 退避上调延长失败路径墙钟（最坏多 75 s） | 失败用例体感变慢 | 低 | 成功路径零影响（不命中签名不重试）；`total_budget = 1500 s` 硬顶不变；常量集中一处便于回调 |
| R-3 | 卡死早停让「偶发同数」少一次机会，而第 3 次本可能成功 | 环境面假失败 | 低 | 早停只在「已命中签名且非降」时触发，第一轮永不触发；fault 文案带被锁文件与占用者点名，用户能判是环境问题并重发 |
| R-4 | 推进序列行拼进 `fault` 文本后终局文案变长 | 用户阅读负担 / 文案溢出 | 低 | 序列长度 `O(attempts)`（至多 5 个数字）；只进 `TXT_ENV_*`，不占 `findings_detail` 的 2000 字符预算 |
| R-5 | 跨 plan 总预算收口丢弃已完成 plan 结果（不写 summary） | 用户看不到前两个 suite 的汇总 | 中 | 沿既有 fault 收口口径（半套报告本就不可信、会被复用门永久复用）；已完成 plan 的工件仍在报告树可查，输出经 record 单点保留，fault 文案写明超预算 |
| R-6 | `2400 s` 常量缺实测依据（suite 数量增长后可能偏小或偏大） | 过早收口或起不到闸门作用 | 中 | 单 plan 1200 s × 2 有余；当前形态（1 rust + 1 vite）实测最坏 ≈ 1575 s，留约 1.5 倍余量；常量集中一处，后续 suite 增长时复核 |
| R-7 | 覆盖聚合收敛 `None` 让「有覆盖率的 suite 达标」也不再显示为通过 | 用户困惑「明明 96% 为何不通过」 | 中 | 同步追加该 suite 的 `ExecutionError` problem（点名哪个 suite 缺覆盖率）；`diagnose_findings` 既有 execution_error 归因链照旧输出 |
| R-8 | P7（worktree 根 `C:\Users` vs D 盘）未分离：若 H1 成立，本次未采纳成本最低的根治手段（改一个常量、不依赖 IT） | 现场仍靠重试兜底 | 中 | 实验（`mklink /J` 到 D 盘，5 分钟、不改代码）列为后续第一步；成立则另立变更把 `DATA_DIR` / target 落位改到非用户目录；本变更的六处补丁与该根治正交，不因之作废 |
| R-9 | 三处新纯函数（`compiling_count` / `progress_series` / `is_progress_stalled`）稀释 functions 覆盖率（距阈值余量有限） | 门禁红 | 低 | 三者均为纯函数，先以单元测试全覆盖（含空 / 非 UTF-8 / 前导空白 / 越界序列）再计入 |

## 遗留观察项（不在本篇范围）

1. P7 的 H1 / H2 分离实验（见 R-8）；成立后另立变更。
2. 云枢（`TBProtect`）是否提供本地排除 UI（无版本信息，可能云控下发）。
3. 火绒企业版排除后是否即时生效（minifilter 通常需重启驱动）。
4. rust suite 仍跑两遍测试（`cargo llvm-cov` 内部行为，无进一步优化空间）。
5. `export-bindings` 覆盖率 0%（`crates/tools` 无测试；实测 lines 96.64 ≥ 90，补测试可另立变更）。
6. 环境侧根治（排除 workspace 父目录、`.cargo`、pnpm 缓存）属 IT 行动项，产品只负责指出（`ENV_GUIDANCE` 文案已在位）。
