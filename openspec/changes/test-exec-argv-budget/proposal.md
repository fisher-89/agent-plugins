# 提案: test-exec-argv-budget

> **变更**: test-exec-argv-budget
> **日期**: 2026-10-26
> **状态**: draft
> **工作流**: bug-fix
> **来源**: `explore.md`（根因已实测确认；处置一已在归档变更 `2026-10-26-desktop-build-mode-split` 落地）
> **前置事实**: `openspec/config.json` 的 vite-plus suite `includes` 已收窄为 `src/**/*.test.{ts,tsx}`（命中 87 个测试文件，实测展开 4527 字符 < 8191）——本篇**不再改该文件**，只处理 harness 侧的结构性缺口

---

## 问题

### 现象：门禁判 `error`，但一条用例都没跑

`desktop-build-mode-split` 变更的 test-execution 门禁判 `error`，输出首行只有 `The command line is too long.`，而同一份报告里 rust suite 实际全绿（`total 2944 / passed 2944`）、`coverage-summary.json` 齐备。

| 证据 | 位置 | 说明 |
|---|---|---|
| 首行 `The command line is too long.` | `reports/test/exec-outputs/exec-1792996742834-28.log` | 随后的大段 `cargo-llvm-cov` 编译输出属 **rust plan** |
| `problems[0].framework = "vite-plus"`、`exit_code 1`、`duration_ms 76` | `reports/test/summary.json` | 76 ms 即退出 = `cmd /C` 进程级拒绝，非执行失败 |
| `conclusion = "error"` | 同上 | harness 限制被归到「被测代码非零退出」面 |

结论不是代码坏了：**harness 把展开后的命令拼超了 Windows `cmd.exe` 的 8191 字符硬上限**，`cmd /C` 直接在 shell 层拒绝，一个用例都没跑。

### 上限实测与长度核算

`cmd /C <script>` 实测（本机）：

| 脚本长度 N | 结果 |
|---|---|
| 8000 / 8150 | `exit 0` |
| 8200 / 8300 | `exit 1`，`The command line is too long.` |

按 `detect.rs` 模板 + `runner.rs` `expand_template` 的 `files.join(" ")` 同口径重建：

| 配置 | 文件数 | 命令长度 | 余量（8191） |
|---|---|---|---|
| 旧 `src/**/*.{ts,tsx}`（全部源文件） | 180 | ≈ 8230 | **−39** |
| 现 `src/**/*.test.{ts,tsx}`（本变更前置事实） | 87 | **4527**（本 worktree 实测） | +3664 |

旧配置的 baseline 是 178 个文件 ≈ 8145，余量仅 46 字符；新增一个 `.test.ts`（77 字符）即越线。**这是长期贴红线运行的隐性脆弱点，不是某次变更引入的缺陷**——收窄 `includes` 只是止血，harness 没有任何长度防线。

### 根因分层

| 层 | 缺口 | 位置 |
|---|---|---|
| **L1 配置面** | `includes` 被配成「全部源文件」（180 个），而 `detect_files` 里 `includes` 是**测试文件发现 glob**；93 个纯源文件对 `vp` 的位置参数是无效过滤器，白耗 3.9k 字符 | `openspec/config.json`（**已收窄**） |
| **L2 执行面** | `plan.files` 被无条件全量塞进 `{files}`，**没有任何平台长度预算**；超限的唯一结局是 shell 层拒绝 | `testexec/runner.rs:84` / `expand_template:195` |
| **L3 归因面** | 非零退出走 `non_zero_exit_error` → `PlanExecution.error` → 子报告 `findings` → `problems: execution_error` → `conclusion = error`，harness 限制被当作被测代码问题，会诱导修复会话去改代码 | `testexec/runner.rs:188` / `core/checks/src/aggregate.rs:450` |
| **L4 诊断面** | `exec-outputs` 单文件**无 plan 分隔头**、不记展开长度 / 是否降级 / 文件数，人工回查必须反推模板（本次误判的直接成因：rust 输出被读成超长命令错误的上下文） | `testexec/mod.rs:170` |

