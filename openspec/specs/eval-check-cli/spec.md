## 权威边界

编排门禁与评估写入的 MCP/CLI 契约。phase 集合见 `pge-workflow-engine`；回溯写入见 `pipeline-backtrack`（决议 C3=A、C4=A）。

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

`phase_log` SHALL 向 `openspec/changes/<name>/eval.json` 追加条目。输入可含：`change`、`phase`、`verdict`、`report`、`items`/`checklist`、`attempt`、`skipped` 等（以现行 schema 为准）。

MUST NOT 接受 `backtrack_to` 或 `backtrack_reason`。回溯由 `backtrack` MCP 原地改写条目（见 `pipeline-backtrack`）。

phase 标识符为无前缀 ID（如 `proposal`、`test-execution`），不得使用 `01-requirements` / `06-unit-test` 等旧前缀形式作为规范要求。

#### Scenario: pass 追加

- **WHEN** `phase_log` 以 `verdict: "pass"` 与有效 `change`/`phase` 调用
- **THEN** eval.json 追加一条 pass 记录

#### Scenario: 带 backtrack_to 的输入被拒绝

- **WHEN** 调用方在 `phase_log` 输入中提供 `backtrack_to`
- **THEN** schema / 工具拒绝该调用

### Requirement: 归档前校验 eval 链

归档流程（如 `openspec-archive-change`）SHALL 在 archive 前校验：工作流 phase 表中各 phase 的最新有效条目为 `pass`（非 stale），且无未完成的活跃回溯阻塞。实现可以是 CLI（若仍提供 `dev-team eval-check`）或等价逻辑；phase 序列以 `getPhaseTable(workflow_type)` 为准（requirement 8 / test-only 5）。

#### Scenario: 全 phase pass 允许归档

- **WHEN** requirement 工作流 8 个 phase 最新有效条目均为 pass
- **THEN** 归档前校验通过

#### Scenario: 缺 phase 或最新为 fail 则失败

- **WHEN** 某必需 phase 无有效 pass，或最新为 fail
- **THEN** 校验失败并阻止 archive

## Module Contract

| 接口 | Contract |
|------|----------|
| MCP `phase_next` | 编排门禁；见 pge-workflow-engine |
| MCP `phase_log` | 追加 eval；无 backtrack 输入 |
| MCP `backtrack` | 回溯字段写入；见 pipeline-backtrack |
| `phase_check` | 不注册 / 不作为规范要求 |
