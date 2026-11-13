import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiApiCheck } from '../../../types/generated/bindings';
import { useArchiApiCheck } from './use-archi-api-check';

/**
 * `useArchiApiCheck` 单测（AC-12）：三触发取数（挂载 / `root` 变化 / 手动 `refresh()`）、
 * `root === null` 零 invoke、失败清旧数据只落行内 `error`、新 `root` 未回时旧结果不冒充、
 * 迟到应答不回写、卸载后不写状态、零 watch 零轮询、返回键集恰四成员、空差异与超大差异
 * 纯透传。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（生成的 `commands.archiApiCheck` 最终
 * 经此调用），按命令名 `archi_api_check` 分发 fixture 并记录调用序列与载荷。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

/** 五字段 DTO 夹具（三列缺省空、`enabled` 缺省 true）。 */
function apiCheck(overrides: Partial<ArchiApiCheck> = {}): ArchiApiCheck {
  return {
    enabled: true,
    meta: { toolVersion: '0.6.40', commonDigest: `sha256:${'a'.repeat(64)}` },
    mismatched: [],
    orphaned: [],
    missing: [],
    ...overrides,
  };
}

/** 取数调用序列（命令名 + 入参）。 */
function calls(): { command: string; params?: Record<string, unknown> }[] {
  return invokeMock.mock.calls.map(([command, params]) => ({
    command: command as string,
    params: params as Record<string, unknown> | undefined,
  }));
}

function checkCalls() {
  return calls().filter((call) => call.command === 'archi_api_check');
}

/**
 * 挂起的应答队列（手工 resolve / reject，用于竞态与 loading 断言）。
 *
 * 只改实现、**不 reset 调用记录**：`mockReset` 会连已发生的 `mock.calls` 一并清掉，
 * 让「第几次 invoke 对应哪一次挂起」的偏移判据错位（调用计数与应答序即被测面）。
 */
function deferred(): {
  resolvers: ((value: ArchiApiCheck) => void)[];
  rejecters: ((reason: unknown) => void)[];
} {
  const resolvers: ((value: ArchiApiCheck) => void)[] = [];
  const rejecters: ((reason: unknown) => void)[] = [];
  invokeMock.mockImplementation(
    () =>
      new Promise<ArchiApiCheck>((resolve, reject) => {
        resolvers.push(resolve);
        rejecters.push(reject);
      }),
  );
  return { resolvers, rejecters };
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'archi_api_check') return Promise.resolve(apiCheck());
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
});

describe('useArchiApiCheck 三触发', () => {
  it('应在挂载时发起一次 archi_api_check 并落地五字段 DTO', async () => {
    const { result } = renderHook(() => useArchiApiCheck('/ws/alpha'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(checkCalls()).toHaveLength(1);
    expect(checkCalls()[0]?.params).toEqual({ root: '/ws/alpha' });
    expect(result.current.check).toEqual(apiCheck());
    expect(result.current.error).toBeNull();
  });

  it('应在 root 变化时以新 root 重发一次', async () => {
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiApiCheck(root),
      { initialProps: { root: '/ws/alpha' } },
    );
    await waitFor(() => expect(result.current.check).not.toBeNull());

    rerender({ root: '/ws/beta' });
    await waitFor(() => expect(checkCalls()).toHaveLength(2));
    expect(checkCalls()[1]?.params).toEqual({ root: '/ws/beta' });
    await waitFor(() => expect(result.current.check).not.toBeNull());
  });

  it('应在 refresh() 时重发一次（载荷同 root，loading 期间为 true）', async () => {
    const { result } = renderHook(() => useArchiApiCheck('/ws/alpha'));
    await waitFor(() => expect(result.current.check).not.toBeNull());
    const baseline = checkCalls().length;

    const pending = deferred();
    act(() => result.current.refresh());
    await act(async () => {
      await Promise.resolve();
    });
    expect(checkCalls()).toHaveLength(baseline + 1);
    expect(checkCalls()[baseline]?.params).toEqual({ root: '/ws/alpha' });
    expect(result.current.loading).toBe(true);

    await act(async () => {
      pending.resolvers[0]?.(apiCheck());
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
  });
});

describe('useArchiApiCheck 无根与复位', () => {
  it('应在 root 为 null 时零 invoke 且四个成员复位', async () => {
    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useArchiApiCheck(root),
      { initialProps: { root: null as string | null } },
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(checkCalls()).toHaveLength(0);
    expect(result.current.check).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();

    rerender({ root: '/ws' });
    await waitFor(() => expect(checkCalls()).toHaveLength(1));
  });

  it('应在 root 由值转 null 时清空上一轮结果（旧差异 MUST NOT 残留）', async () => {
    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useArchiApiCheck(root),
      { initialProps: { root: '/ws/alpha' as string | null } },
    );
    await waitFor(() => expect(result.current.check).not.toBeNull());

    rerender({ root: null });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.check).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(checkCalls()).toHaveLength(1);
  });
});

describe('useArchiApiCheck 失败面与竞态', () => {
  it('应在失败时清空旧数据并只落行内 error', async () => {
    const { result } = renderHook(() => useArchiApiCheck('/ws/alpha'));
    await waitFor(() => expect(result.current.check).not.toBeNull());

    const pending = deferred();
    act(() => result.current.refresh());
    await act(async () => {
      pending.rejecters[0]?.(new Error('模型结构错'));
    });

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.check).toBeNull();
    expect(result.current.error).toContain('模型结构错');
  });

  it('应在失败后刷新成功时清空 error', async () => {
    invokeMock.mockImplementationOnce(() => Promise.reject(new Error('首次失败')));
    invokeMock.mockImplementation(() => Promise.resolve(apiCheck()));
    const { result } = renderHook(() => useArchiApiCheck('/ws/alpha'));
    await waitFor(() => expect(result.current.error).not.toBeNull());

    act(() => result.current.refresh());
    await waitFor(() => expect(result.current.check).not.toBeNull());
    expect(result.current.error).toBeNull();
  });

  it('应在新 root 未回时保持 check 为 null（旧结果不冒充）', async () => {
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiApiCheck(root),
      { initialProps: { root: '/ws/alpha' } },
    );
    await waitFor(() => expect(result.current.check).not.toBeNull());

    const pending = deferred();
    rerender({ root: '/ws/beta' });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.check).toBeNull();

    await act(async () => {
      pending.resolvers[0]?.(apiCheck({ mismatched: ['beta.i.rs'] }));
    });
    await waitFor(() => expect(result.current.check?.mismatched).toEqual(['beta.i.rs']));
  });

  it('应在旧 root 迟到应答时保留新 root 的结果（cancelled 旗标拦旧应答）', async () => {
    const pending = deferred();
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiApiCheck(root),
      { initialProps: { root: '/ws/alpha' } },
    );
    await waitFor(() => expect(checkCalls()).toHaveLength(1));

    rerender({ root: '/ws/beta' });
    await waitFor(() => expect(checkCalls()).toHaveLength(2));

    await act(async () => {
      pending.resolvers[1]?.(apiCheck({ mismatched: ['beta.i.rs'] }));
    });
    await waitFor(() => expect(result.current.check?.mismatched).toEqual(['beta.i.rs']));

    await act(async () => {
      pending.resolvers[0]?.(apiCheck({ mismatched: ['alpha.i.rs'] }));
    });
    expect(result.current.check?.mismatched).toEqual(['beta.i.rs']);
  });

  it('应在卸载后丢弃挂起应答（不写状态、不报错）', async () => {
    const pending = deferred();
    const { unmount } = renderHook(() => useArchiApiCheck('/ws/alpha'));
    await waitFor(() => expect(checkCalls()).toHaveLength(1));

    unmount();
    await act(async () => {
      pending.resolvers[0]?.(apiCheck());
    });
    expect(checkCalls()).toHaveLength(1);
  });
});

