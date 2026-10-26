# desktop-test-execution Specification（delta: test-exec-argv-budget）

## MODIFIED Requirements

### Requirement: 执行链与报告落盘

执行链 SHALL 依序：detect（config tests[] + glob → plan entries）→ 完整性校验（字段 / 计数 / conclusion 一致性 checklist）→ 复用门 → 逐 suite 命令模板展开（`{results_file}` / `{report_dir}` / `{config_args}` / `{files}`）→ **展开长度预算裁决**（超平台预算 → 安全降级或环境归因，见「命令展开长度预算与安全降级」）→ spawn（超时 + 输出捕获）→ 报告写盘 → 聚合（阈值 / override → conclusion）→ 诊断 findings。报告 SHALL 恒落 change 报告目录（`summary.json` + 逐 suite 子报告；路径经 foundation layout 推导，desktop 产品源码零磁盘路径字面量）；报告 MUST NOT 登记 file inventory / artifacts registry（MVP 口径）。突变面 SHALL 由独立 MutationTest 门禁步承载（见「MutationTest 确定性门禁步与时序」与 desktop-mutation-testing）：TestExecution 执行链不产突变结果；summary 的 `mutation` 位由 MutationTest 归并块回填（在位时），未跑 / diff 空跳过时保持 null（null 合法，见「机械 checklist 双槽与 evidence 三态」）。

#### Scenario: 全链绿跑落盘

- **WHEN** 全部 suite 执行成功且覆盖率达阈值
- **THEN** summary.json 与逐 suite 子报告落 change 报告目录，聚合 conclusion=pass，机械 checklist 可静态判定

#### Scenario: 报告路径经 layout 推导

- **WHEN** runner 写盘且对 desktop 产品源码做命名隔离扫描
- **THEN** 报告目录取自 foundation layout 推导（`reports/test` 挂 change 目录），零硬编码磁盘路径字面量，双禁令扫描通过

#### Scenario: 突变面独立步承载

- **WHEN** TestExecution 全链绿跑完成进入 MutationTest
- **THEN** TestExecution 产出不含突变结果（mutation 位由 MutationTest 归并回填或保持 null），两步报告各落其位

#### Scenario: 展开预算裁决先于 spawn

- **WHEN** 某 suite 的 `{files}` 展开后命令串超平台预算
- **THEN** 该 suite 在 spawn **之前**被裁决为「安全降级」或「环境归因」两种结局之一，MUST NOT 以未经裁决的超长命令串直接 spawn；其余 suite 的链路步序零变

### Requirement: 输出记录为脚本真实输出

test-execution 的输出记录 SHALL 由 summary JSON 重序列化改为**脚本真实输出**：runner 捕获的 `stdout_bytes + stderr_bytes` 合并流（不加 `2>&1`——rust 模板 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"` 已把 stdout 重定向进结果工件，`2>&1` 会让 stderr（编译错误）混入污染 `parse_cargo_test`）。`PlanExecution` SHALL 增 `output` 字段承载该合并流（当前 stdout_bytes / stderr_bytes 是局部变量、只进 error 尾部）；`mod.rs` 聚合各 suite `output` 按 plan 序（字典序稳定）拼接，经 `record_exec_output(root, change, merged)` 落单一 `.log` 并回填 `exec_id`。全量结构化报告（`summary.json` + 逐 suite 子报告）仍留 report_dir 子树，各框架 report 工件各自为政，MUST NOT 强行统一各框架脚本输出格式。

**plan 分隔头（本变更新增，元数据面）**：`mod.rs` 聚合时 SHALL 在每个**已执行** plan 的 `output` 之前写入一行
`=== plan {id} ({framework}) · files={n} · argv={len} 字符 ===`（plan 序稳定、以换行起手与收尾）；该 plan 发生 `{files}` 省略降级时 SHALL 紧随一行 `[argv] {降级说明}`（展开长度 / 平台预算 / 文件数 / 已降级为框架自发现）。分隔头与 `[argv]` 行为**记录元数据**，MUST NOT 被视为脚本输出，故不与本条首句「内容为脚本真实输出」冲突：去掉这些行后，记录正文 MUST 与各 suite 脚本输出逐字节一致、按 plan 序保序。零 plan 执行的复用分支记录仍为空。降级说明 MUST NOT 进入 `SubReport.findings` / `summary.findings`（`collect_problems` 会把「`failed == 0` 且 findings 非空」判成 `execution_error`，绿跑降级会因此翻红——信息面单点在本条记录头）。

#### Scenario: rust 套件不加 2>&1

- **WHEN** test-execution 执行 rust 套件（模板为单条 `cargo llvm-cov --workspace … > "{results_file}"`）
- **THEN** spawn 不施加 `2>&1`（stdout 仍重定向进结果工件供 `parse_cargo_test`）；record 内容主要为 stderr（编译错误面），`results.txt` 在报告树可见，不强行 tee

#### Scenario: 各 suite 输出按 plan 序聚合

