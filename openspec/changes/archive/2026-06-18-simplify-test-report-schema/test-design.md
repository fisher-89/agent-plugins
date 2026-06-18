# 测试设计: simplify-test-report-schema

> **日期**: 2026-06-18
> **基于**: proposal.md, design.md

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | Executor 报告模板中覆盖率使用嵌套 `coverage` 对象（含 `pass`、`measured`、`thresholds`、`by_framework`、`overrides`），`by_framework` 条目不含 `html_report`；无顶层 `coverage_thresholds`/`coverage_pass`/`coverage_by_framework`/`coverage_overrides`/`html_reports` | 集成测试 | `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | unit-test-executor.md — 嵌套 coverage 报告模板 |
| AC-2 | 无覆盖率时报告仅含 `"coverage": null`，不含其他 coverage 相关顶层字段 | 集成测试 | `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | unit-test-executor.md — no-coverage 示例 |
| AC-3 | 失败用例仅在 `test_cases[]` 中，`status: "failed"` 条目含 `line`、`error_type`、`error_message`、`stack_trace`；无顶层 `failures[]` | 集成测试 | `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | unit-test-executor.md — 失败详情合并到 test_cases |
| AC-4 | `integration_test` 嵌套对象同样无 `failures[]`，失败详情在 `test_cases` 中 | 集成测试 | `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | unit-test-executor.md — integration_test 子报告 schema |
| AC-5 | Evaluator U1/U3 与 Step 1 引用新字段路径（`coverage.pass`、`coverage.measured` 等） | 集成测试 | `plugins/dev-team/bin/__tests__/unit_test_evaluator_schema_static/unit_test_evaluator_schema_static.test.ts` | unit-test-evaluator.md — U1/U3/Step 1 新 schema |
| AC-6 | Evaluator 决策树从 `test_cases` 失败条目读取 `error_type`、`file`、`line`，逻辑与改前等价 | 集成测试 | `plugins/dev-team/bin/__tests__/unit_test_evaluator_schema_static/unit_test_evaluator_schema_static.test.ts` | unit-test-evaluator.md — Step 4 决策树输入源 |
| AC-7 | `coverage === null` 时 Evaluator 覆盖率项自动 pass，evidence 为「覆盖率检查未配置或生成失败，跳过」 | 集成测试 | `plugins/dev-team/bin/__tests__/unit_test_evaluator_schema_static/unit_test_evaluator_schema_static.test.ts` | unit-test-evaluator.md — Step 3 覆盖率 null 跳过 |
| AC-8 | Evaluator findings 不引用任何 HTML 报告路径（`html_reports` 及 `html_report` 均已移除） | 集成测试 | `plugins/dev-team/bin/__tests__/unit_test_evaluator_schema_static/unit_test_evaluator_schema_static.test.ts` | unit-test-evaluator.md — Step 5 findings 模板 |
| AC-9 | `FRAMEWORK_REGISTRY` 中各框架 `coverage_cmd` 使用 JSON-only reporter 参数，`coverage_artifacts` 仅含 JSON 摘要文件路径 | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — JSON-only coverage 配置 |
| AC-9 | `test_detect_frameworks` 的 plan 条目从注册表继承 JSON-only `coverage_cmd` 与收窄后的 `coverage_artifacts` | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — plan JSON-only coverage 传播 |
| AC-9 | plan 中 `coverage_cmd`/`coverage_artifacts` 与注册表一致，且 `generateScript` 脚本末行等于新 `coverage_cmd` | 集成测试 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | test_detect_frameworks — plan 与 script 端到端一致性 |

---

## 单元测试

### 用例

#### AC-9: FRAMEWORK_REGISTRY JSON-only 覆盖率配置

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — jest JSON-only | 正向 | jest 返回 `coverage_cmd: "npx jest --coverage --coverageReporters=json-summary"`，`coverage_artifacts: ["coverage/coverage-summary.json"]` | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — vitest JSON-only | 正向 | vitest 返回 `coverage_cmd` 含 `--coverage.reporter=json-summary`，`coverage_artifacts` 为单元素 JSON 摘要路径 | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — vite-plus JSON-only | 正向 | vite-plus 返回 `coverage_cmd` 含 `--coverage.reporter=json-summary`，`coverage_artifacts: ["coverage/coverage-summary.json"]` | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — bun JSON-only | 正向 | bun 返回 `coverage_cmd` 含 `--coverageReporters=json-summary`，`coverage_artifacts: ["coverage/coverage-summary.json"]` | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — rust JSON-only | 正向 | rust 返回 `coverage_cmd: "cargo llvm-cov --json"`，`coverage_artifacts: ["coverage/coverage-summary.json"]`（不含 `target/llvm-cov/**`） | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 全部框架 JSON-only | 正向 | 遍历五个框架，`coverage_cmd` 均含 JSON reporter 参数，`coverage_artifacts` 均为 `["coverage/coverage-summary.json"]` | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — coverage_cleanup 不变 | 正向 | 各框架 `coverage_cleanup` 与变更前一致（jest/vitest/vite-plus 含 `.nyc_output`，rust 含 `target/llvm-cov`） | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — EXPECTED_CONFIGS 全量匹配 | 正向 | 更新 `EXPECTED_CONFIGS` 后，五个框架 `toEqual` 期望值全部通过 | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — vitest coverage_artifacts glob | 废弃 | 期望 `coverage_artifacts: ["coverage/**"]` 的断言 | 废弃 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — rust coverage_artifacts 双 glob | 废弃 | 期望 `coverage_artifacts` 含 `target/llvm-cov/**` 的断言 | 废弃 |

