# desktop-test-execution Specification

## Purpose

Desktop 后端 test-execution 相位确定性门禁步：walker 经 `TestExecutionRunner` port（checks 边界 `crates/infra/checks` 实现）执行 jest / vitest / vite-plus / rust / node-test 五框架测试矩阵——绿跑零 WorkerAgent 会话、fail / error 走独立预算定向反馈边；报告恒落 change 报告目录、复用门幂等、结论与 CLI zod 权威 schema corpus 对拍。

## Requirements

### Requirement: test-execution 确定性门禁步与绿跑零 agent

Walker SHALL 在 test-execution 相位收敛点执行确定性 test-execution 门禁步（`ToolCommand::TestExecution`，经 `TestExecutionRunner` port 由 checks 边界 infra 实现；对齐插件工作流的 test-execution 独立相位语义）。结论 pass 时相位 SHALL 以机械 eval checklist 代写 `phase_log`（verdict=pass）推进，全程 MUST NOT 发起任何 WorkerAgent 会话（插件工作流 executor agent 驱动 CLI 子命令的路径桌面不采用）；结论 fail / error 时走「结论反馈边与独立预算」requirement 的反馈边。runner 以 `Err` 收场（程序不可达 / 配置不支持 / 写盘失败）SHALL 停给用户（显式失败，与 WorkerAgent 失败同等呈现）。

#### Scenario: 绿跑零 agent

- **WHEN** walker 走到 test-execution 相位且全部 suite 通过、覆盖率达阈值
- **THEN** 零 WorkerAgent 会话发起（无 `source_ref` 为 `<change>/test-execution/<role>/<attempt>` 的会话记录），机械 checklist 代写 `phase_log` 落账 pass，相位推进

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

执行链 SHALL 依序：detect（config tests[] + glob → plan entries）→ 完整性校验（字段 / 计数 / conclusion 一致性 checklist）→ 复用门 → 逐 suite 命令模板展开（`{results_file}` / `{report_dir}` / `{config_args}` / `{files}`）spawn（超时 + 输出捕获）→ 报告写盘 → 聚合（阈值 / override → conclusion）→ 诊断 findings。报告 SHALL 恒落 change 报告目录（`summary.json` + 逐 suite 子报告；路径经 foundation layout 推导，desktop 产品源码零磁盘路径字面量）；报告 MUST NOT 登记 file inventory / artifacts registry（MVP 口径）。mutation 报告位 SHALL 恒为 null（本 MVP 不移植 mutation）。

#### Scenario: 全链绿跑落盘

- **WHEN** 全部 suite 执行成功且覆盖率达阈值
- **THEN** summary.json 与逐 suite 子报告落 change 报告目录，聚合 conclusion=pass，机械 checklist 可静态判定

#### Scenario: 报告路径经 layout 推导

- **WHEN** runner 写盘且对 desktop 产品源码做命名隔离扫描
- **THEN** 报告目录取自 foundation layout 推导（`reports/test` 挂 change 目录），零硬编码磁盘路径字面量，双禁令扫描通过

### Requirement: 复用门幂等

复用门 SHALL 以输入新鲜度（mtime 递归比较）判定：输入未变 SHALL 复用上次 summary（零 spawn 零重跑、结论照旧），输入变化 SHALL 失效并全量重跑。复用门是 test-execution 相位自动化的幂等前提——自动触发 MUST NOT 演变为反复全量跑。

#### Scenario: 输入未变复用

- **WHEN** 同一 change 连续两次进入 test-execution 相位且工作区输入未变
- **THEN** 第二次复用上次 summary，零测试进程 spawn，结论一致

#### Scenario: 输入变化失效重跑

- **WHEN** 上次报告之后任一输入文件 mtime 更新
- **THEN** 复用门失效，全量重跑产出新 summary 与新结论

### Requirement: 结论反馈边与独立预算

结论 fail / error 时 walker SHALL 走定向反馈边（对齐 static-check Upgraded 语义）：将 findings 诊断**明细**注入 executor 会话（Continue）修复——明细为有界装配文本（诊断面 findings 全量、缺省回落 problems 消息面 + suite 执行概览（框架 / root / 退出码 / 计数 / 覆盖三维度对照、null 维度跳过）+ 失败用例明细（名称 / 文件行号 / 错误类型 / 消息 / 堆栈）），修复会话输入面自足、MUST NOT 依赖修复会话自读报告文件；报告目录随 prompt 兜底携带（全量权威所在）。修复重入后复用门自动失效重跑。反馈边 SHALL 独立计数、上限 5 次（与 static-check 反馈计数器分立，各自计满各自升格），MUST NOT 计入相位 retry 预算；超限 walker SHALL 以代写 fail checklist 经写面 `phase_log` 升格相位 fail（不跑 evaluator，进入重试 / 决策路径）。

#### Scenario: 反馈修复重入

- **WHEN** conclusion=fail 且反馈边未超限
- **THEN** findings 明细文本（诊断 + suite 概览 + 失败用例名 / 文件行号 / 错误消息）注入 executor 会话（Continue），反馈边计数 +1；修复重入后复用门失效并重跑

#### Scenario: 明细有界装配

