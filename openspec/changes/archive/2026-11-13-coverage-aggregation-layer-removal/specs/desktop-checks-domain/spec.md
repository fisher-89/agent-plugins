# desktop-checks-domain Specification（delta: coverage-aggregation-layer-removal）

## MODIFIED Requirements

### Requirement: core/checks 纯层零进程边界

`crates/core/checks` SHALL 承载检查域纯层：报告 schema 模型（对齐 CLI zod 权威 schema）、coverage 解析器（istanbul / llvm-cov / coverage 文本）、**逐 suite 阈值判定与汇总布尔归并（conclusion 判定）**、完整性校验与诊断树确定性分支、复用门判定（mtime 新鲜度比较纯函数）。core/checks MUST NOT 进程 spawn、MUST NOT 依赖 tokio process、MUST NOT 依赖 Tauri；解析与聚合 SHALL 为纯函数，以 fixtures 直驱可测（corpus 黄金语料先行，文件系统扫描等 IO 侧落 infra）。

「逐 suite 阈值判定」的边界 SHALL 明确：跨 suite 的覆盖率数值 MUST NOT 在纯层被加权 / 摊平 / 圈定为 override 组——纯层对覆盖率只做两件事：**按单份子报告判通过与否**，以及**把各 suite 的布尔归并成汇总结论**。

#### Scenario: 纯函数 fixtures 直驱

- **WHEN** 以真实 summary.json / report.json fixtures 驱动 core/checks 的解析器与聚合判定
- **THEN** 无需进程、无需文件系统即可得出结论与诊断分支，测试全程进程内完成

#### Scenario: 零 spawn 审查

- **WHEN** 审查 core/checks 源码的依赖声明与进程创建调用
- **THEN** 无 tokio process 依赖、无进程 spawn、无 Tauri 依赖

#### Scenario: 纯层无跨 suite 数值聚合面

- **WHEN** 审查 `crates/core/checks` 对覆盖率数值的处理
- **THEN** 全 crate 无「多子报告逐文件条目累加成一个百分点」的函数、无按 suite 圈定 override 组的函数、无单框架缺省 glob 镜像；覆盖率数值只在单份子报告内被换算与比对

### Requirement: 覆盖率采集完整性（缺测不假通过）

覆盖率判定 SHALL **唯一化在子报告层（逐 suite）**，汇总层 MUST NOT 把多个 suite 的实测值加权 / 摊平成任何「全局覆盖率」：

- **判定点单源**：每 suite 的 `coverage.pass` SHALL 由**该 suite 自身阈值**判定（`coverage_meets_thresholds`；阈值定位沿 `suite_thresholds` 既有四档：framework + root 精确 → 框架 → 首个 suite → 缺省档，零改动）。
- **四形态判定（封闭集）**：

  | 形态 | 判定 |
  |---|---|
  | 该维度有值且达阈 | pass |
  | 该维度有值未达阈 | **fail** |
  | `measured` 内某维度为 `None`（工具自述不支持：llvm-cov `totals.<dim>.count == 0 → None`、node-test 覆盖表 `-` 列 → `None`、istanbul `total` 缺该指标键 → `None`） | 该维度**跳过**（其余维度照判） |
  | `coverage == None`（覆盖工件缺失 / 解析失败） | **该 suite 覆盖判定非通过**（不可测 ≠ 达标） |

- **`coverage == None` 的判据无条件**：五框架注册表模板恒带采集旗标（`detect.rs` 的 `coverage_format` 为 `CoverageFormat` 非 `Option`、`coverage_output` 恒有值，`config::TestSuite` 无任何 coverage 开关），故「本就不采覆盖率」在本仓不成立——MUST NOT 为 `None` 追加「仅当该 suite 结论非 pass 才判」之类的前置条件，MUST NOT 引入 config 声明键来表达「不采集」或「维度不适用」。
- **汇总 MUST NOT 产全局覆盖块**：`summary` MUST NOT 携 `coverage` 位（无 `measured` / `thresholds` / `overrides[]`）；跨 suite 只做布尔归并（见「汇总结论布尔归并与报告瘦身」）。覆盖率数值的唯一权威面是逐 suite 子报告（`reports/<slot>/<planId>/report.json`）。
- **归因逐 suite（报告自描述）**：每 suite 覆盖判定非通过 → `problems` 恰一条 `Problem { problem_type: CoverageFailure, framework: <该 suite 框架> }`——有实测未达阈时携逐维度缺口措辞（沿 `format_coverage_failure`，null 维度跳过），无覆盖数据时携采集缺失措辞（含退出码与「工件缺失 / 解析失败」定性），每 suite 至多一条。
- **与执行面归因互不顶替**：`coverage == None` MUST NOT 记为 `ProblemType::ExecutionError`（`conclusion = Error` 的成因面 SHALL 收窄为「结果工件 / 覆盖工件解析失败、harness 崩、框架探测失败」这类已经被既有 `execution_error` 覆盖的面）；同一 suite 同时有执行面与覆盖面问题时，两类条目**并存且互不改写**（覆盖缺失不再需要「与既有 `execution_error` 去重」的规则）。
- **既有归并规则零改**：失败用例归并（`test_failure` 至多 10 条 / suite）、`exit_code != 0 && failed == 0` 与「零失败但 findings 非空」的 `execution_error` 归并逐字保留。

