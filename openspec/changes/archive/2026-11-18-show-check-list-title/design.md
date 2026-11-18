# 设计: show-check-list-title

> **变更**: show-check-list-title
> **状态**: 设计定稿（基于已定稿 proposal + 2 份 delta spec）
> **基于**: `proposal.md`（问题 / 提案 / D1–D6 / 变更范围 / AC-1…AC-7 / R1–R6）、`explore.md`（一句话需求）与 `specs/{desktop-agent-chat-infra,desktop-change-orchestration}/spec.md`
> **性质**: `finish_step` evaluator 形态的**检查项标题**从「只在 prompt 静态清单里」搬到「随工具调用载荷走、校验器守门、前端只靠 `input` 就能呈 `id + 标题`」。改动面只有四处源码（`finish.rs` / `verdict.rs` / `prompt.rs` / `blocks.tsx`）+ 一行版本号。
>
> **本篇边界**: 钉住 proposal 三个未决项（比较归一化口径 / 取值纪律单点落位 / 卡片版式）并给实现形态；逐条 AC 给落点。**零新增 crate、零新增依赖边、零架构模型改动、零 store 改动、零 IPC 出线变更**（`finish_step` 是工具面，不进 `bindings:export`）。

---

## 提案与规格同步状态

proposal 的 D1–D6 六条裁决与 2 份 delta spec 均已定稿，本设计不重复其论证，只做三件事：

1. **把 spec 已钉语义、未钉落点的三处收敛到具体形状**（见「关键设计决策」DD1–DD7）：取值纪律的**单点落位**、与权威正文的**比较归一化口径**、**校验顺序**（决定既有三类 `Err` 文案的可达性与夹具面）、前端**呈现位与版式**。
2. **核实「零 store 改动也能让新会话卡片可读」的机制依据**（见「起点核实」与「数据模型」）：`title` 随 `AgentBlock::ToolUse.input` 原文进会话转录——store 对这一列是不透明 JSON，故零 schema 认知、零迁移、零 backfill。
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略章节（用例编写与门禁执行由独立相位承接）。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **协议面 `crates/core/orchestration/src/finish.rs`**：`description(FinishSpec)` `:33-47`（evaluator 臂 `:35-40` 逐项字段说明写作「逐项给 pass 与 evidence」）；`parameters(spec, prompt)` `:51-113`（evaluator 臂 `:53-106`：`id_schema` `:54-57` 带 `description`「取本 prompt「静态清单」表首列」+ `:62-64` 条件追加 `enum`；`items.items.properties` `{id, pass, evidence}` `:77-95`；`required: ["id","pass","evidence"]` `:96`；decision 臂 `:107-112`）；`validate(spec, args, prompt)` `:117-123` 仅两臂分派。
- **校验面 `crates/core/orchestration/src/verdict.rs`**：`validate_checklist(args, prompt)` `:26-141`——形状（`report` `:28-31` / `items` `:32-38` / 空数组 `:39-41`）→ `entries` 非空守卫 `:42-49` → 逐项形态 `id`/`pass`/`evidence` 收集 `:52-87`（`passing: Vec<(String, bool, String)>` `:52`）→ 缺项 `:90-99` → 清单外 ID `:100-108` → `report` 上限 `:108-115` → `id → item` 映射 `:116-126`（`item: entries.iter().find(...)` `:119-124`）→ `derive_verdict` `:128` → 装配 `Ok(EvaluatorChecklist { … })` `:129-134`。
- **prompt 面 `crates/core/orchestration/src/prompt.rs`**：`evaluator_protocol()` `:46-56`，第 3 条逐项字段枚举在 `:51`（现为 `id` + `pass` + `evidence` 三项）；`evaluator_prompt` `:58-61` 是唯一 append 点。
- **权威源 `crates/core/workflow/src/model/checklist.rs`**：`checklist_entries(prompt)` `:26-53`（只扫 `## 静态清单` 段、行首 `| <ID> |`、按序去重）；`parse_row` `:55-84` 的第二列以 `columns.split('|').next()?.trim()` 取（`:71`）——**权威正文已 trim**，这是 DD2 归一化口径的直接依据。
- **前端收口卡 `packages/desktop/src/components/agent/message-cards/blocks.tsx`**：`FinishItem` `:301-305`（`{ id, pass, evidence }`）；`isCheckListFinish` `:335-338`（判据 = `report` 为字符串 且 `items` 为数组，**不校验元素形态**）；`CheckListCard` `:340-392`（`failedCount` `:347`、摘要行 `:357`、逐项 `Field` / `Checkbox` / `FieldLabel` `:372-374`（子节点仅 `{item.id}`）/ `FieldDescription` `:375`）；`FinishStepCardBase` `:393-437`（decision 形态 + JSON 回退）；`export const FinishStepCard = memo(FinishStepCardBase)` `:439`。
- **前端解析惯例 `packages/desktop/src/lib/parse.ts`**：`asRecord` `:11-28`（数组 / 标量归空对象）、`textOf` `:30-33`（非字符串 → `null`，空串原样返回 `''`）——DD5 的宽容读取口径即这两个既有件的组合。
- **`FieldLabel` 版式 `packages/desktop/src/components/ui/field.tsx`**：`FieldLabel` `:99-118` 以 `Label` 为载体，合并类为 `flex w-fit gap-2 leading-snug`（`Label` 本体 `packages/desktop/src/components/ui/label.tsx` 为 `flex items-center gap-2 text-sm leading-none font-medium`）——**无 `truncate` / `line-clamp-*` / `overflow-hidden` / `text-ellipsis`**，容器宽度不足时文本按默认规则折行，故 DD6 的「不截断」是既有类的既然后果，不需要新增类。
- **校验与 schema 同源**：`crates/infra/agent/src/sdk/loop.rs:222` 以 `mount::definitions(&turn.injections, &turn.question)` 铸造工具定义、`:691` 以 `finish::validate(spec, &input, &turn.question)` 校验——**同一份 `turn.question` 字符串**既是模型眼前的 prompt（含静态清单表），也是 `checklist_entries` 的解析输入，故「校验的权威正文 == 模型可读到的那份表」天然自一致（零新数据源）。
- **转录落库面**：`crates/infra/store/src/model.rs:559-600` 把 `AgentEvent` 整体序列化为不透明 JSON 存列（`AgentBlock::ToolUse { id, name, input: serde_json::Value }`，`crates/core/agent/src/event.rs:62-67`）——`title` 随 `input` 原文落库，**零 schema 认知**（这是「零 store 改动」的机制依据）。
- **版本**：`packages/desktop/package.json:3` 现为 `0.6.51`（落地时以 `git diff HEAD -- packages/desktop/package.json` 的版本行判「恰好 +1 patch」）。

