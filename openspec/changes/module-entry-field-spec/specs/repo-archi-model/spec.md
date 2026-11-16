# repo-archi-model Specification（delta: module-entry-field-spec）

## Purpose

本 delta 只补**字段纪律与两处模型数据**：`entry` 字段的基线口径（只在约定默认指不到时声明；本变更落地时**零显式声明**，三处目标值随宿主重建后的后续变更归位）、两个工具 crate 的 `api = none` 显式留白，并把「无入口」与「无 API」两类空态各给一个实例。元素与关系计数（51 / 113、逐文件分项）、`gate` 全 `off`、`path` / `docs` / `metadata` / `desc` 纪律**逐字不动**。

> **数据分两批的理由（写进 spec 以免后人「顺手补齐」）**：架构漂移门禁在**宿主进程内**用宿主内嵌的 archi crate 解析模型。模型里出现宿主不认识的字段面（如本变更新增的 `entry`）时，该文件必被判 `unknownField`，其元素随之「未声明」并连带 `unknownReference`（实测 13 条结构错 ⇒ `status = errors`）——这是**版本耦合**，不是数据错误。故新字段面的数据 SHALL 与「宿主已重建并重启」同批落地；`api` 属既有字段面，可先落。

## MODIFIED Requirements

### Requirement: 基线落位与模型文件划分

架构模型基线 SHALL 落位 `<archi_dir>/models/`（架构域根经 `foundation::layout::archi_dir` 派生，本期落点 `.dev-team/architecture/models/`），MUST NOT 落到 `src/`、`docs/`、`openspec/specs/` 或任何其它位置。

模型 SHALL 划分为**六个**文件，文件名形如 `NN-<域>.toml`，字典序即聚合序，每个元素与每条关系 SHALL 恰在一处声明（跨文件或同文件重复即 `duplicateElement`）：

| 文件 | 内容 | 元素 | 关系 |
|---|---|---|---|
| `01-app.toml` | 包根与两个域容器 | 3 | 0 |
| `02-backend-core.toml` | `Core` 容器 + 7 个 core crate 元素 | 8 | 9 |
| `03-backend-infra.toml` | `Infra` 容器 + 7 个 infra crate 元素 | 8 | 24 |
| `04-backend-shell.toml` | Tauri 壳（命令面 / 装配 / bindings 导出） | 1 | 13 |
| `05-backend-tools.toml` | `Tools` 容器 + `export-bindings` crate + `archi-api-gen` crate | 3 | 3 |
| `06-frontend.toml` | 前端 `Shell` / 9 视图 / 3 视图内子域 / 10 共享组件 / `Ui` + 四个容器（`App.Frontend` 容器在 `01`） | 28 | 64 |

合计 SHALL 为 **51 元素 / 113 关系**，逐文件可加：元素侧 `3 + 8 + 8 + 1 + 3 + 28 = 51`（容器 10 + 叶 41），关系侧 `0 + 9 + 24 + 13 + 3 + 64 = 113`。关系侧另按取数口径分：**113 = 后端 49（`Cargo.toml` `[dependencies]` 的 workspace 内声明，拆在 `02`–`05`：9 + 24 + 13 + 3）+ 前端 64（`import` 证据，全在 `06`）**；两种拆分 MUST 得同值。前端 64 的构成 SHALL 为 **58 条叶 → 叶 / 叶 → 容器边 + 6 条 `App.Frontend.Components.Agent` 的 barrel 端点边**（2 条出边：`index.tsx` 重导出 `AgentInput` / `AgentSessionView`；4 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）——这 6 条由「容器 `path` 唯一例外 = 持 barrel」与「`@/components/agent` → `Components.Agent` 映射」两条规则必然导出，MUST NOT 从记数中剔除。其中 `06-frontend.toml` 的 28 元素 = **4 个容器**（`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Agent` / `App.Frontend.Components.Standard`）+ **24 个叶**（`Shell` 1 + 9 视图 + 3 子域 + 10 共享组件 + `Ui` 1）；`App.Frontend` 容器 SHALL 只在 `01-app.toml` 声明一次，`06` MUST NOT 重复声明（重复即 `duplicateElement`，并会使元素总数变成 52 而非 51）。

- **关系归属**：每条 `[[relation]]` SHALL 声明在**其 `from` 元素所在文件**内（定位一致、diff 不跨文件散落）。
- **`gate` 全 `off`**：本基线内每个元素 SHALL 省略 `gate` 或显式写 `off`，MUST NOT 出现 `warn` / `strict`——冷启动 ratchet（收紧按模块独立变更，一个 change 收紧一块）。
- **`summary`**：每个元素 SHALL 有一行中文职责说明，MUST NOT 用 FQN 字面复述自身名字。
- **`path`**：叶元素必填、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器元素 SHALL 留空，唯一例外是持有 barrel 的 `App.Frontend.Components.Agent`（`path` 为其 `index.tsx`）。
- **`entry`**：叶元素 SHALL **仅在约定默认指不到时**声明（单值、`./` 前缀、repo 相对）；单文件根元素（`views/welcome-view.tsx`、`components/agent/*.tsx`、`standard/*.tsx`、持 barrel 的 `App.Frontend.Components.Agent`）与 14 个 `src/lib.rs` 命中的 crate MUST NOT 声明（根文件自身 / 语言约定默认已给出入口）。**本变更落地时显式 `entry` SHALL 为 0 处**，目标三处（下列值，随宿主重建后的后续变更归位，届时本条同步为「恰三处」）：
  - `App.Backend.Shell` → `./packages/desktop/src-tauri/src/lib.rs`（目录型壳的根路径 `src-tauri/src/` 拼出的约定默认是 `src-tauri/src/src/lib.rs`，指不到）；
  - `App.Backend.Tools.ExportBindings` → `./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs`；
  - `App.Backend.Tools.ArchiApiGen` → `./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`。

  未声明期间的后果 SHALL 被接受且可读：三者在报告里各产一条 `apiEntryMissing`（`Absent`），`api = none` MUST NOT 被用来豁免或掩盖它（两者是不同事实）。

  第四节「四条判据」（基准 / 落位 / 存在 / 跨元素唯一）与三态解析链归 `desktop-archi-api`，本节只钉基线数据；违规走 `apiFindings` 的 `apiEntryMissing`（`warn`，不影响 `passed`），MUST NOT 引入结构化诊断。
