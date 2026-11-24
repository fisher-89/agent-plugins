# 设计: archi-model-path-and-silent-trap

> **变更**: archi-model-path-and-silent-trap（架构模型 · `kind` × `path` 判据 + 「静默陷阱」理由纠偏）
> **日期**: 2026-11-24
> **基于**: `proposal.md`（问题三节 + 提案七节 + 硬前提四条 + 能力四份 delta + AC-1…AC-13 + R-1…R-11）、`explore.md`（只读输入）、`specs/` 下 4 份 delta（`desktop-archi-model` / `desktop-archi-drift` / `desktop-archi-agent` / `repo-archi-model`）
> **修订**: 2026-11-24（回溯返修）——AC-6 由「全仓零命中 + delta 命中须同行含 `MUST NOT`」改为**三面分判**：正向表述面零命中 / 防线面必须在场 / spec 面收窄到本 delta 且只许否定形态。动因与落点见 D14 与「回溯修订记录（AC-6 判据口径）」
> **性质**: **判据新增 1 处（`package` 的 `path` 上界）+ 判据语义收窄 1 处（`noPath` 按 `kind`）+ 数据面 `kind` 迁移 3 行 + 文本面假因果收口 9 处**——零新增诊断词、零新增漂移词、零 IPC / UI 变化、零契约面变化。
> **不含**: L4（`module` 多 `path` × 契约面只认 `path.first()`）；「凡以静默为由留散文的句子 MUST 附行为测试」的元纪律入 spec；`metadata` 子表推荐形态改向；`kind` 层级序校验；语料 / 金样任何改动。

---

## 起点实况（`HEAD` 基线实测）

| 面 | 实测（文件:行） |
|---|---|
| 格式判定单点 | `parse/toml.rs:30` `RawElement` 八字段（`path: Vec<Spanned<String>>`——**逐条带 span**，位置可取）；`:101` `to_model(raw, source) -> (Model, Vec<Diagnostic>)`（诊断源 = serde + `apis` 值面）；`:163` `validate_apis`（带位置 `invalidValue` 的既有形态）；`:232` `classify` |
| 聚合 | `parse/mod.rs:36` `parse_dir`：按文件名字典序解析并**累积**诊断（不停摆、不丢文件）；`:67` 逐文件回填 `file` |
| 停摆 | `dto.rs:646` `load` → `:754` `graph_view`：`diagnostics` 非空 ⇒ `elements` / `relations` 清空（**唯一收口点**，全或无语义） |
| 判定面 | `drift/mod.rs:89` `check`；`:114` `if element.path.is_empty()` ⇒ **不分 kind** push `NoPath`（本仓 6 条）；`:148` `pathNotFound` push（定级面）；`model_warnings.push(` 恰 **2** 处、`errors.push(` **3** 处、`warnings.push(` **6** 处（`drift/mod_test.rs:3476` 以源文本计数钉住） |
| 词汇 | `drift/report.rs:94` `ArchiDriftKind`（封闭五值）；`:88` 枚举级总述尾句「`noPath`（元素未声明 path）」；`:101` 变体注释「元素未声明 `path`（不参与归属）」；`:120` `ALL` 五值序 |
| 模型数据 | 55 元素 / 129 关系；`kind` 分布 `package` **22** / `domain` 3 / `module` 30 / `composite` **0**（**迁移前**；当前工作树已落盘为 19 / 3，见文末「落地面」）；三容器 `Backend.Core`（`02:4`）/ `Backend.Infra`（`03:4`）/ `Backend.Tools`（`05:4`）当时为 `kind = "package"` + 空 `path` |
| 报告基线 | `reports/archi/20261123T1753385247126Z.json`：`elements` 55 / `relations` 129 / `model.warnings` **6**（全 `noPath`）/ `observations` 53 / `apiFindings` 39（全 `docMissing`）/ `errors` 0 / `warnings` 0 / `structuralErrors` 0 / `status = clean` / `passed = true` / `scan` = 422 / 359 / 422 / 0 / 0 |
| 知识骨架 | `archi-knowledge.md`（42 行）：键形句 `:5`（「点分嵌套与子表读成**另一元素**（`metadata` 必须内联表）」）、`path` 纪律 `:10`（「模块 / 包必填」）、`composite` 语义 `:24`（无「纯分层容器」句）。冻结锚 **2459 B / 2602 B**（LF 归一），区间 2300–2600 / 2450–2750，门限 2700 / 2850 |
| 模板 | `sdk/archi_tool.rs:198` `SCAFFOLD_TEMPLATE`：`kind` 行三值（`package` = 包根（无 path）/ `domain` = 容器（path 恒空）/ `module`）、`path` 行「叶元素必填…容器（package / domain）恒空」、`metadata` 行「必须是内联表（子表会被读成另一个元素）」；`:237` `scaffold_draft`（`__KIND__` 单点替换） |
| 断言面 | `archi_knowledge_test.rs:340` `骨架静默陷阱四条在场`（needle `内联表`）、`:120` 指路表（含 `缺 \`path\`` + `archi_module` + `hasPath`）、`:381` / `:400` 两个体量锚；`archi_tool_test.rs:987` `archi_scaffold三kind逐值`、`:961` 省略 kind（needle `内联表` / `叶元素必填`）；`toml_test.rs:310` `metadata非字符串值被拒且子表形态不静默成元数据`、`:345` `未引号点分表头被拒且不与合法形态合并`；`dto_test.rs:216` `graph_view九类结构错一律停摆`；`drift/mod_test.rs:906` `noPath警告`（`Ghost` = `module`，逐字不动） |
| 版本 | `packages/desktop/package.json` = **0.7.8**；`src-tauri/tauri.conf.json` = `"../package.json"`；`src-tauri/Cargo.toml` = `0.1.0`（自立版本） |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **`to_model` 签名本次不变**：上一变更已把它改成 `(Model, Vec<Diagnostic>)`，本变更只需在元素循环里 `diagnostics.extend(...)`——`parse` 的诊断合并逻辑零改动。
2. **位置取第 2 条无需新增解析**：`RawElement.path` 已是 `Vec<::toml::Spanned<String>>`，`path[1].span()` 直接可用，行列经既有 `line_of` / `column_of` 换算（不做二次扫文本）。
3. **元素照常入模型**：与 `validate_apis` 不丢关系同形——`to_model` 逐元素产诊断但仍 `elements.push`。`MUST NOT 跳过坏元素` 的纪律落在**既有**的 `graph_view` 停摆分支，parse 层零新分支。
4. **`package` 越界不触发结构诊断**：`graph.rs` 的四类结构诊断只看 FQN 前缀与关系端点、不读 `kind`；L2 迁移与 L1 新增判据都不改变它们的命中面（AC-4 的四类零命中由既有基线保证）。
5. **`ElementKind` 的产品面消费只有三处**（`dto` 词汇 / `model` AST / `parse` 反序列化）；`scan/owner.rs` 不读 `kind`、`api/**` 只读 `path` ⇒ **L2 迁移零代码联动**（`element-tree.tsx:115` 的 `composite → LayersIcon` 已就位）。
6. **定级面计数是硬约束**：`drift/mod_test.rs` 以源文本断言 `model_warnings.push(` = 2、`errors.push(` = 3、`warnings.push(` = 6——L3 的收窄只改条件表达式，三处计数逐字不变。
7. **`NoPath` 的 `message` 字符串逐字不动**：`api-mismatch` / `target-unowned` 两份金样逐字含它；改文案即连带改金样（本变更不做，R-7 的直接后果）。
8. **L1 零误伤（本次静态复算）**：`crates/**/tests/corpus/**/*.toml` 的 23 个 `package` 元素、`.dev-team/architecture/graph/model/*.toml` 的 22 个（迁移后 19 + 3 `composite`）`package` 元素、以及 `crates/**/*.rs` 与 `src/**/*.ts*` 内的内联测试模型，**零「`package` + ≥2 条 `path`」**。
9. **L3 零误伤**：含 `noPath` 的两份金样其 `App` 是 `kind = "package"` ⇒ 方案 B 仍报，20 份 `expected/*.json` 逐字节不变。
10. **`INDEX.toml` 与 schema 零产物变化**：索引 49 条 `[[entry]]` = 55 元素 − 6 容器，`kind` 不进索引 / `.schema.json` / `.i.*`；三容器本就 `path` 空而退出契约面 ⇒ `api:export` 预期零 diff。
11. **`NoPath` 的两处 JSDoc 落点已确认**：`bindings.ts:998`（枚举级总述尾句）与 `:1007`（变体行）；`bindings.zod.ts` 无注释面 ⇒ 零 diff。
12. **知识文本三条不可撞的既有断言**：① `骨架零字段形态教学` 禁 `path = ["`；② `md不列结构诊断全量词表` 禁 `invalidValue` / `unknownField` / `missingField` 等七词（只放 `orphanHierarchy` / `dependsOnAncestor`）；③ `骨架_指路表逐句在场` 要求「缺 `path`」+ `archi_module` + `hasPath` **同现** ⇒ 新 `path` 行必须保留「缺 `path`」这四字（原句尾「缺 `path` 看 `archi_module` 的 `hasPath`」原样留）。
13. **AC-6 按面分判（回溯修订；取代旧「`crates/**` 全零命中 + delta 命中须同行含 `MUST NOT`」口径）**：短语集 = 四条骨架 forbidden needle（`读成另一个元素` / `读成**另一**元素` / `必须内联表` / `被读成元素`）＋等价旧文（`metadata` MUST 用内联表 / 必须内联表、禁子表）。三面各立判据，**判红看该句是否把假因果陈述为事实，不看同行是否含某个词**：
    - **正向表述面（零命中）**：`crates/**` 除防线单元之外的一切（生产源码与注释、`archi-knowledge.md`、`SCAFFOLD_TEMPLATE`、测试文件内防线单元之外的注释）＋ `.dev-team/architecture/decisions/**`。实测：骨架 / 模板 / ADR 零命中；`crates/**` 的命中只落防线单元（`archi_knowledge_test.rs:338-360`）。
    - **防线面（必须在场，删了即红）**：`archi_knowledge_test.rs::骨架形态纪律四条在场` 的 `for forbidden in [四 needle]` 循环、`assert!(!source.contains(forbidden), …)` 与函数 doc comment——命中依据是**否定断言形态**（钉住「骨架零假因果」），不属待清残留，MUST NOT 因 grep 洁癖删除，亦 MUST NOT 退化成「同行含 `MUST NOT`」的文本匹配。
    - **spec 面（收窄到本 delta，只许否定形态）**：追改面 = `openspec/changes/archi-model-path-and-silent-trap/specs/**`，实测 8 处命中（`desktop-archi-agent` `:8` / `:34` / `:60` / `:129` / `:143`，`desktop-archi-model` `:8` / `:56` / `:91`），逐条读句均为否定 / 禁令语境（`MUST NOT` 或「不是」），零断言陈述。基线 `openspec/specs/**`（`desktop-archi-model:51` / `:80`、`desktop-archi-agent:184` / `:384` / `:498`）、`openspec/changes/archive/**` 与 `explore.md` **不在追改面**——delta-only 下 MUST NOT 手改基线，其命中不计红，旧句由归档合并本 delta 的 MODIFIED 块收口。
