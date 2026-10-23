# 设计: lock-fault-diagnostics

> **变更**: lock-fault-diagnostics
> **日期**: 2026-10-22
> **基于**: `proposal.md`、`explore.md` 与 `specs/{desktop-exec-runtime,desktop-checks-domain,desktop-test-execution,desktop-mutation-testing,desktop-change-orchestration,desktop-crate-layout}/spec.md`

---

## 提案与规格同步状态

proposal 的六面与六份 delta spec 已定稿，本设计不重复其论证，只做四件事：逐文件落定实现形态（含随必填字段 / 新导出而必须随动的三处代码）、把规格未定的成形缺口（`conclusion` 占位取值、分隔头的单次尝试豁免、归因文案与处置指引的分层归属、多 plan fault 早停与聚合）以决策留痕方式收口、给出 `RunFault` 的全链数据流、按 AC-1..AC-11 逐条对齐。

起点核实（工作树实况；路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）：

- **执行体单次收口**：`crates/infra/exec/src/run.rs:112` `run_shell`；`RunOptions` 四字段（`:45-54`，无 `Default`）；`RunReport` 四字段（`:73-83`）；入口取消短路（`:121-128`）；三臂竞速（`:155-166`）；杀后收尾常量 `CANCEL_POLL_INTERVAL = 200 ms`（`:33`）/ `FINISH_GRACE = 2 s`（`:39`）。
- **`RunOptions` 构造点四实量**：`checks/src/static_check.rs:79`、`checks/src/testexec/runner.rs:94`、`checks/src/mutation/cargo_mutants.rs:41`、`checks/src/mutation/stryker.rs:68`。后两处**不在** proposal「实现文件」清单内，但 `retry` 为必填字段 → 必须各加一行 `retry: RetryPolicy::none()`（见「变更清单 - 修改文件」的随动注记）。
- **static-check 现状口径**：诊断 `static_check.rs:102`（`stdout` lossy、**不读** `stderr`，命令串带 `2>&1`）、`passed = exit_code == 0`（`:103`）、record 取 `run.stdout`（`:106`）、超时 / 取消两臂 `Err`（`:88-97`）。
- **test-execution 现状口径**：三臂 `runner.rs:103-130`（`Cancelled` → `Err("执行已取消")`、`TimedOut` → `EXIT_CODE_NO_STATUS` + `execution_error`）；`PlanExecution`（`runner.rs:29-49`）；聚合循环逐 plan `?` 早停（`testexec/mod.rs:133-139`）；record 与 outcome 装配单点（`testexec/mod.rs:169-194`）。
- **mutation 现状口径**：`SegmentFailure{Fatal,Interrupted,Cancelled}`（`mutation/mod.rs:60-64`）；中断收束面（`:228-253`，`Cancelled` 与 `Interrupted` 同走 `conclusion = Error`）；`SEGMENT_SPAWN_TIMEOUT_MS = 3_600_000`（`:50`，两段共用）。
- **port 载荷**：`StaticCheckOutcome`（`port.rs:169`）、`TestExecutionOutcome`（`:207`）、`MutationTestOutcome`（`:292`）；三 runner trait 的取消文档均写「实现侧返回 `Err("执行已取消")`」（`:325` / `:338` / `:350` 区），为本变更必须同步改写的第二套口径。
- **walker 分派点**：`step_static_check`（`walker.rs:923`）、`step_test_execution_escalate`（`:1055`）、`step_static_check_terminal`（`:1131`）、`step_test_execution_terminal`（`:1194`）、`step_mutation_test_escalate`（`:1281`）；基线文案三常量 `:59-64`；`run_tool`（`:3508`）与 `check_cancel`（`:3538`）本次零改动。
- **`RunOptions` 之外无第五个 spawn 消费点**：`crates/infra/agent/src/sdk/bash.rs` 只用 `kill_tree`，不接 `run_shell`（proposal「不改 agent 租户语义」成立）。
- **出线面**：`packages/desktop/src/types/generated/bindings.ts` 中**不存在** `StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome`（三者均为进程内载荷）→ `fault` 字段零 diff 成立。
- **依赖面**：`exec/Cargo.toml` 现依赖 `foundation` + `tokio`（features `process,io-util,time,macros`）；`windows-sys` 未在 workspace 任何 crate 直接声明，但锁内已有 `0.61.2`（传递依赖）；`windows-sys 0.61.2` 的 `Win32_System_RestartManager` feature 提供 `RmStartSession` / `RmRegisterResources` / `RmGetList` / `RmEndSession` / `RM_PROCESS_INFO`（已核对 vendored 源）。
- **版本面**：`packages/desktop/package.json` = `0.5.24`。
- **字面量实量**：`StaticCheckOutcome {` 29 处 / `TestExecutionOutcome {` 20 处 / `MutationTestOutcome {` 22 处 ≈ proposal 估的 71 处（含类型定义点），测试相位逐处补 `fault: None`。

**一处规格事实偏差（须在规格相位回写，见「待决问题」2）**：`desktop-mutation-testing` delta 写「段超时预算 SHALL 维持 **1800 s**（`SEGMENT_SPAWN_TIMEOUT_MS` 不变）」「文案仍为『段执行超时（**1800 秒**）强制终止』」，而工作树实值为 `3_600_000 ms`（文案由 `SEGMENT_SPAWN_TIMEOUT_MS / 1000` 派生 → 实际呈现「3600 秒」）。本设计**不动该值、不改文案**（proposal：mutation 面只改取消归因），偏差写入待决问题。

