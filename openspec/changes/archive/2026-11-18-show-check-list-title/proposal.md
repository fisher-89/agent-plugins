# 提案: show-check-list-title

> **变更**: show-check-list-title
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/show-check-list-title/explore.md`（一句话需求：finish_step 的 checklist 场景在 id 后显示检查项标题）
> **范围接缝**: 本变更只做**`finish_step` evaluator 形态的「检查项标题」从「只在 prompt 里」搬到「随工具调用载荷走 + 前端可读」**：① 工具参数增 `items[].title`（必填）、② 校验器守标题与 prompt 静态清单表第二列逐字一致、③ 收口卡逐项行呈 `id + 标题`。MUST NOT 做：落库形状与 `ChecklistItem` 字段变更、`eval-checklist` 抽屉渲染改动、prompt md 静态清单表改写、前端静态 id→标题映射表或 md→TS 代码生成、`finish_step` decision 形态改动。

---

## 问题

### 现象：卡片上只有 `R1` / `C1` 这类短码，含义要回 prompt 里查

`finish_step` 收口卡的 evaluator 形态逐项行呈「勾选框 + 项 `id` + `evidence`」（`packages/desktop/src/components/agent/message-cards/blocks.tsx` 的 `CheckListCard`）。`id` 是 prompt 静态清单表的首列（`R1`…`R11` / `C1`…`C10` / `T1`…），本身不自明；它对应的**检查项标题**（「问题描述清晰，包含背景和动机」）只活在 evaluator prompt 的 `## 静态清单` 表第二列里。用户看卡片时要么记得该 prompt 的表，要么去翻 prompt，卡片单独不可读。

### 根因：标题在工具边界被有意丢掉

现状数据面是一条**单向丢弃链**：

| 环节 | 手上有什么 |
|---|---|
| evaluator prompt（md 静态清单） | `id` + 检查项标题 + 判断依据（三列） |
| `finish_step` 工具参数（模型提交） | `{ report, items: [{ id, pass, evidence }] }`——**没有标题** |
| `verdict.rs::validate_checklist` | 拿 `checklist_entries(prompt)` 把 `id` 映射成权威正文，写进落账面 |
| 落账面 `ChecklistItem` | `{ item（权威正文）, pass, evidence }`——**标题在这里，但不含 id** |
| 前端收口卡（`AgentBlock::ToolUse.input`） | 模型提交原文，逐字——**只剩 id** |

即：标题一路都在，只是卡片唯一可见的那一侧（工具调用输入）恰好没有它。前端也拿不到 prompt（收口卡是通用工具卡，输入只有 `input` / `output` 两个位，`output` 在 evaluator 形态按既有 spec 明令不呈），因此**纯前端无解**。

### 硬约束（决定解法形状）

1. **手写真相只有一份**：检查项标题的权威正文唯一来源是 `workflow::model::checklist::checklist_entries(prompt)`（md 静态清单表第二列）。任何「再写一遍标题」的地方都必须是**被校验过的搬运**，或干脆不写。
2. **不自造新约定**：仓库没有「把 Rust 侧 md 表导出成前端常量 / TS 代码生成」的机制，为一行展示文本新起一条代码生成链不划算。
3. **落库面零变更**：`ChecklistItem { item, pass, evidence }` 是落账与 `eval-checklist` 产物线面形状，改动会外溢到 store schema、bindings 与抽屉渲染——本次不碰。
4. **前端只消费工具调用载荷**：收口卡的两形态都是 input 驱动（evaluator 形态明文 MUST NOT 呈工具结果区），把展示数据塞进工具结果会破坏这个不变量。

## 提案

```
今天                                   之后
──────────────────────────────────     ──────────────────────────────────
finish_step items:                     finish_step items:
  { id: "R1", pass: true, evidence }     { id: "R1",
                                           title: "问题描述清晰，包含背景和动机",   ← 新增（必填）
                                           pass: true, evidence }
       │                                      │
       │                                      ├─ validate：title == 静态清单第二列原文？
       │                                      │     否 → Err 当场回灌（文案自足）
       │                                      │     是 → 落账面照旧取权威正文
       ▼                                      ▼
卡片逐项行：R1                         卡片逐项行：R1 问题描述清晰，包含背景和动机
```

一句话：**让标题成为收口协议载荷的一部分（模型搬运、校验器守门），于是前端只靠 input 就能呈 `id + 标题`，而权威性仍锁在 prompt 静态清单那个单点上。**

### 决策表（含被拒备选与理由，逐条自足成证）

