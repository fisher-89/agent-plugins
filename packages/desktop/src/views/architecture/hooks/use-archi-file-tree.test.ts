import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiFileTree } from '../../../types/generated/bindings';
import { useArchiFileTree } from './use-archi-file-tree';

/**
 * `useArchiFileTree` 单测（AC-12 / AC-13 / D12）：三触发取数（挂载 / `root` 变化 /
 * 上层 `key` 重挂）、`root = null` 零 invoke、失败与换根都清空树、零 watch 零轮询。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（生成的 `commands.archiFileTree`
 * 最终经此调用），按命令名分发 fixture 并记录调用序列。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

/** 文件树 fixture（三条含五种标记代表值的扁平条目）。 */
function fileTree(tag: string): ArchiFileTree {
  return {
    entries: [
      {
        path: `crates/core/${tag}`,
        name: tag,
        kind: 'dir',
        mark: 'elementRoot',
        owners: ['App.Core.Archi'],
        contains: 1,
        exists: true,
      },
      {
        path: `crates/core/${tag}/src`,
        name: 'src',
        kind: 'dir',
        mark: 'member',
        owners: ['App.Core.Archi'],
        contains: 0,
        exists: true,
      },
      {
        path: `crates/core/${tag}/src/lib.rs`,
        name: 'lib.rs',
        kind: 'file',
        mark: 'ambiguous',
        owners: ['App.Core.Archi', 'App.Core.Other'],
        contains: 0,
        exists: true,
      },
    ],
  };
}

/** `archi_file_tree` 取数调用序列（入参）。 */
function calls(): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'archi_file_tree')
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'archi_file_tree') return Promise.resolve(fileTree('alpha'));
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
});

describe('useArchiFileTree 取数触发', () => {
  it('应在挂载时以当前 root 发起一次 archi_file_tree 并落地树数据', async () => {
    const { result } = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.tree).toEqual(fileTree('alpha'));
    expect(result.current.error).toBeNull();
    expect(calls()).toHaveLength(1);
    expect(calls()[0]).toEqual({ root: 'C:\\demo\\alpha' });
  });

  it('应在 root 变化时以新 root 重发并把树换成新根的 fixture', async () => {
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command !== 'archi_file_tree') {
        return Promise.reject(new Error(`未预期命令: ${command}`));
      }
      const root = typeof params?.root === 'string' ? params.root : '';
      return Promise.resolve(fileTree(root === 'B' ? 'beta' : 'alpha'));
    });
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiFileTree(root),
      {
        initialProps: { root: 'A' },
      },
    );
    await waitFor(() => expect(result.current.tree).not.toBeNull());

    rerender({ root: 'B' });
    await waitFor(() => expect(calls()).toHaveLength(2));
    await waitFor(() => expect(result.current.tree).toEqual(fileTree('beta')));
    expect(calls().at(-1)).toEqual({ root: 'B' });
  });

  it('应在卸载后重新挂载（上层 key 重挂）时再发一次，两次载荷相同', async () => {
    const first = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
    await waitFor(() => expect(first.result.current.tree).not.toBeNull());
    first.unmount();

    const second = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
    await waitFor(() => expect(second.result.current.tree).not.toBeNull());

    expect(calls()).toHaveLength(2);
    expect(calls()[0]).toEqual({ root: 'C:\\demo\\alpha' });
    expect(calls()[1]).toEqual({ root: 'C:\\demo\\alpha' });
  });
});

describe('useArchiFileTree 空根与返回面', () => {
  it('应在 root 为 null 时零 invoke 并复位三成员，随后换到有效根再取数', async () => {
    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useArchiFileTree(root),
      { initialProps: { root: null as string | null } },
    );

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.tree).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();

    rerender({ root: 'A' });
    await waitFor(() => expect(result.current.tree).not.toBeNull());
    expect(calls()).toEqual([{ root: 'A' }]);
  });

  it('应在返回值里恰有三个成员且零 refresh（刷新归上层 key 重挂）', async () => {
    const { result } = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
    await waitFor(() => expect(result.current.tree).not.toBeNull());

    expect(Object.keys(result.current).sort()).toEqual(['error', 'loading', 'tree']);
    expect(result.current).not.toHaveProperty('refresh');
    expect('refresh' in result.current).toBe(false);
  });
});

