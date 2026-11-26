# 测试设计: toml-sorting-and-layout

> **日期**: 2026-11
> **依据**: `proposal.md`（AC-1–AC-12）+ `design.md`（C1–C8、D-1–D-10、变更清单「新增文件 / 修改文件 / 测试文件」、公共函数 / API、内部接线面、类型定义）
> **被测模块来源**: design.md「变更清单 → 新增文件」`edit/layout.rs`；「修改文件」`edit/mod.rs` / `archi/src/lib.rs` / `sdk/archi_tool.rs` / `sdk/mount.rs`。数据面（`06-frontend.toml`）、配置面（`package.json`）、文档面（两份 planner md、`specs/**`）无测试路径，登记在「不可测试项」。

---

## 测试框架与路径约定（框架识别结论）

- **框架**：Rust 内建 `cargo test` / nextest，测试与源文件**同目录、同模块**、文件名 `<源主干>_test.rs`；测试模块在源文件底部以 `#[cfg(test)] mod <主干>_test;` 声明（`edit/mod.rs:1703-1709` 的 `mod_test` / `messages_test` / `warnings_test` 即既有形态）。断言面为 `assert_eq!` / `assert!` / `matches!`，线面 / 结构化比对用 `serde_json::json!`；工具执行体用例为异步（`#[tokio::test]` + `ArchiContext::execute`），纯函数 / 描述面用 `#[test]`。
- **新测试文件的落位与可见性**：`edit/layout_test.rs` 由 `edit/mod.rs` 底部声明（design「新增文件」注记），故它是 `edit` 的**子模块**、与 `layout` 平级——只可达 `layout` 的 `pub` / `pub(super)` 项（`Reorder` / `Reorder::of` / `REORDER_BLOCKS_CAP` / `normalize`）。`source_order` / `element_target_order` / `relation_target_order` / `minimal_moved` / `blocks_of` 是 `layout` 私有项，**只能经 `normalize` 的产出（交付文本 + `Reorder`）观测**，用例不得新增导出为测试开路。
- **临时目录**：`archi` crate 零 dev-dependency（`Cargo.toml` 无 `[dev-dependencies]`，design「构建 / 测试依赖」表的 `tempfile` 记法与代码事实不符——以代码事实为准、不新增依赖）⇒ 沿既有 `edit/mod_test.rs` 装置（std 自建 `TempDir` RAII + `AtomicU64` 唯一命名）；`infra/agent` 侧用既有 `tempfile::TempDir`（`archi_tool_test.rs::tempdir`）。
- **仓库面只读**：需要「在盘六文件」事实时用 `CARGO_MANIFEST_DIR` 上溯六层取仓库根并只读（沿 `crates/core/archi/tests/pilot_test.rs::repository_root` 先例），零写盘。
- **Mock 纪律**（沿仓库既有）：Rust 侧零框架级 mock——`toml_edit` / `TomlParser` / `ModelGraph` / `drift::file_hash_of` / `ProtectRules` / `write_atomic` 一律真实组合；进程边界（模型目录、仓库文件）用真件 tempdir / 真件只读。仅当被测 API 显式入参为依赖时才用假件，本节无此情形。
- **路径口径**：「验收范围」表内「被测文件或模块」列省略 `packages/desktop/src-tauri/` 前缀（Rust 侧）；「单元测试」各节标题写全路径。

---

## 验收范围

