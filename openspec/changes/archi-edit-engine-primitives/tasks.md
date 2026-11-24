# 任务: archi-edit-engine-primitives

> **覆盖口径**：覆盖 proposal「变更范围 - 实现文件」**全量 7 行**（`edit.rs` 删除、`edit/mod.rs` 新增、`edit/messages.rs` 新增、`drift/mod.rs`、`lib.rs`、`src-tauri/src/commands/archi/mod.rs`、`package.json`），并**单列一条退役删除**——`crates/core/archi/src/edit_test.rs`（与 `edit.rs` 同批离场，见 2.5；它 `use crate::edit::{plan_set_gate, set_gate}`，不删即 `cargo test` 编译失败 ⇒ 属退役动作而非测试编写）。proposal「测试文件」表其余**测试编写**（用例迁移 / 扩写 / needle 随动）**不属本相位、不入本文**——用例名 / needle / 拆分边界由 design 末节「交接给下游相位的随动清单」单点持有。
>
> **测试面交接**：本文件不写用例、不跑测试。唯一与测试文件相关的动作是**退役文件离场**——`src/edit_test.rs` 必须与 `src/edit.rs` 同批删除（它 `use crate::edit::{plan_set_gate, set_gate}`，两名字一离场即 `cargo test` 编译失败；AC-12 亦要求两次删除与 `edit/` 新增同批）。`edit/mod_test.rs` / `edit/messages_test.rs` 的落地与扩写归测试相位。
>
> **不在本 change**：agent 工具定义 / 注入矩阵 / 知识文本 / phase prompt（主篇切片 A）；`ArchiModuleDetail.modelHash` 与 `bindings.ts` 重导；落点策略（最近已声明祖先）与删除级联；`archi` 内的任何写盘。
>
> **静态步骤归属**：`pnpm check` 链与 `pnpm test` 由桌面端 static-check / test-execution 执行，本文件只登记边界与只读自查命令（`grep` / `git diff`）。`bindings:export` / `api:export` 属**生成步骤**，本相位只核对零 diff。
>
> **阶段收口可编译**：阶段一只加两个 `pub fn`（零消费者、零删除）⇒ 独立可编译；阶段二是**原子切换**（`src/edit.rs` 与 `src/edit/` 不能共存，壳层的 `plan_set_gate` 调用必须同批换成 `plan_element_upsert`）⇒ 一次提交收口；阶段三零代码（版本 + 只读核对）；**阶段四是回溯修订的小步重做**（`messages.rs` + `edit/mod.rs` + `drift/mod.rs` 三文件同批；测试文件不动，用例随动归测试相位）⇒ 一次提交收口，版本**不再二次 bump**。
>
> **回溯修订（第二轮）**：首版实现已落地（工作区未提交）后发现设计缺陷与两项归属未裁，design 已修订（D9 修订 / D18 / D19），本节据此新增**阶段四**。**阶段四的判据取代**：2.1 的「18 值」→ 19 值（+ `RelationDuplicated`）；2.4 的「关系落点 = `from` 元素声明文件」→ 既有关系落 `model.relations` 的 `file`、仅新增落 `from` 文件；阶段一的「`hash_with` 抽取」→ 撤回（`hash_files` 加私有 `read` 形参升为唯一累加核心）。
>
> 自查 cwd：`packages/desktop/src-tauri/crates/core/archi`（引擎面）、`packages/desktop/src-tauri`（壳层面）、`packages/desktop`（`package.json` / 生成物）、仓库根（模型文件零 diff）。

## 阶段零：基线核对（只读，确认 design「起点实况」仍成立）

- [x] 核对 `crates/core/archi/src/edit.rs` 仍 89 行且公开面恰三件（`GateWrite:9` / `set_gate:19` / `plan_set_gate:64`），10 条内联文案的行号与 design「起点实况」表一致
- [x] 核对 `crates/core/archi/src/drift/mod.rs`：`fn hash_files` 在 `:580` 附近、`Sha256::new()` 全 crate 恰 `:581` 一处、`model_hash` / `assets_hash` 的调用面未变
- [x] 核对 `crates/core/archi/src/lib.rs`：`pub mod edit;`（`:26`）与 `#[cfg(test)] mod edit_test;`（`:66`）在场；`pub use drift::{check, CheckError, CheckInput};`（`:49`）形状不变
- [x] 核对 `src/commands/archi/mod.rs:144` 仍是 `archi::edit::plan_set_gate(root, element, gate)?`、文件头 `:17` 仍指称它；`commands/archi/mod_test.rs:1624` 的负断言 needle 仍是 `["plan_set_gate", "write_atomic"]`
- [x] 核对六个模型文件表头计数（`2/0`、`9/10`、`8/25`、`1/14`、`3/3`、`32/77`）与 `packages/desktop/package.json` 的 `version = 0.7.9`；记录 `git status --porcelain` 基线（除本 change 目录外应为空）

