# 设计: archi-scaffold-field-dictionary

> **变更**: archi-scaffold-field-dictionary（`archi_scaffold` 升级为字段词典 · P2）
> **日期**: 2026-11-24
> **基于**: `proposal.md`（问题四节 + 提案六节 + 开单前实测五条 + 2 份 delta + AC-1…AC-9 + R-1…R-8）、`explore.md`（只读输入）、`specs/desktop-archi-agent`（MODIFIED ×1）与 `specs/desktop-archi-model`（ADDED ×1）
> **性质**: **模板文本补三块五行 + 字段面常量一层 + 三面机械锚 + 描述面去枚举 + patch 版本 +1**——零新参数、零新字段、零新诊断词、零 IPC / DTO / UI 变化、零生成物 diff、零模型元素与关系变化。
> **不含**: `mode = "draft" | "dict"` 双形态；纯 Markdown 词典；按字段查询（`field = "path"`）；`archi_module` 响应附「该元素可改字段」；知识骨架「字段纪律」段压缩与相位 prompt 的「取词典」指路句（归主篇切片 A）。

---

## 起点实况（`HEAD` 基线实测）

| 面 | 实测（文件:行） |
|---|---|
| 模板单点 | `sdk/archi_tool.rs:198` `const SCAFFOLD_TEMPLATE`（**1885 B / 37 行**，本次实测）；`:202` `kind = "__KIND__"` 是草稿里**唯一**未注释赋值行；八字段纪律行 `:203`（`# kind：`）/ `:205`（`# gate：`）/ `:208`（`# api：`）/ `:211`（`# entry：`）/ `:214`（`# path：`）/ `:218`（`# doc：`）/ `:221`（`# summary：`）/ `:224`（`# metadata：`）；关系块 `:227`–`:233`（`# apis：` 在 `:233`） |
| 三处值面缺口 | `:211` `entry` 行无「单值」；`:224` `metadata` 单行、无「值 MUST 是字符串」；`:233` `apis` 单行、缺「逐项非空 / 同名不重复 / `apis = []` 与省略同义 / 显式空数组被拒」 |
| 执行体（零改动面） | `:110` `fn scaffold_payload(&self, input: &Value) -> Result<Value, String>`（`kind` 可选 / 词汇外 `Err` 点名四值 / `{ draft }` 形状）；`:237` `fn scaffold_draft(kind: archi::dto::ElementKind) -> String`（`__KIND__` 单点替换） |
| 字段面 | `parse/toml.rs:33` `struct RawElement`（八字段，声明序 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`；`///` 与 `#[serde(default)]` 行穿插；字段行均以 `,` 收尾）；`:61` `struct RawRelation` |
| 模块出口 | `parse/mod.rs:10` `pub mod toml;` / `:12` `pub use toml::TomlParser;`（再导出的既有形态） |
| 描述面三处 | `sdk/mount.rs:59`–`:61` `ARCHI_SCAFFOLD_TOOL` 常量 doc / `:343`–`:344` `archi_scaffold_definition()` fn doc 首段 / `:351`–`:355` description 的 `concat!` 三行（均枚举四枚字段名短标签） |
| 断言面（既有，逐字保留） | `archi_tool_test.rs` 九支 scaffold 用例（`:963` / `:990` / `:1016` / `:1043` / `:1064` / `:1082` / `:1108` / `:1133` / `:1145`）；`mount_test.rs:1604` `archi_scaffold定义描述要素`（四枚短标签 + 三条要素）；`toml_test.rs:1761` `解析doc白名单源码自证`（`include_str!("toml.rs")` 范式）；`model_test.rs:384` `element字段数恰十一个且两个指针各一处`（结构体块扫描范式） |
| 封闭词汇 | `dto.rs:42` `ElementKind::ALL`（4）/ `:76` `GateTier::ALL`（3，`Default = Off`）/ `:107` `ApiStatus::ALL`（2，`Default = Declared`）——三组均带 `Default`，缺省档可派生 |
| 契约状态 | `graph/INDEX.toml:24` `Backend.Core.Archi` = `absent`（`types = 0` / `ops = 0`）/ `:87` `Backend.Infra.AgentRuntime` = `absent`（同）⇒ 零契约资产可对拍 |
| 读数基线 | `reports/archi/20261124T0416454735342Z.json`：55 元素 / 129 关系 / `byGate.off` 55 / 结构诊断 0 / `modelWarnings` 0 / `observations` 53 / `apiFindings` 39（全 `docMissing`）/ `errors` / `warnings` 空 / `status = clean` / `passed = true` / `modelHash` `sha256:6cc50dbf…` / `scan` = 422 scanned / 359 excluded / 422 unmatched / 0 matched / 0 unresolved。本次 `archi_drift(summary)` 复读同一 `modelHash`（`durationMs` 570），`status = clean` / `passed = true` 逐字不变 |
| 版本 | `packages/desktop/package.json:3` = `0.7.9`；`src-tauri/tauri.conf.json` = `"version": "../package.json"`；`src-tauri/Cargo.toml` 自立版本行（不跟随） |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **模板补三块、共五行注释，不是「四处」**：proposal 标题写「补四处」，但表内枚举三行（`entry` / `metadata` / `apis`），delta 的 Module Contract 记「改五处注释 / 零改执行体」——实现面 = **3 个字段块 / 5 行注释**（`entry` 1 行；`metadata` 1 → 2 行；`apis` 1 → 2 行）。三块之外零字段行、零裸 TOML 行、零新占位符。
2. **「恰有一个行首载体」与模板实况不符**：模板里 `path` 有 **3 个**行首载体（`# path：` + 两条 `# path = `）、`kind` 有 **2 个**（`kind = ` 实值行 + `# kind：`）、其余六个字段各 **2 个**（`# {f}：` 纪律行 + `# {f} = ` 示例行）。因此 delta Scenario 的「恰有一个」只能读作**存在**语义；判据落成两条互不冲突的断言（见事实校准 3 与 D4）。
3. **「三形态并集」单判会让 AC-2 的两条反向断言失效**：若只断言「三形态任一存在」，删掉 `# api：` 后 `# api = "none"` 仍在场、删掉 `kind = ` 实值行后 `# kind：` 仍在场——两条「必红」都判不出来。本设计取**三元判据**：① 三形态并集（delta 的字面要求，保留）；② 纪律行行首 `# {f}：`（八项，删 `# api：` 即红）；③ **唯一实值行**（草稿内未注释的赋值行恰一条、其键 ∈ 常量，删 `kind = ` 即红）。三条并集覆盖 AC-2 正向与两条反向，且零手抄字段名字面量。
4. **描述面有三处持有，proposal 只点了两处**：除 `ARCHI_SCAFFOLD_TOOL` 常量 doc 与 description 字面外，`archi_scaffold_definition()` 的 fn doc 首段（`mount.rs:343`）也枚举四枚短标签。三处同批改；改后全仓（除测试）`内联 metadata` / `字段序 / 内联` 零命中，可用 grep 直接复核（D7）。
5. **`# {f} = ` 示例行是草稿的可抄改面，MUST NOT 删**：`# gate = "off"` / `# path = [...]` / `# metadata = { … }` / `# apis = ["ExportedName"]` 既是示例也是「注释外零非法行」的形态锚；本变更只改纪律行文本，示例行逐字不动。
6. **零生成物是结构性的，不是巧合**：两个受影响 crate 的元素在 `INDEX.toml` 均 `absent`（`types = 0` / `ops = 0`），`pub const` 无 serde / specta，`lib.rs` 根出口零改动 ⇒ `api:export` / `api:check` / `bindings:export` 预期零 diff（实现相位复核，AC-7）。
7. **体量读数取代估算**：proposal 估「~4.5 KB」，实测**模板正文 1885 B / 37 行 → 2213 B / 39 行（+328 B）**；`{"draft":"…"}` 经 `serde_json` 转义后 **1963 B → 2293 B**，≪ `tools::MAX_RESULT_BYTES`（50 000 B）。AC-5 的体量读数以实测入账。
8. **源码自证扫描的定位锚不受新常量干扰**：`include_str!("toml.rs")` 找 `struct RawElement {` 后取到首个 `\n}`；`RawElement` 块体内无嵌套花括号、无 `pub` 字段（故 `model_test.rs` 的 `pub ` 过滤器不能照搬，改滤 `///` 与 `#[`）。**新常量的 doc 注释 MUST NOT 含 `struct RawElement {` 字面**，否则定位锚提前命中。
9. **常量是「第二份清单」但消费侧零手抄**：`ELEMENT_FIELD_NAMES` 贴在字段定义旁；`RawElement` 增删字段会因 `[&str; 8]` 的类型元数 + 源码自证对拍两面同时逼更新；两个消费面（`archi_tool_test` 遍历 + `toml_test` 对拍）都不重抄八个字面量。
10. **`kind` 的 `# kind：` 纪律行是四值语义的唯一承载**：既有用例 `archi_scaffold模板kind行四值语义` 逐值断言语义词，`apis` / `api` 的前缀差不会假命中——本变更不动该行。
11. **再导出的导入序取 fmt 实序（`TomlParser` 在前、`ELEMENT_FIELD_NAMES` 在后）**：rustfmt 的 version-sort 把 CamelCase 排在 SHOUTY 之前——先例 `sdk/archi_tool.rs:19` `{…, drift_summary, DRIFT_SCOPES}`、`sdk/db_tool_test.rs:23` `{DbContext, ENVELOPE_RESERVE_BYTES, …}`；`parse/mod.rs:12` 落地后即 `pub use toml::{TomlParser, ELEMENT_FIELD_NAMES};`，`cargo fmt --all --check` 零退出。任何文稿把它反过来写都只是文本脏序（会被 fmt 重排），实现 MUST 取 fmt 实序、零改动。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 实施序 | 四段：① `parse/toml.rs` 常量 + `parse/mod.rs` 再导出 → ② `SCAFFOLD_TEMPLATE` 三块五行 → ③ `mount.rs` 描述面三处 + `package.json` 版本 → ④ 生成物 / 读数 / 白名单零扩散核对。测试面归测试相位（用例名 / needle / 反向锚见末节交接清单） | 常量先落，infra 侧断言才能 `use archi::parse::ELEMENT_FIELD_NAMES`；词典正文与描述面无编译依赖，可在常量之后一次收口；生成步骤只在 ④ 跑一次 |
| D2 | 常量落点与形态 | `pub const ELEMENT_FIELD_NAMES: [&str; 8]` 落在 `struct RawElement`（`toml.rs:33`）之后、`struct RawRelation`（`:61`）之前；doc 三句（机器可读副本 / 权威是字段定义且同批维护 / 消费侧遍历不手抄 + 命名带元素前缀的理由）；**不进 `lib.rs` 根 `pub use`**，只经 `parse` 出口 | 贴在字段定义旁（新增字段时视线所及）；`[&str; 8]` 的元数本身即编译期锚；字段面清单不是跨 crate 通用词汇（delta 的 Module Contract 明写 `lib.rs` 零改动） |
| D3 | 源码自证对拍 | `include_str!("toml.rs")` → `find("struct RawElement {")` → 截到首个 `\n}` → `trim()` 后滤 `///` 与 `#[` 行、只留以 `,` 收尾的行、取首个 `:` 前的名字；三步断言（**双向差集**点名缺失 / 多余 → **同序** `assert_eq!` → 常量**恰八项**）；消费 `crate::parse::ELEMENT_FIELD_NAMES`（顺带证再导出） | 范式同 `model_test.rs:384` 与 `toml_test.rs:1761`；反向可判（`RawElement` 增删改名而常量未同批 ⇒ 红）；`docs` 零命中不重复断言（既有 `解析doc白名单源码自证` 已持） |
| D4 | 字段全集判据 | **三元**：① 三形态并集（`{f} = ` / `# {f}：` / `# {f} = `，delta 字面）；② 纪律行行首 `# {f}：`（八项）；③ 唯一实值行（未注释赋值行恰一条且键 ∈ 常量）。测试 doc MUST 点名反例「`apis` 不得顶替 `api`」 | 事实校准 2 / 3：单判成立不了 AC-2 的两条反向断言；三元里 ② 承正向的「删 `# api：` 即红」、③ 承「删 `kind = ` 即红」，① 保证与 delta 字面不冲突（零成本） |
| D5 | 值域派生对拍 | `# gate：` / `# api：` 行**行首定位后行内**逐值断言（`GateTier::ALL` / `ApiStatus::ALL` 的 `as_str()`）；缺省档由 `GateTier::default()` / `ApiStatus::default()` **派生**（零手抄 `off` / `declared`），断言纪律行含 `缺省 {缺省值}`；`ElementKind::ALL` 四值语义沿用既有用例 | 全串匹配会假命中：`declared` 也出现在 `# doc：` 行、`none` 也出现在别处；行内限定是既有 `archi_scaffold模板path行出路` 的成熟形态。缺省档从 `Default` 派生而非字面量，新增变体即红 |
| D6 | 值面纪律断言形态 | `metadata` / `apis` 按**块**取：从 `# {f}：` 行起到**下一个空行**止（续行是缩进对齐的 `#` 行）；`entry` 的「单值」按 `# entry：` **行内**限定（`# doc：单值指针` 不顶替） | 事实校准 5：`metadata` 的「子表与内联表等价」移到续行，单行断言必假；块取法对「一行 → 两行」的扩张免疫 |
| D7 | 描述面去枚举 | `mount.rs` **三处**同批改：常量 doc / fn doc 首段 / description `concat!` 三行；新措辞 = 「字段词典 / 逐字段一行纪律与值域 / 关系四键模板」；fn doc 第二段（`kind` enum 派生 + `TomlParser` 零诊断自证锚）逐字保留；参数面（`kind` enum）零改动 | 「描述面不枚举字段名」是 delta 的硬条；doc 注释虽不进 agent 上下文，留着枚举会让全仓 grep 复核出噪声、也会在字段面扩容时静默陈旧（事实校准 4） |
| D8 | 模板 const 的 doc 注释零改动 | `archi_tool.rs:192`–`:197` 的 `///` 逐字不动（它只描述形状约束，不枚举字段） | 「字段词典」措辞的单点在 `mount.rs` 描述面；两处持同一结论即第二份事实源。形状约束（唯一实值行 / 关系全注释 / 单段占位 FQN）仍准确 |
| D9 | 零诊断词纪律 | 模板正文只写行为（「非字符串值被拒」），`invalidValue` / `unknownField` / `missingField` / `syntaxError` 零命中；既有 `archi_scaffold草稿零结构诊断词` 逐字保留并约束新文本 | proposal「问题 · 四」的裁决：语义在场、词面不进；词典职责是值面规则，不是错误词表 |
| D10 | 版本 | `packages/desktop/package.json` `0.7.9` → `0.7.10`；`tauri.conf.json` 仍 `"../package.json"`；`src-tauri/Cargo.toml` 版本行逐字不变 | AGENT.md 版本纪律（AC-8） |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 字段面清单（新） | 元素字段名的机器可读副本：八项、顺序 = `RawElement` 声明序；供 `archi_scaffold` 的字段全集断言遍历 | `crates/core/archi/src/parse/toml.rs` | 无（纯常量，零 serde / 零 specta） | Rust `pub const [&str; 8]` |
| 模块出口（改） | 把常量与 `TomlParser` 一起从 `parse` 出口导出 | `crates/core/archi/src/parse/mod.rs` | `toml` 子模块 | 既有 `pub use` 形态 |
| 字段词典（改） | 草稿即词典：八字段逐字段一行纪律（值域 / 缺省 / 基数 / 单值）+ 关系四键模板；仍为可抄改 TOML 且经 `TomlParser` 零诊断 | `crates/infra/agent/src/sdk/archi_tool.rs`（`SCAFFOLD_TEMPLATE`） | `archi::dto::ElementKind`（`__KIND__` 替换） | 字面串 + 单点替换 |
| 工具定义面（改） | 描述改词典措辞、去字段名枚举；参数面与档位零变化 | `crates/infra/agent/src/sdk/mount.rs` | `archi::dto::{ElementKind, GateTier}` | `ToolDefinition` + `concat!` |

