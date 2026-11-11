# desktop-agent-chat-infra Specification (Delta)

## MODIFIED Requirements

### Requirement: 展示组件族(对话透镜)

> 2026-10-10 回溯修订(无 change 流直修):保真透镜 `AgentTimeline` 撤编,组件族收敛为 `AgentMessages` 单对话透镜——辅助载体(system 角色消息:run-started / system-notice / raw / run-result;2026-11-06 起 `run-record` 退役,轮行改挂回合元数据)整条不进对话呈现;保真面由适配层状态部件(序列与归因字段见「事件适配层」requirement)与调试页页面级「原始 JSONL」切换承载,不再双透镜并行。

`packages/desktop/src/components/agent/` SHALL 提供共享展示组件族,全部 agent 会话页面(explore 会话 / agent 调试 / change 流程抽屉 / 历史重放 / 未来页面)统一消费:

- `AgentSessionView` 会话级唯一入口: 入参 `root` + `sessionId`(+ 可选 `refreshKey` / composer 插槽),内部经通用查询面装配会话并按会话状态挂流(契约见「会话级统一渲染组件」requirement);消费点 MUST NOT 自行拼装消息、轮行或事件流;
- `AgentMessages` 对话透镜: user / assistant 气泡(**一轮一条 assistant 气泡**,部件按到达顺序)、工具对卡、思考折叠、AskUserQuestion 静态卡(问题与选项原样可读);辅助载体消息(system 角色承载的 data 部件)MUST NOT 进对话呈现,对话透镜 SHALL 基于过滤后列表判定空态;空内容轮的可见性 SHALL 按该轮 `metadata.record.status` 判定而非 `parts` 是否为空(失败 / 停止轮呈轮内状态行,见「轮内状态行与空内容轮可见性」);
- `AgentInput` composer: prompt 输入（空串禁发;纯空白字符不单独拦截——trim 守卫不在组件侧）、permission-mode 档位（`StandardSelect` 选择器）、发送、停止入口;并 SHALL 提供**整面板禁用态**(`disabled` + 禁用理由文案——输入框与发送同时禁用,理由文案原样呈现),供来源侧按业务判据(如 change 流程运行中 / 归档只读,见 desktop-change-chat)整面板禁发;`running` 语义 SHALL 为「呈停止入口」——发送入口不禁用,重复发送由会话层 `isStreaming` 守卫忽略;停止入口 SHALL 只随 `running` 呈现且 MUST NOT 被 `disabled` 隐去(运行中即便整面板禁用仍可从面板停止);`disabled` MUST NOT 复用为「运行中」的第二表达;
- 工具卡注册表: 特化卡(AskUserQuestionCard 等)为可插拔位,未注册工具名回退默认对卡;
- 组件族 SHALL 共享底层块渲染件(文本 markdown / 思考折叠 / 工具卡一套实现);滚动 SHALL 由 `components/ui/message-scroller.tsx` 承载——该 shim 内部以 `@tanstack/react-virtual` 渲染**可见窗口切片**,并通过 sticky 吸附层让用户提问常驻(契约见「对话透镜虚拟窗口与用户消息顶部吸附」);消息状态仍留在应用层,MUST NOT 以分页窗口限制进入 DOM 的消息集合(MUST NOT 保留自研历史分页层)。上游 `message-scroller` primitives 退役后 `@shadcn/react` 在仓内再无**可达**消费者,该依赖与其零消费者残件(`components/ui/questionnaire.tsx`)SHALL 一并剪除(判据沿用 desktop-app-shell「knip 报出的未用导出 SHALL 删减」),MUST NOT 新增 `ignoreDependencies` / `ignore` 条目把未用面关成豁免。

调试页的 run 表单 / 历史列表 / JSONL 开关 SHALL 为页面级 chrome,包在共享核心外圈,MUST NOT 强行塞进组件族。

会话呈现形态 SHALL 归一为一个: 交互页(调试 / explore)在 composer 插槽注入自己的 composer(文本组装留来源侧),只读消费点(change 流程抽屉 / 历史重放)不接插槽;三形态历史差异(发起流独占 / 通知重查转录库 / 裸事件折叠无轮行)SHALL 随本次收编退役,MUST NOT 保留第二套会话渲染实现或第二套转录查询 hook。

#### Scenario: 对话透镜映射

- **WHEN** 会话状态含文本、思考、工具调用与工具结果
- **THEN** `AgentMessages` 按序呈现 markdown 气泡、可展开思考块、成对工具卡,AskUserQuestion 呈静态可读卡

#### Scenario: 辅助载体不进对话

