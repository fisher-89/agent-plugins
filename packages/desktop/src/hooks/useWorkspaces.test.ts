import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { WorkspaceRecord } from '../types/dto';
import { useWorkspaces } from './useWorkspaces';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

/** 构造一份最小 WorkspaceRecord。 */
function record(root: string, lastOpenedAt = 2): WorkspaceRecord {
  const name = root.split(/[\\/]/).pop() ?? root;
  return { root, name, addedAt: 1, lastOpenedAt };
}

/** 可变"库存"：list_workspaces 返回当前库存，add/remove 同步修改库存。 */
let stored: WorkspaceRecord[];

/** 按命令名分发固定 fixture（沿用 useChangeList.test.ts 的替身模式）。 */
function mockDispatch() {
  invokeMock.mockImplementation((command: string, params?: { root?: string }) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([...stored]);
    }
    if (command === 'add_workspace') {
      const rec = record(params?.root ?? '', 3);
      stored = [...stored.filter((r) => r.root !== rec.root), rec];
      return Promise.resolve(rec);
    }
    if (command === 'remove_workspace') {
      stored = stored.filter((r) => r.root !== params?.root);
      return Promise.resolve(true);
    }
    if (command === 'touch_workspace') {
      return Promise.resolve(true);
    }
    return Promise.resolve(null);
  });
}

async function settleLoaded(): Promise<void> {
  await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('list_workspaces'));
}

/** mock.calls 中某命令的调用次数。 */
function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

