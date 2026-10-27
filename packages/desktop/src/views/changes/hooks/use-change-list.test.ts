import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, driftReports, resetDrift } from '../../../lib/ipc-drift';
import type { ChangeList } from '../../../types/dto';
import { useChangeList } from './use-change-list';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

/** 构造一份最小 ChangeList DTO（条目携 id——identity 模型下 id 为键、name 为
 * 展示属性，fixture 两者取值相异以证 DTO 逐字段透传）。 */
function fakeList(activeName: string): ChangeList {
  return {
    active: [
      {
        id: `id-${activeName}`,
        name: activeName,
        title: activeName,
        source: 'active',
        status: 'active',
        activePhase: null,
        created: null,
      },
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
    expect(result.current.data?.active[0].id).toBe('id-add-feature');
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

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-5 回归锁定）：list_changes 裸 invoke → typed bindings
// 机械替换后，invoke 命令名与参数逐字不变（生成绑定底层仍走同模块 invoke，
// mock 机制切换后依旧生效）。
// ---------------------------------------------------------------------------

describe('useChangeList：生成绑定调用面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('经生成绑定入口后 invoke 收到 "list_changes" 与 { root }，ChangeList DTO 透传 data 更新', async () => {
    invokeMock.mockResolvedValue(fakeList('typed-binding'));
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: '/repo' });
    expect(result.current.data?.active[0].id).toBe('id-typed-binding');
    expect(result.current.data?.active[0].name).toBe('typed-binding');
    expect(result.current.error).toBeNull();
  });

  it('root 为 null 时不发起任何调用（零 IPC 语义在绑定切换后保持）', () => {
    renderHook(() => useChangeList(null));

    expect(invokeMock.mock.calls).toHaveLength(0);
  });

  it('invoke reject → error 置位、data 保持原值、不抛未捕获异常（错误路径行为不变）', async () => {
    invokeMock.mockResolvedValueOnce(fakeList('kept'));
    const { result } = renderHook(() => useChangeList('/repo'));
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
});

// ---------------------------------------------------------------------------
// 经 `ipc` 的运行期校验面（change ipc-zod-runtime-schema）：`listChanges` 的
// 参数面 strict / 结果面 soft-report 与既有分组渲染的组合回归。
// ---------------------------------------------------------------------------

describe('useChangeList：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    resetDrift();
    // DEV 汇总告警静音（漂移记录本身仍是判据）
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('listChanges(root) 放行：结果干净零漂移、既有分组渲染不变', async () => {
    invokeMock.mockResolvedValue(fakeList('add-feature'));
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: '/repo' });
    expect(result.current.data?.active[0].name).toBe('add-feature');
    expect(driftCount()).toBe(0);
  });

  it('空清单（active + archiveGroups 皆空）→ 零漂移、既有空态不变', async () => {
    invokeMock.mockResolvedValue({ active: [], archiveGroups: [] });
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(result.current.data?.active).toEqual([]);
    expect(result.current.data?.archiveGroups).toEqual([]);
    expect(driftCount()).toBe(0);
  });

  it('reject 串 → 既有 error 态文案逐字不变、零漂移', async () => {
    invokeMock.mockRejectedValue('开库失败：/repo/ws.db');
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.error).not.toBeNull());

    expect(result.current.error).toBe('开库失败：/repo/ws.db');
    expect(driftCount()).toBe(0);
  });

  it('结果面脏（created 非 string）→ 漂移恰 1 条、分组逻辑不崩（原值透传）', async () => {
    const dirty = {
      active: [
        {
          id: 'id-1',
          name: 'add-feature',
          title: 'add-feature',
          source: 'active',
          status: 'active',
          activePhase: null,
          created: 42,
        },
      ],
      archiveGroups: [],
    };
    invokeMock.mockResolvedValue(dirty);
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('listChanges');
    expect(driftReports()[0].path).toBe('#/active/0/created');
    // 软失败不阻断 UI：既有渲染按原值继续
    expect(result.current.data).toBe(dirty);
    expect(result.current.error).toBeNull();
  });

  it('结果面多余键（strictObject）→ 漂移 1 条、渲染不变', async () => {
    invokeMock.mockResolvedValue({ ...fakeList('kept'), surplusKey: 1 });
    const { result } = renderHook(() => useChangeList('/repo'));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].path).toBe('#');
    expect(result.current.data?.active[0].name).toBe('kept');
  });

  it('root 连续切换两次取数 → 各自零误拒、漂移保持 0', async () => {
    invokeMock.mockResolvedValue(fakeList('first'));
    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useChangeList(root),
      { initialProps: { root: '/old' as string | null } },
    );
    await waitFor(() => expect(result.current.data?.active[0].name).toBe('first'));

    invokeMock.mockResolvedValue(fakeList('second'));
    rerender({ root: '/new' });
    await waitFor(() => expect(result.current.data?.active[0].name).toBe('second'));

    expect(invokeMock).toHaveBeenLastCalledWith('list_changes', { root: '/new' });
    expect(driftCount()).toBe(0);
  });
});
