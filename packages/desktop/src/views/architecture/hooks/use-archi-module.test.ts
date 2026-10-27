import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiModuleDetail } from '../../../types/generated/bindings';
import { useArchiDoc, useArchiModule } from './use-archi-module';

/**
 * `useArchiModule` / `useArchiDoc` 单测（AC-6 / AC-18）：`fqn` 变化即重取、
 * `null` 不发 invoke、错误面落 `error`、正文按需取（展开才 invoke）与 `reset`
 * 清态、零 watch 零轮询。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（按命令名分发 fixture）。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

/** 元素详情 fixture（含邻接 / 子元素 / 文档）。 */
function detailOf(fqn: string): ArchiModuleDetail {
  return {
    element: {
      fqn,
      kind: 'domain',
      gate: 'strict',
      path: ['./src/auth/'],
      summary: '认证域',
      metadata: { owner: 'core' },
      modelFile: 'openspec/architecture/models/01-core.toml',
      modelLine: 6,
      mark: 'none',
    },
    dependsOn: [{ target: 'App.Billing', description: '认证调用计费' }],
    dependedBy: [],
    children: [`${fqn}.Token`],
    docs: [
      {
        path: 'src/auth/README.md',
        title: '认证说明',
        kind: 'md',
        source: 'explicit',
      },
      {
        path: 'openspec/architecture/decisions/0007-token-sink.md',
        title: '令牌收口',
        kind: 'adr',
        source: 'text',
      },
    ],
    warnings: [],
  };
}

function callsOf(command: string): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === command)
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_module') {
      const element = typeof params?.element === 'string' ? params.element : '';
      return Promise.resolve(detailOf(element));
    }
    if (command === 'archi_read_doc') {
      return Promise.resolve('# 令牌收口\n\n- 正文\n\n```ts\ncode\n```\n');
    }
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
});

describe('useArchiModule 取数', () => {
  it('应在 root 与 fqn 均在场时发起一次 archi_module 并落地详情', async () => {
    const { result } = renderHook(() => useArchiModule('C:\\demo\\alpha', 'App.Auth'));

    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(result.current.detail?.element.fqn).toBe('App.Auth');
    expect(result.current.detail?.dependsOn).toHaveLength(1);
    expect(result.current.detail?.docs).toHaveLength(2);
    const calls = callsOf('archi_module');
    expect(calls).toHaveLength(1);
    expect(calls[0]).toEqual({ root: 'C:\\demo\\alpha', element: 'App.Auth' });
  });

  it('应在 fqn 或 root 变化时重发，同一 fqn 重渲不重复 invoke', async () => {
    const { result, rerender } = renderHook(
      ({ root, fqn }: { root: string; fqn: string }) => useArchiModule(root, fqn),
      { initialProps: { root: 'C:demoalpha', fqn: 'App.Auth' } },
    );
    await waitFor(() => expect(result.current.detail).not.toBeNull());

    rerender({ root: 'C:demoalpha', fqn: 'App.Billing' });
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(2));
    expect(callsOf('archi_module')[1]).toEqual({
      root: 'C:demoalpha',
      element: 'App.Billing',
    });

    rerender({ root: 'C:demo\beta', fqn: 'App.Billing' });
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(3));
    expect(callsOf('archi_module')[2]).toEqual({
      root: 'C:demo\beta',
      element: 'App.Billing',
    });

    // 同一 (root, fqn) 重渲不重复取数
    rerender({ root: 'C:demo\beta', fqn: 'App.Billing' });
    await act(async () => {
      await Promise.resolve();
    });
    expect(callsOf('archi_module')).toHaveLength(3);
  });

  it('应在 fqn 或 root 为 null 时不发 invoke 且 detail 为 null', async () => {
    const onlyFqnNull = renderHook(() => useArchiModule('C:\\demo\\alpha', null));
    const onlyRootNull = renderHook(() => useArchiModule(null, 'App.Auth'));
    const bothNull = renderHook(() => useArchiModule(null, null));
    await act(async () => {
      await Promise.resolve();
    });
    expect(callsOf('archi_module')).toHaveLength(0);
    expect(onlyFqnNull.result.current.detail).toBeNull();
    expect(onlyRootNull.result.current.detail).toBeNull();
    expect(bothNull.result.current.detail).toBeNull();
  });

  it('应把 reject 收进 error 并在切到有效 fqn 后清空', async () => {
    invokeMock.mockImplementationOnce(() => Promise.reject(new Error('元素不存在：App.Missing')));
    const { result, rerender } = renderHook(
      ({ fqn }: { fqn: string }) => useArchiModule('C:\\demo\\alpha', fqn),
      { initialProps: { fqn: 'App.Missing' } },
    );
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('元素不存在');
    expect(result.current.detail).toBeNull();
    expect(result.current.loading).toBe(false);

    rerender({ fqn: 'App.Auth' });
    // 等待信号取「成功后才有」的详情：`error` 在 effect 起始即同步清空，
    // 以它作信号会在成功分支落状态前通过（与 use-archi-graph 同源竞态）。
    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(result.current.error).toBeNull();
    expect(result.current.detail?.element.fqn).toBe('App.Auth');
  });
});

