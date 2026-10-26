# 桌面端架构看护系统方案（Local Architecture Guardrails · packages/desktop）
workflow_type: requirement

---

## 0. 范围与前提

本方案只在 `packages/desktop` 一个包内落地：Rust 侧 crate 分层 + Tauri 壳 + React 前端。全部能力自含于桌面端，不依赖任何外部进程型工具链或宿主侧集成——`.c4` 解析、图计算、门禁判定、报告落盘与可视化均由桌面端自有 Rust / TS 代码承担。

三条从现状推导出的前提：

1. **模型解析必须原生**：桌面端 agent 工具面是封闭七件套（`crates/infra/agent/src/sdk/tools.rs` 的 `TOOL_NAMES`），会话的 `mcp_servers` 恒为空（`sdk/loop.rs` 铸 `Vec::new()`，`runner_test` 断言恒空）。没有 MCP / node 子进程通道可借来解析 DSL。
2. **门禁只能落在相位编排里**：桌面端唯一的刚性拦截形态是 `phase_table` 的 `StepNode` + `OnFail::{Terminal, FeedbackLoop, Escalate}`（`crates/core/workflow/src/write/phase_table.rs`）。不存在也不会引入 git hook 型提交拦截。
3. **取数是显式的**：`desktop-file-watch` spec 把「显式取数、无 watch 无轮询」定为纪律，watch 是唯一例外且限「单文件、信号不含内容、前端防抖、后端零去抖」。架构图与门禁结论都走显式命令，不做常驻守护进程。

目标是消解「设计图 / 契约文档 / 真实源码」三者之间的**架构漂移**：模型声明什么，代码就得是什么；漂移在相位门禁处被静态算法拦下，剩下的语义重构交给相位 agent。

---

## 1. 设计原则

* **声明式单一事实源**：架构拓扑与模块边界完全由 `openspec/architecture/models/*.c4` 决定，纯文本、git 友好、可 review。
* **不造第二套契约**：契约复用既有的 `openspec/specs/<capability>/spec.md` 与 `openspec/architecture/decisions/*.md`（ADR）。不引入「每模块一份 md + 语言无关 YAML API schema」这层中间物。
* **模型 = 源码**：模型文件随 git 提交，在 change 的 worktree 里与源码同权地被修改，归档时回合。**回合必须人工确认**——归档确认面单列模型 diff 区块。设计权体现在「回合要人批」，不体现在「写盘要拦」。
* **三级判定，各管各的**：结构校验管模型自洽；相位门禁管代码与模型一致；架构视图管设计意图的观察。混级就会把「还没写」误判成「写错了」。
* **级别二值**：报告只有 `errors` / `warnings`，定级由元素的 `gate` 档决定，不由问题类型决定。
* **刚柔分层**：可判定的问题用静态算法做刚性拦截（零 LLM、确定性、可复现）；「这样拆对不对」「怎么重构」交相位 agent。
* **零持久化优先**：模型每次全量解析进内存（与 `code_stats` 每次重新 `tokei` 解析同哲学），不引入第二套存储引擎，不做隐式缓存失效管理。
* **模型变更不交给 LLM 裁决**：模型文件冲突不进 agent 解算，一律中断交人。

---

## 2. 模型

### 2.1 实体与映射

四层实体收敛为三层：

```
【 拓扑层 】  C4 element（kind: package / domain / module / component）
              ├─ extend <parent> { … }   ⇒ Contains（来自 DSL 结构本身）
              └─ A -> B "desc"           ⇒ DependsOn（来自显式关系声明）
                    │
                    ▼  语义锚点
【 锚定层 】  metadata { path ['./src/x/', './src/y.ts'] }   ⇒ 1:N 物理边界
                    │
                    ▼  交叉核对
【 代码层 】  变更清单（vcs）∪ import / use 解析              ⇒ 事实依赖边
```

| 项 | 取舍 | 理由 |
|---|---|---|
| 逻辑模块 | **C4 element 即模块**，不另建 LogicalModule | `c4-dsl-syntax` 已定义四类 kind 的层级（`extend` 表包含、`->` 表依赖），另立一层只会双写 |
| 物理映射 | **`metadata.path` 即 FileMapping**，天生是数组 | 一个元素可覆盖多个路径（目录 + 单文件混合） |
| API 契约 | **不落权威 schema**；需要时从源码提取符号清单作派生视图 | 契约权威在 specs + ADR；再造语言无关 schema 必然漂移，且要维护 Rust/TS 两套提取器对齐 |
| 模块文档 | **不规定形态，只做关联与检索**（§5.2） | 文档该写什么因团队而异；钉死 schema 会把「还没想清楚」变成「落后于文档」 |

### 2.2 门禁档位（ratchet）

`metadata` 是自由 key-value（解析器已支持任意 key 与数组值），「按空间收紧」不需要新语法、不需要新配置项：

```
metadata { path './src/auth/' }                    ← 缺省 off，不纳管
metadata { path './src/order/', gate 'strict' }    ← 该元素严格
metadata { path './src/legacy/', gate 'warn' }     ← 只提示不拦
```

| 档位 | 参与归属 | 作为引入方时的违例定级 |
|---|---|---|
| `off`（缺省） | 否——其文件算 unmatched，只计数不报错 | 不报 |
| `warn` | 是 | warning |
| `strict` | 是 | **error** |

**定级只看引入方档位**（谁引入了不该有的依赖，谁负责），避免一个 strict 元素把全仓库的 off 元素拽进门禁。

`gate` 取值非法时按 `off` 处理并出诊断（与 `config` crate 既有的「非法值降级 + diagnostic」同风格）。

### 2.3 扫描范围

架构扫描 MUST 排除所有测试文件与测试夹具。**不复用 `config.json` 的 `tests[].excludes`**——那是测试执行 scope（可因 e2e 慢而排除、因 fixture 需要而包含），与架构扫描范围正交。

```
默认约定（可被 architecture/config.json 的 exclude 覆盖）
  **/*.test.{ts,tsx,js,jsx}   **/*.spec.{ts,tsx,js,jsx}   **/__tests__/**
  **/*_test.rs                **/tests/**                 **/fixtures/**   **/__mocks__/**
```

理由不止噪声：测试文件天然跨元素引用被测代码，纳入扫描会让每个 `foo.test.ts` 对 `foo.ts` 产生一条未声明依赖——这不是架构问题，是测试的固有形状。

### 2.4 豁免

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

### 2.5 架构域目录

```
openspec/architecture/
├── models/*.c4       模型（结构 + metadata.path / gate / docs）
├── decisions/*.md    ADR
├── config.json       豁免与扫描排除
└── reports/*.json    全局报告（change 内的报告落 change 的 reports 槽）
```

架构域的一切落在这一个目录下：一次架构重构的 diff 全在这里，可整体 review、可整体搬动。

