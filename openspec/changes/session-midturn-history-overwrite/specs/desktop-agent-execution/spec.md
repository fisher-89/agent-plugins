# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: ask 工具协议与阻塞应答通道

SDK 引擎 SHALL 提供基线阻塞工具 `AskUserQuestion`，其应答通道 SHALL 为**会话级注册表**（`AskRegistry`，与 `StopRegistry` 同址托管）：

- **工具契约**（定义单源在 `core/orchestration`，见 desktop-change-orchestration「流程内 ask 停等与双入口应答」）：工具名 `AskUserQuestion`，`parameters` 为 `{ name: string（必填，`pattern` = `^[A-Za-z][A-Za-z0-9_-]{0,63}$`）, question: string（必填）, options: string[]（必填，`minItems: 1`）}`——**一次问一个问题**，多问经多次调用表达；`name` 是**协议声明**的本问标识，即应答 map 的键（首字符须为字母，故控制键位不可能与之撞车）；`name` 非法或缺失、`options` 为空数组 SHALL 被工具校验 `Err` 拒绝（文案自足：字段名 + 期望 + 当前值）；用户应答 MAY 为任一选项文本或自由文本，工具 MUST NOT 限制应答取值域。**本工具要求必答**（无取消通路）：用户想改变主意时以自由文本表达，`status` 之外无第二种终态。
- **应答结果形状**（单点在 `infra/agent` 的 ask 执行体）：成功臂的 tool result 正文 SHALL 为**信封** `{"status":"answered","answers":{"<name>":"<应答原文>"}}`——`status` 为封闭枚举，本版仅 `answered`（`declined` 为预留位，见下条）；`answers` 为「问题名 → 用户原文」map，原文逐字保真（零 trim、零截断）。信封形状 MUST NOT 在前端铸造（前端两处入口一律上行用户原文，见 desktop-agent-chat-infra / desktop-change-flow-view）。多问扩展只让 `answers` 多键，信封 MUST NOT 变更；读侧 SHALL 兼容历史三种正文——旧扁平 map（无 `status`）、更早的自由文本原文、错误文案（后者不进信封，随 is_error 回灌）。`status` 字段 MUST NOT 因「本版只有一个取值」而被删除——它是取消通路的唯一扩展位。
- **取消通路（预留，未实现）**：若将来开放取消，结果 SHALL 为 `{"status":"declined"}`（无 `answers` 键），且 SHALL 以 `is_error: false` 回灌（用户主动取消不是工具故障）；`AskRegistry` 的投递类型相应扩为「应答 / 取消」两态。本条为形状预留，当前实现 MUST NOT 提供取消入口。
- **`AskRegistry`（`core/agent`）**：形态 SHALL 为 `{ slots: Mutex<HashMap<String, oneshot::Sender<String>>> }`，键 = **core 铸的会话 id**（`core/agent` 不认识 workspace root / change——两段寻址由壳层翻译）；方法三件：`register(session_id, tx) -> bool`（登记等待方，同键覆盖 = 上一问已被放弃，返回是否登记成功）、`answer(session_id, text) -> Result<(), String>`（**take 语义**：单次投递，重复 / miss → `Err`，不产生第二次回灌）、`remove(session_id)`（轮收敛除名，miss 幂等）。注册表 MUST NOT 携带任何编排域类型（无 change id、无 `RunEventSink`）。
- **会话 id 下传**：`open_session` 的入参 SHALL 携带 core 铸的会话 id（`SessionOpen.session_id`），使引擎侧能以自己的执行体上下文登记到正确键；逐轮的 `TurnQuestion` MUST NOT 重复携带该 id（会话级标识在会话建立时一次可知）。
- **两档执行体（同名覆盖）**：`ToolExec::Ask(AskMode)` 为单一挂载变体带档位，`TOOL_NAME` 与 `parameters` 两档共用（schema 单源），差别只在执行体：
  - `Generic`：`register(session_id, tx)` → 阻塞等待应答（无编排广播）→ 返回**应答信封**（形状见上条）作为 tool result；
  - `Flow`：经 `AskPort` 先广播「ask 已开始」（`AskPayload` 含 `tool_call_id` / `session_id`）→ `register` → 阻塞等待 → **两臂（应答 / 停止）返回后都广播「ask 已解除」**（流程停等态清理单点在此，见 desktop-change-orchestration）。
  两档 SHALL 与 `finish_step` **互不依赖**：`ask` 位与 `finish` 位独立置位，两模块代码互不引用（`grep` 可验的零互引，见 desktop-change-orchestration 同条）。
