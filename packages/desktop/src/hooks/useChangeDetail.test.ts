import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArtifactDescriptor, ArtifactEnvelope, ChangeDetail } from '../types/dto';
import { useChangeDetail, type ChangeDetailState } from './useChangeDetail';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

function fakeDetail(artifacts: ArtifactDescriptor[]): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: '2026-09-01',
    unparsable: false,
    pipeline: [
      {
        phase: 'proposal',
        attempts: [
          {
            attempt: 1,
            verdict: 'pass',
            report: '通过',
            checklist: [{ item: '问题清晰', pass: true, evidence: 'L1-10' }],
            skipped: false,
            stale: false,
            startAt: null,
            timestamp: '2026-09-01T00:00:00Z',
            backtrackTo: null,
            backtrackReason: null,
          },
        ],
      },
    ],
    activePhase: null,
    interrupted: [],
    fileLog: [],
    artifacts,
  };
}

const twoArtifacts: ArtifactDescriptor[] = [
  { kind: 'tasks-progress', source: 'tasks.md', title: '任务进度' },
  { kind: 'markdown-doc', source: 'proposal.md', title: '提案' },
];

function envelopeFor(descriptor: ArtifactDescriptor) {
  return {
    kind: descriptor.kind,
    version: 1,
    title: descriptor.title,
    payload: { markdown: `# ${descriptor.title}` },
    fallbackText: `保底：${descriptor.title}`,
  };
}

describe('useChangeDetail：详情取数与产物信封同周期组装（AC-6 / AC-7）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('refresh 触发 invoke("get_change_detail")，detail 更新', async () => {
    invokeMock.mockResolvedValue(fakeDetail([]));
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));

    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
      root: '/repo',
      change: 'add-feature',
    });
    expect(result.current.loading).toBe(false);
  });

  it('详情返回产物清单后同一周期内逐个 invoke("read_artifact") 并组装信封', async () => {
    // 按命令名 + 参数精确分发
    invokeMock.mockImplementation((command: string, params: { kind?: string; source?: string }) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(fakeDetail(twoArtifacts));
      }
      const descriptor = twoArtifacts.find(
        (d) => d.kind === params.kind && d.source === params.source,
      );
      return Promise.resolve(descriptor ? envelopeFor(descriptor) : null);
    });

    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));

    await waitFor(() => expect(result.current.artifacts).toHaveLength(2));
    const readCalls = invokeMock.mock.calls.filter(([name]) => name === 'read_artifact');
    expect(readCalls).toHaveLength(2);
    expect(readCalls[0][1]).toEqual({
      root: '/repo',
      change: 'add-feature',
      kind: 'tasks-progress',
      source: 'tasks.md',
    });
    expect(readCalls[1][1]).toEqual({
      root: '/repo',
      change: 'add-feature',
      kind: 'markdown-doc',
      source: 'proposal.md',
    });
    expect(result.current.artifacts[0].kind).toBe('tasks-progress');
    expect(result.current.artifacts[1].fallbackText).toBe('保底：提案');
  });

  it('单个 read_artifact reject 时该产物以 Fallback 形态呈现，其余正常组装', async () => {
    invokeMock.mockImplementation((command: string, params: { kind?: string; source?: string }) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(fakeDetail(twoArtifacts));
      }
      if (params.kind === 'tasks-progress') {
        return Promise.reject(new Error('读取失败'));
      }
      const descriptor = twoArtifacts.find((d) => d.kind === params.kind);
      return Promise.resolve(descriptor ? envelopeFor(descriptor) : null);
    });

    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));

    await waitFor(() => expect(result.current.artifacts).toHaveLength(2));
    const fallback = result.current.artifacts[0];
    expect(fallback.kind).toBe('tasks-progress');
    expect(fallback.version).toBe(0);
    expect(fallback.payload).toBeNull();
    expect(fallback.fallbackText).toContain('产物读取失败');
    expect(fallback.fallbackText).toContain('tasks-progress');
    // 其余产物不受阻断
    expect(result.current.artifacts[1].version).toBe(1);
    expect(result.current.error).toBeNull();
  });

  it('get_change_detail 返回 null 时 detail 为 null 且不发起任何 read_artifact', async () => {
    invokeMock.mockResolvedValue(null);
    const { result } = renderHook(() => useChangeDetail('/repo', 'ghost'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.detail).toBeNull();
    expect(result.current.artifacts).toEqual([]);
    // 返回 null 不是错误：error 保持空、loading 复位
    expect(result.current.error).toBeNull();
    expect(invokeMock.mock.calls.every(([name]) => name === 'get_change_detail')).toBe(true);
  });

  it('change 为 null 时不发起任何 invoke', () => {
    renderHook(() => useChangeDetail('/repo', null));
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('产物清单为空数组时 artifacts 为空数组且无额外 invoke', async () => {
    invokeMock.mockResolvedValue(fakeDetail([]));
    const { result } = renderHook(() => useChangeDetail('/repo', 'empty'));

    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(result.current.artifacts).toEqual([]);
    expect(invokeMock.mock.calls.every(([name]) => name === 'get_change_detail')).toBe(true);
  });

  it('除显式刷新外无轮询：推进虚拟计时后 invoke 次数不增长', async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue(fakeDetail([]));
    renderHook(() => useChangeDetail('/repo', 'calm'));
    await act(async () => {});
    const calls = invokeMock.mock.calls.length;
    expect(calls).toBeGreaterThan(0);

    act(() => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length).toBe(calls);
  });
});

