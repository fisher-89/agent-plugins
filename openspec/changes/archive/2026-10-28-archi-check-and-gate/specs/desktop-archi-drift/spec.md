# desktop-archi-drift Specification（delta: archi-check-and-gate）

## ADDED Requirements

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

### Requirement: 三类漂移与定级只看引入方

检查 SHALL 判定三类漂移，并 SHALL 把「模型自身不自洽的结构错」作为独立门禁判据：

| # | 漂移 | 定义 | 产出位置 |
|---|---|---|---|
| ① | 声明未兑现 | 模型声明 `A -> B`，代码中无任何 `A -> B` 的 import | `observations[]`（永不进 `status` / `passed`） |
| ② | 未声明依赖 | 代码存在跨元素 import，模型未声明该关系 | `errors[]` / `warnings[]`（按引入方 `gate` 档） |
| ④ | path 失效 | 元素 `path` 指向不存在的目录 / 文件 | `model.warnings[]` + `warnings[]` / `errors[]`（按该元素 `gate` 档） |

- **② 的定级只看引入方**：`A` 引入 `B` 且模型无 `A -> B` 时级别 = `A` 的 `gate` 档（`strict` → `errors`、`warn` → `warnings`、`off` → 不出现）。`B` 的档位 MUST NOT 影响本条——否则一个 `strict` 元素会把全仓库 `off` 元素拽进门禁。
- **层级逆向并入 ②**：kind 层级是**包含**层级不是约束层级，「允许的依赖方向」无权威来源；「声明了就合法」是唯一自洽规则。`A -> anc(A)` 一类图结构自相矛盾由结构校验判（`desktop-archi-model`），MUST NOT 在此另立方向规则。
- **type-only 算架构依赖**：`import type` 与普通 import 同权（只记 `typeOnly: true`）；MUST NOT 因类型导入而豁免。
- **目标未建模**：跨元素 import 的 `to` 元素名不在模型内时 SHALL 记 warning（归因不明），MUST NOT 记 error。
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
  落盘 = 否（一次可重算的观测，落盘只会积累垃圾）
  用途 = 架构全景 / ratchet 决策 / 一次性体检

增量模式（change 级）
  基准 = 本 change 编辑集（`ArchiveVcsPort::change_edit_set(root, "HEAD")`，worktree 语义下即本 change 改动；唯一入口，MUST NOT 自算 diff）
  扫描 = 只扫变更集内的源文件
  产出 = ②④；① 无基准 → 只作观察项
  落盘 = 是（`reports/archi/<timestamp>.json`，门禁审计面）