## 阶段一：单文件哈希（`drift/mod.rs`，纯增量）

> **回溯修订**：本节「抽出 `hash_with`」的形态已由 design D9 修订撤回——改成「`hash_files` 加私有 `read` 形参、升为唯一累加核心」，使 `desktop-archi-drift` delta 的「两者都经既有私有 `hash_files`」字面成立。落地见阶段四 4.4；下列条目保留为当时事实记录。

- [x] 把 `:580` `hash_files` 的累加循环提为私有 `fn hash_with(files: &[PathBuf], base: &Path, read: &dyn Fn(&Path) -> Vec<u8>) -> String`——**唯一 `Sha256::new()` 拼装点**；逐文件累加「`relative_name(file, base)` + `0` + 字节 + `0`」的口径逐字照搬（名字 = `strip_prefix(base)` 后 `to_string_lossy().replace('\\', "/")`；读失败按空字节）
- [x] `fn hash_files(files: &[PathBuf], base: &Path) -> String` 改成 `hash_with(files, base, &|path| std::fs::read(path).unwrap_or_default())`：签名、可见性（私有）与输出逐字不变；`model_hash` / `assets_hash` 的实现与 doc 一行不动
- [x] 新增私有 `fn parent_of(path: &Path) -> &Path`（`path.parent().unwrap_or_else(|| Path::new(""))`），供两个新函数共用（文件名参与累加的口径单点）
- [x] 新增 `pub fn file_hash(path: &Path) -> String` = `hash_files(&[path.to_path_buf()], parent_of(path))`；doc 写明「`sha256:` + 64 位小写 hex」「读失败按空字节参与」「文件名参与累加 ⇒ 与 `modelHash` / `assets_hash` **数值不可比**，唯一可比对象是同一路径的前后取值」
- [x] 新增 `pub fn file_hash_of(path: &Path, content: &str) -> String` = `hash_with(&[path.to_path_buf()], parent_of(path), &|_| content.as_bytes().to_vec())`；doc 写明「写后等价锚：与把 `content` 落盘后的 `file_hash(path)` 逐字相等——写面据此把该值作为 `ModelWrite.hash`，调用方可直接拿去作下一次 `expect`」
- [x] 自查 阶段一：`grep -c 'Sha256::new()' crates/core/archi/src/drift/mod.rs` = 1；`grep -n 'fn hash_files\|fn hash_with\|fn parent_of\|pub fn file_hash\|pub fn file_hash_of' crates/core/archi/src/drift/mod.rs` 恰 5 处且 `hash_*` 均不带 `pub`；`git diff -- crates/core/archi/src/drift/mod.rs` 只含累加核心的抽取 + 两个新函数（`model_hash` / `assets_hash` / `check` / `CheckInput` / `CheckError` 的 hunk 零改动）

## 阶段二：写回原语族（`edit/` 原子切换，一次提交收口）

### 2.1 文案族（`edit/messages.rs`）

- [x] 新建 `crates/core/archi/src/edit/messages.rs`：`pub struct EditError { pub kind: EditErrorKind, pub facts: String }`（`#[derive(Debug, Clone, PartialEq, Eq)]`）+ `pub enum EditErrorKind`（`#[derive(Debug, Clone, Copy, PartialEq, Eq)]`，**18 值**，次序 = design 数据模型表 1–18）——**回溯修订：19 值（+ `RelationDuplicated`），见阶段四 4.1**
- [x] `impl EditErrorKind`：`pub fn criterion(self) -> &'static str`（穷尽 `match`，判据句逐字取 design 表，**零「改档」字面**）+ `pub const ALL: [EditErrorKind; 18]`（与 `match` 分支集合逐值相等）——**回溯修订：`[EditErrorKind; 19]`，见阶段四 4.1**
- [x] `impl EditError`：`pub fn message(&self) -> String` **恒为** `format!("{}：{}", self.kind.criterion(), self.facts)`；`EditError` **不实现** `Display` / `Error`（错误渲染只有这一个出口）
- [x] 同文件私有 `pub(super) fn error(kind: EditErrorKind, facts: impl Into<String>) -> EditError`：全 crate 的 `EditError` 构造单点（`MUST NOT` 在 `mod.rs` 里另有第二种拼装）
- [x] 自查 2.1：`grep -c '改档' crates/core/archi/src/edit/messages.rs` = 0；`grep -n 'EditErrorKind; 18\]' …/messages.rs` = 1（`ALL` 的长度即对拍判据）；`grep -c 'EditErrorKind::' …/messages.rs` = 36（`ALL` 18 项 + `criterion()` 18 臂）；`grep -n 'impl .*Display' …/messages.rs` 零命中 ——**回溯修订后应为：`EditErrorKind; 19]` = 1、`EditErrorKind::` = 38（19 + 19），见阶段四 4.5**

