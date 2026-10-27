import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, driftReports, resetDrift } from '../lib/ipc-drift';
import { useAppEnvironment } from './use-app-environment';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

// 进程边界 Mock：生成物 `commands.appEnvironment()` 内部走 `invoke`，mock 该
// 边界即覆盖生成包装（不 mock 生成物模块本身）。沿 use-workspaces.test.ts /
// use-agent-options.test.ts 先例。
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

/** dev 构建的线面载荷（后端 `mode()` / `RootSource::wire()` 产出形态）。 */
function devEnvironment(): { mode: string; dataRoot: string; source: string } {
  return { mode: 'dev', dataRoot: 'C:\\u\\.dev-team\\dev', source: 'dev-default' };
}

/** 让挂载期 IPC 的 then / catch 链跑完（一次微任务冲洗，无轮询）。 */
async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useAppEnvironment：构建模式单源 hook（挂载一次 IPC + 载荷收窄）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(devEnvironment());
  });

  it('成功返回三字段：不加工、不改写大小写、不加尾分隔符', async () => {
    const { result } = renderHook(() => useAppEnvironment());

    await waitFor(() => expect(result.current).not.toBeNull());
    expect(result.current).toEqual({
      mode: 'dev',
      dataRoot: 'C:\\u\\.dev-team\\dev',
      source: 'dev-default',
    });
  });

  it('挂载恰一次：无第二次轮询、无额外命令调用', async () => {
    const { result } = renderHook(() => useAppEnvironment());

    await waitFor(() => expect(result.current).not.toBeNull());
    await flush();
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('app_environment');
  });

  it('mode 两值透传且值域闭集', async () => {
    for (const mode of ['dev', 'prod']) {
      invokeMock.mockReset();
      invokeMock.mockResolvedValue({ ...devEnvironment(), mode });
      const { result, unmount } = renderHook(() => useAppEnvironment());

      await waitFor(() => expect(result.current?.mode).toBe(mode));
      unmount();
    }
  });

  it('source 三值遍历原样透传（与 RootSource::wire 值域一致）', async () => {
    for (const source of ['prod', 'dev-default', 'env']) {
      invokeMock.mockReset();
      invokeMock.mockResolvedValue({ ...devEnvironment(), source });
      const { result, unmount } = renderHook(() => useAppEnvironment());

      await waitFor(() => expect(result.current?.source).toBe(source));
      unmount();
    }
  });

  it('invoke reject：静默返回 null，不 console.error、不抛、不产生错误态字段', async () => {
    const errorSpy = vi.spyOn(console, 'error');
    invokeMock.mockRejectedValue(new Error('IPC 不可用'));
    const { result } = renderHook(() => useAppEnvironment());

    await flush();
    expect(result.current).toBeNull();
    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it('多余字段不破坏合法性：只透出三字段，多余字段不透出、不报错', async () => {
    invokeMock.mockResolvedValue({
      ...devEnvironment(),
      extra: 1,
      nested: { a: 1 },
    });
    const { result } = renderHook(() => useAppEnvironment());

    await waitFor(() => expect(result.current).not.toBeNull());
    expect(Object.keys(result.current ?? {}).sort()).toEqual(['dataRoot', 'mode', 'source']);
    expect(result.current).toEqual({
      mode: 'dev',
      dataRoot: 'C:\\u\\.dev-team\\dev',
      source: 'dev-default',
    });
  });

  it('未返回（pending）不判定：初值 null，resolve 后回填三字段', async () => {
    let resolvePayload: (payload: unknown) => void = () => {};
    invokeMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolvePayload = resolve;
        }),
    );
    const { result } = renderHook(() => useAppEnvironment());

    await flush();
    expect(result.current).toBeNull(); // 未就绪 ≠ 错误

    await act(async () => {
      resolvePayload(devEnvironment());
    });
    await waitFor(() => expect(result.current?.mode).toBe('dev'));
  });

  it('值边界字符原样透传：空串 / 超长 / 中文与 emoji / 换行与反斜杠', async () => {
    const dataRoots = ['', 'C:\\中文目录\\🙂', 'C:\\a\nb\\\\c', 'x'.repeat(1200)];
    for (const dataRoot of dataRoots) {
      invokeMock.mockReset();
      invokeMock.mockResolvedValue({ mode: 'dev', dataRoot, source: 'env' });
      const { result, unmount } = renderHook(() => useAppEnvironment());

      await waitFor(() => expect(result.current).not.toBeNull());
      expect(result.current?.dataRoot).toBe(dataRoot);
      unmount();
    }
  });

  it('卸载后 resolve：不产生状态更新告警、不抛（cancelled 守卫）', async () => {
    const errorSpy = vi.spyOn(console, 'error');
    let resolvePayload: (payload: unknown) => void = () => {};
    invokeMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolvePayload = resolve;
        }),
    );
    const { unmount } = renderHook(() => useAppEnvironment());

    unmount();
    await act(async () => {
      resolvePayload(devEnvironment());
    });

    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it('返回形态恒为三键或 null，且 mode / source 值域闭合', async () => {
    const { result } = renderHook(() => useAppEnvironment());

    await waitFor(() => expect(result.current).not.toBeNull());
    const environment = result.current;
    expect(environment === null || typeof environment === 'object').toBe(true);
    expect(environment?.mode === 'dev' || environment?.mode === 'prod').toBe(true);
    expect(
      environment?.source === 'prod' ||
        environment?.source === 'dev-default' ||
        environment?.source === 'env',
    ).toBe(true);
  });
});

