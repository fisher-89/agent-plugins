# desktop-archi-drift Specification

## Purpose

定义架构漂移的**判定域**：扫哪些文件（默认排除与测试 scope 正交、配置可追加）、怎么归属（元素 `path` 最长前缀单点，同长不任选）、什么算漂移（声明未兑现 / 未声明依赖 / path 失效三类，定级只看引入方）、什么时候豁免（两层、元素对粒度封闭、留痕可审计）、两模式观察根（判定域零写盘、落盘归消费面——命令面不落盘 / 门禁面两模式均落盘，变更集经唯一入口）、报告唯一形状（`status` 五值、`occurrences` 聚合、`observations` 与 `warnings` 分界、`modelHash`、并列槽位 `apiFindings` 的纳管子集与 `error` 档进 `passed`）、架构域看护配置（`<archi_dir>/config.json`，缺失即空、未知键拒绝）、判定单源（命令面与门禁面同一纯函数）与运行期预算（release p95 ≤ 3 s、语料对拍、依赖成本回退序）。判定只读盘：零写盘、零 spawn、零 Tauri、零 store、零 `workflow`、零 `config`；`desktop-archi-model` 的模型语义（`kind` / `gate` / `path` 形状）由该能力定义，本能力只**消费** `ModelGraph`。

## Requirements

### Requirement: 扫描范围与测试文件排除正交

架构漂移扫描的源文件范围 SHALL 由 `desktop-archi-drift` 自身定义，MUST NOT 复用工作区 `config.json` 的 `tests[].excludes`（那是**测试执行 scope**——可因 e2e 慢而排除、因 fixture 需要而包含，与架构扫描范围正交；两者混用会让调测试范围意外改变架构判定面）。

- **默认排除约定**（可被架构域配置的 `exclude` 覆盖，见「架构域看护配置」）：`**/*.test.{ts,tsx,js,jsx}` / `**/*.spec.{ts,tsx,js,jsx}` / `**/__tests__/**` / `**/*_test.rs` / `**/tests/**` / `**/fixtures/**` / `**/__mocks__/**`。
- **排除的语义理由**：测试文件天然跨元素引用被测代码，纳入扫描会让每个 `foo.test.ts` 对 `foo.ts` 产生一条「未声明依赖」——这不是架构问题，是测试的固有形状。
- **排除留痕**：被排除的文件 SHALL 逐条记入报告的 `scan.excluded[]`（`{ file, rule }`），MUST NOT 静默丢弃。
- **遍历面**：文件枚举 SHALL 经 gitignore 感知的目录遍历（跳过构建 / 依赖产物树），MUST NOT 逐目录 `read_dir` 自建遍历。

#### Scenario: 测试文件跨元素 import 不产生违例

- **WHEN** `src/auth/auth.test.ts` import 了 `src/billing/x.ts`（两文件分属不同元素，模型无该关系）后执行检查
- **THEN** 该文件被排除、报告 `errors` 为空；`scan.excluded[]` 含 `{ file: "src/auth/auth.test.ts", rule: "**/*.test.ts" }`

#### Scenario: 调整测试 scope 不改变架构扫描范围

- **WHEN** 在工作区 `config.json` 的 `tests[].excludes` 中新增一个源码目录（即把它排除出测试执行），随后执行架构检查
- **THEN** 架构扫描范围与该修改前逐条相同（该目录下源文件照常进扫描），两份排除面互不影响

#### Scenario: 配置 exclude 覆盖默认约定

- **WHEN** 架构域配置声明 `exclude: ["src/legacy/**"]`
- **THEN** 该目录下文件不进扫描、`scan.excluded[]` 记录规则 `src/legacy/**`；默认测试文件排除仍照常生效（覆盖是追加，不是替换）

### Requirement: 文件归属与两套解析器一套归属逻辑

检查 SHALL 把每个扫描文件归属到架构元素，判据 SHALL 为**元素 `path` 的最长前缀匹配**（`path` 可指目录、crate 目录、子目录或单文件，目录以 `/` 结尾；语言无关）。归属规则：

- 命中唯一最长前缀即归属该元素；同一最长长度命中多个元素 SHALL 判为归属不确定——文件计入 `scan.unmatchedFiles[]` 并记 `ambiguousOwner` warning，MUST NOT 任选其一。
- 只被 `gate = off` 元素覆盖的文件 SHALL 计入 `scan.unmatchedFiles[]`（不产生任何违例；off 的语义是「本模块不参与判定」）。
- 无元素认领的文件 SHALL 计入 `scan.unmatchedFiles[]`；元素未声明 `path` 时 SHALL 记 `noPath` warning 且不参与归属。
- 归属结果 SHALL 按元素聚合出线到 `scan.matched[]`（`{ element, files[] }`），供门禁诊断与视图复用。
- **生成物归属按三候选并集判据**：`.i.<ext>` 生成物（`ext ∈ {rs, ts, tsx, js, jsx}`）SHALL 由 `OwnerIndex` 的「整组剥离 `<name>.i.<ext>` → `<name>` ∪ 替换扩展名 `<stem>.i.<ext>` → `<stem>.<ext>` ∪ 主干对齐 `<name>.i.<ext>` → `<name>` 与**文件型 `path` 的去扩展名形态**全等」并集判据归属（判据定义与断言面见 `desktop-archi-api`「生成物落点与入口查找」）——生成器产出的是**入口同级**形态：目录型根的落点仍在元素根之内（`crates/infra/archi/src/lib.i.rs`，整组剥离即命中），**文件型根**的落点是根的兄弟文件（`views/welcome-view.i.ts`），与 `path` 全名 `views/welcome-view.tsx` 既不相等也不互为前缀，只有第三候选能命中。归属判据 MUST NOT 在漂移侧另写一份：`scan` 的两处 `owner_of` 调用点（源文件归属与 `target_rel` 目标归属）自动获得同一判据。
  - **对报告的语义影响**：`gate != off` 的已声明元素，其生成物 SHALL 由 `scan.unmatchedFiles[]` 移入 `scan.matched[]`（这是判定面的一次语义修正：生成物本就属于该元素）；`gate = off` 元素下的生成物仍落 `unmatchedFiles[]`（与改动前同分支，报告零变化）。本仓 `gate` 全 `off` ⇒ 既有报告逐字段、逐序不变。
  - **目标侧同等受益**：生成物被 import 时 SHALL 可作为合法 `to` 归属（本仓生成物零 `use` 语句、无人 import，故同样零变化）。

依赖提取 SHALL 由**两套解析器**承担、归属判定**只一套**：

- **TS / JS**：静态 `import` 声明（含 `import type`）与 `export … from` re-export；模块解析 SHALL 走 tsconfig / jsconfig 的 `baseUrl` + `paths`（自被解析文件向上取最近配置）与 node 解析规则，产出磁盘路径；解析失败、动态 `import()` / `require()`、宏式无法静态判定者 SHALL 记入 `scan.unresolved[]`（`{ file, specifier, message }`），**MUST NOT 冒充违例**。
- **Rust**：先建 crate 清单（解析 workspace 清单与各 member 的 `Cargo.toml`，得 crate 名与 `src/lib.rs` / `src/main.rs` 位置——MUST NOT 经 `cargo metadata` 一类子进程取数），再逐 crate 建**模块树**（`mod x;` 外部文件、`mod x { … }` 内联、`#[path = "…"]` 优先覆盖默认推导、`cfg` 条件编译两臂都进树且同模块路径不报重），最后解析 `use` 树（`crate::` / `self::` / `super::` 取当前文件模块路径上溯，其它 crate 名查 workspace crate 表）并做**模块路径最长前缀匹配**落到文件。宏生成的 `mod`、`include!`、无 `mod` 声明的孤儿文件 SHALL 记 `scan.unresolved[]`；外部 crate（`std` / 第三方）SHALL 不进归属（无元素覆盖，不算违例）。
- **不做的面**：模块树只用于路径前缀匹配，MUST NOT 做类型检查、MUST NOT 展开宏、MUST NOT 做完整 name resolution；`pub use` re-export 只算「声明了依赖」，MUST NOT 穿透追源。
- **同元素内不报**：同一元素内部文件之间的 import SHALL NOT 产生违例（跨元素边界才是判定对象）。

#### Scenario: 同一元素内 import 与跨元素 import 分辨

- **WHEN** `src/billing/x.ts` 既 import 同元素内 `src/billing/y.ts`，又 import `src/auth/token.ts`（模型无 `App.Billing → App.Auth` 声明）
- **THEN** 前者零记录；后者记一条 `undeclaredDependency`（`from = App.Billing`、`to = App.Auth`）

#### Scenario: TS 别名与 tsconfig paths 解析

- **WHEN** 以 tsconfig `paths` 声明 `@/* → src/*`，某文件写 `import { x } from '@/auth/token'`
- **THEN** specifier 解析到磁盘路径并归属到其元素（不因别名记 unresolved）

#### Scenario: Rust 模块树四种形态

- **WHEN** 分别以 `src/lib.rs` 的 `mod foo;` + `src/foo.rs`、`src/foo/mod.rs` + `src/foo/bar.rs`、`#[path = "custom/x.rs"] mod x;`、`#[cfg(unix)] mod a;` + `#[cfg(windows)] mod a;` 构造 crate
- **THEN** 模块路径分别落到 `src/foo.rs`、`src/foo/bar.rs`、`src/custom/x.rs`、两臂同模块路径且不报重复；对应 `use` 均可解析到文件