describe('useArchiModule 加载态与竞态', () => {
  it('应在新 fqn 取数未回时置空 detail 并保持 loading（不展示上一元素详情）', async () => {
    const pending: ((detail: ArchiModuleDetail) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiModuleDetail>((resolve) => {
          pending.push(resolve);
        }),
    );
    const { result, rerender } = renderHook(
      ({ fqn }: { fqn: string }) => useArchiModule('C:\\demo\\alpha', fqn),
      { initialProps: { fqn: 'App.Auth' } },
    );

    await act(async () => {
      pending[0]?.(detailOf('App.Auth'));
    });
    await waitFor(() => expect(result.current.detail?.element.fqn).toBe('App.Auth'));
    expect(result.current.loading).toBe(false);

    rerender({ fqn: 'App.Billing' });
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(2));
    expect(result.current.loading).toBe(true);
    expect(result.current.detail).toBeNull();

    await act(async () => {
      pending[1]?.(detailOf('App.Billing'));
    });
    await waitFor(() => expect(result.current.detail?.element.fqn).toBe('App.Billing'));
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('应丢弃被替换请求的迟到应答（旧 fqn 结果不覆盖新 fqn 详情）', async () => {
    const pending: ((detail: ArchiModuleDetail) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiModuleDetail>((resolve) => {
          pending.push(resolve);
        }),
    );
    const { result, rerender } = renderHook(
      ({ fqn }: { fqn: string }) => useArchiModule('C:\\demo\\alpha', fqn),
      { initialProps: { fqn: 'App.Auth' } },
    );
    rerender({ fqn: 'App.Billing' });
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(2));

    await act(async () => {
      pending[1]?.(detailOf('App.Billing'));
    });
    await waitFor(() => expect(result.current.detail?.element.fqn).toBe('App.Billing'));

    // 旧请求迟到：不得回写
    await act(async () => {
      pending[0]?.(detailOf('App.Auth'));
    });
    expect(result.current.detail?.element.fqn).toBe('App.Billing');
  });
});