**只读红线零触点**：`crates/core/checks/**`、`crates/core/workflow/**`、`crates/core/orchestration/src/state.rs`、`crates/infra/exec/src/{output,shell}.rs` 与 `record_exec_output` / `read_exec_output` / `mint_exec_id` / `kill_tree` 本体、`crates/infra/agent/**`、`crates/infra/vcs/**` 生产源码、`crates/core/foundation/**`、`crates/infra/checks/src/testexec/detect.rs`、前端 `packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**`。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 重试循环的形态 | `run.rs` 内新增私有 `attempt_once(...)`（现 `run_shell` 主体原样搬入，返回 `Attempt`），`run_shell` 只做「入口短路 → 循环（attempt_once → 瞬态判定 → 上限/预算判定 → 退避）→ 组 `RunReport`」 | spec「重试循环 MUST 复用同一执行骨架，MUST NOT 另写第二条 spawn 路径」：把骨架整体下沉为单次助手，循环层零 spawn 知识 |
| D2 | 预算判定点 | `total_budget` 自**首跑开始**（循环前 `Instant::now()`）起算；**每次重试尝试开始前**判定 `elapsed >= total_budget` → 不再发起新尝试、把末次结果原样返回（`Completed` + 非零）；`attempt_once` 内既有的 `timeout` 臂不受预算影响（预算 MUST NOT 在尝试进行中强杀） | spec「预算硬顶」逐字；墙钟上界 = `total_budget + 单次 timeout`（有界） |
| D3 | 退避期取消观测 | 每次重试前 `tokio::select! { biased; _ = wait_cancel(&cancel) => 取消收口, _ = sleep(backoff[k]) => 继续 }`——复用现成 `wait_cancel`（200 ms 轮询），不新增计时原语 | spec「退避等待 MUST 同样观测 `cancel`，置位即刻收敛，不先等完退避」；且复用 `run.rs` 既有轮询单点 |
| D4 | `attempts` 口径的三处边界 | ①入口取消短路（未 spawn）记 `attempts = 1`、`retried = []`；②预算 / 上限早停记**实际完成次数**（如 4），末次不进 `retried`；③退避等待期取消 → `outcome = Cancelled`、`exit_code = -1`、`attempts` = 已完成次数、`stdout`/`stderr` 取**刚完成的末次**尝试字节、`retried` **不含**该末次（把「转入 `retried`」推迟到退避成功之后，`retried` 恒等于「末次之前」的尝试集） | spec「`attempts` ≥ 1」「超时 / 取消 → `attempts == 1`、`retried` 空」「退避取消 → `attempts` 记已完成次数」与「`retried` 只承载此前各次尝试（末次不入）」四句需同时成立：把入栈动作放在退避成功之后，是唯一不破坏「末次不入」的排布；`Cancelled` 三消费点均不读输出（既有语义），字节取值不构成回归 |
| D5 | 瞬态判定的形态 | `transient.rs` 内**单一封闭常量表** `TRANSIENT_SIGNATURES: [&str; 13]`（三族按注释分组，不建族枚举——无消费者读族名）；判定核心为 `pub(crate) fn transient_signature(outcome: RunOutcome, exit_code: i32, stdout: &[u8], stderr: &[u8]) -> Option<&'static str>`（`Completed` && `exit_code != 0` && 合并输出（`stdout ++ stderr`，lossy、整体 `to_lowercase()`）包含表中任一条（逐条 `to_lowercase()` 比对）），`pub fn is_transient(report: &RunReport) -> Option<&'static str>` 只做委托；命中返回**表中原文**（`LNK1104` / `EBUSY` 等保持大写形态） | spec 封闭表 + 「逐行包含匹配」——本表签名均不含换行，整串包含与逐行包含等价；`pub(crate)` 核心是必需的：重试循环在组建出 `RunReport` **之前**就要判定（`report` 尚不存在），公开面仍与 spec 的单一签名一致；返回原文而非输出片段，使 `AttemptTrace.transient` 与分隔头稳定可断言 |
| D6 | 精确探针的判据与平台臂 | `lockprobe::blocks_delete(path) -> bool`（私有）：Windows 用 std `OpenOptionsExt`——`access_mode(DELETE \| FILE_READ_ATTRIBUTES)`（`0x0001_0000 \| 0x0000_0080`）+ `share_mode(READ \| WRITE \| DELETE)`（`0x1 \| 0x2 \| 0x4`）打开后立即 `drop`，`Err(e)` 且 `e.raw_os_error() == Some(32)` → `true`（阻塞），其余一切（含 `NotFound` / `AccessDenied` / 打开成功）→ `false`；非 Windows 臂恒 `false`（POSIX 无强制共享语义，探针不适用，点名仍走 `lsof`） | spec「只问是否阻塞 rename / delete」「MUST NOT 独占 / 写入 / 截断」；单一 `bool` 判据不留「误报残留」中间态；`raw_os_error == 32` 是 `ERROR_SHARING_VIOLATION` 的唯一判定键 |
| D7 | 占用者点名与阻塞集的关系 | `attribution_text` 先 `locked_paths` → 再 `blocked = paths.filter(blocks_delete)` → **点名对象 = blocked；blocked 空则退化为全部解析路径** | 探针判据因此有真实落点（不把「占用已解除」的文件当占用者），且 RM / `lsof` 仍能在「内核驱动瞬时持有后已释放」场景给出有效归因；两函数各守一职（判据 / 点名），探针本身零副作用 |
| D8 | 点名实现（Windows / 非 Windows） | Windows：单 session 注册全部点名路径（`RmStartSession(&mut handle, 0, key_buf)` → `RmRegisterResources(handle, n, *const PCWSTR, 0, null, 0, null)` → 两次 `RmGetList`（首取 `n_needed` / `n_proc`，`ERROR_MORE_DATA = 234` 视为正常）→ 读 `RM_PROCESS_INFO.strAppName`（UTF-16 到 `0` 止）+ `.Process.dwProcessId` → `RmEndSession`）；按 `(name, pid)` 去重；任一步失败 / 空清单 → `vec![]`。非 Windows：逐路径 `SystemCommand::bare("lsof").args(["-F", "pc", "--"]).arg(path).into_std().output()`，解析 `p<pid>` / `c<name>` 成对行；`lsof` 缺失（spawn 失败）静默跳过 | spec「`RmStartSession → RmRegisterResources → RmGetList`」「session MUST 在返回前 `RmEndSession`」「`lsof` 不可用 MUST NOT 阻断主流程」；`lsof` 走 std（非 tokio），探针全程同步、零 async 传染 |
| D9 | 三种错误串的路径解析规则 | 逐行扫描，命中 `failed to rename archive file` / `failed to write` / `failed to remove`（大小写不敏感）后取**该行关键词之后的剩余片段**为候选：`trim` → 去前导 `:` 与空白 → 在首个 `": "`（冒号+空格）处截断（Windows 盘符 `C:\` 是冒号+反斜杠，不受影响）→ 剥包裹的反引号 / 双引号 / 尾随标点 → 非空才收；去重（按 `PathBuf` 原样，保留输入分隔符）、上限 8 条 | spec 三形态 + 去重 + ≤8；规则均为字符串层可判定，不引入路径规范化（避免改写用户可见路径） |
| D10 | 归因文案的分层归属 | `exec::lockprobe::attribution_text(diagnostics) -> Option<String>` **只**产「占用进程 / 被锁文件 / 未定位占用者降级说明」三行原料；「已自动重试 N 次 + 最后一次诊断摘要 + 环境侧处置指引」由检查域新模块 `checks-runtime::environment::fault_text(...)` **单点**装配（`static_check.rs` 与 `testexec/runner.rs` 共用，MUST NOT 两写） | spec exec「执行体 MUST NOT 自带日志格式（尝试分隔头 / 归因文案归调用方）」与 checks「诊断与记录」两句同时成立：exec 只供事实原料，业务文案（重试次数 / 指引）由业务层单点装配 |
| D11 | 输出记录的分隔头口径 | `checks-runtime::environment::attempts_log(report) -> Vec<u8>`：`attempts == 1 && retried.is_empty()` → **原样返回** `stdout ++ stderr`（零分隔头，逐字节同现状）；发生过重试 → 逐次（`retried` 按序）+ 末次各产一段 `===== attempt {n}/{max} · exit={exit_code} · transient={命中签名或 -} =====\n{该次合并流}`（`max` = `exec::RetryPolicy::transient().max_attempts`） | AC-8 要求既有 record 断言全绿（`static_check_test` / `testexec/mod_test` 均以「record 内容 == 输出」钉住单跑），故单次尝试 MUST 零分隔头；「全部尝试按序拼接」在重试面完整成立（补 spec 措辞，见待决问题 1） |
| D12 | fault 装配与 `conclusion` / `passed` 口径 | `StaticCheckOutcome.passed` 恒为 `exit_code == 0`（超时 / 取消天然 `false`，`-1` 哨兵）；`TestExecutionOutcome.conclusion` 在 fault 在位时取 **`Fail`** 占位（**MUST NOT 取 `Error`**——`Error` 成因面已收窄）；`MutationTestOutcome.conclusion` 在取消面取 **`Fail`** 占位；`fault` 在位时三值零门禁语义（walker 先查 `fault`），doc 明写 | spec「`passed` / `conclusion` MUST 保持既有取值面（MUST NOT 升三值）」「`conclusion = Error` 成因面收窄为解析失败 / harness 崩 / 框架探测失败」——若让环境面落 `Error`，收窄条款自相矛盾；`Fail` 是「非绿但未被归因为环境」的唯一无歧义占位 |
| D13 | env 面不写 summary、不落 `execution_error` | `testexec/runner.rs::execute_plan` 在 fault 在位时**直接**返回 `PlanExecution { cases: vec![], source_files: vec![], measured: None, error: None, fault: Some(..), output: 全尝试记录 }`（跳过工件解析）；`testexec/mod.rs::execute` 在聚合到任一 fault 时**不写** summary / 不跑 `diagnose_findings` / 不落盘，直接返回 fault outcome（`report_dir` + `exec_id` 照旧回填） | 「不落 plan 级 `execution_error`」+ 防止半套 `summary.json` 进复用门（`is_reusable` / `check_integrity` 会把坏 plan 的 report 当权威而永久复用） |
| D14 | 多 plan fault 聚合与早停 | 循环内 fold：`Stopped` → 记 `Stopped` 并 **`break`**（spec：取消后 MUST NOT 继续跑余下 plan）；`Environment` → 记首个 `Environment`、**继续**跑余下 plan（`Stopped` 优先于 `Environment` 的聚合规则才有可观测面，且混合栈（rust + js）里余下 plan 可能正常）；fault 为 `None` 的 plan 照常 `build_sub_report` | spec「多 plan 聚合时 fault SHALL 上抛（任一 plan 有 fault 即 outcome 带 fault；`Stopped` 优先于 `Environment`）」+「取消 MUST NOT 继续跑余下 plan」两句同时满足 |
| D15 | walker 分派单点助手 | 新增私有 `fn settle_fault(guard, phase, attempt, step, instance_no, fault: &RunFault, template: Option<&str>, report_dir: &str, output_ref: Option<String>) -> Terminal`：`Stopped` → `emit_node(StepSettle::stopped(Some("用户请求停止")).with_output_ref(..))` + `Terminal::stopped("用户请求停止")`；`Environment(text)` → `message = template.replace("{fault}", text).replace("{dir}", report_dir)`（`template = None` 时退化为 `text` 原样）+ `emit_node(StepSettle::failed(diagnose_brief(&message)))` + `Terminal::failed(message)`。五处步执行体一律 `return Err(settle_fault(...))` 前置 | spec 五处分派表 + 「`Environment` MUST 一律 `Terminal::failed`」+「MUST NOT 升格相位 fail / MUST NOT 进决策重试 / MUST NOT 注回 executor」：早退（而非继续走反馈边 / 升格分支）是「零 worker 调用」的唯一实现形态；单点助手保证五处文案与节点形态同形 |
| D16 | 环境面终局文案常量 | `TXT_ENV_STATIC = "静态检查无法完成（环境错误）：\n{fault}\n报告目录 {dir}"`；`TXT_ENV_TEST = "测试执行无法完成（环境错误）：\n{fault}\n报告目录 {dir}"`——重试次数与「排除 workspace 父目录（逐 worktree 无效）」指引**经 `{fault}` 嵌入**（MUST NOT 在模板里再写一份） | spec ①..⑤ 为「MUST 含」下界：`{fault}` 供 ②③④，模板供 ① 与 ⑤；与 `TXT_BASELINE_*` 零共用句（前者引导「排除安全软件路径」，后者引导「在基线上修复后重新发起」）→ AC-6 可辨 |
| D17 | mutation 取消面 | `SegmentFailure::Cancelled` → `MutationTestOutcome { fault: Some(Stopped), conclusion: Fail, feedback_text: String::new(), ..既有 partial 面 }`（`report_paths` / 临时面保留纪律不变）；`Interrupted` → 既有 `conclusion = Error` + 「突变执行中断」文案 + `fault: None`；`Fatal` → `Err` 零改；三段 spawn `retry: RetryPolicy::none()` | spec mutation「`Cancelled` → `fault = Some(Stopped)`」「`Interrupted`（含段超时）→ `fault = None` + `conclusion = Error`」「零重试」；`feedback_text` 清空因 walker（stopped 面）不消费它，保留会被误读为「需修复」 |
| D18 | `RunFault` 出线纪律 | `port.rs` 定义 `RunFault`，**零 `Serialize`**、不进 `RunUpdate` / `RunNodeRecord`、不改 `bindings.ts`；三 outcome 均只在进程内 walker 消费 | spec「`RunFault` MUST NOT 出线」；`bindings:check` 零 diff 是 AC-9 的机械锚 |
| D19 | `is_transient` 的导出 | `lib.rs` 额外 `pub use transient::is_transient;`（proposal 的导出清单漏列，但 `infra/checks` 是唯一调用方，无替代形态） | 不导出则检查域要么重算签名表（违反「字典扩展 SHALL 经本模块一处修改」），要么把判定下沉到 `RunReport` 方法（改类型契约更大） |
| D20 | 版本面 | `packages/desktop/package.json` `version` 0.5.24 → **0.5.25** | 用户可见变更（占用者点名 + 自动重试 + 「环境坏 ≠ 检查红」文案），沿「仅用户可见变更才提升」纪律；`plugins/**` 零改动零 bump |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 瞬态判定与重试策略 | 封闭签名表（三族）+ `is_transient` + `RetryPolicy{none,transient}` + `AttemptTrace` | `packages/desktop/src-tauri/crates/infra/exec/src/transient.rs`（新） | `crate::run::{RunOutcome, RunReport}`、`std::time::Duration` | std-only（零 tokio、零第三方） |
| 统一执行体重试循环 | `attempt_once` 单次骨架 + 预算 / 上限 / 退避 / 取消观测循环 + 尝试面装配 | `crates/infra/exec/src/run.rs`（修改） | `transient::{AttemptTrace, RetryPolicy, is_transient}`、`tokio`（既有特性面） | tokio `select!` + `Instant` 预算 |
| 锁占用诊断探针 | `locked_paths`（三形态解析）/ `blocks_delete`（私有判据）/ `lock_holders`（RM / `lsof`）/ `attribution_text`（原料装配） | `crates/infra/exec/src/lockprobe.rs`（新） | `crate::SystemCommand`（`lsof` 与 `kill_tree` 同式 std spawn）、`windows-sys`（`cfg(windows)` 仅 Restart Manager）、std `OpenOptionsExt` | Windows 系统 API 绑定 + std fs |
| exec 导出面 | `mod transient; mod lockprobe;` + 六项再导出（含 `is_transient`，D19） | `crates/infra/exec/src/lib.rs`（修改） | 同上 | 既有 crate 形态 |
| 检查域环境归因装配 | `attempts_log`（全尝试记录，D11）/ `fault_text`（重试次数 + 末次诊断 + 处置指引，D10）+ 两条文案常量 | `crates/infra/checks/src/environment.rs`（新；`lib.rs` 加 `mod environment;`） | `exec::{ProcessOutput, RunReport, RetryPolicy, is_transient, attribution_text}` | 纯函数（零 io / 零 async） |
| static-check 装配 | 开 `RetryPolicy::transient()`；fault 装配表；record 换 `attempts_log`；诊断 / `passed` 口径零变 | `crates/infra/checks/src/static_check.rs`（修改） | 同上 + `orchestration::port::RunFault` | 既有 runner 形态 |
| suite 执行装配 | 同 `RetryPolicy::transient()`；`PlanExecution.fault`；超时 / 瞬态耗尽 / 取消三臂不再 `Err`、不再落 `execution_error` | `crates/infra/checks/src/testexec/runner.rs`（修改） | 同上 | 既有 |
| test-execution 聚合 | 多 plan fault fold（`Stopped` 优先 / `Stopped` 早停）+ 任一 fault 不写 summary + fault outcome 装配 | `crates/infra/checks/src/testexec/mod.rs`（修改） | — | 既有 |
| 突变段失败面 | 三段零重试；`Cancelled` → `fault = Stopped`（不再 `conclusion = Error` + 取消文案） | `crates/infra/checks/src/mutation/{mod,cargo_mutants,stryker}.rs`（修改） | `orchestration::port::RunFault` | 既有 E5 分流形态 |
| 归因载荷 | `RunFault { Stopped, Environment(String) }` + 三 outcome 的 `fault: Option<RunFault>` + 取消口径文档改写 | `crates/core/orchestration/src/port.rs`（修改） | 零新增依赖 | 既有进程内缝 |
| 门禁步分派 | 五处 fault 前置分派（`settle_fault` 单点）+ `TXT_ENV_STATIC` / `TXT_ENV_TEST` | `crates/core/orchestration/src/walker.rs`（修改） | `port::RunFault` | 既有 walker |

---

## 变更清单

> 路径口径：新增文件表用仓库根相对全路径；修改文件表与 proposal「变更范围 - 实现文件」同口径，`crates/...` 前缀省略 `packages/desktop/src-tauri/`。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/infra/exec/src/transient.rs` | 瞬态签名表（13 条 / 三族注释分组）、`is_transient`、`RetryPolicy`（`none` / `transient` 两构造 + 三常量）、`AttemptTrace`；模块 doc 说明「刻意不收裸 `failed to write`」「`TimedOut` / `Cancelled` 永不命中」 | 公共函数表 `is_transient` / `RetryPolicy::none` / `RetryPolicy::transient` 行；类型表 `RetryPolicy` / `AttemptTrace` 行 |
| `packages/desktop/src-tauri/crates/infra/exec/src/lockprobe.rs` | `locked_paths`（三错误串解析、去重、≤8）、私有 `blocks_delete`（D6 判据，两平台臂）、`lock_holders`（RM 单 session / `lsof` 逐路径，D8）、`LockHolder`、`attribution_text`（D10 原料装配） | 公共函数表 `locked_paths` / `lock_holders` / `attribution_text` 行；类型表 `LockHolder` 行 |
| `packages/desktop/src-tauri/crates/infra/checks/src/environment.rs` | 检查域环境归因装配单点：`attempts_log`（D11，单次零分隔头）、`fault_text`（D10，重试次数 + 末次诊断摘要 + 处置指引）、`ENV_GUIDANCE` 常量、诊断摘录上限常量 | 修改文件 `static_check.rs` / `testexec/runner.rs` 两行；D10 / D11 |

<!-- 无新增类型别名 / interface：本变更的类型面全部为 struct / enum 定义，见「类型定义」。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/exec/src/run.rs` | ① 新增私有 `struct Attempt { outcome: RunOutcome, exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8> }` 与 `async fn attempt_once(command: &SystemCommand, cwd: Option<&Path>, env: &[(OsString, OsString)], budget: Duration, cancel: &CancelFlag) -> Result<Attempt, RunError>`（现 `run_shell:112-200` 主体整体下沉：入口取消短路 → `command.clone().into_tokio()` + `cwd` / `envs` / `stdin(null)` / 双管道 / `kill_on_drop(true)` → `spawn`（`Err(Spawn)`）→ 三臂 `select!` → 被杀两臂 `kill_tree` + 有界收尸 + 有界排水）；② `run_shell` 改为循环层：`let started = Instant::now(); let mut attempts = 0u32; let mut retried: Vec<AttemptTrace> = Vec::new();` → `loop { attempts += 1; let attempt = attempt_once(..)?; }`，每次尝试后按序判定「非 `Completed`（`TimedOut` / `Cancelled`）即返 → `transient_signature(..)` 为 `None` 即返（真红 / 绿）→ `attempts >= retry.max_attempts` 即返末次 → `started.elapsed() >= retry.total_budget` 即返末次 → `select!{ cancel 置位即返 Cancelled, sleep(backoff[attempts-1 界限内]) 成功 → 把本次尝试 `push` 进 `retried` 并进入下一轮 }`」——「转入 `retried`」落在退避成功之后，使 `retried` 恒等于「末次之前」的尝试集（D4 ③）；③ `RunOptions` 增 `pub retry: RetryPolicy`（doc：缺省语义单次，`none()`）；④ `RunReport` 增 `pub attempts: u32` 与 `pub retried: Vec<AttemptTrace>`（doc：`stdout` / `stderr` 仍为末次）；⑤ 模块 doc 补第五能力项「有限重试（瞬态自愈）」 | D1 / D2 / D3 / D4；`CANCEL_POLL_INTERVAL` / `FINISH_GRACE` / `drain` / `wait_cancel` / `await_exit` 逐一保留（`attempt_once` 内）。`SystemCommand: Clone` 已在位，循环内按次 clone（命令规格小，零 IO） |
| `crates/infra/exec/src/lib.rs` | ① 头部模块 doc 补两条能力项「瞬态重试」「锁占用诊断」；② `mod transient; mod lockprobe;`；③ `pub use lockprobe::{attribution_text, lock_holders, locked_paths, LockHolder};`、`pub use transient::{is_transient, AttemptTrace, RetryPolicy};`；④ `pub use run::{...}` 行保持（`run_shell` / `RunError` / `RunOptions` / `RunOutcome` / `RunReport`） | `kill_tree` / `ProcessOutput` / `ExecOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` 逐字零改（AC-8） |
| `crates/infra/exec/Cargo.toml` | 新增段（`[dependencies]` 之后、`[dev-dependencies]` 之前）：`[target.'cfg(windows)'.dependencies]` + `windows-sys = { version = "0.61", features = ["Win32_System_RestartManager"] }`，逐项注释（用途 = 占用者点名；为何条件段 = 非 Windows 零新增；为何只开一个 feature = 精确探针走 std `OpenOptionsExt`，不引 FileSystem / Security） | AC-11 / D6 / D8；`[dependencies]` 段零改动（仍 `foundation` + `tokio`） |
| `crates/infra/checks/src/lib.rs` | `mod environment;`（私有模块，无再导出） | 检查域共用装配单点的挂载点（D10 / D11）；其余零改 |
| `crates/infra/checks/src/static_check.rs` | ① `RunOptions` 增 `retry: exec::RetryPolicy::transient()`；② 三臂改为：`RunError` → 既有 `Err(format!("static-check 命令{failure}"))`；`TimedOut` → `fault = Some(RunFault::Environment(format!("static-check 命令超时（{} 秒）强制终止", STATIC_CHECK_TIMEOUT.as_secs())))`；`Cancelled` → `fault = Some(RunFault::Stopped)`；`Completed` → `fault = exec::is_transient(&run).map(\|_\| RunFault::Environment(environment::fault_text(run.attempts, run.exit_code, &run.stdout)))`；三臂均 `Ok(ToolStepOutput::StaticCheck(..))`（**不再 `Err`**）；③ record 由 `&run.stdout` 换 `environment::attempts_log(&run)`；④ 诊断仍 `String::from_utf8_lossy(&run.stdout).trim_end()`（不读 `stderr`）、`passed = run.exit_code == 0` 口径零变；⑤ 无配置直过分支补 `fault: None` | AC-3 / AC-8；spec checks「超时与取消走 fault」；`ensure_program_resolvable` / `report_dir` / `STATIC_CHECK_TIMEOUT` 零改 |
| `crates/infra/checks/src/testexec/runner.rs` | ① `RunOptions` 增 `retry: exec::RetryPolicy::transient()`；② `PlanExecution` 增 `pub fault: Option<RunFault>`；③ `Cancelled` → 不再 `Err`，返回 `PlanExecution { exit_code: EXIT_CODE_NO_STATUS, output: attempts_log(&run), fault: Some(Stopped), cases: vec![], source_files: vec![], measured: None, error: None, .. }`（跳过工件解析）；④ `TimedOut` → 同上但 `fault = Some(Environment("测试命令超时（1200 秒）强制终止"))`、**不落** `execution_error`；⑤ `Completed` → `exit_code = run.exit_code`；`fault = is_transient(&run).map(\|_\| Environment(fault_text(run.attempts, exit_code, &merged)))`；`exec_error = if exit_code != 0 && fault.is_none() { Some(non_zero_exit_error(..)) } else { None }`；`output = attempts_log(&run)`；`fault` 在位 → 直接返回（不解析工件）；⑥ 合并流顺序（stdout 前缀 + stderr 后缀）与「不加 `2>&1`」零变 | spec test-execution「超时入 fault」「瞬态耗尽入 fault」「取消返回 fault 而非 Err」「合并流与工件解析不受影响」 |
| `crates/infra/checks/src/testexec/mod.rs` | ① 执行循环改 fold（D14）：`Some(Stopped)` → 记并 `break`；`Some(Environment)` → 记首个并继续；`None` → `sub_reports.push(build_sub_report(..))`；② 任 fault 在案 → `return Ok(fault_outcome(..))`（私有助手：`conclusion: Fail`、四项计数 0、`findings_brief` / `findings_detail` 空串、`report_dir` + `exec_id = record_exec_output(..)` 照旧回填）——**不写 summary / 不跑 `diagnose_findings` / 不落报告文件**；③ 复用门分支与既有 `outcome` 装配补 `fault: None`；④ 模块 doc 补「`conclusion = Error` 成因面收窄为解析失败 / harness 崩 / 框架探测失败」 | AC-5 / D12 / D13 / D14；`is_reusable` / `prune_roots` / `newest_input_mtime_ms` / `findings_*` 零改 |
| `crates/infra/checks/src/mutation/mod.rs` | ① `SegmentFailure` 的 doc 改写（`Cancelled` → `fault = Stopped` 收敛 stopped，删除「与段中断同面收场」表述）；② 中断收束面按变体分派：`Cancelled` → `MutationTestOutcome { conclusion: Fail, fault: Some(RunFault::Stopped), feedback_text: String::new(), score / threshold / counts / report_paths 同既有 partial 面 }`、`Interrupted` → 既有 `conclusion = Error` + 「突变执行中断」文案 + `fault: None`；③ `skip_outcome` / pass / fail 三处字面量补 `fault: None` | AC-7 / D17；`Fatal` → `Err` 零改；`SEGMENT_SPAWN_TIMEOUT_MS` 值与其文案**零改**（见待决问题 2） |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | `RunOptions` 增 `retry: exec::RetryPolicy::none()`（一处一行） | 随动（D17 零重试）；proposal 未列，但 `retry` 为必填字段 |
| `crates/infra/checks/src/mutation/stryker.rs` | 同上（一处一行） | 随动（D17 零重试）；proposal 未列，同上 |
| `crates/core/orchestration/src/port.rs` | ① 新增 `pub enum RunFault { Stopped, Environment(String) }`（`derive(Debug, Clone, PartialEq, Eq)`、零 `Serialize`、doc 明写「不出线」）；② `StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome` 各增 `pub fault: Option<RunFault>`（doc：`None` = 结论本身有效）；③ 三 runner trait 的取消 doc 改写为「置位 → 进程树击杀 → 实现侧返回 `fault = Some(RunFault::Stopped)`（MUST NOT 返回 `Err`）」——MUST NOT 留「`Err("执行已取消")`」第二套口径 | AC-9 / D18；`ToolStepOutput` / `ToolCommand` / `ToolStepRequest` 与三 trait **签名**零改（proposal「不改三 runner port 签名」） |
| `crates/core/orchestration/src/walker.rs` | ① 新增两文案常量 `TXT_ENV_STATIC` / `TXT_ENV_TEST`（D16，写在 `TXT_BASELINE_*` 段之后，注释说明与基线文案的可辨关系）；② 新增私有 `fn settle_fault(...) -> Terminal`（D15）；③ 五处步执行体在 `run_tool` 之后、既有 `passed` / `conclusion` 分支**之前**插入 `if let Some(fault) = outcome.fault.as_ref() { return Err(settle_fault(..)); }`：`step_static_check`（`:923`，反馈计数不 +1、零 worker 调用）、`step_static_check_terminal`（`:1131`，不落基线红文案）、`step_test_execution_escalate`（`:1055`，不代写 fail phase_log）、`step_test_execution_terminal`（`:1194`）、`step_mutation_test_escalate`（`:1281`，`template = None`，仅 `Stopped` 面可达）；④ 相应步函数 doc 补「fault 先于结论」 | AC-5 / AC-6 / AC-7；`run_tool` / `check_cancel` / `StepSettle` / `Terminal` / `emit_node` / `drive` 结构与节点词汇零改 |
| `packages/desktop/package.json` | `version` `"0.5.24"` → `"0.5.25"`（单行） | AC-11 / D20 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|--------|----------|-----------|----------|------|
| `is_transient` | `crates/infra/exec/src/transient.rs`（经 `lib.rs` 再导出） | 新增 | `pub fn is_transient(report: &RunReport) -> Option<&'static str>` | 三合一判据（`Completed` + `exit_code != 0` + 合并输出命中签名），命中返表中原文；`TimedOut` / `Cancelled` 恒 `None`（D5） |
| `RetryPolicy::none` | 同上 | 新增 | `pub fn none() -> RetryPolicy` | 单次执行：`max_attempts = 1`，`backoff` / `total_budget` 取同类常量（不消费） |
| `RetryPolicy::transient` | 同上 | 新增 | `pub fn transient() -> RetryPolicy` | `max_attempts = 5`（首跑 + 4 次重试）、`backoff = [1 s, 2 s, 4 s, 8 s]`、`total_budget = 1500 s` |
| `run_shell` | `crates/infra/exec/src/run.rs` | 修改（签名不变，语义扩展） | `pub async fn run_shell(command: SystemCommand, options: RunOptions) -> Result<RunReport, RunError>` | 入口短路 → 重试循环（预算 / 上限 / 退避 / 逐次取消观测）→ 尝试面装配；`RetryPolicy::none()` 或未命中签名时逐字节等价于单次执行（D1–D4） |
| `locked_paths` | `crates/infra/exec/src/lockprobe.rs` | 新增 | `pub fn locked_paths(diagnostics: &str) -> Vec<PathBuf>` | 三错误串形态解析、去重、≤8（D9）；零 IO |
| `lock_holders` | 同上 | 新增 | `pub fn lock_holders(paths: &[PathBuf]) -> Vec<LockHolder>` | Windows Restart Manager 单 session / 非 Windows `lsof`；按 `(name, pid)` 去重；失败与空清单均返回空 vec（不 `Err`、不 panic，D8） |
| `attribution_text` | 同上 | 新增 | `pub fn attribution_text(diagnostics: &str) -> Option<String>` | 归因原料单点：`占用进程：…` / `被锁文件：…`（或「未定位占用者（可能为内核过滤驱动（安全软件），无法归因到用户进程）」）；解析不出路径 → `None`（D7 / D10） |

<!-- 私有 / pub(crate) 不列：`attempt_once` / `transient_signature`（run.rs 与 transient.rs 的判定核心）、`blocks_delete`（lockprobe.rs，平台两臂）、`attempts_log` / `fault_text`（checks::environment）、`fault_outcome`（testexec/mod.rs）、`settle_fault`（walker.rs）。
     三 runner trait 方法（`StaticCheckRunner::run` / `TestExecutionRunner::run` / `MutationRunner::run`）签名零改，仅文档口径更新，故不入本表。
     无 CLI 子命令 / 无 HTTP 端点 / 无新增 Tauri IPC 命令变更。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `RetryPolicy`（struct） | `crates/infra/exec/src/transient.rs` | 新增 | `pub max_attempts: u32`、`pub backoff: [Duration; 4]`、`pub total_budget: Duration`；`derive(Debug, Clone, PartialEq, Eq)`；字段 `pub` 供 `run.rs` 兄弟模块读取与测试断言（构造面经两 fn 收口）；参数 MUST NOT 进 config schema |
| `AttemptTrace`（struct） | 同上 | 新增 | `pub exit_code: i32`、`pub transient: Option<String>`（命中签名原文，未命中 `None`）、`pub stdout: Vec<u8>`、`pub stderr: Vec<u8>`；`derive(Debug, Clone, PartialEq, Eq)`；只承载**此前**各次尝试 |
| `RunOptions`（struct） | `crates/infra/exec/src/run.rs` | 修改 | 增 `pub retry: RetryPolicy`；字段集 `{ cwd, env, timeout /* 必填 */, cancel, retry }`；仍无 `Default`、`derive(Debug, Clone)` |
| `RunReport`（struct） | 同上 | 修改 | 增 `pub attempts: u32`（≥1）与 `pub retried: Vec<AttemptTrace>`（末次不入）；`stdout` / `stderr` 仍为末次尝试字节；`derive(Debug, Clone, PartialEq, Eq)` |
| `LockHolder`（struct） | `crates/infra/exec/src/lockprobe.rs` | 新增 | `pub name: String`（`strAppName` / `lsof` 命令名）、`pub pid: u32`；`derive(Debug, Clone, PartialEq, Eq)` |
| `RunFault`（enum） | `crates/core/orchestration/src/port.rs` | 新增 | `Stopped` / `Environment(String)`；`derive(Debug, Clone, PartialEq, Eq)`；**零 `Serialize`**（不出线，D18） |
| `StaticCheckOutcome`（struct） | 同上 | 修改 | 增 `pub fault: Option<RunFault>`；`passed` / `diagnostics` / `output_ref` / `report_dir` 取值面零变；`derive` 不变（`Eq` 仍成立） |
| `TestExecutionOutcome`（struct） | 同上 | 修改 | 增 `pub fault: Option<RunFault>`；`conclusion` 三值面零变（D12 占位规则）；`derive` 不变 |
| `MutationTestOutcome`（struct） | 同上 | 修改 | 增 `pub fault: Option<RunFault>`；仍零 `Serialize`；`derive` 不变（`PartialEq`，无 `Eq`——f64 字段） |
| `PlanExecution`（struct，`pub(crate)`） | `crates/infra/checks/src/testexec/runner.rs` | 修改 | 增 `pub fault: Option<RunFault>`；`error` 语义收窄为「被测代码 / 工件 / harness 面」，env 面恒 `None` |
| `SegmentFailure`（enum，`pub(crate)`） | `crates/infra/checks/src/mutation/mod.rs` | 修改（仅文档 + 映射） | 三变体集合零变；`Cancelled` 的收敛口径改判 `fault = Stopped`（D17） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.25`（原 `0.5.24`） | 用户可见变更提升补丁号（AC-11 / D20）；`tauri.conf.json` 经 `../package.json` 跟随 |
| 重试策略三常量（`RETRY_MAX_ATTEMPTS` / `RETRY_BACKOFF` / `RETRY_TOTAL_BUDGET`） | `crates/infra/exec/src/transient.rs` | 代码常量（非配置面） | `u32` / `[Duration; 4]` / `Duration` | `5` / `[1 s, 2 s, 4 s, 8 s]` / `1500 s` | spec「参数 MUST NOT 进 config schema」：与既有超时预算同性质，调整属代码常量变更（评审可见） |
| `windows-sys` 依赖声明 | `crates/infra/exec/Cargo.toml` | 新增（`[target.'cfg(windows)'.dependencies]`） | `version = "0.61"` + `features = ["Win32_System_RestartManager"]` | — | 仅 Windows 目标；非 Windows 零新增编译依赖（见「依赖」） |
| `openspec/config.json` | — | 否（零改动） | — | 既有声明不变 | 无新增 schema 字段、无阈值调整 |

