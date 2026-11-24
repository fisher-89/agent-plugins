# 测试设计: archi-edit-engine-primitives

> **日期**: 2026-11-24
> **输入**: 本 change 目录 `proposal.md`（AC-1…AC-13 / R-1…R-12）、`design.md`（**第二轮修订版**：「变更清单」新增文件 2 行 + 修改文件 4 行 + 删除文件 2 行、「公共函数 / API」22 行、「类型定义」子表、D1–D19、事实校准 14 条、末节「交接给下游相位的随动清单」4 行 + 紧接其后的「`test-design.md` 面的随动」注）、`tasks.md`（阶段一 / 二 / 三 / **四**的实现点与自查面）、`specs/` 三份 delta（`desktop-archi-model` / `desktop-archi-drift` / `desktop-config-agent`）。
> **性质**: 纯引擎演进——`archi::edit` 由单函数泛化为写回原语族（4 × `apply_*` + 4 × `plan_*`），文案族抽为封闭枚举（**19 值**，含回溯修订新增的 `RelationDuplicated`），`drift` 补两个单文件哈希薄封装（`hash_files` 加私有 `read` 形参升为唯一累加核心，`hash_with` 不取）。被测面 = **8 个公开函数 + 4 个公开类型 + `drift` 两个新函数 + 壳层一条命令的内部接线**；测试文件净新增为 0——`edit_test.rs` 退役删除，同批新增 `edit/mod_test.rs`（迁移并扩写）与 `edit/messages_test.rs`（新），`drift/mod_test.rs` / `src/commands/archi/mod_test.rs` 为既有文件扩写。
> **回溯修订（第二轮）的五处随动**: ① 错误面 **18 → 19 值**（`RelationDuplicated`，模型面 / 文件面两臂 facts）；② 关系落点判据（design D18）——既有 `(from, to)` 经 `model.relations` 的 `file` 单点定位并原地改 / 删，**仅新增关系落 `from` 元素文件**，重复声明 fail-closed；③ `hash_with` **退役**（design D9 修订）——`hash_files(files, base, read)` 三形参 + 私有 `read_bytes` 为唯一拼装 / 字节来源单点；④ 表头回填补第五例「回填后正文不截断」；⑤ 壳层 6 条新增用例按 design D19 归本相位并入（见 `src/commands/archi/mod_test.rs` 节）。
> **路径口径**: `crates/**` / `src/**` 均指 `packages/desktop/src-tauri/` 之下；本文一律写仓库相对全路径。`edit.rs` 与 `edit_test.rs` 是**退役删除**，不纳入被测范围（design「删除文件」表）。
> **框架识别**: Rust 单元测试（`framework = rust`，见 `.dev-team/config.json`）——`#[test]` 属性宏 + `assert!` / `assert_eq!` / `expect_err` 的 std 断言，**零第三方断言库**；测试文件与源文件同目录共置 `*_test.rs`，由 `#[cfg(test)] mod <name>_test;` 在 `lib.rs` / `<dir>/mod.rs` 内登记；`archi` crate **零 dev-dependencies**，临时目录由 `std` 自建（`TempDir` RAII + `AtomicU64` 计数器，沿 `edit_test.rs` / `drift/mod_test.rs` 先例）。壳层为 `#[tokio::test]`（异步命令）+ `tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）。测试运行与覆盖率 / 变异由桌面端 test-execution 执行，本文不复制命令细节。
> **同批约束**: `edit.rs` / `edit_test.rs` 的删除与 `edit/` 两个新增文件必须同批（`edit_test.rs` 的 `use crate::edit::{plan_set_gate, set_gate}` 在旧名离场后即编译失败）；`#[cfg(test)] mod mod_test;` / `mod messages_test;` 由测试相位在落地对应文件时同批补入 `edit/mod.rs`（design D1）。
> **用例名口径**: 沿用既有中文用例名 + 小写 ASCII 标识符（如 `plan_element_upsert`）；需要 ASCII 驼峰时补 `#[allow(non_snake_case)]`（`src/commands/archi/mod_test.rs` 既有写法）。用例名不写测试命令、不写断言细节。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 13 条 AC。被测文件或模块 = 承载用例的测试文件；
     仓级读数 / 生成物 / 一行字面量 / spec 文本面落「—（见不可测试项 N）」，并写明替代保证。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 公开面恰为原语族：8 函数（4 `apply_*` + 4 `plan_*`）签名与提案表逐字一致 + 4 类型（`ElementSpec` / `ModelWrite` / `EditError` / `EditErrorKind`）；`mod messages` 私有 ⇒ 无 `edit::messages::EditError` 第五路径；crate 根 12 个新出口名齐；生产面 `set_gate`（排除 `archi_set_gate*`）/ `plan_set_gate` / `GateWrite` 零命中 | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（8 签名 + 2 类型 + `^pub fn` / `^pub use` / `mod messages;` 源码锚）、`packages/desktop/src-tauri/crates/core/archi/src/edit/messages_test.rs`（`ALL` 恰 **19** 值 + 2 类型）、`packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（crate 根 `lib.rs` 出口面锚）；全仓生产面 grep 见不可测试项 2 |
| AC-2 | `archi_set_gate` 行为等价：同一临时模型改档 ⇒ 落盘文本与「只把目标行 `gate` 值换掉」的期望文本逐字节相等（按行 diff 恰 1 行）；命令签名 / `Result<(), String>` 错误面 / 失败零写入逐条不变；gate 改档 = `ElementSpec { gate }` 的一处调用（语义等价靠此条断言） | `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（按行 diff + 签名 + 零写入；**6 条新增用例按 design D19 并入**）、`packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（`plan_element_upsert` 的 gate 退化调用 + 幂等零差异） |
| AC-3 | 三态逐条成立：`entry` / `doc` / `summary` / `path` / `metadata` / 关系 `apis` 六个可清空位 `None` 不动 / `Some(None)`（或 `Some(空集合)`）删键 / `Some(Some(v))` 设值；`gate` / `kind` / `api` 三个封闭枚举位无清空臂（`Some(GateTier::Off)` = 显式化缺省）；反向：写面 MUST NOT 产出 `apis = []` 一类被 parser 判 `invalidValue` 的形态 | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` |
| AC-4 | `expect` 乐观锁：`Some(当前 file_hash)` ⇒ `Ok`；`Some(错值)` ⇒ `Err` 且零文本产出；目标文件缺席 + `Some(_)` ⇒ `Err`；`None` ⇒ 不校验；四个 `plan_*` 共用同一校验点（无绕过分支）；关系面 `expect` 绑**落点文件**——既有关系 = 该条目的声明文件 / 新增关系 = `from` 元素文件（design D18），随落点走而非随 `from` 走 | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（四个 `plan_*` 逐入口）、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs`（`file_hash_of` ↔ 落盘 `file_hash` 等价锚——`expect` 闭环的前提） |
| AC-5 | 表头回填可判：括号内数字 == 该文件重解析的 `elements` / `relations` 条数；括号外整行逐字不变；无该括号 ⇒ 不新增；首部注释块之外同形文本零改动；**回填后首部注释块之后的正文逐字保留（正文不截断）**；本仓六个模型文件「gate 改成当前值」plan ⇒ 与源逐字节相等（计数准确 ⇒ 零差异） | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（五例 + 合成 fixture 的零差异臂）、`packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（命令 → plan → apply → `write_atomic` 全链零差异）；本仓六文件见不可测试项 3 |
| AC-6 | 自证链闭合：结构诊断非空的模型 ⇒ 四个 `plan_*` 全 `Err`；渲染后重解析出诊断 ⇒ `Err` 且零文本；回读逐字段不一致（**关系面另含条数**：既有 ⇒ 不变 / 追加 ⇒ +1 / 删除 ⇒ −1）⇒ `Err` 且零文本；`apply_*` 内部顺序固定「改 AST → 渲染 → 表头回填 → 重解析 → 回读」，被自证 / 被回读的文本逐字等于交付 `content` | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` |
| AC-7 | 文案族封闭且操作中立：`ALL` 与 `criterion()` 的穷尽 `match` 分支集合逐值相等；逐值非空、互异、零「改档」；`message()` 形态恒为「判据句：事实」；基线 10 条文案逐条归宿（逐字 4 / 语序落位 2 / 改词 5）；**`RelationDuplicated`（19 值中的第 17）无基线来源**，其模型面 / 文件面两臂 facts 由关系重复用例钉住；既有断言随动（`拒绝改档` → `拒绝写回`） | `packages/desktop/src-tauri/crates/core/archi/src/edit/messages_test.rs`（**19** 值对拍 + 文案表 + 形态）、`packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` 与 `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（needle 随动 + 两臂 facts） |
| AC-8 | 哈希单源：`file_hash` 形态 `sha256:` + 64 位小写 hex、同盘稳定、内容变即变、文件名参与累加（同名同内容跨目录相等 / 异名同内容不等）、与 `modelHash` / `assets_hash` 数值不可比；`file_hash_of` 与落盘后 `file_hash` 逐字相等；`ModelWrite.hash` 即该值；`edit/` 内零 `Sha256` / `sha2` 字面；两函数都**字面经唯一累加核心** `hash_files(files, base, read)`（`hash_with` 不取）、私有 `read_bytes` 是「读失败按空字节」单点 | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs`、`packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（`ModelWrite.hash` 等价锚） |
| AC-9 | 删除 fail-closed：有已声明子元素 ⇒ `Err` 点名列出的子元素 FQN；仍被任一关系引用（出边或入边）⇒ `Err` 点名关系对；无从属叶元素 ⇒ 删除成功、表头计数随动下降、其余内容逐字保留 | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` |
| AC-10 | 关系原语：**新关系（模型面 0 处）落 `from` 元素声明文件；既有 `(from, to)` 经 `model.relations` 的 `file` 单点定位并原地改 / 删——声明文件可以不是 `from` 元素文件（design D18）**；既有走更新（回读条数不变）、在场删除成功（条数 −1、计数随动）；重复声明（模型面跨文件 ≥ 2 处 / 同一源内 ≥ 2 处）⇒ `RelationDuplicated` fail-closed 零文本；`desc` / `apis` 三态成立且 `Some(&[])` 不发空数组；越界 `apis` 值由重解析自证拒绝（写面零值面判据）；端点缺席 ⇒ `Err` 点名缺失端；**模型面关系缺席 ⇒ `RelationNotFound`（facts 列该 `from` 的出边对，不读 `from` 文件后由 apply 报「该文件没有关系」）** | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` |
| AC-11 | 生成物与读数零变化：`bindings.ts` / `bindings.zod.ts` 零 diff、`api:check` 零差异退出、`api:export` 对 `INDEX.toml` / `.schema.json` / `.i.*` 零 diff、`crates/core/archi/tests/**` 语料与金样逐字节不变、报告读数 55 / 129 / 0 / 0 / 39 / 53 / `clean` / `modelHash` 逐字不变 | —（见不可测试项 4：生成物与仓级读数面） |
| AC-12 | 版本与改动集：`packages/desktop/package.json` `version` 恰 +1 patch、`tauri.conf.json` 仍 `"version": "../package.json"`、`src-tauri/Cargo.toml` 逐字不变、`git status --porcelain` 改动集 ⊆ 提案白名单（两次删除与 `edit/` 两新增同批） | —（见不可测试项 1：一行字面量与改动集面） |
| AC-13 | spec delta 形态：`specs/` 恰三个能力目录、RENAMED 的 `FROM:` / `TO:` 与基线标题逐字一致、MODIFIED 块为基线整段拷贝后改、每条 `### Requirement` 至少一个 `#### Scenario:`、`## Module Contract` 段齐备 | —（见不可测试项 5：spec 文本面） |

