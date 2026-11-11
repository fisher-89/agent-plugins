# 提案: agent-messages-virtual-list-pinning

> **变更**: agent-messages-virtual-list-pinning
> **日期**: 2026-11-12
> **状态**: draft
> **来源**: `openspec/changes/agent-messages-virtual-list-pinning/explore.md`（requirement 类型，三轮探索 + 终局决策台账）
> **前置**: `agent-stream-single-message` 已合入（一轮一条 assistant 消息 + step 分段 + 轮状态行 + 过滤判据按 `metadata.record.status`）
> **性质**: 纯前端呈现层重构——对话透镜从「自研分页窗口 + 全量 DOM」改为「全量虚拟窗口 + 用户提问 sticky 常驻吸附」。**Rust 后端、适配层、transport、会话基建 hook 全部零改动**。

---

## 问题

### 现状链路（读过的真实代码）

```
AgentSessionView (components/agent/agent-session-view.tsx)
  └─ AgentMessages (components/agent/agent-messages.tsx)   ← 唯一对话透镜
       ├─ useHistoryWindow()  自研分页窗口：默认最新 10 条，触顶每次 +10
       ├─ MessageScrollerProvider autoScroll defaultScrollPosition="end"
       └─ MessageContainer
            ├─ MessageScroller / Viewport(aria-label=对话消息) / Content(p-3)
            ├─ topSentinelRef  ← IntersectionObserver(rootMargin 300px) 触发 loadMore
            └─ messages.map → MessageScrollerItem(messageId, scrollAnchor=role==='user')
                              └─ memo(ChatBubble) → TextBlock / ThinkingBlock / ToolPairCard…
```

### 五个具体症状

1. **分页不是性能兜底，只是「慢一点装满」**。`MESSAGE_PAGE_SIZE = 10`，触顶每滚一次 +10，装满即**全量进 DOM**。长会话（数千轮）的首屏成本由 DOM 节点数决定；`MessageScrollerItem` 的 `[content-visibility:auto]` + `[contain-intrinsic-size]` 只压绘制成本，不压节点数。
2. **悬空的分页状态机**。装载进度靠自研 `offset` 状态 + `IntersectionObserver` + `HISTORY_PRELOAD_MARGIN_PX = 300` sentinel 维护：换会话要复位、观察器要随 `remaining` 重挂、`remaining` 归零后还要撤销观察器（`useTopReachObserver`）。这些状态与 DOM 无关，却必须随渲染同步——它是「为分页而分页」的复杂度。
3. **与 shadcn 滚动引擎的意图错配**。`@shadcn/react@0.3.1` 的 `message-scroller` 是一台完整滚动引擎（模式机 `following-bottom` / `free-scrolling` / `anchored-to-message` / `settling-jump`；`MutationObserver` + rect 扫描 + `preserveScrollOnPrepend` + `scrollPreviousItemPeek`）。它的每一条前提都是「**全量 DOM**」——一旦引入虚拟化（只有窗口切片在 DOM），这些机制全部失效且无法局部修补（explore §1 / §2 / §11 已逐条核对）。而其中我们真正在用的只有样式面（`scroll-fade-b` / `scrollbar-thin` / HUD 属性 / `data-slot` 钩子）与一个到底按钮。
4. **呈现缺口：长会话里「我在看哪一问的回答」不可知**。需求方要求：最近一条**已完全滚出顶部**的用户提问吸附在视口顶端，下一轮提问到达时把它顶走。今天没有任何实现（`scrollAnchor` 只是把 user 消息标成锚点，交回 shadcn 引擎做 peek，不是吸顶）。
5. **「打开会话即落在最新一轮」当前的实现方式随分页一起消失**。今天它靠 shadcn 引擎的 `defaultScrollPosition="end"` 一次性定位；退役该引擎后，虚拟器的 `anchorTo:'end'` **不覆盖**首帧与换会话——源码核对：端锚定分支只在「已有上一轮选项且滚动元素在场」时进入，首帧 `initialOffset` 缺省 0，故两种入口都必须由滚动承载显式补一次跳转。若不显式补上，长会话打开会停在会话开头、切到另一个会话会停在上一会话遗留的位置——这是本变更必须自己承接的语义（AC-13）。

探明的关键事实（压缩自 explore）：

- `useMessageScroller` / `useMessageScrollerScrollable` / `useMessageScrollerVisibility` / `scrollToMessage` / `scrollToEnd` / `scrollToStart` 在 `src/` 全域**零外部消费者**（唯一消费点 `agent-messages.tsx` 只用组合件 `Provider` / `Button` / `Content` / `Item` / `Viewport`）——shim 可自由改造，不背兼容包袱。
- TanStack `@tanstack/react-virtual@3.14.26`（`virtual-core@3.19.11`）公开面已备齐本需求所需：`anchorTo: 'end'` + `followOnAppend` + `scrollEndThreshold`（贴底跟随）、`getItemKey`（稳定键）、`gap`（行距）、`resizeItem(index, size)`（外部回填实测高度，`index.d.ts:186`）、`getDistanceFromEnd()` / `isAtEnd()`（到底按钮）、`getOffsetForIndex` / `scrollToIndex`（替代 `scrollToMessage`）。peer 仅 react / react-dom，已核实可装。
- `scroll-fade-b` 是**底边**渐隐（`shadcn/dist/tailwind.css:317`，`--scroll-fade-b` 为底部透明段），与顶部吸附层零交集；它靠 `animation-timeline: scroll(self y)` 生效，**要求承载元素自身是滚动容器**——因此 MUST 留在 Viewport 上。
- jsdom 无布局（`clientHeight = 0`、`getBoundingClientRect` 全 0）⇒ 不打桩时虚拟窗口 range 恒空，**所有气泡断言会变成 0 条**；仓库已有 `offsetHeight` / `clientHeight` 打桩先例（`architecture-view.test.tsx:189`、`stdout-panel.test.tsx:366`）。

---

## 提案

### 一、总决策：退役分页，全量虚拟化（W2）

```
messages(入参) ─ filterConversation() ─ virtualizer(count = 过滤后长度)
                                         ├─ getItemKey = messages[i].id
                                         ├─ estimateSize = 按角色分型常量
                                         ├─ gap = 24（取代 flex gap-6）
                                         └─ anchorTo:'end' + followOnAppend + scrollEndThreshold
```

- **数据一次给全**：调用方本来就把全量 `messages` 传入（分页只在透镜内部发生），退役分页不改变入参契约。
- **渲染按需**：content 高度 = `getTotalSize()`，虚拟项以 `position:absolute; top:0; transform:translateY(start)` 落位；行距与内边距走虚拟器的 `gap` / `paddingStart` / `paddingEnd`（现有 `Content` 的 `flex gap-6` 与 `p-3` 退役——sticky 的夹取边界与绝对定位包含块都是 padding box，而 `border-box` 下 padding 计入 `height`，二者叠加会让末项越出容器、吸附层跨度与账目对不上）。
- **删除面**：`useHistoryWindow`、`useTopReachObserver`、`HistoryLoadStatus`、`MESSAGE_PAGE_SIZE`、`HISTORY_PRELOAD_MARGIN_PX`、`remaining` / `history-load-hint` / `history-load-complete` 全部分支与 `topSentinelRef`。
- **保留面**：`filterConversation` 的判据语义**逐字不变**（system 角色过滤 + 空内容轮按 `metadata.record.status`），只是从 hook 里抽成纯函数。

