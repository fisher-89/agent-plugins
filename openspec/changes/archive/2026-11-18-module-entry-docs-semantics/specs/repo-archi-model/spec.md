# repo-archi-model Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**目录双身份的基线声明**（E-1 目录迁移）：`<archi_dir>/models/` 从此一级住六个模型文件、`<Domain>/` 子目录住契约资产（`<FQN>.md` + `.schema.json`），契约索引落 `<archi_dir>/graph/INDEX.toml`。**元素 / 关系数据零改动**：53 元素 / 120 关系（`06` 段 30 / 71）、`gate` 全 `off`、`entry` 恰三处、`api = none` 恰两处、六个文件的内容逐字不变——本变更只承认「模型目录同时是契约资产目录」这一事实，并把「一级 `NN-<域>.toml`」写成机械判据（收面判据与解析实现零改动，见 `desktop-archi-model`「多文件聚合与声明定位」）。

## MODIFIED Requirements

### Requirement: 基线落位与模型文件划分

架构模型基线 SHALL 落位 `<archi_dir>/models/`（架构域根经 `foundation::layout::archi_dir` 派生，本期落点 `.dev-team/architecture/models/`），MUST NOT 落到 `src/`、`docs/`、`openspec/specs/` 或任何其它位置。

模型 SHALL 划分为**六个一级文件**，文件名形如 `NN-<域>.toml`，字典序即聚合序，每个元素与每条关系 SHALL 恰在一处声明（跨文件或同文件重复即 `duplicateElement`）：

| 文件 | 内容 | 元素 | 关系 |
|---|---|---|---|
| `01-app.toml` | 包根与两个域容器 | 3 | 0 |
| `02-backend-core.toml` | `Core` 容器 + 7 个 core crate 元素 | 8 | 9 |
| `03-backend-infra.toml` | `Infra` 容器 + **7** 个 infra crate 元素 | **8** | **24** |
| `04-backend-shell.toml` | Tauri 壳（命令面 / 装配 / bindings 导出） | 1 | **13** |
| `05-backend-tools.toml` | `Tools` 容器 + `export-bindings` crate + `archi-api-gen` crate | **3** | **3** |
| `06-frontend.toml` | 前端 `Shell` / 9 视图 / 3 视图内子域 / **12** 共享组件 / **目录模块 `Components.Agent`** / `Ui` + **三个**容器（`App.Frontend` 容器在 `01`） | **30** | **71** |

合计 SHALL 为 **53 元素 / 120 关系**，逐文件可加：元素侧 `3 + 8 + 8 + 1 + 3 + 30 = 53`（容器 9 + 叶 44），关系侧 `0 + 9 + 24 + 13 + 3 + 71 = 120`。关系侧另按取数口径分：**120 = 后端 49（`Cargo.toml` `[dependencies]` 的 workspace 内声明，拆在 `02`–`05`：9 + 24 + 13 + 3）+ 前端 71（`import` 证据，全在 `06`）**；两种拆分 MUST 得同值。前端 71 的构成 SHALL 为 **64 条叶 → 叶边 + 7 条 `App.Frontend.Components.Agent` 的 barrel 端点边**（3 条出边：入口 `index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`；4 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）——这 7 条由「`components/agent/` 是 `Components.Agent` 的目录模块入口」与「`@/components/agent` → `Components.Agent` 映射」两条规则必然导出，MUST NOT 从记数中剔除；12 个目录模块入口的其余 re-export 全在本元素内（`from == to` 自边被丢弃）⇒ 零新边。其中 `06-frontend.toml` 的 30 元素 = **3 个容器**（`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Standard`）+ **27 个叶**（`Shell` 1 + 9 视图 + 3 子域 + 12 共享组件 + `Components.Agent` 1 + `Ui` 1）；`App.Frontend` 容器 SHALL 只在 `01-app.toml` 声明一次，`06` MUST NOT 重复声明（重复即 `duplicateElement`，并会使元素总数变成 54 而非 53）。

