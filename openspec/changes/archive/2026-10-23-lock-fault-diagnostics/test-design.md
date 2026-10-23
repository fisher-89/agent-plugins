# 测试设计: lock-fault-diagnostics

> **变更**: lock-fault-diagnostics
> **日期**: 2026-10-22
> **基于**: `proposal.md`（验收标准 AC-1 ~ AC-11；「变更范围 - 实现文件」12 项 +「变更范围 - 测试文件」表）、`design.md`（「新增文件」3 项 /「修改文件」13 项；「公共函数 / API」子表 7 行；「类型定义」子表 11 行；「配置」子表 4 行；关键设计决策 D1 ~ D20）、`specs/{desktop-exec-runtime,desktop-checks-domain,desktop-test-execution,desktop-mutation-testing,desktop-change-orchestration}/spec.md`
> **被测模块来源**: `design.md`「新增文件」3 项（`crates/infra/exec/src/transient.rs`、`crates/infra/exec/src/lockprobe.rs`、`crates/infra/checks/src/environment.rs`）+「修改文件」13 项（`crates/**` 11 个 Rust 实现文件 + `crates/infra/exec/Cargo.toml` + `packages/desktop/package.json`）；design 无「删除文件」子表（`read_all` / `parse_*` 的删除属文件内退化，不构成模块消失）。
> **测试框架**: 沿用仓库既有约定——Rust 侧 `#[test]` / `#[tokio::test]`（dev-deps `tokio` 的 `rt` / `macros` / `test-util`），测试模块以**同目录兄弟文件**承载：exec / checks 子模块用 `#[cfg(test)] #[path = "<name>_test.rs"] mod <name>_test;`（`run.rs` / `output.rs` / `testexec/runner.rs` / `mutation/*` 先例），crate 根模块用 `#[cfg(test)] mod <name>_test;`（`orchestration/src/lib.rs` / `checks/src/lib.rs` 先例）；断言用标准 `assert!` / `assert_eq!` / `assert!(matches!(..))`，测试函数名为中文标识符、断言消息为中文；无外部断言库、无独立测试运行器。新增 `transient_test.rs` / `lockprobe_test.rs` / `environment_test.rs` 沿同目录兄弟文件形态挂载（design AC-10 回应：三新模块均以私有面设计，可被同模块 `#[cfg(test)]` 子模块直驱，**不新增测试专用 `export`**——AGENT.md 纪律）。

用例表读法：`路径类型` ∈ {正向, 异常, 边界}；`迭代类型` ∈ {单值, 参数化, 表驱动, 遍历枚举}。跨模块组合用例按「链路发起方 / 最上层调用方」挂靠入口模块的 `#### 用例` 表（本变更组合锚分别挂靠 `run.rs`——执行体重试全链、`static_check.rs`——瞬态耗尽到 fault 文本全链、`testexec/mod.rs`——多 plan fault 聚合全链、`walker.rs`——fault 分派到 run 收敛全链），不设独立集成测试章节。

边界取值口径（按 design「类型定义」表的 Rust 类型逐项收敛）：

| 类型（载体） | 边界取值 | 最少条数 |
|---|---|---|
| `u32`（`RunReport.attempts` / `RetryPolicy.max_attempts`） | `1`（单次）/ `2`（一次重试）/ `5`（`transient()` 上限）/ 上限之外不发起（`attempts` 不再增长）；`0` 为类型不变量外状态（公开 API 不可产生，见 `## 不可测试项` 第 12 项） | 4 边界 + 1 正常 |
| `i32`（`RunReport.exit_code` / `AttemptTrace.exit_code`） | `0`（绿）/ `1` / `101` / `255`（谱内非零）/ `-1` 哨兵（`TimedOut` / `Cancelled` / 无可读退出码） | 4 边界 + 1 正常 |
| `Vec<u8>`（`stdout` / `stderr` / `attempts_log` 产出） | `[]`（空）/ 单字节 / 大块（>64 KiB 管道缓冲）/ 非 UTF-8 原始字节（`0xFF 0xFE`）/ 特殊字符（`\n` `\0` emoji） | 5 边界 + 1 正常 |
| `String` / `&str`（诊断、`Environment(String)` 载荷、`ENV_GUIDANCE`） | `""`（空）/ 全空白 / 超长（>1000 字符）/ 特殊字符（引号、换行、`\0`、emoji、中文签名 `另一个程序正在使用此文件`） | 4 边界 + 1 正常 |
| `Vec<PathBuf>`（`locked_paths` 产出 / `lock_holders` 入参） | `[]`（空）/ 单元素 / 超过上限（>8 条输入 → 至多 8 条产出）/ 不存在路径 / 重复路径 | 4 边界 + 1 正常 |
| `[Duration; 4]`（`RetryPolicy.backoff`） | 四段逐值 `[1 s, 2 s, 4 s, 8 s]`；只消费前 `attempts - 1` 段（不越界、不多等） | 2 边界 + 1 正常 |
| `Duration`（`RetryPolicy.total_budget` / `RunOptions.timeout`） | `Duration::ZERO`（首跑后即判耗尽）/ 常量值 `1500 s` / 必填不可缺省 | 2 边界 + 1 正常 |
| `Option<RunFault>` | `None`（结论本身有效）/ `Some(Stopped)` / `Some(Environment(""))` / `Some(Environment(超长 / 特殊字符))` | 4 边界 + 1 正常 |
| enum | `RunOutcome` 三值、`RunFault` 两变体、`SegmentFailure` 三变体、`TestExecutionConclusion` / `MutationTestConclusion` 三值逐个取值遍历（封闭集，非法值属编译期不可表达） | N / N+1 |

`RetryPolicy` 字段为 `pub`（design 类型表：供 `run.rs` 兄弟模块读取与测试断言），故测试侧可直构「`max_attempts = 1/2`」「`total_budget = ZERO`」等边界策略；生产构造面仍收口于 `none()` / `transient()` 两 fn（用例表以「直构边界策略」标注）。

路径省略说明：`验收范围` 表内 `.../` 前缀补齐为 `packages/desktop/src-tauri/crates/`；下文 `## 单元测试` 各节标题给全路径。

---

## 验收范围

| AC ID | 验收条件（摘要，原文见 `proposal.md`） | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | 命中瞬态签名自动重试；耗尽后 `attempts == 5`；flaky 场景 `attempts == 3` 即绿；总预算 1500 s 生效（超预算不再发起新尝试） | `.../infra/exec/src/transient.rs` -> `.../infra/exec/src/transient_test.rs`（13 条签名表驱动 + 两构造常量锚定）、`.../infra/exec/src/run.rs` -> `.../infra/exec/src/run_test.rs`（flaky 三臂 / 耗尽上限 / 预算硬顶 / 退避序列）、`.../infra/checks/src/static_check.rs` -> `.../static_check_test.rs`（`RetryPolicy::transient()` 开面后重试可见面）、`.../infra/checks/src/testexec/runner.rs` -> `.../testexec/runner_test.rs`（同上） |
| AC-2 | `TimedOut` / `Cancelled` 零重试（`attempts == 1`、`retried` 空）；真红（无签名非零）零重试且行为与现状逐字节一致 | `.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（四臂：超时 / 取消 / 真红 / 裸 `failed to write` + 单次等价）、`.../exec/src/transient.rs` -> `.../exec/src/transient_test.rs`（`TimedOut` / `Cancelled` 恒 `None`）、`.../checks/src/mutation/mod.rs` -> `.../mutation/mod_test.rs`（三段零重试）、`.../mutation/cargo_mutants.rs` -> `.../mutation/cargo_mutants_test.rs`、`.../mutation/stryker.rs` -> `.../mutation/stryker_test.rs`（`RetryPolicy::none()` 随动） |
| AC-3 | 耗尽后归因点名占用者：文案含「占用进程：<名> (pid)」或「未定位占用者（可能为内核过滤驱动）」+ 被锁文件路径 + 「已自动重试 N 次」+「排除 workspace 父目录（逐 worktree 无效）」 | `.../exec/src/lockprobe.rs` -> `.../exec/src/lockprobe_test.rs`（三形态路径解析 / RM 与 `lsof` 点名 / 归因原料）、`.../checks/src/environment.rs` -> `.../checks/src/environment_test.rs`（重试次数 + 末次摘要 + 指引单点装配）、`.../checks/src/static_check.rs` -> `.../static_check_test.rs`（组合锚：瞬态耗尽 → fault 文本全链）、`.../core/orchestration/src/walker.rs` -> `.../walker_test.rs`（终局文案含点名与次数） |
| AC-4 | 探针不误报、不扰动：share 含 delete 的读句柄判「未阻塞」；无 delete 共享判「阻塞 err=32」；探针自身只读 | `.../exec/src/lockprobe.rs` -> `.../exec/src/lockprobe_test.rs`（三臂判据 + 打开即关 + 文件字节/长度不变）；「源码零 `create` / `truncate` / 独占 share」的面见 `## 不可测试项` 第 6 项 |
| AC-5 | 环境错误不再注回 executor：`fault = Environment` 时 walker 零 worker 调用、相位 fail 未升格、决策未分叉；run 收敛 `failed` 且文案引导环境侧 | `.../core/orchestration/src/walker.rs` -> `.../walker_test.rs`（`FakeWorker` 请求计数 0 / 零 `phase_log` 代写 / 零 decision 会话 / run `Failed`）、`.../core/orchestration/src/port.rs` -> `.../port_test.rs`（`fault` 载荷面）、`.../checks/src/testexec/mod.rs` -> `.../testexec/mod_test.rs`（不写 summary / 不落 `execution_error`） |
| AC-6 | 环境错误与基线检查红文案可辨：前者引导「排除安全软件路径」，后者仍引导「修基线 / 修测试」，两态不共用同一段文案 | `.../core/orchestration/src/walker.rs` -> `.../walker_test.rs`（`TXT_ENV_STATIC` / `TXT_ENV_TEST` vs `TXT_BASELINE_*` 两组断言）、`.../checks/src/environment.rs` -> `.../checks/src/environment_test.rs`（指引与摘要原料）、`.../checks/src/static_check.rs` / `.../testexec/runner.rs`（超时文案逐字） |
| AC-7 | 取消运行中的检查步 → 该步节点 `stopped`、run 秒级收敛 `stopped`（非 failed），static-check / test-execution / mutation 三面一致 | `.../core/orchestration/src/walker.rs` -> `.../walker_test.rs`（三面 `Stopped` 分派 + run `Stopped` + 无 failed 节点）、`.../checks/src/static_check.rs` -> `.../static_check_test.rs`、`.../checks/src/testexec/runner.rs` -> `.../testexec/runner_test.rs`、`.../checks/src/testexec/mod.rs` -> `.../testexec/mod_test.rs`（`Stopped` 早停）、`.../checks/src/mutation/mod.rs` -> `.../mutation/mod_test.rs`（旧「收敛 failed / `Error`」断言改写） |
| AC-8 | 既有语义零变：static-check `2>&1` 交错流与 `diagnostics` 口径不变；test-execution / mutation 合并流顺序不变；报告 / summary / 复用门 / 段级断点续跑不受影响；无配置直过 / 程序不可达 `Err` / `Fatal` 分流不变 | `.../exec/src/run.rs` -> `.../exec/src/run_test.rs`（单次尝试逐字节等价 + 既有用例全绿）、`.../exec/src/lib.rs` -> `.../exec/src/lib_test.rs`（既有导出面零变）、`.../checks/src/static_check.rs` -> `.../static_check_test.rs`（交错流 / `stderr` 不读 / `passed` 口径 / 无配置直过 / 不可达 `Err`）、`.../checks/src/testexec/runner.rs` -> `.../testexec/runner_test.rs`（合并流顺序 / 工件解析）、`.../checks/src/testexec/mod.rs` -> `.../testexec/mod_test.rs`（复用门 / summary / `exec_id`）、`.../checks/src/mutation/mod.rs` -> `.../mutation/mod_test.rs`（`Fatal` / `Interrupted` 分流 / 临时面保留纪律） |
| AC-9 | `RunFault` 不出线：`bindings:check` 零 diff、无新 IPC 命令、载荷类型未进 `RunUpdate` / `RunNodeRecord` | `.../core/orchestration/src/port.rs` -> `.../port_test.rs`（`RunFault` 零 `Serialize` 的编译期面 + 三 outcome 字段面穷尽解构）；`bindings` / IPC 面见 `## 不可测试项` 第 4 项 |
| AC-10 | 全部既有测试通过（含约 71 处 outcome 字面量补 `fault: None`），覆盖率不低于阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90） | 本文件全部映射对（逐文件 `#### 用例` 表）+ 字面量随动与覆盖率统计面见 `## 不可测试项` 第 5、8 项 |
| AC-11 | `packages/desktop/package.json` 0.5.24 → 0.5.25；`plugins/**` 零改动零 bump；`windows-sys` 仅为 `cfg(windows)` 目标依赖且 feature 只含 `Win32_System_RestartManager` | `packages/desktop/package.json`、`.../infra/exec/Cargo.toml`、`plugins/**` 均无单元测试路径 → `## 不可测试项` 第 1、2、9 项 |

> 被测范围边界：`crates/core/checks/**`（解析 / 聚合 / 复用门纯层）、`crates/core/workflow/**`、`crates/core/orchestration/src/state.rs`、`crates/infra/exec/src/{output,shell}.rs` 及 `record_exec_output` / `read_exec_output` / `mint_exec_id` / `kill_tree` 本体、`crates/infra/agent/**`、`crates/infra/vcs/**` 生产源码、`crates/core/foundation/**`、`crates/infra/checks/src/testexec/{detect,report}.rs`、`packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**` 均为只读红线（design「只读红线零触点」），不纳入被测范围。`mutation/cargo_mutants.rs` / `stryker.rs` 仅「`RunOptions.retry` 必填随动」一行属本变更（proposal 未列，design 修改文件表列明），故纳入映射对但不设公共 API 待测项。
>
> 机械随动（`core/orchestration/src/steps_test.rs`、`infra/vcs/src/lib_test.rs` 与三 outcome 字面量补 `fault: None`）不构成源模块变更，见 `## 不可测试项` 第 5 项。

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/exec/src/transient.rs -> packages/desktop/src-tauri/crates/infra/exec/src/transient_test.rs

