# desktop-test-execution Specification

## Purpose

Desktop 后端 test-execution 相位确定性门禁步：walker 经 `TestExecutionRunner` port（checks 边界 `crates/infra/checks` 实现）执行 jest / vitest / vite-plus / rust / node-test 五框架测试矩阵——绿跑零 WorkerAgent 会话、fail / error 走独立预算定向反馈边；报告恒落 change 报告目录、复用门幂等、结论与 CLI zod 权威 schema corpus 对拍。

## Requirements

### Requirement: test-execution 确定性门禁步与绿跑零 agent

Walker SHALL 在 test-execution 相位收敛点执行确定性 test-execution 门禁步（`ToolCommand::TestExecution`，经 `TestExecutionRunner` port 由 checks 边界 infra 实现；对齐插件工作流的 test-execution 独立相位语义）。结论 pass 时相位 SHALL 以机械 eval checklist 代写 `phase_log`（verdict=pass）推进，全程 MUST NOT 发起任何 WorkerAgent 会话（插件工作流 executor agent 驱动 CLI 子命令的路径桌面不采用）；结论 fail / error 时 SHALL 走「失败即升格交决策（零 agent 修复）」requirement 的升格路径——首败即落账交决策，MUST NOT 在相位内发起修复会话。runner 以 `Err` 收场（程序不可达 / 配置不支持 / 写盘失败）SHALL 停给用户（显式失败，与 WorkerAgent 失败同等呈现）。

#### Scenario: 绿跑零 agent

- **WHEN** walker 走到 test-execution 相位且全部 suite 通过、覆盖率达阈值
- **THEN** 零 WorkerAgent 会话发起（该步 run 节点行 `session_id` 为空、无 provenance `source_ref` 为 `<change-id>/test-execution/<role>/<attempt>` 的会话记录），机械 checklist 代写 `phase_log` 落账 pass，相位推进

#### Scenario: runner 显式失败停给用户

- **WHEN** runner 以 `Err` 收场（测试程序不可达 / 配置声明不支持框架）
- **THEN** 图上 TestExecution 节点呈失败态（与 executor 会话失败同等可观测），run 不静默越过该节点、不产出部分结论

### Requirement: 框架矩阵与显式报错

Test-execution SHALL 支持 jest / vitest / vite-plus（istanbul 族，`coverage-summary.json`）/ rust（cargo test + llvm-cov）/ node-test（`--test-reporter=spec` 文本结果）五框架的探测与执行；text 解析器 SHALL 以 node-test / rust 文本分支进移植范围。tests[] 声明 bun / go / pytest suite 时 runner SHALL 显式 `Err`（框架不被支持），MUST NOT 静默跳过该 suite；go-parser / coverage-py 不移植。

#### Scenario: 五框架探测产出 plan

- **WHEN** tests[] 声明五框架 suite（含 includes / excludes glob）
- **THEN** detect 逐 suite 产出 plan entry（root / cwd / id / files），排除过滤生效

#### Scenario: 不支持框架显式报错

- **WHEN** tests[] 声明 `framework: bun`（或 go / pytest）
- **THEN** runner 显式 `Err` 指明框架不被支持，不产出部分结论、不静默跳过

### Requirement: 执行链与报告落盘

执行链 SHALL 依序：detect（config tests[] + glob → plan entries）→ 完整性校验（字段 / 计数 / conclusion 一致性 checklist）→ 复用门 → 逐 suite 命令模板展开（`{results_file}` / `{report_dir}` / `{config_args}` / `{files}`）spawn（超时 + 输出捕获）→ 报告写盘 → 聚合（阈值 / override → conclusion）→ 诊断 findings。报告 SHALL 恒落 change 报告目录（`summary.json` + 逐 suite 子报告；路径经 foundation layout 推导，desktop 产品源码零磁盘路径字面量）；报告 MUST NOT 登记 file inventory / artifacts registry（MVP 口径）。突变面 SHALL 由独立 MutationTest 门禁步承载（见「MutationTest 确定性门禁步与时序」与 desktop-mutation-testing）：TestExecution 执行链不产突变结果；summary 的 `mutation` 位由 MutationTest 归并块回填（在位时），未跑 / diff 空跳过时保持 null（null 合法，见「机械 checklist 双槽与 evidence 三态」）。

