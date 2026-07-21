# 实施任务: refactor-backtrack-to-skill

> 按依赖顺序排列，前一阶段的任务为后一阶段的前置条件。

---

## 第一阶段：Schema 和类型定义

此阶段建立所有数据契约变更，是后续逻辑实现的基础。

- [x] 1.1 创建 `plugins/dev-team/bin/src/schemas/backtrack.schema.ts`
  - 定义 `backtrackInputSchema`：`z.object({ change: z.string(), phase: z.string(), backtrack_to: z.string(), backtrack_reason: z.string().max(500) })`
  - 定义 `backtrackOutputSchema`：`z.object({ modified: z.boolean(), phase: z.string(), target: z.string() })`

- [x] 1.2 修改 `plugins/dev-team/bin/src/schemas/phase-log.schema.ts`
  - 从 `phaseLogInputSchema` 中移除 `backtrack_to` 和 `backtrack_reason` 字段（Evaluator 不再通过 phase_log 设置这些字段）
  - 保留 `phaseLogSchema` 中的 `backtrack_to` 和 `backtrack_reason` 字段（向后兼容，用于解析现有 eval.json 条目）
  - `phaseLogInputSchema` 的 extend 中不再需要 `backtrack_to` 和 `backtrack_reason`

- [x] 1.3 修改 `plugins/dev-team/bin/src/schemas/phase-next.schema.ts`
  - 在 `phaseNextOutputSchema` 中增加 `last_result` 字段：
    ```typescript
    last_result: z.object({
      phase: phaseIdSchema,
      verdict: z.enum(['pass', 'fail']),
      report: z.string(),
      timestamp: z.iso.datetime(),
    }).nullable().describe('Latest eval entry snapshot (null if no entries exist)')
    ```

- [x] 1.4 修改 `plugins/dev-team/bin/src/schemas/index.ts`
  - 新增导出 `backtrackInputSchema` 和 `backtrackOutputSchema` from `./backtrack.schema`

- [x] 1.5 修改 `plugins/dev-team/bin/src/lib/eval-json.ts`
  - 从 `BuildEntryParams` 类型中移除 `backtrack_to` 和 `backtrack_reason` 字段
  - 从 `buildEntry()` 函数中移除 `backtrack_to` 和 `backtrack_reason` 的处理逻辑（删除第 86-87 行）
  - 确保 `EvalEntry` 类型（派生自 `phaseLogSchema`）仍包含 `backtrack_to` 和 `backtrack_reason`（用于兼容读取旧条目）
  - `markPhaseStale()` 和 `propagateStale()` 保持 public 导出（backtrack 命令需要调用）

## 第二阶段：核心命令实现

此阶段实现 backtrack 工具和修改现有的 phase-log / phase-next 命令。

- [x] 2.1 实现 `plugins/dev-team/bin/src/commands/backtrack.ts`
  - 导入：`readEvalJson`, `markPhaseStale`, `writeEvalJson`, `getChangeDir`, `getWorkflowType`, `getPhaseTable`, `type EvalEntry`
  - 导入 schema 类型
  - 实现 `runBacktrack(options: BacktrackOptions): BacktrackResult`：
    ```
    a. getChangeDir(change) — 验证 change 存在
    b. getWorkflowType(change) — 获取 workflow type
    c. getPhaseTable(workflowType) — 获取 phase 表
    d. 验证 phase 在 phase 表中存在，否则抛错
    e. 验证 backtrack_to 的 phase 表索引 < phase 的索引，否则抛错（不能回溯到当前或未来 phase）
    f. readEvalJson(changeDir) — 读取 eval.json
    g. 按 timestamp 降序查找 phase 的最新 entry——未找到则抛错
    h. 原地修改该 entry：设置 `backtrack_to` 和 `backtrack_reason`
    i. markPhaseStale(entries, backtrack_to, workflowType) — 标记目标 phase stale 并向下游传播
    j. writeEvalJson(changeDir, entries) — 写入
    k. 返回 { modified: true, phase, target: backtrack_to }
    ```

