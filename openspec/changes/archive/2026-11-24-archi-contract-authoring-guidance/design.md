# 设计: archi-contract-authoring-guidance

> **变更**: archi-contract-authoring-guidance（契约起草引导与工具描述补位 · P1 前置片）
> **日期**: 2026-11-24
> **基于**: `proposal.md`（五个缺口实测 / 提案三节 / 两份能力 delta / AC-1…AC-10 / R-1…R-8）、`explore.md`（只读输入）、`specs/desktop-archi-api/spec.md`（ADDED 一条）、`specs/desktop-archi-agent/spec.md`（MODIFIED 两条）
> **性质**: **只读面补位**——一处草稿骨架文本（`## 边界` / `## 承载理由` 两段 + 尾部惰性作者注）+ 四条工具描述补位。零新增工具、零新增判据、零新增 DTO / IPC / 前端 / 生成物、零写盘变化。
> **不含**: `docMissing` 文案里的 bootstrap 指路、`bootstrap` 新增参数、自动生成 `api-reexport` fence、注入矩阵扩展、把散文语义（职能 / 边界 / 不变量 / 承载理由）变成机械判据、存量 10 份契约回填作者注。

---

## 起点实况（`HEAD` 基线实测）

| 面 | 实测（文件:行） |
|---|---|
| 草稿生成 | `crates/core/archi/src/api/mod.rs:759` `fn bootstrap_text(fqn: &str, symbols: &[ApiSymbol]) -> String`：`format!` 出 H1 + 职能 / 不变量 / 操作 / 类型四段；`:779`–`:782` 出 `## 类型` + `api-types` fence 后 `push_str("\n```\n")` 直接结束 |
| 草稿入口 | `api/mod.rs:690` `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>`：`elements_of` → `paths::entry_state` → `extract::extract_element` → `bootstrap_text`，全程零写盘 |
| 解析器 fence 跳过 | `api/contract.rs:164` `// 其余 fence（示例代码等）不参与契约，内容不入散文区` + `continue`（非 `api-*` fence 内容**不入** `section.body`） |
| 空段丢弃 | `api/contract.rs:1089` `fn finish_section`：`closed.body.trim().is_empty()` 即不入 `sections`——`## 类型`（正文全在 fence 内）与 `## 操作`（正文只有 `### ` 锚点）当前**都进不了** `sections` |
| 两个抽取面 | `api/contract.rs:1001` `collect_reexport_prose`（`- ` / `* ` 起手且整行无 CJK）+ `:1048` `collect_surface_table`（行首 `\|` + 首列整体反引号）——两者只在 `section.body` 上跑 |
| op 锚点 | `api/contract.rs:177` `strip_prefix("### ")`：只有**行首**的 `### ` 才算 op 锚点，其下必须紧随一个 `api-op` fence |
| 描述面 | `crates/infra/agent/src/sdk/mount.rs:296` `archi_api_definition()`（`concat!` 六段串）、`:252` `archi_module_definition()`（`format!` 单串 + `vocabulary`）；`:45` / `:53` 两名字常量 |
| 路径 DTO | `crates/core/archi/src/dto.rs:305`–`315` `ArchiApiRef { status, contract_path, schema_path, generated_path }`；派生单点 `api/paths.rs:324` `schema_path` / `:338` `generated_path` |
| 冷启动字段 | `api/mod.rs:285` `artifact_check` 出 `enabled`；`:265` `pub fn check` 在 `enabled == false` 时短路空集（`findings.total = 0` 与「零问题」同形）；人面文案 `packages/desktop/src/views/architecture/components/contract-panel.tsx:276`「契约面未启用（应生成集此时只含 `INDEX.toml`）」 |
| 两条出口 | CLI `crates/tools/archi-api-gen/src/main.rs:74`（`println!` 只出 stdout）、工具 `crates/infra/agent/src/sdk/archi_tool.rs:179`（回 `{ draft }` 过 `tools::cap_result`，L1 = `tools.rs:29` `MAX_RESULT_BYTES = 50_000`） |
| 断言面 | `api/mod_test.rs:2002` `bootstrap草稿形态`、`:2100` `bootstrap只出字符串`、`:4988` `注记面entry退出源码自证`、`:5040` `bootstrap空符号集草稿`（断言 `!text.contains("### ")`，`:5048` / `:5070` 两处）；`sdk/mount_test.rs:1169` `archi_api入参schema形态`、`:1199` `工具描述零项目专用命令`、`:1816` `archi_module描述补三句指路`、`:1834` `archi_module描述零字段形态教学`、`:1528` `mount描述enum词面零旧值`；`sdk/archi_tool_test.rs:618` `archi_api_bootstrap出草稿`（与 `api::bootstrap` 逐字相等） |
| 知识骨架 | `crates/infra/agent/src/archi-knowledge.md` 实测 **2597 B**（含行尾换行；LF 归一并 `trim_end` 后 2596 B）；`archi_knowledge_test.rs:478` / `:497` 两条冻结锚 2596 / 2739 |
| 基线读数 | `archi_drift` summary：55 元素 / 129 关系 / `apiFindings` 39（全 `docMissing`，warn 档）/ `modelWarnings` 0 / `status = clean` / `passed = true` / `modelHash sha256:6cc50dbf…` |
| 版本 | `packages/desktop/package.json:3` = **0.7.9**；`src-tauri/tauri.conf.json` = `"../package.json"`；`src-tauri/Cargo.toml` = `0.1.0`（自立版本） |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **六段只有各带正文才进 `sections`**：`finish_section`（`contract.rs:1089`）丢空 body。现草稿过 `parse` 后 `sections` 恰 **2** 段（职能 / 不变量）；只按 proposal 表加 `## 边界` / `## 承载理由` 也只到 **4** 段——`## 操作`（正文只有 `### ` 锚点）与 `## 类型`（正文全在 `api-types` fence 内）永远被丢弃。⇒ `sections.len() == 6` 的机械锚要求这两段各带一行正文；delta 的「占位句形态」bullet 本就写「**六段**的占位正文 SHALL 是含 CJK 的「（待补：…）」句」，故这一层不是新加约束，而是**执行**它。
2. **作者注是唯一结构惰性载体**：`parse:164` 对非 `api-*` fence 显式 `continue`，内容既不进 `body`（故不被两个抽取面看见），也不触发 op 锚点判据；`### ` 段正文反而被 `finish_section` 提前收束、整段落不进 `sections`。⇒ 说明块只能是 `text` fence，放文件末尾；放裸正文 / `## ` / `### ` 三种错位各有装载期后果（proposal 三·节表，delta 末条 scenario）。
3. **作者注里的 `### <op-id>` 会误伤既有锚**：`bootstrap空符号集草稿`（`mod_test.rs:5040`）以 `!text.contains("### ")` 表达「零 op heading」。作者注第一行含 `` `### <op-id>` `` ⇒ 字面断言必红。honest 锚是**行首扫描**（`parse` 用 `strip_prefix("### ")`，只有行首才是锚点）：`!text.lines().any(|line| line.starts_with("### "))`。这是断言面修正，不是作者注让步。
4. **作者注含 `{}` ⇒ 不能进 `format!`**：块内有 `{ "导出名": "直接子元素 FQN" }`，若塞进 `format!("…")` 会编译失败。⇒ 作者注取**模块级 `const`**，`bootstrap_text` 末尾一次 `push_str`，零插值、零条件。
5. **占位句双重惰性**：六段占位一律「（待补：…）」——既非 `- ` / `* ` 起手（`collect_reexport_prose` 零命中），也非 `|` 起手（`collect_surface_table` 零命中），且含 CJK。新增两条占位（`## 操作` / `## 类型`）MUST NOT 含行首 `### `：`###` 会与「零 op 锚点」的行首扫描混淆（事实 3）。
6. **描述面三处缺口与两条禁则**：`archi_api_definition` 现状零 `第一步` / 零 `.schema.json`·`INDEX.toml`·`.i.` / 零 `enabled`·`findings`；`archi_module_definition` 已提 `api.contractPath` 而 `schemaPath` / `generatedPath` 零提及。禁则：`工具描述零项目专用命令`（`pnpm` / `cargo run` / `api:export` / `archi-api-gen`）、`archi_module描述零字段形态教学`（零 ``` 、零「三档」）。
7. **`findings` 进描述不撞既有断言**：`tools_test.rs:1832 archi工具描述零旧多值词` 的词表含 `findings`，但其夹具 `archi_injections(true, true)`（`:1422`）只下发 `archi_module` + `archi_search`，**不含** `archi_api`；`mount_test.rs:1528 mount描述enum词面零旧值` 只禁带引号的 `"docs"`。⇒ 冷启动口径那句可写 `findings`。
8. **两条出口同源，新形态自动覆盖**：CLI 与 `archi_api` 都直出 `api::bootstrap` 的同一份文本；`archi_tool_test.rs:618` 的「逐字相等」断言无需改动即覆盖六段 + 作者注。
9. **零写盘路径不变**：`bootstrap` 只有 `elements_of` / `entry_state` / `extract_element` / `bootstrap_text` 四次调用；改形态不引入 `fs::` 写调用，既有 `bootstrap只出字符串`（`mod_test.rs:2100`）的快照断言继续成立。
10. **体量实测**：作者注 665 B（8 行块含 ```text 围栏，353 字符）、`## 操作` 占位 75 B、`## 类型` 占位 81 B、`## 边界` / `## 承载理由` 两段含分隔 118 B ⇒ **草稿净增 939 B**（proposal R-7 记为「约 +0.6 KB」，实测值以本条为准）。`cap_result` L1 = 50 000 B，余量充足。
11. **知识骨架零字节变化是硬门**：四条描述补位全落 `ToolDefinition.description`，`archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs` 零 diff（2596 / 2739 冻结锚不动）。
12. **前端零消费草稿**：`packages/desktop/src/**` 内零 `api::bootstrap` / 草稿调用（仅 `types/generated/bindings.ts` 的 JSDoc 与 `views/changes/flow/run-state.test.ts` 的 `bootstrap` **阶段名**夹具命中，与本变更无关）⇒ 前端零 diff 成立。
13. **版本**：`0.7.9` → `0.7.10`（AGENT.md 版本纪律：desktop 源码一拍 patch）；`tauri.conf.json` 经 `"../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行不动。
14. **作者注的英文词不撞草稿既有禁词断言**：草稿文本面既有四条否定断言——`!text.contains("unused")`（`mod_test.rs:3732`、`archi-api-gen/src/main_test.rs:851`）、`!text.contains("used_by")` / `"usedBy"` / `"引用方"`（`mod_test.rs:2150`–`2158`）。作者注只引入 `api-op` / `api-types` / `api-reexport` / `MUST NOT` / `enabled` / `findings` / `archi_api` 等词，与上述四条零交集。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 作者注落点 | 文件末尾一个 ```text fence（模块级 `const BOOTSTRAP_NOTE: &str`，零插值）；`bootstrap_text` 在 `## 承载理由` 之后一次 `push_str` | 非 `api-*` fence 是解析器**结构性排除**的唯一载体（事实 2）；`const` 避开 `format!` 的大括号转义（事实 4） |
| D2 | 六段正文 | `## 操作` / `## 类型` 各补一行「（待补：…）」占位，使 `sections.len() == 6` 在**有符号 / 无符号**两形态都成立 | `finish_section` 丢空段（事实 1）；delta「占位句形态」bullet 要求六段占位正文；这样「六段 body 零作者注」从 4 段扩到 6 段全覆盖。**唯一代价**：delta 无符号场景句「`## 操作` 段为空」需按「零 op 锚点」读（见待决 1） |
| D3 | 占位措辞 | `## 操作` → 「（待补：每个操作一个三级标题 + 紧随一个 api-op fence）」；`## 类型` → 「（待补：本模块自有 $defs，一个 api-types fence，同文件唯一）」 | 两条兼作起草引导（与本变更目的同向）；含 CJK、非列表项 / 非表格行、**零行首 `### `**（事实 5） |
| D4 | 六段与作者注的相对序 | H1 / 职能 / 不变量 / 操作（占位 + 各 op）/ 类型（占位 + fence）/ 边界 / 承载理由 / 作者注 | 既有四段的 heading 与前缀逐字保留、次序不动；两段追加在 `## 类型` 之后（proposal 表）；说明块放末尾 = 「可读部分在前、说明在后」 |
| D5 | `archi_api` 描述 | 首句 `（自检）` → `（自检与草稿）`（对齐 delta 的语义句「契约面自检与草稿」）；再加三句：① `bootstrap` 是新建 / 补契约的第一步；② 四类生成物点名 + 保留「过期用既有生成命令重生成，MUST NOT 手改生成物」；③ `artifacts.enabled = false` ⇒ 契约面未启用（`findings` 恒空 ≠ 零问题）。保留 `concat!` 形态与参数 schema | 三句是 proposal 的硬要求（AC-5 needle：`第一步` / `.schema.json` / `INDEX.toml` / `.i.` / `enabled` + `findings` 同句）；首句改动是 4 个字，消除「描述说是自检、delta 说是自检与草稿」的分叉；`findings` 不撞禁则（事实 7） |
| D6 | `archi_module` 描述 | 末尾补第四句「`api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）」，落在 `archi_scaffold` 一句之后、诊断收尾句之前；其余句子逐字保留 | 与 delta 的「四句」序号对齐（hasPath / resolvedEntry / scaffold / 两条路径）；零 ``` 、零「三档」、零本仓命令名 |
| D7 | 不动面 | `contract.rs`（零改动：只**使用**既有惰性语义）、`archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs`（冻结锚）、`crates/core/workflow/src/prompts/*.md`、`.dev-team/architecture/**`（契约与模型）、DTO / IPC / `src/types/generated/**` / 前端、`crates/tools/archi-api-gen/**`、`sdk/archi_tool.rs`、`sdk/archi_tool_test.rs` | proposal「不要修改」逐条；`archi_tool_test.rs:618` 的逐字相等断言自动覆盖新形态（事实 8），无需改 |
| D8 | 版本 | `0.7.9` → `0.7.10` | AGENT.md 版本纪律（事实 13） |
| D9 | 断言面随动 | 交测试相位（本相位不写用例）：`mod_test.rs` 三处（`bootstrap草稿形态` 扩六段 / 新增自证用例 / `bootstrap空符号集草稿` 的 `### ` 锚改行首扫描）+ `mount_test.rs` 两处 needle 扩写 | 事实 3 是**必须**随动的锚（否则新形态下该用例假红）；其余是把新形态钉住的最小新增面 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 草稿骨架（改） | 出 H1 + 六段（`## 操作` / `## 类型` 各带一行占位正文）+ 末尾 `text` fence 作者注；`bootstrap_text` 形参与返回类型不变 | `crates/core/archi/src/api/mod.rs` | `contract`（`is_constructor`）、`extract::ApiSymbol` | `String` 拼接 + 模块级 `const` |
| 契约解析器（零改） | 提供「非 `api-*` fence 惰性」「空段不入 `sections`」「`### ` 行首才是 op 锚点」三条既有语义 | `crates/core/archi/src/api/contract.rs` | — | 行级状态机 |
| 工具描述面（改） | `archi_api` 三句补位（含首句语义对齐）+ `archi_module` 第四句 | `crates/infra/agent/src/sdk/mount.rs` | `vocabulary`、`ElementKind` / `GateTier` | `ToolDefinition.description` 字面串（零 preamble 字节） |
| 工具出口（零改） | `bootstrap` 分支回 `{ draft }`，过 `cap_result` L1 | `crates/infra/agent/src/sdk/archi_tool.rs` | — | 既有单点 |
| CLI 出口（零改） | `archi-api-gen bootstrap <元素>` 打 stdout | `crates/tools/archi-api-gen/src/main.rs` | `archi::api::bootstrap` | 既有单点 |
| 注入知识（零改） | 骨架 / 派生段逐字不变（2596 / 2739 冻结锚） | `crates/infra/agent/src/archi-knowledge.md`、`archi_knowledge.rs` | — | 静态文本 |
| 版本（改） | `0.7.9` → `0.7.10` | `packages/desktop/package.json` | — | JSON |

