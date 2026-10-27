# repo-archi-model Specification（delta: init-archi-models）

## ADDED Requirements

### Requirement: 基线落位与模型文件划分

架构模型基线 SHALL 落位 `<archi_dir>/models/`（架构域根经 `foundation::layout::archi_dir` 派生，本期落点 `openspec/architecture/models/`），MUST NOT 落到 `src/`、`docs/`、`openspec/specs/` 或任何其它位置。

模型 SHALL 划分为**六个**文件，文件名形如 `NN-<域>.toml`，字典序即聚合序，每个元素与每条关系 SHALL 恰在一处声明（跨文件或同文件重复即 `duplicateElement`）：

| 文件 | 内容 | 元素 | 关系 |
|---|---|---|---|
| `01-app.toml` | 包根与两个域容器 | 3 | 0 |
| `02-backend-core.toml` | `Core` 容器 + 7 个 core crate 元素 | 8 | 9 |
| `03-backend-infra.toml` | `Infra` 容器 + 6 个 infra crate 元素 | 7 | 21 |
| `04-backend-shell.toml` | Tauri 壳（命令面 / 装配 / bindings 导出） | 1 | 12 |
| `05-backend-tools.toml` | `Tools` 容器 + `export-bindings` crate | 2 | 1 |
| `06-frontend.toml` | 前端 `Shell` / 9 视图 / 3 视图内子域 / 10 共享组件 / `Ui` + **四个**容器（`App.Frontend` 容器在 `01`） | 28 | 64 |

合计 SHALL 为 **49 元素 / 107 关系**，逐文件可加：元素侧 `3 + 8 + 7 + 1 + 2 + 28 = 49`（容器 10 + 叶 39），关系侧 `0 + 9 + 21 + 12 + 1 + 64 = 107`。关系侧另按取数口径分：**107 = 后端 43（`Cargo.toml` `[dependencies]` 的 workspace 内声明，拆在 `02`–`05`：9 + 21 + 12 + 1）+ 前端 64（`import` 证据，全在 `06`）**；两种拆分 MUST 得同值。前端 64 的构成 SHALL 为 **58 条叶 → 叶 / 叶 → 容器边 + 6 条 `App.Frontend.Components.Agent` 的 barrel 端点边**（2 条出边：`index.tsx` 重导出 `AgentInput` / `AgentSessionView`；4 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）——这 6 条由「容器 `path` 唯一例外 = 持 barrel」与「`@/components/agent` → `Components.Agent` 映射」两条规则必然导出，MUST NOT 从记数中剔除。其中 `06-frontend.toml` 的 28 元素 = **4 个容器**（`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Agent` / `App.Frontend.Components.Standard`）+ **24 个叶**（`Shell` 1 + 9 视图 + 3 子域 + 10 共享组件 + `Ui` 1）；`App.Frontend` 容器 SHALL 只在 `01-app.toml` 声明一次，`06` MUST NOT 重复声明（重复即 `duplicateElement`，并会使元素总数变成 50 而非 49）。

