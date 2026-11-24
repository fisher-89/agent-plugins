# 设计: archi-edit-engine-primitives

> **变更**: archi-edit-engine-primitives（架构写引擎原语 · `edit` 模块泛化）
> **日期**: 2026-11-24
> **基于**: `proposal.md`（问题四节 + 提案八节 + 能力三份 delta + AC-1…AC-13 + R-1…R-12）、`explore.md`（只读输入）、`specs/` 下 3 份 delta（`desktop-archi-model` / `desktop-archi-drift` / `desktop-config-agent`）
> **性质**: **纯引擎演进**——`archi::edit` 的单函数（gate 改档）泛化为写回原语族（4 × `apply_*` 纯函数 + 4 × `plan_*` 计划函数），文案族抽为封闭枚举，`drift` 补两个单文件哈希薄封装，写面补 `expect` 乐观锁与表头计数回填。零新增 IPC 命令、零线面（DTO / `bindings.ts`）变化、零模型数据变化、零判据面变化。
> **不含**: agent 工具定义 / 注入矩阵 / 知识文本 / phase prompt（主篇切片 A）；`ArchiModuleDetail.modelHash`（线面变化，A 一并接线）；落点策略（最近已声明祖先）与删除级联语义（A）；`archi` 内的任何写盘（写盘单点仍是壳层 `write_atomic`）。
> **回溯修订（第二轮 · 本文件为修订后版本）**: 首版设计已落地为工作区实现（未提交），核查发现**一处设计缺陷**与**两项归属未裁**。本版修订四件：① **关系落点与重复声明**（D18 · 缺陷修复）——既有 `(from, to)` MUST 经 `model.relations` 的 `file` 单点定位并原地改 / 删，仅**新增**关系落 `from` 文件；重复声明 fail-closed（新增 `EditErrorKind::RelationDuplicated`，错误面 18 → **19 值**）。首版「落点 = `from` 元素声明所在文件」在 `Desktop → Desktop.Components.Ui` 这类既有关系上会把更新写成重复声明、让在场删除报 `RelationNotFound`（事实校准 13）。② **哈希形态**（D9 修订 · `file_hash_of` 措辞归属的裁定）——撤回 `hash_with`，`hash_files` 加私有 `read` 形参升为**唯一累加核心**，使 `desktop-archi-drift` delta 的「两者都经既有私有 `hash_files`」字面成立（零 spec 改写）。③ **壳层 6 条新增用例**（D19 · 归属裁定）——归测试相位、授权并入（proposal 测试表该行「其余零改动」按此扩写）。④ **测试交接同步**（非本相位写测试）——非 `from` 文件的既有关系、重复声明、`hash_files` 三形参、19 值四处随动登记在末节交接清单。

---

## 起点实况（`HEAD` = `98b4e77` 基线实测）

| 面 | 实测（文件:行） |
|---|---|
| 写面现状 | `crates/core/archi/src/edit.rs` **89 行三件**：`GateWrite`（`:9`，`{ file, content }`）、`set_gate`（`:19`）、`plan_set_gate`（`:64`）；`lib.rs:26` `pub mod edit;`、`:66` `#[cfg(test)] mod edit_test;` |
| 既有文案（10 条，内联 `format!` / `ok_or_else`） | `:22` `模型源解析失败：{error}` / `:32` `元素不存在：{fqn}（现有元素：…）` / `:39` `元素 {fqn} 的声明不是表形态：无法改档` / `:47` `改档渲染自证失败（重解析出 {n} 条诊断），未产出文本` / `:54` `改档渲染自证失败：回读档位为 {x}，目标为 {y}` / `:58` `改档渲染自证失败：回读找不到元素 {fqn}` / `:68` `模型存在 {n} 条结构诊断，拒绝改档（先修好模型）：{kinds}` / `:80` `元素不存在：{fqn}（模型共 {n} 个元素）`；另有 `模型源缺少 element 表：无从定位元素`（`:28`）与 `模型文件读取失败（{path}）：{error}`（`:86`） |
| 唯一消费者 | `src/commands/archi/mod.rs:144` `archi::edit::plan_set_gate(root, element, gate)?`；文件头 `:17` 指称它。全仓 `plan_set_gate` / `GateWrite` 只此两处 + `edit_test.rs` |
| 既有测例 | `crates/core/archi/src/edit_test.rs` **506 行 / 14 条 `#[test]`**（排版保留 / 无 gate 行插入 / 三档往返 / 元素缺席 / 注入式 FQN / 坏源与无 element 表 / 排版五形态 / 特殊 FQN / 500 元素规模 / 定位与零写盘 / plan 异常三形态 / plan 边界 / entry 保留两条）；壳层 `commands/archi/mod_test.rs:1624` 的负断言 needle 含 `plan_set_gate`，`:441/:466/:495` 三个用例名以 `set_gate` 起头 |
| 哈希单点 | `drift/mod.rs:580` `fn hash_files(files: &[PathBuf], base: &Path) -> String`（私有）；`:573` `model_hash`；`:606` `pub fn assets_hash`；`Sha256::new()` 全 crate 恰 `:581` 一处；`use sha2::{Digest, Sha256};`（`:26`） |
| 层级 / 邻接判据 | `graph.rs:108` `children`（**直接子元素**：`strip_prefix("{fqn}.")` 且余下无 `.`）；`:147` `orphan_hierarchy`（自叶向上逐级查**所有**祖先）；`depends_on` / `depended_by`（`out_edges` / `in_edges`）；`ModelGraph::build` 是唯一建图点 |
| 模型数据形态 | 元素 = `[element."<完整 FQN>"]` 单层表；字段序 `kind / gate / api / entry / path / doc / summary / metadata`；关系 = `[[relation]]` 条目数组（`from` / `to` / `desc` / `apis`），条目间空行；`metadata` 只可能是**内联表**（语料 `tests/corpus/dsl/gate-docs-metadata/01-core.toml:9` `metadata = { owner = "core", tier = "1" }`），`apis` 是内联数组（`tests/corpus/drift/api-declared-exact/models/01-app.toml:23`）；本仓六个模型文件**零 `metadata` / 零 `apis`** |
| 六个表头 | `01-app` `（2 元素 / 0 关系）` / `02-backend-core` `（9 元素 / 10 关系）` / `03-backend-infra` `（8 元素 / 25 关系）` / `04-backend-shell` `（1 元素 / 14 关系）` / `05-backend-tools` `（3 元素 / 3 关系）` / `06-frontend` `（32 元素 / 77 关系）`；括号后全角 `）`，数字为 ASCII |
| 既有 toml_edit 范式 | `api/schema.rs:330` `toml_edit::ArrayOfTables::new()` / `:350` `document["entry"] = Item::ArrayOfTables(array)` / `:325` `document.decor_mut().set_prefix(…)`（首部注释块即 `DocumentMut` 的 decor 前缀）；`edit.rs:40` 既有的「改既有键」写法是 `entry.insert("gate", toml_edit::value(gate.as_str()))` |
| 依赖就绪 | `archi/Cargo.toml` 已含 `toml_edit`（workspace `0.25`）与 `sha2`（`0.10`）——**零新增依赖** |
| 报告读数 | `reports/archi/20261124T1011324160001Z.json`：`elements` 55 / `relations` 129 / `structuralErrors` 0 / `model.warnings` 0 / `apiFindings` **39（全 `docMissing`）** / `observations` 53 / `status = clean` / `modelHash` `sha256:6cc50dbf…` |
| 版本 | `packages/desktop/package.json` = **0.7.9**；`src-tauri/tauri.conf.json` = `"../package.json"`；`src-tauri/Cargo.toml` = `0.1.0`（自立版本） |

---

## 事实校准（不改提案意图，只定实现形态）

