## REMOVED Requirements

### Requirement: phaseLogInputSchema 中的 backtrack_to 和 backtrack_reason 字段

**Reason**: `backtrack_to` 和 `backtrack_reason` 从 `phaseLogInputSchema` 中移除。回溯不再是 Evaluator 的职责——所有 Evaluator（包括 test-execution-evaluator）一致地做纯诊断，不做路由决策。新增的 `backtrack` MCP 工具接管所有回溯状态变更。

**Migration**: 从 `phase-log.schema.ts` 的 `phaseLogInputSchema` 中移除 `backtrack_to` 和 `backtrack_reason` 字段。`phaseLogSchema` 保留这两个字段（向后兼容，用于解析已有 eval.json 条目）。`backtrack.schema.ts` 中的 `backtrackInputSchema` 将包含约束相同的字段。

### Requirement: phase_log 在 backtrack_to 设置时验证 backtrack_reason

**Reason**: 随着 `backtrack_to` 和 `backtrack_reason` 从 `phase_log` 中移除，此验证不再需要。`backtrack` MCP 工具执行等效验证。

**Migration**: 从 `phase-log.ts` 中移除 `validateBacktrackReason()` 函数。`backtrack.ts` 中的 `backtrack` 命令 SHALL 验证 `backtrack_reason` 在 `backtrack_to` 设置时为非空字符串。

### Requirement: buildEntry() 透传 backtrack_to 和 backtrack_reason

**Reason**: `BuildEntryParams` 不再包含 `backtrack_to` 和 `backtrack_reason`，因为条目不再在创建时携带回溯状态。`backtrack` 命令原地修改条目而非创建新条目。

**Migration**: 从 `eval-json.ts` 的 `BuildEntryParams` 类型中移除 `backtrack_to` 和 `backtrack_reason`。移除 `buildEntry()` 中对应的逻辑。

### Requirement: phase-log.ts 中的 handleBacktrackMarking

**Reason**: 回溯触发的 stale 标记现在由 `backtrack` MCP 工具处理，不再由 `phase_log` 处理。当 `backtrack` 被调用时，它标记目标 phase 的 pass 条目为 stale 并向下游传播。

**Migration**: 从 `phase-log.ts` 中移除 `handleBacktrackMarking()` 函数。将 stale 标记逻辑（调用 `markPhaseStale()` 和 `propagateStale()`）移入 `backtrack.ts`。

### Requirement: phase-next.ts 中的 computeAllowedBacktrackPhases 和 buildBacktrackHint

**Reason**: 这些函数构建 evaluator prompt 后缀，列出允许回溯的 phase。既然所有 Evaluator 不再设置 `backtrack_to`（那是 skill 的工作），这些函数在 `phase_next` 中不再需要。`allowed_backtrack_phases` 响应字段保留但返回空数组（向后兼容）。

**Migration**: 从 `phase-next.ts` 中移除 `computeAllowedBacktrackPhases()` 和 `buildBacktrackHint()`。从 `buildPhaseResponse()` 中移除对它们的调用。

### Requirement: test-execution-evaluator 回溯目标（特例逻辑）

**Reason**: test-execution-evaluator 不再拥有特殊的回溯决策逻辑。6 分支决策树移至 `phase-test-execution/SKILL.md`，与 code-review 和 acceptance evaluator 的决策迁移方式一致。所有 Evaluator 统一为纯诊断模式，不做路由决策。

**Migration**: 从 `test-execution-evaluator.md` 中移除 Step 4（6 分支决策树）。Evaluator 仅执行诊断，在 report 中描述失败根因。Skill 读取报告并按需调用 `backtrack()`。新增约束"禁止设置 backtrack_to"。

---

## ADDED Requirements

### Requirement: backtrack MCP 工具验证 phase 存在性和回溯目标

`backtrack` MCP 工具 SHALL 验证：
1. `change` 作为目录存在于 `openspec/changes/` 下
2. `phase` 存在于工作流的 phase 表中（通过 `getPhaseTable(workflowType)`）
3. `backtrack_to` 在 phase 表中位于 `phase` 之前（更小的索引）
4. `backtrack_reason` 在 `backtrack_to` 设置时为非空字符串
5. `phase` 的最新条目在 eval.json 中存在——若未找到则抛出错误

