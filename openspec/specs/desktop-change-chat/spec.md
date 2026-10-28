# desktop-change-chat Specification

## Purpose

定义 change 详情页左栏**通用 Agent 面板**的能力边界：面板骨架与分栏并排形态、会话归属（`source = "change"` + change id 两段 `source_ref`）与新会话 / 历史会话语义、change 环境边界（worktree 作会话 cwd、store 身份恒为 workspace root）、以及「可发送 / 可开新会话 / 可切历史」的单一判据矩阵。面板本身不实现对话渲染——一律经 desktop-agent-chat-infra 的会话级统一组件注入形态渲染，后端经 desktop-agent-execution 的命令面（`agent_start` 的 `cwd` 位与本能力新增的 chat 清单薄包装）承载。

## Requirements

### Requirement: change 详情左栏通用 Agent 面板

change 详情 SHALL 在**左栏**提供常驻的通用 Agent 面板，与右侧详情区并排（`ResizablePanelGroup orientation="horizontal"`，与探索详情页同构的分栏范式），MUST NOT 以右侧抽屉 / 浮层承载对话：

- **分栏骨架**：左栏 SHALL 由共享骨架 `SessionPanel` 自带的可折叠分栏承载，默认收起（挂载即窄条形态，收起尺寸 32px / 展开尺寸下限 500px / 留窄条 `collapsible`），窄条内 SHALL 留展开入口、展开体头区 SHALL 提供「收起Agent」入口；`DetailHeader` MUST NOT 另设面板显隐开关（折叠入口全部收在骨架内）；右栏 SHALL 保持既有纵向次序（刷新失败提示 → 运行控制面板 → 流程图区）。
- **对话体单一实现**：对话区 SHALL 经会话级统一组件 `AgentSessionView`（desktop-agent-chat-infra）的**注入形态**渲染（来源侧建 `useAgentSession`，composer 经插槽注入），MUST NOT 在视图层自行拼装消息 / 轮行 / 事件流，MUST NOT 引入第二套对话渲染或第二套转录查询 hook。
- **会话头动作面**：面板头 SHALL 提供「新会话」与「历史会话」两个动作（后者为可展开的会话清单），以及一行状态提示（禁发 / 只读理由，见「禁发与只读状态矩阵」）；会话元信息（会话 id / 运行态 / 轮数 / tokens）由 `AgentSessionView` 自带元信息行承载，MUST NOT 复制第二份。
- **面板态归属**：面板本地态（当前 `sessionId` / 清单展开态 / 折叠态）SHALL 挂在页面身份源（URL change id）上——change id 变化时重置（MUST NOT 让旧 change 的会话 id 落到新 change 上）；折叠态由骨架内部持有，change 切换（组件重挂）SHALL 回到默认收起形态；`sessionId = null` 即空态（可发送草案态，见「change 会话归属与新会话 / 历史会话」）。
- **实时面纪律**：面板的交互式会话 MUST NOT 接收详情视图的重评估 tick（`refreshTick`）作 `refreshKey`——该值在 run 期间由每条密封事件驱动，传入会导致整轮反复重放；会话状态重评估 SHALL 由 `useAgentSession` 自身的库态推导与订阅面承担（只读消费点如流程抽屉继续收 `refreshKey`，其语义不变）。

#### Scenario: 分栏并排与折叠

- **WHEN** 打开一个已建档 change 的详情页
- **THEN** 页面呈左右两栏：左栏通用 Agent 面板默认收起为窄条（仍留展开入口）、右栏刷新提示 / 运行控制 / 流程图；点窄条展开入口可展开左栏，点头区「收起Agent」可再收起，右栏内容次序不变

#### Scenario: 对话体归一为会话级组件

- **WHEN** 检索面板实现
- **THEN** 对话与轮行经 `AgentSessionView` 注入形态渲染，composer 经 `composer` 插槽注入；面板内无 `eventsToUIMessages` 直调、无自有转录查询 hook、无第二套对话渲染组件

#### Scenario: change 切换重置面板态

