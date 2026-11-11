# 设计: agent-messages-virtual-list-pinning

> **变更**: agent-messages-virtual-list-pinning
> **日期**: 2026-11-12
> **基于**: `proposal.md`（提案一~五、变更范围、AC-1..AC-13、R-1..R-14）、`explore.md`（只读，三轮探索 + §22 决策台账）、两份 delta spec（`desktop-agent-chat-infra`：3 MODIFIED + 1 ADDED；`desktop-explore-page`：1 MODIFIED）
> **性质**: 纯前端呈现层重构——对话透镜从「自研分页窗口 + 全量 DOM」改为「全量虚拟窗口 + 用户提问 sticky 常驻吸附」。
>
> **本篇边界**（改动 5 个实现文件 + 删除 1 个死件，全部在前端 TS 面）：
> `packages/desktop/src/lib/agent-virtual.ts`（新增）、`packages/desktop/src/components/ui/message-scroller.tsx`、`packages/desktop/src/components/agent/agent-messages.tsx`、`packages/desktop/src/test-setup.ts`、`packages/desktop/package.json`；删除 `packages/desktop/src/components/ui/questionnaire.tsx`（`@shadcn/react` 剪枝的连带删除，见 D19）。
> **零 diff 面**：`packages/desktop/src-tauri/**`（含 `crates/**`）、`src/types/generated/**`、`src/types/dto.ts`、`src/lib/{agent-adapter,agent-transport,parse}.ts`、`src/hooks/**`、`src/components/agent/{agent-blocks,agent-input,tool-cards,session-panel,agent-session-view}.{tsx}` 与 `agent-error.ts`、`src/components/ui/**` 除 `message-scroller.tsx` 的改动与 `questionnaire.tsx` 的删除外全部、`src/views/**`（源码）、`openspec/specs/**`、`openspec/config.json`、`packages/desktop/knip.json`、`.dev-team/architecture/**`、本 change 的 `explore.md`。
> **零新增 IPC / 零新增 crate / 零新增配置键**；运行时依赖净变化 = 增 `@tanstack/react-virtual`、删 `@shadcn/react`。

---

## 提案与规格同步状态

proposal 的五条提案条款（W2 退役分页 / shim 军民分界 / 容器内占位块 + sticky / 纯函数落 `src/lib` / 打桩 + 零测量护栏）与 D1–D7 决策已定稿，两份 delta spec 已定稿（`desktop-agent-chat-infra` ADDED 一条 11 scenario 的 requirement，Module Contract 三张表；`desktop-explore-page` 改一句滚动条款）。本设计不重述论证，只做五件事：

1. 把 spec 已钉语义、未钉落点的接缝收敛到具体形态：**shim 的 context 契约与几何通道**（D3：虚拟器实例归 Provider，`measurements` / `totalSize` 经 `renderSticky(geometry)` 交给应用层，几何纯函数仍由应用层调）、**吸附层高度回填的归属**（D5：`ResizeObserver` 住 `MessageScrollerSticky`，避免为回填再开一个 hook 导出）、**零测量护栏的两条落地路径**（D6：虚拟项 `measureElement` 与吸附层 ResizeObserver 用同一句「≤0 回落 `estimateSize`」）、**贴底复位的判据通道**（D10：复位键由 `conversationIdentity` 派生 → shim 的可选入参 `resetKey`，不新增上游入参）。
2. 把 proposal / delta spec 中与工作树实测不符的七处按实测纠正并留痕（打桩的真实落点、`min-h-full` 漏列、水平 gutter 无归属、`data-autoscrolling` 是存在性变体、`hasVisibleParts` 的单点取舍、`-scroller` 既有 `overflow-hidden` 与「祖先链无 overflow」措辞的口径收窄、`anchorTo:'end'` 不负责首帧贴底）——见「事实偏差」七则。
3. 给出**几何账目**（§数据模型）：`content height = getTotalSize()`、`turnSpan(i) = start(i+1) − start(i)`、末轮 `totalSize − start`，以及五条不变量（同基准、容器高度恰为账目、回填后总量一致与末项对齐、零测量回落不塌陷、user 索引账目占位而虚拟层零节点）。
4. 逐条 AC 给落点（「验收标准回应」表），**不写测试策略**（用例编写与门禁执行由独立相位承接）。
5. 把三处「违反即静默失效」的结构前提写成可实现形态（D7：`top` 定位禁 `transform`、祖先链禁 `overflow` 裁剪类、`scroll-fade-b` 留守 Viewport），并补三条探索未覆盖但必须处理的细节：**吸附层占位块的可交互性**（D8，`pointer-events` 隔离）、**首帧与换会话贴底不在 `anchorTo:'end'` 的职责内**（D10 的 `useLayoutEffect` 按复位键发起一次 `scrollToIndex`）、**`measurementsCache` 是稠密数组**（源码核对，故吸附层可直接按索引取 `start`，无需额外推导）。

### 起点实况核实（工作树实测；路径前缀 `packages/desktop/`，锚点为改动前行号）

- **shim 现状**：`src/components/ui/message-scroller.tsx`（130 行）——`MessageScrollerProvider`（`:15-19`）直透 `@shadcn/react/message-scroller` 的 `Provider`；`MessageScroller`（`:21-35`，`data-slot="message-scroller"`，类名 `group/message-scroller relative flex size-full min-h-0 flex-col overflow-hidden`）；`MessageScrollerViewport`（`:37-51`，`data-slot="message-scroller-viewport"`，类名含 `scroll-fade-b scrollbar-thin scrollbar-gutter-stable overflow-y-auto overscroll-contain contain-content data-autoscrolling:* data-pending-scroll:invisible`）；`MessageScrollerContent`（`:53-64`，`data-slot="message-scroller-content"`，类名 `flex h-max min-h-full flex-col gap-6`）；`MessageScrollerItem`（`:66-82`，入参 `messageId` / `scrollAnchor`，类名含 `[contain-intrinsic-size:auto_10rem] [content-visibility:auto]`）；`MessageScrollerButton`（`:84-118`，`direction` / `render` / `variant` / `size`，`data-active` 全靠 shadcn 内部 store 决定）；导出（`:120-130`）含 `useMessageScroller` / `useMessageScrollerScrollable` / `useMessageScrollerVisibility` 三个再导出。
- **shim 上游依赖面**（`node_modules/@shadcn/react/dist/message-scroller/index.d.ts`）：`Provider` 入参 `autoScroll` / `defaultScrollPosition` / `scrollEdgeThreshold` / `scrollPreviousItemPeek` / `scrollMargin`；`Viewport` 入参含 `preserveScrollOnPrepend`；`Content` 入参含 `spacerClassName`；`Item` 入参 `messageId` / `scrollAnchor`；`Button` 入参 `behavior` / `direction` / `render`。三个 hook 与方法（`scrollToMessage` / `scrollToEnd` / `scrollToStart`）在 `src/` 全域**零外部消费者**（唯一消费点是 `agent-messages.tsx`，只用 `Provider` / `Root` / `Viewport` / `Content` / `Item` / `Button`）。
- **透镜现状**：`src/components/agent/agent-messages.tsx`（364 行）——`MESSAGE_PAGE_SIZE = 10`（`:31`）、`HISTORY_PRELOAD_MARGIN_PX = 300`（`:33`）；`ToolPartView`（`:36-49`）、`parseCitation` / `citationsOfPart`（`:52-70`）、`PartView`（`:77-82`）、`hasVisibleParts`（`:86-90`）、`turnStatusOf`（`:93-95`）、`noticeError`（`:99-107`）、`statusRawText`（`:110-117`）、`TurnStatusLine`（`:125-164`）、`ChatBubbleBase`（`:168-199`）、`ChatBubble = memo(…)`（`:204`）、`useTopReachObserver`（`:211-237`）、`HistoryLoadStatus`（`:240-249`）、`MessageContainer`（`:251-290`，含 `topSentinelRef` 与 `MessageScrollerContent className="p-3"`）、`useHistoryWindow`（`:293-327`，过滤判据在 `:301-307`）、`AgentMessages`（`:329-364`，Provider 传 `autoScroll defaultScrollPosition="end" scrollPreviousItemPeek={64}`，三态 `agent-messages-loading` / `conversation-empty` / `agent-messages-running`）。
- **测试基座现状**：`src/test-setup.ts`（38 行）已有四处桩——`Element.prototype.getAnimations` 空实现、`BASE_UI_ANIMATIONS_DISABLED`、`ResizeObserver` **空实现类**（`observe` / `unobserve` / `disconnect` 零回调，`:24-31`）、`PointerEvent` 桩，以及 `configure({ asyncUtilTimeout: 5000 })`。虚拟器侧的实测事实（`virtual-core@3.19.11` 源码 + 本仓 jsdom ≥26）：视口尺寸同步读 `clientHeight`（`index.js:6-9`，挂载即回调）⇒ 零回调的 `ResizeObserver` 桩不致命；`Element` 无 `scrollTo` ⇒ 滚动写入静默无效；`clientHeight` / `scrollHeight` / `getBoundingClientRect` 在 jsdom 下均为 0（已用 node 实测确认）⇒ 不打桩则 `outerSize = 0`、range 恒空（见事实偏差 1 / 7）。
- **门禁面**：`vite.config.ts` —— `NO_STATIC_CHECK = ['src-tauri/**','src/types/generated/**','src/components/ui/**']`（fmt / lint 双忽略）；`lint.rules` 含 `max-lines-per-function`（tsx 80 行）、`typescript/no-unsafe-type-assertion`、`typescript/no-non-null-assertion`（测试文件豁免）。`knip.json` —— `project` 排除 `src/**/*.test.*`；`ignore` 含 `src/components/ui/**`（**shim 内导出不受死代码门禁约束**，`src/lib/**` 不在 ignore 面内）。
- **archi 现状**：`App.Frontend.Components.Agent.AgentMessages`（`models/06-frontend.toml:117`，`path = ["./packages/desktop/src/components/agent/agent-messages.tsx"]`，出边 `AgentBlocks` / `ToolCards` / `Ui`）；`06-frontend.toml` 头部注释写明「元素 = 页壳 1 + 视图 9 + 视图内子域 3 + 共享组件文件 10 + 原语层 1」「关系只取 import 证据……**指向 hooks / lib / types 的 import 不建边**」——即 `src/lib/**` 是封闭排除面，新增文件在此为零漂移。
- **版本现状**：`package.json:3` `version = "0.6.31"`；`dependencies` 无 `@tanstack/*`；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不跟。

