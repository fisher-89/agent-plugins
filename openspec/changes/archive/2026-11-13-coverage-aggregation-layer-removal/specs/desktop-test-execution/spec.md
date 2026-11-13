# desktop-test-execution Specification（delta: coverage-aggregation-layer-removal）

## MODIFIED Requirements

### Requirement: 执行链与报告落盘

执行链 SHALL 依序：detect（config tests[] + glob → plan entries）→ 完整性校验（字段 / 计数 / conclusion 一致性 checklist）→ 复用门 → 逐 suite 命令模板展开（`{results_file}` / `{report_dir}` / `{config_args}` / `{files}`）→ **展开长度预算裁决**（超平台预算 → 安全降级或环境归因，见「命令展开长度预算与安全降级」）→ spawn（超时 + 输出捕获）→ 报告写盘 → 聚合（**逐 suite 布尔归并 → conclusion**，见 desktop-checks-domain「汇总结论布尔归并与报告瘦身」）→ 诊断 findings。报告 SHALL 恒落 change 报告目录（`summary.json` + 逐 suite 子报告；路径经 foundation layout 推导，desktop 产品源码零磁盘路径字面量）；报告 MUST NOT 登记 file inventory / artifacts registry（MVP 口径）。突变面 SHALL 由独立 MutationTest 门禁步承载（见「MutationTest 确定性门禁步与时序」与 desktop-mutation-testing）：TestExecution 执行链不产突变结果；summary MUST NOT 携 `mutation` 位——突变结论的唯一载体是 `MutationTest` 步载荷（`conclusion` / `score` / `threshold` / `counts`）与机械 checklist 第三槽（见 desktop-mutation-testing「score 口径与逐 suite 阈值判定」与「机械 checklist 双槽与 evidence 三态」）。同样地 summary MUST NOT 携 `coverage` 位：覆盖率判定唯一在子报告层（逐 suite），跨 suite 只做布尔归并。

#### Scenario: 全链绿跑落盘

- **WHEN** 全部 suite 执行成功且覆盖率达阈值
- **THEN** summary.json 与逐 suite 子报告落 change 报告目录，聚合 conclusion=pass，机械 checklist 可静态判定

#### Scenario: 报告路径经 layout 推导

- **WHEN** runner 写盘且对 desktop 产品源码做命名隔离扫描
- **THEN** 报告目录取自 foundation layout 推导（`reports/test` 挂 change 目录），零硬编码磁盘路径字面量，双禁令扫描通过

#### Scenario: 突变面独立步承载

- **WHEN** TestExecution 全链绿跑完成进入 MutationTest
- **THEN** TestExecution 产出不含突变结果、summary 无 `mutation` 位；MutationTest 的 score / threshold / counts 只在步载荷与机械 checklist evidence 呈现，两步报告各落其位

#### Scenario: 展开预算裁决先于 spawn

- **WHEN** 某 suite 的 `{files}` 展开后命令串超平台预算
- **THEN** 该 suite 在 spawn **之前**被裁决为「安全降级」或「环境归因」两种结局之一，MUST NOT 以未经裁决的超长命令串直接 spawn；其余 suite 的链路步序零变


### Requirement: 双实现对拍黄金语料

报告 schema 与聚合判定 SHALL 以**已固化 Rust 实现 + corpus 金样**为权威：`core/checks` 的解析与聚合实现为唯一权威源，`core/checks/tests/golden`（及 `tests/fixtures` 的 summary.json / report.json 真实样本）为冻结金样；原对齐源（CLI zod schema）已随插件退役删除，MUST NOT 再作为对齐目标或断言依据。金样对拍 SHALL 拦住实现漂移——金样变更必须经显式重写流程（与 `core/workflow` corpus golden 同款纪律），MUST NOT 以 `--force` / 全量重生成方式静默吞掉差异。