- **WHEN** 会话状态含 run-started / system-notice / raw / run-result 等 system 载体消息
- **THEN** 对话透镜不呈现其任何 data 部件(空态判定基于过滤后列表),seq / `parentToolUseId` 保真仍可考于部件元数据

#### Scenario: composer 运行中重复发送与停止

- **WHEN** 会话处于运行中
- **THEN** `AgentInput` 呈现停止入口,发送入口保持可用;重复发送由会话层 `isStreaming` 守卫忽略(`agent_start` 零发起);停止后恢复可发送

#### Scenario: 会话级入口归一

- **WHEN** 检索全部会话呈现消费点(调试页 / explore 详情 / change 流程抽屉 / 历史重放)
- **THEN** 全部经 `AgentSessionView` 渲染,视图层无折叠入口(`foldSessionMessages`)直调、无自有转录查询 hook、无自有事件流装配

#### Scenario: 只读消费点形态

- **WHEN** change 流程抽屉以只读方式渲染某 WorkerAgent 会话
- **THEN** 组件呈对话透镜与各轮状态(轮行以回合元数据承载、由该轮消息呈现,不再是独立 system 消息),不呈发送 / 停止入口;运行中会话的 delta 流式呈现,收口后与重放形状一致

#### Scenario: 整面板禁用与理由呈现

- **WHEN** 来源侧以 `disabled = true` 与理由文案渲染 `AgentInput`
- **THEN** 输入框与发送按钮同时禁用(空串禁用之外再叠加整面板禁用)、理由文案可见(用户不会「打了字发不出去」);`running` 的停止入口 SHALL 照常呈现(两标志语义正交,禁用只收发送面不收停止面)

### Requirement: 轮内状态行与空内容轮可见性

对话透镜 SHALL 让每一轮在时间线上**可见且带轮状态**，异常与主动停止 MUST NOT 表现为「界面只是不再增长」：

- **每轮恒一条 assistant 消息**：折叠与实时两路 SHALL 都按轮产出恰一条 assistant 消息（id `turn-<turnId>`），空内容轮不例外；轮状态 SHALL 只从该消息的 `metadata.record.status` 读取（MUST NOT 从 `parts` 是否为空反推轮状态）。
- **状态行（`parts` 为空时）**：`status = failed` SHALL 呈现状态行——**友好错误标题 + 可展开的原始错误原文**（原文取自 `metadata.record.error` 与该轮 `api_error` 通知，缺失时回退通用标题）；`status = stopped` SHALL 呈现「已停止」行；`status = completed` 或轮行缺席 SHALL 不渲染。状态行 SHALL 带 `data-testid` 挂钩（供测试与来源侧定位），MUST NOT 依赖样式类名查询。
- **`parts` 非空时**：SHALL 只呈现正常气泡（思考 / 正文 / 工具卡），MUST NOT 额外叠加状态行；轮失败/停止的事实仍可由 `metadata.record` 与顶部提示读出。
- **顶部失败横幅边界**：会话级入口的失败横幅 SHALL 只表达「最新一轮**失败**」（`status = failed`），`stopped` MUST NOT 触发该横幅（用户主动停止不是运行失败）；两处提示并存不冲突（横幅 = 全局最新状态，状态行 = 该轮位置）。
- **过滤单一来源**：对话透镜的可见性判据 SHALL 只在一处维护（system 角色过滤 + 空内容轮按轮状态判定），MUST NOT 在其他组件重复实现第二份判据；该判据经 `src/lib/agent-virtual.ts` 的 `filterConversation` 单点实现，虚拟窗口的项数 SHALL 基于该过滤后列表（MUST NOT 把不渲染的空内容消息计入虚拟列表 `count`）。

#### Scenario: 失败轮空内容呈状态行

- **WHEN** 某轮以 `parts` 空收口，`metadata.record.status = failed`、`error` 携 provider 错误原文
- **THEN** 该轮在时间线该位置呈现状态行：友好标题可见，展开后原始错误原文可见；消息 id 为 `turn-<turnId>`（不参与虚拟列表项数与吸附层几何的错误计数）

#### Scenario: 停止轮空内容呈已停止行

- **WHEN** 用户在运行中点击停止，该轮以 `parts` 空收口、`metadata.record.status = stopped`
- **THEN** 该轮位置呈现「已停止」行（不再是「界面只是不再增长」）；顶部失败横幅 MUST NOT 出现（`stopped` 不算失败）

#### Scenario: 空内容 completed 轮不进时间线

- **WHEN** 某轮 `parts` 空且 `metadata.record.status = completed`（或该轮无轮行）
- **THEN** 对话透镜不渲染该轮（无空气泡、无状态行），虚拟列表 `count` 不计入

