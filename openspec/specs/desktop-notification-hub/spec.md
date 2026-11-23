# desktop-notification-hub Specification

## Purpose

定义桌面端**通知旁路**的完整契约：一条通知的词表与扇出点（`Notification` / `trait Sink` / `Hub`，core 侧零基础设施依赖）、两个写入点（前端动作反馈门面 + 后端事件投影）、首批事件白名单与幂等主键、user 维度收件箱的持久化与五命令读面、顶栏铃铛 + 抽屉的呈现面，以及「应用不在场即不可能产生通知」的诚实边界。本能力只管**通知的产生、留存与呈现**：`RunUpdate` / `RunNotice` 的线面与语义、change run 的状态机、sonner 的渲染实现、文件失效信号通道均不在本能力内。

## Requirements

### Requirement: 通知词表与扇出点

后端 SHALL 以独立 core crate（`crates/core/notification`）承载通知词表与扇出点，且该 crate MUST 只依赖 `serde` / `specta`（MUST NOT 依赖 workspace 内任何其它 crate、MUST NOT 依赖 `tauri` / `tokio` / `native_db`）：

- **两 kind**：`NotificationKind::Event`（后端事件）与 `NotificationKind::Feedback`（前端动作反馈）——同一模型两用途，MUST NOT 拆两张表。
- **四 level**：`NotificationLevel::{Info, Success, Warning, Error}`——逐档对位 sonner 的四档呈现，MUST NOT 引入第五档或与 level 正交的 `silent` 类字段。
- **载荷**：`Notification { id, kind, level, message, body: Option<String>, root: Option<String>, source_ref: Option<String>, created_at: i64 }`。`id` 是通知的稳定标识（事件为幂等串，反馈为前端铸造的 UUID）；`root` 是工作区归属字符串（记录自带归属，MUST NOT 依赖跨库 join）；`source_ref` 是触发源的引用（事件为 change id）。**读状态 `read_at` MUST NOT 进载荷**——它是收件箱记录面的字段，投递面是「不可变的一条内容」。
- **出线形态**：`Notification` SHALL 全量出线（`Channel<Notification>` 与命令返回同型），MUST NOT 降位为 kind-only 信号——通知是**内容本身**，与 `RunNotice`（「状态已变、请重查」的失效信号）语义不同。
- **Sink 缝**：`trait Sink { fn deliver(&self, n: &Notification); }` SHALL 为同步、无返回值、非 `async`、无 `Result`——重试 / 退避 / 批量 / 排队全部归 sink 内部，core 侧不 await。
- **扇出点**：`Hub { sinks: Vec<Arc<dyn Sink + Send + Sync>> }` 的 `publish(&self, n: &Notification)` SHALL 顺序遍历全部 sink 各投递一次。
- **三条不变量 SHALL 写在 crate doc 而不靠调用方自觉**：① 一个 sink 失败 MUST NOT 影响其他 sink，MUST NOT 让业务动作失败（通知是旁路）；② 通知 MUST NOT 为自己的失败再发通知（递归禁止）；③ core 只保证「投递一次」，重试 / 退避 / 批量归 sink 内部，同一 sink 内 FIFO、跨 sink 不保证顺序。
- **渠道缝与「MUST NOT 预铺」**：首批 SHALL 恰两个 sink —— 落库 sink（驻 `infra/store`）与 in-app sink（驻壳，broadcast → `Channel`）。写文件 / 其他应用 CLI / 远程 API 三渠道 MUST NOT 在本能力内建 crate、MUST NOT 引入配置键、MUST NOT 建渠道注册表；第二渠道到场时才裁决落点。

#### Scenario: 一 sink 失败不影响其他 sink

- **WHEN** 装配两个 sink 并让第一个在 `deliver` 中 panic 或以「失败」收场（落库 sink 的 `Err` 路径），随后 `publish` 一条通知
- **THEN** 第二个 sink 仍收到恰一次投递，`publish` 本身不返回错误、不 panic、不重投；业务调用方的返回值与状态零变化