#### Scenario: 无法静态判定的形态只留痕不冒充违例

- **WHEN** 扫描遇到宏生成的 `mod`、`include!`、无 `mod` 声明的孤儿 `.rs`，或动态 `import()` / `require()`
- **THEN** 逐条记入 `scan.unresolved[]`（含 file 与 message），`errors` 为空

#### Scenario: 外部 crate 不进归属

- **WHEN** 某文件写 `use serde::Deserialize`
- **THEN** 该 `use` 不产生任何归属、不进 `errors` / `warnings` / `scan.unresolved[]`

#### Scenario: 归属不确定不任选

- **WHEN** 两个不同元素各声明同一长度的 `path` 前缀同时覆盖某文件
- **THEN** 该文件计入 `scan.unmatchedFiles[]` 且记 `ambiguousOwner` warning，`errors` 不因它产生

#### Scenario: 生成物按三候选并集归元素并进入 matched

- **WHEN** 分三组：① 元素 `App.X` 声明 `path = ["./src/x/"]` 且 `gate = "warn"`，盘上存在入口同级生成物 `src/x/src/lib.i.ts`；② 元素 `App.W` 声明 `path = ["./views/welcome-view.tsx"]`，盘上存在入口同级生成物 `views/welcome-view.i.ts`；③ 元素 `App.Mod` 声明 `path = ["./src/mod/"]`，盘上存在目录内形态 `src/mod/mod.i.rs`
- **THEN** 三者均归属各自元素并出现在 `scan.matched[]` 对应条目下，MUST NOT 出现在 `scan.unmatchedFiles[]`；把 ① 的 `gate` 改为 `off` 后该文件回到 `unmatchedFiles[]`（与改动前同分支），报告其余字段不变

#### Scenario: gate 全 off 时归属判据改动不移动报告字段

- **WHEN** 在 `gate` 全为 `off` 的真仓上，**保持排除配置不变**（`exclude` 条目不在本对照内，避免两个变量混在一条断言里）分别以并集判据改动前 / 改动后执行全量检查
- **THEN** 两份报告的 `unmatchedFiles[]` 集合与序、`matched[]`、`errors` / `warnings` / `excluded[]` 逐字段相同——唯一生成物 `crates/infra/archi/src/lib.i.rs` 所属元素 `gate = off`，`Owner::None` 与 `Owner::Element{off}` 同落 `unmatchedFiles[]`

### Requirement: 三类漂移与定级只看引入方

检查 SHALL 判定三类漂移，并 SHALL 把「模型自身不自洽的结构错」作为独立门禁判据：

| # | 漂移 | 定义 | 产出位置 |
|---|---|---|---|
| ① | 声明未兑现 | 模型声明 `A -> B`，代码中无任何 `A -> B` 的 import | `observations[]`（永不进 `status` / `passed`） |
| ② | 未声明依赖 | 代码存在跨元素 import，模型未声明该关系 | `errors[]` / `warnings[]`（按引入方 `gate` 档） |
| ④ | path 失效 | 元素 `path` 指向不存在的目录 / 文件 | `model.warnings[]` + `warnings[]` / `errors[]`（按该元素 `gate` 档） |

① 与 ② 各有一个**名字级对偶**（本变更新增，粒度更细、前置条件是关系已声明且写了 `apis`）：

| # | 名字级对偶 | 定义 | 产出位置 |
|---|---|---|---|
| ②ᵃ | 用了没声明（`undeclaredApi`） | 关系写了 `apis`，而代码对该元素对用了 `apis` 之外的名字 | `errors[]` / `warnings[]`（按引入方 `gate` 档，与 ② 完全同档规则） |
| ①ᵃ | 声明没用（`unusedDeclaration`） | 关系写的名字在扫描事实里没出现 | `observations[]`（永不进 `status` / `passed`；两桶口径与成因豁免见「依赖 API 两桶对账与求差单点」） |

- **② 的定级只看引入方**：`A` 引入 `B` 且模型无 `A -> B` 时级别 = `A` 的 `gate` 档（`strict` → `errors`、`warn` → `warnings`、`off` → 不出现）。`B` 的档位 MUST NOT 影响本条——否则一个 `strict` 元素会把全仓库 `off` 元素拽进门禁。
- **②ᵃ 复用 ② 的档位规则，不新增档位轴**：同一元素对若关系缺失，报 ②（元素对级）；若关系在且写了 `apis`，关系内的名字差额报 ②ᵃ（符号级）。两者 MUST NOT 互相替代——② 判的是「依赖没声明」，②ᵃ 判的是「声明的清单不完整」；`undeclaredApi` MUST NOT 在关系缺失时产出（那时没有 `apis` 可比）。
- **关系省略 `apis` 即退出 ①ᵃ / ②ᵃ**：两桶皆空（`apis` 的缺省语义是「不声明 API 粒度」，不是「零依赖」）。
- **②ᵃ 是唯一进 errors 的新增路径，且带证据**：import 是硬事实，故 ②ᵃ 可按 `gate` 定级；①ᵃ 无证据（≠ 反证），故恒为观察项。这条不对称是取数精度的直接后果，MUST NOT 被「对称美观」抹平。
- **层级逆向并入 ②**：kind 层级是**包含**层级不是约束层级，「允许的依赖方向」无权威来源；「声明了就合法」是唯一自洽规则。`A -> anc(A)` 一类图结构自相矛盾由结构校验判（`desktop-archi-model`），MUST NOT 在此另立方向规则。
- **type-only 算架构依赖**：`import type` 与普通 import 同权（只记 `typeOnly: true`）；MUST NOT 因类型导入而豁免；`apis` 的取数同理（`type` 前缀剥除，不因 type-only 少产名字）。
- **目标未建模**：跨元素 import 的 `to` 元素名不在模型内时 SHALL 记 warning（归因不明），MUST NOT 记 error。②ᵃ 不适用（它的前置就是关系在模型内）。
- **结构错**：模型 `structuralErrors` 非空时检查结论 SHALL 为不通过（见「判定单源与报告结构」的 `passed` 定义），三类策略分工不变——读取照常（返回结构化错误、绝不 panic）、渲染停摆（画布）、门禁判 fail。

#### Scenario: 三档定级

- **WHEN** 同一 `A 引入 B` 的未声明依赖分别在 `A.gate = strict` / `warn` / `off` 三个模型下检查
- **THEN** 分别产出 1 条 `errors[]` / 1 条 `warnings[]` / 零记录（off 下该文件计入 `unmatchedFiles[]`）

#### Scenario: 按元素对聚合不按行铺开

- **WHEN** 同一 `(App.Billing, App.Auth)` 元素对在 3 个文件各命中 1 处
- **THEN** `errors[]` 恰 1 条，其 `occurrences[]` 含 3 项（各带 `file` / `line` / `specifier` / `typeOnly`），MUST NOT 铺开成 3 条

#### Scenario: 声明未兑现只作观察项

- **WHEN** 模型声明 `App.Billing -> App.Auth`，代码中零 `A → B` import
- **THEN** 报告 `observations[]` 含一条 `unfulfilledDeclaration`，`status` 不因它变化、`passed` 不因它变假

#### Scenario: 名字级对偶的三档定级

- **WHEN** 模型某关系写 `apis = ["A"]`，代码对该元素对用了 `A` 与 `B`，分别在引入方 `gate = strict` / `warn` / `off` 三个模型下检查
- **THEN** 分别产出 1 条 `errors[]` / 1 条 `warnings[]` / 零记录（`kind = undeclaredApi`）；条目按元素对聚合恰一条，`message` 点名 `B`（多出的名字）与引入方档位，`occurrences` 复用既有形状（MUST NOT 新增报告字段）

#### Scenario: 声明多余只作观察项

- **WHEN** 模型某关系写 `apis = ["A", "B"]`，代码只用了 `A`
- **THEN** 报告 `observations[]` 恰一条 `unusedDeclaration`（点名 `B`），`errors` / `warnings` 零新增，`status` 与 `passed` 不变

#### Scenario: 关系缺失时不出 undeclaredApi

- **WHEN** 代码存在 `A → B` 的 import 而模型完全无该关系（因此也无 `apis`）
- **THEN** 只产 ①/② 元素对级结论（此处为一条 `undeclaredDependency`），MUST NOT 产出 `undeclaredApi`

#### Scenario: 同一元素对的两条观察并存

- **WHEN** 模型关系写 `apis = ["A"]` 而代码对该元素对零 import
- **THEN** `observations[]` 同时含一条 `unfulfilledDeclaration`（元素对无任何 import）与一条 `unusedDeclaration`（`A` 未出现）；两条各有自己的 `message`，MUST NOT 合并成一条，MUST NOT 因并存而把任一条抬进 `status`

#### Scenario: path 失效按元素档位定级

- **WHEN** `App.Billing`（`gate = strict`）的 `path` 指向不存在的目录，另一元素 `App.Legacy`（`gate = off`）同样失效
- **THEN** 前者产 `errors[]`（或与其它错误并列）、后者只在 `model.warnings[]` 与 `warnings[]` 留痕；两者都记 `pathNotFound`

#### Scenario: 结构错即不通过

- **WHEN** 模型含一条 `dependsOnAncestor` 结构错，代码侧零违例
- **THEN** 报告 `model.structuralErrors[]` 记录该错，检查结论为不通过（`passed = false`），且检查过程零 panic

