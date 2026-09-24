import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ExploreDoc } from '../../../types/dto';
import { useExploreDoc } from './useExploreDoc';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（read_explore / explore_doc_path /
// watch_subscribe / watch_unsubscribe）；Channel mock 为可编程 class（捕获
// onmessage，测试内手动注入 FileWatchEvent 信号模拟后端推送）。防抖窗口用
// vi.useFakeTimers 精确推进 500ms。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture 与 mock 行为
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
const NAME = 'api-retry';

function doc(content: string): ExploreDoc {
  return { name: NAME, content };
}

/** Channel mock 实例形状（vi.hoisted 内 class 不外泄类型，取其结构）。 */
interface ChannelLike {
  onmessage: ((event: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('未创建 watch Channel');
  return instance;
}

let docResult: ExploreDoc | null | string;
let derivedPath: string | null;
let subscribeResult: number | string;

function mockIpc() {
  ChannelMock.instances.length = 0;
  docResult = doc('# 探索笔记\n初稿');
  derivedPath = `${ROOT}\\openspec\\explores\\${NAME}.md`;
  subscribeResult = 7;
  invokeMock.mockImplementation((command: string) => {
    if (command === 'read_explore') {
      return typeof docResult === 'string' ? Promise.reject(docResult) : Promise.resolve(docResult);
    }
    if (command === 'explore_doc_path') {
      return Promise.resolve(derivedPath);
    }
    if (command === 'watch_subscribe') {
      return typeof subscribeResult === 'string'
        ? Promise.reject(subscribeResult)
        : Promise.resolve(subscribeResult);
    }
    if (command === 'watch_unsubscribe') {
      return Promise.resolve(true);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
});

afterEach(() => {
  vi.useRealTimers();
});

/** 挂载并冲刷微任务：首轮 read_explore 与订阅链（path → subscribe）均就绪。 */
async function mounted(root: string | null = ROOT, name: string | null = NAME) {
  return renderHook((args: [string | null, string | null]) => useExploreDoc(args[0], args[1]), {
    initialProps: [root, name] as [string | null, string | null],
  });
}

/** 渲染完成后冲刷一次微任务（fake timers 下 waitFor 不可用的替代）。 */
async function flush() {
  await act(async () => {});
}

function readCallCount(): number {
  return invokeMock.mock.calls.filter(([name]) => name === 'read_explore').length;
}

function subscribeCall(): { onEvent: unknown; path: unknown } | null {
  const call = invokeMock.mock.calls.find(([name]) => name === 'watch_subscribe');
  return call ? (call[1] as { onEvent: unknown; path: unknown }) : null;
}

describe('useExploreDoc：文档取数与 watch 订阅（AC-7）', () => {
  it('有效 root+name 挂载：read_explore 拉取一次、doc 呈现内容；explore_doc_path 后恰发起一次 watch_subscribe', async () => {
    const { result } = await mounted();
    await flush();

    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
    expect(result.current.error).toBeNull();
    expect(readCallCount()).toBe(1);
    expect(invokeMock).toHaveBeenCalledWith('explore_doc_path', { root: ROOT, name: NAME });
    const subscription = subscribeCall();
    expect(subscription).not.toBeNull();
    expect(subscription?.path).toBe(derivedPath);
    expect(subscription?.onEvent).toBeInstanceOf(ChannelMock);
    expect(invokeMock.mock.calls.filter(([name]) => name === 'watch_subscribe')).toHaveLength(1);
  });

  it('root/name 为 null：不取数、不订阅，loading 复位为 false（AC-8 新话题未选中的空态半）', async () => {
    const { result } = await mounted(ROOT, null);
    await flush();

    expect(readCallCount()).toBe(0);
    expect(subscribeCall()).toBeNull();
    expect(result.current.doc).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });
});

describe('useExploreDoc：信号 500ms trailing 防抖重拉（AC-7）', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it('Channel 推入 1 个信号：推进 500ms 后恰重拉一次 read_explore', async () => {
    const { result } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();
    expect(readCallCount()).toBe(1);

    act(() => {
      lastChannel().onmessage?.({ path: derivedPath });
    });
    expect(readCallCount()).toBe(1);

    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();
    expect(readCallCount()).toBe(2);
    expect(invokeMock).toHaveBeenLastCalledWith('read_explore', { root: ROOT, name: NAME });
  });

  it('500ms 窗口内推入 3 个信号：合并为恰一次重拉', async () => {
    const { result } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();

    act(() => {
      lastChannel().onmessage?.({ path: derivedPath });
      vi.advanceTimersByTime(200);
      lastChannel().onmessage?.({ path: derivedPath });
      vi.advanceTimersByTime(200);
      lastChannel().onmessage?.({ path: derivedPath });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(2);
  });

  it('信号载荷仅 { path }（无内容字段）仍正常触发重拉；契约破坏注入内容字段也只按信号处理', async () => {
    const { result } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();

    act(() => {
      lastChannel().onmessage?.({ path: derivedPath, content: '不应被读取的字节' });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(2);
    // 通道无内容纪律：预览只经显式拉取更新
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
  });

  it('卸载：以订阅时返回的 subscription_id 发起 watch_unsubscribe，卸载后信号不再引发任何取数', async () => {
    const { result, unmount } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();

    unmount();

    expect(invokeMock).toHaveBeenCalledWith('watch_unsubscribe', { subscriptionId: 7 });
    expect(invokeMock.mock.calls.filter(([name]) => name === 'watch_unsubscribe')).toHaveLength(1);
    const docBefore = result.current.doc;
    act(() => {
      lastChannel().onmessage?.({ path: derivedPath });
      vi.advanceTimersByTime(1000);
    });
    await flush();
    expect(result.current.doc).toBe(docBefore);
  });
});

describe('useExploreDoc：缺失与失败语义（AC-7/AC-8）', () => {
  it('explore_doc_path 返回 null（穿越名/blank root）：不发起订阅、doc 空态不崩', async () => {
    derivedPath = null;

    const { result } = await mounted();
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(subscribeCall()).toBeNull();
    expect(readCallCount()).toBe(1);
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
    expect(result.current.error).toBeNull();
  });

  it('read_explore reject：error 置位、不崩；watch 订阅仍建立（文档缺失与订阅生命周期解耦，D5）', async () => {
    docResult = 'IPC 断开';

    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).toContain('IPC 断开'));

    expect(result.current.doc).toBeNull();
    expect(subscribeCall()).not.toBeNull();
  });

  it('watch_subscribe reject（IPC 失败）：页面不崩、refresh 显式拉取仍可用', async () => {
    subscribeResult = '订阅失败';
    const { result } = await mounted();
    await waitFor(() => expect(result.current.doc).not.toBeNull());

    expect(result.current.error).toBeNull();

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(readCallCount()).toBe(2));
    expect(result.current.doc).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 强化断言（变异补杀）：loading 生命周期、参数变更竞态（取消抑制）、防抖
// 取消、订阅退订时序。
// ---------------------------------------------------------------------------

describe('useExploreDoc：loading 生命周期与订阅时序', () => {
  it('有效 root+name 挂载：同步进入 loading 态，取数完成后复位', async () => {
    const { result } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, NAME] as [string | null, string | null] },
    );

    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.doc).not.toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('订阅在挂载期间保持就绪：不提前退订', async () => {
    const { result } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();

    expect(invokeMock.mock.calls.filter(([name]) => name === 'watch_unsubscribe')).toHaveLength(0);
  });

  it('卸载先于订阅建立：订阅就绪后仍以订阅 id 补发退订', async () => {
    let resolveSubscribe: (id: number) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'read_explore') return Promise.resolve(doc('# 慢订阅'));
      if (command === 'explore_doc_path') return Promise.resolve(derivedPath);
      if (command === 'watch_subscribe') {
        return new Promise<number>((resolve) => {
          resolveSubscribe = resolve;
        });
      }
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { unmount } = await mounted();
    unmount();

    await act(async () => {
      resolveSubscribe(21);
    });
    expect(invokeMock).toHaveBeenCalledWith('watch_unsubscribe', { subscriptionId: 21 });
  });
});

describe('useExploreDoc：信号防抖与卸载取消', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it('信号推入后在防抖窗口内卸载：挂起的重拉被取消丢弃（不触发取数）', async () => {
    const { result, unmount } = await mounted();
    await flush();
    expect(readCallCount()).toBe(1);

    act(() => {
      lastChannel().onmessage?.({ path: derivedPath });
    });
    unmount();
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    await flush();

    expect(readCallCount()).toBe(1);
    expect(result.current.doc).not.toBeNull();
  });
});

describe('useExploreDoc：参数变更竞态（取消抑制）', () => {
  it('旧文档迟到的 resolve 不覆盖新文档（cancelled 抑制迟到写入）', async () => {
    let resolveOld: (value: ExploreDoc | null) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'read_explore') {
        if (params?.name === 'old-doc') {
          return new Promise<ExploreDoc | null>((resolve) => {
            resolveOld = resolve;
          });
        }
        return Promise.resolve({ name: 'new-doc', content: '新文档内容' });
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(`${ROOT}\\openspec\\explores\\${String(params?.name)}.md`);
      }
      if (command === 'watch_subscribe') return Promise.resolve(11);
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, 'old-doc'] as [string | null, string | null] },
    );
    await flush();

    rerender([ROOT, 'new-doc'] as [string | null, string | null]);
    await flush();
    expect(result.current.doc).toEqual({ name: 'new-doc', content: '新文档内容' });

    await act(async () => {
      resolveOld({ name: 'old-doc', content: '迟到的旧文档' });
    });
    expect(result.current.doc).toEqual({ name: 'new-doc', content: '新文档内容' });
    expect(result.current.error).toBeNull();
  });

  it('旧文档迟到的 reject 不置错误（cancelled 抑制迟到失败）', async () => {
    let rejectOld: (err: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'read_explore') {
        if (params?.name === 'old-doc') {
          return new Promise<ExploreDoc | null>((_resolve, reject) => {
            rejectOld = reject;
          });
        }
        return Promise.resolve({ name: 'new-doc', content: '新文档内容' });
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(`${ROOT}\\openspec\\explores\\${String(params?.name)}.md`);
      }
      if (command === 'watch_subscribe') return Promise.resolve(11);
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, 'old-doc'] as [string | null, string | null] },
    );
    await flush();

    rerender([ROOT, 'new-doc'] as [string | null, string | null]);
    await flush();

    await act(async () => {
      rejectOld(new Error('迟到的 IPC 失败'));
    });
    expect(result.current.error).toBeNull();
    expect(result.current.doc).toEqual({ name: 'new-doc', content: '新文档内容' });
  });
});
