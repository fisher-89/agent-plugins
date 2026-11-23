# repo-archi-model Specification（delta: backend-notification-hub）

## Purpose

本 delta 只**换段计数**：新增一个 core crate（`crates/core/notification`）⇒ 元素 +1、关系 +4（三条后端 `Cargo.toml` 边 + 一条前端 `import` 边）。元素与关系的判据（覆盖判据、取数口径、`gate` / `summary` / `path` / `entry` / `api` / `doc` 字段纪律、目录分家、停摆禁令）逐字不动——本 delta 只把读数与随之变化的两个「`src/lib.rs` 命中的 crate」计数写实。

## MODIFIED Requirements

### Requirement: 基线落位与模型文件划分

架构模型基线 SHALL 落位 `<archi_dir>/graph/model/`（架构域根经 `foundation::layout::archi_dir` 派生、模型声明目录经 `archi_graph_model_dir` 派生，本期落点 `.dev-team/architecture/graph/model/`），MUST NOT 落到 `src/`、`docs/`、`openspec/specs/`、`<archi_dir>/models/`（契约资产目录）或任何其它位置。

模型 SHALL 划分为**六个一级文件**，文件名形如 `NN-<域>.toml`，字典序即聚合序，每个元素与每条关系 SHALL 恰在一处声明（跨文件或同文件重复即 `duplicateElement`）：

| 文件 | 内容 | 元素 | 关系 |
|---|---|---|---|
| `01-app.toml` | 包根与两个域容器 | 3 | 0 |
| `02-backend-core.toml` | `Core` 容器 + **8** 个 core crate 元素 | **9** | **10** |
| `03-backend-infra.toml` | `Infra` 容器 + **7** 个 infra crate 元素 | **8** | **25** |
| `04-backend-shell.toml` | Tauri 壳（命令面 / 装配 / bindings 导出） | 1 | **14** |
| `05-backend-tools.toml` | `Tools` 容器 + `export-bindings` crate + `archi-api-gen` crate | **3** | **3** |
| `06-frontend.toml` | 前端 `Shell` / 9 视图 / 3 视图内子域 / **13** 共享组件 / **目录模块 `Components.Agent`** / `Ui` / **生成物中继元素 `GeneratedBindings`** + **三个**容器（`App.Frontend` 容器在 `01`） | **32** | **75** |

合计 SHALL 为 **56 元素 / 127 关系**，逐文件可加：元素侧 `3 + 9 + 8 + 1 + 3 + 32 = 56`（容器 9 + 叶 47），关系侧 `0 + 10 + 25 + 14 + 3 + 75 = 127`。关系侧另按取数口径分：**127 = 后端 52（`Cargo.toml` `[dependencies]` 的 workspace 内声明，拆在 `02`–`05`：10 + 25 + 14 + 3）+ 前端 75（`import` 证据，全在 `06`）**；两种拆分 MUST 得同值。前端 75 的构成 SHALL 为 **67 条叶 → 叶边 + 8 条 `App.Frontend.Components.Agent` 的 barrel 端点边**（3 条出边：入口 `index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`；5 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / **`Views.Config`** / `Views.Explores` import `@/components/agent`）——这 8 条由「`components/agent/` 是 `Components.Agent` 的目录模块入口」与「`@/components/agent` → `Components.Agent` 映射」两条规则必然导出，MUST NOT 从记数中剔除；12 个目录模块入口的**其余 re-export 全在本元素内**（`from == to` 自边被丢弃）⇒ 零新边。其中 `06-frontend.toml` 的 32 元素 = **3 个容器**（`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Standard`）+ **29 个叶**（`Shell` 1 + 9 视图 + 3 子域 + 13 共享组件（7 `agent` + 6 `standard`）+ `Components.Agent` 1 + `Ui` 1 + **`GeneratedBindings` 1**）；`App.Frontend` 容器 SHALL 只在 `01-app.toml` 声明一次，`06` MUST NOT 重复声明（重复即 `duplicateElement`，并会使元素总数变成 57 而非 56）。