### 事实偏差（已在本文档裁定）

1. **打桩的作用面比 proposal 描述的更窄、但落点不同**：proposal 写「给 `[data-slot="message-scroller-viewport"]` 非零 `clientHeight` / `scrollHeight`」。核对 `virtual-core@3.19.11` 源码后确认：①视口尺寸由 `observeElementRect` 的 `getRect(element)` 同步读 **`clientWidth` / `clientHeight`**（`index.js:6-9`）——它在挂载时立即回调一次，**不依赖 `ResizeObserver`**；`ResizeObserver` 只负责后续尺寸变化（`index.js:57-63`），本仓 `test-setup.ts` 的零回调桩因此**不致命**；②`getBoundingClientRect` 只在**项**测量路径（`measureElement` 的 `offsetHeight` 回落侧）出现，而本设计用零测量护栏在 `≤0` 时改走 `estimateSize`，故**不必**桩 `getBoundingClientRect`；③`scrollHeight` 只影响 `_elementMaxScrollOffset`（`index.js:1168-1179`，即 `getDistanceFromEnd` / `isAtEnd` 的分母）。裁定：打桩只做**两件**——`clientHeight` 与 `scrollHeight`（均按 `[data-slot="message-scroller-viewport"]` 收窄、值 600）在 `Element.prototype` 上定义 getter，未命中回落原生 getter。`clientHeight = 600` 使 range 非空；`scrollHeight = 600` 使 `maxScroll = 0` ⇒ 全局语义即「内容不足一屏、贴底、到底按钮不出现」，需要验证「到底按钮互斥」的用例在自身内局部抬高 `scrollHeight`（测试相位处理）。
2. **`MessageScrollerContent` 还带 `min-h-full`**：proposal 只点名 `flex gap-6` 与调用点的 `p-3`。`min-h-full`（`min-height:100%`）会让容器高度 ≥ 视口高度，而 AC-6 的机械判据是「末项底边与内容容器底边对齐」——两者叠加失效。裁定：`h-max` / `min-h-full` / `flex gap-6` **一并退役**，容器高度只由内联 `height: getTotalSize()` 表达。
3. **水平 gutter 无归属**：`paddingStart` / `paddingEnd` 只管**纵向**；`p-3` 的 12px 横向分量若不另置，气泡会贴视口左右边（视觉回归）。且绝对定位项的包含块是**padding box**，若把 `px-3` 留在 `Content` 上，`left:0;width:100%` 的项反而会横向溢出 24px。裁定：横向 gutter 落到**行级容器**——`MessageScrollerItem` 与 `MessageScrollerSticky` 内层各带 `px-3`，空态块由调用点带 `px-3`；`Content` 自身零 padding。
4. **`data-autoscrolling` / `data-pending-scroll` 是存在性变体**：`message-scroller.tsx:45` 的类名是 `data-autoscrolling:scrollbar-thumb-transparent`（Tailwind v4 语义 = `&[data-autoscrolling]`，与属性值无关）。若恒输出 `data-autoscrolling="false"`，滚动条透明样式将**常亮**。裁定：两个属性必须**条件输出**（真时输出 `"true"`，假时 `undefined` 不落属性）；`MessageScrollerButton` 走 `data-[active=true]` / `data-[active=false]` 显式值形态，可恒输出。
5. **`hasVisibleParts` 的单点取舍**：现状该判据住在 `agent-messages.tsx:86-90`，同时服务两处——①过滤（搬去 `filterConversation`）与②「`parts` 为空时以状态行取代部件列表」（留在 `ChatBubbleBase`）。只搬不导出会留下第二份三行判据（违 spec「过滤单一来源」的精神）；导出则使 spec Module Contract 的 `agent-virtual.ts` 函数表从两行变三行。裁定：**导出**，由 `filterConversation` 与 `ChatBubbleBase` 共用（两处都是生产消费，`knip` 无风险），并在待决问题 1 登记「归档合入基线时该表需补第三行」。
6. **「吸附层祖先链无 `overflow:hidden/auto`」的字面口径需要一处收窄**：delta spec 的 ADDED requirement 写「吸附层与其祖先链 MUST NOT 出现 `overflow-hidden` / `overflow-auto` 一类裁剪」。实测祖先链为 `-content`（`relative`，无 overflow）→ `-viewport`（`overflow-y-auto` + `contain-content`）→ `-scroller`（`overflow-hidden`，`:29` 既有类名）。按 CSS 语义，`position: sticky` 的夹取基准是**最近**的滚动容器 —— 即 `-viewport`；`-scroller` 位于其**之上**，不参与夹取，且它承担 `rounded-lg` 一族的裁剪职能，摘掉属纯视觉回归。裁定：**`-scroller` 的 `overflow-hidden` 保持不动**（零 diff），把 spec 该句理解为「吸附层与被夹取范围内的祖先链」；若口径要求逐字执行，则须同批改这一既有类名（视觉面需重验）。登记待决问题 7，供归档合入时收紧措辞。
7. **`anchorTo: 'end'` 不负责「首帧贴底」**：`setOptions` 里的端锚定逻辑只在 `prevOptions !== undefined`（即已有上一轮选项）时才生效（`index.js:397`），且 `initialOffset` 缺省 `0`（`index.js:361`）。故首次挂载 / 换会话（`0 → N`）时视口停在**顶部**，`followOnAppend` 也不会触发（它要求 `isAtEnd` 为真）。裁定：这两种情形的贴底由 `MessageScrollerProvider` 的一个 `useLayoutEffect` 显式发起（`scrollToIndex(count − 1, { align: 'end' })`），配合 `data-pending-scroll` 的首帧隐形遮盖这一跳（D11）。**复位判据不得是一次性闩**（「`count > 0` 且尚未贴过」只覆盖首帧，换会话 A → B 时 `count` 恒 > 0 ⇒ 落在中部）：判据改为「①消息集合首次非空或由空变非空；②两侧 `resetKey` 均非 `null` 且不等」（D10）。另：`jsdom` 的 `Element` **无 `scrollTo`**（实测），而虚拟器的写入侧是 `scrollElement?.scrollTo?.({...})`（`index.js:150-158`）⇒ 该跳在默认测试环境下静默无效、**不抛错**；需要断言写入的用例在**实例级**装 `scrollTo` 桩（子组件的 layout effect 先于父 Provider 的复位 effect 生效，故桩可先落地；桩只覆盖该渲染实例、不进 `test-setup.ts` 原型）。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 窗口层数 | **一层**：`count = filterConversation(messages).length`，虚拟器覆盖过滤后全量；`useHistoryWindow` 的 offset / page / remaining 全部退役 | proposal D1（W2）；分页本就只是「慢一点装满」，两层窗口只会把「触顶装载」逻辑搬到 scroll 阈值上重写一遍 |
| D2 | shim 的改造边界 | 保留 DOM 外壳与全部 UI 面（`data-slot` 五钩子、`scroll-fade-b` / `scrollbar-*` / `overscroll-contain` / `contain-content`、到底按钮、`[content-visibility:auto]` + `[contain-intrinsic-size]`），**内部换引擎**：`@shadcn/react/message-scroller` primitives 与三个 hook / `scrollToMessage` 再导出全部退役 | proposal D2；UI 面承载 36 处 `chat-bubble` 与 `data-slot` 结构断言，扔掉的成本是「重建一套等价物」 |
| D3 | 虚拟器实例的归属与几何通道 | 实例归 `MessageScrollerProvider`（他同时持有 `viewportRef` 并经 context 交给 `Viewport`）；`Content` 在渲染期读 `virtualizer.getTotalSize()` / `getVirtualItems()` / `measurementsCache`，并以 `renderSticky(geometry)`（`geometry = { measurements, totalSize }`）把**几何**交给应用层；「哪些索引是 user 消息、span 怎么算」仍归应用层的 `userTurnSpans(messages, measurements, totalSize)` | spec Module Contract 的纯函数签名是 `(messages, measurements, totalSize)` —— `messages` 是应用层知识，几何是 shim 知识；render prop 只传几何，两者不互相越界。且 shim 无需知道 `AgentUIMessage` 形状（`count` / `estimateSize` / `getItemKey` / `renderItem` 全部泛型无关） |
| D4 | 元素划分 | shim 上下文不外泄为 hook：只有 `renderItem` / `renderSticky` 两个 render prop 与 `MessageScrollerSticky` 一个装配件 | 少一个跨层 hook 就少一处「谁能调、何时调」的隐式契约；吸附层回填也不需要应用层碰实例（D5） |
| D5 | 高度回填的归属 | `ResizeObserver` 住 `MessageScrollerSticky` 自己：它知道 `index`，也经 context 拿得到 `resizeItem`；测量对象是内层 sticky 容器的 `borderBoxSize`（吸顶时量到的仍是**布局盒**高度） | 让应用层不必持虚拟器实例，`renderSticky` 退化为纯渲染；`borderBoxSize` 不受 sticky 视觉偏移影响（explore §9 已核实） |
| D6 | 零测量护栏 | 虚拟项：`measureElement` 包装器先量 `getBoundingClientRect().height`，`≤ 0` 时 `resizeItem(index, estimateSize(index))` 并跳过原生测量；吸附层：ResizeObserver 回报 `≤ 0` 时同样 `resizeItem(index, estimateSize(index))` | 护栏进生产代码（proposal D5）：jsdom 无布局是常态，零高项会让 list 塌陷；同时用它把测试几何变成**可确定性推导**（估算值即几何） |
| D7 | 三条结构前提（违反即静默失效） | ①吸附层容器只用 `top`，**禁 `transform`**；②吸附层祖先链上**最近的滚动容器**必须是 `MessageScrollerViewport`（`-content` 无 overflow 类、`-viewport` 的 `contain-content` = `contain: layout paint` 不创建包含块；`-scroller` 的 `overflow-hidden` 在 `-viewport` 之上不参与夹取基准，**保持不动**以免破 `rounded-lg` 裁剪）；③`scroll-fade-b` 原样留 `MessageScrollerViewport` | explore §23；①transform 会另建包含块 ⇒「吸住后永不顶走」；②另立滚动容器（如给当前层加 `overflow`）⇒「完全不吸附」；③`animation-timeline: scroll(self y)` 要求承载元素自身是滚动容器 |
| D8 | 吸附层的可交互性 | 占位块容器 `pointer-events: none`，内层 sticky 容器 `pointer-events: auto` | 占位块高度 = 整轮跨度，会盖住 assistant 正文与工具卡；不隔离则正文不可选中、工具卡不可点。探索未覆盖，属实现期必然撞上的细节 |
| D9 | 视觉 | 吸附气泡与普通气泡同一 `ChatBubble`（`Bubble variant="tinted"`，user 靠右）；仅在内层 sticky 容器加不透明 `bg-card` 底衬；零渐变 / 零阴影 / 零「上方还有内容」提示 | proposal 三·零装饰 + D6；`bg-card` 只为挡住气泡圆角边缘透出的下层正文 |
| D10 | 贴底跟随、首帧/换会话复位与到底按钮 | ①跟随：`anchorTo: 'end'` + `followOnAppend: true` + `scrollEndThreshold`（24px）；②**首帧与换会话复位**由 Provider 的 `useLayoutEffect` 发起（`scrollToIndex(count − 1, { align: 'end' })`）—— `anchorTo:'end'` 只在选项变更且滚动元素已挂载时生效、首帧 `initialOffset` 缺省 0，本身不负责这两者（事实偏差 7）。复位条件**恰为二者之一**：①消息集合首次非空或由空变非空；②两侧 `resetKey` 均非 `null` 且不相等。比对基准是 `useRef` 记住的上一轮 `resetKey`（`undefined` 初值 = 尚未有过键，与 `null`（未知）区分）；任一侧 `null`（首轮 `record` 落库 / 收口重放）与键不变（同会话追加）都不复位。**MUST NOT 用一次性闩**（只贴首次 ⇒ 换会话漏掉）；③到底按钮：`data-active = getDistanceFromEnd() > scrollEndThreshold`，点击同 ② | spec「贴底跟随」「首帧与换会话贴底」两条条款；按钮语义等价于原「可滚动且未贴底」（贴底时距离 0；内容不足一屏时距离亦 0，按钮自动不出现）；②③共用一个 `scrollToEnd()` 闭包；`resetKey` 只参与复位时机判定，**不进虚拟器选项**（进选项会让每次会话身份变化触发 `setOptions` 端锚定重算，与复位语义重复且时序不可控） |
| D11 | HUD 两属性与首帧 | `data-autoscrolling` ← `virtualizer.isScrolling`；`data-pending-scroll` ← 「首测完成标志」：`useState(true)`，在 D10 ② 的同一个 `useLayoutEffect` 里于首次运行后置 false；两者**条件输出** | 保留既有视觉语义（滚动时滚动条透明、首帧隐形防跳——它正是遮住 D10 ② 那一跳的手段）；条件输出是事实偏差 4 的必需形态 |
| D12 | 纵向几何的单一来源 | `gap` / `paddingStart` / `paddingEnd` 全走虚拟器选项（24 / 12 / 12），CSS 侧不给 `Content` 任何 padding / gap；横向 gutter 走行级 `px-3` | R-13：sticky 的夹取边界与绝对定位包含块都是 padding box，`border-box` 下 padding 又计入 `height` ⇒ 两处叠加会让末项越出容器、跨度与账目对不上 |
| D13 | 到底按钮收窄 | `direction` 入参退役（零外部消费者用 `start`），`data-direction="end"` 静态保留以延续 CSS 选择器与结构断言 | 简化；`start` 方向的滚动入口在本仓无消费面，虚拟化后其语义（`scrollTop > 阈值`）也无需求驱动 |
| D14 | 估算高度 | `estimateSize` 按角色分型：user 72px、assistant 240px（常量住 `agent-messages.tsx`），`overscan` 8 | spec 钉住「按角色分型估值」；不追求准——首次测量即校正，重要的是滚动条比例与首帧贴底 |
| D15 | 退役面 | 删 `useHistoryWindow` / `useTopReachObserver` / `HistoryLoadStatus` / `MESSAGE_PAGE_SIZE` / `HISTORY_PRELOAD_MARGIN_PX` / `topSentinelRef` / `remaining` / `history-load-*` / Provider 的 `autoScroll`·`defaultScrollPosition`·`scrollPreviousItemPeek` / `Item` 的 `messageId`·`scrollAnchor` / shim 的三个 hook 与 `scrollToMessage` 再导出 / `@shadcn/react` 依赖与其零消费者残件 `components/ui/questionnaire.tsx` | AC-2 / AC-10；**不保留兼容层**（不留 `@deprecated` 转发、不留双路分支），否则 `knip` 会把死代码判活、`grep` 判据失效；依赖剪枝 MUST NOT 改用 `ignoreDependencies` / `ignore` 豁免（见 D19） |
| D16 | 版本与依赖 | `package.json`：`@tanstack/react-virtual: "3.14.26"`（精确锁版）入 `dependencies`；`version` `0.6.31 → 0.6.32`（一次 patch） | AC-10；精确锁版与 `@tauri-apps/*` 的既有钉法定式一致（虚拟器选项面是本变更的机械前提，浮动版本会引入不可控差异）；`src-tauri/Cargo.toml` crate 版本行**不跟** |
| D17 | 编译闭合批 | 阶段二（`agent-virtual.ts`）→ 阶段三（shim）→ 阶段四（透镜）构成**同一条编译闭合批**：shim 的 `Provider` 入参与 `Item` 形状一变，`agent-messages.tsx` 立即断编译。唯一闭合点设在阶段四末 | 与前置 change 同款纪律；本相位只做源码级自查，`vp check` / `knip` / `vp test` 由桌面端静态检查与测试相位执行 |
| D18 | 明确不做 | ①不做吸附层挂载窗口（全部 user 常驻）；②不加顶走缓冲 gutter；③不做「回到该提问」跳转入口；④不动 `agent-error.ts` / `agent-blocks.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx` / `src/views/**`；⑤不改适配层 / transport / `use-agent-session*` / Rust 侧；⑥不新增 archi 元素（纯函数落 `src/lib/**` 封闭排除面） | 逐条对应 proposal「不要修改」与 explore §18 / §21；越界项一律拒 |
| D19 | `@shadcn/react` 剪枝的依据与禁区 | primitives 退役后该包在仓内零**可达**消费者：删 `components/ui/questionnaire.tsx`（全历史零 import / 零测试引用 / 零 archi 元素 / 零 API 契约登记，仅因 `knip.ignore` 含 `src/components/ui/**` 才未被报为 unused file）并从 `dependencies` 移除 `@shadcn/react` | `desktop-app-shell`「knip 报出的未用导出 SHALL 删减」；MUST NOT 改用 `ignoreDependencies` / `ignore` 豁免（`desktop-archi-api` 明文禁「关判据而非满足判据」，且 `knip.json` 在 `openspec/config.json` 的 `write_protection` 内正是拦此捷径）。`desktop-agent-execution` 的「questionnaire 接线为二期」只约束**接线**（AskUserQuestion 现由 `tool-cards.tsx` 静态卡承载），二期经 shadcn CLI 重新生成即可 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 可见性 / 吸附几何 / 会话身份纯函数（**新增**） | `hasVisibleParts`（可见部件判据）、`filterConversation`（对话透镜可见性单点）、`userTurnSpans`（吸附层占位块几何）、`conversationIdentity`（贴底复位键） | `packages/desktop/src/lib/agent-virtual.ts` | `@/lib/agent-adapter`（类型 `AgentUIMessage`）、`@/types/dto`（`TurnSummary`）——均只 import 类型 | TS 纯函数；零 IO、零 React、零 DOM |
| 滚动承载 shim（**重写体内**） | `Provider` 持有 `useVirtualizer` 与 `viewportRef`、按 `resetKey` 发起贴底复位；`Viewport` 挂 ref 并保留全部 UI 类名 / `data-slot` / HUD 属性；`Content` 高度 = `getTotalSize()`、渲可见切片与吸附层条目（**按索引交织**）、调 `renderSticky`；`Item` 绝对定位 + `translateY(start)` + 零测量回落；`Sticky` 占位块 + 内层 `position:sticky; top:0` + ResizeObserver 回填；`Button` 距底判定 + `scrollToEnd()` | `packages/desktop/src/components/ui/message-scroller.tsx` | `@tanstack/react-virtual`（`useVirtualizer`）、`cn`、`lucide-react`（`ArrowDownIcon`）、`@/components/ui/button` | React context + `@tanstack/react-virtual` v3（`anchorTo:'end'` / `followOnAppend` / `scrollEndThreshold` / `getItemKey` / `gap` / `paddingStart` / `paddingEnd` / `resizeItem` / `getDistanceFromEnd` / `scrollToIndex` / `measureElement`） |
| 对话透镜（**重写滚动接线**） | 过滤走 `filterConversation`；`count` / `estimateSize`（按角色）/ `getItemKey`（`id`）/ `renderItem`（assistant → `ChatBubble`，user → `null`）/ `renderSticky`（`userTurnSpans` → `MessageScrollerStickyEntry[]`）/ `resetKey`（`conversationIdentity(过滤后列表)`，`useMemo` 派生、不新增 props）；三态与全部块渲染件逐字不变 | `packages/desktop/src/components/agent/agent-messages.tsx` | `@/lib/agent-virtual`、`@/components/ui/message-scroller`、`./agent-blocks`、`./tool-cards`、`@/components/ui/{bubble,message,spinner,trace}` | React（`useMemo` / `useCallback` + `memo` 语义保持） |
| 测试渲染基座（**加桩**） | 让 jsdom 下的滚动视口有非零尺寸，使虚拟窗口产出可见 range；作用面严格限定在 `data-slot="message-scroller-viewport"`；**不桩 `scrollTo`**（那会改变其余套件里「写入静默无效」的既有语义） | `packages/desktop/src/test-setup.ts` | 无（原型级打桩） | jsdom 原型打桩（`Element.prototype` 上 `clientHeight` / `scrollHeight` 两个 getter，未命中回落原生 getter） |
| 依赖与版本行（**修改**） | 加 `@tanstack/react-virtual`；删 `@shadcn/react`；`version` `0.6.31 → 0.6.32` | `packages/desktop/package.json` | — | — |

