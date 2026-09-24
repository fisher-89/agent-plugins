// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { ChangeList, ExploreRecord, WorkspaceRecord } from '../types/dto';

// ---------------------------------------------------------------------------
// 集成关系「清单页 → 新建流程 → explore记录CRUD」（AC-8/AC-2）：render(<App />)
// 驱动真实 HashRouter，仅 invoke 按命令名 mock。核心验收点：
// - 清单只呈 store 绑定记录（按当前 workspace 过滤）；
// - 导入对话框只呈未绑定文件（求差在命令层，mock 返回求差后形态）；
// - 新话题建档只写 DB——invoke 序列中不得出现任何落盘类命令；
// - 新建记录详情 read_explore 为空 → 预览空态（应用不落盘 explore.md）。
// ---------------------------------------------------------------------------

const { ChannelMock, checkMock, getVersionMock, invokeMock, openMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
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

const fakeList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', inventory: 'v2', created: null, unparsable: false },
  ],
  archiveGroups: [],
};

function record(id: number, name: string): ExploreRecord {
  return {
    id,
    root: FIRST.root,
    name,
    createdAt: 1727000000000 + id,
    updatedAt: 1727000000000 + id,
  };
}

// ---------------------------------------------------------------------------
// 可切换 mock 行为
// ---------------------------------------------------------------------------

let records: ExploreRecord[];
let scanResult: { name: string; modifiedAt: number | null }[];
let createFailure: string | null;
let docResult: { name: string; content: string } | null;

function mockIpc() {
  records = [record(1, 'alpha')];
  scanResult = [];
  createFailure = null;
  docResult = null;
  invokeMock.mockImplementation(
    (command: string, params?: { root?: string; name?: string; newName?: string }) => {
      if (command === 'list_workspaces') {
        return Promise.resolve([FIRST]);
      }
      if (command === 'list_changes') {
        return Promise.resolve(fakeList);
      }
      if (command === 'get_change_detail') {
        return Promise.resolve(null);
      }
      if (command === 'list_explore_records') {
        return Promise.resolve(records.filter((item) => item.root === params?.root));
      }
      if (command === 'scan_explores') {
        return Promise.resolve(scanResult);
      }
      if (command === 'create_explore_record') {
        if (createFailure !== null) {
          return Promise.reject(new Error(createFailure));
        }
        const rec = record(records.length + 1, String(params?.name));
        records = [...records, rec];
        return Promise.resolve(rec);
      }
      if (command === 'rename_explore_record') {
        const target = records.find((item) => item.name === params?.name);
        if (!target) return Promise.reject(new Error('记录不存在'));
        return Promise.resolve({ ...target, name: String(params?.newName) });
      }
      if (command === 'delete_explore_record') {
        records = records.filter((item) => item.name !== params?.name);
        return Promise.resolve(true);
      }
      if (command === 'read_explore') {
        return Promise.resolve(docResult);
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(null);
      }
      if (command === 'watch_subscribe') {
        return Promise.resolve(1);
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
    },
  );
}

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

function commandsInvoked(): string[] {
  return invokeMock.mock.calls.map(([name]) => name);
}

/** 有记录启动并进入 /explores 清单页。 */
async function onExploreList() {
  render(<App />);
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
  );
  await act(async () => {
    fireEvent.click(screen.getByTestId('nav-explores'));
  });
  await waitFor(() => expect(window.location.hash).toBe('#/explores'));
  await waitFor(() => expect(screen.getByTestId('explore-list') !== null).toBe(true));
}

