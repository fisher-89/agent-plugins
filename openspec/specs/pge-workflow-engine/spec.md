## ADDED Requirements

### Requirement: phase_next 输出 schema 包含 last_result 字段

`phaseNextOutputSchema` SHALL 包含 `last_result` 字段，类型为 `{ phase: string, verdict: 'pass' | 'fail', report: string, timestamp: string } | null`。

此字段代表最新的 eval.json 条目（按时间戳降序排列）。当 eval.json 为空时，`last_result` SHALL 为 `null`。

此字段供 skill 用于在 evaluator 运行后确定下一步操作：
- `verdict === "pass"` → 继续到下一 phase
- `verdict === "fail"` → 进入回溯决策流程（重试、回溯或询问用户）

#### Scenario: 正常 phase 时 phase_next 响应包含 last_result
- **WHEN** 调用 `runPhaseNext({change: "my-change"})` 且 `phase_next` 返回正常 phase 响应（非 done、非 error）
- **THEN** 响应包含 `last_result`，带有 `phase`、`verdict`、`report` 和 `timestamp` 字段
- **AND** 所有其他已有字段（`next_phase`、`planner`、`evaluator`、`round` 等）不变

#### Scenario: eval.json 为空时 phase_next 响应包含 null 的 last_result
- **WHEN** eval.json 为空
- **THEN** `last_result` 为 `null`

#### Scenario: 出错时 phase_next 响应包含 null 的 last_result
- **WHEN** `phase_next` 返回错误响应（例如超过轮次上限）
- **THEN** `last_result` 为 `null`

#### Scenario: 完成时 phase_next 响应包含 null 的 last_result
- **WHEN** 所有 phase 已通过且 `phase_next` 返回 `done: true`
- **THEN** `last_result` 为 `null`

#### Scenario: last_result 来自按时间戳排序的最新条目
- **WHEN** eval.json 有条目 `[{phase:"proposal", verdict:"pass", report:"ok", timestamp:"2026-01-01T00:00:00Z"}, {phase:"dev-design", verdict:"fail", report:"设计不完整", timestamp:"2026-01-01T01:00:00Z"}]` 且调用 `runPhaseNext()`
- **THEN** `last_result` 为 `{phase: "dev-design", verdict: "fail", report: "设计不完整", timestamp: "2026-01-01T01:00:00Z"}`

#### Scenario: 乱序条目按时间戳取最新
- **WHEN** eval.json 有条目乱序：`[{phase:"acceptance", verdict:"pass", report:"ok", timestamp:"2026-01-02T00:00:00Z"}, {phase:"proposal", verdict:"pass", report:"ok", timestamp:"2026-01-01T00:00:00Z"}]`
- **THEN** `last_result` 反映 `acceptance` 条目（按时间戳最新）

---

## MODIFIED Requirements

### Requirement: 八阶段工作流结构（原为九阶段）

需求工作流 SHALL 支持 8 个顺序 phase，标识符如下：

| Phase | Identifier | Pattern | Description |
|-------|-----------|---------|-------------|
| proposal | proposal | DESIGN Planner->Evaluator | 提案与需求 |
| dev-design | dev-design | DESIGN Planner->Evaluator | 实现设计 |
| test-design | test-design | DESIGN Planner->Evaluator | 测试场景设计 |
| implement | implement | EXEC Generator->Evaluator + AUTO static-check | 实现代码生成 |
| test-gen | test-gen | EXEC Generator->Evaluator | 测试代码生成 |
| test-execution | test-execution | EXEC Executor->Evaluator (sonnet) | 全自动测试执行（单元 + 集成） |
| code-review | code-review | EVAL-ONLY Evaluator | 代码审查评估 |
| acceptance | acceptance | EVAL-ONLY Evaluator | 验收评估 |

执行顺序仅由 `PHASE_REQUIREMENT` 表中的数组位置决定。Phase 标识符不再编码顺序信息。

**执行顺序：** `implement` SHALL 在规范 `PHASES` 数组中位于 `test-gen` 之前（从 `PHASE_REQUIREMENT` 派生）。

每个 phase SHALL 在完成后将其结果追加到 eval.json。

#### Scenario: Phase proposal 使用 proposal-planner 子 agent
- **WHEN** Phase `proposal` 执行
- **THEN** `proposal-planner` 子 agent 写入 proposal.md + specs/
- **AND** `proposal-evaluator` 子 agent 根据 checklist 评估
- **AND** 该 phase 遵循与 `dev-design` 和 `test-design` 相同的 P→E 循环模式

#### Scenario: 完整工作流按前置依赖推进
- **WHEN** 所有 phase 从 `proposal` 到 `acceptance` 成功完成
- **THEN** eval.json 包含所有 8 个 phase 的条目，verdict 为 "pass"

