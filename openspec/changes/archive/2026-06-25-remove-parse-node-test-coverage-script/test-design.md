# 测试设计: remove-parse-node-test-coverage-script

> **日期**: 2026-06-24

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | node-test `coverage_cmd` 为 `node --test --experimental-test-coverage`，不含 `tee`、`parse-node-test-coverage.mjs` 或管道符 `\|` | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test coverage_cmd 简化 (AC-1)` |
| AC-2 | `coverage_output` 为 `coverage/node-test-output.txt` | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test coverage_output 更改 (AC-2)` |
| AC-3 | `coverage_artifacts` 为 `['coverage/node-test-output.txt']` | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test coverage_artifacts 更改 (AC-3)` |
| AC-4 | `parse-node-test-coverage.mjs` 及其测试文件删除 | 不可测试 | — | 文件系统层面验证，无逻辑可测试 |
| AC-5 | `unit-test-executor.md` 中 node-test 覆盖率解析描述改为直接解析原始文本表格，通过 regex 匹配 `all files` 行 | 单元测试 | `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | `unit-test-executor.md — node-test 覆盖率解析方式 (AC-5)` |
| AC-5 | 同上（节点解析指令变更） | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test coverage_format 仍为 "node-test" (AC-5)` |
| AC-6 | `test-get-framework-config.test.ts` 和 `test-detect-frameworks.test.ts` 中所有测试通过 | 回归测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts`, `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | 全部 describe 块回归验证 (AC-6) |
| AC-7 | 从 `node-test-output.txt` 原始文本中通过 regex `all files` 行可正确提取 lines/branches/functions | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test 覆盖率产物路径指向原始文本 (AC-7)` |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-1)` | 正向 | node-test coverage_cmd 应为 "node --test --experimental-test-coverage"，不含 "tee"、"parse-node-test-coverage.mjs"、"\|" | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-1)` | 边界 | node-test coverage_cmd 为仅含 `--test` 和 `--experimental-test-coverage` 两个 flags 的单条 node 命令，shell 管道符 `\|`、命令链接符 `&&`、tee、或任何 .mjs 脚本路径出现次数均为 0 — 与 go-cover 模式的单命令结构对标 | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-1)` | 异常 | 其他框架（jest/vitest/etc.）的 coverage_cmd 不受影响，仍含各自 reporter 参数 | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-2)` | 正向 | node-test coverage_output 应为 "coverage/node-test-output.txt"，而非 "coverage/coverage-summary.json" | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-2)` | 边界 | node-test coverage_output 路径以 `.txt` 后缀结尾，区别于其他七框架的 `.json` 输出路径 — 确认覆盖率产物格式从结构化 JSON 转变为原始文本（与 go-cover 的 `coverage/func-summary.txt` 一致） | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-2)` | 正向 | 其他框架 coverage_output 路径不变（jest/vitest/bun 仍为 coverage/coverage-summary.json） | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-3)` | 正向 | node-test coverage_artifacts 应为 ["coverage/node-test-output.txt"]，而非 ["coverage/coverage-summary.json"] | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-3)` | 正向 | node-test coverage_cleanup 仍为 ["coverage"]（不变） | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-5)` | 正向 | node-test coverage_format 仍为 "node-test"（枚举值不变） | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `runTestGetFrameworkConfig — node-test (AC-7)` | 正向 | coverage_output 指向原始文本文件，非 JSON 文件 — 路径类型从 .json 变为 .txt | 修改 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `EXPECTED_CONFIGS — node-test 新条目` | 正向 | EXPECTED_CONFIGS 应包含 node-test 条目，覆盖全部 8 字段且与注册表一致 | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `NEW_FRAMEWORK_EXPECTED — node-test 旧期望值` | 修改 | NEW_FRAMEWORK_EXPECTED['node-test'] 的 coverage_cmd/coverage_output/coverage_artifacts 更新为简化值；旧版本（含 tee/parser）不再出现 | 废弃 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | `八框架完整性 — node-test` | 正向 | 遍历八个框架，node-test 的 test_cmd/coverage_cmd/coverage_artifacts/coverage_cleanup 均非空 | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks — node-test plan (AC-5)` | 正向 | config framework: "node-test" 时 plan 的 coverage_cmd 为简化命令，不含 "parse-node-test-coverage.mjs"；script 仍含 bash shebang | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks — node-test plan script 末行 (AC-6)` | 正向 | node-test plan 的 script 最后一行为简化后的覆盖命令，不含管道符 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks — node-test plan script 末行 (AC-6)` | 边界 | node-test plan 的 script 最后一行等于简化后的 coverage_cmd（"node --test --experimental-test-coverage"），确认 plan 正确传播了单条命令而非多步脚本链 | 新增 |
| `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | `unit-test-executor.md — node-test 覆盖率解析 (AC-5)` | 正向 | unit-test-executor.md 步骤 4 中 node-test 描述应说明 "regex match `all files` row"，而非 "Read parser-produced coverage-summary.json" | 修改 |
| `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | `unit-test-executor.md — node-test 覆盖率解析 (AC-5)` | 边界 | unit-test-executor.md 步骤 4 中 node-test 的 regex 提取描述应覆盖 `all files` 行三列（Lines、Branch、Funcs）的百分比值解析，包括 0.00%（最小值边界）和 100.00%（最大值边界） | 修改 |

