# 设计: reorder-workflow-phases

> **变更**: reorder-workflow-phases
> **日期**: 2026-05-28
> **基于**: proposal.md, test-design.md

---

## 架构组件

本变更为纯结构性变更——不引入新逻辑，仅涉及重命名、重排序和引用更新。所有变更集中在以下组件：

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| PGE Workflow Engine | 定义阶段顺序、索引和前序阶段推导 | `plugins/dev-team/bin/src/lib/workflow.ts` | 无（纯数据） | TypeScript |
| dev-design phase skill | 编排 "设计阶段" 的 Planner->Evaluator 循环 | `plugins/dev-team/skills/phase-dev-design/SKILL.md` | workflow.ts (PHASES), eval-check MCP, eval-log MCP | Markdown (skill) |
| test-design phase skill | 编排 "测试设计阶段" 的 Planner->Evaluator 循环 | `plugins/dev-team/skills/phase-test-design/SKILL.md` | workflow.ts (PHASES), eval-check MCP, eval-log MCP | Markdown (skill) |
| dev-design-planner agent | 根据 proposal.md 编写 design.md + tasks.md | `plugins/dev-team/agents/dev-design-planner.md` | proposal.md, design.md.template, codebase | Markdown (agent) |
| dev-design-evaluator agent | 评估 design.md + tasks.md 质量 | `plugins/dev-team/agents/dev-design-evaluator.md` | design.md, tasks.md, proposal.md, eval.schema.json | Markdown (agent) |
| test-design-planner agent | 根据 proposal.md + design.md 编写测试策略 | `plugins/dev-team/agents/test-design-planner.md` | proposal.md, design.md (新增输入), test-design.md.template | Markdown (agent) |
| test-design-evaluator agent | 评估 test-design.md 质量 | `plugins/dev-team/agents/test-design-evaluator.md` | test-design.md, proposal.md, eval.schema.json | Markdown (agent) |
| code-review-evaluator agent | 代码审查评估，可 backtrack 到 dev-design | `plugins/dev-team/agents/code-review-evaluator.md` | design.md, proposal.md, codebase | Markdown (agent) |
| unit-test-evaluator agent | 单元测试报告评估，诊断决策树 | `plugins/dev-team/agents/unit-test-evaluator.md` | test execution report, codebase | Markdown (agent) |
| integration-test-evaluator agent | 集成测试报告评估，诊断决策树 | `plugins/dev-team/agents/integration-test-evaluator.md` | test execution report, codebase | Markdown (agent) |
| phase-unit-test skill | 编排单元测试执行阶段 | `plugins/dev-team/skills/phase-unit-test/SKILL.md` | eval-check MCP, eval-log MCP | Markdown (skill) |
| phase-integration-test skill | 编排集成测试执行阶段 | `plugins/dev-team/skills/phase-integration-test/SKILL.md` | eval-check MCP, eval-log MCP | Markdown (skill) |
| phase-code-review skill | 编排代码审查阶段 | `plugins/dev-team/skills/phase-code-review/SKILL.md` | eval-check MCP, eval-log MCP | Markdown (skill) |
| MCP Server | 注册 eval_log / eval_check 工具 | `plugins/dev-team/bin/src/mcp.ts` | workflow.ts (phase 标识符示例) | TypeScript |
| 项目配置文件 | 插件元数据 | `plugins/dev-team/.claude-plugin/plugin.json` | 无 | JSON |

### 调整后的工作流顺序

```
01-requirements → 02-dev-design → 03-test-design → 04-test-gen → 05-implement → 06-unit-test → 07-code-review → 08-integration-test → 09-acceptance
```

---

## 数据流

### 阶段间数据流动

本变更调整了两个阶段的顺序，改变了 design.md 和 test-design.md 的生成时序：

```
变更前:
proposal.md → test-design.md (无 design.md 上下文)
           ↘ design.md + tasks.md (基于 proposal.md + test-design.md)

变更后:
proposal.md → design.md + tasks.md (基于 proposal.md + codebase)
           ↘ test-design.md (基于 proposal.md + design.md)
```

具体数据流：

1. **01-requirements**: 产出 `proposal.md`（需求 + 验收标准）
2. **02-dev-design** (原 03-dev-proposal): 读取 `proposal.md` + codebase + 模板，产出 `design.md` + `tasks.md`
   - **不再读取** `test-design.md`（此时尚未生成）
3. **03-test-design** (原 02-test-design): 读取 `proposal.md` + **新增加 `design.md`** + 模板，产出 `test-design.md`
   - **新增输入**: `design.md` 提供架构、数据流、路由设计等上下文，使测试策略制定更加精准
