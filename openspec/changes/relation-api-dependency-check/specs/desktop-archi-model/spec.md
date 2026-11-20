# desktop-archi-model Specification (Delta)

## RENAMED Requirements

- FROM: `### Requirement: IPC 命令面契约（八命令）`
- TO: `### Requirement: IPC 命令面契约（九命令）`

> 改名与内容修改同一 change：命令数由八变九，标题必须随动（本能力既有纪律：「Purpose 与标题必须同一事实」——留旧标题即「同一事实两种自称」）。

## MODIFIED Requirements

### Requirement: 架构模型文件格式与字段封闭

架构模型 SHALL 落位 `<archi_dir>/graph/model/*.toml`（模型声明目录经 `foundation::layout::archi_graph_model_dir` 派生，MUST NOT 行内字面量），格式 SHALL 为 TOML「一元素一表头」：

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
# （本仓三处显式 entry 在案的实例，与本示例同形；数据纪律见 repo-archi-model）
[element."App.Backend.Shell"]
kind = "module"
path = ["./packages/desktop/src-tauri/src/"]
entry = "./packages/desktop/src-tauri/src/lib.rs"

# 无公共 API 的元素用 doc 声明入口文档（declared 元素 MUST NOT 写 doc）
[element."App.Backend.Tools.ExportBindings"]
kind = "module"
path = ["./packages/desktop/src-tauri/crates/tools/export-bindings/"]
api = "none"
doc = "./packages/desktop/src-tauri/crates/tools/export-bindings/README.md"

