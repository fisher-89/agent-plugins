# 提案: coverage-aggregation-layer-removal

> **变更**: coverage-aggregation-layer-removal
> **日期**: 2026-11-12
> **状态**: draft
> **来源**: `openspec/changes/coverage-aggregation-layer-removal/explore.md`（requirement 类型；含 §9 启动条件复核、§10 两项裁定落地）
> **性质**: test-execution 汇总层的**覆盖率加权聚合面整体退役**——`summary.coverage` / `summary.mutation` 两块不再产出，判定点唯一化到「逐 suite 子报告」，汇总层只做布尔归并与计数求和。零 config schema 变更、零 IPC / 前端影响、零新能力；一份行为语义收紧（覆盖率采集缺席由「静默跳过」改为「判 fail」）。

---

## 问题

### 现状链路（读过的真实代码）

```
crates/infra/checks/src/testexec/mod.rs
  ├─ build_sub_report(plan, execution, suites)            :541  ← 子报告层：coverage.pass 已按该 suite 阈值判定
  └─ build_summary_report(sub_reports, suites, …)         :252
       └─ checks::aggregate::build_summary_report          aggregate.rs:611
            ├─ collect_problems(sub_reports)               :453  失败用例 / execution_error / 覆盖缺测归因
            ├─ aggregate_coverage(sub_reports, suites)     :108  ← 汇总层：加权成一个全局块
            │    ├─ weighted_measured                      :240  多 suite 自全部逐文件条目加权
            │    ├─ compute_overrides(sub_reports, suites) :139  第二副本：per-suite override 组
            │    │    ├─ in_suite_scope                    :394  root ∧ excludes ∧ includes(吃 default_glob(:415))
            │    │    ├─ aggregate_measured                :201  逐文件计数加权
            │    │    ├─ file_meets_thresholds             :177
            │    │    └─ to_thresholds                     :260
            │    └─ coverage_meets_thresholds              :75
            ├─ aggregate_mutation(sub_reports)             :600  突变计数求和 → 回填 summary.mutation
            └─ determine_conclusion(failed, coverage, mutation, has_execution_error)   :56
crates/infra/checks/src/mutation/mod.rs
  └─ backfill_summary(reports_dir, merged)                 :770  ← 执行后二次写盘 summary.json 塞 mutation 块
```

一次 test-execution 的事实被**判两次、量三遍**：

| 口径 | 落点 | 谁在判定 |
|---|---|---|
| 子报告 `coverage` 块（per-suite） | `reports/test/<planId>/report.json` | `coverage_meets_thresholds`（该 suite 阈值） |
| 条目加权 / override 组（summary） | `summary.json.coverage` | `aggregate_coverage`（首 suite 阈值 + 逐文件加权） |
| 汇总整体结论 | `summary.json.conclusion` | `determine_conclusion`（**只看汇总块，不看子报告**） |

explore §1 实测同一份 rust suite 的「行覆盖率」在三个口径下是 95.73 / 95.94 / 95.92——三个数都不是错的，但它们量的不是同一个东西。

### 为什么加权在语义上不成立（三条独立理由）

1. **量的对象不同**：vite-plus 量 `src/**`（排 `components/ui/**` / `routes.tsx` / `types/generated/**`、独立 `.test.tsx` 文件）；rust 量 `crates/**/src/**`（无排除、**行内 `*_test.rs` 计入分母**）。实测 rust 的 334 条里 161 条是测试文件，占行数 84%——它的 95.94% 天然虚高约 8 个百分点（只算生产码为 87.60%）。把两者加权平均，等于把两种度量混成一个百分比。
2. **加权的净效果是掩盖**：全库 90 份报告实测 **26 份**出现「子报告判非 pass、汇总判 pass、`conclusion=pass`」；失守维度 `branches` 25 次 / `functions` 19 次 / `lines` **0** 次——漏斗专挑 branches / functions，因为某一维不达标会被另一个 suite 的小数点稀释。
3. **系统自己早发现，门禁永不放行**：4 份报告（`2026-09-28-agent-component-ai-sdk` / `2026-09-30-desktop-agent-management` / `2026-10-01-agent-core-session-kernel` / `2026-10-03-agent-turn-eof`）的 agent `findings` 里**逐条写下了这个缺陷的诊断**，而 `determine_conclusion` 不读 findings，四次全判 pass。

### 加权的两个可能理由都不成立

- **展示**：产品面需要一个「总体覆盖率」——用户已明确裁定**不需要覆盖率数字**；
- **门禁**：总体门槛的正确形态是「每个 suite 各自达标」，不是「平均达标」（平均达标允许一个 100% 掩盖一个 0%）。

### 一个具体的可达缺口（不是理论风险）

`runner.rs::parse_coverage_artifact`（:501）读不到覆盖工件即返回 `(Vec::new(), None)`，**不进 `error` 也不进 `notes`**——覆盖采集失败在子报告上完全无声；`misses_coverage` 只对「非 pass 且 `coverage == None`」发力，于是「绿跑但无覆盖工件」（如 jest 未开 coverage、工件写坏）会掉进 `weighted_measured` 取**别的** suite 的数值 → `pass=true`。explore 前篇实测此类假绿 51 条，现网仍在。

