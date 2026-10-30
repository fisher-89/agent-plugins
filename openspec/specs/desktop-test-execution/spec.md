# desktop-test-execution Specification

## Purpose

Desktop 后端 test-execution 相位确定性门禁步：walker 经 `TestExecutionRunner` port（checks 边界 `crates/infra/checks` 实现）执行 jest / vitest / vite-plus / rust / node-test 五框架测试矩阵——绿跑零 WorkerAgent 会话、fail / error 走独立预算定向反馈边；报告恒落 change 报告目录、复用门幂等、结论与 CLI zod 权威 schema corpus 对拍。

## Requirements

### Requirement: test-execution 确定性门禁步与绿跑零 agent

Walker SHALL 在 test-execution 相位收敛点执行确定性 test-execution 门禁步（`ToolCommand::TestExecution`，经 `TestExecutionRunner` port 由 checks 边界 infra 实现；相位序由 `phase_table` 的 `TEST_EXECUTION_STEPS` 静态定义）。结论 pass 时相位 SHALL 以机械 eval checklist 代写 `phase_log`（verdict=pass）推进，全程 MUST NOT 发起任何 WorkerAgent 会话（「executor agent 驱动 CLI 子命令跑测试」的旧路径随插件退役删除，本条为**防回退护栏**（MUST NOT 因参照物消失而放宽为可选））；结论 fail / error 时 SHALL 走「失败即升格交决策（零 agent 修复）」requirement 的升格路径——首败即落账交决策，MUST NOT 在相位内发起修复会话。runner 以 `Err` 收场（程序不可达 / 配置不支持 / 写盘失败）SHALL 停给用户（显式失败，与 WorkerAgent 失败同等呈现）。

#### Scenario: 绿跑零 agent

- **WHEN** walker 走到 test-execution 相位且全部 suite 通过、覆盖率达阈值
- **THEN** 零 WorkerAgent 会话发起（该步 run 节点行 `session_id` 为空、无 provenance `source_ref` 为 `<change-id>/test-execution/<role>/<attempt>` 的会话记录），机械 checklist 代写 `phase_log` 落账 pass，相位推进

#### Scenario: runner 显式失败停给用户

- **WHEN** runner 以 `Err` 收场（测试程序不可达 / 配置声明不支持框架）
- **THEN** 图上 TestExecution 节点呈失败态（与 executor 会话失败同等可观测），run 不静默越过该节点、不产出部分结论

#### Scenario: 零会话面不因参照物退役而放宽

- **WHEN** 审查 test-execution 收敛点的会话发起面
- **THEN** 全档结论（pass / fail / error / `Err` 收场）均零 WorkerAgent 会话，相位序取自 `TEST_EXECUTION_STEPS` 静态表
- **AND** 无「对齐插件工作流相位语义」类外部参照断言或注释（参照物已删除）

### Requirement: 框架矩阵与显式报错

Test-execution SHALL 支持 jest / vitest / vite-plus（istanbul 族，`coverage-summary.json`）/ rust（单条 `cargo llvm-cov nextest … --message-format libtest-json` 同产 libtest-json 结果与覆盖，见「rust suite 单构建（一条命令同产结果与覆盖率）」）/ node-test（`--test-reporter=spec` 文本结果）五框架的探测与执行；解析器 SHALL 为 node-test 文本分支（`parse_spec_report`）与 rust libtest-json 分支（`parse_libtest_json`）。tests[] 声明 bun / go / pytest suite 时 runner SHALL 显式 `Err`（框架不被支持），MUST NOT 静默跳过该 suite；go-parser / coverage-py 不移植。

#### Scenario: 五框架探测产出 plan

- **WHEN** tests[] 声明五框架 suite（含 includes / excludes glob）
- **THEN** detect 逐 suite 产出 plan entry（root / cwd / id / files），排除过滤生效

#### Scenario: 不支持框架显式报错

- **WHEN** tests[] 声明 `framework: bun`（或 go / pytest）
- **THEN** runner 显式 `Err` 指明框架不被支持，不产出部分结论、不静默跳过

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

### Requirement: 复用门幂等

复用门 SHALL 以输入新鲜度（mtime 递归比较）判定：输入未变 SHALL 复用上次 summary（零 spawn 零重跑、结论照旧），输入变化 SHALL 失效并全量重跑。复用门是 test-execution 相位自动化的幂等前提——自动触发 MUST NOT 演变为反复全量跑。

#### Scenario: 输入未变复用

- **WHEN** 同一 change 连续两次进入 test-execution 相位且工作区输入未变
- **THEN** 第二次复用上次 summary，零测试进程 spawn，结论一致

#### Scenario: 输入变化失效重跑

- **WHEN** 上次报告之后任一输入文件 mtime 更新
- **THEN** 复用门失效，全量重跑产出新 summary 与新结论

### Requirement: 失败即升格交决策（零 agent 修复）

