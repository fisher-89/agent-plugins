# 测试设计: archi-scaffold-field-dictionary

> **变更**: archi-scaffold-field-dictionary（`archi_scaffold` 升级为字段词典 · P2）
> **来源**: `design.md`（**新增文件 0**；修改文件 5 行——`parse/toml.rs`（常量）/ `parse/mod.rs`（再导出）/ `sdk/archi_tool.rs`（模板三块五行）/ `sdk/mount.rs`（描述面三处）/ `packages/desktop/package.json`（版本行）；**删除文件 0**）、`proposal.md`（AC-1…AC-9）、2 份 delta（`specs/desktop-archi-agent` MODIFIED ×1 / `specs/desktop-archi-model` ADDED ×1）。
> **被测面口径**: 只覆盖 design「新增文件」+「修改文件」子表（**删除文件不纳入**，本变更无删除文件）。design「测试文件」子表的 3 行是**承接面不是被测面**——其用例已挂靠对应源文件章节（`toml_test.rs` ↔ `parse/toml.rs` 的常量、`archi_tool_test.rs` ↔ `sdk/archi_tool.rs` 的词典正文、`mount_test.rs` ↔ `sdk/mount.rs` 的描述面）；版本行（`package.json`）与生成物 / 读数记入 `## 不可测试项`。
> **测试框架（实测盘点）**: Rust 侧为语言内建 `#[test]` + `assert!` / `assert_eq!`（**无第三方断言库**），测试文件与源文件同级、命名 `<模块>_test.rs`，经父模块 `#[cfg(test)] mod <模块>_test;` 注册；`crates/core/archi/src/parse/toml_test.rs` 为纯 `&str` 面（零落盘），另有 `include_str!` 源码自证范式（同文件 `解析doc白名单源码自证`、`crates/core/archi/src/model_test.rs` 的 `element字段数恰十一个且两个指针各一处`）；`crates/infra/agent/src/sdk/archi_tool_test.rs` 用 `tempfile::TempDir` 合成盘面 + `#[tokio::test]` 驱动 `ArchiContext::execute`，`mount_test.rs` 为纯定义面 `#[test]`（`lookup(name)` 取挂载表）。本 change 不引新测试依赖。
> **设计约束对齐**: `#### 待测功能` 条目逐条取 design「公共函数 / API」表中「所在文件」为该源文件的行，**不 grep 源文件导出声明虚构条目**；跨模块组合用例按链路发起方 / 最上层调用方挂靠到入口模块的 per-file 章节（词典链路挂 `sdk/archi_tool.rs`，常量 ↔ 视图链路挂 `parse/toml.rs`），不新建独立集成测试章节。
> **反向可判优先级**: 本变更的三条机械锚（常量 ↔ 字段面 / 词典 ↔ 常量 / 词典 ↔ 值域）都以「手工删一行即红」为验收核心（AC-2 / AC-3 / AC-4），故用例判据一律按**行首 / 块取**设计，`contains(字段名)` 形态在用例 doc 中作为反例点名（R-3）。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 三处值面缺口逐字补齐：`entry` 行含 `单值`（限定 `# entry：` 行）；`metadata` 块含 `值 MUST 是字符串` / `非字符串值被拒` / `子表与内联表等价`；`apis` 块含 `只写名字` / `可留空` / `逐项非空` / `同名不重复` / `显式空数组被拒` / `生成物面留空` / `archi_drift` 与整句（反引号逐字）`` `apis = []` 与省略同义 ``；`invalidValue` 零命中 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`（`SCAFFOLD_TEMPLATE` 三块五行）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（`archi_scaffold模板值面纪律四条` / `archi_scaffold模板值域逐值且零诊断词`） |
| AC-2 | 字段面全集锚不空转：草稿逐字段存在行首形态（`{f} = ` / `# {f}：` / `# {f} = `），断言遍历 `archi::parse::ELEMENT_FIELD_NAMES`、测试内零手抄八字面量；反向可判——删 `# api：` 即红（`apis` 不顶替 `api`）、删 `kind = ` 实值行即红 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（`archi_scaffold草稿含字段面全集`）、`packages/desktop/src-tauri/crates/core/archi/src/parse/mod.rs`（再导出消费路径） |
| AC-3 | 常量与字段面同批：`ELEMENT_FIELD_NAMES` 恰八项、顺序 = `RawElement` 声明序（`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）；源码自证取 `struct RawElement { … }` 块内字段名并同序同集比较，`RawElement` 增删改名而常量未同批即红 | `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs`、`packages/desktop/src-tauri/crates/core/archi/src/parse/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs`（`字段面清单与RawElement同序同集`） |
| AC-4 | 值域派生：草稿逐值含 `GateTier::ALL` 三值与 `ApiStatus::ALL` 二值，缺省档（`off` / `declared`）以「缺省」字样在场且由 `Default` 派生；`ElementKind::ALL` 四值语义既有断言保持绿；新增任一变体未补词典即红 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（`archi_scaffold模板值域逐值且零诊断词`；词表源 `crates/core/archi/src/dto.rs` 的 `ALL` / `Default`，本变更零改动） |
| AC-5 | 既有锚全绿：三 kind 草稿经真实 `TomlParser` 零结构诊断 / 恰一元素 / `path` 未声明；草稿零结构诊断词；零写盘；关系块全注释且四键齐备；响应经 `tools::cap_result` 单点未截断、体量 ≪ `MAX_RESULT_BYTES` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`（执行体零改动）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（既有九支逐字保留 + 体量 / 未截断断言） |
| AC-6 | 描述面改措辞且不再枚举字段名：含 `字段词典` / `逐字段一行纪律` / `值域` / `关系四键` / `kind 省略取 domain` / `只读零写盘` / `write / edit`；零 `字段序` / `内联 metadata` / `path 必填提示` / `关系模板`（正反两向都断言） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`（`archi_scaffold_definition()` 的 description）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`（`archi_scaffold定义描述要素`） |
| AC-7 | 零读数与零生成物变化：模型 55 元素 / 129 关系、`status = clean` / `passed = true`、结构诊断 0 逐字不变；`api:check` 零差异退出；`bindings:export` 后 `git diff` 为零；`scan.*` 三项计数与基线相同 | 生成物 / 读数面（`INDEX.toml` / `.schema.json` / `.i.rs` / `.i.ts` / `types/generated/**` / 报告读数）见 `## 不可测试项`；代码侧背书 = `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs`（常量无 serde / specta）、`packages/desktop/src-tauri/crates/core/archi/src/parse/mod.rs`（不进 `lib.rs` 根出口） |
| AC-8 | 版本纪律：`package.json` 的 `version` 相对 `HEAD` 恰 +1 patch（0.7.9 → 0.7.10）；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 版本行逐字不变 | `packages/desktop/package.json`（见 `## 不可测试项`） |
| AC-9 | spec delta 形态：恰 2 份 delta（MODIFIED ×1 / ADDED ×1）；`### Requirement` 标题与基线逐字一致；每条 Requirement 至少一个 `#### Scenario:`；两份各带 `## Module Contract` | `openspec/changes/archi-scaffold-field-dictionary/specs/{desktop-archi-agent,desktop-archi-model}/**`（见 `## 不可测试项`） |

