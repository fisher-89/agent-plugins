## Module Contract

### Module: commands/unit-test.ts (CLI Action Handler)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/commands/unit-test.ts` |
| **Exports** | `runUnitTest(options: UnitTestOptions): Promise<UnitTestExitCode>` |
| **Input** | `UnitTestOptions`: `{ projectRoot?: string }` — 解析自 CLI 选项 |
| **Output** | `UnitTestExitCode`: `0` (全部通过或阈值达标) / `1` (存在失败或阈值不达标) |
| **Side Effects** | 执行 shell 命令；读写 `reports/unit-test/<fw>.json` 和 `reports/unit-test-execution.json`；process.exit |
| **Behavior** | 1) 调用 `runTestDetectFrameworks({})` 获取 plan；2) 对 plan 中每个 framework 调用 `executePlanEntry`；3) 解析输出生成子报告；4) 聚合生成汇总报告；5) 根据结论设置退出码 |

### Module: lib/test-runner.ts (Execution Orchestration)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-runner.ts` |
| **Exports** | `executePlanEntry(entry: PlanEntry, config: TestConfig): ExecutionResult` |
| **Input** | `PlanEntry` (含 framework, directory, test_cmd)，`TestConfig` (含 thresholds) |
| **Output** | `ExecutionResult`: `{ framework, exitCode, stdout, stderr, testCases, coverage, durationMs, error? }` |
| **Side Effects** | 执行 shell 命令（通过 `execCommand`） |
| **Execution** | 统一执行 `test_cmd`（单一 shell 命令）。对于链式命令框架（pytest/rust），test_cmd 内部通过 `; _X=$?; ...; exit $_X` 模式内嵌了测试+覆盖率两步。覆盖率解析从文件读取 |

### Module: lib/test-parser/index.ts (Parser Dispatch)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/index.ts` |
| **Exports** | `parseTestOutput(output: { stdout, stderr, exitCode }, format: string): ParsedTestResult` |
| **Input** | 命令 stdout/stderr 字符串、退出码、输出格式标识 |
| **Output** | `ParsedTestResult`: `{ total, passed, failed, skipped, testCases[], durationMs? }` |
| **Dispatch** | `"json"` → `json-parser.ts`；`"go-json"` → `go-parser.ts`；default → `text-parser.ts` |
| **Side Effects** | None — 纯函数 |

### Module: lib/test-parser/json-parser.ts (JSON Output Parser)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/json-parser.ts` |
| **Exports** | `parseJsonOutput(stdout: string): ParsedTestResult` |
| **Input** | vitest/jest JSON reporter stdout 字符串 |
| **Output** | `ParsedTestResult` |
| **Supported formats** | vitest JSON reporter（`testResults` 数组）、jest JSON reporter（`testResults` 数组） |
| **Side Effects** | None |

### Module: lib/test-parser/go-parser.ts (Go Line-Delimited JSON Parser)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts` |
| **Exports** | `parseGoOutput(stdout: string): ParsedTestResult` |
| **Input** | `go test -json` 的逐行 JSON stdout |
| **Output** | `ParsedTestResult` |
| **Parsing** | 逐行解析 JSON，过滤 `Action` 字段为 `"pass"`/`"fail"`/`"skip"` 的行，聚合 `Test` 字段 |
| **Side Effects** | None |

### Module: lib/test-parser/text-parser.ts (Text Output Parser)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts` |
| **Exports** | `parseTextOutput(stdout: string): ParsedTestResult` |
| **Input** | 自由文本 stdout（cargo test, bun test, node --test 等） |
| **Output** | `ParsedTestResult` |
| **Parsing** | 多层 fallback: 精确正则匹配（bun 的 `[PASS]`/`[FAIL]`、cargo 的 `test result: ok`、node-test 的 `# pass`） → 逐行 heuristic |
| **Side Effects** | None |

