# 测试设计: show-check-list-title

> **日期**: 2025-11-18
> **基于**: `design.md`（修改文件 4 处源码 + 1 行版本号；公共函数 / API 表 3 行；影响传导表 6 个夹具点位）
> **测试框架**：Rust 侧 `#[test]` 共置模块（`src/*_test.rs`，经 `lib.rs` 的 `#[cfg(test)] mod` 挂载，`cargo test -p orchestration` / `cargo nextest`）；前端侧 Vite+（`vite-plus/test` 导出 `describe` / `it` / `expect` / `vi`，vitest API + `@testing-library/react`，文件头 `// @vitest-environment jsdom`，`vp test run <文件>`）。
> **范围口径**：只设计 `design.md` 修改文件表内的模块；删除面（本变更无）与零改动面（`workflow/src/model/**` / `infra/store/**` / `tool-cards.tsx` / `renderers/**`）不设章节。跨模块链路的用例挂靠发起方 / 最上层调用方模块的 per-file 章节，无独立集成测试区。

---

## 验收范围

<!-- 逐条映射 proposal.md 验收标准（AC-1…AC-7）；`被测文件或模块` = 承载用例的测试文件（单值，
     同一 AC 的多文件落点按「同上——…半边」拆行）。版本纪律与全仓门禁落「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | **卡片呈 `id` + 标题**：evaluator 形态逐项 `FieldLabel` 文本为 `id + 单个半角空格 + 标题原文`（不截断）；未通过项该标签带 `text-destructive` 且标题一并着色；摘要行仍为「检查 1 项未通过」、`evidence` 仍在 `field-description` 位、零 `block-tool-result` 节点 | `packages/desktop/src/components/agent/message-cards/blocks.test.tsx` |
| AC-2 | **历史转录降级**：`items` 元素缺 `title` / `title` 为空串 / `title` 为非字符串（`null`、数字、对象、数组）→ 逐项行只呈 `id`，无占位符、无 `undefined` / `null` 文本、不抛错、不回退 JSON（仍为 checklist 形态） | `packages/desktop/src/components/agent/message-cards/blocks.test.tsx` |
| AC-3 | **schema 形态**：`parameters(FinishSpec::EvaluatorChecklist, prompt)` 的 `items.items.properties.title.type == "string"`、`items.items.required` 逐值等于 `["id","title","pass","evidence"]`；同函数 `FinishSpec::Decision` 臂返回值与改动前逐字相同（既有断言零改即证） | `packages/desktop/src-tauri/crates/core/orchestration/src/finish_test.rs` |
| AC-4 | **权威守门（正向）**：`title` 与 prompt 静态清单第二列逐字一致 → `validate_checklist` 返回 `Ok`，`EvaluatorChecklist.checklist` 每项 `item` 为该权威正文、序列化载荷不含 `title` 键；`verdict` 仍由 `derive_verdict` 推导 | `packages/desktop/src-tauri/crates/core/orchestration/src/verdict_test.rs` |
| AC-5 | **权威守门（反向）**：`title` 被改写 → `Err`（文案含该 `id`、期望原文、当前值）；`title` 缺失 / 非字符串 / 空串 → `Err`（文案点明 `items[<i>].title` 与期望）；两类文案与既有「ID 缺项 / 清单外 ID / 未知 ID」三类分列可辨 | `packages/desktop/src-tauri/crates/core/orchestration/src/verdict_test.rs` |
| AC-6 | **协议文本面**：`evaluator_protocol()`（经 `evaluator_prompt` 观测）的逐项字段枚举含 `id` / `title` / `pass` / `evidence` 且 `title` 指明「同表第二列」；既有断言（工具收口 / 不含 `verdict`·`phase`·`attempt`·`skipped` / `report` 上限）零改；函数体内「逐字」零命中 | `packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs` |
| AC-6 | （同上——单点判据正向半边：`finish.rs` 内「逐字」计数恰 1 且唯一处为 `title.description`） | `packages/desktop/src-tauri/crates/core/orchestration/src/finish_test.rs` |
| AC-6 | （同上——单点判据反向半边：`verdict.rs` 内「逐字」零命中，`Err` 文案只陈述事实） | `packages/desktop/src-tauri/crates/core/orchestration/src/verdict_test.rs` |
| AC-7 | **回归不破（前端半边）**：decision 三动作摘要 / 未知动作 / JSON 回退 / `data-testid` 与逐项行结构 / `pass` 真值判定 / `evidence` 真值渲染——既有用例零改 | `packages/desktop/src/components/agent/message-cards/blocks.test.tsx` |
| AC-7 | （同上——Rust 半边：decision 臂 schema 与 `description` 逐字不变、`id.enum` 派生与空集省略不变、`report` 上限与 ID 三分类文案不变） | `packages/desktop/src-tauri/crates/core/orchestration/src/finish_test.rs` |