#### Scenario: 有内容的轮不叠加状态行

- **WHEN** 某轮 `parts` 非空且 `metadata.record.status = stopped`（停止前已产出部分回答）
- **THEN** 该轮只呈现正常气泡（思考 / 正文 / 工具卡按到达顺序），MUST NOT 追加「已停止」状态行；轮状态仍可从消息 metadata 读出

### Requirement: web 搜索活动与引用来源呈现

前端 SHALL 把一次云端 `web_search` 呈现为**搜索卡 + 正文引用列表**，并保证「空内容轮按轮状态决定是否可见」：

- **承载部件**：搜索活动 SHALL 复用既有工具卡管线承载为 `tool-web_search` 部件（MUST NOT 新增 `AgentDataParts` 成员、MUST NOT 引入独立 `data-citations` 部件）——`planBlock` 产 `tool` 计划、`planToolPart` 产部件、`plansToChunks` 产 chunk、`BUILD_IN_TOOL_CARDS` 注册特化卡，四段均为既有机械管线（`plansToChunks` 与 `planToolPart` SHALL 零改动）。
- **搜索卡**：`WebSearchCard`（`packages/desktop/src/components/agent/agent-blocks.tsx`）标题行 SHALL 呈查询词（`input.query`，缺席时不显）——MUST NOT 出现「正在搜索…」态、MUST NOT 出现超时降级文案、MUST NOT 依赖流内状态切换；body SHALL 呈查询词、来源列表（`title ?? 主机名`，落定后）、错误码（`error` 非空时，与来源列表并存不互斥）。卡片根 SHALL 带 `data-testid="block-web-search"`，查询词行带 `data-testid="block-web-search-query"`，来源列表带 `data-testid="block-web-search-sources"`。判定为「未落定」还是「已落定」仅由 `output` 在场与否决定（无状态文案使两态视觉一致，用户不把「进行中」误读为「加载失败」）。
- **引用列表**：`CitationList` SHALL 挂在正文 text 部件**底部**，条目显示 `title ?? url`，**按 url 去重**；MUST NOT 做行内角标、MUST NOT 做原句高亮。数据来源 SHALL 为 text 部件的 `providerMetadata.agent.citations`（命名空间 `agent`——消费点在 provider 之外，MUST NOT 借用 `anthropic` 命名空间）。
- **外链**：`ExternalLink` SHALL 为**唯一外链调用点**（搜索卡来源与引用列表共用）：仅 `http:` / `https:` 目标的 url SHALL 渲染为可点击链接并经壳侧 opener 打开；其他 scheme（`javascript:`、`file:`、`mailto:` 等）MUST NOT 渲染为链接（降级为纯文本）。壳侧插件注册与 capabilities 权限面见 desktop-app-shell「外链打开插件与 capabilities 最小权限面」。
- **空内容轮可见性**：`AgentMessages` 的过滤判据（`src/lib/agent-virtual.ts` 的 `filterConversation`）SHALL 在既有「system 角色不进对话呈现」过滤之后，按**轮状态**判定空内容 assistant 消息（`parts` 为空）是否呈现——`metadata.record.status` 为 `failed` / `stopped` 时 SHALL 呈现轮内状态行（见「轮内状态行与空内容轮可见性」），为 `completed` 或轮行缺席时 MUST NOT 渲染空气泡；虚拟列表项数 SHALL 基于过滤后列表（不把不渲染的空内容消息计入虚拟列表 `count` 与吸附层几何）。MUST NOT 仅以 `message.parts.length > 0` 作为唯一判据（那会吞掉失败 / 停止轮）。
- **工具卡注册表**：`BUILD_IN_TOOL_CARDS` SHALL 注册内建工具特化卡——`AskUserQuestion`（静态卡）、`web_search`（`WebSearchCard`）、`bash` / `read` / `write`（终端 / 文件读写卡）；`Object.keys` 断言随注册表键面更新。未注册工具名 SHALL 回退默认对卡，且特化卡与默认对卡 SHALL 共用同一卡根钩子（`data-testid="block-tool-use"` + `data-tool-id`）与结果正文钩子（`data-testid="block-tool-result"`，展开后挂载），使消费方无需区分卡型即可定位。
- **两路一致**：实时路（流内）与重放路（库为准）SHALL 收敛到同一卡片形状——实时路 `pause_turn` 期间卡片只显查询词（`output` 未到），轮收口后由重放折叠补出来源列表；`pause_turn` 拆出的两段响应 SHALL 累积在**同一轮同一条 assistant 消息**内（MUST NOT 因拆段产生第二条气泡）；终态一致性由「收口以重放重建」保证（见「会话级统一渲染组件」），MUST NOT 为搜索另立一条收口路径。

