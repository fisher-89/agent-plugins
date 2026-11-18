# 设计: bash-tool-description-param

> **变更**: bash-tool-description-param
> **状态**: 设计定稿（基于已定稿 proposal + 2 份 delta spec）
> **基于**: `proposal.md`（问题 / D1–D7 / 变更范围 / AC-1…AC-10 / R1–R8）、`explore.md`（需求原文与实现草图）与 `specs/{desktop-agent-execution,desktop-agent-chat-infra}/spec.md`
> **性质**: 「每次 bash 调用自带一行意图摘要、折叠态即可见」由三处拼成——`mount.rs` 定义面（`intent` 进 `required`）+ `bash.rs` 执行体硬闸 + `tool-cards.tsx` 折叠态标题。改动面 = **3 处源码 + 1 行版本号**。
>
> **本篇边界**: 收敛 proposal 两个未决项（折叠态是否分色 / 两处引导措辞如何分工）并给**逐字文案台账**；钉死意图闸的插入点与两条 `Err` 文案；逐条 AC 给落点。**零新增文件、零新增 crate、零新增依赖边、零 store 改动、零 IPC 出线变更、零架构模型改动**（`App.Backend.Infra.AgentRuntime` 的 `api.status = none` 且 `gate = off`、`App.Frontend.Components.Agent.ToolCards` 的 `gate = off`，两处均无契约文件）。

---

## 提案与规格同步状态

proposal 的 D1–D7 七条裁决与 2 份 delta spec 均已定稿，本设计不重复其论证，只做三件事：

