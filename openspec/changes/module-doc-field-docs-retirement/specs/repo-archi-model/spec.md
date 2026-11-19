# repo-archi-model Specification（delta: module-doc-field-docs-retirement）

## Purpose

本 delta 做**基线字段纪律与检索面的换面**（E-2）：元素字段 `docs: Vec<String>` 退役，改 `doc: Option<String>` 单值，且**只对 `api = none` 的元素有意义**——`api = declared`（缺省）元素的入口文档恒为派生落点 `<models>/<Domain>/<FQN>.md`，既不写也不许写。模型数据随之只动一处：`01-app.toml` 的 `docs = ["./README.md", "./AGENT.md"]` 一行删除——`App` 是 `package`（无 `path`），新纪律不要求也不允许它写 `doc`；两份根文档回归普通仓库文件，**不进模型面**。元素 / 关系计数零变化（53 / 120），`modelHash` 因该行删除而必然变化。检索面同时换面：ADR 不再经 `docs` 三源的 `text` 源自动关联到元素。

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
- **契约子树不计入模型计数**：`models/<Domain>/` 下的任何文件 MUST NOT 改变元素数 / 关系数 / `modelHash`——收面判据是「一级 + `.toml` 后缀」，子目录不递归即不可见。
- **关系归属**：每条 `[[relation]]` SHALL 声明在**其 `from` 元素所在文件**内（定位一致、diff 不跨文件散落）。
- **`gate` 全 `off`**：本基线内每个元素 SHALL 省略 `gate` 或显式写 `off`，MUST NOT 出现 `warn` / `strict`——冷启动 ratchet（收紧按模块独立变更，一个 change 收紧一块）。
- **`summary`**：每个元素 SHALL 有一行中文职责说明，MUST NOT 用 FQN 字面复述自身名字。
- **`path`**：叶元素必填、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器元素 SHALL 留空（**零例外**）。目录模块入口由 `desktop-frontend-module-entry` 承载：`App.Frontend.Components.Agent` 是 `kind = "module"` 的**叶**元素，`path` 为目录 `./packages/desktop/src/components/agent/`，入口是该目录内既有的 `index.tsx`（不再以文件型 `path` 自任端点）。
- **`entry`**：叶元素 SHALL **仅在约定默认指不到时**声明（单值、`./` 前缀、repo 相对）；单文件根元素（`views/welcome-view.tsx`、`components/agent/` 的 7 个组件文件、`standard/*.tsx`）、12 个目录模块根（入口由 ③ 的 `<root>/index.ts` / `<root>/index.tsx` 命中，含 `components/agent/` 与 `components/ui/`）与 14 个 `src/lib.rs` 命中的 crate MUST NOT 声明（根文件自身 / 语言约定默认已给出入口）。**本基线内显式 `entry` SHALL 恰为 3 处**（下列三值逐字在案；宿主能解析 `entry` 字段是这三行落地的前提，见 `desktop-archi-api`「生成物落点与入口查找」）：
  - `App.Backend.Shell` → `./packages/desktop/src-tauri/src/lib.rs`（目录型壳的根路径 `src-tauri/src/` 拼出的约定默认是 `src-tauri/src/src/lib.rs`，指不到）；
  - `App.Backend.Tools.ExportBindings` → `./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs`；
  - `App.Backend.Tools.ArchiApiGen` → `./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`。

  三处声明的后果 SHALL 被接受且可读：壳由 `apiEntryMissing`（`Absent`）转为 `docMissing`（`declared` 而派生契约 md 不在盘）；两个工具 crate 的入口判据面**整体不适用**（`api = none` 的元素退出入口判据，见 `desktop-archi-api-check`「契约判据词汇封闭十三值与开放分类」）——「有入口」与「无库 API」两个事实同时成立，MUST NOT 被读成「入口补齐 ⇒ 判据面清零」。MUST NOT 出现第四处显式 `entry`：其余叶元素由约定默认覆盖（单文件根 = 根文件自身、目录型根 = `<root>/index.ts(x)`、14 个 crate 的 `src/lib.rs`），多写即违「仅在约定默认指不到时声明」，并使本条的三处计数失守；`App.Frontend.Components.Agent` 的 `path` 由文件改目录后正是 ③ 的场景，**加行即违本条**。

  第四节「四条判据」（基准 / 落位 / 存在 / 跨元素唯一）与三态解析链归 `desktop-archi-api`，本节只钉基线数据；违规走 `apiFindings` 的 `apiEntryMissing`（`warn`，不进 `passed`），MUST NOT 引入结构化诊断。
