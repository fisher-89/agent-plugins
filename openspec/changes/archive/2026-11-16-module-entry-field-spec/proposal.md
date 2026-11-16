# 提案: module-entry-field-spec

> **变更**: module-entry-field-spec（主线变更 A · 模块入口字段与接口文件落点）
> **日期**: 2026-11-15
> **状态**: draft
> **来源**: `openspec/changes/module-entry-field-spec/explore.md`（requirement 类型；主线 `openspec/explores/module-entry-declaration.md` 的第一单元）
> **性质**: 模型新增 `entry` 字段 + 入口解析链三态 + 生成物落点改为**入口同级** + 归属判据跟上落点。零新增判据种类、零新增 IPC 命令、零契约资产（`.dev-team/architecture/api/**/*.md`）内容改动；改动集中在 `crates/core/archi` 的模型/路径/判据面、一份生成物搬迁、**两处模型数据的 `api = none` 留白**（`entry` 数据分两批：本变更零声明，三处显式值随宿主重建后的后续变更补入，见 §六）、一份知识文本与仓库根文档契约纪律节的一处措辞。
> **不含**: 前端 barrel（变更 B）、TS 取数面与 TS 透传相对路径真算（变更 C）、契约格式补丁（变更 D）、模块入口文档（变更 E）。

---

## 问题

### 现状链路（读过的真实代码）

```text
元素 path ──path.first()──┬── generated_path(<root>) → <root>.i.<ext>   (元素根同级)
  (D42)                   └── entry_form(<root>)     → 七候选探测         (Rust 3 + 前端 4)
                                 └─ mod_path_attr(form, name) → "../../<name>.i.rs" / "../<name>.i.rs" / "<name>.i.rs"
check.rs ①  apiEntryMissing ⟸ entry_form().is_none() && !has_generated(<root>.i.*)
```

三个真实病灶，共同根因一句话：**「入口」这个事实在模型里无处可写**，于是它只能被探测、被猜，猜不到就被判成「没有」。

### 病灶一 · 入口靠探测，只在一种形态上有效

`paths::entry_form`（`api/paths.rs:242-300`）的七候选对 42 个 `path` 非空元素实测只命中 **14** 个（core 7 + infra 7 个 crate 的 `src/lib.rs`）。其余 **28** 个**全部**落 `apiEntryMissing`，但它们的真实成因分四类，报告里看不出区别：

| 形态 | 元素数 | 探测为什么失败 | 真相应为 |
|---|---|---|---|
| 前端目录根（`views/agent/` 等） | 12 | 全仓零 `index.ts(x)`（唯一 barrel 是 `components/agent/index.tsx`，且它是**文件级** `path`） | **无入口**（缺门，变更 B 补 barrel） |
| 前端单文件根（`views/welcome-view.tsx`、`components/agent/*.tsx`、`standard/*.tsx`） | 12 | 探测在 `path` 已带扩展名的根上再拼 `.tsx`（`welcome-view.tsx.tsx`），必不命中 | **入口 = 根文件本身**（今天是假报） |
| 多文件根（`App.Frontend.Shell` 的 4 个文件） | 1 | `path.first()` 是 `app.tsx`，探测在文件型根上再拼 `.tsx` / `index.ts`，必不命中 | **入口 = `app.tsx`**（即 `path.first()` 自身） |
| 壳 `src-tauri/src/` | 1 | 三条候选拼成 `src-tauri/src/src/lib.rs` 等，皆不存在 | **`src-tauri/src/lib.rs`**（该文件真实存在） |
| 工具 crate（只有 `main.rs`） | 2 | 二进制 crate 无 `src/lib.rs` | **`src/main.rs`** 是事实，但「公共面 = CLI」⇒ `api = none` |

也就是说：**今天 28 条 `apiEntryMissing` 里只有 12 条是真话**（前端目录根确实没有门），另 16 条是探测能力不足的噪声（12 个单文件根 + 1 个多文件根 + 1 个壳 + 2 个工具 crate——它们的入口事实本来都存在）。

### 病灶二 · 落点由 `path.first()` 决定，与入口脱钩 ⇒ `#[path]` 只能写 `../`

生成物落 `<root>.i.rs`（元素根同级），而 `#[path]` 的值是**字面量**（Rust 属性值不能由函数求值），于是 `paths::mod_path_attr` 必须按入口形态产出三种带 `../` 层数的串（`"../../archi.i.rs"` / `"../archi.i.rs"` / `"archi.i.rs"`），并靠 `paths_test` 三形态断言 + 逐入口自查行双守。一处间接就多一处漂移面：入口一改位置，生成物落点与 `#[path]` 值两个事实要各改一遍。

### 病灶三 · 「一个入口只属于一个模块」这条原则无处校验

`entry` 不可声明 ⇒ 「入口必须落在本元素 `path` 内」「入口必须在盘上」「两个元素不得声明同一入口」三条本来最基础的模块边界判据，代码里一条都不存在。当前唯一的替代证据是 `has_generated`（`.i` 存在即当作有入口）——它是**生成物的存在**冒充**入口的存在**，因果倒置。

### 与 explore 的三处事实勘误（本提案按代码事实落，MUST NOT 沿用错数）

| 位置 | explore 现文 | 实测 | 依据 |
|---|---|---|---|
| A.10.1 / A.8 | 今日 `apiEntryMissing` = **27** | **28** | `42 − 14（有入口的 crate）= 28`；explore 在 42 已减 14 后又减了「ArchiRuntime 生成物免报」一次，而 `ArchiRuntime` 本来就在那 14 个里有入口的元素中（`crates/infra/archi/src/lib.rs` 存在） |
| A.8 第 5 条 | 「`App.Backend.Shell` 不再报 `apiEntryMissing`」是 A 的**唯一**可见收益 | 不止一条：12 个前端单文件根元素**同时**由 `apiEntryMissing` 转为有入口（并因此承担「有门无契约」→ `apiMissing`，见 AC-4） | A.1.3 的「单文件根元素：入口 = 根文件本身」一旦实现，`views/welcome-view.tsx` 一类必命中 |
| A.10.2 第 2 条 | `scan/owner.rs` 可出 range（归属判据零改动） | **不可**：该条的反例用的是目录型根；文件型根的入口同级生成物（`views/welcome-view.i.ts` 对文件型 `path` `views/welcome-view.tsx`）在「整组剥离 ∪ 替换扩展名」两候选下都不命中（`welcome-view` ≠ `welcome-view.tsx`，`welcome-view.ts` ≠ `welcome-view.tsx`）⇒ 必须补第三候选 | `scan/owner.rs:183 strip_candidates` + `owner_of` 的最长前缀判据 |

