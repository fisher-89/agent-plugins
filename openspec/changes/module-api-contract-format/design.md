# 设计: module-api-contract-format

> **变更**: module-api-contract-format
> **日期（本稿 · 第三十稿：两处「清单外 / 表外」形态的实现面闭合）**: 上一轮的两条 objection 都属**同一类**——「盘上有一份形态没被任何清单收纳、也没被表的口径收纳」，而不是需求或裁定有错，故由实现相位自身闭合、不动 D1–D60 与任何 AC：
> ① **`declare_ancestors` 内联进 `api/mod_test.rs`**：原 `crates/core/archi/src/api/test_support.rs`（47 行、以 `#[cfg(test)] pub(crate) mod` 挂在产品文件 `api/mod.rs` 上）既不在新增 18 / 机械随动白名单 18 / 测试相位 44 任一并集内 ⇒ 交付态实测 **105** ≠ 声明 **104**；把它整条迁入 `api/mod_test.rs`（`pub(crate) fn declare_ancestors`）、撤回 `mod.rs` 的模块声明、四处引用改指 `crate::api::mod_test::declare_ancestors` 后，**104 等式恢复且用例集零变化**（44 文件不变，无需改本节「零新增文件」与 `test-design.md` 的「11 份」两处声明）。
> ② **`emit::rust::variant_name` 撤出公共函数表**：其盘上形态是私有 `fn`，而表内曾写 `pub fn`；按本文件「公共函数 / API」小节纪律①「私有工序一律不入表」删该行（`emit/rust.rs` 行由 **3 行** 改 **2 行**），**实现形态逐字不动**（私有 `fn` 与 `grep -c "fn variant_name"` ⇒ 1 的 D45 闸二判据均不受影响）。
> 两条的完整论证与判据见「与 spec / 提案措辞的 38 处事实校准」第 37 / 38 条与 `tasks.md` 阶段三十 ⑦。
> **日期（前一稿 · 第二十九稿，逐字保留：四条设计面裁定的单次收口）**: 四条失败同源于**设计面未收口**，本稿在设计层一次裁定、再由实现 / 测试相位各自承接（三条裁定各落一条决策 D58 / D59 / D60，第四条与它们同源、只改文本口径）：
> ① **`EmitTarget` 收为三字段**（**D58**）：`path` / `element_root` 两字段产品面**零读取**（产品面只读 `kind` / `form` / `modules`；头注释四个来源全在 `ApiMeta`、诊断定位来源是 `Contract.file`）⇒ 删两字段，构造面只留 `kind` / `form` / `modules`；**「specifier 只由 `form` 与 `ModuleRef.name` 决定」成为唯一口径**——旧文「`element_root` 只影响 TS specifier 的形态语境与诊断定位」MUST NOT 再现（经不起核对：specifier 值 = `paths::ts_import_specifier(form, module.name)`，`element_root` 不在其中）。
> ② **`choice_payload` 删除**（**D59**）：`variants_block` 的分支负载**一律**经 `value_type(choice, Position::Field, common)`；同一 schema 的同一分支经 `value_type` 与经 `variants_block` 产出**逐字相等**（含协议键分支——两路径都经 `common_type` 查表）。**行为变化的射程如实登记**：`$ref` 与四种标量产出不变，**新增**带负载的是协议键 / `array` / 带 `additionalProperties` 的 `object` 三类（此前落 `Variant{index}`，且 TS 侧一直有形态 ⇒ 本裁定把 Rust 侧对齐同一通路）；**试点零影响**（试点契约无 `oneOf` / `anyOf`，四类生成物逐字节不变）。
> ③ **`api/mod.rs` 的 `pub use check::category_of;` 删除**（**D60**）：`category_of` 在产品面零消费者、测试面走 `super::check::` 模块路径 ⇒ 该行只扩大出口面。
> ④ **本稿零新增 / 零删除文件**：实现相位封闭数仍 **60 路径**（清单 42 + 机械随动白名单 18）、测试相位 **12 组 / 44 文件**、交付态并集 **104 路径**（「核对口径」段逐值不变）；`proposal.md` 与九份 delta spec **不改**（零需求面变化）。新增**事实校准 34 / 35 / 36** 三条实测证据。**两段承接**：实现相位做 `tasks.md` 阶段三十 ①②③（产品面：删两字段 / 构造点改三值 / 删 `choice_payload` 与一处 `pub use`）与 ④（**19 处** `EmitTarget {` 构造字面量、两字段初始化行 **38 行**的构造形态适配，D53 第四类随动）；测试相位做 ⑤ 的移交清单（`test-design.md` 的 `emit/mod.rs` 说明段与「单契约粒度」/「三字段面」/「三字段承载」三处用例行的三字段改写、`emit/mod_test.rs` 的五字段用例改判、`contract_test.rs` 的注释词面、`variants_block` 协议键分支的正例）。
> **日期（前一稿，逐字保留）**: 2026-11-11（**第二十八稿 · 路径前缀口径的单口归一**：上一稿有两处 objection 属实、**同根同因**——「表内省略写法」被当成了「集合内容」，而 `/tmp/expected.txt` 与 `/tmp/test_phase_whitelist.txt` 里是**物理路径**（`git status --porcelain` 的路径列带 `packages/desktop/src-tauri/` 前缀）。① `tasks.md` 阶段二十九 ⑥-a 以 `src/commands/change_flow/mod_test.rs` 省略形态取证，而 `expected.txt` 内无任何 `src/…` 起头行（清单与白名单均以 `crates/` 或 `packages/` 起头）⇒ 该串恒 0 命中，既无法证明「不在集合内」、也与 `git status` 里的真实文件名不同一（同一次核实中该文件在盘已改动）；② 「测试相位产出白名单 - 壳命令面用例」行的 3 项写成 `src/…` 省略形态，而 `comm -23 actual_all test_phase_whitelist` 以**物理路径**做集合匹配 ⇒ 该 3 条无一命中，收尾自查 F 的 actual 侧必红（该组不在 66 内，无法在清单侧补救）。**处置（单一口径）**：归一权收进「变更清单 - 核对口径」段的**「路径归一口径」/「`/tmp/expected.txt` 的生成口径」/「判据串口径」三新条**（唯一持有者），并把「路径口径」页头、机械随动白名单说明、测试相位白名单纪律④、`D56`、`AC-33` 五处改为**指向该唯一口径**，壳命令面那 3 项与 AC-33 的判据串改成**物理全路径**。**射程**：仅 `design.md` / `tasks.md` 的路径形态与判据串，**不动** D1–D57、R1–R4 收口、判据基线表（28 / 13 / 其余 0）与三数（66 / 47 / 113）；`proposal.md` 已完成且 pass，**不改**。
> **日期（前一稿，逐字保留）**: 2026-11-11（**第二十七稿 · 四项 objection 的终态文本收口**：四项 objection 全部落在 `design.md` / `tasks.md` 的**终态文本**上——不涉 `proposal.md` 的需求与九份 delta spec（其评估已 pass，且 proposal 白名单内无任一可修项能消除这四项，理由是四项互斥且彼此同源）。本稿**零产品面改动、零裁定形态改动**，只做四件文本闭环：
> ① **「八行 / 九行」的唯一终态源**：本文件「变更清单 - 新增文件」表 `api/mod.rs` 行的**九行**（八 + D55 的 `display`）是**唯一终态源** ⇒ 把「起点核实」段（原 `:50` 写死「八行（与 proposal 一致）」）、`proposal.md` 实现文件表 `api/mod.rs` 行（原写「按 design 公共函数表 **八行**」）、`tasks.md` 阶段九（原写「⇒ **终态 8 个 `pub fn`**」）三处统一改写为「**现状八行 → 终态九行**」并各自留痕；三处 MUST NOT 再自称终态值。判据：`grep -c '^pub fn ' crates/core/archi/src/api/mod.rs` 起点 ⇒ **8**、落地 D55 后 ⇒ **9**、`grep -c '^pub(crate) fn '` 起点 ⇒ **5**、终态 ⇒ **4**。
> ② **末段形态闸的单一承接地**：Rust 侧末段形态闸**只由** `paths::is_module_segment` 承接（形态 + 非空 + 非关键字，D48 / D50），`contract::verify` 的 `is_identifier`（D45 闸一）**只管七族名字**、MUST NOT 承接末段形态、MUST NOT 被反向复用来判模块段（`paths` 是 `api` 的地基层，方向不可反）。两函数**事实源不同**（契约文本里写的名字 vs 路径派生的模块段），故「两个同类字符集判据同时在场」不是缺陷而是分工；判据见 AC-23 与 AC-27 的并列读法。
> ③ **`api::meta` 的 doc 并入 D54② 口径**：公共函数表 `api::meta` 行的「解析失败即 panic 化的编译期资产错误不可接受」这一半句与 D54②「解析失败 ⇒ `ApiError::Contract`、**静默跳过不允许**」互斥读法 ⇒ 统一为「`meta()` **零失败**（`include_str!` 资产恒在场，`CommonRule` 的合成与 `{}` 计数不在本函数承担）+ `load_contracts` 另以 `parse_common` 复核并映射为 `ApiError::Contract`」，与 AC-31 ① 的「`parse_common` 是唯一解析点」同源。
> ④ **自查串改 `grep -cF` 锚文本形**：`tasks.md` 阶段二十九 的自查串里残留 **反斜杠转义引号**（即串内引号前加一个 `\` 的写法）——该形态依赖双引号包裹 + shell 保留反斜杠才成立，实测 Bash 得 1 而 PowerShell / `cmd.exe` 语义不同，属**跨解释器不稳** ⇒ 一律改 `grep -cF` + **单引号包裹**、串内引号**原样书写**（如 `grep -cF '"rust": "Result<{}, String>"' …`、`grep -cF 'Outcome' …`），判据值不变；本文件与 `tasks.md` 的**判据串面零反斜杠转义引号**（本段自身的示例已按同一纪律避开该形态）。<br>前稿（第二十六稿）页头的五件 FAIL 文本自洽收口（`api` 七子模块 / `ArchiElement` 十字段 / 公共函数表射程无第二份 / `change_flow/mod_test.rs` 回滚归属 / `grep -c` 转义面）**逐字保留、本稿不重开**；更早各稿页头同样逐字沿用。
> ① **`api` 子模块数（`archi` 现状段）**：旧文「`api` 十一子模块（`check` / `contract` / `emit` / `extract` / `mod` / `paths` / `schema` 及其 sidecar）」同行自列 **7** 项、盘上 `crates/core/archi/src/api/` 亦为 **7 子模块**（`check` / `contract` / `emit/`（子模块族）/ `extract/`（子模块族）/ `mod` / `paths` / `schema`）+ **1 资产文件**（`common.json`）⇒ **十一**改为「**七**子模块」；与第二十五稿已修的 `render` 可见性残留同型（数词与实列不一致）。
> ② **`ArchiElement` 字段数**：本文件修改表 `dto.rs` 行已含 `ArchiElement.api`，而类型表只登三行、且未在「起点核实」段交代起点 ⇒ 旧文「`ArchiElement` 九字段」与「已改」并存不自洽。盘上 `dto.rs` 的 `ArchiElement` 实为 **10 字段**（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `model_file` / `model_line` / `mark` / `api`，末位即本变更新增的那一个）。改判为「起点核实段的**改前**九字段（无 `api`）／**改后**十字段（`api` 落第 10 位）」，类型表三行**全数在场**、三处同值。
> ③ **公共函数表口径（射程无第二份）**：旧稿的「公共函数表十二行」与表体实测 **13 行**不符，且表内两行 `pub(crate)` 助手（`api::parse_common` / `paths::is_module_segment`）与 `tasks.md` 阶段九的「七个 `pub fn` + 私有工序」互斥。本稿**删掉行数计数面**，改由「射程定义 + 现算命令」两处唯一自洽（MUST NOT 在别处出现「N 行」这类可漂移的计数）：① 射程 = 本文件首部口径（模块级导出函数 / 关联常量 / 公共类型的公开方法 / CLI 子命令），`pub(crate)` 与私有工序**一律不入表**——`parse_common` / `protocol_key` / `is_module_segment` / `index_entries` / `categories_of` / `def_shape` / `module_refs` / `generated_path_of` 及两侧 `common_type` 全数落此；② 现算命令与射程细则**只在「公共函数 / API」小节的说明段写一次**（本页头不复述第二条命令，避免出现两份可漂移的计数口径）。与 `tasks.md` 阶段九的定义面逐项一致（对照口径见「公共函数 / API」小节的说明段）。
> ④ **`change_flow/mod_test.rs` 回滚的归属**：`tasks.md` 阶段二十九 ⑤ 把该测试装置的越界回滚以**产品任务形态**下达，与 `tasks.md` 首部「本相位不写测试、不改用例」冲突。回滚本身**必须保留**（D56 / AC-33 的法条不动），只改归属与形态：该文件**不入变更清单、不入机械随动白名单、不入实现相位封闭集合**（66 路径不变），其回滚是**测试相位的门禁前提**——tasks 侧改写为「实现相位零触碰 + 测试相位接手红点 + 判据移交」，design 侧在测试相位产出白名单的「壳命令面用例」行与 D56 内明写归属。
> ⑤ **`grep -c` 的转义面**：`tasks.md` 阶段十五自查行的 `grep -c '"api:check"\|"api:export"'` 依赖 shell 保留反斜杠才成立（BRE 交替），在 `-E` / `-F` / 不同 shell 下语义漂移 ⇒ 一律改为 **`grep -cF` 双串分列计数**（判据值不变：两串各 ⇒ **1**，合计 ⇒ **2**）。
> 前稿（第二十五稿）页头的六件机械闭环 + 两处判断纠偏**逐字保留在下文**、本稿不重开：
> ① **render 可见性纠偏（D43 措辞误撞）**：盘上 `api/emit/{rust,ts}.rs` 实为 `pub fn render`（rust `:46` / ts `:96`，D46 已定四参单形出口），`tasks.md` 旧稿 阶段二十三 ① 的「`render` 保持私有、**不升格 `pub`**」是 D43 语境下对**私有 `render_scope`** 的措辞误撞 ⇒ 按「设计追代码形状」改为「`render_scope` 保持私有，`render` 为 D46 的四参 `pub` 出口」，MUST NOT 降级产品面。
> ② **`bash.rs` 提升保留在测试相位白名单**：`crates/infra/agent/src/sdk/bash.rs` 的 `pub(crate) fn resolve_timeout_ms` 已在盘（`git diff HEAD` 仅 **+2 / −1**：一行签名改写 + 一行 doc），且 `bash_test.rs:182` 直测它 ⇒ 「移出清单」不可行（移出即让工作树出现清单外改动，收尾自查 F ① 必红）。**正解是保留在测试相位产出白名单并给阶段二十九立一条自查行**。
> ③ **行数与路径数重算**：现行新增表 **18 行 / 18 路径**、修改表 **22 行 / 24 路径** ⇒ 清单 = **40 行 / 42 路径**；白名单 **18 路径**；实现相位封闭数 **60 路径**；测试相位 **12 组 / 44 文件**；交付态并集 **104 路径**（现算命令见核对口径段；**2019 稿之后的旧值 41/48/66/13/47/113 一律作废**）。
> ④ **测试相位表体重算**：12 组逐组相加 = **44 文件**（`bash.rs` 归清单侧、`checks/mutation/mod_test.rs` 零 diff 已移除、`change_flow/mod_test.rs` 已按 D56 回滚，三处均不再入本节）。
> ⑤ **协议键口径补裁定（新增 D57）**：`api/common.json` 的资产键 = **协议名字 `Outcome`**（与 spec `:149` / `:158` / `:185`、`ApiType::Outcome`、`#/$defs/Outcome` 一致）；契约 / schema 内的**承载键** = `"outcome"`；两者 MUST 经单点 `protocol_key` 换算，MUST NOT 混为一谈（混则查表恒 miss、静默退回硬编码）。AC-31 ③ 的硬编码判据串因此锚**文本形**而非键名。
> ⑥ **AC-19 改指向实际 Scenario**：「四条用例」改为指向 `specs/workspace-layout-resolution/spec.md` 的四条既有 Scenario（`archi_api_dir 与其他派生互不遮蔽` :46 / `archi_api_rel_dir 同源且前缀一致` :82 / `与绝对路径族尾部一致` :72 / `零 IO 与零字面量` :77），并保留四条机械 `grep` 作附加判据。
> ⑦ **`file_stem` 迁入位置在修改表立行**：`crates/infra/archi/src/runner.rs` 行补「`file_stem` 从 `report.rs` 私有自由函数**迁入** `impl LocalArchiCheck` 并升为 `pub`（实现体逐字照搬）」。
> 前稿页头的以下内容**逐字沿用、不作废**：D54（`common.json` 是一等映射规则资产，渲染器 MUST 消费表、MUST NOT 硬编码映射）/ D55（`api::display` 由 `pub(crate)` 转 `pub` + `lib.rs` 出口，工具 crate 删本地逐字实现）/ D56（`src/commands/change_flow/mod_test.rs` 的装置超时越界弱化回滚）/ AC 表 AC-21…AC-33 十三行 / 决策表 D43…**D60 十八条**（D57 / D58 / D59 / D60 四条为本稿及其上一稿新增）/ S 项 17 条（旧原文串 14 / 新原文串 18）/ §9 的 `common.json` 键名口径（键 = `$defs` 协议名，首期唯一键 `Outcome`）。**不改变**：D1–D57 的裁定形态、判据基线表（28 / 13 / 其余 0）、AC-1…AC-33 的设计落点、proposal 与九份 delta spec（零改动）；**本稿新增的四条裁定**（D58 / D59 / D60 + 前稿 D57）只改实现形态，不动任何 AC 的判据口径。
> **本稿作废的历史数值**（只留痕、不作判据）：清单「46 个文件 / 40 行 / 28 文件」、实现相位封闭数「64 节点 / 65 路径」、交付态并集「115 文件」、测试相位「49 条 / 47 文件」并列口径。
> **基于**: `proposal.md`（**20 条 AC** + 设计补入 **AC-21…AC-34 十四条** = **34 条** / 三张子表）、`explore.md`（10 章）与 `specs/{desktop-archi-api,desktop-archi-api-check,desktop-archi-model,desktop-archi-drift,desktop-archi-agent,desktop-archi-module-docs,desktop-crate-layout,repo-archi-model,workspace-layout-resolution}/spec.md`
> **路径口径**: 新增文件写仓库根相对全路径；修改文件 Rust 侧省略 `packages/desktop/src-tauri/` 前缀；前端侧写全 `packages/desktop/…`。**「省略 / 缩写」只是本文件表内的书写约定，MUST NOT 外溢到任何机械清零 / 集合比对面**——`/tmp/expected.txt`（清单 + 机械随动白名单）、`/tmp/test_phase_whitelist.txt`（测试相位产出白名单）与任何 `grep -cF` 判据串一律取**物理路径**（仓库根相对、带 `packages/desktop/src-tauri/` 前缀），归一单点在「核对口径」段，本节与两张白名单表 MUST NOT 另立第二份口径。
> **读法（本文件全局）**：凡出现「该稿时点值」「历史快照」的数值一律只作留痕、不作判据；**现行唯一口径**只有三处——「变更清单」的**核对口径**段（封闭数）、「spec 措辞同步项」的**射程判据**段（S 项条数）、「验收标准对齐」的**判据基线表**（28 / 13 / 其余 0）。其余位置出现不同数值即缺陷，须按这三处改正。

---

## 提案与规格同步状态

proposal 的范围裁决（资产形态取 md + fenced json、两层派生、十条判据、工具边界写盘、本仓模型同 change 同步）与**九份** delta spec 均已定稿。本设计不重复其论证，只做四件事：把「手写 md → 四类生成物 → 十条判据 → 零 diff 门」落成可实现的结构（`archi` 内 `api/` 模块族的文件边界、两条事实源的分工、判据求值序与报告装配的落点）；把 spec 未给的成形缺口以决策留痕收口（`<root>` 归一形态、CLI 的 root 锚定（D32）、签名见证在 Rust 侧的写法、归属剥离的精确判据、`api:check` 的门禁形态、`CheckError` 的扩用口径、`severity()` 的类型、`common.json` 的消费方式（D54）、跨 crate 投影函数的可见性（D55））；给出逐函数签名与逐文件改动；按 **AC-1…AC-33** 逐条对齐；并按 **D57–D60** 四条收口裁定（协议键两层口径 / `EmitTarget` 三字段 / 分支负载单通路 / `category_of` 出口面）把「接口面大于需求面」与「同一映射两处实现」两类形态消掉。

**交付面三分**（三块互不重叠，MUST NOT 用其一的口径豁免另一处）：

1. **判定与装配面**（`crates/core/archi/src/api/**` + `drift/` + `dto` + `model` + `parse` + `scan/owner`）：纯读盘、零写盘、零 spawn。
2. **写盘面与门禁驱动**（`crates/tools/archi-api-gen` + `packages/desktop/package.json` + `knip.json`）：唯一写盘面是工具 crate。
3. **数据资产**（`.dev-team/architecture/api/**` 的试点契约与生成物、`.dev-team/architecture/models/05-backend-tools.toml` 与 ADR 的计数复述、`AGENT.md` / `CLAUDE.md` 新节）：零运行期角色；② 的新 crate 必须在同一 change 内入模型（`repo-archi-model` 漂移纪律）。

### 三类写面的边界（执行纪律，逐 `hunk` 判）

本 change 的三类写面按**改动种类**划分，MUST NOT 按文件划分（同一文件可同时出现在两类的路径级并集里，内容级不冲突）：

| 类别 | 判据（该 `hunk` 为什么必须改） | 归属 | 允许的形态 |
|---|---|---|---|
| 清单改动 | 交付项本身（新增 / 修改文件表内的语义改动） | 实现相位 | 产品逻辑、契约资产、脚本、数据资产 |
| 机械随动（白名单 18） | **只为可编译**（字段初始化 / `use` 行 / 构造点实参） | 实现相位 | 零逻辑、零断言值、零新增用例 |
| 测试相位产出（44 文件） | 用例 / 装置 / 语料 / 期望值 / 断言值 | 测试相位 | 用例与夹具；**MUST NOT** 改产品语义 |
| 第四类机械随动（D53） | **被删符号的调用形态适配**（`use` 行 / 实参元数 / 装置同名提取） | 实现相位（编译必需那一档） | 逐形态封闭三种，**MUST NOT** 含断言值 / 断言意图 / 用例名 / 增删用例 / 夹具语义 |

### 起点核实（工作树实况，锚点为改动前行号）

- **`archi` 现状**：`lib.rs` 十模块（`diff` / `docs` / `drift` / `dto` / `edit` / `graph` / `model` / `parse` / `scan` / `search`）+ `api` **七**子模块（`check` / `contract` / `emit`（子模块族）/ `extract`（子模块族）/ `mod` / `paths` / `schema`，各带 sidecar；另有一份资产文件 `common.json`）；`drift/mod.rs` 持 `const REPORT_VERSION: u32 = 2;`（第二版；常量不在 `report.rs`）；`drift/mod.rs` 的 `pub fn check(input: CheckInput<'_>) -> Result<Report, CheckError>` 是判定单点，命令面与门禁面共用；`drift/report.rs` 持四组封闭词汇（`ArchiCheckStatus` 五值 / `ArchiDriftKind` 四值 / `ArchiObservationKind` 两值 / `ArchiApiFindingKind` 十值，各带 `ALL`）+ `ArchiDriftReport`（既有 14 字段 + 末位 `api_findings`）。
- **`api` 现状（盘上实测，本稿不以旧稿散文为准）**：`api/mod.rs` 的模块级出口 **起点 8 个 `pub fn` → 终态 9 个 `pub fn`**——起点 = `meta` / `enabled` / `load_contracts` / `render_all` / `bootstrap` / `check` / `api_ref` / `api_ref_of`（`grep -c '^pub fn '` 现算 ⇒ **8**），终态 = 上述八行 + **`display`**（D55 把 `pub(crate) fn display` 升为 `pub fn`，落第 9 行；盘上现状行首为 `pub(crate) fn display`、`^pub fn ` 不计入，故 8 为起点值、9 为终态值，两值都须按各自时点读）；起点另有 **五个 `pub(crate)` 助手**（`short_digest`（摘要截断）/ `display`（repo 相对串，D55 的直接对象）/ `ref_name` / `reexport_target` / `kind_of`；`pub(crate) fn` 现算 `grep -c '^pub(crate) fn '` ⇒ **5**），终态余 **四个**（`display` 出线后不再计入 `pub(crate)` 面）；`api/paths.rs` 的 `pub fn` 计数以 `grep -c "^pub fn "` 现算为准 ⇒ **15**（`is_module_segment` 为 `pub(crate)`，不计入）；`api/emit/{rust,ts}.rs` 的 `value_type` 各含一处 **`outcome` 键触发的硬编码映射**（`Result<{}, String>` / `{ ok: {} } | { err: string }`）——这正是 D54 要消的形态；`api/common.json` 的实盘内容为**单键 `Outcome`**，其值为 `{ "rust": "Result<{}, String>", "typescript": "{ ok: {} } | { err: string }", "degraded": "typescript", "note": … }`。
- **`dto` 现状**：`ArchiElement` **改前九字段**（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `model_file` / `model_line` / `mark`）→ **改后十字段**（`api` 落**第 10 位**，即本变更新增的那一个，见「类型定义」表 `ArchiElement.api` / `ArchiElementBrief.api` 行）；`ArchiElementBrief` 四字段（改后五字段）；`ArchiModuleDetail` 六字段（改后七字段）；投影入口 `graph_view` / `module_detail` / `module_overview` / `search` / `change_summary` 五个；`dto` 已直接依赖 `foundation::layout`。
- **工具 crate 现状**：`crates/tools/archi-api-gen/src/main.rs` 的 `fn display(root: &Path, path: &Path) -> String` 与 `api/mod.rs:pub(crate) fn display(..)` **逐字重复**（D55 的直接对象）——工具 crate 无法复用 `pub(crate)` 函数，于是复制了一份。

### 与 spec / 提案措辞的 38 处事实校准（不改 spec 意图，只定实现形态）

