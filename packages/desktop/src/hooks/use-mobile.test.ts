import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { useIsMobile } from './use-mobile';

// ---------------------------------------------------------------------------
// window.matchMedia / window.innerWidth stub：jsdom 无 matchMedia 实现。
// stub 的 matches 与 window.innerWidth 同源计算（< 768px），保证首测与监听消费
// 同一断点口径；捕获 addEventListener 注册的监听，供用例手动派发 change。
// ---------------------------------------------------------------------------

type ChangeListener = (event: MediaQueryListEvent) => void;

function stubViewport(initialWidth: number) {
  const queries: string[] = [];
  const registered: ChangeListener[] = [];
  const removed: ChangeListener[] = [];

  const widthDescriptor = Object.getOwnPropertyDescriptor(window, 'innerWidth');
  const setWidth = (width: number) => {
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: width,
    });
  };
  setWidth(initialWidth);

  const matchMedia = vi.fn((query: string) => {
    queries.push(query);
    return {
      matches: window.innerWidth < 768,
      media: query,
      onchange: null,
      addEventListener: (type: string, listener: ChangeListener) => {
        if (type === 'change') registered.push(listener);
      },
      removeEventListener: (type: string, listener: ChangeListener) => {
        if (type === 'change') removed.push(listener);
      },
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    };
  });
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    writable: true,
    value: matchMedia,
  });

  return {
    queries,
    registered,
    removed,
    matchMedia,
    setWidth,
    /** 模拟 MediaQueryList 派发 change（当前窗口宽度决定新 matches） */
    dispatchChange() {
      act(() => {
        for (const listener of registered) {
          listener({ matches: window.innerWidth < 768 } as MediaQueryListEvent);
        }
      });
    },
    restore() {
      if (widthDescriptor) Object.defineProperty(window, 'innerWidth', widthDescriptor);
      Reflect.deleteProperty(window, 'matchMedia');
    },
  };
}

describe('useIsMobile：< 768px 断点判定（matchMedia 查询 + change 监听）', () => {
  let viewport: ReturnType<typeof stubViewport>;

  afterEach(() => {
    viewport.restore();
  });

  it('matchMedia matches=true（窗口 < 768px）：返回 true', () => {
    viewport = stubViewport(500);

    const { result } = renderHook(() => useIsMobile());

    expect(result.current).toBe(true);
  });

  it('matchMedia matches=false（窗口 ≥ 768px）：返回 false', () => {
    viewport = stubViewport(1100);

    const { result } = renderHook(() => useIsMobile());

    expect(result.current).toBe(false);
  });

  it('查询串恰为 (max-width: 767px)：767px 命中、768px 不命中（监听与查询同串）', () => {
    viewport = stubViewport(767);
    const narrow = renderHook(() => useIsMobile());
    expect(narrow.result.current).toBe(true);
    narrow.unmount();

    viewport.setWidth(768);
    const wide = renderHook(() => useIsMobile());
    expect(wide.result.current).toBe(false);
    wide.unmount();

    // 断点边界由查询串表达：register change 监听与首测消费同一查询串
    expect(viewport.queries.length).toBeGreaterThan(0);
    expect(viewport.queries.every((query) => query === '(max-width: 767px)')).toBe(true);
  });

  it('change 事件推送新 matches：state 跟随更新（监听回调生效）', () => {
    viewport = stubViewport(1100);
    const { result } = renderHook(() => useIsMobile());
    expect(result.current).toBe(false);

    // 窗口宽跨过断点 → MediaQueryList 派发 change
    viewport.setWidth(500);
    viewport.dispatchChange();

    expect(result.current).toBe(true);
  });

  it('卸载后 change 事件：监听已移除（removeEventListener 被调）、不更新不泄漏', () => {
    viewport = stubViewport(1100);
    const { unmount } = renderHook(() => useIsMobile());
    unmount();

    // 卸载时注册的 change 监听全部被移除，此后派发 change 不再回流 state
    expect(viewport.registered.length).toBeGreaterThan(0);
    expect(viewport.removed).toEqual(viewport.registered);
    expect(() => viewport.dispatchChange()).not.toThrow();
  });
});
