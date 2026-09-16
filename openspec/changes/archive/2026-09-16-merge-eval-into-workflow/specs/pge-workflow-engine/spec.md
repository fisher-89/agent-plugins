## 权威边界

`phase_next` 引擎：必填 `run_id`、进程内 session window、`round`/`retry` 限额、phase 表。本变更不改算法，只改评估条目数组的读取来源（`readEvalJson` → `workflow.json.eval`），并把 `workflow.json` 的读取前置条件严格化（缺失 / 格式非法即抛错，不再缺省 `requirement`）。

## MODIFIED Requirements

### Requirement: session 窗口由进程内 (change, run_id) anchor 界定

进程内存 map：key=`(change, run_id)`，value=首次见到时评估条目数组的 `entries.length`（anchor）。同 key 复用 anchor；窗口为 `entries[anchor..]`。`entries` 来自 `readEvalJson`（权威 `workflow.json.eval`，无该数组时回退遗留 `eval.json`）。

- 首次：`anchor = entries.length`，此时 window 空，`round = 1`
- 不同 `change` MUST NOT 共享同一 `run_id` 的 anchor
- map MUST NOT 落盘；进程重启后清空，同一 `run_id` 再出现视为新窗

#### Scenario: 首次建立 / 复用 / 隔离 / 重启

- **WHEN** 生涯已有 12 条且首次 `run_id: "s1"` → anchor=12，`round=1`
- **WHEN** 同 key 下又增 3 条再调用 → window 为末 3 条，`round=4`
- **WHEN** change `B` 首次用同一 `"s1"` → 独立 anchor
- **WHEN** 进程重启后再用曾用过的 `run_id` → 以当前 `entries.length` 新建 anchor

### Requirement: 八阶段工作流结构

`requirement` 工作流 SHALL 支持 8 个顺序 phase（标识符不编码序号；顺序由 `PHASE_REQUIREMENT` 数组位置决定）：

| Phase | Pattern | Description |
|-------|---------|-------------|
| proposal | DESIGN P→E | 提案与需求 |
| dev-design | DESIGN P→E | 实现设计 |
| test-design | DESIGN P→E | 测试场景设计 |
| implement | EXEC G→E + AUTO | 实现代码 |
| test-gen | EXEC G→E | 测试代码 |
| test-execution | EXEC Executor→E | 全自动测试（单元+集成） |
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

### Requirement: phase_next remains read-only on the eval store

`runPhaseNext` SHALL 只调用 `readEvalJson`，MUST NOT 调用 `writeEvalJson`，MUST NOT 创建或删除 `eval.json` / 改写 `workflow.json.eval`。

读取工作流类型时 SHALL 直接使用 `getWorkflowType()` 的严格结果：文件缺失或格式非法时抛错 MUST NOT 被 catch 吞掉而退化为缺省类型。

读取失败时的错误信息 SHALL 指向评估存储（可提 `workflow.json`），不必再写死 `Failed to read eval.json`。

#### Scenario: phase_next 不写盘

- **WHEN** `phase_next` 成功返回下一 phase
- **THEN** `workflow.json` 与遗留 `eval.json`（若有）内容与调用前相同

#### Scenario: 类型解析失败直接终止

- **WHEN** `getWorkflowType()` 抛错（`workflow.json` 缺失或格式非法）
- **THEN** `phase_next` SHALL 把该错误透出为调用失败
- **AND** MUST NOT 退化为缺省 `requirement` 后继续返回 `proposal`

## Module Contract（当前态）

| 位置 | 要点 |
|------|------|
| `phase-next.ts` | `getWorkflowType(change)`（严格，抛错不吞）+ `readEvalJson(changeDir)`；只读；anchor=`entries.length` |
| `eval-json.ts` | 条目数组来源改为 `workflow.json.eval` |
| `change-config.ts` | `workflow.json` 必须存在且通过 `workflowFileSchema`；删除 `DEFAULT_WORKFLOW_TYPE` |
| `schemas/workflow.schema.ts` | 上述校验的唯一来源（新增） |
| `phase-next.schema.ts` | 输入/输出字段不变 |