4. **04-test-gen 及之后阶段**: 编号行为不变，但所有回溯到 dev-design 或 test-design 的引用需同步更新

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| PHASES | `readonly string[]` — 9 个阶段标识符 | `getPhaseIndex()` 返回索引, `getPriorPhases()` 返回前序阶段列表 | `workflow.ts` 常量 |
| phase-dev-design SKILL.md | name, gate check phase (02-dev-design), subagent_type (dev-design-planner/evaluator), eval-log phase | 引用 workflow.ts 中的阶段标识符 | `skills/phase-dev-design/SKILL.md` |
| phase-test-design SKILL.md | gate check phase (03-test-design) | 引用 workflow.ts 中的阶段标识符 | `skills/phase-test-design/SKILL.md` |
| agent.md (6 个文件) | name, description, eval-log phase, backtrack_to phase | 引用 workflow.ts 中的阶段标识符 | `agents/*.md` |
| plugin.json | version | 无 | `.claude-plugin/plugin.json` |

---

## 路由/API 设计

本变更为结构性变更，不涉及 API 路由设计。但 MCP 工具的描述字符串中包含阶段标识符示例，需要更新：

| MCP 工具 | 变更内容 | 文件位置 |
|----------|----------|----------|
| eval_check | description 中的示例阶段 "03-dev-proposal" → "02-dev-design" | `plugins/dev-team/bin/src/mcp.ts` |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | `dev-proposal` 重命名为 `dev-design` | 该阶段产出 design.md（架构设计文档），"design" 比 "proposal" 更准确表达阶段产出。避免命名与 proposal.md（需求文档）混淆。 | **备选**: 保留 `dev-proposal` 命名，仅调整顺序。拒绝理由: 命名与阶段产出不匹配，继续使用 "proposal" 会造成语义混淆——proposal.md 是需求阶段产出，而该阶段产出的是设计文档。 |
| D2 | dev-design 移到 test-design 之前 | 设计驱动测试策略，符合工程最佳实践。test-design-planner 需要 design.md 中的架构和数据流信息才能制定准确的测试策略。 | **备选**: 保持 test-design 在 dev-design 之前。拒绝理由: 测试策略缺乏架构上下文，导致测试范围判断不准确，且 dev-design-planner 读取 test-design.md 形成了不合理的反向依赖。 |
| D3 | test-design-planner 新增 design.md 作为输入 | dev-design 先行后，design.md 已就绪。test-design-planner 可利用设计中的架构组件、数据流、路由设计等信息制定更精确的测试策略。 | **备选**: 保持 test-design-planner 仅读取 proposal.md。拒绝理由: 缺少架构上下文，测试策略无法判断哪些组件需要测试、集成边界在哪里。 |
| D4 | dev-design-planner 移除 test-design.md 输入 | 调整顺序后 dev-design 在 test-design 之前执行，test-design.md 尚未生成。 | **备选**: 让 dev-design-planner 在 test-design.md 不存在时跳过读取。拒绝理由: 产生不必要的条件分支，且 agent 指令应清晰明确，不存在时仍会尝试读取产生误导。 |
| D5 | 更新 openspec/specs/ 中的规范文件 | spec 文件是体系结构规范，包含旧阶段标识符的引用。更新 spec 确保架构文档与实际实现一致。 | **备选**: 不更新 spec 文件。拒绝理由: spec 与实现不一致会导致后续变更产生混淆，spec 应始终反映当前架构。 |

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。所有变更均为文件重命名、内容引用更新和数组重排序。

### 构建/测试依赖

- `vite-plus/test` — 单元测试框架（已有，用于 `workflow.test.ts`）
- `bats` — shell 自动化测试框架（已有，用于集成测试）
- `grep` / `find` — 文件系统完整性验证（已有）

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| eval.json 中旧 phase 标识符与新版本不兼容 | 进行中变更无法继续 | 低（当前无活跃变更） | proposal 声明现有 eval.json 条目使用旧标识符的需手动迁移或丢弃 |
| 重命名遗漏引用导致运行时错误 | gate check 使用错误的 phase 标识符 | 中 | grep 全量搜索 `dev-proposal`、`03-dev-proposal`、`02-test-design`，确保无遗漏 |
| agent 文件名变更导致引用断开 | Agent 工具找不到 agent | 低 | 同步更新所有 SKILL.md 中的 subagent_type 引用 |
| dev-team-mcp.cjs 未同步重建 | MCP 工具描述字符串仍是旧示例 | 低 | 加入构建步骤作为任务 |
| openspec-propose 已废弃但仍有引用 | 用户看到过时迁移提示 | 低 | 一并更新 SKILL.md 中的迁移路径 |

---

## 迁移步骤

1. 执行 grep 全量搜索 `dev-proposal`、`03-dev-proposal`、`02-test-design` 建立基线
2. 按 tasks.md 顺序执行修改（从核心 workflow.ts 开始，向外围 spec 文件扩展）
3. 每次修改后执行 grep 验证无旧标识符残留
4. 更新 plugin.json 版本号
5. 运行单元测试和集成测试验证
6. 最后执行全仓 grep 确认所有引用已更新（排除 archive/ 目录和当前变更的 spec/ 目录）

---

## 待决问题

- 无。本变更范围明确，所有决策已在设计中覆盖。
