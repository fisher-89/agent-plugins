# desktop-archi-model Specification（delta: module-doc-field-docs-retirement）

## Purpose

本 delta 做**字段面与线面的换面**（E-2）：元素字段 `docs: Vec<String>` 退役、改 `doc: Option<String>` 单值（`api = declared` 元素 MUST NOT 写），`api` 状态的语义升格为**入口文档载体的开关**；`ArchiModuleDetail` 的 `docs` / `warnings` 两个位次退场、`doc` 进场；元素投影 `ArchiElement` 同步携带 `doc` 的模型拷贝（`archi_graph` 的 `elements[]` 与 `archi_module` 的 `element` 共用该投影）——这是元素投影上唯一的授权进场字段；消费面词汇集对拍删掉已退役的「文档诊断 `docNotFound`」一行（`WarningKind` 整组词汇随 `docs` 三源消失）。值域封闭、`deny_unknown_fields`、投影零改写（新字段只是模型拷贝）、八命令面与写面身份逐字不变。

## MODIFIED Requirements

### Requirement: 架构模型文件格式与字段封闭

架构模型 SHALL 落位 `<archi_dir>/models/*.toml`（域根与子目录经 `foundation::layout` 派生，MUST NOT 行内字面量），格式 SHALL 为 TOML「一元素一表头」：

```toml
[element."App"]
kind = "package"

[element."App.Auth"]
kind = "domain"
path = ["./src/auth/", "./src/auth-legacy/"]
gate = "strict"
summary = "认证与令牌签发"
metadata = { owner = "platform" }

[element."App.Backend"]
kind = "domain"

# 目录型壳的约定默认拼成 .../src-tauri/src/src/lib.rs（指不到），故此处显式 entry 是正当用法
[element."App.Backend.Shell"]
kind = "module"
path = ["./packages/desktop/src-tauri/src/"]
entry = "./packages/desktop/src-tauri/src/lib.rs"

# 无公共 API 的元素用 doc 声明入口文档（本变更新增字段；declared 元素 MUST NOT 写 doc）
[element."App.Backend.Tools.ExportBindings"]
kind = "module"
path = ["./packages/desktop/src-tauri/crates/tools/export-bindings/"]
api = "none"
doc = "./packages/desktop/src-tauri/crates/tools/export-bindings/README.md"

[[relation]]
from = "App.Auth"
to   = "App.Shared"
desc = "复用错误类型"
```

