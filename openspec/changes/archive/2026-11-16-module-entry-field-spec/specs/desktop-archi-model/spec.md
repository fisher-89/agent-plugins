# desktop-archi-model Specification（delta: module-entry-field-spec）

## Purpose

本 delta 只给元素字段面加**一个可选路径指针** `entry`（单值、repo 相对，语义与四条判据归 `desktop-archi-api`）：值域加一档、「模型不含协议正文」由七项改八项、新增一条 `entry` 三态（省略 → `null` / 单值 / 数组拒收）的解析场景。其余 requirements 逐字不动。

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

[element."App.Backend"]
kind = "domain"

# 目录型壳的约定默认拼成 .../src-tauri/src/src/lib.rs（指不到），故此处显式 entry 是正当用法
# （本变更模型数据零处：显式值随宿主重建后的后续变更落，见 repo-archi-model）
[element."App.Backend.Shell"]
kind = "module"
path = ["./packages/desktop/src-tauri/src/"]
entry = "./packages/desktop/src-tauri/src/lib.rs"

[[relation]]
from = "App.Auth"
to   = "App.Shared"
desc = "复用错误类型"
```

- **键形裁决**：`element` 表的键 SHALL 为**带引号的完整 FQN 单键**（`[element."App.Auth"]`），MUST NOT 用未引号点分形式（`[element.App.Auth]`）声明层级——两种写法并存会使同一层级存在两种拼法、歧义不可判。层级 SHALL 由 **FQN 最长前缀**推导（`App.Auth.Token` 的父亲是 `App.Auth`），MUST NOT 由 TOML 表嵌套推导。
- **字段面封闭**：元素字段 SHALL 为标量 / 数组 / **内联表**三形态；`metadata` MUST NOT 用子表（`[element."X".metadata]`）表达——子表与「子元素表头」同构，会被读成元素 `X.metadata`。
- **值域封闭**：`kind` 必填且 SHALL 为 `package` / `domain` / `module` / `component` 四值之一；`gate` 可选且 SHALL 为 `off` / `warn` / `strict` 三值之一（缺省 `off`）；**`api`** 可选且 SHALL 为 `declared` / `none` 二值之一（缺省 `declared`）；**`entry`** 可选且 SHALL 为**单个**字符串（缺省 `null`，MUST NOT 为数组——数组会让「一个入口只属于一个模块」这条约束不可表达）；`path` / `docs` SHALL 为字符串数组（缺省空）；`summary` 可选字符串；`metadata` SHALL 为字符串→字符串的自由表（未知键忽略，非字符串值即 `invalidValue`）。
- **未知字段拒绝**：元素表内出现未列出的字段（如 `gatte`）SHALL 由 `deny_unknown_fields` 拒绝（`unknownField`）。
- **属性与语义分离**：`gate` 档位 SHALL 是模型属性（本能力定义其值域与缺省）；档位的**语义消费**（参与归属、违例定级、门禁结论）归 `desktop-archi-drift` 与 `desktop-archi-gate`，本能力 MUST NOT 定义任何定级规则。
- **`api` 状态指针（显式字段，非 `metadata`）**：元素 SHALL 有封闭二值的 `api` 字段（`declared` 缺省 / `none`，附 `ApiStatus::ALL`），语义为「本模块是否有公共 API 契约」的状态指针，MUST NOT 承载任何协议正文（签名 / 符号 / 接口 / 规范全文）。走显式字段而非 `metadata` 自由键的理由是可获得 `deny_unknown_fields` 的拼写校验：`api = "non"` 报 `invalidValue` 并列全合法值，而 `metadata = { api = "non" }` 会静默变成「未声明」。完备性判据：每个 `path` 非空元素 ∈ {`none`} ∪ {契约文件存在}，不存在「忘了建契约」这第三种状态（判据归 `desktop-archi-api-check`）。
- **`entry` 是实现入口指针（显式字段，非 `metadata`）**：语义为「本模块的实现入口文件」——**单值**、repo 相对 POSIX 路径（与 `path` 同口径，`./` 前缀由归一剥除、不参与身份）。四条判据（基准 / 落位 = 某条 `path` 或目录型 `path` 之内 / 存在 = 盘上必须有 / 跨元素唯一）与三态解析链（声明 → 语言约定默认 → 无入口）归 `desktop-archi-api`「生成物落点与入口查找」，本能力只定义字段本身的值域与解析面。走显式字段而非 `metadata` 自由键的理由同 `api`：`entry = ["a"]`（数组）与 `entery`（拼错）必须能机械拒收。
- **两个指针都 MUST NOT 承载协议正文**：`entry` 是**位置**指针（文件在哪），`api` 是**状态**指针（有无公共 API）；二者都 MUST NOT 让模型开始携带签名 / 导出符号 / 接口定义。单文件根元素（`views/welcome-view.tsx` 一类）MUST NOT 声明 `entry`——它的根文件就是入口，声明即冗余；显式 `entry` 的正当用法只有「约定默认指不到」的两类：目录型壳（`src-tauri/src/` → 显式 `src/lib.rs`）与二进制 crate（只有 `src/main.rs`）。
- **协议正文不经模型**（知情边界，防误加字段）：元素上 MUST NOT 出现接口 / 导出符号 / 签名一类协议**正文**字段——唯一允许的协议相关字段是上述两个指针 `api` / `entry`。原边界要求的两个问题：**契约权威在哪**——在 `<archi_dir>/api/<FQN>.md`（见 `desktop-archi-api`），模型只存状态指针；**两套提取器怎么对齐**——不提两套：单一 schema + 单一生成器，`scan/rust.rs` / `scan/ts.rs` 是既有取数器（非权威），只供一次性 bootstrap。模型内再存一份协议正文必然漂移，该做法仍被否决；重提须独立变更。
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

#### Scenario: entry 单值可解析且缺省为 null

- **WHEN** 解析三份模型：一份省略 `entry`、一份对目录型壳 `App.Backend.Shell`（`path = ["./packages/desktop/src-tauri/src/"]`）写 `entry = "./packages/desktop/src-tauri/src/lib.rs"`、一份写 `entry = ["./a.rs", "./b.rs"]`
- **THEN** 前者投影出 `entry = null`（字段不省略）；中者逐字读出该字符串（`./` 由归一剥除后参与判据；目录型壳的约定默认指不到，显式值正是本字段的正当用法——14 个 `src/lib.rs` 命中的 crate 与单文件根元素 MUST NOT 声明 `entry`，基线数据纪律见 `repo-archi-model`）；后者报 `invalidValue`（`entry` 是单值，数组形态直接拒收，MUST NOT 静默取首条）

#### Scenario: 模型不含协议正文

- **WHEN** 审查任一模型文件的元素字段面
- **THEN** 元素字段只有 `kind` / `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata` 八项；任何签名 / 导出符号 / 接口定义写在 `<archi_dir>/api/<FQN>.md` 而非模型内

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/model.rs` | 格式无关 AST | `Model` / `Element`（`fqn` / `kind` / `gate` / `api` / **`entry`** / `path` / `docs` / `summary` / `metadata` / `file` / `line`）/ `Relation` / `SourceSpan` / `Diagnostic`；`Element.api: ApiStatus`（缺省 `Declared`）与 `Element.entry: Option<String>`（缺省 `None`，语义归 `desktop-archi-api`）；其余字段名 / 顺序 / 语义零变化 |
| `crates/core/archi/src/parse/toml.rs` | TOML 实现 | `toml` + span 反序列化；`deny_unknown_fields`；`kind` / `gate` 封闭枚举；`RawElement` 含 `api` 与 **`entry`**（`#[serde(default)]`，`Option<String>`）——`entry = ["a"]` 与 `entry = {…}` 由 serde `invalid type` 归 `invalidValue`，拼错的键归 `unknownField`；parser 错误 → 结构化诊断（保留原文与 line/column） |
| `crates/core/archi/src/dto.rs` | 三面共用投影 + 封闭词汇单点 | `ArchiGraph`（含停摆态）/ `ArchiModuleDetail` / `ArchiSearchResult` / `ArchiChangeSummary`；封闭词汇枚举 `ElementKind` / `GateTier` / `ApiStatus` / `DiagnosticKind` / `EdgeMark` / `NodeMark`（`specta::Type`）；`ArchiElement` 增 `entry: Option<String>`（线格式 `entry`，缺省 `null`，**显式字段不省略**——与「投影不补默认值」同款）；既有 DTO 字段名 / 顺序 / 语义零变化；投影只裁剪，MUST NOT 重算（入口解析在 `api::paths`，不在投影层） |
| `crates/core/archi/src/model_test.rs` + `parse/toml_test.rs` + `dto_test.rs` | 模型语料（扩展） | `entry` 三态（省略 → `null` / 单值 / 数组被拒 `invalidValue`）；投影显式 `null` 不省略；既有 `api` / `path` / `docs` 语料逐字保留；`ArchiElement` 字面量补 `entry: None`（机械） |
