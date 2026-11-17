# repo-archi-model Specification（delta: frontend-barrel-entry-migration）

## Purpose

本 delta 只改**模型数据的一处形态与随之而来的计数 / 叙述**，判据面（三条粒度档、关系只取 `Cargo.toml` 与 `import` 证据、四类结构诊断零命中、`gate` 全 `off`、封闭排除面、漂移纪律）**逐字不动**：

1. **一个元素的形态翻转**：`App.Frontend.Components.Agent` 由 `kind = "domain"` + 文件型 `path`（`components/agent/index.tsx`）翻转为 `kind = "module"` + 目录型 `path`（`components/agent/`）。理由是 ADR 0001 §3 的「容器 `path` 恒空，唯一例外是 barrel」在新口径下语义错位——barrel 是**目录模块的入口**，不是容器的特权；12 个目录补入口后同一形态会出现 12 处，例外条款当场失去解释力。翻转**不加 `entry`**（③ 的 `<root>/index.tsx` 已命中）。
2. **零元素 / 零关系增减**：53 元素 / 120 关系、`06` 段 30 / 71 与 71 = 64 + 7 的恒等式**逐条不变**；变的只有容器与叶的切分（容器 10 → 9、叶 43 → 44；`06` 段 3 容器 + 27 叶）与「12 个前端目录根无入口」这条现状陈述（现基线**无入口集合为空**）。
3. **三处叙述性失真同批校准**（都是本变更要动的分母）：`components/ui` 的 35 → **39** 个文件、子域计数 `flow` 15 → **16** / `renderers` 7 → **8** / `agent` 3 → **4**（含其 `hooks/` 的口径）、`views/changes/components/` 1 → **2** 个文件。**只改数、不扩枚举面**，并把「排除条对 `views/*/` 全体生效」写成纪律文本。
4. **为什么现在做**：入口面（`entry` 字段 + 三态解析链）与取数面（TS 导出闭包）均已落地，12 个目录的 barrel 是该链路在本仓的**第一个批量实例**；模型不跟上就会留下「有入口却按无入口记账」的假话。

## MODIFIED Requirements

### Requirement: 基线落位与模型文件划分
架构模型基线 SHALL 落位 `<archi_dir>/models/`（架构域根经 `foundation::layout::archi_dir` 派生，本期落点 `.dev-team/architecture/models/`），MUST NOT 落到 `src/`、`docs/`、`openspec/specs/` 或任何其它位置。

模型 SHALL 划分为**六个**文件，文件名形如 `NN-<域>.toml`，字典序即聚合序，每个元素与每条关系 SHALL 恰在一处声明（跨文件或同文件重复即 `duplicateElement`）：

| 文件 | 内容 | 元素 | 关系 |
|---|---|---|---|
| `01-app.toml` | 包根与两个域容器 | 3 | 0 |
| `02-backend-core.toml` | `Core` 容器 + 7 个 core crate 元素 | 8 | 9 |
| `03-backend-infra.toml` | `Infra` 容器 + **7** 个 infra crate 元素 | **8** | **24** |
| `04-backend-shell.toml` | Tauri 壳（命令面 / 装配 / bindings 导出） | 1 | **13** |
| `05-backend-tools.toml` | `Tools` 容器 + `export-bindings` crate + `archi-api-gen` crate | **3** | **3** |
| `06-frontend.toml` | 前端 `Shell` / 9 视图 / 3 视图内子域 / **12** 共享组件 / **目录模块 `Components.Agent`** / `Ui` + **三个**容器（`App.Frontend` 容器在 `01`） | **30** | **71** |