#### Scenario: core 侧零基础设施依赖

- **WHEN** 审查 `crates/core/notification` 的 `Cargo.toml` 与源码
- **THEN** 依赖面只有 `serde` / `specta`（及其 derive），无 `tauri` / `tokio` / `native_db` / 其它 workspace 内部包；源码无 `spawn`、无 `async fn deliver`、无 `Result` 返回的投递面

#### Scenario: 第二渠道未预铺

- **WHEN** 检索 `crates/infra/` 与 `crates/core/` 的 crate 清单、`KNOWN_KEYS` 与 `config.schema.json`
- **THEN** 无 `infra/notification` crate、无 file / cli / remote 渠道实现、无渠道开关配置键；sink 装配点恰两处（store 与壳的 in-app）

#### Scenario: 通知载荷全量出线

- **WHEN** 对一条通知经 `Channel<Notification>` 出线并在前端读其字段
- **THEN** `id` / `kind` / `level` / `message` / `body` / `root` / `source_ref` / `created_at` 逐字段在场且语义与后端一致；`read_at` 不在出线类型中（若出现在生成的 `Notification` 类型上即违本条）

### Requirement: 双写入点与前端通知门面

前端 SHALL 以单一门面 `src/lib/notify.ts` 收口全部「弹提示」动作，并区分两个写入点：

- **门面是 sonner 的唯一持有者**：`packages/desktop/src` 的生产代码中，`import { toast } from 'sonner'` SHALL 恰出现在两处——`components/ui/sonner.tsx`（`Toaster` 渲染件，恒不动）与本门面。视图与 hooks MUST NOT 直引 sonner。
- **两个面**：`present(level, message, body?)` SHALL 只呈现（不落库）；`notify(level, message, body?)` SHALL 等于 `present(...)` + fire-and-forget 落库（`notification_record`，主键由前端铸造 `crypto.randomUUID()`）。`present` 的存在理由是后端事件轨的通知**已经落库**，此时只差呈现——MUST NOT 为呈现再写一条 `Feedback`。
- **调用形态逐字不变**：门面对 sonner 的四档调用 SHALL 与收口前逐字同形——`Info` → 裸 `toast(message)`（对位既有的裸调用点）、`Success` → `toast.success(message)`、`Warning` → `toast.warning(message, { description: body })`、`Error` → `toast.error(message)`；无 `body` 时 MUST NOT 补第二实参。门面 MUST NOT 向 sonner 传 `id`：前端反馈的 id 天生唯一（无「同 id 更新」需求），传 id 只会让既有精确参数断言同时失效。
- **level 词汇单一来源**：门面的 level 取值 SHALL 取自生成的 IPC 类型（`NotificationLevel`），MUST NOT 在门面内另立同名枚举或字符串字面集。
- **落库是旁路**：`notify` 的落库调用的失败（reject、参数校验同步抛 `IpcArgsError`）SHALL 被就地吞掉：MUST NOT 抛出、MUST NOT 影响已发生的 toast、MUST NOT 重试、MUST NOT 为落库失败再发一条通知；DEV 构建下 SHALL 留一行 `console.warn`（沿 `lib/ipc-drift.ts` 的 soft-report 形状），prod 构建零留痕。
- **反馈轨的可见面**：`kind = Feedback` 的记录 SHALL NOT 进收件箱列表、SHALL NOT 计入未读数、SHALL NOT 经 `Channel` 广播（前端反馈是本窗口的交互反馈，不是跨窗口事件）。

#### Scenario: sonner 直调点收口到两处