test-execution 相位两步（TestExecution / MutationTest）结论 fail / error 时 walker SHALL 立即以固定三段升格相位 fail：桌面代写一条 fail checklist（`phase_log` 落账）→ 截断本相位余下步（`StepFlow::Upgraded`）→ 回 `phase_next` 进决策分叉。升格全程 MUST NOT 发起任何 WorkerAgent 会话（不注入修复、不续注会话、不自走重跑循环），裁决权 SHALL 交回决策 agent（backtrack / stop / ask）。升格落账的 `report` SHALL 携完整失败明细（test-execution = `findings_detail`，上限 `MAX_REPORT_CHARS = 2000` 字符；原反馈边修复 prompt 的明细面转喂决策信息面），evidence SHALL 携诊断摘要（`diagnose_brief` 口径）与报告目录（`report_dir`）——决策 agent 据此判根因并可自行 `read` 报告文件取全量。fail 条目 MUST NOT 携 executor / evaluator 会话槽位（本相位零会话；决策会话槽位由 `decision-log` 单独挂账）。runner 以 `Err` 收场（环境 / 配置 / 写盘问题）SHALL 仍按「test-execution 确定性门禁步与绿跑零 agent」的 Terminal 语义停给用户，MUST NOT 升格进决策（环境问题不该由决策 agent 裁决）。

#### Scenario: 首败即升格零会话

- **WHEN** test-execution 相位首次执行且 runner 结论 `conclusion=Fail`（或 `Error`）
- **THEN** 零 `WorkerRole::Executor` / `WorkerRole::Evaluator` 请求、恰一条 `verdict=Fail` PhaseRecord（`executor` / `evaluator` / `decision` 三槽位均 None）、`TestExecution` 节点呈失败态、余下步（MutationTest 与主分支 PhaseLog）不执行；run 回 `phase_next` 进决策分叉

#### Scenario: 升格落账的 report 与 evidence 分工

- **WHEN** runner 产出超 200 字的 findings 明细后升格
- **THEN** fail 条目 `report` 携完整明细文本（≤2000 字符，超长按上限截断并留省略号），`checklist` 恰一条 `pass=false`、evidence 为 `diagnose_brief` 摘要 + 报告目录路径；决策 prompt 的「失败相位报告」段取该 `report`

#### Scenario: 同相位每次失败各自升格

- **WHEN** 决策 backtrack 回前置相位、修复后再次进入 test-execution 且再次 fail
- **THEN** 新落一条 fail 条目（无决策槽位），照常升格并触发一次新的决策分叉（幂等闸只作用于单条目：已挂决策会话的条目不再重复分叉）

### Requirement: 步骤可观测与最小载荷

`ChangeStepKind::TestExecution` 与 `ChangeStepKind::MutationTest` SHALL 入步词汇封闭集（18→19，`mutation_test` 线词 1:1 全映射），门禁步状态经 `RunUpdate::Step` 上图（前端流程视图现有步骤渲染直接吃，零渲染改动）；IPC 类型随 bindings 再生成出线。`ToolStepOutput::TestExecution` 载荷面 SHALL 为 conclusion + 计数 + 诊断摘要 + 明细文本 + 报告路径：摘要面服务步状态 detail（`diagnose_brief` 口径截断），明细面服务升格 fail 条目的 `report`（有界装配、pass 态恒空串；原反馈边修复 prompt 的消费点退役后转喂决策信息面）。`ToolStepOutput::MutationTest` 载荷面 SHALL 为 conclusion + score / threshold + 计数 + 报告路径清单 + 反馈文本（结论摘要式，升格时同样转喂 `report`）。载荷 SHALL 零 Serialize 不进 IPC——明细文本 MUST NOT 经 bindings / `RunUpdate` 出线，只在进程内消费；全量 findings / 突变明细留报告文件，MUST NOT 经 IPC 面全量透传。

#### Scenario: 步骤上图零渲染改动

- **WHEN** 门禁步状态变化且 bindings 再生成
- **THEN** `RunUpdate::Step` 携 testExecution / mutationTest 步词汇流出，前端流程视图以既有步骤渲染直接消费

#### Scenario: 载荷双面各司其职

- **WHEN** 审查 `ToolStepOutput::TestExecution` 载荷
- **THEN** 无结构化 findings 全量数组字段：摘要面（`findings_brief`）走步 detail，明细面（`findings_detail`，有界文本）走升格 fail 条目的 `report`（供决策 prompt 的失败报告段）；两者均不进 bindings / IPC，全量以报告文件为权威（`report_dir` 定位）

### Requirement: 双实现对拍黄金语料

报告 schema 与聚合判定 SHALL 以**已固化 Rust 实现 + corpus 金样**为权威：`core/checks` 的解析与聚合实现为唯一权威源，`core/checks/tests/golden`（及 `tests/fixtures` 的 summary.json / report.json 真实样本）为冻结金样；原对齐源（CLI zod schema）已随插件退役删除，MUST NOT 再作为对齐目标或断言依据。金样对拍 SHALL 拦住实现漂移——金样变更必须经显式重写流程（与 `core/workflow` corpus golden 同款纪律），MUST NOT 以 `--force` / 全量重生成方式静默吞掉差异。

#### Scenario: corpus 对拍拦截漂移