---

## 数据模型

持久化面**零结构改动**（无表、无索引、无迁移）。本变更只涉及三类**进程内载体**与一处**落盘记录的内容形态**：

```
exec::run_shell（每次调用）
└── RunReport
    ├── outcome / exit_code / stdout / stderr      ← 既有面（stdout/stderr = 末次尝试）
    ├── attempts: u32                              ← 新增：实际尝试次数（≥1）
    └── retried: Vec<AttemptTrace>                 ← 新增：此前各次尝试（末次不入）
          └── AttemptTrace { exit_code, transient: Option<String>, stdout, stderr }

checks 装配（每次门禁执行）
├── StaticCheckOutcome.fault   : Option<RunFault>  ┐
├── TestExecutionOutcome.fault : Option<RunFault>  ├─ walker 先查 fault 再查结论
└── MutationTestOutcome.fault  : Option<RunFault>  ┘

core::orchestration::port::RunFault
├── Stopped                                     → 节点 stopped  → run Stopped
└── Environment(String)                         → 节点 failed   → run Failed（不升格、不注回 executor）
```

| 面 | 载体 | 语义 / 生命周期 |
|----|------|-----------------|
| 尝试面 | `RunReport.attempts` / `RunReport.retried` | 单次 `run_shell` 的内存产出，不持久化；`retried` 只在命中瞬态签名时累积（未命中恒空 → 最坏 5 份同量级输出，record 落盘后随 `RunReport` 释放） |
| 归因面 | 三 outcome 的 `fault` | 进程内载荷，随 `ToolStepOutput` 到 walker 即止；零 `Serialize`、零 db、零 IPC（D18） |
| 输出记录（形态变更） | `<report_dir>/exec-outputs/<exec_id>.log` | 单次尝试：**逐字节同现状**（`stdout ++ stderr`，零分隔头）；发生重试：逐次一段 `===== attempt {n}/{max} · exit={exit_code} · transient={签名或 -} =====` + 该次合并流（`retried` 按序在前、末次在后）。`record_exec_output` / `read_exec_output` / `mint_exec_id` 与文件命名零改（D11） |
| 报告树（结构零变） | `reports/<slot>/<plan_id>/{report.json, results.txt, coverage*.json}`、`summary.json` | 字段集零改：`exit_code` 仍以 `-1` 表示超时 / 缺码；`error` 面收窄为被测代码 / 工件 / harness 面；**env fault 时整链不写 summary**（半套报告不进复用门，D13） |
| mutation partial 面（零变） | `MutationTestOutcome.{score, threshold, counts, report_paths}` | 取消面仍回填已产报告路径与原临时面保留纪律（`report.rs` / Stryker sandbox 清理口径零改） |

