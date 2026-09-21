import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeList } from '../types/dto';
import { useChangeList } from './useChangeList';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

/** 构造一份最小 ChangeList DTO。 */
function fakeList(activeName: string): ChangeList {
  return {
    active: [
      { name: activeName, source: 'active', inventory: 'v2', created: null, unparsable: false },
    ],
    archiveGroups: [],
  };
}

describe('useChangeList：显式刷新取数纪律（AC-12）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('root 为 null 时不发起 invoke，data 为 null', () => {
    const { result } = renderHook(() => useChangeList(null));
    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.data).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('选定 root / refresh 触发 invoke("list_changes")，成功后 data 更新', async () => {
    invokeMock.mockResolvedValue(fakeList('add-feature'));
    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useChangeList(root),
      {
        initialProps: { root: null as string | null },
      },
    );

    rerender({ root: '/repo' });
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: '/repo' });
    expect(result.current.data?.active[0].name).toBe('add-feature');
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();

    // 显式 refresh 再次触发同一命令
    const callsAfterMount = invokeMock.mock.calls.length;
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBeGreaterThan(callsAfterMount));
    expect(invokeMock).toHaveBeenLastCalledWith('list_changes', { root: '/repo' });
  });

  it('root 变更后再刷新，invoke 参数携带新 root', async () => {
    invokeMock.mockResolvedValue(fakeList('first'));
    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useChangeList(root),
      {
        initialProps: { root: '/old' },
      },
    );
    await waitFor(() => expect(result.current.data).not.toBeNull());

    invokeMock.mockResolvedValue(fakeList('second'));
    rerender({ root: '/new' });
    await waitFor(() => expect(result.current.data?.active[0].name).toBe('second'));
    expect(invokeMock).toHaveBeenLastCalledWith('list_changes', { root: '/new' });
  });

  it('invoke reject 时 error 置位、data 保持原值、不抛未捕获异常', async () => {
    invokeMock.mockResolvedValueOnce(fakeList('kept'));
    const { result } = renderHook(({ root }: { root: string | null }) => useChangeList(root), {
      initialProps: { root: '/repo' },
    });
    await waitFor(() => expect(result.current.data).not.toBeNull());

    invokeMock.mockRejectedValueOnce(new Error('IPC 断开'));
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.data?.active[0].name).toBe('kept');
    expect(result.current.loading).toBe(false);
  });

  it('连续两次 refresh 后最终状态以后一次结果为准', async () => {
    invokeMock
      .mockResolvedValueOnce(fakeList('first'))
      .mockResolvedValueOnce(fakeList('second'))
      .mockResolvedValueOnce(fakeList('third'));
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data?.active[0].name).toBe('first'));

    // 同一渲染批次内的两次 refresh 会被 React 合并为一次取数；
    // 分批触发两次刷新，最终状态以后一次刷新的结果为准
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.data?.active[0].name).toBe('second'));
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.data?.active[0].name).toBe('third'));
    expect(result.current.error).toBeNull();
  });

  it('除显式刷新外无任何定时器/轮询调用：推进虚拟计时后 invoke 次数不增长', async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue(fakeList('stable'));
    const { result } = renderHook(() => useChangeList('/repo'));
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledTimes(1);

    act(() => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(result.current.data?.active[0].name).toBe('stable');
  });
});
