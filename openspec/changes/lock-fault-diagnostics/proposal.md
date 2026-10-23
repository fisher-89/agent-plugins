# 提案: lock-fault-diagnostics

> **变更**: lock-fault-diagnostics
> **日期**: 2026-10-22
> **状态**: draft
> **来源**: `explore.md`（requirement 类型，根因已实测确认、设计完成未实施）
> **前置依赖**: `exec-run-shell`（2026-10-21 已归档合入）——`exec::run_shell` 四能力执行体、`foundation::cancel`、取消贯通四 spawn 点均已在位；本篇只在其上追加「瞬态重试」「锁诊断」「执行面归因」三面，**不改三 runner port 签名**

---

## 问题

### 现象：构建产物被外部进程占用，产品把它呈现成「检查红」

实测现场（explore §1，`static-check-file-lock.md` §12 监视记录）：企业级安全软件的**内核 minifilter 驱动**持有 `target/` 下构建产物，导致 rustc / cargo 收尾的三个动词全部失败：

| 动词 | 现场报错 | 谁在做 |
|---|---|---|
| `rename` | `failed to rename archive file` | rustc 写 `.rlib` 收尾 |
| `write` | `failed to write …libpotential_utf-….rmeta` | rustc 写 `.rmeta` |
| `remove` | `failed to remove …*.rcgu.o` | cargo 增量清理 |

持有者：`HipsDaemon.exe` / `usysdiag.exe`（火绒**企业版** `ESEndpoint`，排除项由服务端策略下发）、`TBProtect.exe`（云枢 YunShu，`tbprotect64.sys` 内核驱动 + Sophos AV 引擎）、以及 `System [pid 4]`（上述驱动的内核态持有——**Restart Manager 无法归因到用户进程**）。三者共同点：都需要 **DELETE 共享语义**，cargo 的 build-dir 锁只互斥自家 cargo，对 minifilter 完全无效。

### 现状代码的五处缺口

| 缺口 | 代码位置 | 后果 |
|---|---|---|
| 诊断无归因——只有 rustc 原始报错，没有「谁占着」「重试过几次」「该找谁」 | `crates/infra/checks/src/static_check.rs:102`（`diagnostics = String::from_utf8_lossy(&run.stdout)`） | 用户与 agent 都读不出「环境坏」，只能对着 `os error 32` 猜 |
| 环境错误被当成检查红 | `static_check.rs:103`（`passed = run.exit_code == 0`）→ walker `step_static_check` 定向反馈边 | 5 次反馈预算被烧在「修不存在的代码问题」上，agent 反复空转 |
| 超时 / 取消与结论混在同一根管道 | `static_check.rs:88-97`（`TimedOut` / `Cancelled` → `Err`）、`testexec/runner.rs:103-130`（`TimedOut` → plan `execution_error` → `conclusion=error`）、`mutation/mod.rs:230-250`（`Cancelled` → `conclusion=Error`） | 「环境坏」与「检查红」在 walker 侧不可辨，文案一律引导「修基线 / 修代码」 |
| 瞬态锁冲突零重试 | `crates/infra/exec/src/run.rs`（`run_shell` 单次收口，`RunReport` 无尝试面） | 一次碰撞即红；而 cargo 增量进度本可在重试中自愈（已建成 crate 不重建，剩余待写文件递减） |
| 无「不扰动」的锁探针 | 无 | 旧式「独占打开探测」把正常读句柄误报成锁，且它自己独占打开 rustc 待 finalize 的文件，**可能反向制造 `os error 32`**——本篇 MUST NOT 引入该形态 |

### 环境侧才是根治（代码改不掉，但产品要指出来）

云枢（`TBProtect`）与火绒企业版（服务端策略）都需排除 workspace 父目录（`C:\Users\wps\.dev-team`，逐 worktree 排除无效——worktree 动态新建），建议一并排除 `.cargo` / pnpm 缓存。产品侧的价值因此收敛为三件：**诊断（一次定位）+ 重试（自愈）+ 语义（不误导）**。

## 提案

只改桌面端 `packages/desktop/src-tauri` 与版本号；CLI / 插件 / 前端零改动。六面：

