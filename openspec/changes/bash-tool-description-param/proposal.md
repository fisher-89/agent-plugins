# 提案: bash-tool-description-param

> **变更**: bash-tool-description-param
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/bash-tool-description-param/explore.md`（需求原文：bash 工具增加参数「描述」，必须传入简短字符（一行可显示）说明 bash 的行为或目的，并在工具卡片**未展开时**即可见）
> **范围接缝**: 本变更只做**「让每次 bash 调用自带一行意图摘要，并在卡片折叠态可见」**三件事：① `bash` 工具定义增 `intent` 参数（进 `required`）、② `bash::execute` 入参闸硬拒缺失 / 非字符串 / 空白 `intent`、③ `BashCard` 折叠态标题呈 `Bash · <intent>`（缺席回落 `Bash`）。MUST NOT 做：命令文本上提折叠态、`read` / `write` / `edit` / `web_search` / ask / `finish_step` 卡改版、落库形状变更、命令级白/黑名单或审批通道、意图质量机械校验。

---

## 问题

### 现象：bash 是唯一「折叠态零信息」的工具卡

工具卡折叠（未展开）时的标题是用户在长会话里扫读的唯一抓手。`read` / `write` / `edit` 折叠态标题都拼了参数（`Read src/a.ts:10-15`、`Write app.tsx`），`web_search` 折叠态标题干脆就是查询词本体。**只有 bash 折叠态恒为两个字 `Bash`**——一次会话跑十几条命令时，卡片列表退化成一片同形 `Bash`，用户必须逐张展开才知道每条在干什么。

### 根因：命令文本天然不适合当摘要

直觉解法是「把 `command` 提到折叠态」，但探索阶段已排除：bash 命令常常长到无法当摘要读（多段 `&&`、管道、heredoc、内联脚本），首行截断后既不完整也不表意，且会把「用户想知道的『这步在干什么』」替换成「命令文本前缀」。**正确解法是让模型给一行意图摘要**——模型是唯一知道「我为什么要跑这条命令」的一方。

### 现状链路与断点

| 环节 | 手上有什么 |
|---|---|
| `mount.rs::generic_definition("bash")` | `properties: { command, timeout_ms }`，`required: ["command"]`——**没有意图位** |
| `tools::execute` → `bash::execute(root, input)` | 只读 `command` / `timeout_ms`；`command` 缺失或空白 → 硬 `Err`，`timeout_ms` 越界 → 硬 `Err` |
| `core/agent` `ToolUseBlock { input: Value }` | input 原样 JSON 落库（转录自足：卡片所需一切只能来自 input） |
| `BashCard`（`tool-cards.tsx`） | 折叠态：`TerminalIcon` + `TraceTitle>Bash`；展开态：`CodeBlockHeader{command}` + 输出 |

即：前端卡片能拿到的只有 `input` / `output`，**纯前端无解**（前端不可能知道命令目的，也无法可靠地从命令文本反推）。断点在工具协议面。

### 硬约束（决定解法形状）

1. **卡片折叠态只能消费工具调用载荷**：`BashCard` 入参只有 `input` / `output`，且重放路（库内转录）与实时路必须同形——数据必须随 input 落库，MUST NOT 新起查询面或落库新列。
2. **不自造新约定**：工具定义与执行体闸的唯一真源分别是 `sdk/mount.rs`（定义面）与 `sdk/bash.rs`（执行体）；卡片的省略链由 `TraceTitle` 的 `min-w-0 truncate` 承载，`TraceDescription` 全仓零使用先例。
3. **写保护闸只读 `command`**：`protect/mod.rs` 的判定输入与会话执行根不涉其它字段，本变更 MUST NOT 触碰该面。
4. **长度是审美约束，不是协议约束**：模型难以稳定命中字数，硬长度闸会显著抬高重试率；软提示 + 前端 truncate 兜底即可。

## 提案

```
今天                                    之后
──────────────────────────────────      ─────────────────────────────────────
bash { command, timeout_ms? }           bash { command, intent, timeout_ms? }
      │                                       │  schema required: ["command","intent"]
      │                                       ├─ bash::execute 硬闸：
      │                                       │    缺失 / 非字符串 / trim 为空 → Err
      │                                       └─ 长度仅软提示（不加 maxLength、不 Err）
      ▼                                       ▼
