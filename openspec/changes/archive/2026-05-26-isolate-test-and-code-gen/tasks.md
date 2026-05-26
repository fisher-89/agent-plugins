# 任务列表: isolate-test-and-code-gen

---

## 阶段一: PHASES 常量与工作流引擎更新

- [x] **T1.1** 更新 `plugins/dev-team/bin/src/lib/workflow.ts` 中的 `PHASES` 常量：从 7 阶段扩展到 9 阶段（新增 `06-unit-test` 和 `08-integration-test`），将 `"05-implementation"` 重命名为 `"05-implement"`
- [x] **T1.2** 更新 `plugins/dev-team/bin/src/lib/workflow.ts` 中的 `getPhaseIndex` 和 `getPriorPhases` 函数（逻辑不变，需确认新列表的顺序正确）
- [x] **T1.3** 更新 `plugins/dev-team/bin/src/lib/eval-json.ts` 中的 `BuildEntryParams` 接口：新增可选的 `skipped`、`findings`、`phase_suffix` 字段
- [x] **T1.4** 更新 `plugins/dev-team/bin/src/lib/eval-json.ts` 中的 `buildEntry` 函数：支持 `skipped`、`findings`、`phase_suffix` 扩展字段的输出
- [x] **T1.5** 更新 `plugins/dev-team/bin/src/lib/eval-json.ts` 中的 `validateVerdict` 函数：允许 `skipped: true` 时 verdict 为 `"pass"`
- [x] **T1.6** 为 workflow.ts 和 eval-json.ts 的变更编写 vitest 单元测试

## 阶段二: Agent prompts — 新增测试执行 agent

- [x] **T2.1** 创建 `plugins/dev-team/agents/unit-test-executor.md` — sonnet 模型 agent prompt，职责：执行单元测试命令、捕获输出、生成结构化报告
- [x] **T2.2** 创建 `plugins/dev-team/agents/unit-test-evaluator.md` — 读取测试执行报告、校验报告完整性、应用诊断决策树、设置 verdict 和 backtrack_to
- [x] **T2.3** 创建 `plugins/dev-team/agents/integration-test-executor.md` — sonnet 模型 agent prompt，职责：执行集成测试命令、捕获输出、生成结构化报告
- [x] **T2.4** 创建 `plugins/dev-team/agents/integration-test-evaluator.md` — 读取集成测试执行报告、校验报告完整性、应用诊断决策树、设置 verdict 和 backtrack_to

## 阶段三: Agent prompts — 文件隔离约束

- [x] **T3.1** 修改 `plugins/dev-team/agents/test-gen-generator.md`：在 Constraints 部分添加源码文件类型黑名单（.ts/.py/.js/.rs/.go/.java/.c/.cpp/.h/.hpp 不可读取）
- [x] **T3.2** 修改 `plugins/dev-team/agents/implementation-generator.md`：在 Constraints 部分添加 tests/ 目录黑名单（tests/、__tests__/、test/ 下文件不可读取）
- [x] **T3.3** 修改 `plugins/dev-team/agents/test-gen-generator.md`：在 Process 部分添加引用模块边界契约（spec.md）的步骤
- [x] **T3.4** 修改 `plugins/dev-team/agents/implementation-generator.md`：在 Input 部分添加引用模块边界契约（spec.md）的步骤

## 阶段四: Agent prompts — 模块边界契约

- [x] **T4.1** 修改 `plugins/dev-team/agents/requirements-planner.md`：在 Process 步骤中添加"识别受影响的模块目录"并在 specs/ 中生成接口契约表格的逻辑
- [x] **T4.2** 在 requirements-planner.md 中添加四种接口契约的结构化表格格式定义：函数签名、API 接口、CLI 命令、前端组件 props/events

## 阶段五: Skill 文件 — 创建和修改

- [x] **T5.1** 创建 `plugins/dev-team/skills/phase-unit-test/SKILL.md` — EXEC (Executor->Evaluator) 模式，包含 no-op 判定逻辑（检查 `**/*.test.*`、`**/tests/unit/**`、`**/__tests__/**` 文件存在性）
- [x] **T5.2** 创建 `plugins/dev-team/skills/phase-integration-test/SKILL.md` — EXEC (Executor->Evaluator) 模式，包含 no-op 判定逻辑（检查 `**/*.integration.test.*`、`**/tests/integration/**` 文件存在性）
- [x] **T5.3** 修改 `plugins/dev-team/skills/phase-implement/SKILL.md`：移除 AUTO 步骤中的测试执行命令 (`python plugins/dev-team/utils/test-runner.py`) 调用，仅保留 static-check
- [x] **T5.4** 修改 `plugins/dev-team/skills/phase-implement/SKILL.md`：在 AUTO static-check 失败步骤中添加"不触发 unit-test 阶段"的说明
- [x] **T5.5** 修改 `plugins/dev-team/skills/phase-code-review/SKILL.md`：移除 integration-test 子步骤，恢复为单 Evaluator 模式
- [x] **T5.6** 修改 `plugins/dev-team/skills/phase-code-review/SKILL.md`：更新描述和注释，移除对 integration-test 的引用
- [x] **T5.7** 在每个 Generator 执行后的 skill 路由（test-gen 和 implement SKILL.md）中添加 Read 调用路径校验步骤

