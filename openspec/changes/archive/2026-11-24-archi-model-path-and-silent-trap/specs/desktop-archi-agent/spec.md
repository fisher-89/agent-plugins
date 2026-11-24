# desktop-archi-agent Specification（delta: archi-model-path-and-silent-trap）

## Purpose

本 delta 让工具面与知识文本同批跟上 `kind` × `path` 判据，并把「静默陷阱」的假因果从 spec / 知识文本 / 模板 / 测试断言里清掉：

- `archi_scaffold` 模板的 `kind` 行覆盖 `ElementKind::ALL` **四值**（补 `composite` 语义）、`path` 行改「模块必填（可多条）/ 包至多一条包根 / `domain` · `composite` 选填」、`metadata` 行改「内联表与子表等价」；草稿对 `package` 仍零结构诊断（0 条 path 合法）。
- 架构会话知识：`path` 一行改基数口径；`kind` 四值语义的 `package` 补「至多一条包根」、`composite` 补「纯分层容器是标准形态」；键形句 MUST NOT 再出现「会被读成另一个元素 / `metadata` 必须内联表」一类假因果，改为「未引号点分 → 整份源解析失败；子表与内联表等价」（骨架不写结构诊断词）。
- 知识分层：C 档补充规则 ① 由「静默错误必须留上下文」改写为「形态错误不静默、但发生在写盘之后」，`metadata` 子表等价、写前无校验点因此留散文；对应 Scenario 更名，断言改为**真实**纪律（不带假因果、不含内联表禁令）。

知识文本体量锚（骨架 2459 B / 全文 2602 B）同批实测更新。

## MODIFIED Requirements

### Requirement: archi_scaffold 只读工具的新元素模板契约

SDK 引擎 SHALL 提供专用只读工具 `archi_scaffold`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**给模板**（要写新元素但记不住字段形态）：

```jsonc
{ "name": "archi_scaffold",
  "parameters": { "kind": "可选。元素 kind 四值之一（值集由 ElementKind::ALL 派生）；省略取 domain" } }
```

- **值集封闭且派生**：`kind` 的合法值 SHALL 由 `ElementKind::ALL` 派生（MUST NOT 手抄四值）；词汇外值 SHALL 以 `is_error` 回灌并**点名**合法值（MUST NOT 静默退化为缺省）。
- **响应形状**：`{ "draft": "<TOML 片段>" }`——**一段**元素表模板，字段按模型字段面顺序列出（`kind` 实值；`gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 以注释行给出并各附一行纪律），另附一条 `[[relation]]` 模板注释。
- **模板自证可解析**：草稿 SHALL 为合法 TOML 且经 `archi::parse` 的 `TomlParser` 解析后**零结构诊断**（机械锚：字段名 / 键形 / `metadata` 形态 / `kind` 四值语义任一漂移即红）。纪律性提示 SHALL 只出现在注释行——MUST NOT 以注释外的非法行（如 `path = "<待填>"`）破坏可解析性。
- **零写盘**：本工具 MUST NOT 创建 / 覆盖 / 删除任何文件（与 `archi_api` 的 `bootstrap` 同款「只出 stdout 语义」）；MUST NOT 成为「写模型工具」（写面仍是既有 `write` / `edit`，见「模型 = 源码（写纪律与不设专用写工具）」）。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）。
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

### Requirement: 架构会话的系统知识注入

`source = "archi"` 的会话 SHALL 注入一段架构会话知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`），内容 SHALL 覆盖以下关注点（**按 A/B/C/D 分层后**的清单，分层判据见「知识分层与工具面同批移动」）：

