# 任务: desktop-fix-mutation-test

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖实现文件。顺序按依赖排列：步词汇封闭集扩展 → port 三件套与 vcs 缝声明 → vcs 清单 / 补丁封装实现 → core/checks 纯层（映射 / score / 归并）→ infra/checks mutation 执行模块 → steps.rs 接线 → walker 拆步与双时点反馈 → 组合根装配 + bindings 再生 → 前端步词汇归一 → 版本交付。
>
> 依赖序要点：`phase_table.rs`（阶段一）消费 `StepNode::MutationTest`（阶段一前项）；port.rs（阶段二）的 `MutationTestOutcome` 被 walker（阶段七）与 mutation 执行模块（阶段五）共同消费；`steps.rs`（阶段六）消费 `MutationRunner`（阶段二）+ `ArchiveVcsPort` 两方法（阶段二声明 / 阶段三实现）+ `ProcessMutation`（阶段五）；mutation 执行模块（阶段五）消费 `parse_*_seg` / `merge_mutation_blocks` / `build_mutation_block`（阶段四）；bindings 再生（阶段八）依赖词汇扩展（阶段一 / 二）并先于前端（阶段九读 bindings 新词）；`run-step-node.tsx` 的 `STEP_LABEL` 穷尽表（阶段九）随 bindings 新词编译期强制。

## 阶段一：步词汇封闭集扩展与拆步相位表

- [x] 修改 `crates/core/workflow/src/state.rs`：`StepNode` 增 `MutationTest` 变体（文档注记 18→19 值封闭集）；`StepNode::as_str` 增 `StepNode::MutationTest => "mutation_test"` 臂；`StepNode::group` 增 `MutationTest` 归 `StepGroup::Exec`（与 PhaseStart / StaticCheck / TestExecution 同臂）
- [x] 修改 `crates/core/workflow/src/state.rs`：`RunStepKind` 增 `MutationTest` 变体 + `as_str` 增 `"mutation_test"` 臂（文档注记 18→19 值）；`StepKind` 增 `MutationTest` 变体 + `as_str` 增 `"mutation_test"` 臂（命令包络 7→8 臂，文档注记随动）
- [x] 修改 `crates/core/workflow/src/write/phase_table.rs`：`TEST_EXECUTION_STEPS` 在 `StepNode::TestExecution` 与 `StepNode::PhaseLog` 间插 `StepDefinition { node: StepNode::MutationTest, prompt_ref: None, on_fail: Some(OnFail::FeedbackLoop { limit: FEEDBACK_LOOP_LIMIT }) }`；注释改写为拆步语义（`PhaseStart → TestExecution → MutationTest → PhaseLog`，测试 / 覆盖率修绿后才跑突变；requirement / test-only 两表共用同一 static 天然同式）
- [x] 修改 `crates/core/orchestration/src/state.rs`：`ChangeStepKind` 增 `MutationTest` 变体（camelCase 线词 `mutationTest`）+ `as_str` 增 `ChangeStepKind::MutationTest => "mutation_test"` 臂；文档注记 18→19 值
- [x] 修改 `crates/core/orchestration/src/run_history.rs`：`mapped_step` 增 `ChangeStepKind::MutationTest => RunStepKind::MutationTest` 臂；文档注记「十八值 1:1」→「十九值 1:1」

## 阶段二：port 三件套 + vcs 缝声明

