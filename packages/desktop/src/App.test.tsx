import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeDetail, ChangeList, WorkspaceRecord } from './types/dto';

const { getVersionMock, invokeMock, openMock } = vi.hoisted(() => ({
  getVersionMock: vi.fn(),
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));

import App from './App';

// ---------------------------------------------------------------------------
// fixture：workspace 清单按 last_opened_at 降序排列（FIRST 即"最近打开"第一名）
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
// 进程边界 Mock：IPC 按命令名分发；文件夹对话框按用例 resolve / reject
// ---------------------------------------------------------------------------

let remaining: WorkspaceRecord[];
let addBehavior: 'ok' | 'reject' = 'ok';
let removeReject: string | null = null;
let clock = 1000;

function mockIpc() {
  remaining = [FIRST, SECOND];
  addBehavior = 'ok';
  removeReject = null;
  clock = 1000;
  invokeMock.mockImplementation((command: string, params?: { root?: string; change?: string }) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([...remaining]);
    }
    if (command === 'touch_workspace') {
      // 对齐后端：刷新 last_opened_at（单调时钟），清单随之降序重排
      remaining = remaining
        .map((r) => (r.root === params?.root ? { ...r, lastOpenedAt: ++clock } : r))
        .sort((a, b) => b.lastOpenedAt - a.lastOpenedAt);
      return Promise.resolve(true);
    }
    if (command === 'remove_workspace') {
      if (removeReject !== null) return Promise.reject(new Error(removeReject));
      remaining = remaining.filter((r) => r.root !== params?.root);
      return Promise.resolve(true);
    }
    if (command === 'add_workspace') {
      if (addBehavior === 'reject') {
        return Promise.reject(new Error('canonicalize: 入库失败'));
      }
      const rec: WorkspaceRecord = {
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

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

/** 有记录启动：等待自动恢复第一名进入列表视图（列表已渲染）。 */
async function restored() {
  render(<App />);
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
  );
  await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
}

describe('App：启动恢复、欢迎屏清单与视图状态（AC-9）', () => {
  beforeEach(() => {
    getVersionMock.mockReset();
    invokeMock.mockReset();
    openMock.mockReset();
    // useUpdater 挂载拉取版本号：走独立 mock，不混入 invoke 调用序列断言
    getVersionMock.mockResolvedValue('0.1.0');
    mockIpc();
  });

  it('list_workspaces 返回空清单启动：呈现欢迎屏空态与「添加新文件夹」入口，不进入列表视图', async () => {
    remaining = [];
    render(<App />);

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
    expect(screen.getByText(/选择一个项目根目录/) !== null).toBe(true);
    // 欢迎屏清单标题与计数（D7：空清单计数为 0）
    expect(screen.getByText(/最近的 workspace/).textContent).toContain('(0)');
    expect(screen.queryByText('刷新列表')).toBeNull();
    // 除挂载自动 load 外无任何取数
    expect(invokeMock.mock.calls.every(([name]) => name === 'list_workspaces')).toBe(true);
  });

  it('欢迎屏加载中：挂载取数未返回时呈现「加载中…」，清单返回后消失', async () => {
    let resolveLoad: (records: WorkspaceRecord[]) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') {
        return new Promise<WorkspaceRecord[]>((resolve) => {
          resolveLoad = resolve;
        });
      }
      return Promise.resolve(null);
    });
    render(<App />);

    await waitFor(() => expect(screen.getByText('加载中…') !== null).toBe(true));
    await act(async () => {
      resolveLoad([]);
    });
    await waitFor(() => expect(screen.queryByText('加载中…')).toBeNull());
    // 加载结束仍未有根：停留欢迎屏空态
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);
  });

  it('有记录启动：自动恢复 last_opened_at 第一名为当前根并进入列表视图', async () => {
    await restored();

    expect(screen.getByText(/进行中/) !== null).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('touch_workspace', { root: FIRST.root });
  });

  it('Header 下拉切换：change 选中清空、以新根重取列表、清单项悬停 title 含完整 path', async () => {
    await restored();
    // 先进入详情视图，验证切换后选中被清空
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );

    const optionAlpha = screen.getByRole('option', { name: 'alpha' });
    expect(optionAlpha.getAttribute('title')).toBe(SECOND.root);
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
    // change 选中清空：详情视图退出回到列表视图，且不以新根重发 get_change_detail
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    const detailCalls = invokeMock.mock.calls.filter(([name]) => name === 'get_change_detail');
    expect(detailCalls.length).toBeGreaterThan(0);
    expect(detailCalls.every(([, params]) => params?.root === FIRST.root)).toBe(true);
  });

  it('移除当前打开的清单项：切换到剩余第一名，被移除项从下拉消失', async () => {
    await restored();

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
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    // 停留列表视图，不回欢迎屏；下拉仅剩剩余第一名
    expect(screen.queryByText('添加新文件夹')).toBeNull();
    const values = screen.getAllByRole('option').map((option) => option.getAttribute('value'));
    expect(values).toEqual([SECOND.root]);
  });

  it('添加对话框取消（返回 null）：不调用 add_workspace、停留欢迎屏', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    openMock.mockResolvedValue(null);
    fireEvent.click(screen.getByText('添加新文件夹'));

    await waitFor(() => expect(openMock).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(invokeMock.mock.calls.every(([name]) => name === 'list_workspaces')).toBe(true);
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);
  });

  it('添加对话框 reject 或 add_workspace reject：error-note 呈现、停留欢迎屏、可重试', async () => {
    remaining = []; // 必须先于 render：挂载即自动取数
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    // 分支一：对话框调用 reject —— 保持欢迎屏可重试
    openMock.mockRejectedValue(new Error('对话框崩溃'));
    fireEvent.click(screen.getByText('添加新文件夹'));
    await waitFor(() => expect(openMock).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);

    // 分支二：add_workspace reject —— error-note 呈现、停留欢迎屏
    openMock.mockResolvedValue('C:\\picked');
    addBehavior = 'reject';
    fireEvent.click(screen.getByText('添加新文件夹'));
    await waitFor(() => expect(screen.getByTestId('error-note') !== null).toBe(true));
    expect(screen.getByText('添加新文件夹') !== null).toBe(true);

    // 可重试：再次添加成功后以返回记录的 root 进入列表视图
    addBehavior = 'ok';
    fireEvent.click(screen.getByText('添加新文件夹'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: 'C:\\picked' }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: 'C:\\picked' }),
    );
    expect(screen.queryByText('添加新文件夹')).toBeNull();
  });

  it('恢复进入列表后：列表项点击进入详情视图、刷新按钮重发 list_changes', async () => {
    await restored();

    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );
    expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true);

    const before = invokeMock.mock.calls.filter(([name]) => name === 'list_changes').length;
    fireEvent.click(screen.getByText('刷新列表'));
    await waitFor(() => {
      const after = invokeMock.mock.calls.filter(([name]) => name === 'list_changes').length;
      expect(after).toBe(before + 1);
    });
  });

  it('Header 下拉在清单非空时保持可操作（未禁用）', async () => {
    await restored();

    const select = screen.getByRole('combobox');
    expect(select.hasAttribute('disabled')).toBe(false);
  });

  it('Header error-note：无错误时不渲染，workspace 动作失败后呈现错误文本且停留列表视图', async () => {
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    // 无错误：error-note 一个都不渲染（空 span 也不允许）
    expect(screen.queryAllByTestId('error-note')).toHaveLength(0);

    removeReject = 'db: 移除失败';
    fireEvent.click(screen.getByText('移除'));

    await waitFor(() => expect(screen.getByTestId('error-note') !== null).toBe(true));
    expect(screen.getByTestId('error-note').textContent).toContain('db: 移除失败');
    // 错误呈现不切换视图：仍停留列表视图
    expect(screen.getByRole('combobox').tagName).toBe('SELECT');
    expect(screen.queryByText('添加新文件夹')).toBeNull();
  });

  it('添加流以标准参数调用文件夹对话框，并将选中路径入库后打开', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    openMock.mockResolvedValue('C:\\picked');
    fireEvent.click(screen.getByText('添加新文件夹'));

    // 对话框参数契约：目录模式 + 单选
    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith({ directory: true, multiple: false }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: 'C:\\picked' }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: 'C:\\picked' }),
    );
    expect(screen.queryByText('添加新文件夹')).toBeNull();
  });
});
