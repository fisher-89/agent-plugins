# 设计: coverage-aggregation-layer-removal

> **变更**: coverage-aggregation-layer-removal
> **日期**: 2026-11-12
> **基于**: `proposal.md`（问题 / 提案一~六 / 决策 D1~D7 / 变更范围 / AC-1…AC-16 / R-1…R-12）、`explore.md`（只读，§9 启动条件复核 + §10 两项裁定落地）与三份 delta spec（`desktop-checks-domain` 2 MODIFIED + 2 ADDED / `desktop-test-execution` 6 MODIFIED / `desktop-mutation-testing` 1 MODIFIED）
> **性质**: test-execution 汇总层的**覆盖率加权聚合面整体退役**——`summary.coverage` / `summary.mutation` 两块不再产出，判定点唯一化到逐 suite 子报告，汇总层只做布尔归并与计数求和。零 config schema、零 IPC、零前端、零新能力；一份行为语义收紧（覆盖率采集缺席由「静默跳过」改为「判非通过」）。

---

## 提案与规格同步状态

proposal 的六段提案、七条决策（D1~D7）、§10 两项裁定（裁定 A 采集声明 = 注册表模板恒开；裁定 B `summary.mutation` 不写）与三份 spec delta 均已定稿，`explore.md` 的 §9 事实订正（`check_integrity` ①② 零改动、③ 可重算、golden 影响面 4 份）已被 proposal 采纳。本设计**不重述论证**，只做四件事：

1. 把 spec 已钉语义、未钉**代码形态**的四点收敛到可编译的具体签名：`determine_conclusion` 的入参形态（D1）、覆盖判据单点与归因落点（D2 / D3）、`check_integrity` ③ 的重算复用面（D4）、`diagnose_findings` 4c 的新取数与 4c/4d/4f 的存废（D5）。
2. 给出精确到符号的**删除面 / 保留面**边界（D6），并把 proposal 删除清单之外**因成孤而必须一并删**的两个私有助手（`threshold_findings` / `threshold_brief`，事实校准 3）写进变更清单。
3. 逐条 AC 给落点（「验收标准回应」表），**不写测试策略**（用例编写与门禁执行由独立相位承接）。
4. 把三处必须显式留痕的边界写成可实现形态：`CoverageFailure` 采集缺失文案（含 R-1 的动作指引）、「Error 降 Fail」的成因面收窄与复用门连带（R-2）、golden 重写的预期 diff 契约（R-3）。

### 起点实况核实（工作树实测；Rust 侧前缀 `packages/desktop/src-tauri/`；锚点为改动前行号）

- **聚合与判定**：`crates/core/checks/src/aggregate.rs`（687 行）——`DEFAULT_THRESHOLDS` `:23`、`derive_plan_id` `:35`、`determine_conclusion` `:56`（`failed` + `Option<&CoverageBlock>` + `Option<&MutationBlock>` + `has_execution_error`）、`coverage_meets_thresholds` `:75`（null 维度跳过）、`aggregate_coverage` `:108`、`compute_overrides` `:139`、`file_meets_thresholds` `:177`、`aggregate_measured` `:201`、`weighted_measured` `:240`、`to_thresholds` `:260`、路径助手族 `:275-321`（`to_forward_slash` / `normalize_root` / `posix_normalize` / `posix_join` / `is_absolute_posix` / `under_root`）、`is_file_excluded` `:343`、`is_excluded_by_suite` `:349`、`relative_to_root` `:371`、`in_suite_scope` `:394`、`default_glob` `:415`、`misses_coverage` `:439`、`collect_problems` `:453`（覆盖归因尾块 `:503-512`，`has_execution_error` 局部变量作去重闸）、`format_coverage_failure` `:518`、`build_mutation_block` `:554`、`merge_mutation_blocks` `:579`、`aggregate_mutation` `:600`、`build_summary_report` `:611`（coverage 问题条目 `:625-633`、mutation 问题条目 `:634-645`）。
- **诊断树**：`crates/core/checks/src/diagnose.rs`（219 行）——`check_integrity` `:13`（③ 在 `:63-79`，把 `summary.coverage` / `summary.mutation` 自比）、`diagnose_findings` `:85`（4a `:89-118` / 4b `:120-144` / 4c `:146-161` / 4d+4e `:163-174` / 4f `:176-187`）、`threshold_findings` `:191`、`threshold_brief` `:209`。
- **模型面**：`crates/core/checks/src/model.rs`（683 行）——`Problem` `:91`（`framework` doc 写「覆盖 / 突变全局问题为 "all"」）、`CoverageOverride` `:132`、`CoverageBlock` `:150`、`MutationBlock` `:199`（doc 写「由 MutationTest 步归并回填 summary 的 mutation 位」）、`SourceFileEntry` `:279`（doc 写「override 聚合的原始计数源」）、`SubReport` `:305-336`、`PlanIndexEntry` `:341-350`、`SummaryReport` `:356-390`（`coverage` 字段 `:378-380`、`mutation` 字段 `:381-383`）、`parse_summary_report` `:403`（宽容解析，无 `deny_unknown_fields`）。
- **出口面**：`crates/core/checks/src/lib.rs`（36 行）——`aggregate` 段 `pub use` `:8-11`（含 `aggregate_coverage` / `aggregate_measured` / `in_suite_scope`）。
- **执行链装配**：`crates/infra/checks/src/testexec/mod.rs`（750 行）——`build_summary_report(&sub_reports, suites, …)` 调用点 `:252`、`build_sub_report` `:541`（`coverage_meets_thresholds` + `suite_thresholds`，**唯一判定点**）、`suite_thresholds` `:595`、`coverage_brief` `:475`、`findings_detail` `:397`（读**子报告**覆盖块，`report.coverage.as_ref().map(coverage_brief)`）、复用门 `:159-183`（`plan_ids_match` + `check_integrity` 空 + `is_reusable`）。
- **突变回填**：`crates/infra/checks/src/mutation/mod.rs`（867 行）——`backfill_summary(&reports_dir, &merged)` 调用点 `:362`、定义 `:770`（读 summary.json → `value["mutation"] = …` → 整文件重写）、`all_pass = suite_results.iter().all(|(_, block)| block.pass)` `:371`、`merge_mutation_blocks(&blocks)` `:363`。
- **复用门**：`crates/core/checks/src/reuse.rs`（29 行）——`is_reusable` 只判 `conclusion != Error` 与 `timestamp` 新鲜度。
- **可用的等价断言面**：`crates/core/checks/tests/corpus_golden_test.rs`——投影 `subReport.coveragePresent` `:110`（子报告覆盖块在场性），`aggregate_coverage` 导入 `:4`、`suites_for` `:43`（唯一消费者 `:154`）、`computedCoverage` 投影段 `:152-161`、llvm-cov 的三条断言 `:257-261`。
- **版本面**：`packages/desktop/package.json:3` = `0.6.38`；`src-tauri/tauri.conf.json:4` 为 `"../package.json"` 跟随；`src-tauri/Cargo.toml:6` 的 crate 版本 `0.1.0` **不跟**。
- **契约面**：`.dev-team/architecture/api/INDEX.toml` 的 `App.Backend.Core.Checks` / `App.Backend.Infra.ChecksRuntime` 均 `status = "absent"`（`archi_module` 复核：元素 `api = declared` 但 `api.status = none`）⇒ 删 `pub` 项不产生契约 finding。

### 事实校准（不改 spec 意图，只定实现形态）

