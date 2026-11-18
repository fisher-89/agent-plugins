# bash 工具新增「调用描述」参数

workflow_type: requirement

需求原文：bash 工具增加参数「描述」，必须传入简短字符（一行可显示）说明 bash 的行为或目的，并在工具卡片**未展开时**即可见。

## 现状链路（从模型到卡片）

```
mount.rs generic_definition("bash")          模型调用 {"command": "...", "timeout_ms"?}
  description: "执行 shell 命令并返回合并输出…"
  parameters.properties: command / timeout_ms
  required: ["command"]
        │
        ▼
tools::execute ──► bash::execute(root, input)      只读 command / timeout_ms
        │            （command 缺失或空白 → 硬 Err；timeout 越界 → 硬 Err）
        ▼
core/agent event.rs:65  ToolUseBlock { input: Value }   input 原样 JSON 落库
        │
        ▼
前端 agent-adapter ──► BUILD_IN_TOOL_CARDS["bash"] = BashCard
                        折叠态（trigger）：<TerminalIcon/> + TraceTitle>Bash</TraceTitle>   ← 只有两个字
                        展开态：CodeBlockHeader{command} + 输出
```

对照既有卡：`read` / `write` / `edit` 折叠态标题都拼了参数（`Read src/a.ts:10-15`、`Write app.tsx`），`web_search` 折叠态标题干脆就是查询词本体（blocks.tsx:225-275）。**bash 是唯一折叠态零信息的卡**——命令常常长到无法当摘要读（多段 `&&`、管道、heredoc），所以正确的做法不是把 command 提上来，而是让模型给一行意图摘要。

## 触点清单

| 面 | 位置 | 变更性质 |
| --- | --- | --- |
| 工具定义（唯一真源） | `crates/infra/agent/src/sdk/mount.rs:473-487` | properties 增字段、required 增项、工具 description 加引导 |
| 执行体入参闸 | `crates/infra/agent/src/sdk/bash.rs:16-26` | 取决于「必须」的强度（见取舍 A） |
| 前端卡片折叠态 | `packages/desktop/src/components/agent/message-cards/tool-cards.tsx:288-322` | BashCard trigger 排版 + 历史回落 |
| 规格 | `openspec/specs/desktop-agent-execution/spec.md`（「SDK 引擎 bash 工具」477-505）、`openspec/specs/desktop-agent-chat-infra/spec.md`（工具卡注册表 398） | 需求条同步 |
| 测试 | `mount_test.rs`（定义面）、`tools_test.rs`（9 处 bash 夹具）、`bash_test.rs`（入参闸）、`tool-cards.test.tsx`（bash 三组用例 + `getByText('Bash')`） | 夹具随 schema 硬校验扩散 |
| 版本 | `packages/desktop/package.json` | patch bump |

## 不受影响（已核实）

- `protect/mod.rs:91` 只读 `input["command"]`，写保护判定与描述字段无关；
- `tools.rs::input_path` 对 bash 恒返回 `None`，无路径改写面；
- `context.rs` / `compact.rs` 不裁剪 tool input（描述随 input 进历史，每轮少量 token）；
- 16 份 phase role 提示词只提「bash 只用于…」，不含 bash 入参样例，无需同步；
- `.dev-team/architecture` 里 `App.Backend.Infra.AgentRuntime` 为 `gate = "off"` 且无契约文件，api-check 不拦。

## 关键取舍

**A. 「必须」的强度。** schema `required` 对多数模型只是提示，不填也不会失败；要真「必须」只能执行体硬 Err（仓库既有先例：command 空串硬 Err、timeout 越界硬 Err、反静默钳位）。代价是一轮白费 + 模型重试。

**B. 「一行可显示」的约束形式。** 三种叠加度：schema `maxLength`（软）/ 前端 CSS `truncate`（兜底）/ 执行体超长 Err（硬）。TraceTitle 已自带 `truncate`；TraceDescription（`shrink-0`、未 truncate、目前全仓无使用先例）可作次要信息位但需补 `min-w-0 truncate`。

**C. 折叠态排版 + 历史回落。** 历史会话的 input 里没有描述（event.rs 存原 JSON），这些卡必须回落——回落成 `Bash`（现状）还是命令首行（改观更大的选项）。

**D. 字段命名。** `description` 与 JSON Schema 每个 property 自带的 `description` 键同名，写 schema 时字面重复（`"description": { "type": "string", "description": "…" }`）；`intent` / `summary` / `purpose` 可避开歧义，代价是与用户原话「描述」不同名。

## 已定决策

