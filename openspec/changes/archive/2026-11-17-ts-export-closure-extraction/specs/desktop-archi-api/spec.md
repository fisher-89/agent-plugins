# desktop-archi-api Specification（delta: ts-export-closure-extraction）

## Purpose

本 delta 只给**取数面**补一件缺件：TypeScript / JavaScript 元素的「代码导出集」由「入口单文件」扩为「入口导出闭包」，并给 `ApiSymbol` 补上「定义文件」这一事实。既有能力逐字不动：

1. **契约权威与资产形态零改动**：`<archi_api_dir>/<Domain>/<FQN>.md` 仍是唯一手写真相，三 fence、（生成物）`schema.json` / `INDEX.toml` / 入口同级 `.i.*` 落点与零 diff 门全部不动；本 delta 不产任何契约资产、不改 `INDEX.toml`（本仓仍 44 条）。
2. **取数器仍非权威**：闭包只决定「代码导出集」这一侧，判据仍只读契约 + 代码两套事实并在 `desktop-archi-api-check` 汇合。
3. **留白一条不减**：`export default` 不抽、`extract(element, file, source)`（D52）零产品调用者、`unreachable` 的 `crateOnly` / `testOnly` / `unused` 三值仍留白——本 delta 新增的留白清单（`export { localA }` / `export * as ns` / 跨元素 re-export）按同一纪律登记。
4. **为什么现在补**：barrel 的内容天然全是 re-export，取数器对此零产出 ⇒ 前端契约一写就红、`bootstrap` 对前端元素恒空骨架——「入口 = 公共面」这条机械定义在前端缺了唯一实现。
5. **顺带一处事实校准（MODIFIED）**：「模块嵌套与选择性透传」里对 `App.Frontend.Components.Agent` 的描述停在补建元素之前（「6 个子元素」「自有导出 `SessionPanel`（其文件不是元素）」），而该文件随 `frontend-component-element-coverage` 已是元素、子元素计数为 7。闭包落地后 barrel 的真实可达面第一次成为机械事实，故同批把这句改准——契约格式与「父 → 子单向 / MUST NOT 自动推导」的判据**逐字不动**。

---

## ADDED Requirements

### Requirement: TS 导出闭包的取数面

`api::extract::extract_element` SHALL 对 TypeScript / JavaScript 元素产出**入口导出闭包**：从实现入口出发，沿 `export … from` 一跳或多跳并入目标文件的导出。闭包 SHALL 是该元素「代码导出集」的唯一来源（判据的导出集一侧与 `bootstrap` 草稿同源），MUST NOT 存在第二份 TS 导出集口径。

