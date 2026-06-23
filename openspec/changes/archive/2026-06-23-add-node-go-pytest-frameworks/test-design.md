# 测试设计: add-node-go-pytest-frameworks

> **日期**: 2026-06-23

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `testFrameworkSchema` 接受 `node-test`、`go`、`pytest` 作为有效框架名 | 单元测试 | `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.framework enum validation |
| AC-1 | 无效名如 `"mocha"` 仍被拒绝 | 单元测试 | `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.framework enum validation |
| AC-1 | `test.overrides` 条目中 framework 为 `go` / `node-test` / `pytest` 时通过验证 | 单元测试 | `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.overrides framework 字段 |
| AC-2 | `test_get_framework_config({"framework": "go"})` 返回完整配置，含 `coverage_format: "go-cover"` | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — go |
| AC-2 | 八框架注册表每条返回非空 `test_cmd`、`coverage_cmd`、`coverage_artifacts`、`coverage_cleanup` | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 八框架完整性 |
| AC-2 | 未知框架名抛出错误，错误信息列出全部八个框架名 | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 未知框架 |
| AC-2 | 空字符串、null、undefined 框架名均抛出 | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 边界 |
| AC-2 | 新框架 `default_glob` 映射正确 | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | getDefaultGlobForFramework |
| AC-3 | `test_get_framework_config({"framework": "node-test"})` 的 `coverage_cmd` 为两步脚本（native coverage + parser） | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — node-test |
| AC-4 | `test_get_framework_config({"framework": "pytest"})` 返回 `coverage_format: "coverage-py"`、`coverage_output: "coverage.json"` | 单元测试 | `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — pytest |
| AC-5 | `test_get_framework_config` 输出 schema 的 `coverage_format` 接受五个值 | 单元测试 | `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts` | testGetFrameworkConfigOutputSchema — coverage_format |
| AC-5 | `test_detect_frameworks` 输出 schema 的 `coverage_format` 枚举含五个值 | 单元测试 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan coverage_format |
| AC-5 | config `framework: "go"` 时 plan 含 `coverage_format: "go-cover"` 及注册表 artifacts/cleanup | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — go plan |
| AC-5 | config `framework: "node-test"` 时 plan 的 `coverage_cmd` 为两步脚本且 `script` 含 bash shebang | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — node-test plan |
| AC-5 | config `framework: "pytest"` 时 plan 含 `coverage_format: "coverage-py"` | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — pytest plan |
| AC-5 | 配置 `[vitest, go]` 时 plan 两条目各自携带正确 coverage_format | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — 多框架 |
| AC-5 | `*_test.go` 文件在 go 配置下映射为 `go` 框架 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — glob 检测 |
| AC-6 | `parse-node-test-coverage.mjs` 从 node:test stdout 文本表格提取 lines/branches/functions 并写入 istanbul 兼容的 `coverage-summary.json` | 单元测试 | `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage |
| AC-6 | CLI 缺少输出路径或输入路径为空字符串时以非零退出码失败 | 单元测试 | `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage — CLI 参数 |
| AC-7 | Go 框架解析后 `measured.branches` 和 `measured.functions` 为 `null`，`measured.lines` 为数值 | 不可测试（见下文） | — | unit-test-executor Agent 运行时解析 |
| AC-8 | pytest 框架解析后 `measured.functions` 为 `null`，`measured.lines` 和 `measured.branches` 为数值 | 不可测试（见下文） | — | unit-test-executor Agent 运行时解析 |
| AC-9 | null 维度跳过阈值比较：`coverage.pass` 计算时不因 null 维度失败 | 不可测试（见下文） | — | Go-only 项目端到端门控 |
| AC-10 | 加权平均计算时 null 维度不参与：`measured.branches` 仅由提供 branches 值的框架贡献 | 不可测试（见下文） | — | 多框架混合端到端聚合 |
| AC-11 | Evaluator 对 null 维度在 findings 中标注 "N/A (框架不支持)" | 不可测试（见下文） | — | unit-test-evaluator Agent 文档规则 |
| AC-12 | `test-framework.py` 检测到 `go.mod` 时返回 `("go", "go test ./...")` | 单元测试 | `plugins/dev-team/utils/test_test-framework.py` | detect_test_framework — Go |
| AC-12 | 同时含 `go.mod` 与 `package.json` 时按设计检测顺序返回预期框架 | 单元测试 | `plugins/dev-team/utils/test_test-framework.py` | detect_test_framework — 优先级 |
| AC-12 | 无 `go.mod` 时不返回 `go` 框架 | 单元测试 | `plugins/dev-team/utils/test_test-framework.py` | detect_test_framework — 无 Go |
| AC-12 | `project_root` 为空字符串或不存在路径时行为与现有惯例一致 | 单元测试 | `plugins/dev-team/utils/test_test-framework.py` | detect_test_framework — 边界 |
| AC-13 | `test-runner.py` 支持 `go` 和 `node-test` 框架运行 | 单元测试 | `plugins/dev-team/utils/test_test-runner.py` | run_tests — go |
| AC-13 | `framework="node-test"` 时构建 `node --test` 命令 | 单元测试 | `plugins/dev-team/utils/test_test-runner.py` | run_tests — node-test |
| AC-13 | go / node-test stdout fixture 正确解析 passed/failed/total | 单元测试 | `plugins/dev-team/utils/test_test-runner.py` | run_tests — 输出解析 |
| AC-13 | 传入未扩展前的未知框架名抛出 `ValueError` 或返回失败 TestResult | 单元测试 | `plugins/dev-team/utils/test_test-runner.py` | run_tests — 不支持框架 |
| AC-13 | `timeout=0` 或极大 timeout、`test_file=None` 时不崩溃 | 单元测试 | `plugins/dev-team/utils/test_test-runner.py` | run_tests — 边界 |

