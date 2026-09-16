## 权威边界

用户可调用的 `phase-*` skill 清单与 agent 链。门禁/`run_id` 引擎语义见 `pge-workflow-engine`；fail 后三叉决策见 `pipeline-backtrack`。本文件不复述 phase 前置依赖表。本变更修正「写入 eval.json」文案，并禁止 `phase-proposal` 用 Write 工具写已被 hook 保护的 `workflow.json`；`workflow.json` 成为工作流的前置文件后，缺失时 skill SHALL 停止并指引，而不是依赖缺省类型继续。

门禁工具统一为 `phase_next`（决议 C4=A）。不使用 `phase_check`。

## ADDED Requirements

### Requirement: phase skill 调用 phase_next 时必须传入 run_id

所有 `phase-*` skill（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance）SHALL：

1. 在本 turn 入口生成非空 `run_id`
2. 本 turn 内每一次 `phase_next`（含 backtrack 后 recall）传入同一 `run_id`
3. 不得省略（省略 → 引擎 `missing_run_id`）

**用户反馈的任意消息**（含 AskUser 回复、中断后继续、闲聊后再开）MUST 生成新的 `run_id`，MUST NOT 复用上一值（决议 C9）。

独立调用时 MUST 自行生成 `run_id`，MUST NOT 依赖 workflow 注入。

```
__MCP:phase_next__(change=<change-name>, run_id=<run_id>)
```

#### Scenario: 独立 phase skill 传入 run_id

- **WHEN** 用户直接调用任一上述 `phase-*` skill
- **THEN** 初始 gate 与 backtrack 后 recall 的 `phase_next` SHALL 均携带本 turn 的 `run_id`

#### Scenario: 用户任意新消息换 run_id

- **WHEN** 用户发送任意新消息（含对 AskUserQuestion 的回复）
- **THEN** skill SHALL 生成新的 `run_id` 再调用 `phase_next`

### Requirement: phase skills describe eval persistence as workflow.json

所有 `phase-*` skill 在检查 evaluator 是否写入评估结果时，SHALL 将成功条件表述为 `phase_log` 已写入 `workflow.json`（或 `phase_next.last_result` 已更新），MUST NOT 再把「未写入 eval.json」当作错误文案的唯一文件名。

`workflow-test-only` / `workflow-requirement` 中「Evaluator 调用 `phase_log`」的说明 SHALL 指向 `workflow.json`。

`openspec-archive-change` 对 `artifacts` 的示例 SHALL NOT 再列举 `eval.json` 作为完成物；完成性以 `workflow_done` 为准，并 SHALL 说明 `workflow_done: false` 也可能源于 `workflow.json` 缺失或格式非法，此时指引 `change_create`。

`workflow-requirement` / `workflow-test-only` SHALL 说明 `workflow.json` 由 `change_create` 建立，是 `phase_next` / `backtrack` 的前置文件（不得手写）。

#### Scenario: phase skill 错误文案

- **WHEN** evaluator 返回后 `phase_next.last_result` 缺失或未反映本次评估
- **THEN** skill 报错可说明 evaluator 未通过 `phase_log` 写入 `workflow.json`
- **AND** 文案 MUST NOT 把独立文件 `eval.json` 当作唯一合法存储

### Requirement: phase-proposal must not Write workflow.json

`phase-proposal` SHALL NOT 使用 Write/Edit 创建或覆盖 `openspec/changes/<name>/workflow.json`。

- 新建 change：经 `change_create` 写入（已有行为）
- 目录已存在且 `workflow.json` 已存在：跳过类型确认（已有行为）
- 目录已存在但缺少 `workflow.json`：SHALL **停止**，不进入 Phase Check（`phase_next` 会因缺文件报错），并给出二选一指引：(a) 由用户手动写入 `{ "workflow_type": "<type>", "created": "<YYYY-MM-DD>" }`；(b) 改走对应 `workflow-*` skill / `change_create` 新建。MUST NOT 用 Write/Edit 创建该文件，MUST NOT 依赖缺省 `"requirement"` 继续门禁