- **WHEN** 对 `packages/desktop/src` 的生产代码（排除 `*.test.ts` / `*.test.tsx`）检索 `from 'sonner'`
- **THEN** 命中行恰两处：`components/ui/sonner.tsx` 与 `lib/notify.ts`；原先 6 个文件 12 处业务直调（`use-workspaces` 2 / `use-agent-instances` 3 / `use-agent-providers` 2 / `use-change-list` 2 / `contract-panel` 1 / `explore-open-file` 2）全部改为经门面，且文案逐字不变

#### Scenario: 四档调用形态与既有断言对齐

- **WHEN** 经门面分别发起四档提示（含带 `body` 的 warning）
- **THEN** sonner 收到的调用逐档为 `toast(message)` / `toast.success(message)` / `toast.warning(message, { description })` / `toast.error(message)`；均无第三个实参、无 `id` 键；既有 19 处 `toHaveBeenCalledWith(<文案>)` 形态的断言零改动通过

#### Scenario: 落库失败不污染交互

- **WHEN** `notify('error', …)` 触发后 `notification_record` reject（或参数校验就地抛错）
- **THEN** toast 照常弹出、调用方无异常外溢、零重试、零第二条通知；DEV 下恰一行 `console.warn`，prod 下零输出

#### Scenario: present 不写库

- **WHEN** 对一条 Channel 投递的后端事件调 `present(...)`
- **THEN** 只发生一次 toast，`notification_record` 调用数为 0（该通知的记录已由后端落库）

### Requirement: 事件白名单与幂等主键

后端事件轨 SHALL 经**单一投影函数**把 `RunUpdate` 投影为通知，MUST NOT 在 sink 或命令层按变体散落 `if` 白名单：

- **三变体永久排除**：`Step` / `Edge` / `SessionEvent` SHALL 恒投影为 `None`（一次 run 数十上百条，是「视图重查」信号而非事件；转录另有面）。
- **三变体纳入**：

| 输入 | level | message 必含 | body |
|---|---|---|---|
| `Ask` | `Warning` | 变更名 + 「等你的回答」 | `ask.question`（选项 MUST NOT 进 body） |
| `ConfirmWait` | `Warning` | 变更名 + 相位 + 「等确认」 | 相位表 `PhaseDefinition.description`（人话标签的唯一来源，MUST NOT 另立相位翻译表） |
| `Finished(Completed)` | `Success` | 变更名 + 完成语义 | 无 |
| `Finished(Failed)` | `Error` | 变更名 + 失败语义 | `reason` |
| `Finished(Stopped)` | `Info` | 变更名 + 已停止语义 | 无 |

- **必填上下文**：`kind` 恒 `Event`、`root` 为 workspace root、`source_ref` 为 change id；变更名 SHALL 由发起侧已读出的 change 记录随 sink 携带（MUST NOT 为文案再查一次 store）。
- **投影侧零线面改动**：`RunUpdate` / `RunNotice` / `ChangeRunStatus` / `AskPayload` 的定义与广播面 MUST NOT 因本能力改动；两个 hook 点（`RunEventSink::emit` 覆盖 `ConfirmWait` / `Finished`，`AskPort::ask_started` 覆盖 `Ask`）SHALL 共用同一投影函数。
- **主键 = 幂等串**（MUST NOT 另开二级索引做查重）：`Finished` → `run:{run_id}:finished`；`ConfirmWait` → `run:{run_id}:confirm:{phase}`；`Ask` → `run:{run_id}:ask:{tool_call_id}`，`tool_call_id` 缺位时退 `run:{run_id}:ask:{created_at}`。同一主键重投 SHALL upsert 同一行。
- **读状态保真**：upsert SHALL 只更新 `message` / `body`（及 `level`），MUST NOT 把 `read_at` 从 `Some` 翻回 `None`——重投不得复活已读。
- **死未读清理**：`Finished` 落库后，同一 run 的 `confirm:*` / `ask:*` 未读行 SHALL 一并置为已读（用户在停等点上按停止键时，那条「等你确认」永远等不到人）。清理失败 MUST NOT 影响 run 的终态收敛。
- **呈现口径**：`level = Info` 的通知 SHALL 落库与计数，但前端 MUST NOT 为它弹 toast（机械规则，MUST NOT 引入 `silent` 字段或按 status 分叉特判）。