1. **新公开面不会改动 `apiFindings`**：`Backend.Core.Archi` 是 `api = declared` 且派生契约 md 不在盘 ⇒ 判定链 ④ `docMissing` 直接短路（`api/check.rs:127`），走不到「⑬ 代码有 `external` 面但契约未声明」（`apiUndeclared`，档位 `off`）。故新增 12 个 `pub` 项不改变 39 条读数，AC-11 可作硬判据。
2. **表头注释确实在 `DocumentMut` 的 decor 前缀里**（`api/schema.rs:325` 的既有写法即证据），但回填**按渲染后的文本块扫描**实现——见 D8 的三条理由。两者产出的字节相同，差别只在「谁持有回填动作」。
3. **既有「改键」写法会重置该行的 decor**：`entry.insert(key, value(…))` 给的是默认 decor（默认前缀 `" "`、默认后缀空），目标行若带行内注释或非标准缩进会被规范化。本仓六个模型的 `gate` 行都是纯值行，故 AC-5 的零差异**今日成立**；本设计仍把「更新既有键沿用原 decor」定为纪律（D7），使 AC-5 不依赖源文本形态。
4. **`orphanHierarchy` 查的是全祖先链**（`graph.rs:147` 逐级上溯）：模型零诊断 ⇒ 祖先链闭合 ⇒ 新建元素只要**直接父**在案，其继承链必在案。故新建的父判据只查直接父（`fqn.rsplit_once('.')`），不重写全链遍历。
5. **直接子元素是删除阻塞的正确口径**：`App.A.B` 在案而 `App.A` 不在案属孤儿诊断（模型零诊断时不成立）⇒ 有后代必有直接子元素 ⇒ 查直接子元素即等价于查全部后代，且与 `ModelGraph::children` 同源。
6. **`parse_dir` 对目录缺席 / 空目录返回空模型零诊断**（`parse/mod.rs:51`）⇒ 新建元素的「模型声明目录缺席」必须**单独判**，否则会退化成「往不存在的目录里写」。
7. **文件可以没有 `relation` 键**（`01-app.toml` 就是 0 关系）⇒ 关系 upsert 在键缺席时**新建** `ArrayOfTables`，不报「缺表」。
8. **`hash_files` 加一个「字节来源」注入点即可让 `file_hash_of` 字面经由它**：`fn hash_files(files: &[PathBuf], base: &Path, read: &dyn Fn(&Path) -> Vec<u8>) -> String`——读盘面传 `&read_bytes`（`std::fs::read(path).unwrap_or_default()`，「读失败按空字节」的口径单点），「字节由调用方给」面传返回 `content` 字节的 reader。唯一 `Sha256::new()` 拼装仍在函数体内（全 crate 恰一处），`model_hash` / `assets_hash` 的输出字节零变化——签名是私有面，加形参不构成可见性放宽（D9 修订；上一版另立 `hash_with` 的抽取已撤回）。
9. **`edit_test.rs` 不删会让 `cargo test` 编译失败**：它 `use crate::edit::{plan_set_gate, set_gate}`，两名字离场即解析失败。故 `edit.rs` 与 `edit_test.rs` 的删除必须与 `edit/` 目录同批（AC-12 已写明），属「退役文件离场」而非测试编写。
10. **AC-1 的 grep 口径需分面**：`commands/archi/mod_test.rs` 的三个用例名以 `set_gate` 起头（`set_gate改档原子写回无残留` / `set_gate失败零写入` / `set_gate往返与跨文件与插入`），它们测的是 IPC 命令 `archi_set_gate`（命令名不变，proposal 的测试表也只要求改两处 needle）⇒ AC-1 的「`set_gate` 零命中」按**生产面**判（排除 `*_test.rs` / `mod_test.rs`），见 D16。
11. **文案族的 10 条基线不是同一种句式**：其中 `元素 {fqn} 的声明不是表形态：…` 与 `模型文件读取失败（{path}）：…` 把动态事实放在判据句**中间**，与「`message()` = 判据句：事实」的固定形态不能同时逐字成立——这是本设计唯一需要裁决的文案面冲突，落 D3。
12. **六个模型文件零 `metadata` / 零 `apis`** ⇒ 表头回填与「gate 改成本值」两条 AC 的判据面都不受内联表 / 内联数组写法影响。
13. **既有关系可以声明在非 `from` 文件**（本轮回溯的直接缺陷）：本仓六个模型实测 **129 条关系里 2 条**如此——`Desktop → Desktop.Components.Ui`（`06-frontend.toml:196`）与 `Desktop → Desktop.GeneratedBindings`（`:201`），而 `[element."Desktop"]` 声明在 `01-app.toml:10`。首版「落点 = `from` 元素声明所在文件」对这两条：upsert 会在 `01-app.toml` **追加重复声明**（原条目留在 `06-frontend.toml`），delete 会在 `01-app.toml` 报 `RelationNotFound`（真实条目在场）——与 AC-10「既有走更新 / 在场删除成功」不自洽。落点判据落 D18。
14. **重复关系声明既无诊断也不去重**：`parse/mod.rs:98` 逐条 `push`（无身份去重）、`TomlParser` 无重复关系诊断、`ModelGraph::build` 逐条建边 ⇒ 同一 `(from, to)` 在 `model.relations` 里可以有 N 份（跨文件或同文件）；`diff` 的「关系身份 = `(from, to)`」只影响计数。写面因此在**模型面**（`plan_*`）与**文件面**（`apply_*` 自身）各有一处判据（同一 `EditErrorKind`，D18）；本仓当前 129 条关系**零重复对**。现存实现的两种失真：`apply_relation_upsert` 命中第一条即 `break`（第二条留陈旧副本）、`apply_relation_delete` 用 `retain`（一次删多条）。
15. **spec delta 的哈希措辞是持有者**：`desktop-archi-drift` 的三处措辞（requirement bullet「两者都经既有私有 `hash_files`」、Module Contract 行同句、scenario「全仓只有一处累加实现（`drift::hash_files`，仍为私有）」）把实现形态写成了要求——**实现向措辞收敛**（D9 修订）比让 spec 追改便宜（零 spec 改写、零 AC-13 风险）。代价：`drift/mod_test.rs` 里引用 `hash_with` / 两形参 `hash_files` 的三条用例须随动（交接清单第 3 行）。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 目录化与文件切分 | `edit.rs` → `edit/` 目录：`mod.rs`（原语族 + `ModelWrite` + `ElementSpec` + 全部私有 helper）、`messages.rs`（`EditError` / `EditErrorKind` / `criterion` / `ALL` / `message`）、`mod_test.rs` 与 `messages_test.rs`（归测试相位）；`edit/mod.rs` 内 `mod messages;` **私有**，只 `pub use messages::{EditError, EditErrorKind};`；**两个 `#[cfg(test)] mod …_test;` 声明由测试相位在落地对应文件时同批补入**（本相位声明会因缺文件让 `cargo test` 解析失败） | 沿 `api/` `drift/` `parse/` `scan/` 的既有目录约定；`mod messages` 私有使「公开面恰 8 函数 + 4 类型」只有一个路径（AC-1），不制造 `edit::messages::EditError` 这条第二路径；测试声明后置使「每阶段收口可编译」在测试文件尚未落地时也成立（事实校准 9 的同一条纪律） |
| D2 | 错误面形态 | `EditError { pub kind: EditErrorKind, pub facts: String }`；`message()` **恒为** `format!("{}：{}", kind.criterion(), facts)`；`EditErrorKind` **19 值**封闭集（前置组 17 + 自证组 2），`ALL: [EditErrorKind; 19]` 与 `criterion()` 的穷尽 `match` 逐值相等；`EditError` **不实现** `Display` / `Error` | 单一形态才能让「变体集合 == 文案集合」成为机械对拍；`Display` 会给出第二条错误渲染路径（壳层唯一出口是 `.message()`）；构造统一走私有 `pub(super) fn error(kind, facts)`，不暴露第五个公开项 |
| D3 | 文案归宿与两处语序落位 | 19 值的 `criterion()` 全操作中立（零「改档」）；基线 10 条的归宿实测 = **逐字 4 条**（#1–#4）+ **语序落位 2 条**（#5 / #6）+ **改词 5 条**（#6–#10；#6 兼属两类，故 4 + 2 + 5 = 11，含一处重叠即 10 条）。两条语序落位因「判据句：事实」分家而改（知情变更）：`元素 {fqn} 的声明不是表形态：无法改档` → `元素声明不是表形态：{fqn}（无法写回）`；`模型文件读取失败（{path}）：{error}` → `模型文件读取失败：{path}（{error}）`。逐条对照见「AC-7 归宿表」 | `criterion()` 是 `&'static str`，**不可能**携带 `{fqn}` / `{path}`；两条基线的动态事实夹在判据句中间，与固定形态不可兼得。取舍依据：结构不变量（判据句 ↔ 事实）是 `ALL` 对拍与「判据句零改档」两条机械判据的前提，字面逐字的收益只是免一次测试随动——而既有断言本就只 needle `元素不存在` / `element` / `结构诊断` / `拒绝改档`，改词面已收缩到一处（`拒绝写回`）。回落方案见待决问题 1 |
| D4 | 三态实现形态 | 可清空位（`entry` / `doc` / `summary` / 关系 `desc`）= `Option<Option<T>>`；集合位（`path` / `metadata` / 关系 `apis`）= `Option<集合>`，`Some(空集合)` 即删键；封闭枚举位（`kind` / `gate` / `api`）= `Option<枚举>`，无清空臂（`Some(Off)` 是显式化缺省） | 提案 §四.1 逐条落地；写面 MUST NOT 产出 `apis = []` 一类被 parser 判 `invalidValue` 的形态——故「空集合」在写入侧一律翻成**删键**，绝不渲染空数组 |
| D5 | `kind` 的存在 = 新建意图 | `apply_element_spec` / `plan_element_upsert` 的判别单点：元素**不在案 + `kind` 有值** = 新建；**在案 + `kind` 有值** ⇒ `ElementAlreadyExists`；**不在案 + `kind` 缺** ⇒ `ElementKindMissing`；在案 + `kind` 缺 = 更新 | 提案未给「create 标志」参数位（否决过恒被忽略的布尔）；`kind` 是新建的唯一必填字段，语义上就是「新建声明」。改既有元素的 `kind`（层级重排）本就不在射程，fail-closed 比静默改层级安全 |
| D6 | 落点最小版 | `ElementSpec.file` 是唯一落点入参：新建**必填**且须是 `archi_graph_model_dir` **一级** `*.toml`（无分隔符、非绝对、`.toml` 结尾）；既有元素给了不同 `file` ⇒ `TargetInvalid`；模型声明目录缺席 ⇒ `TargetInvalid`（**不代建目录**）；**新建允许目标文件尚不存在**（读源按 `""` 处理）。新建臂判据次序固定：`FqnInvalid` → `ElementKindMissing` → `TargetInvalid` → `ParentMissing`（缺 `kind` 的判据句不被落点判据抢先） | 沿探索「落点最小版」；「最近已声明祖先」推演属 A。允许新文件是 `ElementSpec.file` 存在的意义（否则新建只能挤进既有文件）；`expect = Some(_)` + 文件缺席仍判不匹配（AC-4），故新文件只能以 `expect = None` 创建 |
| D7 | 键序：保序 | 既有键原位不动（含行内注释 / 缩进 / 相对序）；**更新既有键时沿用原值的 `decor`**（`decor().clone()` 拷到新值），新键追加到元素表尾；**新建**元素按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序整体写出 | 提案 §五（否决统一序：第二份字段序常量 + 推翻既有断言 + diff 最小）。decor 沿用是「写面零重排」承诺的加固（事实校准 3），代价 3 行，收益是 AC-5 不依赖源文本是否带行内注释 |
| D8 | 表头回填的实现位置 | 在**渲染之后的文本**上做：先重解析渲染文本取计数（`model.elements.len()` / `model.relations.len()`），再对**首部注释块**（从首行起连续的空行 / `#` 注释行，遇首个其它行即止）内的**第一处** `（<数字> 元素 / <数字> 关系）` 替换两个数字，然后对交付文本重解析自证 | 与提案 §六「改 AST → 渲染 → 表头回填 → 重解析自证 → 回读」的固定顺序逐字一致；不依赖 toml_edit 把首部注释放在 `document.decor` 还是首个表的 prefix（版本差异面）；注释是惰性文本，替换不动 TOML 语义，坏形态由随后的重解析兜住。计数取自重解析而非 AST 计数，落实「计数的唯一权威是重解析结果」 |
| D9 | 哈希单点（**回溯修订**：实现向 spec 措辞收敛 · `file_hash_of` 措辞归属的裁定） | `fn hash_files(files: &[PathBuf], base: &Path, read: &dyn Fn(&Path) -> Vec<u8>) -> String` 是**唯一累加核心与唯一 `Sha256::new()` 拼装点**（私有，可见性零放宽）；`pub fn file_hash(path)` = `hash_files(&[path], parent_of(path), &read_bytes)`；`pub fn file_hash_of(path, content)` = `hash_files(&[path], parent_of(path), <返回 content 字节的 reader>)`；私有 `fn read_bytes(&Path) -> Vec<u8>` 是「读失败按空字节」的单点，`model_hash` / `assets_hash` 的调用点改传 `&read_bytes`（**输出字节逐字不变**）。**撤回**上一版的 `hash_with` 抽取 | `desktop-archi-drift` 的「两者都经既有私有 `hash_files`」「全仓只有一处累加实现（`drift::hash_files`）」是**措辞持有者**——让 `hash_files` 自己吃注入的字节来源，spec 即字面成立（零 spec 改写、零 AC-13 风险），而「零第二份 `Sha256` 拼装」「`hash_files` 保持私有」两条判据一字不动。上一版把累加循环另立 `hash_with`，判据等价但让 spec 的那句话变成半真——措辞归属归 spec，实现向措辞收敛 |
| D10 | `expect` 的比对对象 | `expect` 与 `file_hash_of(&落点文件, &source)` 比对（`source` = 本次读到的**落点文件**源文本；关系面落点见 D18），不另调一次 `file_hash` 读盘 | 校验必须绑在「我实际读到的那份字节」上；再读一次盘反而制造新的 TOCTOU 窗口。`file_hash_of` 与落盘后 `file_hash` 的等价锚（AC-8）保证两种取法同值 |
| D11 | 层级 / 邻接判据不重写 | `plan_*` 用 `graph::ModelGraph::build(parse_dir(root).model)` 取 `model()` / `children()` / `depends_on()` / `depended_by()`；MUST NOT 自写 FQN 前缀扫描或关系线性过滤 | 本仓「单点构建 / 判定单源」纪律；`children` 与 `orphan_hierarchy` 的口径差异（事实校准 4 / 5）已被既有实现吸收，重写即第二份判据 |
| D12 | 删除 fail-closed 口径 | 有**直接子元素**（`ModelGraph::children` 非空）⇒ `DeleteHasChildren`（facts 点名列出的子元素）；被任一关系引用（`depends_on` ∪ `depended_by` 非空）⇒ `DeleteHasRelations`（facts 点名 `from → to` 对）；两者皆无 ⇒ 删除 | 事实校准 5；级联语义属 A（提案 §七已否决 `cascade` 参数位） |
| D13 | 自证链与回读 | `apply_*` 的固定顺序：改 AST → 渲染 → **重解析取计数（诊断非空即 `RenderSelfCheck`）** → 表头回填 → **重解析自证交付文本（诊断非空即 `RenderSelfCheck`）** → 回读逐字段（不一致即 `ReadBackMismatch`）→ `Ok(content)`；关系面回读另断言**关系条数**（既有 ⇒ 与改前相等、追加 ⇒ +1、删除 ⇒ −1，`before` 取 `ArrayOfTables` 条目数）；被自证 / 被回读的文本逐字等于交付的 `content` | 提案 §六；两次重解析的第二次才是「对交付物的自证」，第一次只为取计数并**早失败**（坏 AST 不必先回填）。值面判据全归 parser：`apis = ["A","A"]` / `[""]` / `["a::b"]`、`package` 两条 `path` 等越界值都在这一步变成 `RenderSelfCheck`——写面零第二份值域 / 形态判据；**条数断言**是 AC-10「既有走更新而非重复追加」的结构性判据（区分「更新」与「追加一条副本」） |
| D14 | 写盘仍单点 | `edit/` 零 `std::fs::write` / 零 `write_atomic` / 零 spawn / 零 Tauri；`plan_*` 只读盘（`parse_dir` + 一次 `read_to_string`），落盘仍是壳层 `commands::atomic::write_atomic` | 提案 §三 + `desktop-archi-model` 的「写盘单点」；`archi` crate 的只读边界不变 |
| D15 | 壳层接线 | `archi_set_gate_inner` = `check_root` → 组 `ElementSpec { fqn, gate: Some(gate), ..Default::default() }` → `plan_element_upsert` → `.map_err(\|error\| error.message())` → `write_atomic`；签名 / `Result<(), String>` / 失败零写入逐字不变；文件头 `:17` 的 `archi::edit::plan_set_gate` 指称改为原语族 | 提案 §三示例逐字落地；命令层仍零领域规则（不判存在性、不判结构诊断） |
| D16 | AC-1 的 grep 口径 | 生产面（`crates/**`、`src/**`，排除 `*_test.rs` / `mod_test.rs`）对 `set_gate`（排除 `archi_set_gate` / `archi_set_gate_inner`）、`plan_set_gate`、`GateWrite` **零命中**；测试面只允许 `archi_set_gate*` 相关用例名（命令名不变），`plan_set_gate` / `GateWrite` 在测试面也须零命中（needle 随动后） | 事实校准 10：`commands/archi/mod_test.rs` 的三个用例名以 `set_gate` 起头，它们测的是 IPC 命令（proposal 测试表只要求改两处 needle）——把测试用例名判红等于要求改命令名，超出本变更 |
| D17 | 版本 | `packages/desktop/package.json` `0.7.9` → `0.7.10`；`tauri.conf.json` 跟随 `"../package.json"`；`src-tauri/Cargo.toml` 版本行逐字不变 | AGENT.md 版本纪律（AC-12） |
| D18 | **关系落点与重复声明（回溯修订 · 缺陷修复）** | 判据三条：① **既有** `(from, to)`（身份键 = `from` 与 `to` 双等，与 `diff::model_diff` / `relation::relation_detail` 同口径）在 `graph.model().relations` **恰 1 处** ⇒ 落点 = **该条目的 `file`**（`root.join`），原地改 / 删——MUST NOT 用 `from` 元素声明文件；② 模型面 **0 处**（仅 upsert）⇒ 落点 = `from` 元素声明文件（新建，规则不变）；③ 模型面 **≥ 2 处**或 `apply_*` 在**给定源内 ≥ 2 处命中** ⇒ `RelationDuplicated`（19 值）fail-closed、零文本。配套两条：`apply_relation_delete` 的「一次删多条」（`retain`）退役，只删唯一命中条目；`plan_relation_delete` 在模型面 0 处即报**模型面** `RelationNotFound`（facts 列该 `from` 元素的现有出边对），MUST NOT 读 `from` 文件后让 apply 报「该文件没有关系」。`expect` 恒绑**落点文件**。判据次序：端点在案 → 重复声明 → 落点解析 → 读源 → `expect` → apply | 实测 2/129 条既有关系的声明文件 ≠ `from` 元素声明文件（事实校准 13）：按 `from` 文件落点会把更新写成重复声明、把在场删除报成 `RelationNotFound`，与 AC-10 不自洽。重复声明既无诊断也不去重（事实校准 14），任选其一都是静默裁决（删除留残余关系、更新留陈旧副本）⇒ fail-closed；检出点两层（模型面跨文件 / 文件面 apply 自身可被直接调用），但**同一 `EditErrorKind` + 同一语义定义**，MUST NOT 引第三处 |
| D19 | **壳层 6 条新增用例的归属（回溯修订）** | 归**测试相位**：`改档落盘与期望文本逐字节相等` / `计数准确时改档零字节变化` / `陈旧表头随写回修正` / `命令错误串即message形态` / `inner体只调原语族` / `文件头指称改原语族` 六条由测试相位持有并落在 `src-tauri/src/commands/archi/mod_test.rs`。design 的变更清单与 `tasks.md` **不登记测试编写**，但设计末节交接清单必须逐条登记（单一持有）；proposal 测试表该行「其余零改动」按本裁定扩写为「+ 6 条（4 条行为臂 + 2 条源码锚）」 | 每条都锚一条既有 AC / 决策（AC-2 / AC-5 / AC-7 / D15），不引入新行为；「其余零改动」是范围陈述而非验收条，扩写面已被 AC 覆盖。**回退方案**：若评审严守「其余零改动」，只保留 2 条源码锚（`inner体只调原语族` / `文件头指称改原语族`，证明 D15 的两条纪律），删 4 条行为臂（AC-2 / AC-5 由 `edit/mod_test.rs` 的 plan 面与 AC-5 的 git diff 零差异兜住） |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 写回原语族（**新**） | 4 × `apply_*`（源文本进、改写文本出；改 AST → 渲染 → 表头回填 → 重解析自证 → 回读）+ 4 × `plan_*`（结构诊断前置 → 存在性 / 端点 / 重复声明 / 落点前置 → 读源 → `expect` → `apply_*` → 填 `hash`）；**关系落点 = 既有关系的声明文件 / 新增关系的 `from` 文件**（`relation_hits` / `check_relation_unique`）；`ModelWrite` / `ElementSpec`；零写盘、零 spawn | `crates/core/archi/src/edit/mod.rs` | `toml_edit`（AST 改写）、`crate::parse`（`TomlParser` / `parse_dir`）、`crate::graph`（`ModelGraph`）、`crate::drift`（`file_hash` / `file_hash_of`）、`foundation::layout`（模型声明目录）、`super::messages` | 纯函数 + 计划函数双层（沿 `set_gate` / `plan_set_gate` 的既有形状） |
| 文案族（**新**） | `EditError` / `EditErrorKind`（19 值）/ `criterion()` / `ALL` / `message()`；判据句操作中立，事实段承载「当前事实 + 下一步」 | `crates/core/archi/src/edit/messages.rs` | 无（零 IO、零 crate 依赖） | 封闭枚举 + `ALL` 常量 + 穷尽 `match` 对拍 |
| 单文件哈希（**改**） | `file_hash`（读盘）/ `file_hash_of`（字节由调用方给），两者都字面经唯一累加核心 `hash_files`（加私有 `read` 形参）；`model_hash` / `assets_hash` 字节输出不变 | `crates/core/archi/src/drift/mod.rs` | `sha2`（既有） | 私有 `hash_files`（唯一拼装点）+ `read_bytes` 字节来源单点 |
| 出口面（**改**） | `pub mod edit;` 保留、删 `#[cfg(test)] mod edit_test;`；补 12 个 `pub use`；crate 头注释的「`gate` 写回纯函数」改为「写回原语族」 | `crates/core/archi/src/lib.rs` | — | 纯重导出 |
| 壳层写面（**改**） | `archi_set_gate_inner` 改走 `plan_element_upsert` + `EditError::message()`；命令签名 / 错误面 / 零写入语义不变 | `src-tauri/src/commands/archi/mod.rs` | `archi::edit`（原语族）、`crate::commands::atomic` | 三件事纪律（参数转换 → 调用 → 错误映射） |
| 版本（**改**） | `version` `0.7.9` → `0.7.10` | `packages/desktop/package.json` | — | — |