14. **`kind` 与 drift 收窄的「同批」硬耦合（R-4）**：本相位已把三个容器的 `kind` 落盘（见文末「落地面」），drift 收窄归 implement——中间态 `model.warnings` 仍为 **6**（drift 尚未读 `kind`），无回归、无单侧交付风险；两半在同一 change 交付。
15. **`path` 行的两半都必须进骨架**：spec 的 `path` 纪律 bullet 由「基数口径 + 工具指路（`hasPath`）」与「容器空 `path` 的漂移读数按 `kind` 分档」两部分组成。后者以压缩句「`domain` / `composite` 空 `path` 零告警」进骨架（约 +25 B）——它是**稳定规则**而非逐次读数（读数仍由 `archi_drift` 事后给），故不与「A 档不复述读数」冲突；压缩成本计入 D6 的同段等量删减。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 实施序 | 实现面四段：①L1 判据（`parse/toml.rs`）→ ②L3 收窄 + 词汇 doc + `bindings:export` → ③知识骨架 / 模板字面 → ④版本 + 零扩散核对。行为锚（`package` 基数四条 / `metadata` 子表等价 / 子表唯一声明）归测试相位，登记在末节交接清单；`metadata` 等价句的成立以该锚为前提（锚失败即按 OQ-3 回落到「不推荐但合法」） | L2 数据已在 design 相位落盘，L3 一落地读数即 6 → 0 一次到位（R-4）；生成物只在 ② 重导一次（R-6）；锚与文本分属测试 / 实现两相位，故不做「锚先于文本」的同相位排序假设（R-2 的落实方式是交接清单 + 回落方案） |
| D2 | L1 形状 | 私有 `validate_path(source, fqn, kind, path: &[Spanned<String>]) -> Option<Diagnostic>`；`kind == Package && path.len() >= 2` ⇒ 恰一条 `DiagnosticKind::InvalidValue`，位置 = `path[1].span()`（第 2 条），message 含「至多一条包根」并给出路（0 条 = 纯容器 / 多条 `path` 属 `module`）；`0` / `1` 条与其余三 kind 零诊断 | 复用九值词汇与 `validate_apis` 形态（零新词）；`Option` 即「至多一条」，`extend` 直接吃 `Option` 的 `IntoIterator` |
| D3 | L1 边界 | 只守上界：MUST NOT 顺手加 `module ≥ 1`、不加目录尾 `/` 形态校验、不加 `kind` 层级序校验 | 实测零误伤且不是当下痛点；扩张即超出本变更（proposal「只守上界」） |
| D4 | L3 收窄 | `drift/mod.rs` 的 `path.is_empty()` 分支内加 `if matches!(element.kind, ElementKind::Package \| ElementKind::Module)` 才 push；`continue` 对所有空 `path` 元素**保留**（不进入 path 存在性循环）；`message` 逐字不动；`use crate::dto::{self, ElementKind, GateTier};` | parse 期放行、drift 期提醒，与 L1 互补；两处 push 计数与两段定级分支零变化（事实校准 6） |
| D5 | 词汇面 | 只改 `drift/report.rs` 的两处 **doc**（枚举级总述尾句 + `NoPath` 变体行），`ALL` / `as_str` / `REPORT_VERSION` / DTO 字段集逐字不变；`bindings.ts` 由 `bindings:export` 重导，`bindings.zod.ts` 零 diff | 预期 diff 恰两处 JSDoc（AC-9）；`ElementKind` 的变体 doc **不动**——语义归知识文本与 ADR，动了即超出预期 diff |
| D6 | 知识文本 | 三处改写：键形句（未引号点分 → 整份源解析失败、诊断点名非法子键；`metadata` 子表与内联表等价）、`path` 行（模块必填可多条 / 包至多一条包根 / `domain`·`composite` 选填 + 压缩句「`domain` / `composite` 空 `path` 零告警」，尾句「缺 `path` 看 `archi_module` 的 `hasPath`」保留）、`composite` bullet（补「纯分层容器是标准形态」）；同段压缩使骨架仍落 2300–2600 B | 事实校准 12 / 15 的断言是硬门；字面统一为「子表与内联表等价」（见 OQ-1），且骨架零结构诊断词 |
| D7 | 模板三行 | `SCAFFOLD_TEMPLATE` 的 `kind` 行四值齐列（`package` 至多一条包根 / `domain` 领域容器 / `module` 模块 / `composite` 纯分组）、`path` 行按新基数（含字面 **`module 必填`**）、`metadata` 行写 **`子表与内联表等价`**；`scaffold_draft` / `__KIND__` / `cap_result` / 参数面零改动 | 两个 needle 承载串（`module 必填` / `子表与内联表等价`）即 `archi_tool_test` 的新断言面；草稿仍全注释、零结构诊断 |
| D8 | 行为锚三条 | 三条行为锚（未引号点分 → 全或无 / 子表与内联表等价 / 子表成为唯一声明即缺 kind）的用例名、拆分边界与 needle 由**末节交接清单**单点持有，本表不重复；`metadata` 等价句的正当性以其为条件（锚失败即按 OQ-3 回落） | AC-7 要求每句断言都有行为锚；「子表等价」此前只有语义直推（R-2 / OQ-3），故锚与文本分属两个相位并显式绑定 |
| D9 | 停摆断言 | `dto_test.rs` 新增**独立**用例（`package` 两条 `path` ⇒ `graph_view` 空图 + 诊断），MUST NOT 把既有「九类」账扩成「十一类」；用例名与断言面见末节交接清单 | 「九类」是 parser 五 + 自实现四的封闭账，为一条新判据扩容会让既有账失真（AC-2） |
| D10 | drift 断言 | 三条新用例按 `kind` 分档各立一条（`package` 仍报 / `module` 仍报 / `domain`·`composite` 静默）；既有 `noPath警告`（`Ghost` = `module`）逐字不动；用例名见末节交接清单 | 既有用例的 `Ghost` 正好是收窄后的仍报面（AC-3） |
| D11 | 版本 | `packages/desktop/package.json` `0.7.8` → `0.7.9`；`tauri.conf.json` 跟随 `../package.json`；`src-tauri/Cargo.toml` 版本行逐字不变 | AGENT.md 版本纪律（AC-11） |
| D12 | 零扩散白名单 | 允许出 diff：本 change 目录文稿与 4 份 delta、`02` / `03` / `05` 三个模型文件、`decisions/0001-model-granularity.md`、`parse/toml.rs`、`drift/{mod,report}.rs`、`archi-knowledge.md`、`sdk/archi_tool.rs`、`types/generated/bindings.ts`（生成物）、`package.json`；测试相位承接面（登记见末节交接清单）：`parse/toml_test.rs`、`dto_test.rs`、`drift/mod_test.rs`、`archi_knowledge_test.rs`、`sdk/archi_tool_test.rs` | 出现白名单外 diff 即回退；`scan/owner.rs` / `edit.rs` / `diff.rs` / `dto.rs` / `graph.rs` / `model.rs` / `api/**` / 前端产品代码 / `AGENT.md` 零 diff 是判据之一（AC-10） |
| D13 | 读数记账 | L2 落盘后 `modelHash` 必变（模型字节变）；`model.warnings` = 6（中间态）→ **0**（L3 落地后）；`elements` 55 / `relations` 129 / `observations` 53 / `apiFindings` 39 / `scan.*` 三项与四类结构诊断逐字段不变 | 模型字节变即 `modelHash` 变，MUST NOT 写成「零变化」（AC-4 / AC-5） |
| D14 | AC-6 判据口径（回溯修订） | **三面分判**：正向表述面零命中（`crates/**` 除防线单元 + `decisions/**`）；防线面**必须在场**（四条 forbidden needle + 其 doc comment）；spec 面收窄到本 delta 的 4 份 spec，以**语义**判否定形态；基线 `openspec/specs/**` / `archive/**` / `explore.md` 不追改（delta-only）；删禁令或删 needle 与留断言同判红 | 回溯给出的两条硬事实：①「`crates/**` 零命中」把 `archi_knowledge_test.rs` 的 `assert!(!…contains…)` 负向 needle 计入，而这些 needle 正是「骨架零假因果」的防线（删即失守 R-7）；②「delta 命中仅当同行含 `MUST NOT`」对基线不可满足（delta-only 禁手改），且会误伤 delta 内已合法的直接否定（如 `MUST NOT 把该表读成另一个元素`）——字面规则逼着把合法禁令改成带引号同义句，与防假因果回流的意图相悖。语义判据与 proposal AC-6 同口径 |