1. **`REPORT_VERSION` 常量不在 `report.rs`** 而在 `drift/mod.rs`——本设计的「报版本递增」落 `drift/mod.rs`。
2. **`<root>` 归一不去扩展名** ⇒ 前端 16 个单文件元素的入口四分支必然零命中，`apiEntryMissing` 基线因此含它们（AC-7）。
3. **试点元素选 `App.Backend.Infra.ArchiRuntime`**：公开面最小（`pub struct` + `pub fn` 构造器 + trait impl + `pub(crate) fn` 四形态齐备）。
4. **`LocalArchiCheck` 的零字段 struct 由生成物接管**：手写同名定义不删即 `pub use api_gen::*` 报 E0255。
5. **签名见证形态取 `super::` 前缀**：生成物在 `<root>.i.rs`（根同级），与被见证的 `impl` 不在同一模块树层。
6. **零内容不生成**：契约无类型无 op 时生成 `.i.*` 会让 `pub use api_gen::*;` 触发 `unused_imports`，`clippy -D warnings` 红。
7. **`.i` 剥离四形态实测**：`views/architecture.i.ts` / `views/welcome-view.i.ts` / `crates/core/archi.i.rs` / `crates/core/archi/src/scan.i.rs` 各归其元素。
8. **`git diff` 不足以做零 diff 判据**：漏 untracked、普通 pathspec 漏顶层文件 ⇒ `--check` 用「磁盘 vs 应生成」比对（D21）。
9. **`#[path = "../../archi.i.rs"]` 实测编译通过**（`rustc --edition 2021`），目标是根同级的 `crates/infra/archi.i.rs`。
10. **不声明 `pub mod api;` 则该族不可编译**（`archi` 的 `lib.rs` 模块清单必须同步）。
11. **`syn` / `proc-macro2` / `oxc_*` / `sha2` / `globset` / `toml_edit` 均已在 `Cargo.lock` 且已是 `archi` 直接依赖** ⇒ 零新增外部包。
12. **proposal 表把试点生成物写成 `crates/infra/archi/archi.i.rs`**，与「生成物落根同级」规则冲突；本清单按规则落到 `crates/infra/archi.i.rs`（唯一一处路径偏离留痕）。
13. **`search_test.rs` 等测试文件的 `Element` 字面量需补 `api` 字段**，否则 E0063（属机械随动白名单）。
14. **`ArchiApiFinding.category` 与 `INDEX.categories` 键空间相同是巧合**，语义不同（前者随债数漂移，后者只随契约改动变化）——MUST NOT 合并。
15. **`x-class` 错误侧必须拦在 `verify_defs`**（契约自身矛盾），警告侧走 `emit::render_target` 的 stderr（不进报告，D27）。
16. **`api:check` 的单条形态是跨平台确定性的前提**：`cmd.exe` 无可靠的「空输出即成功」判据，故不在脚本里做 shell 逻辑。
17. **`Cargo.lock` 必须随 workspace 成员变化提交**（`archi-api-gen` 的 `[[package]]` 条目）。
18. **写模式门使「提交态 = 不动点」不收敛**（实测连续 6 次失败；mtime 时序可证）。D28 / D29 / D30 是根因修复。
19. **`rustfmt(HEAD)` 输出与工作树字节 `cmp` 6/6 相等**是「纯格式化清偿」的唯一正确判据（`--ignore-all-space` 实测仍报 34/17 行，不可用）。
20. **行尾面实测零处不等**：`git diff HEAD --numstat` 与 `git diff HEAD --numstat --ignore-cr-at-eol` 逐项相等（含 `package.json` 与 `crates/infra/agent/src/sdk/**`）。交付前曾出现的四处行尾翻面（`AGENT.md` / `CLAUDE.md` LF→CRLF、`bash_test.rs` LF→CRLF、`vocabulary_test.rs` CRLF→LF）已按各自 `HEAD` 形态归位——**行尾不构成允许项**，任何后续改动 MUST 保持「两命令逐项相等」。
21. **清单行数口径**：修改表**一行可覆盖多文件**（`bindings.ts` + `bindings.zod.ts`、`AGENT.md` + `CLAUDE.md`），故「**40 行 = 18 新增行 + 22 修改行 → 42 路径**（新增 18 + 修改 24）」；行数与路径数 MUST 分列，MUST NOT 互相反推。
22. **收敛性判据只取文件名集合**（`git status` 状态码构成随暂存态漂移，不可复算）。
23. **CLI 的 root 由向上锚定得到**（D32）：`archi-api-gen` 全文件零路径字面量，既有用例以 `include_str!("main.rs")` 断言。
24. **`check` 链第二段是 `bindings:export`**：`package.json` 的 `scripts` 内只有 `bindings:export` / `bindings:zod-check` 两个 `bindings:` 键，无 `bindings:check`。
25. **`version` 判据是「相对 `HEAD` 值 +1 patch」**，MUST NOT 写死字面号（现值随每次源码改动滚动）。
26. **`serde_json` 的重复键折叠不可逆** ⇒ 同名即错的规则只能在 `contract::parse` 的文本层判（D37）。
27. **`apply_knip_report` 产品路径零调用者**、`main.rs` 与 `api::bootstrap` 零 `knip` 词 ⇒ `Visibility::Unused` 生产面不可达（D39）。
28. **`emit/ts.rs` 的 `target.rsplit('.')` 与私有 `entry_form_of` 是二次派生**（D40 / D41 的在案反例）；`api/mod.rs` 的 dummy `ArchiElement` 同族（D42）。
29. **`api::display` 与工具侧 `display` 逐字重复**（本稿实测：两处实现体逐行相同），而前者是 `pub(crate)` ⇒ 跨 crate 物理不可复用（D55）。
30. **`common.json` 的实盘键是 `Outcome`**，且其 `rust` / `typescript` 两槽位的值与两侧渲染器的硬编码**逐字相同** ⇒ 表与实现同值但零连接（D54 的判定依据：删表会与 spec `:158` / `:159` 冲突，故裁决「消费表」而非「删表」）。
31. **`ApiMeta` 的构造点恰三处**（`api::meta()` 产品一处 + `emit/ts_test.rs` / `schema_test.rs` 两个测试装置各一处，后者均委托 `crate::api::meta()`）⇒ 新增 `common` 字段**零测试侧机械随动**。
32. **`emit::rust::render` / `emit::ts::render` 的调用点仅 `emit::render_target` 两处**（产品面），`api/mod.rs` 的 `render_all` 经 `render_target` 进入 ⇒ 签名扩参（D54 的载元注入）的调用面封闭在 `emit/mod.rs` 一处。
33. **`src/commands/change_flow/mod_test.rs` 的装置超时被放宽**（`spawn_command_confirmer` 由 60s 改 180s + 30s 登记窗；复合用例由 `wait_for`(30s) 改 `wait_for_up_to` 120s），而该文件的既有装置约定（见 `test-design.md`「既有装置的正确性前提」条）是 **30s 收敛上限 + 装置退出判据「见过 run 登记后转 `None`」**，且该文件与本变更零产品面关联（D56）。
34. **`EmitTarget` 的 `path` / `element_root` 两字段产品面零读取**（D58 的判定依据）：产品面读 `EmitTarget` 的点恰三处——`emit/mod.rs` 的 `match target.kind`（分派）、`emit/rust.rs` 的 `&target.modules`（透传查表）、`emit/ts.rs` 的 `target.form` + `&target.modules`（specifier 形态 + 查表）；`path` / `element_root` 在 `crates/**` 产品 `.rs` 内**零读取点**（`grep -rn 'target\.path\|target\.element_root' --include=*.rs` 滤测试后零命中）。头注释的四条文本全部来自 `ApiMeta`（`notice_lines(meta)`：工具版本 + `common_digest` 前 12 位 + 两句固定警示 + `generated_notice`），诊断定位的载体是 `Contract.file`（每处 `ApiError::Contract` 都带它），故两字段没有任何「唯一消费者」可指认——它们是接口面大于需求面的残留（与 D42 删 dummy `ArchiElement` 同型）。
35. **`choice_payload` 的 `_common` 参数零使用且与 `value_type` 重复**（D59 的判定依据）：`emit/rust.rs` 的私有 `choice_payload(choice: &Value, _common: &BTreeMap<String, CommonRule>)` 形参带下划线即「不消费」，函数体自写一份标量映射（`string` → `String` / `integer` → `i64` / …）——`value_type` 早已持有同一张表且**多出协议键查表（`common_type`）与 `$ref` 归一**。后果是同一 schema 的同一分支在「`value_type` 路径」（TS 的 `union_of`、Rust 的字段位 / 见证位）与「`variants_block` 路径」产出不同负载形态（协议键分支在前者被展开、在后者落 `None` → `Variant{index}`）——违反 D43「短形写出点唯一」与 D54③「映射唯一查表面」。实测差异面：`$ref` 与四种标量两路同值（`AA(A)` 一类照旧），而协议键 / `array` / 带 `additionalProperties` 的 `object` 三类在 `value_type` 有文本、在 `choice_payload` 落 `None` ⇒ 同一分支在 TS 侧（早经 `value_type`）与 Rust 侧（经 `choice_payload`）形态不同。
36. **`pub use check::category_of;` 的产品面零消费者**（D60 的判定依据）：`category_of` 的调用点只在 `check.rs` 自身（`entry_of` 的 `category: category_of(kind).to_owned()`）与 `check_test.rs`（`use super::check::{category_of, check}`，走模块路径）；`api/mod.rs` 的重导出**零消费者**（`grep -rn 'category_of' --include=*.rs crates src` 滤 `api/check.rs` / `api/check_test.rs` / `api/mod.rs` 后零命中），且 `mod_test.rs` 的既有断言正是「`mod.rs` 内零 `category_of` 调用」——重导出与「不借判据分类」的纪律方向相反。

