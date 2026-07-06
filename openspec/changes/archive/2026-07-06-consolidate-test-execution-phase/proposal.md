# 提案: consolidate-test-execution-phase

> **变更**: consolidate-test-execution-phase
> **日期**: 2026-07-03
> **状态**: 草稿

---

## 问题

当前 PGE 工作流将自动化测试分为两个独立阶段：`unit-test`（单元测试）和 `integration-test`（集成测试）。这种分离带来了以下问题：

1. **执行冗余**：每轮完整工作流需要依次运行两个测试阶段，增加了端到端执行时间。两个阶段共享相同的前置依赖（`test-gen`、`implement`），且执行方法高度相似（Executor 读取 plan → 运行测试命令 → 解析输出 → 写入报告 → Evaluator 诊断）。

2. **复杂度过高**：测试分离迫使初学者理解 unit-test 和 integration-test 两个阶段的差异，而大多数项目使用单一测试框架（vitest/jest/pytest）统一运行全部测试文件。将测试按文件命名约定（`.test.ts` vs `__tests__/`）硬分阶段增加了不必要的认知负担。

3. **维护成本**：维护两套 Executor、Evaluator agent 定义和两套 skill 文件（phase-unit-test / phase-integration-test），实质内容高度重复。任何测试诊断逻辑的变更都需要同步修改两处。

4. **前置依赖膨胀**：`code-review` 和 `acceptance` 等下游阶段需要在 prerequisites 中同时列出 `test-gen`、`implement`、`integration-test` 等条目，依赖关系图复杂。

---

## 提案

将 `unit-test` 阶段重命名为 `test-execution`，`integration-test` 阶段整体移除。`test-execution` 阶段合并执行所有自动化测试（单元测试 + 集成测试），按框架依次执行，不区分单元/集成。

### 核心变更

1. **Phase ID**: `unit-test` → `test-execution`，`integration-test` 删除
2. **执行策略**: 单一 Executor agent + 单一 Evaluator agent，Executor 内部按框架分别运行所有测试（vitest → 所有 *.test.ts，pytest → 所有 tests/，go test → 所有 ./...，cargo test → 所有）
3. **Agent 重构**: `unit-test-executor.md` → `test-execution-executor.md`；`unit-test-evaluator.md` → `test-execution-evaluator.md`；删除 `integration-test-executor.md` 和 `integration-test-evaluator.md`
4. **Skill 重构**: `phase-unit-test` → `phase-test-execution`；删除 `phase-integration-test` 整个目录
5. **CLI 命令**: `dev-team unit-test` → `dev-team test-execution`
6. **前置依赖**: 所有工作流中删除 `integration-test` 条目

### 新的前置依赖关系

**requirement 工作流**:

| Phase | 前置依赖 |
|-------|----------|
| proposal | [] |
| dev-design | [proposal] |
| test-design | [proposal, dev-design] |
| implement | [dev-design] |
| test-gen | [test-design, implement] |
| test-execution | [test-gen, implement] |
| code-review | [test-gen, implement] |
| acceptance | [proposal, dev-design, implement] |

**bug-fix 工作流**:

| Phase | 前置依赖 |
|-------|----------|
| proposal | [] |
| dev-design | [proposal] |
| implement | [dev-design] |
| test-execution | [implement] |
| code-review | [implement] |
| acceptance | [code-review] |

**test-only 工作流**:

| Phase | 前置依赖 |
|-------|----------|
| proposal | [] |
| code-analyze | [proposal] |
| test-design | [proposal, code-analyze] |
| test-gen | [test-design] |
| test-execution | [test-gen] |

### test-design 阶段

`test-resolve-paths` MCP 工具保留 unit_tests / integration_tests 分离输出（设计层面分开描述仍有价值）。test-design-planner 的映射逻辑不变。

---

## 能力

### 修改的能力

