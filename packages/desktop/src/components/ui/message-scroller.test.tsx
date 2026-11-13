import { act, fireEvent, render, waitFor } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import * as scrollerModule from '@/components/ui/message-scroller';
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  type MessageScrollerProviderProps,
  MessageScrollerSticky,
  MessageScrollerViewport,
} from '@/components/ui/message-scroller';

// ---------------------------------------------------------------------------
// 滚动承载 shim 的结构契约：Provider 入参通道与 context、五个既有 data-slot 钩子
// + 新增吸附层钩子、Viewport 类名与 HUD 属性、Content 几何、Item 零测量回落、
// Sticky 的 top 定位与高度回填、Button 的距底判据、导出面收窄。
// `@tanstack/react-virtual` 为真实实现（账目 / range / 测量缓存是被测输出本体）；
// 全局前提是 test-setup.ts 的视口尺寸桩（clientHeight = scrollHeight = 600）。
// ---------------------------------------------------------------------------

type ResizeEntries = { borderBoxSize?: { blockSize: number }[] }[];

/** 可编程 ResizeObserver：捕获回调与目标，由用例手动投 boxSize */
class ProgrammableResizeObserver {
  public static instances: ProgrammableResizeObserver[] = [];

  public readonly targets: Element[] = [];

  constructor(private readonly callback: (entries: ResizeEntries) => void) {
    ProgrammableResizeObserver.instances.push(this);
  }

  public observe(target: Element): void {
    this.targets.push(target);
  }

  public unobserve(): void {}

  public disconnect(): void {}

  public trigger(blockSize: number): void {
    this.callback([{ borderBoxSize: [{ blockSize }] }]);
  }

  /** 投一条不含 `borderBoxSize` 的条目：该字段是可选填充面，部分运行时不填 */
  public triggerWithoutBoxSize(): void {
    this.callback([{}]);
  }

  /** 投一条 `borderBoxSize` 为空数组的条目 */
  public triggerEmptyBoxSize(): void {
    this.callback([{ borderBoxSize: [] }]);
  }
}

/** 观察吸附层内层（含 `position: sticky` 内联样式）的那个替身实例 */
function stickyObserver(): ProgrammableResizeObserver | undefined {
  return ProgrammableResizeObserver.instances.find((instance) =>
    instance.targets.some((target) => target.closest('[data-slot="message-scroller-sticky"]') !== null),
  );
}

function contentOf(container: HTMLElement): HTMLElement {
  const node = container.querySelector('[data-slot="message-scroller-content"]');
  if (node === null) throw new Error('缺少 message-scroller-content');
  return node as HTMLElement;
}

function viewportOf(container: HTMLElement): HTMLElement {
  const node = container.querySelector('[data-slot="message-scroller-viewport"]');
  if (node === null) throw new Error('缺少 message-scroller-viewport');
  return node as HTMLElement;
}

function itemNodes(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>('[data-slot="message-scroller-item"]')];
}

function itemIndexes(container: HTMLElement): (string | null)[] {
  return itemNodes(container).map((node) => node.getAttribute('data-index'));
}

function translateYs(container: HTMLElement): number[] {
  return itemNodes(container).map((node) => {
    const transform = node.style.transform;
    const matched = /translateY\((-?[\d.]+)px\)/.exec(transform);
    if (matched === null) throw new Error(`缺少 translateY: ${transform}`);
    return Number(matched[1]);
  });
}

function styleOf(container: HTMLElement, slot: string): CSSStyleDeclaration {
  const node = container.querySelector<HTMLElement>(`[data-slot="${slot}"]`);
  if (node === null) throw new Error(`缺少 ${slot}`);
  return node.style;
}

interface FrameProps extends Partial<Omit<MessageScrollerProviderProps, 'children'>> {
  children?: React.ReactNode;
}

/** 视口写入面探针句柄：jsdom 的 `Element` 无 `scrollTo`、`scrollTop` 不写回 */
interface ViewportProbe {
  top: () => number;
  setTop: (value: number) => void;
  writes: number[];
}

/** 在自身 layout effect（先于 Provider 的复位 effect）内按实例装最小桩：
 * 只写 `scrollTop`、不派发 `scroll` 事件。 */
function ScrollProbe({
  scrollHeight,
  probe,
}: {
  scrollHeight: number;
  probe: { current: ViewportProbe | null };
}): null {
  React.useLayoutEffect(() => {
    const viewport = document.querySelector<HTMLElement>('[data-slot="message-scroller-viewport"]');
    if (viewport === null) return;
    let top = 0;
    const writes: number[] = [];
    Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: scrollHeight });
    Object.defineProperty(viewport, 'scrollTop', {
      configurable: true,
      get: () => top,
      set: (value: number) => {
        top = value;
      },
    });
    Object.defineProperty(viewport, 'scrollTo', {
      configurable: true,
      value: (options: { top?: number }) => {
        const next = options.top ?? 0;
        writes.push(next);
        top = next;
      },
    });
    probe.current = {
      top: () => top,
      setTop: (value: number) => {
        top = value;
      },
      writes,
    };
  }, [scrollHeight, probe]);
  return null;
}

/** 视口「可滚动」形态：`maxScroll = scrollHeight − clientHeight`（原型桩给 600） */
const PIN_VIEWPORT_SCROLL_HEIGHT = 1200;

/** `align:'end'` 的末项目标值：`clamp(maxScroll − paddingEnd, 0, maxScroll)` */
function endTarget(scrollHeight = PIN_VIEWPORT_SCROLL_HEIGHT, paddingEnd = 0): number {
  const maxScroll = Math.max(scrollHeight - 600, 0);
  return Math.max(Math.min(maxScroll - paddingEnd, maxScroll), 0);
}