本变更（汇总层取消）是金样的**首次非零 diff 演进**，范围 SHALL 逐份可核：summary 面删 `coverage` / `mutation` 两键后，四份含 summary 面的 golden（`green-istanbul` / `green-llvm-cov` / `green-node-test` / `red-run`）的 `computedCoverage` 投影整块退役（`corpus_golden_test.rs` 的 `aggregate_coverage` 导入、投影段与三条 `computedCoverage.*` 断言一并退役），另四份（`mutation-block` / `unknown-fields` / `corrupt-invalid-json` / `corrupt-missing-required`）MUST 零 diff。重写 SHALL 逐份确认「预期差异 = 仅该键消失」，MUST NOT 顺手接受其他字段漂移。

#### Scenario: corpus 对拍拦截漂移

- **WHEN** 以真实 fixtures 驱动 `core/checks` 解析与聚合
- **THEN** 字段 / 计数 / conclusion 与金样逐项一致；实现语义漂移被测试显式指认（差异须以显式金样重写 + spec delta 提出）

#### Scenario: 权威源单点

- **WHEN** 审查 corpus 测试的权威断言来源
- **THEN** 权威只指向 `core/checks` 实现与 `tests/golden` 金样；无「与 CLI 实现一致 / 以 zod schema 为权威」类残留断言（参照物已删除）

#### Scenario: 金样演进范围可核

- **WHEN** 以 `DESKTOP_GOLDEN_REWRITE=1` 重写并 review `git diff` 于 `tests/golden/`
- **THEN** 恰四份含 summary 面的 golden 发生差异，且差异仅为 `computedCoverage` 键消失；另四份零 diff；`corpus_golden_test.rs` 无 `aggregate_coverage` 残留引用


### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续切片偿还，不由实现隐式吸收）：

1. **mutation 已移植（本变更偿还）**：突变由独立 MutationTest 步承载（js=Stryker / rust=cargo-mutants，见 desktop-mutation-testing）；summary 无 `mutation` 位（突变结论由该步载荷与机械 checklist 第三槽承载），「未跑 / diff 空跳过不判 fail」的观察期语义不收紧——观察结果后再决定是否升格 error。
2. **agent 修复能力不进本期**：修阻塞错误（Step1b）与根因唯一性裁决（T4）不实现——红跑以 findings 摘要 + 报告呈现升格相位 fail，根因裁决交决策 agent（本变更进一步明确：相位内零修复会话）。
3. **net-zero 去噪不适用**：突变范围 = change 编辑集（`git diff HEAD` + untracked，`change_edit_set` 单点）；清单反推与 net-zero 去噪（CLI 先例）不移植——diff 面天然无「与 HEAD 同内容」噪音。
4. **无-change 场景不实现**：CLI `<root>/reports/test/` 分支桌面不实现（恒有 change）。
5. **报告不进 file inventory**：不做 artifacts registry 登记。

#### Scenario: 留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含 null 观察期与「相位内零修复会话、根因交决策」的明知接受），后续变更无需重新论证是否知情


### Requirement: 机械 checklist 双槽与 evidence 三态

相位收口的机械 checklist 产物面 SHALL 挂 `test_outcome` + `mutation_outcome` 双槽（两步结论独立成槽）。机械 checklist 第 3 条「突变缺失自动通过」判定语义 SHALL 保留（未跑 / 跳过观察期不收紧），但 evidence SHALL 区分三态：「跑过且达标（score / threshold）/ diff 空跳过（突变面为空）/ null（未跑）」。`mutation_outcome` 槽缺省 SHALL 保持合法（diff 空跳过 / 未跑场景不判 fail）——该槽取自 MutationTest 步载荷，与 summary 位无关（summary 已无 `mutation` 位）。机械 checklist 的构造与落账由相位收口动作承载（`settle_phase` 的三分流之一），MUST NOT 依赖任何计划步节点。

#### Scenario: 三态 evidence 落账

- **WHEN** MutationTest 跑过且达标后相位收口代写机械 checklist
- **THEN** 第 3 条 pass、evidence 为「跑过且达标（score X ≥ threshold Y）」形态；diff 空跳过场景 evidence 为「diff 空跳过」、null 场景为「null（未跑）」，三态可辨

#### Scenario: null 合法不判 fail

- **WHEN** 突变面为空（change 编辑集不含可突变文件）或 MutationTest 未执行
- **THEN** `mutation_outcome` 槽为空，机械 checklist 第 3 条 pass（观察期语义不变），相位照常推进

#### Scenario: 落账不依赖辅助步节点