- **WHEN** 在面板内已选一条历史会话后，从列表切到另一个 change 详情
- **THEN** 新 change 的面板以空态呈现（不沿用上一个 change 的 `sessionId`），亦不发起跨 change 的会话取数

#### Scenario: run 期间面板不重放风暴

- **WHEN** 一次 change flow run 运行中（`refreshTick` 随每条密封事件递增），面板已有会话在渲染
- **THEN** 面板的重放查询次数不随 `refreshTick` 增长（仅在 `sessionId` 变化 / 轮收口收敛时发生），MUST NOT 出现每事件一次的全量重放

### Requirement: change 会话归属与新会话 / 历史会话

面板的会话 SHALL 以 `source = "change"`、`source_ref = <change id>`（两段形态，与 change 流程会话的四段 `{changeId}/{phase}/{role}/{attempt}` 互不相交）写入会话归属；列表读取 SHALL 经专用薄命令，选中后 SHALL 一律以 `sessionId` 直查重放：

- **写侧归属**：发送 SHALL 携带 `source = "change"` 与 `source_ref = <change id>`（前端会话参数面透传，MUST NOT 恒为 null）；`cwd` SHALL 同路透传（见「change 环境边界」）。
- **读侧清单**：命令面 SHALL 提供 `list_change_chat_sessions(root, id, offset, limit) -> SessionPage` 薄包装（等价 `session_query(source="change", source_ref=<id>)` 的分页清单，`source` 词常量收在命令层）；blank root SHALL 返回空页；面板 MUST NOT 自行拼装 `source` / `source_ref` 字符串或直调通用清单命令以隐式过滤。
- **空态与可发送性**：打开面板 SHALL 为**空态**（`sessionId = null`：零重放取数、零订阅、零会话行——MUST NOT 打开即自动恢复最近一条历史会话），且该空态 SHALL 是**可发送的草案态**：发送时 `sessionId` 仍为 null（`New` 语义，会话 id 由后端铸造并经 early-resolve 轮行回写），与调试页 `sessionId: null` 即可发送的既有语义同源；MUST NOT 因 `sessionId === null` 禁用输入或发送（否则「新会话 → 首发」不可达）。
- **新会话**：「新会话」动作 SHALL 将 `sessionId` 置 null 并清空对话本地镜像，回到上述可发送空态（可立即起下一会话）；会话行 SHALL 在首条消息发出后才铸造（MUST NOT 预建空会话），已开的旧会话仍 SHALL 可从历史清单重开。
- **首发后可见性**：「当前会话」呈现 SHALL 取会话镜像（会话行回写后的 `session.row.id`），MUST NOT 仅凭本地 `sessionId` 判定空态；面板 SHALL 在会话 id 确立（轮行回写）或首轮收口时重取历史清单，使新铸会话在「历史会话」中可点选重开。
- **历史会话**：清单 SHALL 呈该 change 的 chat 会话（分页；`updated_at` 降序由查询面保证），点选 SHALL 以 `sessionId` 直查 `session_detail` + 会话转录重放装载；MUST NOT 经 `source` / `source_ref` 反查取最近一条（清单筛选属探测语义，寻址与重放仍唯 `sessionId`）。
- **不新增模型字段**：change 侧会话引用 MUST NOT 落 `ChangeRecord` / `ChangeDetail`（零 native model 升级、零 store 迁移）；「当前会话」为前端面板态。

#### Scenario: 归属写入可查

- **WHEN** 在非归档 change 的面板中发出首轮对话并收口
- **THEN** 该会话在库内 `source = "change"`、`source_ref = <change id>`；`list_change_chat_sessions` 返回该会话，而同 change 的流程节点会话（四段 `source_ref`）一条都不出现

#### Scenario: 打开即空态且可发送

- **WHEN** 打开某 change 详情页（该 change 已有若干历史 chat 会话）
- **THEN** 面板为空态（无活跃 `sessionId`、无重放查询、无会话订阅），输入与发送**可用**（空态即新会话草案态）；历史会话只在用户显式展开清单并点选后装载

#### Scenario: 空态首发铸会话行