#### Scenario: 单 suite 未达阈不被他 suite 掩盖

- **WHEN** vite-plus suite 三维全达标（lines 95.40 / branches 94.06 / functions 91.60），rust suite 的 `functions = 87.60`（< 90）而 `lines = 95.94` 达标、`branches = None`
- **THEN** 汇总 `conclusion == Fail`；`problems` 含一条 `framework == "rust"` 的 `coverage_failure`（措辞只点名 functions 缺口）；改造前该形态经加权稀释后判 pass——该分差即本 Scenario 的判红面

#### Scenario: 采集缺席判 fail 不判跳过也不判 error

- **WHEN** 某 suite 命令退出 0、用例全绿，但覆盖工件缺失（`coverage == None`），同时另一 suite 覆盖三维达标
- **THEN** 该 suite 覆盖判定非通过、汇总 `conclusion == Fail`（MUST NOT `Error`）、`problems` 恰一条该 suite 的 `coverage_failure`（MUST NOT `execution_error`）；另一 suite 的达标数值 MUST NOT 被呈现为整体通过（`summary` 无全局覆盖块）

#### Scenario: 工具自述不支持的维度仍跳过

- **WHEN** rust suite 子报告为 `lines 96.06` / `functions 91.35` 达标、`branches = None`（llvm-cov 分支计数为 0）
- **THEN** 该 suite `coverage.pass == true`、`problems` 零 `coverage_failure`；branches 阈值（85）不参与判定，MUST NOT 因 `None` 判红

#### Scenario: 汇总不落全局数值与 override 组

- **WHEN** 两 suite 各携逐文件覆盖条目且均达标，执行完毕后读 `summary.json`
- **THEN** summary 无 `coverage` 位、无 `overrides[]` 组、无任何跨 suite 加权数字；逐文件条目只在各自子报告内可见

#### Scenario: 执行面与覆盖面归因并存

- **WHEN** 某 suite 非零退出且 `coverage == None`（`collect_problems` 已为该 suite 落一条 `execution_error`）
- **THEN** `problems` 含该 suite 两条条目（一条 `execution_error`、一条 `coverage_failure`），既有 `execution_error` 文案逐字不改；汇总 `conclusion == Error`（execution_error 优先定型）

## ADDED Requirements

### Requirement: 汇总结论布尔归并与报告瘦身

汇总报告组装 SHALL 只做**计数求和 + 布尔归并**，MUST NOT 承载任何覆盖率 / 突变数值块：

- **字段面**：`SummaryReport` SHALL 恰为 `phase` / `command` / `timestamp` / `duration_seconds` / `total` / `passed` / `failed` / `skipped` / `conclusion` / `problems` / `plans` / `findings`——MUST NOT 携 `coverage` 位、MUST NOT 携 `mutation` 位。四计数仍为各子报告之和（仅供人读，MUST NOT 由计数推导覆盖率）。
- **结论判据**：`determine_conclusion` SHALL 只读布尔面——`has_execution_error` → `Error`；`failed > 0` → `Fail`；**任一 suite 覆盖判定非通过**（含 `coverage == None` 形态）→ `Fail`；否则 `Pass`。签名 MUST NOT 接受 `CoverageBlock` / `MutationBlock` 或任何覆盖率数值；MUST NOT 读取 `summary` 的覆盖率 / 突变位置（那两个位置不复存在）。
- **突变不参与汇总结论**：`SubReport.mutation` 块（解析保真面）MUST NOT 影响 `conclusion`。突变结论的唯一载体是 `MutationTest` 步载荷（`MutationTestOutcome.conclusion` / `score` / `threshold` / `counts`）与机械 checklist 第三槽（见 desktop-test-execution / desktop-mutation-testing）。
- **完整性 checklist ③ 改以子报告布尔重算**：`check_integrity` 的 ③（conclusion 一致性）SHALL 以「`summary.failed` + 各 `sub_reports` 的覆盖判定布尔 + `problems` 是否存在 `execution_error`」经**同一个** `determine_conclusion` 重算并与 `summary.conclusion` 比对（MUST NOT 就地另写一套判据）；① 四计数对账、② plan 报告齐全性 MUST NOT 改变（两者与覆盖率无关）。
- **复用门守恒**：复用门三条件（`plan_ids_match` + `check_integrity` 空 + `is_reusable`）语义 MUST NOT 变化；`is_reusable` 仍只判「结论非 `Error`」与「timestamp ≥ 最新输入 mtime」。
- **报告自描述度**：删掉全局覆盖块后，「哪个 suite 覆盖未达 / 哪个 suite 无覆盖数据」SHALL 由 `problems`（`framework` = 该 suite 框架、`problem_type = CoverageFailure`）与 `diagnose_findings` 的逐 suite 措辞回答；`plans[]` MUST NOT 新增 per-plan 结论字段，`PlanIndexEntry` 字段面零改动。
- **宽容解析不倒**：`parse_summary_report` 仍为宽容解析（无 `deny_unknown_fields`）——旧报告中的 `coverage` / `mutation` 键 SHALL 被忽略而不报错。