验证顺序 SHALL 为：change 存在 → phase 存在 → backtrack_to 在 phase 之前 → backtrack_reason 非空 → 最新条目存在。

#### Scenario: 有效参数的回溯成功
- **WHEN** eval.json 有一个 verdict 为 "fail" 的 `test-execution` 条目，且调用 `backtrack({change: "my-change", phase: "test-execution", backtrack_to: "test-gen", backtrack_reason: "语法错误: 测试文件存在 import 路径错误"})`
- **THEN** `test-execution` 条目的 `backtrack_to` 被设置为 `"test-gen"` 且 `backtrack_reason` 被设置为提供的字符串
- **AND** `test-gen` phase 的最新 pass 条目被标记为 `stale: true`
- **AND** `test-gen` 的所有下游依赖也被标记为 stale
- **AND** 函数返回 `{ modified: true, phase: "test-execution", target: "test-gen" }`

#### Scenario: 回溯到不存在的 phase 抛出错误
- **WHEN** 调用 `backtrack({change: "my-change", phase: "test-execution", backtrack_to: "nonexistent", backtrack_reason: "fix"})`
- **THEN** 抛出错误，指示目标 phase 在工作流中不存在

#### Scenario: 回溯到后续 phase 抛出错误
- **WHEN** 调用 `backtrack({change: "my-change", phase: "test-gen", backtrack_to: "acceptance", backtrack_reason: "fix"})`
- **THEN** 抛出错误，指示回溯目标必须在 phase 表中位于当前 phase 之前

#### Scenario: 无最新条目时回溯抛出错误
- **WHEN** eval.json 为空且调用 `backtrack({change: "my-change", phase: "test-execution", backtrack_to: "test-gen", backtrack_reason: "fix"})`
- **THEN** 抛出错误，指示未找到 `test-execution` 的条目

#### Scenario: 无原因回溯抛出错误
- **WHEN** 调用 `backtrack({change: "my-change", phase: "test-execution", backtrack_to: "implement"})` 时未提供 `backtrack_reason`
- **THEN** 抛出错误，指示 `backtrack_reason` 为必填

#### Scenario: 回溯原地修改条目（不创建新条目）
- **WHEN** eval.json 恰好有 5 个条目，且调用了 `backtrack()`
- **THEN** 调用后，eval.json 仍然恰好有 5 个条目（已有条目被修改）
- **AND** `computeRound` 返回 6（与回溯调用前相同）

### Requirement: backtrack 工具标记目标 phase 为 stale 并传播

当 `backtrack()` 在最新条目上设置 `backtrack_to` 时，它 SHALL 立即：
1. 找到目标 phase（`backtrack_to`）的最新 pass 条目
2. 将该条目标记为 `stale: true`
3. 调用 `propagateStale()` 将所有下游依赖条目标记为 stale

这确保当 `phase_next` 在回溯后运行时，它检测到目标 phase 需要重新执行。

#### Scenario: 单 phase 回溯正确传播
- **WHEN** eval.json 有 `proposal`、`dev-design`、`test-design`、`implement` 和 `test-gen` 的有效 pass 条目（按顺序），且调用 `backtrack({change, phase: "test-execution", backtrack_to: "test-gen", backtrack_reason: "fix"})`
- **THEN** 最新的 `test-gen` pass 条目被标记为 `stale: true`
- **AND** `test-execution` 条目（`test-gen` 的依赖）也被标记为 stale

#### Scenario: 回溯标记传递至所有下游
- **WHEN** eval.json 有所有 8 个 phase 的有效 pass 条目，且调用 `backtrack({change, phase: "test-execution", backtrack_to: "implement", backtrack_reason: "fix"})`
- **THEN** `implement` 的最新 pass 条目被标记为 stale
- **AND** `test-gen`、`test-execution`、`code-review`、`acceptance` 条目均被标记为 stale

### Requirement: backtrack 工具从 change 配置验证工作流类型