关系面上无新增外键 / 无跨表引用：`RunFault` 与 `RunHistory` / `run_history` 表无交集，`ChangeRunStatus` / `ChangeStepStatus` 词汇零扩展（`stopped` 词已在位）。

---

<!-- 路由 / API 设计：本变更不涉及。无新增 / 修改 Tauri IPC 命令、无 HTTP 端点、无 CLI 子命令；`RunUpdate` / `RunNodeRecord` / 快照 DTO 零扩展，`src/types/generated/bindings.ts` 零 diff（AC-9）。 -->

---

## 依赖

### 运行时依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| `exec` → `windows-sys`（`[target.'cfg(windows)'.dependencies]`） | **新增**（`version = "0.61"`，features `["Win32_System_RestartManager"]`） | 占用者点名：`RmStartSession` / `RmRegisterResources` / `RmGetList` / `RmEndSession` / `RM_PROCESS_INFO`。仅 Windows 目标解析；feature 按需最小（D6 / D8） |
| `exec` → std `std::os::windows::fs::OpenOptionsExt` | 零新增依赖 | 精确探针的 `access_mode` / `share_mode`（`DELETE \| FILE_READ_ATTRIBUTES` / 全共享），不引 `Win32_Storage_FileSystem` / `Win32_Security` |
| `exec` → `tokio` | 不变（`process, io-util, time, macros`） | 重试循环复用既有 `select!` / `sleep` / `timeout`；**不引** `rt` / `rt-multi-thread`（仍不 `tokio::spawn`） |
| `exec` → `foundation` | 不变（既有边） | `CancelFlag` 逐次观测 |
| `checks-runtime` → `exec` / `orchestration` | 不变（既有边） | 重试策略 / 探针 / `ProcessOutput` 复用（`fault_text` 的诊断摘要走 `ProcessOutput::error_excerpt` / `tail`，不自造截断口径）+ `RunFault` |
| `orchestration` | 零新增依赖 | `RunFault` 为 std-only enum |
| 新增第三方 crate（除 `windows-sys`） | **无** | 瞬态判定 / 探针判据 / 归因装配全为 std |

