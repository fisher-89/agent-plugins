import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, resetDrift } from '../../../lib/ipc-drift';
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

/** provider 记录 fixture（serde camelCase 线格式，协议 + 三档 models + 可空
 * contextLength / maxTokens）。 */
function provider(
  id: number,
  name: string,
  apiKey = 'sk-live-1234567890',
  contextLength: number | null = null,
): AgentProviderRecord {
  return {
    id,
    name,
    protocol: 'openai',
    baseUrl: 'https://api.example.com/v1',
    apiKey,
    models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
    contextLength,
    maxTokens: null,
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
        protocol: 'openai',
        baseUrl: textOf('baseUrl'),
        apiKey: textOf('apiKey'),
        models: TIERS,
        contextLength: typeof rawContextLength === 'number' ? rawContextLength : null,
        maxTokens: null,
      };
      stored = [...stored.filter((row) => row.id !== record.id), record];
      return Promise.resolve(record);
    }
    if (command === 'notification_record') {
      return Promise.resolve(null);
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
        protocol: 'openai',
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
      expect(saved?.name).toBe('新端点');
    });

    expect(invokeMock).toHaveBeenCalledWith('save_agent_provider', {
      id: null,
      name: '新端点',
      baseUrl: 'https://api.example.com/v1',
      apiKey: 'sk-live-1234567890',
      models: TIERS,
      protocol: 'openai',
      tuning: {
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      },
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
        protocol: 'openai',
        contextLength: 200000,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
      expect(saved?.contextLength).toBe(200000);
    });

    expect(invokeMock).toHaveBeenCalledWith('save_agent_provider', {
      id: null,
      name: '窗长端点',
      baseUrl: 'https://api.example.com/v1',
      apiKey: 'sk-live-1234567890',
      models: TIERS,
      protocol: 'openai',
      tuning: {
        contextLength: 200000,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      },
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
        protocol: 'openai',
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
      expect(saved?.contextLength).toBeNull();
    });

    const saveCall = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider');
    expect(saveCall).toBeDefined();
    expect(saveCall?.[1]?.tuning?.contextLength).toBeNull();
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
        protocol: 'openai',
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
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
        protocol: 'openai',
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
    });
    await act(async () => {
      await result.current.remove(1);
    });

    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(result.current.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 协议 / 输出上限入参（DD20 / AC-11 前端半）：`AgentProviderSaveInput` 八参
// 逐键与命令签名对齐，`maxTokens` 留空 = 按协议缺省存 null（不落 0 / 32000）
// ---------------------------------------------------------------------------

describe('useAgentProviders：协议与输出上限入参（DD20）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('save 七形参逐键与 save_agent_provider 签名对齐（protocol 紧跟 name、tuning 居末位）', async () => {
    const response: AgentProviderRecord = {
      id: 3,
      name: '协议端点',
      protocol: 'anthropic',
      baseUrl: 'https://api.anthropic.com',
      apiKey: 'sk-ant-x',
      models: { high: 'claude-x[1m]', medium: 'claude-m', low: 'claude-y' },
      contextLength: 200000,
      maxTokens: 8192,
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(response);
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    let returned: AgentProviderRecord | null = null;
    await act(async () => {
      returned = await result.current.save({
        id: null,
        name: '协议端点',
        protocol: 'anthropic',
        baseUrl: 'https://api.anthropic.com',
        apiKey: 'sk-ant-x',
        models: { high: 'claude-x[1m]', medium: 'claude-m', low: 'claude-y' },
        contextLength: 200000,
        maxTokens: 8192,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
    });

    // 逐键断言（键名 = 命令签名序，位置错序即失败）
    expect(invokeMock).toHaveBeenCalledWith('save_agent_provider', {
      id: null,
      name: '协议端点',
      protocol: 'anthropic',
      baseUrl: 'https://api.anthropic.com',
      apiKey: 'sk-ant-x',
      models: { high: 'claude-x[1m]', medium: 'claude-m', low: 'claude-y' },
      tuning: {
        contextLength: 200000,
        maxTokens: 8192,
        webSearch: false,
        efforts: { high: '', low: '' },
      },
    });
    expect(Object.keys(invokeMock.mock.calls[1]?.[1] ?? {})).toEqual([
      'id',
      'name',
      'protocol',
      'baseUrl',
      'apiKey',
      'models',
      'tuning',
    ]);
    // 返回值透传记录（hook 零裁剪）
    expect(returned).toEqual(response);
  });

  it('maxTokens 留空 → invoke 实参 maxTokens: null（与 contextLength: null 双空并存）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save({
        id: null,
        name: '未配置上限',
        protocol: 'openai',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: TIERS,
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
    });

    const params = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider')?.[1] as
      | Record<string, unknown>
      | undefined;
    // 两可空列居末位 tuning 组（位置参数逐位对齐命令签名）
    const tuning = params?.['tuning'] as Record<string, unknown> | undefined;
    expect(tuning?.['maxTokens']).toBeNull();
    expect(tuning?.['contextLength']).toBeNull();
    // 留空 = 按协议缺省存 null（MUST NOT 落 0 / 32000 字面）
    expect(tuning?.['maxTokens']).not.toBe(0);
    expect(tuning?.['maxTokens']).not.toBe(32000);
  });

  it('maxTokens 数值边界原样透传（0 / 4096 / MAX_SAFE_INTEGER，hook 不做边界校验）', async () => {
    for (const tokens of [0, 4096, Number.MAX_SAFE_INTEGER]) {
      invokeMock.mockReset();
      invokeMock.mockImplementation((command: string) => {
        if (command === 'list_agent_providers') return Promise.resolve([]);
        return Promise.resolve(null);
      });
      const { result } = renderHook(() => useAgentProviders());
      await settleLoaded();

      await act(async () => {
        await result.current.save({
          id: null,
          name: `边界端点 ${tokens}`,
          protocol: 'anthropic',
          baseUrl: 'https://api.anthropic.com',
          apiKey: 'sk-ant-x',
          models: TIERS,
          contextLength: null,
          maxTokens: tokens,
          webSearch: false,
          efforts: { high: '', low: '' },
        });
      });

      const params = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider')?.[1] as
        | Record<string, unknown>
        | undefined;
      const tuning = params?.['tuning'] as Record<string, unknown> | undefined;
      expect(tuning?.['maxTokens']).toBe(tokens);
      expect(tuning?.['contextLength']).toBeNull();
    }
  });

  it('list 回读 protocol / maxTokens 原样承载（两变体 + null 与数值，hook 零归一化）', async () => {
    const rows: AgentProviderRecord[] = [
      {
        id: 1,
        name: 'openai 端点',
        protocol: 'openai',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: TIERS,
        contextLength: null,
        maxTokens: null,
      },
      {
        id: 2,
        name: 'anthropic 端点',
        protocol: 'anthropic',
        baseUrl: 'https://api.anthropic.com',
        apiKey: 'sk-ant-1234567890',
        models: TIERS,
        contextLength: 200000,
        maxTokens: 8192,
      },
    ];
    invokeMock.mockResolvedValue(rows);
    const { result } = renderHook(() => useAgentProviders());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.providers).toEqual(rows);
    expect(result.current.providers[0].protocol).toBe('openai');
    expect(result.current.providers[0].maxTokens).toBeNull();
    expect(result.current.providers[1].protocol).toBe('anthropic');
    expect(result.current.providers[1].maxTokens).toBe(8192);
  });

  it('组合：面板提交形态入参（anthropic + 8192 + 三档含 [1m]）经 invoke 载荷逐键对齐', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save({
        id: 7,
        name: '面板形态端点',
        protocol: 'anthropic',
        baseUrl: 'https://api.anthropic.com',
        apiKey: '',
        models: { high: 'claude-sonnet-4-5[1m]', medium: 'claude-m', low: 'claude-haiku-4-5' },
        contextLength: 200000,
        maxTokens: 8192,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
    });

    expect(invokeMock).toHaveBeenCalledWith('save_agent_provider', {
      id: 7,
      name: '面板形态端点',
      protocol: 'anthropic',
      baseUrl: 'https://api.anthropic.com',
      apiKey: '',
      models: { high: 'claude-sonnet-4-5[1m]', medium: 'claude-m', low: 'claude-haiku-4-5' },
      tuning: {
        contextLength: 200000,
        maxTokens: 8192,
        webSearch: false,
        efforts: { high: '', low: '' },
      },
    });
  });
});

