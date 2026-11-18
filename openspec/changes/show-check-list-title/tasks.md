# 任务: show-check-list-title

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 5 项（4 处源码 + 1 行版本）+ design 的 DD1–DD7。
>
> **本相位不改测试文件的用例与断言，也不执行测试回归**：改造 / 新增用例（proposal「测试文件」表逐行）归 test-design / test-gen / test-execution 相位；`cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration` 与 `pnpm -C packages/desktop exec vp test run …` 由桌面端 test-execution 相位承接。本相位对测试文件**零动作**——本变更不产生编译期破坏（`title` 是 JSON 字面量字段与前端可选呈现位），只是「必填」使若干既有夹具在运行期不再合法，点位已登记在 design「影响传导」，由测试相位同步。
>
> **依赖序（四阶段）**：
>
> - **阶段一**（协议面：`finish.rs` schema/description + `prompt.rs` 附录）先做——它是模型可见契约的成形，且 DD1 的「措辞单点」要求这两处**同批**落地（分先后会出现「字段已要求但纪律未写明」或反之）。
> - **阶段二**（校验守门：`verdict.rs`）在阶段一之后——`Err` 文案要引用「表第二列原文」这一已由 schema 声明的期望，且 DD3 的追加位点以阶段一后的 schema 形态为参照。
> - **阶段三**（前端收口卡：`blocks.tsx`）与阶段一、二**零依赖**（前端只消费载荷原文），但放其后以免在协议未定时对着未定字段名写码。
> - **阶段四**（版本 + 边界自查 + 交接）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ① **零第二真相**：`crates/core/workflow/src/model/checklist.rs`（`checklist_entries` 解析）与本仓库 8 份 `*-evaluator.md` 静态清单表 **MUST 零 diff**；出现 diff 即说明另起了第二解析面 / 改写了权威源。
> ② **落账面零变更**：`ChecklistItem { item, pass, evidence }`、`EvaluatorChecklist`、`FinishPayload`、`crates/infra/store/**`、`eval-checklist` 产物形状 **MUST 零 diff**；`title` 只在 `serde_json::Value` 载荷与前端视图模型里通行，映射时丢弃。
> ③ **取值纪律单点**（DD1）：**模型可见文本**里的「逐字」只在 `finish.rs` 的 `title.description` 出现一次；`evaluator_protocol` 函数体与 `verdict.rs` 零命中——判据的三条命令见 design「AC-6 的 `grep` 判据口径」（源码注释里的既有「逐字不变」类措辞不属判据）。
> ④ **前端零第二数据源**：不新增 md 解析、不新增 `id → 标题` 常量表、不新增查询命令、零新导出（`itemTitle` 为模块私有，避免 knip 死代码污染）。
> ⑤ **门禁分工**：静态脚本链（`pnpm run check` = `server:check` → `bindings:export` → `api:check` → `client:check`）与测试回归由桌面端 static-check / test-execution 相位执行，本相位不运行；自查项只收源码级静态判据（`grep` / 读码 / `git diff` 边界 / 版本行相对 `HEAD` +1 patch）。

## 阶段一：协议面（`finish.rs` schema 与 description + `prompt.rs` 附录）