合计 SHALL 为 **53 元素 / 120 关系**，逐文件可加：元素侧 `3 + 8 + 8 + 1 + 3 + 30 = 53`（容器 9 + 叶 44），关系侧 `0 + 9 + 24 + 13 + 3 + 71 = 120`。关系侧另按取数口径分：**120 = 后端 49（`Cargo.toml` `[dependencies]` 的 workspace 内声明，拆在 `02`–`05`：9 + 24 + 13 + 3）+ 前端 71（`import` 证据，全在 `06`）**；两种拆分 MUST 得同值。前端 71 的构成 SHALL 为 **64 条叶 → 叶边 + 7 条 `App.Frontend.Components.Agent` 的 barrel 端点边**（3 条出边：入口 `index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`；4 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）——这 7 条由「`components/agent/` 是 `Components.Agent` 的目录模块入口」与「`@/components/agent` → `Components.Agent` 映射」两条规则必然导出，MUST NOT 从记数中剔除；12 个目录模块入口的其余 re-export 全在本元素内（`from == to` 自边被丢弃）⇒ 零新边。其中 `06-frontend.toml` 的 30 元素 = **3 个容器**（`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Standard`）+ **27 个叶**（`Shell` 1 + 9 视图 + 3 子域 + 12 共享组件 + `Components.Agent` 1 + `Ui` 1）；`App.Frontend` 容器 SHALL 只在 `01-app.toml` 声明一次，`06` MUST NOT 重复声明（重复即 `duplicateElement`，并会使元素总数变成 54 而非 53）。

- **关系归属**：每条 `[[relation]]` SHALL 声明在**其 `from` 元素所在文件**内（定位一致、diff 不跨文件散落）。
- **`gate` 全 `off`**：本基线内每个元素 SHALL 省略 `gate` 或显式写 `off`，MUST NOT 出现 `warn` / `strict`——冷启动 ratchet（收紧按模块独立变更，一个 change 收紧一块）。
- **`summary`**：每个元素 SHALL 有一行中文职责说明，MUST NOT 用 FQN 字面复述自身名字。
- **`path`**：叶元素必填、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器元素 SHALL 留空（**零例外**）。目录模块入口由 `desktop-frontend-module-entry` 承载：`App.Frontend.Components.Agent` 是 `kind = "module"` 的**叶**元素，`path` 为目录 `./packages/desktop/src/components/agent/`，入口是该目录内既有的 `index.tsx`（不再以文件型 `path` 自任端点）。
- **`entry`**：叶元素 SHALL **仅在约定默认指不到时**声明（单值、`./` 前缀、repo 相对）；单文件根元素（`views/welcome-view.tsx`、`components/agent/` 的 7 个组件文件、`standard/*.tsx`）、12 个目录模块根（入口由 ③ 的 `<root>/index.ts` / `<root>/index.tsx` 命中，含 `components/agent/` 与 `components/ui/`）与 14 个 `src/lib.rs` 命中的 crate MUST NOT 声明（根文件自身 / 语言约定默认已给出入口）。**本基线内显式 `entry` SHALL 恰为 3 处**（下列三值逐字在案；宿主能解析 `entry` 字段是这三行落地的前提，见 `desktop-archi-api`「生成物落点与入口查找」）：
  - `App.Backend.Shell` → `./packages/desktop/src-tauri/src/lib.rs`（目录型壳的根路径 `src-tauri/src/` 拼出的约定默认是 `src-tauri/src/src/lib.rs`，指不到）；
  - `App.Backend.Tools.ExportBindings` → `./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs`；
  - `App.Backend.Tools.ArchiApiGen` → `./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`。

  三处声明的后果 SHALL 被接受且可读：壳由 `apiEntryMissing`（`Absent`）转为 `apiMissing`（`declared` 缺契约），两个工具 crate 在 `apiEntryMissing` 面**零条目**（`entry` 命中 ⇒ ① 不再成立；`api = none` ⇒ ② 不适用）——「有入口」与「无库 API」两个事实同时成立，MUST NOT 被读成「入口补齐 ⇒ 判据面清零」。MUST NOT 出现第四处显式 `entry`：其余叶元素由约定默认覆盖（单文件根 = 根文件自身、目录型根 = `<root>/index.ts(x)`、14 个 crate 的 `src/lib.rs`），多写即违「仅在约定默认指不到时声明」，并使本条的三处计数失守；`App.Frontend.Components.Agent` 的 `path` 由文件改目录后正是 ③ 的场景，**加行即违本条**。

  第四节「四条判据」（基准 / 落位 / 存在 / 跨元素唯一）与三态解析链归 `desktop-archi-api`，本节只钉基线数据；违规走 `apiFindings` 的 `apiEntryMissing`（`warn`，不影响 `passed`），MUST NOT 引入结构化诊断。