---

## 关键设计决策

> proposal 的 D1–D6（承载位 = 工具输入 / `title` 必填 / 不一致即 `Err` / 落账零变更 / 历史降级 id-only / 静态清单表一字不改）已定稿，本篇不重复。下表 DD1–DD7 收敛 proposal 三个未决项与两处 spec 未钉的形状。

| # | 议题 | 定稿 | 理由（为什么另一种写法会坏） |
|---|---|---|---|
| DD1 | 「逐字原文」取值纪律的**单点落位** | 落在 `finish.rs::parameters` 的 **`title.description`**：「检查项标题：取本 prompt「静态清单」表第二列「检查项」原文，逐字照抄（改写 / 省字会被拒绝）」。`prompt.rs` 的附录**只留字段指针**「`title`（同表第二列「检查项」）」，`description(FinishSpec::EvaluatorChecklist)` **只列字段名**「逐项给 title、pass 与 evidence」，`verdict.rs` 的 `Err` 文案**只陈述事实**（`id` + 期望原文 + 当前值），三处均不出现「逐字」 | ① 与既有 `id` 完全同形——`id` 的来源说明已经写在 `id.description`（「取本 prompt「静态清单」表首列」），把 `title` 的来源说明写到 `title.description` 是零自造约定；② 若写进 prompt 附录，则「形状归 schema、prompt 只留纪律」的既有分野被拆成「纪律也在 prompt」，「逐字」与 schema 里的字段说明必然各写一份措辞，两句一旦分叉，模型按哪句都算「照协议做」；③ 若三处都写，AC-6 的 `grep` 单点判据直接失败 |
| DD2 | 与权威正文的比较**归一化口径** | **仅首尾 `trim`**：`title.trim() == entry.item`（Rust `str::trim`，Unicode 空白）。**不做**内部连续空白折叠、**不做**大小写 / 全半角 / 标点归一、**不做**相似度判定 | ① 权威正文在 `parse_row` 里已被 `trim()`（`checklist.rs:71`），故首尾空白在权威侧不承载任何信息，裁剪它不丢字；② 反面：现有清单标题里**内部空白承载语义**（`proposal 的每个验收标准（目标）均已达成`、`` `单元测试 > 用例` 表格每行已生成对应测试骨架 ``——拉丁词、反引号与通配符之间的空格是正文的一部分），折叠内部空白即改写语义，与 spec「MUST NOT 退化为语义等价」冲突；③ 归一化规则必须**单点**（一个比较表达式），MUST NOT 逐项各写 |
| DD3 | 校验**顺序** | 既有链不变并**在末尾追加**：形状（`report` / `items` / 空数组）→ `entries` 非空 → 逐项形态（`id` / `pass` / `evidence`）→ 缺项 → 清单外 ID → `report` 上限 → **`title` 形态 + 逐字比对** → `id → item` 映射 | ① 「清单外 ID」的项**没有权威正文可比**——`title` 校验若前置，一个错 ID 会先撞上「无期望值」的错，既有三类 `Err` 文案（未知 / 缺项 / 清单外）的可达性被破坏（AC-5 要求四类文案分列可辨）；② `title` 读在收集段而校验在末尾，意味着收集段零 `Err`，改动是纯追加（既有 `passing` 只多一位），没有既有分支被重排；③ 一次一错（同 turn 内自纠），不做「汇总多项不一致」——汇总会拉长 `Err` 文案而收益为零（改一处后重提仍会再报下一处，回合数不变） |
| DD4 | `Err` 文案**两条分列** | ① 缺失 / 非字符串 / trim 后空串：`字段 items[<i>].title 缺失 / 非字符串 / 空串：期望本 prompt「静态清单」表第二列原文，当前 <原值>`；② 不一致：`字段 items[<i>].title 与静态清单第二列不一致（ID <id>）：期望「<权威正文>」，当前「<trim 后 title>」`。两条与既有「`items[<i>].id` 未知」「`items[].id` 缺项」「`items[].id` 含清单外 ID」**措辞互斥** | spec 要求 `title` 缺失 / 非字符串「单独成一条 `Err` 文案」且与三类 ID 文案分列可辨——两条各自点名字段路径 `items[<i>].title` 与分类词（`缺失` / `不一致`），ID 三类则恒带 `items[].id` 或 `items[<i>].id` 与 `未知` / `缺项` / `清单外`；② 把 `<id>` 放在括号里而非文案开头，避免与「未知 ID」文案的首字段名混淆 |
| DD5 | 前端**宽容读取的落点** | 只在呈现点做一次性读取：新增模块私有 `itemTitle(value: unknown): string \| null`（`textOf` + 空串归 `null`），逐项调用 `itemTitle(item.title)`；**不把 `items` 归一化成新数组**，`pass`（真值判定）/ `evidence`（真值判定）的既有读取路径**零改动** | ① 归一化整个 `items` 会把 `pass` / `evidence` 的读取一并搬进新函数——那里现为「非布尔按真值」「`evidence` 真值即渲」的宽容语义（既有用例 `pass: 'true'` / `pass: 1` 断言真值判定），搬迁即引入一个本变更不必承担的行为面（AC-7 要求这两行零变化）；② 只读 `title` 一处，`FinishItem.title` 的类型即「已归一化」的声明，而运行时靠 `textOf` 兜底——与同文件 `decisionSummary` 用 `textOf(record.to)` 读未校验字段的既有惯例同形 |
| DD6 | 卡片**版式** | **单个** `FieldLabel` 节点，文本为 `` title === null ? item.id : `${item.id} ${title}` ``（单个半角空格）；`className` 仍只有条件类 `text-destructive`，**不新增** `truncate` / `line-clamp-*` / `overflow-hidden` / `text-ellipsis` / `max-w-*`；显示值**不 trim、不折叠、不截断** | ① 未通过项的破坏性着色必须覆盖「`id` 与标题整体」——分两个节点则要两处类名（第二处漏写即半截着色），单文本节点是唯一不可能漏的形态；② AC-1 断言的正是「该标签文本为 `id + 空格 + 标题`」，单节点天然满足；③ `FieldLabel` 既有类里没有任何截断类（见「起点核实」），长标题按容器宽度折行即满足 R4——补一个 `truncate` 反而直接违反 spec「MUST NOT 截断」；④ 显示值不 trim：trim 属改写，`title` 为空串/纯空白只有在被拒提交或历史载荷里才可能（新提交经校验层 trim 后为空即 `Err`），视觉上等价 id-only |
| DD7 | 版本行与生成物 | `packages/desktop/package.json` 的 `version` 相对**落地时 `HEAD` 现值 +1 patch**（当前盘面 `0.6.51` → 期望 `0.6.52`，不写死字面号）；`bindings:export` **零 diff**（`finish_step` 是工具面，`FinishSpec` / `ChecklistItem` 不出线 IPC） | 版本纪律由 AGENT.md 钉（`tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` crate 版本行不跟）；写死字面号会在并发变更下撞号 |

