# desktop-archi-api Specification（delta: archi-file-tree）

## MODIFIED Requirements

### Requirement: 生成物落点与入口查找

生成物 SHALL 落元素根路径**同级**（`<root>.i.rs` / `<root>.i.ts`），其中 `<root>` 为元素根路径**无后缀形态**（无 `./` 前缀、无尾 `/`、无文件扩展名），`<name>` 为 `<root>` 的**最后一段**（`crates/core/archi/src/scan` → `scan`、`crates/infra/archi` → `archi`、`views/welcome-view` → `welcome-view`）。

- **`<name>` 取路径末段而非 FQN 末段**：两者多数情况相同但不恒等（`App.Backend.Infra.ArchiRuntime` 的 FQN 末段是 `ArchiRuntime`、路径末段是 `archi`），而文件名必须与 `#[path]` / TS import 的实际路径对齐——故判据只有路径末段一条，MUST NOT 引入 FQN 派生的第二套命名。
- **`<root>` 无后缀是硬约束**：`<root>` 是**模块身份**而非文件路径，同时决定生成物落点、实现入口位置与 `#[path]` 写法。归一 SHALL 在模型解析层完成（`OwnerIndex::normalize` 已处理 `./` 前缀与尾 `/`），MUST NOT 批量改写存量 models——实测存量 41 条 `path` 全部带 `./` 前缀 + 尾 `/`、一律需要归一（本变更新增的第 42 条同形）。
- **不强制模块成目录**：两条路对比后取「按文件（根同级）」——生成物路径只有一条规则、无形态分叉，前端 12 个单文件元素零改动；代价（留痕）是目录形态的引用路径带 `../`，生成物与模块文件在 git 视角不聚拢。
- **入口查找顺序封闭**（MUST NOT 增分支）：
  ```
  Rust    <dir>/src/lib.rs   →  <dir>/mod.rs   →  <dir>.rs
  前端    <dir>/index.ts     →  <dir>/index.tsx →  <dir>.ts →  <dir>.tsx
  ```
  `<dir>/lib.rs` MUST NOT 在列表内（crate 的 `lib.rs` 恒在 `src/` 下）。三分支 / 四分支各有真实存量支撑或如实零命中，MUST NOT 因「未来可能需要」加分支。
- **入口编排 SHALL 固定为**：入口文件顶部 `#[path = "<相对路径>"] mod api_gen; pub use api_gen::*;`，既有 `pub mod` 声明原样不动。生成类型经 `pub use` 对外可引用；与既有同名类型冲突时编译器直接报 E0255，**不会静默覆盖**。
- `#[path]` 的值 SHALL 为**字面量**，且 MUST 与 `paths::mod_path_attr` 的输出逐字相等（该函数是等值判据的唯一真值源，由 `paths_test` 三形态断言与逐入口自查行双守）；MUST NOT 手写与函数输出不等值的形态。目标是恒指向 `<root>.i.rs`——即生成物在元素根路径同级、而非入口文件同级：

  | 入口形态 | `#[path]` 基准 | `#[path]` 值 |
  |---|---|---|
  | `<dir>/src/lib.rs`（crate 形态） | `<dir>/src/` | `"../../<name>.i.rs"` |
  | `<dir>/mod.rs`（子目录模块形态） | `<dir>/` | `"../<name>.i.rs"` |
  | `<dir>.rs`（单文件形态） | `<dir>` 的父目录 | `"<name>.i.rs"` |

  同一单点函数 SHALL 产出 TS 侧的 `import type` 路径（目录形态 `"../<name>.i"`、单文件形态 `"./<name>.i"`）。三形态的解析结果 SHALL 逐字等于 `<root>.i.rs` / `<root>.i.ts`；只凭「入口文件同级」写字面量会落到 `<dir>/<name>.i.rs`（错位），故 MUST NOT 手写。