- **WHEN** 一次 test-execution 含多个 suite（如 jest 与 rust），各 suite 脚本分别产出 stdout / stderr
- **THEN** record 落单一 `.log`，内容为各 suite 合并输出按 plan 序拼接（每段前有该 plan 的分隔头）；`read_exec_output` 回查内容在去掉分隔头与 `[argv]` 行后与脚本真实输出逐字节一致，MUST NOT 是 summary JSON 重序列化

#### Scenario: 结构化报告仍为全量权威

- **WHEN** record 改为脚本输出后 fail / error 收场
- **THEN** `summary.json` 与逐 suite 子报告仍落 report_dir 子树（全量结构化结论权威），`findings_detail` / 报告目录兜底语义不变，record 只作人工回查的终端还原

#### Scenario: 分隔头消歧与留痕

- **WHEN** 一次执行含 vp / rust 两 plan，其中 vp plan 展开后被降级
- **THEN** record 中两段输出各以自身 plan 头起手（`=== plan … · files=N · argv=M 字符 ===`），降级 plan 头后紧跟 `[argv]` 行；人工回查无需反推模板即可判定「哪段输出属于哪个 plan」「哪个 plan 发生过降级」

#### Scenario: 空输出多 plan 仍有头

- **WHEN** 各 plan 脚本未产出任何 stdout / stderr（如程序未拉起即被拒绝）
- **THEN** record 内容为各 plan 的分隔头序列（非空、plan 序稳定），仍可经 `exec_id` 回查；零 plan 执行的复用分支记录仍为空串

## ADDED Requirements

### Requirement: 命令展开长度预算与安全降级

test-execution 的命令展开 SHALL 受平台命令行长度预算约束（「命令行超平台上限」不再由 shell 层拒绝来表达）：

- **预算常量**：Windows 臂 SHALL 取 `CMD_LINE_BUDGET: usize = 7000`（字符口径，`chars().count()`），余量口径 = `cmd.exe` 硬上限 8191 − 1191（`cmd /C ` 前缀 + `raw_arg` 转引放大 + 版本差异；实测脚本 8000 / 8150 通过、8200 拒绝）。非 Windows 臂 SHALL 无预算（`line_budget() == None`）——`ARG_MAX` 为 2 MB 量级，MUST NOT 为它引入第二套降级语义。预算与 `SUITE_COMMAND_TIMEOUT` / `TESTEXEC_TOTAL_BUDGET` 同性质，为**代码常量**，MUST NOT 进 config schema。
- **判定时点与对象**：裁决 SHALL 在逐 suite 模板展开之后、程序可达性检查之前单点执行，判定对象为 `WithFiles` 展开后的命令串字符数；边界 MUST 为「`len <= 预算` 不降级 / `len > 预算` 进入降级或拒绝」。
- **三臂裁决**：

  | 展开长度 | 降级安全 | 结局 |
  |---|---|---|
  | `<= 预算` | — | 原样执行（命令与改造前逐字符一致，零降级痕迹） |
  | `> 预算` | 安全 | `OmitFiles` 重新展开后执行 + `argv_note` 留痕 |
  | `> 预算` | 不安全 | `RunFault::Environment(…)`（不 spawn） |
  | `OmitFiles` 展开后仍 `> 预算` | 任意 | `RunFault::Environment(…)`（降级无效，文案载原始与降级后两次长度） |

- **降级安全判据（集合相等，MUST NOT 模式文本比较）**：`TestPlan` SHALL 携 detect 期一次算定的 `omit_files_safe`——判据为「以该框架 `default_glob` + 同一剪枝管线（dot 目录 / 依赖与产物目录，**MUST NOT 施加 suite `excludes`**——框架自发现不认这套 excludes）枚举出的集合」**等于** `plan.files` 集合。`includes` 未设 / 与 `default_glob` 等价 → 相等（省略 `{files}` 跑的还是同一批）；`includes` 为真子集 → 不等 → MUST 拒绝降级；`excludes` 实际剔掉了某个文件 → 对照面仍含该文件 → 不等 → MUST 拒绝（保守：否则降级会跑出被 excludes 挡掉的文件）。MUST NOT 在 runner 侧二次自建 glob 枚举（方言与剪枝单点驻 `detect`）。
- **`OmitFiles` 展开语义**：`{files}` 占位符 SHALL 连其前导空白一并剥离（与 `{config_args}` 未注入时的既有语义对齐）；其余四个占位符（`{report_dir}` / `{results_file}` / `{coverage_file}` / `{config_args}`）逐字不变；模板不含 `{files}` 时（rust 档）两态展开 MUST 逐字相同。
- **环境归因**：拒绝降级时 runner SHALL 以 `fault = Some(RunFault::Environment(argv_budget_text(…)))` 正常返回（MUST NOT 返回 `Err`），该 plan MUST NOT 记 `execution_error`（命令行超限不是被测代码问题）、MUST NOT 写 summary / 子报告（半套报告不进复用门，沿既有 fault 收口）；walker 侧按既有「执行面归因 RunFault 与门禁步分派」收敛 `Terminal::failed`——MUST NOT 升格相位 fail、MUST NOT 进决策分叉、MUST NOT 发起 WorkerAgent 会话。文案三要点（定性 / 数字 / 动作）见 desktop-checks-domain「命令行超限归因文案单点装配」。
- **留痕**：`PlanExecution` SHALL 增 `argv_len: usize`（`WithFiles` 展开长度）与 `argv_note: Option<String>`（降级说明，文本由 `environment::argv_degrade_note(…)` 单点装配；未降级为 `None`），供 `mod.rs` 装配记录 plan 头（见「输出记录为脚本真实输出」）。降级说明 MUST NOT 进入 `SubReport.findings` / `summary.findings`——纯降级场景的 `conclusion` / `problems` MUST 零变化。
- **零扩面约束**：MUST NOT 传 glob 给框架（`vp` / `vitest` 位置参数不认 glob）、MUST NOT 写临时 `.cmd` / 响应文件、MUST NOT 走分块重跑（报告合并成本高于收益）、MUST NOT 改五框架模板文本、MUST NOT 加 `2>&1`。

