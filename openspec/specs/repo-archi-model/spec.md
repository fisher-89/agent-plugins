# repo-archi-model Specification

## Purpose

定义本仓库架构模型基线的内容与判据：模型落位 `<archi_dir>/models/` 并划分为六个 `NN-<域>.toml` 文件（字典序即聚合序、每元素每关系恰声明一处），后端下钻到 workspace crate 粒度且关系只取 `Cargo.toml` `[dependencies]` 的 workspace 内声明，前端下钻到视图与共享组件粒度且关系只由 `import` 证据证成，全基线 53 元素 / 120 关系、`gate` 全 `off`，并保证四类结构诊断零命中、既有只读检索面零改动可用，以及元素覆盖与关系证据可机械核对、漂移在同一变更内补齐。

## Requirements

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

### Requirement: 后端下钻到 crate 层的元素覆盖

后端模型 SHALL 下钻到 **crate 粒度**并在此处收敛：`Cargo.toml` `[workspace] members` 内每个 crate 目录 SHALL 对应**恰一个**元素，crate **内部**的 `.rs` 文件 MUST NOT 建立元素（内部模块划分不是架构声明，且会随重构高频漂移）。

元素清单 SHALL 为（容器 `kind = "domain"`、`path` 空；crate 与壳 `kind = "module"`、`path` 指向 crate 目录或 `src-tauri/src/`）：

| FQN | `kind` | `path` |
|---|---|---|
| `App` | `package` | 空（`docs` 指根文档） |
| `App.Backend` | `domain` | 空 |
| `App.Backend.Core` | `domain` | 空 |
| `App.Backend.Core.Foundation` | `module` | `./packages/desktop/src-tauri/crates/core/foundation/` |
| `App.Backend.Core.Config` | `module` | `./packages/desktop/src-tauri/crates/core/config/` |
| `App.Backend.Core.Workflow` | `module` | `./packages/desktop/src-tauri/crates/core/workflow/` |
| `App.Backend.Core.Archi` | `module` | `./packages/desktop/src-tauri/crates/core/archi/` |
| `App.Backend.Core.Orchestration` | `module` | `./packages/desktop/src-tauri/crates/core/orchestration/` |
| `App.Backend.Core.Agent` | `module` | `./packages/desktop/src-tauri/crates/core/agent/` |
| `App.Backend.Core.Checks` | `module` | `./packages/desktop/src-tauri/crates/core/checks/` |
| `App.Backend.Infra` | `domain` | 空 |
| `App.Backend.Infra.AgentRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/agent/` |
| `App.Backend.Infra.ArchiRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/archi/` |
| `App.Backend.Infra.ChecksRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/checks/` |
| `App.Backend.Infra.Exec` | `module` | `./packages/desktop/src-tauri/crates/infra/exec/` |
| `App.Backend.Infra.Store` | `module` | `./packages/desktop/src-tauri/crates/infra/store/` |
| `App.Backend.Infra.VcsRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/vcs/` |
| `App.Backend.Infra.Watch` | `module` | `./packages/desktop/src-tauri/crates/infra/watch/` |
| `App.Backend.Tools` | `domain` | 空 |
| `App.Backend.Tools.ExportBindings` | `module` | `./packages/desktop/src-tauri/crates/tools/export-bindings/` |
| `App.Backend.Tools.ArchiApiGen` | `module` | `./packages/desktop/src-tauri/crates/tools/archi-api-gen/` |
| `App.Backend.Shell` | `module` | `./packages/desktop/src-tauri/src/` |