<!-- design.md「公共函数 / API」子表三行（`is_transient` / `RetryPolicy::none` / `RetryPolicy::transient`）与「类型定义」子表两行（`RetryPolicy` / `AttemptTrace`）落本文件。测试模块以 `#[cfg(test)] #[path = "transient_test.rs"] mod transient_test;` 挂载（沿 exec crate 既有 `run_test` / `output_test` 形态）；`pub(crate) fn transient_signature(..)` 为私有判定核心，经该内嵌模块可直驱（`super::transient_signature`），公开面断言仍只走 `is_transient`。 -->

#### 待测功能

- `is_transient(report: &RunReport) -> Option<&'static str>`（公共函数表，AC-1/AC-2/D5）：三合一判据——`outcome == Completed` **且** `exit_code != 0` **且** 合并输出（`stdout ++ stderr`，lossy、整体小写）包含签名表中任一条（逐条小写比对）；命中返回**表中原文**（`LNK1104` / `EBUSY` / `os error 32` 等保持原形态），未命中 / 不满足前两条件返回 `None`
- `RetryPolicy::none()`（公共函数表，AC-2/D1）：单次执行——`max_attempts == 1`，`backoff` / `total_budget` 取同类常量但不被消费
- `RetryPolicy::transient()`（公共函数表，AC-1/D1）：`max_attempts == 5`（首跑 + 4 次重试）、`backoff == [1 s, 2 s, 4 s, 8 s]`、`total_budget == 1500 s`；参数 MUST NOT 进 config schema
- `RetryPolicy`（类型定义表）：`pub max_attempts: u32` / `pub backoff: [Duration; 4]` / `pub total_budget: Duration`，`derive(Debug, Clone, PartialEq, Eq)`；构造面经两 fn 收口（字段 `pub` 供 `run.rs` 与测试读取）
- `AttemptTrace`（类型定义表，AC-1）：`pub exit_code: i32` / `pub transient: Option<String>`（命中签名原文，未命中 `None`）/ `pub stdout: Vec<u8>` / `pub stderr: Vec<u8>`；`derive(Debug, Clone, PartialEq, Eq)`；只承载**此前**各次尝试

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `is_transient` — 签名表逐条命中（AC-1） | 正向 | `Completed` + `exit_code = 1` + `stdout` 含该行签名 → 返回签名原文：Windows 锁族 7 条（`os error 32` / `os error 5` / `另一个程序正在使用此文件` / `being used by another process` / `failed to rename archive file` / `failed to remove file` / `Access is denied`）、MSVC 1 条（`LNK1104`）、POSIX·Node 5 条（`EBUSY` / `ETXTBSY` / `Text file busy` / `EPERM: operation not permitted` / `resource busy or locked`） | 表驱动（13 条封闭表） |
| `is_transient` — 大小写不敏感 | 正向 | 同上 13 条各以全大写 / 首字母大写 / 全小写三形态注入（如 `OS ERROR 32` / `Os Error 32` / `lnk1104`）→ 均命中且返回**表中原文**（不返回输出片段） | 参数化（3 形态 × 首条 + 抽样 3 条） |
| `is_transient` — 两路合并匹配 | 正向 | 签名只在 `stderr`（`stdout` 空）→ 命中；签名只在 `stdout`（`stderr` 空）→ 命中；两路各含不同签名 → 命中（`stdout` 在前、返回首条命中原文） | 表驱动（3 形态） |
| `is_transient` — 多行与前后噪声 | 正向 | 多行输出：首行 `Compiling …`、末行 `error: failed to rename archive file: … os error 32` → 命中；签名前后含 CRLF / 制表符 / 前后空白 → 仍命中（整串包含匹配） | 参数化（2 形态） |
| `is_transient` — `TimedOut` 恒不命中（AC-2） | 异常 | `outcome = TimedOut` + `exit_code = -1` + 输出含 `os error 32` 全文 → `None`（超时重试会把 1200 s 变数小时） | 单值 |
| `is_transient` — `Cancelled` 恒不命中（AC-2） | 异常 | `outcome = Cancelled` + `exit_code = -1` + 输出含 `failed to rename archive file` → `None`（取消必须立即生效） | 单值 |
| `is_transient` — 绿输出不命中 | 异常 | `Completed` + `exit_code = 0` + 输出含 `os error 32` 文本（如测试用例自打印该串）→ `None`（绿不因文本重试） | 单值 |
| `is_transient` — 真红不命中（AC-2/R-1） | 异常 | `Completed` + `exit_code = 1` + `error[E0308]: mismatched types`（普通编译错误）→ `None` | 单值 |
| `is_transient` — 裸 `failed to write` 不命中（AC-1/D5） | 边界 | `Completed` + `exit_code = 1` + 输出仅 `failed to write /tmp/x`（无 `os error` 码、无锁措辞）→ `None`；同串外加 `os error 32` → 命中（同 line 覆盖口径） | 表驱动（2 形态） |
| `is_transient` — 输出边界 | 边界 | `stdout` / `stderr` 双空 → `None`；单字节 → `None`；大块（>64 KiB）且签名在末尾 → 命中（不截断判定）；非 UTF-8 原始字节（`0xFF 0xFE` + 签名）→ lossy 后仍命中、不 panic | 表驱动（4 形态） |
| `is_transient` — 特殊字符诊断 | 边界 | 签名混在含引号 / 换行 / `\0` / emoji 的输出中 → 命中且不 panic（中文签名 `另一个程序正在使用此文件` 单独一臂验证 UTF-8 比对） | 参数化（2 形态） |
| `is_transient` — `Completed` + `-1` 哨兵 | 边界 | `Completed` + `exit_code = -1`（无可读退出码）+ 输出含签名 → 按判据（`exit_code != 0`）命中；同条件无签名输出 → `None`（哨兵不被误读为绿） | 表驱动（2 形态） |
| `is_transient` — 私有判定核心直驱 | 边界 | `transient_signature(outcome, exit_code, stdout, stderr)` 与 `is_transient(&report)` 对同一组输入逐值一致（公开面只做委托，判定逻辑单点） | 表驱动（4 组输入） |
| `RetryPolicy::transient()` — 三常量（AC-1） | 正向 | `max_attempts == 5`、`backoff == [Duration::from_secs(1), 2 s, 4 s, 8 s]`、`total_budget == Duration::from_secs(1500)`（常量锚定，改值即被击杀） | 单值 |
| `RetryPolicy::none()` — 单次语义（AC-2） | 正向 | `max_attempts == 1`；`backoff` 四段与 `total_budget == 1500 s` 为同类常量值（不被消费）；`none()` 连续两次调用相等 | 单值 |
| `RetryPolicy` — 相等与 clone | 边界 | `none() != transient()`；`transient().clone() == transient()`；`Debug` 渲染含 `max_attempts` / `total_budget`（derive 可读） | 表驱动（2 形态） |
| `RetryPolicy` — 直构边界策略 | 边界 | 字段直构 `{ max_attempts: 1, ..transient() }` / `{ max_attempts: 2, .. }` / `{ total_budget: Duration::ZERO, .. }` → 结构与取值可读（为 `run_test` 的上限 / 预算边界提供装置，构造面无 `panic`） | 表驱动（3 形态） |
| `AttemptTrace` — 字段面（AC-1） | 正向 | `{ exit_code: 1, transient: Some("os error 32"), stdout: …, stderr: … }` 构造后可读、`clone` 载荷逐字节相等、`PartialEq` 自反；`transient: None`（未命中）合法 | 单值 |
| `AttemptTrace` — 输出边界 | 边界 | `stdout` / `stderr` 双空 / 单字节 / 大块 / 非 UTF-8 → 字段原样保留（零改写零截断）；`transient` 取空串（防御性直构，生产不产生）不 panic | 表驱动（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：`RunReport` / `AttemptTrace` / `RetryPolicy` 均以**结构体字面量直构**（被测 API 的显式入参），纯 std 判定零 IO / 零 spawn / 零时钟 | 全部用例 |
| 环境 / 文件系统 / 子进程 | 不涉及：本模块 std-only（design 组件表「零 tokio、零第三方」） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/exec/src/run.rs -> packages/desktop/src-tauri/crates/infra/exec/src/run_test.rs

<!-- design.md「公共函数 / API」子表一行（`run_shell`，签名不变语义扩展）+「类型定义」子表两行（`RunOptions` 增 `retry`、`RunReport` 增 `attempts` / `retried`）落本文件；私有 `attempt_once` / `Attempt` 与既有 `wait_cancel` / `drain` / `await_exit` 经 `#[cfg(test)] #[path = "run_test.rs"] mod run_test;` 子模块可直驱。既有 914 行用例（三值结局 / 双管道 / `cwd` / `env` / pid 回收）按 design「单次语义零变」补 `retry: RetryPolicy::none()` 后全绿（唯一必须改动的装置面是 `options()` 助手加一行）。 -->

#### 待测功能

- `run_shell(command: SystemCommand, options: RunOptions) -> Result<RunReport, RunError>`（公共函数表，AC-1/AC-2/D1–D4）：入口取消短路 → 循环（`attempt_once` → 瞬态判定 → 上限判定 → 预算判定 → 退避 + 取消观测）→ 组 `RunReport`；重试复用同一执行骨架（`kill_on_drop` spawn / 双管道并发排水 / 三臂 `select!` / 杀树 + 有界收尾），`attempt_once` 内 `timeout` 臂不受预算影响
- `RunOptions`（类型定义表，AC-1）：增 `pub retry: RetryPolicy`；字段集 `{ cwd, env, timeout /* 必填 */, cancel, retry }`，仍无 `Default`（`retry` 为必填字段，四生产调用点各显式给值）
- `RunReport`（类型定义表，AC-1/D4）：增 `pub attempts: u32`（≥1）与 `pub retried: Vec<AttemptTrace>`（**末次不入**）；`stdout` / `stderr` 仍为**末次尝试**字节
- 边界口径（D4 ①~③）：①入口取消短路（未 spawn）`attempts = 1` / `retried = []`；②预算或上限早停记**实际完成次数**、末次不进 `retried`；③退避等待期取消 → `outcome = Cancelled`、`exit_code = -1`、`attempts` = 已完成次数、`stdout` / `stderr` 取刚完成的末次字节、`retried` **不含**该末次
- 零改面（AC-8）：`CANCEL_POLL_INTERVAL`（200 ms）/ `FINISH_GRACE`（2 s）取值与 `drain` / `wait_cancel` / `await_exit` / `kill_tree` 语义逐字不变；`RunError` 两变体文案不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run_shell` — flaky 锁冲突自愈（AC-1 组合锚） | 正向 | shim 第 1、2 次输出 `failed to rename archive file: … os error 32` 并 `exit 1`、第 3 次 `exit 0` 输出成功行；`retry = transient()` → `outcome == Completed`、`exit_code == 0`、`attempts == 3`、`retried.len() == 2`（各 `exit_code == 1` + `transient == Some("os error 32")`）、末次 `stdout` / `stderr` 恰为第 3 次字节（不含历史尝试） | 单值 |
| `run_shell` — 退避序列（AC-1） | 正向 | shim 三次尝试均命中签名后成功；虚拟时钟断言退避恰为 1 s 与 2 s（第 3 次成功前 `elapsed` ≈ 3 s + 尝试耗时，未消费 4 s / 8 s；`backoff` 索引不越界） | 单值 |
| `run_shell` — 持续锁冲突耗尽（AC-1） | 正向 | shim 每次都输出同一锁文本 + `exit 1`，`transient()` → `attempts == 5`（不再增加）、`retried.len() == 4`、末次 `Completed` + `exit_code == 1`、全程虚拟墙钟 ≤ `total_budget`（1500 s） | 单值 |
| `run_shell` — 预算耗尽早停（AC-1/D2） | 边界 | 每次尝试耗时 450 s（shim `sleep` + 虚拟时钟）+ 持续命中签名 → 第 4 次结束时已用约 1807 s（含退避）→ 第 5 次开始前预算判定耗尽，`attempts == 4`、`retried.len() == 3`、末次原样返回（`Completed` + 非零），总墙钟 ≤ `total_budget + 单次 timeout`；shim 标记文件断言「第 5 次未 spawn」 | 单值 |
| `run_shell` — 极短预算边界 | 边界 | 直构 `RetryPolicy { total_budget: Duration::ZERO, ..transient() }` → 首跑后即判耗尽，`attempts == 1`、`retried` 空，零退避等待（虚拟时钟零推进） | 单值 |
| `run_shell` — 自定义上限边界 | 边界 | 直构 `max_attempts = 1` / `2` + 持续命中签名 → 分别 `attempts == 1`（`retried` 空）/ `attempts == 2`（`retried.len() == 1`），spawn 次数与上限逐值相等 | 表驱动（2 形态） |
| `run_shell` — 真红零重试（AC-2） | 正向 | shim 输出 `error[E0308]: mismatched types` + `exit 1`，`retry = transient()` → `attempts == 1`、`retried` 空、`outcome == Completed`、shim 标记文件仅一次（重试策略在位但不触发） | 单值 |
| `run_shell` — `none()` 单次语义（AC-2/AC-8） | 正向 | `retry = none()` + 输出命中签名 + `exit 1` → `attempts == 1`、`retried` 空、单次 spawn；同 shim 在 `transient()` 下 `attempts == 5`（策略是唯一变量，证 `none()` 不开重试面） | 表驱动（2 策略） |
| `run_shell` — 单次等价回归（AC-8） | 正向 | `retry = none()` + `exit 0` + 双路输出 → `outcome` / `exit_code` / `stdout` / `stderr` 与本变更前逐字节一致，`attempts == 1`、`retried.is_empty()`、零退避等待（虚拟时钟不推进） | 单值 |
| `run_shell` — 末次输出非合流（AC-8） | 正向 | 第 1、2 次尝试各输出不同文本后成功 → `stdout` / `stderr` 不含前两次字节（执行体不预合流、不拼接历史）；前两次字节只在 `retried[0]` / `retried[1]` 按尝试序可读 | 单值 |
| `run_shell` — 超时零重试（AC-2） | 正向 | 自旋 shim（输出含 `os error 32`）+ `transient()` + 虚拟时钟 `advance(timeout)` → `outcome == TimedOut`、`exit_code == -1`、`attempts == 1`、`retried` 空；pid 消失（既有 pid 断言装置） | 单值 |
| `run_shell` — 已置位 cancel 零重试（AC-2/D4①） | 正向 | `transient()` + 先 `handle().set()` 再调 → `outcome == Cancelled`、`exit_code == -1`、`attempts == 1`、`retried` 空、shim 零执行（标记文件不存在）、墙钟近零 | 单值 |
| `run_shell` — 运行中 cancel 零重试（AC-2/D4③） | 正向 | shim 落 pid 后自旋并输出签名文本，运行中 `handle().set()` → `Cancelled`、`exit_code == -1`、`attempts == 1`、`retried` 空、pid 消失、`stdout` 为部分字节（完整性不断言） | 单值 |
| `run_shell` — 退避期 cancel 即刻生效（AC-1/D3/D4③） | 正向 | 第 1 次命中签名后进入退避；在退避窗口内 `handle().set()` → 不再发起第 2 次尝试（shim 标记计数 == 1）、`outcome == Cancelled`、`exit_code == -1`、`attempts == 1`（已完成次数）、`retried` 空（该末次不入）、`stdout` / `stderr` 取第 1 次字节、墙钟 < 剩余退避（不先等完 1 s） | 单值 |
| `run_shell` — 退避期 cancel（两次重试后） | 边界 | 第 1、2 次命中、退避 2 s 窗口内取消 → `attempts == 2`、`retried.len() == 1`（第 1 次已入栈，第 2 次为「刚完成的末次」不入栈）、`stdout` 取第 2 次字节 | 单值 |
| `run_shell` — spawn 失败不重试 | 异常 | `SystemCommand::bare("no-such-program-xyz")` + `transient()` → 首次尝试即 `Err(RunError::Spawn(_))`（无 `RunReport` 可携 `attempts`），shim 零 spawn | 单值 |
| `run_shell` — 双管道 + 大输出在重试面 | 边界 | 每次尝试 stdout / stderr 各 >64 KiB（含签名）后成功 → `retried` 每次字节数精确等于写量、末次同样全量（排水语义与单次一致，不互锁） | 单值 |
| `RunOptions.retry` — 必填与缺省语义（AC-1） | 边界 | 四生产调用点（`static_check.rs` / `testexec/runner.rs` / `cargo_mutants.rs` / `stryker.rs`）均显式给 `retry`（编译期即证）；测试侧 `options()` 助手给 `none()`，既有用例零行为变化 | 表驱动（2 策略） |
| `RunReport.attempts` — 不变量 | 边界 | 全路径（绿 / 真红 / 超时 / 取消 / 耗尽 / 预算早停 / 退避取消）`attempts >= 1` 恒成立；`retried.len() == attempts - 1`（退避取消面为 `attempts - 1` 或 `attempts - 2` 两种按 D4③ 可解释形态，逐面显式断言，不写成模糊不等式） | 表驱动（7 路径） |
| 常量与零改面（AC-8） | 边界 | `CANCEL_POLL_INTERVAL == 200 ms`、`FINISH_GRACE == 2 s` 锚定；既有取消轮询粒度 / 杀后收尾上界用例全绿 | 表驱动（2 常量） |
| `never()` 全程回归锚 | 边界 | 既有 `run_test` 全量用例（三值结局 / 退出码谱 / `cwd` / `env` / pid 回收 / 双缓冲）在 `retry: none()` 下全绿（单次语义逐字节等价） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（被测执行体自身） | 不 mock spawn：既有 POSIX shim 装置（`ShimWs` 落盘脚本 + `interpreter_command` 经 `resolve_shell` 探测解释器）真实拉起；重试形态由 shim 读「尝试计数文件」分支（第 N 次输出锁文本 / 成功 / `sleep`），尝试次数与标记文件由 shim 自身落盘 | 全部 spawn 用例 |
| 时钟（预算 / 退避） | `#[tokio::test(start_paused = true)]` + 手动 `tokio::time::advance`（沿既有 `run_test` / `mutation/*_test` 先例）——1500 s 预算与 450 s 尝试耗时以虚拟时钟表达，不真等；取消臂用真实时钟（语义本就是轮询观测，沿既有注释口径） | 预算 / 退避 / 超时用例 |
| 文件系统 | 不 mock：`tempfile::TempDir` 真实 workspace（shim 脚本 / 尝试计数 / pid / 标记文件落位） | 全部用例 |
| 进程树击杀（`exec::kill_tree`） | 不 mock：真实 `taskkill /T /F` / `kill -9`，存活探测 `tasklist` / `kill -0`（既有装置） | 超时 / 取消 pid 回收用例 |
| 内部模块（`transient::is_transient` / `transient_signature` / `AttemptTrace` / `foundation::cancel`） | 不 mock：真实组合（重试判定与取消原语正是被测语义；`CancelFlag` 走真实 `handle().set()`） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/exec/src/lockprobe.rs -> packages/desktop/src-tauri/crates/infra/exec/src/lockprobe_test.rs

