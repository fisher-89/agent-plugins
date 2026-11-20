# desktop-config-agent Specification (Delta)

## ADDED Requirements

### Requirement: 配置页常驻 config 会话面板（布局与骨架复用）

`/config` SHALL 在**左栏**提供常驻的 config 会话面板，与右侧配置呈现区并排（`ResizablePanelGroup orientation="horizontal"`），MUST NOT 以抽屉 / 浮层承载对话：

- **骨架复用**：面板 SHALL 经共享骨架 `SessionPanel`（`testIdPrefix = "config-chat"`）渲染头区动作（新会话 / 历史会话下拉）、状态行、历史清单与折叠窄条，MUST NOT 复制第二套会话面板实现；对话体 SHALL 经 `AgentSessionView` 的**注入形态**渲染（来源侧建 `useAgentSession`），composer 经插槽注入，MUST NOT 在视图层自拼消息 / 轮行 / 事件流。骨架契约本身归 `desktop-change-chat`「面板骨架归一」，本能力只作第三处消费者。
- **折叠自持**：折叠态 SHALL 由骨架自持（缺省收起 + 窄条展开入口 + 头区「收起Agent」），配置页 MUST NOT 另设面板显隐开关、MUST NOT 以页面 state 或分栏 `onResize` 像素派生作第二真相源，MUST NOT 为自身修改骨架的缺省折叠形态。
- **呈现区不受损**：右栏 SHALL 保留既有配置分区（基础 / tests / write_protection / 未知字段 / diagnostics 警示）与显式刷新按钮；左栏收起时右栏 SHALL 全宽可用。
- **零取数纪律**：面板收起（窄条）时 MUST NOT 发起任何会话 / 清单取数；面板挂载但处于空态时 MUST NOT 发起重放查询与订阅。

#### Scenario: 分栏与骨架复用的可观测证据

- **WHEN** 打开 `/config` 并展开左栏面板
- **THEN** 左栏为 `config-chat-*` testid 集合（`-actions` / `-collapse` / `-new` / `-history` / `-note` / `-history-list` / `-rail` / `-expand` / `-session` 在案）的共享骨架实例，对话与轮行经 `AgentSessionView` 注入形态渲染；右栏配置分区与 `config-refresh` 齐备

#### Scenario: 收起时不取数

- **WHEN** 页面以默认收起态挂载并保持无交互
- **THEN** 零会话类 invoke（无 `list_config_chat_sessions`、无 `session_detail`、无 `agent_session_watch`）；展开面板后空态亦不发起重放查询

#### Scenario: 折叠真相单一

- **WHEN** 审查配置页组件与骨架组件源码
- **THEN** 折叠态与分栏命令面（`collapse()` / `expand()`）全在骨架内部，页面零折叠入参、零折叠 state、零面板显隐开关

### Requirement: config 会话归属与清单命令

面板的会话 SHALL 以 `source = "config"`、`source_ref = null` 写入会话归属；清单读取 SHALL 经专用薄命令，选中后 SHALL 一律以 `sessionId` 直查重放：

- **写侧归属**：发起 SHALL 携带 `source = "config"` 与 `source_ref = null`（一个 workspace 一份历史，不挂 change / explore / archi 身份）；`cwd` SHALL 传 `null` 使后端回落 workspace root（主仓根），MUST NOT 传 change worktree 一类的替代根。
- **读侧清单**：命令面 SHALL 提供 `list_config_chat_sessions(root, offset, limit) -> SessionPage` 薄包装（等价 `list_sessions(Some("config"), None, offset, limit)` 的分页清单，`source` 词常量收在命令层）；blank root SHALL 返回空页信封；面板 MUST NOT 自拼 `source` / `source_ref` 字符串，MUST NOT 直调通用清单命令（`agent_sessions`）以隐式过滤。
- **空态与可发送性**：打开面板 SHALL 为**空态**（`sessionId = null`：零重放取数、零订阅、零会话行），MUST NOT 打开即自动恢复最近一条历史会话；该空态 SHALL 是可发送的草案态（首发 `sessionId` 仍为 null → 后端 `New` 语义铸行）。
- **新会话与历史会话**：「新会话」SHALL 将 `sessionId` 置 null 并清空对话本地镜像（会话行不预建）；「历史会话」SHALL 为头区下拉清单（点击历史入口打开；清单只在打开时挂载，行内呈完整会话 id + 更新时间 + 轮数），每次打开 SHALL 重取首屏（不清空已加载页），点选 SHALL 以 `sessionId` 直查 `session_detail` 与会话转录重放装载；面板 SHALL 在会话 id 确立或首轮收口时重取清单。
- **零模型字段**：config 会话引用 MUST NOT 落库为 workspace / change 模型字段（零 native model 升级、零 store 迁移）；`session.source` 保持自由字符串语义，不新增枚举 / CHECK。