**源文件 → 测试文件映射**（design「变更清单」新增文件 2 行 + 修改文件 4 行，逐行给出；删除文件不纳入）：

| 源文件（design 新增 / 修改） | 测试文件 | 映射依据 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/edit/mod.rs`（新增） | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` | 目录化模块共置测试文件，`edit/mod.rs` 内 `#[cfg(test)] mod mod_test;` 登记（design D1）；由 `edit_test.rs` 迁移并扩写 |
| `packages/desktop/src-tauri/crates/core/archi/src/edit/messages.rs`（新增） | `packages/desktop/src-tauri/crates/core/archi/src/edit/messages_test.rs` | 同上，`#[cfg(test)] mod messages_test;` 登记（D1） |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`（修改） | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs` | 既有 `#[cfg(test)] mod mod_test;` 已登记，`file_hash` 族与 `hash_files`（三形参）/ `read_bytes` 私有面的用例落在既有文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（修改） | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` | 纯出口整理无函数面；出口面锚经 `include_str!("../lib.rs")`，沿 `api/mod_test.rs` 先例（本 crate 无 `lib_test.rs` 惯例） |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs`（修改） | `packages/desktop/src-tauri/src/commands/archi/mod_test.rs` | 既有共置测试文件（`#[cfg(test)] mod mod_test;`，壳层同目录 `mod_test.rs`） |
| `packages/desktop/package.json`（修改） | —（无测试文件） | 版本号一行，无行为可断言 ⇒ 见不可测试项 1 |
| （退役删除，不纳入）`crates/core/archi/src/edit.rs` / `crates/core/archi/src/edit_test.rs` | — | design「删除文件」表：删除与 `edit/` 两个新增文件同批（AC-12），用例语义迁入 `edit/mod_test.rs` |

---

## 单元测试

<!-- 框架与装置（落地前必读）：Rust `#[test]` + std 断言宏，零第三方断言库；`archi` crate 零
     dev-dependencies ⇒ 临时目录用 std 自建 `TempDir`（RAII + AtomicU64 计数器，沿 edit_test.rs /
     drift/mod_test.rs 先例）。测试文件与源文件同目录共置，目录化模块的测试文件由 `<dir>/mod.rs`
     内的 `#[cfg(test)] mod <name>_test;` 登记——本变更的两个新测试文件需在 `edit/mod.rs` 末尾
     补 `#[cfg(test)] mod mod_test;` 与 `#[cfg(test)] mod messages_test;`（design D1：声明后置到
     测试相位，避免测试文件未落地时 `cargo test` 解析失败）。

     迁移面（design 末节交接清单第 1 行）：`edit_test.rs` 的 14 条 `#[test]` 迁入 `edit/mod_test.rs`
     并改走 `apply_*` / `plan_*`；两条语义迁移——`元素缺席返回err并附现有元素清单` 改挂
     `plan_element_delete` / `apply_element_delete`（upsert 里缺席是「新建分支」，判据句为
     `ElementKindMissing`）、`坏源与无element表均err` 的「无 element 表」臂改挂
     `apply_element_delete`（`ElementTableMissing`）。`edit_test.rs` 本身同批删除（退役，非测试编写）。

     跨模块组合用例一律挂链路发起方 / 最上层调用方：`plan_* → apply_* → 自证 / 回填`、
     `plan_* → drift::file_hash_of`（`expect` 校验与 `ModelWrite.hash`）挂 `edit/mod.rs` 节；
     `命令 → plan_* → apply_* → write_atomic` 挂 `src/commands/archi/mod.rs` 节；
     `hash_files（三形参） ↔ read_bytes / file_hash / file_hash_of` 挂 `drift/mod.rs` 节。
     不新建独立集成章节、不新建组合测试区。

     源码锚纪律（AC-1 / AC-6 / AC-8 的机械面）：本仓既有先例为 `include_str!` 源文本断言
     （`api/mod_test.rs` 的 `api出口面归零且判据可判红`、`dto_test.rs` 的 `lib出口面零文档关联残留`）；
     源码锚只断言「名字 / 调用序 / 计数」，MUST NOT 退化成对已被行为用例覆盖的语义做第二份判据。 -->

### packages/desktop/src-tauri/crates/core/archi/src/edit/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs

<!-- 挂 AC-1（8 函数 + 2 类型的公开面与出口唯一性）、AC-2（gate 退化调用等价 / 幂等零差异）、
     AC-3（六个可清空位 × 三态 + 三个封闭枚举位无清空臂）、AC-4（`expect` 四形态 + 关系面随落点）、
     AC-5（表头回填**五例** + 合成 fixture 零差异）、AC-6（自证链顺序与零文本，关系面另断条数）、
     AC-7（EditError 形态在 apply 侧的表现，含 `RelationDuplicated` 两臂 facts）、AC-9（删除
     fail-closed）、AC-10（关系原语三态、落点 = 既有关系的声明文件 / 新增关系的 `from` 文件、
     重复声明 fail-closed）。
     条目取自 design「公共函数 / API」子表中「所在文件 = edit/mod.rs」的 **12 行**（8 公开函数 + 4
     私有行：`backfill_header_counts` / `finish` / `check_create`+`check_target`+`read_source`+
     `check_expect` / `relation_hits`+`check_relation_unique`）与「类型定义」子表的 `ModelWrite` /
     `ElementSpec` 两行；未做源文件导出扫描。
     装置：零 mock——真实 `toml_edit`（排版与 decor 保留即被测能力）+ 真实 `TomlParser`
     （自证与回读判据与解析层同源）+ 真实 `ModelGraph`（层级 / 邻接判据单点）+ std 自建 `TempDir`。
     私有件（`backfill_header_counts` / `finish` / `check_create` / `check_target` /
     `read_source` / `check_expect` / `relation_hits` / `check_relation_unique`）经子模块路径
     `super::…` 直调（子模块可见父模块私有项，沿 `config/src/lib_test.rs` 先例）——不新增
     test-only 导出（AGENT.md）。 -->

#### 待测功能