---

## 回溯修订记录（AC-6 判据口径）

**触发**：上一轮 AC-6 与本 change 的测试计划冲突且部分不可满足——(1) 旧口径要求 `crates/**` 零命中，但 `archi_knowledge_test.rs` 的 `assert!(!source.contains(…))` 列表逐字持有四条禁令短语，它们是防线而非残留（test-design「骨架 · 假因果零残留」一行要求在场）；(2) 旧口径要求 delta 内命中所在行含 `MUST NOT`，但基线 `openspec/specs/**` 在 delta-only 下不可手改，且 delta 内已有「不是『读成另一个元素』」「MUST NOT 把该表读成另一个元素」这类不带引号包裹的合法否定。

**落点**（本次返修改的面，全部是判据与文档面，零实现面改写）：

| 面 | 落点 |
|---|---|
| 判据 | 事实校准 13 重写为三面分判；D14 记录定稿与理由；AC-6 回应行改写 |
| 持有者表 | 「AC-6 的九处持有者与落点」逐处标注落面（正向面 / 防线面 / spec 面），第 7 行拆成「迁移已完成（正向面）+ 四 needle 保留（防线面）」 |
| 交接清单 | 第 4 行补「四条 forbidden needle 与其 doc comment 保留」；用例名与体量锚冻结值按落地实测回填 |
| `tasks.md` | 阶段三自查的第一条 grep 改为正向面列举式；新增阶段五「AC-6 三面复核」（只读 grep，不跑测试、不改文件） |

