# desktop-archi-api Specification（delta: frontend-barrel-entry-migration）

## Purpose

本 delta 只做**两处本仓事实校准**，入口解析链、生成物落点、四条判据、取数面算法与四类留白**逐字不动**（本变更**零 Rust 改动**）：

1. **入口面实测换段**：12 个前端目录根补 `index.ts(x)` 后，本仓 44 个 `path` 非空元素的 `entry_state` 由「32 `Resolved` + 12 `Absent`」变为 **44 `Resolved` + 0 `Absent` + 0 `Invalid`**。「`Absent` 是合法终态」这条判据**保留**（它描述的是解析链的能力，不是本仓现状），只把举例从「前端目录根 / `ui`」改为「本仓现基线无一例」。
2. **一处叙述实例翻新**：`App.Frontend.Components.Agent` 的 `path` 由文件型（`components/agent/index.tsx`）改为目录型（`components/agent/`），故「文件型 `path` 的元素因此不需要任何例外分支」的举例与「本仓取数面恰七名」Scenario 的 `path` 形态同批改写——**结论不变**：owner 子树仍放行、7 名仍恰为 7 名（入口由 ③ 命中同一份 `index.tsx`）。
3. **与其它能力的边界不变**：`paths::ts_import_specifier` 与 `.i.ts` 生成链仍是**待接线留白**（本变更零 `.i.ts` 实例，故仍无观测点）；生成物落点、`INDEX.toml` 的 `generated` 列（有契约才填）与四类留白（`export { localA }` / `export * as ns` / 跨元素 re-export / `export default`）零改动。

## MODIFIED Requirements

### Requirement: 生成物落点与入口查找

生成物 SHALL 落**实现入口文件同级**（`<入口主干>.i.rs` / `<入口主干>.i.ts`），其中 `<入口主干>` SHALL 为入口文件名的**去扩展名形态**（`crates/infra/archi/src/lib.rs` → `lib`、`views/welcome-view.tsx` → `welcome-view`、`packages/desktop/src-tauri/src/lib.rs` → `lib`）。入口 SHALL 由本节的三态解析链给出，生成物路径因此是**入口的单值函数**；元素根路径（`path.first()`）MUST NOT 再参与落点派生（它只回答「根在哪」）。

- **`<root>` 归一在模型解析层完成**：`<root>` SHALL 为 `path.first()` 经 `normalize_root` 归一后的形态（`OwnerIndex::normalize` 已处理 `./` 前缀与尾 `/`），MUST NOT 批量改写存量 models——实测存量 44 条 `path` 全部带 `./` 前缀 + 尾 `/`、一律需要归一（文件型根的扩展名是「根即入口」的判据，故归一 MUST NOT 去扩展名）。
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
- **「无入口」是合法终态**：`Absent` 即该元素不参与契约面（`apiEntryMissing` 的语义：**未声明且无约定默认**）。本仓现基线**无一例**——12 个前端目录根已由 `desktop-frontend-module-entry` 的 `index.ts` / `index.tsx` 经 ③ 命中；该状态仍然合法（新加目录元素在其入口落地前就会落此态），MUST NOT 用 `api = none` 冒充——两者是不同的事实（「没有门」vs「明说无公共 API」）。
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
- **无可用入口即 `apiEntryMissing`**：`EntryState` 为 `Absent` 或 `Invalid(_)` ⇒ 该元素不是可用模块 → 报一条 `apiEntryMissing`（定义与档位见 `desktop-archi-api-check`）。它比 `apiMissing` 更基础——**在契约生成之前就失效**，防止给不存在的模块写契约。`.i` 生成物的存在 MUST NOT 再当作入口的替代证据：生成物路径是入口的函数，无入口即无生成物路径可派生。

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

#### Scenario: 本仓入口面实测 44 / 0 / 0