### 2.2 类型与模块骨架（`edit/mod.rs`）

- [x] 删除 `crates/core/archi/src/edit.rs`（三件公开面随之离场），新建 `crates/core/archi/src/edit/mod.rs`：模块 doc 写明「写回原语族：纯函数 `apply_*` + 计划函数 `plan_*`；零写盘、零 spawn、零 Tauri；写盘单点在壳层 `write_atomic`」与「自证对象 = 交付文本」的固定产出顺序
- [x] `edit/mod.rs` 声明**私有** `mod messages;` + `pub use messages::{EditError, EditErrorKind};`（公开面只有一个路径，MUST NOT `pub mod messages;`）；**两个 `#[cfg(test)] mod messages_test;` / `mod mod_test;` 不在本相位声明**——文件由测试相位落地时同批补入（先声明会因缺文件让 `cargo test` 解析失败，design D1）
- [x] 定义 `pub struct ModelWrite { pub file: PathBuf, pub content: String, pub hash: String }`（`#[derive(Debug, Clone, PartialEq, Eq)]`）与 `pub struct ElementSpec`（`#[derive(Debug, Clone, Default)]`，字段与 design 数据模型表逐字一致：`fqn` / `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` / `file` / `expect`）
- [x] `use` 面：`std::collections::BTreeMap` / `std::path::{Path, PathBuf}`、`toml_edit`、`crate::dto::{ApiStatus, ElementKind, GateTier}`、`crate::graph::ModelGraph`、`crate::model::{Element, Model}`、`crate::parse::{self, SourceParser, TomlParser}`、`foundation::layout::archi_graph_model_dir`、`super::messages::{error, EditError, EditErrorKind}`——**MUST NOT** `use sha2`（零独立哈希拼装）

### 2.3 `apply_*` 四函数（纯函数，单文件）