---

## 单元测试

### 用例

#### AC-1: testFrameworkSchema 新框架名

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.framework enum validation | 正向 | 应接受 `node-test`、`go`、`pytest` 三个新枚举值 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.framework enum validation | 正向 | 应仍接受既有五框架（jest/vitest/vite-plus/bun/rust） | 废弃 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.framework enum validation | 异常 | 应拒绝无效框架名 `"mocha"` | 废弃 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.framework enum validation | 边界 | 应拒绝空字符串 `""` 作为 framework | 废弃 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.framework enum validation | 边界 | 应拒绝非字符串 framework 值（如 `123`） | 废弃 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | test.overrides framework 字段 | 正向 | overrides 条目中 framework 为 `go` / `node-test` / `pytest` 时通过验证 | 新增 |

#### AC-2 ~ AC-4: FRAMEWORK_REGISTRY 三条新框架

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — go | 正向 | 返回 `coverage_format: "go-cover"`、`coverage_output: "coverage/func-summary.txt"` 及完整 artifacts/cleanup/default_glob | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — node-test | 正向 | `coverage_cmd` 含 `--experimental-test-coverage`、`tee`、`parse-node-test-coverage.mjs` 两步脚本 | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — pytest | 正向 | 返回 `coverage_format: "coverage-py"`、`coverage_output: "coverage.json"` | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 八框架完整性 | 正向 | 遍历全部八个框架，每个返回非空 `test_cmd`、`coverage_cmd`、`coverage_artifacts`、`coverage_cleanup` | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 未知框架 | 异常 | 未知框架名抛出错误，错误信息列出全部八个框架名 | 新增 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 边界 | 边界 | 空字符串、null、undefined 框架名均抛出 | 废弃 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | getDefaultGlobForFramework | 正向 | `node-test` → `**/*.test.{mjs,js,cjs}`；`go` → `**/*_test.go`；`pytest` → `**/test_*.py` | 新增 |

