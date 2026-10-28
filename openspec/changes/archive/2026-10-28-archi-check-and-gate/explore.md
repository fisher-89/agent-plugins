# 代码检查 · 漂移标记 · 相位门禁
workflow_type: requirement

---

## 0. 范围

这个 explore 管**判定与拦截**：扫哪些文件、怎么把文件归属到元素、什么算漂移、什么时候 fail 流程、报告长什么样、依赖成本多少。

模型怎么存、画布怎么画、模块文档怎么关联、agent 怎么用模型，归另一个 explore（`archi-graph-doc-agent`）。接缝只有一个：**本 explore 消费它的 `ModelGraph`**。

```
  archi-graph-doc-agent                  archi-check-and-gate（本 explore）
  ─────────────────────                  ──────────────────────────────
  ModelGraph（元素 / 关系 /      ──▶      ① 扫源码提取 import / use
   path 映射 / gate 档）                  ② 归属到元素
                                          ③ 与声明对账 → 漂移
                                          ④ 定级 → 门禁结论 / 报告
                                                    │
  UI 画布 ←────── 漂移标记（本 explore 定义何时产生，画布只管呈现）──┘
```

不重定义模型语义（kind / gate / path 的形状都在另一个 explore），只消费。

---

## 1. 扫描范围与豁免

### 1.1 排除测试文件

架构扫描 MUST 排除所有测试文件与测试夹具。**不复用 `config.json` 的 `tests[].excludes`**——那是测试执行 scope（可因 e2e 慢而排除、因 fixture 需要而包含），与架构扫描范围正交。

```
默认约定（可被 architecture/config.json 的 exclude 覆盖）
  **/*.test.{ts,tsx,js,jsx}   **/*.spec.{ts,tsx,js,jsx}   **/__tests__/**
  **/*_test.rs                **/tests/**                 **/fixtures/**   **/__mocks__/**
```

理由不止噪声：测试文件天然跨元素引用被测代码，纳入扫描会让每个 `foo.test.ts` 对 `foo.ts` 产生一条未声明依赖——这不是架构问题，是测试的固有形状。

### 1.2 豁免两层，第一层零成本

```
第 1 层  声明即豁免（语义层，零配置）
         模型里写了 A -> B，这条边即合法。所以「这条依赖是对的」的正确
         动作是补声明，不是加豁免——漂移② 的定义天然是「未声明的才是问题」。

第 2 层  忽略规则（噪声层，仅用于「不能 / 不该声明」）
         内联  // archi-ignore-next-line: 迁移期临时直连，见 ADR-0007
               → 用 import 声明的 span 行号回原文取该行 / 上一行做注释匹配
                 （不读 AST 的 comment trivia，最稳）
         配置  architecture/config.json 的 ignore: [{ from, to, reason }]
               → 结构性、可审计、不进业务代码；reason 必填
```

第 2 层 MUST NOT 支持 glob 粒度（如 `src/legacy/**`）——glob 豁免等于永久后门，ratchet 就白做了。只允许 `(from 元素, to 元素)` 元素对。

### 1.3 配置文件边界

```
openspec/config.json                   既有，桌面 config crate 唯一出口
├── schema / context / rules           相位 planner 的自然语言规则
├── static_analysis                    静态检查命令
├── tests[]                            测试执行 scope
└── write_protection                   写保护

.dev-team/architecture/config.json      新增，架构域自有
├── ignore: [{ from, to, reason }]     元素对豁免（reason 必填）
└── exclude?: [glob]                   扫描排除覆盖（缺省用 §1.1 约定）
```

拆两个文件而不是把 `architecture` 键塞进 `config.json`：`config.json` 的 `KNOWN_KEYS` 是七项白名单，加键要同步改 `KNOWN_KEYS` / `WorkspaceConfig` / 解析分支 / 前端 config 页 / 两份 schema；而架构域配置与「测试怎么跑、流程怎么流」无关，属另一关切面。

管辖面写死：`config.json` 管**执行**（命令、scope、阈值、保护），`architecture/config.json` 管**看护策略**（豁免、扫描排除）。MUST NOT 出现第三个配置文件。

---

## 2. 漂移分类与定级

### 2.1 三类漂移

| # | 漂移 | 定义 | 判定层 | 级别 |
|---|---|---|---|---|
| ① | 声明未兑现 | 模型声明 A→B，代码中无任何 A→B 的 import | 全量可判定 / 增量只观察 | 不进 `status`（→ `observations`） |
| ② | 未声明依赖 | 代码存在跨元素 import，模型未声明该关系 | 门禁 | 按引入方 gate 档 |
| ④ | path 失效 | 元素 `path` 指向不存在的目录 | 结构校验 + 门禁 | 按该元素 gate 档 |

层级逆向（如 `component → package`）**并入 ②**：C4 的 kind 是包含层级不是约束层级，「允许的依赖方向」没有权威来源；「声明了就合法」是唯一自洽规则。例外保留在结构校验（另一个 explore）：`A -> anc(A)` 是图结构自相矛盾，零规则来源即可判定。

