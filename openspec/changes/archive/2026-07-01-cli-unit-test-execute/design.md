# 设计: CLI 单元测试执行下沉

> **变更**: cli-unit-test-execute
> **日期**: 2026-06-30
> **版本**: v2 (backtrack — 移除 merge_mode，统一链式命令)

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| unit-test command handler | CLI action handler，编排 test-runner + parser + report 全流程；读取 config 获取阈值 | `plugins/dev-team/bin/src/commands/unit-test.ts` | test-runner, test-detect-frameworks, test-report, config, project-root | TypeScript, cac |
| test-runner | 执行单一 test_cmd（链式命令模式），模板占位符替换，收集 stdout/stderr/exitCode；无 merge_mode 分支 | `plugins/dev-team/bin/src/lib/test-runner.ts` | exec-command, test-parser, coverage-parser, test-framework | TypeScript, node:child_process |
| parser dispatch | 根据 framework 类型分发到子 parser | `plugins/dev-team/bin/src/lib/test-parser/index.ts` | json-parser, go-parser, text-parser | TypeScript 纯函数 |
| json-parser | 解析 vitest/jest `--reporter=json` 输出的 JSON 格式测试结果 | `plugins/dev-team/bin/src/lib/test-parser/json-parser.ts` | 无 | TypeScript 纯函数 |
| go-parser | 逐行解析 `go test -json` 的 line-delimited JSON，过滤非终态 Action 行 | `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts` | 无 | TypeScript 纯函数 |
| text-parser | 多层 fallback 解析文本输出：精确正则/通用 regex/逐行 heuristic | `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts` | 无 | TypeScript 纯函数 |
| coverage-parser | 按 format 读取并解析覆盖率文件（5种格式） | `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.ts` | 无（读取文件系统） | TypeScript |
| test-report | 生成子报告（per-framework）和汇总报告，阈值判定，coverage.pass 计算 | `plugins/dev-team/bin/src/lib/test-report.ts` | config, unit-test-output schema, exec-command | TypeScript |
| unit-test-output schema | 子报告和汇总报告的 Zod v4 schema 定义及类型推断 | `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` | zod | TypeScript, Zod v4 |
| test-framework registry | FrameworkConfig 存储；**`merge_mode` 已移除**；pytest/rust 使用链式命令；`coverage_cmd` 保留但不再被 runner 使用 | `plugins/dev-team/bin/src/lib/test-framework.ts` | 无（纯数据） | TypeScript |
| CLI entry | 注册 `dev-team unit-test` 子命令，解析 CLI 选项 | `plugins/dev-team/bin/src/cli.ts` | cac, unit-test command | TypeScript |

### 报告输出路径

报告路径根据 `--change <name>` 选项决定：