- **模型位置与聚合（B）**：模型文件位于架构域 `graph/model/` 下（契约资产在 `models/<分层目录…>/`，两目录分家）、按文件名字典序聚合；两处目录路径 SHALL 为**派生所得**的 repo 相对路径（见 `workspace-layout-resolution`），MUST NOT 在产品源码或知识文本中手写目录字面量。契约索引 `INDEX.toml` 的方位 MUST NOT 进知识文本（生成物，零手写面；需要时经 `archi_api` 的 `generatedPath` 取得）。
- **字段纪律（B + C）**：
  - `path`（B，一行 + 工具指路）：**模块必填（可多条 = 若干根的清单）；包至多一条包根（纯容器可留空；≥2 条即模型停摆）**、`domain` / `composite` 选填（缺省空）、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名；容器填了即持文件，子元素依赖它会触发 `dependsOnAncestor`。「这个元素缺不缺 `path`」由 `archi_module` 的 `hasPath` 一次判出；容器空 `path` 的漂移读数按 kind 分档（`domain` / `composite` 零告警）见 `desktop-archi-drift`。
  - `entry`（B，一行 + 工具指路）：只在约定默认指不到时声明（单文件根 = 根文件自身 / 目录型根 = `<root>/index.ts(x)` / crate = `src/lib.rs`）；解析结果与不可用成因由 `archi_module` 的 `resolvedEntry` 给出，MUST NOT 在知识文本里复述三档长表。
  - `doc`（B，一行 + 判据指路）：单值、repo 相对，只有 `api = none` 的元素可写；`declared` 的入口文档恒为派生落点（按 FQN 段链去根与叶分目录、名为 `<FQN>.md`），写了即 `apiUnexpected`。判据长表归 `desktop-archi-api` / `desktop-archi-api-check`。
  - **字段形态与键形（C，散文必留）**：一元素一表头、键为带引号的完整 FQN 单键（未引号点分 `[element.App.Auth]` 使**整份源解析失败**——诊断点名非法子键 `Auth`，不是「读成另一个元素」；骨架 MUST NOT 写结构诊断词，见下「呈现源与对拍源分家」）；`metadata` 的**子表与内联表等价**（`[element."X".metadata]` 写在已声明元素之后零诊断；子表成为唯一声明则缺 `kind`、整份源不解析）；关系写 `[[relation]]`（`from` / `to` / `desc` / **`apis`**）。**字段序与各字段形态的完整模板不入散文**——用 `archi_scaffold` 取（D 档）。
  - **关系 `apis`（B，一行 + 判据指路，本变更新增）**：选填，**只写名字**——`to` 元素的代码导出名（禁签名 / 类型形状 / `::` 限定路径 / 空白），可留空即不声明 API 粒度，`apis = []` 与同名重复被拒；生成物面 MUST 留空（生成物是唯一真相，手写清单必然漂）。两桶结论（用了没声明 / 声明没用）由 `archi_drift` 的 `errors` / `warnings` / `observations` 给出，MUST NOT 在知识文本里复述判据长表，也 MUST NOT 让模型把它当协议正文写（正文仍在契约资产）。
  - **`kind` 四值语义（C）**：`package` = 包（最小编译单元 / 独立依赖管理；**至多一条包根 `path`，纯容器可留空**）/ `domain` = 领域（按业务实体边界聚合；`path` 选填）/ `module` = 目录型模块、视图或单文件组件（`path` 必填，可多条）/ `composite` = 无实际层级的组合（仅把过多的同级元素分类，可嵌套在任意类型之上；`path` 选填；**纯分层容器是它的标准形态**）。四值语义在 md 的「元素类型」段逐值给出（测试遍历 `ElementKind::ALL` 断言值 + 语义在场），不另设 kind 词汇行。
- **层级与缺省（B + C）**：层级由 FQN 最长前缀推导（子元素须先声明祖先——违反时结构诊断 `orphanHierarchy` 由 `archi_module` / `archi_drift` 的 `is_error` 与读数给出）；新元素 `gate` 缺省 `off`（ratchet 逐块收紧）。
- **计数复述单源（B，一行）**：模型文件表头注释里的元素 / 关系数量 SHALL 以工具读数（`archi_drift` 的 `model.elements` / `model.relations`）为准，改模型 SHALL 在同一批内更新表头；MUST NOT 让表头与模型本体各说各话。spec 的计数表与 ADR 的复算表属仓库文档面，知识文本 MUST NOT 复述任何具体数字。
- **架构域看护配置（B，一行）**：`<archi_dir>/config.json` 存在且管 `ignore`（元素对豁免）/ `exclude`（扫描排除），未知键与缺 `reason` 即配置非法——非法时 `archi_drift` 以 `is_error` 点名具体条目；MUST NOT 在知识文本里复述字段长表。
- **边界（C）**：模型里 MUST NOT 写接口 / 导出符号（契约权威在契约资产）；`api` / `entry` / `doc` 是三个**指针**（有无公共 API / 入口在哪 / 入口文档读哪份），MUST NOT 由它们扩张出协议正文；关系 `apis` 是**消费侧的依赖名字清单**（不是提供方的协议正文）——位置在关系、内容只有名字、权威仍在该元素的代码导出面。
- **自检纪律（A，指路）**：改完可**提前**自查——`archi_drift` 取覆盖 / 归属 / 契约读数（`errors` / `warnings` 通常为空，非空即逐条修）、`archi_module` 取结构诊断（带 `file:line`）、契约面 `archi_api`（`action = "check"`）查两轴；生成物过期用本仓既有的生成命令重生成，MUST NOT 手改生成物。**知识文本只给通用指路，MUST NOT 点名本仓特有的命令名**——产品对任意 workspace 生效，生成命令归属各自仓库的文档面。该纪律 SHALL 为纯提示词约束加工具指路；引擎在收口时另有一次兜底自检（见「收口前引擎自检（架构判据读数）」），故自检 SHALL NOT 被表述为「不调工具就不算完」的义务。
- **写面与可见性（C）**：直改主仓（无 worktree、无归档确认），改动随 `git diff` 评审；**禁止修改架构目录之外的文件**（源码 / 配置 / 脚本 / 域外文档一律不改，用户要求亦不越界；生成物重生成除外）；写盘后画布不自动更新，须提示用户点「刷新」。
- **历史会话续接（C）**：续接历史会话时 SHALL 先取当下模型现状（`archi_module` 无参概览或指定元素）再回答「现在是什么样」一类问题，MUST NOT 依赖会话早前的描述。

