# 设计: consolidate-test-execution-phase

> **变更**: consolidate-test-execution-phase
> **日期**: 2026-07-03

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| phaseIdSchema | 定义所有有效 phase ID 的 zod 枚举 | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | zod/v4 | TypeScript |
| workflow.ts | 定义所有工作流的 phase 表、前置依赖表和访问函数 | `plugins/dev-team/bin/src/lib/workflow.ts` | phaseIdSchema | TypeScript |
| CLI 入口 | 注册 CLI 子命令，派发到命令处理函数 | `plugins/dev-team/bin/src/cli.ts` | test-execution command | TypeScript (cac) |
| test-execution 命令 | 执行所有测试、生成子报告和汇总报告 | `plugins/dev-team/bin/src/commands/test-execution.ts` | test-parser, test-runner, test-report, test-detect-frameworks | TypeScript |
| test-report | 生成 per-framework 子报告和汇总 JSON 报告 | `plugins/dev-team/bin/src/lib/test-report.ts` | config, glob, unit-test-output.schema (即将重命名) | TypeScript |
| eval-json | eval.json 的读写、stale 标记和传播 | `plugins/dev-team/bin/src/lib/eval-json.ts` | workflow.getDependents, phase-log.schema | TypeScript |
| phase-next | 决定下一个要执行的 phase | `plugins/dev-team/bin/src/commands/phase-next.ts` | workflow, eval-json | TypeScript |
| phase-log.schema | phase_log MCP 工具的输入/输出 schema | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | zod/v4 | TypeScript |
| test-execution-output.schema | 测试执行报告的 JSON schema | `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` | zod/v4 | TypeScript |
| test-execution-executor agent | 读取 CLI 报告、验证完整性、添加诊断发现 | `plugins/dev-team/agents/test-execution-executor.md` | CLI 生成的报告 | Markdown agent def |
| test-execution-evaluator agent | 验证报告、应用诊断决策树、写入 eval.json | `plugins/dev-team/agents/test-execution-evaluator.md` | Executor 报告 | Markdown agent def |
| phase-test-execution skill | 编排 test-execution 阶段：无操作检测、Executor→Evaluator 循环 | `plugins/dev-team/skills/phase-test-execution/SKILL.md` | test-execution-executor, test-execution-evaluator | Markdown skill def |
| workflow-requirement skill | 完整 PGE 工作流编排器（requirement） | `plugins/dev-team/skills/workflow-requirement/SKILL.md` | phase_next MCP | Markdown skill def |
| workflow-test-only skill | 纯测试 PGE 工作流编排器 | `plugins/dev-team/skills/workflow-test-only/SKILL.md` | phase_next MCP | Markdown skill def |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
-->

### 新增文件