- **WHEN** 面板处于空态（`sessionId === null`）且 change 为 active、无在飞 run 时输入并发送首条消息
- **THEN** 发起 invoke 的 `sessionId` 为 null（`New` 语义），后端铸造会话行并写入 `source = "change"` / `source_ref = <change id>`；轮行回写会话 id 后面板以该会话呈现，且它随后出现在历史清单中

#### Scenario: 新会话回空态仍可发

- **WHEN** 面板正展示一条已开会话时点击「新会话」
- **THEN** `sessionId` 置 null、对话本地镜像清空，输入与发送仍可用（可立即起下一会话），旧会话仍留在历史清单可重开；库内不因该动作新增会话行

#### Scenario: 历史点选直查重放

- **WHEN** 在历史清单中点击一条既有会话
- **THEN** 面板以该 `sessionId` 直查 `session_detail` 与转录取数重放装载（含元信息与轮行）；选中与重放链路零 `source` / `source_ref` 反查调用

#### Scenario: 新会话不堆积空行

- **WHEN** 连续点击「新会话」三次且不发任何消息
- **THEN** 库内不产生任何会话行（会话在首条消息时才铸造）；已开过的旧会话仍可从历史清单重开

### Requirement: change 环境边界（worktree 为 cwd）

面板发起的会话 SHALL 在 **change 环境**下运行，其执行根（cwd）SHALL 为 `detail.worktree ?? root`，且该值只作会话 cwd、MUST NOT 作 store 身份锚：

- **cwd 派生**：change 带 worktree（`detail.worktree !== null`）时 SHALL 以该绝对路径为 cwd；legacy 记录（`worktree === null`）SHALL 回落 workspace root。cwd SHALL 只进会话 cwd 面（工具路径沙箱根 / bash `current_dir` / 每轮 `AGENT.md` 与前置注入的读取根），**store 寻址 SHALL 恒为 workspace root**（双 root 不变量，与 change 流程同构）。
- **存在性策略**：归档 change（worktree 已被 Cleanup 移除）SHALL 以只读态处理（见「禁发与只读状态矩阵」），MUST NOT 因 cwd 缺失向用户抛发起点 `Err`；非归档 change 的 cwd 缺失 SHALL 由命令层 `is_dir` 探测显式 `Err`（文案含路径），MUST NOT 让失败深埋进引擎工具调用。
- **提示面**：执行根（worktree 路径）SHALL 由 change 详情头的 worktree 信息行（`detail-worktree`）可见；面板内 MUST NOT 再复制一条执行根文案行（避免与详情头重复）。沙箱根收缩（看不到主工作区未提交改动）属既定边界，MUST NOT 以文案承诺跨根可读。
- **范围**：本变更只落 cwd 层——工具面 SHALL 为既有 default 七工具（MUST NOT 为 change 来源新设只读工具集 / 新 `SessionToolset` 变体），提示词前导（stance 模板）显式不在本变更内。
- **续话一致性留痕**：cwd 不落会话配置快照，故非来源侧（如调试页按 `change` 筛选）续话不会携带 cwd——来源侧 SHALL 恒显式传 cwd；按归属反查 worktree 的隐式兜底为已知缺口，MUST NOT 在本变更内隐式实现。

#### Scenario: 会话跑在 worktree 内

- **WHEN** 在带 worktree 的非归档 change 面板中发起一轮包含路径读取的对话
- **THEN** 该轮工具路径解析与 bash `current_dir` 均在 worktree 目录内（沙箱根 = cwd 可验），且会话行落在 workspace root 所属库（workspace 侧清单可查、未新生第二个 workspace 库）

#### Scenario: legacy 记录回落主 root

- **WHEN** 对 `worktree === null` 的 legacy change 发起面板对话
- **THEN** cwd 回落 workspace root，行为与既有 exec / explore 轨会话一致（零新增参数语义）

#### Scenario: cwd 缺失显式失败

- **WHEN** 非归档 change 的 worktree 路径在盘上不存在（被手动删除）时尝试发起面板对话
- **THEN** 发起点返回显式错误（携带该路径），不进入引擎、不产生会话行、不留半截状态

