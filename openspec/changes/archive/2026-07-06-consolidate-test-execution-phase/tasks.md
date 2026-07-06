# 任务: consolidate-test-execution-phase

> **变更**: consolidate-test-execution-phase
> **日期**: 2026-07-03

---

## 阶段 1: Schema 和数据模型变更

这些变更定义新的 phase ID 枚举，是所有下游工作的基础。

- [x] **1.1** 修改 `plugins/dev-team/bin/src/schemas/phase-log.schema.ts`：在 `phaseIdSchema` 的 `z.enum([...])` 中将 `'unit-test'` 改为 `'test-execution'`，删除 `'integration-test'`
- [x] **1.2** 修改 `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` 并重命名为 `test-execution-output.schema.ts`：
  - 将文件重命名为 `test-execution-output.schema.ts`
  - 更新所有类型名中的 `UnitTest` 为 `TestExecution`（如 `UnitTestSubReport` → `TestExecutionSubReport`）
  - 更新 `unitTestSummaryReportSchema` 中的 `phase` 默认值 `'06-unit-test'` → `'test-execution'`
  - 更新描述文字中所有 `unit-test` 引用为 `test-execution`
- [x] **1.3** 更新 `plugins/dev-team/bin/src/schemas/index.ts`：
  - 将 `unit-test-output.schema` 的 export 路径改为 `./test-execution-output.schema`
  - 更新所有 `UnitTest*` 类型名为 `TestExecution*`

## 阶段 2: CLI 命令重命名

命令重命名依赖 schema 变更（输出 schema 已在 1.2 更新）。

- [x] **2.1** 将 `plugins/dev-team/bin/src/commands/unit-test.ts` 重命名为 `test-execution.ts`：
  - 重命名函数 `runUnitTest` → `runTestExecution`
  - 重命名接口 `UnitTestOptions` → `TestExecutionOptions`
  - 更新 resolveReportsDir 中 `reports/unit-test` → `reports/test-execution`
- [x] **2.2** 修改 `plugins/dev-team/bin/src/cli.ts`：
  - 将 `import { runUnitTest } from './commands/unit-test'` 改为 `import { runTestExecution } from './commands/test-execution'`
  - 将 `cli.command('unit-test', 'Run unit tests...')` 改为 `cli.command('test-execution', 'Run all automated tests (unit + integration)...')`
  - 将 `action` 回调中的 `runUnitTest({...})` 改为 `runTestExecution({...})`
- [x] **2.3** 更新 `plugins/dev-team/bin/src/lib/test-report.ts`：
  - 修改 `phase: '06-unit-test'` → `phase: 'test-execution'`
  - 修改 `command: 'dev-team unit-test'` → `command: 'dev-team test-execution'`
  - 修改汇总报告输出路径 `'..', 'unit-test-execution.json'` → `'..', 'test-execution.json'`

## 阶段 3: 工作流核心逻辑变更

这些变更依赖 phaseIdSchema 枚举已更新（1.1）。

- [x] **3.1** 修改 `plugins/dev-team/bin/src/lib/workflow.ts`：
  - **PHASE_REQUIREMENT**：将 `unit-test` 条目重命名为 `test-execution`，更新 description/agent_type/prompt 中的 `unit-test` 引用；删除整个 `integration-test` 条目
  - **PHASE_BUG_FIX**：将 `unit-test` 条目重命名为 `test-execution`，更新 agent_type/prompt；删除 `integration-test`（bug-fix 本无 integration-test，确认是否需删除无此条的引用或注释）
  - **PHASE_TEST_ONLY**：将 `unit-test` 条目重命名为 `test-execution`；删除整个 `integration-test` 条目
  - **PHASE_PREREQUISITES**（requirement）：`'unit-test'` → `'test-execution'`；删除 `'integration-test': [...]` 行
  - **PHASE_BUG_FIX_PREREQUISITES**：`'unit-test'` → `'test-execution'`；删除 `'integration-test'` 相关行（如果有）
  - **PHASE_TEST_ONLY_PREREQUISITES**：`'unit-test'` → `'test-execution'`；删除 `'integration-test': ['test-gen']` 行

## 阶段 4: Agent 定义变更

依赖 schema 枚举和工作流表已更新。

- [x] **4.1** 将 `plugins/dev-team/agents/unit-test-executor.md` 重命名为 `test-execution-executor.md`：
  - 更新 frontmatter `name: unit-test-executor` → `name: test-execution-executor`
  - 更新 description 为 `【use proactively】Reads the CLI-generated test execution report...`
  - 将 prompt 和流程中所有 `unit-test` 引用改为 `test-execution`
  - 更新 CLI 命令引用 `dev-team unit-test` → `dev-team test-execution`
  - 更新报告路径 `reports/unit-test-execution.json` → `reports/test-execution.json`，`reports/unit-test/` → `reports/test-execution/`
  - 更新诊断决策树：现在处理所有测试（单元+集成），不再仅限定"单元测试"
- [x] **4.2** 将 `plugins/dev-team/agents/unit-test-evaluator.md` 重命名为 `test-execution-evaluator.md`：
  - 更新 frontmatter `name: unit-test-evaluator` → `name: test-execution-evaluator`
  - 更新 description 为 `Reads the test execution report...`
  - 将 prompt 和流程中所有 `unit-test` 引用改为 `test-execution`
  - 更新报告路径引用
  - 更新 checklist 条目描述：`U1`～`U4` → `T1`～`T4`（可选，原 ID 可保留）
  - 更新决策树逻辑：合并单元测试和集成测试的诊断规则为单一决策树
