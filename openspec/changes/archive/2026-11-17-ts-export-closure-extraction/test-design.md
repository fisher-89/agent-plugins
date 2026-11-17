# 测试设计: ts-export-closure-extraction

> **变更**: ts-export-closure-extraction（TS 取数面沿 `export … from` 走 · 导出闭包 · `ts.rs` 三态单点 · `ApiSymbol.file`）
> **日期**: 2026-11-17
> **输入**: `proposal.md`（AC-1…AC-10 / R-1…R-10 / 「变更范围 - 测试文件」表 5 行 / 「不要修改」表）；`design.md`（D1–D10、「起点实况核实」、「公共函数 / API」表 7 行、「类型定义」表 5 行、「修改文件」表 5 行、「新增文件」表 1 行、「私有签名变化」段）；`tasks.md`（阶段零～五 + 移交清单 T1–T6）；已在案两份 delta（`specs/desktop-archi-api/spec.md` ADDED「TS 导出闭包的取数面」+ MODIFIED「模块嵌套与选择性透传」；`specs/desktop-archi-api-check/spec.md` MODIFIED「十条判据的定义与求值互斥顺序」）
> **性质**: 只设计测试范围与用例，不含评估结论、不含测试运行脚本细节。静态门禁（`api:check` / `server:check` / `client:check`）与全量 `apiFindings` 读数由桌面端 static-check / test-execution 执行，本稿不复制命令细节。

**测试框架与既有布局（读盘识别结果，本变更零新增依赖 / 零新增测试文件）**

- **Rust sidecar**（`packages/desktop/src-tauri/crates/core/archi/src/api/**`）：内置 `#[test]`，测试文件与源文件同目录同前缀 —— `<name>.rs` ↔ `<name>_test.rs`，由源文件末段 `#[cfg(test)] mod <name>_test;` 挂载；断言为 `std` 的 `assert!` / `assert_eq!` / `matches!` / `expect_err`，零第三方断言库；**零 dev-dependency**，需要盘面事实时以 `std::env::temp_dir()` + 进程号 + 计数器自建 `TempDir` RAII（`extract/mod_test.rs` 与 `api/mod_test.rs` 各一份，沿用到本次全部新用例）；源码自证断言沿用 `include_str!("<同名>.rs")` + `matches(...).count()` / `lines().find(...)` 形态。
- **前端侧**（`packages/desktop/src`）：本变更**零前端源码改动**（`ApiSymbol` 不进 `dto` / 不进 specta / 不进 bindings），故 `vp test` 面零新增用例；`src/types/generated/**` 零 diff 由 static-check 承接（见 `## 不可测试项` 5）。
- **mock 纪律**（沿既有 test-design 口径）：核心 crate 的判据事实源（盘面 / AST / 归属 / 符号集 / 模型 / 契约）一律**真实组合** —— `scan::ts::TsResolver`、`scan::owner::OwnerIndex`、`ts::extract_surface`、`rust::extract_crate` 全真实调用（替身会让「零第二份解析 / 零第二份归属」这条被测不变量不可见）；`api/**` 无网络 / 无 DB / 无子进程 / 无环境变量读取，故本变更唯一「进程边界」是**文件系统**，用真实 `TempDir` 驱动而非 mock。

### 被测模块清点（design「变更清单」逐项）

| # | 路径（相对项目根） | 来源 | 解析到的测试文件 | 判定 |
|---|---|---|---|---|
| 1 | `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs` | 修改文件 | `…/api/extract/mod_test.rs` | 行为面 + 机械面，见 `## 单元测试` 第 1 节 |
| 2 | `packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts.rs` | 修改文件 | `…/api/extract/ts_test.rs` | 行为面 + 三闸机械面，见第 2 节 |
| 3 | `packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs` | 修改文件 | `…/api/extract/rust_test.rs` | 行为面（只填 `file`），见第 3 节 |
| 4 | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | 修改文件 | `…/api/mod_test.rs` | 行为面 + 组合链路，见第 4 节 |
| 5 | `packages/desktop/package.json` | 修改文件 | — | 版本号单值 ⇒ `## 不可测试项` 1 |
| 6 | `openspec/changes/ts-export-closure-extraction/reports/api/closure-evidence.md` | 新增文件 | — | AC 实测留痕载体（非源文件）⇒ `## 不可测试项` 2 |

无删除文件 ⇒ 无删除面纳入被测范围。design「修改文件」表的 **5 行** 全部在册（4 源码 + 1 配置），新增文件 **1 行**（留痕 md）；`packages/desktop/src-tauri/crates/core/archi/src/api/check_test.rs` 不在 design 修改表内 —— 它的改动只是 `ApiSymbol` 增字段后的**夹具形状适配**（T1），判据断言值零变化，登记于 `## 不可测试项` 3。

**机械面（跨章节共用，不单列用例章节）**：`ApiSymbol` 增 `pub file: String` 后，全部 `ApiSymbol` 字面量必须同批补值 —— 测试侧 **3 处**：`extract/mod_test.rs` 的 `fn symbol(...)`、`api/check_test.rs` 的 `fn function(...)` 与 `fn type_symbol(...)`；产品侧 **4 处**：`ts::{push_function,push_type}`、`rust::{push_type,function_symbol}`。`extract_element` 签名扩为 4 参后，`extract/mod_test.rs` 的 7 处既有调用点（T2 逐条点名：整元素导出集 / TS 单文件两处 / 入口读不到 / 语言未知 / 与 bootstrap 同源 / `extract` 同源）与 `api/mod_test.rs` 的 `extract::extract_element(` 调用点同批补 `elements` / `element`（T1 / T2），既有断言值不变。

### 参数类型 → 边界覆盖口径