- **模型声明面与契约资产面 SHALL 按目录分家**：模型声明面是 `<archi_dir>/graph/model/` 的**一级**——恰六个 `NN-<域>.toml`，MUST NOT 出现其他一级 `*.toml`（尤其 MUST NOT 出现 `INDEX.toml`）；契约资产面是 `<archi_dir>/models/<分层目录…>/`——`<FQN>.md`（手写真相）+ `<FQN>.schema.json`（生成物），布局与命名归 `desktop-archi-api`「契约资产形态与唯一手写真相」。两面的分离判据是「目录」与「`.toml` vs `.md` / `.schema.json`」，MUST NOT 引入目录级豁免清单或改名方案；契约索引落 `<archi_dir>/graph/INDEX.toml`（`archi_graph_dir`），与模型声明目录（`graph/model/`）同树不同层——索引落 `graph/model/` 内会被当模型文件解析并使模型停摆，理由见 `desktop-archi-model`「多文件聚合与声明定位」。
- **契约子树不计入模型计数**：`models/<分层目录…>/` 下的任何文件 MUST NOT 改变元素数 / 关系数 / `modelHash`——收面判据是「`graph/model/` 一级 + `.toml` 后缀」，契约目录整体不在收面内。
- **关系归属**：每条 `[[relation]]` SHALL 声明在**其 `from` 元素所在文件**内（定位一致、diff 不跨文件散落）。
- **`gate` 全 `off`**：本基线内每个元素 SHALL 省略 `gate` 或显式写 `off`，MUST NOT 出现 `warn` / `strict`——冷启动 ratchet（收紧按模块独立变更，一个 change 收紧一块）。
- **`summary`**：每个元素 SHALL 有一行中文职责说明，MUST NOT 用 FQN 字面复述自身名字。
- **`path`**：叶元素必填、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器元素 SHALL 留空（**零例外**）。目录模块入口由 `desktop-frontend-module-entry` 承载：`App.Frontend.Components.Agent` 是 `kind = "module"` 的**叶**元素，`path` 为目录 `./packages/desktop/src/components/agent/`，入口是该目录内既有的 `index.tsx`（不再以文件型 `path` 自任端点）。**生成物中继元素（本变更新增）**：`App.Frontend.GeneratedBindings` 是 `kind = "module"` 的**叶**元素，`path` 为生成目录 `./packages/desktop/src/types/generated/`——它持有三个生成文件（`bindings.ts` / `bindings.zod.ts` / `bindings.zod.check.ts`）的归属，从而让「前端 import 生成物」这条依赖的目标可归属（归属判据零改动，走既有 `OwnerIndex` 最长前缀）。
- **`entry`**：叶元素 SHALL **仅在约定默认指不到时**声明（单值、`./` 前缀、repo 相对）；单文件根元素（`views/welcome-view.tsx`、`components/agent/` 的 7 个组件文件、`standard/*.tsx`）、**13 个目录型根元素**（入口由 ③ 的 `<root>/index.ts` / `<root>/index.tsx` 命中，含 `components/agent/` 与 `components/ui/`）与 15 个 `src/lib.rs` 命中的 crate MUST NOT 声明（根文件自身 / 语言约定默认已给出入口）。**生成物目录 MUST NOT 声明 `entry`**：它是三个叶子文件的集合、没有模块入口（`entry_state = Absent`），造一个 `index.ts` 只为消 `Absent` 属造假入口（`api = none` 已退出入口判据，零 `apiEntryMissing`，见 `desktop-archi-api-check`）。**本基线内显式 `entry` SHALL 恰为 3 处**（下列三值逐字在案；宿主能解析 `entry` 字段是这三行落地的前提，见 `desktop-archi-api`「生成物落点与入口查找」）：
  - `App.Backend.Shell` → `./packages/desktop/src-tauri/src/lib.rs`（目录型壳的根路径 `src-tauri/src/` 拼出的约定默认是 `src-tauri/src/src/lib.rs`，指不到）；
  - `App.Backend.Tools.ExportBindings` → `./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs`；
  - `App.Backend.Tools.ArchiApiGen` → `./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`。

  三处声明的后果 SHALL 被接受且可读：壳由 `apiEntryMissing`（`Absent`）转为 `docMissing`（`declared` 而派生契约 md 不在盘）；两个工具 crate 的入口判据面**整体不适用**（`api = none` 的元素退出入口判据，见 `desktop-archi-api-check`「契约判据词汇封闭十三值与开放分类」）——「有入口」与「无库 API」两个事实同时成立，MUST NOT 被读成「入口补齐 ⇒ 判据面清零」。MUST NOT 出现第四处显式 `entry`：其余叶元素由约定默认覆盖（单文件根 = 根文件自身、目录型根 = `<root>/index.ts(x)`、15 个 crate 的 `src/lib.rs`），多写即违「仅在约定默认指不到时声明」，并使本条的三处计数失守；`App.Frontend.Components.Agent` 的 `path` 由文件改目录后正是 ③ 的场景，**加行即违本条**。

  第四节「四条判据」（基准 / 落位 / 存在 / 跨元素唯一）与三态解析链归 `desktop-archi-api`，本节只钉基线数据；违规走 `apiFindings` 的 `apiEntryMissing`（`warn`，不进 `passed`），MUST NOT 引入结构化诊断。