`backtrack` 函数 SHALL 从 change 目录的 `workflow.json` 中读取工作流类型（通过 `getWorkflowType()`）。工作流类型决定用于验证和 stale 传播的 phase 表和依赖图。

#### Scenario: bug-fix 工作流中的回溯使用正确的 phase 表
- **WHEN** 一个 change 有 `workflow.json` 含 `workflow_type: "bug-fix"`，且调用 `backtrack({change, phase: "test-execution", backtrack_to: "implement", backtrack_reason: "fix"})`
- **THEN** `implement` phase 使用 `bug-fix` phase 表验证（其中 implement 在 test-execution 之前）

### Requirement: phase_log 不再接受 backtrack_to 或 backtrack_reason

`phase-log.schema.ts` 中的 `phaseLogInputSchema` SHALL 不再包含 `backtrack_to` 或 `backtrack_reason` 字段。

`runPhaseLog()` 函数 SHALL 不再：
- 接受或验证 `backtrack_to` 或 `backtrack_reason` 参数
- 调用 `handleBacktrackMarking()` 或 `validateBacktrackReason()`
- 为回溯目标执行 stale 标记

`eval-json.ts` 中的 `BuildEntryParams` 类型 SHALL 移除 `backtrack_to` 和 `backtrack_reason`。

`phaseLogSchema` SHALL 保留 `backtrack_to` 和 `backtrack_reason` 字段作为 `optional`，用于解析已有 eval.json 条目（向后兼容）。

#### Scenario: 无回溯字段的 phase_log 调用成功
- **WHEN** 调用 `runPhaseLog({change: "my-change", phase: "test-execution", report: "...", checklist: [...]})` 时未提供 `backtrack_to` 或 `backtrack_reason`
- **THEN** 写入新条目并返回 `{written: true, phase: "test-execution", attempt: N}`

#### Scenario: 带 backtrack_to 的 phase_log 被 schema 拒绝
- **WHEN** 调用 `phaseLogInputSchema.parse({change: "my-change", phase: "test-execution", report: "...", checklist: [], backtrack_to: "test-gen", backtrack_reason: "fix"})`
- **THEN** 抛出 Zod 错误，因为 `backtrack_to` 不是已知字段

#### Scenario: 含 backtrack_to 的旧 eval 条目解析无错误
- **WHEN** 此变更前的 eval.json 文件（条目包含 `backtrack_to` 和 `backtrack_reason` 字段）被 `readEvalJson()` 读取
- **THEN** 条目解析无错误（`phaseLogSchema` 保留这些字段作为 optional）

### Requirement: Skill 统一回溯决策流程

所有 phase-* SKILL.md 和 workflow-* SKILL.md SHALL 在 Evaluator 返回后遵循此统一模式。此模式适用于包括 `test-execution` 在内的所有 phase，无任何特例：

```
Evaluator 返回
调用 phase_next 获取 last_result

if last_result.verdict == "pass":
  继续到下一 phase

if last_result.verdict == "fail":
  1. 分析 report 确定根因
  2. 决策:
     - retry: 继续循环（phase_next 自然重试）
     - backtrack: 调用 backtrack(change, phase, target, reason)
     - 询问用户: 如果非常确信失败可以接受
       使用 AskUserQuestion "Phase X failed: <reason>. Continue anyway?"
       如果用户同意 → 用户手动修改 eval.json 将 verdict 改为 "pass"
       Skill 禁止在失败时自行写入 verdict="pass"
  3. 继续循环
```

#### Scenario: Skill 在失败后通过继续循环来重试
- **WHEN** `last_result.verdict` 为 `"fail"` 且 skill 判定失败可以通过重新运行该 phase 来修复
- **THEN** skill 继续循环，不调用 `backtrack()`
- **AND** `phase_next` 返回相同的 phase（不存在 pass 条目）

#### Scenario: Skill 在失败后通过调用 backtrack 工具来回溯
- **WHEN** `last_result.verdict` 为 `"fail"` 且 skill 判定根因需要重新执行更早的 phase
- **THEN** skill 调用 `backtrack(change, phase, target, reason)`
- **AND** 继续循环，之后 `phase_next` 返回目标 phase