- **入口唯一全量起点**：只有从入口可达的导出进公共面——入口自身声明的导出，加上入口 re-export 可达链上的导出。MUST NOT 把元素目录下**未被入口聚合**的文件导出并入：那是「入口 = 公共面」之外的第二套定义。入口读不到 ⇒ 空集（既有语义逐字不变，缺口由 `apiEntryMissing` 报）。
- **具名名单精确**：`export { A } from './x'` / `export type { A } from './x'` SHALL 只把 `A` 入集，MUST NOT 把 `./x` 的全部导出并入（虚增面）。`export { A as B } from './x'` SHALL 在 `./x` 取 `A`、以 **`B`** 入集（改名如实反映，MUST NOT 以 `A` 入集）。
- **通配取全量面**：`export * from './x'` SHALL 取 `./x` 的导出面（`./x` 自身再含 re-export 时递归同理）；`export *` 与逐名 re-export 在闭包内 SHALL 等价——同一簇文件的两种写法产出同名同 kind 的集合。
- **环防键 =（目标文件, 名字集）**：visited SHALL 以「目标文件 + 本次请求的名字集」为键，键命中即返回空集。MUST NOT 只用文件作键——同一目标文件的两条具名语句（本仓 `components/agent/index.tsx` 的 `export { AgentSessionView } from './agent-session-view'` 与 `export type { AgentSessionActions, AgentSessionViewProps } from './agent-session-view'`）会被互相吞掉，静默丢名字且无任何报警。
- **终止性与确定性**：`a → b → a` 环与菱形（同一类型经两条路径到达）SHALL 由 visited 收敛，MUST NOT panic、MUST NOT 重复、MUST NOT 无限递归；终止性 MUST 由 visited 保证，MUST NOT 依赖深度上限（若实现另设上限作第二道保险，触顶 MUST NOT panic、MUST NOT 把部分面冒充完整面）。同一输入两次调用 SHALL 产出逐字相等的结果（稳定序 = 入口语句序 + 目标文件声明序，MUST NOT 依赖哈希序）。
- **去重键 =（name, kind）**：同一符号经多条路径到达 SHALL 只保留首次出现；去重 MUST NOT 按结构（两个同名字不同类型不得合并，与「禁按结构去重」同一条纪律）。
- **同元素约束 = owner 子树**：re-export 目标文件 SHALL 经 `scan::OwnerIndex::owner_of` 归属，仅当结果为 `Owner::Element { fqn }` 且 `fqn == element.fqn` 或以 `"{element.fqn}."` 为前缀时并入；`Owner::Ambiguous(_)` 与 `Owner::None` SHALL 一律跳过（MUST NOT 任选、MUST NOT 放行）。FQN SHALL 按 `.` 分段比较，MUST NOT 裸 `starts_with(element.fqn)`（否则 `…Components.Agent` 会误认 `…Components.AgentFoo` 为子树）。**理由**：父元素的公共面 ⊇ 可达子元素面正是 barrel 的语义，与目录型 `path` 前缀在结论上一致（不产生第二条规则分叉），MUST NOT 被读成虚增——虚增由具名名单纪律守、完备性由 `apiUndeclared` 守。文件型 `path` 的元素（`components/agent/index.tsx` 一类）因此不需要任何例外分支。
- **specifier 解析单点复用**：目标文件解析 SHALL 复用 `scan::ts::TsResolver::resolve`（就近 tsconfig 的 `baseUrl` / `paths` / 别名 / 扩展名 / index 折叠），MUST NOT 造第二份 specifier 解析。解析失败（目标在仓外 / 不在盘 / 不可解析）SHALL 静默跳过该条 re-export：MUST NOT panic、MUST NOT 记 `unresolved`（`unresolved` 只属依赖边抽取的账）。`api::extract → scan::{ts, owner}` 的内部依赖为本能力新增，与 `api/diff.rs` / `api/paths.rs` 已有的 `scan` 依赖同族——理由是「specifier 解析与文件归属各只有一个实现」。
- **visibility 一律 `External`**：闭包内符号 SHALL 全部为 `External`（可达即公开），与 Rust 侧 `pub use` 升格的语义对齐；本 delta MUST NOT 新增 `crateOnly` / `testOnly` / `unused` 的产出路径（三值留白逐字不变）。
- **签名带元素与元素表**：`extract_element(root: &Path, elements: &[ArchiElement], element: &ArchiElement, entry: &str) -> Vec<ApiSymbol>`——同元素约束的唯一依据；与 `paths::entry_state(root, elements, element)` 同形。Rust 分支语义 SHALL 逐字不变（`mod` 树自锚，元素参数不参与，MUST NOT 因此改 `rust::extract_crate` 的抽取面）。
- **导出形态单一分派点**：`api/extract/ts.rs` 内 `match statement` SHALL 恰 1 处，一次分派产出三态（`Declaration` / `Named{local, exported, specifier}` / `Star(specifier)`），其余形态归 `None`。`ts::extract_file(file, source)` 的签名与产出语义 SHALL 逐字不变（只认 `export <声明>`，re-export 零符号）；新增的声明/名单抽取面只读源文本 + AST（零读盘、零写盘、零 spawn）。闭包（递归 + 读盘 + 解析 + 环防）SHALL 落在 `extract_element` 那一层，MUST NOT 下沉进 `ts.rs`。
- **定义位置带文件**：`ApiSymbol` SHALL 新增 `file`（定义所在文件，repo 相对 POSIX 路径）。`bootstrap` 的注记 SHALL 为 `<!-- <visibility> @ <file>:<line> -->` 且 `file` 取该符号的定义文件——MUST NOT 用入口串冒充（闭包一跳后行号属于另一个文件，注释指错文件比没有注释更坏）。Rust 侧 `file` SHALL 为该符号所在模块文件（内联 `mod` 与父同文件）。

**显式留白（MUST NOT 被伪装成已实现）**：

| 形态 | 处置 |
|---|---|
| `export { localA }`（无 `from`，导出本文件的非导出声明） | 不产出：需要「按名字取回本文件声明」的第二遍，与「取数面 = 导出面」的单遍纪律冲突；`api/extract/ts.rs` 内 `Statement::ExportNamedDeclaration` MUST 零命中（由源码断言钉住） |
| `export * as ns from './x'` | 跳过：产出命名空间对象而非符号，不猜 |
| 跨元素 / 仓外 re-export | 静默跳过，**零 finding**：取数器非权威，缺口由契约面判据显示；MUST NOT 为它新增判据（避免与 `apiRefMissing` 语义重复） |
| `export default` | 不抽（既有纪律：对外名字由导入方决定） |