#### Scenario: Phase 顺序强制执行使用前置依赖
- **WHEN** phase/next 扫描且 `dev-design` 无有效 pass
- **THEN** phase/next SHALL 返回 `dev-design`（`test-design` 需要 `dev-design` 作为前置依赖）
- **AND** `test-design` SHALL NOT 在 `dev-design` 有有效 pass 之前被返回

- **WHEN** phase/next 扫描且 `implement` 无有效 pass 但 `test-gen` 有
- **THEN** phase/next SHALL 返回 `implement`（`test-gen` 需要 `implement` 作为前置依赖；仅 `test-gen` pass 不够）

#### Scenario: Implement 在规范顺序中先于 test-gen 执行
- **WHEN** phase `proposal`、`dev-design` 和 `test-design` 有有效 pass 条目，且 `implement` 和 `test-gen` 均未通过
- **THEN** `PHASES` 中 `implement` 的索引小于 `test-gen`
- **AND** phase/next SHALL 返回 `implement` 作为下一 phase

#### Scenario: Test-gen 在 implement pass 之后运行
- **WHEN** phase `proposal` 到 `test-design` 和 `implement` 有有效 pass 条目，且 `test-gen` 未通过
- **THEN** phase/next SHALL 返回 `test-gen` 作为下一 phase

### Requirement: 前置依赖表（test-execution 的 ID 更新）

`requirement` workflow_type 的前置依赖表 SHALL 使用更新后的 phase 标识符：

| Phase | Prerequisites | Notes |
|-------|---------------|-------|
| proposal | [] | 根 phase，无依赖 |
| dev-design | [proposal] | 仅依赖 proposal |
| test-design | [proposal, dev-design] | 依赖 proposal 和 dev-design |
| implement | [dev-design] | 仅依赖 dev-design |
| test-gen | [test-design, implement] | 需要测试设计 AND 完成的实现 |
| test-execution | [test-gen, implement] | 需要两条轨道 |
| code-review | [test-gen, implement] | 与 test-execution 相同的前置依赖 |
| acceptance | [proposal, dev-design, implement] | 纯开发轨道，无测试轨道依赖 |

对于 `bug-fix` workflow_type：

| Phase | Prerequisites |
|-------|--------------|
| proposal | [] |
| dev-design | [proposal] |
| implement | [dev-design] |
| test-execution | [implement] |
| code-review | [implement] |
| acceptance | [code-review] |

对于 `test-only` workflow_type：

| Phase | Prerequisites |
|-------|--------------|
| proposal | [] |
| code-analyze | [proposal] |
| test-design | [proposal, code-analyze] |
| test-gen | [test-design] |
| test-execution | [test-gen] |

#### Scenario: requirement 工作流更新后的 ID 的 getPrerequisites
- **WHEN** 调用 `getPrerequisites("proposal", "requirement")`
- **THEN** 返回 `[]`

- **WHEN** 调用 `getPrerequisites("implement", "requirement")`
- **THEN** 返回 `["dev-design"]`

- **WHEN** 调用 `getPrerequisites("test-gen", "requirement")`
- **THEN** 返回 `["test-design", "implement"]`

- **WHEN** 调用 `getPrerequisites("test-execution", "requirement")`
- **THEN** 返回 `["test-gen", "implement"]`

- **WHEN** 调用 `getPrerequisites("acceptance", "requirement")`
- **THEN** 返回 `["proposal", "dev-design", "implement"]`

- **WHEN** 调用 `getPrerequisites("code-review", "requirement")`
- **THEN** 返回 `["test-gen", "implement"]`（不再包含 `integration-test`）

#### Scenario: bug-fix 工作流更新后的 ID 的 getPrerequisites
- **WHEN** 调用 `getPrerequisites("implement", "bug-fix")`
- **THEN** 返回 `["dev-design"]`

- **WHEN** 调用 `getPrerequisites("acceptance", "bug-fix")`
- **THEN** 返回 `["code-review"]`

- **WHEN** 调用 `getPrerequisites("test-execution", "bug-fix")`
- **THEN** 返回 `["implement"]`

#### Scenario: test-only 工作流的 getPrerequisites
- **WHEN** 调用 `getPrerequisites("test-execution", "test-only")`
- **THEN** 返回 `["test-gen"]`

### Requirement: 依赖图（反向依赖查找，ID 更新）

对于 `requirement` workflow_type，依赖图 SHALL 使用更新后的 phase 标识符：

| Phase | Dependents |
|-------|-----------|
| proposal | [dev-design, test-design, acceptance] |
| dev-design | [test-design, implement, acceptance] |
| test-design | [test-gen] |
| implement | [test-gen, test-execution, code-review, acceptance] |
| test-gen | [test-execution, code-review] |
| test-execution | [] |
| code-review | [] |
| acceptance | [] |

#### Scenario: getDependents 使用更新后的 ID 返回正确依赖
- **WHEN** 调用 `getDependents("proposal")`
- **THEN** 返回 `["dev-design", "test-design", "acceptance"]`