- [x] 2.2 修改 `plugins/dev-team/bin/src/commands/phase-log.ts`
  - 移除 `handleBacktrackMarking()` 函数（整段删除，第 35-63 行）
  - 移除 `validateBacktrackReason()` 函数（整段删除，第 69-74 行）
  - 从 `runPhaseLog()` 中移除：
    - 对 `validateBacktrackReason()` 的调用（第 91 行）
    - 对 `handleBacktrackMarking()` 的调用（第 103 行）
    - 条件写入逻辑（第 118-131 行中 modifiedByBacktrack 分支）
  - 修改 `buildEntry()` 调用：移除 `backtrack_to` 和 `backtrack_reason` 参数
  - 简化写入：始终使用 `appendEntry()`（删除 modifiedByBacktrack 条件分支）
  - 删除不再需要的 import：`getWorkflowType`, `markPhaseStale`, `writeEvalJson`, `getPhaseTable`（验证是否被其他函数使用）

- [x] 2.3 修改 `plugins/dev-team/bin/src/commands/phase-next.ts`
  - 移除 `buildBacktrackHint()` 函数（整段删除，第 56-60 行）
  - 从 `buildPhaseDef()`（第 68-96 行）中移除：
    - 对 `computeAllowedBacktrackPhases()` 的调用（第 74 行）
    - 对 `buildBacktrackHint()` 的调用（第 75 行）
    - evaluator prompt 的 `+ backtrackHint` 拼接（第 92 行）
  - **保留** `reasonSuffix` 逻辑（回溯原因仍需要传播到 planner/evaluator prompt，用于兼容过渡期旧条目）
  - **移除** `computeAllowedBacktrackPhases()` 函数（`allowed_backtrack_phases` 响应字段保留但返回空数组 `[]`，向后兼容）
  - 在所有 response builder 函数中增加 `last_result` 字段：
    - `buildPhaseResponse()` — 增加 `lastResult?: { phase, verdict, report, timestamp } | null` 参数
    - `buildDoneResponse()` — 增加 `lastResult?` 参数
    - `buildErrorResponse()` — 增加 `lastResult?` 参数
  - 在 `resolvePhaseNext()`（第 356-398 行）中：
    - 从 entries 中提取最新 entry 的快照作为 `last_result`（按 timestamp 降序取第一个）
    - 将 `last_result` 传递给所有 response builder 调用
    - 响应 schema 中 `phaseNextOutputSchema` 已包含 `last_result` 字段（1.3 中已定义）
  - 更新 `PhaseNextResult` 类型以匹配新的 output schema

## 第三阶段：MCP 注册

此阶段将 backtrack 工具注册到 MCP 服务器。

- [x] 3.1 修改 `plugins/dev-team/bin/src/mcp.ts`
  - 导入：`runBacktrack` from `./commands/backtrack`
  - 导入：`backtrackInputSchema`, `backtrackOutputSchema` from `./schemas`
  - 实现 `registerBacktrackTool(server: McpServer): void`：
    - 注册 MCP 工具，名称为 `"backtrack"`
    - description: "Set backtrack target and reason for a phase entry in eval.json. This is the only way to modify backtrack state."
    - inputSchema: `backtrackInputSchema`, outputSchema: `backtrackOutputSchema`
    - handler 调用 `runBacktrack(args)` 并返回 `jsonContent(backtrackOutputSchema, result)`
  - 在 `connectToServer()` 中调用 `registerBacktrackTool(server)`
  - 更新 `registerPhaseLogTool()` 和 `registerPhaseNextTool()`——schema 已在 schema 层更新，无需额外修改

## 第四阶段：Evaluator Agent 更新

此阶段从三个 Evaluator Agent 中移除回溯决策逻辑，改为纯诊断模式。

