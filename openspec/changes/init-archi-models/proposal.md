# 提案: init-archi-models

> **变更**: init-archi-models
> **日期**: 2026-10-27
> **状态**: draft
> **来源**: `openspec/changes/init-archi-models/explore.md`（目标一句话：分析源码与 specs 生成初版架构图与模型文档；后端下钻 crate 层，前端下钻视图与组件层，hooks / lib 暂不解析。explore 无追加笔记，粒度口径由本提案裁定）
> **范围接缝**: 本变更只产出**模型数据**（`openspec/architecture/models/*.toml`）与**一份裁定记录**（`openspec/architecture/decisions/*.md`）。解析器、图算法、画布、命令面、agent 工具面**零代码改动**——它们已由归档变更 `2026-10-27-archi-graph-doc-agent` 全量落地。代码扫描、归属核对、漂移判定、门禁挂载归 `archi-check-and-gate`（另一条线），本期 MUST NOT 预铺。
> **前置依赖**: 无阻塞项。`foundation::layout::archi_dir` / `archi_models_dir` / `archi_decisions_dir` 已在位；`archi_graph` / `archi_module` / `archi_search` 命令与工具已在位；`/architecture` 页面与 `ArchitectureCanvas` 已在位；基线报告 `reports/baseline/summary.json` 全绿（3239 通过 / 覆盖率 97.27% / 0 失败）。本变更只填空目录——`openspec/architecture/` 当前**不存在**。

---

## 问题

### 现象一：能力齐备，语料为零

架构能力面（模型格式、结构诊断、图投影、两处画布挂载、两个 agent 只读工具、五条 IPC 命令）已全量落地，但 `openspec/architecture/models/` 不存在。三个可见后果：

| 消费面 | 当前实况 | 后果 |
|---|---|---|
| `/architecture` 页面 | 空模型 ⇒ 空态（零节点零边） | 页面存在但看不到任何架构 |
| `archi_module` / `archi_search` 工具 | 模型为空 ⇒ 薄概览零条目、检索零命中 | dev-design / implement 相位拿到了工具却没有知识可查 |
| 人 | 模块边界与依赖方向仍在 `AGENT.md` 散文、`desktop-crate-layout` 的 spec 段落与提交历史里 | 每次讨论「这条依赖该不该存在」都从零开始 |

冷启动是**一次性窗口**：模型一旦长成，之后的变更都是增量；粒度与口径在这一变更里定死，后面只做加法。

### 现象二：架构知识散落且已陈旧

现有权威只有两处，且都有缺口：

- `AGENT.md` 的 Architecture 段落：一段散文汇总，逐条列 crate 却不含 `core/archi`（该 crate 已落地且被 `agent-runtime` 与壳依赖），依赖方向只有一句「infra → core，never core → infra」，没有任何逐条边；前端只有一句技术栈说明，视图与共享组件的分层与复用关系零记载；
- `openspec/specs/desktop-crate-layout/spec.md` 的依赖规则：`core/archi` 仍写成「未来落地时 SHALL …」，`agent-runtime` 仍写成「SHALL 仅依赖 `agent`」——实际它依赖 8 个 workspace crate；`archi` / `checks` / `checks-runtime` / `exec` / `orchestration` / `watch` 六条边未登记（spec 自己声明「首个真实消费者出现才立该边」，属既知欠账）。

本变更以 **`Cargo.toml` 事实**为唯一取数口径（不是以 spec 散文为口径），因此模型会是仓库内第一份「与代码同源」的架构声明；spec 的陈旧项记入观察项（R-4），修正归其它变更。

### 现象三：不裁定粒度就必然长成一物一价

模型最贵的成本不是写，而是**判据缺失**：哪些目录算一个元素、哪个文件算一个组件、哪条依赖该连边。若不在初版钉死，第二个人就会按自己的口味加元素，随后出现两种同级写法（`App.Backend.Core.Checks` 与 `App.Backend.ChecksImpl` 并存），画布层级与 FQN 前缀推导立刻失效。判据必须在**元素数最少但语义最有用**的点上闭合，并且机械可核（否则判据只是口号）。

### 症结一：内容必须由判据推导，不能由人挑选

