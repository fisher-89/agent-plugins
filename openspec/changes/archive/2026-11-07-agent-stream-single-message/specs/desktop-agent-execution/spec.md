# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: agent_stop 终止与提前 resolve

命令面 SHALL 提供终止入口，发起命令 SHALL 维持提前 resolve 契约：

- 发起命令 SHALL 在会话/轮记录落库并进入 running 后**提前 resolve**，返回 running 态记录（id 立即可知）；执行 SHALL 转入内核编排继续（内核持久面持有 store 收尾），终态轮行 SHALL 经 Channel 以 `AgentRunMessage::Record` 信封（`{ ipc: 'record', record: TurnSummary }`）**收尾流出**，MUST NOT 再依赖 invoke 返回携带终态。后端 SHALL NOT 承诺该终态的**前端呈现载体形状**（部件类型、消息 id、是否独立消息）——前端把该轮行挂为对应回合消息的元数据属前端契约（见 desktop-agent-chat-infra「统一会话状态基建」的「record 归元数据」）。
- 终止命令 SHALL 按运行中的会话活动轮寻址: 将轮状态收敛为受控终态 `stopped`、尽力终止引擎执行体（经停止信号取消泵任务（drop future）——机制由 design 定稿核实；寻址键（run/轮 id 沿用或会话键，root 消解跨库歧义保留）由 design 定稿）、经 Channel 流出 `Record` 信封（`record.status = stopped`）。`stopped` SHALL 与 `completed` / `failed` 同列受控终态（用户主动终止与引擎失败语义区分，收敛后 MUST NOT 再接受状态变更）。停止注册（内存态）收内核治理面。**停止轮 SHALL 恒有轮行**：即便该轮零事件（`loop.rs` 停止路径直接返回、不发 `TurnDone`），内核 `converge` 仍铸 `stopped` 终态行——前端据此在该轮位置呈现「已停止」状态行（见 desktop-agent-chat-infra「轮内状态行与空内容轮可见性」），MUST NOT 表现为「界面只是不再增长」。
- 对非 running 目标（已终态或不存在）的终止 SHALL 幂等忽略，MUST NOT 报错崩溃或误改既有终态。
- 停止后的重放 SHALL 与实时一致: 重放路径从会话转录（含 `stopped` 轮行）还原与实时查看相同的呈现（同一轮分组入口折叠，轮行落同一 `metadata.record`）。

#### Scenario: 提前 resolve

- **WHEN** 前端发起会话运行
- **THEN** invoke 在轮记录落库进入 running 后即 resolve 返回 running 态记录（id 可用），引擎事件随后经 Channel 持续流入，终态以 `Record` 信封经 Channel 收尾

#### Scenario: stop 收敛与引擎终止

- **WHEN** 运行中 invoke 终止命令
- **THEN** 轮状态收敛为 `stopped`，Channel 流出 `Record` 信封（`status = stopped`）；运行的泵任务被取消（事件流终止、future drop），无孤儿泵残留；该轮即便零事件也恒有轮行（`stopped`），前端在该轮位置呈现「已停止」状态行

#### Scenario: 幂等忽略

- **WHEN** 对一条已 `completed` 的轮 invoke 终止命令
- **THEN** 幂等忽略: 状态不变、不报错、不产生新事件

#### Scenario: 停止后重放一致

- **WHEN** 停止一轮运行后重新打开该会话查看
- **THEN** 重放呈现与停止时实时呈现一致（含 `stopped` 轮行挂在该轮 `metadata.record`，同一折叠入口），续话以会话最新状态为准

### Requirement: run 事件落库与重放（workspace 维度）

agent 会话持久化 SHALL 以 **workspace 维度**落盘于所属 workspace 的独立 db 文件（落位与寻址见 desktop-workspace-store），MUST NOT 落全局库。持久化纪律（write-through）：

- **密封唯一 durable**：store sink 只落密封类事件（Message / TurnDone / RunStarted / SystemNotice / Raw）；`MessageDelta` 永不落库（redb 不吃每 token 一行，碎事件根因的落库半边修复）
- **write-through**：内核收到密封事件/返回值即 store 原子操作落盘；落库失败 SHALL 显式失败收敛（failed 记因、尽力流出 `Record` 终态轮行），MUST NOT 静默
- **转录挂 session**：密封事件直挂 session（转录单表，键位形态由 design 定稿），run 退化为轮统计行（挂 session 外键）；转录 = 会话全史 = 重放与重建唯一来源
- **对账纠偏**：查询 api 对账重导（从转录重建统计/索引）保留为纠偏机制，非主路径