### 2.2 定级只看引入方

```
② 未声明依赖：A 引入 B，模型无 A->B
     级别 = A 的 gate 档        ← 谁引入谁负责
     B 的档位不影响本条

为什么  否则一个 strict 元素会把全仓库的 off 元素拽进门禁
```

### 2.3 结构错（不属漂移，但门禁也判）

「结构错」不是漂移类别——它不属「模型与代码不一致」，而是「模型自身不自洽」。判定分工已在另一个 explore 厘清：

```
parser / serde 免费（含 line/column + caret）
├── syntaxError / missingField / invalidValue / unknownField / duplicateElement
必须自实现（petgraph + 规则，共 3 类）
├── cycle / dependsOnAncestor / unknownReference
```

**门禁一并判**：`structuralErrors` 非空即 fail。三条策略各管一段（互不冲突）：

```
读取     照常加载，返回结构化 errors（绝不 panic）
渲染     停摆，显示错误面板（绝不画畸形图）
门禁     判 structuralErrors → fail（本 explore）
```

---

## 3. 全量 / 增量两模式

```
全量模式（workspace 级，主仓根）
  基准 = 无（当前工作区就是全部）
  扫描 = 整个工作区的源文件（ignore 遍历 + §1.1 排除）
  产出 = ① ② ④ 全部
  成本 = 与仓库规模线性（本仓约 250 源文件 → 亚秒级）
  用途 = 架构全景 / ratchet 决策 / 一次性体检
  落盘 = 否（一次可重算的观测，落盘只会积累垃圾）

增量模式（change 级，worktree 根）
  基准 = change 的 base_commit（change_edit_set）
  扫描 = 只扫本 change 变更的文件
  产出 = ② ④；① 无基准 → 只作观察项
  成本 = 与 change 规模相关，通常几十毫秒
  用途 = 相位门禁 / 该 change 的模型实况
  落盘 = 是（reports/archi/，门禁审计面）
```

**观察根就是「当前工作根」**（模型与源码同等资产）：workspace 级读主仓根、change 级读 `detail.worktree`（归档 change 回落主仓根，与会话 cwd 同口径）。命令签名一律带 `root`，不引入「当前选中 change」这类隐式上下文。

**性能分界**：结构校验与文档关联只读模型与目录列表，永远毫秒级；漂移计算要解析源码，与规模线性。因此「打开页面看到图」永远快，「看到漂移」才可能慢。

| 源文件数 | 全量漂移耗时 | 策略 |
|---|---|---|
| ≤ 500 | < 1 s | 页面打开即算 |
| 500 ~ 5 000 | 1 ~ 10 s | 先出图，漂移按钮显式触发 |
| > 5 000 | > 10 s | 同上 + 提示可改用增量模式 |

---

## 4. 门禁挂载

### 4.1 只挂 implement

```
requirement 表（10 相位）
initialize     PhaseStart → Clean → StaticCheck(Terminal) → TestExecution(Terminal) → PhaseLog
proposal       PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog
dev-design     …（模型在这里被改）
test-design    …
implement      PhaseStart → Executor → StaticCheck(FeedbackLoop{5})
               → ArchiCheck(FeedbackLoop{3}) → Evaluator → VerdictGate(FeedbackLoop{5}) → PhaseLog
test-gen       …
test-execution …
code-review    …（无 executor，仅 evaluator）
acceptance     …
archive        Preflight → Commit → Rebase → ResolveConflicts → Merge
               → SpecSync → Cleanup → Seal → Finalize

test-only 表（7 相位）：initialize → proposal → code-analyze → test-design
                        → test-gen → test-execution → archive
```

* **acceptance 不挂**：模型一致性已在 implement 收敛；acceptance 关注验收行为，架构问题转新 change 验收。
* **test-only 不挂**：该表无 implement 相位，代码改动在 code-analyze（`CODE_ANALYZE_STEPS` = Executor → Evaluator → VerdictGate(Terminal) → PhaseLog，无工具步）；而测试文件被扫描全排除，必然 clean/skipped。挂上只会得到一个永远空转的步骤。

### 4.2 用 `FeedbackLoop` 而非 `Escalate`

② 的成因有两种且无法自动区分：

```
代码写了不该写的依赖      → 改代码（executor 的活）
模型声明落后于真实设计    → 改模型（该走 dev-design 回溯）
```

缺省权威是模型（声明式驱动的本意），所以第一反应应该是「让代码符合模型」：executor 拿到具体违例（哪个文件 import 了哪个元素）去改。

而 `FeedbackLoop` 在超限时**本来就升格相位失败**（`walker.rs` 的 `step_static_check`：`feedback >= limit` → 代写 fail phase_log → `StepFlow::Upgraded` → 回 `phase_next` 进决策分叉），所以「先修代码、修不动就交决策裁决是否改模型」这条路径**不需要新 `OnFail` 变体**，`FeedbackLoop { limit: 3 }` 自带。