#### Scenario: 三变体恒不产生通知

- **WHEN** 对 `Step` / `Edge` / `SessionEvent` 三变体各调一次投影
- **THEN** 三次均返回 `None`；`commands/change_flow` 与 `commands/notifications` 源码内无「按变体过滤」的分支（投影函数是唯一单点）

#### Scenario: 三变体的 level 与必含要素

- **WHEN** 对 `Ask` / `ConfirmWait` / `Finished(Completed|Failed|Stopped)` 各投影一次
- **THEN** level 逐档为 `Warning` / `Warning` / `Success` / `Error` / `Info`；message 均含变更名，`ConfirmWait` 的 message 含相位 id 且 body 为该相位在相位表中的 `description`；`Ask` 的 body 为 question 且不含选项清单；`kind` 恒 `Event`、`source_ref` 为 change id

#### Scenario: 主键逐字与多轮 ask 不互相覆盖

- **WHEN** 同一 run 先后产生两次 `Ask`（`tool_call_id` 分别为 A / B）与一次 `ConfirmWait(dev-design)`
- **THEN** 表中出现三行，主键逐字为 `run:{run_id}:ask:A` / `run:{run_id}:ask:B` / `run:{run_id}:confirm:dev-design`；`tool_call_id` 缺位时的退化主键为 `run:{run_id}:ask:{created_at}`

#### Scenario: 重投不复活已读

- **WHEN** 先 publish 一条 `Finished` 并把它标记为已读，再以同一主键 publish 同一条（`message` 有差异）
- **THEN** 表中仍恰一行，`message` 更新为后者，`read_at` 仍为 `Some`

#### Scenario: Finished 清理同 run 的死未读

- **WHEN** 某 run 先落 `confirm:archive` 与 `ask:*` 两条未读，再落该 run 的 `Finished`
- **THEN** 该 run 的两条等待类记录变已读；其他 run 的未读行与全部 `Feedback` 行不受影响

#### Scenario: Info 落库不弹 toast

- **WHEN** 一条 `Finished(Stopped)` 通知经 Channel 到达前端
- **THEN** 收件箱新增一行且未读计数 +1，但 toast 零弹出（`[data-sonner-toast]` 不在文档中）

### Requirement: 收件箱持久化与命令面

后端 SHALL 以 user 维度模型 `NotificationRecord` 持久化通知，并提供专用命令轨道 `commands/notifications/` 的五条命令：

- **落位**：`NotificationRecord` SHALL 归 **user 维度**、注册进全局库模型组（字段与 `Notification` 载荷一致 + `read_at: Option<i64>`）；MUST NOT 落 workspace 库、MUST NOT 落 workspace repo。表与字段说明由 `///` doc 自动派生，信封注册表（`MODEL_ENTRIES`）登记一行，db 查看器零模型特定代码即可见。
- **过滤口径（单点）**：列表与未读计数 SHALL 恒只认 `kind = Event`；`Feedback` 只落库、不进列表、不进未读。该口径 MUST NOT 在前后端各实现一遍（前端不二次过滤）。
- **命令面**：

| 命令 | 形态 | 语义 |
|---|---|---|
| `notification_subscribe(on_event: Channel<Notification>) -> u32` | 订阅 | SHALL **先**注册 broadcast 接收端、**再**读未读计数、**再**返回——订阅建立与计数快照之间无窗口丢更新 |
| `notification_list(offset: u32, limit: u32) -> NotificationPage` | 读 | `NotificationPage { items, total }`；`created_at` 严格降序；`total` 为 `kind = Event` 全量计数 |
| `notification_mark_read(ids: Vec<String>) -> u32` | 写 | 逐条置已读，返回剩余未读数 |
| `notification_mark_all_read() -> u32` | 写 | 全量（含未加载页）置已读，返回 `0` |
| `notification_record(record: NotificationFeedbackInput) -> Result<(), String>` | 写 | 前端反馈轨落库（不广播）；前端不等待结果 |