<!-- design.md「公共函数 / API」子表三行（`locked_paths` / `lock_holders` / `attribution_text`）与「类型定义」子表一行（`LockHolder`）落本文件；私有 `blocks_delete`（D6 判据，两平台臂）经 `#[cfg(test)] #[path = "lockprobe_test.rs"] mod lockprobe_test;` 子模块以 `super::blocks_delete` 直驱（design 依赖表明写：不为测试新增导出）。平台隔离：判据与 Restart Manager 点名用例以 `#[cfg(windows)]` 门控；非 Windows 只测 `lsof` 缺失降级臂（`#[cfg(not(windows))]`），沿 R-8。 -->

#### 待测功能

- `locked_paths(diagnostics: &str) -> Vec<PathBuf>`（公共函数表，AC-3/D9）：逐行扫描三种错误串形态（`failed to rename archive file` / `failed to write` / `failed to remove`，大小写不敏感）→ 取关键词之后剩余片段 → `trim` → 去前导 `:` 与空白 → 首个 `": "`（冒号+空格）截断（Windows 盘符 `C:\` 不受影响）→ 剥包裹反引号 / 双引号 / 尾随标点 → 非空才收；按 `PathBuf` 原样去重、上限 8 条；零 IO
- `lock_holders(paths: &[PathBuf]) -> Vec<LockHolder>`（公共函数表，AC-3/D8）：Windows 单 session（`RmStartSession` → `RmRegisterResources` → 两次 `RmGetList`，`ERROR_MORE_DATA = 234` 视为正常 → 读 `strAppName` + `dwProcessId` → `RmEndSession`），按 `(name, pid)` 去重；非 Windows 逐路径 `lsof -F pc -- <path>` 解析 `p<pid>` / `c<name>`，`lsof` 缺失静默跳过；任一步失败 / 空清单 → 空 `Vec`（不 `Err`、不 panic）
- `attribution_text(diagnostics: &str) -> Option<String>`（公共函数表，AC-3/D7/D10）：`locked_paths` → `blocked = paths.filter(blocks_delete)` → 点名对象 = blocked（blocked 空则退化为全部解析路径）→ 装配「占用进程：<名> (pid)」/「被锁文件：<path>」（或「未定位占用者（可能为内核过滤驱动（安全软件），无法归因到用户进程）」）；解析不出路径 → `None`；**只产原料**（重试次数 / 处置指引由 `checks::environment` 装配）
- `LockHolder`（类型定义表）：`pub name: String` / `pub pid: u32`，`derive(Debug, Clone, PartialEq, Eq)`
- 私有 `blocks_delete(path) -> bool`（D6，经子模块直驱）：Windows 以 `DELETE | FILE_READ_ATTRIBUTES` 访问权 + `FILE_SHARE_READ | WRITE | DELETE` 全共享打开一次即 `drop`；`Err` 且 `raw_os_error() == Some(32)` → `true`，其余（含 `NotFound` / `AccessDenied` / 打开成功）→ `false`；非 Windows 恒 `false`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `locked_paths` — 三形态解析（AC-3） | 正向 | `failed to rename archive file: D:\w\target\debug\deps\liba.rlib` / `failed to write D:\w\target\libb.rmeta` / `failed to remove D:\w\target\c.rcgu.o` 各一条诊断 → 各解析出 1 条对应路径，分隔符按输入原样保留（反斜杠不归一） | 表驱动（3 形态） |
| `locked_paths` — POSIX 形态 | 正向 | `/home/u/w/target/debug/build/x/out/liby.rlib` 三形态各一 → 路径原样（正斜杠保留），无多余条目 | 表驱动（3 形态） |
| `locked_paths` — 行尾噪声截断（D9） | 正向 | `failed to rename archive file: D:\w\liba.rlib: os error 32` → 只在首个 `": "` 截断，得 `D:\w\liba.rlib`；`failed to write D:\w\libb.rmeta (os error 32)` → 取 `D:\w\libb.rmeta`（尾随标点剥离） | 表驱动（2 形态） |
| `locked_paths` — 盘符冒号不误截 | 边界 | `failed to write C:\Users\w\target\lib.rmeta` → 完整 `C:\Users\w\target\lib.rmeta`（`C:` 后是反斜杠而非空格，不被 `": "` 规则截断） | 单值 |
| `locked_paths` — 包裹引号与反引号 | 边界 | `` failed to remove `D:\w\target\a.rcgu.o` `` / `failed to write "D:\w\target\b.rmeta"` → 剥引号后取裸路径 | 表驱动（2 形态） |
| `locked_paths` — 大小写不敏感 | 边界 | `FAILED TO RENAME ARCHIVE FILE: D:\w\liba.rlib` / `Failed To Write D:\w\libb.rmeta` → 与全小写形态等价解析 | 表驱动（2 形态） |
| `locked_paths` — 多路径去重与顺序 | 边界 | 同一诊断含 3 条不同路径 + 1 条重复 → 3 条产出（去重），顺序 = 首次出现序 | 单值 |
| `locked_paths` — 上限 8 条（D9） | 边界 | 诊断含 11 条不同路径 → 恰 8 条产出（`len() == 8`，不 panic） | 单值 |
| `locked_paths` — 空与无匹配 | 异常 | `""` / 全空白 / 无关键词的普通报错（`error[E0308]`）→ `Vec::new()`；仅关键词无路径（`failed to write ` / `failed to remove: `）→ 空（非空判据挡住） | 表驱动（4 形态） |
| `locked_paths` — 超长与特殊字符诊断 | 边界 | >1000 字符诊断（前后大段噪声，中段含路径）→ 仍解析；路径含空格 / emoji / 中文 / `\0` → 原样保留、不 panic | 表驱动（3 形态） |
| `lock_holders` — 自锁自证（AC-3/D8） | 正向 | `#[cfg(windows)]`：临时文件由本进程以「读权限 + `share = READ`（无 delete）」打开 → `lock_holders(&[path])` 含本进程（`name` 与当前可执行名一致、`pid == std::process::id()`），按 `(name, pid)` 去重后恰 1 条 | 单值 |
| `lock_holders` — 释放后空清单（AC-3） | 正向 | 关闭上述句柄后再次调用 → `Vec::new()`（「未定位占用者」由 `attribution_text` 装配），两次调用均不 panic | 单值 |
| `lock_holders` — 多路径与部分存在 | 边界 | 两路径：其一被本进程无 delete 共享持有、其二不存在 → 只归因存在且阻塞者（不存在路径不 panic、不错误点名） | 单值 |
| `lock_holders` — 空入参与不存在路径 | 异常 | `lock_holders(&[])` → 空；`lock_holders(&[不存在路径])` → 空（`NotFound` 静默）；`RmGetList` 空清单路径同型（R-3） | 表驱动（3 形态） |
| `lock_holders` — 非 Windows `lsof` 缺失（AC-4 降级） | 异常 | `#[cfg(not(windows))]` PATH 隔离使 `lsof` 不在 PATH → `Vec::new()`，不 `Err`、不 panic、不阻断调用方（spec「MUST NOT 阻断主流程」） | 单值 |
| `blocks_delete` — 全共享读句柄不误报（AC-4） | 正向 | `#[cfg(windows)]` 另一句柄以「读权限 + `share = READ \| WRITE \| DELETE`」打开 → `blocks_delete(path) == false`（旧「独占打开探测」把正常读句柄误报成锁的形态不复现） | 单值 |
| `blocks_delete` — 无 delete 共享即真阻塞（AC-4） | 正向 | 另一句柄以「读权限 + `share = READ`」打开 → `blocks_delete(path) == true`（`raw_os_error == 32` 判据，与 rustc / cargo 失败同条件） | 单值 |
| `blocks_delete` — 不存在 / 无持有者 | 边界 | 不存在路径 → `false`（`NotFound`）；无任何持有者的真实文件 → `false`（打开成功） | 表驱动（2 形态） |
| `blocks_delete` — 只读不扰动自证（AC-4/R-4） | 边界 | 对真实临时文件调用判据前后：文件长度与字节内容不变、无 `create` / `truncate` 痕迹（`metadata.len()` 与原文对比）；调用返回后立即可 `rename` / 删除成功（零残留句柄）；探针自身零写 | 单值 |
| `blocks_delete` — 非 Windows 恒 false | 边界 | `#[cfg(not(windows))]` 任意路径 → `false`（POSIX 无强制共享语义，探针不适用） | 单值 |
| `attribution_text` — 点名 + 被锁文件（AC-3） | 正向 | 诊断含真实临时路径且该路径被本进程无 delete 共享持有 → 返回 `Some`，文本含「占用进程：」+ 本进程名 + `(pid)` 与「被锁文件：」+ 该路径（三行原料，不含重试次数 / 指引） | 单值 |
| `attribution_text` — 空清单降级（AC-3/R-3） | 正向 | 诊断含不存在路径（探针判「未阻塞」/ 点名空）→ 退化为对全部解析路径点名 → 文本含「未定位占用者（可能为内核过滤驱动（安全软件），无法归因到用户进程）」+ 被锁文件清单；MUST NOT 判「无锁」、不返回 `None`（路径可解析即产原料） | 单值 |
| `attribution_text` — 无路径可解析 | 异常 | 空串 / 无关键词诊断 → `None`（调用方据此只装配摘要与指引，不拼空段） | 表驱动（2 形态） |
| `attribution_text` — 占用者可读性 | 边界 | 多条阻塞路径（同一持有者 + 不同持有者）→ 每个 `(name, pid)` 各一行、去重后无重复；超长诊断（>1000 字符）/ 特殊字符路径 → 文本逐字承载，不 panic | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件句柄（「另一进程持有」的等价面） | 不 mock 系统：以**本进程真实 `std::fs::OpenOptions`**（`cfg(windows)` 下 `share_mode` 两形态）扮演持有者——判据观测的就是真实 `ERROR_SHARING_VIOLATION`，替身无意义 | 判据三臂 / 点名 / 归因用例 |
| Restart Manager（系统 API） | 不 mock：`cfg(windows)` 真实调用（`RmStartSession` / `RmGetList` / `RmEndSession`），断言只取「含本进程 / 空清单」两类稳定形态；`RmGetList` 空与 RM 服务不可用的环境态见 `## 不可测试项` 第 10 项 | 点名用例 |
| `lsof`（非 Windows 外部命令） | 进程边界依赖：以 PATH 隔离（`std::env` 改写）模拟缺失，`#[cfg(not(windows))]` 门控——exec crate 无 `TEST_ENV_LOCK`（R-8 平台隔离，非 Windows 门禁才跑此臂），故用例内不自建全局锁竞争 | 非 Windows 降级臂 |
| 文件系统 | 不 mock：`tempfile` 真实临时文件 / 目录（持有者句柄、只读自证、不存在路径） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/exec/src/lib.rs -> packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`is_transient` / `RetryPolicy` / `LockHolder` / `locked_paths` / `lock_holders` / `attribution_text` 的「所在文件」列均为 `transient.rs` / `lockprobe.rs`）；待测项取自「修改文件」表 `lib.rs` 行 ①~④（模块 doc 两条能力项 / `mod transient; mod lockprobe;` / 六项再导出（含 `is_transient`，D19）/ `pub use run::{...}` 保持）+ AC-8「`kill_tree` / `ProcessOutput` / `ExecOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` 逐字零改」。测试模块以 `#[cfg(test)] #[path = "lib_test.rs"] mod lib_test;` 挂载（既有形态）。 -->

