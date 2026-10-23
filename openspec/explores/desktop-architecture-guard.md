# 桌面端架构看护系统方案（Local Architecture Guardrails · packages/desktop）
workflow_type: requirement

---

## 0. 范围与前提

本方案只在 `packages/desktop` 一个包内落地：Rust 侧 crate 分层 + Tauri 壳 + React 前端。全部能力自含于桌面端，不依赖任何外部进程型工具链或宿主侧集成，`.c4` 解析、图计算、门禁判定、报告落盘与可视化均由桌面端自有 Rust / TS 代码承担。

三条直接从现状推导出的前提：

1. **模型解析必须原生**：桌面端 agent 工具面是封闭七件套（`crates/infra/agent/src/sdk/tools.rs` 的 `TOOL_NAMES`），会话的 `mcp_servers` 恒为空（`sdk/loop.rs` 铸 `Vec::new()`，`runner_test` 断言恒空）。没有任何 MCP / node 子进程通道可借来解析 DSL。
2. **门禁只能落在相位编排里**：桌面端唯一的刚性拦截形态是 `phase_table` 的 `StepNode` + `OnFail::{Terminal, FeedbackLoop}`（`crates/core/workflow/src/write/phase_table.rs`）。不存在、也不会引入 git hook 型提交拦截。
3. **取数是显式的**：`desktop-file-watch` spec 把「显式取数、无 watch 无轮询」定为纪律，watch 是唯一例外且限「单文件、信号不含内容、前端防抖、后端零去抖」。架构图与门禁结论都走显式命令，不做常驻守护进程。

目标是消解「设计图 / 契约文档 / 真实源码」三者之间的**架构漂移**：模型声明什么，代码就得是什么；漂移在相位门禁处被静态算法拦下，剩下的语义重构交给相位 agent。

---

## 1. 设计原则

* **声明式单一事实源**：架构拓扑与模块边界完全由 `openspec/architecture/models/*.c4` 决定，纯文本、git 友好、可 review。
* **不造第二套契约**：契约层复用既有的 `openspec/specs/<capability>/spec.md`（规格）与 `openspec/architecture/decisions/*.md`（ADR）。本方案不引入「每模块一份 md + 语言无关 YAML API schema」这层中间物。
* **三级判定，各管各的**：声明阶段管模型自洽（拒绝写入）；相位门禁管代码与模型一致（fail 可修复）；架构视图管设计意图的观察（不拦流程）。混级就会把「还没写」误判成「写错了」。
* **级别二值**：报告只有 `errors` / `warnings`，定级由元素的 `gate` 档决定，不由问题类型决定（`violation` 这类类型自带严重性的写法淘汰）。
* **刚柔分层**：可判定的问题用静态算法做刚性拦截（零 LLM、确定性、可复现）；「这样拆对不对」「怎么重构」交相位 agent。
* **模型变更必须人工确认**：agent 无自主改模型权（只读 `archi_module`）；模型文件冲突不进 agent 解算，一律中断交人。架构是设计决策，不能由 LLM 代决。
* **零持久化优先**：模型每次全量解析进内存（与 `code_stats` 每次重新 `tokei` 解析同哲学），不引入第二套存储引擎，不做隐式缓存失效管理。

---

## 2. 实体模型与映射

原方案的四层「LogicalModule / ModuleDocument / FileMapping / ApiMapping」在桌面域内收敛为三层：

```
【 拓扑层 】  C4 element（kind: package / domain / module / component）
              ├─ extend <parent> { … }   ⇒ Contains（嵌套包含，来自 DSL 结构本身）
              └─ A -> B "desc"           ⇒ DependsOn（依赖关系，来自显式关系声明）
                    │
                    ▼  语义锚点
【 锚定层 】  metadata { path ['./src/x/', './src/y.ts'] }   ⇒ 1:N 物理边界
                    │
                    ▼  交叉核对
【 代码层 】  变更清单（vcs：change_edit_set）∪ import 解析  ⇒ 事实依赖边
```

三处关键取舍：

| 项 | 取舍 | 理由 |
|---|---|---|
| 逻辑模块 | **C4 element 即模块**，不另建 LogicalModule 类型 | `c4-dsl-syntax` 已定义四类 kind 的严格层级（`extend` 表包含、`->` 表依赖），父子关系已在 DSL 内自描述，另立一层只会双写 |
| 物理映射 | **`metadata.path` 即 FileMapping**，且天生是数组 | 一个元素可覆盖多个路径（目录 + 单文件混合），无需新字段 |
| API 契约 | **不落权威 schema**。需要时从源码提取「对外符号清单」作为派生视图，不写回磁盘 | 契约权威在 specs + ADR；再造一份语言无关 schema 必然与 specs 漂移，且要维护 Rust/TS 两套提取器的对齐 |
| 模块文档 | **不规定形态，只做关联与检索**（详见 §3.6.2） | 文档该写什么因团队而异；钉死 schema 会把「还没想清楚」变成「落后于文档」。只要能定位到，正文怎么写是作者自由 |

### 2.1 门禁档位：用 `metadata` 表达 ratchet，零 DSL 变更

`metadata` 是自由 key-value（解析器已支持任意 key 与数组值），所以「按空间收紧」不需要新语法、不需要新配置项：

```
metadata { path './src/auth/' }                        ← 缺省 off，不纳管
metadata { path './src/order/', gate 'strict' }        ← 该元素严格
metadata { path './src/legacy/', gate 'warn' }         ← 只提示不拦
```

| 档位 | 参与归属 | 被引用时的归因 | 反向（谁 import 我） |
|---|---|---|---|
| `off`（缺省） | 否——其文件算 unmatched，只计数不报错 | 不报 | 不报（否则 off 元素无法与任何 strict 元素相邻） |
| `warn` | 是 | warning | 由调用方档位决定 |
| `strict` | 是 | **error** | 由调用方档位决定 |

严重度归因规则：**只看引入方的档位**（谁引入了不该有的依赖，谁负责），避免一个 strict 元素把全仓库的 off 元素拽进门禁。

存量债务由此自然消失：旧元素保持 `off`，想收紧的模块逐个改 `strict`，一个 change 收紧一块。`gate` 键的取值非法时按 `off` 处理并出诊断（与 `config` crate 既有的「非法值降级 + diagnostic」同风格）。

### 2.2 扫描范围：测试文件全排除

架构扫描 MUST 排除所有测试文件与测试夹具。**不复用 `config.json` 的 `tests[].excludes`**——那是测试执行 scope（可因 e2e 慢而排除、因 fixture 需要而包含），与架构扫描范围正交，复用会让两个概念隐式耦合。

```
默认约定（可被 architecture 配置覆盖）
  **/*.test.{ts,tsx,js,jsx}   **/*.spec.{ts,tsx,js,jsx}   **/__tests__/**
  **/*_test.rs                **/tests/**                 **/fixtures/**   **/__mocks__/**
```