- **写命令落本轨道**：五命令 SHALL 落 `commands/notifications/`，MUST NOT 并入只读的 db 查看轨道（`commands/db/`）；命令层三件事纪律（参数转换 / 调用 / 错误映射）与「空白入参不进入库链路」的口径不变。
- **不做修剪**：本能力 MUST NOT 引入 cap / TTL / 启动清理；无界增长是已登记的已知限制，增长面由列表分页与 db 查看器承载。
- **失败面分流（沿错误呈现双轨）**：订阅失败 SHALL 静默（不阻断壳、不白屏、不弹错误 toast）；列表失败 SHALL inline 持久呈现；`notification_record` 失败 SHALL 静默（旁路，见「双写入点」）。
- **广播语义**：in-app sink SHALL 以 `broadcast` 通道承载投递（订阅端滞后丢弃即丢增量），MUST NOT 让通知通道成为未读的唯一真相源——抽屉打开时的 `notification_list` 是权威补拉（沿 `change_flow_watch` 的补订语义）。

#### Scenario: 收件箱归 user 维度

- **WHEN** 写入若干通知后分别以全局库与当前 workspace 库的 scope 查看模型清单
- **THEN** `NotificationRecord` 只出现在全局库、记录计数只随全局库增长；workspace 库与其 db 文件内零该模型记录；db 查看器无需任何模型特定代码即可列出该表与字段说明

#### Scenario: 过滤口径前后端一致

- **WHEN** 库中插入 N 条 `Event` 与 M 条 `Feedback` 记录
- **THEN** `notification_list` 只返回 N 条（分页后）且 `total = N`，`notification_subscribe` 返回的未读数 ≤ N；前端对返回结果零二次过滤（无 kind 判定的 filter）

#### Scenario: 倒序与页界自洽

- **WHEN** 以 `offset = 0, limit = 50` 与 `offset = 50, limit = 50` 各取一次
- **THEN** 两页并按 `created_at` 严格降序无重叠、`items.len() ≤ 50`、两页的 `total` 同值且等于 Event 全量；`limit` 越界（超上限）由命令显式拒绝或钳位（口径由 design 定夺并落测试）

#### Scenario: 订阅初值不丢更新

- **WHEN** 在「已有 3 条未读」的库上调用 `notification_subscribe`，并在订阅建立与返回之间并发 publish 一条新通知
- **THEN** 返回的初值 + 随后经 Channel 收到的条数 ≥ 4 且 ≤ 4（不丢条）；实现顺序可断言为「先取 receiver、再读计数」

#### Scenario: 标记已读的返回口径

- **WHEN** 调 `notification_mark_read([id1])` 后调 `notification_mark_all_read()`
- **THEN** 第一次返回剩余未读（= 原未读 − 1）、第二次返回 `0`；两次均不产生新记录、不改 `created_at`、不影响 `Feedback` 行

#### Scenario: 零修剪

- **WHEN** 连续 publish 远超任意阈值的通知（如 500 条）后读 `notification_list`
- **THEN** 全量留存（`total = 500`），无 cap 淘汰、无 TTL 清理、启动路径无删除逻辑

### Requirement: 壳层收件箱与通道纪律

前端 SHALL 以**顶栏铃铛 + 抽屉**承载收件箱，且不新增路由：