- **WHEN** test-execution 相位绿跑后审查图节点与落账产物
- **THEN** 机械 checklist 由相位收口一次落账（`PhaseRecord` + 三条 `ChecklistItemRecord`）；图上零 `phase_log` 节点、零 `StepKind::PhaseLog` 审计行；结论与 evidence 内容与本变更前逐字一致


### Requirement: baseline 报告槽位与树隔离

initialize 相位（desktop-change-orchestration「initialize 基线检查相位执行体」）的 test-execution 基线检查 SHALL 与 run 内 test-execution 相位的报告**分树隔离**：基线检查的 summary 与逐 suite 子报告 SHALL 落该 change 目录的 `reports/baseline` 子树（路径经 foundation layout 的 baseline 变体单点推导，见 desktop-checks-domain「baseline 报告槽位参数化」），run 内 test-execution 相位报告仍恒落 `reports/test`（零改动）——两树零混居，MUST NOT 以 exec_id / 时间戳在同目录内区分。基线检查 MUST NOT 执行 mutation（initialize 步序列无 MutationTest 步；summary 已无 `mutation` 位，该位在 run 内相位报告中同样不复存在——突变结论只走 MutationTest 步载荷）。

复用门 SHALL 按报告树各自判定：`reports/baseline` 树自身的 summary 可在输入未变时复用（clean 后输入 mtime 推进自动失效、语义正确）；run 内 test-execution 相位步 MUST NOT 复用 baseline 树的 summary（决策：不做跨树复用红利），`reports/test` 树的复用门语义零改动。static-check 基线执行的输出记录（exec-outputs）SHALL 同锚落 baseline 树；`read_exec_output` 回查面 SHALL 覆盖 baseline 树（exec_id 跨树唯一，查找策略由 design 定稿——槽位参数或跨树查找，回查 MUST 可达）。基线检查结论 fail / error 时 SHALL NOT 走「失败即升格交决策（零 agent 修复）」的升格路径（initialize 语义：Terminal 停止 + Error / Fail 分文案，见 desktop-change-orchestration）——同一 TestExecution 步词在 initialize 表项为 `Terminal`、在 test-execution 表项为 `Escalate`，语义由表项声明承载。

#### Scenario: 基线报告落 baseline 树

- **WHEN** initialize 相位的 test-execution 基线检查执行完毕（绿或红）
- **THEN** summary.json 与逐 suite 子报告、exec-outputs 落 `openspec/changes/<name>/reports/baseline/` 子树（layout 单点推导）；run 内 test-execution 相位后续执行时其报告仍落 `reports/test`，两树互不串扰

#### Scenario: 跨树零复用

- **WHEN** initialize 基线检查全绿（baseline 树 summary=pass）后 run 推进至 test-execution 相位
- **THEN** 相位步照常全量执行（或依 `reports/test` 树自身复用门判定），MUST NOT 复用 baseline 树的 summary 零 spawn 跳过

#### Scenario: baseline 树自身复用与失效

- **WHEN** initialize 基线检查 Terminal 失败后再发起（Clean 步恢复基线）重跑基线检查，与 worktree 输入未变即重跑两种情形
- **THEN** 前者（clean 触碰输入 mtime）复用门失效全量重跑；后者输入未变时复用 baseline 树上次 summary（零 spawn、结论一致）

#### Scenario: 基线回查可达

- **WHEN** 前端打开 initialize 相位 test-execution 节点的 stdout 面板（`outputRef` 回查）
- **THEN** `exec_output_path` 命中 baseline 树的 exec-outputs，`read_file` 读回脚本真实输出，miss 仍为 `None` 非错误


### Requirement: rust suite 单构建（一条命令同产结果与覆盖率）