/** 复位 / 不复位两组用例的共用装置：可滚动视口 + 写入探针 */
function renderProbed(props: FrameProps): {
  container: HTMLElement;
  probe: () => ViewportProbe;
  rerender: (next: FrameProps) => void;
} {
  const ref: { current: ViewportProbe | null } = { current: null };
  const view = render(
    <Frame {...props}>
      <ScrollProbe scrollHeight={PIN_VIEWPORT_SCROLL_HEIGHT} probe={ref} />
    </Frame>,
  );
  const probe = (): ViewportProbe => {
    if (ref.current === null) throw new Error('视口桩未装上');
    return ref.current;
  };
  return {
    container: view.container,
    probe,
    rerender: (next: FrameProps) => {
      view.rerender(
        <Frame {...next}>
          <ScrollProbe scrollHeight={PIN_VIEWPORT_SCROLL_HEIGHT} probe={ref} />
        </Frame>,
      );
    },
  };
}

/** 纯数据驱动的组装件：项内容只带 `data-probe`，与领域类型无关 */
function Frame({ children, ...props }: FrameProps): React.JSX.Element {
  return (
    <MessageScrollerProvider
      count={3}
      estimateSize={() => 100}
      getItemKey={(index) => `key-${index}`}
      renderItem={(index) => <span data-probe={index}>{index}</span>}
      {...props}
    >
      <MessageScroller>
        <MessageScrollerViewport aria-label="对话消息">
          <MessageScrollerContent>{children}</MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
    </MessageScrollerProvider>
  );
}