#### Scenario: 归属写入可查且互不相撞

- **WHEN** 在配置页发出首轮对话并收口
- **THEN** 该会话在库内 `source = "config"`、`source_ref = null`；`list_config_chat_sessions` 返回该会话，而同 workspace 的 archi 会话（`source = "archi"`）、change chat 会话（`source = "change"`、两段 `source_ref`）与 change 流程会话（四段 `source_ref`）一条都不出现

#### Scenario: 空态首发铸会话行

- **WHEN** 面板处于空态时输入并发送首条消息
- **THEN** 发起 invoke 的 `sessionId` 为 null（`New` 语义）、`source = "config"`、`source_ref = null`、`cwd = null`；后端铸造会话行，轮行回写后该会话在历史清单中可点选重开

#### Scenario: 历史点选直查重放

- **WHEN** 在历史清单中点击一条既有 config 会话
- **THEN** 面板以该 `sessionId` 直查 `session_detail` 与转录取数重放装载；选中与重放链路零 `source` / `source_ref` 反查调用

#### Scenario: blank root 返回空页

- **WHEN** 以空白 root 调用 `list_config_chat_sessions`
- **THEN** 返回空页信封（`sessions: []`、`total: 0`），不触库、不 `Err`

### Requirement: config 会话禁发矩阵与状态行

面板 SHALL 以单一判据源决定「可发送 / 可开新会话 / 可切历史」，MUST NOT 由各组件各自判定：

| 本会话库态 `running` | 可发送 | 可开新会话 | 可切历史 |
| --- | --- | --- | --- |
| true | ✗ | ✗ | ✓ |
| false | ✓ | ✓ | ✓ |

- **运行中禁发**：判据 SHALL 为本会话库态 `running`；禁发 SHALL 连「新会话」一并禁，只留历史浏览（MUST NOT 出现「能开新会话但不能发」的半可用态）。架构页把该判据渲染为 `disabled`（输入与发送同禁 + 理由文案），停止入口 SHALL 照常呈现。
- **无归档态、无 flow run 判据**：配置页不承载 change（无 `detail.source = "archive"` 只读态、无在飞 change flow run），MUST NOT 引入这两条判据的占位参数。
- **空态不参与判据**：`sessionId === null` MUST NOT 作为禁用理由，也 MUST NOT 豁免上表任一行。
- **状态行两态**（由来源侧 hook 供 `note`，骨架只渲染）：空态 SHALL 呈草案文案（发送首条消息即开启新会话）；已开会话且无可发送性理由时应呈「配置写回只在提案卡上点『应用』生效」一类提示；禁用理由在场时 SHALL 优先于上述两态。

#### Scenario: 运行中禁发但可读史

- **WHEN** 当前 config 会话库态为运行中
- **THEN** composer 禁用并呈运行中理由，「新会话」禁用，历史清单展开与点选可用，停止入口在场

#### Scenario: 空闲可正常收发

- **WHEN** 无在飞 run、当前会话不在运行
- **THEN** 输入与发送可用，「新会话」可用，历史切换可用；空态同样可发

#### Scenario: 状态行优先级

- **WHEN** 分别观察空态面板、已开会话（空闲）、运行中会话
- **THEN** 依次呈草案文案、写回入口提示、运行中理由；无理由且非空态时不占位

### Requirement: `propose_config` 工具契约

config 会话 SHALL 拿到**唯一**的提案出口 `propose_config`，定义与参数面：

```
propose_config({})                    → Ok { current, candidate: null, changes: [], summary }
propose_config({ candidate: {...} })  → Ok { current, candidate, changes: [...], summary }   ← 零 invalidValue
                                      → Err(is_error) 纯文本逐条诊断（path: message）+「修正后重交完整候选」指引
```