### 4.3 变体追加纪律

| 枚举 | 位置 | 编码 | 追加方式 |
|---|---|---|---|
| `RunStepKind`（21 值） | `workflow/state.rs` | bincode 枚举下标落库 | **只能尾部追加**（源码有明文警告） |
| `StepNode`（21 值） | `workflow/state.rs` | 仅相位表内存结构，不落库 | 位置自由，建议尾部追加保持一致 |
| `StepKind`（审计八臂） | `workflow/state.rs` | 落 `StepRecord`（`native_model` id=12） | 尾部追加 + `StepRecord` 走版本链 |
| `ToolStepOutput` | `orchestration/port.rs` | 进程内封闭集 | 尾部追加 |
| `ReportSlot` | `foundation/layout` | 纯进程内枚举（零 serde / 零 specta） | 加第三值 `Archi` |

### 4.4 报告槽与目录常量

`ReportSlot` 是纯进程内枚举（`Test` / `Baseline`），加 `Archi` → `reports/archi`。**不复用 `Test` 槽靠文件名前缀区分**——`reports/test` 的 `plans[]` 索引语义、覆盖率复用判定、mutation scope 都挂在槽位上，混入架构报告会让三类语义互相污染。加第三槽的成本只是一个 match 臂。

目录派生落在 `foundation::layout`（另四个常量在模型侧 explore）：

```
archi_reports_dir(root)    → <archi_dir>/reports      （全局兜底位置）
```

change 内的报告仍走既有的 `change_reports(root, change, ReportSlot::Archi)`，与其它槽位同一条派生路径——不新造第二套报告定位。

### 4.5 门禁协议（orchestration::port）

照 `StaticCheckRunner` / `TestExecutionRunner` / `MutationRunner` 三兄弟的同型形态加第四个：

```rust
pub trait ArchiCheckRunner: Send + Sync {
    /// root = 被检查根（worktree 或主仓根）
    /// name = change 磁盘名（报告目录派生用）
    /// slot = 报告槽位
    /// cancel = 取消信号（置位 → Err("执行已取消")）
    fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture;
}
```

产出走既有的 `ToolStepOutput` 封闭集（新增 `ArchiCheck` 变体），`workflow` 侧零 `archi` 依赖。

---

## 5. 报告结构

只有增量模式落盘：`reports/archi/<timestamp>.json`（门禁审计面）。两种模式共用同一结构体；`ArchiCheckOutcome` 在进程内从它派生 `passed` / `diagnostics`。

```jsonc
{
  "version": 1,
  "timestamp": "2026-10-20T08:31:12Z",
  "mode": "incremental",                    // incremental | full
  "root": "/abs/path/to/worktree",
  "change": "add-billing-gate",             // full 模式为 null
  "base": "a1b2c3d",                        // full 模式为 null
  "modelHash": "sha256:…",                  // models/*.toml 聚合哈希

  "model": {
    "elements": 24,
    "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [                   // 读存量模型遇错也照录
      { "kind": "dependsOnAncestor", "element": "App.Auth.Token",
        "ancestor": "App.Auth", "message": "元素依赖自己的祖先" }
    ],
    "warnings": [
      { "kind": "pathNotFound", "element": "App.Billing", "path": "./src/billing/",
        "gate": "strict", "message": "metadata.path 指向不存在的目录" },
      { "kind": "noPath", "element": "App.Legacy", "gate": "off",
        "message": "元素未声明 path（不参与归属）" },
      { "kind": "docNotFound", "element": "App.Auth", "doc": "./docs/auth.md",
        "message": "docs 指向的文件不存在" }
    ]
  },

  "scan": {
    "mode": "incremental",
    "changedFiles": 17,                     // full 模式为 null
    "scannedFiles": 12,
    "excluded": [ { "file": "src/a.test.ts", "rule": "**/*.test.ts" } ],
    "unmatchedFiles": ["docs/notes.md"],     // 无元素认领（off 元素覆盖的文件亦在此）
    "matched": [ { "element": "App.Billing", "files": ["src/billing/x.ts"] } ],
    "unresolved": [                          // 解析不到的 specifier（不冒充违例）
      { "file": "src/billing/x.ts", "specifier": "@weird/alias", "message": "路径不可解析" }
    ]
  },

  "errors": [                                // 门禁唯一定级依据：非空即 fail
    {
      "kind": "undeclaredDependency",
      "from": "App.Billing", "to": "App.Auth",
      "gate": "strict",                      // 引入方档位
      "occurrences": [                       // 按 (from,to) 聚合，不按行铺开
        { "file": "src/billing/x.ts", "line": 3, "specifier": "../auth/token", "typeOnly": false },
        { "file": "src/billing/y.ts", "line": 1, "specifier": "../auth/token", "typeOnly": true }
      ]
    }
  ],

  "warnings": [ … ],                         // 同 errors 结构（gate=warn 的②）+ 未声明目标 + 解析失败
  "suppressed": [                            // 被豁免抑制的违例也留痕
    { "from": "App.Billing", "to": "App.Legacy", "via": "config",
      "reason": "迁移期临时直连，见 ADR-0007" },
    { "from": "App.Billing", "to": "App.Auth", "via": "inline",
      "file": "src/billing/x.ts", "line": 2, "reason": "存档迁移，见 ADR-0011" }
  ],

  "observations": [                          // 永不进 status / passed
    { "kind": "unfulfilledDeclaration",      // 漂移①（full 模式产出）
      "from": "App.Billing", "to": "App.Auth",
      "message": "模型声明了依赖，但代码中未发现任何 import" },
    { "kind": "unusedDeclaration",
      "from": "App.Shared", "to": "App.Legacy",
      "message": "模型声明的依赖长期未被使用（候选删除）" }
  ],

  "status": "errors"                         // clean | errors | warnings | noChanges | skipped
}
```