### Requirement: 豁免两层与留痕

豁免 SHALL 分两层，且 MUST NOT 支持 glob 粒度：

- **第 1 层 声明即豁免（零配置）**：模型写了 `A -> B`，该边即合法——「这条依赖是对的」的正确动作是补声明，不是加豁免。
- **第 2 层 忽略规则**（仅用于「不能 / 不该声明」的临时面）：内联注释 `// archi-ignore-next-line: <reason>` 与架构域配置的元素对（见「架构域看护配置」）。内联判据 SHALL 为「用 import 声明的 span 行号回原文取该行或其上一行做注释匹配」，MUST NOT 依赖 AST 注释 trivia。
- **粒度封闭**：忽略规则 SHALL 只允许 `(from 元素, to 元素)` 元素对；MUST NOT 支持 `src/legacy/**` 一类 glob 豁免——glob 豁免等于永久后门，ratchet 语义失效。
- **留痕**：被抑制的违例 SHALL 逐条进 `suppressed[]`（`{ from, to, via: config | inline, reason, file?, line? }`）；reason 必填（缺失即配置非法，见「架构域看护配置」）。

#### Scenario: 内联豁免不报且留痕

- **WHEN** 某处 `import` 的上一行写 `// archi-ignore-next-line: 迁移期临时直连，见 ADR-0007`
- **THEN** 该导入不产生 `errors`；`suppressed[]` 含一条 `via = inline`（带 `file` / `line` / `reason`），画布对应边呈 `exempt`

#### Scenario: 配置豁免不报且留痕

- **WHEN** 架构域配置含 `{ from: "App.Billing", to: "App.Legacy", reason: "迁移期临时直连" }`，代码确有该依赖
- **THEN** 该元素对不产生 `errors`；`suppressed[]` 含一条 `via = config` 且带 reason

#### Scenario: 补声明即消除而非加豁免

- **WHEN** 在模型补上 `App.Billing -> App.Auth` 关系后重跑（此前该对为未声明依赖）
- **THEN** 该对不再出现在 `errors` / `suppressed`，改为正常声明关系；无豁免条目残留

#### Scenario: glob 粒度豁免被拒

- **WHEN** 架构域配置试图以 `{ from: "src/legacy/**" }` 一类非元素 FQN 表达豁免
- **THEN** 配置解析显式 `Err`（文案指明只允许 `(元素, 元素)` 元素对），检查不执行且不产出报告

### Requirement: 全量与增量两模式与观察根

检查 SHALL 有两种模式，二者共用同一判定实现与同一报告结构：

```
全量模式（workspace 级）
  缺省基准 = 无（当前工作区就是全部）
  扫描 = 整个观察根的源文件（遍历 + 排除约定）
  产出 = ①②④ 全部
  落盘 = 由消费面决定：命令面（archi_drift）不落盘（一次可重算的观测）；
         门禁面（initialize 基线检查）落盘 reports/archi/<timestamp>.json（审计面）
  用途 = 架构全景 / ratchet 决策 / 一次性体检 / 变更流程启动前的基线绿门

增量模式（change 级）
  基准 = 本 change 编辑集（`ArchiveVcsPort::change_edit_set(root, "HEAD")`，worktree 语义下即本 change 改动；唯一入口，MUST NOT 自算 diff）
  扫描 = 只扫变更集内的源文件
  产出 = ②④；① 无基准 → 只作观察项
  落盘 = 是（`reports/archi/<timestamp>.json`，门禁审计面）
```

- **模式由消费面声明**：判定纯函数收模式入参（`CheckInput.mode`），MUST NOT 按变更集是否为空、按相位名或按调用方类型推断。门禁面经 port 的 `ArchiCheckMode { Full, Incremental }` 声明（映射见 desktop-archi-gate「ArchiCheck runner port 与报告落盘」）：initialize 表头恒 `full`、implement 恒 `incremental`。
- **落盘者恒为调用方**：判定域本身（`crates/core/archi`）**零写盘**（两种模式同）——全量报告是否落盘不是判定语义，而是消费面的审计需要；落盘实现单点驻 `crates/infra/archi`（报告文件名与目录派生见 desktop-archi-gate）。
- **观察根显式**：命令签名 SHALL 带 `root`，由调用方给定（workspace 级 = 主仓根、change 级 = `detail.worktree`，归档 change 回落主仓根；initialize 基线检查 = 该 change 的 worktree）；MUST NOT 引入「当前选中 change」一类隐式上下文。
- **模型与源码同根**：模型 SHALL 从同一 `root` 派生读盘（架构域路径经 `foundation::layout`），MUST NOT 出现「模型读主仓、源码读 worktree」的混根。
- **一致性**：同一 commit、同一模型下，增量报告 SHALL 为全量报告在变更集上的子集（同一元素对不因模式不同而级别不同）。
- **零改代码面**：两种模式 MUST NOT 写源码或模型（判定域零写盘；唯一写动作是消费面对报告的落盘）。

#### Scenario: 增量只覆盖变更文件

- **WHEN** change 编辑集含 12 个源文件（其中 5 个被排除约定排除），执行增量检查
- **THEN** `scan.changedFiles = 12`、`scan.scannedFiles = 5`、`scan.excluded[]` 含那 5 条；未变更文件的违例不出现

#### Scenario: 全量由门禁面落盘

- **WHEN** 以同一 root 分别触发一次命令面全量检查（`archi_drift(root, "full")`）与一次 initialize 基线门禁全量检查
- **THEN** 两次判定结果逐字段相同（`mode` / `scan` / `errors` / `warnings` / `status` 全等，除 `timestamp` / `change`）；命令面零写盘，门禁面在 `reports/archi/` 落一份 `mode = "full"` 的报告

#### Scenario: 两模式级别一致

- **WHEN** 同一 commit 上分别跑全量与增量，且两者都命中了同一元素对 `App.Billing → App.Auth`
- **THEN** 两报告中该元素对级别相同（同为 `errors` 或同为 `warnings`），仅 `mode` / `changedFiles` / `base` 与扫描清单不同

#### Scenario: 无变更文件不算通过

- **WHEN** 增量模式下变更集内无源文件（如只改了文档）
- **THEN** 报告 `status = noChanges`（MUST NOT 为 `clean`），扫描计数为零、`errors` 为空；该形态正是「initialize 不能复用增量口径」的判据（基线时刻变更集只含本 change 的未跟踪文档与报告树，见 desktop-archi-gate「门禁挂载点」）

### Requirement: 报告结构与 status 五值

检查 SHALL 产出单一报告结构（两模式共用同一形状；落盘与否由消费面决定——命令面全量仅进程内返回、门禁面全量 / 增量均落 `reports/archi/<timestamp>.json`），字段面 SHALL 覆盖：

```jsonc
{
  "version": 3,
  "timestamp": "2026-10-20T08:31:12Z",
  "mode": "incremental",              // incremental | full
  "root": "/abs/path",
  "change": "add-billing-gate",       // 无 change 上下文（命令面）时为 null；门禁面两模式均携 change 名
  "base": "HEAD",                     // full 模式为 null（增量恒 HEAD 差，见「全量与增量两模式」）
  "modelHash": "sha256:…",            // graph/model/*.toml 聚合哈希

  "model": {
    "elements": 24, "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [ … ],        // 读存量模型遇错也照录
    "warnings": [ … ]                 // pathNotFound / noPath / ambiguousOwner
  },
  "scan": { "mode": …, "changedFiles": 17, "scannedFiles": 12,
            "excluded": [ … ], "unmatchedFiles": [ … ], "matched": [ … ],
            "unresolved": [ … ] },
  "errors":   [ { "kind": "undeclaredDependency", "from": …, "to": …, "gate": …, "occurrences": [ … ] } ],
  "warnings": [ … ],
  "suppressed": [ { "from": …, "to": …, "via": "config|inline", "reason": …, "file": …, "line": … } ],
  "observations": [ { "kind": "unfulfilledDeclaration", "from": …, "to": …, "message": … } ],
  "apiFindings": [ { "kind": "docMissing", "category": "doc", "element": …, "target": null,
                     "message": …, "occurrences": [ … ] } ],   // 契约判据（并列槽位；纳管子集按档位抬 status，error 档进 passed）
  "status": "errors"                  // clean | errors | warnings | noChanges | skipped
}
```