> 每条 proposal 验收标准一行；「被测文件或模块」列填源文件（相对 `packages/desktop/src-tauri/`），对应测试文件见「单元测试」各节 `->` 标题。数据面 / 配置面 / 文档面（`06-frontend.toml`、`package.json`、planner md、`specs/**`、生成物）无独立测试路径，承载方式见「不可测试项」。

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 三条不变量成立：六个模型文件「把 `gate` 改成其当前值」的写回与源逐字节相等且 `reordered.moved = 0`；未搬迁 `06` 夹具（临时副本）结果为 `moved = 3`、`blocks` 恰三项 `Desktop.Views.Changes.Flow` / `.Renderers` / `.Agent` | `crates/core/archi/src/edit/layout.rs`、`crates/core/archi/src/edit/mod.rs`（数据面 `.dev-team/architecture/graph/model/06-frontend.toml` 见「不可测试项」） |
| AC-2 | 规范序定义可断言：E1（父先于子）+ E2（子树连续）+ 同层首次出现序；R1（`from` 组连续）+ 组间首次出现序 + 组内序不动；元素区恒在关系区之前；三类交界夹具（单元素 / 零关系 / 混插）各一条断言渲染全文逐字 | `crates/core/archi/src/edit/layout.rs` |
| AC-3 | 最小移动与幂等：健康文件 no-op（同输入排两次同结果、逐字节相等）；`moved` 只计被搬迁块（`06` ⇒ 3 而非 8）；重复同一写 ⇒ `changed = []` ∧ `moved = 0` ∧ 盘上字节零变化 | `crates/core/archi/src/edit/layout.rs`、`crates/core/archi/src/edit/mod.rs` |
| AC-4 | 幂等承诺的可断言形态：`changed = []` ∧ `reordered.moved = 0` ⇔ `content` 与源逐字节相等；`moved > 0` ⇒ `content` 必变而 `changed` 仍空；`expect` 校验在规范化之前 | `crates/core/archi/src/edit/mod.rs` |
| AC-5 | 关系面两处同源：`ArrayOfTables` 成员序 == 渲染文本序 == 重解析出的关系序；新边落 `from` 已有组组尾；组分裂并回后组内相对序不变 | `crates/core/archi/src/edit/layout.rs`、`crates/core/archi/src/edit/mod.rs` |
| AC-6 | 零暴露面：`DiagnosticKind` / `EditErrorKind` / `ArchiDriftKind` / `ArchiApiFindingKind` 值集逐字不变；`layout.rs` 零诊断构造、零 `drift::check` 引用；搬迁不新增 `warnings` 条目 | `crates/core/archi/src/edit/layout.rs`、`crates/core/archi/src/edit/mod.rs`、`crates/infra/agent/src/sdk/archi_tool.rs` |
| AC-7 | 不进自证链：`finish` / `verify_element` / `verify_inline_relations` 逐字不动；源码对「序断言」零命中（机械断言只在测试侧）；含 E2 违例的源 + 一次合法 upsert ⇒ `Ok` | `crates/core/archi/src/edit/mod.rs`、`crates/core/archi/src/edit/layout.rs` |
| AC-8 | `reordered` 形状与上限：字段恒在场（零值也写 `{"moved":0,"blocks":[]}`）；`moved` 真值永不截断；`blocks` ≤ 8 项且按文件序；>8 块夹具下成功响应 < 3 KB；两写工具响应都带该字段且 `archi_relation` 薄形恰 9 键 | `crates/core/archi/src/edit/layout.rs`、`crates/core/archi/src/edit/mod.rs`、`crates/core/archi/src/lib.rs`、`crates/infra/agent/src/sdk/archi_tool.rs` |
| AC-9 | 零解析影响与零生成物 diff：写后 `archi_drift` 结构诊断集合、元素 / 关系计数、`apiFindings` 构成逐项不变；`api:check` / `api:export` / `bindings:export` 零差异 | `crates/core/archi/src/edit/mod.rs`、`crates/core/archi/src/edit/layout.rs`（生成物与静态脚本面见「不可测试项」） |
| AC-10 | `06` 搬迁的行效应：关系区（原 88 行之后）行号零变化、`06-frontend.toml:512` 仍是同一关系条目、总行数与表头计数不变 | `.dev-team/architecture/graph/model/06-frontend.toml`（数据面，见「不可测试项」） |
| AC-11 | spec delta 形态与旧句零残留（恰三能力目录 / 六 MODIFIED 块 / Scenario 净增不减 / 「重排文件」只在禁令句）；两写工具描述四件事齐备且零实现词 | `crates/infra/agent/src/sdk/mount.rs`、`openspec/changes/toml-sorting-and-layout/specs/**`（spec 面见「不可测试项」） |
| AC-12 | 版本与改动集：`package.json` `version` +1 patch；`src-tauri/Cargo.toml` 逐字不变；改动集 ⊆ 提案「变更范围」；基线测试面零回归 | `packages/desktop/package.json`（配置面，见「不可测试项」）、`crates/core/archi/src/edit/layout.rs`、`crates/core/archi/src/edit/mod.rs`（回归面） |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/edit/layout.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/layout_test.rs

#### 待测功能