---

## 提案

### 一、判定点下沉：逐 suite 独立判定（唯一判定点）

子报告层的 `coverage.pass` 已是逐 suite 口径（`build_sub_report` :541 用 `coverage_meets_thresholds` + 该 suite 阈值），本变更把它确立为**唯一判定点**，并把「三态」写死为一等公民：

| 情形 | 判定 |
|---|---|
| 该维度有值且达阈 | pass |
| 该维度有值未达阈 | **fail** |
| 该维度声明不支持（`llvm-cov` `count==0 → None`、node-test `-` → `None`） | 跳过（工具自述不支持，是配置事实） |
| suite 应报未报（`coverage == None`：工件缺失 / 解析失败） | **fail**（不可测 ≠ 达标） |

最后一行是本变更唯一的行为收紧。它可行的根据是**「采不采覆盖率」在本仓不是变量而是常量**：五框架注册表模板全部恒带采集旗标（jest / vitest / vite-plus 的 `--coverage …--coverageReporters=json-summary`、rust 的 `--output-path`、node-test 的 `--experimental-test-coverage`），`detect.rs` 的 `coverage_format: CoverageFormat` **非 Option**、`coverage_output` 恒有值，`config::TestSuite` 里也没有任何 coverage 开关。故 `coverage == None` **只可能**是采集失败，不可能是「本就不采」。

首跑变红风险实测为 0：最近 6 份自产报告的 rust / vite-plus 子报告 `measured` 全部非 None（explore §10.1 表）。

### 二、汇总层只做布尔归并与计数求和

`SummaryReport` **删 `coverage` / `mutation` 两字段**，聚合链上的加权 / 归因 / 归并函数整链退役（精确到函数）：

| 位置 | 动作 |
|---|---|
| `aggregate.rs::aggregate_coverage`（:108） | **删** |
| `aggregate.rs::weighted_measured`（:240） | **删** |
| `aggregate.rs::compute_overrides`（:139） | **删** |
| `aggregate.rs::aggregate_measured`（:201） | **删** |
| `aggregate.rs::file_meets_thresholds`（:177） | **删** |
| `aggregate.rs::to_thresholds`（:260） | **删** |
| `aggregate.rs::in_suite_scope`（:394）+ 其专属 `under_root` + `default_glob`（:415）镜像 | **删** |
| `aggregate.rs::DEFAULT_THRESHOLDS`（:21） | **删**（无 `suite` 时不再需要全局阈值兜底） |
| `aggregate.rs::aggregate_mutation`（:600） | **删** |
| `aggregate.rs::misses_coverage`（:439） | 改：判据退化为「`coverage == None`」，归因面改 `CoverageFailure` |
| `core/checks/src/lib.rs` 再导出 | 收窄（去掉上述被删函数） |
| `build_summary_report` 签名 | `suites: &[TestSuite]` 参数退役（唯一消费者是 `aggregate_coverage`）；调用点 `testexec/mod.rs:252` 同步 |

**保留**（MUST NOT 顺手删）：`is_file_excluded` 及其 `normalize_root` / `posix_normalize` / `posix_join` / `relative_to_root` / `to_forward_slash` / `is_absolute_posix` 助手（`mutation/mod.rs:435` 归桶过滤的消费者）；`coverage_meets_thresholds`（子报告判定）；`CoverageBlock` / `CoverageMeasured` / `CoverageOverride` / `MutationBlock` / `MutationMeasured` 模型（子报告解析保真 + 旧报告宽容解析）；`merge_mutation_blocks` / `build_mutation_block`（见五）。

**删 `in_suite_scope` 同时消掉前篇 §4.6 的 `default_glob` 两份镜像**：grep 实测 `aggregate.rs::default_glob` 的唯一消费者就是 `in_suite_scope`，权威注册表在 `detect.rs` 不动。故前篇 13 条遗留是 **13/13 消失**（explore §9.1 已订正 §4.2 的 11/13）。

### 三、结论判定只读布尔（`determine_conclusion` 不再读任何覆盖率数字）

```
conclusion =
    has_execution_error                              → Error   （成因面不变：工件解析 / harness 崩 / 框架探测失败）
  : failed > 0                                       → Fail
  : 任一 suite 覆盖判定非通过（含 coverage == None） → Fail
  : Pass
```

- **突变不参与汇总结论**：`SummaryReport` 无 `mutation` 位后，突变结论唯一载体是 `MutationTest` 步载荷与机械 checklist 第三槽（见五）。
- **计数仍是求和**：`total / passed / failed / skipped` 照旧求和（仅供人读），MUST NOT 由计数推导覆盖率。
- **`check_integrity` ③ 改为以子报告布尔重算**：`check_integrity` 已收 `sub_reports` 入参，③ 由「拿 `summary.coverage` / `summary.mutation` 自比」改为「以子报告布尔 + `problems` 的 execution_error + `summary.failed` 重算结论」——比现状更强（现状自己跟自己比，删掉块就等于自己跟自己比空集）。① 四计数对账、② plan 齐全性**零改动**（与覆盖率无关，explore §9.3-① 事实订正）。复用门三条件（`plan_ids_match` + `integrity` 空 + `is_reusable`）语义全保留。
- **`is_reusable` 零改动**：仍只看 `conclusion != Error` 与 `timestamp` 新鲜度。

