# 提案: ts-export-closure-extraction

> **变更**: ts-export-closure-extraction（TS 取数面沿 `export … from` 走 · 导出闭包）
> **日期**: 2026-11-17
> **状态**: draft
> **来源**: `openspec/changes/ts-export-closure-extraction/explore.md`（requirement 类型；C.1–C.9 全篇，含两轮 re-explore 与裁定）
> **性质**: **取数面能力补齐**——`api::extract` 的 TS 分支由「入口单文件」扩为「入口导出闭包」（`export { … } from` / `export * from` / 环防 / 同元素约束），`ts.rs` 的导出形态分派扩成三态但仍单点，`ApiSymbol` 增 `file`（定义文件）；两份 spec 落 **3 条 delta（1 ADDED + 2 MODIFIED）**，其中 1 条 MODIFIED 是 barrel 实测叙述的事实校准。零模型数据改动、零契约资产改动、零生成物 diff、零新增判据。
> **不含**: `.i.ts` 试点与 `ts_import_specifier` 接线（提案六）、`export { localA }`（无 `from`）与 `export * as ns` 与 `export default`（提案五留白）、任何跨元素 re-export 语义、任何 `apiFindings` 基线换段。

---

## 问题

### 现状链路

```text
extract_element  api/extract/mod.rs:208
  └─ TypeScript → fs::read_to_string(root.join(entry)) → ts::extract_file(entry, source)
        ts::extract_file  api/extract/ts.rs:49
          └─ exported_declaration(statement)   api/extract/ts.rs:66
               只认 Statement::ExportDeclaration → Declaration::{Function, Class, Variable,
               TSTypeAlias, TSInterface, TSEnum}
               **零 export … from / export * from 分支**
```

模块 doc 自己写着这一留白：`export {…}` / `export * from` 是**透传**（本期不产出操作）。留白与判据的耦合是隐性的——`extract_element` 同时是判据「代码导出集」与 `bootstrap` 草稿的**唯一**取数面。

### 病灶一 · 对 barrel 恒零产出 ⇒ 前端契约一写就红

barrel 的内容天然全是 re-export。取数器对此零产出 ⇒ 元素导出集恒空 ⇒ 任何前端契约只要声明一条 op / 类型，`apiMissing`（`warn`）恒报「代码导出集里没有实现（代码导出集：空）」。于是「统一 i + 前端契约」退化成**一写就红**，实际等于前端继续 `api = none`——补 barrel 的收益被取数面吃掉。

### 病灶二 · `bootstrap` 对前端元素恒空骨架

`bootstrap` 经同一 `extract_element`：`# <FQN>` + 空散文段 + 零 `### <op>` + 空 `api-types`。前端元素的草稿无人可用，起草这件事对前端等于不存在。

### 病灶三 · `ApiSymbol` 无「定义文件」事实 ⇒ 注释必然说谎

`ApiSymbol.line` 只有行号。闭包一跳之后，行号属于**另一个文件**，而 `bootstrap` 的 `<!-- <visibility> @ <entry>:<line> -->` 仍写入口串——指错文件，且这种注释比没有注释更坏（它长得像证据）。Rust 侧今天同样吃这一口：`pub fn` 写在 `src/op.rs`、注记却写 `src/lib.rs`。

### 病灶四 · 同一件事实的两半，schema 侧已做、代码侧缺席

契约模型里「透传」早有位置（`api-reexport` fence + `$defs` 纯 `$ref` alias）；代码侧的 re-export 取数缺席。两者是同一件事实的两半，缺一侧就无法对齐。

### 现状实测（2026-11-17 复核，逐条在案）

| 事实 | 证据 |
|---|---|
| 前置账全清 | A（`2026-11-16-module-entry-field-spec`）/ A 的数据面（`2026-11-16-module-entry-data-backfill`）/ 前端组件元素覆盖（`2026-11-17-frontend-component-element-coverage`，0.6.48）三次变更已归档；模型 53 元素 / 120 关系、`apiFindings` 12 / 29（总 41）、分母 44 |
| 判据语料到位 | `App.Frontend.Components.Agent` 元素在案（`path = ./packages/desktop/src/components/agent/index.tsx`），7 个导出名各自落在其子树内的元素上 |
| `TsResolver` 可复用 | `scan/ts.rs:156` 的 `resolve(&mut self, file_rel, specifier)` 只读缓存、只返回 `Option<String>`，**无 `unresolved` 副作用**（`unresolved` 由依赖边抽取的返回值承载）⇒ explore C.4 第三条关闭 |
| `api → scan` 方向已在 | `api/diff.rs` 用 `walk_source_files`、`api/paths.rs` 用 `is_source_file`；`scan::ts` 是首次但方向不新 |
| 唯一硬阻塞 | `grep -c "ExportFromDeclaration\|ExportAllDeclaration"` 于 `api/extract/ts.rs` = **0** |