- **键形裁决**：`element` 表的键 SHALL 为**带引号的完整 FQN 单键**（`[element."App.Auth"]`），MUST NOT 用未引号点分形式（`[element.App.Auth]`）声明层级——两种写法并存会使同一层级存在两种拼法、歧义不可判。层级 SHALL 由 **FQN 最长前缀**推导（`App.Auth.Token` 的父亲是 `App.Auth`），MUST NOT 由 TOML 表嵌套推导。
- **字段面封闭**：元素字段 SHALL 为标量 / 数组 / **内联表**三形态；`metadata` MUST NOT 用子表（`[element."X".metadata]`）表达——子表与「子元素表头」同构，会被读成元素 `X.metadata`。
- **值域封闭**：`kind` 必填且 SHALL 为 `package` / `domain` / `module` / `component` 四值之一；`gate` 可选且 SHALL 为 `off` / `warn` / `strict` 三值之一（缺省 `off`）；**`api`** 可选且 SHALL 为 `declared` / `none` 二值之一（缺省 `declared`）；**`entry`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组）；`path` SHALL 为字符串数组（缺省空）；**`doc`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组——数组会让「入口文档唯一」不可表达）；`summary` 可选字符串；`metadata` SHALL 为字符串→字符串的自由表（未知键忽略，非字符串值即 `invalidValue`）。**`docs` 已退役**：字段面 MUST NOT 含 `docs`，写了即 `unknownField`。
- **未知字段拒绝**：元素表内出现未列出的字段（如 `gatte` / `docs`）SHALL 由 `deny_unknown_fields` 拒绝（`unknownField`）。
- **属性与语义分离**：`gate` 档位 SHALL 是模型属性（本能力定义其值域与缺省）；档位的**语义消费**（参与归属、违例定级、门禁结论）归 `desktop-archi-drift` 与 `desktop-archi-gate`，本能力 MUST NOT 定义任何定级规则。
- **`api` 状态指针（显式字段，非 `metadata`）**：元素 SHALL 有封闭二值的 `api` 字段（`declared` 缺省 / `none`，附 `ApiStatus::ALL`），语义在本变更新增一层：**它是入口文档载体的开关**——`declared` ⇒ 入口文档 = 派生落点 `<models>/<Domain>/<FQN>.md`（= `contract_path`，`doc` MUST NOT 再声明第二个事实）；`none` ⇒ 入口文档 = `doc` 声明（可选）。该字段 MUST NOT 承载任何协议正文（签名 / 符号 / 接口 / 规范全文）。走显式字段而非 `metadata` 自由键的理由是可获得 `deny_unknown_fields` 的拼写校验：`api = "non"` 报 `invalidValue` 并列全合法值，而 `metadata = { api = "non" }` 会静默变成「未声明」。完备性判据（每个 `path` 非空元素 ∈ {`none`} ∪ {载体在案}）归 `desktop-archi-api-check`。
- **`entry` 是实现入口指针（显式字段，非 `metadata`）**：语义为「本模块的实现入口文件」——**单值**、repo 相对 POSIX 路径（与 `path` 同口径，`./` 前缀由归一剥除、不参与身份）。四条判据（基准 / 落位 = 某条 `path` 或目录型 `path` 之内 / 存在 = 盘上必须有 / 跨元素唯一）与三态解析链（声明 → 语言约定默认 → 无入口）归 `desktop-archi-api`「生成物落点与入口查找」，本能力只定义字段本身的值域与解析面。走显式字段而非 `metadata` 自由键的理由同 `api`。
- **`doc` 是入口文档指针（显式字段，本变更新增）**：语义为「本模块的入口文档路径」——**单值**、repo 相对 POSIX 路径（`./` 前缀由归一剥除、不参与身份；无绝对前缀 / 无盘符 / 无 `..` / 无空段）。两条形态纪律：① **只有 `api = none` 的元素可写**（`declared` 的入口文档由派生落点给出）；② 值 MUST NOT 指向 `<archi_dir>/models/<Domain>/**` 与 `openspec/specs/**`。`doc` 的存活性、fence 有无与三值判据（`docMissing` / `docNotFound` / `apiDocMissing`）归 `desktop-archi-api-check`，本能力只定义字段与值域。走显式字段而非 `metadata` 的理由同 `api`：数组形态与拼错键必须能机械拒收。
- **三个指针都 MUST NOT 承载协议正文**：`entry` 是**位置**指针（文件在哪），`api` 是**状态**指针（有无公共 API），`doc` 是**入口文档**指针（读哪一份就懂这个模块）；三者都 MUST NOT 让模型开始携带签名 / 导出符号 / 接口定义。单文件根元素（`views/welcome-view.tsx` 一类）MUST NOT 声明 `entry`——它的根文件就是入口。
- **协议正文不经模型**（知情边界，防误加字段）：元素上 MUST NOT 出现接口 / 导出符号 / 签名一类协议**正文**字段——唯一允许的协议相关字段是上述三个指针（`api` / `entry` / `doc`）。契约权威在 `<archi_models_dir>/<Domain>/<FQN>.md`（见 `desktop-archi-api`），模型只存状态指针。
- 新元素缺省 `gate = "off"`（ratchet：存量全 `off`，想收紧的模块逐个改 `strict`，一个 change 收紧一块）。

#### Scenario: 四值 kind 与三值 gate 解析成功

- **WHEN** 以合法模型（含 `package` / `domain` / `module` / `component` 四类元素与 `off` / `warn` / `strict` 三档 `gate`，其中至少一个元素省略 `gate`）
- **THEN** 解析成功；省略 `gate` 的元素取 `off`；每个元素的 `path` 缺省为空数组、`metadata` 缺省为空表、`entry` 与 `doc` 缺省为 `null`

#### Scenario: 非法 kind 与非法 gate 被拒并列全合法值

- **WHEN** 模型写 `kind = "pakage"` 或 `gate = "stirct"`
- **THEN** 解析返回 `invalidValue` 诊断，`message` 含 parser 原文（`line` / `column` / 源码片段 / `^^^^` 指示）并**列出全部合法值**（`package` / `domain` / `module` / `component` 或 `off` / `warn` / `strict`），模型整体判定为停摆态

#### Scenario: 内联 metadata 与非内联形态