- `apply_element_spec()`: 单文件元素 upsert（纯函数，零文件系统触碰）——在案 + `kind` 有值 ⇒ `ElementAlreadyExists`；缺席 + `kind` 缺 ⇒ `ElementKindMissing`；缺席按 `kind`/`gate`/`api`/`entry`/`path`/`doc`/`summary`/`metadata` 规范字段序整表写出；在案逐位三态改写（保序 + 沿用原 decor + 新键追加表尾）；`file` / `expect` 是本函数的**非入参旋钮**（plan 层判，apply 不读）
- `apply_element_delete()`: 单文件删元素表——`element` 表缺席 ⇒ `ElementTableMissing`；元素缺席 ⇒ `ElementNotFound`（facts 列该文件现有元素）；命中即整表 `remove` 并回读断言缺席；**不做**子元素 / 关系前置（跨文件，归 `plan_element_delete`）
- `apply_relation_upsert()`: 单文件 upsert 一条 `[[relation]]`——命中 `(from, to)` 走更新（回读条数不变）；缺席追加条目（键序 `from`/`to`/`desc`/`apis`，空行前缀与既有条目同形）；`relation` 键缺席即新建 `ArrayOfTables`（不报缺表）；`desc` 三态、`apis` 三态（`Some(&[])` 翻成**删键**，MUST NOT 渲染空数组）；**同一源内该对 ≥ 2 处 ⇒ `RelationDuplicated`（文件面 facts「同一源内多处声明（N 处）」）**
- `apply_relation_delete()`: 单文件删一条 `[[relation]]`——命中**唯一**条目即删该条（`retain` 的一次删多条退役）并回读断言 `(from, to)` 已缺席、关系条数 −1；缺席 ⇒ `RelationNotFound`（facts 列该文件现有 `from → to` 对）；**同一源内该对 ≥ 2 处 ⇒ `RelationDuplicated`**
- `plan_element_upsert()`: 定位 + 前置 + 读源 + `expect` + `apply_element_spec` + 填 `hash`，**零写盘**——`kind` 有值即新建意图（在案 ⇒ `ElementAlreadyExists`）；新建臂判据次序固定 `FqnInvalid` → `ElementKindMissing` → `TargetInvalid` → `ParentMissing`；既有元素落其声明文件（给了不同 `file` ⇒ `TargetInvalid`）；新建允许目标文件尚不存在（源按 `""`）
- `plan_element_delete()`: 同上路径；直接子元素非空 ⇒ `DeleteHasChildren`（facts 点名 FQN）；`depends_on` ∪ `depended_by` 非空 ⇒ `DeleteHasRelations`（facts 点名 `from → to`）；两者皆无 ⇒ 删除
- `plan_relation_upsert()`: 同上；两端点缺席 ⇒ `RelationEndpointMissing`（点名缺失端）；**落点：模型面命中 1 处（既有关系）⇒ 该条目的 `file`（可非 `from` 元素文件）；命中 0 处（新增）⇒ `from` 元素声明文件**；命中 ≥ 2 处 ⇒ `RelationDuplicated`（模型面 facts 点名各声明文件与处数）；`expect` 绑落点文件
- `plan_relation_delete()`: 同上；端点缺席 ⇒ `RelationEndpointMissing`（先于关系判据）；**模型面命中 0 处 ⇒ `RelationNotFound`（facts 列该 `from` 元素的出边对，MUST NOT 读 `from` 文件后让 apply 报「该文件没有关系」）**；命中 ≥ 2 处 ⇒ `RelationDuplicated`；命中 1 处 ⇒ 落点 = 该关系条目的 `file`，删除成功、条数 −1
- `ModelWrite`（类型）: `{ file, content, hash }` 三元组，`hash` 为**写后**内容哈希（`file_hash_of(file, content)`，可直接作下一次 `expect`）；`Debug` / `Clone` / `PartialEq` / `Eq`，非 IPC 出线类型
- `ElementSpec`（类型）: 11 字段写入规格（`fqn` + 六个可清空位 + 三个封闭枚举位 + `file` + `expect`），`#[derive(Debug, Clone, Default)]` 使 `..Default::default()` 简写构造成立
- `backfill_header_counts()`（私有）: 首部注释块内**第一处** `（<ASCII 数字> 元素 / <ASCII 数字> 关系）` 的两个数字按重解析结果回填；括号外逐字；无该括号即原样返回（不新增）；首部之外同形文本零改动；**替换域是整份渲染文本（首部注释块之后的 `[element."…"]` / `[[relation]]` 逐字保留，不得截断正文）**
- `finish()`（私有）: 自证链收口——渲染 → 重解析取计数（诊断非空即 `RenderSelfCheck`）→ 表头回填 → 对交付文本重解析自证 → `verify` 回读（不一致即 `ReadBackMismatch`；关系面 `verify` 另断条数）；被自证 / 被回读的文本逐字等于返回值
- `check_create()`（私有）: 新建臂的共用判据（FQN 形态七拒 ⇒ `FqnInvalid`；`kind` 缺 ⇒ `ElementKindMissing`），`apply_element_spec` 与 `plan_element_upsert` 同源一份措辞
- `check_target()`（私有）: 落点判据四臂 ⇒ `TargetInvalid`（缺 `file` / 非模型声明目录一级 `*.toml` / 模型声明目录缺席 / 既有元素给不同 `file`）
- `read_source()`（私有）: 读源单点——`allow_missing = true` 且文件缺席 ⇒ `Ok("")`；读失败 ⇒ `SourceReadFailed`（facts `{path}（{error}）`）
- `check_expect()`（私有）: `None` ⇒ 不校验；`Some(_)` 与 `file_hash_of(落点文件, source)` 比对，不等或文件缺席 ⇒ `ExpectMismatch`（不存在「期望缺席」臂）；关系面「落点文件」按上方两函数各自的落点规则取
- `relation_hits()`（私有）: 关系身份键单点——`from` / `to` **双等**（与 `diff::model_diff` / `relation::relation_detail` 同口径），返回 `model.relations` 中的命中条目（含各自 `file`）；MUST NOT 引第三种身份定义
- `check_relation_unique()`（私有）: `hits.len() >= 2` ⇒ `RelationDuplicated`（模型面 facts 点名各声明文件与处数）、`len() <= 1` ⇒ `Ok`；与 `apply_*` 的文件面判据同一 `EditErrorKind`（不引第三处）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `apply_element_spec()` · gate 退化调用 | 正向 | 含注释 / 空行 / 多元素的源把某元素 `gate` 由 `off` 改 `strict`（`GateTier::ALL` 三值遍历）→ 行数不变、按行 diff 恰 1 行且该行含新线值、其余元素 / 注释逐字、`TomlParser::parse` 读回新档位且零诊断——`改档只动目标行且其余逐字保留` | 矩阵迭代（3 值） |
| `apply_element_spec()` · 幂等零差异 | 正向 | 源含**准确**表头括号，把 `gate` 改成其当前值 ⇒ 产出与源**逐字节相等**（回填 no-op + decor 沿用 ⇒ 零差异，AC-2 / AC-5 的单元面）——`gate改成当前值产出零字节差异` | 单点 |
| `apply_element_spec()` · 三态（可清空位） | 正向 | `entry` / `doc` / `summary` 三位 × {`None` / `Some(None)` / `Some(Some(v))`}：`None` ⇒ 该键字节不变；`Some(None)` ⇒ 键从源中消失（回读 `None`）；`Some(Some(v))` ⇒ 写入新值（回读逐字）——`三态逐位可辨` | 矩阵迭代（3 位 × 3 态） |
| `apply_element_spec()` · 三态（集合位） | 正向 | `path` {`None` / `Some(vec![])` / `Some(vec!["a","b"])`} 与 `metadata` {`None` / `Some(BTreeMap::new())` / `Some({k:v})`}：清空臂删键且**产出文本零 `path = []` / 零 `metadata = {}`**；设值臂写内联数组 / 内联表并逐项回读——`集合位清空即删键不写空集合` | 矩阵迭代（2 位 × 3 态） |
| `apply_element_spec()` · 三态（封闭枚举位） | 正向 | 源无 `gate` 行 → `gate = Some(GateTier::Off)` ⇒ 插入 `gate = "off"` 行（显式化缺省，非清空）；`api = Some(ApiStatus::None)` ⇒ 写 `api = "none"`；两处 `None` 时键保持原样（无删键路径）——`封闭枚举位显式化缺省不是清空` | 矩阵迭代（2 值） |
| `apply_element_spec()` · 新建元素 | 正向 | 缺席 FQN + `kind` + 八个字段齐给 ⇒ 新增 `[element."<FQN>"]` 单层表、字段按规范序（`kind`/`gate`/`api`/`entry`/`path`/`doc`/`summary`/`metadata`）写出、逐字段回读相符——`新建元素按规范字段序整表写出` | 单点 |
| `apply_element_spec()` · 保序与 decor | 正向 | 更新既有键：目标行带行内注释与非标准缩进 ⇒ 注释与缩进逐字保留（沿用原 decor）；新键（原本缺席的 `summary`）追加到元素表尾而非按规范序插队；`entry` 位于 `api` 前 / 后两形态的相对序逐字保留——`写面零重排且沿用原decor` | 矩阵迭代（3 形态） |
| `apply_element_spec()` · 在案 + kind | 异常 | 元素在案且 `spec.kind = Some(_)` ⇒ `EditError { kind: ElementAlreadyExists }`，facts 点名该元素已声明的文件与「改既有元素请省去 kind」出路——`在案元素给kind即拒` | 单点 |
| `apply_element_spec()` · 缺席 + 缺 kind | 异常 | 元素缺席且 `spec.kind = None`（含 `spec = ElementSpec { fqn, ..Default::default() }`）⇒ `ElementKindMissing`，facts 点名 FQN 不在案——`缺席元素缺kind即拒` | 单点 |
| `apply_element_spec()` · 非表形态 | 异常 | `element."App" = "x"`（条目非表形态）⇒ `ElementNotTable`，facts 含 FQN 与「无法写回」——`元素声明非表形态即拒` | 单点 |
| `apply_element_spec()` · 坏源 | 异常 | 源不可解析（`[element."App]\n…` 缺引号）⇒ `SourceParseFailed`，facts 为 parser 原文（零加工）；`Err` 臂零文本产出（返回类型即保证）——`坏源解析失败零文本` | 单点 |
| `apply_element_spec()` · 越界值借自证拒绝 | 异常 | `package` 元素被设 2 条 `path`（parser 判 `invalidValue`）⇒ `RenderSelfCheck`、facts 含「重解析出 N 条诊断」；写面零第二份值域判据（拒绝理由是 parser 诊断而非写面自写规则）——`越界值只由重解析自证拒绝` | 单点 |
| `apply_element_spec()` · FQN 形态（新建） | 异常 | 新建 FQN 七形态（空串 / 含空白 / 含 `"` / 含 `[` / 含 `]` / 含 `#` / 含换行）⇒ 各 `FqnInvalid` 且 facts 含 `{fqn:?}` 与字符清单；对照臂：既有元素的同形态 FQN 仍只由 parser 判（写面不重做形态校验）——`新建fqn七形态皆拒` | 矩阵迭代（7 形态 + 1 对照） |
| `apply_element_spec()` · 非入参旋钮 | 边界 | 同一源同一 `gate`，`spec` 带 `file = Some("99-other.toml")` + `expect = Some("sha256:错值")` 与两者皆 `None` 两次调用 ⇒ 产出**逐字节相等**（`apply_*` 不读落点与锁）——`apply不读file与expect` | 矩阵迭代（2 形态） |
| `apply_element_spec()` · str / 规模 / 排版边界 | 边界 | 空串源 + `kind` ⇒ 新建成功（最小文件：单表零其它元素）；超长 FQN（1200 字符）与 emoji FQN（`App.🚀Auth`）⇒ 正常写回并可读回；排版五形态（元素间空行 / 缩进+行内注释 / 无尾换行 / CRLF / 500 元素规模）逐条保留且仅目标行变化——`特殊字符与排版与规模边界` | 矩阵迭代（5 形态） |
| `apply_element_delete()` · 叶元素删除 | 正向 | 源含三个元素与注释，删中间一个 ⇒ 该元素表整段离场、其余元素 / 注释 / 空行逐字、表头计数随动下降（`（3 元素 / 1 关系）` → `（2 元素 / 1 关系）`）——`删除叶元素其余逐字且计数随动` | 单点 |
| `apply_element_delete()` · 无 element 表 | 异常 | 源为 `""` 或只有 `[[relation]]` ⇒ `ElementTableMissing`（facts「无从定位元素」）——`无element表无从定位` | 矩阵迭代（2 形态） |
| `apply_element_delete()` · 元素缺席 | 异常 | 元素缺席 ⇒ `ElementNotFound`，facts 列该文件现有元素 FQN（`a / b` 形态）——`元素缺席列出该文件现有元素` | 单点 |
| `apply_element_delete()` · 前缀近似不误删 | 边界 | 删 `App.A` 时 `App.AB` / `App.A.B`（同文件内声明）保留；删 `App` 与删 `App.A` 两臂各自的删除面精确——`删除按完整fqn精确匹配` | 矩阵迭代（2 形态） |
| `apply_element_delete()` · 删净与特殊 FQN | 边界 | 删唯一元素 ⇒ 元素数 0 且产出仍可解析（重解析零诊断）；emoji / 超长 FQN 删除成功——`删净唯一元素仍可解析` | 矩阵迭代（3 形态） |
| `apply_relation_upsert()` · 命中即更新 | 正向 | 既有 `(from, to)` 改 `desc` ⇒ 关系条数不变、该条 `description` 为新值、其余条目（含空行与注释）逐字保留——`关系命中走更新条数不变` | 单点 |
| `apply_relation_upsert()` · desc 三态 | 正向 | `desc` {`None` / `Some(None)` / `Some(Some("说明"))`} ⇒ 不动 / 键从源中消失 / 写新值；`Some(Some(""))` 写空串仍可解析——`关系desc三态逐位可辨` | 矩阵迭代（4 形态） |
| `apply_relation_upsert()` · apis 三态 | 正向 | `apis` {`None` / `Some(&[])` / `Some(&["a","b"])`} ⇒ 不动 / 键消失（产出文本零 `apis = []`）/ 写内联数组且原文序保留——`关系apis清空即删键` | 矩阵迭代（3 态） |
| `apply_relation_upsert()` · 缺席追加与缺 relation 键 | 正向 | 缺席 `(from, to)` 追加条目（键序 `from`/`to`/`desc`/`apis`、与既有条目同形的前缀空行）；`01-app.toml` 一类**零关系文件**（无 `relation` 键）⇒ 新建 `ArrayOfTables` 而非报缺表——`关系缺席追加且缺键即建数组` | 矩阵迭代（2 形态） |
| `apply_relation_upsert()` · 越界 apis 值 | 异常 | `apis = Some(&["A","A"])` / `Some(&[""])` / `Some(&["a::b"])` ⇒ 各 `RenderSelfCheck`（值面判据归 parser；写面 MUST NOT 自写值面判据）——`apis越界值只由自证拒绝` | 矩阵迭代（3 值） |
| `apply_relation_upsert()` · 坏源 | 异常 | 源不可解析 ⇒ `SourceParseFailed` 且零文本——`关系写入坏源零文本` | 单点 |
| `apply_relation_upsert()` · 多条目与特殊字符 | 边界 | 三条关系的文件更新中间一条（前 / 后条目逐字保留）；`from` 或 `to` 含 emoji / 超长串 ⇒ 正常写回并回读相等——`多条目更新只动命中条` | 矩阵迭代（2 形态） |
| `apply_relation_upsert()` / `apply_relation_delete()` · 同一源内重复声明 | 异常 | 同一源内两处同 `(from, to)` ⇒ 两函数均 `RelationDuplicated`（文件面 facts「在同一源内多处声明（2 处）——先删除重复条目」）、零文本产出；`apply_relation_delete` 的 `retain` 一次删多条行为已退役（对照臂：唯一命中时只删该条，见下行）——`同源重复声明fail-closed` | 矩阵迭代（2 入口） |
| `apply_relation_delete()` · 命中即删 | 正向 | 三条关系中删中间一条 ⇒ 该条目离场、其余条目与元素逐字保留、关系回读条数为 2——`关系删除只动命中条` | 单点 |
| `apply_relation_delete()` · 关系缺席 | 异常 | 缺席 `(from, to)` ⇒ `RelationNotFound`，facts 列该文件现有 `from → to` 对——`关系缺席列出该文件现有对` | 单点 |
| `apply_relation_delete()` · 唯一性与双等匹配 | 边界 | 只剩一条关系时删净（回读关系数 0 且仍可解析）；同 `from` 不同 `to` / 同 `to` 不同 `from` 两臂各只删命中那条（`(from, to)` 双等是唯一命中口径；同对两处属 `RelationDuplicated`，见上行）——`关系删除双等匹配与删净` | 矩阵迭代（3 形态） |
| `plan_element_upsert()` · 定位与零写盘 | 正向 | 两文件模型（目标元素在 `02-auth.toml`）→ `file` == 该文件绝对路径、`content` 含新值、`hash` 形态 `sha256:`；调用前后两文件字节快照全等（`plan_*` 零写盘）——`定位目标文件并零写盘` | 既有（迁移） |
| `plan_element_upsert()` · 新建落点 | 正向 | 缺席 FQN + `kind` + `file = "07-new.toml"`（目标文件尚不存在）+ `expect = None` ⇒ `file` == 模型声明目录下该文件、`content` 含新表、零写盘（文件仍不存在）——`新建可落新文件且不落盘` | 单点 |
| `plan_element_upsert()` · hash 与 TOCTOU 闭环（组合：plan → apply → drift::file_hash_of） | 正向 | `ModelWrite.hash` == `drift::file_hash_of(&plan.file, &plan.content)` == 把 `content` 落盘后 `drift::file_hash(&plan.file)`（三者逐字相等）；把该值作下一次 `expect` 再调一次 ⇒ `Ok`——`写后哈希可作下一次expect` | 单点 |
| `plan_element_upsert()` · 既有元素换 file | 异常 | 既有元素给不同 `file` ⇒ `TargetInvalid`（facts 点名现声明文件与给定值，「迁文件属独立语义」）——`既有元素不接受改落点` | 单点 |
| `plan_element_upsert()` · 落点四臂 | 异常 | 新建 + `file` 缺 ⇒ `TargetInvalid`（「新建必须显式给出」）；`file` 为 `a/b.toml` / 绝对路径 / `x.json` ⇒ `TargetInvalid`（「不在模型声明目录一级」）；模型声明目录缺席 ⇒ `TargetInvalid`（「不存在（写面不代建目录）」）——`落点四臂各点名成因` | 矩阵迭代（4 臂） |
| `plan_element_upsert()` · 父元素缺席 | 异常 | 新建 `App.A.B` 而 `App.A` 不在案 ⇒ `ParentMissing`（facts 点名父 FQN 与「祖先须显式声明」）；新建单段 `App` ⇒ 跳过父判据并 `Ok`——`新建查直接父且单段跳过` | 矩阵迭代（2 形态） |
| `plan_element_upsert()` · expect 四形态 | 异常 | `expect` {`Some(当前值)` ⇒ `Ok` / `Some(错值)` ⇒ `ExpectMismatch`（facts 含期望与实际）/ 文件缺席 + `Some(_)` ⇒ `ExpectMismatch`（facts 点名文件缺席）/ `None` ⇒ 不校验且照常产出}——`expect四形态逐条成立` | 矩阵迭代（4 形态） |
| `plan_element_upsert()` · 判据次序 | 异常 | 同时「缺席 + 缺 `kind` + 落点非法」⇒ 判据句为 `ElementKindMissing`（不被落点判据抢先）；同时「在案 + 给 `kind` + 给不同 `file`」⇒ `ElementAlreadyExists`（先判在案）——`新建与在案的判据次序固定` | 矩阵迭代（2 形态） |
| `plan_element_upsert()` · 源读取失败 | 异常 | 目标文件为非 UTF-8 字节 ⇒ `SourceReadFailed`（facts `{path}（{error}）`，语序落位形态）——`非utf8源读取失败点名路径` | 单点 |
| `plan_element_upsert()` · 与 apply 同源判据 | 异常 | FQN 形态 / 缺 `kind` / 在案 + `kind` 三形态，`plan_element_upsert` 与 `apply_element_spec` 的 `Err` **逐字段相等**（`kind` 与 `facts` 均相同 ⇒ 同一判据一份措辞）——`两入口共用同一判据措辞` | 矩阵迭代（3 形态） |
| `plan_element_upsert()` · root 边界 | 边界 | 空白 `root` / 不存在的 `root` ⇒ `Err` 不 panic；`models/` 为空目录 + 新建 ⇒ 父判据成立（单段 FQN）且 `Ok`；emoji 既有元素 ⇒ `Ok`——`空白与不存在root均err` | 既有（迁移 + 扩） |
| 四个 `plan_*` · 结构诊断前置 | 异常 | 模型含 `cycle` 诊断时依次调 `plan_element_upsert` / `plan_element_delete` / `plan_relation_upsert` / `plan_relation_delete` ⇒ 四次全 `Err(ModelHasDiagnostics)`、判据句「模型存在结构诊断，拒绝写回」、facts 含诊断种类、零文本——`结构错模型四拒` | 矩阵迭代（4 入口） |
| `plan_element_delete()` · 叶元素 | 正向 | 无从属叶元素 ⇒ `Ok`，content 不含该元素、其余逐字、表头计数下降；`ModelWrite.hash` 与 content 自洽——`删除叶元素成功且计数随动` | 单点 |
| `plan_element_delete()` · 有直接子元素 | 异常 | 元素有已声明直接子元素 ⇒ `DeleteHasChildren`，facts 点名列出子元素 FQN——`有子元素拒绝删除` | 单点 |
| `plan_element_delete()` · 出边引用 | 异常 | 元素仍被出边引用（`depends_on` 非空）⇒ `DeleteHasRelations`，facts 点名 `from → to`——`出边引用拒绝删除` | 单点 |
| `plan_element_delete()` · 入边引用 | 异常 | 元素仍被入边引用（`depended_by` 非空）⇒ `DeleteHasRelations`（出 / 入两臂同源，不重写判定）——`入边引用拒绝删除` | 单点 |
| `plan_element_delete()` · 元素缺席 | 异常 | 元素缺席 ⇒ `ElementNotFound`，facts 为「`{fqn}（模型共 N 个元素）`」（模型面形态）——`模型面缺席报元素总数` | 既有（迁移） |
| `plan_element_delete()` · expect 不匹配 | 异常 | `expect = Some(错值)` ⇒ `ExpectMismatch` 且零文本（删除面与 upsert 面共用同一校验点）——`删除面同样受乐观锁保护` | 单点 |
| `plan_element_delete()` · 判据次序 | 边界 | 模型含 `orphanHierarchy`（`App.A.B` 在案而 `App.A` 缺）⇒ `ModelHasDiagnostics` 先于一切删除判据；子元素与关系同时成立的元素 ⇒ `DeleteHasChildren` 先（次序固定）——`删除判据次序固定` | 矩阵迭代（2 形态） |
| `plan_element_delete()` · 删空模型 | 边界 | 模型唯一元素被删 ⇒ `Ok`、回读元素数 0；删前后其余文件字节零变化——`删除唯一元素后模型可解析` | 单点 |
| `plan_relation_upsert()` · 新关系落 from 文件 | 正向 | 跨文件模型（`from` 在 `02` 文件、`to` 在 `01` 文件、该 `(from, to)` 模型面 **0 处**）新增关系 ⇒ `ModelWrite.file` == `from` 元素声明文件绝对路径；落盘后 `dto` 面回读该关系的 `modelFile` 命中该文件；回读关系条数 == 改前 + 1——`新关系落from元素所在文件` | 单点 |
| `plan_relation_upsert()` · 既有关系走更新 | 正向 | 既有 `(from, to)` 改 `desc` ⇒ `Ok`、回读关系条数**不变**、`description` 为新值——`既有关系对走更新` | 单点 |
| `plan_relation_upsert()` · 既有关系声明于非 `from` 文件（组合：`relation_hits` 的 `file` → `apply_relation_upsert`） | 正向 | `[element."A"]` 声明在 `01-app.toml`、`(A, B)` 声明在 `06-frontend.toml`（`from` 元素文件 ≠ 关系文件，本仓 `Desktop → Desktop.Components.Ui` 同形）⇒ `ModelWrite.file` == `06-frontend.toml` 绝对路径、`content` 只改该条 `desc`、`01-app.toml` **零变化**（调用前后字节快照全等）、回读关系条数不变——`既有关系声明于非from文件时原地更新` | 单点 |
| `plan_relation_upsert()` / `plan_relation_delete()` · 模型面重复声明 | 异常 | 同一 `(from, to)` 在 `01` / `06` 两文件各声明一次 ⇒ 两函数均 `RelationDuplicated`（模型面 facts 点名两文件与「N 处」）、**零文本产出**；对照臂：同一 `(from, to)` 单处声明 ⇒ 不报该错（走各自的落点规则）——`模型面重复声明fail-closed` | 矩阵迭代（2 入口 + 1 对照） |
| `plan_relation_upsert()` · 端点缺席 | 异常 | `from` 缺 / `to` 缺 / 双缺三形态 ⇒ `RelationEndpointMissing`，facts 点名缺失端 FQN——`端点缺席点名缺失端` | 矩阵迭代（3 形态） |
| `plan_relation_upsert()` · expect 与结构诊断 | 异常 | `expect = Some(错值)` ⇒ `ExpectMismatch`；`cycle` 模型 ⇒ `ModelHasDiagnostics`；两臂零文本产出——`关系upsert两前置均err` | 矩阵迭代（2 形态） |
| `plan_relation_upsert()` · 零关系落点与清空臂 | 边界 | `from` 元素所在文件无 `relation` 键 ⇒ 新建数组成功；`apis = Some(&[])` ⇒ 落盘文本中该键消失（回读 `apis` 为空且源文本零 `apis = []`）——`零关系文件新建数组与apis清空` | 矩阵迭代（2 形态） |
| `plan_relation_delete()` · 在场删除 | 正向 | 在场关系（模型面命中 1 处）删除成功 ⇒ 回读关系数 −1、该文件表头计数随动、其余条目逐字——`关系删除成功计数随动` | 单点 |
| `plan_relation_delete()` · 缺席两形态 | 异常 | 模型面该 `(from, to)` **0 处** ⇒ **模型面** `RelationNotFound`（facts 列该 `from` 元素的出边对，MUST NOT 读 `from` 文件后由 apply 报「该文件没有关系」）；端点元素缺席 ⇒ `RelationEndpointMissing`（端点判据先于关系判据）——`关系删除的双前置` | 矩阵迭代（2 形态） |
| `plan_relation_delete()` · 既有关系声明于非 `from` 文件 | 边界 | `from` 元素声明在 `01-app.toml`、`(from, to)` 声明在 `06-frontend.toml` ⇒ `ModelWrite.file` == `06-frontend.toml`、该条目删净且 `01-app.toml` 零变化、表头计数随动（关系数 −1）、其余条目逐字——`既有关系声明于非from文件时删除落声明文件` | 矩阵迭代（2 形态） |
| 关系回读条数（组合：既有 / 新增 / 删除三臂） | 边界 | 既有 `(from, to)` upsert ⇒ 重解析关系条数 == 改前（「更新而非追加副本」的结构性判据）；新增关系 ⇒ 改前 + 1；删除 ⇒ 改前 − 1；条数不符即 `ReadBackMismatch`（facts「关系条数为 X，目标为 Y」）——`关系回读条数三臂各成立` | 矩阵迭代（3 臂） |
| `ModelWrite`（类型） | 正向 | 三元组逐字段可读、`PartialEq` / `Eq` 成立：同 `file` / `content` / `hash` 两值相等，`content` 差一字节即不等——`ModelWrite三元组可比` | 单点 |
| `ElementSpec`（类型） | 边界 | `ElementSpec::default()` 全字段缺省（`fqn` 空串 + 九个 `None`）⇒ 传入 `apply_element_spec` 得 `Err`（`FqnInvalid` 或 `ElementKindMissing`，按判据次序）不 panic；`..Default::default()` 简写可构造壳层同形 spec（`fqn` + `gate` 两项）——`ElementSpec缺省可构造且不panic` | 单点 |
| `backfill_header_counts()` · 回填正确性 | 正向 | 表头写 `（9 元素 / 10 关系）` 而文件实为 8 元素 10 关系的源上删一个元素 ⇒ 表头变 `（8 元素 / 10 关系）`、括号外整行逐字（含全角 `）` 与人工标题）——`表头两数按重解析回填` | 单点 |
| `backfill_header_counts()` · 正文不截断 | 边界 | 完整模型文本（首部注释块后紧接 `[element."…"]` 与 `[[relation]]`）在表头被回填后 ⇒ **首部注释块之后的正文逐字保留**、重解析出的元素 / 关系集合与改前相同；只核括号内计数会漏掉「尾部模型表被截断仍零诊断」这一形态（回归臂）——`回填只替换两段数字且正文逐字保留` | 单点 |
| `backfill_header_counts()` · 零差异 | 正向 | 计数准确源（六个模型文件同形：`# 02 · core 分层（9 元素 / 10 关系）`）⇒ 原样返回、与输入逐字节相等——`计数准确即零差异` | 单点 |
| `backfill_header_counts()` · 不新增与不越界 | 异常 | 无该括号 ⇒ 原样返回（MUST NOT 新增）；括号只出现在**首部注释块之外**（首个非注释行之后）⇒ 零改动；首部块内**第二处**同形文本零改动——`无括号不新增且块外零改动` | 矩阵迭代（3 形态） |
| `backfill_header_counts()` · 数字与首部形态 | 边界 | 位数变化（`9`→`10`、`100`→`99`）；CRLF 首部；首部由空行 + `#` 行混合构成；无尾换行——四种形态各只替换两段数字——`表头数字位数与换行边界` | 矩阵迭代（4 形态） |
| `finish()` · 步骤序源码锚 | 正向 | `include_str!("mod.rs")` 内 `TomlParser::parse` 出现两次且 `backfill_header_counts` 的调用夹在两次之间（先渲染 → 取计数 → 回填 → 对交付文本自证 → `verify`）；`backfill_header_counts` / `finish` / `parse_source` 各恰一处且三者非 `pub`——`自证链步骤序固定` | 单点 |
| `finish()` · 零文本产出 | 异常 | 重解析出诊断（如给 `package` 设 2 条 `path`）⇒ `RenderSelfCheck` 且 facts 含「重解析出 N 条诊断，未产出文本」；`verify` 注入返回 `Err(ReadBackMismatch)` ⇒ 该错误原样传出（回读不一致即失败，无降级路径）——`自证与回读失败均零文本` | 矩阵迭代（2 形态） |
| `finish()` · 回填后计数与回读一致 | 边界 | 陈旧表头 + 元素删除 ⇒ 交付文本的表头两数 == `TomlParser::parse(&content)` 得 `elements` / `relations` 条数（钉住「回填在自证之前、被自证的是交付文本」）——`回填后计数等于回读计数` | 单点 |
| `check_create()` · 七拒 + 缺 kind | 异常 | 空串 / 含空白 / 含 `"` / 含 `[` / 含 `]` / 含 `#` / 含换行 ⇒ 各 `FqnInvalid`；合法形态 + `kind = None` ⇒ `ElementKindMissing`；八形态 kinds 与 facts 逐个断言——`新建判据七拒与缺kind` | 矩阵迭代（8 形态） |
| `check_target()` · 四臂 facts | 异常 | 缺 `file` / 非一级（`a/b.toml`、`/abs.toml`、`x.json`）/ 目录缺席 / 既有元素给不同 `file` ⇒ 四臂 `TargetInvalid` 的 facts 逐条对齐 design 数据模型表——`落点四臂文案逐条对齐` | 矩阵迭代（4 臂） |
| `read_source()` · 缺席与读失败 | 边界 | `allow_missing = true` + 文件缺席 ⇒ `Ok(String::new())`（新建臂）；`allow_missing = false` + 缺席 ⇒ `SourceReadFailed`；非 UTF-8 ⇒ `SourceReadFailed`——`读源单点两态` | 矩阵迭代（3 形态） |
| `check_expect()` · 四形态 | 边界 | `None` ⇒ `Ok`；`Some(当前值)` ⇒ `Ok`；`Some(错值)` ⇒ `ExpectMismatch`（facts「期望 …，实际 …——重读后再试」）；文件缺席 + `Some(_)` ⇒ `ExpectMismatch`（facts 点名文件缺席）——`expect校验四形态` | 矩阵迭代（4 形态） |
| `relation_hits()`（私有） · 身份键双等 | 边界 | 同 `from` 不同 `to` / 同 `to` 不同 `from` / 同对两处 / 零命中四形态 ⇒ 分别命中 1 / 1 / 2 / 0 条，且各命中条目携带其声明的 `file`（身份键 = `from` 与 `to` **双等**，与 `diff::model_diff` / `relation::relation_detail` 同口径）——`关系身份键双等且带落点` | 矩阵迭代（4 形态） |
| `check_relation_unique()`（私有） · 两臂 | 异常 | `hits.len() == 2`（跨两文件）⇒ `RelationDuplicated` 且 facts 点名两文件与「2 处」；`len() == 0` / `len() == 1` 两形态 ⇒ `Ok`（不报）；错误 `kind` 与 `apply_*` 的文件面判据同一（不引第三处）——`模型面唯一性两臂` | 矩阵迭代（3 形态） |
| 出口面（`edit/mod.rs`） | 异常 | `include_str!("mod.rs")` 源码锚：行首 `pub fn ` 恰 8 处、`pub struct` / `pub enum` 合计 2 处、行首 `pub use ` 恰 1 处且为 `pub use messages::{EditError, EditErrorKind};`、`mod messages;` 不带 `pub`（无 `edit::messages::…` 第五路径）——`公开面恰八函数两类型且messages私有` | 单点 |
| 去写盘 / 去哈希拼装（`edit/` 全目录） | 异常 | `include_str!("mod.rs")` 与 `include_str!("messages.rs")` 零 `fs::write` / 零 `write_atomic` / 零 `spawn` / 零 `Sha256` / 零 `sha2`（写盘单点在壳层，哈希口径在 `drift`）——`edit零写盘零哈希拼装` | 单点 |

