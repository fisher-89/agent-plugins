# 测试设计: test-exec-argv-budget

> **变更**: test-exec-argv-budget
> **依据**: `proposal.md`（L1–L4 根因分层、处置二 / 三 / 四、AC-0…AC-10）、`design.md`（D1–D13、变更清单、公共函数 / API、类型定义）
> **范围**: design「变更清单 - 修改文件」的 6 个文件中的 4 个源码文件（`testexec/runner.rs` / `testexec/detect.rs` / `testexec/mod.rs` / `environment.rs`）；另两文件为版本行与文档登记，无测试路径（见「不可测试项」）。零新增文件、零新增依赖。
> **路径口径**: 本设计全部路径为**项目根相对**；design.md 中 `crates/...` 的简写对应 `packages/desktop/src-tauri/crates/...`。

## 测试框架与既有布局

- **框架 / 运行器**：Rust 内置 `cargo test`（crate `checks-runtime`，位于 `packages/desktop/src-tauri/crates/infra/checks`）。无第三方断言库，断言为 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`。
- **侧车测试文件**：被测源文件尾以 `#[cfg(test)] #[path = "<name>_test.rs"] mod <name>_test;` 挂载，测试文件与源文件**同目录同名 + `_test` 后缀**（已确认 4 组映射全部存在）。
- **可见性**：测试模块是源文件的子模块，`super::` 即可触达模块私有项（`decide_argv` / `ExpandMode` / `ArgvDecision` / `line_budget` / `push_plan_head` / `collect_files`），故这些新项**不为其可测性加 `pub`**。
- **既有装置**（沿用以零新范式）：`tempfile::TempDir` 真实临时目录；`#[tokio::test]` 驱动真实 spawn；`crate::lock_test_env()` 进程环境锁窗口（`env_window` / `PathWindow` / `EnvWindow`）；平台 shim 可执行替身（`install_cargo_shim` / `set_shim_env` / `ChainWs`）；测试函数名用中文短语 + 断言消息写判据。
- **待补齐的 fixture**：`TestPlan` 12 → 13 字段、`PlanExecution` 8 → 10 字段，故 `runner_test` 的 `node_test_plan` / `rust_plan` / `jest_plan` 等构造器需补 `omit_files_safe`，`mod_test` 的 `PlanExecution` 字面量需补 `argv_len` / `argv_note`——属装置适配，不新增逻辑。

### 待测功能条目来源约定