---

## 提案

### 一 · 模型字段 `entry`（单值，四条判据）

新增可选字符串字段 `entry`，语义四则（与 explore A.1.1 一致）：

| 判据 | 措辞 | 违反的后果 |
|---|---|---|
| 基准 | repo 相对 POSIX 路径，与 `path` 同口径（`./` 前缀由 `normalize_root` 归一、不参与身份） | `EntryProblem::Malformed` |
| 落位 | MUST 等于某条 `path` 条目，或落在某条**目录型** `path` 之内 | `EntryProblem::OutsidePath`（单文件根元素声明「别的文件」也落此桶） |
| 存在 | 声明了就 MUST 在盘上存在 | `EntryProblem::NotOnDisk` |
| 跨元素 | 任何两个元素 MUST NOT 声明同一入口文件 | `EntryProblem::Duplicated`（双向各报一条） |

- **单值而非数组**：数组让「唯一」不可表达；`path` 已是数组、`entry` 只回答「门在哪一扇」。
- **不进结构化诊断**（不改 `DiagnosticKind` 九值、不停摆画布）：四条判据的违反走 `apiFindings` 面，与 `apiEntryMissing` 同族、`warn` 档、只报不拦（explore A.1.6 第 2 条）。

### 二 · 入口解析链三态（候选派生零 IO / 落盘确认与校验单点）

```text
entry_candidates(element) -> Vec<String>           纯文本、按优先序、零 IO
  ① 显式 entry（归一后）           → [entry]                     唯一候选
  ② 文件型根（末段扩展名 ∈ {rs,ts,tsx,js,jsx}）→ [<root>]         根文件即模块
  ③ 目录型根（语言无关，按序取首个落盘者）
                                   → [<root>/src/lib.rs, <root>/mod.rs, <root>/index.ts, <root>/index.tsx]
  ④ 其余（path 空 / 形态非法）     → []

entry_state(root, elements, element) -> EntryState  读盘 + 模型集
  Resolved(<form>, <entry>)  首个落盘的候选，且落位 / 跨元素唯一两条判据通过（form 由「入口与根的关系」派生）
  Invalid(<problem>)  显式候选不在盘 / 候选落在 path 外 / 与另一元素同一入口
  Absent              候选为空（无声明且无约定默认），或目录型根的默认候选全不落盘
```

- **`<root>` 仍是 `path.first()`**（D42 不动）：多文件根元素（全仓唯一 `App.Frontend.Shell` 的 4 个文件）天然被 ② 命中为 `app.tsx`，MUST NOT 为此改模型或引入「相对某个 root」的第二套相对性。
- **约定默认只剩语言约定形态**（R1，explore A.1.6 第 1 条）：**不保留** R2（「目录内顶层非测试源文件恰 1」）——它要列目录，且「新增一个 `helper.ts` 就换默认入口」正是本次要消灭的漂移源。③ **不做语言判定**：目录型根没有扩展名（`language_of` 对它恒为 `Unknown`），若按语言分派，前端目录根就永远拿不到 `index.ts(x)` 候选（变更 B 的 barrel 将全部失效）；四种形态互不相交，一律列出、按序取首个落盘者即可。
- **默认候选比今天少三条**：去掉 Rust `<dir>.rs` 与前端 `<dir>.ts` / `<dir>.tsx`——这三条对**目录型**根只能指到 `path` 之外的兄弟文件（必违落位判据），实测零命中；文件型根由 ② 覆盖，无须它们。
- **「无入口」是合法终态**：`ui`（ADR 0001 刻意收敛的层）永远不会有入口，`api = none` 不能冒充它（那对视图是假话）。`Absent` ⇒ 一条 `warn` 级 `apiEntryMissing`，不进 `status`。

### 三 · 生成物落点改到**入口同级**

| 项 | 今天 | 本变更后 |
|---|---|---|
| Rust 生成物 | `<root>.i.rs`（`crates/infra/archi.i.rs`） | `<入口主干>.i.rs`（`crates/infra/archi/src/lib.i.rs`） |
| 前端生成物 | `<root>.i.ts`（`views/welcome-view.tsx.i.ts`） | `<入口主干>.i.ts`（`views/welcome-view.i.ts`） |
| `#[path]` 值 | `../../<name>.i.rs` 三形态 | **入口同级的文件名**（`"lib.i.rs"`），恒无 `../` |
| 值产出 | `mod_path_attr(form, name)`（唯一直值源 + 双守断言） | **删函数**：值 = `generated_path(entry).file_name()`，等值性由 `paths_test` 直接断言（explore A.3／A.11「删函数、留断言」，与 D40/D41「零二次派生」同族） |
| 语言分派 | `language_of(<root>)`（无扩展名 → Rust 兜底） | `language_of(<入口>)`（入口恒带扩展名，`Unknown` 兜底保留） |

唯一同时拿到「名字可读 + 落入口同级 + `#[path]` 无 `../`」三个收益的读法（explore A.2 签字项 i）。`EntryForm` 三值保留（`Crate` / `SubdirModule` / `SingleFile`，供 TS 侧形态位），派生改为「入口与根的关系」：入口 == 根 → `SingleFile`；入口以 `src/lib.rs` 收尾 → `Crate`；其余 → `SubdirModule`。

