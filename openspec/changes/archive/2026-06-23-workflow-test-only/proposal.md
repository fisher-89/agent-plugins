# 提案: workflow-test-only

> **变更**: workflow-test-only
> **日期**: 2026-06-22
> **状态**: draft

---

## 问题

PGE 工作流引擎目前提供 `requirement`、`bug-fix`、`refactor` 三种 workflow_type，均假设需要编写或修改实现代码。实际开发中常见另一类场景：**实现代码已存在，但测试覆盖不足或测试工程需要补全/修复**。这类场景若走 `workflow-requirement`，会强制执行 `02-dev-design`、`05-implement`、`07-code-review`、`09-acceptance` 等与本目标无关的阶段，浪费轮次且可能误改已有代码。

此外，`test-design-planner` 依赖 `design.md` 作为输入，而 test-only 流程没有 `02-dev-design` 阶段来产出该文件，存在 artifact 链路断裂。

---

## 提案

新增第四种 workflow_type **`test-only`**，专门服务于「已有代码、补全测试」场景。同时将 **workflow_type 的唯一数据源** 迁移至 change 目录的 `workflow.json` 文件。

### 架构变更：`workflow.json` 作为 workflow_type 唯一数据源

1. **`workflow.json` 记录 `workflow_type` 字段** — 取值 `"requirement"` | `"bug-fix"` | `"refactor"` | `"test-only"`，存储于 `openspec/changes/<name>/workflow.json`
2. **`phase_next` 与 `phase_log` 读取 `workflow_type`** — 二者从 change 目录的 `workflow.json` 读取 `workflow_type`，以此选择阶段表与 backtrack 校验规则
3. **`workflow-*` skill 在 scaffold 时写入 `workflow_type`** — 例如 `workflow-test-only` 在 Step 1 创建 change 后，将 `{"workflow_type": "test-only"}` 写入 `workflow.json`
4. **`proposal-planner` 确认 workflow_type** — 若 `workflow.json` 不存在或未设置 `workflow_type`（用户未通过 `workflow-*` 命令进入，例如直接调用 `phase-proposal`），planner 向用户询问并确认 workflow_type，写入 `workflow.json` 后再继续

### test-only 六阶段流水线（其余设计不变）

1. **六阶段测试专用流水线**（跳过实现与验收）：
   - `01-proposal` — 测试导向提案（覆盖缺口、测试策略、验收标准）
   - `02-code-analyze` **[新阶段]** — 逆向分析现有代码，产出与 `test-design-planner` 兼容的 `design.md`（不产出 `tasks.md`）
   - `03-test-design` → `04-test-gen` → `06-unit-test` → `08-integration-test` — 复用现有 agent，prompt 在 `workflow.ts` 中按 workflow_type 定制

2. **新 agent 对**（DESIGN 模式）：
   - `code-analyze-planner` — 读取 proposal.md + 现有代码库，输出 `design.md`
   - `code-analyze-evaluator` — 评估 design.md 完整性（不要求 tasks.md）

3. **新 skill `workflow-test-only`** — 薄编排循环，调用 `phase_next(change=<name>)`；scaffold 时写入 `workflow.json`

4. **结束条件** — 无 `09-acceptance` 阶段；当 `06-unit-test` 与 `08-integration-test` 均 pass 时 workflow 正常完成。若测试发现生产代码 bug，亦视为测试目标达成（见第 6 点）。

5. **Prerequisite DAG**：
   ```
   01-proposal         : []
   02-code-analyze     : [01-proposal]
   03-test-design      : [01-proposal, 02-code-analyze]
   04-test-gen         : [03-test-design]
   06-unit-test        : [04-test-gen]
   08-integration-test : [04-test-gen]
   ```

`code-analyze` 阶段是架构关键：它替代 `dev-design` 为 test track 提供 `design.md`，使 `test-design-planner` 无需修改即可复用。

6. **测试发现生产代码 bug 的处理**（引擎层拦截 + evaluator 自适应）：

   **问题**：`unit-test-evaluator` / `integration-test-evaluator` 的决策树在发现代码 bug 时会设置 `backtrack_to: "05-implement"` 或 `"02-dev-design"`，但 test-only 阶段表中不存在这些 phase。当前 `phase_log` 用全局 `getPhaseIndex()` 校验，无效 backtrack 会被写入 eval.json，随后 `phase_next` 报错 `invalid_backtrack_target`。

   **引擎层拦截**：
   - `phase_log` 从 `workflow.json` 读取 `workflow_type`（缺省 `"requirement"`，向后兼容旧 change）
   - `handleBacktrackMarking()` 用 `getPhaseTable(workflow_type)` 替代全局 `getPhaseIndex()` 校验 backtrack 目标
   - 目标不在当前 workflow 阶段表中时，**拒绝写入** eval.json，抛出错误：`当前工作流 {workflow_type} 不包含 phase '{target}'。可用的 phases: {list}`

   **Evaluator 自适应**（不修改 agent .md 决策树）：
   - evaluator 收到 MCP 错误后，以 `verdict: "fail"`, `backtrack_to: null` 重新调用 `phase_log`，report 记录 bug 详情
   - `workflow.ts` 中 `PHASE_TEST_ONLY` 的 `06-unit-test` / `08-integration-test` evaluator prompt 包含 WORKFLOW_CONTEXT 指令，引导上述自适应流程

   **Workflow skill 终止/继续**：
   - skill 收到 evaluator 返回的 bug 发现结果后，在 `reports/` 生成 bug 报告、通知用户
   - 用户可选择继续执行 integration-test 或终止 workflow

