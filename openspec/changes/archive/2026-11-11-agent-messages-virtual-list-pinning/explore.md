# agent-messages 虚拟列表升级（@tanstack/virtual）+ 顶部吸附用户消息

workflow_type: requirement

调研范围：现状渲染链路、滚动/分页双引擎、虚拟化落点、吸附实现候选、测试与规格纪律。

---

## 1. 现状链路（读过的真实代码）

```
AgentSessionView (components/agent/agent-session-view.tsx)
  └─ AgentMessages (components/agent/agent-messages.tsx)   ← 唯一对话透镜
       ├─ useHistoryWindow()   自研分页窗口：默认最新 10 条，触顶 +10
       ├─ MessageScrollerProvider autoScroll defaultScrollPosition="end"
       └─ MessageContainer
            ├─ MessageScroller / Viewport(aria-label=对话消息) / Content(p-3)
            ├─ topSentinelRef  ← IntersectionObserver(rootMargin 300px) 触发 loadMore
            └─ messages.map → MessageScrollerItem(messageId, scrollAnchor=role==='user')
                              └─ memo(ChatBubble) → TextBlock / ThinkingBlock / ToolPairCard…
```

- `MESSAGE_PAGE_SIZE = 10`，`HISTORY_PRELOAD_MARGIN_PX = 300`。
- 分页**不是**性能兜底的唯一手段：所有消息理论上都可能进 DOM（触顶装满即全量），仅靠
  `MessageScrollerItem` 的 `[content-visibility:auto]` + `contain-intrinsic-size` 兜渲染成本。
- `@shadcn/react@0.3.1` 的 `message-scroller` 是一台完整滚动引擎（已拆读 dist）：
  - 模式机 `following-bottom / free-scrolling / anchored-to-message / settling-jump`；
  - `data-scroll-anchor` 元素 + `scrollPreviousItemPeek(64)` 做「新消息上方留一行」；
  - HUD 的 `data-autoscrolling` / `data-pending-scroll` 驱动样式（`data-pending-scroll:invisible`、滚动条透明）；
  - `useMessageScroller` 暴露 `scrollToEnd / scrollToMessage / scrollToStart / scrollable / visibility`；
  - 内容增删由 `MutationObserver` + `preserveScrollOnPrepend` 做 prepend 锚定；
  - **`MessageScrollerButton` 的 `data-active` 只依赖 `stateStore`（start/end 可滚动）**，不依赖虚拟化。

关键结论：`message-scroller` 的「量元素 rect / 数 children / MutationObserver」全部假定**全量 DOM**。
虚拟化后 `items` 只是窗口切片，这层语义必须显式取舍（见 §3-D）。

---

## 2. 两台引擎的接缝

```
               ┌──────────── @shadcn message-scroller ────────────┐
messages[] ──▶ │ MutationObserver / rect 扫描 / scroll anchor     │──▶ DOM 高度=自然流
               └─────────────────────────────────────────────────┘
vs.
               ┌──────────── @tanstack/react-virtual ────────────┐
messages[] ──▶ │ count/getItemKey/estimateSize/scrollOffset        │──▶ 外层占位高=totalSize + abs 定位 slice
               └─────────────────────────────────────────────────┘
```

TanStack v3.14 现状（从 `virtual-core@3.19.11` 源码确认，可直接借力）：

| 能力 | 选项 | 对本需求的用处 |
|---|---|---|
| 变高项测量 | `measureElement` + ResizeObserver（首次测量补偿「顶部以上」项） | 气泡高度天然不等；重测量只在「整项在折线以上」才改 scrollTop → 流式增长不会拽动视口（源码注释正好点名 "streaming chat message growing at its bottom #1218"） |
| prepend 锚定 | `getItemKey` 稳定 + 边键变化时 eager 调 scrollOffset + `pendingScrollAnchor: anchor` | 触顶加载更早一页时视口不跳（替代 `preserveScrollOnPrepend`） |
| 贴底跟随 | `anchorTo:'end'` + `followOnAppend` + `scrollEndThreshold` | 替代 `autoScroll` + `defaultScrollPosition="end"` |
| 只渲可见区 | `directDomUpdates`（写 transform/height，仅 range 变化才 re-render） | 需要绝对定位 + `containerRef`；与我们「外层 sticky 吸顶」不冲突但要求配置一次定型 |
| 额外 render 槽位 | `rangeExtractor` | **吸附项的落点**（见 §4-C） |

已确认可安装：`@tanstack/react-virtual@3.14.26`（registry 可达），peer 只要 react/react-dom。

---

## 3. 虚拟化必须定的四件事（含取舍）

### A. 滚动窗口几何
推荐：视口 `overflow-y-auto`（沿用现 Viewport 的 `scroll-fade-b / scrollbar-thin / data-autoscrolling`）+
内部 `<div style={{height: totalSize, position:'relative'}}>` + 每项
`<div style={{position:'absolute', top:0, left:0, width:'100%', transform:`translateY(${start}px)`}} ref={measureElement}>`。
`gap-6`（内容列行距）不能再用 flex gap，改成 `virtualizer` 的 `gap: 24`（或 `paddingStart/End`）。

