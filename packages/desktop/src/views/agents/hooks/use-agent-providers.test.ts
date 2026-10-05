import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentModelTiers, AgentProviderRecord } from '../../../types/generated/bindings';
import { useAgentProviders } from './use-agent-providers';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发替身（list_agent_providers / save_agent_provider
// / delete_agent_provider，可切换 resolve / reject 并记录调用序列）；sonner 以
// vi.fn 替身注入 toast.error（动作轨错误呈现断言，沿 use-workspaces 替身模式）。
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

/** provider 记录 fixture（serde camelCase 线格式，三档 models + 可空 contextLength）。 */
function provider(
  id: number,
  name: string,
  apiKey = 'sk-live-1234567890',
  contextLength: number | null = null,
): AgentProviderRecord {
  return {
    id,
    name,
    baseUrl: 'https://api.example.com/v1',
    apiKey,
    models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
    contextLength,
  };
}

const TIERS: AgentModelTiers = { high: 'm-high', medium: 'm-medium', low: 'm-low' };

/** 可变“库存”：list 返回当前库存，save/remove 同步修改库存。 */
let stored: AgentProviderRecord[];

/** 按命令名分发固定 fixture。 */
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
    if (command === 'list_agent_providers') {
      return Promise.resolve([...stored]);
    }
    if (command === 'save_agent_provider') {
      const rawContextLength = params?.['contextLength'];
      const record: AgentProviderRecord = {
        id: numOr('id', stored.length + 1),
        name: textOf('name'),
        baseUrl: textOf('baseUrl'),
        apiKey: textOf('apiKey'),
        models: TIERS,
        contextLength: typeof rawContextLength === 'number' ? rawContextLength : null,
      };
      stored = [...stored.filter((row) => row.id !== record.id), record];
      return Promise.resolve(record);
    }
    if (command === 'delete_agent_provider') {
      const id = numOr('id', -1);
      stored = stored.filter((row) => row.id !== id);
      return Promise.resolve(true);
    }
    return Promise.resolve(null);
  });
}

async function settleLoaded(): Promise<void> {
  await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('list_agent_providers'));
}

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

describe('useAgentProviders：取数收口（挂载一次 + 动作轨道刷新，无轮询）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('挂载 invoke list_agent_providers 恰一次，providers 态回填、loading 收敛', async () => {
    invokeMock.mockResolvedValue([provider(1, '端点甲'), provider(2, '端点乙')]);
    const { result } = renderHook(() => useAgentProviders());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(invokeMock.mock.calls.map(([name]) => name)).toEqual(['list_agent_providers']);
    expect(result.current.providers).toEqual([provider(1, '端点甲'), provider(2, '端点乙')]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('无轮询：连续轮询调用数不再增长（显式动作刷新，无周期取数）', async () => {
    invokeMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          setTimeout(() => resolve([]), 0);
        }),
    );
    const { result } = renderHook(() => useAgentProviders());

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

    expect(result.current.providers).toEqual([]);
    expect(countOf('list_agent_providers')).toBe(1);
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it('save 动作：invoke save_agent_provider 后清单刷新（挂载 1 + save 刷新 1）', async () => {
    mockDispatch();
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      const saved = await result.current.save({
        id: null,
        name: '新端点',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: TIERS,
        contextLength: null,
      });
      expect(saved?.name).toBe('新端点');
    });

    expect(invokeMock).toHaveBeenCalledWith('save_agent_provider', {
      id: null,
      name: '新端点',
      baseUrl: 'https://api.example.com/v1',
      apiKey: 'sk-live-1234567890',
      models: TIERS,
      contextLength: null,
    });
    await waitFor(() => expect(countOf('list_agent_providers')).toBe(2));
    await waitFor(() =>
      expect(result.current.providers.map((row) => row.name)).toContain('新端点'),
    );
  });

  it('save 透传 contextLength 数字：invoke 入参携 contextLength: 200000', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(provider(1, '窗长端点', 'sk-live-1234567890', 200000));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      const saved = await result.current.save({
        id: null,
        name: '窗长端点',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: TIERS,
        contextLength: 200000,
      });
      expect(saved?.contextLength).toBe(200000);
    });

    expect(invokeMock).toHaveBeenCalledWith('save_agent_provider', {
      id: null,
      name: '窗长端点',
      baseUrl: 'https://api.example.com/v1',
      apiKey: 'sk-live-1234567890',
      models: TIERS,
      contextLength: 200000,
    });
  });

  it('save 透传 contextLength null：invoke 入参 contextLength: null（不落 0）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(provider(1, '缺列端点'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      const saved = await result.current.save({
        id: null,
        name: '缺列端点',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: TIERS,
        contextLength: null,
      });
      expect(saved?.contextLength).toBeNull();
    });

    const saveCall = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider');
    expect(saveCall).toBeDefined();
    expect((saveCall?.[1] as Record<string, unknown>)['contextLength']).toBeNull();
  });

  it('库存往返保真：list 回读 contextLength 字段原样（provider fixture 库存形态）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') {
        return Promise.resolve([provider(1, '窗长端点', 'sk-live-1234567890', 131072)]);
      }
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useAgentProviders());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.providers).toHaveLength(1);
    expect(result.current.providers[0].contextLength).toBe(131072);
  });

  it('remove 动作：invoke delete_agent_provider 后清单刷新', async () => {
    mockDispatch();
    stored = [provider(1, '端点甲'), provider(2, '端点乙')];
    const { result } = renderHook(() => useAgentProviders());
    await waitFor(() => expect(result.current.providers).toHaveLength(2));

    await act(async () => {
      const hit = await result.current.remove(1);
      expect(hit).toBe(true);
    });

    expect(invokeMock).toHaveBeenCalledWith('delete_agent_provider', { id: 1 });
    await waitFor(() => expect(countOf('list_agent_providers')).toBe(2));
    await waitFor(() => expect(result.current.providers.map((row) => row.id)).toEqual([2]));
  });
});

