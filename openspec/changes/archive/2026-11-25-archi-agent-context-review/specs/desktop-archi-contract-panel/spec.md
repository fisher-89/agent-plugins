# desktop-archi-contract-panel Specification（delta: archi-agent-context-review）

## Purpose

本 delta 只换呈现面的判据锚与三路径语义，零前端功能改动。

- **`enabled` 判据**：由「载体带 fence」改为「`api = declared` 元素的 schema 在盘」；应生成集只剩三类（手写 `.schema.json` 永不出现在 `mismatched` / `orphaned` / `missing`）。
- **两轴呈现**：`category` chip 仍由数据聚合（七组、`doc` 只剩 `docMissing`），三值离场零前端硬编码；冷启动文案改「接口面未启用」。
- **元素信息面板**：三路径各自独立出线（`contractPath` = 文档、`schemaPath` = 接口、`generatedPath` = 生成物），「只有一份在盘」不再退化成三条全空。

命令面签名、出参形状、取数纪律与五态呈现逐字不变。

## MODIFIED Requirements

### Requirement: 检查命令契约（archi_api_check）

后端 SHALL 提供一条**零写入**的检查命令 `archi_api_check(root: String) -> Result<ArchiApiCheck, String>`，出参 SHALL 为「生成物新鲜度」一条轴的完整事实：

| 字段 | 类型 | 语义 |
|---|---|---|
| `enabled` | `bool` | 接口面是否启用（**存在任一 `path` 非空 ∧ `api = declared` 元素的 `<FQN>.schema.json` 在盘**），`api::enabled` 单点判定，MUST NOT 在命令层另判一套 |
| `meta` | `ArchiApiMeta` | 生成载元：`toolVersion` + `commonDigest`；取自 `api::meta()` |
| `mismatched` | `Vec<String>` | 内容不符：文件在盘但字节与渲染结果不同 |
| `orphaned` | `Vec<String>` | 应删未删：盘上存在、命名符合生成物判据、不在应生成集 |
| `missing` | `Vec<String>` | 应生成未生成 |

- **应生成集只剩三类**：`INDEX.toml` / `.i.rs` / `.i.ts`。`<FQN>.schema.json` MUST NOT 进应生成集（它是手写真相），因此 MUST NOT 出现在 `mismatched` / `orphaned` / `missing` 任一列——收面漏改的症状是「11 份手写文件全进 `orphaned`」（全红而不是少一条）。
- **薄包装形态**：命令 SHALL 为薄包装（`check_root` → 领域函数 → 错误映射），装配在 `archi_api_check_inner(root: &Path)` 内；命令层 MUST NOT 承载渲染、比对或路径归一；`enabled` 的元素表经 `elements_of` 单点取数（与判据面同源）。
- **零写盘零 spawn**：命令 MUST NOT 创建 / 覆盖 / 删除任何文件，MUST NOT 派生进程，MUST NOT 引入 watch 或轮询。
- **三类差异即生成预览**：三列合起来 SHALL 恰好是「点生成会写什么」的完整清单。
- **`enabled = false` 仍照跑**：接口面未启用时命令 SHALL 返回 `Ok`（而非 `Err` 或空壳），三类差异照实计算——此时应生成集只含 `INDEX.toml`，故 `missing` 通常就是那一条；MUST NOT 返回全空的伪造结果。
- **`enabled` 判据（本变更换锚）**：判据 SHALL 为「存在任一 `path` 非空 ∧ `api = declared` 元素在盘 `<FQN>.schema.json`」。三条后果必须说准：
  1. **冷启动态**是「没有任何元素的接口文件在盘」——`graph/model/` 一级的六个 `NN-<域>.toml`、`models/notes.md`（非元素载体）、`models/Backend/notes.schema.json`（非 `<FQN>` 主干）三者都 MUST NOT 构成启用证据；
  2. 本仓读数**仍为 `true`**（11 份 schema 在盘），MUST NOT 被改写成「模型在即启用」；
  3. 判据与判据面同源，命令层 MUST NOT 另判一套、MUST NOT 缓存。`api = none` 元素在盘的 schema 是 `apiUnexpected` 违约，MUST NOT 构成启用证据。
