# 测试设计: consolidate-test-execution-phase

> **日期**: 2026-07-03

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|-------|---------|---------|----------|----------|
| AC-1 | phaseIdSchema 枚举包含 `test-execution`，不包含 `integration-test` 和 `unit-test` | 单元测试 | `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseIdSchema |
| AC-2 | `getPhaseTable("requirement")` 返回 8 个 phase，id 列表为 `[proposal, dev-design, test-design, implement, test-gen, test-execution, code-review, acceptance]` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — requirement |
| AC-3 | `getPhaseTable("bug-fix")` 返回 6 个 phase，不包含 `integration-test` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — regression（bug-fix） |
| AC-4 | `getPhaseTable("test-only")` 返回 5 个 phase，不包含 `integration-test` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — test-only |
| AC-5 | `getPrerequisites("test-execution", "requirement")` 返回 `["test-gen", "implement"]`；`getPrerequisites("code-review", "requirement")` 返回 `["test-gen", "implement"]`（不含 integration-test） | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPrerequisites |
| AC-6 | `getDependents("implement")` 不包含 `integration-test`；`getDependents("test-gen")` 不包含 `integration-test` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents |
| AC-7 | `dev-team test-execution` 可用，`dev-team unit-test` 不可用或显示 deprecated 提示 | 单元测试 + 集成测试 | `plugins/dev-team/bin/src/cli.test.ts` + `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | CLI 命令注册 + CLI 端到端执行 |
| AC-8 | `agents/test-execution-executor.md` 和 `agents/test-execution-evaluator.md` 存在且内容正确；`agents/integration-test-executor.md` 和 `agents/integration-test-evaluator.md` 不存在 | 集成测试 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 文件存在性验证 |
| AC-9 | `skills/phase-test-execution/SKILL.md` 存在且内容正确；`skills/phase-integration-test/` 目录不存在 | 集成测试 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Skill 文件存在性验证 |
| AC-10 | 更新所有 phase 引用后，`workflow.test.ts`、`phase-next.test.ts`、`eval-json.test.ts` 测试通过 | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts`、`plugins/dev-team/bin/src/commands/phase-next.test.ts`、`plugins/dev-team/bin/src/lib/eval-json.test.ts` | 回归（已有 describe 块更新 phase 引用） |
| AC-11 | CLI 输出从 `reports/unit-test-execution.json` 改为 `reports/test-execution.json`；子报告从 `reports/unit-test/<fw>.json` 改为 `reports/test-execution/<fw>.json` | 集成测试 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | 报告路径 — AC-11 |
| AC-12 | `propagateStale(entries, "implement")` 传播到 `test-gen`, `test-execution`, `code-review`, `acceptance`（不含 integration-test） | 单元测试 | `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale |

---

## 单元测试

### 用例

#### 测试文件: `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts`（新增）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseIdSchema | 正向 | 枚举包含 `'test-execution'`（AC-1） | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseIdSchema | 正向 | 枚举不包含 `'unit-test'` | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseIdSchema | 正向 | 枚举不包含 `'integration-test'` | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseIdSchema | 边界 | 枚举值总数 = 9（proposal, dev-design, test-design, implement, test-gen, test-execution, code-review, acceptance, code-analyze） | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseIdSchema | 异常 | 未知 phase ID `'invalid-phase'` 导致 zod parse 失败 | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseLogSchema | 正向 | 有效 entry 通过 zod parse | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseLogSchema | 异常 | 无效 verdict（非 "pass"/"fail"）导致 parse 失败 | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseLogSchema | 边界 | `report` 长度 = 500 字符时通过 | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseLogSchema | 边界 | `report` 长度 = 0（空字符串）时通过 | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseLogSchema | 边界 | `report` 长度 > 500 字符时 parse 失败 | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | phaseLogSchema | 异常 | `backtrack_to` 为字符串 `"null"` 时 refine 校验失败 | 新增 |