### 四 · 归属判据跟上落点（第三候选）

```text
候选 1  整组剥离      <name>.i.<ext> → <name>           （目录型根命中：crates/infra/archi/src/lib.i.rs）
候选 2  替换扩展名    <stem>.i.<ext> → <stem>.<ext>     （保留：`src/dir.ts` 形态的文件型根）
候选 3  主干对齐      <name>.i.<ext> → <name> 与**文件型 path 的去扩展名形态**全等
                                                      （views/welcome-view.i.ts ↔ views/welcome-view.tsx）
命中判据、并集去重、最长档定档、`Ambiguous` 语义、非 `.i` 路径零变化 —— 全部逐字沿用
```

第三候选是落点变更的**必要配套**：没有它，文件型根的生成物会变成 `Owner::None`（漂移 `unmatchedFiles` + 文件树 `none` 档），而变更 C 一旦产出 `.i.ts` 就会立刻暴露。本仓零 `.i.ts` 实例 ⇒ 本变更内无可观测差异，但判据必须先正确。

### 五 · 判据面语义（`apiEntryMissing` = 「无可用入口」）

- `apiEntryMissing` 的定义改为：**该元素没有可用入口** = 无候选（未声明且无约定默认）**或**候选被判 `Invalid`（形态 / 落位 / 存在 / 唯一任一不成立）。文案按理由分列，MUST NOT 只写「无入口」了事。
- **删 `has_generated`**：`.i` 的存在不再当作入口的替代证据（因果倒置），也不能——生成物路径现在是**入口的函数**，无入口即无生成物路径可派生。
- 判据词汇仍封闭为**十值**（MUST NOT 加 `apiEntryInvalid` 一类第 11 值）：四条判据与 `apiEntryMissing` 同族、同档、同 `category = entry`；新增变体会牵动 `ALL` 长度、四面词汇对拍与金样，收益不足。
- 求值互斥序逐字不变（① 命中 → 跳过 ②③④），故「入口不可用」的元素不再报 `apiMissing` / `apiUndeclared` / `apiSignatureMismatch`。

### 六 · 数据迁移与生成物搬迁（必须同一提交）

| # | 对象 | 动作 |
|---|---|---|
| 1 | `crates/infra/archi.i.rs` | 删；新体落 `crates/infra/archi/src/lib.i.rs`（内容逐字不变） |
| 2 | `crates/infra/archi/src/lib.rs` | `#[path = "../../archi.i.rs"]` → `#[path = "lib.i.rs"]`，注释同步（删「与 `mod_path_attr` 输出等值」句） |
| 3 | `.dev-team/architecture/api/INDEX.toml` | 重生成：`ArchiRuntime` 的 `generated` 一行改值，其余逐字不变 |
| 4 | `models/04-backend-shell.toml` | **零改动**：`App.Backend.Shell` 的 `entry` 归后续变更（宿主重建后）——本变更 MUST NOT 写入 |
| 5 | `models/05-backend-tools.toml` | 两个工具 crate 各补一行 `api = "none"`（二进制 crate 的公共面是 CLI 契约而非库 API）；`entry = ".../src/main.rs"` **归后续变更**，本变更 MUST NOT 写入 |
| 6 | `crates/infra/agent/src/archi-knowledge.md` | 「元素字段面」补 `entry`（agent 侧写模型时的字段清单唯一出口） |
| 7 | `AGENT.md` / `CLAUDE.md` 的 `## 模块 API 契约` 第②条 | 方位词「元素根同级」→「**实现入口同级**」、文件名形态 `<root>.i.rs` / `<root>.i.ts` → `<入口主干>.i.rs` / `<入口主干>.i.ts`；**条目数恒 5**（只改措辞，不增删条目、不加第六条）；两文件该节改后仍**逐字相同** |

**为什么数据分两批（`entry` 数据出本变更、`api = none` 进来）**：架构漂移门禁跑在**宿主进程内**，用宿主二进制内嵌的 archi crate 解析模型——宿主的 archi crate 落后于工作树一版时，模型里任何**新字段面**（如 `entry`）都会被 `deny_unknown_fields` 判成 `unknownField`。实测（`reports/archi/20261116T0156308164987Z.json`）：04 与 05 的 `entry` 各产 1 条 `unknownField`，且 04 解析失败使 `App.Backend.Shell` 变成「未声明」，连带 06 的 11 条 `unknownReference`——合计 **13 条结构错 ⇒ `status = errors`**，即「用落后一版的宿主校验新字段面」必然先红，与实现的自由度无关。故本轮只落 **HEAD 字段面内**的 `api = "none"`（旧宿主可解析，13 条结构错归零），字段面数据由宿主重建并重启后的后续变更承接：

| 后续变更（宿主重建后） | 动作 |
|---|---|
| `models/04-backend-shell.toml` | `App.Backend.Shell` 补 `entry = "./packages/desktop/src-tauri/src/lib.rs"` |
| `models/05-backend-tools.toml` | 两个工具 crate 各补 `entry = "./packages/desktop/src-tauri/crates/tools/<crate>/src/main.rs"` |
| `repo-archi-model` / `desktop-archi-api-check` 的基线 | 显式 `entry` 三处归位；`apiFindings` 分桶由 **15 / 26** 回到 **12 / 27**（AC-4 的两条可见收益届时兑现） |

**代码面不受此限**：解析器（`entry` 字段）、解析链三态、四条判据、生成物落点迁移与数据批次解耦——代码随本变更发布，`entry` 一经声明即被新代码解析；本变更内它的判定由 `paths_test` / `check_test` 的合成用例覆盖（AC-1 / AC-2）。

