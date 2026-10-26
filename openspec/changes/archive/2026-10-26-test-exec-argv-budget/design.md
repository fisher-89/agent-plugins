# 设计: test-exec-argv-budget

> **变更**: test-exec-argv-budget
> **日期**: 2026-10-26
> **基于**: `proposal.md`（L1–L4 根因分层、处置二/三/四）、`explore.md`（只读）与 `specs/{desktop-test-execution,desktop-checks-domain,desktop-change-orchestration}/spec.md`
> **前置**: `desktop-build-mode-split` 已把 `openspec/config.json` 的 vp suite `includes` 收窄为 `src/**/*.test.{ts,tsx}`（L1 已止血）；`static-check-file-lock` / `exec-run-shell` / `test-execution-zero-agent-decision` 已把 `RunFault` 面、「fault 先于结论」收口、`record_exec_output` 记录单点建齐。
> **本篇边界**: 只改 `crates/infra/checks` 三文件 + `environment.rs` + `package.json`（+ 一处 `openspec/todo.md` 登记）。**零新模块、零新依赖、零 config / schema 扩展、零 IPC / bindings 变更、零 walker / core 纯层改动**。

---

## 提案与规格同步状态

proposal 的三面处置（L2 预算裁决 / L3 归因修正 / L4 诊断可读性）与三份 delta spec 已定稿。本设计只做四件事：

1. 把每条处置落到**唯一位点**并给出精确形态（spec 只钉语义、未钉实现细节的四处在此收敛：裁决在 `execute_plan` 内的**确切插入位**、`decide_argv` 的完整参数面、`ExpandMode::OmitFiles` 的剥离算法、plan 头的**装配单点**）；
2. 给出 `omit_files_safe` 的「集合相等」在既有 `detect_files` 上的**零新方言**落法（同一收集管线加一个 `apply_excludes` 开关）；
3. 把「降级说明不准进 findings」（explore 原方案的硬缺陷，proposal R-4）写成**结构性不可达**而非纪律约束（信息面只有记录头一条通路）；
4. 对 `argv_budget_text` 的 spec 三参签名作一处**必要细化**（降级无效臂需两个长度），并在待决问题里显式登记。

**起点实况核实**（工作树实况；路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）：

- **展开单点**：`crates/infra/checks/src/testexec/runner.rs:195-221` `fn expand_template(template, report_dir, results_file, coverage_file, config_args, files) -> String`——五占位符逐字 `replace`；`{config_args}` 未注入时以 `while find` + `trim_end` 连前导空白剥离（`:206-215`，本设计 `OmitFiles` 复用**同一算法**）。
- **执行链**：`runner.rs:59-182` `execute_plan(plan, report_dir, cancel) -> Result<PlanExecution, String>`——清目录（`:67-72`）→ 占位符原料 POSIX 化（`:75-79`）→ `expand_template`（`:84-91`）→ `ensure_program_resolvable`（`:94`）→ `exec::run_shell`（`:98-109`）→ `attempts_log`（`:110`）→ 退出码哨兵（`:112-115`）→ fault 三臂（`:119-137`）→ **fault 早返**（`:138-149`）→ 真红 `execution_error`（`:153-159`）→ 工件解析（`:162-170`）→ 正常返回（`:172-181`）。
- **结果载荷**：`PlanExecution`（`runner.rs:31-55`）**八字段**（`exit_code` / `duration_ms` / `cases` / `source_files` / `measured` / `error` / `output` / `fault`）；构造点仅两处（`:139` 早返、`:172` 正常）。
- **detect**：`detect.rs:252-296` `fn detect_files(project_root, abs_root, abs_cwd, suite, entry) -> Result<Vec<String>, String>`——includes 缺省吃 `entry.default_glob`（`:259-262`）→ 花括号展开（`:267`）→ `glob` → `is_pruned_path`（`:282`）→ `is_excluded_by_suite`（`:283`）→ cwd 相对 POSIX 入 `BTreeSet`（`:288-291`）；`TestPlan`（`detect.rs:146-170`）**十二字段**，构造点单处（`:234-246`）。
- **聚合与记录**：`testexec/mod.rs:154-182` 逐 plan 循环（`merged_output.extend_from_slice(&execution.output)` 在 `:171`）→ fault 收口 `:187-196`（`fault_outcome`，`:259-279`，**不写 summary / 不跑诊断 / 不落报告**、`exec_id` 照旧回填）→ 复用分支 `:131-140`（零 plan，`&[]` 空合并流）→ `outcome`（`:226-252`）经 `exec::record_exec_output` 落单 `.log`。`build_sub_report`（`:461-505`）的 `findings: execution.error.map(|error| vec![error])`（`:503`）是 findings 唯一注入面。
- **环境文案单点**：`crates/infra/checks/src/environment.rs`（`mod environment;`，crate 内可见）——`attempts_log`（`:37-66`）/ `fault_text`（`:96-143`）/ `progress_line`（`:152-166`）/ `ENV_GUIDANCE`（`:23-25`，安全软件排除指引）/ `clip_chars`（`:169-175`）。三臂文案面**零 `argv` 相关**（本篇新增两臂）。
- **walker 面**：`crates/core/orchestration/src/walker.rs:73-74` `TXT_ENV_STATIC` / `TXT_ENV_TEST` 均以 `{fault}` 占位嵌摘要；`settle_fault` 收敛 `Environment → Terminal::failed`。分型只发生在**摘要装配方**（checks 域），walker 零改动。
- **版本面**：`packages/desktop/package.json:3` = `0.5.31`。

