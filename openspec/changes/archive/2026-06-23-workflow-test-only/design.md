# 设计: workflow-test-only

> **变更**: workflow-test-only
> **日期**: 2026-06-22
> **基于**: proposal.md, specs/

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `workflow.json` | change 目录内 `workflow_type` 唯一数据源 | `openspec/changes/<name>/workflow.json` | 由 skill / proposal-planner 写入 | JSON |
| `change-config` | 解析 `workflow.json`；`getWorkflowType()` 缺省 `"requirement"` | `plugins/dev-team/bin/src/lib/change-config.ts` | `getChangeDir()` | TypeScript |
| `PHASE_TEST_ONLY` | test-only 六阶段表：agent 映射、prompt 定制（含 WORKFLOW_CONTEXT） | `plugins/dev-team/bin/src/lib/workflow.ts` | 各 phase agent 定义 | TypeScript |
| `PHASE_TEST_ONLY_PREREQUISITES` | test-only 专用 prerequisite DAG | `plugins/dev-team/bin/src/lib/workflow.ts` | 无 | TypeScript |
| `PHASE_TABLES` / `PHASE_PREREQUISITES_TABLES` | 注册第四种 `workflow_type: "test-only"` | `plugins/dev-team/bin/src/lib/workflow.ts` | `PHASE_TEST_ONLY` | TypeScript |
| `phase_next` | 通过 `getWorkflowType(change)` 选表；返回下一 phase、agent、prompt | `plugins/dev-team/bin/src/commands/phase-next.ts` | `change-config`, `workflow.ts`, `eval-json.ts` | TypeScript, MCP |
| `phase_log` / `handleBacktrackMarking` | 通过 `getWorkflowType(change)` 做工作流感知 backtrack 校验 | `plugins/dev-team/bin/src/commands/phase-log.ts` | `change-config`, `getPhaseTable()` | TypeScript, MCP |
| `code-analyze-planner` | 逆向分析现有代码，产出与 `test-design-planner` 兼容的 `design.md`（不含 `tasks.md`） | `plugins/dev-team/agents/code-analyze-planner.md` | proposal.md, design.md.template, 代码库 | agent.md |
| `code-analyze-evaluator` | 评估 design.md 完整性（不要求 tasks.md）；写入 `02-code-analyze` eval 条目 | `plugins/dev-team/agents/code-analyze-evaluator.md` | proposal.md, design.md, phase_log MCP | agent.md |
| `proposal-planner` | `workflow.json` 缺失时 AskQuestion 确认 workflow_type 并写入 | `plugins/dev-team/agents/proposal-planner.md` | `workflow.json`, proposal 模板 | agent.md |
| `workflow-test-only` skill | 薄编排循环；scaffold 写 `workflow.json`；bug 发现时写报告 | `plugins/dev-team/skills/workflow-test-only/SKILL.md` | phase_next MCP, 各 phase agent | SKILL.md |
| `workflow-requirement` skill | scaffold 写 `{"workflow_type":"requirement"}`；循环不传 MCP workflow_type | `plugins/dev-team/skills/workflow-requirement/SKILL.md` | phase_next MCP | SKILL.md |

### 复用组件（不修改 agent 定义）

| 组件 | 在 test-only 中的角色 |
|------|----------------------|
| `proposal-planner` / `proposal-evaluator` | `01-proposal`（prompt 在 workflow.ts 中定制为测试导向；planner 负责 workflow.json 确认） |
| `test-design-planner` / `test-design-evaluator` | `03-test-design`（读取 code-analyze 产出的 design.md） |
| `test-gen-generator` / `test-gen-evaluator` | `04-test-gen` |
| `unit-test-executor` / `unit-test-evaluator` | `06-unit-test`（evaluator prompt 追加 WORKFLOW_CONTEXT） |
| `integration-test-executor` / `integration-test-evaluator` | `08-integration-test`（evaluator prompt 追加 WORKFLOW_CONTEXT） |

### 组件图