依赖方向零新增边：`core/archi` 仍只依赖 `foundation`；`infra/agent` → `archi` 既有边不变；本变更零新 crate 依赖。

### 数据流（改后）

```text
element FQN ──api::bootstrap──> entry_state → extract_element → bootstrap_text
                                                                  │
                          ┌───────────────────────────────────────┘
                          ▼
   # <FQN> + 六段（职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由，各带 CJK 占位正文）
        + 各 ### <op-id> + api-op fence（抽取所得）
        + 一个 api-types fence（抽取所得）
        + 末尾 ```text 作者注（静态 const，零插值）
                          │
     CLI println!（stdout）│ 工具 { draft }（cap_result 50000 B）
                          ▼
   作者手写落盘 ──api::contract::parse──> Contract { ops, types, reexports, sections(6), reexport_prose(空) }
                                          （fence 内容不入 body；作者注整块跳过）
```

---

## 变更清单 (Change Inventory)

### 新增文件

<!-- 无新增文件：本变更只改既有三处产品文件与两处断言文件；不新增模块、脚本、数据文件。 -->

### 修改文件

| 文件 | 具体修改内容 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | ① 新增模块级 `const BOOTSTRAP_NOTE: &str`（`bootstrap_text` 上方），内容 = proposal 的 8 行作者注块含 ```text / ``` 围栏，逐字；doc 一句「作者注：解析器对非 `api-*` fence 显式跳过 ⇒ 惰性、零判据；MUST NOT 移出 fence」。② `bootstrap_text`：`## 操作` 后补 `（待补：每个操作一个三级标题 + 紧随一个 api-op fence）\n\n`；`## 类型` 后补 `（待补：本模块自有 $defs，一个 api-types fence，同文件唯一）\n\n`；`api-types` fence 收尾由 `"\n```\n"` 改为 `"\n```\n\n## 边界\n\n（待补：本模块**不**负责什么）\n\n## 承载理由\n\n（待补：为什么它是独立元素）\n\n"` 并 `push_str(BOOTSTRAP_NOTE)`。③ 更新 `bootstrap_text` 的 doc comment（H1 + 六段 + 尾部惰性作者注，并写明说明块为何不能进正文）。④ 其余函数（`bootstrap` / `bootstrap_op` / `bootstrap_types` / `json_type` / `entry_problem_reason`）与错误文案逐字不动 | 三处缺口 1 的正解；`bootstrap` 签名 / 错误面 / 零写盘不变 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | ① `archi_api_definition()`：首句 `（自检）` → `（自检与草稿）`；`bootstrap` 一句补「新建 / 补契约的第一步就取它」；把泛称「生成物」换成四类点名（契约 md 同目录 `<FQN>.schema.json`、架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）并保留「过期用既有生成命令重生成，MUST NOT 手改生成物」；补冷启动口径「`check` 的 `artifacts.enabled = false` 即契约面未启用（此时 `findings` 恒空，不等于零问题）」。② `archi_module_definition()`：`archi_scaffold` 一句之后补「`api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）」。③ 参数 schema、名字常量、`ToolTier` / `ToolExec` 归属、`registry()` / `lookup()` / `definitions()` 逐字不动 | 缺口 2 / 3 / 4 / 5；四条补位全落描述（知识骨架零字节） |
| `packages/desktop/package.json` | `"version"`: `"0.7.9"` → `"0.7.10"` | AGENT.md 版本纪律；`tauri.conf.json` / `Cargo.toml` 不动 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`（测试相位承接） | ① `bootstrap草稿形态`（`:2002`）扩到六段 needle（含 `## 边界` / `## 承载理由` / 段序）；② 新增「草稿过 `contract::parse` 零错误 + `sections.len() == 6` + `reexport_prose` 空 + 六段 body 零「作者注」/零「三类 fence」」与「无符号元素仍六段」两条用例；③ `bootstrap空符号集草稿`（`:5040`）的 `!text.contains("### ")` 改为**行首扫描**（`!text.lines().any(\|line\| line.starts_with("### "))`，两处 `:5048` / `:5070`）；④ 既有 `bootstrap_text` 签名断言（`:4988`）与注记行用例保留 | 事实 1 / 3；只登记必改面，不写用例正文 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`（测试相位承接） | ① `archi_api入参schema形态`（`:1169`）的 needle 扩到 `第一步` / `.schema.json` / `INDEX.toml` / `.i.` / `enabled` / `findings`（既有 `MUST NOT` / `手改生成物` / `重生成` 三条保留）；② `archi_module描述补三句指路`（`:1816`）的 needle 扩到 `schemaPath` / `generatedPath`；③ 两条边界（`工具描述零项目专用命令`、`archi_module描述零字段形态教学`）保持绿 | AC-5 / AC-6 / AC-7 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|---|---|---|---|---|
| `archi::api::bootstrap` | `crates/core/archi/src/api/mod.rs` | 改（返回值文本形态） | `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>` | 签名、错误面（`ApiError::Element` 四类入口文案与元素不存在）、零写盘逐字不变；仅返回的草稿文本按六段 + 作者注。经 `crates/core/archi/src/lib.rs:37` 的逐名 `pub use` 出口 |
| `archi-api-gen bootstrap <元素>`（CLI 子命令） | `crates/tools/archi-api-gen/src/main.rs` | 零变化 | `Command::Bootstrap(String)` → `run(...) -> Result<i32, String>`，退出码 `0` | 仍 `println!("{text}")` 只出 stdout；随 `api::bootstrap` 自动获得新形态 |
| `archi_api`（工具） | 描述在 `crates/infra/agent/src/sdk/mount.rs`，执行体在 `sdk/archi_tool.rs` | 零变化（描述文本除外） | 入参 `{ action: "check" \| "bootstrap", element?: string }`；响应 `{ draft: string }` / `{ artifacts, findings }` | `ToolTier::Read` / `ToolExec::Archi` / `cap_result` 单点 / 注入矩阵不变 |
| `archi_module`（工具） | 描述在 `crates/infra/agent/src/sdk/mount.rs`，执行体在 `sdk/archi_tool.rs` | 零变化（描述文本除外） | 入参 `{ element?: string }`；响应字段集不变（`api.status` / `contractPath` / `schemaPath` / `generatedPath` …） | 只让描述说明三条路径怎么用 |