### Module: lib/test-parser/coverage-parser.ts (Coverage Parser)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.ts` |
| **Exports** | `parseCoverageOutput(filePath: string, format: string): CoverageMeasured` |
| **Input** | 覆盖率输出文件路径、覆盖率格式 |
| **Output** | `CoverageMeasured`: `{ lines: number, branches: number \| null, functions: number \| null }` |
| **Formats** | `istanbul`（读取 `coverage-summary.json` 的 `total.*.pct`）、`llvm-cov`（读取 JSON 的 `data[0].totals.*.percent`）、`node-test`（读取 `coverage-summary.json` 的 `total.*.pct`）、`go-cover`（从 `func-summary.txt` 的 `total:` 行 regex 提取 lines 百分比；branches/functions 为 null）、`coverage-py`（从 `coverage.json` 的 `totals.percent_covered` 提取 lines，`totals.percent_covered_branches` 提取 branches；functions 为 null） |
| **Side Effects** | 读取文件 |

### Module: lib/test-report.ts (Report Generation)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-report.ts` |
| **Exports** | `generateSubReport(framework: string, result: ExecutionResult, config: TestConfig): SubReport`；`generateSummaryReport(subReports: SubReport[], config: TestConfig): SummaryReport` |
| **Input** | 执行结果和框架配置 |
| **Output** | `SubReport`（写入 `reports/unit-test/<framework>.json`）、`SummaryReport`（写入 `reports/unit-test-execution.json`） |
| **Side Effects** | 创建目录、写 JSON 文件 |
| **Threshold** | `coverage.pass` 计算：每个非 null 维度 `measured >= thresholds`，all pass 则 true；null 维度跳过 |