**预期边界（写进能力，不许口头约定）**：前端元素的真实导出面以**类型面与名面**为主——组件函数的入参是解构 props + 内联类型字面量，抽出的参数类型恒为 `Unknown`。故本能力 MUST NOT 被读成「前端行为契约可写」：`api-op` 面在前端继续留白，`bootstrap` 对前端的价值是「名与类型面的起点」，不是可直接落纸的签名。

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

- **WHEN** 以本仓根对 `App.Frontend.Components.Agent`（`path = ./packages/desktop/src/components/agent/index.tsx`，文件型单值）取数
- **THEN** 导出集恰 7 个名字 `AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`（3 个 `Function` + 4 个 `Type`）；`AgentInputProps` / `AgentMessagesProps` / `isToolPart` 零命中（未被 barrel 导出 ⇒ 具名名单不入集）；同一次取数在 `bootstrap` 草稿里表现为非空骨架

#### Scenario: 留白形态零产出

- **WHEN** 分别对 `export { localA };`（无 `from`）、`export * as ns from './x'`、`export default function f() {}` 三种源文取数
- **THEN** 三者均零符号；`extract_file` 的产出语义不变（透传与 default 都不产符号），且 `api/extract/ts.rs` 内 `Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中（留白由源码断言钉住，不是靠「忘了写」）

#### Scenario: 注记行指向真实定义文件

- **WHEN** 对本仓 `App.Frontend.Components.Agent` 起草（`bootstrap`）
- **THEN** 每个符号的注记指向其真实定义文件与行（`AgentInput` → `packages/desktop/src/components/agent/agent-input.tsx:152`、`SessionPanel` → `…/session-panel.tsx:230`、`AskAnswerActions` → `…/agent-messages.tsx:33`），MUST NOT 出现把入口 `index.tsx` 与目标文件行号拼在一起的形态；Rust 侧同一条纪律（子模块 `pub fn` 的注记指向子模块文件）

#### Scenario: 解析失败静默跳过

- **WHEN** 入口 `export { usePanelRef } from 'react-resizable-panels'`（仓外包）、`export { X } from './not-on-disk'`（不在盘）两种形态
- **THEN** 两者都被静默跳过：`extract_element` 正常返回（零 panic、零符号、零 finding），`unresolved` 留痕为零（该账只属依赖边抽取）

---

## MODIFIED Requirements

### Requirement: 模块嵌套与选择性透传

父子元素的契约关系 SHALL 遵循「父 → 子」单向：父元素契约 SHALL 逐项列出它透传的子元素导出，MUST NOT 由「子元素面的并集」自动推导。

透传 SHALL 同时表达于三处且互相一致：

| 载体 | 内容 | 用途 |
|---|---|---|
| `<FQN>.md` 的 `api-reexport` fence | 透传清单（键 = 导出名，值 = 直接子元素 FQN） | 人读 + 生成器输入 |
| `<FQN>.md` 散文区 | 透传意图 | 人读 |
| `<FQN>.schema.json` 的 `$defs` | 本地定义 **+ 透传项（纯 `$ref`）** | 机器检查（悬空引用编译期即报） |

六条结构规则：

1. `api-reexport` fence 键集 SHALL ≡ `schema.json` 的透传 `$defs` 键集（漂移即 `apiReexportDrift`）。
2. 父 `$defs` 的透传条目 MUST NOT 携带结构关键字（`type` / `properties` / `enum` / `const` / `items` / `oneOf`）——它是纯 `$ref` alias，携带即沦为重复定义。
3. 父 `$defs` 与任一子元素 `$defs` 的**类型名交集必须为空**（`apiShadowing`）。MUST NOT 依赖编译器兜住：实测 `pub use drift::*` + 同名 `struct` 只给 `ambiguous glob re-exports` 警告，不报错。
4. 透传目标与跨模块 `$ref` SHALL 是**直接子元素**（FQN 前缀相邻）且其契约存在且 `api = declared`；跨层引用（父直连孙辈）SHALL 报 `apiRefMissing`——层级语义被架空的信号。
5. 嵌套元素的父子 MUST 同属**一个 crate**（Rust 侧跨 crate 父子即 `cyclic package dependency`，不可表达）→ `apiNestedCrossCrate`；TS 侧无此约束。
6. 子元素 MUST NOT 依赖父元素（含类型）；共享类型 SHALL 下沉到兄弟元素或消解依赖。该规则在代码层由既有 `UndeclaredDependency` 判据报（父子关系不进 relation 图，故必然报），本能力 MUST NOT 新增第二条判据。

两条推论：

- **递归透传 MUST NOT 展开**：若 A ⊃ B ⊃ C，A 只声明透传 B；B 内部已透传 C 则自动带出。
- **透传是选择性的**（实测非假设）：`App.Frontend.Components.Agent` 有 **7** 个子元素，其 barrel（`components/agent/index.tsx`，即该元素的文件型 `path`）经 `export … from` 只重导出其中 **4** 个子元素的导出——`AgentInput`（`Agent.AgentInput`）、`AskAnswerActions`（`Agent.AgentMessages`）、`AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps`（`Agent.AgentSessionView`）、`SessionPanel` / `SessionPanelState`（`Agent.AgentSessionPanel`，该文件已是元素）；`AgentBlocks` / `AgentError` / `ToolCards` 三个子元素不出现。故「子元素存在」≠「父元素透传它」，父契约仍是「透传自子元素 / 自有 / 子元素但不导出」三类混合（本元素现态无自有导出），MUST NOT 自动推导——代码侧可达面由 `desktop-archi-api`「TS 导出闭包的取数面」给出，它**不构成**父契约可以自动生成的许可。
- **环依赖从此不可能**：跨元素 `$ref` 只剩「父→子」与「兄弟→兄弟」两类边，必然可拓扑排序。禁断子→父的最大价值不是禁断本身，而是让「把子目录提升为元素」变成一次真实的解耦决策（`infra/agent` 要把 `sdk/` 提升为元素，先得把 3 个类型从 `lib.rs` 挪出来）。
- **MUST NOT 新增契约层依赖判据**：契约不是依赖事实源，事实链是「契约 → 生成物 → 代码 import → `scan` 取数 → `OwnerIndex` 归属 → `drift::check` 对账既有 `UndeclaredDependency`」。判据建在事实源（代码）上，不在投影（契约）上；新增判据只在「契约自身内部矛盾」时成立。
- **契约记录「提供什么」，不记录「实现了什么」**：MUST NOT 在 `$defs` 条目上记 `implements` 注记（重复表达 model relation、污染契约语义、判据已被编译器与 `UndeclaredDependency` 双重覆盖）。推论：三个 runner 的契约面就是 `new()`——该薄面是代码形状的事实，处置是改代码（暴露已存在的私有纯函数）而非放宽格式。

#### Scenario: 父契约逐项声明而不自动推导

- **WHEN** 为 `App.Frontend.Components.Agent` 写契约：`api-reexport` 只声明 barrel 实际重导出的 7 个名字中的 2 个（`AgentInput` / `AgentSessionView`），其余重导出项（`AskAnswerActions` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`）后续逐条追加，`AgentBlocks` / `AgentError` / `ToolCards` 三个子元素不出现
- **THEN** `api-reexport` fence 恰 2 条、`$defs` 恰 2 条纯 `$ref` 条目、本次零本地定义（该元素现态无自有导出）；三个未导出子元素零出现；生成的 `.i.ts` 为 `export { AgentInput } from …` / `export { AgentSessionView } from …`；代码侧闭包可达 7 名**不构成**「契约应自动含 7 条」的判据——父契约逐项手写（MUST NOT 自动推导）