**只读红线零触点**：`crates/core/checks/**`（含 `aggregate.rs::collect_problems` / `in_suite_scope`）· `crates/core/orchestration/**` 生产源码 · `crates/infra/exec/**` · `testexec/report.rs` · `openspec/config.json` · `packages/desktop/src/**` 与 `src/types/generated/bindings.ts` · `crates/core/config/config.schema.json` · `testexec/detect.rs` 的 `REGISTRY` 五模板 · `explore.md`。

---

## 三面处置 → 落点

| # | 处置 | 落点 | 关键决策 |
|---|------|------|----------|
| P1 | 展开两态（`WithFiles` / `OmitFiles`） | `testexec/runner.rs`（`ExpandMode` + `expand_template` 扩参） | D3 / D4 |
| P2 | 平台命令行预算与三臂裁决 | `testexec/runner.rs`（`CMD_LINE_BUDGET` + `line_budget` + `ArgvDecision` + `decide_argv` + `execute_plan` 接入 + `PlanExecution` 两字段） | D1 / D2 / D5 / D6 |
| P3 | 降级安全判据（集合相等） | `testexec/detect.rs`（`collect_files` 重构 + `TestPlan.omit_files_safe`） | D7 |
| P4 | 命令行超限归因文案两臂 | `crates/infra/checks/src/environment.rs`（`argv_budget_text` / `argv_degrade_note`） | D8 / D9 |
| P5 | 拒绝臂收口（不写半套报告） | `testexec/runner.rs`（沿既有 fault 早返形态）+ `testexec/mod.rs`（既有 `fault_outcome`，零新分支） | D6 |
| P6 | 记录 plan 分隔头 | `testexec/mod.rs`（`push_plan_head` 单点 + 循环内先头后 output） | D10 / D11 |
| P7 | 版本面与遗留登记 | `packages/desktop/package.json`、`openspec/todo.md` | D13 |

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 预算常量与取臂 | `#[cfg(windows)] const CMD_LINE_BUDGET: usize = 7000;`（doc：`cmd.exe` 硬上限 8191 − 1191 余量 = `cmd /C ` 前缀 + `raw_arg` 转引放大 + 版本差异；实测脚本 8000 / 8150 通过、8200 拒绝）＋ `fn line_budget() -> Option<usize>`（Windows 臂 `Some(CMD_LINE_BUDGET)`、非 Windows 臂 `None`，用 `#[cfg(windows)]` / `#[cfg(not(windows))]` 语句级取臂）。长度口径 `chars().count()`（多字节路径按字符计，与 cmd 的字符计数同口径）。**代码常量，MUST NOT 进 config schema**（与 `SUITE_COMMAND_TIMEOUT` / `TESTEXEC_TOTAL_BUDGET` 同性质） | spec「预算常量」「非 Windows 臂无预算」逐字；把平台分叉压到**一行取常量**，裁决逻辑全部经预算入参流转，使三臂在全平台可测（R-8 缓解） |
| D2 | 裁决位点与长度取数 | `execute_plan` 内插在 `expand_template(.., ExpandMode::WithFiles)`（现 `:84-91`）之后、`ensure_program_resolvable`（现 `:94`）之前：`let argv_len = expanded.chars().count();` → `decide_argv(..)`。`report_dir` 清空重建仍留在最前（既有次序零改；拒绝臂不产工件，空目录无害，且 fault 收口本就不写 summary / 不进复用门）。`spawn` 只在 `Keep` / `Omit` 两臂可达 | spec「展开之后、程序可达性检查之前单点执行」「MUST NOT 以未经裁决的超长命令串直接 spawn」：`ensure_program_resolvable` 会 `split_whitespace().next()` 解析程序名，超长串在上面浪费一次解析且语义错序（长度是模板属性、可达性是环境属性） |
| D3 | `ExpandMode` 两态语义 | `enum ExpandMode { WithFiles, OmitFiles }`（模块私有，`runner_test` 经 `super::` 可见）。`WithFiles` = **现有行为逐字不变**（`files` 非空 `replace("{files}", &files.join(" "))`、空清单 `replace("{files}", "")`）。`OmitFiles` = 完全复用 `{config_args}` 未注入臂的既有算法：`while let Some(index) = result.find("{files}") { let before = result[..index].trim_end(); let after = &result[index + "{files}".len()..]; result = format!("{before}{after}"); }`——`{files}` 连其前导空白一并剥离，其前文字 `trim_end` 后与后续拼接。其余四占位符两态**逐字同义**；模板不含 `{files}`（rust 档）时两态输出**同串** | spec「`OmitFiles` 展开语义」「模板不含 `{files}` 时两态逐字相同」：与 `{config_args}` 未注入臂共用同一段代码形态，是本文件内既有的「占位符失效即连前导空白剥离」约定，不自造第二套 |
| D4 | `decide_argv` 的形态与臂序 | 模块私有**纯函数**（零 IO / 零 spawn / 零 async，预算与展开串全作入参）：`fn decide_argv(plan: &TestPlan, report_dir: &str, results_file: &str, coverage_file: &str, expanded: &str, budget: Option<usize>) -> ArgvDecision`。臂序（**先短路、后降级**）：①`budget == None` → `Keep { command: expanded.to_owned() }`；②`expanded.chars().count() <= budget` → `Keep`（**原串回填，不二次展开**）；③`!plan.omit_files_safe` → `Reject { message: environment::argv_budget_text(budget, expanded_len, None, plan.files.len()) }`；④`OmitFiles` 重新展开得 `omitted`，`omitted.chars().count() > budget` → `Reject { message: environment::argv_budget_text(budget, expanded_len, Some(omitted_len), files) }`；⑤否则 `Omit { command: omitted, note: environment::argv_degrade_note(budget, expanded_len, files) }`。`ArgvDecision`（模块私有）：`Keep { command: String }` / `Omit { command: String, note: String }` / `Reject { message: String }` | spec 三臂表 + 「降级无效仍拒绝」：① 使 unix 臂「零降级」在**逻辑上**而非仅在常量上成立；② `Keep` 回填入参串（不重展开）是 AC-2「命令串与改造前逐字符一致」最硬的保证；③④ 顺序固定——先判「能不能降级」再花一次展开成本，且不安全臂的文案不掺「降级后仍超」的噪声 |
| D5 | 拒绝臂的收口与占位 | `Reject` → `decide_argv` 之外**不 spawn**，直接构造 `PlanExecution { exit_code: EXIT_CODE_NO_STATUS, duration_ms: <elapsed>, cases: Vec::new(), source_files: Vec::new(), measured: None, error: None, output: Vec::new(), fault: Some(RunFault::Environment(message)), argv_len, argv_note: None }` 早返（**MUST NOT 返回 `Err`**、MUST NOT 走 `non_zero_exit_error`、MUST NOT 解析工件）。`exit_code` 取既有「进程无退出码」哨兵 `-1`（与超时 / 取消同口径，fault 面下该值不进结论） | spec「环境归因」：沿 `runner.rs:138-149` 既有 fault 早返形态（`error` 恒 `None`、工件解析跳过），`mod.rs` 侧完全复用既有 `fault_outcome`（不写 summary / 子报告 / 诊断、`report_dir` + `exec_id` 回填），零新增收口路径 |
| D6 | 降级与拒绝的**双向**不污染面 | `Omit` 臂只在 `PlanExecution.argv_note` 留痕；`Reject` 臂只在 `fault` 留痕。**两臂都 MUTATE 不到 `error`**，故 `build_sub_report`（`:503`）与 `aggregate.rs::collect_problems`（`failed == 0 ∧ findings 非空 → execution_error`）在结构上不可达——不是靠纪律，是靠「信息面没有第二条通路」（R-4 缓解） | spec「降级说明 MUST NOT 进入 `SubReport.findings` / `summary.findings`」「纯降级场景的 `conclusion` / `problems` MUST 零变化」：explore 原方案把降级说明写进 findings 会把绿跑翻红，本设计把它从「要不要写」变成「写不到」 |
| D7 | 降级安全判据（集合相等，detect 期一次算定） | `detect.rs` 把 `detect_files` 的收集主体抽为 `fn collect_files(project_root: &Path, abs_root: &Path, abs_cwd: &Path, suite: &config::TestSuite, patterns: &[String], apply_excludes: bool) -> Result<BTreeSet<String>, String>`（`detect_files` 变为薄包装：`patterns` 由 includes / `default_glob` 算出、`apply_excludes = true`）；`detect_plans` 在 `files` 非空后再调一次 `collect_files(root, &abs_root, &abs_cwd, suite, &[entry.default_glob.to_owned()], false)` 得对照集合，`omit_files_safe = (对照集合 == files 集合)`（两侧均为 `BTreeSet` → 排序去重后 `Vec` 相等即集合相等）。**零新 glob 方言、零新剪枝**（花括号展开 / `glob` / `is_pruned_path` 全复用）；`excludes` **不施加**于对照面（框架自发现不认这套 excludes）。`TestPlan` 增 `omit_files_safe: bool`（doc 写明判据与「保守」方向） | spec「降级安全判据（集合相等，MUST NOT 模式文本比较）」「MUST NOT 在 runner 侧二次自建 glob 枚举」：`apply_excludes` 开关是**最小**的判据面（若在 runner 侧重枚举就必须把 glob 方言与剪枝复制一份，那正是 spec 禁止的两写）；代价是每 suite 多一次 glob 走盘（`detect` 已有同量级走盘，可接受） |
| D8 | 文案两臂的形态与参数 | `environment.rs` 新增 `pub(crate) fn argv_budget_text(budget: usize, expanded_len: usize, omitted_len: Option<usize>, files: usize) -> String`（拒绝面）与 `pub(crate) fn argv_degrade_note(budget: usize, expanded_len: usize, files: usize) -> String`（降级面）。拒绝面文本 = ①定性（「命令行长度超平台上限，属 harness 限制，非被测代码问题」）＋②数字（展开 `expanded_len` 字符 / 平台预算 `budget` 字符 / `files` 个测试文件；`omitted_len == Some(n)` 时追加「降级为框架自发现后仍为 `n` 字符」）＋③动作（「请收窄该 suite 的 `includes` 或拆分 suite」）。降级面文本 = 「展开 `expanded_len` 字符超平台预算 `budget` 字符，已降级为框架自发现（`files` 个测试文件）」——**零动作建议**。两臂 **MUST NOT** 引用 `ENV_GUIDANCE` / 占用者 / 被锁文件 / 重试次数措辞 | spec checks-domain「文案内容三项」「MUST NOT 复用安全软件指引」「两臂措辞分工」逐字。**`omitted_len` 是对 spec 三参签名的必要细化**（规格正文未说「降级后长度」如何入参，而 test-execution 的「降级无效仍拒绝」scenario 硬要求「文案附原始长度与降级后长度两个数字」）——见待决问题 1 |
| D9 | 既有文案与 `attempts_log` 零改 | `fault_text`（瞬态 / 超时归因）/ `attempts_log`（尝试分隔头）/ `progress_line` / `ENV_GUIDANCE` / `clip_chars` **逐字零改**；模块 doc 由「三个职责」改为「四个职责」（补一条两臂：拒绝面摘要 + 降级面记录注） | spec「既有文案零变（回归锚）」：同模块不同成因各自可辨，`fault_text` 的逐字不动是 checks-domain delta 的回归锚 |
| D10 | 记录 plan 头的装配单点 | `testexec/mod.rs` 新增模块私有 `fn push_plan_head(merged: &mut Vec<u8>, plan: &TestPlan, execution: &runner::PlanExecution)`：写入 `format!("\n=== plan {} ({}) · files={} · argv={} 字符 ===\n", plan.id, plan.framework, plan.files.len(), execution.argv_len)`，`execution.argv_note.as_deref()` 为 `Some(note)` 时紧随 `format!("[argv] {note}\n")`。调用位点：`execute` 的逐 plan 循环内、`merged_output.extend_from_slice(&execution.output)`（现 `:171`）**之前**——先头后正文。**每个 `execute_plan` 返回 `Ok` 的 plan 都写头**（含 `fault` 在位者：超时 / 取消 / 拒绝臂），plan 序 = `plans` 迭代序（字典序稳定）。复用分支（`:131-140`，零 plan 执行）传 `&[]`，记录仍为空串 | spec「plan 分隔头」「空输出多 plan 仍有头」「零 plan 执行的复用分支记录仍为空」：头与正文分离在同一处两行代码内，避免散落；`fault` 在位也写头正是 R-2 / L4 的要点（被拒绝的那次展开长度必须可回查） |
| D11 | 头格式的字面口径 | 头行：以换行起手、以换行收尾、单行；分隔符为 ` · `（U+00B7，与 `environment.rs` `push_attempt` 的 `·` 及本仓报告文案同族）；`argv=` 与 `files=` 均为十进制整数，`字符` 为单位后缀；`[argv]` 行紧随降级 plan 的头（自身亦以换行收尾）。**去掉头与 `[argv]` 行后，记录正文与改造前脚本输出逐字节一致** | spec「分隔头与 `[argv]` 行为记录元数据」「去掉这些行后正文逐字节一致」：单一格式串（无分支拼接），使「去掉头 → 逐字节相等」在测试里可用前缀剥离断言 |
| D12 | 平台无关可测性 | 预算经 `decide_argv` 入参流转（不读常量）、`ArgvDecision` 为纯数据、`ExpandMode` 两态与模板字符串可直接构造——三臂 / 两态 / 边界（`len == 预算` / `预算 + 1`）全部在任意平台可测；`cfg` 只覆盖 `CMD_LINE_BUDGET` 定义与 `line_budget()` 的一行取臂 | spec「预算作纯函数入参 → 表驱动可测、平台无关」（R-8 缓解）：覆盖率不在 Windows 单臂上塌陷 |
| D13 | 版本面与遗留登记 | `packages/desktop/package.json` `0.5.31` → `0.5.32`（`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` 版本行**不跟**）；`openspec/todo.md` 追加一条处置五登记（「`includes` 双语义解耦 + `in_suite_scope` 绝对路径归一，顺序：先 B 后 A」） | AGENT.md 版本纪律 + proposal R-5 / 遗留观察项 1 明写「实施期登记 `openspec/todo.md`」；该条为**文档登记**，不进代码、不进 spec |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 展开两态 + 预算裁决 | `ExpandMode` 两态展开；`CMD_LINE_BUDGET` / `line_budget()` 平台取臂；`ArgvDecision` 三臂与 `decide_argv` 纯函数；`execute_plan` 在 spawn 前裁决 | `crates/infra/checks/src/testexec/runner.rs`（修改） | `crate::environment`（两臂文案）、`super::detect::TestPlan`、`orchestration::port::RunFault`、`std` | std-only（零 tokio 新增、零第三方、零 IO 逻辑） |
| 降级安全判据 | `collect_files`（同一管线 + `apply_excludes` 开关）；`omit_files_safe` 集合相等一次算定 | `crates/infra/checks/src/testexec/detect.rs`（修改） | `glob`、`checks::globmatch::{match_glob, try_expand_braces}`、`foundation::prune`、`config::TestSuite` | 既有 detect 形态（零新方言） |
| 归因文案两臂 | `argv_budget_text`（拒绝面：定性 + 三数字 + 动作）/ `argv_degrade_note`（降级面：三数字 + 已降级） | `crates/infra/checks/src/environment.rs`（修改） | 无（纯函数，零 io / 零 async） | 纯字符串装配（沿用 `format!` 形态） |
| 记录头装配 | `push_plan_head`（`=== plan … ===` + 可选 `[argv]` 行），先头后正文 | `crates/infra/checks/src/testexec/mod.rs`（修改） | `PlanExecution` 两新字段、`TestPlan` | 既有聚合形态（零新分支路径） |
| 版本与登记 | patch bump + 处置五登记 | `packages/desktop/package.json`、`openspec/todo.md` | —— | 人工文本 |