- `Reorder::of()`: 唯一构造点（design「公共函数 / API」表）——在构造处施加 `REORDER_BLOCKS_CAP`（`blocks` 取**前** 8 项、`moved` 保留真值）
- `Reorder` / `REORDER_BLOCKS_CAP`（design「类型定义」表）: `{ moved: usize, blocks: Vec<String> }`；`Default` = `{"moved":0,"blocks":[]}`（序列化 MUST NOT 省略字段）
- `normalize()`（design「内部接线面」表，`pub(super)`）: C1 算法六步全貌——源序复刻「自身 `position`，否则继承前表」语义、目标序（元素面 DFS + 同层首次出现序 / 关系面组连续 + 组间首次出现 + 组内源序）、LCS 补集与平局推进 T、块归组、`position` 重编号与 `ArrayOfTables` 重建、返回 `Reorder::of`；五个私有 helper（`source_order` / `element_target_order` / `relation_target_order` / `minimal_moved` / `blocks_of`）的行为经该函数产出观测（本测试模块与 `layout` 平级，不可直调私有项、也不为其新增导出）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `normalize` 健康文件 no-op（快路径） | 正向 | 六类形态夹具（单元素零关系 / 分层序父先于子 / 多 `from` 组 / 组内 `to` 手选序 / 大文件规模 / 只有内联表条目）逐一：返回 `Reorder::default()`（`moved = 0`、`blocks = []`）且 `document.to_string()` 与源逐字节相等（AC-1 / D-5） | 新增 |
| `normalize` 元素面目标序 | 正向 | DFS + 同层首次出现序：同层源序 `B` 在 `A` 前时保持 `B, A`（**不是**字典序）；每个元素的全部后代聚成紧邻区间（E1 + E2；AC-2） | 新增 |
| `normalize` 子树错位的最小移动 | 正向 | `[P, Q, P.A, P.B]` ⇒ 渲染文本恰为 `[P, P.A, P.B, Q]`（全文逐字），`moved = 1`、`blocks = ["Q"]`（D-2 的反例口径：不得报「两个子元素各一块」；AC-3） | 新增 |
| `normalize` 未搬迁 06 形态 | 正向 | `Views` 组末尾追加三个 `Views.Changes.*` 子块（父已有后代排更前）⇒ `moved = 3`、`blocks` 恰 `["…Changes.Flow", "…Changes.Renderers", "…Changes.Agent"]`（按交付文本序；AC-1） | 新增 |
| `normalize` 关系组分裂并回 | 正向 | `[A→X, B→Y, A→Z]` ⇒ 文本 `[A→X, A→Z, B→Y]`、`moved = 1`、`blocks = ["A"]`（R1 + D-2「并回后出现的那一段」；AC-5） | 新增 |
| `normalize` 组间 / 组内序 | 正向 | 组间先后 = 该 `from` 首次出现位置；组内 `to` 手选序原样保留（`GeneratedBindings` 提前形态）；AoT 尾 append 的同 `from` 新边归位到该组末成员（AC-2 / AC-5） | 新增 |
| `normalize` 区域序理顺 | 正向 | `[[relation]]` 夹在 `[element.*]` 之间的混插源 ⇒ 渲染后元素区全在关系区之前，断言渲染全文逐字（AC-2） | 新增 |
| `normalize` 号分配三类交界 | 正向 | 单元素文件（`k = 1`、`r = 0`）/ 零关系文件（元素 `1..=k`、关系区空）/ 混插文件（元素 `1..=k`、关系 `k+1..=k+r`）各一条：断言渲染全文逐字 + 重解析出的元素序 / 关系序；**不断言 `position` 取值**（R-3 口径；AC-2） | 新增 |
| `normalize` Vec 序与文本序同源 | 正向 | 重建后 `ArrayOfTables` 成员序 == 渲染文本中的关系序 == `TomlParser` 重解析出的 `model.relations` 序（AC-5 / D-7） | 新增 |
| `normalize` 幂等 | 正向 | 同一脏输入连排两次：第二次返回 `Reorder::default()`，两次渲染文本逐字节相等（AC-3） | 新增 |
| `normalize` 块内字节零改写 | 正向 | 被搬迁块的键序 / 值形态 / decor / 行内注释 / 块间空行 / 首部注释块与表头计数逐字保留（只动位置面；AC-2 / AC-9「零改写面」） | 新增 |
| `normalize` 平局推进 T | 正向 | `[C, P]`（子先于父）⇒ 文本 `[P, C]`、`blocks = ["P"]`（平局保留源序更早项，名字给 `P` 而非 `C`；D-2 / Q-1） | 新增 |
| `normalize` 多后代归一块 | 正向 | 一棵被移动子树含两个后代 ⇒ 归**一块**（取最上层被移动祖先）、`moved = 1`；关系面连续同 `from` 的两段并回报一块（D-2 块口径） | 新增 |
| `normalize` 仓库面只读 | 正向 | 六个 `.dev-team/architecture/graph/model/*.toml` 逐一 `normalize` ⇒ `Reorder::default()` 且逐字节相等（AC-1 的在盘事实；只读、零写盘） | 新增 |
| `Reorder::of` cap 与真值 | 边界 | `blocks` 长度 0 / 1 / 8 / 9 / 100 逐一：`blocks.len() <= REORDER_BLOCKS_CAP`、保留**前** 8 项且按入参序、`moved` 保留真值（含 `moved = 0` 与 `moved = 9 > blocks.len()` 的截断可见性）（AC-8 / D-3） | 新增 |
| `Reorder` 零值线面 | 边界 | `Reorder::default()` 序列化恰为 `{"moved":0,"blocks":[]}`（零值 MUST NOT 省略字段）；`REORDER_BLOCKS_CAP == 8` 为编译期单点（AC-8） | 新增 |
| `normalize` 射程：内联表形态 | 边界 | `element` 表内联值条目（`Item::Value`）原样保留、不参与号分配；同文件表形态条目仍编 `1..=k`（AC-2 射程） | 新增 |
| `normalize` 射程：`[element]` 显式表头 | 边界 | `element` 自身写成显式表头（自身有位号）⇒ 不进入重编号区间、文本稳定（快路径 no-op）（C1 附带形态边界） | 新增 |
| `normalize` 空 / 退化文档 | 边界 | 空文档（`""`）、只有 `[[relation]]` 的片段文件、`element` 表为空 ⇒ 不 panic、零重排、`moved = 0`（list / dict 的空值与缺失形态；`relation` 键缺席时 MUST NOT 建空条目数组） | 新增 |
| `normalize` 超长 / 特殊字符 FQN | 边界 | FQN 极值 256 B（design D-3 的 `WRITE_LIMITS.fqn` 口径）× 9 块 ⇒ `moved = 9`、`blocks` 恰 8 项；emoji 段 FQN 的块名原样进 `blocks`（str 极值与特殊字符边界；AC-8） | 新增 |
| `normalize` 零 `Err` 臂 | 异常 | 缺 `element` 键、`element` 为非表形态、`relation` 键形态异常 ⇒ 恒返回 `Reorder`（不 panic、不造诊断、不引入 IO、无失败分支）（AC-6 / AC-7） | 新增 |
| 源码锚：`layout.rs` 依赖面 | 异常 | `include_str!("layout.rs")` 零 `DiagnosticKind` / `EditErrorKind` / `drift::` / `fs::` / `spawn` / `sha2` / `assert!` 命中；`normalize` 签名无 `Result`、`Reorder::of` 为唯一构造点（AC-6 / AC-7） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 真实 `toml_edit::DocumentMut`（解析 + 渲染即被测面）、真实 `TomlParser`（同源断言）、真实 `serde_json`（`Reorder` 线面）；零替身 | 本节除「仓库面只读」外的全部用例 |
| 仓库模型文件（进程边界，只读） | 真实读盘：`CARGO_MANIFEST_DIR` 上溯六层取仓库根（沿 `tests/pilot_test.rs::repository_root` 先例），只读、零写盘 | 「仓库面只读：六文件 no-op」 |