#### Scenario: 工具面与提示词面不扩

- **WHEN** 审查 change 来源会话的工具面装配
- **THEN** 注入面为 default 七工具（无 change 专属工具集分支、无新 `SessionToolset` 变体），首轮提示词无 change 环境前导模板插入

### Requirement: 禁发与只读状态矩阵

面板 SHALL 以单一判据源决定「可发送 / 可开新会话 / 可切历史」，MUST NOT 由各组件各自判定：

| `detail.source` | `activeRun` | 本会话 `running` | 可发送 | 可开新会话 | 可切历史 |
| --- | --- | --- | --- | --- | --- |
| `active` | 非 null | — | ✗ | ✗ | ✓ |
| `active` | null | true | ✗ | ✗ | ✓ |
| `active` | null | false | ✓ | ✓ | ✓ |
| `archive` | — | — | ✗ | ✗ | ✓ |

- **运行中禁发**：判据 SHALL 为 `activeRun !== null` 与本会话库态 `running` 取或（前者防同一 worktree 上 flow 与 chat 并发写，后者沿用既有「运行中禁发」）；禁发 SHALL 连「新会话」一并禁，只留历史浏览（MUST NOT 出现「能开新会话但不能发」的半可用态）。
- **归档只读**：`detail.source === "archive"` 时 SHALL 全禁发（worktree 已被 Cleanup 移除，change 已收口）；历史浏览 SHALL 仍可用。
- **禁用面**：composer SHALL 呈 `disabled` 态（输入框与发送同时禁用）+ 禁用理由文案（运行中 / 只读），MUST NOT 只禁发送按钮而放过输入；`running` 的既有语义（禁发 + 呈停止入口）SHALL 保持。
- **空态不参与判据**：`sessionId === null` MUST NOT 作为禁用理由，也 MUST NOT 豁免上表其余行——可发送行下空态照常可发（首发即 `New`，见「change 会话归属与新会话 / 历史会话」），禁发行（run 期 / 归档）下空态与既有会话同样禁用；禁用理由文案 SHALL 只由矩阵给出，MUST NOT 出现「空态专用」的第三条理由。

#### Scenario: run 期间禁发但可读史

- **WHEN** 某 change 的 flow run 正在运行（`activeRun !== null`）时打开面板
- **THEN** composer 禁用并呈「流程运行中」理由，「新会话」禁用，历史清单展开与点选可用（可读既有会话转录）；面板处于空态时同样禁用（空态不豁免该行）

#### Scenario: 无 run 的 active change 正常收发

- **WHEN** change 为 active 且无在飞 run、当前会话不在运行
- **THEN** 输入与发送可用，「新会话」可用，历史切换可用；面板处于空态时同样可发（首条消息以 `sessionId: null` 发出 → `New`）

#### Scenario: 归档 change 只读

- **WHEN** 打开一个已归档 change 的详情页并尝试在面板输入
- **THEN** 输入框与发送均禁用且呈「已归档 change 只读」理由，「新会话」禁用；历史会话仍可点选浏览

#### Scenario: 空态不被当作禁用门

- **WHEN** 审查面板的禁发判据实现与呈现
- **THEN** 可发送性只由 `detail.source` / `activeRun` / 本会话 `running` 三者决定，`sessionId === null` 既不构成禁用理由也不出现在理由文案中；空态面板无「能输入但发送被拦下」的中间态（因为空态本就可发）

### Requirement: 面板骨架归一（共享 SessionPanel）

change 详情左栏面板的**骨架**（头区动作 / 状态行 / 历史清单 / 折叠窄条）SHALL 抽为共享组件 `SessionPanel`（`components/agent/session-panel.tsx`，经 `components/agent` 桶导出），并由本能力与架构页通用会话（`desktop-archi-chat`）共同消费——它是两处**唯一**的面板骨架实现，MUST NOT 在任一视图侧另存第二套头区动作 / 历史清单 / 折叠窄条：