### Mock 策略

| 测试文件 | Mock 主体 | Mock 方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | 无 | 纯函数测试，测试 `runTestGetFrameworkConfig` 直接返回注册表的浅拷贝对象，无需 mock | 全部 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | 文件系统 | `createTempProject` 辅助函数创建临时目录和 `openspec/config.json`；测试后通过 `cleanup()` 删除 | 全部 |
| `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/unit_test_executor_schema_static.test.ts` | 文件系统 | `readAgent` 读取 `unit-test-executor.md` 文件内容；`extractSection` 提取指定章节的文本段落 | 全部 |

---

## 集成测试

本变更不涉及新集成测试场景。变更范围局限在：

1. `FRAMEWORK_REGISTRY` 中 `node-test` 条目的三个字段值变更（无新增/删除字段，schema 不变）
2. `unit-test-executor.md` 步骤 4 中 `node-test` 覆盖率解析说明的文字更新（Markdown 文档，非代码）
3. 删除两个文件（`parse-node-test-coverage.mjs` 和 `parse-node-test-coverage.test.mjs`）

以上三者的行为均可通过单元测试充分覆盖。现有集成测试场景（如 `test_detect_frameworks_plan_json_only_coverage`、`test_detect_frameworks_plan_output`、`unit_test_executor_schema_static`）不依赖 `node-test` 特有的字段值，无需修改。

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| — | — | — | — | — |

### Mock 策略

| 测试文件 | Mock 主体 | Mock 方案 | 应用场景 |
|---------|---------|----------|----------|
| — | — | — | — |

---

## 不可测试项

- **AC-4: parse-node-test-coverage.mjs 及其测试文件删除** — 原因：该验收条件验证的是文件的"不存在"状态（git 删除操作），而非程序逻辑。可通过以下方式验证：
  - git 状态确认文件已被 `git rm`
  - CI 构建检查文件是否存在于工作区
  - 属于构建/版本管理范畴，不通过自动化测试覆盖

- **AC-6: 测试文件更新后全部通过** — 原因：这不是一个独立的测试用例，而是所有测试修改完成后执行 `npx vitest run` 的结果状态。已在各单元测试用例中通过回归断言覆盖（各修改后的 describe 块在更新后应继续通过）。

- **`plugins/dev-team/agents/unit-test-executor.md`（Markdown agent 文件）** — 原因：该文件是 agent 定义的自然语言指令文档，非可执行代码。对步骤 4 中 `node-test` 覆盖率解析描述的更新（从"Read parser-produced coverage-summary.json"改为"regex match `all files` row"）无法通过代码级别的单元测试验证。已由集成测试（`unit_test_executor_schema_static`）通过字符串匹配覆盖解析指令变更的合规性。