### 四、归因与诊断改为逐 suite（报告自描述度取方向 a）

删掉 `summary.coverage` 后，`summary.json` 单看必须仍能回答「哪个 suite 覆盖未达」。取**方向 a：`problems` 逐 suite 归并**（零 schema 扩展）：

- 每 suite 覆盖判定非通过 → 恰一条 `Problem { problem_type: CoverageFailure, framework: <该 suite 框架>, message }`：
  - 有实测未达阈 → 逐维度缺口措辞（沿既有 `format_coverage_failure`）；
  - 无覆盖数据 → 采集缺失措辞（**改投 `CoverageFailure`，不再是 `ExecutionError`**——这正是 explore §10.1 的「Error 降为 Fail」，`Error` 的成因面收窄为解析 / harness / 探测失败）。
- 既有 `collect_problems` 的失败用例 / `execution_error` 归并**零改动**；覆盖缺测归因与该 suite 已有 `execution_error` 的**去重规则不再需要**（两类问题不再互相顶替：入口条件从「非 pass 且 None」变成无条件 `None`）。
- `diagnose_findings`：
  - 4c 由「读 `summary.coverage` 的 measured / overrides」改为「读 `problems` 中 `CoverageFailure` 条目逐条展开（归属面已有）」，**签名不变**（仍只吃 `summary`）；
  - 4d（pass 态「全部框架未产出覆盖数据」）**删**——缺席即 fail，pass 态不可能无覆盖；
  - 4e 正向单条措辞**逐字保留**（「全部测试通过且覆盖率达阈值」）；
  - 4f（突变缺口）**删**——`summary.mutation` 不复存在。
- 方向 b（`plans[]` 增 per-plan 结论字段）**被拒**：schema 加宽、`check_integrity` ② 的 plan 索引面要连带扩、前端将来一旦出线还要背一层冗余口径。留为「若将来确有 per-plan 结论展示需求」的独立变更。

### 五、`summary.mutation` 不写（归并块留内存，只服务载荷）

门禁**本来就是逐 suite 判定**（`mutation/mod.rs:365` `all_pass = suite_results.iter().all(|(_, block)| block.pass)`），不存在「加权掩盖门禁」；归并块只做三件事：回填 `summary.mutation`、喂 `diagnose` 4f、作 `MutationTestOutcome.score/threshold/counts` 展示。故：

| 位置 | 动作 |
|---|---|
| `model.rs::SummaryReport.mutation` | 删字段 |
| `aggregate.rs::aggregate_mutation`（:600） | 删 |
| `build_summary_report` 的 mutation fail 问题条目（:634-645） | 删 |
| `infra/checks/src/mutation/mod.rs::backfill_summary`（:770）+ 调用点（:362） | 删 |
| `diagnose.rs` ③ 的 mutation 入参 / 4f 分支（:176） | 删 |

**保留**：`merge_mutation_blocks` 与 `build_mutation_block`（`MutationTestOutcome.score/threshold/counts` 仍要它算，`port.rs` 载荷字段零改动）。归并 score 的定位写进 spec：**展示口径，MUST NOT 作门禁判据**（门禁 = 逐 suite `pass` 布尔，本变更零改动）。「改呈最差 suite 的 score/threshold」属新口径，本轮不取。

### 六、兼容面与 golden 重写

- **宽容解析不倒**：`parse_summary_report` 无 `deny_unknown_fields`，旧报告里的 `coverage` / `mutation` 键被忽略——历史归档报告仍可读。
- **corpus golden 不是零 diff**：8 份语料里 4 份含 summary 面（`green-istanbul` / `green-llvm-cov` / `green-node-test` / `red-run`）→ 投影里的 `computedCoverage` 键整块消失；`mutation-block` / `unknown-fields` / 两份 `corrupt-*` 零 diff。`corpus_golden_test.rs` 的 `aggregate_coverage` 导入 + 投影段（:154-161）+ 3 条 `computedCoverage.*` 断言（:257-261）一并退役。金样重写仍走**显式重写 + 人工 review diff**（`DESKTOP_GOLDEN_REWRITE=1`）。
- **生产报告基线重跑**：本 change 目录下 `reports/baseline/**` 是工作流自产产物，由门禁重跑覆盖，MUST NOT 手改。

### 决策表