1. **把 spec 已钉语义、未钉形状的四处收敛到具体形态**（见「关键设计决策」DD1–DD7）：折叠态**是否分色**（proposal 未决项 1）、`intent` 的**读取与归一化写法**、两处引导措辞的**分工与逐字文案**（未决项 2）、执行体闸的**插入点与两条 `Err` 文案**。
2. **核实「折叠态靠 input 自足、重放同形」的机制依据**（见「起点核实」）：`intent` 随 `AgentBlock::ToolUse.input`（`serde_json::Value`）原样进转录与回放，store 对该列是不透明 JSON，故零 schema 认知、零迁移、零回填。
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略章节（用例编写与门禁执行由独立相位承接）。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **定义面 `crates/infra/agent/src/sdk/mount.rs`**：`TOOL_NAMES`（`:20`）为 `["read","grep","glob","ls","write","edit","bash"]` 七件封闭清单；`definitions(...)`（`:166-171`）按注入面取清单、`None` 分支 `take(TOOL_NAMES.len())` 即七工具全量；`generic_definition(name)`（`:387`，**模块私有**）逐名 `match`；`"bash"` 臂 `:473-487`——`description` 为三段（执行与合并口径 / cwd 与非零退出码 / 超时活性护栏），`properties` 仅 `command`（`type: "string"`）与 `timeout_ms`（`type: "integer"`），`required: ["command"]`。
- **执行体 `crates/infra/agent/src/sdk/bash.rs`**：`execute(root, input)` `:16`——`command` 取读 + 非字符串拒（`:17-21`）、`command.trim().is_empty()` 拒（`:22-24`）、`resolve_timeout_ms(input)?` 解析超时（`:26`）；`resolve_timeout_ms` `:81`、`build_command` `:100`、`drain` `:110`、`merge_output` `:117`；超时罩全段并以 `exec::kill_tree(pid)` 收口。
- **分派面 `crates/infra/agent/src/sdk/tools.rs`**：`execute(root, name, input)` `:64`——`"bash" => bash::execute(root, input).await`（`:74`），input **原样透传**（零字段感知），出口统一 L1 截断收口（`:80-85`）。
- **守卫链 `crates/infra/agent/src/sdk/loop.rs`**：成员校验（`:631-641`，`tool_unavailable`）→ 档位准入（`:641-645`，`permission_denied`）→ 路径沙箱（`:647-652`，`sandbox_denied`）→ **写保护闸**（`:658-665`，`turn.protect.evaluate(&name, &sandboxed, &turn.cwd)`）→ 执行臂路由（`:668` 起）。**写保护拒绝臂在执行体之前 `continue`**，故这些夹具从不触 `bash::execute`——即 R5 判据「拒绝臂可不补 `intent`」的代码依据。
- **转录块 `crates/core/agent/src/event.rs`**：`AgentBlock::ToolUse { id: String, name: String, input: serde_json::Value }`（`:62-67`），input 为「结构由具体工具自解释」的原 JSON——`intent` 因此零新列、零编码改动。
- **前端卡 `packages/desktop/src/components/agent/message-cards/tool-cards.tsx`**：`BashCard` `:285-322`——`const command = textOf(asRecord(input).command)`（`:291`）；`TraceLine status` 三态（`output.isError` → `error` / `unpairedRunning(output, turnRunning)` → `running` / 否则省略）；`TraceIcon` 内 Spinner ↔ TerminalIcon 按 `unpairedRunning` 互斥；`TraceTitle>Bash`（`:309`，**唯一折叠态信息**）；展开态 `CodeBlockHeader>{command}` + `CodeBlockContent data-testid="block-tool-result">{output?.content}`；卡根钩子由 `cardAttrs(toolCallId)`（`:277-284`）给 `data-testid="block-tool-use"` + `data-tool-id`。
- **展示件 `packages/desktop/src/components/ui/trace.tsx`**：`TraceTitle` `:58-66` 类为 `min-w-0 truncate motion-reduce:shimmer-none`（`truncate` = `overflow-hidden` + `text-ellipsis` + `whitespace-nowrap`，省略点在**行尾**，故前缀 `Bash` 永不被吃掉）；`TraceLine` `:24-42` 整行 `text-sm text-muted-foreground`，且 `data-[status=running]:*:data-[slot=trace-title]:shimmer`（意图随整行一起流光，正是「这一行在跑」的语义）；`TraceDescription` `:68-80` 为 `shrink-0` 且无 `truncate`，**全仓零消费点**（只在本文件定义与导出）。
- **既有卡无分色先例**：`ReadCard` `:358`、`WriteCard` / `EditCard`、`WebSearchCard`（`blocks.tsx:262`）、`FinishStepCard`（`blocks.tsx:358` / `:420`）的标题全是 `TraceTitle` 内的扁平文本，没有任何一张卡为「工具名 vs 参数」开内层着色节点。
- **既有前端精确断言**：`tool-cards.test.tsx:459` 的 `within(cardRoot()).getByText('Bash')`，其夹具为 `{ command: 'ls -la' }`（**不带 `intent`**，`:453`）。
- **既有定义面断言（决定措辞落位）**：`tools_test.rs:1281` 的快照行 `("bash", &["command"], "执行 shell 命令并返回合并输出")`，`:1290` 以 `starts_with` 锚描述前缀——**新增句只能追加在既有描述末尾**，插到前面即让既有断言红；同文件 `:102-119` 已断言 `command` 必填、`timeout_ms` 不入 `required`、`properties.command.type == "string"`。
- **既有执行夹具面**：`bash_test.rs` 内 10 处 `execute(...)` 调用（`:29` / `:42` / `:70` / `:90` / `:107` / `:118` / `:126` / `:150` / `:276` / `:305`）；`tools_test.rs` 3 处（`:973` 60KB 超限用例、`:998` 分发正向、`:1003` 缺参用例）；`loop_test.rs` 走真实执行的 bash 调用 5 处（`:2739` 六工具直驱 bash 臂、`:2942` 命令分发、`:2995` 写保护**放行臂**、`:3033` 放行、`:5651` 历史配对末条），拒绝臂（`:2974`、`:3345`、`:3350`）先于执行体 `continue`。
- **前端读取件 `packages/desktop/src/lib/parse.ts`**：`asRecord` `:8-20`（数组 / 标量归空对象）、`textOf` `:21-23`（非字符串 → `null`，空串原样返回 `''`）——DD2 的宽容读取口径即这两个既有件的组合。
- **版本**：`packages/desktop/package.json:3` 现为 `0.6.52`；`src-tauri/tauri.conf.json:4` 为 `"version": "../package.json"`（跟随）；`src-tauri/Cargo.toml:6` 的 `version = "0.1.0"` 为 crate 自身版本行（**不随动**）。

---

## 关键设计决策

> proposal 的 D1–D7（意图经工具输入承载 / 执行体硬闸 / `command` 先校验 / 字段名 `intent` / 长度软约束 / 折叠态共享 `TraceTitle` / 历史回落省略）已定稿，本篇不重复。下表 DD1–DD7 收敛 proposal 两个未决项与三处待钉形状。