- **两模式**：`candidate` 缺省 = **只读模式**（返回当前盘面，`candidate: null`、`changes: []`）；给出 `candidate` = **校验模式**（`current` 与 `candidate` 同刻同源一并返回）。参数面为 `{ candidate?: object }`、`required: []`；MUST NOT 有路径参数、MUST NOT 接受片段候选（一次全量：候选即用户将要写入的整份文件）。
- **校验回环**：候选 SHALL 经 `config::validate` 判定；存在任一 `InvalidValue` 诊断即 `Err`（`is_error`），文案逐条列 `path: message` 并附「修正后重交完整候选」——脏候选 MUST NOT 进 `Ok` 分支，也 MUST NOT 落盘或落库。
- **两面一致**：输出恒为**单条 JSON 字符串**（`ToolResult.content` 是扁平字符串）且 `current` / `candidate` **一律 file face**（`static_analysis` / `write_protection` / `depends_on` / 展开回顶层的 `extra`），MUST NOT 用 `serde_json::to_value(&WorkspaceConfig)`（wire face）出线。
- **`current` 取法**：SHALL 为「盘面 file face」= 以盘上原文为模板的 `to_file_value` 投影；config.json 缺失即 `{}`；MUST NOT 读页面 hook 数据、MUST NOT 用 wire face。
- **`changes` 与 `summary` 由后端算**：`changes` SHALL 为 `current` → `candidate` 的字段级差异（新增 / 修改 / 删除，path 用 file-face 键路径），`summary` 为一句中文概述；前端 MUST NOT 重算配置语义。
- **零副作用**：一次调用 MUST NOT 写文件、MUST NOT 写库、MUST NOT 改会话状态；候选只活在会话转录里（工具 input / output 已持久化，历史会话重放时卡片照常出现）；一轮内可反复调用，每次产出恰好一条 `ToolResult`。
- **体量**：满配 fixture 的 `Ok` 输出 SHALL < 50 000 B（`MAX_RESULT_BYTES`）——截断即成不可解析串，MUST NOT 靠截断兜底。
- **挂载与档位**：SHALL 为 infra 挂载清单第 14 项（`ToolTier::Read`：三档全放行；执行体 `ToolExec::Config`），SHALL 进「无路径参数」白名单（`input_path` 恒 `None`）——否则候选里出现顶层未知键 `path` 会被沙箱改写面当工具路径改写。工具定义与档位唯一真源在挂载清单，`desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」管装配语义。

#### Scenario: 只读模式返回盘面

- **WHEN** 模型调用 `propose_config({})`
- **THEN** 返回 `Ok` 的 file-face JSON：`candidate` 为 `null`、`changes` 为 `[]`、`current` 与盘上配置文件同内容（键名形如 `static_analysis` / `write_protection` / `depends_on`、`extra` 在顶层展开）；磁盘与库零变化

#### Scenario: 脏候选当场退回

- **WHEN** 模型提交含枚举外 `framework`、越界覆盖阈值与含 `*` 的 `tests[].root` 的候选
- **THEN** 返回 `Err`（`is_error`），文案逐条列 `path: message` 并附「修正后重交完整候选」；磁盘与库零变化，候选项不进转录以外的任何位置

#### Scenario: 干净候选出 file face 与差异

- **WHEN** 模型提交一份合法候选（含 `projects[].language` / `projects[].depends_on` 与 `write_protection.files`）
- **THEN** 返回 `Ok`：`candidate` 为 file face（无 camelCase 多词键、`extra` 已展开回顶层），`changes` 逐项为字段级新增 / 修改 / 删除，`summary` 为一句中文；磁盘仍零变化

#### Scenario: 输出体量不越 L1 上限

- **WHEN** 以满配 fixture（八键全字段 + 多 suite + 多项目 + 多条写保护规则）分别驱动两模式
- **THEN** 两次输出字节数均 < 50 000，且为可解析 JSON（不被 `cap_result` 截断）

#### Scenario: 无路径参数不进沙箱改写

- **WHEN** 候选含顶层未知键 `path`（或被用于 `extra` 的任意键名）
- **THEN** `tools::input_path` 对 `propose_config` 恒返 `None`，候选内容零改写、零沙箱拒绝

### Requirement: `ConfigContext` 铸造与 root 语义

`propose_config` 的执行上下文 SHALL 为 per-turn 铸造的 `ConfigContext { root }`，并以**名字清单包含 `propose_config`** 为唯一铸造判据（MUST NOT 用枚举比较或自由字符串嗅探）：

- **root = 主仓 root**：`ConfigContext.root` SHALL 取 `PumpPayload.workspace_root`（组合根注入的主仓根，也是写保护规则装配根 = 配置权威所在），MUST NOT 取 `SessionCtx.workspace_root`（exec root；change 会话为 worktree）——配置语义是「权威在仓根」，与 archi 语义（「看我现在改的那棵树」）相反。
- **上下文缺席即错误回灌**：`LoopTurn.config` 为 `None` 时调用 `propose_config` SHALL 以 `is_error` 的 `ToolResult` 回灌（生产路径不可达），run 不中断。
- **不新增 crate 依赖边**：`agent-runtime` 已依赖 `config`；工具执行体 MUST NOT 为取配置新增依赖条目。

#### Scenario: root 取主仓而非 exec root

- **WHEN** 构造一次会话的 per-turn 上下文时令 `PumpPayload.workspace_root`（主仓 root）与 `SessionCtx.workspace_root`（exec root）为两个不同路径（现有 config 会话 `cwd = null` 二者同值；该断言把「未来被赋予 worktree 执行根」的形态提前钉住）
- **THEN** 铸造出的 `ConfigContext.root` 为主仓 root（`payload.workspace_root`）：读到的盘面是主仓 `config.json`，exec 根下的同名文件不参与

#### Scenario: 铸造判据为名字清单

- **WHEN** 审查 per-turn 铸造点
- **THEN** `ConfigContext` 只在名字清单含 `propose_config` 时铸造（与 `mint_archi_context` / `DbContext` 并列的名字判据），无枚举比较、无来源字符串嗅探

### Requirement: 配置协议知识的静态 preamble

`source = "config"` 的会话 SHALL 注入一段配置协议知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）：

- **生产者形态**：知识 SHALL 由静态骨架 md（`include_str!`）+ 代码派生词表 + 使用指引在装配侧拼成单条文本；签名 SHALL 无入参、SHALL 零 IO——**盘面真值不进 preamble**（取数时机由 `propose_config` 只读模式承担，避免「preamble 拍 T0 / propose 拍 T1 / apply 拍 T2」的双真值时刻）。
- **骨架关注点**：字段语义与值域（八键）、数值阈值 0–100、`tests[].root` / `projects[].root` 的路径禁令、`projects[].language` 与 `depends_on` 的软规则语义（协议层零校验、归一只在消费侧）、`projects[].depends_on` 是平级边（安装顺序）而非层级来源、`context` / `rules` 当前零工程消费（用户明确要求时才写）、`defaultApplied` 是正常形态（「清零噪音」的准确含义是给**有默认值的**字段写真值，不是给 Option 字段写 `null`）。
- **派生词表**：`KNOWN_KEYS` 八键 ← `known_keys()`；`TestFramework` 八值 ← `TestFramework::ALL`；MUST NOT 手抄。
- **使用指引**：SHALL 说明 `propose_config` 的两模式（不给候选即返回当前盘面 + 诊断；给候选即校验回环）、候选必须是**完整文件面**、`is_error` 时按逐条诊断修正后重交。
- **尽力增强、不炸 run**：拼装失败 SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；文本 MUST NOT 含 `<>` 一类占位符；体量 SHALL 受上限约束（测试锚定，防与 `AGENT.md` 合体后挤压上下文）。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则归 `desktop-agent-execution`）；`desktop-write-protection` 的规则语义 MUST NOT 被复述成第二真相源（知识只提示「`config.json` 是 agent 不可写的文件」这一事实，判据归该能力）。

#### Scenario: 词表由代码派生

- **WHEN** 新增一个 `KNOWN_KEYS` 条目或一个 `TestFramework` 变体而未改知识骨架
- **THEN** 知识文本随即含该词（派生消费面测试变红即说明存在手抄）；把词表改成手写字面量即由该测试判红

#### Scenario: 不含盘面真值与占位符

- **WHEN** 检查知识文本
- **THEN** 文本内零当前配置值（无阈值实际值、无项目列表、无写保护 glob）、零 `<>` 占位符；两模式使用指引与 `defaultApplied` 口径在案

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常
- **THEN** 装配结果为 `preamble = None`，会话照常发起、工具面不变（六名清单仍在），MUST NOT 抛装配错误

#### Scenario: 与 AGENT.md 合为单条 System

- **WHEN** 以 `source = "config"` 发起会话且 workspace root 存在 `AGENT.md`
- **THEN** chat_history 恰有一条 System 消息，内容为「配置协议知识 + 空行 + `AGENT.md` 逐字」（合成规则归 `desktop-agent-execution`）

### Requirement: 写面命令 `workspace_config_apply` 与写权边界

写回 `config.json` SHALL 只有一个入口：用户点击提案卡上的「应用」触发的 `workspace_config_apply(root, candidateFileJson)` 命令。

- **命令体三段**：`check root`（config 轨道内私有 fn：缺失 / 不可读 / 非目录 / 空白 → `Err`）→ `config::plan_apply_config(root, &candidate)`（纯函数、零 IO）→ `write_atomic(config_path(root), content)`，返回**重读**后的 `WorkspaceConfigReport`（前端直接替换，省一次往返）。
- **零 `InvalidValue` 门**：`plan_apply_config` SHALL 先 `validate(&candidate)`；存在任一 `InvalidValue` 即 `Err`（诊断逐条回传）且 **MUST NOT 产出待写内容**（失败零写入：文件内容与 mtime 双证不变）。`defaultApplied` 诊断 MUST NOT 拦截写回（它是正常形态）。
- **双校验不是冗余**：propose 侧的校验可能发生在很久以前、可能来自历史会话重放；apply 侧 MUST NOT 信任调用方（前端载荷可伪造）。
- **入参面**：SHALL 为 file-face `serde_json::Value`（出线 TS `unknown`），MUST NOT 用 typed `WorkspaceConfig` 作入参（wire / file 两面不同名，typed 入参即要求调用方按 camelCase 反向拼装）。
- **落盘形态**：`content` = `serde_json::to_string_pretty` + 尾换行；写入 SHALL 原子（临时文件 + rename），MUST NOT 直写目标文件留半截。
- **写权边界**：`config.json` SHALL 保持「agent 不可写」的内置锁定（`desktop-write-protection` 零改动）；`workspace_config_apply` MUST NOT 作为 agent 工具下发（模型不可调用、不可经 bash 绕过——bash 的写检测仍按内置锁拒绝）；apply 是用户点击触发的应用进程自写，**MUST NOT 注册 store 模型、MUST NOT 落库、MUST NOT 持有缓存**。
- **失败面**：读失败 / 校验失败 / 写失败 SHALL 返回 `Err(String)`（文案含原因），页面 inline 持久呈现；MUST NOT panic、MUST NOT 静默改动文件。

#### Scenario: 零 invalidValue 才落盘

- **WHEN** 以含越界阈值（或枚举外 `framework`）的候选调用 `workspace_config_apply`
- **THEN** 返回 `Err`、诊断逐条在文案内；目标文件内容与 mtime 逐字不变（零写入）

#### Scenario: 成功写回返回新报告

- **WHEN** 以合法候选调用 `workspace_config_apply`
- **THEN** 文件被整体替换为规范化 file-face 内容（pretty + 尾换行，原子写无残留），返回的报告来自**写后重读**（`diagnostics` 与盘面一致，无 `fileMissing`）

#### Scenario: agent 无法触发写面

- **WHEN** 审查 config 会话的工具面与 bash 写检测
- **THEN** 名字清单不含任何写工具，`workspace_config_apply` 不出现于任何 `<tool>` 定义；模型经 `bash` 重定向写 `config.json` 仍被写保护拒绝（内置锁定规则在案）

#### Scenario: 无效 root 走 reject

- **WHEN** 以不存在 / 非目录 / 空白 root 调用 `workspace_config_apply`
- **THEN** 返回 `Err(String)`，进程无 panic，任何文件零改动

### Requirement: 文件面写入器 `to_file_value`（模板驱动）

写面 SHALL 经 `core/config` 的纯函数 `to_file_value(config: &WorkspaceConfig, template: &Value) -> Value` 产出 file face；`serde_json::to_value(&WorkspaceConfig)` MUST NOT 用于落盘：

- **模板驱动**：输出键集 SHALL 按 `template`（候选原文）出现过的键裁剪，值取规范化结果；`WorkspaceConfig` 不记录 provenance（`cwd: "."` 无法区分「文件写了 `.`」与「吃了默认」），故「写全 / 写最小」不是 flag 而是**入参类型**；「写全字段」= 模板取全键集的同一特例，MUST NOT 设第二形态。
- **结构变换而非手拼**：SHALL 为 `to_value` → 三处递归 `rename_in_place`（`staticAnalysis`→`static_analysis`、`writeProtection`→`write_protection`、`dependsOn`→`depends_on`，`dependsOn` 在 `projects[i]` 内故须递归全层级）→ `extra` 展开回顶层 → 数值整数化 → 模板裁剪；MUST NOT 手拼字段名清单（新增字段必漏）。
- **数值整数化**：`fract() == 0.0` 且绝对值不超过 `2^53` 时出 `i64`，否则保留浮点（serde_json 的 `PosInt` 与 `Float` 不相等，阈值缺省值来自 `f64`，不整数化会让用户写的 `90` 变成 `90.0`）。
- **`extra` 三坑**：① null 值条目 SHALL 保留（丢 null 规则只作用于已知 Option 字段，MUST NOT 作用于 `extra`，否则回读少一条）；② 键序按字母序重排（不可保真，接受）；③ 键冲突 SHALL 显式 `Err`（若将来给结构体加字段却忘进 `KNOWN_KEYS`，字段与 `extra` 副本会同键，`Map::insert` 会静默覆盖）。
- **四面一致性锚**：写入器 SHALL 由四条机械锚守（缺一即漂移无人拦）——**P1** 文件面输出中 `extra` 值内部以外任何键名不含大写字母（自证，零维护）；**P2** 满配 fixture 的顶层键集 == `known_keys()` 且无 `extra`（使 `KNOWN_KEYS` 成为写入器键集的唯一持有者，并继承其与 `config.schema.json` 的既有一致性断言）；**P3** `assemble(to_file_value(cfg, 模板))` 与 `cfg` 逐字段相等（线面值比较，`WorkspaceConfig` 无 `PartialEq`）；**P4** 对「全字段显式」字面量 `L`（字母序、数值写整数）：`to_file_value(assemble(L).config, L) == L`。
- **公开面配套**：`core/config` SHALL 导出 `known_keys()` 与 `TestFramework::ALL` / `as_str()`（与 `parse_framework` 双向互证），供知识词表与 P2 锚共用；`plan_apply_config` SHALL 返回 `ConfigWrite { file: PathBuf, content: String }`（沿 `archi::edit::GateWrite` 的 `{ file, content }` 形状），路径经 `foundation::layout::config_path`（crate 内 MUST NOT 出现 `config.json` 字面量）。
- **规范化明示（不做保序）**：写回会重排键序并格式化（字母序 + pretty 展开，行内数组拆行），`projects[i]` / `tests[i]` / `write_protection.files[i]` 内的未知子键在组装路径上被静默丢弃——该后果 SHALL 在应用确认面明示；MUST NOT 引入 `serde_json/preserve_order`（会把全仓 `Map` 换 `IndexMap`，波及所有 JSON 顺序断言）。

#### Scenario: wire face 绝不用于落盘

- **WHEN** 审查写面实现与 `Ok` 输出形态
- **THEN** 落盘内容与 `propose_config` 输出只经 `to_file_value`；`serde_json::to_value(&WorkspaceConfig)` 未出现在任何落盘路径（出现即判不合格）

#### Scenario: 四条机械锚

- **WHEN** 运行 `core/config` 测试
- **THEN** P1 / P2 / P3 / P4 各自成立：无大写键名（`extra` 值内除外）、满配顶层键集与 `known_keys()` 逐字相同、`assemble ∘ to_file_value` 幂等、全字段字面量往返逐字相等

#### Scenario: 整数化与 extra 保真

- **WHEN** 候选含 `mutation.score: 90`、`coverage.lines: 90.5`，并含一个值为 `null` 的 `extra` 条目
- **THEN** 落盘内容为 `90`（非 `90.0`）与 `90.5`；`extra` 的 null 条目仍在盘上，重读后条目数不减

#### Scenario: 键冲突显式失败

- **WHEN** 构造「结构体字段与 `extra` 键同名」的输入（例如给结构体加字段却忘进 `KNOWN_KEYS`）
- **THEN** `to_file_value` 返回显式 `Err`（附冲突键），MUST NOT 静默覆盖任一侧

#### Scenario: 文件面键形全小写

- **WHEN** 检查任一满配 fixture 的落盘输出
- **THEN** `static_analysis` / `write_protection` / `depends_on` 三处为 snake_case，`extra` 条目展开在顶层，无 `staticAnalysis` / `writeProtection` / `dependsOn` 残留

### Requirement: 提案卡 `ConfigProposeCard` 五态与写回呈现

前端 SHALL 为 `propose_config` 提供专用工具卡（注册进 `BUILD_IN_TOOL_CARDS`，键为该工具名），呈现「盘面 vs 候选」的字段级差异与唯一的写回入口：

- **五态**（判据锚在 `ToolResult` 形态，MUST NOT 由前端重算配置语义；其中「可应用」态按 `changes` 是否为空再分两支呈现）：无 `ToolResult` 块 = 进行中；`is_error` = 失败（诊断原文）；`candidate != null` = 可应用（`changes == []` 时按钮禁用 + 「与当前配置一致」）；`candidate == null` = 只读模式（仅盘面展示）；`content` 不可解析 = 原文降级渲染（MUST NOT 抛错、MUST NOT 空白）。
- **diff 基准**：SHALL 取 payload 内的 `current`（与 `candidate` 同刻同源），MUST NOT 读页面 hook 数据做跨时刻比较。
- **应用**：点击「应用」SHALL 以 payload 的 `candidate` 调 `workspace_config_apply`，成功后触发 `useWorkspaceConfig().refresh()`；失败 SHALL inline 呈现原因且卡片保持可重试（候选不被丢弃）。
- **多次提案**：一轮或多轮内的多次 `propose_config` SHALL 各呈一张卡、都可应用（最后点哪个算哪个）；MUST NOT 做「已被取代」失效标记。
- **规范化明示**：可应用态 SHALL 明示「写回会规范化文件（未知子键、键序与格式会丢失 / 变化）」——用户在下笔前就知道整份文件会被重排。

#### Scenario: 五态齐备

- **WHEN** 分别渲染「工具已调用未收口」「`is_error`」「`candidate != null` 且 `changes` 非空」「`candidate != null` 且 `changes == []`」「`candidate == null`」「不可解析 content」六种 `ToolResult` 形态（覆盖五态及其可应用态的两支）
- **THEN** 依次呈进行中、失败诊断原文、差异 + 可点应用、差异为空 + 禁用 + 「与当前配置一致」、只读盘面、原文降级；零白屏、零抛错

#### Scenario: 应用走唯一写面并刷新

- **WHEN** 在可应用态点击「应用」
- **THEN** 恰一次 `workspace_config_apply` 调用（载荷为该卡的 `candidate`），成功后页面重取一次配置报告（新值就地呈现）；失败时 inline 呈现错误、该卡仍可再次点击

#### Scenario: 多次提案各自可应用

- **WHEN** 一轮内连续两次 `propose_config` 均产出可应用候选
- **THEN** 两张卡并存且都可点；先点第一张后第二张仍在且内容不被改写（无失效标记、无互相覆盖）

#### Scenario: 前端零配置语义

- **WHEN** 审查卡片实现
- **THEN** 卡片不读 `KNOWN_KEYS` / 不校验配置、不自行算 diff（差异、摘要、合法性均来自后端 payload）；唯一决策是「有没有 candidate、changes 是否为空」

## Module Contract

### Component: `packages/desktop/src/views/config/config-view.tsx`（改）

| 项 | 变化 | 描述 |
|---|---|---|
| `ConfigView` | 单列 → 横向分栏 | `ResizablePanelGroup orientation="horizontal"`：左栏 = `ConfigChatPanel`（骨架自带可折叠分栏，缺省收起 + 窄条），右栏 = 既有页头（标题 + `config-refresh`）+ 配置分区；`useWorkspaceConfig` 的 `refresh` 同时交给页头与应用成功回调 |

### Component: `packages/desktop/src/views/config/components/config-chat-panel.tsx`（新增）

| 组件 | Props | 描述 |
|------|-------|------|
| `ConfigChatPanel` | `{ root: string \| null; onApplied: () => void }` | 面板薄封装：`useConfigChat` 状态面 + 共享骨架 `SessionPanel`（`testIdPrefix = "config-chat"`）+ composer 插槽 + `onApplied` 透传给卡片的应用成功回调；归属参数（`source = "config"` / `sourceRef = null` / `cwd = null`）由 hook 承担 |

### Function: `packages/desktop/src/views/config/hooks/use-config-chat.ts`（新增）

| 项 | 签名 | 描述 |
|------|------|------|
| `useConfigChat` | `(params: { root: string \| null; onApplied: () => void }) => ConfigChatState` | `useAgentSession` 注入（`source = "config"` / `sourceRef = null` / `cwd = null`）+ 清单取数（`listConfigChatSessions` 分页；会话 id 确立 / 轮收口后重取）+ 新会话（置 null + `reset()`）+ 历史点选 + root 复位 + 运行中禁发判据（`canSend` / `canStart`）+ 状态行 `note` |

### Component: `packages/desktop/src/components/agent/message-cards/tool-cards.tsx`（扩）

| 组件 | Props | 描述 |
|------|-------|------|
| `ConfigProposeCard` | `AgentToolCardProps`（`toolCallId` / `toolName` / `state` / `input` / `output` / `turnRunning`） | 五态提案卡：盘面 vs 候选字段级 diff（沿用 `EditCard` 的 added / removed 双 `CodeBlockLine` 形态）、只读模式盘面、失败诊断原文、不可解析时原文降级、「应用」按钮 + 规范化明示；注册进 `BUILD_IN_TOOL_CARDS["propose_config"]` |

### Function: `packages/desktop/src-tauri/crates/core/config/src/lib.rs`（扩）

| 项 | 签名 | 描述 |
|------|------|------|
| `known_keys` | `() -> &'static [&'static str]` | `KNOWN_KEYS` 八键的只读出口（知识词表与 P2 锚共用） |
| `TestFramework::ALL` | `&'static [TestFramework; 8]` | 八值枚举清单（与 `parse_framework` 双向互证） |
| `TestFramework::as_str` | `(&self) -> &'static str` | 值 → 出线字面量 |
| `validate` | `(&Value) -> ConfigReport` | 对任意 JSON 值跑同一套校验，产出永远合法的 config + 诊断 |
| `to_file_value` | `(&WorkspaceConfig, &Value) -> Value` | 模板驱动的 file-face 投影（结构变换 + 整数化 + `extra` 展开 + 裁剪） |
| `plan_apply_config` | `(&Path, &Value) -> Result<ConfigWrite, String>` | 纯函数写面规划：零 `InvalidValue` 门 → `to_file_value` → `ConfigWrite { file, content }`（零 IO） |