- **testid 集合**：`data-testid` 集合（`-actions` / `-collapse` / `-new` / `-history` / `-note` / `-history-list` / `-history-item` / `-history-more` / `-history-empty` / `-history-error` / `-rail` / `-expand` / `-session`）与「头区动作 → 状态行 → 历史清单 → 会话体」的渲染次序 SHALL 保留；面板根元素由分栏库占用（`data-slot="resizable-panel"`），消费面以 `-rail` / `-actions` 定位面板分支。
- **testid 前缀参数化**：骨架 SHALL 以 `testIdPrefix` 生成其全部查询挂钩（`change-chat` / `archi-chat` 两套前缀各命中一次），MUST NOT 在骨架内写死单一前缀。
- **业务判据不外移**：可发送 / 可开新会话 / 只读矩阵与禁用理由 SHALL 留在各自来源侧 hook（change 侧仍为 `detail.source` / `activeRun` / 本会话 `running` 三判据），骨架 MUST NOT 自判禁发、MUST NOT 引入第二套判据源。
- **状态行只渲染 `note`**：状态行文案 SHALL 由来源侧 hook 供 `note: string | null`（change 侧 = `disabledReason ??（空态 ? 草案文案 : null）`；`null` = 不占位），骨架 MUST NOT 自行推导文案生成规则。
- **会话流不抽**：骨架 MUST NOT 自建会话流 / 自调 `useAgentSession`——会话状态面由消费者以注入形态传入（既有「对话体单一实现」纪律零变化）。
- **折叠自持**：骨架 SHALL 自持折叠态并自带分栏（`ResizablePanel` + 窄条展开入口 + 头区「收起Agent」），默认收起（收起尺寸 32px、展开下限 500px）；消费者 MUST NOT 传入 `collapsed` / `onToggleCollapsed` 一类的折叠入参，MUST NOT 在视图侧持有第二份折叠 state。

#### Scenario: change 面板经骨架渲染且 testid 集不变

- **WHEN** 打开 change 详情左栏面板并审查渲染树
- **THEN** 头区动作 / 状态行 / 历史清单 / 折叠窄条来自共享骨架组件，`change-chat-*` testid 集合（含 `-collapse`）与文案逐字符合上文清单；既有行为级用例全绿

#### Scenario: 两处共用同一实现

- **WHEN** 检索 change 详情与架构页两处面板的实现
- **THEN** 两处均消费同一骨架组件（仅 `testIdPrefix` / 文案 / 注入的状态面不同），仓库内不存在第二份头区动作或历史清单实现

#### Scenario: 骨架不自判禁发

- **WHEN** 审查骨架组件源码
- **THEN** 其中零 `detail.source` / `activeRun` / `canSend` / `disabledReason` 的业务判定，零 `useAgentSession` 调用；判据与 `note` 全部来自传入的状态面

#### Scenario: 折叠自持

- **WHEN** 审查骨架组件与两处消费面的源码
- **THEN** 折叠态与分栏命令面（`collapse()` / `expand()`）全在骨架内部，消费面零折叠入参、零折叠 state；挂载默认呈窄条，窄条展开入口与头区「收起Agent」互逆

## Module Contract

### Component: `packages/desktop/src/views/changes/agent/`（新增）

| 组件 | Props | 描述 |
|------|-------|------|
| `ChangeChatPanel` | `{ root, changeId, cwd, readOnly, runActive }` | 面板薄封装：会话头（新会话 / 历史会话两个图标动作，`aria-label` 提供可访问名 + 状态提示行）+ `AgentSessionView` 注入形态 + composer 插槽 + 空态提示行（草案态：发送即开新会话，不做禁用门）；折叠窄条与分栏由骨架自持。`data-testid` 挂钩（`change-chat-actions` / `change-chat-collapse` / `change-chat-new` / `change-chat-history` / `change-chat-note` / `change-chat-rail` / `change-chat-expand`）。内部为 `SessionPanel`（`testIdPrefix = "change-chat"`）+ `useChangeChat` 状态面 + `ChangeChatComposer` 插槽 |
| `ChangeChatComposer` | `{ running, disabled, disabledReason, onSend, onStop }` | `AgentInput` 薄适配（`idPrefix="change-chat"`；文案留在本文件，档位 / 发送 / 停止收口在组件族） |