#### Scenario: Skill 在确信失败可忽略时询问用户
- **WHEN** `last_result.verdict` 为 `"fail"` 但 skill 认为失败不阻碍进展
- **THEN** skill 使用 `AskUserQuestion` 呈现情况
- **AND** 如果用户同意，用户手动编辑 eval.json 将该条目的 `verdict` 设置为 `"pass"`
- **AND** skill 禁止在失败时程序化写入 `verdict: "pass"`

### Requirement: test-execution skill 接管 6 分支决策树

`phase-test-execution/SKILL.md` SHALL 包含之前由 `test-execution-evaluator` 持有的 6 分支决策树。与其他 skill 使用完全相同的决策模式——Evaluator 返回后，skill 分析 `last_result.report` 并决定回溯目标，而非由 Evaluator 直接设置 `backtrack_to`。

1. **语法错误（测试文件中的 SyntaxError/TypeError/ReferenceError）** -> `backtrack_to: "test-gen"`
2. **逻辑错误（实现文件中的 AssertionError）** -> `backtrack_to: "implement"`
3. **设计冲突（预期/实际与 test-design.md 不一致）** -> `backtrack_to: "test-design"`
4. **接口签名不匹配** -> `backtrack_to: "dev-design"`
5. **覆盖率不达标（覆盖率低于阈值）** -> `backtrack_to: "test-design"`
6. **无法判断（混合/模糊错误）** -> `AskUserQuestion`

#### Scenario: Skill 检测到语法错误触发了到 test-gen 的回溯
- **WHEN** `last_result.report` 包含"语法错误"且失败详情引用了测试文件的行号
- **THEN** skill 调用 `backtrack(change, "test-execution", "test-gen", "语法错误: ...")`
- **AND** 不询问用户

#### Scenario: 源文件中的断言错误触发了到 implement 的回溯
- **WHEN** `last_result.report` 包含"AssertionError"且失败文件是源文件
- **THEN** skill 调用 `backtrack(change, "test-execution", "implement", "逻辑错误: ...")`

#### Scenario: 模糊诊断导致 skill 询问用户
- **WHEN** `last_result.report` 指示多种错误类型且无明确根因
- **THEN** skill 使用 `AskUserQuestion` 提供推荐选项
- **AND** 等待用户输入后再调用 backtrack 或继续

#### Scenario: test-execution skill 与其他 phase skill 使用相同的决策模式
- **WHEN** 检查 `phase-test-execution/SKILL.md` 和 `phase-code-review/SKILL.md` 的回溯决策章节
- **THEN** 两者的结构相同：调用 phase_next、检查 last_result、分析 report、三叉决策
- **AND** test-execution 的额外分支（6 种错误类型）仅在 report 分析环节体现，不改变整体模式

### Requirement: code-review evaluator 不再设置 backtrack_to

`code-review-evaluator.md` SHALL 移除 Step 8（"如果发现设计矛盾则回溯"）。改为：当发现设计矛盾时，evaluator SHALL 在 report 中描述它们，附带具体的 `file:line` 证据。Evaluator SHALL 约束为："禁止设置 backtrack_to"。

`phase-code-review/SKILL.md` SHALL 读取 evaluator 的 report。如果 report 描述设计矛盾，skill SHALL 评估是调用 `backtrack(change, "code-review", "dev-design", reason)` 还是以 fail verdict 继续并询问用户。

#### Scenario: Code-review evaluator 描述矛盾但不设置 backtrack_to
- **WHEN** code-review-evaluator 发现设计矛盾
- **THEN** report 包含 `file:line` 引用和矛盾描述
- **AND** 条目中不设置 `backtrack_to`
- **AND** verdict 为 "fail"

#### Scenario: Code-review skill 在读取 report 后决定回溯
- **WHEN** skill 读取 `last_result.report` 并看到设计矛盾证据
- **THEN** skill 调用 `backtrack(change, "code-review", "dev-design", "设计矛盾: ...")`

### Requirement: acceptance evaluator 不再设置 backtrack_to

`acceptance-evaluator.md` SHALL 移除 Step 7（"如果发现需求缺口则回溯"）。改为：当发现需求缺口时，evaluator SHALL 在 report 中列出未满足的 AC-ID。Evaluator SHALL 约束为："禁止设置 backtrack_to"。

