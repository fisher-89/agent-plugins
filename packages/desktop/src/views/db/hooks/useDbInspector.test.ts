import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ModelInfo, RecordEnvelope } from '../../../types/dto';
import { useDbInspector } from './useDbInspector';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

// ---------------------------------------------------------------------------
// useDbInspector 取数收口单测：IPC 边界（@tauri-apps/api/core）以 invoke 替身
// 按命令名分发固定 fixture 并记录调用序列（沿 useWorkspaces.test.ts 替身模式）。
// 查询轨无 toast：hook 不引 sonner（错误 inline 由视图承载），不 mock sonner。
// 页长 50 与 hook 内 PAGE_SIZE 常量同源（未导出，按实现值字面书写）。
// ---------------------------------------------------------------------------

/** 模型清单 fixture：agent_run 载荷用于翻页矩阵（60 = 1 整页 + 10）。 */
const MODELS: ModelInfo[] = [
  { name: 'workspace', count: 0 },
  { name: 'agent_run', count: 60 },
  { name: 'agent_event', count: 3 },
];

function envelopes(count: number, from = 0): RecordEnvelope[] {
  return Array.from({ length: count }, (_, index) => ({
    key: from + index,
    value: { id: from + index },
  }));
}

/** 按命令名分发：db_models 恒回清单，db_records 按 offset/limit 切库存。 */
function mockDispatch(inventory: RecordEnvelope[] = []) {
  invokeMock.mockImplementation(
    (command: string, params?: { model?: string; offset?: number; limit?: number }) => {
      if (command === 'db_models') {
        return Promise.resolve(MODELS.map((model) => ({ ...model })));
      }
      if (command === 'db_records') {
        const offset = params?.offset ?? 0;
        const limit = params?.limit ?? 50;
        return Promise.resolve(inventory.slice(offset, offset + limit).map((e) => ({ ...e })));
      }
      return Promise.resolve(null);
    },
  );
}

/** mock.calls 中某命令的调用次数。 */
function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

/** mock.calls 中某命令最近一次的参数。 */
function lastParamsOf(command: string): Record<string, unknown> | undefined {
  const last = invokeMock.mock.calls.filter(([name]) => name === command).at(-1);
  return last?.[1] as Record<string, unknown> | undefined;
}

async function selectModel(result: { current: ReturnType<typeof useDbInspector> }, name: string) {
  act(() => {
    result.current.selectModel(name);
  });
  await waitFor(() => expect(result.current.selected).toBe(name));
}

describe('useDbInspector：挂载取数', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // 置于套件首位（沿 useWorkspaces 稳定判据模式）：失控循环在逐拍解析下被
  // 稳定判据快速观测并判失败
  it('取数链稳定：挂载后调用数不再增长（无轮询、无恢复循环、无多余 db_records）', async () => {
    // 每个响应经 setTimeout(0) 宏任务逐拍解析：失控循环每拍只走一轮，可被稳定判据观测
    invokeMock.mockImplementation(
      (command: string) =>
        new Promise((resolve) =>
          setTimeout(() => resolve(command === 'db_models' ? MODELS : []), 0),
        ),
    );
    const { result } = renderHook(() => useDbInspector());

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

    expect(countOf('db_models')).toBe(1);
    expect(invokeMock).toHaveBeenCalledTimes(1); // 除挂载取数外无任何命令
    expect(result.current.models).toHaveLength(3);
    expect(result.current.error).toBeNull();
  });

  it('挂载即 invoke("db_models") 恰一次，loading 置位后随成功清除', async () => {
    let resolveLoad: (models: ModelInfo[]) => void = () => {};
    invokeMock.mockImplementation(
      (command: string) =>
        new Promise<ModelInfo[]>((resolve) => {
          if (command === 'db_models') resolveLoad = resolve;
        }),
    );
    const { result } = renderHook(() => useDbInspector());

    // 挂载取数进行中：loading 置位（挂载 effect 显式置位）
    expect(result.current.loading).toBe(true);

    await act(async () => {
      resolveLoad([...MODELS]);
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.models).toHaveLength(3);
    expect(result.current.error).toBeNull();
    expect(invokeMock).toHaveBeenCalledWith('db_models');
    expect(countOf('db_models')).toBe(1);
  });

  it('静置后调用数不增长且未选中模型时不取记录（AC-7 只读取数纪律）', async () => {
    mockDispatch();
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));
    const baseline = invokeMock.mock.calls.length;

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });

    expect(invokeMock.mock.calls.length).toBe(baseline);
    expect(countOf('db_records')).toBe(0); // 未选中模型不取数
    expect(result.current.records).toEqual([]);
  });
});

