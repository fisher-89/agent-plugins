# 提案: reorder-workflow-phases

> **变更**: reorder-workflow-phases
> **日期**: 2026-05-27
> **状态**: 设计中

---

## 问题

当前 PGE 工作流阶段顺序为 `01-requirements → 02-test-design → 03-dev-proposal`，存在逻辑倒置：

- `test-design` (02) 在 `dev-proposal` (03) **之前**执行，但测试策略制定需要了解系统架构才能确定测试范围
- `dev-proposal` (03) 反而依赖 `test-design.md` 作为输入，但设计不应由测试策略驱动
- `test-design-planner` 仅基于 `proposal.md` 生成测试策略，缺少 `design.md` 中的架构、数据流、路由设计等关键信息

此外，"dev-proposal"（开发提案）命名不够直观——该阶段产出 `design.md`（架构设计），实际定位是"开发设计"。

## 提案

1. **重命名**: `phase-dev-proposal` → `phase-dev-design`，内部 agent 同步重命名（`dev-proposal-planner` → `dev-design-planner`，`dev-proposal-evaluator` → `dev-design-evaluator`）
2. **调整顺序**: 将 dev-design 移到 test-design **之前**，使设计驱动测试策略
3. **更新依赖**: `test-design-planner` 增加 `design.md` 作为输入；`dev-design-planner` 不再读取 `test-design.md`（尚未生成）
4. **同步引用**: 所有 `backtrack_to`、gate check phase 标识符、eval-log phase 参数同步更新

调整后的顺序:

```
01-requirements → 02-dev-design → 03-test-design → 04-test-gen → 05-implement → ...
```

---

## 能力

### 修改的能力

- **pge-workflow-engine** — PHASES 数组中 02/03 阶段重排，dev-proposal 重命名为 dev-design
- **phase-skills** — 目录重命名（phase-dev-proposal → phase-dev-design），SKILL.md 更新 name/phase/agent 引用
- **phase-agents** — agent 文件重命名，dev-design-planner 输入变更（不再读 test-design.md），test-design-planner 输入变更（新增 design.md）
- **pipeline-backtrack** — backtrack_to 引用更新（03-dev-proposal → 02-dev-design，02-test-design → 03-test-design）

---

## 变更范围

### 实现以下特性

- `workflow.ts` PHASES 数组重排
- 目录 `skills/phase-dev-proposal/` → `skills/phase-dev-design/` 重命名
- `agents/dev-proposal-planner.md` → `agents/dev-design-planner.md` 重命名并更新内容
- `agents/dev-proposal-evaluator.md` → `agents/dev-design-evaluator.md` 重命名并更新内容
- `skills/phase-dev-design/SKILL.md` 更新 phase/agent 引用
- `skills/phase-test-design/SKILL.md` 更新 phase 引用
- `agents/test-design-planner.md` 增加 design.md 输入
- `agents/test-design-evaluator.md` 更新 phase 引用
- 所有 backtrack_to 引用更新（5 个文件）
- `plugin.json` 版本号升级

### 不要修改

- 04-test-gen 及之后阶段的编号（不变）
- `eval-check.ts` / `eval-log.ts` 核心逻辑（只认 phase 标识符字符串）
- DESIGN/EXEC/EVAL-ONLY 执行模式
- P→E / G→E 循环逻辑
- 任何模板文件

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | PHASES 数组顺序为 [01-requirements, 02-dev-design, 03-test-design, ...] | 读取 workflow.ts 验证 | P0 |
| AC-2 | phase-dev-design 目录存在，phase-dev-proposal 目录不存在 | 文件系统检查 | P0 |
| AC-3 | dev-design-planner.md 和 dev-design-evaluator.md 存在，旧 agent 文件已删除 | 文件系统检查 | P0 |
| AC-4 | dev-design-planner 读取 proposal.md + codebase，不读取 test-design.md | 读取 agent.md 验证 | P0 |
| AC-5 | test-design-planner 读取 proposal.md + design.md | 读取 agent.md 验证 | P0 |
| AC-6 | phase-dev-design/SKILL.md 中 name 为 phase-dev-design，gate check 使用 02-dev-design | grep 验证 | P0 |
| AC-7 | phase-test-design/SKILL.md 中 gate check 使用 03-test-design | grep 验证 | P0 |
| AC-8 | 所有 backtrack_to 引用 02-dev-design（非 03-dev-proposal），03-test-design（非 02-test-design） | grep 全量搜索验证 | P0 |
| AC-9 | dev-team eval-check --phase 02-dev-design 正确识别前置阶段为 [01-requirements] | CLI 测试 | P1 |
| AC-10 | dev-team eval-check --phase 03-test-design 正确识别前置阶段为 [01-requirements, 02-dev-design] | CLI 测试 | P1 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| eval.json 中旧 phase 标识符与新的不兼容 | 进行中变更无法继续 | 低（当前无活跃变更） | 在 proposal 中记录：现有 eval.json 条目使用旧标识符的需手动迁移或丢弃 |
| 重命名遗漏引用 | 运行时错误 | 中 | grep 全量搜索 `dev-proposal` 和 `02-test-design`，确保无遗漏 |
| agent 文件名变更导致引用断开 | Agent 工具找不到 agent | 低 | 同步更新所有 SKILL.md 中的 subagent_type 引用 |