**AC-6 的 `grep` 判据口径（DD1 的可机械验证形式）**。判据的对象是**模型可见文本**（两处描述字符串 + 协议附录的字符串字面量），不含源码注释——`orchestration/src` 里既有大量「逐字不变」「逐字保真」类注释与用例名（`ask.rs` / `ask_test.rs` / `control_test.rs` / `prompt.rs:110` 等），它们讲的是别的事，不在取值纪律的判据内。三条命令：

```powershell
grep -c "逐字" packages/desktop/src-tauri/crates/core/orchestration/src/finish.rs            # == 1（唯一处 = title.description）
sed -n '/fn evaluator_protocol/,/^}/p' packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs | grep -c "逐字"   # == 0
grep -c "逐字" packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs           # == 0（Err 文案只陈述事实）
```

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 收口工具协议面（**修改**） | evaluator 形态 `items[].properties` 增 `title`（`type: "string"`）+ 进 `required`；`title.description` 承载「取静态清单第二列、逐字照抄」的**唯一**取值纪律；`description()` 的逐项字段说明只列字段名 | `crates/core/orchestration/src/finish.rs` | `serde_json`（既有）、`workflow::model::checklist_entries`（既有） | 纯函数、零 IO；`id.enum` 派生规则零改动 |
| checklist 校验器（**修改**） | 在既有链末尾追加「`title` 形态 + 与 `checklist_entries(prompt)` 第二列逐字比对（仅首尾 trim）」；`Err` 两条分列自足；`ChecklistItem` 映射仍取权威正文、`title` 映射后丢弃 | `crates/core/orchestration/src/verdict.rs` | `workflow::model::{checklist_entries, derive_verdict, ChecklistItem, Verdict}`（既有） | 纯函数、零 IO；`serde_json::Value` 原值只用于 `Err` 文案 |
| 输出协议附录（**修改**） | `evaluator_protocol()` 第 3 条逐项字段枚举由三项扩为四项（`id` / `title` / `pass` / `evidence`），`title` 只写字段指针「同表第二列「检查项」」 | `crates/core/orchestration/src/prompt.rs` | `crate::verdict::MAX_REPORT_CHARS`（既有） | 静态字符串拼装（`format!` 单点） |
| 前端收口卡 evaluator 形态（**修改**） | `FinishItem` 增 `title`；逐项 `FieldLabel` 呈 `id`（`title` 在场时追加空格 + 原文），未通过项的 `text-destructive` 覆盖整标签；`title` 缺席 / 空串 / 非字符串降级 id-only | `packages/desktop/src/components/agent/message-cards/blocks.tsx` | `@/lib/parse` 的 `textOf`（既有）、`@/components/ui` 的 `Field*`（既有） | React + memo（既有卡形态，零新依赖） |