---

## 提案

只改桌面端 `packages/desktop/src-tauri/crates/infra/checks`、三份 spec 与版本号，**不碰配置、不碰 core 纯层、不碰前端 / bindings / CLI**。三面：

1. **预算与安全降级（L2）**——展开后判定平台长度预算，超限时按「能否安全降级」二分：能则省略 `{files}` 交框架自发现，不能则显式停给用户。
2. **归因修正（L3）**——命令行超限属执行面**环境**错误：`RunFault::Environment`，MUST NOT 落 `execution_error`、MUST NOT 写半套报告、MUST NOT 升格决策。
3. **诊断可读性（L4）**——输出记录按 plan 加分隔头，头内载展开长度 / 文件数 / 是否降级。

### 处置一（已完成，不在本篇）

`includes` 收窄为 `src/**/*.test.{ts,tsx}` 已随归档变更 `2026-10-26-desktop-build-mode-split` 落地，实测本仓 vp 档展开 4527 字符、87 个测试文件与框架自发现（`vp test list` 不传过滤器）逐一致。本篇把它作为**前置事实与回归锚**，不再重复改动。

### 处置二：预算裁决与安全降级（本篇主体）

**位点**：`testexec/runner.rs` —— `expand_template` 之后、`ensure_program_resolvable` 之前（`execute_plan` 内）。

**预算常量**：仅 Windows 臂 `const CMD_LINE_BUDGET: usize = 7000`，留 1191 字符给 `cmd /C ` 前缀、`raw_arg` 转引后的实际线长放大与不同 Windows 版本的细微差异。unix 臂**不设降级**（`ARG_MAX` 2 MB 量级，引入分叉语义没有收益）。预算与既有 `SUITE_COMMAND_TIMEOUT` / `TESTEXEC_TOTAL_BUDGET` 同性质：**代码常量，MUST NOT 进 config schema**。

**裁决三臂**（`len = WithFiles 展开后命令串字符数`）：

```
len <= 预算                          → 原样执行（精确子集，命令与改造前逐字符一致）
len >  预算 ∧ 降级安全               → OmitFiles 重新展开 + info 留痕
len >  预算 ∧ 降级不安全             → RunFault::Environment（可执行指引文案）
（OmitFiles 展开后仍 > 预算 → 一律拒绝：降级无效，报环境错误）
```

**降级安全判据（关键设计）**：判据是**集合相等**，不是模式文本比较——`{files}` 省略后框架走自身发现，只有「自发现集合 == `plan.files`」才保证跑的还是同一批：

```
omit_files_safe := glob(root, 框架 default_glob) 集合 == plan.files 集合
```

- `includes` 未设 / 与 `default_glob` 等价（本仓现状：87 == 87）→ 相等 → 安全降级。
- `includes` 是**真子集**（例如只跑 `src/hooks/**`）→ 不等 → 拒绝降级，显式报环境错误并给动作。宁可显式报错，也不静默把范围放大成「全 suite + 无关失败」。

该布尔量在 **detect 期一次算定**（`TestPlan.omit_files_safe`）：glob 方言与剪枝的枚举单点驻 `detect.rs`（`globmatch` 唯一方言实现），runner 只消费布尔量，MUST NOT 二次自建 glob 枚举。对照面**不施加 suite `excludes`**（框架自发现不认这套 excludes）——`excludes` 一旦真的剔掉了某个文件，两侧即不等，降级被拒。

**OmitFiles 展开语义**：`{files}` 占位符**连其前导空白一并剥离**（与 `{config_args}` 未注入时的既有语义对齐）；其余四个占位符逐字不变；模板无 `{files}` 时（rust 档）两态展开逐字相同。

### 处置三：归因修正

