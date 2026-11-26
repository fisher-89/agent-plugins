# 任务: toml-sorting-and-layout

> 顺序即依赖：前一组解锁后一组。每项以文件为入口，落地口径见 `design.md`（决策 `D-1`…`D-10`）与 `specs/` 三份 delta。
> 测试用例与断言设计归 test-design 相位；本清单只到「实现 + 既有断言口径随动」。

## 阶段 1 · 规范化纯函数（`crates/core/archi`）

- [x] `edit/layout.rs`（新）：定义 `Reorder { moved: usize, blocks: Vec<String> }`（`#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]`）与 `pub const REORDER_BLOCKS_CAP: usize = 8`；唯一构造点 `Reorder::of(moved: usize, blocks: Vec<String>) -> Reorder` 在此对 `blocks` 施加 cap（`truncate(8)`，`moved` 保留真值——D-3）。
- [x] `edit/layout.rs`：`pub(super) fn normalize(document: &mut DocumentMut) -> Reorder` 按 design C1 六步落地——① `source_order` 复刻编码器的「自身 `position`，否则继承前表」语义（`Table::position() -> Option<isize>`，跨面共享 cursor）得交付前文本序；② `element_target_order`（DFS + 同层首次出现序，父不在**本文件**元素集内即当顶级，直接子按 `<父FQN>.` 且余段不含 `.` 现算——**MUST NOT 用 `ModelGraph::children`**，它 `sort_unstable()`，D-4）+ `relation_target_order`（组连续 + 组间首次出现 + 组内源序）；③ `minimal_moved` = LCS 补集，平局**推进 T**（保留源序更早项），`moved` 为空即**快路径**返回 `Reorder::default()`、零 AST 改动（D-2 / D-5）；④ `blocks_of` 归组（元素按最上层被移动祖先、关系按连续同 `from`，按目标序排序）；⑤ 元素区 `set_position(Some(i+1))`、关系区按目标序重建 `ArrayOfTables`（`remove` + `push`，成员整体搬迁）并 `set_position(Some(k + j + 1))`；⑥ 返回 `Reorder::of(块数, blocks)`。
- [x] `edit/layout.rs`：射程守住两条——只处理 `[element."<FQN>"]`（`Item::Table`；`Item::Value` 内联表形态原样保留、不参与重排）；`relation` 键缺席时 MUST NOT 建空条目数组（零关系文件零字节变化）。文件内零 `EditError` / `EditErrorKind` / `DiagnosticKind` 构造、零 `drift::check` 引用、零文件系统调用（AC-6 / AC-7）。
- [x] `edit/mod.rs`：顶部补 `mod layout;` 与 `pub use layout::{Reorder, REORDER_BLOCKS_CAP};`；底部与 `mod_test` / `messages_test` / `warnings_test` 同列补 `#[cfg(test)] mod layout_test;`（只声明落位，用例归 test-design）。
- [x] `archi/src/lib.rs`：`pub use edit::{…}` 补 `REORDER_BLOCKS_CAP` 与 `Reorder`（rustfmt 序）；crate 根 doc 五条纪律零改动（Q-3）。

## 阶段 2 · 写回原语族接线（`crates/core/archi/src/edit/mod.rs`）

- [x] `edit/mod.rs`：四个 `apply_*` 返回类型改 `Result<(String, Reorder), EditError>`——在 `finish(document, …)` **之前**插 `let reorder = layout::normalize(&mut document);`，收口改 `finish(document, …).map(|content| (content, reorder))`；`finish` / `verify_element` / `verify_inline_relations` / `verify_relation_count` / `verify_relation_absent` / `backfill_header_counts` / `recount` / `new_file_content` / `check_*` 前置族**逐字不动**（不进自证链，D-6）。
- [x] `edit/mod.rs`：`WriteOutcome` 末尾追加 `pub reordered: Reorder`（声明序 `… counts, model, reordered`），派生不变（C3）。
- [x] `edit/mod.rs`：四个 `plan_*` 解构 `apply_*` 的元组，`reordered` 直供 `WriteOutcome`（**MUST NOT** 自行比对前后文本重算搬迁块）；新建文件分支由 `body` 带出 `reorder`、经 `new_file_content` 之后仍用原值；`expect` 校验在规范化之前、`hash` 计算在文本定稿之后的既有链顺序零改动。
- [x] `edit/mod.rs`：模块头 doc 的产出顺序句改为「改 AST → **规范化** → 渲染 → 表头计数回填 → 重解析自证 → 回读」，并补一段写入序不变量说明（E1 / E2 / R1 + 最小移动 + 单文件 / 表形态射程 + 不进自证链 + 零暴露面 + 无手动入口）。

## 阶段 3 · 既有断言口径随动（`crates/core/archi/src/edit/mod_test.rs`）

> R-2 的落地：只改既有夹具与断言口径，MUST NOT 新增用例、MUST NOT 用 `#[ignore]` / 删断言换绿（D-10）。