<!-- 零新增 crate、零新增模块、零新增架构模型元素：四个文件都落在既有元素 `App.Backend.Core.Orchestration`（`crates/core/orchestration/`）与前端 `message-cards` 目录内，`.dev-team/architecture/models/**` 零改动。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀；前端侧写全 `packages/desktop/…`。测试文件与用例归测试相位，本清单只列**实现文件**，另设「影响传导」段登记必须随必填扩位同步的夹具点位。

### 新增文件

**无。** 本变更全部落在既有文件内（每个组件都是既有模块的一处扩位，没有独立的新职责单元）。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/orchestration/src/finish.rs` | ① `parameters` 的 evaluator 臂：`items.items.properties` 在 `id` 之后增 `"title": { "type": "string", "description": "检查项标题：取本 prompt「静态清单」表第二列「检查项」原文，逐字照抄（改写 / 省字会被拒绝）" }`；`items.items.required` 由 `["id","pass","evidence"]` 改为 `["id","title","pass","evidence"]`；`report` / `items`（`type` / `minItems` / `description`）、顶层 `required`、`id.enum` 派生与 `:62-64` 的空集省略行为逐字不变。② `description(FinishSpec::EvaluatorChecklist)` 的逐项字段说明由「逐项给 pass 与 evidence」改为「逐项给 title、pass 与 evidence」（**只列字段名**，不复述取值纪律，DD1）。③ decision 臂（`parameters` 与 `description`）与 `validate` 分派逐字不变 | AC-3 / AC-6；`FinishPayload` / `TOOL_NAME` / 模块 doc 零改动 |
| `crates/core/orchestration/src/verdict.rs` | ① `passing` 元素由 `(String, bool, String)` 扩为 `(String, bool, String, Value)`（第四位 = `title` 原值，供 `Err` 文案给「当前值」）；收集段其余分支零改。② 在 `report` 上限检查之后、`checklist` 映射之前新增 `title` 校验段（DD2 / DD3 / DD4）：逐项读 `title` → 缺失 / 非字符串 / trim 后空串 → `Err`（文案①）；否则与 `entries` 中同 `id` 的 `item` 以 `title.trim() == entry.item` 比对 → 不一致 → `Err`（文案②）；`entries` 命中由上游 ID 集合校验保证，**MUST NOT** 用 `unwrap_or_default()` 兜空串（否则文案会出现「期望「」」的自相矛盾期望值）。③ `checklist` 映射的闭包解构（三元素元组）补一位，`item` 仍取权威正文（`title` 映射时丢弃）；`report` 上限、`derive_verdict`、既有三类 ID `Err` 文案、`shape` 段落逐字不变。④ 模块 doc 与 `validate_checklist` doc 补一句「`title` 是展示搬运字段：只用于校验与卡片呈现，映射时丢弃，落账面与载荷零新字段」 | AC-4 / AC-5 / AC-7；`MAX_REPORT_CHARS` 与 `EvaluatorChecklist` 类型零改动 |
| `crates/core/orchestration/src/prompt.rs` | `evaluator_protocol()` 第 3 条：`checklist 逐项需 \`id\`（取自本 prompt「静态清单」表首列）+ \`title\`（同表第二列「检查项」）+ \`pass\`（布尔）+ \`evidence\`（事实依据，含 file:line）`；**不写**「逐字」类措辞（单点在 `title.description`，DD1）；第 1 / 2 / 4 条与 `MAX_REPORT_CHARS` 插值、`evaluator_prompt` / `decision_prompt` / 归档 prompt 逐字不变 | AC-6；判据 = `fn evaluator_protocol` 函数体内 `逐字` 零命中（见 DD 表下的判据口径） |
| `packages/desktop/src/components/agent/message-cards/blocks.tsx` | ① `FinishItem` 增 `title: string \| null`（doc：工具调用载荷原文；缺席 / 空串 / 非字符串 → `null`）。② 新增模块私有 `function itemTitle(value: unknown): string \| null`（`textOf` 读取，`null` 或空串统一归 `null`）。③ `CheckListCard` 逐项：`const title = itemTitle(item.title);` → `<FieldLabel className={\`${item.pass ? '' : 'text-destructive'}\`}>{title === null ? item.id : \`${item.id} ${title}\`}</FieldLabel>`（DD5 / DD6）。④ 其余逐字不变：`failedCount` / 摘要行文案 / `Checkbox` / `FieldDescription`（`item.evidence && …`）/ `Field` / `FieldGroup` / `TraceCollapsible*` / `data-testid`（`block-tool-use` / `data-tool-id` / `block-finish-step-summary` / `block-finish-step-item`）/ `isCheckListFinish` 判据 / `FinishStepCardBase` 的 decision 形态与 JSON 回退 / `memo` 包装 | AC-1 / AC-2 / AC-7；零新增导出（`itemTitle` 私有不导出，避免 knip 死代码污染） |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 +1 patch**（当前 `0.6.51` → 期望 `0.6.52`，不写死字面号，DD7） | `src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随（零改动）；`src-tauri/Cargo.toml` crate 版本行**不跟** |

### 影响传导（必填扩位的夹具点位；实现相位不改测试文件，交测试相位）

> 本变更**不产生编译期破坏**（`title` 是 JSON 字面量字段、前端是可选呈现位），故无「编译闭合批」；但是「必填」使若干既有夹具在**运行期**不再合法。实现相位不改测试文件，只在此登记，由测试相位按 proposal「测试文件」表同步。

| 面 | 点位（实测） | 传导后果（用例与断言的设计归 proposal「测试文件」表，本相位零动作） |
|----|----|----|
| `crates/core/orchestration/src/verdict_test.rs` | 全部合规夹具经 `item_args(id, pass, evidence)` 构造，无 `title` | 追加校验段后这些夹具一律 `Err`——夹具需补 `title` 才能继续表达「合法入参」 |
| `crates/core/orchestration/src/finish_test.rs` | `parameters的evaluator臂结构六断言` 断言 `required == ["id","pass","evidence"]`；`valid_checklist_args()` 无 `title` | `required` 断言与必填扩位矛盾；`valid_checklist_args()` 不再合法 |
| `crates/core/orchestration/src/prompt_test.rs` | 既有断言（工具收口 / 不含 `verdict`·`phase`·`attempt`·`skipped` / `report` 上限）与改动后的附录不冲突 | 无需补位（断言面零改即可继续成立） |
| `crates/infra/agent/src/sdk/loop_test.rs` | `finish_args()` `:5435-5443` 与 `finish_step的err文案三要素可见` 的合法清单夹具（`:5923-5929`）无 `title` | 两类夹具需补 `title`；`:5914` 的「清单外 ID」用例不受影响（DD3 保序：清单外判定先于 `title` 校验即已 `Err`） |
| `packages/desktop/src/components/agent/message-cards/blocks.test.tsx` | `FINISH_EVALUATOR_INPUT` `:413-419` 两项无 `title` | 现有断言（标签文本 == `id`）仍成立——`title` 缺席走 id-only 降级路径；夹具补 `title` 后可断言新形态 |
| `packages/desktop/src/components/agent/message-cards/tool-cards.test.tsx` | evaluator 形态夹具无 `title`（只断言行数 / 不抛错 / 摘要行） | **零影响**（`title` 缺席即 id-only） |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名（参数名、类型、返回类型） | 说明 |
|--------|----------|-----------|------|------|
| `description` | `crates/core/orchestration/src/finish.rs` | 修改（签名不变） | `pub fn description(spec: FinishSpec) -> &'static str` | evaluator 臂逐项字段说明列 `title`（只列字段名，取值纪律单点在 `title.description`）；decision 臂文本逐字不变；两臂仍各 <500 字符 |
| `parameters` | 同上 | 修改（签名不变） | `pub fn parameters(spec: FinishSpec, prompt: &str) -> serde_json::Value` | evaluator 臂 `items.items.properties.title = { type: "string", description: <DD1 单点措辞> }`、`items.items.required = ["id","title","pass","evidence"]`；`id.enum` 派生与空集省略、decision 臂逐字不变 |
| `validate_checklist` | `crates/core/orchestration/src/verdict.rs` | 修改（签名与返回类型不变） | `pub fn validate_checklist(args: &serde_json::Value, prompt: &str) -> Result<EvaluatorChecklist, String>` | 末尾追加 `title` 形态 + 逐字比对两段（DD2 / DD3）；`Err` 两条分列自足（DD4）；`ChecklistItem` 映射（`item` 取权威、`title` 丢弃）、`report` 上限、`derive_verdict` 不变 |