- [x] `crates/core/orchestration/src/finish.rs`：`parameters` 的 evaluator 臂在 `id_schema` 之后、`pass` 之前插入 `title` property——`"title": { "type": "string", "description": "检查项标题：取本 prompt「静态清单」表第二列「检查项」原文，逐字照抄（改写 / 省字会被拒绝）" }`（DD1 的**唯一**落位，措辞逐字照此，MUST NOT 在别处再写一份）
- [x] 同文件：`items.items.required` 由 `json!(["id", "pass", "evidence"])` 改为 `json!(["id", "title", "pass", "evidence"])`（DD3 顺序：`id` → `title` → `pass` → `evidence`；与 spec 的「逐值等于」判据一致，不得重排）
- [x] 同文件：`description(FinishSpec::EvaluatorChecklist)` 的逐项字段说明改为「逐项给 title、pass 与 evidence（evidence 须为事实依据）」——**只列字段名**，不复述取值纪律、不出现「逐字」（DD1）；`report ≤2000 字符` / `verdict 由清单推导` / 三个退役工具名的禁用句逐字保留
- [x] 同文件逐字不动复核点：`TOOL_NAME`、`FinishPayload`、`validate` 的两臂分派、decision 臂的 `parameters` 与 `description`、`id_schema` 的定义与 `:62-64` 的空集省略 `enum` 行为——`git diff -- crates/core/orchestration/src/finish.rs` 的 hunk 只允许出现在 `parameters` 的 evaluator 臂与 `description` 的 evaluator 臂
- [x] `crates/core/orchestration/src/prompt.rs`：`evaluator_protocol()` 第 3 条改为 `checklist 逐项需 \`id\`（取自本 prompt「静态清单」表首列）+ \`title\`（同表第二列「检查项」）+ \`pass\`（布尔）+ \`evidence\`（事实依据，含 file:line）`——`title` 只写**字段指针**，不写「逐字 / 原文 / 照抄」类强度措辞（DD1）
- [x] 同文件逐字不动复核点：第 1 / 2 / 4 条、`{MAX_REPORT_CHARS}` 插值、`evaluator_prompt`、`decision_prompt`、`spec_sync_prompt`、`merge_conflict_prompt`、三段 `backtrack_option_*` 助手
- [x] 自查 阶段一（静态判据）：`grep -c "逐字" crates/core/orchestration/src/finish.rs` 为 **1** 且命中行是 `title` 的 `description`；`sed -n '/fn evaluator_protocol/,/^}/p' crates/core/orchestration/src/prompt.rs | grep -c "逐字"` 为 **0**（**判据只覆盖模型可见文本，不含源码注释**——`prompt.rs:110` 等处的既有「逐字不变」类注释不在判据内）；`grep -n "required" crates/core/orchestration/src/finish.rs` 复核 evaluator 臂四值有序、decision 臂仍为 `["action"]`；`cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration --all-targets` 无新增告警
- [ ] 交接 阶段一（测试相位）：`finish_test` 的 `required` 断言改四值 + 新增 `title.type == "string"` 断言；`prompt_test` 新增「逐项字段枚举含 `title`」断言（既有断言零改）；`valid_checklist_args()` 夹具补 `title`——**由桌面端 test-design / test-gen / test-execution 相位承接**，本相位不以任何用例结果为完成判据

## 阶段二：校验守门（`verdict.rs`）

- [x] `crates/core/orchestration/src/verdict.rs`：`passing` 的类型由 `Vec<(String, bool, String)>` 扩为 `Vec<(String, bool, String, serde_json::Value)>`（第四位 = `title` 的**原值**，供 `Err` 文案给「当前值」），逐项收集段末尾 `passing.push((id, pass, evidence.to_owned(), title_raw));`，其余分支（`id` 未知 / `pass` 非布尔 / `evidence` 非字符串）**逐字不变**
- [x] 同文件：在 `report` 上限检查之后、`checklist` 映射之前插入 `title` 校验段（DD3 追加位点），逐项：
  - `raw.as_str().map(str::trim).filter(|text| !text.is_empty())` 取不到 → `Err(format!("字段 items[{index}].title 缺失 / 非字符串 / 空串：期望本 prompt「静态清单」表第二列原文，当前 {raw}"))`（DD4 文案①）
  - 取到后按 `id` 在 `entries` 内命中权威正文：`entries.iter().find(|entry| entry.id == *id)` → `entry.item`（`None` 分支不可达——ID 集合相等已在上游校验；**MUST NOT** 用 `unwrap_or_default()` 兜空串，那会让 `Err` 文案出现「期望「」」这种自相矛盾的期望值）
  - `title != entry.item`（`title` 已 `trim`，权威侧 `parse_row` 已 `trim`，DD2 **仅首尾 trim**）→ `Err(format!("字段 items[{index}].title 与静态清单第二列不一致（ID {id}）：期望「{expected}」，当前「{title}」"))`（DD4 文案②）
  - MUST NOT 折叠内部空白、MUST NOT 大小写 / 全半角归一、MUST NOT 相似度判定
