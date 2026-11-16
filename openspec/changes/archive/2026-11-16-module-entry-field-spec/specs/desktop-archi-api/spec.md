# desktop-archi-api Specification（delta: module-entry-field-spec）

## Purpose

本 delta 把生成物的落点从「元素根同级」搬到「实现入口同级」，并把「入口是什么」从盘上探测升级为模型字段 + 解析链：

1. **落点**（§三）：`<入口主干>.i.<后缀>`，与入口同目录；`#[path]` 的值随之退化为**文件名**（零 `../`），`paths::mod_path_attr` 删除，等值性由 `paths_test` 直接断言。
2. **入口**（§一 / §二）：模型新增单值 `entry` 字段；解析链封闭三态（显式声明 → 语言约定默认 → 无入口）；四条判据（基准 / 落位 / 存在 / 跨元素唯一）单点判定，「无入口」成为合法终态。
3. **归属**（§四）：生成物归属由「双形态并集」扩为**三候选并集**（新增主干对齐），使文件型根的入口同级生成物仍归其元素。
4. **纪律节**（§五）：仓库根 `AGENT.md` / `CLAUDE.md` 的 `## 模块 API 契约` 第②条方位词随落点同步（「元素根同级」→「实现入口同级」、`<root>.i.*` → `<入口主干>.i.*`），**条目数恒 5**、其余四条要点与同源纪律逐字不动——落点一迁则该条即刻与实现矛盾，而该文件是 agent 每轮注入的系统提示词，故措辞同步属实现面（不是「加第六条要点」）。

其余九个 requirement（资产形态、派生与不可手改、引用与命名、关键字与类型映射、语言无关纪律、verify 闭环与 bootstrap 剪枝、模块嵌套与选择性透传、属性键闸、降级词表）**逐字不动**；TS 侧跨元素透传的相对路径真算**显式留白**（归前端契约变更）。

## MODIFIED Requirements

### Requirement: 生成物落点与入口查找

生成物 SHALL 落**实现入口文件同级**（`<入口主干>.i.rs` / `<入口主干>.i.ts`），其中 `<入口主干>` SHALL 为入口文件名的**去扩展名形态**（`crates/infra/archi/src/lib.rs` → `lib`、`views/welcome-view.tsx` → `welcome-view`、`packages/desktop/src-tauri/src/lib.rs` → `lib`）。入口 SHALL 由本节的三态解析链给出，生成物路径因此是**入口的单值函数**；元素根路径（`path.first()`）MUST NOT 再参与落点派生（它只回答「根在哪」）。

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

  `<root>` SHALL 为 `path.first()` 经 `normalize_root` 归一的形态（`\` → `/`、剥 `./`、去尾 `/`，**MUST NOT 去扩展名**——文件型根的扩展名正是「根即入口」的判据）。**③ 不做语言判定**：目录型根没有扩展名（`language_of` 对它是 `Unknown`），故四种约定形态一律列出、按序取首个落盘者——四种形态互不相交（同一目录不可能既是 crate 又是视图），因此「按序」不构成语言猜测，也**不**退化成 R2 的列目录规则（R2 已明确否决：新增一个源文件就换默认入口，正是本变更要消灭的漂移源）。`<dir>/lib.rs` MUST NOT 在候选列内（crate 的 `lib.rs` 恒在 `src/` 下）；`<dir>.rs` / `<dir>.ts` / `<dir>.tsx` 三条 MUST NOT 在列（对目录型根它们只能指到 `path` 之外的兄弟文件，必违落位判据，实测零命中）。
- **`entry` 的四条判据**（字段定义见 `desktop-archi-model`「架构模型文件格式与字段封闭」）：**基准** repo 相对 POSIX 路径（与 `path` 同口径，`./` 前缀不参与身份）；**落位** MUST 等于某条 `path` 条目或落在某条**目录型** `path` 之内（单文件根元素声明「同目录的另一个文件」也判 `OutsidePath`）；**存在** 声明或默认命中的文件 MUST 在盘上；**跨元素** 任何两个元素 MUST NOT 解析到同一入口文件。四条违反一律走 `EntryState::Invalid`，由 `apiEntryMissing` 报出（语义见 `desktop-archi-api-check`），MUST NOT 进结构化诊断（九类诊断会让整图停摆，过重）。
- **「无入口」是合法终态**：`Absent` 即该元素不参与契约面（今天的 `apiEntryMissing` 语义平移：**未声明且无约定默认**）。前端目录根缺 `index.ts`、`ui` 这类按裁定刻意收敛的层都属此态，MUST NOT 用 `api = none` 冒充——两者是不同的事实（「没有门」vs「明说无公共 API」）。
- **`<name>` 的旧口径只剩透传命名面**：`module_name(<root>)`（路径末段）SHALL 继续只服务 Rust / TS 透传命名（D40），MUST NOT 参与生成物文件名——两者的判据不同（`App.Backend.Infra.ArchiRuntime` 的 FQN 末段是 `ArchiRuntime`、路径末段是 `archi`、入口主干是 `lib`）。
- **不强制模块成目录**：前端 12 个单文件根元素零模型改动——② 让「根文件自身」成为入口，无需补 barrel；目录型根的门由约定默认（③）或显式 `entry` 给出。
- **入口编排 SHALL 固定为**：入口文件顶部 `#[path = "<文件名>"] mod api_gen; pub use api_gen::*;`，既有 `pub mod` 声明原样不动。生成类型经 `pub use` 对外可引用；与既有同名类型冲突时编译器直接报 E0255，**不会静默覆盖**。
- `#[path]` 的值 SHALL 为**字面量**，且 SHALL 恒为**入口同级的生成物文件名**——生成物与入口同目录，故值里零 `../`、零目录段：

  | 入口 | `#[path]` 基准 | `#[path]` 值 |
  |---|---|---|
  | `<dir>/src/lib.rs` | 同目录 | `"lib.i.rs"` |
  | `<dir>/index.ts` | 同目录 | `"index.i.rs"` / `"index.i.ts"` |
  | `<root>.tsx`（文件型根） | 同目录 | `"<主干>.i.ts"` |

  等值判据 SHALL 由 `paths_test` 直接断言「入口行的字面量 == `generated_path(entry).file_name()` == `<入口主干>.i.<后缀>`」——**已删**的 `paths::mod_path_attr` 不再是真值源（入口同级后属性值恒为文件名，再留一层函数只是间接：与 D40/D41「零二次派生」同一条纪律）。MUST NOT 手写与入口主干不等值的形态。