1. **`infra/exec` 增瞬态签名与有限重试**（新模块 `transient.rs`）：签名表（Windows 文件锁 / MSVC 链接器 / POSIX·Node 三族）+ `is_transient(&RunReport)`（`outcome == Completed && exit_code != 0` **且**合并输出命中签名行，大小写不敏感）+ `RetryPolicy { max_attempts: 5, backoff: [1s, 2s, 4s, 8s], total_budget: 1500s }`（`none()` 与 `transient()` 两构造）。刻意**不收**裸 `failed to write`（太泛；rmeta 写失败已被同一行里的 `os error 32` / `另一个程序正在使用此文件` 覆盖）。`TimedOut` / `Cancelled` **永不重试**（超时重试会把 1200 s 变数小时；取消必须立即生效）。

2. **`run_shell` 接重试**（`run.rs`）：`RunOptions` 增 `retry: RetryPolicy`（缺省语义 = 单次，`none()`）；重试循环复用既有执行骨架（同一 `kill_on_drop` spawn + 双管道排水 + 三臂竞速），**每次重试同样观测 cancel**；总预算从首跑起算，每次重试尝试开始前判定（超预算即不再发起新尝试，由调用方按瞬态命中断言 fault）。`RunReport` 增 `attempts: u32`（≥1）与 `retried: Vec<AttemptTrace>`（此前各次尝试的退出码 / 命中签名 / 输出字节）；`stdout` / `stderr` 仍为**最后一次**尝试的输出——既有消费点（诊断 / 合并流 / 工件解析）语义零变。`RetryPolicy::none()` 或未命中签名时 `attempts == 1 && retried.is_empty()`，行为与现状逐字节等价。

3. **`infra/exec` 增精确锁探针与占用者点名**（新模块 `lockprobe.rs`）：
   - **判据只问「是否阻塞 rename / delete」**：以 `DELETE | FILE_READ_ATTRIBUTES` 访问权 + `FILE_SHARE_READ | WRITE | DELETE` 全共享打开一次即关（**不独占、不写、不扰动构建**）；打开成功 = 未阻塞，失败且 OS 错误码为 `32`（共享冲突） = 与 rustc 失败同条件。
   - **点名**：Windows Restart Manager（`RmStartSession → RmRegisterResources → RmGetList`）取 `RM_PROCESS_INFO[].strAppName` + `dwProcessId`，去重后输出 `进程名 (pid)`；非 Windows 降级 `lsof`（存在则用，不存在即跳过，不阻断主流程）；`RmGetList` 返空降级为「未定位占用者」。
   - **触发时机**：仅在「非零退出 + 命中瞬态签名」时调用，避免常态开销；被锁文件路径从三种错误串解析（`failed to rename archive file: <path>` / `failed to write <path>` / `failed to remove <path>`）。

4. **`infra/checks` 装配 fault 与诊断**：static-check 与 test-execution 开 `RetryPolicy::transient()`；把 `RunOutcome` / 重试耗尽 / 探针归因装配成 `fault`：

   | 来源 | fault |
   |---|---|
   | `TimedOut` | `Environment("…超时…")` |
   | `Cancelled` | `Stopped` |
   | `Completed` + 重试耗尽 + 命中签名 | `Environment(最后一次诊断摘要 + 占用者点名 + 已重试次数 + 环境侧处理指引)` |
   | `Completed` + 其余非零（真红） | `None`（走既有反馈边 / 升格） |
   | 无配置直过 | `None` |

   输出记录仍为单一 `.log`：全部尝试按序拼接，每段前加分隔头 `===== attempt N/5 · exit=C · transient=<命中签名> =====`（审计不丢上下文）。test-execution 的 `conclusion = Error` 成因面**收窄**为「结果工件解析失败 / harness 崩 / 框架探测失败」——超时与瞬态耗尽 SHALL 走 fault，且 MUST NOT 再落 plan 级 `execution_error`（那会被当作被测代码问题而产生假失败归因）。