#### 测试文件: `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts`（新增）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | TestExecutionSubReport | 正向 | 有效 subReport 通过 schema parse | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | TestExecutionSubReport | 边界 | `coverage` = null 时通过（框架未收集覆盖率） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | TestExecutionSubReport | 边界 | `findings` 字段缺失时通过（optional） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | TestExecutionSummaryReport | 正向 | 有效 summaryReport 通过 schema parse | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | TestExecutionSummaryReport | 正向 | `phase` 字段值为 `"test-execution"`（非旧 `"06-unit-test"`） | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | TestExecutionSummaryReport | 边界 | `conclusion` 为 `"error"` 时通过 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | TestExecutionSummaryReport | 异常 | 缺少必填字段（如 `total`）导致 parse 失败 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | 类型重命名 | 正向 | `TestExecutionSubReport` 类型可正常引用 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | 类型重命名 | 正向 | `TestExecutionSummaryReport` 类型可正常引用 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | 类型重命名 | 正向 | 旧类型名 `UnitTestSubReport` 不再通过 `schemas/index.ts` export | 新增 |

#### 测试文件: `plugins/dev-team/bin/src/lib/workflow.test.ts`（修改 — 更新所有 phase 引用）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | requirement phase index | 正向 | `requirementPhaseIndex('proposal')` 为 0 | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | requirement phase index | 正向 | `requirementPhaseIndex('implement')` 为 3 | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | requirement phase index | 正向 | `requirementPhaseIndex('test-gen')` 为 4 | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | requirement phase index | 正向 | `requirementPhaseIndex('test-execution')` 为 5（替代旧 `unit-test` 位置） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | requirement phase index | 废弃 | 旧 `requirementPhaseIndex('unit-test')` 返回 -1（不再存在于表中） | 废弃 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | requirement phase index | 废弃 | 旧 `requirementPhaseIndex('integration-test')` 返回 -1 | 废弃 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | requirement phase index | 边界 | `requirementPhaseIndex('')` 未知 phase 返回 -1 | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents | 正向 | `getDependents('proposal', 'requirement')` 返回 `['dev-design', 'test-design', 'acceptance']` | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents | 正向 | `getDependents('test-gen', 'requirement')` 返回 `['test-execution', 'code-review']`（不含 integration-test） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents | 正向 | `getDependents('implement', 'requirement')` 返回 `['test-gen', 'test-execution', 'code-review', 'acceptance']` | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents | 正向 | `getDependents('test-execution', 'requirement')` 返回 `[]`（leaf） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents | 废弃 | 旧 `getDependents('unit-test', 'requirement')` 断言删除 | 废弃 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents | 废弃 | 旧 `getDependents('integration-test', 'requirement')` 断言删除 | 废弃 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents | 边界 | `getDependents('', 'requirement')` 返回 `[]`（空 phaseId） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents | 异常 | `getDependents('99-unknown', 'requirement')` 返回 `[]`（容错） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — test-only | 正向 | test-only 包含 5 个 phase（含 `test-execution`，不含 integration-test） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — test-only | 正向 | test-only 顺序为 `[proposal, code-analyze, test-design, test-gen, test-execution]` | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — test-only | 边界 | code-analyze 的 planner/evaluator agent_type 正确 | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | PHASE_TEST_ONLY — prompt customization | 正向 | test-only proposal planner prompt 不同于 requirement（含 `coverage gaps|testing strategy`） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | PHASE_TEST_ONLY — prompt customization | 异常 | test-only 中 `test-execution` evaluator prompt 包含 `WORKFLOW_CONTEXT` 字样 | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | PHASE_TEST_ONLY — prompt customization | 边界 | test-only 中 `code-analyze` evaluator prompt 不含 `WORKFLOW_CONTEXT`（仅测试阶段含） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents — test-only extended | 正向 | `getDependents('test-design', 'test-only')` 返回 `['test-gen']` | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents — test-only extended | 正向 | `getDependents('test-gen', 'test-only')` 返回 `['test-execution']`（不含 integration-test） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents — test-only extended | 正向 | leaf phase `test-execution` 返回 `[]` | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents — test-only extended | 边界 | `getDependents('test-gen', '')` 默认 requirement 表，无报错 | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents — test-only extended | 边界 | `getDependents('test-gen', 'UNKNOWN_WORKFLOW')` 降级到 requirement 表（不区分大小写容错） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getDependents — test-only extended | 异常 | `getDependents('99-unknown', 'test-only')` 返回 `[]` | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPrerequisites — test-only | 正向 | `inferPrerequisites('test-design', 'test-only')` 返回 `['proposal', 'code-analyze']` | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPrerequisites — test-only | 正向 | `inferPrerequisites('test-gen', 'test-only')` 返回 `['test-design']` | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPrerequisites — test-only | 正向 | `inferPrerequisites('test-execution', 'test-only')` 返回 `['test-gen']`（不含 integration-test） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPrerequisites — test-only | 边界 | `inferPrerequisites('', 'test-only')` 空 phase 返回 `[]` | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPrerequisites — test-only | 边界 | `inferPrerequisites('test-execution', '')` 使用 requirement 默认表返回 `['test-gen', 'implement']` | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPrerequisites — test-only | 异常 | `inferPrerequisites('99-unknown', 'test-only')` 返回 `[]`（容错） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — regression | 正向 | bug-fix 表 6 个 phase，不含 integration-test | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — regression | 正向 | bug-fix 表顺序为 `[proposal, dev-design, implement, test-execution, code-review, acceptance]` | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — regression | 正向 | refactor 表与 requirement 一致（均为 8 个 phase） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — regression | 异常 | `getPhaseTable('INVALID')` 抛出 Error（workflowType 不存在） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | getPhaseTable — regression | 异常 | `getPhaseTable('')` 抛出 Error（空字符串非有效 workflowType） | 新增 |

