# desktop-agent-chat-infra Specification (Delta)

## ADDED Requirements

### Requirement: bash 工具卡折叠态意图行

`BashCard`（`packages/desktop/src/components/agent/message-cards/tool-cards.tsx`，经 `BUILD_IN_TOOL_CARDS['bash']` 注册）折叠态（未展开的 `TraceCollapsibleTrigger`）SHALL 让用户无需展开即可读到「这条命令要干什么」：

- **折叠态标题**：工具调用输入携非空白 `intent` 时，折叠态标题 SHALL 呈 `Bash · <intent>`——工具名 `Bash` 在前，` · `（空格 + 间隔号 + 空格）分隔，接 `intent` 的 `trim` 后原文（MUST NOT 改写、MUST NOT 折叠内部空白、MUST NOT 追加命令文本或其它推导内容）。
- **历史回落**：`intent` 缺席 / 非字符串 / `trim` 后为空串（旧转录、非 bash 工具形态、异常载荷）时，折叠态标题 SHALL 逐字回落为 `Bash`——MUST NOT 回填 `command` 首行、MUST NOT 补「（无描述）」类占位符、MUST NOT 渲染 `undefined` / `null` / 悬空分隔符、MUST NOT 因此抛错或回退默认对卡。
- **单条省略链**：工具名与意图 SHALL 同处 `data-slot="trace-title"` 的单一文本承载（省略由 `TraceTitle` 内建的 `min-w-0 truncate` 收尾），MUST NOT 另起 `trace-description` 次级位或第二个省略节点（两条省略链会让省略点分裂、视觉上出现双截断）；超长 `intent` SHALL 由 CSS 省略，MUST NOT 在组件内 JS 侧截断 / 改写字数、MUST NOT 引入 tooltip / 换行 / 多行布局。
- **不变量（逐字保留）**：展开态 `code-block-header` SHALL 仍为 `command` 原文、`block-tool-result` 区 SHALL 仍为 `output.content`；`TraceLine status` 三态判据（`output.isError` → `error` / 未配对且轮运行中 → `running` / 否则省略）与 Spinner ↔ TerminalIcon 互斥判据、卡根钩子（`data-testid="block-tool-use"` + `data-tool-id`）、`BUILD_IN_TOOL_CARDS` 键面与声明序 MUST NOT 变化。
- **转录自足**：折叠态标题 SHALL 只由工具调用载荷驱动（`intent` 随 input 落库、重放同形），MUST NOT 新增查询面 / 落库列 / 前端常量映射，MUST NOT 从 `output` 或轮记录反推意图。

#### Scenario: 折叠态呈工具名与意图

- **WHEN** 渲染 input 为 `{ command: "pnpm -C packages/desktop exec vp test run", intent: "跑单测" }` 的 `bash` 工具卡且不展开
- **THEN** `data-slot="trace-title"` 的文本为 `Bash · 跑单测`；折叠态文本不含命令原文；展开后命令栏仍为原命令、结果区仍为输出内容

#### Scenario: 意图缺席回落为工具名

- **WHEN** 分别渲染 `intent` 缺席、`intent` 为空串、`intent` 为非字符串（`null` / 数字 / 对象）三种 input（历史转录与异常载荷）
- **THEN** 三种形态折叠态标题逐字为 `Bash`，无 `·` 残留、无占位符与 `undefined` / `null` 文本、渲染不抛错，卡仍为 bash 特化卡（不回退默认对卡）

#### Scenario: 超长意图单行省略

- **WHEN** 渲染 `intent` 为 200 字长串的 `bash` 工具卡
- **THEN** 文本节点内保留 `intent` 原文（组件侧零截断改写），`data-slot="trace-title"` 为唯一文本承载（零 `trace-description` 节点），省略由 CSS `truncate` 呈现且不换行、不撑破卡容器、不新增 tooltip / `title` 属性

#### Scenario: 状态与展开态零回归

- **WHEN** 对同一 bash 卡分别渲染 `output` 为 `null`（运行中）、正常结果、`isError` 结果三态并展开
- **THEN** `data-status` 与 Spinner / TerminalIcon 互斥判据逐条不变，展开后 `code-block-header` 为命令原文、`block-tool-result` 为结果正文；`BUILD_IN_TOOL_CARDS` 键面与顺序、卡根钩子零变化

## Module Contract

### Component: `packages/desktop/src/components/agent/message-cards/tool-cards.tsx`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `BashCard` | 折叠态标题扩位 | 新增 `intent` 读取（`textOf(asRecord(input).intent)`，`trim()` 后为空按缺席归一）；标题文本按「`Bash · <intent>` / 回落 `Bash`」两态；`TraceLine` / `TraceIcon` / `TraceTitle` / `TraceCollapsible*` 结构与既有 `status` 判据、`CodeBlock` 展开态逐字不动；不新增导出、不新增 prop |
| `BUILD_IN_TOOL_CARDS` | 零改动 | 键面与声明序不变（`bash` 注册项仍指同一 `BashCard`） |
| 消费的展示件 | 零改动 | 复用 `components/ui/trace.tsx` 的 `TraceTitle` 内建 `min-w-0 truncate` 作唯一省略链；MUST NOT 新增 `trace.tsx` 变体 / prop / 类名分支，MUST NOT 启用 `TraceDescription` |

### 零改动面

| 模块 | 说明 |
|------|------|
| `packages/desktop/src/components/ui/trace.tsx` | `TraceLine` / `TraceIcon` / `TraceTitle` / `TraceDescription` / `TraceCollapsible*` 零改动（本变更只消费既有省略语义） |
| `packages/desktop/src/components/agent/message-cards/blocks.tsx` | `ReadCard` / `WriteCard` / `EditCard` / `WebSearchCard` / `AskUserQuestionCard` / `FinishStepCard` 零改动 |
| `packages/desktop/src/lib/{agent-adapter,parse}.ts` | 工具部件管线与 `asRecord` / `textOf` 语义零改动（`intent` 走既有 input 透传） |
| `packages/desktop/src/types/generated/**` | 零改动：`ToolUseBlock.input` 仍为自由 JSON，本变更不出线新 IPC 类型，`bindings:export` 再生成后零 diff |
| `packages/desktop/src/views/**` | 消费点接线零改动（折叠态由工具卡注册表统一承载） |

> 既有 Module Contract 表中 `message-cards/tool-cards.tsx` 一行的描述按本条扩充：`bash` 特化卡的折叠态标题呈 `Bash · <intent>`（`intent` 缺席回落 `Bash`），展开态命令栏与结果区、注册表键面与卡根钩子不变；其余措辞不变。