> 迁移对照（design 末节交接清单第 1 行逐条落地）：`改档只动目标行且其余逐字保留` / `无显式gate行插入并可逆`（并入「封闭枚举位显式化缺省不是清空」）/ `三档逐值往返读回一致`（并入「gate 退化调用」矩阵）/ `元素缺席返回err并附现有元素清单`（改挂 `apply_element_delete` 的「元素缺席列出该文件现有元素」与 `plan_element_delete` 的「模型面缺席报元素总数」）/ `注入式fqn一律err零文本产出`（并入「新建 fqn 七形态皆拒」+「坏源解析失败零文本」）/ `坏源与无element表均err`（拆为「无 element 表无从定位」与「坏源解析失败零文本」）/ `排版保留面五形态` 与 `规模面五百元素仅目标行变化`（并入「特殊字符与排版与规模边界」）/ `特殊字符fqn改档与err两臂`（并入同条与删 / 关系面的特殊字符臂）/ `定位目标文件并零写盘` / `plan异常三形态均err`（拆为落点四臂、源读取失败、空白与不存在 root 三组）/ `plan边界_空白root与不存在root与emoji`（并入「空白与不存在root均err」）/ `改档保留entry行逐字不变` 与 `改档保留entry位置且不臆造字段`（并入「写面零重排且沿用原 decor」）——**14 条原有语义无一条丢失**，`拒绝改档` needle 在迁移用例里改 `拒绝写回`。