```

- **观察根显式**：命令签名 SHALL 带 `root`，由调用方给定（workspace 级 = 主仓根、change 级 = `detail.worktree`，归档 change 回落主仓根）；MUST NOT 引入「当前选中 change」一类隐式上下文。
- **模型与源码同根**：模型 SHALL 从同一 `root` 派生读盘（架构域路径经 `foundation::layout`），MUST NOT 出现「模型读主仓、源码读 worktree」的混根。
- **一致性**：同一 commit、同一模型下，增量报告 SHALL 为全量报告在变更集上的子集（同一元素对不因模式不同而级别不同）。
- **零改代码面**：两种模式 MUST NOT 写源码或模型（唯一写动作是增量模式的报告落盘）。

#### Scenario: 增量只覆盖变更文件

- **WHEN** change 编辑集含 12 个源文件（其中 5 个被排除约定排除），执行增量检查
- **THEN** `scan.changedFiles = 12`、`scan.scannedFiles = 5`、`scan.excluded[]` 含那 5 条；未变更文件的违例不出现

#### Scenario: 两模式级别一致

- **WHEN** 同一 commit 上分别跑全量与增量，且两者都命中了同一元素对 `App.Billing → App.Auth`
- **THEN** 两报告中该元素对级别相同（同为 `errors` 或同为 `warnings`），仅 `mode` / `changedFiles` / `base` 与扫描清单不同

#### Scenario: 无变更文件不算通过

- **WHEN** 增量模式下变更集内无源文件（如只改了文档）
- **THEN** 报告 `status = noChanges`（MUST NOT 为 `clean`），扫描计数为零、`errors` 为空

### Requirement: 报告结构与 status 五值

检查 SHALL 产出单一报告结构（增量模式落 `reports/archi/<timestamp>.json`，全量模式仅进程内返回），字段面 SHALL 覆盖：

```jsonc
{
  "version": 1,
  "timestamp": "2026-10-20T08:31:12Z",
  "mode": "incremental",              // incremental | full
  "root": "/abs/path",
  "change": "add-billing-gate",       // 无 change 上下文（full 模式 / 命令面无 change）时为 null
  "base": "HEAD",                     // full 模式为 null（增量恒 HEAD 差，见「全量与增量两模式」）
  "modelHash": "sha256:…",            // models/*.toml 聚合哈希

  "model": {
    "elements": 24, "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [ … ],        // 读存量模型遇错也照录
    "warnings": [ … ]                 // pathNotFound / noPath / docNotFound
  },
  "scan": { "mode": …, "changedFiles": 17, "scannedFiles": 12,
            "excluded": [ … ], "unmatchedFiles": [ … ], "matched": [ … ],
            "unresolved": [ … ] },
  "errors":   [ { "kind": "undeclaredDependency", "from": …, "to": …, "gate": …, "occurrences": [ … ] } ],
  "warnings": [ … ],
  "suppressed": [ { "from": …, "to": …, "via": "config|inline", "reason": …, "file": …, "line": … } ],
  "observations": [ { "kind": "unfulfilledDeclaration", "from": …, "to": …, "message": … } ],
  "status": "errors"                  // clean | errors | warnings | noChanges | skipped
}
```

- **`status` 五值封闭**：`clean`（有扫描、零 errors 零 warnings）、`errors`（`errors` 非空）、`warnings`（零 errors、`warnings` 非空）、`noChanges`（无文件可判）、`skipped`（无模型）。`skipped` 与 `noChanges` MUST NOT 被呈现或消费为「通过」——UI 与门禁日志 SHALL 显式区分，否则「没检查」被误读成「检查过了没问题」。
- **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向 / 目标 / 可解析性 / 归属不确定），随 `status` 影响提示强度；`observations` 是「设计态陈述」（声明未兑现 / 长期未使用），永不影响 `status`、永不进 `passed`。
- **`occurrences` 聚合**：同元素对的全部命中点聚合在一条记录内（按 `(from, to)` 聚合，逐条带 `file` / `line` / `specifier` / `typeOnly`），不按行铺开。
- **`suppressed` 必留痕**：豁免不等于不存在——保留被抑制条目 + 理由 + 来源，豁免了什么才可审计。
- **`modelHash`**：`models/*.toml` 聚合哈希，供消费方判断「手上报告是否对应当前模型」与历史报告追溯；MUST NOT 被用作缓存失效依据（本期零缓存）。
- **词汇封闭且单点**：报告自带三组封闭词汇——`status`（五值）、漂移条目 `kind`（`undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath`）、观察项 `kind`（`unfulfilledDeclaration` / `unusedDeclaration`）；每组 SHALL 在报告结构模块单点定义并附 `ALL` 常量，语料对拍断言「新增变体未同步即红」。报告词汇 MUST NOT 复用既有 DTO 词汇：`desktop-archi-model` 的 `DiagnosticKind`（结构诊断九类）、`EdgeMark` / `NodeMark`（画布标记）与 `WarningKind`（文档警告，单值 `docNotFound`）语义面各不相同——混用会让「结构错」「文档警告」「漂移违例」三类在类型上不可辨。
- **`version`**：报告结构版本号，字段演进 SHALL 递增并走显式兼容说明。

#### Scenario: 五值可辨

- **WHEN** 分别以「零模型」「无变更文件」「零违例」「仅 warn 档违例」「存在 strict 档违例」构造五次检查
- **THEN** `status` 依次为 `skipped` / `noChanges` / `clean` / `warnings` / `errors`；`skipped` 与 `noChanges` 的报告 `errors` 均为空且不带任何「通过」语义字段

#### Scenario: 观察项不影响结论

- **WHEN** 报告含 3 条 `observations` 与零 `errors`
- **THEN** `status = clean`（或 `warnings` 仅当其 `warnings` 非空）、`passed = true`；`observations` 不进门禁判据

#### Scenario: 报告词汇与 DTO 词汇互不混用

- **WHEN** 审查报告 `kind` 词汇与 `desktop-archi-model` 的 `DiagnosticKind` / `WarningKind` / `EdgeMark` / `NodeMark`
- **THEN** 四组词汇各自单点定义、互不复用；报告结构模块为漂移条目与观察项各持一份 `ALL` 常量，语料对拍覆盖全部取值（新增值未同步即红）

#### Scenario: modelHash 随模型变化

- **WHEN** 修改任一 `models/*.toml` 字节后重跑
- **THEN** `modelHash` 变化；未修改模型时两次运行的 `modelHash` 相同

### Requirement: 架构域看护配置（豁免与扫描排除）

架构域配置 SHALL 落 `<archi_dir>/config.json`（路径经 `foundation::layout` 派生，MUST NOT 行内字面量），字段面封闭为两项：

```jsonc
{
  "ignore":  [ { "from": "App.Billing", "to": "App.Legacy", "reason": "迁移期临时直连，见 ADR-0007" } ],
  "exclude": [ "src/legacy/**" ]
}
```

- **归属面写死**：工作区 `openspec/config.json` 管**执行**（命令 / scope / 阈值 / 保护），本文件管**看护策略**（豁免 / 扫描排除）；MUST NOT 出现第三个配置文件，MUST NOT 把 `architecture` 键塞进工作区 `config.json`（其 `KNOWN_KEYS` 七项零变化）。
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

架构漂移的判定 SHALL 只有一处实现（`crates/core/archi` 的漂移判定纯函数：输入 = 观察根 + 模式 + 变更集 + 配置 + 模型，输出 = 报告）。IPC 命令面（`archi_drift(root, mode)`）与相位门禁 runner SHALL 都调用同一实现，MUST NOT 各写一份归属 / 定级 / 状态推导；报告结构体 SHALL 为两消费面共用的唯一形状（命令面出线经同一 DTO 投影，门禁面从同一结构派生 `passed` / 诊断文本）。

#### Scenario: 命令面与门禁面逐字段同源

- **WHEN** 以同一 root、同一模式（增量）与同一变更集分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告除 `timestamp` / `change`（命令面无 change 上下文）与落盘路径外逐字段相等（`model` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `status` 全等）

#### Scenario: 零第二套判定

- **WHEN** 审查命令面与 infra runner 的源码
- **THEN** 两者均只做「取输入 → 调判定纯函数 → 投影 / 落盘」，无归属匹配、无定级分支、无 status 推导的第二份实现

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/scan/mod.rs` | 扫描编排 | `scan(root, mode, changed, config) -> ScanOutcome`：遍历 + 排除 + 归属 + 逐文件依赖收集；纯读盘、零写盘、零 spawn |
| `crates/core/archi/src/scan/exclude.rs` | 排除矩阵 | 默认约定常量 + 配置 `exclude` 覆盖匹配（globset）；产出 `excluded[{ file, rule }]`；与工作区 `tests[].excludes` 零耦合 |
| `crates/core/archi/src/scan/ts.rs` | TS / JS 提取 | 静态 `import` / `export … from` 的 specifier + `typeOnly` + 行号；tsconfig / jsconfig `baseUrl` + `paths` 解析；不可判定 → `unresolved` |
| `crates/core/archi/src/scan/rust.rs` | Rust 提取 | workspace 清单解析（`Cargo.toml` + member glob，零子进程）→ crate 清单；模块树（`mod` 外部 / 内联 / `#[path]` / `cfg` 两臂）；`use` 树最长前缀匹配；孤儿与宏生成 → `unresolved` |
| `crates/core/archi/src/scan/owner.rs` | 归属单点 | `owner_of(path) -> Option<fqn>`：元素 `path` 最长前缀；同长多命中 → `ambiguousOwner` + unmatched；`off` 元素覆盖 → unmatched |
| `crates/core/archi/src/drift/mod.rs` | 判定单点 | `check(CheckInput) -> Report`：事实 vs 声明对账、三档定级、豁免抑制、`status` 推导；命令面与门禁面唯一入口 |
| `crates/core/archi/src/drift/report.rs` | 报告结构 | `Report`（`version` / `mode` / `model` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `status`）+ `serde_json` 序列化 + `ArchiCheckStatus` 五值封闭 |
| `crates/core/archi/src/drift/config.rs` | 看护配置 | `load_config(root) -> Result<CheckConfig, String>`：读 `<archi_dir>/config.json`（经 `foundation::layout` 派生）；缺失即空；`deny_unknown_fields`；`ignore` 元素对 + reason 必填 + FQN 存在性校验 |
| `crates/core/archi/Cargo.toml` | 依赖扩展 | 新增外部依赖 `oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn`（+ `globset` 提为直接依赖）；仍零 Tauri / 零 store / 零 spawn / 零 `workflow` / 零 `config` 依赖 |
| `src-tauri/src/commands/archi/mod.rs`（`archi_drift`） | 命令面薄包装 | `archi_drift(root: String, mode: ArchiDriftMode) -> Result<ArchiDriftReport, String>`（`ArchiDriftMode` = `full` \| `incremental`，serde 小写词、specta `Type` 出线）：增量模式经 vcs 缝取 `change_edit_set(root, "HEAD")` 后调判定纯函数；全量模式直接调；命令层零判定规则 |
| `crates/core/archi/tests/` | 语料与预算 | `corpus/<case>/` + `expected/<case>.json` + `golden_test.rs` + Rust 模块树语料集 + `perf.rs`（release p95）；`tests/README.md` 记实测表 |