- **`ask` 位与挂载**：`SessionInjections.ask: Option<AskMode>` 决定是否挂载（`None` = 不挂 = 硬禁用：`RunStarted.tools` 无该名，模型调用即 `tool_unavailable`；`Some(_)` = 挂载，tier 恒 `Protocol`，三档权限全放行）。执行上下文 SHALL 经组合根以 per-turn 形态铸造（注册表句柄 + core session id + 可选 `AskPort`）；上下文缺席 / 登记失败 SHALL 以 is_error ToolResult 回灌（MUST NOT panic、MUST NOT 静默成功）。
- **停止语义**：ask 阻塞期间的用户停止 SHALL 由外层泵的 `select!` **轮中 drop 整个 loop future** 实现（既有机制，零新增）；因此 ask 执行体 MUST NOT 依赖「停止时回灌 is_error 让模型看见」——做不到，且转录会留下未配对的 `tool_use`，该缺口 SHALL 由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 补齐（既有机制）。`Flow` 档的 `select!` SHALL 显式带停止臂（`RunHandle::wait_requested()`）——外层 drop 不执行清理代码，故停等态清理必须落在执行体内的这个臂上。
- **停等态非 durable 与进程中断归属**：ask 停等态 SHALL 只活在进程内（`AskRegistry` 槽 + 阻塞中的 loop future），MUST NOT 落库——「仍在等待应答」在库中没有任何 durable 痕迹，注册表在进程新生时必然为空。因此进程在工具执行中死亡（关窗 / 崩溃 / 重启，含 ask 停等期与任意工具执行期）时，该轮 SHALL 被当作**客观终止**：由启动标定把残留 `running` 轮行翻为 `interrupted`（见 desktop-workspace-store「agent 轮启动标定与 running 残留终结」），未配对的 `tool_use` 保留在转录（与上条「停止语义」同形，provider 配对由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 兜底）。此后对同一 `session_id` 的 `agent_answer` SHALL 走既有 miss 臂显式 `Err`（「当前无等待中的 ask」）——MUST NOT 幂等 `Ok`、MUST NOT 改写该轮状态、MUST NOT 提供「复活停等」入口（会话接续见下条边界）。本 requirement MUST NOT 新增停等态持久化面或恢复面。
- **幂等与可归因**：同一 ask 二次应答 SHALL 返回 `Err`（文案「当前无等待中的 ask」）；会话已收口后的迟到应答 SHALL 同样 `Err`（注册表槽已由轮收敛除名）；两处 `Err` 都 MUST NOT 改写任何会话状态。

#### Scenario: 通用会话 turn 内阻塞问答

- **WHEN** `ask = Some(Generic)` 的会话里模型调用 `AskUserQuestion { name: "retry", question, options }`，用户经 `agent_answer` 投递应答「指数退避」
- **THEN** 该轮不收敛、不发额外模型请求；该 `tool_use` 的 tool result 正文为 `{"status":"answered","answers":{"retry":"指数退避"}}`（`is_error: false`），模型在同一 turn 内继续（同一 `session_id`、`turn_index` 递增一次）

#### Scenario: 同名覆盖只多广播

- **WHEN** `ask = Some(Flow)` 的 change 会话里模型调用同一工具
- **THEN** 下发的工具名与 `parameters` 与 `Generic` 档逐字相同（模型视角零差异、历史转录无需分辨来源），额外经 `AskPort` 先后广播「已开始（携 `tool_call_id` 与 `session_id`）」与「已解除」；应答以同一信封形状照常回灌（形状与档位无关）

#### Scenario: 显式禁用即硬禁用

- **WHEN** 会话以 `ask = None` 装配，模型仍发出 `AskUserQuestion` 调用
- **THEN** `RunStarted.tools` 不含该名；调用被成员校验拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌，run 不中断，注册表无登记

#### Scenario: 停止不改写协议配对

- **WHEN** ask 阻塞期间用户停止该会话
- **THEN** 轮收敛 `stopped`；转录保留该 `tool_use` 的未配对记录（无 is_error 回灌——future 已 drop）；以该会话续轮时 `resume::rebuild` 补 `INTERRUPTED_RESULT` 使 provider 配对协议不破；注册表该键已除名（其后 `answer` → `Err`）