> **AC-4 的派生源不在本变更改动集内**: `GateTier::ALL` / `ApiStatus::ALL` / `ElementKind::ALL` 与三处 `Default` 实现都在 `crates/core/archi/src/dto.rs`，本变更零改动。用例以「遍历真实 `ALL` + `default()`」的方式消费它们，故将来新增变体时本变更的三支断言即红，无需另测 `dto.rs`。
>
> **AC-5 的既有九支不重写**: `archi_tool_test.rs` 的九支 scaffold 用例（省略 kind / 四 kind 逐值 / kind 行四值语义 / path 行出路 / 零结构诊断词 / 草稿零结构诊断 / 关系块全注释 / 零写盘 / 非法 kind 报错）逐字保留，其断言即 AC-5 的主体；本变更只新增三支，故 `## 验收范围` 的 AC-5 行指向既有用例名而不复制其用例行。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs

#### 待测功能

- `ELEMENT_FIELD_NAMES（公共常量）: 新增 pub const [&str; 8]，顺序 = RawElement 声明序（kind / gate / api / entry / path / doc / summary / metadata）；零 serde / 零 specta；权威仍是字段定义，两者同批维护`

<!-- 出处：design「公共函数 / API」表 `archi::parse::ELEMENT_FIELD_NAMES` 行（所在文件 = 本文件定义 / `parse/mod.rs` 再导出）。契约由 D2 / D3 与 AC-3 承载。用例主体落在新增用例 `字段面清单与RawElement同序同集`（`include_str!("toml.rs")` 源码自证），既有 parse 用例零改动。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ELEMENT_FIELD_NAMES` 源码自证 | 正向 | 同序同集：真实 `include_str!("toml.rs")` 定位首个 `struct RawElement {` 到首个 `\n}` 收尾，块内滤掉 `///` 与 `#[` 行、只留以 `,` 收尾的行、取首个 `:` 前名字 → 与常量 `assert_eq!` 逐位相等 | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 正向 | 常量恰八项：`ELEMENT_FIELD_NAMES.len() == 8`（`[&str; 8]` 的类型元数为编译期锚，运行期断言与源码块计数互证） | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 正向 | 项值形状：逐项非空、无重复、无空白字符、无 `::`（逐项经常量遍历，测试内零手抄八字面量） | 表驱动（8 行） |
| `ELEMENT_FIELD_NAMES` 源码自证 | 边界 | 定位锚唯一：`include_str!("toml.rs").matches("struct RawElement {").count() == 1`——新常量的 doc MUST NOT 含该字面，否则定位提前命中（事实校准 8 的机械锚） | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 边界 | 过滤规则：构造块混入 `///` doc 行、`#[serde(default)]` 属性行、空行 → 三者均不入名字清单（只留 `,` 收尾行） | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 边界 | 单元素边界：构造 `struct RawElement {` + 单字段 + `}` → 名字清单恰一项（外层容器单元素 × 内层枚举类型） | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 边界 | 空容器边界：构造无字段的块 → 名字清单 `[]`，双向差集点名「全部缺失」、零 panic（外层容器空 × 内层零元素） | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 边界 | 超大容器边界：构造 200 字段的块（混入 `///` 行）→ 名字清单 200 项、差集点名多余项、零 panic | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 异常 | `RawElement` 多一字段而常量未同批：构造块含额外字段 → 双向差集的「缺失」侧点名该字段 | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 异常 | `RawElement` 少一字段而常量未同批：构造块去掉 `metadata` → 双向差集的「多余」侧点名 `metadata` | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 异常 | 同集不同序：构造块把 `gate` 与 `api` 互换位置 → 同序 `assert_eq!` 红，消息逐位点名首个错位下标（只比集合不比序的实现被判红） | 单例 |
| `ELEMENT_FIELD_NAMES` 源码自证 | 异常 | 定位锚缺失：构造不含 `struct RawElement {` 的文本 → `expect` 文案点名定位串、零静默返回空清单 | 单例 |
| `ELEMENT_FIELD_NAMES` 字段面零改动（回归） | 正向 | 八字段类型逐项在位（`kind: ElementKind` / `gate: GateTier` / `api: ApiStatus` / `entry: Option<String>` / `path: Vec<…>` / `doc: Option<String>` / `summary: Option<String>` / `metadata: BTreeMap<String, String>`）——常量不改任何字段类型 | 表驱动（8 行） |
| `ELEMENT_FIELD_NAMES` 字段面零改动（回归） | 正向 | 既有全字段 / 缺省 / 白名单自证用例逐字保持绿（`解析全字段源_元素关系逐字段相等且缺省取零值` / `解析entry键缺省与显式逐字保留` / `解析doc单值逐字进模型` / `解析doc白名单源码自证`） | 表驱动（回归 3 行） |
| `ELEMENT_FIELD_NAMES` 字段面零改动（回归） | 异常 | 既有值面违例用例诊断条数与 `kind` 逐字不变（`解析非法api值归invalid_value并列出合法值` / `解析apis显式空数组归invalid_value` / `metadata非字符串值被拒`）——常量不参与解析 | 表驱动（3 行） |
| `ELEMENT_FIELD_NAMES` 字段面零改动（回归） | 边界 | 空源 / 仅注释源 / 500 元素规模源三支用例逐字保持绿（常量无解析期副作用） | 表驱动（3 行） |
| `ELEMENT_FIELD_NAMES` 再导出面 | 组合 | 经 `crate::parse::ELEMENT_FIELD_NAMES`（而非 `super::toml::`）取常量并遍历出八项，且与 `super::toml::ELEMENT_FIELD_NAMES` 逐项相等（顺带证 `parse/mod.rs` 再导出是同一常量而非第二份）——入口模块 = 常量定义处 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 只吃 `&str`——真实 `toml` crate 的 `Spanned` / 原文案即被测契约；`include_str!("toml.rs")` 在编译期嵌入真实源文件（不是替身） | 全部用例 |
| 无（零落盘） | 源码自证不读盘、不建临时目录；判据 helper（块定位 / 行过滤 / 双向差集）是测试内纯函数，不导出、不进产品面 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/parse/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/mod_test.rs