几条设计要点：

* **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向、目标、可解析性），随 `status` 影响 UI 提示强度；`observations` 是「设计态陈述」，永不影响 `status`、永不进 `passed`。应对方式根本不同：前者要修代码，后者要人判断「这是计划还是欠账」。
* **`status` 五值**：`skipped`（无模型）与 `noChanges`（无变更文件）不是「通过」，要在 UI 与门禁日志里显式区分——否则「没检查」会被误读成「检查过了没问题」。
* **`occurrences` 聚合**：按 `(from, to)` 聚合而非按行铺开，因为 FeedbackLoop 的 prompt 有长度上限（`MAX_REPORT_CHARS` 同源先例）。给 executor 看「Billing 依赖了 Auth，共 2 处，在 x.ts:3 / y.ts:1」，不是 40 行流水账。
* **`suppressed` 必留痕**：豁免不等于不存在。保留被抑制条目 + 理由 + 来源（`config` / `inline`），「豁免了什么」才可审计，视图也能把豁免渲染成灰色边。
* **`modelHash`**：零持久化下没有缓存要失效；它的用途是让前端判断「上次看的报告是不是当前模型生成的」，让历史报告可追溯到具体模型版本。

`ArchiCheckOutcome`：

```rust
pub struct ArchiCheckOutcome {
    pub passed: bool,               // errors.is_empty() && structuralErrors.is_empty()
    pub diagnostics: String,        // errors + structuralErrors 渲染（截断到 MAX_REPORT_CHARS 量级）
    pub fault: Option<RunFault>,    // 恒 None（无进程 spawn，无环境错误面）；保留以与 StaticCheckOutcome 同形
    pub report_dir: String,         // reports/archi
    pub report_ref: Option<String>, // 报告文件路径（审计回查）
}
```

`fault` 恒 `None`：检查不 spawn 任何进程（`oxc_resolver` 是纯内存解析），没有「命令挂死 / 文件锁」这类环境错误面。保留字段是为了与 `StaticCheckOutcome` 同形，让 `walker` 的分派代码不必为第四个 runner 写特例。

---

## 6. 技术选型

### 6.1 依赖清单（锁文件实际状态）

| 需要什么 | 库 | 锁内状态 |
|---|---|---|
| 文件遍历 + gitignore | `ignore` 0.4.33（`WalkBuilder`） | 传递（经 tokei） |
| glob 匹配 | `globset` 0.4.20 / `glob` 0.3.4 | 传递 / 已是直接依赖（`checks::globmatch`） |
| 图算法 | `petgraph` 0.8.3（SCC / 拓扑排序） | 传递 |
| Rust crate 依赖 | `cargo_metadata` 0.19.2 | 传递（经 tauri-utils） |
| Rust 文件级 `use` | `syn` 2.x（`full` + `parse_file`） | 传递（各 derive 宏） |
| TS/JS 语法树 | `oxc_parser` + `oxc_allocator` | **真新增** |
| TS/JS 模块解析 | `oxc_resolver`（tsconfig paths / node 解析 / exports） | **真新增** |
| 模型解析 | `toml` + `toml_edit`（另一个 explore 定） | 已在锁 |

`syn` / `cargo_metadata` 在锁内以 proc-macro / build-script 身份出现（host 编译），提为普通依赖会给 target 编译增加时间。`oxc_*` 是唯一实打实的体积新增，成本由 §7 实测量化。

**明确不用**（会破坏「不依赖本地环境、开箱即用」）：

```
madge / dependency-cruiser / ts-morph   → 要 node
cargo-modules / cargo-depgraph          → 要 cargo install 二进制
  （cargo-modules 的模块级依赖图与环检测最贴需求，可作「检测到则增强」的可选路径，
    但不能是必需依赖）
```

### 6.2 边界：库负责解析，我们负责归属

