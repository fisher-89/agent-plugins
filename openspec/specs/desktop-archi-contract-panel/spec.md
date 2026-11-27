# desktop-archi-contract-panel Specification

## Purpose

定义契约面在桌面端的**可操作落点**：左栏第三视图「契约」的两轴并列呈现（`category` 七值含 `doc`）、`archi_api_check` / `archi_api_generate` 两条 IPC 的命令语义与出线 DTO、`enabled` 判据（`api = declared` 元素的 schema 在盘才启用）、判定单源与写盘单点、生成交互（二次确认 / 孤儿如实 / 非原子如实）、按 `mode` 的生成权限、面板取数与成本纪律、元素信息面板「契约」段。契约格式、十判据语义、生成物落点与零 diff 门挂载分别归 `desktop-archi-api` / `desktop-archi-api-check`；漂移 `status` 推导归 `desktop-archi-drift`；画布渲染与元素信息面板其余段落归 `desktop-archi-canvas`；左栏另两视图归 `desktop-archi-file-tree`。

## Requirements

### Requirement: 检查命令契约（archi_api_check）

后端 SHALL 提供一条**零写入**的检查命令 `archi_api_check(root: String) -> Result<ArchiApiCheck, String>`，出参 SHALL 为「生成物新鲜度」一条轴的完整事实：

| 字段 | 类型 | 语义 |
|---|---|---|
| `enabled` | `bool` | 接口面是否启用（**存在任一 `path` 非空 ∧ `api = declared` 元素的 `<叶段>.schema.json` 在盘**），`api::enabled` 单点判定，MUST NOT 在命令层另判一套 |
| `meta` | `ArchiApiMeta` | 生成载元：`toolVersion`（生成器 crate 版本）+ `commonDigest`（映射规则资产摘要 `sha256:<hex>`，渲染侧截前 12 位）；取自 `api::meta()`，MUST NOT 在命令层重算摘要 |
| `mismatched` | `Vec<String>` | 内容不符：文件在盘但字节与渲染结果不同 |
| `orphaned` | `Vec<String>` | 应删未删：盘上存在、命名符合生成物判据、不在应生成集 |
| `missing` | `Vec<String>` | 应生成未生成 |

- **应生成集只剩三类**：`INDEX.toml` / `.i.rs` / `.i.ts`。`<叶段>.schema.json` MUST NOT 进应生成集（它是手写真相），因此 MUST NOT 出现在 `mismatched` / `orphaned` / `missing` 任一列——收面漏改的症状是「11 份手写文件全进 `orphaned`」（全红而不是少一条）。
- **薄包装形态**：命令 SHALL 为薄包装（`check_root` → 调 `core/archi` 领域函数 → 映射错误），领域装配在 `archi_api_check_inner(root: &Path)` 内以便离 Tauri 测试（沿 `commands/archi/` 既有 `*_inner` 形态）；命令层 MUST NOT 承载渲染、比对或路径归一。`enabled` 的计算需要模型元素表（`elements_of` 单点取数，与判据面同源），命令层 MUST NOT 自行读模型。
- **零写盘零 spawn**：命令 MUST NOT 创建 / 覆盖 / 删除任何文件，MUST NOT 派生进程，MUST NOT 引入 watch 或轮询。
- **三类差异即生成预览**：三列合起来 SHALL 恰好是「点生成会写什么」的完整清单（内容不符的会被覆盖、缺失的会被创建、孤儿不会被删）——面板据此先看清单再点生成。
- **`enabled = false` 仍照跑**：接口面未启用时命令 SHALL 返回 `Ok`（而非 `Err` 或空壳），三类差异照实计算——此时应生成集只含 `INDEX.toml`，故 `missing` 通常就是那一条（其落点为 `<archi_graph_dir>/INDEX.toml`）；MUST NOT 因 `enabled = false` 返回空的结构化结果让面板无从呈现冷启动态。
- **`enabled` 判据（本变更换锚）**：判据 SHALL 为「存在任一 `path` 非空 ∧ `api = declared` 元素在盘 `<叶段>.schema.json`」。三条后果必须说准：
  1. **冷启动态**是「没有任何元素的接口文件在盘」——`graph/model/` 一级的六个 `NN-<域>.toml`、`models/notes.md`（非元素载体）、`models/Backend/notes.schema.json`（反查不中任何元素）三者都 MUST NOT 构成启用证据；
  2. 本仓读数**仍为 `true`**（11 份 schema 在盘），MUST NOT 被改写成「模型在即启用」；
  3. 判据与判据面同源，命令层 MUST NOT 另判一套、MUST NOT 缓存。`api = none` 元素在盘的 schema 是 `apiUnexpected` 违约，MUST NOT 构成启用证据。
