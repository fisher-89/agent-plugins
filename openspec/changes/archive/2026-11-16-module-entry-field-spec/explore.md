# 变更 A · 模块入口字段与接口文件落点

workflow_type: requirement

> 主线：`openspec/explores/module-entry-declaration.md`（现状实测与变更切分）。
> 本单元 = 模型新增 `entry` 字段 + 解析链 + 接口文件 `<入口>.i.<后缀>` 落点规则。
> 不含：前端 barrel（变更 B）、TS 取数面（变更 C）、契约格式补丁（变更 D）、模块入口文档（变更 E）。

## A.0 scope 与依赖

| 项 | 内容 |
|---|---|
| 前置依赖 | 无（可第一个落） |
| 被依赖 | 变更 B（barrel 需要默认入口口径）、变更 C（取数面按入口定位） |
| 独立可归档 | 是——只改 `core/archi` 的模型/路径/渲染面与一份生成物迁移 |
| 已裁决 | 默认值取 **R1 语言约定**（见 A.2）；前端统一补 barrel（变更 B） |

## A.1 入口字段与默认值口径（原主线 §9）

### A.1.1 字段形态（先定「入口」是什么事实）

`entry` 是**单值**（不是数组——数组就把「唯一」写死成不可表达），语义四则：

| 判据 | 措辞 | 理由 |
|---|---|---|
| 基准 | repo 相对 POSIX 路径（与 `path` 同口径，`./` 前缀不参与身份） | 与 `path` / `docs` 同一套 `normalize_rel` 纪律；MUST NOT 引入「相对某个 root」的第二套相对性 |
| 落位 | MUST 等于某条 `path` 条目，或落在某条目录型 `path` 之内 | 否则 `owner_of` 的归属与「入口」指向两个不同模块 |
| 存在 | 声明了就 MUST 在盘上存在（缺 → 报一条） | 入口是「架构 ↔ 实现」的锚；声明一个不存在的文件比不声明更坏 |
| 跨元素 | 任何两个元素 MUST NOT 声明同一入口文件 | 「一个入口只属于一个模块」正是原则的后半句 |

### A.1.2 默认值：把三条候选规则拿去实测

对 **29 个目录根元素**（后端 17 + 前端 12）逐条试算（数据取自当前工作树）：

| 规则 | 命中 | 覆盖 | 脆弱性 |
|---|---|---|---|
| **R1 语言约定**（Rust `<root>/src/lib.rs`；TS `<root>/index.ts(x)`） | 14 个 crate | 14/29 | 零 IO、零顺序；TS 侧命中 **0**——前端**一个 `index.ts(x)` 都没有**（全仓唯一 barrel 是 `components/agent/index.tsx`，且它是文件级 `path`） |
| **R2 目录内顶层非测试源文件恰 1** | 前端 6（agent/agents/architecture/config/db/info）+ 工具 crate 2（`main.rs`） | 8/29 | 需列目录（IO）；**新增一个 `helper.ts` 就换默认入口**——把「实现漂移」引回架构层，正好违背本 todo 的初衷 |
| **R1 ∪ R2** | 22 | 22/29 | 前两种缺点的并集，但覆盖面最高 |
| **R3 = 今天的探测** | 14（与 R1 完全等价） | 14/29 | 七候选对**目录根**毫无增益——探测只解决了「Rust crate 有 src/lib.rs」这一件事 |

**R1-only 口径下（A.1.6 已裁）需显式 `entry` 的是 15 个**：12 个前端目录 + 壳 `src-tauri/src/` + 2 个工具 crate。其中前端 12 个分两类——6 个单文件目录（`views/agent`、`views/agents`、`views/architecture`、`views/config`、`views/db`、`views/info`）与 6 个多文件目录（`views/changes`、`views/changes/agent`、`views/changes/flow`、`views/changes/renderers`、`views/explores`、`components/ui`），统一由变更 B 补 barrel 造门；本单元只需裁决壳与两个工具 crate（A.1.5）。前表「剩余 7」是 R1∪R2 口径的残留——R2 依赖 IO 且「新增一个文件即换默认入口」，已在 A.1.6 裁掉。

结论：**默认值只能救「Rust crate」这一类**；前端目录无论选哪条规则都拿不到默认（TS 的 index 约定在本仓从未被采用过）。因此「默认值」这条设计的真正作用域是 Rust crate，其余必须显式——这一点和直觉相反，值得写进 spec 的 Scenario。

### A.1.3 解析链与「无入口」这个状态