<!-- `finish.rs::validate`（`pub fn validate(spec: FinishSpec, args: &serde_json::Value, prompt: &str) -> Result<FinishPayload, String>`）签名与实现逐字不变（仍只是两臂分派、无 `title` 相关代码），故不列条目——`title` 校验经 `validate_checklist` 间接生效。前端零新增 / 零改动的模块级导出面：`FinishStepCard`（`export const FinishStepCard = memo(FinishStepCardBase)`）签名与引用稳定性零变；`itemTitle` 为模块私有函数（不导出，AGENT.md 的 no-test-only-exports 纪律）。无新增 CLI 子命令、无新增 HTTP 端点、无新增 IPC 命令。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `FinishItem`（interface，私有） | `packages/desktop/src/components/agent/message-cards/blocks.tsx` | 修改 | 由 `{ id: string; pass: boolean; evidence: string \| null }` 扩为 `{ id: string; title: string \| null; pass: boolean; evidence: string \| null }`；`title` 的 `null` 语义 = 「缺席 / 空串 / 非字符串」（DD5）；不导出 |
| `CheckListInput`（interface，私有） | 同上 | **未变** | 仍为 `{ report: string; items: FinishItem[] }`；`isCheckListFinish` 判据零改（MUST NOT 加「`title` 在场」类前置条件） |