- **关系归属**：每条 `[[relation]]` SHALL 声明在**其 `from` 元素所在文件**内（定位一致、diff 不跨文件散落）。
- **`gate` 全 `off`**：本基线内每个元素 SHALL 省略 `gate` 或显式写 `off`，MUST NOT 出现 `warn` / `strict`——冷启动 ratchet（收紧按模块独立变更，一个 change 收紧一块）。
- **`summary`**：每个元素 SHALL 有一行中文职责说明，MUST NOT 用 FQN 字面复述自身名字。
- **`path`**：叶元素必填、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器元素 SHALL 留空，唯一例外是持有 barrel 的 `App.Frontend.Components.Agent`（`path` 为其 `index.tsx`）。
- **`docs`**：SHALL 只由包根元素 `App` 显式指向仓库面文档（`./README.md` / `./AGENT.md`），其余元素留空；任何元素 MUST NOT 把 `openspec/specs/**` 路径写进 `docs`（守 `desktop-archi-module-docs` 的迁移边界：模型 MUST NOT 反向依赖将被替代的语料）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `models/` 下六个文件
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 49 条、`relations` 共 107 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` 出现行
- **THEN** 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；`docs` 仅 `<archi_dir>` 之外的根文档两条；全部叶元素 `path` 以 `"./` 开头，全部容器 `path` 为空（`App.Frontend.Components.Agent` 一例除外）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `models/` 恰有六个 `NN-<域>.toml`，`decisions/` 有 `0001-model-granularity.md`；`src/`、`docs/`、`openspec/specs/` 下无模型文件；`archi_module("App")` 的 `docs` 中出现该 ADR 条目（`source = "text"`）

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
| `App.Backend.Infra.ChecksRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/checks/` |
| `App.Backend.Infra.Exec` | `module` | `./packages/desktop/src-tauri/crates/infra/exec/` |
| `App.Backend.Infra.Store` | `module` | `./packages/desktop/src-tauri/crates/infra/store/` |
| `App.Backend.Infra.VcsRuntime` | `module` | `./packages/desktop/src-tauri/crates/infra/vcs/` |
| `App.Backend.Infra.Watch` | `module` | `./packages/desktop/src-tauri/crates/infra/watch/` |
| `App.Backend.Tools` | `domain` | 空 |
| `App.Backend.Tools.ExportBindings` | `module` | `./packages/desktop/src-tauri/crates/tools/export-bindings/` |
| `App.Backend.Shell` | `module` | `./packages/desktop/src-tauri/src/` |

- **命名**：元素段 SHALL 取 crate **裸名**的 CamelCase（`agent-runtime` → `AgentRuntime`），因此 core `agent` 与 infra `agent-runtime` 在 FQN 上天然可辨；壳取 `Shell`（包名 `dev-team` 是构建标识，不作架构元素名）。
- **覆盖判据（双向）**：「`crates/{core,infra,tools}/<dir>/` 目录集合 + `src-tauri/src/`」与「本要求的 `path` 集合」SHALL 相等——漏一个 crate 与多一个不存在目录同样不合格。
- **漂移纪律**：新增 workspace crate SHALL 在引入它的同一 change 内追加元素与容器归属（MUST NOT 等到「下次整理模型」）。

#### Scenario: crate 目录与元素双向覆盖

- **WHEN** 枚举 `crates/core/*`、`crates/infra/*`、`crates/tools/*` 目录名与模型 `path` 集合比对
- **THEN** 两侧集合相等（14 个 crate 目录 ↔ 14 个 crate 元素，无漏无多）；`App.Backend.Shell` 覆盖 `src-tauri/src/`

#### Scenario: crate 内部不建元素

- **WHEN** 检查模型内元素 FQN 与 `crates/*/*/src/**.rs` 文件名的对应关系
- **THEN** 不存在对 crate 内部模块（如 `agent::runner`、`archi::parse`）的元素声明；`archi` 的 `parse` / `graph` / `dto` 等子模块只出现在该 crate 的 `summary` 与文档描述里，不在元素表内

#### Scenario: 新增 crate 未声明即不合格

- **WHEN** 在 `crates/infra/` 下新增一个 crate 目录而模型未追加元素
- **THEN** 双向覆盖核对出现「目录多一个」的差集，判据不合格（本要求不因「下次一起补」而豁免）

### Requirement: 后端关系只取 Cargo 依赖声明

后端关系 SHALL 恰为 **`Cargo.toml` `[dependencies]` 段内指向 workspace 内包的声明**（两种书写形式均计：`{ workspace = true }` 与 `{ path = … }`），方向为**依赖者 → 被依赖者**；`[dev-dependencies]` 与 `[build-dependencies]` SHALL 排除。