| # | 决策 | 理由 | 被拒备选（为何拒） |
|---|---|---|---|
| D1 | 取消汇总层加权，判定点唯一化到子报告 | 加权语义不成立（三口径并存 + 度量对象不同）；产品已裁定不需要数字 | 前篇路径一（修 override 面）：只治第二副本，第一副本仍是问题；前篇路径二（退役 override）：`measured` 仍在加权 |
| D2 | 覆盖率采集缺席判 **fail**（不判跳过） | 采集旗标是注册表常量 ⇒ `None` 只可能是失败；不可测 ≠ 达标 | 判跳过（= 现网口径，51 条假绿继续无人判）；新增 config opt-out（今天无需求，MUST NOT 预埋） |
| D3 | 不新增任何 config 键 | 声明载体已是注册表模板；`TestSuite` 三元阈值非 Option | `unsupported_dimensions: [...]` / `coverage.enabled`（都是为不存在的需求开面） |
| D4 | 维度级「不支持」继续跳过（`measured` 内某维 `None`） | 工具自述（llvm-cov `count==0`、node-test `-`）是配置事实；rust `branches` 恒 `None` 属此类 | 要求 rust 启用分支插桩（撞 `desktop-test-execution` 硬条款「覆盖率口径与阈值 SHALL 零调整」，另一个变更的量级） |
| D5 | `summary.mutation` 不写，归并块只做载荷展示 | 门禁已逐 suite；持久化一个加权 score 只会再造一个可误读的数 | 保留回填（再造一个「全局分」）；逐 suite 写进 summary（schema 扩面最大） |
| D6 | 报告自描述取 `problems` 逐 suite（方向 a） | 零 schema 扩展；`problems.framework` 本就承载 suite 身份；`diagnose` 签名不变 | `plans[]` 增 per-plan 结论（schema 加宽 + 索引面连带） |
| D7 | `check_integrity` ③ 以子报告布尔重算（不删） | 复用门的「齐整」前提要继续成立；重算比自比更强 | 删 ③（复用门失去结论一致性守卫）；重造一套齐整判据（重复发明） |

---

## 能力

### 新增能力

无。本变更是**既有能力的语义收敛与退役**，不引入新 capability。

### 修改的能力

口径：修改 **3** 个既有能力，与 `specs/` 下三个 delta 目录一一对应。

- **desktop-checks-domain**（`MODIFIED` × 2 + `ADDED` × 2）
  - `MODIFIED`「core/checks 纯层零进程边界」：纯层职责表述由「阈值与 override 聚合判定」改为「逐 suite 阈值判定 + 汇总布尔归并（conclusion 判定）」，其余（解析器 / 完整性校验 / 诊断树 / 复用门 / 零 spawn 红线）**逐字保留**。
  - `MODIFIED`「覆盖率采集完整性（缺测不假通过）」：**header 逐字保留**（该名恰是新语义——缺测不得假通过），正文与 Scenario 整体重写为逐 suite 判定形态：四形态判定表（有值达阈 / 有值未达 / 维度 `None` 跳过 / `coverage == None` 判非通过）、汇总层 MUST NOT 加权 / 摊平 / 产 `overrides[]`、逐 suite 归因（`CoverageFailure` + `framework` = suite 框架）与执行面归因互不顶替。原四条 Scenario 替换为新语义的五条。
  - `ADDED`「汇总结论布尔归并与报告瘦身」：`SummaryReport` 删 `coverage` / `mutation`；`determine_conclusion` 只读布尔；**突变不参与汇总结论**；`check_integrity` ③ 以子报告布尔重算（①② 零改动）；`is_reusable` 零改动；report 自描述由 `problems` 承载（`plans[]` 不扩）。
  - `ADDED`「聚合链退役清单与解析保真面保留」：被删函数清单（MUST 全删、MUST NOT 留半截）与**保留清单**（`is_file_excluded` 族 / `coverage_meets_thresholds` / 各块模型 / `merge_mutation_blocks`）互为边界；corpus golden 显式重写纪律。
- **desktop-test-execution**（`MODIFIED` × 6）
  - `MODIFIED`「执行链与报告落盘」：聚合步由「阈值 / override → conclusion」改「逐 suite 布尔归并 → conclusion」；删「summary 的 `mutation` 位由 MutationTest 归并块回填」句与 Scenario 对应表述。
  - `MODIFIED`「V1 范围与边界留痕」：第 1 条由「`mutation: null` 保持合法」改为「summary 无 mutation 位；突变结论由 MutationTest 载荷与机械 checklist 第三槽承载，观察期语义不变」。
  - `MODIFIED`「机械 checklist 双槽与 evidence 三态」：Scenario「null 合法不判 fail」的「summary mutation 位为 null」改为「MutationTest 未执行 / 未跑」，三态 evidence 语义**逐字保留**。
  - `MODIFIED`「双实现对拍黄金语料」：补 golden 重写范围（哪 4 份、哪 3 条断言）与「显式重写 + review diff」纪律不变。
  - `MODIFIED`「rust suite 单构建（一条命令同产结果与覆盖率）」：末条 Scenario 的「覆盖维度按 null 感知跳过」改为「该 suite 覆盖判定未通过（工件缺席即不可测）」，结论定型文案不变。
  - `MODIFIED`「baseline 报告槽位与树隔离」：仅改一处表述——「summary 的 `mutation` 位不适用」改为「summary 无 `mutation` 位（run 内相位报告同形）」，其余树隔离 / 复用门 / 回查条款逐字保留。
- **desktop-mutation-testing**（`MODIFIED` × 1）
  - `MODIFIED`「score 口径与逐 suite 阈值判定」：删「归并 MutationBlock SHALL 回填 `summary.json` 的 `mutation` 位」句，改为「归并块 SHALL 仅服务载荷展示（`MutationTestOutcome.score/threshold/counts`），MUST NOT 落 summary.json，score 展示口径 MUST NOT 作门禁判据」；新增 Scenario「归并块不落盘」。

