# 设计: add-workflow-requirement-skill

> **变更**: add-workflow-requirement-skill
> **日期**: 2026-06-01
> **基于**: proposal.md, specs/

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| phase-proposal skill | 单 phase 执行：解析 change name、gate check、P→E loop（planner 写入 + evaluator 检查）、verdict 循环 | `plugins/dev-team/skills/phase-proposal/SKILL.md` | MCP eval/check, MCP eval/log, proposal-planner agent, proposal-evaluator agent | SKILL.md (Claude Code skill, disable-model-invocation: true) |
| proposal-planner agent | 读取模板和上下文，写入 proposal.md + specs/ 目录 | `plugins/dev-team/agents/proposal-planner.md` | Read/Write/Grep/Glob/Bash 工具 | agent.md (model: opus) |
| proposal-evaluator agent | 根据静态 checklist 评估 proposal.md + specs/，通过 MCP eval/log 写入结果 | `plugins/dev-team/agents/proposal-evaluator.md` | Read 工具, MCP eval/log | agent.md (model: opus, 重命名自 requirements-evaluator) |
| workflow-requirement skill | 薄编排循环：组装上下文 → eval/next → Agent → eval/next → ...，完成后停止并通知用户手动 archive。无硬编码 phase 知识 | `plugins/dev-team/skills/workflow-requirement/SKILL.md` | MCP eval/next, MCP eval/log, 各 phase agent | SKILL.md (disable-model-invocation: true) |
| eval/next MCP tool | 服务端决定下一个执行的 phase、agent_type、prompt；处理 gate check、skip、retry、backtrack、round limit | `plugins/dev-team/bin/src/commands/eval-next.ts` (新增) | eval.json, workflow.ts, change.ts | TypeScript, MCP SDK |
| eval.schema.json | 定义 eval.json 中 phase 枚举值为 `01-proposal` ~ `09-acceptance` | `plugins/dev-team/templates/artifacts/eval.schema.json` | 无 | JSON Schema |
| workflow.ts | 定义 phase 顺序数组，含 `01-proposal` | `plugins/dev-team/bin/src/lib/workflow.ts` | 无 | TypeScript |

### 现有组件变更（安全加固）

此变更对以下现有 Phase 02-09 组件进行最小化安全加固（仅添加 `disallowedTools` frontmatter 字段，不涉及任何逻辑修改）：

| 组件 | 文件位置 | 变更 | 理由 |
|------|----------|------|------|
| dev-design-evaluator | `plugins/dev-team/agents/dev-design-evaluator.md` | +`disallowedTools: Write, Edit` | Evaluator 只读评估 artifact，不需要写权限 |
| test-design-evaluator | `plugins/dev-team/agents/test-design-evaluator.md` | +`disallowedTools: Write, Edit` | Evaluator 只读评估 artifact，不需要写权限 |
| test-gen-evaluator | `plugins/dev-team/agents/test-gen-evaluator.md` | +`disallowedTools: Write, Edit` | Evaluator 只读评估 artifact，不需要写权限 |

**重要**: 生成器 (Generator) 类 agent 不添加此限制：
- `implementation-generator.md` — **不添加** `disallowedTools`，因其职责是使用 Write/Edit 工具将代码写入磁盘
- `test-gen-generator.md` — **不添加** `disallowedTools`，因其职责是使用 Write 工具生成测试骨架文件

### 组件图

```
用户调用 skill 层:

  phase-proposal skill (单 phase)
       |
       |-- Gate: MCP eval/check("01-proposal")
       |-- Agent(proposal-planner) --> 写入 proposal.md + specs/
       |-- Agent(proposal-evaluator) --> MCP eval/log
       |-- 循环直到 verdict=pass 或达到最大重试次数

  workflow-requirement skill (全流程)
       |
       |-- Step 0: 组装上下文（change name + explore 上下文）
       |-- Step 1: 若 change 不存在，scaffold
       |-- Loop:
       |     |-- MCP eval/next(change, workflow_type) --> {next_phase, agent_type, prompt}
       |     |-- if error --> STOP + PushNotification
       |     |-- if done  --> stop + PushNotification（用户手动 archive）
       |     |-- Agent(planner) + Agent(evaluator) + auto_steps
       |     |-- 输出进度

  MCP 服务端 (eval/next):
       |
       |-- 读取 eval.json
       |-- 检查 backtrack_to --> 清空目标阶段及后续条目
       |-- 检查 retry --> 返回同一 phase
       |-- 检查 max_retries --> error
       |-- 检查 round_limit --> error
       |-- 检查 pass --> 返回下一 phase
       |-- 全部完成 --> done: true
```

---

## 数据流

### 流程描述

#### 流程 A: phase-proposal 单 phase 执行

