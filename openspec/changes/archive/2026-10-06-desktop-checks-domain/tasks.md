# 任务: desktop-checks-domain

> 依赖顺序：契约与地基 → 纯层 → 执行层 → walker 接入 → IPC 面与交付收尾。
> 测试编写与测试执行由 test-design / test-gen / test-execution 阶段承接，本清单不含。

## 阶段一：契约与地基（port / state / layout / crate 骨架）

- [x] `packages/desktop/src-tauri/Cargo.toml`：workspace members 增 `crates/core/checks`、`crates/infra/checks`，workspace.dependencies 增 `checks` / `checks-runtime` path 项（glob / serde_json / time / tokio 均 workspace 既有）
- [x] 新建 `crates/core/checks` crate 骨架：`Cargo.toml`（裸名 `checks`；依赖 config / foundation / serde / serde_json / time / glob，零 tokio process 零 Tauri）+ `src/lib.rs` 纯层门面（模块组织与重导出）
- [x] 新建 `crates/infra/checks` crate 骨架：`Cargo.toml`（裸名 `checks-runtime`；依赖 orchestration / checks / config / foundation / serde_json / tokio(process, io-util, rt, macros, time) / glob，零 Tauri）+ `src/lib.rs` 执行层门面
- [x] `crates/core/orchestration/src/port.rs`：`ToolCommand::TestExecution { change }` 变体、`ToolStepOutput::TestExecution(TestExecutionOutcome)` 变体、`TestExecutionOutcome`（conclusion + 四计数 + findings_brief + report_dir）与 `TestExecutionConclusion` 最小载荷类型、`TestExecutionRunner` trait（`fn run(&self, root: &str, change: &str) -> BoxToolFuture`）
- [x] `crates/core/orchestration/src/state.rs`：`ChangeStepKind` 增 `TestExecution` 变体（serde/specta camelCase `testExecution` 出线）
- [x] `crates/core/foundation/src/layout/mod.rs`：报告目录常量组扩展（`reports` / `test` 两级）+ `change_test_reports(root: &Path, change: &str) -> PathBuf` 纯推导（`config_path` 先例同型，常量组内聚）
- [x] `crates/core/orchestration/src/steps.rs`：`LocalToolSteps::new` 增第三参数 `test_execution: Arc<dyn TestExecutionRunner>` + `execute` 增 `ToolCommand::TestExecution` 分发臂（与 StaticCheck 臂同型）

## 阶段二：core/checks 纯层（fixtures 直驱面）

- [x] `crates/core/checks/src/model.rs`：报告 schema 模型逐字段对齐 CLI zod 权威（`plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts`）——`SummaryReport` / `SubReport` / `TestCaseResult` / `Conclusion` / coverage 块族 / mutation 块族 / `Problem` / `PlanIndexEntry` / `SourceFileEntry` + 宽容解析入口 `parse_sub_report` / `parse_summary_report`（未知字段忽略）
- [x] `crates/core/checks/src/parser/js.rs`：`parse_istanbul_summary`（istanbul `coverage-summary.json` → `Vec<SourceFileEntry>`）
- [x] `crates/core/checks/src/parser/rust.rs`：`parse_llvm_cov`（cargo llvm-cov JSON → `Vec<SourceFileEntry>`）
- [x] `crates/core/checks/src/parser/text.rs`：`parse_spec_report`（node-test `--test-reporter=spec` 文本）+ `parse_cargo_test`（rust 测试文本输出）→ `Vec<TestCaseResult>`
- [x] `crates/core/checks/src/parser/mod.rs` + `coverage.rs`：`CoverageFormat` 枚举与 `parse_coverage` 路由
- [x] `crates/core/checks/src/aggregate.rs`：`determine_conclusion`（CLI 同语义：error 优先 → failed>0 / 覆盖未达 / mutation 未达 → fail → pass）、`aggregate_coverage`（全局聚合 + per-suite override 组，glob `Pattern::matches` 纯范围匹配）、`build_summary_report`（计数合并 / problems 归并 / plans 路径索引 / conclusion 判定）
- [x] `crates/core/checks/src/diagnose.rs`：`check_integrity`（聚合计数对账 / plan 报告齐全性 / conclusion 枚举合法性）+ `diagnose_findings`（阈值比对含 null 维度感知、execution_error 归因、失败聚类措辞）
- [x] `crates/core/checks/src/reuse.rs`：`is_reusable`（结论非 error + 时间戳可解析 + 时间戳 ≥ 最新输入 mtime）