#### AC-5: coverage_format 五值枚举 schema

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts` | testGetFrameworkConfigOutputSchema — coverage_format | 正向 | `istanbul`、`llvm-cov`、`node-test`、`go-cover`、`coverage-py` 均通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts` | testGetFrameworkConfigOutputSchema — coverage_format | 异常 | 非法值 `cobertura` 被拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts` | testGetFrameworkConfigOutputSchema — coverage_format | 边界 | 大小写敏感：`Istanbul` 拒绝，`istanbul` 通过 | 废弃 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan coverage_format | 正向 | plan 条目 `coverage_format` 为 `node-test` / `go-cover` / `coverage-py` 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan coverage_format | 异常 | 非法枚举值（如 `cobertura`）被拒绝 | 废弃 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan coverage_format | 边界 | 空字符串 coverage_format 被拒绝 | 新增 |

#### AC-5（续）: test_detect_frameworks plan 生成含新框架

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — go plan | 正向 | config `framework: "go"` 时 plan 含 `coverage_format: "go-cover"` 及注册表 artifacts/cleanup | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — node-test plan | 正向 | config `framework: "node-test"` 时 plan 的 `coverage_cmd` 为两步脚本且 `script` 含 bash shebang | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — pytest plan | 正向 | config `framework: "pytest"` 时 plan 含 `coverage_format: "coverage-py"` | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — 多框架 | 正向 | 配置 `[vitest, go]` 时 plan 两条目各自携带正确 coverage_format | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — glob 检测 | 正向 | `*_test.go` 文件在 go 配置下映射为 `go` 框架 | 新增 |

#### AC-6: parse-node-test-coverage.mjs

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage | 正向 | 有效 node:test stdout fixture 解析出 lines/branches/functions 百分比并写入 istanbul 兼容 `coverage-summary.json`（含 `total` 节点） | 新增 |
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage | 正向 | 输出 JSON 结构与现有 istanbul `coverage-summary.json` 字段兼容（`lines.pct`、`branches.pct`、`functions.pct`） | 新增 |
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage | 异常 | 输入文件不存在时以非零退出码失败并抛出/记录错误 | 新增 |
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage | 异常 | 输入为空文件或非表格文本时失败或以 0 值降级（与实现 D8 一致） | 新增 |
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage | 边界 | 输入仅含表头无数据行时行为确定（0 或失败，与实现一致） | 新增 |
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage | 边界 | 缺失 branches 或 functions 列时对应维度为 0 或按 parser 规则处理 | 新增 |
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage — CLI 参数 | 边界 | 缺少输出路径参数时使用非零退出码 | 新增 |
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | parseNodeTestCoverage — CLI 参数 | 边界 | 输入路径为空字符串时失败 | 新增 |

#### AC-12: test-framework.py Go 检测

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/utils/test_test-framework.py` | detect_test_framework — Go | 正向 | 仅含 `go.mod` 的临时目录返回 `("go", "go test ./...")` | 新增 |
| `plugins/dev-team/utils/test_test-framework.py` | detect_test_framework — 优先级 | 边界 | 同时含 `go.mod` 与 `package.json` 时按设计检测顺序返回预期框架（与实现 D9 一致） | 新增 |
| `plugins/dev-team/utils/test_test-framework.py` | detect_test_framework — 无 Go | 正向 | 无 `go.mod` 时不返回 `go` 框架 | 新增 |
| `plugins/dev-team/utils/test_test-framework.py` | detect_test_framework — 边界 | 边界 | `project_root` 为空字符串或不存在路径时返回 `("unknown", "")` 或抛出（与现有惯例一致） | 新增 |