- **命名**：元素段 SHALL 取 crate **裸名**的 CamelCase（`agent-runtime` → `AgentRuntime`），因此 core `agent` 与 infra `agent-runtime` 在 FQN 上天然可辨；壳取 `Shell`（包名 `dev-team` 是构建标识，不作架构元素名）。
- **覆盖判据（双向）**：「`crates/{core,infra,tools}/<dir>/` 目录集合 + `src-tauri/src/`」与「本要求的 `path` 集合」SHALL 相等——漏一个 crate 与多一个不存在目录同样不合格；比对时 SHALL 把下文所列的**既有已知缺口**单列为差集，其余两侧 MUST 严格相等（不允许以「缺口存在」为由整体放行）。
- **既有已知缺口（不在本变更补齐）**：`crates/tools/specta-zod-patched`（vendored 上游 exporter，`[package] name = "specta-zod"`，patch 面与跟版核对登记在 `desktop-ipc-type-bindings`；上游合入对应能力即整目录删除）先于本变更进入 workspace 而尚未建元素——该缺口**不是**本变更引入的，故本变更 MUST NOT 以「顺手补齐」为由扩范围（其补缺或豁免判据由独立变更承载），也 MUST NOT 以该缺口反推本变更新增的 `App.Backend.Tools.ArchiApiGen` 不成立。本变更后**自研 crate** 集合 SHALL 为 16（`core` 7 + `infra` 7 + `tools` 2：`export-bindings` 与 `archi-api-gen`），逐一对上元素 `path`。
- **漂移纪律**：新增 workspace crate SHALL 在引入它的同一 change 内追加元素与容器归属（MUST NOT 等到「下次整理模型」）。既有两个适用实例：`crates/infra/archi` → `App.Backend.Infra.ArchiRuntime`（三条边、`03` 段计数）；**本变更的 `crates/tools/archi-api-gen` → `App.Backend.Tools.ArchiApiGen`**（两条边、`05` 段计数 MUST 与本变更同 change 落地）。

#### Scenario: crate 目录与元素双向覆盖

- **WHEN** 枚举 `crates/core/*`、`crates/infra/*`、`crates/tools/*` 目录名与模型 `path` 集合比对
- **THEN** 枚举到的 17 个目录中，16 个**自研** crate 目录与 16 个 crate 元素两侧相等（`core` 7 / `infra` 7 / `tools/export-bindings` + `tools/archi-api-gen`，无漏无多）；唯一差集恰为 `crates/tools/specta-zod-patched` 一条既有已知缺口且被单列在案，其余任何差集即不合格；`App.Backend.Shell` 覆盖 `src-tauri/src/`

#### Scenario: crate 内部不建元素

- **WHEN** 检查模型内元素 FQN 与 `crates/*/*/src/**.rs` 文件名的对应关系
- **THEN** 不存在对 crate 内部模块（如 `agent::runner`、`archi::parse`、`archi-runtime::runner`、`archi-api-gen` 的 `api` 子模块族）的元素声明；`archi` 的 `parse` / `graph` / `dto` / `api` 与 `archi-runtime` 的 `runner` / `report` 等子模块只出现在对应 crate 的 `summary` 与文档描述里，不在元素表内

#### Scenario: 新增 crate 未声明即不合格

- **WHEN** 在 `crates/infra/` 下新增一个 crate 目录而模型未追加元素
- **THEN** 双向覆盖核对出现「目录多一个」的差集（既有 `specta-zod-patched` 缺口之外的新差集），判据不合格（本要求不因「下次一起补」而豁免）

#### Scenario: 本变更新增 crate 同 change 补齐

- **WHEN** 检查 `crates/infra/archi` 与 `03-backend-infra.toml`
- **THEN** `App.Backend.Infra.ArchiRuntime` 在案（`kind = "module"`、`gate = off`、`path = ["./packages/desktop/src-tauri/crates/infra/archi/"]`、`summary` 非空且不复述 FQN）；其三条出边与 `Cargo.toml` `[dependencies]` 的 workspace 内声明逐条相等（`Core.Archi` / `Core.Foundation` / `Core.Orchestration`）；`03` 段表头与计数表均为 **8 元素 / 24 关系**

#### Scenario: 新增工具 crate 同 change 补齐

- **WHEN** 检查 `crates/tools/archi-api-gen` 与 `05-backend-tools.toml`
- **THEN** `App.Backend.Tools.ArchiApiGen` 在案（`kind = "module"`、`gate = off`、`path = ["./packages/desktop/src-tauri/crates/tools/archi-api-gen/"]`、`summary` 非空且不复述 FQN）；其两条出边与 `Cargo.toml` `[dependencies]` 的 workspace 内声明逐条相等（`Core.Archi` / `Core.Foundation`）；`05` 段表头与计数表均为 **3 元素 / 3 关系**

