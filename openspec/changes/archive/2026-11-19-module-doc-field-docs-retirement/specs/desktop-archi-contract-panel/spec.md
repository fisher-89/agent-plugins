# desktop-archi-contract-panel Specification（delta: module-doc-field-docs-retirement）

## Purpose

本 delta 只改契约面板的两处口径（E-2）：① `archi_api_check` 的 `enabled` 判据由「`<archi_models_dir>` 递归含 ≥1 份 `*.md`」收紧为「存在任一 `path` 非空元素的入口文档载体在盘且含 api fence」（E-1 移交的未定点，属**行为变化**：`models/` 里的杂项笔记不再误启用）；② 判据轴的 `category` chip 由六值扩**七值**（+ `doc`，承载 `docMissing` / `docNotFound` / `apiDocMissing`）。抽屉契约段的「不与文档段混装」措辞随文档段退役改写，其余呈现与交互零改动。

## MODIFIED Requirements

### Requirement: 检查命令契约（archi_api_check）

后端 SHALL 提供一条**零写入**的检查命令 `archi_api_check(root: String) -> Result<ArchiApiCheck, String>`，出参 SHALL 为「生成物新鲜度」一条轴的完整事实：

| 字段 | 类型 | 语义 |
|---|---|---|
| `enabled` | `bool` | 契约面是否启用（**存在任一 `path` 非空元素的入口文档载体在盘且正文含 api fence 语言标**），`api::enabled` 单点判定，MUST NOT 在命令层另判一套 |
| `meta` | `ArchiApiMeta` | 生成载元：`toolVersion`（生成器 crate 版本）+ `commonDigest`（映射规则资产摘要 `sha256:<hex>`，渲染侧截前 12 位）；取自 `api::meta()`，MUST NOT 在命令层重算摘要 |
| `mismatched` | `Vec<String>` | 内容不符：文件在盘但字节与渲染结果不同 |
| `orphaned` | `Vec<String>` | 应删未删：盘上存在、命名符合生成物判据、不在应生成集 |
| `missing` | `Vec<String>` | 应生成未生成 |

- **薄包装形态**：命令 SHALL 为薄包装（`check_root` → 调 `core/archi` 领域函数 → 映射错误），领域装配在 `archi_api_check_inner(root: &Path)` 内以便离 Tauri 测试；命令层 MUST NOT 承载渲染、比对或路径归一。`enabled` 的计算需要模型元素表（`elements_of` 单点取数，与判据面同源），命令层 MUST NOT 自行读模型。
- **零写盘零 spawn**：命令 MUST NOT 创建 / 覆盖 / 删除任何文件，MUST NOT 派生进程，MUST NOT 引入 watch 或轮询。
- **三类差异即生成预览**：三列合起来 SHALL 恰好是「点生成会写什么」的完整清单（内容不符的会被覆盖、缺失的会被创建、孤儿不会被删）。
- **`enabled = false` 仍照跑**：契约面未启用时命令 SHALL 返回 `Ok`（而非 `Err` 或空壳），三类差异照实计算——此时应生成集只含 `INDEX.toml`，故 `missing` 通常就是那一条；MUST NOT 因 `enabled = false` 返回空的结构化结果让面板无从呈现冷启动态。
- **`enabled` 判据（本变更收紧，行为变化）**：判据 SHALL 为「**存在任一 `path` 非空元素的入口文档载体在盘且正文含 api fence 语言标**」——载体按 `api` 状态取值（`declared` → 派生契约 md、`none` → `doc` 声明），fence 语言标单点复用 `contract::FENCE_OP` / `FENCE_TYPES`。三条后果必须说准：
  1. **冷启动态**不再是「递归零份 `*.md`」而是「**没有任何元素的载体带 fence**」——`models/` 一级的六个 `NN-<域>.toml`、`models/notes.md`（非任何元素的载体）、`<Domain>/notes.md`（无 fence）三者都 MUST NOT 构成启用证据；
  2. 本仓读数**仍为 `true`**（试点契约 `models/Infra/App.Backend.Infra.ArchiRuntime.md` 在盘且含 `api-op` / `api-types` fence），MUST NOT 被改写成「模型在即启用」；
  3. 判据与判据面同源（同一 `doc_facts` 事实），命令层 MUST NOT 另判一套、MUST NOT 缓存。