// ---------------------------------------------------------------------------
// tuning 组位置参（AC-11 invoke 位置参数）：四项旁路载荷收末位一组，值逐位透传
// ---------------------------------------------------------------------------

describe('useAgentProviders：tuning 组位置参数（webSearch / 可空列组合）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('webSearch: true → 载荷末位 tuning 组内为 true 且键序七位逐位对齐', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(provider(1, '搜索端点'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save({
        id: null,
        name: '搜索端点',
        protocol: 'anthropic',
        baseUrl: 'https://api.anthropic.com',
        apiKey: 'sk-ant-x',
        models: TIERS,
        contextLength: null,
        maxTokens: null,
        webSearch: true,
        efforts: { high: '', low: '' },
      });
    });

    const saveCall = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider');
    expect(saveCall?.[1]).toEqual({
      id: null,
      name: '搜索端点',
      protocol: 'anthropic',
      baseUrl: 'https://api.anthropic.com',
      apiKey: 'sk-ant-x',
      models: TIERS,
      tuning: {
        contextLength: null,
        maxTokens: null,
        webSearch: true,
        efforts: { high: '', low: '' },
      },
    });
    // 位置序与命令签名逐位对齐（tuning 组居末位）
    expect(Object.keys(saveCall?.[1] ?? {})).toEqual([
      'id',
      'name',
      'protocol',
      'baseUrl',
      'apiKey',
      'models',
      'tuning',
    ]);
  });

  it('webSearch: false 亦不省略（不得落 undefined——省略会与「保持原值」混淆）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(provider(2, '关态端点'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save({
        id: null,
        name: '关态端点',
        protocol: 'openai',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: TIERS,
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
    });

    const saveCall = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider');
    expect(Object.keys(saveCall?.[1]?.tuning ?? {})).toContain('webSearch');
    expect(saveCall?.[1]?.tuning?.webSearch).toBe(false);
    expect(saveCall?.[1]?.tuning?.webSearch).not.toBeUndefined();
  });

  it('可空列四组合与数值三档原样透传，tuning 组仍居末位', async () => {
    const combos: {
      contextLength: number | null;
      maxTokens: number | null;
      webSearch: boolean;
    }[] = [
      { contextLength: null, maxTokens: null, webSearch: true },
      { contextLength: null, maxTokens: null, webSearch: false },
      { contextLength: 200_000, maxTokens: 8192, webSearch: true },
      { contextLength: 0, maxTokens: 4096, webSearch: false },
      {
        contextLength: Number.MAX_SAFE_INTEGER,
        maxTokens: Number.MAX_SAFE_INTEGER,
        webSearch: true,
      },
    ];

    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(provider(3, '组合端点'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    for (const combo of combos) {
      await act(async () => {
        await result.current.save({
          id: null,
          name: '组合端点',
          protocol: 'anthropic',
          baseUrl: 'https://api.anthropic.com',
          apiKey: 'sk-ant-x',
          models: TIERS,
          contextLength: combo.contextLength,
          maxTokens: combo.maxTokens,
          webSearch: combo.webSearch,
          efforts: { high: '', low: '' },
        });
      });

      const calls = invokeMock.mock.calls.filter(([name]) => name === 'save_agent_provider');
      const payload = calls.at(-1)?.[1] ?? {};
      // 数值原样透传（不夹取、不转字符串）
      expect(payload.tuning.contextLength).toBe(combo.contextLength);
      expect(payload.tuning.maxTokens).toBe(combo.maxTokens);
      expect(payload.tuning.webSearch).toBe(combo.webSearch);
      expect(Object.keys(payload).at(-1)).toBe('tuning');
    }
  });

  it('invoke reject 且 webSearch: true → toast 固定前缀与返回 null 不变（错误面不因开关改变）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.reject(new Error('重名 provider'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      const saved = await result.current.save({
        id: null,
        name: '失败端点',
        protocol: 'anthropic',
        baseUrl: 'https://api.anthropic.com',
        apiKey: 'sk-ant-x',
        models: TIERS,
        contextLength: null,
        maxTokens: null,
        webSearch: true,
        efforts: { high: '', low: '' },
      });
      expect(saved).toBeNull();
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(String(toastErrorMock.mock.calls[0]?.[0])).toContain('保存 provider 失败：');
    expect(result.current.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 经 `ipc` 的运行期校验面（change ipc-zod-runtime-schema）：`saveAgentProvider`
// 的七形参（末位 `tuning` 组）与嵌套 `models` strict 面、三条既有 reject 文案、
// 结果面 soft-report。
// ---------------------------------------------------------------------------

/** 合法 save 入参（id / contextLength / maxTokens 三个可空位取 null）。 */
function saveInput(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: null,
    name: 'prov',
    protocol: 'openai',
    baseUrl: 'https://api.example.com/v1',
    apiKey: 'sk-live-1234567890',
    models: TIERS,
    contextLength: null,
    maxTokens: null,
    webSearch: false,
    efforts: { high: '', low: '' },
    ...overrides,
  };
}

describe('useAgentProviders：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
    resetDrift();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('七形参齐全（末位 tuning 组）、三可空位为 null → 放行；payload 键名逐字命中、invoke 恰 1 次', async () => {
    mockDispatch();
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save(saveInput() as never);
    });

    expect(countOf('save_agent_provider')).toBe(1);
    expect(invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider')?.[1]).toEqual({
      id: null,
      name: 'prov',
      protocol: 'openai',
      baseUrl: 'https://api.example.com/v1',
      apiKey: 'sk-live-1234567890',
      models: TIERS,
      tuning: {
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      },
    });
    expect(driftCount()).toBe(0);
  });

  it('嵌套 models 三档齐全 → strict 深层放行、结果干净零漂移', async () => {
    mockDispatch();
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save(
        saveInput({ id: 1, models: { high: 'h', medium: 'm', low: 'l' } }) as never,
      );
    });

    expect(driftCount()).toBe(0);
    expect(result.current.providers).toHaveLength(1);
  });

  it.each([
    ['模型名非法', '保存失败：models.high 非法模型名：m high'],
    ['maxTokens 低于下限', '保存失败：maxTokens 须 >= 4096'],
    ['api_key 结构外串', '保存失败：api_key 非法结构'],
  ])('后端 reject（%s）→ 既有 toast 文案不变、零漂移', async (_label, message) => {
    invokeMock.mockImplementation((command: string) =>
      command === 'list_agent_providers' ? Promise.resolve([]) : Promise.reject(message),
    );
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save(saveInput() as never);
    });

    expect(toastErrorMock).toHaveBeenCalledWith(`保存 provider 失败：${message}`);
    expect(driftCount()).toBe(0);
  });

  it('models 多传未知子键 → 本地 IpcArgsError、invoke 零调用（strict 深层，零往返）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      // 落库旁路的出线是 `void`（返回 `{}` 会被 drift 记为结果面漂移）
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();
    const afterLoad = countOf('save_agent_provider');

    await act(async () => {
      await result.current.save(saveInput({ models: { ...TIERS, surplus: 'x' } }) as never);
    });

    expect(countOf('save_agent_provider')).toBe(afterLoad);
    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(String(toastErrorMock.mock.calls[0][0])).toContain('IpcArgsError');
    expect(driftCount()).toBe(0);
  });

  it.each([
    [true, 0],
    [false, 0],
    [true, 100],
    [false, 100],
  ])(
    'webSearch=%s 与数值下界=%i 组合 → 前端放行（下限校验单点在 Rust）',
    async (webSearch, bound) => {
      mockDispatch();
      const { result } = renderHook(() => useAgentProviders());
      await settleLoaded();

      await act(async () => {
        await result.current.save(
          saveInput({ webSearch, contextLength: bound, maxTokens: bound }) as never,
        );
      });

      expect(countOf('save_agent_provider')).toBe(1);
      expect(driftCount()).toBe(0);
    },
  );

  it('空 provider 清单 → 放行、零漂移、既有空态不变', async () => {
    invokeMock.mockResolvedValue([]);
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    expect(result.current.providers).toEqual([]);
    expect(driftCount()).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 两档 effort 入参（AC-3 / AC-12 前端半）：`AgentProviderSaveInput.efforts` 与
// 命令末位 `tuning` 组内的 `efforts` 逐位对齐，空串照携（省略会与「保持原值」
// 混淆）；回读含 efforts 的记录原样承载（hook 零归一化，trim 归后端单点）。
// ---------------------------------------------------------------------------

/** 合法 save 入参（efforts 可覆写；其余取最小合法形态）。 */
function saveInputWithEfforts(
  efforts: { high: string; low: string },
  overrides: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    id: null,
    name: '档位端点',
    protocol: 'openai',
    baseUrl: 'https://api.example.com/v1',
    apiKey: 'sk-live-1234567890',
    models: TIERS,
    contextLength: null,
    maxTokens: null,
    webSearch: false,
    efforts,
    ...overrides,
  };
}

describe('useAgentProviders：两档 effort 入参', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('save 携非空两档 ⇒ invoke 载荷 efforts 逐键相等且居 webSearch 之后（tuning 组逐位对齐）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(provider(1, '档位端点'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save(saveInputWithEfforts({ high: 'max', low: 'low' }) as never);
    });

    const payload = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider')?.[1];
    expect(payload?.tuning?.efforts).toEqual({ high: 'max', low: 'low' });
    expect(Object.keys(payload?.tuning ?? {})).toEqual([
      'contextLength',
      'maxTokens',
      'webSearch',
      'efforts',
    ]);
  });

  it.each([
    ['双空', { high: '', low: '' }],
    ['单空', { high: 'max', low: '' }],
  ])('%s ⇒ 载荷恒含 efforts 键（不省略、不落 undefined / null）', async (_label, efforts) => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(provider(1, '档位端点'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save(saveInputWithEfforts(efforts) as never);
    });

    const payload = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider')?.[1];
    expect(Object.keys(payload?.tuning ?? {})).toContain('efforts');
    expect(payload?.tuning?.efforts).toEqual(efforts);
    expect(payload?.tuning?.efforts).not.toBeUndefined();
  });

  it.each([
    ['目录外自定义值', { high: 'ultra-thinking', low: 'none' }],
    ['带两侧空白值', { high: ' max ', low: '\tlow\n' }],
    ['超长与特殊字符', { high: 'h'.repeat(1200), low: '思考 🎉 v2' }],
  ])('%s ⇒ 原样透传（hook 零归一化、零 trim）', async (_label, efforts) => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_agent_providers') return Promise.resolve([]);
      return Promise.resolve(provider(1, '档位端点'));
    });
    const { result } = renderHook(() => useAgentProviders());
    await settleLoaded();

    await act(async () => {
      await result.current.save(saveInputWithEfforts(efforts) as never);
    });

    const payload = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider')?.[1];
    expect(payload?.tuning?.efforts).toEqual(efforts);
  });

  it.each([
    ['openai', { high: '', low: '' }],
    ['openai', { high: 'max', low: 'low' }],
    ['anthropic', { high: '', low: '' }],
    ['anthropic', { high: 'xhigh', low: 'low' }],
  ])('回读 %s 协议 + efforts=%o ⇒ 两档原样承载（四组合全覆盖）', async (protocol, efforts) => {
    const row: AgentProviderRecord = {
      ...provider(1, '回读端点'),
      protocol: protocol as AgentProviderRecord['protocol'],
      efforts,
    };
    invokeMock.mockResolvedValue([row]);
    const { result } = renderHook(() => useAgentProviders());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.providers[0]?.efforts).toEqual(efforts);
    expect(toastErrorMock).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 通知门面落库旁路（desktop-notification-hub AC-2）：两动作失败原文案不变 +
// `notification_record` 各恰一次
// ---------------------------------------------------------------------------

/** `notification_record` 的落库入参序列。 */
function recordCalls(): { level: string; message: string; body: string | null }[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === 'notification_record')
    .map(
      ([, params]) =>
        (params as { record: { level: string; message: string; body: string | null } }).record,
    );
}

