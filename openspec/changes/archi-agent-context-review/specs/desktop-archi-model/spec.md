# desktop-archi-model Specification（delta: archi-agent-context-review）

## Purpose

本 delta 让 `doc` 字段**惰性化**（零判据读者），并把写面 warnings 由七条收为五条。

- **字段面**：`doc` 的「只有 `api = none` 可写」降为**约定**（无机械判据）；字段集、值域、八字段顺序与 `deny_unknown_fields` 面零变化。
- **写面 warnings**：删三条 `doc` 事实（`none` ∧ `doc` 空 / `doc` 指不到盘 / `doc` 诱发 `ApiUnexpected`）；`DocMissing` 的事实仍是**派生 `<FQN>.md` 不在盘**（与 `api` 无关，只由 `created` 诱发），`ApiUnexpected` 的事实是 **`api = none` ∧ schema 在盘**；路径派生 `doc_path` 退役，文档走 `contract_path`、接口走 `schema_path`。
- **降噪**：补「只改 `doc` ⇒ warnings 恒空」一条；`noPath` / `pathNotFound` 的诱发面逐字不变。
- **词汇引用**：契约判据面十三 → 十值（`docNotFound` 不再被模型面引用）；`doc` 的「只有 `api = none` 可写」降为约定（`declared` 元素写了不报——`apiUnexpected` 形态②删除）。

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

