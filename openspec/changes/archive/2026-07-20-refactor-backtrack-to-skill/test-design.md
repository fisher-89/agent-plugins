# 测试设计: refactor-backtrack-to-skill

> **日期**: 2026-07-17

---

## 验收范围

<!-- 逐条映射 proposal.md 的验收标准 -->

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `phaseLogInputSchema` 不再包含 `backtrack_to` 和 `backtrack_reason` 字段；调用 `runPhaseLog({change, phase, report, checklist})` 成功且返回 `{written: true}` | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — 无回溯参数的正常路径 |
| AC-2 | 调用 `backtrack({change, phase:"test-execution", backtrack_to:"test-gen", backtrack_reason:"语法错误"})` 后，eval.json 中 `test-execution` 的最新 entry 的 `backtrack_to` 被设置为 `"test-gen"` 且 `test-gen` 的 pass entry 被标记为 stale | 单元测试 | `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 正常回溯操作 |
| AC-3 | `backtrack({change, phase:"test-gen", backtrack_to:"acceptance"})` 抛出错误（acceptance 在 test-gen 之后） | 单元测试 | `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 目标合法性验证 |
| AC-4 | `runPhaseNext({change})` 的响应包含 `last_result` 字段，其值为最新 entry 的快照（或 null） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — last_result 字段 |
| AC-5 | test-execution-evaluator / code-review-evaluator / acceptance-evaluator 不再设置 backtrack_to | 不可测试 | — | — |
| AC-6 | phase-test-execution/SKILL.md 包含统一回溯决策流程；workflow-requirement/SKILL.md 包含基于 last_result.verdict 的三叉决策 | 不可测试 | — | — |
| AC-7 | `plugins/dev-team/.claude-plugin/plugin.json` 版本号已递增 | 不可测试 | — | — |
| AC-8 | 含 `backtrack_to`/`backtrack_reason` 字段的旧 eval.json 条目可被 `readEvalJson()` 无错误解析（Zod 保留未知字段） | 单元测试 | `plugins/dev-team/bin/src/lib/eval-json.test.ts` / `plugins/dev-team/bin/src/commands/phase-next.test.ts` | 旧 eval.json 兼容性 |

---

## 单元测试

### 用例

#### backtrack.test.ts — 新增文件

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 正常回溯操作 | 正向 | `runBacktrack` 修改指定 phase 最新 entry 的 `backtrack_to` 为 `"test-gen"`，`backtrack_reason` 为指定原因 | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 正常回溯操作 | 正向 | 修改后 entry 的 `backtrack_to` 和 `backtrack_reason` 字段值正确 | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 正常回溯操作 | 正向 | 返回 `{ modified: true, phase: "test-execution", target: "test-gen" }` | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 正常回溯操作 | 正向 | 标记目标 phase `test-gen` 的最新 pass entry 为 `stale: true` | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 正常回溯操作 | 正向 | stale 标记向下游传播到 `test-execution` 等依赖 phase | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 正常回溯操作 | 正向 | 多 workflow 类型（requirement、test-only）均正常工作 | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 目标合法性验证 | 异常 | `backtrack_to` 等于当前 phase 时抛出错误 | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 目标合法性验证 | 异常 | `backtrack_to` 在当前 phase 之后时抛出错误（如 AC-3） | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 目标合法性验证 | 异常 | `backtrack_to` 不在 phase 表中时抛出错误 | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 目标合法性验证 | 异常 | change 不存在时抛出错误 | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 目标合法性验证 | 异常 | phase 不在 phase 表中时抛出错误 | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 目标合法性验证 | 异常 | 目标 phase 无任何 entry 时抛出错误 | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 目标合法性验证 | 异常 | 目标 phase 有 entry 但无 pass entry 时抛出错误（无 pass 可标记 stale） | 新增 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | runBacktrack — 幂等性 | 正向 | 连续调用两次 backtrack 结果一致（第二次覆盖第一次的设置） | 新增 |

#### phase-log.test.ts — 修改文件

