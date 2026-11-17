# 证据留痕 · frontend-barrel-entry-migration

> 本文件按 design D7 的形态记录「12 个目录模块入口 + 调用点收窄 + `Components.Agent` 翻转」的实测面。
> 每节标注取数面：**实现面** = 本相位（七工具 + `bash`）可达的静态事实；**移交面** = 需门禁 runner / IPC 通道的读数（由桌面端 static-check / test-execution 回填），本相位 MUST NOT 手写其读数。

## §0 口径声明

| 面 | 口径 | 取数命令 / 通道 |
|---|---|---|
| 入口在盘、barrel 正文形态、调用点条数、模型 `grep -c`、`INDEX.toml` 条数、`git status` | **元素级 / 静态面**（逐文件 `ls` / `grep` / `git`） | 实现面 |
| `entry_state` 分布（`Resolved` / `Absent` / `Invalid`） | **全量检查**（本仓分母 = `path` 非空的 44 个元素） | 移交面（门禁 runner 恒增量，`scannedFiles = 0` 时不可用） |
| 前端 `relations` 逐条 diff、`unresolved`、`scan.unmatchedFiles` | **全量检查** | 移交面 |
| `apiFindings` 分桶、`status` / `passed` | **全量检查**（`apiFindings` 与扫描模式无关） | 分桶已由 `archi_api` 的 `check` 动作实测（零写盘，与门禁同源）；`status` / `passed` 归门禁 runner |
| `api:check` 的 `missing` / `mismatched` / `orphaned`、`bindings:export` 幂等 | 既有命令 | 移交面（static-check 承接）；实现面以「`INDEX.toml` 44 条逐字不变」「`src/types/generated/**` 零 diff」作等价证据 |

**ui 调用点两组口径（不可互相换算，各记一遍）**：

| 口径 | 命令 | 改动前 | 改动后 |
|---|---|---|---|
| ① 全仓任意上下文（含 `components/ui/**` 内部与测试） | `grep -ro "@/components/ui/" --include=*.ts --include=*.tsx packages/desktop/src \| wc -l` | **133**（其中 `…/ui/lib` **1** 处 ⇒ `<mod>` 形态 **132**） | **133**（同值；`…/ui/lib` 仍 1） |
| ② 调用点口径（排除 `components/ui/**` 内部引用、排除 `…/ui/lib`） | `grep -rnE "@\/components\/ui\/[A-Za-z0-9-]+" --include=*.ts --include=*.tsx packages/desktop/src \| grep -v "^packages/desktop/src/components/ui/" \| grep -v "@/components/ui/lib" \| wc -l` | **112 处 / 55 文件 / 34 模块** | **112 处 / 55 文件 / 34 模块**（同值） |

> 本变更对 `components/ui` 的调用点**零改动**：`components/ui/index.ts` 是唯一新增文件，其首行注释刻意不含 `@/components/ui/` 字面（否则口径 ① 会 +1 而破坏「同命令前后同值」）。

---

## §1 入口面（实现面可核 + 移交面待回填）

### 1.1 12 个入口在盘（实现面）

```
packages/desktop/src/views/agent/index.ts                 1 条 export
packages/desktop/src/views/agents/index.ts                1 条 export
packages/desktop/src/views/architecture/index.ts          1 条 export
packages/desktop/src/views/config/index.ts                1 条 export
packages/desktop/src/views/db/index.ts                    1 条 export
packages/desktop/src/views/info/index.ts                  1 条 export
packages/desktop/src/views/changes/index.ts               2 条 export
packages/desktop/src/views/changes/agent/index.ts         1 条 export
packages/desktop/src/views/changes/flow/index.ts          7 条 export（8 值 + 3 类型，值/类型分语句）
packages/desktop/src/views/changes/renderers/index.ts     2 条 export
packages/desktop/src/views/explores/index.ts              3 条 export
packages/desktop/src/components/ui/index.ts              39 条 export * + 1 行外部约束注释
```