- **dev 边必须排除**（不是风格偏好）：`workflow --dev--> orchestration` 与生产边 `orchestration --> workflow` 并存，纳入即 `cycle`、整张图停摆。dev 依赖的事实由 `Cargo.toml` 自身承载，模型 MUST NOT 重复表达。
- **反向边是合法事实**：`App.Backend.Tools.ExportBindings → App.Backend.Shell`（`path = ../../../`）SHALL 在案——工具 crate 为导出命令面 bindings 而依赖壳包，这一「工具依赖壳」的倒置是本基线的显式产出之一。
- **包含关系 MUST NOT 连边**：属于同一 crate 分组由 FQN 前缀表达；`App.Backend.Core.Workflow → App.Backend.Core` 一类「子 → 父」边 MUST NOT 出现（写入即 `dependsOnAncestor` 停摆）。
- **`desc`**：一句中文用途（如 `Store → Workflow` 写「change 域记录的类型依赖」），MUST NOT 复制 `Cargo.toml` 段落位置。

#### Scenario: 边集合与 Cargo 声明逐条相等

- **WHEN** 枚举 14 个 crate 与壳的 `[dependencies]` 段内 workspace 内声明，与模型 `[[relation]]`（`from` 为后端元素者）比对
- **THEN** 两侧逐条相等（本期 43 条）；`App.Backend.Infra.Watch` 与 `App.Backend.Core.Agent` 出边为零；`App.Backend.Shell` 出边 12 条

#### Scenario: dev 边零出现

- **WHEN** 检查模型内是否存在 `App.Backend.Core.Workflow → App.Backend.Core.Orchestration` 与 `App.Backend.Core.Orchestration → App.Backend.Infra.Store` 两条
- **THEN** 两条均不在模型内（它们来自 `[dev-dependencies]`）；模型聚合后无 `cycle` 诊断

#### Scenario: 反向边在案且含父子禁边检查

- **WHEN** 查询 `App.Backend.Tools.ExportBindings` 的出边，并审查全部后端边是否含「子 → 父」
- **THEN** 出边恰为 `App.Backend.Shell` 一条（`desc` 说明为导出 bindings 而依赖壳）；全部后端边中零 `dependsOnAncestor` 形态

### Requirement: 前端下钻到视图与组件层的元素覆盖

前端模型 SHALL 下钻到**视图**与**组件**两层，并按三条机械判据推导元素，MUST NOT 按个人口味挑选：

| 档 | 判据 | 元素 |
|---|---|---|
| 页 / 壳 | `src/app.tsx` + `routes.tsx` + `main.tsx` + `app-sidebar.tsx` 合成 `App.Frontend.Shell`；`views/<view>/` 每目录一元素；`views/welcome-view.tsx` 单文件一元素 | `App.Frontend.Shell`、`App.Frontend.Views.{Agents,AgentDebug,Architecture,Changes,Config,DbInspector,Explores,Info,Welcome}` |
| 视图内子域 | `views/<view>/` 下**除 `components` / `hooks` 外**、非测试源文件 **≥ 3** 的子目录 | `App.Frontend.Views.Changes.{Flow,Renderers,Agent}` |
| 共享组件 | `src/components/agent/*.{ts,tsx}` 与 `src/components/standard/*.tsx` 每文件一元素（排除 `*.test.*` 与 barrel `index.tsx`）；`src/components/ui/` **收敛为 1 元素** | `App.Frontend.Components.Agent.{AgentBlocks,AgentError,AgentInput,AgentMessages,AgentSessionView,ToolCards}`、`App.Frontend.Components.Standard.{Dailog,Drawer,Pagination,Select}`、`App.Frontend.Components.Ui` |

容器 SHALL 为 `App.Frontend`、`App.Frontend.Views`、`App.Frontend.Components`、`App.Frontend.Components.Agent`（持 barrel）、`App.Frontend.Components.Standard`——`kind = "domain"`、`path` 空（`Components.Agent` 例外）；视图与视图内子域 SHALL 为 `module`、共享组件文件 SHALL 为 `component`、`App.Frontend.Components.Ui` SHALL 为 `module`。