```
开源库给的（不要手搓）
├─ 词法 / 语法 → import / use 声明 + span          oxc_parser / syn
├─ 模块解析 → specifier 落到磁盘绝对路径           oxc_resolver
├─ crate 图 → package 名 / 版本 / 目录              cargo_metadata
├─ 图算法 → SCC / 拓扑序 / 邻接                    petgraph
└─ 文件枚举 → 遍历 + gitignore + glob              ignore / globset

只能自己写的（任何库都给不了）
├─ 「这个文件属于哪个 C4 元素」      ← path 语义是我们发明的
├─ 「这条 import 是否跨元素边界」    ← 需要 path 前缀归属 + 元素内豁免
└─ 「这个元素现在开没开门禁」        ← gate 档 ratchet 策略
```

`oxc_resolver` 使用口径：按 `config.json` 的 `tests[].root` 逐子树加载 tsconfig（`baseUrl` + `paths`），对每个 import specifier 调用一次 `resolve`。解析失败记 warning「路径不可解析」进 `scan.unresolved`，不进门禁（看不见的依赖不该冒充违例）。

### 6.3 Rust 侧的归属：必须深入到 crate 内部

crate 对应 JS 的 package——这个对应在**元素粒度**上成立（一个 element 可映射一个 crate 目录），但在**依赖解析**上不成立：

```
JS     import './foo'          → 直接得到文件路径（文件即模块）
Rust   use crate::foo::bar     → 得到模块路径，文件位置要靠模块树反查
```

所以 Rust 侧多一步「建模块树」：

```
① crate 清单            cargo_metadata
     → 每个 workspace member 的 manifest 路径 / crate 名 / lib.rs 或 main.rs 位置
        │
② 建模块树（每 crate 各建一棵）
     syn::parse_file 所有 .rs，读 `mod x;`（外部文件）/ `mod x { … }`（内联）
     → 模块路径 → 文件路径 映射表
        crate::foo::bar  →  src/foo/bar.rs 或 src/foo.rs（含 mod bar）
        crate::foo       →  src/foo/mod.rs 或 src/foo.rs
        `#[path = "…"]` 优先（覆盖默认推导）
        │
③ 解析 use              syn 的 UseTree
     ├─ use crate::foo::bar::Baz  → 在当前 crate 模块树里**最长前缀匹配** → 文件
     ├─ use super::x / self::x    → 先算当前文件的模块路径，再上溯
     ├─ use other_crate::…        → 查 workspace crate 名表 → 该 crate 目录
     └─ use std:: / serde::…      → 外部 crate，不进归属（无元素覆盖）
        │
④ 文件 → 元素           与 JS 完全相同的规则：path 最长前缀匹配
```

关键取舍：**模块树只用来做路径最长前缀匹配，不做类型检查、不做宏展开、不做完整 name resolution**。我们只需回答「这条 `use` 指向的文件属于哪个元素」，不需要知道它引用了哪个符号。

必须显式处理的边界（全部 → `scan.unresolved` warning，不冒充违例）：

| 形态 | 处理 |
|---|---|
| `#[path = "…"]` | 优先采用，覆盖默认推导（`syn` 能读到该属性） |
| `cfg` 条件编译的 `mod` | 两臂都进模块树（宁可多算归属，不漏） |
| 宏生成的 `mod` / `include!` | 无法静态得知 → unresolved |
| `pub use` re-export | 只算「声明了依赖」，不穿透追源（与 JS barrel 同策略） |
| 孤儿文件（无 `mod` 声明） | 不进模块树 → 其 `use` 全部 unresolved |

元素 `path` 的映射粒度与 JS 统一：可指 crate 目录（package 级）、crate 内子目录（module 级）、或单文件（component 级）。**同一个最长前缀规则，不因语言而异**——这是「两套解析器、一套归属逻辑」的分界点。

### 6.4 其他选型

| 领域 | 选型 | 状态 |
|---|---|---|
| 变更清单 | `ArchiveVcsPort::change_edit_set(root, base)`（`git diff <base> --name-only -z` ∪ `ls-files --others`） | 已有，唯一入口 |
| 模型冲突检测 | `vcs::unmerged_files()`（`diff --name-only --diff-filter=U -z`） | 已有 |
| 代码量 / 语言分布 | `tokei`（`code_stats` 既有） | 已有 |
| 存储 | 默认零持久化；确需落库才用 `store` 的 `native_model` 版本链 | 已有机制 |

---

## 7. 产物标准（验收阈值）

### 7.1 目的

| # | 目的 | 不做的后果 |
|---|---|---|
| 1 | **给回退序一个触发条件** | 「要不要砍 oxc」变成凭感觉的争论；实测数字是唯一裁决依据 |
| 2 | **量化唯一不可估的成本** | `ignore` / `globset` / `cargo_metadata` / `syn` / `petgraph` 都在锁内，成本可估；只有 `oxc_*` 是黑箱——特性开关默认会带进一堆小包，不测不知道 |
| 3 | **守住发版链与开发体感** | 一次 +90 s CI 冷构建直接吃「点发版到出包」的反馈时长；+15 s 本地增量乘上每相位多次的 `static_analysis` 就是分钟级浪费 |
| 4 | **留下可复核基线** | 半年后有人问「装包怎么这么大」，能翻到「哪次引入、多了多少、当时怎么判的」 |

