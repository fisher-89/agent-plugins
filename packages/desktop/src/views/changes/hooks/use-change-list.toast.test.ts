import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeList } from '../../../types/dto';
import { useChangeList } from './use-change-list';

const { invokeMock, toastMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastMock: {
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

// toast 轨：hook 不渲染 DOM，只断言调用参数（沿 use-workspaces.test.ts 先例）
vi.mock('sonner', () => ({ toast: toastMock }));

/** 清单 fixture：一条 active（title ≠ name，证明 toast 用 title）。 */
const LIST: ChangeList = {
  active: [
    {
      id: 'chg-1',
      name: 'add-feature',
      title: '新增特性（人类可读）',
      source: 'active',
      status: 'active',
      activePhase: null,
      created: null,
    },
  ],
  archiveGroups: [
    {
      month: '2026-09',
      changes: [
        {
          id: 'chg-archived',
          name: 'old',
          title: '归档标题',
          source: 'archive',
          status: 'archived',
          activePhase: null,
          created: null,
        },
      ],
    },
  ],
};

/** 按命令名分发：list_changes 恒回报 LIST，delete_change 走可编程应答。 */
let deleteAnswer: () => Promise<unknown>;

function dispatch(command: string): Promise<unknown> {
  if (command === 'list_changes') return Promise.resolve(LIST);
  if (command === 'delete_change') return deleteAnswer();
  return Promise.resolve(null);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => dispatch(command));
  for (const fn of Object.values(toastMock)) fn.mockReset();
  deleteAnswer = () =>
    Promise.resolve({ name: 'add-feature', removedWorktree: true, warnings: [] });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('useChangeList.remove：toast 轨', () => {
  it('无警告 → toast.success 携条目标题（title 而非 name）并刷新清单', async () => {
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());
    const listCallsBefore = invokeMock.mock.calls.filter(([cmd]) => cmd === 'list_changes').length;

    await act(async () => {
      await result.current.remove('chg-1', true);
    });

    expect(toastMock.success).toHaveBeenCalledWith('已删除变更「新增特性（人类可读）」');
    expect(toastMock.warning).not.toHaveBeenCalled();
    expect(invokeMock).toHaveBeenCalledWith('delete_change', {
      root: '/repo',
      id: 'chg-1',
      cleanWorktree: true,
    });
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([cmd]) => cmd === 'list_changes').length).toBe(
        listCallsBefore + 1,
      ),
    );
  });

  it('携警告（worktree 保留）→ toast.warning 携标题与注记 description', async () => {
    const warning = 'worktree 目录与分支 change/add-feature 保留（未勾选清空）: C:/wt/add-feature';
    deleteAnswer = () =>
      Promise.resolve({ name: 'add-feature', removedWorktree: false, warnings: [warning] });
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    await act(async () => {
      await result.current.remove('chg-1', false);
    });

    expect(toastMock.success).not.toHaveBeenCalled();
    expect(toastMock.warning).toHaveBeenCalledWith('已删除变更「新增特性（人类可读）」', {
      description: warning,
    });
  });

  it('归档条目按分组内标题呈现（归档行同样可删）', async () => {
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    await act(async () => {
      await result.current.remove('chg-archived', false);
    });

    expect(toastMock.success).toHaveBeenCalledWith('已删除变更「归档标题」');
  });

  it('清单未载入（data=null）→ 标题回落 id 呈现，不阻断删除', async () => {
    // list_changes 挂起不落定：data 保持 null，删除仍可发起
    invokeMock.mockImplementation((command: string, params: unknown) => {
      if (command === 'list_changes') return new Promise(() => {});
      if (command === 'delete_change') {
        expect(params).toEqual({ root: '/repo', id: 'ghost-id', cleanWorktree: false });
        return Promise.resolve({ name: 'ghost-id', removedWorktree: false, warnings: [] });
      }
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useChangeList('/repo'));

    await act(async () => {
      await result.current.remove('ghost-id', false);
    });

    expect(toastMock.success).toHaveBeenCalledWith('已删除变更「ghost-id」');
  });

  it('root 为 null → 零 invoke、零 toast（无 workspace 无删除语义）', async () => {
    const { result } = renderHook(() => useChangeList(null));

    await act(async () => {
      await result.current.remove('ghost-id', false);
    });

    expect(invokeMock.mock.calls.some(([cmd]) => cmd === 'delete_change')).toBe(false);
    expect(toastMock.success).not.toHaveBeenCalled();
  });

  it('删除 reject → 零 toast（错误由确认弹窗行内呈现）、向外 rethrow、清单不刷新', async () => {
    deleteAnswer = () => Promise.reject(new Error('正在运行中，请先停止运行'));
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());
    const listCallsBefore = invokeMock.mock.calls.filter(([cmd]) => cmd === 'list_changes').length;

    await act(async () => {
      await expect(result.current.remove('chg-1', false)).rejects.toThrow('正在运行中');
    });

    expect(toastMock.success).not.toHaveBeenCalled();
    expect(toastMock.warning).not.toHaveBeenCalled();
    expect(toastMock.error).not.toHaveBeenCalled();
    expect(invokeMock.mock.calls.filter(([cmd]) => cmd === 'list_changes').length).toBe(
      listCallsBefore,
    );
  });
});