- **`api = none` 两处显式留白**：两个工具 crate（`App.Backend.Tools.ExportBindings` / `App.Backend.Tools.ArchiApiGen`）SHALL 声明 `api = none`——二进制 crate 的公共面是 CLI 契约而非库 API，这是**真话**而非「还没写」；其余元素 SHALL 省略 `api`（取缺省 `declared`）。「无入口」与「无 API」是两个不同事实：本基线内**无入口集合为空**（12 个前端目录根由 `desktop-frontend-module-entry` 的 `index.ts(x)` 经 ③ 命中），**无 API = 两个工具 crate**（各有显式 `entry` 在案，即「有入口但无库 API」）——MUST NOT 混用，也 MUST NOT 由「两工具 crate 已无判据」反推「它们没有入口」；反方向同理：12 个前端目录元素仍有 `apiMissing`（有门无契约），MUST NOT 被读成「无 API」。
- **`docs`**：SHALL 只由包根元素 `App` 显式指向仓库面文档（`./README.md` / `./AGENT.md`），其余元素留空；任何元素 MUST NOT 把 `openspec/specs/**` 路径写进 `docs`（守 `desktop-archi-module-docs` 的迁移边界：模型 MUST NOT 反向依赖将被替代的语料）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。
- **计数复述单源**：模型文件的表头注释（如 `# 03 · infra 适配层（8 元素 / 24 关系）`）、本要求的逐文件计数表、与裁定记录 `§9` 的复算表 SHALL 在同一次模型改动内同步为同值；三处任一处陈旧（仍写 50 / 111 / 47 / 2 元素 1 关系，或仍写 51 / 113、`06` 段 28 / 64、仍写「4 个容器」）即漂移，按「同一 change 内补齐」处理，MUST NOT 让计数表与模型本体各说各话。**字段面扩容（`entry` / `api`）与目录模块入口（barrel）不改变元素与关系计数**：12 个 barrel 是元素 `path` 内的新增文件，既不新增元素也不新增边（入口只 re-export 本元素直属文件 ⇒ `from == to` 自边被丢弃），故 `06` 段 30 / 71 与全基线 53 / 120 零变化；**元素粒度补漏改变计数**（`components/agent/session-panel.tsx` 与 `components/standard/tooltip.tsx` 补建元素：元素 +2、关系 +7）；**元素形态翻转改变容器 / 叶切分而不改总数**（`Components.Agent` 容器 → 目录模块：容器 10 → 9、叶 43 → 44、`06` 段 4 容器 → 3 容器 + 27 叶）。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `models/` 下六个文件
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 53 条、`relations` 共 120 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` / `entry` / `api` 出现行
- **THEN** `gate` 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；`docs` 仅 `<archi_dir>` 之外的根文档两条；全部叶元素 `path` 以 `"./` 开头，全部容器 `path` 为空（**零例外**——`App.Frontend.Components.Agent` 已翻转为目录型 `module`，其目录 `path` 属叶元素面）；`entry` **恰三行**（`04` 一行、`05` 两行，各为单值字符串且落在其元素某条 `path` 之内，`01` / `02` / `03` / `06` 零行）；`api` 恰两行且均为 `"none"`（两个工具 crate）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `models/` 恰有六个 `NN-<域>.toml`，`decisions/` 有 `0001-model-granularity.md`；`src/`、`docs/`、`openspec/specs/` 下无模型文件；`archi_module("App")` 的 `docs` 中出现该 ADR 条目（`source = "text"`）

#### Scenario: 计数复述三处同值