`RunFault::Environment(String)` 面已存在（`core/orchestration/src/port.rs:170`），testexec 也已为超时 / 瞬态耗尽 / 取消建立「fault 先于结论」的收口（`mod.rs:187`）：判 `Environment` 后走 `fault_outcome`——**不写 summary、不落子报告、不进复用门**，结论占位 `Fail`、`exec_id` 照旧回填，walker 侧经 `settle_fault` 收敛 `Terminal::failed`（零 agent、零决策分叉）。

命令行超限与它们同层，只是**成因文案不同**：文案要点 = ①这是 harness 限制不是代码问题；②数字（展开长度 / 平台预算 / 文件数）；③可执行动作（收窄 `includes` 或拆分 suite）。文案装配落在既有单点 `crates/infra/checks/src/environment.rs`（「检查域环境归因装配单点，MUST NOT 两写」），**MUST NOT 复用**那里的安全软件排除指引（`ENV_GUIDANCE`）——成因不同，误引会让用户去改安全软件。

### 处置四：诊断可读性

1. **plan 分隔头**（`mod.rs` 聚合面）：每个**已执行** plan 的输出前写入
   `\n=== plan {id} ({framework}) · files={n} · argv={len} 字符 ===\n`；
   降级时紧随一行 `[argv] {降级说明}`。本次误判的直接成因（rust 输出被读成超长命令错误的上下文）就此消除。
2. **留痕三条**：展开长度 / 是否降级 / 文件数全部进上述头（完整命令串因体积不落盘——模板 + 同口径枚举可复算）。
3. **降级说明 MUST NOT 进 `SubReport.findings` / `summary.findings`**（explore 原方案在此有硬缺陷）：`core/checks/src/aggregate.rs::collect_problems` 的规则是「`failed == 0` 且 findings 非空 → `execution_error`」，把纯信息注进 findings 会把一次**绿跑降级**翻成 `conclusion = error` 的假失败，正是本篇要消灭的归因错位。信息面单点 = 输出记录 plan 头（持久化、经 `exec_id` 从节点回查可达）。

### 处置五（出局，另立变更）

`includes` 双语义解耦（config 新增 source 范围键）与 `in_suite_scope` 绝对路径不归一（`aggregate.rs:394` 拿绝对路径去 match root 相对模式 → per-suite override 恒空）**不在本篇**：它要动 config schema 演进 + 纯层判定，收益面（per-suite override 复活）与 argv 预算无因果。**顺序约束（必须写死）**：先解耦（B）再修绝对路径（A）——收窄后的 `includes` 已是测试文件 glob，先修 A 会让 override 圈到的变成测试文件自身，比今天更错。

### 已否决的替代路径

| 手段 | 否决理由 |
|---|---|
| 传 glob 而非展开清单 | `vp`/`vitest` 位置参数是路径子串过滤器不认 glob（实测 `vp test list "src/hooks/**"` 空结果）；jest `testPathPattern` 是 regex；node-test 不展开——跨框架无法统一 |
| `--dir` 收窄扫描根 | 只有 vitest 系支持，且单目录无法表达 include 集合 |
| 写临时 `.cmd` / 响应文件 | 批处理单行同样受 8191 限制；vitest 无 `@file` 语法 |
| argv 直传绕开 `cmd /C` | `vp` 是 `.cmd` 仍过 cmd；换 `.exe` 也只是把上限抬到 `CreateProcess` 32767，治标 |
| 分块重跑（argv 分片） | 多段执行需合并 istanbul `json-summary`（同文件键覆盖、总数重算）与 `results`，退出码还要归并；收益仅是「保留精确子集」这一增量语义，成本与风险远高于「安全降级 + 显式拒绝」 |

---

## 能力

### 新增能力

无。argv 预算是 test-execution 门禁步的**执行链内一环**（跑测试这件事本身），另立能力会把「怎么跑」与「跑到哪一步」长期绑在一起改，故并入既有 **desktop-test-execution**。

### 修改的能力