5. **`core/orchestration` 增执行面归因 `RunFault` 与分派**（`port.rs` / `walker.rs`）：`RunFault::{Stopped, Environment(String)}`，三个 outcome（`StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome`）各加 `fault: Option<RunFault>`；`passed` / `conclusion` **保持原样不升三值**。walker 五个步执行体按「fault 先于结论」分派：

   ```
   step_static_check / step_static_check_terminal / step_test_execution /
   step_test_execution_terminal :  fault? → stopped | failed(env)  : 既有分支
   step_mutation_test           :  fault? → stopped                : 既有升格分支
   ```

   `Environment` **一律 `Terminal::failed`** 且**不升格相位 fail、不进决策重试、不注回 executor**（环境坏的相位重试必然再失败；省下反馈预算）；`Stopped` → `Terminal::stopped`（run 收敛 `stopped`）。终局文案新常量 `TXT_ENV_STATIC` / `TXT_ENV_TEST`，与基线检查红文案**可辨**（前者引导「请 IT 在火绒企业版 / 云枢中排除 workspace 父目录」，后者引导「修基线」）。

6. **mutation 面只改取消归因**：`SegmentFailure::Cancelled` → `fault = Stopped`（不再 `conclusion = Error` + 反馈文案）；**不接 retry**；段超时仍 `Interrupted` → `conclusion = Error`（长跑是产品语义，超时是配置问题，有意不进 fault）；`Fatal` 仍 `Err` 停给用户。

### 范围裁决（explore 未决项 / 实现口径的落地）

- **依赖面最小化**：探针的「DELETE 访问权 + 全共享打开」用 std `OpenOptionsExt`（`access_mode` / `share_mode`）表达，故 `windows-sys` 只为 Restart Manager 引入，feature 只开 `Win32_System_RestartManager`（不引 `Win32` 全量），且落 `[target.'cfg(windows)'.dependencies]`——非 Windows 目标零新增编译依赖。
- **探针不独占**：判据是「能否以 DELETE 权限 + 全共享打开」，因此正常读句柄（share 含 delete）不会被误报；探针自身也 MUST NOT 制造 `os error 32`。
- **尝试面落文口径**：全部尝试 SHALL 拼进**同一** `.log`（每段前加 attempt 分隔头），`output_ref` 指向该单一记录——取代 explore「`output_ref` 落最后一次」的粗口径，沿既有「单一 `.log` 记录」纪律（分离记录会让审计丢前几次上下文）。
- **`RunFault` 不出线**：不出 IPC、不进 `RunUpdate` / `RunNodeRecord` 载荷、不改 `src/types/generated/bindings.ts`（`MutationTestOutcome` 本就零 Serialize；另两 outcome 亦只在进程内 walker 消费）。节点面只经既有 `detail` / `status` 表达（`stopped` / `failed` + 归因摘要）。
- **重试预算不入 config schema**：`max_attempts` / backoff / `total_budget` 与既有超时预算同性质，均为代码常量（观察期后再议用户可配）。
- **与 `exec-run-shell` 的分工**：那篇已把「取消贯通进程」落地并把终态显式留痕为「过渡形态（failed）」，本篇正是其声明的后续变更——因此需要 MODIFY 该篇写进 spec 的过渡表述（`desktop-checks-domain`、`desktop-change-orchestration`、`desktop-mutation-testing` 各一处），MUST NOT 让两套口径并存。
- **遗留观察项不入本篇**（explore §7）：云枢是否有本地排除 UI、火绒排除是否需重启驱动、两者均无法排除时「重试 5 次 + 明确文案」的长期形态、`aggregate_coverage` 静默忽略失败 suite（失败 suite `coverage=null` 不参与加权，会把「rust 一个数没产」报成 pass）——最后一条建议单列小项修复。

## 能力

### 新增的能力

无。锁诊断落 `crates/infra/exec`（infra 层进程/系统 API 面的既有归属），故并入既有 **desktop-exec-runtime**，不另立能力（另立会把「执行体契约」与「执行体诊断面」拆成两个总是一起改的能力）。

### 修改的能力

