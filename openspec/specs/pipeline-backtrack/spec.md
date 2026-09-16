## 权威边界

回溯状态机：`backtrack` MCP、`phase_log` 不再写回溯字段、stale 传播、skill 三叉决策、evaluator 禁止设 `backtrack_to`。

`last_result` 字段形状见 `pge-workflow-engine`；本文件定义如何消费它做路由。

决议 **C3=A**：本文件为回溯路由权威；`eval-check-cli` / `hook-output-import` / `json-design-schemas` / `phase-agents` 已对齐；`intent-router` 已归档（C1/C2=B）。本变更改条目的磁盘文件（`workflow.json.eval`），并要求 `workflow.json` 既存且通过 `workflowFileSchema`（缺失 / 格式非法即抛错）。

## ADDED Requirements

### Requirement: phaseLogSchema 保留 backtrack 字段（只读兼容）

`phaseLogSchema` SHALL 含可选 `backtrack_to` / `backtrack_reason`（`z.string().max(500).optional().nullable()` 等），仅用于解析已有评估条目（无论位于 `workflow.json.eval` 还是遗留 `eval.json`）。新条目不再经 `phase_log` 写入这些字段。

#### Scenario: 旧条目可解析

- **WHEN** 解析含或不含 `backtrack_reason` 的条目（来自 `workflow.json.eval` 或遗留 `eval.json`）
- **THEN** 均不抛错；缺省为 null/undefined

### Requirement: backtrack MCP 工具

`backtrack` SHALL 按序验证：change 存在于 `openspec/changes/` → **`workflow.json` 存在且通过 `workflowFileSchema`** → `phase` 在工作流 phase 表中 → `backtrack_to` 索引小于 `phase` → `backtrack_reason` 非空 → `readEvalJson` 中存在该 `phase` 的最新条目。工作流类型来自 change 的 `workflow.json`（`getWorkflowType()`）；该读取的抛错（缺文件 / 格式非法）SHALL 直接透出为调用失败，MUST NOT 被吞掉后退化为缺省类型。

成功时：原地修改最新条目的 `backtrack_to`/`backtrack_reason`（不新增条目）；标记目标 phase 最新 pass 为 `stale: true`；`propagateStale()` 传播下游；经 `writeEvalJson` 写回 `workflow.json.eval`（保留 `workflow_type`/`created`）。返回 `{ modified, phase, target }`。

若该 phase 没有任何评估条目，SHALL 抛错（文案可仍说明「没有评估条目」，不必再写死 `eval.json` 文件名）。

MCP description SHALL 说明改写的是 `workflow.json` 中的评估条目。

#### Scenario: 成功 / 校验失败 / 不增条目

- **WHEN** 有效 `backtrack(change, phase, backtrack_to, backtrack_reason)` → `workflow.json.eval` 中该 phase 最新条目被改写，目标与下游 stale，`eval` 数组长度不变
- **WHEN** 目标不存在、目标在当前 phase 之后、缺 reason、或无最新条目 → 抛错
- **WHEN** change 目录缺 `workflow.json`，或文件存在但 JSON 非法 / 根非对象 / `workflow_type` 缺失或非枚举 → 抛错（文案含绝对路径，缺文件时含 `change_create` 指引），MUST NOT 创建文件
- **AND** 成功路径 MUST NOT 写入 `eval.json`

### Requirement: phase_log 不再接受 backtrack 字段

`phaseLogInputSchema` / `runPhaseLog` / `BuildEntryParams` SHALL 不含 `backtrack_to`/`backtrack_reason`；不再调用 `handleBacktrackMarking` / `validateBacktrackReason`。`phaseLogSchema` 仍保留字段供读旧文件。

#### Scenario: 输入拒绝多余字段

- **WHEN** `phaseLogInputSchema.parse` 带 `backtrack_to`
- **THEN** Zod 拒绝

### Requirement: Skill 统一回溯决策流程

所有 `phase-*` / `workflow-*` SKILL 在 Evaluator 返回后：

```
调用 phase_next 取 last_result
pass → 下一 phase
fail → 分析 report → retry | backtrack(...) | AskUserQuestion
       Skill 禁止在失败时自行写入 verdict="pass"（用户同意时由用户改 eval.json）
继续循环
```

适用于含 `test-execution` 在内的全部 phase，无特例。

#### Scenario: retry / backtrack / ask-user

- **WHEN** fail 且可重跑本 phase → 继续循环，不调 `backtrack`
- **WHEN** fail 且需更早 phase → 调 `backtrack(...)` 后继续；随后 `phase_next` 返回目标
- **WHEN** fail 可忽略 → `AskUserQuestion`；同意则用户手改 verdict；skill 不写 pass

