import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { Notification } from '../types/generated/bindings';
import { useNotifications } from './use-notifications';

// ---------------------------------------------------------------------------
// 进程边界 Mock：`invoke` 按命令名分派（可编程 resolve / reject）+ `Channel` 为
// 与真实件同构的替身（原型 accessor + 私有字段，投递读私有字段——把 `onmessage`
// 放成实例字段会让投递绕过装配层）。`lib/notify.ts` **不 mock**：消息分流的判据
// 依赖真实门面（替身会让「Info 也弹 toast」类回归不可见）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock, toastMock } = vi.hoisted(() => {
  class ChannelMock {
    #onmessage: ((event: unknown) => void) | null = () => {};
    public static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
    public get onmessage(): ((event: unknown) => void) | null {
      return this.#onmessage;
    }
    public set onmessage(handler: ((event: unknown) => void) | null) {
      this.#onmessage = handler;
    }
    /** Rust → 前端投递（读私有字段，与真实 `Channel` 的 deliver 同构） */
    public deliver(event: unknown): void {
      if (typeof this.#onmessage === 'function') this.#onmessage(event);
    }
  }

  const toastMock = Object.assign(vi.fn(), {
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  });

  return { ChannelMock, invokeMock: vi.fn(), toastMock };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));
vi.mock('sonner', () => ({ toast: toastMock }));

// ---------------------------------------------------------------------------
// 可编程 IPC
// ---------------------------------------------------------------------------

type Behavior = () => Promise<unknown>;

let behaviors: Record<string, Behavior>;

function mockIpc(): void {
  ChannelMock.instances.length = 0;
  behaviors = {
    notification_subscribe: () => Promise.resolve(0),
    notification_list: () => Promise.resolve({ items: [], total: 0 }),
    notification_mark_read: () => Promise.resolve(0),
    notification_mark_all_read: () => Promise.resolve(0),
    notification_record: () => Promise.resolve(null),
  };
  invokeMock.mockImplementation(
    (command: string) => behaviors[command]?.() ?? Promise.resolve(null),
  );
}

function callsOf(command: string): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === command);
}

function channel(): InstanceType<typeof ChannelMock> {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('订阅未建立：无 Channel 实例');
  return instance;
}

function toastCount(): number {
  return (
    toastMock.mock.calls.length +
    toastMock.success.mock.calls.length +
    toastMock.warning.mock.calls.length +
    toastMock.error.mock.calls.length
  );
}

function notification(overrides: Partial<Notification> & Pick<Notification, 'id'>): Notification {
  return {
    kind: 'event',
    level: 'warning',
    message: `变更「add-feature」停在 dev-design 前等你确认（${overrides.id}）`,
    body: null,
    root: '/ws/alpha',
    sourceRef: 'add-feature',
    createdAt: 1_727_000_000_000,
    ...overrides,
  };
}

const CALL_SITES = 'hooks/use-notifications.ts';

function sourceOf(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), 'src', relativePath), 'utf8');
}

/** 去注释后的代码文本（结构锚只数代码字面——doc / 注释里提到同一词不算调用点）。 */
function codeOf(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
  for (const mock of [toastMock, toastMock.success, toastMock.warning, toastMock.error]) {
    mock.mockClear();
  }
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// 订阅装配：Channel 单实例 + 一次订阅 + 初值
// ---------------------------------------------------------------------------

describe('收件箱订阅：Channel 单实例与初值未读', () => {
  it('挂载一次即构造一个 Channel 并以它为实参订阅一次，返回值写入未读', async () => {
    behaviors.notification_subscribe = () => Promise.resolve(3);

    const { result } = renderHook(() => useNotifications());

    expect(ChannelMock.instances).toHaveLength(1);
    const subscribeCalls = callsOf('notification_subscribe');
    expect(subscribeCalls).toHaveLength(1);
    const payload = subscribeCalls[0][1] as { onEvent: unknown };
    expect(payload.onEvent).toBe(channel());

    await waitFor(() => {
      expect(result.current.unread).toBe(3);
    });
  });

  it('零未读与 subscribe 返回非数字两形态均落在未读 0', async () => {
    behaviors.notification_subscribe = () => Promise.resolve(0);
    const zero = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(zero.result.current.unread).toBe(0);
    });
    expect(toastCount()).toBe(0);

    behaviors.notification_subscribe = () => Promise.resolve(null as unknown as number);
    const nonNumeric = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(nonNumeric.result.current.unread).toBe(0);
    });
  });

  it('订阅失败静默：error 保持 null、零 toast、未读仍 0', async () => {
    behaviors.notification_subscribe = () => Promise.reject(new Error('后端不可达'));

    const { result } = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(callsOf('notification_subscribe')).toHaveLength(1);
    });

    expect(result.current.error).toBeNull();
    expect(result.current.unread).toBe(0);
    expect(toastCount()).toBe(0);
  });

  it('卸载后投递不抛错且不新增订阅，二次挂载各订阅一次', async () => {
    const first = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(callsOf('notification_subscribe')).toHaveLength(1);
    });
    const delivered = channel();
    first.unmount();

    expect(() => {
      act(() => {
        delivered.deliver(notification({ id: 'run:r1:finished' }));
      });
    }).not.toThrow();
    expect(callsOf('notification_subscribe')).toHaveLength(1);

    renderHook(() => useNotifications());
    await waitFor(() => {
      expect(callsOf('notification_subscribe')).toHaveLength(2);
    });
  });
});