describe('useArchiFileTree 失败面与陈旧应答', () => {
  it('应在换根后取数失败时清空旧树并把原因落 error', async () => {
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command !== 'archi_file_tree') {
        return Promise.reject(new Error(`未预期命令: ${command}`));
      }
      return params?.root === 'B'
        ? Promise.reject(new Error('root 无效'))
        : Promise.resolve(fileTree('alpha'));
    });
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiFileTree(root),
      {
        initialProps: { root: 'A' },
      },
    );
    await waitFor(() => expect(result.current.tree).not.toBeNull());

    rerender({ root: 'B' });
    await waitFor(() => expect(result.current.error).not.toBeNull());
    // 旧 A 的树 MUST NOT 冒充本次结果
    expect(result.current.tree).toBeNull();
    expect(result.current.error).toContain('root 无效');
    expect(result.current.loading).toBe(false);
  });

  it('应在失败后重挂成功时清空 error 并落地新树', async () => {
    invokeMock.mockImplementationOnce(() => Promise.reject(new Error('root 无效')));
    const first = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
    await waitFor(() => expect(first.result.current.error).not.toBeNull());
    expect(first.result.current.tree).toBeNull();
    first.unmount();

    invokeMock.mockImplementationOnce(() => Promise.resolve(fileTree('alpha')));
    const second = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
    // 等待信号取「成功后才有」的 tree：`error` 在 effect 起始即被同步清空
    await waitFor(() => expect(second.result.current.tree).not.toBeNull());
    expect(second.result.current.error).toBeNull();
  });

  it('应在新根未回时把树置空（不展示旧根树），新根落定后再给出', async () => {
    const pending: ((tree: ArchiFileTree) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiFileTree>((resolve) => {
          pending.push(resolve);
        }),
    );
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiFileTree(root),
      {
        initialProps: { root: 'A' },
      },
    );
    await act(async () => {
      pending[0]?.(fileTree('alpha'));
    });
    await waitFor(() => expect(result.current.tree).not.toBeNull());

    rerender({ root: 'B' });
    await waitFor(() => expect(calls()).toHaveLength(2));
    expect(result.current.tree).toBeNull();

    await act(async () => {
      pending[1]?.(fileTree('beta'));
    });
    await waitFor(() => expect(result.current.tree).toEqual(fileTree('beta')));
  });

  it('应丢弃旧根的迟到应答（不回写旧树）', async () => {
    const pending: ((tree: ArchiFileTree) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiFileTree>((resolve) => {
          pending.push(resolve);
        }),
    );
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiFileTree(root),
      {
        initialProps: { root: 'A' },
      },
    );
    rerender({ root: 'B' });
    await waitFor(() => expect(calls()).toHaveLength(2));

    await act(async () => {
      pending[1]?.(fileTree('beta'));
    });
    await waitFor(() => expect(result.current.tree).toEqual(fileTree('beta')));

    // 旧根应答迟到：不得覆盖已落定的新树
    await act(async () => {
      pending[0]?.(fileTree('alpha'));
    });
    expect(result.current.tree).toEqual(fileTree('beta'));
  });

  it('应在卸载后丢弃未完成的应答（不写状态）', async () => {
    let resolvePending: ((tree: ArchiFileTree) => void) | undefined;
    invokeMock.mockImplementationOnce(
      () =>
        new Promise<ArchiFileTree>((resolve) => {
          resolvePending = resolve;
        }),
    );
    const { unmount } = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
    unmount();
    await act(async () => {
      resolvePending?.(fileTree('alpha'));
    });
    expect(calls()).toHaveLength(1);
  });
});

describe('useArchiFileTree 零轮询与规模边界', () => {
  it('应在假计时器推进一分钟后保持零重发（无 watch 无轮询）', async () => {
    vi.useFakeTimers();
    try {
      renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
      await act(async () => {
        await Promise.resolve();
      });
      const before = calls().length;
      await act(async () => {
        vi.advanceTimersByTime(60_000);
      });
      expect(calls()).toHaveLength(before);
    } finally {
      vi.useRealTimers();
    }
  });

  it('应把空树原样透出（空数组是成功态，不是未载入）', async () => {
    invokeMock.mockImplementation(() => Promise.resolve({ entries: [] }));
    const { result } = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.tree).not.toBeNull();
    expect(result.current.tree?.entries).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  it('应在首轮未回时给出 loading 且树为 null（「解析中…」的派生前提）', async () => {
    invokeMock.mockImplementation(() => new Promise<ArchiFileTree>(() => {}));
    const { result } = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));

    await waitFor(() => expect(result.current.loading).toBe(true));
    expect(result.current.tree).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('应对超大条目列表零裁剪，并对空串 root 原样透传', async () => {
    const entries = Array.from({ length: 1000 }, (_unused, index) => ({
      path: `src/m${String(index).padStart(4, '0')}/f.ts`,
      name: 'f.ts',
      kind: 'file' as const,
      mark: 'none' as const,
      owners: [] as string[],
      contains: 0,
      exists: true,
    }));
    invokeMock.mockImplementation(() => Promise.resolve({ entries }));

    const { result } = renderHook(() => useArchiFileTree('C:\\demo\\alpha'));
    await waitFor(() => expect(result.current.tree).not.toBeNull());
    expect(result.current.tree?.entries).toHaveLength(1000);

    const empty = renderHook(() => useArchiFileTree(''));
    await waitFor(() => expect(empty.result.current.tree).not.toBeNull());
    // 是否 Err 由后端裁，hook 零前置校验：载荷逐字透传
    expect(calls().at(-1)).toEqual({ root: '' });
  });
});