- **WHEN** 以真实 fixtures 驱动 `core/checks` 解析与聚合
- **THEN** 字段 / 计数 / conclusion 与金样逐项一致；实现语义漂移被测试显式指认（差异须以显式金样重写 + spec delta 提出）

#### Scenario: 权威源单点

- **WHEN** 审查 corpus 测试的权威断言来源
- **THEN** 权威只指向 `core/checks` 实现与 `tests/golden` 金样；无「与 CLI 实现一致 / 以 zod schema 为权威」类残留断言（参照物已删除）

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续切片偿还，不由实现隐式吸收）：

1. **mutation 已移植（本变更偿还）**：突变由独立 MutationTest 步承载（js=Stryker / rust=cargo-mutants，见 desktop-mutation-testing）；`mutation: null` 保持合法（diff 空跳过 / 未跑，观察期），机械 checklist 第 3 条与 evaluator T3 null 语义不收紧——观察结果后再决定是否升格 error。
2. **agent 修复能力不进本期**：修阻塞错误（Step1b）与根因唯一性裁决（T4）不实现——红跑以 findings 摘要 + 报告呈现升格相位 fail，根因裁决交决策 agent（本变更进一步明确：相位内零修复会话）。
3. **net-zero 去噪不适用**：突变范围 = change 编辑集（`git diff HEAD` + untracked，`change_edit_set` 单点）；清单反推与 net-zero 去噪（CLI 先例）不移植——diff 面天然无「与 HEAD 同内容」噪音。
4. **无-change 场景不实现**：CLI `<root>/reports/test/` 分支桌面不实现（恒有 change）。
5. **报告不进 file inventory**：不做 artifacts registry 登记。

#### Scenario: 留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含 null 观察期与「相位内零修复会话、根因交决策」的明知接受），后续变更无需重新论证是否知情

### Requirement: 输出记录为脚本真实输出

test-execution 的输出记录 SHALL 由 summary JSON 重序列化改为**脚本真实输出**：runner 捕获的 `stdout_bytes + stderr_bytes` 合并流（不加 `2>&1`——rust 模板 `cargo llvm-cov nextest --workspace --json --output-path "{coverage_file}" --message-format libtest-json --message-format-version 0.1 --no-fail-fast > "{results_file}"` 已把 stdout（libtest-json 事件流）重定向进结果工件，`2>&1` 会让 stderr（人读报告 / 编译错误）混入污染 `parse_libtest_json`）。`PlanExecution` SHALL 增 `output` 字段承载该合并流（当前 stdout_bytes / stderr_bytes 是局部变量、只进 error 尾部）；`mod.rs` 聚合各 suite `output` 按 plan 序（字典序稳定）拼接，经 `record_exec_output(root, change, merged)` 落单一 `.log` 并回填 `exec_id`。全量结构化报告（`summary.json` + 逐 suite 子报告）仍留 report_dir 子树，各框架 report 工件各自为政，MUST NOT 强行统一各框架脚本输出格式。

**plan 分隔头（本变更新增，元数据面）**：`mod.rs` 聚合时 SHALL 在每个**已执行** plan 的 `output` 之前写入一行
`=== plan {id} ({framework}) · files={n} · argv={len} 字符 ===`（plan 序稳定、以换行起手与收尾）；该 plan 发生 `{files}` 省略降级时 SHALL 紧随一行 `[argv] {降级说明}`（展开长度 / 平台预算 / 文件数 / 已降级为框架自发现）。分隔头与 `[argv]` 行为**记录元数据**，MUST NOT 被视为脚本输出，故不与本条首句「内容为脚本真实输出」冲突：去掉这些行后，记录正文 MUST 与各 suite 脚本输出逐字节一致、按 plan 序保序。零 plan 执行的复用分支记录仍为空。降级说明 MUST NOT 进入 `SubReport.findings` / `summary.findings`（`collect_problems` 会把「`failed == 0` 且 findings 非空」判成 `execution_error`，绿跑降级会因此翻红——信息面单点在本条记录头）。

#### Scenario: rust 套件不加 2>&1

- **WHEN** test-execution 执行 rust 套件（模板为单条 `cargo llvm-cov nextest … --message-format libtest-json > "{results_file}"`）
- **THEN** spawn 不施加 `2>&1`（stdout 仍重定向进结果工件供 `parse_libtest_json`）；record 内容为 stderr（人读 PASS/Summary 行与编译错误面），`results.txt` 在报告树可见，不强行 tee

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

### Requirement: MutationTest 确定性门禁步与时序