- [x] 私有 `fn parse_source(source: &str) -> Result<toml_edit::DocumentMut, EditError>`：解析失败 ⇒ `error(SourceParseFailed, …)`
- [x] 私有 `fn finish(document: toml_edit::DocumentMut, verify: impl FnOnce(&Model) -> Result<(), EditError>) -> Result<String, EditError>`：渲染 → `TomlParser::parse(&rendered)` 取 `elements` / `relations` 计数（诊断非空即 `RenderSelfCheck`）→ `backfill_header_counts` → 对交付文本再 `TomlParser::parse` 自证（诊断非空即 `RenderSelfCheck`）→ `verify(&model)` → `Ok(交付文本)`；**被自证 / 被回读的文本逐字等于返回值**
- [x] 私有 `fn backfill_header_counts(rendered: &str, elements: usize, relations: usize) -> String`：从首行起连续的空行 / `#` 注释行构成首部注释块（遇首个其它行即止），块内**第一处** `（<ASCII 数字> 元素 / <ASCII 数字> 关系）` 只替换两段数字；无该括号即原样返回（MUST NOT 新增）；块外零改动
- [x] （重做修正）回填的拼接域是**整份渲染文本**：尾段取 `rendered[relation_end..]`，MUST NOT 取首部注释块切片 `header[relation_end..]`——后者把「（N 元素 / M 关系）」之后的全部模型表丢掉，且自证重解析零诊断（交付文本只剩注释块）、`verify_*` 回读必失、`archi_set_gate` 在既有模型上全 `Err`；六个模型文件表头均带括号，故属必现缺陷。回归用例由测试相位落地（design 末节 `edit/mod_test.rs` ③「回填后正文不截断」一例）
- [x] 私有 `fn element_table_mut` 形态的取用（或就地内联）：`document.as_table_mut().get_mut("element").and_then(Item::as_table_like_mut)`；**更新既有键时沿用原值 decor**（把旧 item 的 `decor().clone()` 拷到新值），新键用默认 decor（D7）
- [x] `pub fn apply_element_spec(source: &str, spec: &ElementSpec) -> Result<String, EditError>`：解析 → 定位 `element` 表；元素缺席 ⇒ 共用 `check_create(&spec.fqn, spec.kind)`（FQN 形态 → `kind` 必填）后**新建**表（`Table::new()` 按 `kind`/`gate`/`api`/`entry`/`path`/`doc`/`summary`/`metadata` 规范序写指定字段，`element` 外层表不存在时置 implicit 以渲染成 `[element."<FQN>"]` 单层表头）；元素在案 + `spec.kind` 有值 ⇒ `ElementAlreadyExists`；在案则逐位三态改写（`entry`/`doc`/`summary` = `Option<Option<String>>`；`path` = `Option<Vec<String>>`，`Some(vec![])` 删键、其余写内联数组；`metadata` = `Option<BTreeMap<..>>`，`Some(空)` 删键、其余写**内联表**；`gate`/`api` = `Option<枚举>` 写线值）；条目非表形态 ⇒ `ElementNotTable`
- [x] `pub fn apply_element_delete(source: &str, fqn: &str) -> Result<String, EditError>`：解析 → `element` 表缺席 ⇒ `ElementTableMissing`；元素缺席 ⇒ `ElementNotFound`（facts 列该文件现有元素）；命中即 `remove`，回读断言该元素已不在模型
- [x] `pub fn apply_relation_upsert(source: &str, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>) -> Result<String, EditError>`：解析 → `document["relation"]` 的 `ArrayOfTables`（键缺席 ⇒ 新建空数组，**不报缺表**）→ 命中 `from` 与 `to` 双等的条目即原地三态改写（条数不变），缺席则追加 `Table`（键序 `from` / `to` / `desc` / `apis`，`decor_mut().set_prefix("\n")` 与既有条目同形）；`apis = Some(&[])` ⇒ 删键（MUST NOT 渲染 `apis = []`）——**回溯修订：同一源内该对 ≥ 2 处 ⇒ `RelationDuplicated`、回读断言关系条数，见阶段四 4.2**
- [x] `pub fn apply_relation_delete(source: &str, from: &str, to: &str) -> Result<String, EditError>`：命中即 `remove` 该条目，缺席 ⇒ `RelationNotFound`（facts 列该文件现有 `from → to` 对）——**回溯修订：只删唯一命中（`retain` 退役）、同一源内 ≥ 2 处 ⇒ `RelationDuplicated`、回读断条数 −1，见阶段四 4.2**
- [x] 回读纪律（四函数共用）：`apply_element_spec` 逐**指定字段**比对（`gate` 的字段名为「档位」、另 `kind`/`api`/`entry`/`path`/`doc`/`summary`/`metadata`；新建另断言元素在案，缺席即 `ReadBackMismatch`「找不到元素」）；`apply_element_delete` 断言缺席；关系两函数断言 `(from, to)` 的 `description` / `apis` 与 spec 一致 / 已缺席
- [x] 自查 2.3：`grep -rn 'Sha256\|sha2' crates/core/archi/src/edit/` 零命中；`grep -rn 'fs::write\|write_atomic\|spawn' crates/core/archi/src/edit/` 零命中；`grep -c 'fn backfill_header_counts\|fn finish\|fn parse_source' …/edit/mod.rs` = 3 且三者均不带 `pub`；`grep -n 'apis = \[\]' …/edit/mod.rs` 零命中

### 2.4 `plan_*` 四函数（读盘，零写盘）