- **WHEN** 对本仓 44 个 `path` 非空元素跑 `entry_state`
- **THEN** `Resolved` **44** 条：14 个 crate 由 ③ 命中 `src/lib.rs` + 多文件根 `App.Frontend.Shell` 由 ② 命中 `app.tsx` + **14** 个前端单文件根由 ② 命中自身（7 个 `components/agent/*.tsx` + 5 个 `standard/*.tsx` + `views/welcome-view.tsx` + `App.Frontend.Components.Agent` 翻转前）——翻转后 `Components.Agent` 改由 ③ 命中 `components/agent/index.tsx` + **12** 个目录模块根由 ③ 命中 `index.ts(x)` + 壳 `App.Backend.Shell` 与两个工具 crate 由 ① 命中显式 `entry` = 44；`Absent` **0** 条、`Invalid` 0 条。三处显式 `entry` 仍是本节 ① 在本仓的**唯一实例**（12 个目录模块入口走 ③，MUST NOT 为其补 `entry` 行——否则违 `repo-archi-model` 的字段纪律）；反向判据：删掉任一目录的 `index.ts` 即该元素回落 `Absent` 并产一条 `apiEntryMissing`，说明「入口面清零」是**文件在盘**的结果而不是判据放宽

#### Scenario: 生成物与手写 impl 共存编译

- **WHEN** 入口文件 `crates/infra/archi/src/lib.rs` 写 `#[path = "lib.i.rs"] mod api_gen; pub use api_gen::LocalArchiCheck;`，同级生成物 `crates/infra/archi/src/lib.i.rs` 含 `pub struct LocalArchiCheck;` + `impl` 内 `new()`，手写文件另有 `impl Default for LocalArchiCheck`
- **THEN** 编译通过（`rustc --edition 2021`），外部可经入口路径引用 `LocalArchiCheck::new()` 且签名受生成物约束

#### Scenario: 同名冲突不静默覆盖

- **WHEN** 生成物定义了元素内已手写的同名类型且入口 `pub use api_gen::*;`
- **THEN** 编译器报 E0255（重复定义），MUST NOT 静默采用其一

#### Scenario: 路径形态非法即拒收（安全边界）

- **WHEN** 元素 `<root>`、入口候选或契约 FQN 取绝对路径 / 含盘符 / 含 `..` 段 / 含空段 / 为空串（如 `path = ["/abs/elsewhere"]`、`path = ["crates/../../outside"]`、`entry = "../x.rs"`、`fqn = "App..X"`）
- **THEN** `contract_path` / `schema_path` / `generated_path` / `entry_state` 一律返回空或 `Absent`（`Malformed` 形态归 `Absent` / `Invalid`），不 panic、不落到仓外；`root.join(绝对路径)` 的「替换 base」与 `..` 的「上溯」因此结构上不可达——「证据在盘」的探测不会变成任意文件读，工具 crate 的写盘不会变成任意文件写。**第二层**：`archi-api-gen` 的写盘函数 SHALL 另以目标须落在观察根之下（`starts_with(root)`）复核，越界即拒绝并零副作用（两层判据交叉验证，任一层失效另一层仍拦得住）

### Requirement: TS 导出闭包的取数面

`api::extract::extract_element` SHALL 对 TypeScript / JavaScript 元素产出**入口导出闭包**：从实现入口出发，沿 `export … from` 一跳或多跳并入目标文件的导出。闭包 SHALL 是该元素「代码导出集」的唯一来源（判据的导出集一侧与 `bootstrap` 草稿同源），MUST NOT 存在第二份 TS 导出集口径。