**为什么第 7 行必须在范围内（原稿的冻结理由不成立）**：落点改到入口同级后，第②条「元素根同级的 `<root>.i.rs` / `<root>.i.ts`」与本变更的实现**直接矛盾**，而这份文件正是 agent 每轮注入的系统提示词（见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）——纪律与真实落点不一致，比少写一条更坏。原先「整体冻结」的依据是「该节被 req 钉死为**恰 5 条要点**，加第六条即违约」，该理由**只约束条目数、不约束条目正文的方位词**：本变更不增删条目、不把 `entry` 字段知识写进该节（字段知识仍归 `archi-knowledge.md`），故冻结面收窄为「条目增删与节结构」，措辞同步纳入实现范围。同步的**规则面**由 spec delta 承托（`desktop-archi-api`「仓库根文档的契约纪律节」MODIFIED，判据见 AC-9），文档与实现在同一提交内同时成立。

**为什么 1–7 同一提交**（第 4 行为零改动，随批核对即可）：生成物、`#[path]` 值、`INDEX.toml` 三处互为条件——分开提交时 `api:check` 在中间态必红（新落点 missing + 旧落点 orphaned + `INDEX.toml` 旧值）；第 7 行与 1–3 同批的理由不同：落点一迁，纪律节的方位词即刻失真，且该文件是 agent 的系统提示词（每轮逐字注入），中间态即误导下一轮 agent。

### 明确不做（出 range，记入风险以钉住边界）

| 项 | 归属 |
|---|---|
| 前端 12 个目录根补 barrel（`views/changes` 等 6 个多文件目录 + 6 个单文件目录） | 变更 B |
| TS 侧跨元素透传的**真相对路径**派生与接线（`entry → entry`）。本变更只定规则（入口同级 ⇒ `./<主干>.i`），`paths::ts_import_specifier` 与 `EntryForm` 保留为待接线形态 | 变更 C（`.i.ts` 由 C 首次产出） |
| 契约格式补丁、`api-reexport` 语义 | 变更 D |
| 模块入口文档（「入口在哪、为什么」的人读面） | 变更 E |
| `AGENT.md` / `CLAUDE.md` 的 `## 模块 API 契约` 节的**条目增删与节结构** | 冻结：该节被 req 钉死为**恰 5 条要点**，加 `entry` 第六条即违约；本变更只改第②条的方位词与文件名形态（§六 #7、AC-9），不增删条目 |
| 04 壳与 05 两个工具 crate 的**显式 `entry` 数据**（三处） | 后续变更（宿主重建并重启后）——本变更写进来即触发旧宿主 `unknownField`，13 条结构错回归（§六、R-10） |
| `scan/owner.rs` 之外的扫描判据、`docs` 关联面、`foundation::layout` | 零改动 |

---

## 能力

| 能力 | 分类 | 变更内容 |
|---|---|---|
| `desktop-archi-api` | **修改** | ① MODIFIED「生成物落点与入口查找」：整段重写——落点 = 入口同级、入口解析链三态与四条判据、`#[path]` 值退化为文件名（`mod_path_attr` 删除、等值性归 `paths_test` 断言）、TS specifier 规则 + 显式留白、归属三候选；② MODIFIED「生成物入仓与零 diff 门挂载」：四类生成物表的 `.i.rs` / `.i.ts` 落点列改「元素入口同级」；③ MODIFIED「仓库根文档的契约纪律节」：第②条方位词「元素根同级」→「实现入口同级」、文件名形态改 `<入口主干>.i.*`（`<root>.i.*` 出清），条目数恒 5、两副本逐字相同条款逐字不动 |
| `desktop-archi-api-check` | **修改** | ① MODIFIED「契约判据词汇封闭十值与开放分类」：`ApiEntryMissing` 一行语义改述为「无可用入口」；② MODIFIED「十条判据的定义与求值互斥顺序」：① 定义与 `has_generated` 退场；③ MODIFIED「判据级档位与元素 gate 解耦」：无入口元素数 / 条目数取新值（15）；④ MODIFIED「报告并列槽位 apiFindings」：存量 backlog 场景的数字取新值（15 / 26）；⑤ MODIFIED「契约判据的验证语料与报告数基线」：基线表分桶 28 / 13 → **15 / 26**（总 41 不变），并写明成因（两个工具 crate 的 `api = none` 不豁免 ①；壳与两工具 crate 的显式 `entry` 归位后才是 12 / 27，见 §六） |
| `desktop-archi-model` | **修改** | MODIFIED「架构模型文件格式与字段封闭」：`entry` 进值域（可选字符串，repo 相对路径指针）、「七项」→「八项」，并明确它**不是**协议正文 |
| `desktop-archi-agent` | **修改** | MODIFIED「架构会话的系统知识注入」：知识文本的元素字段面补 `api` 与 `entry`（前者本已漏记，同批改齐）；`AGENT.md` / `CLAUDE.md` 契约纪律节在该 delta 内只作**冻结面声明**（条目增删与节结构冻结），第②条措辞同步归 `desktop-archi-api`（§六 #7），本 delta 不改该节正文 |
| `desktop-archi-drift` | **修改** | MODIFIED「文件归属与两套解析器一套归属逻辑」：生成物归属由「双形态并集」改为「三候选并集」，并改述「生成器产出入口同级」 |
| `desktop-archi-file-tree` | **修改** | MODIFIED「归属判定与判定单源」：同上的三候选表述（树侧仍零第二份剥离规则） |
| `repo-archi-model` | **修改** | MODIFIED「基线落位与模型文件划分」：字段纪律补 `entry` 一条（四条判据 + 声明纪律 + 本变更**零显式声明**、三处目标值归后续变更）、`api = none` 两处显式留白、元素与关系计数**逐字不变**（51 / 113） |

