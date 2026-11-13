# 测试设计: coverage-aggregation-layer-removal

> **变更**: coverage-aggregation-layer-removal
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，不写测试命令细节
> **输入**: `proposal.md`（问题三口径 / 提案一~六 / D1–D7 / 变更范围实现文件 **7 行** + 测试文件 **9 行** / AC-1…AC-16 / R-1…R-12）、`design.md`（D1–D9 / 架构组件 8 行 / **新增文件 0** + 修改文件 **7 行** / 删除清单 **17 行** / 保留清单 **7 行** / 公共函数·API **10 行** / 类型定义 **6 行** / 编译闭合随动 8 行 / 交接 4 项 / AC 落点表）、三份 delta spec（`desktop-checks-domain` 2 MODIFIED + 2 ADDED / `desktop-test-execution` 6 MODIFIED / `desktop-mutation-testing` 1 MODIFIED）
> **被测范围**: design「修改文件」表 7 行的**源文件 6 项**（`core/checks/src/{aggregate,diagnose,model,lib}.rs`、`infra/checks/src/testexec/mod.rs`、`infra/checks/src/mutation/mod.rs`）+ 版本行 1 项（`packages/desktop/package.json`）。**新增文件零项、删除文件零项**；design 删除清单的 17 个符号属「缺席事实」，不纳入被测范围（判据见「不可测试项」）。
> **本变更性质**: 纯退役 + 一处口径收紧——汇总层加权聚合面整链退役（`summary.coverage` / `summary.mutation` 两块不再产出），判定点唯一化到逐 suite 子报告，汇总层只做**布尔归并与计数求和**；唯一行为收紧 = 覆盖率采集缺席由「静默跳过」改为「判非通过」。行为面集中在四处：① 纯层布尔归并（`determine_conclusion` 新签名 + 两个覆盖谓词 + 逐 suite 归因）；② 完整性 ③ 以子报告布尔重算（复用同一判据）；③ 诊断树 4c 改读 `problems`、4d / 4f 退役；④ 执行链两处落点（testexec 调用点收参、mutation 回填面删除）。