- **入口唯一全量起点**：只有从入口可达的导出进公共面——入口自身声明的导出，加上入口 re-export 可达链上的导出。MUST NOT 把元素目录下**未被入口聚合**的文件导出并入：那是「入口 = 公共面」之外的第二套定义。入口读不到 ⇒ 空集（既有语义逐字不变，缺口由 `apiEntryMissing` 报）。
- **具名名单精确**：`export { A } from './x'` / `export type { A } from './x'` SHALL 只把 `A` 入集，MUST NOT 把 `./x` 的全部导出并入（虚增面）。`export { A as B } from './x'` SHALL 在 `./x` 取 `A`、以 **`B`** 入集（改名如实反映，MUST NOT 以 `A` 入集）。
- **通配取全量面**：`export * from './x'` SHALL 取 `./x` 的导出面（`./x` 自身再含 re-export 时递归同理）；`export *` 与逐名 re-export 在闭包内 SHALL 等价——同一簇文件的两种写法产出同名同 kind 的集合。`components/ui/index.ts` 的 **39 条 `export *`** 即本仓该形态的唯一批量实例（`Components.Ui` 的导出集 = 39 个模块的导出面）。
- **环防键 =（目标文件, 名字集）**：visited SHALL 以「目标文件 + 本次请求的名字集」为键，键命中即返回空集。MUST NOT 只用文件作键——同一目标文件的两条具名语句（本仓 `components/agent/index.tsx` 的 `export { AgentSessionView } from './agent-session-view'` 与 `export type { AgentSessionActions, AgentSessionViewProps } from './agent-session-view'`）会被互相吞掉，静默丢名字且无任何报警。
- **终止性与确定性**：`a → b → a` 环与菱形（同一类型经两条路径到达）SHALL 由 visited 收敛，MUST NOT panic、MUST NOT 重复、MUST NOT 无限递归；终止性 MUST 由 visited 保证，MUST NOT 依赖深度上限（若实现另设上限作第二道保险，触顶 MUST NOT panic、MUST NOT 把部分面冒充完整面）。同一输入两次调用 SHALL 产出逐字相等的结果（稳定序 = 入口语句序 + 目标文件声明序，MUST NOT 依赖哈希序）。
- **去重键 =（name, kind）**：同一符号经多条路径到达 SHALL 只保留首次出现；去重 MUST NOT 按结构（两个同名字不同类型不得合并，与「禁按结构去重」同一条纪律）。
- **同元素约束 = owner 子树**：re-export 目标文件 SHALL 经 `scan::OwnerIndex::owner_of` 归属，仅当结果为 `Owner::Element { fqn }` 且 `fqn == element.fqn` 或以 `"{element.fqn}."` 为前缀时并入；`Owner::Ambiguous(_)` 与 `Owner::None` SHALL 一律跳过（MUST NOT 任选、MUST NOT 放行）。FQN SHALL 按 `.` 分段比较，MUST NOT 裸 `starts_with(element.fqn)`（否则 `…Components.Agent` 会误认 `…Components.AgentFoo` 为子树）。**理由**：父元素的公共面 ⊇ 可达子元素面正是 barrel 的语义，与目录型 `path` 前缀在结论上一致（不产生第二条规则分叉），MUST NOT 被读成虚增——虚增由具名名单纪律守、完备性由 `apiUndeclared` 守。故**目录型 `path` 的 barrel 元素**（本仓 12 个目录模块入口，例如 `Views.Changes` 的 `views/changes/index.ts` 聚合两个直属文件）不需要任何例外分支：入口在 `path` 之内、目标在本元素子树内，`within_path` 与 owner 子树同时成立。
- **specifier 解析单点复用**：目标文件解析 SHALL 复用 `scan::ts::TsResolver::resolve`（就近 tsconfig 的 `baseUrl` / `paths` / 别名 / 扩展名 / index 折叠），MUST NOT 造第二份 specifier 解析。解析失败（目标在仓外 / 不在盘 / 不可解析）SHALL 静默跳过该条 re-export：MUST NOT panic、MUST NOT 记 `unresolved`（`unresolved` 只属依赖边抽取的账）。`api::extract → scan::{ts, owner}` 的内部依赖为本能力新增，与 `api/diff.rs` / `api/paths.rs` 已有的 `scan` 依赖同族——理由是「specifier 解析与文件归属各只有一个实现」。
- **visibility 一律 `External`**：闭包内符号 SHALL 全部为 `External`（可达即公开），与 Rust 侧 `pub use` 升格的语义对齐；本能力 MUST NOT 新增 `crateOnly` / `testOnly` / `unused` 的产出路径（三值留白逐字不变）。
- **签名带元素与元素表**：`extract_element(root: &Path, elements: &[ArchiElement], element: &ArchiElement, entry: &str) -> Vec<ApiSymbol>`——同元素约束的唯一依据；与 `paths::entry_state(root, elements, element)` 同形。Rust 分支语义 SHALL 逐字不变（`mod` 树自锚，元素参数不参与，MUST NOT 因此改 `rust::extract_crate` 的抽取面）。
- **导出形态单一分派点**：`api/extract/ts.rs` 内 `match statement` SHALL 恰 1 处，一次分派产出三态（`Declaration` / `Named{local, exported, specifier}` / `Star(specifier)`），其余形态归 `None`。`ts::extract_file(file, source)` 的签名与产出语义 SHALL 逐字不变（只认 `export <声明>`，re-export 零符号）；新增的声明/名单抽取面只读源文本 + AST（零读盘、零写盘、零 spawn）。闭包（递归 + 读盘 + 解析 + 环防）SHALL 落在 `extract_element` 那一层，MUST NOT 下沉进 `ts.rs`。
- **定义位置带文件**：`ApiSymbol` SHALL 新增 `file`（定义所在文件，repo 相对 POSIX 路径）。`bootstrap` 的注记 SHALL 为 `<!-- <visibility> @ <file>:<line> -->` 且 `file` 取该符号的定义文件——MUST NOT 用入口串冒充（闭包一跳后行号属于另一个文件，注释指错文件比没有注释更坏）。Rust 侧 `file` SHALL 为该符号所在模块文件（内联 `mod` 与父同文件）。