### 零改动的能力及理由

| 能力 | 理由 |
|---|---|
| **desktop-workspace-config** | 零 schema 变更：`tests[].coverage` 三元阈值非 Option 且继续被逐 suite 判定消费；`config.schema.json` / `KNOWN_KEYS` / zod 镜像零 diff |
| **desktop-change-orchestration** | `TestExecutionOutcome` / `MutationTestOutcome` 载荷字段零改动（`conclusion` / 四计数 / `findings_*` / `score` / `threshold` / `counts` / `report_dir` / `exec_id` / `fault`）；升格落账与机械 checklist 落账入口零改动 |
| **desktop-archi-api / desktop-archi-api-check** | `.dev-team/architecture/api/INDEX.toml` 中 `App.Backend.Core.Checks` 与 `App.Backend.Infra.ChecksRuntime` 均 `status = "absent"`，契约面零摩擦 |
| **desktop-ipc-type-bindings** | `summary` 报告体从不出线（载荷零 Serialize）；前端 `bindings.ts` 唯一 coverage 引用是 config 阈值类型 |
| **desktop-exec-runtime / desktop-change-state-store** | 记录链（`LiveExec`）、exec 输出回查、RunNodeRecord 落库零改动 |
| **repo-archi-model** | 元素 summary（「测试报告语义与检查结论的领域规则」）与关系面仍准确；元素 / 关系 / 计数零漂移 |
| **desktop-corpus-regression** | 该 golden 面属 `core/workflow`（db 种子 + detail 投影），与 checks 报告语料无关 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs` | 删 `aggregate_coverage` / `weighted_measured` / `compute_overrides` / `aggregate_measured` / `file_meets_thresholds` / `to_thresholds` / `in_suite_scope` / `under_root` / `default_glob` / `DEFAULT_THRESHOLDS` / `aggregate_mutation`；`determine_conclusion` 入参改布尔面（`failed` + 逐 suite 覆盖判定 + `has_execution_error`，突变退出判定）；`misses_coverage` 判据退化为 `coverage == None` 且归因改 `CoverageFailure`；`build_summary_report` 去 `suites` 参数、不产 `coverage` / `mutation`、覆盖未达阈问题条目改逐 suite（`framework` = suite 框架）；`is_file_excluded` 族与 `coverage_meets_thresholds` / `build_mutation_block` / `merge_mutation_blocks` **逐字保留** |
| `packages/desktop/src-tauri/crates/core/checks/src/diagnose.rs` | `check_integrity` ③ 改以 `sub_reports` 布尔重算（①② 零改动）；`diagnose_findings` 4c 改读 `problems` 的 `CoverageFailure` 条目、删 4d / 4f、4e 措辞逐字保留 |
| `packages/desktop/src-tauri/crates/core/checks/src/model.rs` | `SummaryReport` 删 `coverage` / `mutation` 两字段；`MutationBlock` 文档注释去掉「回填 summary 的 mutation 位」（保留解析保真定位）；`Problem` 的 framework 注释由「覆盖 / 突变全局问题为 all」改为「覆盖问题为该 suite 框架」 |
| `packages/desktop/src-tauri/crates/core/checks/src/lib.rs` | 再导出收窄（去 `aggregate_coverage` / `aggregate_measured` / `in_suite_scope`） |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` | `build_summary_report` 调用点收参（去 `suites`）；`build_sub_report` 覆盖块判定**零改**（仍是唯一判定点的表达）；`coverage_brief` / `findings_detail` 零改动 |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs` | 删 `backfill_summary` 与调用点（归并块仍在内存服务 `MutationTestOutcome`）；`all_pass` 逐 suite 判定**零改动** |
| `packages/desktop/package.json` | `version` `0.6.38 → 0.6.39`（一次 patch）；`src-tauri/Cargo.toml` crate 版本行**不跟** |

### 测试文件

| 文件 | 现状 | 处理 |
|---|---|---|
| `crates/core/checks/src/aggregate_test.rs`（改，1611 行） | `aggregate_coverage` / `in_suite_scope` / `merge_mutation_blocks` / `determine_conclusion` / `build_summary_report` 五组断言 | 删 `aggregate_coverage` 段（含 override 组、加权口径、`in_suite_scope` 三组用例）；`determine_conclusion` 用例改写为布尔面（新增「vite pass + rust functions 未达阈 ⇒ Fail」的掩盖回归锚）；`build_summary_report` 用例改签名并断言 summary **无** `coverage` / `mutation` 键；新增「`coverage == None` ⇒ Fail + 该 suite 一条 `coverage_failure`、无 execution_error」用例；`merge_mutation_blocks` 用例**零改动** |
| `crates/core/checks/src/diagnose_test.rs`（改，442 行） | ③ 自比一致性、4c / 4d / 4f 分支 | ③ 用例改为「summary 结论与子报告布尔矛盾 ⇒ 违例」（含 `coverage == None` 的子报告非通过形态）；4c 用例改为读 `problems` 的 `CoverageFailure`；4d / 4f 用例删；4e 正向例**逐字保留** |
| `crates/core/checks/src/model_test.rs`（改，920 行） | `SummaryReport` 字面量构造 / 字段序断言 | 去掉 `coverage` / `mutation` 两字段；新增「旧报告带 `coverage` / `mutation` 键仍可解析（宽容解析零倒）」用例；`SubReport` 面零改动 |
| `crates/core/checks/tests/corpus_golden_test.rs`（改） | `aggregate_coverage` 导入（:4）+ 投影段（:154-161）+ 3 条 `computedCoverage.*` 断言（:257-261） | 删导入与投影段、删 3 条断言（llvm-cov 用例保留「sub-report 覆盖块在场且 pass」的同义断言，改由子报告面表达）；其余断言零改动 |
| `crates/core/checks/tests/golden/{green-istanbul,green-llvm-cov,green-node-test,red-run}.json`（改） | 各含 `computedCoverage` 块 | `DESKTOP_GOLDEN_REWRITE=1` 显式重写（diff 预期范围 = 仅 `computedCoverage` 键消失），人工 review 留痕；`mutation-block` / `unknown-fields` / `corrupt-*` 四份**零 diff** |
| `crates/infra/checks/src/testexec/mod_test.rs`（改） | :581 `summary.coverage` 断言、:840 `hand_summary` 字面量 | 断言改为「summary 无 coverage 块 + `problems` 携逐 suite 覆盖归因」；字面量去两字段 |
| `crates/infra/checks/src/testexec/report_test.rs`（改） | :48 `summary()` 字面量 | 去 `coverage` / `mutation` 两字段，其余零改 |
| `crates/infra/checks/src/mutation/mod_test.rs`（改） | :943 / :975 / :1030 / :1064 / :1124 / :2010 / :2449 七处「summary mutation 位回填 / 保持 null」断言 | 全部反转为「summary **无** mutation 位（回填面不存在）」；载荷 `score` / `threshold` / `counts` 断言与逐 suite `all_pass` 用例**逐字保留** |

### 不要修改

- **config 面**：`crates/core/config/**`（含 `config.schema.json` 镜像与一致性测试）、`.dev-team/config.json`、`src/types/generated/bindings.zod.ts` 零改动——零 schema 变更，MUST NOT 为本变更新增任何 coverage 开关 / 不支持维度声明。
- **子报告模型面**：`SubReport` 的 `coverage` / `mutation` / `source_files` / `test_files` / `findings` 字段与 `CoverageBlock` / `CoverageMeasured` / `CoverageOverride` / `MutationBlock` / `MutationMeasured` / `MutationOverride` 类型零改动（解析保真 + 旧报告宽容解析）。
- **逐 suite 判定与突变执行面**：`coverage_meets_thresholds`、`build_mutation_block`、`merge_mutation_blocks`、`mutation/mod.rs` 的 `bucket_suites` / `all_pass` / `fail_feedback` / 段执行 / 复用门 / sandbox 清理零改动；`is_file_excluded` 族零改动。
- **编排与载荷面**：`crates/core/orchestration/src/port.rs`（三 outcome 字段）、`walker.rs`（升格落账 / 机械 checklist / fault 分派）、`crates/core/workflow/src/write/phase_table.rs` 零改动。
- **报告写盘与记录面**：`testexec/report.rs` 写盘、`findings_detail` / `coverage_brief` / `diagnose_brief` 装配、`infra/exec` 记录链零改动。
- **前端**：`packages/desktop/src/**` 零改动（唯一 coverage 引用是 config 阈值类型）。
- **架构资产**：`.dev-team/architecture/models/**`、`.dev-team/architecture/api/**` 零改动。
- **规格基线**：`openspec/specs/**` 只在 change 目录出 delta（归档时合入）。
- **`openspec/changes/coverage-aggregation-layer-removal/explore.md`**：只读。
- **`reports/baseline/**`**：工作流自产基线报告，由门禁重跑覆盖，MUST NOT 手改。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | **加权链整链退役**：`aggregate_coverage` / `weighted_measured` / `compute_overrides` / `aggregate_measured` / `file_meets_thresholds` / `to_thresholds` / `in_suite_scope` / `under_root` / `default_glob` / `DEFAULT_THRESHOLDS` / `aggregate_mutation` 全仓零引用（含 `lib.rs` 再导出与测试） | `grep` + `cargo clippy --workspace -- -D warnings`（死代码即红） |
| AC-2 | **summary 面瘦身**：`SummaryReport` 仅剩 `phase` / `command` / `timestamp` / `duration_seconds` / 四计数 / `conclusion` / `problems` / `plans` / `findings`；生产报告 `summary.json` 无 `coverage` / `mutation` 键 | `model_test.rs` 字段断言 + 一次真跑报告 `jq 'has("coverage")'` |
| AC-3 | **判定点唯一且三态**：有值达阈 pass / 有值未达阈 fail / 维度 `None` 跳过 / `coverage == None` fail；四形态各有用例 | `aggregate_test.rs`（或 `testexec/mod_test.rs`）四形态用例 |
| AC-4 | **掩盖不可达（核心回归锚）**：vite-plus `lines 95.40 / br 94.06 / fn 91.60` 全达标 + rust `lines 95.94 / br None / fn 87.60`（functions < 90）⇒ `conclusion == Fail`、`problems` 含 rust 的 `CoverageFailure` | `aggregate_test.rs` 新用例（钉住 explore §2.2 的 26 份实况机质） |
| AC-5 | **缺采集判 fail 不判 Error**：某 suite 绿跑但 `coverage == None` ⇒ `conclusion == Fail`（MUST NOT `Error`）、`problems` 恰一条该 suite 的 `CoverageFailure`（MUST NOT `execution_error`）、他 suite 达标数值不被呈现为整体通过 | `aggregate_test.rs` + `diagnose_test.rs` |
| AC-6 | **rust `branches` 显式不支持仍跳过**：`branches: None` + lines / functions 达标 ⇒ 该 suite 覆盖判定 pass（现网形态零翻转） | `aggregate_test.rs` + 真跑一次（6 份历史形态复算：全部仍 pass） |
| AC-7 | **结论只读布尔**：`determine_conclusion` 签名内无任何 `CoverageBlock` / `MutationBlock` 入参；`build_summary_report` 无 `suites` 参数 | 源码审查 + `grep` |
| AC-8 | **完整性 ③ 更强**：篡改子报告 `coverage.pass` 使 summary 结论无法由布尔重算复现 ⇒ `check_integrity` 返回违例；① 四计数违例、② plan 索引违例的既有用例**逐字保留** | `diagnose_test.rs` |
| AC-9 | **复用门三条件守恒**：`plan_ids_match` + `integrity` 空 + `is_reusable` 语义零改动；覆盖缺失的 fail 报告按既有 fail 同待遇（可复用，只在输入未变时命中） | `testexec/mod_test.rs` 复用门用例 + `reuse_test.rs` 零 diff |
| AC-10 | **`summary.mutation` 不落盘**：`backfill_summary` 与调用点删除；执行完 `summary.json` 无 `mutation` 键；`MutationTestOutcome.score/threshold/counts` 仍在 | `mutation/mod_test.rs` 反转断言 + 载荷用例 |
| AC-11 | **突变不入汇总结论**：`SubReport.mutation` 非 pass 的 corpus 块样本不影响 `summary.conclusion`（突变结论只走 MutationTest 载荷） | `aggregate_test.rs` 用例 + `mutation-block` 语料零 diff |
| AC-12 | **corpus golden 显式重写**：4 份含 summary 面的 golden 仅 `computedCoverage` 键消失（人工 review 逐份确认）、另 4 份零 diff；`corpus_golden_test.rs` 无 `aggregate_coverage` 残留 | `DESKTOP_GOLDEN_REWRITE=1` + `git diff openspec/.../tests/golden` + `grep` |
| AC-13 | **旧报告仍可读**：带 `coverage` / `mutation` 键的历史 `summary.json` 经 `parse_summary_report` 成功解析（宽容解析零倒） | `model_test.rs` 用例 |
| AC-14 | **零 schema / 零 IPC / 零前端**：`config.schema.json`、`bindings.ts` / `bindings.zod.ts`、`packages/desktop/src/**` 零 diff | `git diff --stat` + `bindings:export` 零 diff |
| AC-15 | **门禁**：`pnpm -C packages/desktop run check` 绿（`server:check` → `bindings:export` 零 diff → `api:check` 零 diff → `client:check`）；`cargo nextest run --workspace` 全绿；`package.json` `0.6.38 → 0.6.39`，`src-tauri/Cargo.toml` crate 版本行不变 | 门禁命令 + `git diff --stat` |
| AC-16 | **边界零 diff**：`crates/core/config/**`、`crates/core/orchestration/**`、`crates/core/workflow/**`、`crates/infra/exec/**`、`testexec/report.rs`、`packages/desktop/src/**`、`.dev-team/architecture/**`、`explore.md`、`reports/baseline/**` 全部零 diff | `git diff --stat` 审查 |

**AC ↔ spec requirement 对应**：

| AC | spec requirement / scenario |
|---|---|
| AC-1 / AC-2 / AC-7 / AC-10 / AC-11 | `desktop-checks-domain` ADDED「汇总结论布尔归并与报告瘦身」+ ADDED「聚合链退役清单与解析保真面保留」；`desktop-mutation-testing` MODIFIED「score 口径与逐 suite 阈值判定」 |
| AC-3 / AC-4 / AC-5 / AC-6 | `desktop-checks-domain` MODIFIED「覆盖率采集完整性（缺测不假通过）」 |
| AC-4 / AC-6 / AC-12 | `desktop-test-execution` MODIFIED「执行链与报告落盘」/「双实现对拍黄金语料」/「rust suite 单构建（一条命令同产结果与覆盖率）」 |
| AC-8 / AC-9 | `desktop-checks-domain` ADDED「汇总结论布尔归并与报告瘦身」（③ 重算 + 复用门守恒） |
| AC-10 / AC-11 | `desktop-test-execution` MODIFIED「V1 范围与边界留痕」/「机械 checklist 双槽与 evidence 三态」/「baseline 报告槽位与树隔离」 |
| AC-13 | `desktop-checks-domain` ADDED「聚合链退役清单与解析保真面保留」（宽容解析面） |
| AC-14 / AC-15 / AC-16 | 工程验收面（无 requirement 对应；以门禁与 `git diff` 为判据） |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | **「采集缺席即 fail」在非常规 workspace 首跑变红**（某 suite 未开 coverage / 工件路径写错） | 门禁从静默通过变显式失败，用户误判为「改坏了」 | 低（本仓 6/6 实测 `measured` 全非 None；五框架模板恒带采集旗标） | 该变红是**正确**结论（不可测 ≠ 达标）；`CoverageFailure` 文案显式写「采集缺失」并给「检查注册表采集旗标 / 工件路径」的动作指引；MUST NOT 因怕红而加 opt-out 出口 |
| R-2 | **结论语义从 Error 降为 Fail**（覆盖缺失不再走 `execution_error`） | `is_reusable` 只排除 `Error` ⇒ 覆盖缺失的 fail summary 在输入未变时**可被复用**（不自动修复采集） | 中 | 与「覆盖未达阈的 fail」同待遇，语义上自洽（`Error` 成因面收窄为解析 / harness / 探测失败，已写进 spec）；修复采集必然改配置或改工件 ⇒ 输入 mtime 推进 ⇒ 复用门失效重跑；风险与口径在 spec 明文留痕 |
| R-3 | **corpus golden 重写吞掉非预期差异** | 4 份 golden 整块重写时顺手接受别的字段漂移 | 中 | 重写前先算清「预期 diff = 仅 `computedCoverage` 键消失」，逐份 review；另 4 份必须零 diff；`desktop-test-execution` 的 golden 纪律条款保留 |
| R-4 | **报告自描述度下降**：删 `summary.coverage` 后，只看 `summary.json` 不再有「全局覆盖率数字」 | 决策 agent / 人工回查时误以为「没检查覆盖率」 | 中 | 方向 a：`problems` 逐 suite 归因（framework = suite 框架）+ `findings` 4c 逐 suite 措辞；`plans[]` 已给子报告路径，明细可按 id 直达；决策 prompt 的读报告指引**零改动**（`summary.json` 仍是入口，`problems` / `findings` 足够定位） |
| R-5 | **删除面漏删 / 误删**：`in_suite_scope` 删后 `under_root` / `default_glob` 成孤；反之 `is_file_excluded` 族被顺手删则 mutation 归桶编译失败 | 编译红或死代码告警；最坏是 `mutation` 归桶过滤失效（excludes 不再全桶消费） | 中（该文件 687 行里删除面与保留面交错） | 提案给出精确到函数的删 / 留两张清单；`is_file_excluded` 的消费点（`mutation/mod.rs:435`）写进 spec 的保留条款；AC-1 以 `clippy -D warnings` 兜死代码 |
| R-6 | **`build_summary_report` 签名变更波及测试 / 调用点** | 单一调用点 + 十余处测试字面量，漏改即编译红 | 低 | AC-7 明确签名（去 `suites`）；测试文件清单已点名 `aggregate_test.rs` / `testexec/mod_test.rs` / `report_test.rs` |
| R-7 | **归并 mutation score 仍是一个加权数**（`MutationTestOutcome.score`） | 用户把它误读为门禁判据（门禁其实是逐 suite 布尔） | 中 | spec 明文「展示口径，MUST NOT 作门禁判据」；不落盘（summary 无该位）；逐 suite 明细仍在 `fail_feedback` 文本与报告树 |
| R-8 | **前篇（`test-exec-coverage-scope`）的 13 条遗留未随本变更关闭** | 两个 explore 并存，13 条里有 12 条已随删除面蒸发，白白占用后续注意力 | 中 | `in_suite_scope` 删除使前篇 §4.6（`default_glob` 双镜像）也消失 ⇒ **13/13 消失**；本变更 archive 时一并标注前篇关闭（不立新 change） |
| R-9 | **报告体缩窄影响外部读面**（本仓之外若有消费 `summary.coverage` 的工具） | 读面断裂 | 极低 | 本仓无外部消费（grep 实测：`summary.coverage` 仅 checks 内部与测试）；`parse_summary_report` 宽容解析保证旧文件可读；新文件少两键，未知键读取方本就应容错 |
| R-10 | **`misses_coverage` 的去重规则消失后出现重复归因**（某 suite 既有 `execution_error` 又有覆盖缺失） | `problems` 出现同一 suite 两条不同类型条目 | 低 | 这是**期望形态**（`execution_error` = 执行面、`coverage_failure` = 覆盖面，两件事）；spec 的新 Scenario 明文断言两类互不顶替、`conclusion` 仍由 `execution_error` 优先定型为 `Error` |
| R-11 | **`CoverageOverride` 模型成为「不再产出但保留」的死类型** | 后续读者误以为 override 面仍生效 | 中 | spec 写清「模型保留 = 解析保真 / 旧报告宽容解析，MUST NOT 再生产 override 组」；`model.rs` 注释同步 |
| R-12 | **`check_integrity` ③ 重算与 `determine_conclusion` 语义漂移**（两处各写一份布尔归并） | 复用门的「齐整」前提与门禁结论口径分叉 | 中 | ③ MUST 复用同一个 `determine_conclusion`（不得就地重写判据）；`diagnose_test.rs` 以「篡改子报告 pass ⇒ 违例」钉住同源 |