- **落位**：壳态 `ShellHeader` SHALL 含通知铃铛（未读徽标，展示上限 `99+`），点击打开 `StandardDrawer` 收件箱（列表 / 单条置已读 / 全部已读 / 空态）。组件 SHALL 为独立源文件 `src/components/notification-bell.tsx`，MUST NOT 内联进 `app.tsx`（页壳仅装配）；该文件 SHALL 落架构模型 `Desktop.Shell` 的 `path` 之内（未在 `path` 内的源文件会静默落 `unmatchedFiles` 且不产出任何边）。
- **零新增导航**：MUST NOT 新增路由、MUST NOT 新增侧栏项；`<Toaster />` 的 App 根单点挂载与 `AppRoutes` 逐字不动。欢迎态（`root` 为 null）MUST NOT 挂载铃铛（沿壳态隔离）。
- **通道纪律**：`Channel<Notification>` SHALL 是第三类执行流通道（沿 agent 执行流与 change run 通知先例）——`Channel` 构造与 `notification_subscribe` 调用 SHALL 各恰一处并收口在 `src/hooks/use-notifications.ts`；MUST NOT 并入 `use-file-watch-signal.ts`（该件的划界原则按驱动源切分：文件失效信号 ≠ 后端事件），MUST NOT 引入定时轮询（无 `setInterval` / 轮询型 `setTimeout`）。
- **数据流**：挂载 SHALL 订阅一次并取初值未读；Channel 消息 SHALL 按 level 呈现（`Info` 不弹）、前插列表、未读 +1；抽屉打开 SHALL 拉首屏（`limit = 50`）并分页；标记已读 SHALL 以命令返回的剩余未读数回写徽标（MUST NOT 由前端自行推算）。
- **跨工作区跳转**：收件箱是 user 维度，列表可能出现别的工作区的通知。点击带 `source_ref` 的行 SHALL 先切换到该行 `root`（本地 `select`，零 IPC）再 `navigate('/changes/{source_ref}')`；同根直接跳转；`source_ref` 缺位或不可达时该行 MUST NOT 跳转、MUST NOT 报错。
- **时间显示**：行内 SHALL 以 `new Date(ms).toLocaleString()` 呈现（仓库既有惯例），MUST NOT 为通知新立时间格式化模块。

#### Scenario: 铃铛与徽标三态

- **WHEN** 未读数为 `0` / `7` / `250` 时分别渲染壳态
- **THEN** 徽标分别呈「不显示」/「7」/「99+」；欢迎态下铃铛不在文档中；点击铃铛打开抽屉并呈现列表首屏

#### Scenario: 抽屉交互闭环

- **WHEN** 打开抽屉，点击某行、再点「全部已读」
- **THEN** 首行置灰（已读）、徽标按 `mark_read` 返回值回写；「全部已读」后徽标归零且列表全部置灰；命令 reject 时列表 inline 呈错误态且抽屉不关闭

#### Scenario: 跨工作区条目先切根再跳转

- **WHEN** 当前根为 A，点击一条 `root = B`、`source_ref = <change id>` 的通知行
- **THEN** 先执行本地切根到 B（零 workspace 动作命令），再导航到 `/changes/<change id>`；同根条目直接导航；`source_ref` 为 null 的行点击无副作用

#### Scenario: 订阅失败与零轮询

- **WHEN** `notification_subscribe` reject
- **THEN** 壳照常渲染（侧栏、页壳、既有视图均可用），无错误 toast、无白屏；全仓该 hook 内零定时器轮询、零文件 watch 依赖，`use-file-watch-signal.ts` 零 diff

### Requirement: 应用不在场的不可能面

本能力 MUST 如实表述其边界，MUST NOT 在 spec、代码注释或 UI 文案中承诺超出以下范围的能力：