1. **Module Contract 里 `collect_problems` 的「零改动」只能读作「既有三条归并规则零改动」**：`misses_coverage` 的归因尾块（`aggregate.rs:503-517`）必须改写为 `CoverageFailure` 且去重闸消失，尾块就在 `collect_problems` 体内。裁定：覆盖归因**单点收敛在 `collect_problems` 的逐 suite 尾段**（D3），失败用例 / `execution_error` 三条规则逐字不动。若把覆盖归因挪到 `build_summary_report`，同一事实就有两个归因入口——与「判定点唯一」同一条纪律冲突。
2. **AC-9 的「`reuse_test.rs` 零 diff」不可达**：`reuse_test.rs:9-25` 的 `summary()` 字面量构造 `SummaryReport`，含 `coverage: None` / `mutation: None` 两行，字段删除后即编译失败。最小动作 = **删这两行**，`is_reusable` 的断言零改（该文件的被测语义确实与覆盖率无关）。AC-9 的实质（复用门三条件守恒）不受影响。
3. **`diagnose.rs` 的两个私有助手随 4c 改写死于无消费者**：`threshold_findings` 的唯一消费者是 4c 与 `threshold_brief`，`threshold_brief` 的唯一消费者是 4c 的 override 展开段。proposal 删除清单未列此二者，但 `clippy -D warnings` 会把它们判红 ⇒ 必须删（D5）。
4. **4c 的逐维度中文措辞不可保留**（不是选择而是必然）：`summary` 不再携实测数值，4c 的唯一数据源是 `problems` 的 `CoverageFailure` 条目文案，而该文案来自 `format_coverage_failure`（英文串 `Coverage below threshold: functions 87.6% < 90%`，spec 明令沿既有实现不动）。故 4c 的措辞形态定为「中文前缀 + 条目原文」（D5），MUST NOT 在 `diagnose.rs` 重算一遍数值（那会造出第二个判定面）。
5. **`corpus_golden_test.rs` 的 `suites_for` 成孤**：其唯一消费者是 `computedCoverage` 投影段（`:154`）。随投影段退役一并删（连同 `config::{MutationConfig, TestFramework}` 导入），否则 `-D warnings` 红。
6. **`SourceFileEntry` / `CoverageOverride` 的既有注释将变成假话**：前者 doc 写「override 聚合的原始计数源」，后者是「per-suite override 组结果」。proposal 的 `model.rs` 行只点了 `MutationBlock` 与 `Problem.framework` 两处注释——本设计补这两处（**仅注释**，字段与 derive 零变），与 R-11「模型保留 = 解析保真面，MUST NOT 再生产 override 组」同源。
7. **llvm-cov 用例的同义断言已在投影里**：`corpus_golden_test.rs:110` 的 `subReport.coveragePresent` 就是「子报告覆盖块在场且 pass」的可表达面（`coveragePresent = sub.coverage.is_some()`，该语料 `coverage.pass = true`）——退役三条 `computedCoverage.*` 断言后无需新增投影字段。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 | 被拒备选（为何拒） |
|---|------|------|------|--------------------|
| D1 | `determine_conclusion` 的新签名 | `pub fn determine_conclusion(failed: u64, sub_reports: &[SubReport], has_execution_error: bool) -> Conclusion`；判据顺序 `has_execution_error → Error` / `failed > 0 → Fail` / 任一 suite 覆盖判定非通过 → `Fail` / 否则 `Pass`；**MUST NOT 接受或读取 `CoverageBlock` / `MutationBlock`** | 两个调用点（`build_summary_report` 与 `check_integrity` ③）都手里就有 `&[SubReport]`；传布尔切片需两处各先折叠一遍（第二个判据面）；传 `&SummaryReport` 会把「还没构造完的 summary」与「已解析的 summary」混成一个入参（`build_summary_report` 里 summary 尚不存在） | ①`coverage_non_pass: bool` 单布尔入参（`build_summary_report` 需自行折叠，`check_integrity` 再折叠一次 ⇒ 两处判据、R-12 的同源要求落空）；②`&SummaryReport`（构造期不可得，且把「读报告」与「算结论」绑死）；③保留 `Option<&CoverageBlock>`（AC-7 明令签名内无覆盖率数值入参） |
| D2 | 覆盖判据的实现单点 | `aggregate.rs` 内两个私有谓词互为边界：`fn misses_coverage(report: &SubReport) -> bool { report.coverage.is_none() }`（无条件，采集旗标是注册表常量）、`fn coverage_non_pass(report: &SubReport) -> bool { misses_coverage(report) \|\| report.coverage.as_ref().is_some_and(\|block\| !block.pass) }`；`determine_conclusion` 与归因函数**只经这两个谓词**判覆盖 | 四形态封闭集（有值达阈 / 有值未达 / 维度 `None` 跳过 / 无数据非通过）落在**两处**：维度级 `None` 由 `coverage_meets_thresholds` 在子报告层消化成 `block.pass`，块级缺席由 `misses_coverage` 消化——两个谓词正是这层的全部内容，别处 MUST NOT 再写 `coverage.is_none()` 或 `!block.pass` 的等价判断 | 直接把 `coverage.is_none()` 内联到各调用点（`determine_conclusion` + 归因 + 未来读者三份副本，spec 已把「判据单点」写成条款） |
| D3 | 覆盖率问题归因的落点与文案 | 单点落在 `collect_problems` 的逐 suite 尾段，经私有 `fn coverage_problem(report: &SubReport) -> Option<Problem>` 产出**每 suite 至多一条** `ProblemType::CoverageFailure`、`framework = report.framework.clone()`：<br>① 缺测（`misses_coverage`）：`覆盖率采集缺失（退出码 {exit_code}）：工件缺失 / 解析失败，该 suite 覆盖率不可测——请检查该 suite 的采集旗标与覆盖率工件路径`；<br>② 未达阈（`coverage_non_pass` 且块在场）：`format_coverage_failure(&block.measured, &block.thresholds)`（**逐字沿用**，不新增口径）；<br>③ 通过与块缺席之外的一切形态 → `None` | ① `CoverageFailure` 取代 `ExecutionError` 是 proposal 裁定 A 的落点（`Error` 成因面收窄）；② 与执行面归因**并存互不顶替**（R-10 的期望形态）⇒ 去重闸 `has_execution_error` 整体删除（局部变量随之消失）；③ 文案含 R-1 缓解要求的动作指引（检查采集旗标 / 工件路径），且仍以「采集缺失」+ 退出码 + 「工件缺失 / 解析失败」定性为骨架（spec 明文要素）；④ 单点收敛使「汇总层只归并、不再判定」在源码上可审 | ①把未达阈归因留在 `build_summary_report`、缺测留在 `collect_problems`（两个归因入口，合计顺序会随组装顺序漂移）；② 缺测文案不带动作指引（R-1 缓解无落点，用户只看到「不可测」不知查哪）；③ 缺测仍记 `ExecutionError`（`Error` 成因面不收窄，且 `is_reusable` 会把可修复的覆盖缺失当部分执行拒复用） |
| D4 | `check_integrity` ③ 的重算 | 保留 ③，体改为 `let expected = determine_conclusion(summary.failed, sub_reports, problems_has_execution_error);`（与门禁**同一个**函数、零就地重写判据）；违例文案 `conclusion 一致性失败: 计数 / 逐 suite 覆盖判定面重算结论为 "<重算>", 报告记载 "<记载>"`；① 四计数对账、② plan 双向齐全性**逐字不动**；`execution_error` 存在性仍从 `summary.problems` 取（`Error` 的唯一成因面） | spec ADDED 要求「比现状更强」；现状是拿 summary 自己的块跟自己比（删块后即「比空集」恒过），改成以子报告布尔重算才真正守住复用门的「齐整」前提；复用同一函数使门禁口径与复用门口径不可能分叉（R-12） | ①删 ③（复用门失去结论一致性守卫，D7 已排除）；②在 `diagnose.rs` 就地重写一份布尔归并（两处判据 = R-12 的退化）；③把 ③ 改成「子报告与 summary 的计数面自洽」（① 已覆盖，重复） |
| D5 | 诊断树 4c / 4d / 4f 的形态 | 4c：`summary.problems` 中 `CoverageFailure` 条目逐条展开，`take(CLUSTER_FRAMEWORK_LIMIT)`，措辞**逐字** `覆盖判定未通过 [{framework}]: {message}`；4d（pass 态「全部框架未产出覆盖数据」）删；4e 保留且在 `conclusion == Pass` 时**无条件**产单条（`全部测试通过且覆盖率达阈值`，逐字）；4f（突变缺口）删；`threshold_findings` / `threshold_brief` 随之删；`CoverageMeasured` 导入随 4c 改动退出 | pass 态不可能无覆盖（缺席即 fail）⇒ 4d 与新语义互斥；`summary.mutation` 不复存在 ⇒ 4f 失去对象；4c 的数据源只剩 `problems`，重算数值会造第二判定面（事实校准 4）；4e 的新位置反而更强（Pass ⇔ 全部 suite 覆盖达标） | ①保留 4d（Pass 态永不触达的死分支，且文案与新语义矛盾）；②4c 只输出 `message` 不带 `framework`（多 suite 时无法回答「哪个 suite」，与 spec 的报告自描述要求冲突）；③把 `format_coverage_failure` 改成中文以保住旧措辞（spec 明令沿既有实现；且会动 `problems` 文案 = 语料/断言面额外漂移） |
| D6 | 删除面 / 保留面的机械边界 | 删：`DEFAULT_THRESHOLDS` / `aggregate_coverage` / `compute_overrides` / `file_meets_thresholds` / `aggregate_measured` / `weighted_measured` / `to_thresholds` / `under_root` / `in_suite_scope` / `default_glob` / `aggregate_mutation` / `build_summary_report` 的 `suites` 形参 / `lib.rs` 三条再导出 / `backfill_summary` + 调用点 / `threshold_findings` / `threshold_brief`。留：`is_file_excluded` + `is_excluded_by_suite` + 路径助手族（`to_forward_slash` / `normalize_root` / `posix_normalize` / `posix_join` / `is_absolute_posix` / `relative_to_root`，消费点 `mutation/mod.rs:435` 归桶过滤）/ `coverage_meets_thresholds` / `derive_plan_id` / `format_coverage_failure` / `build_mutation_block` / `merge_mutation_blocks` / 模型块族与全部 `parse_*` / `globmatch`（`is_excluded_by_suite` 与 `agent` 写保护面共消费） | `in_suite_scope` 是 `under_root` 与 `default_glob` 的**唯一**消费者（grep 实测）⇒ 三者同批删，否则死代码告警；`is_file_excluded` 反之必须留（突变归桶的「excludes 全桶消费」口径是 `desktop-mutation-testing` 的明文条款）；`merge_mutation_blocks` 留是因为载荷 `MutationTestOutcome.score/threshold/counts` 仍要它算（D7） | ①「顺手」把 `is_file_excluded` 一起删（`mutation` 归桶编译红 / excludes 口径静默退化）；②保留 `default_glob` 镜像「以防万一」（proposal 明确其唯一消费者即 `in_suite_scope`，`detect.rs` 的权威注册表不动）；③以 `#[allow(dead_code)]` 过渡（spec：死代码零残留） |
| D7 | `summary.mutation` 不写之后归并块的定位 | 删 `SummaryReport.mutation` 字段、`aggregate_mutation`、`build_summary_report` 的 mutation 问题条目、`backfill_summary` 与调用点、`diagnose_findings` 4f；`merge_mutation_blocks(&blocks)` 调用**保留**（内存态，喂 `MutationTestOutcome.score` / `threshold` / `counts` 与 `fail_feedback`）；门禁判据 `all_pass` 零改动；归并 score 的定位写进返回值上方注释：**载荷展示口径，MUST NOT 作门禁判据（门禁 = 逐 suite `pass` 布尔）** | 突变的门禁本来就是逐 suite 全过（`mutation/mod.rs:371`），归并块只服务展示；持久化一个加权 score 只会再造一个可误读的数（R-7）。注释落点是 R-7 缓解的唯一机械面（spec 已把该定位写进 `desktop-mutation-testing`） | ①保留回填（等于把「一个加权数」写进报告，删覆盖率加权的同时留突变加权，纪律不对称）；②逐 suite 写进 `summary.mutation`（schema 扩面最大，且 `check_integrity` ② 的 plan 索引面要连带扩）；③改呈「最差 suite 的 score/threshold」（新口径，proposal 明拒） |
| D8 | 测试面的本相位动作边界 | 本相位对测试文件的唯一动作 = **让工作区随语义变化编译通过**：① 引用已删符号的用例**整段删除**（其语义对象已不存在）；② `build_summary_report` 调用点去 `&suites` 实参；③ `SummaryReport` 字面量去两字段；④ 随投影段退役删 `computedCoverage` 段与三条断言、`suites_for` 与相关导入。MUST NOT 新增用例、MUST NOT 改断言口径（新语义断言与 AC-3/4/5/6/8/10/11/13 的用例由 test-design / test-gen 相位重建）；golden 重写**不在本相位**（需跑对拍重现投影，本相位不执行测试） | 删除面必然使十余个用例不可编译（`determine_conclusion` 旧 5 参 / `aggregate_coverage` / `in_suite_scope` / `aggregate_mutation` 全不存在）；不删则 `clippy --all-targets` 红、整链门不可过。语义改写（例如「缺测 ⇒ Error」翻成「缺测 ⇒ Fail + CoverageFailure」）属测试设计，不是实现动作 | ①本相位代写新用例（越相位，且会在没有测试设计的前提下钉死断言）；②保留红用例不管（`server:check` 的 `--all-targets` 编译即红，交付门不可过）；③顺手重写 golden（需执行测试二进制，本相位不做） |
| D9 | 明确不做 | ① 不新增任何 config 键（无 `coverage.enabled` / `unsupported_dimensions`；`config.schema.json` / `KNOWN_KEYS` / `bindings.zod.ts` 零 diff）；② 不改 `runner.rs`（`parse_coverage_artifact` 的 `(Vec::new(), None)` 静默面保持——收紧发生在判定点，不在采集点）；③ 不改 `build_sub_report` / `suite_thresholds` / `coverage_brief` / `findings_detail` / `outcome` / `fault_outcome`；④ 不改 `port.rs` / `walker.rs` / `phase_table.rs` / `report.rs`（写盘）/ `infra/exec`；⑤ 不改前端、`bindings.ts` / `bindings.zod.ts`（生成物零重导出）、架构模型与契约；⑥ 不改 `format_coverage_failure` 措辞、不改 `MutationTestOutcome` 字段；⑦ 不引入「最差 suite score」等新展示口径；⑧ 不删 `CoverageOverride` / `SourceFileEntry` 等保真模型 | 逐条对应 proposal「不要修改」与 AC-14 / AC-16；越界项一律拒 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 汇总组装与结论判定（**改**） | 计数求和 + 布尔归并；覆盖判据单点（`misses_coverage` / `coverage_non_pass`）；逐 suite 覆盖归因；加权 / override / 突变归并整链退役 | `crates/core/checks/src/aggregate.rs` | `crate::model` / `crate::globmatch` / `config::TestSuite`（`is_file_excluded` 保留面） | 既有纯函数（零 IO、零 spawn） |
| 完整性校验与诊断树（**改**） | ③ 改以子报告布尔重算（复用 `determine_conclusion`）；4c 改读 `problems` 的 `CoverageFailure`；4d / 4f 退役 | `crates/core/checks/src/diagnose.rs` | `crate::aggregate::{derive_plan_id, determine_conclusion}` | 既有纯函数 |
| 报告模型（**改**） | `SummaryReport` 删 `coverage` / `mutation` 两字段；`MutationBlock` / `Problem.framework` / `SourceFileEntry` / `CoverageOverride` 四处注释改口径 | `crates/core/checks/src/model.rs` | `serde` | 既有 serde 模型 |
| 出口面（**改**） | 再导出收窄（去 `aggregate_coverage` / `aggregate_measured` / `in_suite_scope`） | `crates/core/checks/src/lib.rs` | — | 既有 `pub use` 段 |
| 执行链装配（**改**） | `build_summary_report` 调用点去 `suites` 实参 | `crates/infra/checks/src/testexec/mod.rs` | `checks` | 既有 |
| 突变回填面（**删**） | `backfill_summary` 与其调用点删除；归并块仍留内存服务载荷 | `crates/infra/checks/src/mutation/mod.rs` | `checks::aggregate::merge_mutation_blocks`（保留） | 既有 |
| 逐 suite 判定与解析保真（**零改动**） | `coverage_meets_thresholds` / `format_coverage_failure` / `is_file_excluded` 族 / `build_mutation_block` / `merge_mutation_blocks` / 模型块族 / 全部 `parse_*` / `runner.rs` / `report.rs` / `port.rs` / `walker.rs` / `phase_table.rs` / `infra/exec` / 前端 | 现状不变 | — | — |
| 版本行（**改**） | `version` `0.6.38 → 0.6.39`（单 patch） | `packages/desktop/package.json` | — | — |

