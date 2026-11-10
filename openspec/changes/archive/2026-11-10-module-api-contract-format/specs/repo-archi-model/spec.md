# repo-archi-model Specification（delta: module-api-contract-format）

## MODIFIED Requirements

### Requirement: 基线落位与模型文件划分

架构模型基线 SHALL 落位 `<archi_dir>/models/`（架构域根经 `foundation::layout::archi_dir` 派生，本期落点 `.dev-team/architecture/models/`），MUST NOT 落到 `src/`、`docs/`、`openspec/specs/` 或任何其它位置。

模型 SHALL 划分为**六个**文件，文件名形如 `NN-<域>.toml`，字典序即聚合序，每个元素与每条关系 SHALL 恰在一处声明（跨文件或同文件重复即 `duplicateElement`）：

| 文件 | 内容 | 元素 | 关系 |
|---|---|---|---|
| `01-app.toml` | 包根与两个域容器 | 3 | 0 |
| `02-backend-core.toml` | `Core` 容器 + 7 个 core crate 元素 | 8 | 9 |
| `03-backend-infra.toml` | `Infra` 容器 + **7** 个 infra crate 元素（本变更新增 `ArchiRuntime`） | **8** | **24** |
| `04-backend-shell.toml` | Tauri 壳（命令面 / 装配 / bindings 导出） | 1 | **13** |
| `05-backend-tools.toml` | `Tools` 容器 + `export-bindings` crate + **`archi-api-gen` crate**（本变更新增） | **3** | **3** |
| `06-frontend.toml` | 前端 `Shell` / 9 视图 / 3 视图内子域 / 10 共享组件 / `Ui` + **四个**容器（`App.Frontend` 容器在 `01`） | 28 | 64 |

合计 SHALL 为 **51 元素 / 113 关系**，逐文件可加：元素侧 `3 + 8 + 8 + 1 + 3 + 28 = 51`（容器 10 + 叶 41），关系侧 `0 + 9 + 24 + 13 + 3 + 64 = 113`。关系侧另按取数口径分：**113 = 后端 49（`Cargo.toml` `[dependencies]` 的 workspace 内声明，拆在 `02`–`05`：9 + 24 + 13 + 3）+ 前端 64（`import` 证据，全在 `06`）**；两种拆分 MUST 得同值。前端 64 的构成 SHALL 为 **58 条叶 → 叶 / 叶 → 容器边 + 6 条 `App.Frontend.Components.Agent` 的 barrel 端点边**（2 条出边：`index.tsx` 重导出 `AgentInput` / `AgentSessionView`；4 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）——这 6 条由「容器 `path` 唯一例外 = 持 barrel」与「`@/components/agent` → `Components.Agent` 映射」两条规则必然导出，MUST NOT 从记数中剔除。其中 `06-frontend.toml` 的 28 元素 = **4 个容器**（`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Agent` / `App.Frontend.Components.Standard`）+ **24 个叶**（`Shell` 1 + 9 视图 + 3 子域 + 10 共享组件 + `Ui` 1）；`App.Frontend` 容器 SHALL 只在 `01-app.toml` 声明一次，`06` MUST NOT 重复声明（重复即 `duplicateElement`，并会使元素总数变成 52 而非 51）。

- **关系归属**：每条 `[[relation]]` SHALL 声明在**其 `from` 元素所在文件**内（定位一致、diff 不跨文件散落）。
- **`gate` 全 `off`**：本基线内每个元素 SHALL 省略 `gate` 或显式写 `off`，MUST NOT 出现 `warn` / `strict`——冷启动 ratchet（收紧按模块独立变更，一个 change 收紧一块）。
- **`summary`**：每个元素 SHALL 有一行中文职责说明，MUST NOT 用 FQN 字面复述自身名字。
- **`path`**：叶元素必填、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器元素 SHALL 留空，唯一例外是持有 barrel 的 `App.Frontend.Components.Agent`（`path` 为其 `index.tsx`）。
- **`docs`**：SHALL 只由包根元素 `App` 显式指向仓库面文档（`./README.md` / `./AGENT.md`），其余元素留空；任何元素 MUST NOT 把 `openspec/specs/**` 路径写进 `docs`（守 `desktop-archi-module-docs` 的迁移边界：模型 MUST NOT 反向依赖将被替代的语料）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。
- **计数复述单源**：模型文件的表头注释（如 `# 03 · infra 适配层（8 元素 / 24 关系）`）、本要求的逐文件计数表、与裁定记录 `§9` 的复算表 SHALL 在同一次模型改动内同步为同值；三处任一处陈旧（仍写 50 / 111 / 47 / 2 元素 1 关系）即漂移，按「同一 change 内补齐」处理，MUST NOT 让计数表与模型本体各说各话。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `models/` 下六个文件
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 51 条、`relations` 共 113 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` 出现行
- **THEN** 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；`docs` 仅 `<archi_dir>` 之外的根文档两条；全部叶元素 `path` 以 `"./` 开头，全部容器 `path` 为空（`App.Frontend.Components.Agent` 一例除外）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `models/` 恰有六个 `NN-<域>.toml`，`decisions/` 有 `0001-model-granularity.md`；`src/`、`docs/`、`openspec/specs/` 下无模型文件；`archi_module("App")` 的 `docs` 中出现该 ADR 条目（`source = "text"`）

