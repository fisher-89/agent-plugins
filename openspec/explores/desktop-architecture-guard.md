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

`foundation::layout` 目前只有 changes / archive / explores 三棵树（`Layout` struct 三字段 + `config_path` + 报告槽派生）。新增三个**派生函数**，不动 `Layout` struct（零构造点波及）：

```
archi_models_dir(root)     → openspec/architecture/models
archi_decisions_dir(root)  → openspec/architecture/decisions
archi_reports_dir(root)    → openspec/architecture/reports      （全局兜底位置）
```

按 foundation 的「第二个消费者到场才下沉」纪律，这三项落地时需在 `desktop-crate-layout` 的 spec 里登记为新增消费者。

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
| `oxc_parser` + `oxc_allocator` + `oxc_resolver` 是唯一真新增依赖 | `Cargo.lock` 现状 | 编译时间 / 产物体积需在 S1 实测，超标则退回自算路径 |

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
| TS/JS 语法树 | `oxc_parser` + `oxc_allocator` | **真新增** |
| TS/JS 模块解析 | `oxc_resolver`（tsconfig paths / node 解析 / exports） | **真新增** |
| `.c4` 解析 | 手写子集解析器 | 新增（无现成库） |

`syn` / `cargo_metadata` 在锁内是以 proc-macro / build-script 身份出现的（host 编译），提为普通依赖会给 target 编译增加时间，量级需实测。`oxc_*` 是唯一实打实的体积新增。

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
| 目标元素不存在于模型 | strict | warning（目标未建模，归因不明） |
| 测试文件跨元素 import | — | 不扫描（排除规则生效） |
| `import type` 跨元素 | strict | error（type-only 算架构依赖） |
| `// archi-ignore-next-line` 标注 | strict | 不报 + 进豁免清单 |
| `architecture.ignore` 元素对 | strict | 不报 + 进豁免清单 |
| `oxc_resolver` 解析失败 | strict | warning（不冒充违例） |
| 声明了但代码未兑现 | — | 不进门禁，只在视图标 🟡 |
| 模型为空 | — | status = skipped |
| 无变更文件 | — | status = noChanges |

| Spike | 内容 | 验证什么 |
|---|---|---|
| **S1 解析 + 结构校验** | `core/archi` 跑通 §7.1 / §7.2 语料；`petgraph` 做 SCC 与祖先链判定；写模型命令拒绝非法 DSL | 解析语义 + 图算法；零 UI、零门禁 |
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

## 9. 未决问题

1. **archi 读不读 `openspec/specs/`**——桌面目前对 specs 只有归档 SpecSync 的 agent 会话（按 prompt 读写），无 Rust 读面。若「架构看护」要覆盖 specs 与代码之间的漂移，需给 foundation 加 specs 目录常量并加读取层。这是范围问题不是技术问题。
2. **模型放在哪个根**——主仓根 vs change 的 worktree 根。change 在 worktree 内改模型时，声明阶段校验与门禁读哪一份？`foundation::layout::resolve` 已被双根注入（create 的双根先例），需明确口径并在 spec 写死。
3. **`A -> anc(A)` 的校验时机**——已定「声明阶段拒绝写入」，但**读存量模型时遇到**（历史模型里已有这种边）是拒绝加载还是 warning 并在视图标红？我倾向后者（不因历史数据阻断加载），但要在 spec 写死。
4. **`gate` 键的权威位置**——现在设计在 `metadata.gate`（零 DSL 变更）。备选是 `openspec/architecture/config.json` 集中维护元素→档位映射（好处：改档位不碰模型文件；坏处：两处真相源）。目前倾向 `metadata`。
5. **agent 是否需要查图能力**——若需要，走 `SessionToolset::Archi` 受控追加（`create_explore_note` 已证明「专用工具不进七工具封闭集」可行），而不是把 `TOOL_NAMES` 从 7 改成 8。本期不做。
6. **编译成本实测**——`oxc_parser` + `oxc_allocator` + `oxc_resolver` 是本方案唯一的真新增依赖，需在 S1 阶段实测 `cargo build` 增量时间与产物体积，超标则退回「解析用 oxc_parser、路径自己算 150 行」的方案。

