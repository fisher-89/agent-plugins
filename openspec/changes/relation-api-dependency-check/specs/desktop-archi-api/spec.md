# desktop-archi-api Specification (Delta)

## MODIFIED Requirements

### Requirement: 生成物落点与入口查找

生成物 SHALL 落**实现入口文件同级**（`<入口主干>.i.rs` / `<入口主干>.i.ts`），其中 `<入口主干>` SHALL 为入口文件名的**去扩展名形态**（`crates/infra/archi/src/lib.rs` → `lib`、`views/welcome-view.tsx` → `welcome-view`、`packages/desktop/src-tauri/src/lib.rs` → `lib`）。入口 SHALL 由本节的三态解析链给出，生成物路径因此是**入口的单值函数**；元素根路径（`path.first()`）MUST NOT 再参与落点派生（它只回答「根在哪」）。

- **`<root>` 归一在模型解析层完成**：`<root>` SHALL 为 `path.first()` 经 `normalize_root` 归一后的形态（`OwnerIndex::normalize` 已处理 `./` 前缀与尾 `/`），MUST NOT 批量改写存量 models——实测存量 46 条 `path` 全部带 `./` 前缀（目录型根另带尾 `/`）、一律需要归一（文件型根的扩展名是「根即入口」的判据，故归一 MUST NOT 去扩展名）。
- **入口解析链封闭三态，MUST NOT 增分支**（候选派生**零 IO**，只做字符串运算）：

  ```
  entry_candidates(element) -> Vec<String>      纯文本、按优先序
    ① 显式 entry（归一后）                      → [entry]                          唯一候选
    ② 文件型根（末段扩展名 ∈ {rs, ts, tsx, js, jsx}）→ [<root>]                   根文件自身即模块
    ③ 目录型根（语言无关，按序取首个落盘者）
       → [<root>/src/lib.rs, <root>/mod.rs, <root>/index.ts, <root>/index.tsx]
    ④ 其余（`path` 空 / `<root>` 形态非法）      → []

  entry_state(root, elements, element) -> EntryState        读盘 + 模型集
    Resolved(<EntryForm>, <入口>)  首个落盘的候选，且落位 / 跨元素唯一两条判据通过（形态位由
                                    「入口与根的关系」派生，供 `EmitTarget.form`）
    Invalid(<problem>) 显式候选不在盘（`NotOnDisk`）/ 候选落在 `path` 之外（`OutsidePath`）
                       / 与另一元素声明的入口同一文件（`Duplicated`）/ 形态非法（`Malformed`）
    Absent             候选为空（未声明且无约定默认），或目录型根的默认候选全不落盘
  ```

  `<root>` SHALL 为 `path.first()` 经 `normalize_root` 归一的形态（`\` → `/`、剥 `./`、去尾 `/`，**MUST NOT 去扩展名**——文件型根的扩展名正是「根即入口」的判据）。**③ 不做语言判定**：目录型根没有扩展名（`language_of` 对它是 `Unknown`），故四种约定形态一律列出、按序取首个落盘者——四种形态互不相交（同一目录不可能既是 crate 又是视图），因此「按序」不构成语言猜测，也**不**退化成 R2 的列目录规则（R2 已明确否决：新增一个源文件就换默认入口，正是本变更要消灭的漂移源）。`<dir>/lib.rs` MUST NOT 在候选列内（crate 的 `lib.rs` 恒在 `src/` 下）；`<dir>.rs` / `<dir>.ts` / `<dir>.tsx` 三条 MUST NOT 在列（对目录型根它们只能指到 `path` 之外的兄弟文件，必违落位判据）。
- **`entry` 的四条判据**（字段定义见 `desktop-archi-model`「架构模型文件格式与字段封闭」）：**基准** repo 相对 POSIX 路径（与 `path` 同口径，`./` 前缀不参与身份）；**落位** MUST 等于某条 `path` 条目或落在某条**目录型** `path` 之内（单文件根元素声明「同目录的另一个文件」也判 `OutsidePath`）；**存在** 声明或默认命中的文件 MUST 在盘上；**跨元素** 任何两个元素 MUST NOT 解析到同一入口文件。四条违反一律走 `EntryState::Invalid`，由 `apiEntryMissing` 报出（语义见 `desktop-archi-api-check`），MUST NOT 进结构化诊断（九类诊断会让整图停摆，过重）。
- **「无入口」是合法终态**：`Absent` 即该元素不参与契约面（`apiEntryMissing` 的语义：**未声明且无约定默认**）。本仓现基线**恰一例**（本变更新增的生成物中继元素 `App.Frontend.GeneratedBindings`：`path` 指向生成目录，目录内是三个叶子文件、没有 `index.ts(x)`，且它 `api = none` 已退出入口判据 ⇒ **零 `apiEntryMissing`**）；其余元素由 ①②③ 命中。该状态仍然合法（新加目录元素在其入口落地前就会落此态），MUST NOT 用 `api = none` 冒充——两者是不同的事实（「没有门」vs「明说无公共 API」）；反向也 MUST NOT 为消一个 `Absent` 而给生成目录造 `index.ts`（生成目录不是模块入口，造了即造假入口）。
- **`<name>` 的旧口径只剩透传命名面**：`module_name(<root>)`（路径末段）SHALL 继续只服务 Rust / TS 透传命名（D40），MUST NOT 参与生成物文件名——两者的判据不同（`App.Backend.Infra.ArchiRuntime` 的 FQN 末段是 `ArchiRuntime`、路径末段是 `archi`、入口主干是 `lib`）。
- **不强制模块成目录，也不禁止目录成模块**：前端 14 个单文件根元素零模型改动——② 让「根文件自身」成为入口；目录型根的门由约定默认（③）或显式 `entry` 给出。反向同理：目录型根补一个 `index.ts(x)` 即由 ③ 命中，**无需**声明 `entry`（本仓 12 个目录模块入口即此形态，见 `repo-archi-model`「基线落位与模型文件划分」）；本能力 MUST NOT 因「目录型根没有入口」要求新增字段或解析分支。
- **入口编排 SHALL 固定为**：入口文件顶部 `#[path = "<文件名>"] mod api_gen; pub use api_gen::*;`，既有 `pub mod` 声明原样不动。生成类型经 `pub use` 对外可引用；与既有同名类型冲突时编译器直接报 E0255，**不会静默覆盖**。
- `#[path]` 的值 SHALL 为**字面量**，且 SHALL 恒为**入口同级的生成物文件名**——生成物与入口同目录，故值里零 `../`、零目录段：

  | 入口 | `#[path]` 基准 | `#[path]` 值 |
  |---|---|---|
  | `<dir>/src/lib.rs` | 同目录 | `"lib.i.rs"` |
  | `<dir>/index.ts` | 同目录 | `"index.i.rs"` / `"index.i.ts"` |
  | `<root>.tsx`（文件型根） | 同目录 | `"<主干>.i.ts"` |

  等值判据 SHALL 由 `paths_test` 直接断言「入口行的字面量 == `generated_path(entry).file_name()` == `<入口主干>.i.<后缀>`」——**已删**的 `paths::mod_path_attr` 不再是真值源（入口同级后属性值恒为文件名，再留一层函数只是间接：与 D40/D41「零二次派生」同一条纪律）。MUST NOT 手写与入口主干不等值的形态。**`<dir>/index.ts` 这一行在本仓仍零实例**（12 个目录模块的入口都是 TS 侧、无 `#[path]` 行），它描述的是规则而非现状。
