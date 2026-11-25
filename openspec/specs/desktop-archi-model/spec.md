# desktop-archi-model Specification

## Purpose

定义架构模型的格式与解析契约：`<archi_dir>/graph/model/*.toml` 的「一元素一表头」格式与字段封闭（含 `doc` 单值入口文档指针）、多文件聚合与声明位置回填、结构诊断九类分工（parser 免费组 / archi 自实现组）、`ModelGraph` 单点构建与三消费面裁剪型投影、消费面词汇集对拍（三面：进程内诊断 / 工具响应 / IPC DTO）、gate 写回纯函数、模型结构 diff 与九命令（含两条契约面命令）IPC 契约。

## Requirements

### Requirement: 架构模型文件格式与字段封闭

架构模型 SHALL 落位 `<archi_dir>/graph/model/*.toml`（模型声明目录经 `foundation::layout::archi_graph_model_dir` 派生，MUST NOT 行内字面量），格式 SHALL 为 TOML「一元素一表头」：

```toml
[element."Backend"]
kind = "package"
path = ["./packages/desktop/src-tauri/"]

[element."Backend.Core.Auth"]
kind = "domain"
path = ["./src/auth/", "./src/auth-legacy/"]
gate = "strict"
summary = "认证与令牌签发"
metadata = { owner = "platform" }

[element."Backend.Core.Auth.Store"]
kind = "module"
path = ["./src/auth/store/"]

# 目录型壳的约定默认拼成 .../src-tauri/src/src/lib.rs（指不到），故此处显式 entry 是正当用法
# （本仓三处显式 entry 在案的实例，与本示例同形；数据纪律见 repo-archi-model）
[element."Backend.Shell"]
kind = "module"
path = ["./packages/desktop/src-tauri/src/"]
entry = "./packages/desktop/src-tauri/src/lib.rs"

# 无公共 API 的元素用 doc 声明入口文档（declared 元素 MUST NOT 写 doc）
[element."Backend.Tools.ExportBindings"]
kind = "package"
path = ["./packages/desktop/src-tauri/crates/tools/export-bindings/"]
api = "none"
doc = "./packages/desktop/src-tauri/crates/tools/export-bindings/README.md"

[[relation]]
from = "Backend.Core.Auth.Store"
to   = "Backend.Core.Auth"
desc = "复用错误类型"
apis = ["ErrorCode"]        # 选填：这条依赖实际用到 `to` 元素的哪几个代码导出名
```

- **键形裁决**：`element` 表的键 SHALL 为**带引号的完整 FQN 单键**（`[element."Backend.Core.Auth"]`），MUST NOT 用未引号点分形式（`[element.Backend.Core.Auth]`）声明层级——两种写法并存会使同一层级存在两种拼法、歧义不可判。层级 SHALL 由 **FQN 最长前缀**推导（`Backend.Core.Auth.Store` 的父亲是 `Backend.Core.Auth`），MUST NOT 由 TOML 表嵌套推导。
- **字段面封闭**：元素字段 SHALL 为标量 / 数组 / **表**三形态；`metadata` 的内联表（`metadata = { … }`）与其子表形态（`[element."X".metadata]`，写在已声明元素之后）**语义等价**——TOML 表语义，两种形态都读出同一 `metadata` 表、零诊断。元素身份只由表头键决定：两种形态都 MUST NOT 产出名为 `X.metadata` 的元素。子表形态若成为该 FQN 的**唯一**声明（元素表只有 `metadata`、缺必填的 `kind`）即 `missingField`，整份源全或无。**MUST NOT 再表述为「子表会被读成另一个元素」**——该因果不成立（行为锚：`parse/toml_test.rs` 的 `metadata子表与内联表等价` / `metadata子表成为唯一声明即缺kind`）。
- **关系条目字段面（本变更新增第四键）**：每条 `[[relation]]` SHALL 为四键——`from` / `to` 必填，`desc` / **`apis`** 可选；`apis` SHALL 为**字符串数组**（缺省空 = 不声明 API 粒度）。`deny_unknown_fields` 语义不变：`api = ["X"]`（单数拼错）报 `unknownField`，与元素表的 `api` 字段互不干扰。
- **`apis` 的值面四条（只许名字）**：① **只许名字**——逐项 MUST NOT 含空白、MUST NOT 含 `::`、MUST NOT 表达签名 / 参数 / 类型形状，即不得是「`config_path() -> PathBuf`」或「`scan::ImportFact`」一类形态；② **非空**——逐项 MUST NOT 为空串；③ **不重复**——同一关系的 `apis` 内同名重复即拒收（MUST NOT 静默去重，静默去重会让「写重了」与「写对了」不可辨）；④ **空数组拒收**——`apis = []` 与省略同义，显式空数组报 `invalidValue`（同 `doc = [..]` / `entry = [..]` 的「显式错形态直接拒收」先例）。四条违反一律 `invalidValue` + 位置，模型整体停摆。
- **`apis` 的取值域 = `to` 元素的代码导出名**：与扫描事实（`ImportFact.names` / `DependencyEdge.names`）同一名字空间，比对 SHALL **逐字符**（MUST NOT 大小写折叠、MUST NOT 驼峰归一——否则判定侧常驻第二处映射规则）。取值域 MUST NOT 收窄为「契约 fence 声明的 op / `$defs` 名」：契约声明面是留白面（前端契约 `props = 0`），收窄即不可用；也 MUST NOT 放宽为「协议正文」（见下条）。
- **`apis` 是开放值集，不进封闭词汇对拍**：与 `doc` / `entry` 同列——它的值域是任意代码标识符（开放集），不是封闭词汇；因此四组派生 `ALL` 常量与「消费面词汇集一致性对拍」的词表 MUST NOT 因本字段扩张，知识文本也只给纪律句、不给词表。
- **值域封闭**：`kind` 必填且 SHALL 为 `package` / `domain` / `module` / `composite` 四值之一（类型层级从大到小：包 > 领域 > 模块，`domain` 层可省略；`composite` 无实际层级，仅把过多的同级元素分类，可嵌套在任意类型之上）；`gate` 可选且 SHALL 为 `off` / `warn` / `strict` 三值之一（缺省 `off`）；**`api`** 可选且 SHALL 为 `declared` / `none` 二值之一（缺省 `declared`）；**`entry`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组——数组会让「一个入口只属于一个模块」这条约束不可表达）；`path` SHALL 为字符串数组（缺省空；基数随 `kind`，见下条）；**`doc`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组——数组会让「入口文档唯一」不可表达）；`summary` 可选字符串；`metadata` SHALL 为字符串→字符串的自由表（未知键忽略，非字符串值即 `invalidValue`）。**`docs` 已退役**：字段面 MUST NOT 含 `docs`，写了即 `unknownField`。
- **`path` 的基数是 `kind` 的属性（`root` 与 `paths` 两种语义在此点名）**：`package` 是**一个包**（最小编译单元 / 独立依赖），`path` SHALL **至多 1 条**——0 条合法（纯容器，不对应任何目录），写 1 条即该包的**包根**目录；≥2 条 SHALL 报 `invalidValue`，诊断位置指向**第 2 条** `path` 的源码位置（比元素表头更准），message 点名「一个包至多一条包根目录（纯容器可留空）——多条 path 属 `module`」，模型经 `graph_view` **整图停摆**（既有全或无语义，MUST NOT 改成「跳过该元素」）。`module` 的 `path` 是**若干根的清单**，不设上界（目录型 / 单文件 / 多文件装配同形）；`domain` / `composite` 的 `path` 选填且不设上界。本条只守 `package` 的上界，MUST NOT 顺手扩张出 `module ≥ 1`、目录尾 `/` 一类形态校验；容错面为零是已知代价（`package` 写两条 `path` 在契约面本就只有 `path[0]` 可见）。
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

- **WHEN** 以合法模型（含 `package` / `domain` / `module` / `composite` 四类元素与 `off` / `warn` / `strict` 三档 `gate`，其中至少一个元素省略 `gate`）
- **THEN** 解析成功；省略 `gate` 的元素取 `off`；每个元素的 `path` 缺省为空数组、`metadata` 缺省为空表、`entry` 与 `doc` 缺省为 `null`

#### Scenario: 非法 kind 与非法 gate 被拒并列全合法值

- **WHEN** 模型写 `kind = "pakage"` 或 `gate = "stirct"`
- **THEN** 解析返回 `invalidValue` 诊断，`message` 含 parser 原文（`line` / `column` / 源码片段 / `^^^^` 指示）并**列出全部合法值**（`package` / `domain` / `module` / `composite` 或 `off` / `warn` / `strict`），模型整体判定为停摆态

#### Scenario: metadata 子表与内联表等价

- **WHEN** 两份模型只在 metadata 形态上不同：一份写内联表 `metadata = { owner = "platform", tier = "core" }`，另一份在带 `kind` 的 `[element."Backend.Core.Auth"]` 之后写子表 `[element."Backend.Core.Auth".metadata]` + 同两项
- **THEN** 两份都解析成功、零诊断；`Backend.Core.Auth` 的 `metadata` 键值逐项相同；元素集里 MUST NOT 出现名为 `Backend.Core.Auth.metadata` 的元素

#### Scenario: metadata 子表成为唯一声明即缺 kind

- **WHEN** 某 FQN 只以子表形态出现（`[element."Backend.Core.Auth".metadata]`，没有带 `kind` 的 `[element."Backend.Core.Auth"]`）
- **THEN** 报 `missingField`（该 FQN 缺 `kind`），整份源全或无——MUST NOT 静默产出半个模型，MUST NOT 把该表读成另一个元素

#### Scenario: 未引号点分形式被拒

- **WHEN** 模型以 `[element.Backend.Core.Auth]`（未引号点分）声明层级
- **THEN** 报 `unknownField`：文案点名非法子键 `Auth`（不在 `RawElement` 字段面内），整份源解析失败、不产出任何元素；MUST NOT 与 `[element."Backend.Core.Auth"]` 合并为同一元素

#### Scenario: package 两条 path 被拒且整图停摆