const SAVE_INPUT = {
  id: null,
  name: '新 provider',
  protocol: 'openai' as const,
  baseUrl: 'https://api.example.com/v1',
  apiKey: 'sk-live-1234567890',
  models: TIERS,
  contextLength: null,
  maxTokens: null,
  webSearch: false,
  efforts: { high: 'high' as const, low: 'low' as const },
};

describe('useAgentProviders：失败提示的门面落库旁路', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('正向：save / remove 各失败一次 → 原文案 + 落库各一次（error 档、无副文案）', async () => {
    const actions = [
      {
        command: 'save_agent_provider',
        reason: 'db: provider 名已存在',
        prefix: '保存 provider 失败：',
        run: (hook: ReturnType<typeof useAgentProviders>) => hook.save(SAVE_INPUT),
      },
      {
        command: 'delete_agent_provider',
        reason: 'db: provider 被引用',
        prefix: '删除 provider 失败：',
        run: (hook: ReturnType<typeof useAgentProviders>) => hook.remove(1),
      },
    ];

    for (const action of actions) {
      invokeMock.mockImplementation((name: string) => {
        if (name === action.command) return Promise.reject(new Error(action.reason));
        if (name === 'list_agent_providers') return Promise.resolve([...stored]);
        return Promise.resolve(null);
      });
      toastErrorMock.mockReset();
      stored = [provider(1, 'cli-a')];
      const { result } = renderHook(() => useAgentProviders());
      await waitFor(() => expect(result.current.loading).toBe(false));
      invokeMock.mockClear();

      await act(async () => {
        await action.run(result.current);
      });

      const message = `${action.prefix}Error: ${action.reason}`;
      expect(toastErrorMock).toHaveBeenCalledTimes(1);
      expect(toastErrorMock).toHaveBeenCalledWith(message);
      expect(recordCalls()).toHaveLength(1);
      expect(recordCalls()[0].level).toBe('error');
      expect(recordCalls()[0].message).toBe(message);
      expect(recordCalls()[0].body).toBeNull();
    }
  });

  it('边界：同一动作重复失败两次 → 两次提示、两次落库（不去重）', async () => {
    invokeMock.mockImplementation((name: string) => {
      if (name === 'delete_agent_provider') return Promise.reject(new Error('db: provider 被引用'));
      if (name === 'list_agent_providers') return Promise.resolve([...stored]);
      return Promise.resolve(null);
    });
    stored = [provider(1, 'cli-a')];
    const { result } = renderHook(() => useAgentProviders());
    await waitFor(() => expect(result.current.loading).toBe(false));
    invokeMock.mockClear();

    await act(async () => {
      await result.current.remove(1);
    });
    await act(async () => {
      await result.current.remove(1);
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(2);
    expect(recordCalls()).toHaveLength(2);
  });

  it('正向：两动作成功路径零 toast、零落库', async () => {
    mockDispatch();
    stored = [provider(1, 'cli-a')];
    const { result } = renderHook(() => useAgentProviders());
    await waitFor(() => expect(result.current.loading).toBe(false));
    invokeMock.mockClear();

    await act(async () => {
      await result.current.remove(1);
    });

    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(recordCalls()).toHaveLength(0);
  });

  it('异常：落库 reject 时既有 toast 断言仍成立、清单刷新零变化', async () => {
    invokeMock.mockImplementation((name: string) => {
      if (name === 'delete_agent_provider') return Promise.reject(new Error('db: provider 被引用'));
      if (name === 'list_agent_providers') return Promise.resolve([...stored]);
      if (name === 'notification_record') return Promise.reject(new Error('落库失败'));
      return Promise.resolve(null);
    });
    stored = [provider(1, 'cli-a')];
    const { result } = renderHook(() => useAgentProviders());
    await waitFor(() => expect(result.current.loading).toBe(false));
    invokeMock.mockClear();

    await act(async () => {
      await result.current.remove(1);
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(toastErrorMock).toHaveBeenCalledWith('删除 provider 失败：Error: db: provider 被引用');
    expect(countOf('list_agent_providers')).toBe(0);
  });
});
