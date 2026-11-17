# 提案: frontend-component-element-coverage

> **变更**: frontend-component-element-coverage（前端组件元素覆盖补齐 · 模型追上判据）
> **日期**: 2026-11-16
> **状态**: draft
> **来源**: `openspec/changes/frontend-component-element-coverage/explore.md`（requirement 类型；探索侧来源 `openspec/explores/ts-export-closure-extraction.md` C.6.7–C.6.9）
> **性质**: **纯数据 + 基线数字**——两份共享组件文件补建元素（`06` 两元素）、七条关系边落数据、计数三处同值换段（`51 / 113` → `53 / 120`、`06` 段 `28 / 64` → `30 / 71`）、`apiFindings` 分桶 +2（`12 / 27` → `12 / 29`）、两份 spec 共 **8 个 requirement** 同步、裁定记录 `§1` / `§3` / `§6` / `§7` / `§8` / `§9` 同步。零源码改动、零新增判据、零生成物 diff、零新增 / 改写测试。
> **不含**: `components/file/` 粒度档（改能力）、`views/architecture/agent/` 子域元素与 11 条既有漂移边的补齐、任何 `packages/desktop/src/**` 改动、任何契约资产。

---

## 问题

### 现状链路

```text
判据在案：.dev-team/architecture/decisions/0001-model-granularity.md §2
  「前端到组件：components/agent/*.{ts,tsx} 与 components/standard/*.tsx 每文件一元素（排除测试与 barrel index.tsx）」
存活 spec 在案：openspec/specs/desktop-change-chat/spec.md「面板骨架归一（共享 SessionPanel）」
  「骨架 SHALL 抽为共享组件 SessionPanel（components/agent/session-panel.tsx，经 components/agent 桶导出）」
公共面在案：components/agent/index.tsx 桶导出 SessionPanel / SessionPanelState
模型：components/agent/session-panel.tsx 与 components/standard/tooltip.tsx 未出现在 models/*.toml 的任何 path 里
```

三份证据全部指向「这两个文件是架构元素」，只有模型缺席——本变更是**让数据追上判据**，MUST NOT 被读成新增架构设计。

### 病灶一 · 两个文件不在模型内，其依赖与入边双双消失

扫描只对**有归属元素**的文件抽取 import（`scan` 的 `owned` 循环），无主文件落 `unmatchedFiles[]` 且**不产出任何边**。实测两个后果：

- **出边全丢**：`session-panel.tsx` 的 2 条值依赖（`AgentSessionView`、`@/components/ui/*`）与 `tooltip.tsx` 的 1 条（`@/components/ui/tooltip`）在图上为零；
- **入边全丢**：被 import 的目标若不可归属，该边直接丢弃（`scan`：`Owner::None` → `continue`），故 `Views.Changes → ？`（`change-list-view.tsx` 的 `TextTooltip`）、`Views.Explores → ？`、`Components.Agent（barrel）→ ？` 三条也都不存在。

净差 = 7 条边（6 条叶 → 叶 + 1 条 barrel 端点边）——这正是变更 C 的「同元素约束（owner 子树）」实算只得 5 个导出名、而判据说「恰 7 个」的原因。

### 病灶二 · 漂移两年不显形，靠的是两个静默机制

| 机制 | 效果 |
|---|---|
| 增量模式 | `scannedFiles = 0` 时 `unmatchedFiles` 恒空——现有 archi 报告是 `incremental`，缺口从不出现 |
| `gate` 全 `off` | 未声明依赖零违例、零留痕（`ArchiDriftKind::UndeclaredDependency` 在 `off` 引入方上不产出条目） |

两条都是**既有且合法**的纪律（增量省时、冷启动 ratchet），不是缺陷；本变更不碰它们，但验收 MUST 用**全量**模式（见 AC-1 前置）。

### 病灶三 · spec 与模型互相矛盾