#### Scenario: 重复应答只回灌一次

- **WHEN** 同一 ask 被连续应答两次（两入口或同入口重放）
- **THEN** 第一次成功且等待方收到原文，第二次返回 `Err`（「当前无等待中的 ask」），等待方 MUST NOT 收到第二条、转录 MUST NOT 出现第二条 tool result

#### Scenario: 重启后停等轮已终结且应答显式失败

- **WHEN** 一轮阻塞在 `AskUserQuestion` 时进程被终止（模拟中途死亡），重启后对该会话调用 `agent_answer`
- **THEN** 该轮行在所属 workspace 库首次打开时已标定为 `interrupted`（携标定记因、`finished_at` 在位）；`agent_answer` 返回显式 `Err`（文案含「无等待中的 ask」），MUST NOT 幂等 `Ok`、MUST NOT 改写该轮行；该 `tool_use` 在转录中保持未配对，续轮时由 `resume::rebuild` 补合成结果不破 provider 配对

#### Scenario: 上下文缺席回灌而非崩溃

- **WHEN** `ask` 位为 `Some` 但执行上下文未装配（测试构造 / 装配漂移）
- **THEN** 该工具调用以 is_error ToolResult 回灌（「ask 执行上下文未装配」类文案），run 不中断、不 panic

### Requirement: Agent 调试页

前端 SHALL 提供 Agent 调试页（`AgentDebugView`），经侧栏「系统工具」组进入；视图切换 SHALL 沿用本地 state，MUST NOT 引入路由。页面 SHALL 包含：

- **参数面（最小集）**：prompt（必填）、permission-mode 三档下拉（默认 bypassPermissions）、agent 选择器（管理页 agent 实例清单，默认选中默认 agent——与组合根缺省解析一致；**仅调试页暴露**，正式场景无 agent 选择入口）；cwd 不设参数（隐含当前 workspace root）、model 不设参数、env 不设参数
- **事件时间线**：SHALL 经会话级统一组件 `AgentSessionView`（desktop-agent-chat-infra，对话透镜 `AgentMessages`）呈现——**运行中 token 级流式**（delta 累积进**该轮唯一 assistant 消息**的当前 step 部件，思考/回复可辨，密封到达追加整块而非另起消息，无碎行、无第二条气泡）、气泡序（按行到达序）、tool_use / tool_result 成对卡；run-started / system-notice / result / raw 等辅助载体逐事件各为独立 system 消息且不进对话呈现（保真原文经下方「原始转录切换」与部件元数据可考）；两引擎共用同一会话组件
- **运行中停止**：运行中 SHALL 呈现停止入口，触发终止命令（见「agent_stop 终止与提前 resolve」）
- **原始转录切换**：落库密封转录（含 Raw）原文可见（与 store 一致）；delta 仅实时面可见，不进入原始转录视图
- **历史会话**：会话列表 → 点开经同一 `AgentSessionView` 重放该会话（含轮行；invoke 查询 + 状态驱动挂流）
- **会话列表终态文案**：列表行的终态标签 SHALL 覆盖轮状态五值（`running`「运行中」/ `completed`「已完成」/ `stopped`「已停止」/ `failed`「失败」/ `interrupted`「已中断」），MUST NOT 把非失败终态落进失败文案——`interrupted` 是进程中断的客观终止，与用户主动停止同属「非故障」侧，词面沿 change 流程 `run-control-panel` 的 `RUN_STATUS_LABEL` 先例

页面状态 SHALL 经统一会话基建（`use-agent-session`）承载；run 表单 / 历史列表 / 转录开关为页面级 chrome，包在共享核心外圈。实时流经 transport 走 Tauri `Channel<AgentRunMessage>`（执行流通道，不属轮询取数）：发送期由发起流承载、历史会话点开（旁观 / 重挂）期由会话订阅面承载；查询类取数（会话列表 / 转录重放）仍由用户显式动作触发。

#### Scenario: 参数面默认值

- **WHEN** 打开调试页
- **THEN** permission-mode 默认 bypassPermissions、agent 选择器默认选中默认 agent（与组合根缺省解析一致）、prompt 为空且必填；无 model / cwd / env / 引擎直选输入

#### Scenario: agent 切换