**呈现源与对拍源分家**：

- **`gate` / `api` 的值域与缺省档 SHALL 由静态骨架 md 的「字段纪律」段各一行承载**（可读性优先）：`gate`：off / warn / strict（缺省 off）；`api`：declared / none（缺省 declared）。测试 SHALL 遍历 `GateTier::ALL` / `ApiStatus::ALL` 与 `default()`，断言两行**同序相等**且缺省档在场；md 缺值、多值或改序即红。
- **`kind` 四值 SHALL 由 md 的「元素类型」段语义 bullet 承载**：每值一 bullet（值 + 语义）；测试遍历 `ElementKind::ALL` 断言值与语义在场，不另设 kind 词汇行。
- **结构诊断全量词表 MUST NOT 进知识文本**：工具按需返回注解与修复手段；散文只保留 `orphanHierarchy` / `dependsOnAncestor` 两个可执行词。知识文本因此**不是** `DiagnosticKind::ALL` 的词汇消费面——诊断词的三面对拍（进程内 / 工具响应 / IPC DTO）归 `desktop-archi-model`。
- **目录路径 SHALL 仍由代码派生**：程序追加段恰为两处——模型声明目录（`archi_graph_model_rel_dir`）与契约资产（`<archi_models_rel_dir>/<分层目录…>/<FQN>.md`）。骨架 MUST NOT 手写目录字面量；**契约索引方位句 SHALL 删除**（生成物，零手写面）。
- `entry` / `doc` / **`apis`** SHALL **不进**任何值域词表：三者的值域是任意 repo 相对路径 / 任意代码标识符（开放集），不是封闭词汇；md 只给纪律句 + 工具指路，判据定义归 `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-drift`，不在知识文本内复制判据长表。
- 知识文本 SHALL 为**静态骨架 md（`include_str!`）+ 两处目录路径段**在装配侧拼成的单条文本；新增 / 删除 `gate` / `api` 词汇而 md 行未同步时 md ↔ `ALL` 对拍红，`kind` 未同步时语义对拍红。

两条工程约束：

- **自证可解析**：字段形态的机械锚 SHALL 由 `archi_scaffold` 的草稿承担（`TomlParser` 零诊断，见「archi_scaffold 只读工具的新元素模板契约」）；知识文本本身 MUST NOT 再承载示例 TOML（示例是重复的第二事实源）。
- **尽力增强、不炸 run**：知识拼装失败（派生异常 / 读取失败）SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符（路径由派生注入），体量 SHALL 受上限约束（**骨架 ≤ 2700 B、全文 ≤ 2850 B，实测值冻结**；值域收敛为字段行后回落，防与 `AGENT.md` 合体后挤压上下文）。超限 SHALL 压缩同段冗余表述而不是抬高锚；收口自检的装配与本知识解耦（`before_done` 档位按来源置位），知识降级 MUST NOT 影响自检。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 字段值域由 md 承载并与 ALL 对拍

- **WHEN** 检查知识文本的 kind 语义段与 `gate` / `api` 字段行，并新增一个 `gate` 档位（同时补 `GateTier::ALL` 与 md 行）
- **THEN** md ↔ `ALL` 对拍断言 `gate` / `api` 行**同序相等**且缺省档在场；kind 由语义 bullet 对拍；md 少写 / 多写 / 改序 / 漏改缺省档均红；结构诊断全量词不得出现在知识文本（只允许 `orphanHierarchy` / `dependsOnAncestor` 两个可执行词）；`entry` / `doc` / `apis` 不出现在任何值域行（开放值集，零词表）

#### Scenario: 字段纪律走工具指路而非长表

