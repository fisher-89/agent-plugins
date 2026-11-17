# 设计: ts-export-closure-extraction

> **变更**: ts-export-closure-extraction（TS 取数面沿 `export … from` 走 · 导出闭包）
> **日期**: 2026-11-17
> **基于**: `proposal.md`（口径裁定 owner 子树 / 闭包算法 / 三态单点分派 / `ApiSymbol.file` / 四类留白 / 不做 `.i.ts` 试点 / 版本 +1 / AC-1…AC-10 / R-1…R-10）与本 change `specs/` 下两份已在案的 delta（`desktop-archi-api` 1 ADDED + 1 MODIFIED、`desktop-archi-api-check` 1 MODIFIED）
> **性质**: **取数面能力补齐**——`api::extract` 的 TS 分支由「入口单文件」扩为「入口导出闭包」，`ts.rs` 的导出形态分派由单臂扩为三态但仍单点，`ApiSymbol` 增定义文件事实。**零模型数据改动、零契约资产改动、零生成物 diff、零新增判据、零新增依赖**；`package.json` 一次 patch。
> **测试面**: 五份 `*_test.rs` sidecar（case 本体、闭包语料、闸 2 改写、夹具补 `file`）归测试相位——本设计只钉住产品侧不变量与判据语义，`tasks.md` 不列用例任务（末段给逐项移交清单）。

---

## 起点实况核实（工作树实测）

