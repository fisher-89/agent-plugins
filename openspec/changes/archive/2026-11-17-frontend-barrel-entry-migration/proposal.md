# 提案: frontend-barrel-entry-migration

> **变更**: frontend-barrel-entry-migration（前端目录模块入口化 · 12 个 barrel + 调用点收窄 + `Components.Agent` 容器翻转）
> **日期**: 2026-11-17
> **状态**: draft
> **来源**: `openspec/changes/frontend-barrel-entry-migration/explore.md`（B.0–B.7 全篇，含两轮 re-explore；`B.6` 已被 `B.7` 取代，只作推导留痕）
> **性质**: **前端模块入口补件**——12 个前端目录各补一个 `index.ts(x)` barrel、调用点由文件级收窄到目录级、`App.Frontend.Components.Agent` 由「容器 path 唯一例外」翻转为目录型 `module`。源码面只增 12 个 barrel 文件与 4 处调用点改写；模型面只改**一个元素**的 `kind` / `path`，元素数与关系数**不变**（53 / 120、`06` 段 30 / 71）；判据面读数**净 0 全平移**（`apiEntryMissing` 12 → 0、`apiMissing` 29 → 41）。
> **不含**: 任何前端契约资产（`.md` / `.schema.json` / `index.i.ts`）、`paths::ts_import_specifier` 接线、`api = none` 标注、`components/ui` 132 处调用点改写、5 个零引用 ui 模块的清理、`repo-archi-model` `§8` 两条前端缺口与 11 条边面缺口的补齐、后端元素粒度与 `entry` 数据面改动。

---

## 问题

### 现状链路

```text
12 个前端目录根元素（Components.Ui + Views.{AgentDebug,Agents,Architecture,Changes,
Changes.Agent,Changes.Flow,Changes.Renderers,Config,DbInspector,Explores,Info}）
  ├─ path = 目录（如 ./packages/desktop/src/views/changes/）
  ├─ 目录内零 index.ts / index.tsx
  └─ entry_state ⇒ ② 不适用（目录型根）· ③ 四候选全不落盘 ⇒ Absent ⇒ apiEntryMissing（warn）
```

「目录 = 模块」在前端因此只有**一半机械载体**：元素粒度已按目录收敛（`repo-archi-model`「前端下钻到视图与组件层的元素覆盖」），但目录本身没有入口文件，跨元素消费只能穿透到文件级（`routes.tsx` 11 条 import 打到 8 个视图目录、`change-detail-view.tsx` 8 条打到 2 个子域），于是「谁能依赖谁」的边界只剩元素表在说、代码里看不见。

### 病灶一 · 12 条 `apiEntryMissing` 是唯一的固定段，无法推进

契约面启用后全量 44 个 `path` 非空元素纳管，读数恒为 `apiEntryMissing 12 + apiMissing 29 = 41`。① 面那 12 条**只能**由「补入口文件」或「标 `api = none`」消解：前者是入口债的真实内容，后者是对「无公共 API」的伪造（`Components.Ui` / 9 个视图都有实质公共面）。故 ① 面不清，契约面的推进顺序就永远卡在第一段。

### 病灶二 · barrel 的真实语义与容器语义混着

`App.Frontend.Components.Agent` 今天是 `kind = "domain"` + `path = ["./packages/desktop/src/components/agent/index.tsx"]`——ADR 0001 §3 的「容器 `path` 恒空，唯一例外是 barrel」。这个例外在「目录 = 模块」的口径下是**语义错位**：它持 `path` 的理由是「持有一个 barrel」，而 barrel 本应是**目录模块的入口**，不是容器的特权。12 个目录补 barrel 后，同一形态会在 12 处出现，「容器例外」这条判据的解释力当场归零。

### 病灶三 · 前端计数在 spec / ADR 里已失真

| 位置 | 现值写法 | 实测 |
|---|---|---|
| `desktop-archi-api`「生成物落点与入口查找」Scenario | `Resolved` 30 / `Absent` 12 / 分母 42 | **32 / 12 / 44**（「12 个前端单文件根」实为 **14**） |
| ADR §2 子域计数示例 | `flow` 13 / `renderers` 7 / `agent` 3 | **15 / 7 / 3**（含 `hooks/` 口径） |
| ADR §2 / `repo-archi-model`「覆盖判据」 | `components/ui` 的 **35** 个文件 | **39** |
| `repo-archi-model`「视图与子域覆盖」Scenario | `flow`（13）、`agent`（3）、`views/changes/components/`（1 文件） | **15 / 3 / 2** |

这些数字**不修也会漂**，但本变更会**动它们的分母**（barrel 是非测试源），故必须同批重测写死，否则等于把假话再钉一次。

### 现状实测（本轮复核，逐条在案）

