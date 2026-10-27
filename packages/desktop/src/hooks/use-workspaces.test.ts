import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, reportDrift, resetDrift } from '../lib/ipc-drift';
import type { WorkspaceRecord } from '../types/dto';
import { useWorkspaces } from './use-workspaces';

const { invokeMock, toastErrorMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

// 错误双轨（design D2/D8）：hook 层不渲染 toast DOM，仅断言 toast.error 调用参数
// （D8 固定前缀 + String(err)）
vi.mock('sonner', () => ({
  toast: { error: toastErrorMock },
}));

/** 构造一份最小 WorkspaceRecord（三字段；清单默认序与时间戳无关）。 */
function record(root: string): WorkspaceRecord {
  const name = root.split(/[\\/]/).pop() ?? root;
  return { root, name, addedAt: 1 };
}

/** 可变"库存"：list_workspaces 返回当前库存，add/remove 同步修改库存。 */
let stored: WorkspaceRecord[];

/** 按命令名分发固定 fixture（沿用 use-change-list.test.ts 的替身模式）。 */
function mockDispatch() {
  invokeMock.mockImplementation((command: string, params?: { root?: string }) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([...stored]);
    }
    if (command === 'add_workspace') {
      const rec = record(params?.root ?? '');
      stored = [...stored.filter((r) => r.root !== rec.root), rec];
      return Promise.resolve(rec);
    }
    if (command === 'remove_workspace') {
      stored = stored.filter((r) => r.root !== params?.root);
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

describe('useWorkspaces：清单取数收口（挂载自动 load + add/remove 内部刷新 + select 本地切换）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // 置于套件首位：守卫失效的病态循环会在其余用例（立即 resolve 的 mock）中
  // 微任务风暴式失控，须让本用例先运行、以稳定判据快速判失败
  it('取数链稳定：挂载自动取数恰一次后不再增长（无轮询、无恢复循环）', async () => {
    stored = [record('/repo/a'), record('/repo/b')];
    // 每个响应经 setTimeout(0) 宏任务逐拍解析：若取数链失控形成循环，
    // 循环每拍只走一轮而非微任务风暴，可被下方稳定判据观测到并快速判失败
    invokeMock.mockImplementation((command: string) => {
      const respond = (value: unknown) =>
        new Promise((resolve) => setTimeout(() => resolve(value), 0));
      if (command === 'list_workspaces') return respond([...stored]);
      return respond(true);
    });
    const { result } = renderHook(() => useWorkspaces());

    // 等待取数链稳定：连续两次轮询调用数不再增长
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
    expect(countOf('list_workspaces')).toBe(1);
    expect(invokeMock).toHaveBeenCalledTimes(1); // 除挂载取数外无任何命令
    expect(result.current.workspaces.map((r) => r.root)).toEqual(['/repo/a', '/repo/b']);
  });

  it('挂载即自动取数并恢复默认序第一名：root 置为第一名，无任何动作命令', async () => {
    invokeMock.mockResolvedValue([record('/repo/a')]);
    const { result } = renderHook(() => useWorkspaces());

    await waitFor(() => expect(result.current.root).toBe('/repo/a'));
    expect(invokeMock.mock.calls.map(([name]) => name)).toEqual(['list_workspaces']);
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

  it('add 成功：返回记录、root 直接切到返回记录 root、内部刷新清单', async () => {
    mockDispatch();
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();
    // 挂载时清单为空：仅取数一次
    expect(invokeMock).toHaveBeenCalledTimes(1);

    let added: WorkspaceRecord | null = null;
    await act(async () => {
      added = await result.current.add('/repo/b');
    });

    expect(added).not.toBeNull();
    expect(added!.root).toBe('/repo/b');
    expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '/repo/b' });
    // 挂载取数 1 次 + add 1 次 + 内部刷新 1 次 = 3 次
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(3));
    expect(invokeMock).toHaveBeenLastCalledWith('list_workspaces');
    await waitFor(() => expect(result.current.workspaces.map((r) => r.root)).toContain('/repo/b'));
    expect(result.current.root).toBe('/repo/b');
  });

  it('add 成功后 root 为新记录即使其不在默认序首位', async () => {
    mockDispatch();
    stored = [record('/repo/a')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.root).toBe('/repo/a'));

    // mock 库存按加入顺序返回：z 追加在尾部（默认序非第一名）
    await act(async () => {
      await result.current.add('/repo/z');
    });

    expect(result.current.root).toBe('/repo/z');
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

  it('remove 当前根：刷新后 root 顺延剩余第一名；清空后回 null（欢迎屏）', async () => {
    mockDispatch();
    stored = [record('/repo/a'), record('/repo/b')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.root).toBe('/repo/a'));

    await act(async () => {
      await result.current.remove('/repo/a');
    });

    await waitFor(() => expect(result.current.root).toBe('/repo/b'));
    expect(result.current.workspaces.map((r) => r.root)).toEqual(['/repo/b']);

    await act(async () => {
      await result.current.remove('/repo/b');
    });
    await waitFor(() => expect(result.current.root).toBeNull());
  });

  it('remove 非当前根：当前根保持不变', async () => {
    mockDispatch();
    stored = [record('/repo/a'), record('/repo/b')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.root).toBe('/repo/a'));

    await act(async () => {
      await result.current.remove('/repo/b');
    });

    expect(result.current.root).toBe('/repo/a');
    await waitFor(() => expect(result.current.workspaces.map((r) => r.root)).toEqual(['/repo/a']));
  });

  it('select：本地切换当前根，不触发任何 invoke，清单顺序不变', async () => {
    invokeMock.mockResolvedValue([record('/repo/a'), record('/repo/b')]);
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.root).toBe('/repo/a'));
    const callsBefore = invokeMock.mock.calls.length;

    act(() => {
      result.current.select('/repo/b');
    });

    expect(result.current.root).toBe('/repo/b');
    expect(invokeMock.mock.calls.length).toBe(callsBefore); // 无后端调用
    // 清单不重排：仍为默认序
    expect(result.current.workspaces.map((r) => r.root)).toEqual(['/repo/a', '/repo/b']);
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

  it('卸载取消：清单在卸载后才返回时不置状态、不再取数', async () => {
    let resolveLoad: (records: WorkspaceRecord[]) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') {
        return new Promise<WorkspaceRecord[]>((resolve) => {
          resolveLoad = resolve;
        });
      }
      return Promise.resolve(null);
    });
    const { unmount } = renderHook(() => useWorkspaces());
    unmount();

    await act(async () => {
      resolveLoad([record('/repo/a')]);
    });

    // 取消后迟到响应被丢弃：不内部刷新取数
    expect(countOf('list_workspaces')).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// 错误双轨（design D2/D8）：动作失败（add/remove）hook 内直调 toast.error
// 固定文案，error 仅承载 list_workspaces 加载失败——动作失败用例由旧「error 置位」
// 断言改写为 toast 调用断言（error 保持 null）。
// ---------------------------------------------------------------------------

describe('useWorkspaces：错误双轨（动作 toast 化）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('add reject：返回 null、toast.error 以「添加 workspace 失败：」前缀 + 错误串调用、error 保持 null', async () => {
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
    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(toastErrorMock).toHaveBeenCalledWith(
      '添加 workspace 失败：Error: canonicalize: 无效路径',
    );
    expect(result.current.error).toBeNull();
  });

  it('add 传入空字符串 root：参数原样 invoke、toast 以固定前缀呈现、不崩溃', async () => {
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
    expect(toastErrorMock).toHaveBeenCalledWith(
      '添加 workspace 失败：Error: canonicalize: : 系统找不到指定的路径',
    );
    expect(result.current.error).toBeNull();
  });

  it('remove reject：返回 false、toast.error 以「移除 workspace 失败：」前缀调用、error 保持 null', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') return Promise.resolve([record('/repo/a')]);
      // 仅 remove_workspace 走 reject
      return Promise.reject(new Error('db: 打开失败'));
    });
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.workspaces).toHaveLength(1));

    let hit: boolean | undefined;
    await act(async () => {
      hit = await result.current.remove('/repo/a');
    });

    expect(hit).toBe(false);
    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(toastErrorMock).toHaveBeenCalledWith('移除 workspace 失败：Error: db: 打开失败');
    expect(result.current.error).toBeNull();
  });

  it('remove resolve(false)（store miss 幂等）：返回 false、不调 toast、不置 error', async () => {
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
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(result.current.error).toBeNull();
    // 幂等 miss 非错误，但内部刷新仍发生（挂载取数 + 移除后刷新 = 2 次）
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([name]) => name === 'list_workspaces')).toHaveLength(2),
    );
  });

  it('add / remove 全部成功：toast 零调用（动作成功面无惊扰）', async () => {
    mockDispatch();
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();

    await act(async () => {
      await result.current.add('/repo/b');
    });
    await act(async () => {
      await result.current.remove('/repo/b');
    });

    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(result.current.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-5 回归锁定）：list_workspaces / add_workspace /
// remove_workspace 裸 invoke → typed bindings 机械替换后，invoke 命令名与
// 参数逐字不变（生成绑定底层仍走同模块 invoke，mock 机制切换后依旧生效）。
// ---------------------------------------------------------------------------

describe('useWorkspaces：生成绑定调用面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('三命令经生成绑定入口后 invoke 命令名与参数（root 等）逐字不变，返回值透传（清单内部刷新 / root 切换 / remove 返回 true）', async () => {
    mockDispatch();
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();

    await act(async () => {
      const added = await result.current.add('/repo/b');
      expect(added?.root).toBe('/repo/b');
    });
    await waitFor(() => expect(result.current.root).toBe('/repo/b'));
    expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '/repo/b' });

    await act(async () => {
      const hit = await result.current.remove('/repo/b');
      expect(hit).toBe(true);
    });
    expect(invokeMock).toHaveBeenCalledWith('remove_workspace', { root: '/repo/b' });
    // 动作后以 list_workspaces 内部刷新（挂载 1 + add 刷新 1 + remove 刷新 1）
    expect(countOf('list_workspaces')).toBe(3);
  });

  it('add 传入空字符串 root 参数原样穿透（把关责任仍在后端，绑定切换不引入前端改写）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') return Promise.resolve([]);
      return Promise.reject(new Error('canonicalize: 无效路径'));
    });
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();

    await act(async () => {
      await result.current.add('');
    });

    expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '' });
  });

  it('add / remove reject → toast 固定前缀 + 错误串、返回 null / false 的错误双轨行为不变', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') return Promise.resolve([record('/repo/a')]);
      return Promise.reject(new Error('db: 打开失败'));
    });
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.workspaces).toHaveLength(1));

    await act(async () => {
      const added = await result.current.add('/nope');
      expect(added).toBeNull();
    });
    await act(async () => {
      const hit = await result.current.remove('/repo/a');
      expect(hit).toBe(false);
    });

    expect(toastErrorMock).toHaveBeenCalledWith('添加 workspace 失败：Error: db: 打开失败');
    expect(toastErrorMock).toHaveBeenCalledWith('移除 workspace 失败：Error: db: 打开失败');
    expect(result.current.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 漂移聚合生命周期（AC-13 / spec「清空入口有产品消费方」）：`applyRoot` 判变时
// `resetDrift()`——当前根**变更**才清空，同一根的重取数 MUST NOT 清空。
// 断言口径用**真实 collector**（`reportDrift` 造数 → `driftCount()` 读数），
// 不以替身调用次数测「清空」这件事。
// ---------------------------------------------------------------------------

/** 造一条漂移记录并返回当前去重键数（真实 collector，无替身）。 */
function seedDrift(cmd = 'codeStats', path = '#/files'): number {
  reportDrift({ cmd, path, expected: 'number', got: 'dirty' });
  return driftCount();
}

describe('useWorkspaces：当前根变更才清空漂移聚合', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
    resetDrift();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('select 切到不同根 → 清空生效（切换前 1 条，切换后归零）', async () => {
    mockDispatch();
    stored = [record('/repo/a'), record('/repo/b')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.root).toBe('/repo/a'));

    expect(seedDrift()).toBe(1);
    act(() => {
      result.current.select('/repo/b');
    });

    expect(result.current.root).toBe('/repo/b');
    expect(driftCount()).toBe(0);
  });

  it('add 成功且返回记录的 canonical root ≠ 当前根（新记录未必居首）→ 清空生效', async () => {
    mockDispatch();
    stored = [record('/repo/a')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.root).toBe('/repo/a'));

    expect(seedDrift()).toBe(1);
    await act(async () => {
      await result.current.add('/repo/z');
    });
    await waitFor(() => expect(result.current.root).toBe('/repo/z'));

    expect(driftCount()).toBe(0);
  });

  it('remove 当前根后取数回落至另一根 → 根变更 → 清空生效', async () => {
    mockDispatch();
    stored = [record('/repo/a'), record('/repo/b')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.root).toBe('/repo/a'));

    expect(seedDrift()).toBe(1);
    await act(async () => {
      await result.current.remove('/repo/a');
    });
    await waitFor(() => expect(result.current.root).toBe('/repo/b'));

    expect(driftCount()).toBe(0);
  });

  it('边界：同一根重取数（清单仍返回当前根）→ 不清空（保持 1 条）', async () => {
    mockDispatch();
    stored = [record('/repo/a'), record('/repo/b')];
    const { result } = renderHook(() => useWorkspaces());
    await waitFor(() => expect(result.current.root).toBe('/repo/a'));

    // add 已存在的当前根 + 随后的内部刷新：两次 applyRoot 都落在同一根
    expect(seedDrift()).toBe(1);
    await act(async () => {
      await result.current.add('/repo/a');
    });
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([c]) => c === 'list_workspaces')).toHaveLength(2),
    );

    // remove 非当前根同样只触发重取、不改根
    await act(async () => {
      await result.current.remove('/repo/b');
    });
    await waitFor(() => expect(result.current.workspaces.map((r) => r.root)).toEqual(['/repo/a']));

    expect(result.current.root).toBe('/repo/a');
    expect(driftCount()).toBe(1);
  });

  it('边界：挂载首取三种回落形态的判变结果（null→X / X→null 均清空，null→null 零清空）', async () => {
    // null → X：原根 null、清单有记录 → 判变 → 清空
    stored = [record('/repo/a')];
    mockDispatch();
    expect(seedDrift('listChanges', '#/0/createdAt')).toBe(1);
    const first = renderHook(() => useWorkspaces());
    await waitFor(() => expect(first.result.current.root).toBe('/repo/a'));
    expect(driftCount()).toBe(0);
    first.unmount();

    // X → null：原根非 null、清单空回落 null → 判变 → 清空
    mockDispatch();
    stored = [];
    const second = renderHook(() => useWorkspaces());
    await settleLoaded();
    act(() => {
      second.result.current.select('/repo/x');
    });
    expect(seedDrift()).toBe(1);
    await act(async () => {
      await second.result.current.remove('/repo/x');
    });
    await waitFor(() => expect(second.result.current.root).toBeNull());
    expect(driftCount()).toBe(0);
    second.unmount();

    // null → null：清单空、原根 null → 零判变 → 不清空
    invokeMock.mockResolvedValue([]);
    expect(seedDrift()).toBe(1);
    const third = renderHook(() => useWorkspaces());
    await settleLoaded();
    await waitFor(() => expect(third.result.current.loading).toBe(false));
    expect(third.result.current.root).toBeNull();
    expect(driftCount()).toBe(1);
  });

  it('异常：list_workspaces reject → 根不变 → 零清空、error 态照旧、零新增漂移', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));
    expect(seedDrift()).toBe(1);
    const { result } = renderHook(() => useWorkspaces());

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.root).toBeNull();
    // 取数失败不写根（未调 applyRoot）→ 既有漂移不被清空、也不新增
    expect(driftCount()).toBe(1);
  });

  it('边界：清单空 [] → 结果面放行、root 回落 null、零漂移', async () => {
    invokeMock.mockResolvedValue([]);
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.root).toBeNull();
    expect(result.current.workspaces).toEqual([]);
    expect(driftCount()).toBe(0);
  });
});