| 类型 | 本变更入参 / 载荷 | 边界覆盖（最少条数） | 落地用例 |
|---|---|---|---|
| str / string | `extract_element.entry`、`extract_surface.file`、`extract_surface.source`、`NamedExport.{local,exported,specifier}`、`ApiSymbol.file` | `""`、超长（>1000 chars）、特殊字符（emoji / `\n` / 引号）、None（不适用：`&str` 无空档） | 第 1 节「entry 边界」「element 边界」；第 2 节「specifier 边界」「具名名单形态边界」；第 3 节「file 形态规范」 |
| list / array | `extract_element.elements`、`ExportSurface.named` / `star`、`ApiSymbol.params` | `[]`、单元素、超大（500 / 1000 条） | 第 1 节「elements 空表」「单元素 / 超大元素表」「规模」；第 2 节「规模」 |
| dict / object | `ArchiElement`（`path` / `fqn` / `api` 字段面）、knip 报告 JSON | `{}`（无 `files`）、缺失必填字段、多余字段、None | 第 2 节「`apply_knip_report` 零变化」；第 1 节「element 边界」 |
| Optional[T] | `Owner::Element` 的命中 / 未命中（`owner_of` 的三值不是 `Option`，但「命中与否」等价一档）、`iter().find()` 取符号 | None（既有用例已覆盖「缺失即断言失败」的 `find` 夹具） | 第 1 节 owner 四值；第 3 / 4 节 `find` 夹具沿用 |
| Enum | `ExportedItem` 四变体、`Language` 三值、`Owner` 三值、`Visibility` 四值、`ApiSymbolKind` 四值 | 每值 + 非法 / 留白值 | 第 1 节 owner 四值 + 语言未知；第 2 节「私有枚举封闭」+ visibility 留白；第 3 节 visibility 四值 |
| int / number | `ApiSymbol.line`（`u32`）、`NamedExport` / `star` 计数 | `1`（首行）、大文件行号、计数 0 / 500 / 1000 | 第 2 节「规模」；第 3 节「规模」；既有 `extract_file行号与规模` 保留 |
| bool | `ApiSymbol.is_async`、`parsed.panicked` | `true` / `false`（+ None 不适用：非 `Option`） | 第 2 节「异常源」（`panicked = true`）；第 1 节「别名改名」断言 `is_async` 不被改名影响 |
| float | 无 | 不适用 | 本变更零浮点入参 / 载荷；既有 `ApiType::Float` 映射用例零改动 |

---

## 验收范围

<!-- 列口径：`被测文件或模块` = 承载该 AC 的源文件（含其测试 sidecar）+ 组合链路 + 不可测试项序号。
     路径前缀 `packages/desktop/src-tauri/crates/core/archi/src/` 在下表与各章节标题中写全，条目内缩为 `api/…`。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | **本仓语料恰 7 名**：`bootstrap App.Frontend.Components.Agent` 产出非空草稿，名字集逐字 = `AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`（3 op + 4 类型）；`AgentInputProps` / `AgentMessagesProps` / `isToolPart` 零命中；改前同一命令零符号。**crate 断言面**：以合成等价语料（父元素 + 4 子元素 + barrel 6 条语句，含「同一目标两条具名语句」形态）断言闭包名字集恰 7、未导出的 `*Props` 零命中；**本仓 7 名不写成 crate 常量**（语料纪律，见 `## 不可测试项` 2） | `api/extract/mod.rs`（`api/extract/mod_test.rs`：精确名单 / 同目标两条语句 / owner 子树）；`api/mod.rs`（`api/mod_test.rs`：bootstrap 非空骨架与注记）；`api/extract/ts.rs`（`api/extract/ts_test.rs`：具名面如实）；留痕见 `## 不可测试项` 2 |