- **pge-workflow-engine** — 更新所有 PHASE_TABLES（requirement/bug-fix/test-only）：删除 `integration-test`、`unit-test` → `test-execution`；更新所有 PHASE_PREREQUISITES 表和 getDependents 逻辑
- **unit-test-executor** — 从 `unit-test-executor` 更名为 `test-execution-executor`，描述和 prompt 更新为"执行所有自动化测试（单元+集成）"
- **phase-agents** — 重命名 `unit-test-executor/evaluator` → `test-execution-executor/evaluator`；删除 `integration-test-executor/evaluator` agent 文件
- **phase-skills** — 重命名 `phase-unit-test` → `phase-test-execution`；删除 `phase-integration-test` 整个 skill 目录
- **workflow-orchestration** — 更新 workflow-requirement、workflow-test-only skill 中对测试阶段的引用
- **cli-unit-test-execute** — CLI 命令从 `unit-test` 更名为 `test-execution`，文件路径和输出目录同步更新
- **pipeline-backtrack** — 删除 `integration-test` 在依赖传播中的引用；更新 getDependents 返回值
- **eval-json-protection** — 更新 phaseIdSchema 枚举：移除 `integration-test`，`unit-test` → `test-execution`
- **test-execution-diagnostics** — 更新所有 phase 引用从 `unit-test`/`integration-test` 为 `test-execution`；合并两阶段诊断规则为单一决策树
- **test-only-workflow** — 更新 test-only 工作流的 phase 表，删除 `integration-test`

---

## 变更范围

### 实现文件

| 文件 | 变更类型 |
|------|----------|
| `bin/src/schemas/phase-log.schema.ts` | 修改 — phaseIdSchema 枚举 |
| `bin/src/lib/workflow.ts` | 修改 — 三个 PHASE_TABLES + 四个 PREREQUISITES_TABLES |
| `bin/src/commands/phase-next.ts` | 修改 — 更新 getDependents 调用 |
| `bin/src/lib/eval-json.ts` | 修改 — 更新 stale propagation 中的 dependents 引用 |
| `bin/src/commands/unit-test.ts` | 重命名 → `test-execution.ts` |
| `bin/src/cli.ts` | 修改 — 注册 `test-execution` 代替 `unit-test` |
| `bin/src/schemas/unit-test-output.schema.ts` | 重命名 → `test-execution-output.schema.ts`；更新 phase 枚举 |
| `agents/unit-test-executor.md` | 重命名 → `test-execution-executor.md`；更新内容 |
| `agents/unit-test-evaluator.md` | 重命名 → `test-execution-evaluator.md`；更新内容 |
| `agents/integration-test-executor.md` | 删除 |
| `agents/integration-test-evaluator.md` | 删除 |
| `skills/phase-unit-test/SKILL.md` | 重命名 → `skills/phase-test-execution/SKILL.md`；更新内容 |
| `skills/phase-integration-test/SKILL.md` | 删除整个目录 |
| `skills/workflow-requirement/SKILL.md` | 修改 — 更新测试阶段引用 |
| `skills/workflow-test-only/SKILL.md` | 修改 — 更新测试阶段引用 |
| Python 脚本（若 test-runner.py 引用 phase 名） | 修改 — 更新 phase 引用 |

### 测试文件

| 文件 | 变更类型 |
|------|----------|
| `bin/src/lib/workflow.test.ts` | 修改 — 更新所有 phase ID 引用 (~20处) |
| `bin/src/lib/eval-json.test.ts` | 修改 — 更新 integration-test 引用 |
| `bin/src/commands/phase-next.test.ts` | 修改 — 更新所有 phase ID 引用 (~40处) |
| `bin/src/commands/unit-test.test.ts` | 重命名 → `test-execution.test.ts`；更新 phase 引用 |

### 不要修改

