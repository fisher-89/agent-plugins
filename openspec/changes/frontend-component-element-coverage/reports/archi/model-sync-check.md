# 模型同步留痕 · frontend-component-element-coverage

> 留痕对象：`06-frontend.toml` 的两元素 / 七边 / 表头计数、ADR `0001-model-granularity.md` 的计数与缺口登记、`api/INDEX.toml` 的 +2 条。
> 读数口径（design D11 切分）：**实现面实测**＝与扫描模式无关、本相位（桌面只读工具 + 既有生成器）可机械取到的读数（`archi_module` / `archi_api` / `git` / `grep`）；**移交读数**＝依赖全量扫描的读数（`scan.unmatchedFiles` / `observations` / `errors` / `warnings` / `status` / `archi_file_tree.owners` / `archi_graph` 规模），本相位无该通道，见 §0 与 §5。

## 0 · 模式声明与读数归属（D11）

- **全量读数不在实现面**：门禁 runner 恒增量（`infra/archi/src/runner.rs:72` 写死 `ArchiDriftMode::Incremental`，且是 `report.rs` 唯一自动落盘点），命令面 `archi_drift` 零写盘（`commands/archi/mod.rs` 明写「其余六条 MUST NOT 写盘」，只组装不落盘）⇒ **全量槽位由移交相位取**（桌面端 static-check / test-execution 或人）。本文件只留静态面实测与移交指向。
- **本目录内的增量报告是旁证、非基线**：`reports/archi/incremental-20261116T145803082118Z.json` 是门禁 runner 自动落的增量报告（`scan.mode = "incremental"` / `scannedFiles = 0` / `changedFiles = 0` / `unmatchedFiles []` / `status = "noChanges"` / `observations` 120 条 = 全部声明未兑现）。增量模式不扫描 ⇒ 上述扫描面槽位全部退化，**MUST NOT 作任何 AC 的基线、MUST NOT 顶 `full-` 槽位**。
- **报告槽位纪律（硬）**：`full-*.json` 必须满足 `scan.mode = "full"` 且 `scannedFiles > 0`；`incremental-*.json` 必须满足 `mode = "incremental"`；本目录内**零**裸名 `<UTC>.json`（无前缀即歧义源）。改后 `ls reports/archi` = `model-sync-check.md` + `incremental-*.json`，`full-*.json` 缺失是合法态（移交未完成）。
- **实现面可读性的分界（事实校准 13）**：`apiFindings` / `INDEX.toml` / 模型计数 / 边面与 `mode` 无关（不经 `scan`）⇒ 实现面读（§2 / §3）；`status` / `passed` / `unmatchedFiles` / `observations` / `owners` 随 `mode` 变 ⇒ 移交（§1 / §5）。

## 1 · 归属与扫描面（AC-2 / AC-4）

### 1.1 归属的静态判据（本相位实测，与扫描模式无关）

两文件在盘即被既有判据覆盖，本变更让模型追上判据：`gate` 全 `off` ⇒ 元素覆盖的文件**照常进 `unmatched_files`**（`scan/mod.rs` 的 `Owner::Element { gate: Off }` 分支），只有非 `off` 元素才进 `matched`。因此本仓（`gate` 全 `off`）**不可能**出现「加元素后文件从 `unmatchedFiles` 消失」或「`matched` 里出现新元素」——AC-4 的原字面判据不可达，按 design D6 换成三条并列判据（读数移交，见 §1.2）。

