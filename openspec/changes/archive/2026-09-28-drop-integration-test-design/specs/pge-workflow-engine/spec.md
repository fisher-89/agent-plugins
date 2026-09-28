## MODIFIED Requirements

### Requirement: 八阶段工作流结构

`requirement` 工作流 SHALL 支持 8 个顺序 phase（标识符不编码序号；顺序由 `PHASE_REQUIREMENT` 数组位置决定）：

| Phase | Pattern | Description |
|-------|---------|-------------|
| proposal | DESIGN P→E | 提案与需求 |
| dev-design | DESIGN P→E | 实现设计 |
| test-design | DESIGN P→E | 测试场景设计 |
| implement | EXEC G→E + AUTO | 实现代码 |
| test-gen | EXEC G→E | 测试代码 |
| test-execution | EXEC Executor→E | 全自动测试 |
| code-review | EVAL-ONLY | 代码审查 |
| acceptance | EVAL-ONLY | 验收 |

`implement` 在 `PHASES` 中位于 `test-gen` 之前。每 phase 完成后经 `phase_log` 追加到 `workflow.json.eval`。

#### Scenario: 前置依赖推进

- **WHEN** `dev-design` 无有效 pass → `phase_next` 返回 `dev-design`（不得先返回依赖它的 `test-design`）
- **WHEN** `implement` 未通过而 `test-gen` 有 pass → 仍返回 `implement`
- **WHEN** proposal…test-design 已 pass 且 implement/test-gen 均未过 → 返回 `implement`（索引小于 `test-gen`）

#### Scenario: 空评估存储视为 first run

- **WHEN** `workflow.json` **存在且通过 `workflowFileSchema`**，但无 `eval` 键，且无遗留 `eval.json`
- **THEN** `phase_next` 行为与合并前「空 eval.json」相同（requirement 返回 `proposal`）

#### Scenario: workflow.json 缺失或非法时 phase_next 抛错

- **WHEN** change 目录没有 `workflow.json`，或文件存在但 JSON 非法 / 根非对象 / `workflow_type` 缺失或非枚举
- **THEN** `phase_next` SHALL 以可读错误终止（文案含绝对路径，缺文件时含 `change_create` 指引）
- **AND** MUST NOT 按缺省 `requirement` 返回 `proposal`

## ADDED Requirements

### Requirement: phase prompt 文案不区分测试层级

`plugins/dev-team/bin/src/lib/workflow.ts` 中三种工作流（requirement / bug-fix / test-only）的 test-execution phase prompt SHALL 为 `Run and fix all tests for change "<change>".`，MUST NOT 含 `(unit + integration)` 层级表述。phase 表结构、phase id 与前置依赖不变。

#### Scenario: prompt 文案已改写

- **WHEN** 检索 `workflow.ts` 全部 phase prompt
- **THEN** 三处 test-execution prompt 均为 `Run and fix all tests for change "<change>".`
- **AND** 全仓库源码无 `unit + integration` 字样

#### Scenario: phase 表数据不受文案影响

- **WHEN** 读取 `PHASE_REQUIREMENT` / `PHASE_BUG_FIX` / `PHASE_TEST_ONLY` 的 test-execution 条目
- **THEN** phase id、pattern、executor / evaluator、前置依赖均不变，仅 prompt 文案改写

## Module Contract

### `plugins/dev-team/bin/src/lib/workflow.ts`

| 位置 | 变更 |
|------|------|
| `PHASE_REQUIREMENT` / `PHASE_BUG_FIX` / `PHASE_TEST_ONLY` 的 test-execution prompt（原 114 / 188 / 281 行附近） | `Run and fix all tests (unit + integration) for change "<change>".` → `Run and fix all tests for change "<change>".` |
| phase 表结构 / 长度 / 前置依赖 | UNCHANGED（既有守卫测试保持） |
