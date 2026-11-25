# repo-archi-model Specification（delta: archi-agent-context-review）

## Purpose

本 delta 只同步基线读数与 `api` 语义（模型数据零改动：55 元素 / 129 关系 / `gate` 全 `off` / 三处 `api = none`）。

- **`api` 的角色**：由「入口文档载体开关」改为「接口承诺开关」——`declared` ⇒ 接口真相 = 同目录 `<FQN>.schema.json`（文档 = 派生 `<FQN>.md`）；`none` ⇒ 无接口面（`doc` 可声明但零判据）。
- **读数**：`docMissing` **38**（35 条 `declared` 缺派生 md + 3 条 `api = none` 缺派生 md）、其余九值 0、`status = clean`、`passed = true`；三处 `api = none` 的成因句换面。
- **断言**：删 `docNotFound` 零命中句，改为 `apiUnexpected` / `apiMissing` / `apiSignatureMismatch` 各零命中。
- **历史链**：末段由「0 + 36 = 36（分母 46）」推进为「0 + 38 = 38（分母 49、资产齐全 11）」。

## MODIFIED Requirements

### Requirement: 基线落位与模型文件划分

架构模型基线 SHALL 落位 `<archi_dir>/graph/model/`（架构域根经 `foundation::layout::archi_dir` 派生、模型声明目录经 `archi_graph_model_dir` 派生，本期落点 `.dev-team/architecture/graph/model/`），MUST NOT 落到 `src/`、`docs/`、`openspec/specs/`、`<archi_dir>/models/`（契约资产目录）或任何其它位置。

模型 SHALL 划分为**六个一级文件**，文件名形如 `NN-<域>.toml`，字典序即聚合序，每个元素与每条关系 SHALL 恰在一处声明（跨文件或同文件重复即 `duplicateElement`）：

| 文件 | 内容 | 元素 | 关系 |
|---|---|---|---|
| `01-app.toml` | 两个包根 `Backend` / `Desktop`（`kind = "package"`，各持根路径） | 2 | 0 |
| `02-backend-core.toml` | `Core` 容器（`kind = "composite"`）+ **8** 个 core crate 元素（`package`） | **9** | **10** |
| `03-backend-infra.toml` | `Infra` 容器（`kind = "composite"`）+ **7** 个 infra crate 元素（`package`） | **8** | **25** |
| `04-backend-shell.toml` | Tauri 壳（命令面 / 装配 / bindings 导出） | 1 | **14** |
| `05-backend-tools.toml` | `Tools` 容器（`kind = "composite"`）+ `export-bindings` crate + `archi-api-gen` crate（`package`） | **3** | **3** |
| `06-frontend.toml` | 前端 `Shell` / 9 视图 / 3 视图内子域 / **13** 共享组件 / **目录模块 `Components.Agent`** / `Ui` / **生成物中继元素 `GeneratedBindings`** + **三个**容器（`Desktop` 包根在 `01`） | **32** | **77** |

合计 SHALL 为 **55 元素 / 129 关系**，逐文件可加：元素侧 `2 + 9 + 8 + 1 + 3 + 32 = 55`（纯容器 6 + 其余 49），关系侧 `0 + 10 + 25 + 14 + 3 + 77 = 129`。关系侧另按取数口径分：**129 = 后端 52（`Cargo.toml` `[dependencies]` 的 workspace 内声明，拆在 `02`–`05`：10 + 25 + 14 + 3）+ 前端 77（`import` 证据，全在 `06`）**；两种拆分 MUST 得同值。前端 77 的构成 SHALL 为 **69 条元素 → 元素边 + 8 条 `Desktop.Components.Agent` 的 barrel 端点边**（含 `Desktop` 包根兜底共享文件产生的 `Desktop → Desktop.Components.Ui` / `Desktop → Desktop.GeneratedBindings` 两条）（3 条出边：入口 `index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`；5 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / **`Views.Config`** / `Views.Explores` import `@/components/agent`）——这 8 条由「`components/agent/` 是 `Components.Agent` 的目录模块入口」与「`@/components/agent` → `Components.Agent` 映射」两条规则必然导出，MUST NOT 从记数中剔除；12 个目录模块入口的**其余 re-export 全在本元素内**（`from == to` 自边被丢弃）⇒ 零新边。其中 `06-frontend.toml` 的 32 元素 = **3 个容器**（`Desktop.Views` / `Desktop.Components` / `Desktop.Components.Standard`）+ **29 个其余元素**（`Shell` 1 + 9 视图 + 3 子域 + 13 共享组件（7 `agent` + 6 `standard`）+ `Components.Agent` 1 + `Ui` 1 + **`GeneratedBindings` 1**）；`Desktop` 包根 SHALL 只在 `01-app.toml` 声明一次，`06` MUST NOT 重复声明（重复即 `duplicateElement`）。