### 构建 / 测试依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| `exec` dev-dependencies | 不变（`tempfile` + `tokio{rt,macros,test-util}`） | 重试 / 预算用例沿用 `start_paused` + 手动 `advance` 先例；`lockprobe_test` 只用 std + 本 crate 私有面（`super::blocks_delete`），**不**为测试新增导出（AGENT.md 纪律） |
| `checks-runtime` dev-dependencies | 不变 | 已有 `tokio(test-util)` / `workflow` / `tempfile`；static / testexec / mutation 用例沿用既有假可执行体装置 |
| `orchestration` dev-dependencies | 不变 | 已有 `store` / `tempfile` / `tokio(rt, macros)`；walker 用例以可编程假 runner 直给 `fault` |

---

## 验收标准回应

| AC | 设计落点 |
|----|----------|
| AC-1 | `RetryPolicy::transient()` 三常量（配置表）；重试循环上限 / 退避 / 预算（D1–D3）；`RunReport.attempts` / `retried`（类型表）；预算早停返末次结果（D2「预算耗尽」臂） |
| AC-2 | `is_transient` 三合一判据使 `TimedOut` / `Cancelled` 恒不命中（D5）；`attempts == 1 && retried.is_empty()` 的单次路径与现状逐字节等价（`retry: none()` + `attempt_once` 骨架复用，D1） |
| AC-3 | `lockprobe::{locked_paths, lock_holders, attribution_text}`（D7–D9）+ `environment::fault_text`（D10：占用进程 / 被锁文件 / 「已自动重试 N 次」/「排除 workspace 父目录（逐 worktree 无效）」）+ `TXT_ENV_*` 经 `{fault}` 嵌入（D16） |
| AC-4 | 探针判据固定为「`DELETE \| FILE_READ_ATTRIBUTES` 访问权 + 全共享打开一次即关闭」，仅 `raw_os_error == 32` 判阻塞；零 `create` / `truncate` / `write` / 独占共享模式（D6）；非 Windows 臂不探针（D6） |
| AC-5 | `settle_fault` 前置早退（D15）：`Environment` → `Terminal::failed` + 节点 failed，**不**走反馈边计数 / 不代写 fail phase_log / 不进决策重试 / 零 worker 调用 |
| AC-6 | `TXT_ENV_STATIC` / `TXT_ENV_TEST` 与 `TXT_BASELINE_*` 三常量零共用句（D16）；`step_*_terminal` 的 fault 分支先于基线红分支（修改文件 walker 行 ③） |
| AC-7 | 三面取消均落 `fault = Stopped`（static_check.rs 臂、runner.rs 臂、mutation `Cancelled` 映射，D17）；walker 五处 `stopped` 分派（D15）→ 节点 stopped、run `Stopped` |
| AC-8 | static-check `2>&1` 与 `stderr` 不读（诊断行口径零改）；testexec / mutation 合并流顺序零改；`PlanExecution.error` 收窄、单次尝试 record 逐字节同现状（D11）；无配置直过 / `Err` / `Fatal` 分流零改（`settle_fault` 只拦 fault 面） |
| AC-9 | `RunFault` 零 `Serialize`、三 outcome 不出线（现状 bindings.ts 无这三型）、`ToolStepRequest` / 三 trait 签名零改（D18） |
| AC-10 | 三 outcome 字面量补 `fault: None` 的机械面清单见 tasks 末段自查；新模块三处（`transient` / `lockprobe` / `environment`）均以私有面设计（可被同模块 `#[cfg(test)]` 子模块直驱，无需测试专用导出） |
| AC-11 | `package.json` 0.5.24 → 0.5.25（配置表）；`plugins/**` 零改动；`windows-sys` 仅 `cfg(windows)` 目标依赖 + 单 feature（依赖表） |