- **WHEN** 分别以 `metadata = { owner = "platform", tier = "core" }` 与 `[element."App.Auth".metadata]` 子表形态写 metadata
- **THEN** 内联形态解析成功且未知键被接受；子表形态不产生 `App.Auth` 的 `metadata`（该表被读成另一个元素 FQN），并因该元素缺 `kind` 报 `missingField`

#### Scenario: 未引号点分形式被拒

- **WHEN** 模型以 `[element.App.Auth]`（未引号点分）声明层级
- **THEN** 该写法不被承认为合法元素表头（FQN 键形不符）：模型按「无元素 / 缺字段」处置并给出诊断，MUST NOT 与 `[element."App.Auth"]` 合并为同一元素

#### Scenario: 未知字段与元数据未知键的两侧口径

- **WHEN** 元素表写 `gatte = "strict"`，同时 `metadata` 内写 `{ unknownKey = "x" }`
- **THEN** `gatte` 被拒（`unknownField` + 位置）；`metadata.unknownKey` 被接受（元数据可自由演进）——两侧严格性不同且各自可辨

#### Scenario: docs 退役与 doc 单值三态

- **WHEN** 分别解析四份模型：一份写 `docs = ["./README.md"]`、一份省略 `doc`、一份写 `doc = "./README.md"`、一份写 `doc = ["./a.md", "./b.md"]`
- **THEN** 第一份报 `unknownField`（`docs` 已出白名单，模型停摆）；第二份 `doc = null`（字段不省略）；第三份逐字读出该字符串（`./` 由归一剥除后参与消活性判定）；第四份报 `invalidValue`（`doc` 是单值，数组形态直接拒收，MUST NOT 静默取首条）

#### Scenario: api 状态二值与缺省

- **WHEN** 解析三份模型：一份省略 `api`、一份显式 `api = "declared"`、一份 `api = "none"`
- **THEN** 三者均解析成功；省略者取缺省 `declared`；`api = "none"` 原样读出 `none`；三者 `kind` / `gate` / `path` 语义零变化

#### Scenario: api 拼写错被拒且列全合法值

- **WHEN** 元素表写 `api = "non"`，对照写 `metadata = { api = "non" }`
- **THEN** 前者报 `invalidValue`（位置 + 列出 `declared` / `none` 两值）；后者被接受且该元素的 `api` 仍为缺省 `declared`——两侧口径不同且各自可辨

#### Scenario: entry 单值可解析且缺省为 null

- **WHEN** 解析三份模型：一份省略 `entry`、一份对目录型壳 `App.Backend.Shell` 写 `entry = "./packages/desktop/src-tauri/src/lib.rs"`、一份写 `entry = ["./a.rs", "./b.rs"]`
- **THEN** 前者投影出 `entry = null`（字段不省略）；中者逐字读出该字符串；后者报 `invalidValue`（`entry` 是单值，数组形态直接拒收）

#### Scenario: 模型不含协议正文

- **WHEN** 审查任一模型文件的元素字段面
- **THEN** 元素字段只有 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 八项（`docs` 零命中）；任何签名 / 导出符号 / 接口定义写在 `<archi_models_dir>/<Domain>/<FQN>.md` 而非模型内

### Requirement: 消费面词汇集一致性对拍

四个消费面（进程内诊断集、`archi_module` 工具响应、IPC DTO、**架构会话知识文本**）SHALL 对同一份模型语料覆盖**相同的词汇集**。需全覆盖的封闭词汇：

```
element kind   package / domain / module / component
gate 档        off / warn / strict
api 状态       declared / none
结构诊断       syntaxError / missingField / invalidValue / unknownField /
               duplicateElement / cycle / dependsOnAncestor /
               unknownReference / orphanHierarchy
边标记         none / unfulfilled / crossKind / exempt
节点标记       none / unfulfilled
```

对拍 SHALL 以一份**含全部词汇**的模型语料驱动四面投影，断言四面的词汇集相等；新增或删除任一词汇而语料 / 投影未同步时对拍 MUST 变红（防「新增一个诊断类型、UI 静默不显示」）。对拍 SHALL 为机械测试。