1. 用户输入 `/dev-team:phase-proposal [change-name-or-description]`
2. skill 解析 change name:
   - kebab-case 参数 → 直接验证使用
   - 自然语言参数 → 推导 kebab-case，确认，scaffold
   - 无参数 + explore 上下文 → 提取决策，推导名称，scaffold
   - 无参数 + 无上下文 → 询问用户
3. skill 调用 `MCP eval/check(change, "01-proposal")` 进行门控检查
4. skill 调用 `Agent(proposal-planner, "Write proposal.md and specs/ for change '<name>'.")`:
   - planner 读取 `proposal.md.template`
   - 读取 `openspec spec list --json` 获取现有能力列表
   - 读取 CLAUDE.md
   - 写入 `openspec/changes/<name>/proposal.md`
   - 写入 `openspec/changes/<name>/specs/<capability>/spec.md` (每个能力)
5. skill 调用 `Agent(proposal-evaluator, "Evaluate proposal.md for change '<name>' against checklist.")`:
   - evaluator 读取 proposal.md + specs/
   - 对照 R1-R10 静态 checklist 逐项检查
   - 调用 `MCP eval/log(change, "01-proposal", verdict, report, items)` 追加到 eval.json
6. skill 读取 eval.json 最新条目判断 verdict:
   - pass → 输出报告，结束
   - fail → 重做 step 4-5（最多 5 次）

#### 流程 B: workflow-requirement 全流程编排

1. 用户输入 `/dev-team:workflow-requirement [change-name]`
2. Step 0: skill 解析 change name，检测 explore 上下文
3. Step 1: 若 change 目录不存在，scaffold
4. Step 2: 进入编排循环:
   ```
   loop:
     result = MCP eval/next(change, "requirement")
     if result.error -> STOP + PushNotification
     if result.done  -> stop + PushNotification（完成，请用户手动执行 /dev-team:openspec-archive-change）
     if result.planner -> Agent(result.planner.agent_type, result.planner.prompt)
     for step in result.auto_steps -> Bash(step.command)
     if result.evaluator -> Agent(result.evaluator.agent_type, result.evaluator.prompt)
     输出: "[Round R/20] [Phase N/9] {next_phase}: executed"
   ```
5. 工作流完成后停止，显示完成摘要，提醒用户检查后手动执行 `/dev-team:openspec-archive-change`

#### 流程 C: eval/next 服务端逻辑

```
1. 读取 eval.json 获取所有条目
2. 若 round 计数 > 20: return error: "round_limit_exceeded"
3. 若最新条目的 backtrack_to 非空:
   a. 从目标 phase 开始清空所有后续 eval.json 条目
   b. round + 1
   c. return 目标 phase 的 planner + evaluator
4. 获取最新 phase 条目的 verdict:
   a. 若 verdict=fail 且 attempt < 5: return 同一 phase (retry)
   b. 若 verdict=fail 且 attempt >= 5: return error: "max_retries_exceeded"
   c. 若 verdict=pass: 查找下一未 pass phase
   d. 若 evaluator 条目不存在（中断恢复）: return 该 phase
5. 若所有 phase 已 pass: return done: true
```

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| eval.json entry | phase, timestamp, attempt, verdict, report, items[], backtrack_to, skipped, findings, schema_version | 按 phase 和 timestamp 排序 | `openspec/changes/<name>/eval.json` |
| eval/next 输入 | change (string), workflow_type (string) | - | 请求参数 |
| eval/next 输出 (正常) | done (false), error (null), next_phase, phase_pattern, planner: {agent_type, prompt}, evaluator: {agent_type, prompt}, auto_steps[], total_phases, phase_index, round | - | 响应体 |
| eval/next 输出 (done) | done (true), error (null), next_phase (null), planner (null), evaluator (null) | - | 响应体 |
| eval/next 输出 (error) | done (false), error (string), message (string), next_phase (null) | - | 响应体 |
| explore_context | change_name (string), summary (string), key_decisions (string[]) | 传递给 proposal-planner prompt | 会话内存（不持久化） |

---

## 路由/API 设计

### MCP 工具: eval/next

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP | `dev-team:eval/next` | 返回 workfow 中下一个要执行的 phase、agent_type 和 prompt | `{change, workflow_type?}` | `{done, error, next_phase, planner, evaluator, round, ...}` | MCP stdio |

**eval/next 输入参数:**

| 参数 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `change` | string | true | Change name |
| `workflow_type` | string | false | `"requirement"` (默认), `"bug-fix"`, `"refactor"` |

**eval/next 输出:**