- **WHEN** 审查知识文本的字段纪律段
- **THEN** `path` / `entry` / `gate` / `api` / `doc` 各为一行纪律 + 工具指路（`hasPath` / `resolvedEntry` / `apiUnexpected`）；零 `entry` 三档长表、零字段序清单、零示例 TOML 围栏；`kind` 四值语义、引号 FQN 单键、内联 `metadata`、层级先声明四条散文在场

#### Scenario: 关系键面与 apis 纪律在场

- **WHEN** 审查知识文本里关系相关的那一段
- **THEN** `[[relation]]` 的键面为四键（`from` / `to` / `desc` / `apis`）；`apis` 的纪律句含「只写名字」「可留空」「生成物面留空」三条关键词，且不含任何判据长表（两桶 / 档位 / 白名单长句零命中）；同样的句子里零示例 TOML 围栏（键名只作行内引述）

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 五 archi 工具不变），MUST NOT 抛装配错误；收口前自检照常装配（`before_done` 档位与知识解耦；T1 / T2 皆假时仍零 notice）

#### Scenario: 自检纪律与可见性在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完可调 `archi_drift` 取读数并按条修」「结构诊断经 `archi_module` 带 `file:line`」「契约面可调 `archi_api` 查两轴」「生成物过期用本仓的生成命令重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」六条；且知识文本内零 `<>` 占位符

#### Scenario: 体量与目录方位实测

- **WHEN** 取 `knowledge()` 输出与骨架字节
- **THEN** 骨架 ≤ 2700 B、全文 ≤ 2850 B 且实测值被冻结断言；md 含 kind 语义与 `gate` / `api` 值域及两处缺省档（结构诊断全量词表不得出现），程序追加段恰含模型声明目录与契约资产两处方位，**零契约索引串**

### Requirement: 知识分层与工具面同批移动

架构会话知识文本的内容面 SHALL 按**可判定性 × 时机**两轴分四档，并把每一句的落档写进本能力（分层判据单点，MUST NOT 在别处另立一份分类）：

| 档 | 判据 | 承载 |
|---|---|---|
| **A 工具承载** | 机器能算 ∧ 动作后才需要 | 工具响应 / `Err`（MUST NOT 在知识文本里复述读数） |
| **B 上下文·一行指针** | 机器能算 ∧ 动作前就需要 | 知识文本**一行**，内容是「何时查哪个工具」或一个派生串 |
| **C 上下文·散文纪律** | 不可判定（语义 / 意图） | 知识文本散文 |
| **D 模板工具** | 不可判定 ∧ 动作后才需要（形态） | 模板工具输出（`archi_scaffold`） |

- **A 类句子 MUST 携带工具指路且工具面确有该字段**：知识文本里每条「来自 A 档」的纪律 SHALL 点名承载它的工具（`archi_module` / `archi_drift` / `archi_api` / `archi_scaffold`），且该工具的响应或 `Err` 面 SHALL 确有对应字段 / 判据。任一方向缺失（句子里点了不存在的字段、工具加了字段但句子没指路）即违约——由机械断言表守（一行为「纪律句关键词 → 工具名 → 工具面字段」）。
- **三条补充规则 SHALL 逐条在案**：
  1. **形态错误不静默，但发生在写盘之后**：未引号点分表头 `[element.App.Auth]` 归 `unknownField`、**整份源解析失败**（全或无）；`metadata` 子表与内联表**等价**、零诊断。两者都有**行为**测试钉住（`parse/toml_test.rs` 的 `未引号点分表头被拒且不与合法形态合并` / `metadata子表与内联表等价` / `metadata子表成为唯一声明即缺kind`）。归 C 档的理由仍成立但须换成**动作前不可判定**（写面是自由文本 `write` / `edit`，结构诊断只在写盘后可得；四档表的「不可判定」即此义），MUST NOT 再表述为「会被静默读成另一个元素」或「`deny_unknown_fields` 不触发」——后者已被行为测试否证，不是任何档位的依据。
  2. **写入定位必须留一行**：新元素的模型落点（模型目录）与手写契约的落点（契约资产目录）都在动作前需要，且新元素的契约三路径为 `null`（`api.contractPath` 指不到不存在的东西）⇒ 归 B 档，各留一行。
  3. **`kind` 语义不可脚本化**：「容器 = `domain` / crate、目录与单文件组件 = `module` / 包根 = `package`」是架构判断，工具只能事后判「和邻居不像」⇒ 归 C 档，散文必留。
