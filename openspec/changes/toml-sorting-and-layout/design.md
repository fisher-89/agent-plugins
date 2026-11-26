# 设计: toml-sorting-and-layout

> **变更**：`toml-sorting-and-layout`（架构模型 TOML 的写入序：写路径上的规范化）
> **输入**：本目录 `proposal.md`（定稿）、`explore.md`（只读）、`specs/` 三份 delta（`desktop-archi-model` / `desktop-archi-agent` / `desktop-phase-prompts`）
> **性质**：在四个 `apply_*` 的渲染前插一层**纯函数规范化**，把目标文件收到三条不变量（E1 父先于子 / E2 子树连续 / R1 同一 `from` 组连续），并把顺带搬家的块经 `WriteOutcome.reordered` 报出。零新工具、零新诊断、零新错误变体、零新依赖、零新 IO。
> **一句话**：规范化只在**写面出口**做**最小移动**——健康文件逐字节 no-op，`06-frontend` 只搬 3 块；序由测试守，不由运行时守。

## 架构组件 (Architecture Components)

### C1 · 写入序规范化（引擎，新模块）

- **职责**：把「改写完 AST、尚未渲染」的文档收进三条不变量，并回报本次搬迁块。唯一职责是**位置**——块的内部字节、键序、decor、注释、空行、表头计数一律不碰。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/edit/layout.rs`（新）。
- **依赖**：`toml_edit::{DocumentMut, Item, Table, ArrayOfTables}`、`serde::Serialize`；**零** `crate::parse` / `crate::graph` / `crate::drift` / `crate::dto` 引用，**零** `EditError` 构造（无 `Err` 臂），**零** `DiagnosticKind` 构造，零文件系统、零子进程。
- **技术**：两条编码器事实被当作前提（`toml_edit` 0.25.15）——① 渲染序 = `(!path 空, position)` 的**稳定排序**，`position` 为 `Option<isize>`，`None` 时继承遍历中前一个表的位号（`src/encode.rs:183-196`）；② 解析按表头连续分配 `position = 1..N`（`src/parser/document.rs:456-460`）。于是**元素面 = 一次号重分配（permutation）**，**关系面 = 真移动 `ArrayOfTables` 成员 + 同批重编号**（`insert` / `remove` / `push` 可用，`Table::sort_values_by` 不管子表，不用）。

**算法（唯一实现，`normalize` 全貌）**：

```
1. 读源序 S
   元素区：沿 `element` 表 items 序复刻编码器的「自身 position，否则继承前表」语义，
           得到每个 `[element."<FQN>"]` 的有效位号；`Item::Value`（内联表形态）不参与。
   关系区：沿 `ArrayOfTables` 成员序，同一继承语义取有效位号。
   合并：按（有效位号，元素在前，面内 items 序）升序 = 交付前的文本序。
2. 算目标序 T
   元素面 = DFS + 同层首次出现序：顶级 = 父 FQN 不在**本文件**元素集内的 FQN（按源序）；
           直接子 = 以 `<父FQN>.` 起头且余段不含 `.` 的 FQN（按源序）。递归展开。
   关系面 = 组连续 + 组间按该 `from` 首次出现位置 + 组内源序（新边因此落组尾）。
   区域序 = 元素区全部在前，关系区全部在后（混插形态因此被理顺）。
3. 最小移动集合 = S 与 T 的最长公共子序列（LCS）补集；
   平局（两条支路等长）时**推进 T**，即优先保留「源序更早」的项。
   ⇒ `moved` 为空 ⇔ S == T ⇔ 规范化对文本是 no-op（**快路径：直接返回，零 AST 改动**）。
4. 块口径（`reordered` 的计量单位）
   元素面：把被移动元素按「最上层被移动祖先」归组，取该祖先 FQN（自身无被移动祖先即自身）；
           按 T 内位置排序。
   关系面：把被移动的关系条目按 T 内**连续同 `from`** 归段，取该段 `from`；按 T 内位置排序。
   `moved` = 块数（元素块 + 关系块）；`blocks` = 各块名，元素块在前（交付文本序）。
5. 落笔（只动位置面）
   元素区：T 的元素序第 i 位 ⇒ `set_position(Some(i+1))`；
   关系区：按 T 的关系序重建 `ArrayOfTables`（成员整体搬迁，成员内部零改写），
           第 j 位 ⇒ `set_position(Some(k + j + 1))`，k = 表形态元素数。