> 2026-11-06 agent-stream-single-message 修订:空内容轮可见性判据由 `parts.length > 0` 改为按 `metadata.record.status`(空内容 `failed` / `stopped` 呈轮内状态行,`completed` 不渲染),并明确 `pause_turn` 两段响应累积于同一轮同一条 assistant 消息。搜索卡自身条款(标题行 = 查询词、`title ?? 主机名` 回退、`BUILD_IN_TOOL_CARDS` 注册面与 `block-web-search-query` / `block-web-search-sources` 钩子)按主基线现行口径保留——delta 中该段为较早副本,与本变更自述 `agent-blocks.tsx` / `tool-cards.tsx` 零改动一致,MUST NOT 据其回退。

#### Scenario: 搜索卡标题与两态

- **WHEN** 实时流先到达「仅 use 侧」的搜索块（`pause_turn` 进行中），随后轮收口重放补出配对结果
- **THEN** 两态下标题行均为该查询词；前态 body 只有查询词，后态 body 出现来源列表；全程无「正在搜索…」文案、无状态切换闪烁，且两态都在同一轮的同一条 assistant 消息内

#### Scenario: 来源列表与错误码

- **WHEN** 搜索块携 3 条来源（其中两条同 url）与 `error = "max_uses_exceeded"`
- **THEN** 卡片来源列表按 url 去重后呈 2 条（`title` 缺省时回退主机名），并呈现错误码文本（错误区与来源列表并存）

#### Scenario: 正文引用去重列表

- **WHEN** 一条 text 部件携 `providerMetadata.agent.citations`（含重复 url 与 `title` 缺省项）
- **THEN** 正文底部渲染去重后的引用列表（`title ?? url`），无行内角标、无原句高亮；`citations` 缺席或空数组时 MUST NOT 渲染列表、MUST NOT 渲染空容器

#### Scenario: 非 http 目标不渲为链接

- **WHEN** 搜索来源 url 为 `javascript:alert(1)`、`file:///etc/passwd` 或 `mailto:a@b.c`
- **THEN** 该条目以纯文本呈现（无 `<a>`、无点击打开），不触达 opener 插件

#### Scenario: 空内容 completed 轮不渲染

- **WHEN** 会话状态含一条 `parts` 为空、`metadata.record.status = completed` 的 assistant 消息
- **THEN** 对话透镜不渲染该条（无空白气泡），虚拟列表 `count` 不计入它；其余消息计数与呈现不变

## ADDED Requirements

### Requirement: 对话透镜虚拟窗口与用户消息顶部吸附

对话透镜 `AgentMessages` SHALL 以**虚拟窗口**承载任意规模的会话（MUST NOT 以分页窗口限制进入 DOM 的消息集合），并 SHALL 让用户提问以 sticky 吸附层常驻：