| 事实 | 证据 |
|---|---|
| 12 个目标目录 `barrel = 0` | 逐个 `ls`：仅 `components/agent/index.tsx` 一个既有 barrel |
| 取数面已能读 barrel | `api/extract/mod.rs` 的闭包 + `ts.rs` 三态分派（`ts-export-closure-extraction` 已归档） |
| `entry` 字段与三态解析链在案 | `paths::entry_candidates` ③ 含 `<root>/index.ts` / `<root>/index.tsx` |
| 基线 | `apiEntryMissing 12` + `apiMissing 29` = 41，`status = clean`、`passed = true` |
| 调用点扇出 | `routes.tsx` **11** 条 → 目录 9 个；`change-detail-view.tsx` **8** 条；`detail-drawer.tsx` 1；`explore-preview.tsx` 1 ⇒ 合计 **21** |
| `@/components/ui/<mod>` | **132** 处 / **67** 文件 / **35** 模块（另 1 处 `@/components/ui/lib` 是目录不是模块，合计 133 行命中） |
| ui 零引用模块 | 5 个：`button-group` / `context-menu` / `input-group` / `marker` / `sheet` |
| ui 导出面 | 39 个源文件、46 行 `export`、零 `export default`、零同名 |
| `flow` 公开符号 | 约 38 个（46 行 `export`），跨元素实际取用 **11** 个 |

---

## 提案

### 一 · 12 个目录各补一个 barrel（形态统一，单文件目录也补）

| 组 | 目录 | 入口文件 | 导出名字（逐条手挑） |
|---|---|---|---|
| 多文件 | `views/changes` | `index.ts` | `ChangeDetailView` / `ChangeListView` |
| | `views/changes/agent` | `index.ts` | `ChangeChatPanel` |
| | `views/changes/flow` | `index.ts` | 8 值（`mountMaterials` / `ChangeFlowGraph` / `DetailDrawer` / `buildFlowGraph` / `RunControlPanel` / `runStepNodes` / `unifiedRunEdges` / `unifiedRunNodes`）+ 3 类型（`DrawerSelection` / `FlowGraph` / `FlowMaterials`） |
| | `views/changes/renderers` | `index.ts` | `ArtifactTabs` / `MarkdownDocRenderer` |
| | `views/explores` | `index.ts` | `ExploreDetailView` / `ExploreLayout` / `ExploreListView` |
| | `components/ui` | `index.ts` | **39 条 `export *`**（唯一例外，见提案二） |
| 单文件 | `views/{agent,agents,architecture,config,db,info}` | 各 `index.ts` | 各 1 个视图组件（`props` 类型不外露） |

- **`index.ts` 而非 `.tsx`**：12 个 barrel 的正文全是 re-export、零 JSX；既有 `components/agent/index.tsx` **保持原扩展名不动**（③ 的两条候选都认）。
- **单文件目录也补**（已裁，`B.1.1`）：多文件目录靠 barrel、单文件目录靠「唯一文件即入口」＝同仓两套约定，而单文件目录多一个 `helper.ts` 就静默换默认入口。统一形态才守得住「目录 = 模块」。
- **`export type` 分割**：`isolatedModules = true` ⇒ 类型出口一律 `export type { … } from './x'`，值出口用 `export { … } from './x'`，同一来源文件的两类出口拆成两行（与 `components/agent/index.tsx` 既有形态逐字同族）。

### 二 · 入口 = 公共面（这条判据决定了逐条手挑，而不是 `export *`）

- **导出面 = 跨元素实际被 import 的名字**。多导一个 → `knip` 判 unused export（`views/*/index.ts` 在扫描面内）；少导一个 → 调用点编译红。于是「入口 = 公共面」从散文变成**机械可验的事实**，这是本变更最实在的副产物。
- **`flow` 是这条纪律的压力样本**：目录 38 个公开符号，barrel 只出 **11** 个。每次扩公共面都得显式改 barrel，而不是顺手 `export *`。
- **MUST NOT 跨子元素 re-export**（比「禁 hooks/lib」更宽的一条）：`reexport_ok` 的判据是 FQN **前缀**，子元素也在放行面内；而 `scan/ts.rs` 把 `export … from` 记 import fact ⇒ 跨子元素 re-export 会**真加边**（`Views.Changes → Views.Changes.Flow` 一类），前端 71 条就不再是不变量。故每个 barrel 只 re-export **本元素直属文件**。
- **MUST NOT re-export `hooks/` 与 `lib/`**：它们的 owner 就是所属视图（目录归属），机械判据拦不住，只能靠纪律——把「非公共面的内部实现」写进入口，等于让「入口 = 公共面」从第一天起失真。
- **`components/ui` 是唯一例外**：39 条 `export *`（不手维护 34 名单），132 处调用点**保持直引不改**。理由是 ui 整层在 `knip.json` 的 `ignore` 与 `vite.config.ts` 的 `NO_STATIC_CHECK` 内（零校验），若再手维护一份 34 条白名单，一旦某个模块开始被引用就要同步改 barrel，而没有任何机制会提醒；`export *` 让「目录 = 模块」自洽，代价是 5 个零引用模块（`button-group` / `context-menu` / `input-group` / `marker` / `sheet`）随 `export *` 进入公共面——**这 5 条登记为已知缺口**，清理属独立变更。
- `components/ui/index.ts` 首行留一行注释（外部约束型，符合 AGENT.md 注释纪律）：本文件是目录模块的入口声明，视图内直引模块名是刻意选择，勿据此判死代码。