| 不涉及 | 理由 |
|---|---|
| 新增能力 | 入口是模型字段与路径面的事实，无新用户可见能力 |
| `desktop-archi-contract-panel` | 两条 IPC 与三列差异形状不变（`orphaned` 名称即路径，落点变化不改语义） |
| `desktop-archi-canvas` / `desktop-archi-gate` / `desktop-archi-module-docs` | 零判据、零投影、零文档关联改动 |
| `desktop-ipc-type-bindings` | `ArchiElement` 加字段属常规再生成（`bindings:export`），绑定面规则零变化 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/archi/src/model.rs` | `Element` 加 `entry: Option<String>` |
| `crates/core/archi/src/parse/toml.rs` | `RawElement.entry` + `to_model` 搬运（未知字段仍由 `deny_unknown_fields` 拒） |
| `crates/core/archi/src/dto.rs` | `ArchiElement.entry` + `graph_view` 投影（缺省 `null`，显式字段不省略） |
| `crates/core/archi/src/api/paths.rs` | 新增 `entry_candidates`（纯文本）/ `entry_state` + `EntryState` / `EntryProblem`；`generated_path` 入参由 `<root>` 改**入口**（`<入口目录>/<主干>.i.<ext>`）；`generated_kind` 分派输入改入口；**删 `mod_path_attr`**；旧 `entry_form` 的形态位由 `EntryState::Resolved` 一并给出（入口与根的关系派生），函数本身退场 |
| `crates/core/archi/src/api/check.rs` | ① 改用 `entry_state`；`Absent` / `Invalid` ⇒ 一条 `apiEntryMissing`（文案按理由分列）；删 `has_generated` |
| `crates/core/archi/src/api/mod.rs` | `render_all` / `generated_path_of` / `index_entries` / `element_symbols` / `bootstrap` 取入口而非 `path.first()`；`generated_path_of` 入参由元素 `path` 改元素本身（解析入口） |
| `crates/core/archi/src/api/diff.rs` / `api/emit/mod.rs` / `api/emit/ts.rs` | 注释里的旧落点样例与「`entry_form` = 盘上命中哪个入口」表述同步（零逻辑改动） |
| `crates/core/archi/src/scan/owner.rs` | `OwnerEntry` 增文件型前缀的去扩展名形态；`owner_of` 对 `.i` 文件追加第三候选（主干对齐）；`normalize` / `prefix_matches` / 非 `.i` 路径归属逐字不变 |
| `crates/infra/archi/src/lib.rs` | `#[path]` 字面量 → `"lib.i.rs"`，注释同步 |
| `crates/infra/archi/src/lib.i.rs` | 新增（旧 `crates/infra/archi.i.rs` 的内容，**逐字不变**） |
| `crates/infra/archi.i.rs` | **删除**（否则 `api:check` 的 `orphaned` 非空） |
| `crates/infra/agent/src/archi-knowledge.md` | 元素字段面补 `entry` |
| `AGENT.md` / `CLAUDE.md` | `## 模块 API 契约` 第②条措辞：方位词改「实现入口同级」、文件名形态改 `<入口主干>.i.rs` / `<入口主干>.i.ts`；**条目数恒 5**、其余四条要点与节内其余条款逐字不动；两文件该节改后仍逐字相同（全文差异仍恰 2 处且全在节外） |
| `.dev-team/architecture/models/05-backend-tools.toml` | 两处模型数据：两个工具 crate 各一行 `api = "none"`（`04` 零改动；三处显式 `entry` 归后续变更，见 §六） |
| `.dev-team/architecture/api/INDEX.toml` | 重生成（仅 `ArchiRuntime` 的 `generated` 一行变化） |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 再生成（`ArchiElement.entry: string \| null`），入库 |
| `packages/desktop/package.json` | `version` 相对落地时 `HEAD` 现值 +1 patch（`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动） |

### 测试文件

| 文件 | 新增 / 改写 |
|---|---|
| `crates/core/archi/src/api/paths_test.rs` | 解析链三态（显式命中 / 约定默认命中 / `Absent`）；文件型根 = 自身；多文件根取 `path.first()`；四条反例（`Malformed` / `OutsidePath` / `NotOnDisk` / `Duplicated`）+ 单文件根声明别的文件 → `OutsidePath`；`generated_path` 三形态（crate / 子目录模块 / 单文件）落点断言改为入口同级；`mod_path_attr` 两组用例删除，替换为「入口行字面量 == `generated_path().file_name()`」的自证 |
| `crates/core/archi/src/api/check_test.rs` | `apiEntryMissing` 四理由各一条；`Resolved` 元素不报 ①；`api = none` + `entry` 有值的元素零判据；`.i` 在盘但无入口仍报 ①（`has_generated` 已退场） |
| `crates/core/archi/src/api/mod_test.rs` | `INDEX` 的 `generated` 取入口同级；`render_all` 生成物路径取入口；无入口元素不产 `.i.*` |
| `crates/core/archi/src/scan/owner_test.rs` | 第三候选三条正例（`views/welcome-view.i.ts` → 文件型根、`components/agent/index.i.ts` → 容器 barrel、`src/a.i.rs` → 单文件 `.rs` 根）；白名单外四条反例与 `src/dir.i.ts` 并集定档用例保持绿 |
| `crates/core/archi/src/{dto,model,parse/toml,parse/mod,search,graph,edit,tree}_test.rs`、`api/{contract,emit,extract}/**/_test.rs` | `ArchiElement` 字面量补 `entry: None`（机械）；`entry` 的解析 / 投影 / 未知字段各一条 |
| `crates/core/archi/tests/{golden_test,pilot_test,vocabulary_test}.rs` | 金样零改写（`model` 段只含计数）；`api-mismatch` case 仍产 `apiSignatureMismatch` 且 `status = warnings`（其 fixture 的 `src/mod/mod.rs` 由 `<root>/mod.rs` 候选命中，`modelHash` 不变） |
| `packages/desktop/src/views/architecture/**/*.test.tsx` | 元素 fixture 补 `entry: null`（机械） |
| 无新增测试文件（纪律节同步） | `AGENT.md` / `CLAUDE.md` 节内一致性由 `grep` / `diff` 机械断言（AC-9），与「纪律节只指路不带正文」「两份副本不分化」两条既有 scenario 同口径；MUST NOT 为两份文档引入解析测试或把文档正文迁进代码常量 |

**表外同批适配面**（上表按「测试对象」列举，下列文件的改写同属本变更、不留作未归属改动）：

| 文件 | 改写性质 | 归属理由 |
|---|---|---|
| `crates/core/archi/src/{diff,docs}_test.rs`、`src/api/{diff,render}_test.rs`、`src/scan/mod_test.rs`、`src/drift/mod_test.rs` | `ArchiElement` 字面量补 `entry: None`；`entry` 零敏感面各一条（结构 diff / 文档关联 / 扫描 / 纳管子集）；`api/diff_test.rs` 的落点样例改入口同级 | `ArchiElement.entry` 加字段的机械扩散（R-8）+ 落点迁移的既有断言跟随（AC-3 / AC-4） |
| `crates/core/checks/src/diagnose_test.rs` | `check_integrity(&summary, std::slice::from_ref(&sub))` | clippy 机械修正（`&[x.clone()]` → `from_ref`），零行为变化 |
| `crates/infra/archi/src/runner_test.rs` | `include_str!("../../archi.i.rs")` → `include_str!("lib.i.rs")` | 生成物搬迁的既有见证用例跟随（AC-3） |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 字段面八项 / `entry` 三纪律 / 示例单值 `entry` / 判据长表留白 / 边界「两个指针」五组用例 | `desktop-archi-agent` delta 的 Module Contract 已把本文件列为知识面语料宿主（AC-6） |
| `crates/tools/archi-api-gen/src/main_test.rs` | 装置改「文件型根（根文件自身即入口）」、`.i.*` 落点断言改入口同级 | CLI 产品面零改动（`不要修改` 表只冻结 `crates/tools/archi-api-gen/**` 的实现）；生成器落点迁移后既有装置必须跟随（AC-3），否则 `generate` / `check` 的落点断言与实现互相矛盾 |
| `packages/desktop/src-tauri/src/commands/archi/mod_test.rs` | 契约用例补入口落盘（`src/shop/src/lib.rs`）、写盘断言改 `crates/api/subject/src/lib.i.rs` | 壳源码零改动（`不要修改` 表冻结的是 `src-tauri/src/**` 的实现）；`archi_module` 出参的生成物路径改为入口的函数后，该用例的既有断言必须跟随（AC-3） |
| `packages/desktop/src/lib/ipc-contract.test.ts` | `ArchiElement.entry` 类型面 / schema 面 / 两态解析 / `strictObject` 严格性四组用例 | `desktop-ipc-type-bindings` 的线面同源校验（AC-7）：生成物字段的契约面必须由 IPC 契约用例钉住 |
| `packages/desktop/src/app.test.tsx`、`src/views/architecture/hooks/{use-archi-graph,use-archi-module}.test.ts`、`src/views/architecture/lib/drift-marks.test.ts`、`src/views/changes/change-detail-view.test.tsx` | 元素 fixture 补 `entry: null` | 与上表 `views/architecture/**/*.test.tsx` 同一机械补齐面（`ArchiElement` 加字段，`strictObject` / 类型面双拒缺键） |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/api/**/*.md` | 本变更是**落点与入口**变更，契约内容零改动（`api:export` 后 `App.Backend.Infra.ArchiRuntime.schema.json` 应零 diff） |
| `AGENT.md` / `CLAUDE.md` 契约纪律节的**条目面与节结构** | 恰 5 条要点的条目数冻结：MUST NOT 增删条目、MUST NOT 加「`entry`」第六条、MUST NOT 把判据长表 / fenced json / `openspec/specs/` 路径写进该节；字段知识走 `archi-knowledge.md`。第②条的**方位词**属本变更实现面（见「实现文件」与 AC-9） |
| `crates/tools/archi-api-gen/**` | CLI 三子命令形态不变（`generate` / `--check` / `bootstrap`），写盘单点不动（本目录**实现**零改动；`src/main_test.rs` 的既有装置随落点迁移跟随，归属见上「表外同批适配面」） |
| `foundation::layout` / `crates/core/config` / `packages/desktop/src-tauri/src/**`（壳源码） | 零路径派生与零配置键变化（壳**实现**零改动；`commands/archi/mod_test.rs` 的既有断言随落点迁移跟随，归属见上「表外同批适配面」） |
| 前端 12 个目录（`views/*`、`components/ui`） | 补 barrel 归变更 B；本变更新增的 `apiMissing` 是待决策点，不是待实现代码 |
| `paths::ts_import_specifier` 的**语义**与 `ModuleRef.name` 的派生 | TS 透传归变更 C（本变更只留规则 + 显式留白，见风险 R-7） |