`#### 待测功能` 只登记 design.md 明写的命名单元，并在条目末尾标注出处：`（公共函数 / API）` = design「公共函数 / API」子表行（含完整签名）；`（变更清单 ①②… / D<n>）` = design「修改文件」单元格或「关键设计决策」中显式命名者；`（类型定义）` = design「类型定义」子表行。**未在 design 中命名的内部实现细节不入表**（不 grep 导出声明、不虚构条目）。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-0 | 前置事实在位：vp suite `includes = ["src/**/*.test.{ts,tsx}"]`、detect 命中 87 个文件、展开 4527 字符 < 7000 → 裁决落在 `Keep` 臂；该配置文件为本变更零 diff 的非单测面（见「不可测试项」），`Keep` 行为由 `runner_test` 覆盖要点断言 | `openspec/config.json`（零 diff 前置事实）、`.../testexec/runner.rs` |
| AC-1 | 预算常量锚定：`CMD_LINE_BUDGET == 7000`（Windows 臂）、`line_budget()` 双平台臂（`Some(7000)` / `None`）、边界口径 `len == 预算` 不降级而 `预算 + 1` 进裁决；覆盖落在 `runner_test` 的 `decide_argv` 边界表驱动与 `line_budget` / 常量 `cfg` 断言 | `.../testexec/runner.rs` |
| AC-2 | 未超预算零行为变化：命令串原串回填、与改造前逐字符一致、`argv_note == None` 且记录无 `[argv]` 行；覆盖落在 `decide_argv` 正向用例、`expand_template` 的 `WithFiles` 与改造前逐字比对、以及 `execute_plan` / `mod_test` 的 `argv_note` 三态断言 | `.../testexec/runner.rs` |
| AC-3 | 安全降级：`OmitFiles` 展开把 `{files}` 连前导空白剥离且其余四占位符逐字不变，执行成功（`exit_code == 0`）、`argv_note` 在场、头补 `[argv]` 行，`conclusion` / `problems` 零变化；覆盖落在 `expand_template` 两态用例、`decide_argv` 的 `Omit` 臂、`execute_plan` 降级集成用例、`omit_files_safe` 判据用例与 `push_plan_head` 的 `[argv]` 行断言 | `.../testexec/runner.rs`、`.../testexec/detect.rs`、`.../testexec/mod.rs` |
| AC-4 | 拒绝降级：超预算且 `omit_files_safe == false` → `fault = Some(Environment(…))`、`error == None`、零 spawn（不可达程序仍返回 `Ok` + marker 不存在双证据）、不写 summary / 子报告、文案含定性 + 三数字 + 「收窄 `includes` 或拆分 suite」动作且不含 `ENV_GUIDANCE`；覆盖落在 `decide_argv` 与 `execute_plan` 拒绝用例、`argv_budget_text` 用例、`mod_test` 的 `fault_outcome` 路径断言 | `.../testexec/runner.rs`、`.../environment.rs`、`.../testexec/mod.rs` |
| AC-5 | 降级无效也拒绝：`OmitFiles` 展开后仍超预算 → 与拒绝臂同出口，文案载原始长度与降级后长度两个数字；覆盖落在 `decide_argv` 第四臂表驱动用例与 `argv_budget_text` 的 `omitted_len == Some(n)` 用例 | `.../testexec/runner.rs`、`.../environment.rs` |
| AC-6 | 归因不改相位语义：`Environment` 经既有 `settle_fault` 收敛 `Terminal::failed`，零 WorkerAgent、零决策分叉、零 `phase_log` fail 代写；本变更对 walker / port 零代码改动，由既有 `walker_test` fault 用例回归，不新增用例（见「不可测试项」） | `crates/core/orchestration/**`（walker / port，零代码改动） |
| AC-7 | 留痕可回查：每个已执行 plan 输出前有且仅有一行头（plan 序稳定）、格式字面口径（` · ` 分隔 / 十进制数值）、降级 plan 带 `[argv]` 行、空输出仍带头、`exec_id` 回查记录以头起手且去掉头后与改造前脚本输出逐字节一致、降级说明不进 `findings` 且 `conclusion` 仍 `Pass`；覆盖落在 `push_plan_head` 单点用例、`execute` 全链改写用例与 `build_sub_report` / findings 断言 | `.../testexec/mod.rs` |
| AC-8 | 既有语义零变：五框架注册表模板、其余四占位符展开、复用门（含零 plan 执行分支）、报告 schema、超时 / 瞬态 / 取消三面 fault、`fault_text` / `attempts_log` 逐字不变；覆盖落在四章节的回归用例（既有断言全绿，仅补新字段与 `ExpandMode` 入参）与 `bindings:check` 机械门禁（由桌面端执行） | `.../testexec/runner.rs`、`.../testexec/detect.rs`、`.../testexec/mod.rs`、`.../environment.rs` |
| AC-9 | 覆盖率与门禁：新增分支（`Keep` 两条件 / `Omit` / `Reject` 两出口 / `ExpandMode` 两态 / 头装配两形态 / 文案两臂）逐条被单测覆盖——本设计即该分支枚举到用例的映射，覆盖率数值由桌面端门禁产出 | `.../testexec/runner.rs`、`.../testexec/detect.rs`、`.../testexec/mod.rs`、`.../environment.rs` |
| AC-10 | 版本纪律：`0.5.31` → `0.5.32`，`tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` 版本行不动；无单测路径，由版本 diff 审查核验（见「不可测试项」） | `packages/desktop/package.json` |

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs

#### 待测功能