---

## 提案

### 一 · 口径裁定：同元素约束 = owner 子树（explore C.6.5 / C.6.9，用户已选定）

```text
reexport_ok(element, target):
    match OwnerIndex::build(elements).owner_of(target):
        Owner::Element { fqn, .. } => fqn == element.fqn || fqn.starts_with(&format!("{}.", element.fqn))
        Owner::Ambiguous(_)        => false        // MUST NOT 任选（与既有口径一致）
        Owner::None                => false        // 无元素认领 ⇒ 不是本元素公共面
```

三条理由（缺一条就会退化成近似）：

1. **判据语料逼出来的**：`Components.Agent` 的 `path` 是**文件型**单值（`index.tsx`），而 `within_path` 对文件型条目只认精确相等 ⇒ 「目标须落在 `path` 前缀内」会让 7 个 re-export 目标全部越界、闭包恒空、判据 1 恒失败。这不是实现细节，是原草案与判据互相矛盾。
2. **与目录型 `path` 前缀结论一致**：`Views.*` 的目录根本就放行子目录文件，owner 子树不产生第二条规则分叉。语义上「父元素的公共面 ⊇ 可达子元素面」正是 barrel 的存在理由，MUST NOT 被读成「虚增」。
3. **FQN 按 `.` 分段比较**：MUST NOT 裸 `starts_with(fqn)`——否则 `…Components.Agent` 会误认 `…Components.AgentFoo`。

`Owner::None` 不放行：`session-panel.tsx` 曾落 `None`，处置是**补模型**（已归档的微型变更），不是给取数器开例外。`@/lib/**` / `@/components/ui/**` 归其它元素或无元素，一律拒绝。

### 二 · 取数面算法（入口导出闭包）

```text
extract_element(root, elements, element, entry):
  Rust   → rust::extract_crate(root, entry)                      // 逐字不变（mod 树自锚）
  TS     → surface(entry, names = None, visited = ∅)              // 入口是唯一全量起点
  其余   → 空

surface(file, names, visited):
  if visited.contains((file, names)) → []                        // 环防：键 = (文件, 名字集)
  visited.insert((file, names))
  parsed = ts::extract_surface(file, source)                      // 纯读源文本 + AST
  out = parsed.symbols 按 names 过滤                              // names=None ⇒ 全部
  for (local, exported, spec) in parsed.named:
      if names 允许 exported:
          target = resolver.resolve(file, spec)  或 skip
          if !reexport_ok(element, target): continue               // 同元素约束
          out += surface(target, Some({local}), visited) |> 改名 local → exported
  for spec in parsed.star:
      target = resolver.resolve(file, spec)      或 skip
      if !reexport_ok(element, target): continue
      out += surface(target, None, visited)
  按 (name, kind) 去重（保留首次出现；入口语句序 + 声明序 = 稳定序）
  每符号 visibility = External（可达即公开，与 Rust `pub use` 升格同语义）
```

- **visited 键必须含名字集**：`export { X } from './a'` 与 `export type { Y, Z } from './a'` 是同一目标的两条语句，只以文件为键会互相吞掉（本仓 `agent-session-view` 正是这两条，会丢 2 个名字）。
- **终止性由 visited 保证**，MUST NOT 依赖深度上限；`a → b → a` 与菱形（同一类型经两条路径到达）都靠它 + `(name, kind)` 去重收敛。
- **specifier 解析复用 `scan::ts::TsResolver::resolve`**（tsconfig `baseUrl` / `paths` / 别名 / 扩展名 / index 折叠），MUST NOT 造第二份——与仓库「零第二份解析」纪律同族。`resolve` 需要 `&mut self`，闭包内带一个实例（每次 `extract_element` 一个，缓存 tsconfig 编译）。
- **解析失败静默跳过**（不在仓内 / 不在盘 / 不可解析）：取数器非权威，缺口由契约面判据显示，MUST NOT panic、MUST NOT 记 `unresolved`（那是 `scan` 依赖边的账）。
- **入口读不到 ⇒ 空集**（既有语义逐字不变）。