```
workflow.json  (openspec/changes/<name>/workflow.json)
    { "workflow_type": "test-only" | "requirement" | ... }
         ^
         | 写入                          | 读取
         |                               v
workflow-* skill (scaffold)          getWorkflowType(change)
proposal-planner (缺省时确认)              |
         |                               +--> phase_next(change)
         |                               +--> phase_log(change, ...)
         v
用户: /dev-team:workflow-test-only <change>
         |
         v
workflow-test-only skill
  Step 0: change 解析
  Step 1: openspec new change + 写 workflow.json
  Step 2: LOOP phase_next(change)   <-- 无 workflow_type 参数
         |
         +--> 01-proposal        [proposal-planner → proposal-evaluator]
         +--> 02-code-analyze    [code-analyze-planner → code-analyze-evaluator] --> design.md
         +--> 03-test-design     [test-design-planner → test-design-evaluator]
         +--> 04-test-gen         [test-gen-generator → test-gen-evaluator]
         +--> 06-unit-test        [unit-test-executor → unit-test-evaluator*]
         +--> 08-integration-test [integration-test-executor → integration-test-evaluator*]
         |
         * evaluator prompt 含 WORKFLOW_CONTEXT；agent .md 决策树不变

phase_log (evaluator 调用，仅传 change):
         |
         +--> workflowType = getWorkflowType(change)   // 读 workflow.json
         +--> handleBacktrackMarking(workflowType)
         |         getPhaseTable(workflowType) 校验 target 成员资格
         |         无效 target → 抛错，不写入 eval.json
         |
         +--> 有效 target → markPhaseStale → 写入 eval 条目
```

---

## 数据流

### 流程 A: workflow.json 生命周期

1. **workflow-* skill 入口**：Step 1 scaffold 后立即写入 `openspec/changes/<name>/workflow.json`
   - `workflow-test-only` → `{"workflow_type": "test-only"}`
   - `workflow-requirement` → `{"workflow_type": "requirement"}`
2. **phase-proposal 等非 workflow 入口**：`proposal-planner` Process 开头读取 `workflow.json`
   - 文件不存在或缺少 `workflow_type` → AskQuestion 确认 → 写入 `workflow.json` → 继续写 proposal
   - 已设置 → 跳过确认，直接写 proposal
3. **MCP 引擎读取**：`phase_next` / `phase_log` 每次调用 `getWorkflowType(change)`
   - 有 `workflow.json` 且含有效 `workflow_type` → 返回该值
   - 文件不存在或字段缺失 → 返回 `"requirement"`（向后兼容旧 change）
4. **约束**：workflow 执行中不应手动修改 `workflow.json`（文档约定，非代码强制）

### 流程 B: test-only 六阶段正常推进

1. 用户调用 `/dev-team:workflow-test-only <change>` 或带描述的新 change
2. Skill Step 0 解析 change name（逻辑同 `workflow-requirement`）
3. Skill Step 1（新 change）：`openspec new change "<name>"` → 写入 `workflow.json`
4. Skill Step 2 循环：
   - `phase_next(change)` — 服务端从 `workflow.json` 解析为 `test-only`
   - 返回 `{ next_phase, planner, evaluator, done, total_phases: 6 }`
   - 执行 planner agent → auto_steps → evaluator agent
   - evaluator 调用 `phase_log(change=..., phase=..., ...)` — **不传** `workflow_type`
5. Phase 推进顺序（prerequisite DAG）：
   - `01-proposal` → `02-code-analyze` → `03-test-design` → `04-test-gen` → `06-unit-test` / `08-integration-test`
6. 六阶段全部 non-stale pass → `phase_next` 返回 `done: true`，无 `09-acceptance`

### 流程 C: 02-code-analyze 产出 design.md 链路

1. `code-analyze-planner` 读取 `proposal.md`（测试覆盖范围、AC）与 `design.md.template`
2. 通过 Read/Grep/Glob 探索 proposal 范围内的现有源码
3. 写入 `openspec/changes/<change>/design.md`：
   - 架构组件（含文件路径）
   - 数据流（现有代码路径）
   - 路由/API（若适用）
   - 已观察到的架构决策（非未来设计）
   - 变更范围 / 模块边界
4. **不**写入 `tasks.md`；**不**包含测试策略章节
5. `code-analyze-evaluator` 对照 checklist 评估，调用 `phase_log(phase="02-code-analyze")`
6. `03-test-design` 的 `test-design-planner` 读取上述 `design.md`（与 requirement 流程相同输入格式）

### 流程 D: 测试发现生产代码 bug 的自适应流程

