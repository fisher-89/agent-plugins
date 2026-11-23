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
      created: '2026-05-01',
      archivedAt: null,
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
          created: '2026-04-28',
          archivedAt: '2026-04-29',
        },
      ],
    },
  ],
};

/** 按命令名分发：list_changes 恒回报 LIST，delete_change 走可编程应答。 */
let deleteAnswer: () => Promise<unknown>;

/** 反馈落库失败注入位（`notification_record` reject 形态）。 */
let recordReject = false;

function dispatch(command: string): Promise<unknown> {
  if (command === 'list_changes') return Promise.resolve(LIST);
  if (command === 'delete_change') return deleteAnswer();
  if (command === 'notification_record') {
    return recordReject ? Promise.reject(new Error('落库失败')) : Promise.resolve(null);
  }
  return Promise.resolve(null);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => dispatch(command));
  for (const fn of Object.values(toastMock)) fn.mockReset();
  recordReject = false;
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

// ---------------------------------------------------------------------------
// 通知门面旁路（desktop-notification-hub AC-2 校准 3）：toast 形态逐字不变 +
// `notification_record` 一次（warning 档带副文案 / success 档无副文案）
// ---------------------------------------------------------------------------

/** `notification_record` 的落库入参序列。 */
function recordCalls(): { level: string; message: string; body: string | null }[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === 'notification_record')
    .map(
      ([, params]) =>
        (params as { record: { level: string; message: string; body: string | null } }).record,
    );
}

describe('useChangeList.remove：门面落库旁路', () => {
  it('携警告 → toast.warning 带 description 且落库 warning 档 body 为该串', async () => {
    const warning = 'worktree 目录与分支 change/add-feature 保留（未勾选清空）: C:/wt/add-feature';
    deleteAnswer = () =>
      Promise.resolve({ name: 'add-feature', removedWorktree: false, warnings: [warning] });
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    await act(async () => {
      await result.current.remove('chg-1', false);
    });

    expect(toastMock.warning).toHaveBeenCalledWith('已删除变更「新增特性（人类可读）」', {
      description: warning,
    });
    expect(recordCalls()).toHaveLength(1);
    expect(recordCalls()[0].level).toBe('warning');
    expect(recordCalls()[0].body).toBe(warning);
    expect(recordCalls()[0].message).toBe('已删除变更「新增特性（人类可读）」');
  });

  it('无警告 → toast.success 单参且落库 success 档 body 为 null', async () => {
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    await act(async () => {
      await result.current.remove('chg-1', true);
    });

    expect(toastMock.success).toHaveBeenCalledWith('已删除变更「新增特性（人类可读）」');
    expect(recordCalls()).toHaveLength(1);
    expect(recordCalls()[0].level).toBe('success');
    expect(recordCalls()[0].body).toBeNull();
  });

  it('空警告集 → 走成功档（不产出空 description）', async () => {
    deleteAnswer = () =>
      Promise.resolve({ name: 'add-feature', removedWorktree: true, warnings: [] });
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    await act(async () => {
      await result.current.remove('chg-1', true);
    });

    expect(toastMock.warning).not.toHaveBeenCalled();
    expect(toastMock.success).toHaveBeenCalledWith('已删除变更「新增特性（人类可读）」');
    expect(toastMock.success.mock.calls[0]).toHaveLength(1);
    expect(recordCalls()[0].body).toBeNull();
  });

  it('异常：落库 reject 时既有 toast 断言仍成立、清单刷新不变', async () => {
    recordReject = true;
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());
    const listCallsBefore = invokeMock.mock.calls.filter(([cmd]) => cmd === 'list_changes').length;

    await act(async () => {
      await result.current.remove('chg-1', true);
    });

    expect(toastMock.success).toHaveBeenCalledWith('已删除变更「新增特性（人类可读）」');
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([cmd]) => cmd === 'list_changes').length).toBe(
        listCallsBefore + 1,
      ),
    );
    expect(recordCalls()).toHaveLength(1);
  });
});
