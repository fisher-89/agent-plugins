# desktop-archi-agent Specification（delta: archi-scaffold-field-dictionary）

## Purpose

本 delta 把 `archi_scaffold` 的草稿从「手写模板」升格为**字段词典**——八个字段面字段各一行纪律（值域 / 缺省 / 基数 / 单值 / 形态）+ 关系四键，并把一致性锚从「人记得同批改」换成「字段面的视图」：

- **补三处值面缺口**：`entry` 的「单值」（散文删掉后无归宿）、`metadata` 的「值 MUST 是字符串（非字符串值被拒）」、`apis` 的三条（逐项非空 / 同名不重复 / `apis = []` 与省略同义、显式空数组被拒）。
- **新增机械锚**：草稿逐字段行首在场（遍历 `ELEMENT_FIELD_NAMES`，子串不顶替）、值域逐值在场（`ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` + 缺省档）——新增字段 / 新增变体未补词典即红。
- **描述面去枚举**：工具描述改词典措辞，不再列具体字段名（字段面扩容时不会静默陈旧）。

知识骨架（`archi-knowledge.md`）、相位 prompt 与 `archi_module` 描述不在本 delta 内——「取词典」的散文措辞与 827 B 字段纪律压缩归主篇切片 A。

## MODIFIED Requirements

### Requirement: archi_scaffold 只读工具的新元素模板契约

SDK 引擎 SHALL 提供专用只读工具 `archi_scaffold`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**给字段词典**（要写新元素但记不住字段形态）——草稿同时是最新字段面的可抄改模板，一个工具一个形状（MUST NOT 另设 `mode = "draft" | "dict"` 双形态，亦 MUST NOT 改出纯 Markdown 词典）：

```jsonc
{ "name": "archi_scaffold",
  "parameters": { "kind": "可选。元素 kind 四值之一（值集由 ElementKind::ALL 派生）；省略取 domain" } }
```

- **值集封闭且派生**：`kind` 的合法值 SHALL 由 `ElementKind::ALL` 派生（MUST NOT 手抄四值）；词汇外值 SHALL 以 `is_error` 回灌并**点名**合法值（MUST NOT 静默退化为缺省）。
- **响应形状**：`{ "draft": "<TOML 片段>" }`——**一段**元素表模板，字段按模型字段面顺序列出（`kind` 实值；`gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 以注释行给出并各附一行纪律），另附一条 `[[relation]]` 模板注释。字段顺序 SHALL 与 `archi::parse::ELEMENT_FIELD_NAMES` 同序（= `RawElement` 声明序，见 `desktop-archi-model`「元素字段面的机器可读清单」）。
- **描述面不枚举字段名**：工具定义（`mount.rs` 的 `archi_scaffold_definition()`）的描述 SHALL 以词典措辞表述（「字段词典 / 逐字段一行纪律 / 值域 / 关系四键」），MUST NOT 内联枚举具体字段名（如「字段序 / 内联 `metadata` / `path` 必填提示 / 关系模板」）——枚举会随字段面扩容静默陈旧，与 `archi_module` 描述「MUST NOT 复制字段语法长表」同一条纪律。
- **字段词典语义（每个字段一行纪律）**：草稿 SHALL 对八个字段面字段逐一到场并各给一行纪律，覆盖四类事实（字段各取其一或多项）——① 值域（`kind` 四值语义 / `gate` 三值 / `api` 二值）；② 缺省档（`gate` 缺省 `off` / `api` 缺省 `declared` / `entry` 与 `doc` 选填）；③ 基数与形状（`path` 随 `kind` 的基数、`./` 前缀、目录尾 `/`、文件到扩展名）；④ 单值性（`entry` / `doc` 是单值）。纪律 MUST 与模型侧判据同源：`metadata` SHALL 写明值 MUST 是字符串（数字 / 布尔等非字符串值被拒）；关系 `apis` SHALL 写明四条——只写名字（禁签名 / 类型形状 / `::` 限定路径 / 空白）、逐项非空、同名不重复、`apis = []` 与省略同义（显式空数组被拒）。
- **字段面全集对拍**：草稿 SHALL 含字段面全集的逐字段行——断言 SHALL 遍历 `archi::parse::ELEMENT_FIELD_NAMES` 做**行首匹配**（`{field} = ` / `# {field}：` / `# {field} = `），MUST NOT 退化为整串 `contains(field)`（`apis` 会让 `api` 假命中，`doc` 一类行内复述会掩盖整行缺失）。消费侧 MUST NOT 手抄八个字段名字面量。
- **值域派生对拍**：草稿的门 / 状态纪律行 SHALL 逐值覆盖 `GateTier::ALL`（`off` / `warn` / `strict`）与 `ApiStatus::ALL`（`declared` / `none`），且缺省档（`off` / `declared`）以「缺省」字样在场；`kind` 四值语义由 `ElementKind::ALL` 逐值覆盖。三组枚举新增变体而未补词典即红。
- **模板自证可解析且零结构诊断词**：草稿 SHALL 为合法 TOML 且经 `archi::parse` 的 `TomlParser` 解析后**零结构诊断**（机械锚：字段名 / 键形 / `metadata` 形态 / `kind` 四值语义任一漂移即红）。纪律性提示 SHALL 只出现在注释行——MUST NOT 以注释外的非法行（如 `path = "<待填>"`）破坏可解析性。草稿 MUST NOT 含结构诊断词（`invalidValue` / `unknownField` / `missingField` / `syntaxError` 零命中）：值面规则只写行为（「被拒」），诊断词表不进词典（与「结构诊断全量词表 MUST NOT 进知识文本」同源）。
- **零写盘**：本工具 MUST NOT 创建 / 覆盖 / 删除任何文件（与 `archi_api` 的 `bootstrap` 同款「只出 stdout 语义」）；MUST NOT 成为「写模型工具」（写面仍是既有 `write` / `edit`，见「模型 = 源码（写纪律与不设专用写工具）」）。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）；草稿体量 SHALL 远低于该上限（字段词典是「一行一字段」，MUST NOT 演化成判据长表）。
- **只读档位**：归 `ToolTier::Read`（三档全放行），无路径参数（沙箱原样返回、写入保护零射程）。