// ---------------------------------------------------------------------------
// 错误双轨（沿 use-workspaces 口径）：清单加载失败置 inline error 态（查询轨）；
// save / remove 失败 toast 固定前缀且不置 error 态（动作轨）。
// ---------------------------------------------------------------------------

describe('useAgentProviders：错误双轨', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('清单加载 reject → error 态置位（查询轨 inline 呈现，无 toast）', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));
    const { result } = renderHook(() => useAgentProviders());

    await waitFor(() => expect(result.current.error).not.toBeNull());

    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.providers).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(toastErrorMock).not.toHaveBeenCalled();
  });

  it('save reject → toast.error 固定前缀「保存 provider 失败：」且不置 error 态', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.reject(new Error('db: provider 名已存在'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      const saved = await result.current.save({
        id: null,
        name: '重复端点',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: TIERS,
        contextLength: null,
      });
      expect(saved).toBeNull();
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(toastErrorMock).toHaveBeenCalledWith('保存 provider 失败：Error: db: provider 名已存在');
    expect(result.current.error).toBeNull();
  });

  it('remove reject → toast.error 固定前缀「删除 provider 失败：」且不置 error 态', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([provider(1, '被引用端点')]);
      return Promise.reject(new Error('db: provider 被 agent 引用，禁止删除'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await waitFor(() => expect(result.current.providers).toHaveLength(1));

    await act(async () => {
      const hit = await result.current.remove(1);
      expect(hit).toBe(false);
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(toastErrorMock).toHaveBeenCalledWith(
      '删除 provider 失败：Error: db: provider 被 agent 引用，禁止删除',
    );
    expect(result.current.error).toBeNull();
  });

  it('remove resolve(false)（store miss 幂等）：非错误不 toast、不置 error、仍内部刷新', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([provider(1, '端点甲')]);
      if (command === 'delete_agent_provider') return Promise.resolve(false);
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useAgentProviders());
    await waitFor(() => expect(result.current.providers).toHaveLength(1));

    await act(async () => {
      const hit = await result.current.remove(1);
      expect(hit).toBe(false);
    });

    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(result.current.error).toBeNull();
    await waitFor(() => expect(countOf('list_agent_providers')).toBe(2));
  });

  it('save / remove 全部成功：toast 零调用（动作成功面无惊扰）', async () => {
    mockDispatch();
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save({
        id: null,
        name: '顺滑端点',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: TIERS,
        contextLength: null,
      });
    });
    await act(async () => {
      await result.current.remove(1);
    });

    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(result.current.error).toBeNull();
  });
});