事件投递 SHALL 为「持久面 + 实时面」两面：内核持久面（write-through 单点落库）+ 会话实时面（同一事件源的两个出口——发起命令的 `Channel<AgentRunMessage>` 与会话订阅命令 `agent_session_watch` 的 `Channel<AgentRunMessage>`，同源同构、同一信封）；delta 只走实时面（`messageDelta` ephemeral，永不落库），密封事件两面同源同戳。历史查看 SHALL 走所属 workspace 库的会话查询重放（携 root），MUST NOT 依赖全局事件广播或要求运行进程存活。MVP MUST NOT 实现留存清理（转录无上限增长为已知限制留痕）。

轮归属 SHALL 为**消费侧的轮分组唯一依据**：每条事件在信封顶层携带 `turnId`（生产点唯一，见「AgentEvent 信封与 loop 可见性」），前端折叠与实时路 SHALL 以 `turnId` 分组；`RunStarted` MUST NOT 被用作轮边界或轮计数依据——停止轮 / 失败轮 / 重订中途可能看不到它，零事件轮（ask 送达失败）连一枚事件都没有，按它推断会令后续轮整体错位。`TurnDone` SHALL 只用于该轮轮行缺席时兜底 status（`isError ? failed : completed`），MUST NOT 用于轮边界或轮行配对。`turnId` 为 `null` 的旧转录 SHALL 按 `RunStarted` 出现顺序分组（等价既有行为），MUST NOT 因此新增数据迁移或第二套分组逻辑。后端对前端折叠方式零契约要求（信封字段与轮行字段均已在前置变更定稿，本变更零后端改动）。

#### Scenario: write-through 同源投递

- **WHEN** 运行产生密封事件
- **THEN** 每密封事件到达实时面（发起流与订阅流同源）与 store 且以同一盖戳同源；delta 只出现在实时面、永不出现在 store

#### Scenario: 两条实时出口载荷一致

- **WHEN** 同一轮运行既有发起方在听、又有旁观方经 `agent_session_watch` 订阅
- **THEN** 两个出口收到同一事件序列（seq / 载荷逐一相同，含 delta），无出口专属加工

#### Scenario: delta 零落库

- **WHEN** 一轮含 N 个 text delta 的回复收口后审查所属 workspace 库
- **THEN** 该轮落库序列恰一条密封 Message，无 delta 记录；轮统计行收 TurnDone 字段面

#### Scenario: 重放不依赖进程

- **WHEN** 会话收口后（或应用重启后）在原 workspace 下点开历史会话
- **THEN** 经会话查询（携 root）还原全史密封转录时间线，与实时流内容一致

#### Scenario: workspace 库落位与隔离

- **WHEN** workspace A 与 B 各有会话历史后审查两库内容与查询结果
- **THEN** A 的会话/转录/统计仅存在于 A 的 workspace 库文件，B 的任何查询不可见（反之亦然）；全局库无会话数据

#### Scenario: 轮归属可自证而不靠 RunStarted 计序

- **WHEN** 一条会话转录含三轮：第 1 轮正常收口（有 `RunStarted` 与 `TurnDone`）、第 2 轮用户运行中停止（停止路径不发 `TurnDone`、无 `turnDone` 事件）、第 3 轮正常收口
- **THEN** 每枚事件的信封 `turnId` 各自指向所属轮，消费方按 `turnId` 分组后三轮的轮行与事件一一对位（第 2 轮不缺席、第 3 轮不错位）；以 `RunStarted` 出现次数计序会得到错位结果，故该口径 MUST NOT 被采用；第 2 轮的 status 由轮行（`stopped`）而非 `TurnDone` 支撑

### Requirement: Agent 调试页

前端 SHALL 提供 Agent 调试页（`AgentDebugView`），经侧栏「系统工具」组进入；视图切换 SHALL 沿用本地 state，MUST NOT 引入路由。页面 SHALL 包含：