#### AC-9: test_detect_frameworks plan 传播 JSON-only 配置

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — vitest plan coverage_cmd | 正向 | vitest plan 条目 `coverage_cmd` 等于注册表 JSON-only 命令 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — vitest plan coverage_artifacts | 正向 | vitest plan 条目 `coverage_artifacts` 为 `["coverage/coverage-summary.json"]` | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — rust plan coverage_cmd | 正向 | rust plan 条目 `coverage_cmd` 为 `cargo llvm-cov --json` | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — 多框架 plan 传播 | 正向 | [vitest, rust] 时每个 plan 条目 `coverage_cmd`/`coverage_artifacts` 与对应框架注册表一致 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | generateScript — 脚本末行等于 JSON-only coverage_cmd | 正向 | vitest plan 生成的 `script` 最后一行等于含 `json-summary` 的 `coverage_cmd` | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — vitest plan coverage_artifacts glob | 废弃 | 期望 plan `coverage_artifacts: ["coverage/**"]` 的断言 | 废弃 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — rust plan 双 glob | 废弃 | 期望 rust plan `coverage_artifacts` 含 `target/llvm-cov/**` 的断言 | 废弃 |

#### 参数边界与异常（runTestGetFrameworkConfig）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — coverage_artifacts 单元素数组 | 边界 | 五个框架 `coverage_artifacts.length === 1` 且元素为 `coverage/coverage-summary.json` | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — coverage_cmd 非空 | 边界 | 五个框架 `coverage_cmd` 为非空字符串且包含 `coverage` 关键字 | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — unknown framework | 异常 | 未知框架名抛出 Error（行为不变） | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — empty framework | 异常 | 空字符串 framework 抛出 Error | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — nullish framework | 异常 | `null`/`undefined` framework 抛出 Error | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | 无 | `FRAMEWORK_REGISTRY` 为硬编码常量；直接调用 `runTestGetFrameworkConfig` 验证返回值 | 全部 AC-9 注册表测试 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `readConfig` / 文件系统 | 使用已有 `createTempProject` fixture 在临时目录写入 `openspec/config.json`；`runTestGetFrameworkConfig` 使用真实注册表 | plan 传播与 generateScript 测试 |

---

## 集成测试

### 用例