> **回溯修订（第二轮）新增的六组**（design 末节交接清单第 1 行 ③ / ⑤）：`既有关系声明于非from文件时原地更新` / `既有关系声明于非from文件时删除落声明文件`（D18 落点翻转——原 `关系删除落from声明文件` 的语义已改名并改断言，不再假定「关系文件 = `from` 文件」）、`模型面重复声明fail-closed` / `同源重复声明fail-closed`（`RelationDuplicated` 的两臂 facts）、`关系回读条数三臂各成立`（D13 的条数断言：既有 ⇒ 不变 / 新增 ⇒ +1 / 删除 ⇒ −1）、`回填只替换两段数字且正文逐字保留`（tasks 2.3 的重做修正回归臂：回填拼接域是整份渲染文本）、`新关系落from元素所在文件` 补齐「模型面 0 处」前提（与既有关系的落点判据分家）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `toml_edit` 解析 / 渲染（排版、decor、键序保留即被测能力）+ 真实 `TomlParser`（自证与回读判据与解析层同源）+ 真实 `ModelGraph`（层级 / 邻接判据单点，MUST NOT 自写前缀扫描）+ 真实 `drift::file_hash_of`（同 crate 内部模块，显式入参 / 注入依赖之外不得 mock） | 本节全部用例 |
| 文件系统（进程边界） | 不 mock——std 自建 `TempDir`（RAII 清理 + `AtomicU64` 唯一命名）+ `foundation::layout::archi_graph_model_dir` 落模型文件；零写盘用例以「目录字节快照前后相等」表达 | `plan_*` / `read_source` / `check_target` / `check_expect` / 零写盘用例 |
| 时钟 / 随机 / 网络 / 子进程 | 不涉及（`edit/` 零 spawn、零网络、零时间依赖）；不引入任何时钟替身 | — |

### packages/desktop/src-tauri/crates/core/archi/src/edit/messages.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/messages_test.rs

