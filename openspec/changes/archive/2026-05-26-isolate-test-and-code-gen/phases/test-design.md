# 测试设计: isolate-test-and-code-gen

> **变更**: isolate-test-and-code-gen
> **日期**: 2026-05-25
> **基于**: proposal.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | Python 工具函数（文件类型隔离校验、no-op 判定逻辑、诊断决策树、测试报告 schema 校验、模块边界契约生成） | pytest (Python) | 覆盖所有新增/修改的工具函数逻辑，实现 90%+ 语句覆盖 |
| 单元测试 | TypeScript CLI（PHASES 扩展、getPriorPhases/getPhaseIndex、eval-check no-op 检测、archi 子命令路由、c4-cross-ref import 解析） | vitest (TypeScript, plugins/dev-team/bin/) | 覆盖所有新增 CLI 命令和处理函数，实现 85%+ 语句覆盖 |
| 集成测试 | PGE 阶段流水线：implement→unit-test→code-review→integration-test 阶段推进和回溯的正确性 | pytest + Bash 脚本 | 验证 9 阶段流水线的 phase 相序、回溯链完整性和 no-op 跳过 |
| 集成测试 | `dev-team archi` CLI 命令正确性：query/validate/check 输出符合预期 schema | Bash/Python 脚本 | 验证 `@likec4/core` 集成后命令正常执行、输出结构正确 |
| 验证脚本 | Agent prompt 内容校验：文件类型黑名单、目录黑名单 | Python 脚本 (re 匹配) | 验证 test-gen-generator.md 和 implementation-generator.md 的 prompt 中包含正确的隔离约束 |
| 验证脚本 | 架构文件删除确认、E2E 残留确认 | Python 脚本 (os.path.isfile + grep) | 验证 AC-11、AC-18、AC-19 |
| 端到端测试 | 不引入（符合变更范围中"移除 E2E 测试支持"的约束） | — | — |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `tests/test_requirements_module_boundary.py` | 单元测试 | 模块边界契约的 spec.md 输出格式验证 |
| AC-2 | `tests/test_file_type_isolation.py` | 单元测试 | test-gen-generator prompt 黑名单内容 + skill 校验逻辑 |
| AC-3 | `tests/test_file_type_isolation.py` | 单元测试 | implementation-generator prompt 黑名单内容 + skill 校验逻辑 |
| AC-4 | `tests/test_implement_phase_auto.py` | 单元测试 | implement 阶段 skill 路由中 AUTO 步骤列表验证 |
| AC-5 | `tests/test_test_executor_report.py` | 单元测试 | 单元测试执行报告 JSON schema 完整性和字段校验 |
| AC-6 | `tests/test_test_executor_report.py` | 单元测试 | unit-test Evaluator 报告完整性 checklist + 诊断决策树 |
| AC-7 | `tests/test_test_executor_report.py` | 单元测试 | 集成测试执行报告 JSON schema 与单元测试报告一致性 |
| AC-8 | `tests/test_test_executor_report.py` | 单元测试 | integration-test Evaluator 读取报告 + 诊断决策树 |
| AC-9 | `tests/test_diagnostic_decision_tree.py` | 单元测试 | 4 种已知失败场景的诊断决策树回溯目标判定 |
| AC-10 | `tests/test_noop_phase.py` | 单元测试 | no-op phase 判定逻辑 + eval.json skipped 条目验证 |
| AC-11 | `tests/test_no_e2e_validation.py` | 验证脚本 | templates/ 和 agents/ 中 E2E 关键字残留检测 |
| AC-12 | `tests/test_implement_static_check_fail.py` | 集成测试 | static-check 失败后单元测试阶段不触发 + 直接回溯 implement |
| AC-13 | `tests/test_eval_check_noop.ts` | 单元测试 (vitest) | eval-check CLI 对 no-op 跳过的 phase 状态识别 |
| AC-14 | `tests/test_code_review_phase_unittest.py` | 单元测试 | code-review 阶段 skill 路由为单 Evaluator 且 eval.json 仅一条条目 |
| AC-15 | `tests/test_archi_query.py` | 集成测试 | `dev-team archi query` 输出符合预期 JSON schema，包含 elements/relations 结构 |
| AC-16 | `tests/test_archi_validate_likec4.py` | 集成测试 | `dev-team archi validate` 对 LikeC4 语法（views/deployment/tags）兼容 |
| AC-17 | `tests/test_archi_check.py` | 集成测试 | `dev-team archi check --staged` 对有已知问题的模型正确报告 violations |
| AC-18 | `tests/test_architecture_md_commands.py` | 验证脚本 | architecture.md 中无 `python plugins/dev-team/utils/archi-*.py` 残留 |
| AC-19 | `tests/test_archi_files_deleted.py` | 验证脚本 | `plugins/dev-team/utils/` 下无 archi 三文件 |
| AC-20 | `tests/test_diagnostic_ask_user.py` | 单元测试 | "无法判断"场景下 Evaluator 调用 AskUserQuestion 且内容含诊断信息 |
| AC-21 | `tests/test_diagnostic_ask_user.py` | 单元测试 | AskUserQuestion 超时后回溯到 dev-proposal + findings 含超时记录 |

