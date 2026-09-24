// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { ChangeList, ExploreRecord, WorkspaceRecord } from '../types/dto';

// ---------------------------------------------------------------------------
// 集成关系「watch信号 → 防抖 → 显式重拉 → 卸载退订」（AC-7）：进入
// /explores/:name 详情后 explore_doc_path 派生路径 → watch_subscribe 单文件
// 订阅恰一次；Channel 注入失效信号（无内容）→ 500ms trailing 防抖合并为
// 恰一次 read_explore 显式重拉；卸载 watch_unsubscribe 退订、后续帧零拉取。
//
// 进程边界 Mock：invoke 按命令名分发并记录调用；Channel mock 捕获 onmessage
// （测试内按需注入 FileWatchEvent 帧模拟后端推送时序）；vi.useFakeTimers
// 精确推进 500ms 防抖窗口。链路内组件/hooks/路由运行时全部真实实现。
// ---------------------------------------------------------------------------

const { ChannelMock, checkMock, getVersionMock, invokeMock, openMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return {
    ChannelMock,
    checkMock: vi.fn(),
    getVersionMock: vi.fn(),
    invokeMock: vi.fn(),
    openMock: vi.fn(),
  };
});

vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: checkMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

const FIRST: WorkspaceRecord = { root: 'C:\\demo\\alpha', name: 'alpha', addedAt: 1 };
const RECORD_NAME = 'foo';
const DERIVED_PATH = 'C:\\demo\\alpha\\openspec\\explores\\foo.md';

const fakeList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', inventory: 'v2', created: null, unparsable: false },
  ],
  archiveGroups: [],
};

const exploreRecord: ExploreRecord = {
  id: 7,
  root: FIRST.root,
  name: RECORD_NAME,
  createdAt: 1727000000000,
  updatedAt: 1727000000000,
};

let docContent: { name: string; content: string } | null;
let derivedPath: string | null;
let subscribeOutcome: number | string;
const SUBSCRIPTION_ID = 7;

function mockIpc() {
  ChannelMock.instances.length = 0;
  docContent = { name: RECORD_NAME, content: '# 笔记内容' };
  derivedPath = DERIVED_PATH;
  subscribeOutcome = SUBSCRIPTION_ID;
  invokeMock.mockImplementation((command: string) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([FIRST]);
    }
    if (command === 'list_changes') {
      return Promise.resolve(fakeList);
    }
    if (command === 'list_explore_records') {
      return Promise.resolve([exploreRecord]);
    }
    if (command === 'read_explore') {
      return Promise.resolve(docContent);
    }
    if (command === 'explore_doc_path') {
      return Promise.resolve(derivedPath);
    }
    if (command === 'watch_subscribe') {
      return typeof subscribeOutcome === 'string'
        ? Promise.reject(subscribeOutcome)
        : Promise.resolve(subscribeOutcome);
    }
    if (command === 'watch_unsubscribe') {
      return Promise.resolve(true);
    }
    if (command === 'agent_run_chain') {
      return Promise.resolve([]);
    }
    if (command === 'agent_run_events') {
      return Promise.resolve([]);
    }
    return Promise.resolve(null);
  });
}

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

function subscribeCall(): { onEvent: unknown; path: unknown } | null {
  const call = invokeMock.mock.calls.find(([name]) => name === 'watch_subscribe');
  return call ? (call[1] as { onEvent: unknown; path: unknown }) : null;
}

/** 订阅命令入参里的 Channel 实例（测试内注入帧模拟后端推送）。 */
function subscribedChannel(): { onmessage: ((event: unknown) => void) | null } {
  const subscription = subscribeCall();
  if (!subscription) throw new Error('watch_subscribe 未发生');
  return subscription.onEvent as { onmessage: ((event: unknown) => void) | null };
}

/** 冲刷微任务（fake timers 下 waitFor 不可用的替代）。 */
async function flush() {
  await act(async () => {});
  await act(async () => {});
}

/** 轮询冲刷直到条件成立（fake timers 下以微任务轮次代替 waitFor 定时器）。 */
async function settleUntil(check: () => boolean, maxRounds = 100) {
  for (let round = 0; round < maxRounds; round += 1) {
    if (check()) return;
    await act(async () => {});
  }
  if (!check()) throw new Error('settleUntil 条件未达成');
}

/** 深链启动直达详情，等待文档拉取与订阅链就绪（或判定订阅不可建）。 */
async function onDetail() {
  window.location.hash = `#/explores/${RECORD_NAME}`;
  render(<App />);
  await settleUntil(() => screen.queryByTestId('explore-detail') !== null);
  await settleUntil(
    () =>
      countOf('read_explore') === 1 && (derivedPath === null || countOf('watch_subscribe') === 1),
  );
  await flush();
}

/** 推进防抖窗口并冲刷（500ms trailing）。 */
async function advanceDebounceWindow() {
  await act(async () => {
    vi.advanceTimersByTime(500);
  });
  await flush();
}