**测试框架与既有布局（盘点结论）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri/crates/{core,infra}/checks/**`） | `cargo nextest run --workspace`（`server:check` 的 `--all-targets` 编译闸先行） | **sidecar 同目录同前缀** `*_test.rs`，源文件 / `mod.rs` 以 `#[cfg(test)] mod <name>_test;` 挂载（`crates/core/checks/src/lib.rs:32-36` 的 `aggregate_test` / `diagnose_test` / `model_test` / `reuse_test` 四条先例、`crates/infra/checks/src/testexec/mod.rs:4-7` 与 `mutation/mod.rs` 的 `mod mod_test;` 先例）；crate 级集成测试驻 `tests/`（`crates/core/checks/tests/corpus_golden_test.rs` 先例） | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!` / `expect_err`，零第三方断言库；测试函数名为**中文短语 + `_` 连接**；装置为**临时 workspace（`tempfile::TempDir` + 真实 `.dev-team/config.json` + 真实报告树）+ PATH shim 假执行体（`SHIM_*` 环境变量 / `SpawnWindow` / `PathWindowGuard`）+ crate 级 env 锁 `lock_test_env`**（`mutation/mod_test.rs:51`、`testexec/mod_test.rs:47-110` 先例）、异步用例 `#[tokio::test]` |
| 前端（`packages/desktop/src/**`） | — | **零涉及**：summary 报告体从不出线（`TestExecutionOutcome` / `MutationTestOutcome` 载荷字段零改动，前端 `bindings.ts` 的唯一 coverage 引用是 config 阈值类型） | AC-14 以 diff 面登记（见「不可测试项」） |

**测试文件映射口径（源文件 -> 测试文件，逐对一章节）**

| 源文件（design「修改文件」） | 测试文件 | 形态 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs` | `packages/desktop/src-tauri/crates/core/checks/src/aggregate_test.rs` | 既有 sidecar 改造（删段 + 断言口径重定） |
| `packages/desktop/src-tauri/crates/core/checks/src/diagnose.rs` | `packages/desktop/src-tauri/crates/core/checks/src/diagnose_test.rs` | 既有 sidecar 改造（③ / 4c 重定，4d / 4f 删） |
| `packages/desktop/src-tauri/crates/core/checks/src/model.rs` | `packages/desktop/src-tauri/crates/core/checks/src/model_test.rs` | 既有 sidecar 改造（两字段面 + 旧报告宽容） |
| 同上（语料 golden 全链，执行落 `tests/`） | `packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs` | 既有 crate 级集成测试（投影段退役 + 四份 golden 重写） |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs`（同 crate 的 `testexec/report_test.rs` 仅机械闭合） | 既有 sidecar 改造（断言改指子报告面） |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs` | `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs` | 既有 sidecar 改造（七处断言反转） |
| `packages/desktop/src-tauri/crates/core/checks/src/lib.rs` | —（无单元测试载体） | 见「不可测试项」 |
| `packages/desktop/package.json` | — | 见「不可测试项」 |

> **复用（零语义改动，仅编译闭合）**：`crates/core/checks/src/reuse_test.rs`（`summary()` 字面量删 `coverage: None` / `mutation: None` 两行，断言零改——design 事实校准 2 / AC-9）；`crates/infra/checks/src/testexec/report_test.rs`（`summary()` 字面量删两段，余零改）。二者不单立章节，登记于 `model.rs` / `testexec/mod.rs` 章节的「↔ 编译闭合」行与「不可测试项」。

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、极值、恰好阈值、防御式分支、手搓不自洽形态）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **「既有，回归登记」**：该断言由既有用例锁定、本变更**逐字保留**（不新增断言、不改断言口径），本设计只登记判据力以防被顺手删改。
- **「↔ 编译闭合」**：该行只是让工作区随字段面 / 签名收敛编译通过的机械点位（design D8：删不可编译用例 / 去实参 / 去字段），**MUST NOT 新增用例或改断言值**；已在实现相位执行，本设计只登记落点与计数。

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| `u64`（`failed` 入参 / 四计数） | `0`（绿态）、`1`（最小失败）、`u64::MAX`（无溢出的两极）、`None` 不可表达（非 `Option`）；负值在 `u64` 类型层不可表达 ⇒ 以 `0` 与越界表达 |
| `&[SubReport]`（`determine_conclusion` / `build_summary_report` / `check_integrity` 入参） | `[]`（空集）、单元素、50 条超大列表；元素维度交叉：`coverage = None` / `Some(pass: true)` / `Some(pass: false)` / `mutation = Some(pass: false)` |
| `Option<CoverageBlock>`（`SubReport.coverage`） | `None`（采集缺失 / 工件解析失败）、`Some(pass: true)`、`Some(pass: false)`、`Some(pass: false)` 且三维 `measured` 全 `None`（手搓不自洽形态） |
| `Option<f64>`（`measured` 三维） | `None`（工具自述不支持：rust `branches`、node-test `-`）、`Some(0.0)`、`Some(100.0)`、恰等于阈值、低于阈值 0.1 |
| `bool`（`has_execution_error` / `block.pass`） | `true` / `false` 两值全覆盖（非 `Option`，无 `None` 面） |
| `&str`（`framework` / `root` / `command`） | `""`（空 framework）、超长（>1000 字符）、特殊字符（`\n` / `\0` / emoji）、`"."`（`derive_plan_id` 归一分支）、含 `\` 与 `/` 混用 |
| `&[TestSuite]`（`is_file_excluded` 入参） | `[]`（空 config）、单 suite、多 suite、`excludes = None` / `Some(vec![])` / 花括号方言 `**/*.{ts,tsx}` |
| `OffsetDateTime`（`started_at`） | 过去时刻 / 恰现在 / **未来时刻**（`duration_seconds` 的 `max(0.0)` 分支） |
| Enum | `Conclusion` 三值（`pass` / `fail` / `error`）全覆盖；`ProblemType` 三值（`test_failure` / `coverage_failure` / `execution_error`）全覆盖；非法枚举值 → serde `Err`（既有用例零改动） |
| `f64`（`duration_seconds` / mutation `score`） | `0.0`、非整秒舍入、负差截 0；mutation 面 `0.0` / `NaN` / `Inf` / 分母零 → `100.0`（既有用例零改动，回归登记） |

**跨模块组合用例挂靠（按链路发起方 / 最上层调用方）**

- **执行链**（`detect` → `build_sub_report` 逐 suite 判定 → `build_summary_report` 布尔归并 → `diagnose_findings` → `check_integrity` 复用门 → 报告落盘）最上层调用方 = `crates/infra/checks/src/testexec/mod.rs` 的 `ProcessTestExecution::run` ⇒ 组合用例写该章节。
- **突变执行链**（`bucket_suites` → 段执行 → `merge_mutation_blocks` 归并 → 载荷 `score/threshold/counts` → 逐 suite `all_pass`）发起方 = `crates/infra/checks/src/mutation/mod.rs` 的 `ProcessMutation::run` ⇒ 组合用例写该章节。
- **语料 golden 全链**（`parse_sub_report` → `parse_summary_report` → `check_integrity` → `diagnose_findings`）入口 = `crates/core/checks/src/model.rs` 的解析入口 ⇒ 组合用例写 `model.rs` 章节的 `#### 用例`，执行落 `tests/corpus_golden_test.rs`（不新建独立集成测试章节）。
- 不新建独立集成测试章节、不新建组合测试区；纯层（`core/checks`）内的跨函数链路（判定 → 归因 → 重算）一律写 `aggregate.rs` / `diagnose.rs` 各自章节。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **加权链整链退役**：`aggregate_coverage` / `weighted_measured` / `compute_overrides` / `aggregate_measured` / `file_meets_thresholds` / `to_thresholds` / `in_suite_scope` / `under_root` / `default_glob` / `DEFAULT_THRESHOLDS` / `aggregate_mutation` 全仓零引用（含 `lib.rs` 再导出与测试） | `.../crates/core/checks/src/aggregate.rs` -> `aggregate_test.rs`（删除面 ↔ 编译闭合行）、`.../crates/core/checks/src/lib.rs`（再导出，见「不可测试项」） |
| AC-2 | **summary 面瘦身**：`SummaryReport` 仅剩 12 字段；生产报告 `summary.json` 无 `coverage` / `mutation` 键 | `.../crates/core/checks/src/model.rs` -> `model_test.rs`、`.../crates/core/checks/src/aggregate.rs` -> `aggregate_test.rs`（组装面序列化判据）、`.../crates/infra/checks/src/testexec/mod.rs` -> `mod_test.rs`（真跑报告面） |
| AC-3 | **判定点唯一且三态**：有值达阈 pass / 有值未达阈 fail / 维度 `None` 跳过 / `coverage == None` fail；四形态各有用例 | `.../crates/core/checks/src/aggregate.rs` -> `aggregate_test.rs`（`determine_conclusion` 四形态 + `coverage_meets_thresholds` 回归）、`.../crates/infra/checks/src/testexec/mod.rs` -> `mod_test.rs`（`build_sub_report` 唯一判定点经子报告 `coverage.pass` 观测） |
| AC-4 | **掩盖不可达（核心回归锚）**：vite-plus 三维全达标 + rust `functions 87.60`（< 90）⇒ `conclusion == Fail`、`problems` 含 rust 的 `CoverageFailure` | `.../crates/core/checks/src/aggregate.rs` -> `aggregate_test.rs`（`determine_conclusion` + `build_summary_report` 双行）、`.../crates/infra/checks/src/testexec/mod.rs` -> `mod_test.rs`（链级组合行） |
| AC-5 | **缺采集判 fail 不判 Error**：绿跑但 `coverage == None` ⇒ `conclusion == Fail`、`problems` 恰一条该 suite 的 `CoverageFailure`、他 suite 达标值不被呈现为整体通过 | `.../crates/core/checks/src/aggregate.rs` -> `aggregate_test.rs`、`.../crates/core/checks/src/diagnose.rs` -> `diagnose_test.rs`、`.../crates/infra/checks/src/testexec/mod.rs` -> `mod_test.rs` |
| AC-6 | **rust `branches` 显式不支持仍跳过**：`branches: None` + lines / functions 达标 ⇒ 该 suite 覆盖判定 pass（零翻转） | `.../crates/core/checks/src/aggregate.rs` -> `aggregate_test.rs`（`coverage_meets_thresholds` 与 `block.pass` 单一真值源两行） |
| AC-7 | **结论只读布尔**：`determine_conclusion` 签名内无 `CoverageBlock` / `MutationBlock`；`build_summary_report` 无 `suites` 参数 | `.../crates/core/checks/src/aggregate.rs` -> `aggregate_test.rs`（新签名调用面即证据）；`grep` 面见「不可测试项」 |
| AC-8 | **完整性 ③ 更强**：篡改子报告 `coverage.pass` 使结论无法由布尔重算复现 ⇒ `check_integrity` 违例；① 四计数、② plan 索引既有用例逐字保留 | `.../crates/core/checks/src/diagnose.rs` -> `diagnose_test.rs` |
| AC-9 | **复用门三条件守恒**：`plan_ids_match` + `integrity` 空 + `is_reusable` 语义零改动；覆盖缺失的 fail 报告按既有 fail 同待遇 | `.../crates/infra/checks/src/testexec/mod.rs` -> `mod_test.rs`（两行复用门用例）、`.../crates/core/checks/src/reuse_test.rs`（仅字面量去两字段，断言零改，见「不可测试项」） |
| AC-10 | **`summary.mutation` 不落盘**：`backfill_summary` 与调用点删除；执行完 `summary.json` 无 `mutation` 键；载荷 `score` / `threshold` / `counts` 仍在 | `.../crates/infra/checks/src/mutation/mod.rs` -> `mod_test.rs`（七处断言反转 + 载荷用例） |
| AC-11 | **突变不入汇总结论**：`SubReport.mutation` 非 pass 不影响 `summary.conclusion` | `.../crates/core/checks/src/aggregate.rs` -> `aggregate_test.rs`（`determine_conclusion` 突变退出判定行）；语料 `mutation-block` 零 diff 见「不可测试项」 |
| AC-12 | **corpus golden 显式重写**：4 份仅 `computedCoverage` 键消失、另 4 份零 diff；`corpus_golden_test.rs` 无 `aggregate_coverage` 残留 | `.../crates/core/checks/src/model.rs` -> `.../crates/core/checks/tests/corpus_golden_test.rs`（golden 行）；重写纪律见「不可测试项」 |
| AC-13 | **旧报告仍可读**：带 `coverage` / `mutation` 键的历史 `summary.json` 经 `parse_summary_report` 成功解析 | `.../crates/core/checks/src/model.rs` -> `model_test.rs` |
| AC-14 | **零 schema / 零 IPC / 零前端**：`config.schema.json`、`bindings.ts` / `bindings.zod.ts`、`packages/desktop/src/**` 零 diff | 无单测载体——diff 面（见「不可测试项」） |
| AC-15 | **门禁**：`pnpm -C packages/desktop run check` 绿；`cargo nextest run --workspace` 全绿；`package.json` `0.6.38 → 0.6.39`，`src-tauri/Cargo.toml` crate 版本行不变 | `packages/desktop/package.json`（版本行，见「不可测试项」）；机械门禁由桌面端相位执行 |
| AC-16 | **边界零 diff**：`crates/core/config/**`、`crates/core/orchestration/**`、`crates/core/workflow/**`、`crates/infra/exec/**`、`testexec/report.rs`、`packages/desktop/src/**`、`.dev-team/architecture/**`、`explore.md`、`reports/baseline/**` 零 diff | 无单测载体——diff 面（见「不可测试项」）；行为侧证 = 各章节「既有，回归登记」行 |

---

## 单元测试

### `packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs` -> `packages/desktop/src-tauri/crates/core/checks/src/aggregate_test.rs`

**既有 sidecar 改造**：整段删除以 `aggregate_coverage` / `in_suite_scope` 为被测对象的 6 条用例与 8 条旧四参 `结论判定_*` 用例（被测对象不存在，实现相位已按 D8 执行）；`determine_conclusion` 用例组按新签名重写为**布尔面 + 逐 suite 遍历**；`build_summary_report` 用例组去 `suites` / `&[]` 实参、断言改为「summary 无 `coverage` / `mutation` 块 + `problems` 携逐 suite 覆盖归因」；`merge_mutation_blocks` / `build_mutation_block` 组与 `coverage_meets_thresholds` 断言**零改动**（保留面回归登记）。新增两类：`is_file_excluded` 直测组（原唯一间接覆盖面 `in_suite_scope` 已删，保留面必须有直测）、`coverage == None ⇒ Fail + 一条 `coverage_failure`` 的缺采集组。

