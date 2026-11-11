'use client';

import { type ReactVirtualizer, useVirtualizer } from '@tanstack/react-virtual';
import { cn } from 'cn';
import { ArrowDownIcon } from 'lucide-react';
import * as React from 'react';

import { Button } from '@/components/ui/button';

/**
 * 对话透镜的滚动承载面。滚动引擎由 `@tanstack/react-virtual` 承担：上游模式机、
 * rect 扫描、`MutationObserver`、prepend 锚定与逐项 peek 随上游 primitives 一并
 * 退役；UI 面（底边渐隐 / 滚动条 / HUD 属性 / `data-slot` 钩子）逐字保留。
 *
 * 三条结构前提（违反即静默失效，见 `openspec` 的虚拟窗口 requirement）：
 * 1. 吸附层容器只用 `top` 定位、**不得**用 `transform`（transform 会另建包含块，
 *    使 sticky 的夹取基准被替换 ⇒ 吸住后永不顶走）；
 * 2. 吸附层祖先链上**最近的滚动容器**必须是 `MessageScrollerViewport`
 *    （`-content` 无 overflow 类；`-viewport` 的 `contain-content` 是
 *    `contain: layout paint`，不创建包含块）；
 * 3. `scroll-fade-b` 必须留在 `-viewport` 上（`animation-timeline: scroll(self y)`
 *    要求承载元素自身是滚动容器）。
 */

/** render prop 收到的几何账目（吸附层据此算占位块跨度） */
export interface MessageScrollerGeometry {
  /** 虚拟器测量缓存（长度恒为 `count`、按下标稠密可读；含 `paddingStart`） */
  measurements: readonly { start: number; size: number }[];
  /** 内容容器高度 = 虚拟器 `getTotalSize()` */
  totalSize: number;
}

export interface MessageScrollerProviderProps {
  /** 虚拟窗口项数（全量装载，无分页窗口） */
  count: number;
  /** 首帧估值（实测后覆盖；实测 ≤ 0 时回落本函数） */
  estimateSize: (index: number) => number;
  /** 稳定键（滚动与追加不重建测量缓存） */
  getItemKey: (index: number) => React.Key;
  /** 可见切片渲染：返回 `null` 表示该索引不产节点（高度账目照常占位） */
  renderItem: (index: number) => React.ReactNode | null;
  /** 吸附层渲染：收几何、由应用层决定渲哪些索引；条目 MUST 按 `index` 升序返回，
   * 合并器据此与可见切片单遍交织（乱序会让 DOM 顺序偏离会话顺序） */
  renderSticky?: (geometry: MessageScrollerGeometry) => MessageScrollerStickyEntry[];
  /** 贴底复位键（会话身份）：`null` = 未知、不参与换会话比较；缺省 = 调用点不提供 */
  resetKey?: string | null;
  /** 可见窗口外扩项数 */
  overscan?: number;
  /** 行距（纵向几何唯一来源之一，取代 flex `gap`） */
  gap?: number;
  /** 内容容器上内边距（纵向几何唯一来源之一，取代内容容器 CSS padding） */
  paddingStart?: number;
  /** 内容容器下内边距 */
  paddingEnd?: number;
  /** 贴底判定阈值（到底按钮与跟随即此） */
  scrollEndThreshold?: number;
  children?: React.ReactNode;
}

export interface MessageScrollerItemProps {
  /** 虚拟器索引（`data-index` 供 `measureElement` 定位） */
  index: number;
  /** 项起点（`measurementsCache[index].start`，与吸附层 `top` 同基准） */
  start: number;
  children?: React.ReactNode;
  className?: string;
}

/** 吸附层一条：`index` 只用于与可见切片交织（DOM 顺序即会话顺序） */
export interface MessageScrollerStickyEntry {
  index: number;
  node: React.ReactNode;
}

export interface MessageScrollerStickyProps {
  /** 该吸附块对应的过滤后列表索引 */
  index: number;
  /** 占位块起点（与虚拟项 `translateY` 同基准） */
  start: number;
  /** 该轮跨度 = sticky 的被夹边界（下一轮提问到达即顶走，零缓冲） */
  span: number;
  children?: React.ReactNode;
}

export interface MessageScrollerButtonProps extends Pick<
  React.ComponentProps<typeof Button>,
  'variant' | 'size' | 'render'
> {
  className?: string;
  children?: React.ReactNode;
}