#### 待测功能

- `mod transient; mod lockprobe;` + 再导出面（修改文件行 ②③，D19）：`pub use transient::{is_transient, AttemptTrace, RetryPolicy};`、`pub use lockprobe::{attribution_text, lock_holders, locked_paths, LockHolder};`——六项经 crate 根可见且行为与各自模块一致（调用点从 `exec::` 根取的唯一形态，避免 checks 域重算签名表）
- 既有导出面零变（修改文件行 ④，AC-8）：`run_shell` / `RunError` / `RunOptions` / `RunOutcome` / `RunReport` / `SystemCommand` / `ExecOutput` / `record_exec_output` / `read_exec_output` / `read_exec_output_in` / `kill_tree` / `ProcessOutput` / `resolve_shell` 签名与语义逐字不变
- 模块 doc（修改文件行 ①）为文档文本，无运行期断言（结构 / 文本审查口径同 `## 不可测试项` 第 7 项）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 再导出 `is_transient` | 正向 | 经 `crate::is_transient(&report)`（而非 `transient::` 私有路径）调同一组输入 → 与 `transient_test` 判据逐值一致（导出的是同一函数，非包装） | 单值 |
| 再导出 `RetryPolicy` / `AttemptTrace` | 正向 | 经 crate 根构造 `RetryPolicy::none()` / `transient()`、`AttemptTrace { .. }` → 字段可读、三常量与类型表等值（`max_attempts` / `backoff` / `total_budget`） | 表驱动（2 类型） |
| 再导出 `locked_paths` / `lock_holders` / `attribution_text` / `LockHolder` | 正向 | 经 crate 根调 `locked_paths("failed to write /tmp/x")` 等纯函数与空入参 `lock_holders(&[])` → 与模块内行为一致（`LockHolder` 可在 crate 根构造并比较） | 表驱动（4 项） |
| 既有导出面零变（AC-8） | 边界 | 既有 `lib_test.rs` 用例（壳包装四形态 / `raw` 标记 / `into_std` 平台标志 / argv）全绿；`record_exec_output` → `read_exec_output` → `read_exec_output_in` 双锚回查与 `mint_exec_id` 形态校验（`is_valid_exec_id` 的路径逃逸防线）既有断言零改 | 单值 |
| `kill_tree` / `ProcessOutput` 零变（AC-8） | 边界 | `kill_tree(None)` 零副作用；`ProcessOutput::error_excerpt` / `tail` 既有输出口径（被 `environment::fault_text` 复用）逐字不变 | 表驱动（2 项） |
| 模块挂载（编译期） | 边界 | `mod transient;` / `mod lockprobe;` 在位即 `exec::` 根六项可用；缺失即编译失败（`checks-runtime` 的 `use exec::{is_transient, attribution_text, ..}` 不可解析） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（导出面） | 零 mock：纯构造 + 纯函数直调（`is_transient` / `locked_paths` / 空入参 `lock_holders`），零 spawn | 再导出用例 |
| 文件系统（记录面回归） | 不 mock：既有 `tempfile` 装置（`record_exec_output` / `read_exec_output` 真实落盘与回查） | 既有导出面回归用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/environment.rs -> packages/desktop/src-tauri/crates/infra/checks/src/environment_test.rs

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更——`attempts_log` / `fault_text` 属私有面（design 公共函数表下方注释明写「私有 / pub(crate) 不列」），`checks/src/lib.rs` 仅 `mod environment;`（无再导出）。待测项取自 design「新增文件」表该文件行（`attempts_log`（D11，单次零分隔头）、`fault_text`（D10，重试次数 + 末次诊断摘要 + 处置指引）、`ENV_GUIDANCE` 常量、诊断摘录上限常量）与 D10 / D11；测试模块以 `#[cfg(test)] #[path = "environment_test.rs"] mod environment_test;` 挂载于本模块内（design AC-10 回应「同模块 `#[cfg(test)]` 子模块直驱」，不为测试新增导出），本文件节与 `static_check_test` / `runner_test` 的组合断言互补（前者钉格式单点，后者钉装配结果）。 -->

#### 待测功能

- `attempts_log(report: &RunReport) -> Vec<u8>`（D11，AC-3/AC-8）：`attempts == 1 && retried.is_empty()` → **原样返回** `stdout ++ stderr`（零分隔头，逐字节同现状）；发生过重试 → `retried` 按序 + 末次各产一段 `===== attempt {n}/{max} · exit={exit_code} · transient={命中签名或 -} =====\n{该次合并流}`（`{max}` = `exec::RetryPolicy::transient().max_attempts`，即 5；该次合并流 = 该次 `stdout` 前缀 + 该次 `stderr` 后缀）
- `fault_text(attempts, exit_code, diagnostics: &[u8]) -> String`（D10，AC-3）：单点装配「已自动重试 {attempts - 1} 次（每次重试会跳过已完成部分）」+ 末次诊断摘要（经 `exec::ProcessOutput::error_excerpt` / `tail`，不自造截断口径）+ `exec::lockprobe::attribution_text(末次诊断)` 的占用者 / 被锁文件原料 + `ENV_GUIDANCE`（排除 workspace **父目录**、说明逐 worktree 排除无效）；末次诊断字节由调用点给（static-check 传 `&run.stdout`、test-execution 传合并流；参数名以实现为准）
- `ENV_GUIDANCE` 常量与「诊断摘录上限」常量（新增文件表）：单点文案 / 上限，调用点零重复字面量
- 边界纪律：exec **不自带**日志格式（尝试分隔头 / 归因文案归调用方，exec-runtime spec），本模块是这些格式的唯一来源；`static_check.rs` 与 `testexec/runner.rs` 共用本模块（MUST NOT 两写）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `attempts_log` — 单次零分隔头（AC-8/D11） | 正向 | `attempts = 1`、`retried = []`、`stdout = b"out\n"`、`stderr = b"err\n"` → 返回恰为 `b"out\nerr\n"`（逐字节；不含 `===== attempt` 前缀，既有 record 断言不被破） | 单值 |
| `attempts_log` — 单次双空 | 边界 | `attempts = 1` + 双空输出 → 返回空 `Vec`（零字节，不产生空段头） | 单值 |
| `attempts_log` — 重试两段（AC-3） | 正向 | `retried = [{ exit_code: 1, transient: Some("os error 32"), stdout: b"a1", stderr: b"e1" }]` + 末次 `{ exit_code: 0, stdout: b"a2", stderr: b"e2" }`、`attempts = 2` → 两段按序：`===== attempt 1/5 · exit=1 · transient=os error 32 =====\na1e1` 与 `===== attempt 2/5 · exit=0 · transient=- =====\na2e2`（`max` 恒 5，末次段头 `transient=-`） | 单值 |
| `attempts_log` — 五段（耗尽面） | 正向 | `retried` 4 条（`exit_code = 1`、`transient = Some(...)`）+ 末次同 → 恰 5 段，序号 1..5 递增、每段前恰一个换行、段间无多余空行；总长度 == Σ(段头 + 该次双路字节) | 表驱动（5 段） |
| `attempts_log` — 段内合流顺序（AC-8） | 正向 | 任一段 `stdout = b"A"`、`stderr = b"B"` → 段内正文为 `b"AB"`（stdout 前缀 + stderr 后缀，不 `2>&1`、不交错重排） | 单值 |
| `attempts_log` — 未命中此前尝试 | 边界 | `retried` 中一条 `transient = None`（此前尝试未命中）→ 该段头渲染 `transient=-`（不写 `None` / 不省略字段） | 单值 |
| `attempts_log` — 特殊字节与非 UTF-8 | 边界 | 段头内的 `exit_code` 为 `-1` 哨兵 / `255`；正文含非 UTF-8（`0xFF 0xFE`）、`\0`、emoji → 正文逐字节保留（本函数不做 lossy），段头为 ASCII | 表驱动（3 形态） |
| `attempts_log` — 大输出不截断 | 边界 | 每次尝试正文 >64 KiB → 全量拼接，段头计数与段数不变（审计不丢上下文） | 单值 |
| `fault_text` — 重试次数与指引（AC-3） | 正向 | `attempts = 5`、`exit_code = 1`、`diagnostics` 含锁文本 → 含「已自动重试 4 次」+「每次重试会跳过已完成部分」+ `ENV_GUIDANCE`（含「排除 workspace 父目录」与「逐 worktree」无效说明） | 单值 |
| `fault_text` — 占用者原料嵌入 | 正向 | `diagnostics` 含 `failed to rename archive file: <tempdir>/liba.rlib` + `os error 32`（真实临时文件、无持有者）→ 文本含该路径（被锁文件）与「未定位占用者」降级说明（探针真实调用，空清单不阻断） | 单值 |
| `fault_text` — 末次诊断摘要 | 正向 | `diagnostics` 为多行（前置 `Compiling …` + 末行锁错误 + `os error 32`）→ 摘要取末次错误面（含签名行），不经本模块自造截断（沿用 `ProcessOutput` 口径） | 单值 |
| `fault_text` — 次数边界 | 边界 | `attempts = 1`（直构，生产仅在 `max_attempts = 1` 早停面出现）→ 「已自动重试 0 次」，不 panic、无空括号 / 负值 | 单值 |
| `fault_text` — 空输出 | 边界 | `diagnostics` 为空 `&[]` → 摘要段降级为空（不出现空段头 / 不 panic），次数与指引照常在案 | 单值 |
| `fault_text` — 超长与特殊字符 | 边界 | `diagnostics` >1000 字符（含 emoji / 引号 / 换行 / 非 UTF-8 字节）→ lossy 不 panic，摘要按上限截断、指引逐字 | 表驱动（3 形态） |
| 常量锚定（新增文件表） | 边界 | `ENV_GUIDANCE` 含「父目录」与「逐 worktree」两个关键短语（与 `TXT_BASELINE_*` 零共用句，AC-6 的原料面）；诊断摘录上限常量取值 >0 且在 `fault_text` 内单点消费 | 表驱动（2 常量） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（尝试面 / 文案面） | 零 mock：`RunReport` / `AttemptTrace` 以字面量直构（被测 API 的显式入参），`attempts_log` / `fault_text` 为纯函数（设计组件表「纯函数（零 io / 零 async）」） | `attempts_log` 全部用例、`fault_text` 的次数 / 摘要 / 空输出用例 |
| `exec::lockprobe::attribution_text` / `ProcessOutput` | 不 mock：真实组合（归因原料与摘要口径正是被测语义）；诊断路径取真实 `tempfile` 路径（不存在 / 存在且无持有者两档）使探针零阻塞返回，环境变量与全局状态零改写 | `fault_text` 占用者 / 摘要用例 |
| 文件系统 | 不 mock：`tempfile::TempDir` 只为提供真实（且未被锁）的诊断路径 | `fault_text` 占用者用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`ProcessStaticCheck::run` 为 trait impl、签名零改，`execute` 为私有）；待测项取自「修改文件」表 `static_check.rs` 行 ①~⑤（`retry = transient()` / 三臂 fault 装配 / record 换 `attempts_log` / `diagnostics` 与 `passed` 口径零变 / 无配置直过 `fault: None`）+「配置」表 retry 常量 + AC-3 / AC-8 与 checks-domain delta 的 fault 装配表。本节同时承载 AC-3 的组合锚（链路方向：`static_check::execute` → `exec::run_shell`（重试）→ `lockprobe::attribution_text` → `environment::fault_text` → `StaticCheckOutcome.fault`），按链路发起方挂靠本模块。 -->

#### 待测功能