- `bin/src/lib/test-parser/` — 测试解析器逻辑不变（解析器与阶段无关）
- `bin/src/commands/test-resolve-paths.ts` — 仍区分 unit/integration 路径（工具层面）
- `bin/src/commands/test-detect-frameworks.ts` — 框架检测逻辑不变
- `schemas/test-resolve-paths.schema.ts` — schema 不变
- `agents/test-design-planner.md` — 对 integration_tests 的引用不变（设计层面分开描述有价值）
- `skills/phase-test-design/SKILL.md` — 不涉及测试阶段引用
- 其他 planner agent 文档 — 非结构性引用不变

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | phaseIdSchema 更新 | `phase-log.schema.ts` 的 phaseIdSchema 枚举包含 `test-execution`，不包含 `integration-test` 和 `unit-test` |
| AC-2 | requirement 工作流 phase 表 | `getPhaseTable("requirement")` 返回 8 个 phase，id 列表为 `[proposal, dev-design, test-design, implement, test-gen, test-execution, code-review, acceptance]` |
| AC-3 | bug-fix 工作流 phase 表 | `getPhaseTable("bug-fix")` 返回 6 个 phase，不包含 `integration-test` |
| AC-4 | test-only 工作流 phase 表 | `getPhaseTable("test-only")` 返回 5 个 phase，不包含 `integration-test` |
| AC-5 | prerequisites 更新 | `getPrerequisites("test-execution", "requirement")` 返回 `["test-gen", "implement"]`；`getPrerequisites("code-review", "requirement")` 返回 `["test-gen", "implement"]`（不含 integration-test）|
| AC-6 | dependents 更新 | `getDependents("implement")` 不包含 `integration-test`；`getDependents("test-gen")` 不包含 `integration-test` |
| AC-7 | CLI 命令重命名 | `dev-team test-execution` 可用，`dev-team unit-test` 不可用或显示 deprecated 提示 |
| AC-8 | Agent 重命名 | `agents/test-execution-executor.md` 和 `agents/test-execution-evaluator.md` 存在且内容正确；`agents/integration-test-executor.md` 和 `agents/integration-test-evaluator.md` 不存在 |
| AC-9 | Skill 重命名 | `skills/phase-test-execution/SKILL.md` 存在且内容正确；`skills/phase-integration-test/` 目录不存在 |
| AC-10 | 现有工作流测试通过 | 更新所有 phase 引用后，`workflow.test.ts`、`phase-next.test.ts`、`eval-json.test.ts` 测试通过 |
| AC-11 | 报告文件路径更新 | CLI 输出从 `reports/unit-test-execution.json` 改为 `reports/test-execution.json`；子报告从 `reports/unit-test/<fw>.json` 改为 `reports/test-execution/<fw>.json` |
| AC-12 | stale propagation 不含 integration-test | `propagateStale(entries, "implement")` 仅传播到 `test-gen`, `test-execution`, `code-review`, `acceptance`（不含 integration-test）|

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 现有 eval.json 中遗留 `unit-test`/`integration-test` 条目导致 phase_next 行为异常 | 工作流回溯时可能跳过 test-execution | 中 | eval-json 的 hasPhasePassed 将未知 phase ID 视为未完成；旧 entry 被 stale 标记忽略 |
| 集成测试文件中与单元测试的命令差异（如需要额外 setup）未被覆盖 | test-execution 运行结果不完整 | 低 | Executor 按框架运行所有测试文件（如 vitest 同时覆盖 .test.ts 和 __tests__/）；框架级配置（如需 pre-setup）由 test_detect_frameworks 的 plan 管理 |
| 依赖 propagation 的 breadth-first 遍历在移除 integration-test 后边界变化 | 传播范围可能超出预期 | 低 | 所有传播逻辑基于动态 getDependents()，删除 integration-test 后自动调整 |
| 现存 CI/CD 或外部脚本直接调用 `dev-team unit-test` | 集成或管道脚本中断 | 中 | 保留 `unit-test` 别名或输出 migration 提示；在 release notes 中声明 breaking change |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 新 phase ID 名称 | `test-execution` | 直接重命名，保持语义清晰；不关心旧 eval.json 向后兼容（stale 机制可处理） | `test-runner`——"execution"更准确描述运行而非仅"runner" |
| 执行策略 | 方案 B：单一 Executor + 单一 Evaluator | 减少 agent 数量，简化维护；运行所有测试文件不区分单元/集成 | 方案 A：保留两阶段但修改名称——不够简化；方案 C：单一 Executor + 合并规则——over-engineered |
| test-resolve-paths 工具 | 保留 unit/integration 分离输出 | 设计层面分开描述仍有价值，不影响执行层面合并 | 完全合并——失去设计层面的分类信息 |
| `integration-test` 删除是否保留为 no-op phase | 不保留，直接删除 | 删除后 phase 表更简洁，可减少不必要的 eval.json 条目 | 保留为 no-op 但不执行——冗余层 |

### 待决问题

- 无