- **全量虚拟窗口**：过滤后消息列表 SHALL 整体作为虚拟器的 `count`（`getItemKey` = 消息 `id`，稳定不随滚动变化；`estimateSize` 按角色分型估值）；内容容器高度 SHALL 为虚拟器 `getTotalSize()`，虚拟项以绝对定位 + `translateY(item.start)` 落位，行距与内容容器内边距 SHALL 经虚拟器的 `gap` / `paddingStart` / `paddingEnd` 表达（MUST NOT 依赖 flex `gap` 或内容容器的 CSS padding——sticky 的被夹边界与绝对定位的包含块均为**包含块的 padding box**，而 `border-box` 下容器 `height = getTotalSize()` 会含 padding，两者叠加使末项越出容器、吸附层跨度对不上账目）。滚动条比例 SHALL 由全量账目导出（一次装载、无「加载更早」进度状态；`remaining` / `history-load-hint` / `history-load-complete` / 触顶 `IntersectionObserver` MUST NOT 存在）。
- **用户提问不进虚拟层**：`role = 'user'` 的索引 SHALL 在虚拟层渲染 `null`（不产节点），但其高度账目 SHALL 照常参与 `count` / `estimateSize` / `getTotalSize`；其真实高度 SHALL 由吸附层经 `ResizeObserver` 实测后以 `resizeItem(index, height)` 回填。实测尺寸 ≤ 0 时 SHALL 回落该索引的 `estimateSize`（虚拟项 MUST NOT 折叠为零高）。
- **sticky 吸附与顶走**：全部 `role = 'user'` 消息 SHALL 在吸附层无条件常驻（MUST NOT 按可见范围过滤其挂载；会话中用户提问量本就有界）。第 i 条用户消息的吸附容器 SHALL 为 `position: absolute` + **`top` 定位**（MUST NOT 使用 `transform`——transform 会创建新的包含块，使 sticky 的夹取基准被替换、气泡吸住后永不顶走），其高度 SHALL 为**该轮跨度** `span = 下一用户消息的 start − 本条 start`（末轮 = `getTotalSize() − start`）；容器内层气泡容器 SHALL 为 `position: sticky; top: 0`。于是「最近一条已完全滚出顶部的用户消息」停在视口顶端，「下一轮提问到达包含块底边」即把上一条顶走（**零缓冲**，不做 gutter）。
- **吸附层祖先链约束**：吸附层与其祖先链 MUST NOT 出现 `overflow: hidden` / `overflow: auto` 一类裁剪（会另立滚动容器，吸附基准由视口变成本层 ⇒ 完全不吸附）；滚动视口（`data-slot="message-scroller-viewport"`）的 `contain-content`（`contain: layout paint`）SHALL 保留（不创建包含块，不影响 sticky）。
- **零装饰**：吸附气泡 SHALL 与普通气泡**同一渲染件**（同一 tinted 变体），只可多一层不透明 `bg-card` 底衬以防圆角边缘透出下层滚动正文；MUST NOT 加顶部渐隐 / 阴影 / 「上方还有内容」提示层。视口既有的 `scroll-fade-b`（**底边**渐隐，`animation-timeline: scroll(self y)` 要求承载元素自身是滚动容器）SHALL 原样留在视口元素上，MUST NOT 迁到内容容器或吸附层。
- **吸附零滚动状态**：吸附判定 SHALL 为纯 CSS 行为——MUST NOT 为吸附引入 `scroll` 事件监听、MUST NOT 维护「当前吸附项」state、MUST NOT 依赖虚拟器可见范围（`range` / `onChange`）。滚回会话顶部时自然无气泡吸顶；无 `role = 'user'` 消息时不渲吸附层。
- **钩子延续**：`data-slot="message-scroller"` / `-viewport` / `-content` / `-item` / `-button` SHALL 保留（既有断言锚点），吸附层容器 SHALL 带独立钩子 `data-slot="message-scroller-sticky"`；气泡 SHALL 继续以 `data-testid="chat-bubble"` + `data-role` 暴露角色（供按角色分查的断言）；`data-autoscrolling`（滚动条透明）与 `data-pending-scroll`（首帧隐形防跳）SHALL 由滚动实例状态与「首次测量完成」标志驱动，样式语义不变；虚拟项 SHALL 保留 `[content-visibility:auto]` + `[contain-intrinsic-size]`（单条超长 assistant 仍为单虚拟项，两级机制叠加）。
- **贴底跟随**：新消息追加时的贴底跟随 SHALL 由虚拟器的端锚定能力（`anchorTo: 'end'` + `followOnAppend` + `scrollEndThreshold`）承担；用户上滑后 MUST NOT 被强制拽回底部，回到底部后恢复跟随。
- **首帧与换会话贴底**：对话透镜**首次挂载即非空、或由空变非空**（`0 → N`，历史重放 / 新会话首轮）与**切换会话**（同一实例的消息集合换成另一会话）两种情形，视口 SHALL 都落在内容末端（贴底）——端锚定只在选项变更且滚动元素已挂载时生效、首帧 `initialOffset` 缺省 0，故这两种情形的贴底 MUST NOT 依赖 `anchorTo: 'end'`，MUST NOT 依赖一次性「首次贴底」闩（那会把换会话漏掉）。复位 SHALL 由滚动承载 shim 依**复位键**发起：复位键由应用层经 `src/lib/agent-virtual.ts` 的 `conversationIdentity` 从消息集合导出（首条携 `metadata.record.sessionId` 的消息的会话 id，全体缺席时为 `null` = 未知），作为 shim 的可选入参 `resetKey` 传入。复位条件 SHALL 恰为二者之一：①消息集合首次非空或由空变非空；②**两端所给 `resetKey` 均非 `null` 且不相等**。命中即在 layout 阶段（浏览器绘制前）调一次 `scrollToIndex(count − 1, { align: 'end' })`，并由 `data-pending-scroll` 遮住这一跳。复位键的一侧为 `null`（首轮 `record` 落库导致 `null → 已知`，或收口重放导致 `已知 → null`）MUST NOT 触发复位；同一会话内追加（键不变）同样 MUST NOT 触发复位。该机制 MUST NOT 要求上游新增入参（`AgentMessagesProps` 形状不变），MUST NOT 引出 `use-agent-session*` / `agent-session-view` / `src/views/**` 任一文件的改动——复位机制 SHALL 全部落在本变更可改面（`agent-messages.tsx` 与 `components/ui/message-scroller.tsx`）内。