### 三 · 调用点收窄（barrel 必须真被用，否则是死文件）

| 文件 | 现状 | barrel 化后 |
|---|---|---|
| `src/routes.tsx` | 11 条文件级 import（打向 8 个视图目录，`explores` 占 3 条、`changes` 占 2 条） | **8** 条（8 个视图目录各 1 条） |
| `views/changes/change-detail-view.tsx` | `./flow/*` 7 条 + `./agent/*` 1 条 | **2** 条（`./flow` / `./agent`） |
| `views/changes/flow/detail-drawer.tsx` | `../renderers/artifact-tabs` | `../renderers` |
| `views/explores/components/explore-preview.tsx` | `../../changes/renderers/markdown-doc-renderer` | `../../changes/renderers` |
| 合计 | **21 条** | **12 条** |

（`components/ui` 的 132 处**不在**改写面内，见提案二。）

**边集是不变量**：目录 specifier 由 `scan::ts::TsResolver::resolve` 的 index 折叠解析到 `<dir>/index.ts`，其 owner 与改动前被打到的文件**同属一个元素** ⇒ 71 条前端边逐条相等；barrel 自身的 re-export 目标是本元素直属文件 ⇒ 自边在 `scan` 里被丢弃 ⇒ 零新边、零计数变化。

### 四 · `App.Frontend.Components.Agent` 由容器翻转为目录模块

```text
today : kind = "domain" · path = ["./packages/desktop/src/components/agent/index.tsx"]
after : kind = "module" · path = ["./packages/desktop/src/components/agent/"]
```

- **MUST NOT 顺手加显式 `entry`**（与 explore `B.1.6` 的草案相反）：`path` 变目录后 `entry_candidates` ③ 的 `<root>/index.tsx` 当场命中，`entry_state` 即 `Resolved`。而 `repo-archi-model` 的字段纪律是「**仅在约定默认指不到时**声明」且「本基线内显式 `entry` 恰 3 处」——加第四行即违纪律、并使该能力的机械 Scenario 判红。explore 那行 `entry = …` 属**多余而非错**，落地按 ③ 约定默认。
- **连带计数**：前端元素 30 不变 → **3 容器 + 27 叶**（`Components.Agent` 由第 4 个容器变为第 27 个叶）；全库 **9 容器 + 44 叶**（53 不变）；关系 **71 条逐条不变**（3 出 4 入共 7 条 barrel 端点边全在案；`owner_of` 用最长前缀，7 个子元素的文件型 `path` 优先于父的目录型 `path`，barrel `index.tsx` 归父）。
- **ADR 0001 §3 重写**：把「容器 `path` 恒空，唯一例外是 barrel」改为「容器 `path` 恒空（零例外）；barrel 是目录模块的入口，不是容器的特权」。§1 / §2 / §9 的容器 / 叶 / 子域计数与 ui 文件数同批重测。

### 五 · `api` 口径：留空（缺省 `declared`）

补完 barrel 后判据面**不清零，只换面**：`apiEntryMissing 12 → 0`、`apiMissing 29 → 41`（12 条原样搬来）——`api` 缺省即 `declared`，「有入口、无契约」正是 ② 的定义。三种收口里取**留空**：

| 收口 | 结果 | 判据 |
|---|---|---|
| **留空（本变更）** | `0 + 41 = 41`，`status = clean`、`passed = true` | 零门禁代价：纳管子集要求「契约文件在案」，12 个前端元素零契约 ⇒ 落选 ⇒ `status` 不进 `warnings`；「前端 op 面留白」是 `desktop-archi-api` 已写死的事实（解构 props 抽不出字段） |
| 标 `api = none` | 41 → 29 | **伪造**「无公共 API」：9 个视图与 ui 都有实质公共面，`export interface` / 组件函数俱在 |
| 写 12 份 `api-types` 契约 | 41 → 29 且为真 | 首次触发 `.i.ts` 生成链 ⇒ 须先接 `paths::ts_import_specifier`（`desktop-archi-api` 明示「待接线留白」），工程量高一个数量级，且要逐目录人工挑公共面 |

代价落在 **spec 而非门禁**：`desktop-archi-api-check` 的基线表换第五段 `0 + 41 = 41`，并须点明这一段的性质是「**净 0、全平移**」——与 A 数据面那一步「真正收了两条债」正相反，否则下一个人会读成「债务清零」。

### 六 · 版本纪律

`packages/desktop/package.json` 的 `version` 相对**落地时 `HEAD` 现值 +1 patch**（起草时现值 `0.6.49`）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随（文件零内容改动）；`src-tauri/Cargo.toml` 的 `[package] version` **MUST NOT** 随动。判据：本变更改 `packages/desktop/src/**` 源码 ⇒ 属 AGENT.md 的「desktop source change」，+1 patch 无例外。

