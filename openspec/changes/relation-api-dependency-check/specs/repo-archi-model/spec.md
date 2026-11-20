# repo-archi-model Specification (Delta)

## MODIFIED Requirements

### Requirement: 基线落位与模型文件划分

架构模型基线 SHALL 落位 `<archi_dir>/graph/model/`（架构域根经 `foundation::layout::archi_dir` 派生、模型声明目录经 `archi_graph_model_dir` 派生，本期落点 `.dev-team/architecture/graph/model/`），MUST NOT 落到 `src/`、`docs/`、`openspec/specs/`、`<archi_dir>/models/`（契约资产目录）或任何其它位置。

模型 SHALL 划分为**六个一级文件**，文件名形如 `NN-<域>.toml`，字典序即聚合序，每个元素与每条关系 SHALL 恰在一处声明（跨文件或同文件重复即 `duplicateElement`）：

| 文件 | 内容 | 元素 | 关系 |
|---|---|---|---|
| `01-app.toml` | 包根与两个域容器 | 3 | 0 |
| `02-backend-core.toml` | `Core` 容器 + 7 个 core crate 元素 | 8 | 9 |
| `03-backend-infra.toml` | `Infra` 容器 + **7** 个 infra crate 元素 | **8** | **24** |
| `04-backend-shell.toml` | Tauri 壳（命令面 / 装配 / bindings 导出） | 1 | **13** |
| `05-backend-tools.toml` | `Tools` 容器 + `export-bindings` crate + `archi-api-gen` crate | **3** | **3** |
| `06-frontend.toml` | 前端 `Shell` / 9 视图 / 3 视图内子域 / **13** 共享组件 / **目录模块 `Components.Agent`** / `Ui` / **生成物中继元素 `GeneratedBindings`** + **三个**容器（`App.Frontend` 容器在 `01`） | **32** | **74** |

