# 设计: init-archi-models

> **变更**: init-archi-models
> **日期**: 2026-10-27
> **基于**: `proposal.md` 与 `specs/repo-archi-model/spec.md`

---

## 提案与规格同步状态

本变更零源码改动，产出面是**一份仓库数据资产**（`openspec/architecture/models/*.toml` 六文件）与**一份口径记录**（`openspec/architecture/decisions/0001-model-granularity.md`）。proposal 已把范围、判据、排除面、验收方法与风险裁定完毕；本设计不重复论证，只做三件事：

1. 把六条机械判据落成**可逐行抄写的内容计划**（每文件的元素表与关系表 —— 即模型全文的骨架），使实现相位变成「按表落笔 + 对齐证据」而非再裁定一次；
2. 把判据在真实仓库上**实测一遍**，给出每个数的来源（元素 49 / 后端关系 43 / 前端关系 64），并把实测记数写回 proposal 与 `specs/repo-archi-model/spec.md`（关系总 107 = 43 + 64，前端 64 = 58 + 6 条 barrel 端点边）——三处口径在本相位**闭合**，对拍表见「数值口径闭合（49 / 43 / 64 / 107 / 6）」；
3. 给出 ADR 的章节结构与必写条目（正文必须逐字出现 FQN，否则 `docs` 的 `text` 源不关联）。

**口径锚点**：模型消费面（`crates/core/archi` 的 `parse` / `graph` / `docs` / `dto`、`/architecture` 页面、`archi_search` / `archi_module` 工具、`foundation::layout::archi_*` 派生函数）**全部零改动**——本变更只填空目录 `openspec/architecture/`（实测不存在）。格式事实照抄现有实现：

- 元素 = `[element."<FQN>"]` 表头（键为带引号单键）+ `kind`（必填，四值）/ `gate`（缺省 `off`）/ `path`（字符串数组）/ `docs`（字符串数组）/ `summary`（字符串）/ `metadata`（字符串→字符串内联表）；未知字段由 `deny_unknown_fields` 拒收（`parse/toml.rs`）；
- 关系 = `[[relation]]` + `from` / `to`（必填）+ `desc`（可选）；`modelLine` 取 `from` 键行；
- 聚合 = `models/*.toml` 按**文件名**字典序拼接（`parse_dir`），跨文件重复 FQN 即 `duplicateElement`；
- 层级 = FQN 最长前缀（`graph.rs`），祖先未显式声明即 `orphanHierarchy`；
- ADR 关联 = `docs.rs` 对 `decisions/*.md` 做 `content.contains(element.fqn)`（**逐字子串**，不做分词），故正文必须写完整 FQN。

### 起点核实（工作树实况）

- `openspec/architecture/` **不存在**（`models/` / `decisions/` 均缺席）；`openspec/` 现有 `changes/` `config.json` `explores/` `specs/` `todo.md`；`.gitignore` 无 `openspec` 相关行（无需改忽略项）。
- workspace 成员 14 个：`crates/core/{agent,archi,checks,config,foundation,orchestration,workflow}`、`crates/infra/{agent,checks,exec,store,vcs,watch}`、`crates/tools/export-bindings`；壳包 `dev-team` 位于 `src-tauri/`（`Cargo.toml` 同时是 workspace 根）。
- 前端：`src/views/` 9 个视图项（`agent` `agents` `architecture` `changes` `config` `db` `explores` `info` 目录 + `welcome-view.tsx` 单文件）；`src/components/agent/` 6 个源文件 + `index.tsx` barrel；`src/components/standard/` 4 个 `.tsx` + `hooks/`；`src/components/ui/` 35 个文件。
- 仓库内 `crates/**` 与 `src/**` 下**无** `*.md` ⇒ 元素的邻近文档源为空，`docNotFound` 零告警；根 `README.md` / `AGENT.md` 存在 ⇒ `App.docs` 两条可解析。
- `packages/desktop/package.json` = `0.6.0`；`src-tauri/Cargo.toml` 的 crate version = `0.1.0`（不随动）。

### 事实校准（不改范围，只定形态与计数）

1. **关系数实测为 107（后端 43 + 前端 64）**：后端 43 逐 crate 核对 `[dependencies]` 得到；前端 **64**，其中 **6 条恰为 `App.Frontend.Components.Agent` 这一 container 的端点边**——2 条出边（barrel `index.tsx` 重导出 `AgentInput` / `AgentSessionView`）+ 4 条入边（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）。spec 的字段规则（容器唯一例外持 barrel 的 `App.Frontend.Components.Agent`）与映射规则（`@/components/agent`（barrel）→ `Components.Agent`）都是 MUST，两条合起来必然产出这 6 条边。**58 不是独立口径，只是 64 的分解项**（58 = 叶 → 叶 / 叶 → 容器的边；58 + 6 = 64）；任何把它当作「前端边总数」的读法都与 spec 的字段规则冲突。裁定见 D2，数值对拍见下节。
2. **视图内 `hooks/` 作为 source 计入依赖**：11 条命令面边（全部指向 `App.Backend.Shell`）的来源集合与 spec 列的 11 个元素**逐条吻合**（`Views.AgentDebug` / `Views.Agents` / `Views.Architecture` / `Views.Changes` / `Views.Changes.Flow` / `Views.Changes.Agent` / `Views.Config` / `Views.DbInspector` / `Views.Explores` / `Views.Info` / `Components.Agent.AgentSessionView`），其中 `Views.DbInspector`（`views/db/hooks/use-db-inspector.ts`）与 `Views.Changes.Agent`（`views/changes/agent/hooks/use-change-chat.ts`）两条**只**由视图 `hooks/` 文件的 import 证成——与「视图内 hooks 不建元素但其 import 计入所属视图」逐字一致。
3. **子域判据必须递归计数**：`views/changes/agent/` 顶层只有 2 个非测试源文件（`change-chat-composer.tsx` / `change-chat-panel.tsx`），加上其 `hooks/use-change-chat.ts` 才是 3 —— spec 记作「`agent`（3）」，故阈值的计数口径 = **递归**统计该子目录内非测试源文件，`components` / `hooks` 的排除只作用于**候选子域目录名本身**（不递归剔文件）。按此口径 `flow` 13 / `renderers` 7 / `agent` 3 达标，`components` 1 不达标且目录名被排除。
4. **`App.Frontend.Shell` 的 `path` 是 4 项数组**（`app.tsx` / `routes.tsx` / `main.tsx` / `components/app-sidebar.tsx`）；`app-sidebar.test.tsx` 属封闭排除面，不入 `path`。
5. **`gate` 一律显式写 `off`**（不靠缺省）：AC-7 要求「49 元素 `gate` 全 `off`」，显式行使 `grep -c 'gate = "off"'` 可直接对 49，且后续 ratchet 收紧时定位面唯一。
6. **dev 边实测 4 条而非提案举例的 2 条**：`workflow --dev--> orchestration`、`orchestration --dev--> store`、`workflow --dev--> store`、`checks-runtime --dev--> workflow`。其中 `workflow --dev--> orchestration` 与生产边 `orchestration --> workflow`、`workflow --dev--> store` 与生产边 `store --> workflow` 各自成环——排除规则必须**按 `[dev-dependencies]` 段落整体执行**，只按举例排除两条会漏掉 `workflow --dev--> store` 并同样停摆（排除后关系总数不受影响，仍是 43）。