---

### packages/desktop/src-tauri/crates/core/archi/src/edit/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs

#### 待测功能

- `apply_element_spec()`: 产出由 `String` 扩为 `(String, Reorder)`；规范化落在 `finish` 之前、元素字段与内联出边的写回语义与自证链逐字不变
- `apply_element_delete()`: 同上（删除侧仍只有这一个纯函数形态）
- `apply_relation_upsert()`: 同上；新条目仍先 append，规范化随后并回 `from` 组组尾
- `apply_relation_delete()`: 同上
- `plan_element_upsert()`: 签名不变、`WriteOutcome.reordered` 由 `apply_*` 直供（MUST NOT 自行比对前后文本重算）
- `plan_element_delete()`: 同上
- `plan_relation_upsert()`: 同上
- `plan_relation_delete()`: 同上
- `WriteOutcome`（design「类型定义」表）: 末尾追加 `pub reordered: Reorder`（声明序 `… counts, model, reordered`），既有七字段与派生不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `apply_element_spec` 健康夹具 no-op | 正向 | 健康夹具上把 `gate` 改成其当前值 ⇒ 文本与源逐字节相等、`reordered == Reorder::default()`（既有「零字节差异」断言改取元组文本；AC-1 / D-5） | 修改 |
| `apply_element_spec` 新建元素落子树中段 | 正向 | 父已有后代排在更前时新建子元素 ⇒ 文本收成子树连续、`moved >= 1` 且 `blocks` 含该 FQN 或被搬祖先（spec「新建元素」Scenario 的分情形断言；AC-1） | 新增 |
| `apply_element_spec` 新建落文件尾 | 正向 | 子树区间已是文件尾（健康夹具追加末位子元素）⇒ `moved = 0`（同上分情形另一臂；AC-1） | 新增 |
| `apply_element_delete` 删除后仍合规 | 正向 | 删子树中间元素 / 删一条造成组分裂的关系 ⇒ 剩余序已合规、`moved = 0`、删除块外字节逐字保留（含空行与注释） | 新增 |
| `apply_relation_upsert` 组分裂并回 | 正向 | `RELATIONS_SOURCE`（`A` 组被 `B→C` 分裂）改 `desc` ⇒ 文本 `A→B, A→C, B→C` 连续、各块字节逐字保留、`moved = 1`、`blocks = ["A"]`（R-2 归类为 R1 违例夹具；AC-5） | 修改 |
| `apply_relation_upsert` 新边落组尾 | 正向 | `RELATIONS_SOURCE` 上 `A→A` 新条目 append 后并回 `A` 组组尾（紧随 `A→C`，不在 AoT 尾）——既有「新条目在尾」断言改判（R-2；AC-5） | 修改 |
| `apply_relation_upsert` 零关系文件首条 | 边界 | 缺 `relation` 键的文件 upsert 首条边 ⇒ 新建条目数组、`moved = 0`、既有「缺键即建数组」断言逐字保留（AC-5 / 射程边界） | 回归 |
| `apply_relation_delete` 命中条删除 | 正向 | 删唯一命中条后剩余组连续、`moved = 0`、其余条目（含空行 / `desc` / `apis`）逐字保留（AC-5） | 修改 |
| 关系面两处同源 | 正向 | `apply_relation_upsert` / `apply_relation_delete` 后：`TomlParser` 读出的关系序 == 渲染文本中的关系序（Vec 序与文本序同批修改；AC-5 / D-7） | 新增 |
| `plan_element_upsert` 直供对拍 | 正向 | `WriteOutcome.reordered` 与同源 `apply_element_spec` 返回的 `Reorder` 逐字段相等（MUST NOT 重算——第二事实源即违约；AC-8） | 新增 |
| `plan_element_delete` / `plan_relation_upsert` / `plan_relation_delete` 直供对拍 | 正向 | 三个计划函数同上逐字段对拍，且各自覆盖 `moved = 0`（健康夹具）与 `moved > 0`（脏夹具）两态（AC-8） | 新增 |
| `plan_element_upsert` 未搬迁 06 副本 | 正向 | 临时目录内的「未搬迁 06」合成副本（`Views` 组后追加 3 块）改档成当前值 ⇒ `reordered.moved = 3`、`blocks` 恰三值、`write.content` 收成子树连续（AC-1 / AC-3） | 新增 |
| `plan_element_upsert` 幂等重放 | 正向 | 对写后文本再写一次同值（同一临时目录）⇒ `changed = []` ∧ `reordered.moved = 0` ∧ `write.content` 与盘上文本逐字节相等（AC-3 / AC-4） | 新增 |
| AC-4 等价形态 | 正向 | 脏副本首次写且字段零变化 ⇒ `changed = []` ∧ `moved = 3` ∧ `write.content != source`（首次规范化：文本必变而 `changed` 仍空；AC-4） | 新增 |
| `plan_element_upsert` 新建文件分支 | 边界 | `create = true` + 目标文件缺席 ⇒ `reordered == Reorder::default()`（新文件无历史错位）、`write.content` 含表头计数行、`created = true`（落点四情形的 `fresh` 臂；AC-8） | 新增 |
| `plan_element_upsert` `expect` 在规范化之前 | 异常 | 脏副本 + 过期 `expect` ⇒ `Err(ExpectMismatch)` 且零 `write` 产出（规范化不得先于乐观锁；AC-4） | 新增 |
| `EditErrorKind` 值集 | 异常 | 经 crate 根可达的 `EditErrorKind::ALL.len() == 23`、`criterion()` 逐值非空互异逐字不变；坏源 / 已存在 / 重复声明 / 内联边越界等既有 `Err` 臂逐条不变（零新增变体；AC-6） | 回归 |
| 含 E2 违例源 + 合法 upsert | 异常 | 源有子树错位（E2 违例）+ 一次合法元素 upsert ⇒ `Ok`（美观问题 MUST NOT 升级为写入失败；AC-7） | 新增 |
| 源码锚：自证链零触点 | 异常 | `include_str!("mod.rs")`：`finish` / `verify_element` / `verify_inline_relations` / `verify_relation_count` / `verify_relation_absent` / `backfill_header_counts` 体内零 `normalize` 引用；`layout::normalize` 调用点恰 4 处；`mod.rs` 与 `layout.rs` 零 `assert!` / `debug_assert!` 序断言（机械断言只在测试侧；AC-7） | 新增 |
| 结构诊断 / 计数零影响 | 回归 | 写后重解析的 `diagnostics` 集合、`counts` / `model` 计数与变更前逐项一致（搬迁不产诊断、不改计数；AC-6 / AC-9） | 回归 |
| 既有夹具按序归类 | 回归 | 逐夹具按「是否同时满足 E1 / E2 / R1」归类：`RELATIONS_SOURCE` 判为 R1 违例（见上两行改判）；`CORE_SOURCE` / `HEADER_SOURCE` / `FIELDS_SOURCE` / `COLLECTION_SOURCE` / `DELETE_SOURCE` / `APIS_SOURCE` / `RELATION_PLAN_SOURCE` / `TWO_ELEMENTS_SOURCE` / `SPLIT_FROM_SOURCE` / `SPLIT_RELATION_SOURCE` / `DUPLICATED_*` / `IDENTITY_*` 判为合规序，其 `untouched == source` 零差异断言逐条保留在合规夹具上（D-10 / R-2） | 回归 |
| 既有用例零删除 | 回归 | 既有 115 个 `#[test]` 的语义判据逐条保留（零 `#[ignore]`、零删断言）；约 110 处 `apply_*` 调用点机械改取元组文本（模块级 helper 或就地 `.0`）；`WriteOutcome` 字面构造补 `reordered`（D-10；AC-12 基线零回归） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 真实 `toml_edit` + 真实 `TomlParser`（自证 / 回读判据与解析层同源）+ 真实 `ModelGraph` + 真实 `drift::file_hash_of`；`apply_*` 为纯文本面，零替身 | 全部 `apply_*` 用例 |
| 磁盘模型目录（进程边界） | 真实 tempdir（std 自建 `TempDir` RAII + `AtomicU64` 唯一命名，沿既有 `mod_test` 装置），文件真件读写；`plan_*` 只断言 `write` 产物、不真落盘 | 全部 `plan_*` 用例、「未搬迁 06 副本」「幂等重放」 |

