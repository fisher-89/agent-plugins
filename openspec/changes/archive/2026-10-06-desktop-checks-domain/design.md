# 设计: desktop-checks-domain

> **变更**: desktop-checks-domain
> **日期**: 2026-10-06

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 报告 schema 模型 | SubReport / SummaryReport / coverage / mutation / problems / plans / source_files 族 serde 模型（逐字段对齐 CLI zod 权威 `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts`）+ 未知字段宽容解析入口 | `crates/core/checks/src/model.rs`（新） | serde, serde_json | Rust serde（非 IPC 面，零 specta） |
| 解析器族 | istanbul coverage-summary / llvm-cov JSON → 逐文件覆盖原始计数；node-test spec / rust 测试文本 → 用例行；按覆盖格式路由 | `crates/core/checks/src/parser/mod.rs`（新；`js.rs` / `rust.rs` / `coverage.rs` / `text.rs` 四子模块同目录） | model | 纯函数（文本入、模型出） |
| 聚合判定 | 阈值与 per-suite override 聚合、conclusion 判定、summary 报告组装（CLI `test-report.ts` 的 `determineConclusion` / `computeOverrides` 同语义） | `crates/core/checks/src/aggregate.rs`（新） | model, config(TestSuite), glob(Pattern) | 纯函数 |
| 完整性校验与诊断树 | 完整性 checklist（字段 / 计数 / conclusion 一致性）+ 诊断树确定性分支产出 findings（阈值比对 + null 维度感知 + 失败聚类，对齐 CLI executor 定义 4a/4c/4d/4e/4f） | `crates/core/checks/src/diagnose.rs`（新） | model | 纯函数 |
| 复用门判定 | mtime 新鲜度纯比较（结论非 error + 时间戳可解析 + 时间戳 ≥ 最新输入 mtime，CLI `tryReuseFreshSummary` 同语义） | `crates/core/checks/src/reuse.rs`（新） | model | 纯函数 |
| 纯层门面 | checks crate 模块组织与重导出（无自有导出） | `crates/core/checks/src/lib.rs`（新） | — | — |
| static_check 平移 | ProcessStaticCheck 整体平移（config 读命令 → shell 语义 spawn → 诊断捕获 + 退出码映射；无配置直过；程序不可达显式 Err），零逻辑改动 | `crates/infra/checks/src/static_check.rs`（自 `crates/infra/agent/src/static_check.rs` 平移） | orchestration, config, tokio | tokio::process |
| 框架注册表与探测 | jest / vitest / vite-plus / rust / node-test 五框架注册表（shell/cmd 命令模板、coverage_format、coverage_output、default_glob、config_flag、version_command，模板串与 CLI `test-framework.ts` 逐字对齐）+ glob 文件探测 + 排除过滤 + plan id + 版本探测；bun / go / pytest 显式 Err | `crates/infra/checks/src/testexec/detect.rs`（新） | config(TestSuite), glob | glob::glob（FS 遍历） |
| 命令执行 | 占位符模板展开（`{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` / `{files}`）+ 程序解析前置检查（复用 static_check 的 PATH / cwd 探测与可执行后缀候选经验）+ spawn + 超时 + 输出捕获 | `crates/infra/checks/src/testexec/runner.rs`（新） | tokio(process/time), orchestration(port 类型) | tokio::process + tokio::time::timeout |
| 报告写盘 | 子报告 + summary 落 change 报告目录（路径经 foundation layout 推导，零磁盘字面量） | `crates/infra/checks/src/testexec/report.rs`（新） | foundation(layout), checks, serde_json | serde_json |
| 执行链编排 | detect → 完整性校验 → 复用门（mtime 递归扫描 + 纯判定）→ 逐 suite 执行 → 报告写盘 → 聚合 → 诊断；checks 结论映射 port 最小载荷；`ProcessTestExecution` port 实现 | `crates/infra/checks/src/testexec/mod.rs`（新） | checks, orchestration, config, foundation | — |
| 执行层门面 | checks-runtime crate 模块组织与 ProcessStaticCheck / ProcessTestExecution 导出 | `crates/infra/checks/src/lib.rs`（新） | — | — |
| port 契约 | `ToolCommand::TestExecution` / `ToolStepOutput::TestExecution` / `TestExecutionRunner` 三件套 + 最小载荷类型（port 属于消费者，W4 红线延续：core/orchestration 零进程 spawn、零依赖 checks） | `crates/core/orchestration/src/port.rs`（修改） | workflow, agent | std Pin<Box<dyn Future>> 别名 |
| 步词汇 | `ChangeStepKind::TestExecution` 入封闭集（白名单式扩展，serde/specta camelCase 出线） | `crates/core/orchestration/src/state.rs`（修改） | serde, specta | — |
| 工具步分发 | `ToolCommand::TestExecution` 分发臂委托注入的 runner（与 StaticCheck 臂同型） | `crates/core/orchestration/src/steps.rs`（修改） | orchestration::port | — |
| walker 门禁接入 | test-execution 相位门禁步承接相位主体（绿跑零 agent）+ 结论反馈边独立计数（上限 5）+ 超限升格落账 | `crates/core/orchestration/src/walker.rs`（修改） | port, state, workflow(write) | — |
| 报告路径推导 | change 报告目录（`reports/test` 挂 change 目录树）常量组扩展 + 纯推导函数；layout 命名隔离双禁令唯一触点纪律不变 | `crates/core/foundation/src/layout/mod.rs`（修改） | — | 纯路径推导 |
| 组合根装配 | ProcessStaticCheck import 切新路径 + ProcessTestExecution 构造注入 `LocalToolSteps` | `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`（修改） | checks-runtime, orchestration | tauri 命令组 |
| bindings 再生成 | `ChangeStepKind::testExecution` 变体出线；前端流程视图既有步骤渲染直接吃 | `packages/desktop/src/types/generated/bindings.ts`（再生成） | export-bindings | specta TypeScript |