Test-execution 的 rust 档 SHALL 以**单条命令**执行测试并采集覆盖率：注册表 shell 臂与 cmd 臂模板 SHALL 逐字同串，形如 `cargo llvm-cov nextest --workspace --json --output-path "{coverage_file}" --no-fail-fast -P wps-desktop-gate --tool-config-file "wps-desktop:{report_dir}/nextest.toml"`。结果工件 SHALL 为 nextest 的 **JUnit XML**（`profile.<name>.junit` 产物，`--no-fail-fast` 使红跑产出全量用例集）：runner SHALL 在 spawn 前把运行期配置写进 `{report_dir}/nextest.toml`——专属 profile `wps-desktop-gate`（`inherits = "default"`）、`junit.path` 为报告树内绝对路径、`report-skipped = "ignored"`（缺省 `none` 会把 `#[ignore]` 用例整条丢弃，使 skipped 计数失真）；配置 MUST NOT 落在仓库树，MUST NOT 复用 `profile.default`（避免与仓库自带 `.config/nextest.toml` 互相覆盖）。命令 SHALL 用 `-P`（nextest profile 短旗）而非 `--profile`（后者归 `cargo-llvm-cov` 的构建 profile）；JUnit 无命令行参数（`--junit-path` 不存在），故 MUST 经 `--tool-config-file` 注入。MUST NOT 使用 `--message-format libtest-json`——该格式受 `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1` 实验开关门禁，本档 MUST NOT 依赖任何 nextest 实验特性。runner MUST NOT 对展开后的命令串做任何拼装改写——rust 档 MUST NOT 残留 `; _X=$?; … ; exit $_X` 形态的退出码搬运、MUST NOT 残留 `&`（cmd 无条件顺序分隔符）链式分隔、MUST NOT 由 runner 侧插入结果重定向（JUnit 由 nextest 直接写文件，stdout 无承载）。suite 退出码 SHALL 直继承该单条命令的退出码（MUST NOT 被链中后段命令洗 0）：`cargo llvm-cov` 失败（测试红 / 覆盖率工件未产出 / 环境文件锁）即 suite 非零退出，经 `execution_error` 定型为 `conclusion=Error`——「覆盖率工件缺失而结论 pass」的假绿路径 MUST NOT 可达。结果工件 SHALL 由 `parse_junit` 单一读取（逐 `<testcase>` 产用例行：`<skipped>` → skipped、`<failure>` 或 `<error>` → failed、两者皆无 → passed；`time` 属性为秒，换算毫秒，skipped 用例不记耗时；名称按 `classname$name` 合成；失败面取 `<failure>` 正文并沿用 libtest 的 panic 定位行跳过口径，无正文形态（进程退出码类）退化为 `type` / `message` 属性），覆盖率工件 SHALL 由同一命令的 `--output-path` 落位，采集范围 SHALL 为 cargo workspace 全成员（`--workspace`）——结果工件与覆盖率工件 SHALL 由同一批用例产出，MUST NOT 出现「门禁判定的覆盖率数字来自与门禁测试集不同的一批用例」。test-execution 的 rust 档 MUST NOT 写 / 清 `target/debug`（llvm-cov 走 `target/llvm-cov-target`），跨相位（static-check 的 `cargo clippy` / `cargo run`）的 target 清理冲突 MUST NOT 由本相位引入。覆盖率口径与阈值 SHALL 零调整：沿现网形态（不排除 `*_test.rs`）、阈值保持 lines 90 / branches 85 / functions 90，`--workspace` 带来的分母扩大由真件回归证据承接；口径收紧（排除 `*_test.rs`）MUST NOT 在本变更内发生。

#### Scenario: 单条命令两臂同串

- **WHEN** 审查 rust 档注册表条目与 runner 展开结果
- **THEN** shell / cmd 模板逐字同串且为单条 `cargo llvm-cov nextest --workspace --json --output-path "{coverage_file}" --no-fail-fast -P wps-desktop-gate --tool-config-file "wps-desktop:{report_dir}/nextest.toml"`；命令串不含 `cargo test`、不含 ` & `、不含 `; _X=$?`、不含 `--message-format`、不含 `{results_file}`；runner 展开后的命令与模板展开逐字一致（零追加改写）；写出的 `nextest.toml` 含专属 profile、绝对 `junit.path` 与 `report-skipped = "ignored"`

#### Scenario: 退出码直继承不假绿

- **WHEN** `cargo llvm-cov` 因环境文件锁 / 覆盖率工件未产出以非零退出收场，而同批测试用例本身全绿
- **THEN** suite `exit_code` 非零 → summary `problems` 记 `execution_error`（携命令输出尾段）、`conclusion=Error`；MUST NOT 判 `pass`、MUST NOT 静默降级为「覆盖面无数据但通过」