理由不止「噪声」：测试文件天然跨元素引用被测代码，若纳入扫描，每一个 `foo.test.ts` 都会对 `foo.ts` 产生一条未声明依赖——这不是架构问题，是测试的固有形状。

### 2.3 豁免机制：两层，第一层零成本

```
第 1 层  声明即豁免（语义层，零配置）
         模型里写了 A -> B，这条边即合法。所以「这条依赖是对的」的正确
         动作是补声明，不是加豁免——② 的定义天然是「未声明的才是问题」。

第 2 层  忽略规则（噪声层，仅用于「不能 / 不该声明」）
         内联  // archi-ignore-next-line: 迁移期临时直连，见 ADR-0007
               → 用 import 声明的 span 行号回原文取该行 / 上一行做注释匹配
                 （不读 AST 的 comment trivia，最稳）
         配置  architecture.ignore: [{ from, to, reason }]
               → 结构性、可审计、不进业务代码；reason 必填
```

第 2 层 MUST NOT 支持 glob 粒度（如 `src/legacy/**`）——glob 豁免等于开一个永久后门，ratchet 就白做了。只允许 `(from 元素, to 元素)` 元素对。

### 2.4 两个配置文件的边界

```
openspec/config.json                  既有，桌面 config crate 唯一出口
├── schema / context / rules          相位 planner 的自然语言规则
├── static_analysis                   静态检查命令
├── tests[]                           测试执行 scope（root / framework / includes
│                                     / excludes / coverage / mutation）
└── write_protection                  写保护

openspec/architecture/config.json     新增，架构域自有
├── ignore: [{ from, to, reason }]    元素对豁免（reason 必填）
└── exclude?: [glob]                  扫描排除覆盖（缺省用 §2.2 约定）
```

为什么拆两个文件而不是把 `architecture` 键塞进 `config.json`：`config.json` 的 `KNOWN_KEYS` 是七项白名单，加键要同步改 `KNOWN_KEYS` / `WorkspaceConfig` / 解析分支 / 前端 config 页 / 两份 schema，而架构域的配置与「测试怎么跑、流程怎么流」无关，属于另一个关切面。拆开还有个副作用好处：`architecture/config.json` 可以随模型一起被 `archi` crate 自解析，不必让 `config` crate 认识架构语义（保持 `config` 是「工作区配置」而非「万物配置」）。

两个文件的管辖面必须在 spec 里写死，条款大致是：`config.json` 管**执行**（命令、scope、阈值、保护），`architecture/config.json` 管**看护策略**（豁免、扫描排除）。MUST NOT 出现第三个配置文件。

### 2.5 架构域目录

架构域的一切落在一个目录下，不散落：

```
openspec/architecture/
├── models/*.c4                模型（结构 + metadata.gate + metadata.path）
├── decisions/*.md             ADR
├── config.json                豁免与扫描排除（§2.4）
└── reports/*.json             全局报告（change 内的报告仍落 change 的 reports 槽）
```

「模型与它的配置、决策、报告同处一目录」的意义是可整体搬动 / 整体 review：一次架构重构的 diff 全落在这一个目录里，而不是跨十几个位置。

---

## 3. 桌面端落位

### 3.1 crate 图（依赖方向已由 spec 预铺）

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
* `crates/core/orchestration` 增第四个 runner trait（见 3.3），使 `workflow` 能挂载门禁步而不认识 archi。
* 若将来需要 `cargo metadata` 之类进程解析，实现落 `crates/infra/archi`（新建 infra 成员），core 仍只见 trait。

### 3.2 目录常量（foundation）

`foundation::layout` 目前只有 changes / archive / explores 三棵树（`Layout` struct 三字段 + `config_path` + 报告槽派生）。新增四个**派生函数**，不动 `Layout` struct（零构造点波及）：

```
archi_dir(root)            → openspec/architecture              （域根，前缀单点）
archi_models_dir(root)     → <archi_dir>/models
archi_decisions_dir(root)  → <archi_dir>/decisions
archi_config_path(root)    → <archi_dir>/config.json            （§2.4 的豁免与排除）
archi_reports_dir(root)    → <archi_dir>/reports                （全局兜底位置）
```

按 foundation 的「第二个消费者到场才下沉」纪律，这些项落地时需在 `desktop-crate-layout` 的 spec 里登记为新增消费者。`openspec` 域根常量已存在（`domain_dir_name`），`architecture` 段由 `archi_dir` 单点持有，其余全部从它派生——零散字面量不进任何 crate。

### 3.3 门禁协议（orchestration::port）

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

产出走既有的 `ToolStepOutput` 封闭集（新增一个 `ArchiCheck` 变体），`workflow` 侧零 archi 依赖。

### 3.4 门禁挂载点与判定表

**判定分三层，只有中间一层进门禁**：

```
[L1] 声明阶段（写 / 改 .c4）— DSL 校验 + 结构 Lint
     语法错 / 缺 specification / 成环 / A -> anc(A) / 元素重复
     → 拒绝写入并返回诊断（画布连边与 agent 改模型走同一条校验路径）
     特点：模型内部自洽性，不需要读一行代码

[L2] 相位门禁（implement）— ArchiCheck 交叉核对
     漂移② 未声明依赖        引入方 warn → warning / strict → error
     漂移④ 元素 path 失效     该元素 off → 不检查 / warn → warning / strict → error
     特点：要读代码，要 vcs 变更清单

[L3] 架构视图（不进门禁）— 漂移可视化
     漂移① 声明了但代码未兑现 → 画布 🟡
     跨 kind 向上的边          → 橙色虚线（提示价值，非约束）
     特点：设计态的观察面，答案往往是一个新 change
```

**漂移① 不进任何门禁**。理由：① 是「设计已画、代码还没写」，它的正确回应是继续实现或开新 change，而不是让门禁拦下当前 change。放在架构视图里由人判断「这叫欠账还是叫计划」。

**acceptance 相位也不挂 ArchiCheck**：模型一致性已在 implement 收敛；acceptance 关注验收行为。架构问题按一般重构转新 change 验收。

相位表（requirement 10 相位 / test-only 7 相位）：

```
requirement 表（10 相位）
initialize     PhaseStart → Clean → StaticCheck(Terminal) → TestExecution(Terminal) → PhaseLog
proposal       PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog
dev-design     …（模型在这里被改写）
test-design    …
implement      PhaseStart → Executor → StaticCheck(FeedbackLoop{5})
               → ★ArchiCheck(FeedbackLoop{3}) → Evaluator → VerdictGate(FeedbackLoop{5}) → PhaseLog
test-gen       …
test-execution …
code-review    …（无 executor，仅 evaluator）
acceptance     …
archive        Preflight → Commit → Rebase → ResolveConflicts → Merge
               → SpecSync → Cleanup → Seal → Finalize

test-only 表（7 相位）：initialize → proposal → code-analyze → test-design
                        → test-gen → test-execution → archive
```