- **路径串口径**：三列元素 SHALL 为 repo 相对 POSIX 串（剥 `root` 前缀、`\` → `/`、去前导 `./`），经 `archi::display` 单点产出；MUST NOT 在命令层或前端另写一份归一。
- **错误面**：空白 / 不可读 / 非目录 root、模型结构诊断非空、接口文件非法（任意一份 schema 装载校验失败）SHALL 以 `Result<T, String>` 的 `Err` 返回且零部分结果（沿既有命令面口径；`ApiError::message()` 透传原文，MUST NOT 包装、MUST NOT 补默认值）。
- **出线形状**：`ArchiApiCheck` / `ArchiApiMeta` SHALL 带 `specta::Type` 与 `serde(rename_all = "camelCase")`，字段序即声明序，经 `bindings:export` 生成到 `bindings.ts` / `bindings.zod.ts`；MUST NOT 手写前端类型镜像。**本变更这两条命令的形状零改动**（`enabled` 的语义收紧不改字段名 / 类型），但绑定文件不是零 diff——`dto.rs` 的 `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 离场与 `doc` 三处进场（`ArchiModuleDetail.doc` / `ArchiSearchResult.doc` / `ArchiElement.doc`）经 `bindings:export` 反映到生成物；`ArchiApiCheck` / `ArchiApiMeta` 的字段名 / 字段序 / 联合成员零变化、`bindings:export` 幂等。
- **`meta` 只带两项**：`ArchiApiMeta` SHALL 只含 `toolVersion` / `commonDigest`——`ApiMeta` 的 `schemaUri` / `generatedNotice` / `common` 规则表 MUST NOT 出线（前端不消费，出线等于把内部载元变成协议面）。

#### Scenario: 零写盘且三类差异齐备

- **WHEN** 在本仓根分别于调用前后对工作树取字节快照并调用 `archi_api_check`
- **THEN** 两次快照完全相同；出参三列按「内容不符 / 应删未删 / 应生成未生成」分别列出条目（零差异时为三个空数组），`enabled = true`，`meta.toolVersion` 与生成器 crate 版本一致

#### Scenario: 冷启动态照实报告

- **WHEN** 观察根的 `graph/model/` 含六个 `NN-<域>.toml`、`models/` 含一份 `notes.md` 与一份反查不中任何元素的 `*.schema.json`，且没有任何 `declared` 元素的 schema 在盘
- **THEN** `enabled = false` 且命令返回 `Ok`；`missing` 含 `<archi_graph_dir>/INDEX.toml` 一条（应生成集仅含索引），`mismatched` / `orphaned` 依盘上实况列出；MUST NOT 返回 `Err`、MUST NOT 返回全空的伪造结果、MUST NOT 因「模型文件在场」或「有杂项文件」把 `enabled` 报成 `true`

#### Scenario: 手写接口文件不进三列

- **WHEN** 手改一份 `<叶段>.schema.json`（合法改动）后调用 `archi_api_check`
- **THEN** 三列中该文件零出现（既不 `mismatched` 也不 `orphaned`）；`.i.rs` / `.i.ts` / `INDEX.toml` 的随动差异（若有）照实列出

#### Scenario: 接口文件在盘才启用

- **WHEN** 给某 `api = declared` 元素补一份合法 `<叶段>.schema.json` 后重跑
- **THEN** `enabled` 由 `false` 翻为 `true`，三类差异按新应生成集重算；判定函数与判据面相同，命令层零第二份判断

#### Scenario: 结构错与非法接口一律 Err

- **WHEN** 模型含任一结构诊断后调用该命令，或把某份 schema 改成不可装载
- **THEN** 命令返回 `Err`（文案点名成因），三列不出线；磁盘字节零变化

#### Scenario: 三条路径串同源

- **WHEN** 审查出参三列的串形态与 `archi::display` 的输出
- **THEN** 逐条相等（反斜杠归一为 `/`、无前导 `./`、无绝对路径）；`orphaned` / `mismatched` 里出现的契约生成物串形如 `.dev-team/architecture/models/<分层目录…>/<叶段>.schema.json`、索引串形如 `.dev-team/architecture/graph/INDEX.toml`；源码内零第二份「strip 前缀」实现

### Requirement: 生成命令契约（archi_api_generate）

后端 SHALL 提供一条**写盘**命令 `archi_api_generate(root: String) -> Result<ArchiApiWriteOutcome, String>`：

| 字段 | 类型 | 语义 |
|---|---|---|
| `written` | `Vec<String>` | 本次实际写入的生成物（内容变化或新创建） |
| `unchanged` | `Vec<String>` | 应生成且盘上字节已一致（本次零写入） |