`desktop-change-chat` 的存活 spec 已把 `SessionPanel` 描述为共享组件、把 `components/agent/index.tsx` 的桶导出当公共面用；而模型里没有该元素——于是「spec 描述的组件在架构图上不存在」。`tooltip` 是单面漂移：判据在案、spec 未提，但它被两个视图直接 import（`TextTooltip`），同样不可见。

### 病灶四 · 本次对拍新发现的既有漂移（本变更不补，只登记）

按「判据 ↔ 模型 `path` ↔ 代码 import」三面对拍全量前端后，模型还落后于代码三处（全部因 `gate` 全 `off` 而不显形）：

| 类别 | 事实 | 处置 |
|---|---|---|
| 边面缺口 | **11 条**已建元素间的跨元素 `import` 未声明（`Architecture → Components.Agent` / `Architecture → Standard.Dailog` / `Changes → Components.Agent` / `Changes → Standard.Drawer` / `Changes → Changes.Renderers` / `Explores → Standard.Dailog` / `AgentInput → Standard.Select` / `AgentInput → Ui` / `ToolCards → Ui` / `barrel → AgentMessages` / `Changes.Flow → Changes`） | **不补**：其中最后一条是「子 → 父」形态（声明即 `dependsOnAncestor` 停摆），补齐须先改代码分层，属另一个 change |
| 反向缺口 | **1 条**已声明却无证据（`Views.Changes.Agent → Components.Ui`，该目录只有测试文件 import `@/components/ui/resizable`） | **不补**：与本变更的两个文件无关；登记在案 |
| 粒度缺口 | `views/architecture/agent/`（`archi-chat-composer.tsx` / `archi-chat-panel.tsx` / `hooks/use-archi-chat.ts` = 递归非测试源文件 3 个）按 `§2` 判据应为子域元素 `Views.Architecture.Agent`，模型未建 | **不建**：补它会把元素面再扩一档、所有计数再换一轮，且与下游 C 的取数面无交集；登记在案 |

**范围决策**：本变更只把「**其判据覆盖、且下游变更 C 直接消费 owner 归属**」的两个文件补齐（explore 已锁定），其余三处按仓库既有机制登记为已知缺口（`§8`），MUST NOT 顺手补——理由：`gate` 全 `off` 的基线模型是**快照**而非导入闭包，把「快照补两处」与「全量对账」混在一个 change 里，会让「加边前后的差」失去可对拍性，也让本变更从 7 条边膨胀到 19 条边、计数换段两次。

---

## 提案

### 一 · 两个元素（数据唯一新增面）

| # | FQN | `kind` | `gate` | `path` | `summary` |
|---|---|---|---|---|---|
| 1 | `App.Frontend.Components.Agent.SessionPanel` | `component` | `off` | `./packages/desktop/src/components/agent/session-panel.tsx` | 会话面板骨架（头区动作 / 状态行 / 历史清单 / 折叠窄条，两处会话面共用） |
| 2 | `App.Frontend.Components.Standard.Tooltip` | `component` | `off` | `./packages/desktop/src/components/standard/tooltip.tsx` | 文本气泡（原语层 Tooltip 的业务封装） |

- **FQN 由文件名派生**（`session-panel.tsx` → `SessionPanel`、`tooltip.tsx` → `Tooltip`），与兄弟 `Dailog`（既有拼写）/ `Drawer` / `Pagination` / `Select` 同口径；`summary` MUST NOT 复述 FQN。
- **`entry` 零新增**：两者都是单文件根元素，约定默认即根文件自身——多写即违字段纪律（`repo-archi-model`「基线落位与模型文件划分」），并使「恰三处显式 `entry`」失守。
- **`api` 零声明**：取缺省 `declared`（它们有库 API），故各自落一条 `apiMissing`（见提案三）。

### 二 · 七条关系边（逐条取证）