- `ProcessStaticCheck::run(root, name, slot, cancel)`（trait impl，签名零改）：形参透传 `execute`（第四形参取消信号），槽位与报告目录口径零变
- ① `RunOptions.retry = exec::RetryPolicy::transient()`（修改文件行 ①）
- ② 三臂 fault 装配（修改文件行 ②，AC-3/AC-7）：`RunError` → 既有 `Err("static-check 命令{failure}")`；`TimedOut` → `fault = Some(Environment("static-check 命令超时（1200 秒）强制终止"))`；`Cancelled` → `fault = Some(Stopped)`；`Completed` → `fault = is_transient(&run).map(|_| Environment(environment::fault_text(run.attempts, run.exit_code, &run.stdout)))`；**三臂均 `Ok(ToolStepOutput::StaticCheck(..))`（不再 `Err`）**
- ③ record 换 `environment::attempts_log(&run)`（修改文件行 ③，AC-3/D11）：全部尝试拼进**同一** `.log`（重试面带 attempt 分隔头，单次尝试逐字节同现状）
- ④ `diagnostics = String::from_utf8_lossy(&run.stdout).trim_end()`（**不读** `stderr`）、`passed = run.exit_code == 0` 口径零变（修改文件行 ④，AC-8）
- ⑤ 无配置 / 空命令直过分支补 `fault: None`（修改文件行 ⑤，AC-8）
- 零改面（AC-8）：`ensure_program_resolvable` / `report_dir` 三分支 / `STATIC_CHECK_TIMEOUT`（1200 s）/ 命令串 `2>&1` / 交错流单流语义

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| **瞬态耗尽 → `fault = Environment`（AC-3 组合锚）** | 正向 | shim 每次都输出 `failed to rename archive file: <tempdir>/liba.rlib` + `os error 32` 并 `exit 1`（`transient()` → 5 次耗尽）→ `Ok`、`passed == false`、`fault == Some(Environment(text))`；`text` 含「已自动重试 4 次」、被锁文件路径、占用者点名或「未定位占用者」降级说明、`ENV_GUIDANCE`；`diagnostics` 为末次原文 | 单值 |
| **瞬态自愈转绿（AC-1）** | 正向 | shim 前 2 次锁文本 `exit 1`、第 3 次 `exit 0` → `passed == true`、`fault == None`、诊断为空；record `.log` 含 3 段（前两段带 `transient=os error 32`，末段 `transient=-`）；shim 尝试计数 == 3 | 单值 |
| **真红不产 fault（AC-8/R-1）** | 正向 | shim 输出 `error[E0308]: mismatched types` + `exit 1` → `passed == false`、`fault == None`、`attempts` 面为单次（shim 计数 == 1）、`diagnostics` 逐字为该输出（`trim_end` 后） | 单值 |
| **裸 `failed to write` 不产 fault（R-1）** | 边界 | shim 输出仅 `failed to write <path>` + `exit 1` → `fault == None`、单次尝试（不重试、不当环境错误） | 单值 |
| **超时 → `fault = Environment`（checks-domain/AC-7）** | 正向 | 自旋 shim + 虚拟时钟 `advance(STATIC_CHECK_TIMEOUT)` → `Ok`（**不 Err**）、`passed == false`、`fault == Some(Environment("static-check 命令超时（1200 秒）强制终止"))`、pid 消失；文案逐字锚定 1200 s | 单值 |
| **已置位 cancel → `fault = Stopped`（AC-7）** | 正向 | `cancel` 先 `handle().set()` 再 `run` → `Ok`（不 Err）、`fault == Some(Stopped)`、`passed == false`、shim 零执行（标记文件不存在、`exec-outputs` 零新文件） | 单值 |
| **运行中 cancel → `fault = Stopped`（AC-7）** | 正向 | shim 落 pid 后自旋，运行中置位 → `Ok`、`fault == Some(Stopped)`、pid 消失、墙钟秒级；record 面为「单次尝试原样部分字节」（`attempts == 1` → 零分隔头，既有「取消不落输出引用」断言随裁决改写为「`output_ref` 回填且 `.log` == 已排水部分字节」） | 单值 |
| **无配置 / 空命令直过（AC-8）** | 边界 | `static_analysis` 缺席 / 空白串 → `passed = true`、`diagnostics` 空、`fault == None`、`output_ref = None`、`report_dir` 回填本槽位锚点、零 spawn 零探针 | 表驱动（2 形态） |
| **无配置 + 已置位 cancel** | 边界 | 取消已置位且无配置 → 仍走「无配置直过」（分支优先级不变）：`fault == None`、零 spawn、不 `Err` | 单值 |
| **交错流单流语义（AC-8）** | 正向 | shim 交替向 stdout / stderr 打行（命令串仍带 `2>&1`）→ `diagnostics` 逐字等于交错顺序全文、`run.stderr` 恒空（不读）；`.log`（单次尝试 → 零分隔头）与 `diagnostics` 同源同文 | 单值 |
| **退出码 → `passed` 口径（AC-8）** | 正向 | `exit 0` → `passed = true`；`exit 1` / `exit 2` → `passed = false` 且诊断保留（既有锚回归，`passed` 未被 fault 面改口径） | 参数化（3 值） |
| **程序不可达 `Err` 零变（AC-8）** | 异常 | 配置命令裸名 / 相对路径不存在 → `ensure_program_resolvable` 前置 `Err`（文案骨架不变）、不产 outcome、零 spawn（`RunError::Spawn` 臂因此不可达，见 `## 不可测试项` 第 11 项） | 表驱动（2 形态） |
| **`fault` 与 `passed` 正交（D12）** | 边界 | shim 在 `exit 0` 的同时输出签名文本（如用例自打印 `os error 32`）→ `passed == true`、`fault == None`（绿不产 fault）；`passed` 恒 `exit_code == 0` 口径未被 fault 面改动 | 单值 |
| **诊断文本边界** | 边界 | 输出为空 / 超长（>1000 字符）/ 含引号、换行、emoji、`\0` / 非 UTF-8 字节 → `diagnostics` lossy + `trim_end` 逐字、不 panic；无签名时 `fault == None` | 表驱动（4 形态） |
| **`cwd = root` 与槽位口径** | 边界 | 命令以相对路径读 `root` 下文件 → 回显等值；`slot = Baseline` → `report_dir` / `exec-outputs` 落 baseline 树（既有装置，零改面回归） | 表驱动（2 形态） |
| **`STATIC_CHECK_TIMEOUT` 锚定** | 边界 | `assert_eq!(STATIC_CHECK_TIMEOUT, Duration::from_secs(1200))`（预算常量单点，超时文案 1200 s 与之同源） | 单值 |
| **`attempts` 段头数 == 重试次数 + 1（AC-3）** | 边界 | 瞬态耗尽面 record 文本按 `===== attempt ` 计数 == 5、序号 1..5；自愈面 == 3；真红面 == 0（单次零分隔头） | 表驱动（3 形态） |
| **`never()` 既有用例回归锚（AC-8）** | 边界 | 既有 20 余例（通过 / 失败 / 无配置 / 落盘降级 / 重复 run / 槽位）补 `fault` 断言后全绿；`output_ref` / `exec_id` / 报告树路径零变 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（配置的 static-check 命令） | 不 mock spawn：既有 `write_shim` / `PathPrefix` 装置真实落盘平台 shim（`.cmd` / bash：交错行 / 非零退出 / 锁文本重试分支 / 自旋），经 PATH 裸名真实拉起 | 全部 spawn 用例 |
| 全局变量 `PATH` | `crate::TEST_ENV_LOCK` + `PathPrefix` RAII（备份 / 恢复）串行化（既有装置）；重试形态由 shim 读尝试计数文件分支，不改 PATH 语义 | 全部 spawn 用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `tokio::time::advance(STATIC_CHECK_TIMEOUT)`（超时臂不真等 1200 s）；重试退避（1/2/4/8 s）同以虚拟时钟推进（flaky 用例不真等 3 s） | 超时 / flaky / 耗尽用例 |
| 文件系统 | 不 mock：`TempWs` 真实 workspace 根（config、root 相对文件、报告树、`exec-outputs` 落盘、shim 尝试计数与 pid 文件） | 全部用例 |
| 内部模块（`exec::run_shell` / `exec::is_transient` / `environment::*` / `lockprobe::attribution_text` / `record_exec_output` / `config::load` / `layout`） | 不 mock：真实组合（本变更的被测语义正是「重试 → 归因 → fault → 同一 `.log`」整链） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`execute_plan` 为 `pub(crate)`、design 明写不入表）；待测项取自「修改文件」表 `runner.rs` 行 ①~⑥ +「类型定义」表 `PlanExecution` 行（增 `fault: Option<RunFault>`）+ test-execution delta 的「超时入 fault / 瞬态耗尽入 fault / 取消返回 fault 而非 Err / 合并流与工件解析不受影响」。既有超时 / 取消用例的旧断言（「超时落 `execution_error`」「取消返回 `Err`」）随裁决作废并改写。 -->

#### 待测功能

- `execute_plan(plan, report_dir, cancel)`（修改文件行 ①~⑥，AC-1/AC-7/AC-8）：`RunOptions` 增 `retry: exec::RetryPolicy::transient()`；结局按 fault 装配表映射
- `PlanExecution.fault: Option<RunFault>`（类型定义表，AC-5）：`pub` 字段；`error` 语义收窄为「被测代码 / 工件 / harness 面」，env 面恒 `None`
- ③ `Cancelled`（修改文件行 ③，AC-7）：**不再 `Err`**，返回 `PlanExecution { exit_code: EXIT_CODE_NO_STATUS, output: attempts_log(&run), fault: Some(Stopped), cases: vec![], source_files: vec![], measured: None, error: None }`，跳过工件解析
- ④ `TimedOut`（修改文件行 ④，D13）：同上但 `fault = Some(Environment("测试命令超时（1200 秒）强制终止"))`，**不落** `execution_error`
- ⑤ `Completed`（修改文件行 ⑤，AC-3/AC-8）：`exit_code = run.exit_code`；`fault = is_transient(&run).map(|_| Environment(fault_text(..)))`；`exec_error = if exit_code != 0 && fault.is_none() { Some(non_zero_exit_error(..)) } else { None }`；`output = attempts_log(&run)`；fault 在位 → 直接返回（不解析工件）
- ⑥ 合并流顺序零变（AC-8）：`PlanExecution.output` 仍「stdout 前缀 + stderr 后缀」，MUST NOT 加 `2>&1`；模板 / 工件重定向 / `parse_*` 零改
- 零改面：`SUITE_COMMAND_TIMEOUT`（1200 s）/ `expand_template` / `non_zero_exit_error` / `EXIT_CODE_NO_STATUS`（`-1`）/ 程序不可达前置 `Err`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| **真红零变（AC-8）** | 正向 | 预置工件 + `exit 1` + 输出无瞬态签名 → `fault == None`、`error == Some(non_zero_exit_error(..))`、`attempts` 面单次（shim 计数 == 1）、用例行解析链与既有断言逐字不变 | 单值 |
| **瞬态耗尽 → fault 且零 `execution_error`（AC-1/AC-3）** | 正向 | shim 5 次均输出 `failed to rename archive file` + `os error 32` 并 `exit 1`（`transient()` 耗尽）→ `fault == Some(Environment(text))`（含点名 / 被锁文件 / 已自动重试 4 次 / 指引）、`error == None`、`exit_code == 1`、`output` 含 5 段 attempt 分隔头（序号 1/5..5/5） | 单值 |
| **瞬态自愈 → 正常解析（AC-1）** | 正向 | shim 前 2 次锁文本、第 3 次 `exit 0` 且产出工件 → `fault == None`、`error == None`、用例 / 覆盖解析产出与单次绿跑一致、`output` 含 3 段分隔头 | 单值 |
| **超时 → fault，不落 execution_error（AC-7/D13）** | 正向 | 自旋 shim + 虚拟时钟 `advance(SUITE_COMMAND_TIMEOUT)` → `fault == Some(Environment("测试命令超时（1200 秒）强制终止"))`、`exit_code == EXIT_CODE_NO_STATUS (-1)`、`error == None`、不因「工件缺失」再产解析错误（fault 在位跳过解析） | 单值 |
| **取消 → fault = Stopped（AC-7）** | 正向 | 运行中置位 cancel（shim 落 pid 后自旋）→ `Ok(PlanExecution)`（**不 Err**）、`fault == Some(Stopped)`、`error == None`、`cases` / `source_files` 空、`measured == None`、pid 消失、墙钟秒级 | 单值 |
| **已置位 cancel（AC-7）** | 正向 | `cancel` 先置位再 `execute_plan` → `fault == Some(Stopped)`、`exit_code == -1`、shim 零执行（标记文件不存在） | 单值 |
| **`fault` 跨 plan 前不消耗工件** | 边界 | fault 在位但报告目录存在上一次成功工件（陈旧）→ 返回载荷 `cases` 空、`measured == None`（不读陈旧工件，避免假绿与复用门污染） | 单值 |
| **`PlanExecution.error` 收窄（AC-8）** | 异常 | `exit 0` + 工件缺失 / 不可解析 → `error == Some(解析失败文案)`、`fault == None`（真环境面：工件 / harness，`conclusion = Error` 成因面收窄） | 表驱动（2 形态：缺失 / 空） |
| **退出码直继承（AC-8）** | 正向 | `exit` 谱 `0 / 1 / 101 / 255` + 工件在位且可解析 → `exit_code` 逐值透传、`fault == None`、`error` 仅在非零且无签名时产 | 参数化（4 值） |
| **合并流顺序零变（AC-8）** | 正向 | 模板把 stdout 重定向进结果工件、编译错误留 stderr → `output == stdout ++ stderr`（前缀 + 后缀）、不含 `2>&1` 交错；结果工件解析只读 stdout 面（既有断言全绿） | 单值 |
| **重试面段内合流顺序（AC-8/D11）** | 边界 | 每次尝试各写 stdout 与 stderr 不同文本后成功 → 每段正文按「该次 stdout + 该次 stderr」排列，段间按尝试序，末次正文即 `RunReport.stdout`/`stderr` 拼接 | 单值 |
| **裸 `failed to write` 不产 fault（R-1）** | 边界 | `exit 1` + 输出仅 `failed to write <path>`（工件不可解析）→ `fault == None`、`error == Some(..)`、单次尝试 | 单值 |
| **超长 / 空 / 非 UTF-8 输出** | 边界 | 输出为空 / >64 KiB / 非 UTF-8 原始字节 → `output` 逐字节全量（单次面零分隔头）、`fault` 判定按 lossy 后文本、不 panic | 表驱动（3 形态） |
| **`SUITE_COMMAND_TIMEOUT` 锚定** | 边界 | `assert_eq!(SUITE_COMMAND_TIMEOUT, Duration::from_secs(1200))`（预算不被重试面改动，超时文案 1200 s 同源） | 单值 |
| **程序不可达前置 `Err` 零变（AC-8）** | 异常 | 裸名 / 相对 cwd 程序不存在 → `Err`（文案骨架不变）、不产 `PlanExecution`、零 spawn | 表驱动（2 形态） |
| **既有用例回归锚（AC-8）** | 边界 | 既有 `runner_test` 全量（模板展开五占位符 / 双工件 / 多 `running` 块 / 覆盖缺失 null 感知 / jest 档零改 / 超时 / 合并流）在 `retry: transient()` 下全绿；无签名输出不触发重试 → 断言面与改造前逐字一致 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（suite 命令） | 不 mock spawn：既有 `preset` / shim 装置（预置工件源 + `SHIM_*` 环境变量 + 真实 `run_shell`）真实拉起；重试形态由 shim 读尝试计数分支（锁文本 / 成功 / 自旋） | 全部 spawn 用例 |
| 全局变量（`SHIM_RESULTS_SRC` / `SHIM_DEST` / `SHIM_MARKER` / `SHIM_EXIT` / `PATH`） | `TEST_ENV_LOCK` + 既有 `set_shim_env` / `clear_shim_env` / `EnvWindow` RAII 串行化（既有装置，新增 `SHIM_ATTEMPT_FILE` 同窗口管理） | 全部 spawn 用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `advance`（超时臂不真等 1200 s；重试退避随虚拟时钟） | 超时 / 重试用例 |
| 文件系统 | 不 mock：`TempDir` 真实 cwd / 工件 / 报告目录（复用门与 `results.txt` / `coverage-summary.json` 真实落盘） | 全部用例 |
| 内部模块（`testexec::detect::TestPlan` / `expand_template` / `checks::parser` / `exec::is_transient` / `environment::*`） | 不 mock：真实组合（解析链与合并流正是 AC-8 零改面） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`ProcessTestExecution::run` 为 trait impl、签名零改）；待测项取自「修改文件」表 `testexec/mod.rs` 行 ①~④（多 plan fold / `fault_outcome` / 复用门与既有 outcome 装配补 `fault: None` / 文档口径）+ D13 / D14 + AC-5。本节承载 AC-5 的组合锚（链路方向：`ProcessTestExecution::run` → 逐 plan `execute_plan` → fault fold → `fault_outcome`），按最上层调用方挂靠本模块；跨模块组合用例（`runner` 真 spawn → `mod` 聚合）一并落本表。 -->