| # | 议题 | 定稿 | 理由（为什么另一种写法会坏） |
|---|---|---|---|
| DD1 | 折叠态**是否分色**（proposal 未决项 1） | **单文本节点**：`<TraceTitle>{intent === '' ? 'Bash' : \`Bash · ${intent}\`}</TraceTitle>`——不引入内层 `span`、不引入 `text-muted-foreground/70` | ① 既有六卡的 `TraceTitle` 全是扁平文本（见「起点核实」），为一张卡开「工具名正常色 + 意图略淡」的分色节点是新视觉约定，spec 与 AC 均未要求（AC-5 / AC-7 只锚 `trace-title` 的文本与单条省略链）；② `TraceLine` 整行已是 `text-muted-foreground`，分色的实际增量只是一个更淡的色阶，却带来「同族卡样式两套」的读码成本（AGENT.md「沿既有约定，不自造新约定」）；③ 未决项 1 给内层节点方案的**唯一收益**（既有 `getByText('Bash')` 断言零改）在本盘面**本来就不需要兑换**：该断言夹具不带 `intent`（`:453`），走的是回落路、标题逐字 `Bash`，单文本节点下断言同样零改即绿（`getByText` 按元素**自有文本节点**匹配，回落路只有 `Bash` 一个文本节点）；凡新夹具带 `intent`，按 AC-5 锚 `trace-title` 的 `textContent` 全串（等价 `/^Bash/` 正则），不锁 DOM 层级 |
| DD2 | `intent` 的**读取与归一化** | 卡内一行常量：`const intent = textOf(record.intent)?.trim() ?? '';`——`''` 作**缺席哨兵**；标题表达式单点（两态一处三目）。只 `trim()` **首尾**，MUST NOT 折叠内部空白、MUST NOT 截断、MUST NOT 改写 | ① `textOf` 已把非字符串（数字 / 对象 / `null` / 布尔）归 `null`（`parse.ts:21`），`?.trim()` 再吃掉纯空白串，于是「缺席 / 非字符串 / 空串 / 纯空白」四形态统一落 `''` 一个哨兵——与 `ReadCard` 用 `textOf` 宽容读未校验字段的既有惯例同形（不自造 `pickIntent` 类 helper：单处使用，AGENT.md「单次使用不为抽象」）；② 不折叠内部空白是 spec 明文；模型给的多行 `intent` 由 `truncate` 的 `whitespace-nowrap` 折成一行显示（CSS 面收口），JS 侧动手即等于改写原文，且会让「文本节点内保留 `intent` 原文」这条 AC 判据失效；③ 哨兵用 `''` 而非 `null`：三目只需一次比较，`null` 需两处判空，且 `?? ''` 与可选链在同一行读完 |
| DD3 | 两处引导措辞的**分工**（proposal 未决项 2） | 四个**取值要素**（一句话说明这条命令要干什么 / 折叠态用户只读这一行 / 建议不超过 30 字 / 写目的而非复述命令文本）**全部**落在 `intent` 的字段 `description`；工具级 `description` 只追加一句**强制与后果**（「每次调用必须给 intent（缺失或空白即拒绝执行）」），且**追加在既有描述末尾**（逐字文案见下表「文案台账」） | ① 分工单点：字段 `description` 答「怎么写」、工具级答「不给会怎样」，两处**不共享任何一句措辞**——AC-1 的「合力」= 两处相加覆盖四个要素，而非各写一半造成两句都写不清；② 追加到末尾是硬约束：`tools_test.rs:1290` 以 `starts_with` 锚 bash 描述前缀，插到前面直接让既有断言红（AC-4「既有断言零改」的一部分）；③ 不写进 `bash.rs` 的 `Err` 文案（那里只陈述缺失事实——R1 的「模型一次即纠」靠文案同构，不靠重复纪律），也不写进 phase 提示词（proposal 明列不改 16 份 `prompts/*.md` 与 `AGENT.md`） |
| DD4 | 意图闸的**插入点与文案** | 插入点：`command.trim().is_empty()` 判空**之后**、`let timeout = Duration::from_millis(resolve_timeout_ms(input)?)` **之前**；形态 `let intent = input.get("intent").and_then(Value::as_str).ok_or_else(\|\| "入参缺失或非字符串字段: intent".to_owned())?; if intent.trim().is_empty() { return Err("intent 不得为空".to_owned()); }` | ① 位点前界：`command` 先报是 AC-2 的显式判据（`{}` 的报错必须含 `command`）；位点后界：若晚于 `resolve_timeout_ms`，「超时越界 + 缺 `intent`」样本会先报超时，既有超时类断言文案漂移（R8）；② 两条文案与既有 `command` 闸**逐字同构**（`入参缺失或非字符串字段: <名>` / `<名> 不得为空`），模型一次即纠，且既有断言对 `command` 的字符串面零漂移；③ `as_str` 对数字 / 对象 / `null` 一致返回 `None`，正是 AC-2 的四类反向样本，无需手写类型分支；④ 不新增长度分支（D5：长度只软提示）；⑤ `intent` 只在闸内被 `trim` 一次，不进子进程命令行、不进 ToolResult、不进日志（spec「消费边界」） |
| DD5 | 前端**零导出 / 零新件** | `BashCard` 内改动仅三处：`const record = asRecord(input);`（让 `command` 与 `intent` 同源读一份）、`intent` 常量、`TraceTitle` 表达式；**不新增导出、不新增 prop、不改 `AgentToolCardProps`** | 避免 knip 死代码面（AGENT.md「no test-only exports」）；`BashCard` 保持模块私有（既有形态），`BUILD_IN_TOOL_CARDS['bash']` 的同引用关系不变 |
| DD6 | 「`intent` 不参与任何判定」的**边界复核点** | `protect/**`、`sandbox.rs`、`tools.rs::input_path`、`context.rs` / `compact.rs`、`core/agent/**`、`infra/store/**` 的 `git diff` 期望为**空**；写保护闸在 `loop.rs:658` 先于执行臂路由（`:668`），`intent` 即使在 input 里也不被读 | 这是 AC-10 与 R5 的判据来源，直接写进 tasks 自查项（出现 diff 即说明实现把意图闸错放进保护面） |
| DD7 | **版本行**与生成物 | `packages/desktop/package.json` 的 `version`：落地时 `HEAD` 现值 **+1 patch**（当前盘面 `0.6.52` → 期望 `0.6.53`，以 `git diff HEAD -- packages/desktop/package.json` 的版本行为「恰好 +1」，**MUST NOT 写死字面号**）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随（零改动）；`src-tauri/Cargo.toml` crate 版本行**不随动**；`bindings:export` **零 diff**（`ToolUseBlock.input` 仍是自由 JSON，本变更不出线新 IPC 类型） | 版本纪律由 AGENT.md 钉；写死字面号会在并发变更下撞号 |