```text
resolve_entry(element):
  ① 显式 entry            → 校验（存在 / 落位 / 跨元素唯一）→ 命中
  ② 否则语言约定默认       → Rust <root>/src/lib.rs；TS <root>/index.ts(x)
  ③ 否则                  → 无入口（今天的 apiEntryMissing 语义平移：
                            「未声明且无约定默认」，一条 warn，不进 status）
```

- **单文件根元素**（`views/welcome-view.tsx`、`components/agent/*.tsx`、`standard/*.tsx`）：入口 = 根文件本身，`entry` 填写即**无效**（可判为 `invalidValue` 或直接忽略，但 MUST NOT 允许与根文件不同的值）。
- **多文件根元素**（全仓唯一：`App.Frontend.Shell` 的 4 个文件）：todo 的「根文件」规则没覆盖这一态。要么 `entry` 必填且 = 其中一条，要么 fallback 到「无入口」。它是自然答案 `app.tsx`，写成必填最省。
- **多目录根元素**：`entry` 落在其中之一即可（今天没有这种元素，但 `path` 是数组就允许出现）。

### A.1.4 前端 6 个多文件目录：四条路，代价不同

`views/changes` 是最典型的样本：`change-list-view.tsx` 与 `change-detail-view.tsx` 都是 `export function`、都是路由挂载点——**它天然有两个入口**。

| 路 | 做法 | 代价 | 隐含判断 |
|---|---|---|---|
| i **补 barrel** | 目录内加 `index.ts` re-export 公共面 | 6 个新文件；前端唯一 barrel 先例被稀释；`repo-archi-model` 的「容器 `path` 唯一例外 = `Components.Agent`」要扩面；knip 对纯 re-export 的口径要复核 | 「目录就是一个模块」成立，只是缺个门 |
| ii **显式主文件** | `entry = change-detail-view.tsx` | 语义降级：入口 = 「接口文件落点 + 抽取起点」，不再等于「公共面恰等于入口导出集」；`apiUndeclared`（今天 severity = off）对前端彻底失效 | 「入口」只是主文件，公共面可以更宽 |
| iii **拆元素** | `Views.Changes` → `Views.ChangeList` + `Views.ChangeDetail` | 元素计数（51 / 113）、`0001-model-granularity.md` 三档判据、`repo-archi-model` 的 28/64 全废；`flow` 15 文件按「每文件一元素」会涨到近百 | 目录**不是**模块，一个文件才是（与 `components/*` 现状一致） |
| iv **`api = none`** | 不写入口，显式声明无 API | 对视图而言是**假话**（它们确实导出组件给路由）；把「显式留白」污染成「现在不想写」 | 承认契约面暂时只服务后端 |

顺带一个此前没人摆到一起的事实：**前端粒度本来就不统一**——`components/agent/*`、`components/standard/*` 是「每文件一元素」，而 `flow`（15 文件）/ `renderers`（7）/ `ui`（39）是「整目录一元素」。入口唯一性这条原则正好是这处不一致的镜子：**找不到唯一入口的目录，可能本来就不该是一个元素**（或该有一个门）。`ui` 是极端例：它按 ADR 0001 被刻意收敛成「层」，所以它**永远不会有入口**——这就是「无入口」状态必须合法存在、且不能拿 `api = none` 冒充的理由。

### A.1.5 壳与两个工具 crate 的诚实答案

- `App.Backend.Shell`（`src-tauri/src/`）：入口 `src/lib.rs` 真实存在，只是七候选拼不出来 → **显式 entry 是纯收益**，也是本 todo 最直接可见的修复。
- `App.Backend.Tools.*`（只有 `main.rs`）：二进制 crate 的公共面是 CLI 契约而非库 API。R2 会给出 `main.rs` 作默认，但抽取面通常零导出 → 更诚实的答案是 `api = none`（这时「无入口」与「无 API」两个空态要分清：前者是没门，后者是明说无 API）。

### A.1.6 口径（已裁）

1. **默认值只留 R1**（纯静态、零 IO、零顺序），spec 里明写「TS 侧在本仓原无约定默认，本变更后以 `<root>/index.ts(x)` 为约定」（12 个前端目录的补齐见变更 B）。**不保留 R2**：它需要列目录，且「新增一个文件即换默认入口」正是本 todo 要消灭的漂移源。
2. **`entry` 单值 + 落位 + 存在 + 跨元素唯一**四条判据；违反走 `apiFindings` 面（只报不拦，与 `apiEntryMissing` 同族），不进结构化诊断（九个诊断种类会让整图停摆，过重）。
3. **「无入口」是合法状态**（= 不参与契约面），今日 27 条 `apiEntryMissing` 因此从「噪声」变成「27 个待决策点」：写入口、或写 `api = none`——两条都是真话。
4. 前端六个多文件目录的处置**已裁**：统一走 i（补 barrel，粒度不变），见变更 B。