### 三 · `ts.rs` 的单点分派：三态枚举（explore C.9.2）

`ts.rs` 今天只有一处 `match statement`（`exported_declaration`）。拿到 re-export 名单的直觉写法是**新开一个 `match statement`**——那会破坏「导出形态单一收口」这条既有不变量。正确形态是把这一处扩成三态：

```rust
enum ExportedItem<'a, 'b> {
    Declaration(&'b Declaration<'a>),           // export function / class / const / type / interface / enum
    Named(Vec<NamedExport>),                    // export { A, B as C } from './x' → (local, exported, specifier)
    Star(String),                               // export * from './y'
    None,                                       // 其余（含 export * as ns / export { localA } / export default）
}
```

`extract_file(file, source)` 的**签名与产出语义零变化**（= `extract_surface(...).symbols`），新增 `extract_surface` 承载三面。源码自证断言随之从「`Statement::` 恰 1 处」改为「`match statement` 恰 1 处 + 臂集合恰三值 + 两类留白零命中」（见验收 AC-7 与风险 R-1）。

### 四 · 签名扩展与 `ApiSymbol.file`

```rust
pub fn extract_element(root: &Path, elements: &[ArchiElement], element: &ArchiElement, entry: &str) -> Vec<ApiSymbol>
```

与 `paths::entry_state(root, elements, element)` 同形；两处产品调用点（`api/mod.rs` 的 `element_symbols` 与 `bootstrap`）本已持有 `elements` + `element`，零新取数。Rust 分支不使用这两个参数（`mod` 树自锚），doc 写明理由。

`ApiSymbol` 增 `pub file: String`（repo 相对 POSIX，定义所在文件）。它是**进程内类型**（不在 `dto` / 不进 specta / 不进 bindings），线面零成本；受影响的是构造点与测试夹具。`bootstrap` 的注记改为 `<!-- <visibility> @ <symbol.file>:<line> -->`——Rust 侧同样受益（子模块里的 `pub fn` 不再谎报入口）：

| 语言 | `file` 的来源 |
|---|---|
| TS | `extract_file` / `extract_surface` 的 `file` 形参（今天只喂 `source_type_of`，现一并落到符号上） |
| Rust | 贯穿 `walk_items` / `walk_modules` 的**当前模块文件**（内联 `mod` 与父同文件） |

`extract(element, file, source)`（单文件入口面，D52）保持**零产品调用者**：本变更走元素级递归，MUST NOT 为「凑齐形态」接线它。

### 五 · 明确不做（留白清单，写进 spec 而非口头约定）

| 形态 | 处置 |
|---|---|
| `export { localA }`（无 `from`，导出本文件非导出声明） | 不产出：需要「按名字取回本文件声明」的第二遍，与「取数面 = 导出面」的单遍纪律冲突；`Statement::ExportNamedDeclaration` 零命中，由源码断言钉住 |
| `export * as ns from './x'` | 跳过：产出命名空间对象而非符号，不猜 |
| 跨元素 / 仓外 re-export | 静默跳过，零 finding（取数器非权威；缺口由契约面判据显示，MUST NOT 造新判据） |
| `export default` | 不抽（既有纪律：对外名字由导入方决定） |
| 前端行为契约 | 组件入参是解构 props + 内联类型字面量 ⇒ 抽出的参数类型恒 `Unknown`；本能力只承诺「类型面 + 名面可用」，`api-op` 面在前端继续留白 |

### 六 · 不做 `.i.ts` 试点与 `ts_import_specifier` 接线（裁 explore C.7.4 #4 / C.5.1 #5）

explore 把这条留给起草时裁，结论是**不在本变更做**，四条理由按判据强度排序：