- **先比对后写**：命令 SHALL 对每份应生成物先读盘比对，字节相同即计入 `unchanged` 并**不重写**（不触碰 mtime，避免无意义的工作树抖动）；不同或缺文件才 `create_dir_all` + 写入。
- **从不删文件**：命令 MUST NOT 删除任何文件——孤儿生成物只由检查命令报告、由人决定如何处置。写盘语义因此是「只增只改」，MUST NOT 在任何路径上退化出删除分支。
- **越根复核**：每份目标路径 SHALL 在写入前以 `starts_with(root)` 复核确实落在观察根之下（在 `archi::api::paths` 的形态校验之外的第二层），越根即 `Err` 且**该份不写**。
- **非原子，如实呈现**：写盘为逐份 `fs::write`，中途失败 SHALL 返回 `Err` 且**不回滚已写部分**；错误文案 MUST NOT 声称「全或无」，面板侧文案亦同。
- **结构错即零写盘**：模型含任一结构诊断 ⇒ 应生成集不可得 ⇒ 命令返回 `Err` 且零写盘（坏模型不得被固化成入库生成物，与 agent 生成工具同口径）。
- **薄包装形态**：命令 SHALL 为薄包装（`check_root` → `archi::api::workspace_render` → 写盘单点 → 映射错误），装配在 `archi_api_generate_inner(root: &Path)` 内；命令层 MUST NOT 自行拼生成物内容或路径。
- **写命令身份**：本命令是 archi 段**第二条**写命令（第一条仍是只改 `gate` 单字段的 `archi_set_gate`）；两条之外的六条命令 MUST NOT 写盘。

#### Scenario: 首写后检查清空差异

- **WHEN** 在一个三类差异齐备的观察根调用 `archi_api_generate`，随后再调一次 `archi_api_check`
- **THEN** 首次 `written` 非空、`unchanged` 为另一集合（两者不交叠）；随后检查的 `mismatched` 与 `missing` 均为空；若盘上原有孤儿，`orphaned` **仍在**（生成不删文件）且该文件仍在盘

#### Scenario: 幂等且不动 mtime

- **WHEN** 连续两次调用 `archi_api_generate`（中间无任何改动）
- **THEN** 第二次 `written` 为空、`unchanged` 为全部应生成物；各文件内容与 mtime 均与第一次之后相同

#### Scenario: 越根目标被拒且零写盘

- **WHEN** 构造一份目标路径落在观察根之外的应生成物并触发写盘
- **THEN** 命令返回 `Err`（文案含「越出观察根」类表述），该份未被创建，观察根外零文件产生

#### Scenario: 中途失败不回滚

- **WHEN** 让第 k 份的目标父目录不可写（只读目录）后调用
- **THEN** 命令返回 `Err`；第 1..k−1 份已落盘（不回滚、不清理）、第 k 份起未写；错误文案与面板文案均不声称「已全部写入」或「已全部回滚」

#### Scenario: 结构错零写盘

- **WHEN** 模型含结构诊断后调用生成命令
- **THEN** 返回 `Err` 且盘上零变化（既有生成物字节与 mtime 均不变）

### Requirement: 判定单源与写盘单点

本能力的判定与写盘 SHALL 各有唯一落点，各驱动面（IPC / agent 工具）SHALL 消费同一实现：

- **判定与比对在 `core/archi`**：四类生成物的渲染（`workspace_render`）与三类差异的比对（`compare`）SHALL 在 `crates/core/archi/src/api/**` 单点实现，纯读盘、零写盘、零 `std::process`；命令面（`archi_api_check` / `archi_api_generate`）与 agent 工具（`archi_api` / `archi_generate`）MUST NOT 各写一份渲染或比对。
- **写盘单点在 `infra/archi`**：生成物的目录创建与文件写入 SHALL 仅在 `crates/infra/archi` 的单一函数（`write_artifacts`）内出现，与既有报告落盘（`report.rs`）并列为该 crate 的第二处写盘单点；壳层命令 MUST NOT 出现 `fs::write` / `create_dir_all` / `File::create`。
- **`core/archi` 零写盘纪律不动**：写盘上移到 `infra/archi` MUST NOT 反向稀释 core 的零写盘纪律（core 仍零 `fs::write` / 零 `create_dir*`）。
- **`passed` 面零改动**：本能力的两条命令 MUST NOT 改变门禁布尔——门禁 `passed` 仍只看 `errors` 与 `model.structuralErrors`（见 `desktop-archi-gate` / `desktop-archi-drift`）。

#### Scenario: 两驱动面逐字段同源

- **WHEN** 在同一观察根分别调用 IPC `archi_api_check` 与 agent 工具 `archi_api(check)`（差异场景）
- **THEN** 两者三类差异的文件集合逐条相等、顺序按同一稳定序

#### Scenario: 写盘实现单点

- **WHEN** 审查契约生成面（`crates/core/archi/src/api/**` / `crates/infra/archi/src/**` / `src-tauri/src/commands/archi/**`）的文件系统调用
- **THEN** `fs::write` 与 `create_dir_all` 仅在 `crates/infra/archi/src/api_artifacts.rs` 出现（各恰一处）；`core/archi` 侧零写盘调用；命令层零写盘调用

### Requirement: 两轴并列呈现与状态矩阵

左栏第三视图 SHALL 把两个正交事实并列呈现，MUST NOT 混成一个「问题总数」：

| 轴 | 事实源 | 谁修 | 呈现 |
|---|---|---|---|
| **生成物新鲜度** | `archi_api_check`（新命令） | 机器（点生成） | 上区：三类差异分组 + 各自计数 |
| **契约 vs 代码** | 漂移报告的 `apiFindings`（既有取数，父级已取） | 人或 agent | 下区：按 `category` **七值** chip 筛选 + 各自计数 |