---

## A.2 接口文件名的三种读法

| 读法 | Rust（入口 `src/lib.rs`） | TS（入口 `views/welcome-view.tsx`） |
|---|---|---|
| i 主干 + `.i.` + 后缀 | `src/lib.i.rs` | `views/welcome-view.i.ts` |
| ii 全名 + `.i.` + 后缀 | `src/lib.rs.i.rs` | `views/welcome-view.tsx.i.ts`（= 今天） |
| iii `<entry>.i.<ext>` 作为完整路径串 | `src/lib.rs.i.rs` 同 ii | 同 ii |

只有 **i** 能同时拿到「名字可读」+「落在入口同级」+「`#[path]` 无 `../`」三个收益；ii/iii 只是把今天的丑陋名字换个位置。另有一个反向成本：14 个 crate 的生成物都会叫 `lib.i.rs`（`INDEX.toml` 里读起来不如 `archi.i.rs` 有信息量）——如果想两者兼得，就得让名字来自「元素根末段」而落点来自「入口同级」，那又是一套新规则。

## A.3 `#[path]` 还要不要「机械对拍」

入口同级后属性值恒为文件名，仍**不能**由函数求值（Rust 属性值只能是字面量）。所以 `mod_path_attr` 要么删掉（改由 `paths_test` 直接断言「三形态都产同一串」），要么保留为常量真值源。这是「删一层间接」与「留一处真值源」的取舍，与既有纪律（D40/D41 的「零二次派生」）一致性最强的是**删函数、留断言**。

## A.4 TS 透传要真算相对路径了

`../<name>.i` 在入口同级形态下必然错位（子域到兄弟组件要 `../../components/agent/index.i`）。要么补一个「入口文件 → 入口文件」的相对路径派生（`ref_relative` 只处理 `<Domain>/` 一层，不够用），要么**明令禁止跨目录透传**（前端 24 个元素里父子/兄弟组合很多，禁了等于掐掉前端契约的复用能力）。

## A.5 「Rust 模块路径」与「入口文件名」是两个事实

`rust_module_path` 今天由**元素根末段**派生（`crate::<tail>`），且只对「同 crate 内的嵌套元素」成立。入口化以后不能顺手把它改成「入口主干」——`crates/infra/archi/` 的包名是 `archi-runtime`、目录名是 `archi`、入口是 `lib.rs`，三个名字互不相等；用入口名推模块路径会把 `pub use crate::lib::X` 这种不可编译文本写进生成物。这条必须继续单点派生、显式禁止互推。


## A.6 多 `path` 元素与 `entry` 的关系

`entry` 是 **repo 相对**（与 `path` 同口径）还是 **相对某个 root**？多 `path` 时必须是「某 root 之内」还是「任一处」？`App.Frontend.Shell`（4 文件）如果声明 `entry = app.tsx`，另外 3 个文件仍归本元素（覆盖判据不变），这是可接受的；但如果允许 entry 落在 root 之外，`owner_of` 的最长前缀归属就会与「入口」不一致。

- 待裁：单一 `entry` 与多 root 的关系（推荐：entry MUST 等于某条 `path` 或落在目录型 `path` 之内）；
- 本仓唯一的多文件根元素是 `App.Frontend.Shell`（`app.tsx` / `routes.tsx` / `main.tsx` / `components/app-sidebar.tsx`），自然答案 `entry = .../src/app.tsx`；
- 多目录根元素当前不存在（`path` 是数组，规则要允许将来出现）。

## A.7 模型侧改造与迁移清单

| # | 对象 | 动作 |
|---|---|---|
| 1 | `api/model.rs` / `api/parse/toml.rs` / `dto.rs` | 新增 `entry: Option<String>`（字段面 + `RawElement` + 线面投影）→ `bindings:export` |
| 2 | `api/paths.rs` | `entry_form` 改为「声明 → 约定默认 → 无入口」解析链；新增 `<entry>.i.<ext>` 派生；`EntryForm` 一族按 A.3 处置 |
| 3 | `api/mod.rs`（`render_all` / `generated_path_of` / `index_entries`） | `path.first()` 退出；生成物路径取入口 |
| 4 | `api/check.rs`（`apiEntryMissing` / `has_generated`） | 语义改述为「未声明且无约定默认」；`.i` 存在性不再是入口的替代证据 |
| 5 | `scan/owner.rs`（`strip_candidates`） | 生成物落回元素根内 → 单候选（保留一层防御即可） |
| 6 | 生成物迁移 | `crates/infra/archi.i.rs` → 入口同级新落点；`crates/infra/archi/src/lib.rs:16` 的 `#[path]` 值同步；`INDEX.toml` 全量重写 |
| 7 | 模型数据 | **17** 个后端元素：14 个 crate 走默认可省；`App.Backend.Shell` 显式 `entry = ./packages/desktop/src-tauri/src/lib.rs`；2 个工具 crate 判 `api = none` 或 `entry = src/main.rs` |
| 8 | 知识面 | `archi-knowledge.md` 的「元素字段面」补 `entry`；`AGENT.md`/`CLAUDE.md` 的契约节按需补一句 |