依赖方向零新增边：`core/checks` 仍不依赖任何 infra（`config` / `time` / `serde` 全在既有依赖内）；`infra/checks` 对 `checks` 的**入边减少**（去掉 `aggregate_coverage` 与 `is_file_excluded` 之外无新增）。零新增 crate / npm 依赖。

---

## 变更清单 (Change Inventory)

> 路径口径：Rust 侧 `crates/…` 省略 `packages/desktop/src-tauri/` 前缀。
> **覆盖核对**：proposal「变更范围 - 实现文件」7 行全部落位——`core/checks/{aggregate,diagnose,model,lib}.rs`、`infra/checks/testexec/mod.rs`、`infra/checks/mutation/mod.rs`、`package.json`。
> **测试文件**：proposal「测试文件」表逐行归测试相位；本相位只承担「让工作区编译通过」的机械动作（见「编译闭合随动」表，D8）。

### 新增文件

<!-- 本变更为纯退役：零新增源文件、零新增配置、零新增测试模块（无新 `pub mod` 声明，故无需空模块占位）。 -->

无。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/checks/src/aggregate.rs` | ① 删常量 `DEFAULT_THRESHOLDS`（`:23`）与「无 suite 时全局阈值兜底」doc；② `determine_conclusion` 改签名与判据（D1）；③ 新增私有 `coverage_non_pass`、改 `misses_coverage`（D2）——两者放在原 `misses_coverage` 处，doc 写明「四形态封闭集在本层收敛」；④ 删 `aggregate_coverage` / `compute_overrides` / `file_meets_thresholds` / `aggregate_measured` / `weighted_measured` / `to_thresholds` / `under_root` / `in_suite_scope` / `default_glob` / `aggregate_mutation`；⑤ `collect_problems` 删局部 `has_execution_error` 与两处赋值、把覆盖归因尾块改为无条件 `if let Some(problem) = coverage_problem(report)`（D3），并新增私有 `coverage_problem`；⑥ `build_summary_report` 去 `suites: &[TestSuite]` 形参、删 coverage / mutation 两个问题条目、`SummaryReport` 字面量去 `coverage` / `mutation` 两字段、`determine_conclusion(totals.failed, sub_reports, has_execution_error)` 调用；⑦ `use` 区去掉 `CoverageBlock` / `CoverageOverride` / `SourceFileEntry`（成孤），`CoverageMeasured` / `CoverageThresholds` / `MutationBlock` / `MutationMeasured` / `Problem` / `ProblemType` / `PlanIndexEntry` / `CaseSummary` / `SubReport` / `SummaryReport` / `Conclusion` 保留；模块内 doc 段头「覆盖聚合」整段随删除消失。**保留面**：`derive_plan_id` / `coverage_meets_thresholds` / `is_file_excluded` / `is_excluded_by_suite` / 路径助手六件 / `format_coverage_failure` / `build_mutation_block` / `merge_mutation_blocks` 逐字不动 | D1~D3 / D6 / D7；`PHASE_TEST_EXECUTION` / `RELATIVE_REPORTS_PREFIX` 与 `plans[]` / `duration_seconds` 口径零改动 |
| `crates/core/checks/src/diagnose.rs` | ① `check_integrity` ③ 改为 `determine_conclusion(summary.failed, sub_reports, has_execution_error)`（② 的 `has_execution_error` 取值面保留），违例文案改「计数 / 逐 suite 覆盖判定面」（D4）；② `diagnose_findings` 4c 改为遍历 `problems` 的 `CoverageFailure`（`take(CLUSTER_FRAMEWORK_LIMIT)`，措辞 `覆盖判定未通过 [{framework}]: {message}`）；③ 删 4d、4f 分支；4e 改为 `conclusion == Pass` 时无条件产单条；④ 删私有 `threshold_findings` / `threshold_brief`；⑤ `use` 区去掉 `CoverageMeasured`；⑥ 文件头 doc 的「4a → 4b → 4c → 4d/4e → 4f」序号串改「4a → 4b → 4c → 4e」 | D4 / D5；①②（完整性）与 4a / 4b / 4e 文案逐字不动 |
| `crates/core/checks/src/model.rs` | ① `SummaryReport` 删 `coverage: Option<CoverageBlock>` 与 `mutation: Option<MutationBlock>` 两字段及其 doc（字段序变为 `phase` / `command` / `timestamp` / `duration_seconds` / 四计数 / `conclusion` / `problems` / `plans` / `findings`）；② `MutationBlock` doc 改「子报告解析保真 + 载荷展示口径（MUST NOT 落 summary.json）」；③ `Problem.framework` doc 改「覆盖问题为该 suite 的框架（`CoverageFailure` 恒为具体框架）」；④ `SourceFileEntry` doc 改「子报告逐文件覆盖计数（报告保真面；汇总层不再消费）」；⑤ `CoverageOverride` doc 补一句「仅解析保真面：新报告 MUST NOT 再生产 override 组」；⑥ `SummaryReport` 结构体 doc 去掉「CLI schema 逐字段对齐」中已不成立的覆盖率/突变表述（改为「汇总面 = 计数求和 + 布尔归并」） | proposal model.rs 行 + 事实校准 6（④⑤ 为注释同步，字段零变） |
| `crates/core/checks/src/lib.rs` | `pub use aggregate::{…}` 收窄：去 `aggregate_coverage` / `aggregate_measured` / `in_suite_scope`，其余六项（`build_mutation_block` / `build_summary_report` / `coverage_meets_thresholds` / `derive_plan_id` / `determine_conclusion` / `merge_mutation_blocks`）保序保留 | `is_file_excluded` 仍经 `checks::aggregate::…` 路径消费（`mutation/mod.rs:435`），本就不在 crate 根再导出面 |
| `crates/infra/checks/src/testexec/mod.rs` | `:252` 改为 `build_summary_report(&sub_reports, DESKTOP_COMMAND_LABEL, started_at)`（去 `suites` 实参）；`suites` 局部绑定仍被 `detect` / `build_sub_report` 消费，零连带 | spec Module Contract：`build_sub_report` / `coverage_brief` / `findings_detail` / `outcome` / `fault_outcome` **零改动** |
| `crates/infra/checks/src/mutation/mod.rs` | ① 删 `:362` 的 `backfill_summary(&reports_dir, &merged)?;` 与 `:361` 的「summary mutation 位回填（E11…）」注释行；② 删 `:770` 起整个 `fn backfill_summary`；③ `merge_mutation_blocks(&blocks)` 调用与 `:363-371` 的 `all_pass` 逐字保留，仅在 `merged` 绑定处补一行注释「归并块仅服务载荷展示（score / threshold / counts），MUST NOT 作门禁判据（门禁 = 逐 suite `pass`）」；④ `:337` 的「双时点②中断面：不等汇总、不回填 summary」注释按新形态改写（中断面不再有回填动作可谈） | D7；`serde_json` / `report::*` 仍有其他消费者（`duration_ms` 解析 / 段报告写盘）⇒ 导入零连带 |
| `packages/desktop/package.json` | `"version": "0.6.38"` → `"0.6.39"`（单行） | `tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行**不跟** |