#### 待测功能

- `determine_conclusion(failed: u64, sub_reports: &[SubReport], has_execution_error: bool) -> Conclusion`: 判据顺序 `has_execution_error → Error` / `failed > 0 → Fail` / 任一 suite 覆盖判定非通过（含 `coverage == None`）→ `Fail` / 否则 `Pass`；MUST NOT 接受或读取 `CoverageBlock` / `MutationBlock`；**突变不参与汇总结论**。（公共函数 / API，修改）
- `build_summary_report(sub_reports: &[SubReport], command: &str, started_at: OffsetDateTime) -> SummaryReport`: 计数求和 + `problems` 归并（含逐 suite `CoverageFailure`）+ `plans[]` + `conclusion` + `duration_seconds`；MUST NOT 产 `coverage` / `mutation`；`suites` 形参退役。（公共函数 / API，修改）
- `is_file_excluded(path: &str, suites: &[TestSuite]) -> bool`: 突变归桶过滤唯一入口（`mutation/mod.rs:435`）；`excludes` 全桶消费口径。（公共函数 / API，零改动保留面）
- `coverage_meets_thresholds(measured: &CoverageMeasured, thresholds: &CoverageThresholds) -> bool`: 逐 suite 判定单点（维度 `None` 跳过）。（公共函数 / API，零改动保留面）
- `build_mutation_block(measured: MutationMeasured, threshold: f64) -> MutationBlock`: score 公式单点（`(killed+timeout)/(total-ignored-compileError-runtimeError)`、分母零 → `100.0`、两位舍入、`pass = score >= threshold`）。（公共函数 / API，零改动保留面）
- `merge_mutation_blocks(blocks: &[MutationBlock]) -> Option<MutationBlock>`: 载荷展示归并（段计数求和、阈值取首块、空 → `None`）；**MUST NOT 作门禁判据**。（公共函数 / API，零改动保留面）