<!-- Rust 侧零类型变更：`ChecklistItem { item, pass, evidence }` / `EvaluatorChecklist` / `FinishPayload` / `ChecklistEntry` 逐字不动（`crates/core/workflow/src/model/{domain,checklist}.rs` 出现 diff 即越界）。`title` 不作为任何结构化字段存在——它只在 `serde_json::Value` 载荷与前端视图模型里通行。 -->

### 配置

**无。** 零 `KNOWN_KEYS` 增删、零 `TestFramework` 变体、零 `config.schema.json` 镜像改动（`finish_step` 的参数 schema 是模型工具面，不读工作区配置、不进 `crates/core/config`）。`packages/desktop/package.json` 的 `version` 是版本纪律项，已登记在「修改文件」表（DD7）。

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `finish_step` evaluator 入口参数（工具载荷，线形态） | `report: string`；`items: [{ id: string, title: string, pass: boolean, evidence: string }]`（`title` **新增必填**） | `id` ≡ `ChecklistEntry.id`（`id.enum` 由同一份 prompt 派生）；`title` ≡ `ChecklistEntry.item`（校验期逐字锚定，DD2） | 随 `AgentBlock::ToolUse.input` 原文进会话转录（store 以不透明 JSON 存 `AgentEvent`，`:559-600`）——`title` 因此**零列、零迁移、零 backfill**；历史行天然「无标题」 |
| `ChecklistEntry`（权威源，`workflow::model`） | `id: String`；`item: String`（第二列「检查项」正文，已 `trim`） | 由 `## 静态清单` 三列表解析；8 份 `*-evaluator.md` 是唯一数据源（编译期 `include_str!`） | 不持久化（源即 md 文件）——**零改动** |
| `EvaluatorChecklist` / `ChecklistItem`（落账载荷） | `verdict: Verdict`；`report: String`；`checklist: [{ item: String, pass: bool, evidence: String }]` | `checklist[i].item` ← `ChecklistEntry.item`（按 `id` 命中）；`title` **不入本模型** | `PhaseRecord` → `ChecklistItemRecord`（**零字段变更**，`crates/infra/store/**` 零 diff）；`eval-checklist` 产物形状不变 |
| `FinishItem`（前端视图模型，私有） | `id: string`；`title: string \| null`；`pass: boolean`；`evidence: string \| null` | `title` ← `ToolUse.input.items[].title`（`null` = 缺席 / 空串 / 非字符串）；与 `ChecklistItem` 无交叉引用（前端不反查 prompt，MUST NOT 内置 `id → 标题` 映射） | 不持久化（渲染期派生） |

