# desktop-archi-drift Specification（delta: archi-file-tree）

## MODIFIED Requirements

### Requirement: 文件归属与两套解析器一套归属逻辑

检查 SHALL 把每个扫描文件归属到架构元素，判据 SHALL 为**元素 `path` 的最长前缀匹配**（`path` 可指目录、crate 目录、子目录或单文件，目录以 `/` 结尾；语言无关）。归属规则：

- 命中唯一最长前缀即归属该元素；同一最长长度命中多个元素 SHALL 判为归属不确定——文件计入 `scan.unmatchedFiles[]` 并记 `ambiguousOwner` warning，MUST NOT 任选其一。
- 只被 `gate = off` 元素覆盖的文件 SHALL 计入 `scan.unmatchedFiles[]`（不产生任何违例；off 的语义是「本模块不参与判定」）。
- 无元素认领的文件 SHALL 计入 `scan.unmatchedFiles[]`；元素未声明 `path` 时 SHALL 记 `noPath` warning 且不参与归属。
- 归属结果 SHALL 按元素聚合出线到 `scan.matched[]`（`{ element, files[] }`），供门禁诊断与视图复用。
- **生成物归属按双形态并集判据**：`.i.<ext>` 生成物（`ext ∈ {rs, ts, tsx, js, jsx}`）SHALL 由 `OwnerIndex` 的「整组剥离 `<name>.i.<ext> → <name>` ∪ 替换扩展名 `<stem>.i.<ext> → <stem>.<ext>`」并集判据归属（判据定义与断言面见 `desktop-archi-api`「生成物落点与入口查找」）——生成器产出的是**元素根同级**形态，既有前缀判据对它的两条分支（全等 / 目录前缀）都不成立，故并集是「生成物能归其元素」的唯一保证。归属判据 MUST NOT 在漂移侧另写一份：`scan` 的两处 `owner_of` 调用点（源文件归属与 `target_rel` 目标归属）自动获得同一判据。
  - **对报告的语义影响**：`gate != off` 的已声明元素，其生成物 SHALL 由 `scan.unmatchedFiles[]` 移入 `scan.matched[]`（这是判定面的一次语义修正：生成物本就属于该元素）；`gate = off` 元素下的生成物仍落 `unmatchedFiles[]`（与改动前同分支，报告零变化）。本仓 `gate` 全 `off` ⇒ 既有报告逐字段、逐序不变。
  - **目标侧同等受益**：生成物被 import 时 SHALL 可作为合法 `to` 归属（本仓生成物零 `use` 语句、无人 import，故同样零变化）。

依赖提取 SHALL 由**两套解析器**承担、归属判定**只一套**：

- **TS / JS**：静态 `import` 声明（含 `import type`）与 `export … from` re-export；模块解析 SHALL 走 tsconfig / jsconfig 的 `baseUrl` + `paths`（自被解析文件向上取最近配置）与 node 解析规则，产出磁盘路径；解析失败、动态 `import()` / `require()`、宏式无法静态判定者 SHALL 记入 `scan.unresolved[]`（`{ file, specifier, message }`），**MUST NOT 冒充违例**。
- **Rust**：先建 crate 清单（解析 workspace 清单与各 member 的 `Cargo.toml`，得 crate 名与 `src/lib.rs` / `src/main.rs` 位置——MUST NOT 经 `cargo metadata` 一类子进程取数），再逐 crate 建**模块树**（`mod x;` 外部文件、`mod x { … }` 内联、`#[path = "…"]` 优先覆盖默认推导、`cfg` 条件编译两臂都进树且同模块路径不报重），最后解析 `use` 树（`crate::` / `self::` / `super::` 取当前文件模块路径上溯，其它 crate 名查 workspace crate 表）并做**模块路径最长前缀匹配**落到文件。宏生成的 `mod`、`include!`、无 `mod` 声明的孤儿文件 SHALL 记 `scan.unresolved[]`；外部 crate（`std` / 第三方）SHALL 不进归属（无元素覆盖，不算违例）。
- **不做的面**：模块树只用于路径前缀匹配，MUST NOT 做类型检查、MUST NOT 展开宏、MUST NOT 做完整 name resolution；`pub use` re-export 只算「声明了依赖」，MUST NOT 穿透追源。
- **同元素内不报**：同一元素内部文件之间的 import SHALL NOT 产生违例（跨元素边界才是判定对象）。

#### Scenario: 同一元素内 import 与跨元素 import 分辨

