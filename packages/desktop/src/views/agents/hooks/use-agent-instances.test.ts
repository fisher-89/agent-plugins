import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEngineKind, AgentInstanceRecord } from '../../../types/generated/bindings';
import { useAgentInstances } from './use-agent-instances';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发替身（list_agent_instances /
// save_agent_instance / delete_agent_instance / set_default_agent_instance，
// 可切换 resolve / reject 并记录调用序列）；sonner 以 vi.fn 替身注入
// toast.error（动作轨错误呈现断言，沿 use-agent-providers 同口径）。
// ---------------------------------------------------------------------------

const { invokeMock, toastErrorMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('sonner', () => ({
  toast: { error: toastErrorMock },
}));

/** agent 实例 fixture（serde camelCase 线格式）。 */
function instance(
  id: number,
  name: string,
  engine: AgentEngineKind,
  isDefault = false,
): AgentInstanceRecord {
  return { id, name, engine, providerId: null, isDefault };
}

/** 可变“库存”：list 返回当前库存，save/remove/setDefault 同步修改库存。 */
let stored: AgentInstanceRecord[];

function mockDispatch() {
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    // 入参窄化助手（invoke 参数对象为 unknown 载荷，typeof 守卫收窄）
    const textOf = (key: string): string => {
      const value = params?.[key];
      return typeof value === 'string' ? value : '';
    };
    const numOr = (key: string, fallback: number): number => {
      const value = params?.[key];
      return typeof value === 'number' ? value : fallback;
    };
    if (command === 'list_agent_instances') {
      return Promise.resolve([...stored]);
    }
    if (command === 'save_agent_instance') {
      const record: AgentInstanceRecord = {
        id: numOr('id', stored.length + 1),
        name: textOf('name'),
        engine: textOf('engine') === 'sdk' ? 'sdk' : 'cli',
        providerId: params?.providerId === null ? null : numOr('providerId', 0),
        isDefault: false,
      };
      stored = [...stored.filter((row) => row.id !== record.id), record];
      return Promise.resolve(record);
    }
    if (command === 'delete_agent_instance') {
      const id = numOr('id', -1);
      stored = stored.filter((row) => row.id !== id);
      return Promise.resolve(true);
    }
    if (command === 'set_default_agent_instance') {
      const id = numOr('id', -1);
      stored = stored.map((row) => ({ ...row, isDefault: row.id === id }));
      const target = stored.find((row) => row.id === id);
      return target
        ? Promise.resolve(target)
        : Promise.reject(new Error('db: agent 不存在: id=404'));
    }
    return Promise.resolve(null);
  });
}

async function settleLoaded(): Promise<void> {
  await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('list_agent_instances'));
}

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

