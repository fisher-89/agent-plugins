# 测试设计: module-entry-field-spec

> **变更**: module-entry-field-spec（模型 `entry` 字段 + 入口解析链三态 + 生成物落点改入口同级 + 归属第三候选）
> **输入**: `proposal.md`（AC-1…AC-9 / R-1…R-9）、`design.md`（D1–D15、事实校准 1–12、变更清单与公共函数 / API 表）
> **性质**: 只设计测试范围与用例，不含评估结论、不含测试运行脚本细节。

测试框架与既有布局（读盘识别结果）：

- **Rust 侧**（`packages/desktop/src-tauri/crates/core/archi`、`crates/infra/agent`）：内置 `#[test]`，测试文件与源文件同目录的 `*_test.rs`（`src/api/paths_test.rs` 等）+ crate 级 `tests/*.rs`（`golden_test` / `pilot_test` / `vocabulary_test` / `perf`）；断言为 `std` 的 `assert!` / `assert_eq!` 族；**零 dev-dependency**，需要盘面事实时用 `std::env::temp_dir()` 自建 `TempDir`（既有模式，沿用到本次全部新用例）。源码自证断言沿用 `include_str!` + `matches(...).count()` 的既有形态。
- **前端侧**（`packages/desktop/src`）：`vite-plus/test`（vitest 兼容 API）+ `@testing-library/react`，`*.test.ts(x)`；断言 `expect`；进程边界用 `vi.mock('@tauri-apps/api/core')`。类型面由 `vp check` 的 typeCheck 执行。
- **mock 纪律**（沿既有 test-design 口径）：核心 crate 的判据事实源（盘面 / 契约 / schema / 符号集 / 模型）一律真实组合，只以真实临时目录驱动存在性探测；前端只替 Tauri IPC 进程边界。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | 解析链三态可判：① 显式 `entry` 命中 / ② 文件型根 = 根文件自身 / ③ 目录型根四候选按序 / ④ `path` 空或候选全不落盘 → `Absent`；**候选派生零 IO** | `crates/core/archi/src/api/paths.rs`（`api/paths_test.rs`）；`crates/core/archi/src/parse/toml.rs`（`parse/toml_test.rs`，`entry` 值供给） |
| AC-2 | 四条判据各一反例（`Malformed` / `OutsidePath` / `NotOnDisk` / `Duplicated` 双方各一条）+ 单文件根声明同目录另一文件 → `OutsidePath` | `crates/core/archi/src/api/paths.rs`（`api/paths_test.rs`）；文案分列面 `crates/core/archi/src/api/check.rs`（`api/check_test.rs`） |
| AC-3 | 落点迁移停在不动点：`crates/infra/archi/src/lib.i.rs` 在盘、旧 `crates/infra/archi.i.rs` 已删、`#[path] = "lib.i.rs"`、`api:check` 三列空、**`cargo test -p archi` 全绿** | 实现面：`crates/core/archi/src/api/{paths,mod,render,diff}.rs`、`crates/infra/archi/src/{lib.rs,lib.i.rs}`；全绿承托（本 change 触及的全部测试文件）：`crates/core/archi/src/api/{paths,check,mod,render,diff,contract}_test.rs`、`api/emit/{mod,rust,ts}_test.rs`、`api/extract/{mod,rust,ts}_test.rs`、`{model,dto,search,graph,docs,diff,edit,tree}_test.rs`、`parse/{toml,mod}_test.rs`、`scan/{mod,owner,rust,ts,exclude}_test.rs`、`drift/{mod,report}_test.rs`、`crates/core/archi/tests/{golden,pilot,vocabulary}_test.rs`、`tests/perf.rs`；`.dev-team/architecture/api/INDEX.toml` 见 `## 不可测试项` |
| AC-4 | 判据面数字与基线逐项相等（`apiFindings = 41 = 15 + 26`，新代码全量判定）、本轮收益（12 个前端单文件根 + 前端壳 `App.Frontend.Shell` 退出 ①、模型数据零结构错）、`status` 与 `passed` 不变；壳退出 ① / 两个工具 crate 零判据 / 分桶回到 12 + 27 属后续变更（宿主重建后），**不在本变更断言面** | `crates/core/archi/src/api/check.rs`（`api/check_test.rs`）、`api/mod.rs`（`api/mod_test.rs`）、`drift/mod.rs`（`drift/mod_test.rs` 的 `api_findings` 槽位）；模型数据见 `## 不可测试项` |
| AC-5 | 归属判据覆盖新落点（第三候选主干对齐）；白名单外后缀四条反例与 `src/dir.i.ts` 并集定档用例逐值不变 | `crates/core/archi/src/scan/owner.rs`（`scan/owner_test.rs`）；树侧标记 `crates/core/archi/src/tree.rs`（`tree_test.rs`）；扫描面 `crates/core/archi/src/scan/mod.rs`（`scan/mod_test.rs`） |
| AC-6 | `entry` 可解析、缺省读作 `null`、拼错键仍 `unknownField`；知识文本元素字段面含 `entry` 且示例 TOML 零诊断；`archi_graph(root)` 仍 51 元素 / 113 关系、元素 `entry` 全 `null`（模型数据零声明，三处显式值归后续变更） | `crates/core/archi/src/{model.rs,parse/toml.rs,dto.rs}`（对应 `*_test.rs`）；`crates/infra/agent/src/archi-knowledge.md`（`archi_knowledge_test.rs`）；`crates/core/archi/tests/golden_test.rs`（语料形态零改写；51 / 113 由桌面端门禁实测） |
| AC-7 | `ArchiElement.entry: string \| null`、`bindings.zod.ts` 同步、`knip` 零新增死代码、`client:check` 绿 | `packages/desktop/src/types/generated/{bindings.ts,bindings.zod.ts}`（`packages/desktop/src/lib/ipc-contract.test.ts`）；fixture 机械补齐：`packages/desktop/src/views/architecture/architecture-view.test.tsx`、`components/{architecture-canvas,element-drawer,element-tree}.test.tsx` |
| AC-8 | 零回归：drift 金样逐字不变（含 `api-mismatch` 的 `apiSignatureMismatch` / `status = warnings` / `modelHash`）；`api/diff.rs` / `api/emit/mod.rs` / `api/emit/ts.rs` 零逻辑改动 | `crates/core/archi/tests/corpus/drift/**`（`tests/golden_test.rs`）、`tests/vocabulary_test.rs`；`crates/core/archi/src/api/{diff,emit/mod,emit/ts}.rs`（`api/diff_test.rs`、`api/emit/{mod,ts}_test.rs`）；`drift/mod.rs`（`drift/mod_test.rs`） |
| AC-9 | 纪律节与真实落点一致：节内要点数恒 5、`元素根同级` 与 `<root>.i` 零命中、两副本节内零 diff | —（`AGENT.md` / `CLAUDE.md`，见 `## 不可测试项`） |
| —（非 AC） | 版本纪律（`version` +1 patch、`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` crate 版本行不随动；proposal 明示不设为 AC） | `packages/desktop/package.json`，见 `## 不可测试项` |

路径前缀 `packages/desktop/src-tauri/` 在上表与下文章节标题中省略为 `crates/`。被测面分两类，两类都必须进范围——否则 AC-3 的「`cargo test -p archi` 全绿」在验收范围与单元测试两处都没有承托：

- **行为面**：design「公共函数 / API」子表逐条映射（各章节的 `#### 待测功能` 逐项对应）。
- **机械面**：`model::Element` / `dto::ArchiElement` 新增 `entry` 后，所有构造型测试字面量与前端 fixture 必须补 `entry: None` / `entry: null`（读盘实测清单见各机械面章节；R-8 的扩散面）。其中 `api/{render,diff}_test`、`api/mod_test`、`paths_test` 的语料还含 **`<file>.tsx/index.ts` 目录形态**——文件型根在新解析链下取根文件自身，该形态必须改写为「根文件本身」，否则语料落 `NotOnDisk`。

零机械面且零行为面的测试文件（`crates/core/archi/src/api/schema_test.rs`、`drift/config_test.rs`）不单列章节，其回归由 AC-3 的全绿门禁整体覆盖。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs

#### 待测功能