- **`#[path]` 的读盘面 SHALL 与写盘面同一条限根纪律**：取数器定位 `#[path = "…"] mod` 时 MUST 满足三条——① **反斜杠归一在折叠之前**（否则反斜杠形态被当作单段名，`Path::join` 的纯拼接会把 `..` 原样带到 Windows 的路径解析里、逐级上溯到观察根之外）；② 分段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即拒）；③ **`join` 后复核**目标仍在观察根之下（`starts_with(root)`）。任一条不成立即**不命中该子树**（与「模块定位失败即跳过」同口径，MUST NOT 新增错误变体、MUST NOT panic）。理由是 `#[path]` 的值经**手写入口行**落地（属性的值只能是字面量，等值性由机械断言守），属可被写错的入参；放行即让「模块树被带出观察根」，与 `paths` 侧的「任意文件读」是同一件事。判据与 `scan` 的 `normalize_rel` / `docs` 的 `resolved_within` 同一条纪律。
- **TS 侧 `import type` 路径 SHALL 遵循同一「入口同级」规则**：同目录目标为 `./<目标入口主干>.i`；跨目录 SHALL 由**入口到入口**的相对路径派生（与 `ref_relative` 同族的单点函数，MUST NOT 手算 `../` 层数）。**本期显式留白**：`.i.ts` 生成物在当前仓库为零件 ⇒ 该规则无实例可验；`paths::ts_import_specifier` 与 `EntryForm` 三值保留为**待接线形态**，其当前输出（`../<name>.i` 形态）MUST NOT 被当作「入口同级」口径的真值，接线与跨目录派生归前端契约变更。留白 MUST NOT 被写成已实现。
- **MUST NOT 生成 `_ext.rs`**：生成物只含类型定义（`struct` / `trait` / `enum` / newtype）与自动构造器；`impl` 块写在模块内任意手写文件（Rust 允许同 crate 内 `impl` 分散）。生成 / 手写分离靠**文件命名**（`.i.rs` / `.i.ts` 后缀）而非目录分离。
- **归属规则 SHALL 为「三候选并集」**：`.i.<ext>` 生成物（`ext ∈ {rs, ts, tsx, js, jsx}`，标记段恰为 `i` 且模块名非空）的归属 SHALL 由**三个剥离去向取并集**判定——

  ```
  候选 1  整组剥离    <name>.i.<ext>  →  <name>         archi.i.rs → archi
  候选 2  替换扩展名  <stem>.i.<ext>  →  <stem>.<ext>    archi.i.rs → archi.rs
  候选 3  主干对齐    <name>.i.<ext>  →  <name> 与**文件型 `path` 的去扩展名形态**全等
                                                       views/welcome-view.i.ts ↔ views/welcome-view.tsx
  命中判据 = 任一候选满足 rel == prefix 或 rel.startsWith(prefix + "/")
  ```

  理由（三种落点形态互不相交，单条规则覆盖不了）：生成器产出**入口同级**——目录型根下它仍在元素根之内（候选 1 命中，如 `crates/infra/archi/src/lib.i.rs`；`<dir>/<name>.i.<ext>` 的目录内形态由候选 1 一并覆盖）；**文件型根**下它是根的兄弟文件（`views/welcome-view.i.ts`），与 `path` 全名（`views/welcome-view.tsx`）既不相等也不互为前缀，只有候选 3 能命中；候选 2 保留为既有形态（`src/dir.ts` 根对 `src/dir.i.ts`）不做删除。判据改造 SHALL 只此一处（`OwnerIndex` 的剥离函数与其调用点），`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不动，既有非 `.i` 路径的归属语义零变化。
- **并集 MUST NOT 实现为候选短路**：命中集 SHALL 为「对每个候选各跑一遍最长前缀后合并去重」，再按前缀长度定档（同长仍归 `Ambiguous`）；MUST NOT 写成「候选 1 命中即返回」——那会让 `src/dir.i.ts`（候选 1 全等 `src/dir`、候选 2 得 `src/dir.ts`）只走一条路径，与并集语义不符。定档用例：元素根 `src/dir/` 与 `src/dir.ts` 同时在场、探针 `src/dir.i.ts` ⇒ 归更具体的单文件根（前缀更长者胜出）；`src/a.ts` 与 `src/a.tsx` 同时在场、探针 `src/a.i.ts` ⇒ 两文件型根的主干同长，归 `Ambiguous`（MUST NOT 任选）。
- **白名单外后缀零误剥**：`.i.py` / `.d.rs` / 大写 `.I.rs` / 无扩展名 `.i` SHALL 原样返回单个候选（不剥），归属与改动前逐值相同。
- **父子路径重叠不缩窄父契约**：子元素根路径是父元素根路径的前缀，最长前缀匹配已正确处理（`.../src/scan/x.rs` 归 `Scan`、`.../src/dto.rs` 归 `Archi`）。
- **无可用入口即 `apiEntryMissing`**：`EntryState` 为 `Absent` 或 `Invalid(_)` ⇒ 该元素不是可用模块 → 报一条 `apiEntryMissing`（定义与档位见 `desktop-archi-api-check`）。它比 `apiMissing` 更基础——**在契约生成之前就失效**，防止给不存在的模块写契约。`.i` 生成物的存在 MUST NOT 再当作入口的替代证据：生成物路径是入口的函数，无入口即无生成物路径可派生。

#### Scenario: 入口解析链三态各命中一条

- **WHEN** 分别对「显式声明 `entry = "packages/desktop/src-tauri/src/lib.rs"` 的壳元素」、「根为文件型 `views/welcome-view.tsx` 的元素」、「根为目录型 `crates/infra/archi/` 且盘上有 `src/lib.rs` 的元素」取 `entry_state`
- **THEN** 三者均为 `Resolved`，入口分别为声明的路径、根文件自身、`crates/infra/archi/src/lib.rs`；再对根为目录型而 `index.ts` / `index.tsx` 皆不在盘的前端目录元素取同一函数 → `Absent`（无入口）；四例均不 panic、`Absent` 一例不产生成物路径

#### Scenario: 四条判据各一反例

- **WHEN** 分别构造四个反例：`entry = "/abs/x.rs"`（绝对路径）、`entry` 指向另一个元素的 `path` 内的文件、`entry` 指向不存在的文件、两个元素声明同一入口文件；再补一例「文件型根元素声明同目录的另一个文件」
- **THEN** 依次为 `Invalid(Malformed)` / `Invalid(OutsidePath)` / `Invalid(NotOnDisk)` / 双方各 `Invalid(Duplicated)` / `Invalid(OutsidePath)`；五例各自的 `apiEntryMissing` 文案点名具体理由（越界目标 / 缺失文件 / 冲突方 FQN），MUST NOT 只写通用「无入口」

#### Scenario: 生成物落入口同级且 #[path] 无目录段

- **WHEN** 对 crate 形态（入口 `crates/infra/archi/src/lib.rs`）、子目录模块形态（入口 `crates/core/archi/src/scan/mod.rs`）、文件型根（入口 `views/welcome-view.tsx`）分别取 `paths::generated_path` 与入口行字面量
- **THEN** 三者落点为 `crates/infra/archi/src/lib.i.rs`、`crates/core/archi/src/scan/mod.i.rs`、`views/welcome-view.i.ts`（= 各自 `<入口主干>.i.<后缀>`，与入口同目录）；`#[path]` 字面量逐字等于 `generated_path(...).file_name()`（`"lib.i.rs"` / `"mod.i.rs"`），含 `../` 或目录段的形态即判红；`paths::mod_path_attr` 在产品源码内零命中（函数已删）