describe('useDbInspector：selectModel 与翻页', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('selectModel 重置 offset=0 并以 {model, offset:0, limit:50} 取第一页', async () => {
    const inventory = envelopes(60);
    mockDispatch(inventory);
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await selectModel(result, 'agent_run');

    await waitFor(() => expect(result.current.records).toHaveLength(50));
    expect(invokeMock).toHaveBeenCalledWith('db_records', {
      model: 'agent_run',
      offset: 0,
      limit: 50,
    });
    expect(result.current.offset).toBe(0);
    expect(result.current.hasMore).toBe(true); // 整页（50）→ 可能还有下一页
  });

  it('连续两次 selectModel：两次动作均触发取数，最终状态与最后选择一致', async () => {
    mockDispatch(envelopes(10));
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await selectModel(result, 'agent_run');
    await waitFor(() => expect(countOf('db_records')).toBe(1));
    await selectModel(result, 'agent_event');
    await waitFor(() => expect(countOf('db_records')).toBe(2));

    expect(result.current.selected).toBe('agent_event');
    expect(result.current.offset).toBe(0);
    expect(lastParamsOf('db_records')).toEqual({ model: 'agent_event', offset: 0, limit: 50 });
  });

  it('nextPage / prevPage 以 PAGE_SIZE 步进取对应页，refresh 以当前 offset 重取', async () => {
    const inventory = envelopes(120);
    mockDispatch(inventory);
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await selectModel(result, 'agent_run');
    await waitFor(() => expect(result.current.records).toHaveLength(50));

    act(() => {
      result.current.nextPage();
    });
    await waitFor(() => expect(result.current.offset).toBe(50));
    await waitFor(() => expect(result.current.records[0].key).toBe(50));
    expect(lastParamsOf('db_records')).toEqual({ model: 'agent_run', offset: 50, limit: 50 });

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(countOf('db_records')).toBe(3));
    expect(lastParamsOf('db_records')).toEqual({ model: 'agent_run', offset: 50, limit: 50 });

    act(() => {
      result.current.prevPage();
    });
    await waitFor(() => expect(result.current.offset).toBe(0));
    await waitFor(() => expect(result.current.records[0].key).toBe(0));
    expect(lastParamsOf('db_records')).toEqual({ model: 'agent_run', offset: 0, limit: 50 });
  });

  it('首页 prevPage 不动：offset 保持 0 且不触发负 offset 取数', async () => {
    mockDispatch(envelopes(10));
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await selectModel(result, 'agent_run');
    await waitFor(() => expect(result.current.records).toHaveLength(10));
    const recordsCalls = countOf('db_records');

    act(() => {
      result.current.prevPage();
    });

    expect(result.current.offset).toBe(0);
    expect(countOf('db_records')).toBe(recordsCalls); // offset 未变 → 不重取
  });

  it('记录数恰为整页倍数：hasMore 判定使翻页拼接不重不漏', async () => {
    mockDispatch(envelopes(100)); // 恰两整页
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await selectModel(result, 'agent_run');
    await waitFor(() => expect(result.current.records).toHaveLength(50));
    expect(result.current.hasMore).toBe(true);
    const page1Keys = result.current.records.map((envelope) => envelope.key);

    act(() => {
      result.current.nextPage();
    });
    await waitFor(() => expect(result.current.records[0]?.key).toBe(50));
    const page2Keys = result.current.records.map((envelope) => envelope.key);
    // 恰为整页倍数的边界：第二页仍整页 → hasMore 判 true（再翻得空页后转 false）
    expect(page2Keys).toHaveLength(50);
    expect(result.current.hasMore).toBe(true);

    const union = [...page1Keys, ...page2Keys];
    expect(new Set(union).size).toBe(union.length); // 不重
    expect(union).toHaveLength(100); // 不漏
    expect(union.every((key, index) => key === index)).toBe(true); // 拼接即全集自然序
  });
});

describe('useDbInspector：错误态（查询轨 inline，无 toast）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('db_models reject：error 置位含错误串、models 保持空数组、无未捕获异常', async () => {
    invokeMock.mockRejectedValue(new Error('db: 清单打开失败'));
    const { result } = renderHook(() => useDbInspector());

    await waitFor(() => expect(result.current.error).not.toBeNull());

    expect(result.current.error).toContain('db: 清单打开失败');
    expect(result.current.models).toEqual([]);
    expect(result.current.loading).toBe(false);
  });

  it('db_records reject：recordsError 置位、已选模型与清单不丢失', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return Promise.reject(new Error('db: 未知模型'));
    });
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.models).toHaveLength(3));

    await selectModel(result, 'agent_run');
    await waitFor(() => expect(result.current.recordsError).not.toBeNull());

    expect(result.current.recordsError).toContain('db: 未知模型');
    expect(result.current.selected).toBe('agent_run'); // 已选模型不丢失
    expect(result.current.models).toHaveLength(3); // 清单不丢失
    expect(result.current.recordsLoading).toBe(false);
    expect(result.current.error).toBeNull(); // 清单轨与记录轨错误互不顶替
  });
});