### Schema: schemas/unit-test-output.schema.ts

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` |
| **Exports** | `unitTestSubReportSchema`、`unitTestSummaryReportSchema`、`UnitTestSubReport`、`UnitTestSummaryReport`（类型） |
| **Behavior** | Zod v4 schema 定义 |

---

### SubReport Schema (子报告)

```typescript
interface UnitTestSubReport {
  framework: string;
  timestamp: string;          // ISO 8601
  duration_ms: number;
  exit_code: number;
  summary: {
    total: number;
    passed: number;
    failed: number;
    skipped: number;
  };
  test_cases: Array<{
    name: string;
    file?: string;           // 源文件路径
    duration_ms?: number;
    status: "passed" | "failed" | "skipped";
    error_type?: string;     // status=failed 时
    error_message?: string;  // status=failed 时
    stack_trace?: string;    // status=failed 时
    line?: number;           // status=failed 时
  }>;
  test_files: string[];      // 涉及的所有测试文件路径
  source_files: string[];    // 对应的源文件路径
  file_coverage: Array<{
    file: string;
    lines: number;
    branches: number | null;
    functions: number | null;
  }> | null;                 // 框架支持文件级覆盖时非 null
  coverage: {
    measured: CoverageMeasured;
    thresholds: CoverageThresholds;
    pass: boolean;
  } | null;
  findings?: string[];       // 诊断信息
}
```

### SummaryReport Schema (汇总报告)

```typescript
interface UnitTestSummaryReport {
  phase: "06-unit-test";
  command: string;            // 实际执行的 CLI 命令
  timestamp: string;          // ISO 8601
  duration_seconds: number;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  conclusion: "pass" | "fail" | "error";
  problems: Array<{
    framework: string;
    type: "failure" | "coverage" | "timeout" | "error";
    message: string;
  }>;
  coverage: {
    pass: boolean;
    measured: CoverageMeasured;
    thresholds: CoverageThresholds;
    by_framework: Array<{
      framework: string;
      measured: CoverageMeasured;
    }>;
  } | null;                   // 所有框架覆盖率命令失败时 null
}
```

---

## ADDED Requirements

### Requirement: dev-team unit-test CLI 命令存在

**ID**: REQ-CUE-1
**Priority**: MUST
**Description**: `plugins/dev-team/bin/src/cli.ts` SHALL 注册名为 `unit-test` 的子命令，关联到 `commands/unit-test.ts` 的 action handler。命令 SHALL 支持 `--project-root <path>` 选项。当命令执行时，SHALL 调用 `runTestDetectFrameworks({})` 获取框架 plan。

#### Scenario: unit-test 子命令注册

**WHEN** `dev-team unit-test` 被调用
**THEN** CLI SHALL 解析该子命令并路由到 `runUnitTest` 函数
**AND** `runUnitTest` SHALL 调用 `runTestDetectFrameworks({})` 获取框架计划
**AND** SHALL 为 plan 中每个 framework 执行测试

#### Scenario: --project-root 选项支持

**WHEN** `dev-team unit-test --project-root /custom/path` 被调用
**THEN** SHALL 将 `/custom/path` 作为项目根目录传递给所有内部函数
**AND** 测试报告文件 SHALL 写入 `/custom/path/reports/`

### Requirement: test-runner 执行单一 test_cmd（无 merge_mode 分支）

**ID**: REQ-CUE-2
**Priority**: MUST
**Description**: `lib/test-runner.ts` 的 `executePlanEntry` SHALL 统一执行 `PlanEntry.test_cmd`（单一 shell 命令），无 merge_mode 分支。对于 pytest 和 rust，test_cmd 内部使用 shell 链式命令 `; _X=$?; ...; exit $_X` 模式包含测试+覆盖率两步。执行前 SHALL 将 `test_cmd` 中的占位符 `{files}`、`{directory}`、`{project_root}` 替换为实际值。

#### Scenario: 执行 vitest 单命令

**WHEN** `executePlanEntry` 处理 `{framework: "vitest", test_cmd: "npx vitest run --reporter=json --coverage --coverage.reporter=json-summary"}`
**THEN** SHALL 执行一次 shell 命令
**AND** 输出中 SHALL 包含 JSON 格式的测试用例和 `coverage/coverage-summary.json`

#### Scenario: 执行 pytest 链式命令

**WHEN** `executePlanEntry` 处理 `{framework: "pytest", test_cmd: "pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X"}`
**THEN** SHALL 执行一次 shell 命令（内部包含两步）
**AND** shell 退出码 SHALL 为测试命令的退出码（通过 `$_X` 恢复）
**AND** 覆盖率从 `coverage.json` 文件读取

#### Scenario: 执行 rust 链式命令

**WHEN** `executePlanEntry` 处理 `{framework: "rust", test_cmd: "cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X"}`
**THEN** SHALL 执行一次 shell 命令（内部包含两步）
**AND** shell 退出码 SHALL 为 `cargo test` 的退出码
**AND** 覆盖率从 `coverage/coverage-summary.json` 文件读取

#### Scenario: test_cmd 占位符替换

**WHEN** `test_cmd` 为 `"npx vitest run {files}"`
**AND** 当前 entry 的 `directory` 为 `"packages/core"`
**AND** `project_root` 为 `"/home/project"`
**THEN** 替换后的命令 SHALL 为 `"npx vitest run packages/core"`
**AND** `{project_root}` SHALL 被替换为 `"/home/project"`

### Requirement: json-parser 解析 vitest JSON 输出

**ID**: REQ-CUE-3
**Priority**: MUST
**Description**: `lib/test-parser/json-parser.ts` 的 `parseJsonOutput` SHALL 解析 vitest JSON reporter 输出的 JSON（含 `testResults` 数组）。每个 `testResults` 条目包含 `name`、`status`（`"passed"`|`"failed"`|`"skipped"`）、`duration`、`failureMessage` 等字段。

#### Scenario: vitest JSON 正常解析

**WHEN** 输入 JSON 包含两个通过用例和一个失败用例
**THEN** `total` SHALL 为 `3`
**AND** `passed` SHALL 为 `2`
**AND** `failed` SHALL 为 `1`
**AND** `testCases[0].status` SHALL 为 `"passed"`
**AND** `testCases[1].status` SHALL 为 `"failed"`
**AND** `testCases[2].status` SHALL 为 `"passed"`

#### Scenario: vitest JSON 含 skipped 用例

**WHEN** 输入 JSON 包含一个 skipped 用例
**THEN** `skipped` SHALL 为 `1`
**AND** 对应 `testCases[i].status` SHALL 为 `"skipped"`

#### Scenario: 无效 JSON 输入

**WHEN** 输入字符串不是合法 JSON
**THEN** 函数 SHALL 抛出 `ParseError` 或返回含 `error` 字段的结果
**AND** 不崩溃

### Requirement: go-parser 解析 go test -json 输出

**ID**: REQ-CUE-4
**Priority**: MUST
**Description**: `lib/test-parser/go-parser.ts` 的 `parseGoOutput` SHALL 逐行解析 `go test -json` 的输出。每行是一个 JSON 对象，包含 `Action`（`"pass"`|`"fail"`|`"skip"`）和 `Test`（测试名）字段。函数 SHALL 过滤掉 `Action: "output"` 和 `Action: "run"` 的行，只统计最终状态的用例。

#### Scenario: go test -json 正常解析

**WHEN** 输入包含 10 行 JSON，其中 `Action: "pass"` 的 8 行，`Action: "fail"` 的 1 行，`Action: "skip"` 的 1 行
**THEN** `total` SHALL 为 `10`
**AND** `passed` SHALL 为 `8`
**AND** `failed` SHALL 为 `1`
**AND** `skipped` SHALL 为 `1`

#### Scenario: 过滤非最终状态行

**WHEN** 输入包含 `Action: "output"`、`Action: "run"` 和 `Action: "pass"` 行
**THEN** `total` SHALL 只计算 `"pass"`、`"fail"`、`"skip"` 行
**AND** `"output"` 和 `"run"` 行 SHALL NOT 进入用例计数

### Requirement: text-parser 解析文本测试输出

**ID**: REQ-CUE-5
**Priority**: MUST
**Description**: `lib/test-parser/text-parser.ts` 的 `parseTextOutput` SHALL 使用多层 fallback 策略解析自由文本测试输出：
1. 精确正则匹配 — bun 的 `[PASS]`/`[FAIL]` tag、cargo 的 `test result: ok`/`test result: FAILED`、node-test 的 `# pass`/`# fail`/`# skip`
2. 通用 regex — 匹配 `tests: (\d+).*passed.*failed.*skipped` 等常见格式
3. 逐行 heuristic — 按行统计 PASS/FAIL 标识