- **`status` 五值封闭**：`clean`（有扫描、零 errors 零 warnings 且纳管的契约判据为空）、`errors`（`errors` 非空**或纳管子集含任一 `error` 档判据**）、`warnings`（零 errors，且 `warnings` 非空**或**纳管子集非空且其中无 `error` 档）、`noChanges`（无文件可判）、`skipped`（无模型）。**`status` 推导与模式无关**：全量模式在零可判文件时同样落 `noChanges`、无模型时同样落 `skipped`。`skipped` 与 `noChanges` MUST NOT 被呈现或消费为「通过」——UI 与门禁日志 SHALL 显式区分，否则「没检查」被误读成「检查过了没问题」。
- **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向 / 目标 / 可解析性 / 归属不确定），随 `status` 影响提示强度；`observations` 是「设计态陈述」（声明未兑现 / 长期未使用），永不影响 `status`、永不进 `passed`。
- **`observations` 的两种成因不对称（本变更显式豁免）**：`unusedDeclaration` 的两种成因（真没用 / 写错或看不见）不可机械分辨，故该值只作观察项、`message` SHALL 并列两种可能；这是「同一个值 MUST NOT 承载两种缺口」纪律的**唯一豁免点**，理由与补偿见「依赖 API 两桶对账与求差单点」——MUST NOT 在别处援引本条作为放宽口径的先例。
- **`occurrences` 聚合**：同元素对的全部命中点聚合在一条记录内（按 `(from, to)` 聚合，逐条带 `file` / `line` / `specifier` / `typeOnly`），不按行铺开。
- **`suppressed` 必留痕**：豁免不等于不存在——保留被抑制条目 + 理由 + 来源，豁免了什么才可审计。
- **`modelHash`**：`graph/model/*.toml` 聚合哈希（文件名 + 字节累加，不含父目录），供消费方判断「手上报告是否对应当前模型」与历史报告追溯；MUST NOT 被用作缓存失效依据（本期零缓存）。模型字节变化即哈希变化（本变更新增 `06-frontend.toml` 的一个元素表头、改 12 条命令面关系的 `to` / `desc`、改 `06` 表头注释字节 ⇒ 本仓 `modelHash` 必然变；元素计数 54 → 55、关系计数 ±0 不变——MUST NOT 写成「模型零变化」或「计数全不变」）。
- **`scan` 三面按条目级账目验收（本变更留痕，防同一误判复发）**：`scan.scannedFiles` / `scan.unmatchedFiles[]` / `scan.excluded[]` 是**事实面的直接投影**，计数随仓内源文件与新增测试 / 语料变化——`gate = off` 元素覆盖的源文件与无主文件同落 `unmatchedFiles[]`（故某个文件由「无主」改为「`off` 元素覆盖」时条目不变，只是成因变了），命中排除规则（默认 13 条 + 配置追加项）的新增文件必进 `excluded[]`（`rule` 记命中的具体形态）。因此「报告只因模型 / 判定面而变」SHALL 按**条目级账目**验收：既有 `excluded[]` 条目（`file` + `rule`）与 `unmatchedFiles[]` 路径零增删改（数组排序带来的位序平移不计），新增条目逐条可指到本变更新增文件与既有规则；MUST NOT 用三面计数相等充当「归属 / 判定面零变化」的证据（`gate` 全 `off` 的仓 `matched[]` 恒空，计数相等根本不含归属信息）。
- **词汇封闭且单点**：报告自带**四组**封闭词汇——`status`（五值）、漂移条目 `kind`（**五值**：`undeclaredDependency` / `undeclaredApi` / `pathNotFound` / `ambiguousOwner` / `noPath`；`ALL` 由四值扩为五值）、观察项 `kind`（`unfulfilledDeclaration` / `unusedDeclaration`，前者由 ① 产出、后者由 ①ᵃ 产出——两个值此前就有，本变更只点亮后者的产出路径、`ALL` 长度不变）、**契约判据 `kind`**（`ArchiApiFindingKind` **十三值**，定义与语义见 `desktop-archi-api-check`「契约判据词汇封闭十三值与开放分类」）；每组 SHALL 在报告结构模块单点定义并附 `ALL` 常量，语料对拍断言「新增变体未同步即红」。报告词汇 MUST NOT 复用既有 DTO 词汇：`desktop-archi-model` 的 `DiagnosticKind`（结构诊断九类）与 `EdgeMark` / `NodeMark`（画布标记）语义面各不相同——混用会让「结构错」「漂移违例」两类在类型上不可辨。**`WarningKind`（旧文档警告，单值 `docNotFound`）已随 `docs` 三源退役删除**：入口文档面的读数改由契约判据的 `docMissing` / `docNotFound` / `apiDocMissing` 承载（`category = doc`），MUST NOT 在报告里恢复「文档警告」这一类别。
- **`version`**：报告结构版本号 SHALL 走显式兼容说明（`version = 1` 无 `apiFindings` 字段；`version = 2` 有该字段但 `status` 推导不读契约判据；`version = 3` 纳管口径进 `status`）。**本变更不递增**：字段面无增删，变的只是判据取值、档位与纳管口径——`version` 仍为 **3**，MUST NOT 因语义收紧而无字段变化地递增（否则消费方按版本分支会读到「形状未变的伪新版本」）。**依赖 API 面同样不递增**：`undeclaredApi` 是既有漂移词汇组的取值扩充、`unusedDeclaration` 是既有空位的产出点亮，`errors[]` / `observations[]` 的**条目形状逐字不变**（名字写进 `message`，不新增字段）——故 `version` 仍为 3。
- **`apiFindings` 为并列槽位**：契约判据结果 SHALL 落本字段（空数组 = 无契约问题），MUST NOT 与 `errors` / `warnings` / `observations` 混装——三者事实源与定级语义各不相同（模型关系图 / 可解析性与归属 / 设计态陈述 vs 契约文件）。对 `status` 的影响 SHALL 限于**纳管子集**（口径、判据与单点实现归 `desktop-archi-api-check`「报告并列槽位 apiFindings」）——本变更把该子集的口径由「契约在案元素」改写为「**入口文档承诺已成立**」（`api = declared` 且派生契约 md 在盘 / `api = none` 且 `doc` 已声明），并允许子集内 `error` 档条目把结果抬到 `errors`。`docMissing` 永不纳管 ⇒ 存量入口文档债 MUST NOT 把状态词常规化为永久 `warnings`。条目形状 `{ kind, category, element, target, message, occurrences }`：`occurrences` 复用既有聚合形状，`category` 为开放值集字符串（无 `ALL`，渲染层据此分组）。
- **`passed` 与 `apiFindings` 的关系（本变更改写）**：门禁布尔 `passed` 的判据 SHALL 为 `errors` 空 ∧ `model.structuralErrors` 空 ∧ **`error` 档 `apiFindings` 为空**——原纪律「`apiFindings` MUST NOT 影响 `passed`」**作废**，MUST NOT 在任何 spec / 代码注释里残留（它只在「判据级档位最高到 `warn`」的前提下成立）。`unusedDeclaration` MUST NOT 进 `passed`（观察项）。
- **推导序不变**：`skipped` → `errors`（结构错 / `errors[]` 非空 / 纳管子集含 `error` 档）→ `noChanges`（可判文件为 0）→ `warnings`（`warnings[]` 非空 或 纳管子集非空且无 `error` 档）→ `clean`；按此序 `status == errors ⟺ passed == false` SHALL 恒成立（`error` 档判据的触发条件都蕴含纳管成立，故不存在「打断 `passed` 却不进 `status`」的旁路）。

#### Scenario: 五值可辨

- **WHEN** 分别以「零模型」「无变更文件」「零违例」「仅 warn 档违例」「存在 strict 档违例」构造五次检查
- **THEN** `status` 依次为 `skipped` / `noChanges` / `clean` / `warnings` / `errors`；`skipped` 与 `noChanges` 的报告 `errors` 均为空且不带任何「通过」语义字段

#### Scenario: 观察项不影响结论

- **WHEN** 报告含 3 条 `observations` 与零 `errors`
- **THEN** `status = clean`（或 `warnings` 仅当其 `warnings` 非空或纳管子集非空）、`passed = true`；`observations` 不进门禁判据

#### Scenario: 报告词汇与 DTO 词汇互不混用

- **WHEN** 审查报告 `kind` 词汇与 `desktop-archi-model` 的 `DiagnosticKind` / `EdgeMark` / `NodeMark`
- **THEN** 各词汇组各自单点定义、互不复用；报告结构模块为 `status`、漂移条目、观察项与**契约判据**各持一份 `ALL` 常量（`ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiApiFindingKind`），语料对拍覆盖全部取值；`WarningKind` 不在任何一组内（已删除）

#### Scenario: modelHash 随模型变化

- **WHEN** 修改任一 `graph/model/*.toml` 字节后重跑（本变更新增 `06` 的一个元素表头、改 12 条关系的 `to` / `desc` 与 `06` 表头注释即属此列）
- **THEN** `modelHash` 变化；未修改模型时两次运行的 `modelHash` 相同

#### Scenario: apiFindings 并列而不混装

- **WHEN** 报告同时含 1 条 `warnings` 与 36 条 `apiFindings`（全部 `docMissing`，且这些元素均未写入口文档）
- **THEN** `warnings` 仍只 1 条、`status = warnings`（成因是 `warnings` 非空而非契约判据）、`passed` 不受这 36 条影响；两类条目不互相出现

#### Scenario: 入口文档在案的判据按档位抬 status

- **WHEN** 报告 `errors` 为空、`warnings` 为空，`apiFindings` 含一条属于「契约 md 在案」元素的 `apiSignatureMismatch`（`error` 档）
- **THEN** `status = errors`、`passed = false`；而同元素只含一条 `apiDocMissing`（`warn` 档）时 `status = warnings`、`passed = true`；把该元素的契约 md 移走后两者都退化为 `docMissing` ⇒ `status = clean`、`passed = true`

#### Scenario: 存量入口文档债不进状态词

- **WHEN** 报告含 36 条 `docMissing`（本仓当前读数：33 条 `declared` 契约未写 + 3 条 `api = none` 未声明 `doc`）、零 `errors`、零 `warnings`
- **THEN** 纳管子集为空 ⇒ `status = clean`、`passed = true`；MUST NOT 因这 36 条把状态词抬成 `warnings`

#### Scenario: 版本未递增而口径已变