- **WHEN** 逐处读取 `06` 文件表头注释、本要求的计数表与 ADR `§1` / `§9` 的复算表
- **THEN** 三处同值：全基线 53 元素 / 120 关系（容器 9 + 叶 44；后端 49 + 前端 71）、`06` 段 **30 元素 / 71 关系**（3 容器 + 27 叶）、`05` 段 3 元素 / 3 关系、`03` 段 8 元素 / 24 关系、`04` 段 1 元素 / 13 关系；任一处仍写 51 / 113（含 `06` 段 28 / 64）、50 元素 / 111 关系 / 47 关系 / 2 元素 / 容器 10 / 「4 个容器」即不合格

#### Scenario: 三处入口与 12 个目录入口在案后的两类空态归位

- **WHEN** 分别对壳、两个工具 crate、12 个前端目录根元素跑一次全量检查并读 `apiFindings`
- **THEN** 壳**零** `apiEntryMissing`，同趟 `apiMissing` 恰一条（显式 `entry` 命中 ⇒ ① 不成立；`api = declared` 且契约文件缺失 ⇒ ② 成立）——两条 MUST NOT 同时出现（入口债与契约债是两回事，壳「入口已通」不等于「契约已写」）；两个工具 crate 在两处**均零条目**（`entry` 命中 + `api = none`：① 与 ② 都不成立），该结论依赖两条事实同时在场，MUST NOT 因「无库 API」被重算成「无入口」；12 个前端目录根**零** `apiEntryMissing`（各自的 `index.ts` / `index.tsx` 由 ③ 命中 ⇒ ① 不成立），同趟**各一条** `apiMissing`（`api = declared` 且契约文件缺失 ⇒ ② 成立）——12 条由此由 ① 面平移到 ② 面、总数不变，MUST NOT 被读成「入口债已清 ⇒ 判据面干净」（基线第五段 `0 + 41 = 41` 见 `desktop-archi-api-check`）；反方向 MUST NOT 成立：14 个 `src/lib.rs` 命中的 crate、前端 **14** 个单文件根与 **12** 个目录模块根各自由约定默认解析出入口，零 `apiEntryMissing`

### Requirement: 前端下钻到视图与组件层的元素覆盖
前端模型 SHALL 下钻到**视图**与**组件**两层，并按三条机械判据推导元素，MUST NOT 按个人口味挑选：

| 档 | 判据 | 元素 |
|---|---|---|
| 页 / 壳 | `src/app.tsx` + `routes.tsx` + `main.tsx` + `app-sidebar.tsx` 合成 `App.Frontend.Shell`；`views/<view>/` 每目录一元素；`views/welcome-view.tsx` 单文件一元素 | `App.Frontend.Shell`、`App.Frontend.Views.{Agents,AgentDebug,Architecture,Changes,Config,DbInspector,Explores,Info,Welcome}` |
| 视图内子域 | `views/<view>/` 下**除 `components` / `hooks` 外**、递归非测试源文件 **≥ 3** 的子目录 | `App.Frontend.Views.Changes.{Flow,Renderers,Agent}` |
| 共享组件 | `src/components/agent/*.{ts,tsx}` 与 `src/components/standard/*.tsx` 每文件一元素（排除 `*.test.*` 与 barrel `index.tsx`）；`src/components/ui/` **收敛为 1 元素**；`components/agent/` 的 barrel `index.tsx` 由目录模块 `App.Frontend.Components.Agent` 持有 | `App.Frontend.Components.Agent.{AgentBlocks,AgentError,AgentInput,AgentMessages,AgentSessionView,SessionPanel,ToolCards}`、`App.Frontend.Components.Standard.{Dailog,Drawer,Pagination,Select,Tooltip}`、`App.Frontend.Components.Ui` |

容器 SHALL 为 `App.Frontend`、`App.Frontend.Views`、`App.Frontend.Components`、`App.Frontend.Components.Standard`——`kind = "domain"`、`path` 空（**零例外**）；视图与视图内子域 SHALL 为 `module`、共享组件文件 SHALL 为 `component`、`App.Frontend.Components.Ui` SHALL 为 `module`；**`App.Frontend.Components.Agent` SHALL 为目录型 `module`**（`path` 为 `./packages/desktop/src/components/agent/`，入口为目录内 `index.tsx`，见 `desktop-frontend-module-entry`）。