- **WHEN** 某 `kind = "package"` 元素写两条 `path`（`path = ["./a/", "./b/"]`）
- **THEN** 解析返回一条 `invalidValue`，位置命中**第 2 条** `path` 所在行，message 含「至多一条包根」与合法形态提示；此后 `graph_view` 返回 `elements = []` / `relations = []`（整图停摆），MUST NOT 只丢该元素、MUST NOT 保留其余元素

#### Scenario: package 零条 path 与其余 kind 的多条 path 合法

- **WHEN** 解析一份模型：`package` 元素省略 `path`（纯容器），`module` 元素写 5 条 `path`（多根清单），`domain` 元素写 2 条 `path`，`composite` 元素写 2 条 `path`
- **THEN** 四者全部解析成功、零诊断；`package` 的 `path` 为空数组——判据只在 `package` 的**上界**上成立，`module` / `domain` / `composite` 的基数 MUST NOT 被校验

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

- **WHEN** 解析三份模型：一份省略 `entry`、一份对目录型壳 `Backend.Shell`（`path = ["./packages/desktop/src-tauri/src/"]`）写 `entry = "./packages/desktop/src-tauri/src/lib.rs"`、一份写 `entry = ["./a.rs", "./b.rs"]`
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

### Requirement: 多文件聚合与声明定位

解析层 SHALL 聚合 `<archi_dir>/graph/model/` 下的**一级** `*.toml`（不递归），按文件名**字典序**读取并合并为一个 `Model`。每个元素与每条关系 SHALL 携带声明位置：

- `modelFile` = 声明所在文件的 **repo 相对路径**（`/` 分隔）；
- `modelLine` = 元素表头键所在行（1 起始）或关系条目 `from` 键所在行。

位置 SHALL 取自 `toml` + serde 反序列化同时产出的 span（`span` → 行列换算），MUST NOT 以再扫文本、模糊匹配或二次 grep 反推。关系级 span SHALL 保留可选位（`Option<SourceSpan>`），当前只消费行号、不消费列区间。

- **收面判据 SHALL 恰为「一级文件 + `.toml` 后缀」两条**（`read_dir` 逐条 `is_file()` + 后缀匹配，MUST NOT 递归、MUST NOT 按目录名或文件名前缀（`NN-`）白名单筛）：模型声明目录 `graph/model/` 只住模型文件，契约资产在另一目录 `models/<分层目录…>/`（`*.md` / `*.schema.json`），两者按目录分家，互不进对方收面。收面 MUST NOT 因同目录多出住户而扩张或收窄；`models/` 一级出现 `.toml` MUST NOT 被当模型解析（反向判据在案）。
- **索引 MUST NOT 落 `graph/model/` 根**：模型收面的 `.toml` 后缀会把 `<archi_dir>/graph/model/INDEX.toml` 当模型文件解析，其 `[[entry]]` 表在 `deny_unknown_fields` 下产 `unknownField` 诊断并使模型停摆；契约索引的合法落点是 `<archi_graph_dir>/INDEX.toml`（`desktop-archi-api`「schema.json 与 INDEX.toml 为纯派生且不可手改」）。本能力 MUST NOT 为兼容该摆放而放宽收面判据或为 `INDEX.toml` 开字段白名单。

**跨文件重复 FQN**：同一 FQN 在一个文件内重复声明由 TOML map 语义天然拒绝；**跨文件**重复 SHALL 由聚合层自实现检出，诊断 `kind = duplicateElement`，`file` / `line` SHALL 指向**后出现**的那条声明（字典序靠后的文件内位置）。聚合层 MUST NOT 静默取其一。

`graph/model/` 目录缺席或为空 SHALL 解析为**空模型**（零元素零关系零诊断），MUST NOT 报错——空模型是冷启动的合法起点。目录存在而其一级零 `*.toml` SHALL 按同一口径解析为空模型：零诊断、零元素，MUST NOT 因「目录非空」判为异常；`models/`（契约资产目录）下的 `.toml` MUST NOT 进入收面。

#### Scenario: 多文件字典序聚合

- **WHEN** `graph/model/` 内含 `02-auth.toml` 与 `01-core.toml`（各声明不同元素，`02` 引用 `01` 的元素）
- **THEN** 聚合结果含两文件的全部元素与关系，字典序决定聚合次序（`01` 先于 `02`），跨文件 `unknownReference` 不误报；每条元素 / 关系的 `modelFile` 指向各自文件

#### Scenario: 声明行号可点击

- **WHEN** 查询任一元素的 `modelFile` + `modelLine`
- **THEN** 该文件该行为 `[element."<fqn>"]` 表头；查询任一关系的 `modelLine` 为该 `[[relation]]` 条目 `from` 键所在行

#### Scenario: 跨文件重复 FQN 被拒

- **WHEN** `01-core.toml` 与 `03-billing.toml` 都声明 `[element."Backend.Core.Auth"]`
- **THEN** 聚合返回 `duplicateElement` 诊断，`file` 为字典序靠后的那个文件、`line` 为其中的声明行；模型判定为停摆态，MUST NOT 静默保留任一份

#### Scenario: 同文件重复 key 由 parser 拒绝

- **WHEN** 单文件内重复声明 `[element."Backend.Core.Auth"]`
- **THEN** TOML 层直接拒绝（`duplicate key` + 位置 + 指向冲突 key 的 `^^^`），诊断映射为 `duplicateElement`，无需聚合层介入

#### Scenario: 空模型合法

- **WHEN** `<archi_dir>/graph/model/` 不存在或为空目录
- **THEN** 返回空模型（零元素 / 零关系 / 零诊断），画布呈现空态而非停摆态，冷启动可继续

#### Scenario: 模型声明目录与契约资产目录分家零干扰

- **WHEN** `<archi_dir>/graph/model/` 一级为六个 `NN-<域>.toml`，`<archi_dir>/models/` 另有分层目录 `Backend/Core/<FQN>.md`、`Backend/Core/<FQN>.schema.json` 与 `Frontend/Components/Agent/<FQN>.md` 三级子目录，跑一次解析
- **THEN** 元素 / 关系计数与只放六个 toml 时逐字相同、`diagnostics` 空、`modelFile` 仍指向 `graph/model/` 的六个文件；删掉全部六个 toml 后（契约目录保留）解析为空模型且零诊断；反向在 `models/` 一级放一份合法 `.toml` 亦 MUST NOT 进模型收面——两例 MUST NOT 出现「把契约目录里的文件当模型」或「因另一目录存在而报错」

### Requirement: 结构诊断九类分工与不 panic 纪律

模型诊断 SHALL 为结构化值（`kind` + `message` + `file` + `line` + 可选 `column`），按来源分两组：

```
parser / serde 免费（文案与 caret 不由本项目撰写）
├── syntaxError      漏引号 / 漏闭合 ] / 非法 TOML
├── missingField     缺 kind
├── invalidValue     kind / gate 非法值（列出全部合法值）
├── unknownField     gatte 之类拼错的键
└── duplicateElement 同文件内重复 key（+ 跨文件重复由聚合层补上）

archi 自实现（图规则）
├── cycle            成环（SCC 输出环路径）
├── dependsOnAncestor 元素依赖自己的祖先
├── unknownReference 关系引用不存在的元素
└── orphanHierarchy  子元素声明了而祖先未声明（层级取自 FQN 前缀的必然后果）
```

- **免费组 MUST NOT 自研**：五类诊断的文案、`line` / `column`、源码片段与 caret 指示 SHALL 原样沿用 parser 输出（自写只会更差且是纯工量浪费）。
- **自实现组 MUST 带可定位信息**：四类 SHALL 各给出 `file` + `line`（`cycle` 另 SHALL 在 `message` 内列出环上的 FQN 路径；`unknownReference` 另 SHALL 指明引用方与被引用的缺失 FQN）。
- **判定完整覆盖**：`orphanHierarchy` SHALL 在祖先 FQN 无对应元素声明时触发（隐式祖先 MUST NOT 被自动创建——「声明必须显式存在」是本能力的立身前提）。
- **绝不 panic**：解析 / 校验全过程 MUST NOT `panic` / `unwrap` 于输入数据（生产路径）；坏模型的表现恒为「返回诊断 + 停摆态」。
- **单次加载单次诊断**：一次读取 SHALL 返回全部诊断（不做首个错误短路），使错误面板一次列全。

#### Scenario: 环被检出并给出路径

- **WHEN** 模型声明 `App.A -> App.B` 与 `App.B -> App.A`
- **THEN** 返回 `cycle` 诊断，`message` 含环上 FQN 路径（`App.A -> App.B -> App.A` 形态）与声明位置

#### Scenario: 依赖祖先被检出

- **WHEN** `Backend.Core.Auth.Store` 依赖自己的祖先 `Backend.Core.Auth`（`from = "Backend.Core.Auth.Store"`、`to = "Backend.Core.Auth"`）
- **THEN** 返回 `dependsOnAncestor` 诊断并指明元素与祖先

#### Scenario: 未定义引用被检出

- **WHEN** 模型声明 `from = "Backend.Core.Auth.Store"`、`to = "Backend.Core.Auth.Shared"` 而无 `[element."Backend.Core.Auth.Shared"]`
- **THEN** 返回 `unknownReference` 诊断，指明引用方 `Backend.Core.Auth.Store` 与缺失目标 `Backend.Core.Auth.Shared`

#### Scenario: 孤儿层级被检出

- **WHEN** 模型只声明 `[element."Backend.Core.Auth.Store"]` 而无 `Backend` / `Backend.Core` / `Backend.Core.Auth` 声明
- **THEN** 返回 `orphanHierarchy` 诊断（`file` / `line` 指向该声明），MUST NOT 自动补建隐式祖先节点

#### Scenario: 一次列全不短路

- **WHEN** 模型同时含环、未定义引用与孤儿层级三种错
- **THEN** 诊断列表同时含三类（不做首个错误短路），错误面板可一次列全

#### Scenario: 坏模型不 panic

- **WHEN** 以语法错、非法枚举值、缺必填字段等各类坏模型调用读取面
- **THEN** 均返回结构化诊断且进程不 panic、不中断调用方；诊断数量与坏点数一一对应

### Requirement: ModelGraph 单点构建与投影零改写