依赖方向零新增边：`core/archi` 仍只依赖 `foundation`（既有）；`edit/` 与 `drift/` / `graph/` / `parse/` 同 crate 内部引用；`src-tauri` → `archi` 既有边不变。零新 crate 依赖。

### 数据流（改后）

```text
apply_*（纯函数，单文件、零文件系统触碰）
  source ──toml_edit 解析──> DocumentMut
    ├─ 元素 upsert：既有键三态改写（保序 + 沿用原 decor）｜新建 [element."<FQN>"] 按规范序整表写出
    ├─ 元素 delete：remove 该元素表
    ├─ 关系 upsert：命中 (from,to) 改 desc/apis｜缺席则追加 [[relation]]（前缀 "\n"，与既有条目同形）｜同源 ≥ 2 命中 ⇒ RelationDuplicated
    └─ 关系 delete：remove 唯一命中条目（同源 ≥ 2 命中 ⇒ RelationDuplicated）
  → rendered
  → 重解析（计数权威）──── 诊断非空 ──> EditError(RenderSelfCheck)，零文本
  → 表头回填（首部注释块第一处 （N 元素 / M 关系），括号外逐字）
  → 重解析（交付文本自证）─ 诊断非空 ──> EditError(RenderSelfCheck)，零文本
  → 回读逐字段（关系面另断条数）── 不一致 ──> EditError(ReadBackMismatch)，零文本
  → Ok(content)

plan_*（读盘、零写盘）
  parse_dir(root) ──诊断非空──> EditError(ModelHasDiagnostics)
  ModelGraph::build(model) ──> 元素 / 直接子元素 / 出边 / 入边（层级与邻接单点）
  元素面前置：元素在案判别（`kind` = 新建意图）→ 新建臂 `check_create`（FQN 形态 → `kind` 必填）→ 落点（既有 = 声明文件；新建 = ElementSpec.file）→ 父元素（新建）
  关系面前置：端点在案（缺 ⇒ RelationEndpointMissing）→ 重复声明（`model.relations` 身份键 (from,to) ≥ 2 ⇒ RelationDuplicated）
             → 落点：既有关系 = 该条目的 `file`（可以不是 `from` 文件）；新增关系 = `from` 元素声明文件
             → delete 且模型面 0 处 ⇒ RelationNotFound（facts 列该 `from` 的出边对；不读 from 文件）
  读源 ──读失败──> EditError(SourceReadFailed)（新建元素且目标文件缺席 ⇒ source = ""）
  expect 校验（file_hash_of(落点文件, source)）──不匹配 / 文件缺席──> EditError(ExpectMismatch)
  apply_* ──> content（回读：逐字段 + 关系条数：既有 ⇒ 不变 / 追加 ⇒ +1 / 删除 ⇒ −1）
  hash = file_hash_of(落点文件, content)
  → Ok(ModelWrite { file, content, hash })   ← 壳层 write_atomic 落盘（唯一写点）
```