### 二、滚动承载改造：`components/ui/message-scroller.tsx` 军民分界

按 explore §11 的军民分界表落地——**保留 UI，与 headless 组件结合，直接改 shim**：

| 保留（真实在用的 UI 面） | 虚拟化后如何维持 |
|---|---|
| `scroll-fade-b`（底边渐隐） | **原样不动**（纯 CSS + `animation-timeline: scroll(self y)`，承载元素仍是 Viewport） |
| `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content` | 纯类名，原样不动 |
| `data-slot="message-scroller"` / `-viewport` / `-content` / `-item` / `-button` | 保留（既有断言锚点） |
| `data-autoscrolling`（滚动条透明） / `data-pending-scroll`（首帧隐形防跳） | 由虚拟器 `isScrolling` 与「首次测量完成」标志驱动 setAttribute；`data-pending-scroll` 同时遮住首帧贴底那一跳 |
| 到底按钮 `data-active` | 换成 `getDistanceFromEnd() > 阈值`（语义等价：可滚动且未贴底才呈现） |
| `[content-visibility:auto]` + `[contain-intrinsic-size]` | 保留于虚拟项（单条超长 assistant 仍是单虚拟项，两级机制叠加） |

| 退役（交给 virtualizer） | 取代 |
|---|---|
| 模式机四态 | `anchorTo:'end'` + `followOnAppend` + `scrollEndThreshold` |
| `preserveScrollOnPrepend` + prepend 锚定 | W2 无 prepend；虚拟器自带边键变化锚定 |
| `scrollPreviousItemPeek(64)` | sticky 吸附层 + assistant 项 `gap` |
| rect 扫描 / `MutationObserver` / 全量 `getBoundingClientRect` | 虚拟器单点 ResizeObserver 测量 |
| `useMessageScroller*` 三个 hook 与 `scrollToMessage` 再导出 | 零外部消费者，直接删除；跳转能力经 `scrollToIndex(index)` |

**首帧与换会话贴底由 shim 自理，不外溢为上游入参**：虚拟器的 `anchorTo:'end'` 只在「选项变更且滚动元素已挂载」时生效、首帧 `initialOffset` 缺省 0，因此「挂载即贴底」与「换会话贴底」不在它的职责内。复位键由 `src/lib/agent-virtual.ts` 的 `conversationIdentity(messages)` 导出（取首条携 `metadata.record.sessionId` 的消息的会话 id；全体缺席返回 `null` = 未知），经 `MessageScrollerProvider` 的可选入参 `resetKey` 传入 shim。复位条件恰为二者之一：①消息集合首次非空或由空变非空；②两侧复位键均非 `null` 且不相等。命中即在 layout 阶段（浏览器绘制前）调一次 `scrollToIndex(count − 1, { align: 'end' })`；任一侧键为 `null`（首轮 `record` 落库 / 收口重放）或键不变（同会话追加）都不复位，以免拽走已上滑的用户。该通道只在 `agent-messages.tsx` 与 shim 之内流转 ⇒ `AgentMessagesProps` 形状不变、`use-agent-session*` / `agent-session-view` / `src/views/**` 零 diff（一次性 `count > 0` 闩会把「换会话贴底」漏掉，故 MUST NOT 用「只贴一次」的闩）。

`@shadcn/react/message-scroller` 的 primitives 不再被 shim 引用后，该包在本仓再无**可达**消费者（另一处引用面 `components/ui/questionnaire.tsx` 全历史零 import——`knip.ignore` 只抑制报告、不使其可解析）。按 `desktop-app-shell`「knip 报出的未用导出 SHALL 删减」同批剪枝：删 `src/components/ui/questionnaire.tsx`（零消费者 / 零测试引用 / 零 archi 元素 / 零 API 契约登记）并从 `dependencies` 移除 `@shadcn/react`。MUST NOT 改用 `knip` 的 `ignoreDependencies` / `ignore` 条目消解（`desktop-archi-api` 明文禁「关判据而非满足判据」）。`desktop-agent-execution` 的「questionnaire 组件接线为二期」只约束**接线**留待二期（AskUserQuestion 当前由 `tool-cards.tsx` 静态卡承载），MUST NOT 读作「必须保留一个零消费者文件」；二期接线经 shadcn CLI 重新生成即可。

### 三、用户提问吸附：容器内不可见占位块 + `position: sticky`

```
MessageScrollerContent   position:relative   height = virtualizer.getTotalSize()
 │
 ├── 虚拟层（@tanstack/react-virtual）
 │     assistant 索引 → position:absolute  transform:translateY(item.start)   ← 可见切片
 │     user 索引      → 渲染 null（高度账目仍占位）
 │
 └── 吸附层（全部 user 消息无条件常驻，永不进虚拟窗口的渲染集合）
       <div data-slot="message-scroller-sticky" style="position:absolute; top:<start>px">  ← 不用 transform
         <div style="height:<turnSpan>px">                                                 ← sticky 的包含块
           <div style="position:sticky; top:0"><ChatBubble …/></div>                       ← 真实 tinted 气泡
```

- **吸附是纯 CSS 行为**：`position: sticky` 的吸附边界是滚动祖先（Viewport）的 padding box，被顶走边界是包含块（`turnSpan`）底边。因此「下一轮把上一轮顶走」不需要任何 scroll 监听、不需要 range 过滤、不需要组件状态。
- **几何单源**：`turnSpan(i) = 下一 user 的 start − 本条 start`（末轮 = `totalSize − start`），由虚拟器测量账目直接算出（`src/lib/agent-virtual.ts` 的纯函数）。
- **高度回填**：吸附层每个气泡用 `ResizeObserver` 实测（`borderBoxSize` 反映布局盒，吸顶时量到的仍是真实高度）→ `virtualizer.resizeItem(userIndex, h)`，使 `totalSize` 与实际布局一致（虚拟项无重叠、无空洞）。
- **零装饰**：吸附气泡与普通气泡**同一个 `ChatBubble`、同一个 tinted 变体**，只多一层不透明 `bg-card` 底衬（防圆角边缘透出下层滚动正文）；MUST NOT 加渐变 / 阴影 / 「上方还有内容」提示。
- **零缓冲**：下一轮提问紧贴包含块底边即顶走，不做 gutter（explore §21 的唯一开放项按此收口）。

三条结构前提（违反即静默失效，explore §23 已登记）：