---

## 待决问题

1. **分隔头的单次尝试豁免（须在规格相位回写）**。`desktop-checks-domain` / `desktop-test-execution` 写「内容 SHALL 为全部尝试按序拼接、**每段前**加分隔头」，字面上含单次尝试；但 AC-8 要求既有 `static_check_test` / `testexec/mod_test` 的「record 内容 == 输出」断言全绿，恒加分隔头会破。本设计取「发生重试才加分隔头」（D11）。建议 spec 相位补一句「`attempts == 1 && retried.is_empty()` 时不加分隔头（逐字节同单次执行）」。
2. **`SEGMENT_SPAWN_TIMEOUT_MS` 的事实偏差**。`desktop-mutation-testing` delta 写「维持 1800 s」与「段执行超时（1800 秒）强制终止」，工作树实值为 `3_600_000 ms`（文案实际呈现「3600 秒」）。本设计**不动**该值与该文案（proposal 范围裁决），建议 spec 相位改为「维持现值（3600 s）」或去掉具体数字。若评审要求对齐 1800 s，属预算变更，需另立项裁决。
3. **fault 在位时 `conclusion` 的占位取值**。`desktop-test-execution` / `desktop-mutation-testing` 只说「fault 在位时 `conclusion` 值不参与门禁分派」，未定取值。本设计取 `Fail`（D12：`Error` 成因面已收窄，`Pass` 有被误读为绿的风险）。建议 spec 相位明写「fault 在位时取 `Fail` 占位」。
4. **`is_transient` 的导出清单**。proposal「变更范围 - 实现文件」的 `lib.rs` 行未列 `is_transient`，但 `infra/checks` 是唯一调用方且无替代形态（D19）。本设计加导出；若评审坚持按清单收口，备选是把判定做成 `RunReport::transient_signature()` 方法（改类型契约，范围更大）。
5. **`TXT_ENV_STATIC` 是否带报告目录**。spec 只对 test-execution 面点名「⑤ 报告目录」。本设计两侧都带（「MUST 含」为下界，static 侧报告目录可定位 record）；若评审要求严格按面区分，去掉 static 模板的 `{dir}` 即可（一行）。
6. **探针判「未阻塞」时的用户可见措辞**。当前退化为「对全部解析路径点名」，不额外提示「占用可能已解除」。若产品希望明示该态（如「探针判定：这些文件当前已可删除」），需在 `attribution_text` 增一行——本设计不预铺（无验收锚）。
7. **`lsof` 的调用形态**。取逐路径 `lsof -F pc -- <path>`（解析 `p`/`c` 行）；若目标机上 `lsof` 对目录 / 被删文件行为异常，退化为空清单即可（不阻断）。可按评审意见改为 `lsof +D` 等形态，不影响任何验收面。
8. **`cargo` 的版本解析面**。`windows-sys = { version = "0.61" }` 落直接依赖后需确认 `Cargo.lock` 不新增版本行（锁内已有 `0.61.2`）；若解析出第二版本需钉 `=0.61.2`（沿 `native_db = "=0.8.2"` 先例）。