afterEach(() => {
  ProgrammableResizeObserver.instances = [];
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// Provider：入参通道、切片渲染、context 契约
// ---------------------------------------------------------------------------

describe('MessageScrollerProvider：虚拟窗口入参通道', () => {
  it('正向：count 为 100 时只渲窗口切片（项数 ≪ 100 且 > 0），不随 count 线性增长', () => {
    const { container } = render(<Frame count={100} />);

    const rendered = itemNodes(container);
    expect(rendered.length).toBeGreaterThan(0);
    expect(rendered.length).toBeLessThan(100);
    expect(styleOf(container, 'message-scroller-content').height).toBe('10000px');
    expect(rendered.every((node) => node.querySelector('[data-probe]') !== null)).toBe(true);

    const smaller = render(<Frame count={20} />);
    expect(itemNodes(smaller.container).length).toBeLessThanOrEqual(rendered.length);

    const huge = render(<Frame count={1000} />);
    const hugeItems = itemNodes(huge.container);
    expect(hugeItems.length).toBeGreaterThan(0);
    expect(hugeItems.length).toBeLessThan(200);
  });

  it('正向：count 为 0 → 零 item、children 照常渲出、renderSticky 收到 totalSize 0', () => {
    const totalSizes: number[] = [];
    const { container } = render(
      <Frame count={0} renderSticky={(geometry) => (totalSizes.push(geometry.totalSize), [])}>
        <div data-testid="empty-block" />
      </Frame>,
    );

    expect(itemNodes(container)).toHaveLength(0);
    expect(container.querySelector('[data-testid="empty-block"]')).not.toBeNull();
    expect(styleOf(container, 'message-scroller-content').height).toBe('0px');
    expect(totalSizes.at(-1)).toBe(0);
  });

  it('正向：renderItem 对部分索引返回 null → 这些索引在 DOM 零节点，其余照常成节点', () => {
    const { container } = render(
      <Frame count={10} renderItem={(index) => (index % 2 === 0 ? null : <span data-probe={index} />)} />,
    );

    const indexes = itemIndexes(container);
    expect(indexes.length).toBeGreaterThan(0);
    // 被 null 的索引不产空壳节点：留下的项序全为奇数
    expect(indexes.every((index) => Number(index) % 2 === 1)).toBe(true);
    expect(itemNodes(container).every((node) => node.querySelector('[data-probe]') !== null)).toBe(true);
    // 账目照常占位：被 null 的索引仍参与 count / estimateSize
    expect(styleOf(container, 'message-scroller-content').height).toBe('1000px');
  });

  it('正向：renderSticky 收到稠密 measurements 与等于 Content 高度的 totalSize；缺席时不抛错', () => {
    const seen: { length: number; totalSize: number }[] = [];
    const { container } = render(
      <Frame
        count={4}
        gap={24}
        paddingStart={12}
        paddingEnd={12}
        renderSticky={(geometry) => {
          seen.push({ length: geometry.measurements.length, totalSize: geometry.totalSize });
          return [];
        }}
      />,
    );

    const height = Number.parseFloat(styleOf(container, 'message-scroller-content').height);
    expect(seen.at(-1)?.length).toBe(4);
    expect(seen.at(-1)?.totalSize).toBe(height);
    expect(height).toBe(12 + 4 * 100 + 3 * 24 + 12);

    const bare = render(<Frame count={2} renderSticky={undefined} />);
    expect(itemNodes(bare.container).length).toBe(2);
  });

  it('正向：context 契约 —— 六个组装件在 Provider 内全部可渲染并由实例派生属性', () => {
    const { container } = render(<Frame count={5} />);

    expect(container.querySelector('[data-slot="message-scroller"]')).not.toBeNull();
    expect(container.querySelector('[data-slot="message-scroller-viewport"]')).not.toBeNull();
    expect(styleOf(container, 'message-scroller-content').height).toBe('500px');
    expect(itemNodes(container).length).toBeGreaterThan(0);
    const button = container.querySelector('[data-slot="message-scroller-button"]');
    expect(button?.getAttribute('data-active')).toBe('false');
    expect(viewportOf(container).getAttribute('aria-label')).toBe('对话消息');
  });

  it('异常：Content / Button 在 Provider 外使用 → 抛明确错误，不静默渲出零高容器', () => {
    const cases: [string, React.JSX.Element][] = [
      ['Content', <MessageScrollerContent />],
      ['Button', <MessageScrollerButton />],
    ];

    for (const [label, element] of cases) {
      expect(() => render(element), label).toThrow(/MessageScrollerProvider/);
    }
  });

  it('边界：不传 overscan / gap / paddingStart / paddingEnd / scrollEndThreshold → 缺省装配不抛错', () => {
    const { container } = render(<Frame count={3} estimateSize={() => 100} />);

    // 缺省 overscan 8 + 视口 600 ⇒ 序号 0..(600/100 + 8) 内的项
    expect(itemIndexes(container)).toEqual(['0', '1', '2']);
    // 缺省 gap / padding 均为 0 ⇒ 高度 = count × estimate
    expect(styleOf(container, 'message-scroller-content').height).toBe('300px');
    expect(container.querySelector('[data-slot="message-scroller-button"]')?.getAttribute('data-active')).toBe('false');
  });

  it('边界：显式 gap 24 + padding 12 / 12 → totalSize 比同 fixture 缺省形态多 24 × (count − 1) + 24', () => {
    const plain = render(<Frame count={5} estimateSize={() => 100} />);
    const plainHeight = Number.parseFloat(styleOf(plain.container, 'message-scroller-content').height);
    plain.unmount();

    const padded = render(<Frame count={5} estimateSize={() => 100} gap={24} paddingStart={12} paddingEnd={12} />);
    const paddedHeight = Number.parseFloat(styleOf(padded.container, 'message-scroller-content').height);

    expect(plainHeight).toBe(500);
    expect(paddedHeight - plainHeight).toBe(24 * (5 - 1) + 24);
  });
});

// ---------------------------------------------------------------------------
// 贴底复位时机（AC-13）：首帧 / 由空变非空 / 换会话各跳一次，
// 同会话追加与键单侧为 null 均不跳（上滑用户不被拽走）
// ---------------------------------------------------------------------------

describe('MessageScrollerProvider：贴底复位时机', () => {
  it('正向：首帧挂载（count 非零）→ 视口贴到末端目标值', () => {
    const { probe } = renderProbed({ count: 3 });

    expect(probe().writes.length).toBeGreaterThan(0);
    expect(probe().top()).toBe(endTarget());
  });

  it('正向：由空变非空（count 0 → 3）→ 落到同一个末端目标值', () => {
    const { probe, rerender } = renderProbed({ count: 0 });
    // 空列表：只发生虚拟器挂载时的自身同步写入（offset 0），无贴底跳转
    expect(probe().top()).toBe(0);

    rerender({ count: 3 });

    expect(probe().writes.length).toBeGreaterThan(0);
    expect(probe().top()).toBe(endTarget());
  });

  it('正向：换会话（resetKey A → B，count 不变）→ 从别处复位回末端', () => {
    const { probe, rerender } = renderProbed({ count: 3, resetKey: 'A' });
    expect(probe().top()).toBe(endTarget());

    act(() => {
      probe().setTop(0);
    });
    rerender({ count: 3, resetKey: 'B' });

    expect(probe().top()).toBe(endTarget());
  });

  it('异常：同会话追加（resetKey 恒 A、count 3 → 5）→ 不把视口拽回末端', () => {
    const { probe, rerender } = renderProbed({ count: 3, resetKey: 'A' });

    act(() => {
      probe().setTop(0);
    });
    rerender({ count: 5, resetKey: 'A' });

    expect(probe().top()).not.toBe(endTarget());
  });

  it('异常：复位键单侧为 null（null → A 与 A → null）→ 两种变化下 scrollTop 均不变', () => {
    const cases: [string, string | null, string | null][] = [
      ['首轮 record 落库', null, 'A'],
      ['收口重放', 'A', null],
    ];

    for (const [label, before, after] of cases) {
      const { probe, rerender } = renderProbed({ count: 3, resetKey: before });
      act(() => {
        probe().setTop(17);
      });

      rerender({ count: 3, resetKey: after });

      // 条件②要求两侧均非 null；count 不变故连端锚定补偿都不发生
      expect(probe().top(), label).toBe(17);
    }
  });

  it('边界：resetKey 缺省（undefined）→ 首帧仍贴底，随后的增长不再拽回末端', () => {
    const { probe, rerender } = renderProbed({ count: 3 });
    expect(probe().top()).toBe(endTarget());

    act(() => {
      probe().setTop(0);
    });
    rerender({ count: 5 });

    // undefined = 尚未有过键，不参与条件②
    expect(probe().top()).not.toBe(endTarget());
  });

  it('边界：始终空（count 0 → 0）→ 零写入、不抛错（count − 1 不得为负）', () => {
    const { probe, rerender } = renderProbed({ count: 0 });

    expect(() => rerender({ count: 0 })).not.toThrow();
    // count − 1 的下标不得为负：无贴底跳转，视口不动
    expect(probe().top()).toBe(0);
  });

  it('边界：渲染序列 A → A → B（count 不变）→ 键不变时零复位、变键时落到末端', () => {
    const { probe, rerender } = renderProbed({ count: 3, resetKey: 'A' });

    act(() => {
      probe().setTop(17);
    });
    rerender({ count: 3, resetKey: 'A' });
    expect(probe().top()).toBe(17);

    rerender({ count: 3, resetKey: 'B' });
    expect(probe().top()).toBe(endTarget());
  });

  it('异常：resetKey 不进虚拟器选项 —— 仅换键不改变各项起点与 measurements', () => {
    const { rerender, probe, container } = renderProbed({
      count: 6,
      estimateSize: () => 60,
      resetKey: 'A',
    });
    const startsOf = (): { index: string | null; transform: string }[] =>
      itemNodes(container).map((node) => ({
        index: node.getAttribute('data-index'),
        transform: node.style.transform,
      }));
    const before = startsOf();
    const totalBefore = styleOf(container, 'message-scroller-content').height;

    rerender({ count: 6, estimateSize: () => 60, resetKey: 'B' });

    // 键未触发 setOptions 的端锚定重算：几何逐项不变（只断几何，不断写入次数）
    expect(startsOf()).toEqual(before);
    expect(styleOf(container, 'message-scroller-content').height).toBe(totalBefore);
    // 复位本身仍发生（落到末端），与几何不变并不矛盾
    expect(probe().top()).toBe(endTarget());
  });
});

// ---------------------------------------------------------------------------
// 根容器：类名逐字与结构前提 ③ 的落点判据
// ---------------------------------------------------------------------------

describe('MessageScroller：滚动承载根容器', () => {
  it('正向：data-slot 钩子与类名逐字保留，className 入参叠加在后', () => {
    const { container } = render(<MessageScroller className="custom-root" />);
    const root = container.querySelector('[data-slot="message-scroller"]') as HTMLElement;

    for (const className of [
      'group/message-scroller',
      'relative',
      'flex',
      'size-full',
      'min-h-0',
      'flex-col',
      'overflow-hidden',
    ]) {
      expect(root.className, className).toContain(className);
    }
    expect(root.className).toContain('custom-root');
  });

  it('异常：overflow-hidden 只在根容器上，Content 与吸附层 MUST NOT 携带任何 overflow 类', () => {
    const { container } = render(
      <Frame
        count={2}
        renderSticky={() => [
          {
            index: 0,
            node: (
              <MessageScrollerSticky index={0} start={0} span={100}>
                <span>sticky</span>
              </MessageScrollerSticky>
            ),
          },
        ]}
      />,
    );

    expect(container.querySelector('[data-slot="message-scroller"]')?.className).toContain('overflow-hidden');
    for (const slot of ['message-scroller-content', 'message-scroller-sticky']) {
      const className = container.querySelector(`[data-slot="${slot}"]`)?.className ?? '';
      expect(className, slot).not.toContain('overflow-hidden');
      expect(className, slot).not.toContain('overflow-auto');
      expect(className, slot).not.toContain('overflow-y-auto');
    }
    // 链上唯一携带 overflow-y-auto 的元素是 viewport
    expect(container.querySelector('[data-slot="message-scroller-viewport"]')?.className).toContain(
      'overflow-y-auto',
    );
  });

  it('边界：不入 Provider 直接渲染（纯 div、零 context 依赖）→ 正常渲出、不抛错', () => {
    expect(() => render(<MessageScroller />)).not.toThrow();
    const { container } = render(<MessageScroller />);
    expect(container.querySelector('[data-slot="message-scroller"]')).not.toBeNull();
  });

  it('边界：className 缺席 / 空串 / 超长类名串 → 缺省类名清单不受损、不抛错', () => {
    const long = Array.from({ length: 200 }, (_, index) => `x-${index}`).join(' ');

    for (const className of [undefined, '', long]) {
      const { container, unmount } = render(<MessageScroller className={className} />);
      const node = container.querySelector('[data-slot="message-scroller"]') as HTMLElement;
      expect(node.className, String(className?.length)).toContain('group/message-scroller');
      expect(node.className, String(className?.length)).toContain('overflow-hidden');
      unmount();
    }
  });
});

// ---------------------------------------------------------------------------
// 视口：类名清单与 HUD 属性（条件输出）
// ---------------------------------------------------------------------------

describe('MessageScrollerViewport：类名与 HUD 属性', () => {
  it('正向：底边渐隐 / 滚动条 / 惯性与 containment 类名逐字保留，data-slot 在场', () => {
    const { container } = render(<Frame count={3} />);
    const viewport = viewportOf(container);

    for (const className of [
      'scroll-fade-b',
      'scrollbar-thin',
      'scrollbar-gutter-stable',
      'overflow-y-auto',
      'overscroll-contain',
      'contain-content',
      'data-autoscrolling:scrollbar-thumb-transparent',
      'data-pending-scroll:invisible',
    ]) {
      expect(viewport.className, className).toContain(className);
    }
    expect(viewport.getAttribute('data-slot')).toBe('message-scroller-viewport');
  });

  it('正向：调用点透传的 aria-label 落在视口上；shim 独占 ref（视口尺寸桩命中即证）', () => {
    const { container } = render(<Frame count={3} />);
    const viewport = viewportOf(container);

    expect(viewport.getAttribute('aria-label')).toBe('对话消息');
    // test-setup.ts 的原型 getter 按 data-slot 收窄 ⇒ 600 说明 ref 已挂到该元素
    expect(viewport.clientHeight).toBe(600);
    expect(viewport.scrollHeight).toBe(600);
  });

  it('正向：可达性对齐 —— role 缺省为 region、tabIndex 为 0，调用点显式 role 覆盖生效', () => {
    const { container } = render(<Frame count={3} />);
    const viewport = viewportOf(container);

    expect(viewport.getAttribute('role')).toBe('region');
    expect(viewport.getAttribute('tabindex')).toBe('0');

    const custom = render(
      <MessageScrollerProvider
        count={1}
        estimateSize={() => 100}
        getItemKey={(index) => index}
        renderItem={() => null}
      >
        <MessageScrollerViewport role="log" />
      </MessageScrollerProvider>,
    );
    // 调用点的 role 不被吞掉（缺省值可覆盖）
    expect(viewportOf(custom.container).getAttribute('role')).toBe('log');
    expect(viewportOf(custom.container).getAttribute('tabindex')).toBe('0');
  });

  it('正向：data-pending-scroll 生命周期 —— flush 前为 "true"，effect 完成后属性不落', () => {
    const during: (string | null)[] = [];

    function Probe(): null {
      React.useLayoutEffect(() => {
        during.push(
          document
            .querySelector('[data-slot="message-scroller-viewport"]')
            ?.getAttribute('data-pending-scroll') ?? null,
        );
      }, []);
      return null;
    }

    const { container } = render(
      <Frame count={3}>
        <Probe />
      </Frame>,
    );

    expect(during).toEqual(['true']);
    // effect 完成后属性消失（undefined 而非 "false"）
    expect(viewportOf(container).hasAttribute('data-pending-scroll')).toBe(false);
  });

  it('正向：data-autoscrolling 随滚动置 "true"，滚动收尾后属性不再落', async () => {
    const { container } = render(<Frame count={40} />);
    const viewport = viewportOf(container);

    act(() => {
      fireEvent.scroll(viewport);
    });
    expect(viewport.getAttribute('data-autoscrolling')).toBe('true');

    await waitFor(() => expect(viewport.hasAttribute('data-autoscrolling')).toBe(false), {
      timeout: 5000,
    });
  });

  it('异常：两个 HUD 属性 MUST NOT 取 "false"（存在性变体，避免常亮）', () => {
    const { container } = render(<Frame count={3} />);
    const viewport = viewportOf(container);

    for (const attribute of ['data-pending-scroll', 'data-autoscrolling']) {
      expect(viewport.getAttribute(attribute), attribute).not.toBe('false');
    }
  });

  it('边界：count 为 0（内容不足一屏）→ 挂载后两个 HUD 属性均不落', () => {
    const { container } = render(<Frame count={0} />);
    const viewport = viewportOf(container);

    expect(viewport.hasAttribute('data-pending-scroll')).toBe(false);
    expect(viewport.hasAttribute('data-autoscrolling')).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 内容容器：内联 height 与纵向几何单一来源
// ---------------------------------------------------------------------------

describe('MessageScrollerContent：几何账目', () => {
  it('正向：内联 height 等于 getTotalSize()，类名不留 flex / h-max / min-h-full / gap-6 / p-3', () => {
    const { container } = render(<Frame count={3} estimateSize={() => 100} />);
    const content = contentOf(container);

    expect(content.style.height).toBe('300px');
    for (const forbidden of ['flex', 'h-max', 'min-h-full', 'gap-6', 'p-3']) {
      expect(content.className, forbidden).not.toContain(forbidden);
    }
    expect(content.className).toContain('relative');
  });

  it('正向：渲染次序 —— children 先、虚拟切片居中、吸附层索引大于全部可见项时落在末位', () => {
    const { container } = render(
      <Frame
        count={3}
        renderSticky={() => [
          {
            index: 5,
            node: (
              <MessageScrollerSticky index={5} start={0} span={10}>
                <span>sticky</span>
              </MessageScrollerSticky>
            ),
          },
        ]}
      >
        <div data-testid="content-child" />
      </Frame>,
    );

    const children = [...contentOf(container).children];
    const slots = children.map((node) =>
      node.getAttribute('data-slot') ?? node.getAttribute('data-testid'),
    );
    expect(slots[0]).toBe('content-child');
    expect(slots.slice(1, -1).every((slot) => slot === 'message-scroller-item')).toBe(true);
    expect(slots.at(-1)).toBe('message-scroller-sticky');
  });

  it('正向：吸附层条目按索引与可见切片交织（DOM 顺序即会话顺序）', () => {
    const { container } = render(
      <Frame
        count={3}
        renderItem={(index) => <span data-probe={index} />}
        renderSticky={() => [
          {
            index: 1,
            node: (
              <MessageScrollerSticky index={1} start={100} span={100}>
                <span>sticky</span>
              </MessageScrollerSticky>
            ),
          },
        ]}
      />,
    );

    const slots = [...contentOf(container).children].map(
      (node) => `${node.getAttribute('data-slot')}:${node.getAttribute('data-index') ?? '-'}`,
    );
    expect(slots).toEqual([
      'message-scroller-item:0',
      'message-scroller-sticky:-',
      'message-scroller-item:1',
      'message-scroller-item:2',
    ]);
  });

  it('异常：renderItem 全返回 null → 零 item 节点，height 仍为全量 getTotalSize()', () => {
    const { container } = render(<Frame count={5} renderItem={() => null} gap={24} />);

    expect(itemNodes(container)).toHaveLength(0);
    expect(styleOf(container, 'message-scroller-content').height).toBe(`${500 + 4 * 24}px`);
  });

  it('边界：count 为 0 且传 children（空态块宿主面）→ height 为 0、children 在 DOM、不抛错', () => {
    const { container } = render(
      <Frame count={0}>
        <div data-testid="empty-block">尚无对话</div>
      </Frame>,
    );

    expect(styleOf(container, 'message-scroller-content').height).toBe('0px');
    expect(container.querySelector('[data-testid="empty-block"]')?.textContent).toBe('尚无对话');
  });
});

// ---------------------------------------------------------------------------
// 虚拟项：绝对定位、translateY 与零测量回落
// ---------------------------------------------------------------------------

describe('MessageScrollerItem：定位与零测量回落', () => {
  it('正向：绝对定位 + translateY(start) + data-index；保留类名与行级横向 gutter', () => {
    const { container } = render(<Frame count={3} />);
    const item = itemNodes(container)[0] as HTMLElement;

    expect(item.style.transform).toBe('translateY(0px)');
    expect(item.getAttribute('data-index')).toBe('0');
    for (const className of [
      'absolute',
      'top-0',
      'left-0',
      'w-full',
      '[content-visibility:auto]',
      '[contain-intrinsic-size:auto_10rem]',
      'min-w-0',
    ]) {
      expect(item.className, className).toContain(className);
    }
  });

  it('正向：正常测量路径 —— 项级实测 88 计入账目，后续项起点与 totalSize 随之改变', () => {
    // 作用面收窄的项级桩：只有 message-scroller-item 命中时返 88，其余回落原生
    const originalRect = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = function measuredRect(this: Element): DOMRect {
      if (this.getAttribute('data-slot') === 'message-scroller-item') {
        return { height: 88, width: 0, top: 0, left: 0, bottom: 88, right: 0, x: 0, y: 0 } as DOMRect;
      }
      return originalRect.call(this);
    };
    // offsetHeight 的属主是 HTMLElement.prototype（Element.prototype 上的定义会被它遮蔽）
    const originalOffset = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
      configurable: true,
      get(this: Element): number {
        if (this.getAttribute('data-slot') === 'message-scroller-item') return 88;
        return originalOffset?.get?.call(this) ?? 0;
      },
    });

    try {
      const { container } = render(<Frame count={3} estimateSize={() => 100} />);

      // 实测 88 过零测量护栏后走原生 measureElement：估值被实测取代
      expect(translateYs(container)).toEqual([0, 88, 176]);
      expect(styleOf(container, 'message-scroller-content').height).toBe('264px');
    } finally {
      Element.prototype.getBoundingClientRect = originalRect;
      if (originalOffset === undefined) {
        delete (HTMLElement.prototype as { offsetHeight?: number }).offsetHeight;
      } else {
        Object.defineProperty(HTMLElement.prototype, 'offsetHeight', originalOffset);
      }
    }
  });

  it('异常：零测量回落 —— 实测高度为 0 时按 estimateSize 计账，后续项 transform 连续递增', () => {
    const { container } = render(<Frame count={6} estimateSize={(index) => 60 + index * 10} />);

    const starts = translateYs(container);
    expect(starts.length).toBeGreaterThan(1);
    for (let index = 1; index < starts.length; index += 1) {
      // 每项高度回落为 estimateSize(index − 1)，MUST NOT 折叠为零高
      expect((starts[index] ?? 0) - (starts[index - 1] ?? 0)).toBe(60 + (index - 1) * 10);
    }
    expect(styleOf(container, 'message-scroller-content').height).not.toBe('0px');
  });

  it('边界：start 三形态（0 / 12 / 极大值）→ translateY 逐字输出、不四舍五入不夹取', () => {
    const cases: [number, string][] = [
      [0, 'translateY(0px)'],
      [12, 'translateY(12px)'],
      [Number.MAX_SAFE_INTEGER, `translateY(${Number.MAX_SAFE_INTEGER}px)`],
    ];

    for (const [start, expected] of cases) {
      const { container, unmount } = render(
        <MessageScrollerProvider count={0} estimateSize={() => 100} getItemKey={(index) => index} renderItem={() => null}>
          <MessageScrollerItem index={0} start={start}>
            <span>item</span>
          </MessageScrollerItem>
        </MessageScrollerProvider>,
      );
      expect(styleOf(container, 'message-scroller-item').transform, String(start)).toBe(expected);
      unmount();
    }
  });
});

// ---------------------------------------------------------------------------
// 吸附层：top 定位（禁 transform）、内层 sticky、span 高度与 ResizeObserver 回填
// ---------------------------------------------------------------------------

describe('MessageScrollerSticky：吸附层结构契约', () => {
  function renderSticky(span: number, start = 12): ReturnType<typeof render> {
    return render(
      <MessageScrollerProvider
        count={2}
        estimateSize={() => 100}
        getItemKey={(index) => index}
        renderItem={() => null}
        renderSticky={() => [
          {
            index: 0,
            node: (
              <MessageScrollerSticky index={0} start={start} span={span}>
                <span data-testid="sticky-body">提问</span>
              </MessageScrollerSticky>
            ),
          },
        ]}
      >
        <MessageScrollerContent />
      </MessageScrollerProvider>,
    );
  }

  it('正向：外层 absolute + top 定位 + 占位高度 + pointer-events none；内层 sticky top 0', () => {
    const { container } = renderSticky(180, 12);
    const outer = styleOf(container, 'message-scroller-sticky');
    const inner = container.querySelector<HTMLElement>('[data-slot="message-scroller-sticky"] > div');

    expect(outer.position).toBe('absolute');
    expect(outer.top).toBe('12px');
    expect(outer.left).toBe('0px');
    expect(outer.width).toBe('100%');
    expect(outer.height).toBe('180px');
    expect(outer.pointerEvents).toBe('none');
    expect(inner?.style.position).toBe('sticky');
    expect(inner?.style.top).toBe('0px');
  });

  it('正向：内层底衬与可交互类名在场；外层零渐变 / 有阴影 / 零提示类名', () => {
    const { container } = renderSticky(100);
    const outer = container.querySelector('[data-slot="message-scroller-sticky"]') as HTMLElement;
    const inner = outer.querySelector('div') as HTMLElement;

    for (const className of ['pointer-events-auto']) {
      expect(inner.className, className).toContain(className);
    }
    // 与同轮 assistant 正文同层遮挡（z-index 不创建包含块）
    expect(outer.className).toContain('z-10');
    for (const forbidden of ['bg-gradient', 'from-', 'to-']) {
      expect(outer.className, forbidden).not.toContain(forbidden);
      expect(inner.className, forbidden).not.toContain(forbidden);
    }
  });

  it('异常：吸附层自身与内层均无 transform（另建包含块会让气泡永不顶走）', () => {
    const { container } = renderSticky(100);
    const outer = container.querySelector<HTMLElement>('[data-slot="message-scroller-sticky"]');
    const inner = outer?.querySelector<HTMLElement>('div');

    expect(outer?.style.getPropertyValue('transform')).toBe('');
    expect(inner?.style.getPropertyValue('transform')).toBe('');
    for (const node of [outer, inner]) {
      const keys = node?.getAttribute('style') ?? '';
      expect(keys.toLowerCase(), String(node?.className)).not.toContain('transform');
    }
  });

  it('异常：span 小于内容高度 → 外层 height 逐字等于 span（零缓冲、不扣 gutter）', () => {
    const { container } = renderSticky(37);
    expect(styleOf(container, 'message-scroller-sticky').height).toBe('37px');
  });

  it('边界：观察条目缺 borderBoxSize → 回落实测高度（不抛 TypeError），未测量到高度时再回落 estimateSize', () => {
    vi.stubGlobal('ResizeObserver', ProgrammableResizeObserver);
    ProgrammableResizeObserver.instances = [];
    const { container } = renderSticky(100);
    const before = styleOf(container, 'message-scroller-content').height;

    const observer = stickyObserver();
    expect(observer).toBeDefined();
    expect(() => {
      act(() => {
        observer?.triggerWithoutBoxSize();
      });
    }).not.toThrow();

    // jsdom 的 getBoundingClientRect().height 为 0 ⇒ 实测回落仍落在 estimateSize
    expect(styleOf(container, 'message-scroller-content').height).toBe(before);
    expect(before).toBe('200px');
  });

  it('边界：观察条目 borderBoxSize 为空数组 → 同上回落实测高度，不抛错', () => {
    vi.stubGlobal('ResizeObserver', ProgrammableResizeObserver);
    ProgrammableResizeObserver.instances = [];
    const { container } = renderSticky(100);
    const before = styleOf(container, 'message-scroller-content').height;

    expect(() => {
      act(() => {
        stickyObserver()?.triggerEmptyBoxSize();
      });
    }).not.toThrow();

    expect(styleOf(container, 'message-scroller-content').height).toBe(before);
  });

  it('正向：观察条目缺 borderBoxSize 但实例已布好实测高度 → 以实测高度回填', () => {
    vi.stubGlobal('ResizeObserver', ProgrammableResizeObserver);
    ProgrammableResizeObserver.instances = [];
    const { container } = render(
      <Frame
        count={3}
        estimateSize={() => 100}
        renderItem={(index) => <span data-probe={index}>{index}</span>}
        renderSticky={() => [
          {
            index: 0,
            node: (
              <MessageScrollerSticky index={0} start={0} span={100}>
                <span>sticky</span>
              </MessageScrollerSticky>
            ),
          },
        ]}
      />,
    );
    const inner = container.querySelector<HTMLElement>(
      '[data-slot="message-scroller-sticky"] > div',
    );
    if (inner === null) throw new Error('缺少吸附层内层');
    vi.spyOn(inner, 'getBoundingClientRect').mockReturnValue({ height: 88 } as DOMRect);
    act(() => {
      stickyObserver()?.triggerWithoutBoxSize();
    });

    expect(translateYs(container)).toEqual([0, 88, 188]);
    expect(styleOf(container, 'message-scroller-content').height).toBe('288px');
  });

  it('边界：ResizeObserver 回报 0 → 回落 estimateSize，totalSize 不塌陷、不抛错', () => {
    vi.stubGlobal('ResizeObserver', ProgrammableResizeObserver);
    ProgrammableResizeObserver.instances = [];
    const { container } = renderSticky(100);
    const before = styleOf(container, 'message-scroller-content').height;

    const observer = stickyObserver();
    expect(observer).toBeDefined();
    act(() => {
      observer?.trigger(0);
    });

    expect(styleOf(container, 'message-scroller-content').height).toBe(before);
    expect(before).toBe('200px');
  });

  it('边界：ResizeObserver 回报 88 → resizeItem(index, 88)，后续项位移与 totalSize 随之变化', () => {
    vi.stubGlobal('ResizeObserver', ProgrammableResizeObserver);
    ProgrammableResizeObserver.instances = [];
    const { container } = render(
      <Frame
        count={3}
        estimateSize={() => 100}
        renderItem={(index) => <span data-probe={index}>{index}</span>}
        renderSticky={() => [
          {
            index: 0,
            node: (
              <MessageScrollerSticky index={0} start={0} span={100}>
                <span>sticky</span>
              </MessageScrollerSticky>
            ),
          },
        ]}
      />,
    );
    expect(translateYs(container)).toEqual([0, 100, 200]);

    act(() => {
      stickyObserver()?.trigger(88);
    });

    expect(translateYs(container)).toEqual([0, 88, 188]);
    expect(styleOf(container, 'message-scroller-content').height).toBe('288px');
  });

  it('边界：span 为 0（末轮倒挂钳制后）→ 外层 height 0px、内层照常挂载', () => {
    const { container } = renderSticky(0);

    expect(styleOf(container, 'message-scroller-sticky').height).toBe('0px');
    expect(container.querySelector('[data-testid="sticky-body"]')).not.toBeNull();
    expect(container.querySelector<HTMLElement>('[data-slot="message-scroller-sticky"] > div')?.style.position).toBe(
      'sticky',
    );
  });
});

// ---------------------------------------------------------------------------
// 到底按钮：距底判据、点击短路与 direction 入参退役
// ---------------------------------------------------------------------------

describe('MessageScrollerButton：距底判据与点击', () => {
  function buttonOf(container: HTMLElement): HTMLElement {
    const node = container.querySelector('[data-slot="message-scroller-button"]');
    if (node === null) throw new Error('缺少 message-scroller-button');
    return node as HTMLElement;
  }

  it('正向：贴底形态 → data-active 为 "false"，四个 data-* 钩子恒在，类名清单逐字保留', () => {
    const { container } = render(<Frame count={30} />);
    const button = buttonOf(container);

    expect(button.getAttribute('data-active')).toBe('false');
    expect(button.getAttribute('data-direction')).toBe('end');
    expect(button.getAttribute('data-variant')).toBe('secondary');
    expect(button.getAttribute('data-size')).toBe('icon-sm');
    for (const className of [
      'data-[active=false]:pointer-events-none',
      'data-[active=true]:opacity-100',
      'data-[direction=end]:bottom-4',
    ]) {
      expect(button.className, className).toContain(className);
    }
  });

  it('正向：非贴底形态（实例级抬高 scrollHeight）→ data-active 为 "true"', () => {
    const { container } = render(<Frame count={30} />);
    const viewport = viewportOf(container);
    const button = buttonOf(container);

    act(() => {
      Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 5000 });
      fireEvent.scroll(viewport);
    });

    expect(button.getAttribute('data-active')).toBe('true');
  });

  it('正向：count > 0 时点击 → 走 scrollToEnd（jsdom 无 Element.scrollTo ⇒ 静默无效、不抛错）', () => {
    const { container } = render(<Frame count={5} />);
    const viewport = viewportOf(container);
    const scrollTo = vi.fn();
    Object.defineProperty(viewport, 'scrollTo', { configurable: true, value: scrollTo });

    expect(() => fireEvent.click(buttonOf(container))).not.toThrow();
    expect(scrollTo).toHaveBeenCalled();
    expect(scrollTo.mock.calls[0]?.[0]).toMatchObject({ behavior: 'auto' });
  });

  it('正向：可达性对齐 —— type 恒为 button，未激活 tabIndex 为 -1、激活为 0', () => {
    const { container } = render(<Frame count={30} />);
    const viewport = viewportOf(container);
    const button = buttonOf(container);

    expect(button.getAttribute('type')).toBe('button');
    expect(button.getAttribute('tabindex')).toBe('-1');

    act(() => {
      Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 5000 });
      fireEvent.scroll(viewport);
    });

    expect(buttonOf(container).getAttribute('tabindex')).toBe('0');
  });

  it('正向：点击写入末项目标值（至少一次写入、末值为末端目标值）', () => {
    const { probe } = renderProbed({ count: 5 });
    const button = document.querySelector('[data-slot="message-scroller-button"]');
    if (button === null) throw new Error('缺少 message-scroller-button');

    act(() => {
      probe().setTop(0);
      fireEvent.click(button);
    });

    expect(probe().writes.length).toBeGreaterThan(0);
    // 按钮与复位共用同一条路径；reconcile 可能追加同值写入，故只断末值
    expect(probe().writes.at(-1)).toBe(endTarget());
  });

  it('异常：count 为 0 时点击 → 短路无操作、零 scrollTo 调用、不抛错', () => {
    const { container } = render(<Frame count={0} />);
    const viewport = viewportOf(container);
    const scrollTo = vi.fn();
    Object.defineProperty(viewport, 'scrollTo', { configurable: true, value: scrollTo });

    expect(() => fireEvent.click(buttonOf(container))).not.toThrow();
    expect(scrollTo).not.toHaveBeenCalled();
  });

  it('异常：direction 入参退役 —— 传入 start 不改变 data-direction，也不渲「Scroll to start」', () => {
    const { container } = render(
      <MessageScrollerProvider count={3} estimateSize={() => 100} getItemKey={(index) => index} renderItem={() => null}>
        <MessageScrollerButton {...({ direction: 'start' } as object)} />
      </MessageScrollerProvider>,
    );
    const button = buttonOf(container);

    expect(button.getAttribute('data-direction')).toBe('end');
    expect(button.textContent).toContain('Scroll to end');
    expect(button.textContent).not.toContain('Scroll to start');
  });

  it('边界：data-active 取 "true" / "false" 两显式字符串（显式值形态，可恒输出）', () => {
    const { container } = render(<Frame count={30} />);
    const viewport = viewportOf(container);
    const button = buttonOf(container);

    expect(button.getAttribute('data-active')).toBe('false');
    act(() => {
      Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 5000 });
      fireEvent.scroll(viewport);
    });
    expect(button.getAttribute('data-active')).toBe('true');
  });
});

