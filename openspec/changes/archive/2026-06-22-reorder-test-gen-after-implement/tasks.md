# 实现任务: reorder-test-gen-after-implement

## 阶段 1: workflow.ts 核心配置（AC-1, AC-2, AC-3, AC-4）

- [x] 在 `plugins/dev-team/bin/src/lib/workflow.ts` 的 `PHASE_REQUIREMENT` 数组中，将 `05-implement` 条目移到 `04-test-gen` **之前**（交换两元素位置，保留各自 `PhaseDefinition` 内容不变）
- [x] 更新 `PHASE_PREREQUISITES['04-test-gen']`：由 `['03-test-design']` 改为 `['03-test-design', '05-implement']`
- [x] 确认 `PHASE_REFACTOR`（引用 `PHASE_REQUIREMENT`）与 `PHASE_BUG_FIX` / `PHASE_BUG_FIX_PREREQUISITES` **无需修改**
- [x] 确认派生导出自动正确：`PHASES` 顺序为 `[01-proposal, 02-dev-design, 03-test-design, 05-implement, 04-test-gen, 06-unit-test, 07-code-review, 08-integration-test, 09-acceptance]`

## 阶段 2: workflow.test.ts 单元测试（AC-1, AC-2, AC-3, AC-4）

- [x] 更新 `PHASES` 顺序断言：`05-implement` 在 `04-test-gen` 之前
- [x] 更新 `getPhaseIndex('05-implement')` 断言：由 `4` 改为 `3`
- [x] 更新 `getPhaseIndex('04-test-gen')` 断言：由 `3` 改为 `4`
- [x] 更新 `getPhaseIndex('06-unit-test')` 等后续 phase 索引断言（如有硬编码）
- [x] 新增/更新 `getPrerequisites('04-test-gen')` 断言：返回 `['03-test-design', '05-implement']`（AC-2）
- [x] 确认 `getPrerequisites('05-implement')` 仍返回 `['02-dev-design']`（AC-3）
- [x] 更新 `getDependents('05-implement')` 断言：包含 `04-test-gen` 及 `[06-unit-test, 07-code-review, 08-integration-test, 09-acceptance]`（AC-4）
- [x] 确认 `getDependents('03-test-design')` 仍仅返回 `['04-test-gen']`
- [x] 确认 bug-fix 工作流相关测试仍通过（AC-10）

## 阶段 3: phase-next.test.ts 调度语义测试（AC-5, AC-6, AC-7）

- [x] 更新「03 完成后下一 phase」场景：01–03 pass 时期望 `05-implement`（非 `04-test-gen`）（AC-5）
- [x] 更新「01–03 及 05 pass、04 未 pass」场景：期望 `04-test-gen`（AC-6）
- [x] 更新「05 pass 但 04 stale」场景：期望 `04-test-gen`（AC-7）
- [x] 更新「04 pass 但 05 仅 stale / 未 pass」场景：期望 `05-implement`（04 前置 05 未满足）
- [x] 更新正常推进链测试：03 后 → 05 → 04 → 06（替换原 03 → 04 → 05 → 06 顺序）
- [x] 更新「Skip Passed Phases」测试中 01–03 pass 后 skip 目标为 `05-implement`
- [x] 更新「01–05 pass 后 skip 到 06」测试数据：需含 04 pass 条目
- [x] 审查并更新所有硬编码 phase 表顺序断言（`getPhaseTable` 返回顺序等）
- [x] 审查 backtrack 相关测试中涉及 04/05 顺序假设的用例

## 阶段 4: eval-json.test.ts 失效传播测试（AC-8, AC-9）

- [x] 新增 `propagateStale(entries, '05-implement')` 测试：直接标记 `04-test-gen` 及 06/07/08/09 为 stale，`01/02/03` 不受影响（AC-8）
- [x] 新增/确认 `propagateStale(entries, '03-test-design')` 测试：标记 04 及下游 stale，**不**标记 `05-implement`（AC-9）
- [x] 新增 `markPhaseStale(entries, '05-implement')` 测试：05 最新 pass 变 stale，04 作为直接 downstream 被标记 stale
- [x] 确认 `markPhaseStale(entries, '02-dev-design')` 现有测试仍通过（02 回溯仍传播到 05 和 03→04 链）

## 阶段 5: 验证与版本发布

- [x] 运行 `plugins/dev-team/bin` 下全部单元测试，确认通过
- [x] 确认 `phase-next.ts`、`eval-json.ts`、`phase-log.ts` **无需代码变更**（行为由 workflow 配置派生）
- [x] 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（当前 `2.6.26` → patch bump）
- [x] 手动验证 AC-11：构造 eval.json（04 pass、05 未 pass），确认 `phase/next` 返回 `05-implement`