#### Scenario: 计数复述三处同值

- **WHEN** 逐处读取 `03` / `04` 文件表头注释、本要求的计数表与 ADR `§9` 复算表
- **THEN** 三处同值：全基线 51 元素 / 113 关系（后端 49 + 前端 64）、`05` 段 3 元素 / 3 关系、`03` 段 8 元素 / 24 关系、`04` 段 1 元素 / 13 关系；任一处仍写 50 元素 / 111 关系 / 47 关系 / 2 元素即不合格

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

### Requirement: 图自洽与停摆禁令

基线模型 SHALL 使四类结构诊断**全部零命中**：`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`；`archi_graph(root)` SHALL 返回正常态（元素与关系完整、`diagnostics` 为空），MUST NOT 停摆。

- **祖先显式声明**：所有容器元素 SHALL 在模型内声明（层级由 FQN 前缀推导，孤儿层级即 `orphanHierarchy`）；MUST NOT 依赖「隐式祖先自动补建」（本能力体系的既有前提）。
- **停摆零容忍**：交付面 MUST NOT 以「结构错 + 停摆面板」形态收尾——停摆是坏模型的处置，不是基线的可接受状态。
- **`docNotFound` 零告警**：`docs` 只指现存根文档、`path` 指向现存目录 / 文件，故 `warnings` SHALL 为空。

#### Scenario: 正常态而非停摆态

- **WHEN** 调用 `archi_graph(root)`
- **THEN** 返回正常态：`elements` 51、`relations` 113（后端 49 + 前端 64）、`diagnostics` 空、`warnings` 空；`/architecture` 页面呈现元素树 + 画布 + 抽屉三段，无结构错面板、零「修好后再看」文案

#### Scenario: 成环与祖先禁边被机械拦住

- **WHEN** 假设模型被写成含 `orchestration ⇄ workflow`（纳入 dev 边）或 `Standard.Dailog → Standard`（把 hooks 归给容器）一类错误
- **THEN** 结构诊断立即返回 `cycle` / `dependsOnAncestor` 且画布停摆——该写法在交付面上不可通过（基线自身 MUST 为这两种写法的反例）

#### Scenario: 孤儿层级零命中

- **WHEN** 检查每个叶元素的 FQN 前缀链
- **THEN** 每一级祖先（`App` → `App.Backend` → `App.Backend.Core` → crate；`App` → `App.Frontend` → `Views.Components` → 叶）均在模型内显式声明，`orphanHierarchy` 零命中（含本变更新增的 `App.Backend.Tools.ArchiApiGen` 链：`App` → `App.Backend` → `App.Backend.Tools`）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.dev-team/architecture/models/05-backend-tools.toml` | 工具层模型数据 | `Tools` 容器 + `ExportBindings` + **`ArchiApiGen`** 三元素 / 3 关系；表头注释同步为「3 元素 / 3 关系」 |
| `.dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,04-backend-shell,06-frontend}.toml` | 其余模型数据 | 零改动（元素 / 关系 / `path` 逐字不变；`gate` 全 `off` 不动） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 判据 + 计数复述 | 六条判据零改动；`§1` / `§4` / `§6` / `§9` 计数同步为 51 元素 / 113 关系（后端 49、`Shell` 出边 13、`05` 段 3 / 3） |
| 核对口径（命令 + 留痕） | 双向覆盖比对 | `crates/{core,infra,tools}/<dir>/` 目录集合 ↔ 元素 `path` 集合（自研 16 ↔ 16，`specta-zod-patched` 单列差集）；`archi_graph(root)` 正常态 51 / 113 零诊断零告警 |