#### 待测功能

- 多 plan fault fold（D14，AC-5）：`Some(Stopped)` → 记 `Stopped` 并 **`break`**（取消后 MUST NOT 继续跑余下 plan）；`Some(Environment)` → 记**首个** `Environment` 并**继续**跑余下 plan（`Stopped` 优先于 `Environment` 的聚合规则才有可观测面）；`None` → `sub_reports.push(build_sub_report(..))`
- `fault_outcome(..)`（私有助手，D13，AC-5）：`conclusion: Fail` 占位、四项计数 0、`findings_brief` / `findings_detail` 空串、`report_dir` 与 `exec_id = record_exec_output(..)` 照旧回填；**不写 summary / 不跑 `diagnose_findings` / 不落报告文件**（半套报告不进复用门）
- 复用门分支与既有 `outcome` 装配补 `fault: None`（修改文件行 ③，AC-8）：`is_reusable` / `prune_roots` / `newest_input_mtime_ms` 语义零变
- `conclusion = Error` 成因面收窄（修改文件行 ④ + D12）：仅「解析失败 / harness 崩 / 框架探测失败（及既有 `Err` 面）」
- 零改面（AC-8）：`findings_brief` / `findings_detail` / `summary` 落盘 / `exec_id` / 槽位派生 / 不支持框架 `Err` / 程序不可达 `Err` / 完整性校验

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| **单 plan `Environment` → fault 上传（AC-5 组合锚）** | 正向 | 单 plan suite 恒锁文本耗尽（真 `execute_plan` → 5 次重试）→ `TestExecutionOutcome.fault == Some(Environment(text))`、`conclusion == Fail`（占位）、`total/passed/failed/skipped` 全 0、`findings_brief` / `findings_detail` 空串、`exec_id` 非空、`report_dir` 为本槽位锚点 | 单值 |
| **不写 summary（AC-5/D13）** | 正向 | 同上 → `<report_dir>/summary.json` **不存在**；同输入再次 `run` → 复用门未命中（无 summary 可复用）而重跑（不把坏 plan 当权威永久复用） | 单值 |
| **多 plan：`Environment` 继续跑余下（D14）** | 正向 | 2 plan：plan1 恒锁耗尽（Environment）、plan2 正常绿（预置工件 + `exit 0`）→ plan2 的 shim 标记在案（继续执行）、最终 `fault == Some(Environment)`（首个）、`conclusion == Fail`、计数 0（fault 在位不装配半套计数） | 单值 |
| **多 plan：`Stopped` 早停（D14/AC-7）** | 正向 | 2 plan：plan1 运行中置位 cancel → plan2 零 spawn（标记文件不存在）、最终 `fault == Some(Stopped)`、不写 summary、不 `Err` | 单值 |
| **`Stopped` 优先于 `Environment`（D14）** | 边界 | plan1 以 Environment 收场、plan2 以 Stopped 收场（可编程序列注入）→ 最终 `fault == Some(Stopped)`（优先级规则可观测，非「首个胜出」） | 单值 |
| **三 plan 混合面** | 边界 | plan1 绿（`sub_report` 入列）、plan2 Environment、plan3 绿 → 继续跑完、`fault == Some(Environment)`、不写 summary；`sub_reports` 内部装配不泄漏到 outcome（计数仍 0） | 单值 |
| **真红零变（AC-8）** | 正向 | suite `exit 1` + 真实测试失败文本（无签名）→ `fault == None`、`conclusion == Fail`、`findings_brief` / `findings_detail` 非空、summary 落盘、既有升格链输入逐字不变 | 单值 |
| **解析失败 → `Error` + `fault == None`（AC-8）** | 异常 | suite `exit 0` 但结果工件缺失 / 不可解析 → `conclusion == Error`、`fault == None`（成因面收窄后仍归解析面）、既有 `execution_error` 归因链不变 | 表驱动（2 形态） |
| **既有 `Err` 面零变（AC-8）** | 异常 | 不支持框架 / `change` 未建档 / 完整性校验失败 / 程序不可达 → `Err`（文案与短路顺序不变），不产 `fault_outcome` | 表驱动（4 形态） |
| **复用门零 spawn 复用（AC-8）** | 边界 | 输入未变且有合法 summary → 零 spawn 复用，`fault == None`、`conclusion` 由既有装配面产出（复用门先于 spawn 判定，顺序零变） | 单值 |
| **`exec_id` / `report_dir` 回填（D13）** | 边界 | fault 面（Environment / Stopped）与正常面各一 → `report_dir` 恒为本槽位派生、`exec_id` 非空且 `.log` 可经 `read_exec_output` 回查（记录失败降级空串的既有纪律不变） | 表驱动（3 形态） |
| **`conclusion` 占位不参与门禁（D12）** | 边界 | fault 面直构 `TestExecutionOutcome { conclusion: Pass, fault: Some(Environment(..)), .. }` → outcome 原样携带（本层不做「fault + Pass」纠偏），三值面零扩展 | 单值 |
| **槽位与树隔离（AC-8）** | 边界 | `slot = Baseline` 的 fault 面 → 报告 / `exec-outputs` 落 baseline 树，test 树零混居（既有装置回归） | 单值 |
| **`never()` 既有用例回归锚（AC-8）** | 边界 | 既有 `mod_test` 全量（绿跑全链 / 复用门两臂 / `conclusion` 三值映射 / `findings` 装配 / record 幂等 / 槽位树）补 `fault: None` 断言后全绿 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（各 plan 的 suite 命令） | 不 mock spawn：既有 `ChainWs` + `SHIM_*` 装置真实拉起（可编程：复制预置工件 / 退出码 / 自旋 / 锁文本重试 / 标记文件），`TEST_ENV_LOCK` 串行化 | 全部 spawn 用例 |
| 「多 plan」的输入面 | 不 mock 探测：以 `openspec/config.json` 声明两条 `test.frameworks` 条目（真实 `detect_plans` 产出两 plan），装置与既有 `two_plan_ws` 同型 | 多 plan 用例 |
| 时钟 / 文件系统 | 虚拟时钟（`start_paused` + `advance`）用于超时 / 取消臂；`tempfile` 真实报告树（summary / 工件 / `exec-outputs`，复用门依赖真实 mtime / 内容） | 全部用例 |
| 内部模块（`runner::execute_plan` / `detect::detect_plans` / `checks::is_reusable` / `environment::attempts_log`） | 不 mock：真实组合（本层职责是聚合与「早停 / 继续」判定，替身会掩盖 fold 语义） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`ProcessMutation::run` 为 trait impl、签名零改）；待测项取自「修改文件」表 `mutation/mod.rs` 行 ①~③（`SegmentFailure` doc 改写 / 中断收束面按变体分派 / `skip_outcome`+pass+fail 补 `fault: None`）+「类型定义」表 `SegmentFailure` 行 + D17 + AC-7。既有取消用例（「取消 → `conclusion = Error` + 「突变执行已取消」文案」）随裁决**改写**为「取消 → `fault = Some(Stopped)` + `conclusion = Fail` 占位 + `feedback_text` 空」；旧「收敛 failed / `Error`」断言作废（R-7）。 -->

#### 待测功能

- `SegmentFailure::Cancelled` 映射（类型定义表 / 修改文件行 ②，AC-7/D17）：`Cancelled` → `MutationTestOutcome { conclusion: Fail, fault: Some(RunFault::Stopped), feedback_text: String::new(), score / threshold / counts / report_paths 同既有 partial 面, skip: None }`——**不再** `conclusion = Error` + 取消反馈文案；`feedback_text` 清空（stopped 面不被误读为「需修复」）
- `Interrupted`（含段超时）零变（修改文件行 ②，AC-8）：`conclusion = Error` + 「突变执行中断：…」反馈文案 + `fault = None`；段超时仍 `Interrupted`（产品语义，进 fault 会误导「环境坏」）
- `Fatal` 零变（AC-8）：`execute` 返回 `Err`（停给用户），取消面不走 `Fatal`
- `skip_outcome` / pass / fail 三处字面量补 `fault: None`（修改文件行 ③）
- 三段零重试（AC-2）：`cargo_mutants` / `stryker` 段 spawn 以 `RetryPolicy::none()` 语义执行（段内命中签名不追加尝试）
- 零改面（AC-8）：跳过面（`EmptyDiff` / `NoExecutor`）顺序与 evidence、逐 suite 多阈值、计数归并、段级复用门、失败桶临时面保留纪律（取消面同样保留）、`SEGMENT_SPAWN_TIMEOUT_MS`（3_600_000）与其文案

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| **取消 → `fault = Stopped`（AC-7 组合锚，旧断言改写）** | 正向 | 假 spawn 自旋（rust 桶），运行中 `handle().set()` → `Ok(MutationTestOutcome)`、`fault == Some(RunFault::Stopped)`、`conclusion == Fail`（占位）、`feedback_text` 为空串、`skip == None`、`score` / `threshold` / `counts` / `report_paths` 与既有 partial 面同形 | 单值 |
| **已置位 cancel 入口面** | 正向 | `cancel` 先置位再 `run` → 首段即 `Cancelled`（零 spawn）、`fault == Some(Stopped)`、后续桶不执行（段串行 break）、墙钟上界 | 单值 |
| **跳过面先于取消面** | 边界 | `files` 空清单（`EmptyDiff`）+ 已置位 cancel → 仍返回 `skip = Some(EmptyDiff)` + `fault == None`（跳过判定顺序零变，取消不伪造 `Stopped`） | 单值 |
| **`Interrupted` 零变（AC-8）** | 正向 | 段超时（虚拟时钟 `advance(SEGMENT_SPAWN_TIMEOUT_MS + …)`）→ `fault == None`、`conclusion == Error`、`feedback_text` 含「突变执行中断：」与「段执行超时（3600 秒）强制终止」（现值不动）且**不含**「已取消」 | 单值 |
| **`Fatal` 仍 `Err`（AC-8）** | 异常 | 段报告目录创建失败 / 工具不可达 → `Err`（`conclusion` / `fault` 不落位）；取消面不走 `Fatal` | 表驱动（2 形态） |
| **逐 suite 多阈值零变（AC-8）** | 正向 | 两 suite（80 / 70）全过 → `conclusion == Pass`、`fault == None`、`feedback_text` 空；一过一败 → `Fail` + 报告指引式文案（既有断言全绿） | 表驱动（2 形态） |
| **计数归并与 summary 回填零变（AC-8）** | 正向 | 归并块计数与 `summary` mutation 位回填 → 与改造前逐字段等值（`fault` 不参与 summary） | 单值 |
| **`fault = None` 字面量（修改文件行 ③）** | 边界 | `skip_outcome` / pass / fail / `EmptyDiff` 四形态产出经穷尽解构断言 `fault == None`（编译期随动 + 运行期锚） | 表驱动（4 形态） |
| **零重试（AC-2）** | 边界 | 段输出含 `os error 32` + 非零 → 假 spawn 记录次数 == 1、分流按既有面（`Interrupted`）、`fault == None`（签名命中不触发段重试） | 单值 |
| **取消后临时面保留（D17/AC-7）** | 边界 | Stryker 桶因取消中断 → `_stryker-tmp` 与临时 config 保留、`report_paths` 含已产路径（与 `Interrupted` 保留纪律同型）；成功桶照清 | 单值 |
| **取消后复用门与断点续跑（AC-8）** | 边界 | 取消后重入同一 change、输入未变 → 已完段走段级复用门（零 spawn），未完段重跑；段报告与归并块口径零变 | 单值 |
| **多桶中途取消** | 边界 | rust + stryker 双桶：第 1 桶完成 → 置位 → 第 2 桶 `Cancelled`、`fault == Some(Stopped)`、第 1 桶块不进 summary（中断面不回填，既有语义）、`report_paths` 含第 1 桶 | 单值 |
| **循环内 clone 共享原子** | 边界 | 双桶链路捕获的 flag `assert_eq!`（`cancel.clone()` 逐层下传同一原子） | 单值 |
| **常量锚定** | 边界 | `SEGMENT_SPAWN_TIMEOUT_MS == 3_600_000`（design 待决问题 2：本变更不动值不动文案，规格 1800 s 表述的偏差见 `## 不可测试项` 第 13 项） | 单值 |
| **`never()` 既有用例回归锚（AC-8）** | 边界 | 既有 `mod_test` 全量（归桶 / 复用门全链 / 跳过面 / 写盘失败 / 临时面清理 / 组合根链路）在 `fault: None` 随动后全绿 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（两段执行体） | 不 mock spawn：既有 `SpawnWindow` 假执行体装置（PATH 前置 `cargo.exe` / `npx.cmd` / unix shim；记录 argv / cwd / env；自旋 / 非零 / 输出可编程），`TEST_ENV_LOCK` 串行化 | 全部段执行用例 |
| 文件系统 | 不 mock：`tempfile` 真实报告树（`change_test_reports` 布局、sandbox / 临时 config / 段报告 / `summary.json` 回填） | 全部用例 |
| 时钟 | `#[tokio::test(start_paused = true)]` + 手动 `advance`（段超时臂不真等 3600 s；取消臂自旋 + 握手） | 超时 / 取消臂 |
| 内部模块（`bucket_suites` / `merge_mutation_blocks` / `report::*` / `checks` 解析器 / `cargo_mutants::run_segment` / `stryker::run_segment`） | 不 mock：真实组合（归并 / 阈值 / 复用门属 AC-8 零改面）；`available_parallelism` 不 mock（jobs 期望按真实 CPU 重算关系式，沿既有装置） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更——`run_segment` 为 `pub(crate)` 且 design 公共函数表下方注释明写「私有 / pub(crate) 不列」；本文件在「修改文件」表仅一行随动：`RunOptions` 增 `retry: exec::RetryPolicy::none()`（D17 零重试；proposal 未列，因 `retry` 为必填字段）。待测项取自该行 + AC-2 / AC-8。 -->