**废弃的测试（功能移除）：**

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — workflow-aware backtrack rejection | 废弃 | 所有 5 个测试用例：workflow-aware backtrack rejection 系列（由于 `backtrack_to` 参数从 input schema 移除） | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — adaptive fail after invalid backtrack | 废弃 | 单个测试：无效回溯后的 adaptive fail（backtrack_to 不再从 phase_log 传入） | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — pass entry no propagation | 废弃 | 单个测试：pass entry 不触发 markPhaseStale（backtrack_to 不再从 phase_log 传入） | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — backtrack_to triggers markPhaseStale | 废弃 | 单个测试：backtrack_to 触发 markPhaseStale（该逻辑已移至独立 backtrack 工具） | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — backtrack_reason validation (AC-2) | 废弃 | 全部 7 个测试用例：backtrack_reason 校验系列（校验逻辑移至 backtrack 工具） | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — idempotency | 废弃 | 3 个测试用例中的 `backtrack_to: null` 参数（参数已从 input schema 移除） | 废弃 |

**新增/修改的测试：**

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — 无回溯参数的正常路径 | 正向 | 调用 `runPhaseLog({change, phase, report, checklist})` 成功返回 `{written: true}` | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — 无回溯参数的正常路径 | 正向 | fail entry 正常追加到 eval.json | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — 无回溯参数的正常路径 | 正向 | pass entry 正常追加到 eval.json | 新增 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | runPhaseLog — 无回溯参数的正常路径 | 异常 | 传入 `backtrack_to` 时 schema 层静默忽略（Zod stripUnknown 行为） | 新增 |

#### eval-json.test.ts — 修改文件

**废弃的测试（`BuildEntryParams` 移除回溯字段）：**

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 废弃 | `backtrack_to` 被正确写入（BuildEntryParams 移除该字段） | 废弃 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 废弃 | `backtrack_to` 为 null 时 entry 为 null（同上） | 废弃 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 废弃 | `backtrack_to` 作为数组传入（同上） | 废弃 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 废弃 | `backtrack_reason` 被正确写入（AC-3）（BuildEntryParams 移除该字段） | 废弃 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 废弃 | `backtrack_reason` 为 undefined 时 entry 中该字段为 null | 废弃 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 废弃 | `backtrack_reason` 为 null 时 entry 中该字段为 null | 废弃 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 废弃 | `backtrack_reason` 500 字符边界 | 废弃 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | buildEntry | 废弃 | `backtrack_reason` 含有特殊字符 | 废弃 |

**保持不变的测试（`markPhaseStale`、`propagateStale`）：**

`markPhaseStale` 和 `propagateStale` 函数保持公共导出，供 backtrack 命令使用。已有 10 个测试用例全部保留，无需修改。

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 正向 | 标记最新 pass entry 为 stale 并向下游传播（AC-8） | 保持不变 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 边界 | 多个 pass entry 时仅标记最新的 | 保持不变 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 异常 | 无 pass entry 时 no-op | 保持不变 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 正向 | 完整传递闭包传播 | 保持不变 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 正向 | implement → test-gen/test-execution/code-review/acceptance 传播 | 保持不变 |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | markPhaseStale | 边界 | implement 不 stale test-design（AC-9） | 保持不变 |

#### phase-next.test.ts — 修改文件

**新增测试：**

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — last_result 字段 | 正向 | 有 entries 时 `last_result` 字段存在且包含 phase/verdict/report/timestamp | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — last_result 字段 | 正向 | `last_result.phase` 为最新 entry 的 phase（按 timestamp 降序） | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — last_result 字段 | 正向 | `last_result.verdict` 为最新 entry 的 verdict | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — last_result 字段 | 正向 | `last_result.report` 为最新 entry 的 report | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — last_result 字段 | 边界 | 无 entries 时 `last_result` 为 null | 新增 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | runPhaseNext — last_result 字段 | 边界 | fail entry 后 `last_result.verdict` 为 "fail" | 新增 |

**废弃的测试（`buildBacktrackHint` 移除导致 evaluator prompt 不再包含回溯提示）：**

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 废弃 | evaluator prompt 包含"可回退阶段 (backtrack_to)"提示（builtBacktrackHint 已移除） | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 废弃 | 无前置 phase 时 prompt 不包含"可回退阶段" | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 废弃 | retry 时 evaluator prompt 包含 backtrack hint | 废弃 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 废弃 | backtrack 检测后 prompt 不包含"可回退阶段" | 废弃 |