- **`api = none` 三处显式留白**：两个工具 crate（`App.Backend.Tools.ExportBindings` / `App.Backend.Tools.ArchiApiGen`）与**生成物中继元素**（`App.Frontend.GeneratedBindings`）SHALL 声明 `api = none`——前两者的公共面是 CLI 契约而非库 API、后者是生成物（唯一真相在生成器，手写契约必然漂），三者都是**真话**而非「还没写」；其余元素 SHALL 省略 `api`（取缺省 `declared`）。`api` 状态在本基线的角色是**入口文档载体的开关**：`declared` ⇒ 入口文档 = 派生 `contract_path`（不写 `doc`）；`none` ⇒ 入口文档 = `doc` 声明（可选）。「无入口」与「无 API」仍是两个不同事实，但**两者都不由 `api = none` 的元素承担判据**——本基线内 12 个前端目录根由 `desktop-frontend-module-entry` 的 `index.ts(x)` 经 ③ 命中、15 个 `src/lib.rs` 命中的 crate 由约定默认解析出入口，故 `apiEntryMissing` 面为 0；MUST NOT 由「三处 `api = none` 无判据」反推「它们没有入口」（三处显式 `entry` 在案；生成物目录是「无模块入口」的真形态），亦 MUST NOT 由「12 个前端目录元素仍有 `docMissing`」反推「无 API」。
- **`doc`**：SHALL 为**单值** `Option<String>`（repo 相对 POSIX 路径，`./` 前缀由归一剥除、不参与身份；无绝对前缀 / 无盘符 / 无 `..` / 无空段）。纪律三条：
  1. **只有 `api = none` 元素可写**；`api = declared`（含缺省）元素 MUST NOT 写 `doc`——它的入口文档就是派生落点 `contract_path`，写了即 `apiUnexpected`（两事实矛盾）；
  2. 值 MUST NOT 指向 `<archi_dir>/models/<分层目录…>/**`（契约资产的消费者是生成器与门禁，不是文档字段）与 `openspec/specs/**`（spec 语料非模型面）；
  3. 不写 = 无入口文档（合法状态，但会产出 `docMissing` `warn` 读数）——两份工具 crate README 属数据面变更，MUST NOT 在本能力内新建。

  `App`（`package`，无 `path`）SHALL 零 `doc`：包根的 `README.md` / `AGENT.md` 由仓库结构本身表达，**不进模型面**（原 `docs = ["./README.md", "./AGENT.md"]` 一行随 `docs` 字段退役删除）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。**命令面边的 `desc` SHALL 写明「经生成的 IPC 绑定」**（本变更起改指生成物中继元素，语义仍是「前端经生成物调后端命令面」）。