- **MUST NOT 生成 `_ext.rs`**：生成物只含类型定义（`struct` / `trait` / `enum` / newtype）与自动构造器；`impl` 块写在模块内任意手写文件（Rust 允许同 crate 内 `impl` 分散）。生成 / 手写分离靠**文件命名**（`.i.rs` / `.i.ts` 后缀）而非目录分离。
- **归属规则 SHALL 为「双形态并集」**：`.i.<ext>` 生成物（`ext ∈ {rs, ts, tsx, js, jsx}`，标记段恰为 `i` 且模块名非空）的归属 SHALL 由**两个剥离去向取并集**判定——

  ```
  候选 1  整组剥离      <name>.i.<ext>  →  <name>        archi.i.rs → archi
  候选 2  替换扩展名    <stem>.i.<ext>  →  <stem>.<ext>  archi.i.rs → archi.rs
  命中判据 = 任一候选满足 rel == prefix 或 rel.startsWith(prefix + "/")
  ```

  理由（两种落点形态互不相交，单条规则覆盖不了）：生成器 `paths::generated_path` 产出的是**元素根同级**（`crates/infra/archi.i.rs`，需候选 1）；历史 / 假设形态是**元素目录内**（`<dir>/<name>.i.<ext>`，需候选 2）。判据改造 SHALL 只此一处（`OwnerIndex` 的剥离函数与其调用点），`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不动，既有非 `.i` 路径的归属语义零变化。
- **并集 MUST NOT 实现为候选短路**：命中集 SHALL 为「对每个候选各跑一遍最长前缀后合并去重」，再按前缀长度定档（同长仍归 `Ambiguous`）；MUST NOT 写成「候选 1 命中即返回」——那会让 `src/dir.i.ts`（候选 1 全等 `src/dir`、候选 2 得 `src/dir.ts`）只走一条路径，与并集语义不符。定档用例：元素根 `src/dir/` 与 `src/dir.ts` 同时在场、探针 `src/dir.i.ts` ⇒ 归更具体的单文件根（前缀更长者胜出）。
- **白名单外后缀零误剥**：`.i.py` / `.d.rs` / 大写 `.I.rs` / 无扩展名 `.i` SHALL 原样返回单个候选（不剥），归属与改动前逐值相同。
- **父子路径重叠不缩窄父契约**：子元素根路径是父元素根路径的前缀，最长前缀匹配已正确处理（`.../src/scan/x.rs` 归 `Scan`、`.../src/dto.rs` 归 `Archi`）。
- **新判据 `apiEntryMissing`**：元素根路径下既无生成物（`.i`）也无实现入口 → 该元素不是真模块或已删除。它比 `apiMissing` 更基础——**在契约生成之前就失效**，防止给不存在的模块写契约（定义与档位见 `desktop-archi-api-check`）。
- **`#[path]` 的读盘面 SHALL 与写盘面同一条限根纪律**：取数器定位 `#[path = "…"] mod` 时 MUST 满足三条——① **反斜杠归一在折叠之前**（否则反斜杠形态被当作单段名，`Path::join` 的纯拼接会把 `..` 原样带到 Windows 的路径解析里、逐级上溯到观察根之外）；② 分段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即拒）；③ **`join` 后复核**目标仍在观察根之下（`starts_with(root)`）。任一条不成立即**不命中该子树**（与「模块定位失败即跳过」同口径，MUST NOT 新增错误变体、MUST NOT panic）。理由是 `#[path]` 的值经**手写入口行**落地（属性的值只能是字面量，等值性由机械断言守），属可被写错的入参；放行即让「模块树被带出观察根」，与 `paths` 侧的「任意文件读」是同一件事。判据与 `scan` 的 `normalize_rel` / `docs` 的 `resolved_within` 同一条纪律。

#### Scenario: 两种落点形态各归其元素

- **WHEN** 分两组求归属：① 根同级形态（`crates/infra/archi.i.rs` 对目录形前缀 `crates/infra/archi`、`views/welcome-view.tsx.i.ts` 对单文件形前缀 `views/welcome-view.tsx`）；② 目录内形态（`views/architecture/architecture.i.ts`、`crates/core/archi/src/scan/scan.i.rs` 对各自目录形前缀）
- **THEN** 四组用例各归其元素（①由整组剥离命中、②由替换扩展名命中，两者并集）；非 `.i` 路径（如 `crates/core/archi/src/dto.rs`）归属结果与改造前逐字相同

#### Scenario: 并集定档不短路

- **WHEN** 元素根 `src/dir/` 与 `src/dir.ts` 同时在场，探针为 `src/dir.i.ts`
- **THEN** 归属为更具体的单文件根（`src/dir.ts` 所在元素，前缀更长）；若实现成「候选 1 命中即短路」则归目录形根——该差异即本 Scenario 的判红面

#### Scenario: 白名单外后缀不误剥

- **WHEN** 对 `src/mod/mod.i.py`、`src/mod/mod.d.rs`、`src/mod/mod.I.rs`、`src/mod/mod.i` 求归属（元素根 `src/mod/`）
- **THEN** 四者按原路径判定归属（不剥），结果与改动前逐值相同；`.i.<白名单外后缀>` 不产生第二个候选

#### Scenario: 入口三分支与四分支实测命中

- **WHEN** 对全部非容器元素按入口顺序查找实现入口
- **THEN** `<dir>/src/lib.rs` 命中 **14** 个 crate 元素（`core` 7 + `infra` 7，实测复核）、`<dir>/mod.rs` 与 `<dir>.rs` 当前零命中、前端四分支当前零命中；`crates/tools/export-bindings` 与 `crates/tools/archi-api-gen`（均只有 `main.rs`）以及壳元素 `App.Backend.Shell`（根路径 `src-tauri/src` 拼出的三条候选均不存在）记为 `apiEntryMissing`（**不消解**）；`crates/tools/specta-zod-patched` 无元素，不参与入口查找