describe('useArchiDoc 展开语义补充', () => {
  it('应在未展开时保持零 loading / 零正文 / 零错误（按需取而非预取）', async () => {
    const { result } = renderHook(() => useArchiDoc('C:\\demo\\alpha'));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.body).toBeNull();
    expect(result.current.error).toBeNull();
    expect(callsOf('archi_read_doc')).toHaveLength(0);
  });
  it('应在正文取数未回时置 body 为 null 且 loading 为真，落定后为假', async () => {
    const pending: ((body: string) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<string>((resolve) => {
          pending.push(resolve);
        }),
    );
    const { result } = renderHook(() => useArchiDoc('C:\\demo\\alpha'));
    act(() => result.current.load('a.md'));
    expect(result.current.loading).toBe(true);
    expect(result.current.body).toBeNull();

    await act(async () => {
      pending[0]?.('# A\n');
    });
    await waitFor(() => expect(result.current.body).toBe('# A\n'));
    expect(result.current.loading).toBe(false);

    // 换条目：新请求未回时不留上一份正文
    act(() => result.current.load('b.md'));
    expect(result.current.loading).toBe(true);
    expect(result.current.body).toBeNull();
    await act(async () => {
      pending[1]?.('# B\n');
    });
    await waitFor(() => expect(result.current.body).toBe('# B\n'));
  });

  it('应在 root 变化后以新根取正文（取数闭包随根更新）', async () => {
    const { result, rerender } = renderHook(
      ({ root }: { root: string | null }) => useArchiDoc(root),
      { initialProps: { root: 'C:\\demo\\alpha' as string | null } },
    );
    rerender({ root: 'C:\\demo\\beta' });
    act(() => result.current.load('a.md'));
    await waitFor(() => expect(result.current.body).not.toBeNull());
    expect(callsOf('archi_read_doc').at(-1)).toEqual({
      root: 'C:\\demo\\beta',
      path: 'a.md',
    });
  });
});

describe('useArchiDoc 按需取正文', () => {
  it('应初始为空且零 invoke，load 后才发一次 archi_read_doc', async () => {
    const { result } = renderHook(() => useArchiDoc('C:\\demo\\alpha'));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.body).toBeNull();
    expect(callsOf('archi_read_doc')).toHaveLength(0);

    act(() => result.current.load('openspec/architecture/decisions/0007-token-sink.md'));
    await waitFor(() => expect(result.current.body).not.toBeNull());
    expect(result.current.body).toContain('令牌收口');
    expect(callsOf('archi_read_doc')).toEqual([
      {
        root: 'C:\\demo\\alpha',
        path: 'openspec/architecture/decisions/0007-token-sink.md',
      },
    ]);
  });

  it('应在连续 load 两个路径时各发一次且后到者覆盖 body，reset 清态不再 invoke', async () => {
    const { result } = renderHook(() => useArchiDoc('C:\\demo\\alpha'));
    act(() => result.current.load('a.md'));
    await waitFor(() => expect(result.current.body).not.toBeNull());
    act(() => result.current.load('b.md'));
    await waitFor(() => expect(callsOf('archi_read_doc')).toHaveLength(2));

    act(() => result.current.reset());
    await waitFor(() => expect(result.current.body).toBeNull());
    expect(result.current.error).toBeNull();
    expect(callsOf('archi_read_doc')).toHaveLength(2);
  });

  it('应在 root 为 null 时给出错误且不发 invoke', async () => {
    const { result } = renderHook(() => useArchiDoc(null));
    act(() => result.current.load('src/auth/README.md'));
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('无工作根');
    expect(callsOf('archi_read_doc')).toHaveLength(0);
  });

  it('应把 reject 收进 error 且 body 不残留上一份正文', async () => {
    const { result } = renderHook(() => useArchiDoc('C:\\demo\\alpha'));
    act(() => result.current.load('ok.md'));
    await waitFor(() => expect(result.current.body).not.toBeNull());

    invokeMock.mockImplementationOnce(() => Promise.reject(new Error('路径不在模块文档关联集内')));
    act(() => result.current.load('openspec/changes/x/proposal.md'));
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('路径不在模块文档关联集内');
    expect(result.current.body).toBeNull();
  });

  it('应在假计时器推进下零重发（无 watch 无轮询）', async () => {
    vi.useFakeTimers();
    try {
      const { result } = renderHook(() => useArchiDoc('C:\\demo\\alpha'));
      act(() => result.current.load('a.md'));
      await act(async () => {
        await Promise.resolve();
      });
      const before = callsOf('archi_read_doc').length;
      await act(async () => {
        vi.advanceTimersByTime(60_000);
      });
      expect(callsOf('archi_read_doc')).toHaveLength(before);
    } finally {
      vi.useRealTimers();
    }
  });
});
