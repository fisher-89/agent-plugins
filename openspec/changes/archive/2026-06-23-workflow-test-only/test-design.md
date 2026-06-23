# 测试设计: workflow-test-only

> **日期**: 2026-06-22

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | change 的 `workflow.json` 含 `workflow_type: test-only` 时，`phase_next(change)` 在无 eval 条目时返回 `01-proposal` | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only First Run` |
| AC-2 | test-only 六阶段按 prerequisite DAG 顺序推进：`01→02→03→04→06/08` | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` |
| AC-3 | `02-code-analyze` 使用 `code-analyze-planner` + `code-analyze-evaluator`，产出 `design.md` 且不要求 `tasks.md` | 集成测试 | `plugins/dev-team/bin/__tests__/code-analyze agent definitions/code-analyze agent definitions.test.ts` | `code-analyze agent definitions — planner/evaluator 静态定义` |
| AC-4 | `03-test-design` 在 `02-code-analyze` pass 后可执行，planner 能读取 design.md | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Gate (02-code-analyze)` |
| AC-5 | `getDependents("01-proposal", "test-only")` 返回 `[02-code-analyze, 03-test-design]` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents — test-only` |
| AC-6 | `getDependents("02-code-analyze", "test-only")` 返回 `[03-test-design]` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents — test-only` |
| AC-7 | test-only workflow 全部 leaf phase pass 后 `phase_next` 返回 `done: true`（无 09-acceptance） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Completion` |
| AC-8 | `/dev-team:workflow-test-only <change>` 可完整执行六阶段循环；scaffold 写入 `workflow.json` | 集成测试 | `plugins/dev-team/bin/__tests__/workflow-test-only skill scaffold/workflow-test-only skill scaffold.test.ts` | `workflow-test-only skill — scaffold 与循环定义` |
| AC-9 | `01-proposal` prompt 引导测试覆盖缺口，与 requirement workflow 的 proposal prompt 有差异 | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_TEST_ONLY — prompt customization` |
| AC-10 | requirement / bug-fix / refactor workflow 行为不受影响（回归） | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` / `getPhaseTable` 回归 |
| AC-10 | requirement / bug-fix / refactor workflow 行为不受影响（回归） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow.json default` / 现有 workflow 回归用例 |
| AC-11 | `workflow.json` 为 `test-only` 时，`phase_log` 拒绝 `backtrack_to: "05-implement"`，不写入 eval.json | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` |
| AC-12 | 拒绝错误信息包含 workflow_type 与可用 phase 列表 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` |
| AC-13 | `workflow.json` 缺省或不存在时，`phase_next` / `phase_log` 使用 `"requirement"` 阶段表 | 单元测试 | `plugins/dev-team/bin/src/lib/change-config.test.ts` | `getWorkflowType — default requirement` |
| AC-13 | `workflow.json` 缺省或不存在时，`phase_next` / `phase_log` 使用 `"requirement"` 阶段表 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow.json default` |
| AC-13 | `workflow.json` 缺省或不存在时，`phase_next` / `phase_log` 使用 `"requirement"` 阶段表 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — default requirement workflow` |
| AC-14 | `PHASE_TEST_ONLY` 的 `06-unit-test` / `08-integration-test` evaluator prompt 含 WORKFLOW_CONTEXT 自适应指令 | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_TEST_ONLY — WORKFLOW_CONTEXT prompts` |
| AC-15 | workflow-test-only skill 在测试发现代码 bug 后生成 `reports/` bug 报告并通知用户 | 集成测试 | `plugins/dev-team/bin/__tests__/workflow-test-only skill scaffold/workflow-test-only skill scaffold.test.ts` | `workflow-test-only skill — bug 发现报告流程` |
| AC-16 | evaluator 自适应流程：`phase_log` 拒绝无效 backtrack 后以 `fail` + `backtrack_to: null` 重试写入 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — adaptive fail after invalid backtrack` |
| AC-16 | evaluator 自适应流程：`phase_log` 拒绝无效 backtrack 后以 `fail` + `backtrack_to: null` 重试写入 | 集成测试 | `plugins/dev-team/bin/__tests__/phase_log workflow-aware backtrack rejection/phase_log workflow-aware backtrack rejection.test.ts` | `phase_log — invalid backtrack 不污染 eval.json 后 fail 写入` |
| AC-17 | `phase_next` 与 `phase_log` MCP input schema 不含 `workflow_type` 字段 | 单元测试 | `plugins/dev-team/bin/src/schemas/phase-next.schema.test.ts` | `phaseNextInputSchema — no workflow_type` |
| AC-17 | `phase_next` 与 `phase_log` MCP input schema 不含 `workflow_type` 字段 | 单元测试 | `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogInputSchema — no workflow_type` |
| AC-17 | `phase_next` 与 `phase_log` MCP input schema 不含 `workflow_type` 字段 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | `registerPhaseNextTool / registerPhaseLogTool — input schema` |
| AC-18 | `proposal-planner` 在 `workflow.json` 无 `workflow_type` 时向用户确认并写入后再写 proposal | 集成测试 | `plugins/dev-team/bin/__tests__/code-analyze agent definitions/code-analyze agent definitions.test.ts` | `proposal-planner — workflow.json 确认步骤` |
| AC-19 | `workflow-test-only` Step 1 scaffold 后 `workflow.json` 含 `{"workflow_type": "test-only"}` | 集成测试 | `plugins/dev-team/bin/__tests__/workflow-test-only skill scaffold/workflow-test-only skill scaffold.test.ts` | `workflow-test-only skill — Step 1 workflow.json` |
| AC-13 | `readWorkflowConfig` 解析 workflow.json 各种格式（正向/异常） | 单元测试 | `plugins/dev-team/bin/src/lib/change-config.test.ts` | `readWorkflowConfig` |
| AC-2 | `getPhaseTable` 返回 test-only 六阶段表结构与顺序 | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` |
| AC-5/6 | `getPrerequisites` 返回 test-only 各 phase 的前置依赖 | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` |
| AC-2 | `PHASE_TABLES` test-only 表含 6 个 phase | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` |
| AC-11 | test-only 下 backtrack_to 指向表内合法 phase 仍正常推进 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` |
| AC-9 | test-only change name 含特殊字符时 prompt 仍包含 change name | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` |
| AC-10 | pass entry 不触发 markPhaseStale — 回归现有行为 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — pass entry no propagation` |
| AC-11 | test-only 表内合法 backtrack 仍触发 markPhaseStale | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — backtrack_to triggers markPhaseStale` |
| AC-13 | backtrack_to 边界值（空字符串/null）输入校验 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — input validation` |
| AC-16 | `phase_log` 重复调用幂等性：相同 pass 输入连续调用产出一致 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — idempotency` |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/change-config.test.ts` | `readWorkflowConfig` | 正向 | should parse workflow.json with workflow_type test-only | 新增 |
| `plugins/dev-team/bin/src/lib/change-config.test.ts` | `getWorkflowType` | 正向 | should return test-only when workflow.json contains test-only | 新增 |
| `plugins/dev-team/bin/src/lib/change-config.test.ts` | `getWorkflowType` | 正向 | should return requirement when workflow.json contains requirement | 新增 |
| `plugins/dev-team/bin/src/lib/change-config.test.ts` | `getWorkflowType` | 边界 | should return requirement when workflow.json file does not exist（AC-13） | 新增 |
| `plugins/dev-team/bin/src/lib/change-config.test.ts` | `getWorkflowType` | 边界 | should return requirement when workflow.json exists but workflow_type key is missing | 新增 |
| `plugins/dev-team/bin/src/lib/change-config.test.ts` | `getWorkflowType` | 边界 | should return requirement when workflow_type is empty string "" | 新增 |
| `plugins/dev-team/bin/src/lib/change-config.test.ts` | `readWorkflowConfig` | 异常 | should throw when workflow.json contains invalid JSON | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` | 正向 | should have exactly 6 phases for test-only workflow_type | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` | 正向 | test-only phase ids 顺序为 01-proposal → 02-code-analyze → 03-test-design → 04-test-gen → 06-unit-test → 08-integration-test | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` | 正向 | test-only phase table 不含 02-dev-design / 05-implement / 07-code-review / 09-acceptance | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` | 正向 | 02-code-analyze 使用 code-analyze-planner 与 code-analyze-evaluator agent_type | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | should return [02-code-analyze, 03-test-design] for 01-proposal test-only（AC-5） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | should return [03-test-design] for 02-code-analyze test-only（AC-6） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | should return [04-test-gen] for 03-test-design test-only | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | should return [06-unit-test, 08-integration-test] for 04-test-gen test-only | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 边界 | should return [] for leaf phases 06-unit-test and 08-integration-test test-only | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 边界 | should return [] for unknown phase 99-unknown test-only | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 正向 | should return [01-proposal, 02-code-analyze] for 03-test-design test-only | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 正向 | should return [03-test-design] for 04-test-gen test-only | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 正向 | should return [04-test-gen] for 06-unit-test and 08-integration-test test-only | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_TEST_ONLY — prompt customization` | 正向 | 01-proposal planner prompt 含测试覆盖缺口 / 测试策略引导，且与 requirement 01-proposal prompt 不同（AC-9） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_TEST_ONLY — WORKFLOW_CONTEXT prompts` | 正向 | 06-unit-test evaluator prompt 含 WORKFLOW_CONTEXT 自适应指令（AC-14） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_TEST_ONLY — WORKFLOW_CONTEXT prompts` | 正向 | 08-integration-test evaluator prompt 含 WORKFLOW_CONTEXT 自适应指令（AC-14） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | requirement getDependents 行为与变更前一致 — 01-proposal 仍返回 [02-dev-design, 03-test-design, 09-acceptance]（AC-10） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` | 正向 | bug-fix / refactor phase 表长度与 id 列表不变（AC-10） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 正向 | should have 6 phases for test-only workflow_type | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only First Run` | 正向 | should return 01-proposal when workflow.json is test-only and eval.json empty（AC-1） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only First Run` | 正向 | should set total_phases to 6 for test-only | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | should return 02-code-analyze after 01-proposal passes（AC-2） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | should return 03-test-design after 01-proposal and 02-code-analyze pass（AC-2） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | should return 04-test-gen after 01-03 pass（AC-2） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | should return 06-unit-test after 01-04 pass（AC-2） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | should return 08-integration-test when 06 pass and 08 not passed（AC-2 并行 leaf） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Gate (02-code-analyze)` | 正向 | should return 02-code-analyze when 01 pass but 02 not passed — 03 不可提前执行（AC-4） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Gate (02-code-analyze)` | 正向 | should return 03-test-design when 01-02 pass — gate satisfied（AC-4） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Completion` | 正向 | should return done=true when 01-04, 06, 08 all pass — 无 09-acceptance（AC-7） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Completion` | 正向 | should never return 05-implement or 09-acceptance for test-only | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow.json default` | 边界 | should use requirement table when workflow.json missing（AC-13） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow.json default` | 边界 | should use requirement table when workflow.json lacks workflow_type | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 废弃 | should follow test-only when workflow_type MCP param is test-only — 改为读 workflow.json | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | requirement / bug-fix / refactor 现有用例全部通过（AC-10 回归） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` | 边界 | test-only 下 backtrack_to 指向表内 phase（如 01-proposal）仍正常返回目标 phase | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 边界 | test-only change name 含特殊字符时 planner prompt 仍包含 change name | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 异常 | should throw and NOT call appendEntry when test-only backtrack_to is 05-implement（AC-11） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 异常 | should throw and NOT call appendEntry when test-only backtrack_to is 02-dev-design | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 异常 | error message 含 workflow_type test-only 与可用 phase 列表（AC-12） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — default requirement workflow` | 边界 | should allow backtrack_to 05-implement when workflow.json missing（requirement 缺省，AC-13） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — adaptive fail after invalid backtrack` | 正向 | invalid backtrack 抛错后，以 verdict fail + backtrack_to null 调用应成功写入（AC-16） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — pass entry no propagation` | 正向 | pass entry 不触发 markPhaseStale — 回归现有行为 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — backtrack_to triggers markPhaseStale` | 正向 | test-only 表内合法 backtrack（如 03-test-design → 01-proposal）仍触发 markPhaseStale | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — input validation` | 边界 | backtrack_to 为空字符串时不触发 backtrack 校验 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — input validation` | 边界 | backtrack_to 为 null 时正常写入 fail entry | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — idempotency` | 正向 | 相同 phase + verdict pass 连续调用两次，appendEntry 被调用两次且每次入参一致，无副作用累积（AC-16） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — idempotency` | 正向 | 相同 phase + verdict fail + backtrack_to null 连续调用两次，行为一致无异常 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — idempotency` | 异常 | 相同无效 backtrack 连续调用两次，均抛错且 appendEntry 调用次数为 0 | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-next.schema.test.ts` | `phaseNextInputSchema` | 正向 | parsed object keys 仅含 change，不含 workflow_type（AC-17） | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-next.schema.test.ts` | `phaseNextInputSchema` | 边界 | 传入多余 workflow_type 字段时 Zod 拒绝或 strip 行为与 schema 定义一致 | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-next.schema.test.ts` | `phaseNextInputSchema` | 异常 | change 为空字符串时应校验失败 | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogInputSchema` | 正向 | phaseLogInputSchema keys 不含 workflow_type（AC-17） | 新增 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogInputSchema` | 边界 | 传入多余 workflow_type 字段时 Zod 拒绝或 strip 行为与 schema 定义一致 | 新增 |
| `plugins/dev-team/bin/src/mcp.test.ts` | `registerPhaseNextTool` | 正向 | phase_next tool inputSchema 不含 workflow_type 属性（AC-17） | 新增 |
| `plugins/dev-team/bin/src/mcp.test.ts` | `registerPhaseLogTool` | 正向 | phase_log tool inputSchema 不含 workflow_type 属性（AC-17） | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/lib/change-config.test.ts` | `fs`（workflow.json 读写） | `vi.mock('fs')`：`existsSync` / `readFileSync` 按用例返回 JSON 或 false；`getChangeDir` mock 为固定临时目录 | 全部 `readWorkflowConfig` / `getWorkflowType` |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | 无外部依赖 | 直接 import `workflow.ts` 导出，纯函数断言 phase 表与 prompt 字符串 | 全部 `describe` |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `eval.json` + `workflow.json` | 扩展现有 `next()` helper：`readFileSync` 按路径区分 eval.json 与 workflow.json；`existsSync` 分别控制；**移除** `workflow_type` MCP 参数，改由 mock workflow.json 驱动 | `runPhaseNext — test-only *`、`PHASE_TABLES`、回归用例 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `eval-json` 模块 + `change-config` + `fs` | 沿用 parallel-dev-test-tracks 模式：`vi.mock` `readEvalJson` / `appendEntry` / `markPhaseStale`；mock `getWorkflowType` 返回 test-only 或 requirement；验证 `appendEntry` 调用次数 | `runPhaseLog — workflow-aware backtrack rejection`、`adaptive fail` |
| `plugins/dev-team/bin/src/schemas/phase-next.schema.test.ts` | 无 | 对 Zod schema 做 `safeParse` / shape 断言 | `phaseNextInputSchema` 校验用例 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | 无 | 对 Zod schema 做 `safeParse` / shape 断言 | `phaseLogInputSchema` 校验用例 |
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP Server 注册 | import `registerPhaseNextTool` / `registerPhaseLogTool`，检查注册时 `inputSchema` 字段列表 | AC-17 |

---

## 集成测试

跨模块静态定义与 skill 文件结构验证；`phase_log` 与 `eval.json` 协作的端到端场景在 `__tests__/` 中补充。

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-3 | `plugins/dev-team/bin/__tests__/code-analyze agent definitions/code-analyze agent definitions.test.ts` | `code-analyze-planner.md` 静态定义 | Process 要求读取 proposal.md 与 design.md.template；Output 仅 design.md，不含 tasks.md | 新增 |
| AC-3 | `plugins/dev-team/bin/__tests__/code-analyze agent definitions/code-analyze agent definitions.test.ts` | `code-analyze-evaluator.md` 静态定义 | checklist 评估 design.md 完整性，不要求 tasks.md；调用 phase_log phase=02-code-analyze | 新增 |
| AC-18 | `plugins/dev-team/bin/__tests__/code-analyze agent definitions/code-analyze agent definitions.test.ts` | `proposal-planner.md — workflow.json 确认` | Process 含读取 workflow.json、缺省时 AskQuestion 并写入 workflow_type 步骤 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/workflow-test-only skill scaffold/workflow-test-only skill scaffold.test.ts` | `workflow-test-only/SKILL.md` 编排结构 | Step 1 scaffold 后写入 workflow.json；Step 2 循环仅传 change 给 phase_next | 新增 |
| AC-19 | `plugins/dev-team/bin/__tests__/workflow-test-only skill scaffold/workflow-test-only skill scaffold.test.ts` | `workflow-test-only/SKILL.md — Step 1` | Step 1 明确写入 `{"workflow_type": "test-only"}` | 新增 |
| AC-15 | `plugins/dev-team/bin/__tests__/workflow-test-only skill scaffold/workflow-test-only skill scaffold.test.ts` | `workflow-test-only/SKILL.md — bug 报告` | 06/08 fail 且发现代码 bug 时写入 reports/code-bugs-found.md 并通知用户 | 新增 |
| AC-16 | `plugins/dev-team/bin/__tests__/phase_log workflow-aware backtrack rejection/phase_log workflow-aware backtrack rejection.test.ts` | `phase_log — eval.json 不被无效 backtrack 污染` | 模拟 test-only change：第一次 backtrack_to=05-implement 抛错且 eval 未变；第二次 fail+null 成功 append | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/code-analyze agent definitions/code-analyze agent definitions.test.ts` | agent `.md` 文件 | `fs.readFileSync` 读取 `plugins/dev-team/agents/*.md`，断言 Process/Output/Constraints 章节内容 | AC-3、AC-18 |
| `plugins/dev-team/bin/__tests__/workflow-test-only skill scaffold/workflow-test-only skill scaffold.test.ts` | `SKILL.md` 文件 | 读取 `plugins/dev-team/skills/workflow-test-only/SKILL.md` 与 `workflow-requirement/SKILL.md`，对比 Step 1 workflow.json 写入与 MCP 调用参数 | AC-8、AC-15、AC-19 |
| `plugins/dev-team/bin/__tests__/phase_log workflow-aware backtrack rejection/phase_log workflow-aware backtrack rejection.test.ts` | `eval-json` + `workflow.json` | 临时目录或 mock：`readEvalJson` 返回初始 entries；连续两次 `runPhaseLog` 验证 eval 条目数量与内容 | AC-16 端到端 |

---

## 不可测试项

- **AC-8 完整六阶段 Claude Code 循环执行** — **原因**: `/dev-team:workflow-test-only` 依赖 Claude Code 运行时、OpenSpec CLI scaffold 与多轮 agent 对话，无法在 Vitest 中自动化。集成测试仅验证 `SKILL.md` 静态结构与 scaffold 指令；完整流程需手动或 staging E2E。

- **AC-3 design.md 产出质量与 test-design-planner 兼容性** — **原因**: `code-analyze-planner` 产出内容由 LLM 决定，单元/集成测试仅能验证 agent 定义约束（章节、不含 tasks.md），无法确定性断言逆向分析覆盖全部模块。evaluator checklist 与手动 review 保障。

- **AC-16 evaluator agent 运行时自适应决策** — **原因**: WORKFLOW_CONTEXT 重试逻辑由 LLM 在收到 MCP 错误后执行；引擎层可通过 `phase-log.test.ts` 验证拒绝与 fail 写入，但无法强制 agent 一定遵循 prompt 重试路径。prompt 审查 + 引擎硬拒绝作为双重保障。

- **AC-18 AskQuestion 用户交互路径** — **原因**: `proposal-planner` 的 AskQuestion 由 Claude Code 工具运行时处理，集成测试仅验证 Process 步骤文本存在，无法模拟用户选择。

- **`plugin.json` 版本号递增** — **原因**: 元数据变更，无运行时行为断言；code review 验证。

- **旧 change 无 workflow.json 的存量 eval.json 迁移** — **原因**: proposal 明确不迁移历史 eval.json；缺省 requirement 行为由 `change-config.test.ts` 与回归用例覆盖，真实存量 change 兼容性需文档约定与抽样手动验证。

- **`test_resolve_paths` 无法解析的源文件** — **原因**: MCP 返回 `Not a testable source file`，无法生成 colocated 单元测试路径：
  - `plugins/dev-team/agents/code-analyze-planner.md`
  - `plugins/dev-team/agents/code-analyze-evaluator.md`
  - `plugins/dev-team/agents/proposal-planner.md`
  - `plugins/dev-team/skills/workflow-test-only/SKILL.md`
  - `plugins/dev-team/skills/workflow-requirement/SKILL.md`
  
  对应验证通过 `plugins/dev-team/bin/__tests__/` 集成测试读取文件内容完成。