- **两轴分区独立计数**：两区各自呈现计数，MUST NOT 出现跨轴求和的总数（两轴的可修复者不同，跨轴求和会让用户以为都得手改）。
- **`category` 词汇零新增（前端面）**：chip 取值 SHALL 取自报告既有开放值集——**`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor` / `doc`**（仍七组；本变更后 `doc` 组只剩 `docMissing` 一个成员），chip 由 `apiFindings` 数据聚合、**空组自然不渲**，MUST NOT 在前端另立词表或类别常量。
- **词表收缩零前端硬编码**：`apiReexportDrift` / `apiDocMissing` / `docNotFound` 三值离场后，前端 MUST NOT 残留任何按 `kind` 的分支或常量（已核实 `contract-panel.tsx` 零前端词表）。
- **条目点击 = 聚焦**：点击判据条目 SHALL 调既有 `onFocus(element)`（与元素树 / 文件树同一语义：画布切到该元素 1-hop、元素信息面板取该元素详情），MUST NOT 新增交互范式；`element` 为空的条目 SHALL 只呈文本、不聚焦。
- **「打开」走通用读面**：契约 md / schema / 生成物 / 入口文档的路径行 SHALL 经既有 `FileViewerDrawer` + `read_file` 打开（`read_file` 已是通用绝对路径读面），MUST NOT 为面板新增专用读命令。
- **五态全画**：

| 态 | 呈现 |
|---|---|
| `loading` | 「解析中…」（沿既有 loading 文案） |
| `enabled = false` | 「接口面未启用」+ 冷启动说明（应生成集此时只含 `INDEX.toml`），且仍呈现 `missing` 实况——MUST NOT 渲成空面板 |
| 零差异 + 零判据 | 「生成物最新，判据 0 条」 |
| 检查 invoke reject | 面板内行内错误（沿 `archi-file-tree-error` 形态），画布与元素信息面板照常可用 |
| 生成失败 | 错误回显在面板顶部，文案如实（非原子、失败点之前已写） |

- **不阻断画布**：检查或生成失败 MUST NOT 阻断图渲染、MUST NOT 使画布进入停摆态（停摆态的唯一成因仍是模型结构错）。
- **`doc` 分组的呈现语义（本变更收窄）**：`doc` chip 下只剩 `docMissing`（`warn`）——表现为「该模块还没有文档」的迁移待办，永不纳管 `status`；旧的三值分述（`docNotFound` 的笔误语义、`apiDocMissing` 的「有文档无约定」语义）MUST NOT 残留。`doc` 组 MUST NOT 被计入「生成物新鲜度」轴。

#### Scenario: 两轴并列且计数不合并

- **WHEN** 打开契约视图，检查返回 3 条差异（2 不符 + 1 缺失）且报告含 38 条 `docMissing`
- **THEN** 上区呈「差异 2 条不符 / 1 条缺失 / 0 条孤儿」、下区呈「判据 38 条」并按 `category` 分 chip 计数（其中 `doc` 一档可能占满 38 条）；界面上不出现跨轴汇总

#### Scenario: chip 过滤与聚焦

- **WHEN** 点下 `doc` chip 并点击其中一条 `docMissing` 条目
- **THEN** 列表只剩该 category 的条目（计数不变、只筛显示）；条目点击触发一次 `onFocus(element)`——画布切到该元素 1-hop、元素信息面板取该元素详情（`archi_module` 恰一次）

#### Scenario: 三值离场零前端残留

- **WHEN** 审查 `contract-panel.tsx` 与该视图的测试夹具
- **THEN** `apiReexportDrift` / `apiDocMissing` / `docNotFound` 三串零命中；chip 集合由 `apiFindings` 的 `category` 派生；删除某 `kind` 不改前端常量（只改后端打标处）

#### Scenario: 冷启动态不渲空面板

- **WHEN** `archi_api_check` 返回 `enabled = false` 且 `missing` 含 `INDEX.toml`
- **THEN** 面板呈「接口面未启用」+ 冷启动说明，并在差异区照实列出 `INDEX.toml` 一条；MUST NOT 呈现为「无差异」空态

#### Scenario: 检查失败不阻断画布

- **WHEN** `archi_api_check` 以 `Err` 返回（模型结构错 / 接口文件非法）
- **THEN** 面板呈行内错误并可重试；画布与元素信息面板照常渲染（图零降级），MUST NOT 进入停摆态

### Requirement: 生成交互与模式权限

生成是**写工作树**的动作，交互 SHALL 与既有「写要开 change」纪律同构：