#### 测试文件: `plugins/dev-team/bin/src/commands/phase-next.test.ts`（修改 — 更新所有 phase 引用）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | PHASE_TABLES | 正向 | requirement 表 8 个 phase（从 9 减少），含 `test-execution` | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | PHASE_TABLES | 正向 | requirement 表不含 `unit-test` 和 `integration-test` | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | PHASE_TABLES | 正向 | bug-fix 表 6 个 phase，不含 integration-test | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | PHASE_TABLES | 正向 | test-only 表 5 个 phase（从 6 减少），含 `test-execution` | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | PHASE_TABLES | 异常 | `getPhaseTable('UNKNOWN')` 抛出 Error | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | PHASE_TABLES | 异常 | `getPhaseTable('')` 抛出 Error | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — First Run | 正向 | 空 entries 返回 `proposal` | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — First Run | 正向 | phase_index 为 1，total_phases 为 8 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — First Run | 异常 | `change` 参数为空字符串时抛出 `Missing required parameter` Error | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Normal Progression | 正向 | 01-05 pass 后返回 `test-execution` | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Normal Progression | 正向 | 全部 8 个 phase pass 后 `done=true`（无 integration-test） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Normal Progression | 边界 | `code-review` 返回 planner:null（EVAL-ONLY） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Normal Progression | 边界 | `acceptance` 返回 planner:null（EVAL-ONLY） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Skip Passed Phases | 正向 | 01-03 pass 时跳到 implement | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Skip Passed Phases | 正向 | 全部 phase pass 时 `done=true` | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Skip Passed Phases | 异常 | `test-execution` 为 stale 时不被跳过，该 phase 被返回 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Skip Passed Phases | 边界 | pass + stale pass + 新 pass 组合下正确识别下一个未 pass phase | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Dependency-Graph Driven | 正向 | 依赖图正确驱动 next_phase 选择（无 integration-test 干扰） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — Dependency-Graph Driven | 边界 | `test-gen` pass 后但 `test-execution` 未 pass 时返回 `test-execution` | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — workflow_type | 正向 | 默认 requirement 表 total_phases=8 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — workflow_type | 正向 | bug-fix total_phases=6 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — workflow_type | 正向 | test-only total_phases=5 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — workflow_type | 边界 | bug-fix 中 `integration-test` 不再出现在 phase 列表 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — workflow_type | 异常 | test-only 不含 implement、acceptance | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — test-only First Run | 正向 | 空 entries 返回 proposal，total_phases=5 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — test-only Normal Progression | 正向 | 01-04 pass 后返回 `test-execution`（替代原 `unit-test`） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — test-only Normal Progression | 正向 | test-only 5 个 phase 全 pass 后 `done=true` | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — test-only Normal Progression | 边界 | test-only 不返回 implement 或 acceptance | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — test-only Completion | 正向 | 01-04 + test-execution 全部 pass 后 `done=true` | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — test-only Completion | 边界 | `integration-test` 不在 test-only 的输出范围内 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 正向 | 首 phase 返回 `[]` | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 正向 | 第二 phase 返回 `[proposal]` | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 正向 | 所有前序 phase 含 id 和 description | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 边界 | done 时返回 `[]` | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 边界 | error 时返回 `[]` | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 异常 | 回溯目标不在 phase 表中时返回 `invalid_backtrack_target` | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 边界 | `test-execution` 的 allowed_backtrack_phases 不含 `unit-test` 或 `integration-test` | 新增 |

