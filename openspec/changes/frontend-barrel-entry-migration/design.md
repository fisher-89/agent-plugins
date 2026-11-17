# 设计: frontend-barrel-entry-migration

> **变更**: frontend-barrel-entry-migration（前端目录模块入口化 · 12 个 barrel + 调用点收窄 + `Components.Agent` 容器翻转）
> **日期**: 2026-11-17
> **基于**: `proposal.md`（提案一–七 + AC-1…AC-10 + R-1…R-11 + explore 未决项归属表）、`explore.md`（B.7 为唯一有效读数段，B.0–B.6 只作推导留痕）、本 change `specs/` 下**五份已在案**的 delta（`desktop-frontend-module-entry` 1 新增能力 + `repo-archi-model` 3 MODIFIED + `desktop-archi-api` 2 MODIFIED + `desktop-archi-api-check` 4 MODIFIED + `desktop-archi-file-tree` 1 MODIFIED）
> **性质**: **前端源码结构改造 + 模型数据单点翻转**——新增 12 个 `index.ts` barrel（纯 re-export，零实现）、4 个文件 21 处 import 收窄为 12 处、`06-frontend.toml` 一个元素的 `kind` / `path` 翻转 + 表头两行改数、ADR 0001 四节同步、`package.json` 一次 patch。**零 Rust 改动、零 IPC 改动、零生成物 diff、零契约资产、零新增依赖、零测试改动**。
> **落点**: 判据面**不清零只换面**——`apiEntryMissing 12 → 0`、`apiMissing 29 → 41`（净 0、全平移），`status = clean`、`passed = true` 不动；关系面**71 条逐条不变**（barrel 只导本元素文件 ⇒ 自边丢弃）。

---

## 起点实况核实（工作树实测，`d965ba39cb34`）