---

## 3. 测试策略

### 3.1 方法

本变更涉及四个新增能力（module-boundary-contracts、test-execution-diagnostics、file-type-isolation、c4-cli-integration）和六个修改能力的测试。测试策略采用三层验证：

1. **工具函数层** — 对 Python/TypeScript 中新增或修改的逻辑函数（如诊断决策树、no-op 判定、报告 schema 校验、archi CLI 子命令）编写独立单元测试，使用 mock 隔离外部依赖。
2. **CLI 命令层** — 对 `dev-team archi` 系列命令和 `eval-check` 命令进行集成测试，在真实环境或近似真实环境中运行命令并验证输出。
3. **产物验证层** — 对 Agent prompt 内容、文件存在性、命令引用等静态产物编写脚本级验证，确保 markdown 文件中的约束生效。

不引入 E2E 测试（符合变更范围约束）。单元测试和集成测试均使用 mock 模拟外部系统。

### 3.2 测试分类

- **单元测试**: Python pytest 测试覆盖 `test_file_type_isolation.py`（隔离判定逻辑）、`test_test_executor_report.py`（报告 schema + 诊断决策树）、`test_diagnostic_decision_tree.py`（4 种根因回溯）、`test_noop_phase.py`（no-op 判定）、`test_implement_phase_auto.py`（AUTO 步骤列表）、`test_requirements_module_boundary.py`（契约格式）、`test_code_review_phase_unittest.py`（单 Evaluator）和 `test_diagnostic_ask_user.py`（AskUserQuestion + 超时）。TypeScript vitest 测试覆盖 `test_eval_check_noop.ts`（eval-check no-op 检测）。

- **集成测试**: `test_archi_query.py`（query 输出 schema 验证）、`test_archi_validate_likec4.py`（LikeC4 兼容性）、`test_archi_check.py`（check violations 正确性）、`test_implement_static_check_fail.py`（static-check 失败流水线行为）。集成测试以 Bash 脚本或 Python 脚本驱动，在项目根目录执行命令并验证输出。

- **验证脚本**: `test_no_e2e_validation.py`（E2E 残留检测）、`test_architecture_md_commands.py`（命令引用替换验证）、`test_archi_files_deleted.py`（文件删除确认）。验证脚本为一次性手动执行脚本，检查静态产物完整性。

### 3.3 模拟策略