# 无公共 API 的元素可声明入口文档（doc 零判据读者：declared 元素写了也不报）
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
- **关系条目字段面（第四键，更早变更已落地）**：每条 `[[relation]]` SHALL 为四键——`from` / `to` 必填，`desc` / **`apis`** 可选；`apis` SHALL 为**字符串数组**（缺省空 = 不声明 API 粒度）。`deny_unknown_fields` 语义不变：`api = ["X"]`（单数拼错）报 `unknownField`，与元素表的 `api` 字段互不干扰。
- **`apis` 的值面四条（只许名字）**：① **只许名字**——逐项 MUST NOT 含空白、MUST NOT 含 `::`、MUST NOT 表达签名 / 参数 / 类型形状，即不得是「`config_path() -> PathBuf`」或「`scan::ImportFact`」一类形态；② **非空**——逐项 MUST NOT 为空串；③ **不重复**——同一关系的 `apis` 内同名重复即拒收（MUST NOT 静默去重，静默去重会让「写重了」与「写对了」不可辨）；④ **空数组拒收**——`apis = []` 与省略同义，显式空数组报 `invalidValue`（同 `doc = [..]` / `entry = [..]` 的「显式错形态直接拒收」先例）。四条违反一律 `invalidValue` + 位置，模型整体停摆。
- **`apis` 的取值域 = `to` 元素的代码导出名**：与扫描事实（`ImportFact.names` / `DependencyEdge.names`）同一名字空间，比对 SHALL **逐字符**（MUST NOT 大小写折叠、MUST NOT 驼峰归一——否则判定侧常驻第二处映射规则）。取值域 MUST NOT 收窄为「接口定义（`<FQN>.schema.json`）声明的 op / `$defs` 名」：接口声明面是留白面（前端元素的空 schema），收窄即不可用；也 MUST NOT 放宽为「协议正文」（见下条）。
- **`apis` 是开放值集，不进封闭词汇对拍**：与 `doc` / `entry` 同列——它的值域是任意代码标识符（开放集），不是封闭词汇；因此四组派生 `ALL` 常量与「消费面词汇集一致性对拍」的词表 MUST NOT 因本字段扩张，知识文本也只给纪律句、不给词表。
- **值域封闭**：`kind` 必填且 SHALL 为 `package` / `domain` / `module` / `composite` 四值之一（类型层级从大到小：包 > 领域 > 模块，`domain` 层可省略；`composite` 无实际层级，仅把过多的同级元素分类，可嵌套在任意类型之上）；`gate` 可选且 SHALL 为 `off` / `warn` / `strict` 三值之一（缺省 `off`）；**`api`** 可选且 SHALL 为 `declared` / `none` 二值之一（缺省 `declared`）；**`entry`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组——数组会让「一个入口只属于一个模块」这条约束不可表达）；`path` SHALL 为字符串数组（缺省空；基数随 `kind`，见下条）；**`doc`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组——数组会让「入口文档唯一」不可表达）；`summary` 可选字符串；`metadata` SHALL 为字符串→字符串的自由表（未知键忽略，非字符串值即 `invalidValue`）。**`docs` 已退役**：字段面 MUST NOT 含 `docs`，写了即 `unknownField`。
- **`path` 的基数是 `kind` 的属性（`root` 与 `paths` 两种语义在此点名）**：`package` 是**一个包**（最小编译单元 / 独立依赖），`path` SHALL **至多 1 条**——0 条合法（纯容器，不对应任何目录），写 1 条即该包的**包根**目录；≥2 条 SHALL 报 `invalidValue`，诊断位置指向**第 2 条** `path` 的源码位置（比元素表头更准），message 点名「一个包至多一条包根目录（纯容器可留空）——多条 path 属 `module`」，模型经 `graph_view` **整图停摆**（既有全或无语义，MUST NOT 改成「跳过该元素」）。`module` 的 `path` 是**若干根的清单**，不设上界（目录型 / 单文件 / 多文件装配同形）；`domain` / `composite` 的 `path` 选填且不设上界。本条只守 `package` 的上界，MUST NOT 顺手扩张出 `module ≥ 1`、目录尾 `/` 一类形态校验；容错面为零是已知代价（`package` 写两条 `path` 在契约面本就只有 `path[0]` 可见）。
- **未知字段拒绝**：元素表内出现未列出的字段（如 `gatte` / `docs`）SHALL 由 `deny_unknown_fields` 拒绝（`unknownField`）。
- **属性与语义分离**：`gate` 档位 SHALL 是模型属性（本能力定义其值域与缺省）；档位的**语义消费**（参与归属、违例定级、门禁结论）归 `desktop-archi-drift` 与 `desktop-archi-gate`，本能力 MUST NOT 定义任何定级规则。
- **`api` 状态指针（显式字段，非 `metadata`）**：元素 SHALL 有封闭二值的 `api` 字段（`declared` 缺省 / `none`，附 `ApiStatus::ALL`），语义为「本模块是否有公共 API 契约」的状态指针：**`declared` ⇒ 接口真相 = 同目录 `<FQN>.schema.json`；`none` ⇒ 无接口面（schema 在盘即违约）**。本变更起它**不再兼任入口文档载体的开关**——资产① 的落点恒为派生 `<models>/<分层目录…>/<FQN>.md`（= `contract_path`），与 `api` 无关；`doc` 零判据读者（`declared` 元素写了也不报，`apiUnexpected` 形态②已删除）。该字段 MUST NOT 承载任何协议正文（签名 / 符号 / 接口 / 规范全文）。走显式字段而非 `metadata` 自由键的理由是可获得 `deny_unknown_fields` 的拼写校验：`api = "non"` 报 `invalidValue` 并列全合法值，而 `metadata = { api = "non" }` 会静默变成「未声明」。完备性判据（`api = none` 而 schema 在盘 ⇒ `apiUnexpected`）归 `desktop-archi-api-check`。
- **`entry` 是实现入口指针（显式字段，非 `metadata`）**：语义为「本模块的实现入口文件」——**单值**、repo 相对 POSIX 路径（与 `path` 同口径，`./` 前缀由归一剥除、不参与身份）。四条判据（基准 / 落位 = 某条 `path` 或目录型 `path` 之内 / 存在 = 盘上必须有 / 跨元素唯一）与三态解析链（声明 → 语言约定默认 → 无入口）归 `desktop-archi-api`「生成物落点与入口查找」，本能力只定义字段本身的值域与解析面。走显式字段而非 `metadata` 自由键的理由同 `api`：`entry = ["a"]`（数组）与 `entery`（拼错）必须能机械拒收。
- **`doc` 是入口文档指针（显式字段，更早变更已落地；本变更只把它的读者面降为零）**：语义为「本模块的入口文档路径」——**单值**、repo 相对 POSIX 路径（`./` 前缀由归一剥除、不参与身份；无绝对前缀 / 无盘符 / 无 `..` / 无空段）。形态与纪律各一条：① **只有 `api = none` 的元素会写**（`declared` 的文档在派生落点）——该条是**约定而非机械判据**（写了不报、指不到不报）；② **值 MUST NOT 指向** `<archi_dir>/models/<分层目录…>/**` 与 `openspec/specs/**`（机械拒收：`invalidValue`）。**`doc` 零判据读者**：判据面 / 纳管面 / 路径派生面都不读它（指不到盘也不报），其值仅供 `archi_module` 投影与画布「入口文档」段呈现；本能力只定义字段与值域。走显式字段而非 `metadata` 的理由同 `api`：数组形态与拼错键必须能机械拒收。
- **三个指针都 MUST NOT 承载协议正文**：`entry` 是**位置**指针（文件在哪），`api` 是**状态**指针（有无公共 API），`doc` 是**入口文档**指针（读哪一份就懂这个模块）；三者都 MUST NOT 让模型开始携带签名 / 导出符号 / 接口定义。单文件根元素（`views/welcome-view.tsx` 一类）MUST NOT 声明 `entry`——它的根文件就是入口，声明即冗余；显式 `entry` 的正当用法只有「约定默认指不到」的两类：目录型壳（`src-tauri/src/` → 显式 `src/lib.rs`）与二进制 crate（只有 `src/main.rs`）。
- **协议正文不经模型**（知情边界，防误加字段）：元素上 MUST NOT 出现接口 / 导出符号 / 签名一类协议**正文**字段——唯一允许的协议相关字段是上述三个指针（`api` / `entry` / `doc`）。原边界要求的两个问题：**契约权威在哪**——接口在 `<archi_models_dir>/<分层目录…>/<FQN>.schema.json`（理由在同目录 `<FQN>.md`，见 `desktop-archi-api`），模型只存状态指针；**两套提取器怎么对齐**——不提两套：单一 schema + 单一生成器，`scan/rust.rs` / `scan/ts.rs` 是既有取数器（非权威），只供一次性 bootstrap。模型内再存一份协议正文必然漂移，该做法仍被否决；重提须独立变更。
- **关系 `apis` 不属协议正文（本变更的边界区分，防被当作破例）**：上条禁令的对象是**元素**上的正文，理由是「存一份正文必然漂移」。`relation.apis` 在四个维度上都与之可分——① **位置**：在关系上（**消费方**的依赖选择），不是元素上（提供方的自我描述）；② **内容**：只有名字，没有签名 / 类型形状 / 接口定义；③ **权威**：名字的权威仍在代码导出集与接口定义（`properties` / `$defs` 键），模型只回答「这条依赖选了哪几个」；④ **抗漂移**：每次检查与扫描事实机械对差（两桶见 `desktop-archi-drift`），错名不会静默。因此**元素**上的正文禁令逐字保留，关系上的**名字清单**被本条显式授权；把签名 / 类型形状 / `::` 限定路径写进 `apis` 仍按上条拒收。
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

