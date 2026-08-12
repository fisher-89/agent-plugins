## 权威边界

Agent 文件内容、phase ID、报告路径。回溯路由见 `pipeline-backtrack`（决议 C3=A：evaluator 不设 `backtrack_to`，由 skill 调 `backtrack`）。

## MODIFIED Requirements

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

**ID**: REQ-PA-TEF-1  

`test-execution-executor.md` 与 `test-execution-evaluator.md` SHALL 读取：

- 聚合：`reports/test/summary.json`（有 change 时：`openspec/changes/<change-name>/reports/test/summary.json`）
- 原子：经 `summary.plans[].path` 定位的 `<path>/report.json`

MUST NOT 要求 `reports/test-execution.json` 或 `reports/test-execution/<framework>.json` 作为权威路径。

#### Scenario: 路径更新

- **WHEN** 读上述两个 agent 文件
- **THEN** 引用 `reports/test/summary.json` + `plans[]`；不引用旧权威路径

### Requirement: Evaluator 不拥有 backtrack 路由

`test-execution-evaluator`（及其它 evaluator）SHALL 只做诊断并写入 report；MUST NOT 设置 `backtrack_to`。允许的回溯目标语义由 skill 层按 `pipeline-backtrack` 决定（常见目标：`test-gen`、`implement`、`test-design`、`dev-design`；不含 `integration-test`）。

## Module Contract

| Agent | Model | Phase | Input | Backtrack |
|-------|-------|-------|-------|-----------|
| test-execution-executor | sonnet-4.6 | test-execution | `reports/test/summary.json` + `plans[]` | n/a（执行器） |
| test-execution-evaluator | sonnet（系统定义） | test-execution | `reports/test/summary.json` | skill 决策；evaluator 不设 `backtrack_to` |
