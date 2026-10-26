# desktop-agent-chat-infra Specification (Delta)

## ADDED Requirements

### Requirement: web 搜索活动与引用来源呈现

前端 SHALL 把一次云端 `web_search` 呈现为**搜索卡 + 正文引用列表**，并保证「空部件消息不进对话透镜」：

- **承载部件**：搜索活动 SHALL 复用既有工具卡管线承载为 `tool-web_search` 部件（MUST NOT 新增 `AgentDataParts` 成员、MUST NOT 引入独立 `data-citations` 部件）——`planBlock` 产 `tool` 计划、`planToolPart` 产部件、`plansToChunks` 产 chunk、`DEFAULT_TOOL_CARDS` 注册特化卡，四段均为既有机械管线（`plansToChunks` 与 `planToolPart` SHALL 零改动）。
- **搜索卡**：`WebSearchCard`（`packages/desktop/src/components/agent/agent-blocks.tsx`）summary SHALL **恒为「网络搜索」**——MUST NOT 出现「正在搜索…」态、MUST NOT 出现超时降级文案、MUST NOT 依赖流内状态切换；body SHALL 呈查询词（`input.query`）、来源列表（`title ?? url`，落定后）、错误码（`error` 非空时）。卡片根 SHALL 带 `data-testid="block-web-search"`。判定为「未落定」还是「已落定」仅由 `output` 在场与否决定（无状态文案使两态视觉一致，用户不把「进行中」误读为「加载失败」）。
- **引用列表**：`CitationList` SHALL 挂在正文 text 部件**底部**，条目显示 `title ?? url`，**按 url 去重**；MUST NOT 做行内角标、MUST NOT 做原句高亮。数据来源 SHALL 为 text 部件的 `providerMetadata.agent.citations`（命名空间 `agent`——消费点在 provider 之外，MUST NOT 借用 `anthropic` 命名空间）。
- **外链**：`ExternalLink` SHALL 为**唯一外链调用点**（搜索卡来源与引用列表共用）：仅 `http:` / `https:` 目标的 url SHALL 渲染为可点击链接并经壳侧 opener 打开；其他 scheme（`javascript:`、`file:`、`mailto:` 等）MUST NOT 渲染为链接（降级为纯文本）。壳侧插件注册与 capabilities 权限面见 desktop-app-shell「外链打开插件注册与 capabilities 最小权限面」。
- **空部件消息**：`AgentMessages` 的 `useHistoryWindow` SHALL 在既有「system 角色不进对话呈现」过滤之后追加 `message.parts.length > 0` 过滤——实时路 `pause_turn` 第二段可能只有被消费的孤儿搜索结果而产出 0 部件 assistant 消息，MUST NOT 渲染为空气泡；分页计数 SHALL 基于过滤后列表（不把空消息计入 `remaining`）。
- **工具卡注册表**：`DEFAULT_TOOL_CARDS['web_search'] = WebSearchCard`（注册表 `Object.keys` 断言随之更新）；未注册工具名的回退默认对卡语义不变。
- **两路一致**：实时路（流内）与重放路（库为准）SHALL 收敛到同一卡片形状——实时路 `pause_turn` 期间卡片只显查询词（`output` 未到），轮收口后由重放折叠补出来源列表；终态一致性由「收口以重放重建」保证（见「会话级统一渲染组件」），MUST NOT 为搜索另立一条收口路径。

#### Scenario: 搜索卡恒标题与两态

- **WHEN** 实时流先到达「仅 use 侧」的搜索块（`pause_turn` 进行中），随后轮收口重放补出配对结果
- **THEN** 两态下卡片摘要均为「网络搜索」；前态 body 只有查询词，后态 body 出现来源列表；全程无「正在搜索…」文案、无状态切换闪烁

#### Scenario: 来源列表与错误码

- **WHEN** 搜索块携 3 条来源（其中两条同 url）与 `error = "max_uses_exceeded"`
- **THEN** 卡片来源列表按 url 去重后呈 2 条（`title` 缺省时回退 url），并呈现错误码文本