- **计数复述单源**：模型文件的表头注释（如 `# 02 · core 分层（9 元素 / 10 关系）`）、本要求的逐文件计数表、与裁定记录 `§1` / `§9` 的复算表 SHALL 在同一次模型改动内同步为同值；三处任一处陈旧即漂移，按「同一 change 内补齐」处理，MUST NOT 让计数表与模型本体各说各话。**本变更的增量与历史留痕（同批写实，逐项留痕）**：① **本变更净增量** = 元素 **+1**（`App.Backend.Core.Notification`：新 crate `crates/core/notification`，`src/lib.rs` 经约定默认命中入口）、关系 **+4**（后端三条 `[dependencies]` 边 `Orchestration → Core.Notification` / `Store → Core.Notification` / `Shell → Core.Notification`，前端一条 `import` 边 `Shell → Components.Standard.Drawer`）⇒ `02` 段 8 / 9 → **9 / 10**、`03` 段 8 / 24 → **8 / 25**、`04` 段 1 / 13 → **1 / 14**、`06` 段 32 / 74 → **32 / 75**、全基线由 55 / 123 → **56 / 127**；② **历史留痕** = 上一变更（生成物中继元素 `App.Frontend.GeneratedBindings`）的净增量是元素 +1 / 关系 ±0（54 / 123 → 55 / 123），其同批校正的旧读数 `53 / 121` 与 `06` 段 30 / 72 已在案，MUST NOT 被重算进本次增量。**字段面扩容（`entry` / `api` / `doc` / `apis`）与目录模块入口（barrel）不改变元素与关系计数**：12 个 barrel 是元素 `path` 内的新增文件，既不新增元素也不新增边（入口只 re-export 本元素直属文件 ⇒ `from == to` 自边被丢弃）；**元素粒度补漏改变计数**（历史：`components/agent/session-panel.tsx` 与 `components/standard/tooltip.tsx`、`standard/combobox.tsx` 补建元素即各 +1）；**元素形态翻转改变容器 / 叶切分而不改总数**（历史：`Components.Agent` 容器 → 目录模块：容器 10 → 9、叶 43 → 44）；**契约资产迁入 `models/<分层目录…>/`、索引迁入 `graph/`、`docs` → `doc` 换面均不改变任何计数**。`modelHash` 则随模型字节变化——本变更新增一行元素、四条关系与四段表头注释都改字节，故 `modelHash` 必然变，该事实 SHALL 被如实记录，MUST NOT 写成「`modelHash` 逐字不变」。**读数写实边界**：本条只要求写实**模型表头 / 本表 / ADR 三处**与直接判据面（`desktop-archi-api-check` 的基线表、`desktop-archi-api` 的入口面实测）；其余 capability spec 内的历史读数（如 `desktop-frontend-module-entry` 的「71 条 / 44 个 `path` 非空」、`desktop-archi-contract-panel` 的「43 条存量」）不在本变更的追改面内——它们是各自变更交付时的读数（含本 delta 未追改的「55 元素 / 123 关系」一类历史值），与「53 / 120 vs 54 / 123」的陈旧同一处置。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `<archi_dir>/graph/model/` 一级的六个文件（契约子树在另一目录 `<archi_dir>/models/<分层目录…>/`）
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 **56** 条、`relations` 共 **127** 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行；契约子树内的任何文件 MUST NOT 出现在 `modelFile` 里

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` / `entry` / `api` / `doc` / `apis` 出现行
- **THEN** `gate` 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；**`docs` 零命中**（字段已退役，命中即 `unknownField` 停摆）；`doc` 行数 = 「`api = none` 且已声明入口文档的元素数」（本变更交付时 **0 行**——两份工具 crate README 与生成物面都不写 `doc`；反向若出现 `api = declared` 元素的 `doc` 行即违纪律）；全部叶元素 `path` 以 `"./` 开头，全部容器 `path` 为空（**零例外**——`App.Frontend.Components.Agent` 与 `App.Frontend.GeneratedBindings` 都是目录型 `module` 叶）；`entry` **恰三行**（`04` 一行、`05` 两行）；`api` **恰三行**且均为 `"none"`（两个工具 crate + 生成物中继元素）；**`apis` 零命中**（本基线 `apis` 全留空——生成物面是唯一真相，其余元素待有真实依赖清单时再填）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `<archi_dir>/graph/model/` 的**一级**恰有六个 `NN-<域>.toml`（`find -maxdepth 1 -name '*.toml'` 读数 = 6，无 `INDEX.toml`）；`decisions/` 有 `0001-model-granularity.md`；`graph/` 恰有 `INDEX.toml` 一份与 `model/` 子目录；`models/<分层目录…>/` 下为契约资产（`<FQN>.md` + `<FQN>.schema.json`）；`api/` 目录不存在；`src/`、`docs/`、`openspec/specs/` 下无模型文件

#### Scenario: 计数复述三处同值

- **WHEN** 逐处读取 `06` 文件表头注释、本要求的计数表与 ADR `§1` / `§9` 的复算表
- **THEN** 三处同值：全基线 **56 元素 / 127 关系**（容器 9 + 叶 47；后端 52 + 前端 75）、`06` 段 **32 元素 / 75 关系**（3 容器 + 29 叶）、`05` 段 3 元素 / 3 关系、`02` 段 9 元素 / 10 关系、`03` 段 8 元素 / 25 关系、`04` 段 1 元素 / 14 关系；任一处仍写 55 / 123（含 `06` 段 32 / 74、`02` 段 8 / 9、`03` 段 8 / 24、`04` 段 1 / 13）、54 / 123、53 / 120、51 / 113、50 / 111、47 关系、「4 个容器」，或前端恒等式仍写 **66 + 8**（实为 **67 + 8**）即不合格

#### Scenario: 三处入口与 12 个目录入口在案后的判据面归位