<!-- 无新增 crate / 无新增后端模块 / 无新增 archi 元素（纯函数落 src/lib/** 封闭排除面，见 D4 与 archi 现状核实）。 -->

---

## 变更清单

> 路径口径：仓库根相对全路径。测试文件与用例（proposal「测试文件」/「新增用例」两张表）归测试相位，**不在本清单**。

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/lib/agent-virtual.ts` | 四个纯函数 + 一个几何类型（见「公共函数 / API」与「类型定义」）。落 `src/lib/**` 的两条硬理由：①`knip` 的 `project` 排除测试文件，组件内为测试开 `export` 会被判 unused；②`src/lib/**` 是 archi 模型的封闭排除面（`06-frontend.toml` 头部「指向 hooks / lib / types 的 import 不建边」），元素 / 关系 / 计数零漂移 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src/components/ui/message-scroller.tsx` | ①删 `@shadcn/react/message-scroller` 的全部 import（primitives 不再被引用）与三个 hook / `scrollToMessage` 再导出（`:120-130`）；②新增 context（值：`virtualizer` / `viewportRef` / `pendingScroll` / `scrollToEnd` / `renderItem` / `renderSticky`）；③`MessageScrollerProvider`（`:15-19`）重写为 `useVirtualizer` 持有者（入参见「公共函数 / API」）+ 按 `resetKey` 的贴底复位 `useLayoutEffect`（D10 ②），并渲染 context provider；④`MessageScroller`（`:21-35`）改为纯 `div`（类名逐字保留，`data-slot="message-scroller"`）；⑤`MessageScrollerViewport`（`:37-51`）改为纯 `div` + 挂 `viewportRef`，**类名清单逐字保留**（含 `scroll-fade-b` / `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content` / `data-autoscrolling:*` / `data-pending-scroll:invisible`），新增条件输出 `data-autoscrolling` / `data-pending-scroll`（真时 `"true"`、假时不落属性）；⑥`MessageScrollerContent`（`:53-64`）改为 `position: relative` + 内联 `height: getTotalSize()`，类名删 `flex h-max min-h-full flex-col gap-6`（D12 / 事实偏差 2），渲 `children` + 私有子件 `MessageScrollerSlice`（虚拟切片与吸附条目按索引交织）；⑦`MessageScrollerItem`（`:66-82`）入参改 `{ index, start, children, className }`，`messageId` / `scrollAnchor` 退役，类名保留 `[contain-intrinsic-size:auto_10rem] [content-visibility:auto]` 并新增 `absolute top-0 left-0 w-full px-3` + 内联 `translateY(start)`，挂 `data-index={index}` 与零测量回落的 `measureElement`；⑧新增 `MessageScrollerSticky`（占位块 + sticky 内层 + ResizeObserver 回填），吸附层条目按 **`index` 与虚拟切片交织**挂载（DOM 顺序即会话顺序，`block-text` 文本序与 `data-role` 序列断言依赖它），占位块带 `z-10`（绝对定位同层遮挡）；⑨`MessageScrollerButton`（`:84-118`）`direction` 入参退役（D13），`data-active` 改距底判定，`onClick` 改走 context 的 `scrollToEnd()`；⑩新增可选入参 `resetKey?: string \| null` 并据「首次非空 / 由空变非空」与「两侧均非 `null` 且不等」在 layout 阶段复位贴底（D10），`pendingScroll` 遮住该跳；⑪`Viewport` 补 `role="region"`（可被调用点覆盖）与 `tabIndex={0}`、`Button` 补 `type="button"` 与未激活时 `tabIndex={-1}`（对齐原 primitives 的可达性语义） | AC-1 / AC-3 / AC-5 / AC-6 / AC-7 / AC-11 / AC-13；R-1 / R-2 / R-4 / R-11 / R-13 / R-14 的落点全在本文件 |
| `packages/desktop/src/components/agent/agent-messages.tsx` | ①删 `useHistoryWindow`（`:293-327`）/ `useTopReachObserver`（`:211-237`）/ `HistoryLoadStatus`（`:240-249`）/ `MESSAGE_PAGE_SIZE`（`:31`）/ `HISTORY_PRELOAD_MARGIN_PX`（`:33`）/ `topSentinelRef` / `remaining` / `history-load-*` / 本地 `hasVisibleParts`（`:86-90`，改从 `@/lib/agent-virtual` 导入）/ 未再使用的 `useEffect`·`useRef`·`useState`·`useCallback` 导入；②新增模块常量 `USER_MESSAGE_ESTIMATE_PX = 72` / `ASSISTANT_MESSAGE_ESTIMATE_PX = 240` / `MESSAGE_GAP_PX = 24` / `MESSAGE_PADDING_PX = 12` / `VIRTUAL_OVERSCAN = 8`；③`MessageContainer`（`:251-290`）重写：`MessageScrollerProvider` 承接虚拟窗口入参（`renderItem` / `renderSticky` 用 `useCallback` 稳定引用；`resetKey` 由 `AgentMessages` 侧的 `useMemo` 派生后透传），`MessageScrollerContent` 不再传 `className="p-3"`，空态块（`conversation-empty`）以 `px-3` 作 Content 子节点；④`AgentMessages`（`:329-364`）删 Provider 的 `autoScroll` / `defaultScrollPosition` / `scrollPreviousItemPeek={64}`，新增 `const resetKey = useMemo(() => conversationIdentity(conversation), [conversation])` 并透传（`AgentMessagesProps` 形状不变），`loading` / `running` 三态与 `data-testid` 逐字保留；⑤`ChatBubbleBase`（`:168-199`）除 `hasVisibleParts` 改为导入外**逐字不变**（`TurnStatusLine` / `PartView` / `ToolPartView` / `citationsOfPart` / `parseCitation` / `noticeError` / `statusRawText` / `ChatBubble = memo(…)` 全不动） | AC-1 / AC-2 / AC-3 / AC-4 / AC-8 / AC-9 / AC-13；`AgentMessagesProps`（三字段）零改动 ⇒ 全部消费点（`agent-session-view.tsx` / `src/views/**`）零 diff |
| `packages/desktop/src/test-setup.ts` | 增「虚拟滚动视口尺寸桩」：在 `Element.prototype` 上为 `clientHeight` 与 `scrollHeight` 各定义 getter，按 `data-slot="message-scroller-viewport"` 命中时返回 600、否则回落原生描述符（`Object.getOwnPropertyDescriptor` 先取一次）。**不桩 `getBoundingClientRect`、不桩 `clientWidth`、不动既有 `ResizeObserver` 桩**。注释写明三件事：①为何只需两个 getter（视口尺寸由 `observeElementRect` 的 `getRect` 同步读 `clientHeight`，挂载即回调；`scrollHeight` 只进 `getDistanceFromEnd` 的分母——事实偏差 1）；②为何用原型级 getter（实例级 `Object.defineProperty` 打桩优先于原型，互不干扰；`600 / 600` ⇒ `maxScroll = 0` ⇒ 全局呈现「内容不足一屏且贴底」）；③作用面为何必须按 `data-slot` 收窄（不影响其余套件） | R-1 的落地；本文件是测试基座而非测试用例，属 proposal「实现文件」表 |
| `packages/desktop/package.json` | `dependencies` 增 `"@tanstack/react-virtual": "3.14.26"`（精确锁版，peer 仅 `react` / `react-dom`）；`dependencies` 删 `"@shadcn/react"`（primitives 退役后零可达消费者，D19）；`version` `0.6.31` → `0.6.32` | AC-10 / D16 / D19；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随（零手改）；`src-tauri/Cargo.toml` crate 版本行**不跟**；`pnpm-lock.yaml` 由 `pnpm install` 机械更新（生成物，不手改） |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/components/ui/questionnaire.tsx` | 零消费者 / 零测试引用 / 零 archi 元素 / 零 API 契约登记的残件——`@shadcn/react` 的唯一另一引用面（全历史零 import，仅因 `knip.ignore` 含 `src/components/ui/**` 才未被报为 unused file）。删依赖后该件无法解析，按 `desktop-app-shell`「knip 报出的未用导出 SHALL 删减」同批剪除（D19）；`desktop-agent-execution` 的「questionnaire 接线为二期」只约束**接线**（AskUserQuestion 当前由 `tool-cards.tsx` 静态卡承载），二期经 shadcn CLI 重新生成 |

**零改动面（登记防误改）**：`src/lib/agent-adapter.ts` / `agent-transport.ts` / `parse.ts`、`src/hooks/use-agent-session*.ts`、`src/components/agent/{agent-blocks.tsx,agent-input.tsx,tool-cards.tsx,session-panel.tsx,agent-session-view.tsx,agent-error.ts}`、`src/components/ui/**`（除 `message-scroller.tsx` 的改动与 `questionnaire.tsx` 的删除）、`src/views/**`、`src/types/generated/**`、`src-tauri/**`、`.dev-team/architecture/**`、`openspec/specs/**`、`openspec/config.json`、`packages/desktop/knip.json`（禁 `ignoreDependencies` / `ignore` 豁免）、本 change 的 `explore.md`。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `hasVisibleParts` | `packages/desktop/src/lib/agent-virtual.ts` | 新增（自 `agent-messages.tsx` 迁出） | `(message: AgentUIMessage) => boolean` | 存在 `text` / `reasoning` / `tool-<name>` 部件即为真（`tool-` 前缀且非 `dynamic-tool`，与 `agent-blocks.tsx` 的 `isToolPart` 同口径——在 `agent-virtual.ts` 内复写而不 import，避免 `src/lib/**` 反向依赖 `components/`）；`data-*` 与 `step-start` 不计。两个生产消费点：`filterConversation` 与 `ChatBubbleBase` 的状态行判据（事实偏差 5） |
| `filterConversation` | 同上 | 新增（自 `useHistoryWindow` 抽出） | `(all: AgentUIMessage[]) => AgentUIMessage[]` | 对话透镜可见性单点：剔除 `role === 'system'`；空内容 assistant 消息仅 `metadata.record.status` 为 `failed` / `stopped` 时保留，`completed` 或轮行缺席时剔除。**判据语义与改造前逐字等价**，顺序保持 |
| `userTurnSpans` | 同上 | 新增 | `(messages: AgentUIMessage[], measurements: readonly { start: number }[], totalSize: number) => UserTurnSpan[]` | 吸附层几何：仅为 `role === 'user'` 的索引产条，按索引升序；`span = 下一 user 的 start − 本条 start`，末轮 `span = totalSize − start`；`measurements[index]` 缺席时该条跳过、末轮退化到 `totalSize`；`span` 下限钳制为 0；无 user 消息返回空数组 |
| `conversationIdentity` | 同上 | 新增 | `(messages: AgentUIMessage[]) => string \| null` | 会话身份（贴底复位键）：取首条携 `metadata.record.sessionId` 的消息的会话 id；全体缺席返回 `null` = 未知（流式首轮未收口 / 收口重放）。同一会话的追加与流式增长不改变返回值，换会话必变。供 `agent-messages.tsx` 派生 `resetKey` 传给 shim（D10） |
| `MessageScrollerProvider` | `packages/desktop/src/components/ui/message-scroller.tsx` | 修改（重写体内；入参全换） | `(props: MessageScrollerProviderProps) => React.JSX.Element` | 持有 `useVirtualizer` 与 `viewportRef`；`getScrollElement` 恒读 `viewportRef`；据 `resetKey` 与「首次非空 / 由空变非空」在 layout 阶段复位贴底一次（D10 ②）并产出 `pendingScroll` / `scrollToEnd`；渲染 context provider（无 DOM 节点，`children` 直出） |
| `MessageScroller` | 同上 | 修改（去 primitives） | `(props: React.ComponentProps<'div'>) => React.JSX.Element` | 纯 `div`；类名与 `data-slot="message-scroller"` 逐字保留 |
| `MessageScrollerViewport` | 同上 | 修改（去 primitives） | `(props: React.ComponentProps<'div'>) => React.JSX.Element` | 纯 `div` + context 的 `viewportRef`；类名清单逐字保留；`data-autoscrolling` / `data-pending-scroll` 条件输出（D11） |
| `MessageScrollerContent` | 同上 | 修改（几何） | `(props: { children?: React.ReactNode; className?: string }) => React.JSX.Element` | `position: relative` + 内联 `height: getTotalSize()`；渲 `children` → 可见切片（`getVirtualItems()`，`renderItem(index) === null` 的索引**不出节点**）与 `renderSticky?.(geometry)` 返回的条目**按 `index` 升序交织**（DOM 顺序即会话顺序），其中 `geometry.measurements` 直接取 `virtualizer.measurementsCache`（源码核对：长度恒为 `count`、按下标稠密可读） |
| `MessageScrollerItem` | 同上 | 修改（入参） | `(props: MessageScrollerItemProps) => React.JSX.Element` | `data-slot="message-scroller-item"` + `data-index={index}`；`position:absolute; top:0; left:0; width:100%; transform: translateY(start)`；类名保留 `[content-visibility:auto]` / `[contain-intrinsic-size:auto_10rem]` / `min-w-0 shrink-0` 并带 `px-3`；`ref` 挂零测量回落的测量函数（D6） |
| `MessageScrollerSticky` | 同上 | 新增 | `(props: MessageScrollerStickyProps) => React.JSX.Element` | 吸附层占位块：外层 `data-slot="message-scroller-sticky"` + `z-10`（与同轮 assistant 正文同层，绘制顺序决定遮挡；`z-index` 不创建包含块，不影响 sticky 夹取），`position:absolute` + **`top: start`** + `left:0` + `width:100%` + `height: span` + `pointer-events-none`（**禁 `transform`**，D7）；内层 `position:sticky; top:0` + `bg-card` + `px-3` + `pointer-events-auto`；`ResizeObserver` 观测内层 `borderBoxSize` → `resizeItem(index, h > 0 ? h : estimateSize(index))`（D5 / D6） |
| `MessageScrollerButton` | 同上 | 修改（判据） | `(props: MessageScrollerButtonProps & Pick<React.ComponentProps<typeof Button>, 'variant' \| 'size'>) => React.JSX.Element` | 到底按钮：`data-active = getDistanceFromEnd() > scrollEndThreshold`（缺省 24），`onClick = scrollToEnd()`（context 提供，内部 `scrollToIndex(count - 1, { align: 'end' })`，`count === 0` 时无操作）；`data-slot="message-scroller-button"` / `data-direction="end"` 保留，`direction` 入参退役（D13） |
| `useMessageScroller` / `useMessageScrollerScrollable` / `useMessageScrollerVisibility` | 同上 | **退役** | — | 零外部消费者；跳转能力由 `scrollToIndex` 承担（`messageId → index` 查表） |
| `AgentMessages` | `packages/desktop/src/components/agent/agent-messages.tsx` | 修改（体内；入参不变） | `(props: AgentMessagesProps) => React.JSX.Element` | `AgentMessagesProps`（`messages` / `loading?` / `running?`）零改动；过滤、虚拟窗口接线、吸附层接线在此收口 |

<!-- 私有实现件（shim 的 `measureWithFallback` / `useStickyMeasure` / `useScrollerContext` / `ScrollerContext` / `MessageScrollerSlice` / 复位 effect、`agent-messages.tsx` 的 `renderItem` / `renderSticky` 闭包与 `ToolPartView` / `PartView` / `TurnStatusLine`）按纪律不列本表；落点见「修改文件」与「数据模型」。 -->
<!-- 无 CLI 子命令 / 无 HTTP 端点：本变更为纯前端呈现层重构，不新增命令面。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `UserTurnSpan`（interface） | `packages/desktop/src/lib/agent-virtual.ts` | 新增 | 吸附层一条占位块的几何：`index: number`（过滤后列表索引）/ `start: number`（虚拟器测量起点）/ `span: number`（该轮跨度，含下一轮提问前的全部内容） |
| `MessageScrollerProviderProps`（interface） | `packages/desktop/src/components/ui/message-scroller.tsx` | 新增 | 虚拟窗口契约：`count: number`；`estimateSize: (index: number) => number`；`getItemKey: (index: number) => React.Key`；`renderItem: (index: number) => React.ReactNode \| null`（`null` = 该索引不产节点）；`renderSticky?: (geometry: MessageScrollerGeometry) => MessageScrollerStickyEntry[]`；`resetKey?: string \| null`（贴底复位键：`null` = 会话身份未知，不参与比较；缺省 `undefined` = 调用点不提供）；`overscan?: number`（缺省 8）；`gap?: number`（缺省 0）；`paddingStart?: number` / `paddingEnd?: number`（缺省 0）；`scrollEndThreshold?: number`（缺省 24）；`children?: React.ReactNode` |
| `MessageScrollerStickyEntry`（interface） | 同上 | 新增 | 吸附层一条：`index: number`（过滤后列表索引，供与虚拟切片交织）+ `node: React.ReactNode`（已装配的 `MessageScrollerSticky`）。`renderSticky` 返回条目数组而非单个节点，使吸附气泡与同轮 assistant 正文在 DOM 中按会话顺序相邻 |
| `MessageScrollerGeometry`（interface） | 同上 | 新增 | render prop 收到的几何：`measurements: readonly { start: number; size: number }[]`（= `virtualizer.measurementsCache`，长度恒为 `count`、按下标稠密可读——`getMeasurements` 为一次线性构建，见源码核对）/ `totalSize: number`（= `getTotalSize()`） |
| `MessageScrollerItemProps`（interface） | 同上 | 新增 | `index: number` / `start: number` / `children?: React.ReactNode` / `className?: string` |
| `MessageScrollerStickyProps`（interface） | 同上 | 新增 | `index: number` / `start: number` / `span: number` / `children?: React.ReactNode` |
| `MessageScrollerButtonProps`（interface） | 同上 | 修改 | 去 `direction`（D13），保留 `variant` / `size` / `className` / `children`；`render` 透传语义保留（供 base-ui `useRender` 形态的既有调用点，若无消费点则随 `direction` 一并退役——见待决问题 3） |
| `AgentMessagesProps`（interface） | `packages/desktop/src/components/agent/agent-messages.tsx` | **零改动**（登记防误改） | 三字段形态不动 ⇒ 全部消费点零 diff（AC-12） |
| `AgentUIMessage` / `TurnSummary`（生成物路径转发） | `packages/desktop/src/lib/agent-adapter.ts` / `src/types/generated/bindings.ts` | **零改动** | `agent-virtual.ts` 只 import 类型（`TurnSummary` 供轮状态与 `sessionId` 读取）；生成目录 MUST NOT 重导出 / 手改 |

### 配置

<!-- 无配置项：本变更零 KNOWN_KEYS 增删、零 TestFramework 变体、零 openspec/config.json 改动、零 crates/core/config/config.schema.json 镜像改动。package.json 的 version（版本纪律）与 dependencies（运行时依赖）分别登记在「修改文件」与「依赖」表。 -->

---

## 数据模型

> 本变更**零持久化面**：无表、无列、无 IPC 字段、无迁移。以下全部是**纯前端内存态 + DOM 几何**。

### 几何与账目（实现期的单一真相）

```
MessageScrollerViewport   overflow-y-auto   scroll-fade-b / scrollbar-* / contain-content
 │                         ↑ 滚动容器；sticky 的吸附边界 = 本元素的 padding box
 └── MessageScrollerContent   position: relative   height = virtualizer.getTotalSize()
      │                       ↑ 绝对定位的包含块；零 padding、零 flex gap（D12）
      ├── children（空态块 conversation-empty，px-3，仅过滤后为空时出现）
      │
      └── 按索引升序交织的两类条目（DOM 顺序 = 会话顺序）：
            · 可见切片里的 assistant 索引（renderItem 出的虚拟项）
            · 全部 user 索引（renderSticky 出的吸附层占位块，与 range 无关）

          虚拟项（assistant）
            <div data-slot="message-scroller-item" data-index={i}
                 style="position:absolute; top:0; left:0; width:100%; transform:translateY(start)">
              px-3 · [content-visibility:auto] · [contain-intrinsic-size:auto_10rem]
            </div>

          renderItem(i) === null 的索引（全部 user 索引）· 虚拟层不产节点

          吸附层占位块（user）
            <div data-slot="message-scroller-sticky" class="z-10"
                 style="position:absolute; top:{start}px; left:0; width:100%; height:{span}px;
                        pointer-events:none">                 ← 只用 top，禁 transform（D7）
              <div style="position:sticky; top:0"                   ← 吸附行为本体
                   class="bg-card px-3 pointer-events-auto">        ← 底衬 + 可交互（D8 / D9）
                <ChatBubble message={user 消息} />                  ← 与普通气泡同一渲染件
              </div>
            </div>

turnSpan(i) = start(next user) − start(i)        （末轮 = totalSize − start(i)）
```

| 量 | 来源 | 用途 |
|----|------|------|
| `count` | `filterConversation(messages).length` | 虚拟器项数；滚动条比例的唯一分母（AC-1） |
| `estimateSize(index)` | 角色分型常量（user 72 / assistant 240） | 首帧估值；`measureElement` / ResizeObserver 实测后覆盖 |
| `getItemKey(index)` | `filterConversation(messages)[index].id`（`turn-<turnId>` / `turn-<turnId>-user`） | 稳定键：滚动与追加不重建测量缓存 |
| `getTotalSize()` | 虚拟器 | `Content` 高度；`span` 的末轮边界 |
| `measurementsCache[index].start` | 虚拟器（已含 `paddingStart`），长度恒为 `count` | 虚拟项 `translateY`、吸附层 `top`、`span` 差分的共同基准 |
| `getDistanceFromEnd()` | 虚拟器 | 到底按钮 `data-active`（D10） |
| `isScrolling` | 虚拟器 | `data-autoscrolling`（D11） |
| `resetKey` | `conversationIdentity(filterConversation(messages))`（`useMemo`，`string \| null`） | 贴底复位判据（D10）：只参与「是否复位」判定，**不进虚拟器选项** |
| `pendingScroll` | shim 局部 `useState`（首测完成标志） | `data-pending-scroll`，遮住首帧 / 换会话那一跳（D11） |

**五条不变量**（AC-6 的机械判据，任一条失效即几何账目崩）：

1. 吸附层 `top` 与虚拟项 `translateY` 同基准（`measurementsCache[index].start`），差一位就错位；
2. 内容容器高度恰为 `getTotalSize()`，**不得**再有 `min-h-full` / CSS padding / flex gap 叠加；
3. 吸附层实测高度回填后，`Σ 实测 + 估算` 与 `getTotalSize()` 一致，末项底边与容器底边对齐；
4. 实测 `≤ 0` 的项回落 `estimateSize(index)`，**MUST NOT** 出现零高项（D6）；
5. user 索引在虚拟层零节点但**账目照占位**（`count` / `estimateSize` / `getTotalSize` 一视同仁）。

### 贴底复位的判据通道（AC-13）

```
messages ─ filterConversation ─ conversation ─ conversationIdentity(conversation) ─ resetKey ─┐
                                                                                              │
MessageScrollerProvider                                                                       │
  prevKeyRef: string | null | undefined  ← 上一轮 resetKey（undefined = 尚未有过键）           │
  firstNonEmptyRef / prevCount                                                                │
                                                                                              ▼
  condition = (count > 0 && prevCount === 0)              // ①首次非空 / 由空变非空
           || (prevKey !== null && prevKey !== undefined
               && resetKey !== null && prevKey !== resetKey)  // ②两侧均已知且不等
  condition ⇒ useLayoutEffect 内 scrollToIndex(count − 1, { align: 'end' })
```

| 场景 | `resetKey` 变化 | 是否复位 |
|------|----------------|----------|
| 历史重放挂载（`0 → N`） | `undefined → A` | **是**（条件①；`undefined` 不参与比较） |
| 新会话首轮开流（`0 → N`，尚无 `record`） | `undefined → null` | **是**（条件①；`null` 只挡条件②） |
| 首轮 `record` 落库（`null → A`，条数不变） | `null → A` | 否（条件②要求两侧均非 `null`；条件①不成立） |
| 收口重放（`A → null`，条数可能变） | `A → null` | 否（同上；`count` 未从 0 变正） |
| 同会话追加 / 流式增长 | `A → A` | 否（键不变） |
| 换会话（`A → B`，条数可相同） | `A → B` | **是**（条件②） |

比对基准 `prevKeyRef` 只在复位 effect 内推进（`undefined` 初值），因此**首帧之前不产生假阳性**；`resetKey` 不进 `useVirtualizer` 选项（进选项会让每次身份变化触发 `setOptions` 的端锚定重算，与复位语义重复且时序不可控）。

### DOM 钩子面（既有断言锚点，全部保留）

| 钩子 | 归属 | 本变更 |
|------|------|--------|
| `data-slot="message-scroller"` / `-viewport` / `-content` / `-item` / `-button` | shim | 逐字保留 |
| `data-slot="message-scroller-sticky"` | shim（新增） | 吸附层占位块 |
| `data-testid="chat-bubble"` + `data-role` | 透镜 | 逐字保留（条数断言改按 `data-role` 分查属测试相位） |
| `data-testid="conversation-empty"` / `agent-messages-loading` / `agent-messages-running` / `agent-messages` | 透镜 | 逐字保留 |
| `data-autoscrolling` / `data-pending-scroll` | shim（Viewport） | 语义不变，**条件输出**（D11 / 事实偏差 4） |
| `data-active` / `data-direction` | shim（Button） | `data-active` 换判据；`data-direction="end"` 恒在 |
| `aria-label="对话消息"` | 调用点透传到 Viewport | 逐字保留 |

### 吸附判定（为什么零状态）

`position: sticky` 的两个边界都是**布局**概念，与 JS 无关：吸附边界 = 最近滚动祖先（Viewport）的 padding box 顶边；顶走边界 = 包含块（占位块 `span`）的底边。于是「最近一条已完全滚出顶部的用户提问停于视口顶端」与「下一轮提问到达包含块底边即把上一条顶走」由 CSS 直接给出，**不需要** `scroll` 监听、不需要「当前吸附项」state、不需要虚拟器 `range` / `onChange`。滚回会话顶部时所有占位块的顶边都在视口内 ⇒ 自然无气泡吸顶。

### 持久化

| 项 | 本变更 |
|----|--------|
| 会话转录 / 轮行 / 生成绑定 | **零改动**（纯前端呈现层）；`src/types/generated/**` 零 diff |
| 虚拟窗口参数（`count` / `estimateSize` / 测量缓存） | **纯前端内存态**，MUST NOT 落库、MUST NOT 进 IPC 载荷 |
| `data-*` 钩子与几何（`start` / `span`） | **纯 DOM 表现**，不进任何持久面 |

---

## 路由 / API 设计

<!-- 不适用：本变更零 IPC 命令增删、零 payload 变化、零 HTTP 端点（Tauri 本地应用）。命令面（agent_start / agent_session_watch / agent_stop / session_detail / agent_session_transcript）由既有 change 定义，本变更零 diff。 -->

---

## 依赖

### 运行时依赖

| 依赖 | 版本 | 新增/既有 | 用途 |
|------|------|-----------|------|
| `@tanstack/react-virtual` | `3.14.26`（精确锁版） | **新增** | `useVirtualizer`：`anchorTo:'end'` + `followOnAppend` + `scrollEndThreshold`（贴底跟随）、`getItemKey`（稳定键）、`gap` / `paddingStart` / `paddingEnd`（纵向几何）、`measureElement` + `resizeItem`（变高测量与外部回填）、`getTotalSize` / `getVirtualItems` / `measurementsCache`（账目）、`getDistanceFromEnd`（到底按钮）、`scrollToIndex` / `scrollToEnd`（跳转与首帧/换会话复位）、`isScrolling`（HUD）。peer 仅 `react` / `react-dom`（已装），零传递依赖冲突 |
| `@shadcn/react` | — | **移除** | `message-scroller` primitives 退役后仓内零**可达**消费者（D19）；同批删除零消费者残件 `components/ui/questionnaire.tsx` |
| `react` / `react-dom` | 既有 | 既有 | 组件与 hooks；本变更不新增 hook 形态（context + `useVirtualizer`） |
| `cn`（`cn` 包） | 既有 | 既有 | shim 类名合成（既有定式，不用 `lib/utils`） |
| `lucide-react` | 既有 | 既有 | 到底按钮图标（`ArrowDownIcon`，逐字保留） |

### 构建 / 测试依赖

| 依赖 / 工具 | 用途 |
|-------------|------|
| `vite-plus`（`vp check` / `vp test`） | 静态检查与测试运行（机械步骤由桌面端执行）；本变更受 `max-lines-per-function`（tsx 80）约束 ⇒ shim 与透镜按件拆函数 |
| `typescript`（`tsc --noEmit`） | 虚拟器选项面与 render prop 类型的编译期校验（`isolatedModules` / `strict` / `noUnusedLocals`） |
| `knip` | 死代码扫描：三个 hook 再导出与 `useHistoryWindow` 族退役后**零未用导出**、`@shadcn/react` 剪枝后零 unused dependency；`src/components/ui/**` 在 `ignore` 面内（只抑制报告，不使其可解析），`src/lib/agent-virtual.ts` 的四个导出必须各有生产消费 —— 可判性由此成立：`filterConversation` / `hasVisibleParts` / `userTurnSpans` / `conversationIdentity` 全部由 `agent-messages.tsx` 生产消费 |
| `jsdom` + `@testing-library/react` | 组件断言（用例编写归测试相位）；`test-setup.ts` 的打桩是本变更的前置条件 |

**`pnpm-lock.yaml` 随 `pnpm install` 机械更新**（唯一生成物变化）；`Cargo.lock` / `Cargo.toml` 零变化。

---

## 验收标准回应

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 全量虚拟窗口 | D1（一层窗口）+ D3（instance 归 Provider，`Content` 渲 `getVirtualItems()` 切片、高度 = `getTotalSize()`）+ 公共函数表 `MessageScrollerContent` / `MessageScrollerItem`；滚动条比例由全量账目导出；`remaining` / `history-load-*` / 触顶观察器随 D15 退役 |
| AC-2 分页面退役 | D15（退役面逐项点名）+ tasks 阶段四 / 阶段七的 `grep` 自查（九类标识符零残留；`FakeIntersectionObserver` / `triggerReachTop` 属测试相位） |
| AC-3 user 消息常驻吸附 | D3（`renderItem` 对 user 返回 `null`）+ D5（`MessageScrollerSticky`）+ `userTurnSpans`（全部 user 索引无条件产条，不受 range 影响）+ `data-slot="message-scroller-sticky"` |
| AC-4 顶走与边界 | D7（①`top` 定位）、D9（零缓冲：`span` 即整轮跨度，无 gutter）+ 数据模型「吸附判定」段（纯 CSS）+ `userTurnSpans` 的空数组分支（无 user ⇒ 不渲吸附层） |
| AC-5 吸附零 scroll 状态 | 数据模型「吸附判定」段；shim 内唯一的滚动监听来自虚拟器自身（D3），`components/agent/` 零 `addEventListener('scroll'` |
| AC-6 高度回填与账目一致 | D5（`ResizeObserver` → `resizeItem`）+ D6（`≤0` 回落）+ D12（纵向几何单一来源）+ 数据模型「五条不变量」 |
| AC-7 视觉零装饰且既有 UI 保留 | D2（类名与 `data-slot` 逐字保留）+ D9（同组件同 tinted 变体 + `bg-card` 底衬）+ D10 / D11（跟随 / 复位 / 按钮判据 / HUD）+ 事实偏差 2 / 3（`min-h-full` 退役、横向 gutter 落行级） |
| AC-8 过滤判据语义零漂移 | 公共函数表 `filterConversation` / `hasVisibleParts`（判据逐字等价，仅换载体）；`ChatBubbleBase` 的其余面逐字不变 ⇒ 空内容轮用例语义不变 |
| AC-9 测试可渲染 | `test-setup.ts` 打桩（事实偏差 1：两个 getter、原型级、按 `data-slot` 收窄）+ D6（零测量护栏使估算值即几何）⇒ jsdom 下 `clientHeight = 600` 使 range 非空且可确定性推导。滚动写入的可观测性由 D10 的实例级 `scrollTo` 桩承接（不桩原型 ⇒ 不改变其余套件「写入静默无效」的既有语义）；条数断言改按 `data-role` 分查属测试相位 |
| AC-10 依赖与版本 | D16（`3.14.26` 精确锁版 + `0.6.31 → 0.6.32` + `Cargo.toml` 不跟）+ D19（`@shadcn/react` 依赖与 `questionnaire.tsx` 剪枝，禁 `ignoreDependencies` / `ignore` 豁免） |
| AC-11 门禁 | D17（编译闭合批）；`vp check` / `knip` / `vp test` / `cargo nextest` 由桌面端执行；零后端改动 ⇒ `server:check` 与 `api:check` 自然零 diff |
| AC-12 边界零 diff | 本篇边界段 + 「零改动面」清单 + tasks 阶段七的 `git diff --stat` 自查 |
| AC-13 首帧与换会话贴底 | D10（复位条件恰为「首次非空 / 由空变非空」或「两侧 `resetKey` 均非 `null` 且不等」；`prevKeyRef` 用 `undefined` 初值区分「尚未有过键」）+ 公共函数表 `conversationIdentity` + Provider 入参 `resetKey` + 数据模型「贴底复位的判据通道」六场景表；复位面只经 `agent-messages.tsx` 派生 `resetKey` ⇒ `AgentMessagesProps` 形状不变、`use-agent-session*` / `agent-session-view.tsx` / `src/views/**` 零 diff |

---

## 待决问题

1. **`agent-virtual.ts` 的函数表行数（spec 表述差，已按四行定案）**：proposal / spec 的 Module Contract 初稿把该表写成两行。本文档裁定导出四个函数：`hasVisibleParts`（供 `filterConversation` 与 `ChatBubbleBase` 共用，避免第二份三行判据，事实偏差 5）与 `conversationIdentity`（贴底复位键，AC-13）。**现状**：delta spec 已同步为四行，无剩余待决。
2. **虚拟器选项名已逐条对拍（风险降级为已核实）**：`@tanstack/react-virtual@3.14.26` + `@tanstack/virtual-core@3.19.11` 的 `.d.ts` 与源码已核对——`anchorTo: 'start' | 'end'`（缺省 `'start'`）、`followOnAppend: boolean | ScrollBehavior`（缺省 `false`）、`scrollEndThreshold`（缺省 `1`）、`gap` / `paddingStart` / `paddingEnd` / `overscan`（缺省 `1`）、`indexAttribute`（缺省 `'data-index'`）、`resizeItem(index, size)`、`measureElement(node)`、`getDistanceFromEnd()`、`isAtEnd(threshold?)`、`getTotalSize()`、`getVirtualItems()`、`measurementsCache`、`isScrolling`、`scrollToIndex(index, { align, behavior })`、`scrollToEnd({ behavior })` 全部在场。**结论：无命名风险**；剩余不确定项只有 D10 ② 的语义（首帧不自动贴底，已按源码定案）。
3. **`MessageScrollerButton.render` 的去留**：`direction` 退役后，`render`（base-ui `useRender` 形态）在本仓无消费点。设计默认**保留**透传以维持与 `button.tsx` 组合件的既有形状；若静态检查判其为无用面，可连同 `direction` 一并退役（不影响任何 AC）。
4. **打桩面与滚动可观测性（已定案）**：视口尺寸同步读 `clientHeight`、`scrollHeight` 只进 `getDistanceFromEnd`、项高度由零测量护栏兜住 ⇒ `test-setup.ts` 只需两个 getter（事实偏差 1），`getBoundingClientRect` 与 `ResizeObserver` 回调面都不必动。滚动写入的可观测性**不进原型**：`jsdom` 无 `Element.scrollTo` ⇒ 首帧 / 换会话复位与到底按钮的点击跳转在默认环境下静默无效（不抛错）；需要断言写入的用例在**实例级**给视口装一个只写 `scrollTop` / `scrollLeft`、**不派发 `scroll` 事件**的最小 `scrollTo` 桩。理由：装到 `Element.prototype` 会把其余套件里「`scrollTo` 静默无效」的既有语义改成「真写位置」（`stdout-panel.test.tsx` 一类用例的语义边界随之漂移）。
5. **吸附层与底部渐隐的叠放**：`scroll-fade-b` 是**底边**渐隐（explore §16 已纠正），与顶部吸附层零交集，故不存在取舍；但吸附气泡 `bg-card` 底衬在深色主题下的对比度需在实现后手工过一眼（本变更不引入新色值，沿用 `bg-card`）。
6. **顶走时机的观感微调**：按 explore §21 零缓冲落地（`span = 整轮跨度`）。若实现后观感偏早，调整点唯一且局部——`userTurnSpans` 的 `span` 减一个固定 gutter（纯函数一行），不触碰 DOM 结构与测试形态。
7. **`-scroller` 的 `overflow-hidden` 与 spec 措辞（口径收窄，见事实偏差 6）**：delta spec 写「吸附层与其祖先链 MUST NOT 出现 `overflow: hidden` / `auto`」，实测祖先链上 `-scroller` 携带 `overflow-hidden`（既有类名，承担圆角裁剪）。按 CSS 语义它不参与 sticky 的夹取（`-viewport` 才是最近的滚动容器），故本设计**保持不动**。若评审要求逐字满足，替代路径是把 `-scroller` 改为 `rounded-lg` + `contain-paint`（或把裁剪下压到 `-viewport` 的 `overscroll` 面），需同批复验全部消费点的视觉；两种落点的 DOM 钩子与测试面均不变。

---

## 实现期落定（执行相位回填）

设计阶段未钉死、实现时按实测撞出的接缝与后续修正（proposal D7 / AC-13），落定如下：

8. **吸附层与虚拟切片的 DOM 顺序必须按索引交织**：设计原文把吸附层排在虚拟切片之后（`Content` 内 `切片 → renderSticky`）。实测该顺序让「user 气泡恒在 assistant 气泡之前」——同一轮 `user` 提问被排到全局尾部，`chat-bubble` 的 `data-role` 序列与 `block-text` 文本序断言全线反转（`agent-messages` / `agent-session-view` / `agent-debug-view` / `agent-run-history` 四套件共 17 例红）。裁定：`renderSticky` 改为返回 `MessageScrollerStickyEntry[]`（`{ index, node }`），`Content` 按索引与虚拟切片交织挂载。DOM 顺序即会话顺序，零新增状态、零新增契约知识（`index` 本就在 `UserTurnSpan` 里）。

9. **吸附层需要正 z-index**：吸附气泡与同轮 assistant 正文在绝对定位下同层，绘制顺序决定遮挡。裁定：占位块容器加 `z-10`（只影响堆叠层，不影响 sticky 的夹取基准——`z-index` 不创建包含块）。

另两处顺带对齐的既有语义（`@shadcn/react` 原件的 `Viewport` 行为，设计未点名但属「UI 面逐字保留」范围）：`Viewport` 补 `role="region"`（可被调用点覆盖）与 `tabIndex={0}`；到底按钮 `type="button"` + 未激活时 `tabIndex={-1}`（不可聚焦，与原件的 `inert` 语义等价）。

10. **`@shadcn/react` 的 unused dependency 与 `questionnaire.tsx` 的剪枝**（实现期暴露、非本变更引入的死件）：该包在本变更前有两个引用者——`message-scroller.tsx`（经 `agent-messages.tsx` 可达 ⇒ 解析其 import）与 `questionnaire.tsx`（**全历史零 import 者**，仅因 `knip.ignore` 含 `src/components/ui/**` 才未被报为 unused file；`ignore` 只抑制报告、不使其可解析，故其 import 不计入依赖使用）。shim 按 D2 退役 `@shadcn/react/message-scroller` 后，该包再无**可达**消费者，`knip` 报 unused dependency。

    **MUST NOT 用 `ignoreDependencies` 消解**：`desktop-archi-api:263` 明文「MUST NOT 新增 `ignoreExportsUsedInFile` 或 `ignoreDependencies` 条目（那是关判据而非满足判据）」；`openspec/config.json` 的 `write_protection` 亦只锁 `packages/desktop/knip.json` 一处 —— 该保护正是拦住这条关判据的捷径（本文档早前版本曾建议补 `ignoreDependencies`，属误判，已撤回；`shiki` 的既往先例也已于 `224df0db` 反向删除，不构成依据）。

    裁定：按 `desktop-app-shell:337`「knip 报出的未用导出 SHALL 删减」**剪掉该死件**——删 `src/components/ui/questionnaire.tsx`（零消费者、零测试引用、零 archi 元素、零 API 契约登记）并同步移除 `package.json` 的 `@shadcn/react` 依赖。`desktop-agent-execution:874` 的「questionnaire 组件接线为二期」表述不受影响：该条约束的是**接线**留待二期（AskUserQuestion 当前以 `tool-cards.tsx` 的静态卡承载，spec 已写明「questionnaire 接线为二期」），MUST NOT 被读作「必须保留一个零消费者的文件」；二期接线时经 shadcn CLI 重新生成该件即可。仓库同型先例：`code-block-copy-button.tsx`（`224df0db`）与 `sheet.tsx`（`28133ade`）均以「零消费者」被删。

11. **复位判据从「一次性闩」改为「复位键通道」（推翻本文档早前版本与实现初稿）**：本文档早前的 D10 ② 与事实偏差 7 曾按「`count > 0` 且尚未贴过」的一次性闩落地。该闩**只覆盖首帧**——同一透镜实例换会话（A → B）时 `count` 恒 > 0，闩已置，视口停在 A 遗留的滚动位置（proposal D7 / R-14 已点名此偏差）。裁定：复位判据改为 spec「首帧与换会话贴底」的两条，复位键由 `src/lib/agent-virtual.ts` 的 `conversationIdentity(messages)` 导出（取首条携 `metadata.record.sessionId` 的消息的会话 id；全体缺席为 `null`），经 `MessageScrollerProvider` 的可选入参 `resetKey?: string | null` 传入 shim。实现要点三条：
    - **比对基准用 `undefined` 初值**：`prevKeyRef` 初值 `undefined`（尚未有过键）与 `null`（键=未知）语义不同——前者让首帧的 `undefined → A` / `undefined → null` 不受条件②干扰（条件①已负责首帧）；后者使 `null → A`（首轮 `record` 落库）与 `A → null`（收口重放）都不复位。
    - **`resetKey` 不进 `useVirtualizer` 选项**：进选项会让每次身份变化触发 `setOptions` 的端锚定重算，与复位语义重复且时序不可控（`anchorTo:'end'` 的时机由虚拟器掌握，不由复位判据掌握）。
    - **通道只走两文件**：`conversationIdentity` 在 `agent-messages.tsx` 内以 `useMemo` 派生后透传 ⇒ `AgentMessagesProps` 形状不变、`use-agent-session*` / `agent-session-view.tsx` / `src/views/**` 零 diff（AC-12 / AC-13）。