---

## 单元测试

<!-- 覆盖可在进程内验证的场景：Rust `#[test]` 共置模块（内存 `serde_json::json!` + 合成静态清单
     字符串 + 编译期 `include_str!` 语料，零文件 / 网络 / DB）与前端 Vite+ / testing-library 组件测试。
     4 个既有测试文件扩展（无新建测试文件）：finish_test.rs / verdict_test.rs / prompt_test.rs /
     blocks.test.tsx。fixtures 与版本行的夹具同步点位落「不可测试项」。
     design.md「影响传导」登记的 `infra/agent/src/sdk/loop_test.rs`、`tool-cards.test.tsx` 属本变更修改
     文件表外模块，不设章节（见「不可测试项」）。 -->

### packages/desktop/src-tauri/crates/core/orchestration/src/finish.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/finish_test.rs

#### 待测功能

<!-- 既有夹具同步（design「影响传导」登记，断言按本变更新形状改写而非新增能力面）：
     `parameters的evaluator臂结构六断言` 的 `required` 断言由三值改四值 + 新增 `title.type` 断言；
     `valid_checklist_args()` 补 `title`（否则经 `validate` 分派的既有正向用例运行期不再合法）。 -->

- description(spec: FinishSpec) -> &amp;'static str: evaluator 臂逐项字段说明只列字段名（`title`、`pass` 与 `evidence`，不复述取值纪律）；decision 臂文本逐字不变；两臂仍各 &lt;500 字符
- parameters(spec: FinishSpec, prompt: &amp;str) -> serde_json::Value: evaluator 臂 `items.items.properties` 增 `title`（`type: "string"`，`description` 为「取本 prompt「静态清单」表第二列「检查项」原文、逐字照抄」的**唯一**落位，DD1）、`items.items.required` 扩为 `["id","title","pass","evidence"]`；`report` / `items`（`type` / `minItems` / `description`）、顶层 `required`、`id.enum` 派生与空集省略逐字不变；decision 臂零变化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parameters（evaluator 臂 title 段） | 正向 | `parameters(EvaluatorChecklist, synthetic_prompt())` 的 `items.items.properties.title`：`type == "string"`；`description` 非空、含「静态清单」与「第二列」（字段指针落位可断言）且含「逐字」 | 新增 |
| parameters（evaluator 臂 required） | 正向 | `items.items.required` 逐值等于 `["id","title","pass","evidence"]`（顺序断言：`title` 紧接 `id` 之后、`evidence` 收尾）；`title` 与 `id` / `pass` / `evidence` 平级必填（MUST NOT 可选） | 改造（既有六断言扩位） |
| parameters（evaluator 臂既有面零改） | 边界 | `report.type == "string"`、`items.type == "array"`、`items.minItems == 1`、`pass.type == "boolean"`、顶层 `required == ["report","items"]`、`properties` 内零 `verdict` 键——既有六断言其余五条逐字成立（AC-3 后半「既有断言零改即证」的 evaluator 半边） | 回归 |
| parameters（evaluator 臂 id.enum 派生） | 边界 | 八份 `include_str!` evaluator 语料逐份：`id.enum` 仍逐值有序等于 `checklist_entries(prompt)` 的 ID 序；新增 `title` 键不改变 enum 内容与位置（8 份语料 id 序逐相位对拍，既有用例零改） | 回归 |
| parameters（evaluator 臂空 ID 集） | 边界 | 空 prompt / 仅表头的 prompt：`id.enum` 键仍省略（空数组非法）、`id.type == "string"` 仍在场、`title` 段与四值 `required` 照常在位（退化面不吞掉新字段） | 改造（既有退化面用例补 title 断言） |
| parameters（两形态互不渗透） | 异常 | `parameters(Decision, …)` 序列化全文零 `title` / `report` / `items` / `evidence` 命中；`parameters(EvaluatorChecklist, …)` 序列化全文零 `action` / `backtrack` / `stop` / `"to"` 命中（`title` 不引入 decision 词汇渗漏） | 改造（既有互不渗透用例扩关键词） |
| parameters（decision 臂零变化） | 异常 | `parameters(Decision, "")` 的 `action.enum == ["backtrack","stop"]`、`properties` 键集恰 `{action, to, reason}`、`required == ["action"]`、全文零 `question` / `options`；两种不同 prompt 入参结果逐字段相等（prompt 零影响）——decision 臂逐字不变（既有断言零改） | 回归 |
| parameters（幂等与键序） | 边界 | 同入参两次调用：`serde_json::Value` 逐字段相等且序列化字符串逐字相等（新增 `title` 段零内部状态、键序稳定） | 改造（既有幂等用例零改即成立，补 title 在场断言） |
| description（evaluator 臂字段名枚举） | 正向 | `description(EvaluatorChecklist)` 含 `title` / `pass` / `evidence` 三字段名与「清单」语义、含 `report` 上限 `2000`；字符数 &lt;500（只列字段名，不因新增 title 膨胀） | 改造（既有用例补 title 断言） |
| description（取值纪律单点） | 异常 | `description(EvaluatorChecklist)` 全文零「逐字」「照抄」「第二列」字样（取值纪律只在 `title.description` 一处，DD1：三处同写即 AC-6 判据失败） | 新增 |
| description / parameters（「逐字」单点判据，源码自证） | 异常 | `source_of("src/finish.rs")` 全文「逐字」计数恰 == 1；且该唯一次出现在 `title` 的 `description` 字符串字面量所在行（按行定位断言该行同时含 `title` 与 `逐字`）——AC-6 的 `grep -c` 判据的用例化形态 | 新增 |
| parameters / description（decision 臂文本逐字） | 边界 | `description(Decision)` 连调两次逐字相等（`&'static str` 零动态段）；两形态描述 `<500` 字符且互不相等（既有体量 / 可辨断言零改） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（零 mock） | `prompt` 取内存合成静态清单字符串；八份语料经 `workflow::write::phase_table` 的 `include_str!` 编译期常量取得（零 `fs::read`）；`checklist_entries` / `validate_checklist` / `validate_decision` 真实组合（分派语义即被测对象，替身会掩盖臂错配） | 全部行 |
| 产品源文件（只读文件系统） | 进程边界：`source_of("src/finish.rs")` 经 `CARGO_MANIFEST_DIR` 真实读取 + `code_lines` 剔除注释行（既有装置沿用，不 mock 文件系统） | 「逐字」单点判据行 |

### packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/verdict_test.rs

#### 待测功能

<!-- 既有夹具同步（design「影响传导」登记）：`item_args(id, pass, evidence)` 追加第四位 `title`，
     `full_args` / `args_with_items` 随之补位——否则全部合规夹具在运行期一律 `Err`。
     本变更测试面 = 末尾追加的「title 形态 + 逐字比对」两段（DD2 / DD3 / DD4）。 -->

- validate_checklist(args: &amp;serde_json::Value, prompt: &amp;str) -> Result&lt;EvaluatorChecklist, String&gt;: 逐项 `title` 读取（缺失 / 非字符串 / trim 后空串 → `Err` 文案①，点明 `items[<i>].title` 与期望）；与 `checklist_entries(prompt)` 同 `id` 条目第二列正文以 `title.trim() == entry.item` 比对（不一致 → `Err` 文案②，含 `id` + 期望原文 + 当前值）；校验追加在 `report` 上限之后、`id → item` 映射之前；`ChecklistItem.item` 仍取权威正文、`title` 映射时丢弃；`report` 上限、`derive_verdict`、既有三类 ID `Err` 文案逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| validate_checklist（一致即通过） | 正向 | 两项 `title` 与合成清单第二列逐字一致（`"变更代码中无安全漏洞"` / `"变更代码有对应测试"`）→ `Ok`；`checklist[0].item` / `checklist[1].item` 为权威正文且 `!= id`；`pass` / `evidence` / `report` 原样保真 | 改造（既有全链路用例补 title） |
| validate_checklist（载荷零 title） | 正向 | `serde_json::to_value(&EvaluatorChecklist)` 后对象键集恰 `{verdict, report, checklist}`，`checklist[*]` 键集恰 `{item, pass, evidence}`——`title` 键零命中（AC-4「序列化载荷不含 title」判据） | 新增 |
| validate_checklist（verdict 推导） | 正向 | 全 pass 且 title 全一致 → `Verdict::Pass`；含一项 `pass: false` → `Verdict::Fail`（`derive_verdict` 语义零改）；模型传入的 `verdict` 字段仍不读不校验（含非法值 `bogus` 仍 `Ok`） | 改造（既有 verdict 两态 / 忽略用例补 title） |
| validate_checklist（首尾空白归一化） | 边界 | `title = "  变更代码中无安全漏洞  "`（首尾空白 + 全角正文）→ `Ok` 且 `item` 仍为未带空白的权威正文（DD2：仅 `trim`，权威侧 `parse_row` 已 trim） | 新增 |
| validate_checklist（trim 后空串归缺失） | 边界 | `title = ""`、`"   "`、`"\t\n"` 三形态 → `Err` 文案①（trim 后空串不算「一致」也不进不一致分支） | 新增 |
| validate_checklist（不一致即拒） | 异常 | 第二项 `title` 被改写（`"变更代码有对应测试X"` / `"变更代码测试"` 两形态）→ `Err`；文案含该项 `id`、期望原文 `变更代码有对应测试`、当前值原文；含分类词「不一致」，且不含「缺项」「清单外」「未知」（与三类 ID 文案措辞互斥） | 新增 |
| validate_checklist（缺失 / 非字符串文案①） | 异常 | 六形态参数化：`title` 字段缺席、`null`、数字 `42`、布尔 `false`、对象 `{ "a": 1 }`、数组 `["x"]` → 逐形态 `Err`；文案含字段路径 `items[0].title`、含期望（静态清单第二列原文）与「缺失 / 非字符串」类表述；不含「不一致」 | 新增 |
| validate_checklist（元素索引定位） | 异常 | 第二项（`items[1]`）title 缺失 / 不一致 → 文案定位 `items[1].title`（索引与元素对位，不恒指 0） | 新增 |
| validate_checklist（内部空白承载语义） | 边界 | prompt 第二列含内部多空白的项（如 `单元测试 > 用例 表格每行已生成对应测试骨架`）：`title` 与权威逐字相同（内部空白一致）→ `Ok`；把内部连续空白折叠成单空格 → `Err`（DD2 反面判据：MUST NOT 折叠内部空白） | 新增 |
| validate_checklist（校验顺序：清单外 ID 先） | 边界 | 同时「含清单外 ID `C9`」与「某合法项 title 缺失」→ 文案为「清单外 ID」（title 校验后置于 ID 集合比对；既有用例零改即证可达性） | 回归 |
| validate_checklist（校验顺序：缺项先） | 边界 | 同时「缺 `C2`」与「`C1` title 缺失」→ 文案为「缺项」（既有三级文案可达性不被 title 校验抢先） | 回归 |
| validate_checklist（校验顺序：report 上限先） | 边界 | `report` 超长（`MAX_REPORT_CHARS + 1`）且 `title` 不一致 → 文案为「字段 report 超长」（report 上限检查位于 title 校验之前）；恰上限 + title 一致 → `Ok`（既有上限三要素断言零改） | 改造（既有上限用例补 title） |
| validate_checklist（一次一错） | 边界 | 两项 `title` 均被改写 → `Err` 只报首项（文案含 `items[0].title`、不含 `items[1].title`），不做多项汇总（DD3 / 待决问题 1 的当前口径） | 新增 |
| validate_checklist（重复 ID / 多余字段 / 规模） | 边界 | 重复 `C1`（两次，均 title 一致）→ `Ok` 且不静默去重（3 行）；`phase` / `attempt` / `skipped` 多余键静默忽略且不影响 title 校验；100 项与 1000 项全 title 一致 → `Ok` 不 panic（既有规模用例补 title 后零改） | 改造（既有三用例补 title） |
| validate_checklist（特殊字符） | 边界 | `title` 含全角标点 / emoji / `\0` / 换行：与权威正文逐字相同 → `Ok`；`\n` 形态（权威正文不含换行）→ `Err` 且当前值原文出现在文案中；CRLF + emoji 噪声 prompt 下 title 仍一致 → `Ok`（既有噪声用例补 title） | 改造（既有特殊字符 / CRLF 用例补 title） |
| validate_checklist（「逐字」零命中，源码自证） | 异常 | `source_of("src/verdict.rs")` 全文「逐字」计数 == 0（`Err` 文案只陈述 `id` + 期望 + 当前值，不出现取值纪律措辞——AC-6 判据第三行） | 新增 |
| validate_checklist（退役面零残留） | 异常 | `MAX_REPORT_CHARS == 2000` 且 `parse_verdict` / `extract_json` / `fenced_block` / `brace_slice` 在 `verdict.rs` 零命中（既有自证用例零改） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（零 mock） | `args` 为内存 `serde_json::json!` 字面量；`prompt` 为内存合成静态清单字符串（含 CRLF / emoji 噪声形态）；`checklist_entries` / `derive_verdict` 真实调用（单点语义即被测面）；零文件 / 网络 / DB / 子进程 / 环境变量 | 全部行为行 |
| 产品源文件（只读文件系统） | 进程边界：`source_of("src/verdict.rs")` 经 `CARGO_MANIFEST_DIR` 真实读取（既有装置沿用，不 mock） | 「逐字」零命中 / 退役面零残留行 |