Walker SHALL 在 test-execution 相位以独立 MutationTest 门禁步（`ToolCommand::MutationTest`，经 `MutationRunner` port 由 checks 边界 infra 实现）承接突变测试，步序列拆为 `PhaseStart → TestExecution → MutationTest → PhaseLog`：TestExecution 结论 pass（测试 / 覆盖率修绿）后才执行 MutationTest——「修复后再执行突变」由 walker 顺序执行天然保证；TestExecution fail / error 首败即升格（见「失败即升格交决策（零 agent 修复）」），相位内 MUST NOT 修复重入，只有决策 backtrack 重入并真重跑（复用门因输入变化失效）后才可能走到 MutationTest。MutationTest 绿跑零 WorkerAgent 会话；结论 pass 时相位以机械 checklist 代写 `phase_log`（verdict=pass）推进；中断（段执行 error / 超时）与未达标两时点均以升格收场（失败步 `on_fail=Escalate`，零会话，见 desktop-mutation-testing「失败升格交决策（报告指引式信息面）」）。runner 以 `Err` 收场（突变程序不可达 / 配置不支持 / 写盘失败）SHALL 停给用户（显式失败，与 WorkerAgent 失败同等呈现）。MutationTest 载荷 `ToolStepOutput::MutationTest` SHALL 为 conclusion + score / threshold + 计数 + 报告路径清单 + 反馈文本，载荷零 Serialize 不进 IPC（与 TestExecution 同式）；`ChangeStepKind::MutationTest` SHALL 入步词汇封闭集（18→19），经 `RunUpdate::Step` 上图（前端流程视图现有步骤渲染直接吃，零渲染改动）。

#### Scenario: 绿后才跑突变

- **WHEN** TestExecution 首次 fail（升格交决策）后经决策 backtrack 修复重入并 pass，walker 走到 MutationTest
- **THEN** MutationTest 在 TestExecution pass 后才执行（决策等待与修复期零突变 spawn）；MutationTest 节点随 `RunUpdate::Step` 上图；重入的 TestExecution 节点为独立 attempt / `instance_no`

#### Scenario: runner 显式失败停给用户

- **WHEN** cargo-mutants / npx stryker 程序不可达或配置不支持
- **THEN** runner 显式 `Err`，图上 MutationTest 节点呈失败态，run 不静默越过、不产出部分结论

### Requirement: 机械 checklist 双槽与 evidence 三态

PhaseLog 机械 checklist 产物面 SHALL 挂 `test_outcome` + `mutation_outcome` 双槽（两步结论独立成槽）。机械 checklist 第 3 条「mutation null 自动通过」判定语义 SHALL 保留（null 观察期不收紧，与 evaluator T3 一致），但 evidence SHALL 区分三态：「跑过且达标（score / threshold）/ diff 空跳过（突变面为空）/ null（未跑）」。`mutation: null` SHALL 保持合法（diff 空跳过 / 未跑场景不判 fail）。

#### Scenario: 三态 evidence 落账

- **WHEN** MutationTest 跑过且达标后 PhaseLog 代写机械 checklist
- **THEN** 第 3 条 pass、evidence 为「跑过且达标（score X ≥ threshold Y）」形态；diff 空跳过场景 evidence 为「diff 空跳过」、null 场景为「null（未跑）」，三态可辨

#### Scenario: null 合法不判 fail

- **WHEN** 突变面为空（change 编辑集不含可突变文件）或 MutationTest 未执行
- **THEN** summary mutation 位为 null，机械 checklist 第 3 条 pass（观察期语义不变），相位照常推进

### Requirement: baseline 报告槽位与树隔离

initialize 相位（desktop-change-orchestration「initialize 基线检查相位执行体」）的 test-execution 基线检查 SHALL 与 run 内 test-execution 相位的报告**分树隔离**：基线检查的 summary 与逐 suite 子报告 SHALL 落该 change 目录的 `reports/baseline` 子树（路径经 foundation layout 的 baseline 变体单点推导，见 desktop-checks-domain「baseline 报告槽位参数化」），run 内 test-execution 相位报告仍恒落 `reports/test`（零改动）——两树零混居，MUST NOT 以 exec_id / 时间戳在同目录内区分。基线检查 MUST NOT 执行 mutation（initialize 步序列无 MutationTest 步，summary 的 `mutation` 位不适用）。

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
- **THEN** `read_exec_output` 命中 baseline 树的 exec-outputs 并返回脚本真实输出，miss 仍为 `None` 非错误

### Requirement: rust suite 单构建（一条命令同产结果与覆盖率）

