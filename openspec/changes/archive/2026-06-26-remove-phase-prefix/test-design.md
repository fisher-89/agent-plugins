# 测试设计: remove-phase-prefix

> **日期**: 2026-06-25

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | workflow.ts phase 表去前缀：`getPhaseTable('requirement')[0].id` 返回 `"proposal"`，而非 `"01-proposal"` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — requirement phase 表 id 去前缀 |
| AC-1 | workflow.ts phase 表去前缀：所有 workflow_type 的 phase id 均为无前缀格式 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` — 各 workflow_type phase 表 id 校验 |
| AC-2 | workflow.ts prerequisite 表去前缀：`getPrerequisites('implement', 'requirement')` 返回 `["dev-design"]` | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — requirement 依赖关系去前缀 |
| AC-3 | phase-next.ts `interpolatePrompt()` 支持 `<phase>` 占位符替换为当前 phase ID | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — `<phase>` 占位符替换 |
| AC-3 | phase-next.ts `buildPhaseDef()` 生成的 evaluator prompt 包含 `<phase>` 注入后的 phase ID | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — prompt 中 phase ID 动态注入 |
| AC-4 | 所有 phase SKILL.md 中 gate check 断言使用新 phase ID | 不可测试 | — | — 静态 Markdown 文件，通过 grep 全量验证 `0[0-9]-` 无残留 |
| AC-5 | 所有 evaluator agent .md 中 `phase_log` 调用的 phase 参数使用新 ID | 不可测试 | — | — 静态 Markdown 文件，通过 grep 全量验证 `0[0-9]-` 无残留 |
| AC-6 | 所有测试文件通过，phase ID 断言使用新格式 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | 全部 describe — 断言字符串从旧 ID 更新为新 ID |
| AC-6 | 所有测试文件通过，phase ID 断言使用新格式 | 单元测试 | `plugins/dev-team/bin/src/lib/workflow.test.ts` | 全部 describe — 断言字符串从旧 ID 更新为新 ID |
| AC-6 | 所有测试文件通过，phase ID 断言使用新格式 | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | 全部 describe — 断言字符串从旧 ID 更新为新 ID |
| AC-7 | `phase_next` 和 `phase_log` 的完整工作流正常执行（回归） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext` — 全部 progression/retry/backtrack 场景 |
| AC-7 | `phase_next` 和 `phase_log` 的完整工作流正常执行（回归） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog` — 全部 backtrack/pass/fail 场景 |
| AC-8 | `plugins/dev-team/.claude-plugin/plugin.json` 版本号已递增 | 不可测试 | — | — patch bump，人工审查 diff |
| AC-9 | workflow SKILL 无硬编码序号：全文搜索 workflow-requirement 和 workflow-test-only SKILL.md 不包含 `Total phases: 6` 等硬编码数字 | 不可测试 | — | — 静态 Markdown 文件，通过 grep 全量验证无硬编码序号 |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — requirement phase 表 id 去前缀 | 正向 | requirement 首 phase id 应为 `"proposal"` 而非 `"01-proposal"`（AC-1） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — requirement phase 表 id 去前缀 | 正向 | requirement 全部 phase id 顺序为 proposal → dev-design → test-design → implement → test-gen → unit-test → code-review → integration-test → acceptance（AC-1） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — bug-fix phase 表 | 正向 | bug-fix phase 表长度与 id 映射正确（AC-1/AC-10 回归） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — refactor phase 表 | 正向 | refactor phase 表与 requirement 一致（AC-10 回归） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — test-only phase 表 | 正向 | test-only phase 表 id 为 proposal → code-analyze → test-design → test-gen → unit-test → integration-test（AC-1） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — test-only 排除 phase | 正向 | test-only 不含 dev-design / implement / code-review / acceptance（AC-1） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — test-only code-analyze agent | 正向 | 02-code-analyze 使用 code-analyze-planner 与 code-analyze-evaluator agent_type | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — requirement 依赖去前缀 | 正向 | getDependents("proposal") 返回 ["dev-design", "test-design", "acceptance"]（AC-2） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — requirement 依赖去前缀 | 正向 | getDependents("dev-design") 返回 ["test-design", "implement", "acceptance"]（AC-2） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — requirement 依赖去前缀 | 正向 | getDependents("test-design") 返回 ["test-gen"]（AC-2） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — requirement 依赖去前缀 | 正向 | getDependents("test-gen") 返回 ["unit-test", "code-review", "integration-test"]（AC-2） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — implement 的 downstream | 正向 | getDependents("implement") 返回 ["test-gen", "unit-test", "code-review", "integration-test", "acceptance"]（AC-2） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — leaf phase | 边界 | getDependents("unit-test") / getDependents("code-review") / getDependents("integration-test") / getDependents("acceptance") 返回 []（AC-2） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — unknown phase | 边界 | getDependents("unknown") 返回 []（兼容容错） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — requirement 默认 workflow_type | 正向 | getDependents("dev-design") 无 workflow_type 参数时默认为 requirement 返回正确值 | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — test-only | 正向 | test-only getDependents("proposal") 返回 ["code-analyze", "test-design"] | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — test-only | 正向 | test-only getDependents("code-analyze") 返回 ["test-design"] | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — test-only leaf | 边界 | test-only getDependents("unit-test") / getDependents("integration-test") 返回 [] | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getDependents` — test-only unknown | 边界 | test-only getDependents("unknown") 返回 [] | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` — test-only | 正向 | test-only getPrerequisites("test-design") 返回 ["proposal", "code-analyze"] | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` — test-only | 正向 | test-only getPrerequisites("test-gen") 返回 ["test-design"] | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPrerequisites` — test-only leaf | 正向 | test-only getPrerequisites("unit-test") / getPrerequisites("integration-test") 返回 ["test-gen"] | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_TEST_ONLY — prompt customization` | 正向 | proposal planner prompt 引导测试覆盖缺口，与 requirement proposal prompt 有差异 | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_TEST_ONLY — WORKFLOW_CONTEXT prompts` | 正向 | 06-unit-test evaluator prompt 含 WORKFLOW_CONTEXT 自适应指令 | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `PHASE_TEST_ONLY — WORKFLOW_CONTEXT prompts` | 正向 | 08-integration-test evaluator prompt 含 WORKFLOW_CONTEXT 自适应指令 | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — regression | 正向 | bug-fix phase 表长度与 id 列表不变（AC-10 回归） | 修改 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | `getPhaseTable` — regression | 正向 | refactor phase 表与 requirement 一致（AC-10 回归） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 正向 | requirement 共 9 个 phase，首 phase id 为 `"proposal"`（AC-1） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 正向 | requirement phase 表顺序为 proposal→dev-design→test-design→implement→test-gen→unit-test→code-review→integration-test→acceptance（AC-1） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 正向 | bug-fix 共 6 个 phase，不含 test-design/test-gen/integration-test | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 正向 | refactor 与 requirement 一致 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 正向 | test-only 共 6 个 phase | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `PHASE_TABLES` | 边界 | 未知 workflow_type 默认使用 requirement 表 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — First Run` | 正向 | 空 entries 时 next_phase 为 `"proposal"`（AC-1） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — First Run` | 正向 | planner agent_type 为 dev-team:proposal-planner | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — First Run` | 正向 | evaluator agent_type 为 dev-team:proposal-evaluator | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — First Run` | 正向 | round 为 1 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — First Run` | 正向 | phase_index 为 1，total_phases 为 9 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — First Run` | 正向 | planner prompt 包含 change name | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | proposal pass 后返回 `"dev-design"`（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | proposal+dev-design pass 后返回 `"test-design"`（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | proposal+dev-design+test-design pass 后返回 `"implement"`（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | 04-05 对调场景：03 pass → implement → test-gen（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | 01-05 + 04 pass 后返回 `"unit-test"`（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | code-review 为 EVAL-ONLY（planner: null）（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | acceptance 为 EVAL-ONLY（planner: null）（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Normal Progression` | 正向 | 9 个 phase 全部 pass 后 done=true（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Skip Passed Phases` | 正向 | 01-03 pass 跳过 test-gen 返回 implement（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Skip Passed Phases` | 正向 | 全部 pass 后 done=true | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Retry Logic` | 正向 | fail 后同 phase 重试 next_phase 不变 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Retry Logic` | 异常 | 5 次连续 fail 返回 max_retries_exceeded（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Retry Logic` | 异常 | 超过最大重试次数的错误消息为中文 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Retry Logic` | 正向 | pass 后重试计数重置 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` | 正向 | backtrack 返回目标 phase（如 proposal）（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` | 正向 | backtrack 后 planner/evaluator agent_type 正确 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` | 正向 | 从 code-review backtrack 到 test-gen（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` | 正向 | 最新 entry 覆盖 backtrack 后正常推进（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` | 边界 | backtrack 不修改 entries（只读操作） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` | 边界 | array backtrack_to 返回最早目标（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack` | 异常 | 未知 backtrack 目标返回 invalid_backtrack_target | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Stale Entry Filtering` | 正向 | stale pass 被跳过，该 phase 重新执行（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Stale Entry Filtering` | 正向 | 全部 phase 有非 stale pass 后 done=true | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Stale Entry Filtering` | 边界 | 缺失 stale 字段视为 stale:false（向后兼容） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Stale Entry Filtering` | 正向 | stale 在中间 phase 时从该 phase 恢复 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Stale Entry Filtering` | 正向 | 05 pass 但 04 stale 时返回 04-test-gen | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Dependency-Graph Driven` | 正向 | 02 fail 时相邻 03 不可执行（AC-5） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Dependency-Graph Driven` | 正向 | 线性扫描优先未 pass 的 phase（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Dependency-Graph Driven` | 正向 | 04 pass 后返回 implement（AC-6） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Round Limit` | 异常 | round > 20 返回 round_limit_exceeded | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Round Limit` | 异常 | round limit 错误消息为中文 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Round Limit` | 边界 | 恰好 20 轮不触发限流 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Round Limit` | 异常 | backtrack 循环导致的 round limit（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Mid-Phase Interruption` | 正向 | evaluator 未写入时重新返回该 phase | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Mid-Phase Interruption` | 正向 | 未完成的 phase 返回执行 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Skipped Phases` | 正向 | skipped entry 视同 pass | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Skipped Phases` | 正向 | pass 与 skip 混合时正确推进 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 边界 | 默认 requirement（无 workflow.json） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | bug-fix 共 6 个 phase | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | refactor 共 9 个 phase | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | test-only 共 6 个 phase，首 phase 为 proposal | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | test-only proposal prompt 与 requirement 不同（AC-9） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | test-only proposal pass 后返回 code-analyze | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | test-only 01-02 pass 后返回 test-design | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | test-only 六阶段全部 pass 后 done=true | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | bug-fix 不含 test-design/test-gen/integration-test | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow_type` | 正向 | bug-fix code-review 后返回 acceptance（AC-10 回归） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only First Run` | 正向 | test-only 空 eval 时首 phase 为 proposal（AC-1） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only First Run` | 正向 | test-only total_phases 为 6 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | proposal pass 后返回 code-analyze | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | 01+02 pass 后返回 test-design | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | 01-03 pass 后返回 test-gen | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | 01-04 pass 后返回 unit-test | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Normal Progression` | 正向 | 06 pass 后返回 integration-test（并行 leaf） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Gate (02-code-analyze)` | 正向 | 01 pass 但 02 未 pass 时返回 code-analyze | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Gate (02-code-analyze)` | 正向 | 01-02 pass 后 gate 满足返回 test-design | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Completion` | 正向 | 全部六阶段 pass 后 done=true | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — test-only Completion` | 正向 | 不返回 implement 或 acceptance | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow.json default` | 边界 | workflow.json 缺失时使用 requirement 表 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — workflow.json default` | 边界 | workflow.json 无 workflow_type 时使用 requirement 表 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Backtrack (test-only)` | 正向 | test-only backtrack 到 proposal 返回目标 phase | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `runPhaseNext — Input Validation` | 边界 | 空 entries 数组不抛出异常 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 边界 | 空 eval.json（首次运行）正确设置 round=1 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 边界 | 20 轮内全部 pass 不触发限流 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 正向 | 01-03 pass + skip 04 返回 implement | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 边界 | change name 含特殊字符时 prompt 包含该名称 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 边界 | test-only change name 含特殊字符仍出现在 prompt | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 边界 | proposal 与 requirements 不混淆（旧名容错） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `Boundary Scenarios` | 正向 | acceptance pass 后 done=true | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next Output Schema` | 正向 | 下一 phase 就绪时字段完整性 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next Output Schema` | 正向 | 全部完成时 done=true, next_phase=null, planner=null, evaluator=null | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next Output Schema` | 异常 | 错误状态字段完整性 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next Output Schema` | 正向 | retry 时 prompt 与首次一致 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 边界 | 首 phase 无回溯目标（空数组） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 正向 | 次 phase 有一个回溯目标 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 正向 | 所有回溯目标含 id 与 description | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 边界 | done 时回溯目标为空 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 边界 | error 时回溯目标为空 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 正向 | evaluator prompt 含回溯提示（无前缀 phase ID） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 边界 | 首 phase 无回溯提示 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 正向 | retry 时回溯提示存在 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `phase_next — allowed_backtrack_phases` | 正向 | backtrack 检测后回溯提示正确 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — `<phase>` 占位符 | 新增 | `<phase>` 被替换为当前 phase 的 ID（如 `"proposal"`）（AC-3） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — `<phase>` 占位符 | 新增 | `<phase>` 替换为 dev-design / implement 等中间 phase 的 ID | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — `<phase>` 占位符 | 新增 | 模板中不含 `<phase>` 时原样返回 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — `<phase>` 占位符 | 边界 | 模板含多个 `<phase>` 时全部替换 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — `<phase>` 占位符 | 边界 | 模板为仅含 `<phase>` 的字符串时替换正确 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — `<phase>` 占位符 | 边界 | 模板为空白字符串时返回空白 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `interpolatePrompt` — `<phase>` 占位符 | 边界 | change name 含特殊字符时 `<phase>` 替换不受影响 | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `buildPhaseDef` — `<phase>` 动态注入 | 新增 | evaluator prompt 中 `<phase>` 被替换为 phase 表中对应 phase 的 ID | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `buildPhaseDef` — `<phase>` 动态注入 | 新增 | planner prompt 中 `<phase>` 被替换为 phase 表中对应 phase 的 ID | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 异常 | test-only 下 backtrack_to 指向 `"implement"`（旧 `"05-implement"`）时拒绝（AC-5 去前缀后回溯验证） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 异常 | 拒绝消息列出可用 phase（AC-5 去前缀后无前缀 ID 列表） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 异常 | test-only 下 backtrack_to `"dev-design"` 拒绝（AC-5） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 正向 | workflow.json 缺失时 requirement 默认接受 `"implement"`（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 正向 | test-only 下 backtrack_to `"dev-design"` 拒绝（AC-5） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — workflow-aware backtrack rejection` | 正向 | workflow.json 缺失时 requirement 默认接受 `"implement"`（AC-7 回归） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — adaptive fail after invalid backtrack` | 异常 | 无效 backtrack 抛错后 fail+null 成功写入（AC-7） | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — pass entry no propagation` | 正向 | pass 不触发 markPhaseStale | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — backtrack_to triggers markPhaseStale` | 正向 | 合法 backtrack 触发 markPhaseStale | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — input validation` | 边界 | null backtrack_to 正常写入 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — idempotency` | 正向 | 相同 pass 调用幂等 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — idempotency` | 正向 | 相同 fail+null 调用幂等 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — idempotency` | 正向 | 重复无效 backtrack 不写入 | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — auto-calculated verdict from checklist` | 正向 | 全部 pass 自动计算 verdict=pass | 修改 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `runPhaseLog — auto-calculated verdict from checklist` | 正向 | 任一 fail 自动计算 verdict=fail | 修改 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `fs` 模块 (`existsSync`, `readFileSync`) | `vi.mock('fs')` mock 文件系统，`mockImplementation` 根据路径返回模拟的 workflow.json（含 workflow_type）和 eval.json（含虚构 entries 数组） | 全部 `runPhaseNext` describe 块 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `fs` 模块 (`existsSync`, `readFileSync`) | `vi.mock('fs')` mock 文件系统，返回 workflow.json 的 workflow_type 配置 | 全部 `runPhaseLog` describe 块 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `eval-json` 模块 (`markPhaseStale`, `appendEntry`, `writeEvalJson`) | `vi.mock('../lib/eval-json')` 部分 mock，保留实际逻辑但 stub 写入函数；用 `toHaveBeenCalledWith` 验证调用参数 | 全部 `runPhaseLog` describe 块 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `change` 模块 (`getChangeDir`) | `vi.mock('../lib/change')` 返回固定路径 `/tmp/test-change` | 全部 `runPhaseLog` describe 块 |
| `plugins/dev-team/bin/src/lib/workflow.test.ts` | 无 | 纯函数测试，不涉及 IO/文件系统 | 全部 `getPhaseTable`/`getDependents`/`getPrerequisites` describe 块 |

---

## 集成测试

本变更为纯标识符重命名，不涉及新增集成测试场景。`phase_next` / `phase_log` MCP 工具名称不变，API 签名不变。以下 AC 的端到端正确性由单元测试保证：

- **AC-6**: 三个测试文件中的全部断言字符串更新为无前缀 phase ID，测试通过即验证全量断言覆盖
- **AC-7**: 全部正/反向回归用例（progression、retry、backtrack、round limit）在 phase ID 更新后仍通过，验证工作流不受影响

---

## 不可测试项

| 项目 | 原因 |
|------|------|
| AC-4 — 9 个 phase SKILL.md 的 gate check 断言使用新 phase ID | SKILL.md 是静态 Markdown agent 指令，无对应的可执行测试文件；通过全局 grep `0[0-9]-` 模式确保无残留旧 ID 来验证 |
| AC-5 — 9 个 evaluator / 2 个 executor agent .md 的 phase_log phase 参数使用新 ID | agent .md 是静态 Markdown prompt 模板，无对应的可执行测试文件；通过全局 grep `0[0-9]-` 模式确保无残留旧 ID 来验证，且参数的最终值由 `interpolatePrompt` 的 `<phase>` 替换保证正确 |
| AC-8 — plugin.json 版本号 patch bump | 版本号变更由 diff 审查确认，非可自动化测试项 |
| AC-9 — workflow SKILL.md 无硬编码序号 | workflow SKILL.md 是静态 Markdown 指令，无对应的可执行测试文件；通过全局 grep `Total phases: \d+` 模式确保无残留硬编码数字来验证 |