describe('useDbInspector：加载态、翻页竞态取消守卫与 refresh 严格重发', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('未选中模型静置：recordsLoading 保持 false、recordsError 为 null（无幽灵加载态）', async () => {
    mockDispatch(envelopes(10));
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.recordsLoading).toBe(false);
    expect(result.current.recordsError).toBeNull();
    expect(result.current.records).toEqual([]);
    expect(countOf('db_records')).toBe(0);
  });

  it('选中模型取数进行中：recordsLoading 置位，完成后清除并落页', async () => {
    let resolveRecords: (records: RecordEnvelope[]) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return new Promise<RecordEnvelope[]>((resolve) => {
        resolveRecords = resolve;
      });
    });
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.selectModel('agent_run');
    });
    await waitFor(() => expect(result.current.recordsLoading).toBe(true));
    expect(result.current.recordsError).toBeNull(); // 在途不误报错误

    await act(async () => {
      resolveRecords(envelopes(5));
    });
    expect(result.current.recordsLoading).toBe(false);
    expect(result.current.records).toHaveLength(5);
    expect(result.current.records[0]?.key).toBe(0);
  });

  it('翻页竞态：旧页响应迟到不顶替新页（取消守卫生效，新页数据不被污染）', async () => {
    const resolvers: Array<(records: RecordEnvelope[]) => void> = [];
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return new Promise<RecordEnvelope[]>((resolve) => {
        resolvers.push(resolve);
      });
    });
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.selectModel('agent_run');
    });
    await waitFor(() => expect(result.current.recordsLoading).toBe(true)); // 第 1 页在途

    act(() => {
      result.current.nextPage(); // 翻页即取消第 1 页请求（cleanup 置 cancelled）
    });
    await waitFor(() => expect(resolvers.length).toBe(2)); // 第 2 页请求已发出

    await act(async () => {
      resolvers[1](envelopes(3, 50)); // 新页（offset 50）先返回
    });
    await waitFor(() => expect(result.current.records[0]?.key).toBe(50));

    await act(async () => {
      resolvers[0](envelopes(3, 0)); // 旧页迟到返回
    });
    // 取消守卫：旧页响应被丢弃，不顶替新页、不扰加载态
    expect(result.current.records[0]?.key).toBe(50);
    expect(result.current.records).toHaveLength(3);
    expect(result.current.recordsLoading).toBe(false);
  });

  it('翻页竞态（错误向）：旧页迟到失败不置 recordsError、新页数据不丢', async () => {
    const resolvers: Array<(records: RecordEnvelope[]) => void> = [];
    const rejectors: Array<(error: unknown) => void> = [];
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return new Promise<RecordEnvelope[]>((resolve, reject) => {
        resolvers.push(resolve);
        rejectors.push(reject);
      });
    });
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.selectModel('agent_run');
    });
    await waitFor(() => expect(result.current.recordsLoading).toBe(true));

    act(() => {
      result.current.nextPage();
    });
    await waitFor(() => expect(resolvers.length).toBe(2));

    await act(async () => {
      resolvers[1](envelopes(3, 50)); // 新页成功返回
    });
    await waitFor(() => expect(result.current.records[0]?.key).toBe(50));

    await act(async () => {
      rejectors[0](new Error('旧请求迟到失败')); // 已取消的旧请求后置失败
    });
    // 取消守卫：迟到失败不置错、数据不丢、无幽灵加载态
    expect(result.current.recordsError).toBeNull();
    expect(result.current.records[0]?.key).toBe(50);
    expect(result.current.recordsLoading).toBe(false);
  });

  it('refresh 连按两次：每次均重发当前页取数（tick 严格递增，第二次不失效）', async () => {
    mockDispatch(envelopes(60));
    const { result } = renderHook(() => useDbInspector());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await selectModel(result, 'agent_run');
    await waitFor(() => expect(countOf('db_records')).toBe(1));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(countOf('db_records')).toBe(2));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(countOf('db_records')).toBe(3));

    expect(lastParamsOf('db_records')).toEqual({ model: 'agent_run', offset: 0, limit: 50 });
    expect(result.current.records).toHaveLength(50);
  });
});