**为什么用 `FeedbackLoop` 而不是 `Escalate`**：② 的成因有两种且无法自动区分——

```
代码写了不该写的依赖      → 改代码（executor 的活）
模型声明落后于真实设计    → 改模型（该走 dev-design 回溯）
```

缺省权威是模型（声明式驱动的本意），所以第一反应应该是「让代码符合模型」：executor 拿到具体违例（哪个文件 import 了哪个元素）去改。而 `FeedbackLoop` 在超限时**本来就升格相位失败**（`walker.rs` 的 `step_static_check`：`feedback >= limit` → 代写 fail phase_log → `StepFlow::Upgraded` → 回 `phase_next` 进决策分叉），所以「先修代码、修不动就交决策裁决是否改模型」这条路径不需要新 `OnFail` 变体，`FeedbackLoop { limit: 3 }` 自带。

变体追加纪律（必须遵守）：

| 枚举 | 位置 | 编码 | 追加方式 |
|---|---|---|---|
| `RunStepKind`（21 值） | `workflow/state.rs` | bincode 枚举下标落库 | **只能尾部追加**，中间插入令存量 run 节点行整体错位（源码有明文警告） |
| `StepNode`（21 值） | `workflow/state.rs` | 仅相位表内存结构，不落库 | 位置自由，仍建议尾部追加保持一致 |
| `StepKind`（审计八臂） | `workflow/state.rs` | 落 `StepRecord`（`native_model` id=12） | 尾部追加 + `StepRecord` 走版本链 |
| `ToolStepOutput` | `orchestration/port.rs` | 进程内封闭集 | 尾部追加 |
| `ReportSlot` | `foundation/layout` | 纯进程内枚举（零 serde / 零 specta） | 加第三值 `Archi` → `reports/archi` |

报告槽选择说明：不复用 `Test` 槽靠文件名前缀区分——`reports/test` 的 `plans[]` 索引语义、覆盖率复用判定、mutation scope 都挂在槽位上，混入架构报告会让三类语义互相污染。加第三槽的成本只是一个 match 臂。

### 3.4.1 报告结构

两个面共用一套结构：`reports/archi/<timestamp>.json`（落盘，审计面）与 `ArchiCheckOutcome`（进程内，门禁面）。报告是唯一事实源，`ArchiCheckOutcome` 从它派生 `passed` / `diagnostics` 两个门禁消费字段。

```jsonc
{
  "version": 1,
  "timestamp": "2026-10-20T08:31:12Z",
  "root": "/abs/path/to/worktree",          // 被检查根（可区分主仓根 / worktree 根）
  "change": "add-billing-gate",             // change 磁盘名；主仓全域检查时为 null
  "base": "a1b2c3d",                        // 变更清单基线（change 的 base_commit）
  "modelHash": "sha256:…",                  // models/*.c4 聚合内容的哈希（判"模型是否变过"）

  "model": {
    "elements": 24,
    "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [                   // §3.4 [L1] 结果——读存量模型遇错也照录
      { "kind": "dependsOnAncestor", "element": "App.Auth.Token", "ancestor": "App.Auth",
        "message": "元素依赖自己的祖先（extend 包含关系与 -> 方向矛盾）" }
    ],
    "warnings": [
      { "kind": "pathNotFound", "element": "App.Billing", "path": "./src/billing/",
        "gate": "strict", "message": "metadata.path 指向不存在的目录" },
      { "kind": "noPath", "element": "App.Legacy", "gate": "off",
        "message": "元素未声明 metadata.path（不参与归属）" }
    ]
  },

  "scan": {
    "changedFiles": 17,                     // change_edit_set 原始数量
    "scannedFiles": 12,                     // 排除测试后实际扫描数
    "excluded": [ { "file": "src/a.test.ts", "rule": "**/*.test.ts" } ],
    "unmatchedFiles": ["docs/notes.md"],     // 无元素认领（off 元素覆盖的文件也在此，只计数不报错）
    "matched": [ { "element": "App.Billing", "files": ["src/billing/x.ts"] } ],
    "unresolved": [                          // oxc_resolver 解析不到的 specifier（不冒充违例）
      { "file": "src/billing/x.ts", "specifier": "@weird/alias", "message": "路径不可解析" }
    ]
  },

  "errors": [                                // 门禁唯一定级依据：非空即 fail
    {
      "kind": "undeclaredDependency",
      "from": "App.Billing",
      "to": "App.Auth",
      "gate": "strict",                      // 引入方档位（定级依据）
      "occurrences": [                       // 按 (from,to) 聚合，不按行铺开
        { "file": "src/billing/x.ts", "line": 3, "specifier": "../auth/token", "typeOnly": false },
        { "file": "src/billing/y.ts", "line": 1, "specifier": "../auth/token", "typeOnly": true }
      ]
    }
  ],

  "warnings": [ … ],                         // 同 errors 结构（gate=warn 的②）+ 未声明目标 + 解析失败
  "suppressed": [                            // 被豁免抑制的违例也留痕（可审计，不静默）
    { "from": "App.Billing", "to": "App.Legacy", "via": "config",
      "reason": "迁移期临时直连，见 ADR-0007" },
    { "from": "App.Billing", "to": "App.Auth", "via": "inline",
      "file": "src/billing/x.ts", "line": 2, "reason": "存档迁移，见 ADR-0011" }
  ],

  "status": "errors"                         // clean | errors | warnings | noChanges | skipped
}
```

几条设计要点：

**`status` 五值**：`skipped`（无模型）与 `noChanges`（无变更文件）不是「通过」，要在 UI 与门禁日志里显式区分——否则「没检查」会被误读成「检查过了没问题」。`errors` / `warnings` 二值直接对应门禁结论。

**`occurrences` 聚合**：按 `(from, to)` 聚合而非按行铺开，是因为 FeedbackLoop 的 prompt 有长度上限（`MAX_REPORT_CHARS` 同源先例），一个元素对在大型仓库里轻易几十处。给 executor 看的是「Billing 依赖了 Auth，共 2 处，在 x.ts:3 / y.ts:1」，不是 40 行流水账。

**`suppressed` 必留痕**：豁免不等于不存在。报告里保留被抑制的条目 + 理由 + 来源（`config` / `inline`），这样「豁免了什么」是可审计的，也让架构视图能把豁免渲染成灰色边。没有这个字段，豁免就是隐形的技术债。

**`modelHash`**：用来判断「模型变没变」。因为零持久化（每次全量解析），没有缓存需要失效；这个哈希的用途是让前端能判断「上次看的报告是不是当前模型生成的」，以及让 `reports/archi/` 里的历史报告可追溯到具体模型版本。

**`structuralErrors` 只录不拦**（对应已定案 14）：读存量模型遇到 `dependsOnAncestor` 时照常加载，报告里记录，门禁判 error。这样「修模型」这件事本身也走 change 流程，而不是被一个打不开的页面挡住。

**`ArchiCheckOutcome` 的派生**：