初版模型的每一元素、每一条边都要能回答「凭什么有它」。故本变更把内容全部收敛到两类机械判据：

```
元素 ← 目录 / 文件集合的覆盖判据（覆盖完整 + 无多余，双向核对）
关系 ← 声明的依赖事实（Cargo [dependencies]）或 import 语句证据
```

判据之外的任何元素与边一律不写——包括「觉得应该连一条」的直觉边。

### 症结二：内容一落地就要过模型自身的四类图诊断

模型不是自由文本，它落地即被 `cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy` 四类规则校验，任一命中即整张图停摆。规划期已在本仓实测出两个必踩的坑，两者都不是「注意一点」能规避的，必须靠口径裁定：

| 坑 | 实测 | 裁定 |
|---|---|---|
| **dev-dependency 成环** | `workflow --dev--> orchestration` 与 `orchestration --> workflow`（生产）并存；`orchestration --dev--> store` 同理 | 关系只取 `[dependencies]`；dev 边排除并写入判据（否则 `cycle` 停摆） |
| **包含关系当依赖写** | 若把 `components/standard/hooks/` 归给容器元素 `App.Frontend.Components.Standard`，则 `Dailog → Standard` 这条「子 → 父」边立刻是 `dependsOnAncestor` | 包含由 FQN 层级表达；容器元素 `path` 恒空（唯一例外：barrel），hooks 目录不入模型 |

### 症结三：Cargo 依赖是三类事实混装

一个 crate 的依赖面同时含：workspace 内包（`{ workspace = true }`）、workspace 内包（`{ path = … }`）、外部 crate，且分 `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]` 三段。本变更采用封闭口径：**仅 `[dependencies]` 段内指向 workspace 内包的声明**（两种书写形式都算）。这条口径有两个直接后果，都是想要的：

- `export-bindings → dev-team`（`path = ../../../`）被纳入 ⇒ 工具 crate 反向依赖壳这一**架构事实**第一次显式可见；
- `orchestration` / `workflow` 的 test-only 互依被排除 ⇒ 图无环（R-7 记录其代价）。

## 提案

一次 change 把空模型填成基线：**49 元素 / 107 关系 / 6 个模型文件 / 1 份 ADR**，全部由判据推导、机械可核、零代码改动。关系侧拆开即 **107 = 43（Cargo 依赖声明）+ 64（前端 import 证据）**——两个数都按本提案 §2 的判据逐条枚举得出，不存在第三个口径。

```
openspec/architecture/
├── models/
│   ├── 01-app.toml              App · App.Backend · App.Frontend（3 容器）
│   ├── 02-backend-core.toml     Core 容器 + 7 个 core crate
│   ├── 03-backend-infra.toml    Infra 容器 + 6 个 infra crate
│   ├── 04-backend-shell.toml    Shell（Tauri 壳 / 命令面）+ 12 条出边
│   ├── 05-backend-tools.toml    Tools 容器 + export-bindings + 1 条反向边
│   └── 06-frontend.toml         Frontend / Views / Components 全部元素与前端边
└── decisions/
    └── 0001-model-granularity.md   粒度与口径裁定（供人与 agent 共读）
```

### 1. 元素面：两类元素 + 三档粒度

元素分**容器**与**叶**两类，判据与职责完全分离：

| 类 | 判据 | `path` | 参与关系 |
|---|---|---|---|
| 容器 | 包根、域、分层（`App` / `App.Backend` / `App.Backend.Core` / `App.Frontend` / `App.Frontend.Views` …） | 恒空（唯一例外见下） | 仅当 import 命中其声明的文件（barrel） |
| 叶 | 可单独表达依赖方向、可单独收紧 `gate` 的最小单位 | 必填（`./` 前缀 repo 相对） | 是 |

粒度三档（本期封闭裁定）：

