# 任务: frontend-barrel-entry-migration

> 覆盖 proposal「变更范围 - 实现文件」**14 行 / 19 个文件全量**（原表「`views/{agent,agents,architecture,config,db,info}/index.ts`」1 行展开为 6 个文件，其余 13 行逐行 1 文件：`views/changes` 相关 4 个 barrel + `views/explores/index.ts` + `components/ui/index.ts` + `routes.tsx` / `change-detail-view.tsx` / `detail-drawer.tsx` / `explore-preview.tsx` + `06-frontend.toml` + `0001-model-granularity.md` + `package.json`）+ design「新增文件」的一份留痕（`reports/module-entry-evidence.md`）。**测试文件面归测试相位**（`module-entries.test.ts` 新增；既有测试零改动）——本相位不写用例、不跑测试与静态门禁；逐项移交清单见文末。
>
> 依赖序：**阶段零**（起点静态基线）先于任何改动——改动后两组 ui 计数与调用点条数必须有「前」值可对拍。**阶段一**（12 个 barrel 落盘）解锁阶段二（调用点必须有入口可指）。**阶段二**（调用点收窄）让 12 个入口有真实调用方，`knip` 的 unused-export 面才成立。**阶段三**（模型翻转 + ADR）与阶段一 / 二无强依赖，但必须在同一 change 内落，否则 `entry_state` 与模型各说一半。**阶段四**（留痕 + 版本 + 零扩散）收官。
>
> 四条不可动摇的机械锚点：① `grep -c "^entry" .dev-team/architecture/models/*.toml` 恒 **3**（翻转 MUST NOT 加行）、`grep -c 'api = "none"'` 恒 **2**；② 12 个 barrel 只导本元素直属文件——零 `from './hooks`、零 `from './lib`、零 `from './<子目录>/`、零跨元素 specifier；③ ui 的 **132** 处 `@/components/ui/<mod>` 调用点与 5 个零引用模块（`button-group` / `context-menu` / `input-group` / `marker` / `sheet`）**逐条不改不删**；④ `.dev-team/architecture/api/**`、`packages/desktop/src-tauri/**`、`packages/desktop/src/types/generated/**`、`openspec/specs/**`、`packages/desktop/src/components/{agent,standard,ui}/**` 的既有文件 **零 diff**（ui 只多一个 `index.ts`）。
>
> 自查 cwd：**仓库根**（`git` / `openspec` / 模型 / 留痕）与 `packages/desktop`（`package.json` / `views` / `components`）。静态门禁（`server:check` / `bindings:export` / `api:check` / `client:check`）与全量漂移读数由桌面端 static-check / test-execution 承接，本相位不重复执行机械步骤（design 事实校准 10 / D7）。

## 阶段零：起点静态基线（采集，零改动）