### 删除清单（逐符号；AC-1 的机械判据面）

| 符号 | 位置（改前） | 类别 | 成孤理由 |
|------|--------------|------|----------|
| `DEFAULT_THRESHOLDS` | `aggregate.rs:23` | 私有常量 | 「无 suite 时全局阈值兜底」的消费面只有 `aggregate_coverage` |
| `aggregate_coverage` | `aggregate.rs:108` | `pub` 函数 | 汇总层不再产覆盖块 |
| `compute_overrides` | `aggregate.rs:139` | 私有函数 | override 组 MUST NOT 再生产 |
| `file_meets_thresholds` | `aggregate.rs:177` | 私有函数 | 唯一消费者 `compute_overrides` |
| `aggregate_measured` | `aggregate.rs:201` | `pub` 函数 | 逐文件计数加权 = 被退役的第二口径 |
| `weighted_measured` | `aggregate.rs:240` | 私有函数 | 唯一消费者 `aggregate_coverage` |
| `to_thresholds` | `aggregate.rs:260` | 私有函数 | 唯一消费者 `aggregate_coverage` / `compute_overrides` |
| `under_root` | `aggregate.rs:324` | 私有函数 | 唯一消费者 `in_suite_scope` |
| `in_suite_scope` | `aggregate.rs:394` | `pub` 函数 | 「归因范围」概念消失（explore §2.1 A/B/§4.1~4.5 随之蒸发） |
| `default_glob` | `aggregate.rs:415` | 私有函数 | 唯一消费者 `in_suite_scope`；权威注册表在 `detect.rs` 不动 |
| `aggregate_mutation` | `aggregate.rs:600` | 私有函数 | 唯一消费者 `SummaryReport.mutation` |
| `build_summary_report` 的 `suites: &[TestSuite]` 形参 | `aggregate.rs:613` | 形参 | 唯一消费者 `aggregate_coverage` |
| `aggregate_coverage` / `aggregate_measured` / `in_suite_scope` 再导出 | `lib.rs:9-11` | `pub use` | 符号已删 |
| `threshold_findings` | `diagnose.rs:191` | 私有函数 | 4c 改写后零消费者（事实校准 3） |
| `threshold_brief` | `diagnose.rs:209` | 私有函数 | 唯一消费者是 4c 的 override 展开段 |
| `backfill_summary` + 调用点 | `mutation/mod.rs:770` / `:362` | 私有函数 + 语句 | `summary.mutation` 位不复存在 |
| `corpus_golden_test.rs` 的 `aggregate_coverage` 导入 / `computedCoverage` 投影段 / 三条断言 / `suites_for` | 见「编译闭合随动」表 | 测试面 | 被删符号 + 投影键退役 |