describe('useArchiApiCheck 纪律面', () => {
  it('应在 60 秒假时钟推进后零新增 invoke（零轮询零 watch）', async () => {
    vi.useFakeTimers();
    try {
      renderHook(() => useArchiApiCheck('/ws/alpha'));
      await act(async () => {
        await Promise.resolve();
      });
      const baseline = checkCalls().length;
      expect(baseline).toBe(1);

      await act(async () => {
        vi.advanceTimersByTime(60_000);
      });
      expect(checkCalls()).toHaveLength(baseline);
    } finally {
      vi.useRealTimers();
    }
  });

  it('应暴露恰四成员（check / error / loading / refresh）', async () => {
    const { result } = renderHook(() => useArchiApiCheck('/ws/alpha'));
    await waitFor(() => expect(result.current.check).not.toBeNull());
    expect(Object.keys(result.current).sort()).toEqual(['check', 'error', 'loading', 'refresh']);
  });
});

describe('useArchiApiCheck 透传边界', () => {
  it.each([
    ['空差异（成功态）', apiCheck()],
    [
      '三列各 1000 条',
      apiCheck({
        mismatched: Array.from({ length: 1000 }, (_, index) => `m${index}.i.rs`),
        orphaned: Array.from({ length: 1000 }, (_, index) => `o${index}.i.ts`),
        missing: Array.from({ length: 1000 }, (_, index) => `x${index}.schema.json`),
      }),
    ],
  ])('应原样透传 %s（零裁剪）', async (_tag, fixture) => {
    invokeMock.mockImplementation(() => Promise.resolve(fixture));
    const { result } = renderHook(() => useArchiApiCheck('/ws/alpha'));
    await waitFor(() => expect(result.current.check).not.toBeNull());

    expect(result.current.check).toEqual(fixture);
    expect(result.current.check?.mismatched).toHaveLength(fixture.mismatched.length);
    expect(result.current.check?.orphaned).toHaveLength(fixture.orphaned.length);
    expect(result.current.check?.missing).toHaveLength(fixture.missing.length);
  });

  it.each([
    ['空串', ''],
    ['超长（>1000）', 'x'.repeat(1200)],
    ['含 emoji', '/ws/契约🎯'],
  ])('应把 root = %s 逐字透传给命令（零前置校验）', async (_tag, root) => {
    const { result } = renderHook(() => useArchiApiCheck(root));
    await waitFor(() => expect(checkCalls()).toHaveLength(1));
    expect(checkCalls()[0]?.params).toEqual({ root });
    await waitFor(() => expect(result.current.check).not.toBeNull());
  });

  it('应在 root = undefined（越签名脏值）时按非 null 分支 invoke 一次（钉死实际行为）', async () => {
    const { result } = renderHook(() => useArchiApiCheck(undefined as unknown as null));
    await waitFor(() => expect(checkCalls()).toHaveLength(1));
    expect(checkCalls()[0]?.params).toEqual({ root: undefined });
    await waitFor(() => expect(result.current.check).not.toBeNull());
  });
});