- **`components/ui` 收敛的理由**：ui 是 shadcn 原语层，35 个文件恒被依赖、不承担「谁能依赖谁」的判定；逐文件建元素只增噪声（35 元素）不增信息。若将来 ui 内部出现实质分层，再拆元素（须改本 requirement）。
- **封闭排除面**（不建元素、不参与覆盖、不贡献边）：`src/hooks/**`、`src/lib/**`、`src/types/**`（含 `types/generated/bindings.ts`）、`src/components/standard/hooks/**`、`*.test.{ts,tsx}`、`src/test-setup.ts`。
- **粒度 ≠ 解析面**：视图目录内的 `hooks/` / `lib/` 文件不建元素，但其 `import` 仍计入**所属视图元素**的依赖（归属按目录、粒度按是否建元素，两者不是一回事）；`src/components/standard/hooks/` 不入模型是为了守住「容器 `path` 恒空」——把 hooks 归给容器就等于把「子 → 父」写成依赖。
- **覆盖判据（双向）**：上述判据命中的目录 / 文件集合与前端叶元素 `path` 集合 SHALL 相等；判据外的元素 MUST NOT 存在。

#### Scenario: 视图与子域覆盖

- **WHEN** 枚举 `src/views/*/` 与 `src/views/welcome-view.tsx`、并枚举 `views/changes/` 下满足「非 `components`/`hooks` 且非测试源文件 ≥ 3」的子目录
- **THEN** 前者与 9 个视图元素一一对应；后者恰为 `flow`（13 源文件）、`renderers`（7）、`agent`（3）三个子域元素；`views/changes/components/`（1 文件）与各视图的 `components/` 目录不构成子域

#### Scenario: 共享组件逐文件与 ui 收敛

- **WHEN** 检查 `src/components/` 的元素声明
- **THEN** `agent/` 下 6 个源文件各一 `component` 元素（`index.tsx` barrel 排除、由容器 `App.Frontend.Components.Agent` 持有）、`standard/` 下 4 个 `.tsx` 各一元素、`ui/` 恰 1 个 `module` 元素且 `path` 指向该目录

#### Scenario: 排除面零元素

- **WHEN** 检查 `src/hooks`、`src/lib`、`src/types`、`src/components/standard/hooks`、测试文件与 `test-setup.ts`
- **THEN** 以上位置零元素、零 `path` 引用；同时 `views/*/hooks/**` 与 `views/*/lib/**` 文件不构成元素（其依赖计入所属视图）

### Requirement: 前端关系只取 import 证据

前端关系 SHALL 只由**已声明元素覆盖范围内**文件的 `import` 语句证成，MUST NOT 凭语义直觉连边：

- **目标归属**：被 import 的路径 SHALL 按「最具体已声明元素」归属（`@/components/ui/button` → `Components.Ui`；`@/components/standard/drawer` → `Standard.Drawer`；`@/components/agent`（barrel）→ `Components.Agent`）。
- **命令面边**：`types/generated/bindings` 的 `import` SHALL 映射为**命令面依赖**，指向 `App.Backend.Shell`（`desc` 写明「经生成的命令面类型」）——这是前端对后端的唯一合法边来源；`src/lib` / `src/hooks` 未被建模，故壳层对命令面的依赖本期不可见（已知缺口，见裁定记录）。
- **不解析面零边**：指向 `hooks/`、`lib/`、`types/dto` 的 `import` MUST NOT 产生关系（本期不解析）；`App.Frontend.Views.Welcome` 一类「只经 hooks 取数」的元素因而可无后端边。
- **包含与自环**：子元素归属由 FQN 表达，MUST NOT 连边；`from == to` 的自环 MUST NOT 出现（同元素内部互引不建边）。
- **父 → 子的 import 边合法**：`Views.Changes → Views.Changes.Flow`（详情页组装流程画布）是装配依赖，不是包含；禁的只是「子 → 父」。
- **容器作端点仅限 barrel 一例**：容器元素只有 `App.Frontend.Components.Agent` 持 `path`，故只有它可以作为关系端点；其端点边 SHALL 恰为 **6 条**——2 条出边（`index.tsx` 重导出 `AgentInput` / `AgentSessionView`）+ 4 条入边（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）。其余三个容器（`Views` / `Components` / `Standard`）MUST NOT 出现在任何关系的任一端点上。
- **计数恒等式**：前端边 SHALL 为 **64 条 / 22 个源**，且 MUST 满足 `64 = 58（叶 → 叶 / 叶 → 容器）+ 6（barrel 端点边）`；若某次落笔得到 58，说明这 6 条被漏掉（判据自相矛盾），MUST NOT 以「容器不入边」为由删减——要改只能改本要求的字段规则或映射规则（撤销 barrel 例外），不能在实现期折中。

