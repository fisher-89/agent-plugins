# 测试设计: desktop-fix-mutation-test

> **变更**: desktop-fix-mutation-test
> **阶段**: test-design（只设计测试，不写测试命令细节）
> **基于**: proposal.md、design.md（新增文件 / 修改文件 / 公共函数 / API / 类型定义 / 架构组件各表）
> **测试框架识别**：
> - **Rust**：`cargo test`（workspace 根 `packages/desktop/src-tauri`），单元测试为与被测模块同目录的 `*_test.rs`（如 `state_test.rs`、`walker_test.rs`、`mod_test.rs`），断言用 `assert_eq!` / `assert!` / `assert!(matches!(…))`，`#[test]` / `#[tokio::test]` 描述名用中文；port / runner 缝以假实现（`ProgrammableRunner` / `RecordingSteps` 同式）注入；vcs 测试沿 `git_test.rs` 真实 git tempdir 装置（`init_repo` / `fs_write` 助手）；corpus 对拍走 `tests/corpus_golden_test.rs` + fixtures + `DESKTOP_GOLDEN_REWRITE=1` 显式重写。
> - **TypeScript / React**：`vp test`（vitest 内核 + `@testing-library/react` + jsdom），测试文件与被测模块同目录 `*.test.ts` / `*.test.tsx`，断言风格 `import { describe, expect, it } from 'vite-plus/test'`，仅 mock 进程边界（`@tauri-apps/api/core` 的 `invoke` / `Channel`），内部纯函数 / hook 真实组合。
> - **命名惯例**：Rust `{module}_test.rs` 与 TS `{module}.test.{ts,tsx}` 均与被测源文件同目录；无既有测试时按语言惯例推定。
>
> **待测功能条目来源**：优先取 design.md「公共函数 / API」子表行（含签名）；该表无行的文件取「变更清单（新增 / 修改文件）/ 类型定义 / 架构组件」表中已声明的标识符与行为（零虚构、零 grep 源码导出）；design.md 无任何可测行为声明的文件留章节框架并以 HTML 注释说明。
>
> **路径类型取值**：`正向` / `异常` / `边界` / `组合`（跨模块链路用例，按链路发起方 / 最上层调用方挂靠入口模块章节）；**迭代类型取值**：`新增`（本轮新增测试 / 用例）、`改写`（既有测试随行为变化改写）、`保留`（既有用例不变）。每份 `#### 用例` 表对每个 `#### 待测功能` 条目至少覆盖正向 / 异常 / 边界三类。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 拆步相位表：`TEST_EXECUTION_STEPS` = `[PhaseStart, TestExecution, MutationTest, PhaseLog]`；TestExecution fail 走既有反馈边修复重入，绿后才执行 MutationTest（walker 顺序保证）；requirement / test-only 两表同式 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` |
| AC-2 | 步词汇 19 值全映射：`StepNode` / `ChangeStepKind` / `RunStepKind` 含 `MutationTest` / `mutation_test`（1:1 全映射）；bindings 再生后前端零渲染改动，归一表 `mutation_test` ↔ `mutationTest` 在案且归 exec 组 | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/state.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs`、`packages/desktop/src-tauri/src/types/generated/bindings.ts`、`packages/desktop/src/views/changes/flow/run-state.ts`、`packages/desktop/src/views/changes/flow/run-step-node.tsx`、`packages/desktop/src-tauri/crates/core/workflow/tests/golden`（十九值 nodes 投影） |
| AC-3 | `change_edit_set` 唯一入口：跟踪改动 + untracked 并入、排序去重、POSIX 归一；全仓无第二处取 diff / 列清单触点；`MutationRunner::run(root, name, files, patch)` 收清单入参，checks-runtime 依赖面不扩 vcs | `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs`、`packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs`、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` |
| AC-4 | Rust 突变与映射表：cargo-mutants 跑 rust suite（`--in-diff` / `--jobs=half CPU` / per-mutant `--timeout` / `--baseline skip`）；映射 `caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`；score 公式单点；js 族走 Stryker；node-test 等无执行器框架突变跳过记 evidence 不 Err | `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs`、`packages/desktop/src-tauri/crates/core/checks/src/model.rs`、`packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/lib.rs` |
| AC-5 | 分段与 sandbox 复用：估算超段预算（5 分钟目标）自动分段顺序执行、段报告 `mutation-seg-<i>.json` 留存、计数归并沿 `aggregate_mutation`；Stryker 共享 `tempDirPath`、cargo-mutants 共享 `CARGO_TARGET_DIR`；清理仅 `_stryker-tmp` 与临时 config 且全段后一次；单文件超预算不失败 | `packages/desktop/src-tauri/crates/infra/checks/src/mutation/segments.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/mutation/report.rs`、`packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs` |
| AC-6 | 双时点反馈边②：段中断立即反馈（错误摘要 + 报告路径清单）；全部段完成汇总 score，未达标反馈（score / threshold + 报告路径清单）；达标 pass 零反馈；反馈 prompt 零存活突变体内联；独立计数上限 5 且与 static-check / test-execution 分立，超限升格相位 fail | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` |
| AC-7 | null 观察期与双槽：未跑 / diff 空跳过时 `mutation: null` 合法（checklist 第 3 条 / evaluator T3 不改）；PhaseLog 产物面挂 `test_outcome` + `mutation_outcome` 双槽；第 3 条 evidence 三态 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` |
| AC-8 | 段级复用门：段报告在位 + 输入 mtime 未变复用该段（零 spawn）；中断重试只重跑未完段；输入变化段失效重跑 | `packages/desktop/src-tauri/crates/infra/checks/src/mutation/report.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs` |
| AC-9 | 语料与守卫：corpus 含映射表样本与段级报告 fixtures、对拍解析与聚合结论；十九值 nodes 投影 golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写；bindings 一致性守卫绿 | `packages/desktop/src-tauri/crates/core/checks/src/model.rs`、`packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs`、`packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs` 与 fixtures / golden、`packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` 与 golden、`packages/desktop/src-tauri/src/types/generated/bindings.ts` |
| AC-10 | 全管线通过：`client:check` / `pnpm test` / `cargo test --workspace` 全绿；`plugins/dev-team` 零 diff | 上述全部被测文件与测试文件（静态脚本由桌面端执行，本设计不复制运行细节） |
| AC-11 | desktop 版本升级（归档时执行）：`version` 0.5.1 → 0.5.2；`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 不随动 | `packages/desktop/package.json` |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能
- `StepNode::as_str()` / `StepNode::group()`：增 `MutationTest` 变体（线词 `"mutation_test"`、`group()` 归 `StepGroup::Exec`），18→19 值封闭集（类型定义表）。
- `RunStepKind::as_str()`：增 `MutationTest` 变体（线词 `"mutation_test"`），19 值封闭集，落库 / 查询投影同词面（类型定义表）。
- `StepKind::as_str()`：增 `MutationTest` 变体（线词 `"mutation_test"`），命令包络 7→8 臂（类型定义表）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `StepNode::as_str` / `RunStepKind::as_str` | 正向 | 19 值全词汇逐词断言（含 `MutationTest → "mutation_test"`），`StepNode` ↔ `RunStepKind` 逐词同源 1:1（AC-2 全映射） | 改写 |
| `StepNode::group` | 正向 | `MutationTest → StepGroup::Exec`（与 PhaseStart / StaticCheck / TestExecution 同组） | 新增 |
| `StepKind::as_str` | 正向 | 8 臂命令包络逐词断言（含 `MutationTest → "mutation_test"`），相位机步骤审计线词面 | 改写 |
| 三枚举反序列化 | 异常 | 未知线词（枚举外字符串，如 `"mutation"` / `""` / `"MutationTest"` 大小写变体）→ 反序列化显式失败，封闭集不吞词 | 新增 |
| 18 既有词汇 | 边界 | 19 值中除 `mutation_test` 外 18 词线词 / 分组持衡（零回归）；枚举穷尽解构无 `..` 兜底（字段 / 变体增删即编译失败） | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 纯枚举直测（`#[test]` + `assert_eq!` / `assert!(matches!(…))`） | 全部用例零 mock |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能
- `TEST_EXECUTION_STEPS`（修改文件表）：在 `TestExecution` 与 `PhaseLog` 间插 `StepDefinition { node: StepNode::MutationTest, prompt_ref: None, on_fail: Some(OnFail::FeedbackLoop { limit: FEEDBACK_LOOP_LIMIT }) }`，序改 `[PhaseStart, TestExecution, MutationTest, PhaseLog]`；requirement / test-only 两表共用同一 static。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `TEST_EXECUTION_STEPS` 序列 | 正向 | 四步序等值 `[PhaseStart, TestExecution, MutationTest, PhaseLog]`（AC-1）；MutationTest 的 `prompt_ref = None` | 改写 |
| MutationTest `on_fail` 形态 | 正向 | `on_fail = Some(OnFail::FeedbackLoop { limit: FEEDBACK_LOOP_LIMIT })`（limit = 5） | 新增 |
| 两表同式 | 正向 | requirement 与 test-only 两表的 test-execution 相位 steps 同一 static 引用（天然同式，指针 / 逐元素等值双断言） | 新增 |
| 其它相位序列 | 异常 | 其余相位 steps 序与 `on_fail` 形态零改动（MutationTest 插入不影响静态检查 / 归档等相位） | 保留 |
| 序列边界 | 边界 | 步数 3→4、`PhaseLog` 恒末位、`FEEDBACK_LOOP_LIMIT == 5` 常量锚定 | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 编译期 const 表纯函数直测 | 全部用例零 mock |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能
- `ChangeStepKind`（类型定义表）：增 `MutationTest` 变体（camelCase 线词 `"mutationTest"`）+ `as_str` 增臂；18→19 值，与 `RunStepKind` 1:1 同源。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ChangeStepKind` 线词 | 正向 | 19 值 serde 线词逐词断言（含 `MutationTest → "mutationTest"`）+ `as_str` 逐词断言 | 改写 |
| 与 `RunStepKind` 同源 | 正向 | `ChangeStepKind` ↔ `RunStepKind` 1:1 对拍（19 对全等，双射无两词映一） | 新增 |
| 反序列化 | 异常 | 未知线词（`"mutation_test"` snake 词 / `""` / 超长串 / 含 `\n` / emoji 的词）→ 显式失败，camelCase 封闭集不吞词 | 新增 |
| 18 既有词汇 | 边界 | 除 `mutationTest` 外 18 词线词持衡（零回归） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 纯枚举直测 | 全部用例零 mock |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs

#### 待测功能
- `mapped_step`（修改文件表）：增 `ChangeStepKind::MutationTest => RunStepKind::MutationTest` 臂，19 臂 1:1 映射（落库词汇同源契约）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `mapped_step` | 正向 | 19 个 `ChangeStepKind` 逐值映射断言（含 `MutationTest → RunStepKind::MutationTest`） | 改写 |
| `mapped_step` | 异常 | 双射校验：无两个 `ChangeStepKind` 映到同一 `RunStepKind`（词汇同源不漂移） | 新增 |
| `mapped_step` | 边界 | 18 既有臂映射结果持衡（零回归） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 纯函数直测 | 全部用例零 mock |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能
- `ArchiveVcsPort.change_edit_set(&self, root: &Path, base: &str) -> Result<Vec<String>, String>`（公共函数 / API）：port 缝结构化清单取数（walker 第三缝扩方法）。
- `ArchiveVcsPort.change_edit_patch(&self, root: &Path, base: &str) -> Result<String, String>`（公共函数 / API）：port 缝补丁取数。
- `MutationRunner`（trait）（公共函数 / API）：`run(&self, root: &str, name: &str, files: &[String], patch: &str) -> BoxToolFuture`，突变 spawn 缝（与 `TestExecutionRunner` 并列同式）。
- `ToolCommand::MutationTest { change_id }` / `ToolStepOutput::MutationTest(MutationTestOutcome)`（类型定义表）：命令包络扩臂（六→七变体）与载荷变体。
- `MutationTestConclusion` / `MutationSkipReason` / `MutationCounts` / `MutationTestOutcome`（类型定义表）：载荷类型自持（零 Serialize、零 checks 依赖）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ToolCommand` 封闭集 | 正向 | 七变体封闭集且 match 穷尽分发（`tool_command六变体…` 既有用例改写为七变体）；`MutationTest` 的 `change_id` 归键构造面（载荷恒 **id**，寻址纪律同型） | 改写 |
| `ToolStepOutput::MutationTest` | 正向 | 载荷写面原生类型逐字段相等（`MutationTestOutcome` 全字段：conclusion / skip / score / threshold / counts / report_paths / feedback_text） | 新增 |
| `MutationTestOutcome` 形态 | 正向 | `skip = None`（已执行）时 score / threshold 为归并块口径值；`feedback_text` 非空（Fail / Error）携报告路径指引 | 新增 |
| `MutationTestOutcome` 边界形态 | 边界 | `skip = Some(…)` 时 score / threshold 置 `0.0` 占位；`conclusion = Pass` 时 `feedback_text` 恒空串；`report_paths` POSIX（反斜杠不出线） | 新增 |
| `MutationTestConclusion` / `MutationSkipReason` | 正向 | 三值 `Pass` / `Fail` / `Error` 与两值 `EmptyDiff` / `NoExecutor` 的 `as_str` 线格式小写词（`TestExecutionConclusion` 同型）逐值断言 | 新增 |
| `MutationCounts` | 边界 | 七字段（`killed / survived / timeout / compile_error / runtime_error / ignored / total: u64`）逐字段写面；`u64` 计数大值（`u64::MAX`）构造不 panic | 新增 |
| 零 Serialize 边界 | 边界 | `MutationTestOutcome` 族不派生 Serialize（编译期锚，零 IPC / 零 `RunUpdate` 出线）——以文档注记 + 类型面断言承载，反序列化面不留通道 | 新增 |
| `MutationRunner.run` 假实现注入 | 正向 | 假实现记录四参 `(root, name, files, patch)` 逐字透传（`files` = 清单入参、`patch` = 补丁入参，E3）；`Arc<dyn MutationRunner>` 注入 + `Send + Sync` 断言 | 新增 |
| `MutationRunner.run` | 异常 | 假实现产出 `Err` → `BoxToolFuture` 透传不吞；产出 `Ok(ToolStepOutput::MutationTest)` 分发正确 | 新增 |
| `MutationRunner.run` | 边界 | `files` 空 / 单元素 / 超大列表（1000 项）、`patch` 空串 / 超长（>100KB）/ 含特殊字符（`\n` / emoji 文件名）均逐字透传不截断 | 新增 |
| `ArchiveVcsPort` 两方法（trait 面） | 正向 | 假实现注入（trait object）记录 `(root, base)` 两参透传；`base = "HEAD"` 形态 | 新增 |
| `ArchiveVcsPort` 两方法 | 异常 | 假实现 `Err`（String）透传不吞、不降级空清单 / 空补丁 | 新增 |
| `ArchiveVcsPort` 两方法 | 边界 | 空清单 `Ok(vec![])` / 空补丁 `Ok(String::new())` 透传（Ok 面与 Err 面区分） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `MutationRunner` / `ArchiveVcsPort`（被测 API 显式注入依赖） | 假实现（`ProgrammableRunner` / `RecordingSteps` 同式）：记录入参 + 可编程产出 | 签名锚定与四参 / 两参透传用例 |
| —（载荷类型） | 纯类型直构断言（零 Serialize、零 IO） | 载荷形态 / 线格式词用例 |

