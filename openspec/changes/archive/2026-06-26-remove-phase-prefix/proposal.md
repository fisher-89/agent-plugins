# 提案: remove-phase-prefix

> **变更**: remove-phase-prefix
> **日期**: 2026-06-25
> **状态**: 提案中

---

## 问题

当前 PGE 工作流中所有 phase 的 ID 均使用序号前缀，如 `01-proposal`、`02-dev-design`、`09-acceptance` 等。这些带序号前缀的 phase ID 被硬编码在约 30 个文件中，包括：

- `workflow.ts` 中的 phase 表定义和 prerequisite 表
- 9 个 phase SKILL.md 中的 gate check 断言、verdict 读取路径、backtrack 目标
- 2 个 workflow SKILL.md 中的 phase 引用
- 7 个 evaluator agent .md 中 `phase_log` 调用的 phase 参数
- 2 个 executor agent .md 中的 phase 参数
- `phase-next.ts` 中的 backtrack 逻辑
- `phase-log.ts` 中的 phase 存在性验证
- 测试文件中的断言字符串

每次增删或重排 phase（如之前将 `05-implement` 移到 `04-test-gen` 之前）都需要遍历所有文件逐一修改硬编码的 phase ID，极易遗漏且难以审查。序号前缀本身不代表执行顺序——实际的执行顺序由 `workflow.ts` 中 phase 表的数组顺序决定，这使得前缀既冗余又具有误导性。

---

## 提案

移除所有 phase ID 中的序号前缀，使用不带数字的纯 phase 名称作为标识符：

| 当前 ID | 新 ID |
|---------|-------|
| `01-proposal` | `proposal` |
| `02-dev-design` | `dev-design` |
| `03-test-design` | `test-design` |
| `04-test-gen` | `test-gen` |
| `05-implement` | `implement` |
| `06-unit-test` | `unit-test` |
| `07-code-review` | `code-review` |
| `08-integration-test` | `integration-test` |
| `09-acceptance` | `acceptance` |

核心变更包括：

1. **`workflow.ts`**：phase 表 `id` 字段和 prerequisite 表键名去前缀
2. **`phase-next.ts`**：`interpolatePrompt` 增加 `<phase>` 参数注入，允许 phase 名称动态传递给 agent；backtrack 逻辑同步更新
3. **`phase-log.ts`**：phase 存在性验证的 phase 表引用同步更新
4. **所有 SKILL.md**：gate check 和 verdict 读取中的硬编码 phase ID 替换为不带前缀的 ID
5. **所有 agent .md**：`phase_log` 调用中的 phase 参数去前缀
6. **测试文件**：断言字符串同步更新

核心收益：增删或重排 phase 只需修改 `workflow.ts` 一个文件。

---

## 能力

### 修改的能力

