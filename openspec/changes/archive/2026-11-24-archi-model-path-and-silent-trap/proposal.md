# 提案: archi-model-path-and-silent-trap

> **变更**: archi-model-path-and-silent-trap（架构模型 · `kind` × `path` 判据 + 「静默陷阱」理由纠偏）
> **日期**: 2026-11-24
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇——§一 / §二 两处文本漂移（`package` 的 `path` 与所谓「静默陷阱」）、§七–§十二 的 `path` 判据终裁（L1 / L2 / L3）、§二.5 与 §四 的纠偏修法。explore §12.2 曾把变更拟名为 `archi-kind-path-discipline` 并声明「case B 不含」；本变更目录名以「path + silent trap」命名，故**把 case B 一并纳入**，两处漂移的共同根因（推断当事实写进规格）正好由同一个变更收口。
> **性质**: **判据新增 1 处（`package` 的 `path` 上界）+ 判据语义收窄 1 处（`noPath` 按 kind）+ 数据面 `kind` 迁移 3 行 + 文本面假因果收口 9 处**——零新增结构诊断词、零新增漂移词、零 IPC / UI 变化、零契约面变化。唯一读数变化是 `model.warnings` **6 → 0**；唯一分布变化是 `package` 22 → 19、`composite` 0 → 3；元素 55 / 关系 129 / `status = clean` / `passed = true` 逐字不变。
> **不含**: L4（`module` 多 `path` × 契约面只认 `path.first()`，explore §9.6 / §10.3）；「凡以静默为由留散文的句子 MUST 附行为测试」的元纪律入 spec（explore §六.1）；`metadata` 子表的推荐形态改向（explore §六.2）；`kind` 层级序校验（explore §8.4.3）。

---

## 问题

### 一 · case A：`package` 的 `path` 在五处文本里各说一套

同一事实「元素要不要写 `path`」在五个持有者里给出五种说法：三处只对一半（缺上界，或用「仅作分组的包」这种别扭概念），两处直接说错：

| # | 持有者 | 现文 | 判定 |
|---|---|---|---|
| ① | `desktop-archi-model` 值域句 | `path` SHALL 为字符串数组（**缺省空**），不强制 | 正确但**没有上界** |
| ② | `repo-archi-model` 基线 `path` 兜底条 | 模块 / 包必填，`domain` / `composite` 与**仅作分组的包**选填 | 正确但「仅作分组的包」是别扭的复合概念 |
| ③ | `desktop-archi-agent` kind 语义段 | `package` = 包（…；包根与持有文件的包**可带** `path`） | 语义正确、缺上界 |
| ④ | `archi-knowledge.md` 字段纪律 | 「模块 / **包必填**」 | **错**（把纯容器也要求了） |
| ⑤ | `archi_scaffold` 的 `SCAFFOLD_TEMPLATE` | 「package = 包根（**无 path**）」＋「容器（package / domain）**恒空**」 | **错**（把「包根」定义成「没有根的包」） |

实测分布（`.dev-team/architecture/graph/model/*.toml`，55 元素）：`package` **22 个**，其中 19 个带 1 条目录型 `path`、3 个（`Backend.Core` / `Backend.Infra` / `Backend.Tools`）空 `path`；唯一的多条 `path` 元素是 `module` 的 `Desktop.Shell`（5 条）。`kind` 在 `crates/core/archi` 里除 `as_str` / `ALL` 外**零语义分支**，`scan/owner.rs` 不读 `kind`——基数规律强到可当判据，但代码零校验。

另一侧早已有单值假设：契约面 `element.path.first()` 是元素根，其余条目静默不可见（`api/paths.rs`、`api/check.rs`、`api/extract/mod.rs`；`check_test.rs` 还以源码断言钉住「只剩一处消费」）。**「`package` 至多一条包根」不是新约束，而是把契约面已有的暗假设在模型面点亮**。

### 二 · case B：「静默陷阱」的结论成立，理由不成立

`desktop-archi-agent` 的 C 档补充规则①（与同能力的键形段）声称：`metadata` 写子表、`[element.App.Auth]` 点分嵌套「会被解析成**另一个元素**（`deny_unknown_fields` 不触发），工具无法在动作前拦住」。实测三形态：

| 形态 | 实测 | 行为锚 |
|---|---|---|
| `[element.App.Auth]`（未引号点分） | `unknownField`，文案点名非法子键 `Auth`，**整份源全或无** | `parse/toml_test.rs::未引号点分表头被拒且不与合法形态合并` |
| `[element."App.Auth".metadata]` 作为唯一声明 | `missingField`（该 FQN 缺 `kind`），不产出元素 | `parse/toml_test.rs::metadata非字符串值被拒且子表形态不静默成元数据` |
| `[element."X"]` + `[element."X".metadata]`（子表挂在已声明元素后） | **零诊断、语义正确**——TOML 内联表与子表等价 | 无测试（`explore.md` §2.2 记「TOML 语义直推」）；本变更 MUST 补行为测试 |