---

## 变更清单 (Change Inventory)

> **覆盖口径**：`crates/**` / `src/**` 均指 `packages/desktop/src-tauri/` 之下。本清单逐行覆盖 proposal「变更范围 - 实现文件」全量 **7 行**，并**单列一段「退役删除」**——`crates/core/archi/src/edit.rs` 与 `crates/core/archi/src/edit_test.rs` 两个文件的删除（后者出自 proposal「测试文件」表首行的括注「同批删除」；事实校准 9：它 `use crate::edit::{plan_set_gate, set_gate}`，不删即 `cargo test` 编译失败，与 `edit/` 新增必须同一批 ⇒ 属「退役文件离场」而非测试编写）。**测试编写**——proposal「测试文件」表 4 行的用例迁移 / 扩写 / needle 随动——**不进本清单、不进 `tasks.md`**（与本相位「不写测试任务」互斥），其用例名 / needle / 拆分边界由末节「交接给下游相位的随动清单」单点持有。

### 新增文件

| 文件 | 说明 | 关联子表 |
|---|---|---|
| `crates/core/archi/src/edit/mod.rs` | 原语族主体：`ModelWrite` / `ElementSpec` 两类型 + 8 个公开函数 + 全部私有 helper（`finish` / `backfill_header_counts` / `check_create` / `check_target` / `read_source` / `check_expect` / `relation_hits` / `check_relation_unique` / 元素与关系读写内部函数）；**关系落点解析（既有 = 声明文件 / 新增 = `from` 文件）与重复声明判据**；模块私有声明 `mod messages;` + `pub use messages::{EditError, EditErrorKind};`；零写盘、零 spawn | 公共函数 / API（8 行）、类型定义（2 行） |
| `crates/core/archi/src/edit/messages.rs` | `EditError` / `EditErrorKind`（19 值）/ `criterion()` / `ALL` / `message()` / 私有 `pub(super) fn error(kind, facts)`；零 IO | 公共函数 / API（3 行）、类型定义（2 行） |

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/drift/mod.rs` | ① `:580` `hash_files` 加第三形参 `read: &dyn Fn(&Path) -> Vec<u8>` 并**成为唯一累加核心**（`Sha256::new()` 拼装点仍恰一处）：`model_hash` / `assets_hash` 的调用点改传 `&read_bytes`、**输出逐字不变**；新增私有 `fn read_bytes(path: &Path) -> Vec<u8>` = `std::fs::read(path).unwrap_or_default()`（「读失败按空字节」的单点）；**不新增 `hash_with`**（上一版抽取已撤回）；② 新增 `pub fn file_hash(path: &Path) -> String`（= `hash_files(&[path.to_path_buf()], parent_of(path), &read_bytes)`）与 `pub fn file_hash_of(path: &Path, content: &str) -> String`（= `hash_files(&[path.to_path_buf()], parent_of(path), <返回 `content` 字节的 reader>)`），两者**字面经 `hash_files`**，共用私有 `fn parent_of(path: &Path) -> &Path`；③ 两函数 doc 写明「文件名参与累加 ⇒ 与 `modelHash` / `assets_hash` 数值不可比，只可与自身前后比较」「写后等价锚：`file_hash_of(file, content)` == 把 `content` 落盘后 `file_hash(file)`」；④ `model_hash` / `assets_hash` 的输出与 doc / `check` / `CheckInput` / `CheckError` / `use` 面逐字不变 | D9（修订）；`Sha256::new()` 全 crate 仍恰一处；两函数都字面经 `hash_files`（`desktop-archi-drift` 的三处措辞不追改、字面成立） |
| `crates/core/archi/src/lib.rs` | ① crate 头注释（`:1`–`:2`）「解析 / 内存图 / 结构诊断 / 入口文档 / 检索 / 结构 diff / **`gate` 写回纯函数** / …」→「…/ **写回原语族（元素与关系的 upsert / delete）** / …」；② `:26` `pub mod edit;` 保留；③ `:49` `pub use drift::{check, CheckError, CheckInput};` → 增 `file_hash, file_hash_of`；④ 新增 `pub use edit::{apply_element_delete, apply_element_spec, apply_relation_delete, apply_relation_upsert, plan_element_delete, plan_element_upsert, plan_relation_delete, plan_relation_upsert, EditError, EditErrorKind, ElementSpec, ModelWrite};`；⑤ `:66` `#[cfg(test)] mod edit_test;` **删除**（`edit.rs` 退役后无处解析）；其余出口逐字不变 | AC-1；D1 |
| `src-tauri/src/commands/archi/mod.rs` | ① `:144` 函数体改为 `let spec = ElementSpec { fqn: element.to_owned(), gate: Some(gate), ..Default::default() }; let plan = plan_element_upsert(root, &spec).map_err(\|error\| error.message())?; write_atomic(&plan.file, &plan.content)`；② `use` 面补 `archi::edit::{plan_element_upsert, ElementSpec}`；③ doc 注释（`:140`–`:142`）的「写前校验与待落盘产物（`archi::edit`）」保留，文件头 `:16`–`:19` 的 `archi::edit::plan_set_gate` 指称改为 `archi::edit::plan_element_upsert`（写回原语族）；④ `archi_set_gate` 的签名 / 属性 / `Result<(), String>` 逐字不变，其余八个命令零改动 | D15；命令层零领域规则不变 |
| `packages/desktop/package.json` | `"version"`: `0.7.9` → `0.7.10` | D17 / AC-12 |