<!-- 跨模块夹具同步（不设本章节用例，AC 不由此承载）：`crates/infra/agent/src/sdk/loop_test.rs` 的
     `finish_args()` 与 `finish_step的err文案三要素可见` 合法清单夹具需补 `title`（design「影响传导」登记）；
     `:5914` 的「清单外 ID」用例零改即可继续命中 DD3 的追加序。`.rs` 归属 `infra/agent`（本变更零改动模块），
     故其断言面不出现在本文件，见「不可测试项」。 -->

### packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未列该文件行（`prompt.rs` 只改字符串字面量，模块导出面
     `evaluator_prompt` / `decision_prompt` / `spec_sync_prompt` / `merge_conflict_prompt` 签名零变）。
     待测面取自 design「修改文件」表的 `prompt.rs` 行 + 「验收标准落点」AC-6 行：
     改动点为私有 `evaluator_protocol()`，经公开 `evaluator_prompt(phase_prompt)` 的产出文本观测（零新增导出）。 -->

- evaluator_protocol()（私有，经 evaluator_prompt(phase_prompt) 观测）: 第 3 条逐项字段枚举由 `id` + `pass` + `evidence` 扩为 `id` + `title`（同表第二列「检查项」）+ `pass` + `evidence`；不写「逐字」类取值措辞；第 1 / 2 / 4 条与 `MAX_REPORT_CHARS` 插值、`evaluator_prompt` 的 append 结构逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| evaluator_protocol（字段枚举四值） | 正向 | `evaluator_prompt("主体")` 产出文本含 `checklist 逐项需` 所在行：该行同时含 `id`、`title`、`pass`、`evidence` 四字段名，且 `id` 与 `title` 的偏移均小于 `pass` / `evidence`（枚举序 `id` → `title` → `pass` → `evidence`） | 新增 |
| evaluator_protocol（title 字段指针） | 正向 | 该行 `title` 邻域含「第二列」与「检查项」字样（字段指针，非取值纪律复述）——`title` 的来源与 `id`（「表首列」）同形可读 | 新增 |
| evaluator_protocol（既有四断言零改） | 正向 | 产出文本含「调用 `finish_step` 收口」与落账归属红线（`评估落账由桌面编排代写` / `workflow.json`）、含 `2000`（`MAX_REPORT_CHARS` 插值）、零 `"verdict"` / `"checklist"` / `"report":` / json 代码围栏（不定义 JSON 形状）、零 `"phase"` / `"attempt"` / `"skipped"`（零回声禁令）、含 `verdict` 由「推导」的说明与 `evidence` 语义——既有断言逐条成立 | 回归 |
| evaluator_protocol（「逐字」零命中，源码自证） | 异常 | 自 `fn evaluator_protocol` 起至函数体结束的切片内「逐字」计数 == 0（取值纪律单点在 `title.description`，DD1）；切片内零 `minItems` / `properties` / `required` JSON Schema 词（形状归工具 schema） | 新增 |
| evaluator_protocol（措辞不重复 description） | 异常 | 产出文本不含 `finish.rs::description(EvaluatorChecklist)` 的完整取值纪律措辞（「逐字照抄」「改写 / 省字会被拒绝」）——两处各写一份即判据失败 | 新增 |
| evaluator_prompt（拼接契约） | 边界 | 产出以 `phase_prompt` 开头、`\n\n---\n\n## 输出协议` 分隔唯一动态 append、`evaluator_prompt("")` 等于纯协议段且非空（既有拼接契约用例零改，新增字段名不破坏段结构） | 回归 |
| evaluator_prompt（确定性） | 边界 | 同输入两次组装逐字节一致；`evaluator_prompt("")` 的协议段含 `title`（空主体形态同样携带新枚举） | 改造（既有确定性用例补 title 在场断言） |
| decision_prompt（零影响） | 边界 | 五段在场 / 两动作协议 / 白名单注记 / 窗口段四组既有用例零改（本变更不触碰 decision 面文本） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（零 mock） | 全部为解析性字符串断言：入参为主体内存字面量与 `DecisionInput` 内存 fixture（白名单项取真实 `phase_table`），`format!` 组装真实执行 | 全部行 |
| 产品源文件（只读文件系统） | 进程边界：`std::fs::read_to_string(CARGO_MANIFEST_DIR/src/prompt.rs)` 真实读取后做函数切片（既有 `*_fix_prompt` 退役自证用例同型装置，不 mock） | 「逐字」零命中行 |