---

### packages/desktop/src-tauri/crates/infra/vcs/src/git.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs

#### 待测功能
- `change_edit_set(root: &Path, base: &str) -> Result<Vec<String>, String>`（公共函数 / API）：变更文件清单唯一入口——`git diff <base> --name-only -z` ∪ `git ls-files --others --exclude-standard -z`（untracked 并入），排序去重 POSIX 归一；git 失败显式 `Err`（MUST NOT 静默降级空清单）。
- `change_edit_patch(root: &Path, base: &str) -> Result<String, String>`（公共函数 / API）：`--in-diff` 补丁唯一入口——untracked 面先行 `git add -N`（intent-to-add，E2）再 `git diff <base>` 输出 unified diff 文本。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `change_edit_set` | 正向 | tracked 修改 + untracked 新建并入 → 清单含两类文件、字典序排序、去重、POSIX 归一（反斜杠 → `/`）（AC-3 / D3） | 新增 |
| `change_edit_set` | 正向 | 仅 tracked 改动（无 untracked）→ 清单恰为 diff 面；仅 untracked → 清单为 ls-files 面 | 新增 |
| `change_edit_set` | 正向 | `base = "HEAD"` 语义：清单恰为本 change 编辑集（worktree 语义）；其它合法 base（分支名 / sha）透传生效 | 新增 |
| `change_edit_set` | 异常 | 非 git 目录 / git 命令失败 → 显式 `Err`（MUST NOT 静默降级空清单——空清单会误走 diff 空跳过） | 新增 |
| `change_edit_set` | 异常 | `base` 非法：`""` 空串 / 非存在 rev / 含 `\n` / 超长（>1000 字符）→ 显式 `Err` 不 panic | 新增 |
| `change_edit_set` | 边界 | 干净仓零改动 → `Ok(空 Vec)`（与 `Err` 严格区分）；单文件清单 / 超大清单（100+ 文件）顺序稳定 | 新增 |
| `change_edit_set` | 边界 | 双源重复出现（intent-to-add 后 diff 面与 ls-files 面并集）→ 去重单条；特殊文件名（空格 / 中文 / emoji）与深层路径归一 | 新增 |
| `change_edit_set` | 边界 | `root` 不存在 / `root` 指向文件 → 显式 `Err` | 新增 |
| `change_edit_patch` | 正向 | untracked 经 `git add -N` 后补丁含新文件 unified diff hunk（行级精度，E2 / 风险 3 定稿） | 新增 |
| `change_edit_patch` | 正向 | tracked 修改 → 补丁含对应 hunks；多文件补丁含多 `diff --git` 文件段头（供段切片） | 新增 |
| `change_edit_patch` | 异常 | 非 git 目录 / 非法 `base` → 显式 `Err`（与 `change_edit_set` 同口径） | 新增 |
| `change_edit_patch` | 边界 | 干净仓 → 空串（零补丁）；补丁文本含特殊字符（`\n` / CRLF / emoji 内容）往返不破坏 diff 形态 | 新增 |
| `change_edit_patch` | 边界 | `git add -N` 幂等：重复调用结果一致（intent-to-add 重复执行不改语义，E2 单次执行口径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（不 mock） | 真实 git tempdir 仓组合（沿既有 `init_repo` / `init_empty_repo` / `plain_dir` / `fs_write` 装置；git 子进程真实执行——既有 `git_test.rs` 模式） | 全部用例；Err 面经非 git 目录 / 非法 base 真实触发 |