### Requirement: 后端关系只取 Cargo 依赖声明

后端关系 SHALL 恰为 **`Cargo.toml` `[dependencies]` 段内指向 workspace 内包的声明**（两种书写形式均计：`{ workspace = true }` 与 `{ path = … }`），方向为**依赖者 → 被依赖者**；`[dev-dependencies]` 与 `[build-dependencies]` SHALL 排除。

- **dev 边必须排除**（不是风格偏好）：`workflow --dev--> orchestration` 与生产边 `orchestration --> workflow` 并存，纳入即 `cycle`、整张图停摆。dev 依赖的事实由 `Cargo.toml` 自身承载，模型 MUST NOT 重复表达。
- **反向边是合法事实**：`App.Backend.Tools.ExportBindings → App.Backend.Shell`（`path = ../../../`）SHALL 在案——工具 crate 为导出命令面 bindings 而依赖壳包，这一「工具依赖壳」的倒置是本基线的显式产出之一。
- **包含关系 MUST NOT 连边**：属于同一 crate 分组由 FQN 前缀表达；`App.Backend.Core.Workflow → App.Backend.Core` 一类「子 → 父」边 MUST NOT 出现（写入即 `dependsOnAncestor` 停摆）。
- **本变更新增的四条边**：`App.Backend.Infra.ArchiRuntime → App.Backend.Core.Archi` / `→ App.Backend.Core.Foundation` / `→ App.Backend.Core.Orchestration`（`archi-runtime` 的 `[dependencies]`）与 `App.Backend.Shell → App.Backend.Infra.ArchiRuntime`（壳的 `[dependencies]`，装配注入 runner）SHALL 在案，`desc` 各写一句用途；外部依赖（`serde_json` 与 `archi` 侧 `oxc_*` / `syn` / `sha2` 等）MUST NOT 连边。
- **本变更新增的两条边**：`App.Backend.Tools.ArchiApiGen → App.Backend.Core.Archi`（契约解析 / 装配 / 判据 / 渲染复用判定域）与 `→ App.Backend.Core.Foundation`（契约目录路径派生）SHALL 在案，`desc` 各写一句用途；外部依赖（`serde_json`）MUST NOT 连边；`archi-api-gen` MUST NOT 依赖壳（与 `export-bindings` 的反向边形态不同——它不触 bindings 导出）。
- **`desc`**：一句中文用途（如 `Store → Workflow` 写「change 域记录的类型依赖」），MUST NOT 复制 `Cargo.toml` 段落位置。

#### Scenario: 边集合与 Cargo 声明逐条相等

- **WHEN** 枚举 16 个自研 crate 与壳的 `[dependencies]` 段内 workspace 内声明，与模型 `[[relation]]`（`from` 为后端元素者）比对
- **THEN** 两侧逐条相等（本期 49 条，`02` 段 9 条 + `03` 段 24 条 + `04` 段 13 条 + `05` 段 3 条）；`App.Backend.Infra.Watch` 与 `App.Backend.Core.Agent` 出边为零；`App.Backend.Shell` 出边 13 条

#### Scenario: dev 边零出现

- **WHEN** 检查模型内是否存在 `App.Backend.Core.Workflow → App.Backend.Core.Orchestration` 与 `App.Backend.Core.Orchestration → App.Backend.Infra.Store` 两条
- **THEN** 两条均不在模型内（它们来自 `[dev-dependencies]`）；模型聚合后无 `cycle` 诊断

#### Scenario: 反向边在案且含父子禁边检查

- **WHEN** 查询 `App.Backend.Tools.ExportBindings` 的出边，并审查全部后端边是否含「子 → 父」
- **THEN** 出边恰为 `App.Backend.Shell` 一条（`desc` 说明为导出 bindings 而依赖壳）；全部后端边中零 `dependsOnAncestor` 形态（`ArchiRuntime` 的三条边全为「infra → core / 同层」，`ArchiApiGen` 的两条边为「tools → core」，均不含「子 → 父」）

#### Scenario: 新增工具 crate 的两条边在案