beforeEach(() => {
  window.location.hash = '';
  getVersionMock.mockReset();
  invokeMock.mockReset();
  openMock.mockReset();
  checkMock.mockReset();
  checkMock.mockResolvedValue(null);
  getVersionMock.mockResolvedValue('0.3.0');
  toast.dismiss();
  mockIpc();
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe = vi.fn();
      unobserve = vi.fn();
      disconnect = vi.fn();
    },
  );
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// 订阅建立与信号合并拉取（AC-7）
// ---------------------------------------------------------------------------

describe('explore_watch_refresh：订阅建立与信号合并拉取（AC-7）', () => {
  it('挂载 → explore_doc_path → watch_subscribe 恰一次，入参路径与派生路径一致', async () => {
    await onDetail();

    expect(invokeMock).toHaveBeenCalledWith('explore_doc_path', {
      root: FIRST.root,
      name: RECORD_NAME,
    });
    expect(countOf('watch_subscribe')).toBe(1);
    expect(subscribeCall()?.path).toBe(DERIVED_PATH);
    expect(countOf('read_explore')).toBe(1);
    expect(screen.getByTestId('preview-doc')?.textContent).toContain('笔记内容');
  });

  it('单帧信号 → 500ms 后恰一次 read_explore 重拉（trailing 防抖）', async () => {
    await onDetail();

    await act(async () => {
      subscribedChannel().onmessage?.({ path: DERIVED_PATH });
    });
    expect(countOf('read_explore')).toBe(1);

    await advanceDebounceWindow();

    expect(countOf('read_explore')).toBe(2);
    expect(invokeMock).toHaveBeenLastCalledWith('read_explore', {
      root: FIRST.root,
      name: RECORD_NAME,
    });
  });

  it('500ms 窗口内 3 帧 → 合并为恰一次重拉（防抖 trailing，AC-7）', async () => {
    await onDetail();

    await act(async () => {
      subscribedChannel().onmessage?.({ path: DERIVED_PATH });
      vi.advanceTimersByTime(200);
      subscribedChannel().onmessage?.({ path: DERIVED_PATH });
      vi.advanceTimersByTime(200);
      subscribedChannel().onmessage?.({ path: DERIVED_PATH });
    });
    await advanceDebounceWindow();

    expect(countOf('read_explore')).toBe(2);
  });

  it('信号载荷含内容字段（契约破坏注入）：前端仍只按信号处理、重拉恰一次且不读取内容', async () => {
    await onDetail();

    await act(async () => {
      subscribedChannel().onmessage?.({ path: DERIVED_PATH, content: '不应被读取的字节' });
    });
    await advanceDebounceWindow();

    expect(countOf('read_explore')).toBe(2);
    expect(screen.getByTestId('preview-doc')?.textContent).toContain('笔记内容');
    expect(screen.getByTestId('preview-doc')?.textContent).not.toContain('不应被读取的字节');
  });
});

// ---------------------------------------------------------------------------
// 卸载退订与失效语义（AC-7）
// ---------------------------------------------------------------------------

describe('explore_watch_refresh：卸载退订与失效语义（AC-7）', () => {
  it('卸载 → watch_unsubscribe 以订阅 id 恰发一次，后续帧不再驱动数据面', async () => {
    const { unmount } = render(<App />);
    window.location.hash = `#/explores/${RECORD_NAME}`;
    await settleUntil(() => countOf('watch_subscribe') === 1);
    await flush();
    expect(screen.getByTestId('preview-doc') !== null).toBe(true);

    unmount();

    expect(invokeMock).toHaveBeenCalledWith('watch_unsubscribe', {
      subscriptionId: SUBSCRIPTION_ID,
    });
    expect(countOf('watch_unsubscribe')).toBe(1);

    // 卸载后再注入帧：数据面不再被幽灵订阅驱动（预览内容不变）
    const docBefore = screen.queryByTestId('preview-doc');
    await act(async () => {
      subscribedChannel().onmessage?.({ path: DERIVED_PATH });
      vi.advanceTimersByTime(1000);
    });
    await flush();
    expect(screen.queryByTestId('preview-doc')).toBe(docBefore);
  });

  it('explore_doc_path 返回 null（穿越名/blank root）：零订阅、页面不崩', async () => {
    derivedPath = null;
    await onDetail();

    expect(countOf('watch_subscribe')).toBe(0);
    expect(screen.getByTestId('explore-detail') !== null).toBe(true);
    expect(screen.getByTestId('preview-doc') !== null).toBe(true);
  });

  it('watch_subscribe reject：不阻断文档初始拉取与显式 refresh（取数纪律优先于推送）', async () => {
    subscribeOutcome = '订阅失败';
    await onDetail();

    expect(countOf('watch_subscribe')).toBe(1);
    expect(screen.getByTestId('preview-doc')?.textContent).toContain('笔记内容');

    fireEvent.click(screen.getByTestId('preview-refresh'));
    await flush();

    expect(countOf('read_explore')).toBe(2);
    expect(screen.getByTestId('preview-doc') !== null).toBe(true);
  });
});