- [x] 私有 `fn load_model(root: &Path) -> Result<ModelGraph, EditError>`：`parse::parse_dir(root)` → 诊断非空 ⇒ `error(ModelHasDiagnostics, "{N} 条（先修好模型）：{kinds}")` → `ModelGraph::build(outcome.model)`（层级与邻接判据全走它，MUST NOT 自写前缀扫描）
- [x] 私有 `fn check_create(fqn: &str, kind: Option<ElementKind>) -> Result<(), EditError>`：FQN 形态（非空、无空白 / `"` / `[` / `]` / `#` / 换行）⇒ `FqnInvalid`；`kind` 缺 ⇒ `ElementKindMissing`——`apply_element_spec` 与 `plan_element_upsert` **共用**（同一判据一份措辞；MUST NOT 两处各写一份）
- [x] 私有 `fn check_target(root: &Path, fqn: &str, file: Option<&str>, existing: Option<&Element>) -> Result<PathBuf, EditError>`：既有元素 ⇒ `root.join(&element.file)`，给了不同 `file` 即 `TargetInvalid`；新建 ⇒ `file` 必填、单段非绝对、`.toml` 结尾、`archi_graph_model_dir(root)` 必须是已存在目录（不代建）⇒ 否则 `TargetInvalid`（四臂 facts 见 design）
- [x] 私有 `fn read_source(file: &Path, allow_missing: bool) -> Result<String, EditError>`：读失败且 `allow_missing` ⇒ `Ok(String::new())`（新建且目标文件尚不存在）；否则 `SourceReadFailed`（facts `"{path}（{error}）"`）
- [x] 私有 `fn check_expect(file: &Path, source: &str, expect: Option<&str>) -> Result<(), EditError>`：`None` ⇒ `Ok`；`Some(期望)` 且目标文件不在盘 ⇒ `ExpectMismatch`（facts 点名文件缺席——不存在「期望缺席」臂）；否则与 `crate::drift::file_hash_of(file, source)` 比对，不等 ⇒ `ExpectMismatch`（facts `期望 …，实际 …——重读后再试`）
- [x] `pub fn plan_element_upsert(root: &Path, spec: &ElementSpec) -> Result<ModelWrite, EditError>`：`load_model` → 在案判别（`kind` 有值即 `ElementAlreadyExists`，facts 点名声明文件）→ **新建时先 `check_create(&spec.fqn, spec.kind)`**（保证「缺席 + 缺 kind」的判据句恒为 `ElementKindMissing`，不被落点判据抢先）→ `check_target` → 新建时父元素（`fqn.rsplit_once('.')` 的直接父）不在案 ⇒ `ParentMissing`（单段 FQN 无父，跳过）→ `read_source` → `check_expect` → `apply_element_spec` → `ModelWrite { file, content, hash: file_hash_of(&file, &content) }`
- [x] `pub fn plan_element_delete(root: &Path, fqn: &str, expect: Option<&str>) -> Result<ModelWrite, EditError>`：`load_model` → 元素缺席 ⇒ `ElementNotFound`（facts `"{fqn}（模型共 N 个元素）"`）→ 直接子元素非空（`graph.children(fqn)`）⇒ `DeleteHasChildren`（facts 点名 FQN）→ 出边或入边非空（`depends_on` ∪ `depended_by`）⇒ `DeleteHasRelations`（facts 点名 `from → to`）→ 读源 → `check_expect` → `apply_element_delete` → 填 `hash`
- [x] `pub fn plan_relation_upsert(root: &Path, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>, expect: Option<&str>) -> Result<ModelWrite, EditError>`：`load_model` → 两端点缺席即 `RelationEndpointMissing`（facts 点名缺失端）→ 落点 = `from` 元素声明文件 → 读源 → `check_expect` → `apply_relation_upsert` → 填 `hash`——**回溯修订：落点判据被阶段四 4.3 取代（既有关系 = `model.relations` 的 `file`，仅新增落 `from` 文件；重复声明 ⇒ `RelationDuplicated`）**
- [x] `pub fn plan_relation_delete(root: &Path, from: &str, to: &str, expect: Option<&str>) -> Result<ModelWrite, EditError>`：同上前置 → 读源 → `check_expect` → `apply_relation_delete` → 填 `hash`——**回溯修订：落点 = 关系声明文件、模型面缺席即 `RelationNotFound`、重复声明 ⇒ `RelationDuplicated`，见阶段四 4.3**
- [x] 自查 2.4：`grep -c 'pub fn plan_' crates/core/archi/src/edit/mod.rs` = 4 且签名与 proposal 表逐字一致（`&Path` / `&ElementSpec` / `Option<Option<&str>>` / `Option<&[String]>` / `Option<&str>` 逐个核对）；`grep -n 'fs::read_to_string' …/edit/mod.rs` 恰 1 处（只在 `read_source`）；`grep -n 'parse::parse_dir' …/edit/mod.rs` 恰 1 处（只在 `load_model`）

### 2.5 退役与接线（同批收口）