#### Scenario: 预算边界两臂

- **WHEN** 展开长度恰等于预算，及恰为预算 + 1
- **THEN** 前者不降级（命令原样执行、`argv_note == None`），后者进入降级 / 拒绝裁决；边界口径由表驱动单元测试钉住（预算作纯函数入参，全平台可测）

#### Scenario: 未超预算零变化（本仓现状）

- **WHEN** 以收窄后的 `includes`（87 个测试文件）执行 vite-plus 档
- **THEN** 展开长度约 4527 字符 < 7000，命令串与改造前**逐字符一致**、无 `[argv]` 行、`argv_note == None`；record 头载实际长度（`argv=…`），结论与改造前一致

#### Scenario: 安全降级跑同一批且不翻红

- **WHEN** 展开长度超预算且 `omit_files_safe == true`（`includes` 等价 `default_glob`）
- **THEN** runner 以 `OmitFiles` 重新展开（`{files}` 与前导空白一并剥离）执行成功，`exit_code == 0`、`argv_note` 在场；`conclusion` 与 `problems` 零变化（绿跑仍绿）；record 中该 plan 头后跟 `[argv]` 行（长度 / 预算 / 文件数 / 已降级）

#### Scenario: 收窄集拒绝降级并显式报环境错误

- **WHEN** 展开长度超预算且 `omit_files_safe == false`（`includes` 为真子集）
- **THEN** 该 plan 不 spawn、`fault == Some(Environment(…))`、`error == None`；不写 summary / 子报告、`problems` 无 `execution_error`；文案含展开长度 / 平台预算 / 文件数与「收窄 includes 或拆分 suite」动作；walker 收敛 `Terminal::failed`（零 agent、零决策分叉）

#### Scenario: 降级无效仍拒绝

- **WHEN** `OmitFiles` 展开后长度仍超预算
- **THEN** 走与不安全臂相同的拒绝路径，文案附原始长度与降级后长度两个数字，MUST NOT 降级执行

#### Scenario: unix 臂零降级

- **WHEN** 在非 Windows 平台执行任意 suite（含超长展开）
- **THEN** `line_budget() == None`，裁决恒为原样执行，零降级零拒绝，行为与本变更前逐字一致

#### Scenario: 降级说明不污染结论

- **WHEN** 一次安全降级的绿跑完成报告写盘
- **THEN** `SubReport.findings` 与 `summary.findings` 均不含降级说明（findings 非空 + `failed == 0` 会被 `collect_problems` 判成 `execution_error`），`conclusion` 仍为 pass；说明只出现在输出记录的 plan 头

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/checks/src/testexec/runner.rs` | 预算裁决与展开 | `const CMD_LINE_BUDGET: usize = 7000`（`cfg(windows)`）+ `line_budget() -> Option<usize>`；`expand_template(template, report_dir, results_file, coverage_file, config_args, files, expand_mode: ExpandMode)`（`ExpandMode::{WithFiles, OmitFiles}`——`OmitFiles` 剥 `{files}` 前导空白，其余占位符同义）；纯函数 `decide_argv(...) -> ArgvDecision::{Keep{command}, Omit{command, note}, Reject{message}}`；`execute_plan` 在 `expand_template` 之后、`ensure_program_resolvable` 之前裁决；`PlanExecution { …, argv_len: usize, argv_note: Option<String> }` |
| `crates/infra/checks/src/testexec/detect.rs` | 降级安全判据 | `TestPlan.omit_files_safe: bool`——`default_glob` 集合（同一剪枝管线、**不施加 suite excludes**）== `plan.files` 集合；glob 方言与枚举逻辑零新增（复用 `detect_files` 既有函数） |
| `crates/infra/checks/src/testexec/mod.rs` | 记录头装配 | 逐 plan 写 `=== plan {id} ({framework}) · files={n} · argv={len} 字符 ===`（降级再补 `[argv] {note}`），plan 序稳定；复用分支（零 plan）零改动；`build_sub_report` MUST NOT 注入 `argv_note` 到 `findings` |
| `crates/infra/checks/src/testexec/runner.rs` | 归因出口 | 拒绝降级 → `fault = Some(RunFault::Environment(environment::argv_budget_text(…)))`，`error` 恒 `None`、工件解析跳过（沿既有 fault 早返分支） |