<!-- design「公共函数 / API」表另有本文件 3 行标记为「删除」（aggregate_coverage / aggregate_measured / in_suite_scope）——退役符号无被测对象，其零残留判据登记于「不可测试项」。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `determine_conclusion` | 正向 | 全绿布尔面：`failed = 0` + 两子报告 `coverage = Some(pass: true)` + `has_execution_error = false` → `Pass` | 单次 |
| `determine_conclusion` | 正向 | **掩盖不可达（AC-4 核心回归锚）**：vite-plus（`lines 95.40` / `branches 94.06` / `functions 91.60`，全达标）+ rust（`lines 95.94` / `branches None` / `functions 87.60 < 90`）两 `SubReport` → `Fail`（一个 suite 的达标值 MUST NOT 稀释另一个的 functions 缺口） | 单次 |
| `determine_conclusion` | 正向 | `failed > 0` → `Fail`：覆盖全 pass、无 `execution_error`、`failed = 1` | 单次 |
| `determine_conclusion` | 正向 | `has_execution_error` 优先级最高：`failed = 5` + 覆盖未达 + error → `Error`；`failed = 0` + 覆盖全 pass + error → `Error` | 参数化迭代（2 形态） |
| `determine_conclusion` | 正向 | 突变退出判定（AC-11）：`failed = 0` + 覆盖全 pass + `SubReport.mutation = Some(pass: false)` → `Pass`（突变只走 `MutationTest` 载荷） | 单次 |
| `determine_conclusion` | 异常 | 缺采集非 Pass 非 Error（AC-3 / AC-5）：绿跑子报告（`exit_code = 0` / `failed = 0`）`coverage = None` + 另一 suite 达标 → `Fail` | 单次 |
| `determine_conclusion` | 异常 | 两缺口并存：`failed = 0` + 一 suite `coverage = Some(pass: false)` + 另一 suite `coverage = None` → `Fail`（单一结论，零重复判据） | 单次 |
| `determine_conclusion` | 边界 | 空集两极：`sub_reports = []` + `failed = 0` + 无 error → `Pass`；同空集 + `has_execution_error = true` → `Error` | 组合迭代（2 组合） |
| `determine_conclusion` | 边界 | 计数两极：`failed = 0` → 不触发 fail 臂；`failed = u64::MAX` → `Fail`（零溢出、零饱和） | 参数化迭代（2 形态） |
| `determine_conclusion` | 边界 | `block.pass` 单一真值源（D2 不重算维度）：`coverage = Some(pass: true)` 而 `measured.lines < thresholds.lines`（手搓不自洽块）→ `Pass`；`Some(pass: false)` 而三维全 `None` → `Fail` | 组合迭代（2 组合） |
| `determine_conclusion` | 边界 | 维度 `None` 跳过（AC-6）：rust 形态 `measured { lines: Some(95.94), branches: None, functions: Some(87.60) }` 且 `block.pass = true` → `Pass`；同形 `block.pass = false` → `Fail`（判据只读块布尔） | 组合迭代（2 组合） |
| `build_summary_report` | 正向 | 全测齐全零变（改写既有用例）：两 suite 达标 + 全绿 → 四计数求和、`problems` 空、`conclusion = Pass`、`plans` 2 条（`id == derive_plan_id(root, framework)`、`path == "reports/test/<id>"`）、`phase == "test-execution"`、`command` 透传、`duration_seconds` 整秒 | 单次 |
| `build_summary_report` | 正向 | **summary 瘦身面（AC-2）**：上述 summary 经 `serde_json::to_value` → 顶层键集 = `phase` / `command` / `timestamp` / `duration_seconds` / 四计数 / `conclusion` / `problems` / `plans`（+ 可选 `findings`），`has("coverage") == false` 且 `has("mutation") == false` | 单次 |
| `build_summary_report` | 正向 | 逐 suite 覆盖归因（AC-4）：vite 达标 + rust `functions 87.60 < 90` → `problems` 恰一条 `coverage_failure`、`framework == "rust"`、`message == "Coverage below threshold: functions 87.6% < 90%"`（`format_coverage_failure` 原文，零改写） | 单次 |
| `build_summary_report` | 异常 | 缺采集归因（AC-5）：单 suite 绿跑（`exit_code = 0` / `failed = 0`）`coverage = None` → `problems` 恰一条 `coverage_failure`（`framework` = 该 suite、消息含「覆盖率采集缺失」「退出码 0」与动作指引）、**零** `execution_error`、`conclusion = Fail` | 单次 |
| `build_summary_report` | 异常 | 两类归因并存互不顶替（R-10）：同 suite `exit_code = 1` + `failed = 0` + `coverage = None` → 恰两条（`execution_error` 文案 `Exit code 1` 零改写 + `coverage_failure`）、`conclusion = Error`（error 优先定型） | 单次 |
| `build_summary_report` | 异常 | 失败用例 + 缺采集：`exit_code = 0` + `failed = 2` + `coverage = None` → 2 条 `test_failure` + 1 条 `coverage_failure`、`conclusion = Fail`（MUST NOT 升格 `Error`）；`problems[0]` 仍为 `test_failure`（同 suite 内条目序不变） | 单次 |
| `build_summary_report` | 边界 | 零子报告：`build_summary_report(&[], "cmd", now)` → 四计数 `0`、`problems` 空、`plans` 空、`conclusion = Pass`、可序列化且无两键 | 单次 |
| `build_summary_report` | 边界 | 超大列表：50 条子报告中恰 1 条 `coverage = None` → 恰 1 条 `coverage_failure`、其余零归因、`conclusion = Fail` | 单次 |
| `build_summary_report` | 边界 | 每 suite 至多一条：同 suite `Some(pass: false)` 且 measured 三维全缺口 → 恰一条（多维缺口合成一条 message）；`Some(pass: false)` 且三维全 `None` → 仍一条（`message` 前缀 `Coverage below threshold:`、零维度片段——判据只读 `block.pass`） | 参数化迭代（2 形态） |
| `build_summary_report` | 边界 | `plans[]` 边界：`root = "."` → `id == framework`；同 root 不同 framework → 两不同 id；`root` 含 `\` / `/` → 归一为 `_` | 参数化迭代（3 形态） |
| `build_summary_report` | 边界 | `duration_seconds` 边界：`started_at = now + 5s`（未来时刻）→ `0.0`（`max(0.0)` 不产负值）；`started_at ≈ now` → `0.0` | 参数化迭代（2 形态） |
| `build_summary_report` | 边界 | `framework` 串边界：`""` / 超长（1200 字符）/ 含 `\n`·`\0`·emoji → `coverage_failure.framework` 逐字透传（零 trim、零截断） | 参数化迭代（3 形态） |
| `is_file_excluded` | 正向 | 保留面直测（原间接覆盖面 `in_suite_scope` 已删）：`excludes = ["**/ui/**"]` + root `src/app` + 路径 `src/app/ui/button.ts` → `true`；`src/app/main.ts` → `false` | 组合迭代（2 形态） |
| `is_file_excluded` | 异常 | 非排除臂：`excludes = None` → `false`；`excludes = Some(vec![])` → `false`；路径不在任何 suite root 下（`other/x.ts`）→ `false`；`suites = []` → `false`（四形态皆不剔除，零 panic） | 参数化迭代（4 形态） |
| `is_file_excluded` | 边界 | 路径形态边界：反斜杠 `src\app\ui\x.ts`、尾随 `/`、绝对 POSIX 路径（`/repo/src/app/ui/x.ts`，以 `/<root>/` 段标记容忍项目前缀）、`root = "."`（`rel_to_root == "."` 的 `root.clone()` 分支）、花括号方言 `**/*.{ts,tsx}` | 参数化迭代（5 形态） |
| `coverage_meets_thresholds` | 正向 | 三维有值且 ≥ 阈（含恰等于阈值）→ `true`；任一维低于阈值（`79.999 < 80`）→ `false` | 既有，回归登记 |
| `coverage_meets_thresholds` | 异常 | 病态数值不误判通过：`Some(f64::NAN)` 维度（`NaN < threshold` 为 `false` ⇒ 视为达标，沿既有比较语义）/ `Some(-1.0)` 负覆盖 → `false` / 阈值 `0.0` + 实测 `0.0` → `true`（零阈不误 fail） | 参数化迭代（3 形态） |
| `coverage_meets_thresholds` | 边界 | 维度 `None` 跳过（AC-6 / D4）：`branches = None` + lines / functions 达标 → `true`；三维全 `None` → `true`（零维度面不误判） | 既有，回归登记 |
| `build_mutation_block` / `merge_mutation_blocks` | 正向 | score 公式单点与逐段计数求和（`(killed+timeout)/(total-ignored-compileError-runtimeError)`、分母 `0` → `100.0`、两位舍入、`pass = score >= threshold`）；阈值取首块 | 既有，回归登记 |
| `build_mutation_block` / `merge_mutation_blocks` | 异常 | 极端输入：分母零（无有效突变体）/ 阈值恰等 / 超大计数 / `NaN`·`Inf` 阈值不 panic | 既有，回归登记 |
| `merge_mutation_blocks` | 边界 | 空 slice → `None`；单段等值透传；超大列表顺序无关（D7 保留面：载荷 `score/threshold/counts` 的唯一计算面） | 既有，回归登记 |
| 签名面（AC-7） | ↔ 编译闭合 | `determine_conclusion(failed, sub_reports, has_execution_error)` 三实参调用即「签名内无 `CoverageBlock` / `MutationBlock`」的编译面证据（旧四 / 五参调用点全数消失即 `E0061`）；`build_summary_report(sub_reports, command, started_at)` 三实参即「无 `suites` 形参」证据 | 单次 |
| 删除面（11 符号：`aggregate_coverage` / `weighted_measured` / `compute_overrides` / `aggregate_measured` / `file_meets_thresholds` / `to_thresholds` / `in_suite_scope` / `under_root` / `default_glob` / `DEFAULT_THRESHOLDS` / `aggregate_mutation`） | ↔ 编译闭合 | 删 8 条旧四参 `结论判定_*` + 6 条覆盖聚合 / suite 范围用例（`多子报告全局聚合与per_suite_override组` / `suite阈值缺失走默认档不panic` / `子报告source_files全空_产出null零维面` / `suite_glob不命中_override组不产条目` / `suite范围_花括号includes与excludes` / `suite范围_缺省glob花括号命中`）+ `汇总组装逐字段相等_全测齐全` + `aggregate_mutation同式对拍_经summary组装面`；其余 `build_summary_report` 调用点逐处去 `&suites` / `&[]` 实参、删 `summary.coverage` / `summary.mutation` 读取行；成孤装置（`block()` / `config_thresholds()` / `suite()` / `entry()` 等）按 `cargo check --all-targets` 未用告警逐个删——MUST NOT 新增用例或改断言口径 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock（纯函数层） | 无 IO、无 spawn、无网络、无 DB；`core/checks` 的纯层红线（零进程边界）即被测契约的一部分 | 全章节 |
| 不 mock：`SubReport` / `CoverageBlock` / `CoverageMeasured` / `MutationBlock` | 真实模型实例手工构造（`sub_report()` / `block()` / `measured()` 既有装置）——判据只读 `coverage.is_some()` 与 `block.pass`，替身化会让「不重算维度」自证空转 | 全章节 |
| 不 mock：`globmatch::match_glob` | 真实 glob 引擎（花括号交替方言与 `/<root>/` 段匹配即 `is_file_excluded` 的被测语义） | `is_file_excluded` 三行 |
| 时钟：`OffsetDateTime` 走显式入参 | 不 mock 时钟机关——`started_at` 是 `build_summary_report` 的入参，过去 / 恰好 / 未来三形态由调用侧直接构造 | `build_summary_report` 的 `duration_seconds` 行 |
| 不 mock：`serde_json` | 真实序列化文本（「无 `coverage` / `mutation` 键」只能由真实产物观测） | AC-2 瘦身面行 |

### `packages/desktop/src-tauri/crates/core/checks/src/diagnose.rs` -> `packages/desktop/src-tauri/crates/core/checks/src/diagnose_test.rs`

**既有 sidecar 改造**：`consistent_summary` 去掉 `build_summary_report` 的 `&suites` 实参（`suite()` / `config_thresholds()` 随之成孤即删）；`hand_summary` 去 `coverage` 形参与 `coverage` / `mutation` 两字面量字段，七个调用点同步去第 4 实参；删两条旧 4c 用例（`覆盖未达阈_阈值比对findings携维度与数值`、`coverage维度null_null感知跳过不产覆盖findings`）；`全绿summary零误报_findings仅单条正向` 的 `["全部测试通过且覆盖率达阈值"]` 断言**逐字保留**。新增：③ 的「篡改子报告 `coverage.pass` ⇒ 违例」与「缺采集子报告对记载 pass ⇒ 违例」两行、4c 的 `problems` 取数三行。

#### 待测功能

- `check_integrity(summary: &SummaryReport, sub_reports: &[SubReport]) -> Vec<String>`: ③ 由「拿 `summary.coverage` / `summary.mutation` 自比」改为「以子报告布尔 + `summary.failed` + `problems` 的 `execution_error` 重算」，**复用 `determine_conclusion`**（不得就地重写布尔归并）；①② 逐字不动；违例文案改「计数 / 逐 suite 覆盖判定面」。（公共函数 / API，修改，签名不变）
- `diagnose_findings(summary: &SummaryReport) -> Vec<String>`: 4a / 4b / 4e 逐字保留（4e 改为 `conclusion == Pass` 时**无条件**产单条）；4c 改读 `problems` 中 `CoverageFailure` 条目逐条展开（`覆盖判定未通过 [{framework}]: {message}`，`take(CLUSTER_FRAMEWORK_LIMIT)`）；4d / 4f 退役。（公共函数 / API，修改，签名不变）

<!-- design「公共函数 / API」表对本文件只声明上面两行；私有 `threshold_findings` / `threshold_brief` 的退役（design 事实校准 3）行为经 4c 新取数面间接观测，其零残留判据登记于「不可测试项」。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check_integrity` | 正向 | 一致 summary + 子报告全集（由 `build_summary_report` 真实组装，去 `suites` 形参）→ 三查全过、空 findings | 单次 |
| `check_integrity` | 正向 | 缺采集 fail 报告自洽（AC-5 / AC-8）：绿跑 `coverage = None` 的子报告 → summary `conclusion = Fail` + 一条 `coverage_failure` → ③ 重算同为 `Fail`、零违例（新判据不放宽既有结论） | 单次 |
| `check_integrity` | 正向 | 突变块不入重算（AC-11）：子报告 `mutation = Some(pass: false)` + 覆盖全 pass + `failed = 0` + summary `conclusion = Pass` → 零违例 | 单次 |
| `check_integrity` | 异常 | **篡改子报告 `coverage.pass`（AC-8 核心行）**：子报告块 `pass = false` 而 summary 记 `conclusion = Pass` → 恰一条违例，文案含 `conclusion 一致性失败` 且两侧 `"fail"` / `"pass"` 俱陈 | 单次 |
| `check_integrity` | 异常 | 缺采集子报告 vs 记载 pass：子报告 `coverage = None`（`exit_code = 0` / `failed = 0`）而 summary `conclusion = Pass` → 违例（重算 `Fail`） | 单次 |
| `check_integrity` | 异常 | `execution_error` 优先级在重算面成立：`problems` 含 `execution_error` + 子报告覆盖全 pass + `failed = 0` → 重算 `Error`；summary 记 `Error` → 零违例；记 `Fail` → 违例 | 组合迭代（2 组合） |
| `check_integrity` | 异常 | ① 四计数对账失败（summary 记 3 / 子报告之和 2）与 ② plan 双向齐全性（ghost 条目 + 索引缺失）→ 既有用例与文案逐字保留 | 既有，回归登记 |
| `check_integrity` | 边界 | 零子报告零 plans：空 summary → 空 findings；同空集篡改 `conclusion = Fail` → 违例且不 panic | 组合迭代（2 组合） |
| `check_integrity` | 边界 | 超大列表：50 条子报告 + 1 条 `coverage = None` + summary `conclusion = Fail` → 零违例（重算对 50 条遍历不误报、零性能断言） | 单次 |
| `diagnose_findings` | 正向 | 4c 逐 suite 展开（AC-4 / D5）：`problems` 含 `{framework: "rust", type: coverage_failure, message: "Coverage below threshold: functions 87.6% < 90%"}` → findings 含逐字 `覆盖判定未通过 [rust]: Coverage below threshold: functions 87.6% < 90%` | 单次 |
| `diagnose_findings` | 正向 | 4e 正向单条（逐字保留）：`conclusion = Pass` → findings 恰 `["全部测试通过且覆盖率达阈值"]`，且零「失败 / 错误 / 未达」负向行 | 单次 |
| `diagnose_findings` | 正向 | 4a / 4b 既有措辞零改动：Error + `execution_error` 归因行、Error 但 `problems` 无 `execution_error` 的漂移行、单点（1 项）与聚类（3 项）失败措辞 | 既有，回归登记 |
| `diagnose_findings` | 异常 | 4c 零假缺口：`conclusion = Fail` + `problems` 无 `coverage_failure` 条目 → 零 4c 行（不再由 `summary.coverage` 取值重算数值） | 单次 |
| `diagnose_findings` | 异常 | 4c 条数上限：12 条 `coverage_failure`（跨多 framework）→ findings 中 4c 行恰 10 条（`CLUSTER_FRAMEWORK_LIMIT`） | 单次 |
| `diagnose_findings` | 边界 | 并存互不顶替（R-10）：`problems = [execution_error, coverage_failure]` + `conclusion = Error` → 4a 归因行与 4c 行同现，且 4a 的「另有 N 项测试 / 覆盖问题随报告留档」计数含该覆盖条目 | 单次 |
| `diagnose_findings` | 边界 | 空输入：`problems = []` / `failed = 0` / `conclusion = Pass` → 仅 4e 单条；`problems = []` / `failed = 0` / `conclusion = Fail` → 零 findings（无块可读时的既有空集语义） | 组合迭代（2 组合） |
| 退役面（`threshold_findings` / `threshold_brief` / 4d「全部框架未产出覆盖数据」/ 4f 突变缺口） | ↔ 编译闭合 | 删两条旧 4c 用例；`hand_summary` 去 `coverage` 形参与两字段；`consistent_summary` 去 `&suites`；成孤的 `suite()` / `config_thresholds()` 与 `CoverageMeasured` / `CoverageThresholds` 导入按未用告警清理——MUST NOT 新增用例 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock（纯函数层） | 无 IO、无 spawn；`core/checks` 零进程边界红线即被测契约 | 全章节 |
| 不 mock：`build_summary_report` | 真实组装「一致 summary」——③ 的判据是「重算与记载一致」，手搓 summary 会把被测结论当输入 | `check_integrity` 全部行 |
| 不 mock：`determine_conclusion` | 真实同源调用（R-12：门禁口径与复用门口径 MUST NOT 分叉；替身化即失去该判据力） | `check_integrity` 全部行 |
| 不 mock：`derive_plan_id` | 真实（② plan 索引面的 planId 单点） | ② 回归登记行 |
| 手工 `hand_summary` 字面构造 | 非 mock：诊断树分支的定向输入面（仅给「某个分支该产出什么」的确定性输入，不经聚合） | `diagnose_findings` 全部行 |

### `packages/desktop/src-tauri/crates/core/checks/src/model.rs` -> `packages/desktop/src-tauri/crates/core/checks/src/model_test.rs`

**既有 sidecar 改造**：`SummaryReport` 字面量构造与字段序断言去掉 `coverage` / `mutation` 两字段；`FULL_SUMMARY_JSON` 与「最小合法 summary」的输入 JSON 文本**零改**（它们正好充当 AC-13 的旧报告样本）；新增「序列化面无两键」「旧报告带两键仍可解析」「旧报告两键畸形仍宽容」三组。`SubReport` / 各覆盖块与突变块的解析保真用例零改动（回归登记）。

#### 待测功能

- `SummaryReport`（修改）: 删 `coverage: Option<CoverageBlock>` 与 `mutation: Option<MutationBlock>` 两字段（含 doc）；余 12 字段与声明序不变（`phase` / `command` / `timestamp` / `duration_seconds` / 四计数 / `conclusion` / `problems` / `plans` / `findings`）；序列化面自此无两键，旧文件仍可宽容解析（`parse_summary_report` 无 `deny_unknown_fields`）。
- `Problem` / `MutationBlock` / `SourceFileEntry` / `CoverageOverride`（仅注释）: `framework` doc 改「覆盖问题为该 suite 的框架」、`MutationBlock` doc 改「解析保真 + 载荷展示口径」、`SourceFileEntry` doc 改「报告保真面」、`CoverageOverride` doc 补「仅解析保真面」——**字段与 serde 零变**。
- `SubReport` / `CoverageBlock` / `CoverageMeasured` / `CoverageThresholds` / `MutationMeasured` / `MutationOverride` / `PlanIndexEntry` / `Conclusion` / `ProblemType` / `CaseSummary` / `TestCaseResult` / `FileCoverageEntry`（零改动）: 解析保真面与旧报告宽容解析的载体。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SummaryReport` | 正向 | 序列化瘦身（AC-2）：构造 summary → `serde_json::to_value` → 顶层键集 = 11 个必在键（`phase` / `command` / `timestamp` / `duration_seconds` / 四计数 / `conclusion` / `problems` / `plans`）+ `findings` 在 `None` 时**整个键不出现**（`skip_serializing_if`）；`has("coverage") == false` 且 `has("mutation") == false` | 单次 |
| `SummaryReport` | 正向 | 往返无损：含三类 `problems` + 一条 `plans` + `findings = Some([...])` 的 summary → serialize → `parse_summary_report` → 逐字段相等（`PartialEq`） | 单次 |
| `SummaryReport` | 正向 | 旧报告仍可读（AC-13）：`FULL_SUMMARY_JSON` 原文（含 `coverage` 块与 `"mutation": null`）→ `parse_summary_report` 成功，`problems` 两类逐字保真、`plans` / `findings` 保真，两键被**忽略**（零残留、零 `Err`） | 单次 |
| `SummaryReport` | 异常 | 旧报告两键畸形承压（AC-13 的宽容边界）：`"coverage": "oops"` / `"coverage": []` / `"mutation": 42` / `"mutation": {}` → 仍 `Ok`（未知字段宽容不因旧键形态翻面） | 参数化迭代（4 形态） |
| `SummaryReport` | 异常 | 非法枚举 / 必填缺失（既有用例）：`conclusion = "green"` / `"PASS"` / `""` / `42` → `Err`，文案前缀 `汇总报告解析失败:`；`problems[0].type = "coverage"`（`ProblemType` 三值外）→ `Err`；`phase` / 四计数 / `conclusion` 任一缺失 → `Err` | 既有，回归登记 |
| `SummaryReport` | 边界 | 最小合法 summary（无 `problems` / `plans` / `findings`、无两键）→ 解析成功、`total == 0`、序列化文本无两键 → 再出线重解析往返无损 | 单次 |
| `SummaryReport` | 边界 | 极值字段：`duration_seconds = 0` 与 `total = u64::MAX`（四计数大值）两形态 → 解析与往返保真、零饱和 | 参数化迭代（2 形态） |
| `SummaryReport` | 边界 | 新形态反序列化的缺省臂：缺 `problems` / 缺 `plans` / 缺 `findings` 的 JSON → `Vec::new()` / `Vec::new()` / `None`（`#[serde(default)]` 链完好） | 参数化迭代（3 形态） |
| `SubReport` 面零改动 | 正向 | 真实形态子报告（含 `coverage` 块 + `source_files` + `findings`）→ 逐字段保真；`coverage` / `mutation` / `findings` 缺省 → `None` 往返无损 | 既有，回归登记 |
| `SubReport` 面零改动 | 异常 | 必填缺失 / 结构非法仍 `Err`：`framework` / `root` / `summary` 任一缺失、`error_cases[0].status` 非法枚举、`source_files[0].coverage` 结构非法 → `Err` 且文案前缀 `子报告解析失败:`（旧报告宽容**不倒**为「什么都能读」） | 既有，回归登记 |
| `SubReport` 面零改动 | 边界 | 三态同处置：`coverage` / `mutation` / `findings` 的各「缺键」与「显式 `null`」两写形态解析结果逐字段相等；`source_files = []` / `error_cases = []` → 空集往返无损 | 既有，回归登记 |
| `MutationBlock` 解析保真（注释改口径、字段零变） | 正向 | 带 `mutation` 块的子报告样本 → `score` / `threshold` / `pass` / 九计数 / `overrides` 逐字段保真（`mutation-block` 语料面，`score 55.0` / `threshold 70.0` / `measured [11,7,2,3,27,13,10]`） | 既有，回归登记 |
| `MutationBlock` 解析保真 | 异常 | 块内类型非法：`score` 为字符串 / `measured` 为数组 / `pass` 为 `"yes"` → `Err`（保真面不放宽结构校验） | 既有，回归登记 |
| `MutationBlock` 解析保真 | 边界 | `overrides: null` 与缺键同处置；`measured` 全零（零突变体）→ 九计数 `0` 保真（`normalize_detected` 语义面） | 既有，回归登记 |
| `Problem` / `SourceFileEntry` / `CoverageOverride`（注释仅改口径，字段零变） | 正向 | 三类型各自往返保真：`Problem` 序列化恰三键（`framework` / `type` / `message`）；`SourceFileEntry.coverage = None` 与 `Some(FileCoverageEntry)` 两形态；`CoverageOverride` 七字段（`glob` / 两阈值 / 测量 / `pass` / `file_count` / `passed_count`）逐字段保真 | 单次 |
| `Problem` / `SourceFileEntry` / `CoverageOverride`（注释仅改口径，字段零变） | 边界 | `CoverageOverride` 仅作解析保真面：把带 `overrides` 组的子报告样本解析后，`overrides` 数组逐项保真（新报告 MUST NOT 再生产该组，但读面零退化） | 单次 |
| 语料 golden 全链（AC-12，执行落 `crates/core/checks/tests/corpus_golden_test.rs`） | 正向 | 八份语料投影：四份含 summary 面者（`green-istanbul` / `green-llvm-cov` / `green-node-test` / `red-run`）的 `computedCoverage` 键整块消失，其余投影（`subReport.*` / `summary.conclusion` / `summary.counts` / `problemTypes` / `integrity` / `diagnose`）逐字不变；另四份（`mutation-block` / `unknown-fields` / `corrupt-invalid-json` / `corrupt-missing-required`）零 diff | 参数化迭代（8 形态） |
| 语料 golden 全链 | 正向 | 子报告面同义断言改指：`green-llvm-cov` 的 `subReport.coveragePresent == true` 承接原三条 `computedCoverage.*` 断言（`green-istanbul` / `green-node-test` / `red-run` 同形）；`red-run` 的 `problemTypes == ["test_failure","test_failure"]` 与 `diagnose` 聚类措辞逐字不变 | 参数化迭代（4 形态） |
| 语料 golden 全链 | 异常 | 反向判据：投影中 `["summary"]["outcome"] == "parsed"` 的六份语料的 `integrity` 仍为 `[]`（③ 重算不引入新违例）；两份 `corrupt-*` 仍 `subReport.outcome == "unparsable"` 且无 summary 面 | 参数化迭代（2 形态） |
| 语料 golden 装置 | 边界 | 清单与开关边界：`CORPUS_FIXTURES` 恰 8 份且「6 parsed + 2 unparsable」全量统计（零静默丢弃）；`DESKTOP_GOLDEN_REWRITE` 为 `0` / 空串 → 比对模式，非 0 → 写盘模式；golden 缺失时记因文案含 `DESKTOP_GOLDEN_REWRITE` | 参数化迭代（3 形态） |
| 语料 golden 装置退役 | ↔ 编译闭合 | 删 `use checks::aggregate::aggregate_coverage;`、`suites_for`（唯一消费者是已删投影段）、`config::{MutationConfig, TestFramework}` 导入、`computedCoverage` 投影段与 llvm-cov 三条 `computedCoverage.*` 断言——其余投影与断言零改 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock（值类型与解析层） | 纯 serde 往返 + 读盘语料；无 IO 替身需求（语料是真实 fixture 文件，非 mock） | 全章节 |
| 不 mock：`serde_json` | 真实序列化 / 反序列化——线格式键面与宽容解析是被测契约本体 | 全章节 |
| 不 mock：`parse_summary_report` / `parse_sub_report` | 被测本体（替身化即零判据） | 全章节 |
| 语料 fixture（`tests/fixtures/corpus/**`）+ golden（`tests/golden/**`） | 真实磁盘资产；`DESKTOP_GOLDEN_REWRITE` 是既有重写开关（非 mock 面） | golden 投影行 |

### `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs`

**既有 sidecar 改造**：`绿跑全链_pass结论_四计数与报告落盘` 的 `summary.coverage` 三条断言删除并改指子报告 `report.json` 的 `coverage.pass`；`hand_summary` 字面量去两字段；两处 `checks::build_summary_report(…, &[], …)` 去 `&[]` 实参。新增：缺采集链级 failsafe（`coverage == None ⇒ Fail + 一条 coverage_failure + findings_brief 携 4c`）与「复用门对覆盖缺失 fail 报告同待遇」两行。`testexec/report_test.rs` 仅字面量机械闭合。

#### 待测功能

- `ProcessTestExecution::run`（`TestExecutionRunner` 实现，→ 私有 `execute`）: 执行链入口——`build_summary_report(&sub_reports, DESKTOP_COMMAND_LABEL, started_at)` 去 `suites` 实参；`summary.json` 无 `coverage` / `mutation` 块；`problems` 携逐 suite 覆盖归因；`findings` 由新诊断树落地。
- 零改动面（`build_sub_report` / `suite_thresholds` / `coverage_brief` / `findings_detail` / `outcome` / `fault_outcome` / 复用门三条件 `plan_ids_match` + `check_integrity` 空 + `is_reusable`）: 签名与语义逐字不变。

<!-- design「公共函数 / API」表未声明本文件的公共 API 变更（该表无 testexec/mod.rs 行）；上述条目取自 design「修改文件」表该文件行（「`build_summary_report` 调用点收参」）与「架构组件」执行链装配行，行为经 `TestExecutionRunner::run` 公共入口与落盘报告观测。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProcessTestExecution::run` | 正向 | 绿跑全链（AC-2 / AC-5）：node-test suite 实测 `82.5 / 74.1 / 90.3` 达阈（阈 `60/70/75`）→ `summary.json` 解析后 `conclusion = Pass`、四计数 `(5,4,0,1)`、`problems` 空、`has("coverage") == false`、`has("mutation") == false`；子报告 `report.json` 的 `coverage.pass == true` 且 `measured.lines == Some(82.5)`（原 summary 面三条断言改指子报告面） | 单次 |
| `ProcessTestExecution::run` | 正向 | 掩盖不可达链级（AC-4）：双 suite 覆盖工件（vite 形态三维全达标 + rust 形态 `functions 87.60 < 90`）→ `summary.conclusion = Fail`、`outcome.conclusion = Fail`、`problems` 恰一条 `coverage_failure` 且 `framework == "rust"`；两子报告各自 `coverage.pass` 保真（真值源仍逐 suite） | 组合迭代（2 suite × 覆盖两形态） |
| `ProcessTestExecution::run` | 异常 | 缺采集链级（AC-5）：绿跑但覆盖工件缺席（shim 不落工件）→ `exit_code = 0` + 零失败 + 子报告 `coverage = None` → `summary.conclusion = Fail`、`outcome.conclusion = Fail`（MUST NOT `Error`）、`problems` 恰一条 `coverage_failure`（零 `execution_error`）、`outcome.findings_brief` 含 `覆盖判定未通过 [` | 单次 |
| `ProcessTestExecution::run` | 异常 | `Error` 成因面收窄：非零退出且工件不可解析 → 仍 `Error`（既有用例 `非零退出且工件不可解析_error结论产出` 与 `测试程序不可达_run_err` 逐字保留） | 既有，回归登记 |
| `ProcessTestExecution::run` | 异常 | 半套报告不进复用门：任一 plan 出 `fault`（`Stopped` / `Environment`）→ 不写 summary、不跑诊断（既有用例零改动；覆盖缺失属 `Fail` 不属 fault，两臂互不影响） | 既有，回归登记 |
| `ProcessTestExecution::run` | 边界 | 复用门三条件守恒（AC-9）：`plan_ids_match` + `check_integrity` 空 + `is_reusable` → 零 spawn 复用；预置「覆盖缺失的 fail summary」+ 输入未变 → 同样命中复用（fail 与既有 fail 同待遇，MUST NOT 因覆盖面拒复用） | 组合迭代（2 组合） |
| `ProcessTestExecution::run` | 边界 | 输入推进即重跑（AC-9）：suite 输入文件 mtime 推进 → 复用门失效、重跑产新 summary（既有用例 `复用门装配_输入失效重跑` 零改动） | 既有，回归登记 |
| `ProcessTestExecution::run` | 边界 | 单 / 双 plan 形态：单 plan（node-test）与双 plan（`two_plan`）各产 1 / 2 条子报告与 `plans[]` 条目；两形态 summary 面均无两键、`plans[].id` 沿 `derive_plan_id` | 参数化迭代（2 形态） |
| `ProcessTestExecution::run` | 边界 | 槽位分树不变（AC-16 行为侧证）：`ReportSlot::Baseline` 的 summary 落 baseline 树且与 test 树零混居（既有用例零改动；新报告形态在两树同形） | 既有，回归登记 |
| `findings_detail` / `coverage_brief`（零改动面） | 正向 | 装配面零改：`findings_detail` 的取数源仍是**子报告**覆盖块（`report.coverage.as_ref().map(coverage_brief)`），summary 无覆盖块后诊断明细逐行不变；`findings_detail装配口径_*` 与 `sub_report.findings_*` 既有用例零改动 | 既有，回归登记 |
| 装配闭合（`build_summary_report` 调用点 / `hand_summary` / `report_test` 字面量） | ↔ 编译闭合 | 生产调用点 1 处去 `suites` 实参；`mod_test.rs` 两处 `build_summary_report(…, &[], …)` 去 `&[]`；`hand_summary` 去两字段；删 `summary.coverage` 三条断言；`testexec/report_test.rs` 的 `summary()` 字面量删 `coverage: Some(CoverageBlock{…})` 与 `mutation: None`（`coverage` 三类型导入仍被子报告 fixture 消费） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 临时 workspace（`tempfile::TempDir` + 真实 `.dev-team/config.json` + suite 输入文件 + change 目录） | 真实磁盘布局——报告树推导、复用门 mtime 判据、`plans[]` 路径都是被测面 | 全章节 |
| 子进程边界：PATH shim 假执行体（`node` / `npx` 脚本或 `.cmd`）+ `SHIM_*` 环境变量 | 假执行体按 `SHIM_RESULTS_SRC` / `SHIM_DEST` 落预置工件、按 `SHIM_MARKER` 记 marker、按 `SHIM_EXIT` 退出——仅替 `spawn` 边界（无真实框架进程），argv / cwd / 次数断言读记录面 | 全链 / 缺采集 / 双 plan 行 |
| crate 级 env 锁（`lock_test_env`）+ `PathWindowGuard` | 进程全局 env / PATH 串行化装置（非 mock，不可绕过——并行用例会互相污染 PATH） | 全章节 |
| 不 mock：`build_summary_report` / `check_integrity` / `is_reusable` / `build_sub_report` / `diagnose_findings` | 真实组合（装配链与判定同源是判据本体；替身化会让 AC-4 / AC-5 / AC-9 三条自证空转） | 全章节 |
| 不 mock：`infra/exec` 记录链（`LiveExec`） | 真实（记录面降级由既有用例覆盖，本变更零触及） | 全章节 |

### `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs`

**既有 sidecar 改造**：七处「summary mutation 位回填 / 保持 null」断言**反转为**「summary **无** mutation 键 / 未被改写」；`merge_mutation_blocks` 调用、逐 suite `all_pass` 门禁与载荷 `score` / `threshold` / `counts` 断言**逐字保留**。判据升格：由「键值为 `null`」升为「执行前后 `summary.json` 字节逐字相等」（`null` 仍是预置值，读 `Value` 索引分不出「零回写」与「回写后又清空」，字节比对才证得住「回填面不存在」）。

#### 待测功能

- `ProcessMutation::run`（`MutationRunner` 实现）: 突变执行链——`merge_mutation_blocks(&blocks)` 归并块仅服务载荷展示（`MutationTestOutcome.score` / `threshold` / `counts` 与 `fail_feedback`），MUST NOT 落 `summary.json`、MUST NOT 作门禁判据；逐 suite `all_pass` 门禁判定零改动。
- 零改动面（`bucket_suites` / `all_pass` / `fail_feedback` / 段执行 / 复用门 / sandbox 清理 / `SegmentFailure` 三形态分流）: 逐字不变。

<!-- design「公共函数 / API」表未声明本文件的公共 API 变更（该表无 mutation/mod.rs 行）；上述条目取自 design「修改文件」表该文件行（「删 `backfill_summary` 与调用点；`merge_mutation_blocks` 与 `all_pass` 逐字保留」）与「架构组件」突变回填面（删）行，行为经 `MutationRunner::run` 载荷与落盘报告观测。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProcessMutation::run` | 正向 | **零回写（AC-10）**：rust suite 复用门 Pass 面 + 预置原样 `summary.json` → 载荷 `score == 90.0` / `threshold == 80.0` / `counts` 七字段 `(9,1,0,0,0,0,10)`；执行前后 `summary.json` **字节逐字相等**、`Value` 索引的 `mutation` 仍为预置值（未被归并块覆盖） | 单次 |
| `ProcessMutation::run` | 正向 | js suite 全链（Stryker 假 spawn）：载荷 `score == 90.0` / `threshold == 80.0` / 七计数 + `report_paths` 2 条（段级 + 归并报告）+ `feedback_text` 空；summary 面零回写（同字节判据） | 单次 |
| `ProcessMutation::run` | 异常 | 段中断面（`conclusion = Error`、双时点②即时反馈）：summary 零回写（字节不变）；`feedback_text` 的「突变执行中断」骨架与「已产报告路径」段逐字不变 | 单次 |
| `ProcessMutation::run` | 异常 | 取消面（`CancelFlag` 置位）：`fault = Some(RunFault::Stopped)`、`feedback_text` 清空、结论占位 `Fail`；summary 零回写（原「保持 null」断言反转为「未被改写」） | 单次 |
| `ProcessMutation::run` | 异常 | 逐 suite 门禁零改动：一 suite 未达阈（多阈值并存）→ `conclusion = Fail` + `fail_feedback` 逐 suite 明细（`- <planId>: score < threshold`）；归并 `score` 仅展示，结论 MUST NOT 由它推出 | 既有，回归登记 |
| `ProcessMutation::run` | 异常 | 段失败三形态分流零改动：`Fatal`（工具不可达 / 写盘失败）→ 显式 `Err`；`Interrupted`（harness 崩 / 未产段报告）→ `Error` 载荷；`Cancelled` → `fault` 正常返回 | 既有，回归登记 |
| `ProcessMutation::run` | 边界 | 跳过面（files 过滤后空桶）：`skip` 载荷 + summary 零回写（原断言反转） | 单次 |
| `ProcessMutation::run` | 边界 | summary 缺席（未预置）：`run` 仍 `Ok` 且不新建 `summary.json`（既有用例 `不在位不崩` 保留，判据由「null 观察期语义」改为「零写入面」） | 既有，回归登记 |
| `ProcessMutation::run` | 边界 | 归并块边界：单段等值透传 / 空段清单（`ok_or_else` 显式 `Err("突变段归并空…")`）/ 超大计数求和零溢出（`saturating_sub` 分母面零改动） | 参数化迭代（3 形态） |
| `ProcessMutation::run` | 边界 | 多 suite 多阈值（`TWO_RUST_CONFIG` 形态）：归并阈值取首块、逐 suite `all_pass` 独立判定；载荷 `threshold` = 首块阈值；summary 零回写 | 单次 |
| 退役面（`backfill_summary` 定义 + 调用点） | ↔ 编译闭合 | 删 `fn backfill_summary(reports_dir, block)` 定义（`mod.rs:770`）+ 调用点（`:362`）+ 两条 doc/注释；`merged` 绑定处补「载荷展示口径，MUST NOT 落盘 / 不作门禁判据」注释；七处断言由「回填 / 保持 null」反转为「无 mutation 键 / 未被改写」——载荷三字段与逐 suite 用例文本零改 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 临时 workspace（`ChainWs`：真实 `.dev-team/config.json` + 报告树 + 可选预置 `summary.json`） | 真实磁盘布局与真实 config 解析（`bucket_suites` 的 excludes 归桶、复用门 mtime 均在被测面内） | 全章节 |
| 子进程边界：`SpawnWindow`（PATH 前置假 `cargo` / `npx` + `FAKE_SPAWN_LOG` + 预置段报告落位 + canned 工件） | 仅替突变工具进程边界；argv / cwd / spawn 次数读记录面断言 | 执行链三行（Pass / 中断 / 取消） |
| crate 级 env 锁（`crate::lock_test_env()`，`SpawnWindow` 字段序末位持锁） | 进程全局 env / PATH 串行化装置（非 mock） | 全章节 |
| 不 mock：`merge_mutation_blocks` / `build_mutation_block` / `all_pass` / `fail_feedback` / `bucket_suites` | 真实（载荷三字段与逐 suite 判定的唯一来源；替身化会让 AC-10 / AC-11 自证空转） | 全章节 |
| 不 mock：`serde_json::Value` 索引 + 文件字节读盘 | 真实读盘（「零回写」只能由字节比对观测；`Value` 判据单独不足） | AC-10 三行 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` 一行） | 元数据面，无运行期行为可断言；判据是**相对递增**（落地时 `HEAD` 现值 +1 patch，MUST NOT 写死字面号——design 提 `0.6.38 → 0.6.39` 但禁实现写死） | 桌面端 diff 审查：`git diff -- packages/desktop/package.json` 的 `version` 行恰 +1 patch；`src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本行未动（AC-15） |
| `packages/desktop/src-tauri/crates/core/checks/src/lib.rs`（再导出收窄） | 纯可见性面：crate 内 sidecar 一律走 `crate::aggregate::…` / `crate::diagnose::…` 模块路径，不经 crate 根；「未再导出」是编译期事实，无运行期载体 | 编译闭合（引用已删符号即 `E0432`）+ `grep -rn "aggregate_coverage\|aggregate_measured\|in_suite_scope" packages/desktop/src-tauri/crates/` 零命中 + `pnpm -C packages/desktop run api:check` 零 diff（`.dev-team/architecture/api/INDEX.toml` 中 `App.Backend.Core.Checks` / `App.Backend.Infra.ChecksRuntime` 均 `status = "absent"`，删 `pub` 项不产生 finding） |
| 删除面符号的**零残留**（AC-1 的 11 个加权链符号 + `backfill_summary` / `threshold_findings` / `threshold_brief` + `lib.rs` 三条再导出 + `build_summary_report` 的 `suites` 形参：`aggregate_coverage` / `weighted_measured` / `compute_overrides` / `aggregate_measured` / `file_meets_thresholds` / `to_thresholds` / `in_suite_scope` / `under_root` / `default_glob` / `DEFAULT_THRESHOLDS` / `aggregate_mutation`） | 「不存在」是**缺席事实**：运行期只能覆盖被驱动的路径，覆盖不到「某分支 / 某函数已不在」。跨文件 / 跨 crate 零命中须 `grep` | `cargo clippy --workspace --all-targets -- -D warnings` exit 0（死代码 / 未用项即红）+ `grep -rn "<符号名>" packages/desktop/src-tauri/crates/`（仅允许命中 `*_test.rs` 的删除登记与 `openspec/`）+ 编译期（`lib.rs` 仍 `pub use` 已删符号必 `E0432`） |
| 注释面四改（`Problem.framework` / `MutationBlock` / `SourceFileEntry` / `CoverageOverride` 的 doc，design 事实校准 6 / R-11） | 注释文本零运行期路径 | 行为侧证：`aggregate_test` 的「`coverage_failure.framework` 恒为具体 suite 框架、零 `"all"`」行 + `model_test` 的 `MutationBlock` 解析保真行；文本面由桌面端 diff 审查 |
| AC-12 的 **golden 重写纪律**（4 份重写 / 4 份零 diff / 逐份人工 review） | 产物 diff 事实与 review 纪律，运行期无法断言「某文件只少了某个键」 | `DESKTOP_GOLDEN_REWRITE=1` 显式重写后 `git diff -- packages/desktop/src-tauri/crates/core/checks/tests/golden/` 逐份 review（预期 diff = **仅 `computedCoverage` 键消失**；`mutation-block` / `unknown-fields` / 两份 `corrupt-*` 零 diff）；`grep -n "aggregate_coverage" crates/core/checks/tests/corpus_golden_test.rs` 零命中 |
| `crates/core/checks/src/reuse_test.rs`（`summary()` 字面量删 `coverage: None` / `mutation: None` 两行；AC-9 的「零 diff」张力） | 机械编译闭合（字段删除后字面量非法），断言与 `is_reusable` 语义零改 | 编译闭合（缺字段即 `E0063`）+ `git diff -- crates/core/checks/src/reuse_test.rs` 恰两行删除、断言零改；AC-9 的三条件守恒由 `testexec/mod_test.rs` 复用门两行承载 |
| AC-6 的「6 份历史形态复算全 pass」（真跑复算） | 需真实历史报告 + 真实框架执行，属桌面端 test-execution 面；单测只能钉形态 | 单测形态复现（`coverage_meets_thresholds` 的 `branches: None` 行 + `determine_conclusion` 的 `block.pass = true` / `false` 两行）；真跑复算结论由桌面端 test-execution 相位登记 |
| `runner.rs::parse_coverage_artifact` 的「`(Vec::new(), None)` 静默面」（design D9② 明令不改） | 生产面**恒产出** `Some` / `None` 两形态的概率由注册表模板决定；「采集旗标恒开」是配置事实，不是运行期可断言路径 | `git diff -- crates/infra/checks/src/testexec/runner.rs` 为空；行为侧证 = 缺采集链级行（shim 不落工件）证明 `None` 走 `Fail` 而非静默通过 |
| AC-14 / AC-16 的**零 diff 主张**（`crates/core/config/**`、`crates/core/orchestration/**`、`crates/core/workflow/**`、`crates/infra/exec/**`、`testexec/report.rs`、`packages/desktop/src/**`、`config.schema.json`、`bindings.ts` / `bindings.zod.ts`、`.dev-team/architecture/**`、`explore.md`、`reports/baseline/**`） | 均为「一行不变」的结构性主张，运行期不可断言 | 桌面端 `git diff --stat` 逐项审查（设计范围外零改动）+ `pnpm run bindings:export` 后 `git diff --stat -- src/types/generated` 为空 + `api:check` 零 diff；行为侧证 = 各章节「既有，回归登记」行全绿 |
| 机械门禁（`pnpm -C packages/desktop run check` 四段链 / `cargo nextest run --workspace` / `vp check` / `knip`） | 机械步骤，由桌面端 static-check / test-execution 相位执行；在设计里复述脚本细节等于复制运行脚本 | 门禁执行结果 + 本设计各章节的判据支撑（断言全部落在真实契约面：布尔归并 / 逐 suite 归因 / ③ 重算 / 零回写 / 旧报告宽容，不为覆盖率造句） |