- `execute_plan(plan, report_dir, cancel)`: 展开后、`ensure_program_resolvable` 前的预算裁决接入——`Keep` 零行为变化 / `Omit` 降级执行 / `Reject` 环境归因早返（公共函数 / API；修改，签名不变）
- `expand_template(template, report_dir, results_file, coverage_file, config_args, files, expand_mode)`: 末位 `ExpandMode` 两态展开；`OmitFiles` 把 `{files}` 连前导空白一并剥离（变更清单 ② / D3；模块私有）
- `decide_argv(plan, report_dir, results_file, coverage_file, expanded, budget)`: 五臂裁决纯函数（预算作入参 → 平台无关）（变更清单 ④ / D4；模块私有）
- `line_budget()`: Windows 臂 `Some(CMD_LINE_BUDGET)`、非 Windows 臂 `None`（变更清单 ③ / D1；模块私有）
- `CMD_LINE_BUDGET`: `#[cfg(windows)]` 常量锚定 7000（变更清单 ③ / D1；代码常量，不进 config schema）
- `ExpandMode.WithFiles` / `ExpandMode.OmitFiles`: 两态语义与「模板无 `{files}` 时同串」（类型定义）
- `ArgvDecision.Keep` / `ArgvDecision.Omit` / `ArgvDecision.Reject`: 裁决纯数据产物三变体（类型定义）
- `PlanExecution.argv_len` / `PlanExecution.argv_note`: 新字段取值语义 + 既有两处构造点补齐（类型定义；D6 / D10）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `decide_argv` | 正向 | `budget = Some(7000)`、展开 4527 字符（87 条文件清单）→ `Keep`，`command` 与入参 `expanded` **逐字符相等**（原串回填、零二次展开） | 单例 |
| `decide_argv` | 边界 | `len == budget`（构造恰 7000 字符）→ `Keep`（不降级）；`len == budget - 1` → `Keep`；`len == budget + 1` 且 `omit_files_safe` → 进降级 / 拒绝裁决（AC-1 边界口径） | 表驱动 |
| `decide_argv` | 正向 | 超预算 + `omit_files_safe == true` → `Omit`：`command` 为 `OmitFiles` 展开串（不含文件清单、无 `{files}` 残留），`note` 在场 | 表驱动 |
| `decide_argv` | 边界 | 超预算 + `omit_files_safe == true` + `OmitFiles` 展开后恰 `== budget` → `Omit`（降级有效性边界，不得误判为拒绝） | 表驱动 |
| `decide_argv` | 异常 | 超预算 + `omit_files_safe == false` → `Reject`：`message` 含 `expanded_len` / `budget` / `files` 三数字与「收窄」「includes」「拆分」动作，且不含「火绒」「云枢」「排除 workspace 父目录」「已自动重试」「被锁文件」 | 单例 |
| `decide_argv` | 异常 | 超预算 + `omit_files_safe == true` + `OmitFiles` 展开后仍 `> budget` → `Reject`，`message` 同时含原始长度与降级后长度两个数字（AC-5） | 表驱动 |
| `decide_argv` | 边界 | `budget == None`（unix 语义）→ 恒 `Keep`：覆盖展开 0 字符 / 100000 字符 / `files` 空 / `files` 500 条 | 表驱动（4 形态） |
| `decide_argv` | 边界 | `budget == Some(0)`：展开空串（模板 `""`）→ `len 0 <= 0` → `Keep`；展开 1 字符 → 进裁决（safe → `Omit`，unsafe → `Reject`） | 表驱动 |
| `decide_argv` | 边界 | `budget == usize::MAX` → 任意展开恒 `Keep`（无溢出 / 无 panic） | 单例 |
| `decide_argv` | 边界 | 长度口径 `chars().count()`：展开串含 emoji / 中文路径（**字符数 7000 而字节数 > 7000**）→ `Keep`；字符数 7001 → 进裁决（`str` 特殊字符 + 超长面） | 表驱动 |
| `decide_argv` | 边界 | `plan.files == []`（含 `[]` 空清单）且长度来自模板静态填充：safe → `Omit`（`note` 载 `0 个测试文件`）；unsafe → `Reject`（文案载 `0` 个文件） | 表驱动 |
| `expand_template` | 正向 | `ExpandMode::WithFiles`：五占位符全替换、`{files}` 以单空格连接（1 / 2 / 50 元素清单）——与改造前逐字相同（AC-2） | 表驱动 |
| `expand_template` | 正向 | `ExpandMode::OmitFiles`：`{files}` 与其前导空白一并剥离（单空格 / 多空格 / 制表符 / 行首无空白四形态），其余四占位符逐字同义（AC-3） | 表驱动 |
| `expand_template` | 边界 | 模板不含 `{files}`（rust 档模板）→ 两态展开**逐字相同** | 单例 |
| `expand_template` | 边界 | `files == []` 空清单：`WithFiles` 保留前导空白（`{files}` → 空串）、`OmitFiles` 剥离前导空白 → 两态**不同串**（两态判据锚） | 单例 |
| `expand_template` | 边界 | `{files}` 一处 vs 多处出现 → 两态均全量处理（替换 / 逐一剥离），零 `{files}` 残留 | 表驱动 |
| `expand_template` | 异常 | 模板空串 `""` → 两态均返回 `""`；模板含超长填充（> 1000 字符）与特殊字符（emoji / `\n` / 制表符）→ 逐字保留、不 panic | 表驱动 |
| `ExpandMode` | 边界 | 枚举封闭性：仅 `WithFiles` / `OmitFiles` 两变体（穷尽 `match` 编译通过）；非法值在类型系统不可表示（无 `Default`、无 `From` 宽松入口） | 单例 |
| `line_budget` | 正向 | Windows 臂 `Some(7000)`；非 Windows 臂 `None`（`cfg` 断言，双臂各一条） | cfg 门控 ×2 |
| `CMD_LINE_BUDGET` | 边界 | 常量锚定 `== 7000`（`#[cfg(windows)]` 门控）；非 Windows 该常量不可见（编译期隔离即断言面） | cfg 门控 |
| `execute_plan` | 正向 | 未超预算（`Keep` 臂）：`exit_code == 0`、`error == None`、`fault == None`、`argv_note == None`、`argv_len ==` `WithFiles` 展开字符数；既有工件解析面零变化 | 单例 |
| `execute_plan` | 正向 | 超预算 + `omit_files_safe == true`（`Omit` 臂）：`exit_code == 0`、`argv_note == Some(…)`（与 `argv_degrade_note` 同串）、`argv_len` 仍为 `WithFiles` 长度、命令不含文件清单（shim 执行成功可证） | cfg 门控（Windows） |
| `execute_plan` | 异常 | 超预算 + `omit_files_safe == false`（`Reject` 臂）：`fault == Some(RunFault::Environment(…))`、`error == None`、`exit_code == EXIT_CODE_NO_STATUS`、`cases` / `source_files` / `measured` 全空、`argv_note == None`、`report_dir` 无 `summary.json`；**零 spawn 双证据** = ①命令程序名故意不可达仍返回 `Ok`（证明裁决先于 `ensure_program_resolvable` 与 spawn）②marker 文件不存在（AC-4） | cfg 门控（Windows） |
| `execute_plan` | 异常 | `OmitFiles` 后仍超预算 → 与不安全臂同一出口：`fault == Some(Environment(…))`、`error == None`、零 spawn，文案载两长度（AC-5） | cfg 门控（Windows） |
| `execute_plan` | 正向 | unix 臂零降级：展开 > 7000 字符（如 8000）仍原样执行——`fault == None`、`argv_note == None`、命令含文件清单（`line_budget() == None` 的语义臂） | cfg 门控（非 Windows） |
| `execute_plan` | 回归 | 既有超时 / 取消 / 瞬态耗尽 / 合并流 / 退出码谱 / 工件缺失用例：`fault` 三臂与 `error` 面逐字不变，仅补新字段断言（AC-8） | 回归（改写） |
| `PlanExecution.argv_len` | 边界 | 口径为 `chars().count()`：多字节路径展开 → 等于字符数而非字节数；`files` 空 + 模板静态填充 → 等于模板展开字符数 | 表驱动 |
| `PlanExecution.argv_note` | 正向 / 边界 | 三态：`Keep` → `None`；`Omit` → `Some(…)`；fault 早返（超时 / 取消 / `Reject`）→ `None` | 表驱动 |
| `ArgvDecision` | 正向 | 三变体经 `decide_argv` 逐一可达；`command` / `note` / `message` 字段经 `super::` 构造与匹配可断言（不为其可测性加 `pub`、不加仅测试用 `PartialEq` 派生） | 单例 |
| 既有展开调用点 | 回归 | `runner_test` 既有 10 处 `expand_template(..)` 调用补 `ExpandMode::WithFiles` 后逐字断言不变（五占位符 / rust 模板 / 清单规模 / 特殊字符边界）（AC-8） | 批量改写 |
| `non_zero_exit_error` / `ensure_program_resolvable` / `reachable_in` | 回归 | 零改面语义不变（既有用例全绿，零新增断言） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程 / shell 执行（`exec::run_shell` / `SystemCommand`） | **不 mock**：真实 spawn 平台脚本（非 Windows `sh -c '…'`、Windows `cmd /C "…"`）；观测面 = 退出码 / marker 文件 / 工件字节 / `output` 流 | `execute_plan` 全臂用例 |
| 「未拉起」断言 | **不 mock spawn**：以 ①程序名故意不可达仍返回 `Ok(fault)`（顺序证据）②marker 文件不存在（执行证据）双证据钉住（沿既有「已置位 cancel 零 spawn」先例） | `Reject` 两臂 |
| 平台预算 `line_budget()` / `CMD_LINE_BUDGET` | **不 mock**（`cfg` 真实取臂）：`execute_plan` 的 `Omit` / `Reject` 集成用例以 `#[cfg(windows)]` 门控，unix 零降级用例以 `#[cfg(not(windows))]` 门控；平台无关覆盖全部经 `decide_argv` 的 `budget` 入参 | 三臂集成 + unix 零降级 |
| 文件系统（`report_dir` / 工件 / marker） | **不 mock**：`tempfile::TempDir` 真实目录，断言真实落盘与不存在 | 全部 `execute_plan` 用例 |
| 进程环境（`PATH` / spawn cwd） | **不 mock 但隔离**：`crate::lock_test_env()` 环境锁窗口（既有 `env_window`）仅在改写 `PATH` 或与其它环境用例冲突的臂使用 | 程序解析相关臂 |
| `crate::environment` 文案两臂 | **不 mock**：真实组合（文案单点即被测语义）；本文件只断言 `fault` 变体与关键数字在场，文案逐字判据归 `environment_test` | `Reject` / `Omit` 文案传导 |
| 时间（`Instant` / `duration_ms`） | **不 mock**：耗时非确定，仅断言 `duration_ms >= 0`；墙钟上界仅作悬挂防护（既有先例） | 全部 `execute_plan` 用例 |
| `TestPlan` / `PlanExecution` | **非 mock**：结构体字面量直构（fixture 构造器补 `omit_files_safe` / `argv_len` / `argv_note`） | 全臂 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect_test.rs

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`collect_files` 为模块私有重构、`detect_files` / `detect_plans` 签名不变）；下列待测功能取自 design「类型定义」子表的 `TestPlan.omit_files_safe` 与「修改文件」单元格显式命名的 `collect_files`，未由源码导出声明反推。 -->

