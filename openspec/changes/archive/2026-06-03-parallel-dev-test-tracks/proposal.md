# 提案: parallel-dev-test-tracks

> **变更**: parallel-dev-test-tracks
> **日期**: 2026-06-02
> **状态**: 提案中

---

## 问题

当前 PGE 工作流中 `phase-check` 和 `phase-next` 的三个核心逻辑都基于"线性顺序"假设：
- **gate-check**: 每个 phase 依赖**所有**前面的 phase 都 pass——`getPriorPhases()` 返回 `PHASES.slice(0, idx)`
- **timestamp-check**: 检查所有前面 phase 的时间戳单调递增
- **backtrack-clear**: `clearEntriesFromPhase()` 根据线性索引清除目标及之后**所有** entries

这导致两个结构性限制：

1. **轨道无法并行**：`02-dev-design` 和 `03-test-design` 本可并行（都只依赖 `01-proposal`），但 gate-check 强制线性依赖
2. **回溯范围过大**：回溯到 `02-dev-design` 时清除了包括 `03-test-design`/`04-test-gen` 的所有条目，即使 test 轨道未受影响

---

## 提案

三个核心变更：

**1. stale 标记机制替代 entry 删除**
- eval.json entries **永不删除**（完整审计轨迹）
- 新增 `stale` 字段标记失效条目
- 回溯时标记目标 phase 失效，沿依赖图**向下游传播失效**

**2. 前置依赖图替代线性顺序**
- 每个 phase 声明真正的依赖（prerequisites），不再依赖"所有前面的 phase"

**3. phase/check 合并到 phase/next**
- 不再需要独立的 gate-check 步骤
- "检查"逻辑简化为：**当前步骤是否与 phase/next 返回值匹配？**
- phase/next 是唯一的决策入口

### 依赖模型

```
                    ┌──────────────────────────────────────────────────────┐
                    │                                                      │
  ┌──────┐          │         ┌─ Dev Track ───────────────────────────┐    │
  │  01  │──────────┼────────▶│ 02-dev-design ──▶ 05-implement       │    │
  │propos│          │         └────────│──────────────────────────────┘    │
  │  al  │          │                  │                                    │
  │      │          │         ┌────────│──────────────────────────────┐    │
  │      │──────────┼────────▶│ 03-test-design ──▶ 04-test-gen       │    │
  └──────┘          │         └────────│──────────────│───────────────┘    │
                    │                  │              │                     │
                    │                  ▼              ▼                     │
                    │         ┌──────────────────────────────────────┐     │
                    │         │  06-unit-test  07-code-review        │     │
                    │         │  08-integration-test  (并行叶节点)    │     │
                    │         └──────────────────────────────────────┘     │
                    │                                                      │
                    └──────────────────────────────────────────────────────┘
                                          │
                                          ▼
                                   09-acceptance
                              (仅依赖 dev track)
```

**前置依赖表（prerequisites）**:

| Phase | 前置依赖 |
|-------|---------|
| 01-proposal | (无) |
| 02-dev-design | 01-proposal |
| 03-test-design | 01-proposal, 02-dev-design |
| 04-test-gen | 03-test-design |
| 05-implement | 02-dev-design |
| 06-unit-test | 04-test-gen, 05-implement |
| 07-code-review | 04-test-gen, 05-implement |
| 08-integration-test | 04-test-gen, 05-implement |
| 09-acceptance | 01-proposal, 02-dev-design, 05-implement |

### 三个机制

#### 机制 A: 标记失效 → 立即向下游传播

当 evaluator 调用 `phase/log` 并设置 `backtrack_to = "02-dev-design"` 时，phase/log 调用 `markPhaseStale("02-dev-design")`，该函数：

1. 标记 02 的最新 pass 条目为 `stale: true`
2. **立即沿依赖图向下游传播**：标记所有后置 dependents 为 stale（传递闭包）

```
eval.json（回溯前）:
  [{phase:"01", verdict:"pass"}, {phase:"02", verdict:"pass"}, {phase:"03", verdict:"pass"},
   {phase:"04", verdict:"pass"}, {phase:"05", verdict:"pass"}, ...]

phase/log 收到 backtrack_to="02-dev-design" → 调用 markPhaseStale("02-dev-design"):
  → 标记 02 pass 条目 stale: true
  → dependents(02) = [03, 05, 09] → 标记 03,05,09 所有条目 stale
    → dependents(03) = [04] → 标记 04 stale
      → dependents(04) = [06,07,08] → 标记 06,07,08 stale
    → dependents(05) = [06,07,08,09] → 标记 06,07,08,09 stale

eval.json（回溯后）:
  [{phase:"01", verdict:"pass"}, {phase:"02", verdict:"pass", stale:true}, {phase:"03", verdict:"pass"},
   {phase:"04", verdict:"pass"}, {phase:"05", verdict:"pass", stale:true},
   {phase:"06", verdict:"pass", stale:true}, ..., {phase:"05", verdict:"fail", backtrack_to:"02-dev-design"}]
```

