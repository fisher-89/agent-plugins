# 设计: desktop-fix-mutation-test

> **变更**: desktop-fix-mutation-test
> **日期**: 2026-10-13
> **基于**: proposal.md、explore.md（五轮已收敛）、specs/ 十个能力 delta（desktop-mutation-testing / desktop-test-execution / desktop-change-orchestration / desktop-workflow-write-face / desktop-checks-domain / desktop-change-state-store / desktop-change-queries / desktop-change-flow-view / desktop-corpus-regression / desktop-crate-layout）

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 十份 delta 已由提案阶段定稿，本设计基于其文本展开，只覆盖「变更范围 - 实现文件」（+ 编译期强制的少量随动面，见变更清单），并把 proposal 留给 design 的三个定稿点落到具体形态：

1. **vcs 缝形态**（D2 / D3 语境）：`change_edit_set` 清单封装 + `change_edit_patch` 补丁封装同居 `infra/vcs/src/git.rs` 单点，经既有 `ArchiveVcsPort`（walker 第三执行缝）两个结构化方法注入 `MutationRunner`（见 E1 / E3）。
2. **untracked 进 `--in-diff` 的口径**（风险 3）：`git add -N`（intent-to-add）定案，`--file` 双轨被拒（见 E2）。
3. **工具不可达处置**（风险 1）：显式 `Err` 停给用户定案（见 E5）。

工作树实况（本变更起点，`packages/desktop` 0.5.1 基线）：

- `core/workflow/src/state.rs`：`StepNode` / `RunStepKind` 18 值封闭集、`StepKind` 7 臂命令包络，均无 `MutationTest`；`StepNode::group()` 四分类无突变步。
- `core/workflow/src/write/phase_table.rs`：`TEST_EXECUTION_STEPS` = `[PhaseStart, TestExecution, PhaseLog]`；`FEEDBACK_LOOP_LIMIT = 5`。
- `core/orchestration/src/state.rs`：`ChangeStepKind` 18 值（camelCase 线词）；`run_history.rs::mapped_step` 18 臂 1:1 映射。
- `core/orchestration/src/port.rs`：`ToolCommand` 无 `MutationTest`；`ToolStepOutput` 无 `MutationTest`；`TestExecutionRunner` / `StaticCheckRunner` / `ArchiveVcsPort` 在案，`DiffContextPort` 仅声明未接线；无 `MutationRunner`。
- `core/orchestration/src/walker.rs`：`PhaseCursor` 只有 `test_outcome` 单槽；`test_execution_phase_log_input` 机械 checklist 第 3 条 evidence 写死「V1 不移植 mutation」；`feedback_limit(step, label)` 机制在案。
- `core/checks/src/model.rs`：`MutationMeasured` / `MutationBlock` schema 在案（零扩展口径）；`aggregate.rs::aggregate_mutation` 计数求和公式在案但注释「生产聚合恒 None」。
- `infra/checks/src/testexec/mod.rs:394`：`build_sub_report` 硬编码 `mutation: None`；`detect.rs::TestPlan` 无 mutation 字段。
- `infra/vcs/src/git.rs`：有 `diff --name-only` 先例（unmerged 清单）与 `ls-files -z` 解析先例，无 `change_edit_set`。
- 前端 `flow/run-state.ts`：`normalizeKind` / `stepWord` 18 值逐词对应；`run-step-node.tsx` `STEP_LABEL: Record<ChangeStepKind, string>` 18 键穷尽表。

本设计描述**本变更完成后的目标形态**：test-execution 相位拆步 `PhaseStart → TestExecution → MutationTest → PhaseLog`、步词汇 19 值全映射、`change_edit_set` / `change_edit_patch` vcs 单点、js=Stryker / rust=cargo-mutants 框架矩阵与 outcome 映射表、分段顺序执行 + sandbox 跨段复用 + 段级复用门、双时点反馈边②（报告指引式）、机械 checklist 双槽与 evidence 三态。

---

## 关键设计决策（proposal 定稿 + 本设计展开）

proposal D1–D10 已定稿（只桌面端 / diff 确认 / untracked 并入 / unviable→compileError / 5 分钟分段 / sandbox 复用 / 双时点报告指引 / null 观察期 / config 不扩展 / 零计划绿过不治）。本设计补充以下定稿：