#### Scenario: phase-proposal 无 Write workflow.json 步骤

- **WHEN** 读取 `plugins/dev-team/skills/phase-proposal/SKILL.md`
- **THEN** Confirm workflow type 节 MUST NOT 要求 Write `{ "workflow_type": "<choice>" }` 到 `workflow.json`

#### Scenario: 缺 workflow.json 时 phase_next 报错、skill 停止并指引

- **WHEN** change 目录存在但没有 `workflow.json`
- **THEN** `phase_next` SHALL 因缺文件报错（不再按缺省 `requirement` 返回 `proposal`）
- **AND** `phase-proposal` SHALL 在调用 `phase_next` 前停止，并提示用户手写该文件或改走 `change_create` / `workflow-*` skill
- **AND** MUST NOT 用 Write/Edit 创建该文件

## MODIFIED Requirements

### Requirement: Eight user-triggered phase skills

系统 SHALL 提供 8 个 phase skill（原 9：`phase-unit-test`→`phase-test-execution`，`phase-integration-test` 删除并入前者）：

| # | Skill | MCP | Planner / Executor | Evaluator |
|---|-------|-----|--------------------|-----------|
| 1 | phase-proposal | phase_next (+ phase_log via evaluator) | proposal-planner | proposal-evaluator |
| 2 | phase-dev-design | phase_next | dev-design-planner | dev-design-evaluator |
| 3 | phase-test-design | phase_next | test-design-planner | test-design-evaluator |
| 4 | phase-test-gen | phase_next | test-gen-generator | test-gen-evaluator |
| 5 | phase-implement | phase_next | implementation-generator | implementation-evaluator |
| 6 | phase-test-execution | phase_next + phase_log | test-execution-executor | test-execution-evaluator |
| 7 | phase-code-review | phase_next | (none) | code-review-evaluator |
| 8 | phase-acceptance | phase_next | (none) | acceptance-evaluator |

门禁：调用 `phase_next(change, run_id)`，以返回的 `next_phase` / 错误决定是否可执行本 phase。`phase-test-execution` 另经 executor/evaluator 使用 `phase_log`；读 verdict 使用 `phase_next` 的 `last_result` / `next_phase`（与其它 skill 相同）。

#### Scenario: phase-test-execution EXEC 循环

- **WHEN** 用户调用 `/dev-team:phase-test-execution <change-name>`
- **THEN** skill 以 `phase_next(change, run_id)` 做门禁（期望 `next_phase` 为 `test-execution` 或可继续）
- **AND** 依次调用 `test-execution-executor`、`test-execution-evaluator`
- **AND** fail 时按 `pipeline-backtrack` 决策（非 evaluator 设 `backtrack_to`）

#### Scenario: 旧 skill 路径不存在

- **WHEN** 检查 `skills/phase-unit-test/` 或 `skills/phase-integration-test/`
- **THEN** 目录 SHALL NOT 存在；功能由 `phase-test-execution` 承担

## Module Contract

路径：`plugins/dev-team/skills/`。Agent 映射同上表。

| 路径 | 变更 |
|------|------|
| `plugins/dev-team/skills/phase-*/SKILL.md` | 评估写入文案指向 `phase_log` → `workflow.json` |
| `plugins/dev-team/skills/phase-proposal/SKILL.md` | 删除 Write `workflow.json`；缺文件时停止 + 二选一指引 |
| `plugins/dev-team/skills/workflow-requirement/SKILL.md`、`workflow-test-only/SKILL.md` | Evaluator 约定中的文件名；补「`workflow.json` 由 `change_create` 建立，是前置文件」 |
| `plugins/dev-team/skills/openspec-archive-change/SKILL.md` | artifacts 示例去掉 `eval.json`；`workflow_done: false` 的两种成因 |