#### AC-1 ~ AC-4: unit-test-executor.md 静态 schema 校验

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1 | `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | unit-test-executor.md — 嵌套 coverage 报告模板 | Step 7 JSON 示例含 `"coverage": { "pass", "measured", "thresholds", "by_framework", "overrides" }` | 新增 |
| AC-1 | 同上 | unit-test-executor.md — 移除扁平 coverage 字段 | 报告 JSON 示例不含 `coverage_thresholds`、`coverage_pass`、`coverage_by_framework`、`coverage_overrides`、`html_reports` | 新增 |
| AC-1 | 同上 | unit-test-executor.md — by_framework 无 html_report | `by_framework` 条目不含 `html_report` 字段 | 新增 |
| AC-1 | 同上 | unit-test-executor.md — Process 无 HTML 引用 | Process 步骤 2/3/5 不含 `html_reports`、`html_report`、`index.html` | 新增 |
| AC-2 | 同上 | unit-test-executor.md — no-coverage 示例 | no-coverage JSON 块仅含 `"coverage": null`，不含 `coverage_thresholds`/`coverage_pass`/`coverage_by_framework`/`html_reports` | 新增 |
| AC-3 | 同上 | unit-test-executor.md — 失败详情在 test_cases | 主报告 `test_cases[]` 中 `status: "failed"` 条目含 `line`、`error_type`、`error_message`、`stack_trace` | 新增 |
| AC-3 | 同上 | unit-test-executor.md — 无顶层 failures | 报告 JSON 示例与 Step 6/7 说明不含顶层 `"failures"` 数组 | 新增 |
| AC-4 | 同上 | unit-test-executor.md — integration_test schema | `integration_test` 示例含 `test_cases[]` 失败条目 error 字段，不含 `failures[]` | 新增 |
| AC-4 | 同上 | unit-test-executor.md — Step 6 提取说明 | Step 6 提取列表不含 `failures[]`，失败详情写入 `test_cases` | 新增 |

#### AC-5 ~ AC-8: unit-test-evaluator.md 静态消费逻辑校验

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-5 | `plugins/dev-team/bin/__tests__/unit_test_evaluator_schema_static/unit_test_evaluator_schema_static.test.ts` | unit-test-evaluator.md — U1 必需字段 | Static Checklist U1 必需字段含 `coverage`（object 或 null）、`test_cases`；不含 `coverage_thresholds`、`coverage_pass`、`coverage_by_framework`、`html_reports`、`failures` | 新增 |
| AC-5 | 同上 | unit-test-evaluator.md — U3 判定依据 | U3 引用 `coverage.pass === true` 或 `coverage === null` | 新增 |
| AC-5 | 同上 | unit-test-evaluator.md — Step 1 校验 | Step 1 要求 `coverage` 为嵌套对象（含 `pass, measured, thresholds, by_framework, overrides`）或 `null` | 新增 |
| AC-5 | 同上 | unit-test-evaluator.md — Step 3 覆盖率子检查 | Step 3 引用 `coverage.pass`、`coverage.measured`、`coverage.thresholds`、`coverage.overrides` | 新增 |
| AC-6 | 同上 | unit-test-evaluator.md — Step 4 决策树输入 | Step 4 明确从 `test_cases.filter(c => c.status === "failed")` 读取 `error_type`、`file`、`line` | 新增 |
| AC-6 | 同上 | unit-test-evaluator.md — 决策树规则不变 | 决策树 5 类分类、优先级（设计冲突 > 语法 > 逻辑 > 接口 > 无法判断）文本保持 | 新增 |
| AC-7 | 同上 | unit-test-evaluator.md — coverage null 跳过 | Step 3 含 `coverage === null` 时 evidence「覆盖率检查未配置或生成失败，跳过」 | 新增 |
| AC-7 | 同上 | unit-test-evaluator.md — Constraints null 通过 | Constraints 含 `coverage === null` 时覆盖率检查始终 pass | 新增 |
| AC-8 | 同上 | unit-test-evaluator.md — Step 5 findings 无 HTML | Step 5 findings 模板引用 `coverage.measured`、`coverage.pass`、`coverage.by_framework[].measured`；不含 `html_reports`、`html_report`、`index.html` | 新增 |
| AC-8 | 同上 | unit-test-evaluator.md — 全文无旧字段 | 全文不含 `coverage_pass`、`coverage_thresholds`、`html_reports`（grep 断言） | 新增 |

#### AC-9: test_detect_frameworks plan 与 script 端到端

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-9 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | plan 与注册表一致 — 全框架 | 遍历 jest/vitest/vite-plus/bun/rust，临时项目配置各框架后 plan[0].coverage_cmd/coverage_artifacts 与 `runTestGetFrameworkConfig` 返回值一致 | 新增 |
| AC-9 | 同上 | generateScript 末行 — vitest | vitest plan 生成的 script 最后一行含 `--coverage.reporter=json-summary` | 新增 |
| AC-9 | 同上 | generateScript 末行 — jest | jest plan 生成的 script 最后一行含 `--coverageReporters=json-summary` | 新增 |
| AC-9 | 同上 | coverage_cleanup 未变 — rust | rust plan 仍含 `coverage_cleanup: ["coverage", "target/llvm-cov"]` | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | 无 | `fs.readFileSync` 读取 agent markdown；字符串/正则断言字段结构（参照 `agent-definitions-static.test.ts` 模式） | AC-1 ~ AC-4 |
| `plugins/dev-team/bin/__tests__/unit_test_evaluator_schema_static/unit_test_evaluator_schema_static.test.ts` | 无 | 同上，按章节（Static Checklist / Process Step 1~5 / Constraints）切片断言 | AC-5 ~ AC-8 |
| `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | `readConfig` / 文件系统 | `createTempProject` 注入 `openspec/config.json`；调用 `runTestDetectFrameworks` 与 `generateScript`，不 mock 注册表 | AC-9 端到端 plan/script |

---

## 不可测试项

- `plugins/dev-team/agents/unit-test-executor.md` — **原因**: MCP `test_resolve_paths` 判定为非可测试源文件；Agent 运行时行为（实际执行覆盖率命令、解析 JSON 摘要、写入报告）无进程内自动化 harness，通过静态 markdown 集成测试（AC-1 ~ AC-4）间接覆盖文档契约
- `plugins/dev-team/agents/unit-test-evaluator.md` — **原因**: MCP `test_resolve_paths` 判定为非可测试源文件；Evaluator 决策树运行时判定依赖 LLM 读取报告上下文，无法在无 Agent 环境下做确定性 E2E；通过静态 markdown 集成测试（AC-5 ~ AC-8）覆盖字段路径与流程说明
- Evaluator 决策树「逻辑与改前等价」的语义等价性 — **原因**: 变更仅切换输入源（`failures[]` → `test_cases.filter(failed)`），判定规则文本不变；等价性由 AC-6 静态断言（规则文本未变 + 输入源已切换）与人工 review 共同保证，无法编写独立自动化测试对比改前改后 Agent 输出
- 覆盖率加权平均算法与 `coverage === null` 写入时机 — **原因**: proposal/design 明确不在本次变更范围；Executor 文档更新不涉及算法变更，无需新增测试
- `integration-test-executor.md` / `integration-test-evaluator.md` — **原因**: proposal 明确不在变更范围
- 已归档变更目录中的历史 `unit-test-execution.json` — **原因**: proposal 明确不修改历史报告
- `command` 字段单字符串结构 — **原因**: 留待后续变更，本次不测试