---

### packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/lib_test.rs

#### 待测功能
- `impl ArchiveVcsPort for ProcessArchiveVcs`（修改文件表）：增 `change_edit_set` / `change_edit_patch` 两方法，转发 `git.rs` 自由函数（port 实现随动）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ProcessArchiveVcs.change_edit_set` / `change_edit_patch` | 正向 | 经 trait object 动态分发结果与 `git.rs` 自由函数逐字等值（同仓同 base 双跑对拍） | 新增 |
| 两方法 | 异常 | `git.rs` 侧 `Err` 原文透传（不包装不吞） | 新增 |
| 两方法 | 边界 | 空清单 / 空补丁（干净仓）透传 `Ok` 面；`base` 边界形态（`"HEAD"` / 非法）与 `git.rs` 同口径 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（不 mock） | 真实 git tempdir 仓组合（沿 `git_test.rs` 装置），转发层薄实现真实执行 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/checks/src/model.rs -> packages/desktop/src-tauri/crates/core/checks/src/model_test.rs

#### 待测功能
- `parse_cargo_mutants_seg(raw: &str) -> Result<MutationSegReport, String>`（公共函数 / API）：cargo-mutants `--json` 段报告宽容解析 + 映射表（`caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`）；未知 outcome 计 `runtimeError` 并保留原文（宽容不丢数据）。
- `parse_stryker_seg(raw: &str) -> Result<MutationSegReport, String>`（公共函数 / API）：Stryker mutation 段报告宽容解析（outcome 词与 `MutationMeasured` 1:1）；同宽容纪律。
- `MutationTool` / `MutationSegReport`（类型定义表）：执行器标识两值；段级解析纯层模型（`tool` + `measured: MutationMeasured` + `duration_ms: f64`；`MutationMeasured` / `MutationBlock` 零扩展）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `parse_cargo_mutants_seg` | 正向 | 全映射表样本：`caught / missed / timeout / unviable / not-mutated` 五 outcome 逐个归位 `killed / survived / timeout / compileError / ignored`，计数正确、`tool = CargoMutants`、`duration_ms` 取段耗时（AC-4 / AC-9 映射表样本） | 新增 |
| `parse_cargo_mutants_seg` | 正向 | 混合样本（已知 + 未知 outcome 同段）→ 已知归位、未知计 `runtimeError` 且原文保留（宽容不丢数据），`total` 与各计数一致 | 新增 |
| `parse_stryker_seg` | 正向 | Stryker outcome 词与 `MutationMeasured` 1:1 归位（含 `compileError` / `runtimeError` / `ignored` / `noCoverage`），`tool = Stryker` | 新增 |
| 两解析函数 | 异常 | `raw` 非 JSON / 空串 / 结构破坏（缺计数面字段）→ 显式 `Err`（结构面严格、词面宽容的口径分工） | 新增 |
| 两解析函数 | 边界 | `raw` 超长（>100KB / 超 1000 mutants）解析不炸；特殊字符（`\n` / `\0` 转义 / emoji 文件名 / CRLF）往返不丢；未知字段多余 → 忽略不 Err | 新增 |
| `MutationSegReport` / `MutationTool` | 边界 | `MutationMeasured` / `MutationBlock` 既有字段零扩展（穷尽解构无新字段——schema 双侧对齐不变，corpus `mutation-block` fixture 口径不变）；计数全 0 / `u64` 大值构造不 panic | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 纯函数 fixtures 直驱（E4：映射表落纯层，零 spawn 零 Tauri），corpus 样本同源复用 | 全部用例零 mock |

---

### packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs -> packages/desktop/src-tauri/crates/core/checks/src/aggregate_test.rs

#### 待测功能
- `build_mutation_block(measured: MutationMeasured, threshold: f64) -> MutationBlock`（公共函数 / API）：score 公式单点 `(killed+timeout)/(total-ignored-compileError-runtimeError)`（分母 0 → 100.0，两位舍入）+ 阈值判定 `pass = score >= threshold`。
- `merge_mutation_blocks(blocks: &[MutationBlock]) -> Option<MutationBlock>`（公共函数 / API）：段计数求和归并（段当子报告）→ `build_mutation_block`（阈值取首块）；`aggregate_mutation` 改调本函数（MUST NOT 新增归并公式）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `build_mutation_block` | 正向 | 典型计数 → score 逐字等于公式值（两位舍入，如 1/3 → 33.33）；`pass = score >= threshold` | 新增 |
| `build_mutation_block` | 正向 | `killed + timeout` 混合计入分子、`no_coverage` 不入分母排除面（分母仅排 `ignored / compileError / runtimeError`） | 新增 |
| `build_mutation_block` | 异常 | 分母 0（全 `ignored` / `compileError` / `runtimeError` / 全 0）→ score = 100.0（既有语义）；`threshold` = `NaN` → `pass = false`（`>=` 恒假）；`threshold` = `Inf` → fail | 新增 |
| `build_mutation_block` | 边界 | `score == threshold` 等值 → `pass = true`（`>=` 边界）；`threshold` = `0.0` / `-0.0` 恒 pass；`threshold` > 100 → 必 fail；计数极大（f64 转换精度稳定不 panic） | 新增 |
| `merge_mutation_blocks` | 正向 | 多段计数求和归并（各字段和 = 段和）、阈值取首块（`aggregate_mutation` 既有语义） | 新增 |
| `merge_mutation_blocks` | 正向 | 单段（`[单元素]`）→ 归并块计数 / 阈值与原段等值 | 新增 |
| `merge_mutation_blocks` | 异常 | 空 slice `&[]` → `None`（不产块） | 新增 |
| `merge_mutation_blocks` | 边界 | 超大列表（1000 段）归并不炸、顺序不影响计数和；段计数含 0 / `u64` 大值混合 | 新增 |
| `aggregate_mutation` 同式对拍 | 组合 | 组合（链路方向：`aggregate_mutation` → `merge_mutation_blocks` / `build_mutation_block`）：既有 corpus 样本下新旧公式逐字等值（公式不变回归锚），无块样本仍 `None`（corpus 保真语义） | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 纯函数 fixtures 直驱（`MutationMeasured` / `MutationBlock` 内存构造） | 全部用例零 mock |

---

### packages/desktop/src-tauri/crates/core/checks/tests/fixtures/corpus 与 golden（mutation 语料工件） -> packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs

#### 待测功能
- corpus 语料扩展（desktop-corpus-regression delta / proposal 测试文件表）：`mutation-block` 语料扩展 + 新增 cargo-mutants / Stryker 段级报告 fixtures（含映射表样本、unviable / not-mutated 分支、未知 outcome 分支），对拍「宽容解析 → 归并 → 结论」全链投影。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 段级报告 fixtures 对拍 | 正向 | cargo-mutants / Stryker 段级 fixture → `parse_*_seg` → 计数归并 → 归并块结论（score / threshold / pass）投影与 golden 等值（AC-9 映射表样本 + 段级报告 fixtures） | 新增 |
| `mutation-block` 语料 | 正向 | 既有 `mutation-block` fixture 保真解析不丢块、golden 投影口径不变（schema 零扩展锚） | 保留 |
| 未知 outcome 语料 | 边界 | 未知 outcome 样本投影含 `runtimeError` 计数与原文保留字段（宽容不丢数据留痕） | 新增 |
| golden 重写纪律 | 异常 | 对拍差异 → 测试失败并提示；仅 `DESKTOP_GOLDEN_REWRITE=1` 显式重写（禁止静默重写，AC-9） | 保留 |
| 语料清单 | 边界 | 语料 ≥ 规模下限、清单同步 `fixtures/README.md`；损坏族（invalid-json / missing-required）下解析不 panic 口径持衡 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（不 mock） | fixtures 真实读盘 → 纯函数真实组合（解析 / 归并 / 投影），golden 快照对拍 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs

#### 待测功能
- `ProcessMutation::new() -> Self`（公共函数 / API）：构造（组合根装配，无状态）；`Default` 同式。
- `ProcessMutation.run(root, name, files, patch) -> BoxToolFuture`（公共函数 / API）：`MutationRunner` 实现——执行主链（E5 显式 `Err` 面：程序不可达 / 配置不支持 / 写盘失败）。
- `execute` 主链（变更清单 - 新增文件表）：suite 归桶（E10）→ 跳过面（`EmptyDiff` / `NoExecutor` 记 evidence 不 Err）→ 分段顺序执行 → 段报告解析（`parse_*_seg`）→ 计数归并（`merge_mutation_blocks` / `build_mutation_block`）→ 逐 suite 阈值判定（全过即 pass）→ summary mutation 位回填（E11 在位时）→ `MutationTestOutcome` / `feedback_text` 装配；突变常量参数（E9）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ProcessMutation::new` | 正向 | `new()` 与 `Default::default()` 等价可用 | 新增 |
| `ProcessMutation::new` | 异常 | 构造零副作用、零 IO、不 panic（无失败路径声明） | 新增 |
| `ProcessMutation::new` | 边界 | 连续多次构造互不影响（无状态） | 新增 |
| `ProcessMutation.run`（rust suite 全链） | 正向 | 组合（链路方向：`run` → 归桶 → `segments` 分段 → `cargo_mutants` spawn → `report` 写盘 → 纯层归并）：rust suite 全链产出 `ToolStepOutput::MutationTest`（conclusion / score / threshold / counts / report_paths），`mutation.json` 与 summary mutation 位在位（AC-4 / E11） | 新增 |
| `ProcessMutation.run`（js suite 全链） | 正向 | 组合（链路方向同上，`stryker` 面）：js 族 suite 走 Stryker（`mutate` = diff 文件清单）同链产出 | 新增 |
| 跳过面 `EmptyDiff` | 正向 | `files` 空清单 → `skip = EmptyDiff`、evidence「diff 空跳过」、`mutation: null` 保持合法、MUST NOT Err（AC-7） | 新增 |
| 跳过面 `NoExecutor` | 正向 | node-test 等无突变执行器框架的 suite → 该 suite 跳过记 evidence 不 Err；全部 suite 无执行器 → `skip = NoExecutor`（AC-4） | 新增 |
| E5 显式 Err 面 | 异常 | 工具不可达（spawn ENOENT）/ 配置不支持 / 写盘失败 → 显式 `Err` 停给用户（与 WorkerAgent 失败同等呈现），不静默跳过 | 新增 |
| 段中断 | 异常 | 段执行 error / harness 崩 / 超时 → `conclusion = Error`（中断错误摘要 + 已产报告路径清单），不等汇总（双时点②的中断面） | 新增 |
| suite 归桶（E10） | 边界 | 文件按 suite.root POSIX 前缀归桶；桶外文件不突变；跨 suite 文件只归一桶；全空 → `EmptyDiff` | 新增 |
| 逐 suite 阈值判定 | 边界 | 多 suite 多阈值（desktop 80 / src-tauri 70）：全过 → `Pass`；有败 → `Fail`（门禁结论以逐 suite 全过为准，风险 6 留痕）；归并块阈值取首块 | 新增 |
| `feedback_text` 装配 | 边界 | `Pass` 恒空串；`Fail` = score / threshold 结论摘要 + 报告路径清单；`Error` = 中断错误摘要 + 报告路径清单；MUST NOT 内联存活突变体清单（反馈边②报告指引式） | 新增 |
| summary 回填（E11） | 边界 | `summary.json` 在位 → mutation 位回填归并块；不在位 / 跳过面 → 不回填不崩（null 保持） | 新增 |
| 突变常量参数（E9） | 边界 | `MUTATION_JOBS_DIVISOR = 2` / `PER_MUTANT_TIMEOUT_SECS = 300` / `SEGMENT_BUDGET_MS = 300_000` / `SEGMENT_GUARD_MS = 600_000` / `EST_MUTANTS_PER_FILE = 10` 值锚定；`MutationConfig` 零新字段（不进 config schema，D9） | 新增 |
| 组合根链路 | 组合 | 组合（链路方向：`LocalToolSteps` MutationTest 分支 → `vcs.change_edit_set` / `change_edit_patch` → `ProcessMutation::run` 四参）：fake vcs 供清单 / 补丁，真实 `execute` 主链（假 spawn），outcome 载荷逐字段断言 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 子进程 spawn（cargo-mutants / npx stryker——进程边界） | 假 spawn（记录 argv / env / cwd + 可编程输出、超时、非零退出、ENOENT） | 执行链、Err 面、中断面用例 |
| 文件系统 / 报告目录 | 真实 tempdir（`change_test_reports` 布局真实落盘）——写盘 / 回填 / 清理断言真实组合 | 全链与回填用例 |
| `ArchiveVcsPort`（显式入参注入依赖） | 假实现供清单 / 补丁（本模块零 vcs 依赖，数据经入参注入） | 组合根链路用例 |
| `available_parallelism`（运行环境） | 不 mock：jobs 期望值由测试按真实 CPU 重算关系式（`max(1, n/2)`）断言 | 常量 / argv 相关用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/segments.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/segments_test.rs