- **`api = none` 两处显式留白**：两个工具 crate（`App.Backend.Tools.ExportBindings` / `App.Backend.Tools.ArchiApiGen`）SHALL 声明 `api = none`——二进制 crate 的公共面是 CLI 契约而非库 API，这是**真话**而非「还没写」；其余元素 SHALL 省略 `api`（取缺省 `declared`）。`api` 状态在本基线的角色是**入口文档载体的开关**：`declared` ⇒ 入口文档 = 派生 `contract_path`（不写 `doc`）；`none` ⇒ 入口文档 = `doc` 声明（可选）。「无入口」与「无 API」仍是两个不同事实，但**两者都不由 `api = none` 的元素承担判据**——本基线内 12 个前端目录根由 `desktop-frontend-module-entry` 的 `index.ts(x)` 经 ③ 命中、14 个 `src/lib.rs` 命中的 crate 由约定默认解析出入口，故 `apiEntryMissing` 面为 0；MUST NOT 由「两工具 crate 无判据」反推「它们没有入口」（三处显式 `entry` 在案），亦 MUST NOT 由「12 个前端目录元素仍有 `docMissing`」反推「无 API」。
- **`doc`**：SHALL 为**单值** `Option<String>`（repo 相对 POSIX 路径，`./` 前缀由归一剥除、不参与身份；无绝对前缀 / 无盘符 / 无 `..` / 无空段）。纪律三条：
  1. **只有 `api = none` 元素可写**；`api = declared`（含缺省）元素 MUST NOT 写 `doc`——它的入口文档就是派生落点 `contract_path`，写了即 `apiUnexpected`（两事实矛盾）；
  2. 值 MUST NOT 指向 `<archi_dir>/models/<Domain>/**`（契约资产的消费者是生成器与门禁，不是文档字段）与 `openspec/specs/**`（spec 语料非模型面）；
  3. 不写 = 无入口文档（合法状态，但会产出 `docMissing` `warn` 读数）——本变更新增两份工具 crate README 属数据面变更，MUST NOT 在本 change 内新建。

  `App`（`package`，无 `path`）SHALL 零 `doc`：包根的 `README.md` / `AGENT.md` 由仓库结构本身表达，**不进模型面**（原 `docs = ["./README.md", "./AGENT.md"]` 一行随 `docs` 字段退役删除）。
- **`metadata`**：本基线 SHALL 全空（`owner` / `tier` 一类自由键会与 `Cargo.toml`、目录结构并列为第二事实源）。
- **`desc`**：SHALL 为一句中文用途说明（「为什么依赖」），MUST NOT 写入证据清单（import 文件列表 / 依赖段位置）——证据由核对命令逐条取，模型内不存第二事实源。
- **计数复述单源**：模型文件的表头注释（如 `# 03 · infra 适配层（8 元素 / 24 关系）`）、本要求的逐文件计数表、与裁定记录 `§9` 的复算表 SHALL 在同一次模型改动内同步为同值；三处任一处陈旧（仍写 50 / 111 / 47 / 2 元素 1 关系，或仍写 51 / 113、`06` 段 28 / 64、仍写「4 个容器」）即漂移，按「同一 change 内补齐」处理，MUST NOT 让计数表与模型本体各说各话。**字段面扩容（`entry` / `api` / `doc`）与目录模块入口（barrel）不改变元素与关系计数**：12 个 barrel 是元素 `path` 内的新增文件，既不新增元素也不新增边；**元素粒度补漏改变计数**（`components/agent/session-panel.tsx` 与 `components/standard/tooltip.tsx` 补建元素：元素 +2、关系 +7）；**元素形态翻转改变容器 / 叶切分而不改总数**；**契约资产迁入 `models/<Domain>/`、索引迁入 `graph/`、`docs` → `doc` 换面均不改变任何计数**（前两者不进模型收面，第三者只换字段面）。`modelHash` 则随模型字节变化——本变更删 `01-app.toml` 一行即必然改哈希，该事实 SHALL 被如实记录，MUST NOT 写成「`modelHash` 逐字不变」。

裁定记录 SHALL 落位 `<archi_dir>/decisions/0001-model-granularity.md`，MUST NOT 只写在 change 目录内（proposal 归档后不再被日常读取，判据须随架构域可读、可检索）。

#### Scenario: 六文件字典序聚合零诊断

