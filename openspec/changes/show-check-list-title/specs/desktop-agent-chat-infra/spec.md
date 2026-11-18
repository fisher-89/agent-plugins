# desktop-agent-chat-infra Specification (Delta)

## MODIFIED Requirements

### Requirement: finish_step 收口卡呈现

`finish_step` 工具部件 SHALL 以注册表专用卡（`BUILD_IN_TOOL_CARDS['finish_step']`）承载，按输入载荷分两形态（互斥，均不抛错）：

- **evaluator 形态（checklist）**：输入同时满足 `report` 为字符串且 `items` 为数组时命中——摘要行 SHALL 呈「检查通过」（零未通过项）或「检查 N 项未通过」（N = 真值为假的项数），含未通过项时摘要行带 `error` 状态；body SHALL 为卡片（`report` 经 `TextBlock` 渲染）+ 逐项清单：每项一个 `Checkbox`（勾选态 = `pass` 真值）+ `FieldLabel`（**项 id + 项标题**）+ `FieldDescription`（`evidence` 非空时）。该形态 MUST NOT 渲染工具结果区（`block-tool-result`）。
- **逐项标签形态**：`FieldLabel` SHALL 呈「项 `id` + 项 `title`」——`title` 为非空字符串时在 `id` 后以单个半角空格接标题原文，且 MUST NOT 截断（不省略号、不改写、不丢字符；折行方式由组件自定）；`title` 缺席 / 空串 / 非字符串时 SHALL 只呈 `id`，MUST NOT 补「（无标题）」类占位符、MUST NOT 因此回退 JSON。未通过项的破坏性着色（`text-destructive`）SHALL 覆盖该标签整体（`id` 与标题同时着色）。标题的展示值取自工具调用载荷原文——其权威性由工具体校验守（见 desktop-change-orchestration「checklist 项标题的搬运与权威校验」），前端 MUST NOT 自行反查 prompt、MUST NOT 内置 `id → 标题` 映射表。
- **decision 形态**：`action` 为字符串时命中——摘要行逐字为 `backtrack → <to>：<reason>` / `stop：<reason>` / `ask：<question>（<options>）`（历史转录的 `ask` 动作原样兼容；未知动作原样）；body SHALL 在有结果时呈 `block-tool-result`。
- **宽容解析**：两形态均不匹配时回退 JSON 原样（`<pre>`）且摘要行取 `finish_step`，MUST NOT 抛错；`pass` / `evidence` / `title` 按真值 / 字符串性宽容判定（非布尔值、非字符串不做类型校验、不抛错）。

#### Scenario: evaluator checklist 逐项可读

- **WHEN** 渲染输入 `{ report, items: [{ id: "C1", title: "变更代码中无安全漏洞", pass: true, evidence }, { id: "C2", title: "无遗留调试输出", pass: false, evidence }] }` 的 `finish_step` 部件并展开卡
- **THEN** 摘要行为「检查 1 项未通过」；report 与两项 `id` / `title` / evidence 可见；C1 的 checkbox 为勾选、其 FieldLabel 文本为 `C1 变更代码中无安全漏洞`；C2 为未勾选、其 FieldLabel 文本为 `C2 无遗留调试输出` 且整行标签（id 与标题）带破坏性着色；零 `block-tool-result` 节点

#### Scenario: 标题缺席降级为 id-only

- **WHEN** 分别渲染三形态输入：某 `items` 元素无 `title`、`title` 为空串、`title` 为 `null` / 数字 / 对象（历史转录与非预期形态）
- **THEN** 三形态下卡仍为 checklist 形态（不回退 JSON）、逐项标签只呈该项 `id`、无占位符与 `undefined` / `null` 文本、渲染不抛错；摘要行与 `evidence` 行呈现与既有行为逐字一致

#### Scenario: decision 形态摘要与结果

- **WHEN** 渲染输入 `{ action: "backtrack", to: "test-gen", reason: "覆盖不足" }` 且工具结果在场
- **THEN** 摘要行为「backtrack → test-gen：覆盖不足」；展开后呈工具结果正文；输入非两形态时回退 JSON 原样且摘要行为 `finish_step`

## Module Contract

### Component: `packages/desktop/src/components/agent/message-cards/blocks.tsx`

| 项 | 变化 | 描述 |
|------|------|----------|
| `FinishItem` | 增 `title` 位 | 私有入参形态为 `{ id: string; title: string \| null; pass: boolean; evidence: string \| null }`；`title` 经 `textOf` 宽容读取（非字符串即 `null`），不新增导出 |
| `CheckListCard` | 逐项标签扩位 | `FieldLabel` 呈 `id`（`title` 在场时追加空格 + 标题原文）；未通过项的 `text-destructive` 覆盖整标签；`Checkbox` / `FieldDescription` / `Field` 结构与 `data-testid`（`block-tool-use` / `data-tool-id` / `block-finish-step-summary` / `block-finish-step-item`）逐字不变；不新增 tooltip / 折叠 / 筛选等交互 |
| `FinishStepCard` | 形态判据不变 | `isCheckListFinish` 仍以「`report` 为字符串且 `items` 为数组」为唯一判据（**MUST NOT** 加「`title` 在场」类前置条件）；decision 形态与 JSON 回退分支零改动 |

> 既有 Module Contract 表中 `message-cards/blocks.tsx` 一行的描述按本条扩充：evaluator 形态呈「`Checkbox` + `Field` 逐项清单（标签含 `id` 与标题；摘要「检查通过 / 检查 N 项未通过」，report 经 `TextBlock`），checklist 形态不呈工具结果区」；其余措辞不变。

### 零改动面

| 模块 | 说明 |
|------|------|
| `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` | **零改动**：`BUILD_IN_TOOL_CARDS['finish_step']` 注册项与键面不变 |
| `packages/desktop/src/lib/agent-adapter.ts` | **零改动**：`finish_step` 仍是普通工具部件（`tool-finish_step`），配对与输出载荷形状不变 |
| `packages/desktop/src/views/changes/renderers/**` | **零改动**：`eval-checklist` 抽屉仍呈落账面 `item` / `pass` / `evidence`，不补 `id`、不读 `title` |
| `packages/desktop/src/types/generated/**` | **零改动**：本变更不出线新 IPC 类型，`bindings:export` 再生成后零 diff |