### Component: `packages/desktop/src/components/agent/session-panel.tsx`（新增）

| 组件 | Props | 描述 |
|------|-------|------|
| `SessionPanel` | `{ state: SessionPanelState; title: string; testIdPrefix: string; composer: (actions: AgentSessionActions) => React.ReactNode }` | 会话面板骨架唯一实现：自持折叠分栏与折叠窄条（默认收起 / 窄条展开入口 / 头区「收起Agent」）/ 头区动作（新会话 + 历史 ▾）/ 状态行（`state.note`）/ 历史清单（分页 + 加载 / 空 / 错误态）/ `AgentSessionView` 注入形态 + composer 插槽；全部查询挂钩由 `testIdPrefix` 生成；不含业务判据、不建会话流、不接收折叠入参 |
| `SessionPanelState` | `session` / `sessions` / `total` / `listOpen` / `listLoading` / `listError` / `toggleList` / `loadMore` / `newSession` / `selectSession` / `canSend` / `canStart` / `disabledReason` / `note` / `send` / `stop` | 两处面板的公共子集（会话面 + 清单面 + 动作面 + `note`）；类型命名由 design 定型，语义契约不变 |

### Component: `packages/desktop/src/components/agent/index.tsx`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| 桶导出 | 增 `SessionPanel` / `SessionPanelState` | 既有 `AgentInput` / `AgentSessionView` 导出零变化 |

### Function: `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts`（新增）

| 项 | 签名 | 描述 |
|------|------|------|
| `useChangeChat` | `(params: { root: string \| null; changeId: string \| null; worktree: string \| null; runActive: boolean; archived: boolean }) => ChangeChatState` | 当前 `sessionId` 本地态（`null` = 可发送空态，**不作禁发理由**）+ 历史清单取数（`list_change_chat_sessions` 分页，会话 id 确立 / 轮收口后重取）+ 新会话（置 null + `reset()`，回可发送空态）+ 点选历史（`setSessionId`）+ change id 变化重置 + 禁发判据（`canSend` / `canStart` / `disabledReason`——三态矩阵，不含空态）+ 会话状态面（注入 `AgentSessionView`；「当前会话」取会话镜像） |
| `ChangeChatState`（增位） | `note: string \| null` | `disabledReason ??（currentSessionId === null ? 草案文案 : null）`；判据矩阵、清单取数、新会话 / 历史切换、会话面零改动 |

### API: `packages/desktop/src-tauri/src/commands/exec/mod.rs`（命令面随动）

| 命令 | 签名 | 描述 |
|------|------|------|
| `agent_start`（参数面增位） | `(on_event: Channel<AgentRunMessage>, root, prompt, permission_mode, session_id, source, source_ref, agent, cwd: Option<String>) -> Result<TurnSummary, String>` | `cwd` 只赋 `SessionCtx.workspace_root`（None → root）；`cwd` 非空做 `is_dir` 校验并显式 `Err`；store 半边恒 `for_root(root)`（双 root 不变量） |
| `list_change_chat_sessions` | `(root: String, id: String, offset: u32, limit: u32) -> Result<SessionPage, String>` | 薄包装：`session_query` 分页清单，`source` 常量 `"change"` + `source_ref = id`；blank root → 空页 |

### Component: `packages/desktop/src/views/changes/change-detail-view.tsx`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `DetailLoaded` | 纵向单列 → 横向分栏 | `ResizablePanelGroup orientation="horizontal"`：左栏 = 骨架 `SessionPanel` 自带的可折叠分栏（默认收起）/ 右栏 stale note + 动作面板 + 流程图区；右栏次序不变 |
| `DetailHeader` | 返回列表改图标钮 | 返回入口改为图标按钮（`title` / `aria-label` =「返回列表」），MUST NOT 新增面板显隐开关（折叠入口全在骨架内）；既有刷新 / 标题 / 状态 / worktree 呈现不变 |
