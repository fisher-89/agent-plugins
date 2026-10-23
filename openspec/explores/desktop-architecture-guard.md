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
* **刚柔分层**：环路 / 分层 / 声明外依赖这类可判定问题用静态算法做刚性拦截（零 LLM、确定性、可复现）；「这样拆对不对」「怎么重构」交相位 agent。
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

### 3.4 门禁挂载点（phase_table）

```
requirement 表（10 相位）
initialize     PhaseStart → Clean → StaticCheck(Terminal) → TestExecution(Terminal) → PhaseLog
proposal       PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog
dev-design     …
test-design    …
implement      PhaseStart → Executor → StaticCheck(FeedbackLoop)
               → Evaluator → VerdictGate(FeedbackLoop) → PhaseLog
               ★ 在此插入 ArchiCheck(FeedbackLoop)
test-gen       …
test-execution …
code-review    …（无 executor，仅 evaluator）
acceptance     ★ 收口前复核（ArchiCheck），再 evaluator → VerdictGate
archive        Preflight → Commit → Rebase → ResolveConflicts → Merge
               → SpecSync → Cleanup → Seal → Finalize

test-only 表（7 相位）：initialize → proposal → code-analyze → test-design
                        → test-gen → test-execution → archive
```

挂点选择：**implement 相位**（代码刚写完，漂移信息对 executor 最有价值，`FeedbackLoop` 能直接触达修正）+ **acceptance 相位**（收口前的确定性复核）。initialize 基线是否跑 archi check 取决于「基线本身是否要求模型齐全」，见 §8。

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
├─ 中：xyflow 画布（节点 = element，边 = relation）
│       默认只渲染聚焦元素 1-hop，避免过载
└─ 右：抽屉
    ├─ 元素概览（kind / metadata / path 映射）
    ├─ 相邻报告（最近一次 archi check 的结论与 violations）
    └─ ADR 列表（markdown-doc-renderer 渲染 decisions/*.md）
```

数据面沿用 change 详情页那套：specta `Type` 导出 → `src/types/generated/bindings.ts` → `pnpm run bindings:check` 卡住漂移。命令面新增 `commands/archi/` 轨道（`archi_model` / `archi_check_report` / `archi_decision_list` 之类薄命令，三件事纪律：参数转换 → 调用 → 错误映射）。

---

## 4. 运作管线

```
① 加载（Load）        读 models/*.c4 → 解析元素 / 关系 / path 映射 → 内存图
        │             每次显式命令全量重算，零持久化、零守护进程
        ▼
② 校验（Lint）        petgraph 上图算法：环路（SCC）/ 层级合法性 / 浮空元素
        │             （声明了元素但 metadata.path 全不存在）
        ▼
③ 核对（Cross-ref）   change_edit_set(base) 取变更清单
        │             ∪ path→element 映射 → 每文件的 import 目标归属哪个元素
        ▼             → 与声明的 relation 对账：缺声明 / 方向错 / 未使用声明 / 目标不在模型
④ 判定（Verdict）     violations 非空 → 门禁 fail → FeedbackLoop 回 executor
        │             仅 warnings → pass 但报告留痕
        ▼
⑤ 使能（Act）         相位 agent 拿到「契约意图 vs 代码事实」的差异报告做修正
```

### 4.1 三个变更场景的桌面实现

**场景 A：改模块划分（新增 / 删除 / 合并元素）**

1. 用户在画布或直接编辑 `.c4` 文件改变 `extend` 结构。
2. 下一次 `archi_model` 命令重算内存图，与 change 的 `base_commit` 基线比对得出结构差分（新增 / 删除 / 移动的节点与边）。
3. 画布以视觉差分呈现：新增绿、删除红虚线、变更黄。
4. **不做前向脚手架生成**（自动建 crate / 目录骨架）。理由：面板生成骨架会与 `write_protection`、change 的 worktree 边界、以及「实现由 implement 相位 agent 负责」三方冲突。

**场景 B：改依赖关系（改变调用方向）**

1. 新边插入内存图即刻跑 SCC；成环或逆向分层 → 立刻熔断报错（纯函数，毫秒级，不依赖门禁相位）。
2. 拓扑合规后再做代码侧核对：`change_edit_set` 拉出本 change 真实改动的文件，看 import 是否已体现新边。未体现 → warning「代码落后于设计」，在门禁相位拦截并给 executor 具体文件清单。

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

---

## 6. 技术选型（按桌面现状分阶段）

| 领域 | 选型 | 状态 |
|---|---|---|
| 内存图与图算法 | `petgraph`（SCC / 拓扑排序 / 邻接遍历） | 已在 `Cargo.lock`（传递依赖），提为直接依赖零新增面 |
| `.c4` 解析 | **手写子集解析器**（element 关键字、`extend`、`->`、`metadata { path [...] }`、注释、多文件合并） | 新增，语料见 §7 |
| 变更清单 | 复用 `ArchiveVcsPort::change_edit_set(root, base)`（`git diff <base> --name-only -z` 并上 `ls-files --others`） | 已有，唯一入口 |
| TS/JS 依赖提取（一期） | 正则 import 解析（`import … from '…'` / `require` / 动态 import），路径归一后与 `metadata.path` 前缀匹配 | 新增，最便宜 |
| Rust 依赖提取（一期） | `cargo metadata --format-version 1` 经 `exec` 取 crate 级依赖；文件级归属靠 `metadata.path` | 新增（若需进程，实现落 infra） |
| 符号级提取（二期可选） | `oxc_parser` / `syn` | **不预铺**：桌面无先例，语料成本高；只有「必须函数签名碰撞」时才引 |
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
│   ├── models/<case>/*.c4              输入
│   └── expected/<case>.json            期望（elements / relations / path 映射 / 诊断）
└── golden_test.rs                      对拍（与 crates/core/checks/tests/golden 同风格）
```

交叉核对的语料至少覆盖八个场景：声明与 import 一致（clean）、未声明的关系（violation）、方向错（violation）、自引用（忽略）、目标不在模型（warning）、声明未被使用（warning）、`metadata.path` 指向不存在的目录（warning）、模型为空（skipped）。

| Spike | 内容 | 验证什么 |
|---|---|---|
| **S1 解析 + 图** | `core/archi` 跑通语料对拍：解析 models/*.c4 → elements / relations / path 映射；`petgraph` 做 SCC 与分层判定 | 解析语义 + 图算法；零 UI、零门禁 |
| **S2 核对 + 报告** | `change_edit_set` 并 path 映射 → import 交叉核对 → `reports/archi/*.json`；前端一格展示报告 | 报告契约、槽位（含「无模型 → skipped」分支） |
| **S3 门禁挂载** | `StepNode::ArchiCheck` 挂 implement 相位 → 真实 change 跑出一次 fail → FeedbackLoop 回 executor；同时用旧 db 验证存量 run 记录仍可读 | 相位表接线 + bincode 尾部追加纪律 + 端到端手感 |

---

## 8. 未决问题

1. **漂移的可计算定义**——四种候选：① 声明了关系但代码里没有 import（代码落后）；② 代码里有跨元素 import 但模型未声明（模型落后）；③ 层间逆向（如 component → package）；④ 元素声明了却没有有效 `metadata.path`（浮空模块）。四者门禁严格度不同：①② 宜 warning、③④ 宜 violation。需拍板后写进 spec。
2. **archi 读不读 `openspec/specs/`**——桌面目前对 specs 只有归档 SpecSync 的 agent 会话（按 prompt 读写），无 Rust 读面。若「架构看护」要覆盖 specs 与代码之间的漂移，需给 foundation 加 specs 目录常量并加读取层。这是范围问题不是技术问题。
3. **模型放在哪个根**——主仓根 vs change 的 worktree 根。change 在 worktree 内改模型时门禁读哪一份？`foundation::layout::resolve` 已被双根注入（create 的双根先例），需要明确口径并在 spec 写死。
4. **门禁严格度分档**——implement 用 `FeedbackLoop` 已定；acceptance 用 `Terminal` 还是 `FeedbackLoop`、initialize 基线是否跑 archi check，取决于问题 1 的定义。
5. **agent 是否需要查图能力**——若需要，走 `SessionToolset::Archi` 受控追加（`create_explore_note` 已证明「专用工具不进七工具封闭集」可行），而不是把 `TOOL_NAMES` 从 7 改成 8。本期不做。