- `--change` 提供时：`<projectRoot>/openspec/changes/<change>/reports/unit-test/<framework>.json` 和 `.../reports/unit-test-execution.json`
- `--change` 省略时：`<projectRoot>/reports/unit-test/<framework>.json` 和 `<projectRoot>/reports/unit-test-execution.json`

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
  签名格式：Python → create_adr(title: str, status: str = "proposed") -> dict；TS → function parseImports(file: string): Import[]
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/commands/unit-test.ts` | CLI action handler：调用 runTestDetectFrameworks 获取 plan，逐 framework 执行，生成子/汇总报告 |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 执行编排：模板替换、执行单一 test_cmd（无 merge_mode 分支，统一执行）、收集执行结果、派发 parser |
| `plugins/dev-team/bin/src/lib/test-parser/index.ts` | Parser dispatch：根据 framework 名称将 stdout 路由到 json-parser / go-parser / text-parser |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.ts` | 解析 vitest/jest JSON reporter 输出的 testResults[].assertionResults[] |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts` | 逐行解析 go test -json line-delimited JSON，只统计 Action=pass/fail/skip 的行 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts` | 多层 fallback 解析自由文本输出：bun [PASS]/[FAIL]、cargo test result、node-test #pass/#fail |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.ts` | 读取覆盖率文件并解析 5 种格式：istanbul、llvm-cov、go-cover、node-test（文本表格）、coverage-py |
| `plugins/dev-team/bin/src/lib/test-report.ts` | 子报告生成写入 `reports/unit-test/<fw>.json`；汇总报告生成写入 `reports/unit-test-execution.json`；阈值判定 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` | Zod v4 schema：unitTestSubReportSchema、unitTestSummaryReportSchema |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/test-framework.ts` | **移除** `merge_mode: boolean` 字段；pytest/rust 的 `test_cmd` 更新为链式命令（`; _X=$?; ...; exit $_X`）；其他框架 `test_cmd` 更新为模板格式；`coverage_cmd` 保留但不再被 test-runner 使用 | AC-2: pytest/rust 链式命令；AC-12: 无 merge_mode |
| `plugins/dev-team/bin/src/cli.ts` | 注册 `dev-team unit-test` 子命令（--change, --project-root, --files, --framework），关联 `runUnitTest` action handler | AC-1: CLI 命令入口 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `PlanEntry` 接口移除 `merge_mode: boolean`；`buildPlanFromMappings` 不再填充 merge_mode | AC-12: 无 merge_mode |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | plan 数组条目的 Zod schema 移除 `merge_mode` 字段 | AC-12: 无 merge_mode |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `runUnitTest` | `commands/unit-test.ts` | 新增 | `function runUnitTest(options: UnitTestOptions): number` | CLI 入口：获取 plan → 逐 framework 执行 → 生成子/汇总报告 → 返回退出码 |
| `executePlanEntry` | `lib/test-runner.ts` | 新增 | `function executePlanEntry(entry: PlanEntry, projectRoot: string, options: { files?: string[] }): ExecutionResult` | 执行单个 plan entry：模板替换 → 执行单一 test_cmd（无 merge_mode 分支）→ 调用 parser → 返回 ExecutionResult |
| `parseTestOutput` | `lib/test-parser/index.ts` | 新增 | `function parseTestOutput(stdout: string, stderr: string, framework: string): ParsedTestResult` | 按 framework 分发到子 parser |
| `parseJsonOutput` | `lib/test-parser/json-parser.ts` | 新增 | `function parseJsonOutput(stdout: string): ParsedTestResult` | 解析 vitest/jest JSON reporter 输出 |
| `parseGoOutput` | `lib/test-parser/go-parser.ts` | 新增 | `function parseGoOutput(stdout: string): ParsedTestResult` | 逐行解析 go test -json line-delimited JSON |
| `parseTextOutput` | `lib/test-parser/text-parser.ts` | 新增 | `function parseTextOutput(output: string): ParsedTestResult` | 多层 fallback 解析文本测试输出 |
| `parseCoverageFromFile` | `lib/test-parser/coverage-parser.ts` | 新增 | `function parseCoverageFromFile(filePath: string, format: string): ParsedCoverage \| null` | 读取并解析覆盖率文件 |
| `generateSubReport` | `lib/test-report.ts` | 新增 | `function generateSubReport(framework: string, result: ExecutionResult, projectRoot: string, reportsDir: string): UnitTestSubReport` | 生成 per-framework 子报告对象并写入文件 |
| `generateSummaryReport` | `lib/test-report.ts` | 新增 | `function generateSummaryReport(subReports: UnitTestSubReport[], projectRoot: string, reportsDir: string): UnitTestSummaryReport` | 聚合子报告生成汇总报告并写入文件 |
| `getFrameworkConfig` | `lib/test-framework.ts` | 修改 | `function getFrameworkConfig(framework: string): FrameworkConfig` | FrameworkConfig **移除** `merge_mode`；pytest/rust `test_cmd` 为链式命令 |
| `runTestDetectFrameworks` | `commands/test-detect-frameworks.ts` | 修改 | `function runTestDetectFrameworks(options: TestDetectFrameworksOptions): TestDetectFrameworksResult` | 返回的 PlanEntry 数组移除 `merge_mode` |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `UnitTestOptions` | `commands/unit-test.ts` | interface (新增) | `{ change?: string; projectRoot?: string; files?: string[]; framework?: string }` |
| `ExecutionResult` | `lib/test-runner.ts` | interface (新增) | `{ framework: string; exitCode: number; stdout: string; stderr: string; testCases: TestCase[]; coverage: ParsedCoverage \| null; durationMs: number; testFiles: string[]; sourceFiles: string[]; error?: string }` |
| `ParsedTestResult` | `lib/test-parser/index.ts` | interface (新增) | `{ total: number; passed: number; failed: number; skipped: number; testCases: TestCase[]; testFiles: string[]; sourceFiles: string[]; error?: string }` |
| `TestCase` | `lib/test-parser/index.ts` | interface (新增) | `{ name: string; file?: string; durationMs?: number; status: 'passed' \| 'failed' \| 'skipped'; line?: number; errorType?: string; errorMessage?: string; stackTrace?: string }` |
| `ParsedCoverage` | `lib/test-parser/coverage-parser.ts` | interface (新增) | `{ lines: number; branches: number \| null; functions: number \| null; fileCoverage: FileCoverageEntry[] \| null }` |
| `CoverageThresholds` | `lib/test-report.ts` | interface (新增) | `{ lines: number; branches: number; functions: number }` |
| `UnitTestSubReport` | `schemas/unit-test-output.schema.ts` | type (新增) | 子报告类型，见数据模型 |
| `UnitTestSummaryReport` | `schemas/unit-test-output.schema.ts` | type (新增) | 汇总报告类型，见数据模型 |
| `FrameworkConfig` | `lib/test-framework.ts` | interface (修改) | **移除** `merge_mode: boolean`；`test_cmd` 改为模板字符串；pytest/rust 为链式命令 |
| `PlanEntry` | `commands/test-detect-frameworks.ts` | interface (修改) | **移除** `merge_mode: boolean`；保留 `test_cmd: string`、`coverage_cmd: string`（coverage_cmd 保留用于 backward compatibility，不再被 runner 使用） |