- **移动分层的原子性**：把某句从散文改判为 A/D 档时，**同批**必须落地「工具面确有该字段 / 模板」与「散文句替换为指路句」两半——只删不加（知识缺失）与只加不删（知识重复）同样违约。
- **本变更的移动清单**（机械断言逐项在案）：示例 TOML → `archi_scaffold`；字段语法形态 → `archi_scaffold` + `archi_module` 字段清单；`entry` 三条纪律 → `archi_module.resolvedEntry`；`doc` 仅 `api = none` 可写 → 判据面 `apiUnexpected` + 一行指路；祖先先声明 → 结构诊断 `orphanHierarchy` + 一行指路；`path` 缺省判定 → `archi_module.hasPath` + 一行指路。

#### Scenario: A 类句子逐句有工具可指

- **WHEN** 对知识文本的 A 档纪律句逐条取「关键词 → 工具名 → 工具面字段」三列核对
- **THEN** 每条纪律句都点到承载它的工具名，且该工具的响应 / `Err` 确有对应字段（`hasPath` / `resolvedEntry` / 结构诊断 `kind` / 判据 `kind`）；任一列为空即测试红

#### Scenario: 形态纪律与 kind 语义留在散文

- **WHEN** 审查知识文本的 C 档
- **THEN** 带引号 FQN 单键纪律（成因写「未引号点分 → 整份源解析失败、诊断点名非法子键」）、`metadata` 子表与内联表等价的陈述、`kind` 四值语义逐值在场，且骨架零结构诊断词；MUST NOT 出现「子表 / 点分嵌套会被读成另一个元素」的假因果，也 MUST NOT 出现「`metadata` 必须内联表」的禁令

#### Scenario: 只删不加即违约

- **WHEN** 从知识文本删掉示例 TOML 而未同时落地 `archi_scaffold`（或反之：加了模板工具但散文仍抄一份示例）
- **THEN** 视为分层未完成——机械断言表对「移动清单」逐项要求两侧同时在场（模板工具 + 指路句），单侧缺失即红

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/archi_tool.rs` | 模板单点（本变更改三行注释） | `SCAFFOLD_TEMPLATE`：`kind` 行四值齐列（`package` 至多一条包根 / `domain` 领域容器 / `module` 模块 / `composite` 纯分组）、`path` 行按新基数（模块必填可多条 / 包至多一条包根 / `domain`·`composite` 选填）、`metadata` 行写「内联表与子表等价」；`scaffold_draft` 的 `__KIND__` 替换与 `ElementKind::ALL` 派生不变；草稿对四值仍零结构诊断 |
| `crates/infra/agent/src/archi-knowledge.md` | 知识骨架（三处改写 + 体量锚） | ①「位置与键形」键形句：未引号点分 → 整份源解析失败（诊断点名非法子键；骨架不写诊断词）；`metadata` 子表与内联表等价；② 字段纪律 `path` 首条：模块必填（可多条）/ 包至多一条包根（纯容器可空）/ `domain`·`composite` 选填；③ 元素类型 `composite` bullet 补「纯分层容器是标准用法」；骨架 2459 B / 全文 2602 B 两个冻结锚同批实测更新 |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 知识断言面 | `骨架静默陷阱四条在场` 改名（去「静默」）并把 `内联表` needle 换为 `子表与内联表等价`；四条 forbidden needle（`assert!(!source.contains(…))`）与说明注释**保留**——它们是「骨架零假因果」的防线，不属待清残留；A 档指路表、体量锚与降级用例的其余断言不变 |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | 模板断言面 | `archi_scaffold三kind逐值` 改遍历 `ElementKind::ALL`；`叶元素必填` needle 改 `module 必填`；新增/保留「kind 四值语义在场」断言 |

### Function: `crates/infra/agent/src/sdk/archi_tool.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `scaffold_draft(kind)` | 零改动 | 仍为 `SCAFFOLD_TEMPLATE.replace("__KIND__", kind.as_str())`；`kind` 值集仍由 `ElementKind::ALL` 派生 |
| 模板文本 | 改 | 三行注释（`kind` / `path` / `metadata`）；无新增字段行、无新占位符 |

### API: 工具面（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_scaffold` 参数 / 响应 | 零改动 | `kind` 可选四值（`ElementKind::ALL` 派生）；`{ draft }` 形状、`is_error` 回灌面、`cap_result` 截断单点不变 |
| 五 archi 工具注入矩阵 | 零改动 | `(phase, role)` 矩阵、`ToolTier::Read`、挂载清单与成员校验不变 |

### CLI / Component: 架构会话知识

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_knowledge::knowledge()` 装配 | 零改动 | 骨架 + 两处派生目录段的拼装、失败降级 `None` 与 `SessionInjections.preamble` 合并路径不变 |
| 前端组件 | 零改动 | 无 IPC / DTO / UI 变化 |