---

### packages/desktop/src-tauri/crates/core/archi/src/lib.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs

#### 待测功能

- `Reorder`（crate 根再导出）: `pub use edit::{…}` 补 `Reorder`，出口形态沿既有单行 `pub use` 式（design「修改文件」表 lib.rs 行 / Q-3 建议导出）；本项只测出口名集与可达性，类型语义归 `edit/layout_test.rs`
- `REORDER_BLOCKS_CAP`（crate 根再导出）: 同批补 `REORDER_BLOCKS_CAP`，与引擎共用一份数字、避免 infra 侧出现第二份字面量 8（design「修改文件」表 / D-3 / Q-3）

<!-- 说明：lib.rs 只作再导出口（`pub use edit::{…}` 补 `Reorder` 与 `REORDER_BLOCKS_CAP`），无函数体改动；出口名集的断言归 `edit/mod_test.rs` 既有「公开面 / 出口面 / 纪律锚」区。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| crate 根出口名集 | 正向 | `include_str!("../lib.rs")` 的 `pub use edit::{…}` 含 `Reorder` 与 `REORDER_BLOCKS_CAP`，既有 12 名逐字在案（沿既有 `crate根出口含十二个edit名字` 先例扩展期望集；AC-8） | 修改 |
| 出口名集只增两项 | 边界 | 名字集只增 `Reorder` / `REORDER_BLOCKS_CAP` 两项、零改名零删除；`EditErrorKind::ALL.len() == 23` 经 crate 根仍可达（AC-8 / AC-6） | 修改 |
| 编译面可达 | 边界 | `mod_test` / `layout_test` 经 crate 根导入 `archi::{Reorder, REORDER_BLOCKS_CAP}` 即可编译（任一名字离场即编译失败，不新增导出为测试开路） | 新增 |
| crate 根纪律零改动 | 回归 | `pub mod edit;` 登记在案、crate 根 doc 五条机械纪律逐字不变、`edit` 出口分组块形态在案（回归锚；design「零改动」面） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `include_str!("../lib.rs")` 源码锚 + crate 根导入的编译期面；零替身、零 IO | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs

#### 待测功能

- `element_response()`（私有响应装配，design「修改文件」表 `:350` 一带 / C4）: 元素面 `serde_json::json!` 末尾追加 `"reordered": &outcome.reordered`，键面 10 → 11，其余 10 键的取值与形态逐字不变
- `relation_payload()`（同表 `:314` 一带 / C4）: 关系面薄形 `json!` 在 `changed` 之后追加 `"reordered"`，键面 8 → 9，仍不携带 `relations` / `counts` / `model`
- 两工具响应键面（`archi_element` / `archi_relation`）: `reordered` 恒在场（零值也写 `{"moved":0,"blocks":[]}`，MUST NOT 省略）、`moved` 真值永不截断、`blocks` ≤ `REORDER_BLOCKS_CAP` 项且按交付文本序；失败路径零该键（AC-8）

<!-- 说明：design.md 未声明该文件的**公共** API 变更——三项都是私有响应装配面（`element_response` / `relation_payload` 的 `serde_json::json!` 各加一个字段），design 以 C4 与「修改文件」表登记、并把用例设计交给本相位。下列用例锚定 AC-8 / AC-6 与 design C4，不引用源码导出声明。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `archi_element` 响应键面 | 正向 | 成功响应键集恰 11 项且含 `reordered`；其余 10 键（`warnings` / `action` / `element` / `file` / `hash` / `created` / `changed` / `relations` / `counts` / `model`）取值与形态逐字不变（AC-8 / C4） | 修改 |
| `archi_element` 零值形态 | 边界 | 健康模型上改既有元素字段（不产生搬家）⇒ `reordered` 恰 `{"moved":0,"blocks":[]}` 且字段恒在场（MUST NOT 因零值省略；AC-8） | 新增 |
| `archi_element` 非零形态 | 正向 | 新建元素落在父子树中段（父已有后代排更前）⇒ `moved >= 1` 且 `blocks` 含该 FQN（或其被搬祖先），与盘上文本的子树连续一致（AC-1 / AC-8） | 新增 |
| `archi_element` 同值重放 | 正向 | 对写后文件重放同值 upsert（`gate` 写成当前值）⇒ `reordered.moved = 0`（AC-3） | 新增 |
| `archi_relation` 薄形键面 8 → 9 | 边界 | 既有「关系写薄形无计数」期望键向量补 `reordered`（恰 9 项，排序位在 `hash` 与 `to` 之间）；仍零 `counts` / `model` / `relations`；`warnings` 恒空数组、`created` / `changed` 取值不变（AC-8 / C4） | 修改 |
| `archi_relation` 搬迁语义 | 正向 | 新建边造成 `from` 组分裂（脏夹具）⇒ `moved >= 1`、`blocks` 为该 `from`；组连续化后文本序与 `reordered` 一致（AC-5 / AC-8） | 新增 |
| `reordered` 上限（cap 夹具） | 边界 | >8 块搬迁夹具（9 个错位子树或 9 段组分裂）⇒ `moved` 为真值 `> 8`、`blocks` 恰 8 项、成功响应 < 3 KB（AC-8 / D-3） | 新增 |
| 失败路径零 `reordered` | 异常 | E1–E7 各 `Err` 响应（值域 / 必填 / 端点 / 落点 / 乐观锁 / 保护闸）不出现 `reordered` 键，错误文案与既有键面逐字不变（AC-8 异常面） | 回归 |
| `warnings` 不因搬迁新增 | 异常 | 脏夹具搬迁（`moved > 0`）的 `warnings` 与搬前同形（关系面恒空数组，元素面七条判据子集不增项）（AC-6） | 新增 |
| 结果收口 | 回归 | 含 `reordered` 的成功响应仍经 `tools::cap_result`（`MAX_RESULT_BYTES = 50 KB`）收口且未触截断留痕（AC-8） | 回归 |
| 组合链路（入口 = 工具执行体） | 正向 | `archi_element` upsert → `archi::edit::plan_element_upsert` → `layout::normalize` → 保护闸 → `write_atomic`：脏夹具一次写后盘上字节 == `plan` 的 `content` == 规范化序，响应 `moved >= 1`；再重放一次 ⇒ `moved = 0` 且盘上字节零变化（AC-1 / AC-3 / AC-8 的端到端同源） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 磁盘模型目录（进程边界） | 真实 tempdir（既有 `tempdir` / `write_model` 装置）+ 真件 `ProtectRules::load` + 真实 `atomic::write_atomic`（落盘单点即被测链的一环） | 本节全部用例 |
| 无（真实组合） | 真实 `archi::edit::plan_*` / `layout::normalize` / `serde_json` 响应装配 / `tools::cap_result`；工具执行体真实 `ArchiContext::execute`，零替身 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