```
unit-test-evaluator / integration-test-evaluator
    |
    | 决策树判定为代码 bug → 尝试 backtrack_to: "05-implement" 等
    v
phase_log(change=..., backtrack_to="05-implement") 
    |
    | getWorkflowType(change) → "test-only"
    | handleBacktrackMarking: "05-implement" ∉ PHASE_TEST_ONLY
    v
抛出错误: 当前工作流 test-only 不包含 phase '05-implement'。可用的 phases: [...]
    |
    | eval.json 未被修改
    v
evaluator 收到 MCP 错误（WORKFLOW_CONTEXT 指令引导）
    |
    v
phase_log(change=..., verdict="fail", backtrack_to=null,
          report="测试发现 N 个代码 bug (工作流无 implement 阶段): ...")
    |
    v
workflow-test-only skill:
    - 写入 openspec/changes/<change>/reports/code-bugs-found.md
    - 通知用户
    - 询问：继续 integration-test / 终止 workflow
```

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `WorkflowConfig` | `workflow_type: "requirement" \| "bug-fix" \| "refactor" \| "test-only"` | 决定 phase 表与 backtrack 校验规则 | `openspec/changes/<change>/workflow.json` |
| `EvalEntry` | `phase`, `verdict`, `report`, `items`, `backtrack_to`, `attempt`, `stale`, `skipped` | 同一 change 的多 phase 条目序列 | `openspec/changes/<change>/eval.json` |
| `phaseNextInput` | `change` | 单次 phase 决策请求；**无** `workflow_type` | MCP 请求体 |
| `phaseLogInput` | `change`, `phase`, `verdict`, `report`, `items`, `backtrack_to?`, ... | 单次 evaluator 写入；**无** `workflow_type` | MCP 请求体 |
| `phaseNextOutput` | `next_phase`, `planner`, `evaluator`, `done`, `total_phases`, `round` | 由 eval.json + workflow 表推导 | MCP 响应 |
| `design.md`（code-analyze 产出） | 架构组件、数据流、路由、决策、模块边界 | 被 `test-design-planner` 消费；**无** tasks.md | `openspec/changes/<change>/design.md` |
| Bug 报告 | bug 摘要、phase、eval report 引用 | 由 skill 在 06/08 fail 时生成 | `openspec/changes/<change>/reports/code-bugs-found.md` |

---

## 路由/API 设计

### MCP: `phase_next`

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP tool | `mcp__plugin_dev-team_dev-team__phase_next` | 返回下一待执行 phase | `{ change: string }` | `{ next_phase, planner, evaluator, done, total_phases, round, error? }` | MCP 会话 |

**服务端行为**：
- `runPhaseNext()` 调用 `getWorkflowType(change)` 读取 `workflow.json`，缺省 `"requirement"`
- **不再**接受 `workflow_type` MCP 参数（AC-17）

**test-only 特有行为**（当 `workflow.json` 为 `test-only` 时）：
- `total_phases` = 6
- 首 phase：`01-proposal`（测试导向 prompt）
- `02-code-analyze` 使用 `code-analyze-planner` / `code-analyze-evaluator`
- `06-unit-test` / `08-integration-test` evaluator prompt 追加 WORKFLOW_CONTEXT

### MCP: `phase_log`

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP tool | `mcp__plugin_dev-team_dev-team__phase_log` | 记录 phase verdict | `{ change, phase, verdict, report, items, backtrack_to?, skipped? }` | `{ written, phase, attempt }` | MCP 会话 |

**服务端行为**：
- `runPhaseLog()` 调用 `getWorkflowType(change)` 确定 workflow 变体
- **不再**接受 `workflow_type` MCP 参数（AC-17）
- `handleBacktrackMarking()` 用 `getPhaseTable(workflowType)` 校验 `backtrack_to` 目标
- 非法目标：抛错，**不**写入 eval.json
- 错误格式：`当前工作流 {workflow_type} 不包含 phase '{target}'。可用的 phases: {list}`

