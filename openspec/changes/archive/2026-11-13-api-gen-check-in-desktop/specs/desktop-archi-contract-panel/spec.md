# desktop-archi-contract-panel Specification（delta: api-gen-check-in-desktop）

## Purpose

定义契约面在桌面端的**可操作落点**：左栏第三视图「契约」的两轴并列呈现、`archi_api_check` / `archi_api_generate` 两条 IPC 的命令语义与出线 DTO、判定单源与写盘单点、生成交互（二次确认 / 孤儿如实 / 非原子如实）、按 `mode` 的生成权限、面板取数与成本纪律、元素抽屉「契约」段。契约格式、十条判据语义、生成物落点与零 diff 门挂载分别归 `desktop-archi-api` / `desktop-archi-api-check`；漂移 `status` 推导归 `desktop-archi-drift`；画布渲染与抽屉其余段落归 `desktop-archi-canvas`；左栏另两视图归 `desktop-archi-file-tree`。

## ADDED Requirements

### Requirement: 检查命令契约（archi_api_check）

后端 SHALL 提供一条**零写入**的检查命令 `archi_api_check(root: String) -> Result<ArchiApiCheck, String>`，出参 SHALL 为「生成物新鲜度」一条轴的完整事实：

| 字段 | 类型 | 语义 |
|---|---|---|
| `enabled` | `bool` | 契约面是否启用（`<archi_api_dir>` 递归含 ≥1 份 `*.md`），`api::enabled` 单点判定，MUST NOT 在命令层另判一套 |
| `meta` | `ArchiApiMeta` | 生成载元：`toolVersion`（生成器 crate 版本）+ `commonDigest`（映射规则资产摘要 `sha256:<hex>`，渲染侧截前 12 位）；取自 `api::meta()`，MUST NOT 在命令层重算摘要 |
| `mismatched` | `Vec<String>` | 内容不符：文件在盘但字节与渲染结果不同 |
| `orphaned` | `Vec<String>` | 应删未删：盘上存在、命名符合生成物判据、不在应生成集 |
| `missing` | `Vec<String>` | 应生成未生成 |