#### 测试文件: `plugins/dev-team/bin/src/lib/eval-json.test.ts`（修改 — 更新所有 phase 引用）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | validateVerdict | 正向 | `'pass'` 不抛出 | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | validateVerdict | 正向 | `'fail'` 不抛出 | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | validateVerdict | 异常 | `'invalid'` 抛出 | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | validateVerdict | 边界 | skipped=true 时 `'pass'` 通过 | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | validateVerdict | 边界 | skipped=true 时 `'fail'` 抛出 | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 正向 | 使用 `phase:'test-execution'` 构建 entry 通过 | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 正向 | 包含 `backtrack_to`（字符串） | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 正向 | `backtrack_to` 为 null（undefined 时） | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 正向 | `backtrack_to` 为数组 | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 异常 | phase 为 `'integration-test'` 时 zod parse 失败（不在枚举中） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 异常 | phase 为 `'unit-test'` 时 zod parse 失败 | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 边界 | `report` 长度 > 500 字符时 zod parse 失败 | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 边界 | `checklist` 为空数组时通过 | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 正向 | dev-design stale 传播到 `test-design`, `test-gen`, `implement`, `test-execution`, `code-review`, `acceptance`（不含 integration-test） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 正向 | implement stale 传播到 `test-gen`, `test-execution`, `code-review`, `acceptance`（AC-12） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 正向 | implement 不传播到 `test-design` | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 正向 | test-design stale 传播到 `test-gen`, `test-execution`（不含 implement） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 边界 | 无 pass entry 时 no-op | 修改 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 边界 | 同一 phase 新 entry 不被标记 stale | 修改 |

#### 测试文件: `plugins/dev-team/bin/src/commands/test-execution.test.ts`（重命名自 `unit-test.test.ts`）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 正向 | 正向 | 调用 `runTestDetectFrameworks` 获取 plan | 新增 |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 正向 | 正向 | 对每个 plan entry 执行测试命令 | 新增 |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 正向 | 正向 | 子报告路径为 `reports/test-execution/<fw>.json`（非旧 `reports/unit-test/`） | 修改 |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 正向 | 正向 | summaryReport phase 字段为 `"test-execution"`（非旧 `"06-unit-test"`） | 修改 |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 异常 | 异常 | plan 为空时不执行，退出码 0 | 修改 |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 异常 | 异常 | 某 framework 执行失败不阻塞后续 framework | 修改 |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 边界 | 边界 | 相同 plan 重复执行两次结果一致（幂等性） | 修改 |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — 边界 | 边界 | `projectRoot` 为 undefined 时使用默认 project dir | 修改 |