1. **强制力度 = 硬校验**：`intent` 缺失 / 非字符串 / 空白 → `bash::execute` 直接 Err（与 `command` 闸同形、同一处、command 先报以保既有错误语义）。
2. **字段名 = `intent`**：避开 JSON Schema 每个 property 自带 `description` 键的字面撞车。
3. **折叠态排版 = `intent` 与 `Bash` 共享 `TraceTitle`**：同一元素内 `Bash · <intent>`，省略由 `TraceTitle` 自带的 `min-w-0 truncate` 自动收尾（**不用** `TraceDescription` 次级位——那条路要补 `shrink-0` 覆盖并实机调试截断，且两元素各占一条省略链）。
4. **历史回落 = 省略**：`intent` 缺席时标题回落为 `Bash`，老转录与无 intent 新卡同形。
5. **长度 = 软约束，仅提示推荐长度**：执行体**不因长度 Err**（字数是审美约束，模型难以稳定命中，硬 Err 会显著抬高重试率），schema **不加 `maxLength`**（避免「写 60 没人在意」的假精确），只在字段 description 里写「建议 ≤30 字」；前端 `truncate` 兜底。若日后出现稳定的超长样本，再考虑升为硬闸。

## 实现草图

`mount.rs` bash 定义：

```jsonc
"properties": {
  "command":    { "type": "string", "description": "待执行的命令行（…）" },
  "intent":     { "type": "string",
                  "description": "一句话说明这条命令要干什么——折叠状态下用户只读这一行（与 Bash 同排展示，超长自动省略）。建议不超过 30 字，写目的而非复述命令文本" },
  "timeout_ms": { "type": "integer", "description": "超时毫秒（…）" }
},
"required": ["command", "intent"]
```

`bash.rs` 入参闸（紧跟 command 校验之后）：

```rust
let intent = input.get("intent").and_then(Value::as_str)
    .ok_or_else(|| "入参缺失或非字符串字段: intent".to_owned())?;
if intent.trim().is_empty() {
    return Err("intent 不得为空".to_owned());
}
```

`tool-cards.tsx` BashCard：

```tsx
const intent = textOf(asRecord(input).intent);   // undefined / '' → null
<TraceLine …>
  <TraceIcon>{running ? <Spinner/> : <TerminalIcon/>}</TraceIcon>
  <TraceTitle>{intent === null ? 'Bash' : `Bash · ${intent}`}</TraceTitle>
</TraceLine>
```

`TraceTitle` 自带 `min-w-0 truncate`，单文本节点即为一条省略链，无需附加样式；`TraceLine` 是 `w-fit max-w-full`，长 intent 收到容器宽度后由标题自身出省略号。running 态的 `shimmer` 挂在 `[data-slot=trace-title]` 上，intent 随整行一起流动（正是「这一行在跑」的语义）。

**可选强化**：若想在视觉上区分工具名与意图（工具名正常色、意图略淡），可写成

```tsx
<TraceTitle>Bash{intent !== null && <span className="text-muted-foreground/70"> · {intent}</span>}</TraceTitle>
```

外层截断照旧生效；附带收益是内层 `'Bash '` 归一化后仍满足既有 `getByText('Bash')` 精确断言（见下节）。

## 测试扩散面（硬校验的真实成本）

走 `execute` 的夹具必须补 `intent`：

- `sdk/tools_test.rs`（8 处 bash 调用夹具）
- `sdk/loop_test.rs`（10 处，含写保护三连用例的真实执行路径）
- `sdk/runner_test.rs`（4）、`sdk/resume_test.rs`（3，重放史不执行则不受影响，需逐条判）

不走 `execute`（纯保护判定 / 档位表，不受影响）：

- `sdk/protect/mod_test.rs`（16）、`sdk/protect/reason_test.rs`（9）、`sdk/policy_test.rs`（3）、`sdk/mount_test.rs`（5，仅定义面断言，加 required 项后可断言新成员）
- `sdk/archi_tool_test.rs`、`compose_test.rs`、`core/agent/src/runner_test.rs`（工具名清单断言，零改动）

前端：`tool-cards.test.tsx:459` 的 `within(cardRoot()).getByText('Bash')` 是**精确文本断言**——`intent` 拼进同一 `TraceTitle` 后该元素 `textContent` 变为 `Bash · …`，断言失效。两条路：① 改成 `/^Bash/` 正则（最省事）；② 采纳上面的「可选强化」写法，工具名留在独立内层节点，断言不动。三组 bash 用例（output 三态 / 命令栏异常载荷）本身不受影响——`intent` 不参与 `CodeBlockHeader` 与状态判据。

## 未纳入本次范围

`read` / `write` / `edit` 折叠态已有路径可读；`db_query` / `archi_*` 的折叠态摘要若也要意图化，属同一族但独立需求。