**依赖方向零新增边**：`core/archi` 仍不依赖任何 infra crate；`infra/agent` → `core/archi` 既有依赖不变（本变更只多消费一个 `pub const`）。零新 crate 依赖、零新模块、零新文件。

### 数据流（改后）

```text
RawElement（toml.rs:33，权威字段面）
      │  同批维护（源码自证对拍：struct 块内字段名 ⇄ 常量）
      ▼
parse::ELEMENT_FIELD_NAMES  ──再导出──▶  archi::parse::ELEMENT_FIELD_NAMES
                                                │
                   ┌────────────────────────────┴───────────────────────┐
                   ▼                                                    ▼
   archi_tool_test：遍历逐字段断言草稿行首载体            parse/toml_test：与 RawElement 同序同集
   （三形态并集 / 纪律行 / 唯一实值行）
                   │
                   ▼
   SCAFFOLD_TEMPLATE（1885 B → 2213 B）── scaffold_draft(__KIND__) ──▶ { "draft": … } ──▶ cap_result
                   ▲
                   └── mount.rs 描述面：字段词典措辞（零字段名枚举）指向这份草稿
```

---

## 数据模型 (Data Model)

**唯一新增数据结构**：`ELEMENT_FIELD_NAMES: [&str; 8]`——字段**名**的封闭白名单，不持久化、不序列化、不进 IPC / bindings / 生成物。