#### 机制 B: phase/next → 过滤失效条目

`hasPhasePassed()` 改为：该 phase 存在**未失效的 pass 条目**。

```
hasPhasePassed(entries, phaseId):
  return entries.some(e => e.phase === phaseId && (e.verdict === 'pass' || e.skipped) && !e.stale)
```

#### 机制 C: 依赖图驱动

失效传播沿**前置依赖图的逆向**（dependents）进行。轨道隔离由依赖图自然保证——dev 和 test 互不为 dependents。

```
Forward (prerequisites):              Reverse (dependents):
  01 → []                               01 → [02, 03, 09]
  02 → [01]                             02 → [03, 05, 09]
  03 → [01, 02]                         03 → [04]
  04 → [03]                             04 → [06, 07, 08]
  05 → [02]                             05 → [06, 07, 08, 09]
  06 → [04, 05]                         06 → []
  07 → [04, 05]                         07 → []
  08 → [04, 05]                         08 → []
  09 → [01, 02, 05]                     09 → []
```

### 场景推演

**新的 workflow 循环（phase/check 已合并到 phase/next）**:

```
Loop:
  result = phase/next(change)
  if result.done → 完成
  if result.error → 停止报错
  执行 result.next_phase（planner → evaluator → phase/log）
  // 不需要独立 phase/check — phase/next 本身就是决策
```

"检查"逻辑变为隐式：如果你想执行 phase X，但 phase/next 返回 phase Y，说明 X 的前置条件不满足（stale、未 pass、或存在回溯），应以 phase/next 为准。

**场景 1: 首次执行（无回溯）**

```
01 pass → phase/log: 无操作（markPhaseStale 未调用）
02 pass → phase/log: 无操作
...
```
首次执行完全不触发 stale 操作。

**场景 2: 05-implement 失败，回溯到 02-dev-design**

```
初始: 01✓ 02✓ 03✓ 04✓ 05✗(backtrack_to=02)

phase/log 调用 markPhaseStale("02-dev-design"):
  → 02 stale
  → dependents(02)=[03,05,09] → 03,05,09 stale
    → 03 stale → dependents(03)=[04] → 04 stale
      → 04 stale → dependents(04)=[06,07,08] → 06,07,08 stale
    → 05 stale → dependents(05)=[06,07,08,09] → 全部已 stale

结果: 仅 01 保持有效，02-09 全部 stale
(因为 test-design 依赖 dev-design，dev 变更导致 test 轨道也需要重新评估)

phase/next → 02
02 pass → 03 pass → 04 pass → 05 pass → 06/07/08 并行 → 09
```

**场景 3: 两个轨道都需要重做（backtrack_to 数组）**

```
backtrack_to = ["02-dev-design", "03-test-design"]
  → markPhaseStale("02"): 02 stale, 传播: 05,06,07,08,09 stale
  → markPhaseStale("03"): 03 stale, 传播: 04,06,07,08,09 stale
  → 全部传播在标记时一次完成

phase/next → 02 (第一个无有效 pass)
02 pass → 无传播
phase/next → 03
03 pass → 无传播
phase/next → 04
04 pass → 无传播
...
```

---

## 变更范围

### 实现以下特性

| # | 文件 | 改动 |
|---|------|------|
| 1 | `workflow.ts` | 新增 `PHASE_PREREQUISITES` 表 + `getPrerequisites(phase)` + `getDependents(phase)` |
| 2 | `eval-json.ts` | `checkGate()` 改为按 prerequisites 检查 + 过滤 `stale: true` |
| 3 | `eval-json.ts` | 新增 `propagateStale(entries, phaseId)` — 沿 dependents 图递归标记失效（由 `markPhaseStale` 内部调用） |
| 4 | `eval-json.ts` | 新增 `markPhaseStale(entries, phaseId)` — 标记最新 pass 为 stale 并**立即**向下游传播 |
| 5 | `phase-log.ts` | `runPhaseLog()` 写含 `backtrack_to` 条目后调用 `markPhaseStale()`；pass 条目无需额外操作 |
| 6 | `phase-next.ts` | `hasPhasePassed()` 过滤 `stale: true` 条目 |
| 7 | `phase-next.ts` | 移除 `clearEntriesFromPhase()` 及相关的 entry 删除逻辑 |
| 8 | `phase-next.ts` | 移除对 `phase/check` 的依赖，phase/next 自身就是唯一的决策入口 |
| 9 | `schemas/` | `backtrack_to` 支持 `string | string[]` |
| 10 | `skills/` | workflow 循环移除 `phase/check` 调用，改为纯 `phase/next` 驱动 |

### 不要修改