- **desktop-test-execution** —— **MODIFIED**「执行链与报告落盘」（链内插入展开预算裁决步 + 一条 scenario）；**MODIFIED**「输出记录为脚本真实输出」（记录内容 = plan 头 + 脚本输出，分隔头作为元数据显式豁免）；**ADDED**「命令展开长度预算与安全降级」（预算常量与边界 / 三臂裁决 / 集合相等的降级安全判据 / OmitFiles 展开语义 / 环境归因 / 留痕 / 零扩展约束）。
- **desktop-checks-domain** —— **ADDED**「命令行超限归因文案单点装配」（`environment::argv_budget_text` / `argv_degrade_note` 两臂落既有单点模块；文案三要点；MUST NOT 复用安全软件排除指引；既有 `fault_text` 逐字不变）。
- **desktop-change-orchestration** —— **MODIFIED**「执行面归因 RunFault 与门禁步分派」（终局文案由「MUST 含占用者 / 重试次数 / 排除安全软件指引」改为**按成因分型**：锁 / 超时型沿用原四项，命令行超限型携带长度 / 预算 / 动作——否则新 fault 文本会与既有 MUST 直接冲突）。

其余能力零改动，理由：

- **desktop-workspace-config / config schema**：预算为代码常量，零 schema 扩展；`includes` 语义澄清随处置五另议。
- **desktop-exec-runtime**：`run_shell` / `SystemCommand` / 重试策略零改动——本篇不新增 spawn 能力，只在 spawn 之前拦一道裁决。
- **desktop-checks-domain 纯层**：`core/checks`（解析 / 聚合 / 复用门 / 阈值 / override）零改动——降级说明刻意不进 findings 面即为此（见处置四第 3 点）。
- **前端 / bindings / IPC**：`PlanExecution` / `RunFault` 都在进程内，零出线，`bindings:check` 应零 diff。
- **desktop-mutation-testing**：突变输入是 change 文件清单 + diff 面，与 argv 长度无关。

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/infra/checks/src/testexec/runner.rs` | 常量 `CMD_LINE_BUDGET = 7000`（`cfg(windows)` 臂）+ `line_budget() -> Option<usize>`；`expand_template` 增 `expand_mode: ExpandMode::{WithFiles, OmitFiles}`（`OmitFiles` 剥 `{files}` 前导空白）；纯函数 `decide_argv(...) -> ArgvDecision::{Keep{command}, Omit{command, note}, Reject{message}}`（预算作入参 → 表驱动可测、平台无关）；`execute_plan` 在展开后、`ensure_program_resolvable` 前裁决；拒绝 → `fault = Environment(argv_budget_text(...))` 早返；`PlanExecution` 增 `argv_len: usize` / `argv_note: Option<String>` |
| `crates/infra/checks/src/testexec/detect.rs` | `TestPlan` 增 `omit_files_safe: bool`；`detect_files` 以**同一剪枝管线（不施加 suite excludes）**再枚举一次 `default_glob` 集合并与 `files` 比集合相等（glob 方言 / 剪枝 / brace 展开零新逻辑，复用既有函数） |
| `crates/infra/checks/src/testexec/mod.rs` | 聚合面写 plan 头（`=== plan {id} ({framework}) · files={n} · argv={len} 字符 ===`；降级补 `[argv] {note}` 行）；重排为「先写头再拼 `output`」；复用门分支零改动（零 plan 执行 → 记录仍为空） |
| `crates/infra/checks/src/environment.rs` | 新增 `argv_budget_text(budget: usize, expanded_len: usize, files: usize) -> String`（拒绝面 fault 文案：定性「harness 限制非代码问题」+ 三组数字 + 收窄 `includes` / 拆分 suite 动作）与 `argv_degrade_note(budget: usize, expanded_len: usize, files: usize) -> String`（降级面 info 文案，记录 plan 头用）——同模块两臂，runner / mod 侧 MUST NOT 就地拼串；既有 `attempts_log` / `fault_text` / `progress_line` 逐字零改 |
| `packages/desktop/package.json` | `version` 0.5.31 → 0.5.32（沿「桌面源码每次变更 +1 patch」纪律；`src-tauri/tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 不跟） |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| `expand_template` 五占位符 × `WithFiles` / `OmitFiles` 两态（`{files}` 前导空白剥离、`{config_args}` 既有臂不回归、无 `{files}` 模板两态同串） | `crates/infra/checks/src/testexec/runner_test.rs` | 新增 + 既有 10 处调用点补 `WithFiles` |
| `decide_argv` 表驱动：`len == 预算` 不降级 / `预算 + 1` 降级；`omit_files_safe` 三臂（安全 / 真子集 / 无 includes）；`OmitFiles` 后仍超预算 → 拒绝；预算 `None`（unix 语义）恒 `Keep` | 同上 | 新增（显式传预算入参 → 全平台可跑） |
| `line_budget()` 平台臂：Windows `Some(7000)`、非 Windows `None`（`cfg` 断言）+ 常量锚定 | 同上 | 新增 |
| `execute_plan` 集成：超预算 + 不安全 → `fault = Some(Environment(…))` 且 `error == None`、未 spawn（counter shim / 不存在的程序也可断言「未拉起」）；安全降级 → `exit_code == 0`、`argv_note` 在场、命令不含文件清单 | 同上 | 新增（沿既有 tempdir + shim 先例） |
| plan 头装配：多 plan 头按 plan 序、含 `argv=` 与 `files=`、降级 plan 带 `[argv]` 行；空 `output` 多 plan → 记录仅头部（既有「空流 → 空 `.log`」断言随裁决改写） | `crates/infra/checks/src/testexec/mod_test.rs` | 改写 1 例 + 新增 2 例 |
| `omit_files_safe` 判定：`includes` 未设 / 与 `default_glob` 等价 → `true`；`includes` 真子集 → `false`；excludes 实际剔除了文件 → `false`（保守） | `crates/infra/checks/src/testexec/detect_test.rs` | 新增 |
| `argv_budget_text` / `argv_degrade_note`：均含长度 / 预算 / 文件数（前者带动作建议，后者带「已降级为框架自发现」）；**均不含** `ENV_GUIDANCE` 的「排除 workspace 父目录」句；`fault_text` 输出逐字不变（回归锚） | `crates/infra/checks/src/environment_test.rs` | 新增 |