```typescript
// 正常: 有下一个 phase 待执行
{
  done: false,
  error: null,
  next_phase: "01-proposal",          // Phase identifier
  phase_pattern: "DESIGN",             // DESIGN | EXEC | EVAL-ONLY
  planner: {                            // Agent type + prompt (null for EVAL-ONLY)
    agent_type: "dev-team:proposal-planner",
    prompt: "Write proposal.md and specs/ for change '<name>'."
  },
  evaluator: {
    agent_type: "dev-team:proposal-evaluator",
    prompt: "Evaluate proposal.md for change '<name>' against checklist."
  },
  auto_steps: [],                       // Bash commands to execute between planner and evaluator
  total_phases: 9,
  phase_index: 1,
  round: 1
}

// 全部完成
{
  done: true,
  error: null,
  next_phase: null,
  planner: null,
  evaluator: null,
  auto_steps: [],
  total_phases: 9,
  phase_index: 9,
  round: 12
}

// 错误
{
  done: false,
  error: "round_limit_exceeded",       // 或 "max_retries_exceeded"
  message: "超过 20 轮限制，可能存在循环回溯",
  next_phase: null,
  planner: null,
  evaluator: null
}
```

### 现有 MCP 工具（不变）

| 方法 | 路径 | 输入 | 输出 | 变更 |
|------|------|------|------|------|
| MCP | `dev-team:eval/check` | `{change, phase}` | `{passed, phase, block_reasons, ...}` | 无变更（逻辑不变，仅接受新 phase ID） |
| MCP | `dev-team:eval/log` | `{change, phase, verdict, report, items, ...}` | `{written, phase, attempt}` | 无变更（逻辑不变，仅接受新 phase ID） |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **phase-proposal 采用 P→E 模式**：planner 子代理写入，evaluator 子代理检查，与 phase-02/03 一致 | 消除 phase-01 与其余 phase 的模式不一致；使 workflow 技能可以统一调用 Agent 而无需特殊处理 phase-01；explore 上下文通过 prompt 传递 | **备选: 保持 phase-requirements 原名 + 模式不变**。拒绝理由: phase-01 的主模型直接写入模式使得 workflow 技能无法统一调用 subagent，必须为 phase-01 特殊处理；且与 phase-02/03 的 P→E 模式不统一，增加维护成本 |
| D2 | **eval/next 服务端处理所有编排逻辑**：gate check、skip、retry、backtrack、round limit 均由 MCP 服务端决定 | 使 workflow-requirement skill 成为薄循环，不包含任何硬编码 phase 知识；每个 skill 只需要调用 eval/next 并执行返回的 agent；便于未来新增 workflow variant 时无需修改 skill 逻辑 | **备选: 在 skill 中硬编码 phase 顺序和 agent 映射**。拒绝理由: 每个 workflow variant 都需要复制整个编排逻辑；变更 phase 列表时需要修改所有 skill；backtrack/retry 逻辑复杂，在 skill 中实现容易出错 |
| D3 | **workflow-requirement skill 直接调用 Agent（不通过 Skill）** | 避免 Skill→Agent→MCP 的多层嵌套调用；workflow skill 直接使用 Agent/Bash/MCP 原语，与 phase-level skill 独立可执行 | **备选: workflow 调用 phase-level Skill**。拒绝理由: 每个 phase skill 都有独立的 change name 解析和 gate check，workflow 中调用会导致重复逻辑；且 phase skill 的 gate check 结果与 eval/next 冲突 |
| D4 | **phase ID `01-requirements` → `01-proposal`，向后兼容** | 新 eval.json 使用新 ID，旧 eval.json 的 `01-requirements` 条目不做迁移；checkGate 和 eval/log 不做 ID 校验，只是字符串匹配 | **备选: 统一迁移所有旧 eval.json 到新 ID**。拒绝理由: 现有 changes 的 eval.json 不修改，避免破坏历史记录；字符串匹配的两者共存无冲突 |
| D5 | **eval/next 在回溯时清空目标 phase 及后续 eval.json 条目** | 确保 re-evaluation 生效，不会被旧 pass 记录跳过；实现简单：读数组 → 过滤 → 写回 | **备选: 在 eval.json 中添加 `superseded_by` 标记**。拒绝理由: 增加了 eval.json 的数据复杂度；读取和过滤逻辑更复杂；backtrack 次数少，直接清空更可靠 |
| D6 | **workflow_type 固定 phase 表在 MCP 服务端定义** | 新 workflow variant（如 bug-fix、refactor）只需新增一个 phase 表配置 + 一个薄 skill 文件；无需修改现有 skill 的编排逻辑 | **备选: 在 skill 文件中通过参数配置 phase 表**。拒绝理由: 将 phase 表暴露给 skill 层违背"无硬编码 phase 知识"原则；workflow_type 抽象使服务端可以统一控制版本 |
| D7 | **对 Phase 02-09 evaluator agent 添加 `disallowedTools: Write, Edit` 安全加固** | Evaluator 是只读组件 — 它们评估 artifact 但不创建或修改文件。添加 `disallowedTools` 可防止 evaluator 误用写工具，作为 defense-in-depth 安全措施。此变更不修改 evaluator 的识别（name）、模型（model）或逻辑内容（prompt/checklist） | **备选: 遵循"不要修改 Phase 02-09"约束，对 evaluator 不做任何修改**。拒绝理由: 现有 phase-02/03 skill 的 P→E 模式中，evaluator 使用 Write/Edit 工具的唯一场景是意外误操作，没有合法使用路径。`disallowedTools` 仅限制工具调用，不影响 evaluator 的 Read 工具使用和 MCP eval/log 调用。三个 evaluator 的 checklist、prompt 和逻辑均保持不变 |
| D8 | **Generator agent 不添加 `disallowedTools: Write, Edit`** | `implementation-generator` 使用 Write/Edit 直接将代码写入磁盘；`test-gen-generator` 使用 Write 工具创建测试文件。对这些 agent 添加 `disallowedTools: Write, Edit` 会导致功能失效 | **备选: 对所有 agent（包括 generator）统一添加 `disallowedTools`**。拒绝理由: generator 的核心职责就是写文件，添加此限制会使其完全无法工作。设计应区分 evaluator（只读，可加固）和 generator（写文件，不可加固）|