Test-execution 的 rust 档 SHALL 以**单条命令**执行测试并采集覆盖率：注册表 shell 臂与 cmd 臂模板 SHALL 逐字同串，形如 `cargo llvm-cov nextest --workspace --json --output-path "{coverage_file}" --message-format libtest-json --message-format-version 0.1 --no-fail-fast > "{results_file}"`。结果工件 SHALL 为 nextest 的 **libtest-json 事件流**（`--message-format libtest-json` + `--message-format-version 0.1` 钉格式；`--no-fail-fast` 使红跑产出全量用例集）——nextest 的人读报告全部走 stderr、stdout 只有结构化事件，故 MUST NOT 依赖「stdout 里有人读文本」。实验特性开关 `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1` SHALL 经注册表条目 `env` 字段声明、由 runner 经 `RunOptions.env` 施加（MUST NOT 依赖调用者 shell 预置，也 MUST NOT 写进模板串——cmd / sh 的前缀赋值语法不同会让两臂分叉）；缺该变量时 nextest 以 exit 95 明确报错，经 `execution_error` 定型为可诊断失败，MUST NOT 退化成空工件的假绿。runner MUST NOT 对展开后的命令串做任何拼装改写——rust 档 MUST NOT 残留 `; _X=$?; … ; exit $_X` 形态的退出码搬运、MUST NOT 残留 `&`（cmd 无条件顺序分隔符）链式分隔、MUST NOT 由 runner 侧插入结果重定向。suite 退出码 SHALL 直继承该单条命令的退出码（MUST NOT 被链中后段命令洗 0）：`cargo llvm-cov` 失败（测试红 / 覆盖率工件未产出 / 环境文件锁）即 suite 非零退出，经 `execution_error` 定型为 `conclusion=Error`——「覆盖率工件缺失而结论 pass」的假绿路径 MUST NOT 可达。stdout SHALL 全量重定向进 `{results_file}`，由 `parse_libtest_json` 单一读取（逐行扫 `{"type":"test"}` 终态事件：`ok` → passed、`failed` → failed、`ignored` → skipped，`exec_time` 秒换算毫秒；`started` 与 `suite` 行不产用例；`--output-path` 使 coverage JSON 不进 stdout），覆盖率采集范围 SHALL 为 cargo workspace 全成员（`--workspace`）——结果工件与覆盖率工件 SHALL 由同一批用例产出，MUST NOT 出现「门禁判定的覆盖率数字来自与门禁测试集不同的一批用例」。test-execution 的 rust 档 MUST NOT 写 / 清 `target/debug`（llvm-cov 走 `target/llvm-cov-target`），跨相位（static-check 的 `cargo clippy` / `cargo run`）的 target 清理冲突 MUST NOT 由本相位引入。覆盖率口径与阈值 SHALL 零调整：沿现网形态（不排除 `*_test.rs`）、阈值保持 lines 90 / branches 85 / functions 90，`--workspace` 带来的分母扩大由真件回归证据承接；口径收紧（排除 `*_test.rs`）MUST NOT 在本变更内发生。

#### Scenario: 单条命令两臂同串

- **WHEN** 审查 rust 档注册表条目与 runner 展开结果
- **THEN** shell / cmd 模板逐字同串且为单条 `cargo llvm-cov nextest --workspace --json --output-path "{coverage_file}" --message-format libtest-json --message-format-version 0.1 --no-fail-fast > "{results_file}"`；命令串不含 `cargo test`、不含 ` & `、不含 `; _X=$?`；runner 展开后的命令与模板展开逐字一致（零追加改写）；`NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1` 只在注册表 `env` 字段出现，模板串内零出现

#### Scenario: 退出码直继承不假绿

- **WHEN** `cargo llvm-cov` 因环境文件锁 / 覆盖率工件未产出以非零退出收场，而同批测试用例本身全绿
- **THEN** suite `exit_code` 非零 → summary `problems` 记 `execution_error`（携命令输出尾段）、`conclusion=Error`；MUST NOT 判 `pass`、MUST NOT 静默降级为「覆盖面无数据但通过」

#### Scenario: 覆盖率与结果同批用例产出

- **WHEN** rust 档真件执行完毕（工作区含 `crates/**` 多成员加根包）
- **THEN** `results.txt` 的 libtest-json 事件覆盖全 workspace 成员（子报告 `summary.total` 与 nextest 汇总的 `tests run` 同量级）、`coverage-summary.json` 覆盖文件数 ≥ 200（含 `crates/core` / `crates/infra` / `crates/tools` / `src-tauri/src`），二者由同一次命令产出；子报告 `source_files` 不再局限于根包 25 个文件

#### Scenario: 不触碰 target/debug

- **WHEN** 同一 run 内 static-check 相位与本相位先后执行（前者 `cargo clippy` / `cargo run` 写 `target/debug`）
- **THEN** 本相位对 `target/debug` 零写入零清理（覆盖率构建落 `target/llvm-cov-target`），跨相位 `failed to remove …\target\debug\deps\*.rcgu.o (os error 32)` 冲突形态不可达

#### Scenario: 阈值零调整达标与红跑覆盖缺失

- **WHEN** 真件回归聚合 summary，及测试红跑时 llvm-cov 不产覆盖率工件两种情形
- **THEN** 前者 lines / functions 均达阈值（实测 96.64 % / 90.22 %）、conclusion=pass；后者子报告 `coverage=null`、覆盖维度按 null 感知跳过，结论仍由 `failed>0` / `execution_error` 定型为 fail / error——两情形均不产生假绿

### Requirement: 结果工件零用例即执行错误

结果工件解析 SHALL 区分三种成品形态：工件缺失、工件为空（零字节 / 纯空白）、工件在场但**产不出任何用例行**。三者 MUST NOT 落入 pass：

- 工件缺失 → `结果工件缺失: {name}`；
- 工件为空 → `结果工件为空: {name}`；
- 解析成功但用例集为空 → `结果工件零用例: {name}`；报告器侧完全读不出事件行（如纯噪声）时由解析器自身 `Err`（`不含任何事件行`）。

三条 finding 均进 `SubReport.findings` → `collect_problems` 判 `ExecutionError` → `conclusion=Error`。理由：工件在场而零用例是报告器参数漂移 / 写空 / 格式错配的强信号，与「真的没有用例」不可区分，此时判 pass 即假绿；rust 与 node-test 两档已由本守卫覆盖，istanbul 族（jest / vitest / vite-plus）结果工件零字节同样走「工件为空」臂。