图 SHALL 在 `archi` 内**只构建一次**（`ModelGraph`，邻接 / 包含两类边由 petgraph 承载），三个消费面（进程内结构校验、`archi_module` 工具响应、IPC DTO）MUST NOT 各自再建图、再算环、再推祖先。

消费面 MUST 只做**裁剪型投影**（1-hop 邻域 / 字段子集 / 分页），MUST NOT 重算、MUST NOT 推导、MUST NOT 补默认值。投影 SHALL 由 `archi::dto` 单点提供，命令层与 agent 工具执行体 MUST NOT 自行拼装响应 JSON。

邻域查询 SHALL 提供三项：`dependsOn`（出边）、`dependedBy`（入边）、`children`（直接子元素，按 FQN 前缀一级）；画布的 1-hop 投影 SHALL 由 `ModelGraph` 的邻域查询组合而成（聚焦元素的入边 + 出边 + 直接子元素），MUST NOT 在投影层做二次图遍历求可达性。

#### Scenario: 三个消费面同源

- **WHEN** 同一份含环的模型分别经进程内校验、`archi_module` 响应与 IPC DTO 输出
- **THEN** 三者对「该模型有环」的结论一致；工具响应与 DTO 的元素 / 关系条数、`modelFile` / `modelLine` 逐一相同

#### Scenario: 投影不补默认值

- **WHEN** 某元素无 `path`、无 `doc`、无 `summary`
- **THEN** 投影输出为空数组 / `null` 的显式字段（不省略字段、不填占位字符串），三面形状一致

#### Scenario: 1-hop 由邻域查询组合

- **WHEN** 以 `Backend.Core.Auth` 为聚焦元素请求画布数据
- **THEN** 返回集中于该元素的出边、入边与直接子元素；非邻域元素不出现在图数据内（计数可机械验证），且该集合由 `ModelGraph` 邻域查询直接组合

### Requirement: 消费面词汇集一致性对拍

三个消费面（进程内诊断集、`archi_module` 工具响应、IPC DTO）SHALL 对同一份模型语料覆盖**相同的词汇集**。需全覆盖的封闭词汇：

```
element kind   package / domain / module / composite
gate 档        off / warn / strict
api 状态       declared / none
结构诊断       syntaxError / missingField / invalidValue / unknownField /
               duplicateElement / cycle / dependsOnAncestor /
               unknownReference / orphanHierarchy
边标记         none / unfulfilled / crossKind / exempt
节点标记       none / unfulfilled
```

对拍 SHALL 以一份**含全部词汇**的模型语料驱动三面投影，断言三面的词汇集相等；新增或删除任一词汇而语料 / 投影未同步时对拍 MUST 变红（防「新增一个诊断类型、UI 静默不显示」）。对拍 SHALL 为机械测试（形态沿用 `src/views/changes/flow/run-state.test.ts` 的全词汇出图先例：一份语料走全投影、逐词汇断言在场）。

- **「文档诊断 `docNotFound`」一行已从本词汇集删除**：它原属 `WarningKind`（`ArchiModuleDetail.warnings` 的载体），随 `docs` 三源退役而整组消失——**词汇组数由七组减为六组**。谓词不同源：契约判据面另有 `ArchiApiFindingKind`（**十三值**，含新的 `docNotFound`）与 `ArchiApiFinding.category`（开放值集），其词汇对拍归 `desktop-archi-api-check`，MUST NOT 与模型面五组混装进本对拍（前者是「契约 vs 代码」，后者是「模型 vs 代码」）。
- **`ALL` 全量常量为唯一派生源**：`element kind` / `gate` 档 / **`api` 状态** / 结构诊断四类封闭词汇 SHALL 各自提供全量常量（`ElementKind::ALL`（四值）/ `GateTier::ALL` / **`ApiStatus::ALL`** / `DiagnosticKind::ALL`；其中 `DiagnosticKind::ALL` 为九值），且 SHALL 与各自的线格式词函数（`as_str`）逐值对拍。上述常量是**对拍源**：架构会话知识文本（见 `desktop-archi-agent`「架构会话的系统知识注入」）的呈现源是 `archi-knowledge.md`——`kind` 由「元素类型」语义 bullet 承载，`gate` / `api` 由「字段纪律」各一行承载，测试遍历 `ALL` / `default()` 与 md **同序相等**对拍；结构诊断全量词表不进知识文本（工具按需返回注解与修复手段）。产品源码 MUST NOT 另手抄词表（md 是唯一静态呈现面，由测试机械守护）。
- **知识文本的收窄射程**：架构会话知识文本只对 `kind` / `gate` / `api` 承担呈现（值 + 语义 + 缺省档），**不是**结构诊断词的词汇消费面；新增 `gate` / `api` 值而 md 未同步、或 md 多出 `ALL` 之外的值 / 改序时，md ↔ `ALL` 对拍 MUST 变红。
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

#### Scenario: 知识文本只承载字段值域与可执行词

- **WHEN** 取架构会话知识文本（`source = "archi"` 的 `preamble` 内容）并检查其中的封闭词汇
- **THEN** 含 `kind` 四值语义（bullet 逐值）、`gate` 三值与缺省档、`api` 两值与缺省档；结构诊断全量词表**不得出现**（`syntaxError` / `missingField` / `invalidValue` / `unknownField` / `duplicateElement` / `cycle` / `unknownReference` 零命中），只允许 `orphanHierarchy` / `dependsOnAncestor` 两个可执行词在散文里；知识文本不属于 `DiagnosticKind::ALL` 的词汇消费面

#### Scenario: ALL 与线格式词逐值对拍