合计 SHALL 为 **55 元素 / 123 关系**，逐文件可加：元素侧 `3 + 8 + 8 + 1 + 3 + 32 = 55`（容器 9 + 叶 46），关系侧 `0 + 9 + 24 + 13 + 3 + 74 = 123`。关系侧另按取数口径分：**123 = 后端 49（`Cargo.toml` `[dependencies]` 的 workspace 内声明，拆在 `02`–`05`：9 + 24 + 13 + 3）+ 前端 74（`import` 证据，全在 `06`）**；两种拆分 MUST 得同值。前端 74 的构成 SHALL 为 **66 条叶 → 叶边 + 8 条 `App.Frontend.Components.Agent` 的 barrel 端点边**（3 条出边：入口 `index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`；5 条入边：`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / **`Views.Config`** / `Views.Explores` import `@/components/agent`）——这 8 条由「`components/agent/` 是 `Components.Agent` 的目录模块入口」与「`@/components/agent` → `Components.Agent` 映射」两条规则必然导出，MUST NOT 从记数中剔除；12 个目录模块入口的**其余 re-export 全在本元素内**（`from == to` 自边被丢弃）⇒ 零新边。其中 `06-frontend.toml` 的 32 元素 = **3 个容器**（`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Standard`）+ **29 个叶**（`Shell` 1 + 9 视图 + 3 子域 + 13 共享组件（7 `agent` + 6 `standard`）+ `Components.Agent` 1 + `Ui` 1 + **`GeneratedBindings` 1**）；`App.Frontend` 容器 SHALL 只在 `01-app.toml` 声明一次，`06` MUST NOT 重复声明（重复即 `duplicateElement`，并会使元素总数变成 56 而非 55）。

- **模型声明面与契约资产面 SHALL 按目录分家**：模型声明面是 `<archi_dir>/graph/model/` 的**一级**——恰六个 `NN-<域>.toml`，MUST NOT 出现其他一级 `*.toml`（尤其 MUST NOT 出现 `INDEX.toml`）；契约资产面是 `<archi_dir>/models/<分层目录…>/`——`<FQN>.md`（手写真相）+ `<FQN>.schema.json`（生成物），布局与命名归 `desktop-archi-api`「契约资产形态与唯一手写真相」。两面的分离判据是「目录」与「`.toml` vs `.md` / `.schema.json`」，MUST NOT 引入目录级豁免清单或改名方案；契约索引落 `<archi_dir>/graph/INDEX.toml`（`archi_graph_dir`），与模型声明目录（`graph/model/`）同树不同层——索引落 `graph/model/` 内会被当模型文件解析并使模型停摆，理由见 `desktop-archi-model`「多文件聚合与声明定位」。
- **契约子树不计入模型计数**：`models/<分层目录…>/` 下的任何文件 MUST NOT 改变元素数 / 关系数 / `modelHash`——收面判据是「`graph/model/` 一级 + `.toml` 后缀」，契约目录整体不在收面内。
- **关系归属**：每条 `[[relation]]` SHALL 声明在**其 `from` 元素所在文件**内（定位一致、diff 不跨文件散落）。
- **`gate` 全 `off`**：本基线内每个元素 SHALL 省略 `gate` 或显式写 `off`，MUST NOT 出现 `warn` / `strict`——冷启动 ratchet（收紧按模块独立变更，一个 change 收紧一块）。
- **`summary`**：每个元素 SHALL 有一行中文职责说明，MUST NOT 用 FQN 字面复述自身名字。
- **`path`**：叶元素必填、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器元素 SHALL 留空（**零例外**）。目录模块入口由 `desktop-frontend-module-entry` 承载：`App.Frontend.Components.Agent` 是 `kind = "module"` 的**叶**元素，`path` 为目录 `./packages/desktop/src/components/agent/`，入口是该目录内既有的 `index.tsx`（不再以文件型 `path` 自任端点）。**生成物中继元素（本变更新增）**：`App.Frontend.GeneratedBindings` 是 `kind = "module"` 的**叶**元素，`path` 为生成目录 `./packages/desktop/src/types/generated/`——它持有三个生成文件（`bindings.ts` / `bindings.zod.ts` / `bindings.zod.check.ts`）的归属，从而让「前端 import 生成物」这条依赖的目标可归属（归属判据零改动，走既有 `OwnerIndex` 最长前缀）。
- **`entry`**：叶元素 SHALL **仅在约定默认指不到时**声明（单值、`./` 前缀、repo 相对）；单文件根元素（`views/welcome-view.tsx`、`components/agent/` 的 7 个组件文件、`standard/*.tsx`）、**13 个目录型根元素**（入口由 ③ 的 `<root>/index.ts` / `<root>/index.tsx` 命中，含 `components/agent/` 与 `components/ui/`）与 14 个 `src/lib.rs` 命中的 crate MUST NOT 声明（根文件自身 / 语言约定默认已给出入口）。**生成物目录 MUST NOT 声明 `entry`**：它是三个叶子文件的集合、没有模块入口（`entry_state = Absent`），造一个 `index.ts` 只为消 `Absent` 属造假入口（`api = none` 已退出入口判据，零 `apiEntryMissing`，见 `desktop-archi-api-check`）。**本基线内显式 `entry` SHALL 恰为 3 处**（下列三值逐字在案；宿主能解析 `entry` 字段是这三行落地的前提，见 `desktop-archi-api`「生成物落点与入口查找」）：
  - `App.Backend.Shell` → `./packages/desktop/src-tauri/src/lib.rs`（目录型壳的根路径 `src-tauri/src/` 拼出的约定默认是 `src-tauri/src/src/lib.rs`，指不到）；
  - `App.Backend.Tools.ExportBindings` → `./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs`；
  - `App.Backend.Tools.ArchiApiGen` → `./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`。

  三处声明的后果 SHALL 被接受且可读：壳由 `apiEntryMissing`（`Absent`）转为 `docMissing`（`declared` 而派生契约 md 不在盘）；两个工具 crate 的入口判据面**整体不适用**（`api = none` 的元素退出入口判据，见 `desktop-archi-api-check`「契约判据词汇封闭十三值与开放分类」）——「有入口」与「无库 API」两个事实同时成立，MUST NOT 被读成「入口补齐 ⇒ 判据面清零」。MUST NOT 出现第四处显式 `entry`：其余叶元素由约定默认覆盖（单文件根 = 根文件自身、目录型根 = `<root>/index.ts(x)`、14 个 crate 的 `src/lib.rs`），多写即违「仅在约定默认指不到时声明」，并使本条的三处计数失守；`App.Frontend.Components.Agent` 的 `path` 由文件改目录后正是 ③ 的场景，**加行即违本条**。

  第四节「四条判据」（基准 / 落位 / 存在 / 跨元素唯一）与三态解析链归 `desktop-archi-api`，本节只钉基线数据；违规走 `apiFindings` 的 `apiEntryMissing`（`warn`，不进 `passed`），MUST NOT 引入结构化诊断。
- **`api = none` 三处显式留白**：两个工具 crate（`App.Backend.Tools.ExportBindings` / `App.Backend.Tools.ArchiApiGen`）与**生成物中继元素**（`App.Frontend.GeneratedBindings`）SHALL 声明 `api = none`——前两者的公共面是 CLI 契约而非库 API、后者是生成物（唯一真相在生成器，手写契约必然漂），三者都是**真话**而非「还没写」；其余元素 SHALL 省略 `api`（取缺省 `declared`）。`api` 状态在本基线的角色是**入口文档载体的开关**：`declared` ⇒ 入口文档 = 派生 `contract_path`（不写 `doc`）；`none` ⇒ 入口文档 = `doc` 声明（可选）。「无入口」与「无 API」仍是两个不同事实，但**两者都不由 `api = none` 的元素承担判据**——本基线内 12 个前端目录根由 `desktop-frontend-module-entry` 的 `index.ts(x)` 经 ③ 命中、14 个 `src/lib.rs` 命中的 crate 由约定默认解析出入口，故 `apiEntryMissing` 面为 0；MUST NOT 由「三处 `api = none` 无判据」反推「它们没有入口」（三处显式 `entry` 在案；生成物目录是「无模块入口」的真形态），亦 MUST NOT 由「12 个前端目录元素仍有 `docMissing`」反推「无 API」。
- **`doc`**：SHALL 为**单值** `Option<String>`（repo 相对 POSIX 路径，`./` 前缀由归一剥除、不参与身份；无绝对前缀 / 无盘符 / 无 `..` / 无空段）。纪律三条：
  1. **只有 `api = none` 元素可写**；`api = declared`（含缺省）元素 MUST NOT 写 `doc`——它的入口文档就是派生落点 `contract_path`，写了即 `apiUnexpected`（两事实矛盾）；
  2. 值 MUST NOT 指向 `<archi_dir>/models/<分层目录…>/**`（契约资产的消费者是生成器与门禁，不是文档字段）与 `openspec/specs/**`（spec 语料非模型面）；
  3. 不写 = 无入口文档（合法状态，但会产出 `docMissing` `warn` 读数）——两份工具 crate README 属数据面变更，MUST NOT 在本能力内新建。

  `App`（`package`，无 `path`）SHALL 零 `doc`：包根的 `README.md` / `AGENT.md` 由仓库结构本身表达，**不进模型面**（原 `docs = ["./README.md", "./AGENT.md"]` 一行随 `docs` 字段退役删除）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。**命令面边的 `desc` SHALL 写明「经生成的 IPC 绑定」**（本变更起改指生成物中继元素，语义仍是「前端经生成物调后端命令面」）。
- **计数复述单源**：模型文件的表头注释（如 `# 03 · infra 适配层（8 元素 / 24 关系）`）、本要求的逐文件计数表、与裁定记录 `§1` / `§9` 的复算表 SHALL 在同一次模型改动内同步为同值；三处任一处陈旧即漂移，按「同一 change 内补齐」处理，MUST NOT 让计数表与模型本体各说各话。**本变更的增量与既有陈旧（同批写实，逐项留痕）**：① **本变更净增量** = 元素 **+1**（生成物中继元素）、关系 **±0**（12 条命令面边只改 `to` 与 `desc`，不增删）⇒ `06` 段由 31 / 74 → **32 / 74**、全基线由 54 / 123 → **55 / 123**；② **既有陈旧校正** = 本变更之前 `06` 表头仍写「30 元素 / 72 关系」、ADR `§1` 仍写「53 元素 / 121 关系」，而模型本体早已是 31 / 74 与 54 / 123（组件元素 `standard/combobox.tsx` 与其两条边未计入表头与复算表）——本变更同批把这两处写实，MUST NOT 被读成「本变更新增了一个组件元素」。**字段面扩容（`entry` / `api` / `doc` / `apis`）与目录模块入口（barrel）不改变元素与关系计数**：12 个 barrel 是元素 `path` 内的新增文件，既不新增元素也不新增边（入口只 re-export 本元素直属文件 ⇒ `from == to` 自边被丢弃）；**元素粒度补漏改变计数**（历史：`components/agent/session-panel.tsx` 与 `components/standard/tooltip.tsx`、`standard/combobox.tsx` 补建元素即各 +1）；**元素形态翻转改变容器 / 叶切分而不改总数**（历史：`Components.Agent` 容器 → 目录模块：容器 10 → 9、叶 43 → 44）；**契约资产迁入 `models/<分层目录…>/`、索引迁入 `graph/`、`docs` → `doc` 换面均不改变任何计数**。`modelHash` 则随模型字节变化——本变更新增元素行、12 条关系的 `to` / `desc` 与 `06` 表头都改字节，故 `modelHash` 必然变，该事实 SHALL 被如实记录，MUST NOT 写成「`modelHash` 逐字不变」。**读数写实边界**：本条只要求写实**模型表头 / 本表 / ADR 三处**与直接判据面（`desktop-archi-api-check` 的基线表、`desktop-archi-api` 的入口面实测）；其余 capability spec 内的历史读数（如 `desktop-frontend-module-entry` 的「71 条 / 44 个 `path` 非空」、`desktop-archi-contract-panel` 的「43 条存量」）不在本变更的追改面内——它们是各自变更交付时的读数，与既有「53 / 120 vs 54 / 123」的陈旧同一处置。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `<archi_dir>/graph/model/` 一级的六个文件（契约子树在另一目录 `<archi_dir>/models/<分层目录…>/`）
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 **55** 条、`relations` 共 **123** 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行；契约子树内的任何文件 MUST NOT 出现在 `modelFile` 里

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` / `entry` / `api` / `doc` / `apis` 出现行
- **THEN** `gate` 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；**`docs` 零命中**（字段已退役，命中即 `unknownField` 停摆）；`doc` 行数 = 「`api = none` 且已声明入口文档的元素数」（本变更交付时 **0 行**——两份工具 crate README 与生成物面都不写 `doc`；反向若出现 `api = declared` 元素的 `doc` 行即违纪律）；全部叶元素 `path` 以 `"./` 开头，全部容器 `path` 为空（**零例外**——`App.Frontend.Components.Agent` 与 `App.Frontend.GeneratedBindings` 都是目录型 `module` 叶）；`entry` **恰三行**（`04` 一行、`05` 两行）；`api` **恰三行**且均为 `"none"`（两个工具 crate + 生成物中继元素）；**`apis` 零命中**（本基线 `apis` 全留空——生成物面是唯一真相，其余元素待有真实依赖清单时再填）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `<archi_dir>/graph/model/` 的**一级**恰有六个 `NN-<域>.toml`（`find -maxdepth 1 -name '*.toml'` 读数 = 6，无 `INDEX.toml`）；`decisions/` 有 `0001-model-granularity.md`；`graph/` 恰有 `INDEX.toml` 一份与 `model/` 子目录；`models/<分层目录…>/` 下为契约资产（`<FQN>.md` + `<FQN>.schema.json`）；`api/` 目录不存在；`src/`、`docs/`、`openspec/specs/` 下无模型文件

#### Scenario: 计数复述三处同值

- **WHEN** 逐处读取 `06` 文件表头注释、本要求的计数表与 ADR `§1` / `§9` 的复算表
- **THEN** 三处同值：全基线 **55 元素 / 123 关系**（容器 9 + 叶 46；后端 49 + 前端 74）、`06` 段 **32 元素 / 74 关系**（3 容器 + 29 叶）、`05` 段 3 元素 / 3 关系、`03` 段 8 元素 / 24 关系、`04` 段 1 元素 / 13 关系；任一处仍写 53 / 120、54 / 123（含 `06` 段 30 / 72、31 / 74）、51 / 113、50 / 111、47 关系、「4 个容器」，或前端恒等式仍写 67 + 7（实为 **66 + 8**）即不合格

#### Scenario: 三处入口与 12 个目录入口在案后的判据面归位

- **WHEN** 分别对壳、两个工具 crate、生成物中继元素、12 个前端目录根元素跑一次全量检查并读 `apiFindings`
- **THEN** 壳**零** `apiEntryMissing`，同趟恰一条 `docMissing`（显式 `entry` 命中 ⇒ 入口判据不成立；`api = declared` 且派生契约 md 缺失 ⇒ `docMissing` 成立）——两条 MUST NOT 同时出现（入口债与入口文档债是两回事）；两个工具 crate 与生成物中继元素因 `api = none` **退出入口判据**，各恰一条 `docMissing`（`doc` 未声明），该结论 MUST NOT 被重算成「无入口」或「无 API」（生成物目录的 `Absent` 入口事实另见 `desktop-archi-api`「本仓入口面实测」）；12 个前端目录根**零** `apiEntryMissing`（各自的 `index.ts` / `index.tsx` 由 ③ 命中），同趟**各一条** `docMissing`——12 条由此居当前段 36 条之内，MUST NOT 被读成「入口债已清 ⇒ 判据面干净」

#### Scenario: 契约子树与索引不改变模型读数

- **WHEN** 在 `models/<分层目录…>/` 放入契约 md 与 `.schema.json`、在 `graph/` 放入 `INDEX.toml` 之后，分别取一次 `archi_graph`（元素 / 关系 / 诊断）与一次 `archi_drift` 的 `modelHash`
- **THEN** 元素 55 / 关系 123 / `diagnostics` 空与放入前逐字相同；`modelHash` 逐字相同（取面为 `graph/model/*.toml` 一级、按扩展名）；反向判据：把 `INDEX.toml` 复制到 `graph/model/` 根后 `modelHash` 立刻变化且模型转停摆态——两侧读数即本条的存在理由

#### Scenario: docs 写回即停摆

- **WHEN** 在任一模型文件的元素表内写回 `docs = ["./README.md"]`
- **THEN** 解析返回 `unknownField` 诊断（字段面白名单已删 `docs`）、模型判定为停摆态；`archi_graph` 不再给图数据——`docs` → `doc` 的迁移 MUST 与字段白名单同批落地（本仓 `01-app.toml` 的一行已同批删除）

#### Scenario: 包根两份根文档不进模型

- **WHEN** 检查 `App` 元素的字段面与 `README.md` / `AGENT.md` 的模型关联
- **THEN** `App` 零 `doc`（它是 `package`、无 `path`，纪律不要求也不允许它写）；`archi_module("App")` 的响应内不含任何指向两份根文档的字段；两份文件仍在仓根、可被文件树与通用文件搜索抵达

### Requirement: 前端关系只取 import 证据
前端关系 SHALL 只由**已声明元素覆盖范围内**文件的 `import` 语句证成，MUST NOT 凭语义直觉连边：

- **目标归属**：被 import 的路径 SHALL 按「最具体已声明元素」归属（`@/components/ui/button` → `Components.Ui`；`@/components/standard/drawer` → `Standard.Drawer`；`@/components/agent`（barrel）→ `Components.Agent`；`./flow` / `../renderers` 一类目录 specifier 由解析器的 index 折叠落到 `<dir>/index.ts`，归属与折叠前被打到的文件同元素；`@/types/generated/bindings` → **`GeneratedBindings`**）。
- **命令面边（本变更改写）**：`types/generated/bindings` 的 `import` SHALL 映射为**命令面依赖**，指向 **`App.Frontend.GeneratedBindings`**（`desc` 写明「经生成的 IPC 绑定取数与调用」）——这是前端对「后端命令面」的唯一合法边来源（逻辑关系仍是「前端依赖后端命令面」，代码关系是「前端 → 生成物」；生成物自身的上游不声明，见下条）。**该映射不再落 `observations`**：生成目录已由中继元素持有归属（`path` = 生成目录），扫描解析出目标文件后 `OwnerIndex` 命中该元素 ⇒ 12 条命令面边**有证据、即兑现**。`src/lib` / `src/hooks` 未被建模，故壳层对命令面的依赖仍不可见（已知缺口，见裁定记录 `§8`）。
- **生成链不声明**：`App.Frontend.GeneratedBindings → App.Backend.Shell`（或 core 侧任一元素）MUST NOT 出现在模型内——生成器读 `#[tauri::command]` 与 DTO 不是 `import` 事实，声明即永久 `unfulfilledDeclaration`；要表达它得先有「生成链对账」判据（独立能力）。
- **不解析面零边**：指向 `hooks/`、`lib/`、`types/dto` 的 `import` MUST NOT 产生关系（本期不解析）；`App.Frontend.Views.Welcome` 一类「只经 hooks 取数」的元素因而可无后端边。
- **包含与自环**：子元素归属由 FQN 表达，MUST NOT 连边；`from == to` 的自环 MUST NOT 出现（同元素内部互引不建边）——**12 个目录模块入口的 re-export 正是这一条**：入口与被聚合的直属文件同属一个元素，故补入口**零新边**；生成目录内的三个生成文件同属 `GeneratedBindings`，其互引同样零边。
- **父 → 子的 import 边合法**：`Views.Changes → Views.Changes.Flow`（详情页组装流程画布）是装配依赖，不是包含；禁的只是「子 → 父」。
- **目录模块作端点仅限既有 barrel 一例**：13 个目录型 `path` 的元素中唯一**由入口 re-export 产出端点边**的是 `App.Frontend.Components.Agent`（其入口 `components/agent/index.tsx` 重导出 3 个子元素的导出，且有 **5** 个元素 import 它）；其端点边 SHALL 恰为 **8 条**——3 条出边（`index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`）+ **5** 条入边（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Config` / `Views.Explores` import `@/components/agent`）。其余三个容器（`Views` / `Components` / `Standard`）MUST NOT 出现在任何关系的任一端点上；其余 12 个目录型根元素的 **re-export 不产出任何端点边**（目标全在本元素直属文件内、自边被丢弃；若出现即说明有跨元素 / 跨子元素 re-export）——该 12 个元素自身作为普通元素仍可有其它 `import` 关系（如 `Views.Changes → Views.Changes.Flow`），MUST NOT 把本条读成「它们不得参与任何关系」。**生成物中继元素只作 `to`**：12 条入边、**零出边**（生成文件不自写 `import` 到其它元素）。
- **计数恒等式**：前端边 SHALL 为 **74 条 / 25 个源**，且 MUST 满足 `74 = 66（叶 → 叶）+ 8（barrel 端点边）`；若某次落笔得到 66，说明这 8 条被漏掉（判据自相矛盾），MUST NOT 以「容器不入边」为由删减——要改只能改本要求的字段规则或映射规则（撤销 `Components.Agent` 的目录模块入口），不能在实现期折中。**12 条命令面边计入 66**（它们的 `from` 是叶、`to` 是叶），改指向不改变总数。

#### Scenario: 每条边可指到 import 语句

- **WHEN** 逐条核对 74 条前端边（25 个源）
- **THEN** 每条均能在其 `from` 元素覆盖范围内的某文件里找到对应 `import`（证据清单在 change 报告留痕）；反向抽查「有 import、无关系」的跨元素引用为零——唯一例外是裁定记录 `§8` 登记的**既有边面缺口**（11 条已建元素间的跨元素 `import` 未声明，其中 `Views.Changes.Flow → Views.Changes` 是「子 → 父」形态、声明即 `dependsOnAncestor` 停摆，故补齐须另开 change），缺口 MUST 单列且 MUST NOT 在本变更内补齐；12 个目录入口的 re-export 属「同元素内互引」，MUST NOT 被算进这 11 条

#### Scenario: 命令面边恰一处来源且已兑现

- **WHEN** 检查全部指向 `App.Frontend.GeneratedBindings` 的前端边，并同时审查报告的 `observations`
- **THEN** 恰 **12** 条，其来源文件均 import 了 `types/generated/bindings`（`Views.Agents` / `Views.AgentDebug` / `Views.Architecture` / `Views.Changes` / `Views.Changes.Flow` / `Views.Changes.Agent` / `Views.Config` / `Views.DbInspector` / `Views.Explores` / `Views.Info` / `Components.Agent.AgentSessionView` / `Components.Agent.SessionPanel` 一类），MUST NOT 存在以 `hooks` / `lib` 为唯一证据的命令面边；`observations` 内 MUST NOT 出现这 12 个元素对（有主 ⇒ 兑现），MUST NOT 出现 `GeneratedBindings` 的任何出边

#### Scenario: 目录模块端点边恰为 barrel 的 8 条

- **WHEN** 统计模型内全部以 `App.Frontend.Components.Agent` 为任一端点的前端关系，并另查其余目录型根元素是否有由 re-export 产出的端点边
- **THEN** 前者恰 8 条：出边 `AgentInput` / `AgentSessionView` / `SessionPanel`，入边 `Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Config` / `Views.Explores`；后者（其余 12 个目录型根）**零 re-export 产出的端点边**——它们的目标全在本元素内、自边被丢弃（这 12 个元素自身作为普通元素仍可有其它 `import` 关系）；`App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Standard` 零端点，前端边总数 74 = 66 + 8

#### Scenario: 不解析面零边与自环零出现

- **WHEN** 审查模型内前端边集合
- **THEN** 不存在仅由「import `@/hooks/...`」「import `@/lib/...`」「import `@/types/dto`」证成的边；`from == to` 的自环零条；`Components.Ui` 只作为被依赖方出现（出边为零，其内部互引与入口的 `export *`（实测 **41** 条）均不建边）；`GeneratedBindings` 只作为被依赖方出现

### Requirement: 图自洽与停摆禁令

基线模型 SHALL 使四类结构诊断**全部零命中**：`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`；`archi_graph(root)` SHALL 返回正常态（元素与关系完整、`diagnostics` 为空），MUST NOT 停摆。

- **祖先显式声明**：所有容器元素 SHALL 在模型内声明（层级由 FQN 前缀推导，孤儿层级即 `orphanHierarchy`）；MUST NOT 依赖「隐式祖先自动补建」（本能力体系的既有前提）。
- **停摆零容忍**：交付面 MUST NOT 以「结构错 + 停摆面板」形态收尾——停摆是坏模型的处置，不是基线的可接受状态。
- **入口文档面零告警（本变更改写）**：`ArchiModuleDetail` 不再有 `warnings` 字段（`WarningKind` 随 `docs` 三源退役），入口文档的读数全部经漂移报告的 `apiFindings`：本基线 SHALL 恰为 **36 条 `docMissing`（`warn`、不纳管）**、其余十二值各 0 ⇒ `status` 仍 `clean`、`passed` 仍 `true`；`docNotFound` SHALL 零命中（没有任何元素声明 `doc`）。`path` 指向现存目录 / 文件 ⇒ `pathNotFound` / `ambiguousOwner` 零命中、顶层 `warnings` 为空；`model.warnings` SHALL 恰为 **9 条 `noPath`**（9 个容器元素无 `path`，是「容器 `path` 恒空」纪律的必然读数，本变更前后逐条相同）——两层 MUST NOT 记混。
- **声明未兑现读数（本变更改写）**：`observations` SHALL 由本变更前的 **62** 条（49 条后端 + 13 条前端，后者含 12 条命令面边）降为 **50** 条（49 条后端 + 1 条既有边面缺口 `Views.Changes.Agent → Components.Ui`）——12 条命令面边因生成目录有了归属而兑现；后端 49 条仍全部未兑现（观察根无 workspace 清单 ⇒ Rust 事实面为零，属既有缺口，见 `desktop-archi-drift`「依赖 API 两桶对账与求差单点」）。

#### Scenario: 正常态而非停摆态

- **WHEN** 调用 `archi_graph(root)`
- **THEN** 返回正常态：`elements` 55、`relations` 123（后端 49 + 前端 74）、`diagnostics` 空；`/architecture` 页面呈现左栏树 + 右栏元素信息与画布两栏，无结构错面板、零「修好后再看」文案

#### Scenario: 成环与祖先禁边被机械拦住

- **WHEN** 假设模型被写成含 `orchestration ⇄ workflow`（纳入 dev 边）或 `Standard.Dailog → Standard`（把 hooks 归给容器）一类错误
- **THEN** 结构诊断立即返回 `cycle` / `dependsOnAncestor` 且画布停摆——该写法在交付面上不可通过（基线自身 MUST 为这两种写法的反例）

#### Scenario: 孤儿层级零命中

- **WHEN** 检查每个叶元素的 FQN 前缀链
- **THEN** 每一级祖先（`App` → `App.Backend` → `App.Backend.Core` → crate；`App` → `App.Frontend` → `Views.Components` → 叶；`App` → `App.Frontend` → `GeneratedBindings`）均在模型内显式声明，`orphanHierarchy` 零命中（含 `App.Backend.Tools.ArchiApiGen` 链：`App` → `App.Backend` → `App.Backend.Tools`）

#### Scenario: 入口文档债不拉状态词

- **WHEN** 以本仓根跑一次全量检查
- **THEN** `apiFindings` 含 36 条 `docMissing`、顶层 `warnings` 为空、`model.warnings` 恰 **9** 条 `noPath`（9 个容器元素无 `path`——「容器 `path` 恒空」纪律的必然读数，本变更前后逐条相同）、`model.structuralErrors` 为空；`status = clean`、`passed = true`（`docMissing` 是 `warn` 且不纳管——若这 36 条把状态词抬成 `warnings` 即判据缺陷）。`noPath` 落 `model.warnings` 而非顶层 `warnings`，两处 MUST NOT 记混

### Requirement: 覆盖判据的机械核对与漂移纪律
元素的完整性与关系的证据性 SHALL 可**机械核对**（不靠人读图判断），核对口径恰为三条：

| # | 核对 | 判据 |
|---|---|---|
| 1 | 双向覆盖 | 判据命中的 crate 目录 / 视图目录 / 视图内子域 / 共享组件文件 / `components/ui/` / **生成物目录** 集合 == 模型叶元素 `path` 集合（无漏无多；**既有已知缺口**单列、不计入判红、其余零差集——本期三条：`crates/tools/specta-zod-patched`、`components/file/file-viewer.tsx`、`views/architecture/agent/`，与边面缺口清单同批登记在裁定记录 `§8`） |
| 2 | 关系证据 | 后端逐边等于 `Cargo.toml` `[dependencies]` 的 workspace 内声明集；前端逐边可指到 `import` 语句（**命令面边**同样有证据：声明式约定 + 生成物目录的中继元素归属 ⇒ 12 条 `to = GeneratedBindings` 的边由扫描面命中，MUST NOT 再落 `observations`） |
| 3 | 图自检 | `archi_graph` 正常态、`diagnostics` 空、`warnings` 空 |

- **「生成物目录」是第 4 类覆盖判据（本变更新增）**：`packages/desktop/src/types/generated/` 必须恰由 `App.Frontend.GeneratedBindings` 持有（`kind = module` / `api = none` / 零 `entry` / 零出边）；该判据的正当性不是「觉得该有个元素」，而是「不归属则目标不可归属、`apis` 在 12 条最强名字面上永远判不了」——引用该目录的文件是既建元素（视图与组件），归属它使代码关系与逻辑关系的差被一次消除。MUST NOT 用该条为「其它无主目录」开脱（新增此类元素须改本 requirement 并同批计数）。
- 本期核对 SHALL 以**命令 + 留痕**执行（当次 change 的 `reports/archi/` 核对留痕内），MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**` 作语料——crate 语料必须自足可移植，耦合本仓内容会让 crate 单测随仓库演进变红。机制化（漂移判定进流程门禁）归 `desktop-archi-gate`；本能力只定义模型内容与核对判据。
- **漂移纪律**：新增 / 删除 crate、视图、视图内子域、共享组件、**生成物目录**，或新增跨元素 `import` 依赖时，SHALL 在同一 change 内同步模型（元素 / 关系 / `path`），MUST NOT 留待「下次整理」；判据之外的元素与关系 MUST NOT 存在（「觉得该有」不是判据）。
- **「同一 change 内同步」的核对口径**：当 change 新增 crate 时，核对 SHALL 同时看三件事——元素与容器归属在案、该 crate 的 `[dependencies]` 边逐条在案、涉及文件的计数（元素 / 关系）同步为同值；三者缺一即判不合格（只补元素不补边、或只改模型不改计数，都是漂移）。已知缺口清单 SHALL 只含「先于该 change 存在」的条目，MUST NOT 用「已在缺口清单里」为本次新增项开脱。本期前端边面缺口 SHALL 逐条列在裁定记录 `§8`（11 条已建元素间的跨元素 `import` 未声明），普查口径为「`06` 已建元素覆盖范围内文件的跨元素 `import`」与「`[[relation]]` 集合」两侧求差；缺口是既有事实、MUST NOT 被抹成零差集结论。

#### Scenario: 双向覆盖零差集

- **WHEN** 以判据枚举两侧集合并求差集
- **THEN** 两个方向差集均为空（含生成物目录 ↔ `App.Frontend.GeneratedBindings` 一条）；任一新增目录 / 文件未声明即出现非空差集，核对失败（唯一例外为裁定记录 `§8` 的既有已知缺口清单：`crates/tools/specta-zod-patched`、`components/file/file-viewer.tsx`、`views/architecture/agent/` 三条 SHALL 各自单条出现，MUST NOT 被算作本变更的差集，也不得掩盖其它差集）

#### Scenario: 判据之外零元素零边

- **WHEN** 审查模型内元素与关系的来源
- **THEN** 每个元素可归属到某条覆盖判据（crate / 视图 / 子域 / 共享组件 / `ui` / 壳 / 容器 / **生成物目录**），每条关系可归属到 `Cargo.toml` 声明或 `import` 语句；无「直觉元素」与「直觉边」

#### Scenario: 漂移在同一变更内补齐

- **WHEN** 某 change 新增一个视图并新增一条跨视图 `import`
- **THEN** 该 change 的交付内含新增视图元素与其关系；未补齐时双向覆盖核对与「有 import 无关系」抽查同时失败

#### Scenario: 新增 crate 的三件套核对

- **WHEN** 对新增 `crates/infra/archi` 的 change 执行覆盖核对
- **THEN** 三件事同时成立：`App.Backend.Infra.ArchiRuntime` 元素与 `App.Backend.Infra` 容器归属在案；`archi-runtime` 的三条 `[dependencies]` 边与壳指向它的那条边逐条在案；`03` 段 8 元素 / 24 关系与全基线 55 / 123 在模型表头、计数表与 ADR `§9` 三处同值——缺任一件即不合格

#### Scenario: 生成物目录的归属判据单独成立

- **WHEN** 审查 `packages/desktop/src/types/generated/` 的归属与它的关系面
- **THEN** 该目录恰由一个元素持有（`App.Frontend.GeneratedBindings`，`kind = module` / `api = none` / `path` 指向该目录 / 零 `entry` / 零出边）；12 条入边全为「前端元素 → 该元素」；把它从模型移除会让 12 条边退回 `observations`（即本条判据的存在理由），MUST NOT 以「生成物不该建模」为由删除它而不改本 requirement

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.dev-team/architecture/graph/model/06-frontend.toml` | 前端模型数据 | **+1 元素** `App.Frontend.GeneratedBindings`（`module` / `gate = off` / `api = none` / `path = ["./packages/desktop/src/types/generated/"]` / `summary` 中文一行 / 无 `entry` / 无 `doc`）；**12 条关系** `to = "App.Backend.Shell"` → `to = "App.Frontend.GeneratedBindings"` 且 `desc` 微调为「经生成的 IPC 绑定取数与调用」；表头注释「30 元素 / 72 关系」→「**32 元素 / 74 关系**」与元素构成行（含共享组件 13、生成物中继 1） |
| 其余五个模型文件 | 零改动 | `01` / `02` / `03` / `04` / `05` 的元素、关系、表头计数逐字不变 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 裁定记录 | `§1` / `§9` 计数写实（53 / 121 → **55 / 123**（容器 9 + 叶 46）；`06` 30 / 72 → **32 / 74**；`§9` 的前端恒等式 65 + 7 = 72 → **66 + 8 = 74**）；`§8` 的「命令面边落 `observations`」段改写为「已归属生成物中继元素 ⇒ 边即兑现」；新增「生成物目录」覆盖判据一行与「上游生成链不声明」的理由一行 |
| `.dev-team/architecture/graph/INDEX.toml` | 生成物（非手改） | `pnpm api:export` 重导：条目 45 → **46**（新元素 `status = "absent"`、`types = 0`、`ops = 0`） |
| `crates/core/archi/src/scan/owner.rs` | 归属判据（零改动） | 新元素走既有最长前缀；`gate = off` 的文件仍计入 `unmatchedFiles[]`（本仓 3 个生成文件因此仍在 `unmatchedFiles` 内——**报告字段集**零变化，MUST NOT 被读成「归属没生效」；`unmatchedFiles[]` 的**条目集合**另含本变更新增 3 源、计数 412 → 415，验收口径见 `desktop-archi-drift`「报告结构与 status 五值」的条目级账目） |
| 本能力实测段（spec 面） | 改 | 55 / 123（容器 9 + 叶 46）、`06` 32 / 74、前端恒等式 74 = 66 + 8、`observations` 62 → **50**、`docMissing` **35 → 36**（spec 旧读数 43 是两次变更之前的数）；并写明「53 / 120 与 30 / 72（ADR 为 53 / 121）的既有陈旧由本变更同批校正」 |

### CLI: 核对命令（本能力「命令 + 留痕」口径）

| 命令 | 用途 | 本变更期望读数 |
|---|---|---|
| `archi_graph(root)` | 元素 / 关系 / 诊断 | `elements = 55`、`relations = 123`、`diagnostics = []` |
| `archi_drift(root, "full")` | 覆盖 / 归属 / 契约判据 | `observations` = 50（49 后端 + 1 既有边面缺口）、`apiFindings` 的 `docMissing` = 36、`model.warnings` = 9 条 `noPath`（容器面，前后不变）、`status = clean`、`passed = true`；`scan` = `scannedFiles` / `unmatchedFiles` 412 → **415**、`excluded` 341 → **352**、`matched` 0、`unresolved` 0（候选 = `scannedFiles + excluded`：753 → **767** 自洽）——**既有 `excluded[]` / `unmatchedFiles[]` 条目零增删改、新增条目全部命中既有规则**（+3 源文件落 unmatched / +11 测试与语料落 excluded，逐条点名见 `desktop-archi-drift`「报告结构与 status 五值」的条目级账目），故 `scan.unmatchedFiles` 仍含 3 个生成文件（`gate` 全 `off` 的既有分支）；MUST NOT 用三面计数相等验收归属（判定口径见 `desktop-archi-drift`「报告结构与 status 五值」） |
| `archi_file_tree(root)` | 文件树归属标记 | 生成目录的三个文件呈 `FileTreeMark::elementRoot` 归属（元素根标记），不再是「无主」 |
| `pnpm api:export` | 契约索引与生成物重导 | `graph/INDEX.toml` 45 → 46 条（新元素 `status = absent`）；`api:check` 零 diff |