### 文案台账（逐字，实现相位照抄）

| 位置 | 形态 | 逐字内容 |
|---|---|---|
| `mount.rs` `generic_definition("bash")` → `properties.intent` | property 对象 | `{ "type": "string", "description": "一句话说明这条命令要干什么——折叠状态下用户只读这一行（与 Bash 同排展示，超长自动省略）。建议不超过 30 字，写目的而非复述命令文本" }`（**无 `maxLength`**；位置在 `command` 之后、`timeout_ms` 之前） |
| 同上 → `required` | 数组 | `["command", "intent"]`（`timeout_ms` 仍不入） |
| 同上 → 工具级 `description` | 追加句（**末尾**） | 既有串不动，尾部追加 `；每次调用必须给 intent（缺失或空白即拒绝执行）` |
| `bash.rs::execute` → 缺失 / 非字符串 | `Err(String)` | `入参缺失或非字符串字段: intent` |
| `bash.rs::execute` → `trim` 后为空 | `Err(String)` | `intent 不得为空` |
| `tool-cards.tsx` `BashCard` → 折叠态标题 | `TraceTitle` 子节点 | `intent === '' ? 'Bash' : \`Bash · ${intent}\``（分隔符为**空格 + `·` + 空格**三个字符） |

> 「折叠态用户只读这一行」与「建议 ≤30 字」「写目的而非复述命令文本」三项**只出现在**上表第一行；`grep` 判据见 tasks 自查项（工具级描述、`Err` 文案、`bash.rs` 注释均零命中这些措辞）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 工具定义面（**修改**） | `bash` 定义增第三属性 `intent`（`type: "string"`，无 `maxLength`）+ 并入 `required` + 工具级描述末尾追加强制句；其余六工具定义逐字不变 | `crates/infra/agent/src/sdk/mount.rs`（`generic_definition` 的 `"bash"` 臂 `:473-487`） | 既有 `serde_json::json!` 字面量 | 静态字面量构造；`TOOL_NAMES` / `definitions` / 注入面 / `RunStarted.tools` 上报语义零改动 |
| 执行体入参闸（**修改**） | `intent` 三形态硬拒（缺失 / 非字符串 / `trim` 后空），位点在 `command` 闸之后、`resolve_timeout_ms` 之前；长度不设分支 | `crates/infra/agent/src/sdk/bash.rs`（`execute` `:16-26`） | 既有 `serde_json::Value` | 纯同步前置校验；`resolve_timeout_ms` / `build_command` / `drain` / `merge_output` / 超时与进程树清理臂零改动 |
| 折叠态标题（**修改**） | 折叠态标题 `Bash · <intent>`，`intent` 归一后缺席则逐字回落 `Bash`；单条省略链由既有 `TraceTitle` 承载 | `packages/desktop/src/components/agent/message-cards/tool-cards.tsx`（`BashCard` `:285-322`） | 既有 `@/lib/parse` 的 `asRecord` / `textOf`、既有 `@/components/ui/trace` 的 `TraceTitle` | React 函数组件（既有形态），零新依赖、零新导出、零新增 UI 变体 |
| 版本行（**修改**） | patch 递增（`tauri.conf.json` 跟随、crate 版本不跟） | `packages/desktop/package.json` | — | 版本纪律（AGENT.md） |

<!-- 零新增 crate / 零新增模块 / 零新增架构模型元素：三处源码都落在既有元素内（后端 `App.Backend.Infra.AgentRuntime`、前端 `App.Frontend.Components.Agent.ToolCards`），`.dev-team/architecture/models/**` 与 `graph/INDEX.toml` 零改动（两元素均 gate = off 且 api.status = none）。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀；前端侧写全 `packages/desktop/…`。本清单覆盖 proposal「变更范围 - 实现文件」全部 4 项。测试文件与用例归测试相位，本清单另设「影响传导」段登记必填扩位对既有夹具的运行期影响。

