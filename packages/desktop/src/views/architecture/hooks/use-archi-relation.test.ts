import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiRelationDetail } from '../../../types/generated/bindings';
import { useArchiRelation, type RelationEdge } from './use-archi-relation';

/**
 * `useArchiRelation` 单测（`relation-api-dependency-check` / AC-7 / D13 / D17）：按需
 * 取数——无根 / 无选中即零 invoke、同一元素对零重复 invoke（引用缓存）、失败保留
 * `error` 供重试、`retry()` 强制重取、零 watch 零轮询。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（生成的 `commands.archiRelation`
 * 最终经此调用），按命令名分发 fixture 并记录调用序列与载荷。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

/** 关系详情 fixture（两桶各一项，覆盖全部十二字段）。 */
function detailOf(from: string, to: string): ArchiRelationDetail {
  return {
    from,
    to,
    description: '认证调用计费',
    mark: 'none',
    reason: null,
    modelFile: '.dev-team/architecture/graph/model/06-frontend.toml',
    modelLine: 188,
    declared: ['AgentInput'],
    used: ['AgentInput', 'SessionPanel'],
    usedNotDeclared: ['SessionPanel'],
    declaredNotUsed: [],
    occurrences: [{ file: 'src/views/a.ts', line: 3, specifier: '../b', typeOnly: false }],
  };
}

/** 取数调用序列（命令名 + 入参）。 */
function calls(): { command: string; params?: Record<string, unknown> }[] {
  return invokeMock.mock.calls.map(([command, params]) => ({
    command: command as string,
    params: params as Record<string, unknown> | undefined,
  }));
}

/** `archi_relation` 的入参清单（按调用序）。 */
function relationCalls(): Record<string, unknown>[] {
  return calls()
    .filter((call) => call.command === 'archi_relation')
    .map((call) => call.params ?? {});
}

const EDGE: RelationEdge = { from: 'App.Frontend.Canvas', to: 'App.Frontend.GeneratedBindings' };
const OTHER: RelationEdge = { from: 'App.Frontend.Canvas', to: 'App.Frontend.Panel' };

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_relation') {
      const from = typeof params?.from === 'string' ? params.from : '';
      const to = typeof params?.to === 'string' ? params.to : '';
      return Promise.resolve(detailOf(from, to));
    }
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
});