- **WHEN** 分别读取 `version = 3` 的新报告与 `version = 3` 的历史报告
- **THEN** 两者字段面相同；差异只在 `apiFindings` 的取值 / 档位与 `status` 推导结论（历史报告由旧口径产生，不回写、不重算）；消费方 MUST NOT 期望靠 `version` 区分两种口径——本次口径变化由 `desktop-archi-api-check` 的 spec 与 change 留痕承载

#### Scenario: 依赖 API 面不新增报告字段

- **WHEN** 审查含 `undeclaredApi` 与 `unusedDeclaration` 的报告 JSON
- **THEN** `errors[]` / `observations[]` 的条目键集与不含二者时逐字相同（`kind` / `from` / `to` / `gate` / `message` / `occurrences`）；多出的名字只出现在 `message` 内；`version` 仍为 3

#### Scenario: 全量报告形状与增量同构

- **WHEN** 取一份 initialize 基线全量报告与一份 implement 增量报告
- **THEN** 两者字段集完全一致（无「全量独有 / 增量独有」字段），差异只在字段值：`mode`（`full` / `incremental`）、`base`（`null` / `HEAD`）、`scan.changedFiles`（`0` / 变更数）与扫描清单

#### Scenario: 无模型与零可判文件两态可辨

- **WHEN** 分别以「模型目录缺席」与「模型在但零源文件通过排除约定」的观察根执行全量检查
- **THEN** 前者 `status = skipped`、后者 `status = noChanges`；两者 `errors` 皆空但 MUST NOT 呈 `clean`，消费方（门禁 / UI）按各自文案分列「未检查（无模型）」与「未检查（无可判文件）」

### Requirement: 架构域看护配置（豁免与扫描排除）

架构域配置 SHALL 落 `<archi_dir>/config.json`（路径经 `foundation::layout` 派生，MUST NOT 行内字面量），字段面封闭为两项：

```jsonc
{
  "ignore":  [ { "from": "App.Billing", "to": "App.Legacy", "reason": "迁移期临时直连，见 ADR-0007" } ],
  "exclude": [ "src/legacy/**" ]
}
```

- **归属面写死**：工作区 `.dev-team/config.json` 管**执行**（命令 / scope / 阈值 / 保护），本文件管**看护策略**（豁免 / 扫描排除）；MUST NOT 出现第三个配置文件，MUST NOT 把 `architecture` 键塞进工作区 `config.json`（其 `KNOWN_KEYS` 七项零变化）。
- **缺失即空配置**：文件不存在 SHALL 视为 `{ ignore: [], exclude: [] }`（冷启动合法起点），MUST NOT 报错。
- **严格性**：未知键 SHALL 被拒绝（防拼错静默失效）；`ignore[].reason` 必填且非空白；`ignore[].from` / `to` SHALL 为模型内存在的元素 FQN（不存在即显式 `Err`——配置是声明，指向已删除元素就是配置漂移）；`exclude[]` SHALL 为文件级 glob（目录以 `/**` 收尾）。
- **粒度封闭**：`ignore` MUST NOT 接受 glob 或元素名以外的形态（见「豁免两层与留痕」）。
- **零读取面扩张**：本文件 MUST NOT 被工作区配置 crate 读取；`desktop-workspace-config` 的 `KNOWN_KEYS` / schema 镜像 / 前端 config 页零变化。

#### Scenario: 配置文件缺失零豁免

- **WHEN** `<archi_dir>/config.json` 不存在而代码存在未声明依赖
- **THEN** 检查照常执行并按 `gate` 档报出（零豁免生效），零报错、零 warning 关于配置文件缺失

#### Scenario: reason 必填与元素存在性

- **WHEN** 配置写 `{ from: "App.Billing", to: "App.Legacy" }`（缺 reason），或 `to = "App.Ghost"`（模型内不存在）
- **THEN** 两次均显式 `Err`（文案分别指明缺 reason / 未知元素），检查不执行、不落报告

#### Scenario: 未知键被拒

- **WHEN** 配置文件写 `{ ignores: [ … ] }`（拼错键）
- **THEN** 解析显式 `Err`（指明未知字段），MUST NOT 静默忽略而让用户以为豁免生效

#### Scenario: 工作区配置零影响

- **WHEN** 审查工作区 `config.json` 的 `KNOWN_KEYS`、`config.schema.json` 镜像与前端 config 页
- **THEN** 均与本变更前逐字一致；架构域配置只经 `foundation::layout` 的派生路径读取

### Requirement: 判定单源与消费面同源

架构漂移的判定 SHALL 只有一处实现（`crates/core/archi` 的漂移判定纯函数：输入 = 观察根 + 模式 + 变更集 + 配置 + 模型，输出 = 报告）。消费面 SHALL 为**五处**调用点——IPC 命令面（`archi_drift(root, mode)`）、implement 相位门禁 runner（增量）、initialize 基线门禁 runner（全量）、**引擎工具面（`archi_drift` 只读工具的恒全量调用）**、**引擎收口自检（archi 会话自然收敛前的恒全量调用）**——后两者为同一入参形态（全量 + 空变更集 + 无 change 上下文），前两者为**同一 runner 的两次挂载**（见 desktop-archi-gate「门禁挂载点」），MUST NOT 各写一份归属 / 定级 / 状态推导；报告结构体 SHALL 为全部消费面共用的唯一形状（命令面出线经同一 DTO 投影，门禁面从同一结构派生 `passed` / 诊断文本，引擎面从同一结构经`drift_summary` 投影出工具响应与 notice 载荷）。

- **引擎两面的入参恒定**：`mode = Full`、`changed = &[]`、`change = None`、`root = 会话执行根`；MUST NOT 传 `Incremental`（`infra/agent` 不依赖 `vcs`，取不到 `change_edit_set`；而 archi 臂与相位会话的 root 皆无 change 基准语义）、MUST NOT 由消费面自行拼报告。
- **引擎面零写盘**：调用方 MUST NOT 落盘报告（命令面同款裁定）；落盘者恒为门禁 runner（`reports/archi/<timestamp>.json`）。
- **取消除外**：引擎工具面与收口自检 SHALL 以 `CancelFlag::never()` 调用（不为几十毫秒扩取消通道）；`CheckError::Stopped` 对其不可达，出现即按失败面处置（`is_error` / `skipped` notice）。命令面同款：`CancelFlag::never()`，**无取消 ⇒ 无 in-flight 去重、无节流**（下文「并发扫描」条）。
- **并发扫描（已知限制，零代码护栏）**：命令面在阻塞池执行后窗口可交互，重复触发 SHALL 产生**并行的全仓扫描**——连点刷新（`refreshTick` 变 ⇒ `<ArchitectureCanvas key>` 整块重挂 ⇒ 三个 hook 全重取）、反复切页、重挂载各起一条，每条各扫同一批文件并各产完整报告。本能力 MUST NOT 因此新增取消通道、MUST NOT 引入 in-flight 去重或节流、MUST NOT 为此调整 blocking 池配置；此类治理（取消 / 去重 / 增量触发）SHALL 归独立变更。并发扫描 MUST NOT 被读作回归：单次成本改造前后等同，变化只发生在「窗口挡不挡住第二次触发」。

#### Scenario: 命令面与门禁面逐字段同源

- **WHEN** 以同一 root、同一模式（增量）与同一变更集分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告除 `timestamp` / `change`（命令面无 change 上下文）与落盘路径外逐字段相等（`model` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `status` 全等）

#### Scenario: 全量面同源

- **WHEN** 以同一 root 分别经 `archi_drift(root, "full")` 与 initialize 基线门禁、引擎工具面、引擎收口自检触发全量检查
- **THEN** 四份报告除 `timestamp` / `change` / 落盘与否外逐字段相等；`passed` 派生面（`errors.is_empty() && model.structuralErrors.is_empty()` + `error` 档判据）四处一致——MUST NOT 出现「工具面 clean 而门禁面红」的同根分歧

#### Scenario: 零第二套判定

- **WHEN** 审查命令面、infra runner、`sdk/archi_tool.rs` 的 `archi_drift` 分支与 `sdk/preclose.rs` 的源码
- **THEN** 五处调用点均只做「取输入 → 调判定纯函数 → 投影 / 落盘」，无归属匹配、无定级分支、无 status 推导的第二份实现；模式差异只体现为入参，不体现为实现分支；引擎两面的投影共用 `drift_summary` 单点

#### Scenario: 重复触发产生并行扫描（已知限制）

- **WHEN** 在命令面一次全量检查未完成时连点刷新两次，使两次检查的扫描窗口重叠
- **THEN** 两次检查各自完整执行并各产出一份逐字段等价（除 `timestamp`）的报告；命令面零取消（`CancelFlag::never()` 不变）、零去重、零节流；该形态在案为已知限制，MUST NOT 驱动出「为命令面加取消 / 去重」的顺带改动

### Requirement: 运行期预算与语料

检查的运行期成本 SHALL 有明确预算与语料守护：