#### 待测功能

- `archi_element_definition()`（design「修改文件」表：函数 `:462`、`description` `:465` 起、函数 doc `:460` / C5）: `description` 的 `concat!` 加第四件事——写入归一到规范序（子树连续 + 关系组连续）/ 手改拆开的子树会在下一次工具写时被合回 / 顺带搬家的块经响应 `reordered` 报出；函数 doc 的「只答三件事」改「四件事」，两处同批对齐
- `archi_relation_definition()`（同表：函数 `:586`、`description` `:589` 起 / C5）: `description` 加同一件事的关系面口径（同为一句话，不拆多句、不举字段形态示例、不教算法）
- 描述面零实现词与 `parameters` 零改动面（C5）: 两 `description` 零 `DFS` / `position` / `ArrayOfTables` / `LCS` / `truncate` 一类实现词；`parameters` 的 `serde_json::json!`、`WRITE_LIMITS`、挂载清单 18 项、档位与注入矩阵逐字不变

<!-- 说明：design.md 未声明该文件的**公共** API 变更——改动是两处 `description` 的 `concat!` 字面量（各加第四件事）与 `archi_element_definition` 的函数 doc；`parameters` / `WRITE_LIMITS` / 清单 / 档位 / 矩阵零改动。下列用例锚定 AC-11 与 design C5，不引用源码导出声明。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 描述面第四件事齐备 | 正向 | 两写工具 `description` 各含第四项要点——写入归一到规范序（子树连续 + 关系组连续）/ 手改拆开的子树会在下一次工具写时被合回 / 顺带搬家的块经响应 `reordered` 报出（`moved` 真值、`blocks` 有界）；既有三条要点（坏文本不可产出 / `expect` ↔ `hash` / `cascade` 射程）逐字保留（AC-11 / C5） | 修改 |
| 描述面零实现词 | 边界 | 两 `description` 零 `DFS` / `position` / `ArrayOfTables` / `LCS` / `truncate` 一类实现词，零字段形态示例（既有 `kind = ` / `[[relation]]` / `path = [` / `metadata = {` 断言逐字保留）（C5 / Q-4） | 新增 |
| 函数 doc 随动 | 边界 | `archi_element_definition` doc 的「只答三件事」改「四件事」，与 `description` 的四项要点同批对齐（不得只改一处）（AC-11 / C5） | 修改 |
| `parameters` 零改动 | 回归 | `WRITE_LIMITS` 数字单点、`maxLength` / `maxItems` / `maxProperties` 恒为常量引用、`required` 列表与 `enum` 派生逐字不变（C5 零改动面） | 回归 |
| 清单 / 档位 / 矩阵零改动 | 回归 | 工具数量 18 项零重复、五读 + 两写档位、`archi_matrix` 与写权锁对齐断言逐字不变（design「零 delta 的能力」`desktop-agent-execution`） | 回归 |
| 旧句零正面表述 | 异常 | `mount.rs` 与两 `description` 内「重排文件」在**正面表述**零出现（只在禁令句 / 现状引述允许；AC-11 旧句判据） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `mount::registry()` 真实定义读取（`lookup(name)` 取 `description`）+ `include_str!("mount.rs")` 源码锚；零替身、零 IO、零进程边界 | 本节全部用例 |