**显式留白（MUST NOT 被伪装成已实现）**：

| 形态 | 处置 |
|---|---|
| `export { localA }`（无 `from`，导出本文件的非导出声明） | 不产出：需要「按名字取回本文件声明」的第二遍，与「取数面 = 导出面」的单遍纪律冲突；`api/extract/ts.rs` 内 `Statement::ExportNamedDeclaration` MUST 零命中（由源码断言钉住） |
| `export * as ns from './x'` | 跳过：产出命名空间对象而非符号，不猜 |
| 跨元素 / 仓外 re-export | 静默跳过，**零 finding**：取数器非权威，缺口由契约面判据显示；MUST NOT 为它新增判据（避免与 `apiRefMissing` 语义重复）。`desktop-frontend-module-entry` 的同名纪律（入口只导本元素直属文件）是**代码侧**要求，两者不是同一件事：取数器放行子元素目标，代码纪律不许写出这种目标 |
| `export default` | 不抽（既有纪律：对外名字由导入方决定） |

**预期边界（写进能力，不许口头约定）**：前端元素的真实导出面以**类型面与名面**为主——组件函数的入参是解构 props + 内联类型字面量，抽出的参数类型恒为 `Unknown`。故本能力 MUST NOT 被读成「前端行为契约可写」：`api-op` 面在前端继续留白，`bootstrap` 对前端的价值是「名与类型面的起点」，不是可直接落纸的签名。12 个目录模块入口落地后这一边界**不变**：它们把「名面」变得可机械取数（`flow` 的 11 名、`renderers` 的 2 名一类），不使 `api-op` 面变得可写。

#### Scenario: 精确名单与通配等价

- **WHEN** 对同一簇夹具（`./x` 导出 `A` / `B` / `C`）分别构造「逐名 re-export 三条语句」与「一条 `export * from './x'`」两个入口，各跑一次 `extract_element`
- **THEN** 两次产出逐字段相等（名字集与顺序均同，同为 3 个符号）；再构造「入口只 `export { A } from './x'`」⇒ 产出恰 1 个符号，`B` / `C` 零命中（精确名单，MUST NOT 虚增）

#### Scenario: 环与菱形确定收敛

- **WHEN** 夹具含 `a → b → a` 的 re-export 环，以及菱形（`entry` 同时 `export * from './l'` 与 `export * from './r'`，两者都 `export * from './leaf'`）
- **THEN** 两次 `extract_element` 均正常返回（零 panic、零无限递归）；环内符号各出现一次、菱形里 `leaf` 的符号按 `(name, kind)` 恰一条；同一输入连续两次调用结果逐字相等

#### Scenario: 同元素约束按 owner 子树裁

- **WHEN** 夹具含四条 re-export：①目标归属本元素子元素（`Owner::Element`，FQN 以 `"{本元素}."` 为前缀）、②目标归属另一个元素、③目标无元素认领（`Owner::None`，如 `@/lib/**` 形态）、④目标同长多命中（`Owner::Ambiguous`）
- **THEN** 只有 ① 入集；②③④ 全部跳过且零 finding、零 panic；把 ① 的 FQN 改成 `"{本元素}Foo"`（裸前缀相同但不分段相等）⇒ 不入集

#### Scenario: 本仓 Components.Agent 取数面恰七名

- **WHEN** 以本仓根对 `App.Frontend.Components.Agent`（`path = ./packages/desktop/src/components/agent/`，目录型；入口由 ③ 命中目录内 `index.tsx`）取数
- **THEN** 导出集恰 7 个名字 `AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`（3 个 `Function` + 4 个 `Type`）；`AgentInputProps` / `AgentMessagesProps` / `isToolPart` 零命中（未被 barrel 导出 ⇒ 具名名单不入集）；同一次取数在 `bootstrap` 草稿里表现为非空骨架。**路径形态翻转前后取数结果逐字相同**——入口仍是同一份 `index.tsx`，翻转只改 `kind` 与 `path` 的用法