- [x] 4.1 修改 `plugins/dev-team/agents/test-execution-evaluator.md`
  - 更新 frontmatter description：移除 "and sets verdict and backtrack_to"，改为 "and sets verdict and diagnoses root cause"
  - 移除 Step 4（"Apply diagnostic decision tree"，第 70-116 行）中所有设置 `backtrack_to` 的分支决策：
    - 六种分支不再输出 `backtrack_to`
    - "无法判断"分支不再返回 backtrack 建议给 main agent
  - 重构 Step 4 为纯诊断步骤：分析失败根因，确定诊断类型，**不输出 `backtrack_to`**
  - Step 5（构建 findings，第 118-137 行）中移除 "回溯目标: ${backtrack_to}" 行
  - Step 6（第 139-143 行）中移除向 phase_log 传递 `backtrack_to` 的指令
  - 新增约束："禁止设置 backtrack_to。诊断结果通过 report 和 checklist 传递给 main agent，回溯路由决策由 main agent 负责。"
  - Step 1 和 Step 3 中的 `backtrack_to: null` 引用：保留为 "backtrack_to: null"（与新的 phaseLogInputSchema 兼容——该字段已移除，但写 null 也不会被拒绝，因为 Zod 会忽略未知字段）或者直接移除该引用。建议保留为安全粘合。
  - Coverage sub-check（第 63-68 行）中移除 `set backtrack_to: "test-design"`，改为在 report 中描述覆盖率不达标详情

- [x] 4.2 修改 `plugins/dev-team/agents/code-review-evaluator.md`
  - 移除 Step 8："Backtrack if design contradictions found"（第 45 行）
  - 替换为："If design contradictions found, describe them in report with file:line evidence. Include the specific design.md requirement and the code contradiction."
  - 新增约束："禁止设置 backtrack_to。回溯路由决策由 main agent 负责。如果发现设计矛盾，在 report 中详细描述并提供文件位置。"
  - 更新 frontmatter description 以反映纯诊断角色

- [x] 4.3 修改 `plugins/dev-team/agents/acceptance-evaluator.md`
  - 移除 Step 7："Backtrack if requirements gaps found"（第 42 行）
  - 替换为："If requirements gaps found, list unmet AC-IDs in report. For each unmet AC, provide the AC-ID and the reason it's not satisfied."
  - 新增约束："禁止设置 backtrack_to。回溯路由决策由 main agent 负责。如果发现未满足的验收标准，在 report 中列出具体 AC-ID。"
  - 更新 frontmatter description 以反映纯诊断角色

## 第五阶段：Skill 回溯决策流程迁移

此阶段将回溯决策逻辑从 Evaluator Agent 迁移到 Skill 层的统一模式。所有 skill 使用一致的决策模式，不继承 evaluator 的特有决策树逻辑。

**统一回溯决策模式（适用于所有 phase 和 workflow）：**
```
Evaluator 执行完成后调用 phase_next
phase_next 返回 last_result (最新 entry 快照)

if verdict == "pass" → 继续下一 phase / 完成
if verdict == "fail":
  1. 分析 Evaluator 的诊断 report 确定失败根因
  2. 三叉决策分支:
     - retry：继续循环（phase_next 自然重试，因无 pass entry）
     - backtrack：调用 backtrack(change, phase, target, reason)
     - ask-user：使用 AskUserQuestion 请求用户确认
  3. 继续循环
```