- **WHEN** 遍历 `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / `DiagnosticKind::ALL`
- **THEN** 每一项的 `as_str` 产出唯一且与语料词一致；新增变体未补入对应 `ALL`（长度断言）时测试红

#### Scenario: api 状态词汇三面在场

- **WHEN** 检查对拍语料与三面投影（进程内诊断集 / `archi_module` 响应 / IPC DTO）
- **THEN** 三面均含 `declared` 与 `none` 两词（语料含一个 `api = declared` 元素与一个 `api = none` 元素）；删掉 `ApiStatus::ALL` 的一项而 md 仍写该词时，md ↔ `ALL` 对拍红

#### Scenario: 五组词汇与契约判据词汇互不混装

- **WHEN** 审查模型面五组词汇与契约判据面词汇（`ArchiApiFindingKind` 十三值 / `category` 七值）的对拍落点
- **THEN** 前者在 `desktop-archi-model` 的三面词汇对拍内（`vocabulary_test.rs`），后者在 `desktop-archi-api-check` 的词汇对拍内；两处 MUST NOT 互相引用对方的 `ALL`，`WarningKind` 在两处都不出现

### Requirement: 模型写回原语族（元素 / 关系 upsert 与 delete）

`archi` SHALL 提供**模型写回原语族**：纯函数层 `apply_*`（源文本进、改写后的完整源文本出），计划层 `plan_*`（读盘定位 + 纯函数 + 待落盘产物 + 反馈面）。gate 改档是其中 `ElementSpec { gate }` 的一个退化调用——MUST NOT 保留第二套 gate 专用入口（`set_gate` / `plan_set_gate` / `GateWrite` 均已出射程）：

- **八函数两面**：`apply_element_spec` / `apply_element_delete` / `apply_relation_upsert` / `apply_relation_delete` 为纯函数（零文件系统触碰）；`plan_element_upsert` / `plan_element_delete` / `plan_relation_upsert` / `plan_relation_delete` 为计划函数（`parse_dir` 结构诊断前置 → 存在性前置 → 读源 → `apply_*` → 填 `hash` → 装反馈面），**零写盘**。写盘单点仍是 `write_atomic`（同目录临时文件 + rename）：壳层命令面与 agent 写工具 SHALL 共用同一实现，MUST NOT 各自复制一份。**删除侧只有一个纯函数形态**：`apply_element_delete(source, &DeleteSet)`（`DeleteSet` = 待删元素 FQN 集 + 待删关系对集），单元素删除是「集合恰一个元素」的退化形态——MUST NOT 保留第二个「只删一个元素」的删除实现。
- **产出形状**：`plan_*` SHALL 返回 `WriteOutcome`——`write: ModelWrite { file, content, hash }`（P3 三元组逐字不变）+ `warnings: Vec<EditWarning>` + `changed: Vec<String>` + `created: bool` + `relations: RelationTally { created, updated, deleted, unchanged }` + `counts: { elements, relations }`（本文件重解析）+ `model: { elements, relations }`（全模型）。`changed` SHALL 为**参数面字段名的子集**（实际变化者；空 = 幂等）；`relations` 四计数之和 SHALL 恒等于本次 `relations` 入参长度（未给则四项皆 0）；`counts` SHALL 等于写后该文件的重解析计数（表头回填的同一来源）；`model` SHALL 等于前值 ± 本次增量（元素 ±1 或 −子树规模、关系 ±added/−deleted），可对拍写后 `drift` 报告的 `model.elements` / `model.relations`。
- **渲染纪律（基线三条逐字保留）**：渲染 SHALL 经 `toml_edit`（解析 → 改 AST → 渲染），MUST NOT 字符串正则替换；改写后注释、空行与其余元素的原始排版 SHALL 原样保留；**既有键序 MUST NOT 重排**——新键追加到元素表尾，新建元素的表按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序整体写出（MUST NOT 为「重排既有元素的键」引入第二份字段序常量）。
- **三态语义**：可清空位（`entry` / `doc` / `summary` / `path` / `metadata` / 关系 `desc` / 关系 `apis`）SHALL 以 `Option<Option<T>>`（或「`Some` 空集合」的同构形态）表达三种意图——`None` = 不动、`Some(None)` = 删键（`path` / `metadata` / `apis` 以 `Some(空集合)` 表达同一意图）、`Some(Some(v))` = 设值。封闭枚举位（`kind` / `gate` / `api`）无清空臂：`Some(GateTier::Off)` 是**显式化缺省**（ratchet 的正当动作），不是清空。MUST NOT 用 `Option<T>` 混过三态，使「不动」与「清空」不可辨。
- **内联出边（本变更新增）**：`ElementSpec.relations: Vec<RelationSpec>`（`{ to, desc, apis, action }`）SHALL 在一次元素写回里批量处理出边，四条约束逐条成立——① **同一 `to` 在一批里只出现一次**（重复即 `Err`，否则「先 upsert 后 delete」取决于数组序，语义不可判定；错误定位用 **1 基下标**且与 `action` / `to` 同时出现）；② `action = delete` 项 MUST NOT 带 `desc` / `apis`（带了即 `Err`——删除动作与字段赋值互斥，静默忽略会让调用方以为赋值生效）；③ **内联边只作用于目标文件**：某条内联边已声明在**别的**模型文件 ⇒ `Err` 点名该文件（一次 plan 只写一个文件，MUST NOT 静默只改同文件那份）；④ **未列出的既有边保持不动**（全量替换会一次漏列即静默删依赖）。内联 `delete` 与独立 `apply_relation_delete` SHALL 共用同一判据与同一文案族（MUST NOT 为内联路径另写「不存在就静默跳过」的宽松版）。
- **自证对象 = 交付文本**：产出顺序 SHALL 固定为「改 AST → 渲染 → 表头计数回填 → 重解析自证零诊断 → 回读逐字段一致」；重解析出任何诊断、或回读值与规格不一致即 `Err` 且零文本产出——MUST NOT 把坏文本交给调用方落盘，MUST NOT 先验后改（被验的文本必须逐字等于交付的 `content`）。
- **表头计数回填**：目标文件**首部注释块**内的第一处 `（<数字> 元素 / <数字> 关系）`，括号内两个数字 SHALL 按该文件的重解析结果回填；括号外（人工标题与散文）逐字保留；源中无该括号 MUST NOT 新增，首部注释块之外的同形文本 MUST NOT 改动。计数的唯一权威是重解析结果——MUST NOT 另行统计源码行数或表头条目数。
- **乐观锁 `expect`**：`plan_*` SHALL 接受可选的 `expect`（目标文件**当前**内容哈希，`sha256:…` 形态）并在读源之后、渲染之前校验：不匹配即 `Err` 且零文本产出（目标文件缺席而给了 `expect` 同判不匹配；`None` = 不校验，不存在「期望文件缺席」这一臂）。`ModelWrite.hash` SHALL 为**写后**内容哈希，与 `drift::file_hash` 同口径（`ModelWrite.hash` 逐字等于把 `content` 落盘后的 `file_hash(file)`），使调用方能把它直接作为下一次 `expect`。三条写面（画布改档 + 两个 agent 写工具）共用同一校验点，MUST NOT 出现绕过 `expect` 的写面分支。
- **落点（本变更改写）**：既有元素 SHALL 落其声明所在文件（`ElementSpec.file` 给了不同值即 `Err`——迁文件属独立语义）。新建元素的落点按序：① 给了 `file` ⇒ 落该文件（repo 相对、`archi_graph_model_dir` 一级 `*.toml`），且目标文件**不存在**时 MUST 同时给 `create = true`（否则 `Err` 并提示——防拼错名字造出幽灵文件）；② 省略 `file` ⇒ 落**父元素**声明所在文件（祖先须显式声明是既有前置，故「最近已声明祖先」即父元素）；③ 单段 FQN（无父）且省略 `file` ⇒ `Err` 点名需给 `file`（新分层的落点必是人的判断）。MUST NOT 引入「按 `kind` / 首段 FQN 推落点」的约定（文件名里的语义序号工具无法推导，且本仓同文件跨分层元素的反例无法用规则表达）。新建时 `kind` 必填；父元素缺席（会造出 `orphanHierarchy`）、目标 FQN 已在案、落点逃出模型声明目录、模型声明目录缺席 ⇒ 一律 `Err`；写面 MUST NOT 代建目录。删除与关系两侧另有「元素必须存在」前置：缺席即 `Err` 并附现有 FQN 清单或模型元素总数。**新建元素的 FQN 形态**另有一道写面卫生前置：MUST NOT 含空白 / `"` / `[` / `]` / `#` / 换行（写面要把该串渲染成键，含这些字符会产出一个转义后不可读的键）——该前置只作用于**新建**，既有元素的 FQN 形态仍只由 parser 判，写面 MUST NOT 借此对既有值重做形态校验。
- **删除与级联（本变更改写）**：`plan_element_delete(root, fqn, cascade, expect)`——`cascade = false`（缺省）时「该元素仍有已声明子元素」或「仍被任一关系引用（出边或入边）」即 `Err` 并点名阻塞者（P3 行为逐字保留）；`cascade = true` 时 SHALL 先算**闭包**（子树 ∪ 出入边），再按文件分组：闭包全在目标文件 ⇒ 删净（子树按 FQN 前缀、关系按 `(from, to)` 对）；闭包**跨文件** ⇒ `Err` 点名跨向文件与阻塞项且零文本产出。**`cascade` 的承诺面就是「同一模型文件内」**——MUST NOT 被表述为跨模型文件的级联（一次 plan = 一个 `ModelWrite`、一次原子写；多文件事务无解，拒绝并点名优于半成功）。
- **关系落点**：新关系 SHALL 落 `from` 元素声明所在文件；`from` / `to` 端点元素缺席即 `Err` 并点名缺失端；既有 `(from, to)` 走更新（改 `desc` / `apis`）而非重复追加；模型面同一 `(from, to)` 多处声明即 fail-closed（既有裁定逐字保留）。
- **值域与形态判据归既有单点**：`kind` / `gate` / `api` 的非法值由封闭枚举 + IPC 反序列化拒绝（列全合法值的文案归 serde），写面 MUST NOT 重写一份值域表；`path` / `apis` / `entry` / `doc` 的形态仍由 parser 判（写面 MUST NOT 重写第二份形态判据），越界值经自证链以结构化诊断落地并令写回失败。
- **错误面**：所有失败 SHALL 为封闭枚举 `EditErrorKind`（附 `ALL`，本变更由 19 值扩为 **23** 值：新增「内联 `delete` 项带字段」「同批重复 `to`」「级联闭包跨文件」「内联边声明在他处」四条）+ 事实段组成的 `EditError`，`message()` 形态恒为「判据句：当前事实 + 下一步」；壳层与工具面经 `EditError::message()` 映射进既有 `Result<T, String>` 错误面。MUST NOT 出现裸 `String` 错误的第二路径；`ALL` 与 `criterion()` 的 `match` 分支集合 SHALL 逐值相等（新增变体未同批补 `ALL` 即测试红）。
- 该族 MUST NOT 触碰文件系统的**写**动作（读盘归 `plan_*`，写盘归消费面）；`archi` 内 MUST NOT 出现写盘调用。判定域既有边界不变：零 spawn、零 Tauri、零 store、零 `workflow` 依赖。

#### Scenario: 改档保留注释与其余内容

- **WHEN** 对含注释、空行与多个元素的源文本把 `Backend.Core.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 返回文本中该元素 `gate = "strict"`，其余元素、注释、空行与键序逐字保留（唯一例外：首部注释块的 `（N 元素 / M 关系）` 计数括号按重解析结果回填）；再解析该文本得到新档位且零诊断

#### Scenario: 元素缺席分两路

- **WHEN** 以不存在的 FQN 且不给 `kind` 调 `plan_element_upsert`；另以不存在的 FQN 调 `plan_element_delete`
- **THEN** 前者 `Err`（判据句点名新建缺 `kind`，事实段点名该 FQN 不在案——缺席在 upsert 里是**新建分支**，不是错误本身）；后者 `Err`（判据句点名元素不存在，事实段列出模型现有 FQN 清单或元素总数）；两者零文本产出

#### Scenario: 渲染不可解析或回读不一致即不产出

- **WHEN** 渲染路径产出的文本无法被解析器读回，或回读到的字段值与规格不一致
- **THEN** 返回 `Err` 且不产出任何待落盘文本（自证不可解析即失败、回读不一致即失败），调用方零写入

#### Scenario: 三态逐位可辨

- **WHEN** 对同一元素分三次调用 `plan_element_upsert`，依次传 `summary = None`、`summary = Some(None)`、`summary = Some(Some("一句话"))`
- **THEN** 第一次该键字节不变；第二次该键从源中消失；第三次写入新值；`path` / `metadata` 以 `Some(空集合)` 表达清空（回读为空数组 / 空表、键不在源）；`gate = Some(GateTier::Off)` 写显式 `gate = "off"` 行而非删键

#### Scenario: 新建元素按父元素落点与规范字段序

- **WHEN** 对一个缺席 FQN（父元素已在案）省略 `file` 调 `plan_element_upsert`；另给 `file`（模型声明目录一级、文件已存在）与 `file`（文件不存在 + `create = true`）各调一次；再对一个单段 FQN 省略 `file` 调一次
- **THEN** 前者落父元素所在文件、新增一张 `[element."<FQN>"]` 表、字段按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序写出；第二个改既有文件；第三个在 `file` 不存在且无 `create` 时 `Err` 并提示 `create = true`，给了 `create` 则新建文件且表头为 `# <名>（1 元素 / 0 关系）`；第四个 `Err` 点名需给 `file`。缺 `kind`、父元素缺席、FQN 已在案、落点逃出模型声明目录、模型声明目录缺席、FQN 含空白 / 引号 / 方括号 / `#` / 换行——各返回 `Err` 并点名成因

#### Scenario: 内联出边四条约束

- **WHEN** 一次 `plan_element_upsert` 传入 `relations = [{to: A}, {to: B, desc: Some(None)}, {to: C, action: delete}]`；另构造同批重复 `to`、`delete` 项带 `desc`、内联边已声明在另一文件的三种违例
- **THEN** 前者建 A、清 B 的 `desc`、删 C，未列出的既有边零改动，`relations` 四计数之和 = 3；三种违例分别 `Err`（重复项以 1 基下标 + `action` + `to` 定位；带字段项点名互斥；跨文件项点名该文件），全部零文本产出

#### Scenario: 级联闭包只走同一文件