---

## 能力

### 新增能力

- **test-only-workflow** — 六阶段测试专用 workflow_type，含 `02-code-analyze` 阶段、独立 prerequisite 表、phase 推进、完成语义、测试发现代码 bug 时的 evaluator 自适应流程（`specs/test-only-workflow/spec.md`）

### 修改的能力

- **pge-workflow-engine** — 注册 `test-only` workflow_type；`phase_log` 从 `workflow.json` 读取 workflow_type 并做工作流感知 backtrack 校验；保证 `requirement`/`bug-fix`/`refactor` 行为不变（`specs/pge-workflow-engine/spec.md`）
- **workflow-orchestration** — `phase_next` 从 `workflow.json` 读取 workflow_type（移除 MCP 参数）；`workflow.json` 字段规范；扩展 workflow 变体列表（`specs/workflow-orchestration/spec.md`）
- **phase-agents** — 新增 `code-analyze-planner`、`code-analyze-evaluator`；`proposal-planner` 在未设置 workflow_type 时向用户确认（`specs/phase-agents/spec.md`）
- **phase-skills** — 新增 `workflow-test-only` 技能（scaffold 时写入 `workflow.json`）；测试发现代码 bug 时生成报告并通知用户（`specs/phase-skills/spec.md`）

---

## 变更范围

### 实现以下特性

- `plugins/dev-team/bin/src/lib/change-config.ts`（或等价模块）：
  - `readWorkflowConfig(change)` — 解析 `openspec/changes/<change>/workflow.json`
  - `getWorkflowType(change)` — 返回 `workflow_type`，缺省 `"requirement"`
- `plugins/dev-team/bin/src/lib/workflow.ts`：
  - 新增 `PHASE_TEST_ONLY` 数组（6 阶段）
  - 新增 `PHASE_TEST_ONLY_PREREQUISITES` 依赖表
  - 注册到 `PHASE_TABLES` 与 `PHASE_PREREQUISITES_TABLES`
  - `01-proposal` prompt 引导测试覆盖缺口与策略；`03-test-design` 至 `04-test-gen` prompt 与 requirement 一致
  - `06-unit-test` / `08-integration-test` evaluator prompt 含 WORKFLOW_CONTEXT 自适应指令
- `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` — **移除** `workflow_type` 参数
- `plugins/dev-team/bin/src/commands/phase-next.ts` — 调用 `getWorkflowType(change)` 替代 `options.workflow_type`
- `plugins/dev-team/bin/src/commands/phase-log.ts` — `handleBacktrackMarking()` 调用 `getWorkflowType(change)` 校验 backtrack
- `plugins/dev-team/bin/src/mcp.ts` — `registerPhaseNextTool` / `registerPhaseLogTool` 不再转发 `workflow_type`
- `plugins/dev-team/bin/src/commands/phase-log.test.ts` — 基于 `workflow.json` 的 workflow 感知 backtrack 校验测试
- `plugins/dev-team/agents/code-analyze-planner.md` — 新 agent，输出 `design.md`（兼容 test-design-planner 输入格式，不含 tasks.md）
- `plugins/dev-team/agents/code-analyze-evaluator.md` — 新 agent，评估 design.md（不要求 tasks.md）
- `plugins/dev-team/agents/proposal-planner.md` — 新增 Process 步骤：读取 `workflow.json`，若 `workflow_type` 未设置则 AskQuestion 确认并写入
- `plugins/dev-team/skills/workflow-test-only/SKILL.md` — 复制 `workflow-requirement` 结构；Step 1 scaffold 后写入 `workflow.json`（`{"workflow_type": "test-only"}`）
- `plugins/dev-team/skills/workflow-requirement/SKILL.md` — Step 1 scaffold 后写入 `workflow.json`（`{"workflow_type": "requirement"}`）
- `plugins/dev-team/bin/src/commands/phase-next.test.ts` — test-only workflow 的 phase 顺序与 prerequisite 测试（通过 fixture `workflow.json` 设置 workflow_type）
- `plugins/dev-team/bin/src/lib/workflow.test.ts` — test-only 的 `getDependents` 测试
- `plugins/dev-team/.claude-plugin/plugin.json` — 版本号递增

### 不要修改