- [x] **4.3** 删除 `plugins/dev-team/agents/integration-test-executor.md`
- [x] **4.4** 删除 `plugins/dev-team/agents/integration-test-evaluator.md`

## 阶段 5: Skill 定义变更

依赖 Agent 定义已更新（4.1~4.4）。

- [x] **5.1** 将 `plugins/dev-team/skills/phase-unit-test/SKILL.md` 重命名为 `skills/phase-test-execution/SKILL.md`：
  - 更新 frontmatter `name: phase-unit-test` → `name: phase-test-execution`
  - 更新 description 中 agent 引用 `unit-test-executor` → `test-execution-executor`，`unit-test-evaluator` → `test-execution-evaluator`
  - 更新 all phase ID references `unit-test` → `test-execution`
  - 更新无操作检测：检查所有测试文件（不仅是 `*.test.ts`，也包括集成测试文件 `__tests__/`、`tests/integration/` 等）
  - 更新 Executor→Evaluator 循环中的 agent_type 引用
  - 更新 phase_log 调用中的 `phase: "unit-test"` → `phase: "test-execution"`
  - 更新 CLI 命令 `dev-team unit-test` → `dev-team test-execution`
  - 更新 usage 命令 `/dev-team:phase-unit-test` → `/dev-team:phase-test-execution`
- [x] **5.2** 删除 `plugins/dev-team/skills/phase-integration-test/` 整个目录
- [x] **5.3** 更新 `plugins/dev-team/skills/workflow-requirement/SKILL.md`：
  - 注释 `all 9 phases` → `all 8 phases`（无逻辑变更，纯注释修正）
- [x] **5.4** 更新 `plugins/dev-team/skills/workflow-test-only/SKILL.md`：
  - 注释 `executes 6 phases` → `executes 5 phases`
  - 更新 AskUserQuestion 提示：`proceed to integration-test` → `proceed to next phase`

## 阶段 6: 测试文件更新

测试文件的变更必须在所有源代码变更后执行，以确保测试通过。

- [x] **6.1** 将 `plugins/dev-team/bin/src/commands/unit-test.test.ts` 重命名为 `test-execution.test.ts`：
  - 更新 import 路径引用（`../commands/unit-test` → `../commands/test-execution`）
  - 更新所有 phase 引用
- [x] **6.2** 更新 `plugins/dev-team/bin/src/lib/workflow.test.ts`：
  - 更新 requirement 阶段索引测试：`unit-test` → `test-execution`，删除 `integration-test` 行；requirement 表长度隐含从 9 变为 8
  - 更新 `getDependents` 测试：`unit-test` → `test-execution`；删除所有 `integration-test` 相关断言；如 `getDependents('test-gen', 'requirement')` 从 `['unit-test', 'code-review', 'integration-test']` → `['test-execution', 'code-review']`；`getDependents('implement', 'requirement')` 删除 `integration-test`
  - 更新 test-only 表测试：预期长度 6 → 5；phase ID 列表移除 `integration-test`
  - 更新 `WORKFLOW_CONTEXT` 测试：`unit-test` → `test-execution`
  - 更新 test-only `getDependents` 测试
- [x] **6.3** 更新 `plugins/dev-team/bin/src/lib/eval-json.test.ts`：
  - 更新 `buildEntry` 测试中的 `phase: 'unit-test'` → `phase: 'test-execution'`
  - 更新 `markPhaseStale` 传播测试：phase 列表中 `unit-test` → `test-execution`，删除 `integration-test`
  - 更新所有传播断言中的 `integration-test` 引用
- [x] **6.4** 更新 `plugins/dev-team/bin/src/commands/phase-next.test.ts`：
  - 更新 requirement 表长度断言 9 → 8
  - 更新 phase ID 顺序的 `toEqual` 数组：`unit-test` → `test-execution`，删除 `integration-test`
  - 更新 test-only 表长度断言 6 → 5
  - 更新所有测试用例中 `passEntry('unit-test')` → `passEntry('test-execution')`
  - 删除所有 `passEntry('integration-test')`
  - 更新 done 检测测试用例：删除 `integration-test` entry
  - 更新批量 phase 列表常量（如 `testPhases`、`phaseIds`）
  - 更新 `allowed_backtrack_phases` 测试中的 phase 顺序

## 阶段 7: 验证

所有变更完成后执行。

- [x] **7.1** 运行类型检查：确保 `npx tsc --noEmit` 通过（或项目配置的类型检查命令）
- [x] **7.2** 运行所有单元测试：`npx vitest run plugins/dev-team/bin/src/commands/test-execution.test.ts plugins/dev-team/bin/src/lib/workflow.test.ts plugins/dev-team/bin/src/lib/eval-json.test.ts plugins/dev-team/bin/src/commands/phase-next.test.ts`
- [x] **7.3** 验证无遗漏的 `unit-test` 或 `integration-test` 引用：`grep -r "unit-test" --include="*.ts" --include="*.md" plugins/dev-team/`（排除 `node_modules` 和变更目录本身）