- **`#[path]` 的读盘面 SHALL 与写盘面同一条限根纪律**：取数器定位 `#[path = "…"] mod` 时 MUST 满足三条——① **反斜杠归一在折叠之前**（否则反斜杠形态被当作单段名，`Path::join` 的纯拼接会把 `..` 原样带到 Windows 的路径解析里、逐级上溯到观察根之外）；② 分段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即拒）；③ **`join` 后复核**目标仍在观察根之下（`starts_with(root)`）。任一条不成立即**不命中该子树**（与「模块定位失败即跳过」同口径，MUST NOT 新增错误变体、MUST NOT panic）。理由是 `#[path]` 的值经**手写入口行**落地（属性的值只能是字面量，等值性由机械断言守），属可被写错的入参；放行即让「模块树被带出观察根」，与 `paths` 侧的「任意文件读」是同一件事。判据与 `scan` 的 `normalize_rel` / `docs` 的 `resolved_within` 同一条纪律。
- **TS 侧 `import type` 路径 SHALL 遵循同一「入口同级」规则**：同目录目标为 `./<目标入口主干>.i`；跨目录 SHALL 由**入口到入口**的相对路径派生（与 `ref_relative` 同族的单点函数，MUST NOT 手算 `../` 层数）。**本期显式留白**：`.i.ts` 生成物在当前仓库为零件（12 个目录模块入口**不产生** `.i.ts`——它们零契约文件）⇒ 该规则仍无实例可验；`paths::ts_import_specifier` 与 `EntryForm` 三值保留为**待接线形态**，其当前输出（`../<name>.i` 形态）MUST NOT 被当作「入口同级」口径的真值，接线与跨目录派生归前端契约变更。留白 MUST NOT 被写成已实现。
- **MUST NOT 生成 `_ext.rs`**：生成物只含类型定义（`struct` / `trait` / `enum` / newtype）与自动构造器；`impl` 块写在模块内任意手写文件（Rust 允许同 crate 内 `impl` 分散）。生成 / 手写分离靠**文件命名**（`.i.rs` / `.i.ts` 后缀）而非目录分离。
- **归属规则 SHALL 为「三候选并集」**：`.i.<ext>` 生成物（`ext ∈ {rs, ts, tsx, js, jsx}`，标记段恰为 `i` 且模块名非空）的归属 SHALL 由**三个剥离去向取并集**判定——

  ```
  候选 1  整组剥离    <name>.i.<ext>  →  <name>         archi.i.rs → archi
  候选 2  替换扩展名  <stem>.i.<ext>  →  <stem>.<ext>    archi.i.rs → archi.rs
  候选 3  主干对齐    <name>.i.<ext>  →  <name> 与**文件型 `path` 的去扩展名形态**全等
                                                       views/welcome-view.i.ts ↔ views/welcome-view.tsx
  命中判据 = 任一候选满足 rel == prefix 或 rel.startsWith(prefix + "/")
  ```

  理由（三种落点形态互不相交，单条规则覆盖不了）：生成器产出**入口同级**——目录型根下它仍在元素根之内（候选 1 命中，如 `crates/infra/archi/src/lib.i.rs`；`<dir>/<name>.i.<ext>` 的目录内形态由候选 1 一并覆盖）；**文件型根**下它是根的兄弟文件（`views/welcome-view.i.ts`），与 `path` 全名（`views/welcome-view.tsx`）既不相等也不互为前缀，只有候选 3 能命中；候选 2 保留为既有形态（`src/dir.ts` 根对 `src/dir.i.ts`）不做删除。目录型 `path` 的目录模块入口（本仓 12 个 `index.ts(x)`）其生成物为 `<dir>/index.i.ts`，由候选 1 命中（`<dir>` 前缀之内）——**该形态在本仓仍零实例**（无契约即无生成物）。判据改造 SHALL 只此一处（`OwnerIndex` 的剥离函数与其调用点），`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不动，既有非 `.i` 路径的归属语义零变化。
- **并集 MUST NOT 实现为候选短路**：命中集 SHALL 为「对每个候选各跑一遍最长前缀后合并去重」，再按前缀长度定档（同长仍归 `Ambiguous`）；MUST NOT 写成「候选 1 命中即返回」——那会让 `src/dir.i.ts`（候选 1 全等 `src/dir`、候选 2 得 `src/dir.ts`）只走一条路径，与并集语义不符。定档用例：元素根 `src/dir/` 与 `src/dir.ts` 同时在场、探针 `src/dir.i.ts` ⇒ 归更具体的单文件根（前缀更长者胜出）；`src/a.ts` 与 `src/a.tsx` 同时在场、探针 `src/a.i.ts` ⇒ 两文件型根的主干同长，归 `Ambiguous`（MUST NOT 任选）。
- **白名单外后缀零误剥**：`.i.py` / `.d.rs` / 大写 `.I.rs` / 无扩展名 `.i` SHALL 原样返回单个候选（不剥），归属与改动前逐值相同。
- **父子路径重叠不缩窄父契约**：子元素根路径是父元素根路径的前缀，最长前缀匹配已正确处理（`.../src/scan/x.rs` 归 `Scan`、`.../src/dto.rs` 归 `Archi`；`components/agent/` 与 7 个 `components/agent/*.tsx` 子元素的文件型根同理——子元素优先，`index.tsx` 归目录模块）。
- **无可用入口即 `apiEntryMissing`**：`EntryState` 为 `Absent` 或 `Invalid(_)` ⇒ 该元素不是可用模块 → 报一条 `apiEntryMissing`（定义与档位见 `desktop-archi-api-check`）。它比 `apiMissing` 更基础——**在契约生成之前就失效**，防止给不存在的模块写契约。`.i` 生成物的存在 MUST NOT 再当作入口的替代证据：生成物路径是入口的函数，无入口即无生成物路径可派生。**例外恒定**：`api = none` 元素退出入口判据（见 `desktop-archi-api-check`「`api = none` 元素退出入口判据」），故本仓唯一那例 `Absent`（生成物中继元素）零 `apiEntryMissing`。

#### Scenario: 入口解析链三态各命中一条

- **WHEN** 分别对「显式声明 `entry = "packages/desktop/src-tauri/src/lib.rs"` 的壳元素」、「根为文件型 `views/welcome-view.tsx` 的元素」、「根为目录型 `crates/infra/archi/` 且盘上有 `src/lib.rs` 的元素」、「根为目录型 `packages/desktop/src/views/changes/` 且盘上有 `index.ts` 的元素」取 `entry_state`
- **THEN** 四者均为 `Resolved`，入口分别为声明的路径、根文件自身、`crates/infra/archi/src/lib.rs`、`packages/desktop/src/views/changes/index.ts`（③ 的第三候选）；再对根为目录型而 `index.ts` / `index.tsx` 皆不在盘的元素取同一函数 → `Absent`；五例均不 panic、`Absent` 一例不产生成物路径

#### Scenario: 四条判据各一反例

- **WHEN** 分别构造四个反例：`entry = "/abs/x.rs"`（绝对路径）、`entry` 指向另一个元素的 `path` 内的文件、`entry` 指向不存在的文件、两个元素声明同一入口文件；再补一例「文件型根元素声明同目录的另一个文件」
- **THEN** 依次为 `Invalid(Malformed)` / `Invalid(OutsidePath)` / `Invalid(NotOnDisk)` / 双方各 `Invalid(Duplicated)` / `Invalid(OutsidePath)`；五例各自的 `apiEntryMissing` 文案点名具体理由（越界目标 / 缺失文件 / 冲突方 FQN），MUST NOT 只写通用「无入口」

#### Scenario: 生成物落入口同级且 #[path] 无目录段

- **WHEN** 对 crate 形态（入口 `crates/infra/archi/src/lib.rs`）、子目录模块形态（入口 `crates/core/archi/src/scan/mod.rs`）、文件型根（入口 `views/welcome-view.tsx`）分别取 `paths::generated_path` 与入口行字面量
- **THEN** 三者落点为 `crates/infra/archi/src/lib.i.rs`、`crates/core/archi/src/scan/mod.i.rs`、`views/welcome-view.i.ts`（= 各自 `<入口主干>.i.<后缀>`，与入口同目录）；`#[path]` 字面量逐字等于 `generated_path(...).file_name()`（`"lib.i.rs"` / `"mod.i.rs"`），含 `../` 或目录段的形态即判红；`paths::mod_path_attr` 在产品源码内零命中（函数已删）

#### Scenario: 三种落点形态各归其元素

- **WHEN** 分三组求归属：① 入口同级的目录型根落点（`crates/infra/archi/src/lib.i.rs` 对目录形前缀 `crates/infra/archi/`）；② 目录内形态（`views/architecture/architecture.i.ts`、`crates/core/archi/src/scan/scan.i.rs`）；③ 入口同级的文件型根落点（`views/welcome-view.i.ts` 对文件形前缀 `views/welcome-view.tsx`；目录模块入口 `components/agent/index.i.ts` 对目录形前缀 `components/agent/`，候选 1 命中）
- **THEN** 三组用例各归其元素（①③ 由整组剥离 + 主干对齐命中、② 由整组剥离 / 替换扩展名命中，三者并集）；非 `.i` 路径（如 `crates/core/archi/src/dto.rs`）归属结果与改造前逐字相同

#### Scenario: 并集定档不短路

- **WHEN** 元素根 `src/dir/` 与 `src/dir.ts` 同时在场，探针为 `src/dir.i.ts`；另一组为 `src/a.ts` 与 `src/a.tsx` 同时在场、探针 `src/a.i.ts`
- **THEN** 前者归更具体的单文件根（`src/dir.ts` 所在元素，前缀更长）；后者两文件型根主干同长 ⇒ `Ambiguous`（MUST NOT 任选）；若实现成「候选 1 命中即短路」，前者会误归目录形根——该差异即本 Scenario 的判红面

#### Scenario: 白名单外后缀不误剥

- **WHEN** 对 `src/mod/mod.i.py`、`src/mod/mod.d.rs`、`src/mod/mod.I.rs`、`src/mod/mod.i` 求归属（元素根 `src/mod/`）
- **THEN** 四者按原路径判定归属（不剥），结果与改动前逐值相同；`.i.<白名单外后缀>` 不产生第二 / 第三个候选

#### Scenario: 本仓入口面实测 46 / 45 / 1 / 0

- **WHEN** 对本仓 46 个 `path` 非空元素跑 `entry_state`
- **THEN** `Resolved` **45** 条：14 个自研 crate 由 ③ 命中 `src/lib.rs` + 多文件根 `App.Frontend.Shell` 由 ② 命中 `app.tsx` + **14** 个前端单文件根由 ② 命中自身（7 个 `components/agent/*.tsx` + 6 个 `standard/*.tsx`（含 `combobox.tsx`）+ `views/welcome-view.tsx`）——其中目录模块 `App.Frontend.Components.Agent` 改由 ③ 命中 `components/agent/index.tsx` + **12** 个目录模块根由 ③ 命中 `index.ts(x)` + 壳 `App.Backend.Shell` 与两个工具 crate 由 ① 命中显式 `entry` = **45**；`Absent` **1** 条（`App.Frontend.GeneratedBindings`：`path` 为生成目录、目录内无 `index.ts(x)`，且它 `api = none` ⇒ **零 `apiEntryMissing`**）、`Invalid` **0** 条。三处显式 `entry` 仍是本节 ① 在本仓的**唯一实例**（12 个目录模块入口走 ③，MUST NOT 为其补 `entry` 行；生成物中继元素也 MUST NOT 为它补 `entry`——生成目录的叶子文件不是模块入口）；反向判据：删掉任一目录的 `index.ts` 即该元素回落 `Absent` 并产一条 `apiEntryMissing`，说明「入口面清零」是**文件在盘**的结果而不是判据放宽

#### Scenario: 生成物中继元素的 Absent 不是债

- **WHEN** 对生成物中继元素（`kind = module` / `api = none` / `path = ./packages/desktop/src/types/generated/`）跑一次全量检查
- **THEN** 它零 `apiEntryMissing`（`api = none` 退出入口判据）、恰一条 `docMissing`（`doc` 未声明）；给它造一个 `index.ts` 会让 `entry_state` 转 `Resolved` 但**不减少任何判据条目**（`docMissing` 仍在）——故「造入口」不是可接受的处置，MUST NOT 在本变更或后续变更里为消 `Absent` 而造生成目录的 `index.ts`

#### Scenario: 生成物与手写 impl 共存编译

- **WHEN** 入口文件 `crates/infra/archi/src/lib.rs` 写 `#[path = "lib.i.rs"] mod api_gen; pub use api_gen::LocalArchiCheck;`，同级生成物 `crates/infra/archi/src/lib.i.rs` 含 `pub struct LocalArchiCheck;` + `impl` 内 `new()`，手写文件另有 `impl Default for LocalArchiCheck`
- **THEN** 编译通过（`rustc --edition 2021`），外部可经入口路径引用 `LocalArchiCheck::new()` 且签名受生成物约束

#### Scenario: 同名冲突不静默覆盖

- **WHEN** 生成物定义了元素内已手写的同名类型且入口 `pub use api_gen::*;`
- **THEN** 编译器报 E0255（重复定义），MUST NOT 静默采用其一

#### Scenario: 路径形态非法即拒收（安全边界）

- **WHEN** 元素 `<root>`、入口候选或契约 FQN 取绝对路径 / 含盘符 / 含 `..` 段 / 含空段 / 为空串（如 `path = ["/abs/elsewhere"]`、`path = ["crates/../../outside"]`、`entry = "../x.rs"`、`fqn = "App..X"`）
- **THEN** `contract_path` / `schema_path` / `generated_path` / `entry_state` 一律返回空或 `Absent`（`Malformed` 形态归 `Absent` / `Invalid`），不 panic、不落到仓外；`root.join(绝对路径)` 的「替换 base」与 `..` 的「上溯」因此结构上不可达——「证据在盘」的探测不会变成任意文件读，工具 crate 的写盘不会变成任意文件写。**第二层**：`archi-api-gen` 的写盘函数 SHALL 另以目标须落在观察根之下（`starts_with(root)`）复核，越界即拒绝并零副作用（两层判据交叉验证，任一层失效另一层仍拦得住）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/paths.rs` | 入口解析与落点派生（零改动） | `entry_state` 三态链、四条判据、`generated_path` 与 `ts_import_specifier` 逐字不变（新元素只走既有 ③ / ④ 分支） |
| `crates/core/archi/src/api/check.rs` | 入口判据（零改动） | `api = none` 退出 `apiEntryMissing` 的语义不变；`Absent` 只在 `api = declared` 时产条目 |
| `crates/core/archi/src/dto.rs`（`resolved_entry` 投影） | 入口读数（零改动） | `archi_module` 的 `resolvedEntry` 对 `Absent` 给 `via = null` + `problem` 文案；新元素因此可被 `archi_module` 直接判出「无入口」 |
| `.dev-team/architecture/graph/model/06-frontend.toml` | 数据面（见 `repo-archi-model`） | 新元素 `api = none` / 无 `entry` / `path` = 生成目录；不为其造 `index.ts`、不为其补 `entry` 行 |
| 本能力实测段（spec 面） | 改 | 入口面读数 44 / 0 / 0 → **46 / 45 / 1 / 0**，并把「`Absent` 一例为何不是债」写实 |

### Function: `crates/core/archi/src/api/paths.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `entry_state` | 零改动 | 三态链与四条判据逐字不变；新元素落 `Absent`（目录型根、默认候选全不落盘） |
| `generated_path` / `ts_import_specifier` | 零改动 | 落点派生与 TS 侧规则不变；`Absent` 不产生成物路径 |
| `carrier_path`（入口文档载体） | 零改动 | `api = none` 取 `doc`（新元素未声明 ⇒ `docMissing`），`declared` 取派生契约路径 |

### API: `archi_module.resolvedEntry`

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiModuleDetail.resolvedEntry` | 字段级（零改动） | 新元素呈 `path = null` + `via = null` + `problem`（「无可用入口」）；`archiEntryMissing` 不因它产出（`api = none` 退出入口判据） |