---

## 数据模型

### TestCase（解析器中间输出）

```typescript
interface TestCase {
  name: string;                  // 测试用例全名（含层级，如 "describe > it title"）
  file: string;                  // 测试文件路径（相对 project_root）
  durationMs?: number;           // 执行耗时（ms）
  status: 'passed' | 'failed' | 'skipped';
  line?: number;                 // 失败行号（仅 failed）
  errorType?: string;            // 错误类型（仅 failed）
  errorMessage?: string;         // 错误消息（仅 failed）
  stackTrace?: string;           // 堆栈信息（仅 failed）
}
```

### ParsedCoverage（覆盖率中间输出）

```typescript
interface ParsedCoverage {
  lines: number;                 // 行覆盖率百分比
  branches: number | null;       // 分支覆盖率百分比（不支持时为 null）
  functions: number | null;      // 函数覆盖率百分比（不支持时为 null）
  fileCoverage: Array<{          // 文件级覆盖率（null 当框架不支持时）
    file: string;
    lines: number;
    branches: number | null;
    functions: number | null;
  }> | null;
}
```

### UnitTestSubReport（子报告）

写入路径：`reports/unit-test/<framework>.json`

| 字段 | 类型 | 说明 |
|------|------|------|
| `framework` | `string` | 框架标识 |
| `timestamp` | `string` (ISO 8601) | 报告生成时间 |
| `exit_code` | `number` | 命令退出码 |
| `duration_ms` | `number` | 执行耗时 |
| `summary` | `{ total, passed, failed, skipped }` | 用例汇总 |
| `test_cases[]` | `TestCase[]` | 用例清单（含失败详情） |
| `test_files[]` | `string[]` | 涉及的所有测试文件路径 |
| `source_files[]` | `string[]` | 对应的源文件路径 |
| `file_coverage` | `FileCoverageEntry[] \| null` | 文件级覆盖率（框架不支持时为 null） |
| `coverage` | `CoverageBlock \| null` | 覆盖率结论（命令失败时为 null） |