- [x] 记录 12 个目标目录的入口现状：`ls -1 packages/desktop/src/views/*/index.ts packages/desktop/src/views/changes/*/index.ts packages/desktop/src/components/ui/index.ts 2>&1` 应全部 `No such file`（唯一既有 barrel 是 `packages/desktop/src/components/agent/index.tsx`，本变更零改动）
- [x] 记录调用点基线（4 个数，逐条命令取数并写进留痕 §4）：`grep -cE "^import .*from '\./views/" packages/desktop/src/routes.tsx` = **11**（`^import` 共 12 行，另一行是 `react-router`）；`grep -cE "from '\./(flow|agent)/" packages/desktop/src/views/changes/change-detail-view.tsx` = **8**；`grep -c "from '\.\./renderers/" packages/desktop/src/views/changes/flow/detail-drawer.tsx` = **1**；`grep -c "from '\.\./\.\./changes/renderers/" packages/desktop/src/views/explores/components/explore-preview.tsx` = **1**
- [x] 记录 ui 调用点两组口径（design 事实校准 3）：① 全仓任意上下文 `grep -ro "@/components/ui/" --include=*.ts --include=*.tsx packages/desktop/src | wc -l` = **133**（其中 `…/ui/lib` **1** 处 ⇒ `<mod>` 形态 **132**）；② 排除 `components/ui/**` 内部引用后 `grep -rnE "@\/components\/ui\/[A-Za-z0-9-]+" --include=*.ts --include=*.tsx packages/desktop/src | grep -v "^packages/desktop/src/components/ui/" | grep -v "@/components/ui/lib" | wc -l` = **112**（涉及 **55** 文件、**34** 模块，其中测试 8 处）
- [x] 记录模型基线：`grep -c '^\[element\.' .dev-team/architecture/models/*.toml`（合计 53）、`grep -c '^\[\[relation\]\]'`（合计 120，`06` 段 71）、`grep -c '^path = '`（合计 **44**）、`grep -n 'kind = "domain"'` 合计 **9** + `kind = "package"` **1**、`grep -rn "^entry" *.toml` = **3**、`grep -rn 'api = "none"'` = **2**、`06-frontend.toml:1–2` 表头原文
- [x] 记录生成物与版本基线：`grep -c '^\[\[entry\]\]' .dev-team/architecture/api/INDEX.toml` = **44**；`grep -n '"version"' packages/desktop/package.json` = `0.6.49`；`grep -n '"version"' packages/desktop/src-tauri/tauri.conf.json` = `"../package.json"`；`grep -n '^version' packages/desktop/src-tauri/Cargo.toml` = `0.1.0`
- [x] 记录 `git status --porcelain` 的改动集（应只有本 change 目录未跟踪）作为 AC-8 零扩散基线
- [x] 在 `reports/module-entry-evidence.md` 写下 §0 口径声明：① 条目 / 入口面用**元素级**口径，`entry_state` / `relations` / `apiFindings` 用**全量检查**口径；② 两组 ui 计数（133/132 与 112/55/34）各自的口径与命令；③ 两者读数不可互相换算

## 阶段一：12 个 barrel 落盘（纯新增，零调用方）