- **desktop-exec-runtime** — **MODIFIED**「统一执行体 run_shell 契约」（`RunOptions.retry`、`RunReport.attempts` / `retried`，单次语义零变）；**ADDED**「瞬态签名字典与有限重试策略」（签名表 / 触发判据 / 永不重试两臂 / 预算硬顶 / 尝试面保留）；**ADDED**「锁占用精确探针与占用者点名」（DELETE+全共享判据 / Restart Manager / 非 Windows 降级 / 触发时机 / 路径解析）。
- **desktop-checks-domain** — **ADDED**「瞬态重试与环境错误归因（fault 装配）」；**MODIFIED**「检查域 spawn 统一经 exec 执行体」（取消收敛口径由过渡形态 `Err("执行已取消")` 改判 `fault = Stopped`，其余能力面不变）。
- **desktop-test-execution** — **MODIFIED**「suite 执行超时预算与取消」（超时 / 瞬态耗尽改由 fault 承载、不落 `execution_error`；取消改判 `fault = Stopped`；预算 1200 s 与合并流顺序零变）。
- **desktop-mutation-testing** — **MODIFIED**「段执行取消与进程树击杀」（`Cancelled` → `fault = Stopped`；段超时仍 `Interrupted` 且不进 fault；不接 retry）。
- **desktop-change-orchestration** — **ADDED**「执行面归因 RunFault 与门禁步分派」（三 outcome 加 field / 五处分派 / 环境文案与基线文案可辨 / 零 worker 调用）；**MODIFIED**「停止贯通到进程执行（cancel 传播）」（终态语义由 failed 改判 stopped，删过渡留痕）。
- **desktop-crate-layout** — **ADDED**「平台条件依赖的落位例外（infra 的 Windows 系统 API 绑定）」（`target.'cfg(windows)'.dependencies` 例外条款；依赖方向与分层零变）。

其余能力零改动，理由：