// ---------------------------------------------------------------------------
// 导出面收窄：七组件恰为运行时导出键集合，三 hook 与 scrollToMessage 缺席
// ---------------------------------------------------------------------------

describe('message-scroller 导出面', () => {
  it('正向：七个组件全部可从模块导入并各自渲染成功（含新增吸附层）', () => {
    expect(() => render(<MessageScroller />)).not.toThrow();
    expect(() =>
      render(
        <MessageScrollerProvider count={0} estimateSize={() => 0} getItemKey={(index) => index} renderItem={() => null}>
          <MessageScrollerSticky index={0} start={0} span={0}>
            <span>sticky</span>
          </MessageScrollerSticky>
        </MessageScrollerProvider>,
      ),
    ).not.toThrow();

    const { container } = render(<Frame count={2} />);
    for (const slot of [
      'message-scroller',
      'message-scroller-viewport',
      'message-scroller-content',
      'message-scroller-item',
      'message-scroller-button',
    ]) {
      expect(container.querySelector(`[data-slot="${slot}"]`), slot).not.toBeNull();
    }
  });

  it('异常：运行时导出键集合恰为七个组件，三 hook 与 scrollToMessage 缺席', () => {
    expect(Object.keys(scrollerModule).sort()).toEqual([
      'MessageScroller',
      'MessageScrollerButton',
      'MessageScrollerContent',
      'MessageScrollerItem',
      'MessageScrollerProvider',
      'MessageScrollerSticky',
      'MessageScrollerViewport',
    ]);
    for (const retired of [
      'useMessageScroller',
      'useMessageScrollerScrollable',
      'useMessageScrollerVisibility',
      'scrollToMessage',
    ]) {
      expect(scrollerModule, retired).not.toHaveProperty(retired);
    }
  });

  it('边界：导出键集合稳定、无惰性副作用，集合内每项皆为函数（无垫片 / 无默认导出）', () => {
    const first = Object.keys(scrollerModule).sort();
    const second = Object.keys(scrollerModule).sort();

    expect(second).toEqual(first);
    for (const key of first) {
      expect(typeof (scrollerModule as Record<string, unknown>)[key], key).toBe('function');
    }
    expect(scrollerModule).not.toHaveProperty('default');
  });
});
