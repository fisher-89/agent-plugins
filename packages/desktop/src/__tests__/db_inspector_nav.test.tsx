// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { ChangeList, ModelInfo, RecordEnvelope, WorkspaceRecord } from '../types/dto';

// ---------------------------------------------------------------------------
// 集成关系：侧栏系统工具组 → App 顶层切换 → DbInspectorView 挂载（AC-6）。
// 「组归属 + 切换语义 + 欢迎态不可达」横跨 Sidebar 与 App 两层，只有整树组合
// 才能断言「欢迎态无该组 DOM」与「切页不触发 change 取数」。
//
// 进程边界 Mock 沿 agent_page_nav.test.tsx 既有装置：invoke 按命令名分发并
// 记录调用序列；matchMedia / 窗口视口 stub 同源。
// ---------------------------------------------------------------------------

const { checkMock, getVersionMock, invokeMock, openMock } = vi.hoisted(() => ({
  checkMock: vi.fn(),
  getVersionMock: vi.fn(),
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: checkMock }));

// ---------------------------------------------------------------------------
// fixture 与装置（与 App.test.tsx 同源口径；db 命令回数组形态防 hook 置 null）
// ---------------------------------------------------------------------------

const FIRST: WorkspaceRecord = {
  root: 'C:\\demo\\alpha',
  name: 'alpha',
  addedAt: 1,
};
const SECOND: WorkspaceRecord = {
  root: 'C:\\demo\\beta',
  name: 'beta',
  addedAt: 1,
};

const fakeList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', inventory: 'v2', created: null, unparsable: false },
  ],
  archiveGroups: [],
};

const DB_MODELS: ModelInfo[] = [
  { name: 'workspace', count: 2 },
  { name: 'agent_run', count: 1 },
  { name: 'agent_event', count: 0 },
];
const DB_RECORDS: RecordEnvelope[] = [
  { key: FIRST.root, value: { root: FIRST.root, name: 'alpha', addedAt: 1 } },
];

let remaining: WorkspaceRecord[];
let dbModelsReject: string | null = null;

let restoreViewport: () => void = () => {};

function stubViewport() {
  const widthDescriptor = Object.getOwnPropertyDescriptor(window, 'innerWidth');
  Object.defineProperty(window, 'innerWidth', {
    configurable: true,
    writable: true,
    value: 1100,
  });
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    writable: true,
    value: vi.fn((query: string) => ({
      matches: window.innerWidth < 768,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    })),
  });
  restoreViewport = () => {
    if (widthDescriptor) Object.defineProperty(window, 'innerWidth', widthDescriptor);
    Reflect.deleteProperty(window, 'matchMedia');
  };
}

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

function mockIpc() {
  remaining = [FIRST, SECOND];
  dbModelsReject = null;
  invokeMock.mockImplementation((command: string, params?: { offset?: number; limit?: number }) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([...remaining]);
    }
    if (command === 'list_changes') {
      return Promise.resolve(fakeList);
    }
    if (command === 'db_models') {
      if (dbModelsReject !== null) return Promise.reject(new Error(dbModelsReject));
      return Promise.resolve(DB_MODELS.map((model) => ({ ...model })));
    }
    if (command === 'db_records') {
      const offset = params?.offset ?? 0;
      const limit = params?.limit ?? 50;
      return Promise.resolve(DB_RECORDS.slice(offset, offset + limit).map((e) => ({ ...e })));
    }
    return Promise.resolve(null);
  });
}

async function restored() {
  window.location.hash = '#/changes';
  render(<App />);
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
  );
  await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
}

beforeEach(() => {
  getVersionMock.mockReset();
  invokeMock.mockReset();
  openMock.mockReset();
  checkMock.mockReset();
  checkMock.mockResolvedValue(null);
  getVersionMock.mockResolvedValue('0.1.0');
  toast.dismiss();
  mockIpc();
  stubViewport();
});

afterEach(() => {
  restoreViewport();
  cleanup();
});

describe('db_inspector_nav：壳态入口与切换共存', () => {
  it('壳态点击 nav-db → DbInspectorView 渲染且 db_models 取数发起（进页即用户显式动作）', async () => {
    await restored();

    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-db'));
    });
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
    expect(invokeMock).toHaveBeenCalledWith('db_models');
    await waitFor(() => expect(screen.getAllByTestId('db-model-item')).toHaveLength(3));
    expect(screen.getByTestId('nav-db').getAttribute('data-active')).toBe('true');
  });

  it('changes → db → agent → changes 往返切换：各域视图与 workspace 清单共存不串扰', async () => {
    await restored();

    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-db'));
    });
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2); // 清单共存

    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-agent'));
    });
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));
    expect(screen.queryByTestId('db-model-list')).toBeNull(); // db 视图卸载
    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);

    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-changes'));
    });
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);
  });

  it('切至 db 页再切回 changes：list_changes / list_workspaces 调用次数不增长（切页不触发重取）', async () => {
    await restored();

    const listChangesBefore = countOf('list_changes');
    const listWorkspacesBefore = countOf('list_workspaces');

    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-db'));
    });
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-changes'));
    });
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(countOf('list_changes')).toBe(listChangesBefore);
    expect(countOf('list_workspaces')).toBe(listWorkspacesBefore);
  });

  it('db 页挂载时 db_models reject：错误仅 inline 于查看页，侧栏与切页不受影响', async () => {
    await restored();

    dbModelsReject = 'db: 清单打开失败';
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-db'));
    });
    const errorNote = await screen.findByTestId('db-inspector-error');
    expect(errorNote.textContent).toContain('db: 清单打开失败');
    // 页面级错误不冒泡为全局崩溃：无 toast、侧栏在位
    expect(document.querySelector('[data-sonner-toast]')).toBeNull();
    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);
    expect(screen.getByTestId('nav-db').getAttribute('data-active')).toBe('true');

    // 切页链路照常可用
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-agent'));
    });
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));
    expect(screen.queryByTestId('db-inspector-error')).toBeNull();
  });

  it('欢迎态（root=null）DOM 中无「系统工具」组标签与 nav-db（AC-6 欢迎态无该组 DOM）', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(screen.queryByText('系统工具')).toBeNull();
    expect(screen.queryByTestId('nav-db')).toBeNull();
    expect(screen.queryByTestId('nav-agent')).toBeNull();
    expect(screen.queryByTestId('db-model-list')).toBeNull();
  });
});