| 档 | 元素 | 数量 | 判据 |
|---|---|---|---|
| 后端到 crate | `App.Backend.Core.{Foundation,Config,Workflow,Archi,Orchestration,Agent,Checks}`、`App.Backend.Infra.{AgentRuntime,ChecksRuntime,Exec,Store,VcsRuntime,Watch}`、`App.Backend.Tools.ExportBindings`、`App.Backend.Shell` | 15 | `crates/{core,infra,tools}/<dir>/` 每个成员 crate 一元素（裸名按 CamelCase），crate **内部** `.rs` 不建元素；壳 = `src-tauri/src/` |
| 前端到视图 | `App.Frontend.{Shell}`、`App.Frontend.Views.{Agents,AgentDebug,Architecture,Changes,Config,DbInspector,Explores,Info,Welcome}`、视图内子域 `App.Frontend.Views.Changes.{Flow,Renderers,Agent}` | 13 | `views/<view>/` 每个视图目录一元素（`welcome-view.tsx` 单文件一元素）；子域 = 视图内**除 `components` / `hooks` 外**且非测试源文件 ≥ 3 的子目录 |
| 前端到组件 | `App.Frontend.Components.Agent.{AgentBlocks,AgentError,AgentInput,AgentMessages,AgentSessionView,ToolCards}`、`App.Frontend.Components.Standard.{Dailog,Drawer,Pagination,Select}`、`App.Frontend.Components.Ui` | 11 | `src/components/agent/*.{ts,tsx}` 与 `src/components/standard/*.tsx` 每文件一元素（排除测试与 barrel `index.tsx`）；`src/components/ui/` **收敛为 1 个元素**（shadcn 原语，35 文件，不承担架构决策） |

合计：容器 10 + 叶 39 = **49**。

**封闭排除面**（不建元素、不参与覆盖、不贡献边）：`src/hooks/**`、`src/lib/**`、`src/types/**`（含 `types/generated/bindings.ts`）、`src/components/standard/hooks/**`、`*.test.{ts,tsx}`、`src/test-setup.ts`。理由逐条：

- `hooks` / `lib` 是**视图内的实现细节**（explore 明示暂不解析）：其 import 仍计入所属视图元素（归属按目录，粒度只看建不建元素——两者不是一回事）；
- 顶层 `src/hooks` / `src/lib` / `src/types` 无归属元素 ⇒ 其 import 零贡献（代价见 R-4：壳对命令面的依赖缺载体，故无边）；
- `components/standard/hooks/` 不入模型是为了守住「容器 `path` 恒空」——把 hooks 归给容器就把「子 → 父」写成了依赖（症结二第二行）。

### 2. 关系面：只表达依赖，不表达包含

三条裁定：

1. **包含不连边**：`App.Frontend.Views.Changes.Flow` 属于 `App.Frontend.Views.Changes` 由 FQN 前缀表达，MUST NOT 写 `relation`（写了即 `dependsOnAncestor`）。唯一的父子方向边是「父 **import** 子」（如 `Changes` 组装 `Flow` 的画布组件）——那是真依赖，不是包含。
2. **后端边 = Cargo 事实**：`[dependencies]` 段内指向 workspace 内包的声明（`{ workspace = true }` 或 `{ path = … }` 两种书写），方向 **依赖者 → 被依赖者**，dev / build 段排除。本期 43 条，其中 `export-bindings → App.Backend.Shell` 是唯一反向边。
3. **前端边 = import 证据**：只由**已声明元素覆盖范围内**文件的 `import` 语句证成，目标侧按「被 import 路径的最具体已声明元素」归属；`types/generated/bindings` 的 import 映射为**命令面依赖**（指向 `App.Backend.Shell`，`desc` 写明契约载体）；指向 `hooks/` / `lib/` / `types/dto` 的 import 不入边（不解析面）。本期 **64 条 / 22 个源**。

**64 的构成（须显式记住的一笔）**：64 = 58 条叶 → 叶 / 叶 → 容器边 + **6 条 `App.Frontend.Components.Agent` 的 barrel 端点边**（2 条出边：barrel `index.tsx` 重导出 `AgentInput` / `AgentSessionView`；4 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）。这 6 条不是漏洞而是**两条已裁定规则的必然乘积**：§3 的「容器 `path` 恒空，唯一例外是持 barrel 的 `App.Frontend.Components.Agent`」× §2 的「`@/components/agent`（barrel）→ `Components.Agent`」——只要两条规则都在，容器就必然成为关系端点。因此本基线按 **64** 落笔、并让容器作端点仅限这一例；若要回到「容器不入边」的 58，必须撤销 barrel 例外（改字段规则）或改映射规则（把 barrel import 归给其重导出的叶），两者都属 `repo-archi-model` 的 delta，只能在本提案闭合，不能在实现期折中。