type ScrollerVirtualizer = ReactVirtualizer<HTMLDivElement, HTMLDivElement>;

interface ScrollerContextValue {
  virtualizer: ScrollerVirtualizer;
  viewportRef: React.RefObject<HTMLDivElement | null>;
  /** 首帧隐形防跳（首测完成前 Viewport 不可见） */
  pendingScroll: boolean;
  scrollToEnd: () => void;
  renderItem: (index: number) => React.ReactNode | null;
  renderSticky?: (geometry: MessageScrollerGeometry) => MessageScrollerStickyEntry[];
}

const ScrollerContext = React.createContext<ScrollerContextValue | null>(null);

function useScrollerContext(): ScrollerContextValue {
  const value = React.useContext(ScrollerContext);
  if (value === null) {
    throw new Error('message-scroller 组装件必须在 MessageScrollerProvider 内使用。');
  }
  return value;
}

/**
 * 首帧与换会话贴底 + 首测完成标志：`anchorTo: 'end'` 只在选项变更且滚动元素
 * 已挂载时生效、首帧 `initialOffset` 缺省 0，故「挂载即贴底」与「换会话贴底」
 * 必须显式各跳一次。复位条件恰为二者之一：
 * ①消息集合首次非空或由空变非空；②两侧 `resetKey` 均非 `null` 且不等。
 * 判据 MUST NOT 退化成「只贴首次」的一次性闩——换会话时 count 恒 > 0，会漏掉。
 * `null` 表示会话身份未知（首轮 record 落库 / 收口重放），不参与条件②，
 * 以免把已上滑的用户拽回底部。`data-pending-scroll`（Viewport 隐形）遮住这一跳
 * ——jsdom 的 `Element` 无 `scrollTo`，该跳在测试环境静默无效、不抛错。
 */
function useScrollReset(
  virtualizer: ScrollerVirtualizer,
  count: number,
  resetKey: string | null | undefined,
): { pendingScroll: boolean; scrollToEnd: () => void } {
  const [pendingScroll, setPendingScroll] = React.useState(true);
  // `undefined` = 尚未有过键，与 `null`（未知）语义不同：首帧的 `null → 已知`
  // 不得被误判为换会话。
  const prevKeyRef = React.useRef<string | null | undefined>(undefined);
  const prevCountRef = React.useRef(0);
  const scrollToEnd = React.useCallback(() => {
    if (count > 0) virtualizer.scrollToIndex(count - 1, { align: 'end' });
  }, [count, virtualizer]);
  React.useLayoutEffect(() => {
    const prevKey = prevKeyRef.current;
    const firstNonEmpty = count > 0 && prevCountRef.current === 0;
    const sessionChanged =
      prevKey !== undefined &&
      prevKey !== null &&
      resetKey !== undefined &&
      resetKey !== null &&
      prevKey !== resetKey;
    if (firstNonEmpty || sessionChanged) scrollToEnd();
    prevKeyRef.current = resetKey;
    prevCountRef.current = count;
    setPendingScroll(false);
  }, [count, resetKey, scrollToEnd]);
  return { pendingScroll, scrollToEnd };
}

function MessageScrollerProvider({
  count,
  estimateSize,
  getItemKey,
  renderItem,
  renderSticky,
  resetKey,
  overscan = 8,
  gap = 0,
  paddingStart = 0,
  paddingEnd = 0,
  scrollEndThreshold = 24,
  children,
}: MessageScrollerProviderProps): React.JSX.Element {
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const virtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count,
    getScrollElement: () => viewportRef.current,
    estimateSize,
    getItemKey,
    overscan,
    gap,
    paddingStart,
    paddingEnd,
    scrollEndThreshold,
    anchorTo: 'end',
    followOnAppend: true,
  });
  const { pendingScroll, scrollToEnd } = useScrollReset(virtualizer, count, resetKey);
  // 每次 Provider 渲染都重建 context 值：`useVirtualizer` 靠自身重渲染通知可见
  // 范围变化，而实例是同一个可变对象——把值 memo 住会让消费件收不到滚动更新。
  const value: ScrollerContextValue = {
    virtualizer,
    viewportRef,
    pendingScroll,
    scrollToEnd,
    renderItem,
    renderSticky,
  };
  return <ScrollerContext.Provider value={value}>{children}</ScrollerContext.Provider>;
}

