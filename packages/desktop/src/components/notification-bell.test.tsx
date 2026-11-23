// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, useLocation } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { Notification } from '../types/generated/bindings';
import { NotificationBell } from './notification-bell';

// ---------------------------------------------------------------------------
// 进程边界 Mock：`invoke` 按命令名分派 + `Channel` 同构替身；`sonner` 四档
// `vi.fn()` 观测点。`hooks/use-notifications.ts` / `lib/notify.ts` **不 mock**：
// 本组用例的判据是「徽标 + 抽屉 + 跳转」与真实状态面的耦合（替身会掩盖未读
// 回写口径的回归）。路由用真实 `MemoryRouter`（URL 探针读 `useLocation`）。
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

const ROOT = 'C:\\demo\\alpha';
const OTHER_ROOT = 'C:\\demo\\beta';

type Behavior = () => Promise<unknown>;

let behaviors: Record<string, Behavior>;
let pathRef = '';

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

function notification(overrides: Partial<Notification> & Pick<Notification, 'id'>): Notification {
  return {
    kind: 'event',
    level: 'warning',
    message: `变更「add-feature」停在 dev-design 前等你确认（${overrides.id}）`,
    body: null,
    root: ROOT,
    sourceRef: 'add-feature',
    createdAt: 1_727_000_000_000,
    ...overrides,
  };
}

/** URL 探针：把 Router 当前 pathname 投影到 DOM 与 `pathRef`（调用序断言用）。 */
function LocationProbe() {
  const { pathname } = useLocation();
  pathRef = pathname;
  return <span data-testid="location-probe">{pathname}</span>;
}

function mount(root: string = ROOT) {
  const onSelect = vi.fn<(root: string) => void>();
  const view = render(
    <MemoryRouter initialEntries={['/']}>
      <NotificationBell onSelect={onSelect} root={root} />
      <LocationProbe />
    </MemoryRouter>,
  );
  return { onSelect, view };
}