describe('useArchiRelation 取数', () => {
  it('应在根与选中边就位时恰取一次 archi_relation 并落地详情', async () => {
    const { result } = renderHook(() => useArchiRelation('C:\\demo\\alpha', EDGE));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.detail).toEqual(detailOf(EDGE.from, EDGE.to));
    expect(result.current.error).toBeNull();
    expect(result.current.detail?.usedNotDeclared).toEqual(['SessionPanel']);
    expect(relationCalls()).toEqual([{ root: 'C:\\demo\\alpha', from: EDGE.from, to: EDGE.to }]);
  });

  it('应在同一元素对重复选中时零新增 invoke（值身份，不依赖对象引用）', async () => {
    const { result, rerender } = renderHook(
      ({ edge }: { edge: RelationEdge }) => useArchiRelation('C:\\demo\\alpha', edge),
      { initialProps: { edge: { ...EDGE } } },
    );
    await waitFor(() => expect(result.current.detail).not.toBeNull());

    for (let index = 0; index < 1000; index += 1) {
      rerender({ edge: { from: EDGE.from, to: EDGE.to } });
    }

    expect(relationCalls()).toHaveLength(1);
    expect(result.current.detail).not.toBeNull();
  });

  it('应在切到另一对再切回时命中缓存（跨面板两态切换零新增 invoke）', async () => {
    const { result, rerender } = renderHook(
      ({ edge }: { edge: RelationEdge }) => useArchiRelation('C:\\demo\\alpha', edge),
      { initialProps: { edge: EDGE } },
    );
    await waitFor(() => expect(result.current.detail).not.toBeNull());

    rerender({ edge: OTHER });
    await waitFor(() => expect(result.current.detail?.to).toBe(OTHER.to));

    rerender({ edge: EDGE });
    await waitFor(() => expect(result.current.detail?.to).toBe(EDGE.to));

    expect(relationCalls()).toHaveLength(2);
    expect(relationCalls().map((params) => params.to)).toEqual([EDGE.to, OTHER.to]);
  });

  it('应在 retry 时删缓存键并重新取一次（成功后 error 归零）', async () => {
    invokeMock.mockReset();
    invokeMock.mockRejectedValueOnce(new Error('首次取数失败'));
    invokeMock.mockImplementation(() => Promise.resolve(detailOf(EDGE.from, EDGE.to)));

    const { result } = renderHook(() => useArchiRelation('C:\\demo\\alpha', EDGE));
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.detail).toBeNull();

    act(() => result.current.retry());
    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(result.current.error).toBeNull();
    expect(relationCalls()).toHaveLength(2);
  });

  it('应在根为空时零 invoke 且三态复位', async () => {
    const { result } = renderHook(() => useArchiRelation(null, EDGE));
    await act(async () => {
      await Promise.resolve();
    });
    expect(relationCalls()).toHaveLength(0);
    expect(result.current.detail).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it('应在无选中边时零 invoke（未声明 apis 的边由调用方传 null）', async () => {
    const { result } = renderHook(() => useArchiRelation('C:\\demo\\alpha', null));
    await act(async () => {
      await Promise.resolve();
    });
    expect(relationCalls()).toHaveLength(0);
    expect(result.current.detail).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it('应在根变化时以新根重新取数（缓存键含根，旧缓存不串味）', async () => {
    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useArchiRelation(root, EDGE),
      { initialProps: { root: 'C:\\demo\\alpha' } },
    );
    await waitFor(() => expect(result.current.detail).not.toBeNull());

    rerender({ root: 'C:\\demo\\beta' });
    await waitFor(() => expect(relationCalls()).toHaveLength(2));
    expect(relationCalls().map((params) => params.root)).toEqual([
      'C:\\demo\\alpha',
      'C:\\demo\\beta',
    ]);
  });

  it('应在超长元素名时原样作载荷（零 panic）', async () => {
    const long = 'N'.repeat(1100);
    const { result } = renderHook(() =>
      useArchiRelation('C:\\demo\\alpha', { from: long, to: long }),
    );
    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(relationCalls()).toEqual([{ root: 'C:\\demo\\alpha', from: long, to: long }]);
  });
});

describe('useArchiRelation 失败与竞态', () => {
  it('应在 reject 时落 error、清空详情且不写缓存（再次选中重新取数）', async () => {
    invokeMock.mockReset();
    invokeMock.mockRejectedValueOnce(new Error('命令失败'));
    invokeMock.mockImplementation((_command: string, params?: Record<string, unknown>) =>
      Promise.resolve(
        detailOf(
          typeof params?.from === 'string' ? params.from : '',
          typeof params?.to === 'string' ? params.to : '',
        ),
      ),
    );

    const { result, rerender } = renderHook(
      ({ edge }: { edge: RelationEdge }) => useArchiRelation('C:\\demo\\alpha', edge),
      { initialProps: { edge: EDGE } },
    );
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.detail).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toContain('命令失败');

    rerender({ edge: OTHER });
    await waitFor(() => expect(result.current.detail?.to).toBe(OTHER.to));
    rerender({ edge: EDGE });
    await waitFor(() => expect(result.current.detail?.to).toBe(EDGE.to));
    expect(relationCalls()).toHaveLength(3);
  });

  it('应只落最后一次选中态的结果（旧回包零写入）', async () => {
    const pending: ((detail: ArchiRelationDetail) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      (command: string, params?: Record<string, unknown>) =>
        new Promise<ArchiRelationDetail>((resolve) => {
          pending.push((detail) => resolve(detail));
          void command;
          void params;
        }),
    );

    const { result, rerender } = renderHook(
      ({ edge }: { edge: RelationEdge }) => useArchiRelation('C:\\demo\\alpha', edge),
      { initialProps: { edge: EDGE } },
    );
    rerender({ edge: OTHER });
    await waitFor(() => expect(pending).toHaveLength(2));

    // 后发的 B 对先回
    await act(async () => {
      pending[1]?.(detailOf(OTHER.from, OTHER.to));
    });
    await waitFor(() => expect(result.current.detail?.to).toBe(OTHER.to));

    // 先发的 A 对迟到：不得回写
    await act(async () => {
      pending[0]?.(detailOf(EDGE.from, EDGE.to));
    });
    expect(result.current.detail?.to).toBe(OTHER.to);
    expect(result.current.loading).toBe(false);
  });

  it('应在卸载后回包时零 setState（不产生告警）', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const pending: ((detail: ArchiRelationDetail) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise<ArchiRelationDetail>((resolve) => {
          pending.push(resolve);
        }),
    );

    const { unmount } = renderHook(() => useArchiRelation('C:\\demo\\alpha', EDGE));
    await waitFor(() => expect(pending).toHaveLength(1));
    unmount();
    await act(async () => {
      pending[0]?.(detailOf(EDGE.from, EDGE.to));
    });
    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });
});