- [x] 5.1 修改 `plugins/dev-team/skills/phase-test-execution/SKILL.md`
  - Step 3c 中增加统一回溯决策逻辑：
    ```
    3c. Check verdict via last_result:
      result = mcp__plugin_dev-team_dev-team__phase_next(change)
      if result.last_result.verdict == "pass" → phase complete
      if result.last_result.verdict == "fail":
        1. 读取 Evaluator 的诊断 report（来自 last_result.report）
        2. 分析 report 中的诊断类型，确定回溯目标或重试
        3. 决策：
           - 重试：重新执行 Executor → Evaluator 循环
           - 回溯：mcp__plugin_dev-team_dev-team__backtrack({change, phase, backtrack_to, backtrack_reason})
             根据 Evaluator 的 report 确定 backtrack_to 目标：
             - 语法错误 → "test-gen"
             - 逻辑错误 → "implement"
             - 设计冲突/覆盖率不达标 → "test-design"
             - 接口签名不匹配 → "dev-design"
           - 无法判断 → AskUserQuestion 请求用户选择回溯目标
           (注：以上目标映射基于 Evaluator 的诊断 report 动态分析，非硬编码决策树)
        4. 继续循环
    ```
  - **不继承 test-execution-evaluator 的 6 分支决策树**。skill 通过分析 Evaluator 的诊断 report（report 文本内容）动态确定回溯目标
  - 移除旧有的 Step 3c 逻辑（只读 eval.json 判断 pass/fail + 检查 backtrack_to 字段）
  - 更新 frontmatter description 移除 Evaluator "sets backtrack_to" 的描述
  - Step 4 报告格式中移除对 `backtrack_to` 字段的引用

- [x] 5.2 修改 `plugins/dev-team/skills/phase-code-review/SKILL.md`
  - Step 4 中将"Check backtrack"替换为统一回溯决策逻辑：
    ```
    4. Check verdict:
      result = mcp__plugin_dev-team_dev-team__phase_next(change)
      
      if result.last_result.verdict == "pass" → Step 5 (report)
      if result.last_result.verdict == "fail":
        - 检查 Evaluator 的 report 是否提到设计矛盾
        - 如果是：mcp__plugin_dev-team_dev-team__backtrack({change, phase:"code-review", backtrack_to:"dev-design", backtrack_reason: report中的原因})
        - 其他 fail 原因：显示 fail 详情
        - 继续循环
    ```
  - 移除旧有的"Read latest phase code-review entry from eval.json"逻辑
  - 移除对 `backtrack_to` 字段的读取
  - 更新 frontmatter description：移除 "Can set backtrack_to to 'dev-design'" 描述

- [x] 5.3 修改 `plugins/dev-team/skills/phase-acceptance/SKILL.md`
  - Step 4 中将"Check backtrack"替换为统一回溯决策逻辑：
    ```
    4. Check verdict:
      result = mcp__plugin_dev-team_dev-team__phase_next(change)
      
      if result.last_result.verdict == "pass" → Step 5 (report)
      if result.last_result.verdict == "fail":
        - 检查 Evaluator 的 report 是否提到未满足的 AC-ID
        - 如果是：mcp__plugin_dev-team_dev-team__backtrack({change, phase:"acceptance", backtrack_to:"proposal", backtrack_reason: report中的原因})
        - 其他 fail 原因：显示 fail 详情
        - 继续循环
    ```
  - 移除旧有的"Read latest phase acceptance entry from eval.json"逻辑
  - 移除对 `backtrack_to` 字段的读取
  - 更新 frontmatter description：移除 "Can set backtrack_to to 'proposal'" 描述

- [x] 5.4 修改 `plugins/dev-team/skills/workflow-requirement/SKILL.md`
  - Step 2 循环中增加回溯决策流程：
    ```
    LOOP:
      result = mcp__plugin_dev-team_dev-team__phase_next(change)
      
      -- 现有逻辑：执行 planner / evaluator（不变）--
      if result.planner:
        Agent({...})
      if result.evaluator:
        Agent({...})
      
      -- 新增：Evaluator 执行后，再次调用 phase_next 获取 last_result --
      after_eval_result = mcp__plugin_dev-team_dev-team__phase_next(change)
      
      if after_eval_result.error → STOP
      if after_eval_result.done → Step 3
      
      if after_eval_result.last_result:
        if after_eval_result.last_result.verdict == "pass" → 继续 LOOP
        if after_eval_result.last_result.verdict == "fail":
          1. 分析 last_result.report 确定失败类型
          2. 三叉决策分支：
             - retry：继续 LOOP（phase_next 自然处理重试）
             - backtrack：调用 mcp__plugin_dev-team_dev-team__backtrack(...)
             - ask-user：AskUserQuestion 请求用户确认忽略或回溯
          3. 继续 LOOP
    
    输出: "[Round {result.round}/20] [Phase {result.phase_index}/{result.total_phases}] {result.next_phase}: executed"
    ```