| 静态判据 | 实测 |
|---|---|
| `path` 唯一最长前缀命中 | `components/agent/session-panel.tsx` 的新元素 `path` = 该文件自身（文件型根，只覆盖自身）；`components/standard/tooltip.tsx` 同形 |
| 与既有文件型根零冲突 | `components/agent/` 下除 `index.tsx`（容器 `App.Frontend.Components.Agent` 持有，文件型根只覆盖自身）外，其余元素各持**别的**单文件（`agent-blocks.tsx` / `agent-error.ts` / `agent-input.tsx` / `agent-messages.tsx` / `agent-session-view.tsx` / `tool-cards.tsx`）；`components/standard/` 下 `dailog.tsx` / `drawer.tsx` / `pagination.tsx` / `select.tsx` 各持自身、无目录型元素 ⇒ 无等长多命中 |
| 祖先链齐全 | `App` → `App.Frontend` → `App.Frontend.Components` → `Components.{Agent,Standard}` 全部在案 ⇒ `orphanHierarchy` 零命中 |
| 字段形态（不写 `entry` / `api`） | `archi_module` 读出 `kind = "component"` / `gate = "off"` / `entry = null` / `api = "declared"` / `metadata = {}` / `docs = []` / `warnings = []`；`modelFile` = `.dev-team/architecture/models/06-frontend.toml`、`modelLine` = 129 / 170 |

### 1.2 移交读数与失败特征（全量报告）

| # | 判据（全量） | 期望读数 | 失败特征 |
|---|---|---|---|
| ① | `scan.unmatchedFiles` 与加元素前的全量基线**集合相等**（不增不减） | 仍含 `components/agent/session-panel.tsx`、`components/standard/tooltip.tsx`（`off` 语义）；余项只余 `components/file/file-viewer.tsx`（ADR `§8` 已登记）与 `components/standard/hooks/use-open.ts`（`§7` 合法排除） | 任一文件新增进该槽位 ⇒ 有别的元素 / 边进出扫描面，先查证据面 |
| ② | `observations` 相对基线**恰 +1 条**，新增项逐字为 `App.Frontend.Components.Agent.SessionPanel → App.Backend.Shell`（`unfulfilledDeclaration`） | 指向 `App.Backend.Shell` 的前端声明边 **11 → 12** | **+3 条** ⇒ `SessionPanel` 的三条声明（① ② ③）全部未兑现 ⇒ 归属失败（文件仍在 `Owner::None` ⇒ 不在 `owned` 循环内，一条依赖都没抽出来） |
| ③ | `errors` / `warnings` 前后皆空、`scan.scannedFiles` 前后同值、`status = clean`、门禁 `passed = true` | 纳管子集仍只有 `ArchiRuntime` 且判据 0 条 | 任一非空 ⇒ 停手排查，MUST NOT 改模型去迁就 |
| ④ | `archi_file_tree` 两处条目 | `…/components/agent/session-panel.tsx` → `mark = "elementRoot"`、`owners = ["App.Frontend.Components.Agent.SessionPanel"]`；`…/components/standard/tooltip.tsx` → `mark = "elementRoot"`、`owners = ["App.Frontend.Components.Standard.Tooltip"]` | 任一落 `mark = "ambiguous"` 或 `owners.len() ≥ 2` 即不合格（同时会让 ③ 的 `warnings` 非空） |

`owners` 是 `OwnerIndex::owner_of` 的同源投影（归属单源），故 ④ 与 1.1 的静态判据互为交叉校验；② 的 1 与 3 之差是「归属成立」的机械证据。

## 2 · `apiFindings` 分桶（AC-3，实现面实测：`archi_api check` / 报告槽位）

**口径**：契约判据只吃模型与契约盘面、不经 `scan` ⇒ 与 `mode` 无关，下面的读数在增量报告里即可读出（本目录 `incremental-20261116T145803082118Z.json` 与 `archi_api check` 同值）。

| 读数 | 值 |
|---|---|
| `findings.total` | **41** |
| `byCategory` | `contract` **29** / `entry` **12** |
| 其余七条一致性判据（`apiSignatureMismatch` / `apiUnexpected` / `apiShadowing` / `apiRefMissing` / `apiNestedCrossCrate` / `apiReexportDrift` / `apiConstructorDeclared`） | 各 **0** |
| `apiUndeclared`（完备性判据，档位 `off`） | **0**（档位 `off`，不产出条目） |
| 分母（`path` 非空元素） | **44**（= `INDEX.toml` 条目数，两处交叉校验） |
| `artifacts`（`mismatched` / `orphaned` / `missing`） | 三列**皆空** |

### 2.1 `apiEntryMissing` 12（`category = entry`，逐条点名）