- **WHEN** 失败面超大（长消息 / 长堆栈 / 大量失败用例）
- **THEN** 明细按界截断（单条消息 500 / 堆栈 800 字符、每 suite 20 条、总预算 12000 字符），截断面以省略号与余量记数示意，报告目录兜底行恒在场

#### Scenario: 超限升格相位 fail

- **WHEN** 同一相位内反馈边达 5 次仍 fail / error
- **THEN** walker 代写 fail checklist 落账升格相位 fail，不再注入反馈边；相位 retry 预算独立消耗（反馈边次数不计入）

### Requirement: 步骤可观测与最小载荷

`ChangeStepKind::TestExecution` SHALL 入步词汇封闭集，门禁步状态经 `RunUpdate::Step` 上图（前端流程视图现有步骤渲染直接吃，零渲染改动）；IPC 类型随 bindings 再生成出线。`ToolStepOutput::TestExecution` 载荷面 SHALL 为 conclusion + 计数 + 诊断摘要 + 修复边明细文本 + 报告路径：摘要面服务步状态 detail（`diagnose_brief` 口径截断），明细面服务反馈边修复 prompt 直嵌（有界装配、pass 态恒空串）。载荷 SHALL 零 Serialize 不进 IPC——明细文本 MUST NOT 经 bindings / `RunUpdate` 出线，只在进程内 walker 反馈边消费；全量 findings 留报告文件，MUST NOT 经 IPC 面全量透传。

#### Scenario: 步骤上图零渲染改动

- **WHEN** 门禁步状态变化且 bindings 再生成
- **THEN** `RunUpdate::Step` 携 testExecution 步词汇流出，前端流程视图以既有步骤渲染直接消费

#### Scenario: 载荷双面各司其职

- **WHEN** 审查 `ToolStepOutput::TestExecution` 载荷
- **THEN** 无结构化 findings 全量数组字段：摘要面（`findings_brief`）走步 detail，明细面（`findings_detail`，有界文本）走反馈边修复 prompt；两者均不进 bindings / IPC，全量以报告文件为权威（`report_dir` 定位）

### Requirement: 双实现对拍黄金语料

报告 schema 与聚合判定 SHALL 以 CLI zod schema（`plugins/dev-team/bin/src/schemas`）为权威对齐源；SHALL 以真实工作区产出的 summary.json / report.json 为 fixtures 建立 corpus 黄金测试（`core/workflow` corpus_golden_test 先例同款），对拍解析与聚合结论——桌面 Rust 实现与 CLI 实现对同一报告输入 MUST NOT 得出不一致结论。

#### Scenario: corpus 对拍拦截漂移

- **WHEN** 以真实 fixtures 驱动 core/checks 解析与聚合
- **THEN** 字段 / 计数 / conclusion 与 CLI zod 权威一致；schema 或聚合语义漂移被测试显式指认

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续切片偿还，不由实现隐式吸收）：

1. **mutation 不移植**：报告 `mutation: null` 合法（evaluator T3 null 自动通过）；本仓 desktop 档(80) / src-tauri rust 档(70) 开着 mutation，MVP 期桌面与 CLI 对同一 change 结论不一致为明知接受。
2. **agent 修复能力不进本期**：修阻塞错误（Step1b）与根因唯一性裁决（T4）不实现——红跑以 findings 呈现走反馈边或留给用户决策。
3. **net-zero 去噪后补**：突变范围 / net-zero 去噪不在本期。
4. **无-change 场景不实现**：CLI `<root>/reports/test/` 分支桌面不实现（恒有 change）。
5. **报告不进 file inventory**：不做 artifacts registry 登记。

#### Scenario: 留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含 dogfood mutation 口径差异的明知接受），后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/port.rs` | port 契约三件套 | `ToolCommand::TestExecution { change, … }` / `ToolStepOutput::TestExecution(载荷双面：摘要 + 修复边明细)` / `TestExecutionRunner` port（W4：spawn 不进 core）；载荷类型定义于 port 本地 |
| `crates/core/orchestration/src/state.rs` | 步词汇扩展 | `ChangeStepKind::TestExecution` 入封闭集；`ChangeStepState` / `RunUpdate::Step` 形态不变（白名单式扩展） |
| `crates/core/orchestration/src/walker.rs` | 门禁步接入 | test-execution 相位收敛点执行 runner；反馈边独立计数器（与 `STATIC_CHECK_FEEDBACK_LIMIT` 分立、上限 5）；超限代写 fail checklist 升格相位 fail |
| `crates/core/checks`（aggregate / diagnose / reuse） | 纯层判定 | 聚合 conclusion、完整性校验 + 诊断树确定性分支、复用门 mtime 判定；fixtures + corpus 黄金可测 |
| `crates/infra/checks/src/testexec/` | 进程执行 | detect（五框架注册表 + glob）/ runner（模板展开 + spawn + 超时 + 捕获）/ report（summary + 子报告写盘）；bun / go / pytest 显式 Err |
| `crates/core/foundation/src/layout/mod.rs` | 报告路径推导 | change 报告目录（reports/test）常量组扩展；命名隔离双禁令唯一触点纪律不变 |
| `src/types/generated/bindings.ts`（重导） | IPC 类型跟随 | ChangeStepKind 新变体出线；`bindings:check` 守卫拦截过期生成物 |