| 被测对象 | 模拟策略 | 说明 |
|---------|----------|------|
| 诊断决策树 Evaluator | mock `eval.json` 的读写，mock AskUserQuestion 工具调用 | 决策树本身是纯逻辑，不依赖真实测试执行环境 |
| 测试执行报告生成 | mock 子进程调用（`subprocess.run`），返回预设的 stdout/stderr | 避免实际运行测试套件，专注于报告格式校验 |
| no-op 判定逻辑 | mock `os.path.isfile` 和 `glob.glob` 控制文件存在性判定 | 验证不同文件存在组合下的 no-op 判定结果 |
| 模块边界契约生成 | 模拟 requirements-planner 的输入（受影响模块目录列表），验证输出 spec.md 格式 | 不依赖实际的 requirements 阶段执行 |
| archi CLI 命令 | 针对 `@likec4/core` API 调用写单元测试；对 `LikeC4.fromSource()` 返回值做 mock | 外部 npm 包通过 vitest mock 模拟；集成测试在真实环境运行 |
| eval-check no-op 检测 | mock `readEvalJson` 返回值，构造包含 `skipped: true` 条目的 eval.json | 验证 CLI 输出中包含 "skipped" 状态且不阻塞 |
| AskUserQuestion 超时 | mock 计时器或超时异常，验证 fallback 逻辑 | 设定短超时时间（如 10ms），验证超时后行为 |
| static-check 失败流水线 | mock 子进程返回非 0 退出码，验证流水线跳过 unit-test 阶段 | 不依赖真实的语法错误代码 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| test-gen-generator 尝试读取 .ts 源码文件 | Read 工具调用路径包含 `src/auth.ts` | skill 层校验拦截，读取被拒绝，记录违规事件到 eval.json | `tests/test_file_type_isolation.py` |
| implementation-generator 尝试读取 `tests/` 目录 | Read 工具调用路径包含 `tests/test_auth.py` | skill 层校验拦截，读取被拒绝，记录违规事件到 eval.json | `tests/test_file_type_isolation.py` |
| 诊断决策树：语法错误在测试文件行第 15 行 | 报告显示 `test_login.py:15: SyntaxError: invalid syntax` | Evaluator 判定根因为 test-gen 阶段，设置 `backtrack_to: "04-test-gen"` | `tests/test_diagnostic_decision_tree.py` |
| 诊断决策树：逻辑错误在实现文件 `src/auth.py:42` | 报告显示 `FAILED src/auth.py:42 - AssertionError: expected True, got False` | Evaluator 判定根因为 implement 阶段，设置 `backtrack_to: "05-implementation"` | `tests/test_diagnostic_decision_tree.py` |
| 诊断决策树：测试期望值与 test-design.md 冲突 | Evaluator 读取报告发现测试名 `test_should_return_20_per_page` 但 test-design.md 定义 pageSize=10 | Evaluator 判定根因为 test-design 阶段，设置 `backtrack_to: "02-test-design"` | `tests/test_diagnostic_decision_tree.py` |
| 诊断决策树：接口签名双方都符合各自文档但互相不匹配 | 测试 mock 调用 `getUser(id: string)` 但实现提供 `getUser(userId: number)` | Evaluator 判定根因为 dev-proposal 阶段，设置 `backtrack_to: "03-dev-proposal"` | `tests/test_diagnostic_decision_tree.py` |
| 诊断决策树：多种错误同时存在（语法 + 逻辑） | 同一报告中测试文件有语法错误且实现文件有逻辑错误 | Evaluator 优先回溯到语法错误（test-gen），次优先逻辑错误（implement） | `tests/test_diagnostic_decision_tree.py` |
| 诊断决策树：无法归类的失败场景 | 报告显示 `Segmentation fault (core dumped)` 且无文件行号信息 | Evaluator 判定"无法判断"，调用 AskUserQuestion 工具，提问内容含测试名称和已排除项 | `tests/test_diagnostic_ask_user.py` |
| AskUserQuestion 超时（默认 5 分钟） | 构造"无法判断"场景，设置 askUserTimeout=10 秒，用户不响应 | 超时后自动设置 `backtrack_to: "03-dev-proposal"`，eval.json 的 findings 包含 `"ask_user_timeout": true` | `tests/test_diagnostic_ask_user.py` |
| no-op 判定：变更目录无 `*.test.*` 文件 | `glob.glob("**/*.test.*")` 返回空列表 | skill 层判定 unit-test 阶段为 no-op，eval.json 追加 `{"phase": "06-unit-test", "verdict": "pass", "skipped": true}` | `tests/test_noop_phase.py` |
| no-op 判定：变更目录无 `tests/integration/` 文件 | `glob.glob("**/tests/integration/**")` 返回空列表 | skill 层判定 integration-test 阶段为 no-op，eval.json 追加 `{"phase": "08-integration-test", "verdict": "pass", "skipped": true}` | `tests/test_noop_phase.py` |
| no-op 判定：变更目录有 `__tests__/` 但不含测试内容 | `glob.glob("**/*.test.*")` 匹配空文件或仅注释 | skill 层判定 unit-test 非 no-op（文件存在即视为有测试），Executor 正常执行 | `tests/test_noop_phase.py` |
| static-check 失败后 eval.json 状态 | implement 阶段 AUTO static-check 返回退出码 1 | 流水线直接回溯到 implementation-generator，eval.json 中无 06-unit-test 相关条目 | `tests/test_implement_static_check_fail.py` |
| eval-check 读取含 skipped 条目的 eval.json | eval.json 中 08-integration-test 条目含有 `"skipped": true` | eval-check 识别为已跳过，不阻塞归档流程，human-readable 输出包含 "skipped" | `tests/test_eval_check_noop.ts` |
| `dev-team archi validate` 输入含 views 块的 .c4 文件 | LikeC4 官方 example 的 `model.c4` 含 `views { ... }` 块 | 校验通过（`@likec4/core` 原生支持 views 语法，不自实现 parser 不会报错） | `tests/test_archi_validate_likec4.py` |
| `dev-team archi validate` 输入含 deployment 块的 .c4 文件 | LikeC4 官方 example 的 `deployment.c4` 含 `deployment { ... }` 块 | 校验通过（旧 archi_parser.py 不支持此语法会报错，新引擎 100% 兼容） | `tests/test_archi_validate_likec4.py` |
| `dev-team archi check --staged` 输入含跨文件 import 引用断链 | 文件 A 引用 `element .system.user` 但目标元素在另一文件 B 中已重命名 | 报告生成 violations 列表，violations 正确列出断链引用 | `tests/test_archi_check.py` |
| 模块边界契约：受影响的模块无 public API | requirements-planner 处理一个纯内部重构的模块，该模块无 export 函数 | spec.md 中的接口契约表格为空表（仅列标题），不报错 | `tests/test_requirements_module_boundary.py` |
| 模块边界契约：受影响的模块有 20+ 个 export 函数 | 模拟一个大型模块的 export 列表超过单页显示范围 | 契约表格完整列出所有函数签名，不分页/不截断 | `tests/test_requirements_module_boundary.py` |
| code-review 阶段 eval.json 条目数 | 执行完整的 code-review 阶段全流程 | eval.json 中 07-code-review 仅有且只有一条 evaluator 条目（无 integration-test-execution 条目） | `tests/test_code_review_phase_unittest.py` |

