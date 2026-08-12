## 权威边界

`test-only` 工作流的产品叙述与验收场景。phase 表长度、前置依赖的规范性数据以 `pge-workflow-engine` 为准；本文件不重复 bug-fix/requirement 表。

## MODIFIED Requirements

### Requirement: test-only 五阶段结构

`test-only` SHALL 为 5 个 phase（无 `acceptance`、无 `integration-test` / `unit-test`）：

| Identifier | Pattern | Description |
|------------|---------|-------------|
| proposal | DESIGN P→E | 测试向提案 |
| code-analyze | DESIGN P→E | 反构现有架构 |
| test-design | DESIGN P→E | 测试场景 |
| test-gen | EXEC G→E | 测试代码 |
| test-execution | EXEC Executor→E | 执行（单元+集成） |

完成条件：表内 5 个 phase 均有有效（非 stale）pass。

#### Scenario: phase 表内容

- **WHEN** `getPhaseTable("test-only")`
- **THEN** 恰为上述 5 个 ID；executor/evaluator 为 `test-execution-executor` / `test-execution-evaluator`

#### Scenario: 前置依赖

- **WHEN** `getPrerequisites("code-analyze"|"test-execution", "test-only")`
- **THEN** 分别为 `["proposal"]` / `["test-gen"]`（完整表见 pge）

## Module Contract

| Metric | Value |
|--------|-------|
| Phase count | 5 |
| Phases | proposal, code-analyze, test-design, test-gen, test-execution |