#### 待测功能

- `TestPlan.omit_files_safe`: detect 期一次算定的降级安全布尔量——`default_glob` 同管线（不施加 suite `excludes`）集合 == `files` 集合（类型定义；D7）
- `collect_files(project_root, abs_root, abs_cwd, suite, patterns, apply_excludes)`: 收集主体单点（`apply_excludes` 两态共用同一 glob 方言与剪枝）（变更清单 ① / D7；模块私有）
- `detect_plans(root, suites)`: 在 `files` 非空后补一次对照枚举并写入 `omit_files_safe`（变更清单 ③；签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `TestPlan.omit_files_safe` | 正向 | `includes` 未设（吃注册表 `default_glob`）→ 对照集合 == `files` → `true`；树内含 `node_modules` / `.cache` 剪枝目录时两侧同剪，仍 `true` | 单例 |
| `TestPlan.omit_files_safe` | 正向 | `includes` 显式写成与 `default_glob` 等价的同一花括号串（`**/*.{test,spec}.{ts,tsx}`）→ 集合相等 → `true` | 单例 |
| `TestPlan.omit_files_safe` | 异常 | `includes` 为**真子集**（仅 `**/*.test.ts`，同目录另有 `src/b.spec.ts` 命中 `default_glob`）→ 集合不等 → `false`（拒降级，保守方向） | 单例 |
| `TestPlan.omit_files_safe` | 异常 | `includes` 等价但 `excludes` **实际剔除**一个文件 → `files` 少一项、对照面仍含该文件 → `false`（对照面 MUST NOT 施加 excludes） | 单例 |
| `TestPlan.omit_files_safe` | 边界 | `excludes` 已声明但**零剔除** → 集合仍相等 → `true`（「excludes 在位」不等于「必然不安全」） | 单例 |
| `TestPlan.omit_files_safe` | 边界 | 清单规模：1 个文件 / 200 个文件 → 均 `true`（`BTreeSet` 序导出后逐项比较，无 O(n²) 观测面） | 表驱动 |
| `TestPlan.omit_files_safe` | 边界 | `includes` 零命中（`files == []`）→ 沿既有早跳**不产 plan**：无 `omit_files_safe` 面、零 panic、`plans.len() == 0` | 单例 |
| `TestPlan.omit_files_safe` | 边界 | 花括号多形态 + dot 目录剪枝 + 项目根相对口径全部在场 → 对照集合与 `files` 同源（零新 glob 方言、零新剪枝） | 单例 |
| `collect_files`（经 `detect_plans` 间接） | 正向 | `apply_excludes = true`（`detect_files` 薄包装路径）→ 被 suite `excludes` 命中的文件不在 `files`（既有语义零变） | 单例 |
| `collect_files`（经 `detect_plans` 间接） | 正向 | `apply_excludes = false`（对照面）→ 同一 glob 下被 `excludes` 命中的文件仍在对照集合（构成 4 号用例的差异来源） | 单例 |
| `collect_files` | 异常 | 非法 glob（未闭合花括号 / 非法字符）→ `Err` 沿既有文案「…includes glob 非法…」，不 panic；对照面与发现面**同文案同出口** | 表驱动 |
| `TestPlan` 其余字段 | 回归 | 既有 12 字段用例（`id` / `framework` / `root` / `cwd` / `files` / `config_args` / 双模板 / 覆盖格式与工件名 / 早跳 / 不支持框架 `Err`）全绿，仅构造点补 `omit_files_safe`（AC-8） | 回归（改写） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统与 `glob`（`detect_files` / `collect_files` 走盘） | **不 mock**：`TempWs` 真实 tempdir 树 + 真实 `glob` crate + 真实剪枝名单（`foundation::prune`）——「同一管线枚举」正是被测语义，mock 会抹掉判据 | 全部 `omit_files_safe` 用例 |
| 框架版本探测 spawn（`cargo --version` / `npx … --version`） | **mock**：进程边界替身 shim（既有 `install_cargo_shim` / `set_shim_env`，PATH 前插；避免真实工具链依赖与耗时） | 全部 `detect_plans` 用例 |
| 进程环境 `PATH` | **不 mock 但隔离**：`PathWindow` RAII 窗口 + `crate::lock_test_env()`（既有先例，drop 还原） | 需要 shim 命中的用例 |
| `config::TestSuite` | **非 mock**：结构体字面量直构（既有 `suite(root, framework, includes, excludes, config)` 装置） | 全用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`execute` / `outcome` / `fault_outcome` 签名不变）；下列待测功能取自 design「修改文件」单元格显式命名的 `push_plan_head` 与其消费面 `execute`，未由源码导出声明反推。 -->

