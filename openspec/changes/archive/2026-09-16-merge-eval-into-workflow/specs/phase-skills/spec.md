## 权威边界

用户可调用的 `phase-*` skill。本变更修正「写入 eval.json」文案，并禁止 `phase-proposal` 用 Write 工具写已被 hook 保护的 `workflow.json`；`workflow.json` 成为工作流的前置文件后，缺失时 skill SHALL 停止并指引，而不是依赖缺省类型继续。

## ADDED Requirements

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

## Module Contract

| 路径 | 变更 |
|------|------|
| `plugins/dev-team/skills/phase-*/SKILL.md` | 评估写入文案指向 `phase_log` → `workflow.json` |
| `plugins/dev-team/skills/phase-proposal/SKILL.md` | 删除 Write `workflow.json`；缺文件时停止 + 二选一指引 |
| `plugins/dev-team/skills/workflow-requirement/SKILL.md`、`workflow-test-only/SKILL.md` | Evaluator 约定中的文件名；补「`workflow.json` 由 `change_create` 建立，是前置文件」 |
| `plugins/dev-team/skills/openspec-archive-change/SKILL.md` | artifacts 示例去掉 `eval.json`；`workflow_done: false` 的两种成因 |
