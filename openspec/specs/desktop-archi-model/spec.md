# desktop-archi-model Specification

## Purpose

定义架构模型的格式与解析契约：`<archi_dir>/models/*.toml` 的「一元素一表头」格式与字段封闭、多文件聚合与声明位置回填、结构诊断九类分工（parser 免费组 / archi 自实现组）、`ModelGraph` 单点构建与三消费面裁剪型投影、消费面词汇集对拍（四面，含架构会话知识文本）、gate 写回纯函数、模型结构 diff 与五命令 IPC 契约。

## Requirements

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
- **值域封闭**：`kind` 必填且 SHALL 为 `package` / `domain` / `module` / `component` 四值之一；`gate` 可选且 SHALL 为 `off` / `warn` / `strict` 三值之一（缺省 `off`）；`path` / `docs` SHALL 为字符串数组（缺省空）；`summary` 可选字符串；`metadata` SHALL 为字符串→字符串的自由表（未知键忽略，非字符串值即 `invalidValue`）。
- **未知字段拒绝**：元素表内出现未列出的字段（如 `gatte`）SHALL 由 `deny_unknown_fields` 拒绝（`unknownField`）。
- **属性与语义分离**：`gate` 档位 SHALL 是模型属性（本能力定义其值域与缺省）；档位的**语义消费**（参与归属、违例定级、门禁结论）归 `desktop-archi-drift` 与 `desktop-archi-gate`，本能力 MUST NOT 定义任何定级规则。
- **协议不经模型**（知情边界，防误加字段）：元素上 MUST NOT 出现接口 / 导出符号 / 签名一类协议字段。契约权威在 `openspec/specs/` 与 ADR；模型内再存一份必然漂移，且需要 Rust / TS 两套提取器对齐——该方案已被否决，重提须独立变更并先回答「契约权威在哪」与「两套提取器怎么对齐」两问。协议只经 `docs` 引用承载。
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

### Requirement: 多文件聚合与声明定位

解析层 SHALL 聚合 `<archi_dir>/models/` 下全部 `*.toml`，按文件名**字典序**读取并合并为一个 `Model`。每个元素与每条关系 SHALL 携带声明位置：

- `modelFile` = 声明所在文件的 **repo 相对路径**（`/` 分隔）；
- `modelLine` = 元素表头键所在行（1 起始）或关系条目 `from` 键所在行。

位置 SHALL 取自 `toml` + serde 反序列化同时产出的 span（`span` → 行列换算），MUST NOT 以再扫文本、模糊匹配或二次 grep 反推。关系级 span SHALL 保留可选位（`Option<SourceSpan>`），当前只消费行号、不消费列区间。

**跨文件重复 FQN**：同一 FQN 在一个文件内重复声明由 TOML map 语义天然拒绝；**跨文件**重复 SHALL 由聚合层自实现检出，诊断 `kind = duplicateElement`，`file` / `line` SHALL 指向**后出现**的那条声明（字典序靠后的文件内位置）。聚合层 MUST NOT 静默取其一。

`models/` 目录缺席或为空 SHALL 解析为**空模型**（零元素零关系零诊断），MUST NOT 报错——空模型是冷启动的合法起点。

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

- **WHEN** `App.Auth` 依赖自己的祖先 `App`（`from = "App.Auth"`、`to = "App"`）
- **THEN** 返回 `dependsOnAncestor` 诊断并指明元素与祖先

#### Scenario: 未定义引用被检出

- **WHEN** 模型声明 `from = "App.Auth"`、`to = "App.Shared"` 而无 `[element."App.Shared"]`
- **THEN** 返回 `unknownReference` 诊断，指明引用方 `App.Auth` 与缺失目标 `App.Shared`

#### Scenario: 孤儿层级被检出

- **WHEN** 模型只声明 `[element."App.Auth.Token"]` 而无 `App` / `App.Auth` 声明
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