- **二次确认**：点击生成 SHALL 弹二次确认（`StandardDialog` 既有原语），文案 SHALL 含三件事——① 「将写入 N 份（不符 X + 缺失 Y）」；② 「保留 M 份孤儿生成物不做处理」（M 为当前 `orphaned` 长度）；③ 写盘非原子的如实表述。MUST NOT 以轻量 toast 代替确认。
- **成功后重跑检查**：生成成功后 SHALL 触发一次检查重取（用取数 hook 自身的刷新，不重挂整页），并 toast 报 `written` / `unchanged` 两份计数；MUST NOT 让面板停留在陈旧差异上。
- **模式权限（U1，文案本变更改写）**：生成入口 SHALL 仅出现在 `mode = incremental`（change 详情「架构」页签）；`mode = full`（`/architecture`）MUST NOT 出现可提交的生成入口，改为一行提示「**生成需开 change，或让左侧会话生成**」，形态沿既有 `archi-gate-readonly` 提示。该文案 MUST NOT 再出现「本仓的生成命令」一类死指代——通用工作区里没有那条命令，页内会话的生成能力由 `archi_generate` 工具承载（见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」的 archi 来源臂）。
- **只读对象的延伸**：画布「只读约束的对象 = 画布的人类可提交入口」这一既有裁定同样覆盖本面板的生成按钮——MUST NOT 以「会话能写」为理由为 `full` 模式新增生成入口，亦 MUST NOT 以「画布只读」为理由禁止页内会话生成。**「页内会话生成」是既有条款的兑现，不是新入口**：它走 `archi_generate`（写主仓、事后 `git diff` 评审），与面板按钮是两条互不代理的路径。
- **生成期间零并发**：生成进行中 SHALL 禁用生成与检查按钮（防并发写同一批文件）；MUST NOT 在生成未完成时重跑检查。

#### Scenario: 二次确认与重跑检查

- **WHEN** 在 `incremental` 模式下点生成并确认
- **THEN** dialog 文案含「写入 N 份」「保留 M 份孤儿不做处理」与非原子表述；确认后 `archi_api_generate` 恰调用一次，toast 报 written / unchanged 计数，随后 `archi_api_check` 恰调用一次；面板差异按新结果刷新

#### Scenario: 取消零写盘

- **WHEN** 点生成后在 dialog 选择取消
- **THEN** `archi_api_generate` 零调用、磁盘字节零变化；面板维持原差异

#### Scenario: full 模式零生成入口且文案无死指代

- **WHEN** 在 `/architecture`（`mode = full`）审查契约视图与源码
- **THEN** 无任何可提交的生成按钮，呈一行「生成需开 change，或让左侧会话生成」提示；源码内 `full` 分支零 `archiApiGenerate` 调用路径；提示文案对「本仓的生成命令」零命中（含测试断言串）

#### Scenario: 生成中禁并发

- **WHEN** 一次生成尚未返回时再次点击生成或检查
- **THEN** 两个按钮均为禁用态、无第二次 invoke；生成返回后按钮恢复

#### Scenario: 页内会话生成不代理面板按钮

- **WHEN** 在 `full` 模式让左侧会话调用 `archi_generate` 写入主仓生成物后回到契约视图
- **THEN** 面板仍无生成按钮（模式权限不变），用户点刷新后三类差异按新结果呈现；两条路径互不调用、互不隐藏

### Requirement: 面板取数与成本纪律

契约视图取数 SHALL 显式、按需、零轮询：

- **三触发**：`archi_api_check` SHALL 只在三处发起——契约视图首次挂载、`root` 变化、用户点「检查」或生成成功后的显式刷新；MUST NOT 引入目录 watch、定时轮询或事件订阅（沿 `desktop-archi-canvas`「三触发取数与零监听」同一红线）。
- **模式未选零 invoke**：左栏停留在文件 / 元素视图时 MUST NOT 调用 `archi_api_check`（MUST NOT 随画布 `refresh` 自动跑全量渲染——`workspace_render` 是「模型元素 × 契约 × 声明抽取」的全量成本）。
- **`root` 为空零 invoke**：`root === null` 时 MUST NOT 发起任何 invoke（沿既有约定）。
- **判据轴零新取数**：`category` chip 与判据列表 SHALL 复用父级画布已取的漂移报告 `apiFindings`，MUST NOT 为判据轴新增 IPC 或重复 invoke 同一报告。
- **失败清旧数据**：取数失败 SHALL 清空上一次的三类差异（旧结果 MUST NOT 冒充本次结论），与既有 `use-archi-drift` 的同款纪律一致。
- **新 hook 必须被真实消费**：取数 hook SHALL 由面板组件真实消费，MUST NOT 只被测试引用（`knip` 死码扫描会把仅测试引用的导出报为未使用）。

#### Scenario: 切到契约视图才取数

- **WHEN** 打开 `/architecture`（缺省文件视图）后观察到画布已渲染，再切到契约视图
- **THEN** 切换前 `archi_api_check` 零调用；切到契约视图后恰一次调用；点「检查」再恰一次；期间 `archi_drift` / `archi_graph` / `archi_module` 调用次数不因切换增加

#### Scenario: 画布刷新不连带全量渲染

