# desktop-frontend-module-entry Specification

## Purpose

定义前端「**目录 = 模块**」的机械载体：12 个目录元素各持一个 `index.ts(x)` 入口（barrel），入口的导出面**恰等于跨元素被 import 的名字**，调用点一律经入口收窄，`components/ui` 是唯一「建入口但调用点直引」的例外，`App.Frontend.Components.Agent` 由「容器 `path` 唯一例外」翻转为目录型 `module`。本能力只管**前端源码结构的入口面**：入口的解析链、生成物落点与取数面归 `desktop-archi-api`，元素粒度与关系证据归 `repo-archi-model`，判据读数归 `desktop-archi-api-check`。

## Requirements

### Requirement: 目录模块入口的落位与命名

12 个前端目录元素 SHALL 各持**恰一个**入口文件，入口文件 SHALL 落在该元素 `path` 之内，且 SHALL 由入口三态解析链的 ③（目录型根的约定默认 `<root>/index.ts` / `<root>/index.tsx`）命中——MUST NOT 靠显式 `entry` 字段声明（字段纪律见 `repo-archi-model`「基线落位与模型文件划分」）。

| 组 | 目录（元素 `path`） | 入口文件 | 导出名字 |
|---|---|---|---|
| 多文件 | `views/changes/` | `index.ts` | `ChangeDetailView` / `ChangeListView` |
| | `views/changes/agent/` | `index.ts` | `ChangeChatPanel` |
| | `views/changes/flow/` | `index.ts` | `mountMaterials` / `ChangeFlowGraph` / `DetailDrawer` / `buildFlowGraph` / `RunControlPanel` / `runStepNodes` / `unifiedRunEdges` / `unifiedRunNodes` / `DrawerSelection` / `FlowGraph` / `FlowMaterials` |
| | `views/changes/renderers/` | `index.ts` | `ArtifactTabs` / `MarkdownDocRenderer` |
| | `views/explores/` | `index.ts` | `ExploreDetailView` / `ExploreLayout` / `ExploreListView` |
| | `components/ui/` | `index.ts` | 40 条 `export *`（唯一例外，见「`components/ui` 的入口例外」） |
| 单文件 | `views/{agent,agents,architecture,config,db,info}/` | 各 `index.ts` | 各 1 个视图组件（`AgentDebugView` / `AgentsView` / `ArchitectureView` / `ConfigView` / `DbInspectorView` / `InfoView`）；其 `*Props` 类型 MUST NOT 外露 |

- **单文件目录同样补入口**：多文件目录靠 barrel、单文件目录靠「唯一文件即入口」＝同仓两套约定，且单文件目录多一个 `helper.ts` 就静默换默认入口（虽会立刻报 `apiEntryMissing`，仍是漂移源）。统一形态是「目录 = 模块」的前提，MUST NOT 以「一个文件不需要 barrel」为由留缺口。
- **扩展名 `index.ts`**：12 个入口的正文全是 re-export、零 JSX；既有 `components/agent/index.tsx` SHALL 保持原扩展名不动（③ 的两条候选都认，形态差异不影响入口判据）。
- **入口是新增文件**：12 个目录内的既有实现文件 SHALL 逐字不变（barrel 不搬代码、不改 `export` 面）。
- **入口 MUST NOT 成为第二套默认入口**：入口只做 re-export，MUST NOT 在入口内新增实现、类型定义或运行期逻辑（否则「入口 = 公共面」与「桶内实现」两件事混在一处）。

#### Scenario: 12 个目录各有一个入口且全部可解析

- **WHEN** 枚举 12 个目录元素（`Components.Ui` + 8 个视图目录根 + 3 个视图内子域）并逐个跑 `paths::entry_state`
- **THEN** 每个目录各有且仅有一个 `index.ts`（`components/agent/` 不在 12 个之列、其既有 `index.tsx` 逐字不动），全部为 `Resolved` 且入口路径落在其元素 `path` 之内；本仓 44 个 `path` 非空元素因此为 `Resolved` 44 / `Absent` 0 / `Invalid` 0；模型内显式 `entry` 行数仍为 **3**（本变更零加行）

#### Scenario: 单文件目录不漏