---

## 验收标准

1. **AC-1 解析链三态可判（`paths_test`）**：① 显式 `entry` 命中（**合成 fixture**：本变更模型数据零 `entry` 声明，见 §六与 AC-6）；② 文件型根 = 根文件自身（`views/welcome-view.tsx` → 自身，不拼 `.tsx`）；③ 目录型根四种约定形态按序命中的首个落盘候选（`src/lib.rs` → `mod.rs` → `index.ts` → `index.tsx`，不做语言判定）；④ `path` 空 / 候选全不落盘 → `Absent`。**候选派生零 IO**（`entry_candidates` 不读盘），读盘只发生在 `entry_state` 与 `api:check`。
2. **AC-2 四条判据各一反例**：`entry = "/abs/x.rs"` → `Malformed`；`entry` 落在另一个元素的 `path` 内 → `OutsidePath`；`entry` 指向不存在文件 → `NotOnDisk`；两个元素声明同一入口 → 双方各 `Duplicated`。单文件根声明「同目录的另一个文件」归 `OutsidePath`（落位判据覆盖，不设第五条判据）。
3. **AC-3 落点迁移停在不动点**：`crates/infra/archi/src/lib.i.rs` 在盘、`crates/infra/archi.i.rs` 已删；`crates/infra/archi/src/lib.rs` 的 `#[path]` 字面量为 `"lib.i.rs"`（零 `../`）；`pnpm api:check` 零差异（`orphaned` / `missing` / `mismatched` 三列皆空）；`INDEX.toml` 该元素 `generated = "packages/desktop/src-tauri/crates/infra/archi/src/lib.i.rs"`，其余 41 条逐字不变；`cargo test -p archi` 全绿。
4. **AC-4 判据面数字与基线逐项相等（分两批的第二批口径见下）**：本仓根一次全量检查后 `apiFindings` = **41 条**，分桶 `apiEntryMissing = 15`（8 个视图目录根 + 3 个视图内子域 + `components/ui` + 壳 `App.Backend.Shell` + 两个工具 crate——后三者因**本变更未写 `entry`**，仍是 `Absent`；`api = none` 不豁免 ①）+ `apiMissing = 26`（13 个 crate + `App.Frontend.Shell` + 12 个前端单文件根；试点 `ArchiRuntime` 自身零命中），其余七类各 0、`apiUndeclared = 0`。`status` 与改动前逐字相同（唯一契约在案元素的判据仍 0 条 ⇒ `clean`），`passed` 不受影响——**15 + 26 = 41 与 HEAD 的 28 + 13 = 41 总条目数相等**，是**分桶变化**（13 个前端文件型根元素由「猜不到入口（假报）」转为「有门无契约（真话）」），不是新增债务。
   - **本轮即可验证的部分**：模型数据落在 HEAD 字段面内 ⇒ 旧宿主解析**零结构错**（HEAD 上那 13 条 `unknownField` / `unknownReference` 全数归零），门禁 `status` 从 `errors` 回到可判定状态。这是分批带来的**门禁可判定性**收益（HEAD 上那份报告正是据此落到 `status = errors`）。
   - **本变更仍兑现的收益（代码面）**：12 个前端单文件根与多文件根 `App.Frontend.Shell` 由 ② / ③ 命中而退出 `apiEntryMissing`——不需要任何模型数据，故不受分批影响。
   - **降级为后续变更（宿主重建后）的验收项**（本变更 MUST NOT 记为已达成）：① `App.Backend.Shell` 退出 `apiEntryMissing`；② 两个工具 crate（`ExportBindings` / `ArchiApiGen`）零判据；③ 分桶回到 **12 / 27**（总 39）。三项都以「壳 + 两工具 crate 的显式 `entry` 三处归位」为前提。
   - **口径勘误（本提案取实测值，MUST NOT 沿用 15 / 27）**：分批后分桶是 **15 / 26 = 41**，不是 15 / 27 = 42——壳 `App.Backend.Shell` 从 `apiMissing` **移回** `apiEntryMissing`（净变化 0），两个工具 crate 各新增一条 ①（净 +2），故总数 41 与 HEAD 的 41 相等。实测留痕见 `reports/api-baseline/measured.json`（逐条列出 15 + 26 的元素 FQN）。