`phase-acceptance/SKILL.md` SHALL 读取 evaluator 的 report。如果 report 列出未满足的 AC-ID，skill SHALL 评估是调用 `backtrack(change, "acceptance", "proposal", reason)` 还是以 fail verdict 继续并询问用户。

#### Scenario: Acceptance evaluator 列出未满足的 AC-ID 但不设置 backtrack_to
- **WHEN** acceptance-evaluator 发现需求缺口
- **THEN** report 列出没有实现证据的 AC-ID
- **AND** 条目中不设置 `backtrack_to`
- **AND** verdict 为 "fail"

#### Scenario: Acceptance skill 在读取 report 后决定回溯
- **WHEN** skill 读取 `last_result.report` 并看到未满足的 AC-ID
- **THEN** skill 调用 `backtrack(change, "acceptance", "proposal", "需求缺口: AC-1, AC-3 无实现证据")`

### Requirement: Evaluator Agent 统一"禁止设置 backtrack_to"约束

所有涉及回溯决策的 Evaluator Agent（test-execution-evaluator、code-review-evaluator、acceptance-evaluator）SHALL 包含以下约束：

"禁止设置 backtrack_to。回溯路由决策由 main agent（skill）负责。诊断结果通过 report 传递给 skill，skill 根据 report 内容决定是否调用 backtrack 工具以及回溯目标。"

test-execution-evaluator SHALL 不因拥有 6 分支决策树而获得特殊权限——其约束与其他 Evaluator 完全一致。

#### Scenario: 三个 Evaluator 拥有相同的 backtrack_to 约束
- **WHEN** 读取 test-execution-evaluator.md、code-review-evaluator.md 和 acceptance-evaluator.md
- **THEN** 每个文件都包含"禁止设置 backtrack_to"约束
- **AND** 约束措辞一致

---

## MODIFIED Requirements

### Requirement: Stale 传播由 backtrack 工具触发（原为 phase_log）

之前由 `phase-log.ts` 中 `handleBacktrackMarking()` 持有的 stale 标记逻辑现在由 `backtrack.ts` 中的 `backtrack` 命令持有。行为功能上相同：
1. 读取 eval.json
2. 对于 `backtrack_to` 中的目标 phase：
   - 在 phase 表中找到目标 phase 索引
   - 验证它在当前 phase 索引之前
   - 调用 `markPhaseStale(entries, target, workflowType)`
3. 写入更新后的 eval.json

唯一区别：此逻辑由 `backtrack()` 触发，而非 `phase_log`。

#### Scenario: backtrack 工具的 stale 标记与旧行为一致
- **WHEN** 调用 `backtrack({change, phase: "test-execution", backtrack_to: "test-gen", backtrack_reason: "fix"})`
- **THEN** stale 标记行为与旧的 `handleBacktrackMarking()` 调用相同

---

## 模块契约

### backtrack.schema.ts (NEW)

| Export | Purpose |
|--------|---------|
| `backtrackInputSchema` | 输入：`{ change: string, phase: string, backtrack_to: string, backtrack_reason: string }` |
| `backtrackOutputSchema` | 输出：`{ modified: boolean, phase: string, target: string }` |

### phase-log.schema.ts

| Export | Change | Purpose |
|--------|--------|---------|
| `phaseLogInputSchema` | REMOVED `backtrack_to`, `backtrack_reason` | Evaluator 不再设置回溯，输入 schema 不再接受这些字段 |
| `phaseLogSchema` | UNCHANGED (字段保留) | 保留 `backtrack_to`/`backtrack_reason` 作为 optional，用于解析已有 eval.json 条目 |

### eval-json.ts (lib/)