37. **`declare_ancestors` 曾以一个 `#[cfg(test)] pub(crate) mod` 挂在产品文件 `api/mod.rs` 上、且自成一个未被任何清单收纳的文件**（回溯项的实现面闭合）：该装置（`api/test_support.rs`，47 行）被 `api/mod_test.rs` 与 `api/paths_test.rs`（模块路径）、`api/extract/{mod,ts}_test.rs`（`crate::api::` 全路径）四处引用，却既不在新增 18 / 机械随动白名单 18 / 测试相位 44 三表的任一并集内 ⇒ 交付态实测 **105 路径** ≠ 声明 **104 路径**，与本节「零新增文件」与 `test-design.md` 的「用例文件 11 份」同时矛盾。**处置**：装置整条内联进 `api/mod_test.rs`（改为该文件的 `pub(crate) fn declare_ancestors`，doc 写明兄弟 / 后代测试模块经 `crate::api::mod_test::declare_ancestors` 共用），`api/mod.rs` 的 `#[cfg(test)] pub(crate) mod test_support;` 声明撤回，`api/test_support.rs` 删除；四处引用改指新路径，**用例集零变化**（三表并集仍 104，`test_phase` 仍 44 文件）。判据：`grep -rnF "test_support" --include=*.rs crates/ src/` ⇒ **0**；`grep -cF "fn declare_ancestors" crates/core/archi/src/api/mod_test.rs` ⇒ **1**；`git status --porcelain -uall` 扣除本 change 目录后 **104 行**。
38. **`emit::rust::variant_name` 盘上形态是私有 `fn`，而公共函数表曾以 `pub fn` 立行**（回溯项的实现面闭合，纪律①的自证）：按本节「公共函数 / API」小节纪律①「`pub(crate)` 与私有工序**一律不入表**」——该纪律原文已把 `variant_name` 与 `common_type` 并列列入「私有工序」——故删掉表内该行、并在 `emit/rust.rs` 行的行数说明里把它归入私有工序（该行由 **3 行** 改 **2 行**：`render` / `rust_comment_text`）。**实现形态不动**（`fn variant_name` 保持私有，行为经 `render` 的 enum 成员断言间接观测，D45 闸二的判据串 `grep -c "fn variant_name"` 恰 1 不受影响）。判据：`sed -n '/^### 公共函数 \/ API$/,/^### 类型定义$/p' design.md | grep -c '^| `\|^| \*\*`'` ⇒ **59**（删该行前 60），且 `grep -cF 'pub fn variant_name' crates/core/archi/src/api/emit/rust.rs` ⇒ **0**、`grep -c "fn variant_name"` 同文件 ⇒ **1**（私有形态逐字不动）。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 判定 / 写盘的分层 | 契约解析、`$defs` 规则、装配、渲染、十判据全部落 `crates/core/archi/src/api/**`（**纯读盘、零写盘、零 spawn**）；唯一写盘面是 `crates/tools/archi-api-gen`（CLI，与 `export-bindings` 同族） | 沿 `archi` crate 既有四条纪律（单点构建 / 投影零改写 / 词汇对拍 / 判定单源）不动；写盘是工具边界的正当属性，不得反向稀释判定域纪律（AC-12） |
| D2 | `<root>` 归一的步数 | 恰两步：剥前导 `./`、去尾 `/`；**不去文件扩展名**。生成物 = `<root>.i.rs` / `<root>.i.ts`；入口查找按 `<root>` 拼三条 / 四条候选 | 归一越少越可预测；前端单文件元素的 root 保留 `.tsx` 后，入口四分支必然零命中 → 如实报 `apiEntryMissing`（AC-7），MUST NOT 为凑绿去扩展名（AC-16） |
| D3 | 生成物落点与归属剥离 | 根同级落盘；`owner.rs` 增一条剥离：文件名为 `<name>.i.<ext>` 时按**去掉 `.i.<ext>` 的文件段**得到模块根，再用既有 `prefix_matches` 判定。`ext` 集合**恰五值** `{rs, ts, tsx, js, jsx}`（`GENERATED_EXTENSIONS` 一个常量持有，标记段 `"i"` 由 `GENERATED_MARKER` 持有）；`ext` 集合与判据处 MUST NOT 各写一份 | 令 `prefix_matches` 语义零变更；四形态各归其元素（事实校准 7）；非 `.i` 路径逐字零变化。五值而非三值的理由：D2 归一不去扩展名 ⇒ `<root>` 可能以 `.tsx` / `.js` / `.jsx` 结尾 |
| D4 | 入口查找顺序 | Rust `<dir>/src/lib.rs` → `<dir>/mod.rs` → `<dir>.rs`；前端 `<dir>/index.ts` → `index.tsx` → `<dir>.ts` → `<dir>.tsx`。`<dir>/lib.rs` MUST NOT 在列；MUST NOT 增分支 | 三分支 / 四分支各有存量支撑或如实零命中；分支数固定才能让 `apiEntryMissing` 的基线可对拍（本仓 14 / 0 / 0 / 0） |
| D5 | `#[path]` 与 TS import 的单点产出 | `EntryForm { Crate, SubdirModule, SingleFile }` 三值封闭；`mod_path_attr` 产出 `"../../<name>.i.rs"` / `"../<name>.i.rs"` / `"<name>.i.rs"`；`ts_import_specifier` 产出 `"../<name>.i"` / `"./<name>.i"` | 目标是恒指向**根同级**生成物；手写「入口同级」会错位到 `<dir>/<name>.i.rs`（AC-4 反例在案）。**值形态由单点函数持有**，但落到入口行时是**字面量**（见 D38 的修正） |
| D6 | 契约 md 的位置语义 | H1 = `<FQN>`（与文件名比对）；`## <中文二标题>` 为散文；`### <op-id>` 后紧随一个 `api-op` fenced json（块内不含 op 名）；`api-types` / `api-reexport` 各为**同文件唯一**的 fenced json 块 | 按 heading 分段提取，实现即「扫行 + fence 状态机」，零 markdown 解析依赖；双向 gate 三条都能在这一层机械判定（AC-1） |
| D7 | `schema.json` 的装配 | 生成器只做装配：`$schema`（draft 2020-12 常量）/ `$id`（取文件名，不合成 URI）/ `title`（H1）/ `$defs`（`api-types` + `api-reexport` 纯 `$ref` 别名）/ `properties`（op 键 → 原样 `x-function`；`$defs` 键 → `{"$ref": "#/$defs/<k>"}`）/ `$comment`（警示文本） | 六字段逐项有 md 出处，无「只有手改才能表达」的内容（AC-2）；fence 内是原生 JSON Schema，故不做翻译 |
| D8 | `$defs` 五条规则 | 身份 = 键名（非 pointer）；嵌套 `$defs` 提升且**原名保留**（改名即错）；所有 `$ref` target 必须是顶层 `$defs` 条目；同类型只定义一次；**禁按结构去重** | 规则 5 的依据是仓内 17 组同形不同义类型（`ArchiDependsOn` / `ArchiDependedBy` 语义相反），合并即语义错（AC-3） |
| D9 | 关键字集与两类扩展构造 | 仅标准关键字 + `x-class` / `x-function` / `x-common`。`x-class` 带属性即契约错误（另加**警告**：`properties` 全为函数却未标 `x-class`）；标量生成 newtype（不进 `common.json`）；写泛型即契约错误；`x-common` 只作注记、不驱动生成、不加强校验 | `x-function` 是唯一能区分「模块操作」与「函数指针字段」的标记；`x-class` 消灭「形状推导误判 trait」的静默故障；标量 trait 的名义性为零（AC-5） |
| D10 | 签名归一与位置分派 | 位置分派（入参 `&str` / 返回 `String` / 字段 `String` / 入参 `&[T]`）是 `emit` 的**内部表**，MUST NOT 出现在判据或对外面；比对用 `ApiType` 逻辑类型序列（参数名、空白、多行排列不参与） | 归一后比对才能让「契约 `input: string` vs 代码 `input: &str`」判一致（AC-5 / AC-9） |
| D11 | 构造器三条 | 契约 `### 操作` MUST NOT 声明构造器（`new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open`），违反报 `apiConstructorDeclared`；构造器由生成器从 `$defs` 条目自动产出；构造器**双向不参与比对** | 试点纸面验证首版草案写了 `### new` 就被 `apiMissing` 误报——这是本规则与判据的直接来源（AC-13） |
| D12 | `common.json` 与生成物头 | 新增 `crates/core/archi/src/api/common.json`，`include_str!` 内嵌，**只承载映射规则**（当前唯一键 = `Outcome` 信封），生成时就地展开为各语言标准类型 ⇒ 零生成物零 vendor；生成物头写 `// 生成于 archi-api <CARGO_PKG_VERSION> · common.json@sha256:<hex>（前 12 位）` | spec 要求头注释可分辨「工具升级 → 批量重生成」的变更来源；摘要用 `sha2`（零新包）。该文件的**消费方式**由 D54 补全（「它是什么、头怎么写」）；**键名两层口径**由 D57 钉死（资产键 = 协议名字 `Outcome`、承载键 = `"outcome"`、经 `protocol_key` 换算） |
| D13 | 四类生成物的头部警示 | `.i.rs` / `.i.ts` 首行块注释（勿手改 / 下次 generate 覆盖 / `api:check` 会失败 / 手写 impl 与本文件分离）；`.schema.json` 的 `$comment`；`INDEX.toml` 顶部说明段（含 review 口径） | 沿 `bindings.ts` / `bindings.zod.ts` 既有惯例；R-1 与 R-15 的唯一可见缓解 |
| D14 | 判据词汇与档位类型 | 新平行枚举 `ArchiApiFindingKind`（十值 + `ALL: [Self; 10]` + `as_str` + `severity()`）；新增 `ApiFindingSeverity { Off, Warn }`（封闭二值 + `as_str` + `ALL`，进程内、零 serde / specta）；`category` 为开放 `String` | 不复用 `GateTier` 承载判据档位：`strict` 在判据面上不可表达，复用会让「契约判据档位」与「元素 gate」在类型上可互换（正是要解耦的那件事） |
| D15 | `category` 初始取值 | 六个：`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor`；打标在 `check.rs` 的**单处** `category_of(kind) -> &'static str` | 加一个分类只改这一处函数，不动枚举、不动 `ALL`、不动金样词汇对拍（AC-8） |
| D16 | 求值顺序与互斥 | 固定序：① `apiEntryMissing`（命中即**不再**求 ② ③ ④，仅继续 ⑤）→ ② `apiMissing` → ③ `apiUndeclared`（`severity() == Off` ⇒ 直接跳过、零条目）→ ④ `apiSignatureMismatch` → ⑤ `apiUnexpected`（与 ① 独立）→ ⑥–⑩ 嵌套与透传五项（逐元素独立求值） | ① 比 ② 更基础；两条对同一元素并报会让报告数无法单调对拍（AC-7 期望 28 / 13 的直接来源）。⑤ 单向：反方向归 ② |
| D17 | 契约面启用条件 | `<archi_api_dir>` 缺席或其中零份 `*.md` ⇒ `apiFindings = []` 且既有语料逐字不变（`version` 仍递增、槽位仍存在）；目录一旦存在则**全部 `path` 非空元素**纳入，无白名单、无开关、无部分豁免 | 迁移期仓库与全部既有漂移语料都没有契约资产；不做该条件则「金样仅追加空槽位」与「28 + 13 条实测数」互相冲突（AC-8） |
| D18 | 报告装配与 `status` 的关系 | `api_findings` 为 `ArchiDriftReport` **末位并列字段**；装配单点在 `drift::check()` 内调 `api::check`；`status` 五值推导与门禁 `passed` MUST NOT 读 `api_findings`；**稳定序 `(kind, element, target)`**，同元素对多处命中聚在一条 `occurrences` 内，两次运行逐字相同 | 与 `observations`「永不影响 status」同款先例；稳定序使报告可逐字 diff；聚合避免 41 行平铺（AC-8 / AC-9）。排序纪律的唯一记载处是本行 |
| D19 | 代码导出集的取数面 | 归 `api/extract/**`（`syn` / `oxc` AST），与 `bootstrap` 共用一份；**不并入 `scan`**。inherent 方法的可达性按「接收类型是否 `external` 可达」判定；`visibility` 四值封闭（`external` / `crateOnly` / `unused` / `testOnly`）。**取数面 = 导出面**（两条子裁定见 D19-a / D19-b） | `scan` 出的是「依赖事实」，`extract` 出的是「API 面事实」；两套事实源各自单点，判据在 `check.rs` 汇合（AC-9） |
| **D19-a** | **D19 子裁定①：取数面只抽导出面（两语言同一条纪律）** | `extract::{rust,ts}::extract_file` **只**产出「导出面」符号：Rust = 顶层 `pub`（含 `pub(crate)` 降格，见 D19-b）+ `pub mod` 链可达的 `pub` 项 + `impl` 内 `pub` 方法；**TS = 仅有 `export` 关键字的声明**。`ts.rs` MUST 删掉六个非导出顶层分支；`Statement::ExportDeclaration` 一条分支内由 `collect_declaration` 收口（`export function` / `export type` / `export interface` / `export class` / `export const` / `export enum`），`export {…}` / `export * from` 显式零符号（透传由契约 `api-reexport` 表达）；`export default` **本期显式不抽**（对外名由导入方决定，反推名字即造第二套命名规则；本仓唯一实例是 `api = none` 的组件元素，留白零代价） | 取数面的两条消费方都要「导出面」：判据（混进私有符号即假阳）与 `bootstrap` 草稿（起草私有符号等于凭空承诺）。两语言共用一条语义才能让两侧判据同源（AC-9 / AC-17） |
| **D19-b** | **D19 子裁定②：`visibility` 四值的词表封闭、可判值域与留白口径** | `Visibility` 四值在**两语言都封闭存在**（`ALL: [Self; 4]` 不变）；但**本期可判值域按语言分列**：Rust 三值可产（`External` / `CrateOnly` / `TestOnly`，`TestOnly` 由 `#[cfg(test)]` 子树语法判定而非文件名），`Unused` 由调用方按引用事实降级；**TS 生产面只产 `External`**（有 `export`），`CrateOnly` / `TestOnly` **本期留白**（需跨文件 import 反向事实，取数器只吃单文件 AST），`Unused` 的唯一产出点在 `apply_knip_report` 内而该函数产品路径零调用者（D39）。**文件名族（`.test.` / `.spec.` / `__tests__` / `fixtures`）一律禁作 proxy**；`used_by` 同源留白。留白 MUST 在 `Visibility` 枚举 doc 与 `ts.rs` 模块 doc 写明原因与将来所需输入，MUST NOT 用「已实现」掩盖 | 「四值存在」与「两语言都判得出四值」是两件事。TS 侧两值判据需要跨文件引用事实，本 change 的取数器刻意没引入它 ⇒ 正确形态是「值域封闭 + 判据留白 + 留白显式留痕」。反例在案：test-design 自行发明「文件名含 `.test.` → `TestOnly`」，既不在 spec（spec 说的是「**被**测试引用」）也不在实现 |
| D20 | 契约资产非法时的错误面 | `api::*` 的错误面为新 `ApiError`（`Io` / `Contract` / `Element` 三值 + `message()`）；`drift::check` 把 `ApiError` 映射进既有 `CheckError::Config(..)`，**不新增 `CheckError` 变体**；`CheckError::Config` 的 doc 扩为「判定输入资产非法（看护配置或契约资产）」 | 契约是判定的输入资产，资产坏了应「检查不执行、不产出半真报告」；新增第四变体要动两份 spec 的封闭三值，收益为零 |
| D21 | `api:check` 的门禁形态 | `api:check` = `cargo run -q -p archi-api-gen -- --check` **单条**（工具内读盘比对，零写入）；`--check` 必须同时报三类偏差：内容不符、应删未删（孤儿生成物）、应生成未生成。`git status --porcelain` 只作**落地自查命令**写进 tasks 自查行，不进 `package.json` 脚本 | 事实校准 8：`git diff` 漏 untracked、普通 pathspec 漏顶层文件；Windows `cmd.exe` 无可靠的「空输出即成功」判据，故脚本里零 shell 逻辑（事实校准 16） |
| D22 | `api` 的两层口径 | 模型字段 `api: ApiStatus`（`declared` 缺省 / `none`）= **声明意图**；`ArchiApiRef.status` = **文件在案事实**（按 `<archi_api_dir>/<Domain>/<FQN>.md` 定位，`exists()` 即 `declared`，否则 `none` 且三路径为 `null`） | 两者不可合并：`declared` 而文件缺失是**迁移期主形态**，投影若报 `declared` 会让 agent 以为有契约可读（读不到）；反之把 `none` 回写成模型字段就变成了「投影改写」（AC-10 / AC-13） |
| D23 | 试点元素的契约面与代码形状 | 试点 op = `file_stem(timestamp: string) -> string`；`$defs` = `LocalArchiCheck`（零字段 object）；生成物出 `pub struct LocalArchiCheck;` + `new()`；手写侧删同名 struct 与 `new()`、把 `file_stem` 从 `report.rs` 的自由函数挪进 `impl LocalArchiCheck` 并提升为 `pub`（实现体逐字不动）；`compose_text` 保持私有且不进契约 | 事实校准 3 / 4：`compose_text` 的签名含非直接子元素提供的类型（违反「跨模块 `$ref` 须指向直接子元素」），`file_stem` 纯标量；两处删除不是「格式追代码」而是「代码形状与契约一致」（生成物接管类型与构造器） |
| D24 | 本仓模型同步口径 | `05-backend-tools.toml` 追加 `App.Backend.Tools.ArchiApiGen`（`kind = "module"` / `gate = "off"` / `path = ["./packages/desktop/src-tauri/crates/tools/archi-api-gen/"]` / 一句中文 `summary`）+ 2 条出边（→ `Core.Archi` / → `Core.Foundation`）；表头 `2 / 1` → `3 / 3`；ADR `§1` / `§4` / `§6` / `§9` 计数同值：**51 元素 / 113 关系**（后端 49、前端 64、自研 crate 16）；`gate` 全 `off` 不动；`specta-zod-patched` 单列为既有缺口 | 新增 workspace crate 的漂移纪律「同一 change 内补齐元素与边」；两条边的代码侧证据在 `archi-api-gen/Cargo.toml`；`tools` 段容器已存在，故只需叶元素 + 2 边（AC-15） |
| D25 | 知识文本从 `ALL` 派生 | `archi_knowledge.rs` 的派生段新增一行 `api 状态：<ApiStatus::ALL 逐值>`；`archi-knowledge.md` 的「格式约定」字段面补 `api`（二值与缺省），「边界」段改写为「契约权威在 `<archi_api_dir>` 的契约资产，模型只存状态指针」 | spec 要求四面（诊断集 / `archi_module` 响应 / IPC DTO / 知识文本）词汇集相等且 `api` 状态两值在场；改写「契约权威在 spec 文本」是必须的——它正是本变更要替换的旧口径 |
| D26 | `INDEX.toml` 的 `category` 分布语义 | `IndexEntry.categories` SHALL 按**元素的 `$defs` 形状**打标：键空间**当前六值**（开放字符串集）：`object` / `enum` / `variant` / `class` / `scalar` / `reexport`，值为该形状的条目数；打标单点在 `api::index_entries` 的私有 `def_shape(definition) -> &'static str` | spec「按元素列出 category 分布」指契约形状分布；填成判据分类会让 `INDEX` 随债数漂移。`absent` 元素无 `$defs` ⇒ 该表为空、整段省略（事实校准 14） |
| D27 | `x-class` 的两条反向校验落点 | **错误**：`x-class: true` 的 `$defs` 条目同时含 `properties` 或 `required` ⇒ `schema::verify_defs` 返回 `ApiError::Contract`；**警告**：`properties` 非空且全为 `x-function` 而未标 `x-class` ⇒ **`emit::render_target`（语言分派处）** 在分派前 `eprintln!` 一行——单点、每元素恰一次 | 二者性质不同：前者是契约自身矛盾，必须拦住；后者是意图提示（函数指针结构体是合法意图）。警告不进 `ArchiApiFindingKind`（该枚举封十值，且 `apiFindings` 是「契约 vs 代码」判据面），故走运行期 stderr，`--check` 退出码不受影响（事实校准 15） |
| D28 | **`server:check` 的 fmt 段改为校验模式** | `server:check` 由 `cargo fmt --all && cargo clippy …` 改为 **`cargo fmt --all --check && cargo clippy …`**（只换 fmt 段；clippy 段逐字不动）。**六份 `sdk/**` 不在本变更范围内**：它们经 `rustfmt --edition 2021 --check` 对 `git show HEAD:<path>` 逐份核验 **6/6 exit 0**、工作树 `git diff HEAD` 为零 ⇒ 无债务可清偿 | 写模式门禁使「收尾还原」不收敛（事实校准 18）——该问题靠「把门改成校验模式」根治，不靠清偿债务；六份「有债务」的旧判断经实测为假 |
| D29 | **生成物必须是格式化器不动点** | `.i.rs` 的输出 MUST 是 `rustfmt --edition 2021` 的不动点；`.i.ts` 的输出 MUST 是 `vp fmt` 的不动点（prettier 把 `["a","b"]` 归一为 `['a','b']` ⇒ 生成器一律用**单引号**）。落地方式：把格式化器的正常形**内建进渲染模板**，由**任务自查行的 `--check`** 兜住；**MUST NOT 在生成器里 spawn 格式化器**，MUST NOT 为此给 `package.json` 加第二段守卫 | 若生成物不是不动点，`client:check` 的 `vp check --fix` 会持续重写它们 ⇒ `api:check` 的字节零 diff 门**永久对撞**（与 I6 同一失败类）。不 spawn 保住了 `archi` 零 spawn 红线与工具零子进程纪律 |
| D30 | **封闭数 60 与 AC-16 判据扩容** | 允许改动集合 = 变更清单 **40 行 / 42 路径**（新增 **18 行 / 18 路径** + 修改 **22 行 / 24 路径**）+ 机械随动白名单 **18 路径**（路径交集为空）= **60 路径**；`package.json` 行尾取 `vp fmt` 不动点 LF（两侧同形） | **六份 `sdk/**` 不在任一集合内**（零 diff，无需允许项）⇒ **行尾面不构成允许项**：交付态 `--numstat` 与 `--ignore-cr-at-eol` 逐项相等，原先记的 `package.json` 归一实际未发生（两侧皆 LF）。**交付态并集另计**（见核对口径段：60 ∪ 测试相位 44 = **104**） |
| D31 | **spec 措辞冲突的 owner 与落地形态** | 「既有三段命令形态 / 原文逐字不变」这类**规范文本与实现形态的冲突** SHALL 作为可执行交付项列在「spec 措辞同步项」小节（**S-1…S-17**），逐条给「原文（须消失）+ 替换后原文（须在场）+ 校验命令 + owner 相位」；判据是 `grep -F` 的「零命中 / 恰命中」双向对拍。**射程自检的 `grep` 片段 MUST 从表机械抽取，MUST NOT 手抄**。owner 三分：delta 十五条归本 change spec 相位；主基线两处（S-4 / S-5）归**归档 SpecSync 步** | 旧稿把它写成散文等于「设计自认与 spec 冲突但仍按设计实现」——无 owner、无判据、无交付检查点，这正是多轮回溯的直接原因。该冲突只能由文本改动消解（代码侧已是实现事实） |
| D32 | **CLI 的 root 锚定（观察根）** | root SHALL 由**向上取最近的含架构域祖先**得到：从 `std::env::current_dir()` 起逐级 `parent()` 判 `foundation::layout::archi_dir(candidate).is_dir()`，首个命中者即 root；全不命中 ⇒ 退化为 cwd（`current_dir()` 失败退化为 `.`）。MUST NOT 把 root 取作 cwd、MUST NOT 接受 `--root`、MUST NOT 在源码内写任何路径字面量 | `api:export` / `api:check` 是 pnpm 脚本，pnpm 恒以 cwd = `packages/desktop` 执行，而架构域在仓库根 ⇒ cwd 作 root 会让生成物落错位置、`--check` 恒报缺失、AC-12 必红；且零行内字面量让「仓库根在哪」由 layout 回答（事实校准 23）。**与 D21 的边界**：D21 定「`--check` 怎么判」，D32 定「root 从哪来」 |
| **D33** | **路径形态校验是四类派生的共同前置（安全边界）** | `paths::contract_path` / `schema_path` / `generated_path` / `entry_form` SHALL **可失败**：FQN 或元素 `<root>` 不是 repo 相对形态（绝对路径 / 盘符 / `..` 段 / 空段 / 空串）→ `None`。判据由两个私有函数（`is_repo_relative` / `is_fqn`）单点持有；**写盘面另加第二层**：`api_gen::write` 先以 `file.path.starts_with(root)` 复核 | `Path::join` 的语义就是风险本身：绝对路径 / 盘符会替换 base、`..` 会上溯。放行即让 `is_file()` 探测变成任意文件读、写盘面变成任意文件写，两层判据交叉验证（沿 `docs::resolved_within` 先例） |
| **D34** | **`--check` 的孤儿扫描覆盖任意深度的元素根同级生成物** | `generated_files_on_disk` SHALL 全树递归找 `*.i.rs` / `*.i.ts`（命名即身份），并递归收集契约目录内的 `*.schema.json` / `INDEX.toml`；跳过 `target` / `node_modules` / `dist` / `build` / `.git` 且有深度上限。MUST NOT 只扫仓库顶层 | 元素根落点分布在任意深度（两层 / 三层 / 顶层三形态）；只扫顶层会让除顶层外的全部孤儿对门禁不可见——「应删未删」正是删元素 / 改根路径后最可能的残留 |
| **D35** | **`build_schema` 的两处键空间各自唯一（撞名即契约错误）** | `$defs` 内 `api-types` 与 `api-reexport` 不得同名；`properties` 内 op 键与类型键不得同名。两处都**先判后写**（`Map::insert` 会静默覆盖），撞名即 `ApiError::Contract` 并点名该名与键空间 | 静默覆盖不是「宽松」，是把契约内部矛盾藏进产物——下游只看到一个赢家，无从知道有冲突 |
| **D36** | **`ApiCheckInput.absent` 是死数据，删除** | `ApiSnapshot` 与 `ApiCheckInput` 的 `absent` 字段一并移除；装载期不再记「缺契约清单」，判据 ② 按 `element.api == Declared && contract.is_none()` 直接判 | 该字段零消费者，制造同一事实的第二个来源；删掉后装载器少一次状态维护，判据行为面逐字不变 |
| **D37** | **契约 fence 内的重复键在文本层判（`serde_json` 折叠后不可达）** | `contract::parse` 在把 fence 内容交给 `serde_json` **之前**先做一次**对象层级键唯一性**扫描；命中即 `ApiError::Contract`，文案点名该键、fence 语言标与 `file:line`。扫描只覆盖**对象**，且 MUST 与实际解析用同一份文本。`schema::verify_defs` 的 `&Value` 入参与该判据**结构上无关**，MUST NOT 承担该职责 | `from_str` 的语义就是「后者胜、前者消失」，`&Value` 入参看不到已消失的那一份；唯一能看见它的是**文本**，而 `contract.rs` 是唯一持有 fence 原文的地方（事实校准 26） |
| **D38** | **`#[path]` 的值是字面量，`paths::mod_path_attr` 是等值判据的真值源** | `#[path = "<值>"]` 的 `<值>` MUST 是**字符串字面量**（Rust 属性值不能由函数求值）；`mod_path_attr(form, name) -> String` 的职责是**产出被断言的真值**，其消费面是 ① `paths_test` 的三形态断言、② tasks 自查行对每个已编排入口的「字面量 == 函数输出」比对。入口行 MUST NOT 出现与函数输出不等值的形态；`crates/infra/archi/src/lib.rs` 的注释 MUST 如实表述 | 旧口径把「单点函数产出」当成可执行纪律，但属性值只能是字面量——于是它退化为一句无法兑现的注释加一个零产品调用者的函数。改判后两侧各归其位：字面量在人写的入口行里，等值性由机械断言守（事实校准 28 同族） |
| **D39** | **knip 输入本期不读（`Visibility::Unused` 生产面不可达是显式留白）** | 本变**不**读 knip：`archi-api-gen` 的 `bootstrap` 与 `api::bootstrap` 均不读；`extract::ts::apply_knip_report` 保留为**未接线的接口**（签名与 `None` 语义不动），产品路径**零调用**；`ts_test` 的五条用例继续在测它。两处 doc MUST 写明「本期生产面不产出 `Unused`」 | 三处旧说法互斥（D19-b 说「仅由 knip 降级」/ CLI 与 `bootstrap` 说「由 CLI 读」/ 待决 5 说「本期不落实现」），在盘事实站在后者一侧（事实校准 27）。选「不读」的三条理由：报文形态随 knip 版本漂移、ignore 面与元素边界不重合、`Unused` 的唯一消费者是本期只对 Rust 用的 bootstrap |
| **D40** | **透传条目的 `<name>` 取目标元素的路径末段，Rust 模块路径取该元素根的 `::` 形态（不取 FQN）** | 渲染透传条目时，TS specifier 的 `<name>` MUST 由 `paths::module_name(<目标元素根路径>)` 得出，MUST NOT 从 FQN 派生；**「FQN → 元素根路径」的查表属调用方**（`render_all` 经私有 `elements_of` 取表，该函数单点委派 `dto::graph_view`——模型结构错即 `Err(ApiError::Element)`），渲染器 MUST NOT 自行查表或读模型。**承载形态唯一**：`EmitTarget.modules: BTreeMap<String, ModuleRef>`，键 = 目标元素 FQN，`ModuleRef { name, module_path }`。查不到目标元素 ⇒ 该条目不产出 specifier（留 `// 透传自 <FQN>` 注释），MUST NOT 退化到 FQN 末段。Rust 侧同源：`pub use` 的模块路径取 `paths::rust_module_path` | spec 对 `<name>` 有唯一判据「取**路径末段**而非 FQN 末段」，而实盘 `emit/ts.rs` 正是第二套命名（`target.rsplit('.')`），且它在试点元素上不暴露（试点无 `api-reexport`）——属判据缺失导致的静默分叉（事实校准 28） |
| **D41** | **入口形态由调用方经 `paths::entry_form` 求出后传入，渲染器零形态推导** | `emit/ts.rs` 的私有 `entry_form_of` MUST 删除；`EmitTarget` 扩 `form: EntryForm`（调用方调 `paths::entry_form` 得）；调用方拿不到入口时该契约不产 `.i.ts`（与「零内容不生成」同口径）。渲染器 MUST NOT 自行判断「末段是否含 `.`」 | 判定同一事实的两处判据必然分叉：`paths::entry_form` 按**盘上命中**判，`entry_form_of` 按**字符串形态**判；后者在「目录名含点」或「单文件元素根带 `/index.ts`」时即错判，后果是 specifier 层数错，只有到 `tsc` 才暴露 |
| **D42** | **`paths::generated_path` 的入参收窄为 `<root>` 字符串（删除 dummy `ArchiElement`）** | 签名为 `pub fn generated_path(root: &Path, element_root: &str, kind: GeneratedKind) -> Option<PathBuf>`（形态校验仍走 `checked_root`）；「容器元素天然无生成物」由**调用方**以 `element.path.first()?` 表达，`paths` 层 MUST NOT 接收整个 `ArchiElement`；`api/mod.rs` 的 `generated_path_of` 随之只算 `(root, path[0], kind)` | 函数只需要一个字符串却要求一个十字段结构体，于是调用方造了个全占位的 dummy 元素——接口面大于需求面的典型后果：dummy 值一旦被下游误用即产生静默错误，编译器不会提醒（因为 dummy 是合法值） |
| **D43** | **位置词汇与签名短形各只有一份写出点** | `Position { Param, Return, Field }`、`schema_type`、`signature_of` 三者定义在 `api/extract/mod.rs`，是**唯一**写出点；`check.rs` MUST NOT 自建本地 `Position` / 本地归一，两侧渲染器（`emit/{rust,ts}.rs`）MUST NOT 自建本地 `Position`，一律 `use` 该定义 | 判据与渲染共用一份位置词汇才能让「可选参数 + `Option<T>`」的归一不漏分支；第二份位置表即稳定假阳的来源（spec「位置词汇与签名短形 MUST 各只有一份」的落地） |
| **D44** | **`apiMissing` 的 op 与 `$defs` 类型同口径（同一条判据、同一次求值）** | `fn missing_contracts` 内 op 段之后追加**类型段**：遍历契约 `$defs`，逐条在代码导出集里查；两段共用三条排除面（构造器双向排除 / 透传纯 `$ref` alias 排除 / `x-class` 与 `enum` 计入类型面）。取数面另补**两条结构前提**：`#[path = "..."] mod` 的模块定位与 `pub use <mod>::*` 的通配升格——缺任一条则本判据在自有生成物的元素上恒假阳 | 「契约声明的类型在代码里没有」与「契约声明的 op 在代码里没有」是同一事实的两面；分成两条判据会让报告数无法单调对拍，也会让类型面缺口无人报（spec 明写两类缺口同一条判据） |
| **D45** | **契约文本进生成器经两道人字闸（名字闸 + 转义闸）** | **闸一**（`contract::verify`，语言无关）：**七族名字**——op id / `$defs` 键 / 透传键 / `params[].name` / `$defs` 条目的 `properties` 键 / `api-op` 的 `owner` / `$ref` 末段名——MUST 是 ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*`，判据由**单点** `fn is_identifier` 持有；**`is_identifier` 的射程只到七族名字为止**，MUST NOT 承接末段形态闸（末段由 `paths::is_module_segment` 单独承接，D48 / D50），MUST NOT 被 `paths` 反向复用（依赖方向）；**闸二**（渲染期，语言特定）：一切来自契约的**字符串字面量与注释** MUST 经该语言单点转义（`ts_string_literal` / `ts_comment_text` / `rust_comment_text`；单引号形态不变，MUST NOT 用双引号形态），Rust 枚举值走 `variant_name` 无损归一。**插值点枚举表 8 组**（op 名 / 类型名 / 字段名 / 方法名 / 参数字面量 / specifier / `ModuleRef.name` / `x-function` 参数名），每组须指明承接地（闸一 / 闸二 / `paths::is_module_segment`）。渲染期 MUST NOT 重复校验名字或位置 | 契约文本是**外部可写输入**，直插源码即代码注入（断行 / 引号 / `*/`）；名字族错则在 Rust 侧产出不可编译文本。两闸各管一件事：名字的形状（装载期、语言无关）与字面量的形态（渲染期、语言特定），分工写在 spec 的「四条明确不做」之邻 |
| **D46** | **两侧渲染器出口单形（四参、删异名孪生）** | `emit::rust::render` 与 `emit::ts::render` 各**恰一个**四参出口 `render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String`（`target` **末位**，**不主张**与 `emit::render_target` 同序）；`render_with_root` / `render_with_target` 两个异名孪生**一并删除**；`render_scope` 保持私有。「空 scope」由**入参**表达（空查表 + 形态），MUST NOT 靠第二个函数 | 三个出口意味着三种退化语义，而它们只在「specifier 形态 / 查表」上有差别——差异由入参承载即可。单形让「出口面唯一性」可机械判（带完整签名 `grep -c` = 1），也消掉「测试改写函数名」的隐性漂移 |
| **D47** | **`x-function` 落点封闭二处；两侧 `value_type` 零 `x-function` 分支** | `contract::verify` 内加**落点闸**：`x-function` 只许出现在 ① `api-op` 顶层签名、② `$defs` 条目第一层 `properties` 的值；其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 及更深层）即 `ApiError::Contract` 并点名 JSON pointer。两侧 `value_type` MUST NOT 有 `x-function` 分支（该形态在装载期已被拦住 ⇒ 分支不可达，留着即「同一 schema 两侧产物形态不同」的静默源） | 位置类错误的正确处置是**在契约里搬位置**，不是在渲染期兜底；递归入围等于承认任意深度都有语义，那就要回答「深层函数怎么生成」，收益为零 |
| **D48** | **末段进源码的形态闸按语言分列** | Rust 侧由 `paths::is_module_segment` 判「形态 `[A-Za-z_][A-Za-z0-9_]*` 且非空 **且非 Rust 关键字**」（关键字集由 `const` 封闭持有），不成立即该透传目标**不入查表**、渲染器只出 `// 透传自 <FQN>` 注释（零改名 / 零清洗，MUST NOT 产出 `r#loop`）；TS 侧由 `ts_string_literal` 转义、**零筛选**。`checked_root` 只判整条 `<root>` 的段结构、**不判末段字符集** | 两侧刻意不对称是本条正题：Rust 语法位置只能拒绝（放行关键字即产出 `pub use crate::loop::X;` 这种不可编译文本），TS 落在字符串字面量里、任意文本都有安全形态（且带扩展名的 specifier 是前端单文件元素的合法形式）。合成一条更严判据会误杀合法透传 |
| **D49** | **`#[path]` 的读盘面与写盘面同一条限根纪律** | 取数器定位 `#[path = "…"] mod` 时 MUST ①**在折叠之前**归一反斜杠、②按段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即拒）、③`join` 后复核目标仍在观察根之下（`starts_with(root)`）；任一条不成立即**不命中该子树**（零新错误变体、不 panic） | `#[path]` 的值经手写入口行落地，属可被写错的入参；放行即让「模块树被带出观察根」，与 `paths` 侧的「任意文件读」是同一件事（与 `scan/rust.rs` / `docs.rs` 同一条纪律） |
| **D50** | **`is_module_segment` 的关键字面** | 判据收为**合取二项**：形态 `[A-Za-z_][A-Za-z0-9_]*` 且非空，**且**不在 `RUST_KEYWORDS`（38 严格关键字 + 13 保留字，`const` 封闭持有）；不成立即不入查表。MUST NOT 改成输出 `r#<段>`。**单一承接地**：Rust 侧末段形态闸**只**由本函数承接——`contract::verify` 的 `is_identifier`（D45 闸一）只管七族名字、其射程不含关键字面（契约名不是源码名），两函数**MUST NOT 互相复用或合并**（`paths` 是 `api` 地基层，反向依赖即破层） | 第二形态（raw identifier）会让同一段名字有两种文本形态，而契约只认一种；拒绝入表是零歧义且零清洗的处置（见 D48 的 Rust 侧）。两条同类字符集判据并存**不是冗余**：事实源不同（契约文本写的名字 vs 路径派生的模块段）、处置不同（`ApiError::Contract` 中止装载 vs 降级为注释） |
| **D51** | **`apiMissing` 的 `occurrences.line` 分列** | op 缺口取 `ContractOp.line`（heading 行）；`$defs` 类型缺口取 **`0`**（契约里无对应行号）。MUST NOT 为此扩 `Contract` 的字段面 | 一个字段两种语义必须显式分列，否则「类型缺口的行号」只能靠调用方约定，而约定不可判；`0` 是「位置未知」的既有取值惯例 |
| **D52** | **`extract::extract` 的显式留白** | `extract::extract`（单文件入口面）保留接口与两处 doc 留白声明（说明其产品调用者为零、判据面走 `extract_element`），**MUST NOT** 加产品调用者、MUST NOT 连带降级 `extract_file` 的 `pub` | 零调用者要么给出调用面、要么写成显式留白（D38 的同款形式要求）；它属「诊断与测试面」，保留是为 bootstrap 与单文件驱动的将来用途，删掉会削弱取数面的可测性 |
| **D53** | **第四类机械随动 = 「被删符号的调用形态适配」** | D46 删掉三参与两个孪生后，既有 sidecar 用例的调用是硬编译错误（E0061 / E0433），而 `server:check`（`--all-targets`）要求工作树可编译 ⇒ 该适配是**编译前置**。**射程封闭为三种形态**：(a) `use` 行；(b) 实参补齐（元数），新增实参**只能取既有装置**（`empty_target()` / `empty_scope()`）；(c) 装置同名提取。**MUST NOT 含**：断言值 / 断言意图 / 用例名 / 新增或删除用例 / 夹具语义。**归属按改动种类划、不按文件划**，逐 `hunk` 判；**不增白名单项**（两落点已在测试相位 44 文件集内计数） | 根因是分类缺口（design 未给「符号被删后的调用形态适配」立项），不是实施失误。不立项则实现相位要么留下不可编译形态、要么越界改断言——两者都不可接受 |
| **D54** | **`common.json` 是一等映射规则资产：渲染器 MUST 消费表，MUST NOT 硬编码映射** | ① **资产形态**：`api/common.json` 的键 = **协议名字**（首期唯一键 `Outcome`；**键名两层口径见 D57**——资产键是协议名字，契约内承载键是 `"outcome"`，经 `api::protocol_key` 换算），值 = 每语言**一个槽位**（`rust` / `typescript`）+ 可选 `degraded` / `note`；槽位内用 **`{}`** 标记负载位置（`"Result<{}, String>"` / `"{ ok: {} } \| { err: string }"`）。② **装载**：`api::meta()` 解析该资产进 `ApiMeta.common: BTreeMap<String, CommonRule>`，解析失败 / 缺语言槽位 / 槽位内零 `{}` 即 `ApiError::Contract`（**静默跳过不允许**）。③ **消费**：两侧渲染器各有一个**私有单点** `common_type(schema: &Value, position: Position, common: &BTreeMap<String, CommonRule>) -> Option<String>`——按 `schema` 的**协议键**查表、取本语言槽位、把 `{}` 替换为 `value_type(内容位)`；`function_type` 的 async 分支经**同一出口**取语言侧 async 包装（不另立第二份）。④ **绝对禁令**：`emit/{rust,ts}.rs` 的 `value_type` MUST NOT 再出现映射文本（`Result<{}, String>` / `{ ok: {} } \| { err: string }` / `Promise<` 的硬编码形态），MUST NOT 按协议名走 `if let Some(x) = schema.get("<协议名>")` 直接 `format!`。⑤ **纯函数纪律不变**：数据已进载元，渲染器零 IO、零读盘（D46 的四参签名因此只需扩「载元已带表」，**不扩参数**） | spec `:158` / `:159` 把 `common.json` 立为「随工具发布的映射规则资产、生成物头带其摘要」，而两侧渲染器把两条映射**硬编码**在 `value_type` 里（实测两处硬编码与表的 `rust` / `typescript` 槽位**逐字相同**，事实校准 30）⇒ 资产与实现**零连接**：改表不影响产物，改产物不需动表，「摘要」还照旧变化，等于让头注释宣誓一件不成立的事。三条备选逐项否决：**删表**与 spec `:158` / `:159` 正面冲突；**保留硬编码 + 表只作文档**等于承认资产是装饰（摘要的语义随之失真）；**渲染器读盘**破坏零 IO 与四参纯函数纪律。故正解是「装载期解析进载元 + 渲染期查表」——表是唯一来源，渲染器仍是纯函数 |
| **D55** | **`api::display` 收为跨 crate 单点（`pub` + `lib.rs` 出口），工具 crate 删本地逐字实现** | `api/mod.rs` 的 `pub(crate) fn display(root: &Path, path: &Path) -> String` 升为 **`pub fn display`**，并由 `crates/core/archi/src/lib.rs` 出口 `pub use api::display;`；`crates/tools/archi-api-gen/src/main.rs` 的本地 `fn display` **整条删除**，改用 `archi::display`。语义逐字不变（剥 root 前缀、`\` → `/`、剥前导 `./`） | 工具 crate 物理不可调用 `pub(crate)` 函数，于是复制了一份**逐行相同**的实现（事实校准 29）——同一事实两个来源必然漂移（D36 / D42 / D53 同款）。收单点的代价只有一处可见性提升；`short_digest` / `is_product_rs` 两个 `pub(crate)` 助手仍留 `pub(crate)`（工具侧无消费者，**不得顺手**扩大出口面） |
| **D56** | **装置超时越界弱化回滚（撤回一处与本变更零关联的测试装置改动）；**回滚 owner = 测试相位**** | `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`（**此处即取物理全路径**，沿「核对口径 - 判据串口径」条；下游判据串 MUST 与之同形）的 `spawn_command_confirmer` 由**测试相位**回滚（实现相位对该文件**零触碰**）到约定口径：确认窗 `Duration::from_secs(60)`、**不引入** `register_window`；其退出判据保留「见过 run 登记后转 `None`」这一条既有正确性前提。复合用例 `并行冲突键id_同名异id各自异root同id互不误拒` 的收敛等待**去掉显式 120s 窗口**、回到 `wait_for`（= `wait_for_up_to` 的 30s 约定上限）。该文件的产品面零改动 | 该装置的超时被放宽（180s + 30s 登记窗 / 120s）而 `test-design.md` 首部的既有装置约定是 **30s 收敛上限**，且该文件与本变更**零产品面关联**（不属清单、不属机械随动白名单的字段初始化形态、不属第四类「被删符号的调用形态适配」）⇒ 它是**越界的测试面改动**，MUST 撤回。**归属三定**（本稿裁定，消除「以产品任务形态下达测试改动」的越界）：① 该文件**不入变更清单、不入机械随动白名单、不入实现相位封闭集合**（66 路径不含它）；② 回滚动作的 owner 是**测试相位**——实现相位只**零触碰**（不 `git checkout`、不改其装置与用例），回滚未完成即为测试相位的**接手红点**（其用例面本就在「测试相位产出白名单 - 壳命令面用例」组内）；③ 机械判据 `from_secs(180)` / `register_window` 零命中 + `wait_for_up_to` 调用点在场的**执行与闭合**同归测试相位（判据串取**物理全路径** `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`，MUST NOT 取 `src/commands/…` 省略形态）。判据的法条不动，只改 owner 与承载形态（事实校准 33） |
| **D57** | **协议键的两层口径：`common.json` 的键 = 协议名字（`Outcome`），契约 / schema 内的承载键 = 协议名字的 JSON 键（`"outcome"`），两者 MUST 经单点函数换算** | ① **资产键**：`api/common.json` 的顶层键 = **协议名字**，首期唯一键 **`Outcome`**（与 spec `:149` / `:158` / `:185` 的用词、`ApiType::Outcome` 变体名、`#/$defs/Outcome` 三处一致）。② **契约承载键**：契约 `api-op` 签名与 `$defs` 条目里该协议的 JSON 对象键 = **`"outcome"`**（小写；这是**契约作者书写的键**，也是 `schema_type` / `value_type` 的取值键，实盘 `contract` 与 `bootstrap` 的草稿生成同形）。③ **换算单点**：`fn protocol_key(name: &str) -> String`（`api/mod.rs`，`CommonRule` 的持有者一侧）把协议名字转成承载键（首期 = 小写化）；`common_type` 按协议名字查表后，用同一函数的输出作 `schema.get(..)` 的取值键。**MUST NOT** 在两侧渲染器里各写一份键变换，MUST NOT 让 `common.json` 的键改成小写（那会让 spec 三处用词与 `ApiType::Outcome` 变体名同时变成第二形态）。④ **判据口径**：AC-31 ③ 的「硬编码零残留」判据串锚**渲染出的文本形**（`Result<{}, String>` / `{ ok: {} } \| { err: string }`），MUST NOT 用 `"outcome"` 作判据串（它是合法承载键，恒在场） | 两处旧措辞在此正面冲突：spec 与 `common.json` 用 `Outcome`（类型 / 协议名），渲染器与契约用 `"outcome"`（JSON 键）。把二者混为一谈的后果是「改表不影响产物」（查表用 `Outcome` 却拿 `"outcome"` 去取值 ⇒ 恒 miss ⇒ 静默退回硬编码分支）；反之把表键改成小写则与 spec 三处用词、`ApiType::Outcome`、`#/$defs/Outcome` 全面失配。正解是**显式两层 + 单点换算**：名字给人看与给 spec，键给 JSON 看 |
| **D58** | **`EmitTarget` 收为三字段 `{ kind, form, modules }`：`path` / `element_root` 两字段删除（产品面零读取者）** | ① **字段面**：`pub struct EmitTarget { pub kind: GeneratedKind, pub form: EntryForm, pub modules: BTreeMap<String, ModuleRef> }`——三字段；`path: PathBuf` 与 `element_root: String` **整条删除**。② **构造面唯一**：`api::render_all` 建 `EmitTarget` 处只算三值（`kind` 经 `paths::generated_kind` / `form` 经 `paths::entry_form` / `modules` 经私有 `module_refs`），生成物的落点 `path` 与元素根 `scope` 留在 `render_all` 的局部变量里**直接用于 `GeneratedFile { path, .. }`**，MUST NOT 为凑字段面把它们再塞进 `EmitTarget`。③ **specifier 的唯一决定面**：TS specifier 文本 **只由 `form` 与 `ModuleRef.name` 决定**（值 = `paths::ts_import_specifier(target.form, module.name)`，再经 `ts_string_literal` 转义）——`element_root` 不在其中，因此不存在「元素根影响 specifier 形态」这条通路；MUST NOT 在 design / spec / 测试文稿里写「`element_root` 只影响 TS specifier 的形态语境」。④ **诊断定位的真实载体**：渲染期的错误与注释文本取 `Contract.file`（每个 `ApiError::Contract` 都带 `file:line`），头注释四句全部来自 `ApiMeta`——两字段删除后**零可观测行为变化**（生成物逐字节不变）。⑤ **射程（现算：产品 2 处 + 测试面 19 处构造形态）**：产品面 = `emit/mod.rs` 的结构定义 + `api/mod.rs` 的构造点（`render_all`）；测试面 = 五份既有用例里共 **19 处** `EmitTarget {` 构造字面量（逐文件 `grep -cE '^\s*(let [a-z_]+ = |&)?EmitTarget \{$'` 现算：`emit/mod_test.rs` **2** + `emit/rust_test.rs` **5** + `emit/ts_test.rs` **10** + `api/paths_test.rs` **1** + `tests/pilot_test.rs` **1**），两字段初始化行合计 **38 行**（每处 2 行）；**MUST NOT 用 `grep -c 'EmitTarget {'` 计数**——该串同时命中 `-> EmitTarget {` 返回类型行、doc 注释与 `.find("pub struct EmitTarget {")` 字面量，删前给 26、删后给 28，与「构造字面量」不是同一事实（伪判据）。删两字段的初始化行属 D53 第四类机械随动、归实现相位执行，**不增机械随动白名单项**（五路径已在测试相位 44 文件集内计数）；逐条在 `tasks.md` 阶段三十 ①②③④ 与 ⑤ 的移交项 | 产品面读 `EmitTarget` 的点恰三处（`target.kind` / `target.form` / `target.modules`），`path` / `element_root` **零读取点**（事实校准 34）；两字段的 doc 自称「供调用方写盘与头注释溯源 / 仅用于注释与诊断定位」，而头注释与诊断**都不读它们**——这正是 D42（dummy `ArchiElement`）的同一病因：接口面大于需求面时，占位值会被下游当作事实使用且编译器不提醒。第二层理由是可判性：删掉两字段后「specifier 只由 `form` + `ModuleRef.name` 决定」成为**只有一条通路**的事实，测试面无需再为「元素根是否影响 specifier」保留一条无对象的断言 |
| **D59** | **`oneOf` / `anyOf` 的分支负载一律经 `value_type(choice, Position::Field, common)`：私有 `choice_payload` 删除** | ① **删除**：`emit/rust.rs` 的私有 `fn choice_payload(choice: &Value, _common: &BTreeMap<String, CommonRule>) -> Option<String>` **整条删除**（其 `_common` 形参本就零使用）。② **替换**：`variants_block` 的分支判定改为经 `value_type` 的同一条通路面——分支属「具名类型 / 协议键」时产带负载变体（`variant_name(&payload) + payload`），属**封闭标量之外**的形态时退 `Variant{index}`；判定与取值**都从 `value_type` 的返回值来**，MUST NOT 在 `variants_block` 内重写第二份标量表。③ **可判性判据（同源不出分叉）**：同一 `choice` schema 经 `value_type(choice, Position::Field, common)` 与经 `variants_block` 产出**逐字相等**。④ **零新符号**：不新增「判是否可以作变体负载」的第三函数——判定即「`value_type` 的返回是否等于兜底文本 `serde_json::Value`」（退化为兜底即不作负载、出 `Variant{index}`），单点仍在 `value_type` 的 `type` 分派上。⑤ **行为变化的**精确**射程（MUST 如实登记，不得声称「逐字无变化」）**：`$ref` 与四种标量（`string` / `integer` / `number` / `boolean`）**产出逐字不变**（两种写法同值）；**新增**带负载形态的是此前 `choice_payload` 落 `None` 而 `value_type` 能给非兜底文本的三类——**协议键分支**（`{"outcome": {...}}`：经 `common_type` 展开，此前落 `Variant{index}`）、**`array` 分支**（`Vec<T>` / `T[]`，此前落 `Variant{index}`）、**带 `additionalProperties` 的 `object` 分支**（`BTreeMap<String, T>`，此前落 `Variant{index}`）；**仍落 `Variant{index}`** 的是「`object` 无 `additionalProperties`」与「无 `type` 且非 `$ref`」（`value_type` 给 `serde_json::Value` / `unknown` 兜底）。**这是修正而非回归**：TS 侧的 `oneOf` 分支早已经 `value_type`（`union_of`），故上述三类在 TS 侧一直有形态、Rust 侧一直退化成 `Variant{index}` —— 本裁定把 Rust 侧对齐到同一通路。**试点零影响**（试点契约无 `oneOf` / `anyOf`，实测 `grep -c 'oneOf' .dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.schema.json` ⇒ **0**），故四类生成物逐字节不变。⑥ **射程**：`emit/rust.rs` 的 `variants_block` + 被删函数 + 既有用例 `render的oneof为带负载enum`（`$ref` 分支断言 `AA(A)` 不变）；新增的「协议键 / `array` / `object(additionalProperties)` 三类带负载」正例归测试相位（`tasks.md` 阶段三十 ⑤ e） | `choice_payload` 违反两条已裁定的唯一性：「位置 × 语言 → 文本形」的**唯一实现**是 `value_type`（D43 ④：`emit` 独占该映射，判据侧只经 `schema_type` / `signature_of` 取数），映射规则的**唯一查表面**是 `common_type`（D54 ③）。它自带未使用的 `_common`（事实校准 35）说明它本应查表却绕过了表；自写一份标量映射则让同一 `oneOf` 分支在「字段位 / 见证位」（走 `value_type`）与「变体负载位」（走 `choice_payload`）产出不同文本 —— 同一 schema 在两条路径上产出互不相同的负载形态，是「静默分叉」而非实现风格。裁定「一律经 `value_type`」而非「给 `choice_payload` 补查表」的理由：后者会让同一映射在**两处**实现，将来加一个协议键就要同步两条分支，正是 D43 / D54 要消灭的形态 |
| **D60** | **`api/mod.rs` 的 `pub use check::category_of;` 删除：`category_of` 不属 `api` 的出口面** | ① **删除**：`api/mod.rs` 的 `pub use check::category_of;` 整行删除；`check::category_of` 的 `pub` **不动**（同 crate 面仍需它，且 `check.rs` 的 `entry_of` 是它的唯一产品调用点）。② **消费面口径**：`category_of` 的读取方只有两处——`check.rs` 自身（`category_of(kind)`）与 `check_test.rs`（`use super::check::{category_of, check}`，走**模块路径**而非 `api::` 重导出）⇒ 重导出零消费者。③ **出口面纪律（与 D55 同一条）**：`api/mod.rs` 的 `pub use` 面 = 「crate 外或兄弟模块**实际消费**的符号」；`short_digest` / `is_module_segment` 一类私有工序既不入公共函数表也不扩大出口面（D55 已明禁「顺手扩大出口面」），`category_of` 属同一类。④ **判据**：`grep -cF "pub use check::category_of" crates/core/archi/src/api/mod.rs` ⇒ **0**；`grep -c "fn category_of" crates/core/archi/src/api/check.rs` ⇒ **1**（定义仍在、可见性不变）；`check_test.rs` 的 `use super::check::{category_of, check}` **逐字在场**（证明删出口不动测试面）。⑤ **不得反向解释**：本条 MUST NOT 被读成「`category_of` 不再是开放分类的单点」——打标单点仍是它（D15 不变），本条只裁出口面 | 出口面上多一个无消费者的名字不是「多写一行」而是**契约面扩大**：`pub use` 一旦落地即成事实上的公共 API（`api/mod.rs` 的出口面在 design 公共函数表与 `lib.rs` 的 `pub use api::{…}` 两处都被点名），而 `category_of` 既不在公共函数表的出口名单内（表里它是 `check.rs` 的行，不是 `api` 的出口），也不在 `lib.rs` 的重导出清单里 ⇒ 它经 `api/mod.rs` 重导出属于**第三条通路**，实际零消费者（事实校准 36）。更直接的证据是同文件的 `mod_test.rs` 断言「`mod.rs` 内零 `category_of` 调用」——那正是「形状分布不借判据分类」的纪律（D26），保留重导出会让该纪律在出口面出现语义倒错 |