#### 待测功能
- 分段切分（变更清单 - 新增文件表）：估算突变体量（`EST_MUTANTS_PER_FILE × 文件数` × 1~2 s/mutant）> `SEGMENT_BUDGET_MS` 触发，k = ceil(估算耗时 / 段预算)，文件字典序均分 k 段。
- 自适应修正（同表）：首段静态初值，后续段按上一段实测速率（秒 / mutant）动态调段大小；单文件超预算接受（软护栏 `SEGMENT_GUARD_MS` 仅观察留痕，MUST NOT 硬失败）。
- 补丁切片（同表）：`--in-diff` 补丁按 `diff --git` 文件段头切片为 `mutation-seg-<i>.patch`；Stryker 面为 mutate 清单切片。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 分段切分 | 正向 | 估算超预算 → 切 k 段、字典序均分、段并集 = 全清单、段间零交叠、顺序稳定 | 新增 |
| 补丁切片 | 正向 | 多文件 patch → 段补丁只含段内文件 hunks（按 `diff --git` 段头匹配）；Stryker 面 mutate 清单切片同口径 | 新增 |
| 自适应修正 | 正向 | 上一段实测速率慢于初值 → 后续段变小 / 快于初值 → 变大（修正方向正确） | 新增 |
| 分段切分 | 异常 | 空清单 → 零段不 panic（`EmptyDiff` 判定归调用方）；畸形补丁（无 `diff --git` 头 / 半截 hunk）→ 切片不 panic（行级精度交 cargo-mutants） | 新增 |
| 预算阈值 | 边界 | 估算恰等于 `SEGMENT_BUDGET_MS` 不切 / 超 1 即切（阈值边界）；单文件超预算 → 单段接受不失败（AC-5） | 新增 |
| 段规模边界 | 边界 | 文件数 1 / 2 / 1000（超大清单）→ 段数均衡、每段非空；段大小随自适应浮动但段预算 / 软护栏（10 分钟仅留痕）不产生硬失败 | 新增 |
| 特殊文件名 | 边界 | 文件名含空格 / 中文 / emoji / 深层路径 → 段头匹配与清单切片不破坏 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 纯函数直测（清单 / 补丁文本 / 速率观测值为显式入参） | 全部用例零 mock |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker_test.rs