describe('useChangeDetail：加载态、降级信封、错误路径与刷新竞态', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('挂载后请求未返回前 loading 置位且 artifacts 初始为空数组', () => {
    invokeMock.mockImplementation(() => new Promise<ChangeDetail>(() => {}));
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    expect(result.current.loading).toBe(true);
    expect(result.current.artifacts).toEqual([]);
    expect(result.current.detail).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('read_artifact 返回 null（非 reject）时该产物降级为 Fallback 信封', async () => {
    const descriptor: ArtifactDescriptor = {
      kind: 'markdown-doc',
      source: 'proposal.md',
      title: '提案',
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(fakeDetail([descriptor]));
      }
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));

    await waitFor(() => expect(result.current.artifacts).toHaveLength(1));
    const fallback = result.current.artifacts[0];
    expect(fallback.kind).toBe('markdown-doc');
    expect(fallback.title).toBe('提案');
    expect(fallback.version).toBe(0);
    expect(fallback.payload).toBeNull();
    expect(fallback.fallbackText).toContain('产物读取失败');
    expect(fallback.fallbackText).toContain('markdown-doc');
    expect(fallback.fallbackText).toContain('proposal.md');
    expect(result.current.error).toBeNull();
  });

  it('get_change_detail reject 时置错误态、清空数据并复位 loading', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));
    const { result } = renderHook(() => useChangeDetail('/repo', 'broken'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.detail).toBeNull();
    expect(result.current.artifacts).toEqual([]);
  });

  it('change 切回 null 时重置 detail / artifacts / loading / error', async () => {
    const descriptor: ArtifactDescriptor = {
      kind: 'tasks-progress',
      source: 'tasks.md',
      title: '任务进度',
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(fakeDetail([descriptor]));
      }
      return Promise.resolve(envelopeFor(descriptor));
    });
    const { result, rerender } = renderHook<ChangeDetailState, { change: string | null }>(
      (props) => useChangeDetail('/repo', props.change),
      { initialProps: { change: 'add-feature' } },
    );
    await waitFor(() => expect(result.current.artifacts).toHaveLength(1));

    rerender({ change: null });
    await waitFor(() => expect(result.current.artifacts).toEqual([]));
    expect(result.current.detail).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('显式 refresh 触发重新拉取详情', async () => {
    invokeMock.mockResolvedValue(fakeDetail([]));
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await waitFor(() => expect(result.current.detail).not.toBeNull());
    const callsAfterFirst = invokeMock.mock.calls.length;

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(callsAfterFirst + 1));
    expect(result.current.detail).not.toBeNull();
    await waitFor(() => expect(result.current.loading).toBe(false));

    // 连续两次 refresh 必须各自触发一次重新拉取
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(callsAfterFirst + 2));
  });

  it('刷新后旧响应被丢弃：仅最后一次刷新的结果生效', async () => {
    let resolveFirst: ((value: ChangeDetail) => void) | null = null;
    let callCount = 0;
    invokeMock.mockImplementation(() => {
      callCount += 1;
      if (callCount === 1) {
        return new Promise<ChangeDetail>((resolve) => {
          resolveFirst = resolve;
        });
      }
      // 第二次刷新后的请求永不返回，用于观察旧响应是否被正确丢弃
      return new Promise<ChangeDetail>(() => {});
    });

    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    expect(result.current.loading).toBe(true);

    act(() => {
      result.current.refresh();
    });
    await act(async () => {});
    expect(callCount).toBe(2);

    // 解析已被丢弃的第一次响应
    await act(async () => {
      resolveFirst?.(fakeDetail([]));
    });
    expect(result.current.detail).toBeNull();
    expect(result.current.loading).toBe(true);
    expect(result.current.error).toBeNull();
  });

  it('刷新后旧响应的产物读取结果同样被丢弃', async () => {
    let resolveArtifact: ((value: ArtifactEnvelope) => void) | null = null;
    let detailCallCount = 0;
    const descriptor: ArtifactDescriptor = {
      kind: 'tasks-progress',
      source: 'tasks.md',
      title: '任务进度',
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_change_detail') {
        detailCallCount += 1;
        if (detailCallCount === 1) {
          return Promise.resolve(fakeDetail([descriptor]));
        }
        return new Promise<ChangeDetail>(() => {});
      }
      // 旧周期的产物读取一直挂起，稍后手动 resolve
      return new Promise<ArtifactEnvelope>((resolve) => {
        resolveArtifact = resolve;
      });
    });

    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(result.current.artifacts).toEqual([]);

    act(() => {
      result.current.refresh();
    });
    await act(async () => {});
    expect(detailCallCount).toBe(2);

    await act(async () => {
      resolveArtifact?.(envelopeFor(descriptor));
    });
    // 旧周期读取到的信封不得写入新周期的状态
    expect(result.current.artifacts).toEqual([]);
    expect(result.current.loading).toBe(true);
  });

  it('刷新后旧请求的失败（reject）同样被丢弃', async () => {
    let rejectFirst: ((reason: unknown) => void) | null = null;
    let callCount = 0;
    invokeMock.mockImplementation(() => {
      callCount += 1;
      if (callCount === 1) {
        return new Promise<ChangeDetail>((_resolve, reject) => {
          rejectFirst = reject;
        });
      }
      return new Promise<ChangeDetail>(() => {});
    });

    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});

    act(() => {
      result.current.refresh();
    });
    await act(async () => {});
    expect(callCount).toBe(2);

    await act(async () => {
      rejectFirst?.(new Error('过期请求失败'));
    });
    // 已取消的失败不得进入错误态
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(true);
    expect(result.current.detail).toBeNull();
  });
});