- **WHEN** 调用 `getDependents("implement")`
- **THEN** 返回 `["test-gen", "test-execution", "code-review", "acceptance"]`（不再包含 `integration-test`）

- **WHEN** 调用 `getDependents("test-gen")`
- **THEN** 返回 `["test-execution", "code-review"]`（不再包含 `integration-test`）

- **WHEN** 调用 `getDependents("test-execution")`
- **THEN** 返回 `[]`

## REMOVED Requirements

### Requirement: 九阶段工作流结构（含 integration-test）

**Reason**: `integration-test` phase 已作为将所有测试执行合并到单一 `test-execution` phase 的一部分被移除。需求工作流现在有 8 个 phase 而非 9 个。

**Migration**: 从所有 phase 表（PHASE_REQUIREMENT、PHASE_BUG_FIX、PHASE_TEST_ONLY）中移除 `integration-test`。相应地更新所有 phase 引用。

### Requirement: 前置依赖表中的 integration-test phase

**Reason**: `PHASE_PREREQUISITES` 表中所有 `integration-test` 条目已移除。`test-execution` phase 替代了 `unit-test` 和 `integration-test`。

**Migration**: 从所有前置依赖表中移除 `integration-test: ["test-gen", "implement"]`。

### Requirement: 依赖图中的 integration-test

**Reason**: `integration-test` phase 在工作流中不再存在，因此不再是任何 phase 的依赖。

**Migration**: 从 `implement` 和 `test-gen` 的依赖数组中移除 `integration-test`。

## 模块契约

### workflow.ts (`plugins/dev-team/bin/src/lib/`)

| Export / Constant | Change | Purpose |
|-------------------|--------|---------|
| `PHASE_REQUIREMENT` | MODIFIED | 移除 `integration-test` 条目；重命名 `unit-test` → `test-execution`（id、description、planner、evaluator） |
| `PHASE_BUG_FIX` | MODIFIED | 移除 `integration-test`（原本不存在）；重命名 `unit-test` → `test-execution` |
| `PHASE_TEST_ONLY` | MODIFIED | 移除 `integration-test` 条目；重命名 `unit-test` → `test-execution` |
| `PHASE_REQUIREMENT` 长度 | MODIFIED | 从 9 个 phase 到 8 个 |
| `PHASE_BUG_FIX` 长度 | MODIFIED | 从 6 到 6（无变化） |
| `PHASE_TEST_ONLY` 长度 | MODIFIED | 从 6 到 5 个 phase |
| `PHASE_PREREQUISITES` | MODIFIED | 移除 `integration-test` key；重命名 `unit-test` key → `test-execution` |
| `PHASE_BUG_FIX_PREREQUISITES` | MODIFIED | 重命名 `unit-test` → `test-execution` |
| `PHASE_TEST_ONLY_PREREQUISITES` | MODIFIED | 移除 `integration-test` key；重命名 `unit-test` → `test-execution` |
| `getPhaseTable()` | UNCHANGED | API 不变 — phase 表内容更新 |
| `getPrerequisites()` | UNCHANGED | API 不变 — 前置依赖表内容更新 |
| `getDependents()` | UNCHANGED | API 不变 — 依赖从更新后的前置依赖表派生 |

### phase-next.schema.ts

| Export | Change | Purpose |
|--------|--------|---------|
| `phaseNextOutputSchema` | ADDED `last_result` | `z.object({ phase: z.string(), verdict: z.enum(['pass', 'fail']), report: z.string(), timestamp: z.string() }).nullable()` — 最新 eval 条目快照，供 skill 使用 |

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `hasPhasePassed()` | UNCHANGED | 过滤 `stale: true` 条目 — 行为不变 |
| `runPhaseNext()` | MODIFIED | 从 eval.json 读取最新条目并附加为 `last_result` |
| `resolvePhaseNext()` | MODIFIED | 在所有响应构建函数中传递 `last_result` |
| `PhaseNextResult` 类型 | MODIFIED | 包含 `last_result` 字段 |
| `buildPhaseResponse()` | MODIFIED | 增加 `lastResult?` 参数，包含在返回值中 |
| `buildDoneResponse()` | MODIFIED | 增加 `lastResult?` 参数，返回值中包含 `last_result: null` |
| `buildErrorResponse()` | MODIFIED | 增加 `lastResult?` 参数，返回值中包含 `last_result: null` |

### eval-json.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `checkGate()` | UNCHANGED | 接受前置依赖数组；过滤 `stale: true` 条目 |

### Phase 表对比

| Workflow | Before | After |
|----------|--------|-------|
| requirement | 9 个 phase 含 `unit-test`、`integration-test` | 8 个 phase：`test-execution` 替代两者 |
| bug-fix | 6 个 phase：`unit-test`（无 `integration-test`） | 6 个 phase：`test-execution` 替代 `unit-test` |
| test-only | 6 个 phase 含 `unit-test`、`integration-test` | 5 个 phase：`test-execution` 替代两者 |