- **WHEN** 某元素无 `path`、无 `docs`、无 `summary`
- **THEN** 投影输出为空数组 / `null` 的显式字段（不省略字段、不填占位字符串），三面形状一致

#### Scenario: 1-hop 由邻域查询组合

- **WHEN** 以 `App.Auth` 为聚焦元素请求画布数据
- **THEN** 返回集中于该元素的出边、入边与直接子元素；非邻域元素不出现在图数据内（计数可机械验证），且该集合由 `ModelGraph` 邻域查询直接组合

### Requirement: 消费面词汇集一致性对拍

四个消费面（进程内诊断集、`archi_module` 工具响应、IPC DTO、**架构会话知识文本**）SHALL 对同一份模型语料覆盖**相同的词汇集**。需全覆盖的封闭词汇：

```
element kind   package / domain / module / component
gate 档        off / warn / strict
结构诊断       syntaxError / missingField / invalidValue / unknownField /
               duplicateElement / cycle / dependsOnAncestor /
               unknownReference / orphanHierarchy
文档诊断       docNotFound
边标记         none / unfulfilled / crossKind / exempt
节点标记       none / unfulfilled
```

对拍 SHALL 以一份**含全部词汇**的模型语料驱动四面投影，断言四面的词汇集相等；新增或删除任一词汇而语料 / 投影未同步时对拍 MUST 变红（防「新增一个诊断类型、UI 静默不显示」）。对拍 SHALL 为机械测试（形态沿用 `src/views/changes/flow/run-state.test.ts` 的全词汇出图先例：一份语料走全投影、逐词汇断言在场）。

- **`ALL` 全量常量为唯一派生源**：`element kind` / `gate` 档 / 结构诊断三类封闭词汇 SHALL 各自提供全量常量（`ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL`；其中 `DiagnosticKind::ALL` 由本变更补齐为九值），且 SHALL 与各自的线格式词函数（`as_str`）逐值对拍。派生型消费者（当前唯一 = 架构会话知识文本，见 `desktop-archi-agent`「架构会话的系统知识注入」）MUST 从这些常量取词，MUST NOT 在产品源码或静态文本里手抄一份词表。
- **知识文本纳入射程**：架构会话知识文本出现在四面之列——它必须含全部 `kind` / `gate` / 结构诊断词；新增词汇而知识文本未同步（因为绕过了 `ALL`）MUST 由对拍或 `ALL` 对拍变红。

可选字段的**线形态**（缺字段 vs `null`）不在本对拍射程内（见 proposal R-3）；本对拍只锚定词汇集。

#### Scenario: 语料含全词汇

- **WHEN** 检查对拍语料
- **THEN** 上述每一词汇至少各一条在场（`kind` 四类元素、`gate` 三档、九类结构诊断各一坏点、`docNotFound` 一例、边标记四值、节点标记两值）

#### Scenario: 三面（诊断 / 工具 / IPC）词汇集相等

- **WHEN** 同一语料分别经进程内诊断集、`archi_module` 响应与 IPC DTO 投影
- **THEN** 三面枚举出的词汇集完全相等；任一面的缺失项使测试失败并点名缺失词汇

#### Scenario: 新增词汇未同步即红

- **WHEN** 代码新增一个诊断类型或标记值而语料 / 对拍未同步
- **THEN** 对拍失败（词汇集不等），MUST NOT 静默通过

#### Scenario: 知识文本与三面同词汇集

- **WHEN** 取架构会话知识文本（`source = "archi"` 的 `preamble` 内容）并枚举其词汇
- **THEN** 其中含 `kind` 四值、`gate` 三值（含缺省 `off`）、结构诊断九词，与另三面词汇集相等；把某个词从 `ALL` 删掉而知识侧仍能拼出该词（手抄）时测试红

#### Scenario: ALL 与线格式词逐值对拍

- **WHEN** 遍历 `ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL`
- **THEN** 每一项的 `as_str` 产出唯一且与语料词一致；新增变体未补入对应 `ALL`（长度断言）时测试红