| # | `from` → `to` | 证据（文件:行） | 性质 |
|---|---|---|---|
| ① | `Components.Agent.SessionPanel` → `Components.Agent.AgentSessionView` | `session-panel.tsx:9` `import { AgentSessionView, type AgentSessionActions } from './agent-session-view'` | 叶 → 叶 |
| ② | `Components.Agent.SessionPanel` → `Components.Ui` | `session-panel.tsx:4-5` `@/components/ui/button` + `@/components/ui/resizable`（按目标元素去重 1 条） | 叶 → 叶 |
| ③ | `Components.Agent.SessionPanel` → `App.Backend.Shell` | `session-panel.tsx:7` `import type { SessionSummary } from '@/types/generated/bindings'` | 命令面边（**声明式约定**，见下） |
| ④ | `Components.Agent`（barrel）→ `Components.Agent.SessionPanel` | `components/agent/index.tsx:5-6` `export { SessionPanel }` / `export type { SessionPanelState }`（去重 1 条） | barrel 端点边 |
| ⑤ | `Components.Standard.Tooltip` → `Components.Ui` | `standard/tooltip.tsx:1` `import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'` | 叶 → 叶 |
| ⑥ | `Views.Changes` → `Components.Standard.Tooltip` | `views/changes/change-list-view.tsx:5` `import { TextTooltip } from '@/components/standard/tooltip'` | 叶 → 叶 |
| ⑦ | `Views.Explores` → `Components.Standard.Tooltip` | `views/explores/explore-list-view.tsx:5` 同上 | 叶 → 叶 |

**③ 的口径（explore 的 ★ 待核项，已核清）**：`ImportFact.type_only` 在扫描面**只标注、不豁免**（`scan/mod.rs` 的字段注释即「MUST NOT 因它豁免」），故口径不存在「值 import 才建边」的分叉；但 `@/types/generated/bindings` 解析出的目标文件属封闭排除面（`src/types/**`，无元素归属），扫描侧按 `Owner::None` 丢弃该边 ⇒ ③ 与既有 11 条前端命令面边一样，是**声明式约定**：落模型、不进违例桶、在报告里落 `observations[].unfulfilledDeclaration`。故前端命令面边由 11 → 12，`errors` / `warnings` 仍空（`gate` 全 `off`），MUST NOT 用「`observations` 为空」做验收。

### 三 · 计数换段（三处同值）

```text
06-frontend      28 / 64    →  30 / 71
    元素 = 页壳 1 + 视图 9 + 子域 3 + 共享组件文件 10 → 12 + 原语层 1 = 26 叶，另有 4 容器
    关系 = 叶边 58 → 64（+6）、barrel 端点边 6 → 7（+1，3 出 + 4 入）
    恒等式 71 = 64（叶 → 叶 / 叶 → 容器）+ 7（barrel 端点边）    ※ 旧式 64 = 58 + 6
    前端 source（distinct `from`）22 → 24
全仓             51 / 113  →  53 / 120（后端 49 不变）
apiFindings      12 / 27（总 39）→ 12 / 29（总 41）；分母（`path` 非空元素）42 → 44
    +2 全部落 ② `apiMissing`：两个新元素「有入口（文件型根由约定默认解析）、无契约文件」
    ① `apiEntryMissing` 仍 12（8 视图目录根 + 3 视图内子域 + `Components.Ui`）
```

三处 MUST 同值：`06` 表头注释 / `repo-archi-model` 计数表 / 裁定记录 `§9` 复算表；`§1`（结论）、`§3`（barrel 端点边 3 出 + 4 入 = 7）、`§6`（元素总数）同批同步。

### 四 · 缺口登记（documentation，不是补齐）

裁定记录 `§8` 新增三条 + `§7` 补一句口径：

1. `components/file/file-viewer.tsx`：`§2` 三层表无 `components/<其它目录>` 档 ⇒ **判据缺口**，补它须新增粒度档（改能力）；
2. `views/architecture/agent/`：子域判据命中（3 个递归非测试源文件）但未建元素 ⇒ 补齐属独立变更；
3. 11 条已建元素间的跨元素 `import` 未声明（含 `Views.Changes.Flow → Views.Changes` 的「子 → 父」形态，声明即 `dependsOnAncestor` 停摆）⇒ 补齐须先改代码分层；
4. `§7` 补「命令面边是声明式约定：`src/types/**` 无元素归属 ⇒ 扫描丢弃该边，报告落 `observations`」——把既有 11 条 + 本次第 12 条的形态从「隐性事实」变成「显式口径」。