```rust
pub struct ArchiCheckOutcome {
    pub passed: bool,             // errors.is_empty() && structuralErrors.is_empty()
    pub diagnostics: String,      // 从 errors + structuralErrors 渲染的反馈文本（截断到 MAX_REPORT_CHARS 同量级）
    pub fault: Option<RunFault>,  // 无环境错误面（纯内存解析），此字段恒 None，保留结构一致
    pub report_dir: String,       // reports/archi
    pub report_ref: Option<String>, // 报告文件路径（审计回查）
}
```

注意 `fault` 恒 `None`：archi check 不 spawn 任何进程（`oxc_resolver` 是纯内存解析），没有「命令挂死 / 文件锁」这类环境错误面。保留字段是为了与 `StaticCheckOutcome` 同形，让 `walker` 的分派代码不必为第四个 runner 写特例。

### 3.4.2 模型在两个根之间的流转

模型文件随 git 提交，主仓是权威；worktree 从 `base_commit` 拉出，天然带一份副本。change 期间 **读的一定是 worktree 那棵**——否则门禁判的是「基线模型 vs 新代码」，永远滞后一个 change。

```
主仓 /openspec/architecture/models/*.c4       ← 权威，随 git 提交
   │  git worktree add (base_commit)
   ▼
worktree /openspec/architecture/models/*.c4   ← change 期的工作副本
   │
   ├─ 声明阶段校验 / 相位门禁 / 架构视图  全部读这一棵
   │
   └─ 归档：Commit → Rebase → …→ ff_merge 合回主仓
                          ▲
                          └─ 模型文件冲突 → 中断，交用户处理（不进 agent）
```

`git.rs` 已有 `unmerged_files()`（`git diff --name-only --diff-filter=U -z`），冲突清单本来就是既有产物；`ResolveConflicts` 步拿到 `cursor.rebase_conflicts` 后，**先按路径分流**：

```
pending = cursor.rebase_conflicts
   │
   ├─ 含 openspec/architecture/ 下任何路径  →  Terminal::failed
   │      文案：模型冲突需人工裁决，请在 <worktree> 处理模型后重跑归档
   │      理由：模型是设计权威，agent 自动解算等于让 LLM 替人做架构决策
   │
   └─ 全部是代码路径  →  既有 agent 解算流程（零改动）
```

这个分流是**唯一新增的归档判定**，且是纯判定（路径前缀匹配 + 终止），不改 `ResolveConflicts` 现有的会话 / 后验 / lean 收敛任何一步。副作用：模型改动与代码改动在同一个 change 里做时，改动模型那一侧会让归档必然需要人工介入——这是刻意的（模型变更是设计变更，值得一次人工确认）。

#### 保留半程：不 abort，但重跑前必须挡住

「保留半程」恰好是既有语义的自然延伸——`git.rs::rebase_branch` 的注释已写明「冲突态保留于 worktree——MUST NOT 自动 abort」，只有**非冲突失败**才尽力 abort。所以模型冲突这条路径不需要新的 vcs 能力，只需要在 walker 侧不调用 abort。

但保留半程带来一个必须处理的**重入陷阱**：

```
T0  Rebase 步：git rebase → 冲突，worktree 处于 rebase 半程（REBASE_HEAD 在场）
T1  ResolveConflicts：检出模型路径 → Terminal::failed（不 abort）
    用户手动在 worktree 内解模型冲突 + git rebase --continue（或 --abort 放弃）
T2  用户重跑归档 → 从 Preflight 开始重来
        │
        ├─ Commit 步：dirty() 为真 → commit_all()：git add -A + git commit -m
        │      ⚠ 若用户尚未完成/放弃 rebase，此处在 rebase 中间态提交
        │        ——轻则 "cannot commit because you have unmerged files"（报错不可读）
        │        ——重则把冲突标记 commit 进历史
        │
        └─ 所以必须在 Preflight 就挡住
```

对应改动：**`step_preflight` 增加半程检测**（工作树快照里 `rebase_head.is_some()` 即半程；`worktree_snapshot()` 已返回该字段，零新 git 能力）：

```
Preflight 检出 rebase_head 在场
   → Terminal::failed，文案给两条出路：
       ①  在 <worktree> 内解完模型冲突后 git rebase --continue，再重跑归档
       ②  放弃本次重放：git rebase --abort，再重跑归档（会重新 rebase 一次）
   → 不清理 worktree、不删分支、不动模型
```

这样三种重入路径都收敛：

| 用户处置 | 重跑时 Preflight | 结果 |
|---|---|---|
| 已 `rebase --continue` 完成重放 | `rebase_head` 不在 → 放行 | Commit（干净跳过）→ Rebase（up-to-date no-op）→ Merge 快进 |
| 已 `rebase --abort` 回到干净态 | `rebase_head` 不在 → 放行 | Commit → Rebase（正常重放，可能再冲突）→ … |
| 什么都没做直接重跑 | `rebase_head` 在场 → fail + 引导 | 不再产生「rebase 中间态提交」这类难排查的污染 |

一个需要接受的代价：半程 change 会持续占用 active change 名额（上限 5），且 worktree 因 `rebase --force` 不可在此时被 Cleanup 删掉。前端在归档失败面应显式显示 worktree 绝对路径 + 两条处置命令，不要让用户去翻日志找路径。

### 3.5 前端

复用既有的画布与渲染资产，不新建可视化体系：

```
/architecture 路由（app-sidebar 增导航项）
├─ 左：元素树（来自 extend 结构）              ← 复用 standard 组件
│       节点前置状态点：🔴 结构错 / 🟡 声明未兑现 / ⚪ 正常
├─ 中：xyflow 画布（节点 = element，边 = relation）
│       默认只渲染聚焦元素 1-hop，避免过载
│       边样式：实线=已兑现 灰虚线=声明未兑现 橙虚线=跨 kind 向上
└─ 右：抽屉
    ├─ 元素概览（kind / metadata / path 映射 / gate 档）
    ├─ 相邻报告（最近一次 archi check 的 errors / warnings）
    └─ ADR 列表（markdown-doc-renderer 渲染 decisions/*.md）
```

`gate` 档在抽屉里可直接改（写回 `metadata.gate`），这是 ratchet 的操作入口——「这个模块我看住了」与「这个模块先放过」在同一个面板里完成。

数据面沿用 change 详情页那套：specta `Type` 导出 → `src/types/generated/bindings.ts` → `pnpm run bindings:check` 卡住漂移。命令面新增 `commands/archi/` 轨道（`archi_model` / `archi_check` / `archi_write_model` / `archi_decision_list` 之类薄命令，三件事纪律：参数转换 → 调用 → 错误映射）。

### 3.6 agent 工具面：按场景注入，给定位与检索不给内容

沿用 `create_explore_note` 的先例（专用工具不进 `TOOL_NAMES` 七工具封闭集，经 `SessionInjections` 受控追加），新增 `SessionToolset::Archi` + 两个只读工具。