**未变**：`parse/toml.rs` / `drift/{mod,report}.rs` 的判据与 doc、三个模型文件的 `kind` 行、ADR §2 / §3、知识骨架与模板字面、用例名与 needle 迁移、两个体量锚——AC-6 是**判据口径**的改写，不触碰已落地的实现与断言。

**反向锚不可删**：删掉骨架侧的防线 needle 或 delta 的禁令句，与留下断言同判红（R-7——两侧否定锚是防假因果回流的唯一把手）。

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 格式判定（改） | `package` 的 `path` 上界：≥2 条出带位置的 `invalidValue`；0 / 1 条与其余 kind 放行 | `crates/core/archi/src/parse/toml.rs` | `toml` / `serde` | `Spanned` 取 span + 既有 `line_of` / `column_of` |
| 停摆收口（零改） | `diagnostics` 非空 ⇒ `elements` / `relations` 清空（既有唯一分支） | `crates/core/archi/src/dto.rs` | — | 纯投影 |
| 归属判定（改） | `noPath` 入面按 `kind` 收窄：`package` / `module` 仍报，`domain` / `composite` 静默 | `crates/core/archi/src/drift/mod.rs` | `crate::dto`（`ElementKind`） | 既有 `matches!` 分支 |
| 漂移词汇（改 doc） | `NoPath` 变体 doc + 枚举级总述的 `noPath` 括注同批改写（两处经 specta 落 `bindings.ts`） | `crates/core/archi/src/drift/report.rs` | `specta` / `serde` | 封闭枚举 + `ALL`（零变化） |
| 知识骨架（改） | `path` 基数句 / 键形真因果句 / `composite` 标准形态句；体量锚同批实测 | `crates/infra/agent/src/archi-knowledge.md` | `include_str!` + 派生目录段 | 静态 markdown |
| 模板工具（改） | `SCAFFOLD_TEMPLATE` 三行注释（`kind` 四值 / `path` 新基数 / `metadata` 等价） | `crates/infra/agent/src/sdk/archi_tool.rs` | `archi::dto::ElementKind::ALL` | 字面串 + 单点替换 |
| 基线数据（改，已落盘） | 三容器 `kind` → `composite`；ADR §2 指向 `composite`、§3 两组语义成对 | `.dev-team/architecture/graph/model/{02,03,05}-*.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md` | — | TOML / markdown |

依赖方向零新增边：`core/archi` 仍只依赖 `foundation`；`infra/agent` → `archi` 既有边不变；`kind` 的新消费点只有一个（`drift` 读 `ElementKind`，同 crate 内引用）。零新 crate 依赖。

### 数据流（改后）

```text
graph/model/*.toml ──parse_dir──> TomlParser::parse ──to_model──>
    Element { kind, path, … }            逐元素 validate_path：
                                         package ∧ path.len() ≥ 2 ⇒ invalidValue（位置 = path[1]）
                                         ↓ 元素照常入模型、诊断累积（不停摆）
    graph_view: diagnostics 非空 ⇒ elements = [] / relations = []（既有唯一停摆分支）
                                         ↓
    drift::check → ④ 模型警告：path.is_empty() ∧ kind ∈ {package, module} ⇒ noPath
                                     （domain / composite 静默）
                                         ↓
    report.model.warnings（本仓 6 → 0）/ scan / observations / apiFindings（逐字段不变）
```

---

## 数据模型 (Data Model)

**`path` 的基数随 `kind`（唯一新判据，持久化面 = 模型 TOML 文件本身）**：

| `kind` | `path` 基数 | 本仓实测（落地后） | 越界行为 |
|---|---|---|---|
| `package` | **≤ 1**（0 条 = 纯容器；1 条 = 包根目录） | 19 个恰 1 条、3 个 0 条（容器，迁 `composite` 后） | ≥2 条 ⇒ `invalidValue`（位置 = 第 2 条）+ 整图停摆 |
| `module` | 不校验（若干根的清单） | 30 个（含唯一的 5 条 `Desktop.Shell`） | 无 |
| `domain` / `composite` | 不校验（选填） | 3 + 3 个，全 0 条 | 无 |

**`kind` 分布（同一份模型数据，无新增 / 删除元素）**：

| 值 | 变更前 | 变更后 | 说明 |
|---|---|---|---|
| `package` | 22 | **19** | 三个纯分层容器迁走 |
| `composite` | 0 | **3** | `Backend.Core` / `Backend.Infra` / `Backend.Tools`（`path` 仍空） |
| `domain` | 3 | 3 | `Desktop.Views` / `Desktop.Components` / `Desktop.Components.Standard` |
| `module` | 30 | 30 | 含 `Desktop.Shell`（5 条 `path`）与目录模块 |
| 合计 | 55 | **55** | 关系 129 不变 |

**持久化**：无数据库、无迁移——模型是 TOML 文件（`.dev-team/architecture/graph/model/*.toml`，六个一级文件），裁定记录是 markdown（`decisions/0001-model-granularity.md`）；本变更只改 3 行 `kind` 与 ADR 两段。

---

## 变更清单 (Change Inventory)

> **覆盖口径**：`crates/**` / `src/**` 均指 `packages/desktop/src-tauri/` 之下；`packages/desktop/src/**` 指前端。本表只持**实现文件**——proposal「变更范围 - 实现文件」全量 **9 行逐行覆盖**（模型文件行按 `02` / `03` / `05` 三个文件展开为三行），另加生成面登记（`INDEX.toml` / `.schema.json` / `.i.*` 零 diff、`bindings.zod.ts` 零 diff）。proposal「测试文件」表的 5 行**不进本清单**（进清单即要求 `tasks.md` 有对应任务，与「tasks 不写测试任务」互斥），其用例名 / needle / 拆分边界 / 体量锚冻结值由末节「交接给下游相位的随动清单」单点持有。