### 保留清单（MUST NOT 顺手删）

| 符号 | 位置 | 保留理由 |
|------|------|----------|
| `is_file_excluded` / `is_excluded_by_suite` | `aggregate.rs:343` / `:349` | `mutation/mod.rs:435` 的归桶过滤（「`suite.excludes` 全桶消费」口径） |
| `to_forward_slash` / `normalize_root` / `posix_normalize` / `posix_join` / `is_absolute_posix` / `relative_to_root` | `aggregate.rs:275-321` / `:371` | 上两项的路径归一助手族（`is_file_excluded` 族内闭环） |
| `coverage_meets_thresholds` | `aggregate.rs:75` | 逐 suite 判定单点（`build_sub_report` 消费） |
| `derive_plan_id` / `format_coverage_failure` | `aggregate.rs:35` / `:518` | `plans[]` 索引 / 归因文案单点 |
| `build_mutation_block` / `merge_mutation_blocks` | `aggregate.rs:554` / `:579` | `MutationTestOutcome.score` / `threshold` / `counts` 的唯一计算面 |
| `globmatch`（`pub mod` + `match_glob`） | `crates/core/checks/src/globmatch.rs` / `lib.rs:3` | `is_excluded_by_suite` 与 `agent` 写保护面共消费 |
| `CoverageBlock` / `CoverageMeasured` / `CoverageThresholds` / `CoverageOverride` / `MutationBlock` / `MutationMeasured` / `MutationOverride` / `SourceFileEntry` / `FileCoverageEntry` / `SubReport` 与全部 `parse_*` | `model.rs` / `parser/**` | 解析保真 + 旧报告宽容解析（R-11） |

### 公共函数 / API