---

## 变更清单

> 路径口径：`crates/...` 前缀省略 `packages/desktop/src-tauri/`（与 proposal「变更范围 - 实现文件」同口径）。

<!-- 新增文件：本变更零新增文件（三面处置全落既有四文件的落位：runner / detect / mod / environment，另加版本行与一处 todo 登记），故整段省略。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/checks/src/testexec/runner.rs` | ① 新增 `enum ExpandMode { WithFiles, OmitFiles }`（模块私有，doc 写明两态语义与「模板无 `{files}` 时同串」）；② `expand_template` 增加末位参数 `expand_mode: ExpandMode`，`{files}` 段按两态分支（`OmitFiles` 复用 `{config_args}` 未注入臂的 `while find` + `trim_end` 剥离算法）（D3）；③ 新增 `#[cfg(windows)] const CMD_LINE_BUDGET: usize = 7000;`（doc：8191 − 1191 余量的构成与实测依据、代码常量不进 config schema）与 `fn line_budget() -> Option<usize>`（D1）；④ 新增 `enum ArgvDecision { Keep { command: String }, Omit { command: String, note: String }, Reject { message: String } }`（模块私有）与 `fn decide_argv(plan: &TestPlan, report_dir: &str, results_file: &str, coverage_file: &str, expanded: &str, budget: Option<usize>) -> ArgvDecision`（五臂，D4；文案经 `crate::environment::{argv_budget_text, argv_degrade_note}` 取，**本文件零文案字面量**）；⑤ `PlanExecution` 增 `argv_len: usize`（`WithFiles` 展开字符数）与 `argv_note: Option<String>`（降级说明；未降级 `None`）两字段（doc 各写明用途与「不进 `SubReport.findings`」）；⑥ `execute_plan` 主体接入：`expand_template(.., ExpandMode::WithFiles)` → `let argv_len = …chars().count()` → `decide_argv(..)` → `Keep` 原串 / `Omit` 换串并携 `note` / `Reject` 早返 `fault = Some(RunFault::Environment(message))`（D2 / D5 / D6）；⑦ 既有两处 `PlanExecution` 构造点补 `argv_len` / `argv_note`（fault 早返臂 `argv_note: None`、正常臂携降级案）；⑧ `execute_plan` doc 的执行链补「展开 → **预算裁决** → 程序可达性 → spawn」 | P1 / P2 / P5；`non_zero_exit_error` / `parse_results` / `parse_coverage_artifact` / `ensure_program_resolvable` / `reachable_in` / `SUITE_COMMAND_TIMEOUT` / `VERSION_DETECT_TIMEOUT` / `EXIT_CODE_NO_STATUS` 语义零变；`exec::run_shell` 参数面零变 |
| `crates/infra/checks/src/testexec/detect.rs` | ① `detect_files` 主体抽为 `fn collect_files(project_root: &Path, abs_root: &Path, abs_cwd: &Path, suite: &config::TestSuite, patterns: &[String], apply_excludes: bool) -> Result<BTreeSet<String>, String>`（`apply_excludes == false` 时跳过 `is_excluded_by_suite`，`is_pruned_path` 恒施加），`detect_files` 退化为「算 patterns + `apply_excludes = true`」的薄包装；② `TestPlan` 增 `omit_files_safe: bool`（doc：判据 = `default_glob` 同管线（不施加 excludes）集合 == `files` 集合；保守方向 = 集合不等即拒降级）；③ `detect_plans` 在 `files.is_empty()` 早跳（`:223-225`）之后、构造 plan 之前调 `collect_files(.., &[entry.default_glob.to_owned()], false)` 与 `files` 比集合，结果写入 `omit_files_safe` | P3（D7）；`REGISTRY` 五模板 / `framework_name` / `unsupported_framework_reason` / `is_pruned_path` / `is_excluded_by_suite` / `posix_join` / 版本探测家族 / `derive_plan_id` **零改** |
| `crates/infra/checks/src/testexec/mod.rs` | ① 新增模块私有 `fn push_plan_head(merged: &mut Vec<u8>, plan: &TestPlan, execution: &runner::PlanExecution)`（头行 + 可选 `[argv]` 行，D10 / D11）；② `execute` 的逐 plan 循环内 `merged_output.extend_from_slice(&execution.output)` **之前**插 `push_plan_head(&mut merged_output, plan, &execution);`（fault 在位亦写头）；③ `outcome` / `fault_outcome` 的 doc 补一句「记录正文各段前有 plan 分隔头（元数据面）」 | P6；`build_sub_report` **零改**（`findings` 注入面不动，D6）、`fault_outcome` / `outcome` / 复用门 / `findings_*` / `plan_ids_match` / `newest_input_mtime_ms` / `prune_roots` / 六个 `DETAIL_*` 与 `FINDING_*` 常量**零改** |
| `crates/infra/checks/src/environment.rs` | ① 新增 `pub(crate) fn argv_budget_text(budget: usize, expanded_len: usize, omitted_len: Option<usize>, files: usize) -> String`（D8 拒绝面三要点，`omitted_len` 为 `Some` 时追加「降级为框架自发现后仍为 N 字符」）；② 新增 `pub(crate) fn argv_degrade_note(budget: usize, expanded_len: usize, files: usize) -> String`（D8 降级面：三数字 + 「已降级为框架自发现（K 个测试文件）」，零动作建议）；③ 模块 doc 由「三个职责」扩为「四个职责」，补一条「命令行超平台上限成因两臂（拒绝面摘要 / 降级面记录注）——成因与文件锁 / 超时不同，MUST NOT 引用 `ENV_GUIDANCE`」 | P4；`ENV_GUIDANCE` / `DIAGNOSIS_EXCERPT_LIMIT` / `attempts_log` / `push_attempt` / `fault_text` / `progress_line` / `clip_chars` **逐字零改**（D9） |
| `packages/desktop/package.json` | `version` `"0.5.31"` → `"0.5.32"`（单行） | D13；`src-tauri/tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 版本行与 `crates/core/config/config.schema.json` **不**随动（零 `TestFramework` / `KNOWN_KEYS` 扩展） |
| `openspec/todo.md` | 追加一条未决登记：处置五「`includes` 双语义解耦（config 增 source 范围键）+ `in_suite_scope` 绝对路径归一；**顺序：先 B（解耦）后 A（归一）**」 | D13；proposal R-5 / 遗留观察项 1 明写的实施期登记，纯文档条目 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|--------|----------|-----------|----------|------|
| `environment::argv_budget_text` | `crates/infra/checks/src/environment.rs` | 新增（`pub(crate)`） | `pub(crate) fn argv_budget_text(budget: usize, expanded_len: usize, omitted_len: Option<usize>, files: usize) -> String` | 拒绝降级时 `RunFault::Environment` 的摘要：定性「harness 限制、非代码问题」+ 展开长度 / 平台预算 / 文件数三数字（`omitted_len` 为 `Some` 时追加降级后长度）+ 「收窄 `includes` 或拆分 suite」动作；不含 `ENV_GUIDANCE` / 占用者 / 重试次数（D8；第四参为对 spec 三参签名的细化，见待决问题 1） |
| `environment::argv_degrade_note` | 同上 | 新增（`pub(crate)`） | `pub(crate) fn argv_degrade_note(budget: usize, expanded_len: usize, files: usize) -> String` | 安全降级时的记录注：「展开 N 字符超平台预算 M，已降级为框架自发现（K 个测试文件）」；**零动作建议**、零环境指引（D8） |
| `testexec::runner::execute_plan` | `crates/infra/checks/src/testexec/runner.rs` | 修改（签名不变，链路内插裁决步） | `pub(crate) async fn execute_plan(plan: &TestPlan, report_dir: &Path, cancel: CancelFlag) -> Result<PlanExecution, String>` | 展开后、`ensure_program_resolvable` 前插入预算裁决；拒绝臂返回 `Ok` 且 `fault = Some(Environment(..))`、`error == None`、零 spawn（D2 / D5） |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `TestPlan` | `crates/infra/checks/src/testexec/detect.rs` | 修改（12 → 13 字段） | 新增 `pub omit_files_safe: bool`——detect 期一次算定的降级安全布尔量；进程内结构，不序列化、不出线 |
| `PlanExecution` | `crates/infra/checks/src/testexec/runner.rs` | 修改（8 → 10 字段） | 新增 `pub argv_len: usize`（`WithFiles` 展开字符数）与 `pub argv_note: Option<String>`（降级说明，未降级 `None`）；进程内结构，不序列化、不出线 |
| `ExpandMode` | `crates/infra/checks/src/testexec/runner.rs` | 新增（模块私有 enum） | `WithFiles`（现行为逐字不变）/ `OmitFiles`（`{files}` 连前导空白剥离） |
| `ArgvDecision` | `crates/infra/checks/src/testexec/runner.rs` | 新增（模块私有 enum） | `Keep { command: String }` / `Omit { command: String, note: String }` / `Reject { message: String }`——`decide_argv` 的纯数据产物 |

<!-- 配置：本变更零 config 键新增 / 删除（预算为代码常量，MUST NOT 进 config schema；`includes` 语义澄清随处置五出局），故整段省略。 -->

---

## 数据模型

**进程内结构（不持久化、不出线）**

| 结构 | 字段（本变更相关） | 来源 → 去向 |
|------|-------------------|-------------|
| `TestPlan` | `id` / `framework` / `root` / `cwd` / `files: Vec<String>` / `config_args: Option<String>` / 双模板 / 覆盖格式与工件名 / **`omit_files_safe: bool`（新）** | `detect::detect_plans`（读 config + glob）→ `runner::execute_plan` / `mod::push_plan_head` |
| `PlanExecution` | `exit_code` / `duration_ms` / `cases` / `source_files` / `measured` / `error` / `output` / `fault` / **`argv_len: usize`（新）** / **`argv_note: Option<String>`（新）** | `runner::execute_plan` → `mod::push_plan_head`（两新字段）与 `mod::build_sub_report`（其余八字段） |
| `ArgvDecision` | 三变体（见上） | `decide_argv` → `execute_plan`（本函数内消费，边界不越出） |

**持久化面**

| 载体 | 形态变化 | 约束 |
|------|----------|------|
| `exec-outputs/<exec_id>.log`（`exec::record_exec_output` 单点） | 每段脚本输出前新增 plan 分隔头 `=== plan {id} ({framework}) · files={n} · argv={len} 字符 ===`；降级 plan 头后紧跟 `[argv] {note}` | 头与 `[argv]` 行为**记录元数据**；去掉后正文与改造前逐字节一致、按 plan 序保序；零 plan 执行的复用分支记录仍为空串（D10 / D11） |
| `reports/test/summary.json` + 逐 suite `report.json` | **零变化**（schema 不扩展：`argv_len` / `argv_note` 不落盘） | 降级说明 MUST NOT 进 `SubReport.findings` / `summary.findings`（D6）；拒绝臂不写 summary / 子报告 |
| `RunFault`（进程内载荷） | **零变化**（变体仍 `{Stopped, Environment(String)}`；`Environment` 摘要文本按其成因分型装配） | 不出 IPC / `RunUpdate` / `RunNodeRecord` / `bindings.ts` |

---

<!-- 路由 / API 设计：本变更新增零 HTTP 端点、零 IPC command、零 CLI 子命令（`PlanExecution` / `TestPlan` / `ArgvDecision` / `RunFault` 全为进程内结构，`bindings:check` 预期零 diff），故整段省略。 -->

---

## 依赖

| 类别 | 依赖 | 用途 |
|------|------|------|
| 运行时（新增） | **无** | 三面处置只用 `std`（`String` / `usize` / `#[cfg]`）+ 本 crate 既有模块（`environment` / `detect` / `glob` / `globmatch` / `foundation::prune`） |
| 运行时（既有，零版本变更） | `glob`（`detect` 二次枚举）、`orchestration::port::RunFault`、`exec`（`run_shell` / `record_exec_output`，语义零改） | 复用 |
| 构建 / 测试（既有） | `tempfile`（tempdir 装置）、`tokio`（`#[tokio::test]` 真实 spawn 用例）、`crate::lock_test_env`（进程环境锁窗口） | 测试相位用例沿既有先例 |