- **WHEN** 在文件视图点页面「刷新」触发画布重挂
- **THEN** `archi_api_check` 零调用（全量渲染不随刷新自动跑）；`archi_graph` / `archi_drift` 按既有三触发各重取一次

#### Scenario: 无 root 零 invoke

- **WHEN** `root = null` 时切到契约视图
- **THEN** 三次 invoke 均为零次，面板呈空态提示；不抛错、不崩溃

### Requirement: 元素信息面板契约段

元素信息面板（`ModelInfo`）SHALL 在「概览」与「入口文档」之间呈现本元素的契约判据事实，数据来源零新增：

- **本元素判据**：SHALL 从父级同一份 `apiFindings` 按 `element === fqn` 过滤后逐条列出（`kind` + `message`），零命中时一句「本元素无漂移」（空态 testid `archi-contract-drawer-empty`，不按 `api.status` 分叉）；MUST NOT 为面板单独 invoke 检查或报告。`docMissing` 按同一规则呈现，MUST NOT 为它另写分支。
- **三路径面归右栏「文档」分段（换语义）**：`ArchiApiRef` 的三条路径 SHALL 由右栏 `CardContent` 的「文档」分段承接——按**各自非空**逐条出文件页签并挂载 `FileViewerBlock`（首条页签缺省激活、正文经通用 `read_file` 自取）。三条路径现在是**各自独立**的事实（`contractPath` = 文档、`schemaPath` = 接口、`generatedPath` = 生成物），故「只有接口在盘」或「只有文档在盘」时 MUST NOT 出现「三条全空」；元素信息面板 MUST NOT 再渲路径行、MUST NOT 内联路径串。
- **文档页签＝资产①**：`declared` 元素的第一条非空路径（`contractPath`）承担「入口文档」的呈现职责——面板 MUST NOT 为**同一份派生 `<叶段>.md`** 另渲第二条路径行；`api = none` 元素的入口文档走面板的「入口文档」段（`ArchiModuleDetail.doc`，见 `desktop-archi-canvas`「元素信息面板内容与本期边界」），`declared` 元素若写了 `doc` 也照原样投影（该字段零判据，本变更不为它新增呈现规则）。
- **接口页签＝资产②**：`schemaPath` 的页签呈现的是**手写真相**（不是生成物），故本段 MUST NOT 给它挂「生成物 / 勿手改」一类标记；生成物页签（`generatedPath`）沿用既有呈现。
- **零新取数**：本段数据 SHALL 全部来自父级已取的报告（`apiFindings`）——MUST NOT 新增 IPC、MUST NOT 为契约段发起额外 `archi_module`（元素信息面板与右栏文档分段各自复用既有 `useArchiModule` 的同一次取数）。
- **不与入口文档段混装**：契约正文 MUST NOT 进面板正文通路（正文仍由通用 `read_file` 按路径承担，原「`docs` 关联集」一说不存在）；本段只呈判据，MUST NOT 内联契约 md 内容。

#### Scenario: 有接口元素的页签集合

- **WHEN** 聚焦接口与文档都在案的元素（如 `Backend.Infra.ArchiRuntime`）
- **THEN** 右栏「文档」分段按 `contractPath` / `schemaPath` / `generatedPath` 的非空项出三条页签，首条页签激活即经 `read_file` 取正文；元素信息面板契约段只列本元素判据，判据 0 条时呈「本元素无漂移」

#### Scenario: 只有接口在盘

- **WHEN** 聚焦「schema 在盘、文档缺失」的元素
- **THEN** 「文档」分段只出接口与生成物两条页签（文档页签不渲、不显示失效路径）；契约段可能同时呈一条 `docMissing`（本元素的文档迁移待办）

#### Scenario: 零判据元素的空态

- **WHEN** 聚焦 `api = none` 或无任何资产的元素
- **THEN** 契约段呈一行「本元素无漂移」说明（零判据），不渲任何路径行；不报错、不显示失效路径

#### Scenario: declared 元素零第二条入口文档

- **WHEN** 聚焦任一 `api = declared` 元素并审查指向入口文档的呈现位
- **THEN** 恰一条（右栏「文档」分段的文档页签）；「入口文档」段不渲染（`doc` 为 `null`）；两条同时出现即判违例

#### Scenario: 元素信息面板零新 invoke

- **WHEN** 打开任意元素信息面板并审查 invoke 次数
- **THEN** `archi_module` 恰两次（元素信息面板与右栏文档分段各一次，同一 hook / 同一载荷），`archi_api_check` / `archi_drift` 均零新增调用

## Module Contract