---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 契约面装配与出口 | `meta`（生成载元单点，**含 `common` 表的解析**，D54）/ `enabled`（契约面是否启用）/ `load_contracts`（读模型元素表 → 逐元素定位 md → 解析 → 双向 gate）/ `render_all`（装配四类生成物为内存字符串）/ `bootstrap`（出 stdout 草稿）/ `check`（十判据入口）/ `api_ref` 与 `api_ref_of`（契约状态投影）/ `display`（repo 相对串单点，D55）；`ApiSnapshot` 为一次装载的只读快照；`parse_common` / `protocol_key`（D54 / D57 的资产解析与键换算单点） | `crates/core/archi/src/api/mod.rs`（新） | `crate::api::{paths, contract, schema, extract, emit, check}`、`crate::dto`、`crate::drift::report`、`foundation::layout`、`serde_json`（载元解析） | 纯读盘（编译期内嵌资产）、零写盘、零 spawn |
| 路径单点 | `<root>` 归一、契约目录与索引路径、四类路径派生、入口查找与形态判定（`entry_form` 是形态的**唯一**判据，D41）、`#[path]` 值的**等值真值源**（`mod_path_attr`，D38）、`is_module_segment`（Rust 末段形态闸，D48 / D50）、TS import specifier、跨域 `$ref` 相对路径 | `crates/core/archi/src/api/paths.rs`（新） | `foundation::layout::{archi_api_dir, archi_dir}`、`crate::dto::ArchiElement`（仅 `entry_form` 的入参） | 纯路径运算（`std::path`），零 IO（`exists()` 只在投影层用）；`generated_path` / `entry_form` 的形态校验是安全边界（D33） |
| 契约 md 解析 | H1 锚点校验、`##` 散文区、`### <op-id>` + `api-op` fence 分段、`api-types` / `api-reexport` fence 提取、**fence 内重复键的文本层判据**（D37）、**闸一七族名字 + `x-function` 落点闸**（D45 / D47）、双向 gate 三条、构造器声明检测；错误带 `file:line` | `crates/core/archi/src/api/contract.rs`（新） | `serde_json`、`crate::api::paths` | 行扫描 + fence 状态机（零 markdown 依赖）；重复键扫描在 `from_str` **之前** |
| `$defs` 规则与装配 | 五条规则 + `x-class` 带属性判错；`schema.json` 六字段装配；透传条目纯 `$ref` 校验；`INDEX.toml` 装配（FQN 字典序 + `$defs` 形状分布）；两处键空间唯一性（D35） | `crates/core/archi/src/api/schema.rs`（新） | `serde_json`（`Value` + `Map` 保序）、`toml_edit`（INDEX 文档构造） | 装配非翻译；`Map` 保序保证幂等 diff |
| 映射规则资产与消费 | `Outcome` 信封的跨语言展开规则（首期唯一键 = 协议名字），编译期内嵌；两侧渲染器经 `common_type` 查表取本语言槽位（D54），承载键经 `protocol_key` 换算（D57） | `crates/core/archi/src/api/common.json`（新）+ `api/mod.rs` 的解析 + `emit/{rust,ts}.rs` 的 `common_type` | 无（静态 JSON） | 静态 JSON + 载元携带（零 IO 渲染） |
| 取数器分派 | 按元素语言分派 `rust` / `ts`：`extract`（单文件入口面）/ `extract_element`（整元素导出集，**判据唯一取数面**）/ `normalize_signature` / `schema_type` / `signature_of` / `Position` | `crates/core/archi/src/api/extract/mod.rs`（新） | `crate::dto`、`crate::api::paths` | 纯 AST 归一层（零 IO） |
| Rust 声明抽取 | `syn` 遍历：`pub struct` / `trait` / `enum` / `pub fn`；`visibility` 三值判定（`External` / `CrateOnly` / `TestOnly`，`Unused` 不产）；整包走模块树 + crate 根 `pub use` 白名单；`#[path]` 读盘面三步限根（D49） | `crates/core/archi/src/api/extract/rust.rs`（新） | `syn`（`full`）、`proc-macro2`（`span-locations`）、`crate::api::paths` | AST，**零正则** |
| TS 声明抽取 | `oxc` 遍历**只抽导出面**（D19-a）：`export function` / `type` / `interface` / `class` / `const` / `enum` 分类，箭头函数不作操作，`export default` 留白；`visibility` 生产面只产 `External`；`apply_knip_report` 未接线（D39） | `crates/core/archi/src/api/extract/ts.rs`（新） | `oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax`（同版） | AST，零跨文件引用图（留白因此成立） |
| 十判据求值 | entry 优先互斥 → `apiMissing`（op **与 `$defs` 类型**同口径，D44）→ `apiUndeclared`（Off）→ 签名归一比对 → 留白矛盾 → 嵌套与透传五项；`category_of` 单点打标（**只在本模块被消费，不经 `api` 出口面**，D60）；`occurrences.line` 分列（D51） | `crates/core/archi/src/api/check.rs`（新） | `crate::api::{contract, extract, paths, schema}`、`crate::dto`、`crate::drift::report` | 纯函数（入参全为借用切片）；`$defs` 只读 `schema.json` |
| 渲染：分派与类型 | `pub mod rust; pub mod ts;` + `EmitTarget`（**三字段 `kind` / `form` / `modules`**，D58）+ `ModuleRef` + `render_target`（语言分派；分派前发 `x-class` 警告，D27） | `crates/core/archi/src/api/emit/mod.rs`（**设计补入**） | `crate::api::{contract, paths, schema, ApiMeta}` | 纯函数出 `String`；**形态与查表是渲染器的全部入参面**（无落点 / 无元素根，D58） |
| 渲染：Rust | `.i.rs`：`struct` / `trait` / `enum` / newtype / 自动构造器 / 透传 `pub use` / 头注释 / 签名见证；`common_type` 查表（D54）；`oneOf` / `anyOf` 的分支负载经 `value_type`（**无 `choice_payload` 第二份映射**，D59） | `crates/core/archi/src/api/emit/rust.rs`（新） | 同上 | 纯函数，零 IO 零 stderr（格式化器不动点，D29） |
| 渲染：TS | `.i.ts`：`interface` / `as const` + `type` / 别名 / 透传 `export type { } from` / 头注释；`common_type` 查表（D54）；单点转义（D45 闸二） | `crates/core/archi/src/api/emit/ts.rs`（新） | 同上 | 纯函数，零 `eprintln!`（单引号形态，D29） |
| 报告词汇与槽位 | `ArchiApiFindingKind`（十值 + `ALL` + `as_str` + `severity`）/ `ApiFindingSeverity` / `ArchiApiFinding` / `ArchiDriftReport.api_findings` | `crates/core/archi/src/drift/report.rs`（改） | 无 | 封闭词汇 + `match` 穷尽 |
| 工具边界（CLI） | `generate` / `--check` / `bootstrap` 三子命令；root 锚定（D32）；写盘（含 `starts_with(root)` 复核，D33）与差异清单；`display` **改用** `archi::display`（D55） | `crates/tools/archi-api-gen/src/main.rs`（新） | `archi` + `foundation` + `serde_json` | argv 手写解析（无 clap）；零 Tauri / 零 store / 零 config / 零 orchestration |

---

## 变更清单 (Change Inventory)

> 口径：**新增文件**用仓库根相对全路径；**修改文件** Rust 侧省略 `packages/desktop/src-tauri/` 前缀，前端侧写全 `packages/desktop/…`。
> **测试文件不入清单射程**（`*_test.rs` / `tests/**` / `*.test.ts(x)` 的用例与装置归测试相位）。实现相位为**可编译**承担**四类机械随动**（逐项在下方「机械随动白名单」，共 **18 个文件**，全部是测试文件）：① 后端 `Element { … }` / `ArchiElement { … }` 字面量补 `api`（8 个文件各一处）；② `ArchiDriftReport { … }` 字面量补 `api_findings`（2 个文件 3 处）；③ `bindings:export` 之后前端 8 个 `*.test.ts(x)` 的字段补齐；④ **第四类 = 被删符号的调用形态适配**（D53，落 2 个 `api/emit/*_test.rs`，**不增白名单项**——它们已在测试相位集合内计数）。
>
> **核对口径（闭环全文只此一处，收尾自查 F 按此执行；本稿按盘上实测**重算**，前稿的 46 / 40 / 28 / 64 / 65 / 115 全部作废）**：
> - **清单 = 40 行 / 42 路径** = 新增 **18 行 / 18 路径** + 修改 **22 行 / 24 路径**（22 行中 **2 行含多文件写法**：`bindings.ts` + `bindings.zod.ts`、`AGENT.md` + `CLAUDE.md` ⇒ 22 − 2 + 2 + 2 = **24**）。**六份 `sdk/**` 已移出修改表**：它们在 `HEAD` 已无 rustfmt 债务（`rustfmt --check` 对 HEAD 副本 6/6 exit 0），清偿是空动作。
> - **行数口径 MUST 现算**：`awk '/^### 新增文件/,/^### 修改文件/' design.md | grep -c '^| `'` ⇒ **18**；`awk '/^### 修改文件/,/^### 机械随动白名单/' design.md | grep -c '^| `'` ⇒ **22**。（以现算为准，MUST NOT 手抄。）
> - **机械随动白名单 = 18 路径**（表体 18 行逐行一文件：后端 `Element` / `ArchiElement` 字面量补 `api` **8** + `ArchiDriftReport` 字面量补 `api_findings` **2** + 前端 `*.test.ts(x)` 字段补齐 **8** = 18。**六份 `sdk/**` 已移出修改表**（`HEAD` 无债务）；`bash.rs` 的 `pub(crate)` 提升**归清单修改表**、**不在本节**——旧稿把它同时写进两表，是「并集 66 ∪ 47 = 113」算错的唯一来源，本稿按「产品文件只归清单」单列）。
> - **实现相位封闭数 = 42 + 18 = 60 路径**（清单 42 与白名单 18 的路径交集为空 ⇒ 直接相加）。
> - **测试相位产出白名单 = 12 组 / 44 文件**（表体逐组相加 = 44；**组数与文件数分列**——条目数不单列）。
> - **交付态并集 = 60 ∪ 44 = 104 路径**（并集是**上界**，MUST NOT 与「实测改动行数」互相换算；两集交集为空，已机械核对）。
> - **路径归一口径（本节唯一，清单 / 机械随动白名单 / 测试相位产出白名单三表共用）**：三表的所有路径在进入任何机械集合前 MUST 经同一步归一——**Rust 侧补回 `packages/desktop/src-tauri/` 前缀**（修改表与机械随动白名单表内按省略形态书写者）、**前端侧保持 `packages/desktop/` 起头不变**、**`…/` 缩写 MUST 展开为同组首条的全路径**（首条自身可能仍需补 Rust 前缀）。归一后**每条 = 仓库根相对的物理路径，恰一行、无 `…` 缩写、无前缀省略**，与 `git status --porcelain -uall` 的路径列逐字可比。
> - **`/tmp/expected.txt` 的生成口径**：= 清单 42 路径 ∪ 机械随动白名单 18 路径（交集为空，故 **60 行**），**行内容即上述归一后的物理路径**，MUST NOT 出现 `src/…` 或 `crates/…` 这类省略形态、MUST NOT 出现 glob。`/tmp/test_phase_whitelist.txt` 同口径（**44 行**）。
> - **判据串口径**：凡以某文件是否在集合内 / 是否在盘为断言的 `grep -cF`，串 MUST 取**物理全路径**（例：`packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`），MUST NOT 取省略形态（`src/commands/…`）——省略形态在物理路径集合里必然 0 命中，与判据期望值相反（本节新增的闭合项，前稿缺此归口）。
> **判据权威是收尾自查 F 的单条等式**：`comm -3` 双向差集 MUST 为零行。
> ```bash
> git status --porcelain -uall | grep -v '^.. openspec/changes/module-api-contract-format/' | sed 's/^...//' | LC_ALL=C sort -u > /tmp/actual.txt
> LC_ALL=C sort -u /tmp/allowlist.txt | comm -3 - /tmp/actual.txt   # MUST 零行
> wc -l /tmp/allowlist.txt                                          # 104（42 清单 + 18 机械随动 + 44 测试相位）
> ```
> **零例外**：旧稿的「六份 `sdk/**` 恰六行」是「留在修改表但 `HEAD` 已清白」的产物，属把两条互斥断言拼在一起。本稿把这六份**移出修改表**，并让它们回到 `HEAD` 状态（`git diff HEAD` 为空）⇒ `comm -3` 两侧**都必须是零行**，判据不再有任何例外分支。六份的机械随动面仍由「清单不覆盖它们」这一事实自然满足（它们本就不需要任何改动）。
>
> **proposal 实现文件表的逐行覆盖**：proposal 的「变更范围 - 实现文件」表 **27 行**（`grep -o '^| \`[^\`]*\`' proposal.md | wc -l` 现算），**全部被本清单覆盖**（拆细后落点见表）。四处需要留痕的差：
> ① **设计补入 5 项**（proposal 未列或只列其一）：`crates/core/archi/src/lib.rs`（模块声明与出口，含 D55 的 `display` 出口）、`crates/core/archi/src/api/emit/mod.rs`、`api/common.json`、`crates/infra/archi/src/report.rs`（试点 op 的实际来源）、`src-tauri/Cargo.lock`（workspace 成员变化的自动随动，事实校准 17）。
> ② **proposal「不要修改」里的条件项被兑现为改动**：`crates/infra/agent/**` 的 `archi_knowledge.rs` 与 `archi-knowledge.md`（`api` 词汇在场是 `desktop-archi-model` 四面词汇对拍的要求），两文件进修改表；同段其余（工具面 / 注入矩阵 / policy）零改动。
> ③ **一处路径偏离留痕**（事实校准 12）：proposal 表把试点生成物写在 `crates/infra/archi/archi.i.rs`，与「生成物落根同级」规则冲突；本清单按规则落到 `crates/infra/archi.i.rs`。
> ④ **proposal 表 3 项被拆细**：`api/extract/**`（1 行 → 3 行）、`api/emit/**`（1 行 → 2 行 + `emit/mod.rs`）、工具 crate（1 行 → 2 行）。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | 契约面装配与出口：`meta`（**含 `common` 解析**，D54）/ `enabled` / `load_contracts` / `render_all` / `bootstrap` / `check` / `api_ref` / `api_ref_of` + **`display`（跨 crate 单点，D55）** = **九个 `pub fn`**；私有工序 `index_entries`（`INDEX` 条目装配）/ `categories_of` / `def_shape`（D26）/ `module_refs`（D40）/ `generated_path_of`（D42）/ `parse_common`（D54 唯一解析点）/ `protocol_key`（D57 换算单点）**不入表**；**本文件零 `pub use`（出口面唯一落点是 `lib.rs` 的逐名出口）**——`category_of` 的重导出已删（D60），`contract` / `emit` / `extract` / `schema` 的九个名（`ContractOp` / `ContractSection` / `schema_type` / `signature_of` / `ApiParam` / `Position` / `Visibility` / `build_schema` / `verify_defs`）同样零消费者已删（D60 同口径），其余名字一律降为普通 `use`（只供本文件自用）；类型面 `ApiError` / `ApiSnapshot` / `GeneratedFile` / `ApiCheckInput` / `ApiMeta`（含 `common`）/ `CommonRule` | 公共函数表含本文件 **9 行**（`meta` / `enabled` / `load_contracts` / `render_all` / `check` / `bootstrap` / `api_ref` / `api_ref_of` / `display`）；类型表七行；D12 / D16 / D17 / D20 / D22 / D54 / D55 / D57 |
| `…/crates/core/archi/src/api/paths.rs` | 路径单点：`normalize_root` / `contract_path` / `schema_path` / `generated_path` / `generated_kind` / `entry_form` / `mod_path_attr` / `ts_import_specifier` / `ref_relative` / `domain_of` / `directory` / `index_path` / `module_name` / `rust_module_path` / `language_of` / `is_module_segment`（末项 `pub(crate)`，**私有工序、不入表**） | 公共函数表**本文件零行**：15 个 `pub fn` 是 `api/**` 同 crate 面的路径派生内部件（`grep -c "^pub fn "` 现算为准 ⇒ **15**），不在公共函数表的射程内；类型表 `EntryForm` / `GeneratedKind` / `Language`；`ModuleRef` **不在本文件**（定义在 `emit/mod.rs`，本文件只作消费方：`module_name` 供其 `name`、`rust_module_path` 供其 `module_path`）；D2 / D3 / D4 / D5 / D33 / D40 / D48 / D50 |
| `…/crates/core/archi/src/api/contract.rs` | `<FQN>.md` 解析与双向 gate：`parse`（含重复键文本层判据，D37）/ `verify`（双向 gate 三条 + 七族名字闸 + `x-function` 落点闸，D45 / D47）/ `is_constructor` | 公共函数表含本文件三行；类型表 `Contract` / `ContractOp` / `ContractSection`；D6 / D11 / D37 / D45 / D47 |
| `…/crates/core/archi/src/api/schema.rs` | `$defs` 五条规则 + `x-class` 带属性判错 + `schema.json` 六字段装配 + `INDEX.toml` 装配：`build_schema` / `verify_defs` / `build_index` | 公共函数表含本文件三行；类型表 `IndexEntry` / `ApiFormStatus`；D7 / D8 / D26 / D27 / D35 |
| `…/crates/core/archi/src/api/common.json`（**设计补入**） | 跨语言映射规则资产（首期唯一键 `Outcome`，每语言单槽位 + `{}` 占位符），`include_str!` 内嵌；**渲染器经 `common_type` 消费**（D54）；MUST NOT 长出「带生成产物的类型」 | 配置表一行；数据模型「持久化六」；D12 / D54 / AC-31 |
| `…/crates/core/archi/src/api/extract/mod.rs` | 取数器分派 + 归一：`extract` / `extract_element`（判据唯一取数面）/ `normalize_signature` / `schema_type` / `signature_of` / **`Position`（位置词汇唯一写出点，D43）** | 公共函数表含本文件五行（`extract` / `extract_element` / `normalize_signature` / `schema_type` / `signature_of`）；类型表 `ApiSymbol` / `ApiSymbolKind` / `ApiParam` / `ApiType` / `Visibility` / `Position`；D10 / D19 / D19-a / D19-b / D43 / D52 |
| `…/crates/core/archi/src/api/extract/rust.rs` | Rust 声明抽取（`syn`）：`extract_file`（`visibility` 三值）/ `extract_crate`（整包，沿入口走模块树 + crate 根 `pub use` 白名单）；`#[path]` 读盘面三步限根（D49） | 公共函数表含本文件两行（`extract_file` / `extract_crate`）；D19 / D44 / D49 |
| `…/crates/core/archi/src/api/extract/ts.rs` | TS 声明抽取（`oxc`）：`extract_file` + `apply_knip_report`（**本期未接线**，D39）；**取数面 = 导出面**；生产面只产 `External`；`crateOnly` / `testOnly` / `used_by` 留白（模块 doc 须写明缘由与将来所需输入） | 公共函数表含本文件两行（`extract_file` / `apply_knip_report`）；D19-a / D19-b / D39 |
| `…/crates/core/archi/src/api/check.rs` | 十判据单点求值：`check` + `category_of`（**`pub` 不动，但不经 `api` 出口面**，D60）；`apiMissing` 的 op 与类型同口径（D44）、`occurrences.line` 分列（D51） | 公共函数表含本文件两行（`check` / `category_of`）；D14 / D15 / D16 / D17 / D18 / D44 / D51 / D60 |
| `…/crates/core/archi/src/api/emit/mod.rs`（**设计补入**） | `pub mod rust; pub mod ts;` + `EmitTarget`（**三字段 `kind` / `form` / `modules`**，D58）+ `ModuleRef`（D40 的唯一承载形态）+ `render_target`（分派前发 `x-class` 警告，D27） | 公共函数表含本文件一行（`render_target`）；类型表 `EmitTarget` / `ModuleRef`；D13 / D27 / D40 / D41 / D46 / D58 |
| `…/crates/core/archi/src/api/emit/rust.rs` | `.i.rs` 渲染：类型 / 自动构造器 / 透传 `pub use` / 头注释 / 签名见证；**`common_type` 查表（D54）**；`rust_comment_text` / `variant_name`（D45 闸二）；`oneOf` / `anyOf` 分支负载经 `value_type`（**`choice_payload` 已删**，D59） | 公共函数表含本文件 **2 行**（`render` / `rust_comment_text`；`common_type` / `variants_block` / `variant_name` 是私有工序**不入表**——`variant_name` 盘上形态即私有 `fn`，行为经 `render` 的 enum 成员断言间接观测，其入表口径见该小节纪律①）；D11 / D13 / D23 / D29 / D45 / D46 / D54 / D59 |
| `…/crates/core/archi/src/api/emit/ts.rs` | `.i.ts` 渲染：`interface` / `as const` + `type` / 别名 / 透传 `export type { } from` / 头注释；**`common_type` 查表（D54）**；`ts_string_literal` / `ts_comment_text`（D45 闸二） | 公共函数表含本文件 **3 行**（`render` / `ts_string_literal` / `ts_comment_text`；`common_type` 是私有工序**不入表**，同左）；D5 / D13 / D29 / D45 / D46 / D54 |
| `packages/desktop/src-tauri/crates/tools/archi-api-gen/Cargo.toml` | 工具 crate 声明：裸名 `archi-api-gen`；workspace 内依赖恰 `archi` + `foundation`，外部 `serde_json`；注释写明零 Tauri / 零 store / 零 config / 零 orchestration / 零 workflow / 零壳 / 零子进程 / 不被 app 依赖 | 配置表 `Cargo.toml` 行；`desktop-crate-layout` |
| `…/crates/tools/archi-api-gen/src/main.rs` | CLI 唯一入口：`main` / `repository_root`（D32）/ `run`；目录创建经 `foundation::layout` 派生；**零路径字面量**；**`display` 改用 `archi::display`（删本地实现，D55）** | 公共函数表含本文件三行（`main` / `run` / **私有备案** `repository_root`——唯一入表的私有符号，理由见该小节纪律③）；D1 / D21 / D32 / D33 / D55 |
| `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md` | 试点契约（唯一手写真相）：H1 + `## 职能` / `## 不变量` 散文 + `### file_stem` + `api-op` fence + `api-types`（`LocalArchiCheck` 零字段 object）；**无 `### new`** | 数据模型「持久化一」；D6 / D23 |
| `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.schema.json` | 试点 `schema.json` 投影（生成，入库） | 数据模型「持久化一」；D7 |
| `.dev-team/architecture/api/INDEX.toml` | 全局唯一索引（生成，入库）：条目 + `degraded` 计数 + review 口径说明段 | 数据模型「持久化二」；D13 / D26 |
| `packages/desktop/src-tauri/crates/infra/archi.i.rs` | 试点生成物（落 `<root>` 同级 = `crates/infra/` 下）：头注释 + `pub struct LocalArchiCheck;` + `impl LocalArchiCheck { pub fn new() }` + 两行签名见证（`const _: fn(&str) -> String = super::LocalArchiCheck::file_stem;` / `super::LocalArchiCheck::new`） | 数据模型「持久化三」；D12 / D23 / D29 |