即：真正静默的形态不存在，**「`metadata` MUST 用内联表」这条禁令本身是错的**。该假因果在仓内共有 **9 处持有者**（explore 只点了 4 处，本提案按全仓 grep 补齐）：

| # | 持有者 | 形态 |
|---|---|---|
| 1 | `desktop-archi-agent` 知识注入 · 字段形态与键形段 | 「会被读成**另一个元素**」「`metadata` MUST 用内联表」 |
| 2 | `desktop-archi-agent` 知识分层 · 补充规则① | 「静默错误必须留上下文」整条 |
| 3 | `desktop-archi-model` 字段面封闭条 | 「子表与『子元素表头』同构，会被读成元素 `X.metadata`」 |
| 4 | `desktop-archi-model` Scenario「内联 metadata 与非内联形态」 | 「该表被读成另一个元素 FQN」 |
| 5 | `archi-knowledge.md`「位置与键形」 | 「点分嵌套与子表读成**另一元素**（`metadata` 必须内联表）」 |
| 6 | `SCAFFOLD_TEMPLATE` 的 `metadata` 注释 | 「必须是内联表（子表会被读成另一个元素）」 |
| 7 | `archi_knowledge_test.rs::骨架静默陷阱四条在场` | 以「内联表」为 needle，把假因果钉成测试 |
| 8 | `archi_tool_test.rs` 的 `内联表` needle | 同上，钉在模板上 |
| 9 | `parse/toml_test.rs` 的用例名与注释 | 「子表形态不静默成元数据」「被读成元素 `App.Auth.metadata` 的父表」 |

### 三 · 共同根因

两处同形：**把推断当事实写进规格**——case A 的推断是「容器也必须有根」，case B 的推断是「子表会被静默读成另一个元素」。前者靠四方各说一套存活（无任何测试对拍「`package` 的 `path` 是否必填」），后者是 `toml::Value` 手工解析时代的记忆残留（改成强类型 + `deny_unknown_fields` 后陷阱消失，spec 没跟着改）；两处都没有「规格 vs 实测」的行为对拍。

---

## 提案

### 一 · `path` 的基数是 `kind` 的属性（终裁：`package` 至多一条包根）

| kind | `path` | 本仓实测 | 误伤 |
|---|---|---|---|
| `package` | **≤ 1 条**：0 条 = 纯容器（合法）；1 条 = 包根目录；**≥ 2 条 ⇒ `invalidValue`、整图停摆** | 19 个恰 1 条、3 个 0 条 | 0 |
| `module` | 不校验（多条 = 若干根的清单） | 30 个（含唯一的 5 条 `Desktop.Shell`） | — |
| `domain` / `composite` | 不校验（选填） | 3 个 domain 全 0 条 | — |

「`path` 是**根**，不是**清单**」由此落成可判形态：`package` 至多一个 root，`module` 是若干根的清单；`kind` 与 `path` 的搭配第一次有机械判据（此前零校验）。**只守上界**——`module ≥ 1` 与目录尾 `/` 形态**不做**（实测零误伤，不是当下痛点；顺手扩张即超出本变更）。

### 二 · L1 的落点：`parse` 期 `invalidValue`，指向第 2 条 `path`

```
to_model 逐元素校验（复用 validate_apis 的形态）
   ↓ 诊断位置 = 第 2 条 path 的 Spanned（比元素表头更准）
parse_dir 照常累积诊断（不停摆）
   ↓ graph_view：diagnostics 非空 ⇒ elements / relations 清空
整图停摆（既有全或无语义，本变更只钉住、不改写）
```

零新词（`DiagnosticKind` 仍九值，复用 `invalidValue`）、零生成物改动、与 `apis` 五拒同源。容错为零是**已知并接受的代价**：`package` 写两条 `path` 在契约面本就只有 `path[0]` 可见。文案点名「一个包至多一条包根目录（纯容器可留空）——多条 path 属 `module`」。

### 三 · L2 的落点：三个纯分层容器迁 `composite`

`Backend.Core` / `Backend.Infra` / `Backend.Tools` 各改 1 行 `kind = "composite"`（`path` 仍空）。零副作用（逐项核过）：结构诊断只看 FQN 前缀、归属面本就无 `path`、契约面本就退出、前端 `element-tree.tsx` 的 `composite → LayersIcon` 已就位、`INDEX.toml` 不含 `kind`。

收益有两处：① `composite` 由「零实例的理论值」变成有实值（纯分层容器正是它的教科书用例）；② ADR `0001-model-granularity.md` **§2 与 §3 的自相矛盾**（§2 说分层容器同为 `package`、§3 把它们列进 `domain` / `composite` 容器）同批消除——文本判据见 AC-13。

### 四 · L3 的落点：`noPath` 按 `kind` 收窄（方案 B）