- **desktop-workflow-write-face / desktop-phase-lifecycle**：fault 不改计划图、不改步声明、不改 `OnFail` 语义——`Terminal` / `Escalate` 声明原样，fault 只改「同一失败面向哪一分支收口」。
- **desktop-checks-domain 的纯层与输出记录 requirement**：`crates/core/checks`（解析 / 聚合 / 复用门）与 `record_exec_output` / `read_exec_output` 全零改动——record 仍单一 `.log`，只是内容多了尝试分隔头。
- **desktop-agent-execution**：`bash.rs` 不接 retry、不接探针（agent 租户语义，见 desktop-exec-runtime「未迁移 spawn 面」）。
- **前端 / bindings / 插件 / config schema**：零载荷字段出线、零新 IPC、零 `plugins/**`、零 schema 扩展。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/infra/exec/src/transient.rs`（新） | 签名表（三族）+ `is_transient(report) -> Option<&'static str>` + `RetryPolicy { max_attempts, backoff, total_budget }`（`none()` / `transient()`）+ `AttemptTrace { exit_code, transient, stdout, stderr }` |
| `crates/infra/exec/src/run.rs` | `RunOptions.retry: RetryPolicy`；单次执行抽为内部 `attempt_once`；重试循环（首跑 + 至多 4 次、1/2/4/8 s 退避、1500 s 总预算、逐次取消观测、预算耗尽即停）；`RunReport.attempts` / `retried`（`stdout` / `stderr` 仍为末次） |
| `crates/infra/exec/src/lockprobe.rs`（新） | 精确探针（std `OpenOptionsExt`：`access_mode = DELETE \| FILE_READ_ATTRIBUTES`、`share_mode = READ\|WRITE\|DELETE`，错误码 32 判阻塞）；Restart Manager 点名（`cfg(windows)`）；非 Windows `lsof` 降级；三种错误串的路径解析；归因文案装配（占用者 / 被锁文件 / 重试次数 / 处理指引） |
| `crates/infra/exec/src/lib.rs` | `mod transient; mod lockprobe;` + 导出 `RetryPolicy` / `AttemptTrace` / `LockHolder` / `locked_paths` / `lock_holders` / `attribution_text`；`kill_tree` / `ProcessOutput` / `record_exec_output` 零变 |
| `crates/infra/exec/Cargo.toml` | 新增 `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_System_RestartManager"] }`（逐项注释理由） |
| `crates/infra/checks/src/static_check.rs` | `RunOptions.retry = RetryPolicy::transient()`；record 拼全部尝试（分隔头）；fault 装配（`TimedOut` / `Cancelled` / 耗尽命中签名）；`passed` 保持 `exit_code == 0` 口径不变 |
| `crates/infra/checks/src/testexec/runner.rs` | 同上；`PlanExecution` 增 `fault: Option<RunFault>`；`TimedOut` 不再产 plan `execution_error`；`Cancelled` 不再 `Err`；合并流顺序零变 |
| `crates/infra/checks/src/testexec/mod.rs` | `fault` 上传至 `TestExecutionOutcome.fault`（多 plan 聚合规则：任一 fault 即上抛，`Stopped` 优先于 `Environment`）；`conclusion = Error` 成因面收窄 |
| `crates/infra/checks/src/mutation/mod.rs` | 不接 retry；`SegmentFailure::Cancelled` → `fault = Stopped`（不再 `conclusion = Error` + 取消反馈文案；`Fatal` / `Interrupted` 分流零变） |
| `crates/core/orchestration/src/port.rs` | `RunFault { Stopped, Environment(String) }`；`StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome` 各加 `fault: Option<RunFault>` |
| `crates/core/orchestration/src/walker.rs` | 五处步执行体的 fault 前置分派；节点 settle 按 fault 收 `stopped` / `failed`；新文案常量 `TXT_ENV_STATIC` / `TXT_ENV_TEST`；`step_mutation_test` 取消面改判 stopped |
| `packages/desktop/package.json` | `version` 0.5.24 → 0.5.25（用户可见：占用者点名 + 自动重试 + 「环境坏 ≠ 检查红」文案；沿「仅用户可见变更才提升」纪律） |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 签名表驱动（命中 / 不命中 / 大小写 / 裸 `failed to write` 不命中 / `TimedOut`·`Cancelled` 恒不命中） | `crates/infra/exec/src/transient_test.rs`（新） | 新增；常量锚定 `max_attempts = 5` / 退避四段 / `total_budget = 1500 s` |
| flaky shim（第 1、2 次锁文本退出 1、第 3 次成功）→ `attempts == 3` + `Completed`；持续锁 → `attempts == 5` + 预算判定；真红（无签名）→ `attempts == 1`；重试期间 cancel 即刻收敛 | `crates/infra/exec/src/run_test.rs` | 新增（沿既有 sleep shim / `start_paused` + 手动 `advance` 先例）；`retried` 内容与分隔头顺序断言 |
| 探针自证：share 含 delete 的读句柄不误报 / 无 delete 共享 → `BLOCKED err=32` / 本进程独占打开 → 报自己，释放后 `(no lockers)` / 三种错误串路径解析 | `crates/infra/exec/src/lockprobe_test.rs`（新，`cfg(windows)`） | 新增；非 Windows 臂只测 `lsof` 缺失即跳过（编译隔离） |
| static-check：瞬态耗尽 → `fault = Environment` 且 `passed == false`；正常红 → `fault == None`；无配置直过 → `fault == None`；超时 → `fault = Environment`；取消 → `fault = Stopped`；record 含全部 attempt 分隔头 | `crates/infra/checks/src/static_check_test.rs` | 新增；既有用例补 `fault` 断言 |
| test-execution：瞬态耗尽 → `fault = Environment` 且 summary 零 `execution_error`；真红 → `conclusion = Fail` + `fault == None`；超时 → `fault = Environment`；取消 → `fault = Stopped`；解析失败 → `conclusion = Error` + `fault == None`；`fault` 跨 plan 聚合规则 | `crates/infra/checks/src/testexec/{runner,mod}_test.rs` | 新增 + 既有超时 / 取消用例改写（旧断言「超时落 execution_error」「取消返回 Err」随裁决作废） |
| mutation：cancel → `fault = Stopped`（`conclusion` 不再被门禁消费）；段超时仍 `Interrupted` 且 `fault == None`；`Fatal` 仍 `Err` | `crates/infra/checks/src/mutation/mod_test.rs` | 改写取消用例 + 新增超时不入 fault 锚点 |
| walker：`fault = Environment` → run `failed` 且**零 worker 调用**（反馈计数不增长、不进决策重试）；`fault = Stopped` → run `stopped`；环境文案含占用进程名与「已自动重试 N 次」、与基线红文案可辨；terminal 两步（基线 static-check / test-execution）同分派；真红 `fault = None` 走既有分支 | `crates/core/orchestration/src/walker_test.rs` | 新增（可编程假 runner 直接给 `fault`；`RecordingWorker` 计数断言） |
| 三 outcome 字面量补 `fault: None`（约 71 处） | `core/orchestration/src/{port_test,steps_test,walker_test}.rs`、`infra/checks/src/{static_check_test,testexec/mod_test,mutation/mod_test}.rs`、`infra/vcs/src/lib_test.rs` | 机械更新（签名不变，仅结构体字面量） |

### 不要修改