**模块文档不进这个目录**，而是跟代码走——放元素 `metadata.path` 覆盖的目录内（如 `src/auth/README.md`），或用 `metadata.docs` 显式指向任意位置。文档与它描述的代码应当同生命周期；放进 `architecture/` 会让文档与实现分离、更容易过期。

### 2.6 两个配置文件的边界

```
openspec/config.json                   既有，桌面 config crate 唯一出口
├── schema / context / rules           相位 planner 的自然语言规则
├── static_analysis                    静态检查命令
├── tests[]                            测试执行 scope
└── write_protection                   写保护

openspec/architecture/config.json      新增，架构域自有
├── ignore: [{ from, to, reason }]     元素对豁免（reason 必填）
└── exclude?: [glob]                   扫描排除覆盖（缺省用 §2.3 约定）
```

拆两个文件而不是把 `architecture` 键塞进 `config.json`：`config.json` 的 `KNOWN_KEYS` 是七项白名单，加键要同步改 `KNOWN_KEYS` / `WorkspaceConfig` / 解析分支 / 前端 config 页 / 两份 schema；而架构域配置与「测试怎么跑、流程怎么流」无关，属另一关切面。拆开后 `archi` crate 自解析自己的配置，`config` crate 不必认识架构语义。

管辖面写死：`config.json` 管**执行**（命令、scope、阈值、保护），`architecture/config.json` 管**看护策略**（豁免、扫描排除）。MUST NOT 出现第三个配置文件。

### 2.7 长期演进（不在本方案范围）

方向是「模块文档逐步替代 `openspec/specs/`，最终只留少数主要特性文档」。本方案**不做任何预铺**，只守四条边界：

| # | 边界 | 理由 |
|---|---|---|
| 1 | 看护不读 specs | 否则替代完成那天，看护逻辑要整体改写 |
| 2 | 检索不读 specs | 一旦纳入，用户会开始写「给 archi 看的 spec」，反向锁死迁移 |
| 3 | 模块文档关联只认三种来源（`metadata.docs` / 邻近 md / ADR） | 不新增 spec 来源 |
| 4 | 门禁不依赖 specs ↔ element 映射 | 该映射将来由「模块文档 ↔ element」承担 |

迁移的真正难点（将来独立裁决）：specs 现在承担六项职责——契约记录 / 可验收断言（Scenario）/ 能力盘点 / 归档 SpecSync 的增量合并 / 测试设计输入 / 验收输入。后两项实际已不直读 specs（test-design 读 design.md、acceptance 读 proposal.md 的 AC），风险低；但**可断言结构**与**增量合并语义**没有散文形态的对应物。若模块文档是纯散文，acceptance 会失去可核对的形状，归档的 `SpecSync` 步（ADDED/MODIFIED/REMOVED/RENAMED 合并）没有对应物、只能退役。

---

## 3. 判定

### 3.1 三层判定

```
[L1] 结构校验 — DSL 语法 + 结构 Lint
     语法错 / 缺 specification / 成环 / A -> anc(A) / 元素重复 / 引用不存在的元素
     触发时机按写入口分两面（模型内部自洽性，不读一行代码）：
       · 命令面（画布连边）   → 写前校验，非法拒绝写入并返回诊断
       · 工具面（agent write/edit）→ 不写前校验；读取时返回结构化 errors、
                                      门禁时判 structuralErrors（§5.4）

[L2] 相位门禁（implement）— ArchiCheck 交叉核对（增量模式）
     漂移② 未声明依赖      引入方 off 不报 / warn → warning / strict → error
     漂移④ 元素 path 失效  该元素 off 不检查 / warn → warning / strict → error
     特点：要读代码，要 vcs 变更清单；只扫本 change 变更文件

[L3] 架构视图 — 全景呈现（全量 / 增量两模式，§3.3）
     漂移① 声明了但代码未兑现
          · 增量模式（change 级）  → 画布 🟡 观察项
          · 全量模式（workspace 级）→ 可判定事实，可参与体检
     跨 kind 向上的边 → 橙色虚线（提示价值，非约束）
```

**漂移① 不进任何门禁**——即使全量模式下可算。① 是「设计已画、代码还没写」，正确回应是继续实现或开新 change，不是让门禁拦下当前 change。

### 3.2 漂移分类与定级

| # | 漂移 | 定义 | 判定层 | 级别 |
|---|---|---|---|---|
| ① | 声明未兑现 | 模型声明 A→B，代码中无任何 A→B 的 import | L3（全量可判定 / 增量观察） | 不进 `status`（→ `observations`） |
| ② | 未声明依赖 | 代码存在跨元素 import，模型未声明该关系 | L2 门禁 | 按引入方 gate 档 |
| ④ | path 失效 | 元素 `metadata.path` 指向不存在的目录 | L1 报 / L2 定级 | 按该元素 gate 档 |

层级逆向（如 `component → package`）**并入 ②**：C4 的 kind 是包含层级不是约束层级，「允许的依赖方向」没有权威来源；「声明了就合法」是唯一自洽规则。例外保留在 L1：`A -> anc(A)`（元素依赖自己的祖先）是图结构自相矛盾，零规则来源即可判定。

### 3.3 全量 / 增量两模式

