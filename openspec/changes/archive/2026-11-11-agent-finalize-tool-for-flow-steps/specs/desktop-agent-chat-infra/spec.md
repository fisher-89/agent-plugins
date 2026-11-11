# desktop-agent-chat-infra Specification (Delta)

## ADDED Requirements

### Requirement: finish_step 工具卡呈现

前端 SHALL 把流程 Agent 的 `finish_step` 收口调用呈现为**特化工具卡**（不新增部件类型、不改事件适配层），并保持未注册工具回退既有行为：

- **承载面**：`finish_step` SHALL 复用既有工具卡管线（`planBlock` → `planToolPart` → `plansToChunks` → `BUILD_IN_TOOL_CARDS` 注册表四方现有机械路径），MUST NOT 新增 `AgentDataParts` 成员、MUST NOT 改 `agent-adapter.ts` / `agent-transport.ts`（`finish_step` 与其余工具调用同为 `tool-*` 部件）。
- **注册**：`BUILD_IN_TOOL_CARDS` SHALL 增 `finish_step` 键，值为 `FinishStepCard`；`Object.keys` 断言随注册表键面更新。
- **evaluator 形态摘要**：当 `input.items` 为数组时，卡 SHALL 呈摘要——① `items` 逐项一行（ID + pass 标记 + `evidence`，`evidence` 过长按行截断且不抛错）；② `report` 文本（可折叠）；③ `pass` 全真 / 含假时以文字区分（如「清单全通过」/「含未通过项」）。MUST NOT 由前端重算 verdict 并写入任何持久面（verdict 由后端按 checklist 推导，前端只做展示推导）。
- **decision 形态摘要**：当 `input.action` 为字符串时，卡 SHALL 呈 `action` 摘要行——`backtrack` 呈目标相位（`input.to`）与 `reason`；`stop` 呈 `reason`；`ask` 呈 `question` 与选项清单；未知 action 呈原文。
- **宽容解析**：`input` 非对象 / 缺上述字段时 MUST NOT 抛错——回退为 JSON 原样展示（沿 `AskUserQuestionCard` 的宽容解析先例）。
- **钩子与回退**：卡根 SHALL 沿用卡根钩子（`data-testid="block-tool-use"` + `data-tool-id`）与结果正文钩子（`data-testid="block-tool-result"`），摘要区 SHALL 带 `data-testid="block-finish-step-summary"`，使消费方无需区分卡型即可定位；未注册工具名 SHALL 仍回退默认对卡（`ToolPairCard`），该分支语义零改动。

#### Scenario: evaluator 形态摘要

- **WHEN** 渲染一条 `finish_step` 工具部件，`input` 为 `{ report: "...", items: [{ id: "C1", pass: false, evidence: "src/x.rs:42 硬编码密钥" }, { id: "C2", pass: true, evidence: "…" }] }`
- **THEN** 卡内呈现两行逐项（`C1` 标为未通过、`C2` 标为通过）与 `report` 文本；无 `block-tool-result` 之外的额外持久面写入，页面上无原始 JSON 大块

#### Scenario: decision 形态摘要

- **WHEN** 渲染 `finish_step` 部件且 `input` 为 `{ action: "backtrack", to: "test-gen", reason: "测试覆盖不足" }`
- **THEN** 卡内摘要行呈 `backtrack`、目标 `test-gen` 与 reason 文本；不展开逐项清单区

#### Scenario: 宽容回退

- **WHEN** 渲染 `finish_step` 部件且 `input` 为字符串或缺失 `items` / `action` 字段
- **THEN** 卡片不抛错，回退为 JSON 原样展示；卡根 `data-testid="block-tool-use"` 与 `data-tool-id` 照常在位

#### Scenario: 注册表与回退共存

- **WHEN** 审查 `BUILD_IN_TOOL_CARDS` 键面
- **THEN** 含 `finish_step`、`AskUserQuestion`、`web_search`、`bash`、`read`、`write`、`edit`；未注册工具名（如 `grep`）仍由 `ToolPairCard` 呈现（行为零改动）

## Module Contract

### Component: `packages/desktop/src/components/agent/agent-blocks.tsx`

| 组件 | Props | 描述 |
|------|-------|------|
| `FinishStepCard` | `toolCallId: string`；`toolName: string`；`input: unknown`；`output: AgentToolOutput \| null` | `finish_step` 收口卡：evaluator 形态（`items` 逐项 + `report`）与 decision 形态（`action` 摘要）双分支；非预期 `input` 回退 JSON 原样；卡根与结果正文钩子沿用既有卡根约定 |

### Function: `packages/desktop/src/components/agent/tool-cards.tsx`

| 项 | 变化 | 描述 |
|------|------|------|
| `BUILD_IN_TOOL_CARDS` | 增 `finish_step` 键 | 值为 `FinishStepCard`；其余键与回退分支零改动 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `packages/desktop/src/lib/agent-adapter.ts` | **零改动**：`finish_step` 是普通工具调用（`tool-finish_step` 部件），既有 `planBlock` / `planToolPart` 天然覆盖 |
| `packages/desktop/src/lib/agent-transport.ts` | **零改动**：无新事件类型需转 chunk |
| `packages/desktop/src/components/agent/agent-messages.tsx` | **零改动**：工具部件 → 特化卡的分派逻辑（注册表命中 / miss 回退）已存在 |
| `packages/desktop/src/types/generated/**` | **零改动**：`SessionInjections` / `FinishSpec` 不出线 IPC，`bindings:export` 再生成后零 diff |