### 删除文件（退役删除，与新建同批）

| 文件 | 说明 |
|---|---|
| `crates/core/archi/src/edit.rs` | 三件 `GateWrite` / `set_gate` / `plan_set_gate` 随之离场；能力全部由 `edit/mod.rs` 承载（gate 改档 = `ElementSpec { gate }` 的一次调用） |
| `crates/core/archi/src/edit_test.rs` | **退役删除**（非测试编写）：506 行 / 14 条 `#[test]`，`use crate::edit::{plan_set_gate, set_gate}`——两名字离场即 `cargo test` 编译失败，故必须与 `edit.rs` 删除、`edit/` 两个新增文件**同一批**（AC-12）；测例的迁移与扩写（`edit/mod_test.rs` 等）归测试相位，见「交接给下游相位的随动清单」 |

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `apply_element_spec` | `edit/mod.rs` | 新增 | `pub fn apply_element_spec(source: &str, spec: &ElementSpec) -> Result<String, EditError>` | 单文件元素 upsert：在案 + `kind` 有值 ⇒ `ElementAlreadyExists`；缺席 + `kind` 缺 ⇒ `ElementKindMissing`；缺席按规范字段序整表写出；`file` / `expect` 是本函数的**非入参旋钮**（plan 层判，apply 不读） |
| `apply_element_delete` | `edit/mod.rs` | 新增 | `pub fn apply_element_delete(source: &str, fqn: &str) -> Result<String, EditError>` | 单文件删元素表；缺 `element` 表 ⇒ `ElementTableMissing`、元素缺席 ⇒ `ElementNotFound`（facts 列该文件现有元素）；**不做**子元素 / 关系前置（跨文件，归 `plan_element_delete`） |
| `apply_relation_upsert` | `edit/mod.rs` | 新增 | `pub fn apply_relation_upsert(source: &str, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>) -> Result<String, EditError>` | 单文件 upsert：命中 `(from, to)` 走更新（条数不变），缺席追加 `[[relation]]`；`relation` 键缺席即新建 `ArrayOfTables`；**同一源内该对 ≥ 2 处 ⇒ `RelationDuplicated`**（facts「同一源内多处声明」）；回读 = `desc` / `apis` 逐字段 + **关系条数**（既有 ⇒ 不变、追加 ⇒ +1）；`apis = Some(&[])` 翻成删键（MUST NOT 渲染空数组） |
| `apply_relation_delete` | `edit/mod.rs` | 新增 | `pub fn apply_relation_delete(source: &str, from: &str, to: &str) -> Result<String, EditError>` | 命中即删该条目（**只删唯一命中**——「一次删多条」的 `retain` 退役），缺席 ⇒ `RelationNotFound`（facts 列该文件现有关系对）；**同一源内该对 ≥ 2 处 ⇒ `RelationDuplicated`**；回读 = `(from, to)` 已缺席 + 关系条数 −1 |
| `plan_element_upsert` | `edit/mod.rs` | 新增 | `pub fn plan_element_upsert(root: &Path, spec: &ElementSpec) -> Result<ModelWrite, EditError>` | 定位 + 前置 + 读源 + `expect` + `apply_element_spec` + `hash`；零写盘 |
| `plan_element_delete` | `edit/mod.rs` | 新增 | `pub fn plan_element_delete(root: &Path, fqn: &str, expect: Option<&str>) -> Result<ModelWrite, EditError>` | 同上；有直接子元素 ⇒ `DeleteHasChildren`、被任一关系引用 ⇒ `DeleteHasRelations` |
| `plan_relation_upsert` | `edit/mod.rs` | 新增 | `pub fn plan_relation_upsert(root: &Path, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>, expect: Option<&str>) -> Result<ModelWrite, EditError>` | 同上；两端点缺席 ⇒ `RelationEndpointMissing`；**落点 = 既有关系的声明文件（`model.relations` 的 `file`）/ 新增关系的 `from` 元素声明文件**；重复声明 ⇒ `RelationDuplicated`；`expect` 绑落点文件 |
| `plan_relation_delete` | `edit/mod.rs` | 新增 | `pub fn plan_relation_delete(root: &Path, from: &str, to: &str, expect: Option<&str>) -> Result<ModelWrite, EditError>` | 端点缺席 ⇒ `RelationEndpointMissing`、**模型面关系缺席 ⇒ `RelationNotFound`（facts 列该 `from` 的出边对）**、重复声明 ⇒ `RelationDuplicated`；落点 = 关系声明文件；`expect` 绑落点文件 |
| `EditError::message` | `edit/messages.rs` | 新增 | `pub fn message(&self) -> String` | 恒为 `format!("{}：{}", self.kind.criterion(), self.facts)`（壳层唯一错误映射出口） |
| `EditErrorKind::criterion` | `edit/messages.rs` | 新增 | `pub fn criterion(self) -> &'static str` | 穷尽 `match` 的 19 分支；判据句操作中立（零「改档」）、逐值非空且互异 |
| `EditErrorKind::ALL` | `edit/messages.rs` | 新增 | `pub const ALL: [EditErrorKind; 19]` | 与 `criterion()` 的 `match` 分支集合逐值相等（新增变体不补 `ALL` 即数字面不符） |
| `file_hash` | `drift/mod.rs` | 新增 | `pub fn file_hash(path: &Path) -> String` | 读盘 → `hash_files(&[path], parent_of(path), &read_bytes)`；`sha256:` + 64 位小写 hex；读失败按空字节参与（既有口径） |
| `file_hash_of` | `drift/mod.rs` | 新增 | `pub fn file_hash_of(path: &Path, content: &str) -> String` | 字节由调用方给 → `hash_files(…, <返回 `content` 字节的 reader>)`；与「落盘后 `file_hash`」逐字相等（写面乐观锁的闭环前提） |
| `archi_set_gate_inner` | `src-tauri/src/commands/archi/mod.rs` | 改（**签名不变**） | `pub fn archi_set_gate_inner(root: &Path, element: &str, gate: GateTier) -> Result<(), String>` | 体改为 `plan_element_upsert` + `EditError::message()`；错误面仍 `String`、失败零写入 |
| `set_gate` / `plan_set_gate` | `edit.rs` | **删除** | — | 退役（gate 改档 = `ElementSpec { gate }`）；全仓生产面零命中（D16） |
| `GateWrite` | `edit.rs` | **删除** | — | 由 `ModelWrite { file, content, hash }` 取代 |
| `hash_files`（私有） | `drift/mod.rs` | 改（**加 `read` 形参，升为唯一累加核心**） | `fn hash_files(files: &[PathBuf], base: &Path, read: &dyn Fn(&Path) -> Vec<u8>) -> String` | **唯一 `Sha256::new()` 拼装点**（按给定序累加「相对 base 的名字 + `0` + 字节 + `0`」）；可见性零放宽；`model_hash` / `assets_hash` / `file_hash` 传 `&read_bytes`，`file_hash_of` 传返回 `content` 字节的 reader |
| `read_bytes`（私有） | `drift/mod.rs` | 新增 | `fn read_bytes(path: &Path) -> Vec<u8>` | 「读失败按空字节」的字节来源单点（`std::fs::read(path).unwrap_or_default()`） |
| `backfill_header_counts`（私有） | `edit/mod.rs` | 新增 | `fn backfill_header_counts(rendered: &str, elements: usize, relations: usize) -> String` | 首部注释块第一处 `（<数字> 元素 / <数字> 关系）` 回填；无该括号即原样返回（MUST NOT 新增） |
| `finish`（私有） | `edit/mod.rs` | 新增 | `fn finish(document: toml_edit::DocumentMut, verify: impl FnOnce(&Model) -> Result<(), EditError>) -> Result<String, EditError>` | 自证链收口：渲染 → 重解析取计数 → 回填 → 重解析自证 → `verify` 回读（D13） |
| `check_create` / `check_target` / `read_source` / `check_expect`（私有） | `edit/mod.rs` | 新增 | `fn check_create(fqn: &str, kind: Option<ElementKind>) -> Result<(), EditError>`；`fn check_target(root: &Path, fqn: &str, file: Option<&str>, existing: Option<&Element>) -> Result<PathBuf, EditError>`；`fn read_source(file: &Path, allow_missing: bool) -> Result<String, EditError>`；`fn check_expect(file: &Path, source: &str, expect: Option<&str>) -> Result<(), EditError>` | 前置判据各一处；apply 与 plan 共用 `check_create`（同一判据一份措辞，MUST NOT 双份） |
| `relation_hits` / `check_relation_unique`（私有） | `edit/mod.rs` | 新增 | `fn relation_hits<'a>(model: &'a Model, from: &str, to: &str) -> Vec<&'a Relation>`；`fn check_relation_unique(hits: &[&Relation]) -> Result<(), EditError>` | 关系落点与重复声明的**模型面**单点：身份键 = `from` / `to` 双等（与 `diff::model_diff` / `relation::relation_detail` 同口径）；`hits.len() >= 2` ⇒ `RelationDuplicated`（facts 点名各声明文件与处数）；`hits.len() == 1` ⇒ 落点 = 该条目的 `file`；`0` ⇒ 新增落 `from` 文件 |

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `ModelWrite` | `edit/mod.rs` | 新增 | `#[derive(Debug, Clone, PartialEq, Eq)] pub struct ModelWrite { pub file: PathBuf, pub content: String, pub hash: String }`——待落盘产物三元组；`hash` 是**写后**内容哈希（可直接作下次 `expect`）；非 IPC 出线类型（不带 `specta::Type`） |
| `ElementSpec` | `edit/mod.rs` | 新增 | `#[derive(Debug, Clone, Default)]` 的写入规格：`fqn` + 五个可清空位（`entry` / `doc` / `summary` / `path` / `metadata`）+ 两个封闭枚举位（`gate` / `api`）+ 新建判别（`kind`）+ 落点（`file`）+ 乐观锁（`expect`），共 11 字段（见「数据模型」表）；`Default` 供 `..Default::default()` 简写构造 |
| `EditError` | `edit/messages.rs` | 新增 | `#[derive(Debug, Clone, PartialEq, Eq)] pub struct EditError { pub kind: EditErrorKind, pub facts: String }`；唯一渲染出口 `message()`；不出线 |
| `EditErrorKind` | `edit/messages.rs` | 新增 | `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` 的 **19 值封闭枚举**（前置组 17 + 自证组 2，逐值见「数据模型」）；`ALL` 与 `criterion()` 对拍 |
| `GateWrite` | `edit.rs` | **删除** | 由 `ModelWrite` 取代；`desktop-config-agent` delta 只追改 spec 里引用它的那一句（`ConfigWrite` 形状与实现零变化） |
| `ElementKind` / `GateTier` / `ApiStatus` | `dto.rs` | 零改动 | 四 / 三 / 二值封闭枚举，`ALL` 与 `as_str()` 逐字不变——`ElementSpec` 只**持有**它们，值域判据仍归封闭枚举 + IPC 反序列化（写面 MUST NOT 重写值域表） |
| `Model` / `Element` / `Relation` | `model.rs` | 零改动 | 写面只读这些 AST 字段做前置与回读比对，不改结构 |

