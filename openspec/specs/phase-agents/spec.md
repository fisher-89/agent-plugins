# phase-agents Specification

## Purpose
Phase agent file content, phase IDs, and report paths. Backtrack routing lives in `pipeline-backtrack` (decision C3=A: evaluators do not set `backtrack_to`; skills call `backtrack`). 本变更要求 evaluator 经 `phase_log` 把结果写入 `workflow.json.eval`，禁止把独立 `eval.json` 当作写入目标。

## Requirements

### Requirement: Evaluator / Executor 使用 consolidated phase ID

相关 agent SHALL 使用 `phase: "test-execution"`（经 `phase_log`）。已删除：`unit-test-*`、`integration-test-*` agent 文件。

| Agent | Phase |
|-------|-------|
| test-execution-evaluator.md | test-execution |
| test-execution-executor.md | test-execution（含 no-op skip 的 phase_log） |
| 其余 evaluator（proposal…acceptance、code-analyze） | 各自现行 ID，不变 |

#### Scenario: 旧 agent 不存在 / 新 agent 用 test-execution

- **WHEN** 检查 `integration-test-evaluator.md` / `unit-test-executor.md` 等
- **THEN** SHALL NOT 存在
- **WHEN** 读 `test-execution-evaluator.md` / `test-execution-executor.md`
- **THEN** `phase_log` 使用 `phase: "test-execution"`

### Requirement: test-execution agents 报告路径

`test-execution-executor.md` 与 `test-execution-evaluator.md` SHALL 读取聚合 `reports/test/summary.json`（有 change 时：`openspec/changes/<change-name>/reports/test/summary.json`）与经 `summary.plans[].path` 定位的 `<path>/report.json`。MUST NOT 要求 `reports/test-execution.json` 或 `reports/test-execution/<framework>.json` 作为权威路径。

**ID**: REQ-PA-TEF-1

#### Scenario: 路径更新

- **WHEN** 读上述两个 agent 文件
- **THEN** 引用 `reports/test/summary.json` + `plans[]`；不引用旧权威路径

### Requirement: Evaluator 不拥有 backtrack 路由

`test-execution-evaluator`（及其它 evaluator）SHALL 只做诊断并写入 report；MUST NOT 设置 `backtrack_to`。允许的回溯目标语义由 skill 层按 `pipeline-backtrack` 决定（常见目标：`test-gen`、`implement`、`test-design`、`dev-design`；不含 `integration-test`）。

#### Scenario: evaluator 不设 backtrack_to

- **WHEN** 读 `test-execution-evaluator.md` 及其它 evaluator 指令
- **THEN** SHALL NOT 要求设置 `backtrack_to`
- **AND** 回溯由 skill 按 `pipeline-backtrack` 决定

### Requirement: generator Process 结束前引用 static-analysis-gate include

源文件 `plugins/dev-team/agents/implementation-generator.md` 与 `plugins/dev-team/agents/test-gen-generator.md` SHALL 在各自 `## Process` 结束前（全部既有步骤之后）包含字面量：

```
__INCLUDE:static-analysis-gate__
```

该引用 SHALL 作为构建期片段注入点，而非在源文件中手写平台分支正文。Claude 组装后该处展开为空；Cursor / cursorHome 组装后展开为 `run_static_analysis` 软门禁（见 `include-fragments`）。

两 generator MUST NOT 在源正文中再次嵌入与 Claude `SubagentStop` hook 重复的硬编码静态检查步骤清单（避免 Claude 双重要求）；门禁差异仅通过 include 的平台档实现。

#### Scenario: implementation-generator 源含 include

**WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md`
**THEN** `## Process` 区域内 SHALL 包含 `__INCLUDE:static-analysis-gate__`
**AND** 该标记位于既有 Process 步骤之后

#### Scenario: test-gen-generator 源含 include

**WHEN** 读取 `plugins/dev-team/agents/test-gen-generator.md`
**THEN** `## Process` 区域内 SHALL 包含 `__INCLUDE:static-analysis-gate__`
**AND** 该标记位于既有 Process 步骤之后

#### Scenario: Claude 组装后 generator 无软门禁步骤

**WHEN** assemble 完成 `claude` 产物
**AND** 读取产物中对应 implementation-generator / test-gen-generator 文件
**THEN** 文件 SHALL NOT 因该 include 含有 `run_static_analysis` 结束前门禁段落

#### Scenario: Cursor 组装后 generator 含软门禁步骤

**WHEN** assemble 完成 `cursor` 或 `cursorHome` 产物
**AND** 读取产物中对应 implementation-generator / test-gen-generator 文件（含 `namePrefix` 重命名后的文件名）
**THEN** 文件 SHALL 含结束前执行 `run_static_analysis` 并在未通过时不得结束的说明

### Requirement: Evaluator persists via phase_log into workflow.json

所有 evaluator / 会调用 `phase_log` 的 executor（含 `test-execution-evaluator`、`test-execution-executor` 的 skip 路径，以及 proposal…acceptance、code-analyze evaluator）SHALL：

- 使用 MCP `phase_log` 追加评估结果
- 将持久化目标描述为 change 的 `workflow.json`（字段 `eval`）
- MUST NOT 指示使用 Write/Edit/Bash 创建或修改 `eval.json` 或 `workflow.json`

标题或步骤名若仍写「Append to eval.json」，SHALL 改为指向 `phase_log` / `workflow.json`。

#### Scenario: test-execution-evaluator 写入说明

- **WHEN** 读取 `plugins/dev-team/agents/test-execution-evaluator.md` 的 phase_log 步骤
- **THEN** 该步骤要求调用 `phase_log` 且提及 `workflow.json`（或等价「工作流元数据中的评估记录」）
- **AND** SHALL NOT 要求直接写文件 `eval.json`

#### Scenario: evaluator 仍不设 backtrack_to

- **WHEN** evaluator 判定 fail 并调用 `phase_log`
- **THEN** 条目无 `backtrack_to`（不变，见 `pipeline-backtrack`）

## Module Contract

| Agent | Model | Phase | Input | Backtrack |
|-------|-------|-------|-------|-----------|
| test-execution-executor | sonnet-4.6 | test-execution | `reports/test/summary.json` + `plans[]` | n/a（执行器） |
| test-execution-evaluator | sonnet（系统定义） | test-execution | `reports/test/summary.json` | skill 决策；evaluator 不设 `backtrack_to` |

### Agent 源：`implementation-generator` / `test-gen-generator`

| 方面 | 描述 |
|------|------|
| **注入点** | `## Process` 末尾 `__INCLUDE:static-analysis-gate__` |
| **Claude 行为** | include → 空；硬门禁仍由 `SubagentStop` + `static-check` |
| **Cursor 行为** | include → 软门禁文案；无 `subagentStop` hook |
| **禁止** | 源文件手写双端重复静态检查长文；多 fragments 根 |

### Agent 持久化文案（evaluator / executor）

| 文件 | 变更 |
|------|------|
| `plugins/dev-team/agents/*-evaluator.md` | 持久化文案：`phase_log` → `workflow.json` |
| `plugins/dev-team/agents/test-execution-executor.md` | skip 时 `phase_log` 说明同步 |