#### Scenario: 三种落点形态各归其元素

- **WHEN** 分三组求归属：① 入口同级的目录型根落点（`crates/infra/archi/src/lib.i.rs` 对目录形前缀 `crates/infra/archi/`）；② 目录内形态（`views/architecture/architecture.i.ts`、`crates/core/archi/src/scan/scan.i.rs`）；③ 入口同级的文件型根落点（`views/welcome-view.i.ts` 对文件形前缀 `views/welcome-view.tsx`、`components/agent/index.i.ts` 对容器 barrel 前缀 `components/agent/index.tsx`）
- **THEN** 三组用例各归其元素（①③ 由整组剥离 + 主干对齐命中、② 由整组剥离 / 替换扩展名命中，三者并集）；非 `.i` 路径（如 `crates/core/archi/src/dto.rs`）归属结果与改造前逐字相同

#### Scenario: 并集定档不短路

- **WHEN** 元素根 `src/dir/` 与 `src/dir.ts` 同时在场，探针为 `src/dir.i.ts`；另一组为 `src/a.ts` 与 `src/a.tsx` 同时在场、探针 `src/a.i.ts`
- **THEN** 前者归更具体的单文件根（`src/dir.ts` 所在元素，前缀更长）；后者两文件型根主干同长 ⇒ `Ambiguous`（MUST NOT 任选）；若实现成「候选 1 命中即短路」，前者会误归目录形根——该差异即本 Scenario 的判红面