私有函数（不列，改动落在上表「修改文件」行内）：`bootstrap_text`（形参 `(fqn, symbols)` 与返回 `String` 不变）。

### 类型定义

<!-- 无类型定义变化：`Contract` / `ArchiApiCheck` / `ApiSnapshot` / `ArchiApiFinding` / `ArchiApiRef` 零字段新增；零新增 struct / enum / type alias（作者注是 `&str` 常量，不是类型）。 -->

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 改 | string（semver） | `"0.7.10"` | 0.7.9 → 0.7.10；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行（`0.1.0`）MUST NOT 跟随 |

`src-tauri/crates/core/config/config.schema.json` 与 `.dev-team/config.json` 零改动（无 `KNOWN_KEYS` / `TestFramework` 变化）。

---

## 数据模型 (Data Model)

本变更的数据面只有**文本形态**两处，零持久化实体。

### 1 · 草稿文本（`bootstrap_text` 返回值）

| 段 / 块 | 文本（新形态） | 解析期处理 |
|---|---|---|
| H1 | `# <FQN>` | `file_title` 要求 H1 = 文件名主干（不变） |
| `## 职能` | `（待补：一句职能）` | 进 `sections`（body 非空） |
| `## 不变量` | `（待补）` | 进 `sections` |
| `## 操作` | `（待补：每个操作一个三级标题 + 紧随一个 api-op fence）` + 每个 op 一个 `### <op-id>` + 紧随 `api-op` fence | 占位行进 `sections`；`### ` 起手行是 op 锚点（行首判据）；fence 内容不入 body |
| `## 类型` | `（待补：本模块自有 $defs，一个 api-types fence，同文件唯一）` + 一个 `api-types` fence | 占位行进 `sections`；fence 内容不入 body |
| `## 边界` | `（待补：本模块**不**负责什么）` | 进 `sections` |
| `## 承载理由` | `（待补：为什么它是独立元素）` | 进 `sections` |
| 末尾 `text` fence | 见下（作者注 8 行） | `parse:164` 整块跳过：不进 body、不触发锚点判据 |