1. **试点今天只能产出坏契约**：`AgentInput({ ... }: AgentInputProps)` 是解构 + 内联类型字面量 ⇒ 取数器给 `params = [{name: "arg", ty: Unknown("")}]`、`returns = Named("Element")`。契约要么照抄 `?`（元素进纳管子集 ⇒ `apiSignatureMismatch` 立刻 warn ⇒ `status = warnings`），要么手写与代码不同形的签名（同样 mismatch）。**op 面可表达之前，试点没有收益，只有红灯。**
2. **它会改判据基线**：新增契约资产使该元素从「declared 但契约缺失」的 `apiMissing` 桶里退出（41 → 40）、`INDEX.toml` 44 → 45，并把「前端契约长什么样」这个能力决策偷渡进取数面变更。
3. **`ts_import_specifier` 只在契约含 `api-reexport` 时被消费**：无试点即零实例，接线后仍只有合成断言——把「无观测点的接线」写进本变更，只会让它下次被同理由再改一遍。
4. **本变更的判据不需要它**：AC-1 的证据是 `archi-api-gen bootstrap <元素>` 的 stdout（零契约资产即可复算）。

⇒ `ts_import_specifier` 与 `.i.ts` 首个真实实例移交**首个前端契约变更**（barrel 落地 B 或其后的专项），现行「待接线留白」条款逐字保留、MUST NOT 被写成已实现。若下游裁决要求 C 内出实例，须另开 change（本提案 R-4）。

### 七 · 版本纪律

`packages/desktop/package.json` `version` 由 `0.6.48` +1 patch 至 **`0.6.49`**；`tauri.conf.json` 经 `"version": "../package.json"` 跟随（文件零内容改动）；`src-tauri/Cargo.toml` 的 `[package] version`（现值 `0.1.0`）**MUST NOT** 随动。判据：本变更改 `packages/desktop/src-tauri/**` 源码 ⇒ 属 AGENT.md 的「desktop source change」，+1 patch 无例外。

---

## 能力

### 新增能力

**无。** 导出闭包不是新能力面：`desktop-archi-api` 早已定义「取数器是判据与 bootstrap 的唯一取数面」「TS 侧走 AST 抽导出」，本变更是把 TS 分支的取数范围补到这条判据本来要求的宽度（「一个元素的公开面散在入口与它声明的各文件里，只看入口会漏判」——该论述原就写在 `extract_element` doc 与 Rust 分支）。

### 修改的能力

- **desktop-archi-api** — 1 条 **ADDED**（`TS 导出闭包的取数面`：闭包定义 / 环防键 / owner 子树 / 解析单点 / `ApiSymbol.file` / 三态单点分派 / 四类留白 / 前端预期边界）+ 1 条 **MODIFIED**（`模块嵌套与选择性透传`：只校准 `App.Frontend.Components.Agent` 的实测叙述——子元素 6 → 7，删掉「自有导出 `SessionPanel`（其文件不是元素）」这条过期判断，改为「barrel 重导出 4 个子元素、3 个不导出」，并把「父契约三类混合」scenario 改写为「逐项声明而不自动推导」（该元素现态已无自有导出）；「父 → 子单向 / MUST NOT 自动推导 / 六条结构规则」逐字不动）+ Module Contract 六行更新。其余 requirement 逐字不动（含「生成器、verify 闭环与 bootstrap 剪枝」的 visibility 四值、D52 留白与试点纪律）。
- **desktop-archi-api-check** — 1 条 **MODIFIED**（`十条判据的定义与求值互斥顺序`）：只改「导出集口径」一段——TS 侧由「跨元素被 import 的 `export function`」改为「经入口导出闭包可达的导出」，并新增一条 scenario 钉住「闭包只减不增 `apiMissing`、`apiUndeclared` 仍 `off`」。判据十值、互斥序、判据级档位、纳管口径、报告形状、基线 12 / 29（总 41）逐字不动。