#### 待测功能

- `ELEMENT_FIELD_NAMES（再导出）: pub use toml::{TomlParser, ELEMENT_FIELD_NAMES}（fmt 实序：rustfmt version-sort 把 CamelCase 排在 SHOUTY 前）——出口一行、零第二份定义、不进 lib.rs 根出口`

<!-- 出处：design「公共函数 / API」表 `archi::parse::ELEMENT_FIELD_NAMES` 行（所在文件含「`parse/mod.rs`（再导出）」）。契约由 D2 与 AC-3 / AC-2 的消费路径承载。 -->

> **本章不新增 `mod_test.rs` 用例**: tasks.md 阶段四的改动集白名单只含三份测试文件（`toml_test.rs` / `archi_tool_test.rs` / `mount_test.rs`），`mod_test.rs` 不在其中；再导出面的机械判据按 design D3 ⑤ 落在**消费侧**——`toml_test.rs` 的 `crate::parse::` 路径与 `archi_tool_test.rs` 的 `archi::parse::` 路径（编译期即证）。下表即该判据的归属登记。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ELEMENT_FIELD_NAMES` 再导出面 | 正向 | crate 内路径：`crate::parse::ELEMENT_FIELD_NAMES` 可 `use` 并遍历出八项、与 `super::toml::ELEMENT_FIELD_NAMES` 逐项相等（同一常量，非第二份定义）——执行宿主 = `toml_test.rs::字段面清单与RawElement同序同集` 的消费行 | 单例 |
| `ELEMENT_FIELD_NAMES` 再导出面 | 正向 | 跨 crate 路径：`archi::parse::ELEMENT_FIELD_NAMES` 在 `infra/agent` 侧可 `use` 并遍历出八项——执行宿主 = `archi_tool_test.rs::archi_scaffold草稿含字段面全集` 的 import 行 | 单例 |
| `ELEMENT_FIELD_NAMES` 再导出面 | 边界 | 既有出口面零破坏：`TomlParser` 与常量同批再导出后，`mod_test.rs` 既有的 `use super::{parse_dir, SourceParser, TomlParser}` 与全部聚合用例逐字不改、保持绿（`pub mod toml;` / `SourceParser` / `parse_dir` / `relative_path` 零变化） | 表驱动（回归） |
| `ELEMENT_FIELD_NAMES` 再导出面 | 异常 | 缺失即编译失败：若 `pub use` 行被回退（常量不再经 `parse` 出口）→ 上述两处消费 `use` 编译失败（红在编译期而非运行期空转）；用例 doc 点名「本文件无运行期断言，判据以编译期为准」 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 再导出面无可替身对象；判据是真实 `use` 语句的编译结果 + 常量逐项相等 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs

#### 待测功能

- `archi_scaffold(kind?: "package" | "domain" | "module" | "composite") -> { "draft": string }（执行体）: 草稿即字段词典——八字段逐字段一行纪律（`entry` 单值 / `metadata` 值域 / `apis` 四条值面）+ 关系四键模板；响应形状、`kind` 词表、`cap_result` 单点与 `scaffold_draft` / `scaffold_payload` 零改动`

<!-- 出处：design「公共函数 / API」表 `archi_scaffold（工具面）` 行（定义面 `mount.rs` / 执行体本文件）。契约由 D4 / D5 / D6 / D8 / D9 与 AC-1 / AC-2 / AC-4 / AC-5 承载。新增三支用例名按 design 末节交接清单第 2 行：`archi_scaffold草稿含字段面全集` / `archi_scaffold模板值域逐值且零诊断词` / `archi_scaffold模板值面纪律四条`；既有九支逐字保留。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_scaffold` 草稿·字段全集锚 | 正向 | 三形态并集：遍历 `archi::parse::ELEMENT_FIELD_NAMES`（测试内零手抄八字面量），逐 `f` 断言草稿含 `{f} = ` / `# {f}：` / `# {f} = ` 任一（delta 字面判据，零成本保留） | 表驱动（8 行） |
| `archi_scaffold` 草稿·字段全集锚 | 正向 | 纪律行行首：同遍历，逐 `f` 断言草稿存在以 `# {f}：` **开头**的行（`starts_with`，禁用 `contains`）——AC-2 第一反向锚 | 表驱动（8 行） |
| `archi_scaffold` 草稿·字段全集锚 | 正向 | 唯一实值行：草稿内未注释赋值行（非 `#` 起始且含 ` = `）恰一条且键 ∈ `ELEMENT_FIELD_NAMES`（实际 = `kind`）——三形态单判会漏掉「删整行」形态，此行是 AC-2 第二反向锚 | 单例 |
| `archi_scaffold` 草稿·字段全集锚 | 正向 | 四 kind 恒成立：`ElementKind::ALL` 四值各出草稿，以上三元判据逐 kind 均成立（词典正文与 `kind` 值无关，`__KIND__` 单点替换不动纪律行） | 表驱动（4 行） |
| `archi_scaffold` 草稿·字段全集锚 | 边界 | 载体数不唯一：`path` 有 3 个行首载体（`# path：` + 两条 `# path = `）、`kind` 有 2 个（实值行 + `# kind：`）、其余六字段各 2 个——判据按「存在」而非「恰一」成立，零假红（事实校准 2） | 单例 |
| `archi_scaffold` 草稿·字段全集锚 | 异常 | 假命中反例：用例 doc 点名「`apis` 不得顶替 `api`」；对草稿副本删掉 `# api：` 行后，纪律行行首判据为假（`# apis：` 在场不顶替） | 单例 |
| `archi_scaffold` 草稿·字段全集锚 | 异常 | 实值行缺失反例：对草稿副本删掉 `kind = "__KIND__"` 行后，唯一实值行判据为假（0 条 ≠ 1） | 单例 |
| `archi_scaffold` 草稿·值域派生 | 正向 | gate 逐值：行首定位 `# gate：` 行后**行内**逐值断言 `GateTier::ALL` 的 `as_str()`（三值全在场） | 表驱动（3 行） |
| `archi_scaffold` 草稿·值域派生 | 正向 | api 逐值：行首定位 `# api：` 行后行内逐值断言 `ApiStatus::ALL` 的 `as_str()`（二值全在场） | 表驱动（2 行） |
| `archi_scaffold` 草稿·值域派生 | 边界 | 缺省档派生（两行）：`# gate：` 行含 `缺省 {GateTier::default().as_str()}`、`# api：` 行含 `缺省 {ApiStatus::default().as_str()}`——缺省值从 `Default` 派生，零手抄 `off` / `declared` | 表驱动（2 行） |
| `archi_scaffold` 草稿·值域派生 | 边界 | `kind` 四值语义既有断言保持绿（`archi_scaffold模板kind行四值语义` 逐值语义词经 `ElementKind::ALL` 派生）——本变更不动该行 | 表驱动（4 行） |
| `archi_scaffold` 草稿·值域派生 | 异常 | 行内限定防假命中：`declared` 亦出现在 `# doc：` 行——对「仅 `# doc：` 行含 `declared`」的构造片段，`# api：` 行判据为假（全串 `contains` 会假命中） | 单例 |
| `archi_scaffold` 草稿·值面纪律 | 正向 | `entry` 单值：`# entry：` 行内 `contains("单值")`（`Option<String>` 的单值语义在场） | 单例 |
| `archi_scaffold` 草稿·值面纪律 | 正向 | `metadata` 块：从 `# metadata：` 行起到**下一空行**止的块内逐 needle——`值 MUST 是字符串` / `非字符串值被拒` / `子表与内联表等价`（后者已移续行，单行断法必假） | 表驱动（3 行） |
| `archi_scaffold` 草稿·值面纪律 | 正向 | `apis` 块：从 `# apis：` 行起到下一空行止的块内逐 needle——`只写名字` / `可留空` / `逐项非空` / `同名不重复` / `显式空数组被拒` / `生成物面留空` / `archi_drift` / 整句 `` `apis = []` 与省略同义 ``（反引号逐字） | 表驱动（8 行） |
| `archi_scaffold` 草稿·值面纪律 | 边界 | 块取法扩张免疫：`metadata` / `apis` 由 1 行扩为 2 行后续行 needle 仍命中（用例 doc 点名：改回单行 `contains` 断法时「子表与内联表等价」必假） | 单例 |
| `archi_scaffold` 草稿·值面纪律 | 边界 | 末行块边界：`apis` 块位于草稿尾部（无后继空行）时块取到文末、零 panic、零截断（块取 helper 的右开边界） | 单例 |
| `archi_scaffold` 草稿·值面纪律 | 异常 | `entry` 行内限定反例：「单值」仅落在 `# doc：` 行时 `# entry：` 行判据为假（`# doc：单值指针` 不顶替 `entry`，R-3 反例） | 单例 |
| `archi_scaffold` 草稿·值面纪律 | 异常 | 零结构诊断词：草稿四词 `invalidValue` / `unknownField` / `missingField` / `syntaxError` 零命中（既有锚逐字保留；新文本只写行为「非字符串值被拒」，R-1 / D9） | 表驱动（4 行） |
| `archi_scaffold` 草稿·可解析与只读 | 正向 | 三 kind 草稿各落盘经真实 `TomlParser`（`archi::dto::module_overview`）：零结构诊断、恰一元素、`path` 未声明（既有用例逐字保留，AC-5） | 表驱动（3 行） |
| `archi_scaffold` 草稿·可解析与只读 | 边界 | 关系块形态：零裸 `[[relation]]` 表；`# from = ` / `# to = ` / `# desc = ` / `# apis = ` 四键以注释在场；`apis` 既有四 needle（`只写名字` / `可留空` / `生成物面留空` / `archi_drift`）仍在 | 单例 |
| `archi_scaffold` 草稿·可解析与只读 | 边界 | 零写盘：调用前后盘面字节快照逐一相同（既有 `snapshot()` helper，既有用例逐字保留） | 单例 |
| `archi_scaffold` 草稿·可解析与只读 | 边界 | 多字节字符：草稿含 `→` / `≥` / `——`（字符数 < 字节数）时行首匹配、块取、`lines()` 计数按字节安全、零 panic、行数不因多字节失真 | 单例 |
| `archi_scaffold` 草稿·可解析与只读 | 异常 | 解析判据非空转：给 `package` 草稿副本解注释两条 `path` → 真实解析恰一条 `invalidValue`（只看「`unwrap` 成功」不数诊断的实现会漏判模型停摆） | 单例 |
| `archi_scaffold` 响应与体量 | 正向 | 响应单键（`{ draft }`，零第二键）；`kind` 省略 → `kind = "domain"` 实值行；四 kind 逐值 → 实值行逐值对应且零 `__KIND__` 残留（既有两支用例逐字保留） | 表驱动（5 行） |
| `archi_scaffold` 响应与体量 | 边界 | `cap_result` 未截断：`execute` 返回文本零「已截断」且 `text.len() < tools::MAX_RESULT_BYTES`（实测响应 2293 B ≪ 50 000 B；截断即纪律行被切 → 词典静默残废） | 单例 |
| `archi_scaffold` 响应与体量 | 边界 | 草稿体量：草稿字节数 < 8 000 且 `lines().count()` ≤ 45（落地读数 2213 B / 39 行，留余量；超限即红——R-5 的体量护栏） | 单例 |
| `archi_scaffold` 响应与体量 | 异常 | `kind` 非法：`packagex` / `Domain` → `Err` 文案点名四值；`""` / `null` / `42` → 视省略取 `domain`，零静默退化（既有用例逐字保留） | 表驱动（5 行） |
| `archi_scaffold` 词典链路（模板 → 草稿 → 响应） | 正向 | 链路正例：省略 `kind` 单次调用即可从响应取到完整词典正文（一次读全，零二次点查）——`{ draft }` 内八字段纪律行与关系四键逐项在场 | 单例 |
| `archi_scaffold` 词典链路（模板 → 草稿 → 响应） | 异常 | 链路异常：`kind` 值集外时链路在 `scaffold_payload` 处短路——不产草稿、不经 `cap_result` 序列化，`Err` 文案点名四值（词典正文不因错误入参降级为空草稿） | 表驱动（2 行） |
| `archi_scaffold` 词典链路（模板 → 草稿 → 响应） | 组合 | 模板 → `scaffold_draft` → `scaffold_payload` → `serde_json::to_string` → `cap_result`：全链路输出对 `ELEMENT_FIELD_NAMES` 八项逐项行首对齐（常量 ↔ 视图锚的端到端，入口模块 = 本文件的词典正文） | 单例 |
| `archi_scaffold` 词典链路（模板 → 草稿 → 响应） | 组合 | 样例可直接抄改：把草稿的 `# gate = "off"` 解注释并按 `GateTier::ALL` 逐值改写，落盘经真实 `TomlParser` → 零诊断、`gate` 逐值入模型（词典给的值域样例可直用） | 表驱动（3 行） |
| `archi_scaffold` 词典链路（模板 → 草稿 → 响应） | 组合 | `apis` 纪律与判据同向：`# apis = ["ExportedName"]` 解注释并置于有效关系内 → 零诊断；`apis = []` → 恰一条 `invalidValue`（词典「显式空数组被拒」与 `validate_apis` 判据同向，非空口承诺） | 表驱动（2 行） |
| `archi_scaffold` 词典链路（模板 → 草稿 → 响应） | 边界 | 转义增长率边界：草稿经 `serde_json` 转义后长度仍 ≪ `MAX_RESULT_BYTES`（最坏情形按 ~2× 增长估算仍安全，词典扩容不触上限） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `tempfile::TempDir` 合成盘面（既有 `healthy()` / `tempdir()` helper）+ 真实 `ArchiContext::execute` + 真实 `toml` 解析（落盘后经 `archi::dto::module_overview`）；模板是常量字面串，替身会抹掉被断言的文本本身 | 全部用例 |
| 无（零 mock） | 词表一律从真实常量派生（`ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` + `default()`），不在测试内手抄值或字段名；`archi::parse::ELEMENT_FIELD_NAMES` 为唯一遍历源 | 全部用例 |
| 无（进程边界零接触） | 不涉子进程 / 网络 / DB / 配置 / 环境变量；盘面即 `TempDir`，`snapshot()` 是真实字节快照（既有 helper） | 零写盘 / 落盘解析用例 |
| 判据 helper（测试内纯函数，非 mock） | 行首定位、块取（`# {f}：` 到下一空行）、未注释赋值行枚举、双向差集——只在测试文件内定义，MUST NOT 导出成产品面 symbol（AGENT.md「No test-only exports」） | 字段全集 / 值域 / 值面纪律用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