| 空 `path` 的 kind | 读数 | 语义 |
|---|---|---|
| `domain` / `composite` | **零告警**（静默） | 选填容器的标准形态 |
| `module` | `noPath` warning | 模块必填 |
| `package` | `noPath` warning | 「包应有包根」；真要纯分组请改用 `composite` ——这是「`kind` 可能用错了」的提示，不是缺陷 |

parse 期放行、drift 期提醒，与 L1 互补。`ArchiDriftKind` 仍五值、`model_warnings.push(` 仍两处（`drift/mod_test.rs` 的源文本计数断言不破）；`NoPath` 的**变体 doc 与枚举级总述两处**都经 specta 落进 `bindings.ts`，必须同批改写并 `bindings:export`。

**L2 与 L3 必须同批**：L3 生效而 kind 未迁时 3 个 `package` 容器仍报 ⇒ baseline 只能降到 3 条，等迁完 kind 又要从 3 改到 0（同一契约改两次）。反过来 L2 单独交付是安全的（6 条不变），只是收益到不了 0。

### 五 · case B 的落点：换理由、留结论、删禁令

| 面 | 改成 |
|---|---|
| 未引号点分表头 | 「归 `unknownField`、**整份源解析失败**（全或无）」——不是静默误读，而是**写盘后**才可判。**知识骨架里不写这个词**（`md不列结构诊断全量词表` 禁结构诊断词进骨架），只写「整份源解析失败、诊断点名非法子键」 |
| `metadata` 子表 | 「与内联表**等价**、零诊断」；子表成为唯一声明则缺 `kind` 报 `missingField` |
| C 档归类（仍留散文） | 理由换为**动作前不可判定**（写面是自由文本 `write` / `edit`，结构诊断只在写盘后可得；四档表的「不可判定」即此义），MUST NOT 再写「会被静默读成另一个元素」 |
| 行为锚 | 三条断言都由 `parse/toml_test.rs` 的**行为**用例钉住（不是「断言 md 含某词」）；「子表等价」这一条**新建**。骨架侧的 `assert!(!source.contains(…))` 四条 forbidden needle 与之分工不同：行为锚证真，needle 守假（只断言「假因果不进骨架」），**两者都必须在场** |
| 推荐形态 | 不动（explore §六.2）：模板与骨架仍以单行内联表为示例，只陈述「等价」，MUST NOT 主动推荐子表 |

同时把 `desktop-archi-model` 的 Scenario「未引号点分形式被拒」由「按『无元素 / 缺字段』处置」收紧为「`unknownField` + 点名 `Auth` + 全或无」——现文模糊到无法判真伪，也是这一族漂移的一部分。

### 六 · 本仓读数（落地当天应成立）

| 项 | 变更前 | 变更后 |
|---|---|---|
| `model.elements` / `model.relations` | 55 / 129 | **55 / 129**（不变） |
| `kind` 分布（`package` / `composite`） | 22 / 0 | **19 / 3** |
| `model.warnings`（`noPath`） | 6 | **0** |
| `model.structuralErrors`（四类） | 0 | **0** |
| `apiFindings`（`docMissing`） | 39 | **39**（不变） |
| `observations` | 53 | **53**（不变） |
| `status` / `passed` | clean / true | **clean / true** |
| `scan.*` 三项计数 | — | **逐字段不变**（本变更零新增 / 删除文件） |
| `api:export` / `api:check` | — | **零 diff**（`INDEX.toml` / `.schema.json` / `.i.*` 不动） |
| `bindings:export` | — | 预期 diff = `NoPath` 两处 JSDoc 注释 |
| 语料金样（20 份 drift `expected/*.json`，17 full + 3 incremental） | — | **逐字节不变**（唯一含 `noPath` 的两份其 `App` 是 `package`，方案 B 仍报） |

### 七 · 硬前提（开单前实测，四条全过）

| 硬前提 | 核查方式 | 结果 |
|---|---|---|
| L1 不砸既有语料 | 扫 `core/archi/tests/corpus/**/*.toml` 的可解析文件：23 个 `package` 元素；含语法非法的三份语料在内，逐文件无「`package` + ≥2 条 `path`」 | ✅ 零误伤 |
| L3 不砸金样 | 扫 20 份 `expected/*.json` 的 `model.warnings`：仅 `api-mismatch` / `target-unowned` 两份有 `noPath`，其 `App` 是 `package`（方案 B 仍报） | ✅ 金样零变化 |
| L2 不砸契约面 | `INDEX.toml` 恰 49 条 entry = 55 元素 − 6 容器；三个分层容器本就 `path` 空、退出契约面 | ✅ `api:export` 零 diff |
| 无代码固化容器数 | `grep 六个容器 packages/desktop` | ✅ 零命中（只在 spec / ADR 文本里） |

---

## 能力

### 新增能力