- 12 个文件全部落在各自元素的 `path` 之内（`ls -1 … | wc -l` = **12**）。
- `packages/desktop/src/components/agent/index.tsx` **零改动**（`git status --porcelain packages/desktop/src/components/agent` 空输出），`components/agent/` 不在 12 个之列。
- 12 个入口内 `grep -cE "^(const|function|let|class|interface|type) "` = **0**（入口不承载实现）。
- 12 个入口内 `grep -rnE "from '\.\./|from '\./[A-Za-z0-9._-]+/"` **零命中**（零父目录 specifier、零子目录 specifier ⇒ `./hooks` / `./lib` / 跨子目录段被同时覆盖）。
- `grep -c "^export \* from" components/ui/index.ts` = **39**、`grep -c "export default"` = **0**。

### 1.2 `entry_state` 分布（移交面 · 待 test-execution 回填）

期望值（元素级分母 44，全量检查）：`Resolved` **44** / `Absent` **0** / `Invalid` **0**。
模型面等价证据：`grep -rc '^entry' .dev-team/architecture/models/*.toml` 合计 **3**（`04` 一行、`05` 两行，`06` 零行）⇒ `App.Frontend.Components.Agent` 走 ③ 的 `<root>/index.tsx` 默认命中，12 个目录根走 ③ 的 `<root>/index.ts` 默认命中，**零显式 `entry` 加行**。

> 实际读数：**待回填**（本相位不持门禁 / IPC 通道）。

---

## §2 关系面（实现面映射 + 移交面待回填）

### 2.1 调用点收窄实测（实现面）

| 文件 | 收窄前 | 收窄后 | 判据命令 |
|---|---|---|---|
| `src/routes.tsx` | **11** | **8** | `grep -cE "^import .*from '\./views/"` |
| `views/changes/change-detail-view.tsx` | **8** | **2** | `grep -cE "from '\./(flow\|agent)'"` |
| `views/changes/flow/detail-drawer.tsx` | **1** | **1** | `grep -c "from '\.\./renderers'"` |
| `views/explores/components/explore-preview.tsx` | **1** | **1** | `grep -c "from '\.\./\.\./changes/renderers'"` |
| 合计 | **21** | **12** | — |

### 2.2 每条 import 的目标元素与收窄前逐一相等（实现面）

| 收窄后的 specifier | 收窄前的文件级 specifier（同元素） | 目标元素 |
|---|---|---|
| `./views/agent` | `./views/agent/agent-debug-view` | `App.Frontend.Views.AgentDebug` |
| `./views/agents` | `./views/agents/agents-view` | `App.Frontend.Views.Agents` |
| `./views/architecture` | `./views/architecture/architecture-view` | `App.Frontend.Views.Architecture` |
| `./views/changes`（2 名） | `./views/changes/change-detail-view` + `./views/changes/change-list-view` | `App.Frontend.Views.Changes` |
| `./views/config` | `./views/config/config-view` | `App.Frontend.Views.Config` |
| `./views/db` | `./views/db/db-inspector-view` | `App.Frontend.Views.DbInspector` |
| `./views/explores`（3 名） | `./views/explores/{explore-detail-view,explore-layout,explore-list-view}` | `App.Frontend.Views.Explores` |
| `./views/info` | `./views/info/info-view` | `App.Frontend.Views.Info` |
| `./agent` | `./agent/change-chat-panel` | `App.Frontend.Views.Changes.Agent` |
| `./flow`（8 值 + 3 类型） | `./flow/{attachments,change-flow-graph,detail-drawer,graph,run-control-panel,run-state,types}` | `App.Frontend.Views.Changes.Flow` |
| `../renderers` | `../renderers/artifact-tabs` | `App.Frontend.Views.Changes.Renderers` |
| `../../changes/renderers` | `../../changes/renderers/markdown-doc-renderer` | `App.Frontend.Views.Changes.Renderers` |

### 2.3 R-4 探测（目录 specifier 折叠，实现面）

- 探测点取 `change-detail-view.tsx` 的 `./flow`（单条 import 承载 8 值 + 3 类型），改后无类型 / 解析异常。
- 折叠由 `scan::ts::TsResolver::resolve` 单点承担：`ResolveOptions` 的 `extensions` 含 `.ts` / `.tsx`，`main_files` 取 `oxc_resolver` 缺省（`./index`）⇒ `./flow` → `views/changes/flow/index.ts`、`../renderers` → `views/changes/renderers/index.ts`。调用点 specifier 一律不带 `/index`（MUST NOT 写第二份解析）。