- [x] 新增 `packages/desktop/src/views/agent/index.ts`：`export { AgentDebugView } from './agent-debug-view';`（`AgentDebugViewProps` 不外露）
- [x] 新增 `packages/desktop/src/views/agents/index.ts`：`export { AgentsView } from './agents-view';`
- [x] 新增 `packages/desktop/src/views/architecture/index.ts`：`export { ArchitectureView } from './architecture-view';`（`ArchitectureCanvas` 不在此面，见 design D6）
- [x] 新增 `packages/desktop/src/views/config/index.ts`：`export { ConfigView } from './config-view';`
- [x] 新增 `packages/desktop/src/views/db/index.ts`：`export { DbInspectorView } from './db-inspector-view';`（`DbInspectorViewProps` 不外露）
- [x] 新增 `packages/desktop/src/views/info/index.ts`：`export { InfoView } from './info-view';`
- [x] 新增 `packages/desktop/src/views/changes/index.ts`：`ChangeDetailView` ← `./change-detail-view`、`ChangeListView` ← `./change-list-view`（只导本元素直属文件，**MUST NOT** 顺手导 `./flow/*` / `./renderers/*` / `./agent/*`——跨子元素 re-export 会真加边，design R-9 / D1）
- [x] 新增 `packages/desktop/src/views/changes/agent/index.ts`：`export { ChangeChatPanel } from './change-chat-panel';`（`ChangeChatComposer` 与两个 `*Props` 不外露）
- [x] 新增 `packages/desktop/src/views/changes/renderers/index.ts`：`ArtifactTabs` ← `./artifact-tabs`、`MarkdownDocRenderer` ← `./markdown-doc-renderer`（其余 6 个导出不外露）
- [x] 新增 `packages/desktop/src/views/explores/index.ts`：`ExploreDetailView` ← `./explore-detail-view`、`ExploreLayout` ← `./explore-layout`、`ExploreListView` ← `./explore-list-view`（`ExploreDetailPane` / `ExploreRouteContext` / `useExploreRouteContext` 不外露）
- [x] 新增 `packages/desktop/src/views/changes/flow/index.ts`：值 8 名（`mountMaterials` ← `./attachments`、`ChangeFlowGraph` ← `./change-flow-graph`、`DetailDrawer` ← `./detail-drawer`、`buildFlowGraph` ← `./graph`、`RunControlPanel` ← `./run-control-panel`、`runStepNodes` / `unifiedRunEdges` / `unifiedRunNodes` ← `./run-state`）+ **`export type`** 3 名（`DrawerSelection` / `FlowGraph` / `FlowMaterials` ← `./types`）；目录 41 个具名导出只出这 11 个
- [x] 新增 `packages/desktop/src/components/ui/index.ts`：首行一条外部约束注释（本文件是目录模块的入口声明；视图内直引 `@/components/ui/<mod>` 是刻意选择，勿据此判死代码）+ **39** 条 `export * from './<mod>';`（目录内 39 个非测试源文件各一条，按模块名字典序）；**MUST NOT** 手抄「被引用的 34 个模块」白名单、MUST NOT 删除 5 个零引用模块
- [x] 自查 阶段一（入口在盘与形态）：`ls -1 packages/desktop/src/views/{agent,agents,architecture,config,db,info}/index.ts packages/desktop/src/views/changes/index.ts packages/desktop/src/views/changes/{agent,flow,renderers}/index.ts packages/desktop/src/views/explores/index.ts packages/desktop/src/components/ui/index.ts | wc -l` = **12**
- [x] 自查 阶段一（零跨元素 / 零 hooks·lib / 类型出口形态）：对 12 个 barrel `grep -rnE "from '\.\./|from '\./[A-Za-z0-9._-]+/"` **零命中**（父目录 specifier 与子目录 specifier 都不许出现；`./hooks` / `./lib` 因此被覆盖）；`grep -n "DrawerSelection" packages/desktop/src/views/changes/flow/index.ts` 必为 `export type { DrawerSelection, FlowGraph, FlowMaterials } from './types';` 形态（**零** `export { …DrawerSelection… }`）；`grep -c "^export \* from" packages/desktop/src/components/ui/index.ts` = **39**、`grep -c "export default"` = **0**；12 个 barrel 内 `grep -cE "^(const|function|let|class|interface|type) "` = **0**（入口不承载实现）

## 阶段二：调用点收窄（21 处 → 12 处）

- [x] 修改 `packages/desktop/src/views/changes/change-detail-view.tsx`：`./agent/change-chat-panel` → `./agent`；7 条 `./flow/*`（`attachments` / `change-flow-graph` / `detail-drawer` / `graph` / `run-control-panel` / `run-state` / `types`）合并为**一条** `import { … } from './flow'`（8 值 + 3 个 inline `type` 置于花括号末尾，与 `session-panel.tsx:9` 既有 inline-type 形态同族）；`:18` 的 `../architecture/components/architecture-canvas` **逐字不动**（design D6 留白）。**本条先做、单独作为目录折叠解析的探测点**（design D11 / R-4）
- [x] 修改 `packages/desktop/src/views/changes/flow/detail-drawer.tsx`：`import { ArtifactTabs } from '../renderers/artifact-tabs';` → `from '../renderers';`（具名不变）
- [x] 修改 `packages/desktop/src/views/explores/components/explore-preview.tsx`：`from '../../changes/renderers/markdown-doc-renderer';` → `from '../../changes/renderers';`
- [x] 修改 `packages/desktop/src/routes.tsx`：11 条文件级 import → 8 条目录级（`./views/{agent,agents,architecture,changes,config,db,explores,info}`；`changes` 取 `ChangeDetailView` + `ChangeListView`、`explores` 取 `ExploreDetailView` + `ExploreLayout` + `ExploreListView`）；`AppRoutes` 函数体、路由路径 / 参数 / 重定向 / 注释逐字不动
- [x] 自查 阶段二（条数）：`grep -cE "^import .*from '\./views/" packages/desktop/src/routes.tsx` = **8**；`grep -cE "from '\./flow" packages/desktop/src/views/changes/change-detail-view.tsx` = **1**、`grep -cE "from '\./agent'" …` = **1**（ui 面的独立性由下一条自查覆盖）
- [x] 自查 阶段二（12 个目录零深路径残留，design D6 的收窄口径）：以 12 个 barrel 覆盖的顶层实现文件为目标的深路径 import **零命中**（例：`grep -rnE "views/changes/flow/|'\./flow/" --include=*.ts --include=*.tsx packages/desktop/src | grep -v '\.test\.'` **零输出**——收窄后产品侧只剩 `from './flow'`；命中的 `change-detail-view.test.tsx` / `components/agent/index.test.tsx` 均在测试面）；两处 `architecture-canvas` 命中仍在（**允许**，登记为已知留白）
- [x] 自查 阶段二（ui 面零改动）：全仓 `@/components/ui/<mod>` 仍 **132**（+ `…/ui/lib` 1 = 133 行）、排除 ui 内部后仍 **112 处 / 55 文件 / 34 模块**——与阶段零同值