- `crates/core/checks`（解析 / 聚合 / 复用门 / 诊断树纯层）；`crates/infra/exec/src/{output,shell}.rs` 与 `record_exec_output` / `read_exec_output` / `mint_exec_id` / `kill_tree` 本体语义。
- `crates/infra/agent/**`（`sdk/bash.rs` 不接 retry、不接探针）；`crates/infra/vcs/**` 生产源码（仅测试假件字面量）。
- `crates/core/workflow/**`（步声明 / `phase_table` / `OnFail` 封闭集 / 写面）；`crates/core/orchestration/src/state.rs`（`RunUpdate` / 节点词汇 / `ChangeStepStatus` 零扩展）。
- 前端 `packages/desktop/src/**` 与 `src/types/generated/bindings.ts`（`RunFault` 不出线，`bindings:check` 应零 diff）；`openspec/config.json`（重试 / 预算参数零 schema 扩展）。
- `plugins/**`（插件源已弃用，零改动零 bump）；`openspec/changes/lock-fault-diagnostics/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 命中瞬态签名时自动重试；耗尽后 `RunReport.attempts == 5`；flaky 场景 `attempts == 3` 即 `Completed` 绿；重试面总预算 1500 s 生效（超预算不再发起新尝试，墙钟上界 = 总预算 + 单次尝试预算） | `exec::transient_test` + `exec::run_test`（预算用例以 `start_paused` 虚拟时钟断言） |
| AC-2 | `TimedOut` / `Cancelled` 零重试（`attempts == 1`、`retried` 空）；真红（无签名非零）零重试，行为与现状逐字节一致 | `run_test` 三臂用例 |
| AC-3 | 耗尽后归因点名占用者：诊断与终局文案含「占用进程：<名> (pid)」或「未定位占用者（可能为内核过滤驱动）」+ 被锁文件路径 + 「已自动重试 N 次」+「排除 workspace 父目录（逐 worktree 无效）」 | `lockprobe_test` + `static_check_test` + `walker_test` 文案断言 |
| AC-4 | 探针不误报、不扰动：share 含 delete 的读句柄判「未阻塞」；无 delete 共享判「阻塞 err=32」；探针自身只读（零写、零独占打开、零残留句柄） | `lockprobe_test` 三臂；源码审查零 `create` / `truncate` |
| AC-5 | 环境错误不再注回 executor：`fault = Environment` 时 walker 零 worker 调用、相位 fail 未升格、决策未分叉；run 收敛 `failed` 且文案引导环境侧 | `walker_test`（假 runner 给 fault + worker 计数断言） |
| AC-6 | 环境错误与基线检查红**文案可辨**：前者引导「排除安全软件路径」，后者仍引导「修基线 / 修测试」；两态不共用同一段文案 | `walker_test` 两组文案断言（`TXT_ENV_STATIC` / `TXT_ENV_TEST` vs `TXT_BASELINE_*`） |
| AC-7 | 取消运行中的检查步 → 该步节点 `stopped`、run 在秒级收敛 `stopped`（非 failed），static-check / test-execution / mutation 三面一致 | `walker_test` 取消用例 + `mutation/mod_test` 取消用例（旧「收敛 failed」断言作废） |
| AC-8 | 既有语义零变：static-check `2>&1` 交错流与 `diagnostics` 口径不变；test-execution / mutation 合并流（stdout 前缀 + stderr 后缀）不变；报告 / summary 回填 / 复用门 / 段级断点续跑不受影响；无配置直过 / 程序不可达 `Err` / `Fatal` 分流不变 | 既有 `static_check_test` / `runner_test` / `mutation/*_test` 全绿 + 新增断言 |
| AC-9 | `RunFault` 不出线：`bindings:check` 零 diff、无新 IPC 命令、载荷类型未进 `RunUpdate` / `RunNodeRecord` | `pnpm -C packages/desktop run bindings:check` + `grep` 复核 |
| AC-10 | 全部既有测试通过（含约 71 处 outcome 字面量补 `fault: None`），覆盖率不低于阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90，基线 97.06 / 93.04 / 93.63） | desktop 门禁 `pnpm run check` + test-execution 门禁覆盖率 |
| AC-11 | `packages/desktop/package.json` 0.5.24 → 0.5.25；`plugins/**` 零改动零 bump；`windows-sys` 仅为 `cfg(windows)` 目标依赖且 feature 只含 `Win32_System_RestartManager` | 版本 diff + `Cargo.toml` 审查 / `cargo tree --target x86_64-unknown-linux-gnu`（非 Windows 零新增） |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **签名误判** —— 真实编译错误被吞成「环境错误」，重试 5 次后仍失败且文案误导用户去改环境 | 用户找不到真因 | 中 | 签名表刻意收窄（不含裸 `failed to write`）；真红用例锚定 `attempts == 1`；fault 文案必携最后一次诊断原文（用户仍能看到 rustc 报错） |
| R-2 | 重试放大耗时：慢挂时 5 次尝试把单步墙钟推到数十分钟 | 用户以为卡死 | 中 | `total_budget = 1500 s` 硬顶（从首跑起算，超预算即停）；每次尝试输出带分隔头与重试次数，进度可见；`TimedOut` 永不重试 |
| R-3 | `RmGetList` 在某些环境返回空 / Restart Manager 服务不可用 | 点名失效 | 中 | 空清单降级为「未定位占用者」并附「可能为内核过滤驱动」说明，MUST NOT 阻断主流程；探针调用失败静默（仅丧失归因，不改变 fault 判定） |
| R-4 | 探针自身扰动构建（若以「独占打开」形态探测 rustc 待 finalize 的文件） | 反向制造 `os error 32` | 低 | 判据固定为「DELETE 权限 + 全共享打开后立即关闭」——不独占、不写、不截断；`lockprobe_test` 以「share 含 delete 的读句柄不误报」钉住该形态 |
| R-5 | `windows-sys` 依赖膨胀 / 版本漂移 | crate 图审查被质疑 | 低 | 只开 `Win32_System_RestartManager`（不引 `Win32` 全量），且为 `cfg(windows)` 目标依赖；版本锚定锁内已有 `0.61.2`；`desktop-crate-layout` 写入例外条款 |
| R-6 | 「环境坏」与「检查红」的新分界被未来的 runner 改动破坏 | 分界回退到「一切非零皆红」 | 中 | 分派口径写进 spec（`desktop-change-orchestration`「RunFault 与门禁步分派」+ 两处 MODIFIED），`conclusion = Error` 的成因面收窄同写入 spec；`walker_test` 以「零 worker 调用」钉住 |
| R-7 | 取消语义由 failed 改判 stopped 与既有测试 / 文档表述冲突 | 旧断言红、口径并存 | 高（本篇既定） | 三处过渡留痕（checks / orchestration / mutation）在同一变更内显式 MODIFIED（拷贝原段再改），MUST NOT 留两套口径；旧取消用例随裁决改写 |
| R-8 | 新增三个模块（`transient` / `lockprobe` / 重试循环）稀释覆盖率（functions 距阈值余量约 3.6 点） | 门禁红 | 中 | 三模块均先以单元测试全覆盖（签名表驱动 / 探针自证 / flaky shim 三臂）再计入；`cfg(windows)` 面以平台条件测试隔离，非 Windows 只测降级臂 |
| R-9 | 尝试面保留（`retried` 持全部输出字节）放大内存 | 大构建输出下内存抬升 | 低 | 仅重试场景才累积（未命中签名即空），最坏 5 份同量级输出；record 落盘后即随 `RunReport` 释放 |
| R-10 | R-2 的预算与产品「单步 1200 s」直觉不一致（重试面总预算 1500 s 只够一次慢挂 + 少量余量） | 用户困惑「为什么只重试几次」 | 低 | 文案明示「已自动重试 N 次（每次重试会跳过已完成部分）」；预算常量与理由落 spec 与代码注释同源 |

## 遗留观察项（不在本篇范围）

1. 云枢（`TBProtect`）是否提供本地排除 UI（无版本信息，可能云控下发）。
2. 火绒企业版排除后是否即时生效（minifilter 通常需重启驱动）。
3. 若两者均无法排除：接受「重试 5 次 + 明确文案」作为长期形态（每次 run 多 1–3 分钟）。
4. `aggregate_coverage` 静默忽略失败 suite（`weighted_measured?`）——失败 suite 的 `coverage=null` 不参与加权，导致「rust 一个数没产却报 pass」，建议单列小项修复。
5. 环境侧根治（排除 workspace 父目录、`.cargo`、pnpm 缓存）属 IT 行动项，产品只负责指出。