6. 返回 `Reorder::of(moved, blocks)`——cap 在构造处施加，`moved` 保留真值。
```

- **射程两条**（与 spec 同字）：① **单文件**——元素不跨模型文件搬，一次 `plan` = 一个 `ModelWrite`；② **表形态**——只对 `[element."<FQN>"]`（`Item::Table`）动手，内联表形态的条目原样保留、不参与重排（它没有 `position`，渲染在 `element` 表的键区）。附带一条形态边界：`element` 表若写成显式 `[element]` 表头，它自身的位号不在重编号区间内；与 1 号相撞时按「父先于子」的遍历序稳定落在最前，文本仍稳定（快路径对这类文件通常直接 no-op）。
- **可达性前提**（不是防御，是论证）：模型文件顶层只允许 `element` / `relation` 两键——`RawDocument` 带 `deny_unknown_fields`（`parse/toml.rs:22-28`），任何第三方顶层表都会变成 `unknownField` 诊断，`plan_*` 的 `load_model` 前置即拒、`finish` 的重解析自证也会把产出判死。故「号重分配」的相撞面在交付路径上不存在；规范化不为此写 `Err` 臂。

### C2 · 写回原语族接线（引擎）

- **职责**：四个 `apply_*` 在改完 AST 之后、`finish` 之前调一次 `normalize`，把产出由 `String` 扩为「交付文本 + `Reorder`」；`plan_*` 把 `Reorder` 装进 `WriteOutcome.reordered`。
- **文件**：`crates/core/archi/src/edit/mod.rs`（改）。
- **依赖**：`edit::layout::{normalize, Reorder}`（同 crate 子模块）。
- **技术**：产出顺序由「改 AST → 渲染 → 表头回填 → 重解析自证 → 回读」变为「改 AST → **规范化** → 渲染 → 表头回填 → 重解析自证 → 回读」。`finish` / `verify_element` / `verify_inline_relations` / `verify_relation_count` / `verify_relation_absent` / `backfill_header_counts` / `recount` / `new_file_content` / `check_*` 前置族**逐字不动**——规范化不在自证链内，它没有失败臂，也 MUST NOT 在自证里断言序。
- **接线形态**（四个 `apply_*` 各一处，同一形态）：

  ```rust
  let reorder = layout::normalize(&mut document);
  let content = finish(document, |model| { /* 既有 verify 闭包逐字不动 */ })?;
  Ok((content, reorder))
  ```

  规范化落在 `expect` 校验之后、`hash` 计算之前（`write_of` 在 `plan_*` 内、文本定稿后算 `hash`），这两条既有链顺序零改动。

### C3 · 搬迁回报出线（引擎类型 + 出口）

- **职责**：承载「本次顺带搬了几块、都是谁」，恒在场、零值也写。
- **文件**：`crates/core/archi/src/edit/layout.rs`（类型定义）、`crates/core/archi/src/lib.rs`（出口）。
- **依赖**：`serde::Serialize`（`Reorder` 的派生，`archi` crate 既有 `use serde::Serialize`）、`toml_edit` 0.25.15（既有；与 C1 同文件，本组件自身只用 `usize` / `Vec<String>`）；**零** `serde_json` 引用（序列化装配在 infra 侧，见 C4）、零新增 crate。
- **技术**：`Reorder { moved: usize, blocks: Vec<String> }`，`Debug` / `Clone` / `Default` / `PartialEq` / `Eq` / `Serialize`；`REORDER_BLOCKS_CAP = 8`。`Reorder::of(moved, blocks)` 在**构造处**截断 `blocks`（前 8 项、按文件序），`moved` 不动。零值形态恒为 `{"moved":0,"blocks":[]}`（`default()`），序列化 MUST NOT 省略字段。

### C4 · 工具响应面（infra）

- **职责**：把引擎事实序列化——两写工具的响应各加 `reordered`，零计数、零额外解析。
- **文件**：`crates/infra/agent/src/sdk/archi_tool.rs`（改，两处 `serde_json::json!`）。
- **依赖**：`archi::edit::WriteOutcome`（字段直取）、`serde_json`（既有）。
- **技术**：`element_response`（10 键 → **11** 键）与 `relation_payload`（薄形 8 → **9** 键）各加 `"reordered": &outcome.reordered`；落盘链（保护闸 → `write_atomic`）、E1/E2 前置、其余键与取值面逐字不变。

### C5 · 工具描述面（infra）

- **职责**：把「写入会把目标文件归一到规范序」告诉模型——它是工具语义，不是知识骨架内容。
- **文件**：`crates/infra/agent/src/sdk/mount.rs`（改：两处 `description` + 一处函数 doc 的「三件事 → 四件事」）。
- **依赖**：**无新增依赖**——只改 `archi_element_definition` / `archi_relation_definition` 的 `description` 里 `concat!` 字符串字面量与 `archi_element_definition` 的一处函数 doc；`parameters` 的 `serde_json::json!` 与 `WRITE_LIMITS` 零改动，不引入新 crate、不新增常量。
- **技术**：两工具的 `description` 各加**一句**（不拆成多句、不举字段形态示例、不教算法）：写入归一到规范序（子树连续 + 关系组连续）/ 手改拆开的子树会在下一次工具写时被合回 / 顺带搬家的块经响应 `reordered` 报出（`moved` 真值、`blocks` 有界）。`parameters`、`WRITE_LIMITS`、挂载清单 18 项、档位与注入矩阵**零改动**。

### C6 · 模型一次性搬迁（数据面）

- **职责**：把 `06-frontend.toml` 的 3 块历史错位在本变更内做一次纯搬迁，使首个功能写不夹带搬家。
- **文件**：`.dev-team/architecture/graph/model/06-frontend.toml`（改字节位置）。
- **依赖**：**零依赖**——纯文本位置搬迁，不经 `archi` 写面、不新增脚本 / 工具 / 流程；核对只借既有读面（`git diff --stat` 与 `archi_module` / `archi_drift` 的读数）。
- **技术**：18 行（`Desktop.Views.Changes.Flow` / `Renderers` / `Agent` 三块，各 5 行内容 + 1 空行）从 70–87 整体前移到 40（即紧接 `Desktop.Views.Changes` 块后的空行之后）；块内字节、元素 / 关系集合、表头计数、文件总行数（578）逐字不变（行效应见「数据模型」）。

### C7 · 口径面（零代码）

- **职责**：三处 spec 的手改出口口径与两工具描述的四件事——**已由本 change 的三份 delta 承载**（`desktop-archi-model` ×1 / `desktop-archi-agent` ×4 / `desktop-phase-prompts` ×1 的 MODIFIED 块）。
- **文件**：`openspec/changes/toml-sorting-and-layout/specs/**`（在案，本设计不改）、`crates/core/workflow/src/prompts/{proposal-planner,dev-design-planner}.md`（**零改动**）、`crates/infra/agent/src/archi-knowledge.md`（**零字节**）。
- **依赖**：**零依赖**——三份 delta 已在 `specs/` 在案，本设计只消费条款、不产生新文件与新建构输入；`archi-knowledge.md` 与两份 planner md 零字节改动，故无冻结值 / 门限需要调整。
- **技术**：md 侧的「工具表达不了的才手改」是简写，适用边界（批量整理 / 同层手排序）与「拆开会被合回」的裁定落在 spec 与工具描述；骨架 1777 B / 1960 B 冻结值与 `1800 / 2000` 门限不动（净余 23 B 不花、不抬锚）。

### C8 · 版本面

- **职责**：桌面端源改动的版本纪律。
- **文件**：`packages/desktop/package.json`（改）。
- **依赖**：**零依赖**——只改一个版本字符串；`tauri.conf.json` 经既有 `"version": "../package.json"` 间接跟随（本变更不触），`src-tauri/Cargo.toml` 的 crate 版本行独立、不跟随。
- **技术**：`version` 0.7.17 → **0.7.18**（+1 patch）；`src-tauri/Cargo.toml` / `tauri.conf.json` 不跟随。

## 变更清单 (Change Inventory)

### 新增文件

| 路径 | 说明 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/edit/layout.rs` | 写入序规范化的唯一实现（C1 算法六步）+ `Reorder` / `REORDER_BLOCKS_CAP`；零 IO、零 `Err` 臂、零诊断构造、零 `drift::check`、零 `EditErrorKind` 引用（签名见「公共函数 / API」，类型见「类型定义」） |

> 同批新增的 `edit/layout_test.rs` 属测试面（测试设计与用例归 test-design 相位），不在本清单展开；其**落位**在此登记：`edit/mod.rs` 底部与 `mod_test` / `messages_test` / `warnings_test` 同列的 `#[cfg(test)] mod layout_test;`。

### 修改文件

| 路径 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/edit/mod.rs` | ① 顶部 `mod layout;` + `pub use layout::{Reorder, REORDER_BLOCKS_CAP};`；② 四个 `apply_*`（`:217` / `:265` / `:330` / `:365`）返回类型 `Result<String, EditError>` → `Result<(String, Reorder), EditError>`，函数体在 `finish` 前插一行 `let reorder = layout::normalize(&mut document);`，收口改为 `finish(document, …).map(|content| (content, reorder))`（既有 `verify` 闭包逐字不动）；③ `WriteOutcome`（`:191`）末尾追加 `pub reordered: Reorder`；④ 四个 `plan_*`（`:393` / `:453` / `:520` / `:564`）解构 `apply_*` 的元组、`reordered` 进场（新建文件分支由 `body` 传 `reorder` 到 `new_file_content` 之后仍用原值）；⑤ 模块头 doc 的产出顺序句补「规范化」一环，并新增一段「写入序不变量」说明（E1/E2/R1 + 最小移动 + 不进自证链 + 零暴露面） | `finish` / 自证 / 回读 / 表头回填 / 三态 / 落点 / 级联 / `expect` **逐字不动**；`archi` 内仍零写盘 |
| `crates/core/archi/src/edit/mod_test.rs` | ① 既有 `apply_*` 调用点（110 处）机械随动（取元组文本：集中到模块级 helper 或 `.0`，断言对象仍是文本）；② `WriteOutcome` 字面构造（`:4400`）补 `reordered` 字段；③ R-2 的两处违例夹具归类：`RELATIONS_SOURCE`（`:310`，`A` 组分裂）在「逐字保留其余条目」一类断言上改判为「组连续化后仍逐字保留」，`关系缺席追加且缺键即建数组`（`:1281`，`:1296` 的「新条目在尾」）改判为「新条目落 `A` 组组尾」；健康夹具上的 `untouched == source` 类零差异断言一律保留在**合规序**夹具上，MUST NOT 用 `#[ignore]` 或删断言换绿 | 只改既有断言口径，不新增用例（用例归 test-design）；判据 = 夹具是否同时满足 E1/E2/R1 |
| `crates/core/archi/src/lib.rs` | `pub use edit::{…}`（`:60-64`）补 `REORDER_BLOCKS_CAP` 与 `Reorder`（rustfmt 序）；crate 根 doc 五条纪律零改动 | 出口形态沿既有单行 `pub use` 式，出口**名字集**只增两项 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | `relation_payload`（`:314` 一带）与 `element_response`（`:350` 一带）的 `serde_json::json!` 各加 `"reordered": &outcome.reordered`（元素面 10 → 11 键、`reordered` 写在该 `json!` 的末尾；关系面薄形 8 → 9 键、写在 `changed` 之后；线上键序不断言——`serde_json` 未开 `preserve_order`） | 保护闸 / 落盘段 / E1–E7 错误面 / 其余键逐字不变；零额外解析、零计数重算 |
| `crates/infra/agent/src/sdk/mount.rs` | ① `archi_element_definition` 的 `description`（`:465` 起 `concat!`）加第四件事（写入归一到规范序 / 手改拆开的子树会被合回 / `reordered` 报出），函数 doc（`:460`）的「只答三件事」改「四件事」；② `archi_relation_definition` 的 `description`（`:589` 起）加同一件事（关系面口径） | `parameters` / `WRITE_LIMITS` / 清单 / 档位 / 矩阵零改动；描述 MUST NOT 复述字段形态长表、MUST NOT 出现 `DFS` / `position` / `ArrayOfTables` 一类实现词 |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | ① 薄形键面断言（`关系写薄形无计数`，`:2219`）的期望键向量补 `"reordered"`（8 → 9 项，排序位在 `hash` 与 `to` 之间）——既有断言随契约机械随动；② 元素响应侧 `reordered` 的在场 / 零值 / 非零值判定归 test-design 的用例设计 | 本变更只登记既有断言的口径面；新用例不在此展开 |
| `crates/infra/agent/src/sdk/mount_test.rs` | 描述面正向断言（`描述不复述字段形态长表`，`:2410`）的要点集补第四项（写入归一到规范序 / `reordered`），既有三条要点与「零形态示例」断言逐字保留 | 同上（用例设计归 test-design） |
| `.dev-team/architecture/graph/model/06-frontend.toml` | 3 块搬迁：`Desktop.Views.Changes.Flow` / `.Renderers` / `.Agent` 三块（原 `:70-87`，18 行）整体前移到 `:40`（`Desktop.Views.Changes` 块后的空行之后）；块内字节、元素 / 关系集合、表头 `（32 元素 / 77 关系）`、总行数逐字不变 | 纯位置移动（C6）；`modelHash` 必变、`modelLine` 随文本走是**正确行为** |
| `packages/desktop/package.json` | `version` 0.7.17 → 0.7.18（+1 patch） | 版本纪律（C8） |

**登记为零改动（评审核对面，出现 diff 即误改）**：

| 对象 | 理由 |
|---|---|
| `crates/core/workflow/src/prompts/proposal-planner.md` / `prompts/dev-design-planner.md` | 现文「工具表达不了的才手改 `write` / `edit`」不含「重排」字样，与规范化相容（proposal「实现文件」表已登记本行为零改动） |
| `crates/core/archi/src/edit/messages.rs` | 零新增 `EditErrorKind`（规范化无 `Err` 臂）：23 值面、`ALL`、`criterion()` 逐字不变 |
| `crates/core/archi/src/parse/**` / `model.rs` / `graph.rs` / `drift/**` / `api/**` / `dto.rs` | 判据面 / 投影面 / 哈希面零变化；规范化 MUST NOT 重做形态校验、MUST NOT 跑 `drift::check`、MUST NOT 让 `DiagnosticKind` 出现新用法 |
| `crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs` | 骨架 1777 B / 全文 1960 B 冻结值与 `1800 / 2000` 双门限不动（净余 23 B 不花） |
| `crates/infra/agent/src/sdk/policy.rs` / `compose.rs` / `runner.rs` / `tools.rs` / `protect/**` | 工具数量（18）与档位（五读 + 两写）不变；上下文铸造判据、注入矩阵、保护闸规则集逐字不变 |
| 除 `06-frontend.toml` 外的五个模型文件、`graph/INDEX.toml`、`models/**`、`.i.rs` / `.i.ts` | 零字节：`01`–`05` 规范化是 no-op；`api:export` / `api:check` 应零 diff |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | 零 DTO 变化 ⇒ `bindings:export` 零 diff（**出现 diff 即误改**） |
| 前端产品代码（`packages/desktop/src/**`） | 零 UI 变化：`archi_set_gate` 命令面签名、交互与失败语义逐字不变 |
| `src-tauri/Cargo.toml` / `tauri.conf.json`、`AGENT.md` / `CLAUDE.md`、`openspec/specs/**` 基线、本 change 的 `explore.md` | 版本纪律 / 规范面 / delta-only 纪律 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `apply_element_spec` | `crates/core/archi/src/edit/mod.rs` | 修改 | `pub fn apply_element_spec(source: &str, spec: &ElementSpec) -> Result<(String, Reorder), EditError>` | 产出加 `Reorder`；元素字段与内联出边的写回语义、自证链逐字不变 |
| `apply_element_delete` | 同上 | 修改 | `pub fn apply_element_delete(source: &str, set: &DeleteSet) -> Result<(String, Reorder), EditError>` | 同上；删除侧仍只有这一个纯函数形态 |
| `apply_relation_upsert` | 同上 | 修改 | `pub fn apply_relation_upsert(source: &str, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>) -> Result<(String, Reorder), EditError>` | 同上；新条目仍先 append，规范化随后并回 `from` 组组尾 |
| `apply_relation_delete` | 同上 | 修改 | `pub fn apply_relation_delete(source: &str, from: &str, to: &str) -> Result<(String, Reorder), EditError>` | 同上 |
| `plan_element_upsert` | 同上 | 修改（签名不变、产出增字段） | `pub fn plan_element_upsert(root: &Path, spec: &ElementSpec) -> Result<WriteOutcome, EditError>` | `WriteOutcome.reordered` 由 `apply_*` 的 `Reorder` 直供，MUST NOT 自行比对前后文本重算 |
| `plan_element_delete` | 同上 | 修改（同上） | `pub fn plan_element_delete(root: &Path, fqn: &str, cascade: bool, expect: Option<&str>) -> Result<WriteOutcome, EditError>` | 同上 |
| `plan_relation_upsert` | 同上 | 修改（同上） | `pub fn plan_relation_upsert(root: &Path, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>, expect: Option<&str>) -> Result<WriteOutcome, EditError>` | 同上 |
| `plan_relation_delete` | 同上 | 修改（同上） | `pub fn plan_relation_delete(root: &Path, from: &str, to: &str, expect: Option<&str>) -> Result<WriteOutcome, EditError>` | 同上 |
| `Reorder::of` | `crates/core/archi/src/edit/layout.rs` | 新增 | `pub fn of(moved: usize, blocks: Vec<String>) -> Reorder` | 唯一构造点；在此施加 `REORDER_BLOCKS_CAP`（`blocks.truncate(8)`），`moved` 保留真值 |

> 本变更**不新增**任何 IPC 命令、agent 工具、CLI 子命令或 HTTP 端点：`apply_*` / `plan_*` 是 crate 内 Rust API，`Reorder` 是进程内类型，不出线。

**内部接线面**（`pub(super)` / 文件内私有；登记落位与签名，防实现分叉）：

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `normalize` | `crates/core/archi/src/edit/layout.rs` | 新增 | `pub(super) fn normalize(document: &mut DocumentMut) -> Reorder` | C1 算法全貌；调用点只有四个 `apply_*`（同一形态） |
| `source_order`（私有） | 同上 | 新增 | `fn source_order(document: &DocumentMut) -> Vec<Slot>` | 复刻 `last_position` 继承语义，产出跨面的文本序（`Slot` = 元素 FQN 或关系 `(from, to)`） |
| `element_target_order`（私有） | 同上 | 新增 | `fn element_target_order(fqns: &[String]) -> Vec<String>` | DFS + 同层首次出现序；父不在本文件元素集内即当顶级 |
| `relation_target_order`（私有） | 同上 | 新增 | `fn relation_target_order(pairs: &[(String, String)]) -> Vec<usize>` | 组连续 + 组间首次出现 + 组内源序（返回源下标序 = 重建 `ArrayOfTables` 的置换） |
| `minimal_moved`（私有） | 同上 | 新增 | `fn minimal_moved(slots: &[Slot], target: &[Slot]) -> Vec<Slot>` | LCS 补集；平局推进 T（保留更早的源项） |
| `blocks_of`（私有） | 同上 | 新增 | `fn blocks_of(moved: &[Slot], target: &[Slot]) -> Vec<String>` | 元素块按最上层被移动祖先归组、关系块按连续同 `from` 归段；按 T 内位置排序 |

### 类型定义

| 类型名 | 所在文件 | 新增 / 修改 | 说明 |
|---|---|---|---|
| `Reorder` | `crates/core/archi/src/edit/layout.rs` | 新增 | `struct { moved: usize, blocks: Vec<String> }`；`#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]`。`moved` = 本次搬迁的**块数真值**（永不截断）；`blocks` = 各块首元素 FQN（元素面）/ `from` FQN（关系面），按交付文本序、至多 `REORDER_BLOCKS_CAP` 项；`Default` = `{"moved":0,"blocks":[]}`（主路径形态） |
| `REORDER_BLOCKS_CAP` | 同上 | 新增 | `pub const REORDER_BLOCKS_CAP: usize = 8`——`blocks` 的字段级上限（数字单点；依据见 D-3） |
| `WriteOutcome` | `crates/core/archi/src/edit/mod.rs` | 修改 | 末尾追加 `pub reordered: Reorder`（字段声明序 `… counts, model, reordered`）；既有七字段与 `Debug/Clone/PartialEq/Eq` 派生不变 |

<!-- 配置：本变更不新增 / 不修改任何配置键（无 `config.json` 键、无环境变量、无构建特性），子表整段省略。`REORDER_BLOCKS_CAP` 是编译期常量，已登记在「类型定义」。 -->

## 数据模型 (Data Model)

### 模型文件的写入序（磁盘持久化形态的**不变量**，非新格式）

- **格式零变化**：TOML 结构、键形、字段序、`[[relation]]` 键集、首部注释块与表头计数括号全部不变；本变更不引入新键、不新增文件、不改目录布局。
- **写入后恒满足**（`E`=元素面，`R`=关系面）：

| 号 | 不变量 | 规范化如何达成 |
|---|---|---|
| **E1** | 父先于子 | DFS 目标序天然父在前；自由文本造出的「子先于父」被拉回 |
| **E2** | 子树连续 | 目标序把每个元素的全部后代排成紧邻区间 |
| **R1** | 同一 `from` 组连续 | 关系面按「组连续 + 组间首次出现 + 组内源序」重建 `ArrayOfTables` |
| — | 元素区恒在关系区之前 | 号分配 `1..=k`（元素）/`k+1..=N`（关系），混插形态被理顺 |

- **号分配**：`k` = **表形态**元素数（`[element."<FQN>"]`，`Item::Table`）；内联表形态的条目没有 `position`、不参与重排，渲染在 `element` 表的键区（射程见 C1）。元素区 `1..=k`，关系区 `k+1..=k+r`（`r` = 关系条目数）。三类交界形态因此有确定答案：**单元素文件**（`k = 1`、`r = 0`）⇒ 元素 `1`；**零关系文件**（`r = 0`）⇒ 元素 `1..=k`、关系区空；**混插文件**（`[[relation]]` 夹在 `[element.*]` 之间）⇒ 元素 `1..=k`、关系 `k+1..=k+r`，渲染时关系区整体落到元素区之后。
- **零改写面**：块内字节（键序 / 值形态 / decor / 行内注释 / 空行）、首部注释块（含人工标题）、表头计数（`backfill_header_counts` 仍是唯一权威，规范化不碰计数）、根尾装饰逐字保留。
- **幂等的可断言形态**（AC-4）：`changed = [] ∧ reordered.moved = 0` ⇔ `content` 与源逐字节相等；`moved > 0` ⇒ `content` 必变而 `changed` 仍可为空（首次把含历史错位的文件规范化）。这条等价的不变方向由第 3 步保证：`moved = 0` ⇔ 源序 == 目标序 ⇔ 规范化走快路径、零 AST 改动。

### 内存类型

```
WriteOutcome
├─ write:      ModelWrite { file: PathBuf(绝对), content: String, hash: "sha256:<hex>" }
├─ warnings:   Vec<EditWarning>
├─ changed:    Vec<String>
├─ created:    bool
├─ relations:  RelationTally { created, updated, deleted, unchanged }
├─ counts:     WriteCounts { elements, relations }        // 本文件（写后重解析）
├─ model:      WriteCounts { elements, relations }        // 全模型（前值 ± 增量）
└─ reordered:  Reorder { moved: usize, blocks: Vec<String> }   // 本变更新增，恒在场
```

- **序的读源**（实现约束，反直觉处）：`apply_*` 手边只有 `source: &str` 与 `&mut DocumentMut`，**源序只能从 AST 取**。`Model.elements` MUST NOT 用作源序——它由 `RawDocument.element: BTreeMap<Spanned<String>, RawElement>` 反序列化而来（`parse/toml.rs:23-28` / `:113`），而 `serde_spanned::Spanned<T>` 的 `Ord` 只比 `value`（`serde_spanned-1.1.3/src/spanned.rs:171-174`），按源码读解即 **FQN 字典序**、不是文本序（`model.rs:14` 的「文件序」措辞与实现不符；本变更不动那句，实现侧以乱序夹具的实际断言为准，改用 AST 这条约束不受影响）。`Model.relations` 是 `Vec<RawRelation>`，**保文本序**——AC-5 的「模型读出的关系序列 == 文本中的关系序」因此可断言。
- **不得借用 `ModelGraph::children`**：它 `sort_unstable()`（`graph.rs:122`），会把同层序变成字典序，摧毁「首次出现序」。DFS 的直接子判定在 `layout.rs` 内按 FQN 前缀现算。

### 线面响应（agent 工具输出，零 DTO 变化）

```jsonc
// archi_element（成功；十一键）
{ "warnings": [], "action": "upsert", "element": "Backend.Core.Billing",
  "file": ".dev-team/architecture/graph/model/02-backend-core.toml",
  "hash": "sha256:9c2e…", "created": true, "changed": ["kind", "summary"],
  "relations": { "created": 1, "updated": 0, "deleted": 1, "unchanged": 2 },
  "counts": { "elements": 10, "relations": 10 },
  "model":  { "elements": 56, "relations": 129 },
  "reordered": { "moved": 3, "blocks": ["Desktop.Views.Changes.Flow",
                                        "Desktop.Views.Changes.Renderers",
                                        "Desktop.Views.Changes.Agent"] } }

// archi_relation（成功；薄形九键——reordered 进场，仍无 counts / model / relations）
{ "warnings": [], "action": "upsert", "from": "…", "to": "…", "file": "…",
  "hash": "sha256:…", "created": true, "changed": ["desc"],
  "reordered": { "moved": 1, "blocks": ["Desktop.Views.Changes"] } }
```

- `moved = 0` 是主路径（健康文件规范化是 no-op），响应恒带 `"reordered": {"moved":0,"blocks":[]}`——**MUST NOT 因零值省略**。
- `blocks` 至多 8 项、按交付文本序取**前** 8；截断**不加**「…等 N 项」文本；`moved` 永不截断（`> 8` 时可从两者不等看出被截断）。
- 两写工具同形（元素面 / 关系面各一处装配），语义与 cap 归引擎（`desktop-archi-model`「写入序不变量」），执行体零重算。

### 一次性搬迁的行效应（`06-frontend.toml`，AC-10）

| 项 | 前 | 后 |
|---|---|---|
| `Views.Changes.Flow` 块 | `:70` | `:40` |
| `Views.Changes.Renderers` 块 | `:76` | `:46` |
| `Views.Changes.Agent` 块 | `:82` | `:52` |
| `Views.Config` / `DbInspector` / `Explores` / `Info` / `Welcome` 块 | `:40` / `:46` / `:52` / `:58` / `:64` | `:58` / `:64` / `:70` / `:76` / `:82` |
| `Views.Changes` 及其之前的块 | `:1-38` | 不变 |
| `:88` 及其之后（含 `:195` 起的 77 条 `[[relation]]`） | — | **行号零变**（删 18 加 18 都在其上方） |
| 文件总行数 / 表头 | 578 / `（32 元素 / 77 关系）` | 不变 |

⇒ `desktop-archi-canvas` 的 `06-frontend.toml:512` 示意仍指向同一条关系条目（`Desktop.Views.Config → Desktop.GeneratedBindings` 的 `to` 行）；`modelHash` 必变是**运行期事实**，不是判据变化。

### 持久化

- 无新增持久化：无新表、无新库字段、无新事件类型；模型落盘仍经 `write_atomic` 单点（infra 侧），`archi` crate 内零写盘。
- 零额外 IO：`normalize` 只吃已在内存的 `DocumentMut`；不新增读盘、不新增子进程。

## 路由 / API 设计 (Route / API Design)

> `auth`：不适用——本机进程内 agent 工具与 Tauri IPC 命令均无鉴权面；写面准入由既有两级闸表达（`policy::allows` 档位 + 成员校验）+ `ProtectRules` 保护闸，本变更零改动。

### IPC 命令面（零新增、零变化）

| method（Tauri command） | 入参 | 出参 | 变更 |
|---|---|---|---|
| `archi_set_gate` | `root`, `element`, `gate` | `Result<(), String>` | 零改动：仍取 `plan_element_upsert(...)?.write` 落盘；规范化内嵌在同一 `plan_element_upsert`，命令体零感知（首次写到含历史错位的文件时可能夹带搬家，`06` 已人工搬迁清零） |
| 其余八条读命令 | 不变 | 不变 | 零改动（命令清单仍九条、写命令仍两条） |

### agent 工具面（两写工具，响应增一字段）

| 工具 | 入参 | 出参 | 变更 |
|---|---|---|---|
| `archi_element` | 逐字段与上限**零改动** | 十一键（10 → 11，+ `reordered`） | 响应面加一字段；`parameters` / 档位 / 下发面 / E1–E7 错误面逐字不变 |
| `archi_relation` | 零改动 | 薄形九键（+ `reordered`） | 同上；仍不携带 `relations` / `counts` / `model` |

- **落盘链不变**：E1 入参解析与硬闸 → E2 结构诊断前置 → `plan_*`（零写盘）→ 目标文件过 `ProtectRules` → `write_atomic` → 响应装配 → `cap_result`。
- **无第三条通路**：不新增 `archi_sort` 一类工具（任何 `scope`、任何形态，含只读预览）；无新 IPC 写命令、无画布写入口。
- **零扩散**：`reordered` 只出现在 agent 工具响应，不进任何 Tauri 命令与 DTO；`bindings:export` 应零 diff。

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 用途 | 变化 |
|---|---|---|
| `toml_edit` 0.25.15（`core/archi` 既有） | AST 位置重编号（`Table::set_position`）与 `ArrayOfTables` 成员重排（`insert` / `remove` / `push`） | **零新增**（不升级、不加特性） |
| `serde` / `serde_json`（既有） | `Reorder` 的 `Serialize` 与工具响应装配 | 零新增 |
| `std`（`usize` / `Vec` / `String`） | LCS 与块归组（纯内存，无新 crate） | 零新增 |

### 构建 / 测试依赖

| 依赖 | 用途 | 变化 |
|---|---|---|
| `tempfile`（`core/archi` dev-dependency 既有） | 写面计划层与 06 形态夹具的临时模型目录 | 零新增 |
| 既有 nextest / 变异工具链 | `layout.rs` 的纯逻辑用例与变异守卫（口径归 test-design） | 零新增 / 零门限改动 |

## 决策留痕

| # | 决策 | 理由 / 边界 |
|---|---|---|
| D-1 | **规范化挂在四个 `apply_*` 内、`finish` 之前**，函数产出由 `String` 扩成 `(String, Reorder)` 元组；不新增 `Rewrite` / `Applied` 一类中间类型 | ① spec 只允许 `Reorder` 一个新类型（出口面最小）；② `Reorder` 必须由规范化自身产出——让 `plan_*` 比对前后文本重算是 spec 明文禁止的第二事实源；③ 元组把「文本 + 回报」的耦合写在签名上，零隐式状态 |
| D-2 | **「最小移动」= LCS 补集，平局推进 T（优先保留源序更早的项）**；`moved` = 归组后的块数 | 这是唯一能同时满足三件事的定义：① `moved = 0` ⇔ 源序 == 目标序 ⇔ 逐字节 no-op（AC-4 的等价方向；任何「按视角计数」的定义都给不出这个等价）；② `06` 报 3 而不是 8（`Config…Welcome` 被 LCS 保留、`Flow/Renderers/Agent` 记为搬迁）；③ 组分裂并回时报「后出现的那一段」（`A→X, B→Y, A→Z` ⇒ 报 `A`），与 R1 的「并回前组」措辞一致。反例说明为何不能用「只数上移的元素」：`[A, B, A.a, A.b]`（`A.a/A.b` 被追加到 `B` 之后）会报 2（两个子元素各算一块），而最小移动是 1（只把 `B` 挪到 `A` 的子树区间之后）——块数口径不该随「看谁」漂移。块归组：元素按最上层被移动祖先、关系按连续同 `from`。**实测**（六文件按此规则重算）：`01`–`05` 各 `moved = 0`；`06` `moved = 3` 且 `blocks` 恰三项 `Changes.Flow` / `.Renderers` / `.Agent` |
| D-3 | `REORDER_BLOCKS_CAP = 8` 在 `Reorder::of` 构造处施加，取**前** 8 项 | `WRITE_LIMITS.fqn = 256 B` 为 FQN 极值 ⇒ 最坏 `8 × 256 ≈ 2.0 KB`（占 `MAX_RESULT_BYTES = 50 KB` 的 4%），典型 FQN 下约 240 B；`moved` 已在场，再加「…等 N 项」是重复；取前 8 与 `git diff` 的首批 hunk 对得上。cap 只在引擎一处 ⇒ infra 侧零第二份截断逻辑 |
| D-4 | **源序只从 AST 取**（复刻 `last_position` 继承 + 稳定排序语义），不读 `Model`；DFS 的直接子判定在 `layout.rs` 内按 FQN 前缀现算，不用 `ModelGraph::children` | `Model.elements` 按源码读解为 FQN 字典序（`BTreeMap<Spanned<String>, …>`，`Spanned::Ord` 只比 value），与 proposal §二-3「`plan_*` 手里已有 `Model`（全文件元素序与 line）」的前提不符——**以代码事实为准**；`ModelGraph::children` 会 `sort_unstable()`，用了即把同层序变成字典序。两处偏离的落点：六文件重算（proposal 实测表复现）+「数据模型 · 序的读源」条 |
| D-5 | **`moved = 0` 走快路径：零 AST 改动直接返回** `Reorder::default()` | 健康文件的逐字节 no-op 不再依赖「重编号是否等价于原编号」这一间接推理，而是「什么都没动」；也把 AC-1（六文件逐字节相等）的失败面压到零 |
| D-6 | 规范化**不进自证链**、**无 `Err` 臂**、**零诊断 / warning**、**零手动入口** | 判据是「排序对解析无影响，目的只是长期保持 diff 整洁」。若在 `finish` 里断言序，一个美观 bug 就升级为写入失败（fail-closed 用错地方）；`DiagnosticKind` 更不能碰（`diagnostics` 非空即画布停摆，而 `06` 今天就有 1 处 E2 违例）。序由测试守 |
| D-7 | 关系面**真移动 `ArrayOfTables` 成员**并同批重编号，不靠 `Vec` 序与 `position` 序的任一单独面 | 只改一处会让渲染序与读回序打架（`relation_indexes` / `apply_inline_relations` 的 `(from,to)` 定位依赖 `Vec` 序）。AC-5 用「模型读出的关系序列 == 文本中的关系序」一条断言钉死；重建用 `remove(0)` + `push`（n ≤ 数十，O(n²) 可接受），成员内部字节零改写 |
| D-8 | `06-frontend.toml` 的一次性搬迁移到本变更内做**纯位置提交** | 若不搬，首个功能写会把 3 块夹带进无关 diff（R-7 两害相权）；搬后首个功能写是逐字节 no-op，AC-1 可断言。行效应已算清（关系区行号零变、`modelHash` 必变），且与「读数写实」纪律相容 |
| D-9 | 知识骨架与两份 planner md **零字节改动**；口径只落在 spec delta 与两工具 `description` | 骨架实测 1777 B（净余 23 B），现有句子已是「工具表达不了的才手改」，`重排` 零命中——本变更不需要花那 23 B，也不该抬锚；工具描述无预算且是「工具语义」的正当落点 |
| D-10 | 既有夹具按**行为**归类处置，不按「让测试变绿」处置：违例夹具改成合规序或改判断言为「并回组尾 / `moved ≥ 1`」；零差异断言留在健康夹具上 | R-2 的落地口径。`RELATIONS_SOURCE` 的 `A` 组分裂与 `关系缺席追加且缺键即建数组` 的「新条目在尾」是已知两处；其余夹具的归类由 test-design 逐条核对（判据 = 是否同时满足 E1/E2/R1）。MUST NOT 用 `#[ignore]` 或删断言换绿 |

## 验收标准回应

| AC | 设计回应 |
|---|---|
| AC-1 三不变量成立 | C1 算法六步 + D-2（实测六文件 `moved = 0`、未搬迁 `06` 夹具 `moved = 3` 且 `blocks` 恰三项）+ D-5 快路径（健康文件零 AST 改动 ⇒ 逐字节相等）+ C6（`06` 同批搬迁） |
| AC-2 规范序可断言 | C1 第 2 步（DFS + 同层首次出现序 / 组连续 + 组间首次出现 + 组内不动）+ 号分配 `1..=k` / `k+1..=N`（区域序恒成立，混插被理顺）；三类交界夹具的判据由该号分配直接给出 |
| AC-3 最小移动与幂等 | D-2（LCS 补集；`06` ⇒ 3 而非 8）+ D-5（no-op 快路径）+ 幂等性由「目标序是源序的函数、且目标序重算后不变」保证（同一输入排两次同结果） |
| AC-4 幂等承诺的可断言形态 | 「数据模型」的等价式：`changed = [] ∧ moved = 0` ⇔ 逐字节相等；`moved > 0 ⇒ content 必变而 changed 可空`。`moved = 0 ⇔ 源序 == 目标序` 由 LCS 定义保证（快路径使 ⇐ 更强） |
| AC-5 关系面两处同源 | D-7：成员重排 + 同批重编号；新建条目先 append、规范化随后并回 `from` 组组尾；组内相对序由「组内源序」保留 |
| AC-6 零暴露面 | C1「依赖」栏（零诊断构造、零 `drift::check`）+ D-6 + 零改动面（`DiagnosticKind` 九值 / `EditErrorKind` 23 值 / `ArchiDriftKind` 五值 / `ArchiApiFindingKind` 值集、`warnings` 不因搬迁新增） |
| AC-7 不进自证链 | C2：`finish` 与其 `verify` 闭包逐字不动；规范化在 `finish` 之前、无 `Err` 臂 ⇒ 含 E2 违例的源 + 一次合法 upsert 必 `Ok`（D-6） |
| AC-8 `reordered` 形状与上限 | D-3（cap 8、取前 8、`moved` 真值、零值恒在场）+ C3（`Reorder::of` 是唯一构造点）+ C4（薄形九键） |
| AC-9 零解析影响与零生成物 diff | 零改动面（`parse` / `graph` / `drift` / `api` / `dto` / 生成物 / 前端）+ 数据模型「零改写面」（计数仍由 `backfill_header_counts` 单点、表头与块内字节不动） |
| AC-10 `06` 搬迁的行效应 | 数据模型「一次性搬迁的行效应」表：`:88` 之后（含 77 条关系）行号零变、总行数与表头不变、`modelLine` 随文本走 |
| AC-11 spec delta 形态与旧句零残留 | 三份 delta 已在 `specs/` 定稿（本设计只消费条款，不改基线）；`desktop-phase-prompts` 的旧句判据由该 delta 的 Scenario 承载；产品源码面 C5 的两处 `description` 与 `mount.rs` 函数 doc 同批对齐（「重排文件」在正面表述中零出现） |
| AC-12 版本与改动集 | C8（0.7.18）+ 变更清单列全（含 `06` 搬迁、两处测试文件的口径随动）；`src-tauri/Cargo.toml` / `openspec/specs/**` 零改动 |

## 待决问题 (Open Questions)

| # | 事项 | 处置建议 |
|---|---|---|
| Q-1 | `blocks` 的命名观感两处：① 平局时名字给谁（如非法形态 `[C, P]` ⇒ 报 `P` 而不是 `C`）；② `from` 组分裂成 ≥3 段且中间只夹一条别的边时（`[A1, B, A2, A3]`），最小移动选择「把 `B` 挪到组后」（报 `B`）而不是「把后两段 `A` 拉回」（报 `A`）——两者文本结果相同 | 都不影响文本与 `moved` 数，只影响「名字给谁」。按 D-2 落地即可；spec 只钉文本序、`moved` 口径与「新边落组尾」，未钉这两种形态的名字。若评审要求 `blocks` 恒按「被并回的组」命名，需为关系面另加一条结构规则（代价：与元素面的最小移动口径分家） |
| Q-2 | 既有夹具中除已知两处外是否还有 E2/R1 违规夹具 | 由 test-design 逐条核对（判据 = 夹具是否同时满足 E1/E2/R1）；本设计只给处置规则（D-10），不列清单 |
| Q-3 | `REORDER_BLOCKS_CAP` 是否从 crate 根导出 | 建议导出（infra 侧与引擎共用一份数字，避免第二份字面量 8）；若不导出，`archi::edit::REORDER_BLOCKS_CAP` 仍可用，`lib.rs` 出口只加 `Reorder` |
| Q-4 | 两工具 `description` 第四句的逐字措辞 | 判据由 `mount_test` 承载（四组要点齐备、零实现词 `DFS` / `position` / `ArrayOfTables`、零字段形态示例）；措辞由实现定稿，超长先删冗余不删要点 |
| Q-5 | `06` 搬迁与本变更代码是否同一提交 | 建议**同一 change 内、独立一步**（tasks 阶段 5）：搬迁是纯字节移动，独立提交便于评审对照；两者同属本 change 的改动集（AC-12） |
