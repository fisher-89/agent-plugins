import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ExploreRecord } from '../../../types/dto';
import { useExploreList } from './useExploreList';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

// ---------------------------------------------------------------------------
// fixture（对齐 store::ExploreRecord serde camelCase）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';

function record(id: number, name: string, root = ROOT): ExploreRecord {
  return { id, root, name, createdAt: 1727000000000 + id, updatedAt: 1727000000000 + id };
}

beforeEach(() => {
  invokeMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useExploreList：清单取数纪律（AC-8）', () => {
  it('有效 root 挂载：以 { root } 发起 list_explore_records 恰一次，records 呈现返回序', async () => {
    const seeded = [record(1, 'api-retry'), record(2, 'layout-design')];
    invokeMock.mockResolvedValue(seeded);

    const { result } = renderHook(() => useExploreList(ROOT));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('list_explore_records', { root: ROOT });
    expect(result.current.records).toEqual(seeded);
    expect(result.current.error).toBeNull();
  });

  it('root 为 null：不发起任何取数，records 为空态', () => {
    const { result } = renderHook(() => useExploreList(null));

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.records).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('root 变更（A→B）：以新 root 重取，旧 root 的在途结果不覆盖新态', async () => {
    let resolveA: (value: ExploreRecord[]) => void = () => {};
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return new Promise<ExploreRecord[]>((resolve) => {
          resolveA = resolve;
        });
      }
      return Promise.resolve([record(9, 'b-record', 'C:\\demo\\b')]);
    });

    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useExploreList(root),
      { initialProps: { root: 'C:\\demo\\a' } },
    );
    await waitFor(() => expect(result.current.loading).toBe(true));

    rerender({ root: 'C:\\demo\\b' });
    await waitFor(() =>
      expect(result.current.records).toEqual([record(9, 'b-record', 'C:\\demo\\b')]),
    );

    // 旧 root 的在途结果迟到：不得覆盖新态（cancelled 抑制，ChangeList 同款竞态语义）
    resolveA([record(1, 'stale-a', 'C:\\demo\\a')]);
    await act(async () => {});
    expect(result.current.records).toEqual([record(9, 'b-record', 'C:\\demo\\b')]);
    const roots = invokeMock.mock.calls.map(([, params]) => (params as { root: string }).root);
    expect(roots).toEqual(['C:\\demo\\a', 'C:\\demo\\b']);
  });

  it('list_explore_records reject：error 置位、records 保持空态不崩', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));

    const { result } = renderHook(() => useExploreList(ROOT));

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.records).toEqual([]);
    expect(result.current.loading).toBe(false);
  });

  it('refresh 显式调用恰发一次取数；无 root 时 refresh 为 no-op', async () => {
    invokeMock.mockResolvedValue([record(1, 'api-retry')]);
    const { result } = renderHook(() => useExploreList(ROOT));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invokeMock).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('list_explore_records', { root: ROOT });

    const blank = renderHook(() => useExploreList(null));
    act(() => {
      blank.result.current.refresh();
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length).toBe(2);
  });
});

describe('useExploreList：动作薄封装（create / rename / remove 后刷新，AC-8）', () => {
  it('create：发起 create_explore_record，成功后以当前 root 重新取数、新记录入列', async () => {
    const initial = [record(1, 'api-retry')];
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') {
        return Promise.resolve(
          invokeMock.mock.calls.filter(([name]) => name === 'list_explore_records').length >= 2
            ? [...initial, record(2, 'new-topic')]
            : initial,
        );
      }
      if (command === 'create_explore_record') {
        return Promise.resolve(record(2, 'new-topic'));
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useExploreList(ROOT));
    await waitFor(() => expect(result.current.records).toEqual(initial));

    act(() => {
      result.current.create('new-topic');
    });

    await waitFor(() =>
      expect(result.current.records).toEqual([record(1, 'api-retry'), record(2, 'new-topic')]),
    );
    expect(invokeMock).toHaveBeenCalledWith('create_explore_record', {
      root: ROOT,
      name: 'new-topic',
    });
  });

  it('rename / remove：分别发起对应命令，成功后列表刷新反映变更', async () => {
    const initial = [record(1, 'old-name'), record(2, 'api-retry')];
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'list_explore_records') {
        const renamed = invokeMock.mock.calls.some(([name]) => name === 'rename_explore_record');
        const removed = invokeMock.mock.calls.some(([name]) => name === 'delete_explore_record');
        if (renamed && !removed)
          return Promise.resolve([record(1, 'new-name'), record(2, 'api-retry')]);
        if (renamed && removed) return Promise.resolve([record(1, 'new-name')]);
        return Promise.resolve(initial);
      }
      if (command === 'rename_explore_record') {
        return Promise.resolve(record(1, String(params?.newName)));
      }
      if (command === 'delete_explore_record') {
        return Promise.resolve(true);
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useExploreList(ROOT));
    await waitFor(() => expect(result.current.records).toEqual(initial));

    act(() => {
      result.current.rename('old-name', 'new-name');
    });
    await waitFor(() =>
      expect(result.current.records).toEqual([record(1, 'new-name'), record(2, 'api-retry')]),
    );
    expect(invokeMock).toHaveBeenCalledWith('rename_explore_record', {
      root: ROOT,
      name: 'old-name',
      newName: 'new-name',
    });

    act(() => {
      result.current.remove('api-retry');
    });
    await waitFor(() => expect(result.current.records).toEqual([record(1, 'new-name')]));
    expect(invokeMock).toHaveBeenCalledWith('delete_explore_record', {
      root: ROOT,
      name: 'api-retry',
    });
  });

  it('动作 reject：error 置位落 error 态，不中断页面（清单保持原值）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') {
        return Promise.resolve([record(1, 'api-retry')]);
      }
      if (command === 'create_explore_record') {
        return Promise.reject(new Error('记录已存在'));
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useExploreList(ROOT));
    await waitFor(() => expect(result.current.records).toHaveLength(1));

    act(() => {
      result.current.create('api-retry');
    });

    await waitFor(() => expect(result.current.error).toContain('记录已存在'));
    expect(result.current.records).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// 强化断言（变异补杀）：无 root 动作守卫、rename / remove 失败语义、root