- [x] 修改 `crates/core/orchestration/src/port.rs`：新增 `MutationTestConclusion`（`Pass` / `Fail` / `Error` 三值 + `as_str` 线格式小写词，`TestExecutionConclusion` 同型；Fail = 全段跑完未达标、Error = 段中断）
- [x] 修改 `crates/core/orchestration/src/port.rs`：新增 `MutationSkipReason`（`EmptyDiff` / `NoExecutor`）、`MutationCounts`（`killed` / `survived` / `timeout` / `compile_error` / `runtime_error` / `ignored` / `total: u64`）、`MutationTestOutcome`（`conclusion` + `skip: Option<MutationSkipReason>` + `score` / `threshold: f64`（归并块口径，skip 时 0.0 占位）+ `counts` + `report_paths: Vec<String>` + `feedback_text: String`）；全部零 Serialize（不入 IPC），文档注记报告指引式口径（feedback_text MUST NOT 内联存活突变体清单）
- [x] 修改 `crates/core/orchestration/src/port.rs`：`ToolCommand` 增 `MutationTest { change_id: String }` 变体；`ToolStepOutput` 增 `MutationTest(MutationTestOutcome)` 变体；新增 `pub trait MutationRunner: Send + Sync { fn run(&self, root: &str, name: &str, files: &[String], patch: &str) -> BoxToolFuture; }`（与 `TestExecutionRunner` 并列同式；`files` = 清单入参、`patch` = `--in-diff` 补丁入参——E3，checks 侧零 git spawn）
- [x] 修改 `crates/core/orchestration/src/port.rs`：`ArchiveVcsPort` 增 `fn change_edit_set(&self, root: &Path, base: &str) -> Result<Vec<String>, String>` 与 `fn change_edit_patch(&self, root: &Path, base: &str) -> Result<String, String>` 两结构化方法（清单 / 补丁取数缝；文档注记「突变范围与文件清单唯一入口」）

## 阶段三：vcs 清单 / 补丁封装实现

- [x] 修改 `crates/infra/vcs/src/git.rs`：新增 `pub fn change_edit_set(root: &Path, base: &str) -> Result<Vec<String>, String>`——`git diff <base> --name-only -z` ∪ `git ls-files --others --exclude-standard -z`，排序去重、POSIX 归一（复用既有 `git(...)` spawn 助手与 `-z` 解析纪律）；git 失败显式 `Err`，MUST NOT 静默降级空清单
- [x] 修改 `crates/infra/vcs/src/git.rs`：新增 `pub fn change_edit_patch(root: &Path, base: &str) -> Result<String, String>`——untracked 面先行 `git add -N`（intent-to-add，E2；不回退标记——worktree 私有锚 + 归档 `commit_all` 收编）再 `git diff <base>` 输出 unified diff 文本
- [x] 修改 `crates/infra/vcs/src/lib.rs`：`impl ArchiveVcsPort for ProcessArchiveVcs` 增 `change_edit_set` / `change_edit_patch` 两方法（转发 `git.rs` 自由函数）

## 阶段四：core/checks 纯层（映射 / score / 归并）

- [x] 修改 `crates/core/checks/src/model.rs`：mutation 块族注释改写（「生产聚合恒 None」→「突变位由 MutationTest 归并回填；本域提供段级解析与归并模型」）；新增 `MutationTool`（`CargoMutants` / `Stryker`）与 `MutationSegReport`（`tool` + `measured: MutationMeasured` + `duration_ms: f64`）类型（`MutationMeasured` / `MutationBlock` 既有字段零扩展）
- [x] 修改 `crates/core/checks/src/model.rs`：新增 `pub fn parse_cargo_mutants_seg(raw: &str) -> Result<MutationSegReport, String>`——cargo-mutants `--json` 段报告宽容解析 + 映射表（`caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`；未知 outcome 计 `runtimeError` 并保留原文）；新增 `pub fn parse_stryker_seg(raw: &str) -> Result<MutationSegReport, String>`（Stryker outcome 词与 `MutationMeasured` 1:1，同宽容纪律——宽容解析不悄悄吞数据）
- [x] 修改 `crates/core/checks/src/aggregate.rs`：新增 `pub fn build_mutation_block(measured: MutationMeasured, threshold: f64) -> MutationBlock`——score 公式单点 `(killed+timeout)/(total-ignored-compileError-runtimeError)`（分母 0 → 100.0，两位舍入）+ `pass = score >= threshold`；新增 `pub fn merge_mutation_blocks(blocks: &[MutationBlock]) -> Option<MutationBlock>`——段计数求和归并（阈值取首块）；`aggregate_mutation` 改调 `merge_mutation_blocks`（公式逐字不变、注释改写为生产启用口径）

## 阶段五：infra/checks mutation 执行模块（新）

