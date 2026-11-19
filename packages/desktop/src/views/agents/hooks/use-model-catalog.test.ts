import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ModelCatalog } from '../../../types/generated/bindings';
import { useModelCatalog } from './use-model-catalog';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发替身（list_model_catalog，可切换 resolve /
// reject 并记录调用序列）；sonner 以 vi.fn 替身注入 toast.error——目录是建议面，
// 降级路径 MUST NOT 弹提示，故断言零调用。
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

/** 两级目录 fixture（serde camelCase 线格式：协议段 + 协议级 efforts + 模型条目）。 */
function catalogFixture(): ModelCatalog {
  return {
    protocols: [
      {
        protocol: 'anthropic',
        efforts: ['low', 'medium', 'high', 'xhigh', 'max'],
        models: [
          {
            value: 'claude-sonnet-4-6',
            contextLength: 200000,
            maxTokens: 64000,
            supportsThinking: true,
            efforts: null,
          },
          {
            value: 'claude-3-5-haiku-latest',
            contextLength: 200000,
            maxTokens: 8192,
            supportsThinking: false,
            efforts: [],
          },
        ],
      },
    ],
  };
}

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

describe('useModelCatalog：挂载一次取数（无轮询）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
  });

  it('挂载 → invoke list_model_catalog 恰一次，loading 收敛且 catalog 三类型逐字段回填', async () => {
    const fixture = catalogFixture();
    invokeMock.mockResolvedValue(fixture);

    const { result } = renderHook(() => useModelCatalog());

    expect(result.current.loading).toBe(true);
    expect(result.current.catalog).toBeNull();

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(countOf('list_model_catalog')).toBe(1);
    expect(result.current.catalog).toEqual(fixture);
    // 三类型逐字段：协议 / 协议级建议值域 / 条目元数据
    const protocol = result.current.catalog?.protocols[0];
    expect(protocol?.protocol).toBe('anthropic');
    expect(protocol?.efforts).toEqual(['low', 'medium', 'high', 'xhigh', 'max']);
    expect(protocol?.models[0]?.value).toBe('claude-sonnet-4-6');
    expect(protocol?.models[0]?.contextLength).toBe(200000);
    expect(toastErrorMock).not.toHaveBeenCalled();
  });

  it('非重挂载的重新渲染不重复取数（目录是编译期常量，无刷新入口）', async () => {
    invokeMock.mockResolvedValue(catalogFixture());

    const { result, rerender } = renderHook(() => useModelCatalog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    rerender();
    rerender();
    await act(async () => {
      await Promise.resolve();
    });

    expect(countOf('list_model_catalog')).toBe(1);
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});

describe('useModelCatalog：失败降级（建议面零阻断）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
  });

  it('invoke reject → catalog 为 null、loading 收敛、无 error 态、toast 零调用、不重试', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));

    const { result } = renderHook(() => useModelCatalog());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.catalog).toBeNull();
    expect(result.current.loading).toBe(false);
    // hook 面只有 catalog / loading 两键：无 error 态可暴露
    expect(Object.keys(result.current).sort()).toEqual(['catalog', 'loading']);
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(countOf('list_model_catalog')).toBe(1);
  });

  it('空目录（protocols: []）→ catalog 非 null 且 protocols 为空数组（「取到空目录」与「取数失败」两态可辨）', async () => {
    invokeMock.mockResolvedValue({ protocols: [] });

    const { result } = renderHook(() => useModelCatalog());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.catalog).not.toBeNull();
    expect(result.current.catalog?.protocols).toEqual([]);
  });

  it('卸载先于 resolve → 不 setState、无告警、catalog 保持 null', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    let release: (value: ModelCatalog) => void = () => {};
    invokeMock.mockImplementation(
      () =>
        new Promise<ModelCatalog>((resolve) => {
          release = resolve;
        }),
    );

    const { result, unmount } = renderHook(() => useModelCatalog());
    unmount();

    await act(async () => {
      release(catalogFixture());
      await Promise.resolve();
    });

    expect(result.current.catalog).toBeNull();
    expect(result.current.loading).toBe(true);
    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it('条目元数据全缺席（contextLength / maxTokens / efforts 皆 null）→ 原样承载（hook 零归一化）', async () => {
    const bare: ModelCatalog = {
      protocols: [
        {
          protocol: 'openai',
          efforts: ['low', 'high', 'max'],
          models: [
            {
              value: 'bare-model',
              contextLength: null,
              maxTokens: null,
              supportsThinking: true,
              efforts: null,
            },
          ],
        },
      ],
    };
    invokeMock.mockResolvedValue(bare);

    const { result } = renderHook(() => useModelCatalog());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.catalog).toEqual(bare);
    const entry = result.current.catalog?.protocols[0]?.models[0];
    expect(entry?.contextLength).toBeNull();
    expect(entry?.maxTokens).toBeNull();
    expect(entry?.efforts).toBeNull();
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});