- **参数面（最小集）**：prompt（必填）、permission-mode 三档下拉（默认 bypassPermissions）、agent 选择器（管理页 agent 实例清单，默认选中默认 agent——与组合根缺省解析一致；**仅调试页暴露**，正式场景无 agent 选择入口）；cwd 不设参数（隐含当前 workspace root）、model 不设参数、env 不设参数
- **事件时间线**：SHALL 经会话级统一组件 `AgentSessionView`（desktop-agent-chat-infra，对话透镜 `AgentMessages`）呈现——**运行中 token 级流式**（delta 累积进**该轮唯一 assistant 消息**的当前 step 部件，思考/回复可辨，密封到达追加整块而非另起消息，无碎行、无第二条气泡）、气泡序（按行到达序）、tool_use / tool_result 成对卡；run-started / system-notice / result / raw 等辅助载体逐事件各为独立 system 消息且不进对话呈现（保真原文经下方「原始转录切换」与部件元数据可考）；两引擎共用同一会话组件
- **运行中停止**：运行中 SHALL 呈现停止入口，触发终止命令（见「agent_stop 终止与提前 resolve」）
- **原始转录切换**：落库密封转录（含 Raw）原文可见（与 store 一致）；delta 仅实时面可见，不进入原始转录视图
- **历史会话**：会话列表 → 点开经同一 `AgentSessionView` 重放该会话（含轮行；invoke 查询 + 状态驱动挂流）

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

#### Scenario: 历史会话运行中旁观流式

- **WHEN** 从历史列表点开一条仍在运行的会话（有 running 轮行）
- **THEN** 组件以 `since_seq` 订阅该会话并 token 级流式呈现（不经发起流），轮收口后以重放收敛为与重开一致的形状

#### Scenario: 停止轮可观可见

- **WHEN** 在调试页运行中点击停止，该轮未产出任何 assistant 密封消息（零内容停止）
- **THEN** 时间线在该轮位置呈现「已停止」状态行（不再是「界面只是不再增长」）；顶部失败横幅 MUST NOT 出现（`stopped` 不算失败）；轮行 status 为 `stopped` 可从该轮消息 metadata 读出

## Module Contract

### 零改动面

| 模块 | 说明 |
|------|------|
| `packages/desktop/src-tauri/**` | **零改动**：信封 `turn_id`（出线 `turnId`）、`AgentEvent::stamp` 单点、`SessionStreamRegistry`、`agent_session_watch` 与 `since_seq` 过滤、`agent_stop` 幂等、`converge` 铸 `stopped` 轮行全部不变；本变更只改前端如何消化同一批信封 |
| `crates/infra/agent/src/sdk/{loop,normalize,runner}.rs` / `crates/core/agent/src/kernel.rs` | **零改动**：停止路径不发 `TurnDone` 的既有行为保留（由内核 `converge` 铸 `stopped` 轮行兜底），MUST NOT 为「前端轮分组」而新增事件或信封字段 |
| `packages/desktop/src/types/generated/**` | **零改动**：`bindings.ts` / `bindings.zod.ts` MUST NOT 重导出（本变更零信封字段增量） |
| `src/commands/exec/**` | **零改动**：`agent_start` / `agent_session_watch` / `agent_stop` / `session_detail` 签名与语义不变 |

### 前端契约面（消费口径，详述见 desktop-agent-chat-infra）

| 模块 | 变化 | 描述 |
|------|------|------|
| `packages/desktop/src/lib/agent-transport.ts` | `Record` → `finish` chunk 的 `messageMetadata` | 终态轮行落该回合消息元数据；后端 `Record` 信封载荷形状零改动 |
| `packages/desktop/src/lib/agent-adapter.ts` | 折叠按 `turnId` 分组、轮行挂 `metadata.record` | 轮归属口径的唯一实现点 |
| `packages/desktop/src/hooks/use-agent-session.ts` | `buildMessages` 删除，统一走 `foldSessionMessages` | 消除 `TurnDone` 计数配轮行的错位路径 |
| `packages/desktop/src/components/agent/agent-messages.tsx` | 空内容轮按 `metadata.record.status` 判定 | 停止轮 / 失败轮不再从时间线消失 |
