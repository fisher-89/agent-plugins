## Module Contract

### Module: unit-test-executor.md (Agent Specification)

| Aspect | Description |
|--------|-------------|
| Model | `sonnet-4.6` |
| Input | `test-design.md`, project `CLAUDE.md`, **`reports/unit-test-execution.json` (CLI 产出)** |
| Output | `openspec/changes/<change-name>/reports/unit-test-execution.json` |
| Key Change | Executor **no longer executes test commands or parses raw output**. These tasks are performed by the `dev-team unit-test` CLI command. The agent reads the CLI-generated summary report and adds/overrides the `findings` field with diagnostic information. |

### Coverage parsing per `coverage_format` (delegated to CLI)

Coverage parsing is now performed by `lib/test-parser/coverage-parser.ts` in the CLI layer. The executor agent no longer parses raw coverage output. The format-to-source mapping is preserved for reference:

| Format | Source file | Extraction method |
|--------|------------|-------------------|
| `istanbul` | `coverage/coverage-summary.json` | `total.lines.pct` → `lines`, `total.branches.pct` → `branches`, `total.functions.pct` → `functions` |
| `llvm-cov` | JSON output file | `data[0].totals.lines.percent` → `lines`, `data[0].totals.branches.percent` → `branches`, `data[0].totals.functions.percent` → `functions` |
| `node-test` | `coverage/node-test-output.txt` → `coverage-summary.json` | Read istanbul-compatible JSON from parsed output |
| `go-cover` | `coverage/func-summary.txt` | Regex: match `total:` line, extract percentage |
| `coverage-py` | `coverage.json` | `totals.percent_covered` → `lines`, `totals.percent_covered_branches` → `branches` |

---

## REMOVED Requirements

### Requirement: Executor (sonnet) 直接执行测试命令

**ID**: REQ-UTE-0 (previously part of MODIFIED section in prior spec)
**Priority**: MUST
**Description**: The executor agent SHALL no longer execute test commands or parse raw test output. These deterministic tasks are performed by the `dev-team unit-test` CLI command. The executor agent SHALL read the CLI-generated `reports/unit-test-execution.json` summary report and focus on high-level diagnostics.

**Reason**: All deterministic execution (running shell commands, parsing JSON/text output, extracting coverage, generating structured reports) has been moved to TypeScript CLI code for reliability, speed, and cost efficiency. The Agent's role is reduced to reading the CLI report and adding diagnostic `findings`.

**Migration**:
1. The workflow SHALL invoke `dev-team unit-test` before the unit-test-executor agent runs
2. The executor agent SHALL receive `reports/unit-test-execution.json` as input (instead of raw command output)
3. The executor agent SHALL read the summary report and populate/override the `findings` field with diagnostic insights
4. The executor agent SHALL NOT run `test_detect_frameworks` MCP tool for framework detection — this is done by the CLI internally
5. The executor agent SHALL NOT parse coverage output files — this is done by `coverage-parser.ts`

#### Scenario: Agent reads CLI report instead of executing commands

**WHEN** the unit-test-executor agent is invoked in phase 06-unit-test
**THEN** `reports/unit-test-execution.json` SHALL already exist (created by `dev-team unit-test`)
**AND** the agent SHALL read this file as its primary input
**AND** the agent SHALL NOT execute `npm test`, `npx vitest`, or any test command
**AND** the agent SHALL NOT call `test_detect_frameworks`

#### Scenario: Agent adds findings to existing report

**WHEN** the agent identifies a pattern in the summary report that needs diagnostic attention
**THEN** the agent SHALL write diagnostic text to the `findings` field
**AND** SHALL NOT modify other report fields (`conclusion`, `coverage`, `test_cases`, etc.)
**AND** SHALL NOT regenerate the entire report

### Requirement: Executor 调用 test_detect_frameworks MCP 工具

**ID**: (formerly part of MODIFIED section in prior spec)
**Priority**: MUST
**Description**: The executor agent SHALL NOT call `test_detect_frameworks` MCP tool. Framework detection and plan construction are handled internally by the `dev-team unit-test` CLI.

**Reason**: Framework detection is a deterministic look-up from `openspec/config.json` and `FRAMEWORK_REGISTRY`. The CLI performs this more efficiently without LLM overhead.

**Migration**: The CLI `dev-team unit-test` calls `runTestDetectFrameworks({})` internally. Framework plan data is available in the summary report's `coverage.by_framework` field.

#### Scenario: Agent does not call test_detect_frameworks

**WHEN** the unit-test-executor agent is invoked
**THEN** the agent's instructions SHALL NOT reference `test_detect_frameworks`
**AND** SHALL NOT include steps for calling MCP tools to detect frameworks

### Requirement: Executor 移动覆盖率产物到统一位置

**ID**: (formerly part of MODIFIED section in prior spec)
**Priority**: MUST
**Description**: Product movement and cleanup of coverage artifacts is now performed by the CLI. The executor agent SHALL NOT perform file operations (mkdir, mv, rm) for coverage artifacts.