### Requirement: test-execution skill 的 6 分支诊断映射

`phase-test-execution` 分析 `last_result.report` 后选目标（由 skill 调 `backtrack`，非 evaluator）：

1. 测试文件语法类错误 → `test-gen`
2. 实现文件 AssertionError → `implement`
3. 与 test-design 不一致 → `test-design`
4. 接口签名不匹配 → `dev-design`
5. 覆盖率不达标 → `test-design`
6. 无法判断 → `AskUserQuestion`

整体模式与其它 skill 相同，仅 report 分析多分支。

### Requirement: Evaluator 禁止设置 backtrack_to

`test-execution-evaluator`、`code-review-evaluator`、`acceptance-evaluator` SHALL 含约束：「禁止设置 backtrack_to；路由由 skill 调 `backtrack`；诊断写入 report」。

- code-review：设计矛盾写 `file:line` 证据；skill 可 `backtrack(..., "dev-design", ...)`
- acceptance：未满足 AC-ID 列入 report；skill 可 `backtrack(..., "proposal", ...)`

#### Scenario: evaluator 只诊断

- **WHEN** 上述 evaluator 判定 fail
- **THEN** 条目无 `backtrack_to`；verdict 为 fail；report 含证据

## MODIFIED Requirements

### Requirement: 回溯目标使用现行 phase ID

`backtrack_to` 不得引用 `integration-test`。典型映射：acceptance→`proposal`；code-review→`dev-design`；test-execution 语法→`test-gen`、逻辑→`implement`、设计→`test-design`。

#### Scenario: skill 回溯后 phase_next 返回目标

- **WHEN** skill 对 fail 的 `test-execution` 调用 `backtrack(..., backtrack_to: "test-gen", ...)`
- **THEN** 随后 `phase_next` 返回 `next_phase: "test-gen"`

- **WHEN** skill 对 acceptance fail 调用 `backtrack(..., backtrack_to: "proposal", ...)`
- **THEN** 随后 `phase_next` 返回 `next_phase: "proposal"`

### Requirement: Stale 由 backtrack 触发

原 `phase_log.handleBacktrackMarking` 逻辑现由 `backtrack` 持有；`propagateStale` 不再涉及 `integration-test`。

### Requirement: backtrack persist uses same eval store as phase_log

`runBacktrack` SHALL 只通过 `readEvalJson` / `writeEvalJson` 访问评估历史，MUST NOT 直接 `writeFileSync('eval.json')`。`writeEvalJson` 要求 `workflow.json` 既存且合法，因此 `backtrack` MUST NOT 依赖「写时自动补建文件」；文件缺失时 SHALL 报错。写成功后遗留 `eval.json` SHALL 被删除（由 `writeEvalJson` 统一执行）。

#### Scenario: 有 workflow.json + 遗留 eval.json 时迁入权威字段

- **WHEN** `workflow.json` 存在（含 `workflow_type`）但无 `eval` 键，遗留 `eval.json` 含某 phase 最新条目
- **AND** 调用合法 `backtrack`
- **THEN** 改写后的条目位于 `workflow.json.eval`
- **AND** `eval.json` 不再存在

#### Scenario: 缺 workflow.json 时 backtrack 失败且不建文件

- **WHEN** change 目录内没有 `workflow.json`
- **AND** 调用 `backtrack`
- **THEN** 抛错并指引 `change_create`
- **AND** MUST NOT 创建 `workflow.json` 或目录

## Module Contract（当前态）

| 模块 | 要点 |
|------|------|
| `backtrack.schema.ts` | 输入仍为 `change, phase, backtrack_to, backtrack_reason`；注释改为 `workflow.json`，并注明缺文件 / 格式非法时 `getWorkflowType` 先抛错 |
| `phase-log.schema.ts` | 输入无回溯字段；解析 schema 保留 optional（不变） |
| `backtrack.ts` | 校验 + 原地改写 + stale；persist → `writeEvalJson`（要求文件既存且合法） |
| `phase-next.ts` | 响应 `last_result`；`allowed_backtrack_phases` 可空数组兼容 |
| 相关 SKILL / evaluator | skill 决策；evaluator 禁设 `backtrack_to` |
| `change-config.ts` | `getWorkflowType` 严格化（缺文件 / 非法抛错），本工具直接透出该错误 |
| `mcp.ts` | `backtrack` description 指向 `workflow.json` |