function MessageScroller({
  className,
  ...props
}: React.ComponentProps<'div'>): React.JSX.Element {
  return (
    <div
      data-slot="message-scroller"
      className={cn(
        'group/message-scroller relative flex size-full min-h-0 flex-col overflow-hidden',
        className,
      )}
      {...props}
    />
  );
}

function MessageScrollerViewport({
  className,
  role = 'region',
  ...props
}: React.ComponentProps<'div'>): React.JSX.Element {
  const { virtualizer, viewportRef, pendingScroll } = useScrollerContext();
  return (
    <div
      {...props}
      role={role}
      tabIndex={0}
      ref={viewportRef}
      data-slot="message-scroller-viewport"
      data-autoscrolling={virtualizer.isScrolling ? 'true' : undefined}
      data-pending-scroll={pendingScroll ? 'true' : undefined}
      className={cn(
        'size-full min-h-0 min-w-0 scroll-fade-b scrollbar-thin scrollbar-gutter-stable overflow-y-auto overscroll-contain contain-content data-autoscrolling:scrollbar-thumb-transparent data-autoscrolling:scrollbar-track-transparent data-pending-scroll:invisible',
        className,
      )}
    />
  );
}

function MessageScrollerContent({
  className,
  children,
}: {
  children?: React.ReactNode;
  className?: string;
}): React.JSX.Element {
  const { virtualizer } = useScrollerContext();
  return (
    <div
      data-slot="message-scroller-content"
      className={cn('relative', className)}
      style={{ height: virtualizer.getTotalSize() }}
    >
      {children}
      <MessageScrollerSlice />
    </div>
  );
}

/** 可见窗口切片 + 吸附层：两层的条目按索引交织，DOM 顺序与会话顺序一致
 *（文本序断言与「一轮一条气泡」的相邻性由此保持）。`renderItem` 返回 `null`
 * 的索引不产节点，但其高度账目仍参与 `count` / `estimateSize` / `getTotalSize`。
 * `renderSticky` 的契约是「条目按 `index` 升序、`index` 为过滤后列表索引」——
 * 合并只按该契约比较索引，指针越界即「吸附条目已取尽」，不伪造索引兜底。 */
function MessageScrollerSlice(): React.JSX.Element {
  const { virtualizer, renderItem, renderSticky } = useScrollerContext();
  const items = virtualizer.getVirtualItems();
  const sticky =
    renderSticky?.({
      measurements: virtualizer.measurementsCache,
      totalSize: virtualizer.getTotalSize(),
    }) ?? [];
  const stickyNode = (entry: MessageScrollerStickyEntry): React.ReactNode => (
    <React.Fragment key={`sticky-${entry.index}`}>{entry.node}</React.Fragment>
  );
  const nodes: React.ReactNode[] = [];
  // 两条序列各自升序：单遍合并即得按索引交织的节点序
  let stickyIndex = 0;
  let entry = sticky.at(stickyIndex);
  for (const item of items) {
    while (entry !== undefined && entry.index <= item.index) {
      nodes.push(stickyNode(entry));
      stickyIndex += 1;
      entry = sticky.at(stickyIndex);
    }
    const node = renderItem(item.index);
    if (node === null) continue;
    nodes.push(
      <MessageScrollerItem key={item.key} index={item.index} start={item.start}>
        {node}
      </MessageScrollerItem>,
    );
  }
  while (entry !== undefined) {
    nodes.push(stickyNode(entry));
    stickyIndex += 1;
    entry = sticky.at(stickyIndex);
  }
  return <>{nodes}</>;
}

/** 实测 ≤ 0 时回落 `estimateSize`：jsdom 无布局是常态，零高项会让列表塌陷
 * （虚拟项 MUST NOT 折叠为零高）。 */
function measureWithFallback(
  virtualizer: ScrollerVirtualizer,
  index: number,
  node: HTMLDivElement | null,
): void {
  if (node === null) return;
  if (node.getBoundingClientRect().height <= 0) {
    virtualizer.resizeItem(index, virtualizer.options.estimateSize(index));
    return;
  }
  virtualizer.measureElement(node);
}

function MessageScrollerItem({
  index,
  start,
  children,
  className,
}: MessageScrollerItemProps): React.JSX.Element {
  const { virtualizer } = useScrollerContext();
  const measure = React.useCallback(
    (node: HTMLDivElement | null) => {
      measureWithFallback(virtualizer, index, node);
    },
    [virtualizer, index],
  );
  return (
    <div
      ref={measure}
      data-slot="message-scroller-item"
      data-index={index}
      className={cn(
        'absolute top-0 left-0 w-full min-w-0 shrink-0 px-3 [contain-intrinsic-size:auto_10rem] [content-visibility:auto]',
        className,
      )}
      style={{ transform: `translateY(${start}px)` }}
    >
      {children}
    </div>
  );
}