> 口径：只列模块级导出函数（`pub`）。私有函数、私有谓词、常量与测试装置不入表（其存废见上两表）。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `aggregate::determine_conclusion` | `crates/core/checks/src/aggregate.rs` | 修改 | `pub fn determine_conclusion(failed: u64, sub_reports: &[SubReport], has_execution_error: bool) -> Conclusion` | 判据 `Error` 优先 → `failed > 0` → 任一 suite 覆盖判定非通过（`coverage == None` 亦算）→ `Pass`；MUST NOT 接受 `CoverageBlock` / `MutationBlock`；突变退出判定（AC-7 / AC-11） |
| `aggregate::build_summary_report` | 同上 | 修改 | `pub fn build_summary_report(sub_reports: &[SubReport], command: &str, started_at: OffsetDateTime) -> SummaryReport` | 计数求和、`problems` 归并（含逐 suite `CoverageFailure`）、`plans[]`、`conclusion`、`duration_seconds`；MUST NOT 产 `coverage` / `mutation` |
| `aggregate::aggregate_coverage` | 同上 | **删除** | — | 汇总层加权聚合入口退出 |
| `aggregate::aggregate_measured` | 同上 | **删除** | — | 逐文件计数加权退出 |
| `aggregate::in_suite_scope` | 同上 | **删除** | — | 归因范围圈定退出 |
| `aggregate::is_file_excluded` | 同上 | 零改动 | `pub fn is_file_excluded(path: &str, suites: &[TestSuite]) -> bool` | 突变归桶过滤唯一入口（保留面） |
| `aggregate::coverage_meets_thresholds` | 同上 | 零改动 | `pub fn coverage_meets_thresholds(measured: &CoverageMeasured, thresholds: &CoverageThresholds) -> bool` | 逐 suite 判定单点（维度 `None` 跳过） |
| `aggregate::build_mutation_block` / `merge_mutation_blocks` | 同上 | 零改动 | `pub fn build_mutation_block(measured: MutationMeasured, threshold: f64) -> MutationBlock` / `pub fn merge_mutation_blocks(blocks: &[MutationBlock]) -> Option<MutationBlock>` | score 公式单点 / 载荷展示归并（门禁不入） |
| `diagnose::check_integrity` | `crates/core/checks/src/diagnose.rs` | 修改（签名不变） | `pub fn check_integrity(summary: &SummaryReport, sub_reports: &[SubReport]) -> Vec<String>` | ③ 以子报告布尔 + `summary.failed` + `problems` 的 `execution_error` 重算（复用 `determine_conclusion`）；①② 逐字不动 |
| `diagnose::diagnose_findings` | 同上 | 修改（签名不变） | `pub fn diagnose_findings(summary: &SummaryReport) -> Vec<String>` | 4a / 4b / 4e 逐字保留；4c 改读 `problems`；4d / 4f 退役 |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `SummaryReport` | `crates/core/checks/src/model.rs` | 修改 | 删 `coverage: Option<CoverageBlock>` / `mutation: Option<MutationBlock>` 两字段（含 doc）；余 12 字段与声明序不变；序列化面自此无 `coverage` / `mutation` 两键（旧文件仍可宽容解析） |
| `Problem` | 同上 | 修改（仅注释） | `framework` doc 改「覆盖问题为该 suite 的框架（`CoverageFailure` 不再有 `"all"` 全局归因）」；字段 / serde 零变 |
| `MutationBlock` | 同上 | 修改（仅注释） | doc 改「子报告解析保真 + `MutationTestOutcome` 载荷展示口径；MUST NOT 落 `summary.json`」 |
| `SourceFileEntry` | 同上 | 修改（仅注释） | doc 改「子报告逐文件覆盖计数（报告保真面）」——汇总层不再消费（事实校准 6） |
| `CoverageOverride` | 同上 | 修改（仅注释） | doc 补「仅解析保真面：新报告 MUST NOT 再生产 override 组」 |
| `SubReport` / `CoverageBlock` / `CoverageMeasured` / `CoverageThresholds` / `MutationMeasured` / `MutationOverride` / `PlanIndexEntry` / `Conclusion` / `ProblemType` / `CaseSummary` / `TestCaseResult` / `FileCoverageEntry` | 同上 | **零改动** | 解析保真面与旧报告宽容解析的载体 |

### 配置

<!-- 本变更零 config 改动：`.dev-team/config.json` / `KNOWN_KEYS` / `config.schema.json` 镜像与一致性测试 / `bindings.zod.ts` 全部零 diff（D9①）。`coverage` 三阈值仍被逐 suite 判定消费，语义与取值零变。 -->
<!-- 依赖、路由 / API 子表见下方对应章节；CLI 子命令零新增（`api:export` / `api:check` / `bindings:export` / `check` 脚本逐字不变）。 -->

无。

### 编译闭合随动（测试文件，机械动作，**不新增用例**）

> 依据：删除面使多处用例引用不存在的符号（编译红），字段删除使 `SummaryReport` 字面量非法。本相位只做「删不可编译用例 / 去实参 / 去字段」，**MUST NOT 改断言口径**（D8）。

| 文件路径 | 落点 | 改什么 |
|----------|------|--------|
| `crates/core/checks/src/aggregate_test.rs` | `:5-7` import | 去 `aggregate_coverage` / `in_suite_scope`（`build_summary_report` / `coverage_meets_thresholds` / `derive_plan_id` / `determine_conclusion` 保留） |
| 同上 | `:147-317` | **删 8 个 `结论判定_*` 用例**（旧 4/5 参 `determine_conclusion`） |
| 同上 | `:319-563` | **删 6 个用例**（`多子报告全局聚合与per_suite_override组` / `suite阈值缺失走默认档不panic` / `子报告source_files全空_产出null零维面` / `suite_glob不命中_override组不产条目` / `suite范围_花括号includes与excludes` / `suite范围_缺省glob花括号命中`）——全部以 `aggregate_coverage` / `in_suite_scope` 为被测对象 |
| 同上 | `:564-682` | 删 `汇总组装逐字段相等_全测齐全`（断言 `summary.coverage` 字段）；`零子报告空汇总不panic`（`:685`）与 `command字段仅存在性断言不锚措辞`（`:705`）各去一个 `&[]` 实参 |
| 同上 | `:949-1013` | 删 `aggregate_mutation同式对拍_经summary组装面`（经 summary 组装面断言 `summary.mutation`） |
| 同上 | `:1067-1611` | 保留 `build_summary_report(...)` 的其余调用点，逐处去 `&suites` 实参并删 `summary.coverage` / `summary.mutation` 字段引用（断言口径不动——其红面由测试相位按新语义重定） |
| 同上 | 装置区 `:18-146` | 删因上列删除而成孤的装置（`block()` / `config_thresholds()` / `suite()` / `entry()` / `round_two()` / `sub_report()` / `counts()` 中实际零消费者者）——以 `cargo check --all-targets` 的未用项告警为判据逐个删 |
| `crates/core/checks/src/diagnose_test.rs` | `:16-96` | `consistent_summary` 去 `&suites` 实参（`suite()` / `config_thresholds()` 随之成孤即删）；`hand_summary`（`:236-258`）去 `coverage` 形参与字面量两字段，其七个调用点（`:291` / `:320` / `:343` / `:361` / `:375` / `:401` / `:428`，其中 `:291` 与 `:401` 随下列两处用例删除一并消失）同步去第 4 实参 |
| 同上 | `:261-317` / `:385-409` | 删 `覆盖未达阈_阈值比对findings携维度与数值`（断言 4c 旧口径 + override 展开）与 `coverage维度null_null感知跳过不产覆盖findings`（4c 旧口径）；`全绿summary零误报_findings仅单条正向`（`:411`）只去 `coverage` 构造与实参（断言 `["全部测试通过且覆盖率达阈值"]` 逐字保留） |
| `crates/core/checks/src/model_test.rs` | `:505-508` / `:520` / `:558` | 删 `parsed.coverage` / `parsed.mutation` 的读取断言（`FULL_SUMMARY_JSON` 与 `最小合法summary` 输入文本**零改**——它们正好充当 AC-13 的旧报告样本） |
| `crates/core/checks/src/reuse_test.rs` | `:19-21` | 删 `summary()` 字面量的 `coverage: None` / `mutation: None` 两行（断言零改——事实校准 2 / AC-9 的实质） |
| `crates/core/checks/tests/corpus_golden_test.rs` | `:4` / `:43-75` / `:152-161` / `:257-261` | 删 `use checks::aggregate::aggregate_coverage;`、`suites_for` 及其 `config::{MutationConfig, TestFramework}` 导入、`computedCoverage` 投影段、llvm-cov 的三条 `computedCoverage.*` 断言；其余投影与断言零改（事实校准 5 / 7） |
| `crates/infra/checks/src/testexec/report_test.rs` | `:48-79` | `summary()` 字面量删 `coverage: Some(CoverageBlock{…})` 与 `mutation: None` 两段（`coverage` 三类型导入仍被子报告 fixture 消费，保留） |
| `crates/infra/checks/src/testexec/mod_test.rs` | `:840-858` | `hand_summary` 字面量删两字段 |
| 同上 | `:577-585` | 删 `let coverage = summary.coverage.as_ref().expect(...)` 与紧随的三条覆盖断言（子报告面同义断言由测试相位改指 `sub.coverage`） |
| 同上 | `:3324-3330` / `:3371-3377` | 两处 `checks::build_summary_report(…, &[], …)` 去 `&[]` 实参 |
| `crates/infra/checks/src/mutation/mod_test.rs` | `:943-971` | `summary回填_在位回写不在位不崩`：不可编译面为零（断言走 `serde_json::Value`），保留文件名与结构；断言由测试相位反转为「summary 无 `mutation` 键」（本相位不动断言，仅登记） |
| 同上 | `:1030-1034` / `:1124-1128` / `:2007-2010` / `:2446-2449` | 同上（`read_summary` 与 `serde_json` 索引在编译面合法，零机械改动） |