- **「文档诊断 `docNotFound`」一行已从本词汇集删除**（本变更）：它原属 `WarningKind`（`ArchiModuleDetail.warnings` 的载体），随 `docs` 三源退役而整组消失——**词汇组数由七组减为六组**。谓词不同源：契约判据面另有 `ArchiApiFindingKind`（**十三值**，含新的 `docNotFound`）与 `ArchiApiFinding.category`（开放值集），其词汇对拍归 `desktop-archi-api-check`，MUST NOT 与模型面五组混装进本对拍（前者是「契约 vs 代码」，后者是「模型 vs 代码」）。
- **`ALL` 全量常量为唯一派生源**：`element kind` / `gate` 档 / **`api` 状态** / 结构诊断四类封闭词汇 SHALL 各自提供全量常量（`ElementKind::ALL` / `GateTier::ALL` / **`ApiStatus::ALL`** / `DiagnosticKind::ALL`；其中 `DiagnosticKind::ALL` 为九值），且 SHALL 与各自的线格式词函数（`as_str`）逐值对拍。派生型消费者（当前唯一 = 架构会话知识文本，见 `desktop-archi-agent`「架构会话的系统知识注入」）MUST 从这些常量取词，MUST NOT 在产品源码或静态文本里手抄一份词表。
- **知识文本纳入射程**：架构会话知识文本出现在四面之列——它必须含全部 `kind` / `gate` / `api` 状态 / 结构诊断词；新增词汇而知识文本未同步（因为绕过了 `ALL`）MUST 由对拍或 `ALL` 对拍变红。
- **`doc` / `entry` 不进词汇对拍**：两者的值域是任意 repo 相对路径（开放集），不是封闭词汇；知识文本只给纪律句，不给词表。

可选字段的**线形态**（缺字段 vs `null`）不在本对拍射程内；本对拍只锚定词汇集。

#### Scenario: 语料含全词汇

- **WHEN** 检查对拍语料
- **THEN** 上述每一词汇至少各一条在场（`kind` 四类元素、`gate` 三档、`api` 状态两值、九类结构诊断各一坏点、边标记四值、节点标记两值）；语料 MUST NOT 再含「文档诊断」条目（该行已出射程）

#### Scenario: 三面（诊断 / 工具 / IPC）词汇集相等

- **WHEN** 同一语料分别经进程内诊断集、`archi_module` 响应与 IPC DTO 投影
- **THEN** 三面枚举出的词汇集完全相等；任一面的缺失项使测试失败并点名缺失词汇

#### Scenario: 新增词汇未同步即红

- **WHEN** 代码新增一个诊断类型或标记值而语料 / 对拍未同步
- **THEN** 对拍失败（词汇集不等），MUST NOT 静默通过

#### Scenario: 知识文本与三面同词汇集

- **WHEN** 取架构会话知识文本（`source = "archi"` 的 `preamble` 内容）并枚举其词汇
- **THEN** 其中含 `kind` 四值、`gate` 三值（含缺省 `off`）、`api` 状态两值（`declared` / `none`）、结构诊断九词，与另三面词汇集相等；把某个词从 `ALL` 删掉而知识侧仍能拼出该词（手抄）时测试红

#### Scenario: ALL 与线格式词逐值对拍