#### 3.6.1 核心取舍：返回引用，不返回正文

agent 已有 `read` / `grep` / `glob` 三件**通用检索工具**。架构工具再吐正文是重复造轮子，且会把大量 token 烧在「用户其实没问的文档」上。所以架构工具的职责边界是：**把「元素 ↔ 关系 ↔ 文档」的索引告诉 agent，正文由 agent 自己 `read`。**

```
archi_module  →  「App.Auth 是什么，它依赖谁、谁依赖它，相关文档在哪」
read          →  「把那份文档读出来」
```

代价是 agent 多一次工具调用；收益是架构 crate 不需要文档正文的解析 / 索引 / 版本管理，文档形态可以自由演进。

#### 3.6.2 文档关联：三种来源，不一刀切

不预设「模块必须有一篇文档」。只做**关联发现**，任何未来出现的文档形态都能被吸收：

| 来源 | 机制 | 强度 |
|---|---|---|
| 显式 | `metadata { docs ['./docs/auth.md'] }`（可选，与 `path` 同构的数组） | 强——作者亲口指定 |
| 邻近约定 | 元素 `metadata.path` 目录下的 `*.md`（`README.md` / `ARCH.md` / 任意 md） | 中——按位置推断 |
| ADR 文本 | `decisions/*.md` 正文出现该元素 FQN | 弱——检索命中，非承诺 |

三者结果合并去重，按强度排序返回。**契约只有 `{ path, title, kind, source }`**——没有要闻摘要、没有正文、没有 schema。将来想加「接口说明文档」这类新形态，只需加一条索引来源，工具契约不变。

这条设计的实质是把「模块文档该长什么样」这个问题**延期**而不是**回避**：只要 agent 能定位到文档，文档写成什么样是作者自由。若将来发现某类文档需要结构化（如接口契约），再单独裁决，不动本层。

#### 3.6.3 两个工具

```jsonc
// 工具 1：archi_module —— 按元素定位（概述 + 关系 + 文档引用）
{
  "name": "archi_module",
  "parameters": {
    "element": "可选。元素 FQN；省略时返回全部元素的薄概览"
  }
}

// 响应（指定 element）
{
  "element": { "fqn": "App.Auth", "kind": "domain", "gate": "strict",
               "path": ["./src/auth/"], "summary": "认证与令牌签发",
               "metadata": { "owner": "platform" } },
  "dependsOn":  [{ "target": "App.Shared", "description": "复用错误类型" }],
  "dependedBy": [{ "source": "App.Order",  "description": "登录态校验" }],
  "children":   ["App.Auth.Token", "App.Auth.Session"],
  "docs": [
    { "path": "openspec/architecture/decisions/0007-token-sink.md",
      "title": "令牌下沉到 Shared", "kind": "adr", "source": "text" },
    { "path": "src/auth/README.md", "title": "认证模块说明", "kind": "md", "source": "adjacent" }
  ]
}

// 工具 2：archi_search —— 按关键词检索（元素 + 文档混合打分）
{
  "name": "archi_search",
  "parameters": {
    "query": "必填。关键词 / 元素名片段 / 路径片段",
    "scope": "可选。'elements' | 'docs' | 'all'（缺省 all）"
  }
}

// 响应
{
  "elements": [{ "fqn": "App.Auth", "kind": "domain", "summary": "认证与令牌签发" }],
  "docs":     [{ "path": "openspec/specs/desktop-agent-execution/spec.md",
                 "title": "desktop-agent-execution", "matched": "auth" }]
}
```

为什么是两个而不是一个：`archi_module` 是**定位**（我已经知道我在看哪个模块），`archi_search` 是**发现**（我还不知道这个功能归谁管）。两者的注入场景不同——dev-design 阶段最需要 search（「这功能该放哪」），implement 阶段最需要 module（「我改的文件属于谁」）。合并成一个工具会让参数语义含混（`element` 与 `query` 互斥），且两个场景拿到的工具面无法差异化收窄。

#### 3.6.4 注入矩阵

| 场景 / 相位角色 | `archi_module` | `archi_search` | 理由 |
|---|---|---|---|
| proposal planner | ❌ | ✅ | 提案要先判断「这事落在哪个能力域」，正是发现场景 |
| dev-design planner / evaluator | ✅ | ✅ | 写 design.md 要定落点 + 列依赖 |
| test-design planner | ❌ | ✅ | 找被测模块的现有测试位置 |
| implement executor | ✅ | ❌ | 改文件时确认归属与可引用面（不需要发现新模块） |
| code-review evaluator | ✅ | ❌ | 核对改动是否越界 |
| acceptance evaluator | ✅ | ❌ | 验收确认架构未劣化 |
| test-gen / test-execution / archive SpecSync | ❌ | ❌ | 与架构无关，注入只是噪声 + 加 token |

只读约束：两个工具的写面均为零（改模型走画布或直接编辑 `.c4`，两者都过 §3.4 [L1] 校验）。保持「agent 无自主改模型权」这条既有硬规则（原 `architecture` agent prompt 的 "No autonomous modifications" 同源纪律）。



---

## 4. 运作管线

```
[P1] 加载（Load）      读 models/*.c4 → 解析元素 / 关系 / path / gate 档 → 内存图
        │             每次显式命令全量重算，零持久化、零守护进程
        ▼
[P2] 结构校验（Lint）  petgraph 上图算法，纯模型内部、零代码输入：
        │             • 成环（SCC）
        │             • A -> anc(A)（元素依赖自己的祖先——图结构自相矛盾）
        │             • 元素重复 / 缺 specification / 引用了不存在的元素
        │             → 发生在「写模型」而非「跑相位」：拒绝写入并返回诊断
        ▼
[P3] 交叉核对（X-ref） change_edit_set(base) 取变更清单（排除测试文件）
        │             ∪ oxc_resolver / cargo metadata → 文件归属元素
        ▼             → 与声明的 relation 对账：未声明依赖 / 目标不在模型
[P4] 判定（Verdict）   按引入方 gate 档定级：strict → error / warn → warning
        │             error 非空 → 门禁 fail → FeedbackLoop 回 executor 改代码
        │             超限 → 升格交决策（改代码 or 回 dev-design 改模型）
        ▼
[P5] 观察（Observe）   漂移① 声明未兑现 / 跨 kind 向上的边 → 架构视图，不进门禁
```

### 4.1 三个变更场景的桌面实现

**场景 A：改模块划分（新增 / 删除 / 合并元素）**

1. 用户在画布或直接编辑 `.c4` 文件改变 `extend` 结构。
2. 下一次 `archi_model` 命令重算内存图，与 change 的 `base_commit` 基线比对得出结构差分（新增 / 删除 / 移动的节点与边）。
3. 画布以视觉差分呈现：新增绿、删除红虚线、变更黄。
4. **不做前向脚手架生成**（自动建 crate / 目录骨架）。理由：面板生成骨架会与 `write_protection`、change 的 worktree 边界、以及「实现由 implement 相位 agent 负责」三方冲突。