| # | 决策 | 定稿 | 理由 | 被拒绝的备选方案 |
|---|------|------|------|------------------|
| E1 | vcs 缝形态（design 定稿点 1） | `change_edit_set(root, base) -> Result<Vec<String>, String>` 与 `change_edit_patch(root, base) -> Result<String, String>` 同居 `infra/vcs/src/git.rs`；经既有 `ArchiveVcsPort` 扩两个结构化方法注入编排侧 | 清单与补丁同居 vcs 单点满足「唯一入口」（全仓取 diff 触点仅此一处）；`ArchiveVcsPort` 已是 walker 第三缝且已注入 `LocalToolSteps`，扩方法零新缝零构造面涟漪 | 并列新 port（构造面 / 组合根涟漪，无语义收益）；复用 `DiffContextPort`（产出是拼接文本非结构化清单，且未接线）；checks 侧直 spawn git（越 vcs 边界，违反 checks-runtime 依赖面不扩 vcs） |
| E2 | untracked 进 `--in-diff` 口径（design 定稿点 2） | `change_edit_patch` 内对 untracked 面先行 `git add -N`（intent-to-add）再 `git diff <base>` 出补丁——单次执行、行级精度统一 | intent-to-add 不改文件内容、不改 worktree 语义（worktree 为 change 私有执行锚，归档 `commit_all` 本就 `add -A` 收编）；`change_edit_set` 清单口径幂等兼容（diff 面与 ls-files 面并集去重结果一致） | `--file` 补充双轨执行：两次 spawn、两套报告归并、速率估算口径分裂；合成 new-file patch（自造 diff 格式，易与 git 语义漂移） |
| E3 | `MutationRunner` 签名显式化（回应 AC-3） | `run(root, name, files, patch)` 四参——`files` = 清单入参、`patch` = `--in-diff` 补丁文本，两者同经 vcs 缝取数后由 `steps.rs` 注入 | AC-3 三参记法的要点是「收清单入参、checks 侧零 git」；`--in-diff`（AC-4）必须有补丁面，补丁同样只能从 vcs 缝来——四参是三参记法在补丁维度的显式化，checks-runtime 依赖面照旧不扩 vcs | 三参 + checks 侧自取 diff（越界）；三参 + 补丁走独立 port（多一缝）；`--file` 代替 `--in-diff`（与 AC-4 行级精度要求相悖） |
| E4 | 纯层 / 执行层落位 | 段报告宽容解析 + outcome 映射表落 `core/checks/src/model.rs`；score 公式 / 段计数归并 / 逐 suite 阈值判定落 `core/checks/src/aggregate.rs`；分段调度 / 模板展开 spawn / 写盘 / sandbox 复用清理落 `infra/checks/src/mutation/` | 映射表与归并纯函数 fixtures 直驱可测（零 spawn 零 Tauri 红线不变）；spawn 只在 checks 执行层 | 映射表放 infra（corpus fixtures 直驱难）；解析放 infra（同理） |
| E5 | 工具不可达处置（design 定稿点 3，风险 1） | cargo-mutants / npx stryker **程序不可达、配置不支持、写盘失败** → runner 显式 `Err` 停给用户（与 WorkerAgent 失败同等呈现，walker `Terminal`）；**框架无突变执行器**（node-test 等）→ 该 suite 跳过记 evidence，MUST NOT Err | 环境缺工具是显式可修问题，静默跳过会掩盖突变面缺失（null 观察期已容忍未跑，再静默即无信号）；「框架设计上无执行器」与「工具没装」语义不同，前者跳过留痕 | 工具不可达也跳过留痕（环境问题被 null 观察期吞掉，mutation 恒 null 复现）；工具不可达升格相位 fail（无修复指引，停给用户更直接） |
| E6 | 段级复用门判据 | 段报告在位 && 段报告文件 mtime ≥ 该段输入（段内文件 + suite 测试文件）最新 mtime → 复用（零 spawn，重解析段报告归并）；否则失效重跑 | 与 testexec `is_reusable` 同构（时间戳 vs 最新输入 mtime）；判据借用段报告自身 mtime，零旁置状态文件 | 旁置 meta 状态文件记输入快照（多一真相源，崩溃时序下易漂移）；全量重跑（违背 AC-8） |
| E7 | `MutationTestOutcome` 载荷形态 | `conclusion` + `skip: Option<MutationSkipReason>` + `score` / `threshold`（归并块口径）+ `counts: MutationCounts` + `report_paths: Vec<String>` + `feedback_text: String`（零 Serialize）；不入 exec 输出记录（`exec_id` 不设） | 载荷面照 proposal 枚举；反馈边②报告指引式下原始报告即全量权威，重复落 exec 记录无回查价值；`score` / `threshold` 沿 `aggregate_mutation` 归并块口径（首块阈值，风险 6 口径差留痕），逐 suite 明细在 `feedback_text` 内 | 载荷携逐 suite 结构化数组（反馈 prompt 装配面复杂化）；载荷含存活突变体清单（与反馈边②口径相悖） |
| E8 | evidence 三态映射 | `skip=None` → 「跑过且达标（score X ≥ threshold Y）」；`skip=EmptyDiff` → 「diff 空跳过」；`skip=NoExecutor` 归「null（未跑）」态并附注「无突变执行器」；`mutation_outcome=None` → 「null（未跑）」 | AC-7 三态与 summary mutation 位语义一一对应（跑过=块在位；两种跳过=null）；NoExecutor 全跳过时 mutation 位为 null，归 null 态口径自洽 | NoExecutor 单列第四态（超出 AC-7 三态约定） |
| E9 | 突变常量参数（D9 落点） | 常量落 `infra/checks/src/mutation/` 模块内：`MUTATION_JOBS_DIVISOR = 2`（`max(1, available_parallelism()/2)`）、`PER_MUTANT_TIMEOUT_SECS = 300`、`SEGMENT_BUDGET_MS = 300_000`（5 分钟目标）、`SEGMENT_GUARD_MS = 600_000`（10 分钟软护栏）、`EST_MUTANTS_PER_FILE = 10`（首段静态初值密度） | 参数写常量观察（D9 不进 config schema）；初值依 explore 历史实测折算，观察期代码评审校准 | 进 `suite.mutation` 字段（schema 扩展，D9 已拒） |
| E10 | 文件 → suite 归桶 | 突变面文件按 suite.root POSIX 前缀归桶；桶内文件构该 suite 突变面（js 族喂 Stryker `mutate`、rust 喂 `--in-diff` 切片）；不落任何 suite 桶的文件不突变；全空 → `EmptyDiff` 跳过 | suite 是阈值与执行器的归属单元（逐 suite 阈值判定）；前缀归桶与 config `root` 语义一致 | 全清单喂每个 suite（跨 suite 重复突变、阈值归属混乱）；LCA 单桶（`MutationConfig.cwd` 的 LCA 语义只服务执行目录，不解决归属） |
| E11 | summary mutation 位回填 | `infra/checks/src/mutation/` 在归并后**在位时**回写 `summary.json` 的 `mutation` 位（归并 MutationBlock）；`SubReport.mutation` 恒 `None` 不动（testexec 零逻辑改动，仅注释口径随动） | spec 契约「归并块回填 summary mutation 位」；testexec 链不产突变结果（拆步语义），`build_sub_report` 硬编码 `None` 改为语义注记即可 | SubReport 逐份回填（要重写逐 plan 子报告，收益低）；walker 回填（写盘归执行层） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 步词汇封闭集扩展 | `StepNode::MutationTest`（线词 `mutation_test`、`group()` 归 `Exec`）/ `RunStepKind::MutationTest` / `StepKind::MutationTest`（命令包络 7→8 臂）三枚举扩展 | `crates/core/workflow/src/state.rs` | `specta` / `serde`（既有 derive） | Rust（纯枚举） |
| 相位步序列单源 | `TEST_EXECUTION_STEPS` 改 `[PhaseStart, TestExecution, MutationTest, PhaseLog]`，MutationTest `on_fail=Some(OnFail::FeedbackLoop { limit: FEEDBACK_LOOP_LIMIT })`；requirement / test-only 两表共用同一 static | `crates/core/workflow/src/write/phase_table.rs` | `crate::state` | Rust（编译期 const 表） |
| 编排步词汇与映射 | `ChangeStepKind::MutationTest`（camelCase 线词 `mutationTest`）+ `as_str` + `run_history::mapped_step` 19 臂 1:1 映射 | `crates/core/orchestration/src/state.rs` + `run_history.rs` | `workflow::state` | Rust（纯枚举 + 映射） |
| 突变步三件套 port | `ToolCommand::MutationTest { change_id }` / `ToolStepOutput::MutationTest(MutationTestOutcome)` / `MutationRunner` port（`run(root, name, files, patch)`）；载荷类型自持（零 Serialize、零 checks 依赖） | `crates/core/orchestration/src/port.rs` | 无（纯类型 + trait） | Rust |
| 清单 / 补丁封装单点 | `change_edit_set`（`git diff <base> --name-only -z` + `git ls-files --others --exclude-standard -z`，排序去重 POSIX 归一）与 `change_edit_patch`（untracked `git add -N` + `git diff <base>` 补丁文本）；`ArchiveVcsPort` 扩两结构化方法 + `ProcessArchiveVcs` 实现 | `crates/infra/vcs/src/git.rs` + `lib.rs` | git 进程 spawn（vcs 边界既有） | Rust（spawn，禁 Tauri） |
| 突变纯层判定 | 段报告宽容解析 + outcome 映射表（`caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`）落 `model.rs`；score 公式 / 段计数归并（`aggregate_mutation` 既有公式抽出共用）/ 逐 suite 阈值判定落 `aggregate.rs` | `crates/core/checks/src/model.rs` + `aggregate.rs` | `serde_json`（既有） | Rust（纯函数，fixtures 直驱、零 spawn 零 Tauri） |
| 突变进程执行模块（新） | `ProcessMutation`（`MutationRunner` impl）主链：suite 归桶 → 跳过面 → 分段（静态初值 + 自适应修正）→ 逐段 spawn（Stryker / cargo-mutants 模板展开）→ 段级原始报告写盘 → 段级复用门 → 计数归并 → 归并块写盘 + summary mutation 位回填 → outcome / `feedback_text` 装配；sandbox 跨段复用（Stryker 共享 `tempDirPath`、cargo-mutants 共享 `CARGO_TARGET_DIR`）与全段后一次清理 | `crates/infra/checks/src/mutation/{mod,segments,stryker,cargo_mutants,report}.rs` | `checks`（纯层）、`config`、`foundation::layout`、`orchestration::port`、`exec`（既有面）；**零 vcs 依赖**（清单 + 补丁经入参注入） | Rust、tokio process（spawn 禁 Tauri） |
| 工具步接线 | `ToolCommand::MutationTest` 分支：id → 记录 → name 解析 + `vcs.change_edit_set` / `change_edit_patch` 取数注入 + `StepKind::MutationTest` 审计落行 | `crates/core/orchestration/src/steps.rs` | `port`、`workflow::state` | Rust（async） |
| walker 拆步与反馈边② | `run_step` 增 `StepNode::MutationTest` 臂；`step_mutation_test` 双时点反馈（Fail / Error 走 `feedback_limit(step, "mutation-test")` 独立计数、超限代写 fail checklist 升格）；`PhaseCursor` 增 `mutation_outcome` 槽；`test_execution_phase_log_input` 双槽 + evidence 三态 | `crates/core/orchestration/src/walker.rs` | `port`、`state` | Rust（async） |
| 组合根装配 | `LocalToolSteps::new` 增 `mutation` 缝入参，注入 `ProcessMutation::new()` | `src/commands/change_flow/mod.rs` | `checks_runtime`、`orchestration` | Rust、tauri |
| IPC 类型面 | `ChangeStepKind` / `RunStepKind` / `StepKind` 新词随 tauri-specta 再生出线（bindings 一致性守卫既有纪律承载） | `src/types/generated/bindings.ts` | — | codegen |
| 前端步词汇归一 | `normalizeKind` / `stepWord` 扩 `mutation_test` ↔ `mutationTest`；`STEP_LABEL` 穷尽表补 `mutationTest` 文案键（编译期强制）；`mutationTest` 经 `groupOf` default 落 `toolStep`（exec 组），`GROUP_LABEL` 零改动 | `src/views/changes/flow/run-state.ts` + `run-step-node.tsx` | `types/dto` | TypeScript（纯函数 + React 渲染薄层） |
| 版本交付 | `version` 0.5.1 → 0.5.2（归档时执行） | `packages/desktop/package.json` | — | package.json |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs` | 突变执行模块主链：`ProcessMutation`（`MutationRunner` impl）+ `execute`（suite 归桶 / 跳过面 / 分段调度 / 归并 / summary 回填 / outcome 与 `feedback_text` 装配）+ 突变常量参数（E9）。见「公共函数 / API」`ProcessMutation` |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/segments.rs` | 分段切分与自适应修正：首段静态初值（`EST_MUTANTS_PER_FILE` 密度折算）→ 字典序均分 k 段，后续段按上一段实测速率（秒 / mutant）动态调段大小；`--in-diff` 补丁按 `diff --git` 文件段切片为 `mutation-seg-<i>.patch`（Stryker 侧为 mutate 清单切片） |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs` | Stryker 执行面：临时 config 写盘（`mutate` = 段清单、`tempDirPath` = `<mutation 目录>/_stryker-tmp` 全段共享、`concurrency` = `max(1, available_parallelism()/2)`）、`npx --prefix <suite cwd> stryker run <config>` 模板展开 spawn（超时 + 输出捕获）、全段后一次清理（只清 `_stryker-tmp` 与临时 config） |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs` | cargo-mutants 执行面：`cargo mutants --in-diff <seg patch> --json --baseline skip --jobs <max(1, available_parallelism()/2)> --timeout <PER_MUTANT_TIMEOUT_SECS>` 模板展开 spawn（超时 + 输出捕获）、`CARGO_TARGET_DIR` 全段共享（增量编译一次全量） |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/report.rs` | 段级与归并报告写盘 + 段级复用门：段原始报告 `mutation-seg-<i>.json` 写盘（全程留存）、归并块 `mutation.json` 写盘、复用判据（段报告在位且 mtime ≥ 段输入最新 mtime → 零 spawn 复用） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | `StepNode` 增 `MutationTest` 变体（文档注记 18→19 值封闭集）；`StepNode::as_str` 增 `"mutation_test"` 臂；`StepNode::group` 增 `MutationTest` 归 `StepGroup::Exec`；`RunStepKind` 增 `MutationTest` 变体 + `as_str` 臂（文档注记 18→19）；`StepKind` 增 `MutationTest` 变体 + `as_str` 臂（命令包络 7→8 臂） | 步词汇封闭集扩展（AC-2；desktop-change-state-store / desktop-workflow-write-face 词汇契约） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | `TEST_EXECUTION_STEPS` 在 TestExecution 与 PhaseLog 间插 `StepDefinition { node: StepNode::MutationTest, prompt_ref: None, on_fail: Some(OnFail::FeedbackLoop { limit: FEEDBACK_LOOP_LIMIT }) }`；注释改写（拆步语义：测试 / 覆盖率修绿后才跑突变；requirement / test-only 两表共用同一 static 天然同式） | 拆步相位表（AC-1） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | `ChangeStepKind` 增 `MutationTest` 变体（camelCase 线词 `mutationTest`）+ `as_str` 增 `"mutation_test"` 臂；文档注记 18→19 值 | 编排步词汇（AC-2） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` | `mapped_step` 增 `ChangeStepKind::MutationTest => RunStepKind::MutationTest` 臂（文档注记 19 臂 1:1） | 落库映射随动（编译期强制；desktop-change-state-store 词汇同源契约） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `ToolCommand` 增 `MutationTest { change_id: String }` 变体；`ToolStepOutput` 增 `MutationTest(MutationTestOutcome)` 变体；新增 `MutationTestConclusion` / `MutationSkipReason` / `MutationCounts` / `MutationTestOutcome` 类型与 `MutationRunner` trait（见「公共函数 / API」「类型定义」）；`ArchiveVcsPort` 增 `change_edit_set` / `change_edit_patch` 两结构化方法（清单 / 补丁注入面） | 突变步三件套 + vcs 缝扩展（AC-3；E1 / E3 / E7） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `LocalToolSteps` 增 `mutation: Arc<dyn MutationRunner>` 字段与 `new` 入参；`execute` 增 `ToolCommand::MutationTest` 分支：id → 记录 → name 解析（与 test-execution 同式）→ `vcs.change_edit_set(root, "HEAD")` + `vcs.change_edit_patch(root, "HEAD")` → `mutation.run(&root, &name, &files, &patch)` → `StepKind::MutationTest` 审计行（摘要携 conclusion / score / threshold） | id → name 接线 + 清单注入（AC-3） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `PhaseCursor` 增 `mutation_outcome: Option<MutationTestOutcome>` 槽；`run_step` 增 `StepNode::MutationTest => step_mutation_test(...)` 臂；新增 `step_mutation_test`（双时点：Pass → `cursor.mutation_outcome` 落槽 Proceed；Fail / Error → 反馈 prompt 只含 `outcome.feedback_text`（结论摘要 + 报告路径清单），`feedback_limit(step, "mutation-test")` 独立计数，超限代写 fail checklist 升格相位 fail）与 `step_mutation_test_upgraded_phase_log`（照 `step_test_execution_upgraded_phase_log` 同式）；`test_execution_phase_log_input` 签名增 `mutation: Option<&MutationTestOutcome>` 双槽入参，第 3 条 evidence 按 E8 三态组装（判定语义「mutation null 自动通过」恒 pass 不变） | 拆步接线 + 双时点反馈边② + 机械 checklist 双槽（AC-6 / AC-7） |
| `packages/desktop/src-tauri/crates/core/checks/src/model.rs` | mutation 块族注释改写（「生产聚合恒 None」→「突变位由 MutationTest 归并回填；本域提供段级解析与归并模型」）；新增 `MutationTool` / `MutationSegReport` 类型与 `parse_cargo_mutants_seg` / `parse_stryker_seg` 宽容解析函数（内置 outcome 映射表，`MutationMeasured` 既有字段零扩展） | 段级报告纯层模型 + 映射表（AC-4；E4） |
| `packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs` | `aggregate_mutation` 的计数求和公式抽出为 `merge_mutation_blocks`（`build_mutation_block` 承 score 公式与阈值判定），`aggregate_mutation` 改调共用实现（公式逐字不变，注释改写为生产启用口径）；新增 `build_mutation_block` / `merge_mutation_blocks` 纯函数出口（供 mutation 执行模块归并段计数与逐 suite 判定） | 归并公式共用（AC-4 / AC-5；MUST NOT 新增归并公式） |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` | 仅注释口径随动：`build_sub_report` 的 `mutation: None` 注释由「V1 不移植」改为「突变面由独立 MutationTest 步承载并回填 summary mutation 位；本链不产突变结果」；零逻辑改动 | 拆步语义留痕（desktop-test-execution 模块契约「突变面移出」） |
| `packages/desktop/src-tauri/crates/infra/checks/src/lib.rs` | 增 `pub mod mutation;` 与 `pub use mutation::ProcessMutation;` | 执行模块出线（组合根装配面） |
| `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` | 新增 `change_edit_set(root, base)` 与 `change_edit_patch(root, base)`（见「公共函数 / API」）；复用既有 `git(...)` spawn 助手与 `-z` 解析纪律 | 清单 / 补丁单点封装（AC-3；E1 / E2） |
| `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs` | `impl ArchiveVcsPort for ProcessArchiveVcs` 增 `change_edit_set` / `change_edit_patch` 两方法（转发 `git.rs` 自由函数） | port 实现随动 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | `LocalToolSteps::new(...)` 增传 `Arc::new(ProcessMutation::new())`（位于 `ProcessTestExecution` 之后）；import 随动 | 组合根装配 |
| `packages/desktop/src-tauri/src/types/generated/bindings.ts` | 随 `ChangeStepKind` / `RunStepKind` / `StepKind` 新词经 `bindings:export` 再生（非手改）：`ChangeStepKind` 增 `"mutationTest"`、`RunStepKind` / `StepKind` 增 `"mutation_test"` | 类型再生跟随（AC-2 / AC-9） |
| `packages/desktop/src/views/changes/flow/run-state.ts` | `normalizeKind` 增 `case 'mutation_test': return 'mutationTest';`；`stepWord` 增 `case 'mutationTest': return 'mutation_test';`；注释「十八值」→「十九值」 | 步词汇归一表扩展（AC-2） |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `STEP_LABEL: Record<ChangeStepKind, string>` 增 `mutationTest: '突变执行'` 键（穷尽表随新词编译期强制；`GROUP_LABEL` / 渲染逻辑零改动——`mutationTest` 经 `groupOf` default 落 `toolStep` exec 组） | 白名单式扩展的涟漪面（AC-2「零渲染改动」指渲染逻辑，文案键为穷尽表强制项） |
| `packages/desktop/package.json` | `version` 0.5.1 → 0.5.2（归档时执行） | 版本交付（AC-11） |

> **测试文件**（proposal「变更范围 - 测试文件」）由 test-design / test-gen / test-execution 相位承接，本清单不展开。它们包括：`core/workflow/src/state_test.rs` / `write/phase_table_test.rs`、`core/orchestration/src/{port_test,walker_test}.rs`、`core/checks/src/aggregate_test.rs` 与 `tests/corpus_golden_test.rs`（映射表样本 + 段级报告 fixtures + golden 对拍）、`infra/checks/src/mutation/*_test.rs`、`infra/vcs/src/git_test.rs`、`src/views/changes/flow/run-state.test.ts`；golden / 语料（十九值 nodes 投影、`mutation-block` 段级 fixtures）走 `DESKTOP_GOLDEN_REWRITE=1` 显式重写。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `change_edit_set` | `crates/infra/vcs/src/git.rs` | 新增 | `pub fn change_edit_set(root: &Path, base: &str) -> Result<Vec<String>, String>` | 变更文件清单唯一入口：`git diff <base> --name-only -z` ∪ `git ls-files --others --exclude-standard -z`（untracked 并入），排序去重 POSIX 归一；`base = "HEAD"` 时即本 change 编辑集（worktree 语义）；git 命令失败显式 `Err`（MUST NOT 静默降级空清单——空清单会误走 diff 空跳过）。spec 记法 `-> Vec<String>` 为成功面记法，错误通道显式化 |
| `change_edit_patch` | `crates/infra/vcs/src/git.rs` | 新增 | `pub fn change_edit_patch(root: &Path, base: &str) -> Result<String, String>` | `--in-diff` 补丁唯一入口：untracked 面先行 `git add -N`（intent-to-add，E2）再 `git diff <base>` 输出 unified diff 文本；与 `change_edit_set` 同居 vcs 单点，checks 侧零 git spawn |
| `ArchiveVcsPort.change_edit_set` | `crates/core/orchestration/src/port.rs` | 修改 | `fn change_edit_set(&self, root: &Path, base: &str) -> Result<Vec<String>, String>` | port 缝结构化清单取数（walker 第三缝扩方法）；实现 `ProcessArchiveVcs` 转发 `git.rs` |
| `ArchiveVcsPort.change_edit_patch` | `crates/core/orchestration/src/port.rs` | 修改 | `fn change_edit_patch(&self, root: &Path, base: &str) -> Result<String, String>` | port 缝补丁取数；同上 |
| `MutationRunner`（trait） | `crates/core/orchestration/src/port.rs` | 新增 | `pub trait MutationRunner: Send + Sync { fn run(&self, root: &str, name: &str, files: &[String], patch: &str) -> BoxToolFuture; }` | 突变 spawn 缝（checks 边界 infra 实现；与 `TestExecutionRunner` 并列同式）：`root` = exec root（worktree）、`name` = change 磁盘名（报告目录 `change_test_reports(root, name)` 派生）、`files` = 清单入参、`patch` = 补丁入参（E3）；产出 `Result<ToolStepOutput, String>`（`ToolStepOutput::MutationTest`） |
| `ProcessMutation::new` | `crates/infra/checks/src/mutation/mod.rs` | 新增 | `pub fn new() -> Self` | 构造（组合根装配，无状态）；`Default` 同式 |
| `ProcessMutation.run` | `crates/infra/checks/src/mutation/mod.rs` | 新增 | `fn run(&self, root: &str, name: &str, files: &[String], patch: &str) -> BoxToolFuture` | `MutationRunner` 实现：执行主链（E5 显式 `Err` 面：程序不可达 / 配置不支持 / 写盘失败） |
| `parse_cargo_mutants_seg` | `crates/core/checks/src/model.rs` | 新增 | `pub fn parse_cargo_mutants_seg(raw: &str) -> Result<MutationSegReport, String>` | cargo-mutants `--json` 段报告宽容解析 + 映射表（`caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`）；未知 outcome 计 `runtimeError` 并保留原文（宽容不丢数据） |
| `parse_stryker_seg` | `crates/core/checks/src/model.rs` | 新增 | `pub fn parse_stryker_seg(raw: &str) -> Result<MutationSegReport, String>` | Stryker mutation 段报告宽容解析（outcome 词与 `MutationMeasured` 1:1）；同上宽容纪律 |
| `build_mutation_block` | `crates/core/checks/src/aggregate.rs` | 新增 | `pub fn build_mutation_block(measured: MutationMeasured, threshold: f64) -> MutationBlock` | score 公式单点：`(killed+timeout)/(total-ignored-compileError-runtimeError)`（分母 0 → 100.0，两位舍入）+ 阈值判定 `pass = score >= threshold`；与 `aggregate_mutation` 逐字同式 |
| `merge_mutation_blocks` | `crates/core/checks/src/aggregate.rs` | 新增 | `pub fn merge_mutation_blocks(blocks: &[MutationBlock]) -> Option<MutationBlock>` | 段计数求和归并（段当子报告）→ `build_mutation_block`（阈值取首块，`aggregate_mutation` 既有语义）；`aggregate_mutation` 改调本函数（MUST NOT 新增归并公式） |
| `LocalToolSteps::new` | `crates/core/orchestration/src/steps.rs` | 修改 | `pub fn new(anchors: Arc<SessionAnchors>, static_check: Arc<dyn StaticCheckRunner>, test_execution: Arc<dyn TestExecutionRunner>, mutation: Arc<dyn MutationRunner>, vcs: Arc<dyn ArchiveVcsPort>, store: Arc<dyn ChangeStateStore>, run_id: String) -> Self` | 增 `mutation` 缝入参（位于 `test_execution` 之后） |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `StepNode` | `crates/core/workflow/src/state.rs` | 修改 | 增 `MutationTest`（线词 `mutation_test`；19 值封闭集；`group()` → `StepGroup::Exec`） |
| `RunStepKind` | `crates/core/workflow/src/state.rs` | 修改 | 增 `MutationTest`（线词 `mutation_test`；19 值封闭集，落库 / 查询投影同词面） |
| `StepKind` | `crates/core/workflow/src/state.rs` | 修改 | 增 `MutationTest`（线词 `mutation_test`；相位机步骤审计 7→8 臂命令包络） |
| `ChangeStepKind` | `crates/core/orchestration/src/state.rs` | 修改 | 增 `MutationTest`（camelCase 线词 `mutationTest`；与 `RunStepKind` 1:1 同源） |
| `ToolCommand` | `crates/core/orchestration/src/port.rs` | 修改 | 增 `MutationTest { change_id: String }` 变体（载荷恒 change **id**，寻址纪律同型） |
| `ToolStepOutput` | `crates/core/orchestration/src/port.rs` | 修改 | 增 `MutationTest(MutationTestOutcome)` 变体（零 Serialize 不进 IPC） |
| `MutationRunner` | `crates/core/orchestration/src/port.rs` | 新增 | 突变 spawn 缝 trait（签名见「公共函数 / API」）；port 归消费者（`orchestration::port`），core/orchestration 零依赖 checks |
| `MutationTestConclusion` | `crates/core/orchestration/src/port.rs` | 新增 | 三值：`Pass`（逐 suite 全达标）/ `Fail`（全部段跑完但有 suite 未达标）/ `Error`（段中断：段执行 error / harness 崩 / 超时）；`as_str` 线格式小写词（`TestExecutionConclusion` 同型） |
| `MutationSkipReason` | `crates/core/orchestration/src/port.rs` | 新增 | 两值：`EmptyDiff`（突变面为空）/ `NoExecutor`（全部 suite 无突变执行器）；`Option` 缺省 = 已执行 |
| `MutationCounts` | `crates/core/orchestration/src/port.rs` | 新增 | 归并计数载荷：`killed / survived / timeout / compile_error / runtime_error / ignored / total: u64`（`MutationMeasured` 计数面的 port 流量像；自持定义——core/orchestration 零依赖 checks） |
| `MutationTestOutcome` | `crates/core/orchestration/src/port.rs` | 新增 | 载荷 struct：`conclusion: MutationTestConclusion` + `skip: Option<MutationSkipReason>` + `score: f64` / `threshold: f64`（归并块口径；skip 时置 0.0 占位）+ `counts: MutationCounts` + `report_paths: Vec<String>`（段级 + 归并报告路径清单，POSIX）+ `feedback_text: String`（结论摘要 + 报告路径指引，pass 恒空串；MUST NOT 内联存活突变体清单）；零 Serialize |
| `MutationTool` | `crates/core/checks/src/model.rs` | 新增 | 执行器标识两值：`CargoMutants` / `Stryker`（解析分支与 evidence 归因用） |
| `MutationSegReport` | `crates/core/checks/src/model.rs` | 新增 | 段级解析纯层模型：`tool: MutationTool` + `measured: MutationMeasured` + `duration_ms: f64`；`MutationMeasured` / `MutationBlock` 既有字段**零扩展**（schema 双侧对齐不变，corpus `mutation-block` fixture 口径不变） |
| `RunStepKind` / `StepKind`（TS bindings） | `src/types/generated/bindings.ts` | 修改 | 增 `"mutation_test"` 词（随再生出线，非手改） |
| `ChangeStepKind`（TS bindings） | `src/types/generated/bindings.ts` | 修改 | 增 `"mutationTest"` 词（随再生出线） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.5.2` | 0.5.1 → 0.5.2（归档时执行；`src-tauri/tauri.conf.json` 维持 `../package.json` 引用自动跟随，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 零改动） |

> **config schema 零扩展**（D9 / E9）：`suite.mutation` 维持 `{ cwd?, score }` 既有形态（desktop 80 / src-tauri 70 原样消费）；jobs / per-mutant timeout / 段预算 / 分段目标 / 静态密度均为 `infra/checks/src/mutation/` 模块内代码常量（`MUTATION_JOBS_DIVISOR = 2`、`PER_MUTANT_TIMEOUT_SECS = 300`、`SEGMENT_BUDGET_MS = 300_000`、`SEGMENT_GUARD_MS = 600_000`、`EST_MUTANTS_PER_FILE = 10`），MUST NOT 进 schema；`openspec/config.json` 零改动。

---

## 数据模型

### 步词汇封闭集（枚举词面扩展，零 model 版本演进）

- `StepNode` / `RunStepKind` / `ChangeStepKind` 18→19 值（增 `mutation_test` / `mutationTest`），`StepKind` 7→8 臂（增 `mutation_test`）；均为枚举序列化词面扩展（非字段形态变化），`RunNodeRecord` / `StepRecord` 零 native_model 版本演进、零迁移。
- 1:1 全映射链：`StepNode::MutationTest` → `ChangeStepKind::MutationTest`（`run_history::mapped_step`）→ `RunStepKind::MutationTest` → bindings `RunStepKind "mutation_test"` → 前端 `normalizeKind` / `stepWord` 归一。

### 突变报告面（磁盘，无数据库持久化）

报告树（`foundation::layout::change_test_reports(root, name)` 派生，零路径字面量；逐 plan 子树）：

```text
reports/test/<plan>/mutation/
  mutation-seg-<i>.json        # 段级原始报告（cargo-mutants --json / Stryker mutation 报告原样，全程留存）
  mutation-seg-<i>.patch       # 段级 --in-diff 补丁（cargo-mutants 面；Stryker 不产）
  mutation.json                # 归并块（MutationBlock：计数求和 + score 公式 + 首块阈值）
  mutation-stryker-seg-<i>.conf.json   # Stryker 临时 config（全段后一次清理）
  _stryker-tmp/                # Stryker 共享 sandbox（全段后一次清理）
reports/test/summary.json      # mutation 位由归并块回填（在位时；未跑 / 跳过保持 null）
```

- **段级报告** `mutation-seg-<i>.json` = 工具原始输出（E6 复用门判据借用其文件 mtime）；解析（映射表）经 `parse_cargo_mutants_seg` / `parse_stryker_seg` 纯函数按需重放（复用零 spawn、解析零成本）。
- **归并块** `mutation.json` = `MutationBlock`（schema 零扩展）：`merge_mutation_blocks` 计数求和 + `build_mutation_block` score 公式；跨 suite 再归并回填 summary mutation 位（阈值取首块——`aggregate_mutation` 既有语义；**门禁结论以逐 suite 全过为准**，风险 6 口径差留痕观察）。
- **清理边界**：全段完成后一次清理只删 `_stryker-tmp/` 与 `mutation-stryker-seg-*.conf.json`；段级 / 归并报告 MUST NOT 触碰。

### 段级复用门（断点续跑）

- 判据（E6）：`mutation-seg-<i>.json` 在位 && 其文件 mtime ≥ max(段内文件 mtime ∪ suite 测试文件 mtime) → 复用该段（零 spawn，段报告重解析进归并）；否则该段失效重跑。
- 中断 / 重试只重跑未完段与失效段；已完段零 spawn（AC-8）。

### 段切分与自适应

- 触发：估算突变体量（`EST_MUTANTS_PER_FILE × 文件数`）× 预期速率（1~2 s/mutant）> `SEGMENT_BUDGET_MS` → 切 k 段；k = ceil(估算耗时 / 段预算)。
- 切分：文件字典序均分；单文件超预算接受（段是软目标，`SEGMENT_GUARD_MS` 仅观察留痕，MUST NOT 硬失败）。
- 自适应：首段静态初值；后续段按上一段实测速率（秒 / mutant）修正段大小。
- 补丁切片：`patch` 文本按 `diff --git` 文件段头切片，段 i 的补丁只含段 i 文件的 hunk（cargo-mutants `--in-diff`）；Stryker 面为 `mutate` 清单切片。

### 载荷与持久化边界

- `MutationTestOutcome` 零 Serialize：只在进程内 walker 反馈边与机械 checklist 消费，MUST NOT 经 bindings / `RunUpdate` 出线；全量明细留报告文件（报告指引式，反馈边②口径）。
- 零新表、零字段形态变化；`ChangeRecord` / `PhaseRecord` / `RunNodeRecord` / `RunEdgeRecord` 面零 delta（词汇枚举随动除外）。

---

## 路由 / API 设计

<!-- 不涉及：本变更零 IPC 命令新增 / 签名改动（ToolStepOutput 载荷零 Serialize 不进 IPC；RunUpdate::Step 形态不变）；bindings 再生仅类型词汇面（ChangeStepKind / RunStepKind / StepKind 新词）。 -->

---

## 依赖

### 运行时依赖

- **外部工具（checks 边界进程 spawn）**：
  - `cargo-mutants`（rust 突变执行器；`cargo install cargo-mutants` 供给）——不可达显式 `Err` 停给用户（E5）。
  - `npx stryker`（js 族突变执行器，沿 CLI `npx --prefix "{prefix}" stryker run "{config}"` 现状口径）——不可达显式 `Err`（E5）。
  - `git`（vcs 边界既有 spawn；`change_edit_set` / `change_edit_patch` / `git add -N`）。
- **零新增 crate / npm 依赖**：`serde_json`（段报告解析）、tokio process（spawn）、`time`（时间戳）、`exec`（输出记录）均为既有依赖面。
- **crate 依赖方向**：`infra/checks`（checks-runtime）→ `checks` / `config` / `foundation` / `orchestration::port` / `exec`（既有）；**MUST NOT 新增对 `vcs` 的依赖**——清单 + 补丁经 `MutationRunner::run` 入参注入（AC-3）；`core/orchestration` 零依赖 `checks`（载荷类型自持）。

### 构建 / 测试依赖

- `pnpm -C packages/desktop run bindings:export`（`cargo run -p export-bindings`）再生三词汇新词；`bindings:check` 一致性守卫拦截漂移（AC-9）。
- golden / 语料走 `DESKTOP_GOLDEN_REWRITE=1` 显式重写（desktop-corpus-regression 既有纪律；AC-9）。
- 静件管线（`client:check` / `pnpm test` / `cargo test --workspace`）由桌面端执行，agent 内不重复执行（AC-10 为归档验收面）。

---

## 待决问题

- **突变常量初值校准**（非阻塞）：`PER_MUTANT_TIMEOUT_SECS = 300` 与 `EST_MUTANTS_PER_FILE = 10` 为观察初值（E9），待桌面突变首轮实测（rust 突变体速率 / 每文件密度）后经代码评审校准；段预算 5 分钟 / 软护栏 10 分钟已定案不动。
- **Stryker 供给方式**（非阻塞）：js 族沿 CLI `npx --prefix` 口径（suite cwd 的 node_modules 需可解析 stryker），desktop workspace 是否显式加 `@stryker-mutator/core` devDependency 待首轮运行观察再定；当前口径下不可达即显式 `Err`（E5）。
- **多 suite 归并块阈值口径差**（风险 6 留痕观察）：summary mutation 位归并块阈值取首块 vs 门禁结论逐 suite 全过——单 suite 场景一致，多阈值并存（desktop 80 / src-tauri 70）时报告位与结论面口径差**留痕观察**，MUST NOT 擅自统一。
- **两反馈边口径并存防误统一**（风险 4，非未决但需常驻警示）：反馈边①（test-execution，findings_detail 有界内联自足）与反馈边②（MutationTest，报告指引式）**反向并存系用户明确决策**，MUST NOT 被后来者「统一」——任何口径合并须经用户重新裁决。