- **进程不在场 ⇒ 零通知**：后端进程未运行时不可能产生任何通知（无后台常驻进程）。
- **持久化的真实价值**：跨重启保留历史 + 上一窗口期未读的一次性补拉；MUST NOT 被表述为「进程外通知」「离线通知」或「后台提醒」。
- **OS 级通知不在本能力**：应用最小化时的系统桌面通知是另一个 sink（`tauri-plugin-notification` 系），当前未安装、无 capability 配置；本能力 MUST NOT 引入该插件、MUST NOT 改动 `capabilities/**`。
- **ui 反馈不跨窗口**：某窗口发起的前端反馈只在该窗口弹 toast（不广播），其他窗口不可见——它是本地交互反馈，不是跨窗口事件。

#### Scenario: 进程不在场零通知

- **WHEN** 应用完全退出后触发一次本会产生 `Finished` 事件的流程（下次启动前）
- **THEN** 收件箱零新增行；重启后历史与未读从库内读出（不补造缺席期的通知）

#### Scenario: 重启后读数可恢复

- **WHEN** 写入若干未读通知后关闭并重启应用
- **THEN** `notification_subscribe` 返回的初值与重启前一致，`notification_list` 可读全部历史；无「重启即清零」的路径

#### Scenario: 不引入系统通知

- **WHEN** 审查依赖与 capability 配置
- **THEN** `tauri-plugin-notification` 零依赖、`capabilities/**` 零 diff；前端无系统通知 API 调用

## Module Contract

### Function: `crates/core/notification`（词表与扇出）

| 项 | 签名 / 形态 | 描述 |
|---|---|---|
| `Notification` | struct（`id` / `kind` / `level` / `message` / `body` / `root` / `source_ref` / `created_at`） | 出线 DTO 亦是它；serde 序列化 + specta `Type`；不含读状态 |
| `NotificationLevel` | `Info` / `Success` / `Warning` / `Error` | 对位 sonner 四档 |
| `NotificationKind` | `Event` / `Feedback` | 后端事件 / 前端动作反馈 |
| `Sink::deliver` | `fn deliver(&self, &Notification)` | 同步、无返回值、非 async、无 Result |
| `Hub::new` / `Hub::publish` | `new(Vec<Arc<dyn Sink + Send + Sync>>)` / `publish(&self, &Notification)` | 顺序遍历扇出；三不变量在 crate doc |

### Function: `crates/core/orchestration/src/notify.rs`（投影单点）

| 项 | 签名 | 描述 |
|---|---|---|
| `NotifyContext` | struct（`root` / `change_id` / `change_name`） | 发起侧已读出的上下文（零额外 store 查询） |
| `project` | `(&RunUpdate, &NotifyContext, now: i64) -> Option<Notification>` | 三变体 `None`、三变体 `Some`；主键按幂等串；相位中文名取 `workflow::write::phase_table` |

### Function: `crates/infra/store`（收件箱）

| 项 | 签名 | 描述 |
|---|---|---|
| `NotificationRecord` | 模型（`id` 主键 + 载荷字段 + `read_at`） | user 维度；`///` doc 派生表与字段说明 |
| `Store::publish_notification` | `(&self, &Notification) -> Result<(), StoreError>` | upsert by id，保 `read_at` |
| `Store::list_notifications` | `(&self, offset: u32, limit: u32) -> Result<NotificationPage, StoreError>` | 只 `Event`、`created_at` 降序、页界信封 |
| `Store::unread_notification_count` | `(&self) -> Result<u32, StoreError>` | 只 `Event` 且 `read_at` 为空 |
| `Store::mark_notifications_read` / `mark_all_notifications_read` | `(&self, &[String]) -> Result<u32, StoreError>` / `(&self) -> Result<u32, StoreError>` | 返回剩余未读 |
| `Store::retire_run_notifications` | `(&self, run_id: &str) -> Result<(), StoreError>` | `Finished` 后清同 run 的死未读 |
| `StoreNotificationSink` | `impl notification::Sink` | 落库 sink；失败 `eprintln!("[notification] …")` 一行降级（旁路） |

### API: `src-tauri/src/commands/notifications/mod.rs`（命令面）