- `entry_candidates()`: 解析链候选的唯一产出点——纯文本、零 IO、按优先序 ① 显式 `entry`（归一后）→ ② 文件型根 = 根文件自身 → ③ 目录型根四条约定形态（`src/lib.rs` → `mod.rs` → `index.ts` → `index.tsx`，不做语言判定）→ ④ 空表；不做校验（非法声明原样返回）
- `entry_state()`: 四条判据（基准 / 落位 / 存在 / 跨元素唯一）与「首个落盘候选」的唯一判定点，`Resolved { form, entry }` / `Invalid(EntryProblem)` / `Absent` 三态
- `EntryState` / `EntryProblem`: 三态封闭与四变体载荷（`Malformed` / `OutsidePath` / `NotOnDisk` 各带 `entry`，`Duplicated` 另带 `other_fqn`），派生 `Debug / Clone / PartialEq / Eq` 可直接逐值对拍
- `generated_path()`: 入参由元素根改为入口，产出 `<入口目录>/<入口主干>.i.<ext>`；形态非法 → `None`
- `entry_form()`: **删除**（职责由 `entry_state` 整体接管）
- `mod_path_attr()`: **删除**（`#[path]` 值退化为 `generated_path(...).file_name()`，等值性由本测试文件直接断言）
- `ts_import_specifier()`: 保留为待接线形态（doc 改述，本期零语义变化）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `entry_candidates()` | 正向 | 显式 `entry = "./views/welcome-view.tsx"` → 单元素表 `["views/welcome-view.tsx"]`（归一后），且不追加任何约定候选 | 单例 |
| `entry_candidates()` | 正向 | 文件型根（`path = ["views/welcome-view.tsx"]`、无 `entry`）→ `["views/welcome-view.tsx"]`（根文件自身，不拼 `.tsx`、不列 `index.ts(x)`） | 单例 |
| `entry_candidates()` | 正向 | 目录型根（`path = ["crates/infra/archi/"]`、无 `entry`）→ 四条按序：`…/src/lib.rs`、`…/mod.rs`、`…/index.ts`、`…/index.tsx` | 单例 |
| `entry_candidates()` | 正向 | 目录型根的顺序封闭四形态各自独立可取：只落 `mod.rs` / 只落 `index.ts` / 只落 `index.tsx` / 只落 `src/lib.rs`（候选表恒四条，与盘面无关） | 逐值迭代（4 行） |
| `entry_candidates()` | 边界 | `path = []`（容器）→ 空表；根形态非法（`/abs/x`、`../x`、`C:\x`、`~/x`、`""`、含空段 `a//b`）→ 空表（与既有 `entry_form` 对非法根同口径） | 逐值迭代（7 行） |
| `entry_candidates()` | 边界 | 非源扩展名根（`path = ["foo.py"]`、`["x.txt"]`）归目录型根（四条候选），不按文件型根命中 | 逐值迭代（2 行） |
| `entry_candidates()` | 边界 | 目录内 `<dir>/lib.rs` **不入列**：仅落 `<dir>/lib.rs` 的元素 → 候选表只含四条约定形态、`entry_state` 判 `Absent`（既有 `entry_form排除目录内lib_rs` 用例改写） | 单例 |
| `entry_candidates()` | 边界 | **零 IO 行为面**：候选派生在「候选文件全不在盘」的临时目录下仍返回完整文本候选，且调用前后盘上零新增文件 | 单例 |
| `entry_candidates()` | 边界 | **零 IO 源码自证**：`paths.rs` 内全部 `is_file(` 命中都落在 `entry_state` 体内；`entry_candidates` 函数体内零 `is_file(` / 零 `fs::`（「候选派生零 IO」的机械判据） | 单例 |
| `entry_candidates()` | 异常 | 非法显式声明原样进候选（`entry = "/abs/x.rs"`、`""`、超长 >1000 chars、含 emoji 与 `\n`）→ 逐字返回（不滤空、不 panic；`Malformed` 由 `entry_state` 判） | 逐值迭代（4 行） |
| `entry_state()` | 正向 | 显式 `entry` 落盘且等于根 → `Resolved { form: SingleFile, entry }` | 单例 |
| `entry_state()` | 正向 | 目录型根只落 `<dir>/src/lib.rs` → `Resolved { form: Crate }` | 单例 |
| `entry_state()` | 正向 | 目录型根只落 `<dir>/mod.rs` / `index.ts` / `index.tsx` → `Resolved { form: SubdirModule }`（三形态同一形态位） | 逐值迭代（3 行） |
| `entry_state()` | 正向 | 多文件根（`path = ["app.tsx", …共 4 条]`）→ `Resolved { form: SingleFile, entry: "app.tsx" }`（`<root>` 仍是 `path.first()`） | 单例 |
| `entry_state()` | 正向 | 顺序封闭：`src/lib.rs` 与 `mod.rs` 同时在盘 → 取 `src/lib.rs`；`index.ts` 与 `index.tsx` 同时在盘 → 取 `index.ts` | 逐值迭代（2 行） |
| `entry_state()` | 正向 | 显式优先：显式值落盘且同目录 `src/lib.rs` 也在盘 → 取显式值 | 单例 |
| `entry_state()` | 正向 | 形态位派生优先级：`path = ["crates/x/src/lib.rs"]`（文件型根、入口 == 根）→ `SingleFile`（等值判据先于 `src/lib.rs` 收尾判据） | 单例 |
| `entry_state()` | 异常 | `Malformed`：`entry = "/abs/x.rs"` / `../x.rs` / `C:\x.rs` / `~/x.rs` / `""` → `Invalid(Malformed { entry })`，载荷为归一后原文 | 逐值迭代（5 行） |
| `entry_state()` | 异常 | `OutsidePath`：元素 A 的 `entry` 等于元素 B 的 `path` 条目；或落在 B 的**目录型** `path` 之内 → `Invalid(OutsidePath { entry })` | 逐值迭代（2 行） |
| `entry_state()` | 异常 | `OutsidePath`（单文件根特例）：`path = ["views/welcome-view.tsx"]` 声明同目录 `views/other.ts` → `OutsidePath`（落位判据覆盖，不设第五条判据） | 单例 |
| `entry_state()` | 异常 | `NotOnDisk`：显式 `entry` 落位合法、盘上不存在 → `Invalid(NotOnDisk { entry })`；同目录 `src/lib.rs` 在盘也**不**回落约定候选 | 单例 |
| `entry_state()` | 异常 | `Duplicated`：两个元素声明同一入口 → 双方各 `Invalid(Duplicated { entry, other_fqn })`，`other_fqn` 为对方 FQN | 双元素夹具 |
| `entry_state()` | 异常 | `Duplicated` 的对照面：同一入口只被一个元素声明 → `Resolved`（唯一判据不假阳） | 单例 |
| `entry_state()` | 边界 | `Absent` 三形态：无声明且目录型根四候选全不落盘 / `path` 空 / 根形态非法 → `Absent`（与 `Invalid` 可分辨，三态封闭） | 逐值迭代（3 行） |
| `entry_state()` | 边界 | 唯一判据的保守近似（记在案）：同一目录型根下 `src/lib.rs` 与 `mod.rs` 同时在盘，另一元素显式声明 `mod.rs` → `Duplicated`（只多报不瞒报） | 双元素夹具 |
| `entry_state()` | 边界 | 唯一判据零递归读盘：另一元素的约定候选未落盘也参与候选交集（`entry_candidates` 纯文本）→ 仍判 `Duplicated` | 单例 |
| `entry_state()` | 边界 | `Resolved.form` 与 `Resolved.entry` 同时在场且互不替代：`SubdirModule` 的 `entry` 是 `<dir>/mod.rs` 全串（不是主干，也不是形态名） | 单例 |
| `EntryState` / `EntryProblem` | 边界 | 三态 + 四变体载荷逐字段对拍：`entry` 为归一后原文、`other_fqn` 为冲突方 FQN；`PartialEq` 使两态 / 四变体互不相等（可判红） | 五值表 |
| `generated_path()` | 正向 | crate 入口 `crates/infra/archi/src/lib.rs`（`Rust`）→ `crates/infra/archi/src/lib.i.rs`（入口同级，零 `../`） | 单例 |
| `generated_path()` | 正向 | 子目录模块入口 `src/mod/mod.rs` → `src/mod/mod.i.rs`；单文件入口 `views/welcome-view.tsx`（`TypeScript`）→ `views/welcome-view.i.ts` | 逐值迭代（2 行） |
| `generated_path()` | 正向 | 主干取法：入口 `views/a.b.tsx` → `views/a.b.i.ts`（最后一次 `.` 之前）；无扩展名入口 `lib` → `lib.i.rs`；无目录段入口 `lib.rs` → `lib.i.rs`（仓根，不丢文件名） | 逐值迭代（3 行） |
| `generated_path()` | 异常 | 入口形态非法（绝对路径 / `..` 段 / 盘符 / `~` 起始 / 空串 / 空段）→ `None`（安全边界，零穿越） | 逐值迭代（6 行） |
| `generated_path()` | 边界 | 扩展名只随 `kind`：`Rust` → `.i.rs`、`TypeScript` → `.i.ts`；`kind` 由调用方经 `generated_kind(language_of(entry))` 分派（`Unknown` 归 Rust），本函数零第二次语言判定 | 逐值迭代（3 行） |
| `entry_form()`（删除） | 异常 | `paths.rs` 内零 `fn entry_form(` 定义、零 `entry_form(` 调用；`paths_test.rs` 不再 import 该名；产品 `.rs`（滤 `*_test.rs`）零命中 | 单例 |
| `mod_path_attr()`（删除） | 异常 | `paths.rs` 内零 `fn mod_path_attr(` 定义；产品 `.rs` 的 `mod_path_attr(` **调用形**零命中（注释里的 `paths::mod_path_attr` 词形不构成调用，若实现相位顺带改述 `api/extract/rust.rs` 的 doc 属注释同步、不扩测试面）；`paths_test.rs` 不再 import 该名；既有两组 `mod_path_attr` 用例整体删除（不保留改名副本）；`tests/pilot_test.rs` 对 `mod_path_attr` 的等值断言同批删除 | 单例 |
| `checked_root` 消费面 | 边界 | 源码自证改写：`checked_root` 定义仍恰 1 处、总命中数仍 3（定义 1 + 消费 2），消费方由「`generated_path` + `entry_form`」改为「`generated_path` + 解析链根形态前置（`entry_candidates` / `entry_state`，总数仍 3）」；`is_repo_relative` 仍只经 `checked_root` 一处消费 | 单例 |
| `module_name()` 消费面 | 边界 | 措辞改写：`module_name` 不再是生成物文件名 / `#[path]` 值的来源（改由入口主干派生），只服务透传命名（D40 单点）；函数逐值行为不变（末段不去扩展名 / 归一 / 边界四例） | 单例 |
| reexport 端到端语料 | 正向 | 语料落盘形态改写：TS 消费方 `views/architecture.tsx/index.ts` → `views/architecture.tsx` 文件本身、透传目标 `views/welcome-view.tsx/index.ts` → 根文件本身（旧目录形态在新解析链下为 `NotOnDisk`）；两侧 `.i.*` 在案断言与透传行断言逐字保留 | 单例 |
| reexport 形态位与 specifier | 边界 | 文件型根的 `Resolved.form` 为 `SingleFile`：`ts_import_specifier(SingleFile, "welcome-view.tsx")` = `./welcome-view.tsx.i`（旧 `SubdirModule` 的 `../…` 断言按新形态位改写）；specifier 语义零变化，doc 标注「本期未接线，MUST NOT 当既有真值」（R-7） | 单例 |
| `#[path]` 字面量等值（替代 `mod_path_attr` 的双守断言） | 正向 | `crates/infra/archi/src/lib.rs` 的 `#[path = "…"]` 字面量 == `generated_path(root, "crates/infra/archi/src/lib.rs", Rust).file_name()`；字面量零 `../` | 单例 |
| `element()` 夹具 | 边界 | `ArchiElement` 构造器补 `entry` 入参（缺省 `None`，显式声明用例按需传值）；既有 `path` / `api` 原文直传语义不变 | 单例 |
| `ts_import_specifier()` | 边界 | 三形态逐值不变（`../<name>.i` × 2、`./<name>.i`）；doc 标注「本期未接线，当前输出 MUST NOT 当既有真值」 | 逐值迭代（3 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面（入口 / 生成物存在性） | 不 mock：真实 `TempDir`（`std::env::temp_dir()` + 进程号 + 计数器唯一命名），`touch` / `write` 落零内容文件 | `entry_state` 的 `is_file` 探测、`Absent` / `NotOnDisk` / 顺序封闭用例 |
| `ArchiElement` 夹具 | 不 mock：真实构造器（`path` 原文直传、`entry` 按用例给值），零替身类型 | 全部解析链用例 |
| `crate::scan::is_source_file` | 不 mock：真实调用（文件型根判据的唯一扩展名单源，替身会让「零第二份清单」不可见） | 文件型根 / `OutsidePath` 单文件根特例 |
| 仓库内真实入口文件（`crates/infra/archi/src/lib.rs`） | 不 mock：`CARGO_MANIFEST_DIR` 上溯读真实文件 | `#[path]` 字面量等值用例 |
| 四条判据的返回类型 | 不 mock：`EntryState` / `EntryProblem` 真实值逐字段对拍 | 全部 `entry_state` 用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/check.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/check_test.rs

