# 测试设计: reorder-test-gen-after-implement

> **日期**: 2026-06-22

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `PHASES` 数组顺序为 `[01-proposal, 02-dev-design, 03-test-design, 05-implement, 04-test-gen, 06-unit-test, ...]` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASES` / `getPhaseIndex` / `getPriorPhases` |
| AC-1 | 同上（phase 表与 `PHASES` 一致） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` |
| AC-2 | `getPrerequisites("04-test-gen")` 返回 `["03-test-design", "05-implement"]` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` / `PHASE_PREREQUISITES` |
| AC-3 | `getPrerequisites("05-implement")` 仍返回 `["02-dev-design"]` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` |
| AC-4 | `getDependents("05-implement")` 包含 `04-test-gen` 及 `[06-unit-test, 07-code-review, 08-integration-test, 09-acceptance]` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` |
| AC-5 | `phase/next` 在 01–03 pass、05 未 pass 时返回 `05-implement`（而非 04-test-gen） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Skip Passed Phases` / `runPhaseNext — Normal Progression` |
| AC-6 | `phase/next` 在 01–03 及 05 pass、04 未 pass 时返回 `04-test-gen` | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` / `runPhaseNext — Skip Passed Phases` |
| AC-7 | `phase/next` 在 05 pass 但 04 stale 时返回 `04-test-gen`（06 需要两者均有效 pass） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Stale Entry Filtering` / `runPhaseNext — Dependency-Graph Driven` |
| AC-8 | `propagateStale("05-implement")` 直接标记 `04-test-gen` 及 06/07/08/09 为 stale | 单元测试 | `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` |
| AC-9 | `propagateStale("03-test-design")` 仍标记 04 及下游 stale，**不**标记 05-implement | 单元测试 | `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` |
| AC-10 | bug-fix 工作流行为不变 | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` / `getDependents`（bug-fix） |
| AC-10 | bug-fix 工作流 phase 表与调度不变 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` / `PHASE_TABLES` |
| AC-11 | 现有 eval.json（04 pass 在 05 之前写入）在下次 `phase/next` 时因 04 前置 05 未满足而要求重跑 04 或先补 05 | 不可测试项 | — | 见「不可测试项」 |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASES` | 正向 | should have correct order — 05-implement 在 04-test-gen 之前（AC-1） | 废弃 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASES` | 正向 | should have correct order — 期望数组为 `[…, 03-test-design, 05-implement, 04-test-gen, 06-unit-test, …]`（AC-1） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseIndex` | 正向 | should return 3 for 05-implement（AC-1） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseIndex` | 正向 | should return 4 for 04-test-gen（AC-1） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseIndex` | 废弃 | should return correct index for 05-implement — 旧断言 index=4 | 废弃 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseIndex` | 正向 | should return 5 for 06-unit-test（索引随 04/05 对调后仍正确） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPriorPhases` | 正向 | should return prior phases for 06-unit-test — 顺序含 05-implement 在 04-test-gen 之前（AC-1） | 废弃 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPriorPhases` | 正向 | should return `[01, 02, 03, 05-implement, 04-test-gen]` for 06-unit-test（AC-1） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_PREREQUISITES` | 正向 | should have `[03-test-design, 05-implement]` as prerequisites for 04-test-gen（AC-2） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 正向 | should return `["03-test-design", "05-implement"]` for 04-test-gen requirement（AC-2） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 正向 | should return `["02-dev-design"]` for 05-implement requirement（AC-3） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 边界 | should return `[]` for unknown phase 99-unknown（fault-tolerant） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | should return `[04-test-gen, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance]` for 05-implement（AC-4） | 废弃 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | should return `[04-test-gen, 06, 07, 08, 09]` for 05-implement — 04 为直接 downstream（AC-4） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | should return `[04-test-gen]` for 03-test-design（03 不依赖 05，05 非其 downstream） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 正向 | should return `[06, 07, 08]` for 04-test-gen（不变） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` | 边界 | should return `[]` for unknown phase 99-unknown | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 正向 | should return `["02-dev-design"]` for 05-implement bug-fix（AC-10） | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 正向 | should return `["07-code-review"]` for 09-acceptance bug-fix（AC-10） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 正向 | should include all 9 phases in order — 05 在 04 之前（AC-1） | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 正向 | phase 表 id 顺序与 AC-1 一致（AC-1） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | should return 05-implement after phases 01-03 pass（AC-5） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 废弃 | should return 05-implement after phases 01-04 pass — 旧顺序假设 | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | should return 04-test-gen after phases 01-03 and 05 pass（AC-6） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | should return 06-unit-test after phases 01-05 and 04 pass | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Skip Passed Phases` | 正向 | should skip to 05-implement when phases 01-03 already pass（AC-5） | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Skip Passed Phases` | 正向 | should skip to 05-implement when 01-03 pass — 非 04-test-gen（AC-5） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Skip Passed Phases` | 正向 | should skip to 06-unit-test when phases 01-05 and 04 pass | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Stale Entry Filtering` | 正向 | should return 04-test-gen when 05 pass but 04 is stale（AC-7） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Dependency-Graph Driven` | 正向 | should return 05-implement when 04 pass but 05 not passed — 线性扫描优先 05（AC-11 子场景） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Dependency-Graph Driven` | 废弃 | should skip stale 04 and return 04 when 04 stale even though later phases pass — 需确认 05 在 04 前扫描的语义仍成立 | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 正向 | should advance past skipped phases — 01-04 pass/skip 后下一 phase 为 05 的旧断言改为 05 在 04 前逻辑 | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 正向 | 01-03 pass + skip 04 时仍返回 05-implement（05 未 pass） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | bug-fix phase table 仍为 6 个 phase，不含 03/04/08（AC-10） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | bug-fix 在 07 pass 后返回 09-acceptance（AC-10） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` | 正向 | should mark 04-test-gen stale when marking 05-implement — 04 为直接 downstream（AC-8） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` | 正向 | should propagate from 05-implement to 06/07/08/09（AC-8） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` | 正向 | should mark 04 and downstream stale when marking 03-test-design（AC-9） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` | 正向 | should NOT mark 05-implement stale when marking 03-test-design（AC-9） | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` | 边界 | should be no-op when no pass entry exists for target phase | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` | 边界 | should mark latest of multiple pass entries only | 新增 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | `markPhaseStale` | 边界 | should not mark same-phase new entry written after markPhaseStale | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` | 边界 | refactor workflow_type 与 requirement 对 04/05 前置一致 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `resolvePhaseNext — Input Validation` | 边界 | should not throw for empty entries array | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseIndex` | 边界 | should return -1 for unknown phase 99-unknown | 新增 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseIndex` | 边界 | should return -1 for old phase name 05-implementation | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | 无外部依赖 | 直接 import `workflow.ts` 导出，纯函数断言 | 全部 `describe` |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | eval.json 条目 | 使用 `passEntry` / `failEntry` / `staleEntry` / `skippedEntry` 等内存构造 `EvalEntry[]`，调用 `resolvePhaseNext({ change, entries, workflowType })`；不读写磁盘 | `runPhaseNext — *`、`PHASE_TABLES`、`Boundary Scenarios` |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | eval.json 条目 | 使用 `makePassEntry` / `makeFailEntry` 构造内存数组，调用 `markPhaseStale(entries, phaseId)`；`propagateStale` 为内部函数，通过 `markPhaseStale` 间接验证 | `markPhaseStale` 全部用例 |

---

## 集成测试

本变更无跨进程/跨服务集成场景；`phase/next` 与 `markPhaseStale` 的协作已在单元测试中通过内存 mock entries 覆盖。不新增 `__tests__/` 集成测试文件。

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| — | — | — | 无集成测试场景 | — |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| — | — | — | — |

---

## 不可测试项

- **AC-11（进行中变更 eval.json 兼容）** — **原因**: 依赖真实变更目录下历史 `eval.json` 文件与人工工作流补跑（先完成 05 再重跑 04）；无法在 Vitest 中稳定复现「生产环境已写入的 04 pass 时序」。可通过 `phase-next.test.ts` 中「04 pass 但 05 未 pass → 返回 05-implement」单元场景部分覆盖调度语义，但完整迁移路径需 proposal/design 文档说明与手动验证。

- **`plugin.json` 版本号升级** — **原因**: 配置元数据变更，无运行时行为断言；通过 code review 与发布流程验证。
