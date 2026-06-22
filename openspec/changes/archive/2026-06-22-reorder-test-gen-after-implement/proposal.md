# 提案: reorder-test-gen-after-implement

> **变更**: reorder-test-gen-after-implement
> **日期**: 2026-06-22
> **状态**: 设计中

---

## 问题

当前 PGE 工作流（`requirement` / `refactor`）中，`04-test-gen`（测试代码生成）在 `05-implement`（实现）**之前**执行：

```
01-proposal → 02-dev-design → 03-test-design → 04-test-gen → 05-implement → 06-unit-test → ...
```

这带来以下问题：

1. **测试生成缺少真实实现**：`test-gen-generator` 虽会读取源码，但在实现阶段完成前，被测模块可能尚不存在或仅为占位代码，生成的测试骨架基于推测而非实际 API 与行为
2. **与 TDD 后置测试策略不符**：团队希望先完成实现，再基于 `test-design.md` 与实际代码生成测试，使测试断言对准真实签名与逻辑
3. **并行轨道语义混淆**：`parallel-dev-test-tracks` 引入依赖图后，dev 轨（05-implement）与 test 轨（03→04）可并行；但 `PHASES` 数组仍将 04 排在 05 之前，导致 `phase/next` 默认先调度 test-gen，与「实现优先」意图不一致

阶段 **标识符**（`04-test-gen`、`05-implement`）保持不变，仅调整**执行顺序**与**前置依赖**。

---

## 提案

将 `05-implement` 移到 `04-test-gen` 之前执行，并更新前置依赖，使 test-gen 必须等待实现完成：

### 调整后的阶段顺序（`PHASE_REQUIREMENT` 数组）

```
01-proposal → 02-dev-design → 03-test-design → 05-implement → 04-test-gen → 06-unit-test → ...
```

### 调整后的前置依赖（`PHASE_PREREQUISITES`）

| Phase | 变更前 | 变更后 |
|-------|--------|--------|
| 04-test-gen | [03-test-design] | [03-test-design, **05-implement**] |
| 05-implement | [02-dev-design] | [02-dev-design]（不变） |
| 06-unit-test | [04-test-gen, 05-implement] | [04-test-gen, 05-implement]（不变） |

### 依赖图变化

```
                    ┌─────────────────────────────────────────┐
  01-proposal ──────┼──▶ 02-dev-design ──▶ 05-implement ──┐ │
                    │         │                │           │ │
                    │         └──▶ 03-test-design           │ │
                    │                    │                  │ │
                    │                    └──▶ 04-test-gen ◀──┘ │
                    │                              │          │
                    │                    06 / 07 / 08        │
                    └─────────────────────────────────────────┘
                                          │
                                          ▼
                                   09-acceptance
```

**关键语义**：

- `05-implement` 仍仅依赖 `02-dev-design`，可与 `03-test-design` 并行（当 02 完成后）
- `04-test-gen` 新增对 `05-implement` 的依赖，必须等实现 pass 后才能生成测试
- `phase/next` 按数组顺序扫描：03 完成后优先返回 `05-implement`，05 pass 后再返回 `04-test-gen`
- 回溯传播：`markPhaseStale("05-implement")` 将直接使 `04-test-gen` 失效（05 的新 downstream）

### 实现要点

1. **`workflow.ts`**：交换 `PHASE_REQUIREMENT` 中 04/05 条目顺序；更新 `PHASE_PREREQUISITES['04-test-gen']`
2. **单元测试**：更新 `workflow.test.ts`、`phase-next.test.ts` 中对阶段索引、前置依赖、`phase/next` 返回值的断言
3. **无需变更**：阶段 skill 文件（gate check 仍用各自 phase ID）、agent 定义、eval.json schema、bug-fix 工作流

---

## 能力

### 修改的能力

- **pge-workflow-engine** — `PHASES` 数组顺序调整（05 在 04 之前）；`phase/next` 调度语义更新
- **workflow-orchestration** — `PHASE_PREREQUISITES` 与 `getDependents()` 依赖图更新；`phase_next` 阶段表与恢复/并行场景更新
- **pipeline-backtrack** — `propagateStale` 从 `05-implement` 出发的传播链更新（04-test-gen 成为直接 downstream）