| # | 字段 | 声明序位 | 草稿里的纪律行首载体 | 纪律面（值域 / 缺省 / 基数 / 单值） |
|---|---|---|---|---|
| 1 | `kind` | 1 | `kind = "__KIND__"`（**唯一实值行**）+ `# kind：` | 四值语义逐值（`ElementKind::ALL`），必填形态由实值行自证 |
| 2 | `gate` | 2 | `# gate：` + `# gate = "off"` | 三值 + 缺省 `off` + ratchet |
| 3 | `api` | 3 | `# api：` + `# api = "none"` | 二值 + 缺省 `declared` |
| 4 | `entry` | 4 | `# entry：` + `# entry = "…"` | **单值** + 选填 + 约定默认三档 |
| 5 | `path` | 5 | `# path：` + 两条 `# path = […]` | 基数随 `kind` + `./` 前缀 + 目录尾 `/` + 文件到扩展名 |
| 6 | `doc` | 6 | `# doc：` + `# doc = "…"` | 单值 + 仅 `api = none` 可写 |
| 7 | `summary` | 7 | `# summary：` + `# summary = "…"` | 一行摘要 |
| 8 | `metadata` | 8 | `# metadata：` + 续行 + `# metadata = { … }` | 字符串 → 字符串（非字符串值被拒）+ 子表与内联表等价 |