## 阶段三：模型翻转 + 裁定记录同步

- [x] 修改 `.dev-team/architecture/models/06-frontend.toml`：`[element."App.Frontend.Components.Agent"]` 段 `kind = "domain"` → `"module"`、`path = ["./packages/desktop/src/components/agent/index.tsx"]` → `["./packages/desktop/src/components/agent/"]`；`summary` / `gate` 逐字不动；**MUST NOT 加显式 `entry`**（③ 的 `<root>/index.tsx` 已命中，加行即违「本基线显式 `entry` 恰 3 处」）
- [x] 修改同上文件表头 `:2`：`# 元素 = 页壳 1 + 视图 9 + 视图内子域 3 + 共享组件文件 12 + 原语层 1，另有 4 个容器。` → `… + 原语层 1 + 目录模块 1，另有 3 个容器。`；`:1` 的「30 元素 / 71 关系」与 `:3` 的关系口径句逐字不动
- [x] 修改 `.dev-team/architecture/decisions/0001-model-granularity.md` `§2`：`components/ui/` 的 **35 → 39** 个文件、子域句 `flow` **13 → 16** / `renderers` **7 → 8** / `agent` **3 → 4**（含其 `hooks/`）、`views/changes/components/` **1 → 2** 文件
- [x] 修改同文件 `§3`：标题与正文重写为「容器 `path` 恒空（**零例外**）；barrel 是**目录模块**（叶）的入口，不是容器的特权」——保留 7 条端点边（3 出 `AgentInput` / `AgentSessionView` / `SessionPanel` + 4 入）与两条反例（子 → 父停摆 / 父 import 子合法），并把例外段的形态说明改为「`App.Frontend.Components.Agent` 为目录型 `module`，`path` = `./packages/desktop/src/components/agent/`，入口由 ③ 命中目录内 `index.tsx`，与 12 个目录模块入口同形」
- [x] 修改同文件 `§9`：`53（容器 10 + 叶 43）` → `53（容器 9 + 叶 44）`；末句「撤销第 3 节的 barrel 例外」→「撤销第 3 节的目录模块入口」；`§1` / `§4`–`§8` 逐字不动（`§1` 无容器 / 叶数字，proposal 的「§1 容器 10 → 9」实为 `§9` 内容）
- [x] 自查 阶段三（模型计数零溢出）：`grep -c '^\[element\.' 06-frontend.toml` = **30**、`grep -c '^\[\[relation\]\]'` = **71**、`grep -c '^path = '`（六文件合计）= **44**、`grep -rn "^entry" *.toml` = **3**、`grep -rn 'api = "none"'` = **2**、`grep -c 'kind = "domain"' 06-frontend.toml` = **3**；`grep -c 'kind = "domain"' *.toml` 合计 = **8**（+ `package` 1 ⇒ 容器 9）
- [x] 自查 阶段三（三处计数同值）：`06` 表头、`repo-archi-model` delta 的计数表、ADR `§9` 三处同值（53 / 120、`06` 30 / 71、容器 9 + 叶 44）；ADR 正文里 `35` / `13` / `7` / `3` 四处旧数只允许出现在已换值的语境（`grep -n "35 个文件\|13 个\|另有 4 个容器" .dev-team/architecture/` 零命中）

