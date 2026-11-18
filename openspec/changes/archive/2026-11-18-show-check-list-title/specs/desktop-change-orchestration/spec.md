# desktop-change-orchestration Specification (Delta)

## ADDED Requirements

### Requirement: checklist 项标题的搬运与权威校验

evaluator 形态的 `finish_step` SHALL 让逐项结果自带**检查项标题**，使工具调用载荷（前端唯一可见面）自足可读；标题的权威性 SHALL 由工具校验守住，MUST NOT 在前端、落账面或 orchestration 侧另立第二份真相：

- **schema 面**（`crates/core/orchestration/src/finish.rs::parameters`）：evaluator 形态的 `items[].properties` SHALL 增 `title`（`type: "string"`），并 SHALL 进 `items[].required`（与 `id` / `pass` / `evidence` 平级）——缺席即校验 `Err`，MUST NOT 设为可选（可选则「标题恒在场」不可断言，展示面无法给出确定承诺）。decision 形态 schema MUST NOT 变化。
- **模型可见的取值要求**：`title` 的取值纪律（取本 prompt `## 静态清单` 表第二列「检查项」原文、逐字）SHALL 在**单点**承载（schema 的 `title.description`，或 `evaluator_protocol` 附录——二者择一，由 design 定稿），MUST NOT 两处各写一份独立措辞（两句措辞一旦分叉，模型按哪句都是「照协议做」）。
- **校验面**（`crates/core/orchestration/src/verdict.rs::validate_checklist`）：逐项 `title` SHALL 与 `checklist_entries(prompt)` 中同 `id` 条目的第二列正文比对；不一致 SHALL `Err` 拒绝并当场回灌（is_error ToolResult，模型在同一 turn 内纠正重试），Err 文案 SHALL 自足（该项 `id` + 期望原文 + 当前值）。`title` 缺失 / 非字符串 SHALL 单独成一条 `Err` 文案（与「ID 缺项 / 清单外 ID / 未知 ID」三类既成文案分列可辨）。比较 MUST NOT 退化为「语义等价」的模糊匹配（丢字的相似度判定即等于承认第二真相）；是否做首尾空白裁剪 / 内部连续空白折叠由 design 定稿为单点规则，MUST NOT 逐项各写。
- **落账面零变更**：`EvaluatorChecklist.checklist` 的 `ChecklistItem { item, pass, evidence }` 三字段形状与「`item` 取静态清单第二列权威正文」的映射 SHALL 逐字不变；`title` 是**展示搬运字段**，MUST NOT 进 `ChecklistItem`、MUST NOT 落库、MUST NOT 成为 `SystemNotice{finish_step}` 载荷的新字段。
- **零第二解析面**：`workflow::model::checklist::checklist_entries` 仍是「ID + 权威人读文案」的唯一解析实现；本变更 MUST NOT 在 orchestration 或前端新增 prompt md 解析、MUST NOT 把静态清单复制成代码常量 / 生成物 / 前端映射表。
- **可行性锚**：`checklist_entries` 只收第二列非空的行（空列行不入 `entries`，因而不进 `id.enum`），故凡会被要求提交 `title` 的 `id` 必然有权威正文可依——MUST NOT 出现「要求 `title` 却无权威文本」的 `id`。
- **两层严格性分治**：校验层（本 requirement）对**新提交**严格（`title` 缺失 / 空串 / 非字符串 / 不一致一律 `Err`）；展示层（desktop-agent-chat-infra「finish_step 收口卡呈现」）对**历史转录**宽容（`title` 缺席只呈 `id`）。二者不构成矛盾判据：宽容面只服务变更前已落库的会话，MUST NOT 被用作「新提交可省 `title`」的依据。

#### Scenario: 标题一致即通过且落账取权威

- **WHEN** 提交 `{ report, items: [{ id: "R1", title: "问题描述清晰，包含背景和动机", pass: true, evidence: "proposal.md:12" }, …] }`，其中各项 `title` 与该 prompt `## 静态清单` 表第二列逐字一致
- **THEN** `validate_checklist` 返回 `Ok`；`EvaluatorChecklist.checklist` 每项 `item` 为该权威正文、`pass` / `evidence` 原样；载荷序列化后**不含 `title` 键**；`verdict` 仍由 `derive_verdict` 从 checklist 推导

#### Scenario: 标题被改写即拒绝且可当场自纠

- **WHEN** 提交的 `items[2].title` 为改写句（如把「关键决策包含理由和被拒绝的备选方案」写成「关键决策有理由」）
- **THEN** `validate_checklist` 返回 `Err`，文案含该项 `id`、期望原文与当前值；该 `Err` 以 is_error ToolResult 回灌，会话继续（同一 turn 内可重试，不落账、不判相位失败、不产生跨轮计数）

#### Scenario: 标题缺席即拒绝

- **WHEN** 某项缺 `title` 字段，或 `title` 为 `null` / 数字 / 空串
- **THEN** `validate_checklist` 返回 `Err`，文案点明字段名 `items[<i>].title`、期望（本 prompt 静态清单表第二列原文）与实际值；MUST NOT 降级为「无标题通过」，MUST NOT 落账

#### Scenario: 两形态 schema 面分治