### 交接（测试相位承接，本相位不执行）

| 交接项 | 内容 |
|--------|------|
| 新语义用例 | AC-3 四形态（有值达阈 / 有值未达 / 维度 `None` 跳过 / `coverage == None`）、AC-4 掩盖回归锚（vite 全达标 + rust `functions 87.60` ⇒ `Fail`）、AC-5 缺采集判 fail 不判 Error、AC-6 rust `branches: None` 仍跳过、AC-8 篡改子报告 `pass` ⇒ ③ 违例、AC-10 `summary.json` 无 `mutation` 键 + 载荷三字段仍在、AC-11 突变块不入汇总结论、AC-13 旧报告带两键仍可解析 |
| 断言重定 | `aggregate_test.rs` 缺测归因组（`:1067-1611` 保留部分）由「`execution_error` + `Error`」翻为「`CoverageFailure` + `Fail`」；`mutation/mod_test.rs` 七处「回填 / 保持 null」翻为「summary 无该键」 |
| golden 重写 | `DESKTOP_GOLDEN_REWRITE=1` 显式重写**恰四份**（`green-istanbul` / `green-llvm-cov` / `green-node-test` / `red-run`），预期 diff = **仅 `computedCoverage` 键消失**；`mutation-block` / `unknown-fields` / `corrupt-invalid-json` / `corrupt-missing-required` 四份**零 diff**。已核实四份语料的 `problems` 面零变化（coverage 块均在场且 `pass = true`），故 `conclusion` / `counts` / `problemTypes` / `integrity` / `diagnose` 投影逐字不变 |
| 门禁 | `pnpm -C packages/desktop run check`（`server:check` → `bindings:export` 零 diff → `api:check` 零 diff → `client:check`）与 `cargo nextest run --workspace` 全绿 |

---

## 数据模型 (Data Model)

### 汇总报告线面（`reports/{test|baseline}/summary.json`）

```
SummaryReport（改后 12 字段，声明序即序列化序）
  phase            : String    恒 "test-execution"
  command          : String    命令描述固定串（桌面固定串）
  timestamp        : String    RFC 3339（复用门新鲜度判据）
  duration_seconds : f64       链路墙钟差整秒
  total/passed/failed/skipped : u64   各子报告之和（仅供人读，MUST NOT 反推覆盖率）
  conclusion       : "pass" | "fail" | "error"
  problems         : Problem[] （见下「问题归因形态」）
  plans            : PlanIndexEntry[]   逐个 planId 路径索引（字段零改动）
  findings         : String[]?          诊断树产物（缺省省略键）

  删除：coverage（Option<CoverageBlock>）、mutation（Option<MutationBlock>）
```

`parse_summary_report` 仍为宽容解析（无 `deny_unknown_fields`）⇒ 旧文件里的 `coverage` / `mutation` 键被**忽略**，结论 / 计数 / `problems` 面照旧可用（AC-13）。

### 问题归因形态（`problems[]`）

| 情形 | `type` | `framework` | `message` |
|------|--------|-------------|-----------|
| 失败用例（至多 10 条 / suite） | `test_failure` | 该 suite 框架 | 既有措辞（零改动） |
| 非零退出且零失败 / 零失败携 findings | `execution_error` | 该 suite 框架 | 既有措辞（零改动） |
| 覆盖块在场但未达阈（每 suite 至多一条） | `coverage_failure` | **该 suite 框架** | `format_coverage_failure` 原文（如 `Coverage below threshold: functions 87.6% < 90%`） |
| 覆盖块缺席（采集缺失，每 suite 至多一条） | `coverage_failure` | **该 suite 框架** | `覆盖率采集缺失（退出码 {n}）：工件缺失 / 解析失败，该 suite 覆盖率不可测——请检查该 suite 的采集旗标与覆盖率工件路径` |

覆盖面与执行面**并存互不顶替**：同一 suite 可同时出现 `execution_error` 与 `coverage_failure` 两条，`conclusion` 仍由 `execution_error` 优先定型为 `error`（R-10 的期望形态）。`framework = "all"` 的全局归因不再产出。

### 子报告线面（`reports/{test|baseline}/<planId>/report.json`）——**零改动**

`SubReport` 的 `coverage` / `mutation` / `source_files` / `test_files` / `findings` 与 `CoverageBlock`（含 `overrides`）/ `MutationBlock` 全字段保留：覆盖率数值的**唯一权威面**自此是逐 suite 子报告（`coverage.pass` 由 `coverage_meets_thresholds` + 该 suite 阈值判定，`overrides` 字段仅解析保真、新报告不产）。

### 进程内中间结构

| 结构 | 形状 | 语义 |
|------|------|------|
| 覆盖判据 | 私有谓词 `misses_coverage` / `coverage_non_pass`（`&SubReport -> bool`） | 四形态封闭集在纯层的全部内容；`determine_conclusion` 与归因函数只经它判覆盖 |
| 突变归并块 | `MutationBlock`（`merge_mutation_blocks` 产出，内存态） | 只服务 `MutationTestOutcome.score/threshold/counts` 与 `fail_feedback`；**不落盘**、不作门禁判据 |

### 持久化

| 面 | 现状 | 本变更 |
|----|------|--------|
| `summary.json` | 含 `coverage` / `mutation` 两键（后者由 MutationTest 二次回写） | 两键均不产出；`mutation` 回写面（`backfill_summary`）删除 |
| `<planId>/report.json` | 含 `coverage`（逐 suite 判定）/ `mutation`（本链恒 `null`）/ `source_files` | **零改动** |
| 历史归档报告 | 含两键 | 读取侧宽容（键被忽略），零迁移、零重写 |
| 数据库 / 队列 / 缓存 | — | 零改动（报告是文件态产物；`RunNodeRecord` / `LiveExec` 记录链零变） |

---

<!-- 路由 / API 设计：本变更零 IPC、零路由、零 CLI 子命令 —— summary 报告体从不出线（`TestExecutionOutcome` / `MutationTestOutcome` 载荷字段零改动，spec 已核实前端 `bindings.ts` 的唯一 coverage 引用是 config 阈值类型）。故本节整段省略。 -->