- **`components/ui` 收敛的理由**：ui 是 shadcn 原语层，**39** 个文件恒被依赖、不承担「谁能依赖谁」的判定；逐文件建元素只增噪声（39 元素）不增信息。若将来 ui 内部出现实质分层，再拆元素（须改本 requirement）。
- **封闭排除面**（不建元素、不参与覆盖、不贡献边）：`src/hooks/**`、`src/lib/**`、`src/types/**`（含 `types/generated/bindings.ts`）、`src/components/standard/hooks/**`、`*.test.{ts,tsx}`、`src/test-setup.ts`。
- **目录模块入口不建元素**：12 个 `index.ts(x)` SHALL 计入其所属元素（视图目录 / 子域目录 / `components/ui/` 的入口在元素 `path` 之内由目录元素持有；`components/agent/index.tsx` 由目录模块 `Components.Agent` 持有）——入口 MUST NOT 另建元素，也 MUST NOT 被读成「元素面扩容」（barrel 只让子域判据的分母 +1，见下）。
- **视图内子域的分母含 `hooks/`**：判据的「递归非测试源文件 ≥ 3」按**递归**口径计数（`views/changes/agent/` 的 3 = 2 个直属文件 + `hooks/use-change-chat.ts`）。barrel 落地后本仓三处子域的分母为 `flow` **16** / `renderers` **8** / `agent` **4**；MUST NOT 混用「不含 hooks」的窄口径（那会算出 2 → 3，与 3 → 4 矛盾）。
- **`components/` 排除面对 `views/*/` 全体生效**：路径段含 `components` 的目录 MUST NOT 构成子域（本仓实例：`views/architecture/components/`（含其 `graph/`）、`views/config/components/`、`views/explores/components/`、`views/info/components/`、`views/agents/components/`、`views/agent/components/`、`views/changes/components/`）——排除条 MUST NOT 被读成「只对 `views/changes/` 生效」，`components/` 嵌在 `components/` 里的目录同样出局。
- **粒度 ≠ 解析面**：视图目录内的 `hooks/` / `lib/` 文件不建元素，但其 `import` 仍计入**所属视图元素**的依赖（归属按目录、粒度按是否建元素，两者不是一回事）；`src/components/standard/hooks/` 不入模型是为了守住「容器 `path` 恒空」——把 hooks 归给容器就等于把「子 → 父」写成依赖。
- **覆盖判据（双向）**：上述判据命中的目录 / 文件集合与前端叶元素 `path` 集合 SHALL 相等；判据外的元素 MUST NOT 存在。唯一例外是裁定记录 `§8` 的**已知缺口清单**——本期前端两条：`components/file/file-viewer.tsx`（ADR 三层表无 `components/<其它目录>` 档，补齐须**新增粒度档** ＝改本 requirement）与 `views/architecture/agent/`（判据命中「递归非测试源文件 3 个」但未建元素，补齐属独立变更）。缺口 MUST 单列入 `§8` 且 MUST NOT 被读成「判据失效」；本变更 MUST NOT 顺手补这两条（补齐须另开 change）。

#### Scenario: 视图与子域覆盖

- **WHEN** 枚举 `src/views/*/` 与 `src/views/welcome-view.tsx`、并枚举 `views/changes/` 下满足「非 `components`/`hooks` 且递归非测试源文件 ≥ 3」的子目录
- **THEN** 前者与 9 个视图元素一一对应；后者恰为 `flow`（**16** 源文件）、`renderers`（**8**）、`agent`（**4**，含其 `hooks/`）三个**已建元素**的子域；`views/changes/components/`（**2** 文件）与各视图的 `components/` 目录不构成子域（排除条对 `views/*/` 全体生效，枚举面仍以 `views/changes/` 为抽样实例，MUST NOT 据此收窄判据）