- [x] 同文件：`checklist` 映射的解构补一位（`|(id, pass, evidence, _title)|`），`item` 仍取 `entries` 的权威正文；`title` 在此丢弃（不进阶 `ChecklistItem`）
- [x] 同文件：模块 doc 与 `validate_checklist` doc 补一句「`title` 是展示搬运字段：只用于校验与卡片呈现，映射时丢弃——落账面与载荷零新字段」（doc 措辞不得出现 AC-6 的单点词「逐字」）
- [x] 同文件逐字不动复核点：`MAX_REPORT_CHARS`、`EvaluatorChecklist`、`derive_verdict` 调用、`report` 上限文案、既有三类 ID `Err` 文案（「未知」「缺项」「清单外」）、形状段落九形态文案——`git diff` 的 hunk 只允许出现在 `passing` 声明 / 收集段末 / 新增校验段 / 映射解构 / 两处 doc
- [x] 自查 阶段二（静态判据）：`grep -c "逐字" crates/core/orchestration/src/verdict.rs` 为 **0**；`grep -n "items\[{index}\].title" crates/core/orchestration/src/verdict.rs` 恰 **2** 处（两条 `Err` 文案）；`grep -c "str::trim" crates/core/orchestration/src/verdict.rs` 为 **2**（既有 `id` 一处 + 新增 `title` 一处），且 `grep -c "split_whitespace\|to_lowercase\|to_ascii_lowercase\|similarity" crates/core/orchestration/src/verdict.rs` 为 **0**（无内部空白折叠、无大小写归一、无相似度判定）；读码复核校验段位于 `report` 上限之后、映射之前；`cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration --all-targets` 无新增生产面告警
- [ ] 交接 阶段二（测试相位）：`verdict_test` 合规夹具补 `title` + 新增「一致即通过 / 不一致 / 缺失 / 非字符串」四类用例；`infra/agent/src/sdk/loop_test.rs` 的 `finish_args()` 与 report 超长用例夹具补 `title`（`:5914` 的「清单外 ID」用例零改即可继续命中 DD3 的追加序）——**由桌面端 test-design / test-gen / test-execution 相位承接**

## 阶段三：前端收口卡（`blocks.tsx`）

- [x] `packages/desktop/src/components/agent/message-cards/blocks.tsx`：`FinishItem` 增第二位字段 `/** 检查项标题（工具调用载荷原文；缺席 / 空串 / 非字符串 → `null`，只呈 id） */ title: string | null;`（位置在 `id` 之后、`pass` 之前）
- [x] 同文件：新增模块私有 `function itemTitle(value: unknown): string | null`——`const title = textOf(value); return title === null || title === '' ? null : title;`（`textOf` 来自既有 `@/lib/parse`；**不导出**）
- [x] 同文件 `CheckListCard` 逐项：`const title = itemTitle(item.title);` → `FieldLabel` 子节点改为 `{title === null ? item.id : \`${item.id} ${title}\`}`（单个半角空格，原文不 trim、不截断，DD6）；`className` 仍只有条件类 `text-destructive`（未通过项整标签着色），**不新增** `truncate` / `line-clamp-*` / `overflow-hidden` / `text-ellipsis` / `max-w-*`
- [x] 同文件逐字不动复核点：`failedCount` / `pass` 推导与摘要行文案、`Checkbox readOnly checked={item.pass}`、`{item.evidence && <FieldDescription>…}`、`Field` / `FieldGroup` / `FieldContent` / `Card*` / `TextBlock` / `TraceCollapsible*` 结构、四个 `data-testid`（`block-tool-use` / `data-tool-id` / `block-finish-step-summary` / `block-finish-step-item`）、`isCheckListFinish` 判据、`FinishStepCardBase` 的 decision 形态与 JSON 回退、末尾 `export const FinishStepCard = memo(FinishStepCardBase)`
- [x] 自查 阶段三（静态判据）：`grep -c "itemTitle" packages/desktop/src/components/agent/message-cards/blocks.tsx` 为 **2**（定义 + 调用点），且 `grep -n "export function itemTitle\|export const itemTitle" …` 零命中（私有）；`grep -c "truncate\|line-clamp\|text-ellipsis" packages/desktop/src/components/agent/message-cards/blocks.tsx` 与改动前一致（零新增）；`grep -n "text-destructive" …/blocks.tsx` 仍只一处且条件不变；`pnpm -C packages/desktop exec vp check`（源码级 lint/格式，不含测试运行）通过；`git diff -- …/blocks.tsx` 的 hunk 只允许出现在 `FinishItem` / 新增 `itemTitle` / 标签表达式三处
- [ ] 交接 阶段三（测试相位）：`blocks.test.tsx` 的 `FINISH_EVALUATOR_INPUT` 补 `title` + 新增 AC-1 标签全文与着色断言 + AC-2 三形态参数化降级用例；`tool-cards.test.tsx` **零改动**（`title` 缺席即 id-only，既有断言不受影响）——**由桌面端 test-design / test-gen / test-execution 相位承接**