#### Scenario: 正文引用去重列表

- **WHEN** 一条 text 部件携 `providerMetadata.agent.citations`（含重复 url 与 `title` 缺省项）
- **THEN** 正文底部渲染去重后的引用列表（`title ?? url`），无行内角标、无原句高亮；`citations` 缺席或空数组时 MUST NOT 渲染列表、MUST NOT 渲染空容器

#### Scenario: 非 http 目标不渲为链接

- **WHEN** 搜索来源 url 为 `javascript:alert(1)`、`file:///etc/passwd` 或 `mailto:a@b.c`
- **THEN** 该条目以纯文本呈现（无 `<a>`、无点击打开），不触达 opener 插件

#### Scenario: 空部件消息不渲染

- **WHEN** 会话状态含一条 0 部件的 assistant 消息（`pause_turn` 第二段只有被消费的孤儿搜索结果）
- **THEN** 对话透镜不渲染该条（无空白气泡），`remaining` 分页计数不计入它；其余消息计数与呈现不变

## MODIFIED Requirements

### Requirement: 事件适配层(纯函数,保真)

基建 SHALL 提供纯函数适配层(置于 `packages/desktop/src/lib/agent-adapter.ts`),分三层:入口层、语义层(IR)、发射层:

- `eventsToUIMessages(events: AgentEvent[]): UIMessage[]` — 重放重建(仅消费**密封事件**:会话转录中不存在 delta,delta 不进重放路径)
- `eventToChunk(event: AgentEvent, currentMessageToolCalls?: ReadonlySet<string>): UIMessageChunk[]` — 实时增量(含 delta 路;返回**chunk 组**:事件头 + 块 chunk);可选参为**流内跨消息配对视野**(transport 流内簿记:当前 UI 消息内已发出的工具调用 id 集),命中即就地补全
- **语义层 IR**: `BlockPlan` 为块语义的唯一表达(text / reasoning / tool 三变体;tool 的 `output: null` 即未配对),`planBlock(block, paired): BlockPlan | null` 为**唯一语义点**(五 block 变体 + 无主 result 合成在此收口;`null` = 已配对 result 不重复呈现);孤儿 result 与正常 tool 合并为同一 `tool` 变体(占位工具名 + `input: null`),MUST NOT 在两路各写一遍孤儿判定。`text` 变体 SHALL 携 `citations?: AgentCitation[]`(缺席 = 无引用),`tool` 变体 SHALL 为**唯一工具类承载位**——客户端工具与云端 `web_search` 服务端工具共用该变体(`toolName` 区分,MUST NOT 为搜索新增第四变体)
- **视野层**: `PairingScope` 隔离配对视野差异——折叠路 `global`(跨消息预扫全部 toolUse 的配对 result),实时路 `local`(仅当前 message 内配对);两视野 SHALL 产出同一类型 `Map<toolCallId, PairedOutcome>`(配对载体为工具结果块 **或搜索来源载荷**的联合,与块种类同构),差异 MUST NOT 渗入语义层与发射层。实时路另有**流内跨消息配对视野**(`eventToChunk` 可选参,transport 按「最近开件消息」簿记并在开出新消息时重武装):命中的 toolResult 块 SHALL 就地补全既有消息里的工具部件——只投 `tool-output-available`,MUST NOT 另起消息 / 重投 input;全部块均命中时该事件 MUST NOT 产消息头(`start` / `reset-step`)。**搜索块配对 SHALL 以块 `id` 为键沿用同一视野机制**:use 侧块(`rawUse != null`)与 result 侧块(`rawUse == null`)分处两条消息时由 `global` 视野跨消息合并(`pause_turn` 拆分场景),`local` 视野下无配对 result 的 use 仍渲染(只显查询词)
- **发射层**: `plansToParts` / `plansToChunks` 为纯机械映射(无判定、无视野),两路的块呈现 SHALL 由同一 `BlockPlan[]` 驱动;`plansToChunks` 只产块相关 chunk,事件头(`start` / `reset-step`)仍由 `eventToChunk` 负责。text 计划的 `citations` SHALL 由 `plansToParts` / `plansToChunks` 挂为同一份 `providerMetadata: { agent: { citations } }`(空数组 MUST NOT 挂;`text-start` 与 `text-delta` 带同一 metadata——ai-sdk 的 metadata 合并语义为**覆盖非累加**,故 citations 放一次即可,MUST NOT 指望多次 delta 累加)