### UnitTestSummaryReport（汇总报告）

写入路径：`reports/unit-test-execution.json`

| 字段 | 类型 | 说明 |
|------|------|------|
| `phase` | `string` | 固定为 `"06-unit-test"` |
| `command` | `string` | 实际执行的 CLI 命令描述 |
| `timestamp` | `string` (ISO 8601) | 报告生成时间 |
| `duration_seconds` | `number` | 总耗时（秒） |
| `total` | `number` | 聚合用例总数 |
| `passed` | `number` | 聚合通过数 |
| `failed` | `number` | 聚合失败数 |
| `skipped` | `number` | 聚合跳过数 |
| `conclusion` | `"pass" \| "fail" \| "error"` | 总体结论 |
| `problems[]` | `Problem[]` | 问题列表（每个 problem 含 framework, type, message） |
| `coverage` | `CoverageResult \| null` | 覆盖率（所有框架失败时为 null，不含 test_cases 等明细） |

### 覆盖率加权平均规则

权重 = `source_files.length`。每个维度独立计算：

```
weighted_<dim> = Σ(fw.measured.<dim> * fw.source_files.length)
                 / Σ(fw.source_files.length)
                 // 仅对 measured.<dim> !== null 的框架求和
```

- 某框架某维度为 `null` → 不参与该维度加权平均
- 所有框架某维度均为 `null` → 总体该维度为 `null`

### coverage.pass 判定

1. **全局**: 每个非 null 维度 `measured.<dim> >= thresholds.<dim>`；null 维度跳过（自动通过）
2. **overrides**（可选）: 每个 override 匹配的目录独立计算，阈值继承全局；null 维度跳过
3. **ALL 通过** → `pass = true`；任一非 null 维度不达标 → `pass = false`
4. `coverage === null`（全框架覆盖率失败）→ 不计算 pass

### 退出码映射

| 条件 | 退出码 |
|------|--------|
| 全部通过且覆盖率达标（或未配置/null） | 0 |
| 存在测试失败或覆盖率不达标 | 1 |
| plan 为空（未配置 test.framework） | 0 |

---

## 各框架 test_cmd（无 merge_mode，链式命令统一）

| 框架 | test_cmd | 说明 |
|------|----------|------|
| jest | `npx jest --verbose --json --coverage --coverageReporters=json-summary {files}` | 单命令（测试+覆盖率一次完成） |
| vitest | `npx vitest run --reporter=json --coverage --coverage.reporter=json-summary {files}` | 单命令 |
| vite-plus | `vp test --coverage --coverage.reporter=json-summary {files}` | 单命令 |
| bun | `bun test --coverage --coverageReporters=json-summary {files}` | 单命令 |
| node-test | `node --test --experimental-test-coverage {files}` | 单命令 |
| go | `go test -json -coverprofile=coverage.out -covermode=atomic {directory}` | 单命令 |
| rust | `cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X` | 链式命令 |
| pytest | `pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X` | 链式命令 |

`coverage_cmd` 字段保留在 `FrameworkConfig` 中但不再被 test-runner 使用（链式命令已内嵌覆盖率步骤）。`coverage_cmd` 的存在仅用于向后兼容和 reference。

### 链式命令行为

链式命令中的关键模式 `; _X=$?; ...; exit $_X` 确保：

1. `pytest -v {files}` 先执行，测试结果输出到 stdout
2. `_X=$?` 保存测试命令的退出码（无论测试通过或失败）
3. `pytest --cov=. ...` 覆盖率命令执行（即使测试失败也执行），输出写入文件
4. `exit $_X` 恢复测试命令的退出码作为最终 shell 退出码
5. test-runner 的 stdout 包含所有输出，解析器从中提取测试用例
6. 覆盖率从文件读取（`coverage.json` / `coverage/coverage-summary.json`），不依赖 stdout

### 占位符替换规则