## A.8 验收判据（机械可复算）

1. `api:check` 零 diff（新旧落点迁移后工作树停在不动点）；
2. 入口解析链三态各一条断言：显式命中 / 约定默认命中 / 无入口；
3. 违反四条判据各一条反例（不存在 / 落在 path 外 / 跨元素重复 / 单文件根却填了别的文件）；
4. `cargo test -p archi` 全绿；`api:check` 的 orphaned 集为空（旧 `<root>.i.rs` 必须消失）；
5. `App.Backend.Shell` 不再报 `apiEntryMissing`（本条是 A 的唯一「可见收益」样本）。

## A.9 裁决状态（本单元已全部收口，签字见 A.11）

- **文件名读法 → i**（主干 + `.i.` + 后缀，落入口同级）：A.2；
- **默认值只留 R1**（不保留 R2 单文件目录第二档，R2 需 IO 且「新增一个文件即换默认入口」）：A.1.2 / A.1.6；
- **四条判据落 `apiFindings` 面**（只报不拦，不进结构化诊断）：A.1.6；
- **`mod_path_attr` 删函数、留 `paths_test` 断言**：A.3 / A.11；
- **TS 透传真相对路径 → 降级到变更 C**（本变更内 `.i.ts` 零实例）：A.4 / A.10.2。

---

## A.10 可启动性复核（实测，2026-11-15）

### A.10.1 事实逐条复验

| 笔记断言 | 复核方式 | 结论 |
|---|---|---|
| 前端目录零 `index.ts(x)` | `find packages/desktop/src -name 'index.ts*'` | ✅ 全仓唯一 `components/agent/index.tsx`（且它是 `Components.Agent` 的 `path`，文件级条目） |
| 14 个 crate 命中 `<root>/src/lib.rs` | `models/02`（core 7）+ `models/03`（infra 7） | ✅ 14；后端共 **17**（+shell 1 + tools 2） |
| 今日 27 条 `apiEntryMissing` | `path` 非空元素 42（41 叶 + 持 barrel 的容器 `Components.Agent`）− 13 个无生成物的 crate 元素 | ✅ 42−14（有入口的 crate）−1（`ArchiRuntime` 生成物免报）= 27 |
| `entry_form` 七候选 / `mod_path_attr` / `ts_import_specifier` / `ref_relative` | `core/archi/src/api/paths.rs:242–300` | ✅ 逐字相符 |
| `path.first()` 双用（`generated_path` + `entry_form`） | `paths.rs:220/242`、`check.rs:80` | ✅ 另加 `api/mod.rs` 的 `generated_path_of` |
| 归属「双候选并集」 | `scan/owner.rs:183` `strip_candidates` | ✅ 与 spec 逐字一致 |
| 契约面现状 | `.dev-team/architecture/api/` | ✅ 手写 md 仅 **1** 份（`Infra/App.Backend.Infra.ArchiRuntime.md`，Rust）；`INDEX.toml` 42 条，`declared` 1 / `absent` 41 ⇒ **`.i.ts` 生成物为零** |

### A.10.2 三处可以让 A 更小（建议在 proposal 里就写明）

1. **A.4 可降级**：`.i.ts` 生成物为零 ⇒「补真相对路径派生」与「禁跨目录透传」在本变更内**无可观测差异**（没有一件跨元素 TS 透传会跑）。A 只需定义规则（入口同级 ⇒ `./<stem>.i`、跨目录按需补 `ref_relative` 同族函数），验证推给 C（C 才产出前端契约）。A.4 不再阻塞启动。
2. **A.7 #5（`owner.rs`）可出 range**：生成物改到入口同级后**仍落在元素 `path` 内**——① 目录型 `path` 的前缀匹配天然覆盖 `<root>/src/lib.i.rs`；② 单文件元素 `views/welcome-view.tsx` 的剥离去向之一全等根（`welcome-view.i.ts → welcome-view`）。故归属判据零改动，双候选原样保留作防御即可（不必改 `scan/owner.rs`）。
3. **fmt 面零新增**：`#[path]` 模块本就在 rustfmt 的模块树内（实测 `rustfmt --check crates/infra/archi.i.rs` 退出 0），迁移不把生成物新塞进 `cargo fmt --all --check` 的门内。