- **WHEN** 分别对壳、两个工具 crate、生成物中继元素、12 个前端目录根元素跑一次全量检查并读 `apiFindings`
- **THEN** 壳**零** `apiEntryMissing`，同趟恰一条 `docMissing`（显式 `entry` 命中 ⇒ 入口判据不成立；`api = declared` 且派生契约 md 缺失 ⇒ `docMissing` 成立）——两条 MUST NOT 同时出现（入口债与入口文档债是两回事）；两个工具 crate 与生成物中继元素因 `api = none` **退出入口判据**，各恰一条 `docMissing`（`doc` 未声明），该结论 MUST NOT 被重算成「无入口」或「无 API」（生成物目录的 `Absent` 入口事实另见 `desktop-archi-api`「本仓入口面实测」）；12 个前端目录根**零** `apiEntryMissing`（各自的 `index.ts` / `index.tsx` 由 ③ 命中），同趟**各一条** `docMissing`；新增的 `App.Backend.Core.Notification` 同为 `api = declared` 且派生契约 md 不在盘 ⇒ 同趟恰一条 `docMissing`——12 条 + 该条由此居当前段 37 条之内，MUST NOT 被读成「入口债已清 ⇒ 判据面干净」

#### Scenario: 契约子树与索引不改变模型读数

- **WHEN** 在 `models/<分层目录…>/` 放入契约 md 与 `.schema.json`、在 `graph/` 放入 `INDEX.toml` 之后，分别取一次 `archi_graph`（元素 / 关系 / 诊断）与一次 `archi_drift` 的 `modelHash`
- **THEN** 元素 56 / 关系 127 / `diagnostics` 空与放入前逐字相同；`modelHash` 逐字相同（取面为 `graph/model/*.toml` 一级、按扩展名）；反向判据：把 `INDEX.toml` 复制到 `graph/model/` 根后 `modelHash` 立刻变化且模型转停摆态——两侧读数即本条的存在理由

#### Scenario: docs 写回即停摆

- **WHEN** 在任一模型文件的元素表内写回 `docs = ["./README.md"]`
- **THEN** 解析返回 `unknownField` 诊断（字段面白名单已删 `docs`）、模型判定为停摆态；`archi_graph` 不再给图数据——`docs` → `doc` 的迁移 MUST 与字段白名单同批落地（本仓 `01-app.toml` 的一行已同批删除）

#### Scenario: 包根两份根文档不进模型

- **WHEN** 检查 `App` 元素的字段面与 `README.md` / `AGENT.md` 的模型关联
- **THEN** `App` 零 `doc`（它是 `package`、无 `path`，纪律不要求也不允许它写）；`archi_module("App")` 的响应内不含任何指向两份根文档的字段；两份文件仍在仓根、可被文件树与通用文件搜索抵达

## Module Contract

### 模型增量：core 通知 crate 与一条前端 import 边（backend-notification-hub）

| 文件 | 增量 | 读数 |
|---|---|---|
| `02-backend-core.toml` | 新元素 `App.Backend.Core.Notification`（`kind = "module"`、`gate = "off"`、`path = ["./packages/desktop/src-tauri/crates/core/notification/"]`、一行中文 `summary`，零 `entry` / 零 `api` / 零 `doc`）+ 关系 `Orchestration → Core.Notification`（「终态与停等更新投影为通知词表」） | 8 / 9 → **9 / 10** |
| `03-backend-infra.toml` | 关系 `Store → Core.Notification`（「收件箱记录字段复用通知词表并实现落库 sink」） | 8 / 24 → **8 / 25** |
| `04-backend-shell.toml` | 关系 `Shell → Core.Notification`（「通知命令面装配扇出点与事件投影」） | 1 / 13 → **1 / 14** |
| `06-frontend.toml` | `App.Frontend.Shell` 的 `path` 追加 `./packages/desktop/src/components/notification-bell.tsx`（文件随元素，非新元素）+ 关系 `Shell → Components.Standard.Drawer`（「收件箱抽屉复用标准抽屉」） | 32 / 74 → **32 / 75** |
| `.dev-team/architecture/graph/INDEX.toml` | `api:export` 重生成（勿手改） | 机械面 |
| `decisions/0001-model-granularity.md` | `§1` / `§9` 复算表与 `§3` 已知缺口旁证随计数换段 | 同上 |

### 判据面（零变化）

| 面 | 理由 |
|---|---|
| 覆盖判据四档 / 关系取数口径（`[dependencies]`、`import`） | 新元素与新边逐条命中既有判据（crate 粒度 / `[dependencies]` 段内 `{ workspace = true }` / `import` 证据），未新增任何判据档 |
| 字段纪律（`entry` 恰三处、`api = none` 恰三处、`doc` 仅 `api = none` 可写、`metadata` 全空） | 新元素零 `entry` / 零 `api` / 零 `doc` / 零 `metadata`，三处计数不变 |
| 前 11 条「已知缺口」登记 | 本 delta 只新增 `Shell → Components.Standard.Drawer` 一条**已声明**边，缺口清单其余条目（含 `Views.Changes → Components.Standard.Drawer`）逐字不动 |