- **`models/` 的双身份 SHALL 由文件命名与目录层级承担**（本变更承认的事实）：**一级**是模型声明面——恰六个 `NN-<域>.toml`，MUST NOT 出现其他一级 `*.toml`（尤其 MUST NOT 出现 `INDEX.toml`）；**`<Domain>/` 子目录**是契约资产面——`<FQN>.md`（手写真相）+ `<FQN>.schema.json`（生成物），布局与命名归 `desktop-archi-api`「契约资产形态与唯一手写真相」。两面的分离判据是「一级 vs 子目录」与「`.toml` vs `.md` / `.schema.json`」，MUST NOT 引入第二份目录级豁免清单或改名方案；契约索引不属模型目录，落 `<archi_dir>/graph/INDEX.toml`（`archi_graph_dir`）——索引落 `models/` 根会被当模型文件解析并使模型停摆，理由见 `desktop-archi-model`「多文件聚合与声明定位」。
- **契约子树不计入模型计数**：`models/<Domain>/` 下的任何文件 MUST NOT 改变元素数 / 关系数 / `modelHash`——收面判据是「一级 + `.toml` 后缀」，子目录不递归即不可见；`modelHash` 取同一收面，故本变更后金样哈希逐字不变。
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
- **`docs`**：SHALL 只由包根元素 `App` 显式指向仓库面文档（`./README.md` / `./AGENT.md`），其余元素留空；任何元素 MUST NOT 把 `openspec/specs/**` 路径写进 `docs`，也 MUST NOT 把 `<archi_dir>/models/<Domain>/**` 写进 `docs`（守 `desktop-archi-module-docs` 的迁移边界与「契约资产不经 docs 三源」）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。
- **计数复述单源**：模型文件的表头注释（如 `# 03 · infra 适配层（8 元素 / 24 关系）`）、本要求的逐文件计数表、与裁定记录 `§9` 的复算表 SHALL 在同一次模型改动内同步为同值；三处任一处陈旧（仍写 50 / 111 / 47 / 2 元素 1 关系，或仍写 51 / 113、`06` 段 28 / 64、仍写「4 个容器」）即漂移，按「同一 change 内补齐」处理，MUST NOT 让计数表与模型本体各说各话。**字段面扩容（`entry` / `api`）与目录模块入口（barrel）不改变元素与关系计数**：12 个 barrel 是元素 `path` 内的新增文件，既不新增元素也不新增边（入口只 re-export 本元素直属文件 ⇒ `from == to` 自边被丢弃），故 `06` 段 30 / 71 与全基线 53 / 120 零变化；**元素粒度补漏改变计数**（`components/agent/session-panel.tsx` 与 `components/standard/tooltip.tsx` 补建元素：元素 +2、关系 +7）；**元素形态翻转改变容器 / 叶切分而不改总数**（`Components.Agent` 容器 → 目录模块：容器 10 → 9、叶 43 → 44、`06` 段 4 容器 → 3 容器 + 27 叶）；**契约资产迁入 `models/<Domain>/` 与索引迁入 `graph/` 不改变任何计数**（两者都不进模型收面：前者在子目录、后者在模型目录之外）。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `models/` 一级的六个文件（同目录另有 `<Domain>/` 契约子树与若干 `*.schema.json`）
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 53 条、`relations` 共 120 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行；契约子树内的任何文件 MUST NOT 出现在 `modelFile` 里

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` / `entry` / `api` 出现行
- **THEN** `gate` 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；`docs` 仅 `<archi_dir>` 之外的根文档两条；全部叶元素 `path` 以 `"./` 开头，全部容器 `path` 为空（**零例外**——`App.Frontend.Components.Agent` 已翻转为目录型 `module`，其目录 `path` 属叶元素面）；`entry` **恰三行**（`04` 一行、`05` 两行，各为单值字符串且落在其元素某条 `path` 之内，`01` / `02` / `03` / `06` 零行）；`api` 恰两行且均为 `"none"`（两个工具 crate）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `models/` 的**一级**恰有六个 `NN-<域>.toml`（`find -maxdepth 1 -name '*.toml'` 读数 = 6，无 `INDEX.toml`）；`decisions/` 有 `0001-model-granularity.md`；`graph/` 恰有 `INDEX.toml` 一份；`models/<Domain>/` 下为契约资产（`<FQN>.md` + `<FQN>.schema.json`）；`api/` 目录不存在；`src/`、`docs/`、`openspec/specs/` 下无模型文件；`archi_module("App")` 的 `docs` 中出现该 ADR 条目（`source = "text"`）