---

## 依赖

### 运行时依赖

- `@modelcontextprotocol/sdk` — MCP 服务端和 Zod schema 定义
- `openspec-cli.sh` — 通过 skill 中的 source 调用，用于 change scaffold、spec list（不在 MCP 服务端调用）

### 构建/测试依赖

- `vite-plus` — 测试框架（现有，workflow.test.ts 需更新）
- `typescript` — 编译（现有）

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| explore 上下文提取不完整，planner 缺少关键信息 | 中 | 中 | proposal-planner 不仅依赖传递的摘要，也读取 explore 技能的输出结构（决策表、What We Figured Out）；保留 Read 工具权限可自行读取 |
| workflow 长时间运行（10-30 min），用户等待焦虑 | 低 | 中 | 每 phase 完成后输出结构化进度行 `[Round R/20] [Phase N/9] {phase}: executed`；完成/阻塞时 PushNotification |
| phase ID 变更导致现有 eval.json 解析异常 | 低 | 低 | eval/check 和 eval/log 做字符串匹配，不校验 ID 枚举值；旧 entry 与新 entry 不冲突；新 change 使用新 ID |
| proposal-planner 缺乏探索上下文导致提案质量下降 | 中 | 低 | planner 保留 Read 工具权限，可自行读取 conversation 上下文；workflow 自动从 explore 会话提取并传递 EXPLORE_CONTEXT_SUMMARY |
| backtrack 循环导致无限重试 | 高 | 低 | 全局 20 轮硬限制，超限立即停止并报告；每轮回溯清空目标 phase 后续 eval 条目，确保 re-evaluation 生效 |
| workflow-requirement 与现有 phase-requirements skill 冲突 | 低 | 中 | phase-requirements skill 删除前用户仍可使用旧命令；speces 中标记为 deprecated，文档引导使用新命令 |

---

## 迁移步骤

1. **新增 eval/next 命令**: 创建 `plugins/dev-team/bin/src/commands/eval-next.ts`，实现服务端编排逻辑
2. **注册 eval/next MCP 工具**: 在 `plugins/dev-team/bin/src/mcp.ts` 中注册新 tool
3. **更新 workflow.ts**: 将 phase `01-requirements` 改为 `01-proposal`；更新测试
4. **新增 proposal-planner agent**: 创建 `plugins/dev-team/agents/proposal-planner.md`
5. **重命名 requirements-evaluator**: 文件重命名为 `proposal-evaluator.md`，更新 phase ID 引用
6. **新增 phase-proposal skill**: 创建 `plugins/dev-team/skills/phase-proposal/SKILL.md`
7. **删除 phase-requirements skill**: 删除 `plugins/dev-team/skills/phase-requirements/` 目录
8. **新增 workflow-requirement skill**: 创建 `plugins/dev-team/skills/workflow-requirement/SKILL.md`
9. **更新 eval.schema.json**: phase descriptions 添加 `01-proposal`
10. **更新 plugin.json**: 版本号递增
11. **Evaluator 安全加固**: 对 `dev-design-evaluator.md`、`test-design-evaluator.md`、`test-gen-evaluator.md` 添加 `disallowedTools: Write, Edit`。确认 `implementation-generator.md` 和 `test-gen-generator.md` 不添加此限制

---

## 待决问题

- 是否需要为 workflow-requirement 增加 `--workflow-type` CLI 参数以便未来扩展？当前决定暂不添加，默认值为 `"requirement"`，未来可通过 skill 中直接修改默认值支持
