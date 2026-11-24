# desktop-archi-agent Specification（delta: archi-contract-authoring-guidance）

## Purpose

本 delta 只补工具**描述面**与一处草稿场景口径，工具的执行语义零变化：

- `archi_api` 的描述补三句——`bootstrap` 是「新建 / 补契约的第一步」（现状描述只说它出草稿，不说该先用它）、四类生成物点名（契约 md 同目录 `.schema.json` / 架构域 `INDEX.toml` / 入口同级 `.i.rs` · `.i.ts`，仍 MUST NOT 手改）、契约面冷启动口径（`artifacts.enabled = false` 即未启用，此时 `findings` 恒空且**不等于**零问题）。
- `archi_module` 的描述补第四句——`api.schemaPath` / `api.generatedPath` 给出另两条生成物路径（`contractPath` 已指手写真相），使三条路径不必靠猜。
- 草稿形态从「四段」更正为「六段 + 尾部惰性作者注」（形态定义归 `desktop-archi-api` 的同名 delta；本能力只保留「与 CLI `bootstrap` 同一份草稿文本」）；requirement 首句的语义由「契约面自检」改为「契约面自检与草稿」，与它自己的标题「自检与草稿契约」同口径。

`archi_api` 的 action 值集、响应形状、`cap_result` 截断单点、只读档位、注入矩阵与零写盘纪律逐字不变；`archi-knowledge.md` 与 `archi_knowledge.rs` 零改动（骨架 2596 B / 全文 2739 B 两个冻结锚不动——四条描述补位**零 preamble 字节**）。

## MODIFIED Requirements

### Requirement: archi_module 只读工具的定位契约

SDK 引擎 SHALL 提供专用只读工具 `archi_module`（不在七通用工具 `TOOL_NAMES` 封闭集内，经 `SessionToolset` 受控注入），语义为**定位**（已知在看哪个模块）：

```jsonc
{ "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" } }
```

