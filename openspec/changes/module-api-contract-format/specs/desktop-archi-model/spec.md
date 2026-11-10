# desktop-archi-model Specification（delta: module-api-contract-format）

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
docs = ["./src/auth/README.md"]
metadata = { owner = "platform" }

[[relation]]
from = "App.Auth"
to   = "App.Shared"
desc = "复用错误类型"
```

- **键形裁决**：`element` 表的键 SHALL 为**带引号的完整 FQN 单键**（`[element."App.Auth"]`），MUST NOT 用未引号点分形式（`[element.App.Auth]`）声明层级——两种写法并存会使同一层级存在两种拼法、歧义不可判。层级 SHALL 由 **FQN 最长前缀**推导（`App.Auth.Token` 的父亲是 `App.Auth`），MUST NOT 由 TOML 表嵌套推导。
- **字段面封闭**：元素字段 SHALL 为标量 / 数组 / **内联表**三形态；`metadata` MUST NOT 用子表（`[element."X".metadata]`）表达——子表与「子元素表头」同构，会被读成元素 `X.metadata`。
- **值域封闭**：`kind` 必填且 SHALL 为 `package` / `domain` / `module` / `component` 四值之一；`gate` 可选且 SHALL 为 `off` / `warn` / `strict` 三值之一（缺省 `off`）；**`api`** 可选且 SHALL 为 `declared` / `none` 二值之一（缺省 `declared`）；`path` / `docs` SHALL 为字符串数组（缺省空）；`summary` 可选字符串；`metadata` SHALL 为字符串→字符串的自由表（未知键忽略，非字符串值即 `invalidValue`）。
- **未知字段拒绝**：元素表内出现未列出的字段（如 `gatte`）SHALL 由 `deny_unknown_fields` 拒绝（`unknownField`）。
- **属性与语义分离**：`gate` 档位 SHALL 是模型属性（本能力定义其值域与缺省）；档位的**语义消费**（参与归属、违例定级、门禁结论）归 `desktop-archi-drift` 与 `desktop-archi-gate`，本能力 MUST NOT 定义任何定级规则。
- **`api` 状态指针（显式字段，非 `metadata`）**：元素 SHALL 有封闭二值的 `api` 字段（`declared` 缺省 / `none`，附 `ApiStatus::ALL`），语义为「本模块是否有公共 API 契约」的状态指针，MUST NOT 承载任何协议正文（签名 / 符号 / 接口 / 规范全文）。走显式字段而非 `metadata` 自由键的理由是可获得 `deny_unknown_fields` 的拼写校验：`api = "non"` 报 `invalidValue` 并列全合法值，而 `metadata = { api = "non" }` 会静默变成「未声明」。完备性判据：每个 `path` 非空元素 ∈ {`none`} ∪ {契约文件存在}，不存在「忘了建契约」这第三种状态（判据归 `desktop-archi-api-check`）。
- **协议正文不经模型**（知情边界，防误加字段，本变更改写为状态指针口径）：元素上 MUST NOT 出现接口 / 导出符号 / 签名一类协议**正文**字段——唯一允许的协议相关字段是上述状态指针 `api`。原边界要求的两个问题，本变更正面回答：**契约权威在哪**——在 `<archi_dir>/api/<FQN>.md`（见 `desktop-archi-api`），模型只存状态指针；**两套提取器怎么对齐**——不提两套：单一 schema + 单一生成器，`scan/rust.rs` / `scan/ts.rs` 是既有取数器（非权威），只供一次性 bootstrap。模型内再存一份协议正文必然漂移，该做法仍被否决；重提须独立变更。
- 新元素缺省 `gate = "off"`（ratchet：存量全 `off`，想收紧的模块逐个改 `strict`，一个 change 收紧一块）。

#### Scenario: 四值 kind 与三值 gate 解析成功

- **WHEN** 以合法模型（含 `package` / `domain` / `module` / `component` 四类元素与 `off` / `warn` / `strict` 三档 `gate`，其中至少一个元素省略 `gate`）
- **THEN** 解析成功；省略 `gate` 的元素取 `off`；每个元素的 `path` / `docs` 缺省为空数组、`metadata` 缺省为空表

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

#### Scenario: api 状态二值与缺省

- **WHEN** 解析三份模型：一份省略 `api`、一份显式 `api = "declared"`、一份 `api = "none"`
- **THEN** 三者均解析成功；省略者取缺省 `declared`；`api = "none"` 原样读出 `none`；三者 `kind` / `gate` / `path` 语义零变化

#### Scenario: api 拼写错被拒且列全合法值

- **WHEN** 元素表写 `api = "non"`，对照写 `metadata = { api = "non" }`
- **THEN** 前者报 `invalidValue`（位置 + 列出 `declared` / `none` 两值）；后者被接受且该元素的 `api` 仍为缺省 `declared`（元数据可自由演进）——两侧口径不同且各自可辨

#### Scenario: 模型不含协议正文

- **WHEN** 审查任一模型文件的元素字段面
- **THEN** 元素字段只有 `kind` / `gate` / `api` / `path` / `docs` / `summary` / `metadata` 七项；任何签名 / 导出符号 / 接口定义写在 `<archi_dir>/api/<FQN>.md` 而非模型内

### Requirement: 消费面词汇集一致性对拍

四个消费面（进程内诊断集、`archi_module` 工具响应、IPC DTO、**架构会话知识文本**）SHALL 对同一份模型语料覆盖**相同的词汇集**。需全覆盖的封闭词汇：

```
element kind   package / domain / module / component
gate 档        off / warn / strict
api 状态       declared / none
结构诊断       syntaxError / missingField / invalidValue / unknownField /
               duplicateElement / cycle / dependsOnAncestor /
               unknownReference / orphanHierarchy