---

## 关键设计决策

1. **落位（A1，探索 §11 定案）**：检查域双层——`crates/core/checks`（裸名 `checks`，纯层零 spawn 零 Tauri）+ `crates/infra/checks`（裸名 `checks-runtime`，进程执行）；runner port 三件套留 `orchestration::port` 原地（port 属于消费者），checks 域产出到 port 最小载荷的映射发生在 checks-runtime 装配点（core/orchestration 零依赖 checks）。static_check 自 infra/agent 平移零逻辑改动，agent crate 回归会话租户身份。
2. **最小载荷终形（proposal 待决闭环）**：`TestExecutionOutcome { conclusion, total, passed, failed, skipped, findings_brief, report_dir }`。`findings_brief` 由 runner 侧从 findings 逐条拼接（单条 200 字符截断、至多 10 条，对齐 walker `diagnose_brief` 口径）；全量 findings 留报告文件，修复会话经 `report_dir` 自读全量。
3. **glob 选型（proposal 待决闭环）**：用 workspace 既有 `glob = "0.3"`（sdk tools 已消费 `glob::glob`）——infra/checks 探测用 `glob::glob` FS 遍历，core/checks suite 范围匹配用 `glob::Pattern::matches` 纯匹配（不触文件系统）。不引入 globset，**零新增第三方依赖**。
4. **超时来源（proposal 待决闭环）**：对齐 CLI `test-runner.ts` 的 `runCommand(cmd, cwd, 600000)` 与版本探测 `timeout: 30_000` 语义——suite 命令超时 600s、框架版本探测 30s，常量钉在 `testexec/runner.rs`；超时终止子进程收敛 execution_error（不悬挂、不烧反馈边预算）。
5. **机械 eval checklist 形态（proposal 待决闭环）**：写面 `phase_log` 的 verdict 由 checklist 全 pass 推导（`derive_verdict` 既有语义）。绿跑时 walker 从最小载荷机械组装三条 `ChecklistItem`：① suite 结论一致（各 plan report 非 error 且与聚合一致）② 聚合 conclusion 与计数一致 ③ mutation null 自动通过（V1 恒真）；`report` 携 conclusion + 计数 + 报告路径摘要；三会话槽位恒 `None`（绿跑零 agent）。超限升格时单条 item `pass=false` + findings 摘要 evidence（`step_fail_phase_log` 先例同型），executor 槽位携反馈会话 id、evaluator / decision 恒 `None`。
6. **复用门归属**：纯判定落 `core/checks/reuse.rs`（`is_reusable`）；mtime 递归扫描（输入 = 各 suite root 下全部文件，剪枝域根目录树与报告目录，dot 目录剪枝——CLI `computeNewestInputMtime` 同口径）是 IO，落 testexec 执行链内（detect 之后、执行之前）。输入未变复用上次 summary 零 spawn；反馈边修复重入后 mtime 推进自动失效重跑——这是相位自动触发不演变为反复全量跑的幂等基石。
7. **Err 与 conclusion=error 两态分立**：runner 以 `Err` 收场（测试程序不可达 / 配置声明不支持框架 / 写盘失败）经 `run_tool` 映射 run 显式失败终态（停给用户，不产出部分结论）；报告级 conclusion=error（execution_error 问题，如命令非零退出且结果不可解析、超时）走反馈边注入修复。前者是基础设施失败，后者是被测域失败——反馈边只承接后者。
8. **mutation 砍**：生产聚合不产 mutation 块，报告位恒 null（zod 合法，evaluator T3 null 自动通过）；model 类型仍完整定义（corpus fixtures 含真实 mutation 块时保真解析）。本仓 desktop 档(80) / src-tauri rust 档(70) 开着 mutation，MVP 期桌面与 CLI 对同一 change 结论不一致为明知接受（spec V1 留痕）。
9. **反馈边会话语义**：首个 fail 无既有 executor 会话 → 新会话携 findings 摘要 + 报告路径 prompt（`continue_session = None`）；后续 Continue 同会话续注；provenance 沿 `<change>/<phase>/<role>/<attempt>` 定式（role = executor）。反馈边独立计数上限 5（`TEST_EXECUTION_FEEDBACK_LIMIT`，与 `STATIC_CHECK_FEEDBACK_LIMIT` 分立、各自计满各自升格），不计相位 retry 预算。
10. **报告落盘**：恒 change 报告目录（layout 推导，`summary.json` + 逐 plan `<planId>/report.json` 与结果 / 覆盖工件）；不做 artifacts registry 登记（不进 file inventory）；CLI `<root>/reports/` 无-change 分支不实现（desktop 无无-change 场景）。
11. **worker.rs 零触点**：orchestration spec Module Contract 的 worker.rs 行系措辞修订（static-check spawn 缝的实际宿主是 `LocalToolSteps` 注入与 `crates/infra/agent/src/lib.rs` 导出面，worker.rs 从未持有）；本变更对 `crates/infra/agent` 的唯一实现触点是 lib.rs 导出移除 + 两文件删除。
12. **detect 版本探测保留**：jest 模板按版本 ≥ 29.5.0 追加 `--randomize`（CLI `isVersionAtLeast` 同语义）；探测失败 / 空版本按「特性不支持」走基础模板（CLI 同路径），不失败不报错。注册表以常量表移植，模板串与 CLI 逐字对齐（防漂移）。

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。测试文件（proposal「测试文件」节）不在本清单——由 test-design / test-gen 阶段承接；仅 static_check 既有测试文件随源整体平移属实现迁移，随 3.1 条目走。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/checks/Cargo.toml` | 新 crate 清单（裸名 `checks`）：依赖 config / foundation / serde / serde_json / time / glob；零 tokio process、零 Tauri |
| `packages/desktop/src-tauri/crates/core/checks/src/lib.rs` | 纯层门面：模块组织与重导出（无自有导出） |
| `packages/desktop/src-tauri/crates/core/checks/src/model.rs` | 报告 schema 模型（CLI zod 权威逐字段对齐）+ 宽容解析入口 |
| `packages/desktop/src-tauri/crates/core/checks/src/parser/mod.rs` | 解析器路由门面（`CoverageFormat` 路由；js / rust / coverage / text 四子模块同目录） |
| `packages/desktop/src-tauri/crates/core/checks/src/parser/js.rs` | istanbul `coverage-summary.json` → 逐文件覆盖原始计数 |
| `packages/desktop/src-tauri/crates/core/checks/src/parser/rust.rs` | llvm-cov JSON → 逐文件覆盖原始计数 |
| `packages/desktop/src-tauri/crates/core/checks/src/parser/coverage.rs` | 覆盖格式路由（istanbul / llvm-cov / node-test V1 支持面） |
| `packages/desktop/src-tauri/crates/core/checks/src/parser/text.rs` | node-test `--test-reporter=spec` 文本 + rust 测试文本输出 → 用例行 |
| `packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs` | 阈值 / per-suite override 聚合 + conclusion 判定 + summary 组装 |
| `packages/desktop/src-tauri/crates/core/checks/src/diagnose.rs` | 完整性校验 checklist + 诊断树确定性分支 |
| `packages/desktop/src-tauri/crates/core/checks/src/reuse.rs` | 复用门 mtime 新鲜度纯判定 |
| `packages/desktop/src-tauri/crates/infra/checks/Cargo.toml` | 新 crate 清单（裸名 `checks-runtime`）：依赖 orchestration / checks / config / foundation / serde_json / tokio(process, io-util, rt, macros, time) / glob；零 Tauri |
| `packages/desktop/src-tauri/crates/infra/checks/src/lib.rs` | 执行层门面：ProcessStaticCheck / ProcessTestExecution 导出 |
| `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` | ProcessStaticCheck 自 `crates/infra/agent/src/static_check.rs` 平移（零逻辑改动，147 行连同文档注释原样迁移） |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` | 执行链编排（detect → 完整性 → 复用门 → 执行 → 报告 → 聚合 → 诊断）+ `ProcessTestExecution` port 实现 + mtime 输入扫描 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs` | 五框架注册表（模板 / 覆盖格式 / coverage_output / default_glob / config_flag / version_command）+ glob 探测 + 排除过滤 + plan id + 版本探测 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` | 模板展开 + 程序解析前置检查 + spawn + 超时（600s / 30s）+ 输出捕获 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/report.rs` | 子报告 + summary 写盘编排（layout 推导路径） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `ToolCommand` 增 `TestExecution { change }` 变体；`ToolStepOutput` 增 `TestExecution(TestExecutionOutcome)`；新增 `TestExecutionOutcome` / `TestExecutionConclusion` 类型与 `TestExecutionRunner` trait | port 三件套扩展；core/orchestration 零进程 spawn、零依赖 checks（载荷类型定义于 port 本地） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | `ChangeStepKind` 增 `TestExecution` 变体 | 步词汇白名单式扩展；serde/specta camelCase 出线，前端步骤渲染零改动直接吃 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `LocalToolSteps` 增第三构造参数 `test_execution: Arc<dyn TestExecutionRunner>` 与 `ToolCommand::TestExecution` 分发臂 | 机械必需（proposal 未单列；工具步分发宿主，与 StaticCheck 臂同型） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `TEST_EXECUTION_PHASES` / `TEST_EXECUTION_FEEDBACK_LIMIT` 常量；drive 循环 test-execution 相位分支（门禁步承接相位主体，跳过 executor / evaluator 会话）；`test_execution_loop`（pass 机械 checklist 代写 `phase_log`，fail/error 反馈边独立计数修复重入，超限升格落账）；`TryFrom<ToolStepOutput> for TestExecutionOutcome` 窄化 | 反馈边修复重入经 runner 链内复用门自动失效重跑；绿跑路径零 WorkerAgent 调用点 |
| `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 报告目录常量组扩展（`reports` / `test` 两级）+ `change_test_reports(root, change)` 纯推导函数 | 命名隔离双禁令唯一触点纪律不变（常量组内聚，全包其余源码经本函数取路径） |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | `mod static_check` 声明、`pub use static_check::ProcessStaticCheck`、`mod static_check_test` 声明移除 | 检查域成员迁出，agent crate 回归会话租户身份（cli / sdk / worker / compose / store_port / git_diff） |
| `packages/desktop/src-tauri/Cargo.toml` | workspace members 增 `crates/core/checks`、`crates/infra/checks`；workspace.dependencies 增 `checks` / `checks-runtime` path 项 | workspace 注册（glob / serde_json / time / tokio 均 workspace 既有） |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | `ProcessStaticCheck` import 自 `checks_runtime` 取；`LocalToolSteps::new` 增传 `Arc::new(ProcessTestExecution::new())` | 组合根装配（run 作用域一次，W7 先例同型） |
| `packages/desktop/src/types/generated/bindings.ts` | `bindings:export` 再生成 | ChangeStepKind 新变体出线（生成物；`bindings:check` 守卫拦截过期生成物） |
| `packages/desktop/package.json` | version `0.4.11` → `0.4.12` | AC-12 用户可见变更（run 内新增确定性测试执行门禁） |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/static_check.rs` | 平移至 `crates/infra/checks/src/static_check.rs` 后原位删除 |
| `packages/desktop/src-tauri/crates/infra/agent/src/static_check_test.rs` | 随源整体平移后原位删除（既有测试文件迁移，非新写） |

### 公共函数 / API

<!-- identifier：模块级导出函数 / trait / 类型关联函数；pub(crate) 项标注可见性（crate 内部组装面）。私有函数不列。 -->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `TestExecutionRunner` | `crates/core/orchestration/src/port.rs` | 新增 | `pub trait TestExecutionRunner: Send + Sync { fn run(&self, root: &str, change: &str) -> BoxToolFuture; }` | test-execution spawn 缝 port（W4：spawn 不进 core；checks-runtime 实现，产出契约同 `ToolStepOutput` 封闭集） |
| `ProcessTestExecution::new` | `crates/infra/checks/src/testexec/mod.rs` | 新增 | `pub fn new() -> Self` | 无状态构造（组合根装配） |
| `ProcessTestExecution::run` | `crates/infra/checks/src/testexec/mod.rs` | 新增 | `fn run(&self, root: &str, change: &str) -> BoxToolFuture` | `TestExecutionRunner` 实现：执行链编排 + checks 结论映射 port 最小载荷 |
| `ProcessStaticCheck` | `crates/infra/checks/src/static_check.rs` | 修改（平移） | `pub struct ProcessStaticCheck;` + `pub fn new() -> Self` + `impl StaticCheckRunner { fn run(&self, root: &str) -> BoxToolFuture }` | 零逻辑改动平移（无配置直过 / 程序不可达显式 Err / 退出码映射不变） |
| `parse_sub_report` | `crates/core/checks/src/model.rs` | 新增 | `pub fn parse_sub_report(json: &str) -> Result<SubReport, String>` | 子报告宽容解析（未知字段忽略，zod 对齐） |
| `parse_summary_report` | `crates/core/checks/src/model.rs` | 新增 | `pub fn parse_summary_report(json: &str) -> Result<SummaryReport, String>` | 汇总报告宽容解析 |
| `parse_coverage` | `crates/core/checks/src/parser/coverage.rs` | 新增 | `pub fn parse_coverage(format: CoverageFormat, text: &str) -> Result<Vec<SourceFileEntry>, String>` | 按覆盖格式路由至子解析器 |
| `parse_istanbul_summary` | `crates/core/checks/src/parser/js.rs` | 新增 | `pub fn parse_istanbul_summary(text: &str) -> Result<Vec<SourceFileEntry>, String>` | jest / vitest / vite-plus 共享（istanbul 族） |
| `parse_llvm_cov` | `crates/core/checks/src/parser/rust.rs` | 新增 | `pub fn parse_llvm_cov(text: &str) -> Result<Vec<SourceFileEntry>, String>` | cargo llvm-cov JSON 输出 |
| `parse_spec_report` | `crates/core/checks/src/parser/text.rs` | 新增 | `pub fn parse_spec_report(text: &str) -> Result<Vec<TestCaseResult>, String>` | node-test `--test-reporter=spec` 文本结果 |
| `parse_cargo_test` | `crates/core/checks/src/parser/text.rs` | 新增 | `pub fn parse_cargo_test(text: &str) -> Result<Vec<TestCaseResult>, String>` | rust 测试文本输出解析（与 node-test 同路由 text 引擎，CLI `parseTextOutput` rust 分支同语义） |
| `determine_conclusion` | `crates/core/checks/src/aggregate.rs` | 新增 | `pub fn determine_conclusion(failed: u64, coverage: Option<&CoverageBlock>, mutation: Option<&MutationBlock>, has_execution_error: bool) -> Conclusion` | CLI `determineConclusion` 同语义：error 优先 → failed>0 / 覆盖未达 / mutation 未达 → fail → pass |
| `aggregate_coverage` | `crates/core/checks/src/aggregate.rs` | 新增 | `pub fn aggregate_coverage(sub_reports: &[SubReport], suites: &[config::TestSuite]) -> Option<CoverageBlock>` | 全局聚合 + per-suite override 组（glob 范围纯匹配，suite 阈值取 `config::CoverageThresholds`） |
| `build_summary_report` | `crates/core/checks/src/aggregate.rs` | 新增 | `pub fn build_summary_report(sub_reports: &[SubReport], suites: &[config::TestSuite], command: &str, started_at: time::OffsetDateTime) -> SummaryReport` | 汇总组装：计数合并、problems 归并、coverage / mutation 块、plans 路径索引、conclusion 判定 |
| `check_integrity` | `crates/core/checks/src/diagnose.rs` | 新增 | `pub fn check_integrity(summary: &SummaryReport, sub_reports: &[SubReport]) -> Vec<String>` | 完整性 checklist：聚合计数对账、plan 报告齐全性、conclusion 枚举合法性（违例即 findings） |
| `diagnose_findings` | `crates/core/checks/src/diagnose.rs` | 新增 | `pub fn diagnose_findings(summary: &SummaryReport) -> Vec<String>` | 诊断树确定性分支：阈值比对（null 维度感知跳过）、execution_error 归因、失败聚类措辞 |
| `is_reusable` | `crates/core/checks/src/reuse.rs` | 新增 | `pub fn is_reusable(summary: &SummaryReport, newest_input_mtime_ms: Option<u64>) -> bool` | 结论非 error 且时间戳可解析且 ≥ 最新输入 mtime → 复用 |
| `detect_plans` | `crates/infra/checks/src/testexec/detect.rs` | 新增 | `pub(crate) fn detect_plans(root: &Path, suites: &[config::TestSuite]) -> Result<Vec<TestPlan>, String>` | 逐 suite 探测：glob 文件清单 + 排除过滤 + plan id；bun / go / pytest 显式 Err |
| `execute_plan` | `crates/infra/checks/src/testexec/runner.rs` | 新增 | `pub(crate) async fn execute_plan(plan: &TestPlan, report_dir: &Path) -> Result<PlanExecution, String>` | 模板展开 + 程序解析前置 + spawn + 超时 + 捕获 + 工件解析 |
| `newest_input_mtime_ms` | `crates/infra/checks/src/testexec/mod.rs` | 新增 | `pub(crate) fn newest_input_mtime_ms(scan_roots: &[PathBuf], prune: &[PathBuf]) -> Option<u64>` | 输入最新 mtime 递归扫描（剪枝域根目录树与报告目录，dot 目录剪枝） |
| `write_reports` | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/report.rs` | 新增 | `pub(crate) fn write_reports(report_dir: &Path, sub_report: &SubReport, summary: &SummaryReport) -> Result<(), String>` | 子报告 + summary 原子落盘 |
| `change_test_reports` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn change_test_reports(root: &Path, change: &str) -> PathBuf` | change 报告目录纯推导（`config_path` 先例同型；与 `resolve` 同源常量组） |
| `LocalToolSteps::new` | `crates/core/orchestration/src/steps.rs` | 修改 | `pub fn new(anchors: Arc<SessionAnchors>, static_check: Arc<dyn StaticCheckRunner>, test_execution: Arc<dyn TestExecutionRunner>) -> Self` | 第三 runner 注入参数 |
| `TEST_EXECUTION_FEEDBACK_LIMIT` | `crates/core/orchestration/src/walker.rs` | 新增 | `pub const TEST_EXECUTION_FEEDBACK_LIMIT: u32 = 5;` | 反馈边独立上限（与 `STATIC_CHECK_FEEDBACK_LIMIT` 分立） |
| `TEST_EXECUTION_PHASES` | `crates/core/orchestration/src/walker.rs` | 新增 | `pub const TEST_EXECUTION_PHASES: [&str; 1] = ["test-execution"];` | 相位门控布局常量（`STATIC_CHECK_PHASES` 同型；布局词汇非路由权威） |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `TestExecutionConclusion` | `crates/core/orchestration/src/port.rs` | 新增 | Pass / Fail / Error 三值枚举（`as_str` 线格式小写，`WorkerRole` 先例同型） |
| `TestExecutionOutcome` | `crates/core/orchestration/src/port.rs` | 新增 | 最小载荷：`conclusion` + total / passed / failed / skipped 四计数 + `findings_brief` + `report_dir`；全量 findings 留报告文件 |
| `ChangeStepKind::TestExecution` | `crates/core/orchestration/src/state.rs` | 修改 | 步词汇封闭集扩展（serde/specta camelCase `testExecution` 出线） |
| `Conclusion` | `crates/core/checks/src/model.rs` | 新增 | checks 侧聚合结论三值（pass / fail / error）；checks-runtime 装配点映射为 port `TestExecutionConclusion` |
| `SummaryReport` | `crates/core/checks/src/model.rs` | 新增 | summary.json 模型：phase / command / timestamp / duration_seconds / 四计数 / conclusion / problems[] / coverage / mutation / plans[] / findings[] |
| `SubReport` | `crates/core/checks/src/model.rs` | 新增 | report.json 模型：framework / root / timestamp / exit_code / duration_ms / summary / error_cases[] / test_files[] / source_files[] / coverage / mutation / findings[] |
| `TestCaseResult` / `TestCaseStatus` | `crates/core/checks/src/model.rs` | 新增 | 用例行（status 三值 passed / failed / skipped；errorType / errorMessage / stackTrace 仅失败态） |
| `CoverageBlock` / `CoverageMeasured` / `CoverageThresholds` / `CoverageOverride` | `crates/core/checks/src/model.rs` | 新增 | 覆盖率结论块族（与 `config::CoverageThresholds` 同名异域，模块路径区分：前者是报告侧阈值块） |
| `MutationBlock` / `MutationMeasured` | `crates/core/checks/src/model.rs` | 新增 | schema 保真（生产聚合恒 null，corpus 真实 fixtures 解析不丢块） |
| `Problem` / `ProblemType` / `PlanIndexEntry` / `SourceFileEntry` / `FileCoverageEntry` | `crates/core/checks/src/model.rs` | 新增 | 问题三元（test_failure / coverage_failure / execution_error）、plan 路径索引、逐文件覆盖计数 |
| `CoverageFormat` | `crates/core/checks/src/parser/mod.rs` | 新增 | istanbul / llvm-cov / node-test 三值（V1 支持面；go-cover / coverage-py / lcov 不入） |
| `TestPlan` | `crates/infra/checks/src/testexec/detect.rs` | 新增 | detect 产物：id / framework / root / cwd / files / config_args（进程内，不持久化） |
| `PlanExecution` | `crates/infra/checks/src/testexec/runner.rs` | 新增 | 单 suite 执行结果：exit_code / duration_ms / 子报告模型 / 工件解析错误面 |

### 配置

<!-- 零配置键变更：工作区配置的 tests[] 解析已就位（config crate TestSuite 八字段 typed 模型 + 永不失败 diagnostics 信封），checks 两 crate 直接消费；无新增配置文件、无新配置键、插件侧配置零触点（plugins/dev-team 保持 2.10.44）。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `SummaryReport`（summary.json） | phase（恒 "test-execution"）/ command / timestamp / duration_seconds / total / passed / failed / skipped / conclusion / problems[] / coverage / mutation / plans[] / findings[] | `plans[i]` ↔ 子报告目录 planId 一一对应（路径索引，无状态字段；成败在子报告）；`coverage.overrides[]` 由 config tests[] suites × 各子报告 `source_files` 推导 | change 报告目录 `summary.json`（路径经 layout 推导） |
| `SubReport`（report.json，每 plan 一份） | framework / root / timestamp / exit_code / duration_ms / summary{total, passed, failed, skipped} / error_cases[] / test_files[] / source_files[] / coverage / mutation / findings[] | 隶属 `SummaryReport.plans[]` 条目；`mutation` 恒 null（V1）；`source_files` 是 override 聚合的原始计数源 | `<报告目录>/<planId>/report.json` + 同目录结果 / 覆盖工件（results / coverage-summary 等） |
| `TestExecutionOutcome`（port 最小载荷） | conclusion / total / passed / failed / skipped / findings_brief / report_dir | 由 `SummaryReport` 聚合面在 checks-runtime 装配点映射；步 detail 与反馈边 prompt 的唯一 IPC 面 | 不持久化（`RunUpdate::Step` detail 摘要 + 图节点） |
| `TestPlan`（detect 产物） | id / framework / root / cwd / files / config_args | 由 config tests[] suite + glob 探测推导；id 沿 CLI colocated 命名映射（`test-path-naming.ts` 同语义） | 不持久化（进程内） |
| `TestSuite`（config crate 既有） | root / framework / cwd / config / includes / excludes / coverage / mutation | detect 与 aggregate 的共同输入；八 typed 字段零改动消费 | 工作区配置文件（既有解析，diagnostics 信封容错） |
| corpus fixtures | 真实工作区产出的 summary.json / report.json（istanbul 族 / llvm-cov / node-test 文本、红绿两态） | 黄金对拍输入（AC-8）；zod 权威对齐的漂移拦截面 | `crates/core/checks/tests/fixtures/`（语料测试由 test-design / test-gen 阶段承接） |

---

<!-- 路由/API 设计省略：不涉及 HTTP API；Tauri IPC 面零新命令——步可观测沿既有 `RunUpdate::Step` 通道流出，`ChangeStepKind` 新变体经 bindings 再生成出线，前端流程视图既有步骤渲染直接消费（AC-10）。 -->

---

## 依赖

### 运行时依赖

（全部 workspace 既有，**零新增第三方依赖**）

- `glob = "0.3"` — detect 文件探测（`glob::glob` FS 遍历，infra/checks）与 suite 范围纯匹配（`glob::Pattern::matches`，core/checks）——选型定案：不引入 globset
- `tokio`（process / io-util / rt / macros / time features）— infra/checks spawn、超时、输出捕获
- `serde` + `serde_json` — 报告模型解析与写盘
- `time` — summary timestamp 的 ISO 解析 / 生成（复用门与报告组装）
- `config` / `foundation` / `orchestration` — typed tests[] 消费 / layout 路径推导 / port 契约（core→core 与 infra→core 既有先例同型）

### 构建/测试依赖

- `tempfile`（workspace 既有 dev-dep）— infra/checks 与 orchestration 测试临时工作区
- `export-bindings`（既有工具 crate）— bindings 再生成（`bindings:export` / `bindings:check` 守卫）

---

## 提案与规格同步状态

- `proposal.md` 与 specs 四份 delta（desktop-checks-domain / desktop-test-execution / desktop-change-orchestration / desktop-crate-layout）已随提案阶段写入并通过评审，本 design 不再将其列为变更清单条目或任务；AC-11 的措辞修订随归档生效。
- 探索笔记 `explore.md`（三轮方向修正 + 两轮逐条对码审查 E1–E5，事实基线 HEAD `aff4313`）为本设计的权威事实来源，§11 落位修订与 §12 拍板项已全部吸收。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 变更清单新增 `crates/core/checks` 与 `crates/infra/checks` 全部文件 + Cargo.toml workspace 注册（AC 的套件通过条件由 test-execution 阶段承接验证） |
| AC-2 | `static_check.rs` 连同既有测试文件整体平移零逻辑改动；`crates/infra/agent/src/lib.rs` 导出移除 + 删除文件表两条目 |
| AC-3 | port.rs 三件套（`ToolCommand::TestExecution` / `ToolStepOutput::TestExecution` / `TestExecutionRunner`）+ 依赖规则：core/orchestration 零进程 spawn、零依赖 checks，进程执行全部落 infra/checks |
| AC-4 | `testexec/detect.rs` 五框架注册表产出 plan；bun / go / pytest 显式 `Err`（无静默跳过分支） |
| AC-5 | `testexec/mod.rs` 执行链编排 + `report.rs` 写盘 + layout 报告路径推导（链路绿跑由 test-execution 阶段 fixture 直驱承接验证） |
| AC-6 | `reuse.rs` 纯判定 + 执行链内复用门装配（两分支行为由 test-execution 阶段单测覆盖） |
| AC-7 | walker.rs 相位分支 + `ChangeStepKind::TestExecution` 上图 + `TEST_EXECUTION_FEEDBACK_LIMIT = 5` 独立计数 + 超限升格落账；绿跑路径无任何 WorkerAgent 调用点（机械 checklist 代写 `phase_log`） |
| AC-8 | `model.rs` 以 CLI zod schema 为权威对齐源 + 数据模型节 corpus fixtures 黄金对拍面（语料测试由 test-design / test-gen 阶段承接） |
| AC-9 | `layout::change_test_reports` 纯推导 + 新增 `.rs` 注释零双禁令字面量（显式验收项；机械扫描由 test-execution 阶段承接） |
| AC-10 | 组合根装配 `ProcessTestExecution` + bindings 再生成（`testExecution` 出线）+ 前端步骤渲染白名单式扩展零改动消费 |
| AC-11 | orchestration「唯一 spawn 例外」修订为检查域家族、worker.rs 行、crate-layout 六类分类学——spec delta 已随提案写入并通过评审，归档时随事实生效 |
| AC-12 | `packages/desktop/package.json` version `0.4.12`（阶段五任务） |

---

## 待决问题

- corpus fixtures 的来源工作区与样本规模（真实 run 报告采集，需覆盖 istanbul 族 / llvm-cov / node-test 文本与红绿两态）——test-design 阶段定
- summary.json `command` 字段的桌面固定串措辞（CLI 写命令描述，桌面 run 内无 CLI 子命令面）——实现期定稿，候选「desktop change run: test-execution gate」