- **WHEN** 对「子树与出入边全在同一文件」与「某个子元素或某条边在另一文件」两种情形分别调 `cascade = true` 的删除
- **THEN** 前者删净闭包（子树 + 出入边 + 表头计数随动），其余内容逐字保留；后者 `Err` 点名跨向文件与阻塞项且零文本产出；`cascade = false` 的两种拒绝臂（有子 / 被引用）逐字保留

#### Scenario: 反馈面四组可对拍

- **WHEN** 对同一元素以完全相同的参数连调两次 upsert，并在第一次之后取 `counts` / `model` / `changed` / `relations`
- **THEN** 第一次 `created = true`、`changed` 覆盖给予的字段、`counts` 与该文件重解析计数及表头括号数字三者相等、`model` 与写后 `drift` 报告的 `model.elements` / `model.relations` 相等；第二次 `created = false`、`changed = []`、`content` 与源逐字节相等

#### Scenario: 乐观锁不匹配零文本产出

- **WHEN** 以过期的 `expect`（目标文件在取哈希之后已被改动）调用任一 `plan_*`，随后以 `file_hash` 取到的当前值重试
- **THEN** 前者 `Err`（判据句点名期望与实际的摘要），零文本产出；后者 `Ok`，且 `ModelWrite.hash` 与 `content` 落盘后 `file_hash(file)` 的返回值逐字相等

#### Scenario: 表头计数由重解析回填

- **WHEN** 在表头写 `（9 元素 / 10 关系）` 而文件内实为 8 元素 10 关系的临时模型上做一次元素删除；另对计数准确的本仓模型做一次「把 `gate` 改成其当前值」的写回
- **THEN** 前者产出文本的表头变为 `（8 元素 / 10 关系）`、括号外整行逐字不变；后者产出与源**逐字节相等**（计数准确 ⇒ 回填为零差异）；源无该括号时不新增，首部注释块之外的同形文本零改动

#### Scenario: 删除 fail-closed

- **WHEN** 分别对「仍有已声明子元素」「仍被关系引用（出边或入边）」「无从属叶元素」三种元素调 `plan_element_delete(cascade = false)`
- **THEN** 前两者 `Err`：事实段点名阻塞的子元素 FQN / 关系对；后者成功删净该元素表，其余元素与注释逐字保留、该文件表头计数随动下降

#### Scenario: 关系 upsert 三态与落点

- **WHEN** 对既有 `(from, to)` 调 `plan_relation_upsert` 改 `desc`；对缺席 `(from, to)` 调一次新增；再以 `apis = Some(&[])` 清空该项
- **THEN** 更新情形关系条数不变且 `desc` 为新值；新增情形落在 `from` 元素声明所在文件（回读 `modelFile` 命中）；清空情形 `apis` 键从源中消失（MUST NOT 写成 `apis = []`）；端点元素缺席 ⇒ `Err` 点名缺失端

#### Scenario: 结构错模型拒绝全部写回

- **WHEN** 模型含任一结构诊断（如 `cycle`）时依次调用四个 `plan_*`
- **THEN** 四次全 `Err`，判据句为「模型存在结构诊断，拒绝写回」、事实段列出诊断种类，零文本产出

### Requirement: 模型结构 diff

`archi` SHALL 提供两模型的结构 diff 纯函数，输出：元素新增 / 删除计数与 FQN 清单、关系新增 / 删除计数、`gate` 变更清单（FQN + 旧值 + 新值）。diff SHALL 以 FQN（元素）与 `(from, to)`（关系）为身份键，MUST NOT 以文件或行号为身份。

diff 的消费面为归档确认面的「架构变更」区块（数据源经命令面取数）；两侧模型的**取自哪个工作根**不由本函数决定（主仓基线 vs worktree 的取数归命令层）。

#### Scenario: 增删与改档计数

- **WHEN** 对基线与新模型（新增 `Backend.Core.Billing` / `Backend.Core.Billing.Invoice`、删除 `Backend.Core.Legacy`、关系 +3 −1、`Backend.Core.Billing` 的 `gate` 由 `off` 改为 `strict`）求 diff
- **THEN** 输出元素 +2 / −1（含 FQN 清单）、关系 +3 / −1、`gate` 变更一条（`Backend.Core.Billing`：`off` → `strict`）

#### Scenario: 等价模型零差异

- **WHEN** 两侧模型元素 / 关系 / 档位完全相同（文件划分或行序不同）
- **THEN** diff 输出全零（元素 / 关系 / `gate` 三组均空），MUST NOT 因文件或行序差异报告变更

### Requirement: IPC 命令面契约（九命令）

命令层 SHALL 提供九个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff、API 求差全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记；元素投影 `ArchiElement` 含 `doc` 模型拷贝，`api = declared` 者恒 `null`；关系投影 `ArchiRelation` 含 `apis` 模型拷贝）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + **`doc`（单值，仅 `api = none` 有值）** + **`api`（并列字段）** + **`fileHash`（声明文件单文件内容哈希）**；`docs` 与 `warnings` 两个位次已退役 |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（结构诊断非空即拒 / 元素存在 / 渲染自证可解析 / 回读一致）通过后原子写回该模型文件，并顺带把该文件首部注释的「（N 元素 / M 关系）」计数括号按重解析结果回填；失败零写入并显式 `Err` |
| `archi_drift` | `root` + `mode` | `ArchiDriftReport`：全量 / 增量两模式同源判定（`mode` = `full` \| `incremental`）；命令层零判定规则 |
| `archi_file_tree` | `root` | `ArchiFileTree`：仓库源文件与模块根的扁平条目投影（左栏文件树，契约归 `desktop-archi-file-tree`）；结构诊断非空 → `Err` |
| `archi_api_check` | `root` | `ArchiApiCheck`：四类生成物与契约渲染结果的**三类差异**（`mismatched` / `orphaned` / `missing`）+ `enabled` + `meta`；零写盘（契约归 `desktop-archi-contract-panel`） |
| `archi_api_generate` | `root` | `ArchiApiWriteOutcome`：把应生成物写盘（`written` / `unchanged` 两列）；**只增只改、从不删文件**；结构诊断非空 → `Err` 且零写盘 |
| `archi_relation` | `root` + `from` + `to` | `ArchiRelationDetail`：模型面（`from` / `to` / `description` / `mark` / `reason` / `modelFile` / `modelLine` / `declared`）+ 事实面（`used`）+ 两桶（`usedNotDeclared` / `declaredNotUsed`）+ `occurrences`；**零写盘**、命令层零判定规则（求差调 `drift` 的同一纯函数） |

文档正文读取 SHALL 由通用 `commands::fs::read_file`（见 `desktop-file-read`）按绝对路径承担：架构域不持有专用文档读取命令，原 `archi_read_doc`（关联集成员校验 + 读盘）与关联集路径集函数 `dto::linked_doc_paths` 均已退役。

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。`archi_relation` 的两个 FQN 入参 SHALL 显式给定（MUST NOT 由「当前聚焦元素」一类隐式上下文推导），元素或关系缺席即 `Err` 并点名。
- `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_relation` MUST NOT 写盘；写命令 SHALL 恰为两条——`archi_set_gate`（只改 `gate` 单字段 + 该文件表头注释的计数括号回填，零其它字段改写）与 `archi_api_generate`（只写四类生成物，零删除），其余七条 MUST NOT 触碰磁盘。
- **`archi_module` 的 `fileHash` 并列字段（本变更新增）**：`ArchiModuleDetail.fileHash: String`（`sha256:<hex>`）SHALL 为「该元素声明文件的当前内容哈希」，与 `drift::file_hash` **同一函数产出**（命令层零新增计算逻辑，投影在 `dto::module_detail` 内完成）。它是 agent 写工具 `expect` 的**读侧闭环**——同一字段名、同一函数、同一前缀；MUST NOT 与 `archi_drift` 报告的 `modelHash` 比较（后者是全量聚合，非同粒度），MUST NOT 承担「模型是否被改动」的判定（那是 `assets_hash` 的用途）。薄概览条目 MUST NOT 携带该字段。
- **`archi_set_gate` 与写工具同实现（本变更明确）**：该命令 SHALL 走与 agent 写工具**同一** `plan_element_upsert`（取 `WriteOutcome.write` 落盘），产出的文本 SHALL 与 `archi_element(gate = …)` 逐字节相同；命令面 MUST NOT 暴露 `expect`（前端单会话、无并发写场景，暴露只增 IPC 面）——该省略 MUST NOT 被读作「命令面绕过乐观锁校验」（校验点在同一 `plan_*` 入参，`None` 即不校验）。
- `archi_relation` 是一次按需事实取样，不是一次检查：它 MUST NOT 组装 / 落盘 `ArchiDriftReport`，MUST NOT 产出 `status` / `passed`，MUST NOT 缓存跨调用结果；扫描面 SHALL 限于 `from` 元素 `path` 下的源文件（机制借 `ScanInput.mode = Incremental` 的 `changed` 白名单，字面语义即「只扫其中的源文件」），看护配置经既有 `drift::config::load_config` 单点读取（MUST NOT 第二份配置解析），目标归属仍走既有 `OwnerIndex`（不解析目标）。`used` SHALL 取该次扫描 `edges` 中 **`(from, to)` 命中**的那些边的 `names` 并集（去重 + 字典序），`occurrences` 取同一批边（复用既有聚合形状）；扫描顺带产出的 `findings` / `unmatchedFiles` / `unresolved` MUST NOT 进入响应。两桶求差 MUST NOT 另写一份：与 drift 对账共用 `drift` 的同一纯函数（零第二套判定）。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（只读视图约束由前端承载、由命令可用性背书——命令本身不区分模式）；`archi_api_generate` 同理只在 `mode = incremental` 的前端调用路径出现（生成权限归 `desktop-archi-contract-panel`）；`archi_relation` 属只读取数，两处挂载点均可用（呈现归 `desktop-archi-canvas`）。
- 九个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役，`check` 链只前置重导出、不做生成物比对）。命令清单与出线类型清单的机械守卫（`bindings/mod_test.rs`）SHALL 在同一 change 内随动（条数断言只作清单长度连带核对，MUST NOT 作为语义判据）。本变更**不新增命令**：命令清单仍九条，diff 只来自 `ArchiModuleDetail` 的新字段。
- **`ArchiElement.doc` 的授权进场口径**：判据 / 纳管 / 检索的入参是 `ArchiElement` 而非模型 AST，故 `paths::carrier_path`、`api::doc_facts`、`api::gating_findings` 的 `none` 支与 `check` 的两支求值都只能从该字段取 `doc` 事实——它是**同一事实的投影拷贝**，不是第二个事实源：与 `ArchiModuleDetail.doc` 同值同源（均拷自模型 `doc` 原文），`api = declared` 元素恒 `null`；`ArchiElementBrief` MUST NOT 带 `doc`（薄条目仍只有 `fqn` / `kind` / `gate` / `api` / `summary` 五项），`ArchiApiRef` 逐字不变。
- **`ArchiRelation.apis` 的授权进场口径**：画布侧需要「这条边声明了哪几个名字」才能在点开前先呈现声明清单，故 `ArchiRelation` SHALL 带 `apis: Vec<String>`（模型字段的拷贝、缺省空数组、**字段不省略**）；它是**同一事实的投影拷贝**，不是第二个事实源，投影 MUST NOT 归一 / 排序以外的改写（顺序由模型原文保留，比对在 `archi` 判定侧做）。`modelFile` / `modelLine` MUST NOT 随本字段一起进 `ArchiRelation`（123 条边的图载荷不为「点开才用的两个字段」付账）——它们只在 `ArchiRelationDetail` 出线。
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
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留（唯一例外：首部注释块的「（N 元素 / M 关系）」计数括号按重解析结果回填，计数本就准确时零差异）；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