#### 待测功能
- Stryker 执行面（变更清单 - 新增文件表）：临时 config 写盘（`mutate` = 段清单、`tempDirPath` = `<mutation 目录>/_stryker-tmp` 全段共享、`concurrency` = `max(1, available_parallelism()/2)`）、`npx --prefix <suite cwd> stryker run <config>` 模板展开 spawn（超时 + 输出捕获）、全段后一次清理（只清 `_stryker-tmp` 与临时 config）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 临时 config 写盘 | 正向 | config JSON 形态：`mutate` = 段清单逐项、`tempDirPath` = 共享 sandbox 路径、`concurrency` = `max(1, CPU/2)` 计算值 | 新增 |
| spawn 模板展开 | 正向 | argv 逐参等值（`npx --prefix <suite cwd> stryker run <config>`），超时 + 输出捕获完整 | 新增 |
| sandbox 跨段复用（D6） | 正向 | 多段共用同一 `tempDirPath`（段间不重建、不重置）；清理时点：段间零清理、全段完成后一次，只删 `_stryker-tmp` 与临时 config，`mutation-seg-*.json` 报告工件不触碰（AC-5） | 新增 |
| spawn 失败面 | 异常 | `npx` 不可达（ENOENT）→ 显式 `Err`（E5）；非零退出 → 输出捕获 + 中断（`conclusion = Error` 面）；超时 → 中断不挂死 | 新增 |
| config / 清单边界 | 边界 | `mutate` 空 / 单 / 超大（1000 glob）；`concurrency` CPU=1 → 1（`max(1, …)` 下限）；suite cwd 含空格 / 中文 | 新增 |
| 输出边界 | 边界 | 超大输出（>10MB）捕获不炸、截断口径留痕；输出含特殊字符（`\n` / `\0` 转义 / emoji）不破坏落盘 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 子进程 spawn（npx stryker——进程边界） | 假 spawn（记录 argv + 可编程输出 / 超时 / 非零退出 / ENOENT） | 模板展开、失败面、超时用例 |
| 文件系统（临时 config / `_stryker-tmp` / 报告目录） | 真实 tempdir（清理 / 保留断言真实组合） | sandbox 复用与清理时点用例 |
| `available_parallelism`（运行环境） | 不 mock：`concurrency` 期望值按真实 CPU 重算关系式断言 | concurrency 用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs

#### 待测功能
- cargo-mutants 执行面（变更清单 - 新增文件表）：`cargo mutants --in-diff <seg patch> --json --baseline skip --jobs <max(1, available_parallelism()/2)> --timeout <PER_MUTANT_TIMEOUT_SECS>` 模板展开 spawn（超时 + 输出捕获）；`CARGO_TARGET_DIR` 全段共享（增量编译一次全量，MUST NOT 每段重置）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| argv 模板展开 | 正向 | 逐参等值：`--in-diff <段补丁路径>` / `--json` / `--baseline skip` / `--jobs <值>` / `--timeout 300`（AC-4） | 新增 |
| jobs 计算 | 正向 | `max(1, CPU/2)` 关系式（CPU 1 → 1、2 → 1、3 → 1、4 → 2、16 → 8 抽样口径，期望值按真实 CPU 重算） | 新增 |
| `CARGO_TARGET_DIR` 共享（D6） | 正向 | 多段 env 断言同一 `CARGO_TARGET_DIR`（增量编译一次全量，MUST NOT 每段重置） | 新增 |
| spawn 失败面 | 异常 | `cargo mutants` 不可达（ENOENT）/ 非零退出 → 显式 `Err` / 输出捕获 + 中断（E5）；超时 → 中断不挂死 | 新增 |
| 补丁入参边界 | 边界 | 段补丁空串 / 超大（>100KB）/ 含特殊字符（`\n` / CRLF / emoji）→ argv / 文件传参不破坏；单核 CPU（jobs 下限 1） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 子进程 spawn（cargo mutants——进程边界） | 假 spawn（记录 argv / env + 可编程输出 / 超时 / ENOENT） | 模板展开、失败面、超时用例 |
| `available_parallelism`（运行环境） | 不 mock：jobs 期望值按真实 CPU 重算关系式断言 | jobs 用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/report.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/report_test.rs

#### 待测功能
- 段级与归并报告写盘（变更清单 - 新增文件表）：段原始报告 `mutation-seg-<i>.json` 写盘（工具原始输出，全程留存）、归并块 `mutation.json` 写盘。
- 段级复用门（同表 / E6）：段报告在位且 mtime ≥ 段输入（段内文件 + suite 测试文件）最新 mtime → 复用（零 spawn，重解析进归并）；否则失效重跑（断点续跑只重跑未完 / 失效段）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 段报告写盘 | 正向 | `mutation-seg-<i>.json` 内容 = 工具原始输出原样（不重排不丢字段）；路径命名 `<i>` 序号连续 | 新增 |
| 归并块写盘 | 正向 | `mutation.json` = `MutationBlock` 形态（schema 零扩展）；段报告全程留存不被覆盖 | 新增 |
| 复用门（AC-8） | 正向 | 段报告在位且 mtime ≥ 输入最新 mtime → 复用（零 spawn 断言），段报告重解析进归并 | 新增 |
| 复用门（AC-8） | 正向 | 中断重试：已完段零 spawn 复用、未完 / 失效段重跑（断点续跑） | 新增 |
| 复用门失效 | 异常 | 段报告缺失 / 损坏（非 JSON）→ 该段失效重跑（不 panic 不 Err） | 新增 |
| mtime 判据边界 | 边界 | 段报告 mtime 与输入 mtime **相等** → 复用（`>=` 判据）；段内文件 / suite 测试文件任一 mtime 更新 → 失效重跑；报告旧于输入 → 重跑 | 新增 |
| 清理边界 | 边界 | 清理只删 `_stryker-tmp` 与 `mutation-stryker-seg-*.conf.json`，段级 / 归并报告工件不触碰（AC-5） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 文件系统（mtime / 写盘 / 清理） | 真实 tempdir + 可设 mtime（`File::set_modified` 同类机制），零旁置状态文件（E6 判据借段报告自身 mtime） | 复用门与清理边界用例 |
| 子进程 spawn | 不引入（复用门语义 = 零 spawn，以假 spawn 计数器断言零调用） | 复用用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更：仅注释口径随动（`build_sub_report` 的 `mutation: None` 注释由「V1 不移植」改为「突变面由独立 MutationTest 步承载并回填 summary mutation 位」），零逻辑改动。既有 `mod_test.rs` 用例保持不变（`mutation: None` 硬编码断言口径不变，AC-7 null 观察期）。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src-tauri/crates/infra/checks/src/lib.rs -> packages/desktop/src-tauri/crates/infra/checks/src/lib_test.rs