- [x] 5.5 修改 `plugins/dev-team/skills/workflow-test-only/SKILL.md`
  - 与 workflow-requirement 相同的回溯决策流程
  - 在 Evaluator 执行后增加 `last_result` 检查和三叉决策
  - 保留现有的 code-bugs-found 报告逻辑（`eval_result indicates code bugs found`）
  - 当 Evaluator 发现代码 bug 且 verdict fail 时，现有逻辑已使用 AskUserQuestion，此流程保留

- [x] 5.6 修改 `plugins/dev-team/skills/phase-proposal/SKILL.md`
  - Step 4（P→E Loop）中增加统一回溯决策逻辑：
    ```
    Step 4c. Verdict:
      result = mcp__plugin_dev-team_dev-team__phase_next(change)
      
      if result.last_result.verdict == "pass" → Step 5
      if result.last_result.verdict == "fail":
        - 重试：回 Step 4a 重新运行 Planner 和 Evaluator（最多 5 次）
        - 或用 backtrack 回溯到更早 phase
        - 或 AskUserQuestion
    ```
  - 保留现有的 fail→redo 循环（最多 5 次），增加回溯路径

- [x] 5.7 修改 `plugins/dev-team/skills/phase-dev-design/SKILL.md`
  - Step 4（P→E Loop）中增加统一回溯决策逻辑（同上 5.6 的模式）
  - 移除 Step 3 "Check backtrack"逻辑（读取 eval.json 检查 `backtrack_to = "dev-design"`）
  - 替代为：Step 3 改为直接 Gate check 后进入 Step 4 的 P→E Loop（因为回溯决策统一由 Step 4c 中的 last_result 处理）

- [x] 5.8 修改 `plugins/dev-team/skills/phase-test-design/SKILL.md`
  - Step 3c（Verdict）中增加统一回溯决策逻辑（同 5.6 模式）

- [x] 5.9 修改 `plugins/dev-team/skills/phase-implement/SKILL.md`
  - Step 3c（Verdict）中增加统一回溯决策逻辑（同 5.6 模式）

- [x] 5.10 修改 `plugins/dev-team/skills/phase-test-gen/SKILL.md`
  - Step 3c（Verdict）中增加统一回溯决策逻辑（同 5.6 模式）

## 第六阶段：版本升级和收尾

此阶段完成最后的版本和验证工作。

- [x] 6.1 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号
  - 从 `"2.8.13"` 递增为 `"2.9.0"`
  - 遵循语义化版本：minor 递增（新增 backtrack MCP 工具，属于功能增强）

- [x] 6.2 验证已删除函数的引用清理
  - 确认 `handleBacktrackMarking()`、`validateBacktrackReason()`、`buildBacktrackHint()` 已无任何引用
  - 使用 grep 搜索这些函数名确认无残留引用
  - 确认 `computeAllowedBacktrackPhases()` 已无任何引用（该函数已移除）

- [x] 6.3 验证 Schema 兼容性
  - 确认 `phaseLogInputSchema` 不再包含 `backtrack_to` 和 `backtrack_reason`
  - 确认 `phaseLogSchema` 仍包含这两个字段（向后兼容）
  - 确认 `phaseNextOutputSchema` 包含 `last_result` 字段
  - 运行 `tsc --noEmit` 检查类型错误

- [x] 6.4 验证 MCP 工具注册一致性
  - 确认 `backtrack` 工具已注册且 handler 正确
  - 确认 `backtrack`, `phase_log`, `phase_next` 三个工具的 input/output schema 与命令实现一致