## 阶段四：证据留痕 + 版本纪律 + 零扩散收官

- [x] 填 `reports/module-entry-evidence.md` §1（入口面，实现面可核部分）：12 个 `index.ts` 在盘、各落在其元素 `path` 之内、`components/agent/index.tsx` 零改动；`entry_state` 的 **44 / 0 / 0** 期望值与「待 test-execution 回填」标注（MUST NOT 手写伪读数）
- [x] 填 §2（关系面，实现面可核部分）：21 → 12 的逐文件条数（8 / 2 / 1 / 1）+ 每条 import 的目标元素与收窄前逐一相等的映射表 + R-4 探测结果（`./flow` 一处改造后无类型 / 解析异常）；`relations` 71 条 / 24 源的**全量** diff 留「待 test-execution 回填」
- [x] 填 §3（判据面）：`apiFindings` 期望 `apiEntryMissing 0` / `apiMissing 41` / 其余七条 0 / 总 41、`status = clean`、`passed = true`（**净 0、全平移**：12 条由 ① 面搬到 ② 面），并注明口径与来源；`api:check` 三列空 + `INDEX.toml` 零 diff 的实测原文
- [x] 填 §4（零扩散与版本）：两组 ui 计数前后同值、`git status --porcelain` 白名单核对、AC-7 的 `find` 循环改动后重跑输出（期望恰四条：`3 ./architecture/agent`、`4 ./changes/agent`、`16 ./changes/flow`、`8 ./changes/renderers`，`components/` 下零命中）
- [x] 修改 `packages/desktop/package.json`：`"version"` `0.6.49` → **`0.6.50`**（只 +1 一次、不追溯）；其余键与 `scripts` 段零改动
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 仍是 `"version": "../package.json"` 引用形态（**零内容改动**，`git diff` 为空）；`packages/desktop/src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 逐字不变
- [x] 零扩散核对（AC-8 白名单）：`git status --porcelain` ⊆ {本 change 目录、12 个新增 `index.ts`、`routes.tsx`、`change-detail-view.tsx`、`detail-drawer.tsx`、`explore-preview.tsx`、`06-frontend.toml`、`decisions/0001-model-granularity.md`、`package.json`} ∪ {测试相位的 `module-entries.test.ts`}
- [x] 零 diff 核对（「不要修改」面逐项）：`git diff --stat .dev-team/architecture/api packages/desktop/src/types/generated openspec/specs packages/desktop/src-tauri packages/desktop/knip.json packages/desktop/vite.config.ts packages/desktop/tsconfig.json` **零输出**；`git diff --stat packages/desktop/src/components/ui` 只允许新增的 `index.ts` 一个文件（`A` 行），既有文件零改动
- [x] 留白终检（design D9）：`grep -rn "api = \"none\"" .dev-team/architecture/models` 仍 2 处且**未新增到前端**；`grep -rn "\.i\.ts\|ts_import_specifier" packages/desktop/src/views packages/desktop/src/components/ui` 零命中（无契约资产、无生成链接线）；`.dev-team/architecture/api/**` 下无新增文件

**移交清单（不勾为任务；承接方＝测试相位 / 桌面端 static-check · test-execution · 评估相位）**

> 实现面不产出、不勾选：缺项是「移交未完成」，不是「实现未完成」。红线——MUST NOT 为凑读数改模型 / 改 barrel 名单，MUST NOT 手写伪读数，MUST NOT 去改 ui 调用点。
>
> - **T1 `module-entries.test.ts`（测试相位，新增）**：node 环境、纯源文本读取、**零运行期 import**；断言面五条——① 12 个 barrel 的 `export … from` 行集合逐字等于 design D1 的名单（含来源 specifier）；② 每条类型出口必须是 `export type { … }`（`DrawerSelection` / `FlowGraph` / `FlowMaterials`）；③ 每个 barrel 的 specifier 零 `./hooks` / `./lib` / 跨子目录段（`from './<子目录>/`）；④ `components/ui/index.ts` 恰 **39** 条 `export * from './<mod>';`、零 `export { … } from`、零 `export default`、零实现语句；⑤ 39 个 ui 模块的具名导出零重名（`export *` 的零遮蔽前提）。这是 ui 面（在 `knip` `ignore` 内）**唯一**的机械判据，MUST NOT 因「`knip` 绿」省略。
> - **T2 静态门禁（static-check）**：`pnpm -C packages/desktop check`（`server:check` → `bindings:export` → `api:check` → `client:check`）全绿；`client:check` 含 `vp check`（typeCheck + fmt + lint）与 `knip`——零 unused export、零类型错、零格式漂移。反向判据留痕（是否实测由评估相位定）：给任一 `views/*/index.ts` 多导一名 ⇒ `knip` 报该 export unused；从 `flow/index.ts` 删 `runStepNodes` ⇒ `change-detail-view.tsx` 编译红。**反向实验 MUST NOT 留在工作树**。
> - **T3 全量漂移读数（test-execution）**：一次全量检查回填留痕 —— `entry_state` **44 Resolved / 0 Absent / 0 Invalid**（分母 = `path` 非空的 44）；前端 `relations` **71 条 / 24 源**、`from` / `to` / `desc` 三列与改动前 diff 为空、`unresolved` 与 `scan.unmatchedFiles` 零新增；`apiFindings` = `apiEntryMissing 0` + `apiMissing 41`（总 41）、其余七条 0、`status = clean`、`passed = true`；`diagnostics` / `warnings` 空。报告落盘沿槽位纪律（`full-<UTC>.json` 必须 `scan.mode = "full"` 且 `scannedFiles > 0`；`incremental-<UTC>.json` 必须 `mode = "incremental"`；无裸 `<UTC>.json`），**MUST NOT 用增量报告顶替全量读数**。
> - **T4 绑定与生成物幂等（static-check）**：`pnpm -C packages/desktop bindings:export` 二跑零 diff（`src/types/generated/**` 无改动）；`pnpm -C packages/desktop api:export` 二跑「0 份写入」（`INDEX.toml` 仍 44 条、`.dev-team/architecture/api/**` 零 diff）。
> - **T5 `vi.mock` 与 barrel 的相互作用复核（测试相位）**：`change-detail-view.test.tsx` 的 `vi.mock('./flow/attachments', { spy: true })` 与 `vi.mock('@/views/changes/flow/detail-drawer', …)` 直指**实现文件**（不经 barrel）；若因 `change-detail-view.tsx` 改走 `./flow` 导致 spy 失效，处置方向是**改测试的 mock 目标为 barrel 或保持直引不变**，MUST NOT 把产品侧 import 退回文件级（那会破 AC-4 与「入口 = 公共面」）。
> - **T6 反向判据 AC-6（评估相位）**：`archi_graph(root)` 正常态复核——元素 **53** / 关系 **120** / `06` 段 **30 / 71**、`App.Frontend.Components.Agent` 的 `kind = "module"` 且 `path` 为目录且 `entry` 零行、其 **7 条**端点边逐条在案（3 出 `AgentInput` / `AgentSessionView` / `SessionPanel`，4 入 `Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`）、`grep -c "^entry"` 仍 **3**、`grep -c 'api = "none"'` 仍 **2**。