## 阶段六: `@likec4/core` 集成与 TypeScript CLI archi 命令

- [x] **T6.1** 在 `plugins/dev-team/bin/` 目录下执行 `npm install @likec4/core@^1.56.0`
- [x] **T6.2** 在 `plugins/dev-team/bin/src/lib/` 下创建 `c4-types.ts` — 定义 archi 命令相关的 TypeScript 类型（ArchiQueryResult, ArchiValidateResult, CrossRefViolation 等）
- [x] **T6.3** 创建 `plugins/dev-team/bin/src/lib/archi-query.ts` — 封装 `LikeC4.fromSource()` + `LikeC4Model.Computed` 查询逻辑，支持 `query --element` 参数
- [x] **T6.4** 创建 `plugins/dev-team/bin/src/lib/archi-validate.ts` — 封装 `LikeC4.fromSource()` + `getErrors()` 校验逻辑，支持 `validate --source` 参数
- [x] **T6.5** 创建 `plugins/dev-team/bin/src/lib/archi-write.ts` — 封装校验 + 文件写入逻辑，支持 `write --path --source` 参数
- [x] **T6.6** 创建 `plugins/dev-team/bin/src/lib/c4-cross-ref.ts` — 重写 `archi-validate.py` 的逻辑：解析项目文件的 import 语句（TS/JS/Python），映射到 C4 元素，检测 unmodeled_dependency / unmapped_import_target / unused_relationship / path_not_found，生成 violations/warnings 报告
- [x] **T6.7** 创建 `plugins/dev-team/bin/src/commands/archi.ts` — 注册 `dev-team archi` 命令及四个子命令（query / validate / write / check）
- [x] **T6.8** 修改 `plugins/dev-team/bin/src/index.ts` — 在 `main()` 中注册 archi 命令
- [x] **T6.9** 删除 Python archi 文件：`plugins/dev-team/utils/archi_parser.py`、`plugins/dev-team/utils/archi-model.py`、`plugins/dev-team/utils/archi-validate.py`
- [x] **T6.10** 修改 `plugins/dev-team/agents/architecture.md`：将所有 `python plugins/dev-team/utils/archi-*.py` 命令引用替换为 `dev-team archi` 等效命令
- [x] **T6.11** 为 archi 命令和 c4-cross-ref 模块编写 vitest 单元测试

## 阶段七: eval-check CLI 更新

- [x] **T7.1** 修改 `plugins/dev-team/bin/src/commands/eval-check.ts` 中的 `checkPriorPhases` 函数：识别 `skipped: true` 的条目视同 pass
- [x] **T7.2** 修改 `plugins/dev-team/bin/src/commands/eval-check.ts` 中的 `determinePhaseState` 函数：对含有 `skipped: true` 的 phase 返回 `"passed"` 状态
- [x] **T7.3** 修改 `plugins/dev-team/bin/src/commands/eval-check.ts` 中的 `checkTimestampOrder` 函数：跳过 `skipped` 的条目
- [x] **T7.4** 修改 `plugins/dev-team/bin/src/commands/eval-check.ts` 中的 human-readable 输出：当 phase state 为 `"passed"` 且有 `skipped: true` 时，显示 "(skipped: no applicable tests)"
- [x] **T7.5** 为 eval-check CLI 的 no-op 检测和 9 阶段支持编写 vitest 单元测试

## 阶段八: eval-log CLI 更新

- [x] **T8.1** 修改 `plugins/dev-team/bin/src/commands/eval-log.ts`：新增 `--skipped` 命令行选项
- [x] **T8.2** 修改 `plugins/dev-team/bin/src/commands/eval-log.ts`：新增 `--findings` 命令行选项
- [x] **T8.3** 修改 `plugins/dev-team/bin/src/commands/eval-log.ts`：将 `--skipped` 和 `--findings` 传递到 `buildEntry` 的扩展字段中

## 阶段九: E2E 残留清理与 test-design 模板更新