#### Scenario: 覆盖率与结果同批用例产出

- **WHEN** rust 档真件执行完毕（工作区含 `crates/**` 多成员加根包）
- **THEN** `junit.xml` 的用例覆盖全 workspace 成员（子报告 `summary.total` 与 nextest 汇总的 `tests run` 同量级）、`coverage-summary.json` 覆盖文件数 ≥ 200（含 `crates/core` / `crates/infra` / `crates/tools` / `src-tauri/src`），二者由同一次命令产出；子报告 `source_files` 不再局限于根包 25 个文件

#### Scenario: 不触碰 target/debug

- **WHEN** 同一 run 内 static-check 相位与本相位先后执行（前者 `cargo clippy` / `cargo run` 写 `target/debug`）
- **THEN** 本相位对 `target/debug` 零写入零清理（覆盖率构建落 `target/llvm-cov-target`），跨相位 `failed to remove …\target\debug\deps\*.rcgu.o (os error 32)` 冲突形态不可达

#### Scenario: 阈值零调整达标与红跑覆盖缺失

- **WHEN** 真件回归聚合 summary，及测试红跑时 llvm-cov 不产覆盖率工件两种情形
- **THEN** 前者 lines / functions 均达阈值（实测 96.64 % / 90.22 %）、conclusion=pass；后者子报告 `coverage=null`，该 suite 覆盖判定按「采集缺席」判非通过（工件缺席即不可测，见 desktop-checks-domain「覆盖率采集完整性（缺测不假通过）」），结论仍由 `failed>0` / `execution_error` 定型为 fail / error——两情形均不产生假绿


## Module Contract

### Function: `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `build_summary_report` 调用点 | 改 | 去 `suites` 入参（该入参的唯一消费者 `aggregate_coverage` 已退役）；`build_sub_report` 的覆盖块判定（`coverage_meets_thresholds` + `suite_thresholds` 四档）**零改动**——它仍是「判定点在子报告层」的表达 |
| `coverage_brief` / `findings_detail` / `outcome` / `fault_outcome` | **零改动** | 逐 suite 覆盖对照、明细面与载荷装配仍从子报告 / summary 的计数与 problems 取数；载荷字段面零变化 |

### Function: `packages/desktop/src-tauri/crates/core/checks`（纯层）

| 项 | 变化 | 描述 |
|---|---|---|
| `aggregate.rs`（汇总组装 / 结论判定） | 改 | 计数求和 + 布尔归并；`determine_conclusion` 只读布尔（含「任一 suite 覆盖判定非通过」）；覆盖未达阈 / 采集缺席的问题条目逐 suite 产出（`framework` = 该 suite 框架）；加权 / override / 突变归并整链退役（清单见 desktop-checks-domain「聚合链退役清单与解析保真面保留」） |
| `diagnose.rs` | 改 | `check_integrity` ③ 以子报告布尔重算（①② 零改动）；`diagnose_findings` 4c 改读 `problems` 的 `CoverageFailure`、4d / 4f 退役、4a / 4b / 4e 逐字保留 |
| `model.rs` / `lib.rs` | 改 | `SummaryReport` 删 `coverage` / `mutation` 两字段；再导出收窄 |

### Test: corpus 金样

| 项 | 变化 | 描述 |
|---|---|---|
| `crates/core/checks/tests/corpus_golden_test.rs` | 改 | 删 `aggregate_coverage` 导入、`computedCoverage` 投影段与三条 `computedCoverage.*` 断言；其余投影（subReport / summary 的 conclusion / counts / integrity / diagnose）零改动 |
| `crates/core/checks/tests/golden/{green-istanbul,green-llvm-cov,green-node-test,red-run}.json` | 改 | `DESKTOP_GOLDEN_REWRITE=1` 显式重写，差异 = 仅 `computedCoverage` 键消失 |
| `crates/core/checks/tests/golden/{mutation-block,unknown-fields,corrupt-invalid-json,corrupt-missing-required}.json` | **零改动** | 无 summary 聚合面 |