| 不涉及 | 理由 |
|---|---|
| `desktop-archi-model` | 模型数据与字段面零改动；它只说「`scan/rust.rs` / `scan/ts.rs` 是既有取数器」，本变更不动 `scan` |
| `repo-archi-model` | 元素 / 关系 / 计数（53 / 120、30 / 71、`71 = 64 + 7`）零改动 |
| `desktop-archi-drift` | 报告结构、四组词汇、`status` 推导、纳管子集、`REPORT_VERSION = 3` 零改动；`apiFindings` 读数不变（见 AC-8） |
| `desktop-archi-file-tree` | `OwnerIndex` 三候选与 `module_roots` 零改动（本变更只**读** `owner_of`） |
| `desktop-archi-contract-panel` / `desktop-archi-gate` | 消费面按同一份报告工作，判据条目数与档位不变 |
| `desktop-ipc-type-bindings` | `ApiSymbol` 不进 `dto` / specta ⇒ 类型绑定零再生成 |
| `desktop-archi-agent` | agent 侧知识文本（`api` / `entry` 字段面）零改动 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs` | `ApiSymbol` 增 `file: String`；`extract_element(root, elements, element, entry)` 签名扩展；TS 分支实现导出闭包（`surface` 递归 + visited(文件, 名字集) + `TsResolver` + owner 子树 + `(name, kind)` 去重 + 别名改名）；模块 doc 的「边界」一句按真实形态更正（`extract` / `ts` / 归一纯函数零读盘；`extract_element` 与 `rust::extract_crate` 按入口读盘、零写盘、零 spawn） |
| `…/api/extract/ts.rs` | `exported_declaration` 扩为三态 `exported_item`（单点分派）；新增 `extract_surface`（声明 + 具名名单 + 通配名单）供闭包消费；`extract_file` 签名与产出语义逐字不变；符号补 `file` |
| `…/api/extract/rust.rs` | `ApiSymbol.file` 填值：`walk_items` / `walk_modules` 贯穿当前模块文件；抽取面 / `visibility` / 签名归一逐字不变 |
| `…/api/mod.rs` | `element_symbols` 与 `bootstrap` 两处调用点补 `elements` / `element`；`bootstrap_text` 注记改 `symbol.file`（`entry` 退出注记面）；出口八行、`ApiSnapshot` 三面零变化 |
| `packages/desktop/package.json` | `version` `0.6.48` → `0.6.49` |

### 测试文件

| 对象 | 判断 |
|---|---|
| `…/api/extract/mod_test.rs` | `extract_element` 调用点适配（补 `elements` / `element`）；`symbol()` 夹具补 `file`；新增闭包语料：精确名单 / 通配等价 / 环（`a→b→a`）/ 菱形去重 / owner 子树拒绝（`Owner::None` 与跨元素）/ `Owner::Ambiguous` 拒绝 / 别名 `export { A as B }` / 两次调用逐字相等 |
| `…/api/extract/ts_test.rs` | 闸 1（签名逐字）与闸 3（`extract_file的透传不产符号`）**保留不改**；闸 2 由「`Statement::` 恰 1 处」改为「`match statement` 恰 1 处 + 臂集合恰三值 + `Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中」；新增 `extract_surface` 三面断言（留白形态 `symbols` 为零、`named` / `star` 如实） |
| `…/api/extract/rust_test.rs` | 若既有整体相等断言受 `file` 影响则同步；新增「子模块 `pub fn` 的 `file` = 子模块文件、内联 `mod` 的 `file` = 父文件」 |
| `…/api/mod_test.rs` | `bootstrap注记行取解析链入口` 的判据说明改为「注记取符号定义文件（定义在入口内时与入口同值）」，断言值不变；新增「符号定义在子模块 ⇒ 注记指向子模块文件」；`bootstrap草稿形态` 与 `extract_element与bootstrap同源` 随签名适配 |
| `…/api/check_test.rs` | `function()` / `type_symbol()` 夹具补 `file`；判据语义零改动（断言值不变） |
| 验收留痕（`reports/`） | 三份实测：① 改动前后 `archi-api-gen bootstrap App.Frontend.Components.Agent` 的 stdout 对照（前：零符号；后：7 名 + `file:line` 注记）；② 一次全量检查的 `apiFindings` 分桶（应仍 12 / 29、总 41、`status = clean`）；③ `api:check` 三列空 + `bindings:export` 幂等的命令输出。**MUST 注明所用的元素级 / 全量口径**，避免后人把读数差异当回归 |

> **语料纪律**：本仓模型 / 契约 MUST NOT 进 crate 语料（`desktop-archi-api-check` 既有条款）。AC-1 的「7 个名字」是**仓库实测留痕**（落 `reports/`），不写成 crate 断言常量。

### 不要修改