- **WHEN** 遍历 `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / `DiagnosticKind::ALL`
- **THEN** 每一项的 `as_str` 产出唯一且与语料词一致；新增变体未补入对应 `ALL`（长度断言）时测试红

#### Scenario: 五组词汇与契约判据词汇互不混装

- **WHEN** 审查模型面五组词汇与契约判据面词汇（`ArchiApiFindingKind` 十三值 / `category` 七值）的对拍落点
- **THEN** 前者在 `desktop-archi-model` 的四面词汇对拍内（`vocabulary_test.rs`），后者在 `desktop-archi-api-check` 的词汇对拍内；两处 MUST NOT 互相引用对方的 `ALL`，`WarningKind` 在两处都不出现

### Requirement: IPC 命令面契约（八命令）

命令层 SHALL 提供八个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff 全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记；元素投影 `ArchiElement` 含 `doc` 模型拷贝，`api = declared` 者恒 `null`）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + **`doc`（单值，仅 `api = none` 有值）** + **`api`（并列字段）**；`docs` 与 `warnings` 两个位次已退役 |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（元素存在 / 值合法 / 渲染自证可解析）通过后原子写回该模型文件；失败零写入并显式 `Err` |
| `archi_drift` | `root` + `mode` | `ArchiDriftReport`：全量 / 增量两模式同源判定（`mode` = `full` \| `incremental`）；命令层零判定规则 |
| `archi_file_tree` | `root` | `ArchiFileTree`：仓库源文件与模块根的扁平条目投影（左栏文件树，契约归 `desktop-archi-file-tree`）；结构诊断非空 → `Err` |
| `archi_api_check` | `root` | `ArchiApiCheck`：四类生成物与契约渲染结果的**三类差异**（`mismatched` / `orphaned` / `missing`）+ `enabled` + `meta`；零写盘（契约归 `desktop-archi-contract-panel`） |
| `archi_api_generate` | `root` | `ArchiApiWriteOutcome`：把应生成物写盘（`written` / `unchanged` 两列）；**只增只改、从不删文件**；结构诊断非空 → `Err` 且零写盘 |

文档正文读取 SHALL 由通用 `commands::fs::read_file`（见 `desktop-file-read`）按绝对路径承担：架构域不持有专用文档读取命令，原 `archi_read_doc`（关联集成员校验 + 读盘）与关联集路径集函数 `dto::linked_doc_paths` 均已退役。

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。
- `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check` MUST NOT 写盘；写命令 SHALL 恰为两条——`archi_set_gate`（只改 `gate` 单字段）与 `archi_api_generate`（只写四类生成物，零删除）。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径；`archi_api_generate` 同理只在 `mode = incremental` 的前端调用路径出现。
- 八个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对）。命令清单与出线类型清单的机械守卫（`bindings/mod_test.rs`）SHALL 在同一 change 内随动（条数断言只作清单长度连带核对，MUST NOT 作为语义判据）——本变更使 `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 离场、`doc` 恰在**三处**进场：`ArchiModuleDetail.doc`（入口文档并列字段）、`ArchiSearchResult.doc`（路径串数组）、`ArchiElement.doc`（**授权进场字段**——模型字段的拷贝，与 `entry` 同型：单值 `Option<String>`、缺省 `null`、显式字段不省略、字段序紧跟 `entry`）；出线类型清单长度与归档断言同批更新。
- **`ArchiElement.doc` 的授权进场口径**：判据 / 纳管 / 检索的入参是 `ArchiElement` 而非模型 AST，故 `paths::carrier_path`、`api::doc_facts`、`api::gating_findings` 的 `none` 支与 `check` 的 ⑥ / ③ 两处都只能从该字段取 `doc` 事实——它是**同一事实的投影拷贝**，不是第二个事实源：与 `ArchiModuleDetail.doc` 同值同源（均拷自模型 `doc` 原文），`api = declared` 元素恒 `null`；`ArchiElementBrief` MUST NOT 带 `doc`（薄条目仍只有 `fqn` / `kind` / `gate` / `api` / `summary` 五项），`ArchiApiRef` 逐字不变。
- **`archi_module` 的 `api` 并列字段**：`ArchiModuleDetail` 的并列字段 `api`（`ArchiElement` 与薄概览条目同步带 `api` 状态），语义为「该元素的契约是否在案」——按 `<archi_models_dir>/<Domain>/<FQN>.md` 约定定位，文件不存在即 `status = none`。字段结构与定位规则定义归 `desktop-archi-api`。该字段 MUST NOT 承载协议正文。
- **`archi_module` 的 `doc` 并列字段**：`ArchiModuleDetail.doc: Option<String>` 为 repo 相对 POSIX 路径单值（无 `title` / `kind` / `source`），**只有 `api = none` 元素会填**；`api = declared` 元素的入口文档走既有 `api.contractPath`，MUST NOT 在 `doc` 里重复出线（否则抽屉会出现两条入口文档）。

#### Scenario: 读命令零写盘

- **WHEN** 依次调用 `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check`
- **THEN** 磁盘字节零变化（模型、源码、文档与四类生成物均只读）；取数结果与 `archi` 投影逐字段一致（命令层零改写）

#### Scenario: doc 单值与 docs 退役在出参可辨

- **WHEN** 分别对 `api = none` 且声明了 `doc` 的元素、`api = none` 未声明 `doc` 的元素、以及 `api = declared` 的元素调用 `archi_module`
- **THEN** 前者 `doc` 为该路径串；中者 `doc = null`（字段不省略）；后者 `doc = null` 且入口文档只经 `api.contractPath` 出线；三者响应内 MUST NOT 出现 `docs` 数组、MUST NOT 出现 `warnings` 数组

#### Scenario: 元素投影携带 doc 模型拷贝