**关系与持久化要点**

- **`title` 的唯一持久化归属 = 转录里那份工具调用原文**。它不经任何结构化列、不进 `ChecklistItemRecord`、不进 `SystemNotice{finish_step}` 载荷（序列化 `EvaluatorChecklist` 后不含 `title` 键，AC-4 的判据）。
- **两层严格性分治**（spec 明写）：校验层对新提交严格（缺失 / 非字符串 / 空串 / 不一致一律 `Err`），展示层对历史转录宽容（缺席只呈 `id`）。宽容面只服务变更前已落库的会话，不可作「新提交可省 `title`」的依据。
- **无新表、无新列、无外键、无迁移**：本变更一切「新数据」都是既有 JSON 列内的一层键。

---

## 路由 / API 设计

**无。** 本变更不改 IPC 命令面、不改 HTTP 端点、不新增 query 面：`finish_step` 是模型工具（工具 schema 与校验都在进程内），`title` 不进任何出线类型（`FinishSpec` / `ChecklistItem` 均不出线，`bindings:export` 再生成后零 diff）。

---

## 依赖

- **运行时依赖：零新增。** `finish.rs` 只用既有 `serde_json` / `serde` / `workflow::model::checklist_entries`；`verdict.rs` 只用既有 `serde_json::Value` 与 `workflow::model`；`prompt.rs` 只改字符串；前端 `blocks.tsx` 只用既有 `@/lib/parse::textOf` 与 `@/components/ui` 组件。四处 `Cargo.toml`（`core/orchestration`、`infra/agent`、`src-tauri`、workspace 根）与 `packages/desktop/package.json` 的依赖段**零 diff**。
- **依赖方向零变化**：`orchestration → workflow`（既有，`finish.rs` 已 `use workflow::model::checklist_entries`）；`infra/agent → orchestration`（既有，`loop.rs:691` 已调 `finish::validate`）。**零新依赖边**（`.dev-team/architecture/models/**` 零改动）。
- **构建 / 测试依赖：零新增。** Rust 侧沿用既有 `cargo test -p orchestration` 与仓库既有夹具形态（内存 `serde_json::json!` + 合成静态清单字符串，零文件 / 网络 / DB）；前端沿用既有 `vp test` + testing-library 组合，无新测试库。

---