#### Scenario: 千条会话只渲可见切片

- **WHEN** 以 1000 条过滤后消息（含若干空内容 `completed` 轮）挂载对话透镜
- **THEN** DOM 中 assistant 气泡数远小于 1000（仅可见窗口 + overscan），内容容器高度为全量 `getTotalSize()`，零「加载更早」提示、零触顶观察器；空内容 `completed` 轮不参与项数

#### Scenario: 用户消息常驻且虚拟层零节点

- **WHEN** 会话含 12 条用户消息与若干 assistant 消息
- **THEN** 12 条用户气泡全部在吸附层 `data-slot="message-scroller-sticky"` 内（各恰一处），虚拟层虚拟项集合内零 `data-role="user"` 节点；高度账目中用户索引仍占位（`getTotalSize()` 含其高度）

#### Scenario: 完全出顶即吸顶

- **WHEN** 滚动使某轮用户提问整体离开视口顶部（其布局位置在视口之上）
- **THEN** 该气泡停在视口顶端（布局 `top` 不变、DOM 只一份），且吸附过程零 `scroll` 监听、零吸附态 state

#### Scenario: 下一轮顶走上一条

- **WHEN** 继续下滚，使下一轮用户提问容器到达吸附容器的底边
- **THEN** 上一轮吸附气泡被推出视口顶端，下一轮气泡接续吸顶（零缓冲、无重叠、无闪烁）；滚回会话顶部时无任何气泡吸顶

#### Scenario: 高度回填与账目一致

- **WHEN** 吸附层各气泡经 `ResizeObserver` 实测完成（吸顶状态下的实测仍反映布局盒高度）
- **THEN** `resizeItem` 把实测高度回填至对应用户索引，`getTotalSize()` 与实测总和一致，虚拟项之间无重叠、末项底边与内容容器底边对齐

#### Scenario: 零测量回落不塌陷

- **WHEN** 某项的实测尺寸为 0（jsdom 无布局 / 图片未加载的瞬时态）
- **THEN** 该项高度回落为其 `estimateSize` 估值，后续项的 `start` 仍连续、无零高项，不抛错

#### Scenario: 视觉一致且视口样式零改动

- **WHEN** 检查吸附气泡与普通气泡、以及滚动视口
- **THEN** 吸附气泡与普通用户气泡同渲染件（仅多一层不透明底衬），零渐变 / 零阴影 / 零提示层；视口仍携带 `scroll-fade-b` / `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content` 与 `data-slot="message-scroller-viewport"`、`aria-label="对话消息"`；吸附层容器样式不含 `transform`

#### Scenario: 上滑不被拽回底部

- **WHEN** 用户上滑离开底部后，会话继续追加新消息
- **THEN** 视口保持用户当前滚动位置（不被拽回底部）；用户滚回底部后恢复贴底跟随，新消息到达即跟到最新

#### Scenario: 首帧挂载即贴底

- **WHEN** 对话透镜挂载时消息集合为 N 条（N > 0，历史重放或新建会话首轮；含从空集合变为非空的情形）
- **THEN** 视口在浏览器绘制前落到内容末端（用户看到的是最新一轮而非会话开头），`data-pending-scroll` 遮住这一跳（无可见跳动）；该跳由滚动承载 shim 自行发起，MUST NOT 依赖 `anchorTo: 'end'` 的选项变更时机

#### Scenario: 切换会话复位贴底

- **WHEN** 同一对话透镜实例的消息集合从会话 A 换成会话 B（两侧复位键均为已知且不相等，中间不经过空集合，条数可相同）
- **THEN** 视口重新落到会话 B 的内容末端（MUST NOT 停留在 A 遗留的滚动位置）；判据为「两侧复位键均非 `null` 且不相等」——MUST NOT 要求上游新增入参（`AgentMessagesProps` 形状不变），MUST NOT 引出 `use-agent-session*` / `agent-session-view` / `src/views/**` 任一文件的改动；同一会话内追加消息（复位键不变）不触发该复位

#### Scenario: 首轮 record 落库不拽回上滑用户

- **WHEN** 新会话首轮流式期间用户已上滑，该轮收口时 `metadata.record` 首次落库（复位键由 `null` 变为会话 id，消息条数不变）
- **THEN** 视口 MUST NOT 被复位到末端（复位条件只认「首次非空 / 由空变非空」与「两侧键均已知且不等」）；用户当前位置保持，回到底部后恢复贴底跟随