- **WHEN** 以仓库根调用 `archi_graph(root)` 并聚合 `models/` 一级的六个文件（同目录另有 `<Domain>/` 契约子树与若干 `*.schema.json`）
- **THEN** 解析成功、`diagnostics` 为空、`elements` 共 53 条、`relations` 共 120 条；每条元素与关系的 `modelFile` 指向其声明所在文件、`modelLine` 为该表头 / 该 `from` 键所在行；契约子树内的任何文件 MUST NOT 出现在 `modelFile` 里

#### Scenario: 字段纪律可机械审查

- **WHEN** 对六个文件 `grep` 计数 `gate` / `entry` / `api` / `doc` 出现行
- **THEN** `gate` 三档中只出现 `off`（或整行省略），`warn` / `strict` 零命中；`metadata` 零命中；**`docs` 零命中**（字段已退役，命中即 `unknownField` 停摆）；`doc` 行数 = 「`api = none` 且已声明入口文档的元素数」（本变更交付时 **0 行**——两份工具 crate README 属数据面后置；反向若出现 `api = declared` 元素的 `doc` 行即违纪律）；全部叶元素 `path` 以 `"./` 开头，全部容器 `path` 为空（**零例外**——`App.Frontend.Components.Agent` 已翻转为目录型 `module`）；`entry` **恰三行**（`04` 一行、`05` 两行）；`api` 恰两行且均为 `"none"`（两个工具 crate）

#### Scenario: 落点与裁定记录

- **WHEN** 检查架构域目录树
- **THEN** `models/` 的**一级**恰有六个 `NN-<域>.toml`（`find -maxdepth 1 -name '*.toml'` 读数 = 6，无 `INDEX.toml`）；`decisions/` 有 `0001-model-granularity.md`；`graph/` 恰有 `INDEX.toml` 一份；`models/<Domain>/` 下为契约资产（`<FQN>.md` + `<FQN>.schema.json`）；`api/` 目录不存在；`src/`、`docs/`、`openspec/specs/` 下无模型文件

#### Scenario: 计数复述三处同值

- **WHEN** 逐处读取 `06` 文件表头注释、本要求的计数表与 ADR `§1` / `§9` 的复算表
- **THEN** 三处同值：全基线 53 元素 / 120 关系（容器 9 + 叶 44；后端 49 + 前端 71）、`06` 段 **30 元素 / 71 关系**（3 容器 + 27 叶）、`05` 段 3 元素 / 3 关系、`03` 段 8 元素 / 24 关系、`04` 段 1 元素 / 13 关系；任一处仍写 51 / 113（含 `06` 段 28 / 64）、50 元素 / 111 关系 / 47 关系 / 2 元素 / 容器 10 / 「4 个容器」即不合格

#### Scenario: 三处入口与 12 个目录入口在案后的判据面归位

- **WHEN** 分别对壳、两个工具 crate、12 个前端目录根元素跑一次全量检查并读 `apiFindings`
- **THEN** 壳**零** `apiEntryMissing`，同趟恰一条 `docMissing`（显式 `entry` 命中 ⇒ ⑦ 不成立；`api = declared` 且派生契约 md 缺失 ⇒ ④ 成立）——两条 MUST NOT 同时出现（入口债与入口文档债是两回事）；两个工具 crate 因 `api = none` **退出入口判据**，各恰一条 `docMissing`（`doc` 未声明），该结论 MUST NOT 被重算成「无入口」或「无 API」；12 个前端目录根**零** `apiEntryMissing`（各自的 `index.ts` / `index.tsx` 由 ③ 命中），同趟**各一条** `docMissing`——12 条由此居第六段 43 条之内，MUST NOT 被读成「入口债已清 ⇒ 判据面干净」

#### Scenario: 契约子树与索引不改变模型读数

- **WHEN** 在 `models/<Domain>/` 放入契约 md 与 `.schema.json`、在 `graph/` 放入 `INDEX.toml` 之后，分别取一次 `archi_graph`（元素 / 关系 / 诊断）与一次 `archi_drift` 的 `modelHash`
- **THEN** 元素 53 / 关系 120 / `diagnostics` 空与放入前逐字相同；`modelHash` 逐字相同（取面为 `models/*.toml` 一级、按扩展名）；反向判据：把 `INDEX.toml` 复制到 `models/` 根后 `modelHash` 立刻变化且模型转停摆态——两侧读数即本条的存在理由

#### Scenario: docs 写回即停摆