- **预算**：单次检查（300 元素模型 + 1000 源文件语料）SHALL 在 release 构建下 p95 ≤ 3 s（软线 1.5 s）；计时 SHALL 用 `std::time::Instant`，MUST NOT 引入统计框架依赖。
- **语料**：`crates/core/archi/tests/corpus/` SHALL 覆盖判定面用例（归属 / 定级 / 豁免 / 未解析 / 模式一致性 / 结构错 / 空模型 / 无变更），逐 case 以 `expected/<case>.json` 断言；Rust 模块树 SHALL 另有独立语料集（`#[path]` / `cfg` 两臂 / 内联模块 / 孤儿文件 / 外部 crate / 宏生成）。
- **基准规模**：1000 文件是设计基准（本仓约 250 非测试源文件，1000 约为其 4 倍，覆盖中型单体 / 小型 monorepo）；超出该量级时先看模式选择（改用增量），MUST NOT 直接放宽硬线。
- **回退序**：依赖成本（安装包 / 主二进制 / 锁内包数 / 编译用时）超标时 SHALL 按「减 oxc 特性 → 自算路径替代 resolver → 正则 import 替代 parser」的代价递增序回退并重测；走到第三步（放弃 `oxc_parser`）属设计裁决变更，MUST NOT 在实现层自行决定。

#### Scenario: 基准语料 p95 在预算内

- **WHEN** 以 release 构建跑 `crates/core/archi/tests/perf.rs`（1000 源文件 + 300 元素，10 次取 p95）
- **THEN** p95 ≤ 3 s；结果与基线一并记入 `crates/core/archi/tests/README.md` 的实测表

#### Scenario: 判定面语料逐 case 对拍

- **WHEN** 运行语料回归（含未声明依赖三档 / 聚合 occurrences / type-only / 内联与配置豁免 / 解析失败 / 结构错 / 空模型 / 无变更 / 全量与增量一致性）
- **THEN** 每 case 的报告结构（`status` / 计数 / 关键条目）与其 `expected/<case>.json` 一致；golden 变更经显式重写留痕

### Requirement: 看护与门禁不读 specs（迁移边界）

架构看护（结构校验 / 图诊断）与门禁结论 SHALL NOT 读取 `openspec/specs/`，SHALL NOT 依赖 `spec ↔ element` 映射：

| # | 边界 | 理由 |
|---|---|---|
| 1 | 结构校验 / 图诊断不读 `specs` | 否则「模块入口文档替代 `specs`」完成那天，看护逻辑要整体改写 |
| 2 | 门禁结论（`passed` / `status`）不依赖 `specs ↔ element` 映射 | 该映射将来由「入口文档 ↔ element」承担，不由 `specs` 承担 |
| 3 | `specs` 不参与扫描语料与判据语料 | 扫描面由 `ExcludeSet` 与源扩展名界定，判据语料只取模型 + 盘上载体 |

- **零读取路径**：产品源码 MUST NOT 出现 `openspec` 路径拼接（路径一律经 `foundation::layout`），看护链 MUST NOT 新增 `specs` 读取、解析或哈希。
- **零映射预铺**：MUST NOT 引入 `spec ↔ element` 映射、MUST NOT 让 `specs` 参与 `apiFindings` / `errors` / `warnings` 的任一条。
- **零替代**：本能力 MUST NOT 为 `specs` 提供第四条读取来源以「顺便保住检索」——ADR 与文档面的替代路径见 `desktop-archi-agent`「检索面纪律与迁移边界」。
- **迁移归独立变更**：「模块入口文档替代 `specs`」的迁移（含 acceptance 可断言结构与归档 `SpecSync` 增量合并语义的对应物缺失问题）需独立变更裁决，本能力零预铺。

#### Scenario: 扫描与判据语料不含 specs

- **WHEN** 以本仓根跑全量检查并审查 `scan.matched` / `scan.unmatchedFiles` / `apiFindings`
- **THEN** `openspec/` 前缀的条目只出现在 `scan.excluded[]`（`rule = "openspec/**"`，见 `repo-archi-model` 的架构域看护配置）；`apiFindings` 内零条目的成因是 `specs` 内容

#### Scenario: 门禁结论与 specs 无关

- **WHEN** 增 / 删 / 改 `openspec/specs/` 下任意文件后重跑
- **THEN** `modelHash` / `errors` / `warnings` / `apiFindings` / `status` / `passed` 逐字段不变（除 `timestamp`）

### Requirement: 资产哈希（assets_hash）与报告 modelHash 的分工

判定域 SHALL 另提供一份**资产哈希** `assets_hash(root) -> String`（`archi::drift` 公开面），用于回答「本轮会话前后架构资产有没有变」这类消费侧问题（当前唯一消费者 = 引擎收口自检，见 `desktop-archi-agent`「收口前引擎自检（架构判据读数）」）：

- **覆盖面 SHALL 为四类架构资产**：模型声明（`graph/model/*.toml`）、契约资产（`models/**/*.md`）、契约 schema（`models/**/*.schema.json`，含与契约 md 同级的派生 schema）、契约索引（`graph/INDEX.toml`）。路径 SHALL 经 `foundation::layout` 派生，MUST NOT 行内字面量。
- **累加口径 SHALL 与 `model_hash` 同构**：按**相对路径排序**后逐文件累加「文件名 + 字节」，输出 `sha256:` 前缀的十六进制摘要——同一份盘面必得同一值，与文件遍历顺序 / 文件系统时间戳无关。
- **与 `modelHash` 的分工封闭**：报告字段 `modelHash` SHALL 继续只覆盖 `graph/model/*.toml`（口径逐字不变，仍是「手上报告是否对应当前模型」的判据）；`assets_hash` MUST NOT 被写进报告、MUST NOT 取代 `modelHash`、MUST NOT 被用作缓存失效依据（本期零缓存）。
- **零写盘、零 spawn**：本函数只读盘（判定域既有边界逐字不变），不落盘、不触发 git。
- **稳定与容错**：目录 / 文件缺失 SHALL 按「不参与累加」收敛（冷启动合法，MUST NOT 报错）；单文件读取失败 SHALL 以空字节参与（宁可哈希变化，MUST NOT 让消费侧因 IO 抖动失败）。

#### Scenario: 四类资产各改一处即变

- **WHEN** 在固定盘面上依次①改任一 `graph/model/*.toml` 一个字节、②改任一 `models/**/*.md`、③改任一同级 `*.schema.json`、④改 `graph/INDEX.toml`
- **THEN** 四种改动各自使 `assets_hash` 变化；未改动时连续两次调用（含跨进程重启）返回值逐字相同

#### Scenario: 与 modelHash 面不同

- **WHEN** 只改契约 md（`models/**/*.md`）而不动任何模型 toml，分别取报告 `modelHash` 与 `assets_hash`
- **THEN** `modelHash` 逐字不变（既有口径），`assets_hash` 变化——两者覆盖面差异即本条的存在理由

#### Scenario: 空域与缺目录

- **WHEN** 在无 `graph/`、无 `models/` 的空观察根上调用
- **THEN** 返回确定的摘要串（缺失按不参与累加），零错误、零 panic

### Requirement: 依赖 API 两桶对账与求差单点

判定域 SHALL 提供一份**求差纯函数**，供两处消费面共用（漂移对账与 IPC 命令面 `archi_relation`），MUST NOT 存在第二份求差实现：

```text
api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>)
  → (used ∖ declared, declared ∖ used)        两侧各自去重 + 字典序
```

- **输入口径**：`declared` = 某条 `[[relation]]` 的模型 `apis`（空 = 未声明）；`used` = 该元素对的扫描事实名（`DependencyEdge.names` 按元素对聚合、去重后的并集）。比对 SHALL **逐字符**（MUST NOT 大小写折叠、MUST NOT 驼峰归一——否则判定侧常驻第二处映射规则）。
- **两侧对称去重**：同一名字在多文件命中只算一项；输出 SHALL 为稳定字典序，使两消费面对同一元素对输出逐字相同、报告可逐字 diff。
- **`used` 是 under-approx（已知上限三条，显式登记）**：① Rust 方法调用不可见（`use ... CancelFlag` + `flag.is_set()` 只产 `CancelFlag`）；② 内联限定路径（`exec::SystemCommand::bare(..)` 未写 `use`）不产名；③ **观察根无 workspace 清单时 Rust 事实为零**（本仓现状：清单在 `packages/desktop/src-tauri/Cargo.toml`，crate 发现只读 `root/Cargo.toml` ⇒ 49 条后端关系零事实）。三条都只影响「看得见多少」，MUST NOT 被读成「依赖不存在」。
- **`declared ∖ used` 的两种成因不可机械分辨（显式豁免「一个值一种成因」）**：(a) 真没用；(b) 写错或看不见。本能力在既有纪律「同一个值 MUST NOT 承载两种缺口」上**显式豁免这一处**——理由是取数面是 under-approx、下限不得靠猜（把 (b) 判红就是假阳）。补偿三条：① 该桶**只落 `observations[]`**，永不进 `status` / `passed`；② 条目 `message` SHALL **并列**两种可能（「可能已不再依赖，也可能名字写错或该依赖以方法调用 / 内联限定路径 / glob 形态存在而扫描看不见」），MUST NOT 只写「没用」；③ 呈现面（`desktop-archi-canvas` 的关系面板）同款并列。
- **`used ∖ declared` 才是有证据的一侧**：import 是硬事实——模型写了 `apis` 却用了清单外的名字，即可按引入方档位报出（语义与定级见「三类漂移与定级只看引入方」）。
- **零噪声边界**：关系省略 `apis`（或空数组不被接受）⇒ 两桶皆空、报告零新增条目；`apis` 非空但该元素对零事实 ⇒ 只有 `declared ∖ used` 一桶（另一侧本就为空）。
- **不缓存、不落盘、不进报告字段**：本函数是纯计算；`used` 的取样成本由消费面自负（命令面按需、drift 面复用既有一次扫描），MUST NOT 引入跨调用缓存。