| # | 决策 | 要点 | 被拒备选与理由 |
|---|---|---|---|
| D1 | 标题经**工具调用输入**承载（模型提交 + 校验守门） | `items[].title` 必填，`validate_checklist` 与 `checklist_entries(prompt)` 逐字比对，不一致 `Err` 当场回灌；通过校验即意味着卡片显示的标题 == 权威正文 | **被拒：工具结果信封**（后端铸造 `{items:[{id,title}]}`，卡片从 `output.content` 反解，沿 ask 应答信封先例）——信封要进模型上下文（每评估步多一段与 prompt 重复的文本，且历史重放会再喂回一次），且收口前 `output` 缺席 → 标题「先没有、收口后凭空出现」抖动；更关键是它把 evaluator 形态从「input 驱动」改成「input + 结果驱动」，与既有不变量冲突。<br>**被拒：前端静态 id→标题映射**——要么把 8 份 prompt 的表复制进前端（第二份手写真相），要么新起 md→TS 代码生成链（自造约定）。<br>**被拒：新增 `AgentBlock` 变体带结构化 UI 载荷**（web_search 的 `raw_use` / `raw_result` 先例）——为一个标题改 core/agent 事件词汇、specta bindings 与 store 编码，量级远超收益。 |
| D2 | `title` **必填**（进 schema `required`） | 「标题恒在场」成为可断言事实 | **被拒：可选字段**——可选即「有时有有时没有」，展示不可靠且要额外判据；宽容性只留给**历史转录**（见 D5） |
| D3 | 标题不一致即 **`Err` 拒绝** | 沿既有「工具校验 Err 是唯一反馈面、同 turn 内重试」纪律；Err 文案给 `id` + 期望原文 + 当前值 | **被拒：忽略模型值、落账面自动取权威**——落账面本就取权威，问题是**展示面**取的是模型原文；不校验则模型改写的标题直接显示出来，同一标题出现两份可漂移写法。<br>**被拒：不校验但要求模型自拟标题**——卡片上是模型自拟文案，与落账面/抽屉里的权威文案不一致，比今天只显示 id 更糟 |
| D4 | 落账面**逐字零变更** | `ChecklistItem.item` 仍取权威正文；`title` 是纯展示搬运字段，不进载荷新字段、不落库 | **被拒：把 `title` 落进 store**——要动 `ChecklistItemRecord` / `PhaseRecord` 线面与抽屉渲染面，且与 `item` 语义重复 |
| D5 | 历史转录**降级为 id-only** | `title` 缺席（旧会话、旧卡片输入）只呈 id，零回填、零占位符 | **被拒：按 id 反查 prompt 回填**——需前端持有 prompt 表或新增查询面，为旧数据引入第二数据源 |
| D6 | 静态清单表**一字不改** | 8 份 `*-evaluator.md` 零改动；`checklist_entries` 解析规则零改动 | **被拒：把标题并入 id**（如 `R1 问题描述清晰…`）——破坏 `[A-Z]+[0-9]+` ID 契约，API/落账/白名单全链受影响 |

### 明确不做（知情边界，非遗漏）

- **`eval-checklist` 抽屉 / `EvalChecklistRenderer` 不改**：它呈的是落账面 `item`（标题、无 id），本变更不引入「抽屉补 id」。两侧分头演进是既有现状，合并属独立变更。
- **「被拒提交仍按 checklist 形态渲染其原文」的既有行为不修**：一次 `Err` 的提交也会被卡片按 evaluator 形态画出来（含模型写的 title），这是既有形状判据的既然后果；本变更既不放宽校验也不扩范围去修它（见 R3）。
- **卡片交互不加**：不加 tooltip / 折叠 / 按未通过项筛选 / 复制标题。
- **`finish_step` decision 形态与 JSON 回退分支零改动**。
- **`SystemNotice{finish_step}` 载荷不动**：`EvaluatorChecklist { verdict, report, checklist }` 逐字保持。
- **前端无第二套标题来源**：不新增 md 解析、不新增代码生成、不新增查询命令、不新增常量表。

### 未决项（交 design 定稿，写进风险而非范围）

1. **比较归一化口径**：逐字严格，还是允许首尾空白裁剪 / 内部连续空白折叠（不丢字的归一化）。无论哪种，MUST NOT 做「语义等价」模糊匹配。
2. **「逐字原文」要求的落位**：「取本 prompt 静态清单表第二列原文，逐字」写在 schema 的 `title.description`，还是写在 `evaluator_protocol` 附录——只能一处。
3. **卡片版式**：`id` 与标题同处一个 `FieldLabel`（单个空格分隔）还是两个节点；超长标题（最长项约 40 全角字符）的换行策略。