#### Scenario: 本仓目录模块入口的取数面

- **WHEN** 分别对 `App.Frontend.Views.Changes`（入口 `views/changes/index.ts`）与 `App.Frontend.Views.Changes.Renderers`（入口 `views/changes/renderers/index.ts`）取数
- **THEN** 前者导出集恰 `ChangeDetailView` / `ChangeListView` 两名，后者恰 `ArtifactTabs` / `MarkdownDocRenderer` 两名；两元素的目录内其余公开符号（如 `renderers/registry.ts` 的成员、`changes/hooks/**`）**零命中**（未被入口聚合 ⇒ 不在公共面）；`Components.Ui` 的导出面按 39 条 `export *` 取全量（其「目录 = 模块」语义由 `desktop-frontend-module-entry` 定义）

#### Scenario: 留白形态零产出

- **WHEN** 分别对 `export { localA };`（无 `from`）、`export * as ns from './x'`、`export default function f() {}` 三种源文取数
- **THEN** 三者均零符号；`extract_file` 的产出语义不变（透传与 default 都不产符号），且 `api/extract/ts.rs` 内 `Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中（留白由源码断言钉住，不是靠「忘了写」）

#### Scenario: 注记行指向真实定义文件

- **WHEN** 对本仓 `App.Frontend.Components.Agent` 起草（`bootstrap`）
- **THEN** 每个符号的注记指向其真实定义文件与行（`AgentInput` → `packages/desktop/src/components/agent/agent-input.tsx:152`、`SessionPanel` → `…/session-panel.tsx:230`、`AskAnswerActions` → `…/agent-messages.tsx:33`），MUST NOT 出现把入口 `index.tsx` 与目标文件行号拼在一起的形态；Rust 侧同一条纪律（子模块 `pub fn` 的注记指向子模块文件）

#### Scenario: 解析失败静默跳过

- **WHEN** 入口 `export { usePanelRef } from 'react-resizable-panels'`（仓外包）、`export { X } from './not-on-disk'`（不在盘）两种形态
- **THEN** 两者都被静默跳过：`extract_element` 正常返回（零 panic、零符号、零 finding），`unresolved` 留痕为零（该账只属依赖边抽取）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/paths.rs` | 入口与生成物路径单点 | **零改动**：`entry_candidates` 的 ③ 四候选（`src/lib.rs` / `mod.rs` / `index.ts` / `index.tsx`）与 `entry_state` 四判据逐字不变；本变更只让 ③ 在本仓命中 12 次（12 个目录模块入口）。`ts_import_specifier` 仍是待接线留白 |
| `crates/core/archi/src/api/extract/mod.rs` + `extract/ts.rs` | TS 导出闭包取数面 | **零改动**：具名名单 / 通配 / 环防键 / owner 子树 / `(name, kind)` 去重 / 定义文件 `ApiSymbol.file` 逐字不变；本变更只新增被取数的对象（12 个 barrel） |
| `crates/core/archi/src/api/mod.rs`（`index_entries`） | `INDEX.toml` 投影 | **零改动**：`generated` 列只在契约在案时填 ⇒ 12 个目录模块元素仍为 `status = absent` / `generated = ""`，`.dev-team/architecture/api/**` 零 diff |
| `.dev-team/architecture/models/06-frontend.toml` | 本仓入口面的数据来源 | `App.Frontend.Components.Agent` 的 `kind` / `path` 翻转（`domain` + 文件 → `module` + 目录），零 `entry` 行；12 个目录根元素的 `path` 不变（入口由 ③ 从盘上取） |
| `packages/desktop/src/views/**` / `components/ui/index.ts` | 入口文件（被取数对象） | 12 个 `index.ts(x)`；名字面见 `desktop-frontend-module-entry`；MUST NOT 在入口内写实现 |

### Function: 无

本能力不新增函数：`entry_candidates` / `entry_state` / `extract_element` / `extract_surface` / `index_entries` 的签名与语义逐字不变。

### API: 无

本能力不新增契约资产、不改 `ApiSymbol` / `ApiSnapshot` / `INDEX.toml` 形状；`.i.ts` 生成链与 `ts_import_specifier` 仍为待接线留白。

### CLI: 无

`api:export` / `api:check` / `bootstrap` 三条命令形态逐字不变；本变更内 `api:check` 应报零差异。

### Component: 无

本能力零前端组件改动。