**Reason**: File system operations are deterministic and better handled by TypeScript code with proper error handling.

**Migration**: The CLI handles `coverage_artifacts` movement to `reports/coverage/<framework>/` and subsequent `coverage_cleanup`. If any movement fails, the CLI records the error in the sub-report's `findings` field without blocking execution.

#### Scenario: Agent does not move coverage artifacts

**WHEN** the agent inspects coverage data
**THEN** it SHALL read from `reports/unit-test/<framework>.json` directly
**AND** SHALL NOT perform any `mv`, `mkdir`, or `rm` operations
**AND** SHALL NOT reference `coverage_artifacts` or `coverage_cleanup` fields

---

## MODIFIED Requirements

### Requirement: Agent 读取 CLI 汇总报告并添加诊断信息

**ID**: REQ-UTE-DIAG-1
**Priority**: MUST
**Description**: The unit-test-executor agent SHALL read the CLI-generated summary report `reports/unit-test-execution.json` and add diagnostic information to the `findings` field. The agent SHALL:
1. Read `conclusion` — if `"fail"`, analyze `problems[]` for patterns
2. Read `coverage` — if non-null and `pass: false`, identify which dimension(s) fall short
3. Read `coverage.by_framework` — if a framework has unexpectedly low coverage, suggest possible causes (missing tests, config issue)
4. Write diagnostic text to `findings` field (append or override)
5. Leave all other fields unchanged

#### Scenario: Agent 分析覆盖失败原因

**WHEN** `conclusion` 为 `"fail"` 且 `problems` 包含覆盖率不达标的条目
**THEN** 代理 SHALL 在 `findings` 中添加诊断消息，指出具体哪个维度不达标
**AND** SHALL 建议可能的根因（如：该框架缺少测试文件、覆盖率配置错误）

#### Scenario: Agent 分析测试失败原因

**WHEN** `failed > 0` 且 `problems` 包含失败用例
**THEN** 代理 SHALL 读取子报告 `reports/unit-test/<framework>.json` 中的失败用例详情
**AND** SHALL 在 `findings` 中总结失败模式（如：大量超时、特定模块失败）
**AND** SHALL NOT 修改 `test_cases` 或 `conclusion`

### Requirement: 覆盖率 nullable 维度门控规则（保留，仅数据源变化）

**ID**: (prior REQ-UTE-RULES — preserved unchanged)
**Priority**: MUST
**Description**: 当框架的 `coverage_format` 不支持某一覆盖率维度时，该维度的 `measured` 值 SHALL 为 `null`（而非 `0`）。此规则由 CLI 的 `coverage-parser.ts` 执行，agent 在诊断时 SHALL 遵循相同的 null 维度语义。

null 维度规则：
1. **阈值比较**：`coverage.pass` 计算时，null 维度跳过与 `coverage.thresholds` 的比较（视为自动通过）
2. **加权平均**：计算 `coverage.measured` 时，null 维度不参与加权平均；仅非 null 框架贡献权重
3. **全 null 维度**：若所有框架在某维度均为 null，则 `coverage.measured` 中该维度 SHALL 为 `null`
4. **overrides 分组**：overrides 校验中 null 维度同样跳过比较

各框架 null 维度映射：
- `go-cover`：`branches: null`，`functions: null`
- `coverage-py`：`functions: null`
- `node-test`、`istanbul`、`llvm-cov`：三维度均为 number（无 null）

#### Scenario: Agent 诊断时识别 null 维度

**WHEN** agent 读取到 `by_framework` 中 go 框架的 `measured: {lines: 85, branches: null, functions: null}`
**THEN** agent SHALL 识别 branches 和 functions 为 null（非零）
**AND** SHALL NOT 将 null 诊断为覆盖率丢失
**AND** SHALL NOT 在 `findings` 中报告 "branches 未达标"

---

## ADDED Requirements

### Requirement: 确保 CLI 生成汇总报告后再调用 agent

**ID**: REQ-UTE-ORDER-1
**Priority**: MUST
**Description**: The PGE workflow SHALL ensure that `dev-team unit-test` CLI command is executed BEFORE the unit-test-executor agent is invoked in phase 06-unit-test. The agent SHALL assume `reports/unit-test-execution.json` exists at invocation time.

#### Scenario: 工作流先执行 CLI 再调用 agent

**WHEN** phase 06-unit-test 开始
**THEN** SHALL 先执行 `dev-team unit-test` CLI 命令
**AND** SHALL 等待 CLI 完成后才调用 unit-test-executor agent

#### Scenario: 汇总报告不存在时 agent 报错

**WHEN** unit-test-executor agent 被调用但 `reports/unit-test-execution.json` 不存在
**THEN** agent SHALL 输出错误信息指示缺少汇总报告
**AND** SHALL 建议先运行 `dev-team unit-test`