#### Scenario: 全链绿跑落盘

- **WHEN** 全部 suite 执行成功且覆盖率达阈值
- **THEN** summary.json 与逐 suite 子报告落 change 报告目录，聚合 conclusion=pass，机械 checklist 可静态判定

#### Scenario: 报告路径经 layout 推导

- **WHEN** runner 写盘且对 desktop 产品源码做命名隔离扫描
- **THEN** 报告目录取自 foundation layout 推导（`reports/test` 挂 change 目录），零硬编码磁盘路径字面量，双禁令扫描通过

#### Scenario: 突变面独立步承载

- **WHEN** TestExecution 全链绿跑完成进入 MutationTest
- **THEN** TestExecution 产出不含突变结果（mutation 位由 MutationTest 归并回填或保持 null），两步报告各落其位

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

报告 schema 与聚合判定 SHALL 以 CLI zod schema（`plugins/dev-team/bin/src/schemas`）为权威对齐源；SHALL 以真实工作区产出的 summary.json / report.json 为 fixtures 建立 corpus 黄金测试（`core/workflow` corpus_golden_test 先例同款），对拍解析与聚合结论——桌面 Rust 实现与 CLI 实现对同一报告输入 MUST NOT 得出不一致结论。

#### Scenario: corpus 对拍拦截漂移

- **WHEN** 以真实 fixtures 驱动 core/checks 解析与聚合
- **THEN** 字段 / 计数 / conclusion 与 CLI zod 权威一致；schema 或聚合语义漂移被测试显式指认

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

test-execution 的输出记录 SHALL 由 summary JSON 重序列化改为**脚本真实输出**：runner 捕获的 `stdout_bytes + stderr_bytes` 合并流（不加 `2>&1`——rust 模板 `cargo test > "{results_file}"` 已把 stdout 重定向进结果工件，`2>&1` 会让 stderr（编译错误）混入污染 `parse_cargo_test`）。`PlanExecution` SHALL 增 `output` 字段承载该合并流（当前 stdout_bytes / stderr_bytes 是局部变量、只进 error 尾部）；`mod.rs` 聚合各 suite `output` 按 plan 序（字典序稳定）拼接，经 `record_exec_output(root, change, merged)` 落单一 `.log` 并回填 `exec_id`。全量结构化报告（`summary.json` + 逐 suite 子报告）仍留 report_dir 子树，各框架 report 工件各自为政，MUST NOT 强行统一各框架脚本输出格式。

#### Scenario: rust 套件不加 2>&1

- **WHEN** test-execution 执行 rust 套件（模板为 `cargo test > "{results_file}"`）
- **THEN** spawn 不施加 `2>&1`（stdout 仍重定向进结果工件供 `parse_cargo_test`）；record 内容主要为 stderr（编译错误面），`results.txt` 在报告树可见，不强行 tee

#### Scenario: 各 suite 输出按 plan 序聚合

- **WHEN** 一次 test-execution 含多个 suite（如 jest 与 rust），各 suite 脚本分别产出 stdout / stderr
- **THEN** record 落单一 `.log`，内容为各 suite 合并输出按 plan 序拼接；`read_exec_output` 回查内容与脚本真实输出一致，MUST NOT 是 summary JSON 重序列化

#### Scenario: 结构化报告仍为全量权威

- **WHEN** record 改为脚本输出后 fail / error 收场
- **THEN** `summary.json` 与逐 suite 子报告仍落 report_dir 子树（全量结构化结论权威），`findings_detail` / 报告目录兜底语义不变，record 只作人工回查的终端还原

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