#### Scenario: 共享组件逐文件与 ui 收敛

- **WHEN** 检查 `src/components/` 的元素声明
- **THEN** `agent/` 下 **7** 个源文件各一 `component` 元素（`index.tsx` barrel 排除、由目录型 `module` 元素 `App.Frontend.Components.Agent` 持有；含 `session-panel.tsx` → `SessionPanel`）、`standard/` 下 **5** 个 `.tsx` 各一元素（含 `tooltip.tsx` → `Tooltip`）、`ui/` 恰 1 个 `module` 元素且 `path` 指向该目录（入口为该目录内的 `index.ts`）

#### Scenario: 排除面零元素

- **WHEN** 检查 `src/hooks`、`src/lib`、`src/types`、`src/components/standard/hooks`、测试文件与 `test-setup.ts`
- **THEN** 以上位置零元素、零 `path` 引用；同时 `views/*/hooks/**` 与 `views/*/lib/**` 文件不构成元素（其依赖计入所属视图）；`views/*/components/**` 下的任何目录（含二级目录）零元素

### Requirement: 前端关系只取 import 证据
前端关系 SHALL 只由**已声明元素覆盖范围内**文件的 `import` 语句证成，MUST NOT 凭语义直觉连边：

- **目标归属**：被 import 的路径 SHALL 按「最具体已声明元素」归属（`@/components/ui/button` → `Components.Ui`；`@/components/standard/drawer` → `Standard.Drawer`；`@/components/agent`（barrel）→ `Components.Agent`；`./flow` / `../renderers` 一类目录 specifier 由解析器的 index 折叠落到 `<dir>/index.ts`，归属与折叠前被打到的文件同元素）。
- **命令面边**：`types/generated/bindings` 的 `import` SHALL 映射为**命令面依赖**，指向 `App.Backend.Shell`（`desc` 写明「经生成的命令面类型」）——这是前端对后端的唯一合法边来源；`src/lib` / `src/hooks` 未被建模，故壳层对命令面的依赖本期不可见（已知缺口，见裁定记录）。该映射是**声明式约定**：`src/types/**` 无元素归属，扫描解析出目标文件后因 `Owner::None` 丢弃该边，故命令面边在报告里落 `observations[].unfulfilledDeclaration`（`gate` 全 `off` ⇒ 永不进 `errors` / `warnings`）——MUST NOT 用「`observations` 为空」验收本能力，也 MUST NOT 期待这类边出现在违例桶。
- **不解析面零边**：指向 `hooks/`、`lib/`、`types/dto` 的 `import` MUST NOT 产生关系（本期不解析）；`App.Frontend.Views.Welcome` 一类「只经 hooks 取数」的元素因而可无后端边。
- **包含与自环**：子元素归属由 FQN 表达，MUST NOT 连边；`from == to` 的自环 MUST NOT 出现（同元素内部互引不建边）——**12 个目录模块入口的 re-export 正是这一条**：入口与被聚合的直属文件同属一个元素，故补入口**零新边**。
- **父 → 子的 import 边合法**：`Views.Changes → Views.Changes.Flow`（详情页组装流程画布）是装配依赖，不是包含；禁的只是「子 → 父」。
- **目录模块作端点仅限既有 barrel 一例**：12 个目录入口中唯一能作为关系端点的是 `App.Frontend.Components.Agent`（其入口 `components/agent/index.tsx` 重导出 3 个子元素的导出，且有 4 个元素 import 它）；其端点边 SHALL 恰为 **7 条**——3 条出边（`index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`）+ 4 条入边（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）。其余三个容器（`Views` / `Components` / `Standard`）MUST NOT 出现在任何关系的任一端点上；其余 11 个目录入口 MUST NOT 出现在任何关系的任一端点上（它们的 re-export 目标是本元素直属文件，若出现边即说明有跨元素 / 跨子元素 re-export）。
- **计数恒等式**：前端边 SHALL 为 **71 条 / 24 个源**，且 MUST 满足 `71 = 64（叶 → 叶）+ 7（barrel 端点边）`；若某次落笔得到 64，说明这 7 条被漏掉（判据自相矛盾），MUST NOT 以「容器不入边」为由删减——要改只能改本要求的字段规则或映射规则（撤销 `Components.Agent` 的目录模块入口），不能在实现期折中。

