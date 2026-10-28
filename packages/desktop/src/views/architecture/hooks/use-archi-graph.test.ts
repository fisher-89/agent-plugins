import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiGraph } from '../../../types/generated/bindings';
import { useArchiGraph } from './use-archi-graph';

/**
 * `useArchiGraph` 单测（AC-18 / D21）：三触发取数（挂载 / `root` 变化 / 手动
 * 刷新）、`root = null` 不发 invoke、错误面与卸载不写状态、零 watch 零轮询。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（生成的 `commands.archiGraph`
 * 最终经此调用），按命令名分发 fixture 并记录调用序列。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

/** 健康图 fixture（两元素一关系，零诊断）。 */
function healthyGraph(): ArchiGraph {
  return {
    elements: [
      {
        fqn: 'App',
        kind: 'package',
        gate: 'off',
        path: [],
        summary: null,
        metadata: {},
        modelFile: '.dev-team/architecture/models/01-core.toml',
        modelLine: 2,
        mark: 'none',
      },
      {
        fqn: 'App.Auth',
        kind: 'domain',
        gate: 'strict',
        path: ['./src/auth/'],
        summary: '认证域',
        metadata: { owner: 'core' },
        modelFile: '.dev-team/architecture/models/01-core.toml',
        modelLine: 6,
        mark: 'none',
      },
    ],
    relations: [
      {
        from: 'App.Auth',
        to: 'App',
        description: '认证依赖父包',
        mark: 'none',
        reason: null,
      },
    ],
    diagnostics: [],
  };
}

/** 停摆图 fixture（零图数据 + 一条诊断）。 */
function haltedGraph(): ArchiGraph {
  return {
    elements: [],
    relations: [],
    diagnostics: [
      {
        kind: 'orphanHierarchy',
        message: '孤儿层级：App.Auth 已声明，其祖先 App 未声明',
        file: '.dev-team/architecture/models/01-core.toml',
        line: 2,
        column: null,
      },
    ],
  };
}

/** 取数调用序列（命令名 + 入参）。 */
function calls(): { command: string; params?: Record<string, unknown> }[] {
  return invokeMock.mock.calls.map(([command, params]) => ({
    command: command as string,
    params: params as Record<string, unknown> | undefined,
  }));
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'archi_graph') return Promise.resolve(healthyGraph());
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
});

describe('useArchiGraph 取数触发', () => {
  it('应在挂载时以当前 root 发起一次 archi_graph 并落地图数据', async () => {
    const { result } = renderHook(() => useArchiGraph('C:\\demo\\alpha'));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.graph).toEqual(healthyGraph());
    expect(result.current.error).toBeNull();
    const graphCalls = calls().filter((call) => call.command === 'archi_graph');
    expect(graphCalls).toHaveLength(1);
    expect(graphCalls[0]?.params).toEqual({ root: 'C:\\demo\\alpha' });
  });

  it('应在 root 变化时以新 root 重发，且 root 为 null 时不发 invoke 并清空图', async () => {
    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useArchiGraph(root),
      { initialProps: { root: 'C:\\demo\\alpha' as string | null } },
    );
    await waitFor(() => expect(result.current.graph).not.toBeNull());

    rerender({ root: 'C:\\demo\\beta' });
    await waitFor(() => {
      const graphCalls = calls().filter((call) => call.command === 'archi_graph');
      expect(graphCalls).toHaveLength(2);
    });
    const lastCall = calls()
      .filter((call) => call.command === 'archi_graph')
      .at(-1);
    expect(lastCall?.params).toEqual({ root: 'C:\\demo\\beta' });

    rerender({ root: null });
    await waitFor(() => expect(result.current.graph).toBeNull());
    expect(calls().filter((call) => call.command === 'archi_graph')).toHaveLength(2);
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it('应在手动刷新时重发一次（连续两次刷新即两次 invoke，不做去重缓存）', async () => {
    const { result } = renderHook(() => useArchiGraph('C:\\demo\\alpha'));
    await waitFor(() => expect(result.current.graph).not.toBeNull());

    act(() => result.current.refresh());
    await waitFor(() => {
      expect(calls().filter((call) => call.command === 'archi_graph')).toHaveLength(2);
    });

    act(() => result.current.refresh());
    await waitFor(() => {
      expect(calls().filter((call) => call.command === 'archi_graph')).toHaveLength(3);
    });
  });
});