- **WHEN** 检查 `views/{agent,agents,architecture,config,db,info}/` 六个各只有 1 个非测试源文件的目录
- **THEN** 六者各有 1 个 `index.ts`（各 1 条具名 re-export，不外露 `*Props`）；给其中任一目录补一个 `helper.ts` 后默认入口不变（仍 `index.ts`），MUST NOT 出现「新增源文件换默认入口」的形态

#### Scenario: 入口只做透传

- **WHEN** 审查 12 个入口文件的内容
- **THEN** 每个文件只含 `export { … } from './x'` / `export type { … } from './x'` / `export * from './x'`（ui 例外）三类语句与注释；零 `export function` / `export const` / 类型定义 / 运行期语句（入口不承载实现）

### Requirement: 入口 = 公共面

入口的导出面 SHALL **恰等于**「跨元素被 import 的名字」集合：多导一个名字即虚增公共面，少导一个即断调用点。该等式 SHALL 由两条机械判据守——`knip` 的 unused-export 报告与调用点的 typeCheck；`components/ui/` 在 `knip` 的 `ignore` 内，其导出面由源文本断言守（见「`components/ui` 的入口例外」）。

- **逐条手挑，MUST NOT `export *`**（ui 唯一例外）：本仓 `views/changes/flow/` 有约 38 个公开符号，其入口只出 **11** 个——每次扩公共面都得显式改入口，这是「入口 = 公共面」的机械代价，也是它唯一的价值。
- **MUST NOT 跨子元素 re-export**：`api::extract` 的同元素约束按 FQN **前缀**放行（子元素也在面内），而 `scan` 把 `export … from` 记作 import fact ⇒ 跨子元素 re-export 会**真加边**（`Views.Changes → Views.Changes.Flow` 一类），前端 71 条关系就不再是不变量。故每个入口只 re-export **本元素直属文件**。
- **MUST NOT re-export `hooks/` 与 `lib/`**：视图内这两类目录的 owner 就是所属视图（目录归属），机械判据拦不住，只能靠纪律——把内部实现写进入口，等于让「入口 = 公共面」从第一天起失真。
- **类型出口一律 `export type`**：`isolatedModules = true` 下 `export { TypeName } from './x'` 会在转译期被当成值导出；同一来源文件的类型出口与值出口 SHALL 拆成两条语句（与 `components/agent/index.tsx` 既有形态同族）。
- **本条 MUST NOT 被读成「入口是唯一允许的 import 路径」**：`components/ui/` 的调用点直引是已裁例外（见下条 requirement）；其余 11 个入口的调用点 SHALL 经入口。

#### Scenario: 多导一个名字即判红

- **WHEN** 给 `views/changes/renderers/index.ts` 追加一条未被任何跨元素调用点使用的 re-export
- **THEN** `vp check`（含 `knip`）报该 export unused 并判红；撤掉该行后恢复绿——该判据即「入口 = 公共面」的机械面，MUST NOT 以注释或豁免绕过

#### Scenario: 少导一个名字即断调用点

- **WHEN** 从 `views/changes/flow/index.ts` 删掉 `runStepNodes`
- **THEN** `change-detail-view.tsx` 的 `./flow` 具名导入编译失败（`tsc --noEmit` 判红）；该状态 MUST NOT 被「调用点改回文件级 import」绕过——回退必须改回入口，否则「调用点经入口」失守

#### Scenario: 跨子元素 re-export 被纪律拦住

- **WHEN** 让 `views/changes/index.ts` 顺手 re-export `./flow/graph` 的 `buildFlowGraph`
- **THEN** 该形态判不合格：`scan` 会把这条 `export … from` 记成 import fact，`Views.Changes → Views.Changes.Flow` 成为新边、前端关系由 71 变 72；入口 MUST 只导本元素直属文件（`change-detail-view` / `change-list-view`），子域公共面经子域自己的入口取

#### Scenario: 类型出口走 `export type`

- **WHEN** 审查 11 个手挑入口的类型出口行
- **THEN** `DrawerSelection` / `FlowGraph` / `FlowMaterials` 等类型出口逐条为 `export type { … } from './types'`，零 `export { … }` 形态导出类型；把任一条改成值出口即由 `module-entries.test.ts` 的源文本断言与 `vp check` 的 typeCheck 双重判红