作者注原样（`const BOOTSTRAP_NOTE`，665 B）：

````text
```text
作者注（本块不参与契约解析）：
- 三类 fence：api-op（`### <op-id>` 下紧随）/ api-types（本模块自有 $defs，同文件唯一）/
  api-reexport（键 = 导出名、值 = 直接子元素 FQN，同文件唯一）
- 散文只留语义：职能 / 边界 / 不变量 / 承载理由；MUST NOT 用表格复述名字清单
  （名字清单的唯一持有者是三类 fence）
- api-reexport 形态：{ "导出名": "直接子元素 FQN" }；散文里的透传列表须与键集逐名相等
- 起草后跑 archi_api check 看两轴（生成物新鲜度 / 契约判据）；契约面未启用时
  enabled = false 且 findings 恒空
```
````

形态不变量（由断言面钉住）：`sections.len() == 6`（两符号形态皆然）、`reexport_prose` 空集、六段 `body` 零「作者注」与零「三类 fence」、行首 `### ` 计数 = op 数（无符号时为 0）。

### 2 · 工具描述文本

`archi_api`（新描述，逐句要点；实现取 `concat!` 续行，总长 **742 B / 382 字符**，现状 458 B ⇒ 净增 284 B）：

```text
查询模块 API 契约面的两轴事实（自检与草稿）。`check` 返回生成物新鲜度（三类差异：
内容不符 / 应删未删 / 应生成未生成）与契约判据摘要（按 category 计数 + 逐条
kind / element / message）；`bootstrap` 按元素 FQN 输出契约 md 草稿——新建 / 补契约
的第一步就取它（只出 stdout 语义、零写盘）。生成物四类：契约 md 同目录的
`<FQN>.schema.json`、架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`——过期用既有
生成命令重生成，MUST NOT 手改生成物。`check` 的 `artifacts.enabled = false` 即契约面
未启用（此时 `findings` 恒空，不等于零问题）。模型结构错时返回诊断清单（先修模型）
```

`archi_module` 追加句（插在 `archi_scaffold` 一句之后、诊断收尾句之前）：

```text
；`api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）
```