- `pge-workflow-engine` — workflow.ts 中 phase ID 去前缀，prerequisite 表同步更新。`interpolatePrompt` 增加 `<phase>` 动态注入参数。
- `phase-agents` — 所有 evaluator/executor agent .md 中去硬编码 phase ID，改为从 phase_next 返回的动态值或从 phase_log 参数获取。
- `phase-skills` — 所有 phase SKILL.md 中去硬编码 phase ID，gate check 断言和 verdict 读取路径同步更新。
- `workflow-orchestration` — workflow-requirement 和 workflow-test-only SKILL.md 中去除硬编码数字（如 `Total phases: 6`），改用 `phase_next` 返回的 `total_phases` 动态值。
- `pipeline-backtrack` — `phase-next.ts` 中 backtrack 逻辑使用去前缀的 phase ID 进行查找和比较。
- `eval-json-protection` — `phase-log.ts` 中 phase 存在性验证的 phase 表引用同步更新。

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/workflow.ts` — phase 表 id 去前缀，prerequisite 表键名去前缀
- `plugins/dev-team/bin/src/commands/phase-next.ts` — `interpolatePrompt` 增加 `<phase>` 注入；backtrack 逻辑使用新 phase ID
- `plugins/dev-team/bin/src/commands/phase-log.ts` — phase 存在性验证同步更新
- `plugins/dev-team/skills/phase-proposal/SKILL.md` — gate check 和 verdict 读取去前缀
- `plugins/dev-team/skills/phase-dev-design/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-test-design/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-test-gen/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-implement/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-unit-test/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-code-review/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-integration-test/SKILL.md` — 同上
- `plugins/dev-team/skills/phase-acceptance/SKILL.md` — 同上
- `plugins/dev-team/skills/workflow-requirement/SKILL.md` — 去除序号引用
- `plugins/dev-team/skills/workflow-test-only/SKILL.md` — 去除序号引用；修复硬编码 `Total phases: 6` → `{total_phases}`
- `plugins/dev-team/skills/openspec-archive-change/SKILL.md` — phase 引用去前缀
- `plugins/dev-team/agents/proposal-evaluator.md` — `phase_log` phase 参数去前缀
- `plugins/dev-team/agents/dev-design-evaluator.md` — 同上
- `plugins/dev-team/agents/test-design-evaluator.md` — 同上
- `plugins/dev-team/agents/test-gen-evaluator.md` — 同上
- `plugins/dev-team/agents/implementation-evaluator.md` — 同上
- `plugins/dev-team/agents/code-analyze-evaluator.md` — 同上
- `plugins/dev-team/agents/code-review-evaluator.md` — 同上
- `plugins/dev-team/agents/acceptance-evaluator.md` — 同上
- `plugins/dev-team/agents/unit-test-evaluator.md` — phase ID 去前缀 + backtrack 目标去前缀
- `plugins/dev-team/agents/integration-test-evaluator.md` — phase ID 去前缀 + backtrack 目标去前缀
- `plugins/dev-team/agents/unit-test-executor.md` — phase 参数去前缀
- `plugins/dev-team/agents/integration-test-executor.md` — phase 参数去前缀
- `plugins/dev-team/.claude-plugin/plugin.json` — 版本号 patch bump

### 测试文件

- `plugins/dev-team/bin/src/commands/phase-next.test.ts` — 断言字符串更新（约 100+ 处 phase ID 引用）
- `plugins/dev-team/bin/src/lib/workflow.test.ts` — 断言字符串更新
- `plugins/dev-team/bin/src/commands/phase-log.test.ts` — 断言字符串更新

### 不要修改

- 存量 `eval.json` 数据 — 不兼容存量格式。用户需在升级前结束所有进行中的流程，否则存量数据中的旧 phase ID 将无法被新版代码识别。
- 核心 `agent_type` 名称（如 `dev-team:proposal-planner`）— agent_type 名称不变，仅 phase ID 变更。
- 用户可见的 phase 描述文本 — 仅在标准输出/推送通知中使用的描述文本不变。
- MCP 工具名称 — `phase_next`、`phase_log` 等 MCP 工具名不变。
- 非 phase 相关的能力规格 — `embedded-cli`、`config-schema`、`static-check-hook` 等不涉及 phase ID 的模块不受影响。

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | workflow.ts phase 表去前缀 | `getPhaseTable('requirement')[0].id` 返回 `"proposal"`，而非 `"01-proposal"` |
| AC-2 | workflow.ts prerequisite 表去前缀 | `getPrerequisites('implement', 'requirement')` 返回 `["dev-design"]` |
| AC-3 | phase-next.ts <phase> 动态注入 | `interpolatePrompt()` 支持 `<phase>` 占位符替换为当前 phase ID |
| AC-4 | phase SKILL.md gate check 去前缀 | 所有 phase SKILL.md 中 gate check 断言使用新 phase ID |
| AC-5 | agent evaluator .md phase_log 去前缀 | 所有 evaluator agent .md 中 `phase_log` 调用的 phase 参数使用新 ID |
| AC-6 | 测试断言更新 | 所有测试文件通过，phase ID 断言使用新格式 |
| AC-7 | workflow 编排不受影响 | `phase_next` 和 `phase_log` 的完整工作流正常执行 |
| AC-8 | 版本号更新 | `plugins/dev-team/.claude-plugin/plugin.json` 版本号已递增 |
| AC-9 | workflow SKILL 无硬编码序号 | 全文搜索 workflow-requirement 和 workflow-test-only SKILL.md 不包含 `Total phases: 6` 等硬编码数字；`phase_index`/`total_phases` 均来自 `phase_next` 返回值 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 存量 eval.json 不兼容 | 进行中的流程无法继续，需手动清理 | 高 | 升级前告知用户结束所有流程；降级可恢复 |
| 遗漏某处硬编码 phase ID | 运行时 phase 匹配失败 | 中 | grep 全量搜索 `0[0-9]-` 模式确保无遗漏；对照文件清单逐一审查 |
| 测试断言遗漏 | 部分测试在 CI 中因字符串不匹配而失败 | 中 | 全量搜索测试文件中的旧 phase ID；运行全部测试验证 |
| SKILL.md 中自然语言描述的 phase ID 遗漏 | 用户看到的文档/提示中出现旧 ID | 低 | 全量搜索所有 .md 文件 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 是否保留前缀作为注释/文档？ | 不保留 | 数组顺序是唯一真实来源，前缀冗余且误导 | 保留前缀但仅作为注释 —— 增加了改动量和误解风险 |
| SKILL.md 中的 phase ID 如何处理？ | SKILL.md 保留身份标识（如 `"proposal"`）但不带序号 | 每个 phase SKILL 天然知道自己属于哪个 phase，身份稳定不变；gate check 和 verdict 读取用不带序号的新 ID | 完全从 phase_next 取 —— 增加复杂度，无实际收益 |
| agent .md 中 phase ID 如何获取？ | 用 `<phase>` 占位符由 phase_next prompt 注入 | agent 从 prompt 上下文获知自己的 phase ID，`phase_log` 调用直接使用注入的 ID | agent 硬编码 —— 会使 agent 耦合到特定 phase |
| workflow SKILL 中的 total_phases 如何处理？ | 使用 `result.total_phases` 动态值 | `phase_next` 是权威来源；已发现 `workflow-test-only:105` 存在硬编码 `Total phases: 6`，需修复 | 硬编码数字 —— 增删 phase 时易遗漏 |

### 待决问题

- 升级策略：是否提供迁移脚本自动标记存量 eval.json 中的旧 phase ID 为 stale？当前结论：不提供，用户手动结束流程。

---