#### 待测功能

- `archi_scaffold（工具定义面）: description 改词典措辞（`字段词典` / 逐字段一行纪律与值域 / 关系四键模板），零字段名枚举；`kind` enum 仍由 `ElementKind::ALL` 派生、`required: []`、零新参数`

<!-- 出处：design「公共函数 / API」表 `archi_scaffold（工具面）` 行（定义面本文件 / 执行体 `archi_tool.rs`）。契约由 D7 与 AC-6 承载。`ARCHI_SCAFFOLD_TOOL` 常量 doc 与 `archi_scaffold_definition()` fn doc 的措辞无运行时读面，见 `## 不可测试项`。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_scaffold` description（词典措辞） | 正向 | 正 needle：`lookup(ARCHI_SCAFFOLD_TOOL)` 的 `description` 逐项在场——`字段词典` / `逐字段一行纪律` / `值域` / `关系四键` / `kind 省略取 domain` / `只读零写盘` / `write / edit` | 表驱动（7 行） |
| `archi_scaffold` description（词典措辞） | 边界 | 单行散文形态：非空、零换行、零代码围栏（三反引号）、零裸 TOML 行（`[element.`）——`concat!` 三行拼接后仍是单行（工具清单里的可读形态） | 单例 |
| `archi_scaffold` description（词典措辞） | 边界 | 既有保留句：描述仍含 `草稿可直接抄改` 与 `archi_drift` 自查两句（去枚举只删字段名清单，不删行为指引） | 单例 |
| `archi_scaffold` description（词典措辞） | 异常 | 旧标签反 needle：同 `description` 逐项**零命中**——`字段序` / `内联 metadata` / `path 必填提示` / `关系模板`（`关系四键模板` 不含连续子串 `关系模板`；反向断言与正向同批落地，R-4） | 表驱动（4 行） |
| `archi_scaffold` 参数面 | 正向 | `kind` 词表派生：`enum` 逐值等于 `ElementKind::ALL.map(ElementKind::as_str)`，`required == []`，`name == "archi_scaffold"`（既有 `archi_scaffold定义kind词表派生` 逐字保留） | 表驱动（回归） |
| `archi_scaffold` 参数面 | 边界 | `kind` 的属性 `description` 含 `元素 kind 四值之一（省略取 domain）`，与实际 `enum` 长度（4）一致（描述与词表不脱耦） | 单例 |
| `archi_scaffold` 参数面 | 异常 | 零新参数：参数面属性键集恰 `["kind"]`——零 `mode` / 零 `field` / 零 `root`（「一个工具一个形状」与「按字段查询」两条不做项在定义面上可判） | 单例 |
| `archi_scaffold` 定义面同源 | 正向 | 描述单源：`registry()` 内恰一项 `definition.name == ARCHI_SCAFFOLD_TOOL`，且该项 `definition.description` 与 `archi_scaffold_definition()` 直调产物逐字相等（零「清单里存一份、执行体另写一份」） | 单例 |
| `archi_scaffold` 定义面同源 | 边界 | 描述同源边界：`ToolTier::Read` 档位恒放行（三档不拦），且名字在挂载清单内命中一次——档位与清单面均不与描述脱耦 | 单例 |
| `archi_scaffold` 定义面同源 | 异常 | 描述与行为错位即红：省略 `kind` 必须回 `domain`、显式 `kind = "module"` 必须回 `module`——两态对照下若省略态返回别的值，则描述句 `kind 省略取 domain` 与执行体错位 | 表驱动（2 行） |
| `archi_scaffold` 定义面同源 | 组合 | 描述三条既有句与执行行为同源：`kind` 省略 → `ArchiContext::execute` 返回 `kind = "domain"` 草稿；调用前后盘面字节快照相同（只读零写盘）；写面不经本工具 | 表驱动（3 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `lookup(name)` 取真实挂载表与真实 `archi_scaffold_definition()` 产物；`description` 是 `concat!` 字面串，替身会抹掉被断言的文本 | 描述 / 参数面用例 |
| 无（零 mock） | 链路行复用真实 `ArchiContext` + `tempfile::TempDir` 盘面 + `snapshot()` 字节快照（不桩掉执行体） | 链路用例 |

---

## 不可测试项

| 模块 | 原因 | 替代保证 |
|---|---|---|
| 1 · `packages/desktop/package.json`（`version` 0.7.9 → 0.7.10） | 一行字面量，无函数 / 分支 / 输出可断言 | AC-8 由桌面端 `git diff` 核对：`package.json` 恰 +1 patch；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 版本行逐字不变 |
| 2 · `sdk/mount.rs` 的 `ARCHI_SCAFFOLD_TOOL` 常量 doc（`:59`–`:61`）与 `archi_scaffold_definition()` fn doc 首段（`:343`–`:344`） | Rust doc comment 无运行时读面（agent 看到的只有 `description`，已由 `mount_test` 章节断言）；为 doc 加断言只能复制一份文本作第二事实源 | 桌面端 static-check grep：`字段词典` 在 `mount.rs` 内 3 处（常量 doc / fn doc / description）、`逐字段一行纪律与值域` 3 处；`字段序` / `path 必填提示` / `关系模板` 零命中（`关系四键模板` 不含连续子串）；fn doc 第二段（`kind` enum 派生 + `TomlParser` 零诊断自证锚）逐字保留由人工对读 |
| 3 · AC-7 的仓级读数与生成物零 diff（`.dev-team/architecture/graph/INDEX.toml`、`models/**/*.schema.json`、`.i.rs` / `.i.ts`、`packages/desktop/src/types/generated/**`、模型 55 元素 / 129 关系、`status = clean` / `passed = true`、结构诊断 0、`scan.*` 422 / 359 / 422 / 0 / 0） | 仓级状态、生成步骤与报告读数是数据面产物，不是可执行单元；`archi` crate 的测试全用合成 fixture，不读仓内模型 | 桌面端 `pnpm api:export` 后 `git status --porcelain -- .dev-team/architecture` 零输出、`api:check` 零差异退出、`bindings:export` 后 `git diff -- packages/desktop/src/types/generated` 零输出、`archi_drift` 读数复核；代码侧背书 = 常量是 `[&str; 8]` 直构（零 serde / 零 specta），`INDEX.toml` 两元素 `status = absent`（`types = 0` / `ops = 0`） |
| 4 · AC-9 的 spec delta 形态（恰 2 份：`desktop-archi-agent` MODIFIED ×1 / `desktop-archi-model` ADDED ×1；`### Requirement` 标题与基线逐字一致；每条 Requirement ≥1 `#### Scenario:`（四个 `#`）且 WHEN/THEN 成对；两份各带 `## Module Contract`） | spec 是散文资产，不是可执行单元；本相位不改写 delta | 桌面端逐文件核对标题清单与 Scenario 计数（本设计不复制该清单，避免第二份事实源） |
| 5 · `parse/mod.rs` 出口面「零第二处」与 `lib.rs` 根出口零扩散（AC-3 的出口侧 / AC-7 的「不进根 `pub use`」） | 「某符号不存在 / 恰一处」在 Rust 无运行期反射面；加断言需动 `mod_test.rs`，超出本变更改动集白名单（tasks.md 阶段四只列三份测试文件） | 桌面端 static-check：`grep -c 'ELEMENT_FIELD_NAMES' parse/mod.rs` = 1、`grep -rn 'ELEMENT_FIELD_NAMES' crates/core/archi/src/lib.rs` 零命中；再导出可用性由 `parse/toml.rs` 章节的 `crate::parse::` 行与 `sdk/archi_tool.rs` 章节的 `archi::parse::` 行在编译期背书 |
| 6 · 全仓（除测试）旧标签零命中（`字段序` / `内联 metadata` / `path 必填提示` / `关系模板`）与描述修改的零扩散 | 跨文件语义扫描不是可执行单元；把「读全仓源码」写进单测会越过单元测试边界，且会把其它 `*_test.rs` 的防线单元拖进判定面 | 桌面端 static-check：`grep -rn --include=*.rs` 后 `grep -v '_test.rs'` 零命中；描述面自身的正反断言在 `mount_test` 章节（不依赖 grep） |
| 7 · `crates/infra/agent/src/archi-knowledge.md` + `archi_knowledge_test.rs` + `crates/core/workflow/src/prompts/{proposal-planner,dev-design-planner}.md`（design「不要修改」表） | 本变更零改动；知识骨架与相位 prompt 是散文资产，其 `骨架_指路表逐句在场` 的 `("字段序", "archi_scaffold", "取模板")` 行仍准确（模板仍给字段序与可抄改草稿） | 桌面端 test-execution 全量跑兜底（既有断言保持绿）+ `git status --porcelain` 对这两处零 diff 核对 |
| 8 · `RawElement` / `Element` / `Relation` 字段面、`DiagnosticKind` 九值、`ArchiApiFindingKind` 十三值、`REPORT_VERSION` 的零变化 | 本变更零字段 / 零词面增删；「零变化」不产新用例（新用例只能逐字重复既有断言，属第二份事实源） | 既有 `element字段数恰十一个且两个指针各一处`、`漂移词汇五值`、`REPORT_VERSION` 断言在 test-execution 全量跑中兜底；`ELEMENT_FIELD_NAMES` 在 doc 与用例里被当作**副本**消费（权威仍是字段定义） |
| 9 · 私有件 `scaffold_draft(kind: archi::dto::ElementKind) -> String` / `scaffold_payload(&self, &Value) -> Result<Value, String>` | 私有函数不进公共面；为其加 `pub` 或 test-only 导出只服务测试（AGENT.md「No test-only exports」），且会扩大 `sdk/mod.rs` 的可见面 | 经 `archi_scaffold` 公共行为面全覆盖（草稿正文 / 响应形状 / `kind` 词表 / 零写盘 / 体量 / 链路三行）；两个函数本变更零改动（design D1 / AC-5） |
| 10 · AC-5 的体量精确读数（模板正文 2213 B / 39 行、响应 2293 B） | 逐字节硬断言会把「补一句纪律」判红，与 R-5「超限时压缩同类表述」冲突；精确读数属桌面端账目 | 单测取区间判据（草稿 < 8 000 B 且 ≤ 45 行）+ `cap_result` 未截断断言（`sdk/archi_tool.rs` 章节）；精确读数由桌面端 `wc -c` / `archi_drift` 账目核对 |