## Module Contract

### Function: `packages/desktop/src/lib/agent-virtual.ts`（新增）

| 函数 | 签名 | 描述 |
|------|------|------|
| `hasVisibleParts` | `(message: AgentUIMessage) => boolean` | 可见部件判据（`text` / `reasoning` / `tool-<name>` 计入，`data-*` 与 `step-start` 不计）：自 `agent-messages.tsx` 迁出，两个生产消费点——`filterConversation` 与 `ChatBubbleBase` 的空部件状态行判据（避免判据在两处各写一份） |
| `filterConversation` | `(all: AgentUIMessage[]) => AgentUIMessage[]` | 对话透镜可见性判据单点（自原分页层抽出，语义零改）：剔除 `role === 'system'`；空内容 assistant 消息仅 `metadata.record.status` 为 `failed` / `stopped` 时保留，`completed` 或轮行缺席时剔除 |
| `userTurnSpans` | `(messages: AgentUIMessage[], measurements: { start: number }[], totalSize: number) => { index: number; start: number; span: number }[]` | 吸附层几何：仅为 `role === 'user'` 的索引产条；`span` = 下一条用户消息 `start` − 本条 `start`，末轮 = `totalSize − start`；无用户消息时返回空数组（不渲吸附层） |
| `conversationIdentity` | `(messages: AgentUIMessage[]) => string \| null` | 会话身份（贴底复位键）：取首条携 `metadata.record.sessionId` 的消息的会话 id；全体缺席（流式首轮未收口 / 尚无 `record`）返回 `null` = 未知。同一会话的追加 / 流式增长不改变返回值，换会话必变；`null` 不参与换会话判定（两侧均非 `null` 才比较），故首轮收口（`null → 已知`）不会把上滑用户拽回底部 |

### Component: `packages/desktop/src/components/ui/message-scroller.tsx`（改）

| 组件 / 导出 | Props / 变化 | 描述 |
|------|------|------|
| `MessageScrollerProvider` | 新：`count` / `estimateSize` / `getItemKey` / `renderItem` / `renderSticky` / `resetKey`（+ 可选 `overscan` / `gap` / `scrollEndThreshold`） | 持有 `useVirtualizer` 实例并向子层供 context；取代 `@shadcn/react` 的模式机与 `autoScroll` / `defaultScrollPosition` / `scrollPreviousItemPeek` 入参；首帧贴底与换会话复位由本件内的 layout 阶段 effect 发起（条件：消息集合首次非空 / 由空变非空，或两侧 `resetKey` 均非 `null` 且不等，见「首帧与换会话贴底」），复位能力不外溢为上游入参；`resetKey` 只影响复位时机，MUST NOT 参与虚拟器选项的身份判定 |
| `MessageScrollerViewport` | 结构不变（`data-slot` / `aria-label` / 类名全保留） | 滚动容器：`scroll-fade-b`（底边渐隐，必须留在本元素）/ `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content`；`data-autoscrolling` 由虚拟器 `isScrolling` 驱动、`data-pending-scroll` 由首测完成标志驱动 |
| `MessageScrollerContent` | 高度改由虚拟器 `getTotalSize()` 提供 | 绝对定位的项容器（`position: relative`）；行距与内边距由虚拟器的 `gap` / `paddingStart` / `paddingEnd` 承担（`flex gap-6` 与调用点传入的 `p-3` 一并退役——`border-box` 下 padding 计入 `height`，会使末项越出容器且吸附层跨度与 `getTotalSize()` 账目不符） |
| `MessageScrollerItem` | 保留 `data-slot` 与 `[content-visibility:auto]` / `[contain-intrinsic-size]`；定位改 `position:absolute` + `translateY(start)`；入参 `messageId` / `scrollAnchor` 退役（改由虚拟器索引驱动）；新增 `measureElement` 零测量回落估值 | 可见窗口切片项；零测量回落使 jsdom 与瞬时态不塌陷 |
| `MessageScrollerSticky`（新） | `index` / `start` / `span` / `children` | 吸附层容器：`position: absolute` + **`top`**（禁 `transform`）+ `data-slot="message-scroller-sticky"`；高度 = `span` |
| `MessageScrollerButton` | `data-active` 判据改 `getDistanceFromEnd() > 阈值` | 到底按钮：可见性与贴底状态互斥（语义等价于原「可滚动且未到底」判定） |
| 首帧/换会话复位 | 新增私有 layout effect：条件 =「首次非空 / 由空变非空」或「两侧 `resetKey` 均非 `null` 且不等」；命中即 `scrollToIndex(count − 1, { align: 'end' })` | 复位由 shim 自理，不外溢为上游入参（见「首帧与换会话贴底」）；`resetKey` 只作复位时机判据，MUST NOT 进入虚拟器选项 |
| 退役 | `useMessageScroller` / `useMessageScrollerScrollable` / `useMessageScrollerVisibility` / `scrollToMessage` 再导出、`@shadcn/react/message-scroller` primitives 依赖 | 三项 hook 与 `scrollToMessage` 在 `src/` 零外部消费者；跳转能力由 `scrollToIndex(index)` 承担（`messageId → index` 查表） |
| 退役（依赖面） | `packages/desktop/package.json` 的 `@shadcn/react` 依赖 + `packages/desktop/src/components/ui/questionnaire.tsx`（零消费者残件） | primitives 退役后该包在仓内再无**可达**消费者（`questionnaire.tsx` 全历史零 import，`knip.ignore` 只抑制报告）；按 desktop-app-shell「未用导出 SHALL 删减」一并剪除，MUST NOT 以 `ignoreDependencies` / `ignore` 条目豁免；二期接线问卷组件经 shadcn CLI 重新生成 |