### B. item key 必须稳定
`getItemKey: (i) => messages[i].id`（`turn-<turnId>` / `turn-<turnId>-user`）。
不稳定的 key 会让测量缓存与 prepend 锚定全废；这也是虚拟化与「历史窗口追加更早一页」能共存的前提。

### C. 估算高度
`estimateSize` 按索引给分型估值（user ≈ 56–72，assistant ≈ 200–320，含工具卡可再抬），
不追求准——首次测量即校正，重要的是**滚动条比例别太离谱**与首帧贴底正确。

### D. 分页还留不留？（最大的分支点）

```
选项 W1（推荐·改动小）：分页层保留，只把「最近一页的 DOM」虚拟化
   messages(过滤后) ─ useHistoryWindow(offset/slice) ─ virtualizer(count=visible)
   · 触顶逻辑从 IntersectionObserver(sentinel) 迁到「scrollTop < 阈值」→ loadMore
   · 收益：不写散落全仓的断言（40+ 处 chat-bubble 条数/文本 grep）
   · 代价：两层窗口叠加，心智模型更绕；「已装 1000 条仍只渲 15 个 DOM」的收益已到手

选项 W2（更干净·改动大）：退役分页，virtualizer(count=全部过滤后消息)
   · 数据一次给全，渲染按需；`remaining` / 加载提示语义消失
   · 收益：一层窗口，prepend 语义归零，长会话首屏即全境（滚动条可用）
   · 代价：openspec 里 4 处 `remaining` / 分页计数条款要重写；
           agent-messages.test.tsx 的「分页」三组用例（含 1000 条那组）整体重写；
           prepend 锚定虽然 TanStack 已备，但方向反了——**底部追加才是主路径**

选项 W3：分页保留为「数据获取」，虚拟化只做渲染，但取消 offset 钳制（一次装完）
   · 等于 W2 的变体，只是留个钩子给未来「服务端分页」
```

---

## 4. 吸附：候选三种画法

需求原文：「最近一条超出顶部的用户发送的消息吸附在顶端」。

```
视口（overflow-y-auto）
┌─────────────────────────────┐
│ [sticky] user-msg-N  ⬅ 吸附层 │  ← 该用户消息已完全滚出顶部，仍留此
├─────────────────────────────┤
│ assistant 内容（虚拟化的 slice）│
│ …                           │
└─────────────────────────────┘
```

### 候选 A：容器外 sticky 层（最稳，与虚拟化完全解耦）

```
<section>
  <div class="absolute inset-s-0 inset-e-0 top-0 z-10">  ← 视口之上（不受滚动位置影响）
     {pinned && <ChatBubble message={pinned} />}
  </div>
  <viewport>
     <heightSpacer>{virtualItems.map(...)}</heightSpacer>
  </viewport>
</section>
```
- 需要给视口/内容加 `paddingTop = pinnedHeight`（否则吸附层盖住第一条可见项）；`ResizeObserver` 量吸附高度。
- 优点：不参与虚拟化测量，不会被回收，动画/阴影可自由做；「吸附解除」只是条件渲染。
- 缺点：吸附层与虚拟项之间要协调查高，且 `pinned` 依赖滚动位置 → 每次 scroll 都可能切 state（要节流 + 只在 entry 变化时 set）。

### 候选 B：虚拟项内 `position: sticky; top: 0`（视觉连续，但状态坑多）

虚拟化后只有 slice 在 DOM 里，「当前吸附项」往往是 **已经滚出窗口** 的那条——
所以仍然要靠 `rangeExtractor`（或 `overscan: 大量`）把它强行留在渲染集合首部，
再用 sticky 让它贴顶。sticky 在**绝对定位 + transform 父级**下容易失灵，需要额外验证。

### 候选 C：`rangeExtractor` 注入索引 + 容器外渲染（A 与 B 的折中）

```
rangeExtractor: (range) => {
  const base = defaultRangeExtractor(range);
  return pinnedIndex !== null && !base.includes(pinnedIndex)
    ? [pinnedIndex, ...base] : base;
}
```
- 该索引的虚拟项 `style` 改成 `position: sticky; top: 0; transform: none`，
  需要 `scrollMargin` 归零 + `top` 直接写 px。
- 陷阱：一旦把它当普通项参与 measure，它会因 sticky 的「布局位置≠视觉位置」污染 `ResizeObserver` 与
  `getBoundingClientRect` 派生逻辑（`indexFromElement` 用的是 `[indexAttribute]`，可控但要小心）。
- 我的倾向：**A**。吸附是一层 UI，不该塞进虚拟化测量链。

### 「最近一条超出顶部的用户消息」判定（纯函数，好测）

```
候选输入：可见/已装载消息列表 + 当前 scrollOffset + 每项 (start,size)（虚拟化本来就有）
判定：从最后一条 user 往前扫，第一个满足 item.end <= scrollOffset 的 user 消息 → 吸附
      若所有 user 消息都还在视口内 / 或更早的消息根本没装载 → null（不吸附）
边界：user 消息自身被窗口切掉（index < 首个装载索引）→ 复用同一个「已完全出顶」判据
      滚动回顶 / 滚到底 → 自然 null（滚到底时最后一条 user 一定不可见？不一定：末轮到顶时它仍在视口上方 → 仍吸附）
```
判定式只依赖 `(index, start, size)`，用 `virtualizer.measurementsCache` 或 `getOffsetForIndex` 即可，
应做成纯函数（`lib/` 或 `components/agent/`）以便 jsdom 直测（jsdom 无布局，靠打桩三元组驱动）。