<!-- 配置（Config）：本变更零配置键改动——`.dev-team/architecture/config.json` 仍只有 `exclude`（`openspec/**`）；工作区 `KNOWN_KEYS` / `config.schema.json` 镜像零改动；`desktop-config-agent` 的 delta 只是 spec 里引用退役类型名的一句追改，`crates/core/config/**` 零改动。`package.json` 的 `version` 不是配置键，已登记在「修改文件」表。 -->

---

## 数据模型 (Data Model)

**`ElementSpec` 字段（内存结构，无持久化）**：

| 字段 | 类型 | 语义 | 缺席 / 空值 | 生产者 |
|---|---|---|---|---|
| `fqn` | `String` | 目标元素完整限定名 | 空串在新建臂 ⇒ `FqnInvalid` | apply / plan |
| `kind` | `Option<ElementKind>` | **新建意图 + 新建必填**（D5）；在案元素给了即 `ElementAlreadyExists` | `None` = 更新意图 | apply / plan |
| `gate` | `Option<GateTier>` | `Some(v)` 写线值（含 `Some(Off)` 显式化缺省） | `None` = 不动（**无清空臂**） | apply |
| `api` | `Option<ApiStatus>` | 同上 | 同上 | apply |
| `entry` | `Option<Option<String>>` | `Some(None)` 删键 / `Some(Some(v))` 设值 | `None` = 不动 | apply |
| `path` | `Option<Vec<String>>` | `Some(vec![])` 删键 / `Some(v)` 写内联数组（原文不归一） | `None` = 不动 | apply |
| `doc` | `Option<Option<String>>` | 同 `entry` | `None` = 不动 | apply |
| `summary` | `Option<Option<String>>` | 同 `entry` | `None` = 不动 | apply |
| `metadata` | `Option<BTreeMap<String, String>>` | `Some(空表)` 删键 / `Some(m)` 写**内联表**（与语料同形；子表形态会新增一层表头并改变后续键的相对序，不取） | `None` = 不动 | apply |
| `file` | `Option<String>` | 新建落点（repo 相对、模型声明目录一级 `*.toml`）；既有元素只允许同值 | `None` = 用元素声明文件（新建时缺 ⇒ `TargetInvalid`） | plan |
| `expect` | `Option<String>` | **落点文件**的**当前**内容哈希（`sha256:…`；关系面落点见 D18） | `None` = 不校验（不存在「期望缺席」臂） | plan |

**`ModelWrite`**：`file`（绝对路径 = `root.join(<repo 相对模型文件>)`）/ `content`（待落盘全文，自证与回读的对象就是它）/ `hash`（`file_hash_of(file, content)`）。

**`EditErrorKind` 的 19 值**（`criterion()` 即下表第二列，逐值非空且互异、零「改档」）：

| # | 变体 | `criterion()` | `facts` 形态 | 生产者 |
|---|---|---|---|---|
| 1 | `ModelHasDiagnostics` | 模型存在结构诊断，拒绝写回 | `{N} 条（先修好模型）：{kind / kind}` | 四个 `plan_*` |
| 2 | `ElementNotFound` | 元素不存在 | `{fqn}（现有元素：a / b）`（文件面）/ `{fqn}（模型共 N 个元素）`（模型面） | `apply_element_delete` / `plan_element_delete` |
| 3 | `ElementNotTable` | 元素声明不是表形态 | `{fqn}（无法写回）` | `apply_element_spec`（更新臂） |
| 4 | `ParentMissing` | 父元素不在案 | `{fqn} 的父 {parent} 未声明——祖先须显式声明，先声明父元素` | `plan_element_upsert` |
| 5 | `RelationEndpointMissing` | 关系端点元素不存在 | `{from} → {to}：{missing} 未声明` | `plan_relation_upsert` / `plan_relation_delete` |
| 6 | `TargetInvalid` | 落点非法 | 四臂：`{fqn} 未给 file——新建必须显式给出模型声明目录一级的 *.toml` / `{file} 不在模型声明目录一级` / `{dir} 不存在（写面不代建目录）` / `{fqn} 现声明于 {actual}，不接受改落点 {given}（迁文件属独立语义）` | `plan_element_upsert` |
| 7 | `ElementKindMissing` | 新建元素缺 kind | `{fqn} 不在案——新建必须给出 kind` | `apply_element_spec` / `plan_element_upsert`（共用 `check_create`） |
| 8 | `ElementAlreadyExists` | 元素已在案 | `{fqn} 已声明于 {file}——改既有元素请省去 kind` | 同上 |
| 9 | `FqnInvalid` | 元素 FQN 形态非法 | `{fqn:?} 含空白 / 引号 / 方括号 / # / 换行（写面要把它渲染成键）` | 同上（**只作用于新建**） |
| 10 | `SourceReadFailed` | 模型文件读取失败 | `{path}（{error}）` | 四个 `plan_*` |
| 11 | `SourceParseFailed` | 模型源解析失败 | `{error}` | 四个 `apply_*` |
| 12 | `ElementTableMissing` | 模型源缺少 element 表 | `无从定位元素` | `apply_element_delete` |
| 13 | `ExpectMismatch` | 目标文件已变 | `期望 {expect}，实际 {actual}——重读后再试` / `{path} 不存在（期望 {expect}）` | 四个 `plan_*` |
| 14 | `DeleteHasChildren` | 元素仍有已声明子元素，拒绝删除 | `{fqn} 的子元素：a / b` | `plan_element_delete` |
| 15 | `DeleteHasRelations` | 元素仍被关系引用，拒绝删除 | `{fqn}：from → to / from → to` | `plan_element_delete` |
| 16 | `RelationNotFound` | 关系不存在 | `{from} → {to}（现有关系：a → b / …）`（文件面列该文件、模型面列该 `from` 的出边） | `apply_relation_delete` / `plan_relation_delete` |
| 17 | `RelationDuplicated` | 关系多处声明 | `{from} → {to} 在 {file} / {file} 多处声明（N 处）——先删除重复条目`（模型面）/ `{from} → {to} 在同一源内多处声明（N 处）——先删除重复条目`（文件面） | 四个关系函数（`plan_*` 模型面 / `apply_*` 文件面） |
| 18 | `RenderSelfCheck` | 写回渲染自证失败 | `重解析出 {N} 条诊断，未产出文本：{kind / kind}` | 四个 `apply_*` |
| 19 | `ReadBackMismatch` | 回读不一致 | `{字段}为 {actual}，目标为 {expected}` / `找不到元素 {fqn}` / `找不到关系 {from} → {to}` / `关系条数为 {actual}，目标为 {expected}` | 四个 `apply_*` |

**持久化形态（模型 TOML，本变更零数据变化）**：

| 面 | 形态 | 写面规则 |
|---|---|---|
| 元素 | `[element."<完整 FQN>"]` 单层表头 + 键值行 | 更新逐键原地改写（保序、沿用 decor）；新建按 `kind`/`gate`/`api`/`entry`/`path`/`doc`/`summary`/`metadata` 规范序整表写出；删元素 = 整表 `remove` |
| 关系 | `[[relation]]` 条目数组（`from` / `to` / `desc` / `apis`） | 命中即原地改（保序），缺席追加条目（前缀 `"\n"` 与既有条目同形）；删关系 = 条目 `remove`（只删唯一命中）；**落点 = 既有关系的 `file` / 新增关系的 `from` 元素文件；同一 `(from,to)` 多处声明 ⇒ `RelationDuplicated` 拒绝（不落任何文本）** |
| 表头 | 首部注释块（连续空行 / `#` 行）第一处 `（<数字> 元素 / <数字> 关系）` | 括号内两个数字按**该文件重解析结果**回填；括号与括号外逐字保留；无该括号不新增；首部之外同形文本零改动 |

---

## 路由 / API 设计 (Route / API Design)