| AC-2 | **精确名单与通配等价**：同一簇（`./x` 导出 `A`/`B`/`C`）的「三条逐名语句」与「一条 `export * from './x'`」两个入口 ⇒ `extract_element` 产出逐字段相等（名字集与顺序同）；反向：入口只 `export { A }` ⇒ 恰 1 符号，`B`/`C` 零命中 | `api/extract/mod.rs`（`mod_test.rs`：精确名单 / 通配等价 / 通配多跳）；`api/extract/ts.rs`（`ts_test.rs`：`named` / `star` 两面如实） |
| AC-3 | **环与菱形确定收敛**：`a → b → a` 零 panic、零重复、零无限递归；菱形按 `(name, kind)` 恰一条；同一输入两次调用逐字相等（稳定序） | `api/extract/mod.rs`（`mod_test.rs`：环 / 菱形 / 去重键含 `kind` / 稳定序）；`api/extract/ts.rs`（`ts_test.rs`：三面顺序 = 语句序） |
| AC-4 | **同元素约束按 owner 子树裁**：四条 re-export（子元素命中 / 另一元素 / `Owner::None`（含 `@/lib/**` 经 tsconfig `paths` 解析）/ `Owner::Ambiguous`）只有第一条入集，其余零符号零 panic；`…AgentFoo` 形态不因裸前缀误认；仓外 / 不在盘 / 非 TS 目标解析失败静默跳过、零 finding。**crate 断言面以合成夹具表达**（本仓 6 条 re-export 全落子树属留痕面） | `api/extract/mod.rs`（`mod_test.rs`：owner 四值 / 裸前缀不误认 / 解析失败静默跳过）；`api/extract/ts.rs`（`ts_test.rs`：留白形态三面皆空） |
| AC-5 | **注记行指向真实定义文件**：`bootstrap` 的注记为 `@ <符号 file>:<line>`，MUST NOT 出现 `@ <入口>:<line>` 冒充；类型注记 `待剪枝：{kind} @ {file}:{line} {visibility}`；Rust 侧子模块 `pub fn` 的注记指向子模块文件 | `api/mod.rs`（`mod_test.rs`：TS 一跳 / Rust 子模块 / 类型注记 / 注记面 `entry` 退出）；`api/extract/ts.rs` + `api/extract/rust.rs`（各自 sidecar：`file` 落值正确，是注记的真值源）；本仓逐符号 `file:line` 见 `## 不可测试项` 2 |
| AC-6 | **既有 Rust 取数面零变化**：`rust_test` / `mod_test` 的 `extract_crate` 语料逐字不动；`extract_element` 的 Rust 分支与 `extract_crate` 逐字段相等（`file` 一并相等）；`extract_element入口读不到返回空集` / `语言未知返回空集` 语义不变 | `api/extract/rust.rs`（`rust_test.rs`：`file` 来源 / 同源 / 既有抽取面与 `visibility` 四值语料零改动）；`api/extract/mod.rs`（`mod_test.rs`：Rust 分支逐字不变 / 两条 else 语义） |
| AC-7 | **三闸全绿且单点分派不变量在案**：闸 1「`pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {` 首行逐字」与闸 3「透传零符号」**零改动**；闸 2 改为 `match statement` 恰 1 + `Statement::` 出现次数 = 臂数 3 + `Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中；MUST NOT 删断言换绿 | `api/extract/ts.rs`（`ts_test.rs`：闸 1 / 闸 2 / 闸 3 三组机械用例 + `ExportedItem` 四变体封闭） |
| AC-8 | **零扩散且基线不换段**：`git status --porcelain` 改动集 ⊆ 白名单；`.dev-team/architecture/**` 零 diff；`pnpm api:check` 三列皆空；`bindings:export` 幂等；全量 `apiFindings` 仍 12 / 29（总 41）、`status = clean`、`passed = true`、`apiUndeclared` 仍 `off`。**crate 断言面**：闭包只减不增 `apiMissing`（契约声明闭包内 op / 类型 ⇒ 该 4 项零 `apiMissing`，`apiUndeclared` 零新增），且 `extract`（D52）与 `ts_import_specifier` 零新增调用点；全量读数与绑定幂等属移交面 | `api/mod.rs`（`mod_test.rs`：`findings` → `load_contracts` → 闭包 → 判据的组合链路）；`api/extract/mod.rs`（`mod_test.rs`：D52 留白零调用者 / 零 `unresolved` 写入）；机械面见 `## 不可测试项` 4 / 5 / 6 |
| AC-9 | **版本纪律**：`packages/desktop/package.json` `version` = `0.6.49`（只 +1 一次）；`tauri.conf.json` 仍 `"../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version` 逐字不变 | `packages/desktop/package.json`（见 `## 不可测试项` 1）；`tauri.conf.json` / `Cargo.toml`（零改动对照面，见 `## 不可测试项` 8） |
| AC-10 | **相关能力零漂移**：`openspec/specs/**` 除本 change 两份 delta 的目标能力外零 diff；`desktop-archi-model` / `repo-archi-model` / `desktop-archi-drift` / `desktop-archi-file-tree` / `desktop-archi-contract-panel` / `desktop-archi-gate` / `desktop-archi-agent` / `desktop-ipc-type-bindings` 零 diff | 静态 diff 核对面（见 `## 不可测试项` 6）；`scan/**` 与 `api/{check,paths,diff,schema,contract,emit}/**` 零 diff 同样归该面（被复用单点的本体不重复设计用例，见 `## 不可测试项` 7） |

> **覆盖核对**：proposal AC-1…AC-10 十条逐条在表内（10 行，编号独立、无合并、无遗漏）；design「修改文件」5 行逐项落表 —— ① `api/extract/mod.rs`（AC-1/2/3/4/6/8）、② `api/extract/ts.rs`（AC-1/2/3/4/5/7）、③ `api/extract/rust.rs`（AC-5/6）、④ `api/mod.rs`（AC-1/5/8）、⑤ `package.json`（AC-9）；新增文件 1 行（留痕 md）落 `## 不可测试项` 2；无表外新增。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod_test.rs

#### 待测功能

- `extract_element()`: 签名 `pub fn extract_element(root: &Path, elements: &[ArchiElement], element: &ArchiElement, entry: &str) -> Vec<ApiSymbol>`；TS 分支 = 入口派生**导出闭包**（入口唯一全量起点 → 具名名单精确 / 通配取全量面 → `visited` 键 =（目标文件, 名字集）→ `scan::ts::TsResolver::resolve` 解析 → 同元素约束按 `scan::owner::OwnerIndex::owner_of` 的 owner 子树裁（`reexport_ok`） → `(name, kind)` 去重留首次 → 别名逐层改名 → `visibility` 一律 `External`）；Rust 分支 = `rust::extract_crate` 逐字不变（`elements` / `element` 不参与，`mod` 树自锚）；入口读不到 / 语言未知 ⇒ 空 `Vec`
- `extract()`: **零变化**（D52 显式留白：单文件入口面的预留接口，产品侧零调用者，本变更 MUST NOT 接线）——只在本文件留白断言与「与 `extract_element` 同源」行为面被间接覆盖
- `ApiSymbol`（类型，来源 design「类型定义」表）: 增 `pub file: String`（定义所在文件，repo 相对 POSIX；TS 取抽取面形参、Rust 取当前模块文件；MUST NOT 是入口串；非 `Option`）；`PartialEq` / `Eq` / `Clone` 随之外延
- 私有辅助（design「私有签名变化」段登记，**经 `extract_element` 行为面观测，MUST NOT 为可测性升格**）: `struct Closure<'a>` 与 `fn collect(&mut self, file, names)`（8 步）、`reexport_ok` / `export_target` / `visited_key` / `dedupe` / `keeps`
- 模块 doc 边界句（design 修改文件表 ⑤）: 「`extract` / `ts` / 归一纯函数零读盘；`extract_element`（与 `rust::extract_crate`）按入口与闭包内目标文件读盘，零写盘、零 spawn」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract_element()` 精确名单 | 正向 | 入口 `export { A } from './x'`，`./x` 导出 `A`/`B`/`C` ⇒ 产出恰 1 符号（`name = "A"`、`file = "views/x.ts"`、`kind` 与 `line` 为 `./x` 内定义处），`B`/`C` 零命中（具名名单不虚增） | 单例 |
| `extract_element()` 通配等价 | 正向 | 同一簇「三条逐名 re-export」入口 vs「一条 `export * from './x'`」入口 ⇒ 两次产出 `Vec<ApiSymbol>` 逐字段相等（名字集与顺序均同，各 3 条） | 双夹具对拍 |
| `extract_element()` 多跳 | 正向 | 入口 `export * from './mid'`，`mid` 自带一条声明并 `export * from './leaf'` ⇒ 三条全在集，顺序 = 入口语句序 + `mid` 语句序 + `leaf` 声明序 | 单例 |
| `extract_element()` 别名改名 | 正向 | 入口 `export { A as B } from './x'` ⇒ 入集名 `B`，`file` / `line` / `kind` / `is_async` 仍指 `./x` 内 `A` 的定义（改名只改 `name`） | 单例 |
| `extract_element()` 同目标两语句 | 正向 | 入口 `export { V } from './v'` + `export type { Actions, Props } from './v'`（R-3 的本仓形态）⇒ 3 名全部在集（`visited` 键含名字集，两条语句互不吞名） | 单例 |
| `extract_element()` 值域 | 正向 | 闭包内全部符号 `visibility == External`（含跨文件并入的符号）；零 `CrateOnly` / `TestOnly` / `Unused` 产出 | 单例 |
| `extract_element()` 混合序 | 正向 | 入口自带 `export function own()` + 具名 re-export + 通配 re-export ⇒ 顺序恒 `声明 → 具名 → 通配` 且各段内按语句序 | 单例 |
| `extract_element()` 环 | 边界 | `a → b → a`（各含一条声明、互 `export *`）⇒ 零 panic、零无限递归，两符号各恰一次 | 单例 |
| `extract_element()` 菱形 | 边界 | 入口 `export * from './l'` 与 `export * from './r'`，两者都 `export * from './leaf'` ⇒ `leaf` 的符号按 `(name, kind)` 恰一条 | 单例 |
| `extract_element()` 去重键 | 边界 | 两条路径带来**同名不同 kind**（`export type A` 与 `export function A`）⇒ 两条都在（去重不按结构合并） | 单例 |
| `extract_element()` 稳定序 | 边界 | 同一输入连续两次调用 ⇒ 逐字段相等（零哈希序依赖）；同一簇顺序与「通配等价」用例互证 | 单例 |
| `extract_element()` 空面 | 边界 | 入口空源 / 仅注释 / `export * from './x'` 而 `./x` 零导出 ⇒ 空集（`list` 空档） | 逐值迭代（3 值） |
| `extract_element()` 规模 | 边界 | 入口 500 条具名 re-export ⇒ 500 符号零截断零重复；目标文件 1000 条 `export *` 名单 ⇒ 不 panic | 规模迭代（2 夹具） |
| `extract_element()` elements 空表 | 边界 | `elements = []`（无任何归属）⇒ 所有 re-export 目标判 `Owner::None` 被拒，只剩入口自身声明 | 单例 |
| `extract_element()` 元素表极端 | 边界 | 元素表仅本元素（无子元素）⇒ re-export 全拒、入口声明保留；500 元素表 ⇒ 零 panic、归属仍单点（`OwnerIndex` 只建一次） | 逐值迭代（2 值） |
| `extract_element()` owner 四值 | 异常 | 四条 re-export：① 目标归属本元素子树（`…Agent.Input`）② 归属另一元素 ③ 无元素认领（仓内无主文件 / `@/lib/**` 经 tsconfig `paths` 解析）④ 两元素等长前缀 → `Owner::Ambiguous` ⇒ 只 ① 入集，②③④ 零符号、零 panic、零 finding | 表驱动（4 行） |
| `extract_element()` 分段前缀 | 异常 | 目标归属 `…Components.AgentFoo`（裸前缀相同、按 `.` 分段比较不等）⇒ 不入集；对照 `…Components.Agent.Input` 入集 | 单例 |
| `extract_element()` 解析失败 | 异常 | 仓外包（`react-resizable-panels`）/ 不在盘（`./not-on-disk`）/ 非 TS 目标（`./data.json`、`./style.css`）⇒ 静默跳过：零符号、零 panic、零 `unresolved` 记录 | 逐值迭代（4 值） |
| `extract_element()` 入口异常 | 异常 | 入口不存在 / 目录占位 / `entry = ""` / `README.md` / `scripts/x.py` ⇒ 空集（既有两条 else 语义逐字不变） | 逐值迭代（5 值） |
| `extract_element()` entry 边界 | 边界 | `entry` 超长（>1000 chars）/ 含 emoji 与 `\n` / 零扩展名 / 含 `\0` ⇒ 空集或正常返回，零 panic | 表驱动（4 行） |
| `extract_element()` element 边界 | 边界 | `element.path` 空（容器）/ `element.fqn` 空串 / `element.fqn` 超长 ⇒ 零 panic；`path` 空 ⇒ 归属判据恒 `None`（非空 `path` 元素才可能命中子树） | 表驱动（3 行） |
| `extract_element()` Rust 分支 | 正向 | Rust 元素（入口 + `mod op;` + `pub use op::run;`）⇒ 与 `rust::extract_crate` 逐字段相等（含 `file`），`elements` / `element` 不参与分派 | 单例 |
| `extract_element()` 源码自证 | 边界 | `Language::Unknown => Vec::new()` 恰 2 处；`pub fn extract_element(` 恰 1 处；零第二份解析 / 归属：`api/` 内 `oxc_resolver` / `Resolver::new` / `TsconfigOptions` / `fn owner_of` / `prefix_matches` 全零命中；`format!("{}.",…)` 恰 1 处（分段前缀而非裸 `starts_with`）；`TsResolver::new` 恰 1 处（`Closure` 内，每元素一次）；`BTreeSet<(String, Option<Vec<String>>)>` 恰 1 处；零 `unresolved` 写入 | 单例 |
| `Closure` 及私有辅助 | 边界 | `Closure` / `collect` / `reexport_ok` / `export_target` / `visited_key` / `dedupe` / `keeps` 全部**私有**（零 `pub` 命中），只经 `extract_element` 行为面观测（MUST NOT 升格为测试可见） | 单例 |
| `extract()`（D52 留白） | 边界 | 产品 `.rs`（滤 `*_test.rs`）内 `extract::extract(` 零调用；`pub fn extract(` 恰 1 处；同一单文件 Rust 入口上 `extract` 与 `extract_element` 符号集逐字段相等（含 `file`）——留白不是死代码 | 单例 |
| `ApiSymbol.file` 形状 | 边界 | 测试夹具 `fn symbol(...)` 补 `file`；产品侧 4 处构造点（`ts::{push_function,push_type}`、`rust::{push_type,function_symbol}`）各带 `file:`；`ApiSymbol` 内 `file` 紧邻 `line` 且非 `Option` | 单例 |
| 模块 doc 边界句 | 边界 | `extract/mod.rs` doc 明写「`extract_element`（与 `rust::extract_crate`）按入口与闭包内目标文件读盘，零写盘、零 spawn」，且「TS 分支已走入口导出闭包」一句在案；`extract` / `apply_knip_report` 的留白段落逐字保留 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（入口 / 目标文件 / tsconfig） | **不 mock**：真实 `TempDir` RAII（`std::env::temp_dir()` + 进程号 + 计数器唯一命名，`Drop` 清目录），`write(relative, content)` 落真实源文与真实 `tsconfig.json` | 全部闭包用例（精确名单 / 通配 / 多跳 / 环 / 菱形 / owner 四值 / 解析失败 / 入口异常） |
| `scan::ts::TsResolver` | **不 mock**：真实构造与 `resolve`（零第二份解析是被测不变量；替身会让「复用单点」不可见）。别名形态以真实 `tsconfig.json` 的 `compilerOptions.paths` 驱动 | `@/lib/**` 的 `Owner::None` 用例、扩展名 / index 折叠用例 |
| `scan::owner::OwnerIndex` | **不 mock**：真实 `build` / `owner_of`（三值不任选是被测语义）；元素表以真实 `ArchiElement` 夹具给出（父 + 子 + 另一元素 + 等长前缀双元素） | owner 四值、分段前缀、`elements` 空表、元素表极端 |
| `ts::extract_surface` / `rust::extract_crate` | **不 mock**：真实组合（跨模块调用链即被测对象；桩掉就测不到「闭包落在外层、`ts.rs` 零读盘」） | 全部正向 / 边界用例 |
| `ArchiElement` / `ApiSymbol` 夹具 | **不 mock**：真实构造器（`path` / `fqn` / `gate` / `api` 原值给出，`file` 按用例给值） | 全部用例 |
| 进程边界（网络 / DB / 子进程 / 环境变量） | 不存在：本文件零网络、零 DB、零 spawn、零环境变量读取（`TsResolver` 的 `node_path = false` 已是既有事实），故无 mock 位 | — |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts_test.rs

#### 待测功能

- `extract_surface()`: **新增** `pub fn extract_surface(file: &str, source: &str) -> ExportSurface` —— 一次 `program.body` 遍历产出三面（声明 / 具名名单 / 通配名单）；纯读源文本 + AST（零读盘、零写盘、零 spawn）；`parsed.panicked` ⇒ 三面全空（不 panic、不猜）；每符号填 `file`
- `extract_file()`: **零签名 / 零产出语义**（`= extract_surface(file, source).symbols`）——只认 `export <声明>`，对 `export … from` / `export *` / `export default` 零符号（闸 1 / 闸 3 的判红面，保留不改）
- `apply_knip_report()`: **零变化**（`unused` 的唯一降级来源，零产品调用者的预留接口，留白逐字不动）
- 类型面（design「类型定义」表）: `ExportSurface { symbols, named, star }` 与 `NamedExport { local, exported, specifier }`（**pub**，`Debug + Clone`）；`ExportedItem<'a,'b>{ Declaration / Named / Star / None }`（**私有**，四变体封闭，MUST NOT 扩第五态）
- 导出形态单一分派点（AC-7 不变量）: `exported_item(statement)` 三臂 + `_ => None`；`export * as ns` 在第三臂内按 `exported.is_some()` 退回 `None`；`export { localA }` 与 `export default` **靠不匹配实现**

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract_surface()` 声明面 | 正向 | `export function` / `export interface` / `export class` / `export type` / `export enum` / `export const V = memo(...)` 混合源 ⇒ `symbols` 逐条（名字 + `kind`）在案，`named` / `star` 均空 | 表驱动（6 行） |
| `extract_surface()` 具名面 | 正向 | `export { A, B as C } from './x'` ⇒ `named` 恰 2 条：`(local="A", exported="A", specifier="./x")` / `(local="B", exported="C", specifier="./x")`；`symbols` / `star` 空 | 单例 |
| `extract_surface()` 具名面（type 形态） | 正向 | `export type { T } from './z'` ⇒ `named` 恰 1 条（`local == exported == "T"`、specifier 逐字）；`export_kind = Type` 不改变名单面 | 单例 |
| `extract_surface()` 通配面 | 正向 | `export * from './y'` ⇒ `star == ["./y"]`；两条 `export *` ⇒ 两条按语句序；`symbols` 空 | 逐值迭代（2 行） |
| `extract_surface()` 三面同源 | 正向 | 混合源（声明 + 具名 + 通配）⇒ 三面各自如实、互不串味，`symbols` 顺序 = 声明语句序 | 单例 |
| `extract_surface()` `file` 落值 | 正向 | `extract_surface("views/x.ts", src)` ⇒ 每符号 `file == "views/x.ts"`；`extract_file` 同值（`file` 形参是符号事实的唯一来源） | 单例 |
| `extract_file()` 零语义 | 正向 | `extract_file(file, source)` 与 `extract_surface(file, source).symbols` 逐字段相等（含 `file` / `line` / `kind`） | 单例 |
| 留白 · 无 `from` 的具名 | 异常 | `export { localA };`（导出本文件非导出声明）⇒ 三面皆空（`Statement::ExportNamedDeclaration` 零命中由源码断言另守） | 单例 |
| 留白 · `export * as ns` | 异常 | `export * as ns from './x'` ⇒ `star` / `named` / `symbols` 全空（臂内 `exported.is_some()` 退回 `None`） | 单例 |
| 留白 · `export default` | 异常 | `export default function f(){}` / `export default () => null` / `export default class X{}` / `export default 42` ⇒ 三面皆空 | 表驱动（4 行） |
| `extract_surface()` 异常源 | 异常 | 空源 / 未闭合括号 / 非法 token ⇒ 三面全空、零 panic（`panicked = true` 即不猜） | 逐值迭代（3 值） |
| `extract_surface()` 名单形态边界 | 边界 | `export { "x-y" as A } from './x'`（字符串字面量 `local`）⇒ 跳过该条、其余照收；`export {}` ⇒ `named` 空；`export { A as A } from './x'` ⇒ `local == exported` | 表驱动（3 行） |
| `NamedExport` 字符串边界 | 边界 | `from ''`（空 specifier）/ 超长 specifier（>1000 chars）/ 含 emoji 与 `\n` ⇒ 原样进 `specifier`（或 `star`），零 panic（解析归 `TsResolver`，本文件不解析） | 表驱动（3 行） |
| `extract_surface()` 规模 | 边界 | 500 条具名 re-export ⇒ `named.len() == 500`；1000 条 `export *` ⇒ `star.len() == 1000`；100 KB 声明源 ⇒ `symbols` 不截断 | 规模迭代（3 夹具） |
| 闸 2（改写） | 边界 | `include_str!("ts.rs")` 内 `match statement` 恰 **1** 处、`Statement::` 恰 **3** 处（= 臂数）、`Statement::ExportNamedDeclaration` 与 `Statement::ExportDefaultDeclaration` **零命中**（含 doc 注释，doc MUST NOT 写出）、`ArrowFunctionExpression` 零命中；`fn exported_item` 定义恰 1 处 | 单例 |
| 闸 1（零改动） | 边界 | `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {` 首行**逐字**（既有断言原文保留，MUST NOT 改写） | 单例 |
| 闸 3（零改动） | 边界 | `export { a, b } from './x'` / `export * from './y'` / `export type { T } from './z'` ⇒ `extract_file` 零符号（既有断言原文保留） | 逐值迭代（3 值） |
| `ExportedItem` 封闭 | 边界 | 四变体（`Declaration` / `Named` / `Star` / `None`）恰 4 个、枚举**私有**（零 `pub enum ExportedItem` 命中）、定义恰 1 处；`Statement::` 臂集合 = `{ExportDeclaration, ExportFromDeclaration, ExportAllDeclaration}` | 单例 |
| `apply_knip_report()` 零变化 | 边界 | 既有四例（降级 / `None` 与空报告 / 异常报文 / 值域 `{External, Unused}`）逐字保留，断言值不变；`Visibility::CrateOnly` / `TestOnly` 仍零命中、`Unused` 产出点仍恰 1 | 表驱动（4 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `oxc` 解析（AST / `SourceType`） | **不 mock**：真实 `Parser` + `Allocator`（三态分派语义即被测对象；桩掉就测不到「留白靠不匹配实现」） | 全部正向 / 留白 / 异常源用例 |
| `ApiSymbol` / `ExportSurface` / `NamedExport` | **不 mock**：真实值逐字段对拍（`ExportSurface` 三面直接断言） | 三面用例、`file` 落值 |
| knip 报告 | **不 mock 进程**：以 `serde_json::json!` 直传入参（不 spawn `knip` —— 进程边界由 CLI 侧承担，`api/**` 零 spawn） | `apply_knip_report` 四例 |
| 文件系统 | 不使用：本文件零读盘（闸 3 与「闭包在外层」的边界即此），故无 TempDir、无文件 fixture | — |
| 源码自证面 | 不用 mock：`include_str!("ts.rs")` 读真实产品源文做计数断言 | 闸 1 / 闸 2 / 闸 3 / `ExportedItem` 封闭 / `apply_knip_report` 单点 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust_test.rs

#### 待测功能

- `extract_file()`（Rust）: **零签名** `pub fn extract_file(file: &str, source: &str, crate_name: &str) -> Vec<ApiSymbol>`；`file` 由「占位形参」改为**落符号**（`crate_name` 仍只作诊断留痕）
- `extract_crate()`: **零签名**；`ApiSymbol.file` = **当前模块文件**（贯穿 `walk_items` / `walk_modules`；文件型 `mod` 取定位文件、内联 `mod` 沿用父文件、`#[path]` 模块取定位文件；crate 根以 `entry` 作根文件）——与 `extract_element` 的 Rust 分支逐字段相等（`file` 一并相等）
- 私有签名变化（经上述两个公开入口观测，MUST NOT 升格）: `walk_items` / `push_type` / `function_symbol` 各增 `file: &str` 形参并落 `ApiSymbol.file`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract_file()` `file` 落值 | 正向 | `extract_file("crates/x/src/op.rs", "pub fn run() {}", …)` ⇒ 符号 `file == "crates/x/src/op.rs"`（入参逐字） | 单例 |
| `extract_crate()` 入口声明 | 正向 | 入口 `lib.rs` 内 `pub fn top()` / `pub struct Handle` ⇒ `file == entry` 全串 | 单例 |
| `extract_crate()` 子模块 | 正向 | 入口 `mod op;`、`op.rs` 内 `pub fn run` ⇒ `run.file == "…/op.rs"`（= 子模块文件，MUST NOT 是入口串） | 单例 |
| `extract_crate()` `mod.rs` 形态 | 正向 | `mod op;` 命中 `…/op/mod.rs` ⇒ 该子模块符号 `file == "…/op/mod.rs"` | 单例 |
| `extract_crate()` `#[path]` 模块 | 正向 | `#[path = "…/archi.i.rs"] mod api_gen;` 与三层 `../`（`../../archi.i.rs`）两形态 ⇒ 符号 `file` = 定位到的文件路径（两个形态都命中时逐字段相等） | 逐值迭代（2 行） |
| `extract_crate()` 内联 `mod` | 正向 | 入口 `pub mod a { pub fn f() {} }` ⇒ `f.file == entry`（内联 `mod` 与父同文件，MUST NOT 是 `a` 之类的模块名） | 单例 |
| `file` 形态规范 | 正向 | 全部产出符号 `file` 非空、`/` 分隔、零 `\`、零前导 `/` 与 `./`、零 `../`（repo 相对 POSIX） | 表驱动（全部夹具复用） |
| `extract_crate()` 深度 | 边界 | 五层嵌套模块链 ⇒ 每符号 `file` 指向其自身所在文件（不串到上层入口） | 单例 |
| `extract_crate()` 规模 | 边界 | 300 个 `pub fn` 分布在 30 个模块 ⇒ `file` 取值集合恰 30 个互异路径、符号数 300 零丢失零重复 | 规模迭代（1 夹具） |
| `extract_crate()` 同源 | 边界 | 单文件 crate（入口即全部实现、无 `mod`）⇒ `extract_crate` 与 `extract_file(入口源)` 逐字段相等（含 `file`） | 单例 |
| 抽取面零变化 | 异常 | 既有语料断言值不变：四类声明抽取 / 多行签名归一 / `owner` 与可达性 / 构造器不过滤 / `visibility` 四值（`External` / `CrateOnly` / `TestOnly` + 零 `Unused`）/ `#[path]` 限根三组（反斜杠越根 / 前向形态 / 未知形态）——`file` 只是**新增字段**，既有断言逐字保留 | 表驱动（既有 6 组） |
| 入口异常 | 异常 | 入口不存在 / 入口是目录 / 入口非 Rust 源（`index.ts`）/ `entry = ""` / 语法错 ⇒ 空 `Vec`（零 panic、零伪造符号） | 逐值迭代（5 值） |
| 私有形参 | 边界 | `walk_items` / `push_type` / `function_symbol` 的 `file` 形参不外露（零 `pub` 升格）、`ApiSymbol.file` 赋值点与符号构造点一致（4 处构造点含 `rust::` 2 处） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（入口 / `mod` 文件 / `#[path]` 目标） | **不 mock**：真实 `TempDir`（沿用文件内既有自建形态），真实源文落盘 | 子模块 / `mod.rs` / `#[path]` / 深度 / 规模 / 入口异常 |
| `syn` 解析 | **不 mock**：真实 `syn::parse_file`（AST 归一即被测对象；正则抽签名是被明令禁的复发路径） | 全部用例 |
| 模块定位（`locate_module` / `locate_declared_module`） | **不 mock**：真实调用（限根三步纪律与 `file` 取值同一次遍历给出） | `#[path]` 两形态、`mod.rs` 形态 |
| 进程边界 | 不存在：零网络 / 零 DB / 零 spawn / 零宏展开 | — |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

#### 待测功能

- `bootstrap()`: **零签名** `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>`；内部注记改按 `symbol.file` 输出（`entry` 退出注记面）；失败面（`ApiError::Element` 与 `entry_problem_reason` 文案素材）零变化
- 私有 `bootstrap_text(fqn: &str, symbols: &[ApiSymbol])`（**`entry` 形参退出**）与 `bootstrap_types`（`待剪枝：{kind} @ {file}:{line} {visibility}`）—— 由 `bootstrap` 输出面观测（design「私有签名变化」段）
- 两处产品调用点（私有 `element_symbols` 与 `bootstrap`）: 补 `elements` / `element` 实参 —— `extract::extract_element(root, elements, element, &entry)`；`ApiSnapshot` 三面、出口八行、`entry_state` 取数方式零变化
- 组合链路（本文件是链路最上层调用方，用例挂靠于此）: `bootstrap → extract_element → ts::extract_surface / scan::{ts,owner}`、`load_contracts → element_symbols → 闭包`、`findings → load_contracts → 闭包 → check`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `bootstrap()` 注记真值源 | 正向 | 符号定义在入口内（crate 形态 `…/src/lib.rs` 与单文件形态各一）⇒ 注记 `@ <entry>:<line>`（既有断言值不变，判据说明改「取符号定义文件，定义在入口内时与入口同值」） | 双夹具对拍 |
| `bootstrap()` TS 一跳 | 正向 | TS 元素（文件型 `path`，入口 barrel 一跳）⇒ 注记 `@ <目标文件>:<line>`；MUST NOT 出现 `@ <入口 index.tsx>:<line>` 形态 | 单例 |
| `bootstrap()` Rust 子模块 | 正向 | Rust 元素子模块内的 `pub fn` ⇒ 注记指向**子模块文件**（不再是入口串，病灶三的反向判据） | 单例 |
| `bootstrap()` 类型注记 | 正向 | 类型条目的 `待剪枝：{kind} @ {file}:{line} {visibility}` 取 `symbol.file`；同符号 `file:line` 与 op 注记同源 | 单例 |
| 组合 `bootstrap → 闭包` | 正向 | 合成 barrel（父元素 + 4 子元素 + 6 条有效 re-export，含同目标两语句形态，另加 1 条跨元素被拒）⇒ 草稿 `### <op>` 集与 `api-types` 键集 == 闭包符号集（7 名，构造器按既有纪律排除），未导出的 `*Props` 零命中 | 单例 |
| 组合 `load_contracts → element_symbols` | 正向 | 对**有契约文件**的 TS barrel 元素跑 `load_contracts` ⇒ `snapshot.symbols[fqn]` == `extract_element` 的闭包产出（逐字段相等，含 `file`）——`element_symbols` 补参生效、零第二套取数 | 单例 |
| 组合 `findings → 闭包 → check` | 正向 | 契约声明闭包内 2 个 op + 2 个 `$defs` 类型 ⇒ 该 4 项零 `apiMissing`；`apiUndeclared` 仍 `off`（零新增条目）、`apiSignatureMismatch` 仍逐符号比对（闭包只减不增） | 单例 |
| 注记面 `entry` 退出 | 边界 | `api/mod.rs` 内 `@ {entry}` 与 `@ 入口` 零命中；`symbol.file` 写出点恰 2 处（op 注记 + 类型注记）；`bootstrap_text` 签名 `(fqn, symbols)`（无 `entry` 形参） | 单例 |
| `bootstrap()` 签名 | 边界 | `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>` 逐字不变（公开面零变化） | 单例 |
| 空符号集草稿 | 边界 | 入口读不到 / barrel 目标全被拒（`Owner::None`）⇒ 零 `###`、`api-types` 为 `{}`——既有「空骨架」形态回归（改前本仓前端元素的形态） | 逐值迭代（2 值） |
| `bootstrap()` 异常面 | 异常 | 元素不存在 / FQN 空串 / FQN 含 emoji / 无可用入口（`Absent` / `Invalid` 四理由）⇒ `Err(ApiError::Element(..))`，文案与候选清单措辞零变化 | 表驱动（4 行） |
| 零写盘 | 边界 | `bootstrap` 调用前后 `TempDir` 全量字节快照相等（既有断言保留）；闭包读盘不产生写盘 | 单例 |
| 调用点机械锚 | 边界 | `element_symbols` 与 `bootstrap` 两处各恰 1 次 `extract_element(` 调用且实参含 `elements` / `element`（`api/` 内产品侧恰 2 处，零第三处） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（模型 / 契约 / 源码 / barrel） | **不 mock**：真实 `TempDir`（`write_model` / `write_contract` / `write` 既有夹具）+ 真实字节快照 | 全部用例 |
| 模型装载（`elements_of` / `dto::graph_view`） | **不 mock**：真实读模型 TOML（`declare_ancestors` 辅助补祖先段） | 元素面 / 异常面 / 组合链路 |
| `extract::extract_element` / `ts::extract_surface` / `scan::{ts,owner}` | **不 mock**：真实组合（组合链路的被测对象正是「两处调用点传参正确 + 取数面单点」；桩掉即测不到） | 三条组合用例 |
| `contract::parse` / `schema::build_schema` / `check::check` | **不 mock**：真实链路（`findings` 是对外唯一入口，判据单点在 `check.rs`，本变更对它零改动） | `findings` 组合用例 |
| `knip` 报告 | 不涉及：`bootstrap` 不读 knip（既有留白，产品源码零调用） | — |
| 进程边界 | 不存在：零网络 / 零 DB / 零 spawn / 零 IPC | — |

---

## 不可测试项

1. **`packages/desktop/package.json`（`version` `0.6.48` → `0.6.49`）** —— 判定：**无单元测试载体**。原因：① 纯配置值变更，无函数 / 类型 / 入参（design「配置」表该行只有值）；② 为「版本号 = 0.6.49」写测试会把版本纪律耦进套件（既有仓库纪律：基线读数不写进断言）；③ 仓内涉及 `tauri.conf.json` 相对版本解析的既有用例（`crates/infra/checks/src/mutation/cargo_mutants_test.rs`）以自造副本驱动、不读本仓真实文件 ⇒ 与本变更零耦合。承接通道：AC-9 的 `git diff` / `grep -n '"version"'` 核对（`0.6.49`、只 +1 一次、`scripts` 段零 diff），由桌面端 static-check 执行。

2. **`openspec/changes/ts-export-closure-extraction/reports/api/closure-evidence.md`（新增留痕，design「新增文件」唯一一行）** —— 判定：**不是被测源文件，而是 AC 的实测载体**。原因：① 其「正确性」由 `archi-api-gen bootstrap <FQN>` 的 stdout 与门禁报告读数定义（元素级 / 全量两种口径，§0 须声明）；② `desktop-archi-api-check` 与本 change 的「语料纪律」明写：**本仓模型 / 契约 MUST NOT 进 crate 语料**，故 AC-1 的「7 名」、AC-4 的「本仓 6 条 re-export 全落子树」、AC-5 的逐符号 `file:line` 都是**仓库实测留痕**，不写成 crate 断言常量（crate 侧只断言合成等价语料，见 `## 单元测试` 第 1 与第 4 节）；③ 若把读数写进断言，即与「基线不写进断言」冲突并随元素增减漂移。承接通道：AC-1（改前零符号 vs 改后 7 名 + 3 op / 4 类型、`AgentInputProps` / `AgentMessagesProps` / `isToolPart` 零命中）、AC-5（`@ agent-input.tsx:152` / `@ session-panel.tsx:230` / `@ agent-messages.tsx:33` 等逐符号 `file:line`，零 `@ index.tsx:<line>` 形态）、AC-8 §3（`api:check` 三列空与 `api:export` 二跑零写入）。读数若与期望不符，按 design AC-1 排查序（先 `entry_state` 是否 `Resolved`，再 `owner_of` 是否命中四个子元素）处置，**MUST NOT** 放宽同元素约束或改判据凑数。

3. **`crates/core/archi/src/api/check.rs`（零改动）与 `api/check_test.rs`（仅夹具补 `file`）** —— 判定：**无新增用例面，只做形状适配**。原因：① 十条判据实现逐字不变（补丁只改「导出集」口径的**取数面**，判据本体零改动），为它新增用例即制造第二套判定语料；② `function()` / `type_symbol()` 两处夹具补 `ApiSymbol.file` 后，既有十判据正反例、`entry` 互斥序、签名归一、留白双向断言**逐字保留、断言值零变化**（T1）。承接通道：AC-8 的「闭包只减不增 `apiMissing`」在 `## 单元测试` 第 4 节的 `findings` 组合用例里有 crate 断言；`apiFindings` 的分桶读数属移交面（见 4）。

4. **全量 `apiFindings` 读数（12 / 29，总 41，分母 44，`status = clean`，`passed = true`，`apiUndeclared` 仍 `off`）** —— 判定：**无单元测试载体，属移交面**（design D9 的证据面切分）。原因：① 读数的取数通道是门禁 runner / IPC，实现面（七工具 + `bash`）不持有；② 本仓基线明写「MUST NOT 写进断言（易随元素增减漂移）」，断言只锚定「实测表与报告逐项相等」；③ 为它写测试等于测试既有判据而非本变更。承接通道：AC-8 由桌面端 test-execution 回填留痕 §2；若落地时无门禁报告，留痕只能写「待 test-execution 回填」，**MUST NOT** 手写伪读数、MUST NOT 用元素级 `bootstrap` 读数顶替。

5. **`bindings:export` 幂等与 `packages/desktop/src/types/generated/**` 零 diff** —— 判定：**无单元测试载体**（`ApiSymbol` 不进 `dto` / 不进 specta / 不进 bindings ⇒ 绑定零再生成）。承接通道：AC-8 的 `pnpm server:check` + `bindings:export` 二跑零 diff 由桌面端 static-check 执行；实现面以 `src/types/generated/**` 零 diff 作等价证据。

6. **零 diff 面：`.dev-team/architecture/**`、`scan/**`、`api/{check,paths,diff,schema,contract,emit}/**`、`openspec/specs/**`** —— 判定：**无单元测试载体，靠静态 diff 核对**。原因：① 这些面是 design D10 白名单与「不要修改」表逐项点名的零触碰面；② 为「零 diff」写测试需要读取本仓真实路径并与基线比对，正是「基线不写进断言」禁止的形态；③ 其中 `scan/**` 被复用（`TsResolver::resolve` 无 `unresolved` 副作用、`OwnerIndex` 三值不任选），其**本体行为**由既有 `scan/ts_test.rs` / `scan/owner_test.rs` 覆盖，本变更只通过 `extract_element` 行为面观测（见 7）。承接通道：AC-8（`git status --porcelain` ⊆ 白名单）、AC-10（`git diff --stat` 逐目录零输出），由桌面端 static-check 与实现面核对执行。

7. **被复用的既有单点本体：`scan::ts::TsResolver::resolve` 与 `scan::owner::OwnerIndex::{build, owner_of}`** —— 判定：**不重复设计用例**。原因：两文件是 design「不要修改」表逐条点名的零改动对象（出现 diff 即说明造了第二份解析 / 归属）；`resolve` 的 tsconfig 就近 / `baseUrl` / `paths` / 扩展名 / index 折叠语义与 `owner_of` 的三候选并集 / 最长前缀 / `Ambiguous` 不任选语义已在既有 `scan/ts_test.rs` / `scan/owner_test.rs` 在案。本变更的测试面是**消费侧**（owner 四值、别名解析、非 TS 目标过滤），见第 1 节 owner 四值与解析失败用例。

8. **`packages/desktop/src-tauri/tauri.conf.json`（零内容改动）与 `packages/desktop/src-tauri/Cargo.toml`（crate 版本行零改动）** —— 判定：**无单元测试载体**（判据是文件内容事实，无行为可测）。承接通道：AC-9 的 `git diff` 零输出核对（`tauri.conf.json` 仍为 `"../package.json"` 引用形态、`Cargo.toml` 的 `[package] version = "0.1.0"` 逐字不变）。

> **不做项（明确不登记为测试缺口）**：① `.i.ts` 试点与 `paths::ts_import_specifier` 接线 —— proposal 六 / design D7⑥ 裁定不在本变更做（四条理由在案），故**不设计任何端到端 `api-reexport` 契约用例**；反向判据「`api/extract` 与 `api/mod.rs` 内 `ts_import_specifier` 零命中、零 `.i.ts` 试点」落在 AC-8 的静态核对面；② 前端行为契约（`api-op` 面）—— 解构 props + 内联类型字面量 ⇒ 参数类型恒 `Unknown`，本能力只承诺「类型面 + 名面」，D8 明写 AC-1 只数名字不验签名，故**不为前端 op 签名设计用例**；③ `export { localA }` / `export * as ns` / 跨元素与仓外 re-export / `export default` 的**补齐** —— 四类留白只设计「零产出」否定用例，不设正向用例；④ 前端 `vp test` 面与 `knip` 死代码扫描 —— 零前端源码改动、零新增 export-by-test（既有测试辅助函数均为文件内私有 `fn`），其回归由 `client:check` 整体承接。