#### Scenario: 每条边可指到 import 语句

- **WHEN** 逐条核对 64 条前端边（22 个源）
- **THEN** 每条均能在其 `from` 元素覆盖范围内的某文件里找到对应 `import`（证据清单在 change 报告留痕）；反向抽查「有 import、无关系」的跨元素引用为零

#### Scenario: 命令面边恰一处来源

- **WHEN** 检查全部指向 `App.Backend.Shell` 的前端边
- **THEN** 其来源文件均 import 了 `types/generated/bindings`（`Views.Agents` / `Views.AgentDebug` / `Views.Architecture` / `Views.Changes` / `Views.Changes.Flow` / `Views.Changes.Agent` / `Views.Config` / `Views.DbInspector` / `Views.Explores` / `Views.Info` / `Components.Agent.AgentSessionView` 一类），MUST NOT 存在以 `hooks` / `lib` 为唯一证据的命令面边

#### Scenario: 容器端点边恰为 barrel 的 6 条

- **WHEN** 统计模型内全部以容器元素为任一端点的前端关系
- **THEN** 恰 6 条且全部挂在 `App.Frontend.Components.Agent`：出边 `AgentInput` / `AgentSessionView`，入边 `Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores`；`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Standard` 零端点，前端边总数 64 = 58 + 6

#### Scenario: 不解析面零边与自环零出现

- **WHEN** 审查模型内前端边集合
- **THEN** 不存在仅由「import `@/hooks/...`」「import `@/lib/...`」「import `@/types/dto`」证成的边；`from == to` 的自环零条；`Components.Ui` 只作为被依赖方出现（出边为零，其内部互引不建边）

### Requirement: 图自洽与停摆禁令

基线模型 SHALL 使四类结构诊断**全部零命中**：`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`；`archi_graph(root)` SHALL 返回正常态（元素与关系完整、`diagnostics` 为空），MUST NOT 停摆。

- **祖先显式声明**：所有容器元素 SHALL 在模型内声明（层级由 FQN 前缀推导，孤儿层级即 `orphanHierarchy`）；MUST NOT 依赖「隐式祖先自动补建」（本能力体系的既有前提）。
- **停摆零容忍**：交付面 MUST NOT 以「结构错 + 停摆面板」形态收尾——停摆是坏模型的处置，不是基线的可接受状态。
- **`docNotFound` 零告警**：`docs` 只指现存根文档、`path` 指向现存目录 / 文件，故 `warnings` SHALL 为空。

#### Scenario: 正常态而非停摆态

- **WHEN** 调用 `archi_graph(root)`
- **THEN** 返回正常态：`elements` 49、`relations` 107（后端 43 + 前端 64）、`diagnostics` 空、`warnings` 空；`/architecture` 页面呈现元素树 + 画布 + 抽屉三段，无结构错面板、零「修好后再看」文案

#### Scenario: 成环与祖先禁边被机械拦住

- **WHEN** 假设模型被写成含 `orchestration ⇄ workflow`（纳入 dev 边）或 `Standard.Dailog → Standard`（把 hooks 归给容器）一类错误
- **THEN** 结构诊断立即返回 `cycle` / `dependsOnAncestor` 且画布停摆——该写法在交付面上不可通过（基线自身 MUST 为这两种写法的反例）

#### Scenario: 孤儿层级零命中