映射 SHALL 为: `runStarted` → `data-run-started` 部件;`messageDelta` → provisional 消息上的 `text-delta`(文本增量)/ `reasoning-delta`(思考增量)chunk,配对键挂部件/消息元数据(键形态随信封定稿);密封 message 的 text block → text 部件(整块收口,替换同键 provisional);thinking block → reasoning 部件;toolUse block → tool 部件 input;同 id toolResult block → 该 tool 部件 output(配对收口于适配层,跨消息按 id 配对,MUST NOT 再由视图层全局扫描);**`webSearch` block → `tool-web_search` 部件**(`toolCallId` = 块 `id`、`input = { query }`、`output` 见下);`systemNotice` → `data-system-notice`;`turnDone`(现 runResult,随信封演进) → `data-run-result`(携带 finish 语义);`raw` → `data-raw` 原文透传。`eventToChunk` 的 delta 分支与 `agent-transport.ts` SHALL **零改动**(无搜索实时 delta)。

搜索块的部件载荷口径: use 侧块 → `input = { query }`,`output` 为配对命中时的来源载荷或 `null`;result 侧块(孤儿,use 已被裁)→ `input = null` + `output` 携带来源载荷(信息不丢,与既有孤儿工具卡片同哲学);两者 `output` 形状 SHALL 为 `AgentToolOutput` 的**加位形态**——`content` 保留字符串位(搜索无文本结果,置空串),`isError` = `error != null`,`search?: { results: WebSearchSource[]; error: string | null }` 承载结构化来源与错误(加位可选,既有消费面零改)。

两路契约 SHALL 为**终态同构 + 过程就地补全**:折叠路(重放 / 收口)与实时路 SHALL 在轮收口后呈同一形状(MUST NOT 出现成对工具卡的分裂或重复);实时流式期间跨消息 tool 配对 SHALL 就地补全——结果事件不另起消息、input 与 output 合住同一条消息(与折叠路同形)。配对视野随开新消息重武装,失效残差(结果与 toolUse 之间插入其他消息 / 订阅起点晚于 toolUse)SHALL 回退既有占位卡形态并由轮收口以重放折叠纠正,MUST NOT 以「实时路复跑折叠语义」的方式消除(reducer 的配对视野物理上限于当前消息,复用即降级折叠路能力)。视野差异 SHALL 由视野层快照测试显式锁定(同消息配对两视野相等、未携流内视野时跨消息配对两视野可辨差异),MUST NOT 由单一路径 fixture 掩饰。

保真 SHALL 锁定: `seq` 可考(进 message id 或部件元数据,保序与 key 稳定;delta chunk 元数据携带源 seq);`parentToolUseId` 可考(进部件元数据,子代理归因依赖它);未知事件 MUST NOT 丢弃(一律 `data-raw` 透传原文)。

穷尽性 SHALL 编译期可探测: `auxEventData` 与 `planBlock` SHALL 以 `const _exhaustive: never` 承接变体穷尽(新增 `AgentEvent` / `AgentBlock` 变体时编译报错提醒,MUST NOT 仅靠 runtime 静默降级 `data-raw`);runtime 未知变体的 `data-raw` 兜底语义照常保留(编译期穷尽 + runtime 兜底两层)。

> 2026-10-23 回溯修订(无 change 流直修):实时路补流内跨消息配对视野(`eventToChunk` 可选参 `currentMessageToolCalls` + transport 按「最近开件消息」簿记),tool 结果到达即就地补全、不再另起第二条消息;视野失效残差(插入其他消息 / 订阅起点晚于 toolUse)维持占位卡回退,终态仍由轮收口以重放折叠保证。