#### Scenario: 求差逐例

- **WHEN** 依次以 `(declared, used)` = `([] , [])` / `(["A","B"], ["A","B"])` / `(["A","B"], ["A"])` / `(["A"], ["A","B"])` / `(["B","A","A"], ["B","A"])` 调用
- **THEN** 依次得 `([] , [])` / `([] , [])` / `([] , ["B"])` / `(["B"], [])` / `([] , [])`；输出两侧恒字典序、重复项恒合并（模型侧重复本应由 `apis` 值面拒收，此例只证函数自身稳健）

#### Scenario: 两消费面同源

- **WHEN** 对同一 `(root, from, to)` 分别经 `drift::check` 的对账与 `archi_relation` 命令取两桶
- **THEN** 两侧的 `used ∖ declared` / `declared ∖ used` 逐名相同；源码审查显示两处都只调 `api_buckets`，无第二份 `contains` / 集合差实现

#### Scenario: 未填 apis 零噪声

- **WHEN** 模型所有关系的 `apis` 均省略，跑一次全量检查
- **THEN** `errors` / `warnings` / `observations` 与不加本能力时逐字段相同（零 `undeclaredApi`、零 `unusedDeclaration`）

#### Scenario: 事实面为零时只留观察项且文案并列两种成因

- **WHEN** 某后端关系写了 `apis = ["SystemCommand"]`，而观察根无 workspace 清单（Rust 事实面为零）
- **THEN** 报告产出恰一条 `observations[].kind = unusedDeclaration`（`errors` / `warnings` 零新增、`status` / `passed` 不变）；其 `message` 同时含「可能已不再依赖」与「名字写错或扫描看不见」两种可能，MUST NOT 只写「没用」

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/scan/exclude.rs` | 排除矩阵 | 默认约定常量 + 配置 `exclude` 覆盖匹配（globset）；产出 `excluded[{ file, rule }]`；与工作区 `tests[].excludes` 零耦合 |
| `crates/core/archi/src/scan/ts.rs` | TS / JS 提取 | 静态 `import` / `export … from` 的 specifier + `typeOnly` + 行号；tsconfig / jsconfig `baseUrl` + `paths` 解析；不可判定 → `unresolved` |
| `crates/core/archi/src/scan/rust.rs` | Rust 提取 | workspace 清单解析（`Cargo.toml` + member glob，零子进程）→ crate 清单；模块树（`mod` 外部 / 内联 / `#[path]` / `cfg` 两臂）；`use` 树最长前缀匹配；孤儿与宏生成 → `unresolved` |
| `crates/core/archi/src/scan/owner.rs` | 归属单点（判据修正） | `owner_of(path) -> Option<fqn>`：元素 `path` 最长前缀；同长多命中 → `ambiguousOwner` + unmatched；`off` 元素覆盖 → unmatched。生成物候选集为**三候选**（整组剥离 `<name>.i.<ext> → <name>` ∪ 替换扩展名 `<stem>.i.<ext> → <stem>.<ext>` ∪ 主干对齐 = 与文件型 `path` 的去扩展名形态全等），对每候选各跑一遍最长前缀并合并定档；`normalize` / `prefix_matches` / `OwnerIndex::build` 语义零变化；同长多命中仍 `Ambiguous` |
| `crates/core/archi/src/scan/mod.rs` | 扫描编排（零判据改动） | `scan(root, mode, changed, config) -> ScanOutcome`：遍历 + 排除 + 归属 + 逐文件依赖收集；纯读盘、零写盘、零 spawn。两处 `owner_of` 调用点（源文件归属、`target_rel` 目标归属）自动获得并集判据，`gate = off` 分支语义逐字不变；`walk_source_files` / `is_source_file` 提为 `pub(crate)` 供文件树复用 |
| `crates/core/archi/src/scan/owner_test.rs` | 判据断言面（修正） | 三候选各正例（`crates/infra/archi/src/lib.i.rs` / `views/welcome-view.i.ts` / `components/agent/index.i.ts` / `src/dir.i.ts` 对 `src/dir.ts` 根）；`src/a.ts` 与 `src/a.tsx` 同场 ⇒ `Ambiguous`；白名单外后缀四条反例与 `src/dir.i.ts` 并集定档用例保持绿 |
| `.dev-team/architecture/config.json`（新增，见 `repo-archi-model`） | 扫描排除配置 | `{ "exclude": ["openspec/**"] }`（不带 `$schema`）；生效后 `openspec/` 下源文件由 `unmatchedFiles[]` 移入 `scan.excluded[]`，判定规则零变化 |
| `crates/core/archi/src/drift/mod.rs` | 判定单点 + 报告装配单点 | `check(CheckInput) -> Report`：事实 vs 声明对账、三档定级、豁免抑制、`status` 推导；`check()` 内调 `api::check` 装配 `api_findings`（与模型面并列）并经 `api::gating_findings` 取纳管子集参与 `status` 推导（口径：入口文档承诺已成立；子集内任一 `Error` 档 ⇒ `errors`，否则至少 `warnings`）；`passed` 判据追加「error 档 `apiFindings` 空」；命令面与门禁面唯一入口 |
| `crates/core/archi/src/drift/report.rs` | 报告结构与四组词汇单点 | `ArchiDriftReport` 既有 14 字段零改动 + 新增 `api_findings: Vec<ArchiApiFinding>`（线格式 `apiFindings`）；`REPORT_VERSION` = 3（1 无字段 / 2 有字段但 `status` 不读判据 / 3 纳管口径入 `status` 推导；本变更不递增）；四组 `ALL`（`ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiApiFindingKind`）；`ArchiApiFindingKind`（**十三值** + `ALL` + `as_str` + `severity()`）、`ApiFindingSeverity`（三值 `Off` / `Warn` / `Error`）、`ArchiApiFinding { kind, category, element, target, message, occurrences }`；`serde_json` 序列化 + `ArchiCheckStatus` 五值封闭；`WarningKind` 相关表述删除 |
| `crates/core/archi/src/drift/config.rs` | 看护配置 | `load_config(root) -> Result<CheckConfig, String>`：读 `<archi_dir>/config.json`（经 `foundation::layout` 派生）；缺失即空；`deny_unknown_fields`；`ignore` 元素对 + reason 必填 + FQN 存在性校验 |
| `crates/core/archi/src/drift/report_test.rs` | 报告面测试 | `apiFindings` 空 / 非空两态、`version = 3`、纳管子集三条臂（契约在案 `error` 档 ⇒ `errors` / 契约在案 `warn` 档 ⇒ `warnings` / 承诺未成立 ⇒ 结论不变）、既有 14 字段逐字不变、JSON 往返、稳定序 |
| `crates/core/archi/tests/corpus/drift/expected/*.json` | 既有金样 | 十六份随「首错短路 + 纳管分档」显式重录并逐份留痕成因；`api-mismatch` case 保持非空 `apiFindings`；沿 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 |
| `crates/core/archi/Cargo.toml` | 依赖扩展 | 新增外部依赖 `oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn`（+ `globset` 提为直接依赖）；仍零 Tauri / 零 store / 零 spawn / 零 `workflow` / 零 `config` 依赖 |
| `src-tauri/src/commands/archi/mod.rs`（`archi_drift`） | 命令面薄包装 | `archi_drift(root: String, mode: ArchiDriftMode) -> Result<ArchiDriftReport, String>`（`ArchiDriftMode` = `full` \| `incremental`，serde 小写词、specta `Type` 出线）：增量模式经 vcs 缝取 `change_edit_set(root, "HEAD")` 后调判定纯函数；全量模式直接调；命令层零判定规则。命令形态自 archi-file-tree-lazy-load 起为 `pub async fn` + `spawn_blocking`（执行线程纪律的唯一持有者是 `desktop-app-shell`「命令执行线程纪律」）；`archi_drift_inner` 的入参（`mode` / `&dyn ArchiveVcsPort` / `CancelFlag::never()`）与错误文案零改动；命令面零取消、零去重、零节流（并发扫描为在案已知限制） |
| `crates/core/archi/tests/` | 语料与预算 | `corpus/<case>/` + `expected/<case>.json` + `golden_test.rs` + Rust 模块树语料集 + `perf.rs`（release p95）；`tests/README.md` 记实测表 |

### API: `crates/core/archi/src/drift/{mod,report}.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `REPORT_VERSION` | 保持 3 | 本变更不递增：字段面无增删，变的只是判据取值 / 档位 / 纳管口径；兼容说明三点（1 无字段 / 2 有字段但 `status` 不读判据 / 3 纳管进 `status`）逐字保留 |
| `derive_status` | 改 | 纳管子集入参保留；新增 `errors` 分支（子集含 `error` 档）；`warnings` 分支收为「子集非空且无 `error` 档」；`passed` 派生面追加「error 档 `apiFindings` 空」；其余四值序不变 |
| `drift::check` | 改 | `api_findings` 与纳管子集各自装配，子集口径换为「入口文档承诺已成立」；报告字段 / 排序 / 语料面零改动 |
| 扫描 / 归属 / 定级 / 豁免 / 配置 / 单源 / 预算 | 零改动 | 七条既有 requirement 逐字不变 |

### CLI / 语料