<!-- 挂 AC-7（`ALL` ↔ `criterion()` 穷尽 match 逐值相等 / 逐值非空且互异 / 零「改档」/`message()`
     形态恒为「判据句：事实」/ 基线 10 条文案逐条归宿），并背书 AC-1 的「4 类型」之一对。
     条目取自 design「公共函数 / API」子表「所在文件 = edit/messages.rs」的 3 行与「类型定义」
     子表的 `EditError` / `EditErrorKind` 两行；未做源文件导出扫描。
     构造单点：用例经 `super::messages::error(kind, facts)`（tasks 2.1 的 `pub(super)` 构造，全 crate
     唯一 `EditError` 拼装点）生产错误值——`messages` 是 `edit` 的私有子模块，`edit::mod_test` 作为
     `edit` 的后代可见 `pub(super)` 项，零 test-only 导出。 -->

#### 待测功能

- `EditError::message()`: 唯一渲染出口，恒为 `format!("{}：{}", self.kind.criterion(), self.facts)`（壳层经它映射进既有 `Result<T, String>`；`EditError` 不实现 `Display` / `Error`，无第二条渲染路径）
- `EditErrorKind::criterion()`: **19 分支**的**穷尽** `match`，判据句逐字取 design 数据模型表第二列（`RelationDuplicated` ⇒ `关系多处声明`）；操作中立（零「改档」）、逐值非空且互异、零动态占位
- `EditErrorKind::ALL`: `pub const ALL: [EditErrorKind; 19]`，次序 = design 数据模型表 1–19，与 `criterion()` 的 `match` 分支集合逐值相等（新增变体未同批补 `ALL` 即数字面不符 / 编译期即报）
- `EditError`（类型）: `{ pub kind, pub facts }`，`Debug` / `Clone` / `PartialEq` / `Eq`；不出线（无 `specta::Type`）
- `EditErrorKind`（类型）: **19 值**封闭枚举（前置组 17 + 自证组 2，`RelationDuplicated` 为回溯修订新增的第 17 值），`Debug` / `Clone` / `Copy` / `PartialEq` / `Eq`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `EditErrorKind::ALL` · 集合对拍 | 正向 | `ALL.len() == 19`；测试文件内写一份覆盖 **19** 变体的**穷尽 `match`**（新增变体不补该 `match` 即编译失败），逐值比对 `ALL` 与 match 分支集合相等；`ALL` 无重复值（`criterion()` 互异即等价证明）——`ALL与criterion分支集合逐值相等` | 穷举迭代（19 值） |
| `EditErrorKind::criterion()` · 判据句表 | 正向 | 逐值与 design 数据模型表第二列**逐字**相等（**19** 条 needle 表驱动，含 `模型存在结构诊断，拒绝写回` / `元素不存在` / `元素声明不是表形态` / `父元素不在案` / `关系端点元素不存在` / `落点非法` / `新建元素缺 kind` / `元素已在案` / `元素 FQN 形态非法` / `模型文件读取失败` / `模型源解析失败` / `模型源缺少 element 表` / `目标文件已变` / `元素仍有已声明子元素，拒绝删除` / `元素仍被关系引用，拒绝删除` / `关系不存在` / **`关系多处声明`** / `写回渲染自证失败` / `回读不一致`）；基线 #1–#4（`模型源解析失败` / `模型源缺少 element 表` / `元素不存在` 两形态）逐字不变，#5/#6 语序落位（`模型文件读取失败：{path}（{error}）` / `元素声明不是表形态：{fqn}（无法写回）`）由 facts 断言补齐，#6–#10 改词面（`无法写回` / `写回渲染自证失败` / `回读不一致` / `拒绝写回`）；`RelationDuplicated` 无基线来源（D18 新增判据）——`criterion逐值逐字对齐文案表` | 穷举迭代（19 值） |
| `EditErrorKind::criterion()` · 非空与互异 | 正向 | 逐值非空（`!criterion().is_empty()`）；19 条判据句 `HashSet` 去重后仍 19（互异）；逐值首尾无空白——`判据句逐值非空且互异` | 穷举迭代（19 值） |
| `EditErrorKind::criterion()` · 操作中立 | 异常 | 逐值零「改档」字面（R-7 的知情改词面）；逐值零动态占位（不含 `{` / `}`，`criterion()` 恒为 `&'static str`，事实只能进 `facts`）——`判据句零改档且零占位` | 穷举迭代（19 值） |
| `EditError::message()` · 形态 | 正向 | 遍历 `ALL`，以固定事实串构造 `EditError` ⇒ `message()` == `format!("{}：{}", criterion, facts)`、以 `criterion()` 起始、恰含一个全角 `：` 分隔且分隔后即 facts 原文——`message恒为判据句加事实` | 穷举迭代（19 值） |
| `EditError::message()` · facts 边界 | 边界 | `facts` 四形态：空串（`message()` 以「：」收尾但仍以判据句起始）/ 含换行 / 含 emoji / >1000 字符 ⇒ 逐字保留不归一、不截断、不二次转义——`facts边界逐字保留` | 矩阵迭代（4 形态） |
| `EditError`（类型） · 结构 | 边界 | 同 `kind` + 同 `facts` 两值相等；`facts` 差一字符即不等；`kind` 差一值即不等（`PartialEq` 覆盖两字段）；`Debug` 输出含两字段名（排障可读）——`EditError逐字段可比` | 单点 |
| `EditError`（类型） · 单一渲染出口 | 异常 | `include_str!("messages.rs")` 源码锚：零 `impl … Display for` / 零 `impl … Error for`（错误渲染只有 `message()` 这一个出口；壳层 MUST NOT 需要第二个）——`零Display与Error实现` | 单点 |
| 私有构造 `error()` · 入参形态 | 边界 | `error(kind, "字面量")` 与 `error(kind, String::from("字面量"))` 两形态 ⇒ 产出的 `EditError` 相等（`facts: impl Into<String>` 两臂同值）；不 panic——`构造单点两入参形态同值` | 矩阵迭代（2 形态） |

> 用例与「AC-7 归宿表」逐条对应：#1–#4 四条由「criterion 逐值逐字对齐文案表」的 needle 表钉住（逐字不变臂），#5 的语序落位由 `SourceReadFailed` 的 facts 形态用例（`check_expect` / `plan_*` 节的「非 utf8 源读取失败点名路径」）钉住，#6 的语序落位 + 改词由 `ElementNotTable` 的 facts（`{fqn}（无法写回）`）钉住，#7–#10 的改词面由判据句逐值断言与 `plan_*` / `apply_*` 的 kinds 断言（`RenderSelfCheck` / `ReadBackMismatch` / `ModelHasDiagnostics`）共同钉住。**#17 `RelationDuplicated` 无基线文案来源**（D18 的回溯修订新增判据，不进基线 10 条）：判据句 `关系多处声明` 由本节 needle 表钉住，其两条 facts 臂（模型面点名各声明文件与处数 / 文件面「在同一源内多处声明（N 处）」）由 `edit/mod_test.rs` 的 `模型面重复声明fail-closed` 与 `同源重复声明fail-closed` 两例钉住——同一事实不在本节写第二份。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 纯数据面（19 值枚举 + 两字段结构体），零 IO 零进程边界；判据句表与枚举集合由 `ALL` 常量驱动，MUST NOT 在测试里手抄第二份枚举清单（手抄 19 个变体名的 `match` 是**编译期守卫**，不是第二事实源——它不重复文案） | 本节全部用例 |
| 源码锚 | `include_str!("messages.rs")` 取编译期内嵌字节（禁测试内复制副本） | 「零 Display / 零 Error 实现」 |

### packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs

<!-- 挂 AC-8（形态 / 同盘稳定 / 内容敏感 / 文件名参与累加 / 与目录粒度不可比 / 写后等价锚 /
     零独立拼装），并给 AC-4 提供「`expect` 闭环前提」的哈希口径锚。
     条目取自 design「公共函数 / API」子表「所在文件 = drift/mod.rs」的 4 行
     （`file_hash` / `file_hash_of` 新增，`hash_files`（三形参）/ `read_bytes` 私有；**`hash_with`
     按设计 D9 修订撤回、不取**）。
     恒等约束（R 面硬要求）：`model_hash` / `assets_hash` / `check` / `CheckInput` / `CheckError`
     的实现与既有 12 条哈希用例（实测 `drift/mod_test.rs` 中以 `model_hash` / `assets_hash` 命名的
     `#[test]`，逐条见下方用例表末行）**逐字不动**——重构只许把累加循环移回 `hash_files`、不许改字节。
     计数口径：design 交接清单第 3 行写「五条」、本节旧稿写「6 例」，两者均为概数；落发以实测
     12 条为准（`model_hash*` 2 条 + `assets_hash*` 10 条），并用末行「既有面零变化」随动钉住。
     D9 修订的判据改写：唯一累加核心 = **三形参 `hash_files(files, base, read)`**（不是另立的
     `hash_with`），「全仓只有一处 `Sha256::new()`」与「`hash_files` 保持私有」两条一字不动。 -->

#### 待测功能