无。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-model` | **MODIFIED ×1** | 「架构模型文件格式与字段封闭」：① 新增 **`path` 基数随 `kind`** 条（`package` ≤ 1、0 条合法、≥2 条 `invalidValue` + 第 2 条位置 + 整图停摆；`module` / `domain` / `composite` 不设上界）；② 字段面封闭条由「`metadata` MUST NOT 子表」改为「内联表与子表等价 + 子表成为唯一声明即 `missingField`」；③ Scenario「内联 metadata 与非内联形态」拆为「子表与内联表等价」「子表成为唯一声明即缺 kind」两条；④ Scenario「未引号点分形式被拒」收紧为 `unknownField` + 全或无；⑤ 新增两条 `package` `path` 基数 Scenario |
| `desktop-archi-drift` | **MODIFIED ×1** | 「文件归属与两套解析器一套归属逻辑」：`noPath` 判据按 `kind` 分档（`domain` / `composite` 豁免；`package` / `module` 仍报；`package` 空 `path` 的告警语义 = 「`kind` 可能用错了」，不进 `errors`）；新增两条 Scenario（四 kind 空 `path` 的读数分布 / package → composite 消解告警） |
| `desktop-archi-agent` | **MODIFIED ×3** | ① 「archi_scaffold 只读工具的新元素模板契约」：模板自证锚补「`metadata` 形态 / `kind` 四值语义」；Scenario 由「叶元素必填 / 必须内联表、禁子表」改为「模块必填（可多条）/ 包至多一条包根 / 子表与内联表等价」；新增「模板 kind 行覆盖四值语义」Scenario。② 「架构会话的系统知识注入」：`path` 一行改基数口径（模块可多条 / 包至多一条包根 / ≥2 条即 `invalidValue`）；键形段删假因果、改「未引号点分 → `unknownField`、整份源不解析」「子表与内联表等价」；kind 语义段 `package` 补「至多一条包根」、`composite` 补「纯分层容器是标准形态」。③ 「知识分层与工具面同批移动」：补充规则① 由「静默错误必须留上下文」改写为「形态错误不静默、但发生在写盘之后」，并点名三条行为测试；Scenario「静默陷阱与 kind 语义留在散文」更名并改断言 |
| `repo-archi-model` | **MODIFIED ×2**（＋ Module Contract 读数换段） | ① 「基线落位与模型文件划分」：`path` 纪律改写（模块必填可多条 / 包至多一条包根 / `domain`·`composite` 选填）、六个容器按语义分两组（3 `composite` + 3 `domain`）、文件表三行补 kind 标注。② 「图自洽与停摆禁令」：`model.warnings` 由 6 条 `noPath` 改为空（含 Scenario）。③ Module Contract 的 CLI 核对表：`model.warnings` 读数 6 → 0（该表在 MC 节，不在 requirement 内） |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-api` / `desktop-archi-api-check` | 契约面、生成链、十三值判据、纳管口径零改动；`composite` 元素本就 `path` 空 ⇒ `api:export` 零 diff、`docMissing` 39 不变 |
| `desktop-archi-gate` | `model.warnings` 不进 `passed`；门禁布尔口径不变 |
| `desktop-archi-canvas` / `desktop-archi-file-tree` / `desktop-archi-contract-panel` | `kind` 只影响元素树图标（`composite` 映射已就位）；文件树按 `path` 非空判、六个容器仍空；判据读数不变 |
| `desktop-corpus-regression` | 20 份 drift 金样与 15 份 DSL 语料零变化（唯一含 `noPath` 金样的 `App` 是 `package`） |
| `desktop-frontend-module-entry` / `desktop-archi-chat` 等 | 与模型字段面 / 漂移判据无关 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` | `to_model` 增 `package` 的 `path` 条数校验：≥2 条出 `invalidValue`（位置 = 第 2 条 `Spanned`，文案点名「至多一条包根」与合法形态）；0 / 1 条放行 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` | `path.is_empty()` 分支收窄为「`matches!` 只对 `ElementKind::Package` 与 `ElementKind::Module` 为真」才 push；`use crate::dto::{self, ElementKind, GateTier};`；noPath 的 `message` 字符串**逐字不动** |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` | `NoPath` 变体 doc 改「`package` / `module` 未声明 `path`（不参与归属；容器空 `path` 是合法形态）」；枚举级总述里 `noPath` 的括注同步（两处都进 `bindings.ts`） |
| `.dev-team/architecture/graph/model/02-backend-core.toml` / `03-backend-infra.toml` / `05-backend-tools.toml` | 各 1 行 `kind = "composite"`（`path` 仍空；表头计数不动） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | §2 末段：分层容器由「同为 `package`」改为「纯分组容器 `kind = "composite"`」；§3 首句：六个容器拆两组（`composite` 纯分组 / `domain` 业务聚合），删「`composite` 本仓当前无实例」 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` | ①「位置与键形」键形句改写（未引号点分 → 整份源解析失败、诊断点名非法子键；子表与内联表等价；**骨架不写结构诊断词**）；② 字段纪律 `path` 首条改基数口径（骨架不写 `invalidValue`，只写「≥2 条即模型停摆」）；③ 元素类型 `composite` 补「纯分层容器是标准用法」；两个体量锚同批实测更新 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` | `SCAFFOLD_TEMPLATE` 三行：`kind` 四值齐列（`composite` 补语义）、`path` 改「模块必填（可多条）/ 包至多一条包根 / `domain`·`composite` 选填」、`metadata` 改「内联表与子表等价」 |
| `packages/desktop/src/types/generated/bindings.ts`（`.zod.ts` 按生成结果） | `bindings:export` 重导，**勿手改**；预期 diff = `NoPath` 两处 JSDoc |
| `packages/desktop/package.json` | `version` patch +1（0.7.8 → 0.7.9） |

### 测试文件

| 文件 | 改动 |
|---|---|
| `crates/core/archi/src/parse/toml_test.rs` | L1 四条：`package` 2 条 `path` 拒收（`invalidValue` + 行号命中第 2 条 + 文案）/ `package` 0 条零诊断 / `module` 2 条零诊断 / `composite` 2 条零诊断；case B：新增 `metadata子表与内联表等价`，既有 `metadata非字符串值被拒且子表形态不静默成元数据` 拆为 `metadata非字符串值被拒` 与 `metadata子表成为唯一声明即缺kind` 两条（后者的注释去「被读成元素 `App.Auth.metadata` 的父表」假因果） |
| `crates/core/archi/src/dto_test.rs` | 新增独立用例（`package` 两条 `path` ⇒ `graph_view` 停摆），钉住「不改成跳过元素」；既有 `graph_view九类结构错一律停摆` 的表与名字逐字不动（不为新判据扩容成「十一类」） |
| `crates/core/archi/src/drift/mod_test.rs` | 三条：`composite` 空 `path` 零警告 / `domain` 空 `path` 零警告 / `package` 空 `path` 仍报一条 `noPath` 且 `errors` 空；既有 `noPath警告`（`Ghost` 是 `module`）逐字不动 |
| `crates/infra/agent/src/archi_knowledge_test.rs` | `骨架静默陷阱四条在场` 改名（去「静默」）＋ needle 由「内联表」换为「子表与内联表等价」（**MUST NOT 用 `unknownField` 作 needle**——`md不列结构诊断全量词表` 禁该词进骨架）；四条 forbidden needle（`assert!(!source.contains(…))`）与其说明注释**保留**——它们是「骨架零假因果」的防线，不属待清残留（AC-6 防线面）；`骨架体量锚` / `全文体量锚` 的两个冻结值按实测更新（目标区间不抬） |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | `archi_scaffold三kind逐值` 改遍历 `ElementKind::ALL` 四值；`archi_scaffold省略kind取domain` 的 `叶元素必填` needle 改 `module 必填`；`内联表` needle 按新模板文本核 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `DiagnosticKind` 九值 / `ArchiDriftKind` 五值 / `ArchiApiFindingKind` 十三值 / `REPORT_VERSION` | 本变更零新增词面（L1 复用 `invalidValue`、L3 只收窄适用范围） |
| `.dev-team/architecture/graph/INDEX.toml`、`.dev-team/architecture/models/**`、`.schema.json`、`.i.rs` / `.i.ts` | 生成物；`kind` 迁移与判据新增都不产生产物变化，重生成应零 diff |
| `crates/core/archi/tests/corpus/**` 与 `expected/*.json` | 金样零变化；MUST NOT 为新判据改语料或改金样 |
| noPath 的 `message` 字符串 | 两句金样逐字含它；改文案会连带改金样（本变更不做） |
| 前端产品代码（`src/**`，生成物 `bindings.ts` 除外） | 零 IPC / 零 UI 变化；`element-tree.tsx` 的第四枚图标已就位 |
| `AGENT.md` / `CLAUDE.md` | 规范面（`path` / `kind` 纪律归架构域与能力 spec），本变更不改仓库根指引 |
| `src-tauri/Cargo.toml` | crate 版本行 MUST NOT 跟随 `package.json`（AGENT.md 版本纪律） |
| `openspec/changes/archi-model-path-and-silent-trap/explore.md` | 只读输入 |
| `openspec/specs/**`（基线）与 `openspec/changes/archive/**` | delta-only：本变更只写 4 份 delta，基线旧句（假因果三处、`noPath` 读数三处）由归档合并本 delta 的 MODIFIED 块收口；手改基线即「同一事实两份持有者」，与本次病灶同形（AC-6 的 spec 面因此只追 delta，AC-10 同口径） |

---

## 验收标准

1. **AC-1 · L1 判据的三个方向都成立**：`package` 写 2 条 `path` ⇒ 恰一条 `invalidValue`、行号命中第 2 条、文案含「至多一条包根」；`package` 省略 `path` ⇒ 零诊断；`module` / `domain` / `composite` 各写 2 条 ⇒ 零诊断。反向判据：`package` 写 1 条、`module` 写 5 条（`Desktop.Shell` 形态）全绿。
2. **AC-2 · 停摆语义未被改写**：带上述诊断的模型经 `graph_view` 返回 `elements = []` / `relations = []`（不是「跳过坏元素、保留其余」）；`dto_test` 的该分支由独立用例覆盖，既有「九类结构错一律停摆」表逐字不动。
3. **AC-3 · L3 的豁免面精确**：`domain` / `composite` 空 `path` 零 `noPath`；`package` / `module` 空 `path` 仍各记一条；`errors` / 顶层 `warnings` 不受影响；`ArchiDriftKind::ALL` 仍五值、`drift/mod.rs` 的 `model_warnings.push(` 仍两处。
4. **AC-4 · L2 的零副作用**：元素 55 / 关系 129、四类结构诊断（`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`）零命中、`INDEX.toml` / `.schema.json` / `.i.*` 零 diff；`kind` 分布 = `package` 19 / `composite` 3 / `domain` 3 / `module` 30。
5. **AC-5 · baseline 读数换段且三处同值**：`repo-archi-model` 的三处 `model.warnings` 读数一致为**空**（0 条 `noPath`）；`docMissing` 39 / `observations` 53 / `status = clean` / `passed = true` 逐字不变；CLI 核对表与 Scenario 不互相矛盾。delta 内的元素 / 关系读数一律 **55 / 129**（`129 = 后端 52 + 前端 77`），六个文件段与本变更零计数变化一致（`01` 2 / 0、`02` 9 / 10、`03` 8 / 25、`04` 1 / 14、`05` 3 / 3、`06` 32 / 77）；MUST NOT 把上一变更（`Backend.Core.Notification` 已交付的元素 +1 / 关系 +4）的增量叙述为本变更的计数变化，亦 MUST NOT 残留任何与 55 / 129 或 39 条 `docMissing` 不一致的旧读数。
6. **AC-6 · case B 的假因果按面分判（正向面零命中 / 防线面必须在场 / spec 面只许否定形态）**：短语集 =「读成另一个元素」「读成**另一**元素」「必须内联表」「被读成元素」（= 骨架防线的四条 forbidden needle）＋等价旧文「`metadata` MUST 用内联表」「必须内联表、禁子表」；用 `grep -rF` 列全命中后逐条归面，**判红看该句是否把假因果陈述为事实，不看同行是否含某个词**：
   - **正向表述面（零命中）**：`packages/desktop/src-tauri/crates/**` 除下述防线单元之外的一切（生产源码与注释、`archi-knowledge.md`、`SCAFFOLD_TEMPLATE`、测试文件内防线单元之外的注释）＋ `.dev-team/architecture/decisions/**`。`grep … | grep -v '_test\.rs'` 零命中只是下限——测试文件里防线单元之外的命中同样判红。
   - **防线面（必须在场，删了即红）**：`crates/infra/agent/src/archi_knowledge_test.rs` 的 `骨架形态纪律四条在场` 中的 `assert!(!source.contains(forbidden))` 循环及其函数 doc comment——命中依据是**否定断言形态**（钉住「骨架零假因果」），不是残留；四条 needle 与说明注释 MUST 保留，MUST NOT 因 grep 洁癖删除，亦 MUST NOT 退化成「同行含 `MUST NOT`」的文本匹配。
   - **spec 面（收窄到本 delta；只许否定形态）**：追改面 = `openspec/changes/archi-model-path-and-silent-trap/specs/**`。命中只允许出现在**否定 / 禁令 / 删除语境**——所在句带 `MUST NOT` / 「不成立」/「不是」/「零命中」/「删」之一（引号包裹不是必要条件：`MUST NOT 把该表读成另一个元素` 这类直接否定同样合法），且 MUST NOT 以断言形式陈述假因果——判例：「MUST NOT 再表述为『子表会被读成另一个元素』」合法（禁令）、「把假因果句删掉」一类删除叙述合法（该形态当前 delta 未使用）；「子表会被读成另一个元素，故模板只用内联表」判红（断言）。**基线 `openspec/specs/**`、`openspec/changes/archive/**`、`explore.md` 不在本变更追改面**：delta-only 下 MUST NOT 手改基线，其命中不计红——基线旧句由归档合并本 delta 的 MODIFIED 块收口。
   - **反向（否定锚不可删）**：删掉禁令句或防线 needle（两侧否定锚消失）与留断言同判红——它们是防假因果回流的唯一把手（R-7）。
7. **AC-7 · case B 的每句断言都有行为锚**：`parse/toml_test.rs` 恰有「未引号点分 → `unknownField` 全或无」「子表与内联表等价」「子表成为唯一声明 → `missingField`」三条行为用例；`骨架静默陷阱…` 这类「断言 md 含某词」的用例名不再以「静默」为名。
8. **AC-8 · 知识文本与模板同步**：骨架含 `path` 基数句、`composite` 标准形态句、键形真因果句，且**零结构诊断词**（`md不列结构诊断全量词表` 保持绿——`unknownField` 不进骨架）；`SCAFFOLD_TEMPLATE` 的 `kind` 行四值齐列、`path` 行按新基数、`metadata` 行写等价；`archi_tool_test` 遍历 `ElementKind::ALL` 断言四值；骨架 / 全文两个体量锚按实测更新且落在 2300–2600 / 2450–2750 B 区间内（MUST NOT 抬门限）。
9. **AC-9 · 生成物与零 diff 门**：`pnpm bindings:export` 后 `git diff` 只含 `bindings.ts` 的 `NoPath` 注释改写（`.zod.ts` 无变化）；`pnpm api:check` 零差异退出；`api:export` 对 `INDEX.toml` / `.schema.json` / `.i.*` 零 diff。
10. **AC-10 · 零行为与零分布扩散**：`git status --porcelain` 的改动集 ⊆ 本提案「变更范围」所列文件，且 `.dev-team/architecture/decisions/0001-model-granularity.md` **必在改动集内**（「实现文件」表的一行；只迁模型数据不改 ADR、或只改 ADR 不迁数据，两种单侧交付都不满足，内容判据见 AC-13）；`openspec/changes/archi-model-path-and-silent-trap/specs/` 恰 4 个能力目录，`openspec/specs/` 基线、`openspec/changes/archive/**` 与 `explore.md` 零改动（delta-only；ADR 是裁定记录，不进 spec delta）；语料金样 20 份逐字节不变；`scan.*` 三项计数与变更前基线逐字段相同；前端产品代码零改动（生成物除外）。
11. **AC-11 · 版本纪律**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本逐字不变。
12. **AC-12 · spec delta 形态**：4 份 delta 的 `### Requirement` 标题与基线逐字一致；每条 `### Requirement` 至少一个 `#### Scenario:`（恰 4 个 `#`），WHEN/THEN 成对；MODIFIED 块为基线整段拷贝后再改，未丢任何未冲突条目。
13. **AC-13 · ADR 0001 的分层容器语义收敛（§三收益②的判据）**：对 `.dev-team/architecture/decisions/0001-model-granularity.md` 逐项判定——① **§2 分层容器指向 `composite`**：§2「元素类型与粒度档对应」段内存在一个以 `；` / `。` 分隔的分句，同时含三容器的指称（`core` / `infra` / `tools` 或 `Backend.Core` / `Backend.Infra` / `Backend.Tools`）、「分层容器」或「纯分组容器」字样、与字面 `kind = "composite"`；同一段内 MUST NOT 再有「分层容器」与 `package` 并置的分句（现文「`core` / `infra` / `tools` 分层容器同为 `package`」即判红）。② **§3 两组语义在场**：§3 首段把六个容器拆成两组且成员无交叉——`composite` 组 = `Backend.Core` / `Backend.Infra` / `Backend.Tools` 且写明「纯分组」，`domain` 组 = `Desktop.Views` / `Desktop.Components` / `Desktop.Components.Standard` 且写明「业务聚合」，两组标签同段或紧邻句成对（可附「无层级 / 不持文件 / 空 `path` 典型形态」与「可持文件」，核心判据是两枚语义标签与成员配对，读者不必跨节拼装）。③ **全文零旧说法**：grep「同为 `package`」「本仓当前无实例」**零命中**。ADR 的 `kind` 陈述与 `02` / `03` / `05` 三文件的 `kind = "composite"` 实测一致（AC-4 的分布；文本与数据 MUST 同批，不以单侧改写掩盖另一侧未迁），ADR 不进 `openspec/specs/`（spec delta 仍恰 4 份，AC-12 口径不变），其改动集归属 AC-10。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **L1 是产品行为变更**，不只影响本仓：任意 workspace 里 `package` 写 ≥2 条 `path` 都会从「静默取首条」变成「整图停摆」 | 这是有意的零容忍（契约面本就只读 `path[0]`，多写从来无效）；文案自带出路（多条 path 属 `module`）；本仓 + 语料实测零误伤。若要渐进，唯一替代是「告警不拒收」——那会让暗假设继续存活，explore 已裁 parse 期拒收 |
| R-2 | **「子表与内联表等价」目前无行为测试**（explore 记为「TOML 语义直推」） | AC-7 要求先补行为用例、再让 spec / 模板 / 骨架陈述等价；若该用例意外失败（例如 serde 对嵌套表的处理有差异），回落方案是：只保留「`unknownField` 全或无」的纠正、把 `metadata` 子表从「等价」改为「不推荐但合法」并重新实测——**MUST NOT 在无行为锚时把等价写进 spec** |
| R-3 | **C 档归类是否仍成立**：四档表的 C 判据写「不可判定（语义 / 意图）」，而本条被改成「动作前不可判定」 | 本提案把理由落成「动作前不可判定（自由文本写面，结构诊断写后才可得）」，四档表文字不动——表的两轴本就是「可判定性 × 时机」，C 是「动作前不可判定」格。若将来重排四档，本条应迁 B/D（模板已承载形态），属独立变更 |
| R-4 | **baseline 三处计数 + ADR + 六份文件是本变更的“同批”硬耦合**：L2 与 L3 分开做会让 `model.warnings` 读数改两次 | 提案已把 L2 / L3 绑成同一变更（§四）；AC-4 / AC-5 把分布与读数钉成交付判据 |
| R-5 | **知识骨架体量**：新增 `composite` 标准形态句与基数句可能挤向 2600 B 目标上限 | AC-8 要求实测后更新冻结值且 MUST NOT 抬门限；超限时压缩同段冗余（如合并 `domain` / `composite` 的半句），不删纪律 |
| R-6 | **生成物漏重导**：`NoPath` 两处 JSDoc 经 specta 落 `bindings.ts`，忘跑 `bindings:export` 即 `api:check` 失败 | AC-9 用 `api:check` 零差异兜底；`bindings.ts` MUST NOT 手改（会被下次生成静默覆盖） |
| R-7 | **文案洁癖误伤**：把「子表」一律当禁词，会删掉合法的子表说明（`metadata` 子表本身就是合法形态）；同一洁癖会顺手删掉测试里的 forbidden needle 与 delta 里的禁令句 | AC-6 按面分判：正向表述面零命中、**防线面（`assert!(!…contains…)` 四 needle）必须在场**、spec 面（本 delta）只许否定形态——三面之外不追；`metadata` 子表**合法**这一点必须留在 spec / 模板 / 骨架里（删了就变成另一种失真） |
| R-8 | **`composite` 语义被读成「万能容器」**：迁移后有人把业务聚合容器也写 `composite` | `repo-archi-model` 与 ADR 写明两组语义（`composite` 纯分组、`domain` 业务聚合），但**不新增机械判据**（kind 判定仍是架构判断）；本变更只让 `composite` 有实例、有教科书句——ADR 内两组语义的文本在场由 AC-13 判定（§2 指向 `composite`、§3 两组语义成对、全文零旧说法） |
| R-9 | **noPath 豁免掩盖「忘了写 path」**：`domain` / `composite` 空 `path` 从此零告警 | 被用户裁决接受（方案 B）：这两类容器的空 `path` 是定义而非遗漏；「这个元素缺不缺 `path`」仍由 `archi_module.hasPath` 一次判出，不依赖 warning 面 |
| R-10 | **测试名与 needle 的迁移**（`骨架静默陷阱四条在场`、模板的 `内联表` / `叶元素必填`）容易漏改导致红 | AC-7 / AC-8 列出逐项；改动落在既有测试文件内，MUST NOT 新建测试文件（保持 `scan.*` 计数不变） |
| R-11 | **MODIFIED 块「整段拷贝再改」会把基线的旧计数叙述一并带入**：上一变更（`Backend.Core.Notification`）的增量叙述与两条旧读数（关系数、`docMissing`）正因此残留在 delta 里，与实测 55 / 129 / 39 冲突 | 拷贝后 MUST 逐段核计数：元素 / 关系一律 55 / 129（`129 = 后端 52 + 前端 77`）、`docMissing` 39、`observations` 53；上一变更的增量只作历史事实陈述，MUST NOT 记成本次变化；AC-5 已把该纪律写成可判条目 |

---

## explore 未决项与「不含」的归属

| explore 条目 | 处置 |
|---|---|
| §六.1 元纪律「凡以静默为由留散文的句子 MUST 附行为测试」是否入 spec | **不入 spec**：本期按该精神落地（AC-7 的三条行为锚 + 改写后的 spec 句都点名它们），但把元纪律写成 MUST 无法机械执行，只会再造一条无人验证的散文断言——与本次病灶同形。留作后续线索 |
| §六.2 是否主动推荐 `metadata` 子表形态 | **不动**：仍以单行内联表为示例，只陈述等价；推荐形态改向属风格决策，会与既有模型风格冲突 |
| §8.4.1 `domain` 与 `composite` 的分工要不要写死 | **写语义、不写机械判据**：`repo-archi-model` / ADR 写明「`composite` 纯分组、`domain` 业务聚合」，kind 判定仍是架构判断（R-8） |
| §8.4.2 `noPath` 豁免边界 | **已裁：方案 B**（`domain` / `composite` 豁免，`package` / `module` 仍报） |
| §8.4.3 `kind` 层级序校验 | **不做**：独立能力，本变更不顺手扩张 |
| §9.6 L4（`module` 多 `path` × 契约面 `path.first()`） | **不做**：与 L1 正交的裂缝；本变更保留 `module` 无上界的地基，L4 由独立变更处理（`Desktop.Shell` 当前 `status = absent` 零损失） |
| §12.2 拟名 `archi-kind-path-discipline` + 「case B 不含」 | **按实际目录名执行**：`archi-model-path-and-silent-trap` 同时承载 case A（L1 + L2 + L3）与 case B（理由纠偏）；两处共享同一根因，合并交付不扩大代码面（case B 只改文本与测试断言） |