登记 ≠ 判据失效：三条各自写清「为什么现在不补」与「补齐须做什么」，并 MUST NOT 被读成本变更的差集。

### 五 · spec 同步（两份 delta，8 个 MODIFIED requirement）

| 能力 | requirement | 改什么 |
|---|---|---|
| `repo-archi-model` | 基线落位与模型文件划分 | 计数表与合计段换数（53 / 120、`06` 段 30 / 71、叶 26、共享组件 12）、恒等式 `71 = 64 + 7`、`entry` 三处不变但前端单文件根 12 → 14（scenario） |
| 同上 | 前端下钻到视图与组件层的元素覆盖 | 共享组件档的元素清单 +2、逐文件核对 6 → 7 与 4 → 5、覆盖判据（双向）的已知缺口例外条款 |
| 同上 | 前端关系只取 import 证据 | barrel 端点边 6 → 7、恒等式 `71 = 64 + 7` / 24 个源、命令面边补「声明式约定」口径、反向抽查的缺口例外 |
| 同上 | 图自洽与停摆禁令 | 正常态 51 / 113 → 53 / 120（前端 71） |
| 同上 | 覆盖判据的机械核对与漂移纪律 | 已知缺口清单（三条）、关系证据的命令面边例外、三件套核对的基线数 |
| 同上 | 本仓架构域看护配置与排除留痕 | 两处历史读数换现值（53 / 120、`06` 段 30 / 71） |
| `desktop-archi-api-check` | 报告并列槽位 apiFindings | 存量 backlog 场景 12 / 27 / 39 → 12 / 29 / 41 |
| 同上 | 契约判据的验证语料与报告数基线 | 基线表 12 / 29（总 41）、分母 44、四段读数串讲、基线单段、三个 scenario 的桶构成 |

**纪律**：只改数字与措辞，不改判据——三档粒度表、`gate` 全 `off`、`entry` / `api` 字段纪律、封闭排除面、十值判据词汇、求值互斥序、纳管口径、语料守护两层全部逐字不动；旧数只作为**已退场口径**出现在历史叙述句内。

### 六 · 版本纪律

`packages/desktop/package.json` `version` 由 `0.6.47` +1 patch 至 **`0.6.48`**；`tauri.conf.json` 经 `"version": "../package.json"` 跟随（文件本身零内容改动）；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

**判据（MUST 写死，不许留空）**：`.dev-team/architecture/**` 是本仓**随应用同批交付的架构资产**（应用运行期直接读它出图 / 出判据），其改动属「发布物内容变更」⇒ +1 patch；同类前例是 `module-entry-data-backfill`（纯数据 + spec，同样 +1）。反向判据也写明：若判定「零 `packages/desktop/src/**` 与 `src-tauri/**` 改动 ⇒ 不 bump」，则 `package.json` 必须零 diff——两条只许取一条，MUST NOT 既改又说不改。

---

## 能力

### 新增能力

**无。** 两个元素不产生新用户可见能力：它们只让既有判据、既有 spec 描述、既有桶导出在模型里获得对应元素；关系边只补既有规则（`import` 证据 / 命令面边映射 / barrel 端点例外）的落数据。

### 修改的能力

- **repo-archi-model** — 六个 requirement MODIFIED（见提案五）。元素 / 关系**判据**（三档表、封闭排除面、粒度 ≠ 解析面、`gate` 全 `off`、`entry` 恰三处、`api = none` 两处留白、`docs` / `metadata` / `desc` 纪律、诊断零命中）逐字不动；只有集合成员与计数换值，外加两处**已知缺口登记条款**（它记录既成事实，不新增判据）。
- **desktop-archi-api-check** — 两个 requirement MODIFIED：基线表与读数链换段（`12 / 29（总 41）`、分母 44、四段串讲）、存量 backlog 场景换数。判据十值、求值互斥序、判据级档位、纳管子集、`REPORT_VERSION = 3`、语料守护两层逐字不动。