- **路径串口径**：三列元素 SHALL 为 repo 相对 POSIX 串（剥 `root` 前缀、`\` → `/`、去前导 `./`），经 `archi::display` 单点产出；MUST NOT 在命令层或前端另写一份归一。
- **错误面**：空白 / 不可读 / 非目录 root、模型结构诊断非空、契约资产非法 SHALL 以 `Result<T, String>` 的 `Err` 返回且零部分结果（`ApiError::message()` 透传原文，MUST NOT 包装、MUST NOT 补默认值）。
- **出线形状**：`ArchiApiCheck` / `ArchiApiMeta` SHALL 带 `specta::Type` 与 `serde(rename_all = "camelCase")`，字段序即声明序，经 `bindings:export` 生成到 `bindings.ts` / `bindings.zod.ts`；MUST NOT 手写前端类型镜像。**本变更这两条命令的形状零改动**（`enabled` 的语义收紧不改字段名 / 类型），但绑定文件不是零 diff——`dto.rs` 的 `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 离场与 `doc` 三处进场（`ArchiModuleDetail.doc` / `ArchiSearchResult.doc` / `ArchiElement.doc`）经 `bindings:export` 反映到生成物；`ArchiApiCheck` / `ArchiApiMeta` 的字段名 / 字段序 / 联合成员零变化、`bindings:export` 幂等。
- **`meta` 只带两项**：`ArchiApiMeta` SHALL 只含 `toolVersion` / `commonDigest`。

#### Scenario: 零写盘且三类差异齐备

- **WHEN** 在本仓根分别于调用前后对工作树取字节快照并调用 `archi_api_check`
- **THEN** 两次快照完全相同；出参三列按「内容不符 / 应删未删 / 应生成未生成」分别列出条目（零差异时为三个空数组），`enabled = true`，`meta.toolVersion` 与生成器 crate 版本一致

#### Scenario: 冷启动态照实报告

- **WHEN** 观察根的 `models/` 含六个 `NN-<域>.toml` 与一份 `notes.md`（非任何元素的载体），且没有任何元素的载体带 fence
- **THEN** `enabled = false` 且命令返回 `Ok`；`missing` 含 `<archi_graph_dir>/INDEX.toml` 一条，`mismatched` / `orphaned` 依盘上实况列出；MUST NOT 返回 `Err`、MUST NOT 因「模型文件在场」或「有杂项 md」把 `enabled` 报成 `true`

#### Scenario: 载体带 fence 才启用

- **WHEN** 在某 `api = declared` 元素的派生契约路径放入一份带 `api-op` fence 的 md 后重跑
- **THEN** `enabled` 由 `false` 翻为 `true`，三类差异按新应生成集重算；判定函数与判据面相同（`doc_facts` 上的 `has_api_fence`），命令层零第二份判断

#### Scenario: 结构错与非法契约一律 Err

- **WHEN** 模型含任一结构诊断后调用该命令，或把某份契约改成不可解析
- **THEN** 命令返回 `Err`（文案点名成因），三列不出线；磁盘字节零变化

#### Scenario: 三条路径串同源

- **WHEN** 审查出参三列的串形态与 `archi::display` 的输出
- **THEN** 逐条相等（反斜杠归一为 `/`、无前导 `./`、无绝对路径）；契约生成物串形如 `.dev-team/architecture/models/<Domain>/<FQN>.schema.json`、索引串形如 `.dev-team/architecture/graph/INDEX.toml`；源码内零第二份「strip 前缀」实现

### Requirement: 两轴并列呈现与状态矩阵

左栏第三视图 SHALL 把两个正交事实并列呈现，MUST NOT 混成一个「问题总数」：

| 轴 | 事实源 | 谁修 | 呈现 |
|---|---|---|---|
| **生成物新鲜度** | `archi_api_check`（新命令） | 机器（点生成） | 上区：三类差异分组 + 各自计数 |
| **契约 vs 代码** | 漂移报告的 `apiFindings`（既有取数，父级已取） | 人或 agent | 下区：按 `category` **七值** chip 筛选 + 各自计数 |