### 修改文件

| 文件路径 | 具体修改内容 | 说明 |
|----------|--------------|------|
| `crates/core/archi/src/lib.rs`（**设计补入**） | `pub mod api;`（字母序插在 `pub mod diff;` 之前）+ 出口 `pub use api::{api_ref, api_ref_of, bootstrap, check as api_check, enabled as api_enabled, load_contracts, meta, render_all, ApiCheckInput, ApiError, ApiMeta, ApiSnapshot, GeneratedFile};` + **`pub use api::display;` 单列一行**（判据串逐字在场，AC-32）+ `pub use drift::report::{ApiFindingSeverity, ArchiApiFinding, ArchiApiFindingKind};`；模块 doc 第五条纪律「契约面纯读：`api/**` 零写盘零 spawn，写盘只在工具 crate」 | 不声明模块则该族不可编译（事实校准 10）；**`display` 必须在出口内且成行**（D55：工具 crate 的唯一调用面；分组块写法会让 AC-32 的 `grep -cF "pub use api::display"` 恒 0）。`CommonRule` **不出口**（无 crate 外消费者，D55 明禁顺手扩大出口面） |
| `crates/core/archi/src/drift/report.rs` | 新增 `ArchiApiFindingKind`（十值 + `ALL` + `as_str` + `severity`）、`ApiFindingSeverity`（二值 + `ALL` + `as_str`）、`ArchiApiFinding`（六字段，`occurrences` 复用 `ArchiDriftOccurrence`）；`ArchiDriftReport` **末位**追加 `api_findings`（既有 14 字段逐字不动）；模块 doc 三组词汇扩为四组 | AC-8；`version` 常量在 `drift/mod.rs`（事实校准 1） |
| `crates/core/archi/src/drift/mod.rs` | `REPORT_VERSION` 1 → 2；`check()` 在观察项之后、`status` 推导之前插 `let api_findings = api::check(ApiCheckInput { … });` 并写入 `Report`；`skipped_report()` 的 `api_findings` 恒 `Vec::new()`；模块 doc 补「`api_findings` 不参与 `status` 推导」 | AC-9；判定序、`derive_status`、三个豁免层、`occurrences` 聚合零改动 |
| `crates/core/archi/src/dto.rs` | 新增 `ApiStatus`（`declared` / `none`，`#[serde(rename_all="lowercase")]` + `specta::Type` + `Default = Declared` + `as_str` + `ALL`）与 `ArchiApiRef`（四字段）；`ArchiElement` / `ArchiElementBrief` 增 `api: ApiStatus`；`ArchiModuleDetail` 增 `api: ArchiApiRef`；`to_element` / `to_brief` 填纯拷贝，`module_detail` 填 `api::api_ref(root, fqn)` | AC-10；投影零改写纪律不变 |
| `crates/core/archi/src/model.rs` | `Element` 增 `api: ApiStatus`（doc：契约状态指针，缺省 `declared`；MUST NOT 承载协议正文） | AC-10 |
| `crates/core/archi/src/parse/toml.rs` | `RawElement` 增 `#[serde(default)] api: ApiStatus`；`to_model` 拷贝该字段；`classify` 零改动 | AC-10；`deny_unknown_fields` 对拼错键继续生效 |
| `crates/core/archi/src/scan/owner.rs` | `owner_of` 入参准备处增私有函数 `strip_generated(rel_path: &str) -> String`：末段形如 `<stem>.i.<ext>`（`ext` 取 `GENERATED_EXTENSIONS` 五值，标记段取 `GENERATED_MARKER`）时替换为 `<stem>.<ext>`；`normalize` / `prefix_matches` / `build` 零改动 | AC-4；判据只此一处（D3） |
| `crates/core/foundation/src/layout/mod.rs` | 常量组增 `const API_DIR_NAME: &str = "api";`；`archi_config_path` 之后增 `pub fn archi_api_dir(root: &Path) -> PathBuf`；`archi_decisions_rel_dir` 之后增 `pub fn archi_api_rel_dir() -> String` | AC-19；全从 `archi_dir` / `archi_rel_dir` 派生 |
| `crates/infra/archi/src/lib.rs` | 入口编排：模块 doc 首段之后加 `#[path = "../../archi.i.rs"] mod api_gen; pub use api_gen::*;`；**删** `pub use runner::LocalArchiCheck;`（改由生成物提供）；模块 doc 上方三行注释改写为如实表述（**字面量 + 与 `mod_path_attr` 输出逐字相等**，D38），MUST NOT 再写「由该函数产出」 | AC-4 / AC-13；字面量经实测编译（事实校准 9） |
| `crates/infra/archi/src/runner.rs` | ① 删手写 `pub struct LocalArchiCheck;` 与 `impl LocalArchiCheck { pub fn new() }`（生成物接管；`impl Default` 保留并调 `Self::new()`）；② **`file_stem` 迁入**：把 `report.rs` 的私有自由函数 `fn file_stem(timestamp: &str) -> String` **挪进 `impl LocalArchiCheck` 并升为 `pub fn file_stem(timestamp: &str) -> String`**（实现体逐字照搬、零改写，D23）；③ `compose_text` 保持私有（不进契约） | ① 不删则 `pub use api_gen::*;` 报 E0255（事实校准 4）；② 试点 op 的代码形状处置（AC-13） |
| `crates/infra/agent/src/sdk/bash.rs` | 把 `fn resolve_timeout_ms` 提升为 `pub(crate)`（兄弟测试模块 `bash_test` 直测入参闸所需，doc 注明「不涉公共面」）；实现体与返回语义逐字不动（实盘 `git diff HEAD` 仅 **+2 / −1**，`bash_test.rs:182` 直测） | **归属测试相位产出白名单**（用例装置所需的最小可见性）；**MUST NOT 移出清单**——提升已在盘，移出即让工作树出现清单外改动（收尾自查 F ① 必红）。本行登记以免被误判为清单外改动 |
| `crates/infra/archi/src/report.rs`（**设计补入**） | 删私有 `fn file_stem(...)`（挪进 `runner.rs` 的 `impl LocalArchiCheck` 并提升为 `pub`，实现体逐字照搬）；`write_report` 内调用点改为 `LocalArchiCheck::file_stem(&report.timestamp)` | D23；`write_report` 仍 `pub(crate)`、不进契约 |
| `src-tauri/Cargo.toml` | `[workspace] members` 增 `"crates/tools/archi-api-gen"`；`[workspace.dependencies]` 增 `archi-api-gen = { path = "crates/tools/archi-api-gen" }`（注释「CLI 工具，不被 app 依赖」）；壳 `[dependencies]` **零新增** | AC-12 / AC-15 |
| `src-tauri/Cargo.lock`（**设计补列**） | `archi-api-gen` 的 `[[package]]` 条目 + 三条依赖（`archi` / `foundation` / `serde_json`）；必须提交 | 事实校准 17；依赖面恰两条 workspace 边的机器证据 |
| `packages/desktop/package.json` | `scripts` 增 `api:export` / `api:check`；`check` 由三段扩为四段（`server:check && bindings:export && api:check && client:check`——第二段是 `bindings:export`，事实校准 24）；**`server:check` 的 fmt 段由写模式改校验模式**（D28，clippy 段逐字不动）；`version` **取当前 `HEAD` 值 +1 patch**（MUST NOT 写死字面号，事实校准 25）；行尾取 `vp fmt` 不动点（LF） | AC-5 / AC-14；与 spec 字面的冲突有 owner（S-1 / S-2 / S-3 / S-6 delta + S-4 / S-5 主基线） |
| `packages/desktop/knip.json` | `ignore` 增 `"src/**/*.i.ts"`（与 `src/types/generated/**` 同款）；`ignoreExportsUsedInFile` / `ignoreDependencies` **零新增** | AC-6 / R-10 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 再生成：`ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` + `ArchiElement.api` / `ArchiElementBrief.api` / `ArchiModuleDetail.api` + 报告 `apiFindings`（`ApiFindingSeverity` 为零 serde / specta 的进程内枚举，按 D14 不出线） | AC-14；本行覆盖两文件，本清单零手写 |
| `.dev-team/architecture/models/05-backend-tools.toml` | 表头 `2 元素 / 1 关系` → `3 元素 / 3 关系`；`ExportBindings` 之后插 `App.Backend.Tools.ArchiApiGen`（`kind = "module"` / `gate = "off"` / `path` 逐字 / 一句中文 `summary`）；文件尾插 2 条 `[[relation]]`（→ `Core.Archi` / → `Core.Foundation`） | AC-15；既有 2 元素 1 边逐字不变 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 计数复述四处同步：`§1`（`50 / 111` → `51 / 113`，后端 `47` → `49`）、`§4`、`§6`、`§9` 复算表（`05(2)` → `05(3)`、`05(1)` → `05(3)`、元素 `50` → `51`、关系 `111` → `113`）；六条判据与其余节零改动 | AC-15 |
| `crates/infra/agent/src/archi_knowledge.rs` | 派生段增一行 `api 状态：<ApiStatus::ALL 逐值>`；`use` 行并入 `ApiStatus`；模块 doc 词表列举补 `api` 状态 | D25；MUST NOT 手抄词表 |
| `crates/infra/agent/src/archi-knowledge.md` | 「格式约定」补 `api`（`declared` 缺省 / `none`）与一句语义；「边界」段改写为「契约权威在架构域 api 目录的契约资产，模型只存状态指针 `api`，签名正文仍不进模型」 | D25；四面词汇对拍 |
| `AGENT.md` + `CLAUDE.md` | 两文件各在「Architecture」节之后新增**同一节** `## 模块 API 契约`：手写真相位置 + 三类 fence / 四类生成物勿手改 / `api:export`·`api:check` 两脚本与 `check` 链四段 / `api = none` 留白语义 / `apiFindings` 不进 `status` 且指向 `INDEX.toml`；**MUST NOT 复制契约正文或判据长表**；节内正文两文件**逐字相同** | AC-18（对象是两文件；`diff` 恰两处差异且全在节外） |

### 机械随动白名单（测试文件，**18 项**；与上表交集为空）

> 定位：**只为可编译而必须动的测试文件**，零逻辑改动、零新增用例。机械核对时 Rust 侧补回 `packages/desktop/src-tauri/` 前缀（与前端八项同口径）。归一规则**只有一份**（「核对口径」段的「路径归一口径」条）：本节表内的省略 / `…` 缩写写法只是书写形态，**MUST NOT** 被直接抄进 `/tmp/expected.txt` 或任何 `grep -cF` 判据串。

| 文件 | 机械随动内容 | 归属阶段 |
|---|---|---|
| `crates/core/archi/src/diff_test.rs` | `element(fqn)` 工厂补 `api: ApiStatus::Declared`（+ `use` 行） | 阶段二 |
| `crates/core/archi/src/docs_test.rs` | 同上 | 阶段二 |
| `crates/core/archi/src/graph_test.rs` | 同上 | 阶段二 |
| `crates/core/archi/src/model_test.rs` | 同上 | 阶段二 |
| `crates/core/archi/src/search_test.rs` | 同上（事实校准 13 的补入） | 阶段二 |
| `crates/core/archi/src/dto_test.rs` | `ArchiElement { … }` 字面量补 `api: ApiStatus::Declared` | 阶段二 |
| `crates/core/archi/src/scan/mod_test.rs` | `element(fqn, gate, paths)` 工厂补同字段 | 阶段二 |
| `crates/core/archi/src/scan/owner_test.rs` | 同上 | 阶段二 |
| `crates/core/archi/src/drift/report_test.rs` | `full_report()` / `empty_report()` 两处补 `api_findings: Vec::new()` | 阶段八 |
| `crates/infra/archi/src/report_test.rs` | `empty_report(timestamp)` 一处补同字段 | 阶段八 |
| `packages/desktop/src/views/architecture/architecture-view.test.tsx` | `element(fqn)` 工厂补 `api: 'declared'` | 阶段十五 |
| `…/components/architecture-canvas.test.tsx` | 补 `api: 'declared'` + `apiFindings: []` | 阶段十五 |
| `…/components/element-drawer.test.tsx` | 补 `api: 'declared'` | 阶段十五 |
| `…/components/element-tree.test.tsx` | 补 `api: 'declared'` | 阶段十五 |
| `…/hooks/use-archi-drift.test.ts` | 补 `apiFindings: []` | 阶段十五 |
| `…/hooks/use-archi-graph.test.ts` | 两处 `ArchiElement` 字面量补 `api: 'declared'` | 阶段十五 |
| `…/hooks/use-archi-module.test.ts` | `ArchiElement` 补 `api` + `ArchiModuleDetail` 补 `api` 四字段 | 阶段十五 |
| `…/lib/drift-marks.test.ts` | 五处 `ArchiElement` 补 `api: 'declared'` + `ArchiDriftReport` 补 `apiFindings: []` | 阶段十五 |

> 白名单四条纪律：① **18 项全部是测试文件**，MUST NOT 收留任何产品文件；② 每项只允许字段初始化 / `use` 行 / 调用点实参补充，出现逻辑或断言值改动即越界；③ 与「新增 / 修改」两表路径交集为空，故封闭数 **66 路径**（清单 48 + 白名单 18）可机械核对；④ **第四类机械随动（D53）不进本表**（其两落点已在测试相位 44 文件集内计数，收进即双计）。

### 测试相位产出白名单（**12 组 / 44 文件**；全为测试文件与测试夹具，与上两表交集为空）

> **为何单列**：这些是测试相位（test-design / test-gen / test-execution）新建与改写的用例、装置与语料，天然落在「变更清单」与「机械随动白名单」之外。**现行口径（唯一）**：实现相位封闭数 **60 路径**（清单 42 + 白名单 18）、本节 **44 文件**（表体 **12 组**逐组相加 = 44）⇒ **交付态并集 = 60 ∪ 44 = 104 路径**。**旧稿的「49 条」第三口径删除**（条目数不与文件数并列；组数与文件数已足以机械核对）。
>
> **本节纪律**：① 全部是测试文件 / 测试夹具 / 金样期望值，MUST NOT 收留产品文件；② 每项只允许「用例 / 装置 / 语料 / 期望值 / 断言值」形态改动——装置正确性修复与纯 fmt 清偿都在射程内，MUST NOT 借此改产品语义；③ 与机械随动白名单 **18** 项路径交集为空；④ **路径一律物理全路径**：本表既有的省略写法（`src/…`、`crates/…`）在进 `/tmp/test_phase_whitelist.txt` 前 MUST 按「核对口径」段的「路径归一口径」条归一——Rust 侧补 `packages/desktop/src-tauri/` 前缀、前端侧保持 `packages/desktop/` 起头、`…/` 缩写展开为同组首条全路径，与 `git status --porcelain -uall` 的路径列逐字可比，**47 行组成本节的唯一口径**。