文档诊断       docNotFound
边标记         none / unfulfilled / crossKind / exempt
节点标记       none / unfulfilled
```

对拍 SHALL 以一份**含全部词汇**的模型语料驱动四面投影，断言四面的词汇集相等；新增或删除任一词汇而语料 / 投影未同步时对拍 MUST 变红（防「新增一个诊断类型、UI 静默不显示」）。对拍 SHALL 为机械测试（形态沿用 `src/views/changes/flow/run-state.test.ts` 的全词汇出图先例：一份语料走全投影、逐词汇断言在场）。

- **`ALL` 全量常量为唯一派生源**：`element kind` / `gate` 档 / **`api` 状态** / 结构诊断四类封闭词汇 SHALL 各自提供全量常量（`ElementKind::ALL` / `GateTier::ALL` / **`ApiStatus::ALL`** / `DiagnosticKind::ALL`；其中 `DiagnosticKind::ALL` 为九值），且 SHALL 与各自的线格式词函数（`as_str`）逐值对拍。`ApiStatus` 两值（`declared` / `none`）由本变更新增，知识文本与四面投影 MUST 从该常量取词。派生型消费者（当前唯一 = 架构会话知识文本，见 `desktop-archi-agent`「架构会话的系统知识注入」）MUST 从这些常量取词，MUST NOT 在产品源码或静态文本里手抄一份词表。
- **知识文本纳入射程**：架构会话知识文本出现在四面之列——它必须含全部 `kind` / `gate` / `api` 状态 / 结构诊断词；新增词汇而知识文本未同步（因为绕过了 `ALL`）MUST 由对拍或 `ALL` 对拍变红。

可选字段的**线形态**（缺字段 vs `null`）不在本对拍射程内（见 proposal R-3）；本对拍只锚定词汇集。

#### Scenario: 语料含全词汇

- **WHEN** 检查对拍语料
- **THEN** 上述每一词汇至少各一条在场（`kind` 四类元素、`gate` 三档、`api` 状态两值、九类结构诊断各一坏点、`docNotFound` 一例、边标记四值、节点标记两值）

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

#### Scenario: api 状态词汇四面在场

- **WHEN** 检查对拍语料与四面投影（进程内诊断集 / `archi_module` 响应 / IPC DTO / 知识文本）
- **THEN** 四面均含 `declared` 与 `none` 两词（语料含一个 `api = declared` 元素与一个 `api = none` 元素）；删掉 `ApiStatus::ALL` 的一项而知识侧仍能拼出该词时测试红

### Requirement: IPC 命令面契约（五命令）

命令层 SHALL 提供五个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff 全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + `docs` + **`api`（并列字段）** |
| `archi_read_doc` | `root` + `path` | 文档正文：仅接受**关联集内**路径（服务端现算关联集后校验），越界显式 `Err` |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（元素存在 / 值合法 / 渲染自证可解析）通过后原子写回该模型文件；失败零写入并显式 `Err` |

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。
- `archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary` MUST NOT 写盘；唯一写命令 SHALL 为 `archi_set_gate`（且只改 `gate` 单字段）。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（只读视图约束由前端承载、由命令可用性背书——命令本身不区分模式）。
- 五个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役，`check` 链只前置重导出、不做生成物比对）。
- **`archi_module` 的 `api` 并列字段**：本变更为 `ArchiModuleDetail` 增加并列字段 `api`（`ArchiElement` 与薄概览条目同步带 `api` 状态），语义为「该元素的契约是否在案」——按 `<archi_api_dir>/<Domain>/<FQN>.md` 约定定位，文件不存在即 `status = none`（零配置，且「有契约 / 无契约」成为可枚举事实）。字段结构与定位规则定义归 `desktop-archi-api`，本能力只登记「命令数量仍为五个、`archi_module` 出参形状扩展、命令层仍零领域规则」。该字段 MUST NOT 承载协议正文（契约正文的读取通路不在本期；`archi_read_doc` 只接受文档关联集内的路径，不覆盖 `<archi_dir>/api/**`——本期不新增读取命令）。

#### Scenario: 读命令零写盘

- **WHEN** 依次调用 `archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary`
- **THEN** 磁盘字节零变化（模型与文档均只读）；取数结果与 `archi` 投影逐字段一致（命令层零改写）

#### Scenario: 正文取数越界拒绝

- **WHEN** 以关联集之外的路径调用 `archi_read_doc`（如 `openspec/changes/x/proposal.md`）
- **THEN** 返回显式 `Err`，不返回任何文件内容

#### Scenario: gate 写回原子与零残留

- **WHEN** 调用 `archi_set_gate` 改合法元素的 `gate`，以及以非法值 / 不存在元素调用
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

#### Scenario: archi_module 出参带 api 并列字段

- **WHEN** 调用 `archi_module(root, element = "App.Backend.Infra.ArchiRuntime")`（该元素契约在案）与另一无契约元素
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向实际文件；后者 `api.status = "none"` 且三个路径字段为 `null`；两者其余字段（元素 / 邻接 / 子元素 / `docs`）语义零变化，命令总数仍为五个

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/archi/src/model.rs` | 元素模型（改） | `Element` 新增 `api: ApiStatus`（`#[serde(default)]` 语义面 = 缺省 `Declared`）；其余字段名 / 顺序 / 语义零变化 |
| `crates/core/archi/src/parse/toml.rs` | 模型解析（改） | `RawElement` 新增 `api` 字段（`#[serde(default)]`），落在既有 `deny_unknown_fields` 面内 ⇒ `api = "non"` 报 `invalidValue` 并列全合法值；MUST NOT 走 `metadata` 自由键 |
| `crates/core/archi/src/dto.rs` | DTO 投影（改） | 新增 `ApiStatus`（`declared` / `none`，`ALL` + `as_str` + `specta::Type`）与 `ArchiApiRef { status, contractPath, schemaPath, generatedPath }`；`ArchiElement.api` / `ArchiElementBrief.api` / `ArchiModuleDetail.api` 出线；路径由 `archi_api_dir` 派生（定义归 `desktop-archi-api`） |
| `crates/core/archi/src/model_test.rs` + `parse/toml_test.rs` | 模型语料（扩展） | 两态取值 / 缺省 `declared` / 拼写错报 `invalidValue` 并列全合法值 / 与 `metadata.api` 的口径差（静默变「未声明」的反例在案） |
| `crates/core/archi/tests/vocabulary_test.rs` | 词汇对拍（扩展） | `ApiStatus::ALL` 二值与 `as_str` 逐值对拍；封闭词汇组新增 `api` 一面；知识文本含 `declared` / `none` |
| `crates/infra/agent/src/archi_knowledge.rs` + `archi-knowledge.md` | 知识文本（改） | 契约词汇面注入 `api`（`declared` / `none`）与契约目录入口；注入矩阵与 policy 零改动 |