```
全量模式（workspace 级，主仓根）
  基准 = 无（当前工作区就是全部）
  扫描 = 整个工作区的源文件（ignore 遍历 + §2.3 排除）
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

**性能分界**：结构校验与文档关联只读 `.c4` 与目录列表，永远毫秒级；漂移计算要解析源码，与规模线性。因此「打开页面看到图」永远快，「看到漂移」才可能慢。

| 源文件数 | 全量漂移耗时 | 策略 |
|---|---|---|
| ≤ 500 | < 1 s | 页面打开即算 |
| 500 ~ 5 000 | 1 ~ 10 s | 先出图，漂移按钮显式触发 |
| > 5 000 | > 10 s | 同上 + 提示可改用增量模式 |

### 3.4 门禁挂载

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

三条挂载结论：

* **只在 implement 挂**。acceptance 不挂（模型一致性已在 implement 收敛，架构问题转新 change 验收）；test-only 不挂（该表无 implement，代码改动在 code-analyze，而测试文件被扫描全排除，必然 clean/skipped）。
* **用 `FeedbackLoop{3}` 而非 `Escalate`**。② 的成因有两种且无法自动区分——「代码写了不该写的依赖」（改代码）与「模型声明落后于设计」（改模型）。缺省权威是模型，所以先让 executor 改代码；`FeedbackLoop` 超限时本来就升格相位失败（`walker.rs` 的 `step_static_check`：`feedback >= limit` → 代写 fail phase_log → `StepFlow::Upgraded` → 回 `phase_next` 进决策分叉），「先修代码、修不动交决策裁决是否改模型」这条路不需要新 `OnFail` 变体。
* **报告槽新加第三值**。`ReportSlot` 是纯进程内枚举（`Test` / `Baseline`），加 `Archi` → `reports/archi`。不复用 `Test` 槽靠文件名前缀区分——`reports/test` 的 `plans[]` 索引语义、覆盖率复用判定、mutation scope 都挂在槽位上，混入架构报告会让三类语义互相污染。

变体追加纪律：

| 枚举 | 位置 | 编码 | 追加方式 |
|---|---|---|---|
| `RunStepKind`（21 值） | `workflow/state.rs` | bincode 枚举下标落库 | **只能尾部追加**（源码有明文警告） |
| `StepNode`（21 值） | `workflow/state.rs` | 仅相位表内存结构，不落库 | 位置自由，建议尾部追加保持一致 |
| `StepKind`（审计八臂） | `workflow/state.rs` | 落 `StepRecord`（`native_model` id=12） | 尾部追加 + `StepRecord` 走版本链 |
| `ToolStepOutput` | `orchestration/port.rs` | 进程内封闭集 | 尾部追加 |
| `ReportSlot` | `foundation/layout` | 纯进程内枚举（零 serde / 零 specta） | 加第三值 `Archi` |

### 3.5 报告结构

只有增量模式落盘：`reports/archi/<timestamp>.json`（门禁审计面）。两种模式共用同一结构体；`ArchiCheckOutcome` 在进程内从它派生 `passed` / `diagnostics`。

```jsonc
{
  "version": 1,
  "timestamp": "2026-10-20T08:31:12Z",
  "mode": "incremental",                    // incremental | full
  "root": "/abs/path/to/worktree",
  "change": "add-billing-gate",             // full 模式为 null
  "base": "a1b2c3d",                        // full 模式为 null
  "modelHash": "sha256:…",                  // models/*.c4 聚合哈希

  "model": {
    "elements": 24,
    "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [                   // L1 结果——读存量模型遇错也照录
      { "kind": "dependsOnAncestor", "element": "App.Auth.Token",
        "ancestor": "App.Auth", "message": "元素依赖自己的祖先" }
    ],
    "warnings": [
      { "kind": "pathNotFound", "element": "App.Billing", "path": "./src/billing/",
        "gate": "strict", "message": "metadata.path 指向不存在的目录" },
      { "kind": "noPath", "element": "App.Legacy", "gate": "off",
        "message": "元素未声明 metadata.path（不参与归属）" },
      { "kind": "docNotFound", "element": "App.Auth", "doc": "./docs/auth.md",
        "message": "metadata.docs 指向的文件不存在" }
    ],
    "docs": { "explicit": 6, "adjacent": 11, "adr": 4 }
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
* **`structuralErrors` 只录不拦**：读存量模型遇到 `dependsOnAncestor` 时照常加载、报告记录、门禁判 error。这样「修模型」本身也走 change 流程，而不是被一个打不开的页面挡住。

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

---

## 4. 桌面端落位

### 4.1 crate 图

```
                  ┌────────────────────────────────┐
                  │ desktop-app (src-tauri, Tauri) │
                  └───┬────────┬────────┬───────┬──┘
                      │        │        │       │
        ┌─────────────┘        │        │       └──────────────┐
        ▼                      ▼        ▼                      ▼
   ┌─────────┐            ┌────────┐ ┌────────┐          ┌───────────┐
   │ workflow│            │ config │ │ store  │          │ archi  ★  │
   │ (change)│            │        │ │(native │          │ 新增：纯读 │
   └────┬────┘            └───┬────┘ │ _db)   │          │ 图 / 规则  │
        │                     │      └───┬────┘          └─────┬─────┘
        │                     │          │                     │
        └──────────┬──────────┴──────────┴─────────────────────┘
                   ▼
            ┌────────────┐
            │ foundation │  目录常量组 / workspace 身份段 / cancel 原语
            └────────────┘

  workflow ⇄ archi 互不依赖（硬约束，desktop-crate-layout spec 已写死）
  门禁协议经 orchestration::port 的 trait 缝跨过去
```

* 新建 `crates/core/archi`：纯逻辑、零 Tauri、零 store、零 spawn。负责 `.c4` 解析、内存图构建、拓扑校验、交叉核对、报告结构体。
* `crates/core/orchestration` 增第四个 runner trait，使 `workflow` 能挂载门禁步而不认识 archi。
* 若将来需要 `cargo metadata` 之类进程解析，实现落 `crates/infra/archi`（新建 infra 成员），core 仍只见 trait。

### 4.2 目录常量（foundation）

`foundation::layout` 目前只有 changes / archive / explores 三棵树。新增五个**派生函数**，不动 `Layout` struct（零构造点波及）：

```
archi_dir(root)            → openspec/architecture           （域根，前缀单点）
archi_models_dir(root)     → <archi_dir>/models
archi_decisions_dir(root)  → <archi_dir>/decisions
archi_config_path(root)    → <archi_dir>/config.json
archi_reports_dir(root)    → <archi_dir>/reports
```

按 foundation 的「第二个消费者到场才下沉」纪律，这些项落地时需在 `desktop-crate-layout` 的 spec 里登记为新增消费者。`openspec` 域根常量已存在（`domain_dir_name`），`architecture` 段由 `archi_dir` 单点持有，其余全部从它派生——零散字面量不进任何 crate。

### 4.3 门禁协议（orchestration::port）

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

产出走既有的 `ToolStepOutput` 封闭集（新增 `ArchiCheck` 变体），`workflow` 侧零 archi 依赖。

### 4.4 观察根与双根流转

**架构模型是与源码同等的资产**：主仓持基线，change 在 worktree 改后回合。因此「读哪一份」不需要新规则——**跟源码一样，读当前观察根**。

```
观察根                              模型来自                用途
──────────────────────────────────────────────────────────────────────────
workspace 级（/architecture）       主仓根                  架构全景 / ratchet 决策
change 级（change 详情内）          detail.worktree        该 change 的模型实况 + 门禁
                                    （归档 change 回落主仓根，与会话 cwd 同口径）
```

这与前端既有口径一致：`AppRoutes` 每页拿 `root`（workspace 根），change 侧会话用 `detail.worktree`（`use-change-chat.ts` 的 `cwd` 派生：归档 change 恒 `null` 回落 root）。archi 命令面照抄这个入参形态——**命令签名一律带 `root`，不引入「当前选中 change」这类隐式上下文**；根由调用方决定，`archi` crate 只认传入的根。

```
主仓 /openspec/architecture/models/*.c4       ← 权威，随 git 提交
   │  git worktree add (base_commit)
   ▼
worktree /openspec/architecture/models/*.c4   ← change 期的工作副本
   │
   ├─ 结构校验 / 相位门禁            读 worktree
   ├─ 架构视图（change 级）          读 worktree
   ├─ 架构视图（workspace 级）       读主仓
   │
   └─ 归档：Commit → Rebase → … → ff_merge 合回主仓
                          ▲
                          └─ 模型文件冲突 → 中断，交用户处理（不进 agent）
```

`git.rs` 已有 `unmerged_files()`（`git diff --name-only --diff-filter=U -z`）；`ResolveConflicts` 步拿到 `cursor.rebase_conflicts` 后**先按路径分流**：

```
pending = cursor.rebase_conflicts
   │
   ├─ 含 openspec/architecture/ 下任何路径 → Terminal::failed
   │     文案：模型冲突需人工裁决，请在 <worktree> 处理模型后重跑归档
   │     理由：模型是设计权威，agent 自动解算等于让 LLM 替人做架构决策
   │
   └─ 全部是代码路径 → 既有 agent 解算流程（零改动）
```

这是唯一新增的归档判定，纯路径前缀匹配 + 终止，不改 `ResolveConflicts` 现有的会话 / 后验 / lean 收敛任何一步。

**保留 rebase 半程**：`git.rs::rebase_branch` 的既有语义就是「冲突态保留于 worktree——MUST NOT 自动 abort」（只有非冲突失败才尽力 abort），所以模型冲突这条路径只要 walker 侧不调 abort 即可，不需要新 vcs 能力。

但保留半程带来必须处理的**重入陷阱**：

```
T0  Rebase：git rebase → 冲突，worktree 处于 rebase 半程（REBASE_HEAD 在场）
T1  ResolveConflicts：检出模型路径 → Terminal::failed（不 abort）
T2  用户重跑归档 → 从 Preflight 开始
        │
        └─ Commit 步：dirty() 为真 → commit_all() = git add -A + git commit -m
              ⚠ 若用户尚未 rebase --continue / --abort，这是在 rebase 中间态提交
```

所以 `Preflight` MUST 检出半程并挡回（`worktree_snapshot()` 已返回 `rebase_head`，零新 git 能力），给两条出路：

```
① 在 <worktree> 内解完模型冲突后 git rebase --continue，再重跑归档
② 放弃本次重放：git rebase --abort，再重跑归档（会重新 rebase 一次）
```

| 用户处置 | Preflight | 结果 |
|---|---|---|
| 已 `rebase --continue` | `rebase_head` 不在 → 放行 | Commit（干净跳过）→ Rebase（up-to-date no-op）→ Merge 快进 |
| 已 `rebase --abort` | 放行 | Commit → Rebase（正常重放，可能再冲突）→ … |
| 什么都没做直接重跑 | 挡住 + 引导 | 不产生「rebase 中间态提交」这类难排查的污染 |

代价：半程 change 持续占用 active 名额（上限 5），worktree 此时不可被 Cleanup 删。前端在归档失败面必须显示 worktree 绝对路径 + 两条处置命令。

### 4.5 归档确认：模型 diff 要显眼

模型改动混在几十个代码文件里会被橡皮图章掉。在既有 `(proceed, sync_specs)` 停等点前加一个专用区块（**不是新的确认门**，只是提高可见性）：

```
归档确认
├─ 变更概览：N 个文件改动（M 源码 / K 模型 / J 文档）
├─ 🏛 架构变更（仅当模型有改动时出现）
│     元素 +2（App.Billing, App.Billing.Invoice）
│     元素 -1（App.Legacy）
│     关系 +3 / -1
│     gate: App.Billing off → strict
│     [查看 .c4 diff]  [查看新图预览]
└─ 同步 delta specs: 是 / 否
```

「查看新图预览」比读 `.c4` diff 直观：归档前能在画布上看到改动后的图。模型无改动时该区块零占位。

---

## 5. agent 面

### 5.1 两个只读工具

沿用 `create_explore_note` 的先例（专用工具不进 `TOOL_NAMES` 七工具封闭集，经 `SessionInjections` 受控追加），新增 `SessionToolset::Archi` + 两个工具。

核心取舍：**返回引用，不返回正文**。agent 已有 `read` / `grep` / `glob` 三件通用检索工具，架构工具再吐正文是重复造轮子，且会把 token 烧在「用户其实没问的文档」上。架构工具的职责边界是：把「元素 ↔ 关系 ↔ 文档」的索引告诉 agent，正文由 agent 自己 `read`。

```jsonc
// 工具 1：archi_module —— 按元素定位（概述 + 关系 + 文档引用）
{
  "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" }
}

// 响应（指定 element）
{
  "element": { "fqn": "App.Auth", "kind": "domain", "gate": "strict",
               "path": ["./src/auth/"], "summary": "认证与令牌签发",
               "metadata": { "owner": "platform" },
               "modelFile": "openspec/architecture/models/02-auth.c4",  // 声明该元素的文件
               "modelLine": 14 },                                        // 元素起始行
  "dependsOn":  [{ "target": "App.Shared", "description": "复用错误类型" }],
  "dependedBy": [{ "source": "App.Order",  "description": "登录态校验" }],
  "children":   ["App.Auth.Token", "App.Auth.Session"],
  "docs": [
    { "path": "openspec/architecture/decisions/0007-token-sink.md",
      "title": "令牌下沉到 Shared", "kind": "adr", "source": "text" },
    { "path": "src/auth/README.md", "title": "认证模块说明", "kind": "md", "source": "adjacent" }
  ]
}

// 工具 2：archi_search —— 按标识符检索（元素 + 文档路径，轻量版）
{
  "name": "archi_search",
  "parameters": {
    "query": "必填。关键词 / 元素名片段 / 文档路径片段",
    "scope": "可选。'elements' | 'docs' | 'all'（缺省 all）"
  }
}

// 响应
{
  "elements": [{ "fqn": "App.Auth", "kind": "domain", "summary": "认证与令牌签发" }],
  "docs": [{ "path": "src/auth/README.md", "title": "认证模块说明",
             "kind": "md", "source": "adjacent", "matched": "auth" }]
}
```

`modelFile` + `modelLine` 让 agent 能精确定位到要编辑的位置，不必 grep 猜；两者来自解析器已有的 span 信息，零额外成本。

**轻量检索的确切含义**：

```
匹配对象   元素 FQN / element description / metadata 值 / 文档路径与文件名（不含正文）
匹配方式   ASCII 大小写不敏感子串 + 驼峰分词（AuthToken → auth / token）
不做       全文索引、文档正文搜索、相关度打分（按来源强度排序即可）
```

不搜正文：正文搜索是 agent 既有 `grep` 的活，它能带 context、能正则、能按目录缩窄——架构工具重做只会更差。架构工具的价值在「知道去哪里找」，不在「替你去读」。

实现量：`ignore::WalkBuilder` 遍历架构域与元素覆盖路径（自动跳过 `node_modules` / `target` / gitignore），文件名与 FQN 做子串匹配，约百行；不建倒排、不缓存、每次现算（与零持久化原则一致）。

为什么是两个而不是一个：`archi_module` 是**定位**（已知在看哪个模块），`archi_search` 是**发现**（还不知道这功能归谁管）。合并会让 `element` / `query` 参数语义含混，且两个场景无法差异化收窄工具面。

### 5.2 文档关联：三种来源

不预设「模块必须有一篇文档」。只做关联发现，任何未来出现的文档形态都能被吸收：

| 来源 | 机制 | 强度 |
|---|---|---|
| 显式 | `metadata { docs ['./docs/auth.md'] }`（可选，与 `path` 同构的数组） | 强——作者亲口指定 |
| 邻近约定 | 元素 `metadata.path` 目录下的 `*.md` | 中——按位置推断 |
| ADR 文本 | `decisions/*.md` 正文出现该元素 FQN | 弱——检索命中，非承诺 |

三者合并去重，按强度排序返回。**契约只有 `{ path, title, kind, source }`**——没有摘要、没有正文、没有 schema。将来加「接口说明文档」这类新形态，只需加一条索引来源，工具契约不变。

* `metadata.docs` 指向不存在的文件 → `model.warnings` 的 `docNotFound`，只报不拦（文档缺失不影响代码正确性）。
* 邻近搜索与路径校验都在元素 `metadata.path` 覆盖范围内，与 §2.3 排除规则同一套（测试目录里的 md 不算模块文档）。

### 5.3 注入矩阵

| 场景 / 相位角色 | `archi_module` | `archi_search` | 理由 |
|---|---|---|---|
| proposal planner | ❌ | ✅ | 提案要先判断「这事落在哪个能力域」，正是发现场景 |
| dev-design planner | ✅ | ✅ | 写 design.md 要定落点 + 列依赖，模型也在这里被改 |
| dev-design evaluator | ✅ | ✅ | 审设计是否与模型一致 |
| test-design planner | ❌ | ✅ | 找被测模块的现有测试位置 |
| implement executor | ✅ | ❌ | 改文件时确认归属与可引用面（不需要发现新模块） |
| code-review evaluator | ✅ | ❌ | 核对改动是否越界 |
| acceptance evaluator | ✅ | ❌ | 验收确认架构未劣化 |
| test-gen / test-execution / archive SpecSync | ❌ | ❌ | 与架构无关，注入只是噪声 + 加 token |

### 5.4 模型修改：模型 = 源码

**变更的 worktree 允许 agent 直接修改模型**，用既有的 `write` / `edit`——不设专用写工具。改动随变更一起回合并**必须人工确认**（§4.5）。

```
change 的 worktree                         归档            主仓
├─ 源码 *.rs / *.ts                        ↓               ↓
└─ openspec/architecture/models/*.c4   ──▶ 人工确认 ──▶  ff_merge
   agent 用 write / edit 直接改              看模型 diff       权威
   （与改代码完全同权）                      （专用区块）
```

这条设计的两个支撑点：

* **不依赖运行时审批**。`loop.rs` 的工具分派对不合法调用只有 `deny`（合成 `SystemNotice` + is_error ToolResult，不中断 run），没有「挂起等用户点确认再继续」的能力。本方案不需要它——隔离层是 worktree，确认发生在归档，不是写盘那一刻。
* **设计权不靠写盘拦截**。worktree 的改动是 git diff 的一部分，与代码同等地进入人工评审；设计权体现在「回合必须人批」，而非「写盘必须拦」。

**有效性保证：写入时不校验，读取时报错**：

```
写入时        不校验（与写源码同等——编译器 / 检查器会告诉我们）
读取时        解析失败 → 返回结构化 errors，绝不 panic
门禁时        ArchiCheck 检出 structuralErrors → FeedbackLoop 回 executor 修
归档确认时    模型 diff 呈现（含结构错红字提示）
```

校验只发生在三处（读取 / 门禁 / 归档），不是「写盘前」这一处额外插口——少一个插口，少一套状态。代价是写坏模型期间架构视图显示结构错、门禁失败，这与「写坏 Rust 代码编译不过」同构。

**与 `write_protection` 的关系**：`models/` MUST NOT 纳入。`protect` 的规则是绝对路径 glob（`rules.rs` 的 `load` → `match_path`，cwd 只用于 bash 路径解析），加了会在 worktree 里同样命中，agent 就改不了模型。而且 `protect` 本来就不是安全边界（bash 可绕），它是防误触护栏；模型文件的真正护栏是归档时的人工确认。

**三类使用场景全部走 change**（没有「workspace 级直接改模型」的通路——模型 = 源码，源码也只能在 change 里改）：

| 场景 | 入口 | 落点 |
|---|---|---|
| 冷启动（模型为空） | 建一个 change（如 `init-architecture-model`），dev-design 阶段让 agent 生成骨架 | worktree → 归档回合 |
| 增量维护（新增/重构模块） | 正常 change 的 dev-design 阶段 | 同上 |
| 修正漂移（按现状补声明） | 同上，理由来自全量体检的 ② 清单 | 同上 |

冷启动因此也是「起一个 change」。看似重，但换来的是新模型同样走「设计 → 评审 → 合入」，而不是某天有人往主仓塞了个 `.c4`。新生成元素默认 `gate 'off'`——符合 ratchet 的逐块收紧。

---

## 6. 前端

复用既有画布与渲染资产，不新建可视化体系。**一个画布组件，两处挂载点**：

```
ArchitectureCanvas（共享组件，props = { root, mode }）
├─ 左：元素树（来自 extend 结构）              ← 复用 standard 组件
│       节点前置状态点：🔴 结构错 / 🟡 声明未兑现 / ⚪ 正常
├─ 中：xyflow 画布（节点 = element，边 = relation）
│       默认只渲染聚焦元素 1-hop，避免过载
│       边样式：实线=已兑现 灰虚线=声明未兑现 橙虚线=跨 kind 向上 灰=豁免
└─ 右：抽屉
    ├─ 元素概览（kind / metadata / path / gate 档 / modelFile:line）
    ├─ 相邻报告（最近一次 archi check 的 errors / warnings）
    └─ ADR 列表（markdown-doc-renderer 渲染 decisions/*.md）

挂载点 A  /architecture              root = workspace 主仓根，mode = 全量
挂载点 B  change 详情内的架构页签     root = detail.worktree，mode = 增量
          （归档 change 回落主仓根）
```

两处挂载点的能力差异（数据语义差异，不是 UI 差异）：

| 能力 | 全量（workspace 级） | 增量（change 级） |
|---|---|---|
| 结构校验（L1） | ✅ 毫秒级 | ✅ 毫秒级 |
| 文档关联 | ✅ | ✅ |
| ② 未声明依赖 | ✅ 全仓扫描 | ✅ 只扫变更文件 |
| ④ path 失效 | ✅ | ✅ |
| ① 声明未兑现 | ✅ 可判定（有全仓代码） | 🟡 只标观察项（无基准） |
| 改 `gate` 档 | ❌ 只读视图（提示「改档需要开一个 change」） | ✅ 编辑 worktree 的模型，随该 change 归档回合 |

**改 `gate` 档只能通过编辑模型文件**——也就是只能在某个 change 里做，因为模型 = 源码、源码只能在 change 里改。存量冲击不构成问题：该 change 的门禁只扫自己变更的文件，未触碰文件里的历史违例不扫描、不 fail（§3.3）。

数据面沿用 change 详情页那套：specta `Type` → `src/types/generated/bindings.ts` → `pnpm run bindings:check` 卡住漂移。命令面新增 `commands/archi/` 轨道（`archi_model` / `archi_check` / `archi_decision_list` 等薄命令，三件事纪律：参数转换 → 调用 → 错误映射）。**没有 `archi_write_model`**——模型的写走 agent 工具面与人工编辑，不进命令面。

---

## 7. 技术选型

### 7.1 依赖清单（锁文件实际状态）

| 需要什么 | 库 | 锁内状态 |
|---|---|---|
| 文件遍历 + gitignore | `ignore` 0.4.33（`WalkBuilder`） | 传递（经 tokei） |
| glob 匹配 | `globset` 0.4.20 / `glob` 0.3.4 | 传递 / 已是直接依赖（`checks::globmatch`） |
| 图算法 | `petgraph` 0.8.3（SCC / 拓扑排序） | 传递 |
| Rust crate 依赖 | `cargo_metadata` 0.19.2 | 传递（经 tauri-utils） |
| Rust 文件级 `use` | `syn` 2.x（`full` + `parse_file`） | 传递（各 derive 宏） |
| TS/JS 语法树 | `oxc_parser` + `oxc_allocator` | **真新增** |
| TS/JS 模块解析 | `oxc_resolver`（tsconfig paths / node 解析 / exports） | **真新增** |
| `.c4` 解析 | 手写子集解析器 | 新增（无现成库） |

`syn` / `cargo_metadata` 在锁内以 proc-macro / build-script 身份出现（host 编译），提为普通依赖会给 target 编译增加时间。`oxc_*` 是唯一实打实的体积新增，成本由 §8 实测量化。

**明确不用**（会破坏「不依赖本地环境、开箱即用」——`retire-cli-engine` 已把这条定为产品前提）：

```
madge / dependency-cruiser / ts-morph   → 要 node
cargo-modules / cargo-depgraph          → 要 cargo install 二进制
  （cargo-modules 的模块级依赖图与环检测最贴需求，可作「检测到则增强」的可选路径，
    但不能是必需依赖）
```

### 7.2 边界：库负责解析，我们负责归属

```
开源库给的（不要手搓）
├─ 词法 / 语法 → import / use 声明 + span          oxc_parser / syn
├─ 模块解析 → specifier 落到磁盘绝对路径           oxc_resolver
├─ crate 图 → package 名 / 版本 / 目录              cargo_metadata
├─ 图算法 → SCC / 拓扑序 / 邻接                    petgraph
└─ 文件枚举 → 遍历 + gitignore + glob              ignore / globset

只能自己写的（任何库都给不了）
├─ 「这个文件属于哪个 C4 元素」      ← metadata.path 语义是我们发明的
├─ 「这条 import 是否跨元素边界」    ← 需要 path 前缀归属 + 元素内豁免
└─ 「这个元素现在开没开门禁」        ← gate 档 ratchet 策略
```

`oxc_resolver` 使用口径：按 `config.json` 的 `tests[].root` 逐子树加载 tsconfig（`baseUrl` + `paths`），对每个 import specifier 调用一次 `resolve`。解析失败记 warning「路径不可解析」进 `scan.unresolved`，不进门禁（看不见的依赖不该冒充违例）。

### 7.3 Rust 侧的归属：必须深入到 crate 内部

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
④ 文件 → 元素           与 JS 完全相同的规则：metadata.path 最长前缀匹配
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

元素 `metadata.path` 的映射粒度与 JS 统一：可指 crate 目录（package 级）、crate 内子目录（module 级）、或单文件（component 级）。**同一个最长前缀规则，不因语言而异**——这是「两套解析器、一套归属逻辑」的分界点。

### 7.4 其他选型

| 领域 | 选型 | 状态 |
|---|---|---|
| 变更清单 | `ArchiveVcsPort::change_edit_set(root, base)`（`git diff <base> --name-only -z` ∪ `ls-files --others`） | 已有，唯一入口 |
| 模型冲突检测 | `vcs::unmerged_files()`（`diff --name-only --diff-filter=U -z`） | 已有 |
| 代码量 / 语言分布 | `tokei`（`code_stats` 既有） | 已有 |
| 存储 | 默认零持久化；确需落库才用 `store` 的 `native_model` 版本链 | 已有机制 |
| 前端画布 | `@xyflow/react` + change flow 既有布局 / 抽屉 / 运行态资产 | 已有 |
| 类型桥 | specta `Type` → `bindings:check` 守门 | 已有 |
| 模型写面 | 既有 `write` / `edit`（模型 = 源码，无专用写工具） | 已有 |

---

## 8. 产物标准（S1 验收阈值）

### 8.1 目的

| # | 目的 | 不做的后果 |
|---|---|---|
| 1 | **给回退序一个触发条件** | 「要不要砍 oxc」变成凭感觉的争论；实测数字是唯一裁决依据 |
| 2 | **量化唯一不可估的成本** | `ignore` / `globset` / `cargo_metadata` / `syn` / `petgraph` 都在锁内，成本可估；只有 `oxc_*` 是黑箱——特性开关默认会带进一堆小包，不测不知道 |
| 3 | **守住发版链与开发体感** | 一次 +90 s CI 冷构建直接吃「点发版到出包」的反馈时长；+15 s 本地增量乘上每相位多次的 `static_analysis` 就是分钟级浪费 |
| 4 | **留下可复核基线** | 半年后有人问「装包怎么这么大」，能翻到「哪次引入、多了多少、当时怎么判的」 |

第 2 条是核心：本方案唯一的设计风险集中在 `oxc` 三个 crate 的体积与编译成本上，其余选型都是「锁内已有，提为直接依赖」。

### 8.2 基线（HEAD `f5539bbe`，本机 Windows release 构建）

| 指标 | 基线值 | 出处 |
|---|---|---|
| 安装包（NSIS） | **6.68 MB**（`dev-team_0.5.26_x64-setup.exe`） | `src-tauri/target/release/bundle/nsis/` |
| 主二进制 | **26.6 MB**（`dev-team.exe`） | `src-tauri/target/release/` |
| `Cargo.lock` | 999 个包 / 11 548 行 | `grep -c '^name = "'` |
| CI 构建 | 双平台 matrix，各腿 `tauri build` 一次 | `.github/workflows/desktop-release.yml` |
| 相位预算参照 | `STATIC_CHECK_TIMEOUT = 1200 s` | `infra/checks/src/static_check.rs` |

参照系：0.5.4 → 0.5.26 十二个版本，安装包只涨 0.28 MB。所以「一次引入 +1.5 MB」已是常态增速的五倍。

### 8.3 阈值

| 指标 | 软线 | 硬线 | 超线动作 |
|---|---|---|---|
| NSIS 安装包 | +0.8 MB（≈ +12%） | **+1.5 MB（≈ +22%，即 ≤ 8.2 MB）** | 见 8.6 回退序 |
| 主二进制 | +3 MB | **+5 MB（≤ 31.6 MB）** | 同上 |
| `Cargo.lock` 净新增包数 | ≤ 25 | **≤ 40** | 引入面失控，先查特性开关 |
| CI 冷构建（单腿） | +45 s | **+90 s** | 回退序第 ① 步 |
| 本地增量（改 `archi` 一文件后 `cargo build`） | +8 s | **+15 s** | 查是否误引重特性 |
| 单次 `archi_check` 运行 | 1.5 s | **3 s**（300 元素 + 200 变更文件量级） | 加并发 / 加进程内缓存 |

`Cargo.lock` 净增包数单独设线，是因为它比体积更早暴露问题：`oxc` 家族按特性会拉进 `compact_str` / `rustc-hash` / `simdutf8` / `bitflags` 等一串小包，净增超 40 基本可断定某个默认开关把不需要的东西带进来了。

### 8.4 执行方式

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

**D. 运行期（与构建解耦）**：`crates/core/archi/tests/perf.rs`，release 跑，语料 200 变更文件 + 300 元素，10 次取 p95；计时用 `std::time::Instant`，不引 criterion（一次性门禁用不上统计框架）。

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

### 8.5 不常态化跑 CI

共享 runner 的冷启动、磁盘 IO、CPU 差异都能吃掉本方案的差值量级（零点几 MB / 几十秒）。常态跑会得到「天天告警但没人信」的结果，反而消耗阈值公信力。

如需自动化：加一条 `workflow_dispatch` 手动触发的 workflow，输出实测表作为工件，不进 PR 门禁（与 `desktop-release.yml` 的 `force` 输入同类）。

### 8.6 超标回退序

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

## 9. 语料与 Spike

任何解析器都必须先有语料、再有实现。语料覆盖历史 `.c4` 的真实写法：注释、多文件合并（`specification` 只取一次）、`metadata` 单值与数组两种形态、`extend` 嵌套、关系描述引号、空模型与非法模型。

```
crates/core/archi/tests/
├── corpus/
│   ├── dsl/<case>/*.c4                 输入模型
│   ├── src/<case>/**                   输入代码（import / use 语料）
│   └── expected/<case>.json            期望（elements / relations / gate / 诊断 / 报告）
├── golden_test.rs                      对拍（与 crates/core/checks/tests/golden 同风格）
└── perf.rs                             运行期 p95（§8.4 D）
```

### 9.1 DSL 语料

注释、多文件合并（`specification` 只取一次）、`metadata` 单值与数组、`metadata` 带 `gate`、`extend` 嵌套、关系描述引号、空模型、缺 specification、非法 kind。

### 9.2 结构校验语料

| 用例 | 期望 |
|---|---|
| `A -> B` 且 `B -> A` | error：成环（SCC 输出环路径） |
| `A -> anc(A)`（A 被 anc 用 `extend` 包含） | error：依赖自己的祖先 |
| 关系引用不存在的元素 | error：未定义元素 |
| 同 kind 同名重复声明 | error：元素重复 |
| 无环有向图 + 合法祖先关系 | clean |
| 命令面写前校验 vs 工具面写后发现 | 前者拒绝写入；后者写盘成功、读取报错、门禁 fail |

### 9.3 交叉核对语料（门禁）

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
| 存量模型含 `A -> anc(A)` | — | 照常加载（`model.structuralErrors` 记录）+ 门禁判 error |
| 模型为空 | — | status = skipped |
| 无变更文件（增量） | — | status = noChanges |
| 全量模式、仓库无源文件 | — | status = noChanges |

### 9.4 文档关联语料

| 用例 | 期望 |
|---|---|
| `metadata.docs` 指向存在的文件 | `docs` 含该条目，`source: "explicit"`，排第一 |
| `metadata.docs` 指向不存在的文件 | `model.warnings` 有 `docNotFound`；`docs` 不含该条目 |
| 元素 `path` 目录下有 `README.md` | `docs` 含该条目，`source: "adjacent"` |
| `path` 目录在测试夹具内（`__tests__/`） | 其下 md 不出现 |
| ADR 正文提及元素 FQN | `docs` 含该 ADR，`source: "text"` |
| 同一文件被显式与邻近同时命中 | 合并为一条，`source` 取强者（`explicit`） |
| 元素无任何关联文档 | `docs: []`（不报错、不告警） |

### 9.5 检索语料（轻量版）

| 用例 | 期望 |
|---|---|
| `query = "auth"` | 命中 FQN 含 auth 的元素 + 文件名含 auth 的文档 |
| `query = "AuthToken"`（驼峰） | 命中 `App.Auth.Token`（分词后 auth / token 均匹配） |
| `query` 命中文档正文但不在路径 / FQN 中 | **不命中**（不做全文搜索，刻意） |
| `query` 命中 `openspec/specs/` 内内容 | **不命中**（§2.7 边界 2） |
| `scope = "elements"` | `docs` 恒空数组 |
| `query` 为空串 | `Err`（参数校验） |

### 9.6 Rust 模块树语料（§7.3）

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

### 9.7 模型编辑与归档确认语料

| 用例 | 期望 |
|---|---|
| agent 在 worktree 用 `write` 新建 `models/03-billing.c4` | 写入成功（`models/` 不在 `write_protection` 内） |
| 同上提交后归档 | 归档确认面出现架构变更区块（元素 +N） |
| agent 写坏 `.c4`（语法错） | 写盘成功；`archi_model` 返回结构错不 panic；ArchiCheck fail 交 FeedbackLoop |
| agent 写出成环 | 同上 |
| 模型有改动但代码无改动 | 归档确认面仍出现架构变更区块 |
| 模型无改动 | 归档确认面不出现该区块（零占位） |
| `archi_module` 返回 `modelFile` + `modelLine` | 指向声明该元素的文件与起始行，可直接用于 `edit` |
| 模型改动与主仓冲突（rebase） | 中断交人（§4.4），不进 agent 解算 |
| 归档确认「查看新图预览」 | 画布渲染改动后的模型图 |

### 9.8 全量 vs 增量一致性

| 用例 | 期望 |
|---|---|
| 同一 commit，全量与增量都跑 | 增量报告 ⊆ 全量报告（同一模型下，增量只覆盖变更文件） |
| 模型声明 A→B 但代码无 import | 两模式都进 `observations`；`status` 都不因它变 |
| 全量模式仓库无源文件 / 无模型 | status = noChanges / skipped |

### 9.9 Spike 计划

| Spike | 内容 | 验证什么 |
|---|---|---|
| **S1 解析 + 结构校验** | §9.1 / §9.2 语料；`petgraph` 做 SCC 与祖先链判定；解析失败返回结构化 errors 不 panic；**同时按 §8.4 出产物实测表（开发机）** | 解析语义 + 图算法 + 依赖成本量化；零 UI、零门禁 |
| **S2 交叉核对 + 报告** | `change_edit_set` → `oxc_resolver` 归属 → 对账 → `reports/archi/*.json`；前端一格展示 | 归属判定、gate 档定级、豁免机制、报告槽 |
| **S3 门禁挂载** | `StepNode::ArchiCheck` 挂 implement → 真实 change 跑出 fail → FeedbackLoop 回 executor → 超限升格；旧 db 验证存量 run 仍可读 | 相位表接线 + bincode 尾部追加纪律 + 端到端手感 |
| **S4 文档关联与检索** | §9.4 / §9.5 语料；两个工具在 dev-design 与 implement 场景各跑一次真实会话 | 关联发现、轻量检索、按场景注入是否真的省 token |
| **S5 Rust 归属** | §9.6 语料；以本仓自身为靶（13 个 workspace crate） | 模块树构建 + `use` 最长前缀匹配在真实 crate 上是否可靠 |
| **S6 冷启动闭环** | §9.7 语料；空模型仓库起一个 change，agent 在 worktree 生成模型 → 归档确认看到 diff → 合回主仓 → 主仓视图建图 | 「空图起点」能否落地；归档确认面的模型区块够不够看得出问题 |

---

## 10. 硬约束清单

| 约束 | 出处 | 违反后果 |
|---|---|---|
| `RunStepKind` / `StepKind` 新变体尾部追加 | `workflow/state.rs` bincode 下标落库 | 存量 run 节点行 / 审计行步词整体错位 |
| `workflow` 与 `archi` 互不依赖 | `desktop-crate-layout` spec | crate 图破，纯读域引入编排行为 |
| core 禁 Tauri、core 禁 store | 同上 | 解析域被持久化 / 壳层污染 |
| 七工具封闭集、下发面单源 | `tools.rs` `TOOL_NAMES` / `policy.rs` / `compose.rs` / 16 份 prompts | 越权工具幻觉可执行；prompt 逐字断言测试红 |
| 显式取数、无 watch 无轮询；watch 仅单文件信号不含内容 | `desktop-file-watch` spec | 画布想自动刷新必须先改 spec，不能偷加目录监听 |
| 新解析器要带语料金标准 + 过变异门槛 | `checks` 的 `tests/golden` 文化 + `config.json` 的 mutation 80/70 | 变异分不达标，流程在 test-execution 相位卡死 |
| `config.json` 已知键七项，其余进 `extra` | `core/config/src/lib.rs` `KNOWN_KEYS` | 新增配置不登记 → 静默 passthrough，config 页不展示不校验 |
| 架构域配置只落 `openspec/architecture/config.json` | §2.6 | 配置来源分散，用户不知道改哪 |
| 每 workspace 独立 db、模型演进走 `native_model` 版本链 | `infra/store` | 若架构结论要落库，必须走版本段而非改字段 |
| 架构扫描排除规则与 `tests[].excludes` 正交 | `config` crate 的 `TestSuite` 语义 | 调测试 scope 会意外改变架构扫描范围 |
| 豁免只到元素对粒度，禁 glob | §2.4 | glob 豁免 = 永久后门，ratchet 失效 |
| `archi` 目录字面量只经 `foundation::layout::archi_dir` 派生 | §4.2 | 零散 `"architecture"` 字面量散进各 crate，改名即漂移 |
| 模型 = 源码：agent 在 worktree 内可自由改 `.c4`，但回合必须人工确认 | §5.4 | 设计权旁落；模型偷渡进主仓 |
| 归档确认面 MUST 单列模型 diff | §4.5 | 模型改动混在几十个代码文件里被橡皮图章 |
| `models/` MUST NOT 纳入 `write_protection` | §5.4 | 规则是绝对路径 glob，worktree 内同样命中，agent 就改不了模型 |
| 模型解析失败 MUST 返回结构化 errors，绝不 panic | §5.4 | 写坏模型期间整个架构视图崩溃 |
| 模型文件冲突 MUST NOT 进 agent 解算 | §4.4 | LLM 替人做架构裁决，冲突解错无人能察 |
| 归档 Preflight MUST 检出 rebase 半程并挡回 | §4.4 | 重跑会在 rebase 中间态 commit，污染历史 |
| change 期一切架构读写只碰 worktree 那棵模型 | §4.4 | 读到主仓基线模型，门禁永远滞后一个 change |
| `oxc_parser` + `oxc_allocator` + `oxc_resolver` 是唯一真新增依赖 | `Cargo.lock` 现状 | 成本由 §8 实测量化；超硬线按回退序 |

---

## 11. 未决问题

1. **`architecture/config.json` 是否纳入 `write_protection`**——它是看护策略（豁免列表），被随手改会静默放宽门禁。但同样的 glob 问题适用：加了会连 worktree 一起拦。倾向不加，靠归档确认面的 diff 可见性兜底。

2. **归档确认 UI 的信息密度**——§4.5 的三层（摘要 / `.c4` diff / 新图预览）够不够？要不要再加「哪些代码文件改动了这些元素覆盖的路径」这类交叉信息？留到 S6 实测后定。

3. **`.c4` 语法的演进与容错**——未来加新 metadata key / 新 kind 时旧模型怎么办。倾向「未知 metadata key 忽略、未知 kind 报错」（与 DSL 严格性一致），写 spec 时定。

4. **性能语料最终量级**——§8.3 定「300 元素 + 200 变更文件」，若真实工程远超此量级需重估 3 s 硬线。

---

## 12. 不在本方案范围（需独立 change）

1. **「模块文档替代 specs」的迁移**——§2.7 已列清难点：specs 承担六项职责，其中**可验收断言（Scenario）**与**归档 SpecSync 的增量合并语义**没有散文形态的对应物。迁移若要成立，模块文档需继承可断言结构，或接受「SpecSync 步退役、归档链缩短」——两者都是 workflow 级变更。

2. **`archi` 与 specs 的最终关系**——若 specs 收缩到「少数主要特性文档」，这些文档是「特性视角」而非「模块视角」，可能仍需在架构视图里被引用。届时再裁决，本方案不预设。

3. **跨语言依赖**——本方案只做语言内闭环（Rust 侧、TS/JS 侧各自解析、一套归属逻辑）。跨语言（TS 调 Rust 的 Tauri 命令）不在本期。

4. **符号级契约核对**——方案只到「文件 / 元素」粒度。若将来要「函数签名 vs 契约」的碰撞，需要 `syn` 的 item 级解析 + `oxc` 的 AST 语义层，属独立能力。