#### Scenario: 白名单外后缀不误剥

- **WHEN** 对 `src/mod/mod.i.py`、`src/mod/mod.d.rs`、`src/mod/mod.I.rs`、`src/mod/mod.i` 求归属（元素根 `src/mod/`）
- **THEN** 四者按原路径判定归属（不剥），结果与改动前逐值相同；`.i.<白名单外后缀>` 不产生第二 / 第三个候选

#### Scenario: 本仓入口面实测命中

- **WHEN** 对本仓 42 个 `path` 非空元素跑 `entry_state`
- **THEN** `Resolved` **27** 条（14 个 crate 由 ③ 命中 `src/lib.rs` + 多文件根 `App.Frontend.Shell` 由 ② 命中 `app.tsx` + 12 个前端单文件根由 ② 命中自身 = 14 + 1 + 12）；`Absent` **15** 条（8 个视图目录根 + 3 个视图内子域 + `components/ui` 缺 `index.ts`，共 12 条；+ 壳 `App.Backend.Shell`（约定候选 `src-tauri/src/src/lib.rs` 不在盘）+ 两个工具 crate（二进制 crate 无 `src/lib.rs`），共 3 条——三者的显式 `entry` 数据归宿主重建后的后续变更，届时 `Resolved` 30 / `Absent` 12）；`Invalid` 0 条。`App.Frontend.Shell` 因此不再落 `apiEntryMissing`；壳与两个工具 crate 仍落（`Absent`），MUST NOT 记为已解决

#### Scenario: 生成物与手写 impl 共存编译

- **WHEN** 入口文件 `crates/infra/archi/src/lib.rs` 写 `#[path = "lib.i.rs"] mod api_gen; pub use api_gen::LocalArchiCheck;`，同级生成物 `crates/infra/archi/src/lib.i.rs` 含 `pub struct LocalArchiCheck;` + `impl` 内 `new()`，手写文件另有 `impl Default for LocalArchiCheck`
- **THEN** 编译通过（`rustc --edition 2021`），外部可经入口路径引用 `LocalArchiCheck::new()` 且签名受生成物约束

#### Scenario: 同名冲突不静默覆盖

- **WHEN** 生成物定义了元素内已手写的同名类型且入口 `pub use api_gen::*;`
- **THEN** 编译器报 E0255（重复定义），MUST NOT 静默采用其一

#### Scenario: 路径形态非法即拒收（安全边界）

- **WHEN** 元素 `<root>`、入口候选或契约 FQN 取绝对路径 / 含盘符 / 含 `..` 段 / 含空段 / 为空串（如 `path = ["/abs/elsewhere"]`、`path = ["crates/../../outside"]`、`entry = "../x.rs"`、`fqn = "App..X"`）
- **THEN** `contract_path` / `schema_path` / `generated_path` / `entry_state` 一律返回空或 `Absent`（`Malformed` 形态归 `Absent` / `Invalid`），不 panic、不落到仓外；`root.join(绝对路径)` 的「替换 base」与 `..` 的「上溯」因此结构上不可达——「证据在盘」的探测不会变成任意文件读，工具 crate 的写盘不会变成任意文件写。**第二层**：`archi-api-gen` 的写盘函数 SHALL 另以目标须落在观察根之下（`starts_with(root)`）复核，越界即拒绝并零副作用（两层判据交叉验证，任一层失效另一层仍拦得住）