## 能力

### 新增的能力

**无。** 本变更全部落在既有能力面内（前端会话基建的收口卡、编排的 `finish_step` 工具协议与输出协议附录），不引入新能力边界。

### 修改的能力

- **desktop-agent-chat-infra** —
  - **MODIFIED**「finish_step 收口卡呈现」：evaluator 形态逐项行的 `FieldLabel` 由「项 `id`」改为「项 `id` + 项标题」（`title` 为非空字符串时在 id 后以单个半角空格接标题原文；缺席 / 空串时只呈 id，零占位符）；未通过项的破坏性着色覆盖该标签整体；宽容解析扩一位（`title` 非字符串即视为缺席，不抛错、不回退 JSON）；decision 形态、摘要行、`block-tool-result` 缺席、JSON 回退逐字不变。

- **desktop-change-orchestration** —
  - **MODIFIED**「evaluator 输出协议瘦身」：协议附录的逐项字段枚举由 `id` + `pass` + `evidence` 扩为 `id` + `title` + `pass` + `evidence`（`title` 指明取 prompt「静态清单」表第二列原文）。
  - **ADDED**「checklist 项标题的搬运与权威校验」：evaluator 形态 schema 增必填 `items[].title`；`validate_checklist` 以 `checklist_entries(prompt)` 第二列正文比对（不一致 `Err`，文案自足：`id` + 期望原文 + 当前值，同 turn 内重试）；落账面 `ChecklistItem` 三字段与 `item` 取权威逐字不变；MUST NOT 在 orchestration / 前端新增 prompt 解析或标题常量副本。

### 其余能力零改动