## 阶段三：infra/checks 执行层

- [x] `crates/infra/agent/src/static_check.rs` + `static_check_test.rs` 整体平移至 `crates/infra/checks/src/`（零逻辑改动，文档注释原样迁移），`crates/infra/agent/src/lib.rs` 移除 `mod static_check` / `pub use static_check::ProcessStaticCheck` / `mod static_check_test` 声明，原位两文件删除
- [x] `crates/infra/checks/src/testexec/detect.rs`：五框架注册表（jest / vitest / vite-plus / rust / node-test 的 shell/cmd 模板、coverage_format、coverage_output、default_glob、config_flag、version_command，模板串与 CLI `test-framework.ts` 逐字对齐）+ `detect_plans`（glob 探测 + 排除过滤 + plan id + 版本探测）；bun / go / pytest 显式 `Err`
- [x] `crates/infra/checks/src/testexec/runner.rs`：`execute_plan`——占位符模板展开（`{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` / `{files}`）+ 程序解析前置检查（复用 static_check 的 PATH / cwd 探测与可执行后缀候选经验，防 shell 吞命令缺失）+ spawn + 超时（suite 命令 600s / 版本探测 30s，常量钉死）+ 输出捕获 + 工件解析
- [x] `crates/infra/checks/src/testexec/report.rs`：`write_reports` 子报告 + summary 落盘（路径经 `foundation::layout::change_test_reports` 推导，零磁盘字面量）
- [x] `crates/infra/checks/src/testexec/mod.rs`：执行链编排（detect → 完整性校验 → 复用门 [`newest_input_mtime_ms` 递归扫描 + `reuse::is_reusable` 纯判定，剪枝域根目录树与报告目录] → 逐 suite 执行 → 报告写盘 → 聚合 → 诊断）+ `ProcessTestExecution`（`TestExecutionRunner` 实现；checks `Conclusion` 映射 port `TestExecutionConclusion`，`findings_brief` 逐条拼接单条 200 字符截断至多 10 条）
- [x] `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`：`ProcessStaticCheck` import 自 `checks_runtime` 取 + `LocalToolSteps::new` 增传 `Arc::new(ProcessTestExecution::new())`（组合根装配，run 作用域一次）

## 阶段四：walker 接入

- [x] `crates/core/orchestration/src/walker.rs`：`TEST_EXECUTION_PHASES`（`["test-execution"]`）与 `TEST_EXECUTION_FEEDBACK_LIMIT`（5）常量；drive 循环 test-execution 相位分支——门禁步承接相位主体（跳过 executor / evaluator 会话与 verdict 解析门）；`test_execution_loop`：pass 时机械 checklist 三条目（suite 结论一致 / 聚合计数一致 / mutation null 自动通过）代写 `phase_log`（三会话槽位 `None`）；fail / error 时 findings 摘要 + 报告路径注入 executor 会话修复（首个 fail 新会话、后续 Continue 同会话，provenance 沿既有定式），独立计数不占相位 retry 预算；超限单条 item `pass=false` 代写 fail checklist 升格相位 fail（executor 槽位携反馈会话 id）；`TryFrom<ToolStepOutput> for TestExecutionOutcome` 窄化；runner `Err`（程序不可达 / 不支持框架 / 写盘失败）映射 run 显式失败终态

## 阶段五：IPC 面与交付收尾（静态，不含测试执行）

- [x] bindings 再生成：`pnpm -C packages/desktop run bindings:export`（`ChangeStepKind::testExecution` 变体出线；测试执行归 test-execution 阶段承接）
- [x] `packages/desktop/package.json`：version `0.4.11` → `0.4.12`（AC-12 用户可见变更：run 内新增确定性测试执行门禁）
- [x] 静态守线：`cargo fmt --check`、`cargo clippy`、`pnpm -C packages/desktop run check`（server:check + client:check）、`bindings:check` 零 diff；命名隔离双禁令自查——新增 `.rs` 源码与注释零域根目录名 / 配置文件名字面量（报告路径一律经 layout 推导取用；机械扫描验证由 test-execution 阶段承接）