#### 待测功能

- `run_segment(patch_path, exec_dir, cancel)` 的 `RunOptions.retry = RetryPolicy::none()`（修改文件行，AC-2）：段命令**零重试**面——输出含瞬态签名且非零退出时 `attempts == 1`，分流仍为 `Interrupted`
- 零改面（AC-8）：`cwd = exec_dir` / `CARGO_TARGET_DIR` env / `SEGMENT_SPAWN_TIMEOUT_MS`（1800 s 取值不变）/ `TimedOut` → `Interrupted`（文案「cargo-mutants 段执行超时（3600 秒）强制终止」现值不动）/ `Cancelled` → `Cancelled` / `Spawn` → `Fatal` 文案 / 段报告权威 / 零突变体合成载荷

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 零重试（AC-2） | 正向 | 假 cargo 输出 `os error 32` + `exit 1` → 段尝试次数 == 1（记录文件仅一条）、`Err(SegmentFailure::Interrupted(_))`、`fault` 面不产生（段层无 fault） | 单值 |
| argv / cwd / env 零变（AC-8） | 正向 | 假 cargo 记录 argv / cwd / `CARGO_TARGET_DIR` → 与改造前逐字相等（`retry` 新增不改执行形状） | 单值 |
| 超时臂零变（AC-8） | 正向 | 自旋 shim + 虚拟时钟 `advance(SEGMENT_SPAWN_TIMEOUT_MS + …)` → `Interrupted`、文案 `contains("cargo-mutants 段执行超时（3600 秒）强制终止")`、pid 树回收（既有断言） | 单值 |
| 取消臂零变（AC-7 的段层前置） | 正向 | 运行中置位 → `Cancelled("cargo-mutants 段执行已取消")`（`SegmentFailure::Cancelled` 变体，交由 `mod.rs` 映射 `fault = Stopped`） | 单值 |
| 既有失败分流零变（AC-8） | 异常 | PATH 隔离（程序不可达）→ `Fatal` 文案逐字；非零无报告 → `Interrupted(segment_failure_message)` | 表驱动（2 形态） |
| 既有用例回归锚 | 边界 | 既有 `cargo_mutants_test` 全量（常量锚定 / 零突变体载荷 / 报告权威 / 合并流）全绿（`retry` 字段一字随动） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（`cargo mutants`） | 不 mock spawn：既有 `SpawnWindow`（PATH 前置假 `cargo.exe` / unix shim；记录 argv / cwd / env；可编程输出 / 自旋 / 非零 / ENOENT） | 全部 spawn 用例 |
| 全局变量 `PATH` / 假 spawn 记录文件 | `crate::TEST_ENV_LOCK` + `SpawnWindow` RAII（既有装置） | 全部 spawn 用例 |
| 时钟 / 文件系统 | 虚拟时钟（`start_paused` + `advance`）；`tempfile` 真实执行目录与段报告 | 超时 / 取消臂 |
| 内部模块（`exec::ProcessOutput` / `parse_cargo_mutants_seg` / `mutation_jobs`） | 不 mock：真实组合 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker_test.rs

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更——同 `cargo_mutants.rs`（`run_segment` 为 `pub(crate)` 不入表）；「修改文件」表仅一行随动：`RunOptions` 增 `retry: exec::RetryPolicy::none()`（D17 零重试）。待测项取自该行 + AC-2 / AC-8。 -->

#### 待测功能

- `run_segment(segment, cancel)` 的 `RunOptions.retry = RetryPolicy::none()`（修改文件行，AC-2）：js 族段零重试——签名命中不追加尝试
- 零改面（AC-8）：展开命令串 `npx --prefix "<suite_cwd>" stryker run "<config>"` 与 `cwd = suite_cwd`（`raw` 保真）/ 临时 config 写盘字段 / `SEGMENT_SPAWN_TIMEOUT_MS`（不变）/ `TimedOut` → `Interrupted`（「Stryker 段执行超时（3600 秒）强制终止」现值）/ `Cancelled` → `Cancelled` / `Spawn` → `Fatal` / 段报告权威 / sandbox 清理纪律

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 零重试（AC-2） | 正向 | 假 `npx` 输出 `resource busy or locked` + `exit 1` → 段尝试次数 == 1、`Err(SegmentFailure::Interrupted(_))` | 单值 |
| 命令串 / cwd 零变（AC-8） | 正向 | 假 `npx` 记录 → 展开命令串与 `cwd` 逐字不变（`retry` 不改执行形状） | 单值 |
| 临时 config 写盘零变（AC-8） | 正向 | 段执行前 `mutation_dir` 下临时 config 落盘且 `mutate` / `tempDirName` / `concurrency` 字段同改造前（取消臂同样先写盘再 spawn） | 单值 |
| 超时臂零变（AC-8） | 正向 | 自旋 shim + 虚拟时钟 `advance` → `Interrupted`、文案 `contains("Stryker 段执行超时（3600 秒）强制终止")`、pid 树回收 | 单值 |
| 取消臂零变（AC-7 的段层前置） | 正向 | 运行中置位 → `Cancelled("Stryker 段执行已取消")`、临时 config / sandbox 保留（取消不清理，交调用方保留纪律） | 单值 |
| 既有用例回归锚（AC-8） | 边界 | 既有 `stryker_test` 全量（`npx` 不可达 / 报告缺失或损坏 / 合并流 / 常量）全绿 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（`npx … stryker run …`） | 不 mock spawn：既有 `SpawnWindow`（PATH 前置假 `npx`；记录 argv / cwd；可编程输出 / 自旋 / 非零 / ENOENT，与 `cargo_mutants_test` 共享装置） | 全部 spawn 用例 |
| 全局变量 `PATH` / 假 spawn 记录文件 | `crate::TEST_ENV_LOCK` + `SpawnWindow` RAII | 全部 spawn 用例 |
| 时钟 / 文件系统 | 虚拟时钟；`tempfile` 真实 sandbox / 临时 config / 段报告 | 超时 / 取消臂 |
| 内部模块（`report::segment_report_path` / `build_config` / `to_posix` / `parse_stryker_seg`） | 不 mock：真实组合（写盘与解析属 AC-8 零改面） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（三 runner trait 方法签名零改、仅文档口径更新，design 明写不入表）；待测项取自「类型定义」表四行（`RunFault` 新增、三 outcome 各增 `fault`）+ 修改文件表 `port.rs` 行 ③（三 trait 取消 doc 改写为「返回 `fault = Some(RunFault::Stopped)`（MUST NOT 返回 `Err`）」）+ D18 / AC-9。「零 `Serialize`」「不出线」的编译期与生成面见 `## 不可测试项` 第 4 项。 -->

#### 待测功能

- `RunFault`（类型定义表，AC-5/D18）：`pub enum RunFault { Stopped, Environment(String) }`，`derive(Debug, Clone, PartialEq, Eq)`、**零 `Serialize`**（不出线）；`Stopped` = 用户停止、`Environment(String)` = 环境错误摘要
- `StaticCheckOutcome.fault`（类型定义表，AC-5）：`pub fault: Option<RunFault>`（`None` = 结论本身有效）；`passed` / `diagnostics` / `output_ref` / `report_dir` 取值面零变；`derive` 不变（`Eq` 仍成立）
- `TestExecutionOutcome.fault`（类型定义表，AC-5）：`pub fault: Option<RunFault>`；`conclusion` 三值面零变（D12 占位规则：fault 在位取 `Fail`）
- `MutationTestOutcome.fault`（类型定义表，AC-5）：`pub fault: Option<RunFault>`；仍零 `Serialize`；`derive` 保持 `PartialEq`（无 `Eq`——`f64` 字段）
- 三 runner trait 的取消**文档**口径（修改文件行 ③）：置位 → 进程树击杀 → 实现侧返回 `fault = Some(RunFault::Stopped)`（MUST NOT 返回 `Err`）；签名零改（不可运行期断言，见 `## 不可测试项` 第 5 项）
- 零改面（AC-9）：`ToolStepOutput` / `ToolCommand` / `ToolStepRequest` / 三 trait 形参与 `RunUpdate` / `RunNodeRecord` 零扩展

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `RunFault` — 变体与相等 | 正向 | `Stopped` / `Environment("x")` 两变体直构；同变体等值 `assert_eq!`、异变体 `assert_ne!`；`Environment` 载荷参与相等（不同串不等） | 遍历枚举（2 变体） |
| `RunFault` — clone 与 Debug | 正向 | `clone()` 后载荷逐字相等（非 `Arc` 共享）；`format!("{:?}")` 含 `Stopped` / `Environment` 与载荷串（derive 可读，便于故障定位） | 表驱动（2 变体） |
| `RunFault` — 载荷边界 | 边界 | `Environment("")` / 超长（>1000 字符）/ 特殊字符（换行、`\0`、emoji、中文）/ 非 ASCII 混排 → 构造与相等不 panic、载荷逐字保留 | 表驱动（4 形态） |
| `StaticCheckOutcome.fault` — 三态 | 正向 | 字面量 `fault: None` / `Some(Stopped)` / `Some(Environment("…"))` → 结构体可构造、`PartialEq` 区分三态；五字段穷尽解构（无 `..`；新增 `fault` 外四字段零变）证明字段面只增 `fault` | 表驱动（3 形态） |
| `StaticCheckOutcome` — `Eq` 仍成立 | 边界 | 同字段（含 `fault`）两份 `assert_eq!` + 放入 `HashSet` 需 `Eq` 的编译期面（`derive` 未退化为仅 `PartialEq`） | 单值 |
| `TestExecutionOutcome.fault` — 三态 | 正向 | `fault: None` / `Some(Stopped)` / `Some(Environment(..))` 三态各配 `conclusion = Pass / Fail / Error` → 结构可构造、相等按 `conclusion` + `fault` 联合判定；十字段穷尽解构证明零扩展 | 表驱动（3 形态） |
| `TestExecutionOutcome` — `conclusion` 三值面零变 | 边界 | `Pass` / `Fail` / `Error` 与 `fault = None` 组合 → `as_str()` 仍为 `pass` / `fail` / `error`（三值面零扩展、无新变体） | 遍历枚举（3 值） |
| `MutationTestOutcome.fault` — 三态与 `f64` 字段 | 正向 | `fault: None` / `Some(Stopped)` 直构 + `score` / `threshold` 取 `0.0` / `NaN` / `Inf` → 结构可构造、`PartialEq` 按既有 `f64` 语义（无 `Eq`）；八字段穷尽解构 | 表驱动（3 形态） |
| `MutationTestOutcome` — 零 `Serialize` 的结构面 | 边界 | 类型未实现 `Serialize` 的编译期面（`bindings.ts` 本就无此三型）→ 以「三 outcome 只经 `ToolStepOutput` 进程内流转」的相等 / clone 用例承接 | 单值 |
| `ToolStepOutput` — 封闭集零扩展 | 边界 | 十变体穷尽 `match`（无 `_` 臂）→ 编译期即证无新变体；三门禁变体携带 `fault` 后 `PartialEq` 语义不变 | 遍历枚举（10 变体） |
| `ToolStepRequest` / `ToolCommand` — 零改 | 边界 | 两类型字段集穷尽解构（`root` / `command` / `cancel`；各 `ToolCommand` 变体）→ 与改造前逐字一致（本变更不读 `fault` 于请求侧） | 表驱动（2 类型） |
| `fault` 不进既有序列化面 | 边界 | `RunUpdate` / `RunNodeRecord` 字段穷尽解构 → 无 `fault` / `RunFault` 字样（载荷不出线的进程内证明；生成面见 `## 不可测试项` 第 4 项） | 表驱动（2 类型） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：全部为类型构造 / 相等 / 穷尽解构（纯进程内语义），零 IO / 零 spawn / 零时钟 | 全部用例 |
| 假 runner 实现 | 不以本文件承载：trait 形参 / 返回 `fault` 的契约面由 `walker_test`（`FakeTools` 可编程序列）与 `checks` 三面 runner 用例覆盖，避免重复装置 | — |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

<!-- design.md「公共函数 / API」子表无该文件行（`settle_fault` 为私有，design 明写「私有 / pub(crate) 不列」）；待测项取自「修改文件」表 `walker.rs` 行 ①~④（两文案常量 / `settle_fault` 单点 / 五处前置分派 / 步函数 doc）+ D15 / D16 + AC-5 / AC-6 / AC-7。本节承载 AC-5 / AC-6 / AC-7 的组合锚（链路方向：`walk_run` → 步执行体 → `run_tool` → 假 runner 产出 `fault` → `settle_fault` → 节点 settle → run 收敛），按最上层调用方挂靠本入口模块。 -->

#### 待测功能

