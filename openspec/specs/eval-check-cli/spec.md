## 权威边界

编排门禁与评估写入的 MCP 契约。phase 集合见 `pge-workflow-engine`；回溯写入见 `pipeline-backtrack`（决议 C3=A、C4=A）。本变更只改 `phase_log` 的持久化文件。

- 编排门禁：`phase_next`（必填 `run_id`）
- 评估追加：`phase_log`（**不**接受 `backtrack_to` / `backtrack_reason`）
- `phase_check`：**已退役**，本文件不再规定其行为

## Requirements

### Requirement: phase_next 作为编排门禁

Skill / workflow SHALL 通过 `phase_next(change, run_id)` 获取下一可执行 phase、错误与 `last_result`。行为与输出字段以 `pge-workflow-engine` 为准。

前置是否满足、stale、session round/retry 均由 `phase_next` 计算；调用方 MUST NOT 依赖已删除的 `phase_check`。

#### Scenario: 门禁调用形态

- **WHEN** skill 或 workflow 进入某 phase 前
- **THEN** 调用 `phase_next` 且携带非空 `run_id`
- **AND** 以 `next_phase` / `error` / `done` 决定是否继续

### Requirement: phase_log 追加评估结果（无回溯字段）

`phase_log` SHALL 向 `openspec/changes/<name>/workflow.json` 的 `eval` 数组追加条目（经 `appendEntry` / `writeEvalJson`）。输入可含：`change`、`phase`、`report`、`checklist`、`attempt`、`skipped` 等（以现行 `phaseLogInputSchema` 为准；`verdict` 仍由 checklist 推导）。

MUST NOT 接受 `backtrack_to` 或 `backtrack_reason`。回溯由 `backtrack` MCP 原地改写条目（见 `pipeline-backtrack`）。

MUST NOT 创建或更新 `eval.json`。

phase 标识符为无前缀 ID（如 `proposal`、`test-execution`），不得使用 `01-requirements` / `06-unit-test` 等旧前缀形式作为规范要求。

MCP 工具 description SHALL 说明写入目标为 `workflow.json`（而非 `eval.json`）。

#### Scenario: pass 追加

- **WHEN** `phase_log` 以有效 `change`/`phase`/`report`/`checklist` 调用且 checklist 全 pass
- **THEN** `workflow.json.eval` 追加一条 `verdict: "pass"` 记录
- **AND** 不存在新的 `eval.json` 写入

#### Scenario: 带 backtrack_to 的输入被拒绝

- **WHEN** 调用方在 `phase_log` 输入中提供 `backtrack_to`
- **THEN** schema / 工具拒绝该调用

### Requirement: 归档前校验 eval 链

归档流程（如 `openspec-archive-change`）SHALL 在 archive 前校验工作流是否完成。现行实现 SHALL 继续使用 `change_list` 的 `workflow_done`（其读取 `readEvalJson`，即 `workflow.json.eval` / 遗留回退），而不是独立的 `dev-team eval-check` CLI（该 CLI 已不在 `cli.ts` 中注册）。

phase 序列以 `getPhaseTable(workflow_type)` 为准（requirement 8 / test-only 5）。

#### Scenario: 全 phase pass 允许归档

- **WHEN** requirement 工作流各 phase 在 `workflow.json.eval` 中最新有效条目均为 pass（或 skipped）
- **THEN** `change_list.workflow_done` 为 `true`，归档前校验通过

#### Scenario: 缺 phase 或最新为 fail 则失败

- **WHEN** 某必需 phase 无有效 pass，或最新为 fail
- **THEN** `workflow_done` 为 `false`

## Module Contract

| 接口 | Contract |
|------|----------|
| MCP `phase_next` | 只读评估条目；见 pge-workflow-engine |
| MCP `phase_log` | 追加到 `workflow.json.eval`；无 backtrack 输入；description 提及 `workflow.json` |
| MCP `backtrack` | 回溯字段写入同一 `eval` 数组 |
| `phase_check` / `eval-check` CLI | 不注册 |