---

## 5. 与其它既有语义的交叉（易踩）

| 面 | 现状依赖 | 虚拟化后 |
|---|---|---|
| `MessageScrollerItem` 的 `data-slot` 断言 | 测试 `querySelectorAll('[data-slot="message-scroller-item"]').toHaveLength(2)` | 需么保留 `data-slot` 钩子，么改断言 |
| `data-scroll-anchor`（user 消息） | HUD 的 peek / 锚定引擎读它 | 虚拟化后多数锚点元素不在 DOM |
| 到底按钮 `data-active` | 只读 scrollable store | 可沿用；也可换成「scrollOffset + totalSize 判定」 |
| `data-autoscrolling` / `data-pending-scroll` 样式 | provider 内部 ref 切换 | 需自己实现或保留 provider 外壳 |
| `history-load-hint`（加载中… 提示） | sentinel 内文 | W1 保留于顶部；W2 语义消失 |
| `turn-status-line` / 空内容轮过滤 | `useHistoryWindow` 内单一判据 | 判据原样保留，只是输出喂给 virtualizer |
| 滚动到底按钮 / `scrollPreviousItemPeek=64` | provider | 若卸掉 provider，这些都要自建 |
| 挂载时机 | jsdom 无布局（`clientHeight=0`） | 虚拟化在 jsdom 里 range 恒空 → **所有气泡断言会变成 0 条**，必须打桩 `observeElementRect`/`measureElement`（仓库已有 `offsetHeight` 打桩先例：`architecture-view.test.tsx`、`stdout-panel.test.tsx`） |

---

## 6. 工程纪律（这个仓库会真的卡住实现）

- `vite.config.ts` lint：`max-lines-per-function` tsx 80 行、`typescript/no-unsafe-type-assertion`、
  `no-non-null-assertion`（测试除外）→ 实现要拆成 `useVirtualMessages` / `usePinnedUserMessage` /
  `VirtualItem` 小组件。
- `knip`：只被测试引用的导出算未用；纯判据若需导出，须有生产消费点。
- 依赖新增：`@tanstack/react-virtual` 进 `packages/desktop/package.json`；`components/ui/**` 在
  `vite.config.ts` 的 `NO_STATIC_CHECK` 与 `knip.ignore` 里被豁免 —— 若把虚拟列表包成
  `src/components/ui/virtual-messages.tsx`（shadcn 风格 shim，带 `data-slot`），可沿用这条豁免。
- `cn` 来自 `cn` 包（不是 `lib/utils` 的 local cn），新组件照抄 `message-scroller.tsx` 的写法。
- 版本纪律：动 `packages/desktop/src` 就要 bump `packages/desktop/package.json` patch（当前 0.6.27）。
- 规格纪律：`openspec/specs/desktop-agent-chat-infra/spec.md` 里「展示组件族(对话透镜)」明确写了
  「滚动 SHALL 沿用 shadcn `message-scroller` 组合，消息状态留在应用层」，且 `useHistoryWindow` 的
  分页/过滤判据被 AC 锁死 → **本变更必须带 spec delta**（改 W1 则条款措辞微调；改 W2 则 `remaining`
  相关 4 处 scenario 全部重写）。

---

## 7. 未决问题（需要你拍板）

1. **分页去留**：W1（保留分页 + 虚拟化渲染）/ W2（退役分页、全量虚拟化）/ W3（保留加载提示但一次装完）？
   - 提示：W2 最干净但会重写 agent-messages.test.tsx 三组「历史分页」用例与 spec；
     W1 风险最小，但「触顶预载」从 IO 改成 scroll 阈值，仍需改一处测试。
2. **message-scroller 原语保留到什么程度**：
   - 只留 `Root/Viewport/Content`（拿样式与 HUD 钩子）+ 自管滚动？
   - 还是彻底退役 `@shadcn/react/message-scroller`，自己写视口（`data-autoscrolling` 等状态要自维）？
3. **吸附形态选 A / B / C**；以及吸附层的视觉（是否与 user 气泡同款 tinted？是否加高度渐变 / 顶部阴影表示「上方还有内容」）。
4. **吸附触发阈值**：严格「完全出顶」（`item.end <= scrollTop`）还是「顶部以上 8px 淡出」更自然？
   是否需要在吸附项旁边给「回到该提问」点击跳转（`scrollToMessage` 的替代：`scrollToIndex`）？
5. **滑动窗口与吸附的相互影响**：当吸附项落在已装载窗口之外（更早的历史页尚未加载）时，
   是否为了吸附而**自动向前加载一页**？（这条最容易被忽略，也最容易做出「吸附闪烁」）
6. 是否需要「新消息到达时若用户已上滑（非贴底）则显示未读/回底按钮」——现有 `MessageScrollerButton` 已覆盖，是否沿用。

---

## 8. 建议的落地切分（供后续 change 参考，不在本轮实现）

1. `lib/`：`pinnedUserMessage(messages, virtualMeasurements, scrollOffset)` 纯函数 + 单测。
2. `components/ui/virtual-messages.tsx`：`@tanstack/react-virtual` 的 shadcn 风格 shim
   （`<VirtualMessages count=… getItemKey=… estimateSize=… renderItem=… />` + `data-slot` 钩子）。