> 2026-10-23 anthropic-web-search 修订:`AgentBlock` 增 `webSearch` 变体与 `text.citations` 字段;语义层/视野层/发射层各自扩面,**不新增第四种计划变体、不新增 data 部件成员、不改 delta 路与 transport**(搜索活动走既有工具卡管线,引用挂 text 部件 `providerMetadata`)。

#### Scenario: 全变体映射

- **WHEN** 以密封五变体 `AgentEvent` 序列(runStarted / message(五 block 变体:text / thinking / toolUse / toolResult / webSearch)/ systemNotice / turnDone / raw)驱动 `eventsToUIMessages`
- **THEN** 产出对应的 UIMessage 部件序列:text、reasoning、tool、`data-run-started`、`data-system-notice`、`data-run-result`、`data-raw`,顺序与 seq 一致

#### Scenario: delta 路映射

- **WHEN** `eventToChunk` 先后收到 text delta 与 thinking delta 事件
- **THEN** 产出挂同一 provisional 消息键的 `text-delta` 与 `reasoning-delta` chunk(思考/回复可辨),后续密封 message 到达时整块替换 provisional 内容

#### Scenario: 工具同 id 配对

- **WHEN** toolUse(id=`t1`)与 toolResult(id=`t1`)分处不同密封 message
- **THEN** 适配后 `t1` 收敛为单个 tool 部件(input 与 output 同住),视图层无需跨消息扫描配对

#### Scenario: 实时流跨消息工具结果就地补全

- **WHEN** 实时流中密封 assistant(携 toolUse id=`t1`)后,同 id toolResult 以另一事件到达,且其间未插入其他消息
- **THEN** toolResult 事件只产出 `tool-output-available`(无 `start` / `reset-step`、不重投 input),reducer 归约后 `t1` 的 input 与 output 合住同一条消息(无双卡、无第二条气泡、消息 id 仍为 toolUse 事件的 `evt-<seq>`)

#### Scenario: 搜索块两态与视图层零扫描

- **WHEN** 密封 message 含 `webSearch` use 侧块(id=`srvtoolu_1`)与（另一条消息中）同 id result 侧块
- **THEN** `planBlock` 分别产出 `tool` 计划（use 侧携 `output` 来源载荷、result 侧返回 `null` 不重复呈现）;`eventsToUIMessages` 后 `srvtoolu_1` 收敛为单个 `tool-web_search` 部件（`input = { query }`、`output.search.results` 在场）,视图层无需跨消息扫描

#### Scenario: 引用挂 text 部件元数据

- **WHEN** 密封 text 块携 `citations`(两条同 url 的引用)
- **THEN** 对应 text 部件带 `providerMetadata.agent.citations`(两条原样,去重责任在呈现层);`citations` 为空数组或缺席时不挂该键(`providerMetadata` 不出现空壳)

#### Scenario: 保真字段可考

- **WHEN** 检查适配产出的任一 UIMessage
- **THEN** 可从 message id 或部件元数据读出原事件 `seq`,message 部件元数据保留 `parentToolUseId`(含子代理消息的非空归因)

#### Scenario: 未知事件不丢

- **WHEN** 事件流含未知类型事件(未来引擎新增)
- **THEN** 产出 `data-raw` 部件且原文 JSON 完整保留,适配不报错、不丢事件

#### Scenario: 计划层对拍同形

- **WHEN** 同一 `BlockPlan[]`(text / reasoning / tool 已配对 / tool 未配对四形态)分别喂 `plansToParts` 与 `plansToChunks`(后者经 reducer 归约)
- **THEN** 两路呈现的形状一致(部件类型 / 顺序 / 工具态 / output 载荷逐一相等),对拍不再依赖第三份手写语义投影(`shapeOf` 类辅助退役)

#### Scenario: 视野差异显式锁定