- **desktop-phase-prompts**：8 份 `*-evaluator.md` 的 `## 静态清单` 三列表逐字零改动（本变更消费该表，不改写它）。
- **desktop-agent-execution**：挂载面（`mount.rs`）、`ToolTier::Protocol`、loop 收口三纪律、`RunStarted.tools` 语义零改动（`finish.rs` 属 orchestration）。
- **desktop-change-state-store / desktop-change-queries**：`ChecklistItemRecord` 形状与 checklist 线面三字段零变更。
- **desktop-artifact-plugins / desktop-change-flow-view**：`eval-checklist` 信封与列头抽屉渲染零改动。
- **desktop-ipc-type-bindings**：`SessionInjections` / `FinishSpec` / `ChecklistItem` 均不出线 IPC，`bindings:export` 再生成后零 diff。
- **desktop-write-protection / desktop-file-read / desktop-agent-db-tools**：零介入（`finish_step` 不触路径沙箱与写入保护闸）。
- **desktop-crate-layout / repo-archi-model**：零新 crate、零新元素、零新依赖边。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/orchestration/src/finish.rs` | `parameters(FinishSpec::EvaluatorChecklist, prompt)`：`items.items.properties` 增 `title`（`type: "string"`，description 指路「本 prompt 静态清单表第二列原文，逐字」——若「未决项 2」裁定由 prompt 附录承载，则此处只留取值的机械说明）；`items.items.required` 由 `["id","pass","evidence"]` 改为 `["id","title","pass","evidence"]`。`description(FinishSpec::EvaluatorChecklist)` 的逐项字段说明按同一裁定同步（单点，不得与 schema / prompt 两处各写一份）。decision 形态 schema 与两形态 `validate` 分派逐字不变 |
| `crates/core/orchestration/src/verdict.rs` | `validate_checklist(args, prompt)`：逐项读 `title`（缺失 / 非字符串 → Err，文案自足）；与 `entries`（`checklist_entries(prompt)`，按 `id` 命中）的第二列正文比对，不一致 → Err（文案含项 `id`、期望原文、当前值）；`ChecklistItem` 映射与 `report` 上限、`derive_verdict` 推导逐字不变（`item` 仍取权威正文，`title` 不入载荷） |
| `crates/core/orchestration/src/prompt.rs` | `evaluator_protocol()` 的逐项字段枚举增 `title`（"checklist 逐项需 `id` + `title`（同表第二列原文）+ `pass` + `evidence`" 口径）；「协议形状不进 prompt 主体」的既有约束按「未决项 2」裁定表述（形状仍在工具描述面，prompt 侧只留取值纪律） |
| `packages/desktop/src/components/agent/message-cards/blocks.tsx` | `FinishItem` 增 `title: string \| null`（经 `textOf` 宽容读，空串与 `null` 同作缺席）；`CheckListCard` 逐项 `FieldLabel` 呈 `id` 与标题（title 缺席只呈 id）；其余结构（`Checkbox` / `FieldDescription` / `data-testid` / memo 包装）零改动 |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 +1 patch**（当前盘面 `0.6.51` → 落地时以 `git diff HEAD -- packages/desktop/package.json` 的版本行判「恰好 +1」为准；MUST NOT 写死字面号）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| `title` 与静态清单第二列逐字一致 → `Ok`，且 `EvaluatorChecklist.checklist[i].item` 仍为权威正文（`title` 不出现在载荷 / 落账面） | `crates/core/orchestration/src/verdict_test.rs` | 改造（既有合规输入夹具补 `title`；新增「一致即通过」用例） |
| `title` 与权威正文不一致 → `Err`，文案含项 `id` + 期望原文 + 当前值；`title` 缺失 / 非字符串 → `Err`，与「ID 缺项 / 清单外 ID」三类文案分列 | `crates/core/orchestration/src/verdict_test.rs` | 新增（不一致 / 缺失 / 非字符串三形态） |
| `parameters(EvaluatorChecklist, prompt)` 的 `items.items.properties.title.type == "string"`、`required` 含 `title`；decision 形态 schema 逐字不变（既有断言回归） | `crates/core/orchestration/src/finish_test.rs` | 新增断言 + 既有断言零改 |
| `evaluator_protocol()` 文本含 `title` 字段枚举；既有断言（含「调用 `finish_step` 收口」「不含 verdict/phase/attempt/skipped」「report 上限」）零改 | `crates/core/orchestration/src/prompt_test.rs` | 新增断言 |
| evaluator 形态逐项行 `FieldLabel` 文本为 `id + 空格 + 标题`；未通过项该标签带破坏性着色（标题一并着色）；`title` 缺席 / 空串 / 非字符串 → 只呈 id、不抛错、不回退 JSON；摘要行与 `evidence` 行零变化 | `packages/desktop/src/components/agent/message-cards/blocks.test.tsx` | 改造 `FINISH_EVALUATOR_INPUT` 夹具补 `title` + 新增降级用例 |

### 不要修改

- **`crates/core/workflow/src/model/checklist.rs`**：`checklist_entries` 已是「ID + 权威人读文案」唯一解析实现（含空列行丢弃规则），本变更零改动——该文件出现 diff 即说明实现另起了第二解析面。
- **`crates/core/workflow/src/model/domain.rs`**：`ChecklistItem { item, pass, evidence }` 三字段逐字不动。
- **`crates/core/workflow/src/prompts/*-evaluator.md`（8 份）**：静态清单三列表零改动。
- **`crates/infra/store/**`**：`ChecklistItemRecord` / `PhaseRecord` 落库形状零变更。
- **`crates/core/orchestration/src/ask.rs`**：零改动（`finish_step` 与 `AskUserQuestion` 的零互引守卫仍须可 `grep` 验证）。
- **`crates/infra/agent/src/sdk/{mount,loop,policy,tools}.rs`**：零改动（挂载面、收口三纪律、执行分派皆不涉标题）。
- **`packages/desktop/src/components/agent/message-cards/tool-cards.tsx`**：零改动（`finish_step` 注册键与 `BUILD_IN_TOOL_CARDS` 键面不变）。
- **`packages/desktop/src/lib/agent-adapter.ts` / `src/views/changes/renderers/**`**：零改动（收口卡走既有工具部件管线；`eval-checklist` 抽屉不补 id）。
- **`openspec/changes/show-check-list-title/explore.md`**：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | **卡片呈 `id` + 标题**：输入 `{ report, items: [{ id: "R1", title: "问题描述清晰，包含背景和动机", pass: true, evidence }, { id: "R2", title: "…", pass: false, evidence }] }` 展开卡后，逐项 `FieldLabel` 文本依次为 `R1 问题描述清晰，包含背景和动机` 与 `R2 …`（id 在前、单个半角空格分隔、标题原文不截断）；R2 的该标签带 `text-destructive` 且标题一并着色；摘要行仍为「检查 1 项未通过」、`evidence` 仍在 `field-description` 位、零 `block-tool-result` 节点 | `blocks.test.tsx`（evaluator 形态用例改造 + 断言标签全文与类名） |
| AC-2 | **历史转录降级**：`items` 元素缺 `title` / `title` 为空串 / `title` 为非字符串（`null`、数字、对象）三种输入下，逐项行只呈 id，无占位符、无 `undefined` / `null` 文本、不抛错、不回退 JSON（仍为 checklist 形态） | `blocks.test.tsx`（参数化三形态） |
| AC-3 | **schema 形态**：`parameters(FinishSpec::EvaluatorChecklist, prompt)` 的 `items.items.properties.title.type == "string"`、`items.items.required` 逐值等于 `["id","title","pass","evidence"]`；同函数 `FinishSpec::Decision` 臂返回值与改动前逐字相同（既有断言零改即证） | `finish_test.rs` |
| AC-4 | **权威守门（正向）**：`title` 与 prompt 静态清单第二列逐字一致时 `validate_checklist` 返回 `Ok`，且 `EvaluatorChecklist.checklist` 每项 `item` 为该权威正文、序列化后的载荷不含 `title` 键；`verdict` 仍由 `derive_verdict` 推导 | `verdict_test.rs` |
| AC-5 | **权威守门（反向）**：`title` 被改写（如「问题描述清晰」）时 `Err`，文案含该 `id`、期望原文与当前值；`title` 缺失或非字符串时 `Err`，文案点明字段名与期望；两类 Err 均与既有「ID 缺项 / 清单外 ID」文案分列可辨 | `verdict_test.rs`（三形态逐条断言文案片段） |
| AC-6 | **协议文本面**：`evaluator_protocol()` 文本含 `title` 的逐项字段枚举；既有断言（含「调用 `finish_step` 收口」、不含 `verdict` / `phase` / `attempt` / `skipped` 说明、`report` ≤ `MAX_REPORT_CHARS`）零改；「逐字原文」要求全文只在一处出现（`title.description` 或附录之一，`grep` 计数为 1） | `prompt_test.rs` + `grep` 复核 |
| AC-7 | **回归不破**：decision 形态摘要（backtrack / stop / ask / 未知动作）、JSON 回退分支、`BUILD_IN_TOOL_CARDS` 键面与 `ToolPairCard` 回退、`ask.rs` 全文零 `finish` 字样、落账面 `ChecklistItem` 三字段与 `eval-checklist` 产物形状——全部零变化 | 既有用例零改即绿 + `grep` 复核 |

> **验收表口径**：AC-1…AC-7 是能力验收面（每条对应 `specs/` 下某个 delta requirement 的行为判据）。静态检查链、全量测试、版本递增、`bindings:export` 零 diff 是全仓既有交付门，由主基线承载，不进本表。

### 验证命令

```powershell
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration
pnpm -C packages/desktop exec vp test run src/components/agent/message-cards/blocks.test.tsx
```

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 模型逐字搬运失败触发回灌重试**：标题最长约 40 全角字符，模型偶发改写 / 省字 | 每次 `Err` 多一轮模型请求（同 turn 内自纠，步不失败），评估步成本与延迟上升 | 中 | ① Err 文案给 `id` + 期望原文 + 当前值，一次即纠；② 「逐字」要求单点承载（AC-6 以 `grep` 计数守住），避免两处措辞不一造成二次误导；③ 归一化口径交 design 定稿（可裁为 trim / 折叠空白，MUST NOT 丢字模糊匹配）；④ 实测重试率偏高时，可在不改协议的前提下放宽归一化——不回退到「不校验」 |
| **R2 模型输出体量增长**：每项多一列标题（8–11 项，约 150–400 字符） | evaluator 步输出 token 与时长小幅上升 | 高（必然） | 体量与既有 `evidence` 同量级，占评估步总输出的小头；明确不做「标题也进 evidence」类叠加 |
| **R3 被拒提交的卡片仍按 checklist 形态呈其原文**（既有行为） | 一次 `Err` 的提交会在卡上显示模型写的（未通过校验的）标题，可能被误读为权威 | 中 | 明确不做项；`AC-1` 断言的是通过形态；实现期不扩范围。若后续要修，方向是「`output.isError` 时摘要行带 `error` 状态」独立变更 |
| **R4 长标题挤压布局**：逐项行单行容纳 `id + 40 全角字符` | 窄屏换行 / 与 `evidence` 视觉拥挤 | 中 | `FieldLabel` 允许多行、不截断（MUST NOT 自行省略号截断——截断会让标题失去可读性）；版式细节交 design（未决项 3） |
| **R5 历史会话与新会话呈现不一致** | 旧会话卡片只有 id，用户以为功能没生效 | 低 | 显式决策 D5：不回填、不加「（无标题）」类占位；spec 明写降级语义（旧会话仍完整可读，只是标签更短） |
| **R6 `title` 被误当落账字段** | 落库 / 抽屉 / bindings 面被牵连，范围失控 | 低 | spec 与 AC-4 双向锚定：载荷序列化不含 `title`、`ChecklistItem` 三字段零变；`crates/infra/store/**` 进「不要修改」清单（出现 diff 即越界） |