3. `components/agent/agent-messages.tsx`：`useHistoryWindow` 输出直接喂 virtualizer（选 W1/W2），
   触顶判定改为 scroll 阈值；吸附层抽出 `PinnedUserMessage`。
4. `test-setup.ts`：加 `observeElementRect`/`measureElement`/`ResizeObserver` 打桩辅助（仓库已有同类先例）。
5. 测试：气泡断言改为「可见 slice」断言 + 滚动驱动；吸附用例用三元组打桩直测判据；
   保留 40+ 处 `chat-bubble` 断言的兼容性（W1 下可原样）。bump 0.6.28 + spec delta。

---

# 第二轮：用户拍板后的方案收敛

决策：**① W2（退役分页，全量虚拟化） ② 尽量保留 UI，与 headless 组件结合，直接改 `components/ui/message-scroller.tsx`
③ 容器内不可见占位块 + sticky，接受「所有 sticky user 消息不受虚拟化影响」**

## 9. 关键机制确认：sticky 是「被包含块夹住」，不是 overlay

这是第 3 条能成立的**全部依据**，也是「占位块」的真实职责重新定义：

```
position: sticky 的吸附边界 = 最近的滚动祖先的 padding box（这里是 viewport）
position: sticky 的「被顶走」边界 = 它的**包含块**（最近的块级祖先）的底边
```

所以要让「下一轮把上一轮提问顶走」，包含块必须**恰好覆盖一整轮**：

```
MessageScrollerContent   position:relative   height = virtualizer.getTotalSize()
 │
 ├── 虚拟层（assistant 项）  position:absolute  translateY(item.start)  ← @tanstack/virtual
 │
 └── 吸附层（user 项，永不进虚拟窗口的渲染集合）
       <div style={{ position:'absolute', top:userItem.start, height:turnSpan }}>
            ↑ 这是"不可见占位块"——它不遮任何东西，它是 sticky 的包含块
         <div style={{ position:'sticky', top:0 }}>  ← 真实 tinted 用户气泡
       </div>
```

**结论：占位块不是遮罩层，也不需要额外留 padding。** 它的高度 = `下一 user 的 start − 本 user 的 start`
（末轮 = `totalSize − start`），已由虚拟器测量账目直接给出，纯函数可算。

### 三个必须遵守的约束（否则 sticky 静默失效）

| 约束 | 原因 | 违反后果 |
|---|---|---|
| 吸附层**不得**用 `transform` 定位，只能用 `top` | `transform` 会创建新的包含块，sticky 的夹取基准被换掉 | 气泡吸住后永不顶走（末轮除外），表现为「一直覆盖全屏」 |
| 吸附层祖先链上不得出现 `overflow: hidden/auto`（含 `contain: paint` 之外的裁剪） | 会另立滚动容器，吸附基准从 viewport 变成本层 | 完全不吸附 |
| Viewport 的 `contain-content`（`contain: content`）保留 | 它只是布局/绘制隔离，不创建包含块，**不影响** sticky | 无 |

`contain-content` 已在现有 Viewport 类名里，核对过不伤 sticky（`contain: content` = `layout paint`，不含 `size`/`style` 的包含块语义）。
吸附层气泡需 `bg-card` 底衬：`Bubble variant="tinted"` 有不透明底，但**圆角边缘会漏出下层滚动正文**。

### 高度回填：吸附层量到的高度必须喂回虚拟器

```ts
// 吸附层每个占位块用 ResizeObserver 量真实气泡高度
// borderBoxSize 反映**布局盒**，不受 sticky 视觉偏移影响 → 吸顶时量到的仍是真实高度
virtualizer.resizeItem(userIndex, measuredHeight)   // 公开 API：index.d.ts:186
```

虚拟器仍把 user 索引计入 `count` / `estimateSize` / `totalSize`（位置账目不变），
但**虚拟层对 user 索引渲染 `null`**，高度由吸附层的测量独占。这样 `totalSize` 与实际一致，
`turnSpan` 的算法（相邻 start 相减）也自动成立。

## 10. W2 + 「所有 user 常驻」的规模问题与补救

W2 一次给全量。若**字面**执行「所有 sticky user 消息不受虚拟化影响」，5000 条对话 =
5000 个气泡常驻 DOM，虚拟化收益被吃掉一大块。

**补救（推荐，改动极小）**：把吸附层的**挂载窗口**收成可见 range 的前后各一轮——

```
range = virtualizer.getVirtualIndexes()   // 或 onChange 里读 instance.range
吸附层挂载 = [rangeStart 之前最后一个 user, rangeEnd 之后第一个 user]
```

当前被吸附的那条**必然落在窗口内**（它就是 rangeStart 之前最后一个 user），于是：
- 常驻数量从 O(全部) 降到 **O(可见轮数)**；
- **依然零 scroll 监听**（range 来自虚拟器自己的 `onChange`）；
- 依然是纯 CSS 吸顶。

骨架完全一样，差别只在「吸附层渲染哪些 user」这一行的过滤条件。
若你希望先拿最简形态落地，可以先写「全部常驻」，把这段过滤留作后续优化——但**测试要按窗口形态写**，否则以后改会翻红。