#### Scenario: bun 文本输出解析

**WHEN** 输入为 bun test 的 stdout（含 `[PASS]` 和 `[FAIL]` tag 的行）
**THEN** text-parser SHALL 按 `[PASS]`/`[FAIL]`/`[SKIP]` 标记计算用例统计
**AND** 每个有标记的行作为一个 test_case

#### Scenario: cargo test 输出解析

**WHEN** 输入为 `cargo test` 的 stdout，最后一行包含 `test result: FAILED. 20 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out`
**THEN** `total` SHALL 为 `21`
**AND** `passed` SHALL 为 `20`
**AND** `failed` SHALL 为 `1`

#### Scenario: node-test 输出解析

**WHEN** 输入为 `node --test` 的 stdout，包含 `# pass 15`、`# fail 2`、`# skip 1` 行
**THEN** `total` SHALL 为 `18`
**AND** `passed` SHALL 为 `15`
**AND** `failed` SHALL 为 `2`
**AND** `skipped` SHALL 为 `1`

#### Scenario: 通用格式 fallback

**WHEN** 输入包含 `Tests: 10 passed, 3 failed, 13 total` 行
**THEN** text-parser SHALL 匹配通用 regex 提取 `total=13`, `passed=10`, `failed=3`

#### Scenario: 完全无法解析时返回 error

**WHEN** 输入为空字符串或不含任何可识别模式
**THEN** 函数 SHALL 返回 `{ total: 0, passed: 0, failed: 0, skipped: 0, testCases: [], error: "Unable to parse test output" }`
**AND** 不抛出异常

### Requirement: coverage-parser 解析所有覆盖率格式

**ID**: REQ-CUE-6
**Priority**: MUST
**Description**: `lib/test-parser/coverage-parser.ts` 的 `parseCoverageOutput` SHALL 根据 `format` 参数选择正确的解析策略：

| format | 源文件 | lines | branches | functions |
|--------|--------|-------|----------|-----------|
| `istanbul` | `coverage-summary.json` | `total.lines.pct` | `total.branches.pct` | `total.functions.pct` |
| `llvm-cov` | JSON 文件 | `data[0].totals.lines.percent` | `data[0].totals.branches.percent` | `data[0].totals.functions.percent` |
| `go-cover` | `func-summary.txt` | regex `total:` 行 | `null` | `null` |
| `coverage-py` | `coverage.json` | `totals.percent_covered` | `totals.percent_covered_branches` | `null` |