#### Scenario: archi_module 出参带 api 并列字段

- **WHEN** 调用 `archi_module(root, element = "Backend.Infra.ArchiRuntime")`（该元素契约在案）与另一无契约元素
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `<archi_models_dir>/Infra/` 下的实际文件；后者 `api.status = "none"` 且三个路径字段为 `null`；两者其余字段（元素 / 邻接 / 子元素 / `doc` / `fileHash`）语义按本条要求，命令总数仍为九个

#### Scenario: fileHash 与写工具一致

- **WHEN** 对同一元素分别取 `archi_module` 的 `fileHash` 与 `drift::file_hash(该元素声明文件)`
- **THEN** 两者逐字相等（同函数同前缀）；把 `archi_drift` 报告的 `modelHash` 与之比较则不等（不同粒度，MUST NOT 互比）；命令层零新增哈希实现

#### Scenario: 九命令在册且恰两个写面

- **WHEN** 审查 `all_commands!` 的 archi 段与 `bindings.ts` 的 typed 包装
- **THEN** archi 段恰九条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate` / `archi_relation`），每条各有对应包装；源码中含写盘调用的命令恰为 `archi_set_gate` 与 `archi_api_generate` 两条；`bindings.ts` 的 diff 恰为 `ArchiModuleDetail.fileHash` 一字段

#### Scenario: archi_relation 与 archi_drift 同一两桶

- **WHEN** 对同一 `(from, to)` 分别取 `archi_relation` 的两桶与 `archi_drift` 报告中该元素对的条目（模型写了三项目、代码只用其中两项且多用了一项，`from.gate = warn`）
- **THEN** `archi_relation` 的 `declaredNotUsed` 恰为该一项、`usedNotDeclared` 恰为多出的那一项，与 `archi_drift` 的 `observations[].kind = unusedDeclaration` + `warnings[].kind = undeclaredApi` 逐名一致；`archi_relation` 的响应内零 `status` / `passed` 字段

### Requirement: 元素字段面的机器可读清单

`crates/core/archi/src/parse/toml.rs` SHALL 在 `RawElement` 定义旁定义 `pub const ELEMENT_FIELD_NAMES: [&str; 8]`（顺序 = 字段声明序：`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`），并经 `parse/mod.rs` 再导出为 `archi::parse::ELEMENT_FIELD_NAMES`（沿 `TomlParser` 的既有重导出形态）。

- **单点不是第二份事实源**：常量是字段白名单的**机器可读副本**，权威仍是 `RawElement` 的字段定义。两者一致性 SHALL 由同 crate 内的源码自证测试机械对拍（`include_str!("toml.rs")` 取 `struct RawElement { … }` 块内的字段名，滤掉 `///` 与 `#[…]` 行），断言**同序同集**——新增 / 改名 / 删除字段而常量未同批更新即红（范式同 `model_test.rs` 的 `element字段数恰十一个` 与 `toml_test.rs` 的 `解析doc白名单源码自证`）。
- **消费侧零手抄**：消费面（`archi_scaffold` 草稿的字段全集断言）SHALL 遍历本常量，MUST NOT 在测试或产品代码里另抄八个字段名字面量。
- **不是值域词表**：本清单是字段**名**的封闭白名单，MUST NOT 混入 `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` 一族封闭**值**词表，亦 MUST NOT 进「消费面词汇集一致性对拍」（那几组词表的域是值，不是字段名）。
- **零线面**：常量 MUST NOT 派生 `Serialize` / `Deserialize` / `specta::Type`，MUST NOT 进 IPC 契约、TS bindings、`INDEX.toml` 的 `types` / `ops` 计数与任何生成物。
- **零字段面扩张**：本常量 MUST NOT 被用来放宽字段白名单——`RawElement` 仍八项、`Element` 仍十一项，`deny_unknown_fields` 语义逐字不变；`docs` 类退役键仍 `unknownField`。

#### Scenario: 常量与字段面同序同集