- **模型声明面与契约资产面 SHALL 按目录分家**：模型声明面是 `<archi_dir>/graph/model/` 的**一级**——恰六个 `NN-<域>.toml`，MUST NOT 出现其他一级 `*.toml`（尤其 MUST NOT 出现 `INDEX.toml`）；契约资产面是 `<archi_dir>/models/<分层目录…>/`——`<FQN>.md`（理由，无结构）+ `<FQN>.schema.json`（接口，**手写真相**），布局与命名归 `desktop-archi-api`「契约资产形态与唯一手写真相」。两面的分离判据是「目录」与「`.toml` vs `.md` / `.schema.json`」，MUST NOT 引入目录级豁免清单或改名方案；契约索引落 `<archi_dir>/graph/INDEX.toml`（`archi_graph_dir`），与模型声明目录（`graph/model/`）同树不同层——索引落 `graph/model/` 内会被当模型文件解析并使模型停摆，理由见 `desktop-archi-model`「多文件聚合与声明定位」。
- **契约子树不计入模型计数**：`models/<分层目录…>/` 下的任何文件 MUST NOT 改变元素数 / 关系数 / `modelHash`——收面判据是「`graph/model/` 一级 + `.toml` 后缀」，契约目录整体不在收面内。
- **关系归属**：每条 `[[relation]]` SHALL 声明在**其 `from` 元素所在文件**内（定位一致、diff 不跨文件散落）。
- **`gate` 全 `off`**：本基线内每个元素 SHALL 省略 `gate` 或显式写 `off`，MUST NOT 出现 `warn` / `strict`——冷启动 ratchet（收紧按模块独立变更，一个 change 收紧一块）。
- **`summary`**：每个元素 SHALL 有一行中文职责说明，MUST NOT 用 FQN 字面复述自身名字。
- **`path`**：**模块必填（可多条 = 若干根的清单）；包至多一条包根（0 条 = 纯容器，缺省空，本基线六个容器全部留空）**；`domain` / `composite` 选填。**本基线六个容器按语义分两组**：三个**纯分组**容器 `Backend.Core` / `Backend.Infra` / `Backend.Tools` SHALL 为 `kind = "composite"`（无层级、不持文件，典型形态就是空 `path`）；三个**业务聚合**容器 `Desktop.Views` / `Desktop.Components` / `Desktop.Components.Standard` SHALL 为 `kind = "domain"`（可持文件，本基线同样留空）。「仅作分组的包」这个复合概念由 `composite` 承接，MUST NOT 再用 `kind = "package"` + 空 `path` 表达纯分组；`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名。容器填了即持文件、可作关系端点，但子元素依赖它会写出「子 → 父」并触发 `dependsOnAncestor`。目录模块入口由 `desktop-frontend-module-entry` 承载：`Desktop.Components.Agent` 是 `kind = "module"` 的**叶**元素，`path` 为目录 `./packages/desktop/src/components/agent/`，入口是该目录内既有的 `index.tsx`（不再以文件型 `path` 自任端点）。**生成物中继元素（更早变更已落地）**：`Desktop.GeneratedBindings` 是 `kind = "module"` 的**叶**元素，`path` 为生成目录 `./packages/desktop/src/types/generated/`——它持有三个生成文件（`bindings.ts` / `bindings.zod.ts` / `bindings.zod.check.ts`）的归属，从而让「前端 import 生成物」这条依赖的目标可归属（归属判据零改动，走既有 `OwnerIndex` 最长前缀）。
- **`entry`**：叶元素 SHALL **仅在约定默认指不到时**声明（单值、`./` 前缀、repo 相对）；单文件根元素（`views/welcome-view.tsx`、`components/agent/` 的 7 个组件文件、`standard/*.tsx`）、**13 个目录型根元素**（入口由 ③ 的 `<root>/index.ts` / `<root>/index.tsx` 命中，含 `components/agent/` 与 `components/ui/`）与 15 个 `src/lib.rs` 命中的 crate MUST NOT 声明（根文件自身 / 语言约定默认已给出入口）。**生成物目录 MUST NOT 声明 `entry`**：它是三个叶子文件的集合、没有模块入口（`entry_state = Absent`），造一个 `index.ts` 只为消 `Absent` 属造假入口（`api = none` 已退出入口判据，零 `apiEntryMissing`，见 `desktop-archi-api-check`）。**本基线内显式 `entry` SHALL 恰为 3 处**（下列三值逐字在案；宿主能解析 `entry` 字段是这三行落地的前提，见 `desktop-archi-api`「生成物落点与入口查找」）：
  - `Backend.Shell` → `./packages/desktop/src-tauri/src/lib.rs`（目录型壳的根路径 `src-tauri/src/` 拼出的约定默认是 `src-tauri/src/src/lib.rs`，指不到）；
  - `Backend.Tools.ExportBindings` → `./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs`；
  - `Backend.Tools.ArchiApiGen` → `./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`。

  三处声明的后果 SHALL 被接受且可读：壳由 `apiEntryMissing`（`Absent`）转为 `docMissing`（派生 `<FQN>.md` 不在盘——单成因、与 `api` 状态无关，见改后的判据口径）；两个工具 crate 的入口判据面**整体不适用**（`api = none` 的元素退出入口判据，见 `desktop-archi-api-check`「契约判据词汇封闭十值与开放分类」）——「有入口」与「无库 API」两个事实同时成立，MUST NOT 被读成「入口补齐 ⇒ 判据面清零」。MUST NOT 出现第四处显式 `entry`：其余叶元素由约定默认覆盖（单文件根 = 根文件自身、目录型根 = `<root>/index.ts(x)`、15 个 crate 的 `src/lib.rs`），多写即违「仅在约定默认指不到时声明」，并使本条的三处计数失守；`Desktop.Components.Agent` 的 `path` 由文件改目录后正是 ③ 的场景，**加行即违本条**。

  第四节「四条判据」（基准 / 落位 / 存在 / 跨元素唯一）与三态解析链归 `desktop-archi-api`，本节只钉基线数据；违规走 `apiFindings` 的 `apiEntryMissing`（`warn`，不进 `passed`），MUST NOT 引入结构化诊断。
- **`api = none` 三处显式留白**：两个工具 crate（`Backend.Tools.ExportBindings` / `Backend.Tools.ArchiApiGen`）与**生成物中继元素**（`Desktop.GeneratedBindings`）SHALL 声明 `api = none`——前两者的公共面是 CLI 契约而非库 API、后者是生成物（唯一真相在生成器，手写契约必然漂），三者都是**真话**而非「还没写」；其余元素 SHALL 省略 `api`（取缺省 `declared`）。`api` 状态在本基线的角色是**接口承诺的开关**：`declared` ⇒ 接口真相 = 同目录 `<FQN>.schema.json`（文档 = 派生 `<FQN>.md`）；`none` ⇒ 无接口面（`doc` 可声明，但该字段零判据）。「无入口」与「无 API」仍是两个不同事实，但**两者都不由 `api = none` 的元素承担判据**——本基线内 12 个前端目录根由 `desktop-frontend-module-entry` 的 `index.ts(x)` 经 ③ 命中、15 个 `src/lib.rs` 命中的 crate 由约定默认解析出入口，故 `apiEntryMissing` 面为 0；MUST NOT 由「三处 `api = none` 无判据」反推「它们没有入口」（三处显式 `entry` 在案；生成物目录是「无模块入口」的真形态），亦 MUST NOT 由「12 个前端目录元素仍有 `docMissing`」反推「无 API」。
- **`doc`**：SHALL 为**单值** `Option<String>`（repo 相对 POSIX 路径，`./` 前缀由归一剥除、不参与身份；无绝对前缀 / 无盘符 / 无 `..` / 无空段）。纪律三条：
  1. **约定只有 `api = none` 元素会写**（`api = declared` 元素的文档在派生落点 `contract_path`）——本变更起这条只是**约定而非机械判据**：`declared` 元素写了 `doc` **不报任何判据或写面 warning**（`doc` 零判据读者，`apiUnexpected` 的形态②已删除）；
  2. 值 MUST NOT 指向 `<archi_dir>/models/<分层目录…>/**`（契约资产的消费者是判据面与生成器，不是文档字段）与 `openspec/specs/**`（spec 语料非模型面）；
  3. 不写 = 无入口文档声明（合法状态）；`docMissing` 只判**派生 `<FQN>.md` 不在盘**，与 `doc` 字段写不写无关——两份工具 crate README 属数据面变更，MUST NOT 在本能力内新建。

  两个包根 `Backend` / `Desktop` SHALL 零 `doc`：包根的 `README.md` / `AGENT.md` 由仓库结构本身表达，**不进模型面**（原 `docs = ["./README.md", "./AGENT.md"]` 一行随 `docs` 字段退役删除）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。**命令面边的 `desc` SHALL 写明「经生成的 IPC 绑定」**（更早变更已改指生成物中继元素，语义仍是「前端经生成物调后端命令面」；本变更零 `desc` 改动）。
- **计数复述单源**：模型文件的表头注释（如 `# 02 · core 分层（9 元素 / 10 关系）`）、本要求的逐文件计数表、与裁定记录 `§1` / `§9` 的复算表 SHALL 在同一次模型改动内同步为同值；三处任一处陈旧即漂移，按「同一 change 内补齐」处理，MUST NOT 让计数表与模型本体各说各话。**本变更零计数变化（同批写实）**：元素 **55** / 关系 **129** 与六个文件段的计数（`01` 2 / 0、`02` 9 / 10、`03` 8 / 25、`04` 1 / 14、`05` 3 / 3、`06` 32 / 77）逐字不变——本变更**零模型字节改动**（一个元素表头、一条关系、一个字段值都不动），表头注释与计数表不因本变更改动。**两项历史增量已在基线账内、MUST NOT 被重算成本次增量**：① 更早变更把 `02` / `03` / `05` 三个容器的 `kind` 由 `package` 改 `composite`（`path` 仍空）；② 更早变更的 `Backend.Core.Notification`（元素 +1 / 关系 +4）。**字段面扩容（`entry` / `api` / `doc` / `apis`）与目录模块入口（barrel）不改变元素与关系计数**：12 个 barrel 是元素 `path` 内的新增文件，既不新增元素也不新增边（入口只 re-export 本元素直属文件 ⇒ `from == to` 自边被丢弃）；**元素粒度补漏改变计数**（历史：`components/agent/session-panel.tsx` 与 `components/standard/tooltip.tsx`、`standard/combobox.tsx` 补建元素即各 +1）；**元素形态翻转改变容器 / 叶切分而不改总数**（历史：`Components.Agent` 容器 → 目录模块：容器 10 → 9、叶 43 → 44）；**契约资产迁入 `models/<分层目录…>/`、索引迁入 `graph/`、`docs` → `doc` 换面均不改变任何计数**。`modelHash` 则随模型字节变化——更早变更的三个容器各改一行 `kind` 即改字节（元素与关系条目数、表头注释不变），故**那次变更**的 `modelHash` 必然变，该事实 SHALL 被如实记录；**本变更零字节改动 ⇒ 本变更的 `modelHash` 逐字不变**，两件事 MUST NOT 混说（既 MUST NOT 把历史那次变化写成「`modelHash` 从未变」，也 MUST NOT 把本变更写成会改 `modelHash`）。**读数写实边界**：本条只要求写实**模型表头 / 本表 / ADR 三处**与直接判据面（`desktop-archi-api-check` 的基线表、`desktop-archi-api` 的入口面实测）；其余 capability spec 内的历史读数（如 `desktop-frontend-module-entry` 的「71 条 / 44 个 `path` 非空」、`desktop-archi-contract-panel` 的「43 条存量」）不在本变更的追改面内——它们是各自变更交付时的读数，与陈旧计数同一处置，MUST NOT 与本节的 55 / 129 混为一谈。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `<archi_dir>/graph/model/` 一级的六个文件（契约子树在另一目录 `<archi_dir>/models/<分层目录…>/`）
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 **55** 条、`relations` 共 **129** 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行；契约子树内的任何文件 MUST NOT 出现在 `modelFile` 里

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` / `entry` / `api` / `doc` / `apis` 出现行
- **THEN** `gate` 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；**`docs` 零命中**（字段已退役，命中即 `unknownField` 停摆）；`doc` 行数 = 「`api = none` 且已声明入口文档的元素数」（本变更交付时 **0 行**——两份工具 crate README 与生成物面都不写 `doc`；反向若出现 `api = declared` 元素的 `doc` 行即违**约定**——`doc` 零判据读者，写面与判据面都不报，只在 agent 纪律面禁止）；全部持有文件的元素 `path` 以 `"./` 开头，本基线六个容器 `path` 为空（选填字段，当前留空；`Desktop.Components.Agent` 与 `Desktop.GeneratedBindings` 都是目录型 `module` 叶，`Backend` / `Desktop` 是持根路径的包根）；`entry` **恰三行**（`04` 一行、`05` 两行）；`api` **恰三行**且均为 `"none"`（两个工具 crate + 生成物中继元素）；**`apis` 零命中**（本基线 `apis` 全留空——生成物面是唯一真相，其余元素待有真实依赖清单时再填）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `<archi_dir>/graph/model/` 的**一级**恰有六个 `NN-<域>.toml`（`find -maxdepth 1 -name '*.toml'` 读数 = 6，无 `INDEX.toml`）；`decisions/` 有 `0001-model-granularity.md`；`graph/` 恰有 `INDEX.toml` 一份与 `model/` 子目录；`models/<分层目录…>/` 下为契约资产（`<FQN>.md` + `<FQN>.schema.json`）；`api/` 目录不存在；`src/`、`docs/`、`openspec/specs/` 下无模型文件

#### Scenario: 计数复述三处同值

- **WHEN** 逐处读取 `06` 文件表头注释、本要求的计数表与 ADR `§1` / `§9` 的复算表
- **THEN** 三处同值：全基线 **55 元素 / 129 关系**（纯容器 6 + 其余 49；后端 52 + 前端 77）、`06` 段 **32 元素 / 77 关系**（3 容器 + 29 其余元素）、`05` 段 3 元素 / 3 关系、`02` 段 9 元素 / 10 关系、`03` 段 8 元素 / 25 关系、`04` 段 1 元素 / 14 关系；任一处仍写 56 / 127、55 / 123（含 `06` 段 32 / 75 或 32 / 74、`02` 段 9 / 9、`03` 段 8 / 24、`04` 段 1 / 13）、54 / 123、53 / 120、51 / 113、50 / 111、47 关系、「9 个容器」，或前端恒等式仍写 **67 + 8**（实为 **69 + 8**）即不合格

#### Scenario: 三处入口与 12 个目录入口在案后的判据面归位

- **WHEN** 分别对壳、两个工具 crate、生成物中继元素、12 个前端目录根元素跑一次全量检查并读 `apiFindings`
- **THEN** 壳**零** `apiEntryMissing`，同趟恰一条 `docMissing`（显式 `entry` 命中 ⇒ 入口判据不成立；`api = declared` 且派生 `<FQN>.md` 缺失 ⇒ `docMissing` 成立）——两条 MUST NOT 同时出现（入口债与入口文档债是两回事）；两个工具 crate 与生成物中继元素因 `api = none` **退出入口判据**，各恰一条 `docMissing`（派生 md 不在盘；`doc` 未声明不再单独产判据），该结论 MUST NOT 被重算成「无入口」或「无 API」（生成物目录的 `Absent` 入口事实另见 `desktop-archi-api`「本仓入口面实测」）；12 个前端目录根**零** `apiEntryMissing`（各自的 `index.ts` / `index.tsx` 由 ③ 命中），同趟**各一条** `docMissing`；`Backend.Core.Notification` 同为 `api = declared` 且派生文档不在盘 ⇒ 同趟恰一条 `docMissing`——12 条 + 该条由此居当前 38 条之内，MUST NOT 被读成「入口债已清 ⇒ 判据面干净」

#### Scenario: 契约子树与索引不改变模型读数

- **WHEN** 在 `models/<分层目录…>/` 放入 `<FQN>.md`（文档）与 `<FQN>.schema.json`（接口）、在 `graph/` 放入 `INDEX.toml` 之后，分别取一次 `archi_graph`（元素 / 关系 / 诊断）与一次 `archi_drift` 的 `modelHash`
- **THEN** 元素 55 / 关系 129 / `diagnostics` 空与放入前逐字相同；`modelHash` 逐字相同（取面为 `graph/model/*.toml` 一级、按扩展名）；反向判据：把 `INDEX.toml` 复制到 `graph/model/` 根后 `modelHash` 立刻变化且模型转停摆态——两侧读数即本条的存在理由

#### Scenario: docs 写回即停摆

- **WHEN** 在任一模型文件的元素表内写回 `docs = ["./README.md"]`
- **THEN** 解析返回 `unknownField` 诊断（字段面白名单已删 `docs`）、模型判定为停摆态；`archi_graph` 不再给图数据——`docs` → `doc` 的迁移 MUST 与字段白名单同批落地（本仓 `01-app.toml` 的一行已同批删除）

#### Scenario: 包根两份根文档不进模型

- **WHEN** 检查 `Backend` / `Desktop` 元素的字段面与 `README.md` / `AGENT.md` 的模型关联
- **THEN** 两者零 `doc`（包的根文档由仓库结构表达，纪律不要求也不允许写）；`archi_module("Backend")` 的响应内不含任何指向两份根文档的字段；两份文件仍在仓根、可被文件树与通用文件搜索抵达


### Requirement: 图自洽与停摆禁令

基线模型 SHALL 使四类结构诊断**全部零命中**：`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`；`archi_graph(root)` SHALL 返回正常态（元素与关系完整、`diagnostics` 为空），MUST NOT 停摆。

- **祖先显式声明**：所有容器元素 SHALL 在模型内声明（层级由 FQN 前缀推导，孤儿层级即 `orphanHierarchy`）；MUST NOT 依赖「隐式祖先自动补建」（本能力体系的既有前提）。
- **停摆零容忍**：交付面 MUST NOT 以「结构错 + 停摆面板」形态收尾——停摆是坏模型的处置，不是基线的可接受状态。
- **入口文档面零告警（本变更改写）**：`ArchiModuleDetail` 不再有 `warnings` 字段（`WarningKind` 随 `docs` 三源退役），入口文档的读数全部经漂移报告的 `apiFindings`：本基线 SHALL 恰为 **38 条 `docMissing`（`warn`、不纳管）**、其余九值各 0 ⇒ `status` 仍 `clean`、`passed` 仍 `true`（38 = 49 个 `path` 非空元素 − 11 个资产齐全的元素：35 条 `declared` 侧 + 3 条 `api = none` 侧）；`apiUnexpected` / `apiMissing` / `apiSignatureMismatch` SHALL 各零命中（没有元素把 schema 放在 `api = none` 下，11 个资产齐全的元素接口与代码同步）。`path` 指向现存目录 / 文件 ⇒ `pathNotFound` / `ambiguousOwner` 零命中、顶层 `warnings` 为空；`model.warnings` SHALL **为空**（六个容器 = 3 个 `composite` + 3 个 `domain`，空 `path` 是选填容器的标准形态 ⇒ 零 `noPath`；`Backend` / `Desktop` 包根持 `path`，不在其列）——两层 MUST NOT 记混。
- **声明未兑现读数**：`observations` SHALL 恰为 **53** 条（52 条后端 + 1 条既有边面缺口 `Desktop.Views.Changes.Agent → Desktop.Components.Ui`）——前端命令面边因生成目录有了归属而全部兑现；后端 52 条仍全部未兑现（观察根无 workspace 清单 ⇒ Rust 事实面为零，属既有缺口，见 `desktop-archi-drift`「依赖 API 两桶对账与求差单点」）。

#### Scenario: 正常态而非停摆态

- **WHEN** 调用 `archi_graph(root)`
- **THEN** 返回正常态：`elements` 55、`relations` 129（后端 52 + 前端 77）、`diagnostics` 空；`/architecture` 页面呈现左栏树 + 右栏元素信息与画布两栏，无结构错面板、零「修好后再看」文案

#### Scenario: 成环与祖先禁边被机械拦住

- **WHEN** 假设模型被写成含 `orchestration ⇄ workflow`（纳入 dev 边）或 `Standard.Dailog → Standard`（把 hooks 归给容器）一类错误
- **THEN** 结构诊断立即返回 `cycle` / `dependsOnAncestor` 且画布停摆——该写法在交付面上不可通过（基线自身 MUST 为这两种写法的反例）

#### Scenario: 孤儿层级零命中

- **WHEN** 检查每个叶元素的 FQN 前缀链
- **THEN** 每一级祖先（`Backend` → `Backend.Core` → crate；`Desktop` → `Desktop.Views` → 叶；`Desktop` → `Desktop.GeneratedBindings`）均在模型内显式声明，`orphanHierarchy` 零命中（含 `Backend.Tools.ArchiApiGen` 链：`Backend` → `Backend.Tools`）

#### Scenario: 入口文档债不拉状态词

- **WHEN** 以本仓根跑一次全量检查
- **THEN** `apiFindings` 含 38 条 `docMissing`、顶层 `warnings` 为空、`model.warnings` **为空**（六个容器 = 3 个 `composite` + 3 个 `domain`，空 `path` 是合法形态；`Backend` / `Desktop` 包根持 `path`，不在其列）、`model.structuralErrors` 为空；`status = clean`、`passed = true`（`docMissing` 是 `warn` 且不纳管——若这 38 条把状态词抬成 `warnings` 即判据缺陷）。`noPath` 落 `model.warnings` 而非顶层 `warnings`，两处 MUST NOT 记混


## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.dev-team/architecture/graph/model/*.toml` | 数据面（**零改动**） | 55 元素 / 129 关系 / `gate` 全 `off` / 三处 `api = none` 逐字不变；本变更不动模型 |
| `.dev-team/architecture/models/**` | 资产面（**零正文改动**） | 11 份 `<FQN>.md`（文档）+ 11 份 `<FQN>.schema.json`（接口）在盘；`doc` 使用仍为 0 |
| `.dev-team/architecture/graph/INDEX.toml` | 索引 | `declared` 的判定改判「schema 在盘」（本仓与改前同值 ⇒ 除头注释一句外逐字节不变）；`generated` 列与条目字典序零变化 |
| 本能力实测段（spec 面） | 读数随仓演进 | 判据面分母 49、`docMissing` **38**（35 条 `declared` 侧 + 3 条 `api = none` 侧，均因派生 md 不在盘）、其余九值 0、`status = clean`、`passed = true`；历史链末段由「0 + 36 = 36（分母 46）」推进为「0 + 38 = 38（分母 49，资产齐全 11）」 |

### 模块增量：三资产切换后的基线读数

| 项 | 改前 | 改后 |
|---|---|---|
| `path` 非空元素 | 49 | 49 |
| 资产齐全（文档 + 接口都在盘） | 11（md 曾兼作接口真相） | 11（文档与接口分列两份） |
| `apiFindings` | 38（全 `docMissing`） | 38（全 `docMissing`，`message` 文案换面） |
| `api = none` 元素 | 3 | 3（均无 schema ⇒ 零 `apiUnexpected`） |
| `status` / `passed` | `clean` / `true` | `clean` / `true` |