- **WHEN** 分别取 `archi_graph` 的 `elements[]`、`archi_module` 的 `element`，以及 `archi_module` 省略 `element` 时的薄概览条目（三处都含 `api = none` 且写了 `doc` 的元素与 `api = declared` 的元素）
- **THEN** 前两者的元素对象均含 `doc` 字段：`api = none` 者为模型原文、`api = declared` 者为 `null`（字段不省略），且 `archi_graph` 与 `archi_module` 对同一元素给出同一个值；薄概览条目（`ArchiElementBrief`）零 `doc` 字段（五项不变）

#### Scenario: gate 写回原子与零残留

- **WHEN** 调用 `archi_set_gate` 改合法元素的 `gate`，以及以非法值 / 不存在元素调用
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

#### Scenario: archi_module 出参带 api 并列字段

- **WHEN** 调用 `archi_module(root, element = "App.Backend.Infra.ArchiRuntime")`（该元素契约在案）与另一无契约元素
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `<archi_models_dir>/Infra/` 下的实际文件；后者 `api.status = "none"` 且三个路径字段为 `null`；两者其余字段（元素 / 邻接 / 子元素 / `doc`）语义零变化，命令总数仍为八个

#### Scenario: 八命令在册且恰两个写面

- **WHEN** 审查 `all_commands!` 的 archi 段与 `bindings.ts` 的 typed 包装
- **THEN** archi 段恰八条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate`），每条各有对应包装；源码中含写盘调用的命令恰为 `archi_set_gate` 与 `archi_api_generate` 两条

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/model.rs` | 格式无关 AST | `Element` 字段面：`fqn` / `kind` / `gate` / `api` / `entry` / `path` / **`doc: Option<String>`** / `summary` / `metadata` / `file` / `line`（`docs` 已删）；`Relation` / `SourceSpan` / `Diagnostic` 零变化 |
| `crates/core/archi/src/parse/toml.rs` | TOML 实现 | `RawElement` 含 `doc: Option<String>`（`#[serde(default)]`）、不含 `docs`；`deny_unknown_fields` 主体不变（`docs` 写成即 `unknownField`、`doc = [..]` 归 `invalidValue`）；parser 错误 → 结构化诊断零变化 |
| `crates/core/archi/src/dto.rs` | 投影与词汇单点（**退役面 + 一处授权进场**） | `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 删除；`ArchiModuleDetail.docs` 与 `.warnings` 删、`.doc: Option<String>` 加；`ArchiSearchResult.docs: Vec<DocRef>` → `doc: Vec<String>`；`SearchScope::Docs` → `Doc`（线格式词 `doc`，`parse` 匹配臂与非法值文案同步）；`linked_doc_paths` 删；**`ArchiElement.doc: Option<String>` 加**（授权进场字段：模型字段的拷贝，与 `entry` 同型——单值、缺省 `null`、显式字段不省略、字段序紧跟 `entry`；`to_element` 逐元素拷贝，供 `paths::carrier_path` / `api::doc_facts` / `api::gating_findings` / `check` 的 ⑥ 与 ③ 取 `doc` 事实）；`ArchiElementBrief` / `ArchiApiRef` 逐字不变 |
| `crates/core/archi/src/search.rs` | 检索投影 | 语料 = `path` 非空元素的在盘入口文档载体（declared → 派生 `contractPath`、none → `doc`），条目为路径串；`matches_doc` 吃路径串 + `file_stem`（`file_stem` 从 `docs.rs` 迁入，不读正文） |
| `crates/core/archi/src/lib.rs` | 出口面 | `pub mod docs` 与 `#[cfg(test)] mod docs_test` 删；`DocKind` / `DocRef` / `DocSource` / `WarningKind` 出口删；其余出口逐字不变 |
| `src-tauri/src/commands/archi/mod.rs` | 八命令薄包装 | 签名 / 语义 / 错误面逐字不变（命令层零领域规则；文档正文经通用 `read_file`）；`archi_module_inner` 的投影仍只调 `archi::dto` |
| `src-tauri/src/bindings/mod_test.rs` | 清单守卫随动 | 出线 DTO 清单删 `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 五项；归档断言（`DocRef` 四字段）删除；清单长度连带核对随动。`ArchiElement.doc` 属**字段级**进场——`ArchiElement` 本就在清单内，清单成员不增删，只有生成物内容变化 |
| `packages/desktop/src/types/generated/bindings.ts` + `.zod.ts` | 生成物 | `bindings:export` 重导：五个类型离场、`doc` 三处进场（`ArchiModuleDetail.doc` / `ArchiSearchResult.doc` / `ArchiElement.doc`）；类型 / 字段序由生成器决定，MUST NOT 手改 |