<!-- design.md 未声明该文件的公共 API 变更：仅 `pub mod mutation;` 与 `pub use mutation::ProcessMutation;` 模块声明 / re-export（出线面），无独立运行时行为。行为覆盖经 `mutation/*_test.rs` 承接（AC-4 / AC-5）；可见性与出线正确性由编译期与组合根装配（`change_flow/mod.rs`）承载。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

#### 待测功能
- `LocalToolSteps::new(anchors, static_check, test_execution, mutation, vcs, store, run_id)`（公共函数 / API）：增 `mutation: Arc<dyn MutationRunner>` 缝入参（位于 `test_execution` 之后）。
- `execute` 的 `ToolCommand::MutationTest` 分支（修改文件表）：id → 记录 → name 解析（与 test-execution 同式）→ `vcs.change_edit_set(root, "HEAD")` + `vcs.change_edit_patch(root, "HEAD")` 取数 → `mutation.run(&root, &name, &files, &patch)` → `StepKind::MutationTest` 审计行（摘要携 conclusion / score / threshold）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `LocalToolSteps::new` | 正向 | 七缝装配成功，`mutation` 缝位于 `test_execution` 之后；各缝经 `Arc` 独立注入互不串扰 | 改写 |
| `LocalToolSteps::new` | 异常 | 构造零副作用、不 panic（无失败路径声明） | 新增 |
| `LocalToolSteps::new` | 边界 | 同一 `LocalToolSteps` 多次 `run` 各缝调用计数独立递增（无状态残留） | 改写 |
| `execute`（MutationTest 分支） | 正向 | 已建档（`id ≠ name`）→ 解析记录 `name`、`vcs.change_edit_set` / `change_edit_patch` 收 `("HEAD")`、`mutation.run` 收四参 `(root, name, files, patch)` 逐字透传；审计行 `StepKind::MutationTest`（摘要携 conclusion / score / threshold） | 新增 |
| `execute`（MutationTest 分支） | 异常 | change 未建档 → 显式 `Err`，零 vcs 调用、零 mutation 调用（不 spawn） | 新增 |
| `execute`（MutationTest 分支） | 异常 | `vcs.change_edit_set` / `change_edit_patch` `Err` → 传播不静默（MUST NOT 降级空清单），`mutation.run` 零发起；`mutation.run` `Err` → 传播 + 审计 error 行（失败不阻断审计） | 新增 |
| `execute`（MutationTest 分支） | 边界 | `id == name` 形态 name 逐字等于 id；files 空清单照常透传（`EmptyDiff` 判定归 mutation 侧）；重复调用独立解析、runner 调用计数递增 | 新增 |
| 突变执行链 | 组合 | 组合（链路方向：`LocalToolSteps` MutationTest 分支 → `ArchiveVcsPort.change_edit_set` / `change_edit_patch` → `MutationRunner::run` 四参）：真实 `ProcessArchiveVcs`（真实 git tempdir）+ fake `MutationRunner` → files / patch 恰为 worktree 编辑集（AC-3 全链唯一入口） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `MutationRunner` / `StaticCheckRunner` / `TestExecutionRunner` / `ArchiveVcsPort`（显式入参注入依赖） | 假实现（记录入参 + 可编程产出 / 可编程 `Err`） | 分支行为与透传用例 |
| `ChangeStateStore`（进程边界 DB） | `FakeStore` / 真实 tempfile store | name 解析 / 未建档 / 读故障用例 |
| git 子进程（组合链路） | 不 mock：真实 git tempdir（沿既有装置），fake 仅在 `MutationRunner` 缝 | 组合链路用例 |
| 内部模块（`audit_outcome` / `clip_summary`） | 不 mock：真实组合 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能
- `run_step` 增 `StepNode::MutationTest` 臂（修改文件表）：拆步时序——TestExecution fail 走既有反馈边修复重入，绿后才进 MutationTest（walker 顺序保证，AC-1）。
- `step_mutation_test`（同表）：双时点反馈边②——Pass → `cursor.mutation_outcome` 落槽 Proceed；Fail / Error → 反馈 prompt 只嵌 `outcome.feedback_text`（结论摘要 + 报告路径清单），`feedback_limit(step, "mutation-test")` 独立计数，超限走升格。
- `step_mutation_test_upgraded_phase_log`（同表）：照 `step_test_execution_upgraded_phase_log` 同式（fail checklist 条目「突变测试」+ 仅携 executor 槽位）。
- `PhaseCursor.mutation_outcome` 槽（同表）：与 `test_outcome` 双槽并立。
- `test_execution_phase_log_input`（同表）：签名增 `mutation: Option<&MutationTestOutcome>` 双槽入参；第 3 条判定语义恒 pass 不变，evidence 按 E8 三态组装。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 拆步时序（AC-1） | 正向 | 组合（链路方向：`run_step` 序 PhaseStart → TestExecution → MutationTest → PhaseLog）：TestExecution 绿后才进 MutationTest | 改写 |
| 拆步时序 | 异常 | TestExecution fail → 走既有反馈边修复重入，不进 MutationTest；修复重入绿后才续走 MutationTest | 改写 |
| `step_mutation_test`（Pass） | 正向 | 达标（逐 suite 全过）→ 节点 passed（detail 携 `conclusion=pass score=… threshold=…`）+ `cursor.mutation_outcome = Some(outcome)` + Proceed，**零反馈**（达标 pass 直接收口） | 新增 |
| `step_mutation_test`（Fail） | 正向 | 全段跑完但未达标 → 反馈 prompt 只嵌 `feedback_text`（score / threshold + 报告路径清单），注入 executor 会话（`EdgeKind::Feedback` 续注同会话） | 新增 |
| `step_mutation_test`（Error） | 正向 | 段执行 error / harness 崩 / 超时 → **立即反馈**（错误摘要 + 报告路径清单），不等汇总（双时点②） | 新增 |
| 反馈内容口径（AC-6 / D7） | 边界 | 反馈 prompt 零存活突变体内联（断言 prompt 不含 mutant 明细词面）；`feedback_text` 超长 / report_paths 超多路径清单原样内嵌不加工 | 新增 |
| 独立计数预算（AC-6） | 正向 | `feedback_limit_of("test-execution", StepNode::MutationTest)` == 5，且与 static-check / test-execution 反馈计数器分立（三者互不影响） | 新增 |
| 超限升格 | 异常 | 第 6 次反馈 → `step_mutation_test_upgraded_phase_log` 代写 fail checklist（条目「突变测试」+ 仅携 executor 槽位），`cursor.failed = true` + `StepFlow::Upgraded` 升格相位 fail | 新增 |
| `test_execution_phase_log_input` 双槽（AC-7 / E8） | 正向 | `mutation = Some(skip=None)` → evidence「跑过且达标（score X ≥ threshold Y）」；第 3 条「mutation null 自动通过」判定恒 pass（语义不改，D8） | 新增 |
| evidence 三态 | 边界 | `skip = EmptyDiff` → 「diff 空跳过」；`skip = NoExecutor` → 「null（未跑）」附注「无突变执行器」；`mutation = None` → 「null（未跑）」（E8 三态映射） | 新增 |
| `PhaseCursor` 双槽 | 边界 | `test_outcome` 与 `mutation_outcome` 互不覆盖、`take()` 各自独立消费；MutationTest 失败路径下 test 槽已消费状态不回滚 | 新增 |
| 全链路（AC-6 收口） | 组合 | 组合（链路方向：`step_mutation_test` → `LocalToolSteps.execute(MutationTest)` → vcs 取数 → `MutationRunner::run`）：真实 `LocalToolSteps` + 假 `MutationRunner` / 真实 vcs，双时点反馈与计数在全链下成立 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `MutationRunner` / `TestExecutionRunner` / `StaticCheckRunner` / `ToolStepPort`（显式注入依赖） | 假实现（`ProgrammableRunner` / `RecordingSteps` 同式，可编程 Pass / Fail / Error 产出） | 拆步时序 / 反馈双时点用例 |
| `RunEventSink` / `ChangeStateStore`（注入依赖 / 进程边界 DB） | 既有 `RecordingSink` / `FakeStore` 装置 | 边史 / 计数观察用例 |
| 内部模块（`feedback_limit` / prompt 装配） | 不 mock：真实组合 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/tests/golden（十九值 nodes 投影语料） -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