### API: `packages/desktop/src-tauri/src/commands/archi/mod.rs`（命令面）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api_check` | 新增 | `(root: String) -> Result<ArchiApiCheck, String>`；`check_root` → `archi_api_check_inner` → 错误映射；零写盘、零 spawn、零领域规则 |
| `archi_api_check_inner` | 新增 | `(root: &Path) -> Result<ArchiApiCheck, String>`：调 `archi::api::artifact_check`（内部 = `workspace_render` + `compare` + `meta` 投影）；离 Tauri 可测 |
| `archi_api_generate` | 新增 | `(root: String) -> Result<ArchiApiWriteOutcome, String>`；第二条写命令；只增只改不删 |
| `archi_api_generate_inner` | 新增 | `(root: &Path) -> Result<ArchiApiWriteOutcome, String>`：`api::workspace_render` → `archi_runtime::write_artifacts` |
| 既有六命令与六个 `*_inner` | 零改动 | 签名 / 语义逐字不变；文件头注释「六命令」→「八命令」、「唯一写命令」→「写命令两条」 |

### API: `packages/desktop/src-tauri/crates/core/archi/src/api/{render,diff}.rs`（判定面）

| 项 | 变化 | 描述 |
|---|---|---|
| `api::render::workspace_render` | 新增 | `(root) -> Result<Vec<GeneratedFile>, ApiError>`：`dto::graph_view`（结构诊断 → `Err`）→ `load_contracts` → `render_all`；纯读盘 |
| `api::diff::compare` | 新增 | `(root) -> Result<ArtifactDiff, ApiError>`：三类差异 + `generated_files_on_disk`（`ignore` 感知遍历）；repo 相对串经 `archi::display` |
| `api::artifact_check` | 新增 | `(root) -> Result<ArchiApiCheck, ApiError>`：`compare` + `meta()` 的 DTO 投影单点（命令层零投影） |
| `api::gating_findings` | 新增 | `(root, &[ArchiElement], &[ArchiApiFinding]) -> Vec<ArchiApiFinding>`：纳管子集筛选单点（「契约文件在案」判据与 `api_ref` 同源） |
| `api::check` / `contract` / `schema` / `paths` / `emit` / `extract` | 判据面语义见 `desktop-archi-api-check` | 十三条判据与两支首错求值序、`doc_facts` 事实装配、`gating_findings` 纳管口径归 `desktop-archi-api-check`；契约格式、`$defs` 规则、生成物落点与命名判据逐字不变 |

### API: `packages/desktop/src-tauri/crates/infra/archi/src/api_artifacts.rs`（写盘面）

| 项 | 变化 | 描述 |
|---|---|---|
| `write_artifacts` | 新增 | `(root: &Path, &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`：越根复核 → 读盘比对 → `create_dir_all` + `fs::write`；不删文件、不回滚、路径串 repo 相对 |
| `report::write_report` | 零改动 | 报告落盘单点逐字不变（本 crate 现有两处写盘单点并列） |

### Component: `packages/desktop/src/views/architecture/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `hooks/use-archi-api-check.ts` | 新增 | `{ root }` → `{ check, loading, error, refresh }`：三触发（挂载 / `root` 变化 / `refresh`）、`root === null` 零 invoke、零轮询；失败清旧数据 |
| `components/dashboard/contract-panel.tsx` | 新增 | 左栏契约视图：两轴分区 + chip 过滤 + 条目聚焦 + 路径行 `FileViewerDrawer` + `[检查]` / 按模式的 `[生成]` + 五态；`data-testid` = `archi-contract-panel` / `archi-contract-axis-artifacts` / `archi-contract-axis-findings` / `archi-contract-chip` / `archi-contract-item` / `archi-contract-check` / `archi-contract-generate` / `archi-contract-status` / `archi-contract-error` |
| `components/architecture-panel.tsx` + `components/dashboard/architecture-dashboard.tsx` | 改 | 面板拆为薄壳（graph / drift 取数 + 停摆 / 空态早退）与 `ArchitectureDashboard`（两栏布局 + 聚焦 / 选中状态）；左栏 `ResizablePanel` 内的 base-ui `Tabs` 三视图分支在 `dashboard/tree-column.tsx`（第三臂 `value="contract"` 渲 `ContractPanel`；三视图共用同一面板，`defaultSize={300}` / `minSize={200}`）；把 `report.apiFindings` 与本元素过滤结果传给面板与元素信息面板；`mode` 决定生成按钮可用性；渲染与标记面零改动 |
| `components/model-info.tsx` | 改 | 概览与入口文档之间插「契约」段（仅本元素判据 + 「本元素无漂移」空态；三路径面归右栏「文档」分段）；零新 invoke |
| `components/dashboard/file-tree.tsx` | 改 | `TreeMode` 三值枚举已移至 `dashboard/tree-column.tsx`（含 `'contract'`）；分段控件本体在 `dashboard/tree-column.tsx` 的 `Tabs`（`TreeModeSwitch` 退役）；文件 / 元素两视图行渲染零改动 |
| 零改动面 | — | `use-archi-graph.ts` / `use-archi-drift.ts` / `use-archi-module.ts` / `use-archi-file-tree.ts`、`lib/marks.ts` / `lib/drift-marks.ts` / `lib/fqn.ts`、`structure-error-panel.tsx`、`dashboard/element-tree.tsx` |

### 模块增量：E-1 目录迁移（module-entry-docs-semantics）