#### Scenario: 透传条目带结构关键字即错

- **WHEN** 父 `$defs` 的透传条目同时写 `$ref` 与 `type: object` / `properties`
- **THEN** 契约错误（透传条目必须是纯 `$ref` alias）；`apiShadowing` 亦报（与子元素类型名交集非空）

#### Scenario: glob 形态重复定义由契约层拦住

- **WHEN** 父元素代码用 `pub use child::*` 且手写同名 `struct`（编译器只给 warning）
- **THEN** `apiShadowing` 报出父子 `$defs` 类型名交集，检查失败；MUST NOT 以「编译器没报错」为由放行

#### Scenario: 跨层与跨 crate 引用各报其判据

- **WHEN** 分别构造「父契约 `$ref` 孙辈元素」与「父子元素分属两个 crate」
- **THEN** 前者报 `apiRefMissing`（非直接子元素），后者报 `apiNestedCrossCrate`；两者都点名涉事 FQN

#### Scenario: 子→父依赖由既有判据报

- **WHEN** 子元素代码 `use` 父元素直接定义的类型（父 `lib.rs` 里的 `EngineFacade` 一类）
- **THEN** 既有 `UndeclaredDependency` 报出该依赖（契约层不加新判据、不出现第二份判定实现）

---

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/extract/mod.rs` | 取数器分派 + **TS 导出闭包** + nullable 形状识别单点 | `ApiSymbol` 新增 `file: String`（定义文件，repo 相对 POSIX；进程内类型，不进 `dto` / 不进 specta）；`extract_element(root, elements, element, entry)`：Rust 分支 = `rust::extract_crate` 逐字不变，TS 分支 = 入口派生闭包（递归 `surface` + visited(文件, 名字集) + `scan::ts::TsResolver::resolve` + `scan::owner::OwnerIndex::owner_of` 的 owner 子树 + `(name, kind)` 去重 + `A as B` 改名），入口读不到 / 语言未知 ⇒ 空集；`extract(element, file, source)` 仍是**显式留白（D52）零产品调用者**，本变更 MUST NOT 接线；边界 doc 更正为「`extract` / `ts` / 归一纯函数零读盘；`extract_element`（与 `rust::extract_crate`）按入口读盘、零写盘、零 spawn」 |
| `crates/core/archi/src/api/extract/ts.rs` | 单文件声明抽取 + 导出形态单一分派点 | `extract_file(file, source)` 签名与产出语义逐字不变（`= extract_surface(..).symbols`）；新增 `extract_surface(file, source) -> ExportSurface { symbols, named: Vec<NamedExport{local, exported, specifier}>, star: Vec<String> }`（纯读源文本 + AST）；`exported_item(statement) -> ExportedItem` 三态单点（`Declaration` / `Named` / `Star` / `None`），`match statement` 恰 1 处；`Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中；每符号填 `file` |
| `crates/core/archi/src/api/extract/rust.rs` | Rust 声明抽取 | `ApiSymbol.file` 填值为**当前模块文件**（贯穿 `walk_items` / `walk_modules`；内联 `mod` 与父同文件）；抽取面 / `visibility` 四值 / 签名归一 / `#[path]` 限根纪律逐字不变 |
| `crates/core/archi/src/api/mod.rs` | 契约面装配与出口 | 两处调用点补 `elements` / `element`（`element_symbols` 与 `bootstrap`；两者本已持有）；`bootstrap_text` 注记改 `symbol.file`（`entry` 退出注记面）；出口八行、`ApiSnapshot` 三面、`entry_state` 取数方式与「无入口不产 `.i.*`」逐字不变 |
| `crates/core/archi/src/api/extract/{mod_test,ts_test,rust_test}.rs` + `api/{mod_test,check_test}.rs` | 语料与源码自证 | 闭包五类语料（精确名单 / 通配等价 / 环与菱形 / owner 子树四值 / 别名改名 / 稳定性）落在 `mod_test`；`ts_test` 的闸 1（签名逐字）与闸 3（透传零符号）**保留不改**，闸 2 改为「`match statement` 恰 1 处 + 臂集合三值 + 两类留白零命中」，并新增 `extract_surface` 三面断言；`rust_test` 补「子模块 / 内联 `mod` 的 `file`」；`api/mod_test` 的 bootstrap 注记用例改为「符号定义文件」口径并新增子模块用例；`api/check_test` 夹具补 `file`（判据断言值零变化） |
| `crates/core/archi/src/scan/ts.rs` / `scan/owner.rs` | 被复用的既有单点 | **零改动**：`TsResolver::resolve(&mut self, …)`（只读缓存、无 `unresolved` 副作用）与 `OwnerIndex::owner_of`（三候选并集、最长前缀、`Ambiguous` 不任选）原样复用；`api → scan` 的内部依赖新增一条（与 `api/diff.rs` 的 `walk_source_files`、`api/paths.rs` 的 `is_source_file` 同族） |

### API: 取数器内部面（无对外 CLI / IPC 变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `api::extract::extract_element` | 改（签名 + TS 语义） | `(root, elements, element, entry)`；TS 分支由单文件扩为入口导出闭包；Rust 分支逐字不变 |
| `api::extract::ts::extract_surface` | 新增 | 声明 + 具名名单 + 通配名单三面；`extract_file` 委托它取声明面 |
| `api::extract::ts::extract_file` | 零变化 | 签名 `(file: &str, source: &str) -> Vec<ApiSymbol>` 与产出语义逐字不变（闸 1 / 闸 3 的判红面） |
| `api::extract::ApiSymbol` | 改 | 增 `file: String`；`PartialEq` / 字段序外延，既有构造点与夹具同批补值 |
| `api::bootstrap` | 零签名变化 | `bootstrap(root, element) -> Result<String, ApiError>` 不变；内部注记改按 `symbol.file` 输出 |
| `paths::ts_import_specifier` | 零变化 | 仍为待接线留白（`.i.ts` 零实例），本变更 MUST NOT 接线 |

### Component: 无

本 delta 无前端组件 / 页面 / IPC 命令面改动：`ApiSymbol` 不进 `dto`、不进 specta、不进 bindings，报告与 UI 字段面零变化。