- **WHEN** 检查每个叶元素的 FQN 前缀链
- **THEN** 每一级祖先（`App` → `App.Backend` → `App.Backend.Core` → crate；`App` → `App.Frontend` → `Views.Components` → 叶）均在模型内显式声明，`orphanHierarchy` 零命中

### Requirement: 模型与裁定记录的可检索性

基线落位后，既有两个只读工具面（`archi_search` / `archi_module`）SHALL 在**零代码改动**下直接检索到模型内容与裁定记录，MUST NOT 需要新增命令、工具、索引或字段：

- **元素检索**：关键词 SHALL 匹配元素 FQN / `summary` / `metadata` 值拼合文本（ASCII 大小写不敏感子串 + 驼峰分词），故 `App.Backend.Core.Archi` 一类元素 SHALL 可被 `archi` 关键词命中，返回薄条目（`fqn` / `kind` / `gate` / `summary`）。
- **裁定记录检索**：ADR SHALL 经既有 `decisions/*.md` 文本源（`source = "text"`，正文含元素 FQN 即自动关联）成为元素的文档条目，从而按路径 / 文件名可被检索命中，且条目恒四字段（`path` / `title` / `kind` / `source`）；MUST NOT 把 ADR 路径写进元素 `docs` 强行关联（`docs` 只留给包根 `App`）。
- **一跳定位**：`archi_module(<FQN>)` SHALL 给出该元素的 `dependsOn` / `dependedBy` / `children` / `docs` 与 `modelFile` / `modelLine`（指向声明表头，可直接 `edit`），使 agent 能「查名 → 定位 → 改文件」闭环。
- **契约零变化**：本变更只是给两工具第一份非空语料，其注入矩阵、只读档位与响应形状 MUST NOT 因本变更改变（若形状有缺口按缺陷另议，不在本能力内扩字段）。

#### Scenario: 关键词检索同时命中元素与裁定记录

- **WHEN** 以 `query = "archi"`、`scope = all` 调用 `archi_search`
- **THEN** `elements` 含 `App.Backend.Core.Archi`（`kind = module`、`gate = off`、`summary` 非空）与 `App.Frontend.Views.Architecture`；`docs` 含 `openspec/architecture/decisions/0001-model-granularity.md`（`kind = adr`、`source = text`、恰四字段且不含正文）

#### Scenario: 单元素详情给出一跳邻域与定位字段

- **WHEN** 调用 `archi_module(element = "App.Backend.Core.Archi")`
- **THEN** `dependsOn` 恰一条 `App.Backend.Core.Foundation`，`dependedBy` 含 `App.Backend.Infra.AgentRuntime` 与 `App.Backend.Shell`，`children` 为空，`warnings` 为空；`element.modelFile` = `openspec/architecture/models/02-backend-core.toml` 且 `modelLine` 指向该元素表头行

### Requirement: 覆盖判据的机械核对与漂移纪律

元素的完整性与关系的证据性 SHALL 可**机械核对**（不靠人读图判断），核对口径恰为三条：

| # | 核对 | 判据 |
|---|---|---|
| 1 | 双向覆盖 | 判据命中的 crate 目录 / 视图目录 / 视图内子域 / 共享组件文件 / `components/ui/` 集合 == 模型叶元素 `path` 集合（无漏无多） |
| 2 | 关系证据 | 后端逐边等于 `Cargo.toml` `[dependencies]` 的 workspace 内声明集；前端逐边可指到 `import` 语句 |
| 3 | 图自检 | `archi_graph` 正常态、`diagnostics` 空、`warnings` 空 |