#### 测试文件: `plugins/dev-team/bin/src/cli.test.ts`（修改 — 命令注册更新）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/cli.test.ts` | CLI 命令注册 | 正向 | CLI 注册 `test-execution` 子命令（非 `unit-test`） | 修改 |
| `plugins/dev-team/bin/src/cli.test.ts` | CLI 命令注册 | 正向 | 命令 action 调用 `runTestExecution`（非 `runUnitTest`） | 修改 |
| `plugins/dev-team/bin/src/cli.test.ts` | CLI 命令注册 | 异常 | 未注册 `unit-test` 子命令 | 废弃 |
| `plugins/dev-team/bin/src/cli.test.ts` | CLI 命令注册 | 边界 | `--project-root` 选项保留 | 修改 |

#### 测试文件: `plugins/dev-team/bin/src/lib/test-report.test.ts`（修改 — 更新 phase 引用）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 正向 | `phase` 字段从 `"06-unit-test"` 更新为 `"test-execution"` | 修改 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 正向 | `command` 字段从 `"dev-team unit-test"` 更新为 `"dev-team test-execution"` | 修改 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 正向 | 汇总报告写入 `reports/test-execution.json`（非旧 `unit-test-execution.json`） | 修改 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 边界 | 0 个 subReport 时 aggregation 正常 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 正向 | 子报告路径由调用方 `reportsDir` 控制，路径更新逻辑在调用方 | 修改 |

### Mock 策略

| 测试文件 | Mock 主体 | Mock 方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | `runTestDetectFrameworks` | `vi.mock('../../src/commands/test-detect-frameworks')` — 返回预设的 plan 数组 | 全部 describe |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | `executePlanEntry` | `vi.mock('../../src/lib/test-runner')` — 返回预设的 ExecutionResult | 全部 describe |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | `generateSubReport` / `generateSummaryReport` | `vi.mock('../../src/lib/test-report')` — 返回预设的报告对象 | 全部 describe |
| `plugins/dev-team/bin/src/commands/test-execution.test.ts` | 临时文件系统（config.json） | `fs.mkdtempSync` + `fs.writeFileSync` 创建最小 project 结构 | 全部 describe |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `fs.existsSync` / `fs.readFileSync` | `vi.mock('fs')` — 拦截文件系统调用，返回预设的 eval.json / workflow.json | 全部 describe |
| `plugins/dev-team/bin/src/cli.test.ts` | `runTestExecution` | `vi.mock('./commands/test-execution')` — mock 整个命令模块 | CLI 命令注册 describe |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | 临时文件系统 | `fs.mkdtempSync` 创建临时报告目录 | 全部 describe |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | 无（纯函数） | — | 全部 describe |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | 无（纯函数） | — | `validateVerdict`、`buildEntry`、`markPhaseStale` |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | 无（Zod schema） | — | `phaseIdSchema`、`phaseLogSchema` |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.test.ts` | 无（Zod schema） | — | `TestExecutionSubReport`、`TestExecutionSummaryReport` |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-7 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | CLI 端到端执行 — AC-7 | 调用 `runTestExecution` 时触发 `runTestDetectFrameworks` 获取 plan | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | CLI 端到端执行 — AC-7 | 对每个 framework plan entry 执行测试命令 | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | CLI 端到端执行 — AC-7 | 相同 plan 重复执行两次产生相同子报告内容（幂等性） | 新增 |
| AC-11 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | 报告路径 — AC-11 | 子报告写入 `reports/test-execution/<framework>.json`（非旧 `reports/unit-test/`） | 新增 |
| AC-11 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | 报告路径 — AC-11 | 汇总报告写入 `reports/test-execution.json`（非旧 `reports/unit-test-execution.json`） | 新增 |
| AC-11 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | 报告路径 — AC-11 | 汇总报告 `phase` 字段为 `"test-execution"`（非旧 `"06-unit-test"`） | 新增 |
| AC-11 | `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | 报告路径 — AC-11 | 汇总报告 `command` 字段为 `"dev-team test-execution"`（非旧 `"dev-team unit-test"`） | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 文件存在性验证 — AC-8 | `agents/test-execution-executor.md` 存在 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 文件存在性验证 — AC-8 | `agents/test-execution-evaluator.md` 存在 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 文件存在性验证 — AC-8 | `agents/integration-test-executor.md` 不存在 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 文件存在性验证 — AC-8 | `agents/integration-test-evaluator.md` 不存在 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 文件存在性验证 — AC-8 | `agents/unit-test-executor.md` 不存在（已重命名） | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 文件存在性验证 — AC-8 | `agents/unit-test-evaluator.md` 不存在（已重命名） | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 内容验证 — AC-8 | `test-execution-executor.md` 描述含"执行所有自动化测试（单元+集成）" | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Agent 内容验证 — AC-8 | `test-execution-evaluator.md` 不含 `unit-test` 引用 | 新增 |
| AC-9 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Skill 文件存在性验证 — AC-9 | `skills/phase-test-execution/SKILL.md` 存在 | 新增 |
| AC-9 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Skill 文件存在性验证 — AC-9 | `skills/phase-integration-test/` 目录不存在 | 新增 |
| AC-9 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Skill 文件存在性验证 — AC-9 | `skills/phase-unit-test/` 目录不存在（已重命名） | 新增 |
| AC-9 | `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | Skill 内容验证 — AC-9 | `phase-test-execution/SKILL.md` 不含 `unit-test` 和 `integration-test` 引用 | 新增 |

### Mock 策略

| 测试文件 | Mock 主体 | Mock 方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `runTestDetectFrameworks` | `vi.mock('../../src/commands/test-detect-frameworks')` — 返回预设的 plan | CLI 端到端执行、报告路径 |
| `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `executePlanEntry` | `vi.mock('../../src/lib/test-runner')` — 返回预设的 ExecutionResult | CLI 端到端执行、报告路径 |
| `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` | `generateSubReport` / `generateSummaryReport` | `vi.mock('../../src/lib/test-report')` — 返回预设的报告对象 | CLI 端到端执行、报告路径 |
| `plugins/dev-team/bin/__tests__/agent-definitions-static/agent-definitions-static.test.ts` | 文件系统（agent/skill 文件读取） | `fs.readFileSync` 直接读取（无需 mock） | Agent / Skill 文件验证 |

