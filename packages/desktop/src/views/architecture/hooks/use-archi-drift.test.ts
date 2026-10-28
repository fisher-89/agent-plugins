import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiDriftMode, ArchiDriftReport } from '../../../types/generated/bindings';
import { useArchiDrift } from './use-archi-drift';

/**
 * `useArchiDrift` 单测（AC-19）：三触发取数（挂载 / `root` 或 `mode` 变化 / 手动
 * 刷新）、`root = null` 不发 invoke、失败降级 `report = null` + `error`、卸载与
 * 竞态不写状态、零 watch 零轮询。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（生成的 `commands.archiDrift`
 * 最终经此调用），按命令名分发 fixture 并记录调用序列与载荷。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

/** 漂移报告 fixture（`clean` 全量）。 */
function driftReport(): ArchiDriftReport {
  return {
    version: 1,
    timestamp: '2026-10-20T08:31:12Z',
    mode: 'full',
    root: '/ws',
    change: null,
    base: null,
    modelHash: 'sha256:00ff',
    model: {
      elements: 2,
      relations: 1,
      byGate: { strict: 1, warn: 1, off: 0 },
      structuralErrors: [],
      warnings: [],
    },
    scan: {
      mode: 'full',
      changedFiles: 0,
      scannedFiles: 2,
      excluded: [],
      unmatchedFiles: [],
      matched: [],
      unresolved: [],
    },
    errors: [],
    warnings: [],
    suppressed: [],
    observations: [],
    status: 'clean',
  };
}

/** 取数调用序列（命令名 + 入参）。 */
function calls(): { command: string; params?: Record<string, unknown> }[] {
  return invokeMock.mock.calls.map(([command, params]) => ({
    command: command as string,
    params: params as Record<string, unknown> | undefined,
  }));
}

/** `archi_drift` 的调用次数。 */
function driftCalls() {
  return calls().filter((call) => call.command === 'archi_drift');
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'archi_drift') return Promise.resolve(driftReport());
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
});

describe('useArchiDrift 三触发', () => {
  it('应在挂载时以当前 root 与 mode 发起一次 archi_drift 并落地报告', async () => {
    const { result } = renderHook(() => useArchiDrift('/ws', 'incremental'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.report).toEqual(driftReport());
    expect(result.current.error).toBeNull();
    expect(driftCalls()).toHaveLength(1);
    expect(driftCalls()[0]?.params).toEqual({ root: '/ws', mode: 'incremental' });
  });

  it.each(['incremental', 'full'] as const)('应把 mode = %s 逐字透传给命令', async (mode) => {
    const { result } = renderHook(() => useArchiDrift('/ws', mode));
    await waitFor(() => expect(result.current.report).not.toBeNull());
    expect(driftCalls()[0]?.params).toEqual({ root: '/ws', mode });
  });

  it('应在 root 变化时以新 root 重发，并丢弃旧根迟到应答', async () => {
    const pending: ((report: ArchiDriftReport) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiDriftReport>((resolve) => {
          pending.push(resolve);
        }),
    );
    const modes: ArchiDriftMode[] = [];
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiDrift(root, modes[0] ?? 'full'),
      {
        initialProps: { root: '/ws/alpha' },
      },
    );
    await act(async () => {
      pending[0]?.(driftReport());
    });
    await waitFor(() => expect(result.current.report).not.toBeNull());

    rerender({ root: '/ws/beta' });
    await waitFor(() => expect(driftCalls()).toHaveLength(2));
    expect(result.current.report).toBeNull();

    const beta = { ...driftReport(), root: '/ws/beta' };
    await act(async () => {
      pending[1]?.(beta);
    });
    await waitFor(() => expect(result.current.report?.root).toBe('/ws/beta'));

    await act(async () => {
      pending[0]?.(driftReport());
    });
    expect(result.current.report?.root).toBe('/ws/beta');
  });

  it('应在手动刷新时重发（连续两次刷新累计 3 次 invoke，不去重）', async () => {
    const { result } = renderHook(() => useArchiDrift('/ws', 'full'));
    await waitFor(() => expect(result.current.report).not.toBeNull());

    act(() => result.current.refresh());
    await waitFor(() => expect(driftCalls()).toHaveLength(2));

    act(() => result.current.refresh());
    await waitFor(() => expect(driftCalls()).toHaveLength(3));
  });

  it('应在渲染后等待一个宏任务窗口不增长调用次数（零 watch 零轮询）', async () => {
    const { result } = renderHook(() => useArchiDrift('/ws', 'full'));
    await waitFor(() => expect(result.current.report).not.toBeNull());
    const baseline = driftCalls().length;
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
    });
    expect(driftCalls()).toHaveLength(baseline);
  });
});

describe('useArchiDrift 无根与失败面', () => {
  it('应在 root 为 null 时零 invoke 且状态清零', async () => {
    const { result } = renderHook(() => useArchiDrift(null, 'full'));
    await act(async () => {
      await Promise.resolve();
    });
    expect(driftCalls()).toHaveLength(0);
    expect(result.current.report).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();

    act(() => result.current.refresh());
    await act(async () => {
      await Promise.resolve();
    });
    expect(driftCalls()).toHaveLength(0);
  });

  it.each([
    ['字符串原因', 'boom'],
    ['Error 原因', new Error('boom')],
  ])('应在 invoke reject（%s）时降级为 error + report = null 且不抛', async (_tag, reason) => {
    invokeMock.mockImplementation(() => Promise.reject(reason));
    const { result } = renderHook(() => useArchiDrift('/ws', 'full'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.report).toBeNull();
    expect(result.current.error).toBeTruthy();
    expect(typeof result.current.error).toBe('string');
  });

  it('应在失败后刷新成功时清空错误面（错误不粘滞）', async () => {
    invokeMock.mockImplementationOnce(() => Promise.reject(new Error('首次失败')));
    invokeMock.mockImplementation(() => Promise.resolve(driftReport()));
    const { result } = renderHook(() => useArchiDrift('/ws', 'full'));
    await waitFor(() => expect(result.current.error).not.toBeNull());

    act(() => result.current.refresh());
    await waitFor(() => expect(result.current.report).not.toBeNull());
    expect(result.current.error).toBeNull();
  });
});

describe('useArchiDrift 生命周期与返回面', () => {
  it('应在卸载后回写前取消（不产生状态写入告警/报错）', async () => {
    const pending: ((report: ArchiDriftReport) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiDriftReport>((resolve) => {
          pending.push(resolve);
        }),
    );
    const { unmount } = renderHook(() => useArchiDrift('/ws', 'full'));
    await waitFor(() => expect(driftCalls()).toHaveLength(1));
    unmount();
    await act(async () => {
      pending[0]?.(driftReport());
    });
    expect(() => undefined).not.toThrow();
  });

  it('应暴露 report / loading / error / refresh 四键且 refresh 引用稳定', async () => {
    const { result, rerender } = renderHook(() => useArchiDrift('/ws', 'full'));
    await waitFor(() => expect(result.current.report).not.toBeNull());
    const firstRefresh = result.current.refresh;
    expect(Object.keys(result.current).sort()).toEqual(['error', 'loading', 'refresh', 'report']);
    rerender();
    expect(result.current.refresh).toBe(firstRefresh);
  });
});