### Component: `packages/desktop/src/components/agent/agent-messages.tsx`（改）

| 组件 / 项 | 变化 | 描述 |
|------|------|------|
| `useHistoryWindow` | **退役** | 过滤判据抽出为 `filterConversation`（`src/lib/agent-virtual.ts`）；分页窗口与 `remaining` 语义退役 |
| `useTopReachObserver` / `HistoryLoadStatus` / `MESSAGE_PAGE_SIZE` / `HISTORY_PRELOAD_MARGIN_PX` / `topSentinelRef` | **删除** | 触顶 `IntersectionObserver` 与 `history-load-hint` / `history-load-complete` 分支整体退役 |
| 虚拟窗口接线 | 新增 | `count` = `filterConversation(messages).length`；assistant 索引走 `renderItem`，user 索引渲 `null`；`estimateSize` 按角色分型（user 72 / assistant 240 起）；`resetKey` = `conversationIdentity(过滤后列表)`（贴底复位键，见「首帧与换会话贴底」） |
| 吸附层 | 新增 | `renderSticky` 渲染 `userTurnSpans` 的全部条目（每条 user 消息一个 `MessageScrollerSticky` + 内层 `position:sticky; top:0` 的 `ChatBubble`）；`ResizeObserver` 实测 → `resizeItem` |
| `ChatBubble` / `PartView` / `TurnStatusLine` / `ToolPartView` / `citationsOfPart` | 逐字不变 | 块渲染、轮内状态行、工具卡分派和 `memo` 语义零改动（吸附气泡复用同一 `ChatBubble`） |
| 三态保留 | 逐字不变 | `agent-messages-loading`（`loading` 且过滤后为空）/ `conversation-empty` / `agent-messages-running` 判据与挂钩不变 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `packages/desktop/src/lib/agent-adapter.ts` / `agent-transport.ts` / `parse.ts` | 过滤判据**语义**搬去纯函数，适配层与传输层零改动（`foldSessionMessages` / `eventToChunk` / `deltaPartId` 签名不变） |
| `packages/desktop/src/hooks/use-agent-session*.ts` | 会话基建零改动（`messages` 形状、`loading` / `running` 语义不变） |
| `packages/desktop/src/components/agent/agent-blocks.tsx` / `agent-input.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx` / `agent-error.ts` | 块渲染件、composer、工具卡注册表、面板骨架、会话级入口入参与插槽、失败横幅判据零改动 |
| `packages/desktop/src/components/ui/` 其余文件 | `bubble.tsx` / `message.tsx` / `trace.tsx` 等零改动（吸附气泡复用同一组件的 tinted 变体）；该目录本变更只改 `message-scroller.tsx`、只删 `questionnaire.tsx`（依赖剪枝的连带删除，见上表「退役（依赖面）」） |
| `packages/desktop/src/views/**` | 消费点接线零改动（仍以 `useAgentSession` / `AgentSessionView` 为唯一入口） |
| `packages/desktop/src-tauri/**` / `src/types/generated/**` | 后端与生成物零改动（纯前端呈现层重构，零命令面 / 信封 / 落库变化） |
| `.dev-team/architecture/**` | 架构模型零改动（纯函数落 `src/lib/**` 属封闭排除面；`components/agent/` 文件集合与 `components/ui/` 收敛口径不变，元素 / 关系 / 计数复述零漂移） |