- 指定 `element` 时响应 SHALL 含：`element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine` / `entry`）、`dependsOn`（`target` + `description`）、`dependedBy`（`source` + `description`）、`children`（直接子元素 FQN）、**`doc`（单值入口文档路径，仅 `api = none` 元素有值）**、**`api`（并列契约状态字段）**、**`resolvedEntry`（入口解析结果）**。`docs` 数组与 `warnings` 数组两个位次已退役。
- 省略 `element` 时 SHALL 返回全部元素的**薄概览**（`fqn` / `kind` / `gate` / 一行 `summary` / **`api.status`** / **`hasPath`**，无邻接、无入口文档、无契约路径），并受既有单结果字节上限约束（超限截断留痕）。
- **`resolvedEntry`（入口解析结果）**：SHALL 为 `{ path: string | null, via: "explicit" | "convention" | null, problem: string | null }` 三字段（字段不省略）——`path` 为 repo 相对 POSIX 入口路径，`via` 区分「模型显式 `entry`」与「约定默认命中」（单文件根 = 根文件自身 / 目录型根 = `<root>/index.ts(x)` / crate = `src/lib.rs`），`problem` 在入口不可用或不存在时给出人读成因。判据 SHALL 由 `api::paths::entry_state` **单点**求值（MUST NOT 在工具执行体或 `dto` 内另写入口解析、MUST NOT 复用 `apiFindings` 的判定分支）；该字段 MUST NOT 承载 `entry` 四条判据的长表（判据定义归 `desktop-archi-api` / `desktop-archi-api-check`）。
- **`hasPath`（`path` 事实）**：薄概览每条 SHALL 含 `hasPath: bool`（= 模型 `path` 数组非空，纯投影不补默认值），使「谁缺 `path`」一次调用即可判出；MUST NOT 以「`path` 字段是否省略」表达同义（显式字段不省略是既有投影纪律）。
- `modelFile` + `modelLine` SHALL 来自解析器已产出的声明位置（零额外实现），使 agent 能直接 `edit` 到该行而无需 grep 猜测。
- 元素不存在 SHALL 返回可诊断 `Err`（附近似 FQN 或总量提示），MUST NOT 返回空对象。
- 模型结构错时 SHALL 返回结构化诊断清单作为 `Err`（绝不 panic），使 agent 能看出"模型本身坏了"而不是"没有这个模块"。
- 响应 SHALL 经 `archi::dto` 单点投影产出（MUST NOT 在工具执行体内另拼一份 JSON）。
- **`api` 并列字段**：单元素响应 SHALL 并出 `api` 对象（`status`（`declared` / `none`）/ `contractPath` / `schemaPath` / `generatedPath`），定位 SHALL 按 `<archi_models_dir>/<分层目录…>/<FQN>.md` 与相应的 `.schema.json` / 生成物路径**派生**（全经 `foundation::layout` 的派生族与 `api::paths` 单点，MUST NOT 行内拼路径；`api` 段已退役，契约资产落在契约资产目录 `models/` 的 `<分层目录…>/` 子目录内）；文件不存在即 `status = none` 且三个路径字段为 `null`（零配置，「有契约 / 无契约」成为可枚举事实）。该字段 MUST NOT 承载协议正文（正文由 agent 用 `read` 按路径自取），MUST NOT 成为独立工具、MUST NOT 改变本工具的只读属性与注入矩阵。
- **`doc` 并列字段**：`doc` SHALL 为 repo 相对 POSIX 路径**单值**或 `null`（无 `title` / `kind` / `source` 四字段形态）。只有 `api = none` 的元素会填；`api = declared` 元素的入口文档走既有 `api.contractPath`，MUST NOT 在 `doc` 里重复出线。`doc` 指向的文件不存在时该元素的判据面报 `docNotFound`（`error` 档）——工具响应本身 MUST NOT 为此降级或补默认值。
- **描述面（`mount.rs`）**：`archi_module` 的工具描述 SHALL 含四句——「缺 `path` 的元素可由 `hasPath` 一次判出」「`resolvedEntry` 给入口解析结果（显式 / 约定默认 / 不可用成因）」「新元素字段形态用 `archi_scaffold` 取模板」「`api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）」；且 MUST NOT 复制字段语法长表（字段序 / 内联 `metadata` 的形态知识归模板工具）——MUST NOT 残留 `docs` 词汇或警告语义。

#### Scenario: 定位单元素

- **WHEN** 模型会话中调用 `archi_module(element="App.Auth")`
- **THEN** 返回该元素的 `kind` / `gate` / `path` / `metadata` / `modelFile` / `modelLine` / `entry`，其出边与入边（各含对端 FQN 与 `description`）、直接子元素清单、`doc`（单值或 `null`）、`resolvedEntry`；响应内 MUST NOT 出现 `docs` 数组

#### Scenario: 省略元素给薄概览并截断

- **WHEN** 在不传 `element` 的调用下模型含数百元素
- **THEN** 返回全元素薄概览（含 `hasPath`，无邻接、无入口文档），超出单结果字节上限时截断并留痕（提示可指定元素收窄）

#### Scenario: 缺 path 一次判出

- **WHEN** 省略 `element` 调用，模型内既有声明了 `path` 的叶元素、也有无 `path` 的容器元素
- **THEN** 前者 `hasPath = true`、后者 `hasPath = false`（字段不省略）；`path` 数组本身只在单元素响应出线

#### Scenario: 入口三态可辨

- **WHEN** 分别对「显式 `entry` 命中」「目录型根由 `index.ts` 约定默认命中」「`path` 指向不存在目录而入口不可用」的三元素调用 `archi_module(element = …)`
- **THEN** 依次为 `resolvedEntry = { path: "<显式值>", via: "explicit", problem: null }` / `{ path: "<index 路径>", via: "convention", problem: null }` / `{ path: null, via: null, problem: "<不可用成因>" }`；判据与 `apiFindings` 的 `apiEntryMissing` 逐例一致（同一 `entry_state` 求值）

#### Scenario: 坏模型可辨

- **WHEN** 模型文件被写坏（语法错 / 环）后调用 `archi_module`
- **THEN** 返回结构化诊断（含 `file:line`）且 `is_error` 为真；执行体不 panic，run 不中断（agent 可转去修模型）

#### Scenario: api 并列字段两态可辨

- **WHEN** 分别对契约在案的元素（`Backend.Infra.ArchiRuntime`）与无契约元素调用 `archi_module(element = …)`
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `.dev-team/architecture/models/Infra/` 下的实际文件、`generatedPath` 指向入口同级生成物；后者 `api.status = "none"` 且三者 `null`

#### Scenario: doc 只在 api = none 元素上有值

- **WHEN** 分别对 `api = none` 且声明了 `doc` 的元素、`api = none` 未声明 `doc` 的元素、`api = declared` 的元素调用 `archi_module`
- **THEN** 依次为「路径串」/ `null` / `null`（字段不省略）；declared 元素的入口文档只能经 `api.contractPath` 读到——两处 MUST NOT 同时给出口 entry 文档（否则元素信息面板会出现两条）

#### Scenario: 薄概览带 api.status 但不带路径与 doc

- **WHEN** 省略 `element` 调用 `archi_module`
- **THEN** 每条薄概览含 `api.status`（`declared` / `none`）与 `hasPath`，不含 `contractPath` / `schemaPath` / `generatedPath` / `doc` / `resolvedEntry`；超单结果字节上限时仍按既有纪律截断留痕

#### Scenario: 契约正文不经工具响应

- **WHEN** 审查 `archi_module` 响应体与体积
- **THEN** 只含 `api` 四字段（状态 + 三路径）、`doc` 单值、`resolvedEntry` 三字段与 `hasPath` 布尔；零签名、零 `$defs`、零散文、零四字段文档条目；正文由 agent 用 `read` 按路径自取

#### Scenario: 契约路径零 `api` 段残留

- **WHEN** 对本仓契约在案元素（`Backend.Infra.ArchiRuntime`）取 `archi_module` 响应，并审查响应内的三个路径串
- **THEN** `contractPath` 为 `.dev-team/architecture/models/Infra/Backend.Infra.ArchiRuntime.md`、`schemaPath` 为同目录的 `.schema.json`；三串内 MUST NOT 出现 `architecture/api` 形态（旧段已退役）；派生点消失后仍由 `api::paths` 单点产出（工具执行体零路径拼接）

### Requirement: archi_api 只读工具的自检与草稿契约

SDK 引擎 SHALL 提供专用**只读**工具 `archi_api`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**契约面自检与草稿**（与本 requirement 标题同口径：`check` 自检、`bootstrap` 起草）：

```json
{ "name": "archi_api",
  "arguments": { "action": "check" | "bootstrap", "element": "<FQN>" } }