#### Scenario: 空工件与零用例分别定型

- **WHEN** rust 档命令退出 0，但 `results.txt` 分别是零字节、只含 `suite` 汇总行、只含 `started` 行、或纯噪声
- **THEN** 前两种退化为「结果工件为空」/「结果工件零用例」finding，后一种（纯噪声）由 `parse_libtest_json` 返回 `不含任何事件行`；四种情形 `conclusion=Error`、`cases` 为空，MUST NOT 判 pass

#### Scenario: 正常事件流不受守卫误伤

- **WHEN** 结果工件含至少一条 `{"type":"test","event":"ok|failed|ignored"}` 事件
- **THEN** 解析产用例行、`error` 面为空，守卫不介入（`failed > 0` 由既有 `test_failure` 归因定型）

### Requirement: suite 执行超时预算与取消

test-execution suite 命令的执行 SHALL 经 `exec::run_shell`（见 desktop-exec-runtime），并把 suite 命令预算常量 `SUITE_COMMAND_TIMEOUT` 由 **600 s 提升到 1200 s**（取值口径：rust suite 实测 340–393 s 的约 3 倍余量；原 600 s 仅 1.7 倍，热态正常、冷态或多 change 并发即触线）。

- **瞬态重试**：suite 命令 SHALL 以 `RetryPolicy::transient()` 执行（见 desktop-exec-runtime「瞬态签名字典与有限重试策略」——构建产物被安全软件占用时的 `os error 32` / `failed to rename archive file` 等签名）——重试利用 cargo 增量进度自愈（已建成 crate 不重建，剩余待写文件递减）；`TimedOut` / `Cancelled` 零重试。
- **超时语义（本变更改判为执行面归因）**：`RunOutcome::TimedOut` SHALL 映射为 `fault = Some(RunFault::Environment("测试命令超时（1200 秒）强制终止"))`，`exit_code` 仍取 `EXIT_CODE_NO_STATUS`（`-1`）哨兵、`output` 为空；该 plan MUST NOT 记 `execution_error`（超时不是被测代码问题），summary MUST NOT 因此产 `conclusion = error` 的假失败归因；MUST NOT 因超时改判 pass。
- **瞬态耗尽**：重试耗尽且末次仍命中瞬态签名 → `fault = Some(RunFault::Environment(…))`（文本含占用者点名 / 被锁文件 / 已重试次数 / 处理指引）；该 plan MUST NOT 记 `execution_error`（同上，环境面不落「被测代码问题」归因）。
- **取消语义（本变更改判）**：`cancel: CancelFlag` 由 port 透传；`RunOutcome::Cancelled` SHALL 使 runner 以 `fault = Some(RunFault::Stopped)` 正常返回（MUST NOT 返回 `Err("执行已取消")`），MUST NOT 落为 suite 级 `execution_error`、MUST NOT 继续跑余下 plan。
- **`conclusion = Error` 收窄**：成因面 SHALL 仅限「结果工件解析失败 / harness 崩溃 / 框架探测失败 / 程序不可达及配置不支持（后者仍为 `Err`）」；超时与瞬态耗尽 MUST 由 fault 承载（fault 在位时 `conclusion` 值不参与门禁分派）。
- **合并流顺序零变**：`PlanExecution.output` 仍为「stdout 前缀 + stderr 后缀」；重试场景下 SHALL 为各次尝试按序拼接、每段前加分隔头 `===== attempt N/5 · exit=C · transient=<命中签名> =====`（审计不丢上下文）；MUST NOT 加 `2>&1`（会污染 `parse_libtest_json` 的结果工件解析面）。
- 私有管道排水助手（`read_all`）SHALL 随迁移退役，管道排水单点驻 `exec`。

#### Scenario: 超时预算常量锚定

- **WHEN** 审查 `runner.rs` 的 `SUITE_COMMAND_TIMEOUT`
- **THEN** 值为 1200 s，且单元测试以常量断言钉住（原「与 CLI `runCommand(cmd, cwd, 600000)` 同值」的锚定口径退役，改为「实测 340–393 s × 3 倍余量」）

#### Scenario: 超时入 fault 而不再落 execution_error

- **WHEN** suite 命令超过预算未退出
- **THEN** 该 plan 的 `exit_code == -1`、`output` 为空、`fault == Some(Environment(超时文案))`，`error` 面不含「被测代码」归因、summary 不记 `execution_error`；`TestExecutionOutcome.fault` 上抛，`conclusion` 值不构成门禁结论

#### Scenario: 瞬态耗尽入 fault

- **WHEN** 某 suite 命令连续 5 次以锁文本退出 1（重试耗尽）
- **THEN** `fault == Some(Environment(…))` 且文本含占用者点名与已重试次数；`PlanExecution.output` 含 5 段 attempt 分隔头的输出；summary 不记 `execution_error`

#### Scenario: 取消返回 fault 而非 Err