### 持久化

- 草稿**不落盘**：`bootstrap` 只回字符串（stdout / `{ draft }`），落盘是作者的动作。
- 作者注是编译期 `const`，工具描述是运行时 `ToolDefinition.description`；两者零持久化、零配置读取。
- 架构域字节在本变更前后相同（零写盘），`modelHash` 不变（`sha256:6cc50dbf…`）。

---

## 路由 / API 设计 (Route / API Design)

<!-- 不涉及：本变更零新增 / 零修改 IPC 命令、零新增 HTTP 端点、零新增工具名；`archi_api` / `archi_module` 的入参 schema 与响应形状逐字不变，改的只是 `ToolDefinition.description` 文本。 -->

---

## 依赖 (Dependencies)

零新增运行时依赖与构建 / 测试依赖：改动只落在既有 `String` 拼接（`core/archi`）与既有字面串（`infra/agent`），不引入 crate、不引入前端包、不改 `Cargo.toml` / `package.json` 的依赖段。

---

## 待决问题 (Open Questions)

| # | 问题 | 本设计的取法 | 若被驳回的回落 |
|---|---|---|---|
| 1 | delta 的无符号场景句写「`## 操作` 段为空」，而同一 delta 的机械锚写「`sections` SHALL 恰为六段」——`finish_section` 丢空段（事实 1）使两者不可同时成立 | 取**规范面**的那一条（六段各有「（待补：…）」正文，`sections.len() == 6`），场景句按「零 `### ` 起手锚点、零 `api-op` fence」读；建议 spec 相位同批把该句改成这个说法 | 取消 `## 操作` / `## 类型` 两行占位，锚降为 `sections.len() == 4`；此时 proposal AC-3 的 `sections.len() == 6` 与 AC-4 的「六段」口径也须同步收窄，且「六段 body 零作者注」只覆盖 4 段 |
| 2 | 草稿净增实测 939 B（proposal R-7 估「约 +0.6 KB」） | 以实测 939 B 为准，不影响任何判据与 `cap_result` 余量 | 无需回落；若要求压回 0.6 KB，需删作者注第 4 条要点（与 delta 的四要点要求冲突，不推荐） |
| 3 | `archi_api` 描述首句 `（自检）` → `（自检与草稿）` 是否算超出 proposal「三句」的改动 | 保留：4 个字，消除描述与 delta 语义句「契约面自检与草稿」的分叉，且不撞任何禁则 | 回退首句，只留三句补位（描述与 requirement 标题继续分叉，可接受） |
| 4 | `## 操作` / `## 类型` 占位句是否随草稿进入正式契约 | 不设删除纪律（与既有 `（待补：…）` 同处理）；占位含 CJK、非列表项、非表格行 ⇒ 留着也零判据风险 | 若要洁癖：作者起草时删掉占位行（删除后该段 body 为空，`sections` 少一段——但断言只断面**草稿**，不影响装载） |