| 组 | 项数 | 文件 |
|---|---|---|
| `api/**` 新增 sidecar 用例 | 11 | `crates/core/archi/src/api/{check,contract,mod,paths,schema}_test.rs`、`api/emit/{mod,rust,ts}_test.rs`、`api/extract/{mod,rust,ts}_test.rs` |
| `archi` 既有 sidecar 用例随动 | 2 | `crates/core/archi/src/drift/mod_test.rs`（`version = 2` / `api_findings` 槽位）、`crates/core/archi/src/parse/toml_test.rs`（`api` 字段） |
| `archi` crate 级集成用例 | 3 | `crates/core/archi/tests/{golden_test,vocabulary_test,pilot_test}.rs`（`pilot_test` 为试点三方对拍，AC-13 的落点） |
| 判定语料新增 fixture | 4 | `crates/core/archi/tests/corpus/drift/api-mismatch/{contracts/App/App.Mod.md,models/01-app.toml,repo/src/mod/mod.rs}`、`…/expected/api-mismatch.full.json` |
| 判定金样重写（`DESKTOP_GOLDEN_REWRITE=1` 显式重写；`version: 1` → `2` + 逐份追加 `apiFindings: []`） | 15 | `…/corpus/drift/expected/` 下 **12 份 `.full.json`** + **3 份 `.incremental.json`** |
| `foundation` sidecar 用例 | 1 | `crates/core/foundation/src/layout/mod_test.rs` |
| agent 面用例 | 2 | `crates/infra/agent/src/archi_knowledge_test.rs`、`crates/infra/agent/src/sdk/archi_tool_test.rs` |
| `archi-runtime` 用例 | 1 | `crates/infra/archi/src/runner_test.rs` |
| 工具 crate 用例 | 1 | `crates/tools/archi-api-gen/src/main_test.rs` |
| 壳命令面用例 | 2 | `packages/desktop/src-tauri/src/bindings/mod_test.rs`、`packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（**此三行一律写物理全路径**、零 `src/` 省略形态——它们要进 `/tmp/test_phase_whitelist.txt` 并与 `git status` 路径列做集合比对，省略形态在该集合里必然 0 命中；**装置超时的回滚按 D56 由本相位执行**——实现相位对该文件零触碰；回滚完成前，本组该文件是**接手红点**，其「30s 收敛上限」约定与既有退出判据恢复后其用例面仍归本节） |
| 前端用例 | 1 | `packages/desktop/src/lib/ipc.test.ts` |
| 写保护面用例随动（测试相位内） | 1 | `crates/infra/agent/src/sdk/bash_test.rs`（直测 `resolve_timeout_ms` 入参闸）；**`bash.rs` 归清单修改表**（产品文件，`pub(crate)` 可见性提升是其唯一改动），**MUST NOT** 同时列入本节（那会让并集少 1，双计是旧口径 113 的唯一来源） |
| **合计** | **44 文件**（表内 12 组） | — |

### spec 措辞同步项（S-1…S-17 delta 归本 change spec 相位；S-4 / S-5 归归档 SpecSync 步）

> **为何单列**：这些**不是代码交付物**，而是「规范文本与实现形态的冲突」。旧稿把这类冲突留在散文里（「必须放宽为…」），等于让它无 owner、无判据、可被静默遗忘。本稿把它提为可执行交付项：**逐条给替换后原文 + 校验命令 + owner 相位**。
>
> **射程判据（D31 的硬纪律）**：**每一处原文串必有且仅有一个 S 项承接**，少一处即该判据对它是盲的。**S 项条数 = 17**；**旧原文串数 = 14**；**新原文串数 = 18**——三数分列，MUST NOT 混用（条数与串数差的来源：S-7 与 S-10 各覆盖两处原文 ⇒ 旧串比条数多 1；新串比条数多 1 的来源是 S-7 有两条并列新串（`:329` 侧与 `:330` 侧））。射程分五批枚举、**五批之和 = 表内行数（17）**：甲类 S-1…S-6（门禁形态）、乙类 S-7…S-10（取数面边界）、丙类 S-11 / S-12（可执行形态）、丁类 S-13…S-15（单点与闸面收口）、戊类 S-16 / S-17（限根与形态闸）。**射程自检的 `grep` 片段 MUST 从本表机械抽取，MUST NOT 手抄** —— 手抄必然漏项（历史上漏过 `:395`，也漏过乙类全部四处）。
>
> **可执行性判据**：对每一项执行 `grep -F '<旧原文片段>' <文件>` ⇒ **零命中**（保留型项除外，见各行注），且 `grep -F '<新原文片段>' <文件>` ⇒ **恰命中**。
>
> **路径口径**：delta 项的文件列写 `specs/desktop-archi-api/spec.md` 或 `specs/desktop-archi-api-check/spec.md`（相对 change 目录）；**S-4 / S-5 写仓库根相对全路径**（主基线）。
>
> **相位口令**：delta 十五条归 **spec 相位**；**S-4 / S-5 属 `openspec/specs/**`（主基线），本 change 的任何相位 MUST NOT 直接改**，须由**归档 SpecSync 步**执行。主基线两项在 tasks 的勾选态一律是 `[x]`（勾选 = 「已完成登记 + 核对零改动」，不是「文本已替换」）。

| # | 文件 | 锚点（符号锚，非行号） | 原文（须消失） | 替换后原文（须在场） | owner 相位 |
|---|---|---|---|---|---|
| S-1 | `specs/desktop-archi-api/spec.md` | 「生成物入仓与零 diff 门挂载」Requirement 正文的三段命令分句 | 「既有三段命令形态零变化」 | 「`fmt` 段 SHALL 由写模式改为校验模式（`cargo fmt --all --check`）；其余段与段序不变」 | spec |
| S-2 | 同上 | 该 Requirement 下 Scenario 标题 | 「check 链扩一段而既有三段零变化」 | 「check 链扩一段且 fmt 段改校验模式」 | spec |
| S-3 | 同上 | 该 Scenario 的 THEN | 「三段命令原文逐字不变」 | 「两段命令原文逐字不变」 | spec |
| S-4 | `openspec/specs/desktop-ipc-type-bindings/spec.md` | CLI 表 `check` 行 | 三段链原文 | 四段链原文（第二段 `bindings:export`） | **归档 SpecSync** |
| S-5 | 同上 | Module Contract 表 `package.json` / `knip.json` 行 | 「`bindings:export` / `check` 命令形态不变」 | 「`knip.json` 的 `ignore` 增一条生成物 glob；`check` 为四段」 | **归档 SpecSync** |
| S-6 | `specs/desktop-archi-api/spec.md` | Module Contract 表 `package.json` 行末 | 「既有三段原文不变」 | 「两段原文不变（fmt 段改校验模式、clippy 段逐字不变）」 | spec |
| S-7 | 同上 | `visibility` 四值 Requirement 的 TS 分句（两处原文：档位枚举与「`knip` 复用」分句） | 「`knip` 报告未使用 → `unused`」与「`unused` 输入 SHALL 复用 `knip` 报告」 | 「四值词表封闭、本期可控值域按语言分列（Rust 三值可判 / TS 生产面只产 `external`）」+「本期 SHALL NOT 读 `knip` 报告」 | spec |
| S-8 | 同上 | Scenario `visibility 四值判定` 的 THEN 后半句 | 「四档判定」 | 「TS 侧本期只产 `external` 一档」+「`Unused` 本期不接线（显式留白）」 | spec |
| S-9 | 同上 | Module Contract 表 `api/extract/ts.rs` 行的 `export const` 分类分句 | 「箭头函数 → 操作」 | 「箭头函数不作操作」 | spec |
| S-10 | 同上 | Scenario `bootstrap 只出 stdout 不落盘` 的 THEN + Module Contract 表 `api/extract/mod.rs` 行（两处原文） | 「归一签名 / `visibility` / `used_by`」与「signature, visibility, used_by }」 | 「引用方（`used_by`，本期留白）」 | spec |
| S-11 | 同上 | 「生成物落点与入口查找」Requirement 的 `#[path]` 分句 | 「的值 SHALL 由单点函数按入口形态产出」 | 「`#[path]` 的值 SHALL 为**字面量**，且 MUST 与 `paths::mod_path_attr` 的输出逐字相等（等值真值源）」 | spec |
| S-12 | 同上 | Module Contract 表 `api/extract/ts.rs` 行末句 | 「`unused` 读 `knip` 报告」 | 「`unused` 的降级接口已实现但**本期未接线**」 | spec |
| S-13 | 同上 | 「关键字集、类型映射与错误面」的位置分派条（**保留型**：只补句，旧串仍在） | —（无旧串消失） | 「位置词汇与签名短形 MUST 各只有一份」 | spec |
| S-14 | `specs/desktop-archi-api-check/spec.md` | 判据表 `apiMissing` 行 + 「`apiMissing` 覆盖两类缺口」条 + Module Contract 表 `api/check.rs` 行（三处锚点） | Module Contract 表 `api/check.rs` 行旧串「声明集 vs 导出集」 | 「声明集（op **与 `$defs` 类型**）vs 导出集」+「同一条判据、同一次求值 + 三条排除面 + 两条结构前提」 | spec |
| S-15 | `specs/desktop-archi-api/spec.md` | 「四条明确不做」之前（**纯新增**） | — | 「契约文本进生成器 SHALL 经两道人字闸，且 `x-function` 的落点 SHALL 封闭二处」 | spec |
| S-16 | 同上 | 「生成物落点与入口查找」Requirement 末段（**纯新增**） | — | 「`#[path]` 的读盘面 SHALL 与写盘面同一条限根纪律」 | spec |
| S-17 | 同上 | Module Contract 表 `api/paths.rs` 行（**纯新增约束**） | — | 「末段进源码的形态闸按语言分列」+「`checked_root` 只判整条不判末段字符集」 | spec |

> **判据与串面的三条纪律**：① **保留型项只判新串在场**（S-13、S-14 的两处补句），纯新增项（S-15 / S-16 / S-17）无旧串可数；② **新原文串之间两两不得互为子串**，否则「恰命中 1」会假通过——已核对的定形串：`fmt 段 SHALL 由写模式`（S-1）/ `两段命令原文逐字不变`（S-3）/ `两段原文不变`（S-6）/ `本期留白` 的更长子串（S-7、S-10）/ `只产 \`external\` 一档`（S-8）/ `不作操作`（S-9）/ `字面量的等值真值源`（S-11）/ `本期未接线`（S-12）/ `各只有一份`（S-13）/ `同一条判据、同一次求值`（S-14）/ `两道人字闸`（S-15）/ `同一条限根纪律`（S-16）/ `形态闸按语言分列`（S-17）；**S-8 的判据串 MUST 用 `本期不接线`，MUST NOT 用 `本期未接线`**（后者是 S-12 的判据串）；③ **旧串判据 MUST 取定形长串**，MUST NOT 用裸词（裸 `knip` / 裸 `used_by` 在替换文本里仍在场，属自证型假判据）。改任一处的文案时 MUST 先重跑本表判据。

### 不要修改（本变更加固）

- `openspec/specs/**` 主基线（归档前）与 46 份 spec 的 `## Module Contract` 章节（双写过渡，R-2）。
- `.dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,04-backend-shell,06-frontend}.toml` 与全部 `gate` 档位（全 `off` 不动）。
- 存量 `path` 写法（全部保留 `./` 前缀 + 尾 `/`，归一在解析层）、`crates/core/archi/Cargo.toml` 与 `crates/infra/archi/Cargo.toml` 的依赖面（零新增条目）。
- `crates/infra/checks/**` 与 `crates/core/checks/src/model.rs`：检查域 runner 语义面零改动（本变更不新增 step、不新增 port、不新增 runtime 开关）。**唯一例外**：`crates/infra/checks/src/mutation/mod_test.rs` 一处纯 fmt 清偿（测试相位产出白名单成员，非本相位封闭集合）。
- **`packages/desktop/src/**` 的零改动口径是「除三类」**：① 生成物 `src/**/*.i.ts`（本期实际零份）；② 机械随动白名单列的 8 个 `src/views/architecture/**/*.test.ts(x)` 的字段补齐；③ `src/lib/ipc.test.ts`（测试相位产出）。前端视图 / hooks / 组件 / 路由的**产品代码**仍零改动。
- **`crates/infra/agent/src/sdk/archi_tool.rs` 零代码改动**：`module_payload` 只调 `archi::dto::{module_detail, module_overview}` 并序列化，`api` 字段随 DTO 自动出线——工具执行体内不得再拼一份 JSON。
- **`crates/infra/agent/src/sdk/**` 的零改动口径**：**六份 `context` / `loop` / `loop_test` / `resume` / `runner_test` / `tools_test` 完全零改动**（`HEAD` 无 rustfmt 债务，实测 6/6 干净），不在本变更任何集合内；**唯一例外**是 `bash.rs` 的 `resolve_timeout_ms` 提升为 `pub(crate)`（兄弟测试模块 `bash_test` 直测所需，属产品面但仅可见性一提，进清单修改表）；其余文件（`archi_tool.rs` / `bash_test.rs` / 工具面 / 注入矩阵 / policy）零语义改动。

### 公共函数 / API

> 口径（**范围与行数的唯一自洽处；文档内不写死行数，一律现算**）：射程 = 模块级导出函数 / 关联常量 / 公共类型的公开方法 / CLI 子命令。现算命令（cwd = 本 change 目录；**两串都锚到模块边界**，否则段内出现 `### ` 子标题会截断窗口）：
> ```bash
> sed -n '/^### 公共函数 \/ API$/,/^### 类型定义$/p' design.md | grep -c '^| `\|^| \*\*`'
> sed -n '/^### 公共函数 \/ API$/,/^### 类型定义$/p' design.md | grep -c '^| \*\*`'   # 其中「工具侧私有备案」行
> ```
> 三条纪律：① **`pub(crate)` 与私有工序一律不入表**——`parse_common`（D54 唯一解析点）/ `protocol_key`（D57 换算单点）/ `is_module_segment`（D48 / D50）/ `index_entries` / `categories_of` / `def_shape` / `module_refs` / `generated_path_of`，以及两侧渲染器的 `common_type`（D54 的唯一映射查表面，其判据面归 AC-31 ③ 的 `grep -c "fn common_type"`）与 `emit::rust::variant_name`（D45 闸二的承接地）全数落此，**与 `tasks.md` 阶段九「`pub fn` + 私有工序」的二分同一定义面，二者不互斥**；② **`api/**` 的 `pub fn` 全数在表**（含 `paths` 的 15 个路径派生件：它们虽是 crate 内部件，但仍是模块级导出）；③ **工具侧两行**：`api_gen::repository_root`（D32 的「门禁判的是哪个仓」承重点，**首部口径已声明的私有备案**）与 `api_gen::run`（三子命令的实现体，`pub(crate)`；列它是因为 `main` 只做 argv 解析与退出码映射，行为面全在此函数内，不列则 CLI 面无处可判）；其余工具侧私有工序（如 `write` 的限根复核）按私有工序处理。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `layout::archi_api_dir` | `crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn archi_api_dir(root: &Path) -> PathBuf` | `<archi_dir>/api`（段名取 `API_DIR_NAME`）；纯推导零 IO；契约面与生成器工具的唯一定位入口（AC-19） |
| `layout::archi_api_rel_dir` | 同上 | 新增 | `pub fn archi_api_rel_dir() -> String` | `<archi_rel_dir>/api`；`/` 分隔、零前导 `./`；消费者限于知识文本与 `INDEX.toml` 说明段 |
| `layout::API_DIR_NAME` | 同上 | 新增 | `const API_DIR_NAME: &str = "api"` | 段名字面量的**唯一**持有者（AC-19 判据 ③） |
| `ApiStatus::as_str` | `crates/core/archi/src/dto.rs` | 新增 | `pub fn as_str(self) -> &'static str` | `declared` / `none` 线格式词 |
| `ApiStatus::ALL` | 同上 | 新增 | `pub const ALL: [ApiStatus; 2]` | 全量常量（知识文本与语料对拍的唯一派生源） |
| `ArchiApiFindingKind::as_str` | `crates/core/archi/src/drift/report.rs` | 新增 | `pub fn as_str(self) -> &'static str` | 十值 camelCase 线格式词 |
| `ArchiApiFindingKind::severity` | 同上 | 新增 | `pub fn severity(self) -> ApiFindingSeverity` | 判据级档位**唯一来源**：九条 `Warn`、`apiUndeclared` 为 `Off` |
| `ArchiApiFindingKind::ALL` | 同上 | 新增 | `pub const ALL: [ArchiApiFindingKind; 10]` | 十值顺序与 spec 表一致 |
| `ApiFindingSeverity::as_str` / `ALL` | 同上 | 新增 | `pub fn as_str(self) -> &'static str` / `pub const ALL: [ApiFindingSeverity; 2]` | `off` / `warn`；与 `GateTier` 词面相同但**类型分立**（D14） |
| `api::meta` | `crates/core/archi/src/api/mod.rs` | 新增 | `pub fn meta() -> ApiMeta` | 生成载元单点：`tool_version` 取 `CARGO_PKG_VERSION`、`common_digest` 取资产 `sha256` 摘要、`common` 取 `common.json` 的解析结果。**口径并入 D54②**：`meta()` 零失败（资产经 `include_str!` 恒在场，本函数只做装载、不承担槽位合法性判定）；合法性由 `load_contracts` 以同一 `parse_common` 复核，失败即映射为 `ApiError::Contract`（静默跳过不允许）。**MUST NOT 出现「解析失败即 panic」的读法**——签名无 `Result`，其零失败来自「资产恒在场」而非「错误被吞」；`schema` / `emit` / `INDEX` 三处共用同一份载元（D12） |
| `api::enabled` | 同上 | 新增 | `pub fn enabled(root: &Path) -> bool` | `<archi_api_dir>` 存在且递归含 ≥1 份 `*.md`——契约面冷启动的唯一判据（D17） |
| `api::load_contracts` | 同上 | 新增 | `pub fn load_contracts(root: &Path, elements: &[ArchiElement]) -> Result<ApiSnapshot, ApiError>` | 逐 `path` 非空元素定位 md → 解析 → 双向 gate → 装配 `schema`（内存）+ 取数器符号集；缺 md 不是错误（跳过该元素，缺口由判据报——装载器不另记清单，D36）；**顺带校验 `common.json` 形态**（D54，失败即 `ApiError::Contract`） |
| `api::render_all` | 同上 | 新增 | `pub fn render_all(root: &Path, snapshot: &ApiSnapshot) -> Result<Vec<GeneratedFile>, ApiError>` | 装配四类生成物为内存 `Vec<GeneratedFile>`；**零写盘**；`EmitTarget` 的 `kind` / `form` / `modules` 三值在此装配（D40 / D41 / **D58**——落点 `path` 与元素根 `scope` 留在本函数局部、直接进 `GeneratedFile`，不进 `EmitTarget`） |
| `api::check` | 同上 | 新增 | `pub fn check(input: ApiCheckInput<'_>) -> Vec<ArchiApiFinding>` | 十判据对外唯一入口（`drift::check` 的单点调用面）；未启用返回空 `Vec`；返回前按 `(kind, element, target)` 稳定排序 |
| `api::bootstrap` | 同上 | 新增 | `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>` | 取数器草稿：候选符号 + `visibility`（+ 位置），输出 md 骨架文本。**`used_by` 本期不产出**（D19-b / S-10）；**不读 knip**（D39） |
| `api::api_ref` | 同上 | 新增 | `pub fn api_ref(root: &Path, fqn: &str) -> ArchiApiRef` | 契约状态投影：md 存在即 `declared` 且三路径为 repo 相对串；否则 `none` 且三路径 `None`（D22） |
| `api::api_ref_of` | 同上 | 新增 | `pub fn api_ref_of(root: &Path, fqn: &str, element_path: &[String]) -> ArchiApiRef` | 同一投影的**元素形态入口**：消费方手上已有元素 `path` 时用（判定与 `api_ref` 同源） |
| **`api::display`** | 同上 | **修改（`pub(crate)` → `pub`）** | `pub fn display(root: &Path, path: &Path) -> String` | 绝对路径 → repo 相对 POSIX 串（剥 root 前缀、`\` → `/`、剥前导 `./`）。**D55：跨 crate 唯一单点**，经 `lib.rs` 出口供 `archi-api-gen` 复用；工具侧本地逐字实现已删 |
| `paths::normalize_root` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn normalize_root(raw: &str) -> String` | `\` → `/`、剥前导 `./`、去尾 `/`（**不**去扩展名，D2） |
| `paths::contract_path` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn contract_path(root: &Path, fqn: &str) -> Option<PathBuf>` | `<archi_api_dir>/<Domain>/<FQN>.md`；FQN 形态非法 → `None`（D33） |
| `paths::schema_path` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn schema_path(root: &Path, fqn: &str) -> Option<PathBuf>` | 同目录的 `<FQN>.schema.json`；形态判据同左 |
| `paths::generated_path` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn generated_path(root: &Path, element_root: &str, kind: GeneratedKind) -> Option<PathBuf>` | `<root>.i.rs` / `<root>.i.ts`（入参是已归一 `<root>` 字符串，D42）；形态非法 → `None` |
| `paths::generated_kind` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn generated_kind(language: Language) -> GeneratedKind` | 语言 → 生成物种类**分派单点**：`TypeScript` → `TypeScript`；`Rust` / `Unknown` → `Rust`。调用方 MUST NOT 各自 `match` |
| `paths::entry_form` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn entry_form(root: &Path, element: &ArchiElement) -> Option<(EntryForm, String)>` | 按三分支 / 四分支顺序查实现入口，返回形态与入口 repo 相对路径；`path` 空 / `<root>` 非法 / 全不命中 → `None`（即 `apiEntryMissing`）。**形态的唯一判据**（D41） |
| `paths::mod_path_attr` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn mod_path_attr(form: EntryForm, name: &str) -> String` | `#[path]` 值的**等值真值源**（D38）：`"../../<name>.i.rs"` / `"../<name>.i.rs"` / `"<name>.i.rs"`；产品侧零调用者，消费面是 `paths_test` 三形态断言与 tasks 逐入口自查行 |
| `paths::ts_import_specifier` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn ts_import_specifier(form: EntryForm, name: &str) -> String` | `"../<name>.i"`（目录形态）/ `"./<name>.i"`（单文件形态） |
| `paths::ref_relative` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn ref_relative(from_fqn: &str, to_fqn: &str) -> String` | 跨文件 `$ref` 前缀：同 `<Domain>` → `"./"`，跨目录 → `"../<Domain>/"` |
| `paths::domain_of` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn domain_of(fqn: &str) -> Option<String>` | 父 FQN 末段（顶层元素 `None`） |
| `paths::directory` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn directory(root: &Path) -> PathBuf` | `<archi_api_dir>`（`archi_api_dir(root)` 的唯一转发点，即「MUST NOT 行内拼 `api` 段」的落点） |
| `paths::index_path` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn index_path(root: &Path) -> PathBuf` | `<archi_api_dir>/INDEX.toml`（全局唯一一份；文件名常量单点持有） |
| `paths::module_name` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn module_name(element_root: &str) -> String` | `<root>` 末段（透传 `<name>` 的唯一产出点，D40）；纯派生零形态判据（D48） |
| `paths::language_of` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn language_of(file: &str) -> Language` | 按扩展名判语言（`rs` → `Rust`；`ts` / `tsx` / `js` / `jsx` → `TypeScript`；其余 `Unknown`） |
| `paths::rust_module_path` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn rust_module_path(element_root: &str) -> String` | Rust 透传模块路径的单点（D40）：`<root>` 末段 → `crate::<末段>`。MUST NOT 用目标 FQN（点分段不是 Rust 路径） |
| `contract::parse` | `crates/core/archi/src/api/contract.rs` | 新增 | `pub fn parse(source: &str, fqn: &str, file: &str) -> Result<Contract, ApiError>` | H1 锚点 + 散文区 + `### <op-id>` 分段 + 三类 fence；**fence 内容在 `serde_json` 解析前先做对象层级重复键扫描**（D37）；错误带 `file:line` |
| `contract::verify` | 同上 | 新增 | `pub fn verify(contract: &Contract, schema: &serde_json::Value) -> Result<(), ApiError>` | 双向 gate 三条 + **七族名字闸**（D45 闸一）+ **`x-function` 落点闸**（D47），差异点名（含族别与 JSON pointer） |
| `contract::is_constructor` | 同上 | 新增 | `pub fn is_constructor(op_id: &str) -> bool` | 构造器名封闭集判据（`new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open`） |
| `schema::build_schema` | `crates/core/archi/src/api/schema.rs` | 新增 | `pub fn build_schema(contract: &Contract, meta: &ApiMeta) -> Result<serde_json::Value, ApiError>` | 六字段装配；`$defs` 键序 = `api-types` 声明序 + `api-reexport` 声明序（幂等）；两处键空间唯一性先判后写（D35） |
| `schema::verify_defs` | 同上 | 新增 | `pub fn verify_defs(schema: &serde_json::Value) -> Result<(), ApiError>` | 五条规则 + 透传条目纯 `$ref` 校验 + **`x-class` 带属性判错**（D27）；MUST NOT 承担重复键判据（D37） |
| `schema::build_index` | 同上 | 新增 | `pub fn build_index(entries: &[IndexEntry], meta: &ApiMeta) -> String` | `INDEX.toml` 文本：条目按 FQN 字典序 + 说明段（review 口径 / 生成器标识）+ `[entry.categories]` 形状分布（D26）；零语言词 |
| `extract::extract` | `crates/core/archi/src/api/extract/mod.rs` | 新增 | `pub fn extract(element: &ArchiElement, file: &str, source: &str) -> Vec<ApiSymbol>` | 按元素语言分派；未知语言 → 空 `Vec`（不猜测）；`path` 空 → `<root>` 取空串，**MUST NOT 索引 `path[0]`**。**产品调用者为零是显式留白**（D52） |
| `extract::extract_element` | 同上 | 新增 | `pub fn extract_element(root: &Path, entry: &str) -> Vec<ApiSymbol>` | **判据的「代码导出集」唯一取数面**：Rust 走 `rust::extract_crate`，TS 读入口后 `ts::extract_file`；入口读不到 → 空 `Vec` |
| `extract::normalize_signature` | 同上 | 新增 | `pub fn normalize_signature(symbol: &ApiSymbol) -> String` | 归一单行签名（`fn(<类型序列>) -> <返回类型>`）；判据比对与生成器投影共用，**唯一**消费点在 `check.rs` |
| `extract::schema_type` | 同上 | 新增 | `pub fn schema_type(schema: &serde_json::Value, position: Position) -> ApiType` | 契约 JSON 类型面 → 逻辑类型（**协议键固定 `Outcome`** → `ApiType::Outcome`；`Value::get` 的取值键仍写协议名原样）；**位置词汇 `Position` 的唯一写出点之一**（D43） |
| `extract::signature_of` | 同上 | 新增 | `pub fn signature_of(op: &serde_json::Value) -> (Vec<ApiType>, ApiType)` | `api-op` fence → （入参类型序列, 返回类型）；判据 ④ 的唯一短形来源（D43） |
| `extract::rust::extract_file` | `crates/core/archi/src/api/extract/rust.rs` | 新增 | `pub fn extract_file(file: &str, source: &str, crate_name: &str) -> Vec<ApiSymbol>` | `syn` 遍历；`visibility` 三值判定（`External` / `CrateOnly` / `TestOnly`） |
| `extract::rust::extract_crate` | 同上 | 新增 | `pub fn extract_crate(root: &Path, entry: &str) -> Vec<ApiSymbol>` | 整包抽取：沿模块树（含 `#[path]` 三步限根，D49）并用 crate 根 `pub use` 白名单升格；模块定位失败 → 跳过该子树（零伪造） |
| `extract::ts::extract_file` | `crates/core/archi/src/api/extract/ts.rs` | 新增 | `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol>` | `oxc` 遍历**只抽导出面**（D19-a）：仅 `Statement::ExportDeclaration` 一条分支；箭头函数不作操作；六个非导出分支与 `export default` 均不产出符号 |
| `extract::ts::apply_knip_report` | 同上 | 新增（**本期未接线**） | `pub fn apply_knip_report(symbols: &mut [ApiSymbol], report: Option<&serde_json::Value>)` | 把 knip 报的未使用导出降级为 `Visibility::Unused`（`None` 即不降级）。**D39：产品路径零调用者**，`ts_test` 五条用例继续测接口正确性 |
| `check::check` | `crates/core/archi/src/api/check.rs` | 新增 | `pub fn check(input: ApiCheckInput<'_>) -> Vec<ArchiApiFinding>` | 十判据单点求值（序与互斥见 D16）；`$defs` 只读 `schema.json`；op 与类型同口径（D44）；`occurrences.line` 分列（D51） |
| `check::category_of` | 同上 | 新增 | `pub fn category_of(kind: ArchiApiFindingKind) -> &'static str` | 开放分类的单点打标（六值，D15）。**不经 `api` 出口面**：消费方是 `check.rs` 自身与 `check_test.rs` 的 `use super::check::{category_of, check}` 模块路径（D60） |
| `emit::render_target` | `crates/core/archi/src/api/emit/mod.rs` | 新增 | `pub fn render_target(target: &EmitTarget, contract: &Contract, schema: &serde_json::Value, meta: &ApiMeta) -> Result<String, ApiError>` | 按语言分派到 `emit::{rust, ts}`（单契约粒度；批量入口在 `api::render_all`）；分派前发 `x-class` 警告（D27，每元素恰一次）；产出文本（零写盘） |
| `emit::rust::render` | `crates/core/archi/src/api/emit/rust.rs` | 新增 | `pub fn render(contract: &Contract, schema: &serde_json::Value, meta: &ApiMeta, target: &EmitTarget) -> String` | `.i.rs` 文本：类型 / 自动构造器 / 透传 `pub use` / 头注释 / 签名见证。**四参单形**（D46，`target` 末位）；映射经 `common_type` 查表（D54）；`oneOf` / `anyOf` 分支负载一律经 `value_type`（D59） |
| `emit::ts::render` | `crates/core/archi/src/api/emit/ts.rs` | 新增 | `pub fn render(contract: &Contract, schema: &serde_json::Value, meta: &ApiMeta, target: &EmitTarget) -> String` | `.i.ts` 文本：`interface` / `as const` + `type` / 别名 / 透传 `export type { } from` / 头注释。四参单形（D46）；映射经 `common_type` 查表（D54） |
| `emit::ts::ts_string_literal` | 同上 | 新增 | `pub fn ts_string_literal(text: &str) -> String` | 闸二单点（D45）：单引号串 + 转义（形态不变，MUST NOT 产双引号） |
| `emit::ts::ts_comment_text` | 同上 | 新增 | `pub fn ts_comment_text(text: &str) -> String` | 闸二单点：注释断行防护（`*/` / 换行） |
| `emit::rust::rust_comment_text` | `crates/core/archi/src/api/emit/rust.rs` | 新增 | `pub fn rust_comment_text(text: &str) -> String` | 闸二单点：Rust 注释断行防护 |
| `main`（CLI 入口） | `crates/tools/archi-api-gen/src/main.rs` | 新增 | `fn main() -> ExitCode` | argv → `Command`；root 经 `repository_root()` 锚定（D32）；`Usage` 打用法并退出 2；错误 `eprintln` + 退出 1（退出码取返回值，零 `process::exit`） |
| `api_gen::repository_root` | 同上 | 新增（`fn`，私有） | `fn repository_root() -> PathBuf` | **观察根锚定（D32）**：从 `current_dir()` 逐级 `parent()`，首个使 `archi_dir(candidate).is_dir()` 为真者即 root；到根仍不命中 ⇒ 退化为 cwd。**零路径字面量**（可机械核对） |
| `api_gen::run` | 同上 | 新增（`pub(crate)`） | `pub(crate) fn run(command: Command, root: &Path) -> Result<i32, String>` | 三子命令实现体（离 `main` 便于 tempdir 驱动）；目录创建经 `foundation::layout` 派生；`--check` 零写入；写盘前 `starts_with(root)` 复核（D33） |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `ApiStatus` | `crates/core/archi/src/dto.rs` | 新增 | 封闭二值 `Declared`（缺省）/ `None`；`#[serde(rename_all = "lowercase")]` + `specta::Type` + `Default` + `as_str` + `ALL` |
| `ArchiApiRef` | 同上 | 新增 | `{ status: ApiStatus, contract_path: Option<String>, schema_path: Option<String>, generated_path: Option<String> }`（camelCase；`none` 时三者 `null`；路径为 repo 相对 POSIX） |
| `ApiFindingSeverity` | `crates/core/archi/src/drift/report.rs` | 新增 | 封闭二值 `Off` / `Warn`（进程内，零 serde / specta）+ `as_str` + `ALL`；语义 = **判据级档位**（D14） |
| `ArchiApiFindingKind` | 同上 | 新增 | 封闭十值 `ApiEntryMissing` / `ApiMissing` / `ApiUndeclared` / `ApiSignatureMismatch` / `ApiUnexpected` / `ApiShadowing` / `ApiRefMissing` / `ApiNestedCrossCrate` / `ApiReexportDrift` / `ApiConstructorDeclared`；camelCase + `ALL: [Self; 10]` + `as_str` + `severity` |
| `ArchiApiFinding` | 同上 | 新增 | `{ kind, category: String, element: Option<String>, target: Option<String>, message: String, occurrences: Vec<ArchiDriftOccurrence> }` |
| `ApiError` | `crates/core/archi/src/api/mod.rs` | 新增 | 封闭三值 `Io(String)` / `Contract(String)` / `Element(String)` + `pub fn message(&self) -> String`（D20）；`Element` 兼承「元素表不可得」（模型结构性错 ⇒ `elements_of` 的 `Err`），**MUST NOT** 为此新增第四变体 |
| `ApiSnapshot` | 同上 | 新增 | `{ contracts: Vec<Contract>, schemas: BTreeMap<String, Value>, symbols: BTreeMap<String, Vec<ApiSymbol>> }`（**无缺契约清单**，D36） |
| `GeneratedFile` | 同上 | 新增 | `{ path: PathBuf, content: String }`（写盘归工具 crate） |
| `ApiCheckInput<'a>` | 同上 | 新增 | `{ root: &'a Path, elements: &'a [ArchiElement], contracts: &'a [Contract], schemas: &'a BTreeMap<String, Value>, symbols: &'a BTreeMap<String, Vec<ApiSymbol>>, enabled: bool }`（**六字段，零 `absent`**，D36） |
| `ApiMeta` | 同上 | 新增 | `{ tool_version: &'static str, common_digest: String, schema_uri: &'static str, generated_notice: &'static str, common: BTreeMap<String, CommonRule> }`（**`common` 为 D54 新增**：映射规则表的解析结果，渲染器只查表零读盘） |
| **`CommonRule`** | 同上 | 新增 | `{ rust: String, typescript: String, degraded: Option<String>, note: Option<String> }`（`common.json` 的**单键**规则；两语言槽位 MUST 都在场且各含至少一个 `{}` 占位符，否则 `ApiError::Contract`） |
| `Contract` | `crates/core/archi/src/api/contract.rs` | 新增 | `{ fqn: String, title: String, file: String, ops: Vec<ContractOp>, types: serde_json::Map<String, Value>, reexports: serde_json::Map<String, Value>, reexport_prose: Vec<String>, sections: Vec<ContractSection> }` |
| `ContractOp` | 同上 | 新增 | `{ id: String, line: u32, signature: serde_json::Value }`（`line` 供 D51 的 op 缺口行号） |
| `ContractSection` | 同上 | 新增 | `{ heading: String, line: u32, body: String }`（`##` 散文区） |
| `IndexEntry` | `crates/core/archi/src/api/schema.rs` | 新增 | `{ fqn: String, status: ApiFormStatus, types: usize, ops: usize, degraded: usize, categories: BTreeMap<String, usize>, schema_rel: String, generated_rel: Option<String> }`（`categories` 语义见 D26） |
| `ApiFormStatus` | 同上 | 新增 | 封闭二值 `Declared` / `Absent`（INDEX 用；与 `ApiStatus` 分立——后者是模型意图，前者是文件事实） |
| `ApiSymbol` | `crates/core/archi/src/api/extract/mod.rs` | 新增 | `{ name: String, owner: Option<String>, kind: ApiSymbolKind, params: Vec<ApiParam>, returns: ApiType, is_async: bool, line: u32, visibility: Visibility }` |
| `ApiSymbolKind` | 同上 | 新增 | 封闭四值 `Type` / `Trait` / `Enum` / `Function`（构造器不单独成值：用 `is_constructor(name)` 判） |
| `ApiParam` | 同上 | 新增 | `{ name: String, ty: ApiType, optional: bool }` |
| `ApiType` | 同上 | 新增 | 封闭枚举：`Str` / `OwnedString` / `Int` / `Float` / `Bool` / `List(Box<ApiType>)` / `Map(Box<ApiType>)` / `Optional(Box<ApiType>)` / `Named(String)` / `Outcome(Box<ApiType>)` / `Unknown(String)` |
| `Visibility` | 同上 | 新增 | 封闭四值 `External` / `CrateOnly` / `Unused` / `TestOnly` + `as_str` + `ALL: [Self; 4]`。**词表封闭不动，可判值域按语言分列**（D19-b / D39）：Rust = 三值可产（`Unused` 由调用方降级）；TS = 只产 `External`（`Unused` 的降级接口零调用者、`CrateOnly` / `TestOnly` 留白）。枚举 doc MUST 写明留白原因与将来所需输入 |
| **`Position`** | 同上 | 新增 | 封闭三值 `Param` / `Return` / `Field`。**位置词汇的唯一写出点**（D43）：`check.rs` 与两侧渲染器一律 `use` 本类型，MUST NOT 自建第二份 |
| `EntryForm` | `crates/core/archi/src/api/paths.rs` | 新增 | 封闭三值 `Crate` / `SubdirModule` / `SingleFile`（前端四分支只取后两值） |
| `GeneratedKind` | 同上 | 新增 | 封闭二值 `Rust` / `TypeScript` |
| `Language` | 同上 | 新增 | 封闭三值 `Rust` / `TypeScript` / `Unknown`（按扩展名判定，供取数器分派） |
| `ModuleRef` | `crates/core/archi/src/api/emit/mod.rs` | 新增 | `{ name: String, module_path: String }`（D40 的唯一承载形态：`name` 由 `paths::module_name` 产出、`module_path` 由 `paths::rust_module_path` 产出；两侧渲染器各读一个字段、互不派生） |
| `EmitTarget` | 同上 | 新增 | **三字段** `{ kind: GeneratedKind, form: EntryForm, modules: BTreeMap<String, ModuleRef> }`（`form` 由调用方经 `paths::entry_form` 求出，D41；`modules` 是查表结果，D40；**`path` / `element_root` 已删**——产品面零读取者，D58 / 事实校准 34）。**specifier 只由 `form` 与 `ModuleRef.name` 决定**，无第三条输入 |
| `Element.api` | `crates/core/archi/src/model.rs` | 修改 | 追加 `pub api: ApiStatus`（doc：契约状态指针，缺省 `declared`；MUST NOT 承载协议正文） |
| `RawElement.api` | `crates/core/archi/src/parse/toml.rs` | 修改 | 追加 `#[serde(default)] api: ApiStatus` |
| `ArchiElement.api` | `crates/core/archi/src/dto.rs` | 修改 | 追加 `pub api: ApiStatus`——本变更新增的那一个：`ArchiElement` 改前 9 字段、改后 **10** 字段，`api` 落第 10 位（doc：模型字段的拷贝，协议正文不进模型） |
| `ArchiElementBrief.api` | 同上 | 修改 | 追加 `pub api: ApiStatus`（该结构改前 4 字段、改后 **5** 字段） |
| `ArchiModuleDetail.api` | 同上 | 修改 | 追加 `pub api: ArchiApiRef`（该结构改前 6 字段、改后 **7** 字段；唯一用**文件事实**投影而非模型意图的字段，D22） |
| `ArchiDriftReport.api_findings` | `crates/core/archi/src/drift/report.rs` | 修改 | 末位追加 `pub api_findings: Vec<ArchiApiFinding>`（既有 14 字段零改动） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `scripts.api:export` | `packages/desktop/package.json` | 新增 | string | — | `cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`（单命令）。**脚本不带 `cd`**：root 由 D32 从 cwd 向上锚定 |
| `scripts.api:check` | 同上 | 新增 | string | — | 同一调用加 `--check`（零写入；差异即非零退出并列文件；root 同由 D32 锚定） |
| `scripts.check` | 同上 | 修改 | string | — | 三段 → 四段：**`server:check && bindings:export && api:check && client:check`**（事实校准 24） |
| `scripts.server:check` | 同上 | 修改 | string | — | **fmt 段由写模式改校验模式**：`cargo fmt --all --check --manifest-path src-tauri/Cargo.toml && cargo clippy --workspace --all-targets --manifest-path src-tauri/Cargo.toml`（clippy 段逐字不动；D28） |
| `version` | 同上 | 修改 | string | **当前 `HEAD` 值 +1 patch** | patch +1，**MUST NOT 写死字面号**（事实校准 25）；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随、`src-tauri/Cargo.toml` 不随动 |
| `ignore` | `packages/desktop/knip.json` | 修改 | string[] | 既有两项 + `src/**/*.i.ts` | 追加生成物 glob；`ignoreExportsUsedInFile` / `ignoreDependencies` 零新增 |
| `[workspace] members` | `packages/desktop/src-tauri/Cargo.toml` | 修改 | string[] | — | 追加 `"crates/tools/archi-api-gen"`（壳 `[dependencies]` 零新增） |
| `[workspace.dependencies] archi-api-gen` | 同上 | 修改 | table | — | `{ path = "crates/tools/archi-api-gen" }` + 注释「CLI 工具，不被 app 依赖」 |
| **`api/common.json`（映射规则资产）** | `crates/core/archi/src/api/common.json` | 新增 | JSON object | — | 键 = **协议名字**（与 `ApiType::Outcome` 变体名、`#/$defs/Outcome`、spec `:149` 同形），首期唯一键 **`Outcome`**；契约内的**承载键**是 `"outcome"`，两者经 `api::protocol_key` 换算（D57）；值 = `{ "rust": <槽位>, "typescript": <槽位>, "degraded"?: <语言名>, "note"?: <注记> }`，槽位内**恰含 ≥1 个 `{}` 占位符**（`"Result<{}, String>"` / `"{ ok: {} } \| { err: string }"`）。`include_str!` 编译期内嵌；**渲染器 MUST 经 `common_type` 消费**（D54）；缺语言槽位 / 零 `{}` / JSON 非法 ⇒ `ApiError::Contract`。MUST NOT 长出「带生成产物的类型」（D12 的边界约束） |