双重核对同值：文件侧 `0（01）+ 9（02）+ 21（03）+ 12（04）+ 1（05）+ 64（06）= 107`，取数侧「后端 43 + 前端 64 = 107」；后端 43 拆在 02/03/04/05 四文件（9 + 21 + 12 + 1）。

实测四类图诊断**全部为零**：无环、无祖先前置边、无未定义引用、无孤儿层级；`archi_graph` 正常态（非停摆）。

### 3. 字段使用约定（贯穿全模型）

| 字段 | 约定 |
|---|---|
| `kind` | 包根 `package`；域 / 分层容器（含 `Components.Agent` / `Components.Standard`）`domain`；crate、视图、视图内子域、壳、`components/ui` `module`；共享组件文件 `component` |
| `gate` | **全部 `off`** —— 冷启动 ratchet（新元素缺省 `off`，想收紧的模块后续逐个改 `strict`，一个 change 收紧一块；本变更不收紧任何元素） |
| `summary` | 一行中文动词短语，说职责，不重复 FQN 字面（如 `App.Backend.Infra.Store` → 「native_db 本地库边界（按 workspace 分文件）」） |
| `path` | 叶元素必填、`./` 前缀、目录以 `/` 结尾、文件精确到扩展名；容器空（`App.Frontend.Components.Agent` 例外：持有 barrel `index.tsx`） |
| `docs` | 仅包根元素 `App` 显式指仓库面文档（`./README.md` / `./AGENT.md`）；其余一律留空。**MUST NOT 指向 `openspec/specs/**`**（守 `desktop-archi-module-docs` 的迁移边界——把 specs 塞进 `docs` 等于让模型反向依赖将被替代的语料） |
| `metadata` | 本期全空（`owner` / `tier` 一类自由键会变成与 `Cargo.toml` / 目录结构并行的第二事实源） |
| `desc` | 一句中文用途，说明「为什么依赖」而非复述方向；MUST NOT 把证据清单（import 文件列表）写进模型——证据由核对命令逐条取，模型内不存第二事实源 |

### 4. ADR：把判据写在图旁边

`openspec/architecture/decisions/0001-model-granularity.md` 记录本变更的六条裁定（三档粒度 / 容器 `path` 恒空 / dev 边排除 / `components/ui` 收敛 / `gate` 全 `off` / 排除面清单），正文引用相关元素 FQN ⇒ 经 `decisions/*.md` 文本源自动关联到这些元素（`docs` 三源之一），不需要任何额外机制。

写 ADR 而不仅写 proposal 的理由：proposal 归档后进 `changes/archive/`（历史事实，不再被读），而粒度判据要在**日常**（下一个 change 加元素时）可见——ADR 随架构域一起被读、被检索。

### 5. 一致性闭环：先立判据，机制化归 check 侧

本期把三条核对落成**命令级**验收（覆盖双向、关系证据逐条、四类诊断零命中），并在 change 报告留痕；**不新增任何测试代码**——模型是仓库数据资产，把它塞进 `crates/core/archi` 的测试语料会让 crate 单测耦合本仓内容（crate 语料应自足、可移植）。机制化（把核对做成门禁）属 `archi-check-and-gate` 的射程。

### 6. 本期不做

| 不做 | 归属 |
|---|---|
| 代码扫描、归属核对、漂移标记、门禁挂载、报告结构 | `archi-check-and-gate` |
| 解析器 / 图算法 / 画布 / 命令 / 工具任何代码改动 | 已在位，零改动 |
| 模块文档（各元素的 `README.md`） | 文档跟代码走且是人的产物（`desktop-archi-module-docs`）；本期由 `summary` 承载一行说明，不批量生成空壳文档 |
| `gate` 收紧（任一元素设 `warn` / `strict`） | ratchet：先全 `off`，收紧按模块独立变更 |
| 架构域 `config.json`（扫描排除键集等） | 归 check 侧（`foundation::layout` 的「第二消费者到场才下沉」纪律） |
| 修正 `desktop-crate-layout` 的陈旧依赖段 | 独立变更（R-4），本变更不以模型反向改 spec |