- **两轴分区独立计数**：两区各自呈现计数，MUST NOT 出现跨轴求和的总数。
- **`category` 词汇零新增（前端面）**：chip 取值 SHALL 取自报告既有开放值集——**`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor` / `doc`**（本变更由六值扩七值，`doc` 承载 `docMissing` / `docNotFound` / `apiDocMissing`），计数由同一份 `apiFindings` 聚合；MUST NOT 在前端另立词表或类别常量（chip 集合由数据驱动，新增分类只改打标处）。
- **条目点击 = 聚焦**：点击判据条目 SHALL 调既有 `onFocus(element)`（与元素树 / 文件树同一语义）；`element` 为空的条目 SHALL 只呈文本、不聚焦。
- **「打开」走通用读面**：契约 md / schema / 生成物 / 入口文档的路径行 SHALL 经既有 `FileViewer` + `read_file` 打开（`read_file` 已是通用绝对路径读面），MUST NOT 为面板新增专用读命令。
- **五态全画**：

| 态 | 呈现 |
|---|---|
| `loading` | 「解析中…」 |
| `enabled = false` | 「契约面未启用」+ 冷启动说明（应生成集此时只含 `INDEX.toml`），且仍呈现 `missing` 实况——MUST NOT 渲成空面板 |
| 零差异 + 零判据 | 「生成物最新，判据 0 条」 |
| 检查 invoke reject | 面板内行内错误（沿 `archi-file-tree-error` 形态），画布与抽屉照常可用 |
| 生成失败 | 错误回显在面板顶部，文案如实（非原子、失败点之前已写） |

- **不阻断画布**：检查或生成失败 MUST NOT 阻断图渲染、MUST NOT 使画布进入停摆态。
- **`doc` 分组的呈现语义（本变更新增）**：`doc` chip 下的条目是**入口文档面**的读数——`docMissing`（`warn`，43 条存量）表现为「该模块还没有入口文档」的迁移待办，`docNotFound`（`error`）表现为「`doc` 声明指不到文件」的笔误，`apiDocMissing`（`warn`）表现为「有文档无约定」。三者 MUST NOT 被合并呈现为同一句文案，亦 MUST NOT 被计入「生成物新鲜度」轴（它们是契约 vs 代码轴的读数）。

#### Scenario: 两轴并列且计数不合并

- **WHEN** 打开契约视图，检查返回 3 条差异（2 不符 + 1 缺失）且报告含 43 条判据
- **THEN** 上区呈「差异 2 条不符 / 1 条缺失 / 0 条孤儿」、下区呈「判据 43 条」并按 `category` 分 chip 计数（其中 `doc` 一档可能占满 43 条）；界面上不出现「共 46 个问题」类跨轴汇总

#### Scenario: chip 过滤与聚焦

- **WHEN** 点下 `doc` chip 并点击其中一条 `docNotFound` 条目
- **THEN** 列表只剩该 category 的条目（计数不变、只筛显示）；条目点击触发一次 `onFocus(element)`——画布切到该元素 1-hop、抽屉取该元素详情（`archi_module` 恰一次）

#### Scenario: 冷启动态不渲空面板

- **WHEN** `archi_api_check` 返回 `enabled = false` 且 `missing` 含 `INDEX.toml`
- **THEN** 面板呈「契约面未启用」+ 冷启动说明，并在差异区照实列出 `INDEX.toml` 一条；MUST NOT 呈现为「无差异」空态

#### Scenario: 检查失败不阻断画布

- **WHEN** `archi_api_check` 以 `Err` 返回（模型结构错 / 契约资产非法）
- **THEN** 面板呈行内错误并可重试；画布与抽屉照常渲染（图零降级），MUST NOT 进入停摆态

### Requirement: 抽屉契约段

元素抽屉 SHALL 在「概览」与「入口文档」之间呈现本元素的契约事实，数据来源零新增：