**场景 B：改依赖关系（改变调用方向）**

1. 新边声明时即刻跑结构校验；成环或 `A -> anc(A)` → **拒绝写入并返回诊断**（纯函数，毫秒级，发生在声明阶段而非门禁相位）。画布连边与 agent 改模型走同一条校验路径。
2. 结构合规后再做代码侧核对：`change_edit_set` 拉出本 change 真实改动的文件，看 import 是否已体现新边。未体现 → 架构视图标 🟡（**不进门禁**：这是「还没写」，不是「写错了」）。
3. 反向核对（进门禁）：代码里出现了模型未声明的跨元素 import → 按引入方 gate 档定级。

**场景 C：改对外接口（新增符号、变更签名）**

1. 契约变化在 specs / ADR 里表达（不在 DSL 里造第二套 schema）。
2. 需要符号级核对时才提取源码符号清单（见 §6 分阶段：TS import 级 → Rust crate 级 → 签名级）。
3. 差异作为黄金 context 交给相位 agent 做柔性重构；**不自动重写测试文件**。测试文件的生成权归 test-gen 相位（该相位独占测试文件生成，且受 `write_protection` 与变更清单 scope 约束）。本方案只产出「差异 + 建议用例点」交给 test-gen 消费。


---

## 5. 硬约束清单（动手前必须记住）

| 约束 | 出处 | 违反后果 |
|---|---|---|
| `RunStepKind` / `StepKind` 新变体尾部追加 | `workflow/state.rs` bincode 下标落库 | 存量 run 节点行 / 审计行步词整体错位 |
| `workflow` 与 `archi` 互不依赖 | `desktop-crate-layout` spec | crate 图破，纯读域引入编排行为 |
| core 禁 Tauri、core 禁 store | 同上 | 解析域被持久化 / 壳层污染 |
| 七工具封闭集、下发面单源 | `tools.rs` `TOOL_NAMES` / `policy.rs` / `compose.rs` / 16 份 prompts | 越权工具幻觉可执行；prompt 逐字断言测试红 |
| 显式取数、无 watch 无轮询；watch 仅单文件信号不含内容 | `desktop-file-watch` spec | 画布想自动刷新必须先改 spec，不能偷加目录监听 |
| 新解析器要带语料金标准 + 过变异门槛 | `checks` 的 `tests/golden` 文化 + `config.json` 的 mutation 80/70 | 变异分不达标，流程在 test-execution 相位卡死 |
| `config.json` 已知键七项，其余进 `extra` | `core/config/src/lib.rs` `KNOWN_KEYS` | 新增配置不登记 → 静默 passthrough，config 页不展示不校验 |
| 每 workspace 独立 db、模型演进走 `native_model` 版本链 | `infra/store` | 若架构结论要落库，必须走版本段而非改字段 |
| 架构扫描的排除规则与 `tests[].excludes` 正交 | `config` crate 的 `TestSuite` 语义 | 复用会让「调测试 scope」意外改变「架构扫描范围」 |
| 豁免只到元素对粒度，禁 glob | 本方案 §2.3 | glob 豁免 = 永久后门，ratchet 失效 |
| `oxc_parser` + `oxc_allocator` + `oxc_resolver` 是唯一真新增依赖 | `Cargo.lock` 现状 | 编译时间 / 产物体积需在 S1 实测记录 |
| 架构域配置只落 `openspec/architecture/config.json`，MUST NOT 出现第三个配置文件 | 本方案 §2.4 | 配置来源分散，用户不知道改哪 | 
| `archi` 目录字面量只经 `foundation::layout::archi_dir` 派生 | 本方案 §3.2 | 零散 `"architecture"` 字面量散进各 crate，改名即漂移 |
| `archi_module` 只读，永不写入模型 | 本方案 §3.6 + 原 `architecture` agent 的 "No autonomous modifications" 纪律 | agent 自主改架构 = 设计权旁落 |
| 模型文件冲突 MUST NOT 进 agent 解算 | 本方案 §3.4.2 | LLM 替人做架构裁决，且冲突解错无人能察 |
| change 期一切架构读写只碰 worktree 那棵模型 | 本方案 §3.4.2 | 读到主仓基线模型，门禁永远滞后一个 change |
| 归档 Preflight MUST 检出 rebase 半程并挡住 | 本方案 §3.4.2 | 重跑会在 rebase 中间态 commit，污染历史 |

---

## 6. 技术选型：全部走开源库，只有归属判定自己写

### 6.1 依赖清单（锁文件实际状态）

| 需要什么 | 库 | 锁内状态 |
|---|---|---|
| 文件遍历 + gitignore | `ignore` 0.4.33（`WalkBuilder`，自带标准过滤） | 传递（经 tokei） |
| glob 匹配 | `globset` 0.4.20 / `glob` 0.3.4 | 传递 / 已是直接依赖（`checks` 的 `globmatch`） |
| 图算法 | `petgraph` 0.8.3（SCC / 拓扑排序） | 传递 |
| Rust crate 依赖 | `cargo_metadata` 0.19.2 | 传递（经 tauri-utils） |
| Rust 文件级 `use` | `syn` 2.x（`full` + `parse_file`） | 传递（各 derive 宏） |
| TS/JS 语法树 | `oxc_parser` + `oxc_allocator` | **真新增（已定案引入）** |
| TS/JS 模块解析 | `oxc_resolver`（tsconfig paths / node 解析 / exports） | **真新增（已定案引入）** |
| `.c4` 解析 | 手写子集解析器 | 新增（无现成库） |

`syn` / `cargo_metadata` 在锁内是以 proc-macro / build-script 身份出现的（host 编译），提为普通依赖会给 target 编译增加时间，量级需实测。`oxc_*` 是唯一实打实的体积新增——已定案引入，实测只用于记录成本、不改变结论。


**明确不用**（会破坏「不依赖本地环境、开箱即用」——`retire-cli-engine` 已把这条定为产品前提）：

```
madge / dependency-cruiser / ts-morph   → 要 node
cargo-modules / cargo-depgraph          → 要 cargo install 二进制
  （cargo-modules 的模块级依赖图与环检测最贴需求，可作"检测到则增强"的可选路径，
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
├─ 「这个文件属于哪个 C4 元素」      ← metadata.path 语义是我们发明的
├─ 「这条 import 是否跨元素边界」    ← 需要 path 前缀归属 + 元素内豁免
└─ 「这个元素现在开没开门禁」        ← gate 档 ratchet 策略
```

`oxc_resolver` 的使用口径：按 `config.json` 的 `tests[].root` 逐子树加载 tsconfig（`baseUrl` + `paths`），对每个 import specifier 调用一次 `resolve`。解析失败不是错误——记 warning「路径不可解析：<specifier>」，不进门禁（看不见的依赖不该冒充违例）。