#### Scenario: 每条边可指到 import 语句

- **WHEN** 逐条核对 71 条前端边（24 个源）
- **THEN** 每条均能在其 `from` 元素覆盖范围内的某文件里找到对应 `import`（证据清单在 change 报告留痕）；反向抽查「有 import、无关系」的跨元素引用为零——唯一例外是裁定记录 `§8` 登记的**既有边面缺口**（本期 11 条已建元素间的跨元素 `import` 未声明，其中 `Views.Changes.Flow → Views.Changes` 是「子 → 父」形态、声明即 `dependsOnAncestor` 停摆，故补齐须另开 change），缺口 MUST 单列且 MUST NOT 在本变更内补齐；12 个目录入口的 re-export 属「同元素内互引」，MUST NOT 被算进这 11 条

#### Scenario: 命令面边恰一处来源

- **WHEN** 检查全部指向 `App.Backend.Shell` 的前端边
- **THEN** 其来源文件均 import 了 `types/generated/bindings`（`Views.Agents` / `Views.AgentDebug` / `Views.Architecture` / `Views.Changes` / `Views.Changes.Flow` / `Views.Changes.Agent` / `Views.Config` / `Views.DbInspector` / `Views.Explores` / `Views.Info` / `Components.Agent.AgentSessionView` / `Components.Agent.SessionPanel` 一类），MUST NOT 存在以 `hooks` / `lib` 为唯一证据的命令面边

#### Scenario: 目录模块端点边恰为 barrel 的 7 条

- **WHEN** 统计模型内全部以 `App.Frontend.Components.Agent` 为任一端点的前端关系，并另查 12 个目录入口是否有其它端点边
- **THEN** 前者恰 7 条：出边 `AgentInput` / `AgentSessionView` / `SessionPanel`，入边 `Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`；后者（其余 11 个目录入口）**零端点边**——它们的 re-export 目标全在本元素内、自边被丢弃；`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Standard` 零端点，前端边总数 71 = 64 + 7

#### Scenario: 不解析面零边与自环零出现

- **WHEN** 审查模型内前端边集合
- **THEN** 不存在仅由「import `@/hooks/...`」「import `@/lib/...`」「import `@/types/dto`」证成的边；`from == to` 的自环零条；`Components.Ui` 只作为被依赖方出现（出边为零，其内部互引与入口的 39 条 `export *` 均不建边）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.dev-team/architecture/models/06-frontend.toml` | 前端视图与组件层 | 30 元素 = **3 容器**（`Views` / `Components` / `Standard`）+ **27 叶**（`Shell` / 9 视图 / 3 子域 / 12 共享组件 / `Components.Agent`（`kind = "module"`、`path = ./packages/desktop/src/components/agent/`、**零 `entry`**）/ `Ui`）+ **71 条 import 边**（24 个源；`Ui` 恒为被依赖方，`Components.Agent` 的 3 出 / 4 入共 7 条 barrel 端点边）；表头注释同步为「3 个容器」 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 粒度与口径裁定 | §3 重写为「容器 `path` 恒空（零例外）；barrel 是目录模块的入口，不是容器的特权」（7 条端点边与「父 import 子合法」两句保留）；§1 / §9 计数改为 53 / 120（容器 9 + 叶 44、前端 71）；§2 子域计数改 `flow` 16 / `renderers` 8 / `agent` 4、ui 39 文件、`views/changes/components` 2 文件；`entry` / `api` 字段面与 `gate` 全 `off` 判据零改动 |
| 其余五个模型文件（`01`–`05`） | 后端与包根 | 零改动：元素 / 关系 / `path` / `entry` / `api` 逐字不变（后端 49 条、容器 7 条、三处显式 `entry`、两处 `api = none`） |