- 本期核对 SHALL 以**命令 + 留痕**执行（change 报告内），MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**` 作语料——crate 语料必须自足可移植，耦合本仓内容会让 crate 单测随仓库演进变红。机制化（核对进流程门禁）归 `archi-check-and-gate`。
- **漂移纪律**：新增 / 删除 crate、视图、视图内子域、共享组件，或新增跨元素 `import` 依赖时，SHALL 在同一 change 内同步模型（元素 / 关系 / `path`），MUST NOT 留待「下次整理」；判据之外的元素与关系 MUST NOT 存在（「觉得该有」不是判据）。

#### Scenario: 双向覆盖零差集

- **WHEN** 以判据枚举两侧集合并求差集
- **THEN** 两个方向差集均为空；任一新增目录 / 文件未声明即出现非空差集，核对失败

#### Scenario: 判据之外零元素零边

- **WHEN** 审查模型内元素与关系的来源
- **THEN** 每个元素可归属到某条覆盖判据（crate / 视图 / 子域 / 共享组件 / `ui` / 壳 / 容器），每条关系可归属到 `Cargo.toml` 声明或 `import` 语句；无「直觉元素」与「直觉边」

#### Scenario: 漂移在同一变更内补齐

- **WHEN** 某 change 新增一个视图并新增一条跨视图 `import`
- **THEN** 该 change 的交付内含新增视图元素与其关系；未补齐时双向覆盖核对与「有 import 无关系」抽查同时失败

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `openspec/architecture/models/01-app.toml` | 包根与域容器 | `App`（`package`，`docs` 指 `./README.md` / `./AGENT.md`）、`App.Backend`、`App.Frontend`；零关系 |
| `openspec/architecture/models/02-backend-core.toml` | core 引擎侧分层 | `App.Backend.Core` 容器 + 7 crate 元素；9 条 core 内边（`foundation` 与 `agent` 无出边，其余 core crate 只向内依赖） |
| `openspec/architecture/models/03-backend-infra.toml` | infra 适配侧分层 | `App.Backend.Infra` 容器 + 6 crate 元素；21 条边（infra → core 为主，含 `store → workflow` 跨界边） |
| `openspec/architecture/models/04-backend-shell.toml` | Tauri 壳 | `App.Backend.Shell`（`path` = `src-tauri/src/`）+ 12 条装配出边（壳是唯一全依赖汇聚点） |
| `openspec/architecture/models/05-backend-tools.toml` | 构建工具 crate | `App.Backend.Tools` 容器 + `ExportBindings`；唯一反向边 `ExportBindings → App.Backend.Shell`（导出 bindings 需依赖壳包） |
| `openspec/architecture/models/06-frontend.toml` | 前端视图与组件层 | 28 元素 = **4 容器**（`Views` / `Components` / `Components.Agent` / `Components.Standard`）+ 24 叶（`Shell` / 9 视图 / 3 子域 / 10 共享组件 / `Ui`；`App.Frontend` 在 `01`）+ **64 条 import 边**（22 个源；`Ui` 恒为被依赖方，`Components.Agent` 的 2 出 / 4 入共 6 条 barrel 端点边） |
| `openspec/architecture/decisions/0001-model-granularity.md` | 粒度与口径裁定 | 六条裁定：三档粒度、容器 `path` 恒空（barrel 例外）、dev 边排除、`components/ui` 收敛、`gate` 全 `off`、封闭排除面；正文引用元素 FQN ⇒ 经 `decisions/*.md` 文本源被自动关联 |
| `openspec/changes/**/reports/**`（本 change 内） | 核对留痕 | 双向覆盖差集、后端边逐条比对、前端边 import 证据清单三份结果 |
| `crates/core/archi/**`（**零改动**） | 模型读面 | 只被使用：`parse_dir` 聚合、`ModelGraph` 校验、`dto` 投影；MUST NOT 因本变更新增字段 / 变体 / 语料 case |
| `src/views/architecture/**`（**零改动**） | 画布 | 只被使用：`/architecture` 从空态转正常态渲染 49 节点 / 107 边 |
| `archi_search` / `archi_module` 工具面（**零改动**） | 只读检索 / 定位 | 只被使用：`archi_search` 返回元素薄条目 + 四字段文档条目（ADR 经 `text` 源命中），`archi_module` 返回 `dependsOn` / `dependedBy` / `children` / `docs` + `modelFile`·`modelLine`；本基线是它们的首份非空语料，契约零变化 |