- **WHEN** 读取 `archi::parse::ELEMENT_FIELD_NAMES` 与 `parse/toml.rs` 的 `struct RawElement { … }` 块（字段行取 `///` 与 `#[…]` 之外以 `,` 收尾的行）
- **THEN** 常量恰八项且逐项同序相等（`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）；`docs` 在两侧均零命中；测试内不出现另一份手抄的八字面量

#### Scenario: 字段增删未同批更新即红

- **WHEN** 只在 `RawElement` 增加 / 删除 / 改名一个字段，而 `ELEMENT_FIELD_NAMES` 未同批更新
- **THEN** 源码自证测试红，并按集合差指出缺失或多出的字段名——「新增字段未补词典」不再可能以全绿形态漏过

#### Scenario: 常量不进线面与生成物

- **WHEN** 运行 `api:export` 与 `bindings:export` 并与盘上产物比对
- **THEN** `INDEX.toml` / `.schema.json` / `.i.rs` / `.i.ts` / `bindings.ts` 零 diff（`Backend.Core.Archi` 与 `Backend.Infra.AgentRuntime` 的契约状态均为 `absent`，常量不计入 `types` / `ops`）；`ELEMENT_FIELD_NAMES` 在生成物内零命中

### Requirement: 写面反馈判据（七条 warnings 与降噪不变式）

`archi::edit` SHALL 在写回成功后给出一组**已落盘、未阻断**的事实（下称 warnings），供写工具转述。这组事实 MUST NOT 引入新判据、新词汇或新档位——它只是把既有判据面的词在写面重述一次：

| # | 条件（零成本事实：写工具已持有的字段值 + `Path::exists`） | kind（既有词，取自 `as_str`） | severity | message 尾句 |
|---|---|---|---|---|
| 1 | `package` / `module` 且 `path` 空 | `ArchiDriftKind::NoPath` | warn | 纯分组容器请改 `composite` |
| 2 | 某条 `path` 不在盘 | `ArchiDriftKind::PathNotFound` | warn | 先行声明可忽略 |
| 3 | `api = declared` 且派生契约 md 不在盘 | `ArchiApiFindingKind::DocMissing` | warn | 用 `archi_api` 的 `bootstrap` 取草稿 |
| 4 | `api = none` 且 `doc` 空 | `ArchiApiFindingKind::DocMissing` | warn | 入口文档经 `doc` 声明 |
| 5 | `api = none` 且 `doc` 指不到盘 | `ArchiApiFindingKind::DocNotFound` | **error** | 先建文件或改 `declared` |
| 6 | `api = none` 但契约 md 已在盘 | `ArchiApiFindingKind::ApiUnexpected` | warn | 删契约 md 或改 `declared` |
| 7 | `action = delete` 成功且契约 md 仍在盘 | `ArchiApiFindingKind::ApiUnexpected` | warn | 一并清理（否则无人看管） |

- **判据单源**：`kind` 字面量 SHALL 取自 `ArchiDriftKind::as_str` / `ArchiApiFindingKind::as_str`（MUST NOT 在写面新写字面量）；`severity` SHALL 取自 `ArchiApiFindingKind::severity()` 单点，drift 侧两值（`noPath` / `pathNotFound`）SHALL 恒为 `warn`（它们在报告里恒落 `warnings` 桶，该事实由 `desktop-archi-drift` 的桶定义背书，写面 MUST NOT 另立档位表）；路径 SHALL 经 `api::paths::contract_path` / `api::paths::doc_path` 派生（MUST NOT 在写面拼路径）。
- **零 drift 调用**：warnings 的推导 MUST NOT 调 `drift::check`（含 scan——写是高频动作），MUST NOT 读 parser 之外的第二套模型；只允许「已持有的字段值 + `Path::exists`」两类事实。
- **降噪不变式**：`action = upsert` 时，warnings 的 kind 集合 SHALL ⊆ 由 `changed` 诱发的 kind 集合（`created = true` 视为全字段变化）——`noPath` / `pathNotFound` 只由 `path`（或 `kind`）诱发，`docMissing` / `docNotFound` / `apiUnexpected` 只由 `api` 或 `doc` 诱发；**只改 `summary` / `gate` / `metadata` ⇒ warnings 恒空**。`action = delete` 时仅允许第 7 条（由动作本身解释，不要求 `changed` 在场）。
- **不改判据面**：这七条 MUST NOT 进入 `ArchiDriftReport`（不进 `errors` / `warnings` / `apiFindings`、不影响 `status` / `passed`），MUST NOT 让 `ArchiApiFindingKind` 扩张（D10-B：孤儿契约 md 的 drift 判据不做，且不列 backlog）。

#### Scenario: 七条逐条可绿

- **WHEN** 分别构造 ① `package` 空 `path` ② `path` 指向不存在目录 ③ `api = declared` 且契约 md 缺失 ④ `api = none` 且 `doc` 未声明 ⑤ `api = none` 且 `doc` 指向不存在文件 ⑥ `api = none` 但契约 md 在盘 ⑦ 删除元素后契约 md 仍在盘
- **THEN** 七种情形各产一条对应 kind 的 warning，severity 依次为 `warn` / `warn` / `warn` / `warn` / `error` / `warn` / `warn`；`kind` 字面量与 `ArchiDriftKind` / `ArchiApiFindingKind` 的 `as_str` 逐字相等

#### Scenario: 降噪不变式可断言

- **WHEN** 对同一元素分别只改 `summary`、只改 `gate`、只改 `metadata`，再改 `path`
- **THEN** 前三次 warnings 均为空；第四次含 `path` 诱发的条目；任意一次 upsert 的 warnings kind 集合都不超出 `changed` 诱发集合（`created` 情形除外，它视为全字段）

#### Scenario: 判据面零变化

- **WHEN** 对同一模型分别取写工具响应的 warnings 与 `archi_drift` 报告
- **THEN** 报告的 `status` / `passed` / `apiFindings` / `warnings` 不因写面 warnings 变化（后者不进报告）；`ArchiApiFindingKind::ALL` 仍十三值、`ArchiDriftKind::ALL` 仍五值；源码中 `edit/` 目录零 `drift::check` 引用

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/Cargo.toml` | crate 落位与依赖 | 依赖 `foundation` + `serde` / `serde_json` / `specta` + `toml` / `toml_edit` / `petgraph` / `ignore`（均取自锁内既有版本，零新增包）；零 Tauri、零 store、零 spawn、零 `workflow` 依赖 |
| `crates/core/archi/src/model.rs` | 格式无关 AST | `Model` / `Element`（`fqn` / `kind` / `gate` / **`api`** / **`entry`** / `path` / **`doc: Option<String>`** / `summary` / `metadata` / `file` / `line`，`docs` 已删）/ `Relation` / `SourceSpan` / `Diagnostic`；`derive PartialEq`（UI diff 预留）；关系级 `Option<SourceSpan>` 占位；`Element.api: ApiStatus`（`#[serde(default)]` 语义面 = 缺省 `Declared`）与 `Element.entry: Option<String>`（缺省 `None`，语义归 `desktop-archi-api`）；其余字段名 / 顺序 / 语义零变化 |
| `crates/core/archi/src/parse/mod.rs` | 多文件聚合（语义零变化、收面判据显式化） | `trait SourceParser { fn parse(&str) -> ParseOutcome }`；`parse_dir(root) -> ParseOutcome`：收面是「`archi_graph_model_dir(root)` 一级 `is_file()` + `MODEL_FILE_SUFFIX` 后缀」（MUST NOT 递归、MUST NOT 加目录豁免，`models/` 契约资产目录对聚合面不可见）、字典序、跨文件 `duplicateElement`、位置回填；空目录 / 一级零 toml → 空模型 |
| `crates/core/archi/src/model.rs`（doc 串） | 仅 `api` 字段 doc 方位串 | 「接口定义写在架构域 `api` 目录的契约资产里」改为「架构域 `models/<分层目录…>/` 目录」——该行是本文件唯一允许的 diff；字段面（八项）、`deny_unknown_fields` 与解析实现逐字不变 |
| `crates/core/archi/src/parse/toml.rs` | TOML 实现 | `toml` + span 反序列化；`deny_unknown_fields`；`kind` / `gate` 封闭枚举；`RawElement` 含 `api` / **`entry`** / **`doc`**（均 `#[serde(default)]`；`entry` 与 `doc` 为 `Option<String>`）、不含 `docs`，落在既有 `deny_unknown_fields` 面内 ⇒ `api = "non"` 报 `invalidValue` 并列全合法值、`entry = ["a"]` / `doc = ["a"]` / `{…}` 由 serde `invalid type` 归 `invalidValue`、`docs` 与拼错的键归 `unknownField`，MUST NOT 走 `metadata` 自由键；parser 错误 → 结构化诊断（保留原文与 line/column） |
| `crates/core/archi/src/graph.rs` | 图单点 | `ModelGraph`（petgraph）唯一构建点；SCC 成环 / 依赖祖先 / 未定义引用 / 孤儿层级；`depends_on` / `depended_by` / `children` / `neighborhood`（1-hop） |
| `crates/core/archi/src/dto.rs` | 三面共用投影 + 封闭词汇单点（补全量常量） | `ArchiGraph`（含停摆态）/ `ArchiModuleDetail` / `ArchiSearchResult` / `ArchiChangeSummary`；封闭词汇枚举 `ElementKind` / `GateTier` / **`ApiStatus`** / `DiagnosticKind` / `EdgeMark` / `NodeMark`（`specta::Type`）；`ArchiDiagnosticKind::ALL: [Self; 9]`（与 `as_str` 同源；长度与词面双向对拍；既有 `ElementKind::ALL` / `GateTier::ALL` 语义零变）；新增 `ApiStatus`（`declared` / `none`，`ALL` + `as_str` + `specta::Type`）与 `ArchiApiRef { status, contractPath, schemaPath, generatedPath }`；`ArchiElement.api` / `ArchiElementBrief.api` / `ArchiModuleDetail.api` 出线；`ArchiElement` 增 `entry: Option<String>` 与 **`doc: Option<String>`**（线格式 `entry` / `doc`，缺省 `null`，**显式字段不省略**——与「投影不补默认值」同款；`doc` 是模型字段的授权拷贝）；`DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 与 `ArchiModuleDetail.docs` / `.warnings` 删除，`ArchiSearchResult.docs: Vec<DocRef>` → `doc: Vec<String>`，`SearchScope::Docs` → `Doc`，`linked_doc_paths` 删；`ArchiApiRef` 三路径由 `api::paths` 派生后转 repo 相对形态（契约 md / schema 在 `<archi_models_dir>/<分层目录…>/`，定义归 `desktop-archi-api`；绑定面恰 2 行 doc 注释随动、`bindings.zod.ts` 零 diff）；既有 DTO 字段名 / 顺序 / 语义零变化；投影只裁剪，MUST NOT 重算（入口解析在 `api::paths`，不在投影层） |
| `crates/core/archi/src/edit.rs` | gate 写回纯函数 | `set_gate(source, fqn, gate) -> Result<String, String>`：`toml_edit` 渲染保留注释空行、前置校验（存在 / 值域）、渲染后自证可解析；零文件系统写 |
| `crates/core/archi/src/diff.rs` | 结构 diff | `model_diff(base, next) -> ModelDiff`：元素增删（FQN 键）、关系增删（`(from,to)` 键）、`gate` 变更；文件 / 行序不参与身份 |
| `crates/core/archi/src/model_test.rs` + `parse/toml_test.rs` + `dto_test.rs` | 模型语料（扩展） | `entry` 三态（省略 → `null` / 单值 / 数组被拒 `invalidValue`）与 `doc` 三态（省略 → `null` / 单值 / 数组被拒 `invalidValue`）；`docs` 写成即 `unknownField`；投影显式 `null` 不省略；既有 `api` / `path` 语料逐字保留（两态取值 / 缺省 `declared` / 拼写错报 `invalidValue` 并列全合法值 / 与 `metadata.api` 的口径差）；`ArchiElement` 字面量补 `entry: None` 与 `doc: None`（机械） |
| `src-tauri/src/commands/archi/mod.rs` | **九命令**薄包装（九命令口径见「IPC 命令面契约（九命令）」） | `archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate` / `archi_relation`，各配一个 `*_inner` 领域组装纯函数（离 Tauri 可测）；`Result<T, String>` 错误面；写命令两条（`archi_set_gate` 写前校验 + 原子写回；`archi_api_generate` 只增只改零删除）；命令层零领域规则（文档正文经通用 `read_file`） |
| `src-tauri/src/commands/mod.rs` | 命令登记面 | `all_commands!` 的 archi 段 5 → 6 条（追加 `commands::archi::archi_file_tree`，位于 `archi_drift` 之后、`read_file` 之前；builder 与 `generate_handler!` 同源） |
| `crates/core/archi/src/tree.rs`（新增，见 `desktop-archi-file-tree`） | 文件树投影 | `file_tree(root) -> Result<ArchiFileTree, String>`：枚举 + 排除 + 归属 + 目录合成；纯读盘、零写盘、零 spawn；归属与剥离一律经 `OwnerIndex` |
| `crates/core/archi/src/dto.rs`（扩展，见 `desktop-archi-file-tree`） | 出线投影 | 新增 `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark`；既有九组封闭词汇与全部出线 DTO 字段名 / 顺序 / 语义零变化 |
| `src-tauri/src/bindings/mod_test.rs`（扩展，见 `desktop-ipc-type-bindings`） | 清单守卫随动 | 命令名 / 包装名清单 48 → 49、出线 DTO 清单 97 → 101、archi 族两组清单 4 → 6 与 19 → 23；尾部追加序断言把 `archi_file_tree` 插在 `archi_drift` 之后 |
| 本能力 `## Purpose` | 口径与标题同源 | Purpose SHALL 写「与九命令 IPC 契约」，MUST NOT 自称「四命令 / 六命令 / 八命令」——标题与 Purpose 必须同一事实；`archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate` / `archi_relation` 的追加在标题、Purpose 与本表三处同批体现（「同一事实两种自称」是本能力要清的病灶） |
| `crates/core/archi/tests/` | 语料与 golden | `corpus/dsl/<case>/*.toml` + `golden_test.rs` + `vocabulary_test.rs`；golden 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写惯例 |
| `crates/core/archi/tests/vocabulary_test.rs`（扩展） | 四面词汇对拍 | 语料 → 进程内诊断集 / `archi_module` 串 / IPC DTO / 架构会话知识文本四面词汇集相等；`ALL` 与 `as_str` 逐值对拍；`ApiStatus::ALL` 二值与 `as_str` 逐值对拍；封闭词汇组新增 `api` 一面；知识文本含 `declared` / `none` |
| `crates/infra/agent/src/archi_knowledge.rs` + `archi-knowledge.md`（见 `desktop-archi-agent`） | 知识文本（改） | 契约词汇面注入 `api`（`declared` / `none`）与契约目录入口；注入矩阵与 policy 零改动 |