- **「文档诊断 `docNotFound`」一行已从本词汇集删除**：它原属 `WarningKind`（`ArchiModuleDetail.warnings` 的载体），随 `docs` 三源退役而整组消失——**词汇组数由七组减为六组**。谓词不同源：契约判据面另有 `ArchiApiFindingKind`（**十值**；`docNotFound` 已随 `doc` 字段惰性化删除）与 `ArchiApiFinding.category`（开放值集，成员收缩但分组仍七值），其词汇对拍归 `desktop-archi-api-check`，MUST NOT 与模型面五组混装进本对拍（前者是「契约 vs 代码」，后者是「模型 vs 代码」）。
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

- **WHEN** 审查模型面五组词汇与契约判据面词汇（`ArchiApiFindingKind` 十值 / `category` 七值）的对拍落点
- **THEN** 前者在 `desktop-archi-model` 的三面词汇对拍内（`vocabulary_test.rs`），后者在 `desktop-archi-api-check` 的词汇对拍内；两处 MUST NOT 互相引用对方的 `ALL`，`WarningKind` 在两处都不出现


## ADDED Requirements

### Requirement: 写面反馈判据（五条 warnings 与降噪不变式）

`archi::edit` SHALL 在写回成功后给出一组**已落盘、未阻断**的事实（下称 warnings），供写工具转述。这组事实 MUST NOT 引入新判据、新词汇或新档位——它只是把既有判据面的词在写面重述一次：