---

## 验收标准回应

| AC | 落点 |
|---|---|
| AC-1 草稿 H1 + 六段、段序固定 | 修改文件第 1 行（`bootstrap_text`）+ 数据模型 §1；断言面见 `mod_test.rs` 六段 needle |
| AC-2 作者注在末尾非 `api-*` fence 内，含 `api-reexport` 与 `{ "导出名": "直接子元素 FQN" }` | D1 / D3 + 数据模型 §1 的 `const BOOTSTRAP_NOTE` 字面 |
| AC-3 草稿自证可解析（`parse` 零错误、`sections == 6`、`reexport_prose` 空、六段 body 零作者注） | 事实 1 + D2（六段各带 CJK 占位正文是这条的前提）；待决 1 记录 delta 场景句的读法 |
| AC-4 无符号元素仍六段（`## 操作` 零 op 锚点、`api-types` 为 `{}`），仍过 `parse` | 数据模型 §1 表（`bootstrap_types` 零符号返回 `{}` 不变）+ D2；「`## 操作` 段为空」按待决 1 读作「零 op 锚点」 |
| AC-5 `archi_api` 描述三句（`第一步` / `.schema.json`+`INDEX.toml`+`.i.` / `enabled`+`findings` 同句） | 修改文件第 2 行 + 数据模型 §2 的首段逐字文本 |
| AC-6 `archi_module` 描述含 `schemaPath` / `generatedPath` | 修改文件第 2 行 + 数据模型 §2 的追加句 |
| AC-7 两条既有边界仍绿（零本仓专用命令、`archi_module` 零代码围栏） | 新描述零 `pnpm` / `cargo run` / `api:export` / `archi-api-gen`、零 ```、零「三档」（事实 6 / 7） |
| AC-8 知识骨架 / 全文体量锚逐字不变 | D7 + 事实 11：`archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs` 零 diff |
| AC-9 零写盘不变（`bootstrap` 前后架构域字节相同；工具直出与 `api::bootstrap` 逐字相等） | 事实 8 / 9：`bootstrap` 调用链不变，`archi_tool_test.rs:618` 的逐字相等断言自动覆盖新形态 |
| AC-10 零外溢（DTO / IPC / bindings / 四类生成物 / 前端零 diff；`api:check` 与 `client:check` 零差异） | 事实 12 + D7；静态链由桌面端执行（本相位不重复执行） |