/** 打开新建对话框并切到导入页签，等待扫描候选渲染。 */
async function openImportTab() {
  fireEvent.click(screen.getByTestId('explore-create-toggle'));
  await waitFor(() => expect(screen.getByTestId('explore-create-tabs') !== null).toBe(true));
  fireEvent.click(screen.getByTestId('explore-tab-import'));
  await waitFor(() => expect(screen.queryByText('扫描中…')).toBeNull());
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
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// 清单呈现与新建流程（AC-8/AC-2）
// ---------------------------------------------------------------------------

describe('explore_pages：清单呈现 store 记录（AC-8）', () => {
  it('进入 /explores：清单以当前 root 发起 list_explore_records，呈绑定记录（含删除入口）', async () => {
    await onExploreList();

    expect(invokeMock).toHaveBeenCalledWith('list_explore_records', { root: FIRST.root });
    const items = screen.getAllByTestId('explore-item');
    expect(items).toHaveLength(1);
    expect(items[0].getAttribute('data-name')).toBe('alpha');
    expect(screen.getAllByTestId('explore-item-delete')).toHaveLength(1);
  });

  it('空清单 workspace：空态引导呈现（AC-8）', async () => {
    records = [];
    await onExploreList();

    expect(screen.getByTestId('explore-list-empty') !== null).toBe(true);
    expect(screen.queryAllByTestId('explore-item')).toHaveLength(0);
  });

  it('清单删除入口：发起 delete_explore_record 后清单刷新（不动磁盘文件由后端承担）', async () => {
    await onExploreList();

    fireEvent.click(screen.getByTestId('explore-item-delete'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('delete_explore_record', {
        root: FIRST.root,
        name: 'alpha',
      }),
    );
    await waitFor(() => expect(screen.queryByTestId('explore-item')).toBeNull());
  });
});

describe('explore_pages：导入扫描绑定（AC-8/AC-2）', () => {
  it('导入候选恰为未绑定差集（已绑定滤除）、选中即建档并刷新进详情', async () => {
    scanResult = [
      { name: 'beta', modifiedAt: 1727000000000 },
      { name: 'gamma', modifiedAt: 1727000001000 },
    ];
    await onExploreList();
    const listCallsBefore = commandsInvoked().filter(
      (name) => name === 'list_explore_records',
    ).length;

    await openImportTab();

    const candidates = screen.getAllByTestId('explore-import-item').map((item) => item.textContent);
    expect(candidates).toHaveLength(2);
    expect(candidates[0]).toContain('beta');
    expect(candidates[1]).toContain('gamma');
    expect(candidates.join()).not.toContain('alpha');

    fireEvent.click(screen.getAllByTestId('explore-import-item')[0]);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('create_explore_record', {
        root: FIRST.root,
        name: 'beta',
      }),
    );
    // 建档成功后清单刷新 + 导航进详情
    await waitFor(() =>
      expect(
        commandsInvoked().filter((name) => name === 'list_explore_records').length,
      ).toBeGreaterThan(listCallsBefore),
    );
    await waitFor(() => expect(window.location.hash).toBe('#/explores/beta'));
  });

  it('磁盘目录为空（scan_explores 空数组）：导入入口空态、无候选可建（AC-2）', async () => {
    await onExploreList();

    await openImportTab();

    expect(screen.getByTestId('explore-import-empty') !== null).toBe(true);
    expect(screen.queryAllByTestId('explore-import-item')).toHaveLength(0);
  });
});

describe('explore_pages：新话题建档不落盘（AC-8）', () => {
  it('新话题建档：仅发起 create_explore_record，调用序列无任何文件写入类命令', async () => {
    await onExploreList();
    const commandsBefore = commandsInvoked();

    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.change(screen.getByTestId('explore-topic-name'), {
      target: { value: 'retry-strategy' },
    });
    fireEvent.click(screen.getByTestId('explore-topic-create'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('create_explore_record', {
        root: FIRST.root,
        name: 'retry-strategy',
      }),
    );
    expect(invokeMock.mock.calls.filter(([name]) => name === 'create_explore_record')).toHaveLength(
      1,
    );
    // 应用零落盘：全程命令面只允许「清单/读/记录 CRUD/会话读」类命令
    const allowed = new Set([
      'list_workspaces',
      'list_changes',
      'get_change_detail',
      'list_explore_records',
      'create_explore_record',
      'rename_explore_record',
      'delete_explore_record',
      'scan_explores',
      'read_explore',
      'explore_doc_path',
      'watch_subscribe',
      'watch_unsubscribe',
      'agent_run_chain',
      'agent_run_events',
      'agent_start',
      'agent_runs',
    ]);
    const violations = commandsInvoked().filter((name) => !allowed.has(name));
    expect(violations).toEqual([]);
    expect(commandsBefore.length).toBeLessThan(commandsInvoked().length);
  });

  it('新建记录详情：read_explore 为空 → 预览空态呈现（不预建空档，AC-8）', async () => {
    await onExploreList();

    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.change(screen.getByTestId('explore-topic-name'), {
      target: { value: 'retry-strategy' },
    });
    fireEvent.click(screen.getByTestId('explore-topic-create'));

    await waitFor(() => expect(window.location.hash).toBe('#/explores/retry-strategy'));
    await waitFor(() => expect(screen.getByTestId('explore-detail') !== null).toBe(true));
    expect(invokeMock).toHaveBeenCalledWith('read_explore', {
      root: FIRST.root,
      name: 'retry-strategy',
    });
    await waitFor(() => expect(screen.getByTestId('preview-empty') !== null).toBe(true));
    expect(screen.queryByTestId('preview-doc')).toBeNull();
  });

  it('create_explore_record reject（重复名）：错误可见、对话框不崩、清单不被污染（AC-8）', async () => {
    createFailure = '记录已存在';
    await onExploreList();

    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.change(screen.getByTestId('explore-topic-name'), {
      target: { value: 'alpha' },
    });
    fireEvent.click(screen.getByTestId('explore-topic-create'));

    await waitFor(() =>
      expect(screen.getByTestId('explore-create-error')?.textContent).toContain('记录已存在'),
    );
    expect(screen.getAllByTestId('explore-item')).toHaveLength(1);
    expect(screen.getByTestId('explore-topic-name') !== null).toBe(true);
  });
});