第 2 条是核心：本 explore 的技术风险几乎全部集中在 `oxc` 三个 crate 的体积与编译成本上。

### 7.2 基线（HEAD `f5539bbe`，本机 Windows release 构建）

| 指标 | 基线值 | 出处 |
|---|---|---|
| 安装包（NSIS） | **6.68 MB**（`dev-team_0.5.26_x64-setup.exe`） | `src-tauri/target/release/bundle/nsis/` |
| 主二进制 | **26.6 MB**（`dev-team.exe`） | `src-tauri/target/release/` |
| `Cargo.lock` | 999 个包 / 11 548 行 | `grep -c '^name = "'` |
| CI 构建 | 双平台 matrix，各腿 `tauri build` 一次 | `.github/workflows/desktop-release.yml` |
| 相位预算参照 | `STATIC_CHECK_TIMEOUT = 1200 s` | `infra/checks/src/static_check.rs` |

参照系：0.5.4 → 0.5.26 十二个版本，安装包只涨 0.28 MB。所以「一次引入 +1.5 MB」已是常态增速的五倍。

### 7.3 阈值

| 指标 | 软线 | 硬线 | 超线动作 |
|---|---|---|---|
| NSIS 安装包 | +0.8 MB（≈ +12%） | **+1.5 MB（≈ +22%，即 ≤ 8.2 MB）** | 见 §7.6 回退序 |
| 主二进制 | +3 MB | **+5 MB（≤ 31.6 MB）** | 同上 |
| `Cargo.lock` 净新增包数 | ≤ 25 | **≤ 40** | 引入面失控，先查特性开关 |
| CI 冷构建（单腿） | +45 s | **+90 s** | 回退序第 ① 步 |
| 本地增量（改 `archi` 一文件后 `cargo build`） | +8 s | **+15 s** | 查是否误引重特性 |
| 单次 `archi_check` 运行 | 1.5 s | **3 s**（300 元素模型 + 1000 源文件扫描） | 加并发 / 加进程内缓存 |

`Cargo.lock` 净增包数单独设线，是因为它比体积更早暴露问题：`oxc` 家族按特性会拉进 `compact_str` / `rustc-hash` / `simdutf8` / `bitflags` 等一串小包，净增超 40 基本可断定某个默认开关把不需要的东西带进来了。

**1000 文件是设计基准**：本仓 `packages/desktop` 当前约 250 个非测试源文件，1000 文件约为其 4 倍，覆盖「中型单体 / 小型 monorepo」。超出该量级时先看 §3 的三档策略（>5000 提示改用增量模式），而不是直接放宽硬线。

### 7.4 执行方式

四段式：基线 → 变体 → 差分 → 判读。全程**同一台机器**（开发机，共享 CI runner 的 CPU/磁盘抖动会淹没差值）。

**A. 基线（写任何 `Cargo.toml` 之前）**

```bash
cd packages/desktop/src-tauri

grep -c '^name = "' Cargo.lock                       # A1 锁内包数，记 999

cargo build --release -p dev-team                    # A2 二进制体积
ls -l target/release/dev-team.exe                    # 记 26.6 MB

cargo build --release -p dev-team --timings          # A3 逐 crate 用时
# 产出 target/cargo-timings/cargo-timing-*.html；只抄「archi 依赖子树 + dev-team 自身」

touch crates/core/archi/src/lib.rs && time cargo build -p dev-team   # A4 本地增量，5 次取中位
# Windows: Measure-Command { cargo build -p dev-team }

pnpm -C .. run build                                 # A5 安装包（需 TAURI_SIGNING_PRIVATE_KEY）
ls -l target/release/bundle/nsis/*.exe
```

A5 需要完整 `tauri build` 与签名环境变量。当时不便时用代理指标（`dev-team.exe` + 前端 `dist/` 总量），但必须在记录里注明「安装包为代理值，未实测」。

**B. 变体（加完依赖后）**：重复同一批命令，缓存状态可比。冷测不必 `cargo clean` 全局（会毁掉全部缓存），直接看 `--timings` 里新增依赖子树的用时；热测只 `touch` 同一文件。

**C. 差分与判读**

```
差值 = 变体 − 基线
  全部 ≤ 软线            → 通过，记录
  任一项 ∈ (软线, 硬线]   → 通过但留痕：进 spec 的「已知成本」清单 + proposal 记录
  任一项 > 硬线          → 不通过：进回退序，回退后重测三项
```

一条纪律：**「未测」不等于「通过」**。确实测不了的记「未测 + 原因」并在 spec 留痕，不得默认视为达标。

**D. 运行期（与构建解耦）**：`crates/core/archi/tests/perf.rs`，release 跑，语料 **1000 源文件 + 300 元素**，10 次取 p95；计时用 `std::time::Instant`，不引 criterion（一次性门禁用不上统计框架）。