// 切换过渡门闩与迟到现在语义。
// ---------------------------------------------------------------------------

describe('useExploreList：无 root 动作守卫与失败语义（AC-8）', () => {
  it('root 为 null：create / rename / remove 三个动作均为 no-op（零 IPC）', async () => {
    const { result } = renderHook(() => useExploreList(null));

    act(() => {
      result.current.create('topic');
      result.current.rename('topic', 'renamed');
      result.current.remove('topic');
    });
    await act(async () => {});

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('rename reject：error 置位（清单不被污染）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return Promise.resolve([record(1, 'old-name')]);
      if (command === 'rename_explore_record') return Promise.reject(new Error('新名已占用'));
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useExploreList(ROOT));
    await waitFor(() => expect(result.current.records).toHaveLength(1));

    act(() => {
      result.current.rename('old-name', 'clash');
    });

    await waitFor(() => expect(result.current.error).toContain('新名已占用'));
    expect(result.current.records).toEqual([record(1, 'old-name')]);
  });

  it('remove reject：error 置位（记录仍在清单）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return Promise.resolve([record(1, 'keep')]);
      if (command === 'delete_explore_record') return Promise.reject(new Error('删除失败'));
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useExploreList(ROOT));
    await waitFor(() => expect(result.current.records).toHaveLength(1));

    act(() => {
      result.current.remove('keep');
    });

    await waitFor(() => expect(result.current.error).toContain('删除失败'));
    expect(result.current.records).toEqual([record(1, 'keep')]);
  });
});

describe('useExploreList：root 切换过渡与迟到现在语义（AC-8）', () => {
  it('root 切换过渡轮：旧 root 记录不呈现（新 root 在途时为空态）', async () => {
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return Promise.resolve([record(1, 'a-record', 'C:\\demo\\a')]);
      }
      return new Promise<ExploreRecord[]>(() => {});
    });

    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useExploreList(root),
      { initialProps: { root: 'C:\\demo\\a' } },
    );
    await waitFor(() =>
      expect(result.current.records).toEqual([record(1, 'a-record', 'C:\\demo\\a')]),
    );

    rerender({ root: 'C:\\demo\\b' });
    await act(async () => {});

    expect(result.current.records).toEqual([]);
  });

  it('root 切换竞态：旧 root 迟到的 reject 不置错误、不污染新态', async () => {
    let rejectA: (err: unknown) => void = () => {};
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return new Promise<ExploreRecord[]>((_resolve, reject) => {
          rejectA = reject;
        });
      }
      return Promise.resolve([record(9, 'b-record', 'C:\\demo\\b')]);
    });

    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useExploreList(root),
      { initialProps: { root: 'C:\\demo\\a' } },
    );
    rerender({ root: 'C:\\demo\\b' });
    await waitFor(() =>
      expect(result.current.records).toEqual([record(9, 'b-record', 'C:\\demo\\b')]),
    );

    await act(async () => {
      rejectA(new Error('迟到的清单失败'));
    });
    expect(result.current.error).toBeNull();
    expect(result.current.records).toEqual([record(9, 'b-record', 'C:\\demo\\b')]);
  });
});