**关系四键**（`from` / `to` / `desc` / `apis`）不在本常量内——它是关系表的字段面，词典里以全注释块承载，`apis` 的四条值面纪律写在 `# apis：` 行及其续行。

**关系**：常量 ↔ `RawElement` 是「副本 ↔ 权威」，一致性由源码自证测试机械对拍；常量 ↔ 草稿是「清单 ↔ 视图」，一致性由字段全集断言机械对拍。**持久化：无**（零模型文件改动、零 `.schema.json`、零 `INDEX.toml` 变化）。

---

## 变更清单 (Change Inventory)

<!-- 新增文件（New Files）：本变更零新增文件——常量落在既有单点（parse/toml.rs），断言落在既有测试文件内（MUST NOT 新建文件以保 scan.* 三项计数不变，AC-7）。 -->
<!-- 类型定义（Types）：本变更零类型定义——无 interface / type alias / enum / class 新增或修改；ELEMENT_FIELD_NAMES 是 `[&str; 8]` 常量，登记在「公共函数 / API」表。 -->
<!-- 配置（Config）：本变更零配置键改动——`.dev-team/architecture/config.json`、`crates/core/config/config.schema.json` 与 `KNOWN_KEYS` 均零改动；词典是工具文本，不是工作区配置面。 -->
<!-- 路由 / API 设计：不涉及——零新增 / 删除 IPC 命令、零 HTTP 端点、零 DTO 字段变化（见「路由 / API 设计」段的注释说明）。 -->

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/parse/toml.rs` | ① 在 `struct RawElement`（`:33`）之后、`struct RawRelation`（`:61`）之前新增 `pub const ELEMENT_FIELD_NAMES: [&str; 8]` 及其 doc（逐字见下方「落盘文本 A」）；② doc 注释 MUST NOT 含 `struct RawElement {` 字面（事实校准 8——它是源码自证的定位锚）；③ `RawElement` / `RawDocument` / `RawRelation` / `to_model` / `validate_apis` / `validate_path` / `classify` / `line_of` / `column_of` 与模块 doc 逐字不变 | D2 / D3；零字段增删、零值域变化、零诊断词 |
| `crates/core/archi/src/parse/mod.rs` | `:12` `pub use toml::TomlParser;` → `pub use toml::{TomlParser, ELEMENT_FIELD_NAMES};`（fmt 实序，见事实校准 11）；`pub mod toml;`（`:10`）/ `SourceParser` / `parse_dir` / `relative_path` / `#[cfg(test)] mod mod_test;` / `#[cfg(test)] mod toml_test;` 逐字不变 | 沿 `TomlParser` 的既有重导出形态；`lib.rs` 根出口零改动 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | `SCAFFOLD_TEMPLATE`（`:198`）**三块五行**（逐字见下方「落盘文本 B」；`:211` `entry` 行内改、`:224` `metadata` 1 → 2 行、`:233` `apis` 1 → 2 行）；模板其余行逐字不动（`# kind：` 四值语义、`# gate：` / `# api：` / `# path：` / `# doc：` / `# summary：` 纪律行与全部 `# {f} = …` 示例行、关系块六行）；`scaffold_draft`（`:237`）/ `scaffold_payload`（`:110`）/ 常量 doc（`:192`–`:197`）/ 模块 doc 逐字不变 | D6 / D8 / D9；三块合计 +328 B、+2 行（1885 B / 37 行 → 2213 B / 39 行）；`entry` 行的「单值」与 `apis` 块的 `` `apis = []` `` 带反引号（与本模板既有的 `` `./` `` 同风格） |
| `crates/infra/agent/src/sdk/mount.rs` | 三处描述面同批改（逐字见下方「落盘文本 C」）：① `:59`–`:61` `ARCHI_SCAFFOLD_TOOL` 常量 doc；② `:343`–`:344` `archi_scaffold_definition()` fn doc **首段**（第二段——`kind` enum 派生 / `TomlParser` 零诊断自证锚——逐字保留）；③ `:351`–`:355` description 的 `concat!` 三行；参数面（`kind` enum = `ElementKind::ALL.map(as_str)` + 描述）与 `required: []` 逐字不变 | D7；改后全仓（除测试）`内联 metadata` / `字段序` / `path 必填提示` / `关系模板` 零命中（`关系四键模板` 不含连续子串「关系模板」）；`kind 省略取 domain` / `只读零写盘` / `write / edit` 三条既有 needle 保留 |
| `packages/desktop/package.json` | `"version"`: `0.7.9` → `0.7.10`；其余字段零改动 | D10 / AC-8；`tauri.conf.json` 仍 `"../package.json"`，`src-tauri/Cargo.toml` 版本行逐字不变 |

**落盘文本 A · `crates/core/archi/src/parse/toml.rs`**（紧随 `struct RawElement` 之后；doc 按文件既有宽度折行，内容逐字）：

```rust
/// 元素表字段面清单（机器可读副本）：顺序 = `RawElement` 声明序。
///
/// 权威仍是 `RawElement` 的字段定义；两者同批维护（`toml_test` 的源码自证对拍同序同集），
/// 消费侧（`archi_scaffold` 草稿的字段全集断言）遍历本常量、MUST NOT 手抄字段名字面量。
/// 命名带元素前缀：`parse` 命名空间里裸 `FIELD_NAMES` 指代不明。
pub const ELEMENT_FIELD_NAMES: [&str; 8] = [
    "kind", "gate", "api", "entry", "path", "doc", "summary", "metadata",
];
```

**落盘文本 B · `crates/infra/agent/src/sdk/archi_tool.rs` 的 `SCAFFOLD_TEMPLATE`**（三块五行；块间空行与其余行逐字不动，续行缩进对齐到首行正文列）：

```toml
# entry：单值，只在约定默认指不到时写（repo 相对 POSIX；单文件根 = 根文件自身 / 目录型根 = index.ts(x) / crate = src/lib.rs）

# metadata：选填自由表（字符串 → 字符串）——值 MUST 是字符串（数字 / 布尔等非字符串值被拒）；
#           子表与内联表等价（[element."X".metadata] 写在已声明元素之后零诊断；成为唯一声明才缺 kind）

# apis：选填名字清单（可留空 = 不声明 API 粒度）——只写名字（禁签名 / 类型形状 / `::` 限定路径 / 空白）；
#       逐项非空、同名不重复；`apis = []` 与省略同义（显式空数组被拒）；生成物面留空，差额看 archi_drift
```

**落盘文本 C · `crates/infra/agent/src/sdk/mount.rs`** 三处：

```rust
// ① ARCHI_SCAFFOLD_TOOL 常量 doc
/// `archi_scaffold` 专用只读工具名（新元素 TOML 草稿 = 字段词典：逐字段一行纪律与值域
/// + 关系四键模板）。执行体同上；**只读零写盘**——写面仍是既有 `write` / `edit`，故本
/// 工具不在「专用写工具」之列。

// ② archi_scaffold_definition() fn doc 首段（第二段逐字保留）
/// `archi_scaffold` 定义（字段词典：新元素 TOML 草稿，逐字段一行纪律与值域 + 关系四键
/// 模板；零写盘）。

// ③ description 的 concat! 三行
            "取新元素 TOML 草稿，同时是字段词典：模型元素字段面逐字段一行纪律与值域 + 关系四键模板，",
            "只读零写盘——写面仍是既有 write / edit。kind 省略取 domain，值集见 enum。",
            "草稿可直接抄改（单段占位 FQN，多段时先声明祖先），改完用 archi_drift 自查",
```

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `archi::parse::ELEMENT_FIELD_NAMES` | `crates/core/archi/src/parse/toml.rs`（定义）/ `parse/mod.rs`（再导出） | **新增** | `pub const ELEMENT_FIELD_NAMES: [&str; 8]` | 元素字段面清单，顺序 = `RawElement` 声明序（`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）；零 serde / 零 specta；不进 `lib.rs` 根出口 |
| `archi_scaffold`（工具面） | 定义面 `crates/infra/agent/src/sdk/mount.rs`；执行体 `sdk/archi_tool.rs` | 修改（**仅 description 文本**） | `archi_scaffold(kind?: "package" \| "domain" \| "module" \| "composite") -> { "draft": string }` | 描述改词典措辞、零字段名枚举；`kind` enum 仍由 `ElementKind::ALL` 派生，词汇外 `is_error` 点名四值，响应形状与 `cap_result` 单点不变 |

<!-- 私有函数：`scaffold_draft(kind: archi::dto::ElementKind) -> String` 与 `scaffold_payload(&self, input: &Value) -> Result<Value, String>` 均**零改动**，按「私有函数不列」口径不登记。 -->

---

## 路由 / API 设计 (Route / API Design)

<!-- 不涉及：本变更零新增 / 删除 IPC 命令、零 HTTP 端点、零 DTO 字段变化、零 auth 面改动。九条 archi_* 命令与五 archi 工具的 (phase, role) 注入矩阵逐字不变；工具面唯一可见变化是 `archi_scaffold` 的 description 文本（上面「公共函数 / API」表已登记）。 -->

---

## 依赖 (Dependencies)

### 运行时依赖

零新增。`crates/core/archi` 无需新 crate（常量是纯 `&'static str` 数组）；`crates/infra/agent` 无需新 crate（模板是字面串，断言消费既有的 `archi::parse` 出口）；前端零新包。

### 构建 / 测试依赖

零新增。复用既有脚本：`pnpm -C packages/desktop run api:export` / `api:check`（本变更预期零 diff）、`pnpm -C packages/desktop run bindings:export`（预期 `git diff` 为零）、`pnpm check`、`pnpm test`（后两者由桌面端 static-check / test-execution 执行）。语料 / 金样零新 case。

---

## 验收标准回应

| AC | 设计落点 |
|---|---|
| AC-1 | D6 / D9 + 「修改文件」`archi_tool.rs` 行的三块逐字文本：`entry` 行含「单值」（按 `# entry：` 行内限定，`# doc：单值指针` 不顶替）；`metadata` 块含「值 MUST 是字符串」「非字符串值被拒」+ 既有「子表与内联表等价」；`apis` 块含「只写名字」「可留空」「逐项非空」「同名不重复」「显式空数组被拒」「生成物面留空」「archi_drift」与整句「`apis = []` 与省略同义」（含反引号）。`invalidValue` 零命中（D9），既有零诊断词用例逐字保留。needle 与块取法见末节交接清单第 2 行 |
| AC-2 | D4 三元判据 + 末节交接清单第 2 行：遍历 `archi::parse::ELEMENT_FIELD_NAMES`、测试内零手抄八字面量；反向可判两条（删 `# api：` ⇒ 纪律行断言红，`apis` 不顶替 `api`；删 `kind = ` ⇒ 唯一实值行断言红）。测试 doc 点名反例 |
| AC-3 | D2 / D3 + 末节交接清单第 1 行：常量恰八项、顺序 = 声明序；`include_str!("toml.rs")` 取 `struct RawElement { … }` 块内字段名（滤 `///` 与 `#[`）、双向差集 + 同序断言；`RawElement` 增删改名而常量未同批即红 |
| AC-4 | D5 + 末节交接清单第 2 行：`# gate：` / `# api：` 行内逐值（`GateTier::ALL` / `ApiStatus::ALL`）+ 缺省档由 `Default` 派生（`缺省 off` / `缺省 declared`）；`ElementKind::ALL` 四值语义的既有断言零改动 |
| AC-5 | 零改执行体（`scaffold_draft` / `scaffold_payload` / 参数与响应形状）+ 既有九支 scaffold 用例逐字保留；草稿仍全注释（唯一实值行 = `kind`、关系块全注释）；体量实测 2213 B / 39 行、响应 2293 B ≪ 50 000 B（事实校准 7），`cap_result` 单点不变 |
| AC-6 | D7 + 末节交接清单第 3 行：描述含「字段词典」「逐字段一行纪律」「值域」「关系四键」+ 既有三条（`kind 省略取 domain` / `只读零写盘` / `write / edit`）；零「字段序」「内联 metadata」「path 必填提示」「关系模板」（`mount_test` 正反两向断言；`关系四键模板` 不含连续子串「关系模板」） |
| AC-7 | 事实校准 6 + `tasks.md` 阶段四：`INDEX.toml` 两元素 `absent`、常量无 serde / specta ⇒ `api:export` / `api:check` / `bindings:export` 零 diff；模型 55 / 129、`status = clean` / `passed = true`、结构诊断 0 逐字不变；`scan.*` = 422 / 359 / 422 / 0 / 0（零新增 / 删除文件） |
| AC-8 | D10：`0.7.9` → `0.7.10`；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 版本行不变 |
| AC-9 | 本相位只读核对（不改 delta）：恰 2 份 delta（`desktop-archi-agent` MODIFIED ×1 / `desktop-archi-model` ADDED ×1）；MODIFIED 的 `### Requirement: archi_scaffold 只读工具的新元素模板契约` 与基线 `openspec/specs/desktop-archi-agent/spec.md:165` 逐字一致；两份各 7 / 3 个 `#### Scenario:`（四个 `#`、WHEN/THEN 成对）；两份均带 `## Module Contract` |

---

## 待决问题 (Open Questions)

1. **「恰有一个行首载体」只能读作存在语义**：delta Scenario 写「每个字段名都恰有一个行首载体」，但模板里 `path` 有 3 个、`kind` 2 个、其余六个字段各 2 个（事实校准 2）。本设计按「纪律行存在 + 唯一实值行」两条落地（D4），**不改 spec**。若评审要求「恰有一个」逐字成立，须同批改 Scenario 措辞（并解决 `kind` 天然两个载体的矛盾）。
2. **三形态并集单判与 AC-2 的反向断言不可同真**（事实校准 3）：本设计以三元判据同时满足两者；若评审坚持只保留三形态并集，则 AC-2 的两条「必红」须同批撤销（本设计不取——那会让字段全集断言对整行缺失失去判别力）。
3. **proposal「模板补四处」的口径**：与表内三行、delta「五处注释」不一致（事实校准 1）。本设计按 3 块 / 5 行注释实现；若「第四处」另指模板 const 的 doc 注释（`archi_tool.rs:192`–`:197`），本设计明确不取（D8：措辞单点在 `mount.rs` 描述面）。
4. **体量估算与实测的口径差**：proposal 估「~4.5 KB」，实测模板正文 1885 B、响应 2293 B（事实校准 7）。AC-5 以实测入账；若上游的 4.5 KB 是「工具定义 + 草稿 + 系统提示」的合并读数，需指出口径来源（本设计不据此改任何断言）。

---

## 交接给下游相位的随动清单（测试面，不进「变更清单」也不进 `tasks.md`）

> 本相位只写实现，不写用例、不跑测试。下表登记**必改机械面**——用例名 / needle / 块取法 / 反向锚，供测试相位逐条落地；与「修改文件」表文件集不相交，`git status` 的改动集判据取两者并集。

| # | 文件 | 随动面（用例名 / needle / 反向锚） |
|---|---|---|
| 1 | `crates/core/archi/src/parse/toml_test.rs` | **新增** `字段面清单与RawElement同序同集`：① `include_str!("toml.rs")`；② 定位 `struct RawElement {`（首个 `\n}` 收尾）；③ 取名字 = `trim()` 后滤掉 `///` 与 `#[` 行、只留以 `,` 收尾的行、取首个 `:` 前内容；④ 三步断言——双向差集（红时点名缺失 / 多余）、同序 `assert_eq!`、常量恰八项；⑤ 消费 `crate::parse::ELEMENT_FIELD_NAMES`（顺带证 `parse/mod.rs` 再导出）；⑥ doc 明写「常量与字段面同批维护、本测试不手抄八字面量」。`docs` 零命中**不重复断言**（既有 `解析doc白名单源码自证` 已持）。既有用例逐字保留 |
| 2 | `crates/infra/agent/src/sdk/archi_tool_test.rs` | **新增三支**（既有九支逐字保留）：① `archi_scaffold草稿含字段面全集`——遍历 `archi::parse::ELEMENT_FIELD_NAMES`，三元判据（三形态并集 / `# {f}：` 纪律行 / 未注释赋值行恰一条且键 ∈ 常量）；doc MUST 点名反例「`apis` 不得顶替 `api`」；② `archi_scaffold模板值域逐值且零诊断词`——行首定位 `# gate：` / `# api：` 后行内逐值（`GateTier::ALL` / `ApiStatus::ALL` 的 `as_str()`），缺省档由 `GateTier::default()` / `ApiStatus::default()` 派生并断言 `缺省 {缺省值}`；四诊断词零命中；③ `archi_scaffold模板值面纪律四条`——`# entry：` 行含「单值」、`metadata` / `apis` 按块（`# {f}：` 行起到下一空行）断言 AC-1 的 needle 集（含 `` `apis = []` 与省略同义 `` 的反引号逐字）。导入面加 `ApiStatus` / `GateTier`（`archi::dto`）与 `ELEMENT_FIELD_NAMES`（`archi::parse`） |
| 3 | `crates/infra/agent/src/sdk/mount_test.rs` | `archi_scaffold定义描述要素`（`:1604`）：正 needle 换 `字段词典` / `逐字段一行纪律` / `值域` / `关系四键`，保留 `kind 省略取 domain` / `只读零写盘` / `write / edit`；**新增反向断言**四枚旧标签 `字段序` / `内联 metadata` / `path 必填提示` / `关系模板` 零命中；用例 doc（`:1601`）同步为词典措辞。`archi_scaffold定义kind词表派生`（`:1793`）与其余定义面用例零改动 |
