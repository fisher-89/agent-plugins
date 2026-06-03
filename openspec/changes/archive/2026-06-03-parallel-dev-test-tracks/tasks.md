# 实现任务: parallel-dev-test-tracks

## 阶段 1: 核心依赖数据结构 (workflow.ts)

- [x] 在 `workflow.ts` 中新增 `PHASE_PREREQUISITES` 常量，定义 requirement workflow_type 的完整前置依赖表（01-proposal:[]，02-dev-design:[01-proposal]，03-test-design:[01-proposal, 02-dev-design]，04-test-gen:[03-test-design]，05-implement:[02-dev-design]，06-unit-test:[04-test-gen, 05-implement]，07-code-review:[04-test-gen, 05-implement]，08-integration-test:[04-test-gen, 05-implement]，09-acceptance:[01-proposal, 02-dev-design, 05-implement]）
- [x] 在 `workflow.ts` 中新增 `PHASE_BUG_FIX_PREREQUISITES` 常量，定义 bug-fix workflow_type 的依赖表（01-proposal:[]，02-dev-design:[01-proposal]，05-implement:[02-dev-design]，06-unit-test:[05-implement]，07-code-review:[05-implement]，09-acceptance:[07-code-review]）
- [x] 在 `workflow.ts` 中新增 `getPrerequisites(phaseId: string, workflowType?: string): string[]` 函数——返回指定 phase 在指定 workflow 中的前置依赖列表
- [x] 在 `workflow.ts` 中新增 `getDependents(phaseId: string, workflowType?: string): string[]` 函数——从 `getPrerequisites()` 推导反向依赖图，返回后置 phase 列表
- [x] 为 `getPrerequisites()` 和 `getDependents()` 编写单元测试（AC-1, AC-2, AC-3, AC-4）

## 阶段 2: 失效传播机制 (eval-json.ts)

- [x] 在 `eval-json.ts` 中新增 `propagateStale(entries: any[], phaseId: string, workflowType?: string): void` 函数——递归标记所有下游 dependent 的**所有**条目为 `stale: true`；使用 visited-set 防止无限循环；如果 dependent 无条目则跳过不报错
- [x] 在 `eval-json.ts` 中新增 `markPhaseStale(entries: any[], phaseId: string): void` 函数——查找指定 phase 的最新 pass 条目并标记 `stale: true`；若找到则立即调用 `propagateStale()` 向下传播；若无 pass 条目则为 no-op
- [x] 修改 `eval-json.ts` 中 `checkGate()` 函数——将签名从 `(entries, priorPhases)` 改为 `(entries, prerequisites)`，并在检查时过滤 `stale: true` 条目（`!e.stale` 对 `undefined` 返回 `true`）

## 阶段 3: Schema 更新 (phase-log.schema.ts, phase-next.schema.ts)

- [x] 修改 `phase-log.schema.ts` 中 `backtrack_to` 字段类型——从 `z.string().optional()` 改为 `z.union([z.string(), z.array(z.string())]).optional()`（支持字符串或字符串数组）
- [x] 在 `phase-log.schema.ts` 的 input schema 中确认新条目 `stale` 字段不由输入提供（由 `markPhaseStale` 内部设置）
- [x] 确认 `phase-next.schema.ts` 无需修改（phase/next 输入/输出结构不变）

## 阶段 4: phase/log 失效传播集成

- [x] 修改 `phase-log.ts` 中 `runPhaseLog()` 函数——在写入新条目前，若 `backtrack_to` 非空，调用 `markPhaseStale()` 对每个目标处理（字符串转单元素数组统一处理）
- [x] 修改 `phase-log.ts` 中 `runPhaseLog()`——移除写入前的 `checkGate()` 调用（gate 逻辑完全由 phase/next 负责）
- [x] 修改 `phase-log.ts` 中 `runPhaseLog()`——在写入新条目前，对 `markPhaseStale()` 调用的所有目标进行无效 phase ID 校验
- [x] 为 phase/log 的失效传播逻辑编写单元测试（AC-9, AC-10, AC-16）
- [x] 验证 phase/log 写入 pass 条目时不触发任何传播（AC-9 反向验证）

## 阶段 5: phase/next 只读化 + stale 过滤

- [x] 修改 `phase-next.ts` 中 `hasPhasePassed()`——新增 `!e.stale` 过滤条件（`undefined` 视为 `false`）
- [x] 修改 `phase-next.ts` 中 `resolvePhaseNext()`——在 phase 扫描阶段改用 `getPrerequisites()` 替代隐含的线性顺序逻辑（不是简单地找第一个未 pass phase，而是找所有前置依赖都已 pass 的第一个 phase）
- [x] 修改 `phase-next.ts` 中 `resolvePhaseNext()` 的回溯处理——不再调用 `clearEntriesFromPhase()`，直接返回最早的目标 phase（字符串/数组都取最小 phase_index）
- [x] 移除 `phase-next.ts` 中的 `clearEntriesFromPhase()` 函数
- [x] 修改 `phase-next.ts` 中 `resolvePhaseNext()`——移除 `updatedEntries` 返回值
- [x] 修改 `phase-next.ts` 中 `runPhaseNext()`——移除 `writeEvalJson()` 调用
- [x] 为 phase/next 的依赖图驱动逻辑编写单元测试（AC-5, AC-6, AC-7, AC-11, AC-12, AC-14, AC-15）

## 阶段 6: phase/check 弃用

- [x] 在 `phase-check.ts` 中移除 `checkTimestampOrder()` 函数
- [x] 在 `phase-check.ts` 中移除 `checkBacktrack()` 函数
- [x] 在 `phase-check.ts` 中修改 `buildPhaseCheckResult()`——移除非必要的 timestamp/backtrack 字段
- [x] 在 `phase-check.ts` 中修改 `runPhaseCheck()`——保留整体函数供调试用，但更新内部逻辑仅执行前置检查
- [x] 在 `phase-check.schema.ts` 中更新输出 schema（移除 timestamp_order 和 backtrack 字段或改为可选）
- [x] 在 `mcp.ts` 中保留 `phase/check` 工具的注册但更新描述（标注 DEPRECATED）

## 阶段 7: 测试修复与完整验证

- [x] 修复 `phase-next.test.ts` 中依赖线性顺序假设的测试——更新为 `getPrerequisites()` 依赖图语义
  - 涉及 backtrack 部分的测试需改为验证 eval.json 不被修改（stale 条目保留在 entries 中）
  - 涉及 `clearEntriesFromPhase()` 的测试需移除或改为验证函数不存在
  - 涉及 `updatedEntries` 的测试需移除
- [x] 新增 `eval-json.test.ts` 中 `markPhaseStale()` 和 `propagateStale()` 的测试（AC-8, AC-14）
- [x] 新增 `eval-json.test.ts` 中 `checkGate()` 的修改后测试（过滤 stale 条目）
- [x] 验证 bug-fix 工作流单元测试全部通过（AC-13）
- [x] 验证所有现有测试在修改后仍通过
- [x] 验证 `getPriorPhases()` 函数仍可用且行为不变（AC-15 反向验证）

## 阶段 8: 清理与确认

- [x] 全面搜索代码中引用 `clearEntriesFromPhase` 的位置，确认全部移除
- [x] 确认 `resolvePhaseNext` 的返回类型不再包含 `updatedEntries`
- [x] 确认 `runPhaseNext` 不再执行任何 eval.json 写操作
- [x] 确认工作流 skill 文件中不再调用 `phase/check`
- [x] 确认旧格式 eval.json（无 `stale` 字段）在新逻辑下行为不变