| Export | Change | Purpose |
|--------|--------|---------|
| `BuildEntryParams` | REMOVED `backtrack_to`, `backtrack_reason` | Phase log 条目不再在创建时携带回溯状态 |
| `buildEntry()` | REMOVED `backtrack_to`, `backtrack_reason` 透传 | 条目在创建时不再携带回溯字段 |
| `EvalEntry` 类型 | UNCHANGED | `phaseLogSchema` 保留字段，故类型不变——向后兼容 |
| `markPhaseStale()` | UNCHANGED | 仍由 backtrack 工具使用 |
| `propagateStale()` | UNCHANGED | 仍由 backtrack 工具使用 |
| `readEvalJson()` | UNCHANGED | `phaseLogSchema` 保留字段，旧条目可解析 |

### phase-log.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `runPhaseLog()` | MODIFIED | 移除 `backtrack_to`/`backtrack_reason` 参数，移除 stale 标记逻辑，简化写入路径 |
| `handleBacktrackMarking()` | REMOVED | 移至 `backtrack.ts` |
| `validateBacktrackReason()` | REMOVED | 不再需要 |

### phase-next.ts (commands/)

| Export | Change | Purpose |
|--------|--------|---------|
| `runPhaseNext()` | MODIFIED | 在响应中增加 `last_result` 字段 |
| `resolvePhaseNext()` | MODIFIED | 将最新条目作为 `last_result` 传递 |
| `buildPhaseResponse()` | MODIFIED | 移除回溯提示构建逻辑；增加 `last_result` 参数 |
| `buildPhaseDef()` | MODIFIED | 移除 `backtrackReason` 参数和回溯原因后缀逻辑 |
| `computeAllowedBacktrackPhases()` | REMOVED | 不再需要（Evaluator 不再做路由决策） |
| `buildBacktrackHint()` | REMOVED | 不再需要 |

### phase-next.schema.ts

| Export | Change | Purpose |
|--------|--------|---------|
| `phaseNextOutputSchema` | ADDED `last_result` | 新字段 `z.object({ phase, verdict, report, timestamp }).nullable()` |

### mcp.ts

| Export | Change | Purpose |
|--------|--------|---------|
| phase_log 注册 | MODIFIED | 输入 schema 更新（移除回溯字段） |
| backtrack 注册 | ADDED | 新 MCP 工具注册，使用 `backtrackInputSchema` 和 `backtrackOutputSchema` |

### test-execution-evaluator.md

| Section | Change | Purpose |
|---------|--------|---------|
| Step 4 | REMOVED | 6 分支决策树迁移至 skill |
| 所有 `backtrack_to` 引用 | REMOVED | Evaluator 仅诊断 |
| Description | MODIFIED | 改为"sets verdict and diagnoses root cause" |
| Constraint | ADDED | "禁止设置 backtrack_to"，与所有 Evaluator 一致 |

### code-review-evaluator.md

| Section | Change | Purpose |
|---------|--------|---------|
| Step 8 | REMOVED | 回溯逻辑移除；替换为 report 增强 |
| Constraint | ADDED | "禁止设置 backtrack_to" |

### acceptance-evaluator.md

| Section | Change | Purpose |
|---------|--------|---------|
| Step 7 | REMOVED | 回溯逻辑移除；替换为列出未满足的 AC-ID |
| Constraint | ADDED | "禁止设置 backtrack_to" |

### phase-test-execution/SKILL.md

| Section | Change | Purpose |
|---------|--------|---------|
| Step 3c | MODIFIED | 从 `last_result` 检查 verdict；如果 fail，应用 6 分支决策树并调用 `backtrack()`。使用与所有其他 skill 相同的统一决策模式。 |

### phase-code-review/SKILL.md

| Section | Change | Purpose |
|---------|--------|---------|
| Step 4 | MODIFIED | 读取 `last_result.report`；如果描述设计矛盾，调用 `backtrack(change, "code-review", "dev-design", reason)` |

### phase-acceptance/SKILL.md

| Section | Change | Purpose |
|---------|--------|---------|
| Step 4 | MODIFIED | 读取 `last_result.report`；如果列出未满足的 AC-ID，调用 `backtrack(change, "acceptance", "proposal", reason)` |

### workflow-requirement/SKILL.md and workflow-test-only/SKILL.md

| Section | Change | Purpose |
|---------|--------|---------|
| 工作流循环 | MODIFIED | Evaluator 调用后，读取 `last_result`；如果 fail，应用统一三叉决策流程 |