### 七 · 明确不做（留白清单）

| 不做 | 理由 / 归属 |
|---|---|
| 前端契约资产（`<FQN>.md` / `.schema.json` / `index.i.ts`） | 提案五：op 面在前端不可表达；写契约须先接 `.i.ts` 生成链 |
| `paths::ts_import_specifier` 接线 | 无 `.i.ts` 实例即无观测点，留白逐字保留 |
| 标 `api = none` | 提案五：伪造「无公共 API」 |
| `components/ui` 的 132 处调用点改写 | 已裁：barrel 只作入口声明 + 契约落点；ui 在 `knip` ignore 内，改写不带来可验收益而 diff 巨大 |
| 删 5 个零引用 ui 模块（`button-group` / `context-menu` / `input-group` / `marker` / `sheet`） | 可能是 shadcn 预留；清理属独立变更（须连带决定 `export *` 面） |
| 把 `src/components/ui/**` 移出 `knip.json` 的 `ignore` | 那是打开死代码判据，会让 5 个死模块 + 未用导出立刻全红，属独立清理 |
| `hooks/` / `lib/` 的入口或元素 | 非公共面；建入口即让「入口 = 公共面」失真，建元素即改粒度档 |
| 后端元素粒度、`entry` 数据面、14 个 `src/lib.rs` crate 的入口 | 本变更零触及 |
| `repo-archi-model` `§8` 的 11 条边面缺口与 `views/architecture/agent/` 元素缺口 | 既有事实、与 barrel 无因果；补齐须另开 change（`§8` 逐条保留） |
| 扩「`views/*/components/` 不构成子域」的枚举面 | 扩面即改判据；本变更只改数、并把「排除条对 `views/*/` 全体生效」写成纪律文本 |
| `desktop-archi-module-docs`（入口文档语义） | 与「入口文件」正交，可并行不阻塞 |

---

## 能力

### 新增能力

- **`desktop-frontend-module-entry`** — 前端目录模块入口（barrel）的能力面：12 个目录的入口落位与命名、入口 = 公共面的导出面纪律（逐条手挑 / 禁 `export *` 除 ui / 禁跨子元素与 `hooks`·`lib` re-export / `export type` 分割）、`components/ui` 的唯一例外、调用点经入口收窄、`Components.Agent` 由容器翻转为目录模块。

### 修改的能力

- **`repo-archi-model`** — 3 条 **MODIFIED**：
  - `基线落位与模型文件划分`：`06` 段容器 **4 → 3**、叶 **26 → 27**（30 元素不变）；容器 `path` 由「唯一例外」改为**零例外**；`entry` 恰 3 处**不变**（`Components.Agent` 走 ③ 默认，不入显式面）；「无入口 = 12 个前端目录根」改为「无入口集合为**空**」；`计数复述单源` 补一句「目录模块入口不改变计数」；对应 3 个 Scenario 的数字与点名同批换段。
  - `前端下钻到视图与组件层的元素覆盖`：容器清单去掉 `Components.Agent`（改列目录型 `module` 叶）；ui 文件数 **35 → 39**；子域计数 **flow 15 → 16 / renderers 7 → 8 / agent 3 → 4**（含 `hooks/` 口径）、`views/changes/components/` **1 → 2**；「排除条对 `views/*/` 全体生效」写成纪律文本；`§8` 两条前端缺口逐条保留。
  - `前端关系只取 import 证据`：「容器作端点仅限 barrel 一例」改为「目录模块作端点 = 12 个 barrel 的对内 re-export（自边丢弃）」+「`Components.Agent` 的 7 条端点边逐条不变」；对应 Scenario 的「容器端点」表述同步。
- **`desktop-archi-api`** — 2 条 **MODIFIED**：
  - `生成物落点与入口查找`：本仓入口面实测 30 / 12 / 42 → **44 Resolved / 0 Absent / 0 Invalid（分母 44）**；「无入口是合法终态」的实例改为「本仓现基线无一例」（`Absent` 仍是合法终态，本仓只是当前为空）；「不强制模块成目录」保留（② 仍让单文件根零改动），删掉「无需补 barrel」的措辞。
  - `TS 导出闭包的取数面`：`同元素约束` 里「文件型 `path` 的元素（`components/agent/index.tsx` 一类）因此不需要任何例外分支」改为以目录型 `path` 的 barrel 元素为例（结论不变：owner 子树放行）；`本仓 Components.Agent 取数面恰七名` Scenario 的 `path` 形态改为目录。