#### Scenario: istanbul 格式解析

**WHEN** `coverage-summary.json` 包含 `{"total":{"lines":{"pct":85.5},"branches":{"pct":72.3},"functions":{"pct":90.1}}}`
**AND** format 为 `"istanbul"`
**THEN** 返回 `{lines: 85.5, branches: 72.3, functions: 90.1}`

#### Scenario: go-cover 格式解析（null 维度）

**WHEN** `func-summary.txt` 包含 `total:(statements) 78.5%` 行
**AND** format 为 `"go-cover"`
**THEN** 返回 `{lines: 78.5, branches: null, functions: null}`

#### Scenario: coverage-py 格式解析

**WHEN** `coverage.json` 包含 `{"totals":{"percent_covered":82.0,"percent_covered_branches":65.0}}`
**AND** format 为 `"coverage-py"`
**THEN** 返回 `{lines: 82.0, branches: 65.0, functions: null}`

#### Scenario: 文件不存在时返回 error

**WHEN** 指定的覆盖率文件路径不存在
**THEN** 函数 SHALL 抛出错误或返回含 `error` 的结果
**AND** 调用方 SHALL 将该框架的 measured 三个维度均设为 0

### Requirement: 子报告生成与写入

**ID**: REQ-CUE-7
**Priority**: MUST
**Description**: `lib/test-report.ts` 的 `generateSubReport` SHALL 根据 `ExecutionResult` 生成子报告对象，写入 `reports/unit-test/<framework>.json`。子报告 SHALL 包含 `summary`、`test_cases`、`coverage`、`test_files`、`source_files`、`file_coverage`（可选）。

#### Scenario: vitest 子报告写入

**WHEN** 对 vitest 框架的 ExecutionResult 调用 `generateSubReport`
**THEN** SHALL 在 `reports/unit-test/vitest.json` 写入 JSON 文件
**AND** 文件内容 SHALL 包含 `summary` 对象（total/passed/failed/skipped）
**AND** 文件内容 SHALL 包含 `test_cases` 数组
**AND** 文件内容 SHALL 包含 `coverage` 对象（非 null 时含 measured/thresholds/pass）
**AND** 文件内容 SHALL 包含 `test_files` 和 `source_files` 数组

#### Scenario: 子报告含失败用例详情

**WHEN** ExecutionResult 包含 status 为 `"failed"` 的 test_cases
**THEN** 每个失败用例 SHALL 包含 `error_type`、`error_message`、`stack_trace` 字段（若有）
**AND** SHALL 包含 `line` 字段（若有）

### Requirement: 汇总报告生成与写入

**ID**: REQ-CUE-8
**Priority**: MUST
**Description**: `lib/test-report.ts` 的 `generateSummaryReport` SHALL 聚合所有子报告生成汇总报告，写入 `reports/unit-test-execution.json`。汇总报告 SHALL NOT 包含 `test_cases` 明细，SHALL 包含 `conclusion`、`problems`、`coverage`（含 pass/measured/thresholds/by_framework）。

#### Scenario: 汇总报告写入

**WHEN** 聚合两个框架（vitest、pytest）的子报告
**THEN** 汇总报告 SHALL 写入 `reports/unit-test-execution.json`
**AND** `total` SHALL 为两个框架用例数之和
**AND** `passed` SHALL 为两个框架通过数之和
**AND** `failed` SHALL 为两个框架失败数之和
**AND** `coverage.by_framework` SHALL 包含两个框架各自的 measured
**AND** `coverage.pass` SHALL 根据阈值正确计算
**AND** 文件 SHALL NOT 包含 `test_cases` 字段

#### Scenario: 汇总报告不含用例明细

**WHEN** 汇总报告被读取
**THEN** JSON 中 SHALL NOT 存在 `test_cases` 键
**AND** SHALL NOT 存在 `test_files` 键
**AND** SHALL NOT 存在 `source_files` 键
**AND** SHALL NOT 存在 `file_coverage` 键