// ---------------------------------------------------------------------------
// 消息分流：呈现 / 前插 / 计数
// ---------------------------------------------------------------------------

describe('收件箱消息分流：呈现与计数', () => {
  it('warning / error / success 各一条均前插且未读与总数各加一并呈现对应档', async () => {
    behaviors.notification_subscribe = () => Promise.resolve(2);
    const { result } = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(result.current.unread).toBe(2);
    });
    const stream = channel();

    act(() => {
      stream.deliver(notification({ id: 'n-warning', level: 'warning' }));
    });
    act(() => {
      stream.deliver(notification({ id: 'n-error', level: 'error' }));
    });
    act(() => {
      stream.deliver(notification({ id: 'n-success', level: 'success' }));
    });

    expect(result.current.items.map((item) => item.id)).toEqual([
      'n-success',
      'n-error',
      'n-warning',
    ]);
    expect(result.current.unread).toBe(5);
    expect(result.current.total).toBe(3);
    expect(toastMock.warning).toHaveBeenCalledTimes(1);
    expect(toastMock.error).toHaveBeenCalledTimes(1);
    expect(toastMock.success).toHaveBeenCalledTimes(1);
  });

  it('Info 消息只前插与计数、零 toast（机械规则，不按 status 分叉）', async () => {
    const { result } = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(callsOf('notification_subscribe')).toHaveLength(1);
    });

    act(() => {
      channel().deliver(notification({ id: 'n-info', level: 'info' }));
    });

    expect(result.current.items.map((item) => item.id)).toEqual(['n-info']);
    expect(result.current.unread).toBe(1);
    expect(result.current.total).toBe(1);
    expect(toastCount()).toBe(0);
  });

  it('跨模块链路：带 body 的 warning / error 逐字落进第二实参 description', async () => {
    const { result } = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(callsOf('notification_subscribe')).toHaveLength(1);
    });
    const stream = channel();

    act(() => {
      stream.deliver(notification({ id: 'n-warning', level: 'warning', body: '设计相位产出。' }));
    });
    act(() => {
      stream.deliver(notification({ id: 'n-error', level: 'error', body: '写面失败' }));
    });

    expect(toastMock.warning.mock.calls[0]).toEqual([
      result.current.items[1].message,
      { description: '设计相位产出。' },
    ]);
    expect(toastMock.error.mock.calls[0]).toEqual([
      result.current.items[0].message,
      { description: '写面失败' },
    ]);
  });

  it('三条连续消息后条目序为到达逆序且总数等于初值加三条', async () => {
    behaviors.notification_subscribe = () => Promise.resolve(4);
    const { result } = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(result.current.unread).toBe(4);
    });

    act(() => {
      channel().deliver(notification({ id: 'n-1' }));
      channel().deliver(notification({ id: 'n-2' }));
      channel().deliver(notification({ id: 'n-3' }));
    });

    expect(result.current.items.map((item) => item.id)).toEqual(['n-3', 'n-2', 'n-1']);
    expect(result.current.total).toBe(3);
    expect(result.current.unread).toBe(7);
  });
});

// ---------------------------------------------------------------------------
// load：首屏拉取
// ---------------------------------------------------------------------------