| 项 | 变化 | 描述 |
|---|---|---|
| `tests/corpus/drift/expected/*.json` | 16 份重写 | `version` 2 → 3；`api-mismatch.full.json` 的 `status` 由 `clean` 变 `warnings`（契约在案判据纳管），其余 15 份除 `version` 外逐字不变 |
| `tests/corpus/drift/api-mismatch/models/01-app.toml` | 注释随动 | 文件头「契约面 MUST NOT 进 `status` 推导 …… `status = clean`」改为纳管口径（`status = warnings`） |

### 模块增量：纳管口径换面（module-doc-field-docs-retirement）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|------|
| `crates/core/archi/src/drift/report.rs` | 报告结构与词汇 | `ArchiApiFindingKind` **十三值** + `ALL` + `as_str` + `severity()`（三值档位）；`ArchiDriftReport.api_findings`；`REPORT_VERSION = 3` 不递增；`WarningKind` 相关表述删除 |
| `crates/core/archi/src/drift/mod.rs` | 状态推导 | `check()` 装配 `api_findings` 并取纳管子集；`derive_status` 增加 `errors` 分支（纳管子集含 `error` 档）；`warnings` 分支条件调整为「纳管子集非空且无 `error` 档」；`passed` 判据追加「error 档 `apiFindings` 空」 |
| `crates/infra/archi/src/runner.rs`（`LocalArchiCheck`） | 门禁执行体 | 结论只取报告既有字段；`passed` 的口径变化经 `drift::check` 单点传导，执行体 MUST NOT 自行改写（本文件零逻辑改动，只随同源语义变化） |
| `crates/core/archi/src/drift/config.rs` | 看护配置 | 零改动：`ignore` / `exclude` 两键封闭 + `deny_unknown_fields`；求值顺序写死在 `check.rs`，MUST NOT 引入配置键 |
| `crates/core/archi/tests/golden_test.rs` + `corpus/drift/**` | 报告面语料 | 十六份金样随「首错短路 + 纳管分档」显式重录并逐份留痕成因；`DESKTOP_GOLDEN_REWRITE=1` 显式重写 |

### 模块增量：initialize 基线架构检查（init-check-archi）

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/archi/src/drift/mod.rs` | 判定单点（零逻辑改动） | `check(CheckInput) -> Result<Report, CheckError>`：`mode` 为入参、`changed` 全量模式忽略；`derive_status` 与模式无关；零写盘、零 spawn |
| `crates/core/archi/src/drift/report.rs` | 报告形状（零改动） | `ArchiDriftMode { Full, Incremental }`（`as_str` → `full` / `incremental`）、`ArchiDriftReport.mode` / `.change` / `.base` 字段面与 `version = 3` 不变 |
| `crates/core/archi/src/scan/**` | 扫描与归属（零改动） | 全量 / 增量共用同一遍历、排除矩阵与 `OwnerIndex` |
| `crates/infra/archi/src/runner.rs` | 落盘消费面 | port `ArchiCheckMode` → `ArchiDriftMode` 唯一映射；全量 / 增量同树落 `reports/archi/<timestamp>.json`（目录经 `foundation::layout::change_reports(root, name, ReportSlot::Archi)`） |
| `src-tauri/src/commands/archi/mod.rs` | 命令面（零改动） | `archi_drift(root, mode)` 两模式语义不变：`full` → 空变更集、进程内返回、零写盘 |

### 模块增量：资产哈希与引擎调用面（archi-rule-agent-prompt-sync）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/mod.rs` | 资产哈希（新增公开面） | `pub fn assets_hash(root: &Path) -> String`：覆盖面 = `graph/model/*.toml` + `models/**/*.md` + `models/**/*.schema.json` + `graph/INDEX.toml`（路径经 `foundation::layout` 派生）；稳定序「相对路径 + 字节」累加、`sha256:` 前缀；缺失目录 / 文件按不参与累加收敛、读失败按空字节参与；纯读盘、零写盘、零 spawn |
| `crates/core/archi/src/drift/mod.rs`（既有面零改动） | 判定单点 | `check(CheckInput) -> Result<Report, CheckError>` 与 `model_hash` 语义逐字不变；`modelHash` 仍只覆盖 `graph/model/*.toml`；新增调用面只改「谁调」，不改判定 |
| `crates/infra/agent/src/sdk/archi_drift_view.rs`（新，归属 infra） | 引擎侧投影单点 | `drift_summary` / `drift_entries` / `drift_files`：从同一 `Report` 逐级加字段；工具响应与收口 notice 共用 `summary`（notice 另加 `changedAssets`） |
| `crates/infra/agent/src/sdk/archi_tool.rs` / `sdk/preclose.rs` | 引擎两调用面 | 恒 `Full` + `changed: &[]` + `change: None` + `CancelFlag::never()`；零写盘（不落 `reports/archi/**`） |
| `crates/infra/archi/src/runner.rs` / `src-tauri/src/commands/archi/mod.rs` | 零改动 | 门禁 runner 两模式与命令面两模式语义逐字不变（本变更加的是引擎侧两处调用面，不减既有面） |

### Function: `crates/core/archi/src/drift/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `assets_hash` | 新增 | 四类资产聚合哈希；与 `model_hash` 共享累加 helper，覆盖面不同 |
| `model_hash` | 零改动 | 仍只覆盖 `graph/model/*.toml`；报告 `modelHash` 口径不变 |
| `check` / `derive_status` / `Report` | 零改动 | 判定、定级、`status` 推导与报告字段面逐字不变 |

### API: 引擎两调用面

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_drift` 工具分支 | 新增（详见 `desktop-archi-agent`） | `scope` 三值；恒全量；`cap_result` 收口 |
| `preclose`（`BeforeDoneHook` 实现） | 新增（详见 `desktop-archi-agent`） | 自然收敛前一次全量判定；装配由 `SessionInjections.before_done = ArchiPreclose` 档位表达；失败面 `archi_preclose_skipped` |

### 模块增量：依赖 API 两桶对账（relation-api-dependency-check）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/scan/mod.rs` | 扫描事实面（名字通道） | `ImportFact` / `DependencyEdge` 各增 `names: Vec<String>`（去重 + 字典序单点；空 = 取不到名字）；既有字段与归属 / 排除 / 定级语义逐字不变；`gate = off` 分支零改动 |
| `crates/core/archi/src/scan/rust.rs` | Rust 取数 | `use` 树同时产出「路径」（供前缀匹配）与「尾段」（供名字）；`Rename` 取被引入名、`Glob` / `self` 不产名；`resolve` 签名与语义零改动（名字不走前缀匹配） |
| `crates/core/archi/src/scan/ts.rs` | TS / JS 取数 | `ImportDeclaration` 的具名 specifier 与 `ExportFromDeclaration` 的具名清单产名（`type` 前缀剥除）；default / namespace / `export *` 不产名；specifier 解析与 `unresolved` 面零改动 |
| `crates/core/archi/src/drift/mod.rs` | 两桶对账 + 求差单点 | `pub fn api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>)`；对账后按关系 `apis` 非空产出两桶（`used ∖ declared` 按 `from.gate` 落 `errors` / `warnings`，`declared ∖ used` 落 `observations`），`message` 并列两种成因；既有三类漂移与豁免 / `status` 推导零改动 |
| `crates/core/archi/src/drift/report.rs` | 报告结构与词汇 | `ArchiDriftKind` 增 `UndeclaredApi`（`ALL: [Self; 5]` + `as_str` 线格式 `undeclaredApi`）；`ArchiObservationKind::UnusedDeclaration` 注释由「本期不产出」改为「`declared ∖ used` 时产出」（`ALL` 仍二值）；`ArchiDriftReport` 字段集与 `REPORT_VERSION = 3` 逐字不变 |
| `crates/core/archi/src/drift/report_test.rs` | 报告面测试 | `ArchiDriftKind::ALL.len()` 4 → 5、`as_str` 逐值对拍随动；新增「`unusedDeclaration` 不进 `status` / `passed`」与「条目键集不变」用例 |
| `crates/core/archi/tests/corpus/drift/**` | 报告面语料 | 16 份既有金样零变化；**新增 case**（`api-declared-exact` 双桶皆空 / `api-used-not-declared` 缺一项 / `api-declared-not-used` 多一项 / `api-absent` 未填 `apis` 零噪声），各自登记进 `DRIFT_CASES`；golden 只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 |
| `crates/core/archi/src/relation.rs`（新，消费面） | 求差第二消费面 | `relation_detail` 调 `api_buckets`（MUST NOT 自写集合差）；本能力只出「求差口径与不缓存」这两条约束，DTO / 命令形状归 `desktop-archi-model` |

### Function: `crates/core/archi/src/drift/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `api_buckets` | 新增 | 纯函数求差，两侧去重 + 字典序；`used ∖ declared` / `declared ∖ used` |
| `check` | 改 | 对账后追加两桶装配（仅对 `apis` 非空的关系）；其余判定 / 豁免 / `status` 推导逐字不变 |
| `derive_status` / `Report` / `passed` | 零改动 | 结论面不吃 `unusedDeclaration`；`undeclaredApi` 只经 `errors` / `warnings` 既有通道 |

### API: 词汇与报告

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiDriftKind` | 4 → 5 值 | 新增 `UndeclaredApi`（线格式 `undeclaredApi`）；`ALL` 长度断言随动 |
| `ArchiObservationKind` | 零扩张（二值） | `UnusedDeclaration` 由空位转产出；值集与 `ALL` 不变 |
| `REPORT_VERSION` | 保持 3 | 字段面零增删（名字进 `message`），不足以为版本递增提供理由 |