- [x] 删除 `crates/core/archi/src/edit_test.rs`（506 行 / 14 条 `#[test]`，`use crate::edit::{plan_set_gate, set_gate}`——不删即 `cargo test` 编译失败）；它的模块声明 `#[cfg(test)] mod edit_test;` 由**同批的下一条 `lib.rs` 任务**删除（两处必须在同一提交内落地）；测例的迁入与扩写归测试相位
- [x] `crates/core/archi/src/lib.rs`：crate 头注释的「`gate` 写回纯函数」改为「写回原语族（元素与关系的 upsert / delete）」；`#[cfg(test)] mod edit_test;` 删除；`pub use drift::{check, CheckError, CheckInput};` 补 `file_hash, file_hash_of`；新增 `pub use edit::{apply_element_delete, apply_element_spec, apply_relation_delete, apply_relation_upsert, plan_element_delete, plan_element_upsert, plan_relation_delete, plan_relation_upsert, EditError, EditErrorKind, ElementSpec, ModelWrite};`；其余出口逐字不变
- [x] `src/commands/archi/mod.rs`：`archi_set_gate_inner` 改为 `let spec = ElementSpec { fqn: element.to_owned(), gate: Some(gate), ..Default::default() }; let plan = plan_element_upsert(root, &spec).map_err(|error| error.message())?; write_atomic(&plan.file, &plan.content)`；`use` 面补 `archi::edit::{plan_element_upsert, ElementSpec}`（或经 `archi::{…}` 重导）；函数签名 / `#[tauri::command] #[specta::specta]` / `Result<(), String>` 逐字不变
- [x] 同文件文件头注释（`:16`–`:19`）的 `archi::edit::plan_set_gate` 指称改为 `archi::edit::plan_element_upsert`（写回原语族的 gate 退化调用）；`archi_set_gate_inner` 的 doc 保留「root 检查 → 写前校验与待落盘产物 → 原子写回，任一校验失败零写入」
- [x] 自查 2.5：`grep -rn 'plan_set_gate\|GateWrite' packages/desktop/src-tauri --include=*.rs` 在**生产面零命中**——唯一残留是 `src/commands/archi/mod_test.rs` 的两处 needle，归测试相位随动（design 末节随动清单第 4 行 ①；D16 把 AC-1 的 grep 口径限定在生产面）；`grep -rn 'set_gate' packages/desktop/src-tauri --include=*.rs | grep -v 'archi_set_gate'` 的命中只落在 `*_test.rs` / `mod_test.rs`（D16 口径）；公开面计数：`grep -c '^pub fn ' crates/core/archi/src/edit/mod.rs` = 8、`grep -c '^pub struct \|^pub enum ' crates/core/archi/src/edit/`（两文件合计）= 4、`grep -c '^pub fn ' crates/core/archi/src/edit/messages.rs` = 2（`message` / `criterion`）、`grep -c 'pub const ALL' …/messages.rs` = 1；`grep -n '^pub mod\|^pub use' crates/core/archi/src/edit/mod.rs` 只含 `pub use messages::{EditError, EditErrorKind};`（`mod messages;` 不带 `pub`）

## 阶段三：版本与零扩散核对（不重复执行桌面端静态步骤）

- [x] `packages/desktop/package.json`：`"version"` `0.7.9` → `0.7.10`（相对落地时 `HEAD` 恰 +1 patch）；核对 `src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`、`src-tauri/Cargo.toml` 的版本行逐字未变 ——**回溯修订不再二次 bump：阶段四只改同 change 内的三个源文件，`0.7.10` 保持（AGENT.md「每次 desktop 源码变更 +1 patch」按 change 记一次）**
- [x] 生成物零 diff 核对（生成步骤由桌面端执行后核对）：`pnpm bindings:export` ⇒ `git diff -- packages/desktop/src/types/generated/` **零输出**；`pnpm api:export` ⇒ `.dev-team/architecture/graph/INDEX.toml` / `models/**/*.schema.json` / `.i.rs` / `.i.ts` **零 diff**；`pnpm api:check` 零差异退出
- [x] 零扩散核对（`git status --porcelain`）：改动集 ⊆ `edit.rs`（D）+ `edit/mod.rs` / `edit/messages.rs`（新增）+ `edit_test.rs`（D）+ `drift/mod.rs` + `lib.rs` + `src-tauri/src/commands/archi/mod.rs` + `package.json` + 本 change 目录文稿与 `specs/`；出现 `parse/**` / `model.rs` / `dto.rs` / `graph.rs` / `drift/report.rs` / `crates/core/archi/tests/**` / `crates/core/config/**` / 前端产品代码 / `.dev-team/architecture/**` / `openspec/specs/**` / `src-tauri/Cargo.toml` / `tauri.conf.json` 任一 diff 即回退
- [x] 读数核对（只读）：`crates/core/archi/tests/**` 与 `.dev-team/architecture/**` 零 diff ⇒ 语料 / 金样逐字节不变、`modelHash` `sha256:6cc50dbf…` 逐字不变、六个模型文件表头计数逐字不变；`apiFindings` 仍 39（新公开面被 `docMissing` 短路，design 事实校准 1）；`REPORT_VERSION` 与九值 `ALL` 常量零改动
- [x] 验收对照：逐条回应 design「验收标准回应」表（AC-1…AC-13）——AC-1 由阶段二自查（公开面 12 项 + 退役名零命中）背书；AC-2 / 3 / 4 / 5 / 6 / 9 / 10 由阶段二 2.3 / 2.4 的实现点与测试相位断言背书（本相位只核 `expect` 校验点唯一、回填在自证之前、删除前置在 `plan_element_delete` 一处）；AC-7 由 2.1 自查（零「改档」+ 18 值对拍）；AC-8 由阶段一自查；AC-11 由本条生成物与读数核对；AC-12 由版本与白名单；AC-13 由 3 份 spec delta 承接（本变更不改写）——**回溯修订的复核对齐：AC-7 的 18 值 → 19 值（阶段四 4.1 / 4.5）、AC-8 的 `hash_with` → `hash_files` 三形参（4.4）、AC-10 的关系落点 / 重复声明 / 回读条数（4.2 / 4.3）**