- **`api = none` 两处显式留白**：两个工具 crate（`App.Backend.Tools.ExportBindings` / `App.Backend.Tools.ArchiApiGen`）SHALL 声明 `api = none`——二进制 crate 的公共面是 CLI 契约而非库 API，这是**真话**而非「还没写」；其余元素 SHALL 省略 `api`（取缺省 `declared`）。「无入口」与「无 API」是两个不同事实，本基线两类都有实例（12 个前端目录根 + 壳 + 两个工具 crate 是无入口——后三者的显式 `entry` 归后续变更；两个工具 crate 是无 API），MUST NOT 混用。
- **`docs`**：SHALL 只由包根元素 `App` 显式指向仓库面文档（`./README.md` / `./AGENT.md`），其余元素留空；任何元素 MUST NOT 把 `openspec/specs/**` 路径写进 `docs`（守 `desktop-archi-module-docs` 的迁移边界：模型 MUST NOT 反向依赖将被替代的语料）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。
- **计数复述单源**：模型文件的表头注释（如 `# 03 · infra 适配层（8 元素 / 24 关系）`）、本要求的逐文件计数表、与裁定记录 `§9` 的复算表 SHALL 在同一次模型改动内同步为同值；三处任一处陈旧（仍写 50 / 111 / 47 / 2 元素 1 关系）即漂移，按「同一 change 内补齐」处理，MUST NOT 让计数表与模型本体各说各话。**字段面扩容（`entry` / `api`）不改变元素与关系计数**：51 / 113 与逐文件分项在本变更内逐字不变。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `models/` 下六个文件
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 51 条、`relations` 共 113 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` / `entry` / `api` 出现行
- **THEN** `gate` 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；`docs` 仅 `<archi_dir>` 之外的根文档两条；全部叶元素 `path` 以 `"./` 开头，全部容器 `path` 为空（`App.Frontend.Components.Agent` 一例除外）；`entry` **零行**（本变更落地时；宿主重建后随后续变更变为恰三行，各为单值字符串）；`api` 恰两行且均为 `"none"`（两个工具 crate）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `models/` 恰有六个 `NN-<域>.toml`，`decisions/` 有 `0001-model-granularity.md`；`src/`、`docs/`、`openspec/specs/` 下无模型文件；`archi_module("App")` 的 `docs` 中出现该 ADR 条目（`source = "text"`）

#### Scenario: 计数复述三处同值

- **WHEN** 逐处读取 `03` / `04` / `05` 文件表头注释、本要求的计数表与 ADR `§9` 复算表
- **THEN** 三处同值：全基线 51 元素 / 113 关系（后端 49 + 前端 64）、`05` 段 3 元素 / 3 关系、`03` 段 8 元素 / 24 关系、`04` 段 1 元素 / 13 关系；任一处仍写 50 元素 / 111 关系 / 47 关系 / 2 元素即不合格

#### Scenario: 入口与留白两类空态各归其元素

- **WHEN** 分别对壳、两个工具 crate、12 个前端目录根元素跑一次全量检查并读 `apiFindings`
- **THEN** 壳与两个工具 crate 各产一条 `apiEntryMissing`（`Absent`：未声明 `entry`，且目录型根的约定候选 `src-tauri/src/src/lib.rs` 与二进制 crate 的 `src/lib.rs` 皆不在盘）——`api = none` **不**豁免本判据（两工具 crate 同时表现为「无入口」与「无 API」两个事实）；12 个前端目录根各产一条 `apiEntryMissing`（`Absent`：无 `entry`、无 `index.ts`），MUST NOT 因「该模块确实有代码」被错误地算作有入口——补 barrel 属独立变更；反方向 MUST NOT 成立：14 个 `src/lib.rs` 命中的 crate 与前端 12 个单文件根各自由约定默认解析出入口，零 `apiEntryMissing`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.dev-team/architecture/models/04-backend-shell.toml` | Tauri 壳 | **零改动**：`App.Backend.Shell`（`path` = `./packages/desktop/src-tauri/src/`）与 13 条装配出边逐字不变；显式 `entry` 归后续变更（宿主重建后） |
| `.dev-team/architecture/models/05-backend-tools.toml` | 工具层模型数据 | `App.Backend.Tools` 容器 + `ExportBindings` + `ArchiApiGen` 三元素 / 3 关系；两个工具 crate **只**各新增 `api = "none"`（`entry` 归后续变更）；表头计数仍为「3 元素 / 3 关系」 |
| `.dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,06-frontend}.toml` | 其余模型数据 | 零改动（元素 / 关系 / `path` / `gate` 逐字不变；06 段 28 / 64 与 03 段 8 / 24 不变；`entry` 零命中——单文件根与 14 个 crate 均由约定默认覆盖） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 粒度与口径裁定 | 判据六条与计数复述零改动（51 / 113、后端 49、`Shell` 出边 13、`05` 段 3 / 3）；`entry` / `api` 是字段面扩容，不构成新粒度判据，故 ADR MUST NOT 为此新增判据条目 |