- PHASES 数组的排序和内容
- bug-fix / refactor 工作流
- DESIGN/EXEC/EVAL-ONLY 执行模式
- agent 定义和 prompt 模板
- skill 文件
- `getPriorPhases()` 函数（保留向后兼容）
- eval.json 格式兼容：旧条目无 `stale` 字段 → 视为 `stale: false`

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | `getPrerequisites("02-dev-design")` 返回 `["01-proposal"]` | 单元测试 | P0 |
| AC-2 | `getPrerequisites("03-test-design")` 返回 `["01-proposal", "02-dev-design"]` | 单元测试 | P0 |
| AC-3 | `getPrerequisites("07-code-review")` 返回 `["04-test-gen", "05-implement"]`（与 06-unit-test 相同前置，可并行） | 单元测试 | P0 |
| AC-4 | `getDependents("02-dev-design")` 返回 `["03-test-design", "05-implement", "09-acceptance"]` | 单元测试 | P0 |
| AC-5 | phase/next 在 `03-test-design` 无有效 pass 且 `02-dev-design` 未 pass 时，返回 `02-dev-design`（test-design 依赖 dev-design） | 构造 eval.json(01✓, 02✗)，调用 phase/next → next_phase=02-dev-design | P0 |
| AC-6 | phase/next 在 `05-implement` 无有效 pass 且 `02-dev-design` pass 时，返回 `05-implement`，不会因为 `03`/`04` 未 pass 而跳过 | 构造 eval.json(01✓, 02✓)，调用 phase/next → next_phase=05-implement | P0 |
| AC-7 | phase/next 在 `06-unit-test` 无有效 pass 且 `04-test-gen` stale 时，返回 `04-test-gen`（而非 06） | 构造 eval.json(01-05 全 pass 但 04 stale=true)，调用 phase/next → next_phase=04-test-gen | P0 |
| AC-8 | `markPhaseStale("02-dev-design")` 标记 02 stale 并立即传播到所有下游：05,06,07,08,09 stale | 构造完整条目，调用 markPhaseStale("02")，验证 05-09 全部 stale=true, 03/04 stale=false | P0 |
| AC-9 | phase/log 含 `backtrack_to: "02-dev-design"` 时自动调用 `markPhaseStale` | 构造 eval.json(01✓,02✓,03✓,04✓,05✓)，log fail + backtrack_to=02 → 验证 02 stale 且 05 stale（已传播） | P0 |
| AC-10 | phase/log 含 `backtrack_to: ["02","03"]` 数组时：分别调用 `markPhaseStale`，各自传播 | 构造 eval.json，验证 02 和 03 都 stale，且两个下游链都传播 | P0 |
| AC-11 | `hasPhasePassed()` 忽略 stale 条目 | 构造 02 有 pass 但 stale=true → hasPhasePassed(02)=false | P0 |
| AC-12 | 旧条目无 `stale` 字段 → 视为 `stale: false`（向后兼容） | 用旧格式 eval.json 运行 phase/next → 行为不变 | P1 |
| AC-13 | bug-fix 工作流不受影响 | bug-fix 全流程单元测试通过 | P1 |
| AC-14 | 失效传播为传递闭包：markPhaseStale(02) → 02 stale → 05 stale → 06 stale → ... → 09 stale | 构造完整条目，调用 markPhaseStale("02")，验证 02-05-06-07-08-09 全部 stale | P0 |
| AC-15 | 移除 `clearEntriesFromPhase()` 及相关逻辑 | 代码审查 + 单元测试 | P1 |
| AC-16 | `backtrack_to` 保持字符串格式向后兼容 | 字符串 `"02-dev-design"` 与 `["02-dev-design"]` 行为一致 | P1 |
| AC-17 | workflow 循环不再调用 phase/check，仅通过 phase/next 决定下一步 | 审查 skill 文件：无 phase/check 调用；phase/next 是唯一决策点 | P0 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 失效传播遗漏导致状态不一致 | 本应失效重跑的 phase 被跳过 | 中 | `propagateStale()` 有完整单元测试覆盖所有依赖链；传递闭包算法明确 |
| 旧 eval.json 无 stale 字段与新逻辑冲突 | 现有变更 gate-check 异常 | 低 | `!e.stale` 对 `undefined` 返回 `true`（视为未失效）；向后兼容 |
| 递归传播性能 | 大项目 eval.json 条目多时传播慢 | 低 | 依赖图最多 9 层深度；条目数有限（<100）；传播为 O(n) 遍历 |
| `backtrack_to` 数组包含无效 phase ID | 标记失效时找不到目标 | 低 | phase/log 输入校验阶段捕获无效 phase ID |

---

## 能力

### 修改的能力

- **workflow-orchestration** — 新增 `getPrerequisites()`/`getDependents()` 函数和依赖图数据结构；`phase/log` 新增失效传播逻辑；`phase/next` 移除 entry 删除，改用失效过滤；workflow 循环移除 `phase/check` 调用，改为纯 `phase/next` 驱动
- **pge-workflow-engine** — `phase-check.ts` 的 gate-check 逻辑合并到 `phase-next.ts`；内部仍保留 `checkGate()` 工具函数供 `phase/next` 使用
- **pipeline-backtrack** — 回溯机制从"清除条目"改为"标记失效"；`backtrack_to` 支持数组格式
