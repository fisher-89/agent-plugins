# desktop-archi-model Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**目录双身份的声明面校准**（E-1 目录迁移）：`models/` 从此同时住「模型声明」（一级 `NN-<域>.toml`）与「契约资产」（`<Domain>/` 子目录下的 `<FQN>.md` + `.schema.json`）。模型收面判据（一级 + `.toml` 后缀）与解析实现**逐字不变**，变的只是「这条判据在 spec 里必须写成机械事实」——否则同目录出现 md / schema / 子目录时无人知道解析为何仍安全。字段面（`kind` / `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata` 八项）与 `deny_unknown_fields` 语义零改动；契约状态字段 `api` 的定位串换段（`<archi_api_dir>` → `<archi_models_dir>`）。

## MODIFIED Requirements

### Requirement: 多文件聚合与声明定位

解析层 SHALL 聚合 `<archi_dir>/models/` 下的**一级** `*.toml`（不递归），按文件名**字典序**读取并合并为一个 `Model`。每个元素与每条关系 SHALL 携带声明位置：

- `modelFile` = 声明所在文件的 **repo 相对路径**（`/` 分隔）；
- `modelLine` = 元素表头键所在行（1 起始）或关系条目 `from` 键所在行。

位置 SHALL 取自 `toml` + serde 反序列化同时产出的 span（`span` → 行列换算），MUST NOT 以再扫文本、模糊匹配或二次 grep 反推。关系级 span SHALL 保留可选位（`Option<SourceSpan>`），当前只消费行号、不消费列区间。

- **收面判据 SHALL 恰为「一级文件 + `.toml` 后缀」两条**（`read_dir` 逐条 `is_file()` + 后缀匹配，MUST NOT 递归、MUST NOT 按目录名或文件名前缀（`NN-`）白名单筛）：`models/` 同时容纳契约资产（`<Domain>/` 子目录内的 `*.md` / `*.schema.json`）与模型文件，两类靠这两条判据天然分离——子目录不递归即不可见，md / schema 后缀不匹配即不收。收面 MUST NOT 因同目录多出住户而扩张或收窄，也 MUST NOT 引入「跳过契约目录」一类豁免名单（那是第二份身份判据）。
- **索引 MUST NOT 落 `models/` 根**：模型收面的 `.toml` 后缀会把 `<archi_dir>/models/INDEX.toml` 当模型文件解析，其 `[[entry]]` 表在 `deny_unknown_fields` 下产 `unknownField` 诊断并使模型停摆；契约索引的合法落点是 `<archi_graph_dir>/INDEX.toml`（`desktop-archi-api`「schema.json 与 INDEX.toml 为纯派生且不可手改」）。本能力 MUST NOT 为兼容该摆放而放宽收面判据或为 `INDEX.toml` 开字段白名单。

**跨文件重复 FQN**：同一 FQN 在一个文件内重复声明由 TOML map 语义天然拒绝；**跨文件**重复 SHALL 由聚合层自实现检出，诊断 `kind = duplicateElement`，`file` / `line` SHALL 指向**后出现**的那条声明（字典序靠后的文件内位置）。聚合层 MUST NOT 静默取其一。

`models/` 目录缺席或为空 SHALL 解析为**空模型**（零元素零关系零诊断），MUST NOT 报错——空模型是冷启动的合法起点。目录存在而其一级零 `*.toml`（例如只剩 `<Domain>/` 契约子树）SHALL 按同一口径解析为空模型：零诊断、零元素，MUST NOT 因「目录非空」判为异常。

#### Scenario: 多文件字典序聚合

- **WHEN** `models/` 内含 `02-auth.toml` 与 `01-core.toml`（各声明不同元素，`02` 引用 `01` 的元素）
- **THEN** 聚合结果含两文件的全部元素与关系，字典序决定聚合次序（`01` 先于 `02`），跨文件 `unknownReference` 不误报；每条元素 / 关系的 `modelFile` 指向各自文件