## 阶段四：回溯修订（关系落点与重复声明 + 哈希形态 + 归属裁定；design D9 修订 / D18 / D19）

> 触发：首版实现按「关系落点 = `from` 元素声明文件」落地，与 AC-10「既有走更新 / 在场删除成功」不自洽——本仓 `Desktop → Desktop.Components.Ui`（`06-frontend.toml:196`）与 `Desktop → Desktop.GeneratedBindings`（`:201`）两条既有关系的声明文件 ≠ `from` 元素文件（`[element."Desktop"]` 在 `01-app.toml:10`，design 事实校准 13）。附带裁定两项归属：`file_hash_of` 措辞归属（D9 修订：实现向 spec 措辞收敛、撤回 `hash_with`）与壳层 6 条新增用例归属（D19：归测试相位、授权并入）。范围 = `edit/messages.rs` + `edit/mod.rs` + `drift/mod.rs` 三个**已登记**文件；**测试文件不动**（用例随动归测试相位，见 design 末节交接清单）。

### 4.1 文案族 +1 值（`edit/messages.rs`）

- [x] `EditErrorKind` 增 `RelationDuplicated`（紧接 `RelationNotFound` 之后 ⇒ 新序 1–19；`RenderSelfCheck` / `ReadBackMismatch` 顺延为 #18 / #19，判据句与 facts 逐字不变），`criterion()` 出「关系多处声明」
- [x] `criterion()` 的穷尽 `match` 补该臂；`ALL: [EditErrorKind; 19]` 按同序补入；`message()` / `pub(super) fn error` / 其余 18 臂逐字不动
- [x] facts 两臂（构造侧拼）：模型面 `"{from} → {to} 在 {file} / {file} 多处声明（{n} 处）——先删除重复条目"`；文件面 `"{from} → {to} 在同一源内多处声明（{n} 处）——先删除重复条目"`

### 4.2 `apply_relation_*` 文件面（`edit/mod.rs`）

- [x] `apply_relation_upsert`：命中计数 ≥ 2 ⇒ `RelationDuplicated`（文件面 facts）；恰 1 ⇒ 原地三态改写（规则不变）；0 ⇒ 追加（规则不变）
- [x] `apply_relation_delete`：命中计数 ≥ 2 ⇒ `RelationDuplicated`；恰 1 ⇒ **只删该条目**（`retain` 退役）；0 ⇒ `RelationNotFound`（facts 列该文件现有对，逐字不变）
- [x] 两函数的回读闭包补**关系条数**断言：`before` = 编辑前 `ArrayOfTables` 条目数；既有 ⇒ `model.relations.len() == before`、追加 ⇒ `before + 1`、删除 ⇒ `before - 1`（不符 ⇒ `ReadBackMismatch`，facts「关系条数为 {actual}，目标为 {expected}」）

### 4.3 `plan_relation_*` 模型面（`edit/mod.rs`）

- [x] 私有 `fn relation_hits<'a>(model: &'a Model, from: &str, to: &str) -> Vec<&'a Relation>`：身份键 = `from` / `to` 双等（与 `diff::model_diff` 的身份键、`relation::relation_detail` 的查找谓词同口径，MUST NOT 引第三种）
- [x] 私有 `fn check_relation_unique(hits: &[&Relation]) -> Result<(), EditError>`：`len() >= 2` ⇒ `RelationDuplicated`（模型面 facts 点名各声明文件与处数）；`len() <= 1` ⇒ `Ok`
- [x] `plan_relation_upsert`：端点判据之后插 `check_relation_unique`；落点 = 命中 **1** 处时 `root.join(&relation.file)`（该条目的 `file`）、命中 **0** 处时 `root.join(&from_element.file)`；`read_source` / `check_expect` / `apply_relation_upsert` / `hash` 的顺序不变
- [x] `plan_relation_delete`：端点判据之后插 `check_relation_unique`；命中 **0** 处 ⇒ **模型面** `RelationNotFound`（facts 列该 `from` 元素的现有出边对，MUST NOT 读 `from` 文件后由 apply 报「该文件没有关系」）；命中 **1** 处 ⇒ 落点 = `root.join(&relation.file)`；`from_element.file` 不再参与 delete 落点
- [x] `use` 面补 `crate::model::Relation`（`relation_hits` 的返回类型；`model.rs` 零改动）；`graph.children` / `depends_on` / `depended_by` 的调用面不动
- [x] `expect` 语义不变（绑**落点文件**）；`ModelWrite { file, content, hash }` 三元组与两函数签名逐字不变