- **WHEN** agent 选择器选 sdk agent 发起一次含工具调用的运行
- **THEN** 运行走 SDK 引擎（连接配置来自所选 agent 的引用 provider），对话视图 / 落库 / 重放组件零改动复用

#### Scenario: 流式连续呈现

- **WHEN** 一次含长文本回复与思考、且中途调用工具的 SDK 运行进行中
- **THEN** 对话视图呈单条连续增长的 assistant 消息（token 级、思考/回复可辨），第二个响应段的 delta 在同一消息内继续增长（不另起气泡），不出现逐 delta 碎行

#### Scenario: loop 可见性

- **WHEN** 一次含工具调用的运行完成
- **THEN** 对话视图可按序看到 assistant 思考/回复气泡与 tool_use → tool_result 成对卡；result / raw 原文经「原始 JSONL」切换可读（统计字段如实呈现，SDK 的 cost 如实缺省），子代理归因字段可考于部件元数据

#### Scenario: 运行中停止

- **WHEN** 调试页运行中点击停止
- **THEN** 触发终止命令，事件流以终止终态收尾，轮状态收敛为 `stopped`

#### Scenario: 原始转录切换

- **WHEN** 切换原始转录视图
- **THEN** 落库密封转录（含 Raw 透传件）原文可见，与 store 一致；视图中无 delta 记录

#### Scenario: 历史重放

- **WHEN** 从历史会话列表点开一条已完成会话
- **THEN** 经 invoke 查询以会话全史转录渲染完整对话视图（含轮行，经同一 `AgentSessionView`），不要求原运行进程存活

#### Scenario: 中断轮在会话列表呈已中断

- **WHEN** 会话列表呈现一条最新轮为 `interrupted`（进程中断后经启动标定终结）的会话
- **THEN** 该行终态标签为「已中断」（非「失败」）、带 `data-status="interrupted"` 挂钩；点开后经同一 `AgentSessionView` 重放全史，该轮呈轮内「因应用重启中断」状态行

#### Scenario: 历史会话运行中旁观流式

- **WHEN** 从历史列表点开一条仍在运行的会话（有 running 轮行）
- **THEN** 组件以 `since_seq` 订阅该会话并 token 级流式呈现（不经发起流），轮收口后以重放收敛为与重开一致的形状

#### Scenario: 停止轮可观可见

- **WHEN** 在调试页运行中点击停止，该轮未产出任何 assistant 密封消息（零内容停止）
- **THEN** 时间线在该轮位置呈现「已停止」状态行（不再是「界面只是不再增长」）；顶部失败横幅 MUST NOT 出现（`stopped` 不算失败）；轮行 status 为 `stopped` 可从该轮消息 metadata 读出

## Module Contract

### Function: `crates/core/agent/src/ask.rs`（`AskRegistry`）

| 项 | 变化 | 描述 |
|------|------|------|
| `register` / `answer` / `remove` / `is_pending` | **零改动** | 「重启后无等待方」由进程内形态天然保证：注册表空 ⇒ `answer` 走既有 miss 臂 `Err`；不新增停等态持久化 / 恢复面 |

### Function: `src-tauri/src/commands/exec/mod.rs`

| 命令 | 变化 | 描述 |
|------|------|------|
| `agent_answer` | **零改动**（行为由标定保证） | 对已标定会话返回显式 `Err`；命令体仍是参数校验 + 注册表投递 + 错误映射三段 |
| `agent_stop` | **零改动** | miss 幂等 `Ok` 沿用（不改行、不报错）；轮行终结由启动标定承担，不在命令面新增对账 |

### Component: `packages/desktop/src/views/agent/components/agent-run-history.tsx`

| 项 | 变化 | 描述 |
|------|------|------|
| `statusLabel` / 行色 | 增 `interrupted` 档 | 「已中断」；`data-status` 挂钩直出状态词；MUST NOT 落进既有 `failed` 兜底文案 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `crates/infra/agent/src/sdk/resume.rs` | `INTERRUPTED_RESULT` 合成结果零改动（续轮 provider 配对修复的既有机制） |
| 会话接续（`resume_turn` / loop 第三入口 / cwd 入档） | **明确不在本次范围**：中断的轮按客观终止处理，用户重发即可 |
| ask 取消通路（`declined` 预留位） | 照旧不开（形状预留保持不变） |