- `test-design-planner`、`test-design-evaluator`、`test-gen-*`、`unit-test-*`、`integration-test-*` agent 定义（决策树不变；test-only 差异仅在 workflow.ts evaluator prompt）
- `eval-json.ts` 数据结构
- `requirement`、`bug-fix`、`refactor` workflow 的现有阶段表与 prerequisite 表
- `eval.schema.json` phase 枚举（`02-code-analyze` 为已有 phase ID 或无需 schema 变更——若需新增则单独评估）
- 现有 changes 的 eval.json（不迁移）；无 `workflow.json` 的 change 视为 `"requirement"`

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | change 的 `workflow.json` 含 `workflow_type: test-only` 时，`phase_next(change)` 在无 eval 条目时返回 `01-proposal` | `phase-next.test.ts` 单元测试 |
| AC-2 | test-only 六阶段按 prerequisite DAG 顺序推进：`01→02→03→04→06/08` | `phase-next.test.ts` 集成场景 |
| AC-3 | `02-code-analyze` 使用 `code-analyze-planner` + `code-analyze-evaluator`，产出 `design.md` 且不要求 `tasks.md` | agent 定义审查 + evaluator checklist 验证 |
| AC-4 | `03-test-design` 在 `02-code-analyze` pass 后可执行，planner 能读取 design.md | `phase-next.test.ts` gate 场景 |
| AC-5 | `getDependents("01-proposal", "test-only")` 返回 `[02-code-analyze, 03-test-design]` | `workflow.test.ts` |
| AC-6 | `getDependents("02-code-analyze", "test-only")` 返回 `[03-test-design]` | `workflow.test.ts` |
| AC-7 | test-only workflow 全部 leaf phase pass 后 `phase_next` 返回 `done: true`（无 09-acceptance） | `phase-next.test.ts` |
| AC-8 | `/dev-team:workflow-test-only <change>` 可完整执行六阶段循环；scaffold 写入 `workflow.json` | 手动或 E2E 验证 skill 存在且 `workflow.json` 正确 |
| AC-9 | `01-proposal` prompt 引导测试覆盖缺口，与 requirement workflow 的 proposal prompt 有差异 | `workflow.ts` prompt 字符串审查 |
| AC-10 | requirement / bug-fix / refactor workflow 行为不受影响（回归） | 现有 `workflow.test.ts` / `phase-next.test.ts` 全部通过 |
| AC-11 | `workflow.json` 为 `test-only` 时，`phase_log` 拒绝 `backtrack_to: "05-implement"`，不写入 eval.json | `phase-log.test.ts` |
| AC-12 | 拒绝错误信息包含 workflow_type 与可用 phase 列表 | `phase-log.test.ts` |
| AC-13 | `workflow.json` 缺省或不存在时，`phase_next` / `phase_log` 使用 `"requirement"` 阶段表 | `phase-next.test.ts` / `phase-log.test.ts` 回归 |
| AC-14 | `PHASE_TEST_ONLY` 的 `06-unit-test` / `08-integration-test` evaluator prompt 含 WORKFLOW_CONTEXT 自适应指令 | `workflow.ts` prompt 审查 |
| AC-15 | workflow-test-only skill 在测试发现代码 bug 后生成 `reports/` bug 报告并通知用户 | skill 定义审查 + 手动验证 |
| AC-16 | evaluator 自适应流程：`phase_log` 拒绝无效 backtrack 后以 `fail` + `backtrack_to: null` 重试写入 | prompt 指令审查 + 集成场景 |
| AC-17 | `phase_next` 与 `phase_log` MCP input schema 不含 `workflow_type` 字段 | schema 文件审查 |
| AC-18 | `proposal-planner` 在 `workflow.json` 无 `workflow_type` 时向用户确认并写入后再写 proposal | agent 定义审查 + 手动场景 |
| AC-19 | `workflow-test-only` Step 1 scaffold 后 `workflow.json` 含 `{"workflow_type": "test-only"}` | skill 定义审查 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `code-analyze-planner` 产出的 design.md 格式与 `test-design-planner` 期望不兼容 | 高 | 中 | evaluator checklist 强制覆盖架构组件、数据流、变更范围等章节；planner 读取 design.md.template 与 dev-design-planner 输出样例 |
| 逆向分析的 design.md 遗漏边界模块，导致测试设计覆盖不全 | 中 | 中 | proposal.md 明确要求覆盖范围；evaluator 交叉验证 proposal AC 与 design 模块表 |
| 同一 change 在 workflow_type 写入后被手动改为不一致值 | 中 | 低 | `workflow.json` 为唯一数据源；文档说明不要在 workflow 进行中修改 workflow_type |
| 测试执行发现代码 bug 时用户误判为 workflow 失败 | 低 | 中 | engine 层拒绝无效 backtrack；evaluator 以 `fail`+`null` 诚实记录；skill 生成 bug 报告并通知用户 |
| evaluator 未遵循 EXPECTED PATH 仍写入无效 backtrack | 中 | 低 | `phase_log` 引擎层硬拒绝；prompt 明确重试步骤；eval.json 不会被污染 |
| 旧 change 无 `workflow.json` 导致误用 requirement 表 | 中 | 中 | 缺省 `"requirement"` 保持向后兼容；`proposal-planner` 在非 workflow-* 入口主动确认 |
| `02-code-analyze` 新 phase ID 未注册到 eval.schema.json | 低 | 低 | 实现时同步更新 schema 枚举（若在 scope 内） |