| 面 | 实测 |
|---|---|
| 取数面现状 | `extract_element`（`api/extract/mod.rs:208`）TS 分支只有一句 `fs::read_to_string(root.join(entry)) → ts::extract_file(entry, &source)`；`ts::extract_file`（`api/extract/ts.rs:49`）经 `exported_declaration`（`:66`，`match statement` 恰 1 处、`Statement::` 恰 1 处）只认 `Statement::ExportDeclaration` ⇒ 纯 barrel 恒零符号 |
| 分派面现状 | `ts.rs` 内 `grep -c "match statement"` = **1**、`grep -c "Statement::"` = **1**（唯一臂 `ExportDeclaration`）；`export {…} from` / `export * from` 零分支 |
| 判据语料（正面） | `packages/desktop/src/components/agent/index.tsx` 全文 6 条语句：`export { AgentInput } from './agent-input'` / `export type { AskAnswerActions } from './agent-messages'` / `export { AgentSessionView } from './agent-session-view'` / `export type { AgentSessionActions, AgentSessionViewProps } from './agent-session-view'` / `export { SessionPanel } from './session-panel'` / `export type { SessionPanelState } from './session-panel'` ⇒ 7 个名字，其中 `agent-session-view` 被**两条**语句指向（`visited` 键必须含名字集，R-3） |
| 定义位置（AC-5 期望值） | `agent-input.tsx:152 export function AgentInput` / `agent-messages.tsx:33 export interface AskAnswerActions` / `agent-session-view.tsx:122 export function AgentSessionView`、`:19 export interface AgentSessionActions`、`:30 export interface AgentSessionViewProps` / `session-panel.tsx:230 export function SessionPanel`、`:12 export interface SessionPanelState`；未被 barrel 导出的 `AgentInputProps`（`agent-input.tsx:12`）/ `AgentMessagesProps`（`agent-messages.tsx:39`）即「具名名单不虚增」的反向判据 |
| 模型语料 | `archi_module("App.Frontend.Components.Agent")`：`path = ["./packages/desktop/src/components/agent/index.tsx"]`（**文件型单值**）、`api = declared`、`contractPath = null`（⇒ 现落 `apiMissing` 桶）、子元素 **7** 个；四个重导出目标各自是元素（`AgentInput` / `AgentMessages` / `AgentSessionView` / `SessionPanel`），`AgentBlocks` / `AgentError` / `ToolCards` 未出现在 barrel |
| oxc AST 形态（0.146.0） | `Statement::{ExportDeclaration, ExportNamedDeclaration, ExportFromDeclaration, ExportAllDeclaration, ExportDefaultDeclaration}` 五变体并存：`export <声明>` → `ExportDeclaration`；`export { localA }`（无 `from`）→ `ExportNamedDeclaration`；`export { A as B } from './x'` → `ExportFromDeclaration{specifiers, source, export_kind}`；`export * from './y'` 与 `export * as ns from './y'` → `ExportAllDeclaration{exported: Option<…>, source}`；`export default` → `ExportDefaultDeclaration` |
| 可复用单点 | `scan::ts::TsResolver::resolve(&mut self, file_rel, specifier) -> Option<String>`（`scan/ts.rs:156`）只读缓存、无 `unresolved` 副作用；`scan::owner::OwnerIndex::{build, owner_of}`（`scan/owner.rs:74` / `:106`）三值不任选；两处都 `pub`，`api → scan` 已有先例（`api/diff.rs` 用 `walk_source_files`、`api/paths.rs` 用 `is_source_file`） |
| 产品调用点（恰两处） | `api/mod.rs:420`（`element_symbols`，`load_contracts` 内，`elements` / `element` 本已在手）与 `api/mod.rs:651`（`bootstrap`，同样在手）；`extract(element, file, source)`（`extract/mod.rs`）产品侧**零调用者**（D52 留白） |
| 注记面现状 | `bootstrap_text(fqn, entry, symbols)`（`api/mod.rs:673`）写 `<!-- {visibility} @ {entry}:{line} -->`；`bootstrap_types`（`:723`）写 `待剪枝：{kind} @ 入口:{line} {visibility}` ⇒ 闭包一跳后两处都会指错文件 |
| 判据/报告面 | `drift` 的 `apiFindings` 十值与 `check.rs` 的导出集比对逐字不动；本仓契约资产只有 `Infra/App.Backend.Infra.ArchiRuntime.md` 一份（Rust 元素）⇒ `element_symbols` 目前只对 Rust 元素跑，`apiFindings` 12 / 29（总 41）、分母 44 |
| 版本面 | `packages/desktop/package.json:3` `"version": "0.6.48"`；`tauri.conf.json:4` `"../package.json"`（引用形态）；`src-tauri/Cargo.toml:6` `version = "0.1.0"`（独立版本线） |

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 闭包算法形态 | **逐字照 proposal 第二节**：入口唯一全量起点（`names = None`）、具名名单精确（只取 `local`、以 `exported` 入集）、通配取全量面、`visited` 键 = `(目标文件, 本次请求的名字集)`、`(name, kind)` 去重留首次、`visibility` 一律 `External`。递归在 `extract/mod.rs` 一层（私有 `Closure`），MUST NOT 下沉进 `ts.rs` | 三条机械理由：① 键含名字集是 R-3 的唯一拦挡（本仓 `agent-session-view` 两条语句会互相吞掉 2 个名字且零报警）；② 终止性由 `visited` 保证，键集 ⊆ 文件 × ({`None`} ∪ 单元素名字集) ⇒ 有限，故 MUST NOT 再叠深度上限（叠了就要处理「触顶不冒充完整面」的第二套语义）；③ 顺序 = 入口语句序（`symbols` → `named` 语句序 → `star` 语句序）+ 目标文件声明序 ⇒ 同输入两次调用逐字相等（AC-3），零哈希序 |
| D2 | 导出形态的单点收口 | `exported_declaration` 扩成 `exported_item(statement) -> ExportedItem`：**三臂**（`Statement::ExportDeclaration` / `Statement::ExportFromDeclaration` / `Statement::ExportAllDeclaration`）+ `_ => ExportedItem::None`；`export * as ns` 在第三臂内按 `exported.is_some()` 退回 `None`；`export { localA }` 与 `export default` **靠不匹配实现**（`Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 文本零命中）。`extract_file` = `extract_surface(file, source).symbols`，首行签名逐字不变 | 直觉写法是「再开一个 `match statement` 拿 re-export 名单」——那会破坏「导出形态单一收口」这条既有不变量（R-1 已指出闸 2 的计数必须随之升级为「单点 + 臂集合」）。三臂形态让「新形态」只能落在 `_`（显式留白）而不是落进某个既有分支 |
| D3 | `ApiSymbol.file` 的语义与来源 | 单字段 `pub file: String`（repo 相对 POSIX），**语言无关同义**「定义所在文件」：TS 取 `extract_surface` / `extract_file` 的 `file` 形参；Rust 取贯穿 `walk_items` / `walk_modules` 的**当前模块文件**（内联 `mod` 与父同文件）。MUST NOT 以入口串顶替（哪怕只对 Rust），MUST NOT 用 `Option<String>` / 按语言分叉 | 病灶三的本体是「注释说谎」：`pub fn` 写在 `src/op.rs`、注记写 `src/lib.rs` 与 TS 闭包一跳后同类。填入口串会让同一字段在两种语言下不同义，读者无法靠字段名判断；`Option` 则把「总该有值」降级成可选，等于允许第二次说谎（R-2） |
| D4 | 复用而非重造（边界纪律） | specifier 解析只用 `scan::ts::TsResolver::resolve`；文件归属只用 `scan::owner::OwnerIndex::owner_of`。每 `extract_element` **构建一次** `OwnerIndex` 与 `TsResolver`（不预铺、不做跨元素缓存） | 与仓库「零第二份解析 / 零第二份归属」同族（出现 `scan/ts.rs` / `scan/owner.rs` 的 diff 即说明造了第二份）。频次面：唯一两处调用点按元素单发（`load_contracts` 只对有契约文件的元素取数、`bootstrap` 单元素），本期无需预铺（R-6 的硬约束只写「MUST NOT 造第二份」） |
| D5 | 递归目标的白名单 | 只沿 `paths::language_of(target) == Language::TypeScript` 的目标递归；`.json` / 解析不可得 / 不在盘 / 仓外一律静默跳过（零 panic、零 `unresolved`、零 finding） | `TsResolver` 的扩展名表含 `.json`（node 解析规则要求），但 JSON 不是导出面；用既有 `language_of` 单点过滤而不是在闭包里另写扩展名清单。「取数器非权威」：缺口由契约面判据显示，MUST NOT 为它造新判据（与 `apiRefMissing` 的语义分工） |
| D6 | 注记面的口径 | `bootstrap_text(fqn, symbols)`（`entry` 退出注记面）写 `<!-- {visibility} @ {symbol.file}:{line} -->`；`bootstrap_types` 的 `待剪枝` 注记把字面量「入口」换成 `symbol.file`。`pub fn bootstrap(root, element) -> Result<String, ApiError>` **签名零变化** | AC-5 的正反判据都落在这一格：「指向真实定义文件」与「MUST NOT 出现 `@ <入口>:<line>` 冒充」。`entry` 从 `bootstrap_text` 去掉而不是留着不用——留着就是下一次被误用的接口 |
| D7 | 明确不做（与 spec 留白逐条对齐） | ① `export { localA }`（无 `from`）不产出；② `export * as ns from` 跳过；③ 跨元素 / 仓外 re-export 静默跳过、零 finding；④ `export default` 不抽；⑤ `extract(element, file, source)`（D52）保持零产品调用者，MUST NOT 借本变更接线；⑥ `paths::ts_import_specifier` 保持 spec 的「待接线留白（`.i.ts` 零实例）」口径——本变更 **MUST NOT 新增调用点**（既有调用点只在 `api/emit/ts.rs`），也不做 `.i.ts` 试点（提案六的四条理由已在案，若下游要求在 C 内出实例须另开 change） | 每一条都对应一个「看起来顺手」的扩张；不做清单写在这里，是为了让实现期不必重新论证一遍。⑤⑥ 额外有一条机械理由：接线会改 `apiFindings` 基线 / `INDEX.toml` 条目数（AC-8 的「零扩散」会立刻破） |
| D8 | 前端能力边界的表述 | 本变更只承诺「类型面 + 名面可用」：`bootstrap` 对前端元素的产出是非空骨架（名 + 类型条目 + 真实 `file:line`），`api-op` 面在前端继续留白（解构 props + 内联类型字面量 ⇒ 参数类型恒 `Unknown`）。AC-1 **只数名字、不验签名** | R-7：若不写这条，「闭包做完了」会被读成「前端行为契约可写」，紧接着就是一条手写的假签名（`apiSignatureMismatch` 立刻 warn） |
| D9 | 证据面切分（口径诚实） | **实现面可核**：`archi-api-gen bootstrap <FQN>` 的 stdout（元素级口径）、`api:check` 三列空、`git status` 白名单、`cargo` 编译面、`grep` 源码不变量。**移交面**：一次全量检查的 `apiFindings` 分桶（12 / 29 / 41、`status = clean`）与 `bindings:export` 幂等输出——`apiFindings` 与扫描模式无关，但取数通道是门禁 runner / IPC，实现面（七工具 + `bash`）不持有 | 与 `frontend-component-element-coverage` 的 D11 同一纪律：把不可达的读数写成 checkbox 等于要求实现者手写伪证据。留痕 MUST 注明口径（**元素级** `bootstrap` 单元素 vs **全量**检查），避免后人把读数差异当回归 |
| D10 | 零扩散白名单 | 允许改动集 = {本 change 目录、`api/extract/{mod,ts,rust}.rs`、`api/mod.rs`、五份 `*_test.rs`（测试相位）、`packages/desktop/package.json`}。`.dev-team/architecture/**`（含 `api/INDEX.toml` 与三份契约资产）、`scan/**`、`api/{check,paths,diff,schema,contract,emit}/**`、`packages/desktop/src/types/generated/**`、`openspec/specs/**` 一律零 diff | 生成物是模型 + 契约的函数，本变更两者都不动 ⇒ `api:export` 二跑「0 份写入」、`api:check` 三列空；`ApiSymbol` 不进 `dto` / 不进 specta / 不进 bindings ⇒ 绑定零再生成（AC-8） |

---

## 架构组件 (Architecture Components)

零新增组件 / 零新增文件（源码侧）：改动全部落在 `crates/core/archi` 的 `api` 子域内，闭包作为 `extract_element` 的**私有**实现细节（`Closure`）存在。

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 取数分派 + **TS 导出闭包**（**改**） | `extract_element` 按语言分派；TS 分支 = 入口派生闭包（递归 `Closure::collect` + `visited` + 同元素约束 + 去重 + 别名改名）；`ApiSymbol` 承载定义文件 | `crates/core/archi/src/api/extract/mod.rs` | `scan::ts::TsResolver`、`scan::owner::OwnerIndex`、`super::paths::{language_of, module_name}`、`rust::{extract_crate, extract_file}`、`ts::{extract_surface, extract_file}` | 纯 Rust + 读盘（入口与闭包内目标文件），零写盘、零 spawn |
| 单文件声明抽取 + **导出形态单一分派点**（**改**） | `extract_surface` 一次 AST 遍历产出三面（`symbols` / `named` / `star`）；`exported_item` 三态单点；`extract_file` 委托声明面（签名与产出语义逐字不变）；每符号填 `file` | `crates/core/archi/src/api/extract/ts.rs` | `oxc_parser` / `oxc_ast` / `oxc_span`（既有）；`super::{ApiSymbol, ApiParam, ApiType, ApiSymbolKind, Visibility}` | oxc AST 解析（零新依赖、零读盘） |
| Rust 声明抽取（**改**：只填 `file`） | `ApiSymbol.file` = 当前模块文件（贯穿 `walk_items` / `walk_modules`；内联 `mod` 与父同文件）；抽取面 / `visibility` 四值 / 签名归一 / `#[path]` 限根逐字不变 | `crates/core/archi/src/api/extract/rust.rs` | `syn`（既有）、`super::paths` | syn AST |
| 契约面装配（**改**：两处调用点 + 注记） | `element_symbols` / `bootstrap` 补 `elements` / `element`；`bootstrap_text` / `bootstrap_types` 的注记改 `symbol.file`；出口八行、`ApiSnapshot` 三面、`entry_state` 取数方式零变化 | `crates/core/archi/src/api/mod.rs` | `extract::extract_element`、`paths::entry_state`、`contract::is_constructor` | 纯函数 + 字符串拼装 |
| specifier 解析（**零改动 · 被消费**） | `resolve(&mut self, file_rel, specifier) -> Option<String>`：就近 tsconfig（`baseUrl` / `paths`）→ node 规则；失败 `None`；**无** `unresolved` 副作用 | `crates/core/archi/src/scan/ts.rs:156` | `oxc_resolver` | 只读缓存 |
| 归属单点（**零改动 · 被消费**） | `OwnerIndex::{build, owner_of}`：唯一最长前缀 → `Element{fqn, gate}`；同长多命中 → `Ambiguous`；无命中 → `None`（MUST NOT 任选） | `crates/core/archi/src/scan/owner.rs:74` / `:106` | `scan::is_source_file` | 纯函数 |
| 判据面（**零改动**） | 十条判据只消费 `ApiSnapshot.symbols`；导出集口径的变化发生在取数面，`check.rs` 逐字不动 | `crates/core/archi/src/api/check.rs` | `extract`（间接） | 纯函数 |
| 证据留痕（**新增 · 文档**） | 改动前后 `bootstrap` stdout 对照、`apiFindings` 分桶、`api:check` 三列与 `bindings:export` 幂等输出；§0 注明元素级 / 全量口径 | `openspec/changes/ts-export-closure-extraction/reports/api/closure-evidence.md` | `archi-api-gen` CLI、`pnpm` 脚本、门禁报告 | markdown |
| 版本面（**改**） | `version` 一次 patch | `packages/desktop/package.json`（`tauri.conf.json` 经引用跟随） | 既有 `"version": "../package.json"` | JSON |

### 闭包数据流（改后）

```
extract_element(root, elements, element, entry)
  ├─ Rust   → rust::extract_crate(root, entry)                 （逐字不变；file = 当前模块文件）
  ├─ TS     → Closure{root, OwnerIndex::build(elements), element, TsResolver::new(root), visited}.collect(entry, None)
  └─ 其余   → []

Closure::collect(file, names)
  1  key = (file, names 排序后)                     ← visited 键含名字集（R-3）
  2  if !visited.insert(key) → []                    ← 环防；终止性由它保证，零深度上限
  3  source = fs::read_to_string(root.join(file)) 或 → []
  4  parsed = ts::extract_surface(file, &source)     ← 单点分派三态
  5  out = parsed.symbols 按 names 过滤              ← names=None ⇒ 全部
  6  for (local, exported, specifier) in parsed.named:
        names 不允许 exported ⇒ skip
        target = resolve(file, specifier) 且 language_of == TypeScript ⇒ 否则 skip
        reexport_ok(element, target) ⇒ 否则 skip
        out += collect(target, Some([local]))，每符号 name := exported   ← 别名逐层改名
  7  for specifier in parsed.star:
        target = 同上（解析 + 语言 + 同元素约束）
        out += collect(target, None)                                  ← 通配取全量面
  8  out 按 (name, kind) 去重，保留首次出现 → 返回
```

`reexport_ok(element, target)`：

```text
match OwnerIndex::owner_of(target):
    Owner::Element { fqn, .. } => fqn == element.fqn || fqn.starts_with(format!("{}.", element.fqn))
    Owner::Ambiguous(_) | Owner::None => false
```

`visibility` 一律 `External`（可达即公开，与 Rust `pub use` 升格同语义）；本变更 MUST NOT 新增 `crateOnly` / `testOnly` / `unused` 的 TS 产出路径（三值留白逐字不变）。

---

## 变更清单 (Change Inventory)

### 新增文件

| 路径 | 产出相位 | 说明 |
|---|---|---|
| `openspec/changes/ts-export-closure-extraction/reports/api/closure-evidence.md` | 实现面 | 三份实测合一份，§0 声明口径：① **改动前**（落代码前采集）与**改动后** `cargo run -q --manifest-path packages/desktop/src-tauri/Cargo.toml -p archi-api-gen -- bootstrap App.Frontend.Components.Agent` 的 stdout 原文对照（前：零符号骨架；后：7 名 + `file:line` 注记）——**元素级口径**；② 一次**全量**检查的 `apiFindings` 分桶（应仍 12 / 29、总 41、`status = clean`、分母 44）+ 该读数的来源（门禁报告 / 移交相位）；③ `pnpm api:check` 三列空与 `bindings:export` 幂等（二跑零 diff）的命令输出 |

> 本变更新增文件只有这一份留痕文档，**无新增源码文件**（闭包与三面都落在既有三个 `extract/*.rs` 内）；故它不在下方「公共函数 / API」「类型定义」「配置」三张子表中出现——它们在留痕里以读数出现，见「数据模型 - 派生读数」。

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs` | ① `ApiSymbol` 增 `pub file: String`（字段位紧跟 `line`；doc：repo 相对 POSIX、TS 取抽取面形参、Rust 取所在模块文件、内联 `mod` 与父同文件）；② `pub fn extract_element` 签名扩为 `(root, elements, element, entry)`，TS 分支改为 `Closure::new(root, elements, element).collect(entry, None)`，Rust 分支逐字不变；③ 新增**私有** `struct Closure<'a>`（`root` / `owners` / `element` / `resolver` / `visited`）与 `fn collect(&mut self, file: &str, names: Option<&[String]>) -> Vec<ApiSymbol>`；④ 新增**私有**辅助 `reexport_ok` / `export_target`（resolve + `language_of` 过滤）/ `visited_key` / `dedupe` / `keeps`；⑤ 模块 doc 的「边界」句更正为「`extract` / `ts` / 归一纯函数零读盘；`extract_element`（与 `rust::extract_crate`）按入口与闭包内目标文件读盘，零写盘、零 spawn」，并在「两个取数入口」段补一句 TS 分支已走入口导出闭包 | AC-1/2/3/4/6 的实现本体；`extract`（D52）与 `extract_file` 的 doc 零语义变化（留白逐字保留） |
| `…/api/extract/ts.rs` | ① `exported_declaration` → `fn exported_item<'a, 'b>(statement: &'b Statement<'a>) -> ExportedItem<'a, 'b>`（三臂 + `_ => None`；`ExportAllDeclaration` 臂内 `exported.is_some()` ⇒ `None`）；② 新增 `pub struct NamedExport { local, exported, specifier }`、`pub struct ExportSurface { symbols, named, star }` 与**私有** `enum ExportedItem<'a, 'b> { Declaration(&'b Declaration<'a>), Named(Vec<NamedExport>), Star(String), None }`；③ 新增 `pub fn extract_surface(file: &str, source: &str) -> ExportSurface`（一次 `program.body` 遍历，`panicked` ⇒ 空三面）；④ `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol>` 改为 `extract_surface(file, source).symbols`（**首行签名逐字不变**）；⑤ `collect_declaration` / `extract_const` / `push_function` / `push_type` 各增 `file: &str` 形参并把 `ApiSymbol.file` 填成该形参；⑥ 模块 doc 把「`export {…}` / `export * from` 是透传（本期不产出操作）」改写为「透传由 `extract_surface` 的三面交给闭包；`extract_file` 对它们仍零符号」，并登记四类留白 | AC-7 的不变量面：`match statement` 恰 1 处、`Statement::` 恰 3 处（等于臂数）、`Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中——**doc 注释里 MUST NOT 写出这些字面量**（否则计数被注释污染） |
| `…/api/extract/rust.rs` | ① `walk_items(items, reachable, in_test, file: &str, out)`、`push_type(…, file: &str, out)`、`function_symbol(…, file: &str)` 各增「当前模块文件」形参并落 `ApiSymbol.file`；② `extract_file` 把入参 `file` 传下去（`let _ = (file, crate_name)` 改为 `let _ = crate_name`）；③ `extract_crate` 以 `entry` 作根文件；④ `walk_modules` 对文件型 `mod` 传 `&file`、对内联 `mod` 沿用父文件；⑤ doc 补一句 `file` 语义 | AC-6：抽取面 / `visibility` 四值 / 签名归一 / `#[path]` 限根逐字不变，diff 只允许出现在「多一个 file 形参 + 一个字段赋值」 |
| `…/api/mod.rs` | ① `element_symbols(root, elements, element)` 内调用改 `extract_element(root, elements, element, &entry)`；② `bootstrap` 内调用同改（两处的 `elements` / `element` 本已在手）；③ `bootstrap_text(fqn, symbols)`（**`entry` 形参退出**），注记改 `<!-- {visibility} @ {symbol.file}:{line} -->`；④ `bootstrap_types` 的 `待剪枝` 注记把字面量「入口」换成 `symbol.file`；⑤ 三处 doc（`bootstrap` 的「每个符号附 visibility 与定义位置注释」、`bootstrap_text`、`bootstrap_types`）改为「定义文件与行」 | AC-5；出口八行（`lib.rs` 逐名 `pub use`）、`ApiSnapshot` 三面、`entry_state` 取数方式、`bootstrap` 公开签名零变化 |
| `packages/desktop/package.json` | `"version"` `0.6.48` → `0.6.49`（只 +1 一次、不追溯）；其余键与 `scripts` 段零改动 | AC-9 / R-8：本变更改 `packages/desktop/src-tauri/**` 源码 ⇒ 属 desktop source change |

<!-- 测试文件子表：有意省略。proposal「变更范围 - 测试文件」列的五份 sidecar
     （api/extract/{mod,ts,rust}_test.rs、api/{mod,check}_test.rs）的用例本体、闭包语料与
     闸 2 改写归测试相位（见 tasks.md 末段「移交清单」）；本设计只钉产品侧不变量与判据语义。 -->

### 公共函数 / API

> 均为 `archi` crate 内**模块级导出**（经 `pub mod api` 可达，但不进 `lib.rs` 的逐名出口面——与 `ts::extract_file` / `apply_knip_report` 同族）。序列化面零变化：`ApiSymbol` 不进 `dto` / 不进 specta / 不进 bindings。

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `extract_element` | `api/extract/mod.rs` | 修改 | `pub fn extract_element(root: &Path, elements: &[ArchiElement], element: &ArchiElement, entry: &str) -> Vec<ApiSymbol>` | 判据「代码导出集」与 `bootstrap` 的唯一取数面；TS 分支 = 入口派生闭包，Rust 分支 = `rust::extract_crate` 逐字不变（`elements` / `element` 不参与，doc 写明理由：「`mod` 树自锚」）；入口读不到 / 语言未知 ⇒ 空 `Vec`（既有语义） |
| `extract_surface` | `api/extract/ts.rs` | 新增 | `pub fn extract_surface(file: &str, source: &str) -> ExportSurface` | 一次 AST 遍历产出三面（声明 / 具名名单 / 通配名单）；纯读源文本 + AST（零读盘、零写盘、零 spawn）；语法错 / `panicked` ⇒ 三面全空（不 panic、不猜） |
| `extract_file`（TS） | `api/extract/ts.rs` | **零签名 / 零产出语义** | `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol>` | 委托 `extract_surface(file, source).symbols`；只认 `export <声明>`，对 `export … from` / `export *` / `export default` 零符号（闸 1 / 闸 3 的判红面，**保留不改**） |
| `extract_file`（Rust） | `api/extract/rust.rs` | **零签名**（`file` 由占位改为落符号） | `pub fn extract_file(file: &str, source: &str, crate_name: &str) -> Vec<ApiSymbol>` | 单文件入口面（D52 预留）；`crate_name` 仍只作诊断留痕 |
| `extract_crate` | `api/extract/rust.rs` | **零签名** | `pub fn extract_crate(root: &Path, entry: &str) -> Vec<ApiSymbol>` | 整包面（`mod` 树自锚）；`ApiSymbol.file` 由同一遍历给出 ⇒ 与 `extract_element` 的 Rust 分支逐字段相等（AC-6） |
| `extract` | `api/extract/mod.rs` | **零变化**（显式留白 D52） | `pub fn extract(element: &ArchiElement, file: &str, source: &str) -> Vec<ApiSymbol>` | 单文件入口面；产品侧零调用者，本变更 MUST NOT 接线 |
| `apply_knip_report` | `api/extract/ts.rs` | **零变化** | `pub fn apply_knip_report(symbols: &mut [ApiSymbol], report: Option<&Value>)` | `unused` 的唯一降级来源（零产品调用者的预留接口，留白不动） |
| `bootstrap` | `api/mod.rs` | **零签名** | `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>` | 内部注记改按 `symbol.file` 输出；失败面（`ApiError::Element`）与文案素材（`entry_problem_reason`）零变化 |

私有签名变化（不入上表，列此以免实现期误以为可扩）：`api/mod.rs` 的 `fn bootstrap_text(fqn: &str, symbols: &[ApiSymbol]) -> String`（去 `entry`）；`api/extract/ts.rs` 的 `fn collect_declaration(declaration: &Declaration<'_>, source: &str, file: &str, out: &mut Vec<ApiSymbol>)`、`fn extract_const(declaration: &VariableDeclaration<'_>, source: &str, file: &str, out: &mut Vec<ApiSymbol>)`、`fn push_function(function: &Function<'_>, owner: Option<&str>, source: &str, file: &str, out: &mut Vec<ApiSymbol>)`、`fn push_type(name: String, kind: ApiSymbolKind, line: u32, file: &str, out: &mut Vec<ApiSymbol>)`；`api/extract/rust.rs` 的 `fn walk_items(items: &[syn::Item], reachable: bool, in_test: bool, file: &str, out: &mut Vec<ApiSymbol>)`、`fn push_type(vis, ident, span, kind, reachable, in_test, file: &str, out)`、`fn function_symbol(vis, sig, owner, span, reachable, in_test, file: &str) -> Option<ApiSymbol>`。

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `ApiSymbol` | `api/extract/mod.rs` | 修改 | 增 `pub file: String`（定义所在文件，repo 相对 POSIX；TS = 抽取面形参，Rust = 当前模块文件）。`PartialEq` / `Eq` / `Clone` 派生物随之外延（既有断言按新形状比对）；字段序外延 ⇒ 既有构造点与夹具须同批补值（移交清单） |
| `ExportSurface` | `api/extract/ts.rs` | 新增 | `pub struct ExportSurface { pub symbols: Vec<ApiSymbol>, pub named: Vec<NamedExport>, pub star: Vec<String> }`；`Debug + Clone`。三面即「声明 / 具名 re-export / 通配 re-export」的一次遍历产物 |
| `NamedExport` | `api/extract/ts.rs` | 新增 | `pub struct NamedExport { pub local: String, pub exported: String, pub specifier: String }`；`Debug + Clone`。`local` = 目标文件里的名字（取什么），`exported` = 本文件对外名（怎么叫），`specifier` = 原文字面量（交给 `TsResolver` 解析） |
| `ExportedItem<'a, 'b>` | `api/extract/ts.rs` | 新增（**私有**） | `enum ExportedItem<'a, 'b> { Declaration(&'b Declaration<'a>), Named(Vec<NamedExport>), Star(String), None }`；三态 + 显式 `None`，是「导出形态单一分派点」的产出类型（MUST NOT 扩出第五态） |
| `Closure<'a>` | `api/extract/mod.rs` | 新增（**私有**） | `struct Closure<'a> { root: &'a Path, owners: &'a OwnerIndex, element: &'a ArchiElement, resolver: TsResolver, visited: BTreeSet<(String, Option<Vec<String>>)> }`；承载递归状态（`visited` 键 = `(文件, 名字集排序后)`），一次 `extract_element` 一个实例（tsconfig 编译只发生一次） |

<!-- 配置：本变更零新增配置键。唯一变化是 version 值（见下），
     `.dev-team/config.json` / `.dev-team/architecture/config.json` /
     `crates/core/config/**`（含 config.schema.json 镜像）/ `KNOWN_KEYS` / `capabilities/default.json`
     全部零 diff。 -->

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.48` → **`0.6.49`** | 只 +1 一次（R-8：`0.6.48` 已被 `frontend-component-element-coverage` 用掉） |
| `version` | `packages/desktop/src-tauri/tauri.conf.json` | **零改动** | `string` | `"../package.json"` | 实现期只核对引用形态，`git diff` 必须为空 |
| `version`（crate） | `packages/desktop/src-tauri/Cargo.toml` | **零改动** | `string` | `0.1.0` | crate 版本线独立，MUST NOT 随 `package.json` |

---

## 数据模型 (Data Model)

### `ApiSymbol`（进程内中间形态，非持久化、非 IPC）

| 字段 | 类型 | 变化 | 语义 |
|---|---|---|---|
| `name` | `String` | — | 对外符号名（闭包内别名改名后的名字） |
| `owner` | `Option<String>` | — | 归属类型名（方法 / 关联函数；自由函数 `None`） |
| `kind` | `ApiSymbolKind` | — | 封闭四值 `Type` / `Trait` / `Enum` / `Function`（也是去重键的一半） |
| `params` | `Vec<ApiParam>` | — | 参数清单（`Function` 才有） |
| `returns` | `ApiType` | — | 返回类型（`Function` 才有意义） |
| `is_async` | `bool` | — | 异步标记 |
| `line` | `u32` | — | 定义所在行（1 起始） |
| `file` | `String` | **新增** | 定义所在文件（repo 相对 POSIX）。TS：闭包内每层 `extract_surface` 的 `file` 形参；Rust：`walk_items` / `walk_modules` 贯穿的当前模块文件。断言：MUST NOT 是入口串（Rust 子模块 `pub fn` 与 TS 一跳后符号都会因此指向错文件） |
| `visibility` | `Visibility` | — | 封闭四值；TS 闭包内一律 `External`，Rust 四值判据逐字不变 |

### 闭包算法的不变量（可机械核对）

| 不变量 | 内容 | 反向判据 |
|---|---|---|
| `visited` 键 | `(目标文件, 本次请求的名字集)`；名字集先排序再作键（同一集合的不同到达序不重复展开） | 只用文件作键 ⇒ 本仓 7 名变 **5** 名（`AgentSessionActions` / `AgentSessionViewProps` 被第二条语句吞掉，零报警，R-3） |
| 终止性 | 由 `visited` 保证：键集 ⊆ 文件集 × ({`None`} ∪ 各语句 `local` 的单元素集) ⇒ 有限 | 出现深度上限参数 ⇒ 多出「触顶不冒充完整面」的第二套语义（spec 显式禁止把部分面冒充完整面） |
| 去重键 | `(name, kind)`，保留首次出现 | 按结构去重 / 按名字无 kind 去重 ⇒ 同名字不同类型被合并 |
| 顺序 | 入口语句序（声明 → 具名 → 通配）+ 目标文件声明序 | 依赖哈希序 ⇒ 同输入两次调用不等（AC-3） |
| 同元素约束 | `Owner::Element{fqn}` 且 `fqn == element.fqn` 或 `fqn` 以 `"{element.fqn}."` 为前缀 | 裸 `starts_with(element.fqn)` ⇒ `…AgentFoo` 被误认；`Ambiguous` / `None` 放行 ⇒ 跨元素面被当公共面（R-9 的「虚增」误读） |
| 读盘/留痕边界 | 闭包读入口与目标文件；零写盘、零 spawn、零 `unresolved` 记录 | 出现 `unresolved` 写入 ⇒ 与 `scan` 的依赖边账本混账（R-5） |

### 持久化面变化

| 对象 | 变化 |
|---|---|
| 源码 | `api/extract/{mod,ts,rust}.rs` + `api/mod.rs`（见「修改文件」） |
| `.dev-team/architecture/models/**` | **零变化**（53 元素 / 120 关系不动） |
| `.dev-team/architecture/api/**` | **零变化**：`INDEX.toml` 仍 44 条、唯一契约资产 `Infra/App.Backend.Infra.ArchiRuntime.md` 与其 `.schema.json` 零 diff、无新增 `.i.rs` / `.i.ts` |
| `packages/desktop/src/types/generated/**` | **零变化**（`ApiSymbol` 不进 `dto` / 不进 specta） |
| `openspec/specs/**` | **零变化**（存活 spec 由归档链按两份 delta 合并，实现期 MUST NOT 手改） |
| `packages/desktop/package.json` | `version` 一行 |

### 派生读数（非持久化；落 `reports/api/closure-evidence.md`）

| 读数 | 期望值 | 取数面（D9） |
|---|---|---|
| `bootstrap App.Frontend.Components.Agent` 的符号名集合 | 改前 **空**；改后恰 **7** 名 `AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`（3 `Function` + 4 `Type`）；`AgentInputProps` / `AgentMessagesProps` / `isToolPart` 零命中 | **实现面 · 元素级口径**（`archi-api-gen bootstrap` stdout，零契约资产即可复算） |
| 注记行 | 逐符号 `@ {file}:{line}` 指向真实定义文件（`agent-input.tsx:152` / `agent-messages.tsx:33` / `session-panel.tsx:230` …）；**零** `@ packages/desktop/src/components/agent/index.tsx:<line>` 形态 | **实现面**（同上 stdout） |
| `apiFindings` | **12 / 29（总 41）**、分母 44、`status = clean`、`passed = true`；`apiUndeclared` 仍 `off` | **移交面**（门禁报告；`apiFindings` 与扫描模式无关，但取数通道是 runner / IPC）。MUST 注明「全量检查」口径 |
| `api:check` 三列 | `missing` / `mismatched` / `orphaned` 皆空 | **实现面**（`pnpm api:check`；`api:export` 二跑「0 份写入」等价） |
| `bindings:export` | 幂等（再导出零 diff） | **移交面**（`pnpm server:check && bindings:export` 由 static-check 承接；实现面以 `src/types/generated/**` 零 diff 作等价证据） |
| 源码不变量 | `ts.rs`：`match statement` = 1、`Statement::` = 3、两类留白字面量 = 0；`ApiSymbol` 构造点全部带 `file` | **实现面**（`grep -c`） |
| 版本 | `package.json` = `0.6.49`；`tauri.conf.json` 引用形态；`Cargo.toml` `0.1.0` | **实现面** |

---

<!-- 路由 / API 设计：本变更零 IPC 命令、零 CLI 子命令、零 HTTP 端点变化。
     `archi-api-gen` 的 generate / --check / bootstrap 三子命令入出参与退出码逐字不变
     （bootstrap 只是 stdout 内容变宽），`api:check` / `api:export` / check 四段脚本零变化，
     故本段省略。 -->

## 依赖 (Dependencies)

运行时依赖：**零新增**。`oxc_allocator` / `oxc_ast` / `oxc_parser` / `oxc_span`（`api/extract/ts.rs` 已在用）与 `oxc_resolver`（经 `scan::ts` 已在用）逐个已在 workspace 依赖表内；`Cargo.toml` / `Cargo.lock` / `package.json` 的 `dependencies` 段零 diff。

构建 / 测试依赖：**零新增**（本 crate 无 dev-dependency，既有测试用自建 `TempDir`）。

Crate 内部依赖（**新增一条，方向不变**）：

| 依赖 | 用途 | 依据 |
|---|---|---|
| `api::extract → scan::ts::TsResolver` | re-export specifier 解析（`baseUrl` / `paths` / 别名 / 扩展名 / index 折叠），零第二份解析 | 与 `api/diff.rs` 用 `scan::walk_source_files`、`api/paths.rs` 用 `scan::is_source_file` 同族；`resolve` 无 `unresolved` 副作用（`scan/ts.rs:156`） |
| `api::extract → scan::owner::{Owner, OwnerIndex}` | 同元素约束的归属判定（三值不任选） | 归属单点在 `scan`，`api` 只读结果 |

---

## 验收标准回应

| AC | 设计落点 | 承接阶段 |
|---|---|---|
| **AC-1** 本仓语料恰 7 名 | D1（算法 + 具名名单精确 + visited 键含名字集）、事实核实表「判据语料（正面）」逐条列出 6 条语句与 7 名、D8（只数名字不验签名）；不达时的排查序：先查 `entry_state` 是否 `Resolved`、再查 `owner_of` 是否命中 4 个子元素，MUST NOT 放宽同元素约束凑数 | 阶段三（闭包）+ 阶段五（留痕 §1） |
| **AC-2** 精确名单与通配等价 | D1 的第 6/7 步（具名只取 `local`、通配取全量面）+ D2（`extract_surface` 的 `named` / `star` 两面如实）；反向（只 `export { A }` ⇒ 1 个符号） | 阶段二（三面）+ 测试相位语料 |
| **AC-3** 环与菱形确定收敛 | D1 的 `visited` 键与去重键；不变量表「终止性 / 去重键 / 顺序」三行 | 阶段三 + 测试相位语料 |
| **AC-4** 同元素约束按 owner 子树裁 | D4（`owner_of` 单点）+ 数据流里的 `reexport_ok` 三值判据（含 `"{fqn}."` 分段比较）+ D5（仓外 / 不可解析静默跳过） | 阶段三 + 测试相位语料 |
| **AC-5** 注记行指向真实定义文件 | D3（`file` 语义语言无关）+ D6（两处注记改 `symbol.file`）+ 事实核实表「定义位置」的逐符号期望值 | 阶段一（`file`）+ 阶段四（注记） |
| **AC-6** 既有 Rust 取数面零变化 | D3 的「同一遍历给出 `file`」⇒ `extract_element` 的 Rust 分支与 `extract_crate` 逐字段相等（含 `file`）；`extract_crate` / `extract_file` 签名与抽取面逐字不变；两条 else 语义（入口读不到 / 语言未知）不触碰 | 阶段一 / 阶段三 |
| **AC-7** 三闸全绿且单点分派在案 | D2（三臂 + `_` 收口、首行签名逐字、`export * as ns` 臂内退回 `None`）+ 修改文件表里「doc MUST NOT 写出两类留白字面量」的注解；闸 1 / 闸 3 零改动 | 阶段二（产品侧）+ 测试相位（断言改写） |
| **AC-8** 零扩散且基线不换段 | D10 白名单 + D7⑤⑥（不接线 D52 / 不做 `.i.ts` 试点）+ D9（`apiFindings` 读数口径与来源） | 阶段五（`git status` / `api:check` / 留痕 §2 §3） |
| **AC-9** 版本纪律 | 配置表三行（`0.6.49` / 引用形态零改动 / `Cargo.toml` 零改动） | 阶段五 |
| **AC-10** 相关能力零漂移 | D10（`openspec/specs/**` 零 diff；两份 delta 已在案、实现期不编辑）+ 「不要修改」面在任务里逐项 `git diff` 核对 | 阶段五 |

> **不列为 AC 的流程项**（沿 proposal 末尾声明）：十值判据词汇、互斥求值序、纳管子集、`REPORT_VERSION`、`check` 链四段、生成器三子命令——各有既有 spec 落点且本变更零改动。
> **测试面**：五份 sidecar 的用例本体、闭包语料、闸 2 改写、夹具补 `file` 与 `extract_element` 调用点适配归测试相位（`tasks.md` 末段逐项列出）。静态门禁（`api:check` / `server:check` / `client:check`）由桌面端 static-check / test-execution 承接，本相位不重复执行机械步骤。

---

## 待决问题 (Open Questions)

1. **通配递归是否承接外层名字集**：spec 与 proposal 都把 `export * from './x'` 写成「取 `./x` 的全量面」，即 `collect(target, None)`——因此「外层是具名语句、目标是再一层 barrel（`export * from …`）」这一组合下，被请求的单名不会过滤掉兄弟名（理论上的唯一精度缺口）。本仓无该形态实例，AC 也不覆盖；按「实现跟随显式算法」取 `None`。若将来出现该形态的精确判据，改为传 `names` 是一次**局部决策**（键语义已含名字集，无需改算法），但须另开 change 并同批补 AC——**MUST NOT** 在本变更内顺手改。
2. **`apiFindings` 分桶的取数相位**：取数通道只属门禁 runner / IPC，实现面不持有（D9）。若实现相位跑完仍无门禁报告，留痕 §2 只能写「待 test-execution 回填」——不勾为任务、MUST NOT 手写伪读数。承接方与时点在本设计内不指定（沿 `frontend-component-element-coverage` D11 的先例）。
3. **`file` 字段的路径形态在符号改名后的稳定性**：别名改名（`export { A as B }`）只改 `name`、不改 `file` / `line`，本变更已按此定形；若将来出现「改名后需要回指目标文件内原始名字」的需求（例如 `bootstrap` 想加 `原名` 注记），那是一次 `NamedExport` 能力扩张，不在本变更射程。