| 对象 | 理由 |
|---|---|
| `crates/core/archi/src/scan/ts.rs` / `scan/owner.rs` | 被复用：`TsResolver::resolve`（无 `unresolved` 副作用）与 `OwnerIndex::owner_of` 原样调用；出现 diff 即说明造了第二份解析 / 归属 |
| `crates/core/archi/src/api/paths.rs` | `entry_state` 单点、`within_path`、`ts_import_specifier`（保持待接线留白）逐字不动 |
| `crates/core/archi/src/api/emit/**` / `diff.rs` / `schema.rs` / `contract.rs` / `check.rs` | 渲染、三类差异比对、`$defs` 规则、装载期闸、十条判据实现零改动（导出集口径的变化发生在取数面，不在判据） |
| `.dev-team/architecture/**`（models / decisions / api / `INDEX.toml`） | 零数据改动、零契约资产改动；`api:check` 三列非空即误动 |
| `packages/desktop/src/types/generated/**` | `ApiSymbol` 不进 `dto`；绑定零再生成（AC-8 机械核对） |
| `ts_test.rs` 闸 1 / 闸 3 的断言 | 闸 1 钉住 `extract_file` 签名、闸 3 钉住「`extract_file` 对透传零符号」——它们守的正是「闭包在外层、`ts.rs` 零读盘」这条边界，删即失去唯一报警点 |
| `extract(element, file, source)`（D52） | 保持零产品调用者；本变更 MUST NOT 触发接线 |

---

## 验收标准

1. **AC-1 本仓语料恰 7 名（闭包的正面判据）**：`cargo run -q --manifest-path packages/desktop/src-tauri/Cargo.toml -p archi-api-gen -- bootstrap App.Frontend.Components.Agent`（在仓库根执行，根锚由 `repository_root()` 向上找 `archi_dir`） 产出**非空**草稿，符号名集合逐字等于 `AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`（3 个 op + 4 个类型）；`AgentInputProps` / `AgentMessagesProps` / `isToolPart` **零命中**（具名名单纪律的判据）；草稿落 `reports/` 留痕。反向判据：改动前同一命令产出零符号——若改动后仍为零，先查 `entry_state` 是否 `Resolved` 与 owner 子树是否命中，MUST NOT 靠放宽同元素约束凑数。
2. **AC-2 精确名单与通配等价**：对同一簇文件（目标 `./x` 导出 `A` / `B` / `C`）分别以「逐名 re-export 三条语句」与「一条 `export * from './x'`」构造入口，`extract_element` 的产出**逐字段相等**（名字集与顺序均同）；反向判据：入口只写 `export { A } from './x'` ⇒ 产出恰 1 个符号，`B` / `C` **零命中**（精确名单不虚增）。
3. **AC-3 环与菱形确定收敛**：`a → b → a` 的环不 panic、不重复、不无限递归；菱形（同一类型经两条路径到达）按 `(name, kind)` 去重；同一输入连续两次调用产出**逐字相等**（稳定序，不依赖哈希序）。
4. **AC-4 同元素约束按 owner 子树裁**：合成夹具中指向 `@/lib/**`（`Owner::None`）、指向另一元素（FQN 不在子树）、指向 `Owner::Ambiguous` 的 re-export 三条**全不入集**；本仓 `components/agent/index.tsx` 的 6 条 re-export 全部落在 `Components.Agent` 子树（故 AC-1 的 7 名成立）；`…AgentFoo` 形态不因裸前缀误认（`…Agent.` 前缀比较）。反向判据：`components/ui/resizable.tsx:42` 的 `export { usePanelRef } from 'react-resizable-panels'` 属仓外目标 ⇒ 解析失败即静默跳过，零 finding、零 panic。
5. **AC-5 注记行指向真实定义文件**：草稿里 `AgentInput` 的注记为 `@ packages/desktop/src/components/agent/agent-input.tsx:152`、`SessionPanel` 为 `session-panel.tsx:230`、`AskAnswerActions` 为 `agent-messages.tsx:33` 等（逐符号 `file:line` 指向真实定义处），MUST NOT 出现 `@ packages/desktop/src/components/agent/index.tsx:<line>` 的入口串冒充；Rust 侧新增用例证明子模块符号的注记指向子模块文件。
6. **AC-6 既有 Rust 取数面零变化**：`rust_test` / `mod_test` 的 `extract_crate` 语料逐字不动；`extract_element` 的 Rust 分支与 `extract_crate` 仍**逐字段相等**（`file` 一并相等 ⇒ Rust 侧 `file` 由同一遍历给出，零第二套）；`extract_element入口读不到返回空集` / `语言未知返回空集` 两条例外语义不变。
7. **AC-7 三闸全绿且单点分派不变量在案**：闸 1「`pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {` 首行逐字」通过（**零改动**）；闸 3「`export { a, b } from` / `export * from` / `export type { T } from` ⇒ `extract_file` 零符号」通过（零改动）；闸 2 改为 `match statement` 恰 1 处、`Statement::` 出现次数等于臂数 3、`Statement::ExportNamedDeclaration` 与 `Statement::ExportDefaultDeclaration` 零命中——MUST NOT 以删除断言换绿。
8. **AC-8 零扩散且基线不换段**：`git status --porcelain` 的改动集 ⊆ {本 change 目录、`extract/{mod,ts,rust}.rs`、`api/mod.rs`、五份 `*_test.rs` sidecar、`packages/desktop/package.json`}；`.dev-team/architecture/**` 零 diff；`pnpm api:check` 的 `missing` / `mismatched` / `orphaned` 三列皆空；`bindings:export` 幂等（再导出零 diff）；本仓一次全量检查 `apiFindings` 仍 **12 / 29（总 41）**、`status = clean`、`passed = true`——闭包扩容只可能**减少** `apiMissing`（本仓无前端契约在案 ⇒ 读数不变），`apiUndeclared` 仍 `off` ⇒ 不新增任何条目。
9. **AC-9 版本纪律**：`packages/desktop/package.json` `version` = `0.6.49`（相对落地时 `HEAD` 恰好 +1 patch，只加一次）；`tauri.conf.json` 仍为 `"../package.json"`；`packages/desktop/src-tauri/Cargo.toml` 的 `[package] version` 逐字不变。
10. **AC-10 相关能力零漂移**：`openspec/specs/` 下除本 change 落 delta 的两个能力（`desktop-archi-api` / `desktop-archi-api-check`）外零 diff；`desktop-archi-model` / `repo-archi-model` / `desktop-archi-drift` / `desktop-archi-file-tree` / `desktop-archi-contract-panel` / `desktop-archi-gate` / `desktop-archi-agent` / `desktop-ipc-type-bindings` 出现 diff 即误改。