#### Scenario: 结论只读布尔

- **WHEN** 两 suite 覆盖判定均 pass、`failed == 0`、`problems` 无 `execution_error`
- **THEN** `conclusion == Pass`；把任一 suite 的覆盖判定改为非通过（有值未达阈或无数据）后同一输入 → `conclusion == Fail`，判据面上无任何「全局数字」介入

#### Scenario: 完整性 ③ 以子报告重算

- **WHEN** 磁盘上的 `summary.conclusion` 为 `pass`，而同目录子报告的 `coverage.pass` 为 `false`（或 `coverage` 缺席）且 `failed == 0`
- **THEN** `check_integrity` 返回一条 conclusion 一致性违例（重算得 `Fail`）；改造前该形态自己跟自己比（删块后比空集）恒过——该差异即本 Scenario 的判红面

#### Scenario: ① 与 ② 零改动

- **WHEN** 四计数与子报告之和不符，或 `plans[]` 与子报告按 planId 不双向对应
- **THEN** 违例文案与判定与改造前逐字一致（① / ② 不受覆盖率面退役影响）

#### Scenario: 突变不参与汇总结论

- **WHEN** corpus 语料中某子报告携非 pass 的突变块（解析保真样本）而覆盖判定全 pass、`failed == 0`
- **THEN** `conclusion == Pass`、summary 无 `mutation` 位；突变缺口不在 `findings` 里出现（4f 分支退役）

#### Scenario: 报告瘦身与旧报告宽容

- **WHEN** 新报告写盘后读 `summary.json`，以及读一份带 `coverage` / `mutation` 键的历史 `summary.json`
- **THEN** 前者无这两个键；后者仍能被 `parse_summary_report` 解析成功（两键被忽略），结论 / 计数 / problems 面照旧可用

#### Scenario: 复用门三条件守恒

- **WHEN** 既有 summary 齐整（integrity 空）、plan 集未变、输入 mtime 未变
- **THEN** 仍复用该 summary 零 spawn；三者任一不成立即失效重跑，语义与改造前逐字一致

### Requirement: 聚合链退役清单与解析保真面保留

加权聚合面的退役 SHALL 是「整链删除」而非「留半截」，同时 MUST NOT 波及解析保真面：

- **必须删除**（MUST 全删，全仓零引用、MUST NOT 保留私有副本或等价实现）：`aggregate_coverage` / `weighted_measured` / `compute_overrides` / `aggregate_measured` / `file_meets_thresholds` / `to_thresholds` / `in_suite_scope` / `under_root` / `default_glob` / `DEFAULT_THRESHOLDS` / `aggregate_mutation`；`core/checks/src/lib.rs` 对应再导出；`build_summary_report` 的 `suites: &[TestSuite]` 入参（唯一消费者是 `aggregate_coverage`）。
- **必须保留**（MUST NOT 顺手删）：
  - `is_file_excluded` 及其路径归一助手族——消费点仍是 `checks-runtime` 的突变归桶过滤（「`suite.excludes` 全桶消费」口径，见 desktop-mutation-testing）；
  - `coverage_meets_thresholds`（逐 suite 判定单点）；
  - `derive_plan_id` / `collect_problems` 的失败用例与 `execution_error` 归并 / `format_coverage_failure`；
  - `build_mutation_block` / `merge_mutation_blocks`（逐 suite 段块判定与载荷展示归并，见 desktop-mutation-testing）；
  - 模型族 `CoverageBlock` / `CoverageMeasured` / `CoverageThresholds` / `CoverageOverride` / `MutationBlock` / `MutationMeasured` / `MutationOverride` 与全部 `parse_*` 入口。
- **模型保留的理由是解析保真**：子报告仍携 `coverage` / `mutation` 块（`overrides` 字段照旧可解析），旧报告与 corpus 保真样本 MUST NOT 因本变更解析失败；纯层 MUST NOT 再生产 override 组、MUST NOT 由子报告块再导出全局块。
- **死代码零残留**：删除后不得以 `#[allow(dead_code)]`、`_` 前缀重命名、注释块等方式留存未用实现；以编译告警（`-D warnings`）为机械判据。