- **WHEN** 查询 `App.Backend.Tools.ArchiApiGen` 的出边并与 `crates/tools/archi-api-gen/Cargo.toml` 的 `[dependencies]` 比对
- **THEN** 出边恰为 `App.Backend.Core.Archi` 与 `App.Backend.Core.Foundation` 两条（各带一句中文 `desc`）；`serde_json` 等外部依赖零连边；`05` 段关系数为 3

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

### Requirement: 图自洽与停摆禁令
基线模型 SHALL 使四类结构诊断**全部零命中**：`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`；`archi_graph(root)` SHALL 返回正常态（元素与关系完整、`diagnostics` 为空），MUST NOT 停摆。

- **祖先显式声明**：所有容器元素 SHALL 在模型内声明（层级由 FQN 前缀推导，孤儿层级即 `orphanHierarchy`）；MUST NOT 依赖「隐式祖先自动补建」（本能力体系的既有前提）。
- **停摆零容忍**：交付面 MUST NOT 以「结构错 + 停摆面板」形态收尾——停摆是坏模型的处置，不是基线的可接受状态。
- **`docNotFound` 零告警**：`docs` 只指现存根文档、`path` 指向现存目录 / 文件，故 `warnings` SHALL 为空。

#### Scenario: 正常态而非停摆态

- **WHEN** 调用 `archi_graph(root)`
- **THEN** 返回正常态：`elements` 53、`relations` 120（后端 49 + 前端 71）、`diagnostics` 空、`warnings` 空；`/architecture` 页面呈现左栏树 + 画布 + 抽屉三段，无结构错面板、零「修好后再看」文案

#### Scenario: 成环与祖先禁边被机械拦住

- **WHEN** 假设模型被写成含 `orchestration ⇄ workflow`（纳入 dev 边）或 `Standard.Dailog → Standard`（把 hooks 归给容器）一类错误
- **THEN** 结构诊断立即返回 `cycle` / `dependsOnAncestor` 且画布停摆——该写法在交付面上不可通过（基线自身 MUST 为这两种写法的反例）

#### Scenario: 孤儿层级零命中

- **WHEN** 检查每个叶元素的 FQN 前缀链
- **THEN** 每一级祖先（`App` → `App.Backend` → `App.Backend.Core` → crate；`App` → `App.Frontend` → `Views.Components` → 叶）均在模型内显式声明，`orphanHierarchy` 零命中（含本变更新增的 `App.Backend.Tools.ArchiApiGen` 链：`App` → `App.Backend` → `App.Backend.Tools`）

### Requirement: 模型与裁定记录的可检索性

基线落位后，既有两个只读工具面（`archi_search` / `archi_module`）SHALL 在**零代码改动**下直接检索到模型内容与裁定记录，MUST NOT 需要新增命令、工具、索引或字段：

- **元素检索**：关键词 SHALL 匹配元素 FQN / `summary` / `metadata` 值拼合文本（ASCII 大小写不敏感子串 + 驼峰分词），故 `App.Backend.Core.Archi` 一类元素 SHALL 可被 `archi` 关键词命中，返回薄条目（`fqn` / `kind` / `gate` / `summary`）。
- **裁定记录检索**：ADR SHALL 经既有 `decisions/*.md` 文本源（`source = "text"`，正文含元素 FQN 即自动关联）成为元素的文档条目，从而按路径 / 文件名可被检索命中，且条目恒四字段（`path` / `title` / `kind` / `source`）；MUST NOT 把 ADR 路径写进元素 `docs` 强行关联（`docs` 只留给包根 `App`）。
- **一跳定位**：`archi_module(<FQN>)` SHALL 给出该元素的 `dependsOn` / `dependedBy` / `children` / `docs` 与 `modelFile` / `modelLine`（指向声明表头，可直接 `edit`），使 agent 能「查名 → 定位 → 改文件」闭环。
- **契约零变化**：本变更只是给两工具第一份非空语料，其注入矩阵、只读档位与响应形状 MUST NOT 因本变更改变（若形状有缺口按缺陷另议，不在本能力内扩字段）。

#### Scenario: 关键词检索同时命中元素与裁定记录

