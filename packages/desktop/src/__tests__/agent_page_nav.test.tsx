// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { ChangeDetail, ChangeList, WorkspaceRecord } from '../types/dto';

// ---------------------------------------------------------------------------
// 集成关系五：侧栏页面导航组 → App 顶层视图切换（无路由）与 change 域状态
// 共存（D10 选中重置、useChangeList 留 App 层、两入口语义不串扰）。
//
// 进程边界 Mock 沿 App.test.tsx 既有装置：invoke 按命令名分发并记录调用
// 序列（次数断言依赖记录）；matchMedia / 窗口视口 stub。
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
// fixture 与装置（与 App.test.tsx 同源口径）
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

const fakeDetail: ChangeDetail = {
  name: 'add-feature',
  source: 'active',
  inventory: 'v2',
  created: null,
  unparsable: false,
  pipeline: [],
  activePhase: null,
  interrupted: [],
  fileLog: [],
  artifacts: [],
};

let remaining: WorkspaceRecord[];

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
  invokeMock.mockImplementation((command: string) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([...remaining]);
    }
    if (command === 'list_changes') {
      return Promise.resolve(fakeList);
    }
    if (command === 'get_change_detail') {
      return Promise.resolve(fakeDetail);
    }
    return Promise.resolve(null);
  });
}

async function restored() {
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

describe('agent_page_nav：导航切换与 change 域状态共存', () => {
  it('点击「Agent 调试」→ AgentDebugView 呈现且 ChangeView 卸载；点击「变更」→ 切回清单', async () => {
    await restored();

    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-agent'));
    });
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));
    expect(screen.queryByText('add-feature')).toBeNull();

    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-changes'));
    });
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
  });

  it('导航点击不触发任何 workspace 命令（list_workspaces / 动作命令调用次数不变）', async () => {
    await restored();

    const listBefore = countOf('list_workspaces');
    const addBefore = countOf('add_workspace');
    const removeBefore = countOf('remove_workspace');
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-agent'));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-changes'));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-agent'));
    });

    expect(countOf('list_workspaces')).toBe(listBefore);
    expect(countOf('add_workspace')).toBe(addBefore);
    expect(countOf('remove_workspace')).toBe(removeBefore);
  });

  it('进入 change 详情 → 切 Agent 页 → 切回：选中重置回清单、get_change_detail 不以旧选中重发（D10）', async () => {
    await restored();

    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );

    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-agent'));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-changes'));
    });

    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
    expect(countOf('get_change_detail')).toBe(1);
  });

  it('切页往返不重发 list_workspaces / list_changes（清单数据驻留 App 层，查询仍显式触发）', async () => {
    await restored();

    const listWorkspacesBefore = countOf('list_workspaces');
    const listChangesBefore = countOf('list_changes');
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-agent'));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('nav-changes'));
    });

    expect(countOf('list_workspaces')).toBe(listWorkspacesBefore);
    expect(countOf('list_changes')).toBe(listChangesBefore);
    expect(screen.getByText('add-feature') !== null).toBe(true);
  });

  it('无已选 workspace（root=null）进入 Agent 页：欢迎屏持有、导航不在场、启动禁用语义由页面自持、App 不崩', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(screen.queryByTestId('nav-agent')).toBeNull();
    expect(screen.queryByTestId('nav-changes')).toBeNull();
    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
  });
});