| 不涉及 | 理由 |
|---|---|
| `desktop-change-chat` | **预期零改动**：其「面板骨架归一（共享 SessionPanel）」早就把该文件描述为共享组件——本变更是让模型追上它；归档时若该 spec 出现 diff，说明误改了判据 |
| `desktop-archi-model` | 模型**格式 / 字段面**（`kind` / `gate` / `path` / `entry` / `api` / 九类诊断 / 聚合语义）零改动，只多两条数据 |
| `desktop-archi-drift` | 报告结构、四组词汇、`status` 五值、`observations` 与 `warnings` 分界、豁免三层全部零改动；新增元素在报告面只表现为 `unmatchedFiles` −2 与 `apiFindings` +2 |
| `desktop-archi-canvas` / `desktop-archi-file-tree` | 消费面按 `path` 枚举，两个新元素自动成为 2 个节点 / 2 个模块根；两者契约零改动 |
| `desktop-archi-gate` / `desktop-archi-contract-panel` | 门禁布尔与呈现面按同一份报告工作，`status` 仍 `clean`、`passed` 仍 `true`；契约视图条目 39 → 41 不需改代码（按 `category` 分组） |
| `desktop-archi-api` | `entry` 三态解析链与生成物落点零改动（两个新元素由约定默认命中） |
| `desktop-ipc-type-bindings` | `ArchiElement` 形状不变，模型数据变化不触发再生成 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `.dev-team/architecture/models/06-frontend.toml` | 2 个 `[element."…"]` 段 + 7 条 `[[relation]]`（各带一句中文 `desc`）；表头两处数（`28 元素 / 64 关系` → `30 元素 / 71 关系`、`共享组件文件 10` → `12`）；其余 28 元素与 64 条边逐字不动 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | `§1` 结论（51 / 113 → 53 / 120，前端 71）、`§3` barrel 端点边（「2 出 + 4 入 = 6」→「3 出 + 4 入 = 7」）、`§6` 元素总数、`§7` 补命令面边口径一句、`§8` 三条缺口登记、`§9` 复算表（`06` 段 30 / 71、恒等式 `71 = 64 + 7`）；`§2` / `§4` / `§5` 逐字不动 |
| `packages/desktop/package.json` | `version` `0.6.47` → `0.6.48` |
| `packages/desktop/src-tauri/tauri.conf.json` | **零内容改动**（`"version": "../package.json"` 跟随；实现时只需确认未写死版本号） |

### 测试文件