**E. 记录落点**：`crates/core/archi/tests/README.md`（与 `checks/tests/golden/README.md` 同风格），固定五列：

| 指标 | 基线 | 变体 | 差值 | 判定 |
|---|---|---|---|---|
| NSIS 安装包 | 6.68 MB | … | … | 通过 / 超软线留痕 / 超硬线回退 |
| 主二进制 | 26.6 MB | … | … | |
| `Cargo.lock` 包数 | 999 | … | … | |
| 编译（archi 子树） | … | … | … | |
| 本地增量（中位） | … | … | … | |
| `archi_check` p95 | … | … | … | |

同步抄一份进 change 的 `design.md`，让评审者在 review 时直接看到数字。

### 7.5 不常态化跑 CI

共享 runner 的冷启动、磁盘 IO、CPU 差异都能吃掉本方案的差值量级（零点几 MB / 几十秒）。常态跑会得到「天天告警但没人信」的结果，反而消耗阈值公信力。

如需自动化：加一条 `workflow_dispatch` 手动触发的 workflow，输出实测表作为工件，不进 PR 门禁（与 `desktop-release.yml` 的 `force` 输入同类）。

### 7.6 超标回退序

按代价从低到高，每步重测，一旦回到硬线内即停：

```
① 减特性：oxc_parser 只开 parse，不引 oxc_semantic / oxc_codegen / oxc_minifier
          我们要的只是 import 声明，不需要语义分析 → 收益最大、代价最小，先试这步
② 砍 oxc_resolver → 自算路径约 150 行（相对路径拼接 + workspace 包名映射 + tsconfig paths）
          代价：exports / imports 条件解析退化，语料相应条目改标「已知不支持」
③ 砍 oxc_parser → 正则 import
          代价：type-only 判定与 span 行号退化，② 的误报率上升；
          且实质违反「type-only 算架构依赖」的裁决 → 必须回到设计讨论，不是实现层能自定
```

①② 都在方案预设容差内；只有走到 ③ 才需要重新裁决。

---

## 8. 语料与 Spike

```
crates/core/archi/tests/
├── corpus/
│   ├── src/<case>/**          输入代码（import / use 语料）
│   └── expected/<case>.json   期望（报告结构 / 诊断）
└── perf.rs                    运行期 p95（§7.4 D）
```

### 8.1 交叉核对语料（门禁）

| 用例 | gate 档 | 期望 |
|---|---|---|
| import 与声明一致 | strict | clean |
| 未声明依赖（A 引 B，模型无 A->B） | 引入方 strict | error |
| 同上 | 引入方 warn | warning |
| 同上 | 引入方 off | 不出现（文件算 unmatched） |
| 同一元素对在 3 个文件各 1 处 | strict | 1 条 error + `occurrences` 3 项（聚合不铺开） |
| 目标元素不存在于模型 | strict | warning（目标未建模，归因不明） |
| 测试文件跨元素 import | — | 不扫描（`scan.excluded` 留痕） |
| `import type` 跨元素 | strict | error（type-only 算架构依赖，`typeOnly: true`） |
| `// archi-ignore-next-line` 标注 | strict | 不报 + `suppressed` 有 inline 条目（含 file/line/reason） |
| `architecture.ignore` 元素对 | strict | 不报 + `suppressed` 有 config 条目（含 reason） |
| `oxc_resolver` 解析失败 | strict | warning（不冒充违例，`scan.unresolved` 留痕） |
| 声明了但代码未兑现（增量） | — | 不进报告门禁段，只在视图标 🟡 |
| 声明了但代码未兑现（全量） | — | 报告 `observations` 记录，`status` 不变 errors |
| 存量模型含结构错 | — | 照常加载（`model.structuralErrors` 记录）+ 门禁判 error |
| 模型为空 | — | status = skipped |
| 无变更文件（增量） | — | status = noChanges |
| 全量模式、仓库无源文件 | — | status = noChanges |

### 8.2 Rust 模块树语料（§6.3）

| 用例 | 期望 |
|---|---|
| `src/lib.rs` 含 `mod foo;`，`src/foo.rs` 存在 | `crate::foo` → `src/foo.rs` |
| `src/foo/mod.rs` + `src/foo/bar.rs` | `crate::foo::bar` → `src/foo/bar.rs` |
| `#[path = "custom/x.rs"] mod x;` | `crate::x` → `src/custom/x.rs`（`#[path]` 优先） |
| `#[cfg(test)] mod tests { … }` | 内联模块进树但不产生文件条目 |
| `#[cfg(unix)] mod a;` + `#[cfg(windows)] mod a;` | 两臂都进树，同一模块路径（不报重复） |
| `use crate::foo::bar::Baz` | 最长前缀匹配到 `crate::foo::bar` → 文件 → 元素 |
| `use super::sibling::X`（文件 `crate::a::b`） | 解析为 `crate::a::sibling::X` |
| `use other_crate::X`（∈ workspace） | 落到该 crate 目录 → 元素 |
| `use serde::Deserialize`（外部 crate） | 不进归属，不算违例 |
| `use missing_crate::X` | `scan.unresolved` warning |
| 宏生成的 `mod` | `scan.unresolved` warning，不冒充违例 |
| 孤儿文件（无 `mod` 声明） | 该文件 `use` 全部 unresolved |