| 项 | 变化 | 描述 |
|---|---|---|
| `notification_subscribe` | 新 | `on_event: Channel<Notification>` → `Result<u32, String>`（先订阅后读计数） |
| `notification_list` | 新 | `(offset, limit) -> Result<NotificationPage, String>` |
| `notification_mark_read` / `notification_mark_all_read` | 新 | 返回剩余未读 `u32` |
| `notification_record` | 新 | `(NotificationFeedbackInput) -> Result<(), String>`（不广播）；入参 `{ id, level, message, body }`，`id` 由前端铸造（`crypto.randomUUID()`）；后端固定写 `kind = Feedback` 且 `root` / `source_ref` 为 `None`（前端反馈不做工作区归属断言——门面是 `src/lib/` 纯函数，不持有当前根） |
| `InAppSink` | 新 | 壳侧 `impl notification::Sink`：`broadcast::Sender<Notification>`，零 Tauri 依赖以外的逻辑 |
| `ChangeFlowSink`（`commands/change_flow`） | 改 | 增 `hub` / `store` / `change_name` / `run_id`；`emit` 与 `ask_started` 各调一次投影；`Finished` 后 `retire_run_notifications` |
| `main.rs setup_hook` | 改 | 装配 `broadcast` + `Hub::new([StoreSink, InAppSink])` 并 `manage` |

### CLI / 脚本: 生成链

| 项 | 变化 | 描述 |
|---|---|---|
| `bindings:export` | 零变化 | 新命令与出线类型自动进 `bindings.ts` / `bindings.zod.ts`（幂等） |
| `all_commands!` | 追加五条 | 尾部按登记序追加，既有条目零位移 |
| `api:export` / `api:check` | 零变化 | 新元素缺契约 ⇒ 一条 `docMissing`（warn），`INDEX.toml` 由 `api:export` 重生成 |

### Component: 前端

| 项 | 变化 | 描述 |
|---|---|---|
| `src/lib/notify.ts` | 新 | `present(level, message, body?)` / `notify(level, message, body?)`；sonner 唯一持有者（除 `Toaster`）；不传 `id`；落库静默 |
| `src/hooks/use-notifications.ts` | 新 | 订阅 + 初值未读 + 首屏 50 + 标记已读 + 失败分流；`Channel` 与 `notification_subscribe` 各恰一处 |
| `src/components/notification-bell.tsx` | 新 | 铃铛 + 徽标（99+）+ `StandardDrawer` 收件箱 + 跨根跳转；落 `Desktop.Shell` 的 `path` |
| `src/app.tsx`（`ShellHeader`） | 改 | 挂载铃铛并透传 `root` / `select`；`<Toaster />` 与路由零改动 |
| 6 个既有调用点（`use-workspaces` / `use-agent-instances` / `use-agent-providers` / `use-change-list` / `contract-panel` / `explore-open-file`） | 改 | 12 处 sonner 直调 → 门面；文案逐字不变 |
| `Desktop.Shell`（架构模型） | 改 | `path` 追加铃铛文件；新增 `Shell → Components.Standard.Drawer` 一条边 |

### 零 delta 的能力交叉点（backend-notification-hub）

| 能力 | 理由 |
|---|---|
| `desktop-change-orchestration` | `RunUpdate` / `RunNotice` / 广播面零改动；通知投影是旁路消费者，白名单口径只在 `desktop-notification-hub` 持有 |
| `desktop-change-flow-view` | 「变更通知 → 重查统一视图」例外面零变化；通知不是视图取数通道 |
| `desktop-file-watch` | 该能力的「单点消费件」纪律明文只约束文件失效信号；`Channel<Notification>` 不属其管辖 |
| `desktop-workspace-config` | 零新配置键（渠道开关不在本能力） |
| `desktop-db-inspector` | 自动表格零模型特定代码，新表随全局库 scope 自动出现 |
| `desktop-standard-components` | `StandardDrawer` 零改动（只被消费） |