## 11. `message-scroller.tsx` 的军民分界（第 2 条的落点）

已核对 `src/` 全域（排除 shim 自身与测试）：`useMessageScroller` / `useMessageScrollerScrollable` /
`useMessageScrollerVisibility` / `scrollToMessage` / `scrollToEnd` / `scrollToStart` **零外部消费者**。
shim 可自由改造，不背兼容包袱。

### 保留（真实在用的 UI 面）

| 保留项 | 来源 | 虚拟化后如何维持 |
|---|---|---|
| `scroll-fade-b` 顶部渐隐 | `shadcn/dist/tailwind.css` 的 `@utility scroll-fade`，纯 CSS + `animation-timeline: scroll()` | **原样不动**，零成本（不依赖任何 JS） |
| `data-autoscrolling`（滚动条透明） | provider 内部 ref 切换 | 由虚拟器 `isScrolling` 驱动 setAttribute |
| `data-pending-scroll`（首帧隐形防跳） | provider 内部 ref | 由「首次测量完成」标志驱动 |
| 到底按钮 `data-active` | 现读 scrollable store（start/end） | 换成 `getDistanceFromEnd() > threshold`，语义等价 |
| `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content` | 纯类名 | 原样不动 |
| `data-slot="message-scroller-*"` / `data-slot="message-scroller-item"` | 测试锚点 | 保留（40+ 处断言依赖） |
| `[content-visibility:auto]` + `[contain-intrinsic-size]` | Item 类名 | **建议保留**：单条超长 assistant（50 张工具卡）仍是单虚拟项，两级机制叠着用 |
| `<MessageScrollerProvider>` 外壳 | `AgentMessages` 顶层 | 保留外壳与 props（`autoScroll` / `defaultScrollPosition`），内部改为配置 virtualizer |

### 退役（交给 virtualizer）

| 退役项 | 取代 |
|---|---|
| 模式机 `following-bottom` / `free-scrolling` / `anchored-to-message` / `settling-jump` | `anchorTo:'end'` + `followOnAppend` + `scrollEndThreshold` |
| `preserveScrollOnPrepend` + 内容 prepend 锚定 | W2 无 prepend；且 virtualizer 自带边键变化锚定 |
| `scrollPreviousItemPeek(64)` | sticky 吸附层 + assistant 项 `gap` |
| rect 扫描 / `MutationObserver` / `getBoundingClientRect` 全量遍历 | virtualizer 的 ResizeObserver 单点测量 |
| `useMessageScrollerVisibility`（`visibleMessageIds` / `currentAnchorId`） | 由 `instance.range` 直接给 |
| `scrollToMessage(messageId)` | `scrollToIndex(index)`（messageId → index 查表） |

### `AgentMessages` 侧接线

```ts
// useHistoryWindow 退役，过滤判据单独抽为纯函数保留（turn-status-line 语义不变）
const messages = useMemo(() => filterConversation(allMessages), [allMessages]);
const virtualizer = useVirtualizer({ count: messages.length, getItemKey: i => messages[i].id, ... });
```
`remaining` / `history-load-hint` / `history-load-complete` 相关分支与 IO 观察器整体删除。

## 12. 落地清单（W2 形态）

1. **`components/agent/virtual-messages.ts`（新，纯函数落模块）**
   - `filterConversation(all: AgentUIMessage[]): AgentUIMessage[]`（从 `useHistoryWindow` 抽出，语义零改）
   - `userGroupSpans(messages, measurements): { index, start, span }[]`（吸附层占位块几何）
   - 为什么必须落模块：`knip` 的 `project` 排除测试文件，组件内为测试开 `export` 会被判 unused；
     仓库既有同型解（`flow/metrics.ts` / `flow/stdout-text.ts` / `flow/run-state.ts`）
2. **`components/ui/message-scroller.tsx`（改）** — 按 §11 军民分界改造：新 props（`count` / `estimateSize` /
   `getItemKey` / `renderItem` / `renderSticky`），保留全部 `data-slot` 与 UI 类名，内部接 `useVirtualizer`
3. **`components/agent/agent-messages.tsx`（改）** — 退役 `useHistoryWindow` / IO 观察器 / `remaining`，
   user 索引走 `renderSticky`，assistant 索引走 `renderItem`
4. **`package.json`** — 加 `@tanstack/react-virtual`（已确认可装，peer 只要 react/react-dom；3.14.26）
5. **`test-setup.ts`** — 加虚拟化打桩辅助。
   **jsdom 实测确认**：`clientHeight=0`、`getBoundingClientRect` 全 0、`scrollTop` 可写可读
   → 不打桩则 visible range 恒空、**所有 `chat-bubble` 断言变 0 条**。
   仓库已有 `offsetHeight` 打桩先例（`architecture-view.test.tsx:188`、`stdout-panel.test.tsx:360`）
6. **bump** `packages/desktop/package.json` → `0.6.28`（AGENT.md 版本纪律）

## 13. 测试重写面（W2 的真实代价，逐条点名）

`agent-messages.test.tsx`（990 行）里 **「历史分页」整组 5 例 + 「大列表分页」1 例** 失效：