#### Scenario: 声明行号可点击

- **WHEN** 查询任一元素的 `modelFile` + `modelLine`
- **THEN** 该文件该行为 `[element."<fqn>"]` 表头；查询任一关系的 `modelLine` 为该 `[[relation]]` 条目 `from` 键所在行

#### Scenario: 跨文件重复 FQN 被拒

- **WHEN** `01-core.toml` 与 `03-billing.toml` 都声明 `[element."App.Auth"]`
- **THEN** 聚合返回 `duplicateElement` 诊断，`file` 为字典序靠后的那个文件、`line` 为其中的声明行；模型判定为停摆态，MUST NOT 静默保留任一份

#### Scenario: 同文件重复 key 由 parser 拒绝

- **WHEN** 单文件内重复声明 `[element."App.Auth"]`
- **THEN** TOML 层直接拒绝（`duplicate key` + 位置 + 指向冲突 key 的 `^^^`），诊断映射为 `duplicateElement`，无需聚合层介入

#### Scenario: 空模型合法

- **WHEN** `<archi_dir>/models/` 不存在或为空目录
- **THEN** 返回空模型（零元素 / 零关系 / 零诊断），画布呈现空态而非停摆态，冷启动可继续

#### Scenario: 契约子树与模型文件混居零干扰

- **WHEN** `<archi_dir>/models/` 一级为六个 `NN-<域>.toml`，同目录另有 `<Domain>/<FQN>.md`、`<Domain>/<FQN>.schema.json` 与 `<Domain>/Sub/` 三级子目录，跑一次解析
- **THEN** 元素 / 关系计数与只放六个 toml 时逐字相同、`diagnostics` 空、`modelFile` 仍指向一级六个文件；删掉全部六个 toml 后（只留契约子树）解析为空模型且零诊断——两例 MUST NOT 出现「把 md 当模型文件」或「因子目录存在而报错」

### Requirement: IPC 命令面契约（八命令）

命令层 SHALL 提供八个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff 全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + `docs` + **`api`（并列字段）** |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（元素存在 / 值合法 / 渲染自证可解析）通过后原子写回该模型文件；失败零写入并显式 `Err` |
| `archi_drift` | `root` + `mode` | `ArchiDriftReport`：全量 / 增量两模式同源判定（`mode` = `full` \| `incremental`）；命令层零判定规则 |
| `archi_file_tree` | `root` | `ArchiFileTree`：仓库源文件与模块根的扁平条目投影（左栏文件树，契约归 `desktop-archi-file-tree`）；结构诊断非空 → `Err` |
| `archi_api_check` | `root` | `ArchiApiCheck`：四类生成物与契约渲染结果的**三类差异**（`mismatched` / `orphaned` / `missing`）+ `enabled` + `meta`；零写盘（契约归 `desktop-archi-contract-panel`） |
| `archi_api_generate` | `root` | `ArchiApiWriteOutcome`：把应生成物写盘（`written` / `unchanged` 两列）；**只增只改、从不删文件**；结构诊断非空 → `Err` 且零写盘 |

文档正文读取 SHALL 由通用 `commands::fs::read_file`（见 `desktop-file-read`）按绝对路径承担：原 `archi_read_doc`（关联集成员校验 + 读盘）已退役，架构域不再持有专用文档读取命令。

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。
- `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check` MUST NOT 写盘；写命令 SHALL 恰为两条——`archi_set_gate`（只改 `gate` 单字段）与 `archi_api_generate`（只写四类生成物，零删除），其余六条 MUST NOT 触碰磁盘。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（只读视图约束由前端承载、由命令可用性背书——命令本身不区分模式）；`archi_api_generate` 同理只在 `mode = incremental` 的前端调用路径出现（生成权限归 `desktop-archi-contract-panel`）。
- 八个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役，`check` 链只前置重导出、不做生成物比对）。命令清单与出线类型清单的机械守卫（`bindings/mod_test.rs`）SHALL 在同一 change 内随动（条数断言只作清单长度连带核对，MUST NOT 作为语义判据）。
- **`archi_module` 的 `api` 并列字段**：`ArchiModuleDetail` 的并列字段 `api`（`ArchiElement` 与薄概览条目同步带 `api` 状态），语义为「该元素的契约是否在案」——按 `<archi_models_dir>/<Domain>/<FQN>.md` 约定定位（`api` 段退役后契约资产落在模型目录的 `<Domain>/` 子目录内），文件不存在即 `status = none`（零配置，且「有契约 / 无契约」成为可枚举事实）。字段结构与定位规则定义归 `desktop-archi-api`，本能力只登记「`archi_module` 出参形状扩展、命令层仍零领域规则」。该字段 MUST NOT 承载协议正文（契约正文与模块文档正文同一通路：由调用方经 `read_file` 自取）。