describe('useAgentInstances：取数收口（挂载一次 + 动作轨道，无轮询）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('挂载 invoke list_agent_instances 恰一次，instances 态回填；无轮询（调用数稳定）', async () => {
    invokeMock.mockResolvedValue([instance(1, 'cli-a', 'cli'), instance(2, 'sdk-b', 'sdk', true)]);
    const { result } = renderHook(() => useAgentInstances());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(invokeMock.mock.calls.map(([name]) => name)).toEqual(['list_agent_instances']);
    expect(result.current.instances).toHaveLength(2);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();

    // 无轮询：连续轮询调用数不再增长
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
    expect(countOf('list_agent_instances')).toBe(1);
  });

  it('save / remove 动作：对应 invoke 后清单刷新（显式动作刷新）', async () => {
    mockDispatch();
    const { result } = renderHook(() => useAgentInstances());
    await settleLoaded();

    await act(async () => {
      const saved = await result.current.save({
        id: null,
        name: '新实例',
        engine: 'cli',
        providerId: null,
      });
      expect(saved?.name).toBe('新实例');
    });
    expect(invokeMock).toHaveBeenCalledWith('save_agent_instance', {
      id: null,
      name: '新实例',
      engine: 'cli',
      providerId: null,
    });
    await waitFor(() =>
      expect(result.current.instances.map((row) => row.name)).toContain('新实例'),
    );

    await act(async () => {
      const hit = await result.current.remove(1);
      expect(hit).toBe(true);
    });
    expect(invokeMock).toHaveBeenCalledWith('delete_agent_instance', { id: 1 });
    // remove(1) 恰移除刚保存的实例（id=1）：刷新后清单为空
    await waitFor(() => expect(result.current.instances).toEqual([]));
  });

  it('setDefault（AC-4 前端半）：invoke set_default_agent_instance 成功后清单刷新（唯一默认标记呈现）', async () => {
    mockDispatch();
    stored = [instance(1, '甲', 'cli', true), instance(2, '乙', 'cli')];
    const { result } = renderHook(() => useAgentInstances());
    await waitFor(() => expect(result.current.instances).toHaveLength(2));

    await act(async () => {
      const switched = await result.current.setDefault(2);
      expect(switched?.id).toBe(2);
      expect(switched?.isDefault).toBe(true);
    });

    expect(invokeMock).toHaveBeenCalledWith('set_default_agent_instance', { id: 2 });
    await waitFor(() => expect(countOf('list_agent_instances')).toBe(2));
    await waitFor(() => {
      const defaults = result.current.instances.filter((row) => row.isDefault);
      expect(defaults.map((row) => row.id)).toEqual([2]);
    });
  });
});

// ---------------------------------------------------------------------------
// 错误双轨：清单加载失败置 inline error 态（查询轨）；save / remove /
// setDefault 失败 toast 固定前缀且不置 error 态（动作轨）。
// ---------------------------------------------------------------------------

describe('useAgentInstances：错误双轨', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('清单加载 reject → error 态置位（查询轨 inline 呈现，无 toast）', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));
    const { result } = renderHook(() => useAgentInstances());

    await waitFor(() => expect(result.current.error).not.toBeNull());

    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.instances).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(toastErrorMock).not.toHaveBeenCalled();
  });

  it('save reject → toast.error 固定前缀「保存 agent 失败：」且不置 error 态', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_instances') return Promise.resolve([]);
      return Promise.reject(new Error('db: agent 名已存在'));
    });
    const { result } = renderHook(() => useAgentInstances());
    await settleLoaded();

    await act(async () => {
      const saved = await result.current.save({
        id: null,
        name: '重复实例',
        engine: 'cli',
        providerId: null,
      });
      expect(saved).toBeNull();
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(toastErrorMock).toHaveBeenCalledWith('保存 agent 失败：Error: db: agent 名已存在');
    expect(result.current.error).toBeNull();
  });

  it('remove reject → toast.error 固定前缀「删除 agent 失败：」且不置 error 态', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_instances') return Promise.resolve([instance(1, '甲', 'cli')]);
      return Promise.reject(new Error('db: 打开失败'));
    });
    const { result } = renderHook(() => useAgentInstances());
    await waitFor(() => expect(result.current.instances).toHaveLength(1));

    await act(async () => {
      const hit = await result.current.remove(1);
      expect(hit).toBe(false);
    });

    expect(toastErrorMock).toHaveBeenCalledWith('删除 agent 失败：Error: db: 打开失败');
    expect(result.current.error).toBeNull();
  });

  it('setDefault reject → toast.error 固定前缀「设置默认 agent 失败：」且不置 error 态', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_instances') return Promise.resolve([instance(1, '甲', 'cli')]);
      return Promise.reject(new Error('db: agent 不存在: id=404'));
    });
    const { result } = renderHook(() => useAgentInstances());
    await waitFor(() => expect(result.current.instances).toHaveLength(1));

    await act(async () => {
      const switched = await result.current.setDefault(404);
      expect(switched).toBeNull();
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(toastErrorMock).toHaveBeenCalledWith(
      '设置默认 agent 失败：Error: db: agent 不存在: id=404',
    );
    expect(result.current.error).toBeNull();
  });
});