- **WHEN** `src/billing/x.ts` 既 import 同元素内 `src/billing/y.ts`，又 import `src/auth/token.ts`（模型无 `App.Billing → App.Auth` 声明）
- **THEN** 前者零记录；后者记一条 `undeclaredDependency`（`from = App.Billing`、`to = App.Auth`）

#### Scenario: TS 别名与 tsconfig paths 解析

- **WHEN** 以 tsconfig `paths` 声明 `@/* → src/*`，某文件写 `import { x } from '@/auth/token'`
- **THEN** specifier 解析到磁盘路径并归属到其元素（不因别名记 unresolved）

#### Scenario: Rust 模块树四种形态

- **WHEN** 分别以 `src/lib.rs` 的 `mod foo;` + `src/foo.rs`、`src/foo/mod.rs` + `src/foo/bar.rs`、`#[path = "custom/x.rs"] mod x;`、`#[cfg(unix)] mod a;` + `#[cfg(windows)] mod a;` 构造 crate
- **THEN** 模块路径分别落到 `src/foo.rs`、`src/foo/bar.rs`、`src/custom/x.rs`、两臂同模块路径且不报重复；对应 `use` 均可解析到文件

#### Scenario: 无法静态判定的形态只留痕不冒充违例

- **WHEN** 扫描遇到宏生成的 `mod`、`include!`、无 `mod` 声明的孤儿 `.rs`，或动态 `import()` / `require()`
- **THEN** 逐条记入 `scan.unresolved[]`（含 file 与 message），`errors` 为空

#### Scenario: 外部 crate 不进归属

- **WHEN** 某文件写 `use serde::Deserialize`
- **THEN** 该 `use` 不产生任何归属、不进 `errors` / `warnings` / `scan.unresolved[]`

#### Scenario: 归属不确定不任选

- **WHEN** 两个不同元素各声明同一长度的 `path` 前缀同时覆盖某文件
- **THEN** 该文件计入 `scan.unmatchedFiles[]` 且记 `ambiguousOwner` warning，`errors` 不因它产生

#### Scenario: 生成物按并集判据归元素并进入 matched

- **WHEN** 元素 `App.X` 声明 `path = ["./src/x/"]` 且 `gate = "warn"`，盘上存在元素根同级生成物 `src/x.i.ts`
- **THEN** 该文件归属 `App.X` 并出现在 `scan.matched[]` 的 `App.X` 条目下，MUST NOT 出现在 `scan.unmatchedFiles[]`；同场景把 `gate` 改为 `off` 后该文件回到 `unmatchedFiles[]`（与改动前同分支），报告其余字段不变

#### Scenario: gate 全 off 时归属判据改动不移动报告字段

- **WHEN** 在 `gate` 全为 `off` 的真仓上，**保持排除配置不变**（本变更同时新增的 `exclude` 条目不在本对照内，避免两个变量混在一条断言里）分别以并集判据改动前 / 改动后执行全量检查
- **THEN** 两份报告的 `unmatchedFiles[]` 集合与序、`matched[]`、`errors` / `warnings` / `excluded[]` 逐字段相同——唯一生成物 `crates/infra/archi.i.rs` 所属元素 `gate = off`，`Owner::None` 与 `Owner::Element{off}` 同落 `unmatchedFiles[]`

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/scan/owner.rs` | 归属单点（判据修正） | `owner_of` 对生成物候选集（整组剥离 ∪ 替换扩展名）各跑一遍最长前缀并合并定档；`normalize` / `prefix_matches` / `OwnerIndex::build` 语义零变化；同长多命中仍 `Ambiguous` |
| `crates/core/archi/src/scan/mod.rs` | 扫描编排（零判据改动） | 两处 `owner_of` 调用点（源文件归属、`target_rel` 目标归属）自动获得并集判据；`gate = off` 分支语义逐字不变 |
| `crates/core/archi/src/scan/owner_test.rs` | 判据断言面（修正） | 根同级正例新增、`src/dir.i.ts` 期望翻转、并集定档用例、白名单外后缀反例保持（详见 `desktop-archi-api` Module Contract） |
| `.dev-team/architecture/config.json`（新增，见 `repo-archi-model`） | 扫描排除配置 | `{ "exclude": ["openspec/**"] }`（不带 `$schema`）；生效后 `openspec/` 下源文件由 `unmatchedFiles[]` 移入 `scan.excluded[]`，判定规则零变化 |