- **WHEN** 以 `query = "archi"`、`scope = all` 调用 `archi_search`
- **THEN** `elements` 含 `App.Backend.Core.Archi`、`App.Backend.Infra.ArchiRuntime`（`kind = module`、`gate = off`、`summary` 非空）与 `App.Frontend.Views.Architecture`；`docs` 含 `.dev-team/architecture/decisions/0001-model-granularity.md`（`kind = adr`、`source = text`、恰四字段且不含正文）

#### Scenario: 单元素详情给出一跳邻域与定位字段

- **WHEN** 调用 `archi_module(element = "App.Backend.Core.Archi")`
- **THEN** `dependsOn` 恰一条 `App.Backend.Core.Foundation`，`dependedBy` 含 `App.Backend.Infra.AgentRuntime`、`App.Backend.Infra.ArchiRuntime` 与 `App.Backend.Shell`，`children` 为空，`warnings` 为空；`element.modelFile` = `.dev-team/architecture/models/02-backend-core.toml` 且 `modelLine` 指向该元素表头行

### Requirement: 覆盖判据的机械核对与漂移纪律
元素的完整性与关系的证据性 SHALL 可**机械核对**（不靠人读图判断），核对口径恰为三条：

| # | 核对 | 判据 |
|---|---|---|
| 1 | 双向覆盖 | 判据命中的 crate 目录 / 视图目录 / 视图内子域 / 共享组件文件 / `components/ui/` 集合 == 模型叶元素 `path` 集合（无漏无多；**既有已知缺口**单列、不计入判红、其余零差集——本期三条：`crates/tools/specta-zod-patched`、`components/file/file-viewer.tsx`、`views/architecture/agent/`，与边面缺口清单同批登记在裁定记录 `§8`） |
| 2 | 关系证据 | 后端逐边等于 `Cargo.toml` `[dependencies]` 的 workspace 内声明集；前端逐边可指到 `import` 语句（**命令面边**例外：它是声明式约定，扫描面无证据、落 `observations`，见 `§8`） |
| 3 | 图自检 | `archi_graph` 正常态、`diagnostics` 空、`warnings` 空 |