### packages/desktop/src/components/agent/message-cards/blocks.tsx -> packages/desktop/src/components/agent/message-cards/blocks.test.tsx

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未列该文件行（前端零新增 / 零改动的模块级导出：
     `FinishStepCard` 引用稳定性零变；`itemTitle` 为模块私有函数，按 AGENT.md no-test-only-exports
     纪律不导出，经 `FinishStepCard` → `CheckListCard` 公开组件间接驱动）。
     待测面取自 design「修改文件」表 `blocks.tsx` 行 + 「类型定义」表 `FinishItem` 行。 -->

- FinishItem.title（interface 私有，增位）: `{ id: string; title: string | null; pass: boolean; evidence: string | null }`；`title` 经 `textOf` 宽容读，缺席 / 空串 / 非字符串统一归 `null`；`CheckListInput` 与 `isCheckListFinish` 判据零改（MUST NOT 加「`title` 在场」前置条件）
- itemTitle(value: unknown) -> string | null（模块私有）: `textOf` + 空串归 `null` 的单点归一化；`pass` / `evidence` 的既有读取路径零改动（不做整体归一化，DD5）
- CheckListCard 逐项 FieldLabel: 文本为「`title` 为 null 时恰为 `item.id`，否则为 `item.id` + 单个半角空格 + `title` 原文」（不 trim、不折叠、不截断）；未通过项的 `text-destructive` 挂同一节点（整标签着色）；`Checkbox` / `FieldDescription` / `Field` / `data-testid` / 摘要行文案零改

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| FieldLabel 标签全貌 | 正向 | 两项夹具补 `title`（`C1` + `变更代码中无安全漏洞`、`C2` + `无遗留调试输出`，一 pass 一 fail）→ 展开后 `[data-slot="field-label"]` 文本逐字为 `C1 变更代码中无安全漏洞` / `C2 无遗留调试输出`：`id` 在前、恰一个半角空格分隔、标题原文不截断（断言全等而非 `toContain`） | 改造（`FINISH_EVALUATOR_INPUT` 补 title + 既有 `toContain('C1')` 升为全等） |
| FieldLabel 着色覆盖整标签 | 正向 | 未通过项（`pass: false` + 有 title）标签 `classList.contains('text-destructive') === true` 且该节点文本含标题（同一节点，非两个兄弟节点各自着色）；通过项标签不含该类 | 改造（既有着色用例扩为「含标题的整标签」） |
| 摘要行 / evidence / 结果区零改 | 正向 | 同一夹具：摘要行为「检查 1 项未通过」；`evidence` 仍在 `[data-slot="field-description"]` 且文本逐字；`block-tool-result` 零节点；`data-tool-id` 与 `block-tool-use` 在场 | 回归 |
| FieldLabel 标签（多行对位） | 正向 | 100 项各带 `id` + `title`：逐行标签文本等于 `` `${id} ${title}` ``（序号与 ID 对位、不串行），行数为 100 | 新增 |
| title 缺席 / 非字符串降级 | 异常 | 六形态参数化：元素无 `title` 键、`title: null`、`title: 42`、`title: true`、`title: { a: 1 }`、`title: ['x']` → 逐形态标签文本恰等于 `id`（全等）：无占位符、无 `undefined` / `null` / `[object Object]` 文本、渲染不抛错 | 新增 |
| title 空串降级 | 边界 | `title: ''` → 标签文本恰等于 `id`（空串归 `null`，不产生尾随空格）；同形态下 `evidence` 行与摘要行与既有行为逐字一致 | 新增 |
| title 纯空白（不 trim 口径） | 边界 | `title: '   '` → 渲染不抛错、标签文本以 `C1` 起且不含 `undefined` / `null` 字样（DD6：显示值不 trim，视觉等价 id-only；不断言被 trim 掉） | 新增 |
| 形态判据零改（不回退 JSON） | 异常 | 同时满足「`report` 为字符串 + `items` 为数组」且元素 `title` 全缺席的输入 → 仍为 checklist 形态（`block-finish-step-item` 在场、无 `<pre>` JSON 回退面板、摘要非 `finish_step`）：`isCheckListFinish` MUST NOT 加 title 前置条件 | 新增 |
| 形态判据零改（JSON 回退半边） | 异常 | `input` 为 `null` / 裸字符串 / 数组 / `{}` / `{ items: 'str' }` / `{ action: 42 }` → 摘要逐字 `finish_step` 且折叠区为 `JSON.stringify(input, null, 2)`（既有回退参数化用例零改） | 回归 |
| decision 形态零改 | 异常 | `backtrack` / `stop` / `ask` / 未知 action 四形态摘要逐字不变（`backtrack → test-gen：覆盖不足` / `stop：受控终止` / `ask：…（甲 / 乙）` / `retry`）；缺字段六形态摘要逐字不变；decision 分支不落 JSON 回退面板；`output` 在场时呈 `block-tool-result`、`output.isError` 时 `data-status="error"`（既有用例零改） | 回归 |
| pass / evidence 读取路径零改 | 边界 | `pass` 非布尔（`'true'` / `1` / `'false'`）仍按真值判定勾选与着色；`pass` 缺席仍为未勾选；`evidence` 空串 / 缺席仍无 `field-description` 节点；title 在场不改变这些判定（既有参数化用例零改 + 新增含 title 的位置一行） | 改造（既有参数化表补 title 列位，判定断言零改） |
| title 超长不截断 | 边界 | `title` 为 40 全角字符（最长真实项量级）与 1200 字符两形态 → 标签文本全等 `` `${id} ${title}` ``（长度等于拼接期望长度、无省略号 `…`、无 `truncate` / `line-clamp` 类导致的文本变化） | 新增 |
| title 特殊字符 | 边界 | `title` 含换行 / NUL / emoji / 全角标点 → 渲染不抛错、标签 `textContent` 含原文片段、无 `script` 节点（React 转义，无 `dangerouslySetInnerHTML`）；后续行与摘要行结构不被破坏 | 新增 |
| items 集合边界（含 title） | 边界 | `items: []` → 摘要「检查通过」且零逐项行（title 面不引入空态文案）；`items` 单元素 → 一行；`items` 非数组（`null` / `'str'`）→ JSON 回退（既有用例零改 + 新增含 title 的单元素行） | 改造（既有集合边界用例扩一行含 title 形态） |
| 多余字段静默忽略 | 边界 | 元素含多余键（`title2` / `note` / `extra: {}`）→ 标签文本仍为 `id + title`，多余键零影响（禁止第二标题来源） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/plugin-opener`（外壳 IPC，进程边界） | `vi.hoisted` + `vi.mock` 注入 `openUrl` 替身（既有装置沿用，仅 WebSearchCard 相关用例消费；finish_step 用例零调用，`beforeEach` 复位） | 全部行（装置保持，零新增 mock） |
| 其余件（`Checkbox` / `Field*` / `TraceCollapsible*` / `Card*` / `TextBlock` / react-markdown） | 真实组合，零 mock：私有 `itemTitle` 与私有 `FinishItem` 经 `FinishStepCard` → `CheckListCard` 公开路径间接驱动（不为测试新增导出） | 全部行 |

---

## 不可测试项

- `packages/desktop/package.json` 的 `version` 行（DD7：相对落地时 `HEAD` 现值 +1 patch，当前盘面 `0.6.51` → 期望 `0.6.52`）— **原因**：版本纪律为静态交付面（`git diff HEAD -- packages/desktop/package.json` 判「恰好 +1 patch」），无可执行代码单元与自动化断言面；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` crate 版本行**不跟**，二者出现版本 diff 即越界（由 static-check / 交付门核）。
- AC-6 的三条 `grep -c "逐字"` 命令行判据（`finish.rs` == 1 / `evaluator_protocol` 函数体 == 0 / `verdict.rs` == 0）— **原因**：机械静态判据，无独立被测单元；**处置**：已折算为三处源码自证断言（`finish_test.rs` 唯一次定位行、`prompt_test.rs` 函数切片零命中、`verdict_test.rs` 全文零命中），命令行的 `grep` 复核归 static-check 相位（不做用例重复执行）。
- AC-7 的全量回归聚合面（既有 13 个 orchestration 测试模块 + 前端组件套件零改即绿、`vp check` / `knip` / `cargo clippy` / `bindings:export` 零 diff）— **原因**：执行聚合面，由本文件各章节用例全集在全量套件运行时综合承载（test-execution / static-check 阶段承接），无独立被测单元与新增用例；`bindings:export` 零 diff 的判据是「本变更不出线新 IPC 类型」（`FinishSpec` / `ChecklistItem` 均不出线），无断言面。
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`（`finish_args()` 与 `finish_step的err文案三要素可见` 的合法清单夹具补 `title`）— **原因**：design「影响传导」登记的**夹具同步**点位，其所属源模块 `loop.rs` 在本变更零改动（不在修改文件表内），故不设 per-file 章节；夹具补位后断言零改即通过（DD3 追加序使 `:5914` 的「清单外 ID」用例无需改动），title 校验行为面由 `verdict_test.rs` 章节承载。**处置**：test-gen 相位仅做夹具补位，不新增该文件用例。
- `packages/desktop/src/components/agent/message-cards/tool-cards.test.tsx`（evaluator 形态夹具无 `title`）— **原因**：零影响（`title` 缺席即 id-only 降级路径，既有断言「行数 / 不抛错 / 摘要行」照旧成立）；源模块 `tool-cards.tsx` 零改动，不设章节、零用例改动。
- `finish.rs::validate`（两臂分派）与 `validate_checklist` 的对外出口链路（`loop.rs:691` 的 `is_error` ToolResult 回灌）— **原因**：design 公共函数 / API 表显式不列 `validate`（实现逐字不变、仅两臂分派），「`Err` 经 is_error ToolResult 回灌」属 `infra/agent` 既有行为（本变更零改动）；`title` 校验的生效面经 `validate_checklist` 直接断言承载，`validate` 分派由既有 `validate分派evaluator臂…` 用例零改即证。
- `packages/desktop/src/types/generated/**` 与 `crates/core/workflow/src/model/{checklist,domain}.rs`、`crates/infra/store/**`、`packages/desktop/src/views/changes/renderers/**` — **原因**：均属本变更「不要修改」清单内的零改动面（`title` 不是任何出线 / 落库 / 抽屉模型字段）；其零变化由 `grep` / `git diff` 边界比对与既有用例零改承载，不建共置测试。
- R3（被拒提交仍按 checklist 形态呈现模型原文标题）与 R5（旧会话卡片只有 `id`）— **原因**：既有 / 既定行为，proposal 明列「明确不做」与 D5「不回填、不加占位」；本变更不新增用例断言该形态（`AC-1` 只断言通过形态），避免把知情边界固化成契约。