### Requirement: 调用点经入口收窄

跨元素的消费点 SHALL 经目录入口导入，MUST NOT 穿透到元素内的单个文件（唯一例外：`components/ui/`）。

| 调用点 | 收窄前 | 收窄后 |
|---|---|---|
| `packages/desktop/src/routes.tsx` | 11 条文件级 import（打向 8 个视图目录） | **8** 条目录级 import |
| `views/changes/change-detail-view.tsx` | `./flow/*` 7 条 + `./agent/*` 1 条 | **2** 条（`./flow` / `./agent`） |
| `views/changes/flow/detail-drawer.tsx` | `../renderers/artifact-tabs` | `../renderers` |
| `views/explores/components/explore-preview.tsx` | `../../changes/renderers/markdown-doc-renderer` | `../../changes/renderers` |

- **目录 specifier 由单点解析**：`./flow` / `../renderers` 一类目录导入 SHALL 经 `scan::ts::TsResolver::resolve` 的 index 折叠解析到 `<dir>/index.ts`，MUST NOT 为调用点写第二份解析或显式补 `/index`。
- **边集是不变量**：入口与被它聚合的文件 SHALL 归属同一元素（入口的 owner 是该目录元素、re-export 目标是本元素直属文件 ⇒ `scan` 的 `from == to` 自边被丢弃），故前端关系数 MUST 等于收窄前的 71 条 / 24 个源，逐条 `from` / `to` / `desc` 相同。
- **`components/ui/` 的调用点不改**：132 处 `@/components/ui/<mod>` 保持直引（已裁，见「`components/ui` 的入口例外」）。

#### Scenario: 收窄后边集逐条相等

- **WHEN** 分别在收窄前后跑同一次架构检查（`archi_drift` 命令与门禁 runner 是同一实现）
- **THEN** 前端 `relations` 的 `from` / `to` / `desc` 三列 diff 为空、**71 条 / 24 个源**不变、`unresolved` 与 `scan.unmatchedFiles` 零新增；反向判据：若某个目录 specifier 解析失败（未折叠到 `index.ts`），该元素的一条或数条边消失、总数 < 71，判检查器 / 解析面缺陷，MUST NOT 更新基线

#### Scenario: 12 个目录零深路径残留

- **WHEN** `grep` 全仓（`components/ui` 除外）是否存在 `from '…/views/<dir>/<file>'` 形态的跨元素文件级 import
- **THEN** 命中为零；`change-detail-view.tsx` 只余 `./flow` / `./agent` 两条本地目录入口导入（其余 `./hooks/...` 与 `../../../types/dto` 一类不属于跨元素面）

#### Scenario: ui 的 132 处刻意不改

- **WHEN** 统计 `@/components/ui/<mod>` 形态的 import
- **THEN** 仍为 **132** 处 / **67** 个文件 / **35** 个模块（另 1 处 `@/components/ui/lib` 是目录不是模块）；该数字 MUST NOT 被当成待办——它是已裁例外的留痕，改写属独立变更

### Requirement: `components/ui` 的入口例外

`components/ui/index.ts` SHALL 是**唯一**「建入口但调用点不改写」的目录：其正文 SHALL 为 `export * from './<mod>'` 形式的 **40** 条语句（目录内 40 个非测试源文件各一条），MUST NOT 手维护「被引用的 35 个模块」名单；文件首行 SHALL 留一行注释说明「本文件是目录模块的入口声明，视图内直引模块名是刻意选择，勿据此判死代码」。

- **40 而非 35 的理由**：手维护白名单一旦某模块开始被引用就要同步改入口，而 `src/components/ui/**` 在 `knip.json` 的 `ignore` 与 `vite.config.ts` 的 `NO_STATIC_CHECK` 内（零死代码扫描、零格式化检查）——没有任何机制会提醒该同步。`export *` 让「目录 = 模块」自洽且零维护。
- **`export *` 的安全性前提**：ui 目录零 `export default`、具名导出无重名（重名会被静默吞掉），故 40 条 `export *` 无遮蔽风险；该前提 SHALL 由源文本断言守，MUST NOT 靠「读一遍没重名」。
- **已知缺口登记**：5 个零引用模块（`button-group` / `context-menu` / `input-group` / `marker` / `sheet`）随 `export *` 进入公共面。清理（删模块或改白名单）属独立变更，本能力 MUST NOT 顺手清理；残留缺口 MUST NOT 被读成「ui 的公共面等于真实使用面」。
- **零校验的补偿**：ui 入口不在 `knip` 面内 ⇒ 其导出面由 `packages/desktop/src/module-entries.test.ts` 的源文本断言守（40 条 `export *`、零 `export default`、零重名），MUST NOT 以「knip 绿」冒充「ui 入口被校验」。