- [x] `edit/mod_test.rs`：既有 `apply_*` 调用点（约 110 处）机械随动取元组文本（集中到模块级 helper 或就地 `.0`），断言对象仍是文本；`WriteOutcome` 字面构造（现 `:4400`）补 `reordered` 字段。
- [x] `edit/mod_test.rs`：`RELATIONS_SOURCE`（现 `:310`，`A` 组分裂）在「其余条目逐字保留」一类断言上按规范化后的文本改判（组连续化不改变块内字节）。
- [x] `edit/mod_test.rs`：`关系缺席追加且缺键即建数组`（现 `:1281`，`:1296` 的「新条目在尾」）改判为「新条目落 `A` 组组尾」；零关系文件「缺 `relation` 键即建数组」的既有断言保留（规范化不碰该键）。
- [x] `edit/mod_test.rs`：健康夹具上的 `untouched == source` 类零差异断言逐条保留在**合规序**夹具上；违例语义夹具统一改为合规序或改断言为「并回组尾 / `moved ≥ 1`」，不删除语义判据。

## 阶段 4 · 工具面（`crates/infra/agent/src/sdk`）

- [x] `sdk/archi_tool.rs`：`element_response` 与 `relation_payload` 的 `serde_json::json!` 各加 `"reordered": &outcome.reordered`（元素面 10 → 11 键、`reordered` 写在该 `json!` 末尾；关系面薄形 8 → 9 键、写在 `changed` 之后；线上键序不断言）；保护闸 / 落盘段 / E1–E7 错误面 / 其余键逐字不变（零额外解析、零计数重算）。
- [x] `sdk/mount.rs`：两工具 `description` 各加**一句**第四件事（写入归一到规范序 = 子树连续 + 关系组连续 / 手改拆开的子树会在下一次工具写时被合回 / 顺带搬家的块经响应 `reordered` 报出，`moved` 真值、`blocks` 有界）；`archi_element_definition` 的函数 doc（现 `:460`）「只答三件事」改「四件事」；`parameters` / `WRITE_LIMITS` / 清单 18 项 / 档位 / 矩阵零改动；描述零字段形态示例、零实现词（`DFS` / `position` / `ArrayOfTables`）。
- [x] `sdk/archi_tool_test.rs`：薄形键面断言（`关系写薄形无计数`，现 `:2219`）的期望向量补 `"reordered"`（排序位在 `hash` 与 `to` 之间，恰 9 项）——既有断言的机械随动；元素响应侧的 `reordered` 在场 / 零值形态判定归 test-design 的用例设计。
- [x] `sdk/mount_test.rs`：描述面正向断言（`描述不复述字段形态长表`，现 `:2410`）的要点集随描述改写补第四项，既有三条要点与「零形态示例」断言逐字保留——口径随动，用例设计归 test-design。

## 阶段 5 · 模型一次性搬迁（纯位置）

- [x] `.dev-team/architecture/graph/model/06-frontend.toml`：把 `Desktop.Views.Changes.Flow` / `.Renderers` / `.Agent` 三块（现 `:70-87` 共 18 行）整体前移到 `:40`（紧接 `Desktop.Views.Changes` 块后的空行之后）；块内字节、元素 / 关系集合、表头 `（32 元素 / 77 关系）`、文件总行数（578）逐字不变；`:88` 之后（含 `:195` 起的 77 条 `[[relation]]`）行号零变（D-8，AC-10）。
- [x] 搬迁后按 design「一次性搬迁的行效应」表逐项核对：`Flow` `:70→40`、`Renderers` `:76→46`、`Agent` `:82→52`；`Config`/`DbInspector`/`Explores`/`Info`/`Welcome` 各下移 18 行；`:512` 仍是 `Desktop.Views.Config → Desktop.GeneratedBindings` 的同一条目。

## 阶段 6 · 版本与收口自查（只读）

- [x] `packages/desktop/package.json`：`version` 0.7.17 → 0.7.18（+1 patch）；`src-tauri/Cargo.toml` 与 `tauri.conf.json` 不动（版本纪律）。
- [x] 零 diff 面核对：`api:export`（或 `api:check`）零差异；`bindings:export` 零 diff（零 DTO 变化——出现 diff 即误改）；除 `06-frontend.toml` 外五个模型文件零字节；`openspec/specs/**` 基线零改动。
- [x] 写面自查（走工具写，不手改）：对六个模型文件各做一次「把 `gate` 改成其当前值」的写回，确认产出与源逐字节相等且 `reordered.moved = 0`；对 `06` 搬迁前的临时副本（临时目录）验证 `moved = 3` 且 `blocks` 恰为 `["Desktop.Views.Changes.Flow", "Desktop.Views.Changes.Renderers", "Desktop.Views.Changes.Agent"]`（AC-1）。
- [x] 源码锚自查：`edit/layout.rs` 零 `DiagnosticKind` / `EditErrorKind` / `drift::` 构造；`EditErrorKind` 仍 23 值、`ALL` 与 `criterion()` 逐字不变；`mount.rs` / `archi_tool.rs` 的「三通道」与落盘单点措辞未失；`06` 搬迁后 `archi_drift` 的 `model.elements` / `model.relations`（55 / 129）与 `apiFindings` 构成逐项不变。
- [x] 改动集自查：`git status --porcelain` ⊆ proposal「变更范围」所列文件（另加本 change 目录的 `proposal.md` / `design.md` / `tasks.md` / `specs/**` / `reports/**`）+ 本设计登记的两处测试文件口径随动；`openspec/specs/**` 与 `src-tauri/Cargo.toml` 零改动（AC-12）。