/** 吸附层高度回填：`ResizeObserver` 住本件（它知道 `index`，也从 context 拿得到
 * `resizeItem`）；测量对象是内层 sticky 容器的 `borderBoxSize`——吸顶时量到的仍是
 * 布局盒高度。该字段缺席（条目未填 / 空数组）或实测 ≤ 0 时同样回落 `estimateSize`。 */
function useStickyMeasure(index: number): (node: HTMLDivElement | null) => void {
  const { virtualizer } = useScrollerContext();
  const nodeRef = React.useRef<HTMLDivElement | null>(null);
  const setNode = React.useCallback((node: HTMLDivElement | null) => {
    nodeRef.current = node;
  }, []);
  React.useEffect(() => {
    const node = nodeRef.current;
    if (node === null || typeof ResizeObserver !== 'function') return;
    const apply = (height: number): void => {
      virtualizer.resizeItem(
        index,
        height > 0 ? height : virtualizer.options.estimateSize(index),
      );
    };
    // `borderBoxSize` 是可选填充面（部分实现不填、亦可能为空数组）：整条链路缺席时
    // 回落实测高度——`<= 0`（jsdom 无布局）再由 `apply` 回落到 `estimateSize`。
    const observer = new ResizeObserver((entries) => {
      const blockSize = entries[0]?.borderBoxSize?.[0]?.blockSize;
      apply(blockSize ?? node.getBoundingClientRect().height);
    });
    observer.observe(node);
    // 首帧在观察器首报之前先量一次：无布局环境（jsdom）由此得到与估值一致的账目
    apply(node.getBoundingClientRect().height);
    return () => observer.disconnect();
  }, [index, virtualizer]);
  return setNode;
}

function MessageScrollerSticky({
  index,
  start,
  span,
  children,
}: MessageScrollerStickyProps): React.JSX.Element {
  const measure = useStickyMeasure(index);
  return (
    <div
      data-slot="message-scroller-sticky"
      className="z-10"
      style={{
        position: 'absolute',
        top: start,
        left: 0,
        width: '100%',
        height: span,
        pointerEvents: 'none',
      }}
    >
      <div
        ref={measure}
        style={{ position: 'sticky', top: 0 }}
        className="pointer-events-auto bg-card px-3"
      >
        {children}
      </div>
    </div>
  );
}

function MessageScrollerButton({
  className,
  children,
  render,
  variant = 'secondary',
  size = 'icon-sm',
}: MessageScrollerButtonProps): React.JSX.Element {
  const { virtualizer, scrollToEnd } = useScrollerContext();
  const active = virtualizer.getDistanceFromEnd() > virtualizer.options.scrollEndThreshold;
  return (
    <Button
      type="button"
      data-slot="message-scroller-button"
      data-direction="end"
      data-variant={variant}
      data-size={size}
      data-active={active ? 'true' : 'false'}
      tabIndex={active ? 0 : -1}
      variant={variant}
      size={size}
      render={render}
      onClick={scrollToEnd}
      className={cn(
        'absolute inset-s-1/2 -translate-x-1/2 border-border bg-background text-foreground transition-[translate,scale,opacity] duration-200 hover:bg-muted hover:text-foreground data-[active=false]:pointer-events-none data-[active=false]:scale-95 data-[active=false]:opacity-0 data-[active=false]:duration-400 data-[active=false]:ease-[cubic-bezier(0.7,0,0.84,0)] data-[active=true]:translate-y-0 data-[active=true]:scale-100 data-[active=true]:opacity-100 data-[active=true]:ease-[cubic-bezier(0.23,1,0.32,1)] data-[direction=end]:bottom-4 data-[direction=end]:data-[active=false]:translate-y-full rtl:translate-x-1/2',
        className,
      )}
    >
      {children ?? (
        <>
          <ArrowDownIcon />
          <span className="sr-only">Scroll to end</span>
        </>
      )}
    </Button>
  );
}

export {
  MessageScrollerProvider,
  MessageScroller,
  MessageScrollerViewport,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerSticky,
  MessageScrollerButton,
};