#### 待测功能

- `check()`: ① 段改经 `paths::entry_state`；`Absent` / `Invalid` ⇒ 一条 `apiEntryMissing`（文案按 `EntryProblem` 分列理由，列出候选 / 越界目标 / 缺失文件 / 冲突方 FQN）；删 `has_generated`；②–⑩ 判据与求值互斥序逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check()` | 正向 | `Resolved` 元素（crate 入口落盘 + 契约在案 + 符号齐）→ ① 零条，②③④ 照常求值且全零 | 单例 |
| `check()` | 正向 | `Resolved` 的单文件根元素（`views/welcome-view.tsx` 自身落盘）→ ① 零条（旧「假报」形态转正） | 单例 |
| `check()` | 正向 | 单文件根的 `api = declared` + 无契约 → ① 零条、② 一条 `apiMissing`（AC-4 可见收益：12 个前端单文件元素由假报转真话） | 单例 |
| `check()` | 边界 | `element()` / `blank_element()` 构造器补 `entry: None`（唯一构造点）；既有四理由 / 互斥序用例的 `entry` 均为 `None`（显式声明面另由 paths_test 承担） | 单例 |
| `check()` | 异常 | 词汇面仍十值：`ArchiApiFindingKind::ALL.len() == 10`，四条判据理由全部复用 `ApiEntryMissing`（MUST NOT 新增 `apiEntryInvalid` 第 11 值）；`category = "entry"`、档位 `warn` 不变 | 单例 |
| `check()` | 正向 | `Absent`（目录型根四候选全不落盘）→ 恰 1 条 `ApiEntryMissing`，`category == "entry"`、`element = Some(FQN)`、`target = None`、`occurrences` 空 | 单例 |
| `check()` | 异常 | `Invalid(Malformed)`（`entry = "/abs/x.rs"`）→ 1 条 ①，文案点名形态非法值 | 单例 |
| `check()` | 异常 | `Invalid(OutsidePath)`（`entry` 落在另一元素 `path` 内）→ 1 条 ①，文案点名越界目标 | 单例 |
| `check()` | 异常 | `Invalid(NotOnDisk)`（`entry` 指向不存在文件）→ 1 条 ①，文案点名缺失文件 | 单例 |
| `check()` | 异常 | `Invalid(Duplicated)`（两元素同入口）→ 两元素各 1 条 ①，文案各点名冲突方 FQN | 双元素夹具 |
| `check()` | 边界 | 互斥序：① 命中（四理由各一次）⇒ ② `apiMissing` / ③ `apiUndeclared` / ④ `apiSignatureMismatch` 全零；⑤ `apiUnexpected` 仍独立判 | 四理由表 |
| `check()` | 边界 | `api = none` + `entry` 有值的工具 crate 形态（`src/main.rs` 落盘、无契约文件）→ ① 已 `Resolved`、② 因 `none` 不判、⑤ 无契约 → **零判据** | 单例 |
| `check()` | 边界 | `has_generated` 退场：`.i.rs` 在盘但无任何可用入口（无声明、约定候选全不落盘）→ 仍报 1 条 ①（既有「生成物在场即不报」用例翻转） | 单例 |
| `check()` | 边界 | 容器元素（`path` 空）零参与（早退不变）；`path` 非空但根形态非法 → 参与且报 ①（`Absent`） | 逐值迭代（2 行） |
| `check()` | 边界 | 规模面：12 个 `Absent` 元素 → 恰 12 条 ①（断言只取构造数，不取仓内真值；AC-4 的 12 由桌面端实测留痕） | 12 行表 |
| `check()` | 异常 | 源码自证：`check.rs` 零 `has_generated`、零 `paths::generated_path(` 调用、零占位 `ArchiElement {` 构造；① 段经 `paths::entry_state`，① 判定不出现 `element.path.first()` | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面（入口 / 生成物 / 契约文件） | 不 mock：真实 `TempDir`（沿用既有 `Fixture.temp`），`touch_entry` / `touch_mod` 落零内容入口 | 三态与四理由用例 |
| 契约与 schema | 不 mock：真实 `contract::parse` + `schema::build_schema`（替身会让「判定单源」不可见） | ②③④ 互斥与 `Resolved` 正向 |
| 符号集 | 不 mock：真实 `ApiSymbol` 夹具（函数 / 类型两形态） | ②③④ 求值 |
| 多元素表 | 不 mock：`Fixture::with_element` 累加真实 `ArchiElement` | `Duplicated` / 规模面 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

#### 待测功能

- `render_all()`: 生成物落点取 `Resolved` 入口（`generated_path(root, entry, kind)`）；无可用入口（`Absent` / `Invalid`）不产 `.i.*`
- `bootstrap()`: 入口经 `entry_state`；失败文案列出候选与判据理由
- `api_ref()`: 取全元素表后委派 `api_ref_of`，元素缺席 / 模型错时仍报契约事实
- `api_ref_of()`: 第三参由 `element_path: &[String]` 改为 `(elements, element)`；投影与判定走同一入口解析

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `render_all()` | 正向 | crate 形态：产出集合含 `crates/infra/archi/src/lib.i.rs`，且逐字等于 `generated_path(root, "crates/infra/archi/src/lib.rs", Rust)` | 单例 |
| `render_all()` | 正向 | 单文件根 TS 元素（`views/welcome-view.tsx` + 契约）→ `.i.ts` 落 `views/welcome-view.i.ts`，`kind = TypeScript`（入口扩展名分派） | 单例 |
| `render_all()` | 正向 | 目录型根 `src/mod/mod.rs` → `src/mod/mod.i.rs`；`EmitTarget` 的 `form` 取自 `Resolved.form`（三值构造仍只 `kind` / `form` / `modules`） | 单例 |
| `render_all()` | 正向 | 多文件根（`path` 首条 `app.tsx`）→ 落点 `…/app.i.ts`（`path.first()` 作根、文件型根 ② 命中） | 单例 |
| `render_all()` | 异常 | `Absent` 元素有契约有内容 → **零** `.i.*`（与「零内容不生成」同口径），且该元素 `INDEX` 条目 `generated` 留空 | 单例 |
| `render_all()` | 异常 | `Invalid(NotOnDisk)` 元素 → 零 `.i.*`；盘上既有 `.i` 文件不使其复位为「有入口」 | 单例 |
| `render_all()` | 边界 | 幂等：同一快照两次渲染逐字段相等；落点只随入口，与盘上生成物是否存在无关 | 单例 |
| `render_all()` | 边界 | `INDEX.toml` 的 `generated` 列：`ArchiRuntime` 为新落点、无入口元素为空串（一条元素级断言） | 单例 |
| `render_all()` | 异常 | 源码自证：函数体内零 `let scope = element.path.first()`；落点入参唯一来源是 `entry_state` 的 `Resolved.entry`；`EmitTarget {` 字面量仍恰 1 处 | 单例 |
| `render_all()` 语料改写 | 边界 | 文件型根语料落盘形态改写：`views/welcome-view.tsx/index.ts`（目录）→ `views/welcome-view.tsx` 文件本身；`views/consumer.tsx/index.ts` 两处同改（旧形态新链下 `NotOnDisk` ⇒ 零 `.i.ts`），落点断言 `views/welcome-view.i.ts` / `views/consumer.i.ts` | 三处夹具 |
| `render_all()` 形态位 | 边界 | `ts_import_specifier` 的 `form` 取自 `Resolved.form`：文件型根 → `SingleFile`，specifier 断言由 `../welcome-view.tsx.i` 改写为 `./welcome-view.tsx.i`；`form` 仍非渲染器推导（`paths::entry_state` 单点），specifier 语义零变化（R-7 待接线） | 单例 |
| `element()` / `write_element()` 夹具 | 边界 | `ArchiElement` 构造器补 `entry: None`（唯一构造点，`write_element` 复用）；`bootstrap` 的 `crates/core/no-entry/` 语料仍 `Absent`，`ApiError::Element` 文案列出候选 | 单例 |
| `bootstrap()` | 正向 | `Resolved` 元素草稿的 `@ <entry>:line` 注释用 `Resolved.entry`（crate 形态与单文件形态各一） | 逐值迭代（2 行） |
| `bootstrap()` | 异常 | `Absent` → `ApiError::Element`，文案列出四条约定候选（目录型根）或「无候选」（`path` 空） | 逐值迭代（2 行） |
| `bootstrap()` | 异常 | `Invalid` 四理由 → `ApiError::Element` 文案各带理由（形态非法值 / 越界目标 / 缺失文件 / 冲突方 FQN） | 四理由表 |
| `api_ref()` | 正向 | 已声明契约元素 → 三路径齐备，`generated_path` 为入口同级串（`…/src/lib.i.rs`） | 单例 |
| `api_ref()` | 边界 | 元素缺席 / 模型结构错 → 契约与 schema 事实仍报，`generated_path = None`（`ref_projection` 承接，不构造占位元素） | 逐值迭代（2 行） |
| `api_ref()` | 边界 | 入口 `Absent` 的在案契约元素 → `status` 仍按文件事实 `declared`，`generated_path = None`（投影只报文件事实，不回写模型字段） | 单例 |
| `api_ref_of()` | 正向 | `(root, elements, element)` 与 `api_ref(root, fqn)` 逐字段相等（两形态同源） | 单例 |
| `api_ref_of()` | 边界 | 多 `path` 只取 `path.first()` 派生入口；`Absent` → `generated_path = None`；`Invalid` → `None`（零占位元素） | 逐值迭代（3 行） |
| `api_ref_of()` | 异常 | 传入元素与元素表不同源（元素不在表内 / 表内有同 FQN 的另一元素）→ 契约与 schema 按 `element.fqn`、生成物按该元素入口，互不污染（不静默改判） | 逐值迭代（2 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型与盘面 | 不 mock：真实 `TempDir` + `write_model`（`declare_ancestors` 补祖先链）+ 真实入口文件 | `render_all` / `bootstrap` / `api_ref` 全族 |
| 契约装载与渲染 | 不 mock：真实 `load_contracts` / `emit::render_target`（`EmitTarget` 两值只来自 `Resolved`） | 落点与幂等用例 |
| 元素表 | 不 mock：`dto::graph_view` 真实取数（`elements_of` 单点） | `INDEX` 条目与 `api_ref_of` 用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/owner_test.rs

#### 待测功能

- `OwnerIndex::build()`: 每条前缀额外算 `stem`（`is_source_file(prefix)` 为真时 = 去最后一次扩展名的前缀；目录型为 `None`），`module_roots` / `contains_count` 语义不变
- `OwnerIndex::owner_of()`: 候选集扩为三候选（`Prefix` 整组剥离 / `Prefix` 替换扩展名 / `Stem` 主干对齐），对每候选各跑一遍最长前缀后合并去重、最长档定档、同长仍 `Ambiguous`（**MUST NOT 候选短路**）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `owner_of()` | 正向 | `crates/infra/archi/src/lib.i.rs` → `App.Backend.Infra.ArchiRuntime`（整组剥离候选落在目录前缀内） | 单例 |
| `owner_of()` | 正向 | `views/welcome-view.i.ts` → `Views.Welcome`（文件型根 `views/welcome-view.tsx`，第三候选主干对齐） | 单例 |
| `owner_of()` | 正向 | `components/agent/index.i.ts` → `Components.Agent`（容器 barrel 的文件型 `path`） | 单例 |
| `owner_of()` | 正向 | `src/a.i.rs` → 单文件 `.rs` 根 `src/a.rs`（主干对齐，与候选 2 的 `.ts` 形态互不干扰） | 单例 |
| `owner_of()` | 正向 | 目录型根 `src/mod/mod.i.rs` → 替换扩展名候选命中（既有绿，第三候选不改变结论） | 单例 |
| `owner_of()` | 正向 | `src/dir.i.ts` 并集定档：`src/dir/` 与 `src/dir.ts` 同声明 → 更具体的文件型根胜出（禁止候选短路） | 单例 |
| `owner_of()` | 边界 | 同主干两元素 `src/a.ts` / `src/a.tsx` 声明于同一目录 → `src/a.i.ts` 上 `Ambiguous`（全量字典序，MUST NOT 任选） | 单例 |
| `owner_of()` | 边界 | **既有 `Owner::None` 用例翻转**：文件型根 `views/welcome.ts` 旁的 `views/welcome.i.{rs,tsx,js,jsx}` 由第三候选（主干对齐，不比对扩展名）命中 `App.View.Welcome`；旧「两候选均不命中 → 无主」断言按新语义改写为 `Element`（白名单外四条仍 `None`） | 逐值迭代（4 行） |
| `owner_of()` | 边界 | 白名单外后缀 `.i.py` / `.d.rs` / `.i` / `.I.rs` → 零剥离、归属与改动前逐值相等 | 逐值迭代（4 行） |
| `owner_of()` | 边界 | 非 `.i` 路径归属零变化（目录型 / 文件型 / 空元素表 / 无命中四表） | 四表迭代 |
| `owner_of()` | 边界 | 空元素表 / 元素 `path` 为空条目 → `Owner::None`；300 元素规模下重叠前缀仍稳定 | 逐值迭代（2 行） |
| `owner_of()` | 边界 | 同一 `stem` 由两元素共享（`src/x.ts` 与 `src/x.tsx` 同时在场）+ 目录型根 `src/x/` → 合并去重后仍按最长档给全量候选（不因去重退化） | 三元素夹具 |
| `build()` | 正向 | `stem` 只在 `is_source_file(prefix)` 为真时有值（`.rs` / `.ts` / `.tsx` / `.js` / `.jsx` 五扩展名逐值），且等于去最后一次扩展名的前缀 | 逐值迭代（5 行） |
| `build()` | 边界 | 目录型 `path`（尾 `/`）/ 无扩展名 `path` / 非白名单扩展名（`.py`）→ `stem = None`，主干候选对其零命中 | 逐值迭代（3 行） |
| `build()` | 边界 | 同 FQN 多 `path` 条目在 `stem` 面只计一条（去重沿用既有），`module_roots` / `contains_count` 逐值不变 | 单例 |
| `build()` | 异常 | 源码自证：`owner.rs` 零第二份扩展名清单（白名单字面只出现于 `GENERATED_EXTENSIONS`）；`scan::is_source_file` 为文件型判据唯一点；`normalize` / `prefix_matches` / `module_roots` / `contains_count` 零改动 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 元素表 | 不 mock：真实 `ArchiElement` 构造器夹具（零盘面，纯文本判据） | 全部归属用例 |
| `crate::scan::is_source_file` | 不 mock：真实调用（`build` 零读盘，替身会抹掉「扩展名清单单源」判据） | `stem` 派生 |
| 无进程边界依赖 | — | 本模块纯函数零 IO，Mock策略表只留「不 mock」声明 |

---

### packages/desktop/src-tauri/crates/core/archi/src/model.rs -> packages/desktop/src-tauri/crates/core/archi/src/model_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行；下列条目取自同文档「类型定义」子表的 `Element.entry` 一行，非凭空虚构。 -->

#### 待测功能

- `Element.entry`: 单值可选字段（`Option<String>`），插在 `api` 之后、`path` 之前；参与派生相等语义

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Element.entry` | 正向 | 字段数恰 12（原 11 + `entry`）；`pub entry: Option<String>` 恰一处；源码序为 `api` 之后、`path` 之前 | 单例 |
| `Element.entry` | 边界 | 缺省 `None`；显式值逐字保留（`"./packages/…"` 的前导 `./` 不剥，归一归 `api::paths`） | 逐值迭代（2 行） |
| `Element.entry` | 边界 | 与 `api` / `path` 正交：`entry` 有值与 `api = none`、`entry` 缺省与 `api = declared` 四组合逐项可读 | 四组合表 |
| `Element.entry` | 异常 | 派生相等语义：两元素仅 `entry` 不同 → 不相等（`PartialEq` 参与，`Model::element` 查表不受影响） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 AST | 不 mock：真实 `Element` 直构 + `parse_with_models` 真实解析面 | 字段面与相等语义 |
| 无进程边界依赖 | — | 本模块零 IO |

---

### packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行；下列条目取自同文档「类型定义」子表的 `RawElement.entry` 一行，非凭空虚构。 -->

#### 待测功能

- `RawElement.entry`: `#[serde(default)] entry: Option<String>`，`to_model` 搬运；数组 / 表形态由 serde 归 `invalidValue`，拼错键仍 `unknownField`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `RawElement.entry` | 正向 | `entry = "packages/desktop/src-tauri/src/lib.rs"` 解析进模型；省略键 → `None`；`"./…"` 原文保留 | 逐值迭代（3 行） |
| `RawElement.entry` | 异常 | `entry = ["a"]`（数组）/ `entry = { path = "x" }`（表）→ `invalidValue`（零元素产出，文案含类型信息） | 逐值迭代（2 行） |
| `RawElement.entry` | 异常 | 拼错键 `entri = "x"` / `entries = "x"` → `unknownField`（`deny_unknown_fields` 仍生效，不因新字段放宽） | 逐值迭代（2 行） |
| `RawElement.entry` | 边界 | `entry = ""` / 超长（>1000 chars）/ 特殊字符（`\n`、emoji、`\0` 转义）→ 逐字保留、零诊断、不 panic | 逐值迭代（3 行） |
| `RawElement.entry` | 边界 | 与 `api` / `path` / `gate` 正交：四组合逐元素可读（`api = none` + `entry` 有值不互相吞并） | 四组合表 |
| `RawElement.entry` | 边界 | `entry` 与 `metadata = { entry = "…" }` 同名键互不影响（未知键归 metadata） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| TOML 解析器 | 不 mock：真实 `TomlParser`（替身会让 serde 分类判据不可见） | 全部用例 |
| 无进程边界依赖 | — | 本模块零 IO |

---

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行；下列条目取自同文档「类型定义」子表的 `ArchiElement.entry` 一行（投影搬运与 `module_detail` 调 `api_ref_of` 见同文档「修改文件」表），非凭空虚构。 -->

#### 待测功能

- `ArchiElement.entry`: 线面追加末尾的显式字段（缺省 `null`、不省略）；`to_element` 搬运；`module_detail` 以元素表投影后调 `api_ref_of`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiElement.entry` | 正向 | `graph_view`：显式 `entry` 逐字投影；缺省元素出 `"entry": null`（显式字段不省略，`contains_key` 为真） | 双元素夹具 |
| `ArchiElement.entry` | 正向 | `to_element` 搬运：解析 → 投影逐字相等（含 `./` 原文，不做第二次归一） | 单例 |
| `ArchiElement.entry` | 正向 | `module_detail` 投影：`generatedPath` 为入口同级串；元素表只投影一次（`to_element` 覆盖全部元素后 `find` 当前元素） | 单例 |
| `ArchiElement.entry` | 边界 | 字段面：`ArchiElement` 字段数 +1、`entry` 追加在 `api` 之后（末尾），线名 `entry`（`rename_all = camelCase` 下无变化） | 单例 |
| `ArchiElement.entry` | 边界 | 线键序断言改写：既有 10 键序（`fqn` … `mark` / `api`）之后追加 `entry`（dto_test 的 `element_keys` 期望表由 10 项改 11 项）；`api` 不再是末位字段 | 单例 |
| `ArchiElement.entry` | 异常 | `api_ref_of` 调用面改写：`dto_test` 的 `declared_ref` 由 `api_ref_of(root, fqn, &element.path)` 改为 `api_ref_of(root, elements, element)`；三路径断言（含入口同级 `generatedPath`）逐字保留 | 单例 |
| `ArchiElement.entry` | 边界 | 与 `module_detail` 的 `api` / `apiRef` 投影同源：`api_ref` 与 `module_detail.apiRef` 逐字段相等（含 `Absent` → `generatedPath = null`） | 单例 |
| `ArchiElement.entry` | 异常 | 停摆态（结构诊断 / `models/` 缺席）：`elements` 空、零 `entry` 泄漏、不 panic（既有 fail-closed 不变） | 逐值迭代（2 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型目录与契约文件 | 不 mock：真实 `TempDir` + `write` 落 `.toml` / `.md` | 投影与停摆态用例 |
| `graph_view` / `module_detail` | 不 mock：真实投影入口（`to_element` 单点） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/diff_test.rs

<!-- design.md 未声明该文件的公共 API 变更（本次仅 doc 样例改述，零逻辑改动）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；本段只设计「doc 与真实落点一致 + 行为零变化」的机械断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `api/diff.rs` doc 样例 | 边界 | 源码内旧落点样例（`crates/infra/archi.i.rs`、`views/welcome-view.tsx.i.ts`）零命中；新样例为 `crates/infra/archi/src/lib.i.rs` / `views/welcome-view.i.ts` | 单例 |
| `api/diff.rs` 行为 | 边界 | `orphaned` 按命名判据（`*.i.rs` / `*.i.ts`）识别，与落点目录深度无关：既有 `compare` / `generated_files_on_disk` 用例保持绿 | 单例 |
| 语料落盘形态 | 边界 | `write_two_language_corpus` 的文件型根语料改写：`views/welcome-view.tsx/index.ts`（目录）→ `views/welcome-view.tsx` 文件本身（若沿用旧目录形态，新链下该元素 `NotOnDisk` ⇒ 生成物由 5 份掉到 4 份——该差异本身是被测语义）；`generated_path` 调用由 `(root, "<root>", kind)` 改 `(root, "<entry>", kind)`（`crates/core/archi/src/lib.rs` / `views/welcome-view.tsx`），`assert_clean` 基线重建 | 单例 |
| 旧落点游离文件 | 异常 | 迁移后旧落点 `crates/infra/archi.i.rs` 若在盘仍按命名判据入 `orphaned`（不因搬迁移获得豁免）；新落点 `crates/infra/archi/src/lib.i.rs` 属预期集 ⇒ 不入列 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面差异集 | 不 mock：真实 `TempDir` + 既有 diff 夹具 | 行为零变化回归 |
| 源码 doc | 不 mock：`include_str!("diff.rs")` 自证 | 样例改述断言 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更（本次仅注释改述，零逻辑改动）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；本段只设计「旧词形退场 + 形态位消费面不变」的机械断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `emit/mod.rs` doc | 边界 | 文件内 `paths::entry_form` 词形零命中；`EmitTarget.form` 的 doc 表述为「来自 `paths::entry_state` 的 `Resolved` 形态位」 | 单例 |
| `emit/mod.rs` 行为 | 边界 | 零逻辑改动：`EmitTarget` 三字段与两路分派断言逐值不变（既有用例保持绿） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 渲染器两路 | 不 mock：真实 `emit::render_target` / `rust::render` / `ts::render` | 分派面回归 |
| 源码 doc | 不 mock：`include_str!("mod.rs")` 自证 | 词形退场断言 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts_test.rs

<!-- design.md 未声明该文件的公共 API 变更（本次仅注释改述，零逻辑改动）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；本段只设计「doc 改述 + specifier 行为零变化」的机械断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `emit/ts.rs` doc | 边界 | `render` / `render_scope` 的 doc 内 `entry_form` 引用改述为 `entry_state`；补「`ts_import_specifier` 入口同级真值归前端契约变更（待接线）」一句；`ts_test.rs` 自身注释里的 `paths::{entry_form, …}` 引用同批改述 | 单例 |
| `emit/ts.rs` 行为 | 边界 | 零逻辑改动：`ts_import_specifier` 三形态与 `EntryForm` 消费面逐值不变（既有 specifier 断言保持绿） | 逐值迭代（3 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 渲染器 | 不 mock：真实 `ts::render` / `render_scope` | 行为零变化 |
| 源码 doc | 不 mock：`include_str!("ts.rs")` 自证 | 改述断言 |

---

### packages/desktop/src-tauri/crates/infra/archi/src/lib.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs

<!-- design.md 未声明该文件的公共 API 变更（入口编排文件，无公共函数）；下列用例来自 AC-3 的机械断言（`#[path]` 字面量 == `generated_path().file_name()`），沿既有跨 crate 自证模式。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是入口行的字面量与注释表述，由 `paths_test` 跨 crate 读文件断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `#[path]` 入口行 | 正向 | 字面量恰为 `"lib.i.rs"`（零 `../`），且 == `generated_path(root, "crates/infra/archi/src/lib.rs", Rust).file_name()` | 单例 |
| 入口行注释 | 边界 | 零「与 `mod_path_attr` 输出等值」与「`mod_path_attr` 零产品调用者」旧句；含「入口同级」「值恒为文件名」表述 | 单例 |
| 旧落点 | 异常 | `crates/infra/archi.i.rs` 不在盘（旧位置清空，`api:check` 的 `orphaned` 列由此为空） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 仓库真实入口文件 | 不 mock：`CARGO_MANIFEST_DIR` 上溯读真实 `crates/infra/archi/src/lib.rs` | 字面量与注释断言 |
| 生成物路径 | 不 mock：真实 `generated_path` 派生 | 等值性断言 |

---

### packages/desktop/src-tauri/crates/infra/archi/src/lib.i.rs -> packages/desktop/src-tauri/crates/core/archi/tests/pilot_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行；下列条目取自同文档「类型定义」子表的 `LocalArchiCheck` 一行（位置迁移、内容逐字不变），非凭空虚构。 -->

#### 待测功能

- `LocalArchiCheck`: 生成类型 + 自动构造器，随生成物搬到入口同级（`crates/infra/archi/src/lib.i.rs`），`new()` 与签名见证 `const _` 逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 落点 | 正向 | `crates/infra/archi/src/lib.i.rs` 在盘；`crates/infra/archi.i.rs` 不在盘（同一断言内双向） | 单例 |
| 内容 | 正向 | 盘上内容逐字节等于 `emit::rust::render` 重算结果（纯派生，零手改） | 单例 |
| `LocalArchiCheck` | 边界 | `LocalArchiCheck::new()` 可调用、签名见证 `const _` 在场（编译面，零运行期断言） | 单例 |
| `#[path]` 等值断言 | 异常 | 既有对 `paths::mod_path_attr(EntryForm::Crate, "archi")` 的等值断言随函数删除；替换为「入口行字面量 == `generated_path(root, "crates/infra/archi/src/lib.rs", Rust).file_name()`」（工程面）：`pilot_test.rs` 内 `mod_path_attr` 零命中 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 仓库真实生成物 | 不 mock：真实读盘 + 真实 `render` 重算（替身会让「纯派生」不可见） | 落点与内容 |
| 契约 / schema | 不 mock：真实 `parse` / `build_schema` / `meta` | 重算链 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/tests/pilot_test.rs

#### 待测功能

- `render_all()` / `api_ref()` / `api_ref_of()` 的**在盘事实**：试点三件资产（契约 / schema / `.i.rs`）与 `INDEX.toml` 的落点、投影三路径在真实仓库内逐字成立

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 试点三件资产落点 | 正向 | 契约 / schema / `.i.rs` 三路径均由 `paths` 单点派生，`.i.rs` 为入口同级（`…/archi/src/lib.i.rs`） | 单例 |
| `INDEX.toml` 在盘值 | 正向 | `App.Backend.Infra.ArchiRuntime.generated` == `crates/infra/archi/src/lib.i.rs`；其余 41 条逐字不变（重算对拍） | 单例 |
| `api_ref` 投影 | 正向 | 三路径与单点派生逐字相等（`generatedPath` 为新落点串） | 单例 |
| `api_ref_of` 新签名 | 边界 | 容器元素（`path` 空）/ 入口 `Absent` 元素 / 契约缺席三形态可辨，`generatedPath = None` 不 panic | 逐值迭代（3 行） |
| 生成物见证 | 边界 | `LocalArchiCheck` 在 `.i.rs` 中且手写 `impl` 与生成物共存（既有见证用例保持绿） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 仓库内真实资产 | 不 mock：真实读盘（契约 / schema / `.i.rs` / `INDEX.toml`），零写盘 | 全部用例 |
| `foundation::layout::archi_api_dir` | 不 mock：真实派生 | 路径前缀断言基准 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（静态知识文本）；下列条目按 AC-6 与同文档「修改文件」表的知识文本改动设计，非凭空虚构。 -->

#### 待测功能

- 元素字段面：八项（`kind` / `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata`）逐词在场，`entry` 三条纪律在场
- 示例 TOML：`App.Auth` 含单值 `entry`，经真实 `TomlParser` 与 `ModelGraph` 零诊断（既有自证链路）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 元素字段面 | 正向 | 字段面一行逐词在场：`kind` / `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata`（八项，`api` 与 `entry` 同批补齐） | 单例 |
| `entry` 纪律 | 正向 | 三条纪律语义在场：单值 / 只在约定默认指不到时写 / 单文件根 MUST NOT 写 | 单例 |
| 示例 TOML | 正向 | 示例含单值 `entry`，`TomlParser` 零诊断 + `ModelGraph` 零诊断（既有 `SKELETON` 自证用例保持绿） | 单例 |
| 知识面边界 | 边界 | 骨架字节 ≤ 3KB（既有 `体量切分骨架不超3kb` 判据，非 4KB）、零 `<>` 占位与模板片、零 `openspec` / `architecture` / `models/` / `decisions` 字面（既有边界保持绿） | 单例 |
| 字段面追加非替换 | 边界 | 既有六项（`kind` / `gate` / `path` / `docs` / `summary` / `metadata`）仍在枚举行、`kind` 仍标「必填」，`api` 与 `entry` 是**追加**；`api` 二值定义行与 `ApiStatus::ALL` 逐值在场断言不变 | 单例 |
| 判据长表留白 | 异常 | 四条判据的变体名（`Malformed` / `OutsidePath` / `NotOnDisk` / `Duplicated`）与 `apiEntryMissing` 判据表**零命中**（判据长表 MUST NOT 进知识文本，只指路） | 单例 |
| 边界段表述 | 边界 | 「边界」段以「两个指针」表述 `api` 与 `entry`，且零契约正文示例（协议正文不进知识文本） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 知识文本字节 | 不 mock：`include_str!("archi-knowledge.md")`（与生产 `include_str!` 同一份文件，禁测试内副本） | 全部文本断言 |
| 解析与图 | 不 mock：真实 `TomlParser` / `ModelGraph` | 示例 TOML 自证 |
| 词表常量 | 不 mock：真实 `ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL` | 既有词表在场断言 |

---

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src/lib/ipc-contract.test.ts

<!-- design.md 的「公共函数 / API」子表未列本文件的行（机械再生成物）；下列条目取自同文档「类型定义」子表的 `ArchiElement.entry` 一行，非凭空虚构。 -->

#### 待测功能

- `ArchiElement.entry`: 生成线面字段（`string \| null`，追加末尾），与 `bindings.zod.ts` 的 `ArchiElementSchema` 同源；前端 fixture 的机械补齐

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiElement.entry` 类型 | 正向 | `ArchiElement['entry']` 为 `string \| null`；`ArchiElementSchema` 的 shape 含 `entry`（`expectTypeOf` + 运行期键存在） | 单例 |
| `ArchiElement.entry` 取值 | 边界 | 两态经 `ArchiElementSchema.parse` 均通过：`entry: null`、`entry: 'packages/desktop/src-tauri/src/lib.rs'` | 逐值迭代（2 行） |
| `ArchiElement.entry` 缺失 | 异常 | 未补 `entry` 键的对象被 `strictObject` 拒（机械证明 fixture / 夹具必须补键） | 单例 |
| 前端 fixture 补齐（R-8） | 边界 | 读盘实测的四个文件各有一处 `element(...)` 构造器：`views/architecture/architecture-view.test.tsx`、`views/architecture/components/{architecture-canvas,element-drawer,element-tree}.test.tsx` → 字面量补 `entry: null`，`vp check` 类型检查绿、既有断言零改动 | 四文件批 |
| 死代码面 | 边界 | `knip` 零新增死代码：`entry` 由既有消费者（类型面 + fixture）真实读取，无 test-only 导出 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | `vi.mock`（既有 `invokeMock`，按命令名分发） | 契约等价面里的 `ipc` 代理往返（`entry` 断言不依赖它） |
| `bindings.ts` / `bindings.zod.ts` | 不 mock：真实 import（类型主权不转移，替身会让等价断言失效） | 类型与 schema 同源 |
| zod | 不 mock：运行期单实例（`.meta()` / `instanceof` 可观测的前提） | schema 解析断言 |

---

### packages/desktop/src/types/generated/bindings.zod.ts -> packages/desktop/src/lib/ipc-contract.test.ts

<!-- design.md 的「公共函数 / API」子表未列本文件的行（机械再生成物）；下列条目取自同文档「类型定义」子表的 `ArchiElement.entry` 一行与「路由 / API 设计」的 zod 同步要求，非凭空虚构。 -->

#### 待测功能

- `ArchiElementSchema.entry`: `z.string().nullable()`，与 `bindings.ts` 的 `ArchiElement.entry` 互相可赋值；`strictObject` 的严格性不因新增字段放宽

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiElementSchema.entry` | 正向 | shape 含 `entry: z.string().nullable()`；`z.infer<typeof ArchiElementSchema>` 与 `ArchiElement` 互相可赋值 | 单例 |
| schema 单实例 | 边界 | `ArchiGraphSchema` 的 `get elements()` 与 `ArchiModuleDetailSchema` 的 `get element()` 引用同一 `ArchiElementSchema`（零镜像副本） | 单例 |
| 严格性 | 异常 | 多余字段（如 `entryPath`）仍被 `strictObject` 拒；`entry` 之外无字段面变化 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `bindings.zod.ts` | 不 mock：真实 import | 全部用例 |
| 类型断言 | 不 mock：`expectTypeOf` 编译期断言（`vp check`） | 互相可赋值 |

---

### packages/desktop/src-tauri/crates/core/archi/tests/corpus/drift/ -> packages/desktop/src-tauri/crates/core/archi/tests/golden_test.rs

<!-- design.md 未声明该目录的公共 API 变更（回归语料，零改写）；本段只设计「既有金样与词汇面逐字不变」的回归断言。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是既有语料的零改写事实。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| drift 语料快照 | 边界 | 全部 case 快照逐字不变；`api-mismatch` 仍产 `apiSignatureMismatch`、`status = warnings`、`modelHash` 不变（其 fixture 的 `src/mod/mod.rs` 由目录型约定候选命中） | 全量批 |
| 词汇面夹具（同批机械面） | 边界 | 同 crate 的 `tests/vocabulary_test.rs` 机械面（`ArchiElement` 字面量补 `entry: None`）见专章 `tests/vocabulary_test.rs -> 自身`；本文件不重复其用例 | 引用 |
| 全仓结构面（可选前置） | 边界 | 语料 fileset 与登记面互为镜像（既有用例），本变更零新增 case、零 register 改动 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 语料与暂存根 | 不 mock：真实复制 `<case>/*.toml` 到临时 `.dev-team/architecture/models/` | 全量快照对拍 |
| 判定链 | 不 mock：真实 `drift::check` | 快照生成 |

---

### packages/desktop/src-tauri/crates/core/archi/src/parse/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/mod_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`parse/mod.rs` 的聚合面零改动）；proposal 的「测试文件」表把 `parse/mod_test.rs` 列入机械面，本段按该表建立映射。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是「新增 `entry` 字段不改聚合面」这一机械事实。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `parse_dir` 聚合面 | 边界 | 多文件语料各含显式 `entry` / 缺省 → 元素聚合序与 `duplicateElement` / `unknownField` / `missingField` 诊断逐值等于改动前（本文件零字面量改动） | 单例 |
| 解析产物字段面 | 边界 | 聚合出的 `Element.entry` 逐元素保留（显式值原文 / `None`），不参与聚合键、不进任何诊断 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面与解析器 | 不 mock：真实 `TempDir` + 真实 `TomlParser`（替身会让聚合序判据不可见） | 两行用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/search.rs -> packages/desktop/src-tauri/crates/core/archi/src/search_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`search.rs` 零改动）；`entry` 只落在 `model::Element` 的机械面（`search_test.rs:62` 的唯一构造器）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是 `Element` 构造器补 `entry: None` 后的零行为变化。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Element` 夹具 | 边界 | `element()` 构造器补 `entry: None`（唯一构造点）；既有命中集 / `SearchOutcome` 断言逐值不变 | 单例 |
| `search()` 字段面 | 边界 | `entry` 不参与检索：两元素仅 `entry` 不同 → 命中集与文档条目逐值相同（搜索面无 `path` / `api` / `entry` 字段） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 元素 / 文档夹具 | 不 mock：真实 `Element` / `DocRef` 直构 + 真实 `ModelGraph` | 全部用例 |
| 无进程边界依赖 | — | 本模块纯文本检索，零 IO |

---

### packages/desktop/src-tauri/crates/core/archi/src/graph.rs -> packages/desktop/src-tauri/crates/core/archi/src/graph_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`graph.rs` 零改动）；`entry` 只落在 `model::Element` 的机械面（`graph_test.rs:16` 的唯一构造器）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是「结构诊断四类判据不看 `entry`」的回归面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Element` 夹具 | 边界 | `element()` 构造器补 `entry: None`；既有环 / 孤儿层级 / 未定义引用四类诊断用例逐值不变 | 单例 |
| `ModelGraph` 结构面 | 边界 | 两元素仅 `entry` 不同（含 `"/abs/x.rs"` 一类任意值）→ 诊断集合逐值相同（`entry` 违反判据走 `apiFindings`，不进九值诊断） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 AST | 不 mock：真实 `Element` / `Relation` 直构 | 全部用例 |
| 无进程边界依赖 | — | 本模块零 IO |

---

### packages/desktop/src-tauri/crates/core/archi/src/docs.rs -> packages/desktop/src-tauri/crates/core/archi/src/docs_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`docs.rs` 零改动）；`entry` 只落在 `model::Element` 的机械面（`docs_test.rs:70` 的构造器）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是「文档关联不看 `entry`」的回归面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Element` 夹具 | 边界 | `element()` 构造器补 `entry: None`；既有强 / 弱关联用例逐值不变 | 单例 |
| 文档关联面 | 边界 | 两元素仅 `entry` 不同 → 文档条目与强度序逐值相同（关联判据只吃 `docs` / `path` 与目录事实） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面文档与模型 | 不 mock：真实 `TempDir` + 真实 `Element` 直构 | 两行用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/diff.rs -> packages/desktop/src-tauri/crates/core/archi/src/diff_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`diff.rs` 零改动）；`entry` 只落在 `model::Element` 的机械面（`diff_test.rs:13` / `:29` 两个构造器）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是「`entry` 不进结构 diff」的回归面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `element()` / `gated()` 夹具 | 边界 | 两个构造器补 `entry: None`；既有元素增删 / 关系计数 / `gate` 变更用例逐值不变 | 单例 |
| `model_diff()` 字段面 | 边界 | base 与 next 仅 `entry` 不同（`None` ↔ `Some("...")`）→ `ModelDiff::is_empty()` 为真（元素身份仍 FQN、变更面只 `gate`） | 单例 |
| `model_diff()` 规模面 | 边界 | 500 元素全带 `entry` → 与全 `None` 语料给出逐字段相同的 `ModelDiff`（新增字段零噪声） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 AST | 不 mock：真实 `Model` / `Element` 直构（纯函数零 IO） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/edit.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`edit.rs` 零逻辑改动）；proposal 的「测试文件」表把 `edit_test.rs` 列入机械面，本段补「写面不吞新字段」的用例。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是 `set_gate` 对含 `entry` 源文的保留语义。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `set_gate()` | 正向 | 源含 `entry = "..."` → 改档后该行逐字保留（`toml_edit` 保留未知键），渲染自证（`TomlParser` 读回）通过、`Element.entry` 逐字相等 | 单例 |
| `set_gate()` | 边界 | `entry` 位于 `api` 之前 / 之后两种序、含前导 `./` 原文 → 均逐字保留（不重排、不归一） | 逐值迭代（2 行） |
| `set_gate()` | 边界 | 无 `entry` 的源 → 改档后仍无 `entry` 键（写面不臆造字段） | 单例 |
| `plan_set_gate()` | 异常 | 元素表缺 `element` 表 / FQN 不存在 → `Err` 且零文本产出（既有异常面逐值不变） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| TOML 写面与读回 | 不 mock：真实 `toml_edit` + 真实 `TomlParser`（保留语义与自证判据即被测对象） | 全部用例 |
| 盘面 | 不 mock：真实 `TempDir` | `plan_set_gate` 用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/tree.rs -> packages/desktop/src-tauri/crates/core/archi/src/tree_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`tree.rs` 零改动）；proposal 的「测试文件」表把 `tree_test.rs` 列入机械面，AC-5 的树侧标记由本段承托。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是「树侧零第二份剥离规则、标记随 `owner_of` 三候选」的观测面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 生成物条目（旧落点用例改写） | 正向 | `crates/infra/archi/src/lib.i.rs` → 条目 `mark = member`、`owners = ["App.Backend.Infra.ArchiRuntime"]`（目录前缀命中；旧 `crates/infra/archi.i.rs` 语料同步改新落点） | 单例 |
| 生成物条目（第三候选 / 文件型根） | 正向 | 文件型根 `packages/desktop/src/views/welcome-view.tsx` + 生成物 `packages/desktop/src/views/welcome-view.i.ts` → 条目 `mark = member`、`owners = ["App.Frontend.Views.Welcome"]` | 单例 |
| 生成物条目（白名单外） | 边界 | `src/mod/mod.i.py` / `.d.rs` / `.i` / `.I.rs` → 归属与 `mark` 与改动前逐值相等（剥离不生效，树侧零第二份规则） | 逐值迭代（4 行） |
| 非 `.i` 条目 | 边界 | 目录型 / 文件型元素根、祖先 `containsElements`、`exists = false` 语料逐值不变 | 三表迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面与配置 | 不 mock：真实 `TempDir` + 真实 `file_tree`（`owner_of` 是唯一归属判据，替身会让「单源」不可见） | 全部用例 |
| 元素表 | 不 mock：真实 TOML 文本经 `TomlParser` 聚合 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/mod_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`scan/mod.rs` 零改动）；`entry` 只落在 `ArchiElement` 的机械面（`scan/mod_test.rs:94` 的构造器 + `two_element_model`）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是扫描结果对新增字段的零敏感。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `element()` / `two_element_model()` 夹具 | 边界 | `ArchiElement` 构造器补 `entry: None`；既有 `matched` / `unmatchedFiles` / `excluded` / `unresolved` 断言逐值不变 | 单例 |
| 扫描面字段面 | 边界 | 元素带显式 `entry`（合法 / 非法各一）→ 扫描结果与 `entry = None` 逐字段相同（扫描只吃 `path` / 配置） | 逐值迭代（2 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面与配置 | 不 mock：真实 `TempDir` + 真实 `scan`（排除矩阵 / 归属即被测语义） | 两行用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/rust_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`scan/rust.rs` 零改动，不构造 `Element` / `ArchiElement`）；proposal 的「测试文件」表把 `scan/*_test.rs` 列入机械面，本段登记其零改动事实。 -->

#### 待测功能

<!-- 无公共函数 / API 行；本文件在 `entry` 面上零机械改动。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 模块树解析回归 | 边界 | 读盘实测本文件不构造 `Element` / `ArchiElement`（零 `entry` 字面量）；模块树四形态 / `#[path]` 优先 / 清单解析用例逐值不变 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面 crate 树 | 不 mock：真实 `TempDir` + 真实 `RustResolver` | 唯一用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/ts_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`scan/ts.rs` 零改动，不构造 `Element` / `ArchiElement`）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；本文件在 `entry` 面上零机械改动。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| TS import/export 解析回归 | 边界 | 读盘实测本文件不构造 `Element` / `ArchiElement`；静态 import 三形态 / 行号精度 / 动态导入 / 语法错用例逐值不变 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 源文与盘面 | 不 mock：真实 `oxc` 解析 + 真实 `TempDir` | 唯一用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/exclude.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/exclude_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`scan/exclude.rs` 零改动，不构造 `Element` / `ArchiElement`）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；本文件在 `entry` 面上零机械改动。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 排除矩阵回归 | 边界 | 读盘实测本文件不构造 `Element` / `ArchiElement`；默认七条约定、配置追加、最小下标优先用例逐值不变 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| glob 配置 | 不 mock：真实 `globset` + 真实配置对象 | 唯一用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/render.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/render_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`api/render.rs` 零逻辑改动）；但它的语料与期望值全量依赖 `paths::generated_path` 与入口解析，属 AC-3 的必改面（design 事实校准 11 的同一类机械锚点）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是 `workspace_render` 的落点与语料形态随入口化改写。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `workspace_render()` | 正向 | crate 目录根 + 文件型根双语言语料 → `.i.rs` 落 `crates/core/archi/src/lib.i.rs`、`.i.ts` 落 `views/welcome-view.i.ts`；`generated_path` 入参由 `<root>` 改 `<entry>` | 单例 |
| `workspace_render()` | 正向 | 输出序（契约 FQN 序 → `.i.*` → `INDEX.toml` 末位）与两次调用幂等，期望序列按新落点重算 | 单例 |
| `workspace_render()` | 边界 | 语料落盘形态改写：文件型根 `views/welcome-view.tsx/index.ts`（目录）→ `views/welcome-view.tsx` 文件本身；改写后仍 5 份（两份 schema + `.i.rs` + `.i.ts` + 索引），若沿用旧目录形态则掉到 4 份——该差异钉住「语料必须改写」 | 单例 |
| `workspace_render()` | 异常 | 结构诊断 fail-closed（`ApiError::Element`、零部分产出）、坏模型零写盘、空模型 / 零契约冷启动、`root` 形态边界、零 spawn 自证逐值不变 | 五表迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / 契约 / 盘面 | 不 mock：真实 `TempDir` + 真实 `elements_of` → `load_contracts` → `render_all` 组合（渲染序即被测语义） | 全部用例 |
| 文件系统时间戳 | 不 mock：既有 `SystemTime` 读取断言保持原状 | 零写盘查询 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`api/contract.rs` 零改动，不构造 `ArchiElement`）；proposal 的「测试文件」表把 `api/{contract,…}/**/_test.rs` 列入机械面，本段登记其零改动事实。 -->

#### 待测功能

<!-- 无公共函数 / API 行；契约格式面与 `entry` 零交集。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 契约解析 / 校验回归 | 边界 | 读盘实测本文件不构造 `Element` / `ArchiElement`；`api-op` / `api-types` / `api-reexport` 三类 fence、签名断言与校验失败面逐值不变 | 三表迭代 |
| 契约格式留白 | 异常 | 契约文本内 `entry` 词形零语义（不因模型新增字段扩格式）；未知 fence / 未知键仍按既有判据拒收 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 契约源文 | 不 mock：真实 `contract::parse`（判据即被测对象） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`api/emit/rust.rs` 零逻辑改动）；proposal 的「测试文件」表把 `api/emit/**` 列入机械面。 -->

#### 待测功能

<!-- 无公共函数 / API 行；渲染器零形态推导的回归面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `rust::render()` | 边界 | `EmitTarget` 三字段与两类渲染断言逐值不变；渲染器仍零 `paths::` 调用（形态只由调用方经 `Resolved.form` 给） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 契约与符号集 | 不 mock：真实 `ApiDocument` / `ApiSymbol` 夹具 | 唯一用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`api/extract/mod.rs` 零改动：`extract_element` 仍收入口串）；`ArchiElement` 只落在 `extract/mod_test.rs:32` 的构造器。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是取数入参仍为「入口串」的机械事实。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `element()` 夹具 | 边界 | 构造器补 `entry: None`（唯一构造点）；既有分派 / `normalize_signature` / `schema_type` 用例逐值不变 | 单例 |
| `extract_element()` | 边界 | 入参仍是入口串（crate 形态 `x/src/lib.rs`、单文件 `views/index.ts`、`.tsx`）→ 签名不变、取数与 `bootstrap` 同源断言保持绿 | 三形态表 |
| `extract_element()` | 异常 | 入口读不到（不存在 / 目录 / `""`）→ 空集且不 panic（缺口由 `apiEntryMissing` 报，不由取数层猜） | 四值表 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面模块树 | 不 mock：真实 `TempDir` + 真实 `extract::rust` / `extract::ts` | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`api/extract/rust.rs` 零改动）；proposal 的「测试文件」表把 `api/extract/**` 列入机械面。 -->

#### 待测功能

<!-- 无公共函数 / API 行；取数入参不变（入口串），`entry` 零触及。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract_crate()` | 边界 | 入参仍是入口串（读盘实测本文件不构造 `Element` / `ArchiElement`）；`pub use` 出线、可见性、模块树递归用例逐值不变 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 盘面 crate 源文 | 不 mock：真实 `TempDir` + 真实解析 | 唯一用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`api/extract/ts.rs` 零改动）；proposal 的「测试文件」表把 `api/extract/**` 列入机械面。 -->

#### 待测功能

<!-- 无公共函数 / API 行；取数入参不变，`entry` 零触及。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract_file()` / `apply_knip_report()` | 边界 | 读盘实测本文件不构造 `Element` / `ArchiElement`；三分类判定、`visibility` 留白、knip 降级用例逐值不变 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| TS 源文与 knip 报告 | 不 mock：真实 `oxc` 解析；knip 报告以 `json!` 直传入参（零 spawn） | 唯一用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`drift/mod.rs` 零改动）；但 `api_findings` 槽位与 `status` 推导对 ① 语义变化敏感，属 AC-4 / AC-8 的必测回归面。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是入口解析链换血后判定结果的零漂移。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check()` | 正向 | 试点 fixture（目录根 `crates/infra/archi/` + `src/lib.rs` 入口 + 契约在案）在新链下仍 `Resolved` → `api_findings` 空、`status = clean` | 单例 |
| `check()` | 正向 | `api` 目录缺席 → `api_findings` 空槽位（`enabled(root)` 早退不变），`version = 3` 与既有槽位逐值不变 | 单例 |
| `check()` | 边界 | 契约签名不一致 → `apiSignatureMismatch`、`status = warnings`、`passed` 不受影响（纳管子集只看契约在案元素） | 单例 |
| `check()` | 边界 | `Absent` 元素（目录根无入口）→ `api_findings` 出现 `apiEntryMissing`，但 `status` 与 `passed` 与无契约面基线逐字相同（不进纳管子集） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / 盘面 / 契约 | 不 mock：真实 `TempDir` + 真实 `check` 全链路（`run_full` 既有装置） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（`drift/report.rs` 零改动）；读盘实测本文件不构造 `Element` / `ArchiElement`（只构造 `ArchiMatchedElement` / `ArchiDrift*`），故 `entry` 面零机械改动。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是报告线面对新增字段的零敏感。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 报告线面 | 边界 | 完整报告 JSON 往返、camelCase 字段面、`Option` 显式 `null` 逐值不变；`ArchiElement` 不在报告线面内 ⇒ 报告字段集零变化 | 单例 |
| `apiFindings` 槽位 | 边界 | 槽位形状（`kind` / `category` / `element` / `occurrences`）零变化；`entry` 相关理由只体现在 `message` 文本，不扩字段 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde 线形 | 不 mock：真实 `serde_json` 序列化 / 反序列化（线形即被测对象） | 两行用例 |

---

### packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs -> packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行（词表四面零改动）；proposal 的「测试文件」表把 `tests/vocabulary_test.rs` 列入机械面（`ArchiElement` 字面量一处）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是「`entry` 的值域是开放集，不进派生词表」。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiElement` 字面量 | 边界 | 唯一构造点补 `entry: None`；四面词表 `ALL` 长度与逐值线词断言不变（`ElementKind` / `GateTier` / `ApiStatus` / `DiagnosticKind`） | 四表迭代 |
| 词表封闭性 | 异常 | `DiagnosticKind` 仍九值、`ElementKind` / `GateTier` / `ApiStatus` 三面 `ALL` 长度与线词不变：`entry` 文本不出现在任何 `ALL` 常量的线词集合里（`ArchiApiFindingKind` 的十值封闭断言在 `check_test`，不属本文件） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 词表常量 | 不 mock：真实 `*::ALL` 与 `as_str()` 对拍 | 全部用例 |
| 源码自证 | 不 mock：`include_str!` 读产品源码 | 封闭性用例 |

---

### packages/desktop/src-tauri/crates/core/archi/tests/perf.rs -> packages/desktop/src-tauri/crates/core/archi/tests/perf.rs

<!-- design.md 的「公共函数 / API」子表未列本文件的行；读盘实测本文件经 TOML 文本构造 300 个目录型根元素（零 `Element` 字面量），因此 `entry` 面零机械改动——但入口链换血会为该夹具新增大量 `Absent`，属性为 AC-3 / AC-4 的回归面。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是「新增 `apiEntryMissing` 不改 `status` / 预算口径」。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run_full()` 夹具 | 边界 | 300 个目录型根元素（无 `src/lib.rs`）在新链下全 `Absent` → 判定仍走完：`model.elements = 300`、`scannedFiles = 1000`、`matched = 300`、`status = errors`、`errors` / `warnings` 计数逐值不变（`apiFindings` 不进纳管子集） | 单例 |
| 预算口径 | 边界 | 装置自检（`percentile` 最近秩）与「release 10 次 / debug 1 次」样本口径逐值不变；候选派生零 IO 使新增开销只落在 `entry_state` 的 `is_file` 探测面 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 计时与夹具 | 不 mock：真实 `TempDir` 语料 + 真实 `check` + `Instant` 计时（预算装置即被测对象） | 两行用例 |

---

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src/views/architecture/**/*.test.tsx

<!-- design.md 的「公共函数 / API」子表未列这些文件的行（前端组件零改动）；`ArchiElement` 新增必填键后，四个测试文件的 `element()` 构造器必须机械补齐（proposal「测试文件」表 / R-8）。 -->

#### 待测功能

<!-- 无公共函数 / API 行；待测对象是前端 fixture 的机械补齐与清单完整性。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 前端 fixture | 边界 | 读盘实测四处 `element()` 构造器各补 `entry: null`：`views/architecture/architecture-view.test.tsx`、`components/architecture-canvas.test.tsx`、`components/element-drawer.test.tsx`、`components/element-tree.test.tsx` | 四文件批 |
| 类型面 | 正向 | `vp check` 的 typeCheck 绿：`ArchiElement` 必填 `entry` 补齐后零 `@ts-expect-error`；既有 `it` 断言零改动（`overrides: Partial<ArchiElement>` 形态仍可覆盖 `entry`） | 单例 |
| 清单完整性 | 异常 | 漏补任一文件 → 该文件类型检查报缺键（机械证明清单无遗漏）；其余 14 个 `views/architecture/**` 测试文件不构造 `ArchiElement`，零触及 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri IPC | `vi.mock('@tauri-apps/api/core')`（既有 `invokeMock`，按命令名分发；`entry` 断言不依赖它） | 组件渲染用例 |
| 类型与绑定 | 不 mock：真实 `ArchiElement` 类型（类型主权在生成文件） | 类型面用例 |

---

## 不可测试项

| 对象 | 原因 | 替代承托 |
|---|---|---|
| `AGENT.md` / `CLAUDE.md`（`## 模块 API 契约` 第②条措辞） | 仓库根文档；proposal 与 design 明禁为两份文档引入解析测试（MUST NOT 把文档正文迁进代码常量），该节由「只指路不带正文」纪律约束 | AC-9 由桌面端 static-check 的机械断言覆盖：节内 `grep -c '^- \*\*'` = 5、`grep '元素根同级'` 与 `grep '<root>.i'` 零命中、「实现入口同级」与 `<入口主干>.i.rs` / `<入口主干>.i.ts` 在场、两副本节内 `diff` 为空且全文 `diff` 恰 2 处（全在节外） |
| `.dev-team/architecture/api/INDEX.toml` | 机械再生成物（禁手改），零手写逻辑；单测无从表达「生成器输出与仓内资产同源」之外的事实 | AC-3：桌面端 `pnpm api:check` 的 `orphaned` / `missing` / `mismatched` 三列皆空 + review `git diff` 只一行变化 |
| `.dev-team/architecture/models/05-backend-tools.toml` | 纯数据（两工具 crate 各一行 `api = "none"`；`04` 零改动、全库零 `entry`）；`archi` 单测除试点资产外不读仓内真实 models，无单测承载体 | AC-4 / AC-6：全量判定的分桶实测（41 = 15 + 26，新代码；重建前宿主只能复现旧基线）+ `archi_graph(root)` 的 51 元素 / 113 关系 / 零诊断 / 零告警 / `entry` 全 `null` 实测 |
| `packages/desktop/package.json`（`version`） | 版本纪律，无测试落点（proposal 明示不列为 AC） | review `git diff`：`version` 相对 HEAD +1 patch，`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` crate 版本行不随动 |
| `packages/desktop/src-tauri/crates/infra/archi.i.rs` | 删除文件（不纳入被测范围） | 「旧落点不在盘」由 `infra/archi/src/lib.rs -> paths_test.rs`（跨 crate 字面量与旧路径断言）与 `infra/archi/src/lib.i.rs -> pilot_test.rs`（双向落点断言）钉住 |
| `crates/core/archi/src/lib.rs` 的 `pub use api::{…}` 出口行 | 零改动（`paths` 已是 `pub mod`，`EntryState` / `EntryProblem` 经 `archi::api::paths` 可达），无新增导出面可测 | 既有出口面断言保持绿（`api::mod_test` 的出口归零面） |