<!-- 本变更有 3 种类型的新增文件：Skill 重命名后新目录、Agent 重命名后新文件、命令重命名后新文件。新增文件均在后续子表中有对应条目。 -->

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/skills/phase-test-execution/SKILL.md` | phase-unit-test SKILL.md 重命名而来，所有 phase 引用从 `unit-test` 更新为 `test-execution` |
| `plugins/dev-team/agents/test-execution-executor.md` | unit-test-executor.md 重命名而来，描述和 prompt 更新为"执行所有自动化测试（单元+集成）" |
| `plugins/dev-team/agents/test-execution-evaluator.md` | unit-test-evaluator.md 重命名而来，phase 引用更新为 `test-execution` |
| `plugins/dev-team/bin/src/commands/test-execution.ts` | unit-test.ts 重命名而来，所有 `unit-test` 引用更新为 `test-execution` |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` | unit-test-output.schema.ts 重命名而来，phase/command 字符串更新 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | phaseIdSchema 枚举值：`'unit-test'` → `'test-execution'`；删除 `'integration-test'` | AC-1 |
| `plugins/dev-team/bin/src/schemas/index.ts` | 更新 export：`unit-test-output.schema` → `test-execution-output.schema` | 保持暴露路径一致 |
| `plugins/dev-team/bin/src/lib/workflow.ts` | 三个 PHASE_TABLES 中：`unit-test` → `test-execution`，删除所有 `integration-test` 条目；四个 PREREQUISITES_TABLES 中：`'unit-test'` → `'test-execution'`，删除所有 `'integration-test'` 行 | AC-2~AC-6 |
| `plugins/dev-team/bin/src/cli.ts` | import `runUnitTest` → `runTestExecution`（来自 `./commands/test-execution`）；命令注册 `unit-test` → `test-execution` | AC-7 |
| `plugins/dev-team/bin/src/lib/test-report.ts` | `phase: '06-unit-test'` → `phase: 'test-execution'`；`command: 'dev-team unit-test'` → `command: 'dev-team test-execution'`；报告路径 `unit-test-execution.json` → `test-execution.json` | AC-11 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` → `plugins/dev-team/bin/src/commands/test-execution.test.ts` | 文件名重命名；import 路径更新；phase 引用更新 | 测试文件随命令重命名 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | 所有 `unit-test` 引用 → `test-execution`；删除所有 `integration-test` 相关断言 | AC-10 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | 所有 `unit-test` 引用 → `test-execution`；删除 `integration-test` 相关断言；更新 stale propagation 测试中的 phase 列表 | AC-10, AC-12 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | 所有 `unit-test` 引用 → `test-execution`；删除所有 `integration-test` 引用；表长度断言 9→8（requirement）、6→5（test-only）；更新 done/gate 测试用例 | AC-10 |
| `plugins/dev-team/skills/workflow-requirement/SKILL.md` | 注释 `9 phases` → `8 phases`（无其他硬编码 phase 引用，workflow-requirement 基于 phase_next 动态编排，无需修改逻辑） | 纯注释修正 |
| `plugins/dev-team/skills/workflow-test-only/SKILL.md` | 注释 `6 phases` → `5 phases`；AskUserQuestion 提示 `proceed to integration-test` → `proceed to next phase` | 删除 integration-test 后的文案修正 |

### 删除文件

<!-- 删除的文件包括 agent 定义和 skill 目录，无对应的函数/类型条目 -->

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/agents/integration-test-executor.md` | 删除 — 集成测试执行由 test-execution-executor 统一处理 |
| `plugins/dev-team/agents/integration-test-evaluator.md` | 删除 — 集成测试评估由 test-execution-evaluator 统一处理 |
| `plugins/dev-team/skills/phase-integration-test/SKILL.md` | 删除 — 整个 `phase-integration-test/` 目录移除 |
| `plugins/dev-team/bin/src/commands/unit-test.ts` | 重命名 → `test-execution.ts`，原文件删除 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` | 重命名 → `test-execution-output.schema.ts`，原文件删除 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | 重命名 → `test-execution.test.ts`，原文件删除 |
| `plugins/dev-team/agents/unit-test-executor.md` | 重命名 → `test-execution-executor.md`，原文件删除 |
| `plugins/dev-team/agents/unit-test-evaluator.md` | 重命名 → `test-execution-evaluator.md`，原文件删除 |
| `plugins/dev-team/skills/phase-unit-test/SKILL.md` | 重命名 → `skills/phase-test-execution/SKILL.md`，原文件删除 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `runTestExecution` | `plugins/dev-team/bin/src/commands/test-execution.ts` | 重命名 | `function runTestExecution(options: TestExecutionOptions): number` | 原 `runUnitTest`，重命名以匹配新 phase 名称 |
| `TestExecutionOptions` | `plugins/dev-team/bin/src/commands/test-execution.ts` | 重命名 | `interface TestExecutionOptions { change?: string; projectRoot?: string; files?: string[]; framework?: string; noMutation?: boolean }` | 原 `UnitTestOptions`，接口字段不变 |
| `runUnitTest`（旧） | `plugins/dev-team/bin/src/commands/unit-test.ts` | 删除 | — | 被 `runTestExecution` 取代 |
| `UnitTestOptions`（旧） | `plugins/dev-team/bin/src/commands/unit-test.ts` | 删除 | — | 被 `TestExecutionOptions` 取代 |

CLI 子命令：

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `dev-team test-execution` | `plugins/dev-team/bin/src/cli.ts` | 修改 | `cli.command('test-execution', 'Run all automated tests...')` | 原 `dev-team unit-test`，描述更新 |
| `dev-team unit-test`（旧） | `plugins/dev-team/bin/src/cli.ts` | 删除 | — | 不再注册 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `phaseIdSchema` (z.enum) | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | 修改 | 枚举值 `'unit-test'` → `'test-execution'`，移除 `'integration-test'` |
| `UnitTestSubReport` → `TestExecutionSubReport` | `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` | 重命名 | 类型名更新以匹配新 phase 名称；schema 结构不变 |
| `UnitTestSummaryReport` → `TestExecutionSummaryReport` | `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` | 重命名 | 类型名更新；schema 结构不变 |
| 所有 `UnitTest*` export 类型 | `plugins/dev-team/bin/src/schemas/index.ts` | 修改 | 更新 import/export 路径和名称 |

### 配置

<!-- 本变更不涉及 plugin.json / config.json / settings.json 等配置键的增删改 -->

本变更不涉及配置变更。

---

## 数据模型

本变更为纯重命名和删除操作，不涉及数据模型的变更。以下是对现有模型的变更说明：

| 模型 | 字段变更 | 关系变更 | 持久化 |
|------|----------|----------|--------|
| phaseIdSchema 枚举 | `'unit-test'` → `'test-execution'`；删除 `'integration-test'` | 枚举值变更不涉及关系 | Zod schema，编译时类型检查 |
| PhaseDefinition（PhaseDefinition[]） | `unit-test` 条目的 id/description/agent_type 字段更新；`integration-test` 条目删除 | 受 eval.json 中相位引用影响（stale 机制兼容旧 entry） | 内存中的 phase 表定义 |
| eval.json 条目 | 现有 `unit-test`/`integration-test` 条目保留（stale 机制忽略未知 phase） | hasPhasePassed 将未知 phase ID 视为未完成 | JSON 文件持久化 |
| 测试报告 | `reports/unit-test-execution.json` → `reports/test-execution.json`；`reports/unit-test/` → `reports/test-execution/` | 路径更新 | JSON 文件持久化 |

---

## 路由/API 设计

<!-- 本变更为 CLI 工具重构，不涉及 HTTP API -->

本变更为 CLI + MCP 工具重构，不涉及 HTTP API。

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。所有变更仅涉及项目内部文件的重命名、删除和内容更新。

### 构建/测试依赖

- vitest — 已有测试框架，用于运行更新后的 test-execution.test.ts 等测试文件

---

## 待决问题

- 无。proposal.md 已明确所有决策，无待决问题。