- 本期核对 SHALL 以**命令 + 留痕**执行（当次 change 的 `reports/archi/` 核对留痕内），MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**` 作语料——crate 语料必须自足可移植，耦合本仓内容会让 crate 单测随仓库演进变红。机制化（漂移判定进流程门禁）归 `desktop-archi-gate`；本能力只定义模型内容与核对判据。
- **漂移纪律**：新增 / 删除 crate、视图、视图内子域、共享组件，或新增跨元素 `import` 依赖时，SHALL 在同一 change 内同步模型（元素 / 关系 / `path`），MUST NOT 留待「下次整理」；判据之外的元素与关系 MUST NOT 存在（「觉得该有」不是判据）。
- **「同一 change 内同步」的核对口径（本变更新增）**：当 change 新增 crate 时，核对 SHALL 同时看三件事——元素与容器归属在案、该 crate 的 `[dependencies]` 边逐条在案、涉及文件的计数（元素 / 关系）同步为同值；三者缺一即判不合格（只补元素不补边、或只改模型不改计数，都是漂移）。已知缺口清单 SHALL 只含「先于该 change 存在」的条目，MUST NOT 用「已在缺口清单里」为本次新增项开脱。本期前端边面缺口 SHALL 逐条列在裁定记录 `§8`（11 条已建元素间的跨元素 `import` 未声明），普查口径为「`06` 已建元素覆盖范围内文件的跨元素 `import`」与「`[[relation]]` 集合」两侧求差；缺口是既有事实、MUST NOT 被抹成零差集结论。

#### Scenario: 双向覆盖零差集

- **WHEN** 以判据枚举两侧集合并求差集
- **THEN** 两个方向差集均为空；任一新增目录 / 文件未声明即出现非空差集，核对失败（唯一例外为裁定记录 `§8` 的既有已知缺口清单：`crates/tools/specta-zod-patched`、`components/file/file-viewer.tsx`、`views/architecture/agent/` 三条 SHALL 各自单条出现，MUST NOT 被算作本变更的差集，也不得掩盖其它差集）

#### Scenario: 判据之外零元素零边

- **WHEN** 审查模型内元素与关系的来源
- **THEN** 每个元素可归属到某条覆盖判据（crate / 视图 / 子域 / 共享组件 / `ui` / 壳 / 容器），每条关系可归属到 `Cargo.toml` 声明或 `import` 语句；无「直觉元素」与「直觉边」

#### Scenario: 漂移在同一变更内补齐

- **WHEN** 某 change 新增一个视图并新增一条跨视图 `import`
- **THEN** 该 change 的交付内含新增视图元素与其关系；未补齐时双向覆盖核对与「有 import 无关系」抽查同时失败

#### Scenario: 新增 crate 的三件套核对

- **WHEN** 对新增 `crates/infra/archi` 的 change 执行覆盖核对
- **THEN** 三件事同时成立：`App.Backend.Infra.ArchiRuntime` 元素与 `App.Backend.Infra` 容器归属在案；`archi-runtime` 的三条 `[dependencies]` 边与壳指向它的那条边逐条在案；`03` 段 8 元素 / 24 关系与全基线 53 / 120 在模型表头、计数表与 ADR `§9` 三处同值——缺任一件即不合格

### Requirement: 本仓架构域看护配置与排除留痕
本仓库 SHALL 落位一份**架构域看护配置** `<archi_dir>/config.json`（本期落点 `.dev-team/architecture/config.json`），内容 SHALL 恰为扫描排除一条：

```json
{ "exclude": ["openspec/**"] }
```

- **字段面只用 `exclude`**：本次不引入 `ignore`（零豁免条目）——配置缺失与「有配置但 `ignore` 为空」在语义上等价，故 MUST NOT 为凑形态写空 `ignore` 数组。
- **MUST NOT 带 `$schema` 键**：该文件的读取面是 `desktop-archi-drift`「架构域看护配置」的严格解析（`deny_unknown_fields`，只登记 `ignore` / `exclude` 两键，`$schema` 既无 `rename` 也无容忍分支）；写 `$schema` 会让配置解析直接 `Err`，同时打断漂移命令、门禁 runner 与文件树三条消费面。本文件与工作区 `.dev-team/config.json`（带 `$schema`）形态不同，两者 MUST NOT 类比照抄。
- **排除理由**：`openspec/` 下存在**未被 gitignore** 的源文件（实测 `openspec/changes/archive/2026-09-28-agent-component-ai-sdk/prototypes/{index,agent-input,agent-messages}.tsx` 三个原型），它们不在 `decisions/0001-model-granularity.md` §7 的封闭排除面内、也不被默认测试面约定命中，故必须经配置排除，MUST NOT 以「代码里写死一条 `openspec/` 前缀」替代（那会让「什么进扫描面」变成第二套判据）。
- **连带留痕（必须如实登记，不是零改动）**：该文件生效后，上述三个文件在漂移报告中由 `scan.unmatchedFiles[]` 移入 `scan.excluded[]`（`{ file, rule }`，`rule = "openspec/**"`）——`unmatchedFiles` −3 / `excluded` +3；文件树的枚举面（`desktop-archi-file-tree`「枚举面与漂移判定面逐条一致」）随之同源变化（实测进树文件 667 → 358 中 −3 归此、−306 归测试面约定）。判定规则、`status` 推导与报告结构零变化。
- **模型资产零变化**：该配置 MUST NOT 引起任何模型改动——元素 / 关系 / `path` / `gate` / 计数（53 元素 / 120 关系、`06` 段 30 / 71）与裁定记录 `§9` 复算表逐字不变；`views/architecture/**` 已是单元素 `App.Frontend.Views.Architecture`，本变更在该目录内新增的 `components/file-tree.tsx` / `hooks/use-archi-file-tree.ts` 不建元素、不贡献新边（视图内 `lib/` 与 `hooks/` 的依赖计入该视图元素）。
- **工作区配置零触碰**：本文件 MUST NOT 被工作区配置 crate 读取；`.dev-team/config.json` 的 `KNOWN_KEYS`、`config.schema.json` 镜像与前端 config 页零变化（归属面写死：工作区配置管执行，架构域配置管看护策略）。

#### Scenario: 配置文件在案且形态正确

- **WHEN** 读取 `.dev-team/architecture/config.json`
- **THEN** 内容为 `{ "exclude": ["openspec/**"] }`（单键、单条、目录形 glob）；`grep '$schema'` 该文件零命中；`archi_drift` 与 `archi_file_tree` 均正常返回（配置解析零 `Err`）

#### Scenario: 排除生效与报告留痕

- **WHEN** 在真仓执行全量漂移检查与文件树取数
- **THEN** 漂移报告 `scan.excluded[]` 含 `openspec/changes/archive/2026-09-28-agent-component-ai-sdk/prototypes/{index,agent-input,agent-messages}.tsx`（`rule = "openspec/**"`），这三条 MUST NOT 出现在 `scan.unmatchedFiles[]`；文件树中 `openspec/` 前缀下零节点

#### Scenario: 模型与计数零漂移

- **WHEN** 对比配置加入前后的模型资产与计数复述
- **THEN** `models/` 六个文件逐字不变（53 元素 / 120 关系、`06` 段 30 / 71、`gate` 全 `off`）、`decisions/0001-model-granularity.md` 的 `§9` 复算表逐字不变；`archi_graph(root)` 仍为正常态（零诊断、零告警）

#### Scenario: 工作区配置零影响

- **WHEN** 审查工作区 `.dev-team/config.json`、`crates/core/config` 的 `KNOWN_KEYS` 与 `config.schema.json` 镜像
- **THEN** 三者与本变更前逐字一致；架构域配置只经 `foundation::layout::archi_config_path` 派生路径读取，未被工作区配置 crate 读取

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.dev-team/architecture/models/01-app.toml` | 包根与域容器 | `App`（`package`，`docs` 指 `./README.md` / `./AGENT.md`）、`App.Backend`、`App.Frontend`；零关系 |
| `.dev-team/architecture/models/02-backend-core.toml` | core 引擎侧分层 | `App.Backend.Core` 容器 + 7 crate 元素；9 条 core 内边（`foundation` 与 `agent` 无出边，其余 core crate 只向内依赖） |
| `.dev-team/architecture/models/03-backend-infra.toml` | infra 适配侧分层 | `App.Backend.Infra` 容器 + **7** crate 元素（新增 `ArchiRuntime`，`path` 指 `./packages/desktop/src-tauri/crates/infra/archi/`、`gate = off`、中文 `summary`）；**24** 条边（新增 `ArchiRuntime → Core.Archi` / `Core.Foundation` / `Core.Orchestration`；`store → workflow` 跨界边与其余 21 条不变） |
| `.dev-team/architecture/models/04-backend-shell.toml` | Tauri 壳 | `App.Backend.Shell`（`path` = `src-tauri/src/`）+ **13** 条装配出边（新增 `→ App.Backend.Infra.ArchiRuntime`，`desc` 写「架构门禁步骤的 runner 构造与注入」）；加一行 `entry = "./packages/desktop/src-tauri/src/lib.rs"`（单值、`./` 前缀、落在 `path` `./packages/desktop/src-tauri/src/` 之内）；`kind` / `gate` / `path` / `summary` 与 13 条出边逐字不变，表头计数仍为「1 元素 / 13 关系」 |
| `.dev-team/architecture/models/05-backend-tools.toml` | 工具层模型数据 | `App.Backend.Tools` 容器 + `ExportBindings` + **`ArchiApiGen`** 三元素 / 3 关系；表头注释同步为「3 元素 / 3 关系」；反向边 `ExportBindings → App.Backend.Shell` 与 `ArchiApiGen → Core.Archi` / `Core.Foundation` 各一句中文 `desc`；两个工具 crate 各新增 `api = "none"` 与一行 `entry`（`.../crates/tools/export-bindings/src/main.rs` / `.../crates/tools/archi-api-gen/src/main.rs`），两行 `api = "none"` **保留不动**；3 条关系与表头计数（3 元素 / 3 关系）逐字不变 |
| `.dev-team/architecture/models/06-frontend.toml` | 前端视图与组件层 | 30 元素 = **3 容器**（`Views` / `Components` / `Standard`）+ **27 叶**（`Shell` / 9 视图 / 3 子域 / 12 共享组件 / `Components.Agent`（`kind = "module"`、`path = ./packages/desktop/src/components/agent/`、**零 `entry`**）/ `Ui`）+ **71 条 import 边**（24 个源；`Ui` 恒为被依赖方，`Components.Agent` 的 3 出 / 4 入共 7 条 barrel 端点边）；表头注释为「3 个容器」 |
| `.dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra}.toml` | 其余模型数据 | 零改动（元素 / 关系 / `path` 逐字不变；`gate` 全 `off` 不动；03 段 8 / 24 不变；`entry` 零行——单文件根与 14 个 `src/lib.rs` 命中的 crate 均由约定默认覆盖） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 粒度与口径裁定 | §3 为「容器 `path` 恒空（零例外）；barrel 是目录模块的入口，不是容器的特权」（7 条端点边与「父 import 子合法」两句保留）；§1 / §9 计数为 53 / 120（容器 9 + 叶 44、前端 71）、后端 49、`Shell` 出边 13、`05` 段 3 / 3、`06` 段 30 / 71、barrel 端点边 3 出 + 4 入 = 7；§2 子域计数 `flow` 16 / `renderers` 8 / `agent` 4、ui 39 文件、`views/changes/components` 2 文件；`entry` / `api` 是字段面扩容、不构成新粒度判据，故 ADR MUST NOT 为此新增判据条目 |
| `openspec/changes/**/reports/**`（本 change 内） | 核对留痕 | 双向覆盖差集、后端边逐条比对、前端边 import 证据清单三份结果 |
| 核对口径（命令 + 留痕） | 双向覆盖比对 | `crates/{core,infra,tools}/<dir>/` 目录集合 ↔ 元素 `path` 集合（自研 16 ↔ 16，`specta-zod-patched` 单列差集）；`archi_graph(root)` 正常态 53 / 120 零诊断零告警 |
| `crates/core/archi/**`（模型读面） | 只读消费方 | `parse_dir` 聚合、`ModelGraph` 校验、`dto` 投影与结构诊断零改动（本变更只在同 crate 新增 `scan/` / `drift/`）；模型 MUST NOT 因新增扫描面扩字段 |
| `src/views/architecture/**` | 画布 | 只被使用：`/architecture` 正常态渲染 53 节点 / 120 边；视图内 `lib/` 与 `hooks/` 不建元素（粒度 ≠ 解析面），本变更新增的 `lib/drift-marks.ts` 与 `hooks/use-archi-drift.ts` 依赖计入 `Views.Architecture` |
| `.dev-team/architecture/config.json`（新增） | 架构域看护配置 | `{ "exclude": ["openspec/**"] }`；**不带 `$schema`**（读取面 `deny_unknown_fields` 只认 `ignore` / `exclude`）；生效后 `openspec/` 下三个原型 tsx 由 `unmatchedFiles[]` 移入 `excluded[]` |
| `.dev-team/architecture/models/**`（一级六 toml）/ `decisions/0001-model-granularity.md` | 模型与裁定记录（零内容改动） | 元素 / 关系 / `path` / `gate` / 计数与 ADR 复算表逐字不变；ADR §1 / §2 / §3 / §9 的计数与口径本变更零触及（契约目录不在其计数面内） |
| `.dev-team/architecture/models/<Domain>/**`（契约资产迁移落点） | 迁移 | 由 `<archi_dir>/api/<Domain>/**` 搬入：`<FQN>.md`（唯一手写）+ `<FQN>.schema.json`（生成物）；不进模型收面（子目录不递归） |
| `.dev-team/architecture/graph/INDEX.toml`（契约索引迁移落点） | 迁移 | 由 `<archi_dir>/api/INDEX.toml` 搬入并重生成；不在 `models/` 内 ⇒ 不进模型收面，`modelHash` 零变化 |
| `.dev-team/architecture/api/**` | **删除** | `api` 段退役；迁移后该目录不存在（缺席由迁移动作与目录树读数守，不进 `api:check` 的枚举面） |
| `packages/desktop/src/views/architecture/**`（见 `desktop-archi-file-tree`） | 画布（零模型漂移） | 该目录已是单元素 `App.Frontend.Views.Architecture`；新增 `components/file-tree.tsx` 与 `hooks/use-archi-file-tree.ts` 不建元素，其 import 计入该视图元素（计数零变化） |