describe('useArchiGraph 竞态与陈旧窗口', () => {
  it('应在新根取数未回时把 graph 置空（不展示旧根图），并丢弃旧根迟到应答', async () => {
    const pending: ((graph: ArchiGraph) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiGraph>((resolve) => {
          pending.push(resolve);
        }),
    );
    const { result, rerender } = renderHook(({ root }: { root: string }) => useArchiGraph(root), {
      initialProps: { root: 'C:\\demo\\alpha' },
    });
    await act(async () => {
      pending[0]?.(healthyGraph());
    });
    await waitFor(() => expect(result.current.graph).not.toBeNull());

    rerender({ root: 'C:\\demo\\beta' });
    await waitFor(() =>
      expect(calls().filter((call) => call.command === 'archi_graph')).toHaveLength(2),
    );
    // 新根未回：不展示上一根的图
    expect(result.current.graph).toBeNull();

    await act(async () => {
      pending[1]?.(healthyGraph());
    });
    await waitFor(() => expect(result.current.graph).not.toBeNull());

    // 旧根应答迟到：不得回写
    await act(async () => {
      pending[0]?.(haltedGraph());
    });
    expect(result.current.graph?.diagnostics).toHaveLength(0);
    expect(result.current.loading).toBe(false);
  });

  it('应丢弃旧根的迟到失败（error 不被陈旧根污染）', async () => {
    const pending: {
      resolve: (graph: ArchiGraph) => void;
      reject: (reason: Error) => void;
    }[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiGraph>((resolve, reject) => {
          pending.push({ resolve, reject });
        }),
    );
    const { result, rerender } = renderHook(({ root }: { root: string }) => useArchiGraph(root), {
      initialProps: { root: 'C:\\demo\\alpha' },
    });
    rerender({ root: 'C:\\demo\\beta' });
    await waitFor(() =>
      expect(calls().filter((call) => call.command === 'archi_graph')).toHaveLength(2),
    );

    await act(async () => {
      pending[1]?.resolve(healthyGraph());
    });
    await waitFor(() => expect(result.current.graph).not.toBeNull());

    await act(async () => {
      pending[0]?.reject(new Error('旧根已失效'));
    });
    expect(result.current.error).toBeNull();
    expect(result.current.graph).not.toBeNull();
  });
});

describe('useArchiGraph 两态透出', () => {
  it('应原样透出停摆态（不裁剪诊断、不补默认值）', async () => {
    invokeMock.mockImplementation(() => Promise.resolve(haltedGraph()));
    const { result } = renderHook(() => useArchiGraph('C:\\demo\\alpha'));

    await waitFor(() => expect(result.current.graph).not.toBeNull());
    expect(result.current.graph).toEqual(haltedGraph());
    expect(result.current.graph?.diagnostics).toHaveLength(1);
    expect(result.current.error).toBeNull();
  });
});

describe('useArchiGraph 错误面与卸载', () => {
  it('应把 reject 收进 error 并清空图，随后刷新成功即清空 error', async () => {
    invokeMock.mockImplementationOnce(() => Promise.reject(new Error('root 无效')));
    const { result } = renderHook(() => useArchiGraph('   '));

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('root 无效');
    expect(result.current.graph).toBeNull();
    expect(result.current.loading).toBe(false);

    invokeMock.mockImplementationOnce(() => Promise.resolve(healthyGraph()));
    act(() => result.current.refresh());
    // 等待信号必须取「成功后才有」的状态：`error` 在 effect 起始即被同步清空，
    // 以它作信号会在成功分支落状态前就通过（顺序扰动下的竞态）。
    await waitFor(() => expect(result.current.graph).not.toBeNull());
    expect(result.current.graph).toEqual(healthyGraph());
    expect(result.current.error).toBeNull();
  });

  it('应在卸载后丢弃未完成的应答（不写状态）且 root 为 null 时刷新不发 invoke', async () => {
    let resolvePending: ((graph: ArchiGraph) => void) | undefined;
    invokeMock.mockImplementationOnce(
      () =>
        new Promise<ArchiGraph>((resolve) => {
          resolvePending = resolve;
        }),
    );
    const { unmount } = renderHook(() => useArchiGraph('C:\\demo\\alpha'));
    unmount();
    await act(async () => {
      resolvePending?.(healthyGraph());
    });
    expect(invokeMock).toHaveBeenCalledTimes(1);

    const nullRoot = renderHook(() => useArchiGraph(null));
    act(() => nullRoot.result.current.refresh());
    await waitFor(() => expect(nullRoot.result.current.loading).toBe(false));
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it('应在假计时器推进下保持零重发（无 watch 无轮询）', async () => {
    vi.useFakeTimers();
    try {
      renderHook(() => useArchiGraph('C:\\demo\\alpha'));
      await act(async () => {
        await Promise.resolve();
      });
      const before = calls().filter((call) => call.command === 'archi_graph').length;
      await act(async () => {
        vi.advanceTimersByTime(60_000);
      });
      const after = calls().filter((call) => call.command === 'archi_graph').length;
      expect(after).toBe(before);
    } finally {
      vi.useRealTimers();
    }
  });
});