#### 待测功能

- `push_plan_head(merged, plan, execution)`: 记录 plan 分隔头单点装配（头行 + 可选 `[argv]` 行）（变更清单 ① / D10 / D11；模块私有）
- `execute(…)` 的记录聚合面: 先写头再拼 `output`、每已执行 plan 恰一行头、fault 在位亦写头（变更清单 ②；签名不变）
- 零污染面: `build_sub_report` 的 `findings` 注入面零改 + `fault_outcome` / 复用分支零改（D6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `push_plan_head` | 正向 | 单 plan：写入 `\n=== plan {id} ({framework}) · files={n} · argv={m} 字符 ===\n` + `output` 逐字节紧随；头行以换行起手与收尾、恰一行 | 单例 |
| `push_plan_head` | 正向 | 多 plan（3 个）：头按 `plans` 迭代序（`BTreeSet` 字典序稳定）排列、每 plan 恰一行头、各段正文随自身头 | 单例 |
| `push_plan_head` | 正向 | 降级 plan（`argv_note == Some(note)`）→ 头后紧跟 `[argv] {note}\n`；非降级 plan（`None`）无 `[argv]` 行 | 表驱动 |
| `push_plan_head` | 边界 | `output` 空（fault 拒绝臂 / 零输出的已执行 plan）→ 记录**仅头**（含可选 `[argv]` 行），非空且可回查 | 表驱动 |
| `push_plan_head` | 边界 | `files == []`（rust 档，`files=0`）与 `argv_len == 0` → 头格式仍逐字（`files=0 · argv=0 字符`） | 单例 |
| `push_plan_head` | 边界 | `plan.id` / `framework` 含非 ASCII（中文 root 推导 id）与超长 `note`（> 1000 字符）→ 逐字节 UTF-8 写入、记录面不截断、不 panic | 表驱动 |
| `push_plan_head` | 边界 | 头行分隔符字面口径：分隔符为 ` · `（U+00B7，非 ASCII 竖线 / 连字符）；`argv=` / `files=` 为十进制整数 | 单例 |
| `execute` 聚合面 | 正向 | 全链 node-test shim 两 plan：record 以第一 plan 头起手；**去掉头与 `[argv]` 行后正文与各 suite 输出逐字节一致、plan 序保序**（改写既有「record 内容 = 脚本真实输出合并流」断言） | 改写 |
| `execute` 聚合面 | 正向 | 留痕三数一致：头中 `files=` 等于 `plan.files.len()`、`argv=` 等于 `execution.argv_len`（与 `expand_template` / `decide_argv` 同口径） | 单例 |
| `execute` 聚合面 | 异常 | fault 收口路径（`fault_outcome`）：该 plan 的头仍在记录中（被拒绝的展开长度可回查）、summary / 子报告零落盘（既有 fault 用例扩一条记录断言） | 改写 |
| `execute` 聚合面 | 边界 | 空 `output` 多 plan 全链 → 记录 = 头序列（非空、plan 序稳定）；零 plan 执行的复用分支（`&[]`）→ 记录仍为空串（既有用例零改动） | 改写 + 回归 |
| `build_sub_report` / findings | 异常 | 降级 plan（`argv_note == Some(…)`、`error == None`）→ `findings == None`；`summary.findings` 不含 note 文本；`conclusion` 仍 `Pass`（结构性不可达，非纪律）（AC-3 / AC-7） | 单例 |
| `build_sub_report` / findings | 边界 | 真红 plan（`error == Some(…)`）+ `argv_note == None` → findings 仍为该错误（零改注入面，既有语义不回退） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（`report_dir` / `exec-outputs` / 报告树） | **不 mock**：`tempfile::TempDir` + 既有 `ChainWs` 真实工作区（写 `openspec/config.json` + suite 输入文件） | 全链用例 |
| 子进程（suite 命令） | **mock**：既有 node shim（`.cmd` / `sh` 双形态，`--version` 臂在 `.cmd` 层短接；`SHIM_*` 环境变量控制工件拷贝 / marker / 退出码）——shell 执行不是被测面 | `execute` 全链用例 |
| `exec::record_exec_output` / `read_exec_output` | **不 mock**：真实记录单点 + 经 `exec_id` 回查（记录形态即被测语义） | record 断言 |
| 进程环境（shim 环境变量 / `PATH`） | **不 mock 但隔离**：`crate::lock_test_env()` + `set_shim_env` / `clear_shim_env`（既有先例） | `execute` 全链用例 |
| `TestPlan` / `runner::PlanExecution` | **非 mock**：结构体字面量直构（纯数据；`argv_len` / `argv_note` 由用例显式给值） | `push_plan_head` 用例 |
| `SummaryReport` / `SubReport` | **非 mock**：既有 `hand_summary` 等 fixture 直构 | findings / conclusion 断言 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/environment.rs -> packages/desktop/src-tauri/crates/infra/checks/src/environment_test.rs