#### Scenario: 草稿可解析且含必填提示

- **WHEN** 调用 `archi_scaffold()`（省略 `kind`）与 `archi_scaffold(kind = "module")`
- **THEN** 两次均返回单段元素表草稿（后者的 `kind` 行取 `module`）；草稿经 `TomlParser` 解析零结构诊断；`path` 的「模块必填（可多条）/ 包至多一条包根 / `domain` · `composite` 选填」与 `metadata` 的「内联表与子表等价」两句以注释行在场，MUST NOT 再出现「子表会被读成另一个元素」一类假因果

#### Scenario: 模板 kind 行覆盖四值语义

- **WHEN** 遍历 `ElementKind::ALL` 四次调用 `archi_scaffold(kind = …)`
- **THEN** 每次草稿的 `kind` 纪律注释都列出四个值各自的语义（`package` 至多一条包根 / `domain` 领域容器 / `module` 模块 / `composite` 纯分组），MUST NOT 缺任一值——`composite` 不得再是「零承载」的第四值

#### Scenario: 词汇外 kind 被拒

- **WHEN** 调用 `archi_scaffold(kind = "service" )`（词汇外值）
- **THEN** 返回 `is_error` 并点名 `package` / `domain` / `module` / `composite` 四值；零写盘、零副作用

#### Scenario: 模板不写盘

- **WHEN** 在任一相位调用 `archi_scaffold` 并比对调用前后的架构域字节
- **THEN** 磁盘零变化（模型文件、契约资产、生成物均逐字不变）

#### Scenario: 草稿覆盖元素字段面全集（行首匹配，子串不顶替）

- **WHEN** 遍历 `archi::parse::ELEMENT_FIELD_NAMES`（八项：`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）并逐字段在草稿里找**行首形态**（`{field} = ` / `# {field}：` / `# {field} = `）
- **THEN** 每个字段名都恰有一个行首载体（`kind` 是唯一的实值行，其余为注释行）；删掉任一行首载体即红——删 `# api：` 行后 `apis` MUST NOT 顶替 `api`；新增字段而未补词典（草稿缺该字段的行首载体）同样即红