<!-- 新增文件（New Files）：本变更零新增文件——判据落在既有单点（`to_model` / `check` / 模板常量），行为锚落在既有测试文件内（MUST NOT 新建测试文件以保 `scan.*` 计数不变，R-10；随动面见末节交接清单，不入本表）。 -->

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/parse/toml.rs` | ① 元素循环内（`to_model`，`:101`）先算 `fqn` 再 `diagnostics.extend(validate_path(source, &fqn, raw_element.kind, &raw_element.path));`，然后照常 `elements.push`；② 新增私有 `fn validate_path(source: &str, fqn: &str, kind: ElementKind, path: &[::toml::Spanned<String>]) -> Option<Diagnostic>`：`kind != Package \|\| path.len() < 2` ⇒ `None`；否则取 `path[1].span()` 出 `DiagnosticKind::InvalidValue`，message 逐字「元素 {fqn} 是 package：一个包至多一条包根目录（纯容器可留空）——多条 path 属 module」；③ 模块 doc 补一句「`path` 的基数随 `kind`：`package` 至多一条包根」；④ `RawElement` / `RawDocument` / `validate_apis` / `classify` / `line_of` / `column_of` / `parse` 逐字不变 | D2 / D3；`ElementKind` 已在 `use` 面内（`:11`），零新 import |
| `crates/core/archi/src/drift/mod.rs` | ① `use crate::dto::{self, GateTier};` → `use crate::dto::{self, ElementKind, GateTier};`；② `:114` 分支内加 `if matches!(element.kind, ElementKind::Package \| ElementKind::Module) { model_warnings.push(...) }`，`continue` 保持在分支外对所有空 `path` 元素生效；③ `message` 字符串、条目字段、`pathNotFound` 段、归属 / 定级 / 豁免 / `derive_status` / `passed` 逐字不变；④ 模块 doc 的 ④ 段补「按 `kind` 分档」 | D4；定级面三处 push 计数不变（事实校准 6） |
| `crates/core/archi/src/drift/report.rs` | ① `:88` 枚举级总述尾句「`noPath`（元素未声明 path）」→「`noPath`（`package` / `module` 未声明 `path`；`domain` / `composite` 空 `path` 是合法形态）」；② `:101` 变体注释「元素未声明 `path`（不参与归属）」→「`package` / `module` 未声明 `path`（不参与归属；容器空 `path` 是合法形态）」；③ `ArchiDriftKind` 五值 / `ALL` 序 / `as_str` / `REPORT_VERSION` / DTO 字段集逐字不变 | D5；两处经 specta 落 `bindings.ts` 的 JSDoc |
| `.dev-team/architecture/graph/model/02-backend-core.toml` | `Backend.Core` 的 `kind` `"package"` → `"composite"`（1 行）；`path` 仍空；表头 `9 元素 / 10 关系` 与其余 8 元素逐字不变 | D13；**本相位已落盘** |
| `.dev-team/architecture/graph/model/03-backend-infra.toml` | `Backend.Infra` 的 `kind` `"package"` → `"composite"`（1 行）；表头 `8 / 25` 与其余 7 元素逐字不变 | 同上 |
| `.dev-team/architecture/graph/model/05-backend-tools.toml` | `Backend.Tools` 的 `kind` `"package"` → `"composite"`（1 行）；表头 `3 / 3` 不变（两个工具 crate 仍 `package`） | 同上 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | ① §2 末段：`core` / `infra` / `tools` 由「分层容器同为 `package`」改为「**纯分组容器**（`Backend.Core` / `Backend.Infra` / `Backend.Tools`）`kind = "composite"`（无层级、不持文件，空 `path` 正是标准形态）」，`package` 段补「`path` 至多一条包根」，删「`composite` 本仓当前无实例」；`module` 段补「`path` 必填、可多条」；② §3 首段：六个容器拆成两组（**`composite` 纯分组组** = `Backend.Core` / `Backend.Infra` / `Backend.Tools`；**`domain` 业务聚合组** = `Desktop.Views` / `Desktop.Components` / `Desktop.Components.Standard`），`path` 选填口径保留 | D13 / AC-13；**本相位已落盘**（全文「同为 `package`」「本仓当前无实例」零命中） |
| `crates/infra/agent/src/archi-knowledge.md` | ① `:5` 键形句：删「点分嵌套与子表读成**另一元素**（`metadata` 必须内联表）」，改为「未引号点分 `[element.App.Auth]` 使**整份源解析失败**（诊断点名非法子键）；`metadata` 子表与内联表等价（子表写在已声明元素之后零诊断，成为唯一声明才缺 `kind`）」；② `:10` `path` 行改基数口径（模块必填可多条 / 包至多一条包根（纯容器可留空，≥2 条即模型停摆）/ `domain`·`composite` 选填）+ 压缩句「`domain` / `composite` 空 `path` 零告警」，**尾句「缺 `path` 看 `archi_module` 的 `hasPath`」原样保留**；③ `package` bullet 补「至多一条包根」；④ `:24` `composite` bullet 补「纯分层容器是标准形态」；⑤ 同段压缩等量冗余（合并自检 / 边界两段的重复短句），使骨架净增长后仍落 2300–2600 B；零结构诊断词、零示例围栏 / `path = ["`、零判据长表 | D6；事实校准 12 / 15 的断言是硬门 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | `SCAFFOLD_TEMPLATE`（`:198`）三行：① `kind` 行四值齐列（`package` = 包（至多一条包根 `path`，纯容器可留空）/ `domain` = 领域容器（`path` 选填）/ `module` = crate、目录型模块、视图或单文件组件（`path` 必填、可多条）/ `composite` = 纯分组（无层级，可嵌套任意类型之上；纯分层容器是标准形态））；② `path` 行改「**module 必填**（可多条 = 若干根的清单）；package 至多一条包根（纯容器可留空；≥2 条即模型停摆）；domain / composite 选填…」；③ `metadata` 行改「**子表与内联表等价**（`[element."X".metadata]` 写在已声明元素之后零诊断；成为唯一声明才缺 kind）」；`scaffold_draft` / `__KIND__` / `scaffold_payload` / 参数与响应形状逐字不变 | D7；两处 needle 承载串 |
| `packages/desktop/src/types/generated/bindings.ts`（`.zod.ts` 按生成结果） | `pnpm bindings:export` 重导；预期 diff = `NoPath` 两处 JSDoc（枚举级总述 + 变体行）；`bindings.zod.ts` 零 diff | **生成物，勿手改**（R-6） |
| `packages/desktop/package.json` | `"version"`: `0.7.8` → `0.7.9` | D11 |

<!-- 配置（Config）：本变更零配置键改动——`.dev-team/architecture/config.json` 仍只有 `exclude`（`openspec/**`）、不新增 `ignore` 规则（`kind` 迁移与 `path` 判据都不产生新的元素对豁免需求）；工作区 `KNOWN_KEYS` / `config.schema.json` 镜像零改动（判据属架构模型面，不是工作区配置面）。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `TomlParser::parse`（`SourceParser` 实现） | `crates/core/archi/src/parse/toml.rs` | 改（内部新增一条判据） | `fn parse(source: &str) -> ParseOutcome` | 签名与 `ParseOutcome` 形状逐字不变；诊断来源由「serde + `apis` 值面」扩为「serde + `apis` 值面 + `path` 基数」，失败臂仍 `Model::empty()` |
| `validate_path` | 同上 | **新增（私有）** | `fn validate_path(source: &str, fqn: &str, kind: ElementKind, path: &[::toml::Spanned<String>]) -> Option<Diagnostic>` | 私有函数（不进公共面，登记以钉住形状）；`package` ∧ ≥2 条 ⇒ 一条带位置的 `invalidValue`，其余 `None` |
| `check` | `crates/core/archi/src/drift/mod.rs` | 改（仅 ④ 分支） | `pub fn check(input: CheckInput<'_>) -> Result<Report, CheckError>` | 签名与其余段逐字不变；`noPath` 入面从「所有空 `path` 元素」收窄为「`package` / `module` 空 `path`」 |

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `ArchiDriftKind`（含 `NoPath` 变体） | `crates/core/archi/src/drift/report.rs` | 修改（**仅 doc**） | 变体集仍五值、`ALL` 序、`as_str`、线格式词 `noPath` 逐字不变；两处 doc 改写经 specta 落 `bindings.ts` JSDoc |
| `DiagnosticKind` | `crates/core/archi/src/dto.rs` | 零改动 | 仍九值；`invalidValue` 的语义面（值不在封闭值域 / 字面形态违例）被**扩用一次**，零新变体 |
| `ElementKind` | 同上 | 零改动 | 四值、`ALL`、`as_str` 与**变体 doc 一律不动**（动了会让 `bindings.ts` diff 超出「两处 JSDoc」，D5） |

---

## 路由 / API 设计 (Route / API Design)

<!-- 不涉及：本变更零新增 / 删除 IPC 命令，零 HTTP 端点，零 DTO 字段变化。九条 `archi_*` 命令的入参、出参、错误面与 auth 概念（无）逐字不变；新判据只让坏模型更早进入既有停摆分支（`archi_graph` 返空图 + 诊断清单、`archi_module` / `archi_drift` 返 `Err`），`archi_drift` 的唯一读数是 `model.warnings` 6 → 0。 -->

---

## 依赖 (Dependencies)

### 运行时依赖

零新增。`crates/core/archi` 既有依赖即够用：`toml`（`Spanned` 取第 2 条 `path` 的 span）、`serde` / `specta`（DTO 与词汇 doc）、既有 `foundation`（取消标志 / 布局）。`crates/infra/agent` 零新增（模板是字面串、知识是 `include_str!`）。前端零新包。

### 构建 / 测试依赖

零新增。复用既有脚本：`pnpm bindings:export`（两份生成物 + zod 自检）、`pnpm api:export` / `pnpm api:check`（本变更预期零 diff）、`pnpm check`、`pnpm test`。语料面与门禁面零新工具、零新语料 case。

---

## 验收标准回应

| AC | 设计落点 |
|---|---|
| AC-1 | D2 / D3 + 末节交接清单第 1 行的四条 `path` 基数用例：`package` 2 条 ⇒ 恰一条 `invalidValue`、行号 = 第 2 条（`line_at`）、文案含「至多一条包根」；`package` 0 条与 `module` / `domain` / `composite` 2 条 ⇒ 零诊断；反向：`package` 1 条（本仓 19 个）与 `module` 5 条（`Desktop.Shell`）全绿（事实校准 8 的静态复算为零误伤的证据） |
| AC-2 | 事实校准 3 + D9：停摆仍由 `dto::graph_view` 的唯一分支承担，`dto_test` 新增**独立**用例（末节交接清单第 2 行）；既有「九类」表与名字逐字不动 |
| AC-3 | D4 / D10：`domain` / `composite` 零 `noPath`、`package` / `module` 各一条；`errors` / 顶层 `warnings` 不受影响；`ArchiDriftKind::ALL` 仍五值、`model_warnings.push(` 仍两处（事实校准 6） |
| AC-4 | D13 + 落地面读数：元素 55 / 关系 129、四类结构诊断零命中（`graph.rs` 不读 `kind`）、`INDEX.toml` / `.schema.json` / `.i.*` 零 diff；`kind` 分布 19 / 3 / 3 / 30 |
| AC-5 | `repo-archi-model` delta 三处 `model.warnings` 一致为**空**；`docMissing` 39 / `observations` 53 / `status = clean` / `passed = true` 与 55 / 129（后端 52 + 前端 77）逐字不变——本变更零计数变化，上一变更的增量只作历史事实（R-11） |
| AC-6 | D6 / D7 / **D14** + 事实校准 13 + 九处持有者逐处落点（见下表）：**三面分判**——① 正向表述面（`crates/**` 除防线单元、`decisions/**`）零命中（骨架 / 模板 / ADR 实测零命中）；② 防线面（`archi_knowledge_test.rs::骨架形态纪律四条在场` 的四条 forbidden needle + 其 doc comment）**必须在场，删了即红**；③ spec 面收窄到本 delta 的 8 处命中，逐条按**语义**归为否定 / 禁令，零断言（引号包裹不是必要条件）；基线 `openspec/specs/**` / `archive/**` / `explore.md` 不计红、MUST NOT 手改。**反向**：删禁令句或删防线 needle 与留断言同判红（R-7） |
| AC-7 | D8 + 末节交接清单第 1 / 4 / 5 行：`parse/toml_test.rs` 恰三条行为用例（`未引号点分表头被拒且不与合法形态合并` / `metadata子表与内联表等价` / `metadata子表成为唯一声明即缺kind`）；断言「md 含某词」的骨架用例已更名 `骨架形态纪律四条在场`（不再以「静默」为名） |
| AC-8 | D6 / D7：骨架含基数句 + `composite` 标准形态句 + 键形真因果句且零结构诊断词（`unknownField` 不进骨架）；模板三行按新口径；`archi_tool_test` 遍历 `ElementKind::ALL`；两个体量锚按实测更新且区间 / 门限不抬（R-5）；needle 与冻结值见末节交接清单第 4 / 5 行 |
| AC-9 | D5：`bindings:export` 后 `git diff` 只含 `bindings.ts` 的两处 `NoPath` 注释（`.zod.ts` 零 diff）；`api:check` 零差异退出；`api:export` 零 diff（事实校准 10） |
| AC-10 | D12 白名单：改动集 ⊆ proposal「变更范围」，ADR 必在改动集内（已落盘）；语料 20 份逐字节不变（事实校准 9）；`scan.*` 三项与基线同（422 / 359 / 422 / 0 / 0；本变更零新增 / 删除文件 ⇒ 三面逐字段不变）；前端产品代码零改动 |
| AC-11 | D11：`0.7.9`；`tauri.conf.json` 仍 `"../package.json"`；`src-tauri/Cargo.toml` 版本行不变 |
| AC-12 | 4 份 delta 共 **7 条** `### Requirement` 标题与基线逐字一致（本次已复算：`架构模型文件格式与字段封闭` / `文件归属与两套解析器一套归属逻辑` / `archi_scaffold 只读工具的新元素模板契约` / `架构会话的系统知识注入` / `知识分层与工具面同批移动` / `基线落位与模型文件划分` / `图自洽与停摆禁令`）；每条至少一个 `#### Scenario:`——由 spec 面承接，本设计不改 delta |
| AC-13 | §2 / §3 已按判据改写（已落盘）：§2 有「纯分组容器 + 三容器指称 + `kind = "composite"`」同句、无「分层容器 × `package`」并置；§3 两组语义与成员成对；全文「同为 `package`」「本仓当前无实例」零命中；与 `02` / `03` / `05` 的实测 `kind` 一致 |

**AC-6 的九处持有者与落点（按面分判，D14）**——短语集见 proposal AC-6；逐处标注落面（**正向面** = 必须零命中 / **防线面** = 必须在场 / **spec 面** = 只许否定形态）：

| # | 持有者 | 落面与落点 |
|---|---|---|
| 1 | `desktop-archi-agent` 知识注入 · 字段形态与键形段 | **spec 面**：已改写为「未引号点分 → 整份源解析失败；子表与内联表等价」，原假因果只余 `MUST NOT` 否定句 |
| 2 | `desktop-archi-agent` 知识分层 · 补充规则① | **spec 面**：已改写为「形态错误不静默、但发生在写盘之后」并点名三条行为测试；旧表述只余 `MUST NOT` 禁令 |
| 3 | `desktop-archi-model` 字段面封闭条 | **spec 面**：已改写为「语义等价 + 唯一声明即 `missingField`」，旧因果只余 `MUST NOT` 否定 |
| 4 | `desktop-archi-model` Scenario | **spec 面**：拆为「子表与内联表等价」「子表成为唯一声明即缺 kind」两条，THEN 用 `MUST NOT` 否定形态 |
| 5 | `archi-knowledge.md`「位置与键形」 | **正向面**：已改写（真因果 + 等价句），零命中 |
| 6 | `SCAFFOLD_TEMPLATE` 的 `metadata` 注释 | **正向面**：已改写为「子表与内联表等价」，零命中 |
| 7 | `archi_knowledge_test.rs::骨架形态纪律四条在场` | **两面分裂**：用例名与主 needle 迁移属**正向面**（已完成：原名去「静默」、主 needle 换 `子表与内联表等价`）；四条 forbidden needle 与函数 doc comment 属**防线面，必须保留**（`assert!(!source.contains(…))` 形态） |
| 8 | `archi_tool_test.rs` 的 `内联表` needle | **正向面**：已换为 `子表与内联表等价`（模板断言面） |
| 9 | `parse/toml_test.rs` 的用例名与注释 | **正向面**：已拆名（去「静默成元数据」）+ 注释去假因果 |

> 归面口径见 D14：**判红看该句是否把假因果陈述为事实**，不看同行是否含某个词。基线 `openspec/specs/**` / `archive/**` / `explore.md` 的命中不在本变更追改面（delta-only），由归档合并本 delta 的 MODIFIED 块收口——出现的旧句 MUST NOT 被当作「残留」去手改基线。

> **测试面交接**（本相位不写用例、不跑测试）：proposal「测试文件」表 5 行归测试相位，**不进「变更清单」**——本设计只钉住**必改机械面**（用例名 / needle / 拆分边界 / 体量锚冻结值），逐行见末节「交接给下游相位的随动清单」；「等价」句与行为锚的绑定关系（R-2 / OQ-3）见 D1 / D8。

---

## 落地面（本相位已落盘：模型 `kind` 3 行 + ADR §2 / §3）

**为什么在 design 相位落**：`<archi_dir>/graph/model/**` 受会话级写入禁区（相位锁）约束，只允许 proposal / design 相位或其架构页会话写入；而 AC-4 / AC-5 / AC-13 要求「数据 + ADR + 判据」同批——数据落盘归本相位，代码收窄（L3）归 implement。中间态无回归：drift 当前不读 `kind`，`model.warnings` 仍 **6**（= 基线），`kind` 只是数据。

| 面 | 落盘后实测 |
|---|---|
| `archi_drift(full)` | `elements` **55** / `relations` **129** / `byGate.off` 55 / `structuralErrors` **[]** / `modelWarnings` **6**（全 `noPath`，`Backend.Core` / `Backend.Infra` / `Backend.Tools` / `Desktop.Components` / `Desktop.Components.Standard` / `Desktop.Views`）/ `observations` **53** / `apiFindings` **39**（全 `docMissing`）/ `errors` / `warnings` **[]** / `status = clean` / `passed = true` / `scan` = 422 / 359 / 422 / 0 / 0 |
| `archi_module("Backend.Core")` | `kind = "composite"`、`path = []`、`modelFile` `02-backend-core.toml`、`modelLine` 4、`children` 8 条、出边 / 入边空 |
| 模型文件 | `kind` 分布 **`package` 19 / `composite` 3 / `domain` 3 / `module` 30**；`02` / `03` / `05` 的表头计数与元素条目数不变 |
| ADR | §2 末段有「**纯分组容器**（三容器）`kind = "composite"`」同句、无「分层容器 × `package`」并置；§3 首段两组语义与成员成对；全文「同为 `package`」「本仓当前无实例」零命中 |
| `modelHash` | `sha256:05b61bde…` → **`sha256:6cc50dbf…`**（模型字节变，如实记录；MUST NOT 写成「零变化」） |

**L3 落地后应成立**：`model.warnings` **6 → 0**（六个容器 = 3 `composite` + 3 `domain` 全部静默），其余读数逐字段同上。

**L3 落地后的读数归属**：`model.warnings` = 0 由 `drift/mod.rs` 的 `matches!(element.kind, ElementKind::Package | ElementKind::Module)` 收窄 + `drift/mod_test.rs` 的分档用例（`package空path仍记模型警告` / `domain空path零模型警告` / `composite空path零模型警告` / `四kind空path只有package与module记警告`）确认；桌面 `archi_drift` 读数依赖宿主二进制，`reports/archi/*.json` 三份仍显示 6 条 `noPath`（旧二进制产物），MUST NOT 据此判 L3 未落地。

---

## 待决问题 (Open Questions)

1. **「内联表与子表等价」与「子表与内联表等价」的语序**：`desktop-archi-agent` delta 的 Scenario 写「`metadata` 的「内联表与子表等价」两句以注释行在场」，而 proposal「测试文件」行与 delta 的 Module Contract 写「`子表与内联表等价`」。本设计的落定：**产物字面统一为「子表与内联表等价」**（知识骨架与 `SCAFFOLD_TEMPLATE` 各一处），测试的 needle 同此串；Scenario 里的「内联表与子表等价」读作同一陈述的描述性表述，不另写字面。若评审要求 Scenario 的字面也成立，需在模板同一注释行并列两种语序（本设计不取——同一事实两种自称）。
2. **`domain` 组「空 `path` 即纯分组」的旧措辞**：ADR §3 原句让 `domain` 与 `composite` 共用「不填即纯分组」，易被读成两者同义（R-8）。本设计改写为「本基线六个容器全部留空（`composite` 组空 `path` 就是它的形态本身，`domain` 组空 `path` 表示当前不持文件）」，语义靠文字而不靠机械判据（`kind` 判定仍是架构判断）。若评审要求更强约束，须另立机械判据（超出本变更）。
3. **`metadata` 子表与内联表的等价仍是「TOML 语义直推」**：本设计按 R-2 把「`metadata` 子表与内联表等价」这句与测试相位的行为锚（`metadata子表与内联表等价`，见末节交接清单第 1 行）显式绑定；若该锚在测试相位意外失败，回落方案是只保留「未引号点分 → `unknownField` 全或无」的纠正，把 `metadata` 子表降级为「不推荐但合法」并重新实测——MUST NOT 在无行为锚时把等价写进 spec。

---

## 交接给下游相位的随动清单（非本相位改动）

> 本节承接 proposal「测试文件」表的 5 行，**不进「变更清单」也不进 `tasks.md`**：进变更清单即要求 `tasks.md` 有对应任务，而测试任务不属本相位（本相位只写实现）。本表只登记**必改机械面**——用例名 / needle / 拆分边界 / 体量锚冻结值，供测试相位逐条落地；本相位不改、不跑任何用例。绑定关系（R-2 / D1 / OQ-3）：「`metadata` 子表与内联表等价」这句的正当性以本表第 1 行的 `metadata子表与内联表等价` 行为锚为条件——锚在测试相位失败，即按 OQ-3 把骨架 / 模板的「等价」改为「不推荐但合法」。

| 文件 | 随动面（用例名 / needle / 拆分边界 / 冻结值） |
|---|---|
| `crates/core/archi/src/parse/toml_test.rs` | ① **新增** `metadata子表与内联表等价`（内联表 vs 子表两份模型：零诊断、`metadata` 键值逐项相同、元素集零 `…metadata` 元素）；② 既有 `metadata非字符串值被拒且子表形态不静默成元数据` **拆分**为 `metadata非字符串值被拒`（非字符串两例，断言逐字留）与 `metadata子表成为唯一声明即缺kind`（`missingField` + 全或无；注释去「被读成元素 `App.Auth.metadata` 的父表」假因果）；③ **新增**四条 `path` 基数用例：`package两条path被拒且位置命中第二条`（`single_diagnostic` + `line_at` 断言行号 = 第 2 条 + 文案含「至多一条包根」）、`package零条path合法`、`module多条path合法`（2 条 + 5 条 `Desktop.Shell` 形态）、`domain与composite多条path合法`；④ 既有 `未引号点分表头被拒且不与合法形态合并`（`unknownField` + 全或无）**逐字不动**——它是 AC-7 三条行为锚的第一条 |
| `crates/core/archi/src/dto_test.rs` | **新增独立**用例（`package` 写两条 `path` ⇒ `graph_view` 的 `elements` / `relations` 皆空、`diagnostics` 非空），钉住「MUST NOT 改成跳过元素」；落地名 `graph_view包根越界停摆`（另有「不与健康元素共存」「诊断 `file` 回填到出错文件」两条边界）；既有 `graph_view九类结构错一律停摆`（`:216`）的名字与表体**逐字不动**（MUST NOT 扩容成「十一类」，D9） |
| `crates/core/archi/src/drift/mod_test.rs` | 三条新用例：`package空path仍记模型警告`、`domain空path零模型警告`、`composite空path零模型警告`（各含 `errors` 空 + `status` 不变）；另有 `四kind空path只有package与module记警告` 钉住分档全矩阵；既有 `noPath警告`（`:906`，`Ghost` = `module`）与 `:3476` 的源文本计数断言（`model_warnings.push(` 2 / `errors.push(` 3 / `warnings.push(` 6）**逐字不动**（D10） |
| `crates/infra/agent/src/archi_knowledge_test.rs` | ① `骨架静默陷阱四条在场`（`:340`）**改名**去「静默」（落地名 `骨架形态纪律四条在场`）+ needle `内联表` → `子表与内联表等价`（**MUST NOT 用 `unknownField` 作 needle**——`md不列结构诊断全量词表` 禁该词进骨架）；同文件 `:334` 段注释同步；② **四条 forbidden needle 与函数 doc comment 保留**（`for forbidden in [四 needle]` + `assert!(!source.contains(forbidden), …)`）——它们是 AC-6 的**防线面**（D14），MUST NOT 因 grep 洁癖删除、MUST NOT 退化成同行文本匹配；③ **体量锚冻结值** `骨架体量锚` / `全文体量锚` 按实测更新（变更前 **2459 B / 2602 B**；落地实测 **2596 B / 2739 B**，LF 归一），区间 2300–2600 / 2450–2750 与门限 2700 / 2850 **不抬高**；④ `骨架_指路表逐句在场` 的十行表**逐字不动**（「缺 `path`」仍在场） |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | ① `archi_scaffold三kind逐值`（`:987`）→ **落地名 `archi_scaffold四kind逐值`**，遍历 `ElementKind::ALL`（`kind.as_str()` 派生，不手抄四值）；② `archi_scaffold省略kind取domain且字段序齐备`（`:961`）的 needle：`叶元素必填` → `module 必填`、`内联表` → `子表与内联表等价`；③ 模板 `kind` 行四值语义断言（落地名 `archi_scaffold模板kind行四值语义`：`package` 至多一条包根 / `domain` 领域容器 / `module` 模块 / `composite` 纯分组）、`path` 行出路（`archi_scaffold模板path行出路`）与零结构诊断词（`archi_scaffold草稿零结构诊断词`） |

> 该 5 行与「修改文件」表**互不重叠**：表内 11 行是生产面（含生成物与模型 / ADR 数据），本节 5 行是断言面；两处文件集不相交，`git status` 的改动集判据（D12 / AC-10）取两者的并集。