### Requirement: 生成物入仓与零 diff 门挂载

四类生成物 SHALL 全部入仓并进零 diff 门，形态沿既有 `bindings.ts` 先例（MUST NOT 另造第二套守卫机制）：

| 生成物 | 落点 | 入库 | 门禁 |
|---|---|---|---|
| `<FQN>.schema.json` | `<archi_api_dir>/<Domain>/` | 是 | `api:check` 零 diff |
| `INDEX.toml` | `<archi_api_dir>/` 根（全局唯一一份） | 是 | `api:check` 零 diff |
| `<入口主干>.i.rs` | **元素实现入口同级**（`crates/infra/archi/src/lib.i.rs`） | 是 | `api:check` 零 diff + `cargo check` |
| `<入口主干>.i.ts` | **元素实现入口同级**（`views/welcome-view.i.ts`） | 是 | `api:check` 零 diff + `tsc --noEmit` |

- **`INDEX.toml` 的 `generated` 列取入口同级路径**：无入口（`Absent` / `Invalid`）的元素该列留空——生成物路径是入口的函数，无入口即无路径可写。
- **脚本面封闭两条**：`api:export`（`cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`，写盘）与 `api:check`（同一调用加 `--check`；生成到内存后比对、**零写入**，差异即非零退出；比对由工具内完成，**不依赖任何 `git diff` 段**）。`check` 链 SHALL 由三段扩为四段：`server:check && bindings:export && api:check && client:check` —— 该扩展只加一段（`api:check` 插在第二段之后）；`bindings:export` 与 `client:check` 两段命令形态零变化，`server:check` 的 fmt 段 SHALL 由写模式 `cargo fmt --all` 改为校验模式 `cargo fmt --all --check`（写模式门使工作树无法停在不动点，故本变更 MUST 改它；clippy 段逐字不动）。**第二段 SHALL 是 `bindings:export`**（`check` 的生成物零 diff 守卫已退役，见 `desktop-ipc-type-bindings`——`check` 只前置重导出、不比对）；规范内 MUST NOT 引用任何已退役的守卫脚本名；段名 SHALL 以 `package.json` 的 `scripts` 实存键为准（`grep -c '"bindings:export"' package.json` ≥ 1 即证该键在案）。
- **应用内两驱动面与脚本面同源**：生成物的渲染与三类差异比对 SHALL 各有唯一实现（`core/archi` 的渲染 / 比对纯函数），写盘 SHALL 有唯一实现（`infra/archi` 的写盘单点）；应用内两条 IPC（检查 / 生成，见 `desktop-archi-contract-panel`）与 CLI 两子命令 MUST NOT 各写一份渲染、比对或写盘——**四条消费路径共用同一实现**是「零 diff 门」与「应用内检查」结论一致的前提。脚本面两条命令形态本文逐字不变（仅写盘实现的落点变化，命令串不变）。
- **`knip` 边界**：`knip.json` 的 `ignore` SHALL 增 `src/**/*.i.ts`（生成物非手写代码；实测未被 import 的 `.i.ts` 会被报为 unused file，因 `project` glob 覆盖 `src/**/*.ts`）；MUST NOT 新增 `ignoreExportsUsedInFile` 或 `ignoreDependencies` 条目（那是关判据而非满足判据）。
- **类型绑定再生成**：`ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` 与报告字段 `apiFindings` SHALL 经 `bindings:export` 进 `bindings.ts` 与 `bindings.zod.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役）；本能力 MUST NOT 手写前端类型镜像。
- **TS 侧形状校验的机制前提**：实现侧 SHALL `import type` 引用生成的 `.i.ts` 并标注返回类型（否则校验不发生）；`isolatedModules` 下重导出须写 `export type { … }`。
- **版本纪律**：本变更改 `packages/desktop` 源码 ⇒ `package.json` 的 `version` SHALL 相对**落地时 `HEAD` 的现值 +1 patch**（相对递增，规范内 MUST NOT 写死具体版本号——现值随每次源码改动滚动，写死即失效）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

#### Scenario: 四类生成物齐入仓且零 diff

- **WHEN** 执行 `api:export` 后再执行 `api:check`
- **THEN** 四类生成物均在盘且已在 git 内，第二次检查零差异、退出码 0；`git status` 干净

#### Scenario: 手改任一生成物即被拦

- **WHEN** 手改已提交的 `<FQN>.schema.json`、`INDEX.toml`、`.i.rs` 或 `.i.ts` 任一后执行 `api:check`
- **THEN** 检查以非零退出并列出被改文件（`--check` 阶段无写盘）；回退后恢复通过

#### Scenario: 落点迁移后旧位置必须清空

- **WHEN** 落点从元素根同级改到入口同级后，旧文件 `crates/infra/archi.i.rs` 仍在盘
- **THEN** `api:check` 的 `orphaned` 列报出该文件并以非零退出（「应删未删」按命名判据检出，与落点深度无关）；删除后 `orphaned` / `missing` / `mismatched` 三列皆空

#### Scenario: check 链扩一段且 fmt 段改校验模式

- **WHEN** 审查 `package.json` 的 `check` 脚本与既有 `server:check` / `bindings:export` / `client:check` 三段
- **THEN** `check` 为四段，逐字为 `pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check`（新增 `api:check` 位于 `bindings:export` 与 `client:check` 之间）；`bindings:export` 与 `client:check` 两段命令原文逐字不变；`server:check` 的 fmt 段为 `cargo fmt --all --check` 且其 clippy 段逐字不变；`client:check`（含 knip）在 `src/**/*.i.ts` 进 ignore 后仍零 dead code 报告

#### Scenario: 应用内检查与脚本面结论一致

- **WHEN** 在同一观察根分别调用 `archi_api_check`（IPC）与 `api:check`（CLI）
- **THEN** 两者报出的三类差异文件集合逐条相等；随后用 `archi_api_generate`（IPC）生成，再跑一次 `api:check` 得零差异——四条消费路径（两 IPC / 两 CLI 子命令）零第二份渲染、比对或写盘实现

### Requirement: 仓库根文档的契约纪律节

仓库根 `AGENT.md` SHALL 承载一节 `## 模块 API 契约` 作为 agent 侧的操作纪律入口，内容 SHALL 限于**指针与最简规则**，MUST NOT 复制契约正文、判据长表或 fenced json 示例：