### Skill 入口

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| Skill | `/dev-team:workflow-test-only [change]` | 执行 test-only 六阶段编排 | change name 或描述 | 完成摘要 / bug 报告 / 用户选择 | Claude Code skill |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 新增独立 `workflow_type: "test-only"` 而非扩展 `requirement` 的 skip 参数 | 阶段表与 prerequisite 差异大（无 implement/review/acceptance）；独立表更清晰，不影响现有三种 workflow 回归 | 在 requirement 表上加 `skip_phases[]` 参数 — 复杂度高，phase_next 逻辑分支膨胀 |
| D2 | **`workflow.json` 作为 workflow_type 唯一数据源**，MCP 移除 `workflow_type` 参数 | change 目录自描述；skill/agent 无需每次传参；消除 MCP 参数与 change 状态不一致风险 | 保留 MCP `workflow_type` 参数 — 易与 change 目录状态冲突；`.openspec.yaml` 存储 — 非 change 专属、与 OpenSpec CLI 耦合 |
| D3 | 新增 `02-code-analyze` phase + agent 对，替代 `02-dev-design` | `test-design-planner` 硬依赖 `design.md`；逆向分析现有代码产出同格式 artifact，无需修改下游 agent | 修改 `test-design-planner` 直接读源码 — 破坏现有 agent 契约，影响 requirement workflow |
| D4 | `code-analyze-planner` 只产出 `design.md`，不产出 `tasks.md` | test-only 无 `05-implement` 阶段；tasks.md 对测试补全场景无意义 | 仍产出 tasks.md 供参考 — 增加噪音，evaluator 需额外忽略 |
| D5 | backtrack 校验在引擎层（`phase_log` + `getWorkflowType`）硬拒绝，evaluator prompt 层自适应 | agent .md 决策树在 requirement/bug-fix 间共用，不宜为 test-only 分叉；引擎拒绝防止 eval.json 污染 | 修改 `unit-test-evaluator.md` 决策树 — 影响所有 workflow，维护成本高 |
| D6 | WORKFLOW_CONTEXT 注入 workflow.ts evaluator prompt，不改 agent .md | 差异仅在 test-only 的 06/08 阶段；prompt 层定制符合现有 workflow.ts 模式 | 新建 test-only 专用 evaluator agent — 重复代码，决策树同步负担 |
| D7 | bug 发现视为测试目标达成，skill 写报告 + 用户选择继续/终止 | test-only 目的是验证/补全测试，发现生产 bug 即有价值产出 | 自动终止 workflow — 丢失 integration-test 机会 |
| D8 | `workflow-*` skill scaffold 写 `workflow.json`；`proposal-planner` 补全缺失场景 | 双入口覆盖：workflow skill 自动设置；phase-proposal 直接调用时用户确认 | 仅 skill 写入 — phase-proposal 入口无法确定 workflow 变体 |
| D9 | 无 `workflow.json` 时缺省 `"requirement"` | 向后兼容现有 change 与 eval.json；不强制迁移 | 强制 scaffold 时创建 — 破坏历史 change |

---

## 依赖

### 运行时依赖

- OpenSpec CLI — change scaffold、`openspec new change`
- dev-team MCP server — `phase_next`, `phase_log`, `change_list`
- 现有 test phase agents — test-design / test-gen / unit-test / integration-test 全套

### 构建/测试依赖

- TypeScript + Zod — schema 与 MCP 命令实现
- 现有 `workflow.test.ts` / `phase-next.test.ts` / `phase-log.test.ts` 测试框架

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| code-analyze design.md 格式与 test-design-planner 不兼容 | 高 | 中 | planner 遵循 design.md.template；evaluator checklist 强制组件表、数据流、文件路径 |
| 逆向分析遗漏边界模块 | 中 | 中 | proposal AC 明确覆盖范围；evaluator 交叉验证 proposal AC 与 design 模块表 |
| 旧 change 无 `workflow.json` 误用 requirement 表 | 中 | 中 | 缺省 `"requirement"` 保持兼容；`proposal-planner` 在非 workflow-* 入口主动确认 |
| workflow 进行中手动修改 `workflow.json` | 中 | 低 | 文档约定；`workflow.json` 为唯一数据源 |
| `02-code-analyze` 未加入 eval.schema.json phase 枚举 | 低 | 低 | 实现时检查 schema；若校验失败则同步更新枚举 |
| 现有 skill/phase skill 仍传 `workflow_type` 给 MCP | 低 | 中 | 更新 `workflow-requirement`；MCP schema 移除字段后旧调用会被忽略（服务端以文件为准） |

---

## 迁移步骤

1. 新增 `change-config.ts`（`readWorkflowConfig`, `getWorkflowType`）
2. 注册 `PHASE_TEST_ONLY` 与 prerequisite 表
3. `phase-next.schema.ts` 移除 `workflow_type`；`phase-next.ts` 改用 `getWorkflowType(change)`
4. `phase-log.ts` 的 `handleBacktrackMarking` 改用 `getPhaseTable(getWorkflowType(change))` 替代全局 `getPhaseIndex()` 成员校验
5. `mcp.ts` 停止转发 `workflow_type` 至 `runPhaseNext` / `runPhaseLog`
6. 更新 `workflow-requirement` / 新增 `workflow-test-only` skill：scaffold 写 `workflow.json`，循环仅传 `change`
7. 更新 `proposal-planner.md`：缺省时确认并写入 `workflow.json`
8. 测试 fixture 改用 `workflow.json` 设置 workflow 变体（替代向 MCP 传 `workflow_type`）

---

## 待决问题

- `eval.schema.json` 是否需新增 `02-code-analyze` phase 枚举：实现阶段检查 CLI/MCP 校验行为后决定（proposal 标注为「若需新增则单独评估」）。
- 其他 `phase-*` 独立 skill（如 `phase-dev-design`）仍含 `workflow_type="requirement"` 的 MCP 调用：本次 scope 不修改；MCP 移除参数后该字段将被忽略，后续可单独清理。
