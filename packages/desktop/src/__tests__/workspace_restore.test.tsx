// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { ChangeDetail, ChangeList, WorkspaceRecord } from '../types/dto';

// ---------------------------------------------------------------------------
// 进程边界 Mock：IPC（invoke）与对话框（open）；链路内模块间调用一律真实实现。
// 断言焦点：useWorkspaces 启动取数（内含恢复 touch）→ workspace 命令面 的
// invoke 时序、次数与参数契约（视图状态断言归 App.test.tsx 单测）。
// mock 对齐后端 store 语义：touch_workspace 刷新 last_opened_at，
// list_workspaces 结果按 last_opened_at 降序返回。
// ---------------------------------------------------------------------------

const { invokeMock, openMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

const FIRST: WorkspaceRecord = {
  root: 'C:\\demo\\beta',
  name: 'beta',
  addedAt: 1,
  lastOpenedAt: 900,
};
const SECOND: WorkspaceRecord = {
  root: 'C:\\demo\\alpha',
  name: 'alpha',
  addedAt: 1,
  lastOpenedAt: 100,
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

// ---------------------------------------------------------------------------
// 可切换的 mock 行为与按命令名分发
// ---------------------------------------------------------------------------

let remaining: WorkspaceRecord[];
let listReject: string | null = null;
let touchReject: string | null = null;
let addRecord: WorkspaceRecord | null = null;
let clock = 1000;

function mockIpc() {
  remaining = [FIRST, SECOND];
  listReject = null;
  touchReject = null;
  addRecord = null;
  clock = 1000;
  invokeMock.mockImplementation((command: string, params?: { root?: string; change?: string }) => {
    if (command === 'list_workspaces') {
      if (listReject !== null) return Promise.reject(new Error(listReject));
      return Promise.resolve([...remaining]);
    }
    if (command === 'touch_workspace') {
      if (touchReject !== null) return Promise.reject(new Error(touchReject));
      // 对齐后端：刷新 last_opened_at（单调时钟），清单随之降序重排
      remaining = remaining
        .map((r) => (r.root === params?.root ? { ...r, lastOpenedAt: ++clock } : r))
        .sort((a, b) => b.lastOpenedAt - a.lastOpenedAt);
      return Promise.resolve(true);
    }
    if (command === 'remove_workspace') {
      remaining = remaining.filter((r) => r.root !== params?.root);
      return Promise.resolve(true);
    }
    if (command === 'add_workspace') {
      const rec: WorkspaceRecord = addRecord ?? {
        root: params?.root ?? '',
        name: 'picked',
        addedAt: 1,
        lastOpenedAt: 2,
      };
      remaining = [...remaining, rec];
      return Promise.resolve(rec);
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

/** mock.calls 中某命令的调用次数。 */
function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

beforeEach(() => {
  invokeMock.mockReset();
  openMock.mockReset();
  mockIpc();
});

// ---------------------------------------------------------------------------
// 场景：启动恢复链
// ---------------------------------------------------------------------------

describe('启动恢复链：list_workspaces → touch（fire-and-forget）→ list_changes', () => {
  it('有记录启动：自动恢复 last_opened_at 第一名为当前根，list_changes 收到 { root: 第一名.root }', async () => {
    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );

    const names = invokeMock.mock.calls.map(([name]) => name);
    // 挂载自动取数恰一次；touch 成功后 hook 内部刷新清单再取一次（D2）
    expect(countOf('list_workspaces')).toBe(2);
    expect(invokeMock).toHaveBeenCalledWith('touch_workspace', { root: FIRST.root });
    expect(countOf('touch_workspace')).toBe(1);
    // 时序：挂载取数 → touch → 以恢复根取列表
    expect(names.indexOf('touch_workspace')).toBeGreaterThan(names.indexOf('list_workspaces'));
    expect(names.indexOf('list_changes')).toBeGreaterThan(names.indexOf('touch_workspace'));
  });

  it('touch 失败（reject）不阻断 list_changes 取数（D3 fire-and-forget）', async () => {
    touchReject = 'db: touch 失败';
    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    expect(countOf('touch_workspace')).toBe(1);
  });

  it('list_workspaces reject：无 touch、无 list_changes，error 呈现且欢迎屏手动添加路径仍可用', async () => {
    listReject = 'db: 打开失败';
    render(<App />);

    await waitFor(() => expect(document.querySelector('.error-note') !== null).toBe(true));

    expect(countOf('touch_workspace')).toBe(0);
    expect(countOf('list_changes')).toBe(0);
    // 恢复失败不阻断手动添加：入口存在且点击触发对话框
    const addEntry = screen.getByText('添加新文件夹');
    expect(addEntry !== null).toBe(true);
    openMock.mockResolvedValue('C:\\recovered');
    fireEvent.click(addEntry);
    await waitFor(() => expect(openMock).toHaveBeenCalled());
  });
});

// ---------------------------------------------------------------------------
// 场景：无记录停欢迎屏
// ---------------------------------------------------------------------------

describe('无记录停欢迎屏', () => {
  it('空清单启动：不触发 touch 与 list_changes，呈现欢迎屏与「添加新文件夹」入口', async () => {
    remaining = [];
    render(<App />);

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    const names = invokeMock.mock.calls.map(([name]) => name);
    expect(names).toEqual(['list_workspaces']);
    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 场景：移除当前项（切换剩余第一名，不二次恢复）
// ---------------------------------------------------------------------------

describe('移除当前项：切换到剩余第一名且不二次恢复', () => {
  it('移除当前打开项：remove_workspace 调用后以剩余第一名为新根重取列表', async () => {
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );

    fireEvent.click(screen.getByText('移除'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('remove_workspace', { root: FIRST.root }),
    );
    await waitFor(() => {
      const lastListChanges = invokeMock.mock.calls
        .filter(([name]) => name === 'list_changes')
        .at(-1);
      expect(lastListChanges).toEqual(['list_changes', { root: SECOND.root }]);
    });
    // 停留列表视图（下拉仅剩剩余第一名），不回欢迎屏
    expect(screen.queryByText('添加新文件夹')).toBeNull();
    const values = [...screen.getByRole('combobox').querySelectorAll('option')].map((option) =>
      option.getAttribute('value'),
    );
    expect(values).toEqual([SECOND.root]);
  });

  it('移除后 hook 内部刷新取得剩余清单：不发生第二次自动恢复 touch', async () => {
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    expect(countOf('list_changes')).toBe(1);
    expect(countOf('touch_workspace')).toBe(1);

    fireEvent.click(screen.getByText('移除'));

    // 移除触发 hook 内部刷新：挂载 1 次 + touch 刷新 1 次 + 移除刷新 1 次 = 3 次
    await waitFor(() => expect(countOf('list_workspaces')).toBe(3));
    // 切换到剩余第一名：list_changes 恰追加一次且以新根取数
    await waitFor(() => expect(countOf('list_changes')).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('list_changes', { root: SECOND.root });
    // 恢复恰一次：无第二次 touch
    expect(countOf('touch_workspace')).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// 场景：添加与下拉切换链
// ---------------------------------------------------------------------------

describe('添加与下拉切换链', () => {
  it('欢迎屏添加：dialog resolve 路径 → add_workspace → 以返回记录的 canonical root 触发 list_changes', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    // 返回记录的 canonical root 与对话框原串书写不等价（模拟 canonicalize）
    addRecord = { root: 'C:\\raw\\picked dir', name: 'picked dir', addedAt: 1, lastOpenedAt: 2 };
    openMock.mockResolvedValue('/raw/PICKED DIR');
    fireEvent.click(screen.getByText('添加新文件夹'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '/raw/PICKED DIR' }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: 'C:\\raw\\picked dir' }),
    );
  });

  it('下拉切换另一项：touch_workspace(新根) + change 选中清空 + list_changes 以新根重取', async () => {
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    // 先打开一个 change，验证切换后选中被清空
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );

    fireEvent.change(screen.getByRole('combobox'), { target: { value: SECOND.root } });

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('touch_workspace', { root: SECOND.root }),
    );
    await waitFor(() => {
      const lastListChanges = invokeMock.mock.calls
        .filter(([name]) => name === 'list_changes')
        .at(-1);
      expect(lastListChanges).toEqual(['list_changes', { root: SECOND.root }]);
    });
    expect(countOf('touch_workspace')).toBe(2);
    // 选中清空：不以新根重发 get_change_detail
    const detailCalls = invokeMock.mock.calls.filter(([name]) => name === 'get_change_detail');
    expect(detailCalls.every(([, params]) => params?.root === FIRST.root)).toBe(true);
  });

  it('dialog 取消（返回 null）：不调用 add_workspace、停留欢迎屏', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    openMock.mockResolvedValue(null);
    fireEvent.click(screen.getByText('添加新文件夹'));

    await waitFor(() => expect(openMock).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(countOf('add_workspace')).toBe(0);
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);
  });
});