5. **AC-5 归属判据覆盖新落点**：`crates/infra/archi/src/lib.i.rs` → `ArchiRuntime`；`views/welcome-view.i.ts` → `Views.Welcome`（文件型根，第三候选；合成断言）；`components/agent/index.i.ts` → `Components.Agent`；`src/mod/mod.i.rs`（目录型根）仍命中；白名单外后缀四条反例与 `src/dir.i.ts` 并集定档用例逐值不变。本仓实盘 `.i.ts` 仍为 0（第三候选在本变更内零真实实例，MUST NOT 记为已接线）。
6. **AC-6 模型面与知识面**：`entry` 可解析（解析器用例，合成源文）、未声明读作 `null`、拼错的字段仍 `unknownField`；`archi-knowledge.md` 的元素字段面含 `entry`，其示例 TOML 经 `TomlParser` 零诊断（既有自证测试保持绿）；`archi_graph(root)` 为 51 元素 / 113 关系 / 零诊断 / **零告警**，且**全部元素的 `entry` 均为 `null`**——`grep -c '^entry = ' .dev-team/architecture/models/*.toml` 合计为 **0**（三处显式值归后续变更；写进来即违 AC-4 的第一段，因为旧宿主会把新字段面判成 `unknownField`）。
7. **AC-7 IPC 面再生成**：`bindings.ts` 的 `ArchiElement` 含 `entry: string | null`，`bindings.zod.ts` 同步；`knip` 零新增死代码；`client:check` 绿。
8. **AC-8 零回归**：`crates/core/archi/tests/corpus/drift/` 全部金样逐字不变（含 `api-mismatch` 的 `apiSignatureMismatch` 与 `status = warnings`、`modelHash` 不变）；`api-reexport` 与本变更零交集（本仓零透传契约）。
9. **AC-9 纪律节与真实落点一致**：`AGENT.md` 与 `CLAUDE.md` 的 `## 模块 API 契约` 节各**恰 5 条要点**（`grep -c '^- \*\*'` 在该节内 = 5，与原值相同）；该节 `grep '元素根同级'` 与 `grep '<root>.i'` **零命中**，第②条出现「实现入口同级」且文件名形态逐字为 `<入口主干>.i.rs` / `<入口主干>.i.ts`（与 AC-3 的实盘落点 `crates/infra/archi/src/lib.i.rs` 同口径）；两文件该节正文**零 diff**（全文 `diff` 仍恰 2 处且全在节外：`:1` 标题、`:3` 首行说明）；该节仍零 `entry` 字段说明、零 fenced json、零 `apiEntryMissing` 词形。**MUST NOT 以「省事」把 `entry` 写成第六条**——字段知识只进 `archi-knowledge.md`（AC-6）。