---

## 数据模型 (Data Model)

### 持久化一：手写契约 `<archi_api_dir>/<Domain>/<FQN>.md` + 生成物 `<FQN>.schema.json`

- **唯一手写面**：每个 `api = declared` 元素一份 `<FQN>.md`（`<Domain>` = 父元素 FQN 末段，`App.Backend.Infra.ArchiRuntime` → `Infra`；顶层元素退化 `Root`；元素身份仍是 FQN，目录只是布局）。
- **生成物同目录**：`<FQN>.schema.json`（入库、进零 diff 门、头部 `$comment` 警示、MUST NOT 手改）。
- **章节位置语义（封闭五类）**：H1 = `<FQN>`；`## <中文二标题>` = 散文；`### <op-id>` + 紧随的 `api-op` fenced json；`api-types` fenced json（本模块自有 `$defs`）；`api-reexport` fenced json（键 = 导出名，值 = 直接子元素 FQN）。
- **双向 gate 三条**（机械）：heading 集合 ≡ `api-op` fence 归属集合；`api-types` 键集 ⊆ 生成 `$defs` 键集；`api-reexport` 键集 ≡ 散文透传列表。
- **`schema.json` 六字段**（逐项有 md 出处）：`$schema`（draft 2020-12 常量）/ `$id`（文件名，不合成 URI）/ `title`（H1）/ `$defs`（本地 + 透传纯 `$ref`）/ `properties`（op 键 + 类型 `$ref` 项）/ `$comment`（警示）。
- **消费前置**：读 `schema.json` 的工具 MUST 用 **draft 2020-12** 校验器 + `strict: "log"`（`x-*` 是未知关键字），且 MUST 先按 `$id` 逐个注册被引用 schema（跨文件 `$ref` 是结构性的）。
- **字面量纪律**：路径全经 `foundation::layout::archi_api_dir` 派生；产品 `.rs` MUST NOT 行内拼 `api` 段。

### 持久化二：`<archi_api_dir>/INDEX.toml`（全局唯一一份，生成）

```toml
# 由 archi-api-gen 生成 · 勿手改
# review 口径：只读 <FQN>.md；本文件与 *.schema.json 的 diff 视为机械噪声。
# 契约面：启用（<archi_api_dir> 内 N 份契约）

[[entry]]
fqn = "App.Backend.Infra.ArchiRuntime"
status = "declared"          # declared（文件在案）/ absent（declared 但文件缺失）
types = 1
ops = 1
degraded = 0
schema = ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.schema.json"
generated = "packages/desktop/src-tauri/crates/infra/archi.i.rs"

[entry.categories]           # `$defs` 形状分布（开放键集；absent 元素整段省略）
object = 1
```

- **条目序**：FQN 字典序（稳定序，便于断言与 diff）。
- **`status` 词汇**：封闭二值 `declared` / `absent`（文件事实，与模型意图 `ApiStatus` 分立）。
- **`degraded` 计数**：来自各契约 `$defs` 条目的 `x-common.degraded` 条目数（当前两类原因：错误通道走 `Promise` reject、TS 结构化类型无法强制实现）。
- **`categories` 分布**（D26）：键空间当前六值（开放字符串集）：`object` / `enum` / `variant` / `class` / `scalar` / `reexport`；`absent` 元素无 `$defs` ⇒ 表为空且整段省略（MUST NOT 输出空表）。
- **`types` 与 `categories` 同源**：`sum(categories) == types`。
- **说明段必含**：review 口径、生成器标识（`archi-api <CARGO_PKG_VERSION>` + `common.json@sha256:<前 12 位>`）；**零 `crate` / `Cargo.toml` / `Rust` / `TypeScript` 语言词**（AC-17）。
- **路径字段形态**：`schema` / `generated` 为 repo 相对全路径（`display()` 剥根前缀后原样落盘），故带 `packages/desktop/src-tauri/` 前缀——索引值 MUST 是可直接 `git diff --stat` 消费的真实路径。
- **`categories` MUST NOT 取判据分类**（`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor`）：那是 `ArchiApiFinding.category` 的值域（D15 / D26）。

### 持久化三：元素根同级的生成物 `<root>.i.rs` / `<root>.i.ts`

- **落点**：`<root>` = `normalize_root(path[0])`（剥 `./`、去尾 `/`、`\`→`/`，**不去扩展名**）；生成物 = `<root>.i.rs` / `<root>.i.ts`。
- **入口编排**（手写，一处）：入口文件加 `#[path = "<字面量>"] mod api_gen; pub use api_gen::*;`；该字面量 MUST 与 `paths::mod_path_attr(<实际入口形态>, <module_name>)` 的输出逐字相等（D38）；既有 `pub mod` 声明零变化；同名类型冲突由编译器报 **E0255**。
- **内容**：类型定义 + 自动构造器 + 透传 `pub use` / `export type { } from` + 头注释 + **签名见证**（Rust 侧 `const _: fn(<入参类型序列>) -> <返回类型> = super::<Type>::<op>;` 与 `const _: fn() -> super::<Type> = super::<Type>::new;`；TS 侧靠实现侧 `import type` + 标注返回类型，由 `tsc` 报 TS2322）。
- **零 `_ext.rs`**：`impl` 块写模块内任意手写文件（Rust 允许同 crate 内 `impl` 分散）；生成 / 手写分离靠文件命名。
- **零内容不生成**：契约无类型无 op 时不生成 `.i.*`、不做入口编排（否则 `pub use api_gen::*;` 触发 `unused_imports`，`clippy -D warnings` 红，事实校准 6）。

### 持久化四（数据资产）：本仓模型与 ADR 的追加

- `models/05-backend-tools.toml`：`2 元素 / 1 关系` → `3 元素 / 3 关系`（新元素 `App.Backend.Tools.ArchiApiGen` + 2 条出边）。
- `decisions/0001-model-granularity.md`：`§1` / `§4` / `§6` / `§9` 四处计数同值 **51 元素 / 113 关系（后端 49 + 前端 64）**，自研 crate 16。
- 三处计数复述同值：模型表头 / `repo-archi-model` 的计数表（spec delta 已同步）/ ADR `§9` 复算表（AC-15）。
- `specta-zod-patched` 单列为既有已知缺口（MUST NOT 顺手补齐）。

### 持久化五：报告字段面（`reports/archi/<YYYYMMDDTHHMMSSZ>.json` 的增量）

既有 14 字段逐字不变；末位新增两项：

| 字段 | 类型 | 语义 |
|---|---|---|
| `apiFindings` | `ArchiApiFinding[]` | 契约判据结果（空数组 = 无契约问题）；**首期不影响 `status` / `passed`**；**排序**：`(kind, element, target)` 稳定序，两次运行逐字相同（D18） |
| `version` | `u32` | `1` → `2`（`version = 1` 的报告无 `apiFindings` 字段，消费方按版本分支，MUST NOT 补默认值） |

`ArchiApiFinding` 六字段：`kind`（十值封闭）/ `category`（开放字符串，六值初见）/ `element`（对标既有「引入方」）/ `target`（对标既有「被引入方」）/ `message` / `occurrences`（复用 `ArchiDriftOccurrence`，同元素对聚在一条内）。

### 持久化六：映射规则资产 `api/common.json`（编译期内嵌，非运行时文件）

- **形态**：单层 JSON 对象，键 = **协议名字**（首期唯一键 `Outcome`；契约内承载键为 `"outcome"`，换算单点 `api::protocol_key`，D57）；值 = `CommonRule`（`rust` / `typescript` 两槽位必填 + 可选 `degraded` / `note`），槽位内用 `{}` 标注负载位置。
- **加载**：`api::parse_common` 是唯一解析点；`api::meta()` 把结果放进 `ApiMeta.common`；`load_contracts` 以同一函数复核并映射为 `ApiError::Contract`（缺槽位 / 零 `{}` / JSON 非法）。
- **消费**：`emit::rust::common_type` 与 `emit::ts::common_type` 各按语言取槽位；渲染器**零读盘**（数据经载元注入）。缺键 ⇒ 该 schema 不属任何协议，走常规类型映射。
- **不落盘、不 vendor、不版本声明**：`include_str!` 编译期内嵌；生成物头注释带其摘要（D12）。**MUST NOT** 往里加「带生成产物的类型」（加即触发分发 / 版本 / 冻结方案的再评估，R-12）。

### 内存模型（不落库）

- `ApiSnapshot`（一次装载：契约 / 内存 schema / 符号集；**无缺契约清单**，D36）；
- `ApiSymbol` / `ApiParam` / `ApiType`（归一化 API 面事实，判据与渲染的唯一中间形态）；
- `ApiCheckInput`（判据求值入参，全部为借用切片，零 clone 大结构）。

---

## 路由 / API 设计 (Route / API Design)

### IPC 命令面：零新增命令

本变更**不新增 IPC 命令**，五个命令数量不变。唯一变化是 `archi_module` 的出参形状扩展：

| method | path（命令名） | 说明 | 输入 | 输出 | auth |
|---|---|---|---|---|---|
| invoke | `archi_module` | 单元素详情 | `root: String` + `element: String` | `ArchiModuleDetail`（新增并列字段 `api: { status, contractPath, schemaPath, generatedPath }`） | 无（本地桌面应用，沿既有命令面） |

- 命令层仍是薄包装（`dto::module_detail` 单点投影，零代码改动——`api` 字段由 `dto` 内部填）。
- **契约正文的读取通路不在本期**：`archi_read_doc` 只接受文档关联集内路径，不覆盖 `<archi_api_dir>/**`；agent 用 `read` 按 `archi_module` 给出的 `contractPath` 自取正文。MUST NOT 新增读取命令。

### 前端取数：零新路由

- 本变更不产生可见 UI 面；`packages/desktop/src/**` 的改动只有三类（生成物 `.i.ts` 本期零份 / 机械随动白名单 8 个测试文件 / 测试相位的 `ipc.test.ts`），零逻辑改动。另加 `knip.json` 一条 ignore。
- 画布 `driftMarks` 只读 `observations` / `suppressed`，零改动；`apiFindings` 本期不上画布。

### CLI（工具边界，非 HTTP / 非 IPC）

| 子命令 | 说明 | 输入 | 输出 / 退出码 |
|---|---|---|---|
| `archi-api-gen generate` | 写四类生成物（`<FQN>.schema.json` / `INDEX.toml` / `<root>.i.rs` / `<root>.i.ts`）+ 目录创建；幂等 | 无参数。root 由 **D32** 锚定；MUST NOT 接受 `--root`、MUST NOT 把 cwd 当 root | stdout 列写入文件数；`0` 成功 / `1` 契约或 IO 错误 |
| `archi-api-gen --check` | 生成到内存后与磁盘比对，**零写入**；「应删未删」取自**全树递归**的孤儿扫描（D34） | 无参数（root 同 `generate`） | stdout 打 JSON 差异清单（`{ mismatched: [], orphaned: [], missing: [] }`）；`0` 无差异 / `1` 有差异 / `2` 用法错 |
| `archi-api-gen bootstrap <element>` | 输出候选草稿到 stdout | 元素 FQN（root 同 `generate`）。**不读 knip**（D39） | md 骨架文本（含符号 / 形态 / 位置 / 归一签名 / `visibility`）；不含 `used_by`；不写盘 |

三个子命令共用的 root 一律经 **D32** 锚定：`api:export` / `api:check` 是 pnpm 脚本（无 `cd` 前缀），pnpm 恒以 cwd = `packages/desktop` 执行，因此「把 cwd 当 root」会让 `<archi_api_dir>` 派生出不存在的路径 ⇒ 生成物落错位置且 `--check` 恒报缺失。

---

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 类型 | 用途 | 状态 |
|---|---|---|---|
| `syn 2`（`features = ["full"]`） | `archi` 外部（已在） | Rust 声明抽取 + AST 归一签名 | 零新增 |
| `proc-macro2`（`features = ["span-locations"]`） | `archi` 外部（已在） | `syn` 项的行号定位（缺该特性则行号恒 0） | 零新增 |
| `oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax`（同版） | `archi` 外部（已在） | TS 声明抽取（只抽导出面，D19-a） | 零新增 |
| `serde_json`（既有 `raw_value` 特性） | `archi` 外部（已在） | 三类 fence 内容解析、`schema.json` 装配与序列化、**`common.json` 解析（D54）**、`--check` 差异清单 | 零新增 |
| `sha2 0.10` | `archi` 外部（已在） | `common.json` 摘要（生成物头注释） | 零新增 |
| `toml_edit 0.25` | `archi` 外部（已在） | `INDEX.toml` 文档构造（保留注释与稳定序） | 零新增 |
| `globset 0.4` | `archi` 外部（已在） | 契约面启用判定与 `$defs` 键过滤的形态匹配 | 零新增 |
| `foundation` | workspace 内（`archi` 唯一） | `archi_api_dir` / `archi_api_rel_dir` / `archi_dir` 路径派生 | 零新增依赖，新用两个函数 |
| `archi` | workspace 内（`archi-api-gen` 唯一） | 契约解析 / 装配 / 判据 / 渲染 / **`display`**（D55） | 新 crate 的一条边 |
| `foundation` | workspace 内（`archi-api-gen`） | 契约目录与生成物路径派生 | 新 crate 的第二条边 |
| `serde_json` | `archi-api-gen` 外部 | `--check` 差异清单输出 | 工具自身 |

**零新增外部包**：`crates/core/archi/Cargo.toml` 与 `crates/core/checks/Cargo.toml` 的 `[dependencies]` 条目数与内容均零变化；`crates/tools/archi-api-gen/Cargo.toml` 只引入锁内既有的 `serde_json`。四条红线零变化：`archi` 仍零 Tauri / 零 store / 零 config / 零 spawn / 零 `workflow`；`workflow` ⇄ `archi` 双向零依赖；`archi-api-gen` 零 Tauri / 零 store / 零 config / 零 orchestration / 零 workflow / 零 infra / 零壳，且**不被 app / core / infra 任何 crate 依赖**。

**试点生成物的 serde 面**：`crates/infra/archi.i.rs` 只含 `pub struct LocalArchiCheck;` + `impl new()` + 签名见证 —— **零 serde 需求**，`infra/archi` 的 `Cargo.toml` 无需新增依赖（该 crate 现无 serde，生成器硬写 `#[derive(Deserialize)]` 会直接编译失败；`enum` 派生策略见待决问题 1）。

### 构建期与工具链依赖（既有机制，零新增包）

| 依赖 | 用途 | 说明 |
|---|---|---|
| `tempfile`（既有工作区收敛项） | 工具 crate 的 tempdir 驱动与 `archi` 侧契约面用例 | 沿既有用法 |
| `DESKTOP_GOLDEN_REWRITE=1`（既有环境变量） | 漂移语料金样显式重写 | 既有机制，零新增开关 |
| `tsc --noEmit`（既有脚本） | `.i.ts` 形状校验与 `bindings.zod-check` | 既有工具链 |
| `knip`（既有脚本） | **本期只作 `client:check` 的既有死代码门**；TS 侧 `unused` 判定的唯一来源**在接线后**才生效（D39） | 既有工具链 |

**不引入**：无 clap（手写 argv）、无 markdown 解析库、无 JSON Schema 校验库运行时、无 zod。

---

## 验收标准对齐 (Acceptance Mapping)

> 本节覆盖 proposal 的 **20 条 AC** 与设计补入的 **AC-21…AC-34 十四条** = **34 条**。proposal 侧的 AC 措辞以 `proposal.md` 为准，本表只给设计落点；AC-21 起的十四条是本设计为「spec 未给的成形缺口」补的验收面，逐条的判据形态写在本表内。**AC-34 是本稿新增**（D58 / D59 / D60 三条收口裁定的机械判据合一），其余三十三条逐字不动。