#### 待测功能
- 十九值 nodes 投影 golden（proposal 测试文件表 / AC-9）：节点全词汇样本十八类 → 十九类（含 `mutation_test`），投影对拍走 `DESKTOP_GOLDEN_REWRITE=1` 显式重写。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| nodes 投影 golden | 正向 | 十九类节点样本全词汇投影与 golden 等值（含 `mutation_test` 类样本）（AC-2 / AC-9） | 改写 |
| golden 重写纪律 | 异常 | 对比差异 → 失败并提示；仅 `DESKTOP_GOLDEN_REWRITE=1` 显式重写（禁止静默重写，风险 5） | 保留 |
| 词汇边界 | 边界 | 18 既有类投影零回归；样本 ≥ 规模下限 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（不 mock） | 语料真实读盘 → 投影真实组合 → golden 快照对拍 | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更：本变更仅组合根装配（`LocalToolSteps::new(...)` 增传 `Arc::new(ProcessMutation::new())`，import 随动），接线正确性由 `Arc<dyn MutationRunner>` 缝类型编译期强制；MutationTest 步行为经 `walker_test.rs`（拆步时序 / 双时点反馈）与 `steps_test.rs`（执行链）组合用例覆盖（AC-3 / AC-6）。既有 `mod_test.rs` 装置用例保持不变。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能
- `normalizeKind`（修改文件表）：增 `case 'mutation_test': return 'mutationTest';`（十八值 → 十九值归一）。
- `stepWord`（修改文件表）：增 `case 'mutationTest': return 'mutation_test';`（逆归一）。
- `groupOf`（架构组件表）：`mutationTest` 经 default 落 `toolStep`（exec 组），`GROUP_LABEL` 零改动。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `normalizeKind` | 正向 | 19 个 `RunStepKind` 词逐词映射断言（新增 `mutation_test → mutationTest`）（AC-2） | 改写 |
| `stepWord` | 正向 | 19 个 `ChangeStepKind` 词逐词逆映射断言（新增 `mutationTest → mutation_test`）；与 `normalizeKind` 双向 round-trip 全词汇成立 | 改写 |
| `groupOf` | 正向 | `groupOf('mutationTest')` 落 `toolStep`（exec 组）；`GROUP_LABEL` 键集与文案持衡（零改动锚） | 新增 |
| 归一表 | 异常 | 枚举外字符串（未知词 cast）→ default 落 `toolStep` 不崩不抛 | 新增 |
| 18 既有词汇 | 边界 | 除 `mutation_test` / `mutationTest` 外 18 词归一与分组持衡（零回归）；`unifiedRunNodes` / `runStepNodes` 在 19 词全词汇下无过滤丢步 | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 纯函数直测（fixture 自备 19 词 `ChangeRunNodeRecord` / `UnifiedRunStep`），零 react / 零 IPC | 全部用例零 mock |

---

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能
- `STEP_LABEL: Record<ChangeStepKind, string>`（修改文件表）：增 `mutationTest: '突变执行'` 键（穷尽表随新词编译期强制；`GROUP_LABEL` / 渲染逻辑零改动——`mutationTest` 经 `groupOf` default 落 `toolStep` exec 组）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `STEP_LABEL` 键面 | 正向 | 含 `mutationTest: '突变执行'` 键值断言（AC-2 文案键） | 新增 |
| `STEP_LABEL` 穷尽对拍 | 边界 | 19 键与 `ChangeStepKind` 全词汇对拍（键集恰等，运行时断言兜住编译期穷尽表） | 改写 |
| mutationTest 节点渲染 | 正向 | `mutationTest` 步节点渲染显示「突变执行」、归 exec 组列（零渲染改动语义——现有步骤渲染直接吃） | 新增 |
| 渲染回归 | 异常 | 非法 / 缺失 detail 载荷下 mutationTest 节点渲染不炸图（既有渲染边界持衡） | 保留 |
| `GROUP_LABEL` 零改动 | 边界 | `GROUP_LABEL` 键集与 18 既有步渲染结果持衡（零回归） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | React 组件真实渲染（`@testing-library/react` + jsdom），节点 fixture 自备 | 渲染用例；`STEP_LABEL` 键面纯断言零 mock |

---

## 不可测试项

| 文件或模块 | 原因 |
|------------|------|
| `packages/desktop/src-tauri/src/types/generated/bindings.ts` | tauri-specta 再生的 codegen 产物（`ChangeStepKind` 增 `"mutationTest"`、`RunStepKind` / `StepKind` 增 `"mutation_test"`），非手改不可单测；正确性由 `bindings:check` 一致性守卫（AC-9）拦截漂移，词汇行为面已由 `state.rs` / `run-state.ts` 测试承接 |
| `packages/desktop/package.json` | 交付元数据（`version` 0.5.1 → 0.5.2，归档时执行；`tauri.conf.json` 引用自动跟随），无运行时逻辑可单测，由归档时静态校验承接（AC-11） |