### 新增文件

**无。** 本变更三处都是既有模块内的一处扩位（工具定义的一个属性、执行体的一段前置校验、一个组件的标题表达式），没有独立的新职责单元；零新 crate、零新模块。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/agent/src/sdk/mount.rs` | `generic_definition` 的 `"bash"` 臂（`:473-487`）：① `properties` 在 `command` 与 `timeout_ms` 之间插入 `intent`（逐字见「文案台账」第 1 行，**无 `maxLength`**）；② `required` 由 `["command"]` 改为 `["command", "intent"]`；③ 工具级 `description` 追加末尾句 `；每次调用必须给 intent（缺失或空白即拒绝执行）`（既有三段逐字不动）。④ 其余六工具定义、`TOOL_NAMES`、`definitions`、`injected` / `injected_finish` 夹具、`ToolTier` 表逐字不变 | AC-1 / AC-9；追加句落末尾是 `tools_test.rs:1290` 的 `starts_with` 前缀锚要求的 |
| `crates/infra/agent/src/sdk/bash.rs` | `execute`（`:16-26`）：在 `command` 判空（`:22-24`）之后、`let timeout = Duration::from_millis(resolve_timeout_ms(input)?)`（`:26`）之前插入意图闸——`input.get("intent").and_then(Value::as_str)` 缺失 / 非字符串 → `Err("入参缺失或非字符串字段: intent")`；`intent.trim().is_empty()` → `Err("intent 不得为空")`（DD4）。不新增长度分支、不改既有 `command` 两条文案。`resolve_timeout_ms` / `build_command` / `drain` / `merge_output` / 双管道并发排水 / 超时罩全段 / `kill_tree` / 退出码臂逐字不变 | AC-2 / AC-3 / AC-4；`intent` 只在闸内出现（不进命令行 / ToolResult / 日志） |
| `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` | `BashCard`（`:285-322`）：① `const command = textOf(asRecord(input).command)` 一行扩为 `const record = asRecord(input);` + `const command = textOf(record.command);`（`command` 读取语义不变）；② 增 `const intent = textOf(record.intent)?.trim() ?? '';`（DD2 哨兵）；③ `TraceTitle` 子节点由 `Bash` 改为 `{intent === '' ? 'Bash' : \`Bash · ${intent}\`}`（DD1 单文本节点）；④ 组件 doc 注释更新为「命令作展开态标题栏、`intent` 作折叠态标题；输出折叠在下方（`intent` 缺席时折叠态仅工具名）」。⑤ 逐字不动：`TraceLine status` 三态判据、`unpairedRunning` 调用、`TraceIcon` 内 Spinner ↔ TerminalIcon 互斥、`TraceCollapsible` / `TraceCollapsibleTrigger` / `TraceCollapsibleContent` 结构、`CodeBlock` / `CodeBlockHeader>{command}` / `CodeBlockContent`（`max-h-60` / `wrap` / `data-testid="block-tool-result"`）、`cardAttrs` 卡根钩子、`BUILD_IN_TOOL_CARDS` 键面与声明序 | AC-5 / AC-6 / AC-7 / AC-8；零新导出（`BashCard` 仍模块私有）、零新 prop、零 JS 侧截断 |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 +1 patch**（当前 `0.6.52` → 期望 `0.6.53`；以 `git diff HEAD -- packages/desktop/package.json` 的版本行为「恰好 +1」判据，MUST NOT 写死字面号，DD7） | `src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随（零改动）；`src-tauri/Cargo.toml` crate 版本行**不跟** |

### 影响传导（必填扩位对既有夹具的运行期影响；实现相位零动作）

> 本变更**不产生编译期破坏**（`intent` 是 JSON 字面量字段、前端是可选呈现位）；但「`intent` 必填 + 硬闸」使若干既有夹具在**运行期**不再合法。实现相位不改任何测试文件，只在此登记点位，用例与断言的设计归属 proposal「测试文件」表与测试相位。

| 面 | 点位（实测） | 传导后果 |
|----|----|----|
| `crates/infra/agent/src/sdk/bash_test.rs` | 10 处 `execute` 调用（`:29` / `:42` / `:70` / `:90` / `:107` / `:118` / `:126` / `:150` / `:276` / `:305`）全部只带 `command`（部分带 `timeout_ms`） | 一律在意图闸处 `Err`，需补 `intent`；`:118` / `:126` 的「缺 `command`」「`command` 空白」两条断言**零改即可继续命中**（DD4 校验序：`command` 先报） |
| `crates/infra/agent/src/sdk/tools_test.rs` | 定义面快照行 `:1281` 断言 `bash` 的 `required == ["command"]`；执行夹具 `:973`（60KB 超限用例）、`:998`（bash 臂分发正向）、`:1003` 的 `execute(&root, "bash", &json!({}))`（缺参） | `required` 断言与必填扩位矛盾（需改两值）；执行夹具需补 `intent`；`:1003` 的 `{}` 用例**断言零改**（仍报 `command`） |
| `crates/infra/agent/src/sdk/loop_test.rs` | 走真实执行的 5 处（`:2739` 六工具直驱 bash 臂、`:2942` 命令分发、`:2995` 写保护放行臂、`:3033` 放行、`:5651` 历史配对末条） | 需补 `intent`（否则该 `ToolResult` 变成 `is_error`，若用例只断结果条数会静默通过——R5 的假绿风险） |
| 同上（**拒绝臂**） | `:2974`（`find . -delete`）、`:3345` / `:3350`（`echo x > <config>` / `cp /tmp/x <config>`）——写保护闸在 `loop.rs:658` 先于执行臂路由 | **零影响**：不触 `bash::execute`；补齐 `intent` 只是免误读，非必需 |
| `crates/infra/agent/src/sdk/protect/{mod_test,reason_test}.rs`、`policy_test.rs`、`mount_test.rs` | 纯保护判定 / 档位表 / 定义面断言，不入执行体 | 零改动；**出现 diff 即说明实现把意图闸错放进保护面**（DD6） |
| `packages/desktop/src/components/agent/message-cards/tool-cards.test.tsx` | `:453` 的 `output 三态` 夹具 `{ command: 'ls -la' }` 不带 `intent`；`:459` 的 `getByText('Bash')` 精确断言；`:511` 区域命令栏异常载荷四形态（`{}` / `null` / 裸字符串 / 正常） | **零影响**（缺席走回落路，标题逐字 `Bash`，`:459` 零改即绿；命令栏用例不触及标题）；新增带 `intent` 的用例按 AC-5 锚 `trace-title` 的 `textContent` 全串 |
| `crates/infra/agent/src/sdk/runner_test.rs` / `resume_test.rs` / `core/agent/src/runner_test.rs` | 只断言工具名清单与历史块形状（`tool_use_block(…, "bash")`），不入执行体 | 零改动 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名（参数名、类型、返回类型） | 说明 |
|--------|----------|-----------|------|------|
| `execute` | `crates/infra/agent/src/sdk/bash.rs` | 修改（签名与可见性不变） | `pub(crate) async fn execute(root: &Path, input: &Value) -> Result<String, String>` | 新增意图闸（DD4）：`command` 闸之后、`resolve_timeout_ms` 之前，`intent` 缺失 / 非字符串 → `Err("入参缺失或非字符串字段: intent")`，`trim` 后空 → `Err("intent 不得为空")`；长度不设分支；`Ok` 分支的合并输出口径与 `Err` 分支的超时 / 非零退出码文案逐字不变 |

<!-- 未列条目及原因：`mount.rs::generic_definition(name: &str) -> ToolDefinition` 为**模块私有**函数（AGENT.md「私有函数不列」），定义面扩位在「修改文件」表登记；`mount.rs::definitions(...)`、`TOOL_NAMES`、`tools.rs::execute(root: &Path, name: &str, input: &Value) -> Result<String, String>` 签名与实现逐字不变（input 原样透传，零字段感知），故不列；前端零新增 / 零修改的模块级导出面（`BashCard` 模块私有、`BUILD_IN_TOOL_CARDS` 键面与引用不变），无新增 CLI 子命令、无新增 HTTP 端点、无新增 IPC 命令。 -->

### 类型定义

**无新增、无修改。** Rust 侧 `ToolUseBlock.input` 仍是 `serde_json::Value`（`crates/core/agent/src/event.rs:62-67`），`intent` 只是该 JSON 内的一层键——`AgentBlock` 变体词汇、`serde` 派生、事件词汇零扩展；前端 `AgentToolCardProps`（`tool-cards.tsx:46-57`）零改（`input` / `output` 仍是既有形状），`BashCard` 的视图派生量 `intent: string`（`''` = 缺席）是**函数内局部常量**，不成类型。

<!-- 该子表以「无」作答是因为 spec 明写「零新列、无新编码、不出线新 IPC 类型」；若实现期出现任何 `struct` / `interface` / `type` 的字面改动（例如给 `ToolUseBlock` 加 `intent: Option<String>` 字段），即为越界。 -->

### 配置

**无。** 零 `KNOWN_KEYS` 增删、零 `TestFramework` 变体、零 `crates/core/config/config.schema.json` 镜像改动（`intent` 是模型工具面参数，不读工作区配置）。`packages/desktop/package.json` 的 `version` 是版本纪律项，已登记在「修改文件」表（DD7），不属配置键。

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `bash` 工具调用载荷（模型 → 工具的线形态） | `command: string`（必填）；`intent: string`（**新增必填**，语义 = 一行意图摘要）；`timeout_ms?: integer` | 仅 `command` 进执行与写保护裁定；`intent` 只被 `bash.rs` 闸校验一次、被前端标题呈现；三者互不推导 | 随 `AgentBlock::ToolUse.input` 原样进会话转录（store 以不透明 JSON 存 `AgentEvent`）——`intent` 因此**零列、零迁移、零回填**；旧行天然「无意图」 |
| `AgentBlock::ToolUse`（`crates/core/agent`） | `id: String`；`name: String`；`input: serde_json::Value` | `input` 为 `bash` 载荷的字面容器（结构由工具自解释） | **零改动**（无新字段、无编码改动）；重放路（库内转录）与实时路同形，故 AC-9 的「重放同形」由数据结构本身保证 |
| `BashCard` 视图派生量（前端，函数内局部） | `command: string \| null`（既有，`textOf`）；`intent: string`（**新增**，`textOf(...)?.trim() ?? ''`，`''` = 缺席哨兵） | `intent` ← `ToolUse.input.intent`；`''` 与 `Bash` 标题一一对应；与 `output` 无交叉引用（前端不从结果反推意图） | 不持久化（渲染期派生） |

**关系与持久化要点**

- **`intent` 的唯一持久化归属 = 转录里那份工具调用载荷**。它不进 `ToolResult.content`、不进 `PhaseRecord`、不进任何结构化列；`store` / `bindings` / `specta` 面零认知。
- **两层严格性分治**（spec 明写）：执行层对新提交严格（缺失 / 非字符串 / 空白一律硬 `Err`），展示层对历史转录宽容（缺席逐字回落 `Bash`）。宽容面只服务变更前已落库的会话，不可作「新提交可省 `intent`」的依据。
- **上下文代价**：`intent` 随 input 进历史并被每轮请求重发（R4），体量与 `evidence` 同量级；`context.rs` / `compact.rs` 既有的剪裁规则**不为其加特例**。
- **无新表、无新列、无外键、无迁移**：本变更一切「新数据」都是既有 JSON 列内的一层键。

---

## 路由 / API 设计

**无。** 本变更不改 IPC 命令面、不改 HTTP 端点、不新增查询面：`bash` 是模型工具（schema 与校验都在进程内），`intent` 不进任何出线类型（`ToolUseBlock.input` 仍是自由 JSON），`bindings:export` 再生成后**零 diff**。

---

## 依赖

- **运行时依赖：零新增。** `mount.rs` 只用既有 `serde_json::json!` 字面量；`bash.rs` 只用既有 `serde_json::Value`（`as_str`）与 `std`；`tool-cards.tsx` 只用既有 `@/lib/parse`（`asRecord` / `textOf`）与 `@/components/ui/trace`（`TraceTitle` 及其内建 `min-w-0 truncate`）。四处 `Cargo.toml`（`infra/agent`、`src-tauri`、workspace 根、`core/agent`）与 `packages/desktop/package.json` 的依赖段**零 diff**。
- **依赖方向零变化**：三处改动都在既有文件既有函数体内，`infra → core` 的既有单向关系不新增任何边（`.dev-team/architecture/models/**` 零改动）。
- **构建 / 测试依赖：零新增。** Rust 侧沿用既有 `cargo test -p agent-runtime`（内存 `json!` 夹具 + `tempdir`）；前端沿用既有 `vp test` + testing-library 组合，无新测试库、无新 mock 面（`BashCard` 无新 I/O）。

---

## 验收标准落点

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 定义面 | `mount.rs` `"bash"` 臂：`required` 逐值为 `["command","intent"]`、`properties.intent` 为 `{type:"string", description:<文案台账第 1 行>}` 且**无 `maxLength`**；四个取值要素单点在字段 `description`、工具级只追加强制句（DD3）；其余六定义逐字不变（既有断言零改即证） |
| AC-2 意图闸（反向） | `bash.rs::execute` 的两条 `Err`（DD4）：`{}` 与「只带 `command`」落「入参缺失或非字符串字段: intent」；数字 / 对象 / `null` 经 `as_str` 同样落该文案；`"   "` 落「intent 不得为空」；`{}` 的最先报错仍是 `command`（位点在 `command` 闸之后） |
| AC-3 意图闸（正向 + 软约束） | `bash.rs` 闸通过后既有执行路径零改动（输出与合并口径、退出码臂逐字不变）；200 字 `intent` 走同一条 `Ok` 路径——DD4 明令不加长度分支，`intent` 的值除 `trim().is_empty()` 外不被读取 |
| AC-4 执行面零回归 | `bash.rs` 的 `resolve_timeout_ms` / `build_command` / `drain` / `merge_output` / 超时 + `kill_tree` 臂 `git diff` 为空；补 `intent` 后既有夹具断言逐条不变（影响传导表已点出须补位的点位） |
| AC-5 折叠态正向 | `tool-cards.tsx` 的 `TraceTitle` 单文本节点 `` `Bash · ${intent}` ``（DD1 / DD2）；`trace-title` 的 `textContent` 为 `Bash · 跑单测`；命令原文只出现在展开态 `CodeBlockHeader`（既有结构零改） |
| AC-6 折叠态回落 | `intent` 哨兵 `''` 覆盖缺席 / 空串 / 非字符串 / 纯空白四形态 → 标题逐字 `Bash`（无 `·` 残留、无占位符、无 `undefined` / `null` 文本）；读取全程走 `textOf` / `asRecord` 的宽容口径（无强转、无 `JSON.parse`），故不抛错 |
| AC-7 单条省略链 | 复用既有 `TraceTitle`（`min-w-0 truncate`，省略点在行尾故前缀 `Bash` 恒可见），零 `trace-description` 节点、零新增类、零 `title` 属性 / tooltip、零 JS 侧截断（`intent` 原文整串进文本节点） |
| AC-8 展开态与状态零回归 | `BashCard` 的 `TraceLine status` 三态、`TraceIcon` Spinner ↔ TerminalIcon 互斥、`CodeBlockHeader>{command}`、`block-tool-result>{output?.content}`、`cardAttrs` 钩子、`BUILD_IN_TOOL_CARDS` 键面与声明序全部逐字不动（既有用例零改即证） |
| AC-9 转录自足（重放同形） | `intent` 只在 `ToolUseBlock.input`（`serde_json::Value`）内，store / 事件词汇零改动；重放路拿到同一份 input → 折叠态同形；`bindings:export` 零 diff |
| AC-10 零外溢 | DD6 的边界复核：`protect/**`、`sandbox.rs`、`tools.rs::input_path`、`context.rs` / `compact.rs`、`crates/core/agent/**`、`crates/infra/store/**`、`trace.tsx`、`blocks.tsx`、16 份 phase 提示词、`AGENT.md` 全零 diff |

### 交付门（非本变更能力判据）

全仓既有门（静态检查链 `server:check` → `bindings:export` → `api:check` → `client:check`、全量测试、版本递增、`bindings:export` 零 diff）由主基线承载，本设计与 tasks 不派生新判据。tasks 的自查项只收**源码级静态判据**（`grep` 命中数与零命中、读码结构复核、`git diff` 边界比对、版本行相对 `HEAD` +1 patch）；两个验证命令（`cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime`、`pnpm -C packages/desktop exec vp test run src/components/agent/message-cards/tool-cards.test.tsx`）与整链门在 tasks 中只以「交接」条目登记。

---

## 待决问题

1. **漏传率（R1）的观测口径**：本期不做任何采样 / 埋点，`intent` 漏传只体现为一次 `is_error` ToolResult（文案含 `intent`）与既有引擎日志。若落地后漏传率偏高，方向是在工具级描述里补一句「每条命令都要给」（**不改闸强度**），MUST NOT 回退成「可选」——回退即让 AC-1 的「必填」变成不可断言的事实。
2. **意图质量（R2）的回退规则**：模型可能写「运行命令」「执行一下」这类空话，本变更**不做**机械校验（无法判定「写目的 vs 复述命令」的语义差）。若要处置，属独立变更，方向是「折叠态回退规则（判为空话则附加命令首行）」——但那会引入 proposal D7 明确拒绝的「同一位置两种语义」，须先重新裁定 D7。
3. **超长 `intent` 的观感样本**：`truncate` 在极窄容器下会把标题截到只剩 `Bash …`（省略点在行尾，工具名恒可见，符合 AC-7）。本期不做自适应（如先缩后代、分两行）——若实测「意图几乎总被截掉半句」导致信息量回到零，优化方向是前端版式（属独立变更），而非收紧长度闸。
4. **既有 `getByText('Bash')` 断言的最终处置**：设计给的是规则（带 `intent` 的夹具锚 `trace-title` 的 `textContent` 全串 / 等价 `/^Bash/`；不带 `intent` 的夹具零改），具体到每个用例的落笔归测试相位。若测试相位选择给 `:453` 的夹具补 `intent`，则 `:459` 必须同步改为正则——这是唯一允许的既有断言改写点，且须在同一提交内完成。
5. **`intent` 与 `timeout_ms` 的属性顺序是否会误导模型**：本期把 `intent` 放在 `command` 之后、`timeout_ms` 之前（必填项相邻）。若实测模型把 `timeout_ms` 的说明与 `intent` 的说明混读（两者都在描述「何时/多久」的语境里），可考虑在 `intent.description` 里加强「意图」二字的辨识度——本变更不预留开关，属观察后再定。