---

## 不可测试项

- **`.dev-team/architecture/graph/model/06-frontend.toml`（数据面）** — 纯位置搬迁（18 行 / 3 块整体前移），无单元测试路径；AC-1 / AC-9 / AC-10 的行效应、`modelHash` 变化与「`:512` 仍是同一关系条目」由 tasks 阶段 5–6 的写面自查 + `git diff --stat` + `archi_drift` / `archi_module` 读数承载，不由测试断言真实文件。自动化侧以**合成副本**（临时目录内的「未搬迁 06」形态）在 `edit/layout_test.rs` 与 `edit/mod_test.rs` 钉住 `moved = 3` 与 `blocks` 三值；真实六文件的在盘 no-op 面另由 `layout_test.rs` 的只读仓库面用例承载。
- **`packages/desktop/package.json`（版本配置面）** — 只改一个 `version` 字符串（0.7.17 → 0.7.18），无测试路径；AC-12 的 +1 patch 与 `src-tauri/Cargo.toml` 逐字不变由版本纪律静态检查 / diff 面承载。
- **`crates/core/workflow/src/prompts/{proposal-planner,dev-design-planner}.md`（零字节改动）** — 纯文档面，无测试路径；「零改动」由 `git diff` 空承载（AC-12 改动集）。同批 `archi-knowledge.md` 的 1777 B / 1960 B 冻结值不在本变更范围（design「零改动」面）。
- **`openspec/changes/toml-sorting-and-layout/specs/**`（spec delta 文档面）** — AC-11 的形态判据（恰三能力目录 / 六 MODIFIED 块 / `### Requirement` 标题逐字一致 / Scenario 净增不减 / WHEN-THEN 成对 / `## Module Contract` 在场 / 旧句只在禁令句）由 spec 评审与机械步骤承载，无单元测试路径。
- **生成物与静态脚本面（`bindings:export` / `api:export` / `api:check` / `client:check`、`packages/desktop/src/types/generated/bindings*.ts`、`.dev-team/architecture/**` 的 `.i.rs` / `.i.ts` / `INDEX.toml` / `models/**`、除 `06` 外五个模型文件零字节、`src-tauri/Cargo.toml` 与 `tauri.conf.json`）** — AC-9 / AC-12 的零 diff 判据是机械步骤（静态脚本 / diff 面），由桌面端执行、agent 内不重复执行，不在单元测试范围。