## 阶段四：版本 + 边界自查 + 交接

- [x] `packages/desktop/package.json`：`version` 由落地时 `HEAD` 现值 **+1 patch**（当前 `0.6.51` → 期望 `0.6.52`；MUST NOT 写死字面号，以 `git diff HEAD -- packages/desktop/package.json` 的版本行为准）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随（零改动）、`src-tauri/Cargo.toml` crate 版本行**不跟**
- [x] 边界自查（越界即回退，逐条 `git diff --name-only` 核对）：`crates/core/workflow/**`（含 `model/checklist.rs`、`model/domain.rs` 与 8 份 `*-evaluator.md`）零 diff；`crates/infra/store/**` 零 diff；`crates/core/orchestration/src/{ask,decision,transcript,walker,state,control}.rs` 零 diff；`crates/infra/agent/src/sdk/{mount,loop,policy,tools}.rs` 零 diff；`packages/desktop/src/components/agent/message-cards/tool-cards.tsx`、`src/lib/agent-adapter.ts`、`src/views/changes/renderers/**`、`src/types/generated/**` 零 diff；`.dev-team/architecture/models/**` 零 diff
- [x] 边界自查（单点与降级语义）：按 design「AC-6 的 `grep` 判据口径」三条命令复核——`grep -c "逐字" crates/core/orchestration/src/finish.rs` 为 **1**（唯一处 = `title.description`）、`sed -n '/fn evaluator_protocol/,/^}/p' crates/core/orchestration/src/prompt.rs | grep -c "逐字"` 为 **0**、`grep -c "逐字" crates/core/orchestration/src/verdict.rs` 为 **0**；`grep -rn "id → 标题\|标题映射\|CHECKLIST_TITLES" packages/desktop/src` 零命中（前端零常量表）；`grep -c "finish" crates/core/orchestration/src/ask.rs` 为 **0**（AC-7 的既有守卫仍成立）
- [x] 读码复核（不改代码，只确认形状）：`verdict.rs` 的 `ChecklistItem` 映射与 `derive_verdict` 调用未被 `title` 污染（序列化 `EvaluatorChecklist` 后无 `title` 键）；`finish.rs` 的 decision 臂与改动前逐字一致；`blocks.tsx` 的 `items` 未做归一化数组、`pass` / `evidence` 读取路径未被触碰
- [ ] 交接 阶段四（生成物 + 门禁 + 回归，本相位不执行）：`bindings:export` 再生成后**期望零 diff**（本变更不出线新 IPC 类型）由桌面端 **static-check** 承接；整链静态门（`server:check` → `bindings:export` → `api:check` → `client:check`）与两个验证命令（`cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration`、`pnpm -C packages/desktop exec vp test run src/components/agent/message-cards/blocks.test.tsx`）由桌面端 **static-check / test-execution** 承接。**交付物 = 期望登记与移交**