### 数值口径闭合（本相位闭合，不留待评审）

模型全文只有五个数需要对齐：元素 **49**、后端关系 **43**、前端关系 **64**、关系总 **107**、barrel 端点边 **6**。三处权威文本已逐项同值；本设计的内容计划按同值落笔，实现相位照表落笔即得同值。

| 数 | `proposal.md` | `specs/repo-archi-model/spec.md`（本 change 的 delta，可在本相位修改） | 本设计落点 |
|---|---|---|---|
| 元素 49 | AC-1 / AC-2 / AC-4 / AC-7 | 合计段（`3+8+7+1+2+28=49`，容器 10 + 叶 39）与「正常态」Scenario | 「计数恒等式」第一行 + 各文件元素表（3/8/7/1/2/28） |
| 后端关系 43 | AC-3 | 后端关系要求 + 逐文件表（`9 + 21 + 12 + 1 = 43`） | §02–§05 关系表（9 / 21 / 12 / 1）+ 「关系推导规则 · 后端」 |
| 前端关系 64 | AC-1 / AC-5 | 计数恒等式要求 +「每条边可指到 import 语句」Scenario | §06 关系表（22 个源，逐行可加 = 64） |
| 关系总 107 | AC-1 | 合计段 `0+9+21+12+1+64=107` | 「计数恒等式」第二行 |
| barrel 端点边 6 | AC-5 / AC-6 | 「容器端点边恰为 barrel 的 6 条」Scenario | D2 + §06 关系表的 `Components.Agent` 2 出 + 4 入 |

闭合声明与备选记录：

- **闭合**：本 change 的 spec delta（`specs/repo-archi-model/spec.md`）就在 `openspec/changes/init-archi-models/` 内，属本相位可改产物；因此「proposal 与 spec 记数不一致」这类冲突必须、且已在本相位消除，不存在「留待评审 / 不在射程」的第三态。设计、proposal、spec 三处的 49 / 43 / 64 / 107 / 6 同值，且全文不存在第二个前端边总数（`58` 仅作为 `64 = 58 + 6` 的分解项出现）。
- **备选（不采纳）**：撤销 barrel 例外（容器一律无 `path`、不入边）会把前端边降到 58、关系总降到 101。该备选的代价是改 `repo-archi-model` 的**字段规则**（容器 `path` 唯一例外）与**映射规则**（barrel import 归属），并让「`components/agent/index.tsx` 无任何元素覆盖」使前端覆盖双向核对失真；同时 `App.Frontend.Components.Agent` 持有的 `index.tsx` 将无人声明。故不作为本设计的开口项，记录以备将来修订 capability 时对照。

### 验收对齐（每条 AC 的设计落点）