[[relation]]
from = "App.Auth"
to   = "App.Shared"
desc = "复用错误类型"
apis = ["ErrorCode"]        # 选填：这条依赖实际用到 `to` 元素的哪几个代码导出名
```

- **键形裁决**：`element` 表的键 SHALL 为**带引号的完整 FQN 单键**（`[element."App.Auth"]`），MUST NOT 用未引号点分形式（`[element.App.Auth]`）声明层级——两种写法并存会使同一层级存在两种拼法、歧义不可判。层级 SHALL 由 **FQN 最长前缀**推导（`App.Auth.Token` 的父亲是 `App.Auth`），MUST NOT 由 TOML 表嵌套推导。
- **字段面封闭**：元素字段 SHALL 为标量 / 数组 / **内联表**三形态；`metadata` MUST NOT 用子表（`[element."X".metadata]`）表达——子表与「子元素表头」同构，会被读成元素 `X.metadata`。
- **关系条目字段面（本变更新增第四键）**：每条 `[[relation]]` SHALL 为四键——`from` / `to` 必填，`desc` / **`apis`** 可选；`apis` SHALL 为**字符串数组**（缺省空 = 不声明 API 粒度）。`deny_unknown_fields` 语义不变：`api = ["X"]`（单数拼错）报 `unknownField`，与元素表的 `api` 字段互不干扰。
- **`apis` 的值面四条（只许名字）**：① **只许名字**——逐项 MUST NOT 含空白、MUST NOT 含 `::`、MUST NOT 表达签名 / 参数 / 类型形状，即不得是「`config_path() -> PathBuf`」或「`scan::ImportFact`」一类形态；② **非空**——逐项 MUST NOT 为空串；③ **不重复**——同一关系的 `apis` 内同名重复即拒收（MUST NOT 静默去重，静默去重会让「写重了」与「写对了」不可辨）；④ **空数组拒收**——`apis = []` 与省略同义，显式空数组报 `invalidValue`（同 `doc = [..]` / `entry = [..]` 的「显式错形态直接拒收」先例）。四条违反一律 `invalidValue` + 位置，模型整体停摆。
- **`apis` 的取值域 = `to` 元素的代码导出名**：与扫描事实（`ImportFact.names` / `DependencyEdge.names`）同一名字空间，比对 SHALL **逐字符**（MUST NOT 大小写折叠、MUST NOT 驼峰归一——否则判定侧常驻第二处映射规则）。取值域 MUST NOT 收窄为「契约 fence 声明的 op / `$defs` 名」：契约声明面是留白面（前端契约 `props = 0`），收窄即不可用；也 MUST NOT 放宽为「协议正文」（见下条）。
- **`apis` 是开放值集，不进封闭词汇对拍**：与 `doc` / `entry` 同列——它的值域是任意代码标识符（开放集），不是封闭词汇；因此四组派生 `ALL` 常量与「消费面词汇集一致性对拍」的词表 MUST NOT 因本字段扩张，知识文本也只给纪律句、不给词表。
- **值域封闭**：`kind` 必填且 SHALL 为 `package` / `domain` / `module` / `component` 四值之一；`gate` 可选且 SHALL 为 `off` / `warn` / `strict` 三值之一（缺省 `off`）；**`api`** 可选且 SHALL 为 `declared` / `none` 二值之一（缺省 `declared`）；**`entry`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组——数组会让「一个入口只属于一个模块」这条约束不可表达）；`path` SHALL 为字符串数组（缺省空）；**`doc`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组——数组会让「入口文档唯一」不可表达）；`summary` 可选字符串；`metadata` SHALL 为字符串→字符串的自由表（未知键忽略，非字符串值即 `invalidValue`）。**`docs` 已退役**：字段面 MUST NOT 含 `docs`，写了即 `unknownField`。
- **未知字段拒绝**：元素表内出现未列出的字段（如 `gatte` / `docs`）SHALL 由 `deny_unknown_fields` 拒绝（`unknownField`）。
- **属性与语义分离**：`gate` 档位 SHALL 是模型属性（本能力定义其值域与缺省）；档位的**语义消费**（参与归属、违例定级、门禁结论）归 `desktop-archi-drift` 与 `desktop-archi-gate`，本能力 MUST NOT 定义任何定级规则。
- **`api` 状态指针（显式字段，非 `metadata`）**：元素 SHALL 有封闭二值的 `api` 字段（`declared` 缺省 / `none`，附 `ApiStatus::ALL`），语义为「本模块是否有公共 API 契约」的状态指针，并新增一层角色：**它是入口文档载体的开关**——`declared` ⇒ 入口文档 = 派生落点 `<models>/<分层目录…>/<FQN>.md`（= `contract_path`，`doc` MUST NOT 再声明第二个事实）；`none` ⇒ 入口文档 = `doc` 声明（可选）。该字段 MUST NOT 承载任何协议正文（签名 / 符号 / 接口 / 规范全文）。走显式字段而非 `metadata` 自由键的理由是可获得 `deny_unknown_fields` 的拼写校验：`api = "non"` 报 `invalidValue` 并列全合法值，而 `metadata = { api = "non" }` 会静默变成「未声明」。完备性判据（每个 `path` 非空元素 ∈ {`none`} ∪ {载体在案}）归 `desktop-archi-api-check`。
- **`entry` 是实现入口指针（显式字段，非 `metadata`）**：语义为「本模块的实现入口文件」——**单值**、repo 相对 POSIX 路径（与 `path` 同口径，`./` 前缀由归一剥除、不参与身份）。四条判据（基准 / 落位 = 某条 `path` 或目录型 `path` 之内 / 存在 = 盘上必须有 / 跨元素唯一）与三态解析链（声明 → 语言约定默认 → 无入口）归 `desktop-archi-api`「生成物落点与入口查找」，本能力只定义字段本身的值域与解析面。走显式字段而非 `metadata` 自由键的理由同 `api`：`entry = ["a"]`（数组）与 `entery`（拼错）必须能机械拒收。
- **`doc` 是入口文档指针（显式字段，本变更新增）**：语义为「本模块的入口文档路径」——**单值**、repo 相对 POSIX 路径（`./` 前缀由归一剥除、不参与身份；无绝对前缀 / 无盘符 / 无 `..` / 无空段）。两条形态纪律：① **只有 `api = none` 的元素可写**（`declared` 的入口文档由派生落点给出）；② 值 MUST NOT 指向 `<archi_dir>/models/<分层目录…>/**` 与 `openspec/specs/**`。`doc` 的存活性、fence 有无与三值判据（`docMissing` / `docNotFound` / `apiDocMissing`）归 `desktop-archi-api-check`，本能力只定义字段与值域。走显式字段而非 `metadata` 的理由同 `api`：数组形态与拼错键必须能机械拒收。
- **三个指针都 MUST NOT 承载协议正文**：`entry` 是**位置**指针（文件在哪），`api` 是**状态**指针（有无公共 API），`doc` 是**入口文档**指针（读哪一份就懂这个模块）；三者都 MUST NOT 让模型开始携带签名 / 导出符号 / 接口定义。单文件根元素（`views/welcome-view.tsx` 一类）MUST NOT 声明 `entry`——它的根文件就是入口，声明即冗余；显式 `entry` 的正当用法只有「约定默认指不到」的两类：目录型壳（`src-tauri/src/` → 显式 `src/lib.rs`）与二进制 crate（只有 `src/main.rs`）。
- **协议正文不经模型**（知情边界，防误加字段）：元素上 MUST NOT 出现接口 / 导出符号 / 签名一类协议**正文**字段——唯一允许的协议相关字段是上述三个指针（`api` / `entry` / `doc`）。原边界要求的两个问题：**契约权威在哪**——在 `<archi_models_dir>/<分层目录…>/<FQN>.md`（见 `desktop-archi-api`），模型只存状态指针；**两套提取器怎么对齐**——不提两套：单一 schema + 单一生成器，`scan/rust.rs` / `scan/ts.rs` 是既有取数器（非权威），只供一次性 bootstrap。模型内再存一份协议正文必然漂移，该做法仍被否决；重提须独立变更。
- **关系 `apis` 不属协议正文（本变更的边界区分，防被当作破例）**：上条禁令的对象是**元素**上的正文，理由是「存一份正文必然漂移」。`relation.apis` 在四个维度上都与之可分——① **位置**：在关系上（**消费方**的依赖选择），不是元素上（提供方的自我描述）；② **内容**：只有名字，没有签名 / 类型形状 / 接口定义；③ **权威**：名字的权威仍在代码导出集与契约 fence，模型只回答「这条依赖选了哪几个」；④ **抗漂移**：每次检查与扫描事实机械对差（两桶见 `desktop-archi-drift`），错名不会静默。因此**元素**上的正文禁令逐字保留，关系上的**名字清单**被本条显式授权；把签名 / 类型形状 / `::` 限定路径写进 `apis` 仍按上条拒收。
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
- **THEN** 前者报 `invalidValue`（位置 + 列出 `declared` / `none` 两值）；后者被接受且该元素的 `api` 仍为缺省 `declared`（元数据可自由演进）——两侧口径不同且各自可辨

#### Scenario: entry 单值可解析且缺省为 null

- **WHEN** 解析三份模型：一份省略 `entry`、一份对目录型壳 `App.Backend.Shell`（`path = ["./packages/desktop/src-tauri/src/"]`）写 `entry = "./packages/desktop/src-tauri/src/lib.rs"`、一份写 `entry = ["./a.rs", "./b.rs"]`
- **THEN** 前者投影出 `entry = null`（字段不省略）；中者逐字读出该字符串（`./` 由归一剥除后参与判据；目录型壳的约定默认指不到，显式值正是本字段的正当用法——14 个 `src/lib.rs` 命中的 crate 与单文件根元素 MUST NOT 声明 `entry`，基线数据纪律见 `repo-archi-model`）；后者报 `invalidValue`（`entry` 是单值，数组形态直接拒收，MUST NOT 静默取首条）

#### Scenario: 关系四键解析与缺省

- **WHEN** 解析三份模型：一份 `[[relation]]` 省略 `apis`、一份写 `apis = ["AgentInput", "SessionPanel"]`、一份写 `apis = ["SessionPanel", "AgentInput"]`
- **THEN** 三者均解析成功；第一份该关系 `apis = []`（字段不省略、语义为「未声明 API 粒度」）；第二 / 第三份逐字读出各自两项（**顺序不参与身份**：值与集合语义等价，比对前统一去重 + 字典序）

#### Scenario: apis 形状五类拒收

- **WHEN** 分别写 `apis = ["config_path() -> PathBuf"]`（签名）、`apis = ["scan::ImportFact"]`（含 `::`）、`apis = [""]`（空串）、`apis = ["A", "A"]`（同名重复）、`apis = []`（显式空数组）
- **THEN** 五者各报一条 `invalidValue`（带位置；文案点名「只写名字 / 无空白 / 无 `::` / 不重复 / 空数组与省略同义」），模型整体停摆；MUST NOT 静默截断、MUST NOT 静默去重、MUST NOT 把空数组当省略接受

#### Scenario: 元素上的协议正文仍被拒而关系上的名字清单被接受

- **WHEN** 元素表写 `exports = ["parse"]`（元素级正文类字段），对照在同一模型的某条 `[[relation]]` 上写 `apis = ["parse"]`
- **THEN** 前者报 `unknownField`（元素字段面仍是八项白名单，协议正文禁令逐字有效）；后者解析成功——两处的口径差异即本条的存在理由（位置在关系、内容只有名字，权威仍在代码导出面）

#### Scenario: 模型不含协议正文

- **WHEN** 审查任一模型文件的元素字段面
- **THEN** 元素字段只有 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 八项（`docs` 零命中）；任何签名 / 导出符号 / 接口定义写在 `<archi_models_dir>/<分层目录…>/<FQN>.md` 而非模型内

### Requirement: IPC 命令面契约（九命令）

命令层 SHALL 提供九个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff、API 求差全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记；元素投影 `ArchiElement` 含 `doc` 模型拷贝，`api = declared` 者恒 `null`；关系投影 `ArchiRelation` 含 `apis` 模型拷贝）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + **`doc`（单值，仅 `api = none` 有值）** + **`api`（并列字段）**；`docs` 与 `warnings` 两个位次已退役 |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（元素存在 / 值合法 / 渲染自证可解析）通过后原子写回该模型文件；失败零写入并显式 `Err` |
| `archi_drift` | `root` + `mode` | `ArchiDriftReport`：全量 / 增量两模式同源判定（`mode` = `full` \| `incremental`）；命令层零判定规则 |
| `archi_file_tree` | `root` | `ArchiFileTree`：仓库源文件与模块根的扁平条目投影（左栏文件树，契约归 `desktop-archi-file-tree`）；结构诊断非空 → `Err` |
| `archi_api_check` | `root` | `ArchiApiCheck`：四类生成物与契约渲染结果的**三类差异**（`mismatched` / `orphaned` / `missing`）+ `enabled` + `meta`；零写盘（契约归 `desktop-archi-contract-panel`） |
| `archi_api_generate` | `root` | `ArchiApiWriteOutcome`：把应生成物写盘（`written` / `unchanged` 两列）；**只增只改、从不删文件**；结构诊断非空 → `Err` 且零写盘 |
| `archi_relation` | `root` + `from` + `to` | `ArchiRelationDetail`（本变更新增）：模型面（`from` / `to` / `description` / `mark` / `reason` / `modelFile` / `modelLine` / `declared`）+ 事实面（`used`）+ 两桶（`usedNotDeclared` / `declaredNotUsed`）+ `occurrences`；**零写盘**、命令层零判定规则（求差调 `drift` 的同一纯函数） |

文档正文读取 SHALL 由通用 `commands::fs::read_file`（见 `desktop-file-read`）按绝对路径承担：架构域不持有专用文档读取命令，原 `archi_read_doc`（关联集成员校验 + 读盘）与关联集路径集函数 `dto::linked_doc_paths` 均已退役。

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。`archi_relation` 的两个 FQN 入参 SHALL 显式给定（MUST NOT 由「当前聚焦元素」一类隐式上下文推导），元素或关系缺席即 `Err` 并点名。
- `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_relation` MUST NOT 写盘；写命令 SHALL 恰为两条——`archi_set_gate`（只改 `gate` 单字段）与 `archi_api_generate`（只写四类生成物，零删除），其余七条 MUST NOT 触碰磁盘。
- **`archi_relation` 是一次按需事实取样，不是一次检查**：它 MUST NOT 组装 / 落盘 `ArchiDriftReport`，MUST NOT 产出 `status` / `passed`，MUST NOT 缓存跨调用结果；扫描面 SHALL 限于 `from` 元素 `path` 下的源文件（机制借 `ScanInput.mode = Incremental` 的 `changed` 白名单，字面语义即「只扫其中的源文件」），看护配置经既有 `drift::config::load_config` 单点读取（MUST NOT 第二份配置解析），目标归属仍走既有 `OwnerIndex`（不解析目标）。`used` SHALL 取该次扫描 `edges` 中 **`(from, to)` 命中**的那些边的 `names` 并集（去重 + 字典序），`occurrences` 取同一批边（复用既有聚合形状）；扫描顺带产出的 `findings` / `unmatchedFiles` / `unresolved` MUST NOT 进入响应。两桶求差 MUST NOT 另写一份：与 drift 对账共用 `drift` 的同一纯函数（零第二套判定）。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（只读视图约束由前端承载、由命令可用性背书——命令本身不区分模式）；`archi_api_generate` 同理只在 `mode = incremental` 的前端调用路径出现（生成权限归 `desktop-archi-contract-panel`）；`archi_relation` 属只读取数，两处挂载点均可用（呈现归 `desktop-archi-canvas`）。
- 九个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役，`check` 链只前置重导出、不做生成物比对）。命令清单与出线类型清单的机械守卫（`bindings/mod_test.rs`）SHALL 在同一 change 内随动（条数断言只作清单长度连带核对，MUST NOT 作为语义判据）。
- **`ArchiElement.doc` 的授权进场口径**：判据 / 纳管 / 检索的入参是 `ArchiElement` 而非模型 AST，故 `paths::carrier_path`、`api::doc_facts`、`api::gating_findings` 的 `none` 支与 `check` 的两支求值都只能从该字段取 `doc` 事实——它是**同一事实的投影拷贝**，不是第二个事实源：与 `ArchiModuleDetail.doc` 同值同源（均拷自模型 `doc` 原文），`api = declared` 元素恒 `null`；`ArchiElementBrief` MUST NOT 带 `doc`（薄条目仍只有 `fqn` / `kind` / `gate` / `api` / `summary` 五项），`ArchiApiRef` 逐字不变。
- **`ArchiRelation.apis` 的授权进场口径（本变更新增）**：画布侧需要「这条边声明了哪几个名字」才能在点开前先呈现声明清单，故 `ArchiRelation` SHALL 带 `apis: Vec<String>`（模型字段的拷贝、缺省空数组、**字段不省略**）；它是**同一事实的投影拷贝**，不是第二个事实源，投影 MUST NOT 归一 / 排序以外的改写（顺序由模型原文保留，比对在 `archi` 判定侧做）。`modelFile` / `modelLine` MUST NOT 随本字段一起进 `ArchiRelation`（123 条边的图载荷不为「点开才用的两个字段」付账）——它们只在 `ArchiRelationDetail` 出线。
- **`archi_module` 的 `api` 并列字段**：`ArchiModuleDetail` 的并列字段 `api`（`ArchiElement` 与薄概览条目同步带 `api` 状态），语义为「该元素的契约是否在案」——按 `<archi_models_dir>/<分层目录…>/<FQN>.md` 约定定位（模型声明迁入 `graph/model/` 后，`models/` 即契约资产目录，契约落其 `<分层目录…>/` 子目录内），文件不存在即 `status = none`（零配置，且「有契约 / 无契约」成为可枚举事实）。字段结构与定位规则定义归 `desktop-archi-api`，本能力只登记「`archi_module` 出参形状扩展、命令层仍零领域规则」。该字段 MUST NOT 承载协议正文（契约正文与模块文档正文同一通路：由调用方经 `read_file` 自取）。
- **`archi_module` 的 `doc` 并列字段**：`ArchiModuleDetail.doc: Option<String>` 为 repo 相对 POSIX 路径单值（无 `title` / `kind` / `source`），**只有 `api = none` 元素会填**；`api = declared` 元素的入口文档走既有 `api.contractPath`，MUST NOT 在 `doc` 里重复出线（否则元素信息面板会出现两条入口文档）。

#### Scenario: 读命令零写盘

- **WHEN** 依次调用 `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_relation`
- **THEN** 磁盘字节零变化（模型、源码、文档与四类生成物均只读）；取数结果与 `archi` 投影逐字段一致（命令层零改写）

#### Scenario: doc 单值与 docs 退役在出参可辨

- **WHEN** 分别对 `api = none` 且声明了 `doc` 的元素、`api = none` 未声明 `doc` 的元素、以及 `api = declared` 的元素调用 `archi_module`
- **THEN** 前者 `doc` 为该路径串；中者 `doc = null`（字段不省略）；后者 `doc = null` 且入口文档只经 `api.contractPath` 出线；三者响应内 MUST NOT 出现 `docs` 数组、MUST NOT 出现 `warnings` 数组

#### Scenario: 元素投影携带 doc 模型拷贝

- **WHEN** 分别取 `archi_graph` 的 `elements[]`、`archi_module` 的 `element`，以及 `archi_module` 省略 `element` 时的薄概览条目（三处都含 `api = none` 且写了 `doc` 的元素与 `api = declared` 的元素）
- **THEN** 前两者的元素对象均含 `doc` 字段：`api = none` 者为模型原文、`api = declared` 者为 `null`（字段不省略），且 `archi_graph` 与 `archi_module` 对同一元素给出同一个值；薄概览条目（`ArchiElementBrief`）零 `doc` 字段（五项不变）

#### Scenario: 关系投影携带 apis 模型拷贝且不带声明行号

- **WHEN** 分别取 `archi_graph` 的 `relations[]` 与 `archi_relation(root, from, to)` 的响应（同一元素对，模型写了 `apis = ["AgentInput", "SessionPanel"]`）
- **THEN** 前者的 `ArchiRelation.apis` 逐字等于模型原文（字段不省略，空则 `[]`），且 MUST NOT 含 `modelFile` / `modelLine`；后者的 `declared` 与前者同值，且另带 `modelFile` / `modelLine`（指向该 `[[relation]]` 的 `from` 键行）

#### Scenario: gate 写回原子与零残留

- **WHEN** 调用 `archi_set_gate` 改合法元素的 `gate`，以及以非法值 / 不存在元素调用
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

#### Scenario: archi_module 出参带 api 并列字段

- **WHEN** 调用 `archi_module(root, element = "App.Backend.Infra.ArchiRuntime")`（该元素契约在案）与另一无契约元素
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `<archi_models_dir>/Infra/` 下的实际文件；后者 `api.status = "none"` 且三个路径字段为 `null`；两者其余字段（元素 / 邻接 / 子元素 / `doc`）语义零变化，命令总数仍为九个

#### Scenario: 九命令在册且恰两个写面

- **WHEN** 审查 `all_commands!` 的 archi 段与 `bindings.ts` 的 typed 包装
- **THEN** archi 段恰九条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate` / `archi_relation`），每条各有对应包装；源码中含写盘调用的命令恰为 `archi_set_gate` 与 `archi_api_generate` 两条

#### Scenario: archi_relation 与 archi_drift 同一两桶

- **WHEN** 对同一 `(from, to)` 分别取 `archi_relation` 的两桶与 `archi_drift` 报告中该元素对的条目（模型写了三项目、代码只用其中两项且多用了一项，`from.gate = warn`）
- **THEN** `archi_relation` 的 `declaredNotUsed` 恰为该一项、`usedNotDeclared` 恰为多出的那一项，与 `archi_drift` 的 `observations[].kind = unusedDeclaration` + `warnings[].kind = undeclaredApi` 逐名一致；`archi_relation` 的响应内零 `status` / `passed` 字段

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/model.rs` | 格式无关 AST | `Relation` 增 `apis: Vec<String>`（缺省空；与 `from` / `to` / `description` / `file` / `line` / `span` 并列，其余零变化）；`Element` 字段面八项零变化 |
| `crates/core/archi/src/parse/toml.rs` | TOML 实现 | `RawRelation` 增 `apis: Option<Vec<String>>`（`#[serde(default)]`）；`deny_unknown_fields` 逐字保留（`api = [..]` 报 `unknownField`）；反序列化后逐项校验（非空 / 无空白 / 无 `::` / 无重复）与 `Some(vec![])` ⇒ `invalidValue` 诊断（带位置、文案点名四条值面）；`RawElement` 零改动 |
| `crates/core/archi/src/dto.rs` | 投影 | `ArchiRelation` 增 `apis: Vec<String>`（字段不省略，`to_relation` 逐条拷贝、零归一）；新增 `ArchiRelationDetail`（`specta::Type`，线格式 camelCase）：`from` / `to` / `description` / `mark` / `reason` / `model_file` / `model_line` / `declared` / `used` / `used_not_declared` / `declared_not_used` / `occurrences`；其余既有投影零改动 |
| `crates/core/archi/src/relation.rs`（新） | 按需事实取样 + 两桶投影 | `relation_detail(root, from, to) -> Result<ArchiRelationDetail, String>`：模型面取声明与声明行；事实面定向扫描（`ScanInput { mode: Incremental, changed: <from 侧源文件白名单>, .. }`，目标归属走 `OwnerIndex`）；求差调 `drift::api_buckets`（MUST NOT 自写第二份）；零写盘、零缓存、零 `Report` 组装 |
| `crates/core/archi/src/lib.rs` | 出口面 | `pub mod relation`（+ `#[cfg(test)] mod relation_test`）与 `relation_detail` 出口；既有出口逐字不变 |
| `src-tauri/src/commands/archi/mod.rs` | **九命令**薄包装 | 新增 `archi_relation(root, from, to)` + `archi_relation_inner`（root 校验 → 调 `archi::relation::relation_detail` → 错误映射）；文件头「八命令」→「九命令」；既有八条零改动 |
| `src-tauri/src/commands/mod.rs` | 命令登记面 | `all_commands!` 的 archi 段 8 → 9 条（`archi_relation` 追加在 `archi_api_generate` 之后、`read_file` 之前；builder 与 `generate_handler!` 同源） |
| `src-tauri/src/bindings/mod_test.rs` | 清单守卫随动 | 命令名 / 包装名清单 56 → 57、出线 DTO 清单 104 → 105、`ARCHI_COMMANDS` 8 → 9、`ARCHI_DTO_TYPES` 21 → 22（`ArchiRelationDetail` 按字典序插在 `ArchiRelation` 之后）；尾部追加序断言把 `archi_relation` 插在 `archi_api_generate` 之后 |
| `crates/core/archi/tests/vocabulary_test.rs` | 词汇对拍（零扩张） | 六组封闭词汇逐字不变；`apis` 与 `doc` / `entry` 同列——开放值集，不进任何 `ALL` 与词表 |
| 本能力 `## Purpose` | 口径与标题同源 | Purpose SHALL 写「与九命令 IPC 契约」，MUST NOT 自称「八命令」——标题、Purpose 与本表三处同批体现 `archi_relation` 的追加 |

### API: `src-tauri/src/commands/mod.rs` + `src-tauri/src/commands/archi/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `all_commands!` archi 段 | 8 → 9 条 | 追加 `commands::archi::archi_relation`（位于 `archi_api_generate` 之后、`read_file` 之前；`read_file` 仍为尾条）；builder 注册与 `generate_handler!` 同源 |
| `commands/archi/mod.rs` | 改 | 文件头「八命令」→「九命令」；新增一条命令 + 一个 `*_inner`；既有八条零改动 |
| `commands/archi/mod_test.rs` | 改 | root 校验用例由八命令扩到九命令；新命令的 `_inner` 一致性、零写盘与「同一 (from, to) 与 drift 同两桶」用例 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| `crates/core/archi/src/dto.rs` 既有投影 | `ArchiGraph` / `ArchiModuleDetail` / `ArchiApiRef` / `ArchiFileTree` 等逐字不变（本能力只加 `ArchiRelation.apis` 与一个新区块 DTO） |
| 前端调用路径 | `archi_set_gate` 仍只在 `incremental` 路径出现；`archi_api_generate` 的调用面归 `desktop-archi-contract-panel`；`archi_relation` 的呈现归 `desktop-archi-canvas` |