| 占位符 | 替换值 | 示例 |
|--------|--------|------|
| `{files}` | 空格分隔的文件路径列表（相对 project_root）；空列表时替换为空字符串 | `src/a.test.ts src/b.test.ts` |
| `{directory}` | plan entry 的 directory 字段值（相对 project_root） | `packages/core` |
| `{project_root}` | 项目根目录绝对路径 | `/home/user/project` |

运行命令前，runner 会 `cd` 到 plan entry 的 directory 下。

### 覆盖率解析映射

| coverage_format | 源文件 | lines | branches | functions |
|----------------|--------|-------|----------|-----------|
| `istanbul` | `coverage-summary.json` | `total.lines.pct` | `total.branches.pct` | `total.functions.pct` |
| `llvm-cov` | JSON 文件 | `data[0].totals.lines.percent` | `data[0].totals.branches.percent` | `data[0].totals.functions.percent` |
| `node-test` | `coverage/node-test-output.txt` | regex 解析文本表格 `% Lines` 列 | regex 解析 `% Branch` 列 | regex 解析 `% Funcs` 列 |
| `go-cover` | `func-summary.txt` | regex 匹配 `total:` 行百分比 | `null` | `null` |
| `coverage-py` | `coverage.json` | `totals.percent_covered` | `totals.percent_covered_branches` | `null` |

---

## test-runner 简化（无 merge_mode 分支）

移除 `merge_mode` 后，`executePlanEntry` 不再需要 `executeMerged()` 和 `executeTwoPhase()` 两个分支函数。执行逻辑简化为单一流程：

```
function executePlanEntry(entry, projectRoot, options):
  1. 替换 test_cmd 中的占位符
  2. cd 到 entry.directory
  3. 执行 test_cmd（单一 shell 命令，无 merge_mode 分支）
  4. 调用 parseTestOutput 解析 stdout/stderr
  5. 查找覆盖率文件，调用 parseCoverageFromFile
  6. 返回 ExecutionResult
```

对于使用链式命令的框架（pytest、rust），步骤 3 执行的 shell 命令内部已包含测试+覆盖率两步。步骤 5 从文件读取覆盖率。对于单命令框架（jest、vitest 等），命令本身已同时产出测试输出和覆盖率文件。两者共享同一执行路径，无分支。

---

## 依赖

### 运行时依赖

- **cac** (^6.7.14) — CLI 框架，已存在于项目中。用于注册 `unit-test` 子命令
- **zod** (^4.4.3) — schema 验证库，已存在于项目中。用于定义子/汇总报告 schema
- **picomatch** (^4.0.4) — glob 匹配，已存在于项目中。由 test-detect-frameworks 使用

无新增运行时依赖。

### 构建/测试依赖

- **typescript** (^6.0.3) — 类型检查（已引入）
- **@types/node** — Node.js 类型定义（已引入）
- **vite-plus** — 测试运行器（已引入）

无新增构建依赖。

---

## 待决问题

- 无。所有设计决策已在 proposal.md「决策」表和 spec.md 中确定。

---

## 附注：unit-test-executor agent 的调整

本次变更不直接修改 agent 定义文件（`plugins/dev-team/agents/unit-test-executor.md`），但 agent 的执行角色将从「Agent 直接执行测试命令和解析输出」转变为「Agent 读取 CLI 产出的汇总报告」。具体调整包括：

1. agent 不再执行 `runTestDetectFrameworks` MCP 工具调用 — 由 CLI 内部完成
2. agent 不再执行 shell 命令 — 由 CLI 的 test-runner 完成
3. agent 不再解析覆盖率文件 — 由 coverage-parser.ts 完成
4. agent 不再生成完整报告 — 由 test-report.ts 完成
5. agent 输入从「测试命令输出」变为「reports/unit-test-execution.json」
6. agent 保留：读取报告 → 验证完整性 → 诊断决策树 → 写入 eval.json

此调整在 agent 定义文件更新任务中处理（见 tasks.md 阶段 11）。