### 不要修改

- `openspec/config.json`（`includes` 已收窄，本变更零改动——改它会与解耦变更冲突）。
- `crates/core/checks/**`（解析 / 聚合 / 复用门 / 诊断树 / `collect_problems` / `compute_overrides` / `in_suite_scope`）；`crates/core/orchestration/**` 生产源码（`RunFault` 与 `settle_fault` 零扩面）。
- `crates/infra/exec/**`（`run_shell` / `SystemCommand` / `RetryPolicy` / `record_exec_output` 语义零变）、`crates/infra/checks/src/testexec/report.rs`（报告 schema 零扩展）。
- 前端 `packages/desktop/src/**` 与 `src/types/generated/bindings.ts`（零载荷出线）；`openspec/specs/` 下未被本变更 delta 覆盖的能力。
- `openspec/changes/test-exec-argv-budget/explore.md`（只读）。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-0 | 前置事实在位：`openspec/config.json` vp suite `includes = ["src/**/*.test.{ts,tsx}"]`，detect 命中 87 个测试文件、展开 4527 字符 < 7000，本变更为该文件零 diff | `git diff openspec/config.json` 空 + 实跑 record 头 `argv=4527`（±路径长度差异） |
| AC-1 | 预算常量锚定：Windows `CMD_LINE_BUDGET == 7000`（单元测试常量断言）；非 Windows `line_budget() == None`；边界 `len == 预算` **不**降级、`预算 + 1` 降级 | `runner_test` 表驱动 + `cfg` 断言 |
| AC-2 | 未超预算零行为变化：命令串与改造前**逐字符一致**、无 `[argv]` 行、`argv_note == None`；本仓 vp 档与 rust 档真件跑结论与改造前一致 | `runner_test` 逐字断言 + 真件 test-execution 门禁 |
| AC-3 | 安全降级：超预算 ∧ `omit_files_safe` → 不传文件清单执行成功（`exit_code == 0`），`{files}` 及其前导空白剥离、其余四占位符逐字不变；`conclusion` / `problems` **零变化**（绿跑仍绿）；record 头补 `[argv]` 行（长度 / 预算 / 文件数 / 已降级） | `decide_argv` + `execute_plan` 集成用例 + `mod_test` 头断言 |
| AC-4 | 拒绝降级：超预算 ∧ `omit_files_safe == false` → `fault = Some(Environment(…))`、`error == None`、**未 spawn**；**不写 summary / 子报告**（半套报告不进复用门）、`problems` 无 `execution_error`；文案含展开长度 / 平台预算 / 文件数 / 「收窄 includes 或拆分 suite」动作 | `execute_plan` 拒绝用例 + `mod_test` 的 `fault_outcome` 路径断言 |
| AC-5 | 降级无效也拒绝：`OmitFiles` 展开后仍超预算 → 同 AC-4 路径，文案载两次长度（原始 / 降级后） | 表驱动用例 |
| AC-6 | 归因不改相位语义：`fault = Environment` 经既有 `settle_fault` 收敛 `Terminal::failed`（节点 `failed`、run `failed`），**零 WorkerAgent、零决策分叉、零 `phase_log` fail 代写**（沿既有「执行面归因 RunFault 与门禁步分派」，walker 代码零改动） | `walker_test` 既有 fault 用例 + 本变更不新增 walker 代码（diff 审查） |
| AC-7 | 留痕可回查：每个已执行 plan 的输出前有且仅有一行 `=== plan {id} ({framework}) · files=N · argv=M 字符 ===`（plan 序稳定）；`exec_id` 从节点 `output_ref` 回查到的记录以该头起手、去掉头后与改造前脚本输出逐字节一致；纯降级不污染结论（说明 MUST NOT 出现在 `SubReport.findings` / `summary.findings`） | `mod_test` 三例（多 plan / 降级 / 空输出）+ grep 确认 `build_sub_report` 未注入 findings |
| AC-8 | 既有语义零变：五框架注册表模板、`{config_args}` / `{report_dir}` / `{results_file}` / `{coverage_file}` 展开、复用门（mtime / plan id 集）、报告 schema、超时 / 瞬态 / 取消三面 fault 全部逐字不变；`bindings:check` 零 diff | 既有 `runner_test` / `detect_test` / `mod_test` 全绿 + `pnpm -C packages/desktop run bindings:check` |
| AC-9 | 覆盖率与门禁：新增分支（三臂裁决 / 两态展开 / 头装配 / 文案装配）均被单测覆盖，复跑 test-execution 门禁 lines ≥ 90 / branches ≥ 85 / functions ≥ 90 | desktop 门禁 `pnpm -C packages/desktop check` + test-execution 门禁 |
| AC-10 | 版本纪律：`packages/desktop/package.json` 0.5.31 → 0.5.32；`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 版本行不动 | 版本 diff 审查 |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **降级跑出超集（假绿）**：集合相等只按「注册表 `default_glob`」判定，框架自身配置（`vite.config.ts` 的 include）若与之不同源，省略 `{files}` 后实际跑的范围可能大于 `plan.files` | 绿跑覆盖了本不该跑的文件，门禁语义漂移 | 低 | 判据是**集合相等**且含 excludes 保守臂（excludes 实际剔除过文件即拒降级）；降级事实在 record 头可见（可事后核对 `files=N` 与实际用例数）；应急闸是把 `omit_files_safe` 恒置 `false`（恒走显式报错形态，单点布尔量一改即生效，无需改裁决逻辑） |
| R-2 | **预算取值偏差**：8191 是 `cmd.exe` 行上限，`raw_arg` 前缀与转引放大后实际线长 > 脚本长度；7000 的 1191 余量在某些 Windows 版本 / 路径深度下不足 | 降级后仍可能被 cmd 拒绝（表现为普通非零退出，归因回到「真红」面） | 低 | 余量按实测（8000 / 8150 通过、8200 拒绝）取 1191；降级后再判一次长度，仍超即拒绝；两级都失效时输出仍是 `The command line is too long.`，但 record 头已载长度，回查不再需要反推 |
| R-3 | **误拒合法配置**：`includes` 为精心收窄的子集（如只跑 `src/hooks/**`）而文件多到超预算时，用户拿到的是失败而非「跑窄一点」 | 用户困惑「明明配置更省」 | 中 | 文案给出可执行动作（收窄 / 拆分 suite）与三组数字；这是**有意取舍**——静默放大范围会产生「无关失败 + 假绿」双重污染，代价高于一次显式失败 |
| R-4 | **信息面误落 findings 导致假失败**（explore 原方案的硬缺陷）：把降级说明写进 `SubReport.findings` 会被 `collect_problems` 判成 `execution_error`（`failed == 0 ∧ findings 非空`） | 绿跑翻红，且归因错位复发 | 中（若实现期「顺手」改回 explore 写法） | spec 与 Module Contract 双处写死「MUST NOT 进 findings」；AC-7 以 grep + 结论零变化钉住 |
| R-5 | **与处置五耦合埋雷**：有人先修 `in_suite_scope` 的绝对路径归一，收窄后的 `includes`（已是测试文件 glob）会把 per-suite override 圈到测试文件自身 | 覆盖判定错得更彻底 | 中 | 处置五明确出局，并在本文件与 delta spec 的「顺序约束」写死「先解耦 B 再修 A」；实施期登记 `openspec/todo.md` |
| R-6 | **unix 无预算**：Linux CI 上极端文件数仍可能撞 `ARG_MAX` / `E2BIG` | 报错形态为 `Err`（`测试命令拉起失败` 类），无降级 | 低 | 有意不分叉（explore 裁决）：`ARG_MAX` 2 MB 量级，为它引入第二套降级语义不划算；命中时仍是有界显式失败，不是假绿 |
| R-7 | **记录形态变化引发断言与习惯回归**：record 不再是纯脚本输出（多了 plan 头），既有「空输出 → 空 `.log`」断言与人工阅读习惯受影响 | 旧测试红、回查者困惑 | 高（本篇既定） | 分隔头作为**元数据显式豁免**写进 spec（与原「脚本真实输出」不冲突）；受影响断言在同一变更内改写；头格式单调一行、以 `===` / `[argv]` 起手易辨 |
| R-8 | **平台条件编译降低覆盖**：`cfg(windows)` 常量与 `line_budget()` 的 unix 臂在本机不被覆盖 | 覆盖率抖动 / 隐性分叉 | 低 | 裁决逻辑做成平台无关纯函数（预算作入参）→ 主体分支全平台可测；`cfg` 只覆盖一行取常量的适配层，双臂各一条断言 |

## 遗留观察项（不在本篇范围）

1. **处置五：`includes` 双语义解耦 + per-suite override 复活**——config 增可选 source 范围键（`source_globs` / `coverage_scope`，缺省回落 `includes` 保兼容）+ `in_suite_scope` 绝对路径归一；**顺序固定：先 B（解耦）后 A（归一）**。建议独立变更，实施期登记 `openspec/todo.md`。
2. **argv 分片重跑**：本篇已否决（报告合并成本 > 收益），若未来某 suite 的测试文件数真的超过单个 shell 行能承载的量级，再以独立变更论证。
3. **嵌套引号调制度的复核**：`cmd /C "…"` 在字符串自己含引号时的调制度规则（Windows `cmd` 的历史包袱）不在本篇验证范围，仅以 1191 字符余量兜住。
4. **`{files}` 省略后的框架自发现口径自证**：本仓已实测 vp 自发现 == detect 结果（87 一致）；其余四框架的「`default_glob` 等价于框架自发现」为注册表声明级假设，未逐框架实测——新框架接入时应补一次实测。