### API: `packages/desktop/src-tauri/src/commands/config/mod.rs`（扩）

| 命令 | 签名 | 描述 |
|------|------|------|
| `workspace_config_apply` | `(root: String, candidate: serde_json::Value) -> Result<WorkspaceConfigReport, String>` | 薄包装三段；`*_inner` 可离 Tauri 运行测试；无效 root / 脏候选 / 写失败 → `Err`；成功返回写后重读报告 |

### API: `packages/desktop/src-tauri/src/commands/exec/mod.rs`（扩）

| 命令 | 签名 | 描述 |
|------|------|------|
| `list_config_chat_sessions` | `(root: String, offset: u32, limit: u32) -> Result<SessionPage, String>` | 薄包装：`list_sessions(Some("config"), None, offset, limit)`；`source` 常量收命令层；blank root → 空页 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_propose.rs`（新增）

| 项 | 签名 | 描述 |
|------|------|------|
| `PROPOSE_CONFIG_TOOL` / 定义 | 常量 + `ToolDefinition`（`{ candidate?: object }`、`required: []`） | 挂载清单第 14 项、`ToolTier::Read`、`ToolExec::Config`、无路径参数 |
| `ConfigContext::execute` | `(&self, input: &Value) -> Result<String, String>` | 两模式：只读返回盘面 / 候选校验回环；输出单条 file-face JSON 字符串；`changes` 与 `summary` 后端算；零副作用 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/config_knowledge.rs` + `config-knowledge.md`（新增）

| 项 | 签名 | 描述 |
|------|------|------|
| `knowledge` | `() -> Option<String>` | 静态骨架（`include_str!`）+ 派生词表（`config::known_keys` / `TestFramework::ALL`）+ 使用指引；零入参、零 IO；失败返回 `None`（只丢 preamble 位） |

### Component: `packages/desktop/src-tauri/src/commands/atomic.rs`（新增）

| 项 | 签名 | 描述 |
|------|------|------|
| `write_atomic` | `(&Path, &str) -> Result<(), String>` | 临时文件 + rename 的原子写（`pub(crate)`，`archi_set_gate` 与 `workspace_config_apply` 共用；`fs::write` 唯一位点） |