- **薄包装形态**：命令 SHALL 为薄包装（`check_root` → 调 `core/archi` 领域函数 → 映射错误），领域装配在 `archi_api_check_inner(root: &Path)` 内以便离 Tauri 测试（沿 `commands/archi/` 既有 `*_inner` 形态）；命令层 MUST NOT 承载渲染、比对或路径归一。
- **零写盘零 spawn**：命令 MUST NOT 创建 / 覆盖 / 删除任何文件，MUST NOT 派生进程，MUST NOT 引入 watch 或轮询。
- **三类差异即生成预览**：三列合起来 SHALL 恰好是「点生成会写什么」的完整清单（内容不符的会被覆盖、缺失的会被创建、孤儿不会被删）——面板据此先看清单再点生成。
- **`enabled = false` 仍照跑**：契约面未启用时命令 SHALL 返回 `Ok`（而非 `Err` 或空壳），三类差异照实计算——此时应生成集只含 `INDEX.toml`，故 `missing` 通常就是那一条；MUST NOT 因 `enabled = false` 返回空的结构化结果让面板无从呈现冷启动态。
- **路径串口径**：三列元素 SHALL 为 repo 相对 POSIX 串（剥 `root` 前缀、`\` → `/`、去前导 `./`），经 `archi::display` 单点产出；MUST NOT 在命令层或前端另写归一份归一。
- **错误面**：空白 / 不可读 / 非目录 root、模型结构诊断非空、契约资产非法（任一契约不可解析 / 双向 gate 失败 / `$defs` 规则失败）SHALL 以 `Result<T, String>` 的 `Err` 返回且零部分结果（沿既有命令面口径；`ApiError::message()` 透传原文，MUST NOT 包装、MUST NOT 补默认值）。
- **出线形状**：`ArchiApiCheck` / `ArchiApiMeta` SHALL 带 `specta::Type` 与 `serde(rename_all = "camelCase")`，字段序即声明序，经 `bindings:export` 生成到 `bindings.ts` / `bindings.zod.ts`；MUST NOT 手写前端类型镜像。
- **`meta` 只带两项**：`ArchiApiMeta` SHALL 只含 `toolVersion` / `commonDigest`——`ApiMeta` 的 `schemaUri` / `generatedNotice` / `common` 规则表 MUST NOT 出线（前端不消费，出线等于把内部载元变成协议面）。

#### Scenario: 零写盘且三类差异齐备

- **WHEN** 在本仓根分别于调用前后对工作树取字节快照并调用 `archi_api_check`
- **THEN** 两次快照完全相同；出参三列按「内容不符 / 应删未删 / 应生成未生成」分别列出条目（零差异时为三个空数组），`enabled = true`，`meta.toolVersion` 与生成器 crate 版本一致

#### Scenario: 冷启动态照实报告

- **WHEN** 观察根的架构域存在但 `<archi_api_dir>` 缺席（或存在而零份 `*.md`）
- **THEN** `enabled = false` 且命令返回 `Ok`；`missing` 含 `INDEX.toml` 一条（应生成集仅含索引），`mismatched` / `orphaned` 依盘上实况列出；MUST NOT 返回 `Err`、MUST NOT 返回全空的伪造结果

#### Scenario: 结构错与非法契约一律 Err

- **WHEN** 模型含任一结构诊断后调用该命令，或把某份契约改成不可解析
- **THEN** 命令返回 `Err`（文案点名成因），三列不出线；磁盘字节零变化

#### Scenario: 三条路径串同源

- **WHEN** 审查出参三列的串形态与 `archi::display` 的输出
- **THEN** 逐条相等（反斜杠归一为 `/`、无前导 `./`、无绝对路径）；源码内零第二份「strip 前缀」实现

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
- **结构错即零写盘**：模型含任一结构诊断 ⇒ 应生成集不可得 ⇒ 命令返回 `Err` 且零写盘（坏模型不得被固化成入库生成物，与 CLI `generate` 同口径）。
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

本能力的判定与写盘 SHALL 各有唯一落点，两驱动面（CLI / IPC）SHALL 消费同一实现：

- **判定与比对在 `core/archi`**：四类生成物的渲染（`workspace_render`）与三类差异的比对（`compare`）SHALL 在 `crates/core/archi/src/api/**` 单点实现，纯读盘、零写盘、零 `std::process`；命令面（`archi_api_check` / `archi_api_generate`）、CLI（`archi-api-gen --check` / `generate`）与 agent 工具（`archi_api`）MUST NOT 各写一份渲染或比对。
- **写盘单点在 `infra/archi`**：生成物的目录创建与文件写入 SHALL 仅在 `crates/infra/archi` 的单一函数（`write_artifacts`）内出现，与既有报告落盘（`report.rs`）并列为该 crate 的第二处写盘单点；`crates/tools/archi-api-gen` 与壳层命令 MUST NOT 出现 `fs::write` / `create_dir_all` / `File::create`。
- **CLI 收窄为编排**：`archi-api-gen` SHALL 只剩 argv 解析、退出码与三子命令编排（`generate` → 渲染 + 写盘单点；`--check` → 比对 + 打印；`bootstrap` → 草稿文本）；其子命令面、退出码语义（无差异 0 / 有差异或失败 1 / 用法错 2）与「零子进程派生」纪律逐字不变。
- **`core/archi` 零写盘纪律不动**：写盘上移到 `infra/archi` MUST NOT 反向稀释 core 的零写盘纪律（core 仍零 `fs::write` / 零 `create_dir*`）。
- **`passed` 面零改动**：本能力的两条命令 MUST NOT 改变门禁布尔——门禁 `passed` 仍只看 `errors` 与 `model.structuralErrors`（见 `desktop-archi-gate` / `desktop-archi-drift`）。

#### Scenario: 两驱动面逐字段同源

- **WHEN** 在同一观察根分别调用 `archi_api_check` 与 `cargo run -p archi-api-gen -- --check`（差异场景）
- **THEN** 两者三类差异的文件集合逐条相等、顺序按同一稳定序；退出码与「是否有差异」一致

#### Scenario: 写盘实现单点

- **WHEN** 审查契约生成面（`crates/core/archi/src/api/**` / `crates/tools/archi-api-gen/src/**` / `crates/infra/archi/src/**` / `src-tauri/src/commands/archi/**`）的文件系统调用
- **THEN** `fs::write` 与 `create_dir_all` 仅在 `crates/infra/archi/src/api_artifacts.rs` 出现（各恰一处）；`core/archi` 侧零写盘调用；`archi-api-gen` 侧零写盘调用；命令层零写盘调用

#### Scenario: CLI 子命令与退出码零变化

- **WHEN** 分别以 `generate` / `--check`（有差异、无差异）/ `bootstrap <FQN>` / 非法参数驱动 CLI
- **THEN** 退出码依次为 0 / 1 / 0 / 0 / 2；`--check` 有差异时 stdout 仍是单行 JSON 且含 `mismatched` / `orphaned` / `missing` 三键；`bootstrap` 只出 stdout 零写盘

### Requirement: 两轴并列呈现与状态矩阵

左栏第三视图 SHALL 把两个正交事实并列呈现，MUST NOT 混成一个「问题总数」：

| 轴 | 事实源 | 谁修 | 呈现 |
|---|---|---|---|
| **生成物新鲜度** | `archi_api_check`（新命令） | 机器（点生成） | 上区：三类差异分组 + 各自计数 |
| **契约 vs 代码** | 漂移报告的 `apiFindings`（既有取数，父级已取） | 人或 agent | 下区：按 `category` 六值 chip 筛选 + 各自计数 |

- **两轴分区独立计数**：两区各自呈现计数，MUST NOT 出现跨轴求和的总数（两轴的可修复者不同，「41 + 3 = 44 个问题」会让用户以为都得手改）。
- **`category` 词汇零新增**：chip 取值 SHALL 取自报告既有开放值集（`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor`），计数由同一份 `apiFindings` 聚合；MUST NOT 在前端另立词表或类别常量。
- **条目点击 = 聚焦**：点击判据条目 SHALL 调既有 `onFocus(element)`（与元素树 / 文件树同一语义：画布切到该元素 1-hop、抽屉取该元素详情），MUST NOT 新增交互范式；`element` 为空的条目 SHALL 只呈文本、不聚焦。
- **「打开」走通用读面**：契约 md / schema / 生成物的路径行 SHALL 经既有 `FileViewer` + `read_file` 打开（`read_file` 已是通用绝对路径读面），MUST NOT 为面板新增专用读命令。
- **五态全画**：

| 态 | 呈现 |
|---|---|
| `loading` | 「解析中…」（沿既有 loading 文案） |
| `enabled = false` | 「契约面未启用」+ 冷启动说明（应生成集此时只含 `INDEX.toml`），且仍呈现 `missing` 实况——MUST NOT 渲成空面板 |
| 零差异 + 零判据 | 「生成物最新，判据 0 条」 |
| 检查 invoke reject | 面板内行内错误（沿 `archi-file-tree-error` 形态），画布与抽屉照常可用 |
| 生成失败 | 错误回显在面板顶部，文案如实（非原子、失败点之前已写） |

- **不阻断画布**：检查或生成失败 MUST NOT 阻断图渲染、MUST NOT 使画布进入停摆态（停摆态的唯一成因仍是模型结构错）。

#### Scenario: 两轴并列且计数不合并

- **WHEN** 打开契约视图，检查返回 3 条差异（2 不符 + 1 缺失）且报告含 41 条判据
- **THEN** 上区呈「差异 2 条不符 / 1 条缺失 / 0 条孤儿」、下区呈「判据 41 条」并按 `category` 分 chip 计数；界面上不出现「共 44 个问题」类跨轴汇总

#### Scenario: chip 过滤与聚焦

- **WHEN** 点下 `entry` chip 并点击其中一条 `apiEntryMissing` 条目
- **THEN** 列表只剩该 category 的条目（计数不变、只筛显示）；条目点击触发一次 `onFocus(element)`——画布切到该元素 1-hop、抽屉取该元素详情（`archi_module` 恰一次）

#### Scenario: 冷启动态不渲空面板

- **WHEN** `archi_api_check` 返回 `enabled = false` 且 `missing` 含 `INDEX.toml`
- **THEN** 面板呈「契约面未启用」+ 冷启动说明，并在差异区照实列出 `INDEX.toml` 一条；MUST NOT 呈现为「无差异」空态

#### Scenario: 检查失败不阻断画布

- **WHEN** `archi_api_check` 以 `Err` 返回（模型结构错 / 契约资产非法）
- **THEN** 面板呈行内错误并可重试；画布与抽屉照常渲染（图零降级），MUST NOT 进入停摆态

### Requirement: 生成交互与模式权限

生成是**写工作树**的动作，交互 SHALL 与既有「写要开 change」纪律同构：

- **二次确认**：点击生成 SHALL 弹二次确认（`StandardDialog` 既有原语），文案 SHALL 含三件事——① 「将写入 N 份（不符 X + 缺失 Y）」；② 「保留 M 份孤儿生成物不做处理」（M 为当前 `orphaned` 长度）；③ 写盘非原子的如实表述。MUST NOT 以轻量 toast 代替确认。
- **成功后重跑检查**：生成成功后 SHALL 触发一次检查重取（用取数 hook 自身的刷新，不重挂整页），并 toast 报 `written` / `unchanged` 两份计数；MUST NOT 让面板停留在陈旧差异上。
- **模式权限（U1）**：生成入口 SHALL 仅出现在 `mode = incremental`（change 详情「架构」页签）；`mode = full`（`/architecture`）MUST NOT 出现可提交的生成入口，改为一行提示「生成需开 change（或让左侧会话执行 `pnpm api:export`）」，形态沿既有 `archi-gate-readonly` 提示。
- **只读对象的延伸**：画布「只读约束的对象 = 画布的人类可提交入口」这一既有裁定同样覆盖本面板的生成按钮——MUST NOT 以「会话能写」为理由为 `full` 模式新增生成入口，亦 MUST NOT 以「画布只读」为理由禁止页内会话执行生成命令。
- **生成期间零并发**：生成进行中 SHALL 禁用生成与检查按钮（防并发写同一批文件）；MUST NOT 在生成未完成时重跑检查。

#### Scenario: 二次确认与重跑检查

- **WHEN** 在 `incremental` 模式下点生成并确认
- **THEN** dialog 文案含「写入 N 份」「保留 M 份孤儿不做处理」与非原子表述；确认后 `archi_api_generate` 恰调用一次，toast 报 written / unchanged 计数，随后 `archi_api_check` 恰调用一次；面板差异按新结果刷新

#### Scenario: 取消零写盘

- **WHEN** 点生成后在 dialog 选择取消
- **THEN** `archi_api_generate` 零调用、磁盘字节零变化；面板维持原差异

#### Scenario: full 模式零生成入口

- **WHEN** 在 `/architecture`（`mode = full`）审查契约视图与源码
- **THEN** 无任何可提交的生成按钮，呈一行「生成需开 change（或让左侧会话执行 `pnpm api:export`）」提示；源码内 `full` 分支零 `archiApiGenerate` 调用路径

#### Scenario: 生成中禁并发

- **WHEN** 一次生成尚未返回时再次点击生成或检查
- **THEN** 两个按钮均为禁用态、无第二次 invoke；生成返回后按钮恢复

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

### Requirement: 抽屉契约段

元素抽屉 SHALL 在「概览」与「文档」之间呈现本元素的契约事实，数据来源零新增：

- **三路径行**：`api.status = declared` 时 SHALL 逐行呈现契约 md / schema / 生成物三个路径（`ArchiApiRef.contractPath` / `schemaPath` / `generatedPath`），各行可经 `FileViewer` 打开；`status = none` 时 SHALL 呈一行「无契约（`api = none` 或契约文件不在案）」且三路径行不渲染。
- **本元素判据**：SHALL 从父级同一份 `apiFindings` 按 `element === fqn` 过滤后逐条列出（`kind` + `message`），零命中时一句「本元素无漂移」；MUST NOT 为抽屉单独 invoke 检查或报告。
- **零新取数**：本段数据 SHALL 全部来自既有 `ArchiModuleDetail.api`（`ArchiApiRef`）与父级已取的报告——MUST NOT 新增 IPC、MUST NOT 二次调用 `archi_module`。
- **不与文档段混装**：契约正文 MUST NOT 进抽屉正文通路（正文仍由 `docs` 关联集 + `read_file` 承担）；本段只呈状态与路径，MUST NOT 内联契约 md 内容。

#### Scenario: 有契约元素的三路径

- **WHEN** 聚焦契约在案的元素（如 `App.Backend.Infra.ArchiRuntime`）打开抽屉
- **THEN** 契约段呈 `status = declared` + 三路径行，点任一行的「查看」经 `read_file` 打开该文件；本元素判据 0 条时呈「本元素无漂移」

#### Scenario: 无契约元素的空态

- **WHEN** 聚焦 `api = none` 或无契约文件的元素
- **THEN** 契约段呈一行「无契约」说明，三路径行不渲染；不报错、不显示失效路径

#### Scenario: 抽屉零新 invoke

- **WHEN** 打开任意元素抽屉并审查 invoke 次数
- **THEN** `archi_module` 恰一次（既有取数），`archi_api_check` / `archi_drift` 均零新增调用

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
| `api::check` / `contract` / `schema` / `paths` / `emit` / `extract` | 零改动 | 十条判据、契约格式、`$defs` 规则、生成物落点与命名判据逐字不变 |

### API: `packages/desktop/src-tauri/crates/infra/archi/src/api_artifacts.rs`（写盘面）

| 项 | 变化 | 描述 |
|---|---|---|
| `write_artifacts` | 新增 | `(root: &Path, &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`：越根复核 → 读盘比对 → `create_dir_all` + `fs::write`；不删文件、不回滚、路径串 repo 相对 |
| `report::write_report` | 零改动 | 报告落盘单点逐字不变（本 crate 现有两处写盘单点并列） |

### CLI: `packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `generate` | 收窄 | 改调 `api::workspace_render` + `archi_runtime::write_artifacts`；输出报 `written` 份数；退出码 0 / 1 语义不变 |
| `--check` | 收窄 | 改调 `api::compare`；单行 JSON 三键输出与退出码语义不变；零写入 |
| `bootstrap <FQN>` | 零改动 | 仍调 `api::bootstrap`，只出 stdout |
| 本地 `write` / `collect` / `differences` / `Report` / `walk*` | 移除 | 全部上移（渲染与比对 → `core/archi`；写盘 → `infra/archi`）；`Cargo.toml` 加 `archi-runtime` 依赖（带用途注释） |

### Component: `packages/desktop/src/views/architecture/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `hooks/use-archi-api-check.ts` | 新增 | `{ root }` → `{ check, loading, error, refresh }`：三触发（挂载 / `root` 变化 / `refresh`）、`root === null` 零 invoke、零轮询；失败清旧数据 |
| `components/contract-panel.tsx` | 新增 | 左栏契约视图：两轴分区 + chip 过滤 + 条目聚焦 + 路径行 `FileViewer` + `[检查]` / 按模式的 `[生成]` + 五态；`data-testid` = `archi-contract-panel` / `archi-contract-axis-artifacts` / `archi-contract-axis-findings` / `archi-contract-chip` / `archi-contract-item` / `archi-contract-check` / `archi-contract-generate` / `archi-contract-status` / `archi-contract-error` |
| `components/architecture-canvas.tsx` | 改 | 左栏三视图分支（第三臂渲 `ContractPanel`，w-80）；把 `report.apiFindings` 与本元素过滤结果传给面板与抽屉；`mode` 决定生成按钮可用性；渲染与标记面零改动 |
| `components/element-drawer.tsx` | 改 | 概览与文档之间插「契约」段（三路径行 + 本元素判据 + 空态）；零新 invoke |
| `components/file-tree.tsx` | 改 | `TreeMode` 扩 `'contract'` + 第三钮（`data-mode="contract"`）；文件 / 元素两视图渲染零改动 |
| 零改动面 | — | `use-archi-graph.ts` / `use-archi-drift.ts` / `use-archi-module.ts` / `use-archi-file-tree.ts`、`lib/marks.ts` / `lib/drift-marks.ts` / `lib/fqn.ts`、`structure-error-panel.tsx`、`element-tree.tsx` |

### CLI: 脚本面（零改动）

| 项 | 变化 | 描述 |
|---|---|---|
| `package.json` 的 `api:export` / `api:check` / `check` | 零改动 | 三段命令形态逐字不变（写盘实现换落点、命令串不变）；`version` 相对 `HEAD` 现值 +1 patch |
| `src-tauri/Cargo.toml` | 零改动 | 无新 crate、无新 workspace 依赖项；crate 版本行不随前端 patch |