#### Scenario: 全失败时 conclusion 为 fail

**WHEN** 所有测试用例均失败或退出码非零
**THEN** `conclusion` SHALL 为 `"fail"`
**AND** `problems` 数组 SHALL 包含至少一个条目描述失败原因

#### Scenario: 全覆盖率命令失败时 coverage 为 null

**WHEN** 所有框架的覆盖率命令均失败（文件不存在或退出码非零）
**THEN** 汇总报告 `coverage` 字段 SHALL 为 `null`
**AND** SHALL NOT 为 `{pass: false, ...}` 等含有数据进行覆盖率的对象

### Requirement: 阈值判定与 coverage.pass

**ID**: REQ-CUE-9
**Priority**: MUST
**Description**: 汇总报告的 `coverage.pass` SHALL 基于 `coverage.measured` 与 `coverage.thresholds` 的比较结果，规则如下：
1. 每个非 null 维度：`measured >= thresholds` → 通过
2. null 维度：跳过比较（视为自动通过）
3. 所有维度均通过 → `pass = true`
4. 任一非 null 维度不达标 → `pass = false`
5. 若 `coverage` 为 null（所有框架覆盖率均失败），`pass` 不参与计算

#### Scenario: 所有维度达标

**WHEN** `measured: {lines: 85, branches: 72, functions: 90}`，`thresholds: {lines: 80, branches: 70, functions: 75}`
**THEN** `pass` SHALL 为 `true`

#### Scenario: 某非 null 维度不达标

**WHEN** `measured: {lines: 75, branches: 72, functions: 90}`，`thresholds: {lines: 80, branches: 70, functions: 75}`
**THEN** `pass` SHALL 为 `false`（lines 75 < 80）

#### Scenario: null 维度跳过比较

**WHEN** `measured: {lines: 85, branches: null, functions: null}`，`thresholds: {lines: 80, branches: 70, functions: 75}`
**THEN** `pass` SHALL 为 `true`（branches/functions 自动通过）

### Requirement: sub-report schema Zod 定义

**ID**: REQ-CUE-10
**Priority**: SHOULD
**Description**: `schemas/unit-test-output.schema.ts` SHALL 使用 Zod v4 定义子报告 schema `unitTestSubReportSchema`，包含 `summary`、`test_cases`、`test_files`、`source_files`、`coverage`、`file_coverage`、`findings` 等字段。解析失败时 SHALL 提供清晰的错误消息。

#### Scenario: 有效子报告通过 schema 验证

**WHEN** 一个包含所有必需字段的有效子报告对象通过 `unitTestSubReportSchema`
**THEN** 验证 SHALL 成功
**AND** 返回解码后的类型安全对象

#### Scenario: 无效子报告被 schema 拒绝

**WHEN** 一个缺少 `summary` 字段的对象通过 `unitTestSubReportSchema`
**THEN** 验证 SHALL 失败
**AND** 错误消息 SHALL 指明缺失的字段名

### Requirement: summary-report schema Zod 定义

**ID**: REQ-CUE-11
**Priority**: SHOULD
**Description**: `schemas/unit-test-output.schema.ts` SHALL 使用 Zod v4 定义汇总报告 schema `unitTestSummaryReportSchema`，包含 `phase`、`timestamp`、`conclusion`、`problems`、`coverage`、`total`、`passed`、`failed`、`skipped`、`duration_seconds`。验证非 `test_cases` 等明细字段的存在。

#### Scenario: 有效汇总报告通过 schema 验证

**WHEN** 一个包含所有必需字段的有效汇总报告对象通过 `unitTestSummaryReportSchema`
**THEN** 验证 SHALL 成功

#### Scenario: 汇总报告含 test_cases 被拒绝

**WHEN** 一个包含 `test_cases` 字段的对象通过 `unitTestSummaryReportSchema`
**THEN** 验证 SHALL 失败（`test_cases` 不在 schema 中）
**AND** 错误消息 SHALL 提示未预期的字段

### Requirement: 退出码映射规则