**保留的测试（`allowed_backtrack_phases` 响应字段保留）：**

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 正向 | 首个 phase 返回空数组 | 保持不变 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 正向 | 后续 phase 返回前置 phase 列表 | 保持不变 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 正向 | 列表包含 id 和 description | 保持不变 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 边界 | workflow 完成时返回空数组 | 保持不变 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 边界 | 错误时返回空数组 | 保持不变 |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | phase_next — allowed_backtrack_phases | 边界 | test-execution 不包含 unit-test/integration-test | 保持不变 |

#### mcp.test.ts — 修改文件

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — 新工具注册 | 正向 | `backtrack` 工具已注册 | 新增 |
| `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — 其他工具保留 | 正向 | 总工具数从 10 增加到 11 | 修改 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | `fs` 模块 | `vi.mock('fs')` 模拟 `existsSync`、`readFileSync`、`writeFileSync`、`mkdirSync` | runBacktrack — 所有测试场景 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | `../lib/eval-json` | `vi.mock` 部分 mock：`markPhaseStale`/`propagateStale` 保留真实实现，`readEvalJson`/`writeEvalJson` vi.fn() | runBacktrack — 隔离文件系统 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | `../lib/change` | `vi.mock` 模拟 `getChangeDir` | 所有需要 change 路径的场景 |
| `plugins/dev-team/bin/src/commands/backtrack.test.ts` | `../lib/change-config` | `vi.mock` 模拟 `getWorkflowType` | 多 workflow 类型测试 |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `fs` 模块 | `vi.mock('fs')` 模拟文件系统操作 | 所有测试场景（已有，需更新 mock 匹配新接口） |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts` | `../lib/eval-json` | `vi.mock` 部分 mock | 隔离文件系统（已有 mock，需移除 `markPhaseStale` mock 依赖） |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | `fs` 模块 | `vi.mock('fs')` 模拟 eval.json 读取 | 所有测试场景（已有，保持不变） |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts` | 无 | 纯函数测试，不依赖文件系统 | 所有 buildEntry/markPhaseStale 测试 |

---

## 集成测试

本变更不涉及跨进程/跨模块边界的集成测试场景。所有功能可在单元测试中通过进程内 mock 方式验证：

- `backtrack` 工具通过 mock fs 层验证逻辑
- `phase_log` 工具通过 mock fs 层验证 schema 变更后的行为
- `phase_next` 工具通过 mock fs 层验证 `last_result` 字段
- `mcp.ts` 已有通过 `InMemoryTransport` 进行的 MCP 协议级集成测试，新增 `backtrack` 工具注册验证

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-2 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — backtrack 工具 | `backtrack` 工具已注册到 MCP server | 新增 |

### Mock策略

沿用已有 `InMemoryTransport` 方案（mcp.test.ts 中使用 `@modelcontextprotocol/sdk/inMemory` + `Client`）。无需额外 mock。

---

## 不可测试项

| 条目 | 原因 |
|------|------|
| AC-5: 三个 Evaluator Agent 不再设置 backtrack_to（`test-execution-evaluator.md`、`code-review-evaluator.md`、`acceptance-evaluator.md`） | Agent 配置文件为 Markdown 指令文档，非可执行代码。约束"禁止设置 backtrack_to"为文本指令，无法通过自动化测试验证，需 Code Review 阶段人工检查 |
| AC-6: Skill 决策流程统一（10 个 SKILL.md 文件） | SKILL.md 为 Markdown 技能描述文件，非可执行代码。统一回溯决策模式为文本约定，无法通过自动化测试验证，需 Code Review 阶段逐文件对照检查一致性 |
| AC-7: 插件版本升级（`plugins/dev-team/.claude-plugin/plugin.json`） | 配置文件变更，版本号 `2.8.13` → `2.9.0`。可目视验证或 CI pipeline 中通过 grep 确认 |
| Schema 文件（`backtrack.schema.ts`、`index.ts`、`phase-log.schema.ts`、`phase-next.schema.ts`） | 根据 `test_resolve_paths` 结果，schema 文件不在测试配置范围内。Schema 通过消费者命令（backtrack、phase-log、phase-next）的测试间接覆盖 |