## 验收标准落点

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 卡片呈 `id` + 标题 | `blocks.tsx` 的 `itemTitle` + `CheckListCard` 单 `FieldLabel` 文本 `` `${item.id} ${title}` ``（DD6）；未通过项的 `text-destructive` 仍挂在同一节点（整标签着色）；摘要行 / `FieldDescription` / 零 `block-tool-result` 逐字不变 |
| AC-2 历史转录降级 | `itemTitle` 对「缺席 / 空串 / 非字符串」统一归 `null` → `FieldLabel` 只呈 `id`；零占位符；`isCheckListFinish` 判据零改（仍为 checklist 形态，不回退 JSON）；`textOf` 保证不抛错 |
| AC-3 schema 形态 | `finish.rs::parameters` evaluator 臂的 `items.items.properties.title.type == "string"` + `items.items.required == ["id","title","pass","evidence"]`；decision 臂返回值逐字不变（既有断言零改即证） |
| AC-4 权威守门（正向） | `verdict.rs` 的 `title.trim() == entry.item` 通过路径；`EvaluatorChecklist.checklist[i].item` 仍取权威正文、`title` 在映射时丢弃（序列化载荷不含 `title` 键）；`derive_verdict` 不变 |
| AC-5 权威守门（反向） | `verdict.rs` 的两条 `Err` 文案（DD4）+ 与既有三类 ID 文案措辞互斥；`Err` 经既有 `is_error` ToolResult 回灌（`loop.rs:691` 的既有路径，零改动） |
| AC-6 协议文本面 | `prompt.rs::evaluator_protocol` 第 3 条的字段枚举含 `title`；「逐字」措辞只在 `finish.rs::parameters` 的 `title.description` 出现一处（三条 `grep` 判据见 DD 表下的「AC-6 的 `grep` 判据口径」，对象是模型可见文本、不含源码注释）；既有断言面（工具收口 / 不含 `verdict`·`phase`·`attempt`·`skipped` / `report` 上限）零改 |
| AC-7 回归不破 | decision 形态摘要 / JSON 回退 / `BUILD_IN_TOOL_CARDS` 键面 / `ask.rs` 零 `finish` 字样 / `ChecklistItem` 三字段 / `eval-checklist` 产物：全部零改动（`blocks.tsx` 只动 `FinishItem` + 一处标签表达式；`ask.rs` / `tool-cards.tsx` / `renderers/**` / `infra/store/**` / `workflow/**` 零 diff） |

### 交付门（非本变更能力判据）

全仓既有门（静态检查链 `server:check` → `bindings:export` → `api:check` → `client:check`、全量测试、版本递增、`bindings:export` 零 diff）由主基线承载，本设计与 tasks 不派生新判据。tasks 的自查项只收**源码级静态判据**（`grep` 零命中 / 计数、读码结构复核、`git diff` 边界比对、版本行相对 `HEAD` +1 patch）；测试回归与整链门运行在 tasks 中只以「交接」条目登记。

---

## 待决问题

1. **`Err` 是否该汇总多项不一致**：本期一次一错（DD3）——首项不一致即 `Err`，模型改一处重提仍会再报下一处，回合数不变。若实测出现「多个标题同时被改写」（例如模型按记忆重构整表）导致重试轮数上升，可在不改协议的前提下把校验段改为「收集全部不一致后汇总一条」（文案面扩为多行）；本设计已把校验段放在单一位置，届时是局部改写而非结构变更。
2. **实测重试率（R1）的观测口径**：本期不做任何采样 / 埋点（`finish_step` 的 `Err` 只经既有引擎日志可见）。若后续出现「标题校验重试频繁」的实证，先评估放宽归一化（仍不丢字）——例如「全半角括号 / 引号归一」——再考虑把 `title` 的来源说明改成「可省略标点」一类弱纪律；MUST NOT 回退到「不校验」。
3. **旧会话的错位标题**：历史转录里若某次提交的 `title` 与 `id` 不匹配（模型把 A 项标题写进 B 项），展示层无从判断（前端不持有 prompt、不内置映射表，D5 / spec 明文）。本期接受该形态为「历史数据的既有不完美」，不回填、不告警；若后续要为历史会话提供「权威标题」视图，属独立变更（方向是后端提供按 `id` 查询权威正文的只读面，而非前端内置常量表）。
4. **`eval-checklist` 抽屉是否补 `id`**：proposal 明列为不做（抽屉呈落账面 `item`，本变更不给它补 `id`）。两侧分头演进是既有现状——卡片呈 `id + 标题`、抽屉呈标题；若要合并成单一呈现面，属独立变更（须先裁定抽屉的列语义）。