- 该节 SHALL 恰含五条要点：① 手写真相位置 `<archi_api_dir>/<Domain>/<FQN>.md` 与三类 fence 的名字（`api-op` / `api-types` / `api-reexport`）；② 四类生成物勿手改（`.schema.json` / `INDEX.toml` / **实现入口同级**的 `<入口主干>.i.rs` / `<入口主干>.i.ts`）与「已提交手改即零 diff 门红、未提交即被静默覆盖」；③ `api:export` / `api:check` 两脚本与 `check` 链的段序；④ `api = none` 是显式留白而非「还没写」；⑤ **契约在案元素**的判据进 `status`（可把状态词抬到 `warnings`）、不进 `passed`，索引与计数经 `INDEX.toml` 查看，两轴经架构页「契约」视图一眼看全。
- 第②条的**方位词与文件名形态 SHALL 与「生成物落点与入口查找」的落点口径一致**：「实现入口同级」+ `<入口主干>.i.<后缀>`（`crates/infra/archi/src/lib.rs` → `lib.i.rs`）。该条只说方位与命名，MUST NOT 复述入口解析链、四条判据或 `entry` 字段知识（字段面归 `desktop-archi-model`，agent 侧字段清单归 `desktop-archi-agent` 的知识文本）——文档与 spec 双写必然漂移。本条同步 MUST NOT 增删要点条目（条目数恒 5，`entry` 不得成为第六条）。
- 该节 MUST NOT 含契约示例正文、`ArchiApiFindingKind` 的十值长表或判据定义（定义归本能力与 `desktop-archi-api-check`）——文档与 spec 双写必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制即既有反例）。
- 该节 MUST NOT 出现 `openspec/specs/**` 路径（spec 语料非 agent 的契约操作面），且 MUST NOT 把契约路径写进模型元素 `docs`（见 `desktop-archi-module-docs`）。
- `CLAUDE.md` SHALL 同步同一节且与 `AGENT.md` 保持既有同源纪律：两文件该节正文逐字相同，全文件差异 SHALL 限于节外既有差异（标题行、首行说明行、文件末尾换行）；**该节自身的两份副本若分化即违反本条**。

#### Scenario: 纪律节只指路不带正文

- **WHEN** 审查 `AGENT.md` 的 `## 模块 API 契约` 一节
- **THEN** 该节恰有 5 条要点、`grep -c '^## 模块 API 契约' = 1`；`grep 'apiEntryMissing'` 在该节零命中（判据长表未复制）；节内零 `openspec/specs/` 路径；节内无 fenced json 代码块

#### Scenario: 两份副本不分化