| 对象 | 判断 |
|---|---|
| 新增测试 | **无**。本变更零代码逻辑；仓库模型属数据资产，`desktop-archi-api-check` 明写「MUST NOT 把本仓模型与契约塞进 crate 语料」、基线表「不写进断言（易随元素增减漂移）」——为两行数据新增测试或把 `53 / 120` / `12 / 29` 写成期望值，会把「报告数与实测对拍」的责任从 change 留痕搬到脆弱常量上 |
| 改写既有测试 | **无**。全仓测试零读取 `.dev-team/architecture/models/**`（前端 fixture 的 `modelFile` 是字面路径字符串，不读盘；crate 语料自足）；`ArchiElement` 形状不变 |
| 验收留痕 | change 内 `reports/archi/` 留三份实测：① 加元素前后的 `unmatchedFiles` 差集（应恰好少这两条）、② 全量 `apiFindings` 按 `kind` 分桶的元素清单、③ 71 条边逐条 import 证据（含 ③ 声明式标注）。**必须注明本次用全量模式**，否则后来者会以为缺口是新出现的 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,04-backend-shell,05-backend-tools}.toml` | 五份文件元素 / 关系 / `path` / `entry` / `api` 逐字不变（后端 49 条边零改动）；本变更只碰 `06` |
| `packages/desktop/src/**` 与 `packages/desktop/src-tauri/**` | 零源码改动；AC-6 的 `git diff` 白名单是硬拦挡 |
| `.dev-team/architecture/api/**`（含 `INDEX.toml` 与四类生成物） | 零契约改动、零生成物产出的期望是**验收判据**（AC-6）；`api:check` 三列非空即说明误动 |
| `views/architecture/agent/` 的元素化、11 条既有漂移边、`Views.Changes.Agent → Ui` 的无效声明 | 本变更**只登记不补齐**（提案四）：会二次换段计数并使「加边前后差 7」不可对拍 |
| `components/file/file-viewer.tsx` 的粒度档 | 补它 = 新增 `components/<其它目录>` 档 = 改能力；`§8` 登记即止（AC-7 反向判据） |
| `openspec/specs/desktop-change-chat/spec.md` | 它与本变更同向、早已在案；出现 diff 即误改 |
| 扫描面纪律（增量 / 全量语义、`gate` 全 `off`、豁免三层） | 本变更不改判定规则；只在验收侧要求**全量**取数（AC-1） |

---

## 验收标准

1. **AC-1 计数三处同值且图正常态**：一次**全量**检查（门禁步 `LocalArchiCheck` 与 `archi_drift` 两面逐字段相等，取任一面落 `reports/archi/`）后 `archi_graph(root)` 返回 **53 元素 / 120 关系**、`diagnostics` 空、`warnings` 空；`06` 表头写「30 元素 / 71 关系」与「共享组件文件 12」，`repo-archi-model` 计数表与裁定记录 `§9` 复算表逐字同值；前端恒等式 `71 = 64 + 7` 成立（3 出 + 4 入 barrel 端点边）。反向判据：`diagnostics` 若出现 `unknownField` / `unknownReference`，先查宿主与文件形态，MUST 停手排查，MUST NOT 改模型去迁就。
2. **AC-2 归属可复算且不 Ambiguous**：`owners.owner_of("packages/desktop/src/components/agent/session-panel.tsx")` = `Owner::Element { fqn: "App.Frontend.Components.Agent.SessionPanel" }`；`…/standard/tooltip.tsx` = `Owner::Element { fqn: "App.Frontend.Components.Standard.Tooltip" }`。两者 MUST NOT 落 `Owner::Ambiguous`（与既有文件型 `path` 前缀判据冲突即为不合格）。
3. **AC-3 判据面 +2 且不抬黄灯**：同趟 `apiFindings` 总 **41** = `apiEntryMissing` **12**（8 视图目录根 + 3 视图内子域 + `App.Frontend.Components.Ui`）+ `apiMissing` **29**（13 个 crate + `App.Frontend.Shell` + **14** 个前端单文件根 + `App.Backend.Shell`），逐条点名含 `Components.Agent.SessionPanel` / `Components.Standard.Tooltip` 各一条「declared 但契约文件缺失」；其余七条判据各 0；`status = clean`、门禁 `passed = true`（纳管子集仍为空）；分母（`path` 非空元素）= **44**。桶数成因（+2 全落 ②）须写进 change 报告。
4. **AC-4 扫描面无新增未覆盖**：全量 `unmatchedFiles` **不再含** `session-panel.tsx` / `tooltip.tsx`（实测应恰少这两条）；余项只含 `components/file/file-viewer.tsx`（`§8` 已登记）与 `components/standard/hooks/use-open.ts`（`§3` 合法排除）。`matched` 内两个新元素的 `files` 各为 1。
5. **AC-5 七条边逐条可核**：6 条值依赖边逐条能在 `from` 元素覆盖范围内的文件找到对应 `import`（① `session-panel.tsx:9`、② `:4-5`、④ `components/agent/index.tsx:5-6`、⑤ `tooltip.tsx:1`、⑥ `change-list-view.tsx:5`、⑦ `explore-list-view.tsx:5`）；③ 为**声明式命令面边**（`session-panel.tsx:7` 的 `import type … bindings`）——它 MUST 在案，MUST NOT 出现在 `errors` / `warnings`，其形态表现为 `observations[].unfulfilledDeclaration`（前端 → `App.Backend.Shell` 的声明边 11 → 12）；`desc` 各为一句中文用途，MUST NOT 写证据清单。
6. **AC-6 零代码扩散**：`git status --porcelain` 的改动集 ⊆ {`models/06-frontend.toml`、`decisions/0001-model-granularity.md`、本 change 目录、`packages/desktop/package.json`}；`packages/desktop/src/**` 与 `src-tauri/**` 零 diff；`pnpm api:check` 三列（`missing` / `mismatched` / `orphaned`）皆空；`bindings:export` 幂等（再导出零 diff）。
7. **AC-7 缺口登记到位且不越界**：`§8` 出现三条登记（`components/file/file-viewer.tsx` 写明「补它须改本能力：新增 `components/<其它目录>` 粒度档」、`views/architecture/agent/`、11 条未声明边）；反向判据：`components/file/file-viewer.tsx` 与 `views/architecture/agent/**` 仍**不在**任何元素 `path` 内，`06` 元素数仍 30（补了就是越界），模型内除七条新边外零新增边。
8. **AC-8 spec 同步且无现值冲突**：本 change 的两份 delta 与存活 spec 落地后，`openspec/specs/repo-archi-model/spec.md` 内 `51 / 113`、`06` 段 `28 / 64`、`64 = 58 + 6`、`22 个源`、`6 条 barrel` 的命中**只允许**出现在「已退场 / 历史读数」语境内；`53 / 120`、`30 / 71`、`71 = 64 + 7`、`24 个源` 在计数表、恒等式与 ADR `§9` 三处在案；`desktop-archi-api-check` 内 `12 / 27`、`39` 的命中仅作历史读数，现值 `12 / 29（总 41）` 在基线表与「基线单段」两处在案。**MUST NOT 只改存活 spec 而让 delta 与本体各说各话**（归档后 delta 退场，存活 spec 是唯一事实源）。
9. **AC-9 版本纪律**：`packages/desktop/package.json` `version` = `0.6.48`（相对落地时 `HEAD` 现值恰好 +1 patch，**只加一次**）；`tauri.conf.json` 仍为 `"../package.json"` 引用形态；`packages/desktop/src-tauri/Cargo.toml` 的 `[package] version` 逐字不变。
10. **AC-10 相关能力零漂移**：`openspec/specs/desktop-change-chat/spec.md`、`desktop-archi-model`、`desktop-archi-drift`、`desktop-archi-canvas`、`desktop-archi-file-tree`、`desktop-archi-gate`、`desktop-archi-contract-panel`、`desktop-archi-api` 零 diff（本变更不落 delta 的八个能力）。

> **不列为 AC 的流程项**：全量 / 增量模式语义、报告槽位形状、十值词汇封闭、生成器三子命令、`status` 推导序——各有既有 spec 落点且本变更零改动，不另设无 spec 落点的断言。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **边数与计数来自静态推演，非扫描实测**：explore 预估 `+7 / 30 / 71`；本提案按 `owner_of` + `type_only` 语义逐条核过 7 条（含 ③ 的口径），但真实扫描的 `unresolved` / `excluded` 面未跑 | AC-1 以一次全量实测为准；若实测与预期不符，MUST 先查「是否有别的元素 / 边进出扫描面」，MUST NOT 单改模型数字去凑预估 |
| R-2 | **与变更 B / C 的顺序耦合**：B 会把 `Components.Agent` 的 `path` 从 `index.tsx` 翻转为目录（barrel 例外退场）；C 的判据 1 依赖 `session-panel.tsx` 的 owner 落在 `Components.Agent` 子树 | 本变更**先于 B、C**落地；B 落地时 `06` 的元素 / 关系数不变（只改 `path`），C 的 7 名字判据在翻转前后都成立。若顺序颠倒，计数与 `barrel 端点边` 口径须在报告内说明后再改 |
| R-3 | **把 ADR 当数据改**：`§2` 的「`components/<其它目录>` 无档」是能力（判据）而非数据 | AC-7 钉住：本变更只登记缺口、不新增粒度档；新增档须另开 change |
| R-4 | **缺口登记被读成「已修」或「判据失效」** | `§8` 每条写明「为什么现在不补 / 补齐须做什么」；AC-7 的反向判据（元素数仍 30、`file-viewer` 仍不入模型）机械拦挡 |
| R-5 | **用 `observations` 为空做验收**：③ 与既有 11 条命令面边同为声明式约定，恒落 `observations[].unfulfilledDeclaration` | AC-5 明写口径与「11 → 12」；MUST NOT 为消观测项去实现 `bindings` → `App.Backend.Shell` 的代码级映射（属改能力 + 改代码） |
| R-6 | **用增量模式验收**：`scannedFiles = 0` 时 `unmatchedFiles` 恒空，缺口看起来像「新出现」或「已消失」 | AC-1 / AC-4 明写全量；报告内注明本次用全量模式 |
| R-7 | **版本号两跳或漏跳**：`0.6.47` 是 `module-entry-data-backfill` 已用掉的 +1 | AC-9 只 +1 一次（→ `0.6.48`）；`Cargo.toml` 不随动 |
| R-8 | **`summary` / `desc` 写偏**：复述 FQN、写证据清单、或把 `Tooltip` 写成「与原语层重名的设计说明」 | `repo-archi-model` 既有 `summary` / `desc` 纪律（一句中文职责 / 一句用途）；审核时逐条对表 |
| R-9 | **把「模型更完整」误读为「架构变了」** | spec 与 ADR 措辞统一为「让数据追上判据」；本变更 MUST NOT 被当成新增设计（两元素在代码里早已存在且被两处会话面共用） |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| §4.2 ★「`bindings` 的 type-only import 是否建边」 | **已核并定论**：`type_only` 在扫描面只标注不豁免（`scan/mod.rs` 字段注释「MUST NOT 因它豁免」），故无「值 import 才建边」分叉；但目标落封闭排除面（`src/types/**` 无元素归属）⇒ 该边在扫描面被丢弃，模型侧落 `observations`。③ **保留在案**，总边数仍 120、恒等式仍 `71 = 64 + 7`（R-5 的「③ 退场则 119」分支不再适用） |
| §7 R-1「边数来自判据推演」 | 采纳：AC-1 实测优先，实测与预期冲突时先查证据面、不得凑数（本提案 R-1） |
| §7 R-2「与变更 B / C 的基线链」 | 采纳：`api-check` 的读数链由三段扩为**四段**并写明时间序；`12 / 29` MUST NOT 被写成长期值（B 落 12 个 barrel 后 ① 归零、② 重排） |
| §7 R-3「ADR 是能力不是数据」 | 采纳：只登记缺口不改档（AC-7、R-3） |
| §7 R-4「版本 bump 判据缺失」 | 采纳并写死：+1 patch（0.6.47 → 0.6.48），判据见提案六（R-7） |
| §7 R-5「`Tooltip` 的 FQN 命名」 | 采纳 `…Standard.Tooltip`（与文件名对应、与兄弟同口径） |
| §5「`desktop-change-chat` 预期零改动」 | 采纳，并升级为 AC-10（该 spec 出现 diff 即误改） |
| §9「起草时先跑一次全量留痕」 | 采纳为验收前置动作：`reports/archi/` 留加元素前的 `unmatchedFiles` / `apiFindings` 与事后的对照（「测试文件」行） |
| explore 未覆盖、本次新增发现的三处既有漂移（11 条未声明边 / 1 条无证据声明 / `views/architecture/agent/` 子域） | **不补、登记**：写进 `§8` 与 `repo-archi-model` 的既有已知缺口清单（提案四），补齐另开 change |