---

## 5. 测试数据

测试数据分为以下类别：

1. **mock 测试执行报告 JSON**（`tests/fixtures/` 目录存放）:
   - `unit-test-pass.json` — 全通过的单元测试报告
   - `unit-test-fail-syntax.json` — 测试文件语法错误报告
   - `unit-test-fail-logic.json` — 实现文件逻辑错误报告
   - `unit-test-fail-design-conflict.json` — 测试期望与 test-design.md 冲突的报告
   - `unit-test-fail-unknown.json` — 无法归类的失败报告（含 segmentation fault）
   - `unit-test-fail-mixed.json` — 语法错误 + 逻辑错误混合报告
   - `integration-test-pass.json` — 全通过的集成测试报告
   - `integration-test-fail-report-incomplete.json` — 报告缺少关键字段（如覆盖率缺失）

2. **mock C4 模型文件**（`tests/fixtures/` 目录存放）:
   - `simple-model.c4` — 包含 specification block + extend 层级 + metadata 块的基础 C4 文件
   - `likec4-official-views.c4` — 包含 views 块的 LikeC4 官方风格文件
   - `likec4-official-deployment.c4` — 包含 deployment 块的 LikeC4 文件
   - `likec4-official-tags.c4` — 包含 tags 声明的 LikeC4 文件
   - `multi-file-import/` 目录 — 含跨文件 import 引用的多文件 C4 模型

3. **mock eval.json 片段**（`tests/fixtures/` 目录存放）:
   - `eval-with-skipped.json` — 含 `"skipped": true` 条目的 eval.json
   - `eval-7-phase-old.json` — 旧的 7 阶段 eval.json（用于验证向后兼容）
   - `eval-with-backtrack.json` — 含 backtrack_to 标记的 eval.json

4. **archi 命令测试数据**:
   - `archi-query-expected-schema.json` — `dev-team archi query` 输出的预期 JSON schema（elements/relations 结构）

5. **Agent prompt 匹配模板**:
   - `test-gen-blacklist-patterns.txt` — test-gen-generator 应包含的源码文件类型黑名单列表
   - `implement-blacklist-directories.txt` — implementation-generator 应包含的目录黑名单列表

---

## 6. 不可测试项

- AskUserQuestion 工具的实际交互流程 — **原因**: AskUserQuestion 是 Claude Code 内建工具，在测试环境中不可用。通过 mock 工具调用来验证调用的参数的正确性（提问内容格式），异步超时逻辑通过 mock 计时器测试。真实交互流程在人工验收时验证。
- Agent prompt 约束在 Agent 运行时的实际执行力 — **原因**: prompt 约束是软性的，Agent 在理论上可能"越狱"。通过 skill 层的 Read 路径校验代码和 Generator 执行后的读取文件列表来间接约束。测试验证 prompt 内容包含约束文本和 skill 层校验逻辑正确，但不测试 Agent 是否遵守 prompt（这属于模型行为测试，不在 CI 范围内）。
- `@likec4/core` 对新版 C4 DSL 的未来兼容性 — **原因**: 依赖外部包的演进。`package.json` 锁定 `^1.56.0` 版本，后续升级需在升级前用测试文件回归验证。当前测试覆盖已知语法（views, deployment, tags, import, extend, metadata, specification block）。
- 9 阶段流水线的用户体验（认知负担） — **原因**: 用户体验属于主观评估，不通过自动化测试验证。通过在 proposal.md 中设计"免交互"阶段和 no-op 机制来缓解，效果由用户反馈确认。
- 不同 test framework（Jest/Vitest/Mocha/pytest/unittest）下测试执行报告格式的差异覆盖 — **原因**: Executor 运行测试的输出格式因框架而异。报告 schema 定义了统一的结构化格式，Executor 负责格式转换。测试验证的是转换后的 JSON schema 而非原生测试输出，原生框架的测试执行由各框架自身负责。