`App.Frontend.Components.Ui`、`App.Frontend.Views.AgentDebug`、`App.Frontend.Views.Agents`、`App.Frontend.Views.Architecture`、`App.Frontend.Views.Changes`、`App.Frontend.Views.Changes.Agent`、`App.Frontend.Views.Changes.Flow`、`App.Frontend.Views.Changes.Renderers`、`App.Frontend.Views.Config`、`App.Frontend.Views.DbInspector`、`App.Frontend.Views.Explores`、`App.Frontend.Views.Info`。

= **8 个视图目录根**（`AgentDebug` / `Agents` / `Architecture` / `Changes` / `Config` / `DbInspector` / `Explores` / `Info`）+ **3 个视图内子域**（`Changes.Agent` / `Changes.Flow` / `Changes.Renderers`）+ **`App.Frontend.Components.Ui`**。与变更前同值（12 → 12）：两个新元素是单文件根，由约定默认解析出入口。

### 2.2 `apiMissing` 29（`category = contract`，逐条点名）

- 13 个 crate：`App.Backend.Core.Agent`、`App.Backend.Core.Archi`、`App.Backend.Core.Checks`、`App.Backend.Core.Config`、`App.Backend.Core.Foundation`、`App.Backend.Core.Orchestration`、`App.Backend.Core.Workflow`、`App.Backend.Infra.AgentRuntime`、`App.Backend.Infra.ChecksRuntime`、`App.Backend.Infra.Exec`、`App.Backend.Infra.Store`、`App.Backend.Infra.VcsRuntime`、`App.Backend.Infra.Watch`
- `App.Backend.Shell`
- 14 个前端单文件根：`App.Frontend.Components.Agent`（barrel）、`App.Frontend.Components.Agent.AgentBlocks`、`…Agent.AgentError`、`…Agent.AgentInput`、`…Agent.AgentMessages`、`…Agent.AgentSessionView`、**`App.Frontend.Components.Agent.SessionPanel`**、`…Agent.ToolCards`、`App.Frontend.Components.Standard.Dailog`、`…Standard.Drawer`、`…Standard.Pagination`、`…Standard.Select`、**`App.Frontend.Components.Standard.Tooltip`**、`App.Frontend.Views.Welcome`
- `App.Frontend.Shell`

**+2 的成因（一句话）**：两个新元素有入口（单文件根由约定默认命中，`entry_state = Resolved`）、无契约文件（`api` 取缺省 `declared`）⇒ 两条 `apiMissing`；桶位移只落在 ②，① 不动。故 `12 / 27（总 39）` → **`12 / 29（总 41）`**，分母 `42 → 44`，`Resolved` 入口元素 `30 → 32`。

### 2.3 生成物与幂等（实现面实测）

- `pnpm run api:export`：首跑输出「1 份生成物已写入（2 份未变）」——写入的正是 `api/INDEX.toml`；`*.schema.json` / `*.i.rs` / `*.i.ts` 三类零 diff（两元素 `present = false`，不产 `.i.*`、不产 schema）。
- 二跑输出「**0 份写入（3 份未变）**」，`git status --porcelain .dev-team/architecture/api` 仍只有 `M INDEX.toml` 一条 ⇒ 幂等，等价于 `api:check` 三列皆空（`archi_api check` 的 `artifacts` 三列亦为空）。
- `INDEX.toml`：`[[entry]]` **42 → 44**；新增 `App.Frontend.Components.Agent.SessionPanel`（位在 `…Agent.AgentSessionView` 与 `…Agent.ToolCards` 之间）与 `App.Frontend.Components.Standard.Tooltip`（位在 `…Standard.Select` 与 `…Components.Ui` 之间），两条六字段逐字为 `status = "absent"` / `types = 0` / `ops = 0` / `degraded = 0` / `schema = ""` / `generated = ""`；表头（含 `映射规则摘要 sha256:…`）逐字未变。

## 3 · 七条边逐条证据（AC-5）

### 3.1 六条值依赖边的 `import` 证据