命令形态、出参字段、三类差异语义、错误面与面板呈现逐字不变——本变更零前端产品代码改动，变的只是后端派生出的路径串（前端仍只消费 DTO 字符串）：

| 模块 | 变化 | 关键契约 |
|---|---|---|
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 零改动 | `archi_api_check(root) -> Result<ArchiApiCheck, String>` 薄包装、`*_inner` 测试缝、`Err` 面与 `display` 单点逐字不变；`enabled` / 三列全部来自 `core/archi` |
| `crates/core/archi/src/api/mod.rs::enabled` | 改（换根） | 契约面启用判据 = `<archi_models_dir>` 递归含 ≥1 份 `*.md`（判据形态零变化）；命令层 MUST NOT 另判一套 |
| `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` + `hooks/use-archi-api-check.ts` | 零改动 | 两轴呈现、状态矩阵、「契约面未启用」文案分支与三类差异渲染逐字不变（`enabled` 语义与字段名均未变，变的只是后端填值口径中的目录） |
| `packages/desktop/src/views/architecture/**` 的测试 | 零改动 | 面板用例的 `enabled` / 三列均为 DTO 载荷样例，不参与路径派生；测试内的契约路径串保持为 DTO 载荷样例（fixture 契约） |
| `packages/desktop/src/types/generated/` | 仅 `bindings.ts` 的 2 行 doc 注释 | 出参形状零改动；`dto.rs` 的 doc 换段经 specta 进 `bindings.ts`（`ApiStatus` / `ArchiApiRef.contractPath`）⇒ 恰 2 行注释随动，`bindings.zod.ts` 零 diff、`bindings:export` 幂等 |

### CLI: 脚本面（零改动）

| 项 | 变化 | 描述 |
|---|---|---|
| `package.json` 的 `api:export` / `api:check` / `check` | 零改动 | 三段命令形态逐字不变（写盘实现换落点、命令串不变）；`version` 相对 `HEAD` 现值 +1 patch |
| `src-tauri/Cargo.toml` | 零改动 | 无新 crate、无新 workspace 依赖项；crate 版本行不随前端 patch |

### 模块增量：契约面启用判据收紧与 `doc` 分组（module-doc-field-docs-retirement）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|------|
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 零改动 | `archi_api_check(root) -> Result<ArchiApiCheck, String>` 薄包装与 `*_inner` 测试缝逐字不变；`enabled` / 三列全部来自 `core/archi` |
| `crates/core/archi/src/api/mod.rs::enabled` | 改（判据收紧） | 启用判据 = 任一 `path` 非空元素的载体在盘且 `DocFact::Present { has_api_fence: true }`（不再等价于「递归任意 `*.md`」）；命令层 MUST NOT 另判一套、MUST NOT 缓存 |
| `crates/core/archi/src/api/artifact_check` | 改（签名透传） | 因 `enabled` 需元素表，`artifact_check` 内部经 `elements_of` 单点取模型后调用 `enabled(root, elements)`；DTO 形状与命令层零改动 |
| `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` + `hooks/use-archi-api-check.ts` | 零改动（数据驱动） | 两轴呈现、状态矩阵、「契约面未启用」文案分支与三类差异渲染逐字不变；`category` chip 集合由 `apiFindings` 聚合得出，新增 `doc` 档不要求前端改常量 |
| `packages/desktop/src/views/architecture/components/model-info.tsx` | 改（契约段） | 本元素判据过滤与「本元素无漂移」空态；三路径面归右栏「文档」分段（`FileViewerBlock`），抽屉零路径行、零第二条入口文档行 |

### 模块增量：enabled 换锚与三路径独立（archi-agent-context-review）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 命令面（**零改动**） | `archi_api_check` / `archi_api_generate` 的签名、薄包装、`*_inner` 测试缝、`Err` 面与 `display` 单点逐字不变；`enabled` 与三列全部来自 `core/archi` |
| `crates/core/archi/src/api/mod.rs::enabled` | 启用判据（换锚） | = 任一 `path` 非空 ∧ `api = declared` 元素 `<叶段>.schema.json` 在盘；命令层 MUST NOT 另判一套、MUST NOT 缓存 |
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

### 模块增量：生成提示文案换指（archi-generator-access-gap）

主基线既有行逐字沿用；本变更受影响模块如下：

#### Component: `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx`

| 项 | 变化 | 描述 |
|---|---|---|
| `full` 模式提示 | 改 | 文案由「生成需开 change（或让左侧会话执行本仓的生成命令）」改为「生成需开 change，或让左侧会话生成」；形态类名与位次不变 |
| 生成按钮 / 二次确认 / 并发禁用 / 重跑检查 | 零改动 | 逐字不变（`incremental` 分支的 `archiApiGenerate` 调用面零变化） |

#### Component: `contract-panel.test.tsx`

| 项 | 变化 | 描述 |
|---|---|---|
| 提示文案断言 | 改 | 断言串逐字改为新文案；`full` 分支零 `archiApiGenerate` 的断言逐字不变 |