- **WHEN** 在任一模型文件的元素表内写回 `docs = ["./README.md"]`
- **THEN** 解析返回 `unknownField` 诊断（字段面白名单已删 `docs`）、模型判定为停摆态；`archi_graph` 不再给图数据——`docs` → `doc` 的迁移 MUST 与字段白名单同批落地（本仓 `01-app.toml` 的一行已同批删除）

#### Scenario: 包根两份根文档不进模型

- **WHEN** 检查 `App` 元素的字段面与 `README.md` / `AGENT.md` 的模型关联
- **THEN** `App` 零 `doc`（它是 `package`、无 `path`，纪律不要求也不允许它写）；`archi_module("App")` 的响应内不含任何指向两份根文档的字段；两份文件仍在仓根、可被文件树与通用文件搜索抵达

### Requirement: 图自洽与停摆禁令

基线模型 SHALL 使四类结构诊断**全部零命中**：`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`；`archi_graph(root)` SHALL 返回正常态（元素与关系完整、`diagnostics` 为空），MUST NOT 停摆。

- **祖先显式声明**：所有容器元素 SHALL 在模型内声明（层级由 FQN 前缀推导，孤儿层级即 `orphanHierarchy`）；MUST NOT 依赖「隐式祖先自动补建」。
- **停摆零容忍**：交付面 MUST NOT 以「结构错 + 停摆面板」形态收尾——停摆是坏模型的处置，不是基线的可接受状态。
- **入口文档面零告警（本变更改写）**：`ArchiModuleDetail` 不再有 `warnings` 字段（`WarningKind` 随 `docs` 三源退役），入口文档的读数全部经漂移报告的 `apiFindings`：本基线 SHALL 恰为 **43 条 `docMissing`（`warn`、不纳管）**、其余十二值各 0 ⇒ `status` 仍 `clean`、`passed` 仍 `true`；`docNotFound` SHALL 零命中（没有任何元素声明 `doc`）。`path` 指向现存目录 / 文件，故漂移侧 `warnings`（`pathNotFound` / `noPath` / `ambiguousOwner` / `noPath`）SHALL 为空。

#### Scenario: 正常态而非停摆态

- **WHEN** 调用 `archi_graph(root)`
- **THEN** 返回正常态：`elements` 53、`relations` 120（后端 49 + 前端 71）、`diagnostics` 空；`/architecture` 页面呈现左栏树 + 画布 + 抽屉三段，无结构错面板、零「修好后再看」文案

#### Scenario: 成环与祖先禁边被机械拦住

- **WHEN** 假设模型被写成含 `orchestration ⇄ workflow`（纳入 dev 边）或 `Standard.Dailog → Standard`（把 hooks 归给容器）一类错误
- **THEN** 结构诊断立即返回 `cycle` / `dependsOnAncestor` 且画布停摆——该写法在交付面上不可通过

#### Scenario: 孤儿层级零命中

- **WHEN** 检查每个叶元素的 FQN 前缀链
- **THEN** 每一级祖先（`App` → `App.Backend` → `App.Backend.Core` → crate；`App` → `App.Frontend` → `Views.Components` → 叶）均在模型内显式声明，`orphanHierarchy` 零命中

#### Scenario: 入口文档债不拉状态词

- **WHEN** 以本仓根跑一次全量检查
- **THEN** `apiFindings` 含 43 条 `docMissing`、`model.warnings` 为空、`model.structuralErrors` 为空；`status = clean`、`passed = true`（`docMissing` 是 `warn` 且不纳管——若这 43 条把状态词抬成 `warnings` 即判据缺陷）

### Requirement: 模型与裁定记录的可检索性

基线落位后，既有两个只读工具面（`archi_search` / `archi_module`）SHALL 在**零代码改动**下直接检索到模型内容，MUST NOT 需要新增命令、工具、索引或字段：