- `file_hash()`: 读盘 → 累加，输出 `sha256:` + 64 位小写 hex；同盘同字节稳定、内容变即变；**文件名参与累加** ⇒ 与目录粒度值（`modelHash` / `assets_hash`）数值不可比，唯一可比对象是同一路径的前后取值；读失败按空字节参与（宁可哈希变化，不因 IO 抖动失败）；实现 = `hash_files(&[path], parent_of(path), &read_bytes)`
- `file_hash_of()`: 字节由调用方给出 → 同一累加核心；写后等价锚——与「把 `content` 落盘后 `file_hash(path)`」逐字相等（写面据此把该值作为 `ModelWrite.hash` 与下一次 `expect`）
- `hash_files()`（私有，改）: **唯一累加核心与唯一 `Sha256::new()` 拼装点**——签名 `fn hash_files(files: &[PathBuf], base: &Path, read: &dyn Fn(&Path) -> Vec<u8>) -> String`，按给定序逐文件累加「相对 `base` 的名字 + `0` + 字节 + `0`」；可见性（私有）与累加口径逐字不变；四个消费者都**字面经它**（`model_hash` / `assets_hash` / `file_hash` 传 `&read_bytes`，`file_hash_of` 传返回 `content` 字节的 reader）
- `read_bytes()`（私有，新增）: `fn read_bytes(path: &Path) -> Vec<u8>` = `std::fs::read(path).unwrap_or_default()`——「读失败按空字节」的字节来源单点；`model_hash` / `assets_hash` / `file_hash` 三处读盘面共用它，MUST NOT 各写一份 `unwrap_or_default`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `file_hash()` · 形态与同盘稳定 | 正向 | 对同一模型文件连续两次调用 ⇒ 逐字相等；形态匹配 `^sha256:[0-9a-f]{64}$`（前缀 + 64 位小写 hex，无大写、无前缀缺失）——`file_hash形态与同盘稳定` | 单点 |
| `file_hash()` · 内容敏感 | 正向 | 改动该文件一个字节后再取 ⇒ 与上次不等（「同一路径前后取值可比」成立）——`文件变即哈希变` | 单点 |
| `file_hash_of()` · 写后等价锚（组合：`file_hash_of` ↔ `hash_files` 读盘） | 正向 | 取任一内容串，`file_hash_of(&file, content)` 与「把 `content` 落盘后 `file_hash(&file)`」**逐字相等**；同一内容两次调用亦相等——`file_hash_of等于落盘后file_hash` | 单点 |
| `file_hash()` · 文件名参与累加 | 异常 | 同名同内容跨目录（`a/01-x.toml` 与 `b/01-x.toml`）⇒ 相等；异名同内容（`a/01-x.toml` 与 `a/02-x.toml`）⇒ 不等（文件名分量进摘要）——`文件名参与累加` | 矩阵迭代（2 形态） |
| `file_hash()` · 与目录粒度不可比 | 异常 | 同一文件的 `file_hash` 与 `super::assets_hash(root)`、`super::model_hash(root)`（私有件经子模块路径直调）三者只共享 `sha256:` 前缀，数值**不判等**（反向断言：MUST NOT 写等值断言，R-9 的误用防线上移到代码面）——`不可与目录粒度哈希互比` | 单点 |
| `file_hash()` · 读失败容错 | 边界 | 路径不存在 / 路径是目录两形态 ⇒ 仍返回 `sha256:` 形态且两次调用相等（空字节参与）；且此时 `file_hash(缺席) == file_hash_of(缺席, "")`（同分歧路的两函数同值）——`读失败按空字节参与` | 矩阵迭代（2 形态） |
| `file_hash_of()` · content 边界 | 边界 | `content` 四形态：空串 / 含 CRLF / 含 emoji / >1000 字符 ⇒ 各产出稳定值且互异（同路径）；零 panic——`content边界稳定且互异` | 矩阵迭代（4 形态） |
| `hash_files()`（私有） · 委托等价 | 边界 | `super::hash_files(&[file.clone()], parent, &super::read_bytes)` == `file_hash(&file)`（口径单锚：读盘面必得同一值）；`hash_files` 仍**非 `pub`**（`include_str!("mod.rs")` 中 `pub fn hash_files` 零命中）——`hash_files是file_hash的口径单点` | 单点 |
| `hash_files()`（私有） · 唯一拼装点 | 异常 | `include_str!("mod.rs")` 的 `Sha256::new()` 计数恰 1（唯一累加核心 = 三形参 `hash_files`，D9 修订后不再另立 `hash_with`）；`hash_with` 在 `mod.rs` 内**零命中**（撤回的机械锚）；`include_str!("../edit/mod.rs")` 与 `include_str!("../edit/messages.rs")` 内 `Sha256` / `sha2` 零命中——`唯一sha256拼装点` | 单点 |
| `hash_files()`（私有） · 注入字节（`read` 形参） | 边界 | 以自定义 `read` 闭包（返回固定字节）调 `super::hash_files(&[path], parent, &\|_\| bytes.to_vec())` ⇒ 输出与 `file_hash_of(&path, 同一 content)` 同值；空 `files` ⇒ 空累加摘要且形态仍 `sha256:`——`注入reader与空清单` | 矩阵迭代（2 形态） |
| `read_bytes()`（私有） · 读失败按空字节 | 边界 | 正常文件 ⇒ 与 `std::fs::read` 等值；文件缺席 / 路径是目录两形态 ⇒ `Vec::new()`（不 panic、不报错）；`super::hash_files(&[path], parent, &super::read_bytes)` 与 `file_hash(&path)` 同值（字节来源单点）——`read_bytes读失败按空字节` | 矩阵迭代（3 形态） |
| `model_hash` / `assets_hash` · 既有面零变化 | 边界 | 既有 **12 条**哈希用例（实测 `drift/mod_test.rs`：`model_hash随模型变化` / `model_hash口径回归手工复算` / `assets_hash确定性且形态固定` / `assets_hash四类资产各自敏感` / `assets_hash空盘稳定且跨目录同值` / `assets_hash非目标扩展名不参与` / `assets_hash字节参与累加` / `assets_hash文件名参与累加` / `assets_hash乱序创建同值` / `assets_hash索引文件参与` / `assets_hash子目录跳过且稳定` / `assets_hash与model_hash分工`；design 交接清单第 3 行的「五条」与本节旧稿的「6 例」均为概数，落发以实测 12 条为准）**逐字不动**：函数签名、可见性、输出字节与覆盖面全不变——D9 修订只改它们的**调用点**（`hash_files(…, &read_bytes)` 三形参），输出逐字不变；这 12 条不直接调 `hash_files`，故用例体零随动——`既有哈希用例逐字维持` | 既有（维持） |

> 「零独立拼装」的另一半——「`edit/` 内零 `Sha256` / `sha2` 字面」——在上述第 9 行由 `include_str!` 跨文件锚钉住，与 `edit/mod.rs` 节的「edit 零写盘零哈希拼装」同源；两处断言同一事实，故只保留一条判据句（不写第二份）。D9 修订另加的机械锚「`hash_with` 在 `mod.rs` 零命中」与「`hash_files` 三形参」同一行，钉住「撤回的抽取不得复活」——若实现另立第二个累加函数，`Sha256::new()` 计数与 `hash_with` 命中两条中至少一条必红。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `TempDir` 落文件、真实 `std::fs` 读盘（读失败按空字节是既有口径，替身会抹掉该语义）；`sha2` 为被测依赖，零替身 | 本节全部用例 |
| 注入 `read` 闭包（`hash_files` 的第三形参） | 不 mock——直接传真实闭包（`&\|_\| bytes.to_vec()`）；它是被测 API 的显式入参 / 注入依赖，不是进程边界替身 | `注入reader与空清单` 用例 |

### packages/desktop/src-tauri/crates/core/archi/src/lib.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs

<!-- design.md 未声明该文件的公共函数 / API 变更：本文件是纯出口整理（删 `#[cfg(test)] mod edit_test;`、
     补 12 个 `pub use`、crate 头注释把「`gate` 写回纯函数」改为「写回原语族」），零函数 / 类型定义。
     故「待测功能」按 design「修改文件」表第 2 行的 ①–⑤ 五条改动面登记**文件级出口面**两项
     （`lib.rs 出口面` / `lib.rs 头注释`），MUST NOT 以 grep 导出声明虚构函数条目；下方 5 条用例与
     这两项逐条对应（12 名 / drift 五名 / 退役声明与 `pub mod` 登记 / 头注释 / 编译期可达）。

     测试文件归属理由：本 crate 无 `lib_test.rs` 惯例（`dto_test.rs::lib出口面零文档关联残留` 的
     doc 明写「`archi` crate 无 `lib_test.rs` 惯例，故这条自证落在 `dto_test`」）；`lib.rs` 的源码锚
     既有先例是模块测试内 `include_str!("../lib.rs")`（`api/mod_test.rs` 两处）。`edit/mod_test.rs`
     已在测试文件清单内且是 12 个新出口的消费者，故出口面锚落该文件，不为一条出口断言新开文件。 -->

#### 待测功能

- `lib.rs 出口面`（文件级被测面，无函数定义）: `pub use edit::{…}` 分组块含 12 个名字（8 函数 `apply_element_spec` / `apply_element_delete` / `apply_relation_upsert` / `apply_relation_delete` / `plan_element_upsert` / `plan_element_delete` / `plan_relation_upsert` / `plan_relation_delete` + 4 类型 `EditError` / `EditErrorKind` / `ElementSpec` / `ModelWrite`）；`pub use drift::{…}` 的既有三名 `check` / `CheckError` / `CheckInput` 未丢且 `file_hash` / `file_hash_of` 两名在场；`pub mod edit;` 保留、`#[cfg(test)] mod edit_test;` 离场（design「修改文件」表 ②③④⑤）；12 名经 crate 根 `use` 可达（任一名字离场即编译失败）
- `lib.rs 头注释`（文件级被测面）: crate 头注释把「`gate` 写回纯函数」改为「写回原语族（元素与关系的 upsert / delete）」（design「修改文件」表 ①）；只断言新表述在场与旧表述零命中，不裁决措辞

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| crate 根出口面 · `edit` 组 | 正向 | `include_str!("../lib.rs")` 含 `pub use edit::{` 分组块，且 12 个名字逐个在场（8 函数 `apply_element_spec` / `apply_element_delete` / `apply_relation_upsert` / `apply_relation_delete` / `plan_element_upsert` / `plan_element_delete` / `plan_relation_upsert` / `plan_relation_delete` + 4 类型 `EditError` / `EditErrorKind` / `ElementSpec` / `ModelWrite`）——`crate根出口含十二个edit名字` | 穷举迭代（12 名） |
| crate 根出口面 · `drift` 组 | 正向 | 含 `pub use drift::{` 且 `check` / `CheckError` / `CheckInput` 三个既有名 **未丢** + `file_hash` / `file_hash_of` 两个新名在场——`crate根出口drift五名齐` | 矩阵迭代（5 名） |
| crate 根出口面 · 退役与登记 | 异常 | `#[cfg(test)] mod edit_test;` 零命中（`edit_test.rs` 退役后不得留幽灵声明）；`pub mod edit;` 仍在（模块未退役）——`edit_test声明已离场` | 矩阵迭代（2 名） |
| crate 根头注释 | 边界 | crate 头注释含「写回原语族（元素与关系的 upsert / delete）」表述；「`gate` 写回纯函数」旧表述零命中（AC-1「注释里的改档表述不计入判据」的边界：只要求头注释与新面一致，不裁决措辞）——`crate头注释改原语族` | 单点 |
| crate 根出口 · 编译期可达 | 正向 | 测试文件的 `use` 面经 **crate 根**导入（`crate::{EditError, EditErrorKind, ElementSpec, ModelWrite, apply_element_spec, …, plan_relation_delete}` 与 `crate::{file_hash, file_hash_of}`）⇒ 任一名字离场即编译失败；运行期复核 `EditErrorKind::ALL.len() == 19`——`十二名经crate根可达` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `include_str!("../lib.rs")` 取编译期内嵌字节（禁测试内复制副本、禁读盘相对路径）；名字清单以 `use` 面为准，不手抄第二份出口表 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

<!-- 挂 AC-2（命令行为等价：按行 diff 恰 1 行 / 签名 / 错误面 / 失败零写入）、AC-5 的全链臂
     （命令 → plan_* → apply_* → write_atomic）、AC-7 的壳层出口（错误串 = EditError::message()
     形态）与 needle 随动。条目取自 design「公共函数 / API」子表的 `archi_set_gate_inner` 行
     （改，签名不变）；`archi_set_gate` 包装的签名与属性逐字不变，故既有包装用例只维持不重写。
     实测校准：`拒绝改档` 字面在基线只出现在迁移源 `edit_test.rs:374`（壳层 `mod_test.rs` 现存断言
     无该字面）——本节「改词断言」按 design 交接清单第 4 行以**核对**方式落地：落盘后扫一遍
     `archi_set_gate` 相关文案断言，仍在则改 `拒绝写回`，并把唯一改词点记在 `edit/mod_test.rs`
     的迁移用例上。
     **6 条新增用例的归属**（`改档落盘与期望文本逐字节相等` / `计数准确时改档零字节变化` /
     `陈旧表头随写回修正` / `命令错误串即message形态` / `inner体只调原语族` /
     `文件头指称改原语族`）：按 design D19 归本相位并入，proposal 测试表该行「其余零改动」按此
     扩写为「+ 6 条（4 条行为臂 + 2 条源码锚）」——每条锚一条既有 AC / 决策（AC-2 / AC-5 / AC-7 /
     D15），不引入新行为。**关系面（D18）不触本文件的命令面**：`archi_set_gate` 只写元素的
     `gate`，关系落点 / 重复声明判据没有壳层消费者，本节零新增关系用例。 -->