**ID**: REQ-CUE-12
**Priority**: SHOULD
**Description**: `dev-team unit-test` 命令的退出码 SHALL 遵循以下规则：
- `0`：所有测试通过且覆盖率阈值达标（或覆盖率未配置）
- `1`：存在测试失败或覆盖率阈值不达标
- 非零退出码 SHALL NOT 隐藏 stderr 中的异常信息

#### Scenario: 全部通过时退出码 0

**WHEN** 所有框架的测试均通过且覆盖率达标
**THEN** 进程退出码 SHALL 为 `0`

#### Scenario: 存在失败时退出码 1

**WHEN** 任一框架的测试存在 failed > 0
**THEN** 进程退出码 SHALL 为 `1`

#### Scenario: 覆盖率不达标时退出码 1

**WHEN** 测试全部通过但覆盖率的 lines 维度未达阈值
**THEN** 进程退出码 SHALL 为 `1`

## MODIFIED Requirements

### Requirement: test-runner 无 merge_mode 分支（原 REQ-CUE-2 修改）

**ID**: REQ-CUE-2
**Priority**: MUST
**Description**: `lib/test-runner.ts` 的 `executePlanEntry` SHALL **不**包含 merge_mode 分支逻辑。所有框架统一执行 `PlanEntry.test_cmd` 单一 shell 命令。对于需要两步执行的框架（pytest、rust），`test_cmd` 内部使用 `; _X=$?; ...; exit $_X` 模式包含测试+覆盖率两步。

原设计使用 `merge_mode` 布尔字段决定 `executeMerged()` 或 `executeTwoPhase()` 分支。现统一为单一执行路径。

#### Scenario: 执行 pytest 链式命令

**WHEN** `executePlanEntry` 处理 pytest 框架
**THEN** SHALL 执行 `test_cmd` 一次（而非先 test_cmd 再 coverage_cmd 两次）
**AND** 链式命令 SHALL 按顺序执行：测试命令 → 保存退出码 → 覆盖率命令 → 恢复退出码
**AND** shell 最终退出码 SHALL 等于测试命令的退出码

#### Scenario: FrameworkConfig 无 merge_mode 字段

**WHEN** `getFrameworkConfig("vitest")` 被调用
**THEN** 返回的对象 SHALL NOT 包含 `merge_mode` 属性
**AND** 返回的对象 SHALL 包含 `test_cmd`、`coverage_cmd`、`coverage_format` 等字段

## REMOVED Requirements

### Requirement: merge_mode=true 一次执行（原 AC-11）

**ID**: (原 AC-11)
**Priority**: MUST
**Description**: 原设计：当 `merge_mode=true` 时，test-runner SHALL 执行 `test_cmd`（含覆盖率），一次产出用例和覆盖率。

**Reason**: merge_mode 字段已整体移除。所有框架统一使用单一 `test_cmd`，pytest 和 rust 使用链式命令模式。不再需要区分 merge_mode=true/false。

**Migration**: 使用单一 `executePlanEntry` 路径，无分支。pytest 和 rust 的 test_cmd 已更新为链式命令。

### Requirement: merge_mode=false 串行执行（原 AC-10）

**ID**: (原 AC-10)
**Priority**: MUST
**Description**: 原设计：当 `merge_mode=false` 时，test-runner SHALL 先执行 `test_cmd` 解析用例，再执行 `coverage_cmd` 解析覆盖率。

**Reason**: merge_mode 字段已整体移除。所有框架统一使用单一 `test_cmd`，pytest 和 rust 使用链式命令模式。

**Migration**: 链式命令 `test_cmd` 内部通过 `; _X=$?; ...; exit $_X` 模式按序执行测试和覆盖率命令。执行顺序和退出码保护由 shell 处理。

### Requirement: FrameworkConfig 含 merge_mode 字段（原 REQ-TF-5）

**ID**: (原 REQ-TF-5)
**Priority**: MUST
**Description**: 原设计：`FrameworkConfig` 接口 SHALL 包含 `merge_mode: boolean` 字段，各框架按分类设置。

**Reason**: merge_mode 字段已整体移除。框架间执行差异已通过链式命令消除。

**Migration**: `FrameworkConfig` 接口不再包含 `merge_mode`。pytest 和 rust 的 `test_cmd` 已更新为包含链式命令的字符串。