<!-- 不涉及：本变更零新增 / 删除 IPC 命令，零 HTTP 端点，零 DTO 字段变化。九条 `archi_*` 命令的入参、出参、错误面（`Result<T, String>`）与 auth 概念（无）逐字不变；`archi_set_gate` 仅内部调用面从 `plan_set_gate` 换成 `plan_element_upsert`，其 IPC 签名 `(root, element, gate) -> Result<(), String>`、失败零写入与「唯一写命令之一」的地位逐条不变；`EditError` / `ElementSpec` / `ModelWrite` 不带 `specta::Type`、不出线，`bindings.ts` / `bindings.zod.ts` 零 diff。 -->

---

## 依赖 (Dependencies)

### 运行时依赖

零新增。`crates/core/archi` 既有依赖即够用：`toml_edit`（`0.25`，AST 改写与渲染）、`sha2`（`0.10`，既有累加）、`toml` + `serde`（重解析自证经 `TomlParser`）、`foundation`（模型声明目录）、`petgraph`（`ModelGraph` 既有权重）。`src-tauri` 零新增（`archi` 既有边）。前端零新包。

### 构建 / 测试依赖

零新增。复用既有脚本：`pnpm bindings:export`（预期零 diff）、`pnpm api:export` / `pnpm api:check`（预期零 diff）、`pnpm check`、`pnpm test`。无新语料、无新金样、无新工具。

---

## 验收标准回应

| AC | 设计落点 |
|---|---|
| AC-1 | D1 + D16 + 「公共函数 / API」表：公开面 = 8 函数（签名与提案表逐字一致）+ 4 类型（`ElementSpec` / `ModelWrite` / `EditError` / `EditErrorKind`）；`mod messages` 私有 ⇒ 无第五路径；生产面 grep 对 `set_gate`（排除 `archi_set_gate*`）/ `plan_set_gate` / `GateWrite` 零命中，测试面只留 `archi_set_gate*` 用例名（事实校准 10） |
| AC-2 | D15 + D7（decor 沿用）+ D8（回填在自证之前）：临时模型无表头 ⇒ 回填 no-op ⇒ 按行 diff 恰 1 行；`archi_set_gate` 的签名 / 错误面 / 失败零写入逐条不变（壳层 `mod_test.rs` 的 `set_gate改档原子写回无残留` / `set_gate失败零写入` / `set_gate往返与跨文件与插入` 三条既有断言零改动即成立，事实校准 3） |
| AC-3 | D4 + 数据模型表：六个可清空位（`entry` / `doc` / `summary` / `path` / `metadata` / 关系 `apis`）三态逐字段；三个封闭枚举位无清空臂；`Some(空集合)` 一律翻成**删键**，写面零空数组形态（反向判据由 `RenderSelfCheck` 兜住任何越界值） |
| AC-4 | D10 + 「数据模型」`expect` 行：`expect` 比对 `file_hash_of(落点文件, source)`；不匹配 / 文件缺席 ⇒ `ExpectMismatch` 且 `plan_*` 返回 `Err`（`Err` 臂不携带任何 `content`）；`None` 不校验；两条写面共用同一 `plan_*` 入口，无绕过分支；**关系面落点文件 = 既有关系的声明文件 / 新增关系的 `from` 文件（D18），`expect` 随落点走** |
| AC-5 | D8 + 事实校准 3 / 11：括号内 == 该文件重解析计数（计数取自渲染文本的重解析）；括号外逐字（只替换两段数字）；无括号不新增；首部之外同形文本零改动（扫描在首个非注释行即止）；本仓六个模型计数本就准确 ⇒ 「gate 改成本值」零字节差异 |
| AC-6 | D13 + D11：结构诊断非空 ⇒ 四个 `plan_*` 全 `Err`；渲染后重解析出诊断 ⇒ `RenderSelfCheck` 零文本；回读不一致（逐字段 + **关系条数**）⇒ `ReadBackMismatch` 零文本；顺序固定「改 AST → 渲染 → 回填 → 重解析 → 回读」，回填计数取自重解析、「回填后计数 == 回读计数」成为结构性事实而非事后核对 |
| AC-7 | D2 + D3 + 数据模型 **19 值**表 + 下方「AC-7 归宿表」：`ALL` ↔ `criterion()` 逐值相等（穷尽 `match`）、逐值非空且互异、零「改档」；`message()` 恒为「判据句：事实」；基线 10 条归宿实测 = 逐字 4 条 + 语序落位 2 条 + 改词 5 条（#6 兼属两类，4 + 2 + 5 = 11 含一处重叠 = 10 条；与 proposal AC-7「5 改词 / 5 逐字」的差异即两条语序落位，见待决问题 1）；既有断言仅 `拒绝改档` → `拒绝写回` 一处随动 |
| AC-8 | D9（修订）+ `file_hash` / `file_hash_of` 的 doc：`sha256:` + 64 位小写 hex；同盘稳定；内容变即变；文件名参与累加（同名跨目录相等 / 异名不等）；`file_hash_of(file, content)` == 落盘后 `file_hash(file)`（单一累加核心 `hash_files` 保证）；两者都**字面经 `hash_files`**（`desktop-archi-drift` 的措辞不追改）；`ModelWrite.hash` 即该值；`edit/` 零 `Sha256` / `sha2` 字面 |
| AC-9 | D12 + D6：直接子元素 ⇒ `DeleteHasChildren`（点名 FQN）；出 / 入边非空 ⇒ `DeleteHasRelations`（点名关系对）；叶元素 ⇒ 删除成功、`remove` 只带走该元素表、其余逐字保留、表头计数随动 |
| AC-10 | D5 + D6 + D11 + **D18（回溯修订）**：新关系落 `from` 元素所在文件；**既有 `(from, to)` 经 `model.relations` 的 `file` 定位并原地改 / 删——其声明文件可以不是 `from` 元素文件（本仓 `Desktop → Desktop.Components.Ui` 即此形，事实校准 13）**；重复声明（模型面 / 文件面 ≥ 2 处）⇒ `RelationDuplicated` fail-closed；`desc` / `apis` 三态 + 回读断言**关系条数**（既有 ⇒ 不变、追加 ⇒ +1、删除 ⇒ −1）；`apis` 越界值由 `RenderSelfCheck` 拒绝（写面零值面判据）；端点缺席 ⇒ `RelationEndpointMissing`；模型面关系缺席 ⇒ `RelationNotFound`（不再「读 `from` 文件后由 apply 报该文件无此关系」） |
| AC-11 | 事实校准 1 + 「不含」：零 DTO / 零命令 / 零生成物语义变化 ⇒ `bindings:export` 与 `api:export` 零 diff、`api:check` 零差异退出；`crates/core/archi/tests/**` 零改动 ⇒ 语料与金样逐字节不变；报告 55 / 129 / 0 / 0 / 39 / 53 / `clean` / `modelHash` 逐字不变（`edit/` 改动不触判定面） |
| AC-12 | D17 + 修改文件表：`0.7.10`（+1 patch，回溯修订**不再二次 bump**）；`tauri.conf.json` 仍 `"../package.json"`；`src-tauri/Cargo.toml` 逐字不变；改动集 ⊆ 7 个实现文件 + `edit_test.rs` 的退役删除（见「删除文件」表）+ 本 change 文稿（`edit.rs` / `edit_test.rs` 两次删除须与 `edit/` 两个新增文件同批）；**D18 / D9 修订只改这 7 个实现文件中的 3 个（`edit/mod.rs` / `edit/messages.rs` / `drift/mod.rs`），零新增路径** |
| AC-13 | 由 spec 面承接（本设计不改 delta）：`specs/` 恰三个能力目录；RENAMED 的 `FROM:` 与基线标题、`TO:` 与随后的 MODIFIED 标题逐字一致；各 MODIFIED 块为基线整段拷贝后改；每条 `### Requirement` 至少一个 `#### Scenario:`；`## Module Contract` 段已按受影响模块列 Function / API / CLI 表 |

**AC-7 归宿表（基线 10 条逐条核）**：

| # | 基线文案 | 归宿 | 落位后 |
|---|---|---|---|
| 1 | `模型源解析失败：{error}` | 11 `SourceParseFailed` | **逐字不变** |
| 2 | `模型源缺少 element 表：无从定位元素` | 12 `ElementTableMissing` | **逐字不变** |
| 3 | `元素不存在：{fqn}（现有元素：…）` | 2 `ElementNotFound`（文件面 facts） | **逐字不变** |
| 4 | `元素不存在：{fqn}（模型共 N 个元素）` | 2 `ElementNotFound`（模型面 facts） | **逐字不变** |
| 5 | `模型文件读取失败（{path}）：{error}` | 10 `SourceReadFailed` | **语序落位**：`模型文件读取失败：{path}（{error}）`（判据句前置，见 D3 / 待决问题 1） |
| 6 | `元素 {fqn} 的声明不是表形态：无法改档` | 3 `ElementNotTable` | **语序落位 + 改词**：`元素声明不是表形态：{fqn}（无法写回）` |
| 7 | `改档渲染自证失败（重解析出 N 条诊断），未产出文本` | 18 `RenderSelfCheck` | **改词**：`写回渲染自证失败：重解析出 N 条诊断，未产出文本` |
| 8 | `改档渲染自证失败：回读档位为 X，目标为 Y` | 19 `ReadBackMismatch` | **改词**：`回读不一致：档位为 X，目标为 Y` |
| 9 | `改档渲染自证失败：回读找不到元素 {fqn}` | 19 `ReadBackMismatch`（第二 facts 臂） | **改词**：`回读不一致：找不到元素 {fqn}` |
| 10 | `模型存在 N 条结构诊断，拒绝改档（先修好模型）：…` | 1 `ModelHasDiagnostics` | **改词**：`模型存在结构诊断，拒绝写回：N 条（先修好模型）：…` |