## 依赖 (Dependencies)

### 运行时依赖（**零新增**）

| 依赖 | 归属 | 用途（本变更内） |
|------|------|------------------|
| `config`（既有） | `core/checks` | `TestSuite` 仍在 `is_file_excluded`（突变归桶保留面）的签名里 |
| `time`（既有） | `core/checks` | `build_summary_report` 的 `started_at` 与 RFC 3339 序列化 |
| `serde` / `serde_json`（既有） | `core/checks` | 模型序列化与宽容解析 |
| `checks`（既有） | `infra/checks` | `build_summary_report` / `merge_mutation_blocks` / `is_file_excluded`（入边净减少 1 条：`aggregate_coverage`） |

**零新增包**：`Cargo.toml` / `Cargo.lock` / `package.json` 依赖集均不变。

### 构建 / 测试依赖（**零新增**）

| 依赖 / 工具 | 用途 |
|-------------|------|
| `cargo clippy --workspace --all-targets`（`server:check`） | 删除面零残留的死代码判据（`-D warnings`）；测试面机械闭合判据 |
| `DESKTOP_GOLDEN_REWRITE`（`corpus_golden_test.rs` 既有开关） | 四份 golden 的显式重写（归测试相位） |
| `api:check`（`archi-api-gen --check`） | 零 diff 复核（`App.Backend.Core.Checks` / `App.Backend.Infra.ChecksRuntime` 均 `status = "absent"`，删 `pub` 项不产生 finding） |
| `bindings:export`（`export-bindings`） | 零 diff 复核（checks 侧类型不出线） |

---

## 验收标准回应

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 加权链整链退役 | 「删除清单」11 个符号 + `lib.rs` 三条再导出 + `backfill_summary` + 两个 `threshold_*` 助手；`cargo clippy --all-targets -D warnings` 为零残留判据；D6 给出删 / 留互斥边界 |
| AC-2 summary 面瘦身 | 「类型定义」`SummaryReport` 行（12 字段声明序）+「数据模型」字段表；两键删除后生产 `summary.json` 无 `coverage` / `mutation` |
| AC-3 判定点唯一且三态 | D2（两个谓词覆盖四形态）+ 零改动的 `coverage_meets_thresholds`（维度 `None` 跳过）+ 零改动的 `build_sub_report`（唯一判定点）；用例由测试相位按四形态补齐 |
| AC-4 掩盖不可达 | D1（`determine_conclusion` 逐 suite 遍历）+ D3（逐 suite `CoverageFailure`）：vite 全达标 + rust `functions 87.60` ⇒ `Fail` 且 `problems` 携 rust 条目 |
| AC-5 缺采集判 fail 不判 Error | D2（`misses_coverage` 无条件）+ D3（`CoverageFailure` 而非 `execution_error`）+ D1（`Error` 只由 `has_execution_error` 触发）；R-2 的成因面收窄写进 `model.rs` / spec 双面 |
| AC-6 rust `branches` 仍跳过 | 零改动的 `coverage_meets_thresholds`（`None` 维度不参与）+ D2 谓词只读 `block.pass`（不重算维度）⇒ 现网形态零翻转 |
| AC-7 结论只读布尔 | D1 签名（无 `CoverageBlock` / `MutationBlock`）+ 「公共函数」表两行；`build_summary_report` 无 `suites` 形参 |
| AC-8 完整性 ③ 更强 | D4（③ 复用 `determine_conclusion`、以子报告布尔重算）+ ①② 零改动（Module Contract 逐字保留） |
| AC-9 复用门三条件守恒 | D4（③ 仍在 `check_integrity` 内）+ `reuse.rs` 零改动 + 事实校准 2（`reuse_test.rs` 仅删两行字面量，断言零改） |
| AC-10 `summary.mutation` 不落盘 | D7（删字段 / `aggregate_mutation` / `backfill_summary` / 4f）+ 保留 `merge_mutation_blocks` ⇒ 载荷三字段仍在 |
| AC-11 突变不入汇总结论 | D1（`determine_conclusion` 无突变入参）+ D7（`build_summary_report` 不再产突变问题条目）+ 语料 `mutation-block` 无 summary 面（零 diff） |
| AC-12 corpus golden 显式重写 | 「交接」golden 行：恰四份、预期 diff = 仅 `computedCoverage` 键消失、另四份零 diff；`corpus_golden_test.rs` 无 `aggregate_coverage` 残留（「编译闭合随动」表） |
| AC-13 旧报告仍可读 | 「数据模型」宽容解析条 + `parse_summary_report` 零改动 + `FULL_SUMMARY_JSON` 输入文本零改（model_test 的输入即样本） |
| AC-14 零 schema / 零 IPC / 零前端 | D9 ①⑤ + 「配置」子表无（注释说明）+ 「依赖」零新增；生成物零重导出 |
| AC-15 门禁 | 「修改文件」`package.json` 行（`0.6.38 → 0.6.39`，`Cargo.toml` 不跟）+ 「交接」门禁行 |
| AC-16 边界零 diff | D9 全部条目 + 「架构组件」零改动行；`config/**` / `orchestration/**` / `workflow/**` / `infra/exec/**` / `testexec/report.rs` / 前端 / 架构资产 / `explore.md` / `reports/baseline/**` 均不在任何修改条目内 |

---

## 待决问题 (Open Questions)

1. **`collect_problems` 的「零改动」口径（事实校准 1）**：spec Module Contract 把 `collect_problems` 列在「零改动」行，而本设计把逐 suite 覆盖归因收敛在其体内（D3，仅改尾块 + 删去重闸，三条既有归并规则逐字不动）。若评审要求严格按「零改动」字面，则需改为「覆盖归因由 `build_summary_report` 新增一次遍历产出」——代价是覆盖归因出现第二个入口（缺测在 `collect_problems`、未达阈在 `build_summary_report`），与「判定 / 归因单点」纪律冲突。不阻塞实现，请裁决取前者。
2. **缺测文案的动作指引长度（D3 ①）**：spec 要求「含退出码与『工件缺失 / 解析失败』定性」，R-1 要求「给『检查采集旗标 / 工件路径』的动作指引」。本设计把两者合成一句（约 60 字）。若评审希望文案更短（去掉动作指引），只需改一个字符串常量——断言面由测试相位同步。
3. **4c 的措辞退化为「中文前缀 + 英文原文」**（事实校准 4 / D5）：`threshold_findings` 的逐维度中文措辞（「行覆盖 62.4% < 阈值 80%」）不再可得。若评审要求诊断面保持纯中文，需改 `format_coverage_failure`（牵动 `problems` 文案面与 `model_test` 的 `CoverageFailure` 样本），属本变更范围外的一处措辞变更——请裁决保持现状或另开变更。
4. **`reuse_test.rs` 与 AC-9「零 diff」的措辞张力**（事实校准 2）：实际最小改动是删两行字面量。建议在 AC-9 的理解上以「断言零改、复用门语义零改」为准；若要求 AC-9 严格零 diff，则必须保留字段（与 AC-2 冲突）。不阻塞实现。
5. **`aggregate_test.rs` 保留部分的红面规模**：本相位按 D8 只做机械闭合，`:1067-1611` 的缺测归因组将带着旧断言（`ExecutionError` / `Error`）留在树上、由测试相位翻新。若评审希望本相位一并把「`Error` → `Fail`」与「`problem_type` → `coverage_failure`」翻掉（属机械对照改写而非新用例设计），可在 tasks 阶段三追加一条——请裁决。