#### Scenario: 死代码零残留

- **WHEN** 以 `-D warnings` 编译 workspace 并 grep 上述被删符号
- **THEN** 零未用项告警、零引用（含测试与 `lib.rs` 再导出）；被删函数无任何形态的残留副本

#### Scenario: 保留面零 diff

- **WHEN** 审查 `is_file_excluded` / `coverage_meets_thresholds` / `build_mutation_block` / `merge_mutation_blocks` 与模型族
- **THEN** 函数签名与语义逐字不变；`checks-runtime` 的突变归桶过滤仍只经 `is_file_excluded` 取 excludes 口径

#### Scenario: 解析保真面不缩

- **WHEN** 以携 `coverage.overrides` 与 `mutation` 块的真实样本（corpus 保真语料）驱动解析
- **THEN** 两块的字段全部可读、未知字段被忽略，`SubReport` 结构面零变化；仅为「不再由纯层产出全局块」

## Module Contract

### Function: `packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `aggregate_coverage` / `weighted_measured` / `compute_overrides` / `aggregate_measured` / `file_meets_thresholds` / `to_thresholds` / `in_suite_scope` / `under_root` / `default_glob` / `DEFAULT_THRESHOLDS` / `aggregate_mutation` | **删除** | 加权 / override / 跨 suite 归并整链退役；静默跳过与「首 suite 阈值兜底」一并消失 |
| `determine_conclusion` | 改签名 / 改判据 | 入参改为「`failed` + 逐 suite 覆盖判定面（形态由 design 定：`&[SubReport]` 或等价布尔迭代面）+ `has_execution_error`」；判据 `Error` 优先 → `failed > 0` → 任一 suite 覆盖非通过 → `Pass`；MUST NOT 接受或读取 `CoverageBlock` / `MutationBlock` |
| `misses_coverage` | 改 | 判据退化为 `report.coverage.is_none()`（无条件），归因面由 `ExecutionError` 改 `CoverageFailure`；每 suite 至多一条、`framework` = 该 suite 框架 |
| `build_summary_report` | 改签名 / 改语义 | 去 `suites` 入参；不产 `coverage` / `mutation` 位；覆盖未达阈与采集缺席的问题条目逐 suite 产出；四计数求和、`plans[]`、`problems` 其余归并、`duration_seconds` 口径零改动 |
| `is_file_excluded` / `coverage_meets_thresholds` / `build_mutation_block` / `merge_mutation_blocks` / `derive_plan_id` / `collect_problems` / `format_coverage_failure` | **零改动** | 逐 suite 判定单点、突变块单点、归因文案与范围过滤单点均保留 |

### Function: `packages/desktop/src-tauri/crates/core/checks/src/diagnose.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `check_integrity` ③ | 改 | 以 `sub_reports` 的覆盖判定布尔 + `summary.failed` + `problems` 的 `execution_error` 存在性经同一个 `determine_conclusion` 重算并比对；①② 零改动 |
| `diagnose_findings` 4c | 改 | 改读 `problems` 中 `CoverageFailure` 条目逐条展开（framework / 缺口措辞已在条目内）；不再读 `summary.coverage`，签名不变（仍 `summary` 单入参） |
| `diagnose_findings` 4d / 4f | **删除** | 4d（pass 态「全部框架未产出覆盖数据」）与新语义互斥；4f（突变缺口）失去 `summary.mutation` 对象 |
| `diagnose_findings` 4a / 4b / 4e | **零改动** | 4e 措辞「全部测试通过且覆盖率达阈值」逐字保留（corpus 正向锚） |

### Model: `packages/desktop/src-tauri/crates/core/checks/src/model.rs` / `lib.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `SummaryReport.coverage` / `SummaryReport.mutation` | **删除字段** | 报告瘦身的落点；序列化面自此不含两键（旧文件仍可宽容解析） |
| `MutationBlock` 文档注释 | 改 | 去掉「由 MutationTest 归并回填 summary 的 mutation 位」，改为「子报告解析保真 + 载荷展示口径」 |
| `Problem.framework` 文档注释 | 改 | 「覆盖问题为该 suite 框架」（不再有 `all` 的全局覆盖 / 突变归因） |
| `lib.rs` 再导出 | 收窄 | 去 `aggregate_coverage` / `aggregate_measured` / `in_suite_scope`；其余导出零变化 |
| `CoverageBlock` / `CoverageMeasured` / `CoverageThresholds` / `CoverageOverride` / `MutationBlock` / `MutationMeasured` / `MutationOverride` / `SubReport` | **零改动** | 解析保真面与旧报告宽容解析的载体 |