#### Scenario: 40 条 `export *` 与零 default

- **WHEN** 读取 `packages/desktop/src/components/ui/index.ts` 并比对目录内非测试源文件
- **THEN** 恰 40 条 `export * from './<mod>'`、每条对应一个现存模块、零 `export { … } from`、零 `export default`、零实现语句；目录内 40 个模块的具名导出无重名

#### Scenario: 零引用模块在案不误报

- **WHEN** 检查 5 个零引用模块（`button-group` / `context-menu` / `input-group` / `marker` / `sheet`）
- **THEN** 五者仍在 ui 入口的 `export *` 面内且不产生任何死代码报告（目录在 `knip` `ignore` 内）；五条 MUST 单列为已知缺口，MUST NOT 在本变更内删除

#### Scenario: 入口不被判死文件

- **WHEN** 审查 `components/ui/index.ts` 的调用面
- **THEN** 全仓零 import 指向 `@/components/ui`（132 处调用点均直引模块名），而该文件首行注释说明这是刻意选择；MUST NOT 因「零调用者」删该文件——它是 `Components.Ui` 的入口（`apiEntryMissing` 退场的前提）与契约落点

### Requirement: `App.Frontend.Components.Agent` 由容器翻转为目录模块

`App.Frontend.Components.Agent` SHALL 由 `kind = "domain"` + 文件型 `path`（`./packages/desktop/src/components/agent/index.tsx`）翻转为 `kind = "module"` + 目录型 `path`（`./packages/desktop/src/components/agent/`），其入口由入口解析链的 ③ 命中 `<root>/index.tsx`。MUST NOT 为其追加显式 `entry` 字段。

```text
before : kind = "domain" · path = ["./packages/desktop/src/components/agent/index.tsx"]
after  : kind = "module" · path = ["./packages/desktop/src/components/agent/"]
```

- **不加 `entry` 的理由**：`path` 变目录后 `entry_candidates` ③ 的 `<root>/index.tsx` 当场命中、`entry_state` 即 `Resolved`；而 `repo-archi-model` 的字段纪律是「仅在约定默认指不到时声明」且「本基线内显式 `entry` 恰 3 处」——加第四行即违纪律并使该能力的 `字段纪律可机械审查` Scenario 判红。
- **计数后果**：前端元素 **30 不变** ⇒ **3 个容器**（`Views` / `Components` / `Standard`）+ **27 个叶**（`Shell` 1 + 9 视图 + 3 子域 + 12 共享组件 + `Components.Agent` + `Ui` 1）；全库 **9 容器 + 44 叶**（53 不变）；关系 **71 条逐条不变**。
- **7 条端点边逐条在案**：3 条出边（入口 `index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`）+ 4 条入边（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）。归属由 `OwnerIndex` 的最长前缀给出：7 个子元素的文件型 `path` 优先于父的目录型 `path`，入口 `index.tsx` 仍归父。
- **裁定记录同步**：`decisions/0001-model-granularity.md` §3 的「容器 `path` 恒空，唯一例外是 barrel」SHALL 重写为「容器 `path` 恒空（零例外）；barrel 是目录模块的入口，不是容器的特权」，§1 / §2 / §9 的容器 / 叶 / 子域计数与 ui 文件数同批重测。

#### Scenario: 翻转后入口由约定默认命中

- **WHEN** 对翻转后的 `App.Frontend.Components.Agent` 跑 `paths::entry_state`
- **THEN** `Resolved`、入口 = `packages/desktop/src/components/agent/index.tsx`（③ 的第四候选）；模型内该元素零 `entry` 行；`grep -c "^entry"` 于 `models/` 仍为 **3**