- [x] 新增 `crates/infra/checks/src/mutation/segments.rs`：分段切分（估算 = `EST_MUTANTS_PER_FILE × 文件数` × 1~2 s/mutant > `SEGMENT_BUDGET_MS` 触发，字典序均分 k 段）+ 自适应修正（后续段按上一段实测速率调段大小，单文件超预算接受、`SEGMENT_GUARD_MS` 软护栏仅观察留痕）+ `--in-diff` 补丁按 `diff --git` 文件段头切片为段补丁（Stryker 面为 mutate 清单切片）
- [x] 新增 `crates/infra/checks/src/mutation/stryker.rs`：Stryker 模板展开与 spawn——临时 config 写盘（`mutate` = 段清单、`tempDirPath` = `<mutation 目录>/_stryker-tmp` 全段共享、`concurrency` = `max(1, available_parallelism()/2)`）、`npx --prefix <suite cwd> stryker run <config>`（超时 + 输出捕获）、全段后一次清理（只清 `_stryker-tmp` 与临时 config，报告工件不触碰）
- [x] 新增 `crates/infra/checks/src/mutation/cargo_mutants.rs`：cargo-mutants 模板展开与 spawn——`cargo mutants --in-diff <seg patch> --json --baseline skip --jobs <max(1, available_parallelism()/2)> --timeout <PER_MUTANT_TIMEOUT_SECS>`（超时 + 输出捕获）；`CARGO_TARGET_DIR` 全段共享（增量编译一次全量，MUST NOT 每段重置）
- [x] 新增 `crates/infra/checks/src/mutation/report.rs`：段级报告 `mutation-seg-<i>.json` 写盘（工具原始输出，全程留存）+ 归并块 `mutation.json` 写盘 + 段级复用门（段报告在位且 mtime ≥ 段输入最新 mtime → 零 spawn 复用、重解析进归并；否则失效重跑——断点续跑只重跑未完 / 失效段）
- [x] 新增 `crates/infra/checks/src/mutation/mod.rs`：突变常量参数（`MUTATION_JOBS_DIVISOR = 2` / `PER_MUTANT_TIMEOUT_SECS = 300` / `SEGMENT_BUDGET_MS = 300_000` / `SEGMENT_GUARD_MS = 600_000` / `EST_MUTANTS_PER_FILE = 10`，D9 / E9 不进 schema）+ `ProcessMutation`（`new()` / `Default`）+ `execute` 主链：suite 归桶（E10：文件按 suite.root POSIX 前缀归桶，桶外文件不突变）→ 跳过面（`EmptyDiff` / `NoExecutor` 记 evidence 不 Err）→ 分段顺序执行 → 段报告解析（`parse_*_seg`）→ 计数归并（`merge_mutation_blocks` / `build_mutation_block`）→ 逐 suite 阈值判定（全过即 pass）→ summary mutation 位回填（E11，在位时）→ `MutationTestOutcome` 与 `feedback_text` 装配（Fail = score / threshold 结论摘要 + 报告路径清单；Error = 中断错误摘要 + 已产报告路径清单；MUST NOT 内联存活突变体清单）；工具不可达 / 配置不支持 / 写盘失败显式 `Err`（E5）
- [x] 修改 `crates/infra/checks/src/lib.rs`：增 `pub mod mutation;` 与 `pub use mutation::ProcessMutation;`
- [x] 修改 `crates/infra/checks/src/testexec/mod.rs`：仅注释口径随动——`build_sub_report` 的 `mutation: None` 注释改为「突变面由独立 MutationTest 步承载并回填 summary mutation 位；本链不产突变结果」；零逻辑改动

## 阶段六：steps.rs 接线