| # | `from` → `to` | 证据（文件:行） | 形态 |
|---|---|---|---|
| ① | `Components.Agent.SessionPanel` → `…Agent.AgentSessionView` | `packages/desktop/src/components/agent/session-panel.tsx:9` `import { AgentSessionView, type AgentSessionActions } from './agent-session-view'` | 叶 → 叶 |
| ② | `…SessionPanel` → `Components.Ui` | `session-panel.tsx:4` `@/components/ui/button` + `:5` `@/components/ui/resizable`（按目标元素去重 = 1 条） | 叶 → 叶 |
| ④ | `Components.Agent`（barrel）→ `…SessionPanel` | `components/agent/index.tsx:5` `export { SessionPanel } from './session-panel'` + `:6` `export type { SessionPanelState } from './session-panel'`（去重 = 1 条） | barrel 端点边 |
| ⑤ | `Standard.Tooltip` → `Components.Ui` | `components/standard/tooltip.tsx:1` `import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'` | 叶 → 叶 |
| ⑥ | `Views.Changes` → `Standard.Tooltip` | `views/changes/change-list-view.tsx:5` `import { TextTooltip } from '@/components/standard/tooltip'` | 叶 → 叶 |
| ⑦ | `Views.Explores` → `Standard.Tooltip` | `views/explores/explore-list-view.tsx:5` `import { TextTooltip } from '@/components/standard/tooltip'` | 叶 → 叶 |

### 3.2 第三条边是**声明式命令面边**

| # | `from` → `to` | 证据 | 口径 |
|---|---|---|---|
| ③ | `Components.Agent.SessionPanel` → `App.Backend.Shell` | `session-panel.tsx:7` `import type { SessionSummary } from '@/types/generated/bindings'`（`type_only` 在扫描面只标注、不豁免） | 目标 `src/types/**` 属封闭排除面（`Owner::None`）⇒ 扫描侧丢边 ⇒ 该边在模型内在案、在报告里落 `observations[].unfulfilledDeclaration`；`gate` 全 `off` ⇒ 永不进 `errors` / `warnings`。**MUST NOT 用「观测项为空」验收前端关系面** |

新增边的目标集合与两文件的实际 `import` 目标集合逐条相等：`session-panel.tsx:6` 的 `@/hooks/use-agent-session` 属封闭排除面（零边），`tooltip.tsx` 只有一处 import。

### 3.3 模型内指向 `App.Backend.Shell` 的前端边 12 条（静态面）

`grep -c 'to = "App.Backend.Shell"' .dev-team/architecture/models/06-frontend.toml` = **12**，逐条为：`Components.Agent.AgentSessionView`、**`Components.Agent.SessionPanel`（本次新增 ③）**、`Views.AgentDebug`、`Views.Agents`、`Views.Architecture`、`Views.Changes`、`Views.Changes.Agent`、`Views.Changes.Flow`、`Views.Config`、`Views.DbInspector`、`Views.Explores`、`Views.Info`（前缀 `App.Frontend.`；`App.Backend.Tools.ExportBindings` 那条是后端边，不计入）。全量 `observations` 里应有 12 条同名 `unfulfilledDeclaration`，与增量报告的 12 条计数一致（增量下的 120 条是「全部声明未兑现」，不作基线）。

### 3.4 图的邻接与规模（实现面实测：`archi_module`）

| 元素 | `dependsOn`（出边） | `dependedBy`（入边） |
|---|---|---|
| `App.Frontend.Components.Agent.SessionPanel` | `App.Backend.Shell`（经生成的命令面类型取会话清单）、`…Agent.AgentSessionView`（面板正文复用整段会话视图）、`…Components.Ui`（头区按钮与可拖拽分栏使用原语层） | `App.Frontend.Components.Agent`（barrel 重导出面板组件与其状态类型） |
| `App.Frontend.Components.Standard.Tooltip` | `App.Frontend.Components.Ui`（以原语层气泡组合文本提示） | `App.Frontend.Views.Changes`、`App.Frontend.Views.Explores`（列表工具栏的图标按钮以气泡给出提示） |