### API: `src-tauri/src/commands/mod.rs` + `src-tauri/src/commands/archi/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `all_commands!` archi 段 | 6 → 8 条 | 追加 `commands::archi::archi_api_check` 与 `commands::archi::archi_api_generate`（位于 `archi_file_tree` 之后、`read_file` 之前；`read_file` 仍为尾条）；builder 注册与 `generate_handler!` 同源 |
| `commands/archi/mod.rs` | 改 | 文件头「六命令」→「八命令」、「唯一写命令是 `archi_set_gate`」→「写命令两条（`archi_set_gate` / `archi_api_generate`）」；新增两条命令 + 两个 `*_inner`；既有六条零改动 |
| `commands/archi/mod_test.rs` | 改 | root 校验用例由六命令扩到八命令并把名字里的旧口径改符实；新两条命令的 `_inner` 一致性与零写盘 / 写后差异清空用例 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| `crates/core/archi/src/dto.rs` 既有投影 | `ArchiGraph` / `ArchiModuleDetail` / `ArchiApiRef` / `ArchiFileTree` 等逐字不变（本能力只登记命令面清单与写面身份，DTO 投影不变） |
| 前端调用路径 | `archi_set_gate` 仍只在 `incremental` 路径出现；新增 `archi_api_generate` 的调用面归 `desktop-archi-contract-panel` |

### 模块增量：字段面与线面换面（module-doc-field-docs-retirement）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|------|
| `crates/core/archi/src/model.rs` | 格式无关 AST | `Element` 字段面：`fqn` / `kind` / `gate` / `api` / `entry` / `path` / **`doc: Option<String>`** / `summary` / `metadata` / `file` / `line`（`docs` 已删）；`Relation` / `SourceSpan` / `Diagnostic` 零变化 |
| `crates/core/archi/src/parse/toml.rs` | TOML 实现 | `RawElement` 含 `doc: Option<String>`（`#[serde(default)]`）、不含 `docs`；`deny_unknown_fields` 主体不变（`docs` 写成即 `unknownField`、`doc = [..]` 归 `invalidValue`）；parser 错误 → 结构化诊断零变化 |
| `crates/core/archi/src/dto.rs` | 投影与词汇单点（**退役面 + 一处授权进场**） | `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 删除；`ArchiModuleDetail.docs` 与 `.warnings` 删、`.doc: Option<String>` 加；`ArchiSearchResult.docs: Vec<DocRef>` → `doc: Vec<String>`；`SearchScope::Docs` → `Doc`（线格式词 `doc`，`parse` 匹配臂与非法值文案同步）；`linked_doc_paths` 删；**`ArchiElement.doc: Option<String>` 加**（授权进场字段：模型字段的拷贝，与 `entry` 同型——单值、缺省 `null`、显式字段不省略、字段序紧跟 `entry`；`to_element` 逐元素拷贝，供 `paths::carrier_path` / `api::doc_facts` / `api::gating_findings` / `check` 取 `doc` 事实）；`ArchiElementBrief` / `ArchiApiRef` 逐字不变 |
| `crates/core/archi/src/search.rs` | 检索投影 | 语料 = `path` 非空元素的在盘入口文档载体（declared → 派生 `contractPath`、none → `doc`），条目为路径串；`matches_doc` 吃路径串 + `file_stem`（`file_stem` 从 `docs.rs` 迁入，不读正文） |
| `crates/core/archi/src/lib.rs` | 出口面 | `pub mod docs` 与 `#[cfg(test)] mod docs_test` 删；`DocKind` / `DocRef` / `DocSource` / `WarningKind` 出口删；其余出口逐字不变 |
| `src-tauri/src/commands/archi/mod.rs` | 八命令薄包装 | 签名 / 语义 / 错误面逐字不变（命令层零领域规则；文档正文经通用 `read_file`）；`archi_module_inner` 的投影仍只调 `archi::dto` |
| `src-tauri/src/bindings/mod_test.rs` | 清单守卫随动 | 出线 DTO 清单删 `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 五项；归档断言（`DocRef` 四字段）删除；清单长度连带核对随动。`ArchiElement.doc` 属**字段级**进场——`ArchiElement` 本就在清单内，清单成员不增删，只有生成物内容变化 |
| `packages/desktop/src/types/generated/bindings.ts` + `.zod.ts` | 生成物 | `bindings:export` 重导：五个类型离场、`doc` 三处进场（`ArchiModuleDetail.doc` / `ArchiSearchResult.doc` / `ArchiElement.doc`）；类型 / 字段序由生成器决定，MUST NOT 手改 |

### 模块增量：关系 apis 与九命令（relation-api-dependency-check）

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

### 模块增量：path 基数判据与 metadata 形态口径（archi-model-path-and-silent-trap）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/parse/toml.rs` | 格式判定单点（本变更 +1 判据） | `to_model` 对每个元素校验 `kind == "package" && path.len() >= 2` ⇒ 一条 `DiagnosticKind::InvalidValue`，位置取 `path[1].span()`（第 2 条）；`Element.path` 仍 `Vec<String>`、`RawElement` 字段面八项与 `deny_unknown_fields` 逐字不变；0 / 1 条与其余 kind 零新判据；parser 错误到结构化诊断的换算（原文 / line / column 零加工）不变 |
| `crates/core/archi/src/dto.rs` | 停摆语义（零改动） | `graph_view(root)` 的「`diagnostics` 非空 ⇒ `elements` / `relations` 清空」是既有唯一收口点，本变更 MUST NOT 改成「跳过坏元素」；`ArchiDiagnosticKind::ALL` 仍九值 |
| `crates/core/archi/src/model.rs` | AST（零改动） | `Element { kind, path, … }` 字段面与派生 `PartialEq` 零改动；字段 `metadata: BTreeMap<String, String>` 的形态语义只由解析结果决定，本变更只纠正文案、不改类型 |
| `crates/core/archi/src/parse/toml_test.rs` | 判据断言面 | 四条 `path` 基数用例（package 2 条拒收带行号 / package 0 条零诊断 / module 2 条零诊断 / composite 2 条零诊断）+ 两条 metadata 形态行为用例（子表与内联表等价 / 子表唯一声明即 `missingField`）；既有用例名与注释去假因果 |
| `crates/core/archi/src/dto_test.rs` | 停摆断言面 | 新增独立用例（package 两条 path 的 `invalidValue` ⇒ `graph_view` 停摆），既有 `graph_view九类结构错一律停摆` 的表与名字逐字不变 |

### API: 对外面（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiDiagnosticKind` | 零改动 | 仍九值；`invalidValue` 的语义面（「值不在封闭值域 / 字面形态违例」）由本变更扩用一次，不加新变体 |
| `ArchiGraph` / 投影 DTO | 零改动 | 字段名 / 顺序 / 语义与停摆态形状逐字不变 |
| IPC 命令面 | 零改动 | 九命令与错误面不变；本判定经既有 `archi_graph` / `archi_module` 读面回灌 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| `archi_graph` / `archi_module` / `archi_drift` | 命令形态、错误文案语义与投影形状不变；新判据只让坏模型更早进入既有停摆分支 |
| 前端组件 | 零 UI / 零 DTO 变化（停摆面板沿用既有 `structure-error-panel`） |

### 模块增量：字段面机器可读清单（archi-scaffold-field-dictionary）

主基线既有行逐字沿用；本变更只加一条常量（字段面本身零变化：`RawElement` 仍八项、`Element` 仍十一项、`deny_unknown_fields` / 值域封闭 / `apis` 五拒 / `path` 基数逐字不动），受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/parse/toml.rs` | 字段面单点（本变更只加一条常量） | `pub const ELEMENT_FIELD_NAMES: [&str; 8]`（声明序）；`RawElement` / `to_model` / `validate_apis` / `validate_path` / `classify` 零改动 |
| `crates/core/archi/src/parse/mod.rs` | 模块出口 | `pub use toml::{TomlParser, ELEMENT_FIELD_NAMES};`（一行；括号内取 fmt 实序，`cargo fmt --all --check` 零退出；`SourceParser` / `parse_dir` 不动） |
| `crates/core/archi/src/parse/toml_test.rs` | 字段面断言面 | 新增 `字段面清单与RawElement同序同集`（`include_str!("toml.rs")` 源码自证）；既有解析 / 值域 / `apis` 五拒 / `path` 基数用例逐字保留 |
| `crates/core/archi/src/lib.rs` | crate 根出口 | 零改动——常量经 `parse` 再导出，MUST NOT 提成 crate 根 `pub use` 清单项（字段面清单不是跨 crate 通用词汇） |

### API: `archi` crate 的 `parse` 出口

| 项 | 变化 | 描述 |
|---|---|---|
| `archi::parse::ELEMENT_FIELD_NAMES` | 新增 | `[&str; 8]`，顺序 = `RawElement` 声明序；零 serde / 零 specta |
| `archi::parse::TomlParser` / `SourceParser` / `parse_dir` | 零改动 | 解析入口、诊断分类与聚合语义逐字不变 |
| `archi` crate 根出口（`lib.rs`） | 零改动 | 常量不进根 `pub use` 清单；`dto` 词汇出口不变 |
| `RawElement` / `Element` / `Relation` 字段面 | 零改动 | 八项 / 十一项 / 关系四键各自逐字不变（本常量只是字段名的机器可读副本） |

### Function: 解析与校验面

| 项 | 变化 | 描述 |
|---|---|---|
| `to_model` / `validate_apis` / `validate_path` / `classify` | 零改动 | `apis` 五拒、`package` 的 `path` 基数、`invalid type → invalidValue` 分类逐字不变 |

### CLI / Component: 零变化

| 项 | 变化 | 描述 |
|---|---|---|
| IPC 命令面 / 前端组件 | 零改动 | 常量无 serde，无 DTO / bindings / UI 变化 |
| `.dev-team/architecture/**` 生成物 | 零改动 | `INDEX.toml` / `.schema.json` / `.i.*` 零 diff（两元素 `status = "absent"`） |