#### AC-13: test-runner.py go / node-test 运行器

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/utils/test_test-runner.py` | run_tests — go | 正向 | `framework="go"` 时构建 `go test` 命令（mock subprocess，验证 cmd 与 cwd） | 新增 |
| `plugins/dev-team/utils/test_test-runner.py` | run_tests — node-test | 正向 | `framework="node-test"` 时构建 `node --test` 命令 | 新增 |
| `plugins/dev-team/utils/test_test-runner.py` | run_tests — 输出解析 | 正向 | go / node-test stdout fixture 正确解析 passed/failed/total | 新增 |
| `plugins/dev-team/utils/test_test-runner.py` | run_tests — 不支持框架 | 异常 | 传入未扩展前的未知框架名抛出 `ValueError` 或返回失败 TestResult | 新增 |
| `plugins/dev-team/utils/test_test-runner.py` | run_tests — 边界 | 边界 | `timeout=0` 或极大 timeout 不崩溃（与现有 runner 行为一致） | 新增 |
| `plugins/dev-team/utils/test_test-runner.py` | run_tests — 边界 | 边界 | `test_file=None` 时 go 使用 `./...`、node-test 使用默认 glob 或项目根 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | 无 | 直接调用 `runTestGetFrameworkConfig`，注册表为硬编码常量 | AC-2 ~ AC-4 全部框架配置断言 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `readConfig` / 文件系统 | `createTempProject` 在临时目录写入 `openspec/config.json`；真实调用 `runTestDetectFrameworks` | plan 生成、glob 映射 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | 无 | `configSchema.safeParse()` 纯内存验证 | AC-1 枚举 |
| `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts` | 无 | `testGetFrameworkConfigOutputSchema.safeParse()` | coverage_format 枚举 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | 无 | `testDetectFrameworksOutputSchema.safeParse()` | plan coverage_format 枚举 |
| `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | 文件系统 | 使用 `fixtures/` 下 node:test stdout 样本；`fs.readFileSync` / 临时输出目录；不执行真实 `node --test` | AC-6 解析逻辑 |
| `plugins/dev-team/utils/test_test-framework.py` | 文件系统 | `tempfile.TemporaryDirectory` 创建含/不含 `go.mod` 的目录树 | AC-12 |
| `plugins/dev-team/utils/test_test-runner.py` | `subprocess.run` | `unittest.mock.patch` 拦截 subprocess，断言命令字符串与 cwd；不执行真实 go/node | AC-13 |

---

## 集成测试

> **说明**：proposal.md 明确本次变更不涉及集成测试覆盖率（08-integration-test）。下列 AC 的运行时端到端验证留待后续变更；本次以单元测试 + Agent 文档审核覆盖。

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| — | — | — | 本次无新增集成测试 | — |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| — | — | — | 本次无 |

---

## 不可测试项

- **`plugins/dev-team/agents/unit-test-executor.md`** — **原因**: `test_resolve_paths` 返回 `Not a testable source file`。AC-7、AC-8、AC-9、AC-10 依赖 Agent 在真实/模拟执行报告中按 `go-cover` / `coverage-py` 解析 measured 并做 null 维度门控与加权平均，无 TypeScript 可调用实现层；**缓解**: 实现阶段人工审核 executor 文档 Step 4/5 规则；可选扩展 `plugins/dev-team/bin/__tests__/unit_test_executor_schema_static/` 静态校验（非 MCP 解析路径，不作为自动化门禁路径）。
- **`plugins/dev-team/agents/unit-test-evaluator.md`** — **原因**: 同上，Agent Markdown 不可由 `test_resolve_paths` 解析。**缓解**: 人工审核 evaluator 文档含 null 维度 findings 标注 `"N/A (框架不支持)"`（AC-11）；可选扩展 `plugins/dev-team/bin/__tests__/unit_test_evaluator_schema_static/` 静态字符串断言。
- **AC-7 / AC-8 — Go/pytest 覆盖率 measured null 映射** — **原因**: 解析逻辑在 Agent 提示词内，需真实 `func-summary.txt` / `coverage.json` 产物与 Executor 执行链路；proposal 排除集成测试。**缓解**: 单元测试覆盖注册表与 schema；文档审核 nullable 维度映射表。
- **AC-9 — Go-only 项目 coverage.pass 门控** — **原因**: 需构造完整变更目录、执行 bash plan 脚本并生成 `unit-test-execution.json`。**缓解**: 设计文档 D6/D8 规则审核；后续 08-integration-test 复用。
- **AC-10 — 多框架 null 维度加权平均** — **原因**: 需 vitest+go 等多框架混合执行与报告聚合，超出本次单元测试范围。**缓解**: executor Step 5 文档规则审核；后续集成场景补测。
- **node:test 两步脚本在 Windows 原生 shell 下的 bash/tee** — **原因**: 依赖 Git Bash/WSL 环境，CI 矩阵外难以稳定自动化。**缓解**: plan 生成单元测试断言命令字符串；Windows 失败走 findings + measured=0（设计 D8）。