- [x] **T9.1** 搜索 `plugins/dev-team/templates/` 和 `plugins/dev-team/agents/` 中是否包含 "e2e" 或 "E2E" 关键字，如有则移除或替换
- [x] **T9.2** 修改 `plugins/dev-team/templates/artifacts/test-design.md.template`：新增测试分类标记（unit 和 integration 的分类说明）

## 阶段十: 测试文件编写

- [x] **T10.1** 创建 `tests/test_file_type_isolation.py` — pytest 验证文件类型隔离判定逻辑（test-gen 黑名单内容、implement 黑名单内容、skill 层校验逻辑）
- [x] **T10.2** 创建 `tests/test_test_executor_report.py` — pytest 验证测试执行报告 JSON schema 完整性、unit-test/integration-test Evaluator 的 checklist 和诊断决策树
- [x] **T10.3** 创建 `tests/test_diagnostic_decision_tree.py` — pytest 验证 4 种已知失败场景的诊断决策树回溯目标判定（语法错误、逻辑错误、设计冲突、接口不匹配），以及多种错误同时存在的优先级
- [x] **T10.4** 创建 `tests/test_diagnostic_ask_user.py` — pytest 验证"无法判断"场景下 AskUserQuestion 调用、提问内容格式、超时后回退到 dev-proposal 及 findings 记录
- [x] **T10.5** 创建 `tests/test_noop_phase.py` — pytest 验证 no-op 判定逻辑：无 `*.test.*` 文件、无 `tests/integration/` 文件、有 `__tests__/` 但空文件
- [x] **T10.6** 创建 `tests/test_implement_phase_auto.py` — pytest 验证 implement 阶段 AUTO 步骤列表（仅 static-check，无 unit-test）
- [x] **T10.7** 创建 `tests/test_requirements_module_boundary.py` — pytest 验证模块边界契约的 spec.md 输出格式（函数、API、CLI、组件表格；无 public API 的空表处理；20+ export 函数不截断）
- [x] **T10.8** 创建 `tests/test_code_review_phase_unittest.py` — pytest 验证 code-review 阶段 skill 路由为单 Evaluator 且 eval.json 仅一条条目
- [x] **T10.9** 创建 `tests/test_eval_check_noop.ts` — vitest 验证 eval-check CLI 对 no-op 跳过的 phase 状态识别（human-readable 输出含 "skipped"，JSON 中 phase_state 为 "passed"）
- [x] **T10.10** 创建 `tests/test_archi_query.py` — 集成测试验证 `dev-team archi query` 输出符合预期 JSON schema（elements/relations 结构）
- [x] **T10.11** 创建 `tests/test_archi_validate_likec4.py` — 集成测试验证 `dev-team archi validate` 对 LikeC4 语法（views/deployment/tags）的兼容性
- [x] **T10.12** 创建 `tests/test_archi_check.py` — 集成测试验证 `dev-team archi check --staged` 对有已知问题的模型正确报告 violations
- [x] **T10.13** 创建 `tests/test_implement_static_check_fail.py` — 集成测试验证 static-check 失败后单元测试阶段不触发 + 直接回溯 implement
- [x] **T10.14** 创建 `tests/test_no_e2e_validation.py` — 验证脚本：检测 templates/ 和 agents/ 中 E2E 关键字残留
- [x] **T10.15** 创建 `tests/test_architecture_md_commands.py` — 验证脚本：检测 architecture.md 中无 `python plugins/dev-team/utils/archi-*.py` 残留
- [x] **T10.16** 创建 `tests/test_archi_files_deleted.py` — 验证脚本：确认 `plugins/dev-team/utils/` 下无 archi 三文件

## 阶段十一: 测试 fixtures 准备

- [x] **T11.1** 在 `tests/fixtures/` 下创建 mock 测试执行报告 JSON 文件：`unit-test-pass.json`、`unit-test-fail-syntax.json`、`unit-test-fail-logic.json`、`unit-test-fail-design-conflict.json`、`unit-test-fail-unknown.json`、`unit-test-fail-mixed.json`、`integration-test-pass.json`、`integration-test-fail-report-incomplete.json`
- [x] **T11.2** 在 `tests/fixtures/` 下创建 mock C4 模型文件：`simple-model.c4`、`likec4-official-views.c4`、`likec4-official-deployment.c4`、`likec4-official-tags.c4`、`multi-file-import/` 目录含多文件
- [x] **T11.3** 在 `tests/fixtures/` 下创建 mock eval.json 片段：`eval-with-skipped.json`、`eval-7-phase-old.json`、`eval-with-backtrack.json`
- [x] **T11.4** 在 `tests/fixtures/` 下创建 archi 命令测试数据和 Agent prompt 匹配模板文件