> 说明：用例按 design 的四参签名设计（`omitted_len: Option<usize>`，design 待决问题 1）。若规格侧采纳「三参 + environment 内拆第三臂 `argv_budget_after_omit_text`」的替代方案，则第 2 / 4 号用例拆到新函数名下，其余用例映射不变。

#### 待测功能

- `environment::argv_budget_text(budget, expanded_len, omitted_len, files)`: 拒绝降级面 `RunFault::Environment` 摘要——定性 + 三数字（`omitted_len` 为 `Some` 时追加降级后长度）+ 动作（公共函数 / API；新增 `pub(crate)`）
- `environment::argv_degrade_note(budget, expanded_len, files)`: 安全降级面记录注——三数字 + 「已降级为框架自发现」，零动作建议（公共函数 / API；新增 `pub(crate)`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `argv_budget_text` | 正向 | `(7000, 8230, None, 180)` → 含定性（命令长度超平台上限 / `harness` 限制 / 非被测代码问题）、三数字（`8230` / `7000` / `180`）、动作（收窄 `includes` / 拆分 suite） | 单例 |
| `argv_budget_text` | 正向 | `omitted_len == Some(9000)` → 追加「降级为框架自发现后仍为 `9000` 字符」；`omitted_len == None` → 该句零出现 | 表驱动 |
| `argv_budget_text` | 异常 | 负向断言（逐词 `!contains`）：无「火绒」「云枢」「排除 workspace 父目录」（`ENV_GUIDANCE` 全句零误引）、无「已自动重试」「占用」「被锁文件」 | 单例 |
| `argv_budget_text` | 边界 | 三数字边界组合：`budget = 0` / `expanded_len = 0` / `files = 0`；`omitted_len == Some(0)`；三参同时 `usize::MAX` → 数值逐字呈现、不 panic、不回落省略 | 表驱动 |
| `argv_budget_text` | 边界 | 超长文本（多次数字拼接，输出 > 1000 字符路径）→ 不截断、不格式化 panic | 单例 |
| `argv_degrade_note` | 正向 | `(7000, 8230, 180)` → 含「展开 8230 字符」「平台预算 7000」「已降级为框架自发现」「180 个测试文件」 | 单例 |
| `argv_degrade_note` | 异常 | 零动作建议：不含「收窄」「拆分 suite」；不含 `ENV_GUIDANCE` / 占用者 / 重试次数措辞 | 单例 |
| `argv_degrade_note` | 边界 | `files = 0` / `budget = 0` / `expanded_len = 0` / `usize::MAX` 组合 → 逐字呈现、零 panic | 表驱动 |
| 两臂措辞分工 | 边界 | 互不含对方专属措辞：`argv_budget_text` 不含「已降级为框架自发现」；`argv_degrade_note` 不含「请收窄」 | 单例 |
| `fault_text` | 回归 | 逐字不变（回归锚）：既有瞬态耗尽场景（锁文本 + 5 次尝试）输出含归因段 / 「已自动重试 N 次」/ 进展行 / 末次诊断摘要 / `ENV_GUIDANCE`，与变更前一致（AC-8） | 回归 |
| `attempts_log` | 回归 | attempt 分隔头逐字不变（未重试 → 原样 `stdout ++ stderr`；重试 → 分段头齐备）（AC-8） | 回归 |
| `ENV_GUIDANCE` / `progress_line` / `clip_chars` | 回归 | 常量文本与私有函数语义零改（既有断言全绿，零新增） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 两臂被测函数 | **不引入 mock**：纯函数零 io / 零 async，四类参数字面量直传 | 全部文案用例 |
| `fault_text` 回归原料（`exec::attribution_text` / `ProcessOutput` / `progress_series`） | **不 mock**：真实组合（既有 `environment_test` 零 mock 先例——摘要口径与归因原料即被测语义） | `fault_text` / `attempts_log` 回归 |
| 文件系统 / 子进程 / 环境变量 | **零依赖**：本文件不触达任何进程边界 | —— |