describe('useWorkspaces：清单取数收口（挂载自动 load + add/remove/touch 内部刷新）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    stored = [];
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // 置于套件首位：守卫失效的病态循环会在其余用例（立即 resolve 的 mock）中
  // 微任务风暴式失控，须让本用例先运行、以稳定判据快速判失败
  it('恢复守卫恰一次：取数链稳定后 list_workspaces 恰 2 次、touch_workspace 恰 1 次', async () => {
    stored = [record('/repo/a'), record('/repo/b', 1)];
    // 每个响应经 setTimeout(0) 宏任务逐拍解析：若恢复守卫失效形成取数-恢复循环，
    // 循环每拍只走一轮而非微任务风暴，可被下方稳定判据观测到并快速判失败
    invokeMock.mockImplementation((command: string) => {
      const respond = (value: unknown) =>
        new Promise((resolve) => setTimeout(() => resolve(value), 0));
      if (command === 'list_workspaces') return respond([...stored]);
      return respond(true);
    });
    const { result } = renderHook(() => useWorkspaces());

    // 等待取数链稳定：连续两次轮询调用数不再增长（恢复守卫生效即停止循环）
    let previous = -1;
    await waitFor(
      () => {
        const current = invokeMock.mock.calls.length;
        if (current !== previous) {
          previous = current;
          throw new Error('取数链尚未稳定');
        }
      },
      { timeout: 3000, interval: 50 },
    );

    expect(result.current.root).toBe('/repo/a');
    // 挂载取数 + touch 成功后的内部刷新；恢复 touch 仅首次清单非空时一次
    expect(countOf('list_workspaces')).toBe(2);
    expect(countOf('touch_workspace')).toBe(1);
    expect(result.current.workspaces.map((r) => r.root)).toEqual(['/repo/a', '/repo/b']);
  });

  it('挂载即自动取数并恢复第一名：取数 → touch（先派发）→ 内部刷新，root 置为第一名', async () => {
    invokeMock.mockResolvedValue([record('/repo/a')]);
    const { result } = renderHook(() => useWorkspaces());

    await waitFor(() => expect(result.current.root).toBe('/repo/a'));
    // 挂载取数 → 恢复 touch → touch 成功后内部刷新清单，各恰一次
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(3));
    expect(invokeMock.mock.calls.map(([name]) => name)).toEqual([
      'list_workspaces',
      'touch_workspace',
      'list_workspaces',
    ]);
    expect(invokeMock).toHaveBeenCalledWith('touch_workspace', { root: '/repo/a' });
    expect(result.current.workspaces[0].root).toBe('/repo/a');
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('list_workspaces reject：error 置位含错误串、workspaces 保持空数组、不抛未捕获异常', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));
    const { result } = renderHook(() => useWorkspaces());

    await waitFor(() => expect(result.current.error).not.toBeNull());

    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.workspaces).toEqual([]);
    expect(result.current.loading).toBe(false);
  });

  it('add 成功：返回记录且内部刷新清单，首次非空清单触发恢复 touch 并打开新记录', async () => {
    mockDispatch();
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();
    // 挂载时清单为空：仅取数一次，无恢复 touch
    expect(invokeMock).toHaveBeenCalledTimes(1);

    let added: WorkspaceRecord | null = null;
    await act(async () => {
      added = await result.current.add('/repo/b');
    });

    expect(added).not.toBeNull();
    expect(added!.root).toBe('/repo/b');
    expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '/repo/b' });
    // add 1 次 + 内部刷新 1 次；刷新取得首个非空清单 → 恢复 touch 1 次 + 再刷新 1 次
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(5));
    expect(invokeMock).toHaveBeenLastCalledWith('list_workspaces');
    await waitFor(() => expect(result.current.workspaces.map((r) => r.root)).toContain('/repo/b'));
    expect(result.current.root).toBe('/repo/b');
  });

  it('add reject：返回 null 且 error 置位', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') return Promise.resolve([]);
      return Promise.reject(new Error('canonicalize: 无效路径'));
    });
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();

    let added: WorkspaceRecord | null = null;
    await act(async () => {
      added = await result.current.add('/nope');
    });

    expect(added).toBeNull();
    expect(result.current.error).toContain('canonicalize:');
  });

  it('add 传入空字符串 root：参数原样 invoke、失败置 error 不崩溃', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') return Promise.resolve([]);
      return Promise.reject(new Error('canonicalize: : 系统找不到指定的路径'));
    });
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();

    let added: WorkspaceRecord | null = null;
    await act(async () => {
      added = await result.current.add('');
    });

    expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '' });
    expect(added).toBeNull();
    expect(result.current.error).not.toBeNull();
  });

  it('remove 成功：返回 true 且清单内部刷新', async () => {
    mockDispatch();
    stored = [record('/repo/a')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.workspaces).toHaveLength(1));

    let hit: boolean | undefined;
    await act(async () => {
      hit = await result.current.remove('/repo/a');
    });

    expect(hit).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('remove_workspace', { root: '/repo/a' });
    expect(invokeMock).toHaveBeenLastCalledWith('list_workspaces');
    await waitFor(() => expect(result.current.workspaces).toHaveLength(0));
  });

  it('remove resolve(false)（store miss）：返回 false 且不置 error', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') return Promise.resolve([record('/repo/a')]);
      if (command === 'remove_workspace') return Promise.resolve(false);
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.workspaces).toHaveLength(1));

    let hit: boolean | undefined;
    await act(async () => {
      hit = await result.current.remove('/repo/a');
    });

    expect(hit).toBe(false);
    expect(result.current.error).toBeNull();
    // 幂等 miss 非错误，但内部刷新仍发生
    // （挂载取数 + 挂载恢复 touch 刷新 + 移除后刷新 = 3 次）
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([name]) => name === 'list_workspaces')).toHaveLength(3),
    );
  });

  it('remove reject：返回 false 且 error 置位', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') return Promise.resolve([record('/repo/a')]);
      return Promise.reject(new Error('db: 打开失败'));
    });
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.workspaces).toHaveLength(1));

    let hit: boolean | undefined;
    await act(async () => {
      hit = await result.current.remove('/repo/a');
    });

    expect(hit).toBe(false);
    expect(result.current.error).toContain('db: 打开失败');
  });

  it('touch 成功：返回 true 且清单内部刷新', async () => {
    mockDispatch();
    stored = [record('/repo/a')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.workspaces).toHaveLength(1));

    let hit: boolean | undefined;
    await act(async () => {
      hit = await result.current.touch('/repo/a');
    });

    expect(hit).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('touch_workspace', { root: '/repo/a' });
    expect(invokeMock).toHaveBeenLastCalledWith('list_workspaces');
  });

  it('touch reject：返回 false 且 error 置位，不阻塞后续 add 动作', async () => {
    mockDispatch();
    stored = [];
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();

    invokeMock.mockImplementationOnce(() => Promise.reject(new Error('db: 写入失败')));
    let hit: boolean | undefined;
    await act(async () => {
      hit = await result.current.touch('/repo/a');
    });

    expect(hit).toBe(false);
    expect(result.current.error).toContain('db: 写入失败');

    let added: WorkspaceRecord | null = null;
    await act(async () => {
      added = await result.current.add('/repo/b');
    });
    expect(added).not.toBeNull();
    expect(added!.root).toBe('/repo/b');
  });

  it('无轮询无 watch：推进虚拟计时数分钟后 invoke 次数不增长', async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue([]);
    const { result } = renderHook(() => useWorkspaces());
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledTimes(1);

    act(() => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(result.current.workspaces).toEqual([]);
  });

  it('loading 状态转换：取数进行中为 true，清单返回后复位 false 且无错误', async () => {
    let resolveLoad: (records: WorkspaceRecord[]) => void = () => {};
    invokeMock.mockImplementation(
      (command: string) =>
        new Promise<WorkspaceRecord[]>((resolve) => {
          if (command === 'list_workspaces') resolveLoad = resolve;
        }),
    );
    const { result } = renderHook(() => useWorkspaces());

    // 挂载取数进行中：loading 置位（挂载 effect 显式 setLoading(true)）
    expect(result.current.loading).toBe(true);

    await act(async () => {
      resolveLoad([record('/repo/a')]);
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.root).toBe('/repo/a');
  });

  it('卸载取消：清单在卸载后才返回时不触发恢复 touch，也不再取数', async () => {
    let resolveLoad: (records: WorkspaceRecord[]) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') {
        return new Promise<WorkspaceRecord[]>((resolve) => {
          resolveLoad = resolve;
        });
      }
      return Promise.resolve(true);
    });
    const { unmount } = renderHook(() => useWorkspaces());
    unmount();

    await act(async () => {
      resolveLoad([record('/repo/a')]);
    });

    // 取消后迟到响应被丢弃：不 touch、不内部刷新取数
    expect(countOf('touch_workspace')).toBe(0);
    expect(countOf('list_workspaces')).toBe(1);
  });
});