### 4.4 哈希形态（`drift/mod.rs`，撤回 `hash_with`）

- [x] `fn hash_files(files: &[PathBuf], base: &Path, read: &dyn Fn(&Path) -> Vec<u8>) -> String`：把 `hash_with` 的函数体移回 `hash_files`（累加口径「相对 base 的名字 + `0` + 字节 + `0`」逐字不动），删 `hash_with`
- [x] 私有 `fn read_bytes(path: &Path) -> Vec<u8>` = `std::fs::read(path).unwrap_or_default()`（「读失败按空字节」的单点）；`model_hash` / `assets_hash` 的调用点改传 `&read_bytes`，**输出逐字不变**
- [x] `file_hash` = `hash_files(&[path.to_path_buf()], parent_of(path), &read_bytes)`；`file_hash_of` = `hash_files(&[path.to_path_buf()], parent_of(path), <返回 `content` 字节的 reader>)`；两函数 doc 与 `parent_of` 不动
- [x] 自查 4.4：`grep -c 'Sha256::new()' crates/core/archi/src/drift/mod.rs` = 1；`grep -n 'hash_with' crates/core/archi/src/drift/mod.rs` 零命中；`grep -n 'fn hash_files\|fn read_bytes\|fn parent_of\|pub fn file_hash\|pub fn file_hash_of' crates/core/archi/src/drift/mod.rs` 恰 5 处且 `hash_files` / `read_bytes` / `parent_of` 不带 `pub`

### 4.5 自查（生产面只读）

- [x] `grep -c 'EditErrorKind; 19\]' crates/core/archi/src/edit/messages.rs` = 1；`grep -c 'EditErrorKind::' crates/core/archi/src/edit/messages.rs` 命中代码面 38 处（`ALL` 19 + `criterion()` 19），按行计 40 行（另有 2 行模块文档的 `[`EditErrorKind::…`]` 链接）；`grep -c '改档' …/messages.rs` = 0
- [x] `grep -rn 'RelationDuplicated' crates/core/archi/src/edit/` 只落 `messages.rs`（定义 + `ALL` + `criterion`）与 `mod.rs`（模型面 / 文件面两处构造 + 判据 helper）；`grep -n 'retain' crates/core/archi/src/edit/mod.rs` 零命中
- [x] `grep -rn 'Sha256\|sha2' crates/core/archi/src/edit/` 生产面（`edit/mod.rs` / `edit/messages.rs`）零命中——唯一命中是 `edit/mod_test.rs` 的源码锚字符串（测试相位持有）；`grep -rn 'hash_with' packages/desktop/src-tauri/crates/core/archi/src/edit/` 零命中
- [x] `grep -n 'from_element.file' crates/core/archi/src/edit/mod.rs` 只允许落在 upsert 的「新增关系（命中 0 处）」分支；delete 分支零命中

### 4.6 归属与交接（不改测试文件、不改 spec delta）

- [x] 核对**不改**任何 spec delta（`openspec/changes/archi-edit-engine-primitives/specs/**`）：`desktop-archi-drift` 的三处哈希措辞在 4.4 落地后**字面成立**（「两者都经既有私有 `hash_files`」「全仓只有一处累加实现」）；`desktop-archi-model` 的「关系落点」bullet 由 D18 补齐可实现语义（要求面完整持有的回落见 design 待决问题 5）
- [x] 测试交接以 design 末节「交接给下游相位的随动清单」为**单一持有**：`edit/mod_test.rs` ⑤（非 `from` 文件的既有关系 upsert / delete、重复声明两臂、回读条数）、`edit/messages_test.rs`（19 值）、`drift/mod_test.rs`（`hash_files` 三形参 / `注入reader` 改挂 / 注释）、`src-tauri/src/commands/archi/mod_test.rs`（6 条新增用例归测试相位）——本相位**不写、不跑**测试

### 4.7 收口核对

- [x] `git status --porcelain` 改动集仍 ⊆ 阶段三白名单（阶段四只改 `edit/messages.rs` / `edit/mod.rs` / `drift/mod.rs` 三个已登记路径，零新增文件、零删除文件）
- [x] `crates/core/archi/tests/**` 与 `.dev-team/architecture/**` 零 diff（零语料 / 零数据变化；六个模型文件表头计数逐字不变 ⇒ 回填零差异的判据不因本修订变化）；`packages/desktop/package.json` 仍 `0.7.10`（不二次 bump）