- **路径串口径**：三列元素 SHALL 为 repo 相对 POSIX 串（剥 `root` 前缀、`\` → `/`、去前导 `./`），经 `archi::display` 单点产出。
- **错误面**：空白 / 不可读 / 非目录 root、模型结构诊断非空、接口文件非法（任意一份 schema 装载校验失败）SHALL 以 `Err` 返回且零部分结果（`ApiError::message()` 透传原文）。
- **出线形状**：`ArchiApiCheck` / `ArchiApiMeta` SHALL 带 `specta::Type` 与 `serde(rename_all = "camelCase")`，经 `bindings:export` 生成；MUST NOT 手写前端类型镜像。本变更两命令形状零改动。
- **`meta` 只带两项**（`toolVersion` / `commonDigest`）逐字沿用。

#### Scenario: 零写盘且三类差异齐备

- **WHEN** 在本仓根分别于调用前后对工作树取字节快照并调用 `archi_api_check`
- **THEN** 两次快照完全相同；三列按「内容不符 / 应删未删 / 应生成未生成」分别列出条目（零差异时为三个空数组），`enabled = true`

#### Scenario: 手写接口文件不进三列

- **WHEN** 手改一份 `<FQN>.schema.json`（合法改动）后调用 `archi_api_check`
- **THEN** 三列中该文件零出现（既不 `mismatched` 也不 `orphaned`）；`.i.rs` / `.i.ts` / `INDEX.toml` 的随动差异（若有）照实列出

#### Scenario: 冷启动态照实报告

- **WHEN** 观察根的 `graph/model/` 含六个 `NN-<域>.toml`、`models/` 含一份 `notes.md` 与一份非 `<FQN>` 主干的 `*.schema.json`，且没有任何 `declared` 元素的 schema 在盘
- **THEN** `enabled = false` 且命令返回 `Ok`；`missing` 含 `<archi_graph_dir>/INDEX.toml` 一条；MUST NOT 返回 `Err`、MUST NOT 因「模型文件在场」或「有杂项文件」把 `enabled` 报成 `true`

#### Scenario: 接口文件在盘才启用

- **WHEN** 给某 `api = declared` 元素补一份合法 `<FQN>.schema.json` 后重跑
- **THEN** `enabled` 由 `false` 翻为 `true`，三类差异按新应生成集重算；判定函数与判据面相同，命令层零第二份判断

#### Scenario: 结构错与非法接口一律 Err

- **WHEN** 模型含任一结构诊断后调用该命令，或把某份 schema 改成不可装载
- **THEN** 命令返回 `Err`（文案点名成因），三列不出线；磁盘字节零变化

### Requirement: 两轴并列呈现与状态矩阵

左栏第三视图 SHALL 把两个正交事实并列呈现，MUST NOT 混成一个「问题总数」：

| 轴 | 事实源 | 谁修 | 呈现 |
|---|---|---|---|
| **生成物新鲜度** | `archi_api_check` | 机器（点生成） | 上区：三类差异分组 + 各自计数 |
| **接口 vs 代码** | 漂移报告的 `apiFindings` | 人或 agent | 下区：按 `category` chip 筛选 + 各自计数 |

- **两轴分区独立计数**：MUST NOT 出现跨轴求和的总数。
- **`category` 词汇零新增（前端面）**：chip 取值 SHALL 取自报告既有开放值集——**`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor` / `doc`**（仍七组）；本变更后 `doc` 组只剩 `docMissing` 一个成员，chip 由 `apiFindings` 数据聚合、**空组自然不渲**，MUST NOT 在前端另立词表或类别常量。
- **词表收缩零前端硬编码**：`apiReexportDrift` / `apiDocMissing` / `docNotFound` 三值离场后，前端 MUST NOT 残留任何按 `kind` 的分支或常量（已核实 `contract-panel.tsx` 零前端词表）。
- **条目点击 = 聚焦**：点击判据条目 SHALL 调既有 `onFocus(element)`；`element` 为空的条目只呈文本、不聚焦。
- **「打开」走通用读面**：文档 / 接口 / 生成物的路径行 SHALL 经既有 `FileViewerDrawer` + `read_file` 打开，MUST NOT 为面板新增专用读命令。
- **五态全画**：

| 态 | 呈现 |
|---|---|
| `loading` | 「解析中…」 |
| `enabled = false` | 「接口面未启用」+ 冷启动说明（应生成集此时只含 `INDEX.toml`），且仍呈现 `missing` 实况——MUST NOT 渲成空面板 |
| 零差异 + 零判据 | 「生成物最新，判据 0 条」 |
| 检查 invoke reject | 面板内行内错误，画布与元素信息面板照常可用 |
| 生成失败 | 错误回显在面板顶部，文案如实（非原子、失败点之前已写） |

- **不阻断画布**：检查或生成失败 MUST NOT 阻断图渲染、MUST NOT 使画布进入停摆态（停摆态的唯一成因仍是模型结构错）。
- **`doc` 分组的呈现语义（本变更收窄）**：`doc` chip 下只剩 `docMissing`（`warn`）——表现为「该模块还没有文档」的迁移待办，永不纳管 `status`；旧的三值分述（`docNotFound` 的笔误语义、`apiDocMissing` 的「有文档无约定」语义）MUST NOT 残留。`doc` 组 MUST NOT 被计入「生成物新鲜度」轴。

#### Scenario: 两轴并列且计数不合并

- **WHEN** 打开契约视图，检查返回 3 条差异（2 不符 + 1 缺失）且报告含 38 条 `docMissing`
- **THEN** 上区呈「差异 2 条不符 / 1 条缺失 / 0 条孤儿」、下区呈「判据 38 条」并按 `category` 分 chip 计数（`doc` 一档占满 38 条）；界面不出现跨轴汇总

#### Scenario: chip 过滤与聚焦

- **WHEN** 点下 `doc` chip 并点击其中一条 `docMissing`
- **THEN** 列表只剩该 category 条目（计数不变、只筛显示）；条目点击触发一次 `onFocus(element)`

#### Scenario: 三值离场零前端残留

- **WHEN** 审查 `contract-panel.tsx` 与该视图的测试夹具
- **THEN** `apiReexportDrift` / `apiDocMissing` / `docNotFound` 三串零命中；chip 集合由 `apiFindings` 的 `category` 派生；删除某 `kind` 不改前端常量（只改后端打标处）

#### Scenario: 冷启动态不渲空面板

- **WHEN** `archi_api_check` 返回 `enabled = false` 且 `missing` 含 `INDEX.toml`
- **THEN** 面板呈「接口面未启用」+ 冷启动说明，并在差异区照实列出 `INDEX.toml` 一条；MUST NOT 呈现为「无差异」空态

#### Scenario: 检查失败不阻断画布

- **WHEN** `archi_api_check` 以 `Err` 返回（模型结构错 / 接口文件非法）
- **THEN** 面板呈行内错误并可重试；画布与元素信息面板照常渲染，MUST NOT 进入停摆态

### Requirement: 元素信息面板契约段

元素信息面板（`ModelInfo`）SHALL 在「概览」与「入口文档」之间呈现本元素的判据事实，数据来源零新增：

- **本元素判据**：SHALL 从父级同一份 `apiFindings` 按 `element === fqn` 过滤后逐条列出（`kind` + `message`），零命中时一句「本元素无漂移」（空态 testid `archi-contract-drawer-empty`，不按 `api.status` 分叉）；MUST NOT 为面板单独 invoke 检查或报告。`docMissing` 按同一规则呈现，MUST NOT 为它另写分支。
- **三路径面归右栏「文档」分段（换语义）**：`ArchiApiRef` 的三条路径 SHALL 由右栏 `CardContent` 的「文档」分段承接——按**各自非空**逐条出文件页签并挂载 `FileViewerBlock`（首条页签缺省激活、正文经通用 `read_file` 自取）。三条路径现在是**各自独立**的事实（`contractPath` = 文档、`schemaPath` = 接口、`generatedPath` = 生成物），故「只有接口在盘」或「只有文档在盘」时 MUST NOT 出现「三条全空」；元素信息面板 MUST NOT 再渲路径行、MUST NOT 内联路径串。
- **文档页签＝资产①**：`declared` 元素的第一条非空路径（`contractPath`）承担「入口文档」的呈现职责——面板 MUST NOT 为**同一份派生 `<FQN>.md`** 另渲第二条路径行；`api = none` 元素的入口文档走面板的「入口文档」段（`ArchiModuleDetail.doc`），`declared` 元素若写了 `doc` 也照原样投影（该字段零判据，本变更不为它新增呈现规则）。
- **接口页签＝资产②**：`schemaPath` 的页签呈现的是**手写真相**（不是生成物），故本段 MUST NOT 给它挂「生成物 / 勿手改」一类标记；生成物页签（`generatedPath`）沿用既有呈现。
- **零新取数**：本段数据 SHALL 全部来自父级已取的报告（`apiFindings`）——MUST NOT 新增 IPC、MUST NOT 为契约段发起额外 `archi_module`。
- **不与入口文档段混装**：契约正文 MUST NOT 进面板正文通路；本段只呈判据。

#### Scenario: 有接口元素的页签集合

- **WHEN** 聚焦接口与文档都在案的元素（如 `Backend.Infra.ArchiRuntime`）
- **THEN** 右栏「文档」分段按 `contractPath` / `schemaPath` / `generatedPath` 的非空项出三条页签，首条激活即经 `read_file` 取正文；元素信息面板契约段只列本元素判据（0 条时呈「本元素无漂移」）

#### Scenario: 只有接口在盘

- **WHEN** 聚焦「schema 在盘、文档缺失」的元素
- **THEN** 「文档」分段只出接口与生成物两条页签（文档页签不渲、不显示失效路径）；契约段可能同时呈一条 `docMissing`（本元素的文档迁移待办）

#### Scenario: 零判据元素的空态

- **WHEN** 聚焦 `api = none` 或无任何资产的元素
- **THEN** 契约段呈一行「本元素无漂移」；不渲任何路径行；不报错

#### Scenario: declared 元素零第二条入口文档

- **WHEN** 聚焦任一 `api = declared` 元素并审查指向入口文档的呈现位
- **THEN** 恰一条（右栏「文档」分段的文档页签）；「入口文档」段不渲染（`doc` 为 `null`）；两条同时出现即判违例

#### Scenario: 元素信息面板零新 invoke

- **WHEN** 打开任意元素信息面板并审查 invoke 次数
- **THEN** `archi_module` 恰两次（面板与右栏文档分段各一次，同一 hook / 同一载荷），`archi_api_check` / `archi_drift` 均零新增调用

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 命令面（**零改动**） | `archi_api_check` / `archi_api_generate` 的签名、薄包装、`*_inner` 测试缝、`Err` 面与 `display` 单点逐字不变；`enabled` 与三列全部来自 `core/archi` |
| `crates/core/archi/src/api/mod.rs::enabled` | 启用判据（换锚） | = 任一 `path` 非空 ∧ `api = declared` 元素 `<FQN>.schema.json` 在盘；命令层 MUST NOT 另判一套、MUST NOT 缓存 |
| `crates/core/archi/src/api/diff.rs::api_dir_artifacts` | 收面（少一类） | 只枚举 `<archi_graph_dir>/INDEX.toml`；`*.schema.json` 退出孤儿扫描 ⇒ 面板三列永不含手写接口文件 |
| `crates/core/archi/src/api/render.rs::workspace_render` | 应生成集（少一类） | 只产出 `INDEX.toml` / `.i.rs` / `.i.ts`；命令与 CLI 共用（零第二份渲染） |
| `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` + `hooks/use-archi-api-check.ts` | 呈现面（零功能改动） | 两轴呈现 / 五态 / chip 聚合 / 聚焦与打开通路逐字不变；`category` chip 由数据聚合（成员收缩自动生效）；「契约面未启用」文案改为「接口面未启用」 |
| `packages/desktop/src/views/architecture/components/model-info.tsx` | 元素态面板（零功能改动） | 契约段与右栏「文档」分段承接三路径；三路径各自非空出页签（换语义不改结构）；「入口文档」段与改档入口逐字不变 |
| `packages/desktop/src/views/architecture/components/dashboard/{architecture-node.tsx,architecture-panel.tsx,architecture-dashboard.tsx}` | 右栏与取数传递（零功能改动） | `CardContent` 三段 Tabs 与 `report.apiFindings` 传递逐字不变；文档段只渲非空项（因此三路径独立后自然正确） |

### API: 命令出参（形状零变化，取值面换锚）

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiApiCheck.enabled` | 语义换锚 | 由「载体带 fence」改为「schema 在盘」；字段名 / 类型零变化 |
| `ArchiApiCheck.{mismatched,orphaned,missing}` | 内容少一类 | 只含三类生成物；手写 `.schema.json` 零出现 |
| `ArchiApiWriteOutcome` | 零变化 | 只增只改不删的写盘语义不变（写盘单点未动） |

### Component: 文案与测试

| 项 | 变化 | 描述 |
|---|---|---|
| 面板冷启动文案 | 改 | 「契约面未启用」→「接口面未启用」（与 `enabled` 的新语义一致） |
| `contract-panel.test.tsx` / `model-info.test.tsx` / `architecture-node.test.tsx` / `use-archi-module.test.ts` | 夹具随动 | `category` 夹具按新成员集收缩；`enabled=false` 用例的 `missing` 仍为 `INDEX.toml` 一条；三路径夹具补「只有一条非空」形态 |