| 面 | 实测 |
|---|---|
| 12 个目标目录的入口现状 | 逐个 `ls`：`views/{agent,agents,architecture,config,db,info}`、`views/changes`、`views/changes/{agent,flow,renderers}`、`views/explores`、`components/ui` **全部零 `index.ts`/`index.tsx`**；全仓唯一既有 barrel 是 `components/agent/index.tsx`（6 条语句 / 7 名，`export` 与 `export type` 分行，本变更零改动） |
| 逐目录导出面实测（顶层非测试源文件数 / 具名导出数） | `views/agent` 1/**2**、`views/agents` 1/1、`views/architecture` 1/1、`views/config` 1/1、`views/db` 1/**2**、`views/info` 1/1、`views/changes` 2/2、`views/changes/agent` 2/**4**、`views/changes/flow` 15/**41**、`views/changes/renderers` 7/**8**、`views/explores` 4/**7**、`components/ui` 39/**242** |
| 跨元素调用点（非测试） | `routes.tsx` **11** 条 → 8 个视图目录（`explores` 3、`changes` 2，其余各 1）；`views/changes/change-detail-view.tsx` **8** 条（`./flow/*` 7 + `./agent/*` 1）；`views/changes/flow/detail-drawer.tsx` **1** 条（`../renderers/artifact-tabs`）；`views/explores/components/explore-preview.tsx` **1** 条（`../../changes/renderers/markdown-doc-renderer`）⇒ 合计 **21** |
| 12 个 barrel 之外的深路径跨元素 import | 恰 **2** 处，均指向 `views/architecture/components/architecture-canvas`：`views/changes/change-detail-view.tsx:18`、`views/changes/flow/run-control-panel.tsx:24`（事实校准 2） |
| `components/ui` 调用面 | 全仓（含 `components/ui/**` 内部与测试）`@/components/ui/<mod>` **132** 处 + `@/components/ui/lib` **1** 处 = 133 行命中；排除 `components/ui/**` 内部的 13 个文件后为 **112** 处 / **55** 文件 / **34** 模块；裸 `@/components/ui`（目录）**0** 处（事实校准 3） |
| `components/ui` 导出面 | 39 个非测试源文件、46 行 `export`、**242** 个具名导出、`export default` **0**、脚本比对**零重名**（`export *` 无遮蔽风险，事实校准 6） |
| ui 零引用模块 | `button-group` / `context-menu` / `input-group` / `marker` / `sheet` 5 个（全仓零 import） |
| 模型 | **53 元素 / 120 关系**；`kind` 分布 = 9 `domain` + 1 `package` + 43 `module` + `component`（容器 10 / 叶 43）；`path` 非空元素 **44**；`06` 段 30 / 71（`domain` 4 个：`Views` / `Components` / `Components.Agent` / `Standard`）；显式 `entry` **3** 行、`api = "none"` **2** 行 |
| 子域判据分母（递归非测试源，含 `hooks/`） | `changes/flow` **15**、`changes/renderers` **7**、`changes/agent` **3**、`architecture/agent` **3**；`views/changes/components/` **2** 文件；`components/ui` **39** 文件 |
| 生成物与契约资产 | `INDEX.toml` **44** 条 `[[entry]]`，字段 = `fqn` + 六个计数 / 路径字段（`status` / `types` / `ops` / `degraded` / `schema` / `generated`），**无 `path` / `entry` 字段**；契约资产仅 `api/Infra/App.Backend.Infra.ArchiRuntime.md` + `.schema.json` 一份，前端零契约、零 `.i.ts` |
| 入口解析链 | `paths::entry_candidates` ③ = `[<root>/src/lib.rs, <root>/mod.rs, <root>/index.ts, <root>/index.tsx]`；`normalize_root` 剥 `./` 与尾 `/` ⇒ 目录型根 `./…/agent/` 的候选文本为 `…/agent/index.ts(x)` |
| 目录 specifier 的解析面 | `scan::ts::TsResolver::resolve` 走 `oxc_resolver`（`extensions` 含 `.ts` / `.tsx`，node 规则含目录 index 折叠）；tsconfig `moduleResolution = "bundler"` + `isolatedModules = true` + `@/*` 别名（无 `verbatimModuleSyntax`） |
| lint / 格式化约束 | `consistent-type-imports` (`fixStyle: inline-type-imports`)、`no-duplicate-imports`、`import/no-duplicates` 三条同时在场 ⇒ 同一模块只能有**一条** import 语句，类型名以 inline `type` 修饰（事实校准 7） |
| 静态检查面 | `knip.json`：`project` = `src/**/*.{ts,tsx}` 排测试、`ignore` = `src/types/generated/**` + **`src/components/ui/**`** + `src/**/*.i.ts`；`vite.config.ts` 的 `NO_STATIC_CHECK` 同三条 |
| 既有测试 | 测试一律**直引实现文件**（如 `change-detail-view.test.tsx` 的 `./flow/attachments` / `@/views/changes/flow/detail-drawer`），不经 barrel；`components/agent/index.test.tsx` 直引既有 barrel 形态 |
| 版本面 | `packages/desktop/package.json:3` `"version": "0.6.49"`；`tauri.conf.json` `"version": "../package.json"`（引用形态）；`src-tauri/Cargo.toml` `[package] version = "0.1.0"`（独立版本线） |
| 工作树 | 只有本 change 目录未跟踪；分支 `change/frontend-barrel-entry-migration`，HEAD `d965ba39cb34` |
| spec delta | 五份已在案（上述），实现期**只读不编辑**（事实校准 11） |

---

## 事实校准（不改判据，只定实现形态）

1. **「12 个 barrel」的导出名单必须逐条手挑，且名单已由实测固定**：`flow` 目录 15 个非测试源文件、**41 个具名导出**，barrel 只出 **11** 名（8 值 + 3 类型）；`explores` 4 文件 7 名只出 3 名（`ExploreDetailPane` / `ExploreDetailPaneProps` / `ExploreRouteContext` / `useExploreRouteContext` 4 名均只被同元素文件、测试或类型引用直取）；`changes/agent` 4 名只出 1 名（`ChangeChatComposer` 与两个 `*Props` 不外露）；6 个单文件目录各 2 或 1 名，一律只出视图组件（`*Props` 不外露）。名单见 D1，`knip` 的 unused-export 面与调用点 typeCheck 面共同把「多一名 / 少一名」判红。
2. **AC-4 的 `grep` 判据必须按「12 个 barrel 覆盖的实现文件」收窄**：全仓另有 **2 处**深路径跨元素 import 指向 `views/architecture/components/architecture-canvas`（`change-detail-view.tsx:18` 属 `Views.Changes`、`flow/run-control-panel.tsx:24` 属 `Views.Changes.Flow`）。二者打向 `views/architecture/components/`——该目录**无 barrel 计划、也不属于 12 个目录的顶层文件面**；改写它须把 `ArchitectureCanvas` 加进 `views/architecture/index.ts` 的公共面（跨 `components/` 子目录 re-export），属**扩公共面**，超出本变更射程（D6）。故 AC-4 的判红面写成「以 12 个 barrel 覆盖的顶层实现文件为目标的深路径 import = 0」，并把两处命中逐条登记为已知留白；其两条模型边（`Views.Changes → Views.Architecture`、`Views.Changes.Flow → Views.Architecture`）**已在案且不变**（`06` 段 `Views.Changes` 的 7 条出边含前者、`Flow` 的 6 条出边含后者），故不是漂移。
3. **ui 调用点计数的三套口径都已实测，AC-4 的数字须连带口径一起写死**：全仓任意上下文 `@/components/ui/<mod>` = **132**（+ `…/ui/lib` 1 处 = 133 行）；排除 `components/ui/**` 内部引用的**调用点**口径 = **112 处 / 55 文件 / 34 模块**（其中测试 8 处、非测试 104 处）。proposal 的「132 处 / 67 文件 / 35 模块」只有第一项可复算（67 与 35 在任何口径下都不闭合：文件数实测 68（含 ui 内部）/ 55（排 ui 内部），模块数实测 34（把 `…/ui/lib` 计入为 35））。本变更对 ui **零调用点改动**，故 AC-4 的 ui 面判据改写为「**同一条命令在改动前后同值**」，并把上面两组数字逐条写进留痕（D6 / D7）。
4. **容器 / 叶的切分按 `kind` 计，`path` 非空数与之无关**：现状 10 容器（9 `domain` + 1 `package`）+ 43 叶，而 `path` 非空元素恰 **44**（`Components.Agent` 以 `domain` 名分持文件型 `path`）。翻转只改该元素的 `kind` 与 `path` 形态 ⇒ 容器 10 → 9、叶 43 → 44、`path` 非空**恒 44**、元素 53 与关系 120 零变化。这条对账关系决定了 AC-1 的「分母 44」在翻转前后同值、AC-6 的「53 / 120 / 30 / 71」逐条不变。
5. **`INDEX.toml` 零 diff 是结构必然，不是「恰好没变」**：`[[entry]]` 只有 `fqn` + 六个计数 / 路径字段，**不含元素 `path` 或入口**；元素数与 `path` 非空数都不变 ⇒ 44 条逐字不变；前端零契约 ⇒ `generated` 列恒空、不产 `.i.ts` ⇒ `paths::ts_import_specifier` 仍无观测点（其「待接线留白」逐字保留）。
6. **`export *` 的零遮蔽前提是实测的**：39 个 ui 模块共 242 个具名导出、脚本比对零重名、零 `export default` ⇒ 39 条 `export *` 的导出面 = 242 名无吞并。该前提 MUST 由 `module-entries.test.ts` 的源文本断言守（ui 在 `knip` `ignore` 内，是唯一零校验目录）。
7. **`isolatedModules` + 双 lint 规则决定了两处写法**：① barrel 里类型出口必须 `export type { … } from './x'`（值形态导出类型在转译期被当值导出，运行期 `import` 未定义）；② 调用点对同一模块只能有**一条** import 语句（`no-duplicate-imports` / `import/no-duplicates`），类型名以 inline `type` 修饰、置于花括号末尾（既有同族形态：`session-panel.tsx:9` 的 `import { AgentSessionView, type AgentSessionActions } from './agent-session-view'`）。
8. **③ 命中是文本 + 读盘，翻转后自动成立**：`entry_candidates` 对目录型根给出 `<root>/index.ts` 后 `<root>/index.tsx`，按序取首个落盘者；`components/agent/` 无 `index.ts`、有 `index.tsx` ⇒ 命中第四候选。而 `entry_state` 的落位判据用 `within_path`（目录型 `path` 前缀放行）、唯一判据只看其它元素的**候选交集**（7 个子元素是文件型根，候选 = 各自文件）⇒ 无 `Duplicated`。
9. **既有测试零改动，且 mock 目标不被 barrel 改变**：测试直引实现文件（不经 barrel）；`change-detail-view.test.tsx` 的 `vi.mock('./flow/attachments', { spy: true })` 目标仍是实现文件，barrel 的 `export { … } from './attachments'` 指向同一模块 id ⇒ mock 语义不变。**边界**：`vi.mock` 与 barrel 的相互作用归测试相位复核，MUST NOT 为迎合 mock 而把产品侧 import 退回文件级。
10. **读数的取数面必须切开**（沿用 `frontend-component-element-coverage` D11 / `ts-export-closure-extraction` D9 的纪律）：**实现面可核**——`grep`/`ls` 类静态面（12 个入口在盘、barrel 正文形态、`ui` 39 条、调用点行数、模型 `grep -c`、`INDEX.toml` 条数、`git status` 白名单）；**移交面**——一次全量检查的 `entry_state` 44/0/0、前端 `relations` 逐条 diff、`apiFindings` 分桶、`api:check` 三列、`bindings:export` 幂等（取数通道是门禁 runner / IPC，实现相位不持有）。留痕 MUST 注明元素级 / 全量口径（D7）。
11. **存活 spec 不在实现面手改**：`openspec/specs/**` 由归档链按 delta 合并；本变更的 spec 落点就是 change 目录里那五份**已在案**的 delta（实现期零编辑）。
12. **版本一次到位**：落地时 `HEAD` 现值 `0.6.49` ⇒ `0.6.50`（只 +1 一次、不追溯）；`tauri.conf.json` 引用形态零内容改动；`src-tauri/Cargo.toml` 的 `0.1.0` MUST NOT 随动。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 12 个 barrel 的正文与导出名单 | 逐字按表：<br>`views/agent/index.ts` → `export { AgentDebugView } from './agent-debug-view';`<br>`views/agents/index.ts` → `export { AgentsView } from './agents-view';`<br>`views/architecture/index.ts` → `export { ArchitectureView } from './architecture-view';`<br>`views/config/index.ts` → `export { ConfigView } from './config-view';`<br>`views/db/index.ts` → `export { DbInspectorView } from './db-inspector-view';`<br>`views/info/index.ts` → `export { InfoView } from './info-view';`<br>`views/changes/index.ts` → `ChangeDetailView`（`./change-detail-view`）+ `ChangeListView`（`./change-list-view`）两条<br>`views/changes/agent/index.ts` → `export { ChangeChatPanel } from './change-chat-panel';`<br>`views/changes/renderers/index.ts` → `ArtifactTabs`（`./artifact-tabs`）+ `MarkdownDocRenderer`（`./markdown-doc-renderer`）<br>`views/changes/flow/index.ts` → 值 8 名（`mountMaterials` ← `./attachments`、`ChangeFlowGraph` ← `./change-flow-graph`、`DetailDrawer` ← `./detail-drawer`、`buildFlowGraph` ← `./graph`、`RunControlPanel` ← `./run-control-panel`、`runStepNodes` / `unifiedRunEdges` / `unifiedRunNodes` ← `./run-state`）+ `export type` 3 名（`DrawerSelection` / `FlowGraph` / `FlowMaterials` ← `./types`）<br>`views/explores/index.ts` → `ExploreDetailView`（`./explore-detail-view`）+ `ExploreLayout`（`./explore-layout`）+ `ExploreListView`（`./explore-list-view`）<br>**零实现、零注释**（ui 除外，见 D2）；语句按来源模块名字典序排列 | 名单 = 「跨元素实际被 import 的名字」的实测集合（事实校准 1）：多导一名 `knip` 判 unused、少导一名调用点编译红。MUST NOT 跨子元素（`export … from` 会被 `scan` 记 import fact ⇒ 真加边）与 MUST NOT 导 `hooks/` `lib/` |
| D2 | `components/ui/index.ts` | 首行一条外部约束注释 + **39** 条 `export * from './<mod>';`（目录内 39 个非测试源文件各一条，按模块名字典序）。MUST NOT 手维护「被引用的 34 个模块」名单 | 目录在 `knip` `ignore` + `NO_STATIC_CHECK` 内（零死代码扫描、零格式化），手抄白名单一旦某模块开始被引用就要同步、而没有任何机制提醒；`export *` 零维护且与「目录 = 模块」自洽；零重名是实测前提（事实校准 6）。注释只写**外部约束**（本文件是入口声明、视图内直引是刻意选择、勿据此判死代码），不写「本块做什么」 |
| D3 | 调用点收窄的 12 条语句形态 | `routes.tsx`：11 条 → **8** 条目录级（`./views/{agent,agents,architecture,changes,config,db,explores,info}`，`changes` 与 `explores` 各取 2 / 3 名）；`change-detail-view.tsx`：8 条 → **2** 条（`./agent` 取 `ChangeChatPanel`；`./flow` 取 8 值 + inline `type` 三类型，**单条跨行**）；`detail-drawer.tsx`：`'../renderers/artifact-tabs'` → `'../renderers'`；`explore-preview.tsx`：`'../../changes/renderers/markdown-doc-renderer'` → `'../../changes/renderers'`。合计 21 → **12** | 「barrel 必须真被用」是 `knip` 判 unused 的反面；类型与值合一条是 `no-duplicate-imports` 的硬约束（事实校准 7）。目录 specifier 一律不带 `/index`（由解析器单点折叠，MUST NOT 写第二份解析） |
| D4 | 模型翻转与表头 | `06-frontend.toml`：`App.Frontend.Components.Agent` 的 `kind` `"domain"` → `"module"`、`path` `["./packages/desktop/src/components/agent/index.tsx"]` → `["./packages/desktop/src/components/agent/"]`；`summary` / `gate` 逐字不动；**零 `entry` 行**（③ 默认已命中，加行违「本基线显式 `entry` 恰 3 处」）。表头 `:2` 改「… + 原语层 1 + **目录模块 1**，另有 **3** 个容器。」（叶 1+9+3+12+1+1 = 27）；`:1` 的「30 元素 / 71 关系」逐字不动；`:3` 关系口径句逐字不动 | 元素 / 关系逐条不动（事实校准 4）；`entry` 零加行是 `repo-archi-model`「字段纪律可机械审查」Scenario 的判红面（`grep -c "^entry"` 仍 3） |
| D5 | ADR 0001 的四节改写 | `§2`：`components/ui` 的 **35 → 39** 文件、子域计数 `flow` **13 → 16** / `renderers` **7 → 8** / `agent` **3 → 4**（含 `hooks/` 口径）、`views/changes/components/` **1 → 2** 文件；`§3` 标题与正文重写为「容器 `path` 恒空（**零例外**）；barrel 是**目录模块**（叶）的入口，不是容器的特权」——保留「7 条端点边」「父 import 子合法」「子 → 父停摆」三段事实；`§9` 的 `（容器 10 + 叶 43）` → `（容器 9 + 叶 44）`，并把末句「撤销第 3 节的 barrel 例外」改为「撤销第 3 节的目录模块入口」；`§1` 经核对**无容器 / 叶数字**（只有 53 / 120 与前端 71）⇒ 逐字不动（proposal 的「§1 容器 10 → 9」实为 §9 的内容） | 三处计数同值是既有 requirement 的硬纪律；`§3` 的「唯一例外」在新口径下解释力归零（12 处同形），且它是「容器持有 barrel」这一语义错位的唯一载体；`§9` 末句的自指必须跟着改名，否则指向一个不存在的段落 |
| D6 | AC-4 的判红面与被排除的两处深路径 | 判红面 = ① `routes.tsx` / `change-detail-view.tsx` / `detail-drawer.tsx` / `explore-preview.tsx` 的跨元素 import 条数 11 / 8 / 1 / 1 → 8 / 2 / 1 / 1（**合计 21 → 12**）；② 「以 12 个 barrel 覆盖的顶层实现文件为目标的深路径 import = 0」；③ ui 调用点计数**前后同值**（132 与 112 / 55 / 34 两组口径各记一遍）。**不纳入判红面**：`../architecture/components/architecture-canvas` 两处（事实校准 2）——登记为已知留白（改写须扩 `Views.Architecture` 公共面 + 跨 `components/` 子目录 re-export，属独立变更），其两条模型边已在案 | AC-4 的字面 grep 在本仓另有 2 处命中，若照字面判红会让实现期被迫扩公共面或退回文件级 import（两者都破坏别条纪律）。收窄口径同时**加强**了判据：它逐条锁住 12 个 barrel 的「必须被真用」，而 ui 面用「同命令前后同值」替代易漂的绝对值 |
| D7 | 证据留痕的形态与取数面 | 一份 `reports/module-entry-evidence.md`，§0 口径声明 + 四节：§1 入口面（12 个文件在盘 + 逐目录 `entry_state`；本仓期望 44 / 0 / 0）、§2 关系面（改动前后前端 `relations` 的 `from` / `to` / `desc` 三列 diff 为空、71 条 / 24 源、`unresolved` 与 `scan.unmatchedFiles` 零新增）、§3 判据面（`apiFindings` 分桶 `0 + 41 = 41`、`status = clean`、`passed = true`；`api:check` 三列空；`INDEX.toml` 零 diff）、§4 零扩散与版本（`git status` 白名单、`bindings:export` 幂等）。每节标 `实现面` / `移交面`；机器报告（若落盘）沿既有槽位纪律：`full-<UTC>.json` 必须 `scan.mode = "full"` 且 `scannedFiles > 0`，`incremental-<UTC>.json` 必须 `mode = "incremental"`，**MUST NOT 用增量报告顶替全量读数** | 事实校准 10：实现相位（七工具 + `bash`）不持有门禁 / IPC 通道；把不可达的读数写成 checkbox 等于要求实现者手写伪证据。四节同时是 AC-1 / AC-2 / AC-3 / AC-5 / AC-8 / AC-9 的载体 |
| D8 | 版本纪律 | `package.json` `0.6.49` → **`0.6.50`**（一次、不追溯）；`tauri.conf.json` 只核对引用形态（`git diff` 为空）；`Cargo.toml` 零改动 | AGENT.md：改 `packages/desktop/src/**` 源码 ⇒ desktop source change，+1 patch 无例外 |
| D9 | 明确不做 | ① 不写任何前端契约资产（`.md` / `.schema.json` / `index.i.ts`）、不接 `paths::ts_import_specifier`、不标 `api = none`；② 不改 ui 的 132 处调用点、不删 5 个零引用 ui 模块、不把 `src/components/ui/**` 移出 `knip` `ignore`；③ 不改 `packages/desktop/src/components/agent/**`（翻转只发生在模型与裁定记录）；④ 不给 `hooks/` `lib/` 建入口或元素；⑤ 不动后端元素粒度、14 个 `src/lib.rs`、三处显式 `entry`、两处 `api = none`；⑥ 不补 `repo-archi-model` §8 的两条前端缺口与 11 条边面缺口、不扩子域判据的枚举面；⑦ 不改任何测试（含 `module-entries.test.ts` —— 归测试相位）；⑧ 不改 `openspec/specs/**` 与五份 delta；⑨ 不改 `src-tauri/**`、`src/types/generated/**`、`.dev-team/architecture/api/**` | 逐条对应 proposal「明确不做」表与「不要修改」表；①⑤ 保证判据面「净 0 全平移」可一句话归因，② 保证 ui 面 diff 为零，③ 保证翻转不外溢到代码层，⑥ 保证「只改数不扩枚举」 |
| D10 | 零扩散白名单（AC-8 的可判形） | 允许改动集 = {本 change 目录、12 个新增 `index.ts`、`routes.tsx`、`change-detail-view.tsx`、`detail-drawer.tsx`、`explore-preview.tsx`、`06-frontend.toml`、`decisions/0001-model-granularity.md`、`package.json`} ∪ {测试相位的 `module-entries.test.ts`}；零 diff 面：`packages/desktop/src-tauri/**`、`packages/desktop/src/components/ui/**`（除新增 `index.ts`）、`.dev-team/architecture/api/**`、`packages/desktop/src/types/generated/**`、`openspec/specs/**`、`knip.json`、`vite.config.ts` | AC-8 的字面白名单 + 「出现 diff 即误改」的机械核对；把 12 个 barrel 之外的 `views/**` 实现文件全列进零 diff 面（39 + 40 个既有实现文件逐字不变） |
| D11 | 实施顺序与失败定位 | ① 先落 12 个 barrel（纯新增，零调用方，静态可核）；② 调用点收窄按「一条目录一次」推进，**首选 `change-detail-view.tsx` 的 `./flow` 一处**作为目录折叠解析的探测点（R-4 的最小观测面）；③ 模型翻转与 ADR 同步同一次落；④ 留痕 + 版本 + 零扩散收官。任一环节红即停在该环节：入口面红查文件名 / 扩展名，关系面红查 specifier 折叠（边数 < 71），`knip` 红查 barrel 多导一名，typeCheck 红查少导一名 | R-4（目录 specifier 不折叠会整条丢边而 `tsc` / `knip` 仍绿）只能由 AC-2 的边集 diff 见到，故把探测点前移到单条 import，失败时改动面只有一行；其余三条失败特征与 AC-3 / AC-10 的判据一一对应 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 目录模块入口 ×12（**新增**） | 每个 barrel 只做 re-export，导出面恰等于「跨元素被 import 的名字」；只导本元素直属文件，零跨子元素、零 `hooks`/`lib` | `packages/desktop/src/views/{agent,agents,architecture,config,db,info,changes,explores}/index.ts`、`views/changes/{agent,flow,renderers}/index.ts`、`components/ui/index.ts` | 同元素内的既有实现文件（零实现改动） | TypeScript 静态 re-export（`export { … } from` / `export type { … } from` / ui 的 `export * from`） |
| 跨元素调用点（**修改** 4 个文件） | 由文件级深路径改为目录 specifier，让 12 个入口有真实调用方 | `src/routes.tsx`、`views/changes/change-detail-view.tsx`、`views/changes/flow/detail-drawer.tsx`、`views/explores/components/explore-preview.tsx` | 新增的 12 个 barrel；路由表内容（路径 / 参数 / 重定向）零改动 | TS + Vite 别名（`@/*`） |
| 入口解析链（**零改动 · 被消费**） | `entry_candidates` ③ 的四候选 → `entry_state` 三态（`Resolved` / `Absent` / `Invalid`）+ 落位 / 唯一两条判据 | `src-tauri/crates/core/archi/src/api/paths.rs` | `scan::is_source_file`、`normalize_root` | 纯文本派生 + 单点读盘 |
| specifier 解析 / 目录折叠（**零改动 · 被消费**） | `./flow` / `../renderers` 一类目录 specifier 折叠到 `<dir>/index.ts(x)` | `src-tauri/crates/core/archi/src/scan/ts.rs`（`TsResolver::resolve`） | `oxc_resolver`（`extensions` 含 `.ts` / `.tsx`） | 只读缓存 |
| 归属与成边（**零改动 · 被消费**） | 最长前缀归属（7 个子元素的文件型 `path` 优先于父的目录型 `path`），`from == to` 自边丢弃 | `scan/owner.rs`、`scan/mod.rs` | `OwnerIndex` | 纯函数 |
| 取数面闭包（**零改动 · 被消费**） | 入口导出闭包沿 `export … from` 递归、`reexport_ok` 按 owner 子树放行 ⇒ `Components.Agent` 仍恰 7 名 | `src-tauri/crates/core/archi/src/api/extract/{mod,ts}.rs` | `TsResolver`、`OwnerIndex` | oxc AST |
| 前端模型数据（**修改**） | 一个元素的 `kind` / `path` 翻转 + 表头两行改数；元素 / 关系逐条不动 | `.dev-team/architecture/models/06-frontend.toml` | 解析面 `parse/toml.rs`、建图 `dto::graph_view`（零改动） | TOML |
| 粒度与口径裁定（**修改**） | `§3` 重写为「容器 `path` 零例外；barrel 是目录模块的入口」，`§2` / `§9` 计数同批换值 | `.dev-team/architecture/decisions/0001-model-granularity.md` | 被 `archi_module("App").docs` 收录 | markdown |
| 契约生成物（**零改动**） | `INDEX.toml` 仍 44 条 `[[entry]]`（无 `path` / `entry` 字段 ⇒ 翻转不可见）；三类 `.i.*` 与 `.schema.json` 零 diff | `.dev-team/architecture/api/INDEX.toml` | `archi-api-gen`（`api:export` 幂等） | TOML（生成物） |
| 证据留痕（**新增 · 分相位**） | §0 口径 + 四节实测（入口面 / 关系面 / 判据面 / 零扩散与版本） | `openspec/changes/frontend-barrel-entry-migration/reports/module-entry-evidence.md` | `grep` / `ls` / `git`（实现面）+ 门禁报告与 `archi_drift` / `archi_api`（移交面，零写盘） | markdown |
| 版本面（**修改**） | `version` 一次 patch | `packages/desktop/package.json`（`tauri.conf.json` 经引用跟随） | 既有 `"version": "../package.json"` | JSON |

### 数据流（改后，前端片段）

```
12 个 index.ts（只 re-export 本元素直属文件）
        │
        ├─ routes.tsx 8 条目录 specifier ─┐
        ├─ change-detail-view.tsx 2 条 ───┤
        ├─ detail-drawer.tsx 1 条 ────────┼─> scan::ts::TsResolver::resolve 折叠到 <dir>/index.ts
        └─ explore-preview.tsx 1 条 ──────┘        │
                                                   ▼
                              scan::OwnerIndex::owner_of(<dir>/index.ts)
                                ├─ 12 个目录元素各持自己的入口（path 之内）
                                └─ barrel 的 re-export 目标同元素 ⇒ from == to ⇒ 自边丢弃 ⇒ 前端关系 71 不变
                                                   │
                                                   ▼
                              api::paths::entry_state（③ 命中）
                                ├─ 12 个目录根：Absent → Resolved（apiEntryMissing 12 → 0）
                                └─ Components.Agent：目录型 path 命中 index.tsx（零 entry 行）

                              drift::check
                                ├─ relations 71（24 源）逐条不变
                                ├─ apiFindings = apiMissing 41（12 条由 ① 面平移 + 29 原有），其余七条 0
                                └─ 纳管子集为空（12 个前端元素零契约）⇒ status = clean、passed = true
```

> 图中 `scan::` / `drift::check` 段是**全量模式**语义（门禁 runner 恒增量 ⇒ `scannedFiles = 0`、`status = noChanges`、`observations` 退化为「全部声明未兑现」），故其读数按 D7 归移交面；`entry_state` / `apiFindings` / `INDEX.toml` 段与扫描模式无关，实现面以静态面 + 既有命令等价核对。

---

## 变更清单 (Change Inventory)

### 新增文件

| 路径 | 产出相位 | 说明 |
|---|---|---|
| `packages/desktop/src/views/agent/index.ts` | 实现面 | 1 条具名 re-export（`AgentDebugView` ← `./agent-debug-view`）；`AgentDebugViewProps` 不外露（D1） |
| `packages/desktop/src/views/agents/index.ts` | 实现面 | 1 条（`AgentsView` ← `./agents-view`） |
| `packages/desktop/src/views/architecture/index.ts` | 实现面 | 1 条（`ArchitectureView` ← `./architecture-view`）；`ArchitectureCanvas` 不在此面（D6） |
| `packages/desktop/src/views/config/index.ts` | 实现面 | 1 条（`ConfigView` ← `./config-view`） |
| `packages/desktop/src/views/db/index.ts` | 实现面 | 1 条（`DbInspectorView` ← `./db-inspector-view`）；`DbInspectorViewProps` 不外露 |
| `packages/desktop/src/views/info/index.ts` | 实现面 | 1 条（`InfoView` ← `./info-view`） |
| `packages/desktop/src/views/changes/index.ts` | 实现面 | 2 条（`ChangeDetailView` / `ChangeListView`），只导本元素直属文件（禁跨子元素，D1） |
| `packages/desktop/src/views/changes/agent/index.ts` | 实现面 | 1 条（`ChangeChatPanel`）；`ChangeChatComposer` / `ChangeChatPanelProps` / `ChangeChatComposerProps` 不外露 |
| `packages/desktop/src/views/changes/flow/index.ts` | 实现面 | 8 值 + 3 类型（`export type`），目录 41 个具名导出只出这 11 个（压力样本，D1） |
| `packages/desktop/src/views/changes/renderers/index.ts` | 实现面 | 2 条（`ArtifactTabs` / `MarkdownDocRenderer`），其余 6 个导出不外露 |
| `packages/desktop/src/views/explores/index.ts` | 实现面 | 3 条（`ExploreDetailView` / `ExploreLayout` / `ExploreListView`）；`ExploreDetailPane` / `ExploreRouteContext` / `useExploreRouteContext` 不外露 |
| `packages/desktop/src/components/ui/index.ts` | 实现面 | 首行外部约束注释 + **39** 条 `export * from './<mod>';`（D2）；目录在 `knip` / `NO_STATIC_CHECK` ignore 内，零调用方是刻意选择 |
| `openspec/changes/frontend-barrel-entry-migration/reports/module-entry-evidence.md` | 实现面 + 移交面 | §0 口径声明（元素级 / 全量；ui 计数的两组口径）+ §1 入口面 + §2 关系面 + §3 判据面 + §4 零扩散与版本（D7） |

> `packages/desktop/src/module-entries.test.ts` 属**测试文件**面（proposal「变更范围 - 测试文件」），归测试相位——本设计只钉它必须守的判据（12 个入口的 `export … from` 行集合逐字、类型出口必须 `export type`、specifier 零 `./hooks` / `./lib` / 跨子目录段、ui 恰 39 条 `export *` 且零 `export default` 且零重名、纯源文本读取零运行期 import），不设计用例（`tasks.md` 移交清单 T1）。

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src/routes.tsx` | 11 条文件级 import（`:3–13`）→ **8** 条目录级：`./views/agent` / `./views/agents` / `./views/architecture` / `./views/changes`（取 `ChangeDetailView` + `ChangeListView`）/ `./views/config` / `./views/db` / `./views/explores`（取 3 名）/ `./views/info`。`AppRoutes` 函数体、路由路径 / 参数 / 重定向 / 注释逐字不动 | D3 / AC-4；`Views.* → Shell` 之外的元素归属逐条不变（每处收窄的目标元素相同 ⇒ 边不变） |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `:19` `./agent/change-chat-panel` → `./agent`；`:20–25` 的 `./flow/*` 6 条 + `:26` 的 `import type … './flow/types'` → **1 条** `./flow`（8 值 + 3 个 inline `type`，单语句跨行）；`:18` 的 `../architecture/components/architecture-canvas` **逐字不动**（D6 留白） | 8 → 2 条（AC-4）；`Consistent-type-imports` 决定 inline `type`，`no-duplicate-imports` 决定单语句（事实校准 7） |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | `:7` `'../renderers/artifact-tabs'` → `'../renderers'`（具名不变） | 1 → 1 条（AC-4）；目标元素仍 `Views.Changes.Renderers` ⇒ 边不变 |
| `packages/desktop/src/views/explores/components/explore-preview.tsx` | `:4` `'../../changes/renderers/markdown-doc-renderer'` → `'../../changes/renderers'` | 1 → 1 条；目标元素仍 `Views.Changes.Renderers` ⇒ 边不变 |
| `.dev-team/architecture/models/06-frontend.toml` | ① `:94` `kind = "domain"` → `"module"`；② `:96` `path = ["./packages/desktop/src/components/agent/index.tsx"]` → `["./packages/desktop/src/components/agent/"]`；③ `:2` 表头「… + 原语层 1，另有 4 个容器。」→「… + 原语层 1 + 目录模块 1，另有 3 个容器。」；④ `:1` / `:3` / `summary` / `gate` 逐字不动；⑤ 其余 29 个 `[element]`、71 条 `[[relation]]` 逐字不动；⑥ **零 `entry` 新增** | D4；`grep -c '^\[element\.'` 仍 30、`^\[\[relation\]\]` 仍 71、`^entry` 仍 3、`api = "none"` 仍 2 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | `§2` 三处计数换值（表格行 35 → **39**、子域句 13 / 7 / 3 → **16 / 8 / 4**、`views/changes/components/` 1 → **2**）；`§3` 标题 + 首段 + 「唯一例外」段重写为「容器 `path` 恒空（零例外）；barrel 是目录模块的入口」，7 条端点边与两条反例（子 → 父停摆 / 父 import 子合法）逐条保留；`§9` `（容器 10 + 叶 43）` → `（容器 9 + 叶 44）`、末句「撤销第 3 节的 barrel 例外」→「撤销第 3 节的目录模块入口」；`§1` / `§4`–`§8` 逐字不动 | D5；三处计数同值（`06` 表头 / spec delta 计数表 / ADR `§9`）是既有硬纪律 |
| `packages/desktop/package.json` | `"version"` `0.6.49` → **`0.6.50`**（只 +1 一次）；`scripts` / 依赖段零改动 | D8；`tauri.conf.json` 经 `"../package.json"` 跟随 |
| `packages/desktop/src-tauri/tauri.conf.json` | **零内容改动**（只核对 `"version": "../package.json"` 引用形态未被写成字面版本号） | D8；`git diff` 必须为空 |
| `packages/desktop/src-tauri/Cargo.toml` | **零改动**（`[package] version = "0.1.0"` 独立版本线） | D8 |

### 公共函数 / API

> 本变更**零新增模块级函数**：12 个 barrel 只 re-export 既有实现，被导出的实现签名与实现文件逐字不变。下表列「经入口对外可达的**值**出口」（`export … from` 形态，签名取实现文件原样），目的是把「入口 = 公共面」逐条钉死；**45** 个未出口的具名导出（`flow` 30 个、`explores` 4 个、`renderers` 6 个、`changes/agent` 3 个、`views/agent` 与 `views/db` 的 `*Props` 各 1 个）逐条不入表。

| 标识符 | 所在文件（入口 / 实现） | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `AgentDebugView` | `views/agent/index.ts` ← `agent-debug-view.tsx:73` | 新增出口（实现零改动） | `function AgentDebugView({ root }: AgentDebugViewProps): React.JSX.Element`（`AgentDebugViewProps.root: string \| null`，**该 interface 不外露**） | 路由 `/agent` 的唯一消费名 |
| `AgentsView` | `views/agents/index.ts` ← `agents-view.tsx:12` | 新增出口 | `function AgentsView(): React.JSX.Element` | 路由 `/agents` |
| `ArchitectureView` | `views/architecture/index.ts` ← `architecture-view.tsx:10` | 新增出口 | `function ArchitectureView({ root }: { root: string \| null }): React.JSX.Element` | 路由 `/architecture`；`ArchitectureCanvas` 不在出口面（D6） |
| `ConfigView` | `views/config/index.ts` ← `config-view.tsx:91` | 新增出口 | `function ConfigView({ root }: { root: string }): React.JSX.Element` | 路由 `/config` |
| `DbInspectorView` | `views/db/index.ts` ← `db-inspector-view.tsx:285` | 新增出口 | `function DbInspectorView({ root }: DbInspectorViewProps): React.JSX.Element`（`DbInspectorViewProps.root: string`，interface 不外露） | 路由 `/db` |
| `InfoView` | `views/info/index.ts` ← `info-view.tsx:70` | 新增出口 | `function InfoView({ root }: { root: string }): React.JSX.Element` | 路由 `/info` |
| `ChangeDetailView` | `views/changes/index.ts` ← `change-detail-view.tsx:472` | 新增出口 | `function ChangeDetailView({ root }: { root: string \| null })`（无返回类型标注，返回值随 JSX 推断） | 路由 `/changes/:id` |
| `ChangeListView` | `views/changes/index.ts` ← `change-list-view.tsx:143` | 新增出口 | `function ChangeListView({ root }: { root: string \| null })` | 路由 `/changes` |
| `ChangeChatPanel` | `views/changes/agent/index.ts` ← `change-chat-panel.tsx:24` | 新增出口 | `function ChangeChatPanel(props: ChangeChatPanelProps): React.JSX.Element`（`root: string \| null` / `changeId: string \| null` 等，interface 不外露） | `Views.Changes` 详情右栏消费 |
| `mountMaterials` | `views/changes/flow/index.ts` ← `attachments.ts:41` | 新增出口 | `function mountMaterials(detail: ChangeDetail, envelopes: ArtifactEnvelope[]): FlowMaterials` | 详情页产物聚合 |
| `ChangeFlowGraph` | 同上 ← `change-flow-graph.tsx:128` | 新增出口 | `function ChangeFlowGraph(props: ChangeFlowGraphProps): React.JSX.Element`（`graph: FlowGraph` / `materials: FlowMaterials` / `onSelect: (selection: DrawerSelection) => void`，interface 不外露） | 流程画布 |
| `DetailDrawer` | 同上 ← `detail-drawer.tsx:135` | 新增出口 | `function DetailDrawer({ selection, graph, materials, root, changeId, name, sessionRefreshKey, askAnswer, onClose }: DetailDrawerProps): React.JSX.Element \| null`（interface 不外露） | 产物抽屉；`DetailDrawer` 自身是 `detail-drawer` 的消费点（`../renderers`） |
| `buildFlowGraph` | 同上 ← `graph.ts:61` | 新增出口 | `function buildFlowGraph(detail: ChangeDetail, runNodes?: RuntimeFlowNode[], runEdges?: readonly UnifiedRunEdge[]): FlowGraph` | 详情 → 图模型 |
| `RunControlPanel` | 同上 ← `run-control-panel.tsx:525` | 新增出口 | `function RunControlPanel({ changeId, name, root, activeRun, lastRun, actions }: RunControlPanelProps): React.JSX.Element`（interface 不外露） | 运行控制条 |
| `runStepNodes` | 同上 ← `run-state.ts:204` | 新增出口 | `function runStepNodes(steps: UnifiedRunStep[], phaseOrder: string[]): RuntimeFlowNode[]` | 运行节点合成 |
| `unifiedRunEdges` | 同上 ← `run-state.ts:151` | 新增出口 | `function unifiedRunEdges(runs: ChangeRunEntry[], liveRun: LiveRunFace \| null): UnifiedRunEdge[]` | 边源合成 |
| `unifiedRunNodes` | 同上 ← `run-state.ts:134` | 新增出口 | `function unifiedRunNodes(runs: ChangeRunEntry[], liveRun: LiveRunFace \| null): UnifiedRunStep[]` | 节点源合成 |
| `ArtifactTabs` | `views/changes/renderers/index.ts` ← `artifact-tabs.tsx:9` | 新增出口 | `function ArtifactTabs({ artifacts }: { artifacts: ArtifactEnvelope[] }): React.JSX.Element` | 抽屉内产物分页 |
| `MarkdownDocRenderer` | 同上 ← `markdown-doc-renderer.tsx:57` | 新增出口 | `const MarkdownDocRenderer = memo(MarkdownDocRendererBase)`（`MemoExoticComponent`，实现签名零改动） | 探索预览的文档渲染 |
| `ExploreDetailView` | `views/explores/index.ts` ← `explore-detail-view.tsx:9` | 新增出口 | `function ExploreDetailView({ root }: { root: string }): React.JSX.Element` | 路由 `/explores/:name` |
| `ExploreLayout` | 同上 ← `explore-layout.tsx:19` | 新增出口 | `function ExploreLayout({ root }: { root: string }): React.JSX.Element` | 路由 `/explores` 布局 |
| `ExploreListView` | 同上 ← `explore-list-view.tsx:222` | 新增出口 | `function ExploreListView({ root }: { root: string }): React.JSX.Element` | 路由 `/explores` 索引 |
| `components/ui/index.ts` 的 39 条 `export *` | `components/ui/index.ts` ← 39 个模块 | 新增出口面 | 39 条 `export * from './<mod>';`（目录 242 个具名导出）——**逐名不列、不手维护**（D2） | 唯一零调用方的入口：只作入口声明与契约落点 |

### 类型定义

> 本变更**零新增 interface / type alias / enum / class**。下表只列「经入口对外可达的**类型**出口」——它们必须走 `export type`（`isolatedModules`），定义与字段零改动。

| 类型名 | 所在文件（入口 / 定义） | 变化 | 说明 |
|---|---|---|---|
| `DrawerSelection` | `views/changes/flow/index.ts` ← `flow/types.ts:112`（`export type DrawerSelection = …`） | 新增类型出口（`export type { DrawerSelection, FlowGraph, FlowMaterials } from './types';`） | 详情抽屉选中面；与 `FlowGraph` / `FlowMaterials` 合一条语句、必须以 `export type` 形态（值形态导出类型在转译期被当值导出） |
| `FlowGraph` | 同上 ← `flow/types.ts:95`（`export interface FlowGraph`） | 新增类型出口 | 图模型；`ChangeFlowGraph` / `buildFlowGraph` / `DetailDrawer` 的入参面 |
| `FlowMaterials` | 同上 ← `flow/types.ts:107`（`export interface FlowMaterials`） | 新增类型出口 | 产物聚合面；`mountMaterials` 的返回类型 |
| **45** 个未出口的具名导出（`AgentDebugViewProps` / `DbInspectorViewProps` / `ChangeChatPanelProps` / `ChangeChatComposer*` / `ExploreDetailPane*` / `ExploreRouteContext` / `useExploreRouteContext` / `RendererComponent` / `resolveRenderer` / `ArtifactView` / `EvalChecklistRenderer` / `Fallback` / `TasksProgressRenderer` / `flow` 的 30 名等） | 各自实现文件 | **无变化**（MUST NOT 进入口） | 「不外露 ≠ 删」：实现与导出逐字保留，只是不构成目录元素的公共面（D1） |

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.49` → **`0.6.50`** | D8：只 +1 一次；`tauri.conf.json` 经引用跟随 |
| `version` | `packages/desktop/src-tauri/tauri.conf.json` | **零改动** | `string` | `"../package.json"` | 实现期只核对形态，`git diff` 为空 |
| `version`（crate） | `packages/desktop/src-tauri/Cargo.toml` | **零改动** | `string` | `0.1.0` | crate 版本线独立，MUST NOT 随 `package.json` |

> 配置项零新增：`.dev-team/config.json`、`.dev-team/architecture/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像）、`KNOWN_KEYS`、`capabilities/default.json`、`package.json` 的 `scripts` 段、`knip.json`、`vite.config.ts`、`tsconfig.json` 全部零 diff。模型数据（`kind` / `path` / 表头）不是配置键，见「数据模型」。

---

## 数据模型 (Data Model)

### 元素（唯一变更面，1 条，四字段）

| FQN | `kind` | `gate` | `path` | `summary` | `entry` |
|---|---|---|---|---|---|
| `App.Frontend.Components.Agent`（改前） | `domain` | `off` | `["./packages/desktop/src/components/agent/index.tsx"]` | agent 会话组件的统一出口（**不动**） | 无 |
| `App.Frontend.Components.Agent`（改后） | **`module`** | `off` | **`["./packages/desktop/src/components/agent/"]`** | 同上（不动） | **无（MUST NOT 加）** |

- 该元素是 `06-frontend.toml` 的第 93–97 行段，位置（`Components` 容器之后、`Agent.AgentBlocks` 之前）不动：同文件既有顺序是 FQN 字典序，翻转不改名，故不移动。
- 其 7 个子元素（`AgentBlocks` / `AgentError` / `AgentInput` / `AgentMessages` / `AgentSessionView` / `SessionPanel` / `ToolCards`）的文件型 `path` 逐字不动；归属由 `OwnerIndex` 最长前缀给出——7 个子元素文件优先、`index.tsx` 归父元素（`desktop-archi-file-tree` 的 `member` 档）。
- `entry_state` 由 ③ 的第四候选命中 `components/agent/index.tsx`；显式 `entry` 行数全库仍 **3**。
- 派生字段：`api` 缺省 `declared`（零契约文件 ⇒ `apiMissing` 一条）、`docs` / `metadata` 空。祖先链齐全（`App` → `App.Frontend` → `App.Frontend.Components`）⇒ `orphanHierarchy` 零命中。

### 关系（零变化）

| 面 | 改动前 | 改动后 | 判据 |
|---|---|---|---|
| 前端关系总数 / 源数 | **71** / 24 | **71** / 24 | 12 个 barrel 只导本元素直属文件 ⇒ `from == to` 自边丢弃（`scan/mod.rs` 的 `if &to == from { continue; }`） |
| 恒等式 | `71 = 64 + 7` | `71 = 64 + 7` | 7 条 `Components.Agent` 端点边：3 出（`AgentInput` / `AgentSessionView` / `SessionPanel`）+ 4 入（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`）逐条不变 |
| 21 处收窄的边面映射 | `routes.tsx` 11 条 → **8** 个目标元素（`Views.Explores` 3 名合 1、`Views.Changes` 2 名合 1，其余各 1）；`change-detail-view` 8 条 → 2 个目标元素（`Views.Changes.Flow` / `Views.Changes.Agent`）；`detail-drawer` / `explore-preview` 各 1 条 → `Views.Changes.Renderers` | 12 条 import 的**目标元素与收窄前逐条相同**（目录 specifier 折叠到 `<dir>/index.ts`，owner 与被折叠前被打到的文件同元素）⇒ 边集逐条相等 | 事实校准 4 + AC-2；反向判据：任一 barrel 若跨子元素 re-export，边数即 > 71 判红 |
| 其余 11 个目录入口的端点面 | — | **零端点边** | 它们的 re-export 目标全在本元素内（`Components.Ui` 的 39 条 `export *` 同理：目标全归 `Components.Ui` 自身） |

### 计数复述（三处同值 + `path` 面不变）

| 量 | 改动前 | 改动后 | 复算 |
|---|---|---|---|
| 全基线元素 / 关系 | 53 / 120 | **53 / 120** | `01`–`06` 逐文件不动（除 `06` 的一个 `kind` / `path`） |
| 容器 / 叶 | 10 / 43 | **9 / 44** | `kind` 口径：`domain` 9 + `package` 1 → `domain` 8 + `package` 1；叶（`module` + `component`）43 → 44 |
| `path` 非空元素 | 44 | **44** | 翻转不改「是否有 `path`」，只改形态（文件 → 目录） |
| `06` 段 | 30 / 71（4 `domain` + 26 叶） | **30 / 71（3 `domain` + 27 叶）** | 叶 = 页壳 1 + 视图 9 + 子域 3 + 共享组件 12 + 目录模块 1 + 原语层 1 = 27 |
| 显式 `entry` / `api = "none"` | 3 / 2 | **3 / 2** | 翻转零 `entry` 加行；两个工具 crate 不动 |
| 三处同值 | `06` 表头 / spec delta 计数表 / ADR `§9` | 同值（53 / 120、`06` 30 / 71、容器 9 + 叶 44） | 任一处仍写「容器 10」「4 个容器」「35 个文件」「flow 13」即漂移 |

### 生成物与契约资产

| 对象 | 变化 | 依据 |
|---|---|---|
| `.dev-team/architecture/api/INDEX.toml` | **零 diff**（44 条 `[[entry]]` 逐字不变） | 字段面只有 `fqn` + 六个计数 / 路径字段，**无 `path` / `entry`**；元素数与 `path` 非空数不变（事实校准 5） |
| `.dev-team/architecture/api/**`（`.schema.json` / `.i.rs` / `.i.ts`） | **零变化** | 前端零契约 ⇒ 不产 `.i.ts` ⇒ `paths::ts_import_specifier` 仍无实例（留白逐字保留） |
| `.dev-team/architecture/models/{01..05}-*.toml` | **零变化** | 后端 49 条、三处 `entry`、两处 `api = none` 逐字不动 |
| `packages/desktop/src/types/generated/**` | **零变化** | 零 `dto` 改动 ⇒ 绑定零再生成；出现 diff 即误动 |
| `openspec/specs/**` | **零变化** | 存活 spec 由归档链按五份 delta 合并（事实校准 11） |

### 派生读数（非持久化；落 `reports/module-entry-evidence.md`）

| 读数 | 期望值 | 判据 | 取数面（D7） |
|---|---|---|---|
| 12 个入口在盘 | 每个目录恰 1 个 `index.ts`，落在其元素 `path` 之内；`components/agent/index.tsx` 逐字不动 | AC-1 | **实现面**（`ls` / `git status`） |
| `entry_state` 分布 | `Resolved` **44** / `Absent` **0** / `Invalid` **0** | AC-1 | **移交面**（全量检查；实现面以「12 个文件在盘 + 模型零 `entry` 加行」作等价证据） |
| 前端 `relations` | 71 条 / 24 源，`from` / `to` / `desc` 三列 diff 为空；`unresolved` / `scan.unmatchedFiles` 零新增 | AC-2 | **移交面**（全量漂移报告；实现面以「12 条 import 的目标元素逐条相等」+ 两条 R-4 探测点为等价证据） |
| `apiFindings` | `apiEntryMissing` **0**、`apiMissing` **41**、其余七条 **0**、`apiUndeclared` **0**、总数 **41**；`status = clean`、`passed = true` | AC-5 | **移交面**（`apiFindings` 与扫描模式无关，但取数通道是门禁 runner / IPC） |
| `api:check` 三列 | `missing` / `mismatched` / `orphaned` 皆空；`INDEX.toml` 零 diff | AC-5 | **实现面**（`pnpm api:check`；`api:export` 二跑零写入等价） |
| `bindings:export` | 幂等（再导出零 diff） | AC-8 | **移交面**（static-check 承接；实现面以 `src/types/generated/**` 零 diff 作等价证据） |
| `knip` / typeCheck | 零 dead code、零类型错 | AC-3 | **移交面**（`pnpm client:check`）；反向判据：多导一名 ⇒ `knip` 报 unused |
| 子域判据可复算 | `3 ./architecture/agent`、`4 ./changes/agent`、`16 ./changes/flow`、`8 ./changes/renderers`（`components/` 下零命中） | AC-7 | **实现面**（AC-7 给出的 `find` 循环，改动后重跑） |
| ui 调用点 | 全仓 `@/components/ui/<mod>` **132**（+ `…/ui/lib` 1）；排除 ui 内部 13 文件后 **112 处 / 55 文件 / 34 模块**——**两组数字前后同值** | AC-4 | **实现面**（事实校准 3 的两条命令各跑两遍） |
| 报告槽位卫生 | `full-*.json` 内 `scan.mode = "full"` 且 `scannedFiles > 0`；`incremental-*.json` 内 `mode = "incremental"`；无裸 `<UTC>.json` | D7 | **实现面 + 移交面** |

---

<!-- 路由 / API 设计：本变更零 IPC 命令、零 CLI 子命令、零 HTTP 端点变化。`archi_graph` /
     `archi_module` / `archi_search` / `archi_drift` / `archi_file_tree` / `archi_api` 的入出参、
     `archi-api-gen` 的 generate / --check / bootstrap 三子命令、`api:export` / `api:check` /
     check 四段脚本全部逐字不变；`routes.tsx` 的路由路径 / 参数 / 重定向零改动（只改 import 形态）
     ⇒ 本段省略。 -->

## 依赖 (Dependencies)

运行时依赖：**零新增**。`packages/desktop/package.json` 的 `dependencies` / `devDependencies` 段、`Cargo.toml` / `Cargo.lock` 全部零 diff——barrel 只用 TypeScript 语言内建的静态 re-export。

构建 / 测试依赖：**零新增**（本变更零测试改动；`module-entries.test.ts` 走既有 `vite-plus` 测试面，零新 devDependency）。

工具面（实现与核对用，均为既有能力）：

| 能力 | 用途 |
|---|---|
| `ls` / `grep -rn` / `find` | 12 个入口在盘、barrel 正文形态（零 `hooks` / `lib` / 跨子目录段）、调用点条数、ui 调用点计数、模型 `grep -c`、子域判据复算（AC-7 的 `find` 循环） |
| `git status --porcelain` / `git diff --stat` | AC-8 白名单与「不要修改」面逐项零 diff |
| `pnpm -C packages/desktop api:check` / `api:export`（二跑） | AC-5 的三列空与幂等（生成物零 diff） |
| `pnpm -C packages/desktop check`（`server:check` → `bindings:export` → `api:check` → `client:check`） | AC-3 / AC-8 的静态门禁——**由桌面端 static-check 承接**，实现相位不重复执行 |
| `archi_drift` / `archi_graph` / `archi_api`（IPC，零写盘） | AC-1 / AC-2 / AC-5 的全量读数与留痕——**移交面**（门禁 runner 恒增量且是唯一自动落盘点） |

---

## 验收标准回应

| AC | 设计落点 | 承接阶段（tasks） |
|---|---|---|
| **AC-1** 12 个入口在案且解析面归零 | D1（12 条正文逐字）、D4（零 `entry` 加行）、事实校准 4 / 8（③ 命中路径与落位判据）、数据模型「元素」表 | 一（新建 12 个 barrel）+ 三（模型翻转）+ **移交清单**（`entry_state` 44/0/0） |
| **AC-2** 边集逐条相等 | D1（只导本元素直属文件 ⇒ 自边丢弃）、D3 / D11（一条目录一次改、`./flow` 作折叠探测点）、数据模型「关系」表的逐条映射、D6（两处 `architecture-canvas` 不在射程且其边已在案） | 二（调用点收窄）+ **移交清单**（前后 `relations` diff） |
| **AC-3** 入口 = 公共面（knip 闭合） | D1（逐条手挑 = 实测消费名集合）、D2（ui 唯一例外 = 39 条 `export *`，导出面由源文本断言守）、事实校准 6 / 7 | 一 / 二 + **移交清单**（`vp check` + `knip` 绿；反向判据多导一名即红，归测试相位 / static-check） |
| **AC-4** 调用点收窄实测 | D3（11→8、8→2、1→1、1→1 = 21→12）、D6（判红面收窄 + ui 面「同命令前后同值」+ 两组计数口径）、事实校准 2 / 3 | 二（四个文件）+ 四（留痕 §4 记两组 ui 口径） |
| **AC-5** 判据面换段而非清零 | D4 / D9①（`api` 留空 ⇒ 12 条平移到 ② 面）、数据模型「生成物」表（`INDEX.toml` 零 diff）、D7 §3 | 三（模型）+ 四（留痕 §3）+ **移交清单**（`apiFindings` 分桶） |
| **AC-6** 模型与计数换段无溢出 | D4 / D5 / 数据模型「计数复述」表（53 / 120、`06` 30 / 71、容器 9 + 叶 44、`entry` 3、`api none` 2、7 条端点边）、事实校准 4 | 三（模型 + ADR 同批） |
| **AC-7** 子域判据可复算 | 事实校准 1 的分母（16 / 8 / 4 / 3）、ADR `§2` 三处换值、AC-7 命令原文（`components/` 下零命中 = 排除条对 `views/*/` 全体生效） | 三（ADR）+ 四（留痕：改动后重跑该 `find` 循环） |
| **AC-8** 零扩散 | D10 白名单与零 diff 面、D9（明确不做九条）、事实校准 5（生成物零 diff 的结构必然） | 四（`git status` / `git diff` 逐项核对）+ **移交清单**（`bindings:export` 幂等） |
| **AC-9** 版本纪律 | D8（`0.6.50` 一次；`tauri.conf.json` 引用形态；`Cargo.toml` 零改动） | 四 |
| **AC-10** 相关能力零漂移 | 事实校准 11（五份 delta 已在案、实现期零编辑）、D9⑧⑨ | 四（`git diff openspec/specs` 零输出） |

> **不列为 AC 的流程项**（沿 proposal 末尾声明）：判据十值词汇、互斥求值序、纳管子集、`REPORT_VERSION = 3`、`check` 链四段、`entry_candidates` 四候选——各有既有 spec 落点且本变更零改动。
> **测试面**：本变更零测试改动（事实校准 9）；`module-entries.test.ts` 归测试相位（`tasks.md` 移交清单 T1）。静态门禁（`server:check` / `bindings:export` / `api:check` / `client:check`）与全量读数由桌面端 static-check / test-execution 承接，本相位不重复执行机械步骤。

---

## 待决问题 (Open Questions)

1. **`ArchitectureCanvas` 是否应进入 `views/architecture/index.ts` 的公共面**：本设计按 D6 裁定「不进」（两处深路径 import 保持原样，登记为已知留白），理由是 proposal 的 12 个 barrel 名单不含它、改写它等于扩公共面 + 跨 `components/` 子目录 re-export。若下一轮要求「所有跨元素 import 都经入口」，需一次性决定：把 `ArchitectureCanvas` 加进入口（`views/architecture/index.ts` 出第二名）+ 改写两处调用点，或把该形态写成 `desktop-frontend-module-entry` 的显式例外——两条都属改能力，MUST NOT 在本变更内顺手做。
2. **AC-4 的 ui 计数改述是否被评估相位接受**：D6 把「132 处不变」改述为「同一命令前后同值 + 两组口径并列」，因为 132 / 67 / 35 中只有 132 可复算（事实校准 3）。若评估相位坚持字面绝对值，正确处置是**改 AC 而非改代码**（去改 ui 调用点会破 D9② 与 `knip` ignore 口径）。
3. **全量读数（`entry_state` / `relations` diff / `apiFindings`）的产出时点与责任人**在本设计内不指定：承接面已确定为「持 IPC 的相位」（桌面端 static-check / test-execution / 人，D7），本设计只钉判据、文件形态与失败特征。若全量读数与期望值不符，MUST 先查证据面（是否有别的元素 / 文件进出扫描面、specifier 是否未折叠），MUST NOT 单改模型或 barrel 名去凑读数。
4. **`module-entries.test.ts` 的用例粒度**归测试相位裁定（本设计只钉它必须守的五条判据，见「新增文件」注）；若测试相位判定「源文本断言」与「`knip` 面」有重叠，MUST 保留 ui 那一份（`knip` 覆盖不到的唯一条目）。