## 能力

### 新增的能力

- **repo-archi-model**（仓库级资产不变量，命名沿 `repo-surface-scope` 先例——它描述的是 `openspec/architecture/models/` 这份仓库资产的口径，而非某个产品功能面）：
  ① 基线落位与文件划分（`models/NN-<域>.toml` 六文件、字典序聚合、字段使用约定、`gate` 全 `off`）；
  ② 后端 crate 级覆盖与依赖边口径（14 个 workspace crate 各恰一元素、crate 内部不建元素、边只取 `[dependencies]` 的 workspace 内声明、dev 边排除）；
  ③ 前端视图与组件级覆盖与 import 边口径（视图 / 视图内子域 / 共享组件三档、`components/ui` 收敛、封闭排除面、`types/generated/bindings` → 命令面边）；
  ④ 图自洽与零诊断（四类结构诊断零命中、容器 `path` 恒空的成环/祖先禁令、停摆禁令）；
  ⑤ 覆盖判据的双向核对与漂移纪律（集合等式「恰被覆盖」、新增 crate / 视图 / 视图内子域 / 共享组件须同 change 补声明、判据外元素与边一律不写）；
  ⑥ 模型与裁定记录的可检索性（`archi_search` 命中元素薄条目与 ADR 文档条目、`archi_module` 给出一跳邻域与 `modelFile` / `modelLine` 定位，两工具契约零变化）。

### 修改的能力

无。本变更是**纯数据 + 纯记录**：不改解析格式、不改投影、不改渲染、不改工具、不改命令、不改 crate 图。

### 零改动能力与理由

- **desktop-archi-model**：模型格式、字段封闭、多文件聚合、诊断九类、图单点、gate 写回、diff 全部按现有契约被**使用**而非被修改——本变更的模型文件是它的第一份真实输入，若格式或解析有缺口会在验收中暴露（届时按缺陷另议，不在本变更内扩字段）。
- **desktop-archi-canvas**：两处挂载、1-hop 缺省、停摆态、标记词汇零变化；本变更只让画布第一次有图可画（`/architecture` 从空态转正常态）。
- **desktop-archi-agent**：`archi_module` / `archi_search` 契约、注入矩阵、只读档位零变化；本变更只让两工具第一次有语料可返回。
- **desktop-archi-module-docs**：三源关联与四字段契约零变化；本变更**不新增**被关联的模块文档（`docs` 仅包根指根文档），ADR 经既有 `text` 源自动关联。
- **desktop-crate-layout**：crate 图与依赖规则零变化（本变更一条 `Cargo.toml` 都不改）；其 spec 的陈旧段记入 R-4，不作为本变更的 delta。
- **workspace-layout-resolution**：`archi_dir` 族派生函数零变化（本变更消费它们，不改它们）。
- **repo-surface-scope**：新增 capability 对应一份可指认的存活资产（`openspec/architecture/**`），不退役任何资产、不留空壳；活跃面零新增悬空路径引用。
- **desktop-ipc-type-bindings / desktop-workspace-config / desktop-workspace-store / desktop-change-*** 等**：零触碰（无命令、无配置键、无表结构、无流程行为变化）。

## 变更范围

### 实现文件（全部为新增数据 / 文档，零源码改动）