| 用例（现文） | W2 后的命运 |
|---|---|
| 默认仅渲染最新一页（10 条）… 提示「加载中…」 | **删/重写** → 「虚拟窗口内的气泡数」断言 |
| 触顶连续装载：每触发一次增一页 | **删** → 改「scrollTop 驱动 range 变化」 |
| 观察器 root 绑定视口，rootMargin 300px | **删**（IO 退役） |
| system 载体撑大入参不计入分页 | **保留语义，改断言**（过滤纯函数直测） |
| 换会话（头部变化）→ 分页复位 | **删**（无 offset 可复位） |
| 1000 条 → 默认渲 10 条、remaining 正确 | **重写** → 「1000 条仅渲可见区间 + 吸附层窗口」 |

另：`FakeIntersectionObserver`（约 30 行）整体退役；`triggerReachTop()` 退役。

**其余 34 例不受影响**（气泡变体 / system 过滤 / 状态行 / 一轮一条 / citations）——
但全部需要 `test-setup` 的虚拟化打桩才能看到气泡。

## 14. Spec delta 面（`desktop-agent-chat-infra`）

必须改的条款（`openspec/specs/desktop-agent-chat-infra/spec.md`）：

| 位置 | 现文 | 改成 |
|---|---|---|
| L323「空内容轮可见性」 | 「分页计数 SHALL 基于过滤后列表（不把不渲染的空内容消息计入 `remaining`）」 | 「虚拟化 `count` SHALL 基于过滤后列表」 |
| L284 / L294 / L352 三处 scenario | 提及「不参与分页 `remaining` 的错误计数」「`remaining` 分页计数不计入」 | 删 `remaining` 措辞，或换「不计入虚拟列表项数」 |
| 「展示组件族(对话透镜)」末条 | 「滚动 SHALL 沿用 shadcn `message-scroller` 组合，消息状态留在应用层」 | 「滚动 SHALL 由 `message-scroller` 承载，内部以 `@tanstack/react-virtual` 渲染窗口切片；用户提问 SHALL 以 sticky 吸附层常驻（不受虚拟窗口影响）」 |
| 「展示组件族」**新增** requirement/scenario | — | 吸附语义：最近一条完全滚出顶部的 user 消息吸顶；下一轮到达时被顶走；user 消息在虚拟层渲染 `null` 且高度由吸附层回填 |

`desktop-standard-components` 不含 message-scroller 条款（91 行全文已核对），无需改。

## 15. 仍待你确认的两点

1. **吸附层挂载窗口**：先写「全部 user 常驻」（最简、贴合你的第 3 条原话），还是直接写「可见 range ± 1 轮」？
   —— 我倾向后者，但若你想先看效果，前者也能在后续小步优化。
2. **scroll-fade-b 与吸附层的叠放**：`scroll-fade-b` 是 mask，作用在 viewport 上，会给吸顶气泡顶部也加一层渐隐。
   视觉上可能想要「吸附层不参与渐隐」——那就得把 mask 从 viewport 挪到虚拟层容器上。
   要不要现在就定这条？（影响 §11 里 `scroll-fade-b` 的落点）

---

# 第三轮：两条补充决策与一处自我纠正

决策：**① 会话中用户消息非常有限（通常 ≤ 10 条）→ 「所有 sticky user 常驻」按字面执行，不做挂载窗口优化
② 不要渐变**

## 16. 纠正：我上一轮关于 `scroll-fade-b` 的说法是错的

第 2 轮我把 `scroll-fade-b` 说成「顶部渐隐、会污染吸附层」。**读源文件后确认这不是事实**：

```css
/* shadcn/dist/tailwind.css:317 */
@utility scroll-fade-b {
  --scroll-fade-mask: linear-gradient(
    to bottom,
    #000 0,
    #000 calc(100% - var(--scroll-fade-b, 0px)),   /* 上部实心 */
    transparent 100%                                /* 只有最底部透明 */
  );
  @supports (animation-timeline: scroll()) {
    animation-timeline: scroll(self y);             /* 绑自身滚动位置 */
    animation-range: calc(100% - 24sp) 100%;        /* 滚到底时渐隐消失 */
  }
}
```

- **`-b` = bottom**，是「下面还有内容」的底边渐隐，不是顶部；
- 吸附层在**顶部**，与这条 mask **零交集**——第 2 轮提的那个「要不要把 mask 挪到虚拟层」的问题**不存在**；
- `animation-timeline: scroll(self y)` 要求**承载元素自身是滚动容器**，现在挂在 `MessageScrollerViewport` 上正好成立。
  **这条约束很重要**：如果实现时把 `scroll-fade-b` 从 Viewport 挪到 Content 或别处，动画时间轴立刻失效（退化成静态渐变）。

结论：`scroll-fade-b` **原样留在 Viewport 上，一个字都不改**。

## 17. 「不要渐变」的落点

用户指的是**吸附层自身的装饰**，不是既有底边渐隐。因此：

- 吸附层气泡**不加**顶部渐隐 / 底部阴影 / 高度渐变遮罩；
- 不做「上方还有内容」的视觉提示（那条历史由滚动条与内容本身表达）；
- 吸附层与普通气泡**视觉完全一致**（同一个 `ChatBubble`），只多一层 `bg-card` 底衬防圆角透出。

`scroll-fade-b`（底边渐隐）**保留**，它与本决策不冲突——它是既有 UI，且位于视口底部。