---

## 验收标准落点

| AC | 落点（设计 / 文件） |
|----|---------------------|
| AC-0 | 本变更零触碰 `openspec/config.json`（变更清单不含）；vp 档 87 文件 × 4527 字符 < 7000 → `decide_argv` 走 ②`Keep`；record 头 `argv=` 载实测量（D1 / D10） |
| AC-1 | `CMD_LINE_BUDGET = 7000`（`#[cfg(windows)]`）+ `line_budget()` 双平台臂 + 边界口径「`len <= 预算` 不降级 / `len > 预算` 进裁决」在 `decide_argv` 的 ①② 臂（D1 / D4 / D12） |
| AC-2 | 未超预算 → `Keep { command: expanded }` **原串回填、零二次展开**、`argv_note == None` → 无 `[argv]` 行（D3 / D4 / D10） |
| AC-3 | `Omit` 臂：`OmitFiles` 展开（`{files}` + 前导空白剥离、其余四占位符同义）→ `exit_code == 0`、`argv_note` 在场；`error` 恒 `None` → `conclusion` / `problems` 零变化；头补 `[argv]` 行（D3 / D4 / D6 / D10） |
| AC-4 | `Reject` 臂（`!omit_files_safe`）：零 spawn、`fault = Some(Environment(..))`、`error == None`、不写 summary / 子报告（复用既有 `fault_outcome`）；文案三要点由 `environment::argv_budget_text` 装配（D5 / D6 / D8） |
| AC-5 | `Reject` 第四臂（`omitted_len > budget`）复用同一出口，文案以 `omitted_len: Option<usize>` 载原始与降级后两次长度（D4 / D8） |
| AC-6 | walker / `port.rs` **零代码改动**（变更清单无此二文件）；`Environment` 经既有 `settle_fault` 收敛 `Terminal::failed`（D5 说明 + 只读红线清单） |
| AC-7 | `push_plan_head` 单点、每已执行 plan 恰一行头、plan 序稳定、空输出仍带头；降级说明只在 `argv_note` → `[argv]` 行，`build_sub_report` **零改**使其进不了 findings（D6 / D10 / D11） |
| AC-8 | 零改面清单：`REGISTRY` 五模板 / 其余四占位符 / 报告 schema / 复用门 / 超时·瞬态·取消三面 fault / `exec` 面 / `bindings`（见「修改文件」各条 `说明` 列的「零改」项） |
| AC-9 | 新增分支枚举（`Keep` ×2 条件 / `Omit` / `Reject` ×2 / `ExpandMode` 两态 / 头装配 ×2 形态 / 两臂文案）逐条落在 `decide_argv` / `expand_template` / `push_plan_head` / `environment` 的纯函数或纯装配面上，全部可由单元用例直驱；覆盖率为桌面端机械门禁产出的**执行结论**，不由本相位裁定 |
| AC-10 | `packages/desktop/package.json` `0.5.31` → `0.5.32`；`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 版本行不动（D13） |

**R 面缓解对照**：R-1 → D7 集合相等 + excludes 保守臂 + 「`omit_files_safe` 恒 `false`」单点应急闸；R-2 → D1 余量 1191 + D4 ④「降级后再判长度」；R-3 → D8 拒绝面给动作与三数字（有意取舍）；R-4 → D6 结构性不可达；R-5 → D13 todo 登记 + D7 不碰 `in_suite_scope`；R-6 → D1 unix `None` 恒 `Keep`（有界显式失败，不假绿）；R-7 → D10 / D11 元数据豁免口径 + 单点改写在同变更内；R-8 → D12 预算入参。

---

## 待决问题

1. **`argv_budget_text` 的第四参（`omitted_len: Option<usize>`）与 delta spec 三参签名的差异**：spec（`desktop-checks-domain` Module Contract）写作 `argv_budget_text(budget, expanded_len, files) -> String`，而 `desktop-test-execution`「降级无效仍拒绝」scenario 要求文案载**两次长度**。本设计取「扩一参 `omitted_len`」以保证文案单点不被 runner 拼串破坏；若规格方坚持三参，替代方案是 environment 内再拆第三臂 `argv_budget_after_omit_text(budget, expanded_len, omitted_len, files) -> String`（同为单点，仅多一个函数名）。**需 spec 侧确认其一**。
2. **拒绝臂的 `PlanExecution.exit_code` 取值**：设计取既有「进程无退出码」哨兵 `EXIT_CODE_NO_STATUS = -1`（与超时 / 取消同口径；fault 面下不进结论）。若评审要求「未拉起」与「无退出码」语义可辨，可改 `1`；当前无消费方依赖该值，取哨兵是为语义一致。
3. **`omit_files_safe` 的框架自发现等价假设**：判据以 `REGISTRY` 的 `default_glob` 代表「框架自身发现范围」，本仓 vp 档已实测一致（87 == 87），其余四框架为注册表声明级假设（proposal 遗留观察项 4）。新框架接入时应补一次实测；本篇不做跨框架验证。
4. **`detect` 二次 glob 枚举的成本**：每 suite 多一次同模式走盘（相对项目根）。当前两 suite 量级下可接受；若未来 suite 数或仓库规模显著增长，可改为「首次枚举时顺带记录 default_glob 集合」（需把 includes 与 default_glob 两次枚举合流的缓存面），本篇按简单优先不做。
5. **`openspec/todo.md` 登记是否计入本变更范围**：proposal R-5 与遗留观察项 1 两处明写「实施期登记」，但「变更范围 - 实现文件」未列该文件。本设计按 proposal 意图纳入一条纯文档条目；若归档口径要求变更范围严格闭合，可将其移出并在归档后单独补记。