1. 吸附层只能用 `top` 定位，**不得用 `transform`**（transform 会创建新包含块，sticky 的夹取基准被换掉 ⇒「吸住后永不顶走」）；
2. 吸附层祖先链不得出现 `overflow: hidden/auto`（会另立滚动容器 ⇒「完全不吸附」）；Viewport 的 `contain-content`（`contain: layout paint`）不创建包含块，核对通过；
3. `scroll-fade-b` 必须留在 Viewport 上（挪走则 `animation-timeline: scroll(self y)` 失效）。

### 四、纯函数落点：`src/lib/agent-virtual.ts`（替代 explore 建议的 `components/agent/virtual-messages.ts`）

| 函数 | 签名 | 用途 |
|---|---|---|
| `hasVisibleParts` | `(message: AgentUIMessage) => boolean` | 自 `agent-messages.tsx` 迁出；两个生产消费点（`filterConversation` 与 `ChatBubbleBase` 的空部件状态行判据），避免判据写两份 |
| `filterConversation` | `(all: AgentUIMessage[]) => AgentUIMessage[]` | 从 `useHistoryWindow` 抽出，判据语义零改（system 过滤 + 空内容轮按 status） |
| `userTurnSpans` | `(messages, measurements, totalSize) => { index, start, span }[]` | 吸附层占位块几何（`span` = 下一 user `start` − 本条 `start`，末轮 = `totalSize − start`） |
| `conversationIdentity` | `(messages) => string \| null` | 贴底复位键（首条携 `metadata.record.sessionId` 的会话 id；全体缺席 `null` = 未知）——供 shim 的 `resetKey` 判「换会话」 |

落 `src/lib/` 的两条硬理由：

- **knip**：`project` 排除 `*.test.*`，组件内为测试开 `export` 会被判 unused；`src/lib/**` 有既有同型先例（`agent-adapter.ts` / `agent-transport.ts` / `explore-groups.ts`）且**纯函数 + 生产消费点**成立。
- **架构模型零漂移**：`repo-archi-model` 规定「`src/components/agent/*.{ts,tsx}` 每文件一元素」并把前端元素/关系计数写在多处以供机械核对（51 元素 / 113 关系、`06` 段 28 元素 / 64 关系）。新增 `components/agent/virtual-messages.ts` 会强制同 change 追加元素并把三处计数复述改成 52 / 114（`06` 段 29 / 65）——纯噪声；`src/lib/**` 是**封闭排除面**（不建元素、不贡献边），落此处模型零 diff。

### 五、测试可渲染性：打桩 + 零测量护栏

- `test-setup.ts` 增虚拟化打桩：给 `[data-slot="message-scroller-viewport"]` 非零 `clientHeight` / `scrollHeight`（按 `data-slot` 限定，不影响其余套件，也避开 `stdout-panel.test.tsx` 一类逐元素 `Object.defineProperty` 打桩）。
- 测量零值护栏写进 shim（生产语义同样成立）：`measureElement` 实测尺寸 ≤ 0 时回落 `estimateSize(index)`，虚拟项**MUST NOT 折叠为零高**。这样 jsdom 里估算值即几何，断言可确定性推导。
- 条数断言改造：吸附层使「`chat-bubble` 总数」不再等于「可见气泡数」——**按 `data-role` 分查**（`user` 恒全量常驻、`assistant` 只见窗口切片）。只断言「存在 / 文本」的用例逐字保留。
- **滚动写入可观测（新增前提）**：jsdom 的 `Element` 无 `scrollTo`，虚拟器的写入侧是 `scrollElement?.scrollTo?.({...})` ⇒ 首帧贴底与换会话复位在测试里静默无效、无从断言。为让 AC-13 有机械判据，用例在**实例级**给视口装一个最小 `scrollTo` 桩（子组件的 layout effect 里安装——子先于父，故先于 Provider 的复位 effect 生效）：按参数写 `scrollTop` / `scrollLeft`，**不派发 `scroll` 事件**。桩只覆盖本文件的渲染实例，**不进 `test-setup.ts` 原型**（否则会改变其余套件里 `scrollTo` 从「静默无效」变成「真写位置」的既有语义）。
- 相应地，测试设计中「滚动位置类断言一律不写」的旧口径**不再适用**：首帧贴底 / 换会话复位 / 上滑不被拽走三处以可读的 `scrollTop` 断言（配 `data-pending-scroll` 生命周期侧证），其余滚动行为仍以结构面与手工验证为准。

### 关键决策与被拒备选

| # | 决策 | 理由 | 被拒备选（为何拒） |
|---|---|---|---|
| D1 | 退役分页，全量虚拟化（W2） | 一层窗口，prepend 语义归零，长会话首屏即全境且滚动条可用；分页本就只是「慢一点装满」 | **W1（保留分页 + 只虚拟化最新一页）**：两层窗口叠加，心智模型更绕，且触顶逻辑仍要另造；**W3（保留加载提示但一次装完）**：等于 W2 加一个永远显示「已装满」的死提示行 |
| D2 | 直接改 `components/ui/message-scroller.tsx`（保留 UI + 接 headless） | 六个 hook / 方法零外部消费者，UI 面（渐隐 / 滚动条 / HUD / `data-slot`）承载 36 处 `chat-bubble` 与既有 `data-slot` 结构断言、以及既有视觉，扔掉成本高；shim 已在本仓内 | **彻底退役自写视口**：`data-autoscrolling` / `data-pending-scroll` / 到底按钮 / 既有 `data-slot` 结构断言全部要重建，收益为零；**只留 Root/Viewport/Content + 自管滚动**：仍要自建虚拟窗口参数通道，比直接接 headless 多一层胶水 |
| D3 | 吸附 = 容器内占位块 + `position: sticky`；全部 user 消息无条件常驻 | 纯 CSS 吸顶 / 顶走，零 scroll 监听、零状态；用户确认「会话中用户消息通常 ≤ 10 条」，常驻量与虚拟器 `onChange` 完全解耦 | **候选 A（视口上方独立 overlay 层）**：需量吸附高度回填 padding，且 `pinned` 依赖滚动位置 ⇒ 每帧 scroll 状态切换（要节流）；**候选 B（虚拟项内 sticky）**：吸附项往往已滚出窗口，仍需 `rangeExtractor` 强行注入，且 sticky 在「绝对定位 + transform 父级」下易失灵；**挂载窗口 ±1 轮的优化**：为 ≤10 条的规模引入「可见范围」概念，与虚拟器耦合且测试要按窗口形态写死 |
| D4 | 纯函数落 `src/lib/agent-virtual.ts` | knip 可判 + `repo-archi-model` 元素面零漂移（`src/lib/**` 封闭排除） | **`components/agent/virtual-messages.ts`**（explore 建议）：同 change 必须追加 archi 元素并把 51/113、28/64 三处计数复述改写为 52/114、29/65——纯噪声；**留在 `agent-messages.tsx` 内 export**：knip 判 test-only 未用导出，门禁红 |
| D5 | 零测量回落 `estimateSize`（护栏进生产代码） | jsdom 无布局是常态；零高项会让 list 塌陷，回落是生产也想要的健壮性 | **仅测试内打 `offsetHeight`**：需逐元素打桩且与虚拟器内部测量路径耦合，读者看不到护栏，坏值在生产也会塌 |
| D6 | 吸附层零装饰、`scroll-fade-b` 原样留 Viewport | 用户明确「不要渐变」；`scroll-fade-b` 是**底边**渐隐且依赖自身滚动容器语义，与顶部吸附零交集 | **把 mask 挪到虚拟层容器**（explore 第 2 轮的误判）：会让 `animation-timeline: scroll(self y)` 失效退化为静态渐变 |
| D7 | 首帧与换会话贴底由 shim 按 `resetKey`（`conversationIdentity` 导出的会话身份）复位（layout 阶段一次 `scrollToIndex`） | 端锚定不覆盖首帧与换会话（`anchorTo:'end'` 只在选项变更且滚动元素在场时生效、首帧 `initialOffset` 缺省 0）；`resetKey` 在 `agent-messages.tsx` 内派生，`AgentMessagesProps` 形状不变、消费点零 diff | **`count > 0` 的一次性闩**（实现期曾按 design 事实偏差 7 采用）：只贴首次，换会话（A → B，count 恒 > 0）不复位 ⇒ 打开另一会话落在中部；**给 `AgentMessages` / `AgentSessionView` 加 `sessionId` 入参**：触及 `agent-session-view.tsx` / `hooks/**` 与 AC-12 零 diff 面 |

