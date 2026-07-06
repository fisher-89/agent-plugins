# 任务: backtrack-reason-propagation

## 阶段 1: Schema 层 — 定义字段

- [x] 在 `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` 的 `phaseLogSchema` 对象中添加 `backtrack_reason` 字段：`z.string().max(500).optional().nullable().describe('回溯原因（backtrack_to 非空时必填，最长 500 字符）')`
- [x] 在 `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` 的 `phaseLogInputSchema` 的 `.pick()` 列表中添加 `backtrack_reason: true`

## 阶段 2: 存储层 — 写入 eval.json

- [x] 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 的 `BuildEntryParams` 类型 Pick 列表中添加 `'backtrack_reason'`
- [x] 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 的 `buildEntry()` 函数中，将 `params.backtrack_reason` 写入 entry 对象：`backtrack_reason: params.backtrack_reason !== undefined ? params.backtrack_reason : null`

## 阶段 3: 校验层 — phase-log 强校验

- [x] 在 `plugins/dev-team/bin/src/commands/phase-log.ts` 的 `runPhaseLog()` 中，在 `handleBacktrackMarking()` 之前或之后增加校验：当 `options.backtrack_to` 非空（不为 null/undefined/空字符串）时，`options.backtrack_reason` 必须存在且不能为空字符串，否则抛出明确错误信息
- [x] 在 `plugins/dev-team/bin/src/commands/phase-log.ts` 的 `runPhaseLog()` 的 `buildEntry()` 调用中，传入 `backtrack_reason: options.backtrack_reason ?? null`

## 阶段 4: 读取层 — 重构 getLatestBacktrackTarget

- [x] 在 `plugins/dev-team/bin/src/commands/phase-next.ts` 中将 `getLatestBacktrackTarget()` 函数重命名为 `getLatestBacktrackInfo()`，返回类型改为 `{ target: string | string[] | null, reason: string | null }`；reason 值为 `latest.backtrack_reason ?? null`（兼容旧条目无该字段的情况）
- [x] 更新 `plugins/dev-team/bin/src/commands/phase-next.ts` 中所有调用 `getLatestBacktrackTarget()` 的地方为 `getLatestBacktrackInfo()`，并使用解构获取 `target` 和 `reason`

## 阶段 5: Prompt 层 — 回溯原因拼接

- [x] 修改 `plugins/dev-team/bin/src/commands/phase-next.ts` 的 `buildPhaseDef()` 函数签名，增加可选参数 `backtrackReason?: string | null`
- [x] 在 `buildPhaseDef()` 中，当 `backtrackReason` 非空时，生成回溯原因后缀字符串 `\n\n⚠️ 回溯原因: ${backtrackReason}`；将其拼接到 planner prompt 末尾（即 `interpolatePrompt(def.planner.prompt, change, def.id) + reasonSuffix`）和 evaluator prompt 末尾（即 `interpolatePrompt(def.evaluator.prompt, change, def.id) + backtrackHint + reasonSuffix`）
- [x] 修改 `plugins/dev-team/bin/src/commands/phase-next.ts` 的 `buildPhaseResponse()` 函数签名，增加可选参数 `backtrackReason?: string | null`，并将其透传给 `buildPhaseDef()`
- [x] 在 `plugins/dev-team/bin/src/commands/phase-next.ts` 的 `handleBacktrack()` 函数中，从 `getLatestBacktrackInfo()` 获取 `reason`，并将其传入 `buildPhaseResponse()` 调用