- **`desktop-archi-api-check`** — 4 条 **MODIFIED**：
  - `契约判据的验证语料与报告数基线`：基线表换 **第五段 `0 + 41 = 41`**（分母仍 44）；历史链由四段扩五段并点明本段「净 0、全平移」；逐条点名的 Scenario 由「12 条 `apiEntryMissing`」改为「`apiMissing` 41 条、`apiEntryMissing` 0 条」。
  - `判据级档位与元素 gate 解耦`：「MUST NOT 顺带为消 `apiEntryMissing` 而补 12 个前端目录的 barrel」改为「barrel 已由独立变更 `frontend-barrel-entry-migration` 落地；本条纪律不改——判据面 MUST NOT 要求建入口 / 改代码分层」；对应 Scenario 的「12 个无可用入口元素」实例换成合成语料（本仓现基线为 0）。
  - `报告并列槽位 apiFindings`：「存量 backlog 不拉黄灯」Scenario 的举例由「12 条 `apiEntryMissing` + 29 条 `apiMissing`」改为「41 条 `apiMissing`」——纳管结论逐字不变（12 条由 ① 面搬来不改变纳管结论）。
  - `十条判据的定义与求值互斥顺序`：「唯一持 barrel 的容器 `App.Frontend.Components.Agent` 有 `path`，故纳入」改为「持 barrel 的目录型 `module` 有 `path`，故纳入——判据只读 `path` 是否非空，不读 `kind`」。
- **`desktop-archi-file-tree`** — 1 条 **MODIFIED**（`归属判定与判定单源`）：「元素根判据是 `path` 非空而非 `kind`」的实例句更新——原 `Components.Agent` 持 barrel 文件型 `path` 的形态已翻转为目录型 `module`，本仓现基线无「`domain` 但 `path` 非空」实例；判据本身（只读 `path`）逐字不变。