---

## 能力

### 新增能力

**无。** 本变更落在两条既有能力内：虚拟窗口 + 吸附是 `desktop-agent-chat-infra`「展示组件族（对话透镜）」的一个新关注点（ADDED requirement）；滚动承载措辞同能力内三条既有 requirement 需改写（滚动条款、`remaining` 两处、`useHistoryWindow` 一处）。`desktop-explore-page` 的「对话区呈现」含一句滚动条款（「沿用 shadcn `message-scroller` 组合」），改造后不再属实，同 change 改写。

另立「长会话列表」能力会让「会话呈现」与「会话呈现的滚动形态」分居两处，与既有 `desktop-agent-chat-infra` 的收敛方向相反。

### 修改的能力

- **desktop-agent-chat-infra**（`MODIFIED` × 3 + `ADDED` × 1）
  - `MODIFIED`「展示组件族(对话透镜)」：改末条滚动条款——「滚动 SHALL 沿用 shadcn `message-scroller` 组合」→「滚动 SHALL 由 `components/ui/message-scroller.tsx` 承载，内部以 `@tanstack/react-virtual` 渲染可见窗口切片、用户提问以 sticky 吸附层常驻，MUST NOT 以分页窗口限制入 DOM 的消息总数」；同 bullet 补一句依赖剪枝约束（primitives 退役后 `@shadcn/react` 无**可达**消费者 ⇒ 该依赖与其零消费者残件 `components/ui/questionnaire.tsx` SHALL 一并剪除，MUST NOT 新增 `ignoreDependencies` / `ignore` 豁免）。其余四条 bullet（`AgentSessionView` / `AgentMessages` / `AgentInput` / 工具卡注册表）、两段散文与 6 条 scenario **逐字保留**。
  - `MODIFIED`「轮内状态行与空内容轮可见性」：三处措辞改写——bullet「过滤单一来源」补「判据经 `src/lib/agent-virtual.ts` 的 `filterConversation` 单点实现、虚拟窗口项数基于过滤后列表」；scenario「失败轮空内容呈状态行」的「不参与分页 `remaining` 的错误计数」→「不参与虚拟列表项数与吸附层几何的错误计数」；scenario「空内容 completed 轮不进时间线」的「分页 `remaining` 不计入」→「虚拟列表 `count` 不计入」。其余 4 条 bullet 与 2 条 scenario **逐字保留**。
  - `MODIFIED`「web 搜索活动与引用来源呈现」：bullet「空内容轮可见性」的 `useHistoryWindow` 载体与「分页计数 / `remaining`」措辞改写为 `filterConversation` + 虚拟列表 `count`（含吸附层几何）；scenario「空内容 completed 轮不渲染」的 `remaining` 措辞同步改写；其余 6 条 bullet 与 4 条 scenario **逐字保留**。
  - `ADDED`「对话透镜虚拟窗口与用户消息顶部吸附」：虚拟窗口几何与全量装载、user 索引渲染空 + 高度回填（零测量回落）、sticky 吸附与顶走（零缓冲）、零装饰与 `scroll-fade-b` 留守、HUD / `data-slot` / `data-role` 钩子延续、三条结构前提、**首帧与换会话贴底（复位键通道，且复位面限在 `agent-messages.tsx` 与 shim 内）**。11 条 scenario。
  - Module Contract：新增 `src/lib/agent-virtual.ts` 函数表、`components/ui/message-scroller.tsx` 变化表、`agent-messages.tsx` 变化行，并登记零改动面。
- **desktop-explore-page**（`MODIFIED` × 1）
  - `MODIFIED`「对话区呈现」：仅改末句滚动条款（同能力口径），长段正文其余部分与 5 条 scenario **逐字保留**（「重开时会话仍在运行」等断言不受虚拟化影响）。
  - Module Contract：登记 `explore-detail-pane.tsx` 行「滚动与消息状态归组件」语义不变（组件内部形态变化不改变消费点契约）。

### 零改动的能力及理由