### 2.4 全量 `relations` diff（移交面 · 待 test-execution 回填）

期望值：前端 **71 条 / 24 个源**、`from` / `to` / `desc` 三列与改动前 diff 为空、`unresolved` 与 `scan.unmatchedFiles` 零新增；`71 = 64 + 7` 恒等式成立（7 条 `App.Frontend.Components.Agent` 端点边：3 出 `AgentInput` / `AgentSessionView` / `SessionPanel` + 4 入 `Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`）。
已知留白（**不在判红面**）：`change-detail-view.tsx:18` 与 `flow/run-control-panel.tsx:24` 两处 `…/architecture/components/architecture-canvas` 深路径 import 逐字不动（须扩 `Views.Architecture` 公共面，属独立变更），其两条模型边已在案。

> 实际读数：**待回填**。

---

## §3 判据面（判据面读数已实测 · `status` / `passed` 归移交面）

### 3.1 分桶（`archi_api check` 实测，零写盘；门禁 runner 的同源实现）

| 判据 | 改动前 | 改动后（实测） |
|---|---|---|
| `apiEntryMissing` | 12 | **0** |
| `apiMissing` | 29 | **41**（12 条由 ① 面平移 + 29 条原有） |
| 其余七条 / `apiUndeclared` | 0 | **0** |
| 合计 | 41 | **41**（**净 0、全平移**，MUST NOT 读成「债务清零」） |
| `status` / `passed` | `clean` / `true` | **待门禁 runner 回填**（期望 `clean` / `true`；12 个前端元素零契约文件 ⇒ 纳管子集为空 ⇒ 不抬档） |

逐条点名（`apiMissing` 41 条中的 12 条）：8 个视图目录根（`Views.AgentDebug` / `Views.Agents` / `Views.Architecture` / `Views.Changes` / `Views.Config` / `Views.DbInspector` / `Views.Explores` / `Views.Info`）+ 3 个视图内子域（`Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Changes.Renderers`）+ `Components.Ui`。

### 3.2 生成物零 diff（实现面）

- `grep -c '^\[\[entry\]\]' .dev-team/architecture/api/INDEX.toml` = **44**（与改动前同值）。
- `git diff --stat .dev-team/architecture/api` **零输出**；`packages/desktop/src/types/generated` **零 diff**。
- 前端零契约资产：`.dev-team/architecture/api/**` 下无新增文件、`packages/desktop/src/views/**` 与 `components/ui/**` 内 `grep -rn "\.i\.ts\|ts_import_specifier"` 仅测试夹具命中（见 §3.3 末段）⇒ `paths::ts_import_specifier` 留白逐字保留。

### 3.3 `api:check` 三列与 `bindings:export` 幂等

**实测（`archi_api` 自检工具的 `check` 动作，零写盘，与 `api:check` 同一实现）**：

- 生成物新鲜度：`mismatched` **[]** / `orphaned` **[]** / `missing` **[]**（三列皆空）；`commonDigest` = `sha256:1f8f0ec1…`。
- 契约判据摘要：`findings.total` = **41**、`byCategory` = `{ contract: 41 }`、**全部为 `apiMissing`**（`apiEntryMissing` 零条）——逐条点名 8 个视图目录根（`Views.AgentDebug` / `Views.Agents` / `Views.Architecture` / `Views.Changes` / `Views.Config` / `Views.DbInspector` / `Views.Explores` / `Views.Info`）+ 3 个视图内子域（`Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Changes.Renderers`）+ `Components.Ui`，其余 29 条为既有后端 / 共享组件 / 壳面。
- 生成物落点：`.dev-team/architecture/api/**` 零 diff、`INDEX.toml` 仍 **44** 条 `[[entry]]`（`git status --porcelain .dev-team/architecture/api` 空输出）。

`bindings:export` 幂等：**待 static-check 回填**；实现面等价证据 = `packages/desktop/src/types/generated/**` 零 diff（零 `dto` 改动）。