#### Scenario: 计数复述三处同值

- **WHEN** 逐处读取 `06` 文件表头注释、本要求的计数表与 ADR `§1` / `§9` 的复算表
- **THEN** 三处同值：全基线 53 元素 / 120 关系（容器 9 + 叶 44；后端 49 + 前端 71）、`06` 段 **30 元素 / 71 关系**（3 容器 + 27 叶）、`05` 段 3 元素 / 3 关系、`03` 段 8 元素 / 24 关系、`04` 段 1 元素 / 13 关系；任一处仍写 51 / 113（含 `06` 段 28 / 64）、50 元素 / 111 关系 / 47 关系 / 2 元素 / 容器 10 / 「4 个容器」即不合格

#### Scenario: 三处入口与 12 个目录入口在案后的两类空态归位

- **WHEN** 分别对壳、两个工具 crate、12 个前端目录根元素跑一次全量检查并读 `apiFindings`
- **THEN** 壳**零** `apiEntryMissing`，同趟 `apiMissing` 恰一条（显式 `entry` 命中 ⇒ ① 不成立；`api = declared` 且契约文件缺失 ⇒ ② 成立）——两条 MUST NOT 同时出现（入口债与契约债是两回事，壳「入口已通」不等于「契约已写」）；两个工具 crate 在两处**均零条目**（`entry` 命中 + `api = none`：① 与 ② 都不成立），该结论依赖两条事实同时在场，MUST NOT 因「无库 API」被重算成「无入口」；12 个前端目录根**零** `apiEntryMissing`（各自的 `index.ts` / `index.tsx` 由 ③ 命中 ⇒ ① 不成立），同趟**各一条** `apiMissing`——12 条由此由 ① 面平移到 ② 面、总数不变，MUST NOT 被读成「入口债已清 ⇒ 判据面干净」（基线第五段 `0 + 41 = 41` 见 `desktop-archi-api-check`）；反方向 MUST NOT 成立：14 个 `src/lib.rs` 命中的 crate、前端 **14** 个单文件根与 **12** 个目录模块根各自由约定默认解析出入口，零 `apiEntryMissing`

#### Scenario: 契约子树与索引不改变模型读数

- **WHEN** 在 `models/<Domain>/` 放入契约 md 与 `.schema.json`、在 `graph/` 放入 `INDEX.toml` 之后，分别取一次 `archi_graph`（元素 / 关系 / 诊断）与一次 `archi_drift` 的 `modelHash`
- **THEN** 元素 53 / 关系 120 / `diagnostics` 空与放入前逐字相同；`modelHash` 逐字相同（取面为 `models/*.toml` 一级、按扩展名）；反向判据：把 `INDEX.toml` 复制到 `models/` 根后 `modelHash` 立刻变化且模型转停摆态——两侧读数即本条的存在理由

## Module Contract

| 模块 | 变化 | 关键描述 |
|------|------|----------|
| `.dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,04-backend-shell,05-backend-tools,06-frontend}.toml` | 零内容改动 | 六个文件逐字不变（53 元素 / 120 关系、`06` 段 30 / 71、`gate` 全 `off`、`entry` 恰三行、`api = none` 恰两行） |
| `.dev-team/architecture/models/<Domain>/**`（新增，契约资产） | 迁移落点 | 由 `<archi_dir>/api/<Domain>/**` 搬入：`<FQN>.md`（唯一手写）+ `<FQN>.schema.json`（生成物）；不进模型收面（子目录不递归） |
| `.dev-team/architecture/graph/INDEX.toml`（新增，契约索引） | 迁移落点 | 由 `<archi_dir>/api/INDEX.toml` 搬入并重生成；不进模型收面（不在 `models/` 内），`modelHash` 因此零变化 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 零改动 | 裁定记录 §1 / §2 / §3 / §9 的计数与口径本变更零触及（契约目录不在其计数面内） |