---

## 变更范围

### 实现以下特性

- `plugins/dev-team/bin/src/lib/workflow.ts`：`PHASE_REQUIREMENT` 数组中 05-implement 置于 04-test-gen 之前
- `plugins/dev-team/bin/src/lib/workflow.ts`：`PHASE_PREREQUISITES['04-test-gen']` 增加 `'05-implement'`
- `plugins/dev-team/bin/src/lib/workflow.test.ts`：阶段顺序、索引、`getPrerequisites` / `getDependents` 断言更新
- `plugins/dev-team/bin/src/commands/phase-next.test.ts`（及相关测试）：`phase/next` 在 03 完成后返回 05、05 完成后返回 04 等场景更新
- `plugins/dev-team/.claude-plugin/plugin.json`：版本号升级

### 不要修改

- 阶段标识符编号（04-test-gen、05-implement 名称不变）
- bug-fix 工作流阶段表与前置依赖
- 各 phase skill 的 SKILL.md（gate check 逻辑不变，仍校验各自 phase ID）
- test-gen-generator / implementation-generator agent 定义（已具备读源码能力，顺序调整即可受益）
- eval.json schema 与 `stale` 传播算法本身
- DESIGN / EXEC / EVAL-ONLY 执行模式

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `PHASES` 数组顺序为 `[01-proposal, 02-dev-design, 03-test-design, 05-implement, 04-test-gen, 06-unit-test, ...]` | 读取 `workflow.ts` 或 `workflow.test.ts` |
| AC-2 | `getPrerequisites("04-test-gen")` 返回 `["03-test-design", "05-implement"]` | 单元测试 |
| AC-3 | `getPrerequisites("05-implement")` 仍返回 `["02-dev-design"]` | 单元测试 |
| AC-4 | `getDependents("05-implement")` 包含 `04-test-gen` 及 `[06-unit-test, 07-code-review, 08-integration-test, 09-acceptance]` | 单元测试 |
| AC-5 | `phase/next` 在 01–03 pass、05 未 pass 时返回 `05-implement`（而非 04-test-gen） | 构造 eval.json 单元测试 |
| AC-6 | `phase/next` 在 01–03 及 05 pass、04 未 pass 时返回 `04-test-gen` | 构造 eval.json 单元测试 |
| AC-7 | `phase/next` 在 05 pass 但 04 stale 时返回 `04-test-gen`（06 需要两者均有效 pass） | 构造 eval.json 单元测试 |
| AC-8 | `propagateStale("05-implement")` 直接标记 `04-test-gen` 及 06/07/08/09 为 stale | 单元测试 |
| AC-9 | `propagateStale("03-test-design")` 仍标记 04 及下游 stale，**不**标记 05-implement | 单元测试 |
| AC-10 | bug-fix 工作流行为不变 | 现有 bug-fix 相关测试通过 |
| AC-11 | 现有 eval.json（04 pass 在 05 之前写入）在下次 `phase/next` 时因 04 前置 05 未满足而要求重跑 04 或先补 05 | 手动/集成验证（文档说明） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 进行中变更的 eval.json 已有 04 pass 但无 05 pass | `phase/next` 可能阻塞或要求重跑 test-gen | 低 | 04 新前置含 05；无 05 pass 时 04 视为未完成；在 proposal 中说明手动补跑 05 后重跑 04 |
| 测试断言大量依赖旧阶段索引 | CI 失败 | 中 | 同步更新 `workflow.test.ts` 与 `phase-next.test.ts` 中所有 04/05 索引与顺序断言 |
| 并行轨道文档与实现不一致 | 开发者误解执行顺序 | 低 | 更新 spec 中依赖图与 phase 表；本 proposal 明确新顺序 |
| test-gen 在实现后生成可能覆盖手写测试 | 意外 diff | 低 | test-gen-generator 已有 skip/TODO 标记约束；evaluator 仍对照 test-design.md 校验 |