- [x] 修改 `crates/core/orchestration/src/steps.rs`：`LocalToolSteps` 增 `mutation: Arc<dyn MutationRunner>` 字段；`new` 增 `mutation` 入参（位于 `test_execution` 之后）；`ToolStepPort::run` 与 `execute` 签名随动透传
- [x] 修改 `crates/core/orchestration/src/steps.rs`：`execute` 增 `ToolCommand::MutationTest { change_id }` 分支——id → 记录 → name 解析（与 test-execution 同式，未建档显式 `Err`）→ `vcs.change_edit_set(Path::new(&step.root), "HEAD")` + `vcs.change_edit_patch(Path::new(&step.root), "HEAD")` 取数 → `mutation.run(&step.root, &name, &files, &patch)` → `audit_outcome(..., StepKind::MutationTest, ...)`（摘要携 conclusion / score / threshold）

## 阶段七：walker 拆步与双时点反馈边②

- [x] 修改 `crates/core/orchestration/src/walker.rs`：`PhaseCursor` 增 `mutation_outcome: Option<MutationTestOutcome>` 槽（`new` 初始化 `None`）；`run_step` 增 `StepNode::MutationTest => step_mutation_test(ctx, trace, cursor, step).await` 臂
- [x] 修改 `crates/core/orchestration/src/walker.rs`：新增 `step_mutation_test`（照 `step_test_execution` 同式）——`feedback_limit(step, "mutation-test")` 独立计数；Pass → 节点 passed（detail 携 `conclusion=pass score=… threshold=…`）+ `cursor.mutation_outcome = Some(outcome)` + Proceed；Fail / Error → 节点 failed → 超限走 `step_mutation_test_upgraded_phase_log` 代写 fail checklist 升格相位 fail（`cursor.failed = true` + `StepFlow::Upgraded`），否则反馈 prompt 只嵌 `outcome.feedback_text`（结论摘要 + 报告路径清单，MUST NOT 内联存活突变体）注入 executor 会话（`EdgeKind::Feedback`，续注同会话）
- [x] 修改 `crates/core/orchestration/src/walker.rs`：新增 `step_mutation_test_upgraded_phase_log`（照 `step_test_execution_upgraded_phase_log` 同式：fail checklist 条目「突变测试」+ 仅携 executor 槽位）
- [x] 修改 `crates/core/orchestration/src/walker.rs`：`step_phase_log` 的 test 分支消费 `cursor.test_outcome.take()` 时一并 `cursor.mutation_outcome.take()`；`test_execution_phase_log_input` 签名增 `mutation: Option<&MutationTestOutcome>` 双槽入参——第 3 条「mutation null 自动通过」判定语义恒 pass 不变，evidence 三态组装（E8）：`skip=None` → 「跑过且达标（score X ≥ threshold Y）」、`skip=EmptyDiff` → 「diff 空跳过」、`skip=NoExecutor` / `None` → 「null（未跑）」（NoExecutor 附注「无突变执行器」）

## 阶段八：组合根装配 + bindings 再生

- [x] 修改 `src/commands/change_flow/mod.rs`：`LocalToolSteps::new(...)` 增传 `Arc::new(ProcessMutation::new())`（位于 `ProcessTestExecution::new()` 之后）；`checks_runtime::{ProcessStaticCheck, ProcessTestExecution, ProcessMutation}` import 随动
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生 `src/types/generated/bindings.ts`（`ChangeStepKind` 增 `"mutationTest"`、`RunStepKind` / `StepKind` 增 `"mutation_test"`；非手改）——先于阶段九前端读 bindings 新词

## 阶段九：前端步词汇归一

- [x] 修改 `src/views/changes/flow/run-state.ts`：`normalizeKind` 增 `case 'mutation_test': return 'mutationTest';`；`stepWord` 增 `case 'mutationTest': return 'mutation_test';`；注释「十八值」→「十九值」（`mutationTest` 经 `groupOf` default 落 `toolStep` exec 组，`GROUP_LABEL` 零改动）
- [x] 修改 `src/views/changes/flow/run-step-node.tsx`：`STEP_LABEL: Record<ChangeStepKind, string>` 增 `mutationTest: '突变执行'` 键（穷尽表编译期强制随动；渲染逻辑零改动）

## 阶段十：版本交付（归档时执行）

- [x] 修改 `packages/desktop/package.json`：`version` 0.5.1 → 0.5.2（`src-tauri/tauri.conf.json` 维持 `../package.json` 引用自动跟随，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 保持零改动）