/** 打开抽屉（铃铛触发器 + 列表命令就绪）。 */
async function openDrawer(): Promise<void> {
  fireEvent.click(screen.getByRole('button', { name: '通知' }));
  await screen.findByRole('heading', { name: '通知' });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
  pathRef = '';
  for (const mock of [toastMock, toastMock.success, toastMock.warning, toastMock.error]) {
    mock.mockClear();
  }
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// 徽标三态
// ---------------------------------------------------------------------------

describe('铃铛徽标三态', () => {
  it('未读 0 不渲染徽标、250 呈 99+', async () => {
    const zero = mount();
    await waitFor(() => {
      expect(callsOf('notification_subscribe')).toHaveLength(1);
    });
    expect(screen.queryByTestId('notification-badge')).toBeNull();
    zero.view.unmount();

    behaviors.notification_subscribe = () => Promise.resolve(250);
    const big = mount();
    const badge = await screen.findByTestId('notification-badge');
    expect(badge.textContent).toBe('99+');
    big.view.unmount();
  });

  it('阈值边界：1 与 99 呈数字、100 呈 99+', async () => {
    for (const [unread, expected] of [
      [1, '1'],
      [99, '99'],
      [100, '99+'],
    ] as const) {
      behaviors.notification_subscribe = () => Promise.resolve(unread);
      const { view } = mount();
      const badge = await screen.findByTestId('notification-badge');
      expect(badge.textContent).toBe(expected);
      view.unmount();
    }
  });

  it('订阅失败：无徽标、铃铛仍在文档、零 toast', async () => {
    behaviors.notification_subscribe = () => Promise.reject(new Error('后端不可达'));
    mount();
    await waitFor(() => {
      expect(callsOf('notification_subscribe')).toHaveLength(1);
    });

    expect(screen.getByRole('button', { name: '通知' }) !== null).toBe(true);
    expect(screen.queryByTestId('notification-badge')).toBeNull();
    expect(toastMock.error).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 抽屉：打开、内容、空态、错误
// ---------------------------------------------------------------------------

describe('收件箱抽屉内容', () => {
  it('点击铃铛打开抽屉并以 (0, 50) 拉一次首屏，行序即后端返回序', async () => {
    behaviors.notification_list = () =>
      Promise.resolve({
        items: [
          notification({ id: 'run:r2:finished', message: '较新的一条' }),
          notification({ id: 'run:r1:finished', message: '较旧的一条' }),
        ],
        total: 2,
      });
    mount();

    await openDrawer();
    await waitFor(() => {
      expect(callsOf('notification_list')).toHaveLength(1);
    });
    expect(callsOf('notification_list')[0][1]).toEqual({ offset: 0, limit: 50 });

    const rows = await screen.findAllByTestId('notification-item');
    expect(rows.map((row) => row.textContent?.includes('较新的一条'))).toEqual([true, false]);
  });

  it('行内容齐备：主文案、副文案在场才渲染、时间列与四档图标', async () => {
    const items = [
      notification({ id: 'n-info', level: 'info', body: '信息档副文案' }),
      notification({ id: 'n-success', level: 'success' }),
      notification({ id: 'n-warning', level: 'warning', body: '警告档副文案' }),
      notification({ id: 'n-error', level: 'error' }),
    ];
    behaviors.notification_list = () => Promise.resolve({ items, total: items.length });
    mount();

    await openDrawer();
    const rows = await screen.findAllByTestId('notification-item');
    expect(rows).toHaveLength(4);
    for (const row of rows) {
      expect(row.querySelector('svg')).not.toBeNull();
    }
    expect(screen.getByText('信息档副文案') !== null).toBe(true);
    expect(screen.getByText('警告档副文案') !== null).toBe(true);
    expect(rows[1].textContent?.includes('信息档副文案')).toBe(false);
    const createdAt = new Date(items[0].createdAt).toLocaleString();
    expect(rows[0].textContent?.includes(createdAt)).toBe(true);
  });

  it('空态：条目为空时呈空态文案且零列表行', async () => {
    behaviors.notification_list = () => Promise.resolve({ items: [], total: 0 });
    mount();

    await openDrawer();
    const empty = await screen.findByTestId('notification-empty');
    expect(empty.textContent).toBe('暂无通知');
    expect(screen.queryAllByTestId('notification-item')).toEqual([]);
  });

  it('列表失败：抽屉内 inline 错误在场、抽屉不关闭、列表区零行', async () => {
    behaviors.notification_list = () => Promise.reject(new Error('库打开失败'));
    mount();

    await openDrawer();
    const error = await screen.findByTestId('notification-error');
    expect(error.textContent).toBe('Error: 库打开失败');
    expect(screen.queryAllByTestId('notification-item')).toEqual([]);
    expect(screen.getByRole('heading', { name: '通知' }) !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 标记已读
// ---------------------------------------------------------------------------

describe('收件箱标记已读', () => {
  it('点击某行发一次单条置已读并置该行灰态', async () => {
    behaviors.notification_list = () =>
      Promise.resolve({ items: [notification({ id: 'run:r1:finished' })], total: 1 });
    mount();
    await openDrawer();

    const [row] = await screen.findAllByTestId('notification-item');
    expect(row.getAttribute('data-state')).toBe('unread');
    fireEvent.click(row);

    await waitFor(() => {
      expect(callsOf('notification_mark_read')).toHaveLength(1);
    });
    expect(callsOf('notification_mark_read')[0][1]).toEqual({ ids: ['run:r1:finished'] });
    expect(row.getAttribute('data-state')).toBe('read');
  });

  it('单条置已读失败：错误 inline 呈现且抽屉保持打开', async () => {
    behaviors.notification_list = () =>
      Promise.resolve({ items: [notification({ id: 'run:r1:finished' })], total: 1 });
    behaviors.notification_mark_read = () => Promise.reject(new Error('写失败'));
    mount();
    await openDrawer();

    const [row] = await screen.findAllByTestId('notification-item');
    fireEvent.click(row);

    const error = await screen.findByTestId('notification-error');
    expect(error.textContent).toBe('Error: 写失败');
    expect(screen.getByRole('heading', { name: '通知' }) !== null).toBe(true);
  });

  it('全部已读：命令一次、所有行灰态、徽标消失', async () => {
    behaviors.notification_subscribe = () => Promise.resolve(2);
    behaviors.notification_list = () =>
      Promise.resolve({
        items: [notification({ id: 'run:r1:finished' }), notification({ id: 'run:r2:finished' })],
        total: 2,
      });
    behaviors.notification_mark_all_read = () => Promise.resolve(0);
    mount();

    await screen.findByTestId('notification-badge');
    await openDrawer();
    await screen.findAllByTestId('notification-item');
    fireEvent.click(screen.getByRole('button', { name: '全部已读' }));

    await waitFor(() => {
      expect(callsOf('notification_mark_all_read')).toHaveLength(1);
    });
    await waitFor(() => {
      expect(
        screen
          .getAllByTestId('notification-item')
          .every((row) => row.getAttribute('data-state') === 'read'),
      ).toBe(true);
    });
    expect(screen.queryByTestId('notification-badge')).toBeNull();
  });

  it('全部已读失败：错误 inline、抽屉保持打开、徽标保持原值', async () => {
    behaviors.notification_subscribe = () => Promise.resolve(3);
    behaviors.notification_list = () =>
      Promise.resolve({ items: [notification({ id: 'run:r1:finished' })], total: 1 });
    behaviors.notification_mark_all_read = () => Promise.reject(new Error('写失败'));
    mount();

    const badge = await screen.findByTestId('notification-badge');
    expect(badge.textContent).toBe('3');
    await openDrawer();
    await screen.findAllByTestId('notification-item');
    fireEvent.click(screen.getByRole('button', { name: '全部已读' }));

    const error = await screen.findByTestId('notification-error');
    expect(error.textContent).toBe('Error: 写失败');
    expect(screen.getByRole('heading', { name: '通知' }) !== null).toBe(true);
    expect(screen.getByTestId('notification-badge').textContent).toBe('3');
  });
});

// ---------------------------------------------------------------------------
// 跨根跳转
// ---------------------------------------------------------------------------

describe('跨根条目的跳转顺序', () => {
  it('异根行先切根再跳转（切根发生在路由变更之前）', async () => {
    behaviors.notification_list = () =>
      Promise.resolve({
        items: [notification({ id: 'run:r9:finished', root: OTHER_ROOT, sourceRef: 'fix-bug' })],
        total: 1,
      });
    const { onSelect } = mount();
    let pathAtSelect: string | null = null;
    onSelect.mockImplementation(() => {
      pathAtSelect = pathRef;
    });

    await openDrawer();
    const [row] = await screen.findAllByTestId('notification-item');
    fireEvent.click(row);

    await waitFor(() => {
      expect(screen.getByTestId('location-probe').textContent).toBe('/changes/fix-bug');
    });
    expect(onSelect).toHaveBeenCalledWith(OTHER_ROOT);
    expect(pathAtSelect).toBe('/');
    expect(callsOf('add_workspace')).toHaveLength(0);
    expect(callsOf('remove_workspace')).toHaveLength(0);
  });

  it('同根行零切根直接跳转', async () => {
    behaviors.notification_list = () =>
      Promise.resolve({
        items: [notification({ id: 'run:r1:finished', root: ROOT, sourceRef: 'add-feature' })],
        total: 1,
      });
    const { onSelect } = mount();

    await openDrawer();
    const [row] = await screen.findAllByTestId('notification-item');
    fireEvent.click(row);

    await waitFor(() => {
      expect(screen.getByTestId('location-probe').textContent).toBe('/changes/add-feature');
    });
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('source_ref 缺位时零跳转零切根且仍可置已读，root 缺位则直接跳转', async () => {
    behaviors.notification_list = () =>
      Promise.resolve({
        items: [
          notification({ id: 'n-no-source', sourceRef: null, root: OTHER_ROOT }),
          notification({ id: 'n-no-root', sourceRef: 'add-feature', root: null }),
        ],
        total: 2,
      });
    const { onSelect } = mount();

    await openDrawer();
    const rows = await screen.findAllByTestId('notification-item');

    // source_ref 缺位：不跳转、不切根，但仍可置已读
    fireEvent.click(rows[0]);
    await waitFor(() => {
      expect(callsOf('notification_mark_read')).toHaveLength(1);
    });
    expect(screen.getByTestId('location-probe').textContent).toBe('/');
    expect(onSelect).not.toHaveBeenCalled();

    // root 缺位但有 source_ref：直接跳转（不切根）
    fireEvent.click(rows[1]);
    await waitFor(() => {
      expect(screen.getByTestId('location-probe').textContent).toBe('/changes/add-feature');
    });
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('超长与特殊字符主文案原样呈现在文档中', async () => {
    const long = '文'.repeat(1200);
    const special = '含换行\n与 emoji 🎉';
    behaviors.notification_list = () =>
      Promise.resolve({
        items: [
          notification({ id: 'n-long', message: long }),
          notification({ id: 'n-special', message: special }),
        ],
        total: 2,
      });
    mount();

    await openDrawer();
    const rows = await screen.findAllByTestId('notification-item');
    expect(rows.some((row) => row.textContent?.includes(long))).toBe(true);
    expect(rows.some((row) => row.textContent?.includes(special))).toBe(true);
  });
});