折叠态：Bash                            折叠态：Bash · 跑单测
展开态：命令 + 输出                     展开态：命令 + 输出（零改动）
```

一句话：**把「一行意图」变成 bash 调用协议的必填成员（模型写、执行体守），于是折叠态标题只靠 input 就自足可读，而命令文本与展开态一字不动。**

### 决策表（含被拒备选与理由，逐条自足成证）

| # | 决策 | 要点 | 被拒备选与理由 |
|---|---|---|---|
| D1 | 意图经**工具调用输入**承载（模型提交 + 执行体守门） | `intent` 必填、随 input 原样落库 → 折叠态在实时路与重放路同形 | **被拒：前端从 `command` 提首行当摘要**——命令前缀不表意（多段 `&&` / heredoc 时尤其糟），且「这步在干什么」这一信息前端物理上拿不到。<br>**被拒：后端在 ToolResult 信封里回填意图**——信封要进模型上下文（每条命令多一段重复文本），且收口前 `output` 缺席 → 标题「先没有、收口后凭空出现」抖动。<br>**被拒：新增 `AgentBlock` 变体或 store 列**——为一行展示文本改 core/agent 事件词汇、`specta` bindings 与 store 编码，量级远超收益。 |
| D2 | 强制力度 = **执行体硬闸** | `intent` 缺失 / 非字符串 / trim 后为空 → `bash::execute` 直接 `Err`（与 `command` 闸同形、同一处），模型在同一 turn 内自纠重试 | **被拒：只进 schema `required`**——JSON Schema 的 `required` 对多数模型只是提示，漏填不失败，于是「必填」成为不可断言的事实，折叠态又回到时有时无。<br>**被拒：缺失即静默跳过执行并回灌提示**——同一件事两套语义（命令缺失是 Err、意图缺失是提示），模型难学。 |
| D3 | 校验序：`command` 先于 `intent` | 两者都缺时报 command 的错误——既有错误的提示语与既有测试断言全部保持 | **被拒：意图先校验**——会让「缺 command」这一最基础错误变成 intent 报错，既有 `bash_test` / `tools_test` 断言连带重写，收益为零。 |
| D4 | 字段名 = **`intent`** | 避开与 JSON Schema 每个 property 自带的 `description` 键字面撞车（`"description": {..., "description": "…"}` 读起来是歧义源） | **被拒：`description`**——字面重复，写 schema 与读 schema 都会误解层级。<br>**被拒：`summary` / `purpose`**——与用户原话「描述」的偏移不换来任何额外清晰度，`intent` 恰好对齐「这条命令要干什么」。 |
| D5 | 长度 = **软约束** | schema 不加 `maxLength`，执行体不因长度 `Err`；只在 `intent.description` 写「建议不超过 30 字」；前端 `truncate` 兜底 | **被拒：执行体超长 `Err`**——字数是审美判断，模型难稳定命中，硬 Err 换来高重试率与步成本。<br>**被拒：schema `maxLength: 30`**——「写了 60 没人在意」的假精确（无执行体配合即无效约束），反而让模型把注意力花在数字符上。 |
| D6 | 折叠态排版 = 工具名与意图**共享同一条 `TraceTitle`** | 同一元素内 `Bash · <intent>`，省略由 `TraceTitle` 自带的 `min-w-0 truncate` 自动收尾（单条省略链） | **被拒：用 `TraceDescription` 次级位**——该元素 `shrink-0` 且未 truncate（全仓零使用先例），要补覆盖样式并实机调截断，且两元素各占一条省略链（省略点会分裂）。<br>**被拒：意图单独成行**——折叠态行数翻倍，扫读效率反降。 |
| D7 | 历史回落 = **省略** | `intent` 缺席（旧转录、非 bash 工具）时折叠态标题逐字 `Bash`，零回填、零占位符 | **被拒：回填命令首行**——引入「同一位置两种语义」（有的卡是意图、有的是命令），扫读一致性崩坏。<br>**被拒：补「（无描述）」类占位**——纯噪声。 |

### 明确不做（知情边界，非遗漏）

- **命令文本不上提折叠态**：`BashCard` 展开态的 `CodeBlockHeader{command}` 与结果区逐字不变。
- **`read` / `write` / `edit` 折叠态不改**：它们已有可读路径；`db_query` / `archi_*` 的折叠态摘要若也要意图化，属同族独立需求。
- **不做意图质量机械校验**：无法机械判定「写目的 vs 复述命令」，本变更只要求非空。
- **不加卡片交互**：不加 tooltip / 展开态意图行 / 复制 / 意图搜索。
- **命令级白/黑名单、真沙箱、交互审批**：既有知情边界（见 desktop-agent-execution「SDK 租户 MVP 边界与已知限制」）不变，本变更不借机扩面。
- **落库面零变更**：`intent` 随 `ToolUseBlock.input` 原样进出，store 列 / 归纳逻辑 / 重放路径一字不动。
- **`AGENT.md` / 16 份 phase role 提示词不改**：提示词只讲 bash 的使用边界，不含入参样例。

### 未决项（交 design 定稿，写进风险而非范围）

1. **工具名与意图的视觉分色**：是否把意图包进内层节点做略淡着色（`text-muted-foreground/70`）。**采纳则附带收益**：`Bash` 保留为独立文本节点，既有 `getByText('Bash')` 精确断言不动；**不采纳**则标题为单文本节点 `Bash · <intent>`，该断言须改为 `/^Bash/` 正则。二者都满足「单条省略链」，由 design 择一并同步测试。
2. **两处引导措辞的分工**：工具级 `description`（讲「为什么必须给」）与 `intent` 的字段 `description`（讲「怎么写」）各承载哪一半——单点原则要求两处不重复同一句话。

## 能力

### 新增的能力

**无。** 本变更全部落在既有能力面内（SDK 引擎工具面 / 执行体闸、前端会话基建的工具卡）。

### 修改的能力

- **desktop-agent-execution** —
  - **MODIFIED**「SDK 引擎 bash 工具」：入参面由 `command` + `timeout_ms` 扩为 `command` + `intent` + `timeout_ms`；`intent` 进 schema `required`（`properties.intent` 为 `string`、**不加 `maxLength`**）、工具级 `description` 增引导；`bash::execute` 在 `command` 闸**之后**增意图闸（缺失 / 非字符串 / trim 后为空 → `Err`）；长度仅软提示（不因长度 `Err`），展示截断由前端承载；`intent` 随 `ToolUseBlock.input` 原样落转录，store / 重放 / 写保护 / 路径沙箱 / 上下文剪裁全链零消费。

- **desktop-agent-chat-infra** —
  - **ADDED**「bash 工具卡折叠态意图行」：`BashCard` 折叠态（未展开的 `TraceCollapsibleTrigger`）标题 SHALL 呈 `Bash · <intent>`（`intent` 取 input 的 `trim` 后原文，不改写、不折叠空白）；工具名与意图 SHALL 同处 `data-slot="trace-title"` 单条省略链（MUST NOT 另起 `trace-description` 次级位）；`intent` 缺席 / 非字符串 / 空白串时 SHALL 逐字回落 `Bash`（MUST NOT 回填命令、MUST NOT 占位符）；折叠态 MUST NOT 含命令原文；展开态命令栏与结果区、卡根钩子、`running` / `error` 三态判据逐字不变。

### 其余能力零改动

- **desktop-write-protection**：保护闸判定输入仍只有 `command` 与会话执行根，`intent` 不进判别链。
- **desktop-file-read / desktop-agent-db-tools / desktop-archi-agent**：非 bash 工具定义与执行体零改动；`compose` 工具名清单（含 explore / archi / change 三臂）零改动。
- **desktop-phase-prompts**：16 份 `prompts/*.md` 零改动。
- **desktop-workspace-store / desktop-change-state-store**：转录与轮记录形状零变更（`intent` 是 input JSON 内字段，无新列、无编码改动）。
- **desktop-ipc-type-bindings**：`ToolUseBlock.input` 仍是 `serde_json::Value`，不出线新 IPC 类型，`bindings:export` 再生成后零 diff。
- **desktop-app-shell**：`components/ui/trace.tsx`（`TraceLine` / `TraceTitle` / `TraceDescription`）零改动——本变更消费既有 `min-w-0 truncate` 省略语义，MUST NOT 为其新增变体或 prop。
- **desktop-crate-layout / repo-archi-model**：零新 crate、零新元素、零新依赖边（`App.Backend.Infra.AgentRuntime` 本无契约文件且 gate 为 off）。
- **desktop-agent-chat-infra 其余卡片**：`read` / `write` / `edit` / `web_search` / `AskUserQuestion` / `finish_step` 六卡与 `BUILD_IN_TOOL_CARDS` 键面零改动。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | `generic_definition("bash")`：`properties` 增 `intent`（`"type": "string"`，description 写取值纪律「一句话说明这条命令要干什么——折叠状态只读这一行；建议不超过 30 字，写目的而非复述命令文本」，**不加 `maxLength`**）；`required` 改为 `["command", "intent"]`；工具级 `description` 增一句引导（分工见未决项 2）。其余六个工具定义逐字不变 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs` | `execute`：`command` 闸之后、`resolve_timeout_ms` 之前增意图闸——`input.get("intent").and_then(Value::as_str)` 缺失 / 非字符串 → `Err("入参缺失或非字符串字段: intent")`，`trim().is_empty()` → `Err("intent 不得为空")`；不新增长度分支。`resolve_timeout_ms` / `build_command` / 排水 / 合并 / 退出码臂逐字不变 |
| `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` | `BashCard`：`const intent = textOf(asRecord(input).intent)`，空白串按缺席归一（`intent?.trim()` 为空 → `null`）；`TraceTitle` 文本按「决策 D6 + 未决项 1」呈 `Bash · <intent>` 或回落 `Bash`；`TraceLine status` 三态判据、`TraceIcon` 图标互斥、`CodeBlock` / `CodeBlockHeader` / `CodeBlockContent` 与两个 `data-testid` 钩子逐字不变；`BUILD_IN_TOOL_CARDS` 键面与顺序不变 |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 +1 patch**（当前盘面 `0.6.52` → 落地时 `0.6.53`；以 `git diff HEAD -- packages/desktop/package.json` 的版本行为「恰好 +1」判据，MUST NOT 写死字面号）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| `bash` 定义面：`required` 恰为 `["command","intent"]`、`properties.intent.type == "string"`、`properties.intent` **无 `maxLength`**；既有「`timeout_ms` 不在 required」断言零改 | `sdk/mount_test.rs` 与 `sdk/tools_test.rs`（定义面断言段） | 新增断言（既有断言零改即证其它六工具零漂移） |
| 意图闸三形态：缺 `intent` / `intent` 非字符串（数字、对象、`null`）/ 空白串 → `Err` 且文案含 `intent`；`command` 与 `intent` 同缺 → 报 `command`（校验序守门） | `sdk/bash_test.rs` | 新增 |
| 长度软约束：`intent` 为 200 字长串 → 命令正常执行、零 `Err`（软约束不可变硬） | `sdk/bash_test.rs` | 新增 |
| 既有 bash 执行夹具（stdout/stderr 合并、非零退出码、超时与边界、命令空白、git-bash 探测与 `cmd /C` 兜底，约 10 处 `execute` 调用）补 `intent` | `sdk/bash_test.rs` | 改造夹具（断言逐条不变） |
| 走真实 `execute` 的 bash 夹具补 `intent`：六工具直驱用例、写保护三连**放行臂**、命令分发臂、历史重建末条 `bash` 用例 | `sdk/loop_test.rs`（约 6 处）、`sdk/tools_test.rs`（3 处；含「缺 command 必 Err」用例补意图后再断言 command 优先） | 改造夹具。**判据**：凡断言「bash 执行成功 / 输出内容」的夹具必补；写保护**拒绝臂**不触 `execute`（闸在 loop 守卫链，先于执行体），可不改但建议同批补齐以免误读 |
| 纯保护判定夹具（只读 `command` 做拦截判定、不入执行体） | `sdk/protect/mod_test.rs`（约 17 处）、`sdk/protect/reason_test.rs`、`sdk/policy_test.rs` | 零改动（若出现 diff 即说明实现把意图闸错放进保护面） |
| 折叠态标题正向：`{ command: "pnpm -C packages/desktop exec vp test run", intent: "跑单测" }` 未展开时 `data-slot="trace-title"` 文本含 `Bash` 与 `跑单测`、**不含命令原文**；展开后命令栏仍为命令原文 | `message-cards/tool-cards.test.tsx` | 新增 |
| 折叠态回落三形态：`intent` 缺席 / 空串 / 非字符串 → 标题逐字 `Bash`、无 `·` 残留、无 `undefined` / `null` 文本、不抛错 | `message-cards/tool-cards.test.tsx` | 新增 |
| 单条省略链：`trace-title` 节点在场且为唯一承载（零 `trace-description` 节点）、超长 `intent` 原文完整保留在文本节点（不 JS 侧截断） | `message-cards/tool-cards.test.tsx` | 新增 |
| 既有 `within(cardRoot()).getByText('Bash')` 精确断言（`BashCard：output 三态`） | `message-cards/tool-cards.test.tsx` | 随未决项 1 的裁定同步（内层节点方案零改，单文本节点方案改 `/^Bash/` 正则）；output 三态 / 图标互斥 / 命令栏异常载荷三组既有用例其余断言零改 |

### 不要修改

- **`crates/infra/agent/src/sdk/protect/**`**：写保护闸只读 `input["command"]`，`intent` 不进判别（该目录出现 diff 即越界）。
- **`crates/infra/agent/src/sdk/sandbox.rs` / `tools.rs::input_path`**：bash 恒 `None`，路径改写面零介入。
- **`crates/infra/agent/src/sdk/{context,compact}.rs`**：不裁剪 tool input，`intent` 随 input 进历史（本变更不为其加豁免规则）。
- **`crates/core/agent/**`**：`ToolUseBlock.input` 仍是 `serde_json::Value`，事件词汇与块词汇零扩展。
- **`crates/infra/store/**`**：转录与轮记录形状零变更（`intent` 藏在既有 input JSON 里）。
- **`crates/core/workflow/src/prompts/*.md`（16 份）与 `AGENT.md`**：零改动。
- **`packages/desktop/src/lib/{agent-adapter,parse}.ts`**：工具部件管线与 `textOf` / `asRecord` 语义零改动（`intent` 走既有 input 透传）。
- **`packages/desktop/src/components/ui/trace.tsx`**：`TraceLine` / `TraceTitle` / `TraceDescription` 零改动（不新增 prop / 变体 / 类名分支）。
- **`packages/desktop/src/components/agent/message-cards/blocks.tsx`**：其余六卡渲染件零改动。
- **`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` 其余六工具定义、`policy.rs` 三档决策表**：零改动（bash 档位语义不变）。
- **`openspec/changes/bash-tool-description-param/explore.md`**：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | **定义面**：`generic_definition("bash")` 的 `required` 逐值等于 `["command","intent"]`；`properties.intent` 为 `{ "type": "string" }` 形态且**无 `maxLength` 键**；工具级 `description` 与 `intent.description` 合力点出「折叠态只读这一行 / 建议 ≤30 字 / 写目的不复述命令」；`read` / `grep` / `glob` / `ls` / `write` / `edit` 六定义逐字不变（既有断言零改即证） | `mount_test.rs` / `tools_test.rs` |
| AC-2 | **意图闸（反向）**：`execute(root, "bash", {})`、只带 `command`、`intent` 为数字 / 对象 / `null`、`intent` 为 `"   "` 四类输入均 `Err`，文案含 `intent`；且 `{}` 的 `Err` 文案含 `command`（校验序：command 先报） | `bash_test.rs` |
| AC-3 | **意图闸（正向 + 软约束）**：合规 `intent` 下命令正常执行、输出与合并口径不变；`intent` 为 200 字长串时命令照常执行、零 `Err`（长度不可升为硬闸） | `bash_test.rs` |
| AC-4 | **执行面零回归**：补 `intent` 后既有 bash 用例逐条不变——stdout 段在前 / stderr 段带 `[stderr]` 标记在后、非零退出码 `Err` 携全部输出、超时 `[1s,30min]` 越界显式拒绝、`command` 空白 `Err`、git-bash 探测与 `cmd /C` 兜底、`kill_on_drop` 与进程树清理路径 | `bash_test.rs` / `loop_test.rs` / `tools_test.rs` |
| AC-5 | **折叠态正向**：`{ command: "pnpm -C packages/desktop exec vp test run", intent: "跑单测" }` 未展开时 `data-slot="trace-title"` 文本为 `Bash · 跑单测`（工具名 + 分隔符 + `trim` 后原文）；折叠态文本**不含**命令原文 | `tool-cards.test.tsx` |
| AC-6 | **折叠态回落**：`intent` 缺席 / 空串 / 非字符串（历史转录三形态）时标题逐字 `Bash`，无 `·` 残留、无占位符、无 `undefined` / `null` 文本、渲染不抛错 | `tool-cards.test.tsx` |
| AC-7 | **单条省略链**：超长 `intent` 下 `trace-title` 仍在场且为唯一文本承载（零 `trace-description` 节点），文本节点内保留 `intent` 原文（不 JS 侧截断，省略由 CSS `truncate` 出）；不新增 tooltip / `title` 属性 / 多行布局 | `tool-cards.test.tsx` |
| AC-8 | **展开态与状态零回归**：展开后 `code-block-header` 文本仍等于 `command` 原文、`block-tool-result` 仍为 `output.content`；`running` / `error` / 无线三态 `data-status` 与 Spinner / TerminalIcon 互斥判据、`block-tool-use` + `data-tool-id` 钩子、`BUILD_IN_TOOL_CARDS` 键面与顺序全部零变化 | `tool-cards.test.tsx`（既有用例零改即证） |
| AC-9 | **转录自足（重放同形）**：一次带 `intent` 的 bash 调用收口后重放该会话，折叠态标题仍为 `Bash · <intent>`（`intent` 仅经 `ToolUseBlock.input` 落库，无新列、无新查询面） | `loop_test.rs`（历史重建 / 回放路径断言 input 原样）+ 前端重放用例 |
| AC-10 | **零外溢**：`protect/**`、`sandbox.rs`、`tools.rs::input_path`、`context.rs` / `compact.rs`、`crates/infra/store/**`、`trace.tsx`、其余六卡渲染件、16 份 phase 提示词、`bindings:export` 产物均零 diff | `git diff` 复核 + 既有用例零改即绿 |

> **验收表口径**：AC-1…AC-10 是能力验收面（每条对应 `specs/` 下某个 delta requirement 的行为判据）。静态检查链、全量测试、版本递增、`bindings:export` 零 diff 是全仓既有交付门，由主基线承载，不进本表。

### 验证命令

```powershell
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime
pnpm -C packages/desktop exec vp test run src/components/agent/message-cards/tool-cards.test.tsx
```

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 硬闸抬高重试率**：模型漏传 `intent`（尤其长会话 / 历史惯性），每次漏传白费一轮并多一次 `is_error` 结果回灌 | 命令执行延迟上升，用户看到一次失败卡片后再成功 | 中 | ① `intent` 与 `command` 同级进 `required`，并在工具级 `description` 与字段 `description` 两处（分工单点，见未决项 2）明说「必填」；② `Err` 文案与 `command` 闸同形，模型一次即纠；③ 观察落地后的漏传率——若偏高，方向是补一句「每条命令都要给」（不改闸强度），MUST NOT 回退成「可选」 |
| **R2 意图质量稀释**：模型写「运行命令」「执行一下」这类空话，折叠态信息量仍为零 | 需求目标部分未达成（可读性提升打折） | 中 | 字段 description 明确「写目的而非复述命令文本」；不做机械校验（无法判定语义质量，硬判只会逼模型堆砌套话）；若实测空话率高，后续独立变更考虑「折叠态回退规则（如判空话则附加命令首行）」 |
| **R3 历史转录不一致**：旧会话卡片仍只有 `Bash`，用户以为功能没生效 | 观感落差 | 中（必然，存量会话） | 显式决策 D7：不回填、不占位；spec 明写回落语义（旧卡仍完整可读，只是标题更短） |
| **R4 每次调用多几十 token 且随转录回灌**：`intent` 进 input → 进历史 → 每轮请求重发 | 长会话 token 成本小幅上升（与 `evidence` 同量级的小头） | 高（必然） | 建议 ≤30 字（软约束）；`context` / `compact` 不为其加特例；体量在 L2 剪裁面前可忽略 |
| **R5 夹具扩散遗漏 → 假绿 / 假红**：硬闸会把「忘了补 `intent`」的夹具变成 `is_error` 结果，若某用例只断言「结果条数 == 1」就会静默通过 | 测试失去判别力，缺陷漏到用户侧 | 中 | ① 变更范围给出明确判据（凡断言执行成功 / 输出内容者必补，写保护拒绝臂不触 `execute` 可不改）；② 意图闸用例自身锚定文案含 `intent`；③ 全量测试门 + `git diff` 复核夹具清单 |
| **R6 前端既有精确断言随版式抉择失效**：`getByText('Bash')` 与单文本节点 `Bash · …` 不兼容 | 前端测试红（显性失败，非静默） | 高（取决于未决项 1） | 交 design 择一并同步断言（内层节点方案零改 / 单文本节点方案改 `/^Bash/`）；无论哪条路，AC-5 / AC-6 都以 `trace-title` 的语义锚点断言，不锁死 DOM 细节 |
| **R7 折叠态文本被误改成命令首行**（实现期「顺手提上来」） | 扫读语义回到「命令前缀摘要」，需求目标落空 | 低 | AC-5 显式断言折叠态文本不含命令原文；D1 / 明确不做双向锚定 |
| **R8 与既有 `timeout_ms` 校验序耦合**：意图闸插错位置（放在 `resolve_timeout_ms` 之后）会让超时类用例先报超时再报意图 | 既有超时断言文案漂移 | 低 | 变更范围钉死插入点（`command` 闸之后、`resolve_timeout_ms` 之前）；AC-2 的「`{}` 报 command」与 AC-4 的超时用例共同锁定 |