- **三路径行**：`api.status = declared` 时 SHALL 逐行呈现契约 md / schema / 生成物三个路径（`ArchiApiRef.contractPath` / `schemaPath` / `generatedPath`），各行可经 `FileViewer` 打开；`status = none` 时 SHALL 呈一行「无契约（`api = none` 或契约文件不在案）」且三路径行不渲染。
- **契约 md 行＝入口文档行（本变更改写）**：`declared` 元素的入口文档**就是** `contractPath`，故本段第一行 `ContractPathRow("契约", api.contractPath)` 同时承担「入口文档」的呈现职责——抽屉 MUST NOT 另渲第二条入口文档行；`api = none` 元素的入口文档走抽屉的「入口文档」段（`ArchiModuleDetail.doc`，见 `desktop-archi-canvas`「抽屉内容与本期边界」）。
- **本元素判据**：SHALL 从父级同一份 `apiFindings` 按 `element === fqn` 过滤后逐条列出（`kind` + `message`），零命中时一句「本元素无漂移」；MUST NOT 为抽屉单独 invoke 检查或报告。三个新值（`docMissing` / `docNotFound` / `apiDocMissing`）按同一规则呈现，MUST NOT 为它们另写分支。
- **零新取数**：本段数据 SHALL 全部来自既有 `ArchiModuleDetail.api` 与父级已取的报告——MUST NOT 新增 IPC、MUST NOT 二次调用 `archi_module`。
- **不与入口文档段混装**：契约正文 MUST NOT 进抽屉正文通路（正文仍由通用 `read_file` 按路径承担，原「`docs` 关联集」一说不存在）；本段只呈状态与路径，MUST NOT 内联契约 md 内容。

#### Scenario: 有契约元素的三路径

- **WHEN** 聚焦契约在案的元素（如 `App.Backend.Infra.ArchiRuntime`）打开抽屉
- **THEN** 契约段呈 `status = declared` + 三路径行，点任一行的「查看」经 `read_file` 打开该文件；本元素判据 0 条时呈「本元素无漂移」

#### Scenario: 无契约元素的空态

- **WHEN** 聚焦 `api = none` 或无契约文件的元素
- **THEN** 契约段呈一行「无契约」说明，三路径行不渲染；不报错、不显示失效路径

#### Scenario: declared 元素零第二条入口文档

- **WHEN** 聚焦任一 `api = declared` 元素并审查抽屉内指向入口文档的行数
- **THEN** 恰一条（契约段第一行）；「入口文档」段不渲染（`doc` 为 `null`）；两条同时出现即判违例

#### Scenario: 抽屉零新 invoke

- **WHEN** 打开任意元素抽屉并审查 invoke 次数
- **THEN** `archi_module` 恰一次（既有取数），`archi_api_check` / `archi_drift` 均零新增调用

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 零改动 | `archi_api_check(root) -> Result<ArchiApiCheck, String>` 薄包装与 `*_inner` 测试缝逐字不变；`enabled` / 三列全部来自 `core/archi` |
| `crates/core/archi/src/api/mod.rs::enabled` | 改（判据收紧） | 启用判据 = 任一 `path` 非空元素的载体在盘且 `DocFact::Present { has_api_fence: true }`（不再等价于「递归任意 `*.md`」）；命令层 MUST NOT 另判一套、MUST NOT 缓存 |
| `crates/core/archi/src/api/artifact_check` | 改（签名透传） | 因 `enabled` 需元素表，`artifact_check` 内部经 `elements_of` 单点取模型后调用 `enabled(root, elements)`；DTO 形状与命令层零改动 |
| `packages/desktop/src/views/architecture/components/contract-panel.tsx` + `hooks/use-archi-api-check.ts` | 零改动（数据驱动） | 两轴呈现、状态矩阵、「契约面未启用」文案分支与三类差异渲染逐字不变；`category` chip 集合由 `apiFindings` 聚合得出，新增 `doc` 档不要求前端改常量 |
| `packages/desktop/src/views/architecture/components/element-drawer.tsx` | 改（契约段） | 三路径行、本元素判据过滤、「无契约」空态逐字保留；新增「契约行即入口文档行」的单一性约束（零第二条入口文档行） |