- **WHEN** 分别读取 `parameters(FinishSpec::EvaluatorChecklist, prompt)` 与 `parameters(FinishSpec::Decision, prompt)`
- **THEN** 前者 `items.items.properties.title.type == "string"` 且 `items.items.required` 逐值等于 `["id","title","pass","evidence"]`；后者返回值与改动前逐字相同（`action.enum` 仍为 `["backtrack","stop"]`，无 `title` 相关键）

## MODIFIED Requirements

### Requirement: evaluator 输出协议瘦身

`evaluator_protocol` 输出协议附录 SHALL 要求 evaluator **调用 `finish_step` 工具收口**（形状与落账红线由工具参数给出；MUST NOT 要求在最终消息输出任何 JSON 对象、MUST NOT 说明 `verdict` / `phase` / `attempt` / `skipped` 字段——verdict 由 checklist 推导、其余由 walker 按 provenance 盖戳）。协议 SHALL 声明落账归属红线（评估落账由桌面编排代写，agent 只产结论、不写任何落账数据含 `workflow.json`），MUST NOT 点名插件时代的 MCP 工具（`phase_log` / `phase_next` / `phase_start` / `backtrack`——桌面工具面不存在这些名字，点名只会诱发幻觉调用）；checklist 至少一项、逐项含 `id`（取自 prompt「静态清单」表首列）+ `title`（同表第二列「检查项」原文，见「checklist 项标题的搬运与权威校验」）+ `pass`（布尔）+ `evidence`（事实依据，含 `file:line`）；`report` ≤ `MAX_REPORT_CHARS`。协议附录 MUST NOT 与 schema description 各写一份 `title` 取值措辞（措辞单点落位见「checklist 项标题的搬运与权威校验」的「模型可见的取值要求」条：二者择一，由 design 定稿）。

#### Scenario: 协议改为工具收口

- **WHEN** 读取 evaluator 输出协议附录文本
- **THEN** 要求「调用 `finish_step` 收口」
- **AND** 无「输出一个 checklist JSON 对象」「形状为 `{ verdict, report, checklist }`」类要求，无「输出 phase / attempt / skipped 字段」要求
- **AND** 仍含落账归属红线与 `report` 上限

#### Scenario: 逐项字段枚举含标题

- **WHEN** 读取 evaluator 输出协议附录的 checklist 逐项字段枚举
- **THEN** 含 `id` / `title` / `pass` / `evidence` 四项，`title` 指明取 prompt「静态清单」表第二列原文
- **AND** 该附录不重复书写「逐字」等取值措辞的第二份表述（`title` 取值纪律的措辞单点在 schema description 与附录之间只占一处）

#### Scenario: 退役修复注入零命中

- **WHEN** 审查 `prompt.rs` 的动态面组装
- **THEN** 无「verdict 解析失败」相关文案与注入分支（`verdict_fix_prompt` / `decision_fix_prompt` 已退役，`grep` 零命中）

## Module Contract

### Function: `crates/core/orchestration/src/finish.rs`

| 函数 | 签名 | 变化 |
|------|------|------|
| `parameters` | `(spec: FinishSpec, prompt: &str) -> serde_json::Value`（签名不变） | evaluator 臂 `items.items.properties` 增 `title`（`type: "string"`），`items.items.required` 扩为 `["id","title","pass","evidence"]`；`id.enum` 派生规则与 decision 臂逐字不变 |
| `description` | `(spec: FinishSpec) -> &'static str`（签名不变） | evaluator 臂逐项字段说明按「取值纪律单点」裁定同步（若单点定在 schema，则此处只列字段名）；decision 臂文本不变 |

### Function: `crates/core/orchestration/src/verdict.rs`

| 函数 | 签名 | 变化 |
|------|------|------|
| `validate_checklist` | `(args: &Value, prompt: &str) -> Result<EvaluatorChecklist, String>`（签名与返回类型不变） | 增 `title` 读取与「与 `checklist_entries(prompt)` 第二列正文比对」两段；Err 文案自足（`id` + 期望原文 + 当前值 / 字段缺失）；`ChecklistItem` 映射（`item` 取权威）、`report` 上限、`derive_verdict` 推导逐字不变 |
| `MAX_REPORT_CHARS` | `pub const MAX_REPORT_CHARS: usize = 2000` | 零改动 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `crates/core/workflow/src/model/checklist.rs` | **零改动**：`checklist_entries` 的第二列解析与「空列行丢弃」规则即本变更的权威锚——出现 diff 即说明另起了第二解析面 |
| `crates/core/workflow/src/model/domain.rs` | **零改动**：`ChecklistItem { item, pass, evidence }` 三字段不变 |
| `crates/core/orchestration/src/{ask,decision,transcript,walker}.rs` | **零改动**：ask 零互引守卫、decision 校验、收口载荷提取、verdict 裁决与落账语义均不涉标题 |
| `crates/infra/agent/src/sdk/{mount,loop}.rs` | **零改动**：`ToolExec::FinishStep` 分派与 `("已收口", false)` 结果正文不变 |
| `crates/infra/store/**` | **零改动**：`ChecklistItemRecord` 与 `PhaseRecord` 落库形状不变 |
| `crates/core/workflow/src/prompts/*-evaluator.md`（8 份） | **零改动**：静态清单三列表是本变更的输入而非改动面 |