describe('RelationEdge 身份与状态面', () => {
  it('应把 from / to 顺序不同视为不同元素对（零排序归一）', async () => {
    const forward: RelationEdge = { from: 'App.A', to: 'App.B' };
    const reversed: RelationEdge = { from: 'App.B', to: 'App.A' };
    const { result, rerender } = renderHook(
      ({ edge }: { edge: RelationEdge }) => useArchiRelation('C:\\demo\\alpha', edge),
      { initialProps: { edge: forward } },
    );
    await waitFor(() => expect(result.current.detail).not.toBeNull());

    rerender({ edge: reversed });
    await waitFor(() => expect(result.current.detail?.from).toBe('App.B'));
    expect(relationCalls()).toHaveLength(2);
  });

  it('应在元素名含箭头字符时仍按 (root, from, to) 三元拼键（零串味）', async () => {
    const pairA: RelationEdge = { from: 'a->b', to: 'c' };
    const pairB: RelationEdge = { from: 'a', to: 'b->c' };
    const { result, rerender } = renderHook(
      ({ edge }: { edge: RelationEdge }) => useArchiRelation('C:\\demo\\alpha', edge),
      { initialProps: { edge: pairA } },
    );
    await waitFor(() => expect(result.current.detail).not.toBeNull());

    rerender({ edge: pairB });
    await waitFor(() => expect(result.current.detail?.from).toBe('a'));
    expect(relationCalls()).toHaveLength(2);
    expect(relationCalls()[0]).toEqual({
      root: 'C:\\demo\\alpha',
      from: 'a->b',
      to: 'c',
    });
    expect(relationCalls()[1]).toEqual({
      root: 'C:\\demo\\alpha',
      from: 'a',
      to: 'b->c',
    });
  });

  it('应四字段恒在场（detail / loading / error / retry）', async () => {
    const { result } = renderHook(() => useArchiRelation('C:\\demo\\alpha', EDGE));
    expect(result.current).toHaveProperty('detail');
    expect(result.current).toHaveProperty('loading');
    expect(result.current).toHaveProperty('error');
    expect(typeof result.current.retry).toBe('function');
    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(result.current).toHaveProperty('retry');
  });

  it('应在切到另一对并失败时清空上一对详情', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((_command: string, params?: Record<string, unknown>) => {
      if (params?.to === OTHER.to) return Promise.reject(new Error('第二对失败'));
      return Promise.resolve(detailOf(EDGE.from, EDGE.to));
    });

    const { result, rerender } = renderHook(
      ({ edge }: { edge: RelationEdge }) => useArchiRelation('C:\\demo\\alpha', edge),
      { initialProps: { edge: EDGE } },
    );
    await waitFor(() => expect(result.current.detail).not.toBeNull());

    rerender({ edge: OTHER });
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.detail).toBeNull();
    expect(result.current.error).toContain('第二对失败');
  });
});

describe('零 watch 零轮询纪律', () => {
  it('应在源码内零定时器与零订阅（取数只由显式动作发起）', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/views/architecture/hooks/use-archi-relation.ts'),
      'utf8',
    );
    for (const forbidden of [
      'setInterval(',
      'setTimeout(',
      'subscribe(',
      'requestAnimationFrame(',
    ]) {
      expect(source).not.toContain(forbidden);
    }
    expect(source).toContain('useEffect');
  });
});