## 18. 用户消息 ≤ 10 条 → 「全部常驻」直接成立

第 2 轮 §10 提的「挂载窗口 ± 1 轮」优化**取消**，理由充分：

| 量级 | 常驻 sticky 气泡数 | 结论 |
|---|---|---|
| 典型会话 | ≤ 10 | 可忽略 |
| 极端长会话 | 即便 100 轮也只有 100 条 user 消息 | ResizeObserver 观测 100 个元素仍属轻量（只观测，不重排） |

于是吸附层退化为**最简形态**：

```tsx
// 全部 user 消息无条件常驻，零 range 过滤、零窗口计算
const users = messages.map((m, index) => ({ m, index })).filter(({ m }) => m.role === 'user');
```

好处：吸附层代码里**没有任何**「当前可见范围」概念，与虚拟器 `onChange` 完全解耦——
吸附完全是 CSS 的事，虚拟器只负责给 user 索引留出高度账目。

## 19. 由此产生的两个新结论

### 19.1 虚拟层的 user 槽位渲染 `null`，但高度账目照留

```
virtualizer.count = messages.length              // user 与 assistant 一视同仁
virtualizer.getItemKey = i => messages[i].id
estimateSize(i) = messages[i].role === 'user' ? USER_ESTIMATE : ASSISTANT_ESTIMATE

渲染：
  virtualItems.map(item =>
    messages[item.index].role === 'user'
      ? null                                     // ← 吸附层独家持有，虚拟层不出节点
      : <AssistantRow style={{ position:'absolute', top:0, transform:`translateY(${item.start}px)` }} />
  )
```

高度回填：吸附层每个气泡 `ResizeObserver` → `virtualizer.resizeItem(userIndex, h)`
（公开 API，`index.d.ts:186`；`borderBoxSize` 反映**布局盒**，吸顶时量到的仍是真实高度）。

### 19.2 ★ 测试断言语义变了：`chat-bubble` 不再等于「可见气泡数」

这是 W2 + 全量 user 常驻最容易踩的坑，必须提前写进测试设计：

```
改造前：25 条消息 → getAllByTestId('chat-bubble') 恰 10 条（分页窗口）
改造后：25 条消息 → user 消息（~12 条）**全部在 DOM** + assistant 可见切片（若干）
                    ⇒ getAllByTestId('chat-bubble') 的条数既非 10 也非全量，是两者之和
```

**所有按 `chat-bubble` 总数断言的用例必须改写为按角色分查**：

```ts
const users      = screen.getAllByTestId('chat-bubble').filter(b => b.dataset.role === 'user');
const assistants = screen.getAllByTestId('chat-bubble').filter(b => b.dataset.role === 'assistant');
```

仓库现存 40+ 处 `chat-bubble` 断言（`agent-messages.test.tsx` 主流，
`agent-debug-view.test.tsx` 3 处，`change-chat-panel.test.tsx` 1 处，`explore-detail-pane.test.tsx` 2 处），
**其中凡断言「条数」的都受影响**；只断言「存在 / 文本内容」的不受影响。

## 20. 更新后的落地清单（替代 §12）

| # | 文件 | 动作 |
|---|---|---|
| 1 | `components/agent/virtual-messages.ts`（新） | `filterConversation()`（从 `useHistoryWindow` 抽出，**语义零改**）+ `userTurnSpans()`（占位块几何纯函数） |
| 2 | `components/ui/message-scroller.tsx`（改） | 按 §11 军民分界改造；`scroll-fade-b` 原样留在 Viewport；保留全部 `data-slot` |
| 3 | `components/agent/agent-messages.tsx`（改） | 退役 `useHistoryWindow` / IO / `remaining`；user 走常驻 sticky 层，assistant 走虚拟层 |
| 4 | `package.json` | `@tanstack/react-virtual`（3.14.26，peer 仅 react/react-dom） |
| 5 | `test-setup.ts` | 虚拟化打桩（jsdom 实测 `clientHeight=0` / 矩形全 0 ⇒ 不打桩则气泡断言全变 0） |
| 6 | `agent-messages.test.tsx` | 删「历史分页」5 例 + 「大列表分页」1 例；`FakeIntersectionObserver` 整体退役；**条数断言改按角色分查** |
| 7 | spec delta | §14 列出的 4 处 `remaining` 措辞 + 新增吸附 requirement |
| 8 | bump | `0.6.28` |

## 21. 至此仍开放的唯一问题

**吸附的「顶走」时机是否需要额外缓冲？**

因为包含块高度 = 整轮跨度，下一轮提问会**紧贴**把上一条顶走（无缓冲）。
如果你希望「下一条提问进入视口下缘时才开始顶走」（更早松开），
需要在包含块底部留缓冲（`turnSpan` 减去一个 gutter），属于微调项，可在实现后视效果调。
默认按「零缓冲、紧贴顶走」实现。

---

# 终局：决策台账

探索结束。全部开放项已收敛，无待定项。

## 22. 决策全表