| 文件 | 改动 |
|---|---|
| `openspec/architecture/models/01-app.toml`（新） | 3 个容器元素：`App`（`package`，`docs` 指 README / AGENT）、`App.Backend`、`App.Frontend` |
| `openspec/architecture/models/02-backend-core.toml`（新） | `App.Backend.Core` 容器 + 7 个 core crate 元素 + 9 条 core 内边 |
| `openspec/architecture/models/03-backend-infra.toml`（新） | `App.Backend.Infra` 容器 + 6 个 infra crate 元素 + 21 条边（含 `store → workflow` 等跨界边） |
| `openspec/architecture/models/04-backend-shell.toml`（新） | `App.Backend.Shell` 元素 + 壳 → 12 个 crate 的出边 |
| `openspec/architecture/models/05-backend-tools.toml`（新） | `App.Backend.Tools` 容器 + `ExportBindings` 元素 + 反向边 `ExportBindings → App.Backend.Shell` |
| `openspec/architecture/models/06-frontend.toml`（新） | 前端 28 个元素 = **4 容器 + 24 叶**（容器 `Views` / `Components` / `Components.Agent` / `Components.Standard`；叶 `Shell` 1 + 9 视图 + 3 视图内子域 + 10 个共享组件 + `Ui` 1；`App.Frontend` 容器在 `01`，此处 MUST NOT 重复声明）+ **64 条前端边**（22 个源；含 `Components.Agent` 的 2 出 / 4 入共 6 条 barrel 端点边） |
| `openspec/architecture/decisions/0001-model-granularity.md`（新） | 六条裁定：三档粒度、容器 `path` 恒空、dev 边排除、`components/ui` 收敛、`gate` 全 `off`、封闭排除面；正文引用元素 FQN |

### 测试文件

无新增测试。核对以**命令 + 留痕**完成（模型是数据资产，不作为 crate 语料——见提案 §5）：

| 核对 | 命令形态（在 change 报告内留痕） |
|---|---|
| 双向覆盖 | 枚举「判据命中的 crate 目录 / 视图目录 / 共享组件文件」集合与「模型 `path` 集合」，断言两集合相等（无漏无多） |
| 关系证据 | 后端逐边比对 `Cargo.toml` 的 `[dependencies]` workspace 声明；前端逐边 `grep` 到具体 `import` 语句 |
| 图自检 | `archi_graph(root)` 返回正常态、`elements` = 49、`relations` = 107（后端 43 + 前端 64）、`diagnostics` 为空、`warnings` 为空 |

### 不要修改