### A.10.3 两处勘误（写 proposal 前必须改数）

| 位置 | 现文 | 应为 |
|---|---|---|
| A.7 #7 | 「16 个后端元素」 | **17**（core 7 + infra 7 + shell 1 + tools 2） |
| A.1.2 收尾 | 「剩余 7 个必须显式裁决」 | **15**（R1-only 口径：12 前端目录 + 壳 + 2 工具 crate）；「7」是 R1∪R2 口径的残留，A.1.6 已裁掉 R2 后不成立 |

### A.10.4 状态同步缺口

主线 §5 仍把 ②（多 `path`）、④（`#[path]` 对拍）、⑦（判据落面）标「未决」，而本单元 A.1.1 判据表、A.1.6 已把它们写成「已裁」——**主线表过期**，两处必须同一次改齐，否则 proposal 引用时会拿到互相矛盾的状态。

```text
启动门槛（还差 3 处签字）

  A.2 文件名读法   ─┬─ i  主干+.i.+后缀 ← 全文（含主线 §4 图示、A.7#6）已按此写
  （会进 spec 的   │  ii/iii 全名相套（= 今天）
   硬事实）        └─ 待: 显式落字
  A.3 mod_path_attr ─┬─ 保留为常量真值源（改动最小）
  （存废）           └─ 删函数、留 paths_test 断言（A.3 推荐）
  A.4 TS 透传       ─┬─ 降级到 C（推荐，见 A.10.2）
                     └─ A 内补 ref_relative 同族函数
```

### A.10.5 判定

**可以启动**：范围、依赖、验收、spec delta 落点（`desktop-archi-api` 的「生成物落点与入口查找」整段重写 + `desktop-archi-api-check` 的 `apiEntryMissing` 改述 + `repo-archi-model` 的字段面）都已具形，事实基础逐条复验通过。启动前只需把 A.10.3 两处数字改齐、A.10.4 状态同步、A.2/A.3/A.4 签字。

---

## A.11 签字记录（2026-11-15）

| 岔口 | 裁决 | 依据 |
|---|---|---|
| 接口文件名读法 | **i**：`<入口主干>.i.<后缀>`，落入口同级（`archi/src/lib.rs` → `archi/src/lib.i.rs`；`views/welcome-view.tsx` → `views/welcome-view.i.ts`） | 唯一同时拿到「名字可读 + 落入口同级 + `#[path]` 无 `../`」的读法（A.2） |
| `mod_path_attr` 存废 | **删函数、留断言**：`#[path]` 值退化为固定形态（同目录文件名），等值性由 `paths_test` 直接断言，与 D40/D41「零二次派生」一致 | A.3 |
| TS 透传相对路径 | **降级到变更 C**：A 内 `.i.ts` 生成物为零，两条路无可观测差异；A 只定义规则（入口同级 `./<stem>.i`），跨目录真算留给 C | A.10.2 |

启动门槛结清状态：A.10.3 两处改数已落在正文（A.1.2 = 15、A.7 #7 = 17）；A.10.4 主线 §5 状态已同步；A.9 三处签字已落。

**结论：A 单元具备启动条件**，proposal 可写。写 proposal 时需固定的四样：

1. **范围声明**：模型 `entry` + 解析链三态 + 生成物落点；**出 range** 三条（`scan/owner.rs` 不动——入口同级仍落在元素 `path` 内；TS 透传推 C；`api = none` 的判定只在工具 crate 两例上做）；
2. **spec delta 三面**：`desktop-archi-api`「生成物落点与入口查找」整段重写（含 `#[path]` 形态表、TS specifier、归属双形态表述）、`desktop-archi-api-check`（`apiEntryMissing` 语义改述）、`repo-archi-model`（字段面 + `entry` 形态判据 + 模型数据条目）；
3. **验收**：A.8 五条 + 「`entry` 声明面零 IO（解析链 ①② 步均不读盘，读盘只出现在校验与 `api:check`）」；
4. **迁移提交形态**：生成物迁移与 `#[path]` 改写、`INDEX.toml` 全量重写必须在同一提交内落地，否则 `api:check` 在中间态必然红。
