## MODIFIED Requirements

### Requirement: Agent (test-execution-executor) reads CLI summary report and adds diagnostics

**ID**: REQ-TEE-DIAG-1
**Priority**: MUST
**Description**: The test-execution-executor agent SHALL read the CLI-generated summary report `reports/test/summary.json`（有 `--change` 时为 `openspec/changes/<change>/reports/test/summary.json`）and add diagnostic information to the `findings` field. The agent SHALL:
1. Read `conclusion` — if `"fail"`, analyze `problems[]` for patterns
2. Read `coverage` — if non-null and `pass: false`, identify which dimension(s) fall short
3. Read `coverage.by_framework` — if a framework has unexpectedly low coverage, suggest possible causes (missing tests, config issue)
4. Write diagnostic text to `findings` field (append or override)
5. Leave all other fields unchanged

定位各 plan 原子报告时，agent SHALL 优先使用 `summary.plans[]` 的 `path` 打开 `<path>/report.json`。MUST NOT 再使用 `reports/test-execution.json`、`reports/test-execution/<framework>.json` 或扁平 `<framework>.json` 路径约定。

**Changes from previous version**:
- Report file path: `reports/test-execution.json` → `reports/test/summary.json`
- Sub-report path: `reports/test-execution/<framework>.json` → `reports/test/<planId>/report.json`（经 `plans[]`）
- Phase: `test-execution`（不变）

#### Scenario: Agent analyzes coverage failure reasons

**WHEN** `conclusion` 为 `"fail"` 且 `problems` 包含覆盖率不达标的条目
**THEN** 代理 SHALL 在 `findings` 中添加诊断消息，指出具体哪个维度不达标
**AND** SHALL 建议可能的根因（如：该框架缺少测试文件、覆盖率配置错误）

#### Scenario: Agent analyzes test failure reasons via plans index

**WHEN** `failed > 0` 且 `problems` 包含失败用例
**THEN** 代理 SHALL 读取 `summary.plans[]` 中对应条目的 `path`，打开 `path/report.json` 获取失败用例详情
**AND** SHALL 在 `findings` 中总结失败模式（如：大量超时、特定模块失败）
**AND** SHALL NOT 修改 `test_cases` 或 `conclusion`
**AND** SHALL NOT 假设子报告位于 `reports/test-execution/<framework>.json`

### Requirement: Agent runs all tests via CLI (unit + integration combined)

**ID**: REQ-TEE-ALL-1
**Priority**: MUST
**Description**: The `dev-team test-execution` CLI command SHALL run ALL test files discovered by the framework plan, without distinguishing between unit and integration tests（`--change` 时受 change 清单 plan gate 约束：suite root 与清单 written∪deleted 无交集的 plan entry 被跳过，见 `cli-unit-test-execute` 的 REQ-TEC-GATE-1）. For each framework in the plan (`test_detect_frameworks` result), the CLI SHALL:
1. Run the framework's `script` which executes all matching test files
2. Parse test results and coverage from the plan directory file-channel artifacts
3. Generate a single summary report at `reports/test/summary.json`
4. Generate per-plan atomic reports at `reports/test/<planId>/report.json`