> 合计：**逐字 4 条**（#1–#4）+ **语序落位 2 条**（#5 / #6）+ **改词 5 条**（#6–#10）= 10 条（#6 兼属两类，4 + 2 + 5 = 11 含一处重叠）。2 条语序落位（#5 / #6）的成因是 `criterion()` 恒为 `&'static str`，无法承载 `{path}` / `{fqn}`；改词 5 条的全部词面（`无法写回` / `写回渲染自证失败` / `回读不一致` / `拒绝写回`）与 proposal 逐字一致。**新增的 `RelationDuplicated`（#17）无基线文案来源**——它是本轮回溯新增的判据（D18），不进「基线 10 条」；`RenderSelfCheck` / `ReadBackMismatch` 的编号因此由 17 / 18 顺延为 18 / 19（判据句与 facts 不变）。

---

## 交接给下游相位的随动清单（非本相位改动）

> 本节承接 proposal「测试文件」表的 4 行中**测试编写**部分（用例迁移 / 扩写 / needle 随动），**不进「变更清单」也不进 `tasks.md`**：进变更清单即要求 `tasks.md` 有对应任务，而测试任务不属本相位。表首行 `edit_test.rs` 的**删除**是退役动作、非测试编写，已单列进「变更清单·删除文件」与 `tasks.md` 2.5，不在本节重复登记。本表只登记**必改机械面**——用例名 / needle / 拆分边界，供测试相位逐条落地。

| 文件 | 随动面（用例名 / needle / 拆分边界） |
|---|---|
| `crates/core/archi/src/edit/mod_test.rs`（由 `edit_test.rs` **迁移**，同批删除 `edit_test.rs`；并在 `edit/mod.rs` 末尾补 `#[cfg(test)] mod mod_test;` 声明） | ① 14 条既有用例改走 `plan_*`（排版保留 / 无 `gate` 行插入 / 三档往返 / 500 元素规模 / CRLF / 注入式 FQN / entry 保留两条 逐条保留）；② **两条语义迁移**：`元素缺席返回err并附现有元素清单` 改挂 `plan_element_delete` / `apply_element_delete`（upsert 里缺席是**新建分支**，判据句为「新建元素缺 kind」）；`坏源与无element表均err` 的「无 `element` 表」臂改挂 `apply_element_delete`（`ElementTableMissing`），空源 + upsert 走「新建缺 kind」；③ 新增：三态逐字段（`entry` / `doc` / `summary` / `path` / `metadata` / `gate` / `api`）、`expect` 匹配 / 不匹配 / 文件缺席、表头回填五例（括号内 == 重解析计数、括号外逐字、**回填后正文不截断**（首部注释块之后的 `[element."…"]` / `[[relation]]` 逐字保留，且交付文本重解析出的元素 / 关系集合与改前同——只核括号内计数会漏掉「表全被截断仍零诊断」这一形态）、无括号不新增、非首部同形文本不动）、删除 fail-closed 三例、关系 upsert / delete 三态与落点、新建元素（规范字段序 + `file` 必填 + 父缺席拒绝 + FQN 形态七拒）、`ModelWrite.hash` == 落盘后 `file_hash(file)`、结构错模型四拒、临时目录零写盘；④ 现有 needle 面：`元素不存在` / `element` / `结构诊断` 保留；`拒绝改档` → `拒绝写回`（唯一改词断言）；⑤ **回溯修订新增（D18）**：a) **既有关系声明于非 `from` 文件**——`from` 元素声明在 A 文件、`(from,to)` 声明在 B 文件时，upsert 改 `desc` ⇒ `plan.file == B`、`A` 文件零变化、回读条数不变；delete ⇒ 删净且 `A` 零变化、计数随动（现用名 `关系删除落from声明文件` 的语义翻转，改名 / 改断言）；b) **重复声明 fail-closed**——模型面两文件各声明一次 ⇒ `plan_relation_upsert` / `plan_relation_delete` 均 `RelationDuplicated`（facts 点名两文件）；同一源内两处 ⇒ `apply_relation_upsert` / `apply_relation_delete` 均 `RelationDuplicated`（facts「同一源内」），且 `apply_relation_delete` 不再一次删净两条；c) **回读条数断言**——既有关系 upsert 后条数不变、新关系 +1、删除 −1 |
| `crates/core/archi/src/edit/messages_test.rs`（新；同批在 `edit/mod.rs` 补 `#[cfg(test)] mod messages_test;` 声明） | `ALL` 与 `criterion()` 的穷尽 `match` 对拍（新增变体即编译期发现）、逐值非空且互异、`message()` 形态「判据句：事实」、判据句零「改档」字面（**19 值**逐个遍历——D18 新增 `RelationDuplicated`） |
| `crates/core/archi/src/drift/mod_test.rs` | 新增四条：形态（`sha256:` + 64 hex）与同盘稳定、内容变即变、文件名参与累加（同名同内容跨目录相等 / 异名同内容不等）、`file_hash_of` 与落盘后 `file_hash` 等价（外加「与 `assets_hash` / `modelHash` 只共享前缀、数值不判等」）；既有 `assets_hash` / `model_hash` 五条用例逐字不动；**D9 修订随动**：`hash_files是file_hash的口径单点` 改调 `super::hash_files(&[path], parent, &super::read_bytes)`（三形参）、`注入reader与空清单` 改挂 `super::hash_files`（`hash_with` 已退役；test-design 的「`hash_with()`（私有）」项同批退役）、`唯一sha256拼装点` 的注释改「唯一累加核心 = `hash_files`」；「零独立拼装」仍按 `Sha256::new()` 在 `drift/mod.rs` 恰 1 处 + `edit/` 内零 `Sha256` / `sha2` 字面判 |
| `src-tauri/src/commands/archi/mod_test.rs` | ① `archi_file_tree与唯一写命令的边界`（`:1611` 起）的负断言 needle `plan_set_gate` → `plan_element_upsert`（MUST NOT 留一条对已删名字的恒真空断言）；② `archi_set_gate` 相关文案断言按新判据句核对（`拒绝改档` 一处 → `拒绝写回`）；③ **6 条新增用例归测试相位（D19 授权，proposal 该行「其余零改动」按此扩写）**：`改档落盘与期望文本逐字节相等`（AC-2）/ `计数准确时改档零字节变化`（AC-5 全链）/ `陈旧表头随写回修正`（AC-5 / R-2）/ `命令错误串即message形态`（AC-7 壳层出口）/ `inner体只调原语族`（D15 源码锚）/ `文件头指称改原语族`（D15 源码锚）——**归属 = 测试相位**，不进 design 变更清单 / tasks；④ 其余零改动（三个 `set_gate*` 用例名保留——命令名不变，D16） |

> **`test-design.md` 面的随动（同一交接、单一持有）**：本轮修订另有两处测试设计面须随动——① `EditErrorKind::ALL.len() == 18` / 「18 值」逐处改为 **19**（并补 `RelationDuplicated` 的模型面 / 文件面两臂 facts）；② 私有项 `hash_with()` **退役**（`注入reader与空清单` 用例改挂 `hash_files` 的三形参形态，`hash_files是file_hash的口径单点` 的调用同步改三形参）。两处归测试相位（本文不改测试设计），登记于此以免第二份持有。

---

## 待决问题 (Open Questions)

1. **AC-7 的「逐字保留」与「判据句：事实」形态在 2 条文案上不可兼得**（#5 `模型文件读取失败（{path}）：{error}`、#6 `元素 {fqn} 的声明不是表形态：无法改档`）。本设计取固定形态、两条语序落位（D3）。**回落方案**：若评审要求字节逐字，则 `message()` 改为「每变体一份模板、判据句只在句子开头出现」，`messages_test` 的形态断言相应放宽为「`message()` 以 `criterion()` 起始且含 `：`」——代价是「判据句：事实」不再是一条可机械判定的结构不变量，且 #6 的判据句必须以 `元素` 起头（`criterion()` 无法带 `{fqn}`）。本设计不取。
2. **新建元素时目标文件尚不存在是否合法**（D6 定为合法：源按 `""` 处理、`expect` 必须为 `None`）。若评审要求「新建只能追加进既有文件」，回落在 `check_target` 里把「目录下无该文件名」判为 `TargetInvalid`（一处判据、一处文案，其余零变化）——但那样 `ElementSpec.file` 的「新建落点」语义只剩「选一个既有文件」，A 的工具面无法为新域开新文件。
3. **`ElementAlreadyExists` 与 `TargetInvalid` 的判别顺序**（D5 定：`kind` 有值即新建意图，先判在案）。若 A 的工具面希望「给 `kind` 就顺带改 `kind`」，需另立语义（层级重排 / 跨文件迁移）——本变更 fail-closed，MUST NOT 静默改层级。
4. **重复关系声明的清理归属**（D18 只 fail-closed 拒绝写回，不提供清理动作）：清理属读面既有反馈通道（`archi_drift` 的 `structuralErrors` 不诊断关系重复，故只能由写面 facts 点名）+ A 的工具面；若评审要求写面顺带「去重后再写」，须另立变更（一次写改多处 = 多条目语义，与本变更「单条目单写」互斥）。本仓当前零重复对，判据只为 fail-closed 兜底。
5. **spec delta 的「关系落点」bullet 是否补写既有关系定位句**：`desktop-archi-model` 的「关系落点」bullet 现为「新关系 SHALL 落 `from` 元素声明所在文件；…既有 `(from, to)` 走更新（改 `desc` / `apis`）而非重复追加」——**未写既有关系的定位方式**（本轮回溯的缺陷根因）。本设计把定位判据放在 D18（「既有走更新而非重复追加」的唯一可实现解），故 spec 字面不需改；若评审要求要求面完整持有该句，回落在该 bullet 补一句「既有 `(from,to)` SHALL 经 `model.relations` 的 `file` 定位并原地改 / 删；同一 `(from,to)` 多处声明即 `Err`」——属 spec 相位写面，本相位不改。
