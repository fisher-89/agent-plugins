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
      contextLength: null,
      maxTokens: null,
      webSearch: false,
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
      contextLength: 200000,
      maxTokens: null,
      webSearch: false,
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
      });
      expect(saved?.contextLength).toBeNull();
    });

    const saveCall = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider');
    expect(saveCall).toBeDefined();
    expect(saveCall?.[1]?.contextLength).toBeNull();
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

  it('save 九参逐键与 save_agent_provider 签名对齐（protocol 紧跟 name、webSearch 居末位）', async () => {
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
      contextLength: 200000,
      maxTokens: 8192,
      webSearch: false,
    });
    expect(Object.keys(invokeMock.mock.calls[1]?.[1] ?? {})).toEqual([
      'id',
      'name',
      'protocol',
      'baseUrl',
      'apiKey',
      'models',
      'contextLength',
      'maxTokens',
      'webSearch',
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
      });
    });

    const params = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider')?.[1] as
      | Record<string, unknown>
      | undefined;
    expect(params?.['maxTokens']).toBeNull();
    expect(params?.['contextLength']).toBeNull();
    // 留空 = 按协议缺省存 null（MUST NOT 落 0 / 32000 字面）
    expect(params?.['maxTokens']).not.toBe(0);
    expect(params?.['maxTokens']).not.toBe(32000);
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
        });
      });

      const params = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider')?.[1] as
        | Record<string, unknown>
        | undefined;
      expect(params?.['maxTokens']).toBe(tokens);
      expect(params?.['contextLength']).toBeNull();
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
      });
    });

    expect(invokeMock).toHaveBeenCalledWith('save_agent_provider', {
      id: 7,
      name: '面板形态端点',
      protocol: 'anthropic',
      baseUrl: 'https://api.anthropic.com',
      apiKey: '',
      models: { high: 'claude-sonnet-4-5[1m]', medium: 'claude-m', low: 'claude-haiku-4-5' },
      contextLength: 200000,
      maxTokens: 8192,
      webSearch: false,
    });
  });
});

// ---------------------------------------------------------------------------
// webSearch 第九位置参（AC-11 invoke 位置参数）：true / false 均逐位透传
// ---------------------------------------------------------------------------

describe('useAgentProviders：webSearch 位置参数（true / false 与可空列组合）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    stored = [];
  });

  it('webSearch: true → 载荷末位为 true 且键序九位逐位对齐', async () => {
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
      contextLength: null,
      maxTokens: null,
      webSearch: true,
    });
    // 位置序与命令签名逐位对齐（webSearch 居末位）
    expect(Object.keys(saveCall?.[1] ?? {})).toEqual([
      'id',
      'name',
      'protocol',
      'baseUrl',
      'apiKey',
      'models',
      'contextLength',
      'maxTokens',
      'webSearch',
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
      });
    });

    const saveCall = invokeMock.mock.calls.find(([name]) => name === 'save_agent_provider');
    expect(Object.keys(saveCall?.[1] ?? {})).toContain('webSearch');
    expect(saveCall?.[1]?.webSearch).toBe(false);
    expect(saveCall?.[1]?.webSearch).not.toBeUndefined();
  });

  it('可空列四组合与数值三档原样透传，webSearch 仍居末位', async () => {
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
        });
      });

      const calls = invokeMock.mock.calls.filter(([name]) => name === 'save_agent_provider');
      const payload = calls.at(-1)?.[1] ?? {};
      // 数值原样透传（不夹取、不转字符串）
      expect(payload.contextLength).toBe(combo.contextLength);
      expect(payload.maxTokens).toBe(combo.maxTokens);
      expect(payload.webSearch).toBe(combo.webSearch);
      expect(Object.keys(payload).at(-1)).toBe('webSearch');
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
      });
      expect(saved).toBeNull();
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(String(toastErrorMock.mock.calls[0]?.[0])).toContain('保存 provider 失败：');
    expect(result.current.error).toBeNull();
  });
});