| 面 | 理由 |
|---|---|
| `src-tauri/crates/core/archi/**`（含 `tests/corpus/**`） | 本变更零代码改动；把本仓模型塞进 crate 语料破坏「语料自足」 |
| `src-tauri/src/commands/archi/**`、`src-tauri/src/bindings/**` | 命令面与 bindings 零变化 ⇒ 不触发 `bindings:check` |
| `src/views/architecture/**`、`src/routes.tsx` | 画布与路由零变化 |
| `src-tauri/crates/core/foundation/src/layout/**` | 派生函数零变化 |
| `openspec/config.json`、`.gitignore` | 无新增配置键、无新增忽略项（`openspec/architecture/` 必须入库） |
| `openspec/specs/desktop-crate-layout/spec.md` | 陈旧依赖段的修正属独立变更（R-4）；本变更不动主基线 spec |
| `README.md` / `AGENT.md` / `CLAUDE.md` | 根文档的架构叙事改写不在本期（R-4 观察项） |
| `packages/desktop/package.json` / `tauri.conf.json` / `src-tauri/Cargo.toml` | 无 desktop 源码改动 ⇒ 不做版本 bump（沿「版本随 desktop 源码变更」纪律）；若评审视模型为 desktop 资产变更，则 `package.json` 0.6.0 → 0.6.1（R-8） |

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | **模型可解析、零诊断**：`archi_graph(root)` 返回正常态，`diagnostics` 空、`warnings` 空；`elements` = 49、`relations` = 107（= 后端 43 + 前端 64，两数可分别按 AC-2/AC-3 与 AC-4/AC-5 的判据枚举复算）；`/architecture` 页面渲染元素树 + 画布 + 抽屉三段且无结构错面板 | 命令实调 + 页面实跑 |
| AC-2 | **后端覆盖齐备**：「14 个 workspace 成员 crate 目录」与「crate 元素 `path` 集合」双向相等（7 core / 6 infra / 1 tools），另加 `App.Backend.Shell` 覆盖 `src-tauri/src/`；crate 内部 `.rs` 零元素（`grep` 元素 FQN 名单与 `crates/*/*/src/*.rs` 无交集） | 集合比对命令 + `ls` 清单 |
| AC-3 | **后端边口径**：边集 == 各 `Cargo.toml` `[dependencies]` 段内指向 workspace 内包的声明集合（`{ workspace = true }` 与 `{ path = … }` 两种形式均计），逐条可比、43 条；dev 边零出现（`workflow --dev--> orchestration`、`orchestration --dev--> store` 为在案反例）；`export-bindings → dev-team` 反向边在案 | 脚本比对 `Cargo.toml` + 模型 `[[relation]]` |
| AC-4 | **前端覆盖齐备**：「9 个视图 + 3 个视图内子域 + 10 个共享组件文件 + `components/ui`」与模型前端叶元素 `path` 集合双向相等；封闭排除面无元素（`src/hooks` / `src/lib` / `src/types` / `components/standard/hooks` / 测试 / `test-setup.ts`）；`views/changes/components`（1 文件）不构成子域 | 集合比对命令 |
| AC-5 | **前端边口径**：**64 条**边（22 个源）每条可指到覆盖范围内某文件的 `import` 语句（证据清单在 change 报告留痕）；其中 **6 条为 `App.Frontend.Components.Agent` 的 barrel 端点边**（2 出 `AgentInput` / `AgentSessionView` + 4 入 `Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`），其余 58 条为叶 → 叶 / 叶 → 容器边；`types/generated/bindings` 的 import 恰映射到 `App.Backend.Shell`；指向 `hooks/` / `lib/` / `types/dto` 的 import 零边；除该 barrel 外无容器作关系端点；无 `dependsOnAncestor` 形态的父子边 | `grep` 逐边核对 + 诊断面板零命中 |
| AC-6 | **图自洽**：零 `cycle` / 零 `dependsOnAncestor` / 零 `unknownReference` / 零 `orphanHierarchy`；容器元素 `path` 恒空（唯一例外 `App.Frontend.Components.Agent` 持 barrel）；容器作关系端点仅限该 barrel 一例，且其端点边恰 **6 条**（2 出 + 4 入） | 诊断零命中 + 字段审查 |
| AC-7 | **字段纪律**：49 元素 `gate` 全 `off`；`summary` 全非空且一行中文；叶元素 `path` 全 `./` 前缀；`docs` 仅 `App` 非空且不指向 `openspec/specs/**`；`metadata` 全空；每条 `desc` 为一句用途且不含证据路径清单 | 模型全文审查（`grep` 计数） |
| AC-8 | **落位与文件划分**：6 个模型文件在 `openspec/architecture/models/` 且文件名 `NN-<域>.toml`；字典序聚合后零 `duplicateElement`；ADR 在 `openspec/architecture/decisions/` 且正文引用元素 FQN（`archi_module` 的 `docs` 中出现该 ADR 条目，`source = "text"`） | 目录审查 + `archi_module` 响应 |
| AC-9 | **零代码改动**：`git diff --stat` 只含 `openspec/architecture/**` 与本 change 目录；静态检查 / 测试结果与基线一致（3239 通过、覆盖率不降、`bindings:check` 零 diff） | `git diff` + 静态脚本（桌面端执行） |
| AC-10 | **检索可用**：`archi_search(query="archi", scope=all)` 命中元素 `App.Backend.Core.Archi`（薄条目）与 ADR 文档条目（`kind = adr`、`source = text`、恰四字段）；`archi_module(element="App.Backend.Core.Archi")` 返回出边 `Foundation`、入边 `AgentRuntime` / `Shell`，`modelFile` + `modelLine` 指向 `02-backend-core.toml` 的该表头 | 工具实调（agent 会话内一次） |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **粒度争议**：crate 级 / 视图级 / 组件文件级三档是裁定而非唯一正解，「为什么 `changes/flow` 是元素而 `changes/components` 不是」会被反复质疑 | 评审反复，或后人各按口味加元素导致层级失效 | 高 | 判据机械化（子域 = 非 `components`/`hooks` 且非测试源文件 ≥ 3）+ ADR 在案（随架构域日常可读）；偏离判据必须改本 capability 的 requirement，不能就地加元素 |
| R-2 | **前端边随代码漂移且本期无门禁**：新增一个 `import` 不会让模型变化，图会静默落后 | 半年后前端关系图不可信 | 高（必然发生） | 漂移判定与门禁归 `archi-check-and-gate`；本期先把「判据」写死（漂移判定所需的对拍口径已在 spec），并把「新增视图 / 组件 / 依赖须同 change 补声明」写成 requirement + 验收口径；`/architecture` 页面是人工复核入口 |
| R-3 | **容器作端点（barrel 例外）与「包含 ≠ 依赖」的边界模糊**：`Components.Agent` 既是分组又持有 `index.tsx`，因而必然产生 **6 条端点边**（2 出 + 4 入，见 §2 的 64 构成） | 后人误以为「容器也能连边」⇒ 把「父目录」当容器随手连边即 `dependsOnAncestor` 停摆；或误按「容器不入边」把前端边数改回 58 而与字段规则冲突 | 中 | spec 明写「容器 `path` 恒空，唯一例外是 barrel」+「容器作端点仅限该 barrel」，两处都带 6 条端点边的显式记数；模型自检的 `dependsOnAncestor` 是机械兜底（写错即停摆，不会静默） |
| R-4 | **模型与既有散文事实不一致**：`desktop-crate-layout` 的依赖段陈旧（「`agent-runtime` 仅依赖 `agent`」与 8 条实际边相悖、六条边未登记、`archi` 仍标「未来落地」），`AGENT.md` 的 crate 清单不含 `core/archi` 且前端零分层；且顶层 `src/hooks` 无元素 ⇒ 壳对命令面的依赖在图上缺载体（`App.Frontend.Shell → App.Backend.Shell` 无边） | 同一事实两处来源、两套说法；壳层跨端依赖不可见 | 高（已发生） | 本变更以 `Cargo.toml` 事实为准并在 ADR 注明「模型以 Cargo 声明为取数口径」；两处散文的修正列观察项（独立变更）；壳层缺边记入 ADR 的已知缺口，加 `src/hooks` 元素即可补齐（届时改本 capability） |
| R-5 | **`components/ui` 收敛的代价**：单元素使「某视图只用到 `dropdown-menu` 而不碰别的 ui 原语」不可见 | 细节丢失，但不影响架构决策（ui 是叶子原语层，恒被依赖） | 低（已裁定） | ui 原语不承担「谁能依赖谁」的判定；若将来 ui 内部出现「框架层 / 装饰层」分层，再拆元素（改本 capability） |
| R-6 | **元素 49 / 边 107 的图规模**：`archi_module` 无参调用返回 49 条薄概览、画布全量铺开对 1-hop 之外的导航不友好 | agent 概览偏长；画布需靠 1-hop 交互 | 低 | 画布缺省 1-hop（既有契约）+ 元素树按 FQN 前缀导航；49 条薄概览远低于单结果字节上限 |
| R-7 | **dev 边排除导致测试用依赖不可见**：`workflow ⇄ orchestration` 的 test-only 互依被隐去 | 「为什么这两个 crate 在测试里互相引用」在图上不可见 | 中（已裁定） | 排他性口径写进 ADR：包含成环的信息由 `Cargo.toml` 承载，模型不重复表达（模型一旦纳入 dev 边即 `cycle` 停摆，代价远大于收益） |
| R-8 | **版本纪律的口径争议**：本变更零 desktop 源码改动，故不做 `package.json` bump | 若评审要求 bump（视为 desktop 资产变更），交付面需补一行版本改动 + `tauri.conf.json` 跟随 | 低 | 变更范围已显式记录两种口径与首选（不 bump）；若裁决为 bump，改动为 0.6.0 → 0.6.1 一处值 |
| R-9 | **`gate` 全 `off` 使本变更零门禁效果** | 有人期望「模型落地即受约束」的错觉 | 中（已裁定） | ratchet 是既有裁定（新元素缺省 `off`，收紧一个模块一个 change）；本变更的产出是**事实基线**，不是门禁；门禁归 `archi-check-and-gate` |
| R-10 | **逐量核对的成本**：107 条边（后端 43 + 前端 64）逐条留证据属人工可审、脚本可跑，但本变更不落成测试 | 每次复核靠人跑命令 | 中 | 三条核对命令写进验收表并留痕；机制化（断言脚本进门禁）归 check 侧，本期不预铺（避免在无门禁框架时先造半成品校验器） |