- **WHEN** toolUse 与同 id toolResult 分处两条密封 message(跨消息配对场景)
- **THEN** 视野层快照显式断言: `global` 视野折叠为单个 `output-available` 计划;`local` 视野(未携流内跨消息视野时的纯函数面)产出 `input-available` 计划 + 孤儿卡 `output-available` 计划(差异具体形态被锁定,防扩大);同消息内配对两视野相等

#### Scenario: 收口折叠纠正残差双卡

- **WHEN** 实时流式期间配对视野已失效(结果与 toolUse 之间插入其他消息 / 订阅起点晚于 toolUse),跨消息 tool 配对闪现「占位卡 + 结果卡」双卡
- **THEN** 轮收口以重放折叠纠正为单张 `output-available` 工具卡(终态同构),MUST NOT 出现收口后仍双卡或部件重复

#### Scenario: 变体穷尽编译期可探测

- **WHEN** 审查 `auxEventData` / `planBlock` 实现
- **THEN** 两者均以 `const _exhaustive: never` 承接穷尽;新增 `AgentEvent` / `AgentBlock` 变体而漏改时编译报错,runtime 未知事件的 `data-raw` 兜底仍保留

## Module Contract

### Function: `packages/desktop/src/lib/agent-adapter.ts`

| 函数 / 类型 | 签名 / 变化 | 描述 |
|------|------|----------|
| `AgentToolOutput` | `{ content: string; isError: boolean; search?: { results: WebSearchSource[]; error: string | null } }` | 加位可选（既有消费面零改）；搜索类服务端工具的结构化载荷走 `search` |
| `BlockPlan` | `text` 变体增 `citations?: AgentCitation[]`；`tool` 变体承载搜索（`toolName: 'web_search'`、`input: { query } \| null`） | 三变体不扩面（MUST NOT 新增第四变体） |
| `PairedResults` | `Map<string, PairedOutcome>`（工具结果块 / 搜索来源载荷的联合） | 两视野同形态；块种类自洽 |
| `PairingScope.global` / `local` | 语义扩展（签名不变） | 并入 `webSearch` 块配对：use 侧（`rawUse != null`）为索引源、result 侧（`rawUse == null`）为配对产物；跨消息合并由 `global` 承担（`pause_turn` 拆分场景） |
| `planBlock` | 新增 `webSearch` 分支 | use 侧 → `tool` 计划（配对命中携 `output`，未命中 `output: null`）；result 侧且已配对 → `null`；result 侧且无配对 use → `tool` 计划（`input: null` + `output` 来源载荷） |
| `plansToParts` / `plansToChunks` | 扩 metadata 位（机械映射不变） | text 计划挂 `providerMetadata: { agent: { citations } }`（空数组不挂）；`textChunks` 的 `text-start` 带同一 metadata；tool 分支零改 |
| `eventToChunk` | **零改动** | 无搜索实时 delta（`AgentDelta` 词汇不扩） |

### Component: `packages/desktop/src/components/agent/`（呈现层）

| 组件 / 模块 | 变化 | 描述 |
|------|------|----------|
| `agent-blocks.tsx` | 新增 `WebSearchCard` / `CitationList` / `ExternalLink` | 搜索卡 summary 恒「网络搜索」（`data-testid="block-web-search"`）；引用底部去重列表；外链唯一调用点（仅 http(s)） |
| `tool-cards.tsx` | `DEFAULT_TOOL_CARDS['web_search'] = WebSearchCard` | 注册表可插拔位扩展；未注册回退默认对卡不变 |
| `agent-messages.tsx` | `useHistoryWindow` 过滤追加 `parts.length > 0` | 空部件消息不渲染、不计入分页 `remaining` |

### 零改动面

| 文件 | 说明 |
|------|------|
| `packages/desktop/src/lib/agent-transport.ts` | 无新事件类型需转 chunk；transport 零改动 |
| `packages/desktop/src/hooks/use-agent-session.ts` | 收口语义（流结束按库内转录重建）已天然覆盖搜索卡两态，零改动 |
| `AgentDataParts` | MUST NOT 新增成员（无 `data-citations` / `data-web-search`） |