### Requirement: gate 档写回纯函数

`archi` SHALL 提供 gate 写回的**纯函数**：输入源文本 + 元素 FQN + 目标档位，输出改写后的完整源文本或显式 `Err`：

- 渲染 SHALL 经 `toml_edit`（解析 → 改 AST → 渲染），MUST NOT 字符串正则替换；改写后注释、空行与其余元素的原始排版 SHALL 原样保留。
- 前置校验 SHALL 覆盖：元素存在（缺席即 `Err` 并附现有 FQN 清单）、目标值 ∈ `off` / `warn` / `strict`（非法值即 `Err`）。
- 渲染结果 SHALL **自证可解析**（渲染后重新解析一次），解析失败即 `Err` 且不产出文本——MUST NOT 把坏文本交给调用方落盘。
- 该函数 MUST NOT 触碰文件系统（读 / 写盘归命令层）；`archi` 内除解析与关联读盘外 MUST NOT 出现写盘调用。

#### Scenario: 改档保留注释与其余内容

- **WHEN** 对含注释、空行与多个元素的源文本把 `App.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 返回文本中该元素 `gate = "strict"`，其余元素、注释、空行与键序逐字保留；再解析该文本得到新档位且零诊断

#### Scenario: 元素缺席与非法值显式拒绝

- **WHEN** 分别以不存在的 FQN 与 `gate = "tight"` 调用
- **THEN** 均返回 `Err`（前者附现有 FQN 清单或可诊断提示，后者列出三档合法值），零文本产出

#### Scenario: 渲染不可解析即不产出

- **WHEN** 渲染路径产出的文本无法被解析器读回
- **THEN** 返回 `Err` 且不产出任何待落盘文本（自证不可解析即失败），调用方零写入

### Requirement: 模型结构 diff

`archi` SHALL 提供两模型的结构 diff 纯函数，输出：元素新增 / 删除计数与 FQN 清单、关系新增 / 删除计数、`gate` 变更清单（FQN + 旧值 + 新值）。diff SHALL 以 FQN（元素）与 `(from, to)`（关系）为身份键，MUST NOT 以文件或行号为身份。

diff 的消费面为归档确认面的「架构变更」区块（数据源经命令面取数）；两侧模型的**取自哪个工作根**不由本函数决定（主仓基线 vs worktree 的取数归命令层）。

#### Scenario: 增删与改档计数

- **WHEN** 对基线与新模型（新增 `App.Billing` / `App.Billing.Invoice`、删除 `App.Legacy`、关系 +3 −1、`App.Billing` 的 `gate` 由 `off` 改为 `strict`）求 diff
- **THEN** 输出元素 +2 / −1（含 FQN 清单）、关系 +3 / −1、`gate` 变更一条（`App.Billing`：`off` → `strict`）

#### Scenario: 等价模型零差异

- **WHEN** 两侧模型元素 / 关系 / 档位完全相同（文件划分或行序不同）
- **THEN** diff 输出全零（元素 / 关系 / `gate` 三组均空），MUST NOT 因文件或行序差异报告变更

### Requirement: IPC 命令面契约（五命令）

命令层 SHALL 提供五个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff 全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + `docs` |
| `archi_read_doc` | `root` + `path` | 文档正文：仅接受**关联集内**路径（服务端现算关联集后校验），越界显式 `Err` |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（元素存在 / 值合法 / 渲染自证可解析）通过后原子写回该模型文件；失败零写入并显式 `Err` |

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。
- `archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary` MUST NOT 写盘；唯一写命令 SHALL 为 `archi_set_gate`（且只改 `gate` 单字段）。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（只读视图约束由前端承载、由命令可用性背书——命令本身不区分模式）。
- 五个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（生成物随重导出入库，过期由提交前人工核对）。

#### Scenario: 读命令零写盘

- **WHEN** 依次调用 `archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary`
- **THEN** 磁盘字节零变化（模型与文档均只读）；取数结果与 `archi` 投影逐字段一致（命令层零改写）

#### Scenario: 正文取数越界拒绝

- **WHEN** 以关联集之外的路径调用 `archi_read_doc`（如 `openspec/changes/x/proposal.md`）
- **THEN** 返回显式 `Err`，不返回任何文件内容

#### Scenario: gate 写回原子与零残留

- **WHEN** 调用 `archi_set_gate` 改合法元素的 `gate`，以及以非法值 / 不存在元素调用
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/Cargo.toml` | crate 落位与依赖 | 依赖 `foundation` + `serde` / `serde_json` / `specta` + `toml` / `toml_edit` / `petgraph` / `ignore`（均取自锁内既有版本，零新增包）；零 Tauri、零 store、零 spawn、零 `workflow` 依赖 |
| `crates/core/archi/src/model.rs` | 格式无关 AST | `Model` / `Element`（`fqn` / `kind` / `gate` / `path` / `docs` / `summary` / `metadata` / `file` / `line`）/ `Relation` / `SourceSpan` / `Diagnostic`；`derive PartialEq`（UI diff 预留）；关系级 `Option<SourceSpan>` 占位 |
| `crates/core/archi/src/parse/mod.rs` | 多文件聚合 | `trait SourceParser { fn parse(&str) -> ParseOutcome }`；`parse_dir(models_dir) -> ParseOutcome`：字典序、跨文件 `duplicateElement`、位置回填；空目录 → 空模型 |
| `crates/core/archi/src/parse/toml.rs` | TOML 实现 | `toml` + span 反序列化；`deny_unknown_fields`；`kind` / `gate` 封闭枚举；parser 错误 → 结构化诊断（保留原文与 line/column） |
| `crates/core/archi/src/graph.rs` | 图单点 | `ModelGraph`（petgraph）唯一构建点；SCC 成环 / 依赖祖先 / 未定义引用 / 孤儿层级；`depends_on` / `depended_by` / `children` / `neighborhood`（1-hop） |
| `crates/core/archi/src/dto.rs` | 三面共用投影 + 封闭词汇单点（补全量常量） | `ArchiGraph`（含停摆态）/ `ArchiModuleDetail` / `ArchiSearchResult` / `ArchiChangeSummary`；封闭词汇枚举 `ElementKind` / `GateTier` / `DiagnosticKind` / `EdgeMark` / `NodeMark`（`specta::Type`）；`ArchiDiagnosticKind::ALL: [Self; 9]`（与 `as_str` 同源；长度与词面双向对拍；既有 `ElementKind::ALL` / `GateTier::ALL` 语义零变）；投影只裁剪，MUST NOT 重算 |
| `crates/core/archi/src/edit.rs` | gate 写回纯函数 | `set_gate(source, fqn, gate) -> Result<String, String>`：`toml_edit` 渲染保留注释空行、前置校验（存在 / 值域）、渲染后自证可解析；零文件系统写 |
| `crates/core/archi/src/diff.rs` | 结构 diff | `model_diff(base, next) -> ModelDiff`：元素增删（FQN 键）、关系增删（`(from,to)` 键）、`gate` 变更；文件 / 行序不参与身份 |
| `src-tauri/src/commands/archi/mod.rs` | 五命令薄包装 | `archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary` / `archi_set_gate`；`Result<T, String>` 错误面；唯一写命令为 `archi_set_gate`（写前校验 + 原子写回）；命令层零领域规则 |
| `crates/core/archi/tests/` | 语料与 golden | `corpus/dsl/<case>/*.toml` + `golden_test.rs` + `vocabulary_test.rs`；golden 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写惯例 |
| `crates/core/archi/tests/vocabulary_test.rs`（扩展） | 四面词汇对拍 | 语料 → 进程内诊断集 / `archi_module` 串 / IPC DTO / 架构会话知识文本四面词汇集相等；`ALL` 与 `as_str` 逐值对拍 |