```

- **两动作封闭**：`action` SHALL 只接受 `check` / `bootstrap` 两值（封闭值集；词汇外值以 `is_error` 回灌并点名合法值，MUST NOT 静默退化）。
- **`check` = 两轴结构化结果**：返回「生成物新鲜度」与「契约 vs 代码」两轴的并列内容——① 三类差异（`mismatched` / `orphaned` / `missing`，repo 相对串，取自与 IPC / CLI 同一份比对实现）；② 契约判据摘要（按 `category` 计数的映射 + 逐条 `{ kind, category, element, target, message }`）。响应 SHALL 复用既有出线形状（`ArchiApiCheck` 的字段与 `ArchiApiFinding` 的条目），MUST NOT 在工具执行体内另拼一份投影；MUST NOT 各自实现在第二处（判定单源）。
- **`bootstrap` = 草稿文本（起草第一步）**：`element` 必填，返回与 CLI `bootstrap` 同一份草稿文本（契约 md 骨架，只出 stdout 语义）；该动作是新建 / 补契约的**第一步**，描述面 SHALL 说出这句指路（否则 agent 只知它能出草稿、不知该先用它）。草稿形态（六段骨架 + 尾部惰性作者注）归 `desktop-archi-api`「bootstrap 草稿的六段骨架与惰性作者注」，本能力 MUST NOT 另立第二份形态判据。`element` 缺失 / 元素不存在 / 无实现入口 ⇒ `is_error` 并透传 `ApiError` 文案。
- **零写盘**：两动作 MUST NOT 创建 / 覆盖 / 删除任何文件——生成仍由会话经既有 `bash` 工具执行本仓的生成命令（命令名归属本仓文档面，产品 MUST NOT 内建具体命令名）。MUST NOT 存在「写生成物」的专用工具（`archi-runtime` 对 agent 面不可达，见 `desktop-crate-layout`）。
- **结构错以 `is_error` 回灌**：模型含任一结构诊断时两动作 SHALL 返回 `is_error` ToolResult（诊断清单带 `file:line`），MUST NOT 返回部分结果、MUST NOT 抛 panic。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）；超限时摘要（按 `category` 计数）优先保留。
- **上下文路由沿用**：执行体 SHALL 经 `LoopTurn` 的 per-turn `ArchiContext` 路由（root + 模型读面），MUST NOT 新增 `ToolExec` 变体、MUST NOT 把 store 句柄塞进七通用工具的 `execute` 签名。
- **无路径参数**：本工具不带 `path` 参数 ⇒ 路径沙箱对它恒原样返回（MUST NOT 因缺字段报错），写入保护对它零射程（读面全开放）。
- **提示面**：被下发本工具的相位 prompt SHALL 有一行说明（沿既有「额外可用只读工具」写法，当前为 `implementation-generator.md`），内容限于「改完代码可自查生成物新鲜度与契约判据；生成物过期用既有命令重生成，MUST NOT 手改生成物」。
- **描述面（`mount.rs`）**：`archi_api` 的工具描述 SHALL 含三句，且 MUST NOT 复制契约 md 的段形态长表（形态归 `desktop-archi-api`）、MUST NOT 内建本仓专用命令名（生成命令归属各仓文档面）——① `bootstrap` 是新建 / 补契约的第一步；② 四类生成物的点名（契约 md 同目录 `<FQN>.schema.json`、架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）连同「MUST NOT 手改生成物」；③ 冷启动口径「`artifacts.enabled = false` 即契约面未启用，此时 `findings` 恒空且**不等于**零问题」。

#### Scenario: check 返回两轴且零写盘

- **WHEN** 在 implement/executor 会话（或架构页 `source = "archi"` 会话）中调用 `archi_api(action = "check")`
- **THEN** 响应含三类差异与按 `category` 计数的判据摘要（逐条含 `kind` / `element` / `message`）；调用前后磁盘字节完全相同；`archi_api` 与 IPC 的 `archi_api_check` 报的三类差异集合逐条相等

#### Scenario: bootstrap 只出草稿

- **WHEN** 调用 `archi_api(action = "bootstrap", element = "App.Auth")`
- **THEN** 返回契约 md 草稿文本（H1 + 六段：职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由，末尾带惰性作者注——形态归 `desktop-archi-api`），磁盘零变化；缺 `element` 或元素不存在 ⇒ `is_error` 且文案点名成因

#### Scenario: 词汇外动作被拒

- **WHEN** 调用 `archi_api(action = "generate")` 或 `action = "write"`（词汇外值）
- **THEN** 返回 `is_error` 并点名合法值 `check` / `bootstrap`；零写盘、零副作用

#### Scenario: 结构错回灌而不崩

- **WHEN** 模型被写成语法错后调用 `archi_api(action = "check")`
- **THEN** 返回 `is_error` ToolResult（诊断带 `file:line`），run 不中断、进程不 panic

#### Scenario: 体量经单点上限

- **WHEN** 契约面有数十条判据时调用 `check`
- **THEN** 响应被 `tools::cap_result` 截断（沿既有 L1 上限），摘要保留；工具执行体内零第二份截断实现

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/mount.rs` | 工具定义面（本变更改两个描述函数） | `archi_api_definition()` 描述补三句：① `bootstrap` 指路（新建 / 补契约第一步取草稿）；② 四类生成物点名（同目录 `.schema.json` / 架构域 `INDEX.toml` / 入口同级 `.i.rs` · `.i.ts`，保留「MUST NOT 手改生成物」原句）；③ 冷启动口径（`artifacts.enabled = false` ⇒ 契约面未启用，`findings` 恒空不等于零问题）。`archi_module_definition()` 描述补第四句：`api.schemaPath` / `api.generatedPath` 给另两条生成物路径。两处参数 schema、工具名常量、`ToolTier::Read` / `ToolExec::Archi` 归属与 `build_registry` 项数零变化 |
| `crates/infra/agent/src/sdk/mount_test.rs` | 描述断言面 | `archi_api定义描述与参数形状` 的 needle 扩到 `第一步` / `.schema.json` / `INDEX.toml` / `.i.` / `enabled` / `findings`（`bootstrap` 一词既有描述已有，不作新针；既有 `MUST NOT` / `手改生成物` / `重生成` 三条仍须绿）；`archi_module描述补三句指路` 的 needle 扩到 `schemaPath` / `generatedPath`；两条既有边界（`工具描述零项目专用命令`、`archi_module描述零字段形态教学` 的零代码围栏）保持绿 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具执行体（零改动） | `check` 仍回 `{ artifacts, findings: { total, byCategory, items } }`，`bootstrap` 仍回 `{ draft }`；`artifacts.enabled` 的出处（`api::artifact_check`）与判据语义不变——本变更只把「怎么读这个字段」写进描述 |
| `crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` | 注入知识（零改动） | 骨架与派生段逐字不变（2596 B / 2739 B 冻结锚）；四条描述补位 MUST NOT 进 preamble |
| `crates/core/workflow/src/prompts/*.md` | 相位 prompt（零改动） | 14 份（6 executor + 8 evaluator）逐字不变（含 `implementation-generator.md` 的 `archi_api` 一行）——描述面已足够，不进 prompt |

### Function: `crates/infra/agent/src/sdk/mount.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api_definition()` | 改（描述文本） | 在既有描述内加三句（指路 / 生成物点名 / 冷启动口径）；`parameters` 的 `action` 二值 enum 与 `element` 字段零变化 |
| `archi_module_definition()` | 改（描述文本） | 加一句（两条生成物路径）；其余句子与零代码围栏纪律逐字保留 |
| `lookup` / `build_registry` | 零改动 | 成员校验、档位与挂载清单项数不变 |

### API: `archi_api` / `archi_module`

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api` 响应形状 | 零改动 | `check` 两轴并列内容、`bootstrap` 的 `{ draft }`、`is_error` 回灌与 `cap_result` 截断单点逐字不变 |
| `archi_module` 响应形状 | 零改动 | `api` 三路径 / `doc` / `resolvedEntry` / `hasPath` 的投影与 DTO 零变化——本变更只让描述说明这些字段怎么用 |
| 注入矩阵 | 零改动 | `archi_api` 仍只落 `(implement, executor)` 与 `source = "archi"` 两处；不在本变更扩面 |