#### Scenario: 7 条端点边逐条不变

- **WHEN** 统计翻转前后以 `App.Frontend.Components.Agent` 为任一端点的前端关系
- **THEN** 两次均恰 **7** 条：出边 `…AgentInput` / `…AgentSessionView` / `…SessionPanel`，入边 `Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`；前端关系总数 71 不变，`71 = 64 + 7` 的恒等式仍成立

#### Scenario: 计数换段无溢出

- **WHEN** 聚合 `models/` 六个文件
- **THEN** 元素 **53**（容器 **9** + 叶 **44**）、关系 **120**（后端 49 + 前端 71）、`06` 段 **30 元素 / 71 关系**（3 容器 + 27 叶）；`diagnostics` 空、`warnings` 空；表头注释、`repo-archi-model` 计数表与 ADR `§9` 三处同值

#### Scenario: 翻转不外溢到代码层

- **WHEN** 对比翻转前后的 `packages/desktop/src/components/agent/**`
- **THEN** 逐字节零 diff（既有 `index.tsx` 的 6 条 re-export 与 7 个实现文件均不改动）；翻转只发生在模型数据与裁定记录，MUST NOT 借机重排 barrel 或改导出名

## Module Contract

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `packages/desktop/src/views/{agent,agents,architecture,config,db,info}/index.ts` | 6 个单文件目录的入口 | 各 1 条具名 re-export（视图组件）；`*Props` 不外露；MUST NOT 承载实现 |
| `packages/desktop/src/views/changes/index.ts` | 变更页目录入口 | `ChangeDetailView` / `ChangeListView`；只导本元素直属文件 |
| `packages/desktop/src/views/changes/agent/index.ts` | 变更会话子域入口 | `ChangeChatPanel`（`composer` 与两个 `*Props` 不外露） |
| `packages/desktop/src/views/changes/flow/index.ts` | 流程子域入口（压力样本） | 8 值 + 3 类型共 **11** 名；目录内约 38 个公开符号只出这 11 个；类型出口一律 `export type` |
| `packages/desktop/src/views/changes/renderers/index.ts` | 产物渲染子域入口 | `ArtifactTabs` / `MarkdownDocRenderer` |
| `packages/desktop/src/views/explores/index.ts` | 探索页目录入口 | `ExploreDetailView` / `ExploreLayout` / `ExploreListView`（`ExploreDetailPane` / `ExploreRouteContext` / `useExploreRouteContext` 不外露） |
| `packages/desktop/src/components/ui/index.ts` | 原语层入口（唯一例外） | **40** 条 `export *`；零 `export default`；首行外部约束注释；调用点 132 处保持直引；在 `knip` / `NO_STATIC_CHECK` ignore 内 |
| `packages/desktop/src/components/agent/index.tsx` | 既有入口（零改动） | 6 条 re-export（7 名）；`.tsx` 扩展名保留；`components/agent/` 目录成为 `App.Frontend.Components.Agent` 的 `path` |
| `packages/desktop/src/routes.tsx` | 路由表（调用点收窄） | 11 条文件级 import → 8 条目录级；路由路径 / 参数 / 重定向零改动 |
| `packages/desktop/src/module-entries.test.ts`（新增） | 入口面的机械判据 | 12 个入口的 `export … from` 行集合逐字断言（类型出口必须 `export type`）、specifier 零 `./hooks` / `./lib` / 跨子目录段、ui 恰 40 条 `export *` 且零 `export default`、零重名；纯源文本读取，零运行期 import |

### Function: 无

本能力不新增函数或命令面（前端源码结构约束，零 IPC、零 CLI、零 Rust 改动）。

### API: 无

本能力不新增 `api` 契约资产：`api` 缺省 `declared`、12 个目录元素零契约文件，`desktop-archi-api` 的 `.i.ts` 生成链与 `ts_import_specifier` 留白逐字保留。

### CLI: 无

判据面由既有 `vp check`（`knip` + typeCheck）、`api:check` 与架构检查命令承担，本能力 MUST NOT 新增脚本。

### Component: 无

本能力不新增 React 组件；`components/ui` 与 12 个目录内的既有组件逐字不变。