| # | 条件（零成本事实：写工具已持有的字段值 + `Path::exists`） | kind（既有词，取自 `as_str`） | severity | message 尾句 |
|---|---|---|---|---|
| 1 | `package` / `module` 且 `path` 空 | `ArchiDriftKind::NoPath` | warn | 纯分组容器请改 `composite` |
| 2 | 某条 `path` 不在盘 | `ArchiDriftKind::PathNotFound` | warn | 先行声明可忽略 |
| 3 | `created = true` 且派生 `<FQN>.md` 不在盘（与 `api` 无关） | `ArchiApiFindingKind::DocMissing` | warn | 新建元素待补文档：理由写 `.md`、接口写 `.schema.json`（`bootstrap` 取双草稿） |
| 4 | `api = none` 但 `<FQN>.schema.json` 已在盘 | `ArchiApiFindingKind::ApiUnexpected` | warn | 删该 schema 或改 `declared` |
| 5 | `action = delete` 成功且 `<FQN>.schema.json` 仍在盘 | `ArchiApiFindingKind::ApiUnexpected` | warn | 一并清理（否则无人看管） |

- **三条 `doc` 事实已删除**：旧表里「`api = none` 且 `doc` 空 ⇒ `DocMissing`」「`api = none` 且 `doc` 指不到盘 ⇒ `DocNotFound`（error）」「由 `doc` 诱发 `ApiUnexpected`」三条随 `doc` 字段惰性化与 `DocNotFound` 契约判据值删除；**写面 MUST NOT 读 `doc`**（它的任何取值都不产 warning）。
- **判据单源**：`kind` 字面量 SHALL 取自 `ArchiDriftKind::as_str` / `ArchiApiFindingKind::as_str`（MUST NOT 在写面新写字面量）；`severity` SHALL 取自 `ArchiApiFindingKind::severity()` 单点，drift 侧两值（`noPath` / `pathNotFound`）恒 `warn`；路径 SHALL 经单点函数派生——文档走 `api::paths::contract_path`（派生 `<FQN>.md`）、接口走 `api::paths::schema_path`（MUST NOT 在写面拼路径，MUST NOT 再引已退役的 `doc_path`）。
- **零 drift 调用**：warnings 的推导 MUST NOT 调 `drift::check`（含 scan），MUST NOT 读 parser 之外的第二套模型；只允许「已持有的字段值 + `Path::exists`」两类事实。
- **降噪不变式**：`action = upsert` 时，warnings 的 kind 集合 SHALL ⊆ 由 `changed` 诱发的 kind 集合（`created = true` 视为全字段变化）——`noPath` / `pathNotFound` 只由 `path`（或 `kind`）诱发；`apiUnexpected` 只由 `api` 诱发；`docMissing` 只由 `created` 诱发（派生 md 的存在性与字段值无关）；**只改 `summary` / `gate` / `metadata` / `doc` ⇒ warnings 恒空**。`action = delete` 时仅允许第 5 条（由动作本身解释，不要求 `changed` 在场）。
- **不改判据面**：这五条 MUST NOT 进入 `ArchiDriftReport`（不进 `errors` / `warnings` / `apiFindings`、不影响 `status` / `passed`），MUST NOT 让 `ArchiApiFindingKind` 扩张。

#### Scenario: 五条逐条可绿

- **WHEN** 分别构造 ① `package` 空 `path` ② `path` 指向不存在目录 ③ `created = true` 且派生 md 不在盘 ④ `api = none` 但 schema 在盘 ⑤ 删除元素后 schema 仍在盘
- **THEN** 五种情形各产一条对应 kind 的 warning，severity 依次为 `warn` / `warn` / `warn` / `warn` / `warn`；`kind` 字面量与 `ArchiDriftKind` / `ArchiApiFindingKind` 的 `as_str` 逐字相等；`DocNotFound` 零出现