#### Scenario: 读命令零写盘

- **WHEN** 依次调用 `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check`
- **THEN** 磁盘字节零变化（模型、源码、文档与四类生成物均只读）；取数结果与 `archi` 投影逐字段一致（命令层零改写）

#### Scenario: 文档关联集不含契约正文

- **WHEN** 审查 `archi_module` 返回的 `docs` 条目集合
- **THEN** 集合内不含 `<archi_dir>/models/**` 路径（关联集零契约正文通路；契约 md 落在 archi 树内，而 archi 树整棵对 `docs` 三源不可见）；契约正文的存在与否只经 `api` 字段表达

#### Scenario: gate 写回原子与零残留

- **WHEN** 调用 `archi_set_gate` 改合法元素的 `gate`，以及以非法值 / 不存在元素调用
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

#### Scenario: archi_module 出参带 api 并列字段

- **WHEN** 调用 `archi_module(root, element = "App.Backend.Infra.ArchiRuntime")`（该元素契约在案）与另一无契约元素
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `<archi_models_dir>/Infra/` 下的实际文件；后者 `api.status = "none"` 且三个路径字段为 `null`；两者其余字段（元素 / 邻接 / 子元素 / `docs`）语义零变化，命令总数仍为八个

#### Scenario: 八命令在册且恰两个写面

- **WHEN** 审查 `all_commands!` 的 archi 段与 `bindings.ts` 的 typed 包装
- **THEN** archi 段恰八条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate`），每条各有对应包装；源码中含写盘调用的命令恰为 `archi_set_gate` 与 `archi_api_generate` 两条

## Module Contract

### Function: 受影响入口

| 模块 | 变化 | 关键契约 |
|---|---|---|
| `crates/core/archi/src/parse/mod.rs` | 语义零变化、判据显式化 | `parse_dir` 的收面仍是「`archi_models_dir` 一级 `is_file()` + `MODEL_FILE_SUFFIX` 后缀」，MUST NOT 递归、MUST NOT 加目录豁免；`<Domain>/` 契约子树对聚合面不可见 |
| `crates/core/archi/src/dto.rs` | 改（定位串换段） | `ArchiApiRef` 的三路径由 `api::paths` 派生后转 repo 相对形态；契约 md / schema 落在 `<archi_models_dir>/<Domain>/`（原 `archi_api_dir` 已退役）；DTO 形状（字段名 / 顺序 / serde / specta 派生）零变化。**绑定面说准**：`dto.rs` 的两处 doc 方位串经 specta 抄进 `bindings.ts` ⇒ 该文件恰 2 行 doc 注释随动（形状面零 diff），`bindings.zod.ts` 零 diff、`bindings:export` 幂等——MUST NOT 表述为「`bindings` 零再生成」 |
| `crates/core/archi/src/model.rs` | 改（**仅 `api` 字段 doc 串**） | 第 46 行「接口定义写在架构域 `api` 目录的契约资产里」改为「架构域 `models/<Domain>/` 目录」；字段面（`kind` / `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata` 八项）、`deny_unknown_fields` 与解析实现逐字不变——该行是本文件唯一允许的 diff |