describe('收件箱首屏 load', () => {
  it('以 (0, 50) 拉一次并覆盖条目与总数，loading 由真回到假', async () => {
    behaviors.notification_list = () =>
      Promise.resolve({ items: [notification({ id: 'run:r1:finished' })], total: 7 });
    const { result } = renderHook(() => useNotifications());

    act(() => {
      result.current.load();
    });
    expect(result.current.loading).toBe(true);

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(callsOf('notification_list')).toHaveLength(1);
    expect(callsOf('notification_list')[0][1]).toEqual({ offset: 0, limit: 50 });
    expect(result.current.items.map((item) => item.id)).toEqual(['run:r1:finished']);
    expect(result.current.total).toBe(7);
    expect(result.current.error).toBeNull();
  });

  it('空页清空状态且零错误，连续两次 load 以最后一次结果胜出', async () => {
    behaviors.notification_list = () => Promise.resolve({ items: [], total: 0 });
    const { result } = renderHook(() => useNotifications());

    act(() => {
      result.current.load();
    });
    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.items).toEqual([]);
    expect(result.current.total).toBe(0);
    expect(result.current.error).toBeNull();

    behaviors.notification_list = () =>
      Promise.resolve({ items: [notification({ id: 'run:r2:finished' })], total: 1 });
    act(() => {
      result.current.load();
      result.current.load();
    });
    await waitFor(() => {
      expect(result.current.total).toBe(1);
    });
    expect(callsOf('notification_list')).toHaveLength(3);
    expect(result.current.items.map((item) => item.id)).toEqual(['run:r2:finished']);
  });

  it('列表失败置 error（inline 持久）且条目与总数保持原值', async () => {
    behaviors.notification_list = () =>
      Promise.resolve({ items: [notification({ id: 'run:r1:finished' })], total: 2 });
    const { result } = renderHook(() => useNotifications());
    act(() => {
      result.current.load();
    });
    await waitFor(() => {
      expect(result.current.items).toHaveLength(1);
    });

    behaviors.notification_list = () => Promise.reject(new Error('库打开失败'));
    act(() => {
      result.current.load();
    });
    await waitFor(() => {
      expect(result.current.error).toBe('Error: 库打开失败');
    });
    expect(result.current.items).toHaveLength(1);
    expect(result.current.total).toBe(2);
    expect(result.current.loading).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// markRead / markAllRead：以返回值回写
// ---------------------------------------------------------------------------

describe('收件箱标记已读', () => {
  it('markRead 先并入本地已读并调一次命令，未读以返回值为准', async () => {
    behaviors.notification_mark_read = () => Promise.resolve(2);
    const { result } = renderHook(() => useNotifications());

    await act(async () => {
      await result.current.markRead(['run:r1:finished']);
    });

    expect(callsOf('notification_mark_read')).toHaveLength(1);
    expect(callsOf('notification_mark_read')[0][1]).toEqual({ ids: ['run:r1:finished'] });
    expect([...result.current.readIds]).toEqual(['run:r1:finished']);
    expect(result.current.unread).toBe(2);
  });

  it('markRead 空集合照发命令，同 id 两次后本地已读仍单元素', async () => {
    behaviors.notification_mark_read = () => Promise.resolve(0);
    const { result } = renderHook(() => useNotifications());

    await act(async () => {
      await result.current.markRead([]);
    });
    expect(callsOf('notification_mark_read')[0][1]).toEqual({ ids: [] });

    await act(async () => {
      await result.current.markRead(['run:r1:finished']);
    });
    await act(async () => {
      await result.current.markRead(['run:r1:finished']);
    });
    expect(callsOf('notification_mark_read')).toHaveLength(3);
    expect([...result.current.readIds]).toEqual(['run:r1:finished']);
  });

  it('markRead 失败置 error 且未读不自行减一', async () => {
    behaviors.notification_mark_read = () => Promise.reject(new Error('写失败'));
    const { result } = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(callsOf('notification_subscribe')).toHaveLength(1);
    });
    const before = result.current.unread;

    await act(async () => {
      await result.current.markRead(['run:r1:finished']);
    });

    expect(result.current.error).toBe('Error: 写失败');
    expect(result.current.unread).toBe(before);
  });

  it('markAllRead 调一次命令后未读归零且全部已读态置真', async () => {
    behaviors.notification_mark_all_read = () => Promise.resolve(0);
    const { result } = renderHook(() => useNotifications());

    await act(async () => {
      await result.current.markAllRead();
    });

    expect(callsOf('notification_mark_all_read')).toHaveLength(1);
    expect(result.current.unread).toBe(0);
    expect(result.current.allRead).toBe(true);

    await act(async () => {
      await result.current.markAllRead();
    });
    expect(callsOf('notification_mark_all_read')).toHaveLength(2);
    expect(result.current.allRead).toBe(true);
  });

  it('markAllRead 失败置 error 且全部已读态不翻转、未读保持原值', async () => {
    behaviors.notification_subscribe = () => Promise.resolve(3);
    behaviors.notification_mark_all_read = () => Promise.reject(new Error('写失败'));
    const { result } = renderHook(() => useNotifications());
    await waitFor(() => {
      expect(result.current.unread).toBe(3);
    });

    await act(async () => {
      await result.current.markAllRead();
    });

    expect(result.current.error).toBe('Error: 写失败');
    expect(result.current.allRead).toBe(false);
    expect(result.current.unread).toBe(3);
  });
});

// ---------------------------------------------------------------------------
// 结构锚（AC-10）：订阅与轮询纪律
// ---------------------------------------------------------------------------

describe('收件箱结构锚：单订阅点与零轮询', () => {
  it('本文件内 new Channel 与订阅调用各恰一处且零定时轮询', () => {
    const code = codeOf(sourceOf(CALL_SITES));

    expect(code.match(/new Channel/g)).toHaveLength(1);
    expect(code.match(/notificationSubscribe\(/g)).toHaveLength(1);
    expect(code).not.toMatch(/setInterval/);
    expect(code).not.toMatch(/use-file-watch-signal/);
  });

  it('文件失效信号件零通知耦合（两件按驱动源分家）', () => {
    const source = sourceOf('hooks/use-file-watch-signal.ts');

    expect(source).not.toMatch(/notification/i);
  });
});