describe('useWorkspaces：经 ipc 的参数面（参数错本地拒、零往返）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
    resetDrift();
  });

  for (const command of [
    { name: 'add', handler: 'add_workspace' },
    { name: 'remove', handler: 'remove_workspace' },
  ] as const) {
    it(`正向：${command.name}Workspace(root) 字符串放行，invoke 命令名与 payload 键逐字不变`, async () => {
      mockDispatch();
      const { result } = renderHook(() => useWorkspaces());
      await settleLoaded();

      await act(async () => {
        await result.current[command.name]('/repo/b');
      });

      const call = invokeMock.mock.calls.find(([name]) => name === command.handler);
      expect(call).toBeDefined();
      expect(call?.[1]).toEqual({ root: '/repo/b' });
      expect(driftCount()).toBe(0);
    });
  }

  it('异常：addWorkspace 传非字符串（42）→ 本地 IpcArgsError、invoke 零调用、既有 String(err) 呈现、零漂移', async () => {
    mockDispatch();
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();
    invokeMock.mockClear();

    let added: WorkspaceRecord | null = null;
    await act(async () => {
      added = await result.current.add(42 as unknown as string);
    });

    expect(added).toBeNull();
    // 参数面失败零 IPC 往返（不含任何 add_workspace 调用）
    expect(invokeMock.mock.calls.filter(([name]) => name === 'add_workspace')).toHaveLength(0);
    // 错误面仍走既有 toast 固定前缀 + String(err)（IpcArgsError extends Error）
    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    const message = toastErrorMock.mock.calls[0]?.[0] as string;
    expect(message.startsWith('添加 workspace 失败：')).toBe(true);
    expect(message).toContain('addWorkspace');
    expect(message).toContain('参数校验失败');
    expect(result.current.error).toBeNull();
    expect(driftCount()).toBe(0);
  });

  it('异常：removeWorkspace 传非字符串 → 本地 IpcArgsError、invoke 零调用、返回 false', async () => {
    mockDispatch();
    const { result } = renderHook(() => useWorkspaces());
    await settleLoaded();
    invokeMock.mockClear();

    let hit: boolean | undefined;
    await act(async () => {
      hit = await result.current.remove(null as unknown as string);
    });

    expect(hit).toBe(false);
    expect(invokeMock.mock.calls.filter(([name]) => name === 'remove_workspace')).toHaveLength(0);
    const message = toastErrorMock.mock.calls[0]?.[0] as string;
    expect(message.startsWith('移除 workspace 失败：')).toBe(true);
    expect(driftCount()).toBe(0);
  });
});