#### Scenario: 值域逐值在场且缺省档可辨

- **WHEN** 调用 `archi_scaffold` 并遍历 `GateTier::ALL` / `ApiStatus::ALL` 检查草稿
- **THEN** `gate` 纪律行逐值含 `off` / `warn` / `strict` 且 `off` 以「缺省」字样在场；`api` 纪律行逐值含 `declared` / `none` 且 `declared` 以「缺省」字样在场；新增任一变体而未补词典即红；同一次响应里草稿对 `invalidValue` / `unknownField` / `missingField` / `syntaxError` 四词零命中

#### Scenario: metadata 与 apis 的值面纪律在场

- **WHEN** 审查草稿的 `metadata` 与关系 `apis` 两段注释
- **THEN** `metadata` 段含「值 MUST 是字符串」与「非字符串值被拒」，且「子表与内联表等价」仍在场；`apis` 段含「只写名字」「逐项非空」「同名不重复」「`apis = []` 与省略同义」「显式空数组被拒」「生成物面留空」「`archi_drift`」；`entry` 的注释行以 `# entry：` 起首且含「单值」（`doc` 行的「单值」MUST NOT 顶替它）；三段均不点名诊断词

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/archi_tool.rs` | 词典正文单点（本变更改五处注释 / 零改执行体） | `SCAFFOLD_TEMPLATE`：`entry` 行补「单值」；`metadata` 两行补「值 MUST 是字符串（数字 / 布尔等非字符串值被拒）」；`apis` 两行补「逐项非空 / 同名不重复 / `apis = []` 与省略同义（显式空数组被拒）」；`kind` 四值语义、`path` 基数、`doc` 指针纪律逐字不动 |
| `crates/infra/agent/src/sdk/mount.rs` | 工具定义面 | `archi_scaffold_definition().description` 改词典措辞（「字段词典 / 逐字段一行纪律 / 值域 / 关系四键」），零字段名枚举；`kind` 参数 enum 仍 `ElementKind::ALL.map(as_str)` |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | 词典断言面 | 新增三支：字段全集行首对拍（遍历 `archi::parse::ELEMENT_FIELD_NAMES`）/ 值域逐值 + 零诊断词 / `metadata`·`apis` 值面纪律；既有九支逐字保留 |
| `crates/infra/agent/src/sdk/mount_test.rs` | 描述断言面 | `archi_scaffold定义描述要素` 的 needle 换词典措辞，并加反向断言（四枚字段名零命中） |

### Function: `crates/infra/agent/src/sdk/archi_tool.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `scaffold_draft(kind)` | 零改动 | 仍为 `SCAFFOLD_TEMPLATE.replace("__KIND__", kind.as_str())`；值集仍由 `ElementKind::ALL` 派生 |
| `scaffold_payload(input)` | 零改动 | `kind` 可选 / 词汇外 `Err` 点名四值 / `{ "draft": … }` 形状不变 |
| `SCAFFOLD_TEMPLATE` 文本 | 改 | 五处注释：`entry`（+单值）、`metadata`（+值域，拆两行）、`apis`（+三条值面，拆两行）；无新增字段行、无新占位符、无裸 TOML 行 |

### API: 工具面

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_scaffold` 描述 | 改 | 由「新元素 TOML 模板（字段序 / 内联 `metadata` / `path` 必填提示 / 关系模板）」改为词典措辞；MUST NOT 再枚举字段名 |
| `archi_scaffold` 参数 / 响应 | 零改动 | `kind` 可选四值（`ElementKind::ALL` 派生）；`{ draft }` 形状、`is_error` 回灌面、`cap_result` 截断单点不变 |
| 五 archi 工具注入矩阵 | 零改动 | `(phase, role)` 矩阵、`ToolTier::Read`、挂载清单与成员校验不变 |

### CLI / Component: 零变化

| 项 | 变化 | 描述 |
|---|---|---|
| CLI / 前端组件 | 零改动 | 无 IPC / DTO / UI 变化；无新增命令 |
| `archi-knowledge.md` 与相位 prompt | 零改动 | 「取词典」措辞与字段纪律压缩归主篇切片 A（本 delta 只保证词典先就位） |