// 载荷合法性（AC-14 / D13）：非对象 / 缺字段 / 类型错 / 非法枚举名义值一律 null，
// 不抛。逐形态独立用例（失败定位到具体形态）。
describe('useAppEnvironment：载荷不合法一律 null', () => {
  const invalidPayloads: unknown[] = [
    null,
    [],
    'dev',
    5,
    true,
    {},
    { dataRoot: 'C:\\d', source: 'env' },
    { mode: 'dev', source: 'env' },
    { mode: 'dev', dataRoot: 'C:\\d' },
    { mode: 5, dataRoot: 'C:\\d', source: 'env' },
    { mode: 'dev', dataRoot: null, source: 'env' },
    { mode: 'dev', dataRoot: 'C:\\d', source: {} },
    { mode: 'DEV', dataRoot: 'C:\\d', source: 'env' },
    { mode: 'dev ', dataRoot: 'C:\\d', source: 'env' },
    { mode: 'dev', dataRoot: 'C:\\d', source: 'dev' },
  ];

  beforeEach(() => {
    invokeMock.mockReset();
  });

  for (const [index, payload] of invalidPayloads.entries()) {
    it(`非法载荷形态 #${String(index + 1)} → null 且不抛`, async () => {
      invokeMock.mockResolvedValue(payload);
      const { result } = renderHook(() => useAppEnvironment());

      await flush();
      expect(result.current).toBeNull();
    });
  }
});

// ---------------------------------------------------------------------------
// 经 `ipc` 的运行期校验面（change ipc-zod-runtime-schema）：零形参命令
// `appEnvironment` 的参数面放行与结果面 soft-report（旧版「失败静默为 null」
// 语义零改动）。
// ---------------------------------------------------------------------------

describe('useAppEnvironment：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(devEnvironment());
    resetDrift();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('挂载 → invoke("app_environment") 恰 1 次、载荷合法、零漂移', async () => {
    const { result } = renderHook(() => useAppEnvironment());
    await waitFor(() => expect(result.current).not.toBeNull());

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('app_environment');
    expect(result.current?.mode).toBe('dev');
    expect(driftCount()).toBe(0);
  });

  it('零形参命令参数面放行：无 IpcArgsError、payload 为空对象（非 undefined）', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { result } = renderHook(() => useAppEnvironment());
    await waitFor(() => expect(result.current).not.toBeNull());

    expect(invokeMock.mock.calls[0]).toEqual(['app_environment']);
    expect(errorSpy).not.toHaveBeenCalled();
    expect(driftCount()).toBe(0);
  });

  it('reject 串 → 返回 null、console.error 零调用（既有静默语义）、零漂移', async () => {
    invokeMock.mockRejectedValue('IPC 断开');
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { result } = renderHook(() => useAppEnvironment());
    await flush();

    expect(result.current).toBeNull();
    expect(errorSpy).not.toHaveBeenCalled();
    expect(driftCount()).toBe(0);
  });

  it('结果面契约外（dataRoot 给数字）→ 漂移 1 条 + 既有收窄仍返回 null', async () => {
    invokeMock.mockResolvedValue({ ...devEnvironment(), dataRoot: 42 });
    const { result } = renderHook(() => useAppEnvironment());
    await flush();

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('appEnvironment');
    expect(result.current).toBeNull();
  });

  it('结果面多余键（strictObject）→ 漂移 1 条、载荷仍按既有字段收窄', async () => {
    invokeMock.mockResolvedValue({ ...devEnvironment(), surplusKey: 'x' });
    const { result } = renderHook(() => useAppEnvironment());
    await waitFor(() => expect(result.current).not.toBeNull());

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].path).toBe('#');
    expect(result.current?.dataRoot).toBe('C:\\u\\.dev-team\\dev');
  });

  it('卸载前 resolve（cancelled 分支）→ 零状态写入、零漂移', async () => {
    const { unmount } = renderHook(() => useAppEnvironment());
    unmount();
    await flush();

    expect(driftCount()).toBe(0);
  });
});
