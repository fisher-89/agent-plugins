## ADDED Requirements

### Requirement: test-execution agents reference reports/test paths

**ID**: REQ-PA-TEF-1
**Priority**: MUST
**Description**: `test-execution-executor.md` 与 `test-execution-evaluator.md` SHALL 将 CLI 报告输入路径更新为：
- 聚合报告：`reports/test/summary.json`（有 change 时：`openspec/changes/<change-name>/reports/test/summary.json`）
- 原子报告：经 `summary.plans[].path` 定位的 `<path>/report.json`

文案 MUST NOT 继续要求读取 `reports/test-execution.json` 或 `reports/test-execution/<framework>.json`。CLI / phase 标识符仍为 `test-execution`。

#### Scenario: executor agent input path updated

**WHEN** reading `plugins/dev-team/agents/test-execution-executor.md`
**THEN** 其输入/读取步骤 SHALL 引用 `reports/test/summary.json`
**AND** 失败明细步骤 SHALL 通过 `plans[]` 打开各 `report.json`
**AND** SHALL NOT 引用 `reports/test-execution.json` 作为权威路径

#### Scenario: evaluator agent input path updated

**WHEN** reading `plugins/dev-team/agents/test-execution-evaluator.md`
**THEN** 其结构化报告输入 SHALL 为 `reports/test/summary.json`（含 change 前缀形态）
**AND** SHALL NOT 引用 `reports/test-execution.json`

## MODIFIED Requirements

### Requirement: Evaluator agents call phase_log with updated phase ID

（本变更不修改 phase ID；仅要求相关 agent 文件中的报告路径与 Module Contract 与 `reports/test/` 对齐。phase_log 仍使用 `phase: "test-execution"`。）

#### Scenario: test-execution-evaluator calls phase_log with updated ID

**WHEN** reading `test-execution-evaluator.md`
**THEN** the `phase_log` call uses `phase: "test-execution"`
**AND** 报告读取路径使用 `reports/test/summary.json`

#### Scenario: test-execution-executor uses updated ID for skip

**WHEN** reading `test-execution-executor.md`
**THEN** the `phase_log` call for no-op skip uses `phase: "test-execution"`
**AND** CLI 报告路径文案使用 `reports/test/summary.json`

## Module Contract

### Agent: test-execution-executor.md

| Aspect | Value |
|--------|-------|
| **Model** | `sonnet-4.6` |
| **Phase** | `test-execution` |
| **Input** | `reports/test/summary.json`（CLI-generated；经 `plans[]` 读各 `report.json`） |
| **Scope** | All automated tests (unit + integration combined) |

### Agent: test-execution-evaluator.md

| Aspect | Value |
|--------|-------|
| **Model** | sonnet (as defined in system definition) |
| **Phase** | `test-execution` |
| **Input** | `reports/test/summary.json` |
| **Backtrack targets** | 由 skill 层决定（evaluator 不设置 `backtrack_to`） |