- **WHEN** 对 `AGENT.md` 与 `CLAUDE.md` 做 `diff`
- **THEN** 差异恰 2 处且全在 `## 模块 API 契约` 节外（`:1` 标题、`:3` 首行说明）；该节在两文件各恰一处且节内正文零 diff 行（逐字相同）

#### Scenario: 纪律节第 ⑤ 条与纳管口径一致

- **WHEN** 对照该节第 ⑤ 条与 `desktop-archi-api-check`「报告并列槽位 apiFindings」的纳管条款
- **THEN** 两处都表达「契约在案元素的判据影响 `status`、不影响 `passed`」，MUST NOT 残留「不进 `status`」一类旧口径

#### Scenario: 第 ② 条方位词与真实落点一致

- **WHEN** 审查 `AGENT.md` 与 `CLAUDE.md` 该节第 ② 条，并对照入口同级的实盘生成物 `crates/infra/archi/src/lib.i.rs`
- **THEN** 该条写「实现入口同级」且文件名形态为 `<入口主干>.i.rs` / `<入口主干>.i.ts`；`grep '元素根同级'` 与 `grep '<root>.i'` 在两文件该节零命中；该节要点数仍恰 5（措辞同步 MUST NOT 增删条目、MUST NOT 出现 `entry` 字段说明）；两文件全文 `diff` 仍恰 2 处且全在节外

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/paths.rs` | 契约与生成物路径单点 | `normalize_root` / `module_name(element_root)`（Rust / TS 透传命名唯一产出点，**MUST NOT** 参与生成物文件名）/ `rust_module_path(element_root)`（Rust 透传模块路径唯一产出点，MUST NOT 由入口主干互推——`archi-runtime` / `archi` / `lib` 三名互不相等）/ `domain_of(fqn)` / `directory(root)`（`archi_api_dir` 唯一转发点）/ `index_path(root)` / `language_of(file)` / `generated_kind(language)`（语言分派单点，输入改**入口**扩展名）/ `contract_path(root, fqn)` / `schema_path(root, fqn)` / `entry_candidates(element) -> Vec<String>`（**纯文本、零 IO**，三态解析链的唯一产出点）/ `EntryState` + `EntryProblem` + `entry_state(root, elements, element)`（首个落盘候选 + 四条判据的唯一判定点，读盘只在此）/ `EntryForm` 三值（`Crate` / `SubdirModule` / `SingleFile`）由「入口与根的关系」派生（入口 == 根 → `SingleFile`；入口以 `src/lib.rs` 收尾 → `Crate`；其余 → `SubdirModule`），只供 `EmitTarget.form`（TS 侧形态位，本期零 `.i.ts` 实例）/ `generated_path(root, entry, kind) -> Option<PathBuf>`（入参是**入口**而非元素根，产出 `<入口目录>/<入口主干>.i.<ext>`；无入口即 `None`）/ `ts_import_specifier(form, name)`（**待接线留白**：TS 侧入口同级规则的真值尚待前端契约变更实现，MUST NOT 当作既有真值）/ `ref_relative(from_fqn, to_fqn)`；**已删** `mod_path_attr`（`#[path]` 值恒为入口同级文件名，等值性由 `paths_test` 直接断言）；全经 `archi_api_dir` 派生，零行内 `api` 段与零手算相对路径；**四类路径派生 MUST 做路径形态校验**（FQN / 元素 `<root>` / 入口非 repo 相对形态——绝对路径 / 盘符 / `..` 段 / 空段 / 空串——即 `None` / `Malformed`），否则 `Path::join` 会替换 base 或上溯，把在盘探测与工具写盘带出仓库；末段进目标语言源码时的形态闸按语言分列（Rust 由 `is_module_segment` 判「标识符且非关键字」，TS 零筛选走 `ts_string_literal` 转义） |
| `crates/core/archi/src/api/paths_test.rs` | 路径面语料 | 解析链三态（显式 / 文件型根 / 约定默认 / `Absent`）+ 四条判据各一反例 + 单文件根声明别的文件 → `OutsidePath`；`generated_path` 三形态落入口同级；`#[path]` 字面量 == `generated_path().file_name()` 自证；`mod_path_attr` 零命中（已删）；`ts_import_specifier` 既有两值在案并标注待接线 |
| `crates/core/archi/src/api/mod.rs` | 契约面装配与出口 | 出口恰八行：`meta()` / `enabled(root)` / `load_contracts(root, elements)` / `render_all(root, snapshot)` / `bootstrap(root, element)` / `check(input)` / `api_ref` / `api_ref_of`；`render_all` / `generated_path_of` / `index_entries` / `element_symbols` / `bootstrap` 取 `paths::entry_state` 的 `Resolved` 入口（`path.first()` 退出落点面）；无可用入口的元素不产 `.i.*`、`INDEX.toml` 的 `generated` 留空；纯读盘、零写盘、零 spawn；`index_entries` 的 `degraded` 经 `schema::degraded_marks`（零第二套遍历） |
| `crates/core/archi/src/api/check.rs` | 十条判据单点求值 | ① 改经 `entry_state`：`Absent` / `Invalid(_)` ⇒ 一条 `apiEntryMissing`（文案按 `EntryProblem` 分列理由），`Resolved` ⇒ 继续 ②③④；**已删** `has_generated`（`.i` 的存在不再是入口的替代证据）；其余九条与求值互斥序逐字不变 |
| `crates/core/archi/src/api/check_test.rs` | 判据语料 | `apiEntryMissing` 四理由各一条；`Resolved` 元素不报 ①；`api = none` + `entry` 有值 ⇒ 零判据；`.i` 在盘但无入口仍报 ① |
| `crates/core/archi/src/scan/owner.rs` | 生成物归属（判据修正） | 剥离候选集由「双形态」扩为**三候选**：整组剥离 ∪ 替换扩展名 ∪ **主干对齐**（`<name>` 与文件型 `path` 的去扩展名形态全等）；`owner_of` 对每候选各跑一遍最长前缀、按 `(fqn, 前缀长度)` 合并去重后定档（同长仍 `Ambiguous`），**MUST NOT 候选短路**；`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不变；非 `.i.<ext>` 路径归属零变化 |
| `crates/core/archi/src/scan/owner_test.rs` | 归属语料 | 三候选各正例（`crates/infra/archi/src/lib.i.rs` / `views/welcome-view.i.ts` / `components/agent/index.i.ts` / `src/dir.ts` 根对 `src/dir.i.ts`）；`src/a.ts` 与 `src/a.tsx` 同场 ⇒ `Ambiguous`；白名单外四条反例与 `src/dir.i.ts` 并集定档用例保持绿 |
| `crates/core/archi/src/api/diff.rs` | 三类差异比对 | 逐字不变（`orphaned` 按**命名**判据 `*.i.rs` / `*.i.ts` 与落点深度无关，故落点迁移无需改判据）；注释中的旧落点样例同步为入口同级 |
| `crates/core/archi/src/api/emit/mod.rs` + `emit/{rust,ts}.rs` | 渲染面 | 逐字不变（`EmitTarget.form` 仍由调用方给、渲染器零形态推导）；`EntryForm` 三值保留供 TS 侧形态位；`ts_import_specifier` 的接线归前端契约变更（显式留白） |
| `crates/core/archi/src/model.rs` / `parse/toml.rs` / `dto.rs` | 字段面（定义与解析见 `desktop-archi-model`） | `Element.entry` / `RawElement.entry` / `ArchiElement.entry: Option<String>`（缺省 `null`，显式字段不省略）；`deny_unknown_fields` 面内，拼错的字段仍 `unknownField` |
| `crates/infra/archi/src/lib.rs` + `src/lib.i.rs` | 入口编排与生成物（落点迁移样本） | 入口行 `#[path = "lib.i.rs"] mod api_gen; pub use api_gen::*;`（值 = 入口同级文件名，零 `../`）；生成物在入口同级、内容逐字不变；旧 `crates/infra/archi.i.rs` 已删（否则 `api:check` 的 `orphaned` 非空） |
| `.dev-team/architecture/api/**` | 契约与生成物资产 | `<Domain>/<FQN>.md`（唯一手写）+ `<Domain>/<FQN>.schema.json`（生成）+ `INDEX.toml`（生成）；本变更只改 `INDEX.toml` 的 `generated` 一行（`ArchiRuntime` 改 `packages/desktop/src-tauri/crates/infra/archi/src/lib.i.rs`），41 条其余条目与全部 `.md` / `.schema.json` 逐字不变 |
| `AGENT.md` / `CLAUDE.md`（仓库根文档） | 契约纪律节（措辞同步面） | `## 模块 API 契约` 第②条 = 「实现入口同级」+ `<入口主干>.i.rs` / `<入口主干>.i.ts`（与「生成物落点与入口查找」同口径）；条目数恒 5、其余四条要点与节内其余条款逐字不动；两文件该节正文逐字相同（全文差异仍恰 2 处且全在节外）；零 `entry` 字段说明（字段知识归 `desktop-archi-agent` 的知识文本） |
| `packages/desktop/package.json`（脚本面） | 生成与门禁驱动 | `api:export` / `api:check` / `check` 三段命令串逐字不变；`version` 相对落地时 `HEAD` 现值 +1 patch（`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动） |