- **WHEN** 用户停止 run 期间 suite 命令正在执行
- **THEN** 执行体终止进程树并返回 `Cancelled`，runner 以 `fault = Some(Stopped)` 正常返回；MUST NOT 落为某 plan 的 `execution_error`，MUST NOT 继续跑余下 plan，MUST NOT 返回 `Err`

#### Scenario: 真红零变

- **WHEN** suite 命令以真实测试失败退出（输出无瞬态签名）
- **THEN** `attempts == 1`、`fault == None`、`conclusion == Fail`，既有 `execution_error` / 用例行解析链与升格路径逐字不变

#### Scenario: 合并流与工件解析不受影响

- **WHEN** rust 档模板把 stdout 重定向进结果工件、编译错误留在 stderr
- **THEN** `PlanExecution.output` 仍为 stdout + stderr 前后拼接（编译错误面进 record 可回查），结果工件解析仍只读 stdout 面；解析器零改动

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
| `crates/core/orchestration/src/port.rs` | port 契约三件套 | `ToolCommand::TestExecution { change, … }` / `ToolStepOutput::TestExecution(载荷双面：摘要 + 明细文本)` / `TestExecutionRunner` port（W4：spawn 不进 core）；载荷类型定义于 port 本地 |
| `crates/core/orchestration/src/state.rs` | 步词汇扩展 | `ChangeStepKind::TestExecution` 入封闭集；`ChangeStepState` / `RunUpdate::Step` 形态不变（白名单式扩展） |
| `crates/core/orchestration/src/walker.rs` | 门禁步接入 | test-execution 相位收敛点执行 runner；`Escalate` 前置分支（零会话落账 → `StepFlow::Upgraded` → 回 `phase_next` 交决策），相位内零反馈边（原独立计数器与上限 5 预算删除） |
| `crates/core/checks`（aggregate / diagnose / reuse） | 纯层判定 | 聚合 conclusion、完整性校验 + 诊断树确定性分支、复用门 mtime 判定；fixtures + corpus 黄金可测 |
| `crates/infra/checks/src/testexec/` | 进程执行 | detect（五框架注册表 + glob）/ runner（模板展开 + spawn + 超时 + 捕获）/ report（summary + 子报告写盘）；bun / go / pytest 显式 Err |
| `crates/core/foundation/src/layout/mod.rs` | 报告路径推导 | change 报告目录（reports/test）常量组扩展；命名隔离双禁令唯一触点纪律不变 |
| `src/types/generated/bindings.ts`（重导） | IPC 类型跟随 | ChangeStepKind 新变体出线；`bindings:check` 守卫拦截过期生成物 |
| `crates/infra/checks/src/testexec/mod.rs` | 报告槽位参数化消费 | 报告目录经槽位（test 缺省 / baseline）派生；复用门按各自报告树判定（baseline 树内 mtime 判定复用，跨树零复用）；聚合与诊断链零改动 |
| `crates/core/foundation/src/layout/mod.rs` | baseline 目录变体单点 | `reports/baseline` 后缀与 `reports/test` 同源派生（root / change 目录两变体）；零磁盘路径字面量扩散 |
| `crates/infra/exec/src/lib.rs` | 输出记录同锚 | `record_exec_output` / `read_exec_output` 按槽位锚定 exec-outputs；exec_id 跨树唯一，回查覆盖两树 |
| `crates/core/workflow/src/state.rs` | `OnFail` 变体面 | `OnFail::{ FeedbackLoop { limit }, Terminal, Escalate }`（`Escalate` 无载荷，语义 = 落账后交决策） |
| `crates/core/workflow/src/write/phase_table.rs` | 步声明单源 | `TEST_EXECUTION_STEPS` 两步 `on_fail = Some(OnFail::Escalate)`（零 agent、零反馈边） |
| `crates/core/orchestration/src/walker.rs`（`step_test_execution` / `step_mutation_test`） | 门禁步执行体 | 删 `loop { run_tool; run_worker }`；`Escalate` 前置分支（保留 `Terminal` 前置分支与 runner `Err → Terminal`）；零 WorkerAgent 请求 |
| `crates/core/orchestration/src/walker.rs`（升格落账） | 机械 fail 落账 | `report` = 完整明细（≤2000）、`checklist` 一条 `pass=false`、evidence = `diagnose_brief` + `report_dir`、三会话槽位 None |
| `crates/core/orchestration/src/port.rs` | 载荷语义（字段面不变） | `TestExecutionOutcome.findings_detail` / `MutationTestOutcome.feedback_text` 消费者由「反馈边修复 prompt」改为「升格 report」 |
| `crates/core/orchestration/src/decision.rs` + `prompt.rs` | 决策输入面 | `DecisionInput.fail_report: String`（取升格条目 report）；决策 prompt 增失败报告段与读报告文件指引 |
| `crates/infra/checks/src/testexec/detect.rs` | rust 档注册表模板单源 | `shell_template` / `cmd_template` 逐字同串 = `cargo llvm-cov nextest --workspace --json --output-path "{coverage_file}" --message-format libtest-json --message-format-version 0.1 --no-fail-fast > "{results_file}"`；`env = [("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1")]`（其余四档空）；`coverage_format = LlvmCov`、工件名 `coverage-summary.json` / `results.txt` 不变；注释留痕「rust 档单条命令有意偏离 CLI 双段链」 |
| `crates/infra/checks/src/testexec/runner.rs` | 进程执行（零注入） | 删 `applyResultsRedirect` 分支与 `native_output_file` 助手——命令串展开后零改写；spawn / 超时 / 输出捕获 / `PlanExecution.output` 合并流 / 工件解析面不变 |
| `crates/infra/checks/src/testexec/`（detect / runner / report） | 五框架注册表与执行链 | 其余四档模板（jest / vitest / vite-plus / node-test）零改动；`{report_dir}` / `{results_file}` / `{coverage_file}` / `{config_args}` / `{files}` 占位符语义零改动；`mod.rs` 报告组装与复用门零改动 |
| `crates/core/checks`（parser / aggregate） | 纯层判定 | `parse_libtest_json`（libtest-json 事件流 → 用例行；坏行跳过、零事件行 `Err`）、`parse_llvm_cov` 逐文件条目、`coverage_meets_thresholds` 的 null 维度感知、`compute_overrides` 语义均不变——本变更只换工件产出方，不换判定口径 |
| `crates/infra/checks/src/testexec/runner.rs` | suite 执行与归因 | `SUITE_COMMAND_TIMEOUT: Duration = 1200 s`；`execute_plan(plan, report_dir, cancel)` 经 `run_shell`（`timeout` / `cancel` / `retry = RetryPolicy::transient()`）；`TimedOut` / 瞬态耗尽 → `fault = Environment(…)`（不产 plan `execution_error`）；`Cancelled` → `fault = Stopped`；`PlanExecution { …, fault: Option<RunFault> }`；尝试分隔头拼接 |
| `crates/infra/checks/src/testexec/mod.rs` | runner port 实现 | `impl TestExecutionRunner for ProcessTestExecution`：`run(root, name, slot, cancel)` 签名加 `cancel: CancelFlag` 并透传 `execute_plan`；多 plan fault 聚合（`Stopped` 优先于 `Environment`）；`TestExecutionOutcome.fault`；`conclusion = Error` 成因面收窄 |
| `crates/core/orchestration/src/port.rs` | 载荷 | `TestExecutionOutcome { conclusion, total, passed, failed, skipped, findings_brief, findings_detail, report_dir, exec_id, fault: Option<RunFault> }` |
| `crates/core/orchestration/src/walker.rs` | 步分派 | fault 前置分支（见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」）；`conclusion` 三分支仅在 `fault == None` 时命中 |
| `crates/core/orchestration/src/port.rs` | port 契约 | `TestExecutionRunner::run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture` |
| `crates/core/checks` | 纯层 | 零改动（解析 / 聚合 / 复用门 / 阈值判定不变） |
| `crates/core/checks`（解析与聚合） | 报告语义权威 | 唯一权威源；金样（`tests/golden` + `tests/fixtures`）为冻结比对面 |
| `crates/core/checks/tests/corpus_golden_test.rs` | 金样对拍载体 | 头注释与断言语改为「已固化 Rust 实现 + corpus 金样」权威口径；行为零改 |
| `crates/core/checks/tests/fixtures/README.md` | 语料说明 | 去「CLI zod schema 形态」溯源措辞（语料本身原样保留，含旧路径样本） |
| `crates/infra/checks/src/testexec/runner.rs` | 预算裁决与展开 | `const CMD_LINE_BUDGET: usize = 7000`（`cfg(windows)`）+ `line_budget() -> Option<usize>`；`expand_template(template, report_dir, results_file, coverage_file, config_args, files, expand_mode: ExpandMode)`（`ExpandMode::{WithFiles, OmitFiles}`——`OmitFiles` 剥 `{files}` 前导空白，其余占位符同义）；纯函数 `decide_argv(...) -> ArgvDecision::{Keep{command}, Omit{command, note}, Reject{message}}`；`execute_plan` 在 `expand_template` 之后、`ensure_program_resolvable` 之前裁决；`PlanExecution { …, argv_len: usize, argv_note: Option<String> }` |
| `crates/infra/checks/src/testexec/detect.rs` | 降级安全判据 | `TestPlan.omit_files_safe: bool`——`default_glob` 集合（同一剪枝管线、**不施加 suite excludes**）== `plan.files` 集合；glob 方言与枚举逻辑零新增（复用 `detect_files` 既有函数） |
| `crates/infra/checks/src/testexec/mod.rs` | 记录头装配 | 逐 plan 写 `=== plan {id} ({framework}) · files={n} · argv={len} 字符 ===`（降级再补 `[argv] {note}`），plan 序稳定；复用分支（零 plan）零改动；`build_sub_report` MUST NOT 注入 `argv_note` 到 `findings` |
| `crates/infra/checks/src/testexec/runner.rs` | 归因出口 | 拒绝降级 → `fault = Some(RunFault::Environment(environment::argv_budget_text(…)))`，`error` 恒 `None`、工件解析跳过（沿既有 fault 早返分支） |