#### Scenario: 三种入口形态的 #[path] 各自解析到根同级生成物

- **WHEN** 分别对 crate 形态（`crates/infra/archi/src/lib.rs`）、子目录模块形态（`crates/core/archi/src/scan/mod.rs`）、单文件形态（`views/architecture.ts`）取 `paths::mod_path_attr`（`#[path]` 等值判据的真值源；入口行里的值本身是字面量）与 `paths::ts_import_specifier`（import 路径单点产出）的输出并规范化
- **THEN** 三者分别解析为 `crates/infra/archi.i.rs`、`crates/core/archi/src/scan.i.rs`、`views/architecture.i.rs`（等于各自的 `<root>.i.rs`）；把任一形态的值改写成「入口同级字面量」后解析结果错位（`<dir>/<name>.i.rs`）且编译 / 导入失败

#### Scenario: 生成物与手写 impl 共存编译

- **WHEN** 入口文件写与 `paths::mod_path_attr(EntryForm::Crate, "archi")` 输出逐字相等的 `#[path]` **字面量**（crate 形态 `"../../archi.i.rs"`）+ `pub use api_gen::LocalArchiCheck;`，生成物 `<root>.i.rs` 含 `pub struct LocalArchiCheck;` + `impl` 内 `new()`，手写文件另有 `impl Default for LocalArchiCheck`
- **THEN** 编译通过（`rustc --edition 2021`），外部可经入口路径引用 `LocalArchiCheck::new()` 且签名受生成物约束

#### Scenario: 同名冲突不静默覆盖

- **WHEN** 生成物定义了元素内已手写的同名类型且入口 `pub use api_gen::*;`
- **THEN** 编译器报 E0255（重复定义），MUST NOT 静默采用其一

#### Scenario: 路径形态非法即拒收（安全边界）

- **WHEN** 元素 `<root>` 或契约 FQN 取绝对路径 / 含盘符 / 含 `..` 段 / 含空段 / 为空串（如 `path = ["/abs/elsewhere"]`、`path = ["crates/../../outside"]`、`fqn = "App..X"`）
- **THEN** `contract_path` / `schema_path` / `generated_path` / `entry_form` 一律返回空（`None`），不 panic、不落到仓外；`root.join(绝对路径)` 的「替换 base」与 `..` 的「上溯」因此结构上不可达——「证据在盘」的探测不会变成任意文件读，工具 crate 的写盘不会变成任意文件写。**第二层**：`archi-api-gen` 的写盘函数 SHALL 另以目标须落在观察根之下（`starts_with(root)`）复核，越界即拒绝并零副作用（两层判据交叉验证，任一层失效另一层仍拦得住）

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/scan/owner.rs` | 生成物归属（判据修正） | 剥离由「替换扩展名」改为**候选集**：整组剥离 `<name>.i.<ext> → <name>` ∪ 替换扩展名 `<stem>.i.<ext> → <stem>.<ext>`；`owner_of` 对每个候选各跑一遍最长前缀、按 `(fqn, 前缀长度)` 合并去重后定档（同长仍 `Ambiguous`），**MUST NOT 候选短路**；`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不变；非 `.i.<ext>` 路径归属零变化 |
| `crates/core/archi/src/scan/owner_test.rs` | 判据断言面（修正） | `生成物剥离四种形态各归其元素` 保留既有四条目录内形态并**另加**四条根同级实形态正例（含 `crates/infra/archi.i.rs`）；`src/dir.i.ts`（根 `src/dir/`）期望由 `Owner::None` 翻转为 `Owner::Element`（把 bug 写成契约的那条断言即修复本身），`生成物剥离使单文件元素根同级生成物归元素` 的用例名 / doc / 断言意图同步改写；新增并集定档用例（`src/dir/` 与 `src/dir.ts` 同场 ⇒ 单文件根胜出）；白名单外后缀 4 条反例保持绿 |
| `crates/core/archi/src/scan/mod.rs` | 扫描编排（零判据改动） | `walk_source_files` / `is_source_file` 提为 `pub(crate)` 供文件树复用；`scan()` 的枚举 / 排除 / 归属 / 抽取 / 聚合六段语义逐字不变（`owner_of` 的调用点两处——源文件归属与目标归属——自动获得并集判据） |
| `crates/core/archi/src/tree.rs`（新增，见 `desktop-archi-file-tree`） | 文件树投影 | 归属与剥离一律经 `OwnerIndex`；本能力只要求「生成物按并集判据归属，树侧零第二份实现」 |