`archi_module("App")` 结构化诊断为空；`archi_graph` 的 53 / 120 本体读数与 `diagnostics` 空属移交读数（§5 T2），本相位以「模型解析成图且逐元素邻接可读、`archi_api` 按 44 个 `path` 非空元素装配判据面」为正常态的等价证据。

### 3.5 计数三处同值（AC-1 的静态面）

| 面 | 读数 |
|---|---|
| 六个模型的 `[element]` / `[[relation]]` | `3/0`、`8/9`、`8/24`、`1/13`、`3/3`、**`30/71`** ⇒ **53 / 120** |
| `06-frontend.toml` 表头 | 「**30 元素 / 71 关系**」、「共享组件文件 **12**」 |
| ADR `§9` 复算表 | `06(30)`、`06(71)`、`53`、`120`、`前端 71 = 64 + 7` |
| `entry` 合计 | 仍 **3**（`04` 1 + `05` 2），两新元素零 `entry` |
| `api = ` 计数 | 仍 **2** 且均 `"none"`（`05` 两个工具 crate） |
| barrel 端点边 | 恰 **7**（3 出：`AgentInput` / `AgentSessionView` / `SessionPanel`；4 入：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`） |
| 前端 source（distinct `from`） | **24**（新增 `SessionPanel` / `Tooltip`） |
| 指向 `App.Backend.Shell` 的前端边 | **12**（既有 11 + ③，见 §3.3） |
| 容器端点 / 自环 | 三个无 `path` 容器零端点、`from == to` 零条 |

## 4 · 邻接现值未同步（登记，按裁决不改）

以下两处「42 个」系列的现值在本变更落地后应为 **44 / 32 / 12**，本变更**不改**（前者所在 requirement 不属两份 delta 的 8 个 MODIFIED；后者所在能力被 AC-10 钉为零 diff）。承接方：变更 B（12 个 barrel 入口化，会同时改 `apiEntryMissing` 桶构成与前端单文件根数）或独立的「spec 数字现值同步」小 change（design 待决问题 1）。

| 位置 | 现文 | 落地后应为 |
|---|---|---|
| `openspec/specs/desktop-archi-api-check/spec.md:79` | 「本仓 42 个」 | **44 个** |
| `openspec/specs/desktop-archi-api/spec.md:313` | 「42 个 / `Resolved` 30 / `Absent` 12」 | **44 / 32 / 12** |

## 5 · 移交清单（不勾为任务；承接方＝桌面端 static-check / test-execution 或人）

> 缺项是「移交未完成」，不是「实现未完成」。共同红线：MUST NOT 用增量报告、MUST NOT 手写伪 JSON、MUST NOT 单改模型数字去凑预估（proposal R-1）。

1. **全量报告落盘**：跑一次 `archi_drift(root, "full")`，出参**原样**落 `reports/archi/full-<UTC 时间戳>.json`（不裁剪、不改键）；双条件——文件名前缀 `full-` ＋ 文件内 `scan.mode = "full"` 且 `scannedFiles > 0`（`changedFiles = 0`）；缺任一条即不合格，增量报告 MUST NOT 占此槽位。现存 `incremental-20261116T145803082118Z.json` 保持原名、只作「本期未跑全量」的旁证。
2. **图正常态（AC-1）**：`archi_graph(root)` = **53 元素 / 120 关系**、`diagnostics` 空、`warnings` 空、`status = clean`、门禁 `passed = true`；反向判据：出现 `unknownField` / `unknownReference` 即停手查宿主与文件形态。
3. **归属（AC-2）**：按 §1.2 ④ 读 `archi_file_tree` 的两处 `mark` / `owners`。
4. **扫描面（AC-4 / AC-5）**：按 §1.2 ①②③ 三条并列判据对拍。
5. **不符时处置**：先查证据面（是否有别的元素 / 边进出扫描面、`entry` 是否仍 3 处、`INDEX` 是否已重生成），MUST NOT 单改模型数字去凑预估。
6. 若有人想以「回退 `INDEX.toml`」换 AC-6 的字面白名单：先读 design D5 / 待决问题 2——那会让 `api:check` 与仓库 `check` 链同时变红，正确处置是改 AC 而非改数据。