### 6.3 其他选型

| 领域 | 选型 | 状态 |
|---|---|---|
| 变更清单 | `ArchiveVcsPort::change_edit_set(root, base)`（`git diff <base> --name-only -z` 并上 `ls-files --others`） | 已有，唯一入口 |
| 代码量 / 语言分布 | `tokei`（`code_stats` 既有） | 已有，可复用于元素体积概览 |
| 存储 | 默认零持久化；确需落库才用 `store` 的 `native_model` 版本链 | 已有机制 |
| 前端画布 | `@xyflow/react` + change flow 既有布局 / 抽屉 / 运行态资产 | 已有 |
| 类型桥 | specta `Type` → `bindings:check` 守门 | 已有 |

### 6.4 产物标准（S1 验收阈值）

**基线实测**（HEAD `f5539bbe`，本机 Windows release 构建）：

| 指标 | 基线值 | 出处 |
|---|---|---|
| 安装包（NSIS） | **6.68 MB**（`dev-team_0.5.26_x64-setup.exe`） | `src-tauri/target/release/bundle/nsis/` |
| 主二进制 | **26.6 MB**（`dev-team.exe`） | 同目录 |
| `Cargo.lock` | 999 个包 / 11 548 行 | 文件统计 |
| CI 构建 | 双平台 matrix，各腿 `tauri build` 一次 | `.github/workflows/desktop-release.yml` |
| 单次 archi 检查的对外预算 | 与 `STATIC_CHECK_TIMEOUT = 1200s` 同一相位预算池 | `infra/checks/src/static_check.rs` |

**阈值**（硬线 = 必须回退设计；软线 = 记录并在 spec 留痕）：

| 指标 | 软线 | 硬线 | 超标动作 |
|---|---|---|---|
| NSIS 安装包 | +0.8 MB（≈ +12%） | **+1.5 MB（≈ +22%，即 ≤ 8.2 MB）** | 见下回退序 |
| 主二进制 | +3 MB | **+5 MB（≤ 31.6 MB）** | 同上 |
| `Cargo.lock` 净新增包数 | ≤ 25 | **≤ 40** | 说明引入面失控，重审特性开关 |
| CI 冷构建（单腿） | +45 s | **+90 s** | 回退序第 1 步 |
| 本地增量（改 `archi` 一文件后 `cargo build`） | +8 s | **+15 s** | 检查是否误引了重特性 |
| 单次 `archi_check` 运行 | 1.5 s | **3 s**（含 200 变更文件 + 5 000 元素量级） | 加并发 / 加结果缓存（进程内） |

**为什么是这些数**：

- 安装包 +1.5 MB 对应「一个小版本自然增长的量级」——0.5.4→0.5.26 期间安装包只涨了 0.28 MB，一次引入吃掉 5 倍于常态的增长就过量了。
- CI +90 s 的现实压力来自发版链：双平台各腿跑一次完整 `tauri build`，且 `desktop-release.yml` 用 `Swatinem/rust-cache` 缓存；冷构建每多 90 s 直接体现在「点发版到出包」的反馈时长上。
- 本地增量 +15 s 直接决定 `run_static_analysis`（`pnpm run -C ./packages/desktop check`）的体感——它已经串了 `cargo fmt + clippy + vp check + knip`，每相位跑多次。
- `archi_check` 3 s 是「门禁步不能比它检查的东西还慢」的下限直觉：`static_check` 动辄几十秒，archi 若也几十秒，就没人愿意在 implement 里多等一次。

**测量协议**（S1 必须照做并记录进报告）：

```
1. 基线：git stash 掉依赖改动，cargo build --release --timings 记录
         （记总时长 + 各 crate 编译用时），安装包大小取 bundle/nsis 产物
2. 变体：加依赖后重复同一命令，同机同 profile、同 cache 状态
3. 差值：只比「目标 crate 及其新增依赖子树」的编译用时，不比全量（cache 噪声大）
4. 运行期：构造 200 变更文件 + 5000 元素语料，跑 10 次取 p95
5. 三项（包 / 构建 / 运行）全部落进 crates/core/archi/tests/README.md 的实测表
```

**超标回退序**（按代价从低到高，每步都验证是否已达硬线）：

```
① 减特性：oxc_parser 只开 parse，不引 oxc_semantic / oxc_codegen / oxc_minifier
          （我们要的是 import 声明，不需要语义分析）
② 砍 oxc_resolver（≈ 自算路径 150 行：相对路径拼接 + workspace 包名映射 + tsconfig paths）
   代价：exports / imports 条件解析退化，语料里相应用例标为「已知不支持」
③ 砍 oxc_parser 退正则 import
   代价：type-only 判定与 span 行号（豁免注释定位用）退化，② 的误报率上升
   这一步已实质违反「type-only 算架构依赖」的裁决，需要重新裁决
```

`Cargo.lock` 净新增包数的软/硬线单独设，是因为它比体积更早暴露问题：oxc 家族按特性会拉进 `compact_str` / `rustc-hash` / `simdutf8` / `bitflags` 等一串小包，若净增超过 40 个，说明某个开关默认把不需要的东西带进来了。

---

## 7. 语料与验收（Spike）

任何解析器都必须先有语料、再有实现。语料覆盖历史 `.c4` 的真实写法：注释、多文件合并（`specification` 只取一次）、`metadata` 单值与数组两种形态、`extend` 嵌套、关系描述引号、空模型与非法模型。

```
crates/core/archi/tests/
├── corpus/
│   ├── dsl/<case>/*.c4                 输入模型
│   ├── src/<case>/**                   输入代码（import / use 语料）
│   └── expected/<case>.json            期望（elements/relations/gate/诊断/报告）
└── golden_test.rs                      对拍（与 crates/core/checks/tests/golden 同风格）
```

### 7.1 DSL 语料（解析 + 结构校验）

注释、多文件合并（`specification` 只取一次）、`metadata` 单值与数组两种形态、`metadata` 带 `gate` 键、`extend` 嵌套、关系描述引号、空模型、缺 specification、非法 kind。

### 7.2 结构校验语料（声明阶段即拒）

| 用例 | 期望 |
|---|---|
| `A -> B` 且 `B -> A` | error：成环（SCC 输出环路径） |
| `A -> anc(A)`（A 被 anc 用 `extend` 包含） | error：依赖自己的祖先 |
| 关系引用不存在的元素 | error：未定义元素 |
| 同 kind 同名重复声明 | error：元素重复 |
| 无环有向图 + 合法祖先关系 | clean |

### 7.3 交叉核对语料（门禁）