| 不涉及 | 理由 |
|---|---|
| `desktop-archi-model` | 字段面零改动（`kind` / `path` / `entry` 语义与解析零变化） |
| `desktop-archi-drift` / `desktop-archi-gate` | 报告结构、四组词汇、`status` 推导、纳管子集、`REPORT_VERSION = 3` 零改动；`apiFindings` 总数不变（41） |
| `desktop-archi-contract-panel` | 判据轴按 `category` 分组呈现同一份报告；`apiEntryMissing` 12 → 0、`apiMissing` 29 → 41 是数据变化，不是呈现规则变化（「判据 41 条」的示例读数不变） |
| `desktop-archi-module-docs` | 入口文档语义归独立的 E 单元 |
| `desktop-page-routing` | 路由表内容（路径 / 参数 / 重定向）零改动，只改 import 形态 |
| `desktop-standard-components` | `components/standard/**` 零改动；ui 是其底座，只增入口声明 |
| `desktop-ipc-type-bindings` | 零 `dto` 改动 ⇒ 绑定零再生成 |
| `desktop-archi-file-tree` 的其余 requirement | 判据实现零改动（只改一处叙述实例） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src/views/agent/index.ts` / `agents` / `architecture` / `config` / `db` / `info` | 各新增 1 个 barrel，各 1 条具名 re-export（视图组件；`*Props` 不外露） |
| `packages/desktop/src/views/changes/index.ts` | 新增：`ChangeDetailView` / `ChangeListView` |
| `packages/desktop/src/views/changes/agent/index.ts` | 新增：`ChangeChatPanel` |
| `packages/desktop/src/views/changes/flow/index.ts` | 新增：8 值 + 3 类型（共 11 名，目录内 38 个公开符号只出这 11 个） |
| `packages/desktop/src/views/changes/renderers/index.ts` | 新增：`ArtifactTabs` / `MarkdownDocRenderer` |
| `packages/desktop/src/views/explores/index.ts` | 新增：`ExploreDetailView` / `ExploreLayout` / `ExploreListView`（`ExploreDetailPane` / `ExploreRouteContext` / `useExploreRouteContext` **不外露**） |
| `packages/desktop/src/components/ui/index.ts` | 新增：39 条 `export *` + 一行外部约束注释 |
| `packages/desktop/src/routes.tsx` | 11 条文件级 import → 8 条目录级 import |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `./flow/*` 7 条 + `./agent/*` 1 条 → 2 条（`./flow` / `./agent`） |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | `../renderers/artifact-tabs` → `../renderers` |
| `packages/desktop/src/views/explores/components/explore-preview.tsx` | `../../changes/renderers/markdown-doc-renderer` → `../../changes/renderers` |
| `.dev-team/architecture/models/06-frontend.toml` | `App.Frontend.Components.Agent` 的 `kind` `domain` → `module`、`path` 文件 → 目录；表头注释「另有 4 个容器」→「3 个容器」；**元素 / 关系逐条不动**（30 / 71） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | §1（容器 10 → 9、叶 43 → 44）、§2（子域计数 `flow` **16** / `renderers` **8** / `agent` **4**（含 `hooks/` 口径，即 15 / 7 / 3 → 16 / 8 / 4）、ui **39** 文件、`views/changes/components` **2** 文件）、§3（例外段重写为「barrel = 目录模块的入口」）、§9（复算表同值） |
| `packages/desktop/package.json` | `version` +1 patch（相对落地时 `HEAD` 现值） |

### 测试文件

| 对象 | 判断 |
|---|---|
| `packages/desktop/src/module-entries.test.ts` | **新增**（node 环境、纯源文本断言，无运行期 import）：12 个 barrel 的 `export … from` 行集合逐字等于提案一的名单（类型出口必须是 `export type`）；每个 barrel 的 specifier 零 `./hooks` / `./lib` / 跨子目录段；`components/ui/index.ts` 恰 39 条 `export *` 且零 `export default`。这是 ui（在 `knip` ignore 内）唯一的机械判据 |
| `packages/desktop/src/components/agent/index.test.tsx` | **零改动**（既有 barrel 的出口集合与「消费点经 barrel 取 `AskAnswerActions`」自证逐字保留；本变更不改 `change-detail-view.tsx` 的该行 import） |
| 其余既有测试 | 零改动：测试一律直接 import 实现文件，不经 barrel（`knip` 的 `project` glob 排除测试 ⇒ 不走 barrel 也就不会伪装成「公共面使用者」） |
| 验收留痕（`reports/`） | 四份实测：① 12 个目录的 `entry_state` 全 `Resolved`（本仓 44 / 44 / 0）；② 改动前后 `relations` 逐条 diff（前端 71 / 24 源不变、`unresolved` / `unmatchedFiles` 零新增）；③ 一次全量检查的 `apiFindings` 分桶（`0 + 41 = 41`、`status = clean`、`passed = true`）；④ `api:check` 三列空 + `bindings:export` 幂等。**MUST 注明所用口径（元素级 / 全量）**，避免后人把读数差异当回归 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `packages/desktop/src/components/agent/**` 的既有实现与 `index.tsx` | 已有 barrel 且在被使用；翻转只改**模型**，不改代码 |
| 12 个目标目录内的 39 + 40 个既有实现文件 | barrel 是**新增文件**，实现内容零改动（含 `flow` 的 38 个公开符号：不外露不等于删） |
| `packages/desktop/knip.json` / `vite.config.ts` | ui 与 `src/**/*.i.ts` 已在 `ignore` / `NO_STATIC_CHECK` 内，本变更不打开也不新增判据 |
| `packages/desktop/src/types/generated/**` | 零 `dto` 改动 ⇒ 绑定零再生成；出现 diff 即误动 |
| `.dev-team/architecture/api/**` | 零契约改动：新增 barrel 不使任何元素「契约在案」（`INDEX.toml` 的 `generated` 列在有契约时才填，故逐字不变）；`api:check` 三列必须为空 |
| `packages/desktop/src-tauri/**` | 后端零改动（`entry_candidates` ③ 早已含 `index.ts` / `index.tsx`，取数面闭包已支持 re-export） |
| `openspec/specs/**` 除本 change 落 delta 的 5 个能力 | 出现 diff 即误改 |
| `explore.md` | 只读：探索结论以本提案的实测数字为准 |

---

## 验收标准

1. **AC-1 · 12 个入口在案且解析面归零**：`ls` 复核 12 个目录各有且仅有一个 `index.ts(x)`（`components/agent/` 不在 12 个之列，其既有 `index.tsx` 逐字不动），每个入口文件落在其元素 `path` 之内；对本仓 44 个 `path` 非空元素跑 `entry_state`：**`Resolved` 44 / `Absent` 0 / `Invalid` 0**（`App.Frontend.Components.Agent` 由 ③ 命中 `components/agent/index.tsx`，模型**零显式 `entry` 加行**）。
2. **AC-2 · 边集逐条相等（barrel 是边中立的）**：改动前后各跑一次同源检查（`archi_drift` 命令与门禁 runner 是同一实现），前端 `relations` 的 `from` / `to` / `desc` 三列 diff 为空、**71 条 / 24 个源**不变、`unresolved` 与 `scan.unmatchedFiles` 零新增；反向判据：任一 barrel 若 re-export 了跨子元素目标（如 `views/changes/index.ts` 顺手导 `flow` 的名字），边数即 > 71 判红——该形态 MUST NOT 以「knip 绿」为由放过。
3. **AC-3 · 入口 = 公共面（knip 闭合）**：`vp check`（含 `knip`）零 dead code 报告；`views/*/index.ts` 的每个导出名都能在**非 barrel、非测试**的调用点找到 `import`；反向判据：给任一 barrel 多导一个名字 ⇒ `knip` 报该 export unused（判红留痕落 `reports/`）。`components/ui/index.ts` 例外（`ignore` 内），其导出面由 `module-entries.test.ts` 的源文本断言守。
4. **AC-4 · 调用点收窄实测**：`routes.tsx` 11 → **8**、`change-detail-view.tsx` 8 → **2**、`detail-drawer.tsx` 1 → **1**、`explore-preview.tsx` 1 → **1**，合计 21 → **12**；12 个目录（`components/ui` 除外）零残留 `from '…/views/<dir>/<file>'` 形态的文件级跨元素 import（`grep` 判据）；`@/components/ui/<mod>` 仍为 **132 处**（刻意不改，逐条在案）。
5. **AC-5 · 判据面换段而非清零**：一次全量检查的 `apiFindings`：`apiEntryMissing` **0**、`apiMissing` **41**（12 条由 ① 面平移而来，逐条点名 8 个视图目录根 + 3 个视图内子域 + `App.Frontend.Components.Ui`）、其余七条各 0、`apiUndeclared` 0；总数 **41** 与改动前相等；`status = clean`、`passed = true`（纳管子集仍为空 —— 12 个元素零契约文件）；`api:check` 的 `missing` / `mismatched` / `orphaned` 三列皆空，`INDEX.toml` 零 diff。
6. **AC-6 · 模型与计数换段无溢出**：`archi_graph(root)` 正常态：元素 **53** / 关系 **120** / `06` 段 **30 / 71** / `diagnostics` 空 / `warnings` 空；元素面 **9 容器 + 44 叶**（`06` 段 3 容器 + 27 叶）；`App.Frontend.Components.Agent` 的 `kind = "module"`、`path` 为目录、`entry` 零行；其 **7 条**端点边逐条在案（3 出 `AgentInput` / `AgentSessionView` / `SessionPanel`，4 入 `Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`）；`grep -c "^entry"` 仍为 **3**、`grep -c 'api = "none"'` 仍为 **2**。
7. **AC-7 · 子域判据可复算**：按下述命令枚举 `views/` 内命中「递归非测试源 ≥ 3」的子目录，输出 MUST 恰为四条：`3 ./architecture/agent`、`4 ./changes/agent`、`16 ./changes/flow`、`8 ./changes/renderers`；`components/` 下任何目录 MUST NOT 出现（排除条对 `views/*/` 全体生效）：

   ```bash
   cd packages/desktop/src/views
   for d in $(find . -mindepth 2 -type d \( -path './*/components' -o -path './*/hooks' -o -path './*/lib' \) \
                -prune -o -mindepth 2 -type d -print | sort); do
     n=$(find "$d" -type f \( -name '*.ts' -o -name '*.tsx' \) ! -name '*.test.ts' ! -name '*.test.tsx' | wc -l)
     [ "$n" -ge 3 ] && echo "$n  $d"
   done
   ```

8. **AC-8 · 零扩散**：`git status --porcelain` 的改动集 ⊆ {本 change 目录、12 个新增 barrel、`routes.tsx`、`change-detail-view.tsx`、`detail-drawer.tsx`、`explore-preview.tsx`、`module-entries.test.ts`、`06-frontend.toml`、`0001-model-granularity.md`、`packages/desktop/package.json`}；`packages/desktop/src-tauri/**` 与 `.dev-team/architecture/api/**` 零 diff；`bindings:export` 幂等。
9. **AC-9 · 版本纪律**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 现值恰 +1 patch（只加一次）；`tauri.conf.json` 仍为 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version` 逐字不变。
10. **AC-10 · 相关能力零漂移**：`openspec/specs/` 下除 `desktop-frontend-module-entry`（新增）、`repo-archi-model`、`desktop-archi-api`、`desktop-archi-api-check`、`desktop-archi-file-tree` 五个能力外零 diff；`desktop-archi-model` / `desktop-archi-drift` / `desktop-archi-gate` / `desktop-archi-contract-panel` / `desktop-archi-module-docs` / `desktop-page-routing` / `desktop-standard-components` / `desktop-ipc-type-bindings` 出现 diff 即误改。

> **不列为 AC 的流程项**：判据十值词汇、互斥求值序、纳管子集、`REPORT_VERSION`、`check` 链四段、`entry_candidates` 四候选——各有既有 spec 落点且本变更零改动，不另设无 spec 落点的断言。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **ui 的 barrel 零调用者 + 零校验 ⇒ 静默腐化**：它在 `knip` `ignore` 与 `NO_STATIC_CHECK` 内，改名 / 删模块都不会有人报 | 三重对冲：① `export * × 39` 使 barrel 无需维护（不手抄名单）；② 首行留外部约束注释（勿据此判死代码）；③ `module-entries.test.ts` 的源文本断言把「39 条 `export *`、零 default」变成机械判据。**残留缺口**：`export *` 的语义等价性（模块新增 / 删除）无断言，属已知缺口 |
| R-2 | **5 个零引用模块随 `export *` 进入公共面**：`Ui` 的导出集含 `button-group` / `context-menu` / `input-group` / `marker` / `sheet` | 登记为已知缺口（spec 内单列），清理属独立变更；`apiUndeclared` 仍 `off` ⇒ 不产 finding，但 MUST NOT 据此宣称「ui 公共面等于真实使用面」 |
| R-3 | **「判据 41 条不变」被读成「债务没变」** | `desktop-archi-api-check` 第五段 MUST 明写「净 0、全平移」，并逐条点名 12 条从 ① 面搬到 ② 面；报告留痕三桶并列（改前 / 改后 / 差） |
| R-4 | **目录 specifier 的解析依赖 `TsResolver` 的 index 折叠**：若 `oxc_resolver` 对 `./flow` / `../renderers` 不折叠到 `index.ts`，边会整条丢（前端 71 → 69），而 `knip` / `tsc` 仍绿 | AC-2 的「边集逐条相等」即判红面；落地时先单独验证一条（`change-detail-view.tsx` 的 `./flow`），再批量改其余 11 处 |
| R-5 | **`isolatedModules` 下类型出口写成值出口**：`export { DrawerSelection } from './types'` 在转译期被当作值导出 ⇒ 运行期 import 未定义 | spec 钉住「类型出口一律 `export type`」；`module-entries.test.ts` 的源文本断言逐 barrel 校验；`vp check` 的 typeCheck 是第二道 |
| R-6 | **子域判据的分母移动被误算成「判据变了」**：`agent` 由 3（含 `hooks/`）到 4、flow 15 → 16、renderers 7 → 8；explore `B.7.5` 的「2 → 3」是**非 hooks 口径** | AC-7 给可复算命令并写死输出；spec 明确「含其 `hooks/`」这一口径，避免两人各按一套算出矛盾数 |
| R-7 | **explore 数字漂移被照抄**：`routes.tsx` 11 → 9（实为 8）、调用点 20 → 13（实为 21 → 12）、`entry_state` 30 / 42（实为 32 / 44） | 本提案与所有 delta 一律用**本轮实测**值；落地后按 AC 复算，MUST NOT 回抄 explore |
| R-8 | **`Components.Agent` 翻转时顺手加 `entry`** | 明写「MUST NOT 加」（提案四）：④ 候选命中即 `Resolved`，加行即违「仅 3 处」纪律并使 `repo-archi-model` 的 `字段纪律可机械审查` Scenario 判红；AC-6 的 `grep -c "^entry" == 3` 是判据 |
| R-9 | **barrel 跨子元素 / 跨元素 re-export**：`reexport_ok` 按 FQN **前缀**放行（子元素也在面内），而 `scan` 会把 `export … from` 记成 import fact ⇒ 真加边 | 纪律写进新能力 spec（只导本元素直属文件）+ AC-2 的边集相等；`§8` 的 11 条边面缺口 MUST NOT 因此被「顺手补齐」 |
| R-10 | **`views/changes/flow` 的 11 名挑选漏项**：目录 38 个公开符号，漏一个调用点即断、多一个 `knip` 即红 | 名单按「跨元素被 import 的名字」实测（提案一表格逐条给出来源文件）；`module-entries.test.ts` 逐字钉住行集合；`vp check` + `tsc --noEmit` 双绿即判据 |
| R-11 | **`repo-archi-model` 的 Scenario 枚举面被顺手扩写**：现 Scenario 只枚举 `views/changes/` 的子目录却断言「各视图的 `components/` 不构成子域」 | 明写「只改数、不扩枚举面」；纪律文本补「排除条对 `views/*/` 全体生效」使窄枚举不成为将来漏判的依据（扩面＝改判据，属独立变更） |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| B.7.2「`api` 三选一」 | **留空（缺省 `declared`）**：提案五，判据面 `0 + 41 = 41`、`status = clean`；另两选的理由与代价逐条留痕 |
| B.7.8 第 1 条 / B.7.11「ui 建不建 barrel、改不改 132 处」 | **建 barrel、不改调用点**（已裁）；`34 vs 39` 的次级未决**裁定为 39 条 `export *`**，5 个零引用模块登记为已知缺口（提案二 + R-2） |
| B.7.3 / B.7.12「barrel 边界纪律」 | **只导「跨元素被 import 的名字」+ MUST NOT 跨子元素 + MUST NOT 导 `hooks`/`lib`**：写进新能力 spec，`flow` 的 11 / 38 为压力样本 |
| B.7.8 第 2 条「`export type` 分割形态」 | 受 `isolatedModules` 约束：类型出口一律 `export type`，同源两类拆两行（提案一末段 + R-5） |
| B.1.6「`Components.Agent` 翻转 + 显式 `entry`」 | 翻转**采纳**；`entry` 一行**不采纳**（③ 默认已命中，加行违「恰 3 处」纪律，见提案四 + R-8） |
| B.7.4「`.i.ts` 生成链 / `ts_import_specifier` 待接线」 | **不接线**（提案五 + 明确不做表）：无 `.i.ts` 实例即无观测点，留白逐字保留 |
| B.7.5 / B.7.15「覆盖判据与子域计数的三处失真」 | 同批重测写死（提案三的病灶表 + `repo-archi-model` delta）；只改数、不扩枚举面（R-11） |
| B.7.14「三件必答题」 | 全部裁完并落纸（提案二 / 五 + 本表前三条） |
| B.6（启动就绪度评估） | **已过期**：A / C 均已归档；本提案数字一律取 `B.7` 之后的本轮实测 |
| B.5「是否顺势把 ADR『容器 `path` 唯一例外』整段重写」 | **采纳**：ADR §3 重写为「barrel = 目录模块的入口」；§1 / §2 / §9 计数同批重测 |