- **元素检索**：关键词 SHALL 匹配元素 FQN / `summary` / `metadata` 值拼合文本（ASCII 大小写不敏感子串 + 驼峰分词），故 `App.Backend.Core.Archi` 一类元素 SHALL 可被 `archi` 关键词命中，返回薄条目（`fqn` / `kind` / `gate` / `summary`）。
- **入口文档检索（本变更换面）**：文档检索语料 SHALL 为「`path` 非空元素的**在盘入口文档载体**」——`api = declared` 取派生 `contractPath`、`api = none` 取 `doc` 声明，条目退化为**一条 repo 相对路径字符串**（`ArchiSearchResult.doc: Vec<String>`），`scope` 词为 `elements` / `doc` / `all`；命中判据仍只吃路径与文件名去扩展名（不读正文）。MUST NOT 把 ADR 路径写进元素 `doc` 强行关联。
- **裁定记录的抵达路径（本变更换面，显式承认）**：ADR（`decisions/*.md`）**退出 archi 自动检索语料**——它是原 `docs` 三源的 `text` 源（正文含元素 FQN 即自动关联），该来源随 `docs` 退役消失。ADR 的三条替代抵达路径 SHALL 为：① 入口文档（契约 md 或 `doc`）内的 markdown 链接；② 左栏文件树；③ 通用文件搜索（`grep` / README 索引）。本能力 MUST NOT 为 ADR 新增第四种检索来源，MUST NOT 引入 `specs ↔ element` 映射。
- **一跳定位**：`archi_module(<FQN>)` SHALL 给出该元素的 `dependsOn` / `dependedBy` / `children` / `doc`（单值，仅 `api = none` 有值）与 `modelFile` / `modelLine`（指向声明表头，可直接 `edit`），使 agent 能「查名 → 定位 → 改文件」闭环。
- **契约零变化**：本能力只是给两工具第一份非空语料，其注入矩阵、只读档位与响应形状的其余部分 MUST NOT 因本变更改变（`docs` / `warnings` 两个位次的退场与 `doc` 的进场除外）。

#### Scenario: 关键词检索命中元素与入口文档

- **WHEN** 以 `query = "archi"`、`scope = all` 调用 `archi_search`
- **THEN** `elements` 含 `App.Backend.Core.Archi`、`App.Backend.Infra.ArchiRuntime`（`kind = module`、`gate = off`、`summary` 非空）与 `App.Frontend.Views.Architecture`；`doc` 含 `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md`（路径字符串，零四字段、零正文）

#### Scenario: ADR 不再自动入语料

- **WHEN** 以 `query = "granularity"`（只出现在 `decisions/0001-model-granularity.md` 的文件名与正文内）调用 `archi_search(scope = "doc")`
- **THEN** 零条目——`decisions/**` 不在入口文档语料内；该 ADR 仍可经 ① 入口文档链接 / ② 文件树 / ③ 通用文件搜索抵达，三条路径均不改变检索语料

#### Scenario: 单元素详情给出一跳邻域与定位字段

- **WHEN** 调用 `archi_module(element = "App.Backend.Core.Archi")`
- **THEN** `dependsOn` 恰一条 `App.Backend.Core.Foundation`，`dependedBy` 含 `App.Backend.Infra.AgentRuntime`、`App.Backend.Infra.ArchiRuntime` 与 `App.Backend.Shell`，`children` 为空，`doc` 为 `null`（该元素 `api = declared`）；`element.modelFile` = `.dev-team/architecture/models/02-backend-core.toml` 且 `modelLine` 指向该元素表头行

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.dev-team/architecture/models/01-app.toml` | 包根与域容器 | `App`（`package`，**零 `doc`**：两份根文档由仓库结构表达、不进模型）、`App.Backend`、`App.Frontend`；零关系；**本变更删 `docs` 一行**（与字段白名单同批，删后文件其余字节不变） |
| `.dev-team/architecture/models/05-backend-tools.toml` | `Tools` 容器与两个工具 crate | 两个元素保留 `api = "none"`；**本变更零改动**（`doc` 行属数据面后置：两份 README 尚未创建，写入即 `docNotFound`） |
| `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md` + `.schema.json` | 试点契约 | **零改动**——它是第六段唯一零判据样本，也是 `enabled` 判据的唯一在案证据（在盘且含 `api-op` / `api-types` fence） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 裁定记录 | 文件本体留在盘上；`archi_decisions_dir` / `archi_decisions_rel_dir` / `DECISIONS_DIR_NAME` 三个派生符号已删除 ⇒ 程序化访问面归零（将来需要须重新引入单点，MUST NOT 手写字面量） |
| `crates/core/archi/src/model.rs` | 字段面 | `Element.doc: Option<String>`（缺省 `None`）；`docs: Vec<String>` 已删 |
| `crates/core/archi/src/parse/toml.rs` | 字段白名单 | `RawElement` 含 `doc: Option<String>`、不含 `docs`；`deny_unknown_fields` 主体不变（`docs` 写成即 `unknownField`；`doc` 写成数组由 serde `invalid type` 归 `invalidValue`） |
| `crates/core/archi/src/api/paths.rs` | 载体派生单点 | declared → `contract_path`；none → `doc` 归一后 join root（形态校验复用 `normalize_root` + `is_repo_relative`，后者提 `pub(crate)`） |