| 用例 | gate 档 | 期望 |
|---|---|---|
| import 与声明一致 | strict | clean |
| 未声明依赖（A 引 B，模型无 A->B） | source strict | error |
| 同上 | source warn | warning |
| 同上 | source off | 不出现（文件算 unmatched） |
| 同一元素对在 3 个文件各 1 处 | strict | 1 条 error + `occurrences` 3 项（聚合不铺开） |
| 目标元素不存在于模型 | strict | warning（目标未建模，归因不明） |
| 测试文件跨元素 import | — | 不扫描（排除规则生效，`scan.excluded` 留痕） |
| `import type` 跨元素 | strict | error（type-only 算架构依赖，`typeOnly: true`） |
| `// archi-ignore-next-line` 标注 | strict | 不报 + `suppressed` 有 inline 条目（含 file/line/reason） |
| `architecture.ignore` 元素对 | strict | 不报 + `suppressed` 有 config 条目（含 reason） |
| `oxc_resolver` 解析失败 | strict | warning（不冒充违例，`scan.unresolved` 留痕） |
| 声明了但代码未兑现 | — | 不进报告的门禁段，只在视图标 🟡 |
| 存量模型含 `A -> anc(A)` | — | 照常加载（`model.structuralErrors` 有记录）+ 门禁判 error |
| 模型为空 | — | status = skipped |
| 无变更文件 | — | status = noChanges |

| Spike | 内容 | 验证什么 |
|---|---|---|
| **S1 解析 + 结构校验** | `core/archi` 跑通 §7.1 / §7.2 语料；`petgraph` 做 SCC 与祖先链判定；写模型命令拒绝非法 DSL；**同时按 §6.4 测量协议出产物实测表** | 解析语义 + 图算法 + 依赖成本量化；零 UI、零门禁 |
| **S2 交叉核对 + 报告** | `change_edit_set` → `oxc_resolver` 归属 → 对账 → `reports/archi/*.json`；前端一格展示 | 归属判定、gate 档定级、豁免机制、报告槽 |
| **S3 门禁挂载** | `StepNode::ArchiCheck` 挂 implement → 真实 change 跑出 fail → FeedbackLoop 回 executor → 超限升格；旧 db 验证存量 run 仍可读 | 相位表接线 + bincode 尾部追加纪律 + 端到端手感 |

---

## 8. 已定案

| # | 问题 | 结论 |
|---|---|---|
| 1 | 漂移分类 | 只剩 ②（未声明依赖）与 ④（path 失效）进门禁；① 归架构视图；③ 并入 ②（声明即合法，C4 kind 是包含层级不是约束层级） |
| 2 | 级别 | 二值 `errors` / `warnings`；定级看引入方 gate 档 |
| 3 | 测试文件 | 全排除，独立规则，不复用 `tests[].excludes` |
| 4 | 豁免 | 声明即豁免 + 内联 `archi-ignore-next-line` + `architecture.ignore` 元素对（禁 glob 粒度） |
| 5 | type-only import | 算架构依赖（语法层判定即可，不需语义层） |
| 6 | 存量债务 | `metadata.gate` 三档 ratchet，按空间逐元素收紧 |
| 7 | 可自动修复标记 | 不做；架构问题整体转 change |
| 8 | ② 的失败处理 | `FeedbackLoop { limit: 3 }`——②可能是代码不合规也可能是模型滞后，先让代码符合模型，超限自升格交决策（`walker` 既有语义，无需新 `OnFail` 变体） |
| 9 | 增量收紧 | 不做 `touched` 升档；① 的问题在架构视图解决 |
| 10 | 扫描工具 | 全走开源库：`ignore` / `globset` / `cargo_metadata` / `syn` / `petgraph` 提为直接依赖；`oxc_parser` + `oxc_allocator` + `oxc_resolver` 为新增。只有「文件归属哪个元素」自己写 |
| 11 | `A -> anc(A)` | 留在 DSL 校验（Lint），在**声明阶段**拒绝写入，不进门禁 |
| 12 | 门禁挂载点 | 只挂 implement（`FeedbackLoop{3}`）；acceptance 不挂，架构问题转新 change 验收 |
| 13 | `gate` 档权威位置 | `metadata.gate`（模型自描述，零新机制）；不另立元素→档位映射表 |
| 14 | 读存量模型遇 `A -> anc(A)` | **照常加载**，视图标红 + 门禁判 error；不拒绝加载（不因历史数据让页面打不开） |
| 15 | `architecture.ignore` 配置位置 | `openspec/architecture/config.json`（架构域自有配置文件），不进 `config.json` 的七键约束 |
| 16 | archi 读不读 `openspec/specs/` | **不读**。架构看护只管模型 ↔ 代码；specs 与代码的关系归既有 SpecSync 与各相位 agent |
| 17 | 模型放哪个根 | **主仓根随 git 提交**；worktree 自 base 拉出天然带副本；**change 期读 worktree 版本**，声明校验与门禁都读它 |
| 18 | 模型冲突 | worktree 合回主仓时若模型文件冲突 → **中断交用户处理**，不进 agent 解算（与代码冲突分流） |
| 19 | agent 查图 | 做，但**按场景注入**（注入矩阵见 §3.6.4）；不加进 `TOOL_NAMES` 七工具封闭集 |
| 20 | 架构域落位 | `openspec/architecture/` 一个目录装齐：`models/*.c4` + `decisions/*.md` + `config.json` + `reports/` |
| 21 | 产物标准 | 安装包 +1.5 MB / 二进制 +5 MB / 锁内净增 ≤ 40 包 / CI 冷构建 +90 s / 本地增量 +15 s / 单次检查 3 s（硬线，见 §6.4）；超线按「减特性 → 砍 resolver → 砍 parser」回退 |
| 22 | 模块文档 | **不预设文档形态**，只提供「元素 ↔ 文档」关联发现与检索；工具返回**引用**（`{path,title,kind,source}`）不返回正文，正文交 agent 用 `read` 取。三种关联来源：显式 `metadata.docs` / 路径邻近约定 / ADR 文本命中 |
| 23 | 冲突现场处置 | **保留 rebase 半程**不 abort（沿用 `rebase_branch` 既有「冲突态保留」语义）；配套：`Preflight` MUST 检出 `rebase_head` 在场并挡回，防重跑在中间态 commit |

## 9. 未决问题

1. **`archi_search` 的索引范围**——检索只覆盖 `openspec/architecture/` 与元素 `metadata.path` 邻近的 md，还是也覆盖 `openspec/specs/` 与 `openspec/changes/`？（已定案 16 是「不读 specs 做**看护**」，但**检索**是另一回事：dev-design 找「这功能归谁管」时，specs 里往往才是答案。需要区分「看护不读」与「检索可读」。）
2. **`archi_search` 的实现方式**——真实全文索引（要建倒排 / 用 `ignore` 遍历 + 正则），还是让 agent 直接用既有 `grep` 工具？若后者够用，`archi_search` 可以只做「元素名/描述的结构化搜索」，把文档全文搜索让给 `grep`。倾向前者轻量版（只在标识符层面匹配，不做全文）。
3. **产物实测的执行者**——S1 的测量协议需要同机同 cache 状态重复构建，CI 环境噪声大。是在开发机记录，还是加一条 workflow 专门跑？