#### Scenario: doc 零诱发

- **WHEN** 对同一元素只改 `doc`（写值 / 清键 / 指向不存在文件三种）
- **THEN** 三次 upsert 的 `warnings` 均为 `[]`（`doc` 零判据读者）；`changed = ["doc"]`

#### Scenario: 降噪不变式可断言

- **WHEN** 对同一元素分别只改 `summary`、只改 `gate`、只改 `metadata`，再改 `path`
- **THEN** 前三次 warnings 均为空；第四次含 `path` 诱发的条目；任意一次 upsert 的 warnings kind 集合都不超出 `changed` 诱发集合（`created` 情形除外）

#### Scenario: 判据面零变化

- **WHEN** 对同一模型分别取写工具响应的 warnings 与 `archi_drift` 报告
- **THEN** 报告的 `status` / `passed` / `apiFindings` / `warnings` 不因写面 warnings 变化；`ArchiApiFindingKind::ALL` 为**十值**、`ArchiDriftKind::ALL` 仍五值；源码中 `edit/` 目录零 `drift::check` 引用

## REMOVED Requirements

### Requirement: 写面反馈判据（七条 warnings 与降噪不变式）

**Reason**: 七条里有三条建在 `doc` 字段上（「`api = none` 且 `doc` 空」「`doc` 指不到盘 ⇒ `DocNotFound`」「由 `doc` 诱发 `ApiUnexpected`」）——`doc` 字段在本变更惰性化（零判据读者），`DocNotFound` 契约判据值随之删除；另两条的事实源各自收成单义：`DocMissing` 判派生 `<FQN>.md` 不在盘（单成因、与 `api` 无关，写面只由 `created` 诱发），`ApiUnexpected` 判 `api = none` ∧ schema 在盘；路径派生点由 `contract_path` + `doc_path` 收成 `contract_path`（文档）+ `schema_path`（接口）。同批以 ADDED「写面反馈判据（五条 warnings 与降噪不变式）」取代，三通道语义、降噪不变式与「不改判据面」三条纪律逐字保留。

**Migration**: `crates/core/archi/src/edit/**` 的条件表由 7 行删为 5 行；`DocNotFound` 在写面 warnings 的 `kind` 集合中不再可能（该值已不在 `ArchiApiFindingKind`）；写工具响应的 `warnings` 恒在场与降噪不变式零改动；工具描述与 `archi_element` 的响应形状（`warnings` / `changed` / `counts`）逐字不变。

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/edit/**`（写面 warnings 推导） | 五条事实 → `warnings` | 条件表 5 行（`noPath` / `pathNotFound` / `docMissing`（仅 `created`，判派生 md）/ `apiUnexpected` / delete 后的 `apiUnexpected`）；`kind` 取自两个 `as_str`、`severity` 取自 `severity()`；路径经 `contract_path`（文档）+ `schema_path`（接口），`doc_path` 已退役；零 `drift::check` 调用 |
| `crates/core/archi/src/model.rs` / `parse/toml.rs` | 字段面（`doc` 语义） | 字段与值域零改动；`doc` 的「只有 `api = none` 可写」降为**约定**（无机械判据）、`doc` 零判据读者——doc 注释与字段说明同批改写；八字段顺序与 `ELEMENT_FIELD_NAMES` 零变化 |
| `crates/core/archi/src/dto.rs`（词汇面） | 消费面词汇 | `DiagnosticKind` 九值 / `EdgeMark` 四值 / `NodeMark` 两值逐字不变；契约判据词汇面的十值归 `desktop-archi-api-check`（本能力只作互不混装的对拍引用） |
| `.dev-team/architecture/models/**` | 数据面（零改动） | 11 份资产零正文改动；`doc` 使用仍为 0；`api` / `path` / `entry` 逐字不变 |

### API: 写面响应（形状零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_element` 响应的 `warnings` | 取值面收窄 | 恒在场；`kind` 只可能取 `noPath` / `pathNotFound` / `docMissing` / `apiUnexpected` 四值（`DocNotFound` 不再出现） |
| `archi_relation` 响应的 `warnings` | 零变化 | 恒为空数组 |