Frameworks that naturally distinguish test types (e.g., Go's `_test.go` for unit, custom integration test files) SHALL NOT be split into separate executions. All test files matching the framework's glob patterns SHALL be executed together.

#### Scenario: vitest runs all test files including __tests__/

**WHEN** the project uses vitest
**THEN** `dev-team test-execution` SHALL run vitest on all files matching `**/*.{test,spec}.{js,ts,jsx,tsx}`
**AND** files in `__tests__/` directories are included (they match vitest's default glob)
**AND** a single atomic report is generated at `reports/test/vitest/report.json`（当 directory 为 `.`）

#### Scenario: go test runs all ./... packages

**WHEN** the project uses Go
**THEN** `dev-team test-execution` SHALL run `go test`（含文件通道产物）across all packages
**AND** unit tests (`*_test.go`) and integration tests (with `//go:build integration` tags) are both executed
**AND** a single atomic report is generated at `reports/test/go/report.json`（当 directory 为 `.`）

#### Scenario: pytest runs all tests/ files

**WHEN** the project uses pytest
**THEN** `dev-team test-execution` SHALL run `pytest` across all test files matching `**/test_*.py`
**AND** unit and integration test files are both executed together
**AND** a single atomic report is generated at `reports/test/pytest/report.json`（当 directory 为 `.`）

### Requirement: Report file paths updated from unit-test to test-execution

**ID**: REQ-TEE-PATH-1
**Priority**: MUST
**Description**: All CLI output paths SHALL use the `reports/test/` namespace（本变更再次缩短路径；CLI/phase 名仍为 `test-execution`）：

| Old Path | New Path |
|----------|----------|
| `reports/test-execution.json` | `reports/test/summary.json` |
| `reports/test-execution/<planId>.json` | `reports/test/<planId>/report.json` |
| `reports/unit-test-execution.json`（更早） | `reports/test/summary.json` |

The summary report `phase` field SHALL remain `"test-execution"`. Summary SHALL include `plans[]` path index.

Agent 与相关 specs 文案 MUST 使用 `reports/test/`；MUST NOT 继续写 `reports/test-execution.json` 或 `<framework>.json` 扁平子报告路径。

#### Scenario: CLI writes report to new paths

**WHEN** `dev-team test-execution` completes successfully
**THEN** summary report exists at `reports/test/summary.json`
**AND** atomic reports exist at `reports/test/<planId>/report.json`
**AND** summary contains `plans[]`

#### Scenario: Report phase field unchanged

**WHEN** reading `reports/test/summary.json` phase field
**THEN** it equals `"test-execution"`

#### Scenario: agent docs forbid legacy paths

**WHEN** reading `test-execution-executor.md`
**THEN** 文案 SHALL 引用 `reports/test/summary.json` 与经 `plans[]` 定位的 `report.json`
**AND** SHALL NOT 指示读取 `reports/test-execution.json` 或 `reports/test-execution/<framework>.json`

## ADDED Requirements

### Requirement: Executor agent no longer has integration-test counterpart

**ID**: REQ-TEE-NOINT-1
**Priority**: MUST
**Description**: The test-execution-executor agent SHALL be the single executor for all automated test execution. There SHALL be no separate `integration-test-executor` agent. The executor's instructions SHALL NOT reference "unit test" or "integration test" as distinct categories — instead, it SHALL reference "all tests" or "automated tests".

#### Scenario: Executor instructions reference all tests

**WHEN** reading `test-execution-executor.md`
**THEN** the Process section SHALL reference "all automated tests" or "all test files"
**AND** SHALL NOT reference "unit tests" and "integration tests" as separate execution stages

### Requirement: 突变测试范围改读 change 文件清单

`plugins/dev-team/bin/src/commands/test-execution.ts` 的突变 scope 解析 SHALL 从读取 git 工作区 diff 改为读取目标 change 的 `workflow.json` 的 `files.written`，且 SHALL 由既有 CLI 选项 `--change=<change-name>` 直接触发：

- `--change=<change-name>` SHALL 兼作清单突变 scope 的唯一入口（该参数现仅用于定位 reports 目录，本变更后兼作突变 scope 来源）：传入该参数时，突变 scope SHALL 自动取自 `openspec/changes/<change>/workflow.json` 的 `files.written`（叠加净归零去噪过滤，见下条要求），调用方 MUST NOT 被要求另行传入任何 scope 选项
- CLI SHALL NOT 引入 `--mutation-scope` 或任何新的清单语义选项；既有 `--mutation-diff-only` 选项 SHALL 整体删除（含其 git diff 数据路径与 `--mutation-diff-only:` stdout 诊断前缀），突变 scope 解析 MUST NOT 再依赖 git
- `getGitDiffFiles`（`lib/git.ts`）SHALL 停止被引用；测试文件 diff 反推同位源文件的 `expandMutationDiffWithInferredSources` 行为 SHALL 保留（对 `written` 中的测试文件同样反推）
- 目标 change 的 `workflow.json` 缺失 `files` 字段时 SHALL 硬报错（"该 change 创建于文件清单机制之前，请重建"），MUST NOT 回退 git diff
- `--skip-mutation` SHALL 保留：显式跳过突变阶段的逃生口不变；突变 scope 解析 SHALL 惰性执行（仅在突变阶段实际运行时读取清单），`--skip-mutation` 时 MUST NOT 因清单缺失或非法而失败。plan gate（REQ-TEC-GATE-1）的清单读取独立于突变阶段且 fail-open，不改变本条逃生口语义

#### Scenario: 传 --change 自动读清单

- **WHEN** 执行 `dev-team test-execution --change=my-change`，该 change 的 `files.written` 为 `["src/a.ts", "src/a.test.ts"]`
- **THEN** 突变 scope 含 `src/a.ts` 及由 `src/a.test.ts` 反推的同位源文件
- **AND** 调用方未传任何 scope 类选项
- **AND** 进程不再调用 `git add -N` / `git reset` / `git diff HEAD`

#### Scenario: scope 选项不存在且 --skip-mutation 保留

- **WHEN** 检查 `dev-team test-execution` 的 CLI 选项定义
- **THEN** 不存在 `--mutation-scope` 与 `--mutation-diff-only`
- **AND** `--skip-mutation` 仍存在且语义不变（跳过突变阶段）

#### Scenario: skip-mutation 时不受清单缺失影响

- **WHEN** 以 `--change=<机制前旧 change> --skip-mutation` 执行，该 change 的 `workflow.json` 无 `files` 字段
- **THEN** 突变阶段被跳过，命令不因清单缺失而失败

#### Scenario: 旧 change 硬报错

- **WHEN** 以 `--change=<机制前旧 change>` 执行且未跳过突变，其 `workflow.json` 无 `files` 字段
- **THEN** 命令以错误退出，文案含重建指引
- **AND** MUST NOT 回退 git diff

### Requirement: 突变范围叠加净归零去噪过滤

清单模式 SHALL 在 `files.written` 基础上叠加薄过滤：内容与 `HEAD` 版本相同的文件（write 后又 revert 回原内容的净归零文件）SHALL 被排除出突变 scope——这类文件的突变体纯属噪声。过滤 SHALL 只作优化、不作范围权威：被过滤文件的实现正确性仍由 evaluator 内容核对承担。

#### Scenario: 净归零文件被过滤

- **WHEN** `files.written` 含 `src/b.ts` 且其工作区内容与 `HEAD` 版本一致
- **THEN** `src/b.ts` 不进入突变 scope

#### Scenario: 内容有差异的文件保留

- **WHEN** `files.written` 含 `src/a.ts` 且内容与 `HEAD` 不同
- **THEN** `src/a.ts` 进入突变 scope

## REMOVED Requirements

### Requirement: Executor (sonnet) 直接执行测试命令

**Reason**: See original spec. The executor agent SHALL NOT execute test commands directly — this is handled by the `dev-team test-execution` CLI.

### Requirement: Executor 调用 test_detect_frameworks MCP 工具

**Reason**: Framework detection is handled by the CLI internally. No change from original spec — same behavior, updated path references.

**Migration**: CLI command renamed from `dev-team unit-test` to `dev-team test-execution`.

### Requirement: Executor 移动覆盖率产物到统一位置

**Reason**: File system operations remain in CLI. Report path updated from `reports/unit-test/` to `reports/test-execution/`.

**Migration**: All references to `reports/unit-test/` updated to `reports/test-execution/`.

## Module Contract

### Module: test-execution-executor.md (Agent Specification)

| Aspect | Description |
|--------|-------------|
| Model | `sonnet-4.6` |
| Input | `test-design.md`, project `CLAUDE.md`, **`reports/test/summary.json` (CLI output)** |
| Plan details | `summary.plans[].path` → `<path>/report.json` |
| Output | 更新同一 `summary.json` 的 `findings`（路径同上，含 change 前缀时） |
| Key Change | 报告根改为 `reports/test/`；用 `plans[]` 定位原子报告 |

### Module: commands/test-execution.ts (CLI Action Handler)

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/commands/test-execution.ts` |
| Exports | `runTestExecution(options: TestExecutionOptions): Promise<TestExecutionExitCode>` |
| Input | `TestExecutionOptions`: `{ projectRoot?: string }` |
| Output | `TestExecutionExitCode`: `0` (all pass or threshold met) / `1` (failures or threshold not met) |
| Side Effects | 读写 `reports/test/summary.json` 与 `reports/test/<planId>/report.json` |
| Behavior | 1) Call `runTestDetectFrameworks({})` for plan; 2) Execute each plan entry; 3) Parse planDir file-channel artifacts into atomic reports; 4) Aggregate summary report with `plans[]`; 5) Set exit code by conclusion |

### SummaryReport Schema (updated)

```typescript
interface TestExecutionSummaryReport {
  phase: "test-execution";          // UPDATED from "06-unit-test" / "unit-test"
  command: string;
  timestamp: string;
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
  } | null;
}
```