| # | 议题 | 结论 | 依据 |
|---|---|---|---|
| 1 | 分页层去留 | **W2 退役分页**，全量虚拟化 | 用户拍板；一层窗口，prepend 语义归零 |
| 2 | shim 处理 | **保留 UI + 与 headless 结合，直接改 `message-scroller.tsx`** | 用户拍板；三个 `useMessageScroller*` hook 与 `scrollToMessage` 在 `src/` 零外部消费者，可自由改造 |
| 3 | 吸附实现 | **容器内不可见占位块 + `position: sticky`** | 用户拍板；纯 CSS 吸顶/顶走，零 scroll 监听 |
| 4 | 吸附层挂载范围 | **全部 user 消息无条件常驻** | 用户补充「用户消息通常 ≤10 条」→ 挂载窗口优化取消 |
| 5 | 装饰渐变 | **不要** | 用户拍板；吸附气泡与普通气泡视觉一致 + `bg-card` 底衬 |
| 6 | 顶走缓冲 | **暂不加**，先零缓冲实现，看真实效果再调 | 用户拍板 |
| 7 | `scroll-fade-b` | **原样留在 Viewport，一字不改** | 源文件确认为**底边**渐隐（我第 2 轮误称顶部，已更正）；且 `animation-timeline: scroll(self y)` 要求承载元素自身是滚动容器 |
| 8 | `content-visibility:auto` | 建议保留（叠加使用） | 单条超长 assistant 仍是单虚拟项，两级机制不互斥 |

## 23. 三个必须固化的技术前提（违反即静默失效）

1. **吸附层只能用 `top` 定位，不得用 `transform`** —— transform 会创建新包含块，sticky 的夹取基准被换掉，表现为「气泡吸住后永不顶走」。
2. **吸附层祖先链不得出现 `overflow: hidden/auto`** —— 会另立滚动容器，吸附基准从 viewport 变成本层，表现为「完全不吸附」。
3. **`scroll-fade-b` 必须留在 Viewport 上** —— 挪走则 `animation-timeline: scroll(self y)` 失效，退化为静态渐变。

`MessageScrollerViewport` 现有的 `contain-content`（`contain: layout paint`）不创建包含块，**不影响 sticky**，核对通过。

## 24. 几何与账目（实现时的单一真相）

```
content  position:relative  height = virtualizer.getTotalSize()
 │
 ├── 虚拟层   assistant 项：position:absolute  transform:translateY(item.start)
 │            user 项：      渲染 null（高度账目仍占位）
 │
 └── 吸附层   user 项：position:absolute  top:item.start  height:turnSpan
                      └─ 内层 position:sticky  top:0  → 真实气泡
                      高度经 ResizeObserver 回填 virtualizer.resizeItem(index, h)

turnSpan(i) = nextUserStart − thisUserStart        （末轮 = totalSize − start）
```

`resizeItem` 与 `getDistanceFromEnd` / `isAtEnd` / `getOffsetForIndex` 均为公开 API
（`virtual-core/dist/esm/index.d.ts:186/200/201/203`），无需私有 hack。

## 25. 交付面清单

| 类别 | 内容 |
|---|---|
| 新增依赖 | `@tanstack/react-virtual@3.14.26`（peer 仅 react/react-dom；registry 可达） |
| 新增文件 | `components/agent/virtual-messages.ts`（`filterConversation` + `userTurnSpans` 纯函数，落模块而非组件内——knip 会判组件内 test-only 导出为 unused） |
| 改动文件 | `components/ui/message-scroller.tsx`、`components/agent/agent-messages.tsx`、`test-setup.ts` |
| 删除 | `useHistoryWindow`、`useTopReachObserver`、`HistoryLoadStatus`、`remaining` / `history-load-hint` / `history-load-complete` 分支、`FakeIntersectionObserver`（测试） |
| 测试重写 | `agent-messages.test.tsx` 「历史分页」5 例 + 「大列表分页」1 例；**所有按 `chat-bubble` 总数断言的用例改为按 `data-role` 分查** |
| spec delta | `desktop-agent-chat-infra`：L284 / L294 / L323 / L352 四处 `remaining` 措辞 + 「展示组件族」滚动条款 + 新增吸附 requirement/scenario |
| 版本 | `packages/desktop/package.json` → `0.6.28`（AGENT.md 版本纪律） |
| 零改动 | `desktop-standard-components`（91 行全文核对，无 message-scroller 条款）；`agent-transport.ts` / `use-agent-session.ts` / 适配层 / Rust 侧 |

## 26. 已识别的风险与既有兜底

| 风险 | 兜底 |
|---|---|
| jsdom 无布局（`clientHeight=0`、矩形全 0）→ 虚拟窗口恒空，气泡断言全变 0 | `test-setup.ts` 打桩；仓库既有 `offsetHeight` 打桩先例（`architecture-view.test.tsx:188`、`stdout-panel.test.tsx:360`） |
| 单条 50 张工具卡的超长 assistant 仍是单虚拟项 | 保留 `[content-visibility:auto]` + `[contain-intrinsic-size]` 叠加 |
| 流式增长拽动视口 | virtualizer 源码已内置此保护（`resizeItem` 首次测量才补偿折线以上项，注释直接点名 chat streaming `#1218`） |
| 长会话首帧贴底跳变 | `anchorTo:'end'` + `initialOffset` + `data-pending-scroll` 隐形首帧 |
| `max-lines-per-function` 80 行（tsx）/ lint 禁止 `as` 逃逸与 `!` | 拆 `useVirtualMessages` / `useUserTurnSpans` / `AssistantRow` 小组件 |