前端零契约资产核对：`.dev-team/architecture/api/**` 下无新增文件；`packages/desktop/src/views/**` 与 `components/ui/**` 内 `grep -rn "\.i\.ts\|ts_import_specifier"` 的 **5** 行命中全部是**既有测试夹具**（`views/architecture/components/contract-panel.test.tsx` 3 行、`element-drawer.test.tsx` 1 行、`hooks/use-archi-api-check.test.ts` 1 行的 `o1.i.ts` 一类合成路径），非产品侧资产、非生成链接线，本变更零改动 ⇒ `paths::ts_import_specifier` 留白逐字保留。

---

## §4 零扩散、版本与可复算子域计数（实现面）

### 4.1 `git status --porcelain` 白名单核对

```
 M .dev-team/architecture/decisions/0001-model-granularity.md
 M .dev-team/architecture/models/06-frontend.toml
 M packages/desktop/package.json
 M packages/desktop/src/routes.tsx
 M packages/desktop/src/views/changes/change-detail-view.tsx
 M packages/desktop/src/views/changes/flow/detail-drawer.tsx
 M packages/desktop/src/views/explores/components/explore-preview.tsx
?? openspec/changes/frontend-barrel-entry-migration/
?? packages/desktop/src/components/ui/index.ts
?? packages/desktop/src/views/agent/index.ts
?? packages/desktop/src/views/agents/index.ts
?? packages/desktop/src/views/architecture/index.ts
?? packages/desktop/src/views/changes/agent/index.ts
?? packages/desktop/src/views/changes/flow/index.ts
?? packages/desktop/src/views/changes/index.ts
?? packages/desktop/src/views/changes/renderers/index.ts
?? packages/desktop/src/views/config/index.ts
?? packages/desktop/src/views/db/index.ts
?? packages/desktop/src/views/explores/index.ts
?? packages/desktop/src/views/info/index.ts
```

⊆ AC-8 白名单（本 change 目录 + 12 个新增 `index.ts` + 4 个调用点 + `06-frontend.toml` + ADR 0001 + `package.json`）。测试文件 `module-entries.test.ts` 归测试相位。

零 diff 面（逐项命令零输出）：`.dev-team/architecture/api`、`packages/desktop/src/types/generated`、`openspec/specs`、`packages/desktop/src-tauri`、`packages/desktop/knip.json`、`packages/desktop/vite.config.ts`、`packages/desktop/tsconfig.json`；`packages/desktop/src/components/ui` 只多 `index.ts` 一个未跟踪文件（既有文件零改动）。

### 4.2 AC-7 子域判据复算（改动后重跑）

```
3  ./architecture/agent
4  ./changes/agent
16 ./changes/flow
8  ./changes/renderers
```

恰四条、`components/` 下零命中（排除条对 `views/*/` 全体生效）。三值含本变更新增的入口文件（`changes/agent` 3→4、`flow` 15→16、`renderers` 7→8），与 ADR `§2`、`repo-archi-model` delta 的计数同值。

### 4.3 模型面零溢出（实现面）

| 量 | 读数 |
|---|---|
| `06-frontend.toml` 元素 / 关系 | **30** / **71** |
| 六文件元素 / 关系 | **53** / **120** |
| `path` 非空元素 | **44** |
| `kind = "domain"` | **8**（+ `package` 1 ⇒ 容器 9；叶 44） |
| 显式 `entry` 行 | **3** |
| `api = "none"` | **2** |
| `App.Frontend.Components.Agent` | `kind = "module"`、`path = ["./packages/desktop/src/components/agent/"]`、零 `entry` 行 |

三处计数同值：`06` 表头（30 元素 / 71 关系，3 容器 + 27 叶）、`repo-archi-model` delta 计数表（53 / 120，容器 9 + 叶 44）、ADR `§9`（容器 9 + 叶 44）；`grep -rn "35 个文件\|13 个\|另有 4 个容器\|容器 10 + 叶 43" .dev-team/architecture/` 零命中。

### 4.4 版本纪律

- `packages/desktop/package.json`：`0.6.49` → **`0.6.50`**（一次、不追溯）。
- `packages/desktop/src-tauri/tauri.conf.json`：`"version": "../package.json"` 引用形态逐字不变（零内容改动）。
- `packages/desktop/src-tauri/Cargo.toml`：`[package] version = "0.1.0"` 逐字不变。