---

## 不可测试项

| 模块 / 面 | 原因 |
|---|---|
| `packages/desktop/package.json`（AC-10） | 版本字符串无单测路径；由版本 diff 审查核验，`tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` 版本行不动为人工审查项 |
| `openspec/todo.md`（D13 登记） | 纯文档追加条目，无测试路径 |
| `openspec/config.json`（AC-0） | 本变更为该文件**零 diff** 的前置事实，非被测对象；由 `git diff` 空 + 真件 record 头 `argv=` 观察核验 |
| `crates/core/orchestration/**`（walker / port，AC-6） | 本变更零代码改动（design 只读红线），不在被测范围；`Environment → Terminal::failed` 收敛由既有 `walker_test` fault 用例回归，本变更不新增用例 |
| `crates/core/checks/**`（`aggregate.rs` / `collect_problems` / `in_suite_scope`） | 零改动；「降级说明不进 `findings`」以 `mod_test` 侧结构性断言（`build_sub_report` 的 `findings == None` + `conclusion` 仍 `Pass`）间接钉住，不在 core 侧重测 |
| 机械门禁面（AC-8 的 `pnpm -C packages/desktop run bindings:check`、AC-9 覆盖率门禁、`pnpm -C packages/desktop check`） | 静态脚本 / 门禁类步骤由桌面端执行，本设计只做分支枚举映射（见「验收范围」AC-9 行），不复制测试运行命令细节 |
| `plan.template()` 平台选臂 / `derive_plan_id` / `unsupported_framework_reason` 等零改面 | 无行为变更，沿用既有用例断言，不另设新用例章节 |