- `TXT_ENV_STATIC` / `TXT_ENV_TEST`（修改文件行 ①，AC-6/D16）：`"静态检查无法完成（环境错误）：\n{fault}\n报告目录 {dir}"` / `"测试执行无法完成（环境错误）：\n{fault}\n报告目录 {dir}"`；重试次数与「排除 workspace 父目录（逐 worktree 无效）」指引**经 `{fault}` 嵌入**（模板不再写第二份）；与 `TXT_BASELINE_*` 零共用句
- 私有 `settle_fault(...) -> Terminal`（修改文件行 ②，D15/AC-5）：`Stopped` → `emit_node(StepSettle::stopped(Some("用户请求停止")).with_output_ref(..))` + `Terminal::stopped("用户请求停止")`；`Environment(text)` → `message = template.replace("{fault}", text).replace("{dir}", report_dir)`（`template = None` 退化为 `text` 原样）+ `emit_node(StepSettle::failed(diagnose_brief(&message)))` + `Terminal::failed(message)`
- 五处 fault 前置分派（修改文件行 ③，AC-5/AC-6/AC-7）：`step_static_check`（不 +1 反馈计数、零 worker）、`step_static_check_terminal`（不落基线红文案）、`step_test_execution_escalate`（不代写 fail `phase_log`、不进决策分叉）、`step_test_execution_terminal`、`step_mutation_test_escalate`（`template = None`，仅 `Stopped` 面可达）——均在 `run_tool` 之后、既有 `passed` / `conclusion` 分支**之前**早退
- 零改面：`run_tool` / `check_cancel` / `StepSettle` / `Terminal` / `emit_node` / `drive` / 节点词汇（`ChangeRunStatus::Stopped` / `ChangeStepStatus::Stopped` 已在位）/ `step_mutation_test` 的 `Err` 面

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| **`Environment` → 零 worker 且 run failed（AC-5 组合锚）** | 正向 | `FakeTools` 给 `StaticCheckOutcome { passed: false, fault: Some(Environment(fault_text)), diagnostics: … }` → run 收敛 `ChangeRunStatus::Failed`；`FakeWorker` 请求计数 == 0（零 executor / 零 evaluator / 零 decision）；timeline 无 `phase-log`（不代写 fail phase_log）；无「静态检查未通过」修复 prompt | 单值 |
| **反馈计数不增长（AC-5）** | 正向 | 同上（`passed = false` + `fault`）→ `FakeTools` 的 static-check 调用次数 == 1（不因红而重跑反馈边 6 次）、节点 failed 恰一条终态行 | 单值 |
| **`Stopped` → 节点 stopped + run stopped（AC-7）** | 正向 | `fault: Some(Stopped)` + `passed = false` → run `ChangeRunStatus::Stopped`；节点 `ChangeStepStatus::Stopped`、`detail` 含「用户请求停止」、无 failed 节点；`ChangeRunStatus::Failed` 不出现 | 单值 |
| **`fault` 先于结论（D15 边界）** | 边界 | `fault: Some(Environment(..))` + `passed = true`（理论上矛盾的组合）→ 仍 `Terminal::failed`（fault 优先于 `passed`，不因绿放行）；`fault: Some(Stopped)` + `conclusion = Pass` → 仍 `stopped` | 表驱动（2 形态） |
| **基线 static-check 环境错误（AC-6 文案）** | 正向 | `step_static_check_terminal` 收到 `fault: Some(Environment(text))` → run `Failed`；终态 reason 为 `TXT_ENV_STATIC` 替换后文本：含「静态检查无法完成（环境错误）」+ `text` + 报告目录；**不含**「请在基线上修复后重新发起」 | 单值 |
| **基线 static-check 真红对照（AC-6 可辨）** | 正向 | 同一步 `fault: None` + `passed = false` → 终态 reason 含 `TXT_BASELINE_STATIC_FAILED`（「基线静态检查未通过，请在基线上修复后重新发起」+ 报告目录）；与上一条断言两组文本零共用句（逐串 `contains` 互斥） | 单值 |
| **基线 test-execution 环境错误（AC-6）** | 正向 | `step_test_execution_terminal` 收到 `fault: Some(Environment(text))` → run `Failed`；reason 为 `TXT_ENV_TEST` 替换后文本（含「测试执行无法完成（环境错误）」+ `text` + 报告目录），不含 `TXT_BASELINE_TEST_ERROR` / `TXT_BASELINE_TEST_FAILED` 的「请检查配置与环境后重新发起」/「请在基线上修复测试后重新发起」 | 单值 |
| **基线 test-execution 真红对照（AC-6）** | 正向 | 同一步 `fault: None` + `conclusion = Fail` / `Error` → 分别落 `TXT_BASELINE_TEST_FAILED` / `TXT_BASELINE_TEST_ERROR`（既有三态分支零变） | 表驱动（2 形态） |
| **实现相位 test-execution 环境错误 → 零决策（AC-5）** | 正向 | `step_test_execution_escalate` 收到 `fault: Some(Environment(..))` → run `Failed`、零 WorkerAgent 请求（不交决策裁决）、零 `phase_log` 代写、终态 reason 为 `TXT_ENV_TEST` 文本 | 单值 |
| **实现相位 test-execution `Stopped`（AC-7）** | 正向 | 同上但 `fault: Some(Stopped)` → run `Stopped`、节点 stopped、零 worker | 单值 |
| **mutation `Stopped` 分派（AC-7）** | 正向 | `step_mutation_test_escalate` 收到 `MutationTestOutcome { fault: Some(Stopped), conclusion: Fail, feedback_text: "", .. }` → run `Stopped`、节点 stopped、零 worker；`feedback_text` 空不被渲染为「需修复」 | 单值 |
| **环境文案内容锚（AC-3/AC-6）** | 正向 | `text` = 模拟 `environment::fault_text` 产物（含「占用进程：HipsDaemon.exe (1234)」、「被锁文件：D:\w\target\…」、「已自动重试 4 次」、「排除 workspace 父目录（逐 worktree 无效）」）→ 终态 reason 逐项 `contains`；`{fault}` / `{dir}` 占位零残留 | 单值 |
| **文案 `{dir}` 替换** | 边界 | `report_dir` 为空串 / 含空格与中文 / 超长路径 → 模板替换后无 `{dir}` 残留，路径逐字（不转义、不截断） | 表驱动（3 形态） |
| **`Environment` 空串与超长（D15 边界）** | 边界 | `fault: Some(Environment(""))` → reason 含模板骨架、无空占位残留、不 panic；`Environment(>1000 字符)` → 终态 reason 逐字承载（节点 `detail` 经 `diagnose_brief` 摘要截断，既有口径） | 表驱动（2 形态） |
| **runner `Err` 不被误叙述（AC-8）** | 异常 | `FakeTools` 返回 `Err("boom")` → run `Failed`、reason 为「{label} 失败: boom」，不含 `TXT_ENV_*` 文案与「排除 workspace 父目录」（环境文案不误用） | 单值 |
| **真红走既有分支（AC-8）** | 正向 | `fault: None` + `passed = false`（实现相位 static-check）→ 反馈边计数 +1、修复 prompt「第 1/5 次」、≤5 次后升格 fail；与既有用例逐字一致 | 单值 |
| **三面取消一致性（AC-7）** | 边界 | 同一 run 场景分别让 static-check / test-execution / mutation 步收 `fault = Some(Stopped)` → 三面均节点 stopped + run `Stopped`、无 failed 节点、墙钟秒级 | 表驱动（3 面） |
| **`Environment` 后不再有后续步（AC-5）** | 边界 | 环境错误早退后 timeline 无后续 `phase-next` / `phase-start` / `decision` 调用（不升格、不分叉、不进重试）；与「相位 fail 升格」用例的 timeline 形态逐项对比 | 单值 |
| **既有 `never()` 全量回归锚（AC-8）** | 边界 | 既有 `walker_test` 全量（`FakeTools` 可编程序列 / 反馈边 / 升格 / 基线三态 / 停止 / 归档各相位）在三 outcome 补 `fault: None` 后全绿；`run_tool` / `check_cancel` / `StepSettle` / `Terminal` 语义零变 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ToolStepPort`（`FakeTools`） | 假件注入（`walk_run` 的显式入参依赖）：`static_check` / `test_execution` / `mutation` 三条可编程产出队列直给 `fault` 三态；`timeline` 记录调用顺序（`phase-next` / `phase-log` / 门禁步），`cancels` 记录收到的 flag | 全部 fault 分派用例 |
| `WorkerAgentPort`（`FakeWorker`） | 假件注入：`requests: Mutex<Vec<WorkerTurnRequest>>` 作为「零 worker 调用」的计数与角色断言面（`Executor` / `Evaluator` / `Decision` 逐角色 0 条） | AC-5 / AC-6 用例 |
| `RunHistoryPort`（`RecordingHistory`） | 真实记录假件（既有装置）：捕获 `RunFinishCommand.reason` 供文案断言（`TXT_ENV_*` / `TXT_BASELINE_*`） | 文案用例 |
| `ChangeFlowControl` / `RunGuard` | 不 mock：真实实例（停止语义 / 取消 flag 的写读同源） | 取消 / 常态用例 |
| `WorkflowSnapshotPort` / `ArchiveVcsPort` | 既有 `StubSnapshot` / `FakeVcs` 假件（显式注入依赖），本变更零改 | 全量回归锚 |
| 子进程 / 文件系统 | 不引入也不 mock：`fault` 由假 runner 直接给出（环境面 spawn / 探针 / 重试已由 `run_test` / `static_check_test` / `runner_test` / `lockprobe_test` 覆盖）——最小 mock 原则：walker 层不重复进程面 | 全部用例 |

---

## 不可测试项

| 项 | 原因 | 承接方式 |
|---|---|---|
| 1. `packages/desktop/package.json` 版本 `0.5.24` → `0.5.25` | 数据面（JSON 单字段），无运行时测试路径 | 版本 diff 复核（AC-11）；`tauri.conf.json` 经 `../package.json` 跟随的静态确认 |
| 2. `packages/desktop/src-tauri/crates/infra/exec/Cargo.toml`（`[target.'cfg(windows)'.dependencies] windows-sys`，仅 `Win32_System_RestartManager`） | 构建配置，无单元测试断言面 | `Cargo.toml` 逐项注释审查 + `cargo tree --target x86_64-unknown-linux-gnu -p exec`（非 Windows 零新增，AC-11 / R-5）+ `Cargo.lock` 无第二版本行（D8 / 待决问题 8） |
| 3. `packages/desktop/src-tauri/crates/infra/checks/src/lib.rs`（`mod environment;` 单行） | 仅私有模块挂载，design「公共函数 / API」子表无该文件行；checks crate 无 `lib_test.rs` 先例（测试以各模块同级 `*_test.rs` 注册） | 行为面由 `environment.rs -> environment_test.rs` 全覆盖；挂载正确性即编译期（`mod` 缺失则 `static_check.rs` 无法引用） |
| 4. `RunFault` 零 `Serialize` / 不出线 / `bindings:check` 零 diff（AC-9） | 生成面与编译期属性，运行时不可断言 | `pnpm -C packages/desktop run bindings:check`（零 diff）+ `grep -n "RunFault" packages/desktop/src/types/generated/bindings.ts`（零命中）+ `grep -rn "RunFault" crates/core/orchestration/src/state.rs`（零命中）+ 编译期 `Serialize` 未实现即证 |
| 5. 约 71 处三 outcome 字面量补 `fault: None`（含 `steps_test.rs` / `vcs/src/lib_test.rs` 的机械随动）与三 trait doc 口径改写 | 编译期一致性与文档文本，无独立运行时断言 | `cargo test --no-run` / `cargo clippy --all-targets`（编译期即证）+ `grep -rn '"执行已取消"' crates/`（零命中，杜绝第二套口径，R-7；mutation 的「段执行已取消」是 `SegmentFailure` 载荷、非 trait 取消口径，不在此列） |
| 6. AC-4 的「探针源码零 `create` / `truncate` / `write` / 独占 `share_mode`」 | 源码文本面（实现形态审查） | 源码审查 + `grep -n "share_mode\|access_mode\|create(\|truncate(" crates/infra/exec/src/lockprobe.rs`（只应出现「全共享 + DELETE/READ_ATTRIBUTES」一处与零 `create` / `truncate`）；行为面由 `lockprobe_test` 三臂与「只读不扰动自证」钉住 |
| 7. 「签名字典扩展 SHALL 经本模块一处修改」（调用点零特征串） | 结构面，非运行时 | 源码审查 + `grep -rn "os error 32\|LNK1104\|EBUSY" crates/infra/checks/src crates/core/`（零命中——特征串只在 `transient.rs` 一处） |
| 8. AC-10 的「全部既有测试通过」与覆盖率阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90） | 门禁统计面（由桌面端静态脚本 / test-execution 门禁执行，agent 内不重复执行） | `packages/desktop` 门禁（`pnpm run check` / 测试执行 + 覆盖率报告）；新模块（`transient` / `lockprobe` / `environment`）按 R-8 先行全覆盖后再计入 |
| 9. AC-11 的 `plugins/**` 零改动零 bump | 数据 / 目录面 | `git diff --stat plugins/`（零条目）+ 版本号 grep 复核 |
| 10. `RmGetList` 空清单 / Restart Manager 服务不可用的真实环境态（R-3） | 系统服务状态不可稳定构造（需停服务 / 域策略） | 以「不存在路径 → 空 `Vec`」「`attribution_text` 空清单降级文案」承载可构造面；其余由代码审查（任一步失败 / 空清单 → `vec![]`，静默降级不阻断）承接 |
| 11. `static_check.rs` 的 `RunError::Spawn` / `RunError::Wait` 臂 | `ensure_program_resolvable` 前置已挡住裸名 / 相对路径不可达；`child.wait()` io 失败跨平台不可稳定构造 | `RunError` 的 `Display` 前缀由 `run_test` 既有变体直构用例锚定；调用点拼法（`format!("static-check 命令{failure}")`）由代码审查承接 |
| 12. `RunReport.attempts == 0`（类型不变量外状态） | 生产路径恒 ≥ 1，公开 API 不可产生（`run_shell` 每条路径先自增）；直构该值只测防御性分支，无验收锚 | 全路径 `attempts >= 1` 断言（`run_test`）承接不变量；`environment_test` 的 `attempts == 1` 边界覆盖最小合法值 |
| 13. `SEGMENT_SPAWN_TIMEOUT_MS` 与段超时文案的规格偏差（design 待决问题 2：spec 写 1800 s，工作树实值 3600 s） | 规格文本面（本变更不动值不动文案） | 既有常量锚定（`== 3_600_000`）与文案 `contains("（3600 秒）")` 用例承接现值；spec 相位回写建议见 design「待决问题」2 |
| 14. 非 Windows 平台臂（`lockprobe` 探针恒 `false` / `lsof` 降级 / unix 子孙回收） | 当前门禁仅 Windows；跨平台行为不可在同一次运行内断言 | `#[cfg(not(windows))]` 编译隔离（`lockprobe_test` 只留降级 / 恒 `false` 臂）+ unix 臂由 CI 平台补齐时启用；`kill_tree` unix 仅杀直接子进程属既有风险（R-4）不改 |
| 15. 重试 / 预算参数不入 `openspec/config.json`（零 schema 扩展） | 数据面 | `git diff openspec/config.json`（零条目）+ 常量锚定（`transient_test` 三常量）+ 配置表审查 |