---

## 不可测试项

- **Agent 内容语义正确性** — `test-execution-executor.md` 和 `test-execution-evaluator.md` 的 prompt 语义（如"执行所有自动化测试（单元+集成）"）通过集成测试验证文件存在和简单文本匹配，但 prompt 的完整语义正确性依赖人工 code review。**原因**: prompt 语义验证需要领域知识判断"执行所有自动化测试"是否准确描述了合并后的执行行为。
- **`dev-team unit-test` 别名或 deprecated 提示** — 若实现选择保留 `unit-test` 作为 alias（而非直接删除），别名行为需通过手动 CLI 验证。**原因**: CLI 的 `cac` 命令注册逻辑中 alias 处理无法通过纯 unit test 验证其终端输出效果。
- **已有 eval.json 中遗留 `unit-test`/`integration-test` 条目的向后兼容性** — `hasPhasePassed` 将未知 phase ID 视为未完成（返回 false），旧 entry 被 stale 标记忽略。该行为通过 `phaseIdSchema` 的 zod 枚举变更和 `eval-json.ts` 中 `hasPhasePassed` 函数本身的容错性保障，无需特定测试用例覆盖。**原因**: 向后兼容逻辑分布在 zod schema 和 eval-json 的 hasPhasePassed 函数中，无单一测试入口。
- **外部脚本或 CI/CD 调用 `dev-team unit-test` 的中断** — `dev-team unit-test` 从 CLI 中删除后，外部脚本将收到 command not found 错误。此行为是预期的 breaking change，不需要自动化验证。**原因**: 外部脚本不在本项目的测试范围内。
- **`unit-test-output.schema.ts` 中 export 的类型重命名兼容性** — 类型名 `UnitTestSubReport` → `TestExecutionSubReport` 等变更是 TypeScript 编译时类型变更，编译通过即表明正确。**原因**: 编译时验证由 TypeScript 编译器覆盖，无需运行时测试。