| AC | 设计落点 | 见 |
|----|----------|----|
| AC-1 资产形态闭环 | `contract::parse`（五类位置语义）+ `contract::verify`（双向 gate 三条，差异点名）；合写一份、MUST NOT JSON 内嵌描述 / YAML block scalar | D6 |
| AC-2 `schema.json` 纯派生不可手改 | `schema::build_schema`（六字段逐项有 md 出处）+ 头 `$comment`；`--check` 报三类偏差 ⇒ 手改已提交必红、未提交被覆盖并留 R-1；`INDEX` 的 `categories` 同一来源（D26） | D7 / D13 / D21 / D26 |
| AC-3 引用与命名五规则 | `schema::verify_defs`（键名身份 / 提升原名 / `$ref` 顶层 / 只定义一次 / 禁结构去重）+ 透传纯 `$ref` 校验；重复键在文本层判（D37） | D8 / D37 |
| AC-4 生成物落点与入口查找 | `paths::{normalize_root, generated_path, entry_form, module_name, rust_module_path, mod_path_attr, ts_import_specifier}`；`owner.rs` 的 `.i.<ext>` 剥离特例；三分支 / 四分支封闭；**`#[path]` 的值是字面量且 MUST 与 `mod_path_attr` 输出逐字相等**（D38：三处判据 = `paths_test` 三形态断言 / tasks 逐入口自查行 / 注释如实化）；形态与 `<name>` 各只有一个判据（D40 / D41）；**specifier 只由 `form` 与 `ModuleRef.name` 决定**，`EmitTarget` 无第三输入（D58——`element_root` 已删，不存在「元素根影响 specifier 形态」的通路） | D2 / D3 / D4 / D5 / D38 / D40 / D41 / D42 / D58 |
| AC-5 类型映射两侧对等 | `emit::rust::render` / `emit::ts::render`（七种协议形态）+ 构造器由生成器产出且不进契约 + 位置分派为内部表 + 泛型零支持；`oneOf` / `anyOf` 分支负载**一律经 `value_type`**（无第二份标量表，D59）；`x-class` 带属性 ⇒ `verify_defs` 判错、`properties` 全为函数未标 ⇒ `render_target` 出 stderr 警告 | D9 / D10 / D11 / D27 / D45 / D59 |
| AC-6 TS 形状校验闭环 | 实现侧 `import type` + 标注返回类型（`tsc` 报 TS2322）；`as const` + `type` 同名共存；`knip.json` 只加 `src/**/*.i.ts` 一条 ignore | 变更清单 `knip.json` 行 |
| AC-7 十条判据与期望数对拍 | `check::check` 的固定求值序与互斥：`apiEntryMissing` 28 / `apiMissing` 13 / 其七条 0 / `apiUndeclared` 不适用 | D16 / D17 / D19；下方判据基线表 |
| AC-8 判据词汇与报告槽位 | `ArchiApiFindingKind` 十值 + `ALL` + `as_str` + `severity`；`category` 开放（`category_of` 单点，**只经 `check.rs` 模块路径被消费、不经 `api` 出口面**，D60）；报告末位 `api_findings` + `version = 2`；金样仅追加两处 | D14 / D15 / D18 / D60 |
| AC-9 判定单源 | 唯一实现 `api::check`；`drift::check` 单点装配；命令面与门禁面同一函数；`$defs` 只读 `schema.json`；重复键只在 `contract::parse` 的文本层判一次 | D18 / D19 / D37 |
| AC-10 `api` 元素字段 | `ApiStatus`（二值 + `ALL` + 缺省 `declared`）+ `RawElement.api`（`deny_unknown_fields` ⇒ 拼错报 `invalidValue` 并列全合法值）+ `ArchiApiRef` 四字段；容器元素 `path` 空即天然无契约 | D22 |
| AC-11 嵌套与透传 | 选择性透传（三处落位等键集）+ 纯 `$ref` alias + 父子 `$defs` 交集为空 + 直接子元素约束 + 跨 crate 父子；末段形态闸按语言分列 | D8 / D40 / D48 / D50 |
| AC-12 生成器工程面 | `generate` 写四类 + 幂等；`--check` 零写入 + 非零退出 + JSON 差异清单；`bootstrap` 只出 stdout；root 由 D32 锚定；`archi` 零写盘零 spawn；依赖面恰 `archi` + `foundation` + `serde_json`；**生成物须是格式化器不动点**（D29） | D1 / D21 / D29 / D32 / D33 / D34 |
| AC-13 试点端到端 | md + `schema.json` + `.i.rs` 三者落地；无 `### new`；生成物与手写 `impl Default` 共存编译；`archi_module` 该元素 `api.status = declared`；判据零命中 | D23；事实校准 3 / 4 / 9 |
| AC-14 交付面 | `bindings:export` 再生成后零 diff；`api:check` 零 diff；`version` = 当前 `HEAD` 值 +1 patch；`vp check` / `knip` / `clippy -D warnings` 通过；**`server:check` 为校验模式**、`pnpm run check` 连跑两次第二次 `git status` 与第一次逐行相等；`check` 链逐字为四段（第二段 `bindings:export`） | D28 / D30；事实校准 24 / 25 |
| AC-15 模型与新增 crate 同 change 同步 | `05` 段 3 元素 / 3 关系；全基线 51 / 113（后端 49 + 前端 64）；自研 crate 16 ↔ 16；三处计数同值；`archi_graph` 零诊断零告警 | D24；变更清单两行数据资产 |
| AC-16 零债务消解副作用 | 不为消债建 barrel / 改元素根路径 / 挪类型；前端 `path` 写法逐字未变；唯一**契约相关**代码形状改动限试点元素；**并列允许项零条**（行尾面实测零处不等：`package.json` 两侧同形 LF、六份 `sdk/**` 零债务且不在任何集合内；交付前出现的四处行尾翻面已按各自 `HEAD` 形态归位）；封闭集合外零改动（**60 路径** = 清单 42 + 白名单 18） | D3 / D23 / D28 / D30；机械随动白名单 |
| AC-17 语言无关纪律 | 资产面（`.md` / `.schema.json` / `INDEX.toml`）零 `crate` / `Cargo.toml` / `Rust` / `TypeScript` 词；语言特定只落 `api/extract/*` 与 `api/emit/*` | D12 / D13 |
| AC-18 `AGENT.md` / `CLAUDE.md` 契约纪律节 | 两文件各恰一节 `## 模块 API 契约`、节内五条要点、节内正文逐字相同、MUST NOT 复制契约正文 / 判据长表 / fenced json 示例、节内零 `openspec/specs/**` 路径；判据 = `grep -c '^## 模块 API 契约'` 各 1、`grep -c 'apiEntryMissing'` 各 0、`diff` 恰 2 处差异且全在节外（`:1` 标题、`:3` 首行说明；两文件末尾换行同形） | 变更清单 `AGENT.md` + `CLAUDE.md` 行 |
| AC-19 `foundation::layout` 派生函数 | `API_DIR_NAME` + `archi_api_dir` + `archi_api_rel_dir`，全从 `archi_dir` / `archi_rel_dir` 派生、纯推导零 IO、段名字面量只在 `API_DIR_NAME` 一处、与 models / decisions / config 路径互不为前缀。**用例面改指向既有 Scenario（不新写、不重述）**：`specs/workspace-layout-resolution/spec.md` 的 `Scenario: archi_api_dir 与其他派生互不遮蔽`（:46，覆盖「同源 + 零 IO + 互不遮蔽」）、`Scenario: archi_api_rel_dir 同源且前缀一致`（:82，覆盖「相对族前缀 + 与绝对族尾部逐字相等」）、`Scenario: 与绝对路径族尾部一致`（:72）、`Scenario: 零 IO 与零字面量`（:77）。**附加机械 `grep` 四条**（对象 = `crates/core/foundation/src/layout/mod.rs`；凡涉全仓扫描的先滤 `*_test.rs`）：① `grep -c 'API_DIR_NAME'` ⇒ **4**（常量定义 + doc 引用 + 两函数消费）；② `grep -rl 'API_DIR_NAME' --include=*.rs crates/ src/` 滤测试后 ⇒ **1 个文件**；③ `grep -rn '"api"' --include=*.rs crates/ src/` 滤测试后 ⇒ **1 处**（`const API_DIR_NAME: &str = "api";`）；④ `grep -cF "pub fn archi_api_dir("` 与 `grep -cF "pub fn archi_api_rel_dir("` 各 ⇒ **1**（两 `pub fn` 签名行；两串分列计数，MUST NOT 用扩展正则的竖线交替——该形态在 markdown 表格内还会破坏列切分） | 变更清单 `layout/mod.rs` 行；公共函数表三行 |
| AC-20 bootstrap 取数面 | **判据一律用符号锚点，MUST NOT 写死行号**。`normalize_signature` 单点定义 1 处、`check.rs` 消费 1 处、`check.rs` 对语言取数器直连 **0**；`rust.rs` 的 `syn::parse_file` 恰 3 处、`regex` 零命中；`Visibility` 四变体 + `ALL: [Visibility; 4]` 恰 1 + 四词各 1；`rust.rs` 三值产出点合计 6、`Visibility::Unused` 零命中；`ts.rs` 的 `CrateOnly` / `TestOnly` 零命中、`Unused` 唯一产出点在 `apply_knip_report` 内且**产品路径零调用者**；TS 取数面 = `Statement::` 恰 1 条顶层分支、六个非导出分支 / `ArrowFunctionExpression` / `ExportDefault` 各零；`Language::Unknown => Vec::new()` 恰 2 处、`used_by` 产品面零命中、文件名族零非注释命中 | D19-a / D19-b / D39 / D43 |
| **AC-21 位置词汇与签名短形各只有一份** | `Position` / `schema_type` / `signature_of` 定义在 `api/extract/mod.rs`；`check.rs` 零本地 `enum Position` / 零本地归一（`normalize_schema` / `contract_signature` 零命中），④ 步骤经 `extract::signature_of` 取数；两侧渲染器零本地 `Position`，一律 `use`。判据 = 三个符号在 `extract/mod.rs` 各恰 1 处定义 + `check.rs` / `emit/{rust,ts}.rs` 内 `enum Position` 零命中 | D43；公共函数表 `extract::schema_type` / `signature_of` 两行；类型表 `Position` 行 |
| **AC-22 `apiMissing` 的 op 与类型同口径** | `missing_contracts` 内 op 段与类型段**同一次求值**、共用三条排除面（构造器双向 / 透传纯 `$ref` / `x-class` 与 `enum` 计入）；两条结构前提（`#[path]` 模块定位、`pub use <mod>::*` 通配升格）在 `extract_crate` 内实装。判据 = ①「契约声明类型 `T`、代码无 `T`」必报 `apiMissing`；② 试点元素（自有生成物 + `pub use api_gen::*`）零假阳 | D44；S-14 |
| **AC-23 契约文本经两道人字闸** | 闸一：`fn is_identifier` 单点持有，**只管七族名字**（op id / `$defs` 键 / 透传键 / `params[].name` / `$defs` 条目的 `properties` 键 / `api-op` 的 `owner` / `$ref` 末段名）逐族在 `verify` 内判；闸二：契约文本进源码的**插值点枚举 8 组**各有承接地（闸一 / 闸二 / `paths::is_module_segment`）。**单一承接地纪律**：Rust 侧末段形态闸**只由** `paths::is_module_segment` 承接（D48 / D50，形态 + 非空 + 非关键字），`is_identifier` **MUST NOT** 承接末段形态、**MUST NOT** 被 `paths` 反向复用（`paths` 是 `api` 地基层，依赖方向不可反；两函数事实源不同：契约文本里写的名字 vs 路径派生的模块段）。判据 = `grep -c "fn is_identifier"` 滤测试后恰 1 且调用点在 `verify` 体内；`emit` 内 `is_identifier` 零命中；`paths.rs` 内 **`is_identifier(` 调用形态零命中**（该文件 doc 有一处「与 `contract::verify` 的 `is_identifier` 事实源不同」的说明句，故判据串锚调用形态而非裸名）且 `is_module_segment` 内零契约名判据；8 组逐组可指认承接者（末段两组 ⇒ `is_module_segment`，其余六组 ⇒ 闸一或闸二） | D45 / D48 / D50；S-15；AC-27 |
| **AC-24 两侧渲染器出口单形** | 两侧各**恰一个**四参 `pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String`（`target` 末位）；`render_with_root` / `render_with_target` 零命中；`render_scope` 私有。判据 = 逐产品文件带完整签名 `grep -c` 恰 1；产品面 `rust::render(` / `ts::render(` 的直连各恰 1（分派器分支） | D46 |
| **AC-25 `x-function` 落点封闭** | `contract::verify` 内的落点闸只放行 `api-op` 顶层签名与 `$defs` 条目第一层 `properties` 的值；其余位置报 `ApiError::Contract` 并点名 JSON pointer；两侧 `value_type` 零 `x-function` 分支。判据 = 四种越界形态（`items` / `additionalProperties` / `oneOf` 分支 / 深层嵌套）在 `load_contracts` 期即中止；`grep -c "x-function"` 在两侧 `value_type` 体内零命中 | D47 |
| **AC-26 `#[path]` 读盘面三步限根** | 归一在折叠之前 + 分段折叠 + `join` 后 `starts_with(root)`；越根即不命中该子树（零新错误面）。判据 = `grep -cF "starts_with(root)" crates/core/archi/src/api/extract/rust.rs` ≥ 1 + 三个反例（`../../evil.rs` 越根 / 反斜杠形态 / 栈空）各不命中 | D49；S-16 |
| **AC-27 末段形态闸按语言分列** | Rust 侧 `is_module_segment`（形态 + 非关键字）；TS 侧 `ts_string_literal` 零筛选；`checked_root` 不判末段字符集。判据 = `paths.rs` 内 `RUST_KEYWORDS` 恰 1 处常量、`is_module_segment` 恰 1 处定义；`emit/ts.rs` 的 `from` 经 `ts_string_literal`；关键字末段只出注释不产 `pub use` | D48 / D50；S-17 |
| **AC-28 `apiMissing` 的 `occurrences.line` 分列** | op 缺口取 `ContractOp.line`；`$defs` 类型缺口取 `0`；`Contract` 字段面不扩。判据 = 两条缺口的 `occurrences[0].line` 分别为 heading 行与 `0` | D51 |
| **AC-29 `extract::extract` 留白显式化** | 接口保留 + 模块 doc 与函数 doc 各一段留白声明；产品调用者零。判据 = `grep -rn "extract::extract(" crates --include=*.rs` 滤测试与定义后零命中 + 两处 doc 含留白声明句 | D52 |
| **AC-30 第四类机械随动的射程** | 只有三种形态（`use` 行 / 实参元数 / 装置同名提取），**不含**断言值 / 断言意图 / 用例名 / 增删用例 / 夹具语义；两落点已在测试相位集合内、白名单不增项。判据 = 五条机械判据（`use` 行改指 / 实参补齐取既有装置 / 装置提取 / 断言面零改动 / 白名单仍 18）+ 一条行为判据（撤掉该 `hunk` 后可编译性判据转非绿，即「编译必需」自证） | D53 |
| **AC-31 `common.json` 被渲染器消费（映射不硬编码）** | ① **资产形态**：键 = **协议名字**（首期唯一键 `Outcome`，与 spec `:149` / `:158` / `:185` 一致）+ 每语言单槽位 + `{}` 占位符，`ApiMeta.common` 携带解析结果，`api::parse_common` 是唯一解析点（缺语言槽位 / 槽位内零 `{}` / JSON 非法 ⇒ `ApiError::Contract`）；② **消费单点**：两侧各一个私有 `common_type` 查表函数，`value_type` 的协议分支 MUST 经它；**承载键与资产键的换算经 `api::protocol_key` 单点**（D57：`Outcome` → `"outcome"`），MUST NOT 在两侧各写一份、MUST NOT 把表键改小写；③ **机械判据（硬编码零残留，锚文本形而非键名）**：`grep -cF 'Result<{}, String>' crates/core/archi/src/api/emit/rust.rs` **0**、`grep -cF '{ ok: {} } \| { err: string }' …/emit/ts.rs` **0**（两串只允许出现在 `api/common.json` 与测试装置里）；`grep -c "fn common_type"` 在两侧各恰 **1**；`grep -c "fn protocol_key" crates/core/archi/src/api/mod.rs` 恰 **1**。**MUST NOT 用 `"outcome"` 作判据串**——它是合法承载键，恒在场（拿它当判据即自证型假判据）；④ **改表即变产物**：改 `common.json` 的某语言槽位后重跑 `generate`，两侧产物文本随变（不可由「改代码」实现）；⑤ **纯函数纪律不变**：渲染器零 IO、四参签名不扩（D46 不受影响） | D54 / D57；配置表 `common.json` 行；数据模型「持久化六」；**入表的是 `emit::rust::render` / `emit::ts::render` 两行**（承载查表消费），而 `api::parse_common` / `api::protocol_key` / `emit::{rust,ts}::common_type` **不入公共函数表**（`pub(crate)` 或私有工序，见该小节口径纪律①），它们只在 D54 / D57 与本节判据串里被点名 |
| **AC-32 `api::display` 跨 crate 单点** | `api::display` 为 `pub fn` 并经 `lib.rs` 出口；`archi-api-gen` 的 `main.rs` 内本地 `fn display` **零命中**，改用 `archi::display`；语义逐字不变（剥 root 前缀 / `\`→`/` / 剥前导 `./`）。判据 = `grep -c '^fn display' crates/tools/archi-api-gen/src/main.rs` **0**（锚行首定义形态：裸串 `fn display` 会误计 `file.path.display()` 方法调用与 doc 词面）、`grep -c "archi::display" 同文件` ≥ 1、`grep -c "pub fn display" crates/core/archi/src/api/mod.rs` **1**、`grep -c "pub use api::display" crates/core/archi/src/lib.rs` **1** + 一条行为判据（`--check` 的差异清单路径串与改动前逐字相同） | D55；公共函数表 `api::display` 行；修改文件表 `crates/core/archi/src/lib.rs` / `crates/tools/archi-api-gen/src/main.rs` 两行 |
| **AC-33 装置超时越界回滚** | `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` 的 `spawn_command_confirmer` 回到约定口径（确认窗 60s、无 `register_window`），其「见过 run 登记后转 `None`」的退出判据保留；复合用例的收敛等待回到 `wait_for`（30s 约定上限）。该文件产品面零改动，**实现相位对它零触碰**（不在 66 路径内）。判据（**owner = 测试相位**，实现相位只登记、不执行回滚）= `grep -cF 'from_secs(180)' packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` **0**、`grep -cF 'register_window'` 同文件 **0**、`grep -c 'wait_for_up_to'` 同文件 ≥ 1（装置正确性前提仍在）、`git diff HEAD` 对该文件**只剩测试相位认可的最小改动**（否则整文件回到 `HEAD`）。**判据串一律取物理全路径**（零 `…/` 缩写、零 `src/` 省略），与「核对口径」段的「判据串口径」条同源 | D56；测试相位产出白名单「壳命令面用例」行（该行明写全路径与「回滚由本相位执行、实现相位零触碰」） |
| **AC-34 三条收口裁定的机械判据（本稿新增）** | ① **`EmitTarget` 三字段**（D58）：`pub struct EmitTarget` 的定义体内**恰三字段** `kind` / `form` / `modules`，`grep -cF 'pub element_root' crates/core/archi/src/api/emit/mod.rs` ⇒ **0**、`grep -cF 'pub path: PathBuf' …/emit/mod.rs` ⇒ **0**；产品面读 `EmitTarget` 的行恰 3 且只读三字段（判据面取 `emit/mod.rs` / `emit/rust.rs` / `emit/ts.rs` 三份 `include_str!` 源码，**不用全仓 `grep`**——`EmitTarget` 的定义与消费面只在本层，全仓扫描会把任何名为 `target` 的无关局部变量计入：`emit/mod.rs` 的 `.kind`、`emit/rust.rs` 的 `.modules`、`emit/ts.rs` 的 `.form` 与 `.modules` **同行** ⇒ 判据取**行数不取串数**）；配合 `grep -rnF -e 'target.kind' … 滤 _test.rs` 的 3 行作旁证；**`element_root` 在全仓产品 `.rs` 内的 `EmitTarget` 语境零命中**（`paths::{generated_path, module_name, rust_module_path}` 的形参名 `element_root` 保留——那是路径派生的入参、不是 `EmitTarget` 字段）；行为面：删两字段前后同一契约的渲染产物**逐字节相等**（两字段被读即不可能相等，产品面读取点实测零——事实校准 34）；`grep -cF 'use std::path::PathBuf' crates/core/archi/src/api/emit/mod.rs` ⇒ **0**（该 `use` 只服务被删字段，残留即 `unused_imports`）。② **分支负载单通路**（D59）：`grep -cF 'fn choice_payload' crates/core/archi/src/api/emit/rust.rs` ⇒ **0**（函数已删）；`variants_block` 内**零**标量表（`grep -cF '"i64"' …/emit/rust.rs` 由删前的 **3** 降为 **2**，剩下的两处均在 `value_type` 与 `scalar_type` 内、无第三处）；同一 `choice` schema 经 `value_type(choice, Position::Field, common)` 与经 `variants_block` 的负载文本**逐字相等**（含协议键分支——两路径都经 `common_type`）。③ **`category_of` 出口面**（D60）：`grep -cF 'pub use check::category_of' crates/core/archi/src/api/mod.rs` ⇒ **0**；`grep -c 'fn category_of' …/api/check.rs` ⇒ **1**（定义与 `pub` 不变）；`grep -cF 'use super::check::{category_of, check}' …/api/check_test.rs` ⇒ **1**（测试面走模块路径，删出口不动用例）。④ **静态门与试点零影响**：`cargo clippy --workspace --all-targets -- -D warnings` 零告警；`cargo run -q -p archi-api-gen -- --check` **exit 0**（试点四类生成物逐字节不变——D59 的行为扩展面在试点契约上不触发，D59 ⑤） | D58 / D59 / D60；事实校准 34 / 35 / 36；`tasks.md` 阶段三十 |
| **S-1…S-17**（规范文本同步，非 AC —— 规范↔实现冲突的 owner） | **十七条**「原文须消失 / 替换后原文须在场 / 校验命令 / owner 相位」逐条在「spec 措辞同步项」小节。**甲类六条（门禁形态）**：delta 四处（S-1 / S-2 / S-3 / S-6）归本 change spec 相位，主基线两处（S-4 / S-5）归归档 SpecSync。**乙类四条（取数面边界）**：S-7（两处原文）/ S-8 / S-9 / S-10（两处原文）。**丙类两条（可执行形态）**：S-11（D38）/ S-12（D39）。**丁类三条（单点与闸面收口）**：S-13（保留型补句）/ S-14（三处锚点，其中 Module Contract 表行旧串消失）/ S-15（纯新增）。**戊类两条（限根与形态闸，纯新增）**：S-16（D49）/ S-17（D48 / D50）。**射程判据**：五批之和 = 表内条数 **17**；旧原文串 **14**、新原文串 **18**（三数分列，MUST NOT 混用）；射程自检的 `grep` 片段 MUST 从表机械抽取 | D31 / D38 / D39 / D43 / D44 / D45 / D48 / D49 / D50；tasks 阶段二十（delta 自查的命中数为 **4 + 5 + 2 + 1 + 2 = 14** 经典串；保留型与纯新增项按「新串在场」单判） |

### 判据基线表（AC-7 的实测口径；分母 = `path` 非空元素 42 个）

| 判据 | 期望数 | 构成 |
|---|---|---|
| `apiEntryMissing` | **28** | 27（既有清单：壳 1 + `ExportBindings` 1 + 前端 25）+ `App.Backend.Tools.ArchiApiGen` 1（只有 `main.rs`，不消解） |
| `apiMissing` | **13** | 命中 `<dir>/src/lib.rs` 的 14 个 crate 元素 − 1 个试点 |
| `apiSignatureMismatch` | 0 | 试点 op 与代码归一签名一致 |
| `apiUnexpected` | 0 | 无 `api = none` 元素持有契约文件 |
| `apiShadowing` | 0 | 无嵌套元素持契约 |
| `apiRefMissing` | 0 | 试点契约零跨模块 `$ref`（D23 的选型结果） |
| `apiNestedCrossCrate` | 0 | 9 对父子元素全在前端（无 crate 概念） |
| `apiReexportDrift` | 0 | 试点无 `api-reexport` |
| `apiConstructorDeclared` | 0 | 试点契约无 `### new` |
| `apiUndeclared` | 不适用 | `severity() = Off`（唯一完备性判据） |

> 口径纪律：报告数与基线表**逐项相等**即「检查能正确提示」；多于 = 假阳、少于 = 假阴，两者都是检查器缺陷（MUST NOT 更新基线表放行）。数字变更须在 change 报告（`reports/`）内说明成因；基线表不写进断言。

---

## 待决问题 (Open Questions)

1. **生成器对 `enum` 的 `Deserialize` 派生是否无条件下发**（`(待确定)`）。设计取「按元素 crate 现有依赖决定：已有 `serde` ⇒ 下发；否则只有 `Debug/Clone/PartialEq/Eq`」。试点是零字段 struct，不触发该分支；真实 `enum` 元素首个契约落地时按该元素 `Cargo.toml` 定，若届时需要跨语言一致的序列化保证，则须回到设计层补一条决策。
2. **`x-common.degraded` 是否收紧为封闭 reason 枚举**（`(待确定)`）。设计取「自由字符串 + INDEX 的 `degraded` 计数」；本期只落计数，不定词表。
3. **非组件前端元素是否同期生成 `.i.ts`**（`(待确定)`）。本期试点只在 Rust 侧，故 `.i.ts` 路径全链路有实现无真实数据；首个前端元素契约落地时才算首次端到端。若要求本期就有真实 `.i.ts`，须先解决「前端元素零命中的入口四分支」债务，与 AC-16 冲突，故不做。
4. **`api:check` 是否需要在脚本层再挂一次 `git status --porcelain`**（`(待确定)`）。设计取「不挂」（D21）。若评审认为需要脚本层兜底，应新增一条独立脚本（不是塞进 `api:check`），届时另案。
5. **`bootstrap` 的 knip 输入形态**（**已裁定：本期不读**，D39）。接线（含 knip 报文 `files.<path>.exports[].name` 的字段名与版本兼容性确认）随首个 TS bootstrap 落地的变更做，届时同时补 D19-b 的判据形态与 spec 措辞（S-12 的替换文本要再改一次）。
6. **契约面启用判定是否应递归进 `<Domain>` 子目录**（已定）。设计取「递归扫 `*.md`」：只看根一层会让「有契约但目录深一层」被判未启用，与「目录一旦存在即全量纳入」的意图相反。
7. **`INDEX` 的 `categories` 与报告 `category` 是否合并**（已裁定：不合并，D26 / 事实校准 14）。合并会让零 diff 门在每次「消一条债」时对 `INDEX.toml` 报差异。若将来需要「INDEX 带判据分布」，那是第二个表，不是复用 `categories`。
8. **`x-class` 警告是否应该进报告**（已裁定：不进，D27）。若将来需要报告面可见，须先裁决「是否为它开第十一值」，属独立变更。
9. **`client:check` 的 `vp check --fix` 是否也该改成校验模式**（`(待确定)`，本稿有意不动）。本稿只把 `server:check` 的写模式 fmt 段改掉（D28）；`--fix` 是前端既有约定（多份 spec 明文写），改它触发面远大于本变更；且 D29 / D30 已让生成物与 `package.json` 停在不动点，故 `--fix` 在交付态下不再产生新 diff。**残留风险**：将来若有人留下不合 `vp fmt` 的字节，`client:check` 仍会静默改写（与 R-1 同款）。根治是独立的门禁一致性变更，MUST NOT 在本变更内顺手做。
10. **`.gitattributes` 是否该声明行尾策略**（`(待确定)`，本稿不动）。处置是「不靠行尾策略，靠不动点」：门改 `--check` 后不再有写模式改写。仓库层根治需单独引入 `.gitattributes` + `git add --renormalize`（会触碰大量文件，与 AC-16 正面对撞）。**注意**：`package.json` 的 `i/crlf` 索引态仍保留，只有其工作树字节变成 LF（D30 的显式取舍）。
11. **S-4 / S-5 的主基线同步落在归档 SpecSync 步，是否需要在归档前强制校验**（已定）。设计取「登记 + 归档步执行」：本 change 的任何相位 MUST NOT 写 `openspec/specs/**`。归档步的机械判据即 S-4 / S-5 的双向 `grep -F`。**残留风险**：若归档步漏做，主基线会长期保留旧叙述——缓解是把该风险写进 change 报告的「待归档步消解」栏。
12. **TS 侧 `crateOnly` / `testOnly` 的补齐路径与时机**（`(待确定)`，D19-b 的留白项）。两条候选路径：① 复用 `scan` 的 `ImportFact` / `DependencyEdge`（代价是 `api/**` 从「单文件 AST」变成依赖 `scan` 产物）；② 读 knip 报告的引用字段（代价是报文兼容性未确认）。补齐时必须同时给出**判据的机械形态**与 **spec 措辞**（届时 S-7 / S-8 要再改一次）。**MUST NOT** 在补齐前用路径或文件名充当 proxy。
13. **`used_by` 的输出形态**（`(待确定)`，与第 12 条同源）。候选：`Vec<String>`（引用方 repo 相对路径，稳定序）或 `Vec<(String, u32)>`（含首次引用行号）。本期不产出；补齐时须与 `visibility` **并列而非替换**（两个不同问题，MUST NOT 合并成一个字段）。
14. **`common.json` 的键名是否扩展为「协议族」而非单协议**（`(待确定)`）。首期唯一键 `Outcome` 覆盖错误面一个协议；将来若出现「第二类跨语言同构信封」（例如分页 / 游标），应扩键而非扩槽位（一个键 = 一个协议，槽位恒为语言）。若某语言需要两种形态（如 `Promise` reject 与 union 并存），须先裁决「槽位内是否允许并列形态」，本期按单槽位实现。
15. **`common_type` 的 `position` 参数是否真的需要**（`(待确定)`）。当前两类协议（错误面 / 异步包装）都不随位置变化，故该参数可能恒被忽略；保留它是为「将来某协议在入参位与返回位形态不同」留接口。若确认永不使用，应删除参数并同步两侧签名（属独立变更，MUST NOT 在本变更内提前删）。