| 能力 | 理由 |
|---|---|
| **desktop-agent-execution** | 后端零改动；其「Agent 调试页」requirement 只约束「经 `AgentSessionView` / 对话透镜呈现、token 级流式、一轮一条气泡、辅助载体不进对话透镜」，不约束滚动形态与 DOM 规模（该 requirement 与全能力其余条款中的「分页」均指会话 / 记录查询面与 read 工具窗口，与本变更的消息列表无关） |
| **desktop-change-chat / desktop-archi-chat** | 两条能力的契约是「对话经 `AgentSessionView` 注入形态渲染、composer 经插槽注入、无自有转录查询 hook」；本变更不改组件入参与插槽形状，滚动形态对只读消费点透明 |
| **desktop-standard-components** | 149 行全文已核对，无 `message-scroller` 条款（该层只覆盖 `components/standard/`） |
| **repo-archi-model** | 纯函数落 `src/lib/**`（封闭排除面），`components/agent/` 文件集合不变、`components/ui/` 收敛为 1 元素不变 ⇒ 元素与关系集合、三处计数复述（51 / 113、28 / 64）零漂移 |
| **desktop-agent-chat-infra 的其余 requirement** | 「统一会话状态基建」「会话级统一渲染组件」「事件适配层」「ai-sdk 依赖收口」四条全部不涉滚动与 DOM 规模；适配层 / transport / `use-agent-session` 零改动 |
| **desktop-app-shell / desktop-ipc-type-bindings / desktop-workspace-store / desktop-workspace-config** | 零命令面、零生成物、零配置面、零 crate 布局变化（纯前端 TS 面） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src/lib/agent-virtual.ts`（新增） | ① `filterConversation(all)`：从 `useHistoryWindow` 抽出的过滤判据（system 角色过滤 + 空内容轮按 `metadata.record.status` 判定 `failed` / `stopped` 可见、`completed` / 无轮行不渲染），**语义零改**；② `userTurnSpans(messages, measurements, totalSize)`：吸附层占位块几何（`index` / `start` / `span`，`span` = 下一 user `start` − 本条 `start`，末轮 = `totalSize − start`；无 user 消息返回空数组）；③ `hasVisibleParts(message)`：可见部件判据（自 `agent-messages.tsx` 迁出，`filterConversation` 与 `ChatBubbleBase` 两个生产消费点共用，避免判据两份）；④ `conversationIdentity(messages)`：贴底复位键（首条携 `metadata.record.sessionId` 的会话 id；全体缺席返回 `null` = 未知，故 `null → 已知` 不触发复位）——供 `MessageScrollerProvider` 的 `resetKey` 判「换会话」 |
| `packages/desktop/src/components/ui/message-scroller.tsx`（改） | ① 退役 `@shadcn/react/message-scroller` primitives 依赖与其三个 hook / `scrollToMessage` 再导出（零外部消费者）；② 内部接 `useVirtualizer`：Provider 持有实例并向 Viewport / Content / Item / Button 供 context，新增入参 `count` / `estimateSize` / `getItemKey` / `renderItem` / `renderSticky` / `resetKey`（+ 可透传 `overscan` / `gap`）；③ 保留 `scroll-fade-b` / `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content` 与全部 `data-slot`；④ `data-autoscrolling` / `data-pending-scroll` 改由虚拟器 `isScrolling` / 首测完成标志驱动；⑤ 到底按钮 `data-active` 改由 `getDistanceFromEnd() > 阈值` 判定；⑥ 虚拟项 `measureElement` 加零测量回落 `estimateSize` 护栏；⑦ 新增吸附层槽位 `data-slot="message-scroller-sticky"`（`position:absolute` + `top`，禁 `transform`）；⑧ `Content` 的 `flex gap-6` 与调用点传入的 `p-3` 退役，行距 / 内边距改经虚拟器 `gap` / `paddingStart` / `paddingEnd`（`border-box` 下 padding 计入 `height`，会使末项越出容器并与吸附层跨度账目冲突）；⑨ **首帧与换会话贴底**：新增可选入参 `resetKey`（= 调用点传入的会话身份），在「消息集合首次非空 / 由空变非空」或「两侧 `resetKey` 均非 `null` 且不等」时于 layout 阶段调一次 `scrollToIndex(count − 1, { align: 'end' })`（`data-pending-scroll` 遮住这一跳）；任一侧为 `null`（首轮 `record` 落库 / 收口重放）与键不变（同会话追加）都不复位 |
| `packages/desktop/src/components/agent/agent-messages.tsx`（改） | ① 退役 `useHistoryWindow` / `useTopReachObserver` / `HistoryLoadStatus` / `MESSAGE_PAGE_SIZE` / `HISTORY_PRELOAD_MARGIN_PX` / `topSentinelRef` / `remaining` 与 `history-load-*` 分支，以及 Provider 的 `autoScroll` / `defaultScrollPosition` / `scrollPreviousItemPeek={64}` 与 `MessageScrollerItem` 的 `messageId` / `scrollAnchor` 入参；② 过滤走 `filterConversation`，列表走虚拟窗口：assistant 索引 `renderItem`、user 索引恒 `null`；`resetKey` = `conversationIdentity(过滤后列表)`（贴底复位键，经 `useMemo` 派生，不新增 props）；③ `renderSticky` 渲染吸附层（每条 user 消息一个占位块 + 内层 `position:sticky; top:0` 的 `ChatBubble`），`ResizeObserver` 实测 → `resizeItem`；④ 保留 `ChatBubble` / `PartView` / `TurnStatusLine` / `ToolPartView` / `citationsOfPart` 与其 `memo` 语义逐字不变；⑤ `loading` / `running` / `conversation-empty` 三态保留；⑥ `MessageScrollerContent` 的 `p-3` 入参退役（内边距改由虚拟器 `paddingStart` / `paddingEnd` 承担） |
| `packages/desktop/src/test-setup.ts`（改） | 增虚拟化打桩：`[data-slot="message-scroller-viewport"]` 的 `clientHeight` / `scrollHeight` 非零（按 `data-slot` 限定作用面），使虚拟窗口在 jsdom 下产出可见 range。**不在此处桩 `scrollTo`**——那会改变其余套件里「写入静默无效」的既有语义；滚动位置断言所需的 `scrollTo` 桩由 AC-13 的用例在实例级自带 |
| `packages/desktop/package.json`（改） | ① `dependencies` 增 `@tanstack/react-virtual`（`3.14.26`，peer 仅 react / react-dom）；② `dependencies` 删 `@shadcn/react`（primitives 退役后仓内无**可达**消费者，`knip` 报 unused dependency；MUST NOT 改以 `ignoreDependencies` 豁免）；③ `version` `0.6.31 → 0.6.32`（一次 patch；`src-tauri/Cargo.toml` 的 crate 版本行**不跟**）；`pnpm-lock.yaml` 随 `pnpm install` 机械更新 |
| `packages/desktop/src/components/ui/questionnaire.tsx`（**删除**） | 零消费者 / 零测试引用 / 零 archi 元素 / 零 API 契约登记的残件——`@shadcn/react` 的唯一另一引用面（全历史零 import 者，仅因 `knip.ignore` 才未被报为 unused file）。删依赖后该件无法解析，按 `desktop-app-shell`「knip 报出的未用导出 SHALL 删减」同批剪除；`desktop-agent-execution` 的「questionnaire 接线为二期」只约束**接线**，二期经 shadcn CLI 重新生成 |

### 测试文件

| 文件 | 现状 | 处理 |
|---|---|---|
| `src/lib/agent-virtual.test.ts`（新增） | — | `filterConversation` 语义等价（system 过滤 / 空内容 `failed` / `stopped` 可见 / `completed` 与无轮行剔除 / 有内容轮不受影响）；`userTurnSpans` 几何（多轮 / 单 user / 无 user / 末轮 = `totalSize − start` / start 单调） |
| `src/components/ui/message-scroller.test.tsx`（新增） | — | shim 结构契约：虚拟层只渲窗口切片、`data-slot` 五钩子在场、Viewport 类名与 `aria-label` 透传、`data-autoscrolling` 随滚动置位、到底按钮 `data-active` 与贴底互斥、零测量回落 `estimateSize`、吸附层容器无 `transform` 且携 `data-slot="message-scroller-sticky"`；**首帧贴底与换会话复位**（`0 → N` 与 A → B 各贴底一次、同会话追加不复位） |
| `src/components/agent/agent-messages.test.tsx` | 990 行 / 36 例（「历史分页」5 例 + 「大列表分页」1 例 + `FakeIntersectionObserver` ~30 行；`chat-bubble` 断言 30 处） | 删「历史分页」5 例与「大列表分页」1 例及 `FakeIntersectionObserver` / `triggerReachTop`；**条数断言改按 `data-role` 分查**；新增虚拟化 / 吸附 / 换会话复位（`rerender`）用例；空内容轮、气泡变体、状态行、citations 等其余用例的语义断言逐条保留 |
| `src/components/agent/agent-render-memo.test.tsx` | 渲染 memo 语义（历史消息引用稳定 ⇒ 子树跳过） | 断言方式不变，确认打桩后可见；补一条「虚拟窗口重渲不重建历史 assistant 项」 |
| `src/components/agent/agent-session-view.test.tsx` / `session-panel.test.tsx` / `index.test.tsx` | 会话入口与面板骨架（`block-text` 文本断言为主） | 只补虚拟化渲染前提（可见性依赖打桩），结构性断言零改 |
| `src/views/agent/agent-debug-view.test.tsx` | 3 处 `chat-bubble` 断言（其中 1 处已按 `data-role` 分查） | 未分查的条数断言改按 `data-role`；「收口后恰一条 assistant 气泡」类断言保留语义 |
| `src/views/changes/agent/change-chat-panel.test.tsx` / `src/views/explores/explore-detail-pane.test.tsx` | `chat-bubble` 数为 0 的空态断言（不受吸附层影响，但受虚拟窗口可见性影响） | 断言措辞不变，确认打桩后仍绿；如有「气泡数」正例则改按 `data-role` |
| `src/views/changes/flow/session-transcript-panel.test.tsx` / `detail-drawer.test.tsx` / `src/views/architecture/agent/archi-chat-panel.test.tsx` | `block-text` 文本序断言 | 逐字保留（只读消费点走同一透镜，文本序不受虚拟化影响） |

### 新增用例（覆盖新契约面）

| 位置 | 覆盖点 |
|---|---|
| `src/lib/agent-virtual.test.ts` | ① 过滤判据与改造前逐族等价（含「只含被消费孤儿搜索 result ⇒ 0 部件」的既有语义）；② spans 首尾闭合（首条 `start = 0`、末轮 `span = totalSize − start`）；③ 无 user 消息 ⇒ 空数组（不吸附、不炸）；③′ `conversationIdentity` 语义：同会话追加 / 流式增长返回同值、换会话（会话 id 变化）返回值变化、全体无 `record` 时返回 `null`（流式首轮未收口） |
| `src/components/ui/message-scroller.test.tsx` | ④ 100 项 count 只渲窗口切片（DOM 项数 ≪ count）；⑤ 吸附层槽位绝对定位且 `transform` 缺席；⑥ 零测量回落 `estimateSize`（`offsetHeight = 0` 时项的 start 仍连续、无零高项）；⑥′ **首帧贴底**：消息集合首次非空或由空变非空（`count` 0 → N）时视口在 layout 阶段被写到末端（用例内抬高实例级 `scrollHeight` 后断言 `scrollTop` 落到末端目标值，`data-pending-scroll` 在这一跳期间在场）；⑥″ **换会话复位**：`resetKey` 由 A 变为不同的 B（两侧均非 `null`，count 可相同）且 `scrollTop` 先被移到别处 ⇒ 复位回末端目标值；同会话追加（键不变、未贴底）与键单侧为 `null`（首轮 `record` 落库 / 收口重放）⇒ `scrollTop` 均不变（反向钉住「不拽回上滑用户」） |
| `src/components/agent/agent-messages.test.tsx` | ⑦ 1000 条消息（含空内容 completed 轮）⇒ DOM 中 assistant 气泡 ≪ 1000，user 气泡全量在场；⑧ 滚动使某轮提问完全出顶 ⇒ 该气泡 sticky 停在顶端且 DOM 仍只一份；⑨ 滚回顶部 ⇒ 无气泡吸顶；⑩ user 消息在虚拟层零节点（虚拟项集合内无 `data-role="user"`）；⑪ `rerender` 换会话（`metadata.record.sessionId` 由 A 变 B、条数不变）⇒ 视口重新贴底（`scrollTop` 回到末端目标值），证明复位键经透镜重渲染通道成立 |

### 不要修改

- **Rust 后端**：`packages/desktop/src-tauri/**` 全部零改动（本变更不涉命令面、信封、落库与引擎）。
- **生成物**：`src/types/generated/**` MUST NOT 重导出 / 手改（`git diff` 应零 diff）；`src/types/dto.ts` 自动跟随。
- **适配与会话基建**：`src/lib/agent-adapter.ts`、`src/lib/agent-transport.ts`、`src/hooks/use-agent-session*.ts`、`src/lib/parse.ts` 零改动（过滤判据的**语义**搬去纯函数，adapter 侧零改）。
- **组件族其余成员**：`components/agent/agent-blocks.tsx` / `agent-input.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx` / `agent-error.ts` 零改动（块渲染件、composer、卡片注册表、面板骨架、会话级入口入参与插槽形状、失败横幅判据均不变）。
- **`src/views/**` 源码**：消费点接线零改动（仍以 `useAgentSession` / `AgentSessionView` 为唯一入口）。
- **`src/components/ui/**` 的其余文件**：`bubble.tsx` / `message.tsx` / `trace.tsx` 等零改动（吸附气泡复用同一组件的 tinted 变体）；该目录内本变更只动 `message-scroller.tsx`、只删 `questionnaire.tsx`（见「实现文件」），其余件零 diff。
- **架构模型**：`.dev-team/architecture/**` 零改动（落点选择使元素 / 关系 / 计数零漂移；若实现期改判把纯函数放进 `components/agent/`，须同步模型——本变更显式禁止）。
- **规格基线**：`openspec/specs/**` 只在 change 目录出 delta（归档时合入），`openspec/config.json` 零改动。
- **`openspec/changes/agent-messages-virtual-list-pinning/explore.md`**：只读。
- **`packages/desktop/src-tauri/Cargo.toml`**：crate 版本行不随前端 patch 变动。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | **全量虚拟窗口**：1000 条过滤后消息挂载时，DOM 中的 assistant 气泡数 ≪ 1000（只渲可见切片 + overscan），content 高度 = `getTotalSize()`；滚动条比例由全量账目导出（一次装完，无「加载更早」） | `agent-messages.test.tsx` ⑦、`message-scroller.test.tsx` ④ |
| AC-2 | **分页面退役**：`useHistoryWindow` / `useTopReachObserver` / `HistoryLoadStatus` / `MESSAGE_PAGE_SIZE` / `HISTORY_PRELOAD_MARGIN_PX` / `remaining` / `history-load-hint` / `history-load-complete` / `topSentinelRef` 在 `src/` 零残留；`FakeIntersectionObserver` / `triggerReachTop` 在测试零残留 | `grep` 全仓零命中 + `vp check`（含 `knip` 零未用导出） |
| AC-3 | **user 消息常驻吸附**：全部 user 角色消息恒在 DOM 的吸附层（`data-slot="message-scroller-sticky"`，各恰一个节点），虚拟层对 user 索引零节点；滚动使某轮提问完全出顶时该气泡停于视口顶端（布局位置不变），DOM 仍只一份 | `agent-messages.test.tsx` ⑧⑩ + `message-scroller.test.tsx` ⑤ |
| AC-4 | **顶走与边界**：包含块高度 = 该轮跨度（`span`），下一轮提问到达其底边即把上一条顶走（零缓冲）；滚回会话顶部时无气泡吸顶、无第二份气泡；无 user 消息时不渲吸附层 | `agent-virtual.test.ts` ②③ + 手工滚动验证 |
| AC-5 | **吸附零 scroll 状态**：吸附层渲染零 range 依赖、零自建 `scroll` 监听、零 `pinned` state（判定纯 CSS）；吸附层以 `top` 定位且其祖先链无 `overflow` 裁剪类 | 源码审查（无 `addEventListener('scroll'` 于 `components/agent/`）+ `message-scroller.test.tsx` ⑤ |
| AC-6 | **高度回填与账目一致**：吸附层每条气泡 `ResizeObserver` 实测后 `resizeItem(index, h)`，`totalSize` = Σ 实测（含 user 高度），虚拟项无重叠、末项底边与 content 底边对齐；实测 ≤ 0 时回落 `estimateSize`（项 MUST NOT 零高） | `message-scroller.test.tsx` ⑥ + `agent-messages.test.tsx` 几何断言 |
| AC-7 | **视觉零装饰且既有 UI 保留**：吸附气泡与普通气泡同组件同 tinted 变体（仅多 `bg-card` 底衬），无渐变 / 阴影 / 提示层；Viewport 保留 `scroll-fade-b` / `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content` 与 `data-slot="message-scroller-viewport"` / `aria-label="对话消息"`；`data-autoscrolling` 随滚动置位、到底按钮 `data-active` 由距底距离判定 | `message-scroller.test.tsx` + `agent-messages.test.tsx`「容器组合」用例（类名 / 钩子断言保留） |
| AC-8 | **过滤判据语义零漂移**：`filterConversation` 与改造前 `useHistoryWindow` 的过滤结果逐族相等（system 过滤 / 空内容 `failed` / `stopped` 可见 / `completed` 与无轮行剔除 / 只含被消费孤儿搜索 result 的 0 部件轮不进透镜）；既有空内容轮用例的语义断言保留 | `agent-virtual.test.ts` ① + `agent-messages.test.tsx` 空内容轮组 |
| AC-9 | **测试可渲染**：jsdom 下虚拟窗口产出可见 range（`test-setup.ts` 的尺寸桩）、滚动写入可观测（AC-13 用例内实例级 `scrollTo` 桩），`agent-messages` / `agent-session-view` / `agent-render-memo` / `agent-debug-view` / `explore-detail-pane` / `change-chat-panel` / `session-transcript-panel` / `detail-drawer` / `archi-chat-panel` 全套绿；条数断言一律按 `data-role` 分查，文本 / 存在类断言逐字保留 | `pnpm -C packages/desktop run client:test` |
| AC-10 | **依赖与版本**：`@tanstack/react-virtual` 入 `dependencies`（`3.14.26`）；`@shadcn/react` 从 `dependencies` 移除、`components/ui/questionnaire.tsx` 删除（primitives 退役后仓内零**可达**消费者，`knip` 零 unused dependency；MUST NOT 新增 `ignoreDependencies` / `ignore` 条目豁免）；`package.json` `0.6.31 → 0.6.32`（一次 patch）；`src-tauri/Cargo.toml` crate 版本行不变 | `git diff packages/desktop/package.json` 审查 + `knip` 零报告 |
| AC-11 | **门禁**：`pnpm -C packages/desktop run check` 绿（`server:check` → `bindings:export` 零 diff → `api:check` 零 diff → `client:check` 含 `vp check` + `knip`）；`cargo nextest run --workspace` 全绿（零后端改动自然绿） | 门禁命令 + `git diff --stat` |
| AC-12 | **边界零 diff**：`src-tauri/**`、`src/types/generated/**`、`src/lib/agent-adapter.ts`、`src/lib/agent-transport.ts`、`src/hooks/**`、`components/agent/{agent-blocks,agent-input,tool-cards,session-panel,agent-session-view,agent-error}.{tsx,ts}`、`src/views/**`（源码）、`.dev-team/architecture/**`、`explore.md` 全部零 diff | `git diff --stat` 审查 |
| AC-13 | **首帧与换会话贴底**（复位机制限在 `agent-messages.tsx` 与 `components/ui/message-scroller.tsx` 之内）：消息集合首次非空或由空变 N（N > 0）时视口在 layout 阶段贴底一次；同一实例换会话（两侧 `conversationIdentity` 均非 `null` 且不等，count 可相同）再贴底一次；同一会话内追加消息、以及复位键单侧为 `null`（首轮 `record` 落库 / 收口重放）都不触发复位（上滑用户不被拽走）。复位判据经 `agent-virtual.ts` 的 `conversationIdentity` + shim 的 `resetKey` 通道 ⇒ `AgentMessagesProps` 形状不变，`use-agent-session*` / `agent-session-view.tsx` / `src/views/**` 零 diff | `message-scroller.test.tsx` ⑥′⑥″（用例实例级装 `scrollTo` 桩 + 抬高实例级 `scrollHeight` 使写入可观测）+ `agent-messages.test.tsx` ⑪（`rerender` 换会话，重渲染面同一断言）+ `git diff` 证明零 diff |

**AC ↔ spec requirement 对应**（均落在 `specs/desktop-agent-chat-infra/spec.md`，除注明者外）：

| AC | spec requirement / scenario |
|---|---|
| AC-1 / AC-3 / AC-4 / AC-7 | ADDED「对话透镜虚拟窗口与用户消息顶部吸附」的 scenario「千条会话只渲可见切片」「用户消息常驻且虚拟层零节点」「完全出顶即吸顶」「下一轮顶走上一条」「视觉一致且视口样式零改动」 |
| AC-2 | MODIFIED「展示组件族(对话透镜)」末条 bullet（分页窗口禁令 + 依赖剪枝约束） |
| AC-5 / AC-6 | ADDED 的 scenario「完全出顶即吸顶」「高度回填与账目一致」「零测量回落不塌陷」 |
| AC-13 | ADDED 的 scenario「首帧挂载即贴底」「切换会话复位贴底」（附「上滑不被拽回底部」反向钉住）+ Module Contract 的 `conversationIdentity` 行与 `resetKey` 入参 |
| AC-8 | MODIFIED「web 搜索活动与引用来源呈现」bullet「空内容轮可见性」+ MODIFIED「轮内状态行与空内容轮可见性」bullet「过滤单一来源」 |
| AC-9 / AC-10 / AC-11 / AC-12 | 工程验收面（无 requirement 对应；以门禁与 `git diff` 为判据；AC-10 的问卷剪枝依据落在 MODIFIED「展示组件族(对话透镜)」的依赖剪枝句） |
| 消费点契约（explore 页） | `specs/desktop-explore-page/spec.md` MODIFIED「对话区呈现」的滚动条款 + 其 Module Contract 行 |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | **jsdom 无布局导致虚拟窗口恒空**（`clientHeight = 0`、矩形全 0） | 不先打桩就改组件：`chat-bubble` / `block-text` 断言全部变 0 条，相关套件成批变红，无法判定改动对错 | 高 | 先落地 `test-setup.ts` 打桩（按 `[data-slot="message-scroller-viewport"]` 限定，避免污染其他套件）+ shim 的零测量回落 `estimateSize` 护栏；AC-9 以全套绿为机械判据 |
| R-2 | **sticky 静默失效**（吸附层父级有 `transform` / 祖先链出现 `overflow`） | 表现为「完全不吸附」或「吸住后永不顶走」，单测在 jsdom 下**测不出来**（无布局） | 中 | 三条结构前提写进 requirement 正文；`message-scroller.test.tsx` 断言吸附层 `style.transform` 缺席、祖先链无 overflow 类名；合并前手工滚动验证一次（AC-4 / AC-5） |
| R-3 | **条数断言语义漂移**（吸附层常驻 ⇒ `chat-bubble` 总数 ≠ 可见气泡数） | 36 处 `chat-bubble` 断言中凡断言条数者会给出「既非 10 也非全量」的错值，误改易丢真断言 | 高 | 只改「条数」类断言为按 `data-role` 分查；存在 / 文本 / 部件序类断言逐字保留；AC-9 反向钉住 |
| R-4 | **流式增长拽动视口 / 首帧跳变** | 长会话打开时先跳到底再回弹，或输出流式增长时把已上滑用户拽回底 | 中 | `anchorTo:'end'` + `followOnAppend` + `scrollEndThreshold` + `data-pending-scroll`（首测完成前 Viewport 隐形）；首帧 / 换会话的贴底由 shim 的会话身份复位显式发起（AC-13），`data-pending-scroll` 遮住该跳；virtualizer 的重测量只在「整项在折线以上」才改 scrollTop（`#1218` 防护已在源码） |
| R-5 | **高度账目漂移**（`estimateSize` 与实测差大 ⇒ 吸附层错位 / 滚动条比例失真） | 吸附气泡与虚拟项重叠或留洞；滚动条滑块长度与实际内容严重不符 | 中 | 首帧按角色分型估值（user 72 / assistant 240），挂载即 `ResizeObserver` 实测回填 `resizeItem`；AC-6 断言 `totalSize` = Σ 实测与末项底边对齐 |
| R-6 | **退役面误扩** | 顺手删掉 `[content-visibility:auto]` / `[contain-intrinsic-size]` / 块渲染件 / 轮状态行 / `data-slot` 钩子 ⇒ 单条超长 assistant 渲染成本回来了，或 36 处断言换锚点 | 中 | 「不要修改」清单 + explore §11 军民分界表逐行列保留项；AC-7 断言类名与钩子在场；AC-12 断言组件族其余文件零 diff |
| R-7 | **knip 判纯函数为未用导出** | 门禁红（`vp check` 非零），被迫把纯函数塞回组件内再被同样的规则判 red | 中 | 纯函数落 `src/lib/agent-virtual.ts` 并由 `agent-messages.tsx` **生产消费**（不是仅测试引用）；AC-2 含 `knip` 零未用导出 |
| R-8 | **架构模型漂移**（元素 / 关系 / 计数复述） | 若纯函数落 `components/agent/`，`repo-archi-model` 的「每文件一元素」要求同 change 追加元素并把 51/113、28/64 三处复述改值，否则双向覆盖核对出现差集 | 中 | 落点定为 `src/lib/**`（封闭排除面）⇒ 模型零 diff；「不要修改」显式禁改 `.dev-team/architecture/**`；AC-12 以零 diff 为判据 |
| R-9 | **`shadcn` shim 改造波及未察觉的消费面** | 若某处（含未来代码）依赖 `useMessageScroller*` / `scrollToMessage`，删除即编译错 | 低 | 已全域核对：`src/` 仅 `agent-messages.tsx` 消费组合件，三个 hook 与方法零外部消费者；`tsc`（`client:build`）在门禁内兜底 |
| R-10 | **极端长会话的常驻气泡量**（user 消息数即常驻数） | 100+ 轮的会话里 100+ 个气泡常驻（只 `ResizeObserver` 观测，不重排），滚动帧成本略升 | 低 | 用户已确认典型会话 user 消息 ≤ 10 条；测量是观测而非重排；若出现真实回归，后续 change 可加「可见 range ±1 轮」挂载窗口（本期显式不做，避免为小规模引入「可见范围」概念） |
| R-11 | **`scroll-fade-b` 落点被顺手挪走** | `animation-timeline: scroll(self y)` 失效，底边渐隐退化为静态渐变（无人报错、只是视觉退化） | 低 | requirement 正文写明「必须留在 Viewport」并给出机制原因；AC-7 断言 Viewport 携带该类名；`message-scroller.test.tsx` 结构断言兜底 |
| R-12 | **版本纪律**（前端改动忘记 bump / 误改 Cargo 版本行） | 发版流水线版本未递增，或 Rust crate 版本被前端 patch 带走 | 低 | AC-10 以 `0.6.31 → 0.6.32` 为机械判据，并显式断言 `src-tauri/Cargo.toml` 版本行不变 |
| R-13 | **几何基准被 CSS padding / gap 污染**（沿用 `Content` 的 `p-3` + `flex gap-6`） | sticky 的夹取边界与绝对定位包含块都是 padding box，且 `border-box` 下 padding 计入 `height`：末项越出容器、吸附层跨度与 `getTotalSize()` 账目对不上，表现为滚动到底有截断或吸附层错位 | 中 | 行距 / 内边距统一经虚拟器 `gap` / `paddingStart` / `paddingEnd` 表达，CSS 侧不再给内容容器加 padding / gap；AC-6 以「末项底边与内容容器底边对齐」为机械判据 |
| R-14 | **贴底复位误扩到不该动的面**（为「换会话贴底」给 `AgentMessages` / `AgentSessionView` 加会话入参，或散落 `count > 0` 一次性闩） | 一次性闩只贴首次 ⇒ 换会话（A → B，count 可相同）落在中部；加上游入参则 `agent-session-view.tsx` / `hooks/**` / `src/views/**` 全部破零 diff（AC-12） | 中 | 复位判据定为 `agent-virtual.ts` 的 `conversationIdentity` → shim 的 `resetKey`（纯前端派生通道，零上游入参）；AC-13 以 `message-scroller.test.tsx` ⑥′⑥″ + `git diff` 零 diff 为机械判据；requirement 正文写明「MUST NOT 引出 `use-agent-session*` / `agent-session-view` / `src/views/**` 改动」 |