| AC | 本设计的落点 |
|---|---|
| AC-1 模型可解析、零诊断 | 「计数恒等式」（49 / 107）+「数值口径闭合」+「自洽预算」（四个零与 `warnings` 空）；107 的推导（43 + 64，含 6 条 barrel 端点边）见事实校准 1 与「关系推导规则」；`/architecture` 元素树 + 画布 + 抽屉三段渲染见架构组件「图表消费面（零改动）」 |
| AC-2 后端覆盖齐备 | §02 / §03 / §04 / §05 的元素表（15 个叶 + 5 个容器）+ D1（crate 内部 `.rs` 零元素） |
| AC-3 后端边口径 | 「关系推导规则 · 后端」+ 四份关系表（43 条）+ D3（dev / build 边排除、反向边在案） |
| AC-4 前端覆盖齐备 | §06 容器表 + 叶表（4 + 24）+ D1（三档判据、子域递归计数、封闭排除面） |
| AC-5 前端边口径 | 「关系推导规则 · 前端」+ §06 关系表（64 条，22 个源；其中 6 条为 `Components.Agent` barrel 端点边，其余 58 条）+ D4 + 「数值口径闭合」 |
| AC-6 图自洽 | 「自洽预算」四类诊断逐条给「为何为零」+ D2（容器 `path` 恒空与 barrel 唯一例外，端点边恰 6 条） |
| AC-7 字段纪律 | D6（`gate` 显式 `off`、`metadata` 不写、`docs` 仅 `App`、`summary` 一行、`desc` 一句）+ 各内容表的 `summary` 列 |
| AC-8 落位与文件划分 | 变更清单新增文件表（6 个 `NN-<域>.toml` + 1 份 ADR）+ 事实校准 3 / 4 + 「ADR 结构」（正文逐字 FQN） |
| AC-9 零代码改动 | D8（不 bump）+ 依赖表（无新增依赖）+ 变更清单「修改文件」注（全零 diff 面逐项列名） |
| AC-10 检索可用 | 「ADR 结构」的六个必写 FQN + 架构组件表的零改动工具面 + 依赖表的核对通道（元素 49 条薄概览、一跳邻域、ADR 文档条目） |

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 元素判据与计数 | 容器 10 + 叶 39 = **49**。后端叶 15 = 7 core crate + 6 infra crate + `export-bindings` + `Shell`（`src-tauri/src/`）；前端叶 24 = `Shell` 1 + 视图 9 + 视图内子域 3 + 共享组件文件 10 + `Ui` 1。crate **内部** `.rs` 零元素；视图 `components/` / `hooks/` / `lib/` 零元素；`components/ui/` 收敛为 1 | spec 逐条给定判据（crate 目录 / 视图目录 / 单文件视图 / 子域 ≥3 / 组件逐文件 / ui 收敛）；元素数最少而依赖方向可辨 |
| D2 | 容器 `path` 与 barrel 例外 | 容器 `path` 恒空，唯一例外 `App.Frontend.Components.Agent`（`path` = `src/components/agent/index.tsx`）。由此该容器**合法成为关系端点**（2 出 + 4 入，共 6 条）；其余 9 个容器零 `path`、零端点 | spec 的字段规则与目标归属规则都是 MUST；容器端点只在「它自己持有被 import 的文件」时成立，与「包含 ≠ 依赖」不冲突（`Children → Parent` 禁的是**前缀**方向，不是 `barrel → 同目录文件`） |
| D3 | 后端边口径 | 仅 `[dependencies]` 段内指向 workspace 内包的声明（`{ workspace = true }` 与 `{ path = … }` 同权），方向 **依赖者 → 被依赖者**；`[dev-dependencies]` / `[build-dependencies]` / `[target.*.dependencies]` 排除；反向边 `App.Backend.Tools.ExportBindings → App.Backend.Shell`（`path = "../../../"`）在案 | dev 边纳入即 `cycle` 停摆（实测 4 条 dev 边，其中 `workflow ⇄ orchestration` 与 `workflow ⇄ store` 两处成环，见事实校准 6）；`export-bindings → dev-team` 是真实架构事实（工具依赖壳），必须留痕 |
| D4 | 前端边口径 | **source 侧**：`App.Frontend.Shell` 的 4 个文件 + 各叶元素 `path` 覆盖范围内文件（含视图内 `hooks/` / `lib/`，它们不建元素但 import 计入所属视图）；顶层 `src/hooks` / `src/lib` / `src/types` / `components/standard/hooks` 与测试文件不作 source。**target 侧**：被 import 路径按「最具体已声明元素」归属；落在任一 `hooks/` `lib/` `types/` 的目标零边（唯一例外 `types/generated/bindings` → `App.Backend.Shell`）；`from == to` 不建边 | spec 逐条给定；source 侧含 hooks、target 侧不含 hooks，是「粒度 ≠ 解析面」的唯一自洽读法（否则 AC-5 的 11 个命令面来源无法全部成立） |
| D5 | 文件划分与关系归属 | 六文件按域切（`01` 包根 / `02` core / `03` infra / `04` 壳 / `05` tools / `06` 前端）；`App.Frontend` 只在 `01` 声明一次；每条 `[[relation]]` 写在**其 `from` 元素所在文件**内；`01` 零关系 | spec 的逐文件计数表 + 「关系归属与定位一致」；重复声明即 `duplicateElement`（并会使元素数变 50） |
| D6 | 字段纪律 | `kind`：包根 `package`；域/分层容器 `domain`；crate / 视图 / 子域 / 壳 / `Ui` `module`；共享组件文件 `component`。`gate` 显式 `"off"`（49 行）。`metadata` **整键不写**（不写 `metadata = {}`）。`docs` 仅 `App`（`["./README.md", "./AGENT.md"]`），其余不写 `docs` 行。`summary` 一行中文、不复述自身 FQN。`desc` 一句「为什么依赖」，不写 import 清单 / 依赖段位置 | AC-7 的可机械审查形态；`metadata = {}` 会让 `grep metadata` 非零，违背「未使用即不出现」 |
| D7 | ADR 的写法 | `decisions/0001-model-granularity.md` 正文分八节（见「数据模型 · ADR 结构」），六条裁定各带判据与反例；**正文逐字出现**至少 `App.Backend.Core.Archi` / `App.Backend.Tools.ExportBindings` / `App.Backend.Shell` / `App.Frontend.Components.Agent` / `App.Frontend.Components.Ui` / `App.Frontend.Views.Changes.Flow` 六个 FQN（`docs.rs` 用 `contains(fqn)` 关联）；ADR 路径 MUST NOT 写进任何元素 `docs` | 判据要随架构域日常可读（proposal §4）；`text` 源是既有机制，不新增关联字段 |
| D8 | 版本纪律 | **不做版本 bump**：零 desktop 源码改动（`packages/desktop/src/**`、`src-tauri/src/**`、`crates/**`、`bindings.ts` 全零 diff），`package.json` 保持 `0.6.0` | 沿「版本随 desktop 源码变更」纪律；若评审裁定模型属 desktop 资产变更，改动为 `package.json` `0.6.0 → 0.6.1` 单行（`tauri.conf.json` 自动跟随），记入 O-2 |
| D9 | 核对方式 | 三条核对（双向覆盖 / 关系证据 / 图自检）以**命令 + 报告留痕**执行；MUST NOT 新增测试代码、MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**` | 模型是仓库数据资产，塞进 crate 语料会让 crate 单测随本仓演进变红（spec 明令）；机制化归 `archi-check-and-gate` |
| D10 | 不预铺 check 侧 | 不建 `openspec/config.json` 架构域键、不写扫描排除键集、不挂门禁、不改 `archi` crate 任何字段 | proposal §6 与 `foundation::layout` 的「第二消费者到场才下沉」纪律 |
| D11 | 数值口径闭合 | 基线定稿为 **49 元素 / 107 关系（后端 43 + 前端 64）**，前端 64 = 58（叶 → 叶 / 叶 → 容器）+ 6（`App.Frontend.Components.Agent` barrel 端点边）；proposal（AC-1 / AC-5 / AC-6）、`specs/repo-archi-model/spec.md`（合计段与三个相关 Scenario）、本设计（内容计划与计数恒等式）三处同值；实现相位不得改判据迎合数字，也不得以「容器不入边」为由删减这 6 条 | 记数冲突必须在设计相位闭合：spec delta 就在本 change 目录内（`openspec/changes/init-archi-models/specs/`），不存在「射程之外」；把 58 当总数会与 spec 的字段规则（容器 `path` 唯一例外 = barrel）自相矛盾，而 101 = 43 + 58 亦随之失真。备选（撤销 barrel 例外 → 58 / 101）已记录为不采纳，见「数值口径闭合」 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 包根与域容器模型 | `App`（`package`，`docs` 指根文档）/ `App.Backend` / `App.Frontend` 三个容器；零关系 | `openspec/architecture/models/01-app.toml`（新） | 无（被 `parse_dir` 读取） | TOML（`[element."FQN"]`） |
| core 分层模型 | `App.Backend.Core` 容器 + 7 个 core crate 元素 + 9 条域内边 | `openspec/architecture/models/02-backend-core.toml`（新） | 同上 | TOML + `[[relation]]` |
| infra 分层模型 | `App.Backend.Infra` 容器 + 6 个 infra crate 元素 + 21 条边（跨域指向 core） | `openspec/architecture/models/03-backend-infra.toml`（新） | 同上 | TOML + `[[relation]]` |
| 壳模型 | `App.Backend.Shell`（`path` = `src-tauri/src/`）+ 12 条全依赖汇聚边 | `openspec/architecture/models/04-backend-shell.toml`（新） | 同上 | TOML + `[[relation]]` |
| 构建工具模型 | `App.Backend.Tools` 容器 + `ExportBindings` + 唯一反向边（→ `App.Backend.Shell`） | `openspec/architecture/models/05-backend-tools.toml`（新） | 同上 | TOML + `[[relation]]` |
| 前端模型 | 4 容器 + 24 叶（壳 / 9 视图 / 3 子域 / 10 共享组件 / `Ui`）+ 64 条 import 边 | `openspec/architecture/models/06-frontend.toml`（新） | 同上 | TOML + `[[relation]]` |
| 粒度裁定记录 | 六条裁定 + 取数口径 + 已知缺口 + 计数推导；正文逐字 FQN | `openspec/architecture/decisions/0001-model-granularity.md`（新） | 无（被 `docs.rs` 的 `text` 源读取） | Markdown |
| 模型读面（**零改动**） | `parse_dir` 字典序聚合 → `ModelGraph::build` → 四类结构诊断 → `dto` 三面投影 | `packages/desktop/src-tauri/crates/core/archi/**` | `foundation::layout::archi_*` | 既有 Rust crate |
| 图表消费面（**零改动**） | `/architecture` 从空态转正常态（元素树 + 画布 + 抽屉）；change 详情架构页签 | `packages/desktop/src/views/architecture/**`、`src/routes.tsx` | `commands.archiGraph` / `archiModule` | React 19 + xyflow |
| agent 检索面（**零改动**） | `archi_search`（元素薄条目 + 四字段文档条目）/ `archi_module`（一跳邻域 + `modelFile`·`modelLine`） | `crates/infra/agent/src/sdk/archi_tool.rs` | `archi::dto` | 既有工具面 |

依赖方向：本变更的 7 个新文件是**纯数据/文档**，只有入边（被 `archi` 的解析与文档关联读取），零出边——不新增任何 crate 依赖、不改 `Cargo.toml` / `Cargo.lock`。

---

## 变更清单 (Change Inventory)

> 路径为仓库根相对全路径。本变更**只有新增文件**；无源码改动、无生成物重导出、无配置键。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `openspec/architecture/models/01-app.toml` | 3 个元素（包根 `App`（`package`，`docs = ["./README.md", "./AGENT.md"]`）+ 2 个域容器 `App.Backend` / `App.Frontend`）；零 `[[relation]]` | 数据模型 §01；D5 / D6 |
| `openspec/architecture/models/02-backend-core.toml` | `App.Backend.Core` 容器 + 7 个 crate 元素（`Foundation` `Config` `Workflow` `Archi` `Orchestration` `Agent` `Checks`）+ 9 条关系（`Foundation` / `Agent` 出边为零） | 数据模型 §02；D1 / D3 / D5 |
| `openspec/architecture/models/03-backend-infra.toml` | `App.Backend.Infra` 容器 + 6 个 crate 元素（`AgentRuntime` `ChecksRuntime` `Exec` `Store` `VcsRuntime` `Watch`）+ 21 条关系（`Watch` 出边为零；含 `Store → Workflow` 跨域边） | 数据模型 §03；D1 / D3 / D5 |
| `openspec/architecture/models/04-backend-shell.toml` | `App.Backend.Shell`（`module`，`path = ["./packages/desktop/src-tauri/src/"]`）+ 12 条出边（唯一全依赖汇聚点） | 数据模型 §04；D1 / D3 |
| `openspec/architecture/models/05-backend-tools.toml` | `App.Backend.Tools` 容器 + `ExportBindings` 元素 + 1 条反向边 `ExportBindings → App.Backend.Shell` | 数据模型 §05；D1 / D3 |
| `openspec/architecture/models/06-frontend.toml` | 28 元素 = 4 容器（`Views` / `Components` / `Components.Agent` / `Components.Standard`）+ 24 叶（`Shell` + 9 视图 + 3 子域 + 10 共享组件 + `Ui`）+ 64 条 import 边；`App.Frontend` MUST NOT 重复声明 | 数据模型 §06；D1 / D2 / D4 / D5 |
| `openspec/architecture/decisions/0001-model-granularity.md` | 六条裁定（三档粒度 / 容器 `path` 恒空与 barrel 例外 / dev 边排除 / `components/ui` 收敛 / `gate` 全 `off` / 封闭排除面）+ 取数口径 + 已知缺口 + 计数推导；正文逐字 FQN | 数据模型「ADR 结构」；D7 |

### 修改文件

<!-- 本变更零修改文件：模型与 ADR 全部新增；`crates/core/archi/**`、`src-tauri/src/**`、`packages/desktop/src/**`、`Cargo.toml`、`openspec/config.json`、`.gitignore`、`package.json` 均零 diff（proposal「不要修改」表与 AC-9）。 -->

### 公共函数 / API

<!-- 不涉及：本变更零源码改动，不新增 / 不修改任何模块级导出函数、IPC 命令、agent 工具、前端导出函数。既有 `archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary` / `archi_set_gate` 与 `archi_search` / `archi_module` 工具契约零变化，本变更只是给它们第一份非空语料。 -->

### 类型定义

<!-- 不涉及：不新增 / 不修改任何 interface / type alias / enum / class；`bindings.ts` 零重导出（无新命令、无新 DTO 字段）。 -->

### 配置

<!-- 不涉及：不新增 / 不修改配置键。架构域 `config.json`（扫描排除键集等）显式归 archi-check-and-gate（D10）；工作区 `config.json` 的 KNOWN_KEYS 与 `config.schema.json` 镜像零变化。 -->

---

## 数据模型 (Data Model)

### 持久化形态（沿用既有契约，不改格式）

```
[element."App.Backend.Core.Archi"]      # 表头 = 带引号完整 FQN 单键
kind = "module"                         # 必填：package / domain / module / component
gate = "off"                            # 本期 49 元素一律显式 off
path = ["./packages/desktop/src-tauri/crates/core/archi/"]
summary = "架构模型的解析、图投影与检索语料"
# docs / metadata 本期不出现（见 D6）

[[relation]]
from = "App.Backend.Core.Archi"
to = "App.Backend.Core.Foundation"
desc = "模型目录与相对路径派生"
```

| 面 | 规则 |
|---|---|
| 文件序 | `01` → `06` 按文件名字典序聚合，文件内声明序保留；跨文件重复 FQN 即 `duplicateElement` |
| 身份 | 元素 = `fqn`；关系 = `(from, to)`；`path` / `docs` 逐项原文不归一 |
| 层级 | FQN 最长前缀；所有容器显式声明（不依赖隐式祖先），否则 `orphanHierarchy` |
| 关系归属 | 每条 `[[relation]]` 落在其 `from` 元素所在文件；`from` 键行即 `modelLine` |
| 校验面 | 落地即受 `cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy` 四类诊断约束（任一命中整图停摆） |
| 无库表 | 模型不落库：零新表 / 零字段 / 零迁移；画布每次现算，无缓存无倒排 |

### 关系推导规则（写模型前先按此枚举，禁止直觉补边）

**后端（43 条）**：对 14 个 crate 与壳，读各自 `Cargo.toml` 的 `[dependencies]` 段，取值为 `{ workspace = true }`（键名即 workspace 内包名）或 `{ path = … }`（指向 workspace 内目录）的条目；包名 → 元素按「裸名 CamelCase」（`agent-runtime` → `AgentRuntime`，`dev-team` → `Shell`）；方向 `依赖者 → 被依赖者`。`[dev-dependencies]`（workspace 内声明实测 4 条：`workflow → orchestration`、`orchestration → store`、`workflow → store`、`checks-runtime → workflow`）与 `[build-dependencies]` 一律不取。

**前端（64 条）**：对 source 侧文件集（见 D4）正则取静态 `import` / `export … from` 的模块说明符 → 解析为仓内路径（`@/*` → `src/*`，相对路径 `./` / `../`）→ 目录形态补 `index.tsx` / `index.ts` → 目标按「最具体已声明元素」归属（`@/components/ui/button` → `Ui`；`@/components/standard/drawer` → `Standard.Drawer`；`@/components/agent` → `Components.Agent`（barrel）；`@/views/changes/flow/*` → `Changes.Flow`；`views/changes/{其余}` → `Views.Changes`）→ 目标落在 `hooks/` / `lib/` / `types/` ⇒ 丢弃（`types/generated/bindings` 例外，映射 `App.Backend.Shell`）→ `from == to` 丢弃。

### 自洽预算（按上述内容计划落笔后必然成立的四个零）

| 诊断 | 为何为零 |
|---|---|
| `cycle` | 后端只取 `[dependencies]`（4 条 dev 边全排除，去掉 `workflow ⇄ orchestration` 与 `workflow ⇄ store` 两处环）；前端边集实测有向无环（`Shell → 视图 → 子域/组件 → Ui`，跨视图仅 `Changes → Architecture`、`Flow → Architecture`、`Explores → Renderers` 三条单向前向边） |
| `dependsOnAncestor` | 全部 `desc` 化的「包含」一律不写关系；前端唯一的父子向边是 `Views.Changes → Views.Changes.{Flow,Agent}`、`Components.Agent → Components.Agent.*`（父 import 子，方向合法），子 → 父的 import（`views/changes/flow/*` → `views/changes/hooks/*` 等）因目标在 `hooks/` 被 D4 丢弃 |
| `unknownReference` | 关系两端的 FQN 全部在上述元素表中已声明（后端 19 个元素含 5 个容器 + 包根 `App`；前端 28 个元素含 4 个容器） |
| `orphanHierarchy` | 全部 10 个容器显式声明在 `01` / `02` / `03` / `05` / `06`，每个叶元素的每一级前缀都有实体（`App` → `App.Backend` → `App.Backend.Core` → crate；`App` → `App.Frontend` → `Views` / `Components` → 叶） |

`warnings` 同时为空：`docs` 仅 `App` 两条且路径存在（否则即 `docNotFound`），元素的邻近扫描在 `crates/**` 与 `src/**` 下无 `*.md`（实测）。

### 逐文件内容计划

#### 01-app.toml — 3 元素 / 0 关系

| FQN | `kind` | `path` | `summary` |
|---|---|---|---|
| `App` | `package` | 空（`docs = ["./README.md", "./AGENT.md"]`） | 桌面应用包根：Tauri 壳 + React 前端 |
| `App.Backend` | `domain` | 空 | Rust 后端分层：领域引擎、适配器、应用壳与构建工具 |
| `App.Frontend` | `domain` | 空 | React 前端：页壳、视图与共享组件 |

#### 02-backend-core.toml — 8 元素 / 9 关系

| FQN | `kind` | `path` | `summary` |
|---|---|---|---|
| `App.Backend.Core` | `domain` | 空 | 与基础设施无关的领域与流程引擎层 |
| `App.Backend.Core.Foundation` | `module` | `./packages/desktop/src-tauri/crates/core/foundation/` | 工作区路径与目录布局派生的唯一来源 |
| `App.Backend.Core.Config` | `module` | `./packages/desktop/src-tauri/crates/core/config/` | 工作区配置的读取与校验边界 |
| `App.Backend.Core.Workflow` | `module` | `./packages/desktop/src-tauri/crates/core/workflow/` | 相位表、角色提示词与流程写面 |
| `App.Backend.Core.Archi` | `module` | `./packages/desktop/src-tauri/crates/core/archi/` | 架构模型的解析、图投影与检索语料 |
| `App.Backend.Core.Orchestration` | `module` | `./packages/desktop/src-tauri/crates/core/orchestration/` | 变更流程推进器：步骤循环、路由与归档 |
| `App.Backend.Core.Agent` | `module` | `./packages/desktop/src-tauri/crates/core/agent/` | agent 会话的领域契约与词表 |
| `App.Backend.Core.Checks` | `module` | `./packages/desktop/src-tauri/crates/core/checks/` | 测试报告语义与检查结论的领域规则 |

| 源（`from`） | 出边目标（`to`） | 条数 |
|---|---|---|
| `App.Backend.Core.Archi` | `Foundation` | 1 |
| `App.Backend.Core.Checks` | `Config`、`Foundation` | 2 |
| `App.Backend.Core.Config` | `Foundation` | 1 |
| `App.Backend.Core.Orchestration` | `Agent`、`Foundation`、`Workflow` | 3 |
| `App.Backend.Core.Workflow` | `Agent`、`Foundation` | 2 |

（`Foundation` / `Agent` 出边为零——两者是本层的依赖叶子。）

#### 03-backend-infra.toml — 7 元素 / 21 关系

| FQN | `kind` | `path` | `summary` |
|---|---|---|---|
| `App.Backend.Infra` | `domain` | 空 | 外部系统适配层：进程、本地库、版本控制、文件监视与 agent 运行时 |
| `App.Backend.Infra.AgentRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/agent/` | rig 驱动的 agent 会话运行时与工具沙箱 |
| `App.Backend.Infra.ChecksRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/checks/` | 测试命令执行与报告落盘的适配器 |
| `App.Backend.Infra.Exec` | `module` | `./packages/desktop/src-tauri/crates/infra/exec/` | 子进程执行边界：argv 直传与输出流 |
| `App.Backend.Infra.Store` | `module` | `./packages/desktop/src-tauri/crates/infra/store/` | native_db 本地库边界（按工作区分文件） |
| `App.Backend.Infra.VcsRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/vcs/` | git 操作适配：工作树、提交与 rebase |
| `App.Backend.Infra.Watch` | `module` | `./packages/desktop/src-tauri/crates/infra/watch/` | 文件系统变更监视（notify）适配器 |

| 源（`from`） | 出边目标（`to`） | 条数 |
|---|---|---|
| `App.Backend.Infra.AgentRuntime` | `Core.Agent`、`Core.Archi`、`Core.Checks`、`Core.Config`、`Core.Foundation`、`Core.Orchestration`、`Infra.Exec`、`Infra.Store` | 8 |
| `App.Backend.Infra.ChecksRuntime` | `Core.Checks`、`Core.Config`、`Core.Foundation`、`Core.Orchestration`、`Infra.Exec` | 5 |
| `App.Backend.Infra.Exec` | `Core.Foundation` | 1 |
| `App.Backend.Infra.Store` | `Core.Agent`、`Core.Foundation`、`Core.Workflow` | 3 |
| `App.Backend.Infra.VcsRuntime` | `Core.Foundation`、`Core.Orchestration`、`Core.Workflow`、`Infra.Exec` | 4 |
| `App.Backend.Infra.Watch` | —（`notify` / `serde` 均为外部 crate） | 0 |

#### 04-backend-shell.toml — 1 元素 / 12 关系

| FQN | `kind` | `path` | `summary` |
|---|---|---|---|
| `App.Backend.Shell` | `module` | `./packages/desktop/src-tauri/src/` | Tauri 壳：IPC 命令面、端口装配与绑定导出 |

| 源（`from`） | 出边目标（`to`） | 条数 |
|---|---|---|
| `App.Backend.Shell` | `Core.Agent`、`Core.Archi`、`Core.Config`、`Core.Foundation`、`Core.Orchestration`、`Core.Workflow`（6 条 core）、`Infra.AgentRuntime`、`Infra.ChecksRuntime`、`Infra.Exec`、`Infra.Store`、`Infra.VcsRuntime`、`Infra.Watch`（6 条 infra） | 12 |

（`Shell` 是唯一全依赖汇聚点；`desc` 逐条写「为哪一面装配哪个端口」，不写 `Cargo.toml` 段落位置。）

#### 05-backend-tools.toml — 2 元素 / 1 关系

| FQN | `kind` | `path` | `summary` |
|---|---|---|---|
| `App.Backend.Tools` | `domain` | 空 | 构建期工具 crate 容器 |
| `App.Backend.Tools.ExportBindings` | `module` | `./packages/desktop/src-tauri/crates/tools/export-bindings/` | 把 IPC 类型导出为前端 TypeScript 绑定 |

| 源（`from`） | 出边目标（`to`） | 条数 | `desc` 取向 |
|---|---|---|---|
| `App.Backend.Tools.ExportBindings` | `App.Backend.Shell` | 1 | 「为导出命令面 bindings 而依赖壳包」——工具 → 壳的唯一反向边，是本基线的显式产出 |

#### 06-frontend.toml — 28 元素 / 64 关系

**容器（4，`kind = "domain"`）**

| FQN | `path` | `summary` |
|---|---|---|
| `App.Frontend.Views` | 空 | 路由挂载的页面集合 |
| `App.Frontend.Components` | 空 | 跨视图复用的组件集合 |
| `App.Frontend.Components.Agent` | `./packages/desktop/src/components/agent/index.tsx`（barrel，唯一非空 `path` 的容器） | agent 会话组件的统一出口 |
| `App.Frontend.Components.Standard` | 空 | 通用交互组件集合（对话框、抽屉、分页、选择） |

**叶（24）**

| FQN | `kind` | `path` | `summary` |
|---|---|---|---|
| `App.Frontend.Shell` | `module` | 4 项：`./packages/desktop/src/app.tsx`、`.../routes.tsx`、`.../main.tsx`、`.../components/app-sidebar.tsx` | 页壳：入口、路由表与侧栏装配 |
| `App.Frontend.Views.AgentDebug` | `module` | `.../src/views/agent/` | agent 调试页：运行表单、历史与实况流 |
| `App.Frontend.Views.Agents` | `module` | `.../src/views/agents/` | agent 实例与供应商配置页 |
| `App.Frontend.Views.Architecture` | `module` | `.../src/views/architecture/` | 架构页：元素树、画布与元素抽屉 |
| `App.Frontend.Views.Changes` | `module` | `.../src/views/changes/` | 变更页：列表、详情与编排容器 |
| `App.Frontend.Views.Config` | `module` | `.../src/views/config/` | 工作区配置页（基本项、测试与写保护） |
| `App.Frontend.Views.DbInspector` | `module` | `.../src/views/db/` | 本地库检视页（表与记录浏览） |
| `App.Frontend.Views.Explores` | `module` | `.../src/views/explores/` | 探索页：列表、详情与对话编排 |
| `App.Frontend.Views.Info` | `module` | `.../src/views/info/` | 基础信息页：工作区概况与代码统计 |
| `App.Frontend.Views.Welcome` | `module` | `.../src/views/welcome-view.tsx` | 空工作区引导页 |
| `App.Frontend.Views.Changes.Flow` | `module` | `.../src/views/changes/flow/` | 变更详情流程图：节点、运行控制与产物抽屉 |
| `App.Frontend.Views.Changes.Renderers` | `module` | `.../src/views/changes/renderers/` | 产物渲染器注册表与兜底呈现 |
| `App.Frontend.Views.Changes.Agent` | `module` | `.../src/views/changes/agent/` | 变更详情右栏的 agent 对话面板 |
| `App.Frontend.Components.Agent.AgentBlocks` | `component` | `.../src/components/agent/agent-blocks.tsx` | 消息块与工具块的呈现 |
| `App.Frontend.Components.Agent.AgentError` | `component` | `.../src/components/agent/agent-error.ts` | 会话错误行的呈现 |
| `App.Frontend.Components.Agent.AgentInput` | `component` | `.../src/components/agent/agent-input.tsx` | 会话输入框与提交交互 |
| `App.Frontend.Components.Agent.AgentMessages` | `component` | `.../src/components/agent/agent-messages.tsx` | 消息列表组装与滚动 |
| `App.Frontend.Components.Agent.AgentSessionView` | `component` | `.../src/components/agent/agent-session-view.tsx` | 整段会话视图（含命令面类型驱动的状态） |
| `App.Frontend.Components.Agent.ToolCards` | `component` | `.../src/components/agent/tool-cards.tsx` | 工具调用卡片的专门呈现 |
| `App.Frontend.Components.Standard.Dailog` | `component` | `.../src/components/standard/dailog.tsx` | 模态对话框外壳 |
| `App.Frontend.Components.Standard.Drawer` | `component` | `.../src/components/standard/drawer.tsx` | 抽屉面板外壳 |
| `App.Frontend.Components.Standard.Pagination` | `component` | `.../src/components/standard/pagination.tsx` | 分页控件 |
| `App.Frontend.Components.Standard.Select` | `component` | `.../src/components/standard/select.tsx` | 下拉选择控件 |
| `App.Frontend.Components.Ui` | `module` | `.../src/components/ui/` | shadcn 原语层，恒为被依赖方 |

（`...` 均指 `./packages/desktop/src`；上表 `path` 落笔时必须写全 `./packages/desktop/src/...`。）

**关系（22 个源 / 64 条；`from == to` 与指向 `hooks`/`lib`/`types` 的 import 已按 D4 剔除）**

| 源（`from`） | 出边目标（`to`，完整 FQN；花括号为同一父路径下的并列目标，逐项各一条关系） | 条数 |
|---|---|---|
| `App.Frontend.Shell` | `App.Frontend.Components.Ui`、`App.Frontend.Views.{AgentDebug,Agents,Architecture,Changes,Config,DbInspector,Explores,Info,Welcome}` | 10 |
| `App.Frontend.Components.Agent` | `App.Frontend.Components.Agent.{AgentInput,AgentSessionView}` | 2 |
| `App.Frontend.Components.Agent.AgentBlocks` | `App.Frontend.Components.Ui` | 1 |
| `App.Frontend.Components.Agent.AgentMessages` | `App.Frontend.Components.Agent.{AgentBlocks,ToolCards}`、`App.Frontend.Components.Ui` | 3 |
| `App.Frontend.Components.Agent.AgentSessionView` | `App.Backend.Shell`、`App.Frontend.Components.Agent.{AgentError,AgentMessages}`、`App.Frontend.Components.Ui` | 4 |
| `App.Frontend.Components.Agent.ToolCards` | `App.Frontend.Components.Agent.AgentBlocks` | 1 |
| `App.Frontend.Components.Standard.{Dailog,Drawer,Pagination,Select}` | 各 1 条 → `App.Frontend.Components.Ui` | 4 |
| `App.Frontend.Views.AgentDebug` | `App.Backend.Shell`、`App.Frontend.Components.Agent`、`App.Frontend.Components.Standard.{Drawer,Pagination,Select}`、`App.Frontend.Components.Ui` | 6 |
| `App.Frontend.Views.Agents` | `App.Backend.Shell`、`App.Frontend.Components.Standard.Select`、`App.Frontend.Components.Ui` | 3 |
| `App.Frontend.Views.Architecture` | `App.Backend.Shell`、`App.Frontend.Components.Ui` | 2 |
| `App.Frontend.Views.Changes` | `App.Backend.Shell`、`App.Frontend.Components.Standard.Dailog`、`App.Frontend.Components.Ui`、`App.Frontend.Views.Architecture`、`App.Frontend.Views.Changes.{Agent,Flow}` | 6 |
| `App.Frontend.Views.Changes.Agent` | `App.Backend.Shell`、`App.Frontend.Components.Agent`、`App.Frontend.Components.Ui` | 3 |
| `App.Frontend.Views.Changes.Flow` | `App.Backend.Shell`、`App.Frontend.Components.Agent`、`App.Frontend.Components.Standard.Drawer`、`App.Frontend.Components.Ui`、`App.Frontend.Views.Architecture`、`App.Frontend.Views.Changes.Renderers` | 6 |
| `App.Frontend.Views.Changes.Renderers` | `App.Frontend.Components.Ui` | 1 |
| `App.Frontend.Views.Config` | `App.Backend.Shell`、`App.Frontend.Components.Ui` | 2 |
| `App.Frontend.Views.DbInspector` | `App.Backend.Shell`、`App.Frontend.Components.Standard.Drawer`、`App.Frontend.Components.Ui` | 3 |
| `App.Frontend.Views.Explores` | `App.Backend.Shell`、`App.Frontend.Components.Agent`、`App.Frontend.Components.Ui`、`App.Frontend.Views.Changes.Renderers` | 4 |
| `App.Frontend.Views.Info` | `App.Backend.Shell`、`App.Frontend.Components.Ui` | 2 |
| `App.Frontend.Views.Welcome` | `App.Frontend.Components.Ui` | 1 |

（出边为零的叶：`App.Frontend.Components.Agent.AgentError`、`App.Frontend.Components.Agent.AgentInput`、`App.Frontend.Components.Ui`——`Ui` 只作被依赖方，其内部互引为同元素自环已剔除。）

`desc` 写法：一条一句「为什么依赖」，按目标角色给差异化的用途（如 `Views.Changes → Views.Changes.Flow`「详情页组装流程画布」；`* → Components.Ui`「使用按钮 / 表格等原语呈现」；`* → App.Backend.Shell`「经生成的命令面类型取数与调用」；`Views.Explores → Views.Changes.Renderers`「复用 markdown 产物渲染」）。MUST NOT 把 import 文件清单写进 `desc`。

### 计数恒等式（落地后逐项可对）

```
元素 = 01(3) + 02(8) + 03(7) + 04(1) + 05(2) + 06(28)                     = 49
     = 容器 10 + 叶 39（后端 15 + 前端 24）
关系 = 01(0) + 02(9) + 03(21) + 04(12) + 05(1) + 06(64)                   = 107
     = 后端 43（Cargo `[dependencies]` 声明）+ 前端 64（import 证据）
前端 64 = 指向 Shell 的 11 + 指向 Ui 的 20 + 其余 33（含 barrel 容器端点 2 出 / 4 入）
     = 58（叶 → 叶 / 叶 → 容器）+ 6（`App.Frontend.Components.Agent` barrel 端点边）
```

上述等式与 proposal AC-1 / AC-3 / AC-5 / AC-6 及 spec 的合计段逐项同值，口径闭合见「数值口径闭合」；实现相位落笔后按 `grep -c` 逐文件核对（数值不符即判据执行错，不得改判据迎合数字）。

### ADR 结构（`decisions/0001-model-granularity.md`）

| 节 | 内容要点 |
|---|---|
| 1. 结论 | 一句话：模型取数口径 = `Cargo.toml` 声明 + `import` 证据，判据之外零元素零边 |
| 2. 三档粒度 | crate 级（14 + 壳）/ 视图级（9 + 单文件视图）/ 视图内子域（递归非测试源文件 ≥ 3，`components`·`hooks` 目录名排除）/ 共享组件文件级 / `components/ui` 收敛；各带一个「什么不进模型」的反例 |
| 3. 容器 `path` 恒空与 barrel 例外 | 包含由 FQN 前缀表达；唯一例外 `App.Frontend.Components.Agent` 持 `index.tsx`，故它可作端点（`子 → 父` 禁边反例 vs `barrel → 同目录文件` 合法） |
| 4. dev 边排除 | 实测 4 条 dev 边（`workflow --dev--> orchestration` / `orchestration --dev--> store` / `workflow --dev--> store` / `checks-runtime --dev--> workflow`），其中两处与生产边合起来即成 `cycle` ⇒ 按段落整体排除；test-only 依赖的事实由 `Cargo.toml` 承载，模型不重复表达 |
| 5. 反向边 `App.Backend.Tools.ExportBindings → App.Backend.Shell` | 工具 crate 为导出 bindings 依赖壳包；这是本基线的显式产出，不是笔误 |
| 6. `gate` 全 `off` | ratchet：新元素缺省 `off`；收紧按模块独立变更，一次一块 |
| 7. 封闭排除面 | `src/hooks` / `src/lib` / `src/types` / `components/standard/hooks` / 测试 / `test-setup.ts`；理由：无归属元素 ⇒ import 零贡献，且把 hooks 归给容器会把「子 → 父」写成依赖 |
| 8. 已知缺口与修改纪律 | 顶层 `src/hooks` 无元素 ⇒ `App.Frontend.Shell → App.Backend.Shell` 缺载体（加元素即可补齐）；新增 crate / 视图 / 子域 / 共享组件 / 跨元素 import 必须在同一 change 内补声明；偏离判据须改 capability，不得就地加元素 |

正文 MUST 逐字出现：`App.Backend.Core.Archi`、`App.Backend.Tools.ExportBindings`、`App.Backend.Shell`、`App.Frontend.Components.Agent`、`App.Frontend.Components.Ui`、`App.Frontend.Views.Changes.Flow`（`docs.rs` 以 `contains(fqn)` 关联；措辞可自然，不要求每个元素都点名）。

---

## 路由 / API 设计 (Route / API Design)

<!-- 不涉及：零新增路由、零新增 IPC 命令、零命令签名变更。本变更只让既有 `/architecture` 路由（`ArchitectureView`）从空态转正常态，并让既有工具面 `archi_search` / `archi_module` 首次有语料可返回；五条 `archi_*` IPC 的入参 / 出参 / auth 模型与 `bindings.ts` 均零变化。 -->

---

## 依赖 (Dependencies)

| 类 | 依赖 | 用途 |
|---|---|---|
| 运行时 | **无新增** | 模型由既有 `archi` crate（`toml` / `serde` / `petgraph` / `ignore`）解析；ADR 由既有 `docs.rs` 读取。`Cargo.toml` / `Cargo.lock` 零改动 |
| 构建 / 测试 | **无新增** | 无新 crate、无新 npm 包、无新脚本；核对用既有 shell 工具（目录枚举 + `grep` / `sed` 读 `Cargo.toml` 与 import 语句） |
| 核对通道（既有） | `archi_module` / `archi_search`（agent 侧，只读） | 元素总数、单元素一跳邻域、`modelFile` / `modelLine`、ADR 文档条目（`kind = adr` / `source = text`） |
| 核对通道（既有） | `/architecture` 页面（桌面端执行） | 正常态 vs 停摆态、元素树 + 画布 + 抽屉三段渲染 |
| 核对通道（既有） | 桌面端静态脚本 | 确认零代码改动（`git diff --stat` 只含 `openspec/architecture/**` 与本 change 目录）、`bindings:check` 零 diff、测试结果与基线一致 |

---

## 待决问题 (Open Questions)

> 记数类（原 O-1：107 / 64 vs 58 / 101）**不是开口项**——已在「数值口径闭合」记录三处同值与不采纳备选；本表只留真正待评审裁定的口径问题。

| # | 问题 | 首选结论 | 若否的代价 |
|---|---|---|---|
| O-2 | 版本 bump 口径（proposal R-8）：零 desktop 源码改动是否仍 bump `package.json` | 不 bump（保持 `0.6.0`） | 若裁定为 desktop 资产变更，补一行 `0.6.0 → 0.6.1`（`tauri.conf.json` 经 `"../package.json"` 自动跟随；`src-tauri/Cargo.toml` 的 crate version 不随动） |
| O-3 | 顶层 `src/hooks` 无元素 ⇒ `App.Frontend.Shell → App.Backend.Shell` 在图上缺载体（前端壳对命令面的依赖不可见） | 本期作为**已知缺口**写入 ADR 第 8 节，不补元素（补 `src/hooks` 元素属改 capability） | 若评审要求可见，需新增 `App.Frontend.Hooks` 元素并重排封闭排除面 ⇒ 改 capability + 重算前端边 |
| O-4 | `App.Frontend.Components.Agent` 同时是分组容器与 barrel 端点，是否会被误读为「容器可随意连边」 | 保持现状（spec 的字段规则即此）；ADR 第 3 节用反例讲清「禁的是子→父前缀边，不是 barrel 自持文件」 | 若评审不接受容器作端点，需改为「叶子 `Components.AgentIndex` 元素」⇒ 改 capability、元素数 +1、前端边归属重算（数值随之变化，须同步改 spec delta 与 proposal，仍在本相位可闭合范围内） |