### 8.3 全量 vs 增量一致性

| 用例 | 期望 |
|---|---|
| 同一 commit，全量与增量都跑 | 增量报告 ⊆ 全量报告（同一模型下，增量只覆盖变更文件） |
| 模型声明 A→B 但代码无 import | 两模式都进 `observations`；`status` 都不因它变 |
| 全量模式仓库无源文件 / 无模型 | status = noChanges / skipped |

### 8.4 Spike

| Spike | 内容 | 验证什么 |
|---|---|---|
| **C1 归属与核对** | §8.1 / §8.3 语料；`change_edit_set` → `oxc_resolver` 归属 → 对账 → `reports/archi/*.json` | 归属判定、gate 档定级、豁免机制、报告槽 |
| **C2 Rust 归属** | §8.2 语料；以本仓自身为靶（13 个 workspace crate） | 模块树构建 + `use` 最长前缀匹配在真实 crate 上是否可靠 |
| **C3 门禁挂载** | `StepNode::ArchiCheck` 挂 implement → 真实 change 跑出 fail → FeedbackLoop 回 executor → 超限升格；旧 db 验证存量 run 仍可读 | 相位表接线 + bincode 尾部追加纪律 + 端到端手感 |
| **C4 产物实测** | 按 §7.4 出实测表（开发机） | 依赖成本量化；超硬线触发回退序 |

---

## 9. 硬约束

| 约束 | 出处 | 违反后果 |
|---|---|---|
| `RunStepKind` / `StepKind` 新变体尾部追加 | `workflow/state.rs` bincode 下标落库 | 存量 run 节点行 / 审计行步词整体错位 |
| `workflow` 与 `archi` 互不依赖 | `desktop-crate-layout` spec | crate 图破，纯读域引入编排行为 |
| core 禁 Tauri、core 禁 store | 同上 | 判定域被持久化 / 壳层污染 |
| 新解析器要带语料金标准 + 过变异门槛 | `checks` 的 `tests/golden` 文化 + `config.json` 的 mutation 80/70 | 变异分不达标，流程在 test-execution 相位卡死 |
| 架构扫描排除规则与 `tests[].excludes` 正交 | `config` crate 的 `TestSuite` 语义 | 调测试 scope 会意外改变架构扫描范围 |
| 豁免只到元素对粒度，禁 glob | §1.2 | glob 豁免 = 永久后门，ratchet 失效 |
| 架构域配置只落 `.dev-team/architecture/config.json` | §1.3 | 配置来源分散，用户不知道改哪 |
| `oxc_parser` + `oxc_allocator` + `oxc_resolver` 是唯一真新增依赖 | `Cargo.lock` 现状 | 成本由 §7 实测量化；超硬线按回退序 |
| 定级只看引入方 gate 档 | §2.2 | 一个 strict 元素把全仓库 off 元素拽进门禁 |
| `ToolStepOutput` / `StepNode` 尾部追加 | `orchestration/port.rs` / `workflow/state.rs` | 枚举下标错位 |
| 不复用 `Test` 报告槽 | §4.4 | 三类语义（plans 索引 / 覆盖率复用 / mutation scope）互相污染 |
| 模型解析失败 MUST 返回结构化 errors，绝不 panic | 另一个 explore 定 | 写坏模型期间整个检查链崩溃 |
| 报告 MUST 明确区分 skipped / noChanges 与 clean | §5 | 「没检查」被误读成「检查过没问题」 |

---

## 10. 未决问题

1. **性能基准的可复核性**——1000 文件基准已定（§7.3），但真实工程若远超此量级需重估 3 s 硬线。这一条要等第一个真实用户工程才能验证。

2. **`unresolved` 的上限**——`scan.unresolved` 现在只警告不设阈值。若某工程大量使用宏生成模块，unresolved 会淹没真实违例。是否需要「unresolved 超过 N% 时单独提示」待观察。

---

## 11. 不在本 explore 范围

| 事项 | 归属 |
|---|---|
| 模型存储格式 / 结构校验 / 画布 / 文档关联 / agent 工具 / 归档确认 | `archi-graph-doc-agent` |
| 独立 CLI 执行（不依赖 dev-team） | 前期先用 dev-team 做全量检查，CLI 壳后置 |
| 「模块文档替代 specs」的迁移 | 需独立 change |
| 跨语言依赖（TS 调 Rust 的 Tauri 命令） | 不在本期 |
| 符号级契约核对 | 与模块接口协议同源，需独立能力 |