> **不列为 AC 的流程项**：`package.json` 版本纪律、`check` 四段链形态、`api = none` 的显式留白语义——各有现成 spec 落点（`desktop-archi-api`「生成物入仓与零 diff 门挂载」），本 delta 不改这些条款 ⇒ 不另设无 spec 落点的 AC。仓库根文档纪律节**不再属于**这类条款：第②条措辞是本变更的实现面（§六 #7），故设 AC-9。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **报告数跃迁被误读为新增债务**：`apiMissing` 13 → 26、`apiEntryMissing` 28 → 15，总量 41 → 41（分桶变化） | 口径写进 AC-4 与 `desktop-archi-api-check` 的基线 requirement：13 个前端文件型根元素（12 个单文件根 + `App.Frontend.Shell`）由「猜不到入口（假报）」变为「有门无契约（真话）」，`status` / `passed` 双不变（纳管子集只看契约在案元素）；壳与两个工具 crate 仍在 ① 面（显式 `entry` 归后续变更）；变更 B 补 barrel 后 `apiEntryMissing` 只余壳与两工具 crate |
| R-2 | **迁移必须原子**：生成物搬迁 / `#[path]` 改写 / `INDEX.toml` 重生成分三次提交即每次 `api:check` 红 | 三者同一提交落地（提案 §六）；实现第一件事是搬运行并跑 `api:check`，确认 `orphaned` 空后才动生成器逻辑 |
| R-3 | **四条判据挤进一个 finding kind**：`Duplicated` 报成 `apiEntryMissing` 是语义拉伸，读者可能以为「元素没有门」 | 文案 MUST 按 `EntryProblem` 分列理由（点名冲突方 FQN / 落位越界的目标 / 缺失的文件），MUST NOT 只写通用句；新增第 11 值的收益不足（牵动 `ALL` 长度、四面词汇对拍与金样） |
| R-4 | **文件型根不许声明别的文件，但这条只由落位判据兜住**：若将来出现「根是文件、门该是另一个文件」的真实需求，现判据会把它判成 `OutsidePath` | 显式记入本 proposal：该形态需要改解析链与本 requirement（新增判据），MUST NOT 靠放宽落位判据「顺手支持」 |
| R-5 | **默认候选减少三条**（Rust `<dir>.rs`、前端 `<dir>.ts` / `<dir>.tsx`）：本仓零命中，但属行为收窄 | 三条对目录型根只能指到 `path` 外的兄弟文件（必违落位判据）；实测零命中，`entry_candidates` 的注释与 `paths_test` 的反例钉住「不在列 = `Absent`」 |
| R-6 | **第三候选可能改变 `Ambiguous` 定档**：同为文件型根且主干相同的两个元素（`src/a.ts` / `src/a.tsx`）在 `src/a.i.ts` 上会同时命中 | 判据取并集最长档、同长仍 `Ambiguous`（MUST NOT 任选），本仓零实例；`owner_test` 加合成用例钉住 |
| R-7 | **TS 透传 specifier 仍是旧形态**：`ts_import_specifier(form, name)` 产 `../<name>.i`，与「入口同级 ⇒ `./<主干>.i`」在跨目录/同目录两种情形都可能不等 | 显式留白：本仓 `.i.ts` 生成物为零 ⇒ 无可观测差异；spec 只写规则并标注「本期未接线」，MUST NOT 把当前输出当既有真值；变更 C 必须接线并做入口到入口的真相对路径派生 |
| R-8 | **`ArchiElement` 加字段带来机械面扩散**：全仓构造型测试字面量与前端 fixture 都要补 `entry` | 纯机械补齐、零行为变化；金样不含元素对象（`model` 段只有计数）⇒ 16 份快照零改写；若发现某处需要真值而非 `None`，回到提案层确认而不是就地补默认 |
| R-9 | **纪律节两份副本分化**：只改 `AGENT.md` 忘改 `CLAUDE.md`（或反之）即违反「该节正文逐字相同」；另一面是「改着改着顺手加了第六条」 | 两文件**成对同一提交**修改、只动第②条；AC-9 以「节段 `diff` 为空 + 全文 `diff` 恰 2 处且全在节外 + 节内要点数仍 5」三条同批断言；字段知识一律走 `archi-knowledge.md`，MUST NOT 借同步之机塞进该节 |
| R-10 | **数据批次越过宿主版本**：模型数据写入 `entry` 而宿主的 archi crate 尚未重建 ⇒ 该文件 `unknownField` + 依赖它的元素「未声明」连带 `unknownReference`（实测 13 条 ⇒ `status = errors`，门禁从可判定态直接掉到 `errors`） | 本变更 MUST NOT 写 `entry`（AC-6 的 `grep` 计数为 0）；后续变更须在宿主重建并重启后落地，并在同一 change 内把基线由 15 / 26 更新为 12 / 27（AC-4）。**这条风险不是实现瑕疵**：判定与解析耦合在**宿主版本**上（门禁在进程内用宿主内嵌的 archi crate 解析模型），故「数据先于宿主重建落地」这一顺序在本仓 MUST NOT 使用 |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| A.6「多 `path` 与 `entry` 的关系」 | 本提案裁：落位判据（等于某条 `path` 或落在目录型 `path` 内）；多文件根由 `path.first()` 的文件型形态自动命中（`App.Frontend.Shell` → `app.tsx`），不新增模型数据 |
| A.7 #7「工具 crate：`api = none` 或 `entry = src/main.rs`」的「或」 | 收敛为**两者都写**（终态）：`entry` 陈述实现入口的事实，`api = none` 陈述公共面是 CLI 而非库 API。本变更只落 `api = none`（HEAD 字段面内），`entry` 随后续变更归位（§六）；在此之前两个工具 crate 仍报 `apiEntryMissing`（`Absent`），`api = none` 不豁免 ① |
| A.10.2 第 1 条「TS 透传真相对路径可降级」 | 采纳（R-7），但规则必须写进 spec，不得留空 |
| A.10.2 第 2 条「`owner.rs` 可出 range」 | **不采纳**（勘误：文件型根的落点两候选皆不命中），第三候选纳入本变更 |
| 主线 §5 状态同步缺口 | 属主线文档维护，不在本 change 产物内；本提案已按代码事实给出勘误表（见「问题」末节） |