#### 待测功能

- `archi_set_gate_inner()`: 改（**签名不变** `(root: &Path, element: &str, gate: GateTier) -> Result<(), String>`）——函数体改为「组 `ElementSpec { fqn, gate: Some(gate), ..Default::default() }` → `plan_element_upsert` → `EditError::message()` 映射 → `write_atomic`」；错误面仍 `String`、失败零写入、命令层零领域规则（不判存在性 / 不判结构诊断）逐条不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_set_gate` / `archi_set_gate_inner` · 双入口一致 | 正向 | 包装与 `_inner` 各调一次均 `Ok`，两次改档结果一致（命令层零加工，既有用例维持）——`set_gate包装与inner一致` | 既有（维持） |
| `archi_set_gate` · 原子写回无残留 | 正向 | 改档后目标行变、注释逐字保留（`toml_edit` 渲染而非字符串替换）、读回新档位、模型声明目录内无临时文件残留（既有用例维持）——`set_gate改档原子写回无残留` | 既有（维持） |
| `archi_set_gate` · 按行 diff 恰 1 行（AC-2） | 正向 | `healthy` fixture（`CORE`：三元素一关系、**无**表头计数括号）改 `App.Auth` 为 `strict` ⇒ 落盘文件字节与「只把该行 `gate = \"off\"` 换成 `gate = \"strict\"`」的期望文本**逐字节相等**、按行 diff 恰 1 行（AC-2 的主判据）——`改档落盘与期望文本逐字节相等` | 单点（新增） |
| `archi_set_gate` · 全链零差异（AC-5 组合：命令 → plan → apply → write_atomic） | 正向 | 模型带**准确**表头括号（`# 01 · core（3 元素 / 1 关系）`）时把 `gate` 改成其当前值 ⇒ 文件**逐字节不变**（回填零差异经全链成立；本仓六文件的替代臂）——`计数准确时改档零字节变化` | 单点（新增） |
| `archi_set_gate` · 陈旧表头顺带修正（AC-5 / R-2） | 正向 | 模型表头写 `（9 元素 / 0 关系）` 而实为 3 元素 1 关系，改一次 `gate` ⇒ 文件表头变为 `（3 元素 / 1 关系）`、括号外标题与其余内容逐字（「唯一例外」口径的行为化）——`陈旧表头随写回修正` | 单点（新增） |
| `archi_set_gate` · 失败零写入 | 异常 | 元素缺席 / `root` 非法 ⇒ `Err` 且目标文件字节零变化（既有用例维持）——`set_gate失败零写入` | 既有（维持） |
| `archi_set_gate` · 结构错模型文案 | 异常 | `cycle` 模型改档 ⇒ `Err`，文案含「模型存在结构诊断」与「拒绝写回」（`拒绝改档` 已改词，MUST NOT 留旧判据句）+ 诊断条数——`结构错模型拒绝写回` | 既有（改 needle） |
| `archi_set_gate` · 错误串形态（AC-7 壳层出口） | 异常 | 任一 `Err` 串 == `EditError::message()` 形态：以判据句起始、恰一个全角 `：` 分隔、零「改档」字面；对照臂：`Err` 串不得是裸 `String` 自造文案（同一失败经 `plan_element_upsert` 直接取到的 `message()` 与命令面 `Err` **逐字相等**）——`命令错误串即message形态` | 单点（新增） |
| `archi_set_gate` · 三档往返 / 跨文件 / 插入 | 边界 | 三档逐值往返读回一致；目标元素在第二文件时第一文件零变化；元素无显式 `gate` 行 ⇒ 经插入获得该行（既有用例维持）——`set_gate往返与跨文件与插入` | 既有（维持） |
| `archi_set_gate` · 只读目录 | 边界 | `cfg(unix)` 目录置 0o555 ⇒ `Err` 且原文件不变、无 `archi-tmp` 残留（root 运行时可写穿则只断言无残留 + 可读，既有用例维持）——`write_atomic只读目录失败且无残留` | 既有（维持） |
| `archi_file_tree` 边界 · needle 随动 | 异常 | `archi_file_tree与唯一写命令的边界` 的负断言 needle 由 `plan_set_gate` 改为 `plan_element_upsert`（MUST NOT 留一条对已删名字的恒真空断言）；`write_atomic` needle 保留——`只读命令体内零写面名字` | 既有（改 needle） |
| `archi_set_gate_inner` · 零领域规则源码锚 | 边界 | `include_str!("mod.rs")` 的 `archi_set_gate_inner` 函数体内三 needle 在场（`plan_element_upsert` / `ElementSpec` / `.message()`），且零 `element(` 存在性判断 / 零 `diagnostics`（命令层不判存在性、不判结构诊断——判据全在 `archi::edit`）——`inner体只调原语族` | 单点（新增） |
| `archi_set_gate_inner` · 文件头指称随动 | 边界 | 模块头注释含 `archi::edit::plan_element_upsert`（写回原语族），零 `plan_set_gate` / 零 `GateWrite`（退役类型名不得留在壳层指称面）——`文件头指称改原语族` | 单点（新增） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无新增 mock | 本节 `archi_set_gate` 用例不触 store / vcs / 网络：真实 `TempDir` 落模型 + 真实 `archi::edit` 调用 + 真实 `write_atomic`（落盘原子性与零残留即被测语义，替身会抹掉它） | 本节全部用例 |
| Tauri 运行时（既有装置，非本节 mock） | `tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）是该文件既有装置，只服务 `archi_change_summary` 类命令；`archi_set_gate` 两条入口都吃 `&Path`，不经 State | 双入口一致用例（沿用，不新增） |

---

## 不可测试项

| 模块 | 原因 | 替代保证 |
|---|---|---|
| 1 · `packages/desktop/package.json`（`version` `0.7.9` → `0.7.10`）与 `src-tauri/tauri.conf.json` / `src-tauri/Cargo.toml` 的版本行 | 一行字面量，无函数 / 分支 / 输出可断言；版本纪律属交付面机械判据（AC-12） | 桌面端 `git diff` 核对：`package.json` 相对落地时 `HEAD` 恰 +1 patch、`tauri.conf.json` 仍 `"version": "../package.json"`、`src-tauri/Cargo.toml` 逐字不变；`git status --porcelain` 改动集 ⊆ 提案白名单且 `edit.rs` / `edit_test.rs` 两次删除与 `edit/` 两个新增同批出现 |
| 2 · AC-1 的**仓级生产面 grep 零命中**（`set_gate`（排除 `archi_set_gate` / `archi_set_gate_inner`）/ `plan_set_gate` / `GateWrite` 在 `crates/**` 与 `src/**` 的零命中，按 D16 排除 `*_test.rs` / `mod_test.rs`） | 仓级扫描不是可执行单元：单元测试读全仓源码会越过测试边界（并把别的 `*_test.rs` 的防线单元拖进判定面）；排除规则（测试文件白名单）也不宜由单测持有 | 单测面由两条源码锚兜住：`edit/mod_test.rs` 的 `crate根出口…` 组（12 名 + `#[cfg(test)] mod edit_test;` 离场）与 `src/commands/archi/mod_test.rs` 的 `inner体只调原语族` / `文件头指称改原语族`（壳层零旧名）；仓级零命中由桌面端 static-check 按 D16 口径执行（`grep -rn … \| grep -v '_test.rs'`）。反向：`commands/archi/mod_test.rs` 的三个 `set_gate*` 用例名保留（它们测的是 IPC 命令名，命令名不变） |
| 3 · AC-5 的「**本仓六个模型文件**跑一次『把某元素 `gate` 改成其当前值』的 plan ⇒ `content` 与源逐字节相等」 | 单元测试不读仓内模型目录（`archi` 既有测试全用合成 fixture，读真实仓面会让测试与仓数据耦合）；六个文件的计数准确性属数据面事实 | 合成 fixture 以六文件同形表头（`# NN · 域 分层（N 元素 / M 关系）`）覆盖「计数准确 ⇒ 零差异」与「陈旧 ⇒ 修正」两臂（`edit/mod_test.rs` 的 `计数准确即零差异`、`src/commands/archi/mod_test.rs` 的全链两例）；六个文件零字节变化由 `git diff -- .dev-team/architecture/graph/model/` 零输出核对（零字节变化即回填零差异），设计「起点实况」已登记六组计数 |
| 4 · AC-11 的生成物与仓级读数（`bindings.ts` / `bindings.zod.ts` 零 diff、`api:check` 零差异退出、`api:export` 对 `INDEX.toml` / `models/**/*.schema.json` / `.i.rs` / `.i.ts` 零 diff、`crates/core/archi/tests/**` 语料与金样逐字节不变、报告读数 55 / 129 / 0 / 0 / 39 / 53 / `clean` / `modelHash` `sha256:6cc50dbf…` 不变） | 派生生成物与仓级读数不是可执行单元：生成物 MUST NOT 手改（会被下次生成静默覆盖），读数由 `archi_drift` 报告产出 | 桌面端执行 `pnpm bindings:export` / `pnpm api:export` / `pnpm api:check` 后核对零 diff；语料与金样零改动由「不修改 `crates/core/archi/tests/**`」+ 既有语料套件全绿背书；读数由 `archi_drift(full)` 读数复核，且 `edit/` 改动不触判定面（`parse/**` / `model.rs` / `dto.rs` / `graph.rs` / `drift/report.rs` 零 diff 是同一判据的另一半，由改动集核对兜住） |
| 5 · AC-13 的 spec delta 形态（`specs/` 恰三个能力目录、RENAMED 的 `FROM:` / `TO:` 与基线标题逐字一致、MODIFIED 块为基线整段拷贝后改、每条 `### Requirement` 至少一个 `#### Scenario:` 且 WHEN/THEN 成对、`## Module Contract` 段） | spec 文本面，不是可执行单元；本变更不改写 delta 之外的能力 | 桌面端按 AC-13 逐条核对三份 delta（`desktop-archi-model` / `desktop-archi-drift` / `desktop-config-agent`）的标题链与 Scenario 计数；`openspec/specs/**` 基线零改动由改动集核对兜住 |
| 6 · D18 的**仓级实测事实**「129 条关系里 2 条声明于非 `from` 文件」（`Desktop → Desktop.Components.Ui`（`06-frontend.toml:196`）与 `Desktop → Desktop.GeneratedBindings`（`:201`），`[element."Desktop"]` 在 `01-app.toml:10`） | 单元测试不读仓内模型目录（同第 3 行：`archi` 既有测试全用合成 fixture）；仓级关系分布属数据面事实，写进用例即让测试与仓数据耦合 | 合成 fixture 以**同形命名**复刻该形态（`01-app.toml` 声明 `from` 元素、`06-frontend.toml` 声明 `(from, to)`），由 `既有关系声明于非from文件时原地更新` / `既有关系声明于非from文件时删除落声明文件` 两例钉住落点判据；仓级面由「`.dev-team/architecture/**` 零 diff」+ 回溯后 `archi_drift(full)` 仍 55 / 129 / `clean` 读数复核兜住（关系数据零变化 ⇒ 2 条同形事实不变） |