> **不列为 AC 的流程项**：判据十值词汇、互斥求值序、纳管子集、`REPORT_VERSION`、`check` 链四段、生成器三子命令——各有既有 spec 落点且本变更零改动，不另设无 spec 落点的断言。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **闸 2 的「零改动」说法不成立**：explore C.9.3 写「扩同一处为三态 ⇒ 断言零改动」，但该断言数的是 `Statement::` 的**文本出现次数**（今天 1），三态后必然变 3 | 明写处置：断言随本批改为「`match statement` 恰 1 处 + 臂数 3 + 两类留白零命中」，不变量从「计数」升级为「单点 + 臂集合」。MUST NOT 删除断言、MUST NOT 用注释绕过（AC-7） |
| R-2 | **`ApiSymbol.file` 的范围被 explore 低估**：C.2.5 只点「三处 push」，但 Rust 侧要拿到真实文件须把当前模块文件贯穿 `walk_items` / `walk_modules` | 明写在变更范围与 AC-6；替代方案（Rust 侧填入口串）被否——那正是本病灶要修的「注释说谎」，且会让 `file` 语义按语言分叉 |
| R-3 | **visited 键写错会静默丢名**：只以文件为键时，同一目标的两条具名语句（本仓 `agent-session-view` 的 `{AgentSessionView}` 与 `{AgentSessionActions, AgentSessionViewProps}`）互相吞掉 ⇒ AC-1 少 2 名且无任何报警 | spec 钉住键 =（文件, 名字集）；AC-1 的 7 名与 AC-3 的稳定性即判红面；fixture 必含同目标两条语句 |
| R-4 | **`.i.ts` 零实例 / `ts_import_specifier` 待接线在本变更后仍无观测点**（explore C.7.4 #4、C.5.1 #5 的 R-7） | 提案六显式裁定不做试点并给四条理由；移交首个前端契约变更。若下游要求 C 内出实例 ⇒ 另开 change，MUST NOT 就地扩张本变更（会改 `apiFindings` 基线与 `INDEX.toml`） |
| R-5 | **`TsResolver` 复用可能把 `scan` 的记账牵连进来** | 已核：`resolve` 无 `unresolved` 副作用（`scan/ts.rs:156`），只有依赖边抽取的返回值承载留痕；spec 明写「取数面 MUST NOT 记 `unresolved`」，AC-8 的报告读数即反向判据 |
| R-6 | **`extract_element` 每次调用构建 `OwnerIndex` / `TsResolver`**（元素表 53、44 个 `path` 非空元素） | 本期不预铺：唯一两处调用点按元素单发（`load_contracts` 只对**有契约文件**的元素取数、`bootstrap` 单元素）。硬约束只写「MUST NOT 造第二份归属 / 解析」；若将来前端契约铺开成百份，再在 `load_contracts` 处复用一次索引（独立变更） |
| R-7 | **「闭包做完了」被误读成「前端契约可写」** | spec 明写预期边界（类型面 + 名面可用，`api-op` 面留白，因解构 props 抽不出字段）；AC-1 **只数名字、不验签名**，避免用 `Unknown` 形态自证能力 |
| R-8 | **版本号两跳**：`0.6.48` 已被 `frontend-component-element-coverage` 用掉 | AC-9：只 +1 一次 → `0.6.49`；`src-tauri/Cargo.toml` 不随动 |
| R-9 | **同元素约束被误当「放宽判据」**：owner 子树比 `within_path` 宽，读者可能以为 `apiMissing` 会因此变松 | spec 写明「父元素公共面 ⊇ 可达子元素面 = barrel 语义」，且具名名单纪律（不虚增）与 `apiUndeclared`（完备性）各守一面；AC-4 的三条反例即机械判据 |
| R-10 | **事实校准被读成判据变更**：`模块嵌套与选择性透传` 的两处改写（子元素 6 → 7、`SessionPanel` 由「自有导出」改为「子元素重导出」+ scenario 改写）可能被当成「父契约格式变了」 | 该 requirement 的判据面（父 → 子单向、`api-reexport` 三载体一致、六条结构规则、两条推论中的其余一条）逐字不动；改的只是「本仓 `Components.Agent` 的实测叙述」，且新叙述与闭包的机械事实（7 名来自 4 个子元素）互证。审核口径：diff 只允许出现在那一 bullet 与那一个 scenario |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| C.4 ①「`export { localA }` 是否本期补齐」 | **不补**（提案五）：需第二遍按名取回本文件声明，与单遍纪律冲突；用源码断言钉住零命中 |
| C.4 ②「跨元素 re-export 要不要一条 finding」 | **不要**：静默跳过，零 finding（取数器非权威；与 `apiRefMissing` 语义不打架） |
| C.4 ③「`TsResolver` 复用是否与 `scan` 的 `unresolved` 记账冲突」 | **关闭**（C.6.3 已核 + 本提案 R-5）：`resolve` 无副作用 |
| C.6.3 / C.6.5「同元素约束口径」 | **裁定为 ② owner 子树**（提案一），三值判据写进 spec |
| C.6.6 / C.6.9「判据 1 期望 5 还是 7」 | **7**：`SessionPanel` 元素已随 `frontend-component-element-coverage` 补齐 ⇒ 无需任何例外分支 |
| C.7.2「`unmatchedFiles` 不能当覆盖证据（gate 语义）」 | 采纳为措辞纪律：本变更不用 `unmatchedFiles` / `matched` 做任何验收 |
| C.7.4 ②「D52 留白接口保持不接线」 | 采纳（提案四末段 + AC-8 白名单 + 不要修改表） |
| C.7.4 ③ / C.9「`ts.rs` 三道闸」 | 采纳 α 方案（闭包在外层）；闸 1 / 闸 3 零改动、闸 2 按 R-1 改写 |
| C.7.4 ④「`.i.ts` 试点」 | **裁定不做**（提案六，四条理由），移交首个前端契约变更 |
| C.8.7「四条待裁 = proposal 内容」 | 已全部落纸（提案一 / 四 / 五 / 六 + 风险 R-1 / R-4） |
| C.8.1–C.8.6「文档与 B 的 explore 校准」 | **已执行完毕**（explore 内实测全绿）；本变更不再回改 explore 文档，B 的判据刷新归 B 起草 |
| C.8.1「spec / ADR / 模型已全部同步，零遗留」 | **一处例外**：`desktop-archi-api` 的「模块嵌套与选择性透传」仍把 `SessionPanel` 写成「自有导出（其文件不是元素）」、子元素计数写 6。该句随 `frontend-component-element-coverage` 过期，与闭包落地后的机械事实（7 名来自 4 个子元素）冲突 ⇒ 随本变更同批校准（1 条 MODIFIED，见提案六之后的「能力」节与 R-10） |
