# desktop-archi-model Specification（delta: archi-model-path-and-silent-trap）

## Purpose

本 delta 把 `kind` 与 `path` 的搭配从人工纪律变成机械判据，并把一处「静默陷阱」的假因果换成实测口径：

- **`package` 的 `path` 至多 1 条**（0 条 = 纯容器，合法）：≥2 条由 `parse` 期出 `invalidValue`、诊断指向第 2 条，模型经 `graph_view` 整图停摆（既有全或无语义）；`module` / `domain` / `composite` 不设上界。判据只守上界——契约面早已用 `path.first()`，这是把暗假设点亮，不是新增约束。
- **`metadata` 子表禁令作废**：`[element."X".metadata]` 与内联表**等价**（TOML 表语义、零诊断）；真实形态错误只有两类——未引号点分表头 `[element.App.Auth]` 归 `unknownField`（整份源不解析）、子表成为该 FQN 的唯一声明（缺 `kind`）归 `missingField`。两者都有行为测试钉住，MUST NOT 再写成「会被读成另一个元素」。

九值诊断词汇、`kind` / `gate` / `api` 三组 `ALL`、`path` / `doc` / `entry` / `apis` 字段面与退役条款逐字不变。

## MODIFIED Requirements

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

## Module Contract

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
